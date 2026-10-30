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
use crate::vector::{self, Element, VectorDrawing};

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

/// Les deux feuilles d'un dessin bitmap dont le rectangle `src` se déforme : ce qui reste (le
/// dessin sans le rectangle) et ce qui bouge (le rectangle seul).
pub fn split_rect(original: &Raster, src: Rect) -> (Raster, Raster) {
    let mut base = original.clone();
    let mut sheet = Raster::new(original.width, original.height);
    let (x0, y0) = (src.min.x.floor().max(0.0) as usize, src.min.y.floor().max(0.0) as usize);
    let (x1, y1) = ((src.max.x.ceil().max(0.0) as usize).min(original.width), (src.max.y.ceil().max(0.0) as usize).min(original.height));
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = original.tile_index_of(x, y);
            if original.tiles[idx].is_some() {
                let c = original.get(x, y);
                base.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = Color32::TRANSPARENT;
                if c.a() > 0 {
                    put_pixel(&mut sheet, x, y, c);
                }
            }
        }
    }
    (base, sheet)
}

/// Les deux feuilles d'un dessin bitmap dont la zone choisie `mask` se déforme : ce qui reste (en
/// proportion inverse de la couverture) et ce qui bouge (en proportion de la couverture).
pub fn split_masked(original: &Raster, mask: &PixelMask) -> (Raster, Raster) {
    let mut base = original.clone();
    let mut sheet = Raster::new(original.width, original.height);
    for y in mask.y0..(mask.y0 + mask.h).min(original.height) {
        for x in mask.x0..(mask.x0 + mask.w).min(original.width) {
            let m = mask.at(x, y);
            let idx = original.tile_index_of(x, y);
            if m > 0.0 && original.tiles[idx].is_some() {
                let c = original.get(x, y);
                put_pixel(&mut base, x, y, scaled(c, 1.0 - m));
                let moved = scaled(c, m);
                if moved.a() > 0 {
                    put_pixel(&mut sheet, x, y, moved);
                }
            }
        }
    }
    (base, sheet)
}

