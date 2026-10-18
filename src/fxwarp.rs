//! Effets « Déformation » et « Rendu » de la pile d'un calque (Anthony, 2026-10-17 : « ajoute
//! Déformation et Rendu », d'après le menu Filtre de Photoshop). Non destructifs comme les
//! autres (`fx.rs`) : une copie déformée ou éclairée du dessin, le dessin ne change pas.
//! - Déformation : onde, ondulation, tourbillon, pincement. Chaque pixel de la feuille va
//!   chercher sa couleur ailleurs dans le dessin (lecture bilinéaire), donc le dessin peut
//!   déborder de sa place. Les centres sont en pour cent de la feuille (animables par clés).
//! - Rendu : nuages, éclairage, dégradé, qui changent la couleur des pixels dessinés (la
//!   couverture reste) : ils texturent ou éclairent le dessin plutôt que de remplir la feuille
//!   comme dans Photoshop (mon choix, pour un calque d'animation).

use eframe::egui::Color32;

use crate::fx::{into_raster, Region};
use crate::raster::{Raster, TILE};

/// La couleur prémultipliée du dessin au point (x, y) en pixels de la feuille, lue entre ses
/// quatre pixels voisins ; vide hors de la feuille.
pub(crate) fn sample(r: &Raster, x: f32, y: f32) -> [f32; 4] {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (ix, iy) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - ix, fy - iy);
    let get = |x: f32, y: f32| -> [f32; 4] {
        if x < 0.0 || y < 0.0 || x >= r.width as f32 || y >= r.height as f32 {
            return [0.0; 4];
        }
        let p = r.get(x as usize, y as usize);
        [p.r() as f32, p.g() as f32, p.b() as f32, p.a() as f32]
    };
    let (a, b, c, d) = (get(ix, iy), get(ix + 1.0, iy), get(ix, iy + 1.0), get(ix + 1.0, iy + 1.0));
    std::array::from_fn(|k| {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bottom = c[k] + (d[k] - c[k]) * tx;
        top + (bottom - top) * ty
    })
}

/// Toute la feuille, chaque pixel (x, y) prenant la couleur du dessin au point que `from` lui
/// donne (centre du pixel en entrée).
fn warp(raster: &Raster, from: impl Fn(f32, f32) -> (f32, f32) + Sync) -> Raster {
    if raster.tiles.iter().all(Option::is_none) {
        return raster.clone();
    }
    let (w, h) = (raster.width, raster.height);
    let mut px = vec![Color32::TRANSPARENT; w * h];
    crate::render::par_chunks(&mut px, w * 8, |chunk, rows| {
        for (k, o) in rows.iter_mut().enumerate() {
            let (x, y) = ((k % w) as f32 + 0.5, (chunk * 8 + k / w) as f32 + 0.5);
            let (sx, sy) = from(x, y);
            let c = sample(raster, sx, sy);
            let a = c[3].round().clamp(0.0, 255.0);
            let ch = |v: f32| v.round().clamp(0.0, a) as u8;
            *o = Color32::from_rgba_premultiplied(ch(c[0]), ch(c[1]), ch(c[2]), a as u8);
        }
    });
    into_raster(&Region { x0: 0, y0: 0, w, h, px }, raster)
}

/// Centre (en pixels) et demi-côté de la feuille, d'après des pour cent.
fn center(raster: &Raster, cx: f32, cy: f32) -> (f32, f32, f32) {
    let (w, h) = (raster.width as f32, raster.height as f32);
    (w * cx / 100.0, h * cy / 100.0, w.min(h) / 2.0)
}

/// Onde : le dessin ondule, de `amplitude` pixels, avec des vagues de `wavelength` pixels qui
/// avancent dans la direction `angle` (0° : vagues verticales qui déplacent le dessin de haut en
/// bas, comme un drapeau).
pub fn wave(raster: &Raster, amplitude: f32, wavelength: f32, angle: f32) -> Raster {
    let (s, c) = angle.to_radians().sin_cos();
    let k = std::f32::consts::TAU / wavelength.max(1.0);
    warp(raster, |x, y| {
        let d = amplitude * ((x * c + y * s) * k).sin();
        (x - s * d, y + c * d)
    })
}

/// Ondulation : des cercles autour du centre (`cx`, `cy` en % de la feuille), de `amplitude`
/// pixels et de `wavelength` pixels, comme une pierre dans l'eau.
pub fn ripple(raster: &Raster, amplitude: f32, wavelength: f32, cx: f32, cy: f32) -> Raster {
    let (ox, oy, _) = center(raster, cx, cy);
    let k = std::f32::consts::TAU / wavelength.max(1.0);
    warp(raster, |x, y| {
        let (dx, dy) = (x - ox, y - oy);
        let d = (dx * dx + dy * dy).sqrt();
        if d < 1e-3 {
            return (x, y);
        }
        let m = amplitude * (d * k).sin() / d;
        (x + dx * m, y + dy * m)
    })
}

/// Tourbillon : le dessin tourne de `angle` degrés au centre, de moins en moins jusqu'au bord du
/// cercle de rayon `radius` (% du demi-côté de la feuille).
pub fn twirl(raster: &Raster, angle: f32, radius: f32, cx: f32, cy: f32) -> Raster {
    let (ox, oy, half) = center(raster, cx, cy);
    let r = (half * radius / 100.0).max(1.0);
    let a = angle.to_radians();
    warp(raster, |x, y| {
        let (dx, dy) = (x - ox, y - oy);
        let d = (dx * dx + dy * dy).sqrt();
        if d >= r {
            return (x, y);
        }
        let t = -a * (1.0 - d / r).powi(2);
        let (s, c) = t.sin_cos();
        (ox + dx * c - dy * s, oy + dx * s + dy * c)
    })
}

