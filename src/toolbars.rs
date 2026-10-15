//! Barres fixes (6a) : barre de commandes en haut (icônes des commandes courantes) et barre
//! d'outils verticale à gauche. Disposition inspirée des logiciels de storyboard (une barre de
//! commandes, les outils en colonne), avec les icônes de Pelure.

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
    /// Bulle d'une commande : son nom et son raccourci.
    pub(crate) fn command_hint(&self, action: &str, name: &str) -> String {
        let key = self.keymap.label(action);
        if key.is_empty() {
            name.to_string()
        } else {
            format!("{name}   {key}")
        }
    }

    /// Un bouton icône de la barre de commandes ; `on` : commande active (affichée enfoncée).
    fn command_button(&mut self, ui: &mut egui::Ui, icon: &str, action: &str, name: &str, on: Option<bool>) {
        let hint = self.command_hint(action, name);
        let text = RichText::new(icon).size(17.0);
        let resp = match on {
            Some(on) => ui.add_sized([28.0, 26.0], egui::SelectableLabel::new(on, text)),
            None => ui.add_sized([28.0, 26.0], egui::Button::new(text).frame(false)),
        };
        if resp.on_hover_text(hint).clicked() {
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
            self.command_button(ui, ph::FILE_PLUS, "fichier.nouveau", "Nouveau projet", None);
            self.command_button(ui, ph::FOLDER_OPEN, "fichier.ouvrir", "Ouvrir", None);
            self.command_button(ui, ph::FLOPPY_DISK, "fichier.enregistrer", "Enregistrer", None);
            self.command_button(ui, ph::EXPORT, "fichier.exporter_film", "Exporter le film", None);
            sep(ui);
            self.command_button(ui, ph::ARROW_COUNTER_CLOCKWISE, "edition.annuler", "Annuler", None);
            self.command_button(ui, ph::ARROW_CLOCKWISE, "edition.retablir", "Rétablir", None);
            self.command_button(ui, ph::SCISSORS, "edition.couper", "Couper", None);
            self.command_button(ui, ph::COPY, "edition.copier", "Copier", None);
            self.command_button(ui, ph::CLIPBOARD, "edition.coller", "Coller", None);
            sep(ui);
            self.command_button(ui, ph::PLUS_SQUARE, "panneau.nouveau", "Nouveau panneau", None);
            self.command_button(ui, ph::FOLDER_SIMPLE_PLUS, "scene.nouvelle", "Nouvelle scène", None);
            self.command_button(ui, ph::COPY_SIMPLE, "panneau.dupliquer", "Dupliquer le panneau", None);
            self.command_button(ui, ph::TRASH, "panneau.supprimer", "Supprimer le panneau", None);
            sep(ui);
            let thumbs = self.board_view == BoardView::Panels && self.mode == Mode::Storyboard;
            self.command_button(ui, ph::SQUARES_FOUR, "vue.planches", "Vue dessin / vignettes", Some(thumbs));
            self.command_button(ui, ph::FRAME_CORNERS, "camera.vue", "Vue à travers la caméra", Some(self.camera_view));
            let in3d = self.stage3d.on && !self.camera_view;
            self.command_button(ui, ph::CUBE_FOCUS, "vue.3d", "Plateau en 3D", Some(in3d));
            self.command_button(ui, ph::ARROWS_OUT, "vue.ajuster", "Ajuster à la fenêtre", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS_PLUS, "vue.zoom_avant", "Zoom avant", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS_MINUS, "vue.zoom_arriere", "Zoom arrière", None);
            self.command_button(ui, ph::ARROW_ARC_LEFT, "vue.rotation_antihoraire", "Tourner la vue (sens antihoraire)", None);
            self.command_button(ui, ph::ARROW_ARC_RIGHT, "vue.rotation_horaire", "Tourner la vue (sens horaire)", None);
            self.command_button(ui, ph::FLIP_HORIZONTAL, "vue.miroir", "Miroir de la vue", Some(self.view.flip));
            self.command_button(ui, ph::CIRCLES_THREE, "vue.pelure", "Pelure d'oignon", Some(self.onion.enabled));
            self.command_button(ui, ph::LIGHTBULB, "vue.table_lumineuse", "Table lumineuse", Some(self.light_table));
            sep(ui);
            self.command_button(ui, ph::SKIP_BACK, "image.premiere", "Première image du panneau", None);
            self.command_button(ui, ph::CARET_LEFT, "image.precedente", "Image précédente", None);
            let play = if self.playing { ph::PAUSE } else { ph::PLAY };
            self.command_button(ui, play, "lecture.basculer", "Lecture / pause", None);
            self.command_button(ui, ph::CARET_RIGHT, "image.suivante", "Image suivante", None);
            self.command_button(ui, ph::SKIP_FORWARD, "image.derniere", "Dernière image du panneau", None);
            let looping = self.looping;
            let resp = ui
                .add_sized([28.0, 26.0], egui::SelectableLabel::new(looping, RichText::new(ph::REPEAT).size(17.0)))
                .on_hover_text("Lecture en boucle");
            if resp.clicked() {
                self.looping = !self.looping;
            }
            let sound = self.sound_on;
            let icon = if sound { ph::SPEAKER_HIGH } else { ph::SPEAKER_SLASH };
            let resp = ui
                .add_sized([28.0, 26.0], egui::SelectableLabel::new(sound, RichText::new(icon).size(17.0)))
                .on_hover_text("Son pendant la lecture");
            if resp.clicked() {
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
                        let shortcut = self.keymap.label(tool.action());
                        let hint = if shortcut.is_empty() {
                            tool.label().to_string()
                        } else {
                            format!("{}   {shortcut}", tool.label())
                        };
                        let icon = RichText::new(tool.icon()).size(19.0);
                        if ui
                            .add_sized([34.0, 30.0], egui::SelectableLabel::new(self.tool == tool, icon))
                            .on_hover_text(hint)
                            .clicked()
                        {
                            self.set_tool(tool);
                        }
                    }
                }
                // La couleur courante, au pied de la barre.
                ui.add_space(8.0);
                ui.spacing_mut().interact_size = vec2(30.0, 24.0);
                ui.color_edit_button_srgb(&mut self.color).on_hover_text("Couleur");
            });
        });
    }
}
