//! Aide > À propos de FEANOR… (`aide.a_propos`) : une petite fenêtre modale avec le lockup de la marque (le mot, le Joyau
//! jade devant le mot, la devise « Forge your story » alignée sur le R), la version et les polices de marque. Échap, un
//! clic à côté ou « Fermer » la ferment.
//!
//! La date de fabrication n'y est pas : la compilation ne l'écrit nulle part sans coûter une
//! recompilation à chaque fois.

use eframe::egui::{self, pos2, vec2, Align, Layout, RichText, Sense};

use crate::app::App;
use crate::logo::Joyau;
use crate::marque::{self, Mise};

/// Hauteur des capitales du mot dans la fenêtre, en points (le Joyau fait alors plus de 32 points).
const CAPITALES: f32 = 40.0;
/// Largeur de la fenêtre, en points.
const LARGEUR: f32 = 320.0;

/// « Version 0.1.0 », d'après Cargo.toml.
pub(crate) fn version_text() -> String {
    format!("Version {}", env!("CARGO_PKG_VERSION"))
}

/// La ligne sur les polices de marque (licence SIL OFL 1.1, fichiers dans `assets/polices/`).
pub(crate) fn polices_text() -> &'static str {
    "Polices : Big Shoulders Display, IBM Plex (licence SIL OFL 1.1)"
}

impl App {
    /// La fenêtre « À propos de FEANOR ».
    pub(crate) fn about_window_ui(&mut self, ctx: &egui::Context) {
        if !self.about_window {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("a_propos_feanor")).show(ctx, |ui| {
            ui.set_width(LARGEUR);
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                let p = crate::theme::palette();
                // (Le lockup rétrécit plutôt que d'être coupé.)
                let capitales = marque::ajuster(ctx, ui.available_width() - 8.0, CAPITALES, Mise::Devant);
                let mesure = marque::disposer(ctx, pos2(0.0, 0.0), capitales, true, Mise::Devant).rect;
                let (zone, _) = ui.allocate_exact_size(vec2(mesure.width(), mesure.height()), Sense::hover());
                let texture = marque::texture_joyau(ctx, "apropos", marque::cote_joyau(capitales), Joyau::Jade, None);
                let origine = pos2(zone.left() - mesure.min.x, zone.top() - mesure.min.y);
                marque::peindre(ui.painter(), ctx, &texture, origine, capitales, p.texte_fort, Some(p.texte_doux), Mise::Devant);
                ui.add_space(14.0);
                ui.label(RichText::new(version_text()).strong());
                ui.label(RichText::new(polices_text()).small().weak());
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