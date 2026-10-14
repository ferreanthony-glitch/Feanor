//! Niveau storyboard : les plans montés bout à bout, leurs vignettes, leur durée,
//! leurs indications (action, dialogue, notes) et la lecture de tout le montage.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
};

use crate::app::{App, BoardView};
use crate::document::{Document, Panel};
use crate::history::Edit;
use crate::timeline::CUR_FRAME;

pub(crate) enum MontageDrag {
    /// Bord droit d'un plan tiré : on change sa durée. Les calques sont recalculés depuis
    /// l'original à chaque mouvement, rien n'est perdu tant qu'on n'a pas relâché.
    Trim {
        panel: usize,
        original: Panel,
        start_len: usize,
        origin_x: f32,
        snap: Edit,
    },
    /// Plan déplacé dans le montage ; `target` est la place où il sera inséré.
    Move { panel: usize, target: usize },
    Scrub,
}

const RULER_H: f32 = 18.0;
const SEQ_H: f32 = 15.0;
const SCENE_H: f32 = 18.0;
const BANDS_H: f32 = SEQ_H + SCENE_H + 2.0;

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

/// « Sc 3 · P 2 » : numéro de scène et de panneau, pour les vignettes.
fn panel_title(doc: &Document, index: usize) -> String {
    let (n, _) = doc.panel_number(index);
    format!("Sc {} · P {}", doc.scene_label(doc.panels[index].scene), n)
}

/// « Scène 3 — panneau 2/4 » : pour la fiche et la barre d'état.
pub fn panel_long_title(doc: &Document, index: usize) -> String {
    let (n, count) = doc.panel_number(index);
    format!("Scène {} — panneau {n}/{count}", doc.scene_label(doc.panels[index].scene))
}

/// Couleur d'une barre de scène ou de séquence (alternée pour qu'on voie les limites).
fn band_color(number: usize, base: u8) -> Color32 {
    if number % 2 == 0 {
        Color32::from_gray(base)
    } else {
        Color32::from_gray(base + 18)
    }
}

impl App {
    // ------------------------------------------------------------------ opérations sur les panneaux

    /// Applique une opération de montage du modèle (avec annulation) et rend courant le panneau
    /// qu'elle désigne. `None` = opération refusée.
    fn structure_op(&mut self, op: impl FnOnce(&mut Document, usize) -> Option<usize>) -> bool {
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let before = self.panel().id;
        let Some(index) = op(&mut self.doc, self.cur_panel) else {
            return false;
        };
        if self.doc.panels[index].id != before {
            self.cur_frame = 0;
        }
        self.cur_panel = index;
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
        self.vsel = None;
        self.follow_frame = true;
        self.commit_structure(snap);
        true
    }

    /// Nouveau panneau juste après le panneau courant, dans la même scène.
    pub(crate) fn new_panel(&mut self) {
        self.structure_op(|d, i| Some(d.add_panel_after(i)));
    }

    /// Nouvelle scène (avec un panneau) après la scène courante, dans la même séquence.
    pub(crate) fn new_scene(&mut self) {
        if self.structure_op(|d, i| Some(d.add_scene_after(i))) {
            self.status = format!("Nouvelle scène {}.", self.doc.scene_label(self.panel().scene));
        }
    }

    /// Nouvelle séquence (une scène, un panneau) après la séquence courante.
    pub(crate) fn new_sequence(&mut self) {
        if self.structure_op(|d, i| Some(d.add_sequence_after(i))) {
            let seq = self.doc.sequence_of(self.cur_panel);
            self.status = format!("Nouvelle séquence {}.", self.doc.sequence_label(seq));
        }
    }

    pub(crate) fn duplicate_panel(&mut self) {
        self.structure_op(|d, i| Some(d.duplicate_panel_after(i)));
    }

    /// Copie de toute la scène courante, placée juste après elle.
    pub(crate) fn duplicate_scene(&mut self) {
        self.structure_op(|d, i| Some(d.duplicate_scene_after(i)));
    }

    pub(crate) fn delete_panel(&mut self) {
        if !self.structure_op(|d, i| d.remove_panel(i)) {
            self.status = "Il faut garder au moins un panneau.".into();
        }
    }

    /// Supprime la scène courante et tous ses panneaux.
    pub(crate) fn delete_scene(&mut self) {
        if !self.structure_op(|d, i| d.remove_scene(i)) {
            self.status = "Impossible de supprimer la seule scène du projet.".into();
        }
    }

