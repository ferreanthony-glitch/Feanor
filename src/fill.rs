use eframe::egui::Color32;

use crate::raster::{Raster, Tile, TILE};

#[derive(Clone, Copy)]
pub struct FillSettings {
    /// Écart de couleur toléré, de 0 à 1.
    pub tolerance: f32,
    /// Débord en pixels : le remplissage passe sous le bord anticrénelé des traits.
    pub expand: u32,
    /// Chercher les contours sur tous les calques visibles plutôt que sur le seul calque courant.
    pub sample_all: bool,
}

impl Default for FillSettings {
    fn default() -> Self {
        Self {
            tolerance: 0.12,
            expand: 1,
            sample_all: false,
        }
    }
}

fn close(a: Color32, b: Color32, tol: i32) -> bool {
    (a.r() as i32 - b.r() as i32).abs() <= tol
        && (a.g() as i32 - b.g() as i32).abs() <= tol
        && (a.b() as i32 - b.b() as i32).abs() <= tol
        && (a.a() as i32 - b.a() as i32).abs() <= tol
}

/// Zone connexe de même couleur que le point de départ (remplissage par lignes).
pub fn flood(src: &[Color32], w: usize, h: usize, sx: usize, sy: usize, tol: u8) -> Vec<bool> {
    let tol = tol as i32;
    let target = src[sy * w + sx];
    let mut mask = vec![false; w * h];
    let ok = |mask: &[bool], i: usize| !mask[i] && close(src[i], target, tol);
    let mut stack = vec![(sx, sy)];
    while let Some((x, y)) = stack.pop() {
        let row = y * w;
        if !ok(&mask, row + x) {
            continue;
        }
        let mut l = x;
        while l > 0 && ok(&mask, row + l - 1) {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < w && ok(&mask, row + r + 1) {
            r += 1;
        }
        mask[row + l..=row + r].fill(true);
        for ny in [y.wrapping_sub(1), y + 1] {
            if ny >= h {
                continue;
            }
            let nrow = ny * w;
            let mut i = l;
            while i <= r {
                if ok(&mask, nrow + i) {
                    stack.push((i, ny));
                    while i <= r && ok(&mask, nrow + i) {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
    mask
}

/// Élargit la zone de `radius` pixels.
pub fn dilate(mask: &[bool], w: usize, h: usize, radius: u32) -> Vec<bool> {
    let mut cur = mask.to_vec();
    for _ in 0..radius {
        let prev = cur.clone();
        for y in 0..h {
            for x in 0..w {
                if prev[y * w + x] {
                    continue;
                }
                let y_lo = y.saturating_sub(1);
                let y_hi = (y + 1).min(h - 1);
                let x_lo = x.saturating_sub(1);
                let x_hi = (x + 1).min(w - 1);
                let hit = (y_lo..=y_hi).any(|ny| prev[ny * w + x_lo..=ny * w + x_hi].iter().any(|&b| b));
                if hit {
                    cur[y * w + x] = true;
                }
            }
        }
    }
    cur
}

/// Peint la zone dans le dessin et renvoie les tuiles d'avant (pour l'annulation).
///
/// Dans la zone trouvée, la couleur remplace l'existant (ce qui permet de recolorer un aplat) ;
/// dans le débord, elle passe *sous* l'existant pour ne pas abîmer le trait.
pub fn apply(raster: &mut Raster, region: &[bool], grown: &[bool], rgb: [u8; 3]) -> Vec<(usize, Option<Tile>)> {
    let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    let w = raster.width;
    let mut before = Vec::new();
    for idx in 0..raster.tiles.len() {
        let (ox, oy, tw, th) = raster.tile_bounds(idx);
        let touched = (0..th).any(|y| {
            let s = (oy + y) * w + ox;
            grown[s..s + tw].iter().any(|&g| g)
        });
        if !touched {
            continue;
        }
        before.push((idx, raster.tiles[idx].clone()));
        let tile = raster.tile_mut(idx);
        for y in 0..th {
            for x in 0..tw {
                let gi = (oy + y) * w + ox + x;
                if !grown[gi] {
                    continue;
                }
                let li = y * TILE + x;
                tile[li] = if region[gi] { color } else { behind(tile[li], color) };
            }
        }
    }
    before
}

fn behind(top: Color32, under: Color32) -> Color32 {
    let inv = 255 - top.a() as u32;
    let ch = |t: u8, u: u8| (t as u32 + (u as u32 * inv + 127) / 255).min(255) as u8;
    Color32::from_rgba_premultiplied(
        ch(top.r(), under.r()),
        ch(top.g(), under.g()),
        ch(top.b(), under.b()),
        255,
    )
}
