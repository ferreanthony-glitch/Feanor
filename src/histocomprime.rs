//! Les vieilles étapes d'annulation, comprimées (`docs/PERFORMANCE.md`, section 28).
//!
//! Une étape de dessin garde les tuiles d'avant (16 Ko chacune). Deux cents étapes de gros gestes (un remplissage, une
//! gomme sur toute la feuille) tiennent des centaines de mégaoctets que personne ne demandera de sitôt. Les étapes au-delà
//! des `GARDER` dernières (à annuler, et à rétablir) sont donc comprimées (deflate rapide, la caisse `flate2` déjà liée)
//! **sur un fil de fond** : le fil de l'interface ne fait que remplacer l'étape quand le résultat arrive. Les étapes
//! récentes ne sont jamais touchées : Ctrl+Z reste instantané. Une étape comprimée se décomprime à son application (de la
//! mémoire seulement, pas de disque : 20 à 40 µs par tuile), puis redevient une étape ordinaire.
//!
//! Rien ne se perd : le résultat garde les tuiles d'origine jusqu'à ce que l'étape soit remplacée, et l'étape n'est
//! remplacée que si ses tuiles sont encore exactement celles qui ont été comprimées (même mémoire, même dessin) ; une
//! étape appliquée ou oubliée entre-temps fait simplement jeter le résultat.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use eframe::egui::Color32;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

use crate::document::CelId;
use crate::raster::{Tile, TILE_PIXELS};

/// Étapes les plus récentes gardées telles quelles (de chaque côté : à annuler, à rétablir).
pub(crate) const GARDER: usize = 8;

/// Tuiles envoyées au fil de fond à chaque contrôle (le reste au contrôle suivant).
const LOT_TUILES: usize = 256;

/// Fils de compression en route en même temps.
const FILS: usize = 2;

/// Essais (mesure A/B) : la compression est coupée.
#[cfg(test)]
pub(crate) static COUPEE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Les tuiles d'une étape comprimées : celles d'une étape `Tiles` (`coupe: None`) ou `FondTiles`.
pub struct Comprime {
    pub cel: CelId,
    pub coupe: Option<bool>,
    /// Numéro de tuile et ses octets comprimés (rien : la tuile n'existait pas).
    pub tuiles: Vec<(usize, Option<Vec<u8>>)>,
}

impl Comprime {
    /// Octets tenus.
    pub(crate) fn octets(&self) -> usize {
        self.tuiles.iter().map(|(_, t)| t.as_ref().map_or(0, Vec::len)).sum()
    }

    /// Les tuiles d'origine, rendues. Une tuile qui ne se décomprime pas (jamais vu : la mémoire ne s'abîme pas) revient
    /// transparente plutôt que de planter.
    pub(crate) fn decomprimer(self) -> Vec<(usize, Option<Tile>)> {
        self.tuiles.into_iter().map(|(i, t)| (i, t.and_then(|z| decomprimer_tuile(&z)))).collect()
    }
}

pub(crate) fn comprimer_tuile(t: &Tile) -> Vec<u8> {
    let octets: &[u8] = bytemuck::cast_slice(&t[..]);
    let mut e = DeflateEncoder::new(Vec::with_capacity(2048), Compression::fast());
    // (Écrire dans un `Vec` ne rate pas.)
    let _ = e.write_all(octets);
    e.finish().unwrap_or_default()
}

pub(crate) fn decomprimer_tuile(z: &[u8]) -> Option<Tile> {
    let mut pixels = Box::new([Color32::TRANSPARENT; TILE_PIXELS]);
    let octets: &mut [u8] = bytemuck::cast_slice_mut(&mut pixels[..]);
    DeflateDecoder::new(z).read_exact(octets).ok()?;
    Some(Tile::new(*pixels))
}

/// Une étape (une feuille d'un groupe) à comprimer : ses tuiles, tenues telles quelles jusqu'au remplacement.
struct Travail {
    cel: CelId,
    coupe: Option<bool>,
    origine: Vec<(usize, Option<Tile>)>,
}

/// Le résultat : les tuiles d'origine (le remplacement s'y reconnaît) et leur forme comprimée.
struct Resultat {
    origine: Vec<(usize, Option<Tile>)>,
    comprime: Comprime,
}

/// Ce que l'historique sait de sa compression (rangé avec lui).
#[derive(Default)]
pub struct SuiviCompression {
    boite: Arc<Mutex<Vec<Resultat>>>,
    /// Fils en route.
    fils: Arc<std::sync::atomic::AtomicUsize>,
    /// Le numéro de l'historique au dernier examen : on ne le reprend que s'il a changé (ou s'il restait du travail).
    vu: u64,
    reste: bool,
    /// Octets comprimés tenus par les étapes (diagnostic).
    pub(crate) octets: usize,
    /// Étapes remplacées par leur forme comprimée (essais, diagnostic).
    pub(crate) comprimees: usize,
    /// Première tuile de chaque feuille partie au fil de fond et pas encore revenue (on ne la renvoie pas).
    en_route: Vec<Tile>,
}

/// Les tuiles de `a` et `b` sont exactement les mêmes (même mémoire).
fn memes_tuiles(a: &[(usize, Option<Tile>)], b: &[(usize, Option<Tile>)]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((i, x), (j, y))| i == j && match (x, y) {
        (Some(x), Some(y)) => x.same_as(y),
        (None, None) => true,
        _ => false,
    })
}

