//! Formes (5c) : géométrie des outils Ligne (droite, courbe, courbe en S), Rectangle, Ellipse et
//! Polyligne, sans interface.
//!
//! Storyboard Pro 27 a ces outils (LINE, RECTANGLE, ELLIPSE, POLYLINE ; la ligne a trois modes :
//! droite, courbe, courbe en S). Leur maniement exact n'a pas été vérifié : celui-ci est propre
//! à Pelure. La ligne courbe se trace comme une ligne droite, puis chaque glisser la courbe : la
//! courbe passe par le pointeur (à mi-chemin pour une courbe, aux deux tiers pour le second
//! glisser d'une courbe en S).

use std::f32::consts::TAU;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::vector::{Element, VPoint};

/// Mode de l'outil Ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineMode {
    Straight,
    /// Une courbure après le tracé.
    Curve,
    /// Deux courbures : la courbe peut faire un S.
    SCurve,
}

impl LineMode {
    /// Nombre de glissers qui courbent la ligne après son tracé.
    pub fn bends(self) -> u8 {
        match self {
            LineMode::Straight => 0,
            LineMode::Curve => 1,
            LineMode::SCurve => 2,
        }
    }
}

/// Fin de segment alignée par pas de 15° (Maj).
pub fn snap_angle(a: Pos2, b: Pos2) -> Pos2 {
    let d = b - a;
    if d.length() < 1e-6 {
        return b;
    }
    let step = 15f32.to_radians();
    a + Vec2::angled((d.angle() / step).round() * step) * d.length()
}

/// Coin opposé qui fait un carré (Maj).
pub fn square_corner(a: Pos2, b: Pos2) -> Pos2 {
    let d = b - a;
    let m = d.x.abs().max(d.y.abs());
    a + vec2(m.copysign(d.x), m.copysign(d.y))
}

/// Contour fermé d'un rectangle (le dernier point répète le premier).
pub fn rect_points(a: Pos2, b: Pos2) -> Vec<Pos2> {
    let r = Rect::from_two_pos(a, b);
    vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()]
}

/// Contour fermé de l'ellipse inscrite dans le rectangle `a`-`b`.
pub fn ellipse_points(a: Pos2, b: Pos2) -> Vec<Pos2> {
    let r = Rect::from_two_pos(a, b);
    let (c, rx, ry) = (r.center(), r.width() * 0.5, r.height() * 0.5);
    let n = 96;
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32 * TAU;
            pos2(c.x + rx * t.cos(), c.y + ry * t.sin())
        })
        .collect()
}

/// Point de la courbe de Bézier cubique `a`, `c1`, `c2`, `b` en `t`.
pub fn cubic_at(a: Pos2, c1: Pos2, c2: Pos2, b: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    pos2(
        k0 * a.x + k1 * c1.x + k2 * c2.x + k3 * b.x,
        k0 * a.y + k1 * c1.y + k2 * c2.y + k3 * b.y,
    )
}

/// La courbe en `n` segments.
pub fn cubic_points(a: Pos2, c1: Pos2, c2: Pos2, b: Pos2, n: usize) -> Vec<Pos2> {
    let n = n.max(1);
    (0..=n).map(|k| cubic_at(a, c1, c2, b, k as f32 / n as f32)).collect()
}

/// Courbe simple (un seul point de contrôle) qui passe par `p` à mi-chemin de `a` à `b`,
/// écrite en cubique : ses deux points de contrôle.
pub fn bend_through(a: Pos2, b: Pos2, p: Pos2) -> (Pos2, Pos2) {
    let c = pos2(2.0 * p.x - (a.x + b.x) * 0.5, 2.0 * p.y - (a.y + b.y) * 0.5);
    (a + (c - a) * (2.0 / 3.0), b + (c - b) * (2.0 / 3.0))
}

/// Second point de contrôle tel que la courbe passe par `p` aux deux tiers (le premier fixé).
pub fn second_bend_through(a: Pos2, c1: Pos2, b: Pos2, p: Pos2) -> Pos2 {
    // B(2/3) = (a + 6·c1 + 12·c2 + 8·b) / 27.
    pos2(
        (27.0 * p.x - a.x - 6.0 * c1.x - 8.0 * b.x) / 12.0,
        (27.0 * p.y - a.y - 6.0 * c1.y - 8.0 * b.y) / 12.0,
    )
}

