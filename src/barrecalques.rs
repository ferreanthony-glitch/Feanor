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
        // (Les calques 3D ne se disent que dans la vue de la scène, partie 159.)
        let quoi = if self.espace3d_vue_ouverte() {
            "Dessin vectoriel ou bitmap, calque de couleur, couleur unie, calque d'effet, annotation, vidéo, dossier, calques 3D, et « Nom, groupe, place… »."
        } else {
            "Dessin vectoriel ou bitmap, calque de couleur, couleur unie, calque d'effet, annotation, vidéo, dossier, et « Nom, groupe, place… »."
        };
        bulle(resp, NOMS[0], quoi, "");
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
        self.plus_des_calques(ui, case(ph::DOTS_THREE), &[]);
    }

    /// Le « ⋯ » des commandes des calques, dans `bouton` : d'abord les commandes de la barre qui n'y ont pas tenu (`caches`, rangs
    /// dans `NOMS`, partie 179), puis les groupes de calques et les opérations sur les calques.
    fn plus_des_calques(&mut self, ui: &mut egui::Ui, bouton: egui::Button<'_>, caches: &[usize]) {
        let resp = egui::menu::menu_custom_button(ui, bouton, |ui| {
            for &k in caches {
                match k {
                    0 => {
                        ui.menu_button(format!("{}  {}", ph::PLUS, NOMS[0]), |ui| self.layer_new_menu(ui));
                    }
                    1 => self.action_item(ui, &format!("{}  {}", ph::FOLDER_PLUS, NOMS[1]), "dossier.nouveau"),
                    2 => {
                        ui.menu_button(format!("{}  {}", ph::SPARKLE, NOMS[2]), |ui| self.effect_preset_menu(ui));
                    }
                    3 => self.action_item(ui, &format!("{}  {}", ph::COPY_SIMPLE, NOMS[3]), "calque.dupliquer"),
                    4 => self.action_item(ui, &format!("{}  {}", ph::TRASH, NOMS[4]), "calque.supprimer"),
                    _ => {}
                }
            }
            if !caches.is_empty() {
                ui.separator();
            }
            if ui.button(format!("{}  Groupes de calques…", ph::SWATCHES)).on_hover_text("Noms, couleurs, et quels groupes la timeline montre.").clicked() {
                ui.close_menu();
                self.groups_window = !self.groups_window;
            }
            ui.menu_button(format!("{}  Opérations sur les calques", ph::LIST), |ui| self.layer_ops_menu(ui));
            // Partie 179.10 (Anthony : « le repli des calques soit une option ») : un réglage d'affichage, ici et dans les Préférences.
            // (Pas dans la barre du bas de l'espace Montage, qui garde son « Déplier le calque courant ».)
            if self.espace != crate::workspace::Espace::Montage {
                ui.separator();
                self.deplier_calque_actif_case(ui);
            }
        })
        .response;
        bulle(
            resp,
            NOMS[5],
            "Groupes de calques ; les opérations sur les calques (nouveau, dupliquer, supprimer, fusionner, choisir, afficher, pochoirs…). Ctrl + clic sur une ligne pour choisir plusieurs calques, Maj + clic pour une suite.",
            "",
        );
    }

    /// Partie 179 (Anthony, sur la timeline du Storyboard : les boutons de la barre du bas de la colonne entourés, une flèche vers la
    /// ligne « ⊕ Calques ») : les commandes de la barre d'actions des calques sur la ligne d'en-tête « Calques », entre `x` et `fin`,
    /// dans la rangée `r` (les mêmes icônes, bulles, raccourcis et clics droits qu'en bas ; le clic droit de la ligne, hors d'un
    /// bouton : nouveau calque, coller le calque). Partie 179.5 (Anthony : le groupe entouré, une flèche vers la droite jusqu'aux
    /// icônes des colonnes, barrées) : **justifiées à droite**, le dernier bouton (« ⋯ ») finit à `fin`. Quand la place manque
    /// (colonne étroite), « Supprimer », puis « Dupliquer », « Nouveau calque d'effet » et « Nouveau dossier » passent en tête de « ⋯ »,
    /// qui reste avec « Nouveau calque ». Rend le bord gauche du premier bouton posé.
    pub(crate) fn actions_des_calques_en_ligne(&mut self, ui: &mut egui::Ui, r: egui::Rect, x: f32, fin: f32) -> f32 {
        // (Les cases suivent la taille des icônes de la timeline, `tailleicones.rs` : 18 points pour des icônes de 13.)
        let case_w = crate::tailleicones::mesure(CASE[0]);
        let pas = case_w + ECART_EN_LIGNE;
        let place = fin - x;
        // Combien des quatre boutons du milieu tiennent avec « + » et « ⋯ ».
        let milieu = ((((place + ECART_EN_LIGNE) / pas).floor() as i32) - 2).clamp(0, 4) as usize;
        let caches: Vec<usize> = (1 + milieu..5).collect();
        let n = 2 + milieu;
        let debut = fin - n as f32 * pas + ECART_EN_LIGNE;
        let ligne = egui::Rect::from_min_max(egui::pos2(debut, r.top()), egui::pos2(fin, r.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(ligne).id_salt("actions_calques_en_ligne").layout(egui::Layout::left_to_right(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = ECART_EN_LIGNE;
        child.spacing_mut().button_padding = vec2(0.0, 0.0);
        child.set_clip_rect(ligne.intersect(ui.clip_rect()));
        let ui = &mut child;
        let icone_w = crate::tailleicones::icone();
        let case = |icone: &str| egui::Button::new(RichText::new(icone).size(icone_w)).frame(false).min_size(vec2(case_w, CASE[1].min(r.height())));
        let resp = egui::menu::menu_custom_button(ui, case(ph::PLUS), |ui| self.layer_new_menu(ui)).response;
        let quoi = if self.espace3d_vue_ouverte() {
            "Dessin vectoriel ou bitmap, calque de couleur, couleur unie, calque d'effet, annotation, vidéo, dossier, calques 3D, et « Nom, groupe, place… »."
        } else {
            "Dessin vectoriel ou bitmap, calque de couleur, couleur unie, calque d'effet, annotation, vidéo, dossier, et « Nom, groupe, place… »."
        };
        bulle(resp, NOMS[0], quoi, "");
        if milieu >= 1 {
            let resp = ui.add(case(ph::FOLDER_PLUS));
            if self.aide(resp, NOMS[1], "Un dossier de calques vide ; glisser des calques dedans, ou clic droit sur un calque > Organiser > Dossiers.", "dossier.nouveau").clicked() {
                self.new_folder();
            }
        }
        if milieu >= 2 {
            let resp = egui::menu::menu_custom_button(ui, case(ph::SPARKLE), |ui| self.effect_preset_menu(ui)).response;
            bulle(resp, NOMS[2], "Un effet sur tout ce qui est dessous : lueur, flash, grain… Ses préréglages, ou un calque d'effet vide.", "");
        }
        if milieu >= 3 {
            let resp = ui.add(case(ph::COPY_SIMPLE));
            if self.aide(resp, NOMS[3], "Une copie de chaque calque choisi et de ses dessins, juste au-dessus de lui.", "calque.dupliquer").clicked() {
                self.duplicate_chosen_layers();
            }
        }
        if milieu >= 4 {
            let resp = ui.add(case(ph::TRASH));
            if self.aide(resp, NOMS[4], "Le calque courant et ceux choisis par Ctrl + clic sur leur ligne.", "calque.supprimer").clicked() {
                self.delete_chosen_layers();
            }
        }
        self.plus_des_calques(ui, case(ph::DOTS_THREE), &caches);
        debut
    }

    /// Partie 179 : le clic droit d'une ligne d'en-tête « Calques », hors d'un bouton (c'était celui de la barre du bas) : nouveau
    /// calque, coller le calque. `resp` : la ligne entière.
    pub(crate) fn menu_vide_des_calques(&mut self, resp: &egui::Response) {
        resp.context_menu(|ui| {
            ui.menu_button(format!("{}  Nouveau calque", ph::PLUS), |ui| self.layer_new_menu(ui));
            self.action_item(ui, "Coller le calque", "calque.coller");
        });
    }
}

/// Partie 179 : l'écart entre deux boutons d'action sur la ligne « Calques » (2 points : la ligne est courte). (Partie 179.5 : la
/// taille de leurs icônes est celle de toute la timeline, `tailleicones::icone`.)
pub(crate) const ECART_EN_LIGNE: f32 = 2.0;
