//! Enveloppe de Bézier (Anthony, 2026-10-18 : « maintenir Ctrl permet de saisir une poignée de
//! Bézier des coins pour transformer la forme » ; choix B : le dessin courant est déformé, comme
//! avec l'outil Perspective). Sans interface (voir `envelopetools.rs`) : quatre coins, et sur
//! chaque côté deux poignées, celles d'une courbe de Bézier cubique ; l'intérieur suit les quatre
//! côtés (surface de Coons). Appliquée à une image (maillage de triangles, échantillonnage
//! bilinéaire) ou aux points d'éléments vectoriels (rendus plus denses pour pouvoir se courber).

use eframe::egui::{pos2, Color32, Pos2, Rect, Vec2};

use crate::persp;
use crate::pixsel::PixelMask;
use crate::raster::{Raster, TILE};
use crate::vector::{Element, VPoint, VectorDrawing};

/// Coins (haut-gauche, haut-droite, bas-droite, bas-gauche) et poignées : sur le côté `k` (du coin
/// `k` au coin `k + 1`), `handles[2k]` près du coin `k`, `handles[2k + 1]` près du coin `k + 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    pub corners: [Pos2; 4],
    pub handles: [Pos2; 8],
}

fn bezier(p: [Pos2; 4], t: f32) -> Pos2 {
    let s = 1.0 - t;
    let v = p[0].to_vec2() * (s * s * s) + p[1].to_vec2() * (3.0 * s * s * t) + p[2].to_vec2() * (3.0 * s * t * t) + p[3].to_vec2() * (t * t * t);
    v.to_pos2()
}

impl Envelope {
    /// L'enveloppe qui ne déforme rien : le rectangle `r`, poignées au tiers de chaque côté.
    pub fn identity(r: Rect) -> Envelope {
        let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        let mut handles = [Pos2::ZERO; 8];
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            handles[2 * k] = a + (b - a) / 3.0;
            handles[2 * k + 1] = a + (b - a) * (2.0 / 3.0);
        }
        Envelope { corners, handles }
    }

    /// Les points de contrôle du côté `k`, du coin `k` au coin `k + 1`.
    pub fn edge(&self, k: usize) -> [Pos2; 4] {
        [self.corners[k], self.handles[2 * k], self.handles[2 * k + 1], self.corners[(k + 1) % 4]]
    }

    /// Le point (`u`, `v`) de l'enveloppe (0 à 1 de gauche à droite, de haut en bas).
    pub fn point(&self, u: f32, v: f32) -> Pos2 {
        let top = bezier(self.edge(0), u).to_vec2();
        let right = bezier(self.edge(1), v).to_vec2();
        let bottom = bezier(self.edge(2), 1.0 - u).to_vec2();
        let left = bezier(self.edge(3), 1.0 - v).to_vec2();
        let [tl, tr, br, bl] = self.corners.map(|c| c.to_vec2());
        let corners = tl * ((1.0 - u) * (1.0 - v)) + tr * (u * (1.0 - v)) + br * (u * v) + bl * ((1.0 - u) * v);
        (top * (1.0 - v) + bottom * v + left * (1.0 - u) + right * u - corners).to_pos2()
    }

    /// Le point de l'enveloppe qui correspond au point `p` du rectangle `src`.
    pub fn map(&self, src: Rect, p: Pos2) -> Pos2 {
        let (w, h) = (src.width().max(1e-6), src.height().max(1e-6));
        self.point((p.x - src.min.x) / w, (p.y - src.min.y) / h)
    }

    /// Le coin `k` déplacé de `d`, avec ses deux poignées.
    pub fn move_corner(&mut self, k: usize, d: Vec2) {
        self.corners[k] += d;
        self.handles[2 * k] += d;
        self.handles[2 * ((k + 3) % 4) + 1] += d;
    }

    /// Toute l'enveloppe déplacée de `d`.
    pub fn translate(&mut self, d: Vec2) {
        for c in &mut self.corners {
            *c += d;
        }
        for h in &mut self.handles {
            *h += d;
        }
    }

    /// Surface approchée (polygone des côtés échantillonnés).
    fn area(&self) -> f32 {
        let pts: Vec<Pos2> = (0..4).flat_map(|k| (0..8).map(move |i| (k, i as f32 / 8.0))).map(|(k, t)| bezier(self.edge(k), t)).collect();
        let n = pts.len();
        0.5 * (0..n).map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y).sum::<f32>().abs()
    }
}

