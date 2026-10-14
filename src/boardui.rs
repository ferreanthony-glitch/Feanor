//! Planche de la vue Vignettes (phase 7) : interface. Géométrie et notes dans `board.rs`.
//!
//! Un fond gris clair où l'on note librement (pinceau, crayon, formes : un trait de note ;
//! gomme : efface les notes touchées). Les vignettes y sont rangées par scène ; celle du panneau
//! courant est le dessin lui-même (on y dessine, l'outil Caméra y recadre). Molette : défiler ;
//! Ctrl + molette : zoom ; Espace, bouton du milieu ou outil Main : déplacer la planche. Clic,
//! Ctrl + clic, Maj + clic ou lasso (outil Sélection) : choisir des panneaux ; les glisser (par
//! leur titre pour le panneau courant) pour les déplacer ; clic droit : menus.

use std::time::Instant;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Event, FontId, Key, Modifiers, PointerButton, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Vec2,
};

use crate::app::{App, BoardView, Mode, Tool};
use crate::board::{self, BoardLayout, CardPlace, NoteStroke, CARD_MAX, CARD_MIN};
use crate::storyboard::{duration_label, panel_title};
use crate::vector::VPoint;

/// Fond de la planche, titres des vignettes, encre des textes.
const BOARD_BG: Color32 = Color32::from_gray(198);
const HEADER_BG: Color32 = Color32::from_gray(230);
const INK: Color32 = Color32::from_gray(38);
/// Épaisseur des notes : la taille du pinceau, réduite (les notes sont à l'échelle de la planche).
const NOTE_SCALE: f32 = 0.5;
const ZOOM_MIN: f32 = 0.2;
const ZOOM_MAX: f32 = 6.0;

/// Geste en cours sur la planche.
pub(crate) enum BoardDrag {
    /// Panneaux glissés ; `at` : le pointeur, sur la planche.
    Move { ids: Vec<u64>, at: Pos2 },
    /// Trait de note ; `pen` : tracé au stylet (la pression compte).
    Note { points: Vec<VPoint>, pen: bool },
    /// Gomme sur les notes ; `recorded` : l'étape d'annulation est prise.
    Erase { recorded: bool },
    /// Rectangle de choix ; `add` : ajouter aux panneaux déjà choisis.
    Rubber { start: Pos2, now: Pos2, add: bool },
}

/// Passage planche ↔ écran.
#[derive(Clone, Copy)]
struct Xf {
    origin: Pos2,
    zoom: f32,
}

impl Xf {
    fn to_screen(self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.zoom
    }
    fn rect(self, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(r.min), self.to_screen(r.max))
    }
    fn to_board(self, s: Pos2) -> Pos2 {
        ((s - self.origin) / self.zoom).to_pos2()
    }
}

impl App {
    /// La planche est-elle à l'écran (storyboard, vue Vignettes) ?
    pub(crate) fn board_shown(&self) -> bool {
        self.mode == Mode::Storyboard && self.board_view == BoardView::Panels
    }

    /// Zoom 100 %, en haut à gauche de la planche.
    pub(crate) fn board_fit(&mut self) {
        self.board_zoom = 1.0;
        self.board_pan = Vec2::ZERO;
    }

