//! Pinceaux texturés (3d-2) : une pointe en image (niveaux de gris) et le grain du papier, comme
//! un crayon ou une craie sur un papier à grain. Rendu visé : celui de TVPaint (choix d'Anthony),
//! dont on reprend l'idée d'un grain fixe sur la feuille — sans en connaître le calcul exact.
//!
//! Le grain ne suit pas le trait : il est attaché au dessin, si bien que repasser au même endroit
//! retrouve les mêmes creux. Appuyer plus fort fait entrer la couleur dans les creux.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::brush::BrushSettings;

/// Image en niveaux de gris, valeurs de 0 à 1. Pointe : quantité de couleur déposée (1 = pleine).
/// Papier : relief (1 = sommet, qui prend la couleur en premier).
#[derive(Clone, Debug, PartialEq)]
pub struct GrayImage {
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
}

impl GrayImage {
    pub fn new(width: usize, height: usize, values: Vec<f32>) -> GrayImage {
        assert_eq!(values.len(), width * height, "taille de l'image");
        GrayImage { width, height, values }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.width + x]
    }

    /// Valeur au point (`x`, `y`) en texels, interpolée ; hors de l'image : 0.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let get = |x: i64, y: i64| {
            if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
                0.0
            } else {
                self.values[y as usize * self.width + x as usize]
            }
        };
        let top = get(ix, iy) + (get(ix + 1, iy) - get(ix, iy)) * tx;
        let bottom = get(ix, iy + 1) + (get(ix + 1, iy + 1) - get(ix, iy + 1)) * tx;
        top + (bottom - top) * ty
    }

    /// Valeur répétée en motif (papier).
    pub fn tiled(&self, x: usize, y: usize) -> f32 {
        self.values[(y % self.height) * self.width + x % self.width]
    }

    /// Comme `tiled`, pour des coordonnées qui peuvent être négatives (hors de la feuille).
    pub fn tiled_at(&self, x: i64, y: i64) -> f32 {
        self.values[y.rem_euclid(self.height as i64) as usize * self.width + x.rem_euclid(self.width as i64) as usize]
    }

    /// Valeur interpolée au point (`x`, `y`) de l'image répétée sans fin (papier mis à
    /// l'échelle ou tourné).
    pub fn sample_wrapped(&self, x: f32, y: f32) -> f32 {
        let (w, h) = (self.width as i64, self.height as i64);
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let get = |x: i64, y: i64| self.values[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize];
        let top = get(ix, iy) + (get(ix + 1, iy) - get(ix, iy)) * tx;
        let bottom = get(ix, iy + 1) + (get(ix + 1, iy + 1) - get(ix, iy + 1)) * tx;
        top + (bottom - top) * ty
    }

    /// Une pointe importée d'une image, réduite à `max` pixels de côté au plus : 1 où elle dépose.
    /// Une image transparente donne sa transparence ; une image opaque, son noir (le noir dépose).
    pub fn tip_from_image(img: &image::DynamicImage, max: u32) -> GrayImage {
        let img = if img.width() > max || img.height() > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img.clone() };
        let rgba = img.to_rgba8();
        let transparent = rgba.pixels().any(|p| p.0[3] < 250);
        let values = rgba
            .pixels()
            .map(|p| {
                if transparent {
                    p.0[3] as f32 / 255.0
                } else {
                    1.0 - (0.2126 * p.0[0] as f32 + 0.7152 * p.0[1] as f32 + 0.0722 * p.0[2] as f32) / 255.0
                }
            })
            .collect();
        GrayImage::new(rgba.width() as usize, rgba.height() as usize, values)
    }

    /// Le gris d'une image (luminance, 0 à 1), réduite à `max` pixels de côté au plus : un
    /// papier importé.
    pub fn from_image(img: &image::DynamicImage, max: u32) -> GrayImage {
        let img = if img.width() > max || img.height() > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img.clone() };
        let g = img.to_luma8();
        GrayImage::new(g.width() as usize, g.height() as usize, g.pixels().map(|p| p.0[0] as f32 / 255.0).collect())
    }

    /// Image moitié moins grande (moyenne de 2 × 2 texels).
    pub fn half(&self) -> GrayImage {
        let (w, h) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut values = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let (x0, y0) = ((2 * x).min(self.width - 1), (2 * y).min(self.height - 1));
                let (x1, y1) = ((2 * x + 1).min(self.width - 1), (2 * y + 1).min(self.height - 1));
                values.push((self.get(x0, y0) + self.get(x1, y0) + self.get(x0, y1) + self.get(x1, y1)) / 4.0);
            }
        }
        GrayImage::new(w, h, values)
    }

    /// Valeurs étirées sur toute la plage 0..1 (un papier peu contrasté marque quand même).
    pub fn normalized(mut self) -> GrayImage {
        let lo = self.values.iter().copied().fold(f32::MAX, f32::min);
        let hi = self.values.iter().copied().fold(f32::MIN, f32::max);
        if hi - lo > 1e-6 {
            for v in &mut self.values {
                *v = (*v - lo) / (hi - lo);
            }
        }
        self
    }

    /// Chaque valeur remplacée par son rang (répartition uniforme de 0 à 1, les valeurs égales au
    /// même rang moyen), comme pour les papiers fabriqués par `procedural_paper`. Le seuil du
    /// grain (`grain_factor`) suppose cette répartition ; un papier importé — scanné ou
    /// photographié, presque tout son relief dans les gris moyens — passait sinon d'un coup de
    /// « rien » à « tout » entre deux appuis (audit du rendu, 2026-10-18, planche des papiers).
    pub fn equalized(mut self) -> GrayImage {
        let n = self.values.len();
        if n < 2 {
            return self;
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| self.values[a].total_cmp(&self.values[b]));
        let d = (n - 1) as f32;
        let mut ranked = vec![0.0f32; n];
        let mut i = 0;
        while i < n {
            let mut j = i;
            while j + 1 < n && self.values[order[j + 1]] == self.values[order[i]] {
                j += 1;
            }
            let r = (i + j) as f32 / 2.0 / d;
            for &k in &order[i..=j] {
                ranked[k] = r;
            }
            i = j + 1;
        }
        self.values = ranked;
        self
    }
}

