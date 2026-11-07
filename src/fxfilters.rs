//! Effets de la pile d'un calque, au-delà des flous (Anthony, 2026-10-17 : « ajoute une partie
//! effets [qui comprend] les flous, les niveaux, contrastes, etc. Les effets s'inspirent de
//! Photoshop et sont non destructifs ») : réglages de couleur, netteté, bruit, mosaïque, ombre
//! portée, lueur. Comme les flous (`fx.rs`), chacun prend le dessin d'un calque et en rend une
//! copie filtrée ; le dessin, lui, ne change jamais. Les pixels vides restent vides, sauf pour
//! les effets qui débordent (mosaïque, ombre, lueur).

use eframe::egui::Color32;

use crate::fx::{gaussian, into_raster, region_of, Region};
use crate::raster::{Raster, TILE};

/// Couleur sans prémultiplication (0 à 1) et couverture (0 à 1) d'un pixel.
pub(crate) fn unpremul(p: Color32) -> ([f32; 3], f32) {
    let a = p.a() as f32 / 255.0;
    if a <= 0.0 {
        return ([0.0; 3], 0.0);
    }
    ([p.r() as f32 / 255.0 / a, p.g() as f32 / 255.0 / a, p.b() as f32 / 255.0 / a], a)
}

/// Le pixel de couleur `rgb` (0 à 1) et de couverture `a`, prémultiplié.
pub(crate) fn premul(rgb: [f32; 3], a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let ch = |c: f32| (c.clamp(0.0, 1.0) * a * 255.0 + 0.5) as u8;
    Color32::from_rgba_premultiplied(ch(rgb[0]), ch(rgb[1]), ch(rgb[2]), (a * 255.0 + 0.5) as u8)
}

