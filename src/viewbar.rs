//! Barre au-dessus du dessin (9d) : vue du plateau (toute la feuille, avec le cadre de la caméra)
//! ou vue caméra (ce que filme la caméra du plan), et cadres de composition tracés dans le
//! cadre de la caméra : zones de sécurité et cadre d'un format de diffusion (9:16, 1:1…).

use eframe::egui::{self, vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::App;
use crate::frames;

/// Hauteur de la barre.
pub(crate) const VIEW_BAR_H: f32 = 26.0;
/// Couleurs des zones de sécurité et du cadre de format.
const SAFE: Color32 = Color32::from_rgb(0, 165, 195);
const FORMAT: Color32 = Color32::from_rgb(230, 60, 160);

/// Voile blanc d'opacité `a` (couleur prémultipliée : `Color32::from_white_alpha` passe par
/// l'espace linéaire et donne un voile bien plus clair).
pub(crate) fn veil(a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(a, a, a, a)
}

/// Polygone convexe `poly` coupé au rectangle `r` (le morceau qui est dedans, convexe aussi).
pub(crate) fn clip_to_rect(poly: &[Pos2], r: Rect) -> Vec<Pos2> {
    let mut pts = poly.to_vec();
    // Quatre demi-plans : x ≥ gauche, x ≤ droite, y ≥ haut, y ≤ bas.
    let sides: [(fn(Pos2) -> f32, f32, f32); 4] = [(|p| p.x, r.min.x, 1.0), (|p| p.x, r.max.x, -1.0), (|p| p.y, r.min.y, 1.0), (|p| p.y, r.max.y, -1.0)];
    for (coord, limit, sign) in sides {
        if pts.is_empty() {
            break;
        }
        let inside = |p: Pos2| (coord(p) - limit) * sign >= 0.0;
        let mut out = Vec::with_capacity(pts.len() + 2);
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if inside(a) {
                out.push(a);
            }
            if inside(a) != inside(b) {
                let t = (limit - coord(a)) / (coord(b) - coord(a));
                out.push(a + (b - a) * t);
            }
        }
        pts = out;
    }
    pts
}

/// Bande entre deux polygones convexes, `inner` dans `outer`, d'une couleur : on fait le tour des
/// deux ensemble autour d'un point intérieur, un triangle à chaque sommet rencontré.
pub(crate) fn nested_band(outer: &[Pos2], inner: &[Pos2], color: Color32) -> Shape {
    let mut mesh = egui::Mesh::default();
    if inner.len() < 3 {
        for p in outer {
            mesh.colored_vertex(*p, color);
        }
        for k in 1..outer.len().saturating_sub(1) {
            mesh.add_triangle(0, k as u32, k as u32 + 1);
        }
        return Shape::mesh(mesh);
    }
    let c = inner.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / inner.len() as f32;
    let sorted = |pts: &[Pos2]| {
        let mut v: Vec<(f32, Pos2)> = pts.iter().map(|p| ((p.y - c.y).atan2(p.x - c.x), *p)).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    };
    let (o, n) = (sorted(outer), sorted(inner));
    for (_, p) in o.iter().chain(n.iter()) {
        mesh.colored_vertex(*p, color);
    }
    let (no, ni) = (o.len(), n.len());
    // Angle du k-ième sommet en continuant après un tour.
    let turn = |v: &[(f32, Pos2)], k: usize| v[k % v.len()].0 + if k >= v.len() { std::f32::consts::TAU } else { 0.0 };
    let (oi, ii) = (|k: usize| (k % no) as u32, |k: usize| (no + k % ni) as u32);
    let (mut i, mut j) = (0, 0);
    while i < no || j < ni {
        let next_o = if i < no { turn(&o, i + 1) } else { f32::INFINITY };
        let next_i = if j < ni { turn(&n, j + 1) } else { f32::INFINITY };
        if next_o <= next_i {
            mesh.add_triangle(oi(i), oi(i + 1), ii(j));
            i += 1;
        } else {
            mesh.add_triangle(oi(i), ii(j + 1), ii(j));
            j += 1;
        }
    }
    Shape::mesh(mesh)
}

/// Bande entre deux quadrilatères (l'un dans l'autre), d'une couleur.
pub(crate) fn band(outer: [Pos2; 4], inner: [Pos2; 4], color: Color32) -> Shape {
    let mut mesh = egui::Mesh::default();
    for p in outer.iter().chain(inner.iter()) {
        mesh.colored_vertex(*p, color);
    }
    for i in 0..4u32 {
        let j = (i + 1) % 4;
        mesh.add_triangle(i, j, 4 + j);
        mesh.add_triangle(i, 4 + j, 4 + i);
    }
    Shape::mesh(mesh)
}

impl App {
    /// Vue caméra (`on`) ou retour à la vue du plateau telle qu'on l'avait laissée.
    pub(crate) fn set_camera_view(&mut self, on: bool) {
        if on == self.camera_view {
            return;
        }
        if on {
            self.stage_view = Some(self.view);
        } else {
            match self.stage_view.take() {
                Some(v) => self.view = v,
                None => self.fit_pending = true,
            }
        }
        self.camera_view = on;
        self.status = format!("Vue {}.", if on { "caméra" } else { "du plateau" });
    }

