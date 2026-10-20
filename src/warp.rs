//! Maillage de déformation d'une feuille flottante (enveloppe de Bézier, perspective).
//!
//! Règle d'Anthony : jamais d'attente pendant un geste. Avant, chaque image du geste copiait le
//! reste du dessin, projetait un à un les pixels de la feuille des éléments choisis par le
//! maillage, puis renvoyait des centaines de tuiles à la carte graphique (10 à 35 ms pour 10 à 40
//! gros traits). Maintenant la feuille (les pixels en cache des traits, posés une fois à la saisie,
//! `flottante.rs`) ne change plus : seul ce maillage change à chaque image. C'est une grille de
//! quelques centaines de points, chacun avec l'endroit de la feuille d'où il vient et l'endroit du
//! dessin où il tombe ; la carte graphique la dessine en triangles, la texture de la feuille
//! lissée par-dessus (`gpu.rs`). Le processeur n'y touche que pour le calcul de secours
//! (`render.rs`) et pour terminer un geste lâché tôt (`Flottante::flatten_warp`).

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::raster::{Raster, TILE};
use crate::vector::Affine;

/// Plus petit et plus grand nombre de cases par côté.
const MIN_CELLS: usize = 8;
const MAX_CELLS: usize = 48;
/// Taille voulue d'une case, en pixels de la feuille.
const CELL: f32 = 16.0;

/// Une grille de (n + 1)² points, rangée ligne par ligne.
#[derive(Clone, PartialEq, Debug)]
pub struct Warp {
    /// Cases par côté.
    pub n: usize,
    /// Pour chaque point : sa place dans l'image de la feuille, en pixels (marge comprise).
    pub from: Vec<Pos2>,
    /// Pour chaque point : où il tombe dans le dessin (avant la transformation du calque).
    pub to: Vec<Pos2>,
    /// Pour chaque case : elle n'est pas dessinée (un de ses coins est invisible : derrière l'œil d'une caméra,
    /// pour une carte inclinée de l'Espace 3D). Vide : toutes le sont.
    pub hidden: Vec<bool>,
}

/// Un triangle du maillage : `(où il tombe, d'où il vient)` pour chacun de ses trois sommets.
pub type Tri = [(Pos2, Pos2); 3];

impl Warp {
    /// Le maillage de la région `src` du dessin (la feuille est décalée de `margin` : son pixel
    /// (x, y) est le point (x − marge.0, y − marge.1) du dessin), chaque point allant où `map` le
    /// met. `min_cells` : au moins ce nombre de cases par côté (la perspective en veut plus).
    /// `None` si la région est vide ou infinie.
    pub fn new(src: Rect, margin: (usize, usize), min_cells: usize, map: impl Fn(Pos2) -> Pos2) -> Option<Warp> {
        // (Un peu plus large : le bord adouci du trait fait partie de la feuille.)
        let src = src.expand(2.0);
        if !src.is_positive() || !src.is_finite() {
            return None;
        }
        let side = src.width().max(src.height());
        let n = ((side / CELL).ceil() as usize).clamp(min_cells.max(MIN_CELLS), MAX_CELLS);
        let (mx, my) = (margin.0 as f32, margin.1 as f32);
        let mut from = Vec::with_capacity((n + 1) * (n + 1));
        let mut to = Vec::with_capacity((n + 1) * (n + 1));
        for j in 0..=n {
            for i in 0..=n {
                let p = pos2(src.min.x + src.width() * i as f32 / n as f32, src.min.y + src.height() * j as f32 / n as f32);
                from.push(pos2(p.x + mx, p.y + my));
                to.push(map(p));
            }
        }
        Some(Warp { n, from, to, hidden: Vec::new() })
    }

