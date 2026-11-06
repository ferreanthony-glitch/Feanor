//! Couper un plan qui a des versions rangées (partie 102 de `docs/INTERFACE.md`) : une petite fenêtre demande avec quelle partie
//! les versions rangées restent. « Ce plan a N versions rangées. Elles vont avec : la première partie / la seconde partie »,
//! et Annuler (rien ne change).
//!
//! - La version courante est coupée normalement ; les versions rangées restent entières et vont toutes avec la partie choisie
//!   (`Document::split_plan`, `variantes::Cote`) ; l'autre partie n'a que son contenu courant.
//! - Tous les chemins de coupe passent par là : F5 et le menu (`split_current_panel`), la barre entre deux vignettes
//!   (`couper_entre_vignettes`), le dépôt d'une image entre deux images d'un plan (`depot_appliquer`). Une coupe qui n'a pas de
//!   fenêtre (le mouvement de caméra posé sur la première case, `scinder_apres_mouvement`) garde les versions avec la première partie.
//! - **Rien n'est fait en attendant** : la fenêtre garde de quoi refaire le geste (`Refaire`), le document n'est pas touché. Un changement
//!   de plan courant, d'image courante ou de projet pendant qu'elle est ouverte l'annule (rien ne change) ; Échap aussi.
//! - Une réponse = une étape d'annulation, celle de la coupe.

use eframe::egui::{self, vec2, Align2};

use crate::app::App;
use crate::deposer::Cible;
use crate::raster::Raster;
use crate::variantes::{avec_cote, cote_choisi, Cote};

/// Le geste de coupe qui attend la réponse, de quoi le refaire.
pub(crate) enum Refaire {
    /// F5, le menu : le plan courant, à l'image courante.
    Courant,
    /// La barre entre deux vignettes d'un même plan.
    Vignettes { plan: u64, frame: usize },
    /// Des images déposées entre deux images d'un plan.
    Depot { cible: Cible, rasters: Vec<Raster> },
}

/// La question en cours.
pub(crate) struct CoupeEnAttente {
    pub plan: u64,
    pub at: usize,
    /// Le nombre de versions rangées du plan.
    pub n: usize,
    /// Le plan courant et l'image courante quand on a demandé : s'ils changent, la question tombe.
    pub courant: u64,
    pub frame: usize,
    /// Le projet pour lequel elle vaut (`App::doc_epoch`).
    pub epoch: u64,
    pub refaire: Refaire,
}

impl App {
    /// Couper `plan` à l'image `at` demande-t-il la question (le plan a des versions rangées, la coupe aura lieu, et la réponse n'est
    /// pas déjà donnée) ?
    pub(crate) fn coupe_versions_necessaire(&self, plan: u64, at: usize) -> bool {
        cote_choisi().is_none() && self.doc.coupe_a_demander(plan, at).is_some()
    }

    /// Ouvre la question ; le geste `refaire` attend la réponse, le document n'est pas touché (faux : rien n'a changé). Avec une réponse
    /// d'avance (`coupe_versions_auto`, les essais), le geste se fait tout de suite : vrai s'il a changé le document.
    pub(crate) fn coupe_versions_ouvrir(&mut self, plan: u64, at: usize, refaire: Refaire) -> bool {
        let n = self.doc.coupe_a_demander(plan, at).unwrap_or(0);
        let courant = self.doc.panels.get(self.cur_panel).map_or(0, |p| p.id);
        self.coupe_versions = Some(CoupeEnAttente { plan, at, n, courant, frame: self.cur_frame, epoch: self.doc_epoch, refaire });
        self.status = "Ce plan a des versions rangées : choisis la partie avec laquelle elles restent.".into();
        match self.coupe_versions_auto {
            Some(cote) => self.coupe_versions_repondre(cote),
            None => false,
        }
    }

    /// La question vaut-elle encore : même projet, même plan courant et même image, le plan existe.
    fn coupe_versions_valable(&self, c: &CoupeEnAttente) -> bool {
        c.epoch == self.doc_epoch
            && self.doc.panels.iter().any(|p| p.id == c.plan)
            && self.doc.panels.get(self.cur_panel).is_some_and(|p| p.id == c.courant)
            && self.cur_frame == c.frame
    }

    /// La réponse : le geste est refait, les versions rangées avec la partie `cote` (une étape d'annulation). Faux si la question ne
    /// valait plus (rien n'est fait) ou si la coupe n'a rien changé.
    pub(crate) fn coupe_versions_repondre(&mut self, cote: Cote) -> bool {
        let Some(c) = self.coupe_versions.take() else { return false };
        if !self.coupe_versions_valable(&c) {
            self.status = "Plan non coupé : le plan ou le projet a changé pendant la question, rien n'a changé.".into();
            return false;
        }
        avec_cote(cote, || match c.refaire {
            Refaire::Courant => {
                self.split_current_panel();
                true
            }
            Refaire::Vignettes { plan, frame } => self.couper_entre_vignettes(plan, frame),
            Refaire::Depot { cible, rasters } => {
                self.depot_appliquer(cible, rasters);
                true
            }
        })
    }

    /// « Annuler » : la fenêtre se ferme, rien ne bouge.
    pub(crate) fn coupe_versions_annuler(&mut self) {
        if self.coupe_versions.take().is_some() {
            self.status = "Plan non coupé : rien n'a changé.".into();
        }
    }

    /// La fenêtre de la question, une fois par image d'écran.
    pub(crate) fn coupe_versions_ui(&mut self, ctx: &egui::Context) {
        let Some(c) = self.coupe_versions.as_ref() else { return };
        if !self.coupe_versions_valable(c) {
            self.coupe_versions = None;
            self.status = "Plan non coupé : le plan ou le projet a changé pendant la question, rien n'a changé.".into();
            return;
        }
        let n = c.n;
        let mut reponse: Option<Cote> = None;
        let mut annuler = false;
        egui::Window::new("Couper un plan à plusieurs versions")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                if n == 1 {
                    ui.label("Ce plan a 1 version rangée. Elle va avec :");
                } else {
                    ui.label(format!("Ce plan a {n} versions rangées. Elles vont avec :"));
                }
                ui.label(egui::RichText::new("La version courante est coupée en deux ; les versions rangées restent entières, avec la partie choisie.").weak().small());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("La première partie").clicked() {
                        reponse = Some(Cote::Premiere);
                    }
                    if ui.button("La seconde partie").clicked() {
                        reponse = Some(Cote::Seconde);
                    }
                    if ui.button("Annuler").clicked() {
                        annuler = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            annuler = true;
        }
        if let Some(cote) = reponse {
            self.coupe_versions_repondre(cote);
        } else if annuler {
            self.coupe_versions_annuler();
        }
    }
}
