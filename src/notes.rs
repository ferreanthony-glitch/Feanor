//! Notes dessinées sur la timeline (13e), comme la partie Notes de TVPaint : des remarques
//! griffonnées dans une bande alignée sur les images du plan, montrées par-dessus l'image
//! pendant la lecture, jamais exportées.
//!
//! Outils : crayon (bleu, rouge ou noir), gomme (les traits de la couleur choisie), gomme
//! globale (tous), teinte (repeint un trait de la couleur choisie), position (glisse toutes
//! les notes dans le temps), découper (glisse seulement ce qui est à droite du clic). Effacer
//! tout, effacer ou teinter tous les traits d'une couleur ; une grille verticale toutes les n
//! images ; placement et taille de l'affichage pendant la lecture.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Shape, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::document::TimeNote;

/// Hauteur de la rangée des notes dans la timeline.
pub(crate) const NOTES_H: f32 = 72.0;

/// Les trois couleurs des notes.
pub(crate) const COLORS: [(&str, Color32); 3] = [
    ("Bleu", Color32::from_rgb(40, 110, 230)),
    ("Rouge", Color32::from_rgb(220, 45, 45)),
    ("Noir", Color32::from_rgb(20, 20, 20)),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteTool {
    Pencil,
    Eraser,
    GlobalEraser,
    Tint,
    Position,
    Cut,
}

impl NoteTool {
    const ALL: [NoteTool; 6] = [NoteTool::Pencil, NoteTool::Eraser, NoteTool::GlobalEraser, NoteTool::Tint, NoteTool::Position, NoteTool::Cut];

    fn label(self) -> (&'static str, &'static str, &'static str) {
        match self {
            NoteTool::Pencil => (ph::PENCIL_SIMPLE, "Crayon", "Dessiner une note de la couleur choisie."),
            NoteTool::Eraser => (ph::ERASER, "Gomme", "Effacer les traits de la couleur choisie."),
            NoteTool::GlobalEraser => (ph::BROOM, "Gomme globale", "Effacer les traits, quelle que soit leur couleur."),
            NoteTool::Tint => (ph::PAINT_BUCKET, "Teinte", "Repeindre un trait de la couleur choisie."),
            NoteTool::Position => (ph::ARROWS_HORIZONTAL, "Position", "Glisser toutes les notes dans le temps."),
            NoteTool::Cut => (ph::SCISSORS, "Découper", "Glisser seulement les notes à droite du clic."),
        }
    }
}

/// Où montrer les notes par-dessus l'image pendant la lecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotesPlace {
    Hidden,
    Top,
    Bottom,
    Center,
}

impl NotesPlace {
    const ALL: [NotesPlace; 4] = [NotesPlace::Hidden, NotesPlace::Top, NotesPlace::Bottom, NotesPlace::Center];

    fn label(self) -> &'static str {
        match self {
            NotesPlace::Hidden => "Aucune",
            NotesPlace::Top => "En haut",
            NotesPlace::Bottom => "En bas",
            NotesPlace::Center => "Au centre",
        }
    }
}

/// Réglages et geste en cours des notes.
pub(crate) struct NotesState {
    pub tool: NoteTool,
    pub color: u8,
    pub size: f32,
    /// Une ligne verticale toutes les `grid` images (0 : aucune).
    pub grid: usize,
    pub place: NotesPlace,
    /// Taille de l'affichage par-dessus l'image (1 : taille réelle).
    pub scale: f32,
    /// Aussi à l'arrêt (sinon : seulement pendant la lecture).
    pub always: bool,
    /// Trait en cours (crayon), ou glisser en cours (position, découper : image du clic).
    stroke: Option<TimeNote>,
    slide: Option<(f32, Vec<TimeNote>)>,
}

impl Default for NotesState {
    fn default() -> Self {
        Self { tool: NoteTool::Pencil, color: 0, size: 2.0, grid: 0, place: NotesPlace::Bottom, scale: 1.0, always: false, stroke: None, slide: None }
    }
}

/// Distance d'un point à un trait (segments), en points d'écran.
fn touches(note: &TimeNote, at: Pos2, to_screen: &impl Fn([f32; 2]) -> Pos2, radius: f32) -> bool {
    let pts: Vec<Pos2> = note.points.iter().map(|p| to_screen(*p)).collect();
    if pts.len() == 1 {
        return pts[0].distance(at) <= radius;
    }
    pts.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let t = ((at - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
        (a + ab * t).distance(at) <= radius + note.width * 0.5
    })
}

