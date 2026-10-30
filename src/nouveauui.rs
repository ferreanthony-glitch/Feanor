//! Le formulaire « Nouveau projet » (Anthony, 2026-10-18 ; refait le 2026-10-29, `docs/INTERFACE.md` partie 74).
//!
//! Il y a **un seul formulaire** (`NewDocDialog::formulaire`), posé à deux endroits :
//! - dans le cadre « Nouveau projet » de la fenêtre d'accueil (`accueil.rs`), quand aucun projet n'est ouvert ;
//! - dans la fenêtre « Nouveau projet » (Fichier > Nouveau, Ctrl+N) quand un projet est déjà ouvert.
//!
//! Champs : nom du projet (avec « Créer »), dossier (liste des derniers dossiers, « Parcourir… »), titre et sous-titre
//! (facultatifs), préréglage de format, largeur et hauteur, cadence ; puis, sous « Plus d'options », l'unité, la résolution,
//! le papier, la couleur des aplats, la durée du premier plan et un aperçu du cadre. Les valeurs sont retenues dans les
//! réglages (`nouveau.rs`).

use std::path::PathBuf;

use eframe::egui::{self, vec2, Align, Color32, Key, Layout, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::nouveau::{
    apply_builtin, apply_user, build_document, default_folder, delete_user_preset, detect, dossiers_proposes, file_stem, ratio_name, save_user_preset,
    Applied, Category, Choice, NewProjectSettings, Unit, UserPreset, Values, MAX_DPI, MAX_PX, MIN_DPI, MIN_PX, PRESETS,
};
use crate::project;

/// État du formulaire pendant qu'il est ouvert (dans la fenêtre, ou sur l'accueil).
pub struct NewDocDialog {
    pub name: String,
    /// Le titre du projet (facultatif : à défaut, le nom).
    pub titre: String,
    /// Le sous-titre (facultatif) : il est gardé dans les Propriétés du projet, champ « Information ».
    pub sous_titre: String,
    pub folder: PathBuf,
    pub values: Values,
    /// Champ « nom du préréglage » ouvert par la disquette.
    save_name: Option<String>,
    save_focus: bool,
    /// Remarque sous le format (papier dont la résolution a baissé).
    note: Option<String>,
    focused: bool,
    /// Le prochain passage met le curseur dans le champ du nom (Ctrl+N sur l'accueil).
    pub focus_nom: bool,
}

/// Ce que le formulaire a besoin de savoir de son entourage.
pub(crate) struct Contexte<'a> {
    /// Les préréglages de l'utilisateur.
    pub prereglages: &'a [UserPreset],
    /// Les dossiers de la liste déroulante.
    pub dossiers: &'a [PathBuf],
    /// « Créer » attend un nom (l'accueil) ; sinon un projet sans nom se crée (« Enregistrer » demandera où le ranger).
    pub nom_requis: bool,
    /// « Plus d'options » est ouvert au départ (la fenêtre) ou fermé (l'accueil, qui manque de place).
    pub options_ouvertes: bool,
}

/// Ce que le formulaire demande à celui qui l'a posé.
#[derive(Default)]
pub(crate) struct Sortie {
    /// « Créer » (ou Entrée dans le nom) : à créer, si rien ne l'empêche.
    pub creer: bool,
    /// Un préréglage à ajouter à ceux de l'utilisateur.
    pub enregistrer: Option<UserPreset>,
    /// Un préréglage de l'utilisateur à retirer (par son nom).
    pub supprimer: Option<String>,
}

impl NewDocDialog {
    /// Le formulaire, au départ des dernières valeurs retenues.
    pub fn open(settings: &NewProjectSettings) -> Self {
        let mut values = settings.values.clone();
        values.clamp();
        let folder = settings.folder.clone().filter(|f| f.is_dir()).unwrap_or_else(default_folder);
        NewDocDialog {
            name: String::new(),
            titre: String::new(),
            sous_titre: String::new(),
            folder,
            values,
            save_name: None,
            save_focus: false,
            note: None,
            focused: false,
            focus_nom: false,
        }
    }

    /// Le dossier et le fichier du projet à créer, si un nom est donné.
    pub(crate) fn target(&self) -> Option<(PathBuf, PathBuf)> {
        let stem = file_stem(&self.name);
        if stem.is_empty() {
            return None;
        }
        Some(project::target_for(&self.folder.join(format!("{stem}.{}", project::EXTENSION))))
    }

