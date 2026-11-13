//! Le panneau « Espace 3D » (`docs/INTERFACE.md`, parties 150 et 152). Anthony, 2026-11-12 : « même dans l'espace 3D, le
//! panneau réglages affiche les réglages de l'outil sélectionné. Les réglages de l'espace 3D basculent dans un panneau
//! spécifique ». Sa place : d'abord « en dessous de réglages, à la place de la couleur » (partie 150), puis, vu à l'écran
//! le 2026-11-13 (il n'y restait qu'une centaine de points de haut) : « Place-le à côté de réglages plutôt. » Une colonne
//! à lui a été essayée ; entre elle et un onglet du groupe de Réglages, comme il l'avait rangé lui-même dans son essai,
//! il a choisi : « Onglet de Réglages ».
//!
//! **Ce qu'il montre.** Vue ouverte : les réglages de la vue (`App::espace3d_options_panel`, `espace3dui.rs`) : le repère
//! des poignées, l'aimantation et ses pas, la grille, « Plan en Espace 3D », « Nouvelle surface », la liste des cartes, les
//! champs et les boutons de clés de la carte choisie, la caméra du plan. Vue fermée : une phrase, et le bouton qui ouvre
//! la vue. Les réglages de l'outil en main (pinceau, crayon, gomme, Trajectoire dans l'espace…) sont dans le panneau
//! Réglages, que la vue soit ouverte ou non (`App::tool_options_panel`).
//!
//! **Il suit la vue.** Sa place est un onglet du groupe de Réglages, après Réglages et la fiche du plan
//! (`DockLayout::open`) : l'un ou l'autre devant. Il n'est pas dans les dispositions d'origine. **Quand la vue s'ouvre**,
//! il y arrive s'il n'est pas dans la disposition, et dans tous les cas il passe devant les onglets de son groupe ;
//! l'onglet qui était devant (Réglages, d'ordinaire) est retenu. **Quand elle se ferme**, cet onglet repasse devant, si le
//! panneau y est encore devant, dans le même groupe. L'onglet **reste** dans la disposition : c'est un onglet comme les
//! autres, qu'on peut fermer ou glisser ailleurs, et la vue ne le déplace jamais (rangé ailleurs, elle le met seulement
//! devant là où il est). Si les onglets ont été rangés autrement pendant que la vue était ouverte (un autre onglet mis
//! devant, le panneau glissé ailleurs ou fermé), rien n'est défait à la fermeture. La disposition retenue dans les
//! réglages garde l'onglet d'avant devant, pas celui que la vue y a mis pour un moment (`App::current_layout`,
//! `DockLayout::remettre_devant`).

use eframe::egui::{self, RichText};
use egui_dock::Node;
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::dock::Pane;

/// La phrase du panneau quand la vue est fermée.
pub(crate) const VUE_FERMEE: &str = "La vue Espace 3D est fermée : ouvre-la pour régler ici ses cartes (les calques du plan) et la caméra du plan.";
/// La phrase du panneau sur la planche des Vignettes, qui ne montre pas la vue.
pub(crate) const SUR_LA_PLANCHE: &str = "La vue Espace 3D s'ouvre sur le dessin d'un plan, dans le Storyboard ou l'Animation : la planche des Vignettes ne la montre pas.";
/// Le bouton du panneau quand la vue est fermée.
pub(crate) const OUVRIR: &str = "Ouvrir la vue Espace 3D";

/// Ce que le panneau retient de la vue, d'une image d'écran à l'autre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Suivi {
    /// La vue Espace 3D était montrée à la dernière image d'écran.
    pub vue: bool,
    /// L'onglet que le panneau a remplacé devant son groupe quand la vue s'est ouverte (Réglages, d'ordinaire) ; rien s'il
    /// y était déjà devant, s'il y est seul, ou s'il est dans sa propre fenêtre.
    pub avant: Option<Pane>,
}

impl App {
    /// La vue Espace 3D est ce que montre le panneau du dessin : ouverte, ni cachée par la vue caméra, ni par la planche
    /// des Vignettes.
    pub(crate) fn espace3d_vue_ouverte(&self) -> bool {
        self.espace3d.on && !self.camera_view && !self.board_shown()
    }