/// Pointe en image, avec ses réductions successives : une pointe de quelques pixels prend une
/// version réduite (moyennée) plutôt que quelques texels isolés de la grande image.
#[derive(Debug)]
pub struct Tip {
    levels: Vec<GrayImage>,
    /// Dépôt moyen de l'image sur son carré (1 : une pointe pleine ; un amas de grains : 0,2 à 0,4).
    mean: f32,
}

impl Tip {
    pub fn new(image: GrayImage) -> Tip {
        let mean = image.values.iter().sum::<f32>() / image.values.len().max(1) as f32;
        let mut levels = vec![image];
        while levels.last().is_some_and(|l| l.width.min(l.height) > 4) {
            let next = levels.last().expect("au moins un niveau").half();
            levels.push(next);
        }
        Tip { levels, mean }
    }

    pub fn full(&self) -> &GrayImage {
        &self.levels[0]
    }

    /// Dépôt moyen de la pointe sur son carré : ce qu'une pointe clairsemée pose par empreinte
    /// par rapport à une pointe pleine (sert au débit qui s'accumule).
    pub fn mean_coverage(&self) -> f32 {
        self.mean
    }

    /// La plus petite réduction encore au moins aussi large que la pointe à l'écran.
    pub fn level(&self, diameter: f32) -> &GrayImage {
        self.levels
            .iter()
            .rev()
            .find(|l| l.width as f32 >= diameter)
            .unwrap_or(&self.levels[0])
    }
}

/// Part de la couleur qui reste sur un point du papier de relief `height` : les sommets d'abord,
/// les creux quand on appuie (`pressure`) ; `grain` = 0 : papier lisse, 1 : grain marqué.
/// `softness` (rendu des brosses, étape 2) : largeur de la transition entre un creux vide et un
/// sommet plein, en part du relief — 0,12 est le seuil presque net d'avant l'audit, 0,3 un grain
/// qui répond en douceur comme un vrai papier.
pub fn grain_factor(height: f32, pressure: f32, grain: f32, softness: f32) -> f32 {
    if grain <= 0.0 {
        return 1.0;
    }
    // Seuil : bas quand on appuie (seuls les creux profonds restent blancs), haut quand on
    // effleure (seuls les sommets prennent la couleur).
    let t = 0.15 + 0.6 * (1.0 - pressure.clamp(0.0, 1.0));
    let w = softness.clamp(0.02, 0.6);
    let fill = smoothstep(t - w, t + w, height);
    1.0 - grain.clamp(0.0, 1.0) * (1.0 - fill)
}

