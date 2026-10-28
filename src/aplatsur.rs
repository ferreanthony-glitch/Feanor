//! « Peint sur l'aplat » (Anthony, 2026-10-28) : une option du pinceau et du crayon, à côté de « Peindre derrière » (les deux
//! sont exclusives). Allumée, la peinture va **directement dans l'aplat** du dessin, jamais sur le trait :
//!
//! - sur un calque de trait qui a un calque de couleur : dans le dessin de ce calque de couleur à cette image (celui qui est
//!   juste sous le trait, s'il y en a plusieurs) ; il se comporte comme si on l'avait choisi (zone fermée, mode Griffonner) ;
//! - sinon, dans la strate d'aplat du dessin (`fond.rs`), comme quand la sous-ligne de l'aplat est choisie ; sans aplat, il
//!   est créé (comme le bouton « Aplat sous le trait » du calque) et la barre d'état le dit ;
//! - sur un calque de couleur : rien à faire, c'est déjà l'aplat.
//!
//! La gomme, le pot de peinture et les autres outils ne sont pas détournés : ils gardent leur cible (le trait, ou l'aplat
//! quand sa sous-ligne est choisie).

use crate::app::{App, Tool};
use crate::aplatmulti::Visee;
use crate::calquecouleur as cc;
use eframe::egui::Pos2;

/// Où « Peint sur l'aplat » envoie le trait qui commence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurAplat {
    /// Le calque de couleur de rang `li` (le calque courant reste le calque de trait).
    Couleur(usize),
    /// L'aplat sous le trait du calque courant ; `cree` : il n'existe pas encore.
    Strate { cree: bool },
}

impl App {
    /// L'outil courant est un des deux qui peuvent peindre sur l'aplat.
    pub(crate) fn outil_peint_aplat(&self) -> bool {
        matches!(self.tool, Tool::Brush | Tool::Pencil)
    }

    /// Où va le trait qui commence, si « Peint sur l'aplat » est allumé et qu'il faut le détourner. Rien : il va où il allait
    /// (la gomme, un calque de couleur, un dossier, la sous-ligne de l'aplat déjà choisie…).
    pub(crate) fn sur_aplat_cible(&self) -> Option<SurAplat> {
        if !self.paint_on_aplat || !self.outil_peint_aplat() || self.aplat_en_cible() {
            return None;
        }
        let l = self.layers().get(self.cur_layer)?;
        // Un calque de couleur est déjà l'aplat ; un dossier, un calque vidéo ou d'effet n'en prennent pas.
        if l.couleur.is_some() || !crate::fondui::takes_fond(l) || l.effet.is_some() {
            return None;
        }
        match cc::couleurs_de(self.layers(), self.cur_layer).last() {
            Some(&ci) => Some(SurAplat::Couleur(ci)),
            None => Some(SurAplat::Strate { cree: l.fond.is_none() }),
        }
    }

    /// Début d'un trait de pinceau, de crayon ou de gomme. « Peint sur l'aplat » le détourne vers l'aplat (`sur_aplat_cible`) ;
    /// sinon c'est `begin_paint_ici`.
    pub(crate) fn begin_paint(&mut self, p: Pos2, pressure: f32, pen: bool) {
        self.end_stroke();
        // L'aplat que vise ce trait (`aplatmulti.rs`) : plusieurs clés choisies, et il se fait sur l'aplat de chacune.
        let visee = match self.sur_aplat_cible() {
            None => {
                self.begin_paint_ici(p, pressure, pen);
                self.aplat_visee()
            }
            Some(SurAplat::Couleur(ci)) => {
                // Le calque de couleur « est » le calque courant le temps d'ouvrir le trait (sa zone fermée, son mode
                // Griffonner, ses verrous) : le trait garde son dessin, le calque courant revient aussitôt.
                let garde = std::mem::replace(&mut self.cur_layer, ci);
                self.begin_paint_ici(p, pressure, pen);
                self.cur_layer = garde;
                self.layers().get(ci).filter(|l| l.couleur.as_ref().is_some_and(|c| !c.griffonner)).map(|l| Visee::Couleur { layer: l.id })
            }
            Some(SurAplat::Strate { cree }) => {
                let li = self.cur_layer;
                if cree {
                    self.toggle_fond(li);
                    if self.layers().get(li).is_none_or(|l| l.fond.is_none()) {
                        // (Refusé : stockage verrouillé, etc. ; le message est dans la barre d'état.)
                        return;
                    }
                }
                let Some(id) = self.layers().get(li).map(|l| l.id) else { return };
                let garde = self.aplat_cible.replace(id);
                self.begin_paint_ici(p, pressure, pen);
                self.aplat_cible = garde;
                if cree && !self.status.contains("dessine d'abord") {
                    self.status = "Peint sur l'aplat : ce calque n'avait pas d'aplat, il vient d'être créé sous le trait (Ctrl+Z l'enlève) ; la peinture y va, pas sur le trait.".into();
                }
                Some(Visee::Strate { layer: id })
            }
        };
        if let (Some(v), Some(primary)) = (visee, self.stroke.as_ref().map(|s| s.cel)) {
            self.miroirs_begin(v, primary, p, pressure, pen);
        }
    }
}