    /// Un maillage de `n` × `n` cases sur la région `src` du dessin, qui peut ne pas tout montrer : `map` dit où tombe
    /// chaque point, ou rien s'il est invisible (une case dont un coin l'est n'est pas dessinée). `None` si la région est
    /// vide ou infinie, ou si rien n'est visible. Le dessin entier est l'image de la feuille (pas de marge).
    pub fn partial(src: Rect, n: usize, map: impl Fn(Pos2) -> Option<Pos2>) -> Option<Warp> {
        if !src.is_positive() || !src.is_finite() {
            return None;
        }
        let n = n.clamp(1, 128);
        let mut from = Vec::with_capacity((n + 1) * (n + 1));
        let mut to = Vec::with_capacity((n + 1) * (n + 1));
        let mut seen = Vec::with_capacity((n + 1) * (n + 1));
        for j in 0..=n {
            for i in 0..=n {
                let p = pos2(src.min.x + src.width() * i as f32 / n as f32, src.min.y + src.height() * j as f32 / n as f32);
                from.push(p);
                let q = map(p).filter(|q| q.x.is_finite() && q.y.is_finite());
                seen.push(q.is_some());
                to.push(q.unwrap_or(pos2(0.0, 0.0)));
            }
        }
        let mut hidden = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                let a = j * (n + 1) + i;
                hidden.push(!(seen[a] && seen[a + 1] && seen[a + n + 1] && seen[a + n + 2]));
            }
        }
        if hidden.iter().all(|h| *h) {
            return None;
        }
        Some(Warp { n, from, to, hidden })
    }

    /// Les deux triangles de chaque case. `layer` : la transformation du calque, appliquée aux
    /// points d'arrivée. Une case cachée (`hidden`) donne deux triangles sans surface : ils ne dessinent rien, et le
    /// nombre de triangles reste le même.
    pub fn triangles<'a>(&'a self, layer: Option<&'a Affine>) -> impl Iterator<Item = Tri> + 'a {
        let n = self.n;
        let at = move |i: usize, j: usize| {
            let k = j * (n + 1) + i;
            let to = layer.map_or(self.to[k], |a| a.apply(self.to[k]));
            (to, self.from[k])
        };
        (0..n).flat_map(move |j| {
            (0..n).flat_map(move |i| {
                let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
                if self.hidden.get(j * n + i).copied().unwrap_or(false) {
                    return [[a, a, a], [a, a, a]];
                }
                [[a, b, d], [a, d, c]]
            })
        })
    }

    /// Les sommets pour la carte graphique, trois par triangle : position normalisée (`ndc`
    /// convertit un point du dessin) et coordonnées de texture (`dims` : taille de l'image de la
    /// feuille, en pixels).
    pub fn gpu_vertices(&self, dims: (usize, usize), layer: Option<&Affine>, ndc: impl Fn(Pos2) -> [f32; 2]) -> Vec<[f32; 4]> {
        let (w, h) = (dims.0.max(1) as f32, dims.1.max(1) as f32);
        let mut out = Vec::with_capacity(self.n * self.n * 6);
        for tri in self.triangles(layer) {
            for (to, from) in tri {
                let p = ndc(to);
                out.push([p[0], p[1], from.x / w, from.y / h]);
            }
        }
        out
    }

    /// Les pixels de `sheet` posés par le maillage dans une image `width` × `height` (du dessin),
    /// lissés (bilinéaire). Calcul de secours et fin de geste : pas de carte graphique.
    pub fn rasterize(&self, sheet: &Raster, layer: Option<&Affine>, width: usize, height: usize) -> Raster {
        let mut out = Raster::new(width, height);
        self.fill(sheet, layer, (0, 0, width, height), |x, y, c| {
            let idx = out.tile_index_of(x, y);
            out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
        });
        out
    }

    /// Les pixels de `sheet` posés par le maillage dans la zone `(x0, y0, w, h)` du dessin : `put`
    /// reçoit chaque pixel peint (position dans le dessin et couleur prémultipliée).
    pub fn fill(&self, sheet: &Raster, layer: Option<&Affine>, area: (usize, usize, usize, usize), mut put: impl FnMut(usize, usize, Color32)) {
        let (ax, ay, aw, ah) = area;
        let (lo, hi) = (pos2(ax as f32, ay as f32), pos2((ax + aw) as f32, (ay + ah) as f32));
        for tri in self.triangles(layer) {
            let [(pa, fa), (pb, fb), (pc, fc)] = tri;
            if pa.x.max(pb.x).max(pc.x) < lo.x || pa.x.min(pb.x).min(pc.x) > hi.x || pa.y.max(pb.y).max(pc.y) < lo.y || pa.y.min(pb.y).min(pc.y) > hi.y {
                continue;
            }
            if !touches_ink(sheet, &tri) {
                continue;
            }
            let den = (pb.y - pc.y) * (pa.x - pc.x) + (pc.x - pb.x) * (pa.y - pc.y);
            if den.abs() < 1e-6 {
                continue;
            }
            let x0 = pa.x.min(pb.x).min(pc.x).floor().max(lo.x) as usize;
            let x1 = (pa.x.max(pb.x).max(pc.x).ceil().min(hi.x)).max(0.0) as usize;
            let y0 = pa.y.min(pb.y).min(pc.y).floor().max(lo.y) as usize;
            let y1 = (pa.y.max(pb.y).max(pc.y).ceil().min(hi.y)).max(0.0) as usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let l1 = ((pb.y - pc.y) * (p.x - pc.x) + (pc.x - pb.x) * (p.y - pc.y)) / den;
                    let l2 = ((pc.y - pa.y) * (p.x - pc.x) + (pa.x - pc.x) * (p.y - pc.y)) / den;
                    let l3 = 1.0 - l1 - l2;
                    const E: f32 = -1e-4;
                    if l1 < E || l2 < E || l3 < E {
                        continue;
                    }
                    let f = (fa.to_vec2() * l1 + fb.to_vec2() * l2 + fc.to_vec2() * l3).to_pos2();
                    let c = crate::render::sample(sheet, f.x, f.y);
                    if c.a() > 0 {
                        put(x, y, c);
                    }
                }
            }
        }
    }
}

/// Le triangle (d'une case du maillage) recouvre-t-il au moins une tuile dessinée de la feuille ?
/// (Un peu large : la boîte du triangle côté feuille, plus un pixel pour le lissage.)
fn touches_ink(sheet: &Raster, tri: &Tri) -> bool {
    let xs = tri.iter().map(|t| t.1.x);
    let ys = tri.iter().map(|t| t.1.y);
    let (x0, x1) = (xs.clone().fold(f32::MAX, f32::min) - 1.0, xs.fold(f32::MIN, f32::max) + 1.0);
    let (y0, y1) = (ys.clone().fold(f32::MAX, f32::min) - 1.0, ys.fold(f32::MIN, f32::max) + 1.0);
    if x1 < 0.0 || y1 < 0.0 || x0 >= sheet.width as f32 || y0 >= sheet.height as f32 {
        return false;
    }
    let (tx0, ty0) = (x0.max(0.0) as usize / TILE, y0.max(0.0) as usize / TILE);
    let (tx1, ty1) = ((x1 as usize).min(sheet.width - 1) / TILE, (y1 as usize).min(sheet.height - 1) / TILE);
    (ty0..=ty1).any(|ty| (tx0..=tx1).any(|tx| sheet.tiles[ty * sheet.tiles_x + tx].is_some()))
}
