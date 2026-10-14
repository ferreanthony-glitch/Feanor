//! Outil Caméra et vue caméra : régler le cadre d'un panneau à la souris (déplacer, zoomer par un
//! coin, tourner par la poignée), le voir sur la feuille, ou regarder à travers lui.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::App;
use crate::camera::{self, Camera};
use crate::history::Edit;
use crate::view::View;

/// Rayon de prise des poignées, en pixels d'écran.
const GRAB: f32 = 10.0;
/// Distance de la poignée de rotation au-dessus du cadre, en pixels d'écran.
const ROTATE_UP: f32 = 28.0;
const CAMERA: Color32 = Color32::from_rgb(255, 196, 60);

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum CamHandle {
    Move,
    /// Un coin : on resserre ou élargit le cadre autour de son centre.
    Zoom,
    Rotate,
}

pub(crate) struct CamDrag {
    handle: CamHandle,
    start: Pos2,
    cam0: Camera,
    snap: Edit,
    changed: bool,
}

/// Vue de l'écran qui montre exactement le cadre de la caméra `cam` dans la zone `rect`.
pub(crate) fn view_through(cam: &Camera, frame: [usize; 2], stage: Vec2, rect: Rect) -> View {
    let zoom_cam = cam.zoom.max(0.01);
    let (w, h) = (frame[0] as f32 / zoom_cam, frame[1] as f32 / zoom_cam);
    let zoom = (rect.width() / w).min(rect.height() / h) * 0.96;
    let rotation = -cam.angle.to_radians();
    let v = pos2(cam.x, cam.y).to_vec2() - stage * 0.5;
    let (s, c) = rotation.sin_cos();
    let turned = vec2(v.x * c - v.y * s, v.x * s + v.y * c);
    View {
        zoom,
        rotation,
        flip: false,
        pan: -turned * zoom,
    }
}

impl App {
    /// Caméra du panneau courant à l'image courante.
    pub(crate) fn current_camera(&self) -> Camera {
        self.doc.panel_camera(self.cur_panel, self.cur_frame)
    }

    /// Règle la caméra du panneau courant. Caméra fixe (aucune ou une image clé) : on change
    /// cette caméra. Caméra animée (plusieurs clés) : on règle la clé de l'image courante (créée
    /// au besoin).
    pub(crate) fn set_current_camera(&mut self, cam: Camera) {
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let keys = &mut self.doc.panels[self.cur_panel].camera;
        if keys.len() >= 2 {
            camera::set_key(keys, frame, cam);
        } else {
            keys.clear();
            keys.push(camera::CameraKey { frame: 0, cam });
        }
        self.modified = true;
    }

    pub(crate) fn reset_camera(&mut self) {
        if self.doc.panels[self.cur_panel].camera.is_empty() {
            return;
        }
        self.record_step();
        self.doc.panels[self.cur_panel].camera.clear();
        self.status = "Caméra remise au centre, sans zoom ni rotation.".into();
    }

    fn to_screen(&self, p: Pos2) -> Pos2 {
        self.view.to_screen(self.canvas_rect, self.canvas_size(), p)
    }

    /// Poignée de rotation : au-dessus du milieu du bord haut du cadre.
    fn rotate_handle(&self, corners: &[Pos2; 4]) -> Pos2 {
        let (a, b, d) = (self.to_screen(corners[0]), self.to_screen(corners[1]), self.to_screen(corners[3]));
        let mid = a + (b - a) * 0.5;
        let up = (a - d).normalized();
        mid + up * ROTATE_UP
    }

    /// Partie du cadre caméra sous le pointeur (position écran).
    pub(crate) fn camera_handle_at(&self, screen: Pos2) -> Option<CamHandle> {
        let corners = self.current_camera().corners(self.doc.frame);
        if self.rotate_handle(&corners).distance(screen) <= GRAB {
            return Some(CamHandle::Rotate);
        }
        if corners.iter().any(|c| self.to_screen(*c).distance(screen) <= GRAB) {
            return Some(CamHandle::Zoom);
        }
        let quad: Vec<Pos2> = corners.iter().map(|c| self.to_screen(*c)).collect();
        crate::vector::point_in_polygon(screen, &quad).then_some(CamHandle::Move)
    }

