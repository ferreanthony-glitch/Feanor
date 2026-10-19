//! Effets « Cinéma » de la pile d'effets : ceux dont les préréglages d'effets (`prereglages.rs`)
//! ont besoin pour agir sur TOUTE l'image d'un calque d'effet (`calqueffet.rs`) : flash, image
//! d'impact, lueur diffuse, secousse de l'image, vignettage, grain de film, aberration
//! chromatique. Ce sont des effets ordinaires de la pile (réglables, animables par clés) ; ils
//! servent aussi sur un calque de dessin.
//!
//! Contrairement aux effets d'un calque de dessin (« les pixels vides restent vides »), ceux-ci
//! remplissent toute la feuille quand c'est leur sens (un flash couvre l'image entière, même là
//! où rien n'est dessiné). Les valeurs en pixels sont celles de la feuille ; l'aperçu réduit
//! (`fxproxy.rs`) les ramène à l'échelle.
//!
//! Tout est déterministe : le hasard (rayons de l'image d'impact, grain) ne dépend que de la
//! graine, du pixel et, pour le grain, de l'image.

use eframe::egui::Color32;

use crate::fxfilters::{premul, unpremul};
use crate::raster::{empty_tile, tile_bounds, Raster, TILE, TILE_PIXELS};

const TAU: f32 = std::f32::consts::TAU;