    /// Zoom par cran (raccourcis), autour du coin haut gauche de la vue.
    pub(crate) fn board_zoom_step(&mut self, factor: f32) {
        let z = (self.board_zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        self.board_pan *= z / self.board_zoom;
        self.board_zoom = z;
    }

    fn board_zoom_at(&mut self, view: Rect, p: Pos2, factor: f32) {
        let b = (p - view.min - self.board_pan) / self.board_zoom;
        let z = (self.board_zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        self.board_pan = p - view.min - b * z;
        self.board_zoom = z;
    }

    /// Les panneaux choisis, dans l'ordre du film (le panneau courant en fait toujours partie).
    fn board_selection(&self) -> Vec<u64> {
        let cur = self.doc.panels[self.cur_panel].id;
        self.doc.panels.iter().map(|p| p.id).filter(|id| *id == cur || self.board_sel.contains(id)).collect()
    }

    /// Retire les panneaux `ids` (une étape d'annulation).
    pub(crate) fn delete_panels(&mut self, ids: &[u64]) {
        self.end_stroke();
        let snap = self.snapshot();
        match self.doc.remove_panels(ids) {
            Some(i) => {
                self.commit_structure(snap);
                self.cur_panel = i;
                self.clamp_nav();
                self.board_sel.clear();
                self.thumbs.prune(&self.doc);
                self.status = match ids.len() {
                    1 => "Panneau supprimé.".into(),
                    n => format!("{n} panneaux supprimés."),
                };
            }
            None => self.status = "Il faut garder au moins un panneau.".into(),
        }
    }

    pub(crate) fn board_view(&mut self, ui: &mut egui::Ui) {
        self.board_toolbar(ui);
        let view = ui.available_rect_before_wrap();
        let bg = ui.allocate_rect(view, Sense::click_and_drag());
        let painter = ui.painter_at(view);
        painter.rect_filled(view, 0.0, BOARD_BG);
        self.board_sel.retain(|id| self.doc.panels.iter().any(|p| p.id == *id));
        let layout = board::layout(&self.doc, self.layout.card_size, view.width());

        let (events, hover, scroll, zoom_delta, mods, space, any_down, escape) = ui.input(|i| {
            (
                i.events.clone(),
                i.pointer.hover_pos(),
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.modifiers,
                self.keymap.held(i, "vue.pan_libre"),
                i.pointer.any_down(),
                i.key_pressed(Key::Escape),
            )
        });
        let pan_mode = self.tool == Tool::Pan || space;
        let over = hover.filter(|p| view.contains(*p) && ui.ctx().layer_id_at(*p) == Some(bg.layer_id));
        // Pression du stylet (Windows Ink : événements « tactiles »).
        let touch = events.iter().rev().find_map(|e| match e {
            Event::Touch { force: Some(f), .. } => Some(f.clamp(0.0, 1.0)),
            _ => None,
        });
        if let Some(f) = touch {
            self.pen_pressure = f;
        }

        // Molette : défiler ; Ctrl + molette (ou pincement) : zoom autour du pointeur.
        if let Some(p) = over {
            if (zoom_delta - 1.0).abs() > 1e-4 {
                self.board_zoom_at(view, p, zoom_delta);
            } else if scroll != Vec2::ZERO {
                self.board_pan += scroll;
            }
        }
        if bg.dragged_by(PointerButton::Middle) || (pan_mode && bg.dragged_by(PointerButton::Primary)) {
            self.board_pan += bg.drag_delta();
        }
        self.clamp_board_pan(view, layout.size);
        let xf = Xf { origin: view.min + self.board_pan, zoom: self.board_zoom };

        // Gestes commencés sur le fond.
        if bg.drag_started_by(PointerButton::Primary) && !pan_mode {
            let start = xf.to_board(ui.input(|i| i.pointer.press_origin()).unwrap_or(view.center()));
            self.board_drag = Some(match self.tool {
                Tool::Eraser => BoardDrag::Erase { recorded: false },
                Tool::Select | Tool::Camera | Tool::Transform | Tool::Picker | Tool::Perspective | Tool::Ref3d => {
                    BoardDrag::Rubber { start, now: start, add: mods.ctrl || mods.shift }
                }
                _ => BoardDrag::Note { points: vec![self.note_point(start, touch.is_some())], pen: touch.is_some() },
            });
        }
        if bg.clicked() && !mods.ctrl && !mods.shift {
            self.board_sel.clear();
        }
        if escape && matches!(self.board_drag, Some(BoardDrag::Move { .. } | BoardDrag::Rubber { .. })) {
            self.board_drag = None;
        }
        let pointer = ui.input(|i| i.pointer.latest_pos()).map(|p| xf.to_board(p));
        self.update_board_drag(pointer, touch.is_some(), xf.zoom);

        // Titres des scènes, puis les vignettes.
        let title_size = (16.0 * xf.zoom).clamp(7.0, 40.0);
        for row in &layout.scenes {
            let mut title = format!("Scène {}", self.doc.scene_label(row.scene));
            if self.doc.sequence_run(row.start).0 == row.start {
                title = format!("Séquence {}  ·  {title}", self.doc.sequence_label(self.doc.sequence_of(row.start)));
            }
            painter.text(xf.to_screen(row.title), Align2::LEFT_TOP, title, FontId::proportional(title_size), INK);
        }
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        for place in &layout.cards {
            self.board_card(ui, &painter, view, xf, place, pan_mode, mods, deadline);
        }

        // Notes par-dessus, puis les repères du geste.
        for s in &self.doc.board.strokes {
            paint_note(&painter, view, xf, &s.points, s.color);
        }
        self.paint_board_drag(&painter, view, xf, &layout);
        if let (Some(p), Tool::Eraser, false) = (over, self.tool, pan_mode) {
            if bg.hovered() || matches!(self.board_drag, Some(BoardDrag::Erase { .. })) {
                let r = self.note_eraser_radius(xf.zoom) * xf.zoom;
                painter.circle_stroke(p, r, Stroke::new(1.0_f32, INK));
            }
        }
        if bg.hovered() {
            let icon = if pan_mode {
                CursorIcon::Grab
            } else if matches!(self.tool, Tool::Eraser) || self.tool.is_shape() || matches!(self.tool, Tool::Brush | Tool::Pencil) {
                CursorIcon::Crosshair
            } else {
                CursorIcon::Default
            };
            ui.ctx().set_cursor_icon(icon);
        }
        bg.context_menu(|ui| self.board_menu(ui));
        if !any_down {
            self.finish_board_drag(&layout);
        }
    }

    fn board_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label("Taille");
            let mut size = self.layout.card_size;
            if ui
                .add(egui::Slider::new(&mut size, CARD_MIN..=CARD_MAX).show_value(false))
                .on_hover_text("Taille des vignettes")
                .changed()
            {
                self.layout.card_size = size;
            }
            if ui
                .button(format!("{:.0} %", self.board_zoom * 100.0))
                .on_hover_text("Zoom de la planche (Ctrl + molette). Clic : 100 %, en haut de la planche.")
                .clicked()
            {
                self.board_fit();
            }
            let n = self.board_selection().len();
            if n > 1 {
                ui.label(RichText::new(format!("{n} panneaux choisis")).strong());
            }
            ui.label(
                RichText::new(
                    "Pinceau : noter · Gomme : effacer les notes · Sélection : choisir · glisser un titre : déplacer · \
                     Caméra : recadrer · Espace ou Main : se déplacer",
                )
                .small()
                .weak(),
            );
        });
    }

    /// Garde un bout de la planche dans la vue.
    fn clamp_board_pan(&mut self, view: Rect, size: Vec2) {
        let s = size * self.board_zoom;
        let keep = 120.0;
        self.board_pan.x = self.board_pan.x.clamp((keep - s.x).min(0.0), (view.width() - keep).max(0.0));
        self.board_pan.y = self.board_pan.y.clamp((keep - s.y).min(0.0), (view.height() - keep).max(0.0));
    }

    /// Point d'une note au point `p` de la planche, avec l'épaisseur du pinceau de l'outil.
    fn note_point(&self, p: Pos2, pen: bool) -> VPoint {
        let b = self.current_brush().copied().unwrap_or(self.brush);
        let pressure = if pen { self.pen_pressure } else { 1.0 };
        let factor = if b.pressure_size { b.min_size + (1.0 - b.min_size) * pressure } else { 1.0 };
        VPoint { x: p.x, y: p.y, r: (b.size * 0.5 * NOTE_SCALE * factor).clamp(0.3, 60.0) }
    }

    fn note_eraser_radius(&self, zoom: f32) -> f32 {
        (self.eraser.size * 0.5 * NOTE_SCALE).max(4.0 / zoom)
    }

    fn update_board_drag(&mut self, pointer: Option<Pos2>, touch: bool, zoom: f32) {
        let Some(p) = pointer else { return };
        let pen = matches!(self.board_drag, Some(BoardDrag::Note { pen: true, .. })) || touch;
        let point = self.note_point(p, pen);
        let radius = self.note_eraser_radius(zoom);
        let erase_first = matches!(self.board_drag, Some(BoardDrag::Erase { recorded: false }));
        match &mut self.board_drag {
            Some(BoardDrag::Note { points, pen: was }) => {
                *was |= touch;
                board::push_note_point(points, point, 1.0 / zoom);
            }
            Some(BoardDrag::Move { at, .. }) => *at = p,
            Some(BoardDrag::Rubber { now, .. }) => *now = p,
            Some(BoardDrag::Erase { .. }) => {
                if self.doc.board.strokes.iter().any(|s| s.touches(p, radius)) {
                    if erase_first {
                        self.record_step();
                        self.board_drag = Some(BoardDrag::Erase { recorded: true });
                    }
                    self.doc.board.erase_at(p, radius);
                }
            }
            None => {}
        }
    }

    fn finish_board_drag(&mut self, layout: &BoardLayout) {
        match self.board_drag.take() {
            Some(BoardDrag::Note { points, .. }) if !points.is_empty() => {
                self.record_step();
                self.doc.board.strokes.push(NoteStroke { points, color: self.color });
            }
            Some(BoardDrag::Move { ids, at }) => {
                let Some(t) = board::drop_target(layout, at) else { return };
                let before = t.before.map(|i| self.doc.panels[i].id);
                let cur = self.doc.panels[self.cur_panel].id;
                let snap = self.snapshot();
                if self.doc.move_panels(&ids, t.scene, before).is_some() {
                    self.commit_structure(snap);
                    self.cur_panel = self.doc.panels.iter().position(|p| p.id == cur).unwrap_or(0);
                    self.clamp_nav();
                    self.status = match ids.len() {
                        1 => "Panneau déplacé.".into(),
                        n => format!("{n} panneaux déplacés."),
                    };
                }
            }
            Some(BoardDrag::Rubber { start, now, add }) => {
                let found: Vec<u64> =
                    board::cards_in(layout, Rect::from_two_pos(start, now)).into_iter().map(|i| self.doc.panels[i].id).collect();
                if add {
                    for id in found {
                        if !self.board_sel.contains(&id) {
                            self.board_sel.push(id);
                        }
                    }
                } else if let Some(&first) = found.first() {
                    if let Some(i) = self.doc.panels.iter().position(|p| p.id == first) {
                        self.select_panel(i);
                    }
                    self.board_sel = found;
                }
            }
            _ => {}
        }
    }

    /// Une vignette : titre, image (ou le dessin lui-même pour le panneau courant), cadre, gestes.
    #[allow(clippy::too_many_arguments)]
    fn board_card(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        view: Rect,
        xf: Xf,
        place: &CardPlace,
        pan_mode: bool,
        mods: Modifiers,
        deadline: Instant,
    ) {
        let card = xf.rect(place.card);
        if !card.intersects(view) {
            return;
        }
        let thumb = xf.rect(place.thumb);
        let header = Rect::from_min_max(card.min, pos2(card.right(), thumb.top()));
        let i = place.panel;
        let id = self.doc.panels[i].id;
        let current = i == self.cur_panel;
        let chosen = current || self.board_sel.contains(&id);
        let accent = ui.visuals().selection.bg_fill;

        painter.rect_filled(card.translate(vec2(2.0, 3.0)), 3.0, Color32::from_black_alpha(40));
        let (head_bg, head_ink) = if current {
            (accent, Color32::WHITE)
        } else if chosen {
            (accent.gamma_multiply(0.6), Color32::WHITE)
        } else {
            (HEADER_BG, INK)
        };
        painter.rect_filled(header, 0.0, head_bg);
        let font = (11.0 * xf.zoom).clamp(6.0, 26.0);
        if header.height() >= 8.0 {
            let clip = painter.with_clip_rect(header.intersect(view));
            clip.text(
                pos2(header.left() + 5.0 * xf.zoom.min(1.5), header.center().y),
                Align2::LEFT_CENTER,
                panel_title(&self.doc, i),
                FontId::proportional(font),
                head_ink,
            );
            clip.text(
                pos2(header.right() - 5.0 * xf.zoom.min(1.5), header.center().y),
                Align2::RIGHT_CENTER,
                duration_label(self.doc.panels[i].frame_count(), self.doc.fps),
                FontId::monospace(font * 0.9),
                head_ink,
            );
        }

        // Le panneau courant : le dessin lui-même ; les autres : leur vignette.
        let live = current && !pan_mode;
        if live {
            // La vue caméra laisse une marge autour du cadre : on l'agrandit d'autant (et on coupe
            // à la vignette) pour que l'image remplisse la vignette. Pour recadrer, toute la feuille.
            let area = if self.tool == Tool::Camera { thumb } else { Rect::from_center_size(thumb.center(), thumb.size() / 0.96) };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).id_salt("planche_vive"));
            child.set_clip_rect(thumb.intersect(view));
            self.card_canvas(&mut child);
        } else {
            painter.rect_filled(thumb, 0.0, Color32::WHITE);
            let px = thumb.width() * ui.ctx().pixels_per_point();
            if let Some(tex) = self.thumbs.texture(ui.ctx(), &self.doc, i, crate::thumbs::width_for(px), deadline) {
                painter.image(tex, thumb, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        let frame = if current {
            Stroke::new(3.0_f32, accent)
        } else if chosen {
            Stroke::new(2.0_f32, accent)
        } else {
            Stroke::new(1.0_f32, Color32::from_gray(150))
        };
        painter.rect_stroke(card, 2.0, frame, StrokeKind::Outside);

        // Gestes : le titre seulement sur le panneau courant (son image sert à dessiner).
        let hit = if live { header } else { card }.intersect(view);
        let resp = ui.interact(hit, ui.id().with(("planche_carte", id)), Sense::click_and_drag());
        if pan_mode || resp.dragged_by(PointerButton::Middle) {
            if resp.dragged() {
                self.board_pan += resp.drag_delta();
            }
        } else if resp.drag_started_by(PointerButton::Primary) {
            if !chosen {
                self.board_sel = vec![id];
                self.select_panel(i);
            }
            let ids = self.board_selection();
            let at = xf.to_board(resp.interact_pointer_pos().unwrap_or(card.center()));
            self.board_drag = Some(BoardDrag::Move { ids, at });
        }
        if resp.hovered() && !pan_mode {
            ui.ctx().set_cursor_icon(if live { CursorIcon::Grab } else { CursorIcon::PointingHand });
        }
        if resp.clicked() {
            if mods.ctrl {
                if let Some(k) = self.board_sel.iter().position(|s| *s == id) {
                    self.board_sel.remove(k);
                } else if !current {
                    self.board_sel.push(id);
                }
            } else if mods.shift {
                let (a, b) = (self.cur_panel.min(i), self.cur_panel.max(i));
                self.board_sel = self.doc.panels[a..=b].iter().map(|p| p.id).collect();
            } else {
                self.board_sel = vec![id];
                self.select_panel(i);
            }
        }
        if resp.double_clicked() {
            self.enter_animation(i);
        }
        if resp.secondary_clicked() && !chosen {
            self.board_sel = vec![id];
            self.select_panel(i);
        }
        resp.context_menu(|ui| self.card_menu(ui, i));
    }

    /// Le dessin dans la vignette du panneau courant : vue à travers la caméra, cadrée sur la
    /// vignette ; avec l'outil Caméra, toute la feuille et le cadre, pour recadrer. La vue du
    /// dessin en grand est remise telle quelle ensuite.
    fn card_canvas(&mut self, ui: &mut egui::Ui) {
        let (view, camera_view, canvas_rect) = (self.view, self.camera_view, self.canvas_rect);
        let reframe = self.tool == Tool::Camera;
        self.camera_view = !reframe;
        if reframe {
            self.view.fit(ui.max_rect(), self.canvas_size());
        }
        self.in_card = true;
        self.canvas_panel(ui);
        self.in_card = false;
        (self.view, self.camera_view, self.canvas_rect) = (view, camera_view, canvas_rect);
    }

    /// Menu d'une vignette (clic droit) : sur ce panneau, ou sur tous les panneaux choisis.
    fn card_menu(&mut self, ui: &mut egui::Ui, i: usize) {
        let ids = self.board_selection();
        let item = |app: &mut App, ui: &mut egui::Ui, text: &str, f: &dyn Fn(&mut App)| {
            if ui.button(text).clicked() {
                ui.close_menu();
                app.select_panel(i);
                f(app);
            }
        };
        item(self, ui, "Animer ce panneau", &|a| a.enter_animation(a.cur_panel));
        item(self, ui, "Nouveau panneau après", &|a| a.new_panel());
        item(self, ui, "Dupliquer le panneau", &|a| a.duplicate_panel());
        let label = if ids.len() > 1 { format!("Supprimer les {} panneaux choisis", ids.len()) } else { "Supprimer le panneau".into() };
        if ui.button(label).clicked() {
            ui.close_menu();
            self.delete_panels(&ids);
        }
        ui.separator();
        item(self, ui, "Nouvelle scène à partir de ce panneau", &|a| a.run_action("scene.couper"));
        item(self, ui, "Recadrer (outil Caméra)", &|a| a.set_tool(Tool::Camera));
        item(self, ui, "Réinitialiser le cadrage", &|a| a.run_action("camera.reinitialiser"));
    }

    /// Menu du fond de la planche (clic droit).
    fn board_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Nouveau panneau").clicked() {
            ui.close_menu();
            self.new_panel();
        }
        if ui.button("Nouvelle scène").clicked() {
            ui.close_menu();
            self.new_scene();
        }
        ui.separator();
        if ui.button("Tout choisir").clicked() {
            ui.close_menu();
            self.board_sel = self.doc.panels.iter().map(|p| p.id).collect();
        }
        if ui.button("Zoom 100 %, en haut").clicked() {
            ui.close_menu();
            self.board_fit();
        }
        ui.separator();
        if ui.add_enabled(!self.doc.board.strokes.is_empty(), egui::Button::new("Effacer toutes les notes")).clicked() {
            ui.close_menu();
            self.record_step();
            self.doc.board.strokes.clear();
        }
    }

    /// Repères du geste en cours : note, lasso, place où tomberont les panneaux.
    fn paint_board_drag(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout) {
        let accent = Color32::from_rgb(40, 120, 230);
        match &self.board_drag {
            Some(BoardDrag::Note { points, .. }) => paint_note(painter, view, xf, points, self.color),
            Some(BoardDrag::Rubber { start, now, .. }) => {
                let r = Rect::from_two_pos(xf.to_screen(*start), xf.to_screen(*now));
                painter.rect_filled(r, 0.0, accent.gamma_multiply(0.15));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, accent), StrokeKind::Inside);
            }
            Some(BoardDrag::Move { ids, at }) => {
                if let Some(t) = board::drop_target(layout, *at) {
                    painter.line_segment([xf.to_screen(t.line.0), xf.to_screen(t.line.1)], Stroke::new(4.0_f32, accent));
                }
                let label = match ids.len() {
                    1 => "1 panneau".to_string(),
                    n => format!("{n} panneaux"),
                };
                let p = xf.to_screen(*at) + vec2(14.0, 10.0);
                let galley = painter.layout_no_wrap(label, FontId::proportional(13.0), Color32::WHITE);
                painter.rect_filled(Rect::from_min_size(p, galley.size()).expand(4.0), 4.0, accent);
                painter.galley(p, galley, Color32::WHITE);
            }
            _ => {}
        }
    }
}