    pub(crate) fn camera_press(&mut self, p: Pos2, screen: Pos2) {
        self.end_stroke();
        self.playing = false;
        let Some(handle) = self.camera_handle_at(screen) else {
            self.status = "Outil Caméra : glisser dans le cadre pour le déplacer, un coin pour zoomer, \
                           la poignée ronde pour tourner."
                .into();
            return;
        };
        self.cam_drag = Some(CamDrag {
            handle,
            start: p,
            cam0: self.current_camera(),
            snap: self.snapshot(),
            changed: false,
        });
    }

    pub(crate) fn camera_drag(&mut self, p: Pos2, shift: bool) {
        let Some(d) = &mut self.cam_drag else { return };
        let (cam0, start) = (d.cam0, d.start);
        let c = pos2(cam0.x, cam0.y);
        let cam = match d.handle {
            CamHandle::Move => {
                let mut delta = p - start;
                if shift {
                    if delta.x.abs() > delta.y.abs() {
                        delta.y = 0.0;
                    } else {
                        delta.x = 0.0;
                    }
                }
                cam0.shifted(delta.x, delta.y)
            }
            CamHandle::Zoom => {
                // Éloigner le coin du centre élargit le cadre (moins de zoom).
                let ratio = start.distance(c).max(1.0) / p.distance(c).max(1.0);
                Camera {
                    zoom: (cam0.zoom * ratio).clamp(0.05, 50.0),
                    ..cam0
                }
            }
            CamHandle::Rotate => {
                let turn = ((p - c).angle() - (start - c).angle()).to_degrees();
                let mut angle = cam0.angle + turn;
                if shift {
                    angle = (angle / 15.0).round() * 15.0;
                }
                Camera { angle, ..cam0 }
            }
        };
        d.changed = true;
        self.set_current_camera(cam);
        self.status = format!(
            "Caméra : décalage {:+.0} × {:+.0} px, zoom {:.0} %, rotation {:.1}°",
            cam.x - self.doc.width as f32 / 2.0,
            cam.y - self.doc.height as f32 / 2.0,
            cam.zoom * 100.0,
            cam.angle
        );
    }

