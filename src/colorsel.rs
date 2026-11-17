//! Outil Sélection par couleur (Anthony, 2026-10-17 : « ajoute l'outil sélection au-dessus du
//! pinceau. Dans le même groupe, l'outil de sélection par la couleur »).
//!
//! - Calque bitmap : clic sur une couleur, la zone de cette couleur (d'un seul tenant autour du
//!   clic, ou tous les pixels de cette couleur), à la tolérance près. Elle devient la zone choisie
//!   de l'outil Sélection (copier, couper, effacer, flèches, poignées).
//! - Calque vectoriel : clic sur un trait ou une zone, tous les éléments de sa couleur dans le
//!   dessin.
//!
//! Maj ajoute à la sélection. Les poignées de la sélection restent utilisables avec cet outil.

use eframe::egui::{self, pos2, Color32, Pos2};

use crate::app::App;
use crate::document::LayerKind;
use crate::pixsel::PixelMask;
use crate::pixtools::BSelection;
use crate::vector::{self, VectorDrawing};
use crate::vtools::VSelection;

#[derive(Clone, Copy)]
pub struct ColorSelectSettings {
    /// Écart de couleur toléré, de 0 à 1 (comme le pot de peinture).
    pub tolerance: f32,
    /// La zone d'un seul tenant autour du clic, plutôt que tous les pixels de cette couleur.
    pub contiguous: bool,
}

impl Default for ColorSelectSettings {
    fn default() -> Self {
        Self { tolerance: 0.12, contiguous: true }
    }
}

/// Tous les pixels de l'image proches (à `tol` près sur chaque canal) de celui en (`sx`, `sy`).
pub fn similar(src: &[Color32], w: usize, h: usize, sx: usize, sy: usize, tol: u8) -> Vec<bool> {
    let target = src[sy * w + sx];
    let tol = tol as i32;
    let close = |c: Color32| {
        (c.r() as i32 - target.r() as i32).abs() <= tol
            && (c.g() as i32 - target.g() as i32).abs() <= tol
            && (c.b() as i32 - target.b() as i32).abs() <= tol
            && (c.a() as i32 - target.a() as i32).abs() <= tol
    };
    src[..w * h].iter().map(|&c| close(c)).collect()
}

/// Éléments du dessin de la même couleur que l'élément `i`, dans l'ordre du dessin.
pub fn same_color(d: &VectorDrawing, i: usize) -> Vec<usize> {
    let Some(color) = d.elements.get(i).map(|e| e.color()) else { return Vec::new() };
    // Une image collée n'a pas de couleur : elle ne se prend que seule, et jamais avec « les noirs ».
    if !d.elements[i].has_color() {
        return vec![i];
    }
    d.elements
        .iter()
        .enumerate()
        .filter(|(_, e)| e.has_color() && e.color() == color)
        .map(|(k, _)| k)
        .collect()
}

impl App {
    /// Clic avec l'outil Sélection par couleur (`screen` : position à l'écran, pour les poignées).
    pub(crate) fn color_select_press(&mut self, p: Pos2, screen: Pos2, shift: bool) {
        if !self.layer().drawable() {
            self.layer_editable();
            return;
        }
        // Une poignée de la sélection : on la transforme, comme avec l'outil Sélection.
        if self.selection_bounds().is_some() && self.handle_at(screen).is_some() {
            self.select_press(p, screen, false);
            return;
        }
        self.end_stroke();
        self.playing = false;
        match self.layer().kind {
            LayerKind::Vector => self.vector_color_select(p, shift),
            _ => self.bitmap_color_select(p, shift),
        }
    }

    fn bitmap_color_select(&mut self, p: Pos2, shift: bool) {
        let Some(cel) = self.cel_here() else {
            self.status = "Rien à sélectionner : cette image est vide.".into();
            return;
        };
        let add = shift && self.bsel.as_ref().is_some_and(|s| s.cel == cel && s.lifted.is_none());
        if !add {
            self.drop_bitmap_selection();
        }
        let raster = &self.doc.cels[&cel].raster;
        let (w, h) = (raster.width, raster.height);
        if p.x < 0.0 || p.y < 0.0 || p.x >= w as f32 || p.y >= h as f32 {
            return;
        }
        let src = raster.to_flat();
        let tol = (self.color_select.tolerance * 255.0).round() as u8;
        let (x, y) = (p.x as usize, p.y as usize);
        let region = if self.color_select.contiguous {
            crate::fill::flood(&src, w, h, x, y, tol)
        } else {
            similar(&src, w, h, x, y, tol)
        };
        let Some(mask) = PixelMask::from_bools(&region, w, h) else { return };
        let outline: Vec<Vec<Pos2>> = vector::trace_contours(&region, w, h)
            .into_iter()
            .map(|c| c.into_iter().map(|q| pos2(q[0], q[1])).collect())
            .collect();
        let n = region.iter().filter(|&&b| b).count();
        match self.bsel.as_mut().filter(|_| add) {
            Some(sel) => {
                sel.mask = sel.mask.union(&mask);
                sel.outline.extend(outline);
            }
            None => self.bsel = Some(BSelection { cel, mask, outline, lifted: None }),
        }
        self.status = format!("Couleur choisie : {n} pixels.");
    }

    fn vector_color_select(&mut self, p: Pos2, shift: bool) {
        let Some(cel) = self.cel_here() else { return };
        let tol = 4.0 / self.layer_zoom();
        let Some(d) = self.drawing(cel) else { return };
        let Some(i) = d.hit_test(p, tol) else {
            if !shift {
                self.vsel = None;
            }
            return;
        };
        let items = same_color(d, i);
        let n = items.len();
        let sel = self.vsel.get_or_insert(VSelection { cel, items: Vec::new() });
        if !shift || sel.cel != cel {
            sel.cel = cel;
            sel.items.clear();
        }
        for k in items {
            if !sel.items.contains(&k) {
                sel.items.push(k);
            }
        }
        self.status = format!("Couleur choisie : {n} élément(s).");
    }

    /// Réglages de l'outil, puis les commandes de la sélection.
    pub(crate) fn color_select_ui(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // (Le mode d'emploi est dans l'animation « La sélection par couleur » et le registre des explications, partie 153.)
        if self.layer().kind != LayerKind::Vector {
            ui.add(egui::Slider::new(&mut self.color_select.tolerance, 0.0..=1.0).text("Tolérance"));
            crate::bulles::interrupteur(
                ui,
                &mut self.color_select.contiguous,
                ph::LINK_SIMPLE,
                "Pixels voisins seulement",
                "La zone d'un seul tenant autour du clic ; sinon tous les pixels de cette couleur.",
                "",
            );
        }
        ui.separator();
        self.selection_ui(ui, false);
    }
}
