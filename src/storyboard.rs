//! Niveau storyboard : les plans montés bout à bout, leurs vignettes, leur durée,
//! leurs indications (action, dialogue, notes) et la lecture de tout le montage.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
};

use crate::app::{App, BoardView};
use crate::document::Shot;
use crate::history::Edit;
use crate::timeline::CUR_FRAME;

pub(crate) enum MontageDrag {
    /// Bord droit d'un plan tiré : on change sa durée. Les calques sont recalculés depuis
    /// l'original à chaque mouvement, rien n'est perdu tant qu'on n'a pas relâché.
    Trim {
        shot: usize,
        original: Shot,
        start_len: usize,
        origin_x: f32,
        snap: Edit,
    },
    /// Plan déplacé dans le montage ; `target` est la place où il sera inséré.
    Move { shot: usize, target: usize },
    Scrub,
}

const RULER_H: f32 = 18.0;

pub fn timecode(frames: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    let s = frames / fps;
    format!("{}:{:02}:{:02}", s / 60, s % 60, frames % fps)
}

pub fn duration_label(frames: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    if frames >= fps {
        format!("{}s {:02}", frames / fps, frames % fps)
    } else {
        format!("{frames} i")
    }
}

fn shot_title(shot: &Shot, index: usize) -> String {
    if shot.name.is_empty() {
        format!("Plan {}", index + 1)
    } else {
        format!("Plan {} · {}", index + 1, shot.name)
    }
}

impl App {
    // ------------------------------------------------------------------ opérations sur les plans

    pub(crate) fn new_shot(&mut self) {
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let shot = self.doc.make_shot();
        let at = (self.cur_shot + 1).min(self.doc.shots.len());
        self.doc.shots.insert(at, shot);
        self.cur_shot = at;
        self.cur_layer = 0;
        self.cur_frame = 0;
        self.vsel = None;
        self.follow_frame = true;
        self.commit_structure(snap);
    }

    pub(crate) fn duplicate_shot(&mut self) {
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let copy = self.doc.duplicate_shot(self.cur_shot);
        self.doc.shots.insert(self.cur_shot + 1, copy);
        self.cur_shot += 1;
        self.cur_frame = 0;
        self.vsel = None;
        self.follow_frame = true;
        self.commit_structure(snap);
    }

    pub(crate) fn delete_shot(&mut self) {
        self.end_stroke();
        self.playing = false;
        if self.doc.shots.len() <= 1 {
            self.status = "Il faut garder au moins un plan.".into();
            return;
        }
        let snap = self.snapshot();
        self.doc.shots.remove(self.cur_shot);
        self.cur_shot = self.cur_shot.min(self.doc.shots.len() - 1);
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
        self.cur_frame = 0;
        self.vsel = None;
        self.commit_structure(snap);
    }