/// La douceur du grain d'avant l'audit (le seuil presque net) : la valeur par défaut, pour que les
/// anciens réglages ne changent pas.
pub const GRAIN_SOFTNESS_OLD: f32 = 0.12;
/// La douceur des brosses traditionnelles de Pelure.
pub const GRAIN_SOFTNESS_SOFT: f32 = 0.3;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Papier fabriqué, répété sans raccord : un bruit fin (le « grain » d'un papier à dessin),
/// adouci sur `blur` texels (plus = grain plus gros), puis réparti uniformément de 0 à 1 — si
/// bien qu'un seuil de 0,3 laisse 30 % de creux, quelle que soit la texture.
pub fn procedural_paper(size: usize, blur: usize, seed: u32) -> GrayImage {
    // Mélange de MurmurHash3 (`fmix32`) : pas de motif visible entre texels voisins.
    let fmix = |mut h: u32| {
        h ^= h >> 16;
        h = h.wrapping_mul(0x85EB_CA6B);
        h ^= h >> 13;
        h = h.wrapping_mul(0xC2B2_AE35);
        h ^ (h >> 16)
    };
    let hash = |x: usize, y: usize| -> f32 {
        let h = fmix(fmix(seed ^ (x as u32).wrapping_mul(0x9E37_79B1)) ^ (y as u32).wrapping_mul(0x85EB_CA77));
        (h >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut values: Vec<f32> = (0..size * size).map(|i| hash(i % size, i / size)).collect();
    // Flou en boîte, deux passes horizontales et verticales (proche d'un flou gaussien), en
    // bouclant sur les bords pour que le motif se raccorde.
    let r = blur as isize;
    for _ in 0..2 {
        for vertical in [false, true] {
            let src = values.clone();
            for y in 0..size {
                for x in 0..size {
                    let mut sum = 0.0;
                    for d in -r..=r {
                        let (sx, sy) = if vertical { (x as isize, y as isize + d) } else { (x as isize + d, y as isize) };
                        let (sx, sy) = (sx.rem_euclid(size as isize) as usize, sy.rem_euclid(size as isize) as usize);
                        sum += src[sy * size + sx];
                    }
                    values[y * size + x] = sum / (2 * r + 1) as f32;
                }
            }
        }
    }
    // Répartition uniforme : chaque texel prend son rang.
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let n = (values.len() - 1).max(1) as f32;
    let mut ranked = vec![0.0f32; values.len()];
    for (rank, &i) in order.iter().enumerate() {
        ranked[i] = rank as f32 / n;
    }
    GrayImage::new(size, size, ranked)
}

// ------------------------------------------------- textures fabriquées par Pelure (rendu, étape 1)
//
// Pelure ne livre aucune image venue d'ailleurs : ses pointes et ses papiers sont fabriqués ici,
// au démarrage, par des tirages déterministes (la même graine donne toujours la même image, donc la
// même clé de contenu dans les projets). Tailles pensées pour un projet 1920×1080 vu à 50 % : le
// grain doit se voir à moitié taille.

/// Mélange de MurmurHash3 (`fmix32`).
fn fmix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

/// Bruit de 0 à 1 au texel (`x`, `y`) pour la graine `seed`, sans motif entre voisins.
fn hash01(seed: u32, x: u32, y: u32) -> f32 {
    let h = fmix(fmix(seed ^ x.wrapping_mul(0x9E37_79B1)) ^ y.wrapping_mul(0x85EB_CA77));
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Tirages déterministes (xorshift) pour fabriquer une pointe.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(fmix(seed) | 1)
    }

    /// De 0 à 1.
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// Flou en boîte de rayon `r` sur une image carrée qui se répète (deux passes : proche d'un flou
/// gaussien), en place.
fn blur_wrap(values: &mut [f32], size: usize, r: usize) {
    if r == 0 {
        return;
    }
    let r = r as isize;
    for _ in 0..2 {
        for vertical in [false, true] {
            let src = values.to_vec();
            for y in 0..size {
                for x in 0..size {
                    let mut sum = 0.0;
                    for d in -r..=r {
                        let (sx, sy) = if vertical { (x as isize, y as isize + d) } else { (x as isize + d, y as isize) };
                        let (sx, sy) = (sx.rem_euclid(size as isize) as usize, sy.rem_euclid(size as isize) as usize);
                        sum += src[sy * size + sx];
                    }
                    values[y * size + x] = sum / (2 * r + 1) as f32;
                }
            }
        }
    }
}

/// Ramène des valeurs à une moyenne nulle et un écart-type de 1 (un bruit flouté perd de
/// l'amplitude ; on le remet à l'échelle avant de l'additionner à d'autres).
fn standardize(values: &mut [f32]) {
    let n = values.len().max(1) as f32;
    let mean = values.iter().sum::<f32>() / n;
    let var = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n;
    let sd = var.sqrt().max(1e-6);
    for v in values {
        *v = (*v - mean) / sd;
    }
}

/// Papier à plusieurs échelles : la somme de bruits adoucis sur `blur` texels, pondérés par
/// `weight` (une octave par couple), répartie par rang. Un grain de papier à dessin a du relief à
/// 1, 2, 4 et 8 texels à la fois, pas seulement à un.
pub fn multiscale_paper(size: usize, seed: u32, octaves: &[(usize, f32)]) -> GrayImage {
    let size = size.max(4);
    let mut sum = vec![0.0f32; size * size];
    for (k, &(blur, weight)) in octaves.iter().enumerate() {
        let s = seed.wrapping_add((k as u32 + 1).wrapping_mul(0x9E37_79B9));
        let mut v: Vec<f32> = (0..size * size).map(|i| hash01(s, (i % size) as u32, (i / size) as u32)).collect();
        blur_wrap(&mut v, size, blur);
        standardize(&mut v);
        for (acc, x) in sum.iter_mut().zip(&v) {
            *acc += weight * x;
        }
    }
    GrayImage::new(size, size, sum).equalized()
}

/// Toile : des fils qui passent dessus-dessous tous les `period` texels, plus un peu de bruit.
pub fn canvas_paper(size: usize, period: usize, seed: u32) -> GrayImage {
    let size = size.max(4);
    let period = period.max(2);
    let mut noise: Vec<f32> = (0..size * size).map(|i| hash01(seed, (i % size) as u32, (i / size) as u32)).collect();
    blur_wrap(&mut noise, size, 1);
    standardize(&mut noise);
    let tau = std::f32::consts::TAU;
    let values = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size, i / size);
            let warp = ((x as f32 + 0.5) / period as f32 * tau).sin().abs();
            let weft = ((y as f32 + 0.5) / period as f32 * tau).sin().abs();
            let over = ((x / period) + (y / period)) % 2 == 0;
            let weave = if over { 0.7 * warp + 0.3 * weft } else { 0.3 * warp + 0.7 * weft };
            weave + 0.18 * noise[i]
        })
        .collect();
    GrayImage::new(size, size, values).equalized()
}

/// Vergé : de fines lignes horizontales (vergeures) tous les 4 texels, des lignes verticales
/// espacées (pontuseaux), et le grain du papier.
pub fn laid_paper(size: usize, seed: u32) -> GrayImage {
    let size = size.max(8);
    let grain = multiscale_paper(size, seed, &[(0, 0.5), (1, 0.5), (3, 0.3)]);
    let tau = std::f32::consts::TAU;
    let chain = (size / 4).max(8);
    let values = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size, i / size);
            let wobble = 0.35 * ((x as f32) / size as f32 * tau * 3.0).sin();
            let lines = 0.5 + 0.5 * ((y as f32 + wobble) / 4.0 * tau).sin();
            let dx = (x % chain) as f32 - chain as f32 * 0.5;
            let posts = (-(dx * dx) / 2.0).exp();
            0.55 * lines + 0.6 * grain.values[i] + 0.3 * posts
        })
        .collect();
    GrayImage::new(size, size, values).equalized()
}

/// Pose un disque doux : plein jusqu'à `soft` du rayon, puis une chute jusqu'au bord ; ce qu'il
/// dépose (`amount`) se cumule avec ce qui est déjà là, comme des couches de pigment.
fn stamp_disc(v: &mut [f32], w: usize, h: usize, cx: f32, cy: f32, r: f32, soft: f32, amount: f32) {
    let x0 = (cx - r - 1.0).floor().max(0.0) as usize;
    let y0 = (cy - r - 1.0).floor().max(0.0) as usize;
    let x1 = ((cx + r + 1.0).ceil().max(0.0) as usize).min(w);
    let y1 = ((cy + r + 1.0).ceil().max(0.0) as usize).min(h);
    let inner = r * soft.clamp(0.0, 1.0);
    for y in y0..y1 {
        for x in x0..x1 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            if d >= r {
                continue;
            }
            let t = ((d - inner) / (r - inner).max(1e-3)).clamp(0.0, 1.0);
            let a = amount * (1.0 - t * t * (3.0 - 2.0 * t));
            let p = &mut v[y * w + x];
            *p = 1.0 - (1.0 - *p) * (1.0 - a.clamp(0.0, 1.0));
        }
    }
}