/// Aperçu rapide d'une déformation pendant un geste (règle d'Anthony : jamais d'attente pendant un
/// geste) : les pixels dessinés de `sheet` dans `src` sont projetés là où `map` les envoie, au plus
/// proche voisin, par blocs qui couvrent l'agrandissement local (pas de trous) ; `map` est
/// interpolée sur une grille de 8 px. Le coût suit le nombre de pixels dessinés, pas la surface. Le
/// résultat exact (`warp_raster`, bilinéaire ; ou le rejeu des traits) vient au lâcher.
pub fn warp_raster_fast(sheet: &Raster, src: Rect, map: impl Fn(Pos2) -> Pos2) -> Raster {
    let mut out = Raster::new(sheet.width, sheet.height);
    if !src.is_positive() || !src.is_finite() {
        return out;
    }
    const CELL: f32 = 8.0;
    let nx = (src.width() / CELL).ceil().max(1.0) as usize;
    let ny = (src.height() / CELL).ceil().max(1.0) as usize;
    let grid: Vec<Pos2> = (0..=ny).flat_map(|j| (0..=nx).map(move |i| (i, j))).map(|(i, j)| map(pos2(src.min.x + i as f32 * CELL, src.min.y + j as f32 * CELL))).collect();
    let at = |i: usize, j: usize| grid[j * (nx + 1) + i];
    // Par case : le côté du bloc, l'agrandissement local arrondi au-dessus.
    let block: Vec<i64> = (0..ny)
        .flat_map(|j| (0..nx).map(move |i| (i, j)))
        .map(|(i, j)| {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
            let sx = a.distance(b).max(c.distance(d)) / CELL;
            let sy = a.distance(c).max(b.distance(d)) / CELL;
            (sx.max(sy).ceil() as i64).clamp(1, 16)
        })
        .collect();
    let (w, h) = (sheet.width as i64, sheet.height as i64);
    let (x0, y0) = (src.min.x.floor().max(0.0) as usize, src.min.y.floor().max(0.0) as usize);
    let (x1, y1) = ((src.max.x.ceil().max(0.0) as usize).min(sheet.width), (src.max.y.ceil().max(0.0) as usize).min(sheet.height));
    if x0 >= x1 || y0 >= y1 {
        return out;
    }
    for ty in (y0 / TILE)..=((y1 - 1) / TILE) {
        for tx in (x0 / TILE)..=((x1 - 1) / TILE) {
            let idx = ty * sheet.tiles_x + tx;
            let Some(tile) = &sheet.tiles[idx] else { continue };
            let (ox, oy) = (tx * TILE, ty * TILE);
            for y in oy.max(y0)..(oy + TILE).min(y1) {
                for x in ox.max(x0)..(ox + TILE).min(x1) {
                    let c = tile[(y - oy) * TILE + (x - ox)];
                    if c.a() == 0 {
                        continue;
                    }
                    let u = (x as f32 + 0.5 - src.min.x) / CELL;
                    let v = (y as f32 + 0.5 - src.min.y) / CELL;
                    let (i, j) = ((u.floor().max(0.0) as usize).min(nx - 1), (v.floor().max(0.0) as usize).min(ny - 1));
                    let (fu, fv) = ((u - i as f32).clamp(0.0, 1.0), (v - j as f32).clamp(0.0, 1.0));
                    let (a, b, cc, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
                    let top = a.to_vec2() * (1.0 - fu) + b.to_vec2() * fu;
                    let bottom = cc.to_vec2() * (1.0 - fu) + d.to_vec2() * fu;
                    let p = top * (1.0 - fv) + bottom * fv;
                    let bsz = block[j * nx + i];
                    let (px, py) = ((p.x - 0.5).round() as i64 - bsz / 2, (p.y - 0.5).round() as i64 - bsz / 2);
                    for by in py.max(0)..(py + bsz).min(h) {
                        for bx in px.max(0)..(px + bsz).min(w) {
                            put_pixel(&mut out, bx as usize, by as usize, c);
                        }
                    }
                }
            }
        }
    }
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
                // Les appuis et les instants suivent les points ajoutés (interpolés).
                let (pts, samples) = vector::densify_traced(points, trace, step);
                let trace = if samples.len() == pts.len() { trace.with_samples(&samples) } else { trace.without_samples() };
                Element::Stroke { points: pts, color: *color, opacity: *opacity, hardness: *hardness, brush: brush.clone(), trace }
            }
            Element::Fill { contours, color, opacity, gradient } => Element::Fill {
                contours: contours.iter().map(|c| densify_points(c, step, true)).collect(),
                color: *color,
                opacity: *opacity,
                gradient: gradient.clone(),
            },
            Element::Text { .. } | Element::Image { .. } => e.clone(),
        };
        *e = persp::warp_element_with(&dense, &map, k);
    }
    out
}

/// L'état d'avant chaque geste d'une séance de déformation bitmap (enveloppe, perspective), pour
/// l'annulation : l'original pour le premier geste, ensuite le résultat exact du précédent (la
/// déformation se refait toujours depuis l'original, mais Ctrl+Z du second geste doit revenir au
/// premier, pas sauter jusqu'à l'original).
#[derive(Default)]
pub struct Committed(Option<Raster>);

impl Committed {
    /// Un geste se termine : `shown` est l'image affichée (l'aperçu), `new` le résultat exact,
    /// `original` l'image du début de la séance. Rend les tuiles d'avant le geste (celles que
    /// l'annulation remet) et les tuiles changées ; `None` : rien n'a changé.
    pub fn step(&mut self, original: &Raster, shown: &Raster, new: &Raster) -> Option<(Vec<(usize, Option<crate::raster::Tile>)>, Vec<usize>)> {
        let prior = self.0.as_ref().unwrap_or(original);
        let mut before = Vec::new();
        let mut changed = Vec::new();
        for idx in 0..new.tiles.len() {
            if prior.tiles.get(idx) != new.tiles.get(idx) || shown.tiles.get(idx) != new.tiles.get(idx) {
                before.push((idx, prior.tiles[idx].clone()));
                changed.push(idx);
            }
        }
        if changed.is_empty() {
            return None;
        }
        self.0 = Some(new.clone());
        Some((before, changed))
    }
}