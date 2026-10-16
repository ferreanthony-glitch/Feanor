//! Roue chromatique (passe couleur) : un anneau des teintes, et dedans un carré ou un triangle
//! saturation / valeur. Géométrie pure (point ↔ teinte, saturation, valeur), testée, et les
//! maillages colorés que le peintre d'egui dessine.
//!
//! Repère de l'écran (y vers le bas). Teinte 0 (rouge) en haut, les teintes tournent dans le
//! sens des aiguilles d'une montre : jaune, vert, cyan, bleu, magenta.

use eframe::egui::{pos2, vec2, Color32, Mesh, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::couleur::hsv_to_rgb;

/// Ce qu'il y a dans l'anneau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum WheelShape {
    /// Saturation de gauche à droite, valeur de bas en haut.
    #[default]
    #[serde(rename = "carre")]
    Square,
    /// Sommets : la teinte pure (vers sa place sur l'anneau), le blanc, le noir.
    #[serde(rename = "triangle")]
    Triangle,
}

/// Où l'on a appuyé dans la roue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Ring,
    Inside,
}

/// Une roue posée à l'écran : centre, rayons extérieur et intérieur de l'anneau.
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub center: Pos2,
    pub outer: f32,
    pub inner: f32,
}

/// Épaisseur de l'anneau, en part du rayon extérieur.
const RING: f32 = 0.17;

fn dir(hue: f32) -> Vec2 {
    let a = hue.to_radians();
    vec2(a.sin(), -a.cos())
}

fn color(hsv: [f32; 3]) -> Color32 {
    let [r, g, b] = hsv_to_rgb(hsv);
    Color32::from_rgb(r, g, b)
}

impl Wheel {
    /// La plus grande roue qui tient dans `rect`.
    pub fn in_rect(rect: Rect) -> Wheel {
        let outer = (rect.width().min(rect.height()) * 0.5 - 1.0).max(4.0);
        Wheel { center: rect.center(), outer, inner: outer * (1.0 - RING) }
    }

    /// Teinte (0 à 360) dans la direction de `p`.
    pub fn hue_at(&self, p: Pos2) -> f32 {
        let d = p - self.center;
        if d.length_sq() < 1e-6 {
            return 0.0;
        }
        d.x.atan2(-d.y).to_degrees().rem_euclid(360.0)
    }

    /// Point de l'anneau, au milieu de son épaisseur, pour une teinte.
    #[cfg(test)]
    pub fn ring_point(&self, hue: f32) -> Pos2 {
        self.center + dir(hue) * (self.outer + self.inner) * 0.5
    }

    /// Le trait qui traverse l'anneau à une teinte (repère de la teinte choisie).
    pub fn ring_segment(&self, hue: f32) -> [Pos2; 2] {
        [self.center + dir(hue) * (self.inner - 1.0), self.center + dir(hue) * (self.outer + 1.0)]
    }

    /// Le carré inscrit dans l'anneau (un peu de marge).
    pub fn square(&self) -> Rect {
        Rect::from_center_size(self.center, Vec2::splat(self.inner * std::f32::consts::SQRT_2 * 0.92))
    }

    /// Saturation et valeur au point `p` du carré (bornées au carré).
    pub fn square_sv(&self, p: Pos2) -> (f32, f32) {
        let r = self.square();
        (((p.x - r.left()) / r.width()).clamp(0.0, 1.0), ((r.bottom() - p.y) / r.height()).clamp(0.0, 1.0))
    }

    /// Point du carré pour une saturation et une valeur.
    pub fn square_point(&self, s: f32, v: f32) -> Pos2 {
        let r = self.square();
        pos2(r.left() + s.clamp(0.0, 1.0) * r.width(), r.bottom() - v.clamp(0.0, 1.0) * r.height())
    }

    /// Sommets du triangle : teinte pure, blanc, noir. La teinte pointe vers sa place sur
    /// l'anneau ; le blanc suit à 120° dans le sens des teintes, le noir à 240°.
    pub fn triangle(&self, hue: f32) -> [Pos2; 3] {
        let r = self.inner * 0.96;
        [hue, hue + 120.0, hue + 240.0].map(|h| self.center + dir(h) * r)
    }

    /// Point du triangle : noir + v (blanc − noir) + v s (teinte − blanc). À valeur nulle, tout
    /// est au noir ; à valeur pleine, du blanc (s = 0) à la teinte pure (s = 1).
    pub fn triangle_point(&self, hue: f32, s: f32, v: f32) -> Pos2 {
        let [h, w, b] = self.triangle(hue);
        let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        b + (w - b) * v + (h - w) * (v * s)
    }