/// Pose un segment doux (un poil) de `a` à `b`, large de `thick`, de force `amount`.
fn stamp_hair(v: &mut [f32], w: usize, h: usize, a: (f32, f32), b: (f32, f32), thick: f32, amount: f32) {
    let x0 = (a.0.min(b.0) - thick - 1.0).floor().max(0.0) as usize;
    let y0 = (a.1.min(b.1) - thick - 1.0).floor().max(0.0) as usize;
    let x1 = ((a.0.max(b.0) + thick + 1.0).ceil().max(0.0) as usize).min(w);
    let y1 = ((a.1.max(b.1) + thick + 1.0).ceil().max(0.0) as usize).min(h);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    for y in y0..y1 {
        for x in x0..x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let t = (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0);
            let d = ((px - a.0 - dx * t).powi(2) + (py - a.1 - dy * t).powi(2)).sqrt();
            let k = ((thick - d) / 1.2).clamp(0.0, 1.0);
            if k <= 0.0 {
                continue;
            }
            // Un poil dépose plus au milieu de sa longueur qu'à ses bouts.
            let along = (t * std::f32::consts::PI).sin().sqrt();
            let p = &mut v[y * w + x];
            *p = 1.0 - (1.0 - *p) * (1.0 - (amount * k * along).clamp(0.0, 1.0));
        }
    }
}

/// Bruit adouci (flou `blur`), étiré de 0 à 1, pour une case de pointe `size`×`size`.
fn cell_noise(size: usize, blur: usize, seed: u32) -> Vec<f32> {
    let mut n: Vec<f32> = (0..size * size).map(|i| hash01(seed, (i % size) as u32, (i / size) as u32)).collect();
    blur_wrap(&mut n, size, blur);
    let (lo, hi) = n.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    for v in &mut n {
        *v = (*v - lo) / (hi - lo).max(1e-6);
    }
    n
}

/// Les tirages d'une pointe « amas de grains » : le corps (taches faibles), les grains francs et
/// le sable fin — nombre, rayon (en part du rayon de la case), netteté du bord (`soft`, 0 = tout
/// doux, 1 = franc) et force de chacun.
#[derive(Clone, Copy, Debug)]
pub struct Grains {
    pub body: (usize, f32),
    pub grains: usize,
    pub grain_r: (f32, f32),
    pub grain_soft: f32,
    pub sand: usize,
    pub sand_r: (f32, f32),
    pub sand_soft: f32,
}

/// Les grains de la première version (étapes 1 à 3) : nuageux (gardés pour les planches
/// avant / après de l'essai d'audit).
#[allow(dead_code)]
pub const GRAINS_V1: Grains = Grains { body: (3, 0.18), grains: 16, grain_r: (0.08, 0.18), grain_soft: 0.7, sand: 24, sand_r: (0.03, 0.07), sand_soft: 0.5 };
/// Les grains « sable » (Anthony, 2026-10-18 : « plus franc, comme le 2B de TVPaint, moins
/// nuageux ») : presque pas de corps, des grains nets et serrés.
pub const GRAINS_SAND: Grains = Grains { body: (2, 0.08), grains: 26, grain_r: (0.06, 0.13), grain_soft: 0.92, sand: 30, sand_r: (0.03, 0.06), sand_soft: 0.85 };

/// Pointe « amas de grains » (graphite, crayon de couleur) : `cells` dessins côte à côte de
/// `size` texels, chacun un corps faible, des grains francs (le dépôt de la mine sur les sommets
/// du papier) et du sable plus fin, plus denses au centre. Les grains sont gros par rapport à la
/// case : à 6 px de diamètre ils font encore un demi-pixel et se voient.
pub fn tip_grains(size: usize, cells: usize, seed: u32) -> GrayImage {
    tip_grains_with(size, cells, seed, &GRAINS_SAND)
}

pub fn tip_grains_with(size: usize, cells: usize, seed: u32, g: &Grains) -> GrayImage {
    let (size, cells) = (size.max(8), cells.max(1));
    let (w, h) = (size * cells, size);
    let mut v = vec![0.0f32; w * h];
    let mut rng = Rng::new(seed);
    let rad = size as f32 * 0.5;
    for c in 0..cells {
        let (cx, cy) = (c as f32 * size as f32 + rad, rad);
        for _ in 0..g.body.0 {
            let a = rng.next() * std::f32::consts::TAU;
            let d = rng.next() * rad * 0.3;
            stamp_disc(&mut v, w, h, cx + d * a.cos(), cy + d * a.sin(), rad * rng.range(0.3, 0.5), 0.0, g.body.1);
        }
        for _ in 0..g.grains {
            let a = rng.next() * std::f32::consts::TAU;
            let d = rad * 0.8 * rng.next().powf(0.7);
            let r = rad * rng.range(g.grain_r.0, g.grain_r.1);
            let amount = rng.range(0.6, 1.0) * (1.0 - 0.35 * d / rad);
            stamp_disc(&mut v, w, h, cx + d * a.cos(), cy + d * a.sin(), r, g.grain_soft, amount);
        }
        for _ in 0..g.sand {
            let a = rng.next() * std::f32::consts::TAU;
            let d = rad * 0.9 * rng.next().powf(0.6);
            let r = rad * rng.range(g.sand_r.0, g.sand_r.1);
            stamp_disc(&mut v, w, h, cx + d * a.cos(), cy + d * a.sin(), r, g.sand_soft, rng.range(0.4, 0.9));
        }
    }
    GrayImage::new(w, h, clamped(v))
}

/// Valeurs ramenées entre 0 et 1 (les arrondis des cumuls peuvent déborder d'un rien).
fn clamped(mut v: Vec<f32>) -> Vec<f32> {
    for x in &mut v {
        *x = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
    }
    v
}