use crate::history::Edit;

/// Les feuilles à comprimer de l'étape `e`, ajoutées à `out` (celles déjà comprimées, ou sans tuile, sont laissées).
fn a_comprimer(e: &Edit, out: &mut Vec<Travail>, en_route: &[Tile]) {
    let (cel, coupe, tiles) = match e {
        Edit::Tiles { cel, tiles } => (*cel, None, tiles),
        Edit::FondTiles { cel, tiles, coupe } => (*cel, Some(*coupe), tiles),
        Edit::Group(v) => return v.iter().for_each(|e| a_comprimer(e, out, en_route)),
        _ => return,
    };
    let Some(premiere) = tiles.iter().find_map(|t| t.1.as_ref()) else { return };
    if en_route.iter().any(|t| t.same_as(premiere)) {
        return;
    }
    out.push(Travail { cel, coupe, origine: tiles.clone() });
}

/// Remplace dans `e` la feuille dont les tuiles sont celles de `r.origine`. Vrai si elle y était.
fn remplacer(e: &mut Edit, r: &mut Option<Resultat>) -> bool {
    let Some(res) = r.as_ref() else { return false };
    let trouve = match e {
        Edit::Tiles { cel, tiles } => res.comprime.coupe.is_none() && *cel == res.comprime.cel && memes_tuiles(tiles, &res.origine),
        Edit::FondTiles { cel, tiles, coupe } => res.comprime.coupe == Some(*coupe) && *cel == res.comprime.cel && memes_tuiles(tiles, &res.origine),
        Edit::Group(v) => return v.iter_mut().any(|e| remplacer(e, r)),
        _ => false,
    };
    if trouve {
        if let Some(res) = r.take() {
            *e = Edit::Comprime(Box::new(res.comprime));
        }
    }
    trouve
}

impl crate::history::History {
    /// Une image d'écran : les résultats arrivés remplacent leurs étapes ; les étapes au-delà des `garder` dernières
    /// (à annuler, à rétablir) qui ne sont pas encore comprimées partent au fil de fond. Rend vrai si des étapes ont été
    /// remplacées.
    pub(crate) fn comprimer_les_anciennes(&mut self, garder: usize, reveil: Option<&eframe::egui::Context>) -> bool {
        #[cfg(test)]
        if COUPEE.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        let mut change = false;
        let arrives: Vec<Resultat> = std::mem::take(&mut *self.compression.boite.lock().unwrap_or_else(|e| e.into_inner()));
        for res in arrives {
            if let Some(p) = res.origine.iter().find_map(|t| t.1.as_ref()) {
                if let Some(k) = self.compression.en_route.iter().position(|t| t.same_as(p)) {
                    self.compression.en_route.swap_remove(k);
                }
            }
            let mut r = Some(res);
            if self.etapes_mut().any(|e| remplacer(e, &mut r)) {
                change = true;
                self.compression.comprimees += 1;
            }
            // (Introuvable : l'étape a été appliquée ou oubliée ; les tuiles d'origine tombent ici avec le résultat.)
        }
        if change {
            self.compression.octets = self.octets_comprimes();
        }
        let serial = self.serial();
        if serial == self.compression.vu && !self.compression.reste {
            return change;
        }
        if self.compression.fils.load(std::sync::atomic::Ordering::Relaxed) >= FILS {
            self.compression.reste = true;
            return change;
        }
        self.compression.vu = serial;
        let mut travaux: Vec<Travail> = Vec::new();
        let mut tuiles = 0;
        let mut reste = false;
        'cotes: for anciennes in self.etapes_anciennes(garder) {
            for e in anciennes {
                let avant = travaux.len();
                a_comprimer(e, &mut travaux, &self.compression.en_route);
                tuiles += travaux[avant..].iter().map(|t| t.origine.len()).sum::<usize>();
                if tuiles >= LOT_TUILES {
                    reste = true;
                    break 'cotes;
                }
            }
        }
        self.compression.reste = reste;
        if travaux.is_empty() {
            return change;
        }
        let deja = self.compression.en_route.len();
        self.compression.en_route.extend(travaux.iter().filter_map(|t| t.origine.iter().find_map(|t| t.1.clone())));
        let boite = self.compression.boite.clone();
        let fils = self.compression.fils.clone();
        let ctx = reveil.cloned();
        fils.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let fils_fin = fils.clone();
        let lance = std::thread::Builder::new().name("annulation".into()).spawn(move || {
            crate::parallele::fil_sous_la_normale();
            for t in travaux {
                let z = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t.origine.iter().map(|(i, tuile)| (*i, tuile.as_ref().map(comprimer_tuile))).collect::<Vec<_>>()));
                if let Ok(tuiles) = z {
                    boite.lock().unwrap_or_else(|e| e.into_inner()).push(Resultat { comprime: Comprime { cel: t.cel, coupe: t.coupe, tuiles }, origine: t.origine });
                }
            }
            fils_fin.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        if lance.is_err() {
            // (La fermeture est lâchée avec ses tuiles : les étapes restent comme elles sont, et on réessaiera.)
            fils.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            self.compression.en_route.truncate(deja);
            self.compression.reste = true;
        }
        change
    }
}
