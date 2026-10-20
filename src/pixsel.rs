//! Sélection dans un dessin bitmap : une zone (masque anticrénelé), des pixels « soulevés » que
//! l'on déplace, met à l'échelle ou tourne, puis que l'on repose.
//!
//! Les pixels soulevés gardent leur image d'origine : chaque nouvelle transformation repart d'elle,
//! si bien que tourner puis tourner encore ne floute pas l'image deux fois.

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::raster::{Raster, TILE};
use crate::render::over;
use crate::vector::{self, Affine};

/// Zone sélectionnée : couverture de 0 à 1 de chaque pixel d'un rectangle de l'image.
#[derive(Clone, Debug)]
pub struct PixelMask {
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl PixelMask {
    /// Zone délimitée par un ou plusieurs polygones (lasso, rectangle…).
    pub fn from_polygons(polys: &[Vec<Pos2>], width: usize, height: usize) -> Option<Self> {
        let contours: Vec<Vec<[f32; 2]>> = polys
            .iter()
            .filter(|p| p.len() >= 3)
            .map(|p| p.iter().map(|q| [q.x, q.y]).collect())
            .collect();
        let (x0, y0, w, h, data) = vector::polygon_coverage(&contours, width, height)?;
        let mask = Self { x0, y0, w, h, data };
        (!mask.is_empty()).then_some(mask)
    }

    /// Zone des pixels `true` d'une image `width` × `height` (sélection par couleur), ou rien.
    pub fn from_bools(mask: &[bool], width: usize, height: usize) -> Option<Self> {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..height {
            for x in 0..width {
                if mask[y * width + x] {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        if x0 >= x1 {
            return None;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let data = (0..w * h).map(|i| if mask[(y0 + i / w) * width + x0 + i % w] { 1.0 } else { 0.0 }).collect();
        Some(Self { x0, y0, w, h, data })
    }

    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&m| m <= 0.0)
    }

    pub fn at(&self, x: usize, y: usize) -> f32 {
        if x < self.x0 || y < self.y0 || x >= self.x0 + self.w || y >= self.y0 + self.h {
            return 0.0;
        }
        self.data[(y - self.y0) * self.w + (x - self.x0)]
    }

    pub fn bounds(&self) -> Rect {
        Rect::from_min_max(
            pos2(self.x0 as f32, self.y0 as f32),
            pos2((self.x0 + self.w) as f32, (self.y0 + self.h) as f32),
        )
    }

    /// Cette zone moins `other` (Ctrl+Maj + clic sur un objet déjà choisi : le retirer).
    pub fn without(&self, other: &PixelMask) -> PixelMask {
        let mut data = self.data.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let o = other.at(self.x0 + x, self.y0 + y);
                if o > 0.0 {
                    let v = &mut data[y * self.w + x];
                    *v = (*v - o).max(0.0);
                }
            }
        }
        PixelMask { x0: self.x0, y0: self.y0, w: self.w, h: self.h, data }
    }

    /// Réunion de deux zones (Maj : ajouter à la sélection).
    pub fn union(&self, other: &PixelMask) -> PixelMask {
        let x0 = self.x0.min(other.x0);
        let y0 = self.y0.min(other.y0);
        let x1 = (self.x0 + self.w).max(other.x0 + other.w);
        let y1 = (self.y0 + self.h).max(other.y0 + other.h);
        let (w, h) = (x1 - x0, y1 - y0);
        let mut data = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                data[y * w + x] = self.at(x0 + x, y0 + y).max(other.at(x0 + x, y0 + y));
            }
        }
        PixelMask { x0, y0, w, h, data }
    }
}

/// Pixels soulevés (couleurs prémultipliées), à leur place d'origine dans l'image.
#[derive(Clone)]
pub struct Floating {
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<Color32>,
}