/// Pointe de bâton (fusain, craie) : une forme pleine au bord déchiqueté, trouée là où le bruit
/// est creux. `aspect` aplatit la forme (1 : ronde), `edge` la netteté du bord (0,8 : fusain
/// cotonneux, 0,92 : craie nette), `holes` la part de trous (0,4 : fusain, 0,25 : craie).
pub fn tip_stick(size: usize, cells: usize, seed: u32, aspect: f32, edge: f32, holes: f32) -> GrayImage {
    let (size, cells) = (size.max(8), cells.max(1));
    let (w, h) = (size * cells, size);
    let mut v = vec![0.0f32; w * h];
    let mut rng = Rng::new(seed);
    let rad = size as f32 * 0.44;
    let half = size as f32 * 0.5;
    for c in 0..cells {
        let phases = [rng.next(), rng.next(), rng.next()].map(|p| p * std::f32::consts::TAU);
        let noise = cell_noise(size, 1, seed.wrapping_add(c as u32 * 7919));
        let fine = cell_noise(size, 0, seed.wrapping_add(c as u32 * 104_729 + 17));
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (x as f32 + 0.5 - half, (y as f32 + 0.5 - half) / aspect.clamp(0.2, 1.0));
                let theta = dy.atan2(dx);
                let ragged = 1.0 + 0.12 * (3.0 * theta + phases[0]).sin() + 0.07 * (7.0 * theta + phases[1]).sin() + 0.05 * (11.0 * theta + phases[2]).sin();
                let n = noise[y * size + x];
                let d = (dx * dx + dy * dy).sqrt() / (rad * ragged) * (1.0 + 0.18 * (n - 0.5));
                let body = 1.0 - smoothstep(edge, 1.0, d);
                if body <= 0.0 {
                    continue;
                }
                let open = smoothstep(holes * 0.5, holes.max(0.05) + 0.25, n);
                let speck = 0.85 + 0.15 * fine[y * size + x];
                v[y * w + x + c * size] = (body * open * speck).clamp(0.0, 1.0);
            }
        }
    }
    GrayImage::new(w, h, clamped(v))
}

/// Pointe à poils plats (gouache, huile) : des poils le long de la largeur, de longueurs et de
/// forces inégales, dans une enveloppe aplatie ; la pointe suit la direction du trait.
pub fn tip_bristles_flat(size: usize, cells: usize, seed: u32) -> GrayImage {
    let (size, cells) = (size.max(8), cells.max(1));
    let (w, h) = (size * cells, size);
    let mut v = vec![0.0f32; w * h];
    let mut rng = Rng::new(seed);
    let s = size as f32;
    for c in 0..cells {
        let x0 = c as f32 * s;
        // Une douzaine de poils assez gros pour rester des stries à 18 px de large.
        let n = 12;
        for k in 0..n {
            let y = s * (0.14 + 0.72 * k as f32 / (n - 1) as f32) + rng.range(-0.02, 0.02) * s;
            let len = s * rng.range(0.55, 0.92);
            let xa = x0 + s * 0.5 - len * 0.5 + rng.range(-0.05, 0.05) * s;
            let thick = s * rng.range(0.02, 0.045);
            let amount = rng.range(0.3, 1.0);
            stamp_hair(&mut v, w, h, (xa, y), (xa + len, y + rng.range(-0.012, 0.012) * s), thick, amount);
        }
        // Enveloppe : les poils s'effilent aux bouts de la touffe.
        for y in 0..size {
            for x in 0..size {
                let (ex, ey) = ((x as f32 + 0.5 - s * 0.5) / (s * 0.47), (y as f32 + 0.5 - s * 0.5) / (s * 0.4));
                let r = (ex * ex + ey * ey).sqrt();
                v[y * w + x + c * size] *= 1.0 - smoothstep(0.8, 1.05, r);
            }
        }
    }
    GrayImage::new(w, h, clamped(v))
}

/// Pointe à poils ronds (pinceau rond chargé) : un nuage de poils vus de bout, plus denses au
/// centre, sur un corps doux.
pub fn tip_bristles_round(size: usize, cells: usize, seed: u32) -> GrayImage {
    let (size, cells) = (size.max(8), cells.max(1));
    let (w, h) = (size * cells, size);
    let mut v = vec![0.0f32; w * h];
    let mut rng = Rng::new(seed);
    let rad = size as f32 * 0.5;
    for c in 0..cells {
        let (cx, cy) = (c as f32 * size as f32 + rad, rad);
        stamp_disc(&mut v, w, h, cx, cy, rad * 0.7, 0.2, 0.3);
        for _ in 0..48 {
            let a = rng.next() * std::f32::consts::TAU;
            let d = rad * 0.84 * rng.next().powf(0.6);
            let r = rad * rng.range(0.04, 0.09);
            stamp_disc(&mut v, w, h, cx + d * a.cos(), cy + d * a.sin(), r, 0.4, rng.range(0.4, 0.95));
        }
    }
    GrayImage::new(w, h, clamped(v))
}

/// Le grain propre du crayon (rendu, étape 3 ; le « Grain » Silk / Soft / Medium / Hard du crayon
/// de TVPaint) : un papier fin fabriqué une fois, collé à la feuille, que les crayons voient même
/// sans papier. Relief sur 1 à 2 texels, un peu plus sur 4 : visible à 50 % de zoom sans faire
/// de taches.
static PENCIL_GRAIN: LazyLock<Arc<GrayImage>> = LazyLock::new(|| Arc::new(multiscale_paper(256, 0x5A17_C0DE, &[(0, 0.45), (1, 0.45), (2, 0.3)])));

pub fn pencil_grain_paper() -> Arc<GrayImage> {
    PENCIL_GRAIN.clone()
}

/// Textures d'un trait, prêtes à l'emploi.
#[derive(Clone, Default)]
pub struct BrushTextures {
    pub tip: Option<Arc<Tip>>,
    pub paper: Option<Arc<GrayImage>>,
}

/// Outil auquel s'applique un réglage de pinceau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Target {
    #[serde(rename = "pinceau")]
    Brush,
    #[serde(rename = "gomme")]
    Eraser,
}

/// Un réglage de pinceau nommé.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "outil")]
    pub target: Target,
    #[serde(rename = "reglages")]
    pub settings: BrushSettings,
    /// « Pelure », ou le fichier d'où il a été importé.
    pub source: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum TextureKind {
    #[serde(rename = "pointe")]
    Tip,
    #[serde(rename = "papier")]
    Paper,
}

