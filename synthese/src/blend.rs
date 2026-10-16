//! Mélange de deux séquences (étape c). Entre deux images clés, chaque image est fabriquée deux
//! fois : depuis la clé d'avant (A) et depuis celle d'après (B). Un fondu enchaîné
//! (1 − α)·A + α·B double les traits et délave la texture là où A et B diffèrent. Comme dans
//! « Stylizing Video by Example » (Jamriška et al. 2019), on mélange plutôt dans le domaine des
//! gradients :
//! - chaque pixel prend ses détails (les écarts avec ses voisins) d'une seule des deux images,
//!   celle qui a le moins d'erreur de synthèse par rapport à un seuil qui glisse de A vers B au fil
//!   des images ;
//! - les couleurs d'ensemble suivent le fondu enchaîné (attache de poids `screen`) ;
//! - l'image est la solution de l'équation de Poisson « écrantée » qui concilie les deux, trouvée
//!   par gradient conjugué.

use crate::Image;

/// Erreur de synthèse réduite : moyenne par carrés de `cell` pixels, assez pour décider quelle
/// séquence garder par régions, sans bruit.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorMap {
    pub width: usize,
    pub height: usize,
    pub cell: usize,
    pub data: Vec<f32>,
}

impl ErrorMap {
    /// Réduit l'erreur par pixel `error` d'une image `width` × `height`.
    pub fn new(error: &[f32], width: usize, height: usize, cell: usize) -> ErrorMap {
        assert_eq!(error.len(), width * height, "taille de l'erreur");
        let cell = cell.max(1);
        let (cw, ch) = (width.div_ceil(cell), height.div_ceil(cell));
        let (mut sum, mut count) = (vec![0f32; cw * ch], vec![0u32; cw * ch]);
        for y in 0..height {
            for x in 0..width {
                let k = (y / cell) * cw + x / cell;
                sum[k] += error[y * width + x];
                count[k] += 1;
            }
        }
        let data = sum.iter().zip(&count).map(|(s, n)| s / *n as f32).collect();
        ErrorMap { width, height, cell, data }
    }

    /// Erreur au pixel (x, y), bilinéaire entre les centres des carrés.
    pub fn at(&self, x: usize, y: usize) -> f32 {
        let (cw, ch) = (self.width.div_ceil(self.cell) as i32, self.height.div_ceil(self.cell) as i32);
        let fx = (x as f32 + 0.5) / self.cell as f32 - 0.5;
        let fy = (y as f32 + 0.5) / self.cell as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (a, b) = (fx - x0, fy - y0);
        let v = |i: i32, j: i32| self.data[(j.clamp(0, ch - 1) * cw + i.clamp(0, cw - 1)) as usize];
        let (i, j) = (x0 as i32, y0 as i32);
        (v(i, j) * (1.0 - a) + v(i + 1, j) * a) * (1.0 - b) + (v(i, j + 1) * (1.0 - a) + v(i + 1, j + 1) * a) * b
    }
}

/// Réglages du mélange.
#[derive(Clone, Debug, PartialEq)]
pub struct BlendParams {
    /// Poids de l'attache au fondu enchaîné : les formes plus grandes qu'environ 2π / √screen
    /// pixels suivent le fondu, les détails plus fins viennent d'une seule des deux images.
    pub screen: f32,
    /// Itérations au plus du gradient conjugué.
    pub iters: usize,
    /// Largeur de la bascule de A vers B, en part d'erreur : 0 = chaque pixel bascule d'un coup
    /// (un petit saut de texture), plus grand = il passe par des détails mélangés pendant
    /// plusieurs images. Mesuré sur l'animatique BRAWLIDAYS (clés 40 et 50) : scintillement 2,33
    /// (net), 2,19 (0,6), 2,01 (fondu enchaîné) ; détail 4,63, 4,59, 4,46.
    pub softness: f32,
    /// Simple fondu enchaîné, pour comparer.
    pub linear: bool,
}

impl Default for BlendParams {
    fn default() -> BlendParams {
        BlendParams { screen: 0.1, iters: 300, softness: 0.5, linear: false }
    }
}

/// Pour chaque pixel, la part de A dans ses détails (0..1). A est gardé là où sa part de
/// l'erreur, ea / (ea + eb), reste sous 1 − α : au début (α proche de 0) presque partout, puis de
/// moins en moins, d'abord là où B est meilleur. `softness` étale la bascule.
pub fn selection(ea: &ErrorMap, eb: &ErrorMap, alpha: f32, softness: f32) -> Vec<f32> {
    let (w, h) = (ea.width, ea.height);
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (ea.at(x, y), eb.at(x, y));
            let margin = 1.0 - alpha - a / (a + b).max(1e-6);
            out.push(if softness > 0.0 {
                (margin / softness + 0.5).clamp(0.0, 1.0)
            } else {
                (margin > 0.0) as u8 as f32
            });
        }
    }
    out
}

