//! Fenêtre « Nouveau projet » (Anthony, 2026-10-18), ouverte par l'accueil et par Fichier >
//! Nouveau (action `fichier.nouveau`) : nom et dossier, préréglage (vidéo, réseaux, papier, les
//! siens), format (largeur, hauteur, orientation, résolution, cadence), papier, aperçu. Entrée
//! valide, Échap annule. Les valeurs sont retenues dans les réglages (`nouveau.rs`).

use std::path::PathBuf;

use eframe::egui::{self, vec2, Align, Align2, Color32, Key, Layout, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::nouveau::{
    apply_builtin, apply_user, build_document, default_folder, delete_user_preset, detect, file_stem, ratio_name, save_user_preset, Applied, Category,
    Choice, NewProjectSettings, Unit, UserPreset, Values, MAX_DPI, MAX_PX, MIN_DPI, MIN_PX, PRESETS,
};
use crate::project;

/// État de la fenêtre pendant qu'elle est ouverte.
pub struct NewDocDialog {
    pub name: String,
    pub folder: PathBuf,
    pub values: Values,
    /// Champ « nom du préréglage » ouvert par la disquette.
    save_name: Option<String>,
    save_focus: bool,
    /// Remarque sous le format (papier dont la résolution a baissé).
    note: Option<String>,
    focused: bool,
    /// L'accueil était montré : on y revient en annulant.
    from_welcome: bool,
}

impl NewDocDialog {
    /// La fenêtre, au départ des dernières valeurs retenues.
    pub fn open(settings: &NewProjectSettings, from_welcome: bool) -> Self {
        let mut values = settings.values.clone();
        values.clamp();
        let folder = settings.folder.clone().filter(|f| f.is_dir()).unwrap_or_else(default_folder);
        NewDocDialog { name: String::new(), folder, values, save_name: None, save_focus: false, note: None, focused: false, from_welcome }
    }

    /// Le dossier et le fichier du projet à créer, si un nom est donné.
    fn target(&self) -> Option<(PathBuf, PathBuf)> {
        let stem = file_stem(&self.name);
        if stem.is_empty() {
            return None;
        }
        Some(project::target_for(&self.folder.join(format!("{stem}.pelure"))))
    }

    /// Pourquoi on ne peut pas créer là (un projet y est déjà), s'il y a lieu.
    fn problem(&self) -> Option<String> {
        let (_, manifest) = self.target()?;
        project::check_target(&manifest, None).err().map(|e| format!("Impossible ici : {e}"))
    }
}

/// Choix fait dans la liste des préréglages.
enum Pick {
    Builtin(usize),
    User(usize),
}

impl App {
    /// Ouvre la fenêtre « Nouveau projet ».
    pub(crate) fn open_new_project(&mut self) {
        let from_welcome = self.welcome;
        self.welcome = false;
        self.new_dialog = Some(NewDocDialog::open(&self.settings.new_project, from_welcome));
    }

    /// La fenêtre « Nouveau projet », si elle est ouverte.
    pub(crate) fn new_project_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.new_dialog.take() else { return };
        // Avant d'afficher : un champ de texte déjà en saisie garde Entrée pour lui.
        let editing = ctx.wants_keyboard_input();
        let popup = ctx.memory(|m| m.any_popup_open());
        let (enter, escape) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let user = self.settings.new_project.presets.clone();
        let problem = d.problem();
        let mut ok = enter && !editing && d.save_name.is_none();
        let mut cancel = false;
        let mut pick: Option<Pick> = None;
        let mut save: Option<UserPreset> = None;
        let mut delete: Option<String> = None;
        let mut choose_folder = false;
        egui::Window::new("Nouveau projet").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(540.0);
            // ----------------------------------------------------------------- nom et dossier
            egui::Grid::new("grille_nouveau_nom").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Nom");
                let resp = egui::TextEdit::singleline(&mut d.name).hint_text("Mon film (facultatif)").desired_width(420.0).show(ui).response;
                if !d.focused {
                    resp.request_focus();
                    d.focused = true;
                }
                if resp.lost_focus() && enter {
                    ok = true;
                }
                ui.end_row();
                ui.label("Dossier");
                ui.horizontal(|ui| {
                    let shown = d.folder.display().to_string();
                    ui.allocate_ui_with_layout(vec2(388.0, 20.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.add(egui::Label::new(shown.as_str()).truncate()).on_hover_text(shown);
                    });
                    if crate::bulles::icone(ui, ph::FOLDER_OPEN, "Choisir le dossier", "Le dossier où créer le projet : il y aura un dossier à son nom.", "").clicked() {
                        choose_folder = true;
                    }
                });
                ui.end_row();
            });
            let where_ = match d.target() {
                Some((_, manifest)) => format!("Sera créé : {}", manifest.display()),
                None => "Sans nom : « Enregistrer » demandera où ranger le projet.".to_string(),
            };
            ui.label(RichText::new(where_).weak().small());
            ui.separator();

            // ----------------------------------------------------------------- préréglage et unité
            let choice = detect(&d.values, &user);
            ui.horizontal(|ui| {
                ui.label("Préréglage");
                let current = match &choice {
                    Choice::Builtin(i) => PRESETS[*i].name.to_string(),
                    Choice::User(i) => user[*i].name.clone(),
                    Choice::Custom => Choice::CUSTOM_LABEL.to_string(),
                };
                egui::ComboBox::from_id_salt("prereglage_nouveau_projet").selected_text(current).width(190.0).show_ui(ui, |ui| {
                    for cat in [Category::Video, Category::Social, Category::Paper] {
                        ui.label(RichText::new(cat.label()).weak().small());
                        for (i, p) in PRESETS.iter().enumerate().filter(|(_, p)| p.category == cat) {
                            let text = format!("{}   {}", p.name, p.detail());
                            if ui.selectable_label(choice == Choice::Builtin(i), text).clicked() {
                                pick = Some(Pick::Builtin(i));
                            }
                        }
                    }
                    if !user.is_empty() {
                        ui.label(RichText::new(Category::User.label()).weak().small());
                        for (i, p) in user.iter().enumerate() {
                            let text = format!("{}   {}×{} · {} i/s", p.name, p.width, p.height, p.fps);
                            if ui.selectable_label(choice == Choice::User(i), text).clicked() {
                                pick = Some(Pick::User(i));
                            }
                        }
                    }
                });
                if crate::bulles::icone(ui, ph::FLOPPY_DISK, "Enregistrer le préréglage", "Garde la taille, la cadence et la résolution actuelles sous un nom, pour les prochains projets.", "").clicked() {
                    d.save_name = Some(String::new());
                    d.save_focus = false;
                }
                let can_delete = matches!(choice, Choice::User(_));
                if crate::bulles::icone_si(ui, can_delete, ph::TRASH, "Supprimer le préréglage", "Retire ce préréglage de la liste. Seuls les tiens se suppriment, pas ceux d'origine.", "").clicked() {
                    if let Choice::User(i) = choice {
                        delete = Some(user[i].name.clone());
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for u in Unit::ALL.into_iter().rev() {
                        ui.selectable_value(&mut d.values.unit, u, u.label()).on_hover_text("Unité des champs largeur et hauteur (le projet garde des pixels).");
                    }
                    ui.label("Unité");
                });
            });
            if let Some(name) = &mut d.save_name {
                let mut confirm = false;
                let mut close = false;
                ui.horizontal(|ui| {
                    ui.label("Nom du préréglage");
                    let resp = egui::TextEdit::singleline(name).hint_text("HD sombre, Pub verticale…").desired_width(220.0).show(ui).response;
                    if !d.save_focus {
                        resp.request_focus();
                        d.save_focus = true;
                    }
                    if resp.lost_focus() && enter && !name.trim().is_empty() {
                        confirm = true;
                    }
                    if crate::bulles::icone_si(ui, !name.trim().is_empty(), ph::CHECK, "Enregistrer", "Ajoute le préréglage à la liste (même nom : il est remplacé).", "Entrée").clicked() {
                        confirm = true;
                    }
                    if crate::bulles::icone(ui, ph::X, "Fermer", "Ne pas enregistrer de préréglage.", "Échap").clicked() {
                        close = true;
                    }
                });
                if confirm {
                    let v = &d.values;
                    save = Some(UserPreset { name: name.trim().to_string(), width: v.width, height: v.height, fps: v.fps, dpi: v.dpi });
                }
                if confirm || close {
                    d.save_name = None;
                }
            }
            ui.add_space(4.0);

            // ----------------------------------------------------------------- format et aperçu
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Format").strong());
                    let unit = d.values.unit;
                    let dpi = d.values.dpi;
                    let (lo, hi) = (unit.from_px(MIN_PX, dpi), unit.from_px(MAX_PX, dpi));
                    let before_paper = match &choice {
                        Choice::Builtin(i) if PRESETS[*i].is_paper() => Some(*i),
                        _ => None,
                    };
                    ui.horizontal(|ui| {
                        for (label, is_width) in [("Largeur", true), ("Hauteur", false)] {
                            ui.label(label);
                            let px = if is_width { d.values.width } else { d.values.height };
                            let mut value = unit.from_px(px, dpi);
                            let resp = ui.add(egui::DragValue::new(&mut value).speed(unit.speed()).range(lo..=hi).fixed_decimals(unit.decimals()).suffix(format!(" {}", unit.label())));
                            if resp.changed() {
                                let px = unit.to_px(value, dpi);
                                if is_width {
                                    d.values.width = px;
                                } else {
                                    d.values.height = px;
                                }
                                d.note = None;
                            }
                            if is_width && crate::bulles::icone(ui, ph::DEVICE_ROTATE, "Inverser l'orientation", "Échange la largeur et la hauteur (paysage ↔ portrait).", "").clicked() {
                                d.values.swap();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Proportions");
                        ui.label(RichText::new(ratio_name(d.values.width, d.values.height)).strong());
                        if unit != Unit::Px {
                            ui.label(RichText::new(format!("({}×{} px)", d.values.width, d.values.height)).weak());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Résolution");
                        let resp = ui.add(egui::DragValue::new(&mut d.values.dpi).range(MIN_DPI..=MAX_DPI).suffix(" dpi"));
                        crate::bulles::bulle(
                            resp.clone(),
                            "Résolution",
                            "Points par pouce : lit les formats papier et les unités mm, cm, in. Elle reste dans les réglages, pas dans le projet.",
                            "",
                        );
                        // Un format papier reste lui-même quand la résolution change.
                        if resp.changed() {
                            if let Some(i) = before_paper {
                                let a = apply_builtin(&mut d.values, i);
                                d.note = capped_note(i, &a);
                            }
                        }
                        ui.add_space(10.0);
                        ui.label("Cadence");
                        egui::ComboBox::from_id_salt("cadence_nouveau_projet").selected_text(format!("{} i/s", d.values.fps)).width(70.0).show_ui(ui, |ui| {
                            for f in crate::projetui::FPS_PRESETS {
                                ui.selectable_value(&mut d.values.fps, f, format!("{f} i/s"));
                            }
                        });
                        ui.add(egui::DragValue::new(&mut d.values.fps).range(1..=120).suffix(" i/s"));
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| preview(ui, &d.values));
            });
            if let Some(note) = &d.note {
                ui.label(RichText::new(note.as_str()).weak().small());
            }
            ui.separator();

            // ----------------------------------------------------------------- papier
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.values.paper_on, "Papier").on_hover_text("L'arrière-plan du projet, qui sort à l'export. Décoché : transparent.");
                ui.add_enabled_ui(d.values.paper_on, |ui| {
                    let resp = ui.color_edit_button_srgb(&mut d.values.paper);
                    crate::bulles::bulle(resp, "Couleur du papier", "La couleur de l'arrière-plan du projet.", "");
                });
                ui.add_space(18.0);
                ui.label("Fond sous le trait");
                let resp = ui.color_edit_button_srgb(&mut d.values.fond);
                crate::bulles::bulle(resp, "Couleur des fonds", "La couleur d'un fond qu'on allume sous le trait (option « Fond sous le trait » du pinceau).", "");
                ui.add_space(18.0);
                ui.label("Premier plan");
                let resp = ui.add(egui::DragValue::new(&mut d.values.first_seconds).speed(0.1).range(0.1..=600.0).max_decimals(1).suffix(" s"));
                crate::bulles::bulle(resp, "Durée du premier plan", "Combien de temps dure le premier plan du projet, en secondes.", "");
                ui.label(RichText::new(format!("{} im.", d.values.first_frames())).weak());
            });
            ui.add_space(6.0);

            // ----------------------------------------------------------------- boutons
            ui.horizontal(|ui| {
                let button = egui::Button::new(RichText::new("OK").strong()).min_size(vec2(70.0, 0.0));
                if ui.add_enabled(problem.is_none(), button).on_hover_text("Créer le projet (Entrée).").clicked() {
                    ok = true;
                }
                if ui.add(egui::Button::new("Annuler").min_size(vec2(70.0, 0.0))).on_hover_text("Fermer sans rien créer (Échap).").clicked() {
                    cancel = true;
                }
                if let Some(p) = &problem {
                    ui.colored_label(crate::theme::erreur(), p.as_str());
                }
            });
        });

        if choose_folder {
            if let Some(dir) = rfd::FileDialog::new().set_title("Dossier où créer le projet").set_directory(&d.folder).pick_folder() {
                d.folder = dir;
            }
        }
        match pick {
            Some(Pick::Builtin(i)) => {
                let a = apply_builtin(&mut d.values, i);
                d.note = capped_note(i, &a);
            }
            Some(Pick::User(i)) => {
                apply_user(&mut d.values, &user[i]);
                d.note = None;
            }
            None => {}
        }
        if let Some(p) = save {
            let name = p.name.clone();
            if save_user_preset(&mut self.settings.new_project.presets, p) {
                self.save_settings_quietly();
                self.status = format!("Préréglage « {name} » enregistré.");
            }
        }
        if let Some(name) = delete {
            if delete_user_preset(&mut self.settings.new_project.presets, &name) {
                self.save_settings_quietly();
                self.status = format!("Préréglage « {name} » supprimé.");
            }
        }
        if escape && !popup {
            if d.save_name.is_some() {
                d.save_name = None;
            } else {
                cancel = true;
            }
        }
        if cancel {
            if d.from_welcome {
                self.welcome = true;
            }
            return;
        }
        if ok && problem.is_none() {
            self.create_project(&d);
            return;
        }
        self.new_dialog = Some(d);
    }

    /// Crée le projet de la fenêtre : le remplace, retient les valeurs, et l'enregistre tout de suite
    /// dans son dossier quand un nom est donné.
    fn create_project(&mut self, d: &NewDocDialog) {
        let mut values = d.values.clone();
        values.clamp();
        let doc = build_document(&values, &d.name);
        if !self.charger_projet(doc) {
            self.status = "Le projet n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.".into();
            return;
        }
        self.welcome = false;
        self.settings.new_project.values = values.clone();
        self.settings.new_project.folder = Some(d.folder.clone());
        self.save_settings_quietly();
        let frame = format!("cadre {}×{} à {} i/s, feuille de dessin {}×{}", values.width, values.height, values.fps, values.width * 2, values.height * 2);
        match d.target() {
            Some((_, manifest)) => {
                if self.save_to_blocking(manifest) {
                    let status = std::mem::take(&mut self.status);
                    self.status = format!("Nouveau projet : {frame}. {status}");
                } else {
                    // Le projet existe en mémoire : quitter proposera de l'enregistrer.
                    self.modified = true;
                }
            }
            None => self.status = format!("Nouveau projet : {frame}. « Enregistrer » demandera où le ranger."),
        }
    }

    fn save_settings_quietly(&mut self) {
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
    }
}

