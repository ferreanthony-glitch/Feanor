//! Écran d'accueil (Anthony, 2026-10-18, sa suggestion retenue : « un écran d'accueil (récents,
//! « Nouveau ») quand aucun projet n'est ouvert ») : au lancement sans projet, ou après « Fermer
//! le projet », une carte au milieu de la fenêtre propose deux grandes entrées, puis les projets récents. Elle s'en va
//! dès qu'un projet s'ouvre ou se crée, ou qu'on commence à dessiner.
//!
//! Passe « simplicité » du 2026-10-21, puis demande d'Anthony du 2026-10-28 : deux entrées, et rien d'autre de grand.
//! - **Nouveau projet** : la fenêtre de nouveau projet, avec le choix du format, de la cadence et du dossier (la même que
//!   Fichier > Nouveau projet…). Le projet créé s'ouvre sur la page Vignettes (la planche) ou, selon le réglage de
//!   Préférences > Général, sur la page Storyboard : voir `nouveau.rs`, `OuvreSur`.
//! - **Ouvrir un projet** : un projet SHOT ou un projet Storyboard Pro (la fenêtre d'ouverture accepte les deux).
//! Dessous : les projets récents. (« Importer un scénario », « Dessiner des vignettes » et « Dessiner un storyboard » ne sont
//! plus ici : l'import reste dans le menu Fichier > Importer.)
//!//! En tête de la carte : une image de storyboard tirée au hasard dans le dossier des images d'accueil, une fois par
//! apparition de l'accueil (`accueilimages.rs`) ; sans image, le logo de SHOT (« Frame the story », la devise est dans le
//! dessin), rasterisé une fois à la taille de l'écran (`logo.rs`) et gardé.
//!
//! Charte graphique (2026-10-28, `docs/INTERFACE.md`, partie 61) : « Nouveau projet » est le bouton principal, rempli de
//! l'accent (l'orange de SHOT) ; la carte prend le fond des fenêtres du thème. Ce fichier fabrique aussi le petit
//! logotype (« SHOT » et ses deux angles) tout à gauche de la barre de menus.

use eframe::egui::{self, vec2, Align2, FontId, RichText, Sense};
use egui_phosphor::regular as ph;

use crate::accueilimages::Visuel;
use crate::app::{App, Pending};
use crate::logo::{self, Variante, VUE_H, VUE_L};
use crate::theme;

/// Projets récents montrés sur l'accueil.
const RECENTS: usize = 8;

/// Largeur du logo sur la carte, en points.
const LOGO_LARGEUR: f32 = 400.0;

/// La largeur du logo pour une fenêtre de cette hauteur (en points) : la moitié de la hauteur, de 240 à `LOGO_LARGEUR`, par
/// dizaines de points (la texture ne se refait pas à chaque pixel quand on tire le bord de la fenêtre).
pub(crate) fn logo_largeur(hauteur: f32) -> f32 {
    ((hauteur * 0.5 / 10.0).floor() * 10.0).clamp(240.0, LOGO_LARGEUR)
}

/// La taille à l'écran d'une image de `[w, h]` pixels dans `place` : toute la place si elle est de ses proportions, sinon la plus
/// grande taille qui y tient, proportions gardées.
pub(crate) fn place_de_l_image(pixels: [usize; 2], place: egui::Vec2) -> egui::Vec2 {
    let (w, h) = (pixels[0].max(1) as f32, pixels[1].max(1) as f32);
    let echelle = (place.x / w).min(place.y / h);
    vec2(w * echelle, h * echelle)
}

/// Hauteur d'une grande entrée, en points.
pub(crate) const ENTREE_H: f32 = 58.0;

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

/// Les deux grandes entrées de l'accueil : (icône, titre, une ligne d'explication, commande du fichier de raccourcis).
pub(crate) const ENTREES: [(&str, &str, &str, &str); 2] = [
    (ph::FILE_PLUS, "Nouveau projet", "Choisis le format, la cadence et le dossier, puis commence.", "fichier.nouveau"),
    (ph::FOLDER_OPEN, "Ouvrir un projet", "Un projet SHOT ou Storyboard Pro.", "fichier.ouvrir"),
];
/// Une grande entrée : l'icône, le titre et sa ligne d'explication, sur toute la largeur. L'entrée `principale` (Nouveau
/// projet) est le bouton principal : fond d'accent, texte posé dessus (`sur_accent`) ; l'autre est un bouton ordinaire,
/// qui prend un filet d'accent au survol.
fn entree(ui: &mut egui::Ui, icone: &str, titre: &str, aide: &str, principale: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ENTREE_H), Sense::click());
    let p = theme::palette();
    let appui = resp.is_pointer_button_down_on();
    let survol = resp.hovered() || resp.has_focus();
    let fond = match (principale, appui, survol) {
        (true, true, _) => p.accent_actif,
        (true, false, true) => p.accent_survol,
        (true, false, false) => p.accent,
        (false, true, _) => p.controle_actif,
        (false, false, true) => p.controle_survol,
        (false, false, false) => p.controle,
    };
    let (couleur_icone, couleur_titre, couleur_aide) = if principale { (p.sur_accent, p.sur_accent, p.sur_accent) } else { (p.accent_texte, p.texte_fort, p.texte_doux) };
    let peintre = ui.painter();
    peintre.rect_filled(rect, 8.0, fond);
    if resp.hovered() && !principale {
        peintre.rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, p.accent_texte), egui::StrokeKind::Inside);
    }
    peintre.text(rect.left_center() + vec2(18.0, 0.0), Align2::LEFT_CENTER, icone, FontId::proportional(26.0), couleur_icone);
    peintre.text(rect.left_top() + vec2(60.0, 11.0), Align2::LEFT_TOP, titre, FontId::proportional(16.0), couleur_titre);
    peintre.text(rect.left_top() + vec2(60.0, 33.0), Align2::LEFT_TOP, aide, FontId::proportional(12.0), couleur_aide);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// La texture du petit logotype (le mot « SHOT » et ses deux angles, sans la devise) de la barre de menus, à la taille
