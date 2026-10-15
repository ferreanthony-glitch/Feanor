//! Vue de dessus (2c-3) : la scène vue d'en haut. La caméra est en bas et son champ s'ouvre vers
//! le haut ; chaque calque est un trait horizontal à sa distance. Glisser un calque vers le haut
//! l'éloigne (il défilera moins vite), vers le bas le rapproche.
//!
//! Géométrie : un calque de profondeur z est à la distance D = 1 + z ; un point x de son dessin
//! est, vu de dessus, à X = D × (x − centre de la feuille). La caméra centrée en C est en
//! X = C − centre, et voit à la distance D une largeur D × cadre / zoom. Vu par ce sténopé, chaque
//! point tombe exactement où le met le défilement multiplan (`Layer::transform_seen`) : la vue de
//! dessus montre donc ce que la caméra filme, sans approximation.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Shape, Stroke};

use crate::app::App;
use crate::document::Layer;
use crate::history::Edit;

/// Au-delà de cette distance, les calques sont posés au bord haut (ciel, décor à l'infini).
const FAR: f32 = 12.0;
const GRAB: f32 = 7.0;
const MARGIN: f32 = 18.0;
const CAMERA: Color32 = Color32::from_rgb(255, 196, 60);
const CURRENT: Color32 = Color32::from_rgb(120, 190, 255);

/// Distance d'un calque de profondeur `depth` (1 = plan de référence).
pub fn distance(depth: f32) -> f32 {
    1.0 + depth.max(Layer::MIN_DEPTH)
}

/// Profondeur d'un calque posé à la distance `d` (bornée comme le champ « Profondeur »).
pub fn depth_of(d: f32) -> f32 {
    (d - 1.0).clamp(Layer::MIN_DEPTH, 1000.0)
}

/// Position vue de dessus d'un point `x` de la feuille, sur un calque à la distance `d`.
pub fn world_x(x: f32, stage_cx: f32, d: f32) -> f32 {
    (x - stage_cx) * d
}

/// Demi-largeur du champ de la caméra à la distance `d`.
pub fn half_field(frame_w: f32, zoom: f32, d: f32) -> f32 {
    d * frame_w / (2.0 * zoom.max(1e-3))
}

/// Cadrage de la vue de dessus : gardé tel quel pendant un glisser, pour que le calque reste sous
/// la souris.
#[derive(Clone, Copy)]
struct Fit {
    /// Milieu horizontal (en pixels de feuille, vu de dessus).
    x_mid: f32,
    /// Pixels d'écran par pixel de feuille.
    scale: f32,
    /// Longueur d'une unité de distance, en pixels de feuille (la largeur du cadre).
    unit: f32,
}

impl Fit {
    fn to_screen(&self, rect: Rect, x: f32, d: f32) -> Pos2 {
        pos2(
            rect.center().x + (x - self.x_mid) * self.scale,
            rect.bottom() - MARGIN - d * self.unit * self.scale,
        )
    }

    /// Distance correspondant à une hauteur d'écran.
    fn distance_at(&self, rect: Rect, y: f32) -> f32 {
        (rect.bottom() - MARGIN - y) / (self.unit * self.scale)
    }
}

pub(crate) struct TopDrag {
    layer: usize,
    fit: Fit,
    /// Écart entre la souris et le calque au moment de le prendre.
    grab: f32,
    snap: Edit,
    changed: bool,
}

/// Un calque tel qu'on le dessine vu de dessus.
struct Item {
    index: usize,
    /// Distance réelle et distance affichée (bornée à `FAR`).
    d: f32,
    shown: f32,
    x0: f32,
    x1: f32,
    /// Pas de dessin à cette image : on montre toute la feuille, en pointillés.
    empty: bool,
}

impl App {
    pub(crate) fn top_view_window(&mut self, ctx: &egui::Context) {
        if !self.top_view {
            return;
        }
        let mut open = true;
        egui::Window::new("Vue de dessus")
            .open(&mut open)
            .resizable(true)
            .default_size([480.0, 400.0])
            .min_size([320.0, 260.0])
            .show(ctx, |ui| self.top_view_ui(ui));
        if !open {
            self.top_view = false;
            self.finish_top_drag();
        }
    }

    /// Avancée de la caméra en distances de la vue de dessus (1 : jusqu'au plan de référence).
    fn camera_advance(&self, cam: &crate::camera::Camera) -> f32 {
        cam.dolly / crate::camera::ref_distance(self.doc.frame)
    }

