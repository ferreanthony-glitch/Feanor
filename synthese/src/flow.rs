//! Flux optique dense (étape b) : pour chaque pixel d'une image, où il se trouve dans une autre.
//! Méthode DIS (« Fast Optical Flow using Dense Inverse Search », Kroeger et al., ECCV 2016),
//! celle d'OpenCV sans le raffinement variationnel :
//! - pyramide d'images, du niveau le plus grossier au plus fin ; chaque niveau part du flux du
//!   précédent ;
//! - l'image est couverte de patchs carrés qui se chevauchent ; chacun cherche son déplacement par
//!   Lucas-Kanade « inverse compositionnel » (le gradient du patch est calculé une fois), après
//!   avoir essayé celui de ses voisins de gauche et du dessus ;
//! - le flux d'un pixel est la moyenne des déplacements des patchs qui le couvrent, pondérée par
//!   la ressemblance des couleurs.

use crate::Image;

/// Image en niveaux de gris, valeurs 0..255 en flottants.
#[derive(Clone, Debug, PartialEq)]
pub struct Gray {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Gray {
    pub fn new(width: usize, height: usize, data: Vec<f32>) -> Gray {
        assert_eq!(data.len(), width * height, "taille du tampon");
        Gray { width, height, data }
    }

    /// Luminance d'une image (Rec. 601) ; une image à 1 ou 2 canaux est déjà grise.
    pub fn from_image(im: &Image) -> Gray {
        let data = im
            .data
            .chunks_exact(im.channels)
            .map(|p| match im.channels {
                1 | 2 => p[0] as f32,
                _ => 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32,
            })
            .collect();
        Gray::new(im.width, im.height, data)
    }

    fn at(&self, x: i32, y: i32) -> f32 {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.data[y * self.width + x]
    }

    /// Valeur bilinéaire en (x, y), les bords prolongés.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let (fx, fy) = (x.floor(), y.floor());
        let (a, b) = (x - fx, y - fy);
        let (ix, iy) = (fx as i32, fy as i32);
        let top = self.at(ix, iy) * (1.0 - a) + self.at(ix + 1, iy) * a;
        let bottom = self.at(ix, iy + 1) * (1.0 - a) + self.at(ix + 1, iy + 1) * a;
        top * (1.0 - b) + bottom * b
    }

    /// Image deux fois plus petite, filtrée par [1 3 3 1] / 8 dans chaque sens (une simple
    /// moyenne 2×2 laisse passer les détails fins, qui se replient en fausses ondes). Le pixel
    /// (i, j) a son centre en (2i + 0,5, 2j + 0,5) dans l'image d'origine.
    fn half(&self) -> Gray {
        const K: [f32; 4] = [0.125, 0.375, 0.375, 0.125];
        let (w, h) = (self.width.div_ceil(2), self.height.div_ceil(2));
        // D'abord les lignes (largeur divisée par 2), puis les colonnes.
        let mut rows = Vec::with_capacity(w * self.height);
        for y in 0..self.height as i32 {
            for x in 0..w as i32 {
                rows.push((0..4).map(|k| K[k] * self.at(2 * x - 1 + k as i32, y)).sum::<f32>());
            }
        }
        let rows = Gray::new(w, self.height, rows);
        let mut data = Vec::with_capacity(w * h);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                data.push((0..4).map(|k| K[k] * rows.at(x, 2 * y - 1 + k as i32)).sum::<f32>());
            }
        }
        Gray::new(w, h, data)
    }

    /// Dérivées en x et en y (Sobel divisé par 8 : une vraie pente, en valeur par pixel).
    fn gradients(&self) -> (Vec<f32>, Vec<f32>) {
        let (mut gx, mut gy) = (Vec::with_capacity(self.data.len()), Vec::with_capacity(self.data.len()));
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                let p = |dx: i32, dy: i32| self.at(x + dx, y + dy);
                gx.push((p(1, -1) + 2.0 * p(1, 0) + p(1, 1) - p(-1, -1) - 2.0 * p(-1, 0) - p(-1, 1)) / 8.0);
                gy.push((p(-1, 1) + 2.0 * p(0, 1) + p(1, 1) - p(-1, -1) - 2.0 * p(0, -1) - p(1, -1)) / 8.0);
            }
        }
        (gx, gy)
    }
}

/// Champ de déplacements : `data[y * largeur + x] = [dx, dy]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    pub width: usize,
    pub height: usize,
    pub data: Vec<[f32; 2]>,
}