/// « A3 : résolution réduite à 247 dpi… » quand le papier a dû baisser la résolution.
fn capped_note(index: usize, a: &Applied) -> Option<String> {
    a.capped.then(|| format!("{} : résolution réduite à {} dpi pour tenir dans {MAX_PX} px.", PRESETS[index].name, a.dpi))
}

/// Le cadre en petit, à ses proportions, de la couleur du papier (damier s'il est transparent).
fn preview(ui: &mut egui::Ui, v: &Values) {
    let (card, _) = ui.allocate_exact_size(vec2(150.0, 112.0), Sense::hover());
    let painter = ui.painter_at(card);
    painter.rect_filled(card, 4.0, ui.visuals().extreme_bg_color);
    painter.rect_stroke(card, 4.0, ui.visuals().widgets.noninteractive.bg_stroke, StrokeKind::Inside);
    let inner = card.shrink(10.0);
    let scale = (inner.width() / v.width as f32).min(inner.height() / v.height as f32);
    let frame = Rect::from_center_size(card.center(), vec2(v.width as f32, v.height as f32) * scale);
    if v.paper_on {
        painter.rect_filled(frame, 0.0, Color32::from_rgb(v.paper[0], v.paper[1], v.paper[2]));
    } else {
        let step = 8.0;
        let (mut y, mut row) = (frame.top(), 0);
        while y < frame.bottom() {
            let (mut x, mut col) = (frame.left(), 0);
            while x < frame.right() {
                let cell = Rect::from_min_max(egui::pos2(x, y), egui::pos2((x + step).min(frame.right()), (y + step).min(frame.bottom())));
                painter.rect_filled(cell, 0.0, if (row + col) % 2 == 0 { Color32::from_gray(200) } else { Color32::from_gray(150) });
                x += step;
                col += 1;
            }
            y += step;
            row += 1;
        }
    }
    painter.rect_stroke(frame, 0.0, Stroke::new(1.0_f32, ui.visuals().text_color()), StrokeKind::Outside);
}
