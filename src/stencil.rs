//! Pochoirs (13b) : le dessin d'un calque-pochoir limite où l'on peint sur les autres calques.
//! Positif : seulement dans sa forme ; négatif : seulement hors d'elle. Plusieurs pochoirs se
//! cumulent (on ne peint que là où tous laissent passer).
//!
//! Le masque est calculé une fois au début du trait (le pochoir ne change pas pendant qu'on
//! peint ailleurs), tuile par tuile du dessin peint : une tuile que rien ne touche garde une
//! seule valeur, seules les tuiles au bord d'une forme ont un facteur par pixel.

use eframe::egui::{pos2, Pos2, Rect};

use crate::raster::{Raster, TILE, TILE_PIXELS};
use crate::vector::Affine;

enum Cover {
    /// Toute la tuile laisse passer autant (0 : rien, 255 : tout).
    All(u8),
    Pixels(Box<[u8; TILE_PIXELS]>),
}

/// Où un trait peut peindre : un facteur de 0 à 255 par pixel du dessin peint.
pub struct Stencil {
    tiles_x: usize,
    tiles: Vec<Cover>,
}

impl Stencil {
    /// Masque qui laisse tout passer, pour un dessin de `width`×`height`.
    pub fn new(width: usize, height: usize) -> Self {
        let tiles_x = width.div_ceil(TILE);
        let tiles_y = height.div_ceil(TILE);
        Self { tiles_x, tiles: (0..tiles_x * tiles_y).map(|_| Cover::All(255)).collect() }
    }

    /// Ajoute un calque-pochoir : `raster` est son dessin ; `to_stencil` mène d'un point du dessin
    /// peint au point du dessin du pochoir (aucun : les deux calques sont à la même place).
    pub fn add(&mut self, raster: &Raster, negative: bool, to_stencil: Option<Affine>) {
        let pass = |a: u8| if negative { 255 - a } else { a };
        let outside = pass(0);
        for idx in 0..self.tiles.len() {
            let (ox, oy) = ((idx % self.tiles_x) * TILE, (idx / self.tiles_x) * TILE);
            // Facteur de chaque pixel de la tuile ; aucun : le même partout.
            let factors: Option<Box<[u8; TILE_PIXELS]>> = match &to_stencil {
                None => raster.tiles.get(stencil_tile_index(raster, ox, oy)).and_then(|t| t.as_ref()).map(|t| {
                    let mut f = Box::new([0u8; TILE_PIXELS]);
                    for (k, c) in t.iter().enumerate() {
                        f[k] = pass(c.a());
                    }
                    f
                }),
                Some(m) => {
                    let r = Rect::from_min_size(pos2(ox as f32, oy as f32), eframe::egui::vec2(TILE as f32, TILE as f32));
                    if !touches(raster, m.apply_rect(r)) {
                        None
                    } else {
                        let mut f = Box::new([0u8; TILE_PIXELS]);
                        for py in 0..TILE {
                            for px in 0..TILE {
                                let p = m.apply(pos2((ox + px) as f32 + 0.5, (oy + py) as f32 + 0.5));
                                f[py * TILE + px] = pass(alpha_at(raster, p));
                            }
                        }
                        Some(f)
                    }
                }
            };
            let cover = &mut self.tiles[idx];
            match (factors, &mut *cover) {
                (None, Cover::All(v)) => *v = mul(*v, outside),
                (None, Cover::Pixels(px)) => {
                    for v in px.iter_mut() {
                        *v = mul(*v, outside);
                    }
                }
                (Some(f), Cover::All(v)) => {
                    let v = *v;
                    let mut f = f;
                    for x in f.iter_mut() {
                        *x = mul(*x, v);
                    }
                    *cover = Cover::Pixels(f);
                }
                (Some(f), Cover::Pixels(px)) => {
                    for (v, x) in px.iter_mut().zip(f.iter()) {
                        *v = mul(*v, *x);
                    }
                }
            }
        }
    }

    /// Rien ne passe dans la tuile `idx` : inutile d'y peindre.
    pub fn blocks(&self, idx: usize) -> bool {
        matches!(self.tiles.get(idx), Some(Cover::All(0)))
    }

    /// Facteur (0 à 1) du pixel `i` de la tuile `idx`.
    pub fn at(&self, idx: usize, i: usize) -> f32 {
        match self.tiles.get(idx) {
            Some(Cover::All(v)) => *v as f32 / 255.0,
            Some(Cover::Pixels(px)) => px[i] as f32 / 255.0,
            None => 1.0,
        }
    }
}

fn mul(a: u8, b: u8) -> u8 {
    ((a as u32 * b as u32 + 127) / 255) as u8
}

/// Tuile du pochoir qui contient le coin (`ox`, `oy`) (mêmes repères, même taille de feuille).
fn stencil_tile_index(raster: &Raster, ox: usize, oy: usize) -> usize {
    if ox >= raster.width || oy >= raster.height {
        return usize::MAX;
    }
    (oy / TILE) * raster.tiles_x + ox / TILE
}

/// Une des tuiles dessinées du pochoir touche `r` (repère du pochoir).
fn touches(raster: &Raster, r: Rect) -> bool {
    let (w, h) = (raster.width as f32, raster.height as f32);
    if r.max.x < 0.0 || r.max.y < 0.0 || r.min.x >= w || r.min.y >= h {
        return false;
    }
    let tx0 = (r.min.x.max(0.0) as usize) / TILE;
    let ty0 = (r.min.y.max(0.0) as usize) / TILE;
    let tx1 = ((r.max.x.min(w - 1.0)) as usize) / TILE;
    let ty1 = ((r.max.y.min(h - 1.0)) as usize) / TILE;
    (ty0..=ty1).any(|ty| (tx0..=tx1).any(|tx| raster.tiles[ty * raster.tiles_x + tx].is_some()))
}

/// Opacité du pochoir au point `p` (pixel le plus proche ; hors de la feuille : rien).
fn alpha_at(raster: &Raster, p: Pos2) -> u8 {
    if p.x < 0.0 || p.y < 0.0 || p.x >= raster.width as f32 || p.y >= raster.height as f32 {
        return 0;
    }
    raster.get(p.x as usize, p.y as usize).a()
}