/// Une forme tracée qui s'éloigne de `reach` pixels du dessin de son point de départ est-elle
/// assez grande pour être gardée (au moins 2 pixels d'écran au zoom `zoom`) ?
pub fn big_enough(reach: f32, zoom: f32) -> bool {
    reach * zoom >= 2.0
}

/// Le point `p` referme-t-il la polyligne (assez près de son premier point, à `tol` près) ?
pub fn closes(points: &[Pos2], p: Pos2, tol: f32) -> bool {
    points.len() >= 3 && points[0].distance(p) <= tol
}

/// L'élément vectoriel d'une forme : zone remplie (fermée, au moins 3 points) ou trait (ouvert
/// ou fermé). Rien pour une forme sans étendue.
pub fn element(points: &[Pos2], closed: bool, filled: bool, width: f32, color: [u8; 3]) -> Option<Element> {
    let mut pts: Vec<Pos2> = points.to_vec();
    // Points doublés (double-clic, clic au même endroit) retirés.
    pts.dedup_by(|b, a| a.distance(*b) < 0.01);
    if closed && pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) < 0.01 {
        pts.pop();
    }
    let extent = Rect::from_points(&pts);
    if pts.len() < 2 || extent.width().max(extent.height()) < 0.5 {
        return None;
    }
    if filled && closed && pts.len() >= 3 {
        return Some(Element::Fill {
            contours: vec![pts.iter().map(|p| [p.x, p.y]).collect()],
            color,
            opacity: 1.0,
        });
    }
    if closed && pts.len() >= 3 {
        pts.push(pts[0]);
    }
    let r = width * 0.5;
    Some(Element::Stroke {
        points: pts.iter().map(|p| VPoint { x: p.x, y: p.y, r }).collect(),
        color,
        opacity: 1.0,
        hardness: 0.9,
        brush: None,
        trace: Default::default(),
    })
}

/// Forme en plusieurs gestes.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeEdit {
    /// Ligne tracée, que l'on courbe : `done` glissers faits sur `mode.bends()`.
    Curve {
        a: Pos2,
        b: Pos2,
        c1: Pos2,
        c2: Pos2,
        mode: LineMode,
        done: u8,
        /// Un glisser de courbure est en cours.
        bending: bool,
    },
    /// Polyligne : un point par clic ; `last_press` (instant, point) repère le double-clic.
    Polyline { points: Vec<Pos2>, last_press: Option<(f64, Pos2)> },
}

impl ShapeEdit {
    /// Ligne droite de `a` à `b`, prête à être courbée.
    pub fn curve(a: Pos2, b: Pos2, mode: LineMode) -> ShapeEdit {
        ShapeEdit::Curve {
            a,
            b,
            c1: a + (b - a) / 3.0,
            c2: a + (b - a) * (2.0 / 3.0),
            mode,
            done: 0,
            bending: false,
        }
    }

    /// Pendant un glisser de courbure : la courbe passe par `p`.
    pub fn bend_to(&mut self, p: Pos2) {
        if let ShapeEdit::Curve { a, b, c1, c2, done, .. } = self {
            if *done == 0 {
                (*c1, *c2) = bend_through(*a, *b, p);
            } else {
                *c2 = second_bend_through(*a, *c1, *b, p);
            }
        }
    }

    /// Fin d'un glisser de courbure ; vrai si la ligne a reçu toutes ses courbures.
    pub fn end_bend(&mut self) -> bool {
        match self {
            ShapeEdit::Curve { mode, done, bending, .. } => {
                *bending = false;
                *done += 1;
                *done >= mode.bends()
            }
            ShapeEdit::Polyline { .. } => false,
        }
    }

    /// Points de la forme telle qu'elle est (la courbe en 64 segments).
    pub fn points(&self) -> Vec<Pos2> {
        match self {
            ShapeEdit::Curve { a, b, c1, c2, .. } => cubic_points(*a, *c1, *c2, *b, 64),
            ShapeEdit::Polyline { points, .. } => points.clone(),
        }
    }
}
