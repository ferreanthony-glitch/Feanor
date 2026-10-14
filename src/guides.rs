//! Guides de perspective à 1, 2 ou 3 points de fuite : affichage sur le dessin, lignes de
//! référence, lignes qui suivent le curseur, et traits alignés sur le guide.
//!
//! Un trait aligné part du point cliqué ; dès que le pointeur a un peu bougé, on choisit la
//! direction du guide la plus proche du mouvement (vers un point de fuite, ou verticale /
//! horizontale selon le guide) et tout le trait suit cette droite.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use serde::{Deserialize, Serialize};

use crate::app::{App, Tool};

/// Un guide : ses points de fuite, en pixels du dessin (souvent hors du cadre).
/// 1 point : fuyantes + horizontales et verticales. 2 points : deux fuyantes sur l'horizon +
/// verticales. 3 points : le troisième remplace les verticales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    #[serde(rename = "nom", default)]
    pub name: String,
    pub points: Vec<[f32; 2]>,
}

impl Guide {
    /// Guide neuf à `n` points de fuite, placés pour être visibles dans le cadre `frame`.
    pub fn new(n: usize, frame: Rect) -> Guide {
        let (x0, y0, w, h) = (frame.min.x, frame.min.y, frame.width(), frame.height());
        let horizon = y0 + h * 0.42;
        let points = match n {
            1 => vec![[x0 + w * 0.5, horizon]],
            2 => vec![[x0 + w * 0.08, horizon], [x0 + w * 0.92, horizon]],
            _ => vec![[x0 + w * 0.08, horizon], [x0 + w * 0.92, horizon], [x0 + w * 0.5, y0 + h * 0.98]],
        };
        Guide {
            name: format!("Perspective {} point{}", n.min(3), if n > 1 { "s" } else { "" }),
            points,
        }
    }

    pub fn point(&self, i: usize) -> Pos2 {
        pos2(self.points[i][0], self.points[i][1])
    }

    /// Directions possibles d'un trait qui part de `s`.
    pub fn directions(&self, s: Pos2) -> Vec<Vec2> {
        let mut dirs: Vec<Vec2> = (0..self.points.len())
            .filter_map(|i| {
                let d = self.point(i) - s;
                (d.length() > 1e-3).then(|| d.normalized())
            })
            .collect();
        match self.points.len() {
            1 => {
                dirs.push(vec2(1.0, 0.0));
                dirs.push(vec2(0.0, 1.0));
            }
            2 => dirs.push(vec2(0.0, 1.0)),
            _ => {}
        }
        dirs
    }

    /// Direction du guide la plus proche du mouvement (dans un sens ou dans l'autre).
    pub fn best_direction(&self, s: Pos2, motion: Vec2) -> Option<Vec2> {
        let m = motion.normalized();
        self.directions(s)
            .into_iter()
            .max_by(|a, b| a.dot(m).abs().total_cmp(&b.dot(m).abs()))
    }

    /// Ligne d'horizon : un point et une direction.
    pub fn horizon(&self) -> Option<(Pos2, Vec2)> {
        match self.points.len() {
            0 => None,
            1 => Some((self.point(0), vec2(1.0, 0.0))),
            _ => {
                let d = self.point(1) - self.point(0);
                Some((self.point(0), if d.length() > 1e-3 { d.normalized() } else { vec2(1.0, 0.0) }))
            }
        }
    }
}

/// Projection de `p` sur la droite qui passe par `s` dans la direction `dir` (unitaire).
pub fn project(s: Pos2, dir: Vec2, p: Pos2) -> Pos2 {
    s + dir * (p - s).dot(dir)
}

