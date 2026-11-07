//! Vue Projet (13e-4) : ce que la vue Projet de TVPaint 12 offre et que Pelure n'avait pas
//! encore. On peut ajouter un plan avant le plan courant, verrouiller le storyboard, remplir les
//! propriétés du projet, et faire un nouveau projet des plans choisis.
//!
//! Le storyboard verrouillé refuse ce qui change l'ordre, le nombre ou la durée des plans, et
//! (choix d'Anthony, 2026-10-15) ce qui change leurs dessins, leurs expositions ou leurs
//! calques. Restent permis : l'affichage (visibilité, pelure, opacité), les repères, les notes,
//! les noms et les marques de couleur. Le mot de passe du verrou de TVPaint n'est pas repris.

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::App;

/// Message des opérations de montage refusées par le storyboard verrouillé.
pub(crate) const STORYBOARD_LOCKED: &str =
    "Le storyboard est verrouillé : l'ordre, le nombre et la durée des plans ne changent pas (Storyboard > Déverrouiller le storyboard).";

/// Message des dessins et commandes d'images refusés par le storyboard verrouillé.
pub(crate) const CONTENT_LOCKED: &str =
    "Le storyboard est verrouillé : on ne dessine plus, et les images et les calques des plans ne changent pas (Storyboard > Déverrouiller le storyboard).";

/// Onglet de la fenêtre « Réglages du projet ».
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    #[default]
    Settings,
    Resolution,
}

/// Taille de l'image et cadence en cours de réglage (onglet Résolution), appliquées d'un coup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolutionEdit {
    pub w: usize,
    pub h: usize,
    pub fps: u32,
    /// Largeur et hauteur liées (mêmes proportions).
    pub linked: bool,
    /// Mettre les dessins à l'échelle (sinon : le cadre seul change).
    pub scale: bool,
    /// Nouvelle cadence : garder les durées en secondes (sinon : les mêmes images).
    pub keep_seconds: bool,
    pub interpolate: bool,
}

/// Tailles d'image proposées (nouveau projet, réglages du projet).
pub(crate) const RESOLUTIONS: [(&str, usize, usize); 12] = [
    ("HD 1080", 1920, 1080),
    ("HD 720", 1280, 720),
    ("4K UHD", 3840, 2160),
    ("2K DCI", 2048, 1080),
    ("4K DCI", 4096, 2160),
    ("Cinéma 1,85:1", 1998, 1080),
    ("Scope 2,39:1", 1920, 804),
    ("4:3", 1440, 1080),
    ("Carré", 1080, 1080),
    ("Portrait 4:5", 1080, 1350),
    ("Vertical 9:16", 1080, 1920),
    ("PAL 4:3", 768, 576),
];

/// Cadences proposées (entières : le projet ne connaît pas 23,976).
pub(crate) const FPS_PRESETS: [u32; 8] = [12, 15, 24, 25, 30, 48, 50, 60];

/// Proportions d'une image : « 16:9 » quand c'est simple, sinon « 2,39:1 ».
pub fn ratio_label(w: usize, h: usize) -> String {
    fn gcd(a: usize, b: usize) -> usize {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }
    let g = gcd(w, h).max(1);
    let (a, b) = (w / g, h / g);
    if a <= 32 && b <= 32 {
        format!("{a}:{b}")
    } else {
        format!("{:.2}:1", w as f64 / h.max(1) as f64).replace('.', ",")
    }
}

/// Date du jour `days` compté depuis le 1er janvier 1970 : (année, mois, jour), calendrier
/// grégorien.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Date et heure d'un instant (secondes depuis 1970), en temps universel : « 15/10/2026, 14:05
/// UTC », ou « inconnue » pour 0 (projet d'avant le format 17).
pub fn date_label(secs: u64) -> String {
    if secs == 0 {
        return "inconnue".into();
    }
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let s = secs % 86_400;
    format!("{d:02}/{m:02}/{y}, {:02}:{:02} UTC", s / 3600, s % 3600 / 60)
}

impl App {
    /// Refuse (avec un message) ce qui changerait l'ordre, le nombre ou la durée des plans quand
    /// le storyboard est verrouillé.
    pub(crate) fn storyboard_refuses(&mut self) -> bool {
        if self.doc.storyboard_locked {
            self.status = STORYBOARD_LOCKED.into();
            return true;
        }
        // Film partagé : il faut le jeton du montage (`cooperationui.rs`).
        self.partage_refuse_montage()
    }