/// Un trait de note à l'écran : des lignes continues par tronçons d'épaisseur presque égale
/// (la pression fait varier l'épaisseur), bouts arrondis.
fn paint_note(painter: &egui::Painter, view: Rect, xf: Xf, points: &[VPoint], color: [u8; 3]) {
    if points.is_empty() || !xf.rect(board::points_bounds(points)).intersects(view) {
        return;
    }
    let color = Color32::from_rgb(color[0], color[1], color[2]);
    let mut run: Vec<Pos2> = Vec::new();
    let mut run_w = 0.0;
    let flush = |run: &[Pos2], w: f32| {
        if run.len() >= 2 {
            painter.add(egui::Shape::line(run.to_vec(), Stroke::new(w * 2.0, color)));
        }
    };
    for p in points {
        let s = xf.to_screen(p.pos());
        let w = (p.r * xf.zoom).max(0.4);
        if run.last().is_some_and(|l| l.distance(s) < 1.0) {
            continue;
        }
        if !run.is_empty() && (w - run_w).abs() > 0.3 {
            flush(&run, run_w);
            let last = *run.last().expect("tronçon non vide");
            run.clear();
            run.push(last);
        }
        if run.len() <= 1 {
            run_w = w;
        }
        run.push(s);
    }
    flush(&run, run_w);
    let (a, b) = (points[0], points[points.len() - 1]);
    painter.circle_filled(xf.to_screen(a.pos()), (a.r * xf.zoom).max(0.4), color);
    painter.circle_filled(xf.to_screen(b.pos()), (b.r * xf.zoom).max(0.4), color);
}
