//! Outils de forme (5c) : Ligne (droite, courbe, courbe en S), Rectangle, Ellipse, Polyligne.
//! Sur un calque vectoriel, la forme devient un élément modifiable ; sur un calque bitmap, des
//! pixels. Géométrie dans `shapes.rs`.

use eframe::egui::{self, Color32, Key, Pos2, Shape, Stroke};

use crate::app::{App, ShapeKind, Tool};
use crate::document::LayerKind;
use crate::shapes::{self, LineMode, ShapeEdit};
use crate::vector;

/// Écart (pixels d'écran) en deçà duquel deux clics de polyligne font un double-clic, ou un
/// clic referme la forme sur son premier point.
const CLOSE_GRAB: f32 = 8.0;
/// Délai d'un double-clic, en secondes.
const DOUBLE_CLICK: f64 = 0.35;

impl App {
    /// Points d'une ligne, d'un rectangle ou d'une ellipse tracés de `a` à `b`.
    fn drag_shape_points(&self, kind: ShapeKind, a: Pos2, b: Pos2, shift: bool) -> Vec<Pos2> {
        match kind {
            ShapeKind::Line => vec![a, if shift { shapes::snap_angle(a, b) } else { b }],
            ShapeKind::Rect | ShapeKind::Ellipse => {
                let b = if shift { shapes::square_corner(a, b) } else { b };
                if kind == ShapeKind::Rect {
                    shapes::rect_points(a, b)
                } else {
                    shapes::ellipse_points(a, b)
                }
            }
        }
    }

    pub(crate) fn shape_press(&mut self, p: Pos2, shift: bool) {
        self.playing = false;
        // Courbure d'une ligne déjà tracée.
        if let Some(e @ ShapeEdit::Curve { .. }) = &mut self.shape_edit {
            if let ShapeEdit::Curve { bending, .. } = e {
                *bending = true;
            }
            e.bend_to(p);
            return;
        }
        if self.tool == Tool::Polyline {
            self.polyline_press(p, shift);
            return;
        }
        self.end_stroke();
        self.shape_drag = Some((p, p));
    }

    fn polyline_press(&mut self, p: Pos2, shift: bool) {
        let now = self.ctx.input(|i| i.time);
        let tol = CLOSE_GRAB / self.layer_zoom().max(1e-3);
        let Some(ShapeEdit::Polyline { points, last_press }) = &mut self.shape_edit else {
            self.end_stroke();
            self.shape_edit = Some(ShapeEdit::Polyline { points: vec![p], last_press: Some((now, p)) });
            self.status = "Polyligne : un clic par point ; double-clic ou Entrée pour finir, clic sur le \
                           premier point pour fermer, Retour arrière pour retirer un point, Échap pour annuler."
                .into();
            return;
        };
        // Double-clic : on termine (forme ouverte, ou fermée si l'option est cochée).
        if last_press.is_some_and(|(t, q)| now - t < DOUBLE_CLICK && q.distance(p) <= tol) {
            self.finish_shape_edit();
            return;
        }
        if shapes::closes(points, p, tol) {
            let pts = points.clone();
            self.shape_edit = None;
            self.commit_shape_points(&pts, true, self.shape.filled);
            return;
        }
        let q = match (shift, points.last()) {
            (true, Some(&last)) => shapes::snap_angle(last, p),
            _ => p,
        };
        points.push(q);
        *last_press = Some((now, p));
    }

    /// Mouvement du pointeur pendant une courbure ; vrai s'il y en a une.
    pub(crate) fn shape_bend_move(&mut self, p: Pos2) -> bool {
        match &mut self.shape_edit {
            Some(e @ ShapeEdit::Curve { bending: true, .. }) => {
                e.bend_to(p);
                true
            }
            _ => false,
        }
    }