    /// Pourquoi on ne peut pas créer là (un projet y est déjà), s'il y a lieu.
    pub(crate) fn problem(&self) -> Option<String> {
        let (_, manifest) = self.target()?;
        project::check_target(&manifest, None).err().map(|e| format!("Impossible ici : {e}"))
    }

    /// « Créer » peut-il partir ? Un nom si le contexte l'exige, et pas de projet déjà là.
    pub(crate) fn peut_creer(&self, nom_requis: bool) -> bool {
        (!nom_requis || self.target().is_some()) && self.problem().is_none()
    }

    /// **Le** formulaire : les champs, dans l'ordre de la demande d'Anthony. Dessiné dans `ui`, sur toute sa largeur.
    pub(crate) fn formulaire(&mut self, ui: &mut egui::Ui, cx: &Contexte) -> Sortie {
        let mut sortie = Sortie::default();
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        let user = cx.prereglages;
        let probleme = self.problem();
        let peut = (!cx.nom_requis || self.target().is_some()) && probleme.is_none();
        // (La colonne des étiquettes : 96 points ; le reste va aux champs, moins la place d'un bouton.)
        let champ = (ui.available_width() - 96.0 - 10.0 - 104.0).max(140.0);
        let mut choose_folder = false;
        let mut pick: Option<Pick> = None;

        egui::Grid::new("grille_nouveau_projet").num_columns(2).min_col_width(96.0).spacing([10.0, 7.0]).show(ui, |ui| {
            // ------------------------------------------------------------- nom et « Créer »
            ui.label("Nom du projet");
            ui.horizontal(|ui| {
                let resp = egui::TextEdit::singleline(&mut self.name).hint_text("Mon film").desired_width(champ).show(ui).response;
                // Le curseur va dans le nom au départ (et sur demande : Ctrl+N). La demande se répète jusqu'à ce qu'il y soit : la
                // toute première image d'une grille ne fait que mesurer, et ne garde pas le focus qu'on y demande.
                let ailleurs = ui.memory(|m| m.focused().is_some());
                if self.focus_nom || (!self.focused && !ailleurs) {
                    resp.request_focus();
                }
                if resp.has_focus() {
                    self.focused = true;
                    self.focus_nom = false;
                } else if ailleurs {
                    self.focused = true;
                }
                if resp.lost_focus() && enter {
                    sortie.creer = true;
                }
                let bouton = egui::Button::new(RichText::new("Créer").strong()).min_size(vec2(96.0, 0.0));
                let aide = if cx.nom_requis && self.target().is_none() { "Donne un nom au projet pour le créer." } else { "Crée le projet dans son dossier (Entrée dans le champ du nom)." };
                if crate::bulles::bulle(ui.add_enabled(peut, bouton), "Créer", aide, "Entrée").clicked() {
                    sortie.creer = true;
                }
            });
            ui.end_row();

            // ------------------------------------------------------------- dossier
            ui.label("Dossier");
            ui.horizontal(|ui| {
                let courant = self.folder.display().to_string();
                let resp = egui::ComboBox::from_id_salt("dossier_nouveau_projet").selected_text(courant.as_str()).width(champ).show_ui(ui, |ui| {
                    for d in cx.dossiers {
                        if ui.selectable_label(*d == self.folder, d.display().to_string()).clicked() {
                            self.folder = d.clone();
                        }
                    }
                });
                resp.response.on_hover_text(format!("{courant}\nLe dossier où créer le projet : il y aura un dossier à son nom."));
                if ui.add(egui::Button::new("Parcourir…").min_size(vec2(96.0, 0.0))).on_hover_text("Choisir un autre dossier.").clicked() {
                    choose_folder = true;
                }
            });
            ui.end_row();

            // ------------------------------------------------------------- titre et sous-titre
            ui.label("Titre");
            egui::TextEdit::singleline(&mut self.titre).hint_text("Facultatif : le nom, par défaut").desired_width(champ + 106.0).show(ui).response.on_hover_text("Le titre du projet (Propriétés du projet, fiches et exports). Vide : le nom.");
            ui.end_row();
            ui.label("Sous-titre");
            egui::TextEdit::singleline(&mut self.sous_titre)
                .hint_text("Facultatif")
                .desired_width(champ + 106.0)
                .show(ui)
                .response
                .on_hover_text("Gardé dans les Propriétés du projet, au champ « Information ».");
            ui.end_row();

            // ------------------------------------------------------------- format
            let choice = detect(&self.values, user);
            ui.label("Format");
            ui.horizontal(|ui| {
                let current = match &choice {
                    Choice::Builtin(i) => format!("{}   {}", PRESETS[*i].name, PRESETS[*i].detail()),
                    Choice::User(i) => format!("{}   {}×{}", user[*i].name, user[*i].width, user[*i].height),
                    Choice::Custom => format!("{}   {}×{}", Choice::CUSTOM_LABEL, self.values.width, self.values.height),
                };
                egui::ComboBox::from_id_salt("prereglage_nouveau_projet").selected_text(current).width(champ - 56.0).show_ui(ui, |ui| {
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
                    self.save_name = Some(String::new());
                    self.save_focus = false;
                }
                let can_delete = matches!(choice, Choice::User(_));
                if crate::bulles::icone_si(ui, can_delete, ph::TRASH, "Supprimer le préréglage", "Retire ce préréglage de la liste. Seuls les tiens se suppriment, pas ceux d'origine.", "").clicked() {
                    if let Choice::User(i) = choice {
                        sortie.supprimer = Some(user[i].name.clone());
                    }
                }
            });
            ui.end_row();

            // ------------------------------------------------------------- largeur et hauteur
            ui.label("Taille");
            ui.horizontal(|ui| {
                let unit = self.values.unit;
                let dpi = self.values.dpi;
                let (lo, hi) = (unit.from_px(MIN_PX, dpi), unit.from_px(MAX_PX, dpi));
                for (label, is_width) in [("Largeur", true), ("Hauteur", false)] {
                    ui.label(label);
                    let px = if is_width { self.values.width } else { self.values.height };
                    let mut value = unit.from_px(px, dpi);
                    let resp = ui.add(egui::DragValue::new(&mut value).speed(unit.speed()).range(lo..=hi).fixed_decimals(unit.decimals()).suffix(format!(" {}", unit.label())));
                    if resp.changed() {
                        let px = unit.to_px(value, dpi);
                        if is_width {
                            self.values.width = px;
                        } else {
                            self.values.height = px;
                        }
                        self.note = None;
                    }
                    if is_width && crate::bulles::icone(ui, ph::DEVICE_ROTATE, "Inverser l'orientation", "Échange la largeur et la hauteur (paysage ↔ portrait).", "").clicked() {
                        self.values.swap();
                    }
                }
                ui.label(RichText::new(ratio_name(self.values.width, self.values.height)).strong()).on_hover_text("Les proportions du cadre.");
            });
            ui.end_row();

            // ------------------------------------------------------------- cadence
            ui.label("Cadence");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("cadence_nouveau_projet").selected_text(format!("{} i/s", self.values.fps)).width(70.0).show_ui(ui, |ui| {
                    for f in crate::projetui::FPS_PRESETS {
                        ui.selectable_value(&mut self.values.fps, f, format!("{f} i/s"));
                    }
                });
                ui.add(egui::DragValue::new(&mut self.values.fps).range(1..=120).suffix(" i/s")).on_hover_text("Images par seconde.");
            });
            ui.end_row();
        });