    /// Déplace le plan `from` à la place `slot` (0 = avant le premier, `len` = après le dernier).
    pub(crate) fn move_shot(&mut self, from: usize, slot: usize) {
        let n = self.doc.shots.len();
        if from >= n || slot > n || slot == from || slot == from + 1 {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let current = self.shot().id;
        let shot = self.doc.shots.remove(from);
        self.doc.shots.insert(if slot > from { slot - 1 } else { slot }, shot);
        self.cur_shot = self.doc.shots.iter().position(|s| s.id == current).unwrap_or(0);
        self.follow_frame = true;
        self.commit_structure(snap);
    }

    pub(crate) fn shift_shot(&mut self, later: bool) {
        let i = self.cur_shot;
        if later && i + 1 < self.doc.shots.len() {
            self.move_shot(i, i + 2);
        } else if !later && i > 0 {
            self.move_shot(i, i - 1);
        }
    }

    /// Durée du plan courant. Chaque valeur repart du plan tel qu'il était au début du geste
    /// (raccourcir puis rallonger ne perd rien), avec une seule étape d'annulation par geste.
    pub(crate) fn edit_shot_length(&mut self, n: usize) {
        self.length_field_active = true;
        let id = self.shot().id;
        if self.length_origin.as_ref().is_none_or(|s| s.id != id) {
            self.end_stroke();
            let snap = self.snapshot();
            self.history.push(snap);
            self.length_origin = Some(self.shot().clone());
        }
        let Some(origin) = &self.length_origin else { return };
        let mut shot = origin.clone();
        shot.set_length(n);
        *self.shot_mut() = shot;
        self.vsel = None;
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Champ de saisie de la durée du plan courant (fiche du plan, timeline).
    pub(crate) fn shot_length_field(&mut self, ui: &mut egui::Ui, prefix: &str) -> egui::Response {
        let mut len = self.frame_count();
        let resp = ui.add(
            egui::DragValue::new(&mut len)
                .range(1..=100_000)
                .prefix(prefix)
                .suffix(" i")
                .update_while_editing(false),
        );
        if resp.dragged() || resp.has_focus() {
            self.length_field_active = true;
        }
        if resp.changed() {
            self.edit_shot_length(len);
        }
        resp
    }

    /// Fin du geste sur la durée quand plus aucun champ de durée n'est utilisé.
    pub(crate) fn end_length_gesture_if_idle(&mut self) {
        if !std::mem::take(&mut self.length_field_active) {
            self.length_origin = None;
        }
    }

    /// Abandonne un glissé en cours dans la bande de montage (la durée revient à l'original).
    pub(crate) fn cancel_montage_drag(&mut self) {
        if let Some(MontageDrag::Trim { shot, original, .. }) = self.montage_drag.take() {
            if self.doc.shots.get(shot).is_some_and(|s| s.id == original.id) {
                self.doc.shots[shot] = original;
                self.renderer.invalidate_all();
            }
        }
    }

    // ------------------------------------------------------------------ panneau du montage

    pub(crate) fn montage_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        let total = self.doc.total_frames();
        let fps = self.doc.fps;
        ui.horizontal_wrapped(|ui| {
            if ui.button("⏮").on_hover_text("Début du montage (Origine)").clicked() {
                self.goto_global(0);
            }
            let play = if self.playing { "⏸ Pause" } else { "▶ Lire le montage" };
            if ui.button(play).on_hover_text("Lecture / pause (Entrée)").clicked() {
                self.toggle_play();
            }
            ui.checkbox(&mut self.looping, "Boucle");
            ui.label(
                RichText::new(format!("{} / {}", timecode(self.global_frame(), fps), timecode(total, fps)))
                    .monospace(),
            );
            ui.separator();
            if ui
                .button("Nouveau plan")
                .on_hover_text("Ajoute un plan après le plan courant (Inser)")
                .clicked()
            {
                self.new_shot();
            }
            if ui.button("Dupliquer").on_hover_text("Copie le plan avec ses dessins").clicked() {
                self.duplicate_shot();
            }
            if ui.button("Supprimer").on_hover_text("Supprime le plan (Ctrl+Suppr)").clicked() {
                self.delete_shot();
            }
            if ui.button("< Avancer").on_hover_text("Place le plan plus tôt dans le montage").clicked() {
                self.shift_shot(false);
            }
            if ui.button("Reculer >").on_hover_text("Place le plan plus tard dans le montage").clicked() {
                self.shift_shot(true);
            }
            ui.separator();
            if ui
                .button(RichText::new("Animer ce plan").strong())
                .on_hover_text("Entrer dans le plan pour l'animer (Tab, ou double-clic sur le plan)")
                .clicked()
            {
                self.enter_animation(self.cur_shot);
            }
            ui.separator();
            ui.selectable_value(&mut self.board_view, BoardView::Scene, "Scène")
                .on_hover_text("Maj+Tab");
            ui.selectable_value(&mut self.board_view, BoardView::Panels, "Planches")
                .on_hover_text("Vue d'ensemble des plans (Maj+Tab)");
            ui.separator();
            ui.add(
                egui::Slider::new(&mut self.montage_zoom, 0.5..=16.0)
                    .logarithmic(true)
                    .show_value(false)
                    .text("zoom"),
            );
        });
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_salt("montage_h")
            .auto_shrink([false, false])
            .show(ui, |ui| self.montage_strip(ui));
    }

    fn montage_strip(&mut self, ui: &mut egui::Ui) {
        let ppf = self.montage_zoom;
        let fps = self.doc.fps.max(1) as usize;
        let total = self.doc.total_frames();
        let block_h = (ui.available_height() - RULER_H - 6.0).clamp(56.0, 170.0);
        let width = (total as f32 * ppf + 260.0).max(ui.available_width());
        let (resp, painter) = ui.allocate_painter(vec2(width, RULER_H + block_h + 4.0), Sense::click_and_drag());
        let r = resp.rect;
        let left = r.left() + 6.0;
        let x_of = |g: usize| left + g as f32 * ppf;
        let top = r.top() + RULER_H;
        let visuals = ui.visuals().clone();

        // Règle : une graduation par seconde, un repère lisible tous les ~60 pixels.
        let label_every = ((60.0 / (fps as f32 * ppf)).ceil() as usize).max(1);
        for s in 0..=(total / fps + 2) {
            let x = x_of(s * fps);
            let major = s % label_every == 0;
            painter.line_segment(
                [pos2(x, r.top() + if major { 3.0 } else { 10.0 }), pos2(x, top)],
                Stroke::new(1.0_f32, Color32::from_gray(90)),
            );
            if major {
                painter.text(
                    pos2(x + 3.0, r.top() + 2.0),
                    Align2::LEFT_TOP,
                    format!("{}:{:02}", s / 60, s % 60),
                    FontId::monospace(10.0),
                    visuals.weak_text_color(),
                );
            }
        }

        // Plans.
        let mut budget = 4;
        let thumb_h = (block_h - 24.0).max(10.0);
        let aspect = self.doc.width as f32 / self.doc.height as f32;
        let mut blocks: Vec<Rect> = Vec::with_capacity(self.doc.shots.len());
        let mut start = 0;
        for i in 0..self.doc.shots.len() {
            let len = self.doc.shots[i].frame_count();
            let rect = Rect::from_min_max(
                pos2(x_of(start) + 1.0, top + 2.0),
                pos2(x_of(start + len) - 1.0, top + block_h),
            );
            blocks.push(rect);
            start += len;
            let selected = i == self.cur_shot;
            let bg = if selected {
                visuals.selection.bg_fill.gamma_multiply(0.55)
            } else {
                Color32::from_gray(58)
            };
            painter.rect_filled(rect, 4.0, bg);
            let clip = painter.with_clip_rect(rect.shrink(2.0).intersect(r));
            if let Some(tex) = self.thumbs.texture(ui.ctx(), &self.doc, i, &mut budget) {
                let tr = Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(thumb_h * aspect, thumb_h));
                clip.image(
                    tex,
                    tr,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let shot = &self.doc.shots[i];
            let title = clip.layout_no_wrap(shot_title(shot, i), FontId::proportional(12.0), visuals.text_color());
            let duration = clip.layout_no_wrap(
                duration_label(len, self.doc.fps),
                FontId::monospace(10.0),
                visuals.weak_text_color(),
            );
            // La durée n'est affichée que si elle tient à côté du titre.
            if title.size().x + duration.size().x + 20.0 < rect.width() {
                let at = pos2(rect.right() - 7.0 - duration.size().x, rect.bottom() - 4.0 - duration.size().y);
                clip.galley(at, duration, visuals.weak_text_color());
            }
            let at = pos2(rect.left() + 5.0, rect.bottom() - 4.0 - title.size().y);
            clip.galley(at, title, visuals.text_color());
            if selected {
                painter.rect_stroke(
                    rect,
                    4.0,
                    Stroke::new(2.0_f32, visuals.selection.stroke.color),
                    StrokeKind::Inside,
                );
            }
            // Poignée de durée sur le bord droit.
            painter.rect_filled(
                Rect::from_min_max(pos2(rect.right() - 4.0, rect.top() + 10.0), pos2(rect.right() - 1.0, rect.bottom() - 10.0)),
                1.0,
                Color32::from_gray(130),
            );
        }

        let shot_at = |x: f32| blocks.iter().position(|b| x >= b.left() - 1.0 && x <= b.right() + 1.0);
        let edge_at = |p: Pos2| {
            if p.y < top {
                return None;
            }
            blocks.iter().position(|b| (p.x - b.right()).abs() <= 5.0)
        };
        let frame_at = |x: f32| (((x - left) / ppf).floor().max(0.0) as usize).min(total - 1);

        if let Some(p) = resp.hover_pos() {
            if edge_at(p).is_some() || matches!(self.montage_drag, Some(MontageDrag::Trim { .. })) {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
        }

        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(r.center());
            // Sans recentrer la bande : le bord tiré doit rester sous le pointeur.
            self.playing = false;
            let drag = if origin.y < top {
                MontageDrag::Scrub
            } else if let Some(i) = edge_at(origin) {
                self.set_shot(i);
                MontageDrag::Trim {
                    shot: i,
                    original: self.doc.shots[i].clone(),
                    start_len: self.doc.shots[i].frame_count(),
                    origin_x: origin.x,
                    snap: self.snapshot(),
                }
            } else if let Some(i) = shot_at(origin.x) {
                self.set_shot(i);
                MontageDrag::Move { shot: i, target: i }
            } else {
                MontageDrag::Scrub
            };
            self.montage_drag = Some(drag);
        }

        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                match &mut self.montage_drag {
                    Some(MontageDrag::Scrub) => {
                        let g = frame_at(p.x);
                        if g != self.global_frame() {
                            self.goto_global(g);
                        }
                    }
                    Some(MontageDrag::Move { target, .. }) => {
                        *target = blocks.iter().filter(|b| b.center().x < p.x).count();
                    }
                    Some(MontageDrag::Trim { .. }) => self.update_trim(p.x),
                    None => {}
                }
            }
        }

        if resp.drag_stopped() {
            match self.montage_drag.take() {
                Some(MontageDrag::Trim {
                    snap,
                    original,
                    shot,
                    ..
                }) => {
                    let changed = self
                        .doc
                        .shots
                        .get(shot)
                        .is_some_and(|s| s.id == original.id && s.frame_count() != original.frame_count());
                    if changed {
                        self.history.push(snap);
                        self.modified = true;
                    }
                }
                Some(MontageDrag::Move { shot, target }) => self.move_shot(shot, target),
                _ => {}
            }
        }

        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.goto_global(frame_at(p.x));
            }
        }
        if resp.double_clicked() {
            if let Some(i) = resp.interact_pointer_pos().filter(|p| p.y >= top).and_then(|p| shot_at(p.x)) {
                self.enter_animation(i);
            }
        }

        // Tête de lecture et repère de déplacement.
        let gx = x_of(self.global_frame());
        painter.line_segment([pos2(gx, r.top()), pos2(gx, r.bottom())], Stroke::new(2.0_f32, CUR_FRAME));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(gx - 5.0, r.top()), pos2(gx + 5.0, r.top()), pos2(gx, r.top() + 7.0)],
            CUR_FRAME,
            Stroke::NONE,
        ));
        if let Some(MontageDrag::Move { target, .. }) = &self.montage_drag {
            let x = match blocks.get(*target) {
                Some(b) => b.left() - 1.0,
                None => blocks.last().map_or(left, |b| b.right() + 1.0),
            };
            painter.line_segment(
                [pos2(x, top - 2.0), pos2(x, top + block_h + 2.0)],
                Stroke::new(3.0_f32, visuals.selection.stroke.color),
            );
        }

        if self.follow_frame {
            ui.scroll_to_rect(
                Rect::from_center_size(pos2(gx, top + block_h * 0.5), vec2(ppf * 24.0, 10.0)),
                None,
            );
            self.follow_frame = false;
        }
    }

    fn update_trim(&mut self, x: f32) {
        let ppf = self.montage_zoom;
        let fps = self.doc.fps;
        let Some(MontageDrag::Trim {
            shot,
            original,
            start_len,
            origin_x,
            ..
        }) = &self.montage_drag
        else {
            return;
        };
        let n = ((*start_len as f32) + ((x - *origin_x) / ppf).round()).max(1.0) as usize;
        let i = *shot;
        if self.doc.shots.get(i).is_none_or(|s| s.id != original.id) {
            self.montage_drag = None;
            return;
        }
        let mut s = original.clone();
        s.set_length(n);
        self.doc.shots[i] = s;
        self.renderer.invalidate_all();
        self.status = format!("Durée du plan {} : {}", i + 1, duration_label(n, fps));
    }

    // ------------------------------------------------------------------ vue « planches »

    pub(crate) fn panels_view(&mut self, ui: &mut egui::Ui) {
        let card_w = 250.0;
        let thumb_w = card_w - 12.0;
        let thumb_h = thumb_w * self.doc.height as f32 / self.doc.width as f32;
        let card_h = thumb_h + 120.0;
        let fps = self.doc.fps;
        let mut budget = 6;
        let mut clicked = None;
        let mut double = None;
        egui::ScrollArea::vertical()
            .id_salt("planches")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                    ui.add_space(6.0);
                    for i in 0..self.doc.shots.len() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(card_w, card_h), Sense::click());
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                        if resp.double_clicked() {
                            double = Some(i);
                        }
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let visuals = ui.visuals();
                        let painter = ui.painter_at(rect);
                        let selected = i == self.cur_shot;
                        painter.rect_filled(rect, 6.0, Color32::from_gray(if selected { 66 } else { 52 }));
                        if selected {
                            painter.rect_stroke(
                                rect,
                                6.0,
                                Stroke::new(2.0_f32, visuals.selection.stroke.color),
                                StrokeKind::Inside,
                            );
                        }
                        let tr = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(thumb_w, thumb_h));
                        painter.rect_filled(tr, 0.0, Color32::from_gray(30));
                        if let Some(tex) = self.thumbs.texture(ui.ctx(), &self.doc, i, &mut budget) {
                            painter.image(
                                tex,
                                tr,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        let shot = &self.doc.shots[i];
                        let mut y = tr.bottom() + 6.0;
                        painter.text(
                            pos2(rect.left() + 8.0, y),
                            Align2::LEFT_TOP,
                            shot_title(shot, i),
                            FontId::proportional(14.0),
                            visuals.strong_text_color(),
                        );
                        painter.text(
                            pos2(rect.right() - 8.0, y + 2.0),
                            Align2::RIGHT_TOP,
                            duration_label(shot.frame_count(), fps),
                            FontId::monospace(11.0),
                            visuals.weak_text_color(),
                        );
                        y += 22.0;
                        for (label, text) in [("Action", &shot.action), ("Dialogue", &shot.dialogue)] {
                            if text.trim().is_empty() {
                                continue;
                            }
                            let galley = painter.layout(
                                format!("{label} : {text}"),
                                FontId::proportional(12.0),
                                visuals.text_color(),
                                card_w - 16.0,
                            );
                            let height = galley.size().y;
                            painter.galley(pos2(rect.left() + 8.0, y), galley, visuals.text_color());
                            y += height + 4.0;
                        }
                    }
                });
            });
        if let Some(i) = clicked {
            self.select_shot(i);
        }
        if let Some(i) = double {
            self.enter_animation(i);
        }
    }

    // ------------------------------------------------------------------ fiche du plan

    pub(crate) fn shot_inspector(&mut self, ui: &mut egui::Ui) {
        let i = self.cur_shot;
        let fps = self.doc.fps;
        ui.add_space(4.0);
        ui.heading(format!("Plan {} / {}", i + 1, self.doc.shots.len()));
        let mut changed = false;
        egui::Grid::new("fiche_plan").num_columns(2).show(ui, |ui| {
            ui.label("Titre");
            changed |= ui
                .add(egui::TextEdit::singleline(&mut self.doc.shots[i].name).desired_width(150.0))
                .changed();
            ui.end_row();
            ui.label("Durée");
            ui.horizontal(|ui| {
                self.shot_length_field(ui, "");
                ui.label(RichText::new(duration_label(self.frame_count(), fps)).weak());
            });
            ui.end_row();
        });
        let shot = &mut self.doc.shots[i];
        for (label, text, rows) in [
            ("Action", &mut shot.action, 3),
            ("Dialogue", &mut shot.dialogue, 2),
            ("Notes", &mut shot.notes, 2),
        ] {
            ui.label(label);
            changed |= ui
                .add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(rows)
                        .desired_width(f32::INFINITY),
                )
                .changed();
        }
        if changed {
            self.modified = true;
        }
    }
}
