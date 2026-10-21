//! Écran d'accueil (Anthony, 2026-10-18, sa suggestion retenue : « un écran d'accueil (récents,
//! « Nouveau ») quand aucun projet n'est ouvert ») : au lancement sans projet, ou après « Fermer
//! le projet », une carte au milieu de la fenêtre propose trois grandes entrées, puis les projets récents. Elle s'en va
//! dès qu'un projet s'ouvre ou se crée, qu'on commence à dessiner, ou d'un clic sur « Dessiner des vignettes ».
//!
//! Passe « simplicité » du 2026-10-21 : trois entrées, et rien d'autre de grand.
//! - **Dessiner des vignettes** : un projet vide, tout de suite, dans l'onglet Vignettes (la planche : on dessine dans les
//!   Cases, ou on y dépose des images de l'Explorateur, `deposer.rs`). C'est l'ancien « Commencer sans projet », dans
//!   l'onglet où l'on dessine d'abord ; « Enregistrer » demandera où ranger le projet.
//! - **Importer un scénario** : Fountain, Final Draft, Word ou texte, la fenêtre d'import (`scenarioui.rs`) ; depuis
//!   l'accueil, elle part d'un nouveau projet.
//! - **Ouvrir un projet** : un projet SHOT ou un projet Storyboard Pro (la fenêtre d'ouverture accepte les deux).
//! Dessous : un petit « Nouveau projet… » (avec le choix du format), puis les projets récents.
//!
//! En tête de la carte : le logo de SHOT (« Frame the story », la devise est dans le dessin),
//! rasterisé une fois à la taille de l'écran (`logo.rs`) et gardé.

use eframe::egui::{self, vec2, Align2, FontId, RichText, Sense};
use egui_phosphor::regular as ph;

use crate::app::{App, Pending};
use crate::logo::{self, Variante, VUE_H, VUE_L};
use crate::theme;
use crate::workspace::Espace;

/// Projets récents montrés sur l'accueil.
const RECENTS: usize = 8;

/// Largeur du logo sur la carte, en points.
const LOGO_LARGEUR: f32 = 400.0;

/// La largeur du logo pour une fenêtre de cette hauteur (en points) : la moitié de la hauteur, de 240 à `LOGO_LARGEUR`, par
/// dizaines de points (la texture ne se refait pas à chaque pixel quand on tire le bord de la fenêtre).
pub(crate) fn logo_largeur(hauteur: f32) -> f32 {
    ((hauteur * 0.5 / 10.0).floor() * 10.0).clamp(240.0, LOGO_LARGEUR)
}

/// Hauteur d'une grande entrée, en points.
const ENTREE_H: f32 = 58.0;

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

/// Les trois grandes entrées de l'accueil : (icône, titre, une ligne d'explication, action ou rien pour « Dessiner des vignettes »).
pub(crate) const ENTREES: [(&str, &str, &str, &str); 3] = [
    (ph::SQUARES_FOUR, "Dessiner des vignettes", "Une planche vide, tout de suite : tu dessines dans les Cases.", ""),
    (ph::FILE_TEXT, "Importer un scénario", "Fountain, Final Draft, Word ou texte : SHOT en fait des Plans.", "fichier.importer_scenario"),
    (ph::FOLDER_OPEN, "Ouvrir un projet", "Un projet SHOT ou Storyboard Pro.", "fichier.ouvrir"),
];

/// Une grande entrée : l'icône, le titre et sa ligne d'explication, sur toute la largeur.
fn entree(ui: &mut egui::Ui, icone: &str, titre: &str, aide: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ENTREE_H), Sense::click());
    let fond = if resp.is_pointer_button_down_on() {
        theme::controle_actif()
    } else if resp.hovered() || resp.has_focus() {
        theme::controle_survol()
    } else {
        theme::controle()
    };
    let peintre = ui.painter();
    peintre.rect_filled(rect, 8.0, fond);
    if resp.hovered() {
        peintre.rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, theme::accent_texte()), egui::StrokeKind::Inside);
    }
    peintre.text(rect.left_center() + vec2(18.0, 0.0), Align2::LEFT_CENTER, icone, FontId::proportional(26.0), theme::accent_texte());
    peintre.text(rect.left_top() + vec2(60.0, 11.0), Align2::LEFT_TOP, titre, FontId::proportional(16.0), theme::texte_fort());
    peintre.text(rect.left_top() + vec2(60.0, 33.0), Align2::LEFT_TOP, aide, FontId::proportional(12.0), theme::texte_doux());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl App {
    /// L'accueil est-il à montrer ? Pas pendant qu'une fenêtre d'import (scénario, Storyboard Pro) ou de « Nouveau projet »
    /// est ouverte : elle est ce qu'on regarde, et l'accueil revient si on la ferme sans rien importer.
    pub(crate) fn welcome_shown(&self) -> bool {
        self.welcome && self.path.is_none() && !self.modified && self.new_dialog.is_none() && self.scenario_window.is_none() && self.sb_window.is_none()
    }

    /// « Dessiner des vignettes » : un projet vide, dans l'onglet Vignettes, l'accueil parti.
    pub(crate) fn commencer_par_les_vignettes(&mut self) {
        self.welcome = false;
        self.set_espace(Espace::Vignettes);
    }

    /// La carte d'accueil, si aucun projet n'est ouvert.
    pub(crate) fn welcome_ui(&mut self, ctx: &egui::Context) {
        if !self.welcome_shown() {
            return;
        }
        let mut action: Option<&str> = None;
        let mut vignettes = false;
        let mut open: Option<std::path::PathBuf> = None;
        egui::Window::new("Accueil")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    // (Le logo rapetisse avec la hauteur de la fenêtre : à 150 %, la carte ne déborde pas sur les menus.)
                    let largeur = logo_largeur(ctx.screen_rect().height());
                    let texture = logo_texture(ctx, largeur);
                    ui.image(egui::load::SizedTexture::new(texture.id(), egui::vec2(largeur, largeur * VUE_H / VUE_L)));
                    ui.add_space(10.0);
                });
                for (icone, titre, aide, a) in ENTREES {
                    if entree(ui, icone, titre, aide).clicked() {
                        if a.is_empty() {
                            vignettes = true;
                        } else {
                            action = Some(a);
                        }
                    }
                    ui.add_space(4.0);
                }
                ui.horizontal(|ui| {
                    let nouveau = ui.add(egui::Button::new(RichText::new(format!("{}  Nouveau projet…", ph::FILE_PLUS)).color(theme::texte_doux())).frame(false));
                    if crate::bulles::bulle(nouveau, "Nouveau projet", "Choisir le format, la cadence et le dossier du projet avant de commencer.", &self.raccourci("fichier.nouveau")).clicked() {
                        action = Some("fichier.nouveau");
                    }
                });
                ui.add_space(8.0);
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
                if ui.checkbox(&mut self.settings.reopen_last, "Rouvrir le dernier projet au lancement").changed() {
                    let _ = self.settings.save();
                }
            });
        if vignettes {
            self.commencer_par_les_vignettes();
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