/// La feuille entière (toutes les tuiles posées), chaque pixel (x, y) calculé par `f` d'après le
/// pixel de `raster`.
fn dense(raster: &Raster, f: impl Fn(usize, usize, Color32) -> Color32 + Sync) -> Raster {
    let mut out = Raster::new(raster.width, raster.height);
    let (w, h) = (out.width, out.height);
    let tiles_x = out.tiles_x.max(1);
    crate::render::par_chunks(&mut out.tiles, tiles_x, |k, row| {
        for (tx, slot) in row.iter_mut().enumerate() {
            let idx = k * tiles_x + tx;
            let (ox, oy, tw, th) = tile_bounds(w, h, idx);
            let src = raster.tiles.get(idx).and_then(Option::as_ref);
            let mut tile = empty_tile();
            {
                let px: &mut [Color32; TILE_PIXELS] = &mut tile;
                for y in 0..th {
                    for x in 0..tw {
                        let s = src.map_or(Color32::TRANSPARENT, |t| t[y * TILE + x]);
                        px[y * TILE + x] = f(ox + x, oy + y, s);
                    }
                }
            }
            *slot = Some(tile);
        }
    });
    out
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn luma(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// Un nombre dans [0, 1[ qui ne dépend que de `a`, `b` et `c`.
pub(crate) fn hash01(a: u32, b: u32, c: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

// ---------------------------------------------------------------------- flash

/// Flash : la couleur `rgb` (0 à 255) posée sur toute l'image à `amount` % ; `screen` : en écran
/// (éclaircit seulement) plutôt qu'en fondu.
pub fn flash(raster: &Raster, amount: f32, rgb: [f32; 3], screen: bool) -> Raster {
    let a = (amount / 100.0).clamp(0.0, 1.0);
    let col = rgb.map(|v| (v / 255.0).clamp(0.0, 1.0));
    dense(raster, move |_, _, p| {
        if screen {
            let (c, cov) = unpremul(p);
            let out = [0, 1, 2].map(|k| 1.0 - (1.0 - c[k]) * (1.0 - col[k] * a));
            // Là où rien n'est dessiné, l'écran pose la couleur.
            let cov2 = cov + a * (1.0 - cov);
            let base = [0, 1, 2].map(|k| if cov > 0.0 { out[k] } else { col[k] });
            premul(base, cov2)
        } else {
            let k = 1.0 - a;
            let ch = |v: u8, c: f32| ((v as f32 * k + c * 255.0 * a) + 0.5).clamp(0.0, 255.0) as u8;
            let alpha = (p.a() as f32 * k + 255.0 * a + 0.5).clamp(0.0, 255.0) as u8;
            Color32::from_rgba_premultiplied(ch(p.r(), col[0]).min(alpha), ch(p.g(), col[1]).min(alpha), ch(p.b(), col[2]).min(alpha), alpha)
        }
    })
}

// ---------------------------------------------------------------------- image d'impact

/// Réglages de l'image d'impact (rangs de `fx::IMPACT`).
pub const IMPACT_AMOUNT: usize = 0;
pub const IMPACT_MODE: usize = 1;
pub const IMPACT_THRESHOLD: usize = 2;
pub const IMPACT_TINT: usize = 3; // 3, 4, 5 : rouge, vert, bleu
pub const IMPACT_TINT_FORCE: usize = 6;
pub const IMPACT_BURST: usize = 7;
pub const IMPACT_LINES: usize = 8;
pub const IMPACT_CX: usize = 9;
pub const IMPACT_CY: usize = 10;
pub const IMPACT_SEED: usize = 11;

/// Image d'impact : l'image devient très contrastée, négative ou en noir et blanc, teintée en
/// option, avec un éclat de rayons (burst) et des lignes de vitesse tracés au calcul. `v` : les
/// réglages (voir `IMPACT_*`).
pub fn impact(raster: &Raster, v: &[f32]) -> Raster {
    let at = |k: usize| v.get(k).copied().unwrap_or(0.0);
    let amount = (at(IMPACT_AMOUNT) / 100.0).clamp(0.0, 1.0);
    let mode = at(IMPACT_MODE).round() as i32;
    let thr = at(IMPACT_THRESHOLD) / 255.0;
    let tint = [at(IMPACT_TINT), at(IMPACT_TINT + 1), at(IMPACT_TINT + 2)].map(|c| (c / 255.0).clamp(0.0, 1.0));
    let tint_force = (at(IMPACT_TINT_FORCE) / 100.0).clamp(0.0, 1.0);
    let burst = (at(IMPACT_BURST) / 100.0).clamp(0.0, 1.0);
    let lines = (at(IMPACT_LINES) / 100.0).clamp(0.0, 1.0);
    let (w, h) = (raster.width as f32, raster.height as f32);
    let (cx, cy) = (w * at(IMPACT_CX) / 100.0, h * at(IMPACT_CY) / 100.0);
    let seed = at(IMPACT_SEED).max(0.0) as u32;
    let half = (w.min(h) / 2.0).max(1.0);
    // L'éclat est blanc sur une image sombre, noir sur une image claire : le contraire du fond.
    dense(raster, move |x, y, p| {
        let (c, cov) = unpremul(p);
        // Posée sur du blanc : un fond vide est du papier.
        let base = [0, 1, 2].map(|k| c[k] * cov + (1.0 - cov));
        let l = luma(base);
        let mut out = match mode {
            0 => base.map(|v| 1.0 - v),
            1 => [if l >= thr { 1.0 } else { 0.0 }; 3],
            2 => [if l >= thr { 0.0 } else { 1.0 }; 3],
            _ => base.map(|v| ((v * 2.0).round() / 2.0).clamp(0.0, 1.0)),
        };
        if tint_force > 0.0 {
            let lo = luma(out);
            out = [0, 1, 2].map(|k| out[k] + (tint[k] * lo - out[k]) * tint_force);
        }
        let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
        let dist = (px * px + py * py).sqrt();
        let mut mark = 0.0f32;
        if burst > 0.0 {
            mark = mark.max(burst * burst_mask(px, py, dist, half, seed));
        }
        if lines > 0.0 {
            mark = mark.max(lines * lines_mask(x, y, w, h, (px, py), seed));
        }
        if mark > 0.0 {
            let ink = if luma(out) > 0.5 { 0.0 } else { 1.0 };
            out = out.map(|v| v + (ink - v) * mark.clamp(0.0, 1.0));
        }
        let mixed = [0, 1, 2].map(|k| base[k] + (out[k] - base[k]) * amount);
        premul(mixed, 1.0)
    })
}

/// Rayons d'un éclat, partant du centre : largeur de chaque rayon tirée de la graine, bords doux,
/// visibles à partir d'un peu de distance du centre.
fn burst_mask(px: f32, py: f32, dist: f32, half: f32, seed: u32) -> f32 {
    const RAYS: f32 = 36.0;
    let turn = (py.atan2(px) / TAU + 1.0).fract();
    let k = turn * RAYS;
    let (ray, f) = (k.floor() as u32, k.fract());
    let width = 0.25 + 0.65 * hash01(seed, ray, 11);
    let edge = ((0.5 - (f - 0.5).abs()) / (0.5 * width).max(1e-3)).clamp(0.0, 1.0);
    // (Un rayon sur trois est éteint : l'éclat n'est pas une roue.)
    let alive = if hash01(seed, ray, 29) < 0.72 { 1.0 } else { 0.0 };
    alive * smoothstep(0.0, 1.0, edge) * smoothstep(0.18, 0.55, dist / half)
}

/// Lignes de vitesse : traits horizontaux qui s'allongent vers les bords de l'image.
fn lines_mask(x: usize, y: usize, w: f32, h: f32, (px, _py): (f32, f32), seed: u32) -> f32 {
    let bands = 70.0;
    let band = ((y as f32 / h.max(1.0)) * bands).floor() as u32;
    let fy = (y as f32 / h.max(1.0) * bands).fract();
    if hash01(seed, band, 53) > 0.45 {
        return 0.0;
    }
    let thick = 0.15 + 0.5 * hash01(seed, band, 71);
    let across = (1.0 - ((fy - 0.5).abs() / (0.5 * thick).max(1e-3))).clamp(0.0, 1.0);
    // Plus long vers l'extérieur, d'une longueur propre à chaque ligne.
    let start = 0.15 + 0.35 * hash01(seed, band, 97);
    let along = smoothstep(start, start + 0.12, px.abs() / (w / 2.0).max(1.0));
    let _ = x;
    across * along
}

// ---------------------------------------------------------------------- lueur diffuse

/// Niveau de réduction (puissance de deux) pour flouter un rayon de `radius` pixels : le flou
/// large se fait sur une image réduite, puis agrandie (une lueur n'a pas de détail fin).
pub(crate) fn blur_level(radius: f32) -> u8 {
    if radius >= 80.0 {
        3
    } else if radius >= 30.0 {
        2
    } else if radius >= 12.0 {
        1
    } else {
        0
    }
}

/// Les pixels d'un dessin en un seul tableau, pour les lire au hasard sans passer par les tuiles
/// (une lecture bilinéaire par pixel de la feuille, plusieurs fois : la différence se compte en
/// dizaines de millisecondes sur une feuille 4K).
struct Flat {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

impl Flat {
    fn of(r: &Raster) -> Flat {
        Flat { w: r.width, h: r.height, px: r.to_flat() }
    }

    /// Comme `fxwarp::sample` : le point (x, y) en pixels (centres à +0,5), lu entre ses quatre pixels
    /// voisins, prémultiplié ; vide hors du dessin.
    #[inline(always)]
    fn sample(&self, x: f32, y: f32) -> [f32; 4] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        if ix < -1.0 || iy < -1.0 || ix >= self.w as f32 || iy >= self.h as f32 {
            return [0.0; 4];
        }
        let (ix, iy) = (ix as i64, iy as i64);
        // Au milieu du dessin : quatre lectures directes, sans contrôle de bord.
        if ix >= 0 && iy >= 0 && (ix as usize) + 1 < self.w && (iy as usize) + 1 < self.h {
            let i = iy as usize * self.w + ix as usize;
            let (a, b, c, d) = (self.px[i].to_array(), self.px[i + 1].to_array(), self.px[i + self.w].to_array(), self.px[i + self.w + 1].to_array());
            let (w00, w10, w01, w11) = ((1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty);
            return std::array::from_fn(|k| a[k] as f32 * w00 + b[k] as f32 * w10 + c[k] as f32 * w01 + d[k] as f32 * w11);
        }
        let get = |x: i64, y: i64| -> [f32; 4] {
            if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
                return [0.0; 4];
            }
            let p = self.px[y as usize * self.w + x as usize];
            [p.r() as f32, p.g() as f32, p.b() as f32, p.a() as f32]
        };
        let (a, b, c, d) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            top + (bottom - top) * ty
        })
    }
}

/// Lueur diffuse : ce qui dépasse `threshold` (0 à 255 de luminance) est flouté de `radius` pixels
/// et ajouté à l'image en écran, à `intensity` %.
pub fn bloom(raster: &Raster, radius: f32, intensity: f32, threshold: f32) -> Raster {
    let level = blur_level(radius);
    let small = crate::fxproxy::downscale(raster, level);
    let thr = (threshold / 255.0).clamp(0.0, 0.99);
    // Ce qui brille : la luminance au-dessus du seuil, étirée sur le reste de l'échelle.
    let bright = dense(&small, move |_, _, p| {
        let (c, cov) = unpremul(p);
        if cov <= 0.0 {
            return Color32::TRANSPARENT;
        }
        let k = ((luma(c) - thr) / (1.0 - thr)).clamp(0.0, 1.0);
        premul(c.map(|v| v * k), cov * k)
    });
    // Le halo reste à la taille réduite : il est lu en bilinéaire pendant qu'on l'ajoute à l'image.
    let halo = Flat::of(&crate::fx::gaussian(&bright, (radius / (1usize << level) as f32).max(1.0)));
    let f = (1usize << level) as f32;
    let k = (intensity / 100.0).max(0.0);
    dense(raster, move |x, y, p| {
        let g = halo.sample((x as f32 + 0.5) / f, (y as f32 + 0.5) / f);
        if g[3] < 0.5 {
            return p;
        }
        let add = |s: u8, g: f32| {
            let (s, g) = (s as f32 / 255.0, (g / 255.0 * k).min(1.0));
            ((1.0 - (1.0 - s) * (1.0 - g)) * 255.0 + 0.5) as u8
        };
        let a = add(p.a(), g[3]);
        Color32::from_rgba_premultiplied(add(p.r(), g[0]).min(a), add(p.g(), g[1]).min(a), add(p.b(), g[2]).min(a), a)
    })
}

// ---------------------------------------------------------------------- secousse de l'image

/// Secousse de l'image : l'image entière décalée de (`dx`, `dy`) pixels, tournée de `angle`
/// degrés et agrandie de `zoom` % autour du centre de la feuille ; `blur` (0 à 100 %) la floute
/// dans le sens du décalage (flou de bougé, sur la longueur du décalage).
pub fn shake(raster: &Raster, dx: f32, dy: f32, angle: f32, zoom: f32, blur: f32) -> Raster {
    let (w, h) = (raster.width as f32, raster.height as f32);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let z = (zoom / 100.0).max(0.05);
    let (s, c) = (-angle).to_radians().sin_cos();
    let len = (dx * dx + dy * dy).sqrt();
    let spread = (blur / 100.0).clamp(0.0, 1.0);
    // (Six lectures au plus : le flou de bougé n'a pas besoin de plus pour paraître continu.)
    let n = if spread > 0.0 && len >= 1.0 { ((len * spread / 3.0).ceil() as usize + 1).clamp(2, 6) } else { 1 };
    if raster.tiles.iter().all(Option::is_none) {
        return raster.clone();
    }
    let flat = Flat::of(raster);
    let (iw, ih) = (flat.w, flat.h);
    let mut out = vec![Color32::TRANSPARENT; iw * ih];
    crate::render::par_chunks(&mut out, iw * 8, |chunk, rows| {
        for (j, row) in rows.chunks_mut(iw).enumerate() {
            let y = (chunk * 8 + j) as f32 + 0.5;
            for (x, o) in row.iter_mut().enumerate() {
                let mut acc = [0.0f32; 4];
                for i in 0..n {
                    // Le décalage balaye de (1 - flou / 2) à (1 + flou / 2) de sa valeur.
                    let m = if n == 1 { 1.0 } else { 1.0 + spread * (i as f32 / (n - 1) as f32 - 0.5) };
                    let (qx, qy) = (x as f32 + 0.5 - cx - dx * m, y - cy - dy * m);
                    let (rx, ry) = ((qx * c - qy * s) / z, (qx * s + qy * c) / z);
                    let p = flat.sample(rx + cx, ry + cy);
                    for k in 0..4 {
                        acc[k] += p[k];
                    }
                }
                let inv = 1.0 / n as f32;
                let a = (acc[3] * inv).round().clamp(0.0, 255.0);
                let ch = |v: f32| (v * inv).round().clamp(0.0, a) as u8;
                *o = Color32::from_rgba_premultiplied(ch(acc[0]), ch(acc[1]), ch(acc[2]), a as u8);
            }
        }
    });
    crate::fx::into_raster(&crate::fx::Region { x0: 0, y0: 0, w: iw, h: ih, px: out }, raster)
}

// ---------------------------------------------------------------------- vignettage

/// Vignettage : l'image s'assombrit vers les bords d'une ellipse centrée en (`cx`, `cy`) % de la
/// feuille et large de `fw` × `fh` % d'elle (le cadre de la caméra, pour les préréglages). Le noir
/// commence à `radius` % du demi-axe et pleinement atteint `softness` % plus loin.
pub fn vignette(raster: &Raster, amount: f32, radius: f32, softness: f32, cx: f32, cy: f32, fw: f32, fh: f32) -> Raster {
    let (w, h) = (raster.width as f32, raster.height as f32);
    let a = (amount / 100.0).clamp(0.0, 1.0);
    let (ox, oy) = (w * cx / 100.0, h * cy / 100.0);
    let (rx, ry) = ((w * fw / 200.0).max(1.0), (h * fh / 200.0).max(1.0));
    let r0 = radius / 100.0;
    let r1 = r0 + (softness / 100.0).max(0.01);
    dense(raster, move |x, y, p| {
        let (u, v) = ((x as f32 + 0.5 - ox) / rx, (y as f32 + 0.5 - oy) / ry);
        let d = (u * u + v * v).sqrt();
        let k = 1.0 - a * smoothstep(r0, r1, d);
        if k >= 0.999 {
            return p;
        }
        let ch = |c: u8| (c as f32 * k + 0.5) as u8;
        Color32::from_rgba_premultiplied(ch(p.r()), ch(p.g()), ch(p.b()), p.a())
    })
}

// ---------------------------------------------------------------------- grain de film

/// Grain de film : un hasard par grain de `size` pixels, différent à chaque image `frame` et à
/// chaque graine, ajouté à la luminance de toute l'image (fort dans les tons moyens). `level` :
/// aperçu réduit `1 << level` fois (le grain garde sa taille à l'écran).
pub fn grain(raster: &Raster, amount: f32, size: f32, seed: f32, frame: f32, level: u8) -> Raster {
    let k = (amount / 100.0).clamp(0.0, 1.0) * 0.5;
    let f = 1usize << level;
    let cell = ((size.round().max(1.0) as usize) / f).max(1);
    let (seed, frame) = (seed.max(0.0) as u32, frame.max(0.0) as u32);
    dense(raster, move |x, y, p| {
        let a = p.a();
        if a == 0 {
            return p;
        }
        let n = hash01((x / cell) as u32 ^ (seed << 16), (y / cell) as u32, frame.wrapping_mul(7919).wrapping_add(seed)) * 2.0 - 1.0;
        let (c, cov) = unpremul(p);
        let mid = 1.0 - (2.0 * luma(c) - 1.0).abs().powi(2) * 0.6;
        premul(c.map(|v| v + n * k * mid), cov)
    })
}

// ---------------------------------------------------------------------- aberration chromatique

/// Aberration chromatique : le rouge s'écarte du centre (`cx`, `cy` en %) et le bleu s'en
/// rapproche, de `amount` pixels aux bords de la feuille, le vert reste en place.
pub fn aberration(raster: &Raster, amount: f32, cx: f32, cy: f32) -> Raster {
    let (w, h) = (raster.width as f32, raster.height as f32);
    let (ox, oy) = (w * cx / 100.0, h * cy / 100.0);
    let reach = ((w * w + h * h).sqrt() / 2.0).max(1.0);
    let k = amount / reach;
    let flat = Flat::of(raster);
    dense(raster, move |x, y, _| {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (dx, dy) = (px - ox, py - oy);
        // (Le rouge qu'on voit ici vient d'un point plus proche du centre : son image est plus loin.)
        let r = flat.sample(px - dx * k, py - dy * k);
        let g = flat.sample(px, py);
        let b = flat.sample(px + dx * k, py + dy * k);
        let a = r[3].max(g[3]).max(b[3]).round().clamp(0.0, 255.0);
        let ch = |v: f32| v.round().clamp(0.0, a) as u8;
        Color32::from_rgba_premultiplied(ch(r[0]), ch(g[1]), ch(b[2]), a as u8)
    })
}
