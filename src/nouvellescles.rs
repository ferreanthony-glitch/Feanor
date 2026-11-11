//! « Nouvelle clé à l'image courante » et « Nouvelle clé vide à l'image courante » sur tous les calques choisis (Anthony,
//! 2026-11-10, partie 132 : la sélection agit sur tout). Par le raccourci, le bouton ou le menu d'une piste (`dessin.nouvelle_cle`,
//! `dessin.nouvelle_cle_vide`).
//!
//! - Avec un seul calque choisi : `montage_new_key`, comme avant.
//! - Avec plusieurs : chacun reçoit sa clé à l'image courante, selon son genre (une copie de son dessin montré, ou un dessin
//!   vide de sa sorte, bitmap ou vectoriel), en **une seule étape d'annulation**. Les calques verrouillés (eux ou leur dossier),
//!   les dossiers, les calques vidéo et les calques de couleur liés (leurs images suivent leur calque de trait) sont laissés de
//!   côté ; un calque qui a déjà une clé à cette image suit la règle d'un seul calque (refus) : la barre d'état donne ces comptes.
//! - Un calque **masqué** reçoit sa clé comme les autres (Anthony, 2026-11-11 : « Sélectionner 3 calques et cliquer sur F5 ne crée
//!   une clé d'image que sur un seul calque au lieu des trois » — deux des trois avaient l'œil fermé). Une clé est une affaire de
//!   durée, pas de dessin qu'on ne voit pas ; et un calque masqué seul la reçoit déjà (`montage_new_key`).

use crate::app::App;
use crate::handles::{key_at, key_source, key_start_taken};

impl App {
    /// Les calques que vise une nouvelle clé, du plus bas au plus haut : les calques choisis et, dès que deux clés au moins sont
    /// choisies dans la timeline ou le montage, les calques du plan courant qui portent une de ces clés (« calque par clé », la règle
    /// des outils, partie 132 ; 2026-11-11 : des clés prises sur plusieurs pistes ne faisaient pas de leurs calques des calques
    /// choisis, la nouvelle clé n'allait que sur le calque courant).
    pub(crate) fn calques_de_nouvelle_cle(&self) -> Vec<usize> {
        let mut vises = self.chosen_layers();
        if self.key_sel.len() >= 2 {
            if let Some(p) = self.doc.panels.get(self.cur_panel) {
                for (li, l) in p.layers.iter().enumerate() {
                    if !l.folder && !vises.contains(&li) && self.key_sel.iter().any(|k| k.panel == p.id && k.layer == l.id) {
                        vises.push(li);
                    }
                }
            }
            vises.sort_unstable();
        }
        vises
    }

    /// Le calque `li` est parmi les calques visés (`calques_de_nouvelle_cle`), et ils sont plusieurs : la nouvelle clé va sur tous.
    pub(crate) fn nouvelle_cle_sur_tous(&self, li: usize) -> bool {
        let vises = self.calques_de_nouvelle_cle();
        vises.len() >= 2 && vises.contains(&li)
    }

    /// Une nouvelle clé à l'image courante sur chaque calque choisi (`copie` : une copie du dessin montré ; sinon vide).
    pub(crate) fn nouvelle_cle_choisis(&mut self, copie: bool) {
        if self.content_refuses() {
            return;
        }
        let (pi, f) = (self.cur_panel, self.cur_frame);
        let Some(len) = self.doc.panels.get(pi).map(|p| p.frame_count()) else { return };
        let f = f.min(len.saturating_sub(1));
        let choisis = self.calques_de_nouvelle_cle();
        // Qui reçoit une clé, et qui est laissé de côté (et pourquoi) : tout est décidé avant d'écrire.
        let mut vises = Vec::new();
        let mut laisses: Vec<(String, &'static str)> = Vec::new();
        {
            let layers = &self.doc.panels[pi].layers;
            for &li in &choisis {
                let l = &layers[li];
                let raison = if l.folder {
                    Some("dossier")
                } else if l.video.is_some() {
                    Some("vidéo")
                } else if l.locked || crate::folders::locked_by_folder(layers, li) {
                    Some("verrouillé")
                } else if l.keys_follow_line() {
                    Some("suit son calque de trait")
                } else if key_start_taken(l, f) {
                    Some("une clé commence déjà ici")
                } else {
                    None
                };
                match raison {
                    Some(r) => laisses.push((l.name.clone(), r)),
                    None => vises.push(li),
                }
            }
        }
        let texte_laisses = (!laisses.is_empty()).then(|| {
            let liste: Vec<String> = laisses.iter().map(|(n, r)| format!("{n} ({r})")).collect();
            format!(" Laissé{} de côté : {}.", if liste.len() > 1 { "s" } else { "" }, liste.join(", "))
        });
        if vises.is_empty() {
            self.status = format!("Nouvelle clé : aucun des calques choisis ne peut en recevoir une ici.{}", texte_laisses.unwrap_or_default());
            return;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let mut nouvelles = Vec::new();
        for &li in &vises {
            let (kind, source) = {
                let l = &self.doc.panels[pi].layers[li];
                (l.kind, key_source(l, f).filter(|_| copie))
            };
            let cel = match source {
                Some(src) => self.doc.duplicate_cel(src),
                None => self.doc.new_cel(kind),
            };
            let end = key_at(&mut self.doc.panels[pi].layers[li], f, cel, len).unwrap_or(f + 1);
            if let Some(src) = source {
                crate::calquecouleur::noter_copie(&mut self.doc, pi, li, src, cel);
            }
            nouvelles.push((li, end));
        }
        self.cur_frame = f;
        self.vsel = None;
        self.commit_structure(snap);
        // Les nouvelles clés deviennent le choix (la timeline et le montage les éclairent).
        let keys: Vec<_> = nouvelles.iter().filter_map(|&(li, _)| self.key_ref(pi, li, f)).collect();
        self.key_anchor = keys.first().copied();
        self.key_sel = keys;
        let n = nouvelles.len();
        let quoi = if copie { "copie du dessin" } else { "dessin vide" };
        self.status = format!("Nouvelle clé sur {n} calque{} ({quoi}), image {}.{}", if n > 1 { "s" } else { "" }, self.frame_label(f), texte_laisses.unwrap_or_default());
    }
}
