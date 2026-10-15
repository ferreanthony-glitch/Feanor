//! Barres fixes (6a) : barre de commandes en haut (icônes des commandes courantes) et barre
//! d'outils verticale à gauche. Disposition inspirée des logiciels de storyboard (une barre de
//! commandes, les outils en colonne), avec les icônes de Pelure. Bulles d'aide : `bulles.rs`.

use eframe::egui::{self, vec2, Color32, RichText};

use crate::app::{App, BoardView, Mode, Tool};

/// Groupes de la barre d'outils verticale (séparés par un trait).
pub(crate) const TOOL_GROUPS: &[&[Tool]] = &[
    &[Tool::Brush, Tool::Pencil, Tool::Eraser, Tool::Fill, Tool::Stamp, Tool::Clone],
    &[Tool::Line, Tool::Rect, Tool::Ellipse, Tool::Polyline, Tool::Text],
    &[Tool::Select, Tool::Transform, Tool::Perspective, Tool::Picker],
    &[Tool::Pan, Tool::Camera, Tool::Ref3d],
];

impl App {
    /// Un bouton icône de la barre de commandes ; `on` : commande active (affichée enfoncée).
    fn command_button(&mut self, ui: &mut egui::Ui, icon: &str, action: &str, name: &str, description: &str, on: Option<bool>) {
        let text = RichText::new(icon).size(17.0);
        let resp = match on {
            Some(on) => ui.add_sized([28.0, 26.0], egui::SelectableLabel::new(on, text)),
            None => ui.add_sized([28.0, 26.0], egui::Button::new(text).frame(false)),
        };
        if self.aide(resp, name, description, action).clicked() {
            self.run_action(action);
        }
    }

