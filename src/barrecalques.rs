//! La barre d'actions des calques (partie 160, Anthony : « Nouveau calque · Nouveau dossier · Nouveau calque d'effet ·
//! Dupliquer · Supprimer · "⋯" (Groupes, Opérations), posée dans la barre du bas, sous la colonne des calques, pour ne jamais
//! bouger »). Elle remplace les boutons du haut du panneau Calques, qui n'existe plus, et ceux de l'en-tête de la colonne des
//! calques de la timeline (Nouveau dossier, Groupes, ☰).
//!
//! Elle est à gauche de la barre du bas de la timeline d'animation (avant les onglets Notes, Son…) et de la barre du bas de la
//! bande des plans (avant le zoom), de la largeur de la colonne des calques : quel que soit le nombre de calques ou de pistes son,
//! elle ne bouge pas. Un clic droit dans la barre, hors d'un bouton : nouveau calque, coller le calque (c'était le clic droit dans
//! le vide du panneau Calques).
//!
//! Pas de « masque » : il n'existe pas dans FEANOR ; le pochoir est un réglage du calque, sur sa ligne.

use eframe::egui::{self, vec2, RichText, Sense};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;

/// La taille d'une case de la barre, et l'écart entre deux cases (partie 161 : 16 points et 1 d'écart avant, des icônes qui se
/// touchaient).
pub(crate) const CASE: [f32; 2] = [18.0, 18.0];
pub(crate) const ECART: f32 = 5.0;
/// La taille des icônes de la barre.
const ICONE: f32 = 14.0;

/// La largeur où les six cases se lisent : la barre la prend au moins, même sous une colonne plus étroite (la colonne des
/// pistes du Storyboard, 104 points d'origine) ; plus large, les cases s'écartent pour remplir la largeur de la colonne.
pub(crate) const LARGEUR_LISIBLE: f32 = CASE[0] * 6.0 + ECART * 5.0;

/// Les noms (ceux des bulles) des commandes de la barre, dans l'ordre.
pub(crate) const NOMS: [&str; 6] = ["Nouveau calque", "Nouveau dossier", "Nouveau calque d'effet", "Dupliquer les calques choisis", "Supprimer les calques choisis", "Plus de commandes des calques"];

impl App {
    /// L'action `vue.calques` (l'icône, la case d'Affichage > Panneaux et le raccourci de l'ancien panneau Calques, partie 160) : la
    /// bande du bas se montre si elle était cachée, et la colonne des calques prend la main (Ctrl+C, Ctrl+V copient et collent le
    /// calque, comme dans l'ancien panneau). Dans les Vignettes, il n'y a pas de calques : un mot dans la barre d'état.
    pub(crate) fn montrer_colonne_des_calques(&mut self) {
        if self.espace == crate::workspace::Espace::Vignettes {
            self.status = "Vignettes : rien à ranger ici. La pile du plan est dans le Storyboard, sous le dessin.".into();
            return;
        }
        if !self.pane_shown(crate::dock::Pane::Montage) {
            self.show_pane(crate::dock::Pane::Montage);
        }
        self.take_layers_pane_focus();
        self.status = "Calques : la colonne de gauche de la timeline ; leurs commandes, dans la barre dessous.".into();
    }

    /// La barre d'actions, `largeur` points de large (celle de la colonne des calques), dans une rangée de barre.
    pub(crate) fn barre_calques(&mut self, ui: &mut egui::Ui, largeur: f32) {
        let largeur = largeur.max(LARGEUR_LISIBLE);
        let (rect, vide) = ui.allocate_exact_size(vec2(largeur, CASE[1]), Sense::click());
        // (Le clic droit dans la barre, posé avant les boutons : ils restent au-dessus et gardent leurs clics.)
        vide.context_menu(|ui| {
            ui.menu_button(format!("{}  Nouveau calque", ph::PLUS), |ui| self.layer_new_menu(ui));
            self.action_item(ui, "Coller le calque", "calque.coller");
        });
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt("barre_calques").layout(egui::Layout::left_to_right(egui::Align::Center)));
        // (Les cases s'écartent pour remplir la colonne, sans dépasser 12 points d'écart.)
        child.spacing_mut().item_spacing.x = ((largeur - CASE[0] * 6.0) / 5.0).clamp(ECART, 12.0);
        // (Une case a la largeur de son icône et deux points de chaque côté : `CASE`.)
        child.spacing_mut().button_padding = vec2(2.0, 1.0);
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        let ui = &mut child;
        let case = |icone: &str| egui::Button::new(RichText::new(icone).size(ICONE)).frame(false).min_size(vec2(CASE[0], CASE[1]));
        let resp = egui::menu::menu_custom_button(ui, case(ph::PLUS), |ui| self.layer_new_menu(ui)).response;
        bulle(resp, NOMS[0], "Dessin vectoriel ou bitmap, calque de couleur, couleur unie, calque d'effet, annotation, vidéo, dossier, calques 3D, et « Nom, groupe, place… ».", "");
        let resp = ui.add(case(ph::FOLDER_PLUS));
        if self.aide(resp, NOMS[1], "Un dossier de calques vide ; glisser des calques dedans, ou clic droit sur un calque > Organiser > Dossiers.", "dossier.nouveau").clicked() {
            self.new_folder();
        }
        let resp = egui::menu::menu_custom_button(ui, case(ph::SPARKLE), |ui| self.effect_preset_menu(ui)).response;
        bulle(resp, NOMS[2], "Un effet sur tout ce qui est dessous : lueur, flash, grain… Ses préréglages, ou un calque d'effet vide.", "");
        let resp = ui.add(case(ph::COPY_SIMPLE));
        if self.aide(resp, NOMS[3], "Une copie de chaque calque choisi et de ses dessins, juste au-dessus de lui.", "calque.dupliquer").clicked() {
            self.duplicate_chosen_layers();
        }
        let resp = ui.add(case(ph::TRASH));
        if self.aide(resp, NOMS[4], "Le calque courant et ceux choisis par Ctrl + clic sur leur ligne.", "calque.supprimer").clicked() {
            self.delete_chosen_layers();
        }
        let resp = egui::menu::menu_custom_button(ui, case(ph::DOTS_THREE), |ui| {
            if ui.button(format!("{}  Groupes de calques…", ph::SWATCHES)).on_hover_text("Noms, couleurs, et quels groupes la timeline montre.").clicked() {
                ui.close_menu();
                self.groups_window = !self.groups_window;
            }
            ui.menu_button(format!("{}  Opérations sur les calques", ph::LIST), |ui| self.layer_ops_menu(ui));
        })
        .response;
        bulle(
            resp,
            NOMS[5],
            "Groupes de calques ; les opérations sur les calques (nouveau, dupliquer, supprimer, fusionner, choisir, afficher, pochoirs…). Ctrl + clic sur une ligne pour choisir plusieurs calques, Maj + clic pour une suite.",
            "",
        );
    }
}