        // Le préréglage choisi dans la liste (hors de la grille : il change les valeurs que la grille lisait).
        match pick {
            Some(Pick::Builtin(i)) => {
                let a = apply_builtin(&mut self.values, i);
                self.note = capped_note(i, &a);
            }
            Some(Pick::User(i)) => {
                apply_user(&mut self.values, &user[i]);
                self.note = None;
            }
            None => {}
        }
        if let Some(note) = &self.note {
            ui.label(RichText::new(note.as_str()).weak().small());
        }

        // ----------------------------------------------------------------- « Enregistrer le préréglage »
        if let Some(name) = &mut self.save_name {
            let mut confirm = false;
            let mut close = false;
            ui.horizontal(|ui| {
                ui.label("Nom du préréglage");
                let resp = egui::TextEdit::singleline(name).hint_text("HD sombre, Pub verticale…").desired_width(220.0).show(ui).response;
                if !self.save_focus {
                    resp.request_focus();
                    self.save_focus = true;
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
                let v = &self.values;
                sortie.enregistrer = Some(UserPreset { name: name.trim().to_string(), width: v.width, height: v.height, fps: v.fps, dpi: v.dpi });
            }
            if confirm || close {
                self.save_name = None;
            }
            // (Entrée a servi à valider le nom du préréglage : elle ne crée pas le projet.)
            if confirm {
                sortie.creer = false;
            }
        }

        // ----------------------------------------------------------------- plus d'options
        egui::CollapsingHeader::new("Plus d'options").id_salt("options_nouveau_projet").default_open(cx.options_ouvertes).show(ui, |ui| {
            let choice = detect(&self.values, user);
            let before_paper = match &choice {
                Choice::Builtin(i) if PRESETS[*i].is_paper() => Some(*i),
                _ => None,
            };
            ui.horizontal(|ui| {
                ui.label("Unité");
                for u in Unit::ALL {
                    ui.selectable_value(&mut self.values.unit, u, u.label()).on_hover_text("Unité des champs largeur et hauteur (le projet garde des pixels).");
                }
                ui.add_space(14.0);
                ui.label("Résolution");
                let resp = ui.add(egui::DragValue::new(&mut self.values.dpi).range(MIN_DPI..=MAX_DPI).suffix(" dpi"));
                crate::bulles::bulle(
                    resp.clone(),
                    "Résolution",
                    "Points par pouce : lit les formats papier et les unités mm, cm, in. Elle reste dans les réglages, pas dans le projet.",
                    "",
                );
                // Un format papier reste lui-même quand la résolution change.
                if resp.changed() {
                    if let Some(i) = before_paper {
                        let a = apply_builtin(&mut self.values, i);
                        self.note = capped_note(i, &a);
                    }
                }
            });
            if self.values.unit != Unit::Px {
                ui.label(RichText::new(format!("({}×{} px)", self.values.width, self.values.height)).weak());
            }
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.values.paper_on, "Papier").on_hover_text("L'arrière-plan du projet, qui sort à l'export. Décoché : transparent.");
                ui.add_enabled_ui(self.values.paper_on, |ui| {
                    let resp = crate::champcouleur::srgb(ui, &mut self.values.paper);
                    crate::bulles::bulle(resp, "Couleur du papier", "La couleur de l'arrière-plan du projet.", "");
                });
                ui.add_space(14.0);
                ui.label("Aplat sous le trait");
                let resp = crate::champcouleur::srgb(ui, &mut self.values.fond);
                crate::bulles::bulle(resp, "Couleur des aplats", "La couleur d'un aplat qu'on ajoute à un calque (bouton « Aplat sous le trait » de sa ligne).", "");
            });
            ui.horizontal(|ui| {
                ui.label("Premier plan");
                let resp = ui.add(egui::DragValue::new(&mut self.values.first_seconds).speed(0.1).range(0.1..=600.0).max_decimals(1).suffix(" s"));
                crate::bulles::bulle(resp, "Durée du premier plan", "Combien de temps dure le premier plan du projet, en secondes.", "");
                ui.label(RichText::new(format!("{} im.", self.values.first_frames())).weak());
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| preview(ui, &self.values));
            });
        });

        // ----------------------------------------------------------------- où, et pourquoi pas
        let where_ = match self.target() {
            Some((_, manifest)) => format!("Sera créé : {}", manifest.display()),
            None if cx.nom_requis => "Le projet sera un dossier à son nom, dans ce dossier.".to_string(),
            None => "Sans nom : « Enregistrer » demandera où ranger le projet.".to_string(),
        };
        ui.add(egui::Label::new(RichText::new(where_).weak().small()).truncate());
        if let Some(p) = &probleme {
            ui.colored_label(crate::theme::erreur(), p.as_str());
        }

        if choose_folder {
            if let Some(dir) = rfd::FileDialog::new().set_title("Dossier où créer le projet").set_directory(&self.folder).pick_folder() {
                self.folder = dir;
            }
        }
        sortie
    }
}

