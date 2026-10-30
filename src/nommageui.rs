//! La fenêtre « Noms et production » (action `projet.noms_production`, hub Exporter, Propriétés du projet) : la série, l'épisode et le modèle de
//! noms des plans (`nommage.rs`), avec les modèles prêts, les jetons à insérer en un clic et l'aperçu en direct (noms du premier et du dernier plan,
//! doublons, caractères interdits). Chaque changement est une étape d'annulation du projet.

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::nommage::{self, Jeton};
use crate::theme;

pub(crate) struct NomsWindow {
    /// Ce que montre l'aperçu, recalculé seulement quand le projet ou le modèle change.
    cache: Option<((u64, u64, String, String, String), nommage::Verification)>,
}

impl App {
    pub(crate) fn open_noms(&mut self) {
        self.end_stroke();
        self.noms_fenetre = Some(NomsWindow { cache: None });
    }

    pub(crate) fn noms_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.noms_fenetre.take() else { return };
        let mut ouverte = true;
        egui::Window::new("Noms et production")
            .open(&mut ouverte)
            .resizable(false)
            .collapsible(false)
            .default_width(520.0)
            .default_pos(ctx.screen_rect().center() - vec2(260.0, 220.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| self.noms_body(ui, &mut w));
        if ouverte {
            self.noms_fenetre = Some(w);
        }
    }

    fn noms_body(&mut self, ui: &mut egui::Ui, w: &mut NomsWindow) {
        let avert = theme::avertissement();
        ui.label(
            RichText::new("Le nom que prend chaque plan dans les fichiers : vidéos par plan, images, calques séparés, fiche, liste des plans, paquet de livraison. Le modèle est un réglage du projet ; d'origine, il donne les noms de toujours (SQ01_PL012A).")
                .small()
                .color(theme::texte_doux()),
        );
        ui.separator();
        egui::Grid::new("noms_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
            ui.label("Série ou projet");
            let mut t = self.doc.properties.serie.clone();
            let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(300.0).hint_text("(le titre du projet)"));
            bulles::bulle(r.clone(), "Série ou projet de production", "Le nom de la production, pour {projet} et la liste des plans. Vide : le titre du projet.", "");
            if self.edited(&r) {
                self.doc.properties.serie = t;
            }
            ui.end_row();

            ui.label("Épisode");
            let mut t = self.doc.properties.episode.clone();
            let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(100.0).hint_text("1"));
            bulles::bulle(r.clone(), "Épisode", "Le numéro de l'épisode, pour {episode} et la liste des plans (EP02 avec {episode:02}).", "");
            if self.edited(&r) {
                self.doc.properties.episode = t;
            }
            ui.end_row();

            ui.label("Modèle prêt");
            let courant = self.doc.properties.modele_noms.trim().to_string();
            let actuel = nommage::MODELES_PRETS.iter().find(|(_, m)| *m == courant || (courant.is_empty() && *m == nommage::MODELE_DEFAUT)).map_or("Personnalisé", |(n, _)| *n);
            let mut choisi: Option<&str> = None;
            egui::ComboBox::from_id_salt("noms_prets").selected_text(actuel).width(320.0).show_ui(ui, |ui| {
                for (nom, m) in nommage::MODELES_PRETS {
                    if ui.selectable_label(*nom == actuel, format!("{nom}   {m}")).clicked() {
                        choisi = Some(*m);
                    }
                }
            });
            if let Some(m) = choisi {
                self.doc.properties.modele_noms = if m == nommage::MODELE_DEFAUT { String::new() } else { m.to_string() };
                self.history_note_noms();
            }
            ui.end_row();

            ui.label("Modèle");
            let mut t = if self.doc.properties.modele_noms.trim().is_empty() { nommage::MODELE_DEFAUT.to_string() } else { self.doc.properties.modele_noms.clone() };
            let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(380.0).font(egui::TextStyle::Monospace));
            bulles::bulle(r.clone(), "Modèle de noms", "Des jetons entre accolades ({sequence:03} : au moins 3 chiffres) et du texte fixe. {{ et }} donnent une accolade.", "");
            if self.edited(&r) {
                self.doc.properties.modele_noms = if t.trim() == nommage::MODELE_DEFAUT { String::new() } else { t };
            }
            ui.end_row();

            ui.label("Jetons");
            ui.horizontal_wrapped(|ui| {
                for j in Jeton::TOUS {
                    let nom = format!("{{{}}}", j.nom());
                    let r = ui.small_button(&nom);
                    if bulles::bulle(r, &nom, j.aide(), "").clicked() {
                        let mut m = if self.doc.properties.modele_noms.trim().is_empty() { nommage::MODELE_DEFAUT.to_string() } else { self.doc.properties.modele_noms.clone() };
                        m.push_str(&nom);
                        self.doc.properties.modele_noms = m;
                        self.history_note_noms();
                    }
                }
            });
            ui.end_row();
        });
        ui.separator();

        let cle = (self.history.serial(), self.doc_epoch, self.doc.properties.modele_noms.clone(), self.doc.properties.serie.clone(), self.doc.properties.episode.clone());
        if w.cache.as_ref().map(|c| &c.0) != Some(&cle) {
            let v = nommage::verifier(&self.doc, &self.doc.properties.modele_noms);
            w.cache = Some((cle, v));
        }
        let v = w.cache.as_ref().map(|c| c.1.clone()).unwrap_or_default();
        match &v.erreur {
            Some(e) => {
                ui.colored_label(avert, format!("Le modèle ne se lit pas : {e}. Les exports gardent le modèle d'origine tant qu'il n'est pas corrigé."));
            }
            None => {
                ui.label(RichText::new("Aperçu").strong());
                for (k, ex) in v.exemples.iter().enumerate() {
                    ui.label(RichText::new(format!("{} : {ex}", if k == 0 { "premier plan" } else { "dernier plan" })).monospace());
                }
            }
        }
        for a in &v.avertissements {
            ui.label(RichText::new(format!("{}  {a}", ph::WARNING)).small().color(avert));
        }
        if !self.doc.numbering_locked {
            ui.label(RichText::new("La numérotation des plans est suivie : les noms changent si un plan s'ajoute ou se déplace. Verrouille-la (Propriétés du projet) pour un suivi de production.").small().color(avert));
        }
    }

    /// Un changement posé sans passer par un champ de texte (modèle prêt, jeton) : une étape d'annulation.
    fn history_note_noms(&mut self) {
        self.note_change(egui::Id::new("noms_modele"));
    }
}
