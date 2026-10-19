//! Écran d'accueil (Anthony, 2026-10-18, sa suggestion retenue : « un écran d'accueil (récents,
//! « Nouveau ») quand aucun projet n'est ouvert ») : au lancement sans projet, ou après « Fermer
//! le projet », une carte au milieu de la fenêtre propose un nouveau projet, d'en ouvrir un (SHOT
//! ou Storyboard Pro) et les projets récents. Elle s'en va dès qu'un projet s'ouvre ou se crée,
//! qu'on commence à dessiner, ou d'un clic sur « Commencer sans projet ».
//!
//! En tête de la carte : le logo de SHOT (« Frame the story », la devise est dans le dessin),
//! rasterisé une fois à la taille de l'écran (`logo.rs`) et gardé.

use eframe::egui::{self, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::{App, Pending};
use crate::logo::{self, Variante, VUE_H, VUE_L};

/// Projets récents montrés sur l'accueil.
const RECENTS: usize = 8;

/// Largeur du logo sur la carte, en points.
const LOGO_LARGEUR: f32 = 400.0;

/// Le logo à montrer est-il celui des fonds sombres ? Non quand le fond des panneaux est clair
/// (thème clair : `Palette::claire`, `Visuals::light`).
pub(crate) fn logo_sombre() -> bool {
    !crate::theme::palette().claire
}

/// La texture du logo, à la taille exacte de l'écran (points × pixels par point), dans la variante
/// qui va avec le thème. Refaite seulement si l'un des trois change.
pub(crate) fn logo_texture(ctx: &egui::Context, largeur: f32) -> egui::TextureHandle {
    let sombre = logo_sombre();
    let pixels = (largeur * ctx.pixels_per_point()).round().max(8.0) as u32;
    let id = egui::Id::new(("shot_logo", largeur as u32));
    if let Some((cle, texture)) = ctx.data(|d| d.get_temp::<((u32, bool), egui::TextureHandle)>(id)) {
        if cle == (pixels, sombre) {
            return texture;
        }
    }
    let variante = if sombre { Variante::Sombre } else { Variante::Clair };
    let (w, h, rgba) = logo::image(pixels, variante);
    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
    let texture = ctx.load_texture("shot-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, ((pixels, sombre), texture.clone())));
    texture
}

impl App {
    /// L'accueil est-il à montrer ?
    pub(crate) fn welcome_shown(&self) -> bool {
        self.welcome && self.path.is_none() && !self.modified && self.new_dialog.is_none()
    }

    /// La carte d'accueil, si aucun projet n'est ouvert.
    pub(crate) fn welcome_ui(&mut self, ctx: &egui::Context) {
        if !self.welcome_shown() {
            return;
        }
        let mut action: Option<&str> = None;
        let mut open: Option<std::path::PathBuf> = None;
        let mut dismiss = false;
        egui::Window::new("Accueil")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    let texture = logo_texture(ctx, LOGO_LARGEUR);
                    ui.image(egui::load::SizedTexture::new(texture.id(), egui::vec2(LOGO_LARGEUR, LOGO_LARGEUR * VUE_H / VUE_L)));
                    ui.add_space(10.0);
                });
                let big = |ui: &mut egui::Ui, icon: &str, text: &str| ui.add_sized([ui.available_width(), 34.0], egui::Button::new(RichText::new(format!("{icon}  {text}")).size(16.0)));
                if big(ui, ph::FILE_PLUS, "Nouveau projet…").clicked() {
                    action = Some("fichier.nouveau");
                }
                ui.add_space(2.0);
                if big(ui, ph::FOLDER_OPEN, "Ouvrir un projet…").clicked() {
                    action = Some("fichier.ouvrir");
                }
                ui.add_space(2.0);
                if big(ui, ph::FILE_ARROW_DOWN, "Importer un projet Storyboard Pro (.sboard)…").clicked() {
                    action = Some("fichier.importer_sboard");
                }
                ui.add_space(10.0);
                let recents: Vec<std::path::PathBuf> = self.settings.recent_projects.iter().take(RECENTS).cloned().collect();
                if !recents.is_empty() {
                    ui.label(RichText::new("Projets récents").strong());
                    self.recent_presence.refresh(ctx, &recents);
                    for path in recents {
                        let exists = self.recent_presence.exists(&path);
                        let resp = ui.add_enabled(exists, egui::Button::new(crate::recents::label(&path)).frame(false));
                        let resp = resp.on_hover_text(path.display().to_string()).on_disabled_hover_text(format!("Introuvable : {}", path.display()));
                        if resp.clicked() {
                            open = Some(path);
                        }
                    }
                    ui.add_space(6.0);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.settings.reopen_last, "Rouvrir le dernier projet au lancement").changed() {
                        let _ = self.settings.save();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Commencer sans projet").on_hover_text("Dessiner tout de suite ; « Enregistrer » demandera où ranger le projet.").clicked() {
                            dismiss = true;
                        }
                    });
                });
            });
        if dismiss {
            self.welcome = false;
        }
        if let Some(path) = open {
            self.welcome = false;
            self.recent_to_open = Some(path);
            self.request(Pending::OpenRecent);
        }
        if let Some(a) = action {
            self.run_action(a);
        }
    }
}