/// Les pixels de la feuille (`width` × `height`) que couvre l'enveloppe `env`, chacun avec le point
/// du rectangle `src` qui lui correspond : chaque case d'un maillage est faite de deux triangles.
fn rasterize(src: Rect, env: &Envelope, width: usize, height: usize, mut put: impl FnMut(usize, usize, Pos2)) {
    // Des cases d'environ 12 pixels du rectangle d'origine.
    let n = ((src.width().max(src.height()) / 12.0).ceil() as usize).clamp(8, 96);
    let (w, h) = (width as f32, height as f32);
    let grid: Vec<Vec<(Pos2, Pos2)>> = (0..=n)
        .map(|j| {
            (0..=n)
                .map(|i| {
                    let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                    (pos2(src.min.x + u * src.width(), src.min.y + v * src.height()), env.point(u, v))
                })
                .collect()
        })
        .collect();
    let mut triangle = |a: (Pos2, Pos2), b: (Pos2, Pos2), c: (Pos2, Pos2)| {
        let (pa, pb, pc) = (a.1, b.1, c.1);
        let den = (pb.y - pc.y) * (pa.x - pc.x) + (pc.x - pb.x) * (pa.y - pc.y);
        if den.abs() < 1e-6 {
            return;
        }
        let bx0 = pa.x.min(pb.x).min(pc.x).floor().clamp(0.0, w) as usize;
        let bx1 = pa.x.max(pb.x).max(pc.x).ceil().clamp(0.0, w) as usize;
        let by0 = pa.y.min(pb.y).min(pc.y).floor().clamp(0.0, h) as usize;
        let by1 = pa.y.max(pb.y).max(pc.y).ceil().clamp(0.0, h) as usize;
        for y in by0..by1 {
            for x in bx0..bx1 {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let l1 = ((pb.y - pc.y) * (p.x - pc.x) + (pc.x - pb.x) * (p.y - pc.y)) / den;
                let l2 = ((pc.y - pa.y) * (p.x - pc.x) + (pa.x - pc.x) * (p.y - pc.y)) / den;
                let l3 = 1.0 - l1 - l2;
                const E: f32 = -1e-4;
                if l1 < E || l2 < E || l3 < E {
                    continue;
                }
                put(x, y, (a.0.to_vec2() * l1 + b.0.to_vec2() * l2 + c.0.to_vec2() * l3).to_pos2());
            }
        }
    };
    for j in 0..n {
        for i in 0..n {
            let (p00, p10, p01, p11) = (grid[j][i], grid[j][i + 1], grid[j + 1][i], grid[j + 1][i + 1]);
            triangle(p00, p10, p11);
            triangle(p00, p11, p01);
        }
    }
}

fn put_pixel(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

/// Une couleur prémultipliée multipliée par `k` (0 à 1).
fn scaled(c: Color32, k: f32) -> Color32 {
    let m = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(m(c.r()), m(c.g()), m(c.b()), m(c.a()))
}

/// L'image `original` dont le rectangle `src` est posé dans l'enveloppe `env` : le reste de l'image
/// ne bouge pas, le rectangle d'origine est vidé.
pub fn warp_raster(original: &Raster, src: Rect, env: &Envelope) -> Raster {
    let mut out = original.clone();
    let (x0, y0) = (src.min.x.floor().max(0.0) as usize, src.min.y.floor().max(0.0) as usize);
    let (x1, y1) = ((src.max.x.ceil().max(0.0) as usize).min(out.width), (src.max.y.ceil().max(0.0) as usize).min(out.height));
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = out.tile_index_of(x, y);
            if out.tiles[idx].is_some() {
                out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = Color32::TRANSPARENT;
            }
        }
    }
    let (w, h) = (out.width, out.height);
    rasterize(src, env, w, h, |x, y, s| {
        let c = persp::sample(original, s.x, s.y);
        if c.a() > 0 {
            put_pixel(&mut out, x, y, c);
        }
    });
    out
}