fn luminance(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// Le dessin avec la couleur de chaque pixel dessiné passée par `f` (couleur, x, y) ; la
/// couverture ne change pas.
pub(crate) fn map_colors(raster: &Raster, f: impl Fn([f32; 3], usize, usize) -> [f32; 3] + Sync) -> Raster {
    let mut out = raster.clone();
    let (width, height) = (out.width, out.height);
    // Une rangée de tuiles par morceau, répartie sur les cœurs : chaque pixel ne dépend que de
    // lui-même, le résultat ne dépend pas de la répartition.
    let tiles_x = out.tiles_x.max(1);
    crate::render::par_chunks(&mut out.tiles, tiles_x, |k, row| {
        for (tx, tile) in row.iter_mut().enumerate() {
            let Some(tile) = tile.as_mut() else { continue };
            let (ox, oy, tw, th) = crate::raster::tile_bounds(width, height, k * tiles_x + tx);
            for y in 0..th {
                for x in 0..tw {
                    let i = y * TILE + x;
                    let (rgb, a) = unpremul(tile[i]);
                    if a > 0.0 {
                        tile[i] = premul(f(rgb, ox + x, oy + y), a);
                    }
                }
            }
        }
    });
    out
}

/// Luminosité et contraste (−100 à 100 chacun).
pub fn brightness_contrast(raster: &Raster, brightness: f32, contrast: f32) -> Raster {
    let k = if contrast >= 0.0 { 1.0 + contrast / 100.0 * 3.0 } else { 1.0 + contrast / 100.0 };
    let b = brightness / 100.0 * 0.5;
    map_colors(raster, |c, _, _| c.map(|v| (v - 0.5) * k + 0.5 + b))
}

/// Niveaux : `black` et `white` (0 à 255) deviennent le noir et le blanc ; `gamma` courbe les
/// tons moyens (plus grand : plus clair).
pub fn levels(raster: &Raster, black: f32, white: f32, gamma: f32) -> Raster {
    let span = (white - black).max(1.0);
    let g = 1.0 / gamma.max(0.01);
    map_colors(raster, |c, _, _| c.map(|v| ((v * 255.0 - black) / span).clamp(0.0, 1.0).powf(g)))
}

/// Teinte (−180° à 180°), saturation et luminosité (−100 à 100).
pub fn hue_saturation(raster: &Raster, hue: f32, saturation: f32, lightness: f32) -> Raster {
    let (s_k, l_k) = (1.0 + saturation / 100.0, lightness / 100.0);
    map_colors(raster, |c, _, _| {
        let (h, s, l) = rgb_to_hsl(c);
        let h = (h + hue / 360.0).rem_euclid(1.0);
        let s = (s * s_k).clamp(0.0, 1.0);
        let l = if l_k >= 0.0 { l + (1.0 - l) * l_k } else { l * (1.0 + l_k) };
        hsl_to_rgb(h, s, l)
    })
}

/// Noir et blanc : la couleur va vers sa luminance de `amount` (0 à 100 %).
pub fn black_white(raster: &Raster, amount: f32) -> Raster {
    let t = (amount / 100.0).clamp(0.0, 1.0);
    map_colors(raster, |c, _, _| {
        let y = luminance(c);
        c.map(|v| v + (y - v) * t)
    })
}

/// Négatif, de `amount` (0 à 100 %).
pub fn invert(raster: &Raster, amount: f32) -> Raster {
    let t = (amount / 100.0).clamp(0.0, 1.0);
    map_colors(raster, |c, _, _| c.map(|v| v + (1.0 - 2.0 * v) * t))
}

/// Postérisation : `levels` niveaux par couleur (2 à 32).
pub fn posterize(raster: &Raster, levels: f32) -> Raster {
    let n = levels.round().max(2.0) - 1.0;
    map_colors(raster, |c, _, _| c.map(|v| (v * n).round() / n))
}

/// Seuil : noir sous `level` (0 à 255 de luminance), blanc au-dessus.
pub fn threshold(raster: &Raster, level: f32) -> Raster {
    map_colors(raster, |c, _, _| if luminance(c) * 255.0 >= level { [1.0; 3] } else { [0.0; 3] })
}

/// Le hasard (−1 à 1) du pixel (x, y) de la feuille : le même à chaque fois (le résultat du bruit
/// ne change pas d'un affichage à l'autre).
fn noise_at(x: i64, y: i64) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Bruit : chaque pixel dessiné s'éclaircit ou s'assombrit au hasard, jusqu'à `amount` (0 à
/// 100 %). Sur un dessin réduit `1 << level` fois (aperçu, `fxproxy.rs`) : chaque pixel prend la
/// moyenne du hasard des pixels de la feuille qu'il couvre, comme l'affichage réduit ferait du
/// bruit en pleine taille. `level` 0 : le bruit lui-même.
#[cfg(test)]
pub fn noise_scaled(raster: &Raster, amount: f32, level: u8) -> Raster {
    noise_depuis(raster, amount, level, (0, 0))
}

/// Le bruit d'une image dont le pixel (0, 0) est le point `origine` du monde : le hasard est celui de ce point du monde.
pub(crate) fn noise_depuis(raster: &Raster, amount: f32, level: u8, origine: (i64, i64)) -> Raster {
    let k = amount / 100.0 * 0.5;
    let f = 1usize << level;
    map_colors(raster, |c, x, y| {
        let n = if f == 1 {
            noise_at(x as i64 + origine.0, y as i64 + origine.1)
        } else {
            let mut sum = 0.0;
            for j in 0..f {
                for i in 0..f {
                    sum += noise_at((x * f + i) as i64, (y * f + j) as i64);
                }
            }
            sum / (f * f) as f32
        };
        c.map(|v| v + n * k)
    })
}

/// Netteté (accentuation) : la différence avec un flou de rayon `radius` (pixels), accentuée de
/// `gain` (0 à 300 %).
pub fn sharpen(raster: &Raster, gain: f32, radius: f32) -> Raster {
    let blur = gaussian(raster, radius.max(0.5));
    let g = gain / 100.0;
    let mut out = raster.clone();
    let (width, height) = (out.width, out.height);
    let tiles_x = out.tiles_x.max(1);
    let blur = &blur;
    crate::render::par_chunks(&mut out.tiles, tiles_x, |k, row| {
        for (tx, tile) in row.iter_mut().enumerate() {
            let Some(tile) = tile.as_mut() else { continue };
            let (ox, oy, tw, th) = crate::raster::tile_bounds(width, height, k * tiles_x + tx);
            for y in 0..th {
                for x in 0..tw {
                    let i = y * TILE + x;
                    let (o, a) = unpremul(tile[i]);
                    if a <= 0.0 {
                        continue;
                    }
                    let (b, ba) = unpremul(blur.get(ox + x, oy + y));
                    let b = if ba > 0.0 { b } else { o };
                    tile[i] = premul([0, 1, 2].map(|k| o[k] + (o[k] - b[k]) * g), a);
                }
            }
        }
    });
    out
}

/// Mosaïque : des carrés de `size` pixels (alignés sur la feuille), chacun de la couleur
/// moyenne de ce qu'il couvre.
#[cfg(test)]
pub fn mosaic(raster: &Raster, size: f32) -> Raster {
    mosaic_depuis(raster, size, (0, 0))
}

/// La mosaïque d'une image dont le pixel (0, 0) est le point `origine` du monde : les carrés tombent sur la grille du monde, celle de la feuille.
pub(crate) fn mosaic_depuis(raster: &Raster, size: f32, origine: (i64, i64)) -> Raster {
    let s = size.round().max(1.0) as usize;
    let Some(mut r) = region_of(raster, s) else { return Raster::new(raster.width, raster.height) };
    // Les carrés tombent sur la grille de la feuille, pas de la zone (en reculant, au besoin, avant le bord de l'image).
    let cale = |lo: usize, o: i64| lo as i64 - (lo as i64 + o).rem_euclid(s as i64);
    let (bx0, by0) = (cale(r.x0, origine.0), cale(r.y0, origine.1));
    let mut by = by0;
    while by < (r.y0 + r.h) as i64 {
        let mut bx = bx0;
        while bx < (r.x0 + r.w) as i64 {
            let (x0, y0) = (bx.max(r.x0 as i64) as usize, by.max(r.y0 as i64) as usize);
            let (x1, y1) = (((bx + s as i64) as usize).min(r.x0 + r.w), ((by + s as i64) as usize).min(r.y0 + r.h));
            let mut acc = [0u32; 4];
            let n = ((x1 - x0) * (y1 - y0)) as u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = r.px[(y - r.y0) * r.w + (x - r.x0)];
                    for (k, v) in p.to_array().iter().enumerate() {
                        acc[k] += *v as u32;
                    }
                }
            }
            if n > 0 && acc[3] > 0 {
                let m = |v: u32| ((v + n / 2) / n) as u8;
                let c = Color32::from_rgba_premultiplied(m(acc[0]), m(acc[1]), m(acc[2]), m(acc[3]));
                for y in y0..y1 {
                    for x in x0..x1 {
                        r.px[(y - r.y0) * r.w + (x - r.x0)] = c;
                    }
                }
            }
            bx += s as i64;
        }
        by += s as i64;
    }
    into_raster(&r, raster)
}

