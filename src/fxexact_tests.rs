//! Les calculs d'effets accélérés (flou en boîte parallèle, transposition par bandes, effets par
//! pixel répartis sur les cœurs) rendent exactement la même image que l'ancien calcul, pixel pour
//! pixel : l'export ne change pas d'un octet.

use eframe::egui::Color32;

use crate::fx::{self, EffectKind};
use crate::raster::Raster;

/// Le flou d'avant ce chantier (copie du code d'alors), sans parallélisme : la référence.
mod old {
    use super::*;
    use crate::fx::Region;
    use crate::raster::TILE;

    fn region_of(raster: &Raster, margin: usize) -> Option<Region> {
        let mut lo = (usize::MAX, usize::MAX);
        let mut hi = (0, 0);
        for (idx, t) in raster.tiles.iter().enumerate() {
            if t.is_some() {
                let (ox, oy, tw, th) = raster.tile_bounds(idx);
                lo = (lo.0.min(ox), lo.1.min(oy));
                hi = (hi.0.max(ox + tw), hi.1.max(oy + th));
            }
        }
        if lo.0 == usize::MAX {
            return None;
        }
        let (x0, y0) = (lo.0.saturating_sub(margin), lo.1.saturating_sub(margin));
        let (x1, y1) = ((hi.0 + margin).min(raster.width), (hi.1 + margin).min(raster.height));
        let (w, h) = (x1 - x0, y1 - y0);
        let mut px = vec![Color32::TRANSPARENT; w * h];
        for (y, row) in px.chunks_mut(w).enumerate() {
            for (x, p) in row.iter_mut().enumerate() {
                *p = raster.get(x0 + x, y0 + y);
            }
        }
        Some(Region { x0, y0, w, h, px })
    }

    fn into_raster(r: &Region, like: &Raster) -> Raster {
        let mut out = Raster::new(like.width, like.height);
        let (tx0, ty0) = (r.x0 / TILE, r.y0 / TILE);
        let (tx1, ty1) = ((r.x0 + r.w).div_ceil(TILE), (r.y0 + r.h).div_ceil(TILE));
        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                let idx = ty * out.tiles_x + tx;
                let (ox, oy, tw, th) = out.tile_bounds(idx);
                let mut any = false;
                let mut tile = crate::raster::empty_tile();
                for y in 0..th {
                    for x in 0..tw {
                        let (sx, sy) = (ox + x, oy + y);
                        if sx < r.x0 || sy < r.y0 || sx >= r.x0 + r.w || sy >= r.y0 + r.h {
                            continue;
                        }
                        let c = r.px[(sy - r.y0) * r.w + (sx - r.x0)];
                        if c.a() != 0 {
                            any = true;
                            tile[y * TILE + x] = c;
                        }
                    }
                }
                if any {
                    out.tiles[idx] = Some(tile);
                }
            }
        }
        out
    }

    fn box_rows(px: &mut [Color32], w: usize, r: usize) {
        if r == 0 || w == 0 {
            return;
        }
        let size = (2 * r + 1) as u32;
        let mut line = vec![Color32::TRANSPARENT; w];
        for row in px.chunks_mut(w) {
            line.copy_from_slice(row);
            let mut acc = [0u32; 4];
            let add = |acc: &mut [u32; 4], c: Color32, sign: bool| {
                for (k, v) in c.to_array().iter().enumerate() {
                    if sign {
                        acc[k] += *v as u32;
                    } else {
                        acc[k] -= *v as u32;
                    }
                }
            };
            for c in line.iter().take(r.min(w - 1) + 1) {
                add(&mut acc, *c, true);
            }
            for x in 0..w {
                let m = |v: u32| ((v + size / 2) / size) as u8;
                row[x] = Color32::from_rgba_premultiplied(m(acc[0]), m(acc[1]), m(acc[2]), m(acc[3]));
                if x + r + 1 < w {
                    add(&mut acc, line[x + r + 1], true);
                }
                if x >= r {
                    add(&mut acc, line[x - r], false);
                }
            }
        }
    }

    fn transpose(px: &[Color32], w: usize, h: usize) -> Vec<Color32> {
        let mut out = vec![Color32::TRANSPARENT; w * h];
        for y in 0..h {
            for x in 0..w {
                out[x * h + y] = px[y * w + x];
            }
        }
        out
    }

    fn boxes_for(sigma: f32) -> [usize; 3] {
        let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
        let mut wl = ideal.floor() as i64;
        if wl % 2 == 0 {
            wl -= 1;
        }
        let wu = wl + 2;
        let m = ((12.0 * sigma * sigma - 3.0 * (wl * wl) as f32 - 12.0 * wl as f32 - 9.0) / (-4.0 * wl as f32 - 4.0)).round() as i64;
        std::array::from_fn(|i| (((if (i as i64) < m { wl } else { wu }) - 1) / 2).max(0) as usize)
    }

    pub fn gaussian(raster: &Raster, radius: f32) -> Raster {
        let margin = radius.ceil() as usize + 2;
        let Some(mut r) = region_of(raster, margin) else { return Raster::new(raster.width, raster.height) };
        let boxes = boxes_for(radius / 3.0);
        for b in boxes {
            box_rows(&mut r.px, r.w, b);
        }
        let mut t = transpose(&r.px, r.w, r.h);
        for b in boxes {
            box_rows(&mut t, r.h, b);
        }
        r.px = transpose(&t, r.h, r.w);
        into_raster(&r, raster)
    }
}

