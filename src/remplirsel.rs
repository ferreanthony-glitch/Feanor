//! Remplir une zone bitmap de la couleur courante (Anthony, 2026-10-20 : « LASSO : en bitmap, une
//! sélection peut être remplie par le raccourci Maj+Suppr »).
//!
//! `selection.remplir` (Maj+Suppr) : avec une zone bitmap choisie (lasso, rectangle, sélection par
//! couleur), la zone prend la couleur courante, en une étape d'annulation nommée « Remplir la
//! sélection ». Le calque verrouillé (ou dont la position l'est) refuse ; un calque qui préserve la
//! transparence ne change que la couleur de ce qui est déjà dessiné ; les pochoirs des autres
//! calques limitent le remplissage comme ils limitent le pinceau. Le bord de la zone (anticrénelé)
//! se mélange à l'image. Sans zone bitmap, Maj+Suppr fait ce qu'elle faisait avant : « Effacer ce qui
//! est hors de la sélection » (`dessin.supprimer_hors_selection`, toujours sur Maj+Retour arrière).

use eframe::egui::Color32;

use crate::app::App;
use crate::raster::{Tile, TILE};
use crate::render::over;

/// Le pixel `dst` (prémultiplié) après le remplissage de `rgb` avec la couverture `cover` (0 à 1) :
/// la couleur par-dessus ; sur un calque qui préserve la transparence, la couleur remplace celle du
/// pixel (son opacité reste) dans la mesure de la couverture, et le vide reste vide.
pub fn filled_pixel(dst: Color32, rgb: [u8; 3], cover: f32, keep_alpha: bool) -> Color32 {
    let cover = cover.clamp(0.0, 1.0);
    if cover <= 0.0 {
        return dst;
    }
    if keep_alpha {
        let a = dst.a();
        if a == 0 {
            return dst;
        }
        let k = a as f32 / 255.0;
        let m = |old: u8, new: u8| (old as f32 + (new as f32 * k - old as f32) * cover).round().clamp(0.0, 255.0) as u8;
        let out_a = a;
        return Color32::from_rgba_premultiplied(m(dst.r(), rgb[0]).min(out_a), m(dst.g(), rgb[1]).min(out_a), m(dst.b(), rgb[2]).min(out_a), out_a);
    }
    let s = |v: u8| (v as f32 * cover).round() as u8;
    let src = Color32::from_rgba_premultiplied(s(rgb[0]), s(rgb[1]), s(rgb[2]), s(255));
    over(dst, src, 255, None)
}

impl App {
    /// Remplit la zone bitmap choisie de la couleur courante. Faux (avec le message) si rien n'a
    /// changé.
    pub(crate) fn bitmap_fill_selection(&mut self) -> bool {
        if !self.layer_editable() {
            return false;
        }
        let Some(sel) = self.bsel.as_ref() else {
            self.status = "Rien de sélectionné : fais d'abord un lasso ou un rectangle avec l'outil Sélection.".into();
            return false;
        };
        if sel.lifted.is_some() {
            self.status = "La zone est soulevée : pose-la d'abord (Échap ou autre outil), puis remplis.".into();
            return false;
        }
        let (cel, mask) = (sel.cel, sel.mask.clone());
        let (rgb, keep_alpha) = (self.color, self.layer().keep_alpha);
        let stencil = self.stencil_for(cel);
        let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
        let raster = &mut c.raster;
        let b = mask.bounds();
        let (x0, y0) = (b.min.x.max(0.0) as usize, b.min.y.max(0.0) as usize);
        let (x1, y1) = ((b.max.x.max(0.0).ceil() as usize).min(raster.width), (b.max.y.max(0.0).ceil() as usize).min(raster.height));
        if x0 >= x1 || y0 >= y1 {
            self.status = "La zone est vide : rien à remplir.".into();
            return false;
        }
        // Les tuiles de la zone, avec leur état d'avant (de quoi annuler en une étape).
        let mut undo: Vec<(usize, Option<Tile>)> = Vec::new();
        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                let idx = ty * raster.tiles_x + tx;
                if stencil.as_ref().is_some_and(|st| st.blocks(idx)) {
                    continue;
                }
                let before = raster.tiles[idx].clone();
                let (ox, oy) = (tx * TILE, ty * TILE);
                let mut touched = false;
                for y in y0.max(oy)..y1.min(oy + TILE) {
                    for x in x0.max(ox)..x1.min(ox + TILE) {
                        let m = mask.at(x, y) * stencil.as_ref().map_or(1.0, |st| st.at(idx, (y - oy) * TILE + (x - ox)));
                        if m <= 0.0 {
                            continue;
                        }
                        let dst = raster.get(x, y);
                        let out = filled_pixel(dst, rgb, m, keep_alpha);
                        if out != dst {
                            raster.tile_mut(idx)[(y - oy) * TILE + (x - ox)] = out;
                            touched = true;
                        }
                    }
                }
                if touched {
                    undo.push((idx, before));
                }
            }
        }
        if undo.is_empty() {
            self.status = if keep_alpha { "Rien à remplir : ce calque préserve la transparence et la zone est vide de dessin.".into() } else { "Rien n'a changé : la zone avait déjà cette couleur.".into() };
            return false;
        }
        self.color_used(self.color);
        self.named_step("Remplir la sélection", |app| app.push_pixels(cel, None, undo));
        self.status = "Zone remplie de la couleur courante.".into();
        true
    }
}
