//! La fenêtre « Enregistrer comme modèle d'épisode » (action `projet.enregistrer_modele_episode`, hub Exporter) : un nom, ce que le modèle garde, la liste
//! des modèles enregistrés (qu'on peut supprimer). « Nouvel épisode depuis un modèle » est dans le formulaire Nouveau projet (`nouveauui.rs`).

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::modeleepisode;
use crate::theme;

pub(crate) struct ModeleWindow {
    nom: String,
    result: Option<Result<String, String>>,
    modeles: Vec<String>,
}

impl App {
    pub(crate) fn open_modele_episode(&mut self) {
        self.end_stroke();
        let nom = match self.doc.properties.serie.trim() {
            "" => String::new(),
            s => format!("{s} : modèle d'épisode"),
        };
        self.modele_episode = Some(ModeleWindow { nom, result: None, modeles: modeleepisode::lister() });
    }

    pub(crate) fn modele_episode_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.modele_episode.take() else { return };
        let mut ouverte = true;
        egui::Window::new("Enregistrer comme modèle d'épisode")
            .open(&mut ouverte)
            .resizable(false)
            .collapsible(false)
            .default_width(460.0)
            .default_pos(ctx.screen_rect().center() - vec2(230.0, 180.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| self.modele_episode_body(ui, &mut w));
        if ouverte {
            self.modele_episode = Some(w);
        }
    }

    fn modele_episode_body(&mut self, ui: &mut egui::Ui, w: &mut ModeleWindow) {
        ui.label(
            RichText::new("Garde les réglages de cet épisode pour créer les suivants en quelques clics : le format, la numérotation, la série et le modèle de noms, les calques du premier plan, les personnages et les réglages d'export. Aucun dessin n'est gardé.")
                .small()
                .color(theme::texte_doux()),
        );
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Nom");
            ui.add(egui::TextEdit::singleline(&mut w.nom).desired_width(300.0).hint_text("Brawlidays, épisode type"));
            let ok = !w.nom.trim().is_empty();
            if bulles::icone_si(ui, ok, ph::FLOPPY_DISK, "Enregistrer le modèle", "Garde les réglages de ce projet sous ce nom (même nom : le modèle est remplacé).", "").clicked() {
                let m = modeleepisode::depuis(&self.doc, &self.settings, &w.nom);
                w.result = Some(modeleepisode::enregistrer(&m).map(|p| format!("Modèle enregistré : {}", p.display())));
                w.modeles = modeleepisode::lister();
                if let Some(Ok(m)) | Some(Err(m)) = &w.result {
                    self.status = m.clone();
                }
            }
        });
        match &w.result {
            Some(Ok(m)) => {
                ui.label(RichText::new(m).small().color(theme::succes()));
            }
            Some(Err(e)) => {
                ui.colored_label(theme::avertissement(), e);
            }
            None => {}
        }
        ui.separator();
        ui.label(RichText::new("Modèles enregistrés").strong());
        if w.modeles.is_empty() {
            ui.label(RichText::new("Aucun pour l'instant.").small().weak());
        }
        let mut retirer: Option<String> = None;
        for n in &w.modeles {
            ui.horizontal(|ui| {
                ui.label(n);
                if bulles::icone(ui, ph::TRASH, "Supprimer le modèle", "Efface ce modèle d'épisode (les projets déjà créés avec lui ne changent pas).", "").clicked() {
                    retirer = Some(n.clone());
                }
            });
        }
        if let Some(n) = retirer {
            w.result = Some(modeleepisode::supprimer(&n).map(|_| format!("Modèle « {n} » supprimé.")));
            w.modeles = modeleepisode::lister();
        }
        ui.label(RichText::new("Pour créer un épisode : Fichier > Nouveau, puis « Modèle d'épisode ».").small().weak());
    }
}