/// `top` posé sur `bottom` (prémultipliés).
fn over(top: Color32, bottom: Color32) -> Color32 {
    crate::brushfill::over_premul(top, bottom)
}

/// Le dessin posé sur `under` (même taille), pixel par pixel.
fn over_raster(raster: &Raster, under: &Raster) -> Raster {
    let mut out = under.clone();
    let tiles_x = out.tiles_x.max(1);
    // Une rangée de tuiles par morceau, répartie sur les cœurs.
    crate::render::par_chunks(&mut out.tiles, tiles_x, |k, row| {
        for (tx, slot) in row.iter_mut().enumerate() {
            let Some(top) = raster.tiles[k * tiles_x + tx].as_ref() else { continue };
            match slot {
                None => *slot = Some(top.clone()),
                Some(tile) => {
                    for (i, p) in tile.iter_mut().enumerate() {
                        *p = over(top[i], *p);
                    }
                }
            }
        }
    });
    out
}

/// Ombre portée : la silhouette du dessin, noire, décalée de (`dx`, `dy`) pixels, floutée de
/// `blur` pixels, à `opacity` (0 à 100 %), sous le dessin.
pub fn drop_shadow(raster: &Raster, dx: f32, dy: f32, blur: f32, opacity: f32) -> Raster {
    let (dx, dy) = (dx.round() as i64, dy.round() as i64);
    let margin = dx.unsigned_abs().max(dy.unsigned_abs()) as usize + 2;
    let Some(r) = region_of(raster, margin) else { return Raster::new(raster.width, raster.height) };
    let k = (opacity / 100.0).clamp(0.0, 1.0);
    let mut px = vec![Color32::TRANSPARENT; r.w * r.h];
    let (rw, rh) = (r.w as i64, r.h as i64);
    let src = &r.px;
    crate::render::par_chunks(&mut px, r.w * 16, |chunk, rows| {
        for (j, row) in rows.chunks_mut(r.w).enumerate() {
            let y = (chunk * 16 + j) as i64;
            let sy = y - dy;
            if sy < 0 || sy >= rh {
                continue;
            }
            for (x, d) in row.iter_mut().enumerate() {
                let sx = x as i64 - dx;
                if sx < 0 || sx >= rw {
                    continue;
                }
                let a = src[sy as usize * r.w + sx as usize].a() as f32 * k;
                *d = Color32::from_rgba_premultiplied(0, 0, 0, (a + 0.5) as u8);
            }
        }
    });
    let shadow = into_raster(&Region { px, ..r }, raster);
    let shadow = if blur >= 0.5 { gaussian(&shadow, blur) } else { shadow };
    over_raster(raster, &shadow)
}

/// Lueur : le dessin flouté de `radius` pixels, renforcé de `intensity` (0 à 200 %), sous le
/// dessin.
pub fn glow(raster: &Raster, radius: f32, intensity: f32) -> Raster {
    let mut halo = gaussian(raster, radius.max(1.0));
    let k = intensity / 100.0;
    crate::render::par_chunks(&mut halo.tiles, 32, |_, tiles| {
        for t in tiles.iter_mut().flatten() {
            for p in t.iter_mut() {
                let ch = |v: u8| (v as f32 * k).min(255.0);
                let a = ch(p.a());
                let c = |v: u8| ch(v).min(a) as u8;
                *p = Color32::from_rgba_premultiplied(c(p.r()), c(p.g()), c(p.b()), a as u8);
            }
        }
    });
    over_raster(raster, &halo)
}

/// Teinte, saturation, luminosité (0 à 1) d'une couleur.
pub fn rgb_to_hsl(c: [f32; 3]) -> (f32, f32, f32) {
    let (max, min) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let l = (max + min) / 2.0;
    if max - min < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == c[0] {
        (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
    } else if max == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    (h / 6.0, s, l)
}

/// La couleur d'une teinte, saturation et luminosité (0 à 1).
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s <= 0.0 {
        return [l; 3];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}