/// Choix fait dans la liste des préréglages.
enum Pick {
    Builtin(usize),
    User(usize),
}

impl App {
    /// Ouvre la fenêtre « Nouveau projet » (un projet est déjà ouvert). Sans projet ouvert, le formulaire est celui de l'accueil :
    /// on y met le curseur dans le nom (`accueil.rs`).
    pub(crate) fn open_new_project(&mut self) {
        if self.accueil_visible() {
            self.accueil_etat.focus_nom = true;
            if let Some(f) = &mut self.accueil_etat.form {
                f.focus_nom = true;
            }
            return;
        }
        self.new_dialog = Some(NewDocDialog::open(&self.settings.new_project));
    }

    /// Les dossiers de la liste déroulante : le choisi, ceux des projets récents, Documents.
    pub(crate) fn dossiers_du_formulaire(&self, choisi: &std::path::Path) -> Vec<PathBuf> {
        dossiers_proposes(choisi, &self.settings.recent_projects)
    }

    /// Les préréglages ajoutés ou retirés dans le formulaire, retenus dans les réglages.
    pub(crate) fn appliquer_prereglages(&mut self, s: &mut Sortie) {
        if let Some(p) = s.enregistrer.take() {
            let name = p.name.clone();
            if save_user_preset(&mut self.settings.new_project.presets, p) {
                self.save_settings_quietly();
                self.status = format!("Préréglage « {name} » enregistré.");
            }
        }
        if let Some(name) = s.supprimer.take() {
            if delete_user_preset(&mut self.settings.new_project.presets, &name) {
                self.save_settings_quietly();
                self.status = format!("Préréglage « {name} » supprimé.");
            }
        }
    }