    fn top_items(&self) -> Vec<Item> {
        let cx = self.doc.stage_center().x;
        let pivot = self.doc.stage_center();
        (0..self.layers().len())
            .map(|i| {
                let l = &self.layers()[i];
                let d = distance(l.depth);
                let shown = d.min(FAR);
                let (x0, x1, empty) = match self.layer_content_box(i, self.cur_frame) {
                    Some(b) => {
                        // Le dessin avec la transformation propre du calque (sans le défilement :
                        // c'est la caméra qui le produit, vue de dessus).
                        let aff = l.transform.affine(pivot);
                        let xs = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|p| aff.apply(p).x);
                        let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
                        (lo, hi, false)
                    }
                    None => (0.0, self.doc.width as f32, true),
                };
                Item { index: i, d, shown, x0: world_x(x0, cx, shown), x1: world_x(x1, cx, shown), empty }
            })
            .collect()
    }

    fn top_fit(&self, rect: Rect, items: &[Item]) -> Fit {
        let unit = self.doc.frame[0] as f32;
        let d_max = items.iter().map(|it| it.shown).fold(1.5_f32, f32::max) * 1.12;
        let cam = self.doc.panel_camera(self.cur_panel, self.cur_frame);
        let cam_x = cam.x - self.doc.stage_center().x;
        let half = half_field(unit, cam.zoom, d_max - self.camera_advance(&cam));
        let (mut lo, mut hi) = (cam_x - half, cam_x + half);
        for it in items {
            lo = lo.min(it.x0);
            hi = hi.max(it.x1);
        }
        let w = (rect.width() - 2.0 * MARGIN).max(10.0);
        let h = (rect.height() - 2.0 * MARGIN - 14.0).max(10.0);
        let scale = (w / (hi - lo).max(1.0)).min(h / (d_max * unit));
        Fit { x_mid: (lo + hi) / 2.0, scale, unit }
    }

    /// Calque sous le pointeur (le calque courant d'abord, puis du dessus vers le dessous).
    fn top_hit(&self, rect: Rect, fit: &Fit, items: &[Item], p: Pos2) -> Option<usize> {
        let mut order: Vec<&Item> = items.iter().rev().collect();
        order.sort_by_key(|it| it.index != self.cur_layer);
        order
            .into_iter()
            .find(|it| {
                let a = fit.to_screen(rect, it.x0, it.shown);
                let b = fit.to_screen(rect, it.x1, it.shown);
                (p.y - a.y).abs() <= GRAB && p.x >= a.x.min(b.x) - GRAB && p.x <= a.x.max(b.x) + GRAB
            })
            .map(|it| it.index)
    }

    fn top_view_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(
                "Glisser un calque vers le haut l'éloigne (il défile moins vite quand la caméra \
                 bouge), vers le bas le rapproche. Maj : par pas de 0,1.",
            )
            .small(),
        );
        let size = ui.available_size().max(vec2(300.0, 200.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let items = self.top_items();
        let fit = match &self.top_drag {
            Some(d) => d.fit,
            None => self.top_fit(rect, &items),
        };
        let pointer = resp.interact_pointer_pos().or(resp.hover_pos());

        // Prendre, glisser, lâcher un calque.
        if resp.drag_started() || resp.clicked() {
            if let Some(li) = pointer.and_then(|p| self.top_hit(rect, &fit, &items, p)) {
                self.select_layer(li);
                if resp.drag_started() {
                    if self.layers()[li].locked {
                        self.status = "Calque verrouillé : déverrouille-le pour changer sa profondeur.".into();
                    } else {
                        let y = fit.to_screen(rect, 0.0, items[li].shown).y;
                        self.top_drag = Some(TopDrag {
                            layer: li,
                            fit,
                            grab: pointer.map_or(0.0, |p| p.y - y),
                            snap: self.snapshot(),
                            changed: false,
                        });
                    }
                }
            }
        }
        if resp.dragged() {
            if let (Some(p), Some(d)) = (pointer, &self.top_drag) {
                let mut z = depth_of(d.fit.distance_at(rect, p.y - d.grab));
                if ui.input(|i| i.modifiers.shift) {
                    z = (z * 10.0).round() / 10.0;
                }
                let li = d.layer;
                if li < self.layers().len() && self.layers()[li].depth != z {
                    if let Some(d) = &mut self.top_drag {
                        d.changed = true;
                    }
                    self.layers_mut()[li].depth = z;
                    self.modified = true;
                    let l = &self.layers()[li];
                    self.status = format!(
                        "Calque « {} » : profondeur {:.2} (défile à {:.0} % du plan de référence).",
                        l.name,
                        z,
                        l.perspective() * 100.0
                    );
                }
            }
        }
        if resp.drag_stopped() || (self.top_drag.is_some() && !resp.dragged()) {
            self.finish_top_drag();
        }
        if self.top_drag.is_some() || pointer.is_some_and(|p| self.top_hit(rect, &fit, &items, p).is_some()) {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        }

        let items = self.top_items();
        self.paint_top_view(ui, rect, &fit, &items);
    }

    fn finish_top_drag(&mut self) {
        let Some(d) = self.top_drag.take() else { return };
        if d.changed {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    fn paint_top_view(&self, ui: &egui::Ui, rect: Rect, fit: &Fit, items: &[Item]) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_gray(28));
        let font = FontId::proportional(11.0);
        let dim = Color32::from_gray(150);

        // Distance du haut de la vue : le champ et les repères vont jusqu'au bord.
        let d_top = fit.distance_at(rect, rect.top());

        // Repères de distance : une ligne par unité, le plan de référence plus marqué.
        let step = if d_top > 8.0 { 2.0 } else { 1.0 };
        let mut d = 1.0;
        while d <= d_top {
            let y = fit.to_screen(rect, 0.0, d).y;
            let reference = d == 1.0;
            let color = if reference { Color32::from_gray(90) } else { Color32::from_gray(48) };
            painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0_f32, color));
            let label = if reference { "z 0 : plan de référence".to_string() } else { format!("z {}", d - 1.0) };
            painter.text(pos2(rect.right() - 4.0, y - 2.0), Align2::RIGHT_BOTTOM, label, font.clone(), Color32::from_gray(110));
            d += step;
        }

        // Champ de la caméra : au début et à la fin du panneau (si elle bouge), et à l'image courante.
        let unit = fit.unit;
        let cx = self.doc.stage_center().x;
        let cone = |frame: usize| {
            let cam = self.doc.panel_camera(self.cur_panel, frame);
            let x = cam.x - cx;
            // La caméra avancée (travelling) est plus haut, plus près des calques.
            let from = self.camera_advance(&cam);
            let half = half_field(unit, cam.zoom, d_top - from);
            [
                fit.to_screen(rect, x, from),
                fit.to_screen(rect, x - half, d_top),
                fit.to_screen(rect, x + half, d_top),
            ]
        };
        if self.panel().camera.len() > 1 {
            for f in [0, self.frame_count().saturating_sub(1)] {
                let [o, a, b] = cone(f);
                let ghost = Stroke::new(1.0_f32, CAMERA.gamma_multiply(0.35));
                painter.add(Shape::dashed_line(&[a, o, b], ghost, 4.0, 4.0));
            }
        }
        let [o, a, b] = cone(self.cur_frame);
        painter.add(Shape::convex_polygon(vec![o, a, b], CAMERA.gamma_multiply(0.08), Stroke::NONE));
        painter.line_segment([o, a], Stroke::new(1.5_f32, CAMERA));
        painter.line_segment([o, b], Stroke::new(1.5_f32, CAMERA));
        painter.add(Shape::convex_polygon(
            vec![o + vec2(-8.0, 10.0), o + vec2(8.0, 10.0), o + vec2(5.0, 0.0), o + vec2(-5.0, 0.0)],
            CAMERA,
            Stroke::NONE,
        ));

        // Calques : le courant en dernier, par-dessus.
        let mut labels: Vec<f32> = Vec::new();
        let mut order: Vec<&Item> = items.iter().collect();
        order.sort_by_key(|it| it.index == self.cur_layer);
        for it in order {
            let l = &self.layers()[it.index];
            let current = it.index == self.cur_layer;
            let mut color = if current { CURRENT } else { Color32::from_gray(200) };
            if !l.visible {
                color = color.gamma_multiply(0.35);
            }
            let a = fit.to_screen(rect, it.x0, it.shown);
            let b = fit.to_screen(rect, it.x1, it.shown);
            let stroke = Stroke::new(if current { 4.0_f32 } else { 3.0 }, color);
            if it.empty {
                painter.add(Shape::dashed_line(&[a, b], stroke, 6.0, 5.0));
            } else {
                painter.line_segment([a, b], stroke);
            }
            let mut text = format!("{}  z {:.2}", l.name, l.depth);
            if it.d > FAR {
                text.push_str(" (au loin)");
            }
            if l.locked {
                text.push_str(" 🔒");
            }
            // Étiquettes empilées quand plusieurs calques sont à la même distance.
            let stacked = labels.iter().filter(|&&y| (y - a.y).abs() < 10.0).count() as f32;
            labels.push(a.y);
            let at = pos2(a.x.max(rect.left() + 4.0).min(rect.right() - 120.0), a.y - 4.0 - stacked * 13.0);
            painter.text(at, Align2::LEFT_BOTTOM, text, font.clone(), if current { CURRENT } else { dim });
        }
    }
}
