//! Barre au-dessus du dessin (9d) : vue du plateau (toute la feuille, avec le cadre de la caméra)
//! ou vue caméra (ce que filme la caméra du plan), et cadres de composition tracés dans le
//! cadre de la caméra : zones de sécurité et cadre d'un format de diffusion (9:16, 1:1…).

use eframe::egui::{self, vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::App;
use crate::frames;

/// Hauteur de la barre.
pub(crate) const VIEW_BAR_H: f32 = 26.0;

/// Ce que font les touches avec l'outil `tool`, en une ligne sous le dessin (Anthony,
/// 2026-10-18 : les gestes à touche, comme l'enveloppe ou le lasso de Ctrl, ne se devinent pas).
pub(crate) fn tool_hint(tool: crate::app::Tool) -> &'static str {
    use crate::app::Tool;
    match tool {
        Tool::Brush | Tool::Pencil | Tool::Eraser => "Alt + clic : pipette · Ctrl tenu : lasso · Espace : déplacer la vue · ^ $ : taille",
        Tool::Fill => "Clic : remplir · Alt + clic : pipette",
        Tool::Cutter => "Glisser en travers des bouts à enlever · clic : un bout",
        Tool::Select => "Lasso (Maj : ajouter · Ctrl+Maj : tous les calques) · boîte : dedans déplacer · coin ou côté : échelle (Maj : proportions) · hors d'un coin : tourner · bord : incliner · point rond : pivot · flèches : 1 px (Maj : 10)",
        Tool::SelectColor => "Clic : une couleur (Maj : ajouter) · la boîte se manie comme avec la Sélection",
        Tool::Line => "Glisser : tracer · Maj : par 15°",
        Tool::Rect | Tool::Ellipse => "Glisser : tracer · Maj : carré ou cercle",
        Tool::Polyline => "Un clic par point (Maj : par 15°) · double-clic ou Entrée : terminer",
        Tool::Text => "Clic : poser un texte · Ctrl + Entrée : valider · Échap : annuler",
        Tool::Transform => "Coin : agrandir (Maj : proportions) · côté : étirer · rond : tourner · Ctrl+R : annuler",
        Tool::Perspective => "Enveloppe ou perspective (Réglages) · tirer un coin ou une poignée ronde · l'intérieur : déplacer · Entrée : terminer",
        Tool::Picker => "Clic : prendre la couleur",
        Tool::Pan => "Glisser : déplacer la vue",
        Tool::Camera => "Glisser : déplacer (Maj : à l'horizontale ou à la verticale) · coin : zoomer · rond : tourner (Maj : 15°)",
    }
}
/// Hauteur de la barre des commandes de la vue, sous le dessin.
pub(crate) const TOOLS_BAR_H: f32 = 30.0;
/// Couleurs des zones de sécurité et du cadre de format.
const SAFE: Color32 = crate::theme::CADRAGE_SECURITE;
const FORMAT: Color32 = crate::theme::CADRAGE_FORMAT;

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
    /// Barre sous le dessin : les commandes de la vue (Anthony, 2026-10-17 : « ajoute les outils
    /// de la fenêtre Vue sous l'espace de travail » ; puis « change la disposition comme ceci » :
    /// celles de la barre du haut y passent). Vues (planche, caméra, 3D) ; ajuster, 100 %, zoom ;
    /// rotation ; miroir ; pelure d'oignon, table lumineuse. En 3D, seulement les vues. (Zones de
    /// sécurité et format : barre au-dessus du dessin.)
    pub(crate) fn view_tools_bar(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::entete());
        let in3d = self.espace3d.on && !self.camera_view;
        // Étroit : curseurs plus courts, sans la force de la table lumineuse.
        let narrow = ui.available_width() < 700.0;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            let sep = |ui: &mut egui::Ui| {
                ui.add_space(3.0);
                ui.separator();
                ui.add_space(3.0);
            };
            ui.add_space(4.0);
            let thumbs = self.board_view == crate::app::BoardView::Panels && self.mode == crate::app::Mode::Storyboard;
            self.command_button(ui, ph::SQUARES_FOUR, "vue.planches", "Vue dessin / vignettes", "Dessiner le plan, ou voir toutes ses images de storyboard en planche.", Some(thumbs));
            self.command_button(ui, ph::FRAME_CORNERS, "camera.vue", "Vue à travers la caméra", "Voir ce que filme la caméra du plan.", Some(self.camera_view));
            self.command_button(ui, ph::CUBE, "espace3d.basculer", "Espace 3D", "Les calques (cartes) et la caméra du plan dans un seul espace : les déplacer, les tourner, les mettre à l'échelle, pour la profondeur et la parallaxe.", Some(in3d));
            if in3d {
                return;
            }
            sep(ui);
            self.command_button(ui, ph::ARROWS_OUT, "vue.ajuster", "Ajuster à la fenêtre", "Toute la feuille dans la vue.", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS, "vue.reinit_zoom", "Zoom 100 %", "Un pixel du dessin pour un pixel de l'écran.", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS_MINUS, "vue.zoom_arriere", "Zoom arrière", "", None);
            self.command_button(ui, ph::MAGNIFYING_GLASS_PLUS, "vue.zoom_avant", "Zoom avant", "", None);
            sep(ui);
            self.command_button(ui, ph::ARROW_ARC_LEFT, "vue.rotation_antihoraire", "Tourner la vue", "Dans le sens antihoraire ; le dessin ne change pas.", None);
            // Rotation de la vue : un curseur court et sa valeur.
            ui.spacing_mut().slider_width = if narrow { 60.0 } else { 100.0 };
            let mut degrees = self.view.rotation.to_degrees();
            let resp = ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°").fixed_decimals(0));
            if crate::bulles::bulle(resp, "Rotation de la vue", "Tourner la vue ; le dessin ne change pas. Double-clic sur la valeur pour taper un angle.", "").changed() {
                let delta = degrees.to_radians() - self.view.rotation;
                let (rect, size) = (self.canvas_rect, self.canvas_size());
                self.view.rotate_around(rect, size, rect.center(), delta);
            }
            self.command_button(ui, ph::ARROW_ARC_RIGHT, "vue.rotation_horaire", "Tourner la vue", "Dans le sens horaire ; le dessin ne change pas.", None);
            sep(ui);
            self.command_button(ui, ph::FLIP_HORIZONTAL, "vue.miroir", "Miroir de la vue", "Voir le dessin retourné, pour vérifier ses proportions ; le dessin ne change pas.", Some(self.view.flip));
            sep(ui);
            self.command_button(ui, ph::CIRCLES_THREE, "vue.pelure", "Pelure d'oignon", "Par défaut, les dessins d'avant en rouge, ceux d'après en vert.", Some(self.onion.enabled));
            // Réglages de la pelure à côté de son icône (Anthony, 2026-10-18) : combien avant et
            // après, et le panneau complet (comme dans la barre de la timeline).
            if self.onion.enabled && !narrow {
                let max = crate::onion::MAX_NEIGHBOURS;
                let resp = ui.add(egui::DragValue::new(&mut self.onion.before).range(0..=max).prefix("‹ "));
                crate::bulles::bulle(resp, "Avant", "Nombre de dessins (ou d'images) montrés avant.", "");
                let resp = ui.add(egui::DragValue::new(&mut self.onion.after).range(0..=max).suffix(" ›"));
                crate::bulles::bulle(resp, "Après", "Nombre de dessins (ou d'images) montrés après.", "");
            }
            let key = self.raccourci("pelure.panneau");
            let resp = ui.add(egui::Button::new(crate::bulles::symbole_etat(ph::GEAR, self.onion_window)).selected(self.onion_window).frame(false));
            if crate::bulles::bulle(resp, "Réglages de la pelure d'oignon", "Avant et après, mode, devant, couleurs, opacité de chaque image, réglages rangés, décalage.", &key).clicked() {
                self.onion_window = !self.onion_window;
            }
            self.command_button(ui, ph::LIGHTBULB, "vue.table_lumineuse", "Table lumineuse", "Les autres calques du plan estompés ; le calque courant reste net.", Some(self.light_table));
            if self.light_table && !narrow {
                ui.spacing_mut().slider_width = 70.0;
                let resp = ui.add(egui::Slider::new(&mut self.light_table_strength, 0.05..=0.8).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                crate::bulles::bulle(resp, "Autres calques", "Combien ils restent visibles.", "");
            }
            // Ce que font les touches avec l'outil courant (Anthony, 2026-10-18, axe d'ergonomie 4).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                // Sur l'aplat (`fondui.rs`), ce que font les outils : « Aplat de <calque> ».
                let aplat = self.aplat_en_cible().then(|| self.aplat_indication());
                let hint = match self.tool {
                    crate::app::Tool::Fill | crate::app::Tool::Cutter | crate::app::Tool::Eraser | crate::app::Tool::Brush | crate::app::Tool::Pencil if aplat.is_some() => {
                        aplat.as_deref().unwrap_or_default()
                    }
                    // Plusieurs calques choisis (Ctrl ou Maj + clic sur leur ligne).
                    crate::app::Tool::Select if self.chosen_layers().len() >= 2 => "Calques choisis : le lasso prend sur tous · une boîte commune : dedans déplacer · coin : échelle · hors d'un coin : tourner · bord : incliner · point rond : pivot · flèches : 1 px (Maj : 10) · Suppr",
                    crate::app::Tool::Transform if self.chosen_layers().len() >= 2 => "Calques choisis ensemble : glisser dans la boîte : déplacer · coin : agrandir · rond : tourner · Ctrl+R : remettre en place",
                    crate::app::Tool::Perspective if self.chosen_layers().len() >= 2 => "Enveloppe ou perspective : le calque courant seul (choisis un seul calque pour déformer)",
                    t => tool_hint(t),
                };
                ui.add(egui::Label::new(egui::RichText::new(hint).small().weak()).truncate()).on_hover_text(hint);
            });
        });
    }

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
        ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::entete());
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
            // Le bouton « Espace 3D » n'est que dans la barre du bas (`view_tools_bar`, Anthony 2026-10-20 : pas
            // deux fois la même icône) ; ici, seulement ses outils quand il est actif.
            let in_scene = self.espace3d.on && !cam;
            if in_scene {
                self.espace3d_tools(ui);
                return;
            }
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
            ui.add_space(4.0);
            self.framing_aids_button(ui);
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
        if format.is_none() && !self.layout.safe_frames && !self.cadrage.any_on() {
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
        // Aides au cadrage (`cadrage.rs`) : tiers, grille d'or, spirale, diagonales, croix, dans le
        // cadre du format s'il y en a un.
        self.paint_framing_aids(painter, inside, &|p| self.view.to_screen(rect, size, cam.to_stage(frame, p.x, p.y)));
        if self.layout.safe_frames {
            let (action, title) = self.cadrage.safe.parts();
            for (part, name, corner) in [(action, "action", 0), (title, "titres", 3)] {
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