    /// Déplace le panneau `from` à la place `slot` (voir `Document::move_panel`).
    pub(crate) fn move_panel(&mut self, from: usize, slot: usize) {
        let current = self.panel().id;
        self.structure_op(|d, _| {
            d.move_panel(from, slot)?;
            // Le panneau courant reste courant, même si ce n'est pas lui qu'on a déplacé.
            d.panels.iter().position(|p| p.id == current)
        });
    }

    pub(crate) fn shift_panel(&mut self, later: bool) {
        let i = self.cur_panel;
        if later && i + 1 < self.doc.panels.len() {
            self.move_panel(i, i + 2);
        } else if !later && i > 0 {
            self.move_panel(i, i - 1);
        }
    }

    /// Premier panneau de la scène précédente (ou suivante).
    pub(crate) fn step_scene(&mut self, forward: bool) {
        let (start, end) = self.doc.scene_run(self.cur_panel);
        let target = if forward {
            (end < self.doc.panels.len()).then_some(end)
        } else {
            start.checked_sub(1).map(|i| self.doc.scene_run(i).0)
        };
        if let Some(i) = target {
            self.select_panel(i);
        }
    }

    /// Durée du plan courant. Chaque valeur repart du plan tel qu'il était au début du geste
    /// (raccourcir puis rallonger ne perd rien), avec une seule étape d'annulation par geste.
    pub(crate) fn edit_panel_length(&mut self, n: usize) {
        self.length_field_active = true;
        let id = self.panel().id;
        if self.length_origin.as_ref().is_none_or(|s| s.id != id) {
            self.end_stroke();
            let snap = self.snapshot();
            self.history.push(snap);
            self.length_origin = Some(self.panel().clone());
        }
        let Some(origin) = &self.length_origin else { return };
        let mut panel = origin.clone();
        panel.set_length(n);
        *self.panel_mut() = panel;
        self.vsel = None;
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Champ de saisie de la durée du plan courant (fiche du plan, timeline).
    pub(crate) fn panel_length_field(&mut self, ui: &mut egui::Ui, prefix: &str) -> egui::Response {
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
            self.edit_panel_length(len);
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
        if let Some(MontageDrag::Trim { panel, original, .. }) = self.montage_drag.take() {
            if self.doc.panels.get(panel).is_some_and(|s| s.id == original.id) {
                self.doc.panels[panel] = original;
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
            let hint = |app: &App, text: &str, action: &str| {
                let key = app.keymap.label(action);
                if key.is_empty() {
                    text.to_string()
                } else {
                    format!("{text} ({key})")
                }
            };
            if ui
                .button("+ Panneau")
                .on_hover_text(hint(self, "Nouveau panneau après le panneau courant", "panneau.nouveau"))
                .clicked()
            {
                self.new_panel();
            }
            if ui
                .button("+ Scène")
                .on_hover_text(hint(self, "Nouvelle scène après la scène courante", "scene.nouvelle"))
                .clicked()
            {
                self.new_scene();
            }
            if ui
                .button("+ Séquence")
                .on_hover_text(hint(self, "Nouvelle séquence après la séquence courante", "sequence.nouvelle"))
                .clicked()
            {
                self.new_sequence();
            }
            ui.menu_button("Panneau…", |ui| {
                if ui.button("Dupliquer le panneau").clicked() {
                    self.duplicate_panel();
                    ui.close_menu();
                }
                if ui.button("Supprimer le panneau").clicked() {
                    self.delete_panel();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Dupliquer la scène").clicked() {
                    self.duplicate_scene();
                    ui.close_menu();
                }
                if ui.button("Supprimer la scène").clicked() {
                    self.delete_scene();
                    ui.close_menu();
                }
            });
            if ui.button("< Avancer").on_hover_text("Place le panneau plus tôt dans le montage").clicked() {
                self.shift_panel(false);
            }
            if ui.button("Reculer >").on_hover_text("Place le panneau plus tard dans le montage").clicked() {
                self.shift_panel(true);
            }
            ui.separator();
            if ui
                .button(RichText::new("Animer ce panneau").strong())
                .on_hover_text(hint(
                    self,
                    "Entrer dans le panneau pour l'animer, ou double-clic sur le panneau",
                    "mode.basculer",
                ))
                .clicked()
            {
                self.enter_animation(self.cur_panel);
            }
            ui.separator();
            ui.selectable_value(&mut self.board_view, BoardView::Scene, "Scène")
                .on_hover_text(hint(self, "Dessiner le panneau", "vue.planches"));
            ui.selectable_value(&mut self.board_view, BoardView::Panels, "Planches")
                .on_hover_text(hint(self, "Vue d'ensemble des panneaux", "vue.planches"));
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
        let block_h = (ui.available_height() - RULER_H - BANDS_H - 6.0).clamp(56.0, 170.0);
        let width = (total as f32 * ppf + 260.0).max(ui.available_width());
        let (resp, painter) =
            ui.allocate_painter(vec2(width, RULER_H + BANDS_H + block_h + 4.0), Sense::click_and_drag());
        let r = resp.rect;
        let left = r.left() + 6.0;
        let x_of = |g: usize| left + g as f32 * ppf;
        let bands_top = r.top() + RULER_H;
        let top = bands_top + BANDS_H;
        let visuals = ui.visuals().clone();

        // Règle : une graduation par seconde, un repère lisible tous les ~60 pixels.
        let label_every = ((60.0 / (fps as f32 * ppf)).ceil() as usize).max(1);
        for s in 0..=(total / fps + 2) {
            let x = x_of(s * fps);
            let major = s % label_every == 0;
            painter.line_segment(
                [pos2(x, r.top() + if major { 3.0 } else { 10.0 }), pos2(x, bands_top)],
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
        let mut blocks: Vec<Rect> = Vec::with_capacity(self.doc.panels.len());
        let mut start = 0;
        for i in 0..self.doc.panels.len() {
            let len = self.doc.panels[i].frame_count();
            let rect = Rect::from_min_max(
                pos2(x_of(start) + 1.0, top + 2.0),
                pos2(x_of(start + len) - 1.0, top + block_h),
            );
            blocks.push(rect);
            start += len;
            let selected = i == self.cur_panel;
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
            let title = clip.layout_no_wrap(panel_title(&self.doc, i), FontId::proportional(12.0), visuals.text_color());
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

        // Barres des séquences et des scènes au-dessus des panneaux.
        let mut runs: Vec<(Rect, usize)> = Vec::new();
        for (band, y0, h) in [(0, bands_top, SEQ_H), (1, bands_top + SEQ_H, SCENE_H)] {
            let mut i = 0;
            while i < self.doc.panels.len() {
                let (start, end) = if band == 0 {
                    self.doc.sequence_run(i)
                } else {
                    self.doc.scene_run(i)
                };
                let rect = Rect::from_min_max(
                    pos2(blocks[start].left(), y0 + 1.0),
                    pos2(blocks[end - 1].right(), y0 + h - 1.0),
                );
                let (label, number) = if band == 0 {
                    let seq = self.doc.sequence_of(start);
                    (format!("Séquence {}", self.doc.sequence_label(seq)), self.doc.sequence_number(seq))
                } else {
                    let scene = self.doc.panels[start].scene;
                    (format!("Scène {}", self.doc.scene_label(scene)), self.doc.scene_number(scene))
                };
                let current = (start..end).contains(&self.cur_panel);
                let fill = if current {
                    visuals.selection.bg_fill.gamma_multiply(if band == 0 { 0.45 } else { 0.7 })
                } else {
                    band_color(number, if band == 0 { 48 } else { 62 })
                };
                painter.rect_filled(rect, 3.0, fill);
                painter.with_clip_rect(rect.shrink(1.0)).text(
                    pos2(rect.left() + 5.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(if band == 0 { 10.0 } else { 11.0 }),
                    visuals.text_color(),
                );
                runs.push((rect, start));
                i = end;
            }
        }

        let panel_at = |x: f32| blocks.iter().position(|b| x >= b.left() - 1.0 && x <= b.right() + 1.0);
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
                self.set_panel(i);
                MontageDrag::Trim {
                    panel: i,
                    original: self.doc.panels[i].clone(),
                    start_len: self.doc.panels[i].frame_count(),
                    origin_x: origin.x,
                    snap: self.snapshot(),
                }
            } else if let Some(i) = panel_at(origin.x) {
                self.set_panel(i);
                MontageDrag::Move { panel: i, target: i }
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
                    panel,
                    ..
                }) => {
                    let changed = self
                        .doc
                        .panels
                        .get(panel)
                        .is_some_and(|s| s.id == original.id && s.frame_count() != original.frame_count());
                    if changed {
                        self.history.push(snap);
                        self.modified = true;
                    }
                }
                Some(MontageDrag::Move { panel, target }) => self.move_panel(panel, target),
                _ => {}
            }
        }

        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                // Clic sur une barre de scène ou de séquence : son premier panneau.
                match runs.iter().find(|(rect, _)| rect.contains(p)) {
                    Some(&(_, start)) => self.select_panel(start),
                    None => self.goto_global(frame_at(p.x)),
                }
            }
        }
        if resp.double_clicked() {
            if let Some(i) = resp.interact_pointer_pos().filter(|p| p.y >= top).and_then(|p| panel_at(p.x)) {
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
            panel,
            original,
            start_len,
            origin_x,
            ..
        }) = &self.montage_drag
        else {
            return;
        };
        let n = ((*start_len as f32) + ((x - *origin_x) / ppf).round()).max(1.0) as usize;
        let i = *panel;
        if self.doc.panels.get(i).is_none_or(|s| s.id != original.id) {
            self.montage_drag = None;
            return;
        }
        let mut s = original.clone();
        s.set_length(n);
        self.doc.panels[i] = s;
        self.renderer.invalidate_all();
        self.status = format!("{} : {}", panel_long_title(&self.doc, i), duration_label(n, fps));
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
                ui.add_space(6.0);
                // Une rangée de cartes par scène, sous son titre (et celui de la séquence).
                let mut run_start = 0;
                while run_start < self.doc.panels.len() {
                    let (start, end) = self.doc.scene_run(run_start);
                    let scene = self.doc.panels[start].scene;
                    let mut header = format!("Scène {}", self.doc.scene_label(scene));
                    if self.doc.sequence_run(start).0 == start {
                        let seq = self.doc.sequence_of(start);
                        header = format!("Séquence {}  ·  {header}", self.doc.sequence_label(seq));
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(header).heading());
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                    ui.add_space(6.0);
                    for i in start..end {
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
                        let selected = i == self.cur_panel;
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
                        let panel = &self.doc.panels[i];
                        let mut y = tr.bottom() + 6.0;
                        painter.text(
                            pos2(rect.left() + 8.0, y),
                            Align2::LEFT_TOP,
                            panel_title(&self.doc, i),
                            FontId::proportional(14.0),
                            visuals.strong_text_color(),
                        );
                        painter.text(
                            pos2(rect.right() - 8.0, y + 2.0),
                            Align2::RIGHT_TOP,
                            duration_label(panel.frame_count(), fps),
                            FontId::monospace(11.0),
                            visuals.weak_text_color(),
                        );
                        y += 22.0;
                        for (label, text) in [("Action", &panel.action), ("Dialogue", &panel.dialogue)] {
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
                    ui.add_space(16.0);
                    run_start = end;
                }
            });
        if let Some(i) = clicked {
            self.select_panel(i);
        }
        if let Some(i) = double {
            self.enter_animation(i);
        }
    }

    // ------------------------------------------------------------------ fiche du plan

    pub(crate) fn panel_inspector(&mut self, ui: &mut egui::Ui) {
        let i = self.cur_panel;
        let fps = self.doc.fps;
        let (n, count) = self.doc.panel_number(i);
        let scene = self.doc.panels[i].scene;
        let seq = self.doc.sequence_of(i);
        let scene_number = self.doc.scene_number(scene).to_string();
        let seq_number = self.doc.sequence_number(seq).to_string();
        ui.add_space(4.0);
        ui.heading(panel_long_title(&self.doc, i));
        let mut changed = false;
        egui::Grid::new("fiche_panneau").num_columns(2).show(ui, |ui| {
            ui.label("Séquence");
            if let Some(s) = self.doc.sequence_mut(seq) {
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut s.name)
                            .hint_text(seq_number)
                            .desired_width(150.0),
                    )
                    .on_hover_text("Vide = numéro automatique")
                    .changed();
            }
            ui.end_row();
            ui.label("Scène");
            if let Some(s) = self.doc.scene_mut(scene) {
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut s.name)
                            .hint_text(scene_number)
                            .desired_width(150.0),
                    )
                    .on_hover_text("Numéro ou nom de la scène (vide = numéro automatique)")
                    .changed();
            }
            ui.end_row();
            ui.label("Panneau");
            ui.label(format!("{n} sur {count}"));
            ui.end_row();
            ui.label("Durée");
            ui.horizontal(|ui| {
                self.panel_length_field(ui, "");
                ui.label(RichText::new(duration_label(self.frame_count(), fps)).weak());
            });
            ui.end_row();
        });
        let panel = &mut self.doc.panels[i];
        for (label, text, rows) in [
            ("Action", &mut panel.action, 3),
            ("Dialogue", &mut panel.dialogue, 2),
            ("Notes", &mut panel.notes, 2),
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
