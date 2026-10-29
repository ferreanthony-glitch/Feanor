//! Les clés de tous les calques d'un plan, en petits traits sur son bloc du montage (Anthony, 2026-10-28, branche
//! `timeline-repliee` : pas de ligne « Tous les calques » à part, pour gagner de la place ; les clés s'affichent « sur le bloc du plan,
//! dans la rangée des plans »).
//!
//! Un trait par image qui porte une clé, le long du bas du bloc, sans cacher la vignette ni le nom du plan. Le trait est de la couleur de la
//! sorte de son calque (`theme::couleur_du_calque` : vectoriel, bitmap, calque de couleur et aplat, effet, annotation ; le décor a la
//! sienne) ; si plusieurs calques ont une clé à la même image, le trait est coupé en autant de petits morceaux de leurs couleurs. Ainsi le
//! rythme du film se lit même quand tous les calques sont repliés.
//!
//! Une « clé » est : un dessin qui commence (une exposition), une clé de position, une clé d'effet. Les calques de couleur liés (l'aplat)
//! comptent, comme tous les autres ; les dossiers et les vidéos n'ont pas de clés.
//!
//! **Sans latence** : les clés d'un plan se calculent une fois par changement du document (le numéro de l'historique, `History::serial`,
//! change à chaque étape, annulation, projet ouvert) et se gardent ; au repos, rien n'est recalculé ni repeint. Pendant qu'on tient la
//! souris (un geste qui change le plan sans encore avoir d'étape), elles se recalculent à chaque image, pour les seuls plans à l'écran.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use eframe::egui::{pos2, Color32, Painter, Rect};

use crate::document::{Layer, Panel};
use crate::theme;

/// Une image du plan qui porte au moins une clé, et les couleurs (distinctes) des calques qui y en ont une.
pub(crate) type Cle = (usize, Vec<Color32>);

/// Hauteur des traits.
const TRAIT_H: f32 = 5.0;

/// La couleur de la clé d'un calque : celle de sa sorte, ou celle du décor.
pub(crate) fn couleur_de_cle(l: &Layer) -> Color32 {
    if l.decor || l.scene_wide.is_some() {
        theme::decor()
    } else {
        theme::couleur_du_calque(l)
    }
}

/// Les clés de tous les calques du plan `p`, par image croissante.
pub(crate) fn cles_du_plan(p: &Panel) -> Vec<Cle> {
    let max = p.frame_count();
    let mut par_image: BTreeMap<usize, Vec<Color32>> = BTreeMap::new();
    for l in &p.layers {
        if l.folder || l.video.is_some() {
            continue;
        }
        let couleur = couleur_de_cle(l);
        let mut ajouter = |f: usize| {
            if f < max {
                let v = par_image.entry(f).or_default();
                if !v.contains(&couleur) {
                    v.push(couleur);
                }
            }
        };
        let mut f = 0;
        while f < l.frames.len() {
            let (s, e) = l.run_bounds(f);
            if l.frames[f].is_some() {
                ajouter(s);
            }
            f = e.max(f + 1);
        }
        for k in &l.pos_keys {
            ajouter(k.frame);
        }
        for k in &l.opacity_keys {
            ajouter(k.frame);
        }
        for e in &l.fx {
            for f in crate::fxkeys::marker_frames(e) {
                ajouter(f);
            }
        }
    }
    par_image.into_iter().collect()
}

/// Ce qui fait que les clés gardées d'un plan ne valent plus : ses calques, leur nombre, la durée du plan.
fn empreinte(p: &Panel) -> u64 {
    p.layers.iter().fold(p.layers.len() as u64 ^ ((p.frame_count() as u64) << 20), |h, l| h.wrapping_mul(0x100_0000_01b3) ^ l.id)
}

/// Les clés gardées des plans, au numéro d'historique `serial`.
#[derive(Default)]
pub(crate) struct ClesBlocs {
    serial: Option<u64>,
    plans: HashMap<u64, (u64, Arc<Vec<Cle>>)>,
    /// Combien de fois des clés ont été calculées (les essais vérifient que le repos ne recalcule rien).
    pub calculs: usize,
}

impl ClesBlocs {
    /// Les clés du plan `p` : celles qu'on a gardées si le document n'a pas changé, sinon calculées (et gardées). `tenu` : la souris est tenue,
    /// un geste est peut-être en cours, qui change le plan sans étape d'historique : on recalcule.
    pub fn pour(&mut self, serial: u64, p: &Panel, tenu: bool) -> Arc<Vec<Cle>> {
        if self.serial != Some(serial) {
            self.serial = Some(serial);
            self.plans.clear();
        }
        let sig = empreinte(p);
        if !tenu {
            if let Some((s, cles)) = self.plans.get(&p.id) {
                if *s == sig {
                    return cles.clone();
                }
            }
        }
        self.calculs += 1;
        let cles = Arc::new(cles_du_plan(p));
        self.plans.insert(p.id, (sig, cles.clone()));
        cles
    }
}

/// Peint les clés sur le bloc `rect` d'un plan (son image 0 est à `rect.left() - 1`), à `ppf` points par image. `painter` : coupé au bloc.
pub(crate) fn peindre(painter: &Painter, rect: Rect, cles: &[Cle], ppf: f32) {
    let x0 = rect.left() - 1.0;
    let bas = rect.bottom() - 1.5;
    let haut = bas - TRAIT_H;
    let large = (ppf * 0.6).clamp(1.5, 3.0);
    let visible = painter.clip_rect();
    let mut dernier = f32::NEG_INFINITY;
    for (f, couleurs) in cles {
        let x = x0 + *f as f32 * ppf + ppf * 0.5;
        if x < visible.left() - 4.0 {
            continue;
        }
        if x > visible.right() + 4.0 {
            break;
        }
        // (Dézoomé, deux images tombent dans le même point : un seul trait.)
        if x - dernier < 0.8 {
            continue;
        }
        dernier = x;
        // Un fin contour sombre : le trait reste lisible sur la vignette.
        painter.rect_filled(Rect::from_min_max(pos2(x - large * 0.5 - 0.75, haut - 0.75), pos2(x + large * 0.5 + 0.75, bas + 0.75)), 0.0, Color32::from_black_alpha(150));
        let n = couleurs.len().clamp(1, 4);
        let morceau = TRAIT_H / n as f32;
        for (k, c) in couleurs.iter().take(n).enumerate() {
            let y = haut + k as f32 * morceau;
            painter.rect_filled(Rect::from_min_max(pos2(x - large * 0.5, y), pos2(x + large * 0.5, y + morceau)), 0.0, *c);
        }
    }
}