impl Flow {
    pub fn zero(width: usize, height: usize) -> Flow {
        Flow { width, height, data: vec![[0.0; 2]; width * height] }
    }

    fn at(&self, x: i32, y: i32) -> [f32; 2] {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.data[y * self.width + x]
    }

    /// Déplacement bilinéaire en (x, y), les bords prolongés.
    pub fn sample(&self, x: f32, y: f32) -> [f32; 2] {
        let (fx, fy) = (x.floor(), y.floor());
        let (a, b) = (x - fx, y - fy);
        let (ix, iy) = (fx as i32, fy as i32);
        let (p00, p10, p01, p11) = (self.at(ix, iy), self.at(ix + 1, iy), self.at(ix, iy + 1), self.at(ix + 1, iy + 1));
        let mix = |k: usize| (p00[k] * (1.0 - a) + p10[k] * a) * (1.0 - b) + (p01[k] * (1.0 - a) + p11[k] * a) * b;
        [mix(0), mix(1)]
    }

    /// Le même flux à une autre taille : positions et longueurs mises à l'échelle.
    pub fn resized(&self, width: usize, height: usize) -> Flow {
        let (sx, sy) = (self.width as f32 / width as f32, self.height as f32 / height as f32);
        let mut data = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let u = self.sample((x as f32 + 0.5) * sx - 0.5, (y as f32 + 0.5) * sy - 0.5);
                data.push([u[0] / sx, u[1] / sy]);
            }
        }
        Flow { width, height, data }
    }
}

/// Réglages, proches du préréglage « fast » d'OpenCV.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowParams {
    /// Côté des patchs.
    pub patch_size: usize,
    /// Écart entre deux patchs voisins (plus petit que le côté : ils se chevauchent).
    pub stride: usize,
    /// Itérations de Lucas-Kanade par patch.
    pub iters: usize,
    /// Niveau le plus fin calculé (0 = pleine taille, 1 = moitié…), le flux est ensuite agrandi.
    pub finest_level: usize,
    /// Nombre de fils de calcul ; 0 = tous les cœurs.
    pub threads: usize,
}

impl Default for FlowParams {
    fn default() -> FlowParams {
        FlowParams { patch_size: 8, stride: 4, iters: 16, finest_level: 0, threads: 0 }
    }
}

/// Le flux de `from` vers `to` : pour chaque pixel x de `from`, `from(x) ≈ to(x + flux(x))`.
pub fn flow(from: &Gray, to: &Gray, params: &FlowParams) -> Flow {
    assert_eq!((from.width, from.height), (to.width, to.height), "deux images de même taille");
    let patch = params.patch_size.max(2);
    let threads = match params.threads {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    };
    // Pyramide : on divise par 2 tant que le plus petit côté garde au moins deux patchs.
    let mut levels = vec![(from.clone(), to.clone())];
    loop {
        let (a, b) = levels.last().unwrap();
        if a.width.min(a.height) / 2 < 2 * patch {
            break;
        }
        let next = (a.half(), b.half());
        levels.push(next);
    }
    let finest = params.finest_level.min(levels.len() - 1);
    let mut current: Option<Flow> = None;
    for (i0, i1) in levels[finest..].iter().rev() {
        let init = match current.take() {
            Some(f) => f.resized(i0.width, i0.height),
            None => Flow::zero(i0.width, i0.height),
        };
        current = Some(level_flow(i0, i1, &init, patch, params.stride.max(1), params.iters, threads));
    }
    let result = current.expect("au moins un niveau");
    if finest == 0 {
        result
    } else {
        result.resized(from.width, from.height)
    }
}

/// Positions des patchs sur une ligne de `len` pixels : tous les `stride`, le dernier collé au bord.
fn grid(len: usize, patch: usize, stride: usize) -> Vec<usize> {
    if len <= patch {
        return vec![0];
    }
    let mut v: Vec<usize> = (0..=len - patch).step_by(stride).collect();
    if *v.last().unwrap() != len - patch {
        v.push(len - patch);
    }
    v
}

/// Tampons d'un fil pour la recherche d'un patch.
struct Scratch {
    t: Vec<f32>,
    tx: Vec<f32>,
    ty: Vec<f32>,
    res: Vec<f32>,
}

impl Scratch {
    fn new(patch: usize) -> Scratch {
        let n = patch * patch;
        Scratch { t: vec![0.0; n], tx: vec![0.0; n], ty: vec![0.0; n], res: vec![0.0; n] }
    }
}