impl TextureKind {
    /// Nom dans les fichiers de textures d'un projet.
    pub fn file_word(self) -> &'static str {
        match self {
            TextureKind::Tip => "pointe",
            TextureKind::Paper => "papier",
        }
    }
}

// ---------------------------------------------------------------- textures partagées (7b)

/// Image en octets (0 à 255), telle qu'elle est écrite dans un fichier.
pub fn quantized(img: &GrayImage) -> Vec<u8> {
    img.values.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// Clé d'une texture d'après son contenu (sorte, taille, octets) : la même image a la même clé
/// d'une séance et d'une machine à l'autre. Le bit de poids fort est mis : une clé ne ressemble
/// jamais à un numéro de la bibliothèque de pinceaux.
pub fn content_key(kind: TextureKind, img: &GrayImage) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    let mut eat = |b: u8| {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    };
    eat(kind as u8);
    for b in (img.width as u32).to_le_bytes().into_iter().chain((img.height as u32).to_le_bytes()) {
        eat(b);
    }
    for b in quantized(img) {
        eat(b);
    }
    h | 0x8000_0000
}

/// Textures que voient les traits vectoriels texturés, par clé de contenu : celles de la
/// bibliothèque de pinceaux et celles des projets ouverts. Partagées, parce que le rendu d'un
/// dessin vectoriel n'a pas accès à l'application.
#[derive(Default)]
struct Shared {
    tips: HashMap<u32, Arc<Tip>>,
    papers: HashMap<u32, Arc<GrayImage>>,
    /// Numéro dans la bibliothèque → clé de contenu.
    from_library: HashMap<(TextureKind, u32), u32>,
}

static SHARED: LazyLock<RwLock<Shared>> = LazyLock::new(Default::default);

/// Clé de contenu de la texture `id` de la bibliothèque (après `BrushLibrary::share`).
pub fn shared_key(kind: TextureKind, id: u32) -> Option<u32> {
    SHARED.read().ok()?.from_library.get(&(kind, id)).copied()
}

/// Les textures d'un réglage dont `tip` et `paper` sont des clés de contenu (celles qui manquent
/// sont ignorées : pointe ronde, papier lisse).
pub fn shared_textures(s: &BrushSettings) -> BrushTextures {
    let Ok(shared) = SHARED.read() else { return BrushTextures::default() };
    BrushTextures {
        tip: s.tip.and_then(|k| shared.tips.get(&k)).cloned(),
        paper: s.paper.and_then(|k| shared.papers.get(&k)).cloned(),
    }
}

/// L'image d'une texture partagée (pour l'écrire dans un projet).
pub fn shared_image(kind: TextureKind, key: u32) -> Option<GrayImage> {
    let shared = SHARED.read().ok()?;
    match kind {
        TextureKind::Tip => shared.tips.get(&key).map(|t| t.full().clone()),
        TextureKind::Paper => shared.papers.get(&key).map(|p| p.as_ref().clone()),
    }
}

/// Ajoute une texture lue dans un projet, sous sa clé (sans remplacer celle déjà là).
pub fn share_texture(kind: TextureKind, key: u32, img: GrayImage) {
    let Ok(mut shared) = SHARED.write() else { return };
    match kind {
        TextureKind::Tip => {
            shared.tips.entry(key).or_insert_with(|| Arc::new(Tip::new(img)));
        }
        TextureKind::Paper => {
            shared.papers.entry(key).or_insert_with(|| Arc::new(img));
        }
    }
}

/// Pointes, papiers et réglages nommés. Ceux de Pelure sont fabriqués au démarrage ; ceux
/// importés sont gardés dans %APPDATA%\Pelure\pinceaux.
pub struct BrushLibrary {
    tips: HashMap<u32, (String, Arc<Tip>)>,
    papers: HashMap<u32, (String, Arc<GrayImage>)>,
    pub presets: Vec<Preset>,
    next_id: u32,
}

/// Provenance des réglages enregistrés par Anthony (moteur de dessin, étape 9) : gardés comme
/// les importés dans %APPDATA%\Pelure\pinceaux.
pub const MINE: &str = "Mes brosses";

/// Papiers de Pelure. Les deux premiers, des bruits par texel (d'avant l'audit du rendu), restent
/// pour les réglages enregistrés qui s'en servent ; les suivants ont une structure.
pub const PAPER_FINE: u32 = 1;
pub const PAPER_COARSE: u32 = 2;
/// « Papier à dessin » : relief à plusieurs échelles (2 à 8 texels), celui des crayons.
pub const PAPER_DRAWING: u32 = 3;
/// « Toile » : fils dessus-dessous.
pub const PAPER_CANVAS: u32 = 4;
/// « Vergé » : vergeures et pontuseaux.
pub const PAPER_LAID: u32 = 5;
/// « Papier à dessin fin » : le papier des crayons graphite et de couleur depuis le retour
/// d'Anthony (grain « sable », relief fin à 1 et 2 texels). Un papier à part : le « Papier à
/// dessin » (3) garde le relief des étapes 1 à 3 pour le fusain, la gomme à grain et les réglages
/// enregistrés qui s'en servent.
pub const PAPER_GRAPHITE: u32 = 6;
/// Pointes de Pelure (fabriquées au démarrage).
pub const TIP_GRAPHITE: u32 = 10;
pub const TIP_CHARCOAL: u32 = 11;
pub const TIP_CHALK: u32 = 12;
pub const TIP_BRISTLES_FLAT: u32 = 13;
pub const TIP_BRISTLES_ROUND: u32 = 14;
/// Numéros des textures importées : à partir d'ici.
const FIRST_IMPORTED: u32 = 100;

/// Relief du « Papier à dessin » (étapes 1 à 3 : surtout 2 à 8 texels) : ne change plus, les
/// réglages qui s'en servent gardent leur rendu.
pub const PAPER_DRAWING_OCTAVES: &[(usize, f32)] = &[(0, 0.25), (1, 0.45), (2, 0.55), (4, 0.35)];
/// Relief du « Papier à dessin fin » des crayons (retour d'Anthony : grain « sable »).
pub const PAPER_GRAPHITE_OCTAVES: &[(usize, f32)] = &[(0, 0.5), (1, 0.55), (2, 0.4), (4, 0.25)];