/// Glisse dans le temps les notes (toutes, ou les points à droite de `from` pour « découper »).
pub(crate) fn slide_notes(notes: &mut [TimeNote], dx: f32, from: Option<f32>) {
    for n in notes {
        for p in &mut n.points {
            if from.is_none_or(|x| p[0] >= x) {
                p[0] += dx;
            }
        }
    }
}

fn paint_note(painter: &egui::Painter, note: &TimeNote, to_screen: &impl Fn([f32; 2]) -> Pos2, scale: f32) {
    let color = COLORS[note.color.min(2) as usize].1;
    let pts: Vec<Pos2> = note.points.iter().map(|p| to_screen(*p)).collect();
    let stroke = Stroke::new(note.width * scale, color);
    if pts.len() == 1 {
        painter.circle_filled(pts[0], note.width * scale * 0.5, color);
    } else {
        painter.add(Shape::line(pts, stroke));
    }
}

impl App {
    /// Réglages des notes (onglet Notes du bas de la timeline).
    pub(crate) fn notes_strip_ui(&mut self, ui: &mut egui::Ui) {
        for tool in NoteTool::ALL {
            let (icon, name, description) = tool.label();
            let resp = ui.add(egui::Button::new(RichText::new(icon).size(crate::bulles::TAILLE)).selected(self.notes.tool == tool));
            if bulle(resp, name, description, "").clicked() {
                self.notes.tool = tool;
            }
        }
        ui.separator();
        for (k, (name, color)) in COLORS.iter().enumerate() {
            let chosen = self.notes.color == k as u8;
            let resp = ui.add(egui::Button::new(RichText::new(if chosen { "●" } else { "○" }).color(*color).size(16.0)).frame(false));
            if bulle(resp, name, "Couleur des notes.", "").clicked() {
                self.notes.color = k as u8;
            }
        }
        ui.add(egui::DragValue::new(&mut self.notes.size).range(0.5..=20.0).speed(0.1).prefix("taille "));
        ui.separator();
        let color_name = COLORS[self.notes.color as usize].0.to_lowercase();
        if ui.button(format!("Effacer le {color_name}")).on_hover_text("Effacer tous les traits de la couleur choisie.").clicked() {
            let k = self.notes.color;
            self.change_notes(move |n| n.retain(|x| x.color != k));
        }
        if ui.button(format!("Tout en {color_name}")).on_hover_text("Teinter tous les traits avec la couleur choisie.").clicked() {
            let k = self.notes.color;
            self.change_notes(move |n| n.iter_mut().for_each(|x| x.color = k));
        }
        if ui.button("Tout effacer").clicked() {
            self.change_notes(|n| n.clear());
        }
        ui.separator();
        ui.add(egui::DragValue::new(&mut self.notes.grid).range(0..=100).prefix("grille toutes les ").suffix(" images"))
            .on_hover_text("Une ligne verticale toutes les n images, pour caler les notes ; 0 : aucune.");
        ui.separator();
        egui::ComboBox::from_id_salt("notes_place").selected_text(format!("Pendant la lecture : {}", self.notes.place.label())).show_ui(ui, |ui| {
            for p in NotesPlace::ALL {
                ui.selectable_value(&mut self.notes.place, p, p.label());
            }
        });
        ui.add(egui::DragValue::new(&mut self.notes.scale).range(0.25..=3.0).speed(0.01).prefix("taille ×"))
            .on_hover_text("1 : taille réelle (comme dans la timeline).");
        ui.checkbox(&mut self.notes.always, "Aussi à l'arrêt");
    }

    /// Change les notes du plan courant (une étape d'annulation).
    fn change_notes(&mut self, change: impl FnOnce(&mut Vec<TimeNote>)) {
        self.record_step();
        change(&mut self.panel_mut().cues.notes);
    }

