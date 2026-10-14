use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Event, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, TouchPhase, Vec2,
};

use crate::app::{App, Tool};
use crate::document::LayerKind;
use crate::pixtools::BDrag;
use crate::vtools::{Handle, VDrag};

const ACCENT: Color32 = Color32::from_rgb(70, 165, 255);

impl App {
    pub(crate) fn canvas_panel(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        self.canvas_rect = rect;
        let size = self.canvas_size();
        if self.fit_pending {
            self.view.fit(rect, size);
            self.fit_pending = false;
        }

        self.canvas_input(ui.ctx(), &response, rect);

        let plan = self.build_plan(self.cur_frame, true, true);
        let painter = ui.painter_at(rect);
        let corners = [
            pos2(0.0, 0.0),
            pos2(size.x, 0.0),
            pos2(size.x, size.y),
            pos2(0.0, size.y),
        ]
        .map(|p| self.view.to_screen(rect, size, p));
        painter.add(Shape::convex_polygon(
            corners.iter().map(|p| *p + vec2(4.0, 5.0)).collect(),
            Color32::from_black_alpha(110),
            Stroke::NONE,
        ));
        if let Some(gpu) = &mut self.gpu {
            // Les calques sont superposés par la carte graphique, directement à leur place.
            let (all, tiles) = self.renderer.take_gpu_dirty();
            gpu.sync(&self.doc, &plan, all, &tiles);
            painter.add(gpu.paint(&plan, corners, rect));
        } else {
            let texture = self.renderer.update(ui.ctx(), &self.doc, plan);
            let mut mesh = egui::Mesh::with_texture(texture);
            let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
            for (pos, uv) in corners.into_iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos,
                    uv,
                    color: Color32::WHITE,
                });
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(Shape::mesh(mesh));
        }

        let shift = ui.input(|i| i.modifiers.shift);
        self.paint_guides(&painter, rect, self.hover_canvas);
        self.paint_vector_overlay(&painter, rect, shift);
        self.paint_cursor(ui.ctx(), &painter, &response);
    }

    fn canvas_input(&mut self, ctx: &egui::Context, response: &egui::Response, rect: Rect) {
        let size = self.canvas_size();
        let (events, hover, scroll, zoom, mods, space, primary_down) = ctx.input(|i| {
            (
                i.events.clone(),
                i.pointer.hover_pos(),
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.modifiers,
                self.keymap.held(i, "vue.pan_libre"),
                i.pointer.primary_down(),
            )
        });
        let modal = self.modal_open();

        // Molette : zoom (ou rotation avec Maj), Ctrl+molette / pincement : zoom.
        if response.hovered() && !modal {
            if let Some(p) = hover {
                if mods.shift {
                    let d = scroll.x + scroll.y;
                    if d != 0.0 {
                        self.view.rotate_around(rect, size, p, d * 0.004);
                    }
                } else {
                    let factor = zoom * (scroll.y * 0.003).exp();
                    if (factor - 1.0).abs() > 1e-4 {
                        self.view.zoom_around(rect, size, p, factor);
                    }
                }
            }
        }

        // Déplacement de la vue : bouton du milieu, Espace + glisser, ou outil Main.
        let panning = self.tool == Tool::Pan || space;
        if response.dragged_by(PointerButton::Middle)
            || (panning && response.dragged_by(PointerButton::Primary))
        {
            self.view.pan += response.drag_delta();
        }

        let on_canvas =
            |pos: Pos2| rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id);

        for event in &events {
            match event {
                // Le stylet arrive sous forme d'événements « tactiles » qui portent la pression ;
                // egui les double d'événements souris, que l'on suit pour la position.
                Event::Touch { phase, force, .. } => {
                    if let Some(f) = force {
                        self.pen_pressure = f.clamp(0.0, 1.0);
                    }
                    if *phase == TouchPhase::Start {
                        self.pen_touch = true;
                    }
                }
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let pen = std::mem::take(&mut self.pen_touch);
                    if modal || panning || !on_canvas(*pos) {
                        continue;
                    }
                    let p = self.view.to_canvas(rect, size, *pos);
                    // Un point de fuite du guide se déplace avec n'importe quel outil.
                    if let Some(k) = self.guide_point_at(*pos) {
                        self.guides.drag = Some(k);
                        continue;
                    }
                    if modifiers.alt || self.tool == Tool::Picker {
                        self.picking = true;
                        self.pick_color(p);
                        continue;
                    }
                    match self.tool {
                        Tool::Fill => self.fill_at(p),
                        Tool::Brush | Tool::Pencil | Tool::Eraser => {
                            let pressure = if pen { self.pen_pressure } else { 1.0 };
                            self.begin_paint(p, pressure, pen);
                        }
                        Tool::Shape => {
                            self.end_stroke();
                            self.playing = false;
                            self.shape_drag = Some((p, p));
                        }
                        Tool::Select => self.select_press(p, *pos, modifiers.shift),
                        Tool::Picker | Tool::Pan => {}
                    }
                }
                Event::PointerMoved(pos) => {
                    let p = self.view.to_canvas(rect, size, *pos);
                    if let Some(k) = self.guides.drag {
                        self.move_guide_point(k, p);
                    } else if let Some(pen) = self.stroke.as_ref().map(|s| s.pen) {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        // Trait aligné sur le guide : rien tant que la direction n'est pas choisie.
                        if let Some(q) = self.guided_point(p) {
                            self.stroke_to(q, pressure);
                        }
                    } else if let Some(pen) = self.erase_pen() {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        if let Some(q) = self.guided_point(p) {
                            self.erase_to(q, pressure);
                        }
                    } else if let Some((_, end)) = &mut self.shape_drag {
                        *end = p;
                    } else if self.vdrag.is_some() {
                        self.select_drag(p, mods.shift);
                    } else if self.bdrag.is_some() {
                        self.bitmap_drag(p, mods.shift);
                    } else if self.picking {
                        self.pick_color(p);
                    }
                }
                Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                }
                | Event::PointerGone => {
                    if let Some((a, b)) = self.shape_drag.take() {
                        self.commit_shape(a, b, mods.shift);
                    }
                    if self.vdrag.is_some() {
                        self.finish_vdrag(mods.shift);
                    }
                    if self.bdrag.is_some() {
                        self.finish_bdrag();
                    }
                    self.guides.drag = None;
                    self.end_stroke();
                    self.picking = false;
                }
                _ => {}
            }
        }
        // Relâchement manqué (perte de focus…) : on clôt le geste quand même.
        if !primary_down {
            self.end_stroke();
            self.picking = false;
            self.guides.drag = None;
        }

        self.hover_canvas = hover
            .filter(|p| rect.contains(*p))
            .map(|p| self.view.to_canvas(rect, size, p));
    }

    /// Sélection vectorielle, lasso et aperçu des formes, par-dessus le dessin.
    fn paint_vector_overlay(&self, painter: &egui::Painter, rect: Rect, shift: bool) {
        let size = self.canvas_size();
        let to_s = |p: Pos2| self.view.to_screen(rect, size, p);

        if let Some((a, b)) = self.shape_drag {
            let pts: Vec<Pos2> = self.shape_points(a, b, shift).into_iter().map(to_s).collect();
            let width = (self.shape.width * self.view.zoom).clamp(1.0, 40.0);
            painter.add(Shape::line(pts, Stroke::new(width, self.preview_color())));
        }

        // Lasso ou rectangle en cours de tracé (vectoriel ou bitmap).
        let drawing_lasso: Option<Vec<Pos2>> = match (&self.vdrag, &self.bdrag) {
            (Some(VDrag::Lasso(points)), _) | (_, Some(BDrag::Lasso { points, .. })) => Some(points.clone()),
            (_, Some(BDrag::Rect { from, to, .. })) => Some(crate::pixsel::rect_polygon(Rect::from_two_pos(*from, *to))),
            _ => None,
        };
        if let Some(points) = drawing_lasso.filter(|p| p.len() >= 2) {
            let pts: Vec<Pos2> = points.iter().map(|p| to_s(*p)).collect();
            painter.add(Shape::closed_line(pts.clone(), Stroke::new(2.5_f32, Color32::from_black_alpha(120))));
            painter.add(Shape::closed_line(pts, Stroke::new(1.0_f32, ACCENT)));
        }

        // Contour de la zone bitmap sélectionnée : pointillés noirs et blancs.
        for poly in self.bitmap_selection_outline() {
            let mut pts: Vec<Pos2> = poly.into_iter().map(to_s).collect();
            if let Some(&first) = pts.first() {
                pts.push(first);
            }
            painter.add(Shape::line(pts.clone(), Stroke::new(1.0_f32, Color32::WHITE)));
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0_f32, Color32::BLACK), 5.0, 5.0));
        }

        let Some(b) = self.selection_bounds() else {
            return;
        };
        if let Some(sel) = &self.vsel {
            if let Some(d) = self.drawing(sel.cel) {
                for e in sel.items.iter().filter_map(|&i| d.elements.get(i)) {
                    for line in e.outline() {
                        if line.len() >= 2 {
                            painter.add(Shape::line(
                                line.into_iter().map(to_s).collect(),
                                Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.85)),
                            ));
                        }
                    }
                }
            }
        }
        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(to_s);
        painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(1.0_f32, ACCENT)));
        for (handle, q) in self.handle_points(b) {
            let s = to_s(q);
            match handle {
                Handle::Rotate => {
                    painter.line_segment([to_s(pos2(b.center().x, b.min.y)), s], Stroke::new(1.0_f32, ACCENT));
                    painter.circle(s, 5.0, Color32::WHITE, Stroke::new(1.5_f32, ACCENT));
                }
                _ => {
                    painter.rect(
                        Rect::from_center_size(s, Vec2::splat(8.0)),
                        1.0,
                        Color32::WHITE,
                        Stroke::new(1.5_f32, ACCENT),
                        StrokeKind::Middle,
                    );
                }
            }
        }
    }

    fn paint_cursor(&self, ctx: &egui::Context, painter: &egui::Painter, response: &egui::Response) {
        let Some(p) = response.hover_pos() else { return };
        if self.modal_open() {
            return;
        }
        let (space, alt) = ctx.input(|i| (self.keymap.held(i, "vue.pan_libre"), i.modifiers.alt));
        if self.guides.drag.is_some() || self.guide_point_at(p).is_some() {
            ctx.set_cursor_icon(if self.guides.drag.is_some() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            });
            return;
        }
        if space || self.tool == Tool::Pan {
            ctx.set_cursor_icon(if response.dragged() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            });
            return;
        }
        if alt || matches!(self.tool, Tool::Picker | Tool::Fill | Tool::Shape) {
            ctx.set_cursor_icon(CursorIcon::Crosshair);
            return;
        }
        if self.tool == Tool::Select {
            let icon = match self.handle_at(p) {
                Some(Handle::Rotate) => CursorIcon::Alias,
                Some(Handle::Corner(0 | 2)) => CursorIcon::ResizeNwSe,
                Some(Handle::Corner(_)) => CursorIcon::ResizeNeSw,
                Some(Handle::Edge(0 | 2)) => CursorIcon::ResizeVertical,
                Some(Handle::Edge(_)) => CursorIcon::ResizeHorizontal,
                _ => match (self.hover_canvas, self.selection_bounds()) {
                    (Some(c), Some(b)) if b.contains(c) => CursorIcon::Move,
                    _ => CursorIcon::Default,
                },
            };
            ctx.set_cursor_icon(icon);
            return;
        }
        let Some(brush) = self.current_brush() else { return };
        ctx.set_cursor_icon(CursorIcon::None);
        let r = (brush.size * 0.5 * self.view.zoom).max(1.5);
        painter.circle_stroke(p, r, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
        painter.circle_stroke(p, r + 1.0, Stroke::new(1.0_f32, Color32::from_white_alpha(150)));
        let cross = Stroke::new(1.0_f32, Color32::from_gray(230));
        painter.line_segment([p - vec2(4.0, 0.0), p + vec2(4.0, 0.0)], cross);
        painter.line_segment([p - vec2(0.0, 4.0), p + vec2(0.0, 4.0)], cross);
        if self.layer().kind == LayerKind::Vector {
            // Petit repère « V » : le trait sera vectoriel.
            painter.text(
                p + vec2(r + 4.0, -r - 4.0),
                egui::Align2::LEFT_BOTTOM,
                "V",
                egui::FontId::proportional(10.0),
                ACCENT,
            );
        }
    }
}
