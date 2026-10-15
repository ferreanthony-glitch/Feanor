//! Planche de la vue Vignettes (phases 7 et 8) : interface. Géométrie et notes dans `board.rs`.
//!
//! Des pages blanches sur un fond gris, où les vignettes se suivent de gauche à droite (colonnes,
//! lignes par page, écart réglables), et où l'on note librement : pinceau, crayon ou formes, un
//! trait de note ; Texte, un texte ; gomme, efface les traits touchés ; Sélection, choisir et
//! déplacer des notes (ou des panneaux, au lasso). Une note peut être attachée à une vignette :
//! elle la suit. La vignette du panneau courant est le dessin lui-même (l'outil Caméra y
//! recadre). Molette : défiler ; Ctrl + molette : zoom ; Espace, bouton du milieu ou outil Main :
//! déplacer la planche. Panneaux : clic, Ctrl + clic, Maj + clic ; on les glisse (le panneau
//! courant par son titre) pour les déplacer ; clic droit : menus.

use std::time::Instant;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Event, FontId, Key, Modifiers, PointerButton, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Vec2,
};

use crate::app::{App, BoardView, Mode, Tool};
use crate::board::{self, BoardLayout, CardPlace, NoteRef, CARD_MAX, CARD_MIN, MAX_COLUMNS, MAX_ROWS, MAX_SPACING, NOTE_TEXT_SIZE};
use crate::storyboard::{duration_label, panel_title};
use crate::vector::VPoint;

/// Fond de la planche, pages, encre des textes.
const BOARD_BG: Color32 = Color32::from_gray(150);
const PAGE_BG: Color32 = Color32::WHITE;
const INK: Color32 = Color32::from_gray(30);
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
    /// Rectangle de choix ; `add` : ajouter à ce qui est déjà choisi.
    Rubber { start: Pos2, now: Pos2, add: bool },
    /// Notes choisies glissées ; `last` : le pointeur à l'image précédente (planche).
    MoveNotes { last: Pos2, recorded: bool },
}

/// Texte de note en cours d'écriture : nouveau (`index` : aucun) ou existant ; coin haut gauche
/// et taille sur la planche.
pub(crate) struct NoteTextEdit {
    index: Option<usize>,
    pos: Pos2,
    size: f32,
    buffer: String,
    focused: bool,
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

    /// La disposition de la planche, avec ses réglages.
    pub(crate) fn board_layout(&self) -> BoardLayout {
        board::layout(&self.doc, &self.layout.board_settings())
    }

    /// Après une opération qui a retiré des panneaux : les notes qui leur étaient attachées
    /// restent où elles étaient (`before` : la planche d'avant), libres.
    pub(crate) fn release_removed_notes(&mut self, before: &BoardLayout) {
        let gone: Vec<u64> =
            before.cards.iter().map(|c| c.id).filter(|id| !self.doc.panels.iter().any(|p| p.id == *id)).collect();
        if !gone.is_empty() {
            self.doc.board.release(before, &gone);
        }
    }

    /// La page à la largeur de la vue, en haut de la planche (fait à l'image suivante, quand la
    /// vue est connue : un zoom nul le demande).
    pub(crate) fn board_fit(&mut self) {
        self.board_zoom = 0.0;
        self.board_pan = Vec2::ZERO;
    }

