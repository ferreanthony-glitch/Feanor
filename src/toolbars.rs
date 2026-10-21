//! Barres fixes (6a) : barre de commandes en haut (icônes des commandes courantes) et barre
//! d'outils verticale à gauche. Disposition inspirée des logiciels de storyboard (une barre de
//! commandes, les outils en colonne), avec les icônes de Pelure. Bulles d'aide : `bulles.rs`.

use eframe::egui::{self, vec2, RichText, Shape, Stroke};

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
    &[&[Tool::Pan], &[Tool::Camera]],
];

/// La colonne d'outils de l'espace Vignettes (« rien de distraction, que du dessin ») : le pinceau, le crayon et la
/// gomme. Les autres outils restent à leurs raccourcis (Alt + lettre) et dans la palette (Ctrl+K). Une case de plus
/// (le stylo d'annotation de la planche) vient se poser ici.
pub(crate) const TOOL_GROUPS_VIGNETTES: &[&[&[Tool]]] = &[&[&[Tool::Brush], &[Tool::Pencil], &[Tool::Eraser]]];

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
    /// L'icône d'un outil dans la barre : celle du mode en cours pour l'outil Déformer (courbe de
    /// Bézier ou perspective).
    pub(crate) fn tool_icon(&self, tool: Tool) -> &'static str {
        if tool == Tool::Perspective && self.deform_mode == crate::envelopetools::DeformMode::Envelope {
            egui_phosphor::regular::BEZIER_CURVE
        } else {
            tool.icon()
        }
    }

    /// Un bouton icône de la barre de commandes ; `on` : commande active (affichée enfoncée).
    pub(crate) fn command_button(&mut self, ui: &mut egui::Ui, icon: &str, action: &str, name: &str, description: &str, on: Option<bool>) {
        if self.case_icone(ui, icon, on, name, description, action).clicked() {
            self.run_action(action);
        }
    }

    /// Une case d'icône des barres (même taille, même icône, mêmes écarts partout, `barres::CASE`) avec sa bulle
    /// et son clic droit « Assigner un raccourci » ; `on` : enfoncée quand c'est vrai, plate sinon. Le clic est à
    /// l'appelant.
    pub(crate) fn case_icone(&self, ui: &mut egui::Ui, icon: &str, on: Option<bool>, name: &str, description: &str, action: &str) -> egui::Response {
        let text = RichText::new(icon).size(crate::barres::ICONE);
        let resp = match on {
            Some(on) => ui.add_sized(crate::barres::CASE, egui::SelectableLabel::new(on, text)),
            None => ui.add_sized(crate::barres::CASE, egui::Button::new(text).frame(false)),
        };
        self.aide(resp, name, description, action)
    }

    /// Barre de commandes, sous les menus : Nouveau / Ouvrir / Enregistrer / Exporter, Annuler /
    /// Rétablir, Couper / Copier / Coller ; le reste (les plans, le transport, le son) sous « Plus… »
    /// (`barres.rs`). Les commandes de la vue sont sous le dessin (`viewbar::view_tools_bar`).
    pub(crate) fn command_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            self.barre_ui(ui, crate::barres::Barre::Commandes);
        });
    }

    /// Barre d'outils verticale, à gauche.
    pub(crate) fn vertical_tool_bar(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, crate::barres::ECART);
                ui.add_space(4.0);
                let groups = if self.espace == crate::workspace::Espace::Vignettes { TOOL_GROUPS_VIGNETTES } else { TOOL_GROUPS };
                for (g, group) in groups.iter().enumerate() {
                    if g > 0 {
                        ui.add_space(crate::barres::MARGE_TRAIT);
                        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 1.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, crate::theme::bord());
                        ui.add_space(crate::barres::MARGE_TRAIT);
                    }
                    for (s, slot) in group.iter().enumerate() {
                        self.tool_slot(ui, g, s, slot);
                    }
                }
                // Sur la planche des Vignettes : le stylo et la gomme d'annotation (`annotationsui.rs`).
                self.annot_outils_ui(ui, crate::barres::CASE, crate::barres::ICONE);
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
        let icon = RichText::new(self.tool_icon(shown)).size(crate::barres::ICONE);
        let resp = ui.add_sized(crate::barres::CASE, egui::SelectableLabel::new(slot.contains(&self.tool), icon));
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
        let groups = if self.espace == crate::workspace::Espace::Vignettes { TOOL_GROUPS_VIGNETTES } else { TOOL_GROUPS };
        let Some(slot) = groups.get(g).and_then(|group| group.get(s)) else {
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
                        let text = format!("{}  {}", self.tool_icon(tool), tool.label());
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