    /// Relâchement : fin d'un tracé de forme ou d'une courbure.
    pub(crate) fn shape_release(&mut self, shift: bool) {
        if let Some((a, b)) = self.shape_drag.take() {
            let Some(kind) = self.tool.shape_kind() else { return };
            let pts = self.drag_shape_points(kind, a, b, shift);
            // Trop petite à l'écran : rien (le contour d'un rectangle revient à son départ, on
            // mesure donc le point le plus éloigné).
            let reach = pts.iter().map(|p| a.distance(*p)).fold(0.0, f32::max);
            if !shapes::big_enough(reach, self.layer_zoom()) {
                return;
            }
            if kind == ShapeKind::Line && self.shape.line_mode != LineMode::Straight {
                self.shape_edit = Some(ShapeEdit::curve(a, pts[1], self.shape.line_mode));
                self.status = "Ligne courbe : glisser pour courber la ligne (Entrée : la poser telle quelle, \
                               Échap : annuler)."
                    .into();
                return;
            }
            let closed = kind != ShapeKind::Line;
            self.commit_shape_points(&pts, closed, closed && self.shape.filled);
            return;
        }
        let done = match &mut self.shape_edit {
            Some(e @ ShapeEdit::Curve { bending: true, .. }) => e.end_bend(),
            _ => false,
        };
        if done {
            self.finish_shape_edit();
        }
    }

    /// Pose la forme en cours (courbe telle quelle, polyligne avec ses points).
    pub(crate) fn finish_shape_edit(&mut self) {
        match self.shape_edit.take() {
            Some(e @ ShapeEdit::Curve { .. }) => {
                self.commit_shape_points(&e.points(), false, false);
            }
            Some(ShapeEdit::Polyline { points, .. }) => {
                let closed = self.shape.closed && points.len() >= 3;
                self.commit_shape_points(&points, closed, closed && self.shape.filled);
            }
            None => {}
        }
    }

