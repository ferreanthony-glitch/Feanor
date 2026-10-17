//! Outil Transformer : déplacer, mettre à l'échelle, étirer et tourner un calque entier (tous ses
//! dessins). La transformation est enregistrée à part (`Layer::transform`) : les dessins ne sont
//! pas recalculés, et Ctrl+R la retire. Comme l'outil de transformation de Storyboard Pro (Anthony,
//! 2026-10-18 : « beaucoup plus de transformations comme l'étirement latéral ») : un coin agrandit
//! librement (Maj : en gardant les proportions), le milieu d'un côté étire dans un seul sens, le
//! côté opposé restant en place ; comportement exact de Storyboard Pro à vérifier.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::App;
use crate::document::LayerTransform;
use crate::history::Edit;

const GRAB: f32 = 10.0;
const ROTATE_UP: f32 = 28.0;
const COLOR: Color32 = Color32::from_rgb(120, 220, 140);

/// Partie de la boîte de l'outil Transformer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoxHandle {
    Move,
    /// Coins : haut-gauche, haut-droite, bas-droite, bas-gauche.
    Corner(usize),
    /// Milieux des côtés : haut, droite, bas, gauche.
    Edge(usize),
    Rotate,
}

/// Étirement par un coin ou le milieu d'un côté de la boîte `b` du contenu (repère du dessin du
/// calque) : `m`, la souris ramenée dans ce repère ; le coin (ou le côté) opposé reste en place sur
/// la feuille. Coin : largeur et hauteur (Maj : les deux pareil) ; côté : un seul sens.
pub(crate) fn stretched(t0: LayerTransform, pivot: Pos2, b: Rect, handle: BoxHandle, m: Pos2, shift: bool) -> LayerTransform {
    let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()];
    let mids = [b.center_top(), b.right_center(), b.center_bottom(), b.left_center()];
    let (h, anchor, along_x, along_y) = match handle {
        BoxHandle::Corner(k) => (corners[k], corners[(k + 2) % 4], true, true),
        BoxHandle::Edge(k) => (mids[k], mids[(k + 2) % 4], k % 2 == 1, k % 2 == 0),
        _ => return t0,
    };
    let ratio = |num: f32, den: f32| if den.abs() < 1e-3 { 1.0 } else { (num / den).clamp(0.01, 100.0) };
    let (mut fx, mut fy) = (
        if along_x { ratio(m.x - anchor.x, h.x - anchor.x) } else { 1.0 },
        if along_y { ratio(m.y - anchor.y, h.y - anchor.y) } else { 1.0 },
    );
    if shift && along_x && along_y {
        let d = h - anchor;
        let f = ((m - anchor).dot(d) / d.length_sq().max(1e-6)).clamp(0.01, 100.0);
        (fx, fy) = (f, f);
    }
    let t1 = LayerTransform {
        stretch_x: (t0.stretch_x * fx).clamp(0.01, 100.0),
        stretch_y: (t0.stretch_y * fy).clamp(0.01, 100.0),
        ..t0
    };
    // Le point opposé reste où il était sur la feuille.
    let keep = t0.affine(pivot).apply(anchor) - t1.affine(pivot).apply(anchor);
    LayerTransform { x: t1.x + keep.x, y: t1.y + keep.y, ..t1 }
}

pub(crate) struct LayerDrag {
    handle: BoxHandle,
    start: Pos2,
    t0: LayerTransform,
    /// Centre de la boîte dans le repère de la transformation du calque : point fixe de l'échelle
    /// et de la rotation.
    center: Pos2,
    /// Le même centre tel qu'on le voit (avec le défilement multiplan) : pour lire la souris.
    seen_center: Pos2,
    /// Boîte du contenu (repère du dessin) et passage de la feuille à ce repère, tels qu'au début
    /// du geste : pour les étirements.
    content: Rect,
    to_local: Option<crate::vector::Affine>,
    snap: Edit,
    changed: bool,
}

