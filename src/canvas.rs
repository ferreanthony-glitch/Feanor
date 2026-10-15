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
        let mut rect = ui.available_rect_before_wrap();
        // Barre de la vue (plateau / caméra, cadres de composition), sauf dans une vignette.
        if !self.in_card {
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), crate::viewbar::VIEW_BAR_H));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar).id_salt("barre_vue"));
            self.view_bar(&mut child);
            rect.min.y = bar.max.y;
            // Plateau en 3D (9e) : à la place du dessin à plat.
            if self.stage3d.on && !self.camera_view {
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                self.canvas_rect = rect;
                self.stage3d_panel(ui, rect, &response);
                return;
            }
        }
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        // Un clic dans le dessin reprend la main au montage : l'édition vise de nouveau le dessin.
        if response.is_pointer_button_down_on() {
            self.panel_focus = false;
        }
        self.canvas_rect = rect;
        let size = self.canvas_size();
        // En vue caméra, la vue suit la caméra du plan (avant de lire la souris).
        self.apply_camera_view(rect);
        if self.fit_pending && !self.camera_view {
            // On cadre la caméra (centrée sur la feuille), avec un peu de hors-champ autour.
            let [fw, fh] = self.doc.frame;
            let margin = if [fw, fh] == [self.doc.width, self.doc.height] { 1.0 } else { 1.12 };
            self.view.fit(rect, vec2(fw as f32, fh as f32) * margin);
            self.fit_pending = false;
        }

        self.canvas_input(ui.ctx(), &response, rect);
        self.update_ref3d(ui.ctx());
        self.update_text_preview(ui.ctx());
        self.check_persp();

        let plan = self.build_plan(self.cur_frame, true, true);
        let painter = ui.painter_at(rect);
        let stage_corners = [pos2(0.0, 0.0), pos2(size.x, 0.0), pos2(size.x, size.y), pos2(0.0, size.y)];
        let corners_h = stage_corners.map(|p| self.view.to_screen_h(rect, size, p));
        let corners = corners_h.map(|c| c.0);
        // Vue en perspective (caméra tournée, 9f) : w des coins pour la carte graphique.
        let ws = self.view.persp.is_some().then(|| corners_h.map(|c| c.1));
        if ws.is_none() {
            painter.add(Shape::convex_polygon(
                corners.iter().map(|p| *p + vec2(4.0, 5.0)).collect(),
                Color32::from_black_alpha(110),
                Stroke::NONE,
            ));
        }
        // Dessins des images qui viennent, à envoyer d'avance à la carte graphique.
        let ahead = match &self.gpu {
            Some(gpu) => crate::playback::upcoming_cels(
                &self.doc,
                self.play_position(),
                &plan,
                gpu.ahead_capacity(&self.doc, plan.items.len()),
            ),
            None => Vec::new(),
        };
        if let Some(gpu) = &mut self.gpu {
            // Les calques sont superposés par la carte graphique, directement à leur place.
            let (all, tiles) = self.renderer.take_gpu_dirty();
            gpu.sync(&self.doc, &plan, all, &tiles, &ahead);
            painter.add(gpu.paint(&plan, corners, ws, rect, [self.doc.width, self.doc.height]));
        } else {
            let texture = self.renderer.update(ui.ctx(), &self.doc, plan);
            let mut mesh = egui::Mesh::with_texture(texture);
            // En perspective, egui plaque la texture par triangle (sans perspective) : on
            // subdivise, et on laisse ce qui passe derrière la caméra.
            let n = if ws.is_some() { 24 } else { 1 };
            let mut front = Vec::with_capacity((n + 1) * (n + 1));
            for j in 0..=n {
                for i in 0..=n {
                    let uv = pos2(i as f32 / n as f32, j as f32 / n as f32);
                    let (pos, w) = self.view.to_screen_h(rect, size, pos2(uv.x * size.x, uv.y * size.y));
                    front.push(w > 0.0);
                    mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: Color32::WHITE });
                }
            }
            for j in 0..n {
                for i in 0..n {
                    let a = j * (n + 1) + i;
                    let (b, c, d) = (a + 1, a + n + 1, a + n + 2);
                    if front[a] && front[b] && front[c] && front[d] {
                        mesh.add_triangle(a as u32, b as u32, d as u32);
                        mesh.add_triangle(a as u32, d as u32, c as u32);
                    }
                }
            }
            painter.add(Shape::mesh(mesh));
        }

        self.paint_ref3d(&painter, rect);
        self.paint_camera(&painter, rect);
        if !self.in_card {
            self.paint_frame_overlays(&painter, rect);
        }
        self.paint_layer_transform(&painter);
        let shift = ui.input(|i| i.modifiers.shift);
        self.paint_guides(&painter, rect, self.hover_canvas);
        self.paint_vector_overlay(&painter, rect, shift);
        self.paint_text_overlay(&painter);
        self.paint_persp_overlay(&painter);
        self.paint_stroke_tip(&painter);
        if !self.in_card {
            self.paint_canvas_title(&painter, rect);
        }
        self.paint_cursor(ui.ctx(), &painter, &response);
        self.text_edit_window(ui.ctx());
    }

    /// Scène et panneau en cours, en haut à droite du dessin.
    fn paint_canvas_title(&self, painter: &egui::Painter, rect: Rect) {
        let text = crate::storyboard::panel_long_title(&self.doc, self.cur_panel);
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(17.0), Color32::from_gray(235));
        let at = egui::pos2(rect.right() - galley.size().x - 14.0, rect.top() + 8.0);
        let bg = Rect::from_min_size(at - vec2(8.0, 4.0), galley.size() + vec2(16.0, 8.0));
        painter.rect_filled(bg, 4.0, Color32::from_black_alpha(150));
        painter.galley(at, galley, Color32::from_gray(235));
    }

    /// Pendant un trait : relie le dernier point posé à la pointe du stylet, pour que le trait
    /// reste attaché à la croix (le lissage le fait sinon traîner un peu derrière). Rien n'est
    /// écrit dans le dessin : le trait définitif arrive dans les images suivantes.
    fn paint_stroke_tip(&self, painter: &egui::Painter) {
        let Some(s) = &self.stroke else { return };
        let (Some((last, raw, r)), Some((rgb, alpha))) = (s.tip(), s.tip_paint()) else { return };
        let (a, b) = (self.layer_to_screen(last), self.layer_to_screen(raw));
        let w = (r * self.layer_zoom()).max(0.5);
        let color = Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8);
        if a.distance(b) > 0.5 {
            painter.line_segment([a, b], Stroke::new(2.0 * w, color));
        }
        painter.circle_filled(b, w, color);
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

        // Partie visible seulement (une vignette de la planche peut dépasser de la vue).
        let on_canvas = |pos: Pos2| {
            rect.contains(pos) && response.interact_rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id)
        };

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
                    // `stage` : point de la feuille (guides, caméra, pipette) ; `p` : point du dessin
                    // du calque courant, qui peut être transformé (pinceaux, formes, sélection…).
                    let stage = self.view.to_canvas(rect, size, *pos);
                    let p = self.to_layer(stage);
                    // Un point de fuite du guide se déplace avec n'importe quel outil.
                    if let Some(k) = self.guide_point_at(*pos) {
                        self.guides.drag = Some(k);
                        continue;
                    }
                    if modifiers.alt || self.tool == Tool::Picker {
                        self.picking = true;
                        self.pick_color(stage);
                        continue;
                    }
                    match self.tool {
                        Tool::Fill => self.fill_at(p),
                        Tool::Brush | Tool::Pencil | Tool::Eraser => {
                            let pressure = if pen { self.pen_pressure } else { 1.0 };
                            self.begin_guided_stroke(stage);
                            self.begin_paint(p, pressure, pen);
                        }
                        Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline => self.shape_press(p, modifiers.shift),
                        Tool::Text => self.text_press(p),
                        Tool::Stamp => {
                            let pressure = if pen { self.pen_pressure } else { 1.0 };
                            self.stamp_press(p, pressure, pen);
                        }
                        Tool::Clone => self.clone_press(p, modifiers.command || modifiers.ctrl),
                        Tool::Perspective => self.persp_press(p, *pos),
                        Tool::Select => self.select_press(p, *pos, modifiers.shift),
                        Tool::Camera => self.camera_press(stage, *pos),
                        Tool::Transform => self.transform_press(stage, *pos),
                        Tool::Ref3d => self.ref3d_press(*pos, *modifiers),
                        Tool::Picker | Tool::Pan => {}
                    }
                }
                Event::PointerMoved(pos) => {
                    let stage = self.view.to_canvas(rect, size, *pos);
                    let p = self.to_layer(stage);
                    if let Some(k) = self.guides.drag {
                        self.move_guide_point(k, stage);
                    } else if let Some(pen) = self.stroke.as_ref().map(|s| s.pen) {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        // Trait aligné sur le guide (sur la feuille) : rien tant que la direction
                        // n'est pas choisie.
                        if let Some(q) = self.guided_point(stage) {
                            self.stroke_to(self.to_layer(q), pressure);
                        }
                    } else if let Some(pen) = self.stamp_stroke.as_ref().map(|s| s.pen) {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        self.stamp_to(p, pressure);
                    } else if self.clone_stroke.is_some() {
                        self.clone_to(p);
                    } else if let Some(pen) = self.erase_pen() {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        if let Some(q) = self.guided_point(stage) {
                            self.erase_to(self.to_layer(q), pressure);
                        }
                    } else if self.persp_drag(p) {
                    } else if self.shape_bend_move(p) {
                    } else if let Some((_, end)) = &mut self.shape_drag {
                        *end = p;
                    } else if self.vdrag.is_some() {
                        self.select_drag(p, mods.shift);
                    } else if self.bdrag.is_some() {
                        self.bitmap_drag(p, mods.shift);
                    } else if self.cam_drag.is_some() {
                        self.camera_drag(stage, mods.shift);
                    } else if self.layer_drag.is_some() {
                        self.transform_drag(stage, mods.shift);
                    } else if self.ref3d.dragging() {
                        self.ref3d_drag_to(*pos);
                    } else if self.picking {
                        self.pick_color(stage);
                    }
                }
                Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                }
                | Event::PointerGone => {
                    self.shape_release(mods.shift);
                    self.persp_release();
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
    fn paint_vector_overlay(&self, painter: &egui::Painter, _rect: Rect, shift: bool) {
        // Tout ce qui suit est en coordonnées du dessin du calque courant (transformé ou non).
        let to_s = |p: Pos2| self.layer_to_screen(p);

        self.paint_shape_overlay(painter, shift);

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
        if alt || matches!(self.tool, Tool::Picker | Tool::Fill) || self.tool.is_shape() {
            ctx.set_cursor_icon(CursorIcon::Crosshair);
            return;
        }
        if self.tool == Tool::Text {
            ctx.set_cursor_icon(CursorIcon::Text);
            return;
        }
        if self.tool == Tool::Perspective {
            ctx.set_cursor_icon(self.persp_cursor(p));
            return;
        }
        if self.paint_stamp_cursor(ctx, painter, p) {
            return;
        }
        if self.tool == Tool::Ref3d {
            ctx.set_cursor_icon(if self.ref3d.dragging() { CursorIcon::Grabbing } else { CursorIcon::Grab });
            return;
        }
        let handle = match self.tool {
            Tool::Camera => Some(self.camera_handle_at(p)),
            Tool::Transform => Some(self.transform_handle_at(p)),
            _ => None,
        };
        if let Some(h) = handle {
            ctx.set_cursor_icon(match h {
                Some(crate::camtools::CamHandle::Move) => CursorIcon::Move,
                Some(crate::camtools::CamHandle::Zoom) => CursorIcon::ResizeNwSe,
                Some(crate::camtools::CamHandle::Rotate) => CursorIcon::Alias,
                None => CursorIcon::Default,
            });
            return;
        }
        if self.tool == Tool::Select {
            let icon = match self.handle_at(p) {
                Some(Handle::Rotate) => CursorIcon::Alias,
                Some(Handle::Corner(0 | 2)) => CursorIcon::ResizeNwSe,
                Some(Handle::Corner(_)) => CursorIcon::ResizeNeSw,
                Some(Handle::Edge(0 | 2)) => CursorIcon::ResizeVertical,
                Some(Handle::Edge(_)) => CursorIcon::ResizeHorizontal,
                _ => match (self.hover_canvas.map(|c| self.to_layer(c)), self.selection_bounds()) {
                    (Some(c), Some(b)) if b.contains(c) => CursorIcon::Move,
                    _ => CursorIcon::Default,
                },
            };
            ctx.set_cursor_icon(icon);
            return;
        }
        let Some(brush) = self.current_brush() else { return };
        // La croix du système suit le stylet sans retard ; le cercle (taille du pinceau), dessiné
        // dans l'image, arrive une image plus tard.
        let system = self.settings.system_cursor;
        ctx.set_cursor_icon(if system { CursorIcon::Crosshair } else { CursorIcon::None });
        let r = (brush.size * 0.5 * self.layer_zoom()).max(1.5);
        painter.circle_stroke(p, r, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
        painter.circle_stroke(p, r + 1.0, Stroke::new(1.0_f32, Color32::from_white_alpha(150)));
        if !system {
            let cross = Stroke::new(1.0_f32, Color32::from_gray(230));
            painter.line_segment([p - vec2(4.0, 0.0), p + vec2(4.0, 0.0)], cross);
            painter.line_segment([p - vec2(0.0, 4.0), p + vec2(0.0, 4.0)], cross);
        }
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