/// Mélange A et B à l'instant `alpha` (0 = A, 1 = B) d'après leurs erreurs de synthèse.
pub fn blend(a: &Image, ea: &ErrorMap, b: &Image, eb: &ErrorMap, alpha: f32, params: &BlendParams) -> Image {
    assert_eq!((a.width, a.height, a.channels), (b.width, b.height, b.channels), "deux images pareilles");
    let (w, h, c) = (a.width, a.height, a.channels);
    let fade = |k: usize| (1.0 - alpha) * a.data[k] as f32 + alpha * b.data[k] as f32;
    if params.linear {
        let data = (0..a.data.len()).map(|k| (fade(k) + 0.5).clamp(0.0, 255.0) as u8).collect();
        return Image::new(w, h, c, data);
    }
    let from_a = selection(ea, eb, alpha, params.softness);
    let mut channels: Vec<Vec<f32>> = vec![Vec::new(); c];
    std::thread::scope(|scope| {
        for (ch, out) in channels.iter_mut().enumerate() {
            let from_a = &from_a;
            scope.spawn(move || {
                let px = |im: &Image, p: usize| im.data[p * c + ch] as f32;
                // Écart voulu entre p et son voisin q : ceux de A et de B, selon la part de A des
                // deux pixels (la moyenne des deux quand l'un a choisi A et l'autre B).
                let wanted = |p: usize, q: usize| {
                    let share = 0.5 * (from_a[p] + from_a[q]);
                    share * (px(a, q) - px(a, p)) + (1.0 - share) * (px(b, q) - px(b, p))
                };
                let lambda = params.screen.max(1e-4);
                let start: Vec<f32> = (0..w * h).map(|p| fade(p * c + ch)).collect();
                let mut rhs: Vec<f32> = start.iter().map(|v| lambda * v).collect();
                for y in 0..h {
                    for x in 0..w {
                        let p = y * w + x;
                        if x + 1 < w {
                            let d = wanted(p, p + 1);
                            rhs[p] -= d;
                            rhs[p + 1] += d;
                        }
                        if y + 1 < h {
                            let d = wanted(p, p + w);
                            rhs[p] -= d;
                            rhs[p + w] += d;
                        }
                    }
                }
                *out = solve_screened(w, h, lambda, &rhs, start, params.iters);
            });
        }
    });
    let mut data = Vec::with_capacity(w * h * c);
    for p in 0..w * h {
        for ch in &channels {
            data.push((ch[p] + 0.5).clamp(0.0, 255.0) as u8);
        }
    }
    Image::new(w, h, c, data)
}

/// (λ + Laplacien) appliqué à x, bords sans voisin (Neumann).
fn apply(w: usize, h: usize, lambda: f32, x: &[f32], out: &mut [f32]) {
    for y in 0..h {
        for i in 0..w {
            let p = y * w + i;
            let (mut deg, mut sum) = (0.0, 0.0);
            if i > 0 {
                deg += 1.0;
                sum += x[p - 1];
            }
            if i + 1 < w {
                deg += 1.0;
                sum += x[p + 1];
            }
            if y > 0 {
                deg += 1.0;
                sum += x[p - w];
            }
            if y + 1 < h {
                deg += 1.0;
                sum += x[p + w];
            }
            out[p] = (deg + lambda) * x[p] - sum;
        }
    }
}

fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum()
}

/// Gradient conjugué sur (λ + Laplacien) x = rhs, en partant de `x`. On s'arrête quand le résidu
/// moyen passe sous 0,01 (l'erreur est alors sous 0,01 / λ niveau de couleur).
fn solve_screened(w: usize, h: usize, lambda: f32, rhs: &[f32], mut x: Vec<f32>, iters: usize) -> Vec<f32> {
    let n = w * h;
    let mut ap = vec![0f32; n];
    apply(w, h, lambda, &x, &mut ap);
    let mut r: Vec<f32> = rhs.iter().zip(&ap).map(|(b, v)| b - v).collect();
    let mut p = r.clone();
    let mut rs = dot(&r, &r);
    let tol = 1e-4 * n as f64;
    for _ in 0..iters {
        if rs < tol {
            break;
        }
        apply(w, h, lambda, &p, &mut ap);
        let step = rs / dot(&p, &ap);
        for k in 0..n {
            x[k] += (step * p[k] as f64) as f32;
            r[k] -= (step * ap[k] as f64) as f32;
        }
        let rs_new = dot(&r, &r);
        let beta = rs_new / rs;
        for k in 0..n {
            p[k] = r[k] + (beta * p[k] as f64) as f32;
        }
        rs = rs_new;
    }
    x
}
