//! Espaces de travail dans l'application (4b) : appliquer, retenir, enregistrer, menu.

use eframe::egui;

use crate::app::{App, BoardView};
use crate::dock::{Group, Pane};
use crate::workspace::{self, Layout};

impl App {
    /// La disposition telle qu'elle est à l'écran.
    pub(crate) fn current_layout(&self) -> Layout {
        let panes = self.dock_layout();
        Layout {
            tools: panes.contains(Pane::Tools),
            sheet: panes.contains(Pane::Sheet) || panes.contains(Pane::Layers),
            strip: panes.contains(Pane::Montage),
            panneaux: Some(panes),
            top_view: self.top_view,
            vignettes: self.board_view == BoardView::Panels,
            ..self.layout.clone()
        }
    }

    /// Applique une disposition : panneaux et leur place, vue vignettes, vue de dessus. Les cadres
    /// de composition (9d) restent ceux qu'on a choisis : ce sont des réglages de la vue.
    pub(crate) fn apply_layout(&mut self, layout: Layout) {
        let layout = Layout {
            safe_frames: self.layout.safe_frames,
            frame_format: self.layout.frame_format,
            ..layout
        }
        .sanitized();
        self.top_view = layout.top_view;
        self.board_view = if layout.vignettes { BoardView::Panels } else { BoardView::Scene };
        self.set_dock_layout(layout.dock_layout());
        self.layout = layout;
    }

    /// Retient la disposition quand elle change (pas pendant qu'on tire un panneau).
    pub(crate) fn remember_layout(&mut self, ctx: &egui::Context) {
        // (Ni pendant qu'un panneau glisse vers la hauteur de son contenu : une fois arrêté.)
        if ctx.input(|i| i.pointer.any_down()) || self.fit_busy {
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
            "vue.panneau_outils" => self.toggle_pane_group(Group::Left),
            "vue.panneau_fiche" => self.toggle_pane_group(Group::Right),
            "vue.panneau_bande" => self.toggle_pane_group(Group::Bottom),
            "vue.reajuster_panneaux" => self.refit_panes(),
            "vue.barre_panneaux" => self.toggle_panel_bar(),
            // Les panneaux de la barre des panneaux : même règle que son clic.
            a => match crate::barrepanneaux::pane_de_action(a) {
                Some(pane) => self.clic_panneau(pane),
                None => return false,
            },
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
            ui.checkbox(&mut self.layout.command_bar, "Barre de commandes (en haut)");
            ui.checkbox(&mut self.layout.tool_bar, "Barre d'outils (à gauche)");
            // La barre des panneaux est un réglage de l'application, pas de l'espace de travail.
            let key = self.keymap.label("vue.barre_panneaux");
            let text = if key.is_empty() { "Barre des panneaux (à droite)".to_string() } else { format!("Barre des panneaux (à droite) ({key})") };
            let resp = ui.checkbox(&mut self.settings.panel_bar, text).on_hover_text("Une icône par panneau de côté, sur le bord droit : un clic l'ouvre, le met devant ou le ferme.");
            if resp.changed() {
                let _ = self.settings.save();
            }
            ui.separator();
            let panes = self.dock_layout();
            let detached = self.detached_panes();
            // Un par un…
            for pane in Pane::ALL.into_iter().filter(|p| *p != Pane::Canvas) {
                let mut on = panes.contains(pane);
                let apart = if detached.iter().any(|d| d.panneau == pane) { " (fenêtre à part)" } else { "" };
                if ui.checkbox(&mut on, format!("{}  {}{apart}", pane.icon(), pane.name())).changed() {
                    self.toggle_pane(pane);
                }
            }
            ui.separator();
            // … ou par groupe (actions du fichier de raccourcis).
            let groups = [
                ("Colonne des réglages (et fiche)", "vue.panneau_outils", Group::Left),
                ("Calques", "vue.panneau_fiche", Group::Right),
                ("Montage et timeline", "vue.panneau_bande", Group::Bottom),
            ];
            for (label, action, group) in groups {
                let mut on = group.panes().iter().any(|p| panes.contains(*p));
                let key = self.keymap.label(action);
                let text = if key.is_empty() { label.to_string() } else { format!("{label} ({key})") };
                if ui.checkbox(&mut on, text).changed() {
                    self.workspace_action(action);
                }
            }
            ui.separator();
            // Panneaux qui s'ajustent à leur contenu (`ajuste.rs`).
            let resp = ui.checkbox(&mut self.settings.panes_fit, "Ajuster les panneaux à leur contenu").on_hover_text(
                "Les réglages de l'outil, les guides, la fiche et les effets prennent la hauteur de ce \
                 qu'ils montrent ; la couleur, les pinceaux et les calques gagnent le reste. Un bord \
                 tiré à la main reste où on l'a mis.",
            );
            if resp.changed() {
                let _ = self.settings.save();
            }
            let fixed = self.settings.panes_fixed.len();
            let label = if fixed > 0 { format!("Réajuster les panneaux ({fixed} bord(s) tiré(s) à la main)") } else { "Réajuster les panneaux".to_string() };
            self.action_item(ui, &label, "vue.reajuster_panneaux");
            ui.separator();
            ui.label(
                egui::RichText::new(
                    "Glisse l'onglet d'un panneau : il s'aimante sur un bord ou dans un autre panneau, \
                     ou flotte si on le lâche ailleurs. Clic droit sur l'onglet : « Dans une fenêtre à \
                     part », pour le poser sur un autre écran.",
                )
                .small()
                .weak(),
            );
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

/// Deux dispositions identiques, aux parts près (elles bougent d'un rien d'une image à l'autre).
fn same_layout(a: &Layout, b: &Layout) -> bool {
    a.vignettes == b.vignettes && a.top_view == b.top_view && a.dock_layout().close_to(&b.dock_layout())
}
