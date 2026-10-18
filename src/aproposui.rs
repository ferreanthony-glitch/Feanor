//! Aide > À propos de SHOT… (`aide.a_propos`) : une petite fenêtre modale avec le logo (celui des
//! fonds sombres ou celui des fonds clairs, selon le thème ; la devise « Frame the story » est
//! dans le dessin), la version, et le nom d'avant. Échap, un clic à côté ou « Fermer » la ferment.
//!
//! La date de fabrication n'y est pas : la compilation ne l'écrit nulle part sans coûter une
//! recompilation à chaque fois.

use eframe::egui::{self, Align, Layout, RichText};

use crate::app::App;
use crate::logo::{VUE_H, VUE_L};

/// Largeur du logo dans la fenêtre, en points.
const LOGO_LARGEUR: f32 = 280.0;

/// Le nom d'avant le logiciel.
const ANCIEN_NOM: &str = "Pelure";

/// « Version 0.1.0 », d'après Cargo.toml.
pub(crate) fn version_text() -> String {
    format!("Version {}", env!("CARGO_PKG_VERSION"))
}

/// La ligne sur le nom d'avant.
pub(crate) fn ancien_nom_text() -> String {
    format!("Anciennement {ANCIEN_NOM}")
}

impl App {
    /// La fenêtre « À propos de SHOT ».
    pub(crate) fn about_window_ui(&mut self, ctx: &egui::Context) {
        if !self.about_window {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("a_propos_shot")).show(ctx, |ui| {
            ui.set_width(LOGO_LARGEUR + 40.0);
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                let texture = crate::accueil::logo_texture(ctx, LOGO_LARGEUR);
                ui.image(egui::load::SizedTexture::new(texture.id(), egui::vec2(LOGO_LARGEUR, LOGO_LARGEUR * VUE_H / VUE_L)));
                ui.add_space(10.0);
                ui.label(RichText::new(version_text()).strong());
                ui.label(RichText::new(ancien_nom_text()).weak());
                ui.add_space(10.0);
            });
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Fermer").clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.about_window = false;
        }
    }
}
