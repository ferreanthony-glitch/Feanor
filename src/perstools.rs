//! Outil Perspective (5f) : on tire les quatre coins du dessin (ou de la sélection vectorielle)
//! pour le mettre en perspective. Chaque lâcher refait la déformation depuis le dessin d'origine
//! (pas de perte de qualité d'un geste à l'autre) et compte pour une étape d'annulation.
//! Entrée, Échap, un clic hors du cadre ou un autre outil terminent. Géométrie dans `persp.rs`.

use eframe::egui::{self, vec2, Color32, Key, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::app::App;
use crate::document::CelId;
use crate::history::Edit;
use crate::persp;
use crate::raster::{Raster, Tile};
use crate::vector::VectorDrawing;

/// Rayon de prise des coins, en pixels d'écran.
const GRAB: f32 = 9.0;
const ACCENT: Color32 = Color32::from_rgb(70, 165, 255);

enum Original {
    Bitmap(Raster),
    Vector { drawing: VectorDrawing, items: Option<Vec<usize>> },
}

/// Ce que l'on tire : un coin, ou tout le cadre.
#[derive(Clone, Copy)]
enum Grip {
    Corner(usize),
    All,
}

pub(crate) struct PerspEdit {
    cel: CelId,
    src: Rect,
    quad: [Pos2; 4],
    original: Original,
    drag: Option<(Grip, Pos2, [Pos2; 4])>,
}

impl App {
    fn persp_grip(&self, screen: Pos2) -> Option<Grip> {
        let e = self.persp.as_ref()?;
        let corners = e.quad.map(|p| self.layer_to_screen(p));
        if let Some(i) = (0..4).find(|&i| corners[i].distance(screen) <= GRAB) {
            return Some(Grip::Corner(i));
        }
        crate::vector::point_in_polygon(screen, &corners).then_some(Grip::All)
    }

    pub(crate) fn persp_press(&mut self, p: Pos2, screen: Pos2) {
        self.playing = false;
        if let Some(grip) = self.persp_grip(screen) {
            if let Some(e) = &mut self.persp {
                e.drag = Some((grip, p, e.quad));
            }
            return;
        }
        // Clic hors du cadre : on termine, puis on prend le dessin courant.
        self.persp = None;
        self.end_stroke();
        if !self.layer_editable() {
            return;
        }
        let Some(cel) = self.cel_here() else {
            self.status = "Perspective : rien à déformer sur cette image.".into();
            return;
        };
        let Some(c) = self.doc.cels.get(&cel) else { return };
        let (original, src) = match &c.vector {
            Some(d) => {
                let items = self.vsel.as_ref().filter(|s| s.cel == cel && !s.items.is_empty()).map(|s| s.items.clone());
                let bounds = d
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| items.as_ref().is_none_or(|list| list.contains(i)))
                    .fold(Rect::NOTHING, |r, (_, e)| r.union(e.bounds()));
                (Original::Vector { drawing: d.clone(), items }, bounds)
            }
            None => match persp::raster_bounds(&c.raster) {
                Some(b) => (Original::Bitmap(c.raster.clone()), b),
                None => (Original::Bitmap(c.raster.clone()), Rect::NOTHING),
            },
        };
        if !src.is_positive() {
            self.status = "Perspective : rien à déformer sur cette image.".into();
            return;
        }
        self.persp = Some(PerspEdit {
            cel,
            src,
            quad: [src.left_top(), src.right_top(), src.right_bottom(), src.left_bottom()],
            original,
            drag: None,
        });
        self.status = "Perspective : tire un coin (ou l'intérieur pour tout déplacer). Entrée ou clic ailleurs : \
                       terminer ; Ctrl+Z : annuler."
            .into();
    }

    /// Mouvement du pointeur ; vrai si un coin (ou le cadre) est tiré.
    pub(crate) fn persp_drag(&mut self, p: Pos2) -> bool {
        let Some(e) = &mut self.persp else { return false };
        let Some((grip, start, quad0)) = e.drag else { return false };
        let mut q = quad0;
        match grip {
            Grip::Corner(i) => q[i] = quad0[i] + (p - start),
            Grip::All => q = quad0.map(|c| c + (p - start)),
        }
        // Un quadrilatère qui se croise ne se déforme pas : on garde le dernier correct.
        if persp::is_convex(&q) {
            e.quad = q;
        }
        true
    }

    /// Lâcher : la déformation est appliquée (une étape d'annulation).
    pub(crate) fn persp_release(&mut self) {
        let Some(e) = &mut self.persp else { return };
        let Some((_, _, quad0)) = e.drag.take() else { return };
        if quad0 == e.quad {
            return;
        }
        let (cel, src, quad) = (e.cel, e.src, e.quad);
        match &e.original {
            Original::Bitmap(orig) => {
                let Some(new) = persp::warp_raster(orig, src, quad) else { return };
                let Some(c) = self.doc.cels.get_mut(&cel) else { return };
                let mut before: Vec<(usize, Option<Tile>)> = Vec::new();
                let mut changed = Vec::new();
                for idx in 0..new.tiles.len() {
                    if c.raster.tiles.get(idx) != new.tiles.get(idx) {
                        before.push((idx, c.raster.tiles[idx].clone()));
                        changed.push(idx);
                    }
                }
                if changed.is_empty() {
                    return;
                }
                c.raster = new;
                c.version += 1;
                self.renderer.invalidate_tiles(&changed);
                self.history.push(Edit::Tiles { cel, tiles: before });
                self.modified = true;
            }
            Original::Vector { drawing, items } => {
                let Some(new) = persp::warp_drawing(drawing, items.as_deref(), src, quad) else { return };
                let Some(d) = self.drawing_mut(cel) else { return };
                let before = std::mem::replace(d, new);
                self.refresh_cel(cel);
                self.push_vector(cel, before, None);
            }
        }
    }

    /// Entrée ou Échap terminent la déformation ; vrai si la touche a servi.
    pub(crate) fn persp_key(&mut self, key: Key) -> bool {
        if self.persp.is_none() || !matches!(key, Key::Enter | Key::Escape) {
            return false;
        }
        self.persp = None;
        self.status = "Perspective terminée.".into();
        true
    }

    /// Le cadre suit le dessin : changer d'image ou de calque termine la déformation.
    pub(crate) fn check_persp(&mut self) {
        if self.persp.as_ref().is_some_and(|e| Some(e.cel) != self.cel_here()) {
            self.persp = None;
        }
    }

    /// Cadre, grille en perspective et coins.
    pub(crate) fn paint_persp_overlay(&self, painter: &egui::Painter) {
        let Some(e) = &self.persp else { return };
        let to_s = |p: Pos2| self.layer_to_screen(p);
        let q = e.quad.map(to_s);
        if let Some(h) = persp::rect_to_quad(e.src, e.quad) {
            let grid = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.45));
            for k in 1..4 {
                let t = k as f32 / 4.0;
                let x = e.src.min.x + e.src.width() * t;
                let y = e.src.min.y + e.src.height() * t;
                let v: Vec<Pos2> = (0..=16).map(|j| to_s(persp::apply(&h, egui::pos2(x, e.src.min.y + e.src.height() * j as f32 / 16.0)))).collect();
                let w: Vec<Pos2> = (0..=16).map(|j| to_s(persp::apply(&h, egui::pos2(e.src.min.x + e.src.width() * j as f32 / 16.0, y)))).collect();
                painter.add(Shape::line(v, grid));
                painter.add(Shape::line(w, grid));
            }
        }
        painter.add(Shape::closed_line(q.to_vec(), Stroke::new(2.5_f32, Color32::from_black_alpha(120))));
        painter.add(Shape::closed_line(q.to_vec(), Stroke::new(1.0_f32, ACCENT)));
        for c in q {
            painter.rect(Rect::from_center_size(c, vec2(9.0, 9.0)), 1.0, Color32::WHITE, Stroke::new(1.5_f32, ACCENT), StrokeKind::Middle);
        }
    }

    /// Curseur au-dessus du cadre.
    pub(crate) fn persp_cursor(&self, screen: Pos2) -> egui::CursorIcon {
        match (self.persp_grip(screen), self.persp.as_ref().is_some_and(|e| e.drag.is_some())) {
            (_, true) => egui::CursorIcon::Grabbing,
            (Some(Grip::Corner(_)), _) => egui::CursorIcon::Grab,
            (Some(Grip::All), _) => egui::CursorIcon::Move,
            (None, _) => egui::CursorIcon::Crosshair,
        }
    }

    /// Réglages de l'outil Perspective (panneau Réglages).
    pub(crate) fn persp_options_ui(&mut self, ui: &mut egui::Ui) {
        let editing = self.persp.is_some();
        ui.horizontal(|ui| {
            if ui.add_enabled(editing, egui::Button::new("Remettre droit")).on_hover_text("Les coins reviennent à leur place").clicked() {
                if let Some(e) = &mut self.persp {
                    let s = e.src;
                    e.drag = Some((Grip::All, Pos2::ZERO, e.quad));
                    e.quad = [s.left_top(), s.right_top(), s.right_bottom(), s.left_bottom()];
                }
                self.persp_release();
            }
            if ui.add_enabled(editing, egui::Button::new("Terminer")).on_hover_text("Entrée").clicked() {
                self.persp = None;
            }
        });
        ui.label(
            egui::RichText::new(
                "Clic sur le dessin : son cadre apparaît (sur un calque vectoriel, la sélection s'il y en a \
                 une). Tire un coin pour mettre en perspective, l'intérieur pour déplacer. Ctrl+Z annule \
                 chaque geste.",
            )
            .small()
            .weak(),
        );
    }
}