/// Partie de la droite (ou du segment si `segment`) entre `a` et `a + d` qui tombe dans `r`.
pub fn clip_line(a: Pos2, d: Vec2, r: Rect, segment: bool) -> Option<(Pos2, Pos2)> {
    let (mut t0, mut t1) = if segment { (0.0f32, 1.0f32) } else { (f32::MIN, f32::MAX) };
    for (p, q) in [
        (-d.x, a.x - r.min.x),
        (d.x, r.max.x - a.x),
        (-d.y, a.y - r.min.y),
        (d.y, r.max.y - a.y),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

/// Réglages des guides (affichage) et gestes en cours.
#[derive(Default)]
pub struct GuideState {
    /// Guide affiché et utilisé (G).
    pub enabled: bool,
    /// Les traits suivent le guide (Maj+A).
    pub align: bool,
    /// Points de fuite non déplaçables (Maj+L).
    pub locked: bool,
    /// Lignes de référence (Maj+G).
    pub reference: bool,
    /// La gomme suit aussi le guide.
    pub eraser: bool,
    /// Lignes du curseur prolongées jusqu'au bord du dessin.
    pub full_cursor: bool,
    /// Guide courant dans la liste du projet.
    pub current: usize,
    /// Point de fuite en cours de déplacement.
    pub drag: Option<usize>,
    /// Trait aligné en cours : point de départ et direction (choisie après un petit mouvement).
    pub stroke: Option<(Pos2, Option<Vec2>)>,
}

const GUIDE: Color32 = Color32::from_rgba_premultiplied(230, 110, 30, 200);
const GRAB: f32 = 9.0;

impl App {
    /// Guide utilisé en ce moment (affiché et existant).
    pub(crate) fn active_guide(&self) -> Option<&Guide> {
        if !self.guides.enabled {
            return None;
        }
        self.doc.guides.get(self.guides.current)
    }

    pub(crate) fn add_guide(&mut self, n: usize) {
        self.record_step();
        let frame = self.doc.frame_rect();
        self.doc.guides.push(Guide::new(n, frame));
        self.guides.current = self.doc.guides.len() - 1;
        self.guides.enabled = true;
        self.status = "Guide ajouté : fais glisser ses points de fuite (Maj+L pour les verrouiller).".into();
    }

    pub(crate) fn delete_guide(&mut self) {
        let i = self.guides.current;
        if i < self.doc.guides.len() {
            self.record_step();
            self.doc.guides.remove(i);
            self.guides.current = i.saturating_sub(1);
        }
    }

    /// Point de fuite du guide sous le pointeur (position écran), s'il est déplaçable.
    pub(crate) fn guide_point_at(&self, screen: Pos2) -> Option<usize> {
        if self.guides.locked {
            return None;
        }
        let g = self.active_guide()?;
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        (0..g.points.len()).find(|&i| self.view.to_screen(rect, size, g.point(i)).distance(screen) <= GRAB)
    }

    /// Déplace un point de fuite (tout le glissé = une étape d'annulation).
    pub(crate) fn move_guide_point(&mut self, i: usize, p: Pos2) {
        let current = self.guides.current;
        if self.doc.guides.get(current).is_some_and(|g| i < g.points.len()) {
            self.note_change(egui::Id::new(("point_de_fuite", current, i)));
            self.doc.guides[current].points[i] = [p.x, p.y];
        }
    }

    /// Au début d'un trait : s'il doit suivre le guide, on retient son point de départ.
    pub(crate) fn begin_guided_stroke(&mut self, p: Pos2) {
        let follows = self.guides.align
            && self.active_guide().is_some()
            && (self.tool != Tool::Eraser || self.guides.eraser);
        self.guides.stroke = follows.then_some((p, None));
    }

    /// Point du trait aligné correspondant au pointeur en `p` ; `None` tant que la direction n'est
    /// pas encore choisie (le pointeur n'a presque pas bougé).
    pub(crate) fn guided_point(&mut self, p: Pos2) -> Option<Pos2> {
        let Some((s, dir)) = self.guides.stroke else { return Some(p) };
        let dir = match dir {
            Some(d) => d,
            None => {
                if (p - s).length() * self.view.zoom < 6.0 {
                    return None;
                }
                let d = self.active_guide().and_then(|g| g.best_direction(s, p - s))?;
                self.guides.stroke = Some((s, Some(d)));
                d
            }
        };
        Some(project(s, dir, p))
    }

    /// Guide par-dessus le dessin : horizon, points de fuite, lignes de référence et du curseur.
    pub(crate) fn paint_guides(&self, painter: &Painter, rect: Rect, hover: Option<Pos2>) {
        let Some(g) = self.active_guide() else { return };
        let size = self.canvas_size();
        let frame = Rect::from_min_size(Pos2::ZERO, size);
        let to_s = |p: Pos2| self.view.to_screen(rect, size, p);
        let line = |a: Pos2, b: Pos2, stroke: Stroke| painter.line_segment([to_s(a), to_s(b)], stroke);
        let thin = Stroke::new(1.0_f32, GUIDE.gamma_multiply(0.22));

        if self.guides.reference {
            for i in 0..g.points.len() {
                let vp = g.point(i);
                for d in fan(vp, frame, if i == 2 { 12 } else { 18 }) {
                    if let Some((a, b)) = clip_line(vp, d, frame, false) {
                        line(a, b, thin);
                    }
                }
            }
            let (w, h) = (size.x, size.y);
            if g.points.len() <= 2 {
                for k in 1..12 {
                    let x = w * k as f32 / 12.0;
                    line(pos2(x, 0.0), pos2(x, h), thin);
                }
            }
            if g.points.len() == 1 {
                for k in 1..8 {
                    let y = h * k as f32 / 8.0;
                    line(pos2(0.0, y), pos2(w, y), thin);
                }
            }
        }

        if let Some((a, d)) = g.horizon() {
            if let Some((p, q)) = clip_line(a, d, frame, false) {
                line(p, q, Stroke::new(1.5_f32, GUIDE));
            }
        }

        // Lignes qui partent du curseur (ou la droite du trait aligné en cours).
        let cursor = Stroke::new(1.0_f32, GUIDE.gamma_multiply(0.8));
        match self.guides.stroke {
            Some((s, Some(d))) => {
                if let Some((a, b)) = clip_line(s, d, frame, false) {
                    line(a, b, Stroke::new(1.5_f32, GUIDE));
                }
            }
            _ => {
                if let Some(c) = hover.filter(|c| frame.contains(*c)) {
                    for d in g.directions(c) {
                        let seg = if self.guides.full_cursor {
                            clip_line(c, d, frame, false)
                        } else {
                            // Jusqu'au point de fuite (ou au bord pour les verticales / horizontales).
                            let target = (0..g.points.len()).map(|i| g.point(i)).find(|v| ((*v - c).normalized() - d).length() < 1e-3);
                            match target {
                                Some(v) => clip_line(c, v - c, frame, true),
                                None => clip_line(c, d, frame, false),
                            }
                        };
                        if let Some((a, b)) = seg {
                            line(a, b, cursor);
                        }
                    }
                }
            }
        }

        for i in 0..g.points.len() {
            let s = to_s(g.point(i));
            let fill = if self.guides.locked { Color32::from_gray(120) } else { Color32::WHITE };
            painter.circle(s, 6.0, fill, Stroke::new(2.0_f32, GUIDE));
            painter.text(
                s + vec2(9.0, -9.0),
                Align2::LEFT_BOTTOM,
                format!("{}", i + 1),
                FontId::proportional(11.0),
                GUIDE,
            );
        }
        if self.guides.align {
            // Repère discret : les traits suivent le guide.
            painter.text(
                rect.left_top() + vec2(8.0, 6.0),
                Align2::LEFT_TOP,
                "Traits alignés sur le guide",
                FontId::proportional(12.0),
                GUIDE,
            );
        }
    }

    /// Réglages et liste des guides (panneau d'outils).
    pub(crate) fn guides_ui(&mut self, ui: &mut egui::Ui) {
        let key = |app: &App, action: &str| {
            let k = app.keymap.label(action);
            if k.is_empty() {
                String::new()
            } else {
                format!(" ({k})")
            }
        };
        let labels = [
            format!("Afficher et utiliser{}", key(self, "guide.activer")),
            format!("Aligner les traits{}", key(self, "guide.aligner")),
            format!("Verrouiller{}", key(self, "guide.verrouiller")),
            format!("Lignes de référence{}", key(self, "guide.lignes_reference")),
        ];
        let s = &mut self.guides;
        ui.checkbox(&mut s.enabled, &labels[0]);
        ui.add_enabled_ui(s.enabled, |ui| {
            ui.checkbox(&mut s.align, &labels[1]);
            ui.checkbox(&mut s.locked, &labels[2]);
            ui.checkbox(&mut s.reference, &labels[3]);
            ui.checkbox(&mut s.eraser, "Aligner aussi la gomme");
            ui.checkbox(&mut s.full_cursor, "Lignes du curseur jusqu'au bord");
        });
        let mut pick = None;
        for (i, g) in self.doc.guides.iter().enumerate() {
            if ui.selectable_label(i == self.guides.current, &g.name).clicked() {
                pick = Some(i);
            }
        }
        if let Some(i) = pick {
            self.guides.current = i;
            self.guides.enabled = true;
        }
        if self.doc.guides.is_empty() {
            ui.label(egui::RichText::new("Aucun guide dans ce projet.").small().weak());
        }
        ui.horizontal(|ui| {
            for (n, text) in [(1, "+ 1 point"), (2, "+ 2 points"), (3, "+ 3 points")] {
                if ui.button(text).clicked() {
                    self.add_guide(n);
                }
            }
        });
        let current = self.guides.current;
        if let Some(mut name) = self.doc.guides.get(current).map(|g| g.name.clone()) {
            let mut delete = false;
            ui.horizontal(|ui| {
                ui.label("Nom");
                let resp = ui.add(egui::TextEdit::singleline(&mut name).desired_width(110.0));
                if self.edited(&resp) {
                    self.doc.guides[current].name = name;
                }
                delete = ui.button("Supprimer").clicked();
            });
            if delete {
                self.delete_guide();
            }
        }
    }
}

/// Directions de `n` lignes de référence partant de `vp`, réparties sur l'angle sous lequel on voit
/// le dessin (tout le tour si le point est dedans).
fn fan(vp: Pos2, frame: Rect, n: usize) -> Vec<Vec2> {
    let n = n.max(2);
    if frame.contains(vp) {
        return (0..n * 2)
            .map(|k| Vec2::angled(k as f32 / (n * 2) as f32 * std::f32::consts::TAU))
            .collect();
    }
    let corners = [frame.left_top(), frame.right_top(), frame.right_bottom(), frame.left_bottom()];
    let base = (frame.center() - vp).angle();
    let rel = |p: Pos2| {
        let a = (p - vp).angle() - base;
        (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    };
    let lo = corners.iter().map(|&c| rel(c)).fold(f32::MAX, f32::min);
    let hi = corners.iter().map(|&c| rel(c)).fold(f32::MIN, f32::max);
    (0..=n)
        .map(|k| Vec2::angled(base + lo + (hi - lo) * k as f32 / n as f32))
        .collect()
}
