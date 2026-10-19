//! Vue du plateau en 3D dans l'application (9e) : affichage (sol, plans des calques texturés par
//! leur dessin, caméra du plan), navigation (orbite, glisser, avancer), et outil Déplacer : un
//! calque dans son plan (Maj : en profondeur), ou la caméra en prenant son œil (Maj : avancée).

use std::collections::HashMap;

use eframe::egui::{
    self, epaint::Vertex, pos2, vec2, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Shape,
    Stroke, TextureHandle, TextureId,
};

use crate::app::App;
use crate::camera::Camera;
use crate::document::{CelId, LayerTransform};
use crate::history::Edit;
use crate::stage3d::{self, Projector, World, V3, NEAR};
use crate::vector::Affine;
use crate::view3d::Cam3d;
use crate::viewbar::veil;

/// La couleur de la caméra : celle de ses clés dans la timeline et le montage (palette du thème).
fn camera_color() -> Color32 {
    crate::theme::camera()
}
const ACCENT: Color32 = crate::theme::POIGNEE_SELECTION;
/// Rayon de prise de l'œil de la caméra, en pixels d'écran.
const GRAB: f32 = 14.0;
/// Subdivisions d'un plan (la texture est posée par triangles : sans elles, la perspective
/// déformerait le dessin).
const SUBDIV: usize = 16;

/// Outils de la vue 3D.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Nav3d {
    #[default]
    Orbit,
    Pan,
    Dolly,
    Move,
}

impl Nav3d {
    pub(crate) const ALL: [Nav3d; 4] = [Nav3d::Orbit, Nav3d::Pan, Nav3d::Dolly, Nav3d::Move];

    pub(crate) fn action(self) -> &'static str {
        match self {
            Nav3d::Orbit => "outil3d.orbite",
            Nav3d::Pan => "outil3d.glisser",
            Nav3d::Dolly => "outil3d.avancer",
            Nav3d::Move => "outil3d.deplacer",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Nav3d::Orbit => "Orbite",
            Nav3d::Pan => "Glisser la vue",
            Nav3d::Dolly => "Avancer / reculer",
            Nav3d::Move => "Déplacer",
        }
    }

    /// Petite description, pour la bulle d'aide (12d).
    pub(crate) fn description(self) -> &'static str {
        match self {
            Nav3d::Orbit => "Tourner autour du plateau.",
            Nav3d::Pan => "Déplacer la vue sans la tourner.",
            Nav3d::Dolly => "Glisser vers le haut ou le bas pour avancer ou reculer.",
            Nav3d::Move => {
                "Un calque dans son plan (Maj : en profondeur), ou la caméra par son œil (Maj : avancée ; \
                 Ctrl : la tourner, panoramique et bascule)."
            }
        }
    }
    pub(crate) fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Nav3d::Orbit => ph::GLOBE,
            Nav3d::Pan => ph::HAND,
            Nav3d::Dolly => ph::ARROWS_IN_SIMPLE,
            Nav3d::Move => ph::ARROWS_OUT_CARDINAL,
        }
    }
}

/// Geste en cours dans la vue 3D.
enum Drag3d {
    /// La vue (orbite, glisser, avancer).
    View,
    /// Un calque : dans son plan ou, `along`, en profondeur.
    Layer { id: u64, along: bool, z0: f32, depth0: f32, hit0: V3, screen0: Pos2, t0: LayerTransform, snap: Edit, changed: bool },
    /// La caméra : dans son plan ou, `along`, en avancée.
    Camera { along: bool, hit0: V3, screen0: Pos2, cam0: Camera, snap: Edit, changed: bool },
    /// La caméra tournée sur elle-même (Ctrl, 9f) : panoramique et bascule.
    Aim { screen0: Pos2, cam0: Camera, snap: Edit, changed: bool },
}

/// État de la vue 3D.
#[derive(Default)]
pub(crate) struct Stage3dState {
    pub(crate) on: bool,
    pub(crate) tool: Nav3d,
    /// Vue 3D ; aucune : cadrée sur le plateau au prochain affichage.
    pub(crate) cam: Option<Cam3d>,
    drag: Option<Drag3d>,
    /// Textures des dessins, par dessin et version.
    textures: HashMap<CelId, (u64, TextureHandle)>,
}

