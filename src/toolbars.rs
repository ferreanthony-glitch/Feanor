//! Barres fixes (6a) : barre de commandes en haut (icônes des commandes courantes) et barre
//! d'outils verticale à gauche. Disposition inspirée des logiciels de storyboard (une barre de
//! commandes, les outils en colonne), avec les icônes de Pelure. Bulles d'aide : `bulles.rs`.

use eframe::egui::{self, vec2, Color32, RichText, Shape, Stroke};

use crate::app::{App, Tool};

/// Cases de la barre d'outils verticale, en groupes séparés par un trait. Une case à plusieurs
/// outils montre le dernier pris (au début, le premier) et les affiche tous au clic maintenu
/// (Anthony, 2026-10-17 : « les outils lignes, rectangle, ellipse et polyligne sont dans un seul
/// groupe qui s'affiche quand le clic est maintenu sur l'icône de base (le rectangle) » ;
/// « ajouter l'outil sélection au-dessus du pinceau. Dans le même groupe, l'outil de sélection
/// par la couleur »).
pub(crate) const TOOL_GROUPS: &[&[&[Tool]]] = &[
    &[&[Tool::Select, Tool::SelectColor], &[Tool::Brush], &[Tool::Pencil], &[Tool::Eraser], &[Tool::Cutter], &[Tool::Fill]],
    &[&[Tool::Rect, Tool::Line, Tool::Ellipse, Tool::Polyline], &[Tool::Text]],
    // Transformer et Perspective dans une seule case (Anthony, 2026-10-18).
    &[&[Tool::Transform, Tool::Perspective], &[Tool::Picker]],
    &[&[Tool::Pan], &[Tool::Camera], &[Tool::Ref3d]],
];

/// Clic maintenu si longtemps (secondes) sur une case à plusieurs outils : ils s'affichent.
pub const FLYOUT_HOLD: f64 = 0.35;

/// Outil montré par une case : l'outil courant s'il en est, sinon le plus récent de ses outils
/// (`recent` : du plus récent au plus ancien), sinon le premier.
pub(crate) fn slot_tool(slot: &[Tool], current: Tool, recent: &[Tool]) -> Tool {
    if slot.contains(&current) {
        return current;
    }
    recent.iter().copied().find(|t| slot.contains(t)).unwrap_or(slot[0])
}

impl App {
    /// Un bouton icône de la barre de commandes ; `on` : commande active (affichée enfoncée).
    pub(crate) fn command_button(&mut self, ui: &mut egui::Ui, icon: &str, action: &str, name: &str, description: &str, on: Option<bool>) {
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
            self.command_button(ui, ph::FOLDER_OPEN, "fichier.ouvrir", "Ouvrir", "Ouvrir un projet SHOT.", None);
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
            // Les commandes de la vue sont sous le dessin (`viewbar::view_tools_bar`).
            self.command_button(ui, ph::SKIP_BACK, "image.premiere", "Première image", "Première image du plan.", None);
            self.command_button(ui, ph::CARET_LEFT, "image.precedente", "Image précédente", "", None);
            let play = if self.playing { ph::PAUSE } else { ph::PLAY };
            self.command_button(ui, play, "lecture.basculer", "Lecture / pause", "", None);
            self.command_button(ui, ph::CARET_RIGHT, "image.suivante", "Image suivante", "", None);
            self.command_button(ui, ph::SKIP_FORWARD, "image.derniere", "Dernière image", "Dernière image du plan.", None);
            let looping = self.looping;
            let resp = ui.add_sized([28.0, 26.0], egui::SelectableLabel::new(looping, RichText::new(ph::REPEAT).size(17.0)));
            if self.aide(resp, "Lecture en boucle", "Reprendre au début une fois à la fin.", "lecture.boucle").clicked() {
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
                    for (s, slot) in group.iter().enumerate() {
                        self.tool_slot(ui, g, s, slot);
                    }
                }
                // Les couleurs d'avant-plan et d'arrière-plan, au pied de la barre (passe couleur).
                ui.add_space(8.0);
                self.tool_bar_colors(ui);
            });
        });
        self.tool_flyout_ui(ui.ctx());
    }

    /// Une case de la barre d'outils : son outil montré ; s'il y en a plusieurs, un petit coin et,
    /// au clic maintenu, la liste de ses outils.
    fn tool_slot(&mut self, ui: &mut egui::Ui, g: usize, s: usize, slot: &[Tool]) {
        let shown = slot_tool(slot, self.tool, &self.tool_recent);
        let icon = RichText::new(shown.icon()).size(19.0);
        let resp = ui.add_sized([34.0, 30.0], egui::SelectableLabel::new(slot.contains(&self.tool), icon));
        let mut description = shown.description().to_string();
        if slot.len() > 1 {
            let r = resp.rect;
            let c = r.right_bottom() - vec2(4.0, 4.0);
            let corner = vec![c, c - vec2(5.0, 0.0), c - vec2(0.0, 5.0)];
            ui.painter().add(Shape::convex_polygon(corner, ui.visuals().text_color().gamma_multiply(0.7), Stroke::NONE));
            if resp.is_pointer_button_down_on() {
                // L'image d'écran suivante vient d'elle-même : le temps de tenir se mesure sans bouger.
                ui.ctx().request_repaint();
                let held = ui.input(|i| i.pointer.press_start_time().is_some_and(|t| i.time - t >= FLYOUT_HOLD));
                if held && self.tool_flyout.is_none() {
                    self.tool_flyout = Some((g, s, r));
                }
            }
            description.push_str(" Clic maintenu : les autres outils de la case.");
        }
        let resp = self.aide(resp, shown.label(), &description, shown.action());
        // Le relâché d'un clic maintenu n'est pas un choix : la liste reste ouverte.
        if resp.clicked() && self.tool_flyout.is_none() {
            self.set_tool(shown);
        }
    }

    /// Liste des outils d'une case, à droite de celle-ci. Un clic (ou le relâché du clic maintenu)
    /// sur un outil le prend ; un appui ailleurs ou Échap la ferme.
    fn tool_flyout_ui(&mut self, ctx: &egui::Context) {
        let Some((g, s, rect)) = self.tool_flyout else { return };
        let Some(slot) = TOOL_GROUPS.get(g).and_then(|group| group.get(s)) else {
            self.tool_flyout = None;
            return;
        };
        let released = ctx.input(|i| i.pointer.primary_released());
        let mut pick = None;
        let area = egui::Area::new(egui::Id::new("outils_de_la_case"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.right_top() + vec2(4.0, 0.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(180.0);
                    for &tool in slot.iter() {
                        let key = self.raccourci(tool.action());
                        let text = format!("{}  {}", tool.icon(), tool.label());
                        let resp = ui
                            .horizontal(|ui| {
                                let resp = ui.add(egui::SelectableLabel::new(self.tool == tool, RichText::new(text).size(14.0)));
                                if !key.is_empty() {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(key).small().weak());
                                    });
                                }
                                resp
                            })
                            .inner;
                        if resp.clicked() || (released && resp.contains_pointer()) {
                            pick = Some(tool);
                        }
                    }
                });
            });
        let inside = area.response.rect;
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !inside.contains(p)));
        if let Some(tool) = pick {
            self.set_tool(tool);
            self.tool_flyout = None;
        } else if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.tool_flyout = None;
        }
    }
}