/// Un dessin de test : des tuiles de pixels (couleurs prémultipliées valides) séparées de tuiles
/// vides, dans une feuille dont les bords ne tombent pas sur des tuiles entières.
pub(crate) fn random_raster(w: usize, h: usize, seed: u32) -> Raster {
    let mut s = seed.wrapping_mul(2654435761).wrapping_add(12345);
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    let mut r = Raster::new(w, h);
    for idx in 0..r.tiles.len() {
        if next() % 3 == 0 {
            continue;
        }
        let (_, _, tw, th) = r.tile_bounds(idx);
        let tile = r.tile_mut(idx);
        for y in 0..th {
            for x in 0..tw {
                let v = next();
                if v % 5 == 0 {
                    continue;
                }
                let a = (v >> 8) as u8;
                let ch = |q: u32| ((q & 0xFF) * a as u32 / 255) as u8;
                tile[y * crate::raster::TILE + x] = Color32::from_rgba_premultiplied(ch(next()), ch(next()), ch(next()), a);
            }
        }
    }
    r
}

/// Nombre de pixels différents entre deux dessins de même taille.
pub(crate) fn differing(a: &Raster, b: &Raster) -> usize {
    assert_eq!((a.width, a.height), (b.width, b.height));
    (0..a.height).flat_map(|y| (0..a.width).map(move |x| (x, y))).filter(|&(x, y)| a.get(x, y) != b.get(x, y)).count()
}