impl Floating {
    pub fn bounds(&self) -> Rect {
        Rect::from_min_max(
            pos2(self.x0 as f32, self.y0 as f32),
            pos2((self.x0 + self.w) as f32, (self.y0 + self.h) as f32),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.pixels.iter().all(|c| c.a() == 0)
    }

    /// Valeur interpolée (bilinéaire) au point `(x, y)` exprimé en pixels du rectangle soulevé.
    fn sample(&self, x: f32, y: f32) -> Color32 {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let get = |x: i64, y: i64| -> [f32; 4] {
            if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
                return [0.0; 4];
            }
            let c = self.pixels[y as usize * self.w + x as usize];
            [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
        };
        let (ix, iy) = (ix as i64, iy as i64);
        let (a, b, c, d) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
        let mut out = [0u8; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            out[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
        }
        // En prémultiplié, une couleur ne dépasse jamais son alpha.
        let a = out[3];
        Color32::from_rgba_premultiplied(out[0].min(a), out[1].min(a), out[2].min(a), a)
    }
}

/// Multiplie une couleur prémultipliée par `k` (0 à 1).
fn scaled(c: Color32, k: f32) -> Color32 {
    if k >= 1.0 {
        return c;
    }
    let m = |v: u8| (v as f32 * k).round() as u8;
    Color32::from_rgba_premultiplied(m(c.r()), m(c.g()), m(c.b()), m(c.a()))
}

fn set(raster: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = raster.tile_index_of(x, y);
    if c.a() == 0 && raster.tiles[idx].is_none() {
        return;
    }
    raster.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

/// Retire de l'image les pixels de la zone et les renvoie.
pub fn lift(raster: &mut Raster, mask: &PixelMask) -> Floating {
    let mut pixels = vec![Color32::TRANSPARENT; mask.w * mask.h];
    for ly in 0..mask.h {
        for lx in 0..mask.w {
            let m = mask.data[ly * mask.w + lx];
            if m <= 0.0 {
                continue;
            }
            let (x, y) = (mask.x0 + lx, mask.y0 + ly);
            let c = raster.get(x, y);
            if c.a() == 0 {
                continue;
            }
            pixels[ly * mask.w + lx] = scaled(c, m);
            set(raster, x, y, scaled(c, 1.0 - m));
        }
    }
    Floating {
        x0: mask.x0,
        y0: mask.y0,
        w: mask.w,
        h: mask.h,
        pixels,
    }
}

/// Copie les pixels de la zone, sans toucher à l'image.
pub fn copy(raster: &Raster, mask: &PixelMask) -> Floating {
    let mut copy = raster.clone();
    lift(&mut copy, mask)
}

/// Garde seulement ce qui est dans la zone (« effacer hors de la sélection »).
pub fn keep_only(raster: &mut Raster, mask: &PixelMask) {
    for idx in 0..raster.tiles.len() {
        if raster.tiles[idx].is_none() {
            continue;
        }
        let (ox, oy, tw, th) = raster.tile_bounds(idx);
        for y in oy..oy + th {
            for x in ox..ox + tw {
                let m = mask.at(x, y);
                if m < 1.0 {
                    let c = raster.get(x, y);
                    set(raster, x, y, scaled(c, m));
                }
            }
        }
    }
}

/// Pose les pixels soulevés, transformés par `aff`, par-dessus l'image. Renvoie la zone touchée.
pub fn stamp(raster: &mut Raster, f: &Floating, aff: &Affine) -> Option<Rect> {
    let inv = aff.inverse()?;
    let area = aff.apply_rect(f.bounds()).expand(1.0);
    let x0 = area.min.x.floor().max(0.0) as usize;
    let y0 = area.min.y.floor().max(0.0) as usize;
    let x1 = (area.max.x.ceil().max(0.0) as usize).min(raster.width);
    let y1 = (area.max.y.ceil().max(0.0) as usize).min(raster.height);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (ox, oy) = (f.x0 as f32, f.y0 as f32);
    for y in y0..y1 {
        for x in x0..x1 {
            let src = inv.apply(pos2(x as f32 + 0.5, y as f32 + 0.5));
            let c = f.sample(src.x - ox, src.y - oy);
            if c.a() == 0 {
                continue;
            }
            let dst = raster.get(x, y);
            set(raster, x, y, over(dst, c, 255, None));
        }
    }
    Some(Rect::from_min_max(pos2(x0 as f32, y0 as f32), pos2(x1 as f32, y1 as f32)))
}

/// Contour d'un rectangle, pour l'affichage et les masques.
pub fn rect_polygon(r: Rect) -> Vec<Pos2> {
    vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
}