/// Recherche du déplacement d'un patch : Lucas-Kanade inverse compositionnel.
struct PatchSearch<'a> {
    i0: &'a Gray,
    i1: &'a Gray,
    gx: &'a [f32],
    gy: &'a [f32],
    patch: usize,
    iters: usize,
}

impl PatchSearch<'_> {
    /// Le patch de `i0` dont le coin est (px, py) : on part du meilleur des candidats, puis on
    /// affine.
    fn refine(&self, px: usize, py: usize, candidates: impl Iterator<Item = [f32; 2]>, s: &mut Scratch) -> [f32; 2] {
        let (patch, w, n) = (self.patch, self.i0.width, self.patch * self.patch);
        // Le patch du modèle, ses pentes et la matrice de Lucas-Kanade.
        let (mut hxx, mut hxy, mut hyy) = (0f32, 0f32, 0f32);
        for j in 0..patch {
            for i in 0..patch {
                let (k, o) = (j * patch + i, (py + j) * w + px + i);
                (s.t[k], s.tx[k], s.ty[k]) = (self.i0.data[o], self.gx[o], self.gy[o]);
                hxx += s.tx[k] * s.tx[k];
                hxy += s.tx[k] * s.ty[k];
                hyy += s.ty[k] * s.ty[k];
            }
        }
        let (fx, fy) = (px as f32, py as f32);
        // Écart (sans la différence moyenne de luminosité) du patch déplacé de u.
        let cost = |u: [f32; 2], t: &[f32]| {
            let (mut sum, mut sum2) = (0.0, 0.0);
            for j in 0..patch {
                for i in 0..patch {
                    let d = self.i1.sample(fx + i as f32 + u[0], fy + j as f32 + u[1]) - t[j * patch + i];
                    sum += d;
                    sum2 += d * d;
                }
            }
            sum2 - sum * sum / n as f32
        };
        let mut best = (f32::MAX, [0f32; 2]);
        for cand in candidates {
            let e = cost(cand, &s.t);
            if e < best.0 {
                best = (e, cand);
            }
        }
        let (start, mut u) = (best.1, best.1);
        let det = hxx * hyy - hxy * hxy;
        if det <= 1e-6 * (hxx + hyy).powi(2).max(1e-6) {
            return u;
        }
        for _ in 0..self.iters {
            // Résidu centré, puis b = Σ ∇T · résidu et Δu = H⁻¹ b.
            let mut mean = 0.0;
            for j in 0..patch {
                for i in 0..patch {
                    let k = j * patch + i;
                    s.res[k] = self.i1.sample(fx + i as f32 + u[0], fy + j as f32 + u[1]) - s.t[k];
                    mean += s.res[k];
                }
            }
            mean /= n as f32;
            let (mut bx, mut by) = (0f32, 0f32);
            for k in 0..n {
                bx += s.tx[k] * (s.res[k] - mean);
                by += s.ty[k] * (s.res[k] - mean);
            }
            let dx = (hyy * bx - hxy * by) / det;
            let dy = (hxx * by - hxy * bx) / det;
            u = [u[0] - dx, u[1] - dy];
            if dx * dx + dy * dy < 1e-4 {
                break;
            }
        }
        // Un patch qui part trop loin s'est perdu : il garde son départ.
        let (ddx, ddy) = (u[0] - start[0], u[1] - start[1]);
        if ddx * ddx + ddy * ddy > (patch * patch) as f32 || !u[0].is_finite() || !u[1].is_finite() {
            start
        } else {
            u
        }
    }
}