/// Plan subdivisé : les points `at(u, v)` (u, v de 0 à 1), texturé ou d'une couleur.
fn plane_mesh(proj: &Projector, at: impl Fn(f32, f32) -> V3, texture: Option<TextureId>, color: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(texture.unwrap_or_default());
    let n = SUBDIV;
    let mut front = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let c = proj.to_view(at(u, v));
            front.push(c[2] >= NEAR);
            let uv = if texture.is_some() { pos2(u, v) } else { egui::epaint::WHITE_UV };
            mesh.vertices.push(Vertex { pos: proj.screen_of(c), uv, color });
        }
    }
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            let (b, c, d) = (a + 1, a + n + 1, a + n + 2);
            // Un morceau qui passe derrière l'œil n'est pas dessiné.
            if front[a] && front[b] && front[c] && front[d] {
                mesh.add_triangle(a as u32, b as u32, d as u32);
                mesh.add_triangle(a as u32, d as u32, c as u32);
            }
        }
    }
    mesh
}

impl App {
    fn world(&self) -> World {
        World::new([self.doc.width, self.doc.height], self.doc.frame)
    }

    /// Vue 3D ou plateau à plat (`on`) ; la vue 3D montre le plateau, pas la vue caméra.
    pub(crate) fn set_stage3d(&mut self, on: bool) {
        if on {
            self.set_camera_view(false);
            self.end_stroke();
        }
        self.stage3d.on = on;
        self.stage3d.drag = None;
        self.status = if on {
            "Plateau en 3D : glisser pour tourner autour, molette pour avancer, bouton du milieu pour glisser la vue ; \
             outil Déplacer pour bouger un calque ou la caméra."
                .into()
        } else {
            "Plateau à plat.".into()
        };
    }

    /// Recadre la vue 3D sur tout le plateau (plans des calques et caméra), vue sous l'angle donné.
    pub(crate) fn fit_stage3d(&mut self, azimuth: f32, elevation: f32) {
        let world = self.world();
        let cam = self.current_camera();
        let mut points: Vec<V3> = vec![world.eye(&cam)];
        points.extend(world.frame_corners(&cam));
        let layers = self.layers();
        for i in (0..layers.len()).filter(|&i| crate::folders::effect(layers, i).is_some()) {
            points.extend(world.plane_corners(layers[i].depth));
        }
        points.extend(world.plane_corners(0.0));
        let rect = self.canvas_rect;
        self.stage3d.cam = Some(Cam3d::fit_at(stage3d::bounds(points), [rect.width().max(1.0), rect.height().max(1.0)], azimuth, elevation));
    }

    /// Outils de la vue 3D, dans la barre au-dessus du dessin.
    pub(crate) fn stage3d_tools(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.add_space(6.0);
        for t in Nav3d::ALL {
            if self.bouton_etat(ui, self.stage3d.tool == t, t.icon(), t.label(), t.description(), t.action()).clicked() {
                self.stage3d.tool = t;
            }
        }
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
        if self.bouton(ui, ph::CORNERS_OUT, "Recadrer", "Tout le plateau dans la vue.", "vue3d.recadrer").clicked() {
            self.fit_stage3d(35.0, 20.0);
        }
        let views: [(&str, &str, &str, f32, f32); 3] = [
            (ph::SQUARE, "Vue de face", "Du côté de la caméra.", 0.0, 0.0),
            (ph::ARROW_FAT_LINES_DOWN, "Vue de dessus", "Pour voir les profondeurs.", 0.0, crate::view3d::MAX_ELEVATION),
            (ph::ARROWS_HORIZONTAL, "Vue de côté", "", 90.0, 0.0),
        ];
        for (icon, name, description, az, el) in views {
            if crate::bulles::icone(ui, icon, name, description, "").clicked() {
                self.fit_stage3d(az, el);
            }
        }
        if crate::bulles::icone(ui, ph::VIDEO_CAMERA, "Par la caméra", "La vue 3D à l'œil de la caméra du plan, avec sa focale.", "").clicked() {
            self.stage3d_through_camera();
        }
    }
    /// La vue 3D placée à l'œil de la caméra du plan.
    pub(crate) fn stage3d_through_camera(&mut self) {
        self.stage3d.cam = Some(self.world().look_through(&self.current_camera()));
    }