/// exacte de l'écran (`hauteur` points × pixels par point). Les angles sont d'accent dans un thème de la charte ; les
/// lettres, de la couleur du texte (blanc cassé ou noir cinéma). Rendu une fois et gardé : refait seulement si la taille
/// en pixels, la variante ou la couleur des angles change.
pub(crate) fn logotype_texture(ctx: &egui::Context, hauteur: f32) -> (egui::TextureHandle, egui::Vec2) {
    let p = theme::palette();
    let pixels = (hauteur * ctx.pixels_per_point()).round().max(8.0) as u32;
    let angles = if p.identite { Some([p.accent_texte.r(), p.accent_texte.g(), p.accent_texte.b()]) } else { None };
    let cle = (pixels, !p.claire, angles);
    let id = egui::Id::new("shot_logotype");
    let taille = |w: u32, h: u32| vec2(w as f32 / ctx.pixels_per_point(), h as f32 / ctx.pixels_per_point());
    if let Some((c, texture, w, h)) = ctx.data(|d| d.get_temp::<((u32, bool, Option<[u8; 3]>), egui::TextureHandle, u32, u32)>(id)) {
        if c == cle {
            return (texture, taille(w, h));
        }
    }
    let variante = if p.claire { Variante::Clair } else { Variante::Sombre };
    let (w, h, rgba) = logo::logotype(pixels, variante, angles);
    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
    let texture = ctx.load_texture("shot-logotype", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (cle, texture.clone(), w, h)));
    (texture, taille(w, h))
}

/// Hauteur du petit logotype de la barre de menus, en points : celle de la barre (un peu moins, pour qu'il y respire).
pub(crate) const LOGOTYPE_HAUTEUR: f32 = 20.0;

/// Le petit logotype tout à gauche de la barre de menus : discret, sans bulle ni clic, suivi d'un petit espace.
pub(crate) fn logotype_barre(ui: &mut egui::Ui) {
    let (texture, taille) = logotype_texture(ui.ctx(), LOGOTYPE_HAUTEUR);
    ui.add(egui::Image::new(egui::load::SizedTexture::new(texture.id(), taille)).sense(Sense::hover()));
    ui.add_space(6.0);
}

impl App {
    /// L'accueil est-il à montrer ? Pas pendant qu'une fenêtre d'import (scénario, Storyboard Pro) ou de « Nouveau projet »
    /// est ouverte : elle est ce qu'on regarde, et l'accueil revient si on la ferme sans rien importer.
    pub(crate) fn welcome_shown(&self) -> bool {
        self.welcome && self.path.is_none() && !self.modified && self.new_dialog.is_none() && self.scenario_window.is_none() && self.sb_window.is_none()
    }

    /// La carte d'accueil, si aucun projet n'est ouvert.
    pub(crate) fn welcome_ui(&mut self, ctx: &egui::Context) {
        if !self.welcome_shown() {
            // L'accueil s'en va pour de bon (un projet est là) : la prochaine apparition tirera une autre image. (La
            // fenêtre « Nouveau projet », posée par-dessus, ne compte pas : annulée, l'accueil revient avec la même image.)
            if !(self.welcome || self.new_dialog.is_some()) || self.path.is_some() || self.modified {
                self.confort.accueil.oublier_le_tirage();
            }
            return;
        }
        let visuel = self.accueil_visuel(ctx);
        let mut action: Option<&str> = None;
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
                    // (La place rapetisse avec la hauteur de la fenêtre : à 150 %, la carte ne déborde pas sur les menus.)
                    let largeur = logo_largeur(ctx.screen_rect().height());
                    let place = vec2(largeur, largeur * VUE_H / VUE_L);
                    match visuel {
                        // L'image se range dans la place du logo, proportions gardées, centrée.
                        Visuel::Image(texture, [w, h]) => {
                            let taille = place_de_l_image([w, h], place);
                            ui.add_space((place.y - taille.y) / 2.0);
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(texture.id(), taille)).corner_radius(6.0));
                            ui.add_space((place.y - taille.y) / 2.0);
                        }
                        // (Pendant que le fil décode : la place du logo, vide.)
                        Visuel::Attente => {
                            ui.allocate_space(place);
                        }
                        Visuel::Logo => {
                            let texture = logo_texture(ctx, largeur);
                            ui.image(egui::load::SizedTexture::new(texture.id(), place));
                        }
                    }
                    ui.add_space(10.0);
                });
                for (k, (icone, titre, aide, a)) in ENTREES.into_iter().enumerate() {
                    // (La première entrée, Nouveau projet, est le bouton principal.)
                    if entree(ui, icone, titre, aide, k == 0).clicked() {
                        action = Some(a);
                    }
                    ui.add_space(4.0);
                }
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
