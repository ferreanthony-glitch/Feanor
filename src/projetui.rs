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
        }
        self.doc.storyboard_locked
    }

    /// Refuse (avec un message) ce qui changerait les dessins, les expositions ou les calques
    /// des plans quand le storyboard est verrouillé.
    pub(crate) fn content_refuses(&mut self) -> bool {
        if self.doc.storyboard_locked {
            self.status = CONTENT_LOCKED.into();
        }
        self.doc.storyboard_locked
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
        let doc = doc.created_now();
        let n = doc.panels.len();
        self.load_doc(doc);
        // Pas encore enregistré : quitter le proposera.
        self.modified = true;
        let s = if n > 1 { "s" } else { "" };
        self.status = format!("Nouveau projet fait de {n} plan{s} (le son n'est pas repris). « Enregistrer sous » pour le garder.");
    }

    /// La fenêtre « Propriétés du projet », si elle est ouverte.
    pub(crate) fn properties_window_ui(&mut self, ctx: &egui::Context) {
        if !self.properties_window {
            return;
        }
        let mut open = true;
        egui::Window::new("Propriétés du projet")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_pos(ctx.screen_rect().center() - vec2(230.0, 220.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| self.properties_ui(ui));
        if !open {
            self.properties_window = false;
        }
    }

    /// Un champ des propriétés (une étape d'annulation par geste) ; la nouvelle valeur s'il a changé.
    fn property_field(&mut self, ui: &mut egui::Ui, value: &str, lines: usize) -> Option<String> {
        let mut text = value.to_owned();
        let edit = if lines > 1 { egui::TextEdit::multiline(&mut text).desired_rows(lines) } else { egui::TextEdit::singleline(&mut text) };
        let resp = ui.add(edit.desired_width(300.0));
        self.edited(&resp).then_some(text)
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
        });
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
