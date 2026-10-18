//! Outil Caméra et vue caméra : régler le cadre d'un panneau à la souris (déplacer, zoomer par un
//! coin, tourner par la poignée), le voir sur la feuille, ou regarder à travers lui.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Shape, Stroke, Vec2};

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

/// Rang de la clé sous l'abscisse `x` dans une rangée de clés : la plus proche, à `tol` près.
/// `x_of(f)` : abscisse du milieu de l'image `f`.
pub(crate) fn camera_key_at(keys: &[camera::CameraKey], x: f32, x_of: impl Fn(usize) -> f32, tol: f32) -> Option<usize> {
    keys.iter()
        .enumerate()
        .map(|(i, k)| (i, (x_of(k.frame) - x).abs()))
        .filter(|&(_, d)| d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
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
    if !cam.is_level() {
        return turned_view(cam, frame, rect);
    }
    let zoom_cam = cam.magnification(frame);
    let (w, h) = (frame[0] as f32 / zoom_cam, frame[1] as f32 / zoom_cam);
    let zoom = (rect.width() / w).min(rect.height() / h) * 0.96;
    // (0 et non -0 : la vue affiche sa rotation en degrés.)
    let rotation = if cam.angle == 0.0 { 0.0 } else { -cam.angle.to_radians() };
    let v = pos2(cam.x, cam.y).to_vec2() - stage * 0.5;
    let (s, c) = rotation.sin_cos();
    let turned = vec2(v.x * c - v.y * s, v.x * s + v.y * c);
    View {
        zoom,
        rotation,
        flip: false,
        pan: -turned * zoom,
        persp: None,
    }
}

/// Vue à travers une caméra basculée ou en panoramique (9f) : l'image finale centrée dans
/// `rect` (à 96 %), et la feuille vue par la caméra, en perspective.
pub(crate) fn turned_view(cam: &Camera, frame: [usize; 2], rect: Rect) -> View {
    use crate::persp;
    let (fw, fh) = (frame[0] as f32, frame[1] as f32);
    let z = (rect.width() / fw).min(rect.height() / fh) * 0.96;
    let (ox, oy) = (rect.center().x - z * fw / 2.0, rect.center().y - z * fh / 2.0);
    let frame_to_screen: persp::Homography = [z, 0.0, ox, 0.0, z, oy, 0.0, 0.0, 1.0];
    let screen_to_frame: persp::Homography = [1.0 / z, 0.0, -ox / z, 0.0, 1.0 / z, -oy / z, 0.0, 0.0, 1.0];
    View {
        zoom: z * cam.magnification(frame),
        rotation: if cam.angle == 0.0 { 0.0 } else { -cam.angle.to_radians() },
        flip: false,
        pan: Vec2::ZERO,
        persp: Some(crate::view::Persp {
            to_screen: persp::mul(&frame_to_screen, &cam.homography_inverse(frame)),
            to_stage: persp::mul(&cam.homography(frame), &screen_to_frame),
        }),
    }
}

impl App {
    /// Caméra du panneau courant à l'image courante.
    pub(crate) fn current_camera(&self) -> Camera {
        self.doc.panel_camera(self.cur_panel, self.cur_frame)
    }

    /// Règle la caméra du plan courant. Caméra fixe (aucune ou une image clé) : on change
    /// cette caméra. Caméra animée (plusieurs clés) : on règle la clé de l'image courante (créée
    /// au besoin).
    pub(crate) fn set_current_camera(&mut self, cam: Camera) {
        // Tournée, elle doit encore voir le plan de référence par les quatre coins du cadre.
        let cam = cam.limited(self.doc.frame);
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let keys = &mut self.doc.panels[self.cur_panel].camera;
        if keys.len() >= 2 {
            camera::set_key(keys, frame, cam);
        } else {
            keys.clear();
            keys.push(camera::CameraKey::new(0, cam));
        }
        self.modified = true;
    }

    /// Ajoute une image clé de caméra à l'image `frame` du panneau courant, avec la caméra telle
    /// qu'elle y est déjà (l'image ne change pas) ; on la règle ensuite à la souris.
    pub(crate) fn add_camera_key(&mut self, frame: usize) {
        let frame = frame.min(self.frame_count().saturating_sub(1));
        let cam = self.doc.panel_camera(self.cur_panel, frame);
        self.record_step();
        let keys = &mut self.doc.panels[self.cur_panel].camera;
        // Une caméra fixe sans clé à elle devient la clé du début.
        if keys.is_empty() && frame > 0 {
            keys.push(camera::CameraKey::new(0, cam));
        }
        camera::set_key(keys, frame, cam);
        let n = keys.len();
        self.status = format!(
            "Image clé de caméra à l'image {} ({n} clé{} dans ce plan). Règle la caméra avec l'outil Caméra.",
            frame + 1,
            if n > 1 { "s" } else { "" }
        );
    }

    /// Retire l'image clé de caméra de l'image courante.
    pub(crate) fn remove_camera_key_here(&mut self) {
        let frame = self.cur_frame;
        let Some(i) = self.doc.panels[self.cur_panel].camera.iter().position(|k| k.frame == frame) else {
            self.status = "Pas d'image clé de caméra sur cette image.".into();
            return;
        };
        self.record_step();
        self.doc.panels[self.cur_panel].camera.remove(i);
        self.status = "Image clé de caméra retirée.".into();
    }

    /// Image clé de caméra de l'image courante, s'il y en a une.
    pub(crate) fn camera_key_here(&self) -> Option<&camera::CameraKey> {
        self.doc.panels[self.cur_panel].camera.iter().find(|k| k.frame == self.cur_frame)
    }

    // (Les clés de caméra se choisissent, se glissent, se copient… dans `camkeys.rs`.)

    /// Adoucit (ou non) le mouvement à la clé de l'image courante.
    pub(crate) fn toggle_camera_key_smooth(&mut self) {
        let frame = self.cur_frame;
        if self.camera_key_here().is_none() {
            self.status = "Place-toi sur une image clé de caméra pour l'adoucir.".into();
            return;
        }
        self.record_step();
        if let Some(k) = self.doc.panels[self.cur_panel].camera.iter_mut().find(|k| k.frame == frame) {
            k.smooth = !k.smooth;
        }
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
        // Centre du cadre filmé (caméra tournée : le point visé, pas l'aplomb de l'œil).
        let [fw, fh] = self.doc.frame;
        let c = cam0.to_stage(self.doc.frame, fw as f32 / 2.0, fh as f32 / 2.0);
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
        // Caméra tournée, sur la feuille : le cadre filmé (un trapèze) peut déborder de la
        // feuille ; on estompe la feuille moins le cadre coupé à la feuille (9f).
        if alpha > 0 && !self.camera_view && !cam.is_level() {
            let outer = [stage.left_top(), stage.right_top(), stage.right_bottom(), stage.left_bottom()].map(to_s);
            let inner: Vec<Pos2> = crate::viewbar::clip_to_rect(&corners, stage).into_iter().map(to_s).collect();
            painter.add(crate::viewbar::nested_band(&outer, &inner, Color32::from_black_alpha(alpha)));
        } else if alpha > 0 {
            let outer: [Pos2; 4] = if self.camera_view {
                [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]
            } else {
                [stage.left_top(), stage.right_top(), stage.right_bottom(), stage.left_bottom()].map(to_s)
            };
            let inner = corners.map(to_s);
            let mut mesh = egui::Mesh::default();
            // En vue caméra, rien ne doit transparaître hors du cadre : couleur du fond, opaque.
            let color = if self.camera_view {
                crate::theme::fond()
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
            // Centre du cadre (caméra tournée : le point visé).
            let [fw, fh] = self.doc.frame;
            let c = to_s(cam.to_stage(self.doc.frame, fw as f32 / 2.0, fh as f32 / 2.0));
            painter.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(1.0_f32, CAMERA));
            painter.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], Stroke::new(1.0_f32, CAMERA));
        }
    }

    /// Boutons des images clés de caméra, compacts (barre de la timeline, 12c) : clés début / ici /
    /// fin, retirer, adoucir ; le détail est dans la fenêtre de la caméra (outil Caméra).
    pub(crate) fn camera_keys_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, etat_si, TAILLE};
        use egui_phosphor::regular as ph;
        let here = self.camera_key_here().copied();
        let resp = ui.label(RichText::new(ph::VIDEO_CAMERA).size(TAILLE).color(crate::theme::camera()));
        bulle(resp, "Images clés de la caméra", "Position, focale et adoucis en détail : réglages de l'outil Caméra.", "");
        if let Some(a) = self.camera_key_buttons(ui, here.is_some()) {
            self.run_action(a);
        }
        if self.bouton_si(ui, here.is_some(), ph::X_CIRCLE, "Retirer la clé", "Retirer l'image clé de cette image.", "camera.cle_supprimer").clicked() {
            self.run_action("camera.cle_supprimer");
        }
        let smooth = here.is_some_and(|k| k.smooth);
        let resp = etat_si(
            ui,
            here.is_some(),
            smooth,
            ph::WAVE_SINE,
            "Adoucir",
            "La caméra arrive à cette clé et en repart en douceur ; réglage fin en images dans les réglages de l'outil Caméra.",
            "",
        );
        if resp.clicked() {
            self.toggle_camera_key_smooth();
        }
    }

    /// Boutons « clé au début », « clé ici », « clé à la fin » (« ici » grisé s'il y a déjà une
    /// clé à cette image) ; renvoie l'action du bouton cliqué.
    fn camera_key_buttons(&self, ui: &mut egui::Ui, key_here: bool) -> Option<&'static str> {
        use egui_phosphor::regular as ph;
        let buttons = [
            (format!("|{}", ph::DIAMOND), "Clé au début", "Image clé de caméra sur la première image du plan.", "camera.cle_debut", true),
            (ph::DIAMOND.to_string(), "Clé ici", "Image clé de caméra à l'image courante.", "camera.cle_ici", !key_here),
            (format!("{}|", ph::DIAMOND), "Clé à la fin", "Image clé de caméra sur la dernière image du plan.", "camera.cle_fin", true),
        ];
        let mut clicked = None;
        for (icon, name, description, action, enabled) in buttons {
            if self.bouton_si(ui, enabled, &icon, name, description, action).clicked() {
                clicked = Some(action);
            }
        }
        clicked
    }

    /// Paramètres de la caméra (outil Caméra, 9c), sur le modèle d'une fenêtre de propriétés de
    /// caméra : images clés et adoucis, position (X, Y, Z), objectif et rotation, durées.
    pub(crate) fn camera_panel(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
            egui::Frame::group(ui.style()).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(title).small().weak());
                body(ui);
            });
        };
        let fps = self.doc.fps;
        let here = self.camera_key_here().copied();
        let keys: Vec<usize> = self.doc.panels[self.cur_panel].camera.iter().map(|k| k.frame).collect();

        // Images clés : poser, aller de l'une à l'autre, retirer ; adoucis de la clé courante.
        let mut action = None;
        let mut goto = None;
        let mut eases: Option<(f32, f32)> = None;
        section(ui, "Images clés", &mut |ui| {
            ui.horizontal_wrapped(|ui| {
                action = self.camera_key_buttons(ui, here.is_some());
                let prev = keys.iter().rev().find(|&&f| f < self.cur_frame).copied();
                let next = keys.iter().find(|&&f| f > self.cur_frame).copied();
                if crate::bulles::icone_si(ui, prev.is_some(), ph::CARET_LEFT, "Clé précédente", "Aller à l'image clé d'avant.", "").clicked() {
                    goto = prev;
                }
                if crate::bulles::icone_si(ui, next.is_some(), ph::CARET_RIGHT, "Clé suivante", "Aller à l'image clé d'après.", "").clicked() {
                    goto = next;
                }
                if self.bouton_si(ui, here.is_some(), ph::TRASH, "Retirer la clé", "Retirer l'image clé de cette image.", "camera.cle_supprimer").clicked() {
                    action = Some("camera.cle_supprimer");
                }
            });
            ui.add_enabled_ui(here.is_some(), |ui| {
                let mut values = here.map_or([0.0, 0.0], |k| [k.ease_in, k.ease_out]);
                let mut changed = false;
                egui::Grid::new("adoucis_camera").num_columns(3).show(ui, |ui| {
                    let rows = [
                        ("Adoucir l'entrée", "La caméra ralentit en arrivant sur cette clé, sur ce nombre d'images."),
                        ("Adoucir la sortie", "La caméra repart doucement de cette clé, sur ce nombre d'images."),
                    ];
                    for (k, (label, hint)) in rows.into_iter().enumerate() {
                        ui.label(label);
                        let r = ui
                            .add(egui::DragValue::new(&mut values[k]).range(0.0..=10_000.0).speed(0.2).max_decimals(0).suffix(" i"))
                            .on_hover_text(hint);
                        ui.label(RichText::new(crate::storyboard::timecode(values[k].round() as usize, fps)).small().weak());
                        changed |= self.edited(&r);
                        ui.end_row();
                    }
                });
                if changed {
                    eases = Some((values[0], values[1]));
                }
            });
            if here.is_none() {
                ui.label(RichText::new("Place-toi sur une image clé pour régler ses adoucis.").small().weak());
            }
        });
        if let Some((ein, eout)) = eases {
            let frame = self.cur_frame;
            if let Some(k) = self.doc.panels[self.cur_panel].camera.iter_mut().find(|k| k.frame == frame) {
                (k.ease_in, k.ease_out) = (ein.max(0.0), eout.max(0.0));
                // Les nouveaux adoucis remplacent l'ancien « adoucir ».
                k.smooth = false;
            }
        }
        if let Some(a) = action {
            self.run_action(a);
        }
        if let Some(f) = goto {
            self.cur_frame = f;
        }

        // Position, objectif, rotation : la caméra à l'image courante.
        let cam = self.current_camera();
        let frame = self.doc.frame;
        let (cx, cy) = (self.doc.width as f32 / 2.0, self.doc.height as f32 / 2.0);
        let d0 = camera::ref_distance(frame);
        let (mut x, mut y, mut z, mut focal, mut angle) = (cam.x - cx, cam.y - cy, cam.distance(frame), cam.focal(), cam.angle);
        let (mut tilt, mut pan) = (cam.tilt, cam.pan);
        let mut next = None;
        section(ui, "Caméra à cette image", &mut |ui| {
            egui::Grid::new("caméra_position").num_columns(2).show(ui, |ui| {
                ui.label("Position");
                ui.horizontal(|ui| {
                    let r = ui.add(egui::DragValue::new(&mut x).speed(2.0).prefix("X ").max_decimals(0)).on_hover_text("Décalage horizontal depuis le centre de la feuille (px)");
                    if self.edited(&r) {
                        next = Some(Camera { x: cx + x, ..cam });
                    }
                    let r = ui.add(egui::DragValue::new(&mut y).speed(2.0).prefix("Y ").max_decimals(0)).on_hover_text("Décalage vertical depuis le centre de la feuille (px)");
                    if self.edited(&r) {
                        next = Some(Camera { y: cy + y, ..cam });
                    }
                    let r = ui
                        .add(egui::DragValue::new(&mut z).speed(4.0).range(d0 * 0.05..=d0 * 20.0).prefix("Z ").max_decimals(0))
                        .on_hover_text(
                            "Distance de la caméra au plan de référence (px). Moins : la caméra avance \
                             (travelling) et les calques proches grossissent plus que les lointains.",
                        );
                    if self.edited(&r) {
                        next = Some(Camera { dolly: d0 - z, ..cam });
                    }
                });
                ui.end_row();
                ui.label("Focale");
                ui.horizontal(|ui| {
                    let r = ui
                        .add(egui::DragValue::new(&mut focal).speed(0.2).range(2.0..=1000.0).suffix(" mm").max_decimals(1))
                        .on_hover_text(format!("Objectif (zoom) : {:.0} mm = 100 %.", camera::REF_FOCAL));
                    if self.edited(&r) {
                        next = Some(Camera { zoom: (focal / camera::REF_FOCAL).clamp(0.05, 50.0), ..cam });
                    }
                    self.focal_presets_ui(ui, focal);
                });
                ui.end_row();
                ui.label("");
                ui.label(
                    RichText::new(format!("champ {:.0}° · grossissement {:.0} %", camera::angle_of_view(focal), cam.magnification(frame) * 100.0))
                        .small()
                        .weak(),
                )
                .on_hover_text("Angle de champ horizontal d'un capteur de 36 mm de large, et grossissement du plan de référence (focale et avancée).");
                ui.end_row();
                ui.label("Rotation");
                ui.horizontal(|ui| {
                    let r = ui
                        .add(egui::DragValue::new(&mut angle).speed(0.5).suffix("°").max_decimals(1))
                        .on_hover_text("Roulis : le cadre tourne autour de l'axe de visée (sens horaire à l'écran)");
                    if self.edited(&r) {
                        next = Some(Camera { angle, ..cam });
                    }
                    let resp = self.bouton(ui, ph::ARROW_COUNTER_CLOCKWISE, "Réinitialiser la caméra", "Au centre, sans zoom, avancée ni rotation.", "camera.reinitialiser");
                    if resp.clicked() {
                        self.reset_camera();
                    }
                });
                ui.end_row();
                // Bascule et panoramique (9f) : la caméra tourne sur elle-même, sans bouger.
                ui.label("Bascule");
                ui.horizontal(|ui| {
                    let r = ui
                        .add(egui::DragValue::new(&mut tilt).speed(0.2).range(-85.0..=85.0).suffix("°").max_decimals(1))
                        .on_hover_text("Rotation autour de l'axe horizontal (X) : positive, la caméra regarde vers le haut");
                    if self.edited(&r) {
                        next = Some(Camera { tilt, ..cam });
                    }
                    ui.label("Panoramique");
                    let r = ui
                        .add(egui::DragValue::new(&mut pan).speed(0.2).range(-85.0..=85.0).suffix("°").max_decimals(1))
                        .on_hover_text("Rotation autour de l'axe vertical (Y) : positif, la caméra regarde vers la droite");
                    if self.edited(&r) {
                        next = Some(Camera { pan, ..cam });
                    }
                });
                ui.end_row();
            });
            if !cam.is_level() {
                ui.label(
                    RichText::new("Caméra tournée : le cadre filmé sur la feuille est un trapèze (vue caméra en perspective).")
                        .small()
                        .weak(),
                );
            }
        });
        if let Some(c) = next {
            self.set_current_camera(c);
        }

        // Effets de caméra : des préréglages qui posent des clés (`camfxui.rs`).
        section(ui, "Effets de caméra", &mut |ui| self.camera_effects_ui(ui));

        // Vue et durées.
        section(ui, "Vue", &mut |ui| {
            ui.horizontal(|ui| {
                let on = self.camera_view;
                if self.bouton_etat(ui, on, ph::FRAME_CORNERS, "Vue à travers la caméra", "Voir ce que filme la caméra du plan.", "camera.vue").clicked() {
                    self.set_camera_view(!on);
                }
                let key = self.raccourci("camera.masque");
                crate::bulles::interrupteur_a(ui, &mut self.camera_mask, ph::SELECTION_INVERSE, "Estomper le hors-champ", "Voiler ce qui est hors du cadre de la caméra.", &key, "camera.masque");
            });
        });
        // (Les aides au cadrage sont dans le menu « Cadrage » de la barre au-dessus du dessin.)
        let (s, e) = self.doc.sequence_run(self.cur_panel);
        let sequence_len: usize = self.doc.panels[s..e].iter().map(|p| p.frame_count()).sum();
        section(ui, "Durées", &mut |ui| {
            ui.label(format!("Séquence : {}", crate::storyboard::duration_label(sequence_len, fps)));
            ui.label(format!("Plan : {}", crate::storyboard::duration_label(self.frame_count(), fps)));
        });
        ui.label(
            RichText::new(
                "Glisser dans le cadre : déplacer (Maj : à l'horizontale ou à la verticale). Coin : zoomer. \
                 Poignée ronde : tourner (Maj : par 15°). Un plan sans clé : caméra au centre, cadre entier. \
                 Losanges jaunes de la piste Caméra : clic pour choisir (Ctrl : ajouter, Maj : la suite), glisser pour \
                 déplacer, glisser dans le vide de la piste pour en choisir plusieurs au rectangle (Maj : ajouter, Ctrl : \
                 basculer), Ctrl+C / Ctrl+V à l'image courante, Suppr. Aides au cadrage : menu « Cadrage » au-dessus du dessin.",
            )
            .small()
            .weak(),
        );
    }

    /// Réglages de la caméra du plan courant (fiche du panneau et outil Caméra).
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
            let resp = self.bouton(
                ui,
                egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
                "Réinitialiser la caméra",
                "Au centre, sans zoom, avancée ni rotation.",
                "camera.reinitialiser",
            );
            if resp.clicked() {
                self.reset_camera();
            }
        });
        if let Some(c) = next {
            self.set_current_camera(c);
        }
    }
}
