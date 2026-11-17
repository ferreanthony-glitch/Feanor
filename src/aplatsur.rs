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
//!
//! # Le piège du 2026-11-10 (partie 133)
//!
//! Anthony, sur la planche des Vignettes, l'interrupteur resté allumé sans qu'il le voie : « la planche ne prend plus le trait ». Rien ne
//! restait dans les cases (pas de trait d'où déduire l'aplat), chaque essai créait un aplat sur le calque, chaque case pâle un plan vide, et
//! sur le papier le trait disparaissait au lâcher. Depuis :
//!
//! - **un trait posé sur le papier** (hors du champ de toute case, ou sur une case écartée : `Vignettes::appui_papier`) n'est pas détourné,
//!   il va au papier comme d'habitude : le papier n'a pas d'aplat ;
//! - **un trait qui ne laisserait rien** (le dessin n'a pas de trait à cette image) ne crée rien : ni aplat, ni dessin, ni étape
//!   d'annulation, ni plan (la case pâle qui naissait se défait au lâcher, `finish_creation`) ; la barre d'état dit que « Peint sur
//!   l'aplat » est allumé et comment l'éteindre (`SANS_TRAIT`) ;
//! - **la barre du bas de la planche** le montre tant qu'il est allumé (avec le pinceau ou le crayon) ; un clic l'éteint
//!   (`vignettesprefs::planche_barre_basse`).

use crate::app::{App, Tool};
use crate::aplatmulti::Visee;
use crate::calquecouleur as cc;
use crate::history::Edit;
use crate::raster::Tile;
use eframe::egui::Pos2;

/// Ce que dit la barre d'état quand « Peint sur l'aplat » n'a rien où peindre : le dessin n'a pas de trait ici (partie 133).
pub(crate) const SANS_TRAIT: &str = "Peint sur l'aplat est allumé : la peinture va sous un trait déjà dessiné, et ce calque n'en a pas ici. Éteins-le (sous l'aperçu du pinceau, ou en bas de la planche) pour dessiner le trait.";

/// Ce que dit la barre d'état quand le trait envoyé dans l'aplat n'a rien posé : il n'est pas entré dans la forme du trait (partie 133).
pub(crate) const HORS_DE_LA_FORME: &str = "Peint sur l'aplat est allumé : la peinture ne va que dans la forme du trait, et ce coup de pinceau n'y est pas entré (rien n'est gardé). Éteins-le (sous l'aperçu du pinceau, ou en bas de la planche) pour dessiner le trait.";

/// Le trait en cours, envoyé dans l'aplat sous le trait par « Peint sur l'aplat » (`App::sur_aplat_geste`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GesteSurAplat {
    /// L'aplat a été créé pour ce trait : la longueur de l'historique juste après sa création.
    pub cree: Option<usize>,
}

