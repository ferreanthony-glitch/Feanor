//! Choisir ses raccourcis dans Pelure (passe interface, 2026-10-16). Clic droit sur une
//! commande (icône ou entrée de menu) : « Assigner un raccourci… ». On tape la combinaison ;
//! Pelure dit si elle sert déjà, et l'écrit dans le fichier de raccourcis personnel (qui
//! remplace, action par action, le fichier par défaut). La fenêtre « Raccourcis clavier » liste
//! toutes les commandes, avec une recherche, et les mêmes boutons.

use eframe::egui::{self, vec2, Align2, Event, Id, Response, RichText};
use egui_phosphor::regular as ph;

use crate::app::{App, Mode};
use crate::keymap::{self, Keymap};

/// Une assignation en cours : l'action, et la combinaison tapée (pas encore).
pub(crate) struct Assign {
    action: String,
    captured: Option<String>,
}

/// Clé où une commande dessinée sans accès à l'application demande l'assignation.
const ASK: &str = "pelure_raccourci_demande";

/// Demande d'ouvrir l'assignation d'un raccourci à `action` (lue à l'image suivante).
pub(crate) fn ask_assign(ctx: &egui::Context, action: &str) {
    ctx.data_mut(|d| d.insert_temp(Id::new(ASK), action.to_string()));
}

/// Menu du clic droit d'une commande liée à `action` : son raccourci, et l'assigner.
pub(crate) fn shortcut_menu(resp: &Response, action: &str, current: &str) {
    if action.is_empty() {
        return;
    }
    resp.context_menu(|ui| {
        let now = if current.is_empty() { "aucun raccourci".to_string() } else { format!("raccourci : {current}") };
        ui.label(RichText::new(now).weak());
        if ui.button(format!("{}  Assigner un raccourci…", ph::KEYBOARD)).clicked() {
            ask_assign(ui.ctx(), action);
            ui.close_menu();
        }
    });
}

fn label_of(action: &str) -> String {
    keymap::action_info(action).map_or(action.to_string(), |i| i.label.to_string())
}

impl App {
    /// Ouvre l'assignation d'un raccourci à `action`.
    pub(crate) fn start_assign(&mut self, action: &str) {
        self.assign = Some(Assign { action: action.to_string(), captured: None });
    }

    /// Pendant une assignation : la première combinaison tapée (lue avant les raccourcis, qui
    /// ne jouent pas). Vrai si une assignation attend une touche.
    pub(crate) fn capture_shortcut(&mut self, ctx: &egui::Context) -> bool {
        let Some(a) = self.assign.as_mut() else { return false };
        if a.captured.is_some() {
            return false;
        }
        let mut keys: Vec<(egui::Key, Option<egui::Key>, egui::Modifiers)> = std::mem::take(&mut self.pending_keys);
        ctx.input(|i| {
            for e in &i.events {
                if let Event::Key { key, physical_key, pressed: true, repeat: false, modifiers } = e {
                    keys.push((*key, *physical_key, *modifiers));
                }
            }
        });
        for (k, p, m) in keys {
            if k == egui::Key::Escape {
                self.assign = None;
                return true;
            }
            if let Some(text) = keymap::combo_text(k, p, m) {
                a.captured = Some(text);
                break;
            }
        }
        true
    }