    /// Texture du dessin `cel` (refaite quand il change).
    fn cel_texture(&mut self, ctx: &egui::Context, cel: CelId) -> Option<TextureId> {
        let c = self.doc.cels.get(&cel)?;
        if let Some((v, t)) = self.stage3d.textures.get(&cel) {
            if *v == c.version {
                return Some(t.id());
            }
        }
        let image = crate::render::cel_image(&c.raster, 1024);
        let tex = ctx.load_texture(format!("plateau 3d {cel}"), image, egui::TextureOptions::LINEAR);
        if self.stage3d.textures.len() > 256 {
            self.stage3d.textures.clear();
        }
        let id = tex.id();
        self.stage3d.textures.insert(cel, (c.version, tex));
        Some(id)
    }

    /// Le calque `li` a quelque chose de dessiné près du point `p` de la feuille (image courante).
    fn layer_drawn_at(&self, li: usize, p: Pos2) -> bool {
        let Some(layer) = self.layers().get(li) else { return false };
        let Some(cel) = layer.shown_at(self.cur_frame).and_then(|c| self.doc.cels.get(&c)) else { return false };
        let Some(inv) = layer.transform.affine(self.doc.stage_center()).inverse() else { return false };
        let q = inv.apply(p);
        const R: f32 = 8.0;
        (-2..=2).any(|j| {
            (-2..=2).any(|i| {
                let (x, y) = (q.x + i as f32 * R / 2.0, q.y + j as f32 * R / 2.0);
                x >= 0.0
                    && y >= 0.0
                    && (x as usize) < cel.raster.width
                    && (y as usize) < cel.raster.height
                    && cel.raster.get(x as usize, y as usize).a() > 24
            })
        })
    }

    /// La vue 3D dans la zone du dessin.
    pub(crate) fn stage3d_panel(&mut self, ui: &mut egui::Ui, rect: Rect, response: &egui::Response) {
        if self.stage3d.cam.is_none() {
            self.fit_stage3d(35.0, 20.0);
        }
        let world = self.world();
        self.stage3d_input(ui.ctx(), rect, response, &world);
        let Some(cam3d) = self.stage3d.cam else { return };
        let proj = Projector::new(&cam3d, rect);
        let painter = ui.painter_at(rect);

        // Fond en dégradé.
        let mut bg = egui::Mesh::default();
        let (top, bottom) = (Color32::from_gray(44), Color32::from_gray(74));
        for (p, c) in [(rect.left_top(), top), (rect.right_top(), top), (rect.right_bottom(), bottom), (rect.left_bottom(), bottom)] {
            bg.colored_vertex(p, c);
        }
        bg.add_triangle(0, 1, 2);
        bg.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(bg));

        let cam = self.current_camera();
        let eye = world.eye(&cam);
        // Calques qui entrent dans l'image (ni cachés, ni dans un dossier caché, ni dossiers).
        let visible: Vec<usize> = (0..self.layers().len()).filter(|&i| crate::folders::effect(self.layers(), i).is_some()).collect();
        let far_z = visible.iter().map(|&i| world.plane_z(self.layers()[i].depth)).fold(0.0_f32, f32::min);
        let near_z = visible.iter().map(|&i| world.plane_z(self.layers()[i].depth)).fold(0.0_f32, f32::max);

        self.paint_floor(&painter, &proj, &world, &visible, far_z, near_z.max(eye[2]));

        // Plans du fond vers l'avant (par rapport à l'œil de la vue) : chacun recouvre ceux
        // qui sont derrière lui.
        let view_z = proj.eye()[2];
        let mut order = visible.clone();
        order.sort_by(|&a, &b| {
            let da = (world.plane_z(self.layers()[a].depth) - view_z).abs();
            let db = (world.plane_z(self.layers()[b].depth) - view_z).abs();
            db.total_cmp(&da)
        });
        for li in order {
            self.paint_plane(ui.ctx(), &painter, &proj, &world, li);
        }