    /// Zoom par cran (raccourcis), autour du coin haut gauche de la vue.
    pub(crate) fn board_zoom_step(&mut self, factor: f32) {
        if self.board_zoom <= 0.0 {
            self.board_zoom = 1.0;
        }
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
    pub(crate) fn board_selection(&self) -> Vec<u64> {
        let cur = self.doc.panels[self.cur_panel].id;
        self.doc.panels.iter().map(|p| p.id).filter(|id| *id == cur || self.board_sel.contains(id)).collect()
    }

    /// Retire les panneaux `ids` (une étape d'annulation).
    pub(crate) fn delete_panels(&mut self, ids: &[u64]) {
        self.end_stroke();
        let snap = self.snapshot();
        let before = self.board_layout();
        match self.doc.remove_panels(ids) {
            Some(i) => {
                self.release_removed_notes(&before);
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

    /// Retire les notes choisies (Suppr).
    pub(crate) fn delete_board_notes(&mut self) {
        if self.board_notes.is_empty() {
            return;
        }
        self.record_step();
        let sel = std::mem::take(&mut self.board_notes);
        self.doc.board.remove(&sel);
        self.status = "Notes supprimées.".into();
    }

    /// Ce que couvre chaque texte de note, sur la planche (pour les choisir et les toucher).
    fn text_rects(&self, painter: &egui::Painter, layout: &BoardLayout) -> Vec<Option<Rect>> {
        (0..self.doc.board.texts.len())
            .map(|i| {
                let (pos, size) = self.doc.board.text_place(layout, i)?;
                let t = &self.doc.board.texts[i];
                let shown = if t.text.is_empty() { " " } else { t.text.as_str() };
                let galley = painter.layout_no_wrap(shown.to_string(), FontId::proportional(size.max(1.0)), INK);
                Some(Rect::from_min_size(pos, galley.size()))
            })
            .collect()
    }

    /// La note sous le point `p` (planche) : un texte d'abord, sinon un trait.
    fn note_at(&self, layout: &BoardLayout, texts: &[Option<Rect>], p: Pos2, tol: f32) -> Option<NoteRef> {
        if let Some(i) = texts.iter().rposition(|r| r.is_some_and(|r| r.expand(tol).contains(p))) {
            return Some(NoteRef::Text(i));
        }
        self.doc.board.stroke_at(layout, p, tol).map(NoteRef::Stroke)
    }

    /// Ce que couvre une note, sur la planche.
    fn note_bounds(&self, layout: &BoardLayout, texts: &[Option<Rect>], n: NoteRef) -> Option<Rect> {
        match n {
            NoteRef::Stroke(i) => self.doc.board.stroke_points(layout, i).map(|p| board::points_bounds(&p)),
            NoteRef::Text(i) => texts.get(i).copied().flatten(),
        }
    }

    pub(crate) fn board_view(&mut self, ui: &mut egui::Ui) {
        self.board_toolbar(ui);
        let view = ui.available_rect_before_wrap();
        let bg = ui.allocate_rect(view, Sense::click_and_drag());
        let painter = ui.painter_at(view);
        painter.rect_filled(view, 0.0, BOARD_BG);
        self.board_sel.retain(|id| self.doc.panels.iter().any(|p| p.id == *id));
        let (ns, nt) = (self.doc.board.strokes.len(), self.doc.board.texts.len());
        self.board_notes.retain(|n| match *n {
            NoteRef::Stroke(i) => i < ns,
            NoteRef::Text(i) => i < nt,
        });
        let settings = self.layout.board_settings();
        let layout = board::layout(&self.doc, &settings);
        if self.board_zoom <= 0.0 {
            self.board_zoom = (view.width() / layout.size.x.max(1.0)).clamp(ZOOM_MIN, 1.5);
            self.board_pan = Vec2::ZERO;
        }

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
        let writing = self.board_text.is_some();
        let pan_mode = self.tool == Tool::Pan || (space && !writing);
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
        let texts = self.text_rects(&painter, &layout);
        let tol = 5.0 / xf.zoom;

        // Gestes commencés sur le fond.
        if bg.drag_started_by(PointerButton::Primary) && !pan_mode {
            self.commit_note_text(&layout);
            let start = xf.to_board(ui.input(|i| i.pointer.press_origin()).unwrap_or(view.center()));
            let add = mods.ctrl || mods.shift;
            self.board_drag = Some(match self.tool {
                Tool::Eraser => BoardDrag::Erase { recorded: false },
                Tool::Select => match self.note_at(&layout, &texts, start, tol) {
                    Some(n) => {
                        if !self.board_notes.contains(&n) {
                            if !add {
                                self.board_notes.clear();
                            }
                            self.board_notes.push(n);
                        }
                        BoardDrag::MoveNotes { last: start, recorded: false }
                    }
                    None => BoardDrag::Rubber { start, now: start, add },
                },
                Tool::Camera | Tool::Transform | Tool::Picker | Tool::Perspective | Tool::Ref3d | Tool::Text => {
                    BoardDrag::Rubber { start, now: start, add }
                }
                _ => BoardDrag::Note { points: vec![self.note_point(start, touch.is_some())], pen: touch.is_some() },
            });
        }
        if let Some(p) = bg.interact_pointer_pos().filter(|_| bg.clicked()).map(|p| xf.to_board(p)) {
            self.board_click(&layout, &texts, p, mods);
        }
        if let Some(p) = bg.interact_pointer_pos().filter(|_| bg.double_clicked()).map(|p| xf.to_board(p)) {
            if let Some(NoteRef::Text(i)) = self.note_at(&layout, &texts, p, tol) {
                self.edit_note_text(&layout, i);
            }
        }
        if escape && !writing {
            match self.board_drag {
                Some(BoardDrag::Move { .. } | BoardDrag::Rubber { .. }) => self.board_drag = None,
                _ => self.board_notes.clear(),
            }
        }
        let pointer = ui.input(|i| i.pointer.latest_pos()).map(|p| xf.to_board(p));
        self.update_board_drag(&layout, pointer, touch.is_some(), xf.zoom);

        // Pages, cases vides, vignettes.
        for page in &layout.pages {
            let r = xf.rect(*page);
            painter.rect_filled(r.translate(vec2(3.0, 4.0)), 0.0, Color32::from_black_alpha(50));
            painter.rect_filled(r, 0.0, PAGE_BG);
        }
        let font = (11.0 * xf.zoom).clamp(6.0, 26.0);
        for (k, slot) in layout.empty.iter().enumerate() {
            let r = xf.rect(*slot);
            if !r.intersects(view) {
                continue;
            }
            let thumb = Rect::from_min_max(pos2(r.left(), r.top() + board::HEADER_H * xf.zoom), r.max);
            painter.rect_stroke(thumb, 0.0, Stroke::new(1.0_f32, Color32::from_gray(190)), StrokeKind::Inside);
            painter.text(
                pos2(r.left() + 2.0, thumb.top() - 2.0),
                Align2::LEFT_BOTTOM,
                (layout.cards.len() + k + 1).to_string(),
                FontId::proportional(font),
                Color32::from_gray(170),
            );
        }
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        for place in &layout.cards {
            self.board_card(ui, &painter, view, xf, place, pan_mode, mods, deadline);
        }

        // Notes par-dessus, puis ce qui est choisi, puis les repères du geste.
        for i in 0..self.doc.board.strokes.len() {
            if let Some(points) = self.doc.board.stroke_points(&layout, i) {
                paint_note(&painter, view, xf, &points, self.doc.board.strokes[i].color);
            }
        }
        let editing = self.board_text.as_ref().and_then(|e| e.index);
        for i in 0..self.doc.board.texts.len() {
            let Some((pos, size)) = self.doc.board.text_place(&layout, i) else { continue };
            if editing == Some(i) {
                continue;
            }
            let t = &self.doc.board.texts[i];
            let [r, g, b] = t.color;
            painter.text(xf.to_screen(pos), Align2::LEFT_TOP, &t.text, FontId::proportional(size * xf.zoom), Color32::from_rgb(r, g, b));
        }
        let accent = Color32::from_rgb(40, 120, 230);
        for &n in &self.board_notes {
            if let Some(b) = self.note_bounds(&layout, &texts, n) {
                let r = xf.rect(b).expand(3.0);
                painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, accent), StrokeKind::Outside);
                let anchored = match n {
                    NoteRef::Stroke(i) => self.doc.board.strokes[i].anchor.is_some(),
                    NoteRef::Text(i) => self.doc.board.texts[i].anchor.is_some(),
                };
                if anchored {
                    painter.text(r.right_top() + vec2(2.0, -2.0), Align2::LEFT_BOTTOM, egui_phosphor::regular::PAPERCLIP, FontId::proportional(13.0), accent);
                }
            }
        }
        self.paint_board_drag(&painter, view, xf, &layout, settings.spacing);
        if let (Some(p), Tool::Eraser, false) = (over, self.tool, pan_mode) {
            if bg.hovered() || matches!(self.board_drag, Some(BoardDrag::Erase { .. })) {
                let r = self.note_eraser_radius(xf.zoom) * xf.zoom;
                painter.circle_stroke(p, r, Stroke::new(1.0_f32, INK));
            }
        }
        if bg.hovered() {
            let icon = if pan_mode {
                CursorIcon::Grab
            } else if self.tool == Tool::Text {
                CursorIcon::Text
            } else if matches!(self.tool, Tool::Eraser | Tool::Brush | Tool::Pencil) || self.tool.is_shape() {
                CursorIcon::Crosshair
            } else {
                CursorIcon::Default
            };
            ui.ctx().set_cursor_icon(icon);
        }
        bg.context_menu(|ui| self.board_menu(ui, &layout, &texts));
        self.note_text_editor(ui, xf, &layout);
        if !any_down {
            self.finish_board_drag(&layout, &texts, settings.spacing);
        }
    }

    /// Clic sur le fond : texte (outil Texte), choix d'une note (Sélection), sinon rien de choisi.
    fn board_click(&mut self, layout: &BoardLayout, texts: &[Option<Rect>], p: Pos2, mods: Modifiers) {
        let tol = 5.0 / self.board_zoom;
        let add = mods.ctrl || mods.shift;
        match self.tool {
            Tool::Text => {
                if self.board_text.is_some() {
                    self.commit_note_text(layout);
                } else if let Some(NoteRef::Text(i)) = self.note_at(layout, texts, p, tol) {
                    self.edit_note_text(layout, i);
                } else {
                    let size = NOTE_TEXT_SIZE;
                    self.board_text = Some(NoteTextEdit { index: None, pos: p, size, buffer: String::new(), focused: false });
                }
            }
            Tool::Select => match self.note_at(layout, texts, p, tol) {
                Some(n) if add => {
                    if let Some(k) = self.board_notes.iter().position(|m| *m == n) {
                        self.board_notes.remove(k);
                    } else {
                        self.board_notes.push(n);
                    }
                }
                Some(n) => self.board_notes = vec![n],
                None if !add => {
                    self.board_notes.clear();
                    self.board_sel.clear();
                }
                None => {}
            },
            _ => {
                self.commit_note_text(layout);
                if !add {
                    self.board_sel.clear();
                    self.board_notes.clear();
                }
            }
        }
    }

    /// Ouvre le texte `i` pour le modifier.
    fn edit_note_text(&mut self, layout: &BoardLayout, i: usize) {
        self.commit_note_text(layout);
        if let Some((pos, size)) = self.doc.board.text_place(layout, i) {
            let buffer = self.doc.board.texts[i].text.clone();
            self.board_text = Some(NoteTextEdit { index: Some(i), pos, size, buffer, focused: false });
        }
    }

    /// Pose le texte en cours (vide : rien, ou le texte existant est retiré). Une étape d'annulation.
    fn commit_note_text(&mut self, layout: &BoardLayout) {
        let Some(ed) = self.board_text.take() else { return };
        let text = ed.buffer.trim_end().to_string();
        match ed.index {
            Some(i) if i < self.doc.board.texts.len() => {
                if text.trim().is_empty() {
                    self.record_step();
                    self.doc.board.remove(&[NoteRef::Text(i)]);
                    self.board_notes.clear();
                } else if self.doc.board.texts[i].text != text {
                    self.record_step();
                    self.doc.board.texts[i].text = text;
                }
            }
            _ if !text.trim().is_empty() => {
                self.record_step();
                let anchor = self
                    .layout
                    .board_attach
                    .then(|| board::nearest_card(layout, Rect::from_min_size(ed.pos, vec2(40.0, ed.size))))
                    .flatten();
                self.doc.board.add_text(layout, ed.pos, text, ed.size, self.color, anchor);
            }
            _ => {}
        }
    }

    /// Le cadre où l'on écrit un texte de note, à sa place sur la planche.
    fn note_text_editor(&mut self, ui: &mut egui::Ui, xf: Xf, layout: &BoardLayout) {
        let Some(ed) = self.board_text.as_mut() else { return };
        let font = FontId::proportional((ed.size * xf.zoom).clamp(8.0, 72.0));
        let mut done = None;
        egui::Area::new(egui::Id::new("planche_texte"))
            .order(egui::Order::Foreground)
            .fixed_pos(xf.to_screen(ed.pos) - vec2(6.0, 6.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut ed.buffer)
                            .font(font)
                            .desired_rows(2)
                            .desired_width(300.0)
                            .hint_text("Note…"),
                    );
                    if !ed.focused {
                        resp.request_focus();
                        ed.focused = true;
                    }
                    let (enter, esc) = ui.input(|i| (i.modifiers.ctrl && i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                    ui.horizontal(|ui| {
                        use egui_phosphor::regular as ph;
                        if crate::bulles::icone(ui, ph::CHECK, "Poser la note", "Ctrl + Entrée.", "").clicked() || enter {
                            done = Some(true);
                        }
                        if crate::bulles::icone(ui, ph::X, "Annuler", "Annuler la saisie (Échap).", "").clicked() || esc {
                            done = Some(false);
                        }
                    });
                });
            });
        match done {
            Some(true) => self.commit_note_text(layout),
            Some(false) => self.board_text = None,
            None => {}
        }
    }