/// Nombre de cases d'une pointe de Pelure (aucun : pointe importée).
pub fn builtin_tip_cells(id: u32) -> Option<u32> {
    match id {
        TIP_GRAPHITE => Some(8),
        TIP_CHARCOAL | TIP_CHALK => Some(6),
        TIP_BRISTLES_FLAT | TIP_BRISTLES_ROUND => Some(4),
        _ => None,
    }
}

#[derive(Serialize, Deserialize)]
struct SavedTexture {
    id: u32,
    #[serde(rename = "nom")]
    name: String,
    #[serde(rename = "type")]
    kind: TextureKind,
    #[serde(rename = "fichier")]
    file: String,
}

#[derive(Serialize, Deserialize, Default)]
struct SavedLibrary {
    textures: Vec<SavedTexture>,
    #[serde(rename = "reglages")]
    presets: Vec<Preset>,
}

impl BrushLibrary {
    /// Les pinceaux de Pelure (sans les importés).
    pub fn builtin() -> BrushLibrary {
        let mut lib = BrushLibrary {
            tips: HashMap::new(),
            papers: HashMap::new(),
            presets: Vec::new(),
            next_id: FIRST_IMPORTED,
        };
        lib.papers.insert(PAPER_FINE, ("Grain fin".into(), Arc::new(procedural_paper(256, 0, 11))));
        lib.papers.insert(PAPER_COARSE, ("Grain gros".into(), Arc::new(procedural_paper(256, 1, 29))));
        // Papiers et pointes fabriqués (rendu, étape 1) : pour un projet 1920×1080 vu à 50 %.
        lib.papers.insert(PAPER_DRAWING, ("Papier à dessin".into(), Arc::new(multiscale_paper(256, 0x0A11, PAPER_DRAWING_OCTAVES))));
        lib.papers.insert(PAPER_CANVAS, ("Toile".into(), Arc::new(canvas_paper(256, 8, 0x70A1))));
        lib.papers.insert(PAPER_LAID, ("Vergé".into(), Arc::new(laid_paper(256, 0x7E86))));
        // Papier des crayons : plus de relief fin (1 et 2 texels) depuis le retour d'Anthony (grain
        // « sable », pas nuageux), un peu de large pour la respiration.
        lib.papers.insert(PAPER_GRAPHITE, ("Papier à dessin fin".into(), Arc::new(multiscale_paper(256, 0x0A11, PAPER_GRAPHITE_OCTAVES))));
        let tip = |img: GrayImage| Arc::new(Tip::new(img));
        lib.tips.insert(TIP_GRAPHITE, ("Graphite".into(), tip(tip_grains(48, 8, 0x6A1))));
        lib.tips.insert(TIP_CHARCOAL, ("Fusain".into(), tip(tip_stick(96, 6, 0xF05A, 0.9, 0.8, 0.4))));
        lib.tips.insert(TIP_CHALK, ("Craie".into(), tip(tip_stick(96, 6, 0xC2A1, 0.75, 0.92, 0.22))));
        lib.tips.insert(TIP_BRISTLES_FLAT, ("Poils plats".into(), tip(tip_bristles_flat(96, 4, 0x901A))));
        lib.tips.insert(TIP_BRISTLES_ROUND, ("Poils ronds".into(), tip(tip_bristles_round(80, 4, 0x9014))));
        let pelure = |name: &str, target, settings| Preset {
            name: name.into(),
            target,
            settings,
            source: "Pelure".into(),
        };
        let base = BrushSettings::brush();
        lib.presets = vec![
            // Le Pinceau rond reste le premier et le réglage de départ de l'outil Pinceau (choix
            // d'Anthony, 2026-10-16) ; les brosses traditionnelles viennent juste après.
            pelure("Pinceau rond", Target::Brush, base),
            pelure("Crayon graphite", Target::Brush, BrushSettings::graphite()),
            pelure("Crayon de couleur", Target::Brush, BrushSettings::colour_pencil()),
            pelure("Encre", Target::Brush, BrushSettings::ink()),
            pelure("Feutre", Target::Brush, BrushSettings::felt()),
            pelure("Fusain", Target::Brush, BrushSettings::charcoal()),
            pelure("Craie", Target::Brush, BrushSettings::chalk()),
            pelure("Gouache", Target::Brush, BrushSettings::gouache()),
            // Le crayon net d'avant l'audit (anticrénelé, sans matière), gardé à portée de main.
            pelure("Crayon net", Target::Brush, BrushSettings::pencil()),
            pelure("Gomme", Target::Eraser, BrushSettings::eraser()),
            pelure(
                "Gomme à grain",
                Target::Eraser,
                BrushSettings {
                    grain: 0.8,
                    paper: Some(PAPER_DRAWING),
                    paper_scale: 2.0,
                    paper_softness: GRAIN_SOFTNESS_SOFT,
                    pressure_opacity: true,
                    min_opacity: 0.2,
                    ..BrushSettings::eraser()
                },
            ),
        ];
        lib
    }

    /// L'image d'une pointe de la bibliothèque (pour les planches et les aperçus).
    pub fn tip(&self, id: u32) -> Option<Arc<Tip>> {
        self.tips.get(&id).map(|t| t.1.clone())
    }