    /// Saturation et valeur au point `p` du triangle (bornées au triangle). Au noir, la
    /// saturation n'a pas de sens : elle vaut 0.
    pub fn triangle_sv(&self, hue: f32, p: Pos2) -> (f32, f32) {
        let [h, w, b] = self.triangle(hue);
        let (e1, e2, d) = (w - b, h - w, p - b);
        let det = e1.x * e2.y - e1.y * e2.x;
        if det.abs() < 1e-9 {
            return (0.0, 0.0);
        }
        let v = ((d.x * e2.y - d.y * e2.x) / det).clamp(0.0, 1.0);
        let t = ((e1.x * d.y - e1.y * d.x) / det).clamp(0.0, v);
        (if v > 1e-6 { t / v } else { 0.0 }, v)
    }

    /// Ce que vise un appui en `p` : l'anneau, l'intérieur (le carré, le triangle et ce qui les
    /// entoure jusqu'à l'anneau), ou rien (les coins hors de la roue).
    pub fn zone(&self, p: Pos2) -> Option<Zone> {
        let r = (p - self.center).length();
        if r > self.outer + 3.0 {
            None
        } else if r >= self.inner - 1.0 {
            Some(Zone::Ring)
        } else {
            Some(Zone::Inside)
        }
    }

    /// Saturation et valeur au point `p` de l'intérieur, selon la forme.
    pub fn sv_at(&self, shape: WheelShape, hue: f32, p: Pos2) -> (f32, f32) {
        match shape {
            WheelShape::Square => self.square_sv(p),
            WheelShape::Triangle => self.triangle_sv(hue, p),
        }
    }

    /// Point de l'intérieur pour une saturation et une valeur, selon la forme.
    pub fn sv_point(&self, shape: WheelShape, hue: f32, s: f32, v: f32) -> Pos2 {
        match shape {
            WheelShape::Square => self.square_point(s, v),
            WheelShape::Triangle => self.triangle_point(hue, s, v),
        }
    }

    /// L'anneau des teintes, en `segments` morceaux. Avec un multiple de 6, les teintes pures
    /// tombent sur des sommets : entre deux sommets, la couleur varie en ligne droite, comme le
    /// mélange de deux sommets voisins.
    pub fn ring_mesh(&self, segments: usize) -> Mesh {
        let mut mesh = Mesh::default();
        let n = segments.max(6);
        for k in 0..=n {
            let hue = k as f32 * 360.0 / n as f32;
            let c = color([hue, 1.0, 1.0]);
            mesh.colored_vertex(self.center + dir(hue) * self.inner, c);
            mesh.colored_vertex(self.center + dir(hue) * self.outer, c);
            if k > 0 {
                let i = (k * 2) as u32;
                mesh.add_triangle(i - 2, i - 1, i);
                mesh.add_triangle(i - 1, i + 1, i);
            }
        }
        mesh
    }

    /// Le carré saturation / valeur de la teinte `hue`, en n × n cases dont chaque sommet a sa
    /// couleur exacte.
    pub fn square_mesh(&self, hue: f32, n: usize) -> Mesh {
        let mut mesh = Mesh::default();
        let n = n.max(1);
        for j in 0..=n {
            for i in 0..=n {
                let (s, v) = (i as f32 / n as f32, 1.0 - j as f32 / n as f32);
                mesh.colored_vertex(self.square_point(s, v), color([hue, s, v]));
            }
        }
        let row = (n + 1) as u32;
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let a = j * row + i;
                mesh.add_triangle(a, a + 1, a + row);
                mesh.add_triangle(a + 1, a + row + 1, a + row);
            }
        }
        mesh
    }

    /// Le triangle de la teinte `hue`, découpé en n² petits triangles (rangée i : valeur i / n ;
    /// colonne j : v s = j / n), chaque sommet avec sa couleur exacte.
    pub fn triangle_mesh(&self, hue: f32, n: usize) -> Mesh {
        let mut mesh = Mesh::default();
        let n = n.max(1);
        for i in 0..=n {
            let v = i as f32 / n as f32;
            for j in 0..=i {
                let s = if i == 0 { 0.0 } else { j as f32 / i as f32 };
                mesh.colored_vertex(self.triangle_point(hue, s, v), color([hue, s, v]));
            }
        }
        let at = |i: usize, j: usize| (i * (i + 1) / 2 + j) as u32;
        for i in 0..n {
            for j in 0..=i {
                mesh.add_triangle(at(i, j), at(i + 1, j), at(i + 1, j + 1));
                if j < i {
                    mesh.add_triangle(at(i, j), at(i + 1, j + 1), at(i, j + 1));
                }
            }
        }
        mesh
    }
}