    fn board_toolbar(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, etiquette, interrupteur};
        use egui_phosphor::regular as ph;
        ui.horizontal_wrapped(|ui| {
            ui.add_space(6.0);
            etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Taille des vignettes", "");
            let mut size = self.layout.card_size;
            let resp = ui.add(egui::Slider::new(&mut size, CARD_MIN..=CARD_MAX).show_value(false));
            if bulle(resp, "Taille des vignettes", "", "").changed() {
                self.layout.card_size = size;
            }
            etiquette(ui, ph::COLUMNS, "Colonnes", "Vignettes par ligne (6 au plus).");
            let resp = ui.add(egui::DragValue::new(&mut self.layout.board_columns).range(1..=MAX_COLUMNS).speed(0.05));
            bulle(resp, "Colonnes", "Vignettes par ligne (6 au plus).", "");
            etiquette(ui, ph::ROWS, "Lignes par page", "« toutes » : une seule page.");
            let resp = ui.add(
                egui::DragValue::new(&mut self.layout.board_rows)
                    .range(0..=MAX_ROWS)
                    .speed(0.05)
                    .custom_formatter(|v, _| if v < 0.5 { "toutes".into() } else { format!("{v:.0}") }),
            );
            bulle(resp, "Lignes par page", "« toutes » : une seule page.", "");
            etiquette(ui, ph::ARROWS_OUT_LINE_HORIZONTAL, "Écart", "Entre les vignettes.");
            let resp = ui.add(egui::DragValue::new(&mut self.layout.board_spacing).range(0.0..=MAX_SPACING).speed(0.5).max_decimals(0));
            bulle(resp, "Écart", "Entre les vignettes.", "");
            ui.separator();
            interrupteur(
                ui,
                &mut self.layout.board_scene_breaks,
                ph::KEY_RETURN,
                "Scène à la ligne",
                "Chaque scène commence une nouvelle ligne (sinon les plans se suivent).",
                "",
            );
            interrupteur(
                ui,
                &mut self.layout.board_attach,
                ph::PAPERCLIP,
                "Attacher les notes",
                "Les nouvelles notes suivent la vignette la plus proche quand on la déplace ou la redimensionne.",
                "",
            );
            let resp = ui.button(format!("{:.0} %", self.board_zoom * 100.0));
            if bulle(resp, "Zoom de la planche", "Ctrl + molette. Clic : la page à la largeur de la vue.", "").clicked() {
                self.board_fit();
            }
            let n = self.board_selection().len();
            if n > 1 {
                ui.label(RichText::new(format!("{n} panneaux choisis")).strong());
            }
            if !self.board_notes.is_empty() {
                ui.label(RichText::new(format!("{} note(s) choisie(s)", self.board_notes.len())).strong());
            }
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

    fn update_board_drag(&mut self, layout: &BoardLayout, pointer: Option<Pos2>, touch: bool, zoom: f32) {
        let Some(p) = pointer else { return };
        let pen = matches!(self.board_drag, Some(BoardDrag::Note { pen: true, .. })) || touch;
        let point = self.note_point(p, pen);
        let radius = self.note_eraser_radius(zoom);
        match &mut self.board_drag {
            Some(BoardDrag::Note { points, pen: was }) => {
                *was |= touch;
                board::push_note_point(points, point, 1.0 / zoom);
            }
            Some(BoardDrag::Move { at, .. }) => *at = p,
            Some(BoardDrag::Rubber { now, .. }) => *now = p,
            _ => {}
        }
        // Gomme et déplacement de notes : l'étape d'annulation au premier changement du geste.
        if let Some(BoardDrag::Erase { recorded }) = self.board_drag {
            if self.doc.board.stroke_at(layout, p, radius).is_some() {
                if !recorded {
                    self.record_step();
                    self.board_drag = Some(BoardDrag::Erase { recorded: true });
                }
                self.doc.board.erase_at(layout, p, radius);
                self.board_notes.clear();
            }
        }
        if let Some(BoardDrag::MoveNotes { last, recorded }) = self.board_drag {
            let delta = p - last;
            if delta != Vec2::ZERO {
                if !recorded {
                    self.record_step();
                }
                self.board_drag = Some(BoardDrag::MoveNotes { last: p, recorded: true });
                let sel = self.board_notes.clone();
                self.doc.board.translate(layout, &sel, delta);
            }
        }
    }

    fn finish_board_drag(&mut self, layout: &BoardLayout, texts: &[Option<Rect>], gap: f32) {
        match self.board_drag.take() {
            Some(BoardDrag::Note { points, .. }) if !points.is_empty() => {
                self.record_step();
                let anchor = self.layout.board_attach.then(|| board::nearest_card(layout, board::points_bounds(&points))).flatten();
                self.doc.board.add_stroke(layout, points, self.color, anchor);
            }
            Some(BoardDrag::Move { ids, at }) => {
                let Some(t) = board::drop_target(layout, at, gap) else { return };
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
                let r = Rect::from_two_pos(start, now);
                let found: Vec<u64> = board::cards_in(layout, r).into_iter().map(|i| self.doc.panels[i].id).collect();
                let mut notes = self.doc.board.strokes_in(layout, r);
                notes.extend((0..texts.len()).filter(|&i| texts[i].is_some_and(|t| t.intersects(r))).map(NoteRef::Text));
                if add {
                    for id in found {
                        if !self.board_sel.contains(&id) {
                            self.board_sel.push(id);
                        }
                    }
                    for n in notes {
                        if !self.board_notes.contains(&n) {
                            self.board_notes.push(n);
                        }
                    }
                } else {
                    if let Some(&first) = found.first() {
                        if let Some(i) = self.doc.panels.iter().position(|p| p.id == first) {
                            self.select_panel(i);
                        }
                    }
                    self.board_sel = found;
                    self.board_notes = notes;
                }
            }
            _ => {}
        }
    }

    /// Une vignette : numéro et titre, image (ou le dessin lui-même pour le panneau courant),
    /// cadre, gestes.
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
        let id = place.id;
        let current = i == self.cur_panel;
        let chosen = current || self.board_sel.contains(&id);
        let accent = ui.visuals().selection.bg_fill;

        let head_ink = if chosen {
            painter.rect_filled(header, 0.0, if current { accent } else { accent.gamma_multiply(0.6) });
            Color32::WHITE
        } else {
            INK
        };
        let font = (11.0 * xf.zoom).clamp(6.0, 26.0);
        if header.height() >= 8.0 {
            let clip = painter.with_clip_rect(header.intersect(view));
            let number = clip.layout_no_wrap((i + 1).to_string(), FontId::proportional(font), head_ink);
            let w = number.size().x;
            let left = header.left() + 3.0 * xf.zoom.min(1.5);
            clip.galley(pos2(left, header.center().y - number.size().y * 0.5), number, head_ink);
            // Début de scène : son titre en gras (tracé deux fois, un demi-point à côté).
            let first = i == 0 || self.doc.panels[i - 1].scene != place.scene;
            let galley = clip.layout_no_wrap(panel_title(&self.doc, i), FontId::proportional(font), head_ink);
            let at = pos2(left + w + 6.0 * xf.zoom.min(1.5), header.center().y - galley.size().y * 0.5);
            if first {
                clip.galley(at + vec2(0.6, 0.0), galley.clone(), head_ink);
            }
            clip.galley(at, galley, head_ink);
            clip.text(
                pos2(header.right() - 3.0 * xf.zoom.min(1.5), header.center().y),
                Align2::RIGHT_CENTER,
                duration_label(self.doc.panels[i].frame_count(), self.doc.fps),
                FontId::monospace(font * 0.85),
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
        painter.rect_stroke(thumb, 0.0, Stroke::new(1.0_f32, Color32::from_gray(40)), StrokeKind::Outside);
        if chosen {
            let w: f32 = if current { 3.0 } else { 2.0 };
            painter.rect_stroke(card.expand(1.0), 2.0, Stroke::new(w, accent), StrokeKind::Outside);
        }

        // Gestes : le titre seulement sur le panneau courant (son image sert à dessiner).
        let hit = if live { header } else { card }.intersect(view);
        let resp = ui.interact(hit, ui.id().with(("planche_carte", id)), Sense::click_and_drag());
        // Un clic sur une vignette donne la main aux panneaux : copier, coller, supprimer (12e).
        if resp.is_pointer_button_down_on() {
            self.panel_focus = true;
        }
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
        ui.separator();
        self.panel_clipboard_items(ui);
        ui.separator();
        item(self, ui, "Nouvelle scène à partir de ce panneau", &|a| a.run_action("scene.couper"));
        item(self, ui, "Recadrer (outil Caméra)", &|a| a.set_tool(Tool::Camera));
        item(self, ui, "Réinitialiser le cadrage", &|a| a.run_action("camera.reinitialiser"));
    }

    /// Menu du fond de la planche (clic droit) : les notes choisies, puis la planche.
    fn board_menu(&mut self, ui: &mut egui::Ui, layout: &BoardLayout, texts: &[Option<Rect>]) {
        if !self.board_notes.is_empty() {
            let sel = self.board_notes.clone();
            ui.label(RichText::new(format!("{} note(s) choisie(s)", sel.len())).weak());
            if ui.button(format!("{} Attacher à la vignette la plus proche", egui_phosphor::regular::PAPERCLIP)).clicked() {
                ui.close_menu();
                self.record_step();
                for n in &sel {
                    let target = self.note_bounds(layout, texts, *n).and_then(|b| board::nearest_card(layout, b));
                    self.doc.board.set_anchor(layout, &[*n], target);
                }
            }
            if ui.button("Détacher (laisser sur la planche)").clicked() {
                ui.close_menu();
                self.record_step();
                self.doc.board.set_anchor(layout, &sel, None);
            }
            if ui.button("Couleur courante").clicked() {
                ui.close_menu();
                self.record_step();
                self.doc.board.recolor(&sel, self.color);
            }
            let has_text = sel.iter().any(|n| matches!(n, NoteRef::Text(_)));
            for (label, k) in [("Texte plus grand", 1.25), ("Texte plus petit", 0.8)] {
                if has_text && ui.button(label).clicked() {
                    ui.close_menu();
                    self.record_step();
                    for n in &sel {
                        if let NoteRef::Text(i) = n {
                            if let Some(t) = self.doc.board.texts.get_mut(*i) {
                                t.size = (t.size * k).clamp(0.01, 500.0);
                            }
                        }
                    }
                }
            }
            if ui.button("Supprimer les notes choisies (Suppr)").clicked() {
                ui.close_menu();
                self.delete_board_notes();
            }
            ui.separator();
        }
        if ui.button("Nouveau panneau").clicked() {
            ui.close_menu();
            self.new_panel();
        }
        if ui.button("Nouvelle scène").clicked() {
            ui.close_menu();
            self.new_scene();
        }
        ui.separator();
        if ui.button("Tout choisir (panneaux)").clicked() {
            ui.close_menu();
            self.board_sel = self.doc.panels.iter().map(|p| p.id).collect();
        }
        if ui.button("Page à la largeur de la vue").clicked() {
            ui.close_menu();
            self.board_fit();
        }
        ui.separator();
        let any = !self.doc.board.strokes.is_empty() || !self.doc.board.texts.is_empty();
        if ui.add_enabled(any, egui::Button::new("Effacer toutes les notes")).clicked() {
            ui.close_menu();
            self.record_step();
            self.doc.board = Default::default();
            self.board_notes.clear();
        }
    }

    /// Repères du geste en cours : note, lasso, place où tomberont les panneaux.
    fn paint_board_drag(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, gap: f32) {
        let accent = Color32::from_rgb(40, 120, 230);
        match &self.board_drag {
            Some(BoardDrag::Note { points, .. }) => paint_note(painter, view, xf, points, self.color),
            Some(BoardDrag::Rubber { start, now, .. }) => {
                let r = Rect::from_two_pos(xf.to_screen(*start), xf.to_screen(*now));
                painter.rect_filled(r, 0.0, accent.gamma_multiply(0.15));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, accent), StrokeKind::Inside);
            }
            Some(BoardDrag::Move { ids, at }) => {
                if let Some(t) = board::drop_target(layout, *at, gap) {
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
