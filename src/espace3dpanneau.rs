//! Le panneau « Espace 3D » (`docs/INTERFACE.md`, parties 150 et 152). Anthony, 2026-11-12 : « même dans l'espace 3D, le
//! panneau réglages affiche les réglages de l'outil sélectionné. Les réglages de l'espace 3D basculent dans un panneau
//! spécifique ». Sa place : d'abord « en dessous de réglages, à la place de la couleur » (partie 150), puis, vu à l'écran
//! le 2026-11-13 (il n'y restait qu'une centaine de points de haut) : « Place-le à côté de réglages plutôt. »
//!
//! **Ce qu'il montre.** Vue ouverte : les réglages de la vue (`App::espace3d_options_panel`, `espace3dui.rs`) : le repère
//! des poignées, l'aimantation et ses pas, la grille, « Plan en Espace 3D », « Nouvelle surface », la liste des cartes, les
//! champs et les boutons de clés de la carte choisie, la caméra du plan. Vue fermée : une phrase, et le bouton qui ouvre
//! la vue. Les réglages de l'outil en main (pinceau, crayon, gomme, Trajectoire dans l'espace…) sont dans le panneau
//! Réglages, que la vue soit ouverte ou non (`App::tool_options_panel`).
//!
//! **Il suit la vue.** Sa place est une colonne à lui, à droite de celle des réglages de l'outil, sur toute sa hauteur
//! (`DockLayout::poser_en_colonne`) : les réglages de l'outil, la couleur dessous, et lui se voient en même temps. La
//! colonne **arrive quand la vue s'ouvre** (sa largeur est prise au dessin) et **repart quand elle se ferme** : le reste
//! du temps elle ne prend aucune place. Si le panneau est déjà dans la disposition (ouvert à la main, ou rangé ailleurs :
//! un onglet d'un autre groupe, un panneau flottant, sa propre fenêtre), la vue ne le déplace pas : elle le met devant
//! les onglets de son groupe, et rend l'onglet d'avant en se fermant. Rangé ailleurs ou fermé pendant que la vue est
//! ouverte, rien n'est défait à la fermeture. La disposition retenue dans les réglages est celle sans la colonne que la
//! vue a posée pour un moment (`App::current_layout`).

use eframe::egui::{self, RichText};
use egui_dock::{Node, NodeIndex};
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
    /// L'onglet que le panneau a remplacé devant son groupe quand la vue s'est ouverte ; rien s'il y était déjà devant,
    /// s'il y est seul, ou s'il est dans sa propre fenêtre.
    pub avant: Option<Pane>,
    /// La vue, en s'ouvrant, a posé le panneau dans sa colonne à côté des réglages de l'outil : elle la retire en se
    /// fermant, s'il y est encore.
    pub colonne: bool,
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
        let description = "Les calques (cartes) et la caméra du plan dans un seul espace : les déplacer, les tourner, les mettre à l'échelle, pour la profondeur et la parallaxe. Ce panneau s'ouvre alors tout seul à côté des réglages de l'outil, et se referme avec la vue.";
        if self.aide(resp, "Espace 3D", description, "espace3d.basculer").clicked() {
            self.run_action("espace3d.basculer");
        }
    }

    /// À chaque image d'écran, avant les panneaux : la vue Espace 3D vient de s'ouvrir, son panneau arrive dans sa colonne
    /// à côté des réglages de l'outil (ou passe devant, là où on l'a rangé) ; elle vient de se fermer, la colonne repart
    /// (ou l'onglet qui était devant revient).
    pub(crate) fn espace3d_panneau_suivre(&mut self) {
        let vue = self.espace3d_vue_ouverte();
        // (La colonne posée par la vue, fermée à la main entre-temps : rouverte ensuite à la main, elle est à l'utilisateur.)
        if self.e3_panneau.colonne && !self.pane_shown(Pane::Espace3d) {
            self.e3_panneau.colonne = false;
        }
        // (Sans disposition encore posée, on attend l'image suivante.)
        if vue == self.e3_panneau.vue || self.dock.is_none() {
            return;
        }
        self.e3_panneau.vue = vue;
        if vue {
            self.espace3d_panneau_montrer();
        } else if std::mem::take(&mut self.e3_panneau.colonne) {
            self.espace3d_panneau_retirer();
        } else if let Some(avant) = self.e3_panneau.avant.take() {
            self.espace3d_panneau_rendre(avant);
        }
    }

    /// La largeur, en points, de ce que les panneaux occupent dans la fenêtre principale, telle qu'egui_dock l'a posée à
    /// la dernière image d'écran ; rien avant la première.
    fn largeur_des_panneaux(&self) -> Option<f32> {
        let tree = self.dock.as_ref()?.main_surface();
        if tree.len() == 0 {
            return None;
        }
        let rect = match &tree[NodeIndex::root()] {
            Node::Leaf { rect, .. } | Node::Vertical { rect, .. } | Node::Horizontal { rect, .. } => *rect,
            Node::Empty => return None,
        };
        (rect.is_finite() && rect.width() > 0.0).then(|| rect.width())
    }

    /// La vue s'ouvre. Le panneau n'est pas dans la disposition : il arrive dans sa colonne, à droite de celle des
    /// réglages de l'outil (`DockLayout::poser_en_colonne` ; sans réglages de l'outil dans la fenêtre principale, la
    /// place que `DockLayout::open` lui trouve, un onglet de plus). Il y est déjà : il passe devant les onglets de son
    /// groupe, là où il est, et l'onglet d'avant est retenu.
    fn espace3d_panneau_montrer(&mut self) {
        let pane = Pane::Espace3d;
        // (Dans sa propre fenêtre, il se voit déjà.)
        if self.detached.iter().any(|d| d.panneau == pane) {
            return;
        }
        if self.dock.as_ref().is_some_and(|s| s.find_tab(&pane).is_none()) {
            let mut layout = self.dock_layout();
            let largeur = crate::dock::part_de_la_colonne_espace3d(self.largeur_des_panneaux());
            self.e3_panneau.colonne = layout.poser_en_colonne(pane, largeur);
            if !self.e3_panneau.colonne {
                layout.open(pane);
            }
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

    /// La vue se ferme : la colonne qu'elle avait posée repart, si le panneau y est encore (seul, à droite des réglages
    /// de l'outil) ; sa largeur revient au dessin. Rangé ailleurs entre-temps, le panneau reste où on l'a mis.
    fn espace3d_panneau_retirer(&mut self) {
        let mut layout = self.dock_layout();
        if layout.retirer_la_colonne(Pane::Espace3d) {
            self.set_dock_layout(layout);
        }
    }

    /// La vue se ferme : `avant` repasse devant le panneau, s'ils partagent encore leur groupe d'onglets et si le panneau
    /// y est encore devant.
    fn espace3d_panneau_rendre(&mut self, avant: Pane) {
        let Some(state) = self.dock.as_mut() else { return };
        let (Some(ici), Some(la)) = (state.find_tab(&Pane::Espace3d), state.find_tab(&avant)) else { return };
        let devant = matches!(&state[ici.0][ici.1], Node::Leaf { active, .. } if *active == ici.2);
        if devant && (ici.0, ici.1) == (la.0, la.1) {
            state.set_active_tab(la);
        }
    }
}