/// Seulement la zone choisie `mask` de l'image `original` (Anthony, 2026-10-18 : l'enveloppe sur
/// une zone bitmap choisie) : elle quitte sa place (en proportion de sa couverture : ses bords
/// adoucis aussi) et se pose, déformée, par-dessus ce qui reste.
pub fn warp_raster_masked(original: &Raster, mask: &PixelMask, env: &Envelope) -> Raster {
    let src = mask.bounds();
    let mut out = original.clone();
    for y in mask.y0..(mask.y0 + mask.h).min(out.height) {
        for x in mask.x0..(mask.x0 + mask.w).min(out.width) {
            let m = mask.at(x, y);
            let idx = out.tile_index_of(x, y);
            if m > 0.0 && out.tiles[idx].is_some() {
                let c = out.get(x, y);
                put_pixel(&mut out, x, y, scaled(c, 1.0 - m));
            }
        }
    }
    let (w, h) = (out.width, out.height);
    rasterize(src, env, w, h, |x, y, s| {
        if s.x < 0.0 || s.y < 0.0 {
            return;
        }
        let m = mask.at(s.x as usize, s.y as usize);
        if m <= 0.0 {
            return;
        }
        let c = scaled(persp::sample(original, s.x, s.y), m);
        if c.a() > 0 {
            let under = out.get(x, y);
            put_pixel(&mut out, x, y, crate::render::over(under, c, 255, None));
        }
    });
    out
}

/// Une suite de points rendue plus dense (au plus `step` entre deux), pour qu'elle puisse se courber.
fn densify_points(points: &[[f32; 2]], step: f32, closed: bool) -> Vec<[f32; 2]> {
    let n = points.len();
    let mut out = Vec::with_capacity(n * 2);
    let last = if closed { n } else { n.saturating_sub(1) };
    for i in 0..last {
        let (a, b) = (points[i], points[(i + 1) % n]);
        out.push(a);
        let d = ((b[0] - a[0]).hypot(b[1] - a[1]) / step).ceil() as usize;
        for k in 1..d {
            let t = k as f32 / d as f32;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    if !closed {
        if let Some(&p) = points.last() {
            out.push(p);
        }
    }
    out
}

/// Le dessin vectoriel `original` dont les éléments `items` (tous si `None`), compris dans `src`,
/// sont posés dans l'enveloppe `env`.
pub fn warp_drawing(original: &VectorDrawing, items: Option<&[usize]>, src: Rect, env: &Envelope) -> VectorDrawing {
    let k = (env.area() / (src.width() * src.height()).max(1e-6)).sqrt();
    // Points au plus à 1/48 du rectangle l'un de l'autre : les côtés courbés restent lisses.
    let step = (src.width().max(src.height()) / 48.0).max(1.0);
    let map = |p: Pos2| env.map(src, p);
    let mut out = original.clone();
    for (i, e) in out.elements.iter_mut().enumerate() {
        if !items.is_none_or(|list| list.contains(&i)) {
            continue;
        }
        let dense = match e {
            Element::Stroke { points, color, opacity, hardness, brush, trace } => {
                let mut pts: Vec<VPoint> = Vec::with_capacity(points.len() * 2);
                for (j, p) in points.iter().enumerate() {
                    if j > 0 {
                        let a = points[j - 1];
                        let d = (a.pos().distance(p.pos()) / step).ceil() as usize;
                        for s in 1..d {
                            let t = s as f32 / d as f32;
                            pts.push(VPoint { x: a.x + (p.x - a.x) * t, y: a.y + (p.y - a.y) * t, r: a.r + (p.r - a.r) * t });
                        }
                    }
                    pts.push(*p);
                }
                Element::Stroke { points: pts, color: *color, opacity: *opacity, hardness: *hardness, brush: brush.clone(), trace: trace.without_samples() }
            }
            Element::Fill { contours, color, opacity } => Element::Fill {
                contours: contours.iter().map(|c| densify_points(c, step, true)).collect(),
                color: *color,
                opacity: *opacity,
            },
            Element::Text { .. } => e.clone(),
        };
        *e = persp::warp_element_with(&dense, &map, k);
    }
    out
}
