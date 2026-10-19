//! La vue d'édition de la scène 3D : navigation (tourner, glisser, avancer), choix des objets à la
//! souris, manipulateurs (déplacer, tourner, échelle), caméra du plateau, tracé. Les formes sont
//! dessinées par la carte graphique (`scene3dgpu.rs`) ; les poignées, la boîte de sélection, la
//! caméra et les légendes par egui, par-dessus.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::scene3d::{SceneCam, Shape3d, Target};
use crate::scene3dgizmo::{self as gz, Gizmo, Handle, Mode as GizmoMode, Originals, CAMERA_ID};
use crate::scene3dgpu::{DrawObject, Frame, GridLine, Gpu3d};
use crate::scene3dmath::{self as m, Ray, V3, View};
use crate::scene3dmesh;
use crate::scene3dui::Drag;
use crate::view3d::Cam3d;

const ACCENT: Color32 = crate::theme::POIGNEE_SELECTION;
const AXIS_COLORS: [Color32; 3] = [Color32::from_rgb(232, 80, 80), Color32::from_rgb(100, 205, 100), Color32::from_rgb(86, 138, 245)];
const HOT: Color32 = Color32::from_rgb(255, 214, 70);
/// Distance (pixels d'écran) à laquelle on attrape une poignée.
const GRAB: f32 = 9.0;
/// Geste « déplacer la caméra du plateau » dans la vue par la caméra (une seule étape d'annulation).
const GESTURE_CAMERA: u64 = 0x5c3d_0001;

/// La vue d'une caméra d'orbite, pour une zone de `width` pixels.
pub(crate) fn orbit_view(cam: &Cam3d, width: f32) -> View {
    let (right, up, forward) = cam.basis();
    View { eye: cam.eye(), right, up, forward, focal: cam.focal, ref_width: width }
}

/// Le plus grand rectangle de proportions `frame` dans `rect`, centré (avec une petite marge).
pub(crate) fn letterbox(rect: Rect, frame: [usize; 2]) -> Rect {
    let aspect = frame[0].max(1) as f32 / frame[1].max(1) as f32;
    let avail = rect.shrink(8.0);
    let (w, h) = if avail.width() / avail.height().max(1.0) > aspect { (avail.height() * aspect, avail.height()) } else { (avail.width(), avail.width() / aspect) };
    Rect::from_center_size(rect.center(), vec2(w.max(1.0), h.max(1.0)))
}

/// Un pas rond (1, 2 ou 5 × 10ⁿ) proche de `span` / 12.
pub(crate) fn nice_step(span: f32) -> f32 {
    let raw = (span / 12.0).max(1e-3);
    let p = 10f32.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|k| k * p).find(|s| *s >= raw).unwrap_or(10.0 * p)
}

/// Les traits du quadrillage du sol autour de `target`, à l'échelle de ce qu'on regarde.
pub(crate) fn grid_lines(target: V3, distance: f32) -> Vec<GridLine> {
    let step = nice_step((distance * 2.4).clamp(1.0, 400.0));
    let n = 15i32;
    let (cx, cz) = ((target[0] / step).round() * step, (target[2] / step).round() * step);
    let half = n as f32 * step;
    let mut out = Vec::with_capacity(((2 * n + 1) * 2) as usize);
    for i in -n..=n {
        let (x, z) = (cx + i as f32 * step, cz + i as f32 * step);
        let major = ((x / step).round() as i64) % 5 == 0;
        let minor = [0.7, 0.7, 0.72, if major { 0.32 } else { 0.14 }];
        let line_x = if z.abs() < step * 0.01 { [0.9, 0.32, 0.32, 0.75] } else { minor };
        let line_z = if x.abs() < step * 0.01 { [0.35, 0.5, 0.95, 0.75] } else { minor };
        out.push(GridLine { a: [cx - half, 0.0, z], b: [cx + half, 0.0, z], color: line_x });
        out.push(GridLine { a: [x, 0.0, cz - half], b: [x, 0.0, cz + half], color: line_z });
    }
    out
}

impl App {
    /// La vue d'édition, ou la vue de la caméra du plateau.
    fn s3_view(&self, draw: Rect) -> View {
        let (panel, _) = self.s3_ids();
        if self.scene3d.through_camera {
            self.doc.scenes3d.plan(panel).map_or_else(SceneCam::default, |p| p.cam).view(draw.width())
        } else {
            orbit_view(&self.scene3d.cam.unwrap_or(Cam3d { target: [0.0; 3], azimuth: 30.0, elevation: 22.0, distance: 8.0, focal: 35.0 }), draw.width())
        }
    }

