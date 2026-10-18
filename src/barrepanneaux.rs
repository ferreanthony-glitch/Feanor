//! Barre des panneaux (2026-10-18) : une fine bande d'icônes sur le bord droit de la fenêtre, une
//! icône par panneau de côté ; un clic l'ouvre, le met devant ou le ferme. L'idée vient des
//! logiciels de retouche (une colonne d'icônes qui allument et éteignent les panneaux) ; les
//! icônes sont celles de SHOT (`Pane::icon`). Elle n'est pas un panneau de l'espace de travail :
//! elle est posée avant la zone des panneaux, comme la barre d'outils de gauche.
//!
//! Chaque icône est liée à une action (`vue.calques`…), donc une bulle d'aide avec son raccourci,
//! un clic droit « Assigner un raccourci » et une entrée dans la palette (Ctrl+K) ; les raccourcis
//! suivent la même règle de clic que l'icône. Le dessin ne coûte que dix icônes : la place des
//! panneaux n'est lue qu'au clic.

use eframe::egui::{self, vec2, Color32, RichText};
use egui_dock::{DockState, Node};

use crate::app::{App, Mode};
use crate::dock::Pane;

/// Largeur de la barre, marges comprises, en points.
pub(crate) const LARGEUR: f32 = 30.0;

/// Une icône de la barre.
pub(crate) struct Entree {
    pub pane: Pane,
    /// L'action qui ouvre, met devant ou ferme ce panneau (`keymap::ACTIONS`).
    pub action: &'static str,
    pub nom: &'static str,
    pub description: &'static str,
}

impl Entree {
    /// L'icône se voit-elle dans ce mode ? La feuille d'exposition n'existe qu'en animation.
    pub(crate) fn visible(&self, animation: bool) -> bool {
        animation || !self.pane.animation_only()
    }
}

/// Les icônes de la barre dans un mode, en groupes (un groupe vide n'est pas rendu).
#[cfg(test)]
pub(crate) fn icones(animation: bool) -> Vec<Vec<&'static Entree>> {
    GROUPES.iter().map(|g| g.iter().filter(|e| e.visible(animation)).collect::<Vec<_>>()).filter(|g| !g.is_empty()).collect()
}

const fn entree(pane: Pane, action: &'static str, nom: &'static str, description: &'static str) -> Entree {
    Entree { pane, action, nom, description }
}

/// Les icônes, en groupes séparés par un trait : le dessin, le plan, puis l'historique.
pub(crate) const GROUPES: &[&[Entree]] = &[
    &[
        entree(Pane::ToolOptions, "vue.reglages", "Réglages de l'outil", "Taille, opacité et options de l'outil courant. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Brushes, "vue.pinceaux", "Pinceaux", "Les pinceaux et leurs réglages, chacun avec l'aperçu de son trait. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Color, "vue.couleur", "Couleur", "Roue, curseurs, couleurs récentes et nuanciers. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Guides, "vue.guides", "Guides", "Les guides de perspective. Un clic l'ouvre ou le met devant, un autre le ferme."),
    ],
    &[
        entree(Pane::Layers, "vue.calques", "Calques", "Les calques du plan, leur mode et leur opacité. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Effects, "fx.panneau", "Effets", "La pile d'effets du calque courant. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Sheet, "vue.fiche", "Fiche du plan", "Textes, durée et caméra du plan. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::XSheet, "vue.feuille_exposition", "Feuille d'exposition", "Le plan image par image, calque par calque. Un clic l'ouvre ou le met devant, un autre le ferme."),
    ],
    &[entree(Pane::History, "vue.historique", "Historique", "Les étapes d'annulation, par leur nom. Un clic l'ouvre ou le met devant, un autre le ferme.")],
];

/// Le panneau qu'une action ouvre ou ferme, si c'en est une de la barre.
pub(crate) fn pane_de_action(action: &str) -> Option<Pane> {
    GROUPES.iter().flat_map(|g| g.iter()).find(|e| e.action == action).map(|e| e.pane)
}

/// Ce que fait un clic sur l'icône d'un panneau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clic {
    /// Fermé : on l'ouvre, devant les onglets de son groupe.
    Ouvrir,
    /// Ouvert mais caché derrière un autre onglet : on le met devant.
    Avancer,
    /// Ouvert et visible : on le ferme.
    Fermer,
}

/// La règle du clic : `ouvert` : le panneau est dans l'espace de travail ; `devant` : on le voit
/// (onglet actif de son groupe, ou fenêtre à part).
pub(crate) fn regle_clic(ouvert: bool, devant: bool) -> Clic {
    match (ouvert, devant) {
        (false, _) => Clic::Ouvrir,
        (true, false) => Clic::Avancer,
        (true, true) => Clic::Fermer,
    }
}

/// `pane` est-il l'onglet actif de son groupe dans `state` ? (Faux s'il n'y est pas.)
pub(crate) fn est_devant(state: &DockState<Pane>, pane: Pane) -> bool {
    match state.find_tab(&pane) {
        Some((surface, node, tab)) => matches!(&state[surface][node], Node::Leaf { active, .. } if *active == tab),
        None => false,
    }
}

impl App {
    /// Ouvre, met devant ou ferme `pane` selon `regle_clic` (clic sur la barre, ou son action).
    pub(crate) fn clic_panneau(&mut self, pane: Pane) {
        let ouvert = self.pane_shown(pane);
        // Une fenêtre à part se voit toujours ; sans disposition encore, rien n'est caché.
        let devant = self.detached.iter().any(|d| d.panneau == pane) || self.dock.as_ref().is_none_or(|s| est_devant(s, pane));
        match regle_clic(ouvert, devant) {
            Clic::Ouvrir | Clic::Fermer => self.toggle_pane(pane),
            Clic::Avancer => self.show_pane(pane),
        }
    }

    /// Basculer la barre elle-même (Affichage > Panneaux), retenu dans les réglages.
    pub(crate) fn toggle_panel_bar(&mut self) {
        self.settings.panel_bar = !self.settings.panel_bar;
        let _ = self.settings.save();
    }

    /// La barre à l'écran : une icône par panneau, enfoncée quand le panneau est dans l'espace de
    /// travail.
    pub(crate) fn panel_bar(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
            for (g, groupe) in GROUPES.iter().enumerate() {
                if g > 0 {
                    ui.add_space(3.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 1.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, Color32::from_gray(70));
                    ui.add_space(3.0);
                }
                let animation = self.mode == Mode::Animation;
                for e in groupe.iter().filter(|e| e.visible(animation)) {
                    let on = self.pane_shown(e.pane);
                    let resp = ui.add_sized([24.0, 26.0], egui::SelectableLabel::new(on, RichText::new(e.pane.icon()).size(17.0)));
                    if self.aide(resp, e.nom, e.description, e.action).clicked() {
                        self.run_action(e.action);
                    }
                }
            }
        });
    }
}