    /// Refuse (avec un message) ce qui changerait les dessins, les expositions ou les calques
    /// des plans quand le storyboard est verrouillé.
    pub(crate) fn content_refuses(&mut self) -> bool {
        // Tout le film est écarté : tous ces gestes écriraient dans le plan vide qui le tient (`planannule.rs`, partie 120). Ce qui ne le touche pas (la taille du
        // projet, les personnages) passe par `contenu_verrouille`.
        if self.doc.film_vide() {
            self.status = crate::planannuleui::REFUS_SUPPORT.into();
            return true;
        }
        self.contenu_verrouille()
    }

    /// Le verrou du storyboard et le film partagé, pour ce qui change le projet sans écrire dans le plan courant.
    pub(crate) fn contenu_verrouille(&mut self) -> bool {
        if self.doc.storyboard_locked {
            self.status = CONTENT_LOCKED.into();
            return true;
        }
        // Film partagé : le plan courant doit être à moi (`cooperationui.rs`).
        self.partage_refuse_plan()
    }

    /// Verrouille ou déverrouille le storyboard (une étape d'annulation).
    pub(crate) fn toggle_storyboard_lock(&mut self) {
        self.end_stroke();
        self.montage_drag = None;
        self.record_step();
        self.doc.storyboard_locked = !self.doc.storyboard_locked;
        self.status = if self.doc.storyboard_locked {
            "Storyboard verrouillé : les plans, leurs dessins et leurs calques ne changent plus.".into()
        } else {
            "Storyboard déverrouillé.".into()
        };
    }

    /// Nouveau plan juste avant le plan courant, dans sa séquence ; il devient le plan courant.
    pub(crate) fn new_panel_before(&mut self) {
        if self.structure_op(|d, i| Some(d.add_scene_before(i))) {
            let title = crate::storyboard::panel_title(&self.doc, self.cur_panel).to_lowercase();
            self.status = format!("Nouveau {title}, avant l'ancien plan courant.");
        }
    }

    /// Remplace le projet par un projet neuf fait des plans choisis (Fichier ; demande d'abord
    /// d'enregistrer le projet en cours s'il a changé).
    pub(crate) fn new_project_from_chosen(&mut self) {
        self.end_stroke();
        let ids = self.board_selection();
        let Some(doc) = self.doc.from_plans(&ids) else {
            self.status = "Aucun plan choisi.".into();
            return;
        };
        let mut doc = doc.created_now();
        self.numerotation_du_projet_neuf(&mut doc);
        let n = doc.panels.len();
        if !self.charger_projet(doc) {
            self.status = "Le projet n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.".into();
            return;
        }
        // Pas encore enregistré : quitter le proposera.
        self.modified = true;
        let s = if n > 1 { "s" } else { "" };
        self.status = format!("Nouveau projet fait de {n} plan{s} (le son n'est pas repris). « Enregistrer sous » pour le garder.");
    }

