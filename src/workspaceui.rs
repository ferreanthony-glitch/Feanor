//! Espaces de travail dans l'application (4b) : appliquer, retenir, enregistrer, menu.

use eframe::egui;

use crate::app::{App, BoardView};
use crate::workspace::{self, Layout};

impl App {
    /// La disposition telle qu'elle est à l'écran.
    pub(crate) fn current_layout(&self) -> Layout {
        Layout {
            top_view: self.top_view,
            vignettes: self.board_view == BoardView::Panels,
            ..self.layout.clone()
        }
    }

    /// Applique une disposition : panneaux, tailles (les panneaux repartent de ces tailles),
    /// vue vignettes, vue de dessus.
    pub(crate) fn apply_layout(&mut self, layout: Layout) {
        let layout = layout.sanitized();
        self.top_view = layout.top_view;
        self.board_view = if layout.vignettes { BoardView::Panels } else { BoardView::Scene };
        self.layout = layout;
        self.layout_gen += 1;
    }

    /// Retient la disposition quand elle change (pas pendant qu'on tire un panneau).
    pub(crate) fn remember_layout(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let current = self.current_layout();
        let differs = self.settings.layout.as_ref().is_none_or(|l| !same_layout(l, &current));
        if differs {
            self.settings.layout = Some(current);
            let _ = self.settings.save();
        }
    }

    /// Actions des espaces de travail ; vrai si `action` en était une.
    pub(crate) fn workspace_action(&mut self, action: &str) -> bool {
        if let Some((_, name, layout)) = workspace::builtin().into_iter().find(|w| w.0 == action) {
            self.apply_layout(layout);
            self.status = format!("Espace de travail « {name} ».");
            return true;
        }
        match action {
            "espace.enregistrer" => self.workspace_name = Some(String::new()),
            "vue.panneau_outils" => self.layout.tools = !self.layout.tools,
            "vue.panneau_fiche" => self.layout.sheet = !self.layout.sheet,
            "vue.panneau_bande" => self.layout.strip = !self.layout.strip,
            _ => return false,
        }
        true
    }

    /// Sous-menu « Espaces de travail » (menu Affichage).
    pub(crate) fn workspace_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Espaces de travail", |ui| {
            for (action, name, _) in workspace::builtin() {
                self.action_item(ui, name, action);
            }
            let saved: Vec<String> = self.settings.workspaces.iter().map(|w| w.name.clone()).collect();
            if !saved.is_empty() {
                ui.separator();
                for name in &saved {
                    if ui.button(name).clicked() {
                        if let Some(w) = self.settings.workspaces.iter().find(|w| &w.name == name) {
                            let layout = w.layout.clone();
                            self.apply_layout(layout);
                            self.status = format!("Espace de travail « {name} ».");
                        }
                        ui.close_menu();
                    }
                }
            }
            ui.separator();
            self.action_item(ui, "Enregistrer la disposition…", "espace.enregistrer");
            if !saved.is_empty() {
                ui.menu_button("Supprimer un espace", |ui| {
                    for name in &saved {
                        if ui.button(name).clicked() {
                            self.settings.workspaces.retain(|w| &w.name != name);
                            let _ = self.settings.save();
                            self.status = format!("Espace de travail « {name} » supprimé.");
                            ui.close_menu();
                        }
                    }
                });
            }
        });
        ui.menu_button("Panneaux", |ui| {
            let checks = [
                ("Outils", "vue.panneau_outils", self.layout.tools),
                ("Fiche et calques", "vue.panneau_fiche", self.layout.sheet),
                ("Montage / timeline", "vue.panneau_bande", self.layout.strip),
            ];
            for (label, action, on) in checks {
                let mut v = on;
                let key = self.keymap.label(action);
                let text = if key.is_empty() { label.to_string() } else { format!("{label} ({key})") };
                if ui.checkbox(&mut v, text).changed() {
                    self.workspace_action(action);
                }
            }
        });
    }

    /// Fenêtre « Enregistrer la disposition » : un nom, puis enregistrer (remplace un espace du
    /// même nom).
    pub(crate) fn workspace_name_window(&mut self, ctx: &egui::Context) {
        let Some(mut name) = self.workspace_name.take() else { return };
        let mut done = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("nom_espace")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading("Enregistrer la disposition");
            ui.label("Nom de l'espace de travail :");
            let r = ui.text_edit_singleline(&mut name);
            r.request_focus();
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = true;
            }
            ui.horizontal(|ui| {
                done |= ui.add_enabled(!name.trim().is_empty(), egui::Button::new("Enregistrer")).clicked();
                cancel = ui.button("Annuler").clicked();
            });
        });
        if done && !name.trim().is_empty() {
            let layout = self.current_layout();
            workspace::save_workspace(&mut self.settings.workspaces, &name, layout);
            self.status = match self.settings.save() {
                Ok(()) => format!("Espace de travail « {} » enregistré (menu Affichage).", name.trim()),
                Err(e) => format!("Espace de travail non enregistré : {e}"),
            };
        } else if !cancel && !modal.should_close() {
            self.workspace_name = Some(name);
        }
    }
}

/// Deux dispositions identiques, à un pixel près (les tailles relues à l'écran bougent un peu).
fn same_layout(a: &Layout, b: &Layout) -> bool {
    let close = |x: f32, y: f32| (x - y).abs() < 1.0;
    a.tools == b.tools
        && a.sheet == b.sheet
        && a.strip == b.strip
        && a.vignettes == b.vignettes
        && a.top_view == b.top_view
        && close(a.sheet_width, b.sheet_width)
        && close(a.montage_height, b.montage_height)
        && close(a.timeline_height, b.timeline_height)
}