    pub(crate) fn finish_camera_drag(&mut self) {
        let Some(d) = self.cam_drag.take() else { return };
        if d.changed {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    /// Ce qui est fait avant de dessiner le canevas : en vue caméra, la vue suit la caméra.
    pub(crate) fn apply_camera_view(&mut self, rect: Rect) {
        if self.camera_view {
            self.view = view_through(&self.current_camera(), self.doc.frame, self.canvas_size(), rect);
        }
    }

    /// Cadre de la caméra par-dessus le dessin : hors-champ estompé (masqué en vue caméra), cadre,
    /// et poignées avec l'outil Caméra.
    pub(crate) fn paint_camera(&self, painter: &egui::Painter, rect: Rect) {
        let size = self.canvas_size();
        let to_s = |p: Pos2| self.view.to_screen(rect, size, p);
        let cam = self.current_camera();
        let corners = cam.corners(self.doc.frame);
        let stage = Rect::from_min_size(Pos2::ZERO, size);
        let fills_stage = cam == self.doc.default_camera() && self.doc.frame_rect() == stage;
        if fills_stage && !self.camera_view {
            return;
        }
        // Hors-champ : en vue caméra, tout l'écran autour du cadre ; sinon la feuille, si on veut.
        let alpha = if self.camera_view {
            235
        } else if !self.camera_mask {
            0
        } else if self.playing {
            150
        } else {
            70
        };
        if alpha > 0 {
            let outer: [Pos2; 4] = if self.camera_view {
                [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]
            } else {
                [stage.left_top(), stage.right_top(), stage.right_bottom(), stage.left_bottom()].map(to_s)
            };
            let inner = corners.map(to_s);
            let mut mesh = egui::Mesh::default();
            // En vue caméra, rien ne doit transparaître hors du cadre : couleur du fond, opaque.
            let color = if self.camera_view {
                Color32::from_gray(42)
            } else {
                Color32::from_black_alpha(alpha)
            };
            for p in outer.iter().chain(inner.iter()) {
                mesh.colored_vertex(*p, color);
            }
            // Quatre bandes entre le bord extérieur (0..4) et le cadre (4..8).
            for i in 0..4u32 {
                let j = (i + 1) % 4;
                mesh.add_triangle(i, j, 4 + j);
                mesh.add_triangle(i, 4 + j, 4 + i);
            }
            painter.add(Shape::mesh(mesh));
        }
        if self.camera_view {
            return;
        }
        let quad: Vec<Pos2> = corners.iter().map(|c| to_s(*c)).collect();
        let editing = self.tool == crate::app::Tool::Camera;
        painter.add(Shape::closed_line(quad.clone(), Stroke::new(3.0_f32, Color32::from_black_alpha(120))));
        painter.add(Shape::closed_line(
            quad.clone(),
            Stroke::new(if editing { 2.0_f32 } else { 1.0 }, if editing { CAMERA } else { Color32::from_gray(235) }),
        ));
        painter.text(
            quad[0] + vec2(4.0, -3.0),
            egui::Align2::LEFT_BOTTOM,
            if self.doc.panels[self.cur_panel].camera.len() >= 2 {
                "Caméra (animée)"
            } else {
                "Caméra"
            },
            egui::FontId::proportional(11.0),
            if editing { CAMERA } else { Color32::from_gray(225) },
        );
        if editing {
            let r = self.rotate_handle(&corners);
            painter.line_segment([quad[0] + (quad[1] - quad[0]) * 0.5, r], Stroke::new(1.0_f32, CAMERA));
            painter.circle(r, 5.5, Color32::WHITE, Stroke::new(1.5_f32, CAMERA));
            for q in &quad {
                painter.rect(
                    Rect::from_center_size(*q, Vec2::splat(9.0)),
                    1.0,
                    Color32::WHITE,
                    Stroke::new(1.5_f32, CAMERA),
                    egui::StrokeKind::Middle,
                );
            }
            // Centre du cadre.
            let c = to_s(pos2(cam.x, cam.y));
            painter.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(1.0_f32, CAMERA));
            painter.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], Stroke::new(1.0_f32, CAMERA));
        }
    }

    /// Réglages de la caméra du panneau courant (fiche du panneau et outil Caméra).
    pub(crate) fn camera_fields(&mut self, ui: &mut egui::Ui) {
        let cam = self.current_camera();
        let (cx, cy) = (self.doc.width as f32 / 2.0, self.doc.height as f32 / 2.0);
        let (mut dx, mut dy, mut zoom, mut angle) = (cam.x - cx, cam.y - cy, cam.zoom * 100.0, cam.angle);
        let mut next = None;
        ui.horizontal(|ui| {
            let r = ui.add(egui::DragValue::new(&mut dx).speed(2.0).prefix("x ").suffix(" px"));
            if self.edited(&r) {
                next = Some(Camera { x: cx + dx, ..cam });
            }
            let r = ui.add(egui::DragValue::new(&mut dy).speed(2.0).prefix("y ").suffix(" px"));
            if self.edited(&r) {
                next = Some(Camera { y: cy + dy, ..cam });
            }
        });
        ui.horizontal(|ui| {
            let r = ui.add(egui::DragValue::new(&mut zoom).speed(1.0).range(5.0..=5000.0).suffix(" %"));
            if self.edited(&r) {
                next = Some(Camera { zoom: zoom / 100.0, ..cam });
            }
            let r = ui.add(egui::DragValue::new(&mut angle).speed(0.5).suffix("°"));
            if self.edited(&r) {
                next = Some(Camera { angle, ..cam });
            }
            if ui.small_button("Réinitialiser").clicked() {
                self.reset_camera();
            }
        });
        if let Some(c) = next {
            self.set_current_camera(c);
        }
    }
}