    /// Le contenu du panneau « Espace 3D ».
    pub(crate) fn espace3d_panneau(&mut self, ui: &mut egui::Ui) {
        // Tout le film est écarté : rien à régler sur le plan vide qui le tient (`planannule.rs`).
        if self.panneau_film_vide(ui) {
            return;
        }
        if self.espace3d_vue_ouverte() {
            self.espace3d_options_panel(ui);
            return;
        }
        if self.board_shown() {
            ui.label(RichText::new(SUR_LA_PLANCHE).small().weak());
            return;
        }
        ui.label(RichText::new(VUE_FERMEE).small().weak());
        let resp = ui.button(format!("{}  {OUVRIR}", ph::CUBE));
        let description = "Les calques (cartes) et la caméra du plan dans un seul espace : les déplacer, les tourner, les mettre à l'échelle, pour la profondeur et la parallaxe. Ce panneau passe alors devant Réglages, dans le même groupe d'onglets ; Réglages revient devant quand la vue se ferme.";
        if self.aide(resp, "Espace 3D", description, "espace3d.basculer").clicked() {
            self.run_action("espace3d.basculer");
        }
    }

    /// À chaque image d'écran, avant les panneaux : la vue Espace 3D vient de s'ouvrir, son panneau passe devant les
    /// onglets de son groupe (il arrive d'abord en onglet de Réglages s'il n'est pas dans la disposition) ; elle vient de
    /// se fermer, l'onglet qui était devant revient.
    pub(crate) fn espace3d_panneau_suivre(&mut self) {
        let vue = self.espace3d_vue_ouverte();
        // (Sans disposition encore posée, on attend l'image suivante.)
        if vue == self.e3_panneau.vue || self.dock.is_none() {
            return;
        }
        self.e3_panneau.vue = vue;
        if vue {
            self.espace3d_panneau_montrer();
        } else if let Some(avant) = self.e3_panneau.avant.take() {
            self.espace3d_panneau_rendre(avant);
        }
    }

    /// La vue s'ouvre. Le panneau n'est pas dans la disposition : il y arrive à sa place (`DockLayout::open` : un onglet
    /// du groupe de Réglages, après ceux qui y sont ; sans Réglages dans la fenêtre principale, un onglet de son groupe), et
    /// il y reste ensuite. Puis, là où il est, il passe devant les onglets de son groupe, et l'onglet qui était devant est
    /// retenu (`Suivi::avant`).
    fn espace3d_panneau_montrer(&mut self) {
        let pane = Pane::Espace3d;
        // (Dans sa propre fenêtre, il se voit déjà.)
        if self.detached.iter().any(|d| d.panneau == pane) {
            return;
        }
        if self.dock.as_ref().is_some_and(|s| s.find_tab(&pane).is_none()) {
            let mut layout = self.dock_layout();
            layout.open(pane);
            self.set_dock_layout(layout);
        }
        let Some(state) = self.dock.as_mut() else { return };
        let Some(ici) = state.find_tab(&pane) else { return };
        let avant = match &state[ici.0][ici.1] {
            Node::Leaf { tabs, active, .. } if *active != ici.2 => tabs.get(active.0).copied(),
            _ => None,
        };
        state.set_active_tab(ici);
        self.e3_panneau.avant = avant;
    }

    /// La vue se ferme : `avant` (Réglages, d'ordinaire) repasse devant le panneau, s'ils partagent encore leur groupe
    /// d'onglets et si le panneau y est encore devant. Le panneau reste dans la disposition.
    fn espace3d_panneau_rendre(&mut self, avant: Pane) {
        let Some(state) = self.dock.as_mut() else { return };
        let (Some(ici), Some(la)) = (state.find_tab(&Pane::Espace3d), state.find_tab(&avant)) else { return };
        let devant = matches!(&state[ici.0][ici.1], Node::Leaf { active, .. } if *active == ici.2);
        if devant && (ici.0, ici.1) == (la.0, la.1) {
            state.set_active_tab(la);
        }
    }
}