/// Pincement : dans le cercle de rayon `radius` (% du demi-côté), le dessin rentre vers le
/// centre (`amount` > 0, jusqu'à 100) ou bombe comme une sphère (`amount` < 0).
pub fn pinch(raster: &Raster, amount: f32, radius: f32, cx: f32, cy: f32) -> Raster {
    let (ox, oy, half) = center(raster, cx, cy);
    let r = (half * radius / 100.0).max(1.0);
    let e = 1.0 - (amount / 100.0).clamp(-1.0, 1.0) * 0.9;
    warp(raster, |x, y| {
        let (dx, dy) = (x - ox, y - oy);
        let d = (dx * dx + dy * dy).sqrt();
        if d >= r || d < 1e-3 {
            return (x, y);
        }
        let f = (d / r).powf(e) * r / d;
        (ox + dx * f, oy + dy * f)
    })
}

/// La couleur de chaque pixel dessiné multipliée par `factor(x, y)` (la couverture reste).
fn shade(raster: &Raster, factor: impl Fn(f32, f32) -> f32 + Sync) -> Raster {
    let mut out = raster.clone();
    let (width, height) = (out.width, out.height);
    let tiles_x = out.tiles_x.max(1);
    // Une rangée de tuiles par morceau, répartie sur les cœurs (chaque pixel est indépendant).
    crate::render::par_chunks(&mut out.tiles, tiles_x, |k, row| {
        for (tx, tile) in row.iter_mut().enumerate() {
            let Some(tile) = tile.as_mut() else { continue };
            let (ox, oy, tw, th) = crate::raster::tile_bounds(width, height, k * tiles_x + tx);
            for y in 0..th {
                for x in 0..tw {
                    let i = y * TILE + x;
                    let p = tile[i];
                    if p.a() == 0 {
                        continue;
                    }
                    let k = factor((ox + x) as f32 + 0.5, (oy + y) as f32 + 0.5).max(0.0);
                    let ch = |v: u8| (v as f32 * k).round().clamp(0.0, p.a() as f32) as u8;
                    tile[i] = Color32::from_rgba_premultiplied(ch(p.r()), ch(p.g()), ch(p.b()), p.a());
                }
            }
        }
    });
    out
}

/// Bruit lissé (0 à 1) au point (x, y), en unités de cases, pour la graine `seed`.
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let hash = |i: i32, j: i32| {
        let mut h = (i as u32).wrapping_mul(0x8DA6_B343) ^ (j as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5BD1_E995);
        h ^= h >> 15;
        (h & 0xFFFF) as f32 / 65535.0
    };
    let (ix, iy) = (x.floor(), y.floor());
    let (tx, ty) = (x - ix, y - iy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (i, j) = (ix as i32, iy as i32);
    let top = hash(i, j) + (hash(i + 1, j) - hash(i, j)) * sx;
    let bottom = hash(i, j + 1) + (hash(i + 1, j + 1) - hash(i, j + 1)) * sx;
    top + (bottom - top) * sy
}

/// Nuages : le dessin assombri ou éclairci par une texture de nuages de `scale` pixels, de
/// `amount` (0 à 100 %) ; `seed` change les nuages.
pub fn clouds(raster: &Raster, scale: f32, amount: f32, seed: f32) -> Raster {
    let s = scale.max(2.0);
    let q = (amount / 100.0).clamp(0.0, 1.0);
    let seed = seed.round() as u32;
    shade(raster, |x, y| {
        // Quatre octaves, chacune deux fois plus fine et moitié moins forte.
        let (mut n, mut w, mut f, mut total) = (0.0, 1.0, 1.0 / s, 0.0);
        for o in 0..4 {
            n += value_noise(x * f, y * f, seed.wrapping_add(o)) * w;
            total += w;
            w *= 0.5;
            f *= 2.0;
        }
        1.0 - q + q * (0.4 + 1.2 * n / total)
    })
}

/// Éclairage : un spot de lumière au centre (`cx`, `cy` en %), de rayon `radius` (% du
/// demi-côté) ; le dessin s'éclaire au centre et s'assombrit au-delà, de `intensity` (0 à
/// 100 %).
pub fn lighting(raster: &Raster, cx: f32, cy: f32, radius: f32, intensity: f32) -> Raster {
    let (ox, oy, half) = center(raster, cx, cy);
    let r = (half * radius / 100.0).max(1.0);
    let i = (intensity / 100.0).clamp(0.0, 1.0);
    shade(raster, |x, y| {
        let d = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt() / r;
        let fall = (1.0 - d).clamp(0.0, 1.0);
        let fall = fall * fall * (3.0 - 2.0 * fall);
        1.0 + i * (2.0 * fall - 1.0)
    })
}

/// Dégradé : le dessin fonce vers un côté de la feuille (direction `angle`, 0° : vers la
/// droite), jusqu'à `strength` (0 à 100 %) au bord.
pub fn gradient(raster: &Raster, angle: f32, strength: f32) -> Raster {
    let (w, h) = (raster.width as f32, raster.height as f32);
    let (s, c) = angle.to_radians().sin_cos();
    let extent = (w * c.abs() + h * s.abs()).max(1.0) / 2.0;
    let k = (strength / 100.0).clamp(0.0, 1.0);
    shade(raster, |x, y| {
        let t = (((x - w / 2.0) * c + (y - h / 2.0) * s) / extent + 1.0) / 2.0;
        let t = t.clamp(0.0, 1.0);
        1.0 - k * t * t * (3.0 - 2.0 * t)
    })
}