    /// La fenêtre « Réglages du projet » (Fichier), si elle est ouverte : un onglet Réglages (les
    /// propriétés de 13e) et un onglet Résolution (taille de l'image, cadence).
    pub(crate) fn properties_window_ui(&mut self, ctx: &egui::Context) {
        if !self.properties_window {
            self.res_edit = None;
            return;
        }
        let mut open = true;
        egui::Window::new("Réglages du projet")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_pos(ctx.screen_rect().center() - vec2(240.0, 260.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let tab = self.settings_tab;
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Settings, crate::bulles::encre_etat(RichText::new("Réglages").strong(), tab == SettingsTab::Settings));
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Resolution, crate::bulles::encre_etat(RichText::new("Résolution").strong(), tab == SettingsTab::Resolution));
                });
                ui.separator();
                match self.settings_tab {
                    SettingsTab::Settings => self.properties_ui(ui),
                    SettingsTab::Resolution => self.resolution_ui(ui),
                }
            });
        if !open {
            self.properties_window = false;
            self.res_edit = None;
        }
    }

    /// L'onglet Résolution : taille de l'image et cadence, appliquées ensemble.
    fn resolution_ui(&mut self, ui: &mut egui::Ui) {
        let (frame, fps) = (self.doc.frame, self.doc.fps);
        let current = ResolutionEdit { w: frame[0], h: frame[1], fps, linked: true, scale: true, keep_seconds: true, interpolate: false };
        let mut e = self.res_edit.unwrap_or(current);
        ui.set_width(440.0);
        ui.label(RichText::new("Préréglages").strong());
        egui::ScrollArea::vertical().max_height(160.0).id_salt("prereglages_resolution").show(ui, |ui| {
            for (name, w, h) in RESOLUTIONS {
                if ui.selectable_label(e.w == w && e.h == h, format!("{name}  —  {w}×{h}")).clicked() {
                    e.w = w;
                    e.h = h;
                }
            }
        });
        ui.add_space(4.0);
        let ratio = e.h as f32 / e.w.max(1) as f32;
        ui.horizontal(|ui| {
            ui.label("Largeur");
            if ui.add(egui::DragValue::new(&mut e.w).range(16..=4096).suffix(" px")).changed() && e.linked {
                e.h = ((e.w as f32 * ratio).round() as usize).clamp(16, 4096);
            }
            crate::bulles::interrupteur(ui, &mut e.linked, egui_phosphor::regular::LINK, "Lier", "Garder les proportions en changeant la largeur ou la hauteur.", "");
            ui.label("Hauteur");
            if ui.add(egui::DragValue::new(&mut e.h).range(16..=4096).suffix(" px")).changed() && e.linked {
                e.w = ((e.h as f32 / ratio.max(1e-6)).round() as usize).clamp(16, 4096);
            }
        });
        ui.label(RichText::new(format!("Proportions {}  ·  actuellement {}×{} ({})", ratio_label(e.w, e.h), frame[0], frame[1], ratio_label(frame[0], frame[1]))).weak());
        let resized = [e.w, e.h] != frame;
        if resized {
            ui.add_space(4.0);
            ui.radio_value(&mut e.scale, true, "Mettre les dessins à l'échelle");
            let mut text = "Dessins (hors de la feuille compris), caméra, guides, placement des calques et effets en pixels suivent la nouvelle taille.".to_string();
            if e.w * frame[1] != e.h * frame[0] {
                text += " Autres proportions : le dessin remplit le nouveau cadre sans être déformé ; ce qui dépasse sort du cadre.";
            }
            ui.label(RichText::new(text).weak().small());
            ui.radio_value(&mut e.scale, false, "Garder les dessins à leur taille");
            ui.label(RichText::new("Seul le cadre change, centré : il montre plus ou moins de la feuille.").weak().small());
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Cadence");
            egui::ComboBox::from_id_salt("cadence_prereglee").selected_text(format!("{} i/s", e.fps)).width(80.0).show_ui(ui, |ui| {
                for f in FPS_PRESETS {
                    ui.selectable_value(&mut e.fps, f, format!("{f} i/s"));
                }
            });
            ui.add(egui::DragValue::new(&mut e.fps).range(1..=120).suffix(" i/s"));
        });
        if e.fps != fps {
            ui.radio_value(&mut e.keep_seconds, true, "Garder les durées en secondes");
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.add_enabled(e.keep_seconds, egui::Checkbox::new(&mut e.interpolate, "Interpoler les images ajoutées (sinon : dessins tenus)"));
            });
            ui.radio_value(&mut e.keep_seconds, false, "Garder les images (le film va plus vite ou plus lentement)");
        }
        if resized {
            ui.add_space(4.0);
            crate::theme::avertir(ui, "Changer la taille ne s'annule pas (Ctrl+Z) : l'historique d'annulation est vidé.");
        }
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui.add_enabled(resized || e.fps != fps, egui::Button::new("Appliquer")).clicked();
            if ui.button("Remettre").on_hover_text("Revenir aux valeurs du projet.").clicked() {
                e = current;
            }
        });
        if apply {
            self.res_edit = None;
            self.apply_resolution(e);
        } else {
            self.res_edit = Some(e);
        }
    }

    /// Applique la cadence puis la taille réglées dans l'onglet Résolution.
    pub(crate) fn apply_resolution(&mut self, e: ResolutionEdit) {
        let mut msgs = Vec::new();
        if e.fps != self.doc.fps {
            if e.keep_seconds {
                self.change_fps(e.fps, e.interpolate);
                msgs.push(self.status.clone());
            } else {
                self.end_stroke();
                self.record_step();
                self.doc.fps = e.fps;
                msgs.push(format!("Cadence {} i/s : mêmes images, le film va à une autre vitesse.", e.fps));
            }
        }
        if [e.w, e.h] != self.doc.frame {
            if self.contenu_verrouille() {
                return;
            }
            self.end_stroke();
            self.playing = false;
            self.doc.resize_frame(e.w, e.h, e.scale);
            self.after_stage_change();
            msgs.push(format!(
                "Image {}×{}{} (feuille {}×{}). L'historique d'annulation est vidé.",
                self.doc.frame[0],
                self.doc.frame[1],
                if e.scale { ", dessins mis à l'échelle" } else { ", dessins gardés à leur taille" },
                self.doc.width,
                self.doc.height
            ));
        }
        if !msgs.is_empty() {
            self.status = msgs.join(" ");
        }
    }

    /// Un champ des propriétés (une étape d'annulation par geste) ; la nouvelle valeur s'il a changé.
    fn property_field(&mut self, ui: &mut egui::Ui, value: &str, lines: usize) -> Option<String> {
        let mut text = value.to_owned();
        let edit = if lines > 1 { egui::TextEdit::multiline(&mut text).desired_rows(lines) } else { egui::TextEdit::singleline(&mut text) };
        let resp = ui.add(edit.desired_width(300.0));
        self.edited(&resp).then_some(text)
    }

    /// Un champ de durée cible (« 1:30 », « 90 » : des secondes) ; la nouvelle valeur en millisecondes s'il a changé (une étape d'annulation par geste).
    fn champ_cible(&mut self, ui: &mut egui::Ui, ms: u32) -> Option<u32> {
        let mut secondes = ms as f64 / 1000.0;
        let resp = ui.add(
            egui::DragValue::new(&mut secondes)
                .speed(1.0)
                .range(0.0..=crate::budget::MAX_MS as f64 / 1000.0)
                .custom_formatter(|n, _| crate::budget::texte_duree((n * 1000.0).round() as u32))
                .custom_parser(|s| crate::budget::lire_duree(s).map(|ms| ms as f64 / 1000.0)),
        );
        let resp = crate::bulles::bulle(resp, "Durée cible", "La durée voulue, en minutes:secondes (11:30) ou en secondes (690). Glisser pour l'ajuster, double-clic pour taper, 0 pour l'enlever.", "");
        self.edited(&resp).then(|| (secondes * 1000.0).round() as u32)
    }

    /// Une cible de durée par séquence, avec l'écart en direct.
    fn budget_sequences_section(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Durée cible par séquence").strong());
        let ordre = self.doc.sequence_order();
        if ordre.is_empty() {
            ui.label(RichText::new("Aucune séquence.").weak());
            return;
        }
        let budgets = crate::budget::budgets_des_sequences(&self.doc);
        egui::Grid::new("budget_sequences").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
            for seq in ordre {
                ui.label(format!("Séquence {}", self.doc.sequence_label(seq)));
                let courante = self.doc.properties.budgets_sequences.get(&seq).copied().unwrap_or(0);
                if let Some(v) = self.champ_cible(ui, courante) {
                    self.doc.poser_cible_de_sequence(seq, v);
                }
                match budgets.get(&seq) {
                    Some(b) if b.depasse() => {
                        ui.colored_label(ui.visuals().error_fg_color, format!("{} : trop long", b.texte(self.doc.fps)));
                    }
                    Some(b) => {
                        ui.label(RichText::new(b.texte(self.doc.fps)).weak());
                    }
                    None => {
                        ui.label(RichText::new(crate::budget::duree_courte(self.doc.panels.iter().zip(self.doc.sequence_ids()).filter(|(_, s)| *s == seq).map(|(p, _)| p.frame_count()).sum(), self.doc.fps)).weak());
                    }
                }
                ui.end_row();
            }
        });
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.doc.properties.clone();
        egui::Grid::new("proprietes_projet").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Titre");
            if let Some(v) = self.property_field(ui, &p.title, 1) {
                self.doc.properties.title = v;
            }
            ui.end_row();
            ui.label("Information");
            if let Some(v) = self.property_field(ui, &p.info, 2) {
                self.doc.properties.info = v;
            }
            ui.end_row();
            ui.label("Auteur");
            if let Some(v) = self.property_field(ui, &p.author, 1) {
                self.doc.properties.author = v;
            }
            ui.end_row();
            ui.label("Date de création");
            ui.label(date_label(p.created));
            ui.end_row();
            ui.label("Dernière modification");
            ui.label(date_label(p.modified)).on_hover_text("Posée à chaque enregistrement.");
            ui.end_row();
            ui.label("Notes");
            if let Some(v) = self.property_field(ui, &p.notes, 4) {
                self.doc.properties.notes = v;
            }
            ui.end_row();
            ui.label("Copyright");
            if let Some(v) = self.property_field(ui, &p.copyright, 1) {
                self.doc.properties.copyright = v;
            }
            ui.end_row();
            // Production : la série et l'épisode ({projet} et {episode} du modèle de noms, nommage.rs).
            ui.label("Série ou projet de production");
            if let Some(v) = self.property_field(ui, &p.serie, 1) {
                self.doc.properties.serie = v;
            }
            ui.end_row();
            ui.label("Épisode");
            if let Some(v) = self.property_field(ui, &p.episode, 1) {
                self.doc.properties.episode = v;
            }
            ui.end_row();
            // Budget de durée (`budget.rs`) : la cible du film, et l'écart en direct (rouge quand il dépasse).
            ui.label("Durée cible du film");
            ui.horizontal(|ui| {
                if let Some(v) = self.champ_cible(ui, p.budget_ms) {
                    self.doc.properties.budget_ms = v;
                }
                match self.doc.budget_du_film() {
                    Some(b) => {
                        let texte = b.texte(self.doc.fps);
                        if b.depasse() {
                            ui.colored_label(ui.visuals().error_fg_color, format!("{texte} : trop long"));
                        } else {
                            ui.label(RichText::new(texte).weak());
                        }
                    }
                    None => {
                        ui.label(RichText::new("aucune : tape 11:30 ou 690").weak());
                    }
                }
            });
            ui.end_row();
            ui.label("Noms des plans");
            ui.horizontal(|ui| {
                ui.label(self.doc.nom_de_plan(self.cur_panel.min(self.doc.panels.len().saturating_sub(1))));
                if ui.button("Modèle de noms…").on_hover_text("Le modèle de noms des plans pour tous les exports (SQ01_PL012A, ou celui du studio).").clicked() {
                    self.run_action("projet.noms_production");
                }
            });
            ui.end_row();
            ui.label("Numéro de la première image");
            let mut first = self.doc.start_frame;
            let resp = ui.add(egui::DragValue::new(&mut first).range(0..=100_000));
            if self.edited(&resp) {
                self.doc.start_frame = first;
            }
            ui.end_row();
            // Aplat sous le trait (`fond.rs`) : la couleur que prend un aplat qu'on ajoute.
            ui.label("Couleur des aplats sous le trait");
            ui.horizontal(|ui| {
                let mut c = self.doc.properties.fond.unwrap_or(crate::fond::FondReglages::default().couleur);
                let resp = crate::champcouleur::srgb(ui, &mut c);
                let resp = crate::bulles::bulle(resp, "Couleur des aplats", "La couleur d'un aplat qu'on ajoute à un calque (bouton « Aplat sous le trait » de sa ligne), par exemple un gris clair.", "");
                if self.edited(&resp) {
                    self.doc.properties.fond = Some(c);
                }
                let all = ui.button("Aux aplats déjà posés").on_hover_text("Donne aussi cette couleur à tous les aplats du projet.");
                if all.clicked() {
                    self.run_action("fond.couleur_partout");
                }
            });
            ui.end_row();
        });
        ui.separator();
        self.budget_sequences_section(ui);
        ui.separator();
        self.numbering_section(ui);
        ui.separator();
        let doc = &self.doc;
        let total: usize = doc.panels.iter().map(|p| p.frame_count()).sum();
        let plans = doc.panels.len();
        let where_ = match &self.path {
            Some(path) => path.display().to_string(),
            None => "pas encore enregistré".into(),
        };
        ui.label(
            RichText::new(format!(
                "Cadre {}×{}, {} i/s, {plans} plan{}, {}.\n{where_}",
                doc.frame[0],
                doc.frame[1],
                doc.fps,
                if plans > 1 { "s" } else { "" },
                crate::storyboard::duration_label(total, doc.fps)
            ))
            .weak(),
        );
    }
}