    /// Touches pendant une forme en plusieurs gestes : Entrée la pose, Échap l'annule, Retour
    /// arrière retire le dernier point d'une polyligne. Vrai si la touche a servi.
    pub(crate) fn shape_key(&mut self, key: Key) -> bool {
        if self.shape_edit.is_none() {
            return false;
        }
        match key {
            Key::Enter => self.finish_shape_edit(),
            Key::Escape => {
                self.shape_edit = None;
                self.status = "Forme annulée.".into();
            }
            Key::Backspace => {
                if let Some(ShapeEdit::Polyline { points, .. }) = &mut self.shape_edit {
                    points.pop();
                    if points.is_empty() {
                        self.shape_edit = None;
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Ajoute la forme au dessin courant.
    pub(crate) fn commit_shape_points(&mut self, points: &[Pos2], closed: bool, filled: bool) {
        if let Some(element) = shapes::element(points, closed, filled, self.shape.width, self.color) {
            self.commit_element(element);
        }
    }

    /// Ajoute un élément au dessin courant : élément vectoriel modifiable, ou pixels sur un
    /// calque bitmap. Une étape d'annulation.
    pub(crate) fn commit_element(&mut self, element: vector::Element) {
        if !self.layer_editable() {
            return;
        }
        let kind = self.layer().kind;
        let (cel, prefix) = self.ensure_cel();
        match kind {
            LayerKind::Vector => {
                let Some(d) = self.drawing_mut(cel) else { return };
                let before = d.clone();
                d.elements.push(element);
                self.refresh_cel(cel);
                self.push_vector(cel, before, prefix);
            }
            LayerKind::Bitmap => {
                let Some(c) = self.doc.cels.get_mut(&cel) else { return };
                let tiles = vector::tiles_in(&c.raster, element.bounds().expand(2.0));
                let before = tiles.iter().map(|&i| (i, c.raster.tiles[i].clone())).collect();
                vector::render_element(&mut c.raster, &element);
                self.push_pixels(cel, prefix, before);
            }
        }
    }

    /// Aperçu de la forme en cours, en coordonnées du dessin du calque courant.
    pub(crate) fn paint_shape_overlay(&self, painter: &egui::Painter, shift: bool) {
        let to_s = |p: Pos2| self.layer_to_screen(p);
        let width = (self.shape.width * self.layer_zoom()).clamp(1.0, 40.0);
        let color = self.preview_color();
        let guide = Stroke::new(1.0_f32, Color32::from_rgb(70, 165, 255));
        if let (Some((a, b)), Some(kind)) = (self.shape_drag, self.tool.shape_kind()) {
            let pts: Vec<Pos2> = self.drag_shape_points(kind, a, b, shift).into_iter().map(to_s).collect();
            painter.add(Shape::line(pts, Stroke::new(width, color)));
        }
        match &self.shape_edit {
            Some(e @ ShapeEdit::Curve { a, b, c1, c2, .. }) => {
                painter.add(Shape::line(e.points().into_iter().map(to_s).collect(), Stroke::new(width, color)));
                for (end, c) in [(*a, *c1), (*b, *c2)] {
                    painter.extend(Shape::dashed_line(&[to_s(end), to_s(c)], guide, 4.0, 4.0));
                    painter.circle(to_s(c), 3.5, Color32::WHITE, guide);
                }
            }
            Some(ShapeEdit::Polyline { points, .. }) => {
                let mut line: Vec<Pos2> = points.iter().map(|p| to_s(*p)).collect();
                // Segment en attente jusqu'au pointeur.
                if let Some(h) = self.hover_canvas.map(|c| self.to_layer(c)) {
                    let near_first = shapes::closes(points, h, CLOSE_GRAB / self.layer_zoom().max(1e-3));
                    line.push(if near_first { to_s(points[0]) } else { to_s(h) });
                    if near_first {
                        painter.circle_stroke(to_s(points[0]), 7.0, Stroke::new(2.0_f32, Color32::WHITE));
                    }
                }
                painter.add(Shape::line(line, Stroke::new(width, color)));
                for p in points {
                    painter.rect_stroke(
                        egui::Rect::from_center_size(to_s(*p), egui::vec2(6.0, 6.0)),
                        0.0,
                        guide,
                        egui::StrokeKind::Middle,
                    );
                }
            }
            None => {}
        }
    }

    /// Réglages des outils de forme (panneau Réglages).
    pub(crate) fn shape_options_ui(&mut self, ui: &mut egui::Ui) {
        let note = |ui: &mut egui::Ui, text: &str| {
            ui.label(egui::RichText::new(text).small().weak());
        };
        use crate::bulles::interrupteur;
        use egui_phosphor::regular as ph;
        if self.tool == Tool::Line {
            ui.horizontal(|ui| {
                let key = |a: &str| self.raccourci(a);
                let (k1, k2, k3) = (key("outil.ligne_droite"), key("outil.ligne_courbe"), key("outil.ligne_s"));
                let mode = &mut self.shape.line_mode;
                crate::bulles::choix_a(ui, mode, LineMode::Straight, ph::LINE_SEGMENT, "Droite", "Un segment droit.", &k1, "outil.ligne_droite");
                crate::bulles::choix_a(ui, mode, LineMode::Curve, ph::BEZIER_CURVE, "Courbe", "Un arc qui passe par le pointeur.", &k2, "outil.ligne_courbe");
                crate::bulles::choix_a(ui, mode, LineMode::SCurve, ph::WAVE_SINE, "Courbe en S", "Une courbe à deux bosses.", &k3, "outil.ligne_s");
            });
        }
        // Épaisseur, puis les interrupteurs de la forme au bout de la même ligne.
        let filled = self.shape.filled;
        ui.horizontal(|ui| {
            ui.add_enabled(
                !(filled && self.tool != Tool::Line),
                egui::Slider::new(&mut self.shape.width, 0.5..=100.0).logarithmic(true).text("Épaisseur"),
            );
            if self.tool == Tool::Polyline {
                interrupteur(ui, &mut self.shape.closed, ph::POLYGON, "Fermer la forme", "Relier le dernier point au premier en terminant.", "");
            }
            if matches!(self.tool, Tool::Rect | Tool::Ellipse) || (self.tool == Tool::Polyline && self.shape.closed) {
                interrupteur(ui, &mut self.shape.filled, ph::SQUARE_HALF, "Forme pleine", "Remplie de la couleur courante, sans contour.", "");
            }
        });
        note(
            ui,
            match self.tool {
                Tool::Line => match self.shape.line_mode {
                    LineMode::Straight => "Glisser pour tracer. Maj : angle par pas de 15°.",
                    LineMode::Curve => "Glisser pour tracer, puis glisser pour courber : la courbe passe par le pointeur.",
                    LineMode::SCurve => {
                        "Glisser pour tracer, puis deux glissers pour courber (le second fait le S). \
                         Entrée : poser, Échap : annuler."
                    }
                },
                Tool::Polyline => {
                    "Un clic par point (Maj : par 15°). Double-clic ou Entrée : terminer. Clic sur le premier \
                     point : fermer. Retour arrière : retirer un point. Échap : annuler."
                }
                _ => "Glisser pour tracer. Maj : carré ou cercle.",
            },
        );
    }
}