    /// Écrit des changements dans le fichier de raccourcis personnel, puis le relit.
    pub(crate) fn apply_shortcut_changes(&mut self, changes: &[(String, Option<Vec<String>>)]) -> bool {
        let path = match self.keymap.user_file.clone().or_else(Keymap::appdata_path) {
            Some(p) => p,
            None => {
                self.status = "Dossier %APPDATA% introuvable : raccourci non enregistré.".into();
                return false;
            }
        };
        let current = std::fs::read_to_string(&path).ok();
        let text = match self.keymap.edited_user_file(current.as_deref(), changes) {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("Raccourci non enregistré : {e}");
                return false;
            }
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, text) {
            self.status = format!("Impossible d'écrire {} : {e}", path.display());
            return false;
        }
        self.reload_keymap();
        true
    }

    /// Les fenêtres des raccourcis : l'assignation en cours, et la liste.
    pub(crate) fn shortcut_windows(&mut self, ctx: &egui::Context) {
        if let Some(action) = ctx.data_mut(|d| d.remove_temp::<String>(Id::new(ASK))) {
            self.start_assign(&action);
        }
        self.assign_window(ctx);
        self.shortcuts_list_window(ctx);
    }

    fn assign_window(&mut self, ctx: &egui::Context) {
        let Some(a) = self.assign.as_ref() else { return };
        let (action, captured) = (a.action.clone(), a.captured.clone());
        let label = label_of(&action);
        let current = self.keymap.labels_of(&action);
        let warn = crate::theme::avertissement();
        let mut open = true;
        let mut choice: Option<&str> = None;
        egui::Window::new("Assigner un raccourci")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.label(RichText::new(&label).strong());
                let now = if current.is_empty() { "Aucun raccourci.".to_string() } else { format!("Raccourci actuel : {}", current.join("  ou  ")) };
                ui.label(RichText::new(now).weak());
                ui.add_space(6.0);
                match &captured {
                    None => {
                        ui.label(RichText::new(format!("{}  Appuie sur la combinaison voulue…", ph::KEYBOARD)).size(15.0));
                        ui.label(RichText::new("Échap : fermer sans rien changer.").weak());
                    }
                    Some(text) => {
                        let shown = keymap::parse_combo(text).ok().and_then(|c| c.first().map(|c| c.label())).unwrap_or_else(|| text.clone());
                        ui.label(RichText::new(shown).monospace().size(18.0).strong());
                        let others = self.keymap.conflicts(text, &action);
                        if others.is_empty() {
                            ui.label(RichText::new("Libre.").weak());
                        } else {
                            let names: Vec<String> = others.iter().map(|o| format!("« {} »", label_of(o))).collect();
                            ui.colored_label(warn, format!("Déjà pris par {} : il le perdra.", names.join(", ")));
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Remplacer").on_hover_text("Ce raccourci devient le seul de la commande.").clicked() {
                                choice = Some("remplacer");
                            }
                            if !current.is_empty() && ui.button("Ajouter").on_hover_text("Garder aussi le raccourci actuel.").clicked() {
                                choice = Some("ajouter");
                            }
                            if ui.button("Autre touche").clicked() {
                                choice = Some("autre");
                            }
                        });
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if !current.is_empty() && ui.button("Retirer le raccourci").clicked() {
                        choice = Some("retirer");
                    }
                    if self.keymap.is_user(&action) && ui.button("Raccourci par défaut").clicked() {
                        choice = Some("defaut");
                    }
                });
            });
        let done = match (choice, captured) {
            (Some("remplacer"), Some(t)) => {
                let changes = self.keymap.assign_changes(&action, &t, false);
                self.apply_shortcut_changes(&changes)
            }
            (Some("ajouter"), Some(t)) => {
                let changes = self.keymap.assign_changes(&action, &t, true);
                self.apply_shortcut_changes(&changes)
            }
            (Some("autre"), _) => {
                if let Some(a) = self.assign.as_mut() {
                    a.captured = None;
                }
                false
            }
            (Some("retirer"), _) => self.apply_shortcut_changes(&[(action.clone(), Some(Vec::new()))]),
            (Some("defaut"), _) => self.apply_shortcut_changes(&[(action.clone(), None)]),
            _ => false,
        };
        if done {
            let now = self.keymap.labels_of(&action);
            self.status = if now.is_empty() {
                format!("« {label} » : plus de raccourci.")
            } else {
                format!("« {label} » : {}.", now.join(" ou "))
            };
        }
        if done || !open {
            self.assign = None;
        }
    }

    /// Fichier > Raccourcis clavier (et F1) : toutes les commandes, leurs raccourcis, et les
    /// boutons pour les changer.
    fn shortcuts_list_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut open = true;
        let mut edit = false;
        let mut reload = false;
        let mut assign: Option<String> = None;
        let mut changes: Vec<(String, Option<Vec<String>>)> = Vec::new();
        egui::Window::new("Raccourcis clavier")
            .open(&mut open)
            .resizable(true)
            .default_size([680.0, 580.0])
            .default_pos(ctx.screen_rect().center() - vec2(340.0, 290.0))
            .show(ctx, |ui| {
                let file = match &self.keymap.user_file {
                    Some(p) => format!("Fichier personnel : {}", p.display()),
                    None => "Raccourcis par défaut (pas encore de fichier personnel).".into(),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(file).small());
                    edit = crate::bulles::icone(ui, ph::PENCIL_SIMPLE, "Modifier à la main", "Ouvrir le fichier de raccourcis dans le Bloc-notes.", "").clicked();
                    reload = crate::bulles::icone(ui, ph::ARROW_CLOCKWISE, "Recharger", "Relire le fichier après l'avoir modifié.", "").clicked();
                });
                ui.label(
                    RichText::new("Astuce : un clic droit sur une icône ou une entrée de menu permet aussi de lui donner un raccourci.")
                        .small()
                        .weak(),
                );
                for w in &self.keymap.warnings {
                    ui.colored_label(crate::theme::avertissement(), format!("⚠ {w}"));
                }
                ui.horizontal(|ui| {
                    ui.label(ph::MAGNIFYING_GLASS);
                    ui.add(egui::TextEdit::singleline(&mut self.keys_filter).hint_text("Chercher une commande ou une touche").desired_width(260.0));
                    if !self.keys_filter.is_empty() && ui.small_button(ph::X).clicked() {
                        self.keys_filter.clear();
                    }
                });
                ui.separator();
                let filter = self.keys_filter.to_lowercase();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("grille_raccourcis").num_columns(3).striped(true).show(ui, |ui| {
                        for info in keymap::ACTIONS.iter().filter(|a| a.implemented) {
                            let keys = self.keymap.labels_of(info.id);
                            let mut label = info.label.to_string();
                            match self.keymap.context_of(info.id) {
                                Some(Mode::Storyboard) => label += " (storyboard)",
                                Some(Mode::Animation) => label += " (animation)",
                                None => {}
                            }
                            let keys_text = keys.join("  ou  ");
                            if !filter.is_empty() && !label.to_lowercase().contains(&filter) && !keys_text.to_lowercase().contains(&filter) {
                                continue;
                            }
                            let resp = ui.label(&label);
                            shortcut_menu(&resp, info.id, &keys_text);
                            if keys.is_empty() {
                                ui.label(RichText::new("—").weak());
                            } else {
                                let source = self.keymap.source_of(info.id);
                                let resp = ui.label(RichText::new(&keys_text).strong());
                                if !source.is_empty() {
                                    resp.on_hover_text(format!("Origine : {source}"));
                                }
                            }
                            ui.horizontal(|ui| {
                                if crate::bulles::icone(ui, ph::KEYBOARD, "Assigner", "Taper un nouveau raccourci pour cette commande.", "").clicked() {
                                    assign = Some(info.id.to_string());
                                }
                                if crate::bulles::icone_si(ui, !keys.is_empty(), ph::X, "Retirer", "Plus de raccourci pour cette commande.", "").clicked() {
                                    changes.push((info.id.to_string(), Some(Vec::new())));
                                }
                                if self.keymap.is_user(info.id)
                                    && crate::bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, "Par défaut", "Revenir au raccourci par défaut.", "").clicked()
                                {
                                    changes.push((info.id.to_string(), None));
                                }
                            });
                            ui.end_row();
                        }
                    });
                    let reserved: Vec<&keymap::Binding> = self.keymap.bindings.iter().filter(|b| b.reserved).collect();
                    if !reserved.is_empty() && filter.is_empty() {
                        ui.add_space(8.0);
                        ui.collapsing(format!("En réserve ({}) : raccourcis de commandes pas encore faites", reserved.len()), |ui| {
                            for b in reserved {
                                let keys = if b.keys.is_empty() { "—".to_string() } else { b.keys.join("  ou  ") };
                                ui.label(RichText::new(format!("{keys} : {}", label_of(&b.action))).weak());
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Souris et stylet").strong());
                    for (geste, effet) in crate::panels::MOUSE_GESTURES {
                        ui.label(format!("{geste} : {effet}"));
                    }
                });
            });
        self.show_help = open;
        if edit {
            self.edit_keymap();
        }
        if reload {
            self.reload_keymap();
        }
        if let Some(a) = assign {
            self.start_assign(&a);
        }
        if !changes.is_empty() {
            self.apply_shortcut_changes(&changes);
        }
    }
}
