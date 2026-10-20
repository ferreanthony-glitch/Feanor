//! Manipulateurs de l'Espace 3D : la géométrie des poignées (déplacer, tourner, mettre à l'échelle),
//! leur détection à la souris, et l'effet d'un geste sur les placements des cartes et de la caméra choisies. Pas
//! d'interface ici : l'application trace les poignées et transmet les rayons de la souris.
//!
//! Poignées (conventions des logiciels 3D courants, à vérifier : couleurs rouge/vert/bleu pour X/Y/Z,
//! flèches pour déplacer, cercles pour tourner, cubes pour l'échelle, carrés pour déplacer dans un
//! plan) :
//! - déplacer : trois flèches (axes) et trois carrés (plans) ;
//! - tourner : trois cercles ;
//! - échelle : trois cubes (axes) et un cube central (uniforme).

use crate::espace3d::{SceneCam, Xf};
use crate::espace3dmath::{self as m, V3, View};

/// Numéro réservé à la caméra du plan dans une sélection.
pub const CAMERA_ID: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Choisir seulement.
    #[default]
    Select,
    Move,
    Rotate,
    Scale,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Select, Mode::Move, Mode::Rotate, Mode::Scale];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Select => "Choisir",
            Mode::Move => "Déplacer",
            Mode::Rotate => "Tourner",
            Mode::Scale => "Mettre à l'échelle",
        }
    }

    pub fn action(self) -> &'static str {
        match self {
            Mode::Select => "espace3d.outil_choisir",
            Mode::Move => "espace3d.outil_deplacer",
            Mode::Rotate => "espace3d.outil_tourner",
            Mode::Scale => "espace3d.outil_echelle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Déplacer ou mettre à l'échelle le long de l'axe `k` ; tourner autour de l'axe `k` (`Ring`).
    Axis(usize),
    /// Déplacer dans le plan perpendiculaire à l'axe `k`.
    Plane(usize),
    Ring(usize),
    /// Échelle uniforme.
    Center,
}

/// Un manipulateur posé dans le monde.
#[derive(Clone, Copy, Debug)]
pub struct Gizmo {
    pub origin: V3,
    pub axes: [V3; 3],
    /// Longueur des flèches, en mètres (à chaque image : proportionnelle à la distance de l'œil, pour
    /// garder la même taille à l'écran).
    pub len: f32,
    pub mode: Mode,
}

impl Gizmo {
    /// Taille voulue : une fraction de la distance de l'œil à l'origine.
    pub fn size_for(view: &View, origin: V3) -> f32 {
        (m::length(m::sub(origin, view.eye)) * 0.16).max(0.05)
    }

    /// Les quatre coins du carré de la poignée « plan » `k` (perpendiculaire à l'axe `k`).
    pub fn plane_quad(&self, k: usize) -> [V3; 4] {
        let (a, b) = (self.axes[(k + 1) % 3], self.axes[(k + 2) % 3]);
        let (lo, hi) = (self.len * 0.28, self.len * 0.52);
        let at = |u: f32, v: f32| m::add(self.origin, m::add(m::scale(a, u), m::scale(b, v)));
        [at(lo, lo), at(hi, lo), at(hi, hi), at(lo, hi)]
    }

    /// Points du cercle de la poignée « tourner » `k`.
    pub fn ring_points(&self, k: usize, n: usize) -> Vec<V3> {
        let (a, b) = (self.axes[(k + 1) % 3], self.axes[(k + 2) % 3]);
        (0..=n)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / n as f32;
                m::add(self.origin, m::scale(m::add(m::scale(a, t.cos()), m::scale(b, t.sin())), self.len))
            })
            .collect()
    }

    pub fn axis_tip(&self, k: usize) -> V3 {
        m::add(self.origin, m::scale(self.axes[k], self.len))
    }

    /// La poignée sous le pixel `px` (tolérance `tol` pixels), la plus proche. `center` : le point de
    /// l'image où l'œil regarde.
    pub fn hit(&self, view: &View, center: [f32; 2], px: [f32; 2], tol: f32) -> Option<Handle> {
        let proj = |p: V3| view.project(p, center);
        let dist_seg = |a: [f32; 2], b: [f32; 2]| {
            let (d, w) = ([b[0] - a[0], b[1] - a[1]], [px[0] - a[0], px[1] - a[1]]);
            let l2 = d[0] * d[0] + d[1] * d[1];
            let t = if l2 > 0.0 { ((w[0] * d[0] + w[1] * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
            ((px[0] - a[0] - d[0] * t).powi(2) + (px[1] - a[1] - d[1] * t).powi(2)).sqrt()
        };
        let mut best: Option<(f32, Handle)> = None;
        let mut offer = |d: f32, h: Handle, limit: f32| {
            if d <= limit && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, h));
            }
        };
        let o = proj(self.origin)?;
        match self.mode {
            Mode::Select => return None,
            Mode::Move | Mode::Scale => {
                for k in 0..3 {
                    if let Some(t) = proj(self.axis_tip(k)) {
                        offer(dist_seg(o, t), Handle::Axis(k), tol);
                    }
                }
                if self.mode == Mode::Move {
                    for k in 0..3 {
                        let q = self.plane_quad(k).map(proj);
                        if let [Some(a), Some(b), Some(c), Some(d)] = q {
                            if point_in_quad(px, [a, b, c, d]) {
                                // Un carré gagne sur les flèches voisines.
                                offer(0.0, Handle::Plane(k), tol);
                            }
                        }
                    }
                } else {
                    // (Le cube du centre l'emporte sur les cubes d'axe qui partent de lui.)
                    offer(((px[0] - o[0]).powi(2) + (px[1] - o[1]).powi(2)).sqrt() - 4.0, Handle::Center, tol);
                }
            }
            Mode::Rotate => {
                for k in 0..3 {
                    let pts: Vec<Option<[f32; 2]>> = self.ring_points(k, 48).into_iter().map(proj).collect();
                    for w in pts.windows(2) {
                        if let [Some(a), Some(b)] = w {
                            offer(dist_seg(*a, *b), Handle::Ring(k), tol);
                        }
                    }
                }
            }
        }
        best.map(|b| b.1)
    }
}