    /// La rangée des notes dans la grille : `row` (son rectangle), `left` (abscisse de l'image
    /// 0), `cell` (largeur d'une image), `resp` : la réponse de la grille.
    pub(crate) fn notes_row(&mut self, ui: &egui::Ui, painter: &egui::Painter, row: Rect, left: f32, cell: f32, resp: &egui::Response) {
        painter.rect_filled(row, 0.0, Color32::from_gray(232));
        if self.notes.grid > 0 {
            let step = self.notes.grid;
            let mut f = 0;
            while left + f as f32 * cell < row.right() {
                let x = left + f as f32 * cell;
                painter.line_segment([pos2(x, row.top()), pos2(x, row.bottom())], Stroke::new(1.0_f32, Color32::from_gray(190)));
                f += step;
            }
        }
        let to_screen = |p: [f32; 2]| pos2(left + p[0] * cell, row.top() + p[1] * row.height());
        let to_note = |p: Pos2| [(p.x - left) / cell, ((p.y - row.top()) / row.height()).clamp(0.0, 1.0)];
        let shown: Vec<TimeNote> = self.panel().cues.notes.clone();
        for n in shown.iter().chain(self.notes.stroke.iter()) {
            paint_note(painter, n, &to_screen, 1.0);
        }
        if shown.is_empty() && self.notes.stroke.is_none() {
            painter.text(pos2(row.left() + 8.0, row.center().y), Align2::LEFT_CENTER, "Notes : dessiner ici", FontId::proportional(11.0), Color32::from_gray(150));
        }

        // Gestes : seulement ceux qui commencent dans la rangée.
        let origin = ui.input(|i| i.pointer.press_origin());
        let started_here = origin.is_some_and(|p| row.contains(p));
        let pointer = resp.interact_pointer_pos();
        let pressed = resp.is_pointer_button_down_on() && started_here;
        let tool = self.notes.tool;
        if pressed {
            let Some(p) = pointer else { return };
            match tool {
                NoteTool::Pencil => {
                    let (color, width) = (self.notes.color, self.notes.size);
                    let s = self.notes.stroke.get_or_insert_with(|| TimeNote { color, width, points: Vec::new() });
                    let q = to_note(p);
                    if s.points.last().is_none_or(|l| to_screen(*l).distance(p) >= 1.5) {
                        s.points.push(q);
                    }
                }
                NoteTool::Eraser | NoteTool::GlobalEraser | NoteTool::Tint => {
                    let k = self.notes.color;
                    let radius = self.notes.size * 2.0 + 3.0;
                    let hit: Vec<usize> = self.panel().cues.notes.iter().enumerate().filter(|(_, n)| {
                        let wanted = match tool {
                            NoteTool::Eraser => n.color == k,
                            NoteTool::Tint => n.color != k,
                            _ => true,
                        };
                        wanted && touches(n, p, &to_screen, radius)
                    }).map(|(i, _)| i).collect();
                    if !hit.is_empty() {
                        // Une étape d'annulation par geste (au premier trait touché).
                        self.note_change(resp.id);
                        let notes = &mut self.panel_mut().cues.notes;
                        for &i in hit.iter().rev() {
                            if tool == NoteTool::Tint {
                                notes[i].color = k;
                            } else {
                                notes.remove(i);
                            }
                        }
                    }
                }
                NoteTool::Position | NoteTool::Cut => {
                    let x = to_note(p)[0];
                    match &self.notes.slide {
                        None => {
                            let before = self.panel().cues.notes.clone();
                            self.notes.slide = Some((x, before));
                            self.record_step();
                        }
                        Some((x0, before)) => {
                            let (x0, mut notes) = (*x0, before.clone());
                            let from = (tool == NoteTool::Cut).then_some(x0);
                            slide_notes(&mut notes, x - x0, from);
                            self.panel_mut().cues.notes = notes;
                        }
                    }
                }
            }
        } else {
            if let Some(s) = self.notes.stroke.take() {
                if !s.points.is_empty() {
                    self.change_notes(move |n| n.push(s));
                }
            }
            self.notes.slide = None;
        }
    }

    /// Les notes par-dessus l'image (pendant la lecture, ou toujours si on le veut) : une bande
    /// centrée sur l'image courante, qui défile avec elle.
    pub(crate) fn paint_notes_overlay(&self, painter: &egui::Painter, area: Rect) {
        if self.notes.place == NotesPlace::Hidden || !(self.playing || self.notes.always) {
            return;
        }
        let notes = &self.panel().cues.notes;
        if notes.is_empty() {
            return;
        }
        let scale = self.notes.scale;
        let h = (NOTES_H * scale).min(area.height());
        let w = area.width() * 0.9;
        let y = match self.notes.place {
            NotesPlace::Top => area.top() + 8.0,
            NotesPlace::Center => area.center().y - h * 0.5,
            _ => area.bottom() - h - 8.0,
        };
        let band = Rect::from_min_size(pos2(area.center().x - w * 0.5, y), vec2(w, h));
        painter.rect_filled(band, 4.0, Color32::from_white_alpha(200));
        let cell = self.timeline_cell * scale;
        // L'image courante au milieu de la bande.
        let left = band.center().x - (self.cur_frame as f32 + 0.5) * cell;
        let to_screen = |p: [f32; 2]| pos2(left + p[0] * cell, band.top() + p[1] * band.height());
        let clipped = painter.with_clip_rect(band);
        for n in notes {
            paint_note(&clipped, n, &to_screen, scale);
        }
        clipped.line_segment([pos2(band.center().x, band.top()), pos2(band.center().x, band.bottom())], Stroke::new(1.5_f32, crate::theme::image_courante()));
    }
}