    /// Le manipulateur de la sélection.
    pub(crate) fn s3_gizmo(&self, view: &View) -> Option<Gizmo> {
        let mode = self.scene3d.mode;
        if mode == GizmoMode::Select || self.scene3d.sel.is_empty() {
            return None;
        }
        let xfs = self.s3_xfs(&self.scene3d.sel);
        let first = xfs.first()?;
        let origin = gz::pivot_of(&xfs);
        // (L'échelle se fait sur les axes de l'objet : ses cubes suivent donc toujours l'objet.)
        let local = (self.scene3d.local || mode == GizmoMode::Scale) && xfs.len() == 1;
        Some(Gizmo { origin, axes: gz::axes_for(local, &first.1), len: Gizmo::size_for(view, origin), mode })
    }

    /// L'objet (modifiable) touché par le rayon, le plus proche ; un plan (le sol) ne passe qu'après
    /// les autres, pour qu'on puisse l'ignorer en visant à côté.
    pub(crate) fn s3_pick(&mut self, ray: &Ray) -> Option<u64> {
        let (panel, seq) = self.s3_ids();
        let target = self.scene3d.target;
        let mut best: Option<(f32, u64)> = None;
        let mut floor: Option<(f32, u64)> = None;
        let objects: Vec<(u64, Shape3d, m::M4, bool)> =
            self.doc.scenes3d.list(panel, seq, target).iter().filter(|o| !o.hidden && !o.locked).map(|o| (o.id, o.shape.clone(), o.xf.matrix(), matches!(o.shape, Shape3d::Plane))).collect();
        for (id, shape, matrix, is_plane) in objects {
            let (_, mesh) = self.scene3d.meshes.get_for_view(&shape, &self.doc.models);
            let inv = m::inverse_affine(&matrix);
            let (o, d) = ray.to_local(&inv);
            let (lo, hi) = mesh.bounds;
            let pad = 0.01;
            let Some((t_box, _)) = m::ray_box(o, d, m::sub(lo, [pad; 3]), m::add(hi, [pad; 3])) else { continue };
            let mut hit: Option<f32> = None;
            for t in mesh.indices.chunks_exact(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
                if let Some(h) = m::ray_triangle(o, d, a, b, c) {
                    hit = Some(hit.map_or(h, |x: f32| x.min(h)));
                }
            }
            // Une boîte seule (modèle trop gros) se prend par sa boîte.
            let hit = hit.or_else(|| (mesh.triangle_count() == 12 && matches!(shape, Shape3d::Model { .. })).then_some(t_box.max(0.0)));
            if let Some(t) = hit {
                let slot = if is_plane { &mut floor } else { &mut best };
                if slot.is_none_or(|(b, _)| t < b) {
                    *slot = Some((t, id));
                }
            }
        }
        best.or(floor).map(|b| b.1)
    }

    /// La caméra du plateau est touchée par le pixel (près de son corps ou de sa pyramide).
    fn s3_pick_camera(&self, view: &View, px: [f32; 2], center: [f32; 2]) -> bool {
        let (panel, _) = self.s3_ids();
        let Some(plan) = self.doc.scenes3d.plan(panel) else { return false };
        let corners = self.s3_camera_corners(&plan.cam, view);
        let near = |a: V3, b: V3| match (view.project(a, center), view.project(b, center)) {
            (Some(p), Some(q)) => seg_dist(px, p, q) < 7.0,
            _ => false,
        };
        let pos = plan.cam.pos;
        (0..4).any(|k| near(pos, corners[k]) || near(corners[k], corners[(k + 1) % 4]))
    }

    /// Les coins de la pyramide de la caméra du plateau, à une distance qui se lit dans la vue.
    fn s3_camera_corners(&self, cam: &SceneCam, view: &View) -> [V3; 4] {
        let d = (m::length(m::sub(cam.pos, view.eye)) * 0.22).clamp(0.5, 6.0);
        cam.corners_at(d, self.doc.frame)
    }

