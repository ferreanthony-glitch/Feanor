//! La palette de mélange (2026-10-28) : une petite zone de peinture, dans le panneau Couleur, repliable comme « Plus de
//! couleurs ». On y peint avec la brosse courante, en mélange (la couleur posée entraîne et se mêle à celle sur laquelle elle
//! passe) pour essayer et fabriquer une couleur ; Alt + clic, ou l'outil Pipette, y prend une couleur ; un bouton la vide.
//!
//! Elle appartient au poste, pas au projet (`Confort::palette`) : elle est gardée en PNG dans le dossier des réglages
//! (`%APPDATA%\SHOT\palette_melange.png`), pas dans `reglages.toml`, et chargée à la première ouverture du volet.
//!
//! L'outil Mélangeur lui-même (la brosse courante en « Lisser ») est dans `app.rs` (`Tool::Mixer`,
//! `BrushSettings::as_mixer`).

use std::path::{Path, PathBuf};

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::brush::{BrushSettings, MixKind, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushTextures;
use crate::raster::Raster;

/// Taille de la palette, en pixels de peinture (elle s'affiche à la largeur du panneau).
pub(crate) const LARGEUR: usize = 320;
pub(crate) const HAUTEUR: usize = 180;
/// Son fichier, dans le dossier des réglages.
const FICHIER: &str = "palette_melange.png";

/// Le fichier de la palette : `%APPDATA%\SHOT\palette_melange.png` (dans un dossier temporaire, pour un essai).
pub(crate) fn chemin() -> Option<PathBuf> {
    crate::donnees::roaming().map(|d| d.join(FICHIER))
}

/// La brosse de la palette : la brosse courante, en mélange. Une brosse qui ne mêle rien (ou la gomme) prend un mélange de
/// pigment moyen : la palette sert à mêler. Sa taille reste dans ce qui tient dans la zone.
pub(crate) fn reglage_de_palette(mut s: BrushSettings) -> BrushSettings {
    s.size = s.size.clamp(4.0, 40.0);
    s.smooth_deferred = false;
    if !s.mixes() {
        s.mix_kind = MixKind::Pigment;
        s.mix_off = false;
        s.mix = 0.5;
        s.charge = 1.0;
    }
    s
}

/// La zone de peinture et le trait en cours.
pub(crate) struct PaletteMelange {
    pub raster: Raster,
    stroke: Option<StrokeState>,
    /// Compte les changements : la texture affichée se refait quand il change.
    pub version: u64,
    texture: Option<(u64, TextureHandle)>,
    /// Où la zone de peinture a été dessinée à la dernière image (écran).
    pub zone: Rect,
}

impl PaletteMelange {
    pub fn vide() -> Self {
        PaletteMelange { raster: Raster::new(LARGEUR, HAUTEUR), stroke: None, version: 0, texture: None, zone: Rect::NOTHING }
    }

    /// La palette gardée dans `chemin` ; vide si le fichier manque, se lit mal ou n'a pas la taille de la palette.
    pub fn lire(chemin: &Path) -> Self {
        let mut p = Self::vide();
        if let Ok(img) = crate::io::load_image(chemin) {
            if img.width() as usize == LARGEUR && img.height() as usize == HAUTEUR {
                p.raster = Raster::from_rgba(LARGEUR, HAUTEUR, img.as_raw());
            }
        }
        p
    }

    /// Garde la palette dans `chemin` (PNG ; sans écriture partielle). Vide : le fichier est retiré.
    pub fn ecrire(&self, chemin: &Path) -> Result<(), String> {
        if self.raster.is_blank() {
            return match std::fs::remove_file(chemin) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        if let Some(dir) = chemin.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let png = crate::io::encode_png(&self.raster.to_rgba(), LARGEUR, HAUTEUR)?;
        crate::project::write_atomic(chemin, &png)
    }

    /// Vide la palette.
    pub fn vider(&mut self) {
        self.stroke = None;
        self.raster = Raster::new(LARGEUR, HAUTEUR);
        self.version += 1;
    }

    /// Début d'un trait de `couleur`, en `pos` (pixels de la palette).
    pub fn commencer(&mut self, pos: Pos2, couleur: [u8; 3], reglage: BrushSettings, textures: BrushTextures, toucher: Toucher) {
        let mut st = StrokeState::new(0, StrokeMode::Paint(couleur), reglage, None, false).with_textures(textures).with_feel(toucher);
        let mut dirty = Vec::new();
        st.add_point(&mut self.raster, pos, 1.0, &mut dirty);
        self.stroke = Some(st);
        self.version += 1;
    }

    /// Le trait avance jusqu'à `pos`.
    pub fn glisser(&mut self, pos: Pos2) {
        if let Some(st) = self.stroke.as_mut() {
            let mut dirty = Vec::new();
            st.add_point(&mut self.raster, pos, 1.0, &mut dirty);
            self.version += 1;
        }
    }

    /// Fin du trait ; vrai s'il y en avait un (la palette est alors à enregistrer).
    pub fn finir(&mut self) -> bool {
        let Some(mut st) = self.stroke.take() else { return false };
        let mut dirty = Vec::new();
        // (Pas d'annulation pour la palette : un bouton la vide, et c'est une zone d'essai.)
        let _ = st.finish(&mut self.raster, &mut dirty);
        self.version += 1;
        true
    }

    pub fn en_cours(&self) -> bool {
        self.stroke.is_some()
    }

    /// La couleur de la palette en `pos` (pixels de la palette) ; rien si la place est vide.
    pub fn couleur_en(&self, pos: Pos2) -> Option<[u8; 3]> {
        let (x, y) = (pos.x.floor(), pos.y.floor());
        if x < 0.0 || y < 0.0 || x >= LARGEUR as f32 || y >= HAUTEUR as f32 {
            return None;
        }
        let p = self.raster.get(x as usize, y as usize);
        (p.a() > 0).then(|| {
            let c = crate::raster::unpremultiply(p);
            [c[0], c[1], c[2]]
        })
    }

    /// La texture de la palette, mise à jour si elle a changé.
    fn texture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        let fraiche = self.texture.as_ref().is_some_and(|(v, _)| *v == self.version);
        if !fraiche {
            let pixels: Vec<Color32> = self.raster.to_flat();
            let image = egui::ColorImage { size: [LARGEUR, HAUTEUR], pixels };
            match self.texture.as_mut() {
                Some((v, tex)) => {
                    tex.set(image, TextureOptions::LINEAR);
                    *v = self.version;
                }
                None => self.texture = Some((self.version, ctx.load_texture("palette de mélange", image, TextureOptions::LINEAR))),
            }
        }
        self.texture.as_ref().map(|(_, t)| t.id()).unwrap_or_default()
    }
}

impl App {
    /// La palette, chargée la première fois qu'on en a besoin.
    pub(crate) fn palette_de_melange(&mut self) -> &mut PaletteMelange {
        self.confort.palette.get_or_insert_with(|| chemin().map_or_else(PaletteMelange::vide, |c| PaletteMelange::lire(&c)))
    }

    /// Garde la palette sur le disque (dans le dossier des réglages) ; l'échec se dit dans la barre d'état.
    pub(crate) fn enregistrer_palette(&mut self) {
        let (Some(chemin), Some(p)) = (chemin(), self.confort.palette.as_ref()) else { return };
        if let Err(e) = p.ecrire(&chemin) {
            self.status = format!("Palette de mélange non enregistrée : {e}");
        }
    }

    /// Le volet « Palette de mélange » du panneau Couleur : replié d'office, il montre la zone de peinture et le bouton
    /// qui la vide.
    pub(crate) fn palette_melange_ui(&mut self, ui: &mut egui::Ui) {
        let mut ouverte = self.confort.palette_ouverte;
        crate::repli::entete(
            ui,
            &mut ouverte,
            "Palette de mélange",
            "Une petite zone pour mêler des couleurs : on y peint avec la brosse courante, en mélange ; Alt + clic ou la pipette y prend une couleur.",
        );
        self.confort.palette_ouverte = ouverte;
        if !ouverte {
            return;
        }
        ui.add_space(2.0);
        let largeur = ui.available_width().clamp(120.0, 340.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(largeur, largeur * HAUTEUR as f32 / LARGEUR as f32), Sense::click_and_drag());
        let echelle = rect.width() / LARGEUR as f32;
        let (premier, pointeur, alt) = ui.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos(), i.modifiers.alt));
        let pos = pointeur.map(|p| {
            let q = ((p - rect.min) / echelle).to_pos2();
            pos2(q.x.clamp(0.0, LARGEUR as f32 - 0.01), q.y.clamp(0.0, HAUTEUR as f32 - 0.01))
        });
        // Le bouton est tenu depuis la zone : l'appui y a commencé (même si le pointeur en sort ensuite) et rien d'autre n'est
        // devant elle à cet endroit (une fenêtre ou un menu devant la palette prend le clic).
        let origine = ui.input(|i| i.pointer.press_origin());
        let layer = ui.layer_id();
        let ctx_ui = ui.ctx().clone();
        let sur_la_zone = |p: Pos2| rect.contains(p) && ctx_ui.layer_id_at(p).map_or(true, |l| l == layer);
        let tenu = premier && origine.is_some_and(sur_la_zone);        let pipette = alt || self.tool == Tool::Picker;
        self.palette_de_melange().zone = rect;
        // Le geste : un appui sur la zone commence un trait (ou prend une couleur) ; le trait se poursuit tant que le bouton est
        // tenu, et se finit au relâché (la palette est alors gardée sur le disque).
        let mut a_enregistrer = false;
        let en_cours = self.palette_de_melange().en_cours();
        match (pos, tenu, en_cours) {
            (Some(pos), true, false) if !pipette => {
                let reglage = reglage_de_palette(if self.tool == Tool::Eraser { self.brush } else { self.current_brush().unwrap_or(self.brush) });
                let textures = self.brushes.textures(&reglage);
                let (couleur, toucher) = (self.color, self.toucher);
                self.palette_de_melange().commencer(pos, couleur, reglage, textures, toucher);
            }
            (Some(pos), true, true) => self.palette_de_melange().glisser(pos),
            (Some(pos), true, false) => {
                // Alt + clic, ou l'outil Pipette : la couleur de la palette sous le pointeur.
                if let Some(c) = self.palette_de_melange().couleur_en(pos) {
                    self.set_front(c);
                }
            }
            (_, false, true) => a_enregistrer = self.palette_de_melange().finir(),
            _ => {}
        }
        if a_enregistrer {
            self.enregistrer_palette();
        }        // La zone : un fond neutre clair, la peinture par-dessus.
        let ctx = ui.ctx().clone();
        let tex = self.palette_de_melange().texture(&ctx);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, Color32::from_gray(232));
        painter.image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        painter.rect_stroke(rect, 3.0, egui::Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
        crate::bulles::bulle(
            resp,
            "Palette de mélange",
            "Peindre avec la brosse courante, en mélange ; Alt + clic (ou l'outil Pipette) : prendre une couleur ; elle est gardée pour ce poste.",
            "",
        );
        let vide = self.palette_de_melange().raster.is_blank();
        ui.horizontal(|ui| {
            if crate::bulles::icone_si(ui, !vide, ph::TRASH, "Vider la palette", "Efface tout ce qui est peint sur la palette de mélange.", "").clicked() {
                self.palette_de_melange().vider();
                self.enregistrer_palette();
            }
            ui.label(egui::RichText::new("Alt + clic : prendre une couleur").small().weak());
        });
    }
}