/// Empreinte (FNV-1a) de tous les octets d'un dessin.
fn fingerprint(r: &Raster) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for y in 0..r.height {
        for x in 0..r.width {
            for b in r.get(x, y).to_array() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    h
}

/// Les 21 effets sur un même dessin : les empreintes ont été relevées avec le code d'avant ce
/// chantier (flous sans parallélisme, boucles pixel par pixel) ; l'export donne les mêmes pixels.
#[test]
fn every_effect_gives_the_same_pixels_as_before_the_speed_up() {
    let r = random_raster(333, 211, 11);
    let table: [(EffectKind, &[f32], u64); 21] = [
        (EffectKind::GaussianBlur, &[17.0], 0xbb92b95dcc0bdd10),
        (EffectKind::MotionBlur, &[21.0, 33.0], 0xebdc396c3a730ea6),
        (EffectKind::BrightnessContrast, &[35.0, -25.0], 0x0483fb9d31173283),
        (EffectKind::Levels, &[20.0, 230.0, 1.6], 0x6dc47c189a6323a1),
        (EffectKind::HueSaturation, &[40.0, -35.0, 12.0], 0x0540b56dab5191bc),
        (EffectKind::BlackWhite, &[80.0], 0xde182e107f7a58c7),
        (EffectKind::Invert, &[70.0], 0xd69468cbce889438),
        (EffectKind::Posterize, &[5.0], 0xb9c7799c50cfee87),
        (EffectKind::Threshold, &[100.0], 0x148ca0af93ecfb88),
        (EffectKind::Sharpen, &[150.0, 3.0], 0x424520d22aecb8d0),
        (EffectKind::Noise, &[45.0], 0x6179faa3e420588e),
        (EffectKind::Mosaic, &[9.0], 0xf2ca30552e2e2f25),
        (EffectKind::DropShadow, &[14.0, -9.0, 5.0, 70.0], 0x26c88686a9b51066),
        (EffectKind::Glow, &[11.0, 120.0], 0x60a3b35421b535a8),
        (EffectKind::Wave, &[8.0, 90.0, 30.0], 0x2d008b3e01d24a52),
        (EffectKind::Ripple, &[6.0, 50.0, 40.0, 60.0], 0xb67ca045b355d03a),
        (EffectKind::Twirl, &[120.0, 70.0, 50.0, 50.0], 0xb6e9fa6425ad8788),
        (EffectKind::Pinch, &[60.0, 60.0, 45.0, 55.0], 0x0247807b75a50b4a),
        (EffectKind::Clouds, &[40.0, 70.0, 7.0], 0xbee97012fe96629b),
        (EffectKind::Lighting, &[35.0, 40.0, 100.0, 60.0], 0xea52b7789eafda25),
        (EffectKind::Gradient, &[30.0, 70.0], 0x2bfa10c1c29357b4),
    ];
    for (kind, values, want) in table {
        let out = fx::apply(&r, &[fx::FxStep { kind, values: values.to_vec() }]);
        assert_eq!(fingerprint(&out), want, "{kind:?}");
    }
    let stack = [
        fx::FxStep { kind: EffectKind::GaussianBlur, values: vec![9.0] },
        fx::FxStep { kind: EffectKind::HueSaturation, values: vec![10.0, -80.0, 0.0] },
    ];
    assert_eq!(fingerprint(&fx::apply(&r, &stack)), 0xf4c55383d7e990be, "pile de deux effets");
}

#[test]
fn the_box_division_by_multiplication_is_exact() {
    for size in (3..=801).step_by(2) {
        assert!(fx::box_div_exact(size), "taille {size}");
    }
}

#[test]
fn the_new_gaussian_blur_is_identical_to_the_old_one() {
    let r = random_raster(333, 211, 7);
    for radius in [0.5, 1.0, 2.7, 8.0, 31.0, 130.0, 200.0] {
        let new = fx::apply(&r, &[fx::FxStep { kind: EffectKind::GaussianBlur, values: vec![radius] }]);
        let old = old::gaussian(&r, radius);
        assert_eq!(differing(&new, &old), 0, "rayon {radius}");
    }
    // Un dessin vide et un dessin minuscule.
    let empty = Raster::new(100, 80);
    assert_eq!(differing(&fx::gaussian(&empty, 9.0), &old::gaussian(&empty, 9.0)), 0);
    let mut dot = Raster::new(130, 70);
    dot.tile_mut(0)[5] = Color32::BLACK;
    assert_eq!(differing(&fx::gaussian(&dot, 12.0), &old::gaussian(&dot, 12.0)), 0);
}

#[test]
fn per_pixel_effects_do_not_depend_on_how_the_work_is_split() {
    let r = random_raster(333, 211, 3);
    let f = |c: [f32; 3], x: usize, y: usize| c.map(|v| v * 0.5 + (x * 7 + y * 13) as f32 / 4000.0);
    let new = crate::fxfilters::map_colors(&r, f);
    let mut old = r.clone();
    for idx in 0..old.tiles.len() {
        if old.tiles[idx].is_none() {
            continue;
        }
        let (ox, oy, tw, th) = old.tile_bounds(idx);
        let tile = old.tile_mut(idx);
        for y in 0..th {
            for x in 0..tw {
                let i = y * crate::raster::TILE + x;
                let (rgb, a) = crate::fxfilters::unpremul(tile[i]);
                if a > 0.0 {
                    tile[i] = crate::fxfilters::premul(f(rgb, ox + x, oy + y), a);
                }
            }
        }
    }
    assert_eq!(differing(&new, &old), 0);
}