    /// Pointes, par numéro.
    pub fn tips(&self) -> Vec<(u32, String)> {
        let mut v: Vec<(u32, String)> = self.tips.iter().map(|(&id, (n, _))| (id, n.clone())).collect();
        v.sort_by_key(|p| p.0);
        v
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_tip(&mut self, name: &str, image: GrayImage) -> u32 {
        let id = self.take_id();
        self.tips.insert(id, (name.into(), Arc::new(Tip::new(image))));
        id
    }

    /// Un papier importé : son relief est réparti uniformément (`equalized`), ce que le grain
    /// suppose.
    pub fn add_paper(&mut self, name: &str, image: GrayImage) -> u32 {
        let id = self.take_id();
        self.papers.insert(id, (name.into(), Arc::new(image.equalized())));
        id
    }

    /// Met les textures de la bibliothèque à la disposition des traits vectoriels (au démarrage
    /// et après un import).
    pub fn share(&self) {
        let Ok(mut shared) = SHARED.write() else { return };
        for (&id, (_, tip)) in &self.tips {
            let key = content_key(TextureKind::Tip, tip.full());
            shared.tips.entry(key).or_insert_with(|| tip.clone());
            shared.from_library.insert((TextureKind::Tip, id), key);
        }
        for (&id, (_, paper)) in &self.papers {
            let key = content_key(TextureKind::Paper, paper);
            shared.papers.entry(key).or_insert_with(|| paper.clone());
            shared.from_library.insert((TextureKind::Paper, id), key);
        }
    }

    /// Le réglage `s` (numéros de la bibliothèque) pour un trait vectoriel : pointe et papier
    /// désignés par leur clé de contenu. Toujours donné depuis l'étape 5 du rendu (le trait
    /// vectoriel rejoue le moteur bitmap avec son réglage complet, texturé ou non) ; avant, seuls
    /// les réglages texturés étaient gardés.
    pub fn for_vector(&self, s: &BrushSettings) -> Option<BrushSettings> {
        // Une texture pas encore partagée (bibliothèque changée depuis) : on partage à nouveau.
        let key = |kind, id: Option<u32>| {
            id.and_then(|id| shared_key(kind, id).or_else(|| {
                self.share();
                shared_key(kind, id)
            }))
        };
        Some(BrushSettings { tip: key(TextureKind::Tip, s.tip), paper: key(TextureKind::Paper, s.paper), ..*s })
    }

    /// Les textures d'un réglage (celles qui manquent sont ignorées).
    pub fn textures(&self, s: &BrushSettings) -> BrushTextures {
        BrushTextures {
            tip: s.tip.and_then(|id| self.tips.get(&id)).map(|t| t.1.clone()),
            paper: s.paper.and_then(|id| self.papers.get(&id)).map(|p| p.1.clone()),
        }
    }

    /// Papiers, par numéro.
    pub fn papers(&self) -> Vec<(u32, String)> {
        let mut v: Vec<(u32, String)> = self.papers.iter().map(|(&id, (n, _))| (id, n.clone())).collect();
        v.sort_by_key(|p| p.0);
        v
    }

    pub fn tip_name(&self, id: u32) -> Option<&str> {
        self.tips.get(&id).map(|t| t.0.as_str())
    }

    /// Dossier des pinceaux importés.
    pub fn user_dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Pelure").join("pinceaux"))
    }

    /// Les pinceaux de Pelure, plus ceux importés et gardés dans `dir`.
    pub fn load(dir: Option<&Path>) -> BrushLibrary {
        let mut lib = BrushLibrary::builtin();
        let Some(dir) = dir else { return lib };
        let Ok(text) = std::fs::read_to_string(dir.join("pinceaux.json")) else { return lib };
        let Ok(saved) = serde_json::from_str::<SavedLibrary>(&text) else { return lib };
        for t in saved.textures {
            let Ok(img) = image::open(dir.join(&t.file)) else { continue };
            let gray = img.to_luma8();
            let values = gray.as_raw().iter().map(|&v| v as f32 / 255.0).collect();
            let image = GrayImage::new(gray.width() as usize, gray.height() as usize, values);
            match t.kind {
                TextureKind::Tip => {
                    lib.tips.insert(t.id, (t.name, Arc::new(Tip::new(image))));
                }
                TextureKind::Paper => {
                    lib.papers.insert(t.id, (t.name, Arc::new(image.equalized())));
                }
            }
            lib.next_id = lib.next_id.max(t.id + 1);
        }
        lib.presets.extend(saved.presets);
        lib.share();
        lib
    }

    /// Enregistre `settings` sous le nom `name` dans « Mes brosses » ; renvoie son rang. Un nom
    /// déjà pris dans « Mes brosses » (pour le même outil) est remplacé.
    pub fn save_mine(&mut self, name: &str, target: Target, settings: BrushSettings) -> usize {
        let name = name.trim();
        let name = if name.is_empty() { "Sans nom" } else { name };
        if let Some(i) = self.presets.iter().position(|p| p.source == MINE && p.target == target && p.name == name) {
            self.presets[i].settings = settings;
            return i;
        }
        self.presets.push(Preset { name: name.to_string(), target, settings, source: MINE.to_string() });
        self.presets.len() - 1
    }

    /// Un nom libre dans « Mes brosses » pour ce `base` (« Craie », « Craie 2 »…).
    pub fn free_name(&self, base: &str, target: Target) -> String {
        let taken = |n: &str| self.presets.iter().any(|p| p.source == MINE && p.target == target && p.name == n);
        if !taken(base) {
            return base.to_string();
        }
        (2..).map(|k| format!("{base} {k}")).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
    }

    /// Garde dans `dir` les pinceaux importés et leurs textures.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut saved = SavedLibrary::default();
        let images = self
            .tips
            .iter()
            .map(|(&id, (name, tip))| (id, name, TextureKind::Tip, tip.full()))
            .chain(self.papers.iter().map(|(&id, (name, p))| (id, name, TextureKind::Paper, p.as_ref())))
            .filter(|t| t.0 >= FIRST_IMPORTED);
        for (id, name, kind, img) in images {
            let file = format!("texture_{id}.png");
            let bytes: Vec<u8> = img.values.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
            image::save_buffer(dir.join(&file), &bytes, img.width as u32, img.height as u32, image::ExtendedColorType::L8)
                .map_err(|e| e.to_string())?;
            saved.textures.push(SavedTexture { id, name: name.clone(), kind, file });
        }
        saved.presets = self.presets.iter().filter(|p| p.source != "Pelure").cloned().collect();
        let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("pinceaux.json"), text).map_err(|e| e.to_string())
    }
}