/// Un niveau de la pyramide : recherche inverse de chaque patch, puis flux dense.
fn level_flow(i0: &Gray, i1: &Gray, init: &Flow, patch: usize, stride: usize, iters: usize, threads: usize) -> Flow {
    let (w, h) = (i0.width, i0.height);
    let patch = patch.min(w).min(h);
    let (xs, ys) = (grid(w, patch, stride), grid(h, patch, stride));
    let cols = xs.len();
    let (gx, gy) = i0.gradients();

    let rows = ys.len();
    let search = PatchSearch { i0, i1, gx: &gx, gy: &gy, patch, iters };

    // Déplacement de chaque patch, par bandes de lignes de patchs (une par fil). Passe aller : on
    // part du flux du niveau précédent ou du voisin de gauche ou du dessus (même bande). Passe
    // retour, en sens inverse : on part de la passe aller ou d'un des quatre voisins (toutes
    // bandes), ou du voisin de droite ou du dessous déjà repris : une erreur ne reste pas coincée
    // d'un côté d'une frontière entre bandes.
    let pass = |forward: bool, before: Option<&[[f32; 2]]>, row0: usize, out: &mut [[f32; 2]]| {
        let mut scratch = Scratch::new(patch);
        let band_rows = out.len() / cols;
        let mut last_row: Vec<[f32; 2]> = Vec::new();
        for k in 0..band_rows {
            let r = if forward { k } else { band_rows - 1 - k };
            let gr = row0 + r;
            for m in 0..cols {
                let c = if forward { m } else { cols - 1 - m };
                let (px, py) = (xs[c], ys[gr]);
                let mut cands = [None; 7];
                match before {
                    None => {
                        let half = (patch as f32 - 1.0) * 0.5;
                        cands[0] = Some(init.sample(px as f32 + half, py as f32 + half));
                    }
                    Some(b) => {
                        cands[0] = Some(b[gr * cols + c]);
                        cands[1] = (c > 0).then(|| b[gr * cols + c - 1]);
                        cands[2] = (c + 1 < cols).then(|| b[gr * cols + c + 1]);
                        cands[3] = (gr > 0).then(|| b[(gr - 1) * cols + c]);
                        cands[4] = (gr + 1 < rows).then(|| b[(gr + 1) * cols + c]);
                    }
                }
                let side = if forward { c.checked_sub(1) } else { (c + 1 < cols).then_some(c + 1) };
                cands[5] = side.map(|s| out[r * cols + s]);
                cands[6] = (k > 0).then(|| last_row[c]);
                out[r * cols + c] = search.refine(px, py, cands.iter().flatten().copied(), &mut scratch);
            }
            last_row.clear();
            last_row.extend_from_slice(&out[r * cols..(r + 1) * cols]);
        }
    };
    let rows_per_band = rows.div_ceil(threads.max(1)).max(1);
    let run = |found: &mut [[f32; 2]], band: &(dyn Fn(usize, &mut [[f32; 2]]) + Sync)| {
        if threads <= 1 {
            band(0, found);
            return;
        }
        std::thread::scope(|scope| {
            for (b, out) in found.chunks_mut(rows_per_band * cols).enumerate() {
                scope.spawn(move || band(b * rows_per_band, out));
            }
        });
    };
    let mut found = vec![[0f32; 2]; cols * rows];
    run(&mut found, &|row0, out| pass(true, None, row0, out));
    let before = found.clone();
    run(&mut found, &|row0, out| pass(false, Some(&before), row0, out));

    // Flux dense : chaque pixel prend la moyenne des patchs qui le couvrent, pondérée par
    // 1 / max(1, écart de couleur).
    let covers = |pos: &[usize], len: usize| {
        let mut v = vec![Vec::new(); len];
        for (k, &p) in pos.iter().enumerate() {
            for item in v.iter_mut().skip(p).take(patch) {
                item.push(k);
            }
        }
        v
    };
    let (cover_x, cover_y) = (covers(&xs, w), covers(&ys, h));
    let mut data = vec![[0f32; 2]; w * h];
    let dense_rows = |y0: usize, rows: &mut [[f32; 2]]| {
        for (r, row) in rows.chunks_mut(w).enumerate() {
            let y = y0 + r;
            for (x, out) in row.iter_mut().enumerate() {
                let v0 = i0.data[y * w + x];
                let (mut sx, mut sy, mut sw) = (0f32, 0f32, 0f32);
                for &py in &cover_y[y] {
                    for &px in &cover_x[x] {
                        let u = found[py * cols + px];
                        let d = (i1.sample(x as f32 + u[0], y as f32 + u[1]) - v0).abs();
                        let wt = 1.0 / d.max(1.0);
                        sx += wt * u[0];
                        sy += wt * u[1];
                        sw += wt;
                    }
                }
                *out = [sx / sw, sy / sw];
            }
        }
    };
    let rows_per_band = h.div_ceil(threads.max(1)).max(1);
    if threads <= 1 {
        dense_rows(0, &mut data);
    } else {
        std::thread::scope(|scope| {
            for (b, rows) in data.chunks_mut(rows_per_band * w).enumerate() {
                let dense_rows = &dense_rows;
                scope.spawn(move || dense_rows(b * rows_per_band, rows));
            }
        });
    }
    Flow { width: w, height: h, data }
}