fn point_in_quad(p: [f32; 2], q: [[f32; 2]; 4]) -> bool {
    let side = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    let s: [f32; 4] = std::array::from_fn(|i| side(q[i], q[(i + 1) % 4]));
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

// ------------------------------------------------------------------- effet d'un geste

/// Les placements d'origine des cartes et de la caméra choisies.
pub type Originals = Vec<(u64, Xf)>;

/// Tous déplacés de `delta`.
pub fn move_xfs(originals: &[(u64, Xf)], delta: V3) -> Originals {
    originals.iter().map(|(id, xf)| (*id, Xf { pos: m::add(xf.pos, delta), ..*xf })).collect()
}

/// Tous tournés de `deg` degrés autour de l'axe `axis` (du monde) qui passe par `origin` : les
/// positions tournent autour du pivot, chaque carte tourne sur elle-même du même angle.
pub fn rotate_xfs(originals: &[(u64, Xf)], origin: V3, axis: V3, deg: f32) -> Originals {
    let r = m::rot_axis(axis, deg);
    originals
        .iter()
        .map(|(id, xf)| {
            let pos = m::add(origin, m::dir(&r, m::sub(xf.pos, origin)));
            let rot = m::euler_of(&m::mul(&r, &m::rotation(xf.rot)));
            (*id, Xf { pos, rot, ..*xf })
        })
        .collect()
}

/// Mis à l'échelle : une carte seule sur son axe local `axis`, ou, `axis` = rien, tous de `factor`
/// autour de `origin`.
pub fn scale_xfs(originals: &[(u64, Xf)], origin: V3, axis: Option<usize>, factor: f32) -> Originals {
    originals
        .iter()
        .map(|(id, xf)| match axis {
            Some(k) => {
                let mut size = xf.size;
                size[k] *= factor;
                (*id, Xf { size, ..*xf })
            }
            None => (*id, Xf { pos: m::add(origin, m::scale(m::sub(xf.pos, origin), factor)), size: m::scale(xf.size, factor), ..*xf }),
        })
        .collect()
}

/// Le centre de la boîte des positions (pivot d'une sélection).
pub fn pivot_of(originals: &[(u64, Xf)]) -> V3 {
    m::bounds_of(originals.iter().map(|(_, xf)| xf.pos)).map_or([0.0; 3], |(lo, hi)| m::scale(m::add(lo, hi), 0.5))
}

/// Les axes du manipulateur : ceux du monde, ou ceux de l'objet (`local`, un seul objet).
pub fn axes_for(local: bool, xf: &Xf) -> [V3; 3] {
    if local {
        m::axes_of(&m::rotation(xf.rot))
    } else {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    }
}

// ----------------------------------------------------------------------------- la caméra

/// La caméra du plan vue comme une carte : sa rotation est (bascule, −panoramique, −rotation) dans
/// l'ordre des cartes (lacet, tangage, roulis), si bien que les mêmes poignées la manient.
pub fn cam_xf(cam: &SceneCam) -> Xf {
    Xf { pos: cam.pos, rot: [cam.tilt, -cam.pan, -cam.roll], size: [1.0; 3] }
}

/// La caméra mise au placement `xf` (la focale est gardée).
pub fn cam_from_xf(base: &SceneCam, xf: &Xf) -> SceneCam {
    SceneCam { pos: xf.pos, tilt: xf.rot[0], pan: -xf.rot[1], roll: -xf.rot[2], focal: base.focal }.sanitized()
}