        self.paint_camera3d(&painter, &proj, &world, far_z);
        painter.text(
            rect.left_bottom() + vec2(8.0, -6.0),
            Align2::LEFT_BOTTOM,
            "Glisser : outil choisi · Alt+glisser ou clic droit : tourner · bouton du milieu : glisser la vue · molette : avancer",
            FontId::proportional(11.0),
            Color32::from_gray(170),
        );
        self.stage3d_cursor(ui.ctx(), response, &proj, &world);
    }

    /// Sol quadrillé sous le plus bas des plans, du fond jusque derrière la caméra.
    fn paint_floor(&self, painter: &egui::Painter, proj: &Projector, world: &World, visible: &[usize], far_z: f32, near_z: f32) {
        let half_h = world.stage[1] / 2.0;
        let spread = visible.iter().map(|&i| 1.0 + self.layers()[i].depth.max(crate::document::Layer::MIN_DEPTH)).fold(1.0_f32, f32::max);
        let y = -half_h * spread - world.d0 * 0.02;
        let half_w = world.stage[0] / 2.0 * spread * 1.2;
        let (z0, z1) = (far_z - world.d0 * 0.3, near_z + world.d0 * 0.3);
        let step = stage3d::grid_step((2.0 * half_w).max(z1 - z0));
        let stroke = Stroke::new(1.0_f32, veil(22));
        let line = |a: V3, b: V3| {
            if let Some(s) = proj.segment(a, b) {
                painter.line_segment(s, stroke);
            }
        };
        let mut x = (-half_w / step).floor() * step;
        while x <= half_w {
            line([x, y, z0], [x, y, z1]);
            x += step;
        }
        let mut z = (z0 / step).floor() * step;
        while z <= z1 {
            line([-half_w, y, z], [half_w, y, z]);
            z += step;
        }
    }

    /// Plan d'un calque : la feuille (voile clair et contour), son dessin, son nom.
    fn paint_plane(&mut self, ctx: &egui::Context, painter: &egui::Painter, proj: &Projector, world: &World, li: usize) {
        let layer = &self.layers()[li];
        let (depth, opacity, name) = (layer.depth, layer.opacity.clamp(0.0, 1.0), layer.name.clone());
        let own: Affine = layer.transform.affine(self.doc.stage_center());
        let cel = layer.shown_at(self.cur_frame);
        let current = li == self.cur_layer;
        let [w, h] = world.stage;
        let sheet = |u: f32, v: f32| world.at_depth(pos2(u * w, v * h), depth);
        painter.add(Shape::mesh(plane_mesh(proj, sheet, None, veil(if current { 46 } else { 34 }))));
        if let Some(tex) = cel.and_then(|c| self.cel_texture(ctx, c)) {
            let drawing = |u: f32, v: f32| world.at_depth(own.apply(pos2(u * w, v * h)), depth);
            let a = (opacity * 255.0).round() as u8;
            painter.add(Shape::mesh(plane_mesh(proj, drawing, Some(tex), veil(a))));
        }
        let corners = world.plane_corners(depth);
        let stroke = if current { Stroke::new(2.0_f32, ACCENT) } else { Stroke::new(1.0_f32, veil(90)) };
        for k in 0..4 {
            if let Some(s) = proj.segment(corners[k], corners[(k + 1) % 4]) {
                painter.line_segment(s, stroke);
            }
        }
        if let Some(p) = proj.project(corners[0]) {
            let label = if depth == 0.0 { name } else { format!("{name}  ({depth:+.2})") };
            let galley = painter.layout_no_wrap(label, FontId::proportional(11.0), if current { ACCENT } else { Color32::from_gray(225) });
            let at = p + vec2(4.0, -galley.size().y - 3.0);
            painter.rect_filled(Rect::from_min_size(at - vec2(3.0, 1.0), galley.size() + vec2(6.0, 2.0)), 3.0, Color32::from_black_alpha(140));
            painter.galley(at, galley, Color32::WHITE);
        }
    }

    /// Caméra du panneau : œil, pyramide de vision jusqu'au cadre (plan de référence) et au-delà,
    /// et sa course pendant le panneau si elle bouge.
    fn paint_camera3d(&self, painter: &egui::Painter, proj: &Projector, world: &World, far_z: f32) {
        let cam = self.current_camera();
        let eye = world.eye(&cam);
        let corners = world.frame_corners(&cam);
        let line = |a: V3, b: V3, stroke: Stroke| {
            if let Some(s) = proj.segment(a, b) {
                painter.line_segment(s, stroke);
            }
        };
        let strong = Stroke::new(1.5_f32, camera_color());
        let faint = Stroke::new(1.0_f32, camera_color().gamma_multiply(0.45));
        for k in 0..4 {
            line(eye, corners[k], strong);
            line(corners[k], corners[(k + 1) % 4], Stroke::new(2.0_f32, camera_color()));
        }
        // Au-delà du cadre, jusqu'au plan le plus lointain.
        if far_z < -1.0 && eye[2] > 0.0 {
            let t = (far_z - eye[2]) / (0.0 - eye[2]);
            let far = corners.map(|c| stage3d::lerp3(eye, c, t));
            for k in 0..4 {
                line(corners[k], far[k], faint);
                line(far[k], far[(k + 1) % 4], faint);
            }
        }
        // Objectif : petit cadre près de l'œil, face à la visée (panoramique, bascule), et l'œil.
        let (st, ct) = cam.tilt.to_radians().sin_cos();
        let (sp, cp) = cam.pan.to_radians().sin_cos();
        let forward = [ct * sp, st, -ct * cp];
        let reach = world.d0 * 0.07;
        let lens = corners.map(|c| {
            let ahead = (0..3).map(|k| (c[k] - eye[k]) * forward[k]).sum::<f32>().max(1e-3);
            stage3d::lerp3(eye, c, reach / ahead)
        });
        for k in 0..4 {
            line(lens[k], lens[(k + 1) % 4], strong);
        }
        // Course de la caméra pendant le panneau.
        let panel = &self.doc.panels[self.cur_panel];
        if panel.camera.len() >= 2 {
            let n = self.frame_count().max(1);
            let pts: Vec<Pos2> = (0..n)
                .filter_map(|f| proj.project(world.eye(&self.doc.panel_camera(self.cur_panel, f))))
                .collect();
            painter.add(Shape::line(pts, Stroke::new(1.0_f32, camera_color().gamma_multiply(0.7))));
        }
        if let Some(p) = proj.project(eye) {
            let editing = self.stage3d.tool == Nav3d::Move;
            painter.circle(p, if editing { 6.0 } else { 4.5 }, camera_color(), Stroke::new(1.5_f32, Color32::from_black_alpha(160)));
            painter.text(p + vec2(9.0, -4.0), Align2::LEFT_BOTTOM, "Caméra", FontId::proportional(11.0), camera_color());
        }
    }

    fn stage3d_cursor(&self, ctx: &egui::Context, response: &egui::Response, proj: &Projector, world: &World) {
        let Some(p) = response.hover_pos() else { return };
        let icon = match (&self.stage3d.drag, self.stage3d.tool) {
            (Some(Drag3d::Layer { .. } | Drag3d::Camera { .. } | Drag3d::Aim { .. }), _) => CursorIcon::Grabbing,
            (Some(Drag3d::View), _) => CursorIcon::Grabbing,
            (None, Nav3d::Move) => {
                let on_eye = proj.project(world.eye(&self.current_camera())).is_some_and(|e| e.distance(p) <= GRAB);
                if on_eye { CursorIcon::Grab } else { CursorIcon::Move }
            }
            (None, Nav3d::Dolly) => CursorIcon::ResizeVertical,
            (None, _) => CursorIcon::Grab,
        };
        ctx.set_cursor_icon(icon);
    }

    /// Souris dans la vue 3D.
    fn stage3d_input(&mut self, ctx: &egui::Context, rect: Rect, response: &egui::Response, world: &World) {
        let (scroll, mods, press, primary_down) =
            ctx.input(|i| (i.smooth_scroll_delta, i.modifiers, i.pointer.press_origin(), i.pointer.primary_down()));
        let Some(mut cam) = self.stage3d.cam else { return };
        let delta = response.drag_delta();
        if response.hovered() && scroll.y != 0.0 {
            cam.dolly((-scroll.y * 0.002).exp());
        }
        if response.dragged_by(PointerButton::Middle) {
            cam.pan(delta.x, delta.y, rect.width());
        }
        if response.dragged_by(PointerButton::Secondary) {
            cam.orbit(-delta.x * 0.35, delta.y * 0.35);
        }
        if response.drag_started_by(PointerButton::Primary) {
            let at = press.or(response.interact_pointer_pos()).unwrap_or(rect.center());
            if self.stage3d.tool == Nav3d::Move && !mods.alt {
                self.press3d(world, &Projector::new(&cam, rect), at, mods.shift, mods.command || mods.ctrl);
            } else {
                self.stage3d.drag = Some(Drag3d::View);
            }
        }
        if response.dragged_by(PointerButton::Primary) {
            match self.stage3d.drag {
                Some(Drag3d::View) => match (self.stage3d.tool, mods.alt) {
                    (Nav3d::Pan, false) => cam.pan(delta.x, delta.y, rect.width()),
                    (Nav3d::Dolly, false) => cam.dolly((delta.y * 0.006).exp()),
                    _ => cam.orbit(-delta.x * 0.35, delta.y * 0.35),
                },
                Some(_) => {
                    if let Some(p) = response.interact_pointer_pos() {
                        self.drag3d(world, &Projector::new(&cam, rect), p);
                    }
                }
                None => {}
            }
        }
        if response.clicked() && self.stage3d.tool == Nav3d::Move {
            // Un clic choisit le calque sous la souris.
            if let Some(p) = response.interact_pointer_pos() {
                if let Some(li) = self.pick3d(world, &Projector::new(&cam, rect), p) {
                    self.select_layer(li);
                }
            }
        }
        if !primary_down {
            self.finish_drag3d();
        }
        self.stage3d.cam = Some(cam);
    }

    /// Calque visible sous le point `at` de l'écran.
    fn pick3d(&self, world: &World, proj: &Projector, at: Pos2) -> Option<usize> {
        let layers = self.layers();
        let planes: Vec<(usize, f32)> =
            (0..layers.len()).filter(|&i| crate::folders::effect(layers, i).is_some()).map(|i| (i, layers[i].depth)).collect();
        let (o, d) = proj.ray(at);
        stage3d::pick(world, o, d, &planes, |i, p| self.layer_drawn_at(i, p))
    }

    /// Début d'un geste de l'outil Déplacer : la caméra si l'on prend son œil, sinon le calque
    /// sous la souris ; rien dessous : on tourne autour.
    fn press3d(&mut self, world: &World, proj: &Projector, at: Pos2, along: bool, aim: bool) {
        let cam = self.current_camera();
        let eye = world.eye(&cam);
        let (o, d) = proj.ray(at);
        if aim && proj.project(eye).is_some_and(|e| e.distance(at) <= GRAB) {
            let snap = self.snapshot();
            self.stage3d.drag = Some(Drag3d::Aim { screen0: at, cam0: cam, snap, changed: false });
            return;
        }
        if proj.project(eye).is_some_and(|e| e.distance(at) <= GRAB) {
            let hit0 = stage3d::ray_plane_z(o, d, eye[2]).map_or(eye, |h| h.0);
            let snap = self.snapshot();
            self.stage3d.drag = Some(Drag3d::Camera { along, hit0: if along { eye } else { hit0 }, screen0: at, cam0: cam, snap, changed: false });
            return;
        }
        let Some(li) = self.pick3d(world, proj, at) else {
            self.stage3d.drag = Some(Drag3d::View);
            return;
        };
        self.select_layer(li);
        if self.content_refuses() {
            self.stage3d.drag = Some(Drag3d::View);
            return;
        }
        let layer = &self.layers()[li];
        if layer.locked || layer.position_locked {
            self.status = if layer.locked {
                format!("Le calque « {} » est verrouillé.", layer.name)
            } else {
                format!("La position du calque « {} » est verrouillée.", layer.name)
            };
            self.stage3d.drag = Some(Drag3d::View);
            return;
        }
        let (id, depth0, t0) = (layer.id, layer.depth, layer.transform);
        let z0 = world.plane_z(depth0);
        let hit0 = stage3d::ray_plane_z(o, d, z0).map_or([0.0, 0.0, z0], |h| h.0);
        let snap = self.snapshot();
        self.stage3d.drag = Some(Drag3d::Layer { id, along, z0, depth0, hit0, screen0: at, t0, snap, changed: false });
    }

    fn drag3d(&mut self, world: &World, proj: &Projector, p: Pos2) {
        let (o, d) = proj.ray(p);
        match &mut self.stage3d.drag {
            Some(Drag3d::Layer { id, along, z0, depth0, hit0, screen0, t0, changed, .. }) => {
                let Some(li) = self.doc.panels[self.cur_panel].layers.iter().position(|l| l.id == *id) else { return };
                let layer = &mut self.doc.panels[self.cur_panel].layers[li];
                if *along {
                    let dz = stage3d::z_motion(proj, *hit0, *screen0, p);
                    layer.depth = (world.depth_of(*z0 + dz) * 100.0).round() / 100.0;
                    self.status = format!("Calque « {} » : profondeur {:+.2}", layer.name, layer.depth);
                } else if let Some((hit, _)) = stage3d::ray_plane_z(o, d, *z0) {
                    let dv = world.stage_delta(*hit0, hit, *depth0);
                    layer.transform = LayerTransform { x: t0.x + dv.x, y: t0.y + dv.y, ..*t0 };
                    self.status = format!("Calque « {} » : décalage {:+.0} × {:+.0} px", layer.name, layer.transform.x, layer.transform.y);
                }
                *changed = true;
                self.modified = true;
            }
            Some(Drag3d::Camera { along, hit0, screen0, cam0, changed, .. }) => {
                let mut cam = *cam0;
                if *along {
                    let dz = stage3d::z_motion(proj, *hit0, *screen0, p);
                    cam.dolly = (cam0.dolly - dz).clamp(-world.d0 * 20.0, world.d0 * 0.95);
                } else if let Some((hit, _)) = stage3d::ray_plane_z(o, d, hit0[2]) {
                    cam.x = cam0.x + (hit[0] - hit0[0]);
                    cam.y = cam0.y - (hit[1] - hit0[1]);
                } else {
                    return;
                }
                *changed = true;
                self.set_current_camera(cam);
                self.status = format!(
                    "Caméra : X {:.0}, Y {:.0}, Z {:.0}",
                    cam.x - self.doc.width as f32 / 2.0,
                    cam.y - self.doc.height as f32 / 2.0,
                    world.d0 - cam.dolly
                );
            }
            Some(Drag3d::Aim { screen0, cam0, changed, .. }) => {
                // Vers la droite : panoramique à droite ; vers le haut : bascule vers le haut.
                let m = p - *screen0;
                let cam = Camera {
                    pan: (cam0.pan + m.x * 0.25).clamp(-85.0, 85.0),
                    tilt: (cam0.tilt - m.y * 0.25).clamp(-85.0, 85.0),
                    ..*cam0
                };
                *changed = true;
                self.set_current_camera(cam);
                let cam = self.current_camera();
                self.status = format!("Caméra : panoramique {:+.1}°, bascule {:+.1}°", cam.pan, cam.tilt);
            }
            _ => {}
        }
    }

    fn finish_drag3d(&mut self) {
        match self.stage3d.drag.take() {
            Some(Drag3d::Layer { snap, changed: true, .. })
            | Some(Drag3d::Camera { snap, changed: true, .. })
            | Some(Drag3d::Aim { snap, changed: true, .. }) => {
                self.history.push(snap);
                self.modified = true;
            }
            _ => {}
        }
    }
}