    /// La fenêtre « Nouveau projet », si elle est ouverte.
    pub(crate) fn new_project_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.new_dialog.take() else { return };
        // Avant d'afficher : un champ de texte déjà en saisie garde Entrée pour lui.
        let editing = ctx.wants_keyboard_input();
        let popup = ctx.memory(|m| m.any_popup_open());
        let (enter, escape) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let user = self.settings.new_project.presets.clone();
        let dossiers = self.dossiers_du_formulaire(&d.folder);
        let mut ok = enter && !editing && d.save_name.is_none();
        let mut cancel = false;
        let mut sortie = Sortie::default();
        egui::Window::new("Nouveau projet").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(600.0);
            let cx = Contexte { prereglages: &user, dossiers: &dossiers, nom_requis: false, options_ouvertes: true };
            sortie = d.formulaire(ui, &cx);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("Annuler").min_size(vec2(70.0, 0.0))).on_hover_text("Fermer sans rien créer (Échap).").clicked() {
                    cancel = true;
                }
            });
        });
        ok |= sortie.creer;
        self.appliquer_prereglages(&mut sortie);
        if escape && !popup {
            if d.save_name.is_some() {
                d.save_name = None;
            } else {
                cancel = true;
            }
        }
        if cancel {
            return;
        }
        if ok && d.peut_creer(false) {
            self.create_project(&d);
            return;
        }
        self.new_dialog = Some(d);
    }

    /// Crée le projet du formulaire : le remplace, retient les valeurs, et l'enregistre tout de suite dans son dossier quand
    /// un nom est donné. Vrai si le projet est ouvert.
    pub(crate) fn create_project(&mut self, d: &NewDocDialog) -> bool {
        let mut values = d.values.clone();
        values.clamp();
        // (Le titre : celui qu'on a écrit, sinon le nom.)
        let titre = if d.titre.trim().is_empty() { d.name.as_str() } else { d.titre.as_str() };
        let mut doc = build_document(&values, titre);
        doc.properties.info = d.sous_titre.trim().to_string();
        self.numerotation_du_projet_neuf(&mut doc);
        if !self.charger_projet(doc) {
            self.status = "Le projet n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.".into();
            return false;
        }
        self.welcome = false;
        // Les lignes de texte sous les cases d'un projet neuf : celles de Préférences > Vignettes (aucune d'origine), 2026-10-30.
        self.layout.board_textes = self.settings.vignettes.textes_defaut;
        // Le projet neuf s'ouvre sur la page choisie dans les Préférences (les Vignettes, d'origine).
        self.set_espace(self.settings.new_project.ouvre_sur.espace());
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
        true
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
    let (card, _) = ui.allocate_exact_size(vec2(112.0, 84.0), Sense::hover());
    let painter = ui.painter_at(card);
    painter.rect_filled(card, 4.0, ui.visuals().extreme_bg_color);
    painter.rect_stroke(card, 4.0, ui.visuals().widgets.noninteractive.bg_stroke, StrokeKind::Inside);
    let inner = card.shrink(8.0);
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