    /// Barre de commandes, sous les menus.
    pub(crate) fn command_bar(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            let sep = |ui: &mut egui::Ui| {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
            };
            self.command_button(ui, ph::FILE_PLUS, "fichier.nouveau", "Nouveau projet", "Commencer un projet vide.", None);
            self.command_button(ui, ph::FOLDER_OPEN, "fichier.ouvrir", "Ouvrir", "Ouvrir un projet Pelure.", None);
            self.command_button(ui, ph::FLOPPY_DISK, "fichier.enregistrer", "Enregistrer", "Enregistrer le projet.", None);
            self.command_button(ui, ph::EXPORT, "fichier.exporter_film", "Exporter le film", "Exporter le montage.", None);
            sep(ui);
            self.command_button(ui, ph::ARROW_COUNTER_CLOCKWISE, "edition.annuler", "Annuler", "Défaire la dernière opération.", None);
            self.command_button(ui, ph::ARROW_CLOCKWISE, "edition.retablir", "Rétablir", "Refaire ce qui vient d'être annulé.", None);
            self.command_button(ui, ph::SCISSORS, "edition.couper", "Couper", "Couper la sélection.", None);
            self.command_button(ui, ph::COPY, "edition.copier", "Copier", "Copier la sélection.", None);
            self.command_button(ui, ph::CLIPBOARD, "edition.coller", "Coller", "Coller ce qui a été copié.", None);
            sep(ui);
            self.command_button(ui, ph::PLUS_SQUARE, "panneau.nouveau", "Nouveau plan", "Après le plan courant, dans la même séquence.", None);
            self.command_button(ui, ph::COPY_SIMPLE, "panneau.dupliquer", "Dupliquer le plan", "Une copie du plan courant, juste après lui.", None);
            self.command_button(ui, ph::TRASH, "panneau.supprimer", "Supprimer les plans choisis", "Le plan courant, et ceux choisis avec Ctrl ou Maj + clic.", None);
            sep(ui);
            let thumbs = self.board_view == BoardView::Panels && self.mode == Mode::Storyboard;
            self.command_button(ui, ph::SQUARES_FOUR, "vue.planches", "Vue dessin / vignettes", "Dessiner le plan, ou voir toutes ses images de storyboard en planche.", Some(thumbs));
            self.command_button(ui, ph::FRAME_CORNERS, "camera.vue", "Vue à travers la caméra", "Voir ce que filme la caméra du plan.", Some(self.camera_view));
            let in3d = self.stage3d.on && !self.camera_view;
            self.command_button(ui, ph::CUBE_FOCUS, "vue.3d", "Plateau en 3D", "Les calques à leur profondeur, la caméra et le sol, vus en 3D.", Some(in3d));
            self.command_button(ui, ph::ARROWS_OUT, "vue.ajuster", "Ajuster à la fenêtre", "Toute la feuille dans la vue.", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS_PLUS, "vue.zoom_avant", "Zoom avant", "", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS_MINUS, "vue.zoom_arriere", "Zoom arrière", "", None);
            self.command_button(ui, ph::ARROW_ARC_LEFT, "vue.rotation_antihoraire", "Tourner la vue", "Dans le sens antihoraire ; le dessin ne change pas.", None);
            self.command_button(ui, ph::ARROW_ARC_RIGHT, "vue.rotation_horaire", "Tourner la vue", "Dans le sens horaire ; le dessin ne change pas.", None);
            self.command_button(ui, ph::FLIP_HORIZONTAL, "vue.miroir", "Miroir de la vue", "Voir le dessin retourné, pour vérifier ses proportions ; le dessin ne change pas.", Some(self.view.flip));
            self.command_button(ui, ph::CIRCLES_THREE, "vue.pelure", "Pelure d'oignon", "Par défaut, les dessins d'avant en rouge, ceux d'après en vert.", Some(self.onion.enabled));
            self.command_button(ui, ph::LIGHTBULB, "vue.table_lumineuse", "Table lumineuse", "Les autres calques estompés, le calque courant net.", Some(self.light_table));
            sep(ui);
            self.command_button(ui, ph::SKIP_BACK, "image.premiere", "Première image", "Première image du plan.", None);
            self.command_button(ui, ph::CARET_LEFT, "image.precedente", "Image précédente", "", None);
            let play = if self.playing { ph::PAUSE } else { ph::PLAY };
            self.command_button(ui, play, "lecture.basculer", "Lecture / pause", "", None);
            self.command_button(ui, ph::CARET_RIGHT, "image.suivante", "Image suivante", "", None);
            self.command_button(ui, ph::SKIP_FORWARD, "image.derniere", "Dernière image", "Dernière image du plan.", None);
            let looping = self.looping;
            let resp = ui.add_sized([28.0, 26.0], egui::SelectableLabel::new(looping, RichText::new(ph::REPEAT).size(17.0)));
            if crate::bulles::bulle(resp, "Lecture en boucle", "Reprendre au début une fois à la fin.", "").clicked() {
                self.looping = !self.looping;
            }
            let sound = self.sound_on;
            let icon = if sound { ph::SPEAKER_HIGH } else { ph::SPEAKER_SLASH };
            let resp = ui.add_sized([28.0, 26.0], egui::SelectableLabel::new(sound, RichText::new(icon).size(17.0)));
            if self.aide(resp, "Son pendant la lecture", "Entendre les pistes son en lisant.", "son.lecture").clicked() {
                self.sound_on = !self.sound_on;
            }
        });
    }

    /// Barre d'outils verticale, à gauche.
    pub(crate) fn vertical_tool_bar(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                ui.add_space(4.0);
                for (g, group) in TOOL_GROUPS.iter().enumerate() {
                    if g > 0 {
                        ui.add_space(3.0);
                        let (rect, _) = ui.allocate_exact_size(vec2(26.0, 1.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, Color32::from_gray(70));
                        ui.add_space(3.0);
                    }
                    for &tool in group.iter() {
                        let icon = RichText::new(tool.icon()).size(19.0);
                        let resp = ui.add_sized([34.0, 30.0], egui::SelectableLabel::new(self.tool == tool, icon));
                        if self.aide(resp, tool.label(), tool.description(), tool.action()).clicked() {
                            self.set_tool(tool);
                        }
                    }
                }
                // La couleur courante, au pied de la barre.
                ui.add_space(8.0);
                ui.spacing_mut().interact_size = vec2(30.0, 24.0);
                let resp = ui.color_edit_button_srgb(&mut self.color);
                crate::bulles::bulle(resp, "Couleur courante", "Clic : la choisir.", "");
            });
        });
    }
}