impl App {
    /// Boîte du contenu du calque `li` à l'image `frame`, en coordonnées de son dessin (rien s'il
    /// est vide). Bitmap : à la tuile près.
    pub(crate) fn layer_content_box(&self, li: usize, frame: usize) -> Option<Rect> {
        let id = self.layers().get(li)?.shown_at(frame)?;
        let cel = self.doc.cels.get(&id)?;
        let r = match &cel.vector {
            Some(d) => d.elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds())),
            None => (0..cel.raster.tiles.len())
                .filter(|&i| cel.raster.tiles[i].is_some())
                .map(|i| {
                    let (x, y, w, h) = cel.raster.tile_bounds(i);
                    Rect::from_min_size(pos2(x as f32, y as f32), vec2(w as f32, h as f32))
                })
                .fold(Rect::NOTHING, |r, t| r.union(t)),
        };
        (r.is_finite() && r.width() > 0.0 && r.height() > 0.0).then_some(r)
    }

    /// Boîte du contenu du calque courant à l'image courante (le cadre de la caméra s'il est vide).
    fn content_box(&self) -> Rect {
        self.layer_content_box(self.cur_layer, self.cur_frame)
            .unwrap_or_else(|| self.doc.frame_rect())
    }

    /// Coins de la boîte sur la feuille (haut-gauche, haut-droite, bas-droite, bas-gauche).
    fn box_corners(&self) -> [Pos2; 4] {
        let b = self.content_box();
        let aff = self.layer_affine();
        [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|p| aff.apply(p))
    }

    fn stage_to_screen(&self, p: Pos2) -> Pos2 {
        self.view.to_screen(self.canvas_rect, self.canvas_size(), p)
    }

    fn box_rotate_handle(&self, corners: &[Pos2; 4]) -> Pos2 {
        let (a, b, d) = (
            self.stage_to_screen(corners[0]),
            self.stage_to_screen(corners[1]),
            self.stage_to_screen(corners[3]),
        );
        let up = (a - d).normalized();
        a + (b - a) * 0.5 + up * ROTATE_UP
    }

    /// Milieux des côtés de la boîte sur la feuille (haut, droite, bas, gauche).
    fn box_mids(corners: &[Pos2; 4]) -> [Pos2; 4] {
        [0, 1, 2, 3].map(|k| corners[k] + (corners[(k + 1) % 4] - corners[k]) * 0.5)
    }

    /// Partie de la boîte sous le pointeur (position écran).
    pub(crate) fn transform_handle_at(&self, screen: Pos2) -> Option<BoxHandle> {
        let corners = self.box_corners();
        if self.box_rotate_handle(&corners).distance(screen) <= GRAB {
            return Some(BoxHandle::Rotate);
        }
        if let Some(k) = corners.iter().position(|c| self.stage_to_screen(*c).distance(screen) <= GRAB) {
            return Some(BoxHandle::Corner(k));
        }
        if let Some(k) = Self::box_mids(&corners).iter().position(|c| self.stage_to_screen(*c).distance(screen) <= GRAB) {
            return Some(BoxHandle::Edge(k));
        }
        let quad: Vec<Pos2> = corners.iter().map(|c| self.stage_to_screen(*c)).collect();
        crate::vector::point_in_polygon(screen, &quad).then_some(BoxHandle::Move)
    }

    pub(crate) fn transform_press(&mut self, stage: Pos2, screen: Pos2) {
        self.end_stroke();
        self.playing = false;
        if self.content_refuses() {
            return;
        }
        if !self.layer().drawable() {
            self.layer_editable();
            return;
        }
        let Some(handle) = self.transform_handle_at(screen) else {
            self.status = "Transformer : glisser dans la boîte pour déplacer le calque, un coin pour \
                           l'agrandir (Maj : proportions), le milieu d'un côté pour l'étirer, la poignée \
                           ronde pour le tourner."
                .into();
            return;
        };
        let corners = self.box_corners();
        // Centre de la boîte telle qu'on la voit, ramené au repère de la transformation du calque
        // (sans le défilement multiplan, qui ne dépend que de la caméra).
        let (seen, own) = (self.layer_seen(), self.layer().transform);
        let seen_center = pos2(
            corners.iter().map(|c| c.x).sum::<f32>() / 4.0,
            corners.iter().map(|c| c.y).sum::<f32>() / 4.0,
        );
        let center = seen_center - vec2(seen.x - own.x, seen.y - own.y);
        self.layer_drag = Some(LayerDrag {
            handle,
            start: stage,
            t0: own,
            center,
            seen_center,
            content: self.content_box(),
            to_local: self.layer_affine().inverse(),
            snap: self.snapshot(),
            changed: false,
        });
    }

    pub(crate) fn transform_drag(&mut self, p: Pos2, shift: bool) {
        let pivot = self.doc.stage_center();
        let Some(d) = &mut self.layer_drag else { return };
        let (t0, start, q, seen) = (d.t0, d.start, d.center, d.seen_center);
        let t = match d.handle {
            BoxHandle::Corner(_) | BoxHandle::Edge(_) => match d.to_local {
                Some(inv) => stretched(t0, pivot, d.content, d.handle, inv.apply(p), shift),
                None => t0,
            },
            BoxHandle::Move => {
                let mut delta = p - start;
                if shift {
                    if delta.x.abs() > delta.y.abs() {
                        delta.y = 0.0;
                    } else {
                        delta.x = 0.0;
                    }
                }
                LayerTransform {
                    x: t0.x + delta.x,
                    y: t0.y + delta.y,
                    ..t0
                }
            }
            BoxHandle::Rotate => {
                let mut turn = ((p - seen).angle() - (start - seen).angle()).to_degrees();
                if shift {
                    turn = ((t0.angle + turn) / 15.0).round() * 15.0 - t0.angle;
                }
                t0.around(pivot, q, 1.0, turn)
            }
        };
        d.changed = true;
        let li = self.cur_layer;
        self.layers_mut()[li].transform = t;
        self.modified = true;
        self.status = format!(
            "Calque : décalage {:+.0} × {:+.0} px, largeur {:.0} %, hauteur {:.0} %, rotation {:.1}°",
            t.x,
            t.y,
            t.scale * t.stretch_x * 100.0,
            t.scale * t.stretch_y * 100.0,
            t.angle
        );
    }

    pub(crate) fn finish_layer_drag(&mut self) {
        let Some(d) = self.layer_drag.take() else { return };
        if d.changed {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    /// Ctrl+R : le calque courant retrouve sa place, sa taille et son orientation d'origine.
    pub(crate) fn reset_layer_transform(&mut self) {
        if self.layer().transform.is_identity() {
            self.status = "Ce calque n'est pas transformé.".into();
            return;
        }
        if self.layer().position_locked {
            self.status = crate::app::POSITION_LOCKED.into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.record_step();
        let li = self.cur_layer;
        self.layers_mut()[li].transform = LayerTransform::default();
        self.status = "Transformation du calque annulée.".into();
    }

    /// Boîte et poignées de l'outil Transformer.
    pub(crate) fn paint_layer_transform(&self, painter: &egui::Painter) {
        if self.tool != crate::app::Tool::Transform {
            return;
        }
        let corners = self.box_corners();
        let quad: Vec<Pos2> = corners.iter().map(|c| self.stage_to_screen(*c)).collect();
        painter.add(Shape::closed_line(quad.clone(), Stroke::new(3.0_f32, Color32::from_black_alpha(110))));
        painter.add(Shape::closed_line(quad.clone(), Stroke::new(1.5_f32, COLOR)));
        let r = self.box_rotate_handle(&corners);
        painter.line_segment([quad[0] + (quad[1] - quad[0]) * 0.5, r], Stroke::new(1.0_f32, COLOR));
        painter.circle(r, 5.5, Color32::WHITE, Stroke::new(1.5_f32, COLOR));
        for q in &quad {
            painter.rect(
                Rect::from_center_size(*q, Vec2::splat(9.0)),
                1.0,
                Color32::WHITE,
                Stroke::new(1.5_f32, COLOR),
                egui::StrokeKind::Middle,
            );
        }
        // Les milieux des côtés : étirer dans un seul sens.
        for m in Self::box_mids(&corners) {
            painter.rect(
                Rect::from_center_size(self.stage_to_screen(m), Vec2::splat(7.0)),
                1.0,
                Color32::WHITE,
                Stroke::new(1.5_f32, COLOR),
                egui::StrokeKind::Middle,
            );
        }
    }

    /// Valeurs de la transformation du calque courant (outil Transformer).
    pub(crate) fn transform_fields(&mut self, ui: &mut egui::Ui) {
        if self.layer().locked {
            ui.label("Calque verrouillé : déverrouille-le pour le transformer.");
            return;
        }
        if self.layer().position_locked {
            ui.label("Position verrouillée : déverrouille-la pour transformer le calque.");
            return;
        }
        let t = self.layer().transform;
        let (mut x, mut y, mut angle) = (t.x, t.y, t.angle);
        let (mut width, mut height) = (t.scale * t.stretch_x * 100.0, t.scale * t.stretch_y * 100.0);
        let mut next = None;
        ui.horizontal(|ui| {
            let r = ui.add(egui::DragValue::new(&mut x).speed(2.0).prefix("x ").suffix(" px"));
            if self.edited(&r) {
                next = Some(LayerTransform { x, ..t });
            }
            let r = ui.add(egui::DragValue::new(&mut y).speed(2.0).prefix("y ").suffix(" px"));
            if self.edited(&r) {
                next = Some(LayerTransform { y, ..t });
            }
        });
        ui.horizontal(|ui| {
            // Largeur et hauteur : l'échelle du calque fois son étirement dans chaque sens.
            let r = ui.add(egui::DragValue::new(&mut width).speed(1.0).range(1.0..=10000.0).prefix("largeur ").suffix(" %"));
            if self.edited(&r) {
                next = Some(LayerTransform { stretch_x: (width / 100.0 / t.scale).clamp(0.01, 100.0), ..t });
            }
            let r = ui.add(egui::DragValue::new(&mut height).speed(1.0).range(1.0..=10000.0).prefix("hauteur ").suffix(" %"));
            if self.edited(&r) {
                next = Some(LayerTransform { stretch_y: (height / 100.0 / t.scale).clamp(0.01, 100.0), ..t });
            }
        });
        ui.horizontal(|ui| {
            let r = ui.add(egui::DragValue::new(&mut angle).speed(0.5).suffix("°"));
            if self.edited(&r) {
                next = Some(LayerTransform { angle, ..t });
            }
        });
        let resp = self.bouton_si(
            ui,
            !t.is_identity(),
            egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
            "Annuler la transformation",
            "Le calque revient à sa place, à sa taille, sans rotation ; ses dessins n'ont jamais changé.",
            "calque.reinit_transformation",
        );
        if resp.clicked() {
            self.reset_layer_transform();
        }
        if let Some(t) = next {
            let li = self.cur_layer;
            self.layers_mut()[li].transform = t;
        }
    }
}
