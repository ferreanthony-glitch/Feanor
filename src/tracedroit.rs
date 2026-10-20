//! Tracé droit (Anthony, 2026-10-20 : « SHIFT tenu lors d'un tracé pour un tracé droit »).
//!
//! Pendant un trait de pinceau, de crayon ou de gomme (vectoriels ou bitmap), Maj tenue :
//! - le trait s'arrête là où il était (`Droit::last`, le dernier point donné au moteur) et un
//!   segment droit le relie à la pointe : on le voit en aperçu (une ligne à la largeur du pinceau,
//!   `paint_droit`), le moteur n'a rien à refaire pendant le geste ;
//! - Maj+Alt : l'angle du segment est aligné par pas de 15° (comme la Maj des formes, `shapes.rs`) ;
//! - Maj relâchée (ou bouton relâché) : le segment est posé en une fois, par le moteur, comme un trait
//!   rapide et régulier ; le tracé libre reprend de son bout. Un seul geste = une seule étape
//!   d'annulation (le trait entier).
//! Maj tenue dès l'appui : le segment part du début du trait. Un trait aligné sur un guide
//! (`guides.rs`) est déjà droit : Maj n'y change rien.

use eframe::egui::{self, Color32, Modifiers, Pos2, Stroke};

use crate::app::{App, Tool};

/// Vitesse à laquelle le segment est posé (pixels par seconde) : celle d'une main rapide, pour que
/// les réglages qui suivent la vitesse donnent un résultat plausible.
const VITESSE: f32 = 900.0;
/// Au plus autant de points par segment.
const MAX_POINTS: usize = 3000;

/// L'état du tracé droit d'un trait en cours.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Droit {
    /// Dernier point donné au moteur (point du dessin du calque) : le départ du segment droit.
    pub last: Option<Pos2>,
    /// Départ, bout et appui du segment droit en cours (Maj tenue).
    pub anchor: Option<Pos2>,
    pub target: Option<Pos2>,
    pub pressure: f32,
}

impl Droit {
    /// Y a-t-il un segment droit en attente ?
    pub fn pending(&self) -> bool {
        self.anchor.is_some() && self.target.is_some()
    }
}

impl App {
    /// Un trait vient de commencer en `p` (point du dessin du calque).
    pub(crate) fn droit_start(&mut self, p: Pos2) {
        self.droit = Droit { last: Some(p), ..Droit::default() };
    }

    /// Le pointeur est en `p` (point du dessin) pendant un trait, avec ces touches : tracé libre, ou
    /// segment droit si Maj est tenue. `erase` : la gomme vectorielle.
    pub(crate) fn stroke_point(&mut self, p: Pos2, pressure: f32, mods: &Modifiers, erase: bool) {
        if self.droit_wanted(mods) {
            self.droit_aim(p, pressure, mods);
            return;
        }
        // (Maj relâchée : le segment attendu est posé avant la suite.)
        self.droit_flush();
        self.feed_point(p, pressure, erase);
    }

    /// Maj tenue pendant un trait qui n'est pas déjà aligné sur un guide : le tracé devient droit.
    fn droit_wanted(&self, mods: &Modifiers) -> bool {
        mods.shift && self.guides.stroke.is_none() && (self.stroke.is_some() || self.verase.is_some())
    }

    /// Vise `p` avec le segment droit (aligné par 15° avec Alt). Le départ est le dernier point posé.
    fn droit_aim(&mut self, p: Pos2, pressure: f32, mods: &Modifiers) {
        let Some(a) = self.droit.anchor.or(self.droit.last) else {
            // (Rien posé encore : le premier point est le départ.)
            self.feed_point(p, pressure, self.verase.is_some());
            return;
        };
        self.droit.anchor = Some(a);
        self.droit.target = Some(if mods.alt { crate::shapes::snap_angle(a, p) } else { p });
        self.droit.pressure = pressure;
    }

    /// Un point de plus au moteur (trait ou gomme vectorielle).
    fn feed_point(&mut self, p: Pos2, pressure: f32, erase: bool) {
        if erase {
            self.erase_to(p, pressure);
        } else {
            self.stroke_to(p, pressure);
        }
        self.droit.last = Some(p);
    }

    /// Une fois par image, pendant un trait : Maj tenue sans que la souris bouge, le segment suit
    /// quand même la pointe (`hover` : sa place sur la feuille) ; Maj relâchée, le segment est posé.
    pub(crate) fn droit_follow(&mut self, mods: &Modifiers, primary_down: bool, hover: Option<Pos2>, pressure: f32) {
        if !primary_down {
            return;
        }
        if self.droit_wanted(mods) {
            if let Some(stage) = hover {
                let p = self.to_layer(stage);
                self.droit_aim(p, pressure, mods);
            }
        } else if self.droit.pending() {
            self.droit_flush();
        }
    }

    /// Pose le segment droit en attente, s'il y en a un : de son départ à son bout, en points réguliers
    /// (un tous les quarts de la taille du pinceau), le dernier au bout exact.
    pub(crate) fn droit_flush(&mut self) {
        let d = std::mem::take(&mut self.droit);
        let (Some(a), Some(b)) = (d.anchor, d.target) else {
            self.droit.last = d.last;
            return;
        };
        if self.verase.is_some() {
            // La gomme vectorielle suit elle-même le segment parcouru.
            self.erase_to(b, d.pressure);
        } else if self.stroke.is_some() {
            let size = self.current_brush().map_or(4.0, |s| s.size);
            let step = (size * 0.25).clamp(2.0, 12.0);
            let len = a.distance(b);
            let n = ((len / step).ceil() as usize).clamp(1, MAX_POINTS);
            let (t0, span) = (self.input_time, (len / VITESSE).max(0.02) as f64);
            for k in 1..=n {
                let t = k as f32 / n as f32;
                self.input_time = t0 + span * t as f64;
                self.stroke_to(a + (b - a) * t, d.pressure);
            }
            self.input_time = t0;
        }
        self.droit.last = Some(b);
    }

    /// L'aperçu du segment droit en attente : une ligne à la largeur du pinceau, dans sa couleur.
    pub(crate) fn paint_droit(&self, painter: &egui::Painter) {
        let (Some(a), Some(b)) = (self.droit.anchor, self.droit.target) else { return };
        let (a, b) = (self.layer_to_screen(a), self.layer_to_screen(b));
        let width = (self.current_brush().map_or(4.0, |s| s.size) * self.layer_zoom()).max(1.0);
        let color = if self.tool == Tool::Eraser {
            Color32::from_rgba_unmultiplied(160, 160, 160, 150)
        } else {
            let [r, g, bl] = self.color;
            let alpha = self.current_brush().map_or(1.0, |s| s.opacity).clamp(0.35, 1.0);
            Color32::from_rgba_unmultiplied(r, g, bl, (alpha * 220.0) as u8)
        };
        painter.line_segment([a, b], Stroke::new(width, color));
        painter.circle_filled(a, width * 0.5, color);
        painter.circle_filled(b, width * 0.5, color);
    }
}