    /// La barre : onglets de vue à gauche, puis les cadres de composition.
    pub(crate) fn view_bar(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::ENTETE);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            ui.add_space(4.0);
            let cam = self.camera_view;
            if self.bouton_etat(ui, !cam, ph::BOUNDING_BOX, "Vue du plateau", "Toute la feuille, avec le cadre de la caméra.", "camera.vue").clicked() {
                self.set_camera_view(false);
            }
            if self.bouton_etat(ui, cam, ph::VIDEO_CAMERA, "Vue caméra", "Ce que filme la caméra du plan.", "camera.vue").clicked() {
                self.set_camera_view(true);
            }
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            // Plateau en 3D (9e) et ses outils.
            let in3d = self.stage3d.on && !cam;
            if self.bouton_etat(ui, in3d, ph::CUBE_FOCUS, "Plateau en 3D", "Les calques à leur profondeur, la caméra et le sol, vus en 3D.", "vue.3d").clicked() {
                self.run_action("vue.3d");
            }
            if in3d {
                self.stage3d_tools(ui);
                return;
            }
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            if self
                .bouton_etat(ui, self.layout.safe_frames, ph::SELECTION, "Zones de sécurité", "Action 93 %, titres 90 %, tracées dans le cadre.", "vue.cadres_securite")
                .clicked()
            {
                self.run_action("vue.cadres_securite");
            }
            ui.add_space(4.0);
            self.frame_format_ui(ui);
        });
    }

    /// Choix du cadre de format : aucun, l'un des formats proposés, ou un format personnalisé.
    pub(crate) fn frame_format_ui(&mut self, ui: &mut egui::Ui) {
        let current = self.layout.frame_format.and_then(frames::sanitized);
        // (Un format qui n'est pas proposé, retenu d'une séance précédente, est personnalisé.)
        let custom = current.is_some_and(|r| self.format_custom || !frames::is_preset(r));
        let shown = match current {
            None => "Aucun".to_string(),
            Some(_) if custom => "Personnalisé".to_string(),
            Some(r) => frames::label(r),
        };
        let resp = egui::ComboBox::from_id_salt(ui.id().with("format_cadre"))
            .selected_text(format!("{} {shown}", egui_phosphor::regular::CROP))
            .width(96.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "Aucun").clicked() {
                    self.layout.frame_format = None;
                    self.format_custom = false;
                }
                for r in frames::PRESETS {
                    if ui.selectable_label(current == Some(r) && !custom, frames::label(r)).clicked() {
                        self.layout.frame_format = Some(r);
                        self.format_custom = false;
                    }
                }
                if ui.selectable_label(custom, "Personnalisé…").clicked() {
                    self.layout.frame_format = Some(current.unwrap_or([2.39, 1.0]));
                    self.format_custom = true;
                }
            })
            .response;
        crate::bulles::bulle(resp, "Format", "Cadre d'un format de diffusion, tracé dans le cadre de la caméra (hors-format estompé).", "");
        if let (true, Some(mut r)) = (custom, current) {
            fn drag(v: &mut f32) -> egui::DragValue<'_> {
                egui::DragValue::new(v).range(0.01..=1000.0).speed(0.01).max_decimals(2)
            }
            let a = ui.add(drag(&mut r[0])).on_hover_text("Largeur relative");
            ui.label(":");
            let b = ui.add(drag(&mut r[1])).on_hover_text("Hauteur relative");
            if a.changed() || b.changed() {
                self.layout.frame_format = frames::sanitized(r);
            }
        }
    }

    /// Cadres de composition par-dessus l'image : cadre du format (hors-format estompé) et zones
    /// de sécurité (dans le cadre du format s'il y en a un, sinon dans celui de la caméra). Ils
    /// suivent la caméra : position, zoom, rotation.
    pub(crate) fn paint_frame_overlays(&self, painter: &egui::Painter, rect: Rect) {
        let format = self.layout.frame_format.and_then(frames::sanitized);
        if format.is_none() && !self.layout.safe_frames {
            return;
        }
        let (cam, frame, size) = (self.current_camera(), self.doc.frame, self.canvas_size());
        let quad = |r: Rect| {
            [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
                .map(|p| self.view.to_screen(rect, size, cam.to_stage(frame, p.x, p.y)))
        };
        let full = Rect::from_min_size(Pos2::ZERO, vec2(frame[0] as f32, frame[1] as f32));
        let mut inside = full;
        if let Some(ratio) = format {
            let r = frames::format_rect(frame, ratio);
            let q = quad(r);
            painter.add(band(quad(full), q, Color32::from_black_alpha(140)));
            painter.add(Shape::closed_line(q.to_vec(), Stroke::new(3.0_f32, Color32::from_black_alpha(90))));
            painter.add(Shape::closed_line(q.to_vec(), Stroke::new(1.5_f32, FORMAT)));
            painter.text(q[0] + vec2(4.0, 3.0), Align2::LEFT_TOP, frames::label(ratio), FontId::proportional(11.0), FORMAT);
            inside = r;
        }
        if self.layout.safe_frames {
            for (part, name, corner) in [(frames::ACTION_SAFE, "action", 0), (frames::TITLE_SAFE, "titres", 3)] {
                let q = quad(frames::safe_rect(inside, part));
                let pts = vec![q[0], q[1], q[2], q[3], q[0]];
                painter.add(Shape::line(pts.clone(), Stroke::new(2.5_f32, veil(40))));
                painter.extend(Shape::dashed_line(&pts, Stroke::new(1.2_f32, SAFE), 6.0, 4.0));
                let (at, align) = if corner == 0 {
                    (q[0] + vec2(3.0, 2.0), Align2::LEFT_TOP)
                } else {
                    (q[3] + vec2(3.0, -2.0), Align2::LEFT_BOTTOM)
                };
                painter.text(at, align, name, FontId::proportional(10.0), SAFE);
            }
            // Croix au centre.
            let c = quad(Rect::from_center_size(inside.center(), Vec2::ZERO))[0];
            let s = Stroke::new(1.0_f32, SAFE);
            painter.line_segment([c - vec2(7.0, 0.0), c + vec2(7.0, 0.0)], s);
            painter.line_segment([c - vec2(0.0, 7.0), c + vec2(0.0, 7.0)], s);
        }
    }
}