    /// La souris dans la zone de dessin 3D.
    fn s3_input(&mut self, ctx: &egui::Context, draw: Rect, response: &egui::Response) {
        let (panel, _) = self.s3_ids();
        let (scroll, mods, press, primary_down) = ctx.input(|i| (i.smooth_scroll_delta, i.modifiers, i.pointer.press_origin(), i.pointer.primary_down()));
        let Some(mut cam) = self.scene3d.cam else { return };
        let through = self.scene3d.through_camera;
        let delta = response.drag_delta();
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        let rel = |p: Pos2| [p.x - draw.left(), p.y - draw.top()];

        // Navigation. Par la caméra du plateau, elle déplace la caméra elle-même.
        let moving_camera = through && response.hovered() && (scroll.y != 0.0 || response.dragged_by(PointerButton::Middle) || response.dragged_by(PointerButton::Secondary));
        if moving_camera {
            self.gesture_change(GESTURE_CAMERA, |s| {
                let snap = s.snapshot();
                let before = s.history.context.replace("Déplacer la caméra 3D".to_string());
                s.history.push(snap);
                s.history.context = before;
            });
            self.keep_gesture(GESTURE_CAMERA);
            let mut sc = self.doc.scenes3d.plan(panel).map_or_else(SceneCam::default, |p| p.cam);
            let view = sc.view(draw.width());
            if scroll.y != 0.0 {
                sc.pos = m::add(sc.pos, m::scale(view.forward, scroll.y * 0.01));
            }
            if response.dragged_by(PointerButton::Middle) {
                let k = 0.004 * m::length(m::sub(sc.pos, [0.0, 1.0, 0.0])).clamp(1.0, 20.0);
                sc.pos = m::add(sc.pos, m::add(m::scale(view.right, -delta.x * k), m::scale(view.up, delta.y * k)));
            }
            if response.dragged_by(PointerButton::Secondary) {
                sc.pan += delta.x * 0.15;
                sc.tilt = (sc.tilt - delta.y * 0.15).clamp(-crate::scene3d::MAX_TILT, crate::scene3d::MAX_TILT);
            }
            self.doc.scenes3d.plan_mut(panel).cam = sc.sanitized();
            self.doc.scenes3d.touch();
        } else if !through {
            if response.hovered() && scroll.y != 0.0 {
                cam.dolly((-scroll.y * 0.002).exp());
            }
            if response.dragged_by(PointerButton::Middle) {
                cam.pan(delta.x, delta.y, draw.width());
            }
            if response.dragged_by(PointerButton::Secondary) {
                cam.orbit(-delta.x * 0.35, delta.y * 0.35);
            }
        }

        let view = if through { self.s3_view(draw) } else { orbit_view(&cam, draw.width()) };
        if response.drag_started_by(PointerButton::Primary) {
            let at = press.or(response.interact_pointer_pos()).unwrap_or(draw.center());
            self.s3_press(&view, center, rel(at), mods);
        }
        if response.dragged_by(PointerButton::Primary) {
            let orbiting = matches!(self.scene3d.drag, Some(Drag::Orbit));
            let handling = matches!(self.scene3d.drag, Some(Drag::Handle { .. }));
            if orbiting && !through {
                cam.orbit(-delta.x * 0.35, delta.y * 0.35);
            }
            if handling {
                if let Some(p) = response.interact_pointer_pos() {
                    self.s3_drag(&view, center, rel(p));
                }
            }
        }
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                self.s3_click(&view, center, rel(p), mods);
            }
        }
        if !primary_down {
            self.s3_finish_drag();
        }
        // Survol d'une poignée (pour la mettre en évidence).
        self.scene3d.hover = response.hover_pos().filter(|_| self.scene3d.drag.is_none()).and_then(|p| self.s3_gizmo(&view)?.hit(&view, center, rel(p), GRAB));
        if !through {
            self.scene3d.cam = Some(cam);
        }
    }

    /// Un clic sans glisser : choisir l'objet (ou la caméra) sous la souris, ou ne plus rien choisir.
    fn s3_click(&mut self, view: &View, center: [f32; 2], px: [f32; 2], mods: egui::Modifiers) {
        let (panel, seq) = self.s3_ids();
        let ray = view.ray(px, center);
        let additive = mods.shift || mods.command;
        if self.s3_gizmo(view).is_some_and(|g| g.hit(view, center, px, GRAB).is_some()) {
            return;
        }
        let hit = self.s3_pick(&ray);
        let cam_hit = hit.is_none() && !self.scene3d.through_camera && self.s3_pick_camera(view, px, center);
        match (hit, cam_hit) {
            (Some(id), _) => {
                let ids = self.doc.scenes3d.expand_groups(panel, seq, &[id]);
                if additive {
                    for i in ids {
                        match self.scene3d.sel.iter().position(|s| *s == i) {
                            Some(p) => {
                                self.scene3d.sel.remove(p);
                            }
                            None => self.scene3d.sel.push(i),
                        }
                    }
                } else {
                    self.scene3d.sel = ids;
                }
            }
            (None, true) => {
                if additive {
                    self.scene3d.sel.push(CAMERA_ID);
                } else {
                    self.scene3d.sel = vec![CAMERA_ID];
                }
            }
            (None, false) => {
                if !additive {
                    self.scene3d.sel.clear();
                }
            }
        }
    }

    /// Début d'un glisser : une poignée, un objet qu'on tire directement, ou la vue qu'on tourne.
    fn s3_press(&mut self, view: &View, center: [f32; 2], px: [f32; 2], mods: egui::Modifiers) {
        self.end_stroke();
        let ray = view.ray(px, center);
        if mods.alt {
            self.scene3d.drag = Some(Drag::Orbit);
            return;
        }
        // Une poignée du manipulateur.
        if let Some(gizmo) = self.s3_gizmo(view) {
            if let Some(mut handle) = gizmo.hit(view, center, px, GRAB) {
                // Plusieurs objets : échelle uniforme seulement.
                if gizmo.mode == GizmoMode::Scale && (self.scene3d.sel.len() > 1 || self.scene3d.sel.contains(&CAMERA_ID)) {
                    handle = Handle::Center;
                }
                if gizmo.mode == GizmoMode::Scale && self.scene3d.sel.contains(&CAMERA_ID) {
                    self.scene3d.drag = Some(Drag::Orbit);
                    return;
                }
                let name = match gizmo.mode {
                    GizmoMode::Rotate => "Tourner dans la scène 3D",
                    GizmoMode::Scale => "Mettre à l'échelle dans la scène 3D",
                    _ => "Déplacer dans la scène 3D",
                };
                self.s3_begin(handle, gizmo.mode, gizmo, ray, px, name);
                return;
            }
        }
        // Un objet tiré directement : il glisse sur le sol (Maj : en hauteur).
        let (panel, seq) = self.s3_ids();
        let hit = self.s3_pick(&ray);
        let cam_hit = hit.is_none() && !self.scene3d.through_camera && self.s3_pick_camera(view, px, center);
        if hit.is_some() || cam_hit {
            let id = hit.unwrap_or(CAMERA_ID);
            if !self.scene3d.sel.contains(&id) {
                self.scene3d.sel = if id == CAMERA_ID { vec![id] } else { self.doc.scenes3d.expand_groups(panel, seq, &[id]) };
            }
            let xfs = self.s3_xfs(&self.scene3d.sel);
            let origin = gz::pivot_of(&xfs);
            let gizmo = Gizmo { origin, axes: gz::axes_for(false, &xfs.first().map_or_else(Default::default, |x| x.1)), len: 1.0, mode: GizmoMode::Move };
            let handle = if mods.shift { Handle::Axis(1) } else { Handle::Plane(1) };
            let name = if id == CAMERA_ID { "Déplacer la caméra 3D" } else { "Déplacer dans la scène 3D" };
            self.s3_begin(handle, GizmoMode::Move, gizmo, ray, px, name);
            return;
        }
        if !self.scene3d.through_camera {
            self.scene3d.drag = Some(Drag::Orbit);
        }
    }

    pub(crate) fn s3_begin(&mut self, handle: Handle, mode: GizmoMode, gizmo: Gizmo, start: Ray, px: [f32; 2], name: &'static str) {
        let originals = self.s3_xfs(&self.scene3d.sel);
        let snap = self.snapshot();
        self.scene3d.drag = Some(Drag::Handle { handle, mode, gizmo, start, start_px: px, originals, snap, changed: false, name });
    }

    /// Le geste suit la souris : nouveaux placements, aimantés si l'aimantation est allumée.
    pub(crate) fn s3_drag(&mut self, view: &View, center: [f32; 2], px: [f32; 2]) {
        let ray = view.ray(px, center);
        let Some(Drag::Handle { handle, mode, gizmo, start, start_px, originals, .. }) = &self.scene3d.drag else { return };
        let (handle, mode, gizmo, start, start_px, originals) = (*handle, *mode, *gizmo, *start, *start_px, originals.clone());
        let snap_on = self.scene3d.snap;
        let (step_move, step_rot, step_scale) = (self.scene3d.step_move, self.scene3d.step_rot, self.scene3d.step_scale);
        let world_axes = gizmo.axes == [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut message = String::new();
        let mut new: Originals = match (mode, handle) {
            (GizmoMode::Move, Handle::Axis(k)) => gz::move_xfs(&originals, m::drag_along_axis(&start, &ray, gizmo.origin, gizmo.axes[k])),
            (GizmoMode::Move, Handle::Plane(k)) => gz::move_xfs(&originals, m::drag_in_plane(&start, &ray, gizmo.origin, gizmo.axes[k])),
            (GizmoMode::Rotate, Handle::Ring(k)) => {
                let mut angle = m::drag_angle(&start, &ray, gizmo.origin, gizmo.axes[k]);
                if snap_on {
                    angle = m::snap(angle, step_rot);
                }
                message = format!("Rotation {angle:+.0}°");
                gz::rotate_xfs(&originals, gizmo.origin, gizmo.axes[k], angle)
            }
            (GizmoMode::Scale, Handle::Axis(k)) => {
                let factor = m::drag_scale_axis(&start, &ray, gizmo.origin, gizmo.axes[k]);
                let mut out = gz::scale_xfs(&originals, gizmo.origin, Some(k), factor);
                if snap_on {
                    for (_, xf) in &mut out {
                        let s = m::snap(xf.size[k].abs(), step_scale).max(0.01);
                        xf.size[k] = s * xf.size[k].signum();
                    }
                }
                message = format!("Taille ×{factor:.2}");
                out
            }
            (GizmoMode::Scale, _) => {
                let factor = m::scale_from_screen(px[1] - start_px[1]);
                message = format!("Taille ×{factor:.2}");
                gz::scale_xfs(&originals, gizmo.origin, None, factor)
            }
            _ => return,
        };
        if mode == GizmoMode::Move {
            // Aimantation : le premier objet tombe sur la grille (sur les axes que le geste a bougés),
            // les autres suivent le même décalage ; puis le sol.
            if snap_on && world_axes {
                let moved = match handle {
                    Handle::Axis(k) => [k == 0, k == 1, k == 2],
                    Handle::Plane(k) => [k != 0, k != 1, k != 2],
                    _ => [false; 3],
                };
                if let Some(first) = new.first().map(|n| n.1.pos) {
                    let snapped = m::snap_position(first, step_move, moved);
                    let fix = m::sub(snapped, first);
                    for (_, xf) in &mut new {
                        xf.pos = m::add(xf.pos, fix);
                    }
                }
            }
            let lifts = matches!(handle, Handle::Axis(1) | Handle::Plane(0) | Handle::Plane(2));
            if self.scene3d.snap_floor && lifts {
                if let Some(dy) = self.s3_floor_offset(&new) {
                    for (_, xf) in &mut new {
                        xf.pos[1] += dy;
                    }
                }
            }
            if let Some((_, xf)) = new.first() {
                message = format!("Position {:.2} · {:.2} · {:.2} m", xf.pos[0], xf.pos[1], xf.pos[2]);
            }
        }
        self.s3_set_xfs(&new);
        if !message.is_empty() {
            self.status = message;
        }
        if let Some(Drag::Handle { changed, .. }) = &mut self.scene3d.drag {
            *changed = true;
        }
    }

    /// De combien descendre ou monter la sélection pour que son bas touche le sol, si elle en est
    /// proche.
    fn s3_floor_offset(&mut self, xfs: &Originals) -> Option<f32> {
        let (panel, seq) = self.s3_ids();
        let mut lowest = f32::MAX;
        for (id, xf) in xfs {
            if *id == CAMERA_ID {
                continue;
            }
            let shape = self.doc.scenes3d.find(panel, seq, *id)?.0.shape.clone();
            let (_, mesh) = self.scene3d.meshes.get_for_view(&shape, &self.doc.models);
            lowest = lowest.min(scene3dmesh::world_bounds(&mesh, &xf.matrix()).0[1]);
        }
        (lowest < f32::MAX).then(|| m::floor_snap(lowest, 0.15)).flatten()
    }

    /// Fin du geste : l'étape d'annulation, si quelque chose a bougé.
    pub(crate) fn s3_finish_drag(&mut self) {
        if let Some(Drag::Handle { snap, changed: true, name, .. }) = self.scene3d.drag.take() {
            let before = self.history.context.replace(name.to_string());
            self.history.push(snap);
            self.history.context = before;
            self.modified = true;
        } else {
            self.scene3d.drag = None;
        }
    }

    /// Les objets à dessiner dans la vue d'édition.
    fn s3_draw_objects(&mut self, panel: u64, seq: u64) -> Vec<DrawObject> {
        let target = self.scene3d.target;
        let scenes = &self.doc.scenes3d;
        let mut shown = scenes.visible_objects(panel, seq);
        // Modifier le décor d'un plan qui ne le montre pas : on le montre quand même, le temps de le modifier.
        if target == Target::Decor && scenes.plan(panel).is_some_and(|p| !p.use_decor) {
            let decor = scenes.list(panel, seq, Target::Decor).iter().map(|o| (o, true));
            shown = decor.chain(shown).collect();
        }
        let objects: Vec<(u64, Shape3d, m::M4, [u8; 3], bool, bool)> = shown
            .into_iter()
            .filter(|(o, _)| !o.hidden)
            .map(|(o, decor)| (o.id, o.shape.clone(), o.xf.matrix(), o.color, o.gray, decor != (target == Target::Decor)))
            .collect();
        objects
            .into_iter()
            .map(|(id, shape, matrix, color, gray, ghost)| {
                let (key, mesh) = self.scene3d.meshes.get_for_view(&shape, &self.doc.models);
                DrawObject { key, mesh, matrix, normal_matrix: m::normal_matrix(&matrix), color: color.map(|c| c as f32 / 255.0), gray, selected: self.scene3d.sel.contains(&id), ghost }
            })
            .collect()
    }

    /// La zone du dessin, en mode Scène 3D.
    pub(crate) fn scene3d_panel(&mut self, ui: &mut egui::Ui, rect: Rect, response: &egui::Response) {
        let (panel, seq) = self.s3_ids();
        if self.scene3d.cam.is_none() {
            self.scene3d_fit();
        }
        let through = self.scene3d.through_camera;
        let draw = if through { letterbox(rect, self.doc.frame) } else { rect };
        self.s3_input(ui.ctx(), draw, response);
        let plan = self.doc.scenes3d.plan(panel).cloned().unwrap_or_default();
        let view = self.s3_view(draw);
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        let painter = ui.painter_at(rect);
        let to_screen = |q: [f32; 2]| pos2(draw.left() + q[0], draw.top() + q[1]);
        let proj = |p: V3| view.project(p, center).map(to_screen);

        painter.rect_filled(rect, 0.0, Color32::from_gray(26));
        // La carte graphique dessine les formes ; sans elle, un fil de fer.
        if self.scene3d.gpu.is_none() {
            if let Some(rs) = self.gpu.as_ref().map(|g| g.render_state().clone()) {
                self.scene3d.gpu = Some(Gpu3d::new(&rs));
            }
        }
        let objects = self.s3_draw_objects(panel, seq);
        let chosen: Vec<DrawObject> = objects.iter().filter(|o| o.selected).cloned().collect();
        let distance = self.scene3d.cam.map_or(8.0, |c| c.distance);
        let target = self.scene3d.cam.map_or([0.0; 3], |c| c.target);
        let grid = if self.scene3d.show_grid { grid_lines(target, if through { 8.0 } else { distance }) } else { Vec::new() };
        match &self.scene3d.gpu {
            Some(gpu) => {
                let frame = Frame {
                    view,
                    size_pts: [draw.width(), draw.height()],
                    light: plan.light,
                    style: plan.render.style,
                    lines: plan.render.has_lines(),
                    line_px: plan.render.line_px.min(3.0),
                    objects,
                    grid,
                };
                painter.add(gpu.paint(frame, draw));
            }
            None => self.s3_paint_wireframe(&painter, &proj, &objects, &grid),
        }

        // La boîte des objets choisis.
        for o in &chosen {
            self.s3_paint_box(&painter, &proj, o);
        }
        if !through {
            self.s3_paint_camera(&painter, &proj, &view, &plan.cam);
        }
        if let Some(g) = self.s3_gizmo(&view) {
            self.s3_paint_gizmo(&painter, &proj, &g);
        }
        // Légendes.
        if through {
            painter.rect_stroke(draw, 0.0, Stroke::new(1.0_f32, crate::theme::camera()), egui::StrokeKind::Outside);
            painter.text(
                draw.left_top() + vec2(8.0, 6.0),
                Align2::LEFT_TOP,
                format!("Caméra du plateau · {:.0} mm · {:.0}° · glisser (milieu) : déplacer · clic droit : tourner · molette : avancer", plan.cam.focal, plan.cam.angle_of_view()),
                FontId::proportional(11.0),
                crate::theme::camera(),
            );
        } else {
            painter.text(
                rect.left_bottom() + vec2(8.0, -6.0),
                Align2::LEFT_BOTTOM,
                "Clic : choisir · glisser : poser sur le sol (Maj : en hauteur) · Alt+glisser ou clic droit : tourner · milieu : glisser la vue · molette : avancer",
                FontId::proportional(11.0),
                Color32::from_gray(170),
            );
        }
        let label = format!("{} · {} objet(s){}", if self.scene3d.target == Target::Decor { "Décor" } else { "Plan" }, self.doc.scenes3d.list(panel, seq, self.scene3d.target).len(), if self.scene3d.job.is_some() { " · rendu du calque…" } else { "" });
        painter.text(rect.right_top() + vec2(-8.0, 6.0), Align2::RIGHT_TOP, label, FontId::proportional(11.0), Color32::from_gray(170));
        // Curseur.
        if response.hover_pos().is_some() {
            let icon = match (&self.scene3d.drag, self.scene3d.hover) {
                (Some(_), _) => CursorIcon::Grabbing,
                (None, Some(_)) => CursorIcon::Grab,
                _ => CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }
    }

    /// Boîte orientée d'un objet choisi : pointillés et petits carrés aux coins (comme la boîte de
    /// sélection du dessin).
    fn s3_paint_box(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, o: &DrawObject) {
        let (lo, hi) = o.mesh.bounds;
        let corners = m::corners_of(lo, hi).map(|c| proj(m::point(&o.matrix, c)));
        let stroke = Stroke::new(1.0_f32, ACCENT);
        for i in 0..8usize {
            for bit in [1usize, 2, 4] {
                if i & bit == 0 {
                    if let (Some(a), Some(b)) = (corners[i], corners[i | bit]) {
                        painter.extend(Shape::dashed_line(&[a, b], stroke, 4.0, 3.0));
                    }
                }
            }
        }
        for c in corners.into_iter().flatten() {
            let r = Rect::from_center_size(c, vec2(6.0, 6.0));
            painter.rect_filled(r, 1.0, Color32::WHITE);
            painter.rect_stroke(r, 1.0, Stroke::new(1.0_f32, ACCENT), egui::StrokeKind::Outside);
        }
    }

    /// La caméra du plateau : pyramide de vision, cadre, haut de l'image.
    fn s3_paint_camera(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, view: &View, cam: &SceneCam) {
        let selected = self.scene3d.sel.contains(&CAMERA_ID);
        let color = if selected { ACCENT } else { crate::theme::camera() };
        let corners = self.s3_camera_corners(cam, view);
        let strong = Stroke::new(if selected { 2.0_f32 } else { 1.5 }, color);
        let line = |a: V3, b: V3, stroke: Stroke| {
            if let (Some(p), Some(q)) = (proj(a), proj(b)) {
                painter.line_segment([p, q], stroke);
            }
        };
        for k in 0..4 {
            line(cam.pos, corners[k], Stroke::new(1.0_f32, color.gamma_multiply(0.7)));
            line(corners[k], corners[(k + 1) % 4], strong);
        }
        // Un triangle au-dessus du cadre : le haut de l'image.
        let up = m::scale(m::add(corners[0], corners[1]), 0.5);
        let v = cam.view(self.doc.frame[0] as f32);
        let tip = m::add(up, m::scale(v.up, m::length(m::sub(corners[0], corners[1])) * 0.12));
        line(corners[0], tip, strong);
        line(corners[1], tip, strong);
        if let Some(p) = proj(cam.pos) {
            painter.circle(p, if selected { 6.0 } else { 4.5 }, color, Stroke::new(1.5_f32, Color32::from_black_alpha(160)));
            painter.text(p + vec2(9.0, -4.0), Align2::LEFT_BOTTOM, format!("Caméra · {:.0} mm", cam.focal), FontId::proportional(11.0), color);
        }
    }

    /// Les poignées du manipulateur.
    fn s3_paint_gizmo(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, g: &Gizmo) {
        let hot = |h: Handle| self.scene3d.hover == Some(h) || matches!(&self.scene3d.drag, Some(Drag::Handle { handle, .. }) if *handle == h);
        let color = |h: Handle, k: usize| if hot(h) { HOT } else { AXIS_COLORS[k] };
        let Some(o) = proj(g.origin) else { return };
        match g.mode {
            GizmoMode::Select => {}
            GizmoMode::Move | GizmoMode::Scale => {
                for k in 0..3 {
                    let h = Handle::Axis(k);
                    let Some(t) = proj(g.axis_tip(k)) else { continue };
                    let c = color(h, k);
                    painter.line_segment([o, t], Stroke::new(if hot(h) { 3.0_f32 } else { 2.0 }, c));
                    if g.mode == GizmoMode::Move {
                        // Pointe de flèche.
                        let d = (t - o).normalized();
                        let n = vec2(-d.y, d.x);
                        painter.add(Shape::convex_polygon(vec![t + d * 9.0, t - d * 3.0 + n * 5.0, t - d * 3.0 - n * 5.0], c, Stroke::NONE));
                    } else {
                        painter.rect_filled(Rect::from_center_size(t, vec2(9.0, 9.0)), 1.0, c);
                    }
                }
                if g.mode == GizmoMode::Move {
                    for k in 0..3 {
                        let q = g.plane_quad(k).map(proj);
                        if let [Some(a), Some(b), Some(c), Some(d)] = q {
                            let col = color(Handle::Plane(k), k);
                            painter.add(Shape::convex_polygon(vec![a, b, c, d], col.gamma_multiply(0.35), Stroke::new(1.0_f32, col)));
                        }
                    }
                } else {
                    painter.rect_filled(Rect::from_center_size(o, vec2(11.0, 11.0)), 1.0, if hot(Handle::Center) { HOT } else { Color32::from_gray(225) });
                }
            }
            GizmoMode::Rotate => {
                for k in 0..3 {
                    let pts: Vec<Pos2> = g.ring_points(k, 64).into_iter().filter_map(|p| proj(p)).collect();
                    let h = Handle::Ring(k);
                    painter.add(Shape::line(pts, Stroke::new(if hot(h) { 3.0_f32 } else { 2.0 }, color(h, k))));
                }
            }
        }
    }

    /// Sans carte graphique : fil de fer (arêtes vives de chaque objet, ou sa boîte).
    fn s3_paint_wireframe(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, objects: &[DrawObject], grid: &[GridLine]) {
        for l in grid {
            if let (Some(a), Some(b)) = (proj(l.a), proj(l.b)) {
                painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_white_alpha((l.color[3] * 120.0) as u8)));
            }
        }
        for o in objects {
            let color = if o.selected { ACCENT } else { Color32::from_gray(if o.ghost { 90 } else { 200 }) };
            if o.mesh.edges.is_empty() {
                let (lo, hi) = o.mesh.bounds;
                let c = m::corners_of(lo, hi).map(|c| proj(m::point(&o.matrix, c)));
                for i in 0..8usize {
                    for bit in [1usize, 2, 4] {
                        if i & bit == 0 {
                            if let (Some(a), Some(b)) = (c[i], c[i | bit]) {
                                painter.line_segment([a, b], Stroke::new(1.0_f32, color));
                            }
                        }
                    }
                }
            } else {
                for e in &o.mesh.edges {
                    if let (Some(a), Some(b)) = (proj(m::point(&o.matrix, e[0])), proj(m::point(&o.matrix, e[1]))) {
                        painter.line_segment([a, b], Stroke::new(1.0_f32, color));
                    }
                }
            }
        }
    }
}

/// Distance du point `p` au segment [a, b] (tout en pixels).
fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (a, b) = (pos2(a[0], a[1]), pos2(b[0], b[1]));
    let (d, w) = (b - a, pos2(p[0], p[1]) - a);
    let l2 = d.length_sq();
    let t = if l2 > 0.0 { (w.dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (w - d * t).length()
}