/// Une tuile d'avant un trait et la même maintenant : le trait y a-t-il changé un pixel ? (Une tuile vide vaut une tuile absente.)
fn tuile_changee(avant: Option<&Tile>, maintenant: Option<&Tile>) -> bool {
    let vide = |t: Option<&Tile>| t.is_none_or(|t| t.iter().all(|c| c.a() == 0));
    match (avant, maintenant) {
        (Some(a), Some(b)) => a != b,
        (a, b) => !(vide(a) && vide(b)),
    }
}

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
        // Un trait posé sur le papier de la planche va au papier, qui n'a pas d'aplat (partie 133).
        if self.vignettes.appui_papier {
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
        self.sur_aplat_geste = None;
        // Aplat intelligent (platintel.rs) : le dessin d'avant le trait, pour que l'aplat en garde les couleurs après.
        self.intel_avant_trait();
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
                    // Rien d'où déduire l'aplat : on ne le crée pas pour rien (partie 133).
                    let r = self.aplat_pour_calque(li, self.new_fond());
                    if !self.aplat_a_un_trait(&r) {
                        self.status = SANS_TRAIT.into();
                        return;
                    }
                    self.toggle_fond(li);
                    if self.layers().get(li).is_none_or(|l| l.fond.is_none()) {
                        // (Refusé : stockage verrouillé, etc. ; le message est dans la barre d'état.)
                        return;
                    }
                }
                let apres = cree.then(|| self.history.undo_len());
                let Some(id) = self.layers().get(li).map(|l| l.id) else { return };
                let garde = self.aplat_cible.replace(id);
                self.begin_paint_ici(p, pressure, pen);
                self.aplat_cible = garde;
                if self.stroke.is_some() {
                    self.sur_aplat_geste = Some(GesteSurAplat { cree: apres });
                } else if let Some(n) = apres {
                    // (Le trait ne s'est pas ouvert : l'aplat créé pour lui est défait.)
                    let dit = std::mem::take(&mut self.status);
                    self.defaire_l_aplat_cree(n);
                    self.status = dit;
                }
                if self.stroke.is_none() && self.status.contains("dessine d'abord") {
                    // (Refusé faute de trait : c'est « Peint sur l'aplat » qui l'a voulu, la barre d'état le dit.)
                    self.status = SANS_TRAIT.into();
                } else if cree && self.stroke.is_some() && !self.status.contains("dessine d'abord") {
                    self.status = "Peint sur l'aplat : ce calque n'avait pas d'aplat, il vient d'être créé sous le trait (Ctrl+Z l'enlève) ; la peinture y va, pas sur le trait.".into();
                }
                Some(Visee::Strate { layer: id })
            }
        };
        if let (Some(v), Some(primary)) = (visee, self.stroke.as_ref().map(|s| s.cel)) {
            self.miroirs_begin(v, primary, p, pressure, pen);
        }
    }

    /// Les étapes d'un trait sur la strate d'aplat (`FondTiles`) ont-elles changé un pixel de sa **peinture** ? La coupe que le pinceau
    /// recoud en passant ne compte pas (hors de la forme du trait elle ne se voit pas). Une étape d'une autre sorte (la cassure automatique
    /// qui ouvre le dessin) compte comme un changement.
    pub(crate) fn peinture_changee(&self, edits: &[Edit]) -> bool {
        edits.iter().any(|e| match e {
            Edit::FondTiles { coupe: true, .. } => false,
            Edit::FondTiles { cel, tiles, coupe: false } => {
                let Some(s) = self.doc.cels.get(cel).and_then(|c| c.fond.as_ref()) else { return true };
                tiles.iter().any(|(i, avant)| tuile_changee(avant.as_ref(), s.peinture.raster.slot(*i)))
            }
            _ => true,
        })
    }

    /// Fin d'un trait que « Peint sur l'aplat » a envoyé dans l'aplat sous le trait (`end_stroke`), avec ses étapes `edits` : s'il n'a rien
    /// peint (il n'est pas entré dans la forme du trait), il est défait (la coupe qu'il a recousue en passant revient comme avant), il ne
    /// laisse pas d'étape, l'aplat créé pour lui est défait, et la barre d'état le dit (partie 133). Rien à faire pour un trait qui a peint,
    /// ou dont d'autres clés suivent le geste (`aplatmulti.rs`).
    pub(crate) fn sur_aplat_fin(&mut self, edits: &mut Vec<Edit>) {
        let Some(g) = self.sur_aplat_geste.take() else { return };
        if !self.aplat_miroirs.est_vide() || self.peinture_changee(edits) {
            return;
        }
        let mut nav = crate::history::Nav { panel: self.cur_panel, layer: self.cur_layer };
        for e in edits.drain(..).rev() {
            let _ = e.apply(&mut self.doc, &mut nav);
        }
        self.renderer.invalidate_all();
        if let Some(n) = g.cree {
            self.defaire_l_aplat_cree(n);
        }
        self.status = HORS_DE_LA_FORME.into();
    }

    /// L'aplat que « Peint sur l'aplat » a créé pour un trait qui ne pose rien est défait, sans laisser d'étape à rétablir. `apres` : la
    /// longueur de l'historique juste après sa création ; rien n'est défait si une autre étape est venue depuis.
    fn defaire_l_aplat_cree(&mut self, apres: usize) -> bool {
        if self.history.undo_len() != apres {
            return false;
        }
        let mut nav = crate::history::Nav { panel: self.cur_panel, layer: self.cur_layer };
        if !self.history.undo_and_forget(&mut self.doc, &mut nav) {
            return false;
        }
        self.cur_panel = nav.panel.min(self.doc.panels.len().saturating_sub(1));
        self.cur_layer = nav.layer;
        self.revalider_rangs();
        self.renderer.invalidate_all();
        true
    }

    /// Le dessin du calque courant à l'image courante a-t-il un trait d'où déduire un aplat (réglages `r`) ? Rien n'est créé pour le
    /// savoir : pas de dessin à cette image, pas de trait (partie 133).
    pub(crate) fn aplat_a_un_trait(&self, r: &crate::fond::FondReglages) -> bool {
        let (w, h) = (self.doc.width, self.doc.height);
        self.cel_here().and_then(|id| self.doc.cels.get(&id)).is_some_and(|c| crate::fond::silhouette_monde(&c.raster, r).feuille(w, h).iter().any(|&v| v != 0))
    }
}
