//! Cœur de la synthèse, d'après `ebsynth_cpu.cpp` : champ des correspondances (pour chaque pixel
//! de la cible, le centre du patch du style qu'on y recopie), coût d'un patch, PatchMatch avec la
//! contrainte d'uniformité, vote, rééchantillonnage de la pyramide.

use std::sync::atomic::{AtomicI32, Ordering::Relaxed};

/// Image d'un niveau de la pyramide : canaux du style puis canaux des guides, entrelacés.
pub(crate) struct Plane {
    pub w: usize,
    pub h: usize,
    pub c: usize,
    pub data: Vec<u8>,
}

/// Hasard reproductible (SplitMix64).
pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as u32
    }

    fn below(&mut self, n: usize) -> i32 {
        (self.next_u32() % n as u32) as i32
    }
}

/// Le petit générateur qu'ebsynth fait tourner dans la recherche aléatoire (`RANDI`).
fn randi(u: u32) -> u32 {
    18000u32.wrapping_mul(u & 65535).wrapping_add(u >> 16)
}

/// Taille d'un niveau : la taille entière divisée par 2 autant de fois qu'il y a de niveaux plus
/// fins, arrondie vers le bas (`pyramidLevelSize`).
pub(crate) fn level_size(base: (usize, usize), levels: usize, level: usize) -> (usize, usize) {
    let f = 2f32.powi(-((levels - 1 - level) as i32));
    ((base.0 as f32 * f) as usize, (base.1 as f32 * f) as usize)
}

/// Rééchantillonnage bilinéaire (`resampleCPU`) : l'échelle de la largeur sert aussi pour la
/// hauteur, sans décalage d'un demi-pixel, valeurs tronquées. Pour un facteur entier, cela revient
/// à prendre un pixel sur deux (ou sur quatre…), comme l'original.
pub(crate) fn resample(src: &Plane, w: usize, h: usize) -> Plane {
    let c = src.c;
    let s = src.w as f32 / w as f32;
    let (sw, sh) = (src.w as i32, src.h as i32);
    let px = |x: i32, y: i32| ((y.clamp(0, sh - 1) * sw + x.clamp(0, sw - 1)) as usize) * c;
    let mut data = vec![0u8; w * h * c];
    for y in 0..h {
        for x in 0..w {
            let fx = s * x as f32;
            let fy = s * y as f32;
            let (ix, iy) = (fx as i32, fy as i32);
            let (a, b) = (fx - ix as f32, fy - iy as f32);
            let (p00, p10, p01, p11) = (px(ix, iy), px(ix + 1, iy), px(ix, iy + 1), px(ix + 1, iy + 1));
            let o = (y * w + x) * c;
            for k in 0..c {
                let v = (1.0 - a) * (1.0 - b) * src.data[p00 + k] as f32
                    + a * (1.0 - b) * src.data[p10 + k] as f32
                    + (1.0 - a) * b * src.data[p01 + k] as f32
                    + a * b * src.data[p11 + k] as f32;
                data[o + k] = v as u8;
            }
        }
    }
    Plane { w, h, c, data }
}

/// Correspondances tirées au hasard, centres de patch entièrement dans le style (`nnfInitRandom`).
pub(crate) fn nnf_random(target: (usize, usize), source: (usize, usize), patch: usize, rng: &mut Rng) -> Vec<[i32; 2]> {
    let r = (patch / 2) as i32;
    (0..target.0 * target.1)
        .map(|_| {
            let x = r + rng.below(source.0 - 2 * r as usize);
            let y = r + rng.below(source.1 - 2 * r as usize);
            [x, y]
        })
        .collect()
}

/// Passage au niveau deux fois plus fin (`nnfUpscale`) : chaque correspondance est doublée, le
/// pixel garde sa position dans le carré 2×2, puis on reste à `patch` pixels des bords du style.
pub(crate) fn nnf_upscale(
    nnf: &[[i32; 2]],
    size: (usize, usize),
    target: (usize, usize),
    source: (usize, usize),
    patch: usize,
) -> Vec<[i32; 2]> {
    let (w, h) = (size.0 as i32, size.1 as i32);
    let p = patch as i32;
    let (sw, sh) = (source.0 as i32, source.1 as i32);
    let mut out = Vec::with_capacity(target.0 * target.1);
    for y in 0..target.1 as i32 {
        for x in 0..target.0 as i32 {
            let n = nnf[((y / 2).clamp(0, h - 1) * w + (x / 2).clamp(0, w - 1)) as usize];
            out.push([
                (n[0] * 2 + x % 2).clamp(p, sw - p - 1),
                (n[1] * 2 + y % 2).clamp(p, sh - p - 1),
            ]);
        }
    }
    out
}

/// Vote simple (`krnlVotePlain`) : chaque pixel de la cible prend la moyenne (tronquée) des pixels
/// que lui donnent les patchs qui le recouvrent. Seuls les canaux du style sont écrits.
pub(crate) fn vote(target: &mut Plane, source: &Plane, ns: usize, nnf: &[[i32; 2]], patch: usize, threads: usize) {
    let (w, h, c) = (target.w, target.h, target.c);
    let r = (patch / 2) as i32;
    let rows_per_band = h.div_ceil(threads.max(1)).max(1);
    let band = |y0: usize, rows: &mut [u8]| {
        let mut sum = vec![0u32; ns];
        for (dy, row) in rows.chunks_exact_mut(w * c).enumerate() {
            let y = (y0 + dy) as i32;
            for x in 0..w as i32 {
                sum.fill(0);
                let mut count = 0u32;
                for py in -r..=r {
                    for px in -r..=r {
                        let (nx, ny) = (x + px, y + py);
                        if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                            continue;
                        }
                        let n = nnf[(ny * w as i32 + nx) as usize];
                        let (sx, sy) = (n[0] - px, n[1] - py);
                        if sx < 0 || sx >= source.w as i32 || sy < 0 || sy >= source.h as i32 {
                            continue;
                        }
                        let o = (sy as usize * source.w + sx as usize) * source.c;
                        for (k, s) in sum.iter_mut().enumerate() {
                            *s += source.data[o + k] as u32;
                        }
                        count += 1;
                    }
                }
                let o = x as usize * c;
                for (k, s) in sum.iter().enumerate() {
                    row[o + k] = (s / count) as u8;
                }
            }
        }
    };
    if threads <= 1 {
        band(0, &mut target.data);
        return;
    }
    std::thread::scope(|scope| {
        for (i, rows) in target.data.chunks_mut(rows_per_band * w * c).enumerate() {
            let band = &band;
            scope.spawn(move || band(i * rows_per_band, rows));
        }
    });
}

/// Ce que cherche PatchMatch à un niveau : la cible actuelle, le style, les poids par canal.
pub(crate) struct Search<'a> {
    pub target: &'a Plane,
    pub source: &'a Plane,
    pub weights: &'a [f32],
    pub patch_size: usize,
    pub uniformity: f32,
    pub iters: usize,
    pub threads: usize,
}

/// Coût d'un patch (`PatchSSD_Split`) : somme pondérée des carrés des écarts, canaux du style et
/// des guides confondus.
struct Cost<'a> {
    t: &'a Plane,
    s: &'a Plane,
    weights: &'a [f32],
    /// Poids d'une ligne de patch (les poids des canaux répétés `patch` fois).
    row_weights: Vec<f32>,
    patch: usize,
}

impl Cost<'_> {
    /// Le patch de la cible centré en (tx, ty) contre celui du style centré en (sx, sy), qui est
    /// toujours entier. On arrête dès qu'une ligne dépasse `ebest`, comme l'original.
    fn eval(&self, tx: i32, ty: i32, sx: i32, sy: i32, ebest: f32) -> f32 {
        let (t, s, c) = (self.t, self.s, self.t.c);
        let r = (self.patch / 2) as i32;
        let (tw, th) = (t.w as i32, t.h as i32);
        if tx - r >= 0 && tx + r < tw && ty - r >= 0 && ty + r < th {
            let len = self.patch * c;
            let mut error = 0.0;
            for j in 0..self.patch {
                let to = ((ty - r) as usize + j) * t.w * c + (tx - r) as usize * c;
                let so = ((sy - r) as usize + j) * s.w * c + (sx - r) as usize * c;
                error += weighted_ssd(&t.data[to..to + len], &s.data[so..so + len], &self.row_weights);
                if error > ebest {
                    break;
                }
            }
            error
        } else {
            let (sw, sh) = (s.w as i32, s.h as i32);
            let mut error = 0.0;
            for py in -r..=r {
                for px in -r..=r {
                    let to = ((ty + py).clamp(0, th - 1) * tw + (tx + px).clamp(0, tw - 1)) as usize * c;
                    let so = ((sy + py).clamp(0, sh - 1) * sw + (sx + px).clamp(0, sw - 1)) as usize * c;
                    for k in 0..c {
                        let d = t.data[to + k] as f32 - s.data[so + k] as f32;
                        error += self.weights[k] * d * d;
                    }
                }
            }
            error
        }
    }
}

/// Somme de w·(a − b)² sur une ligne, avec huit accumulateurs pour que le compilateur vectorise.
fn weighted_ssd(a: &[u8], b: &[u8], w: &[f32]) -> f32 {
    let mut acc = [0f32; 8];
    let (ca, cb, cw) = (a.chunks_exact(8), b.chunks_exact(8), w.chunks_exact(8));
    let (ra, rb, rw) = (ca.remainder(), cb.remainder(), cw.remainder());
    for ((a, b), w) in ca.zip(cb).zip(cw) {
        for l in 0..8 {
            let d = a[l] as f32 - b[l] as f32;
            acc[l] += w[l] * d * d;
        }
    }
    let mut e = acc.iter().sum::<f32>();
    for ((a, b), w) in ra.iter().zip(rb).zip(rw) {
        let d = *a as f32 - *b as f32;
        e += w * d * d;
    }
    e
}

/// Occupation du style (`Omega`) : combien de pixels de patchs de la cible recouvrent chaque
/// pixel du style. Partagée entre les fils, d'où les entiers atomiques.
struct Omega<'a> {
    counts: &'a [AtomicI32],
    w: usize,
    patch: usize,
}

impl Omega<'_> {
    fn sum(&self, b: [i32; 2]) -> i32 {
        let r = (self.patch / 2) as i32;
        let mut sum = 0;
        for j in 0..self.patch as i32 {
            let o = ((b[1] - r + j) * self.w as i32 + b[0] - r) as usize;
            sum += self.counts[o..o + self.patch].iter().map(|v| v.load(Relaxed)).sum::<i32>();
        }
        sum
    }

    fn add(&self, b: [i32; 2], inc: i32) {
        let r = (self.patch / 2) as i32;
        for j in 0..self.patch as i32 {
            let o = ((b[1] - r + j) * self.w as i32 + b[0] - r) as usize;
            for v in &self.counts[o..o + self.patch] {
                v.fetch_add(inc, Relaxed);
            }
        }
    }
}

/// Ce qu'un fil partage avec les autres pendant une passe.
struct Worker<'a> {
    cost: Cost<'a>,
    omega: Omega<'a>,
    /// Occupation idéale d'un pixel du style si tous étaient également utilisés.
    omega_best: f32,
    lambda: f32,
    irad: &'a [i32],
}

impl Worker<'_> {
    /// Essaie de remplacer la correspondance du pixel (x, y) par `b` (`tryPatch`) : on garde la
    /// meilleure selon le coût plus l'uniformité × l'occupation du patch du style.
    fn try_patch(&self, x: i32, y: i32, b: [i32; 2], cur: &mut [i32; 2], err: &mut f32) {
        if self.lambda == 0.0 {
            let new_err = self.cost.eval(x, y, b[0], b[1], *err);
            if new_err < *err {
                *cur = b;
                *err = new_err;
            }
            return;
        }
        let area = (self.cost.patch * self.cost.patch) as f32;
        let cur_occ = self.omega.sum(*cur) as f32 / area / self.omega_best;
        let new_occ = self.omega.sum(b) as f32 / area / self.omega_best;
        let cur_total = *err + self.lambda * cur_occ;
        let new_err = self.cost.eval(x, y, b[0], b[1], cur_total);
        if new_err + self.lambda * new_occ < cur_total {
            self.omega.add(b, 1);
            self.omega.add(*cur, -1);
            *cur = b;
            *err = new_err;
        }
    }

    /// Une passe de PatchMatch sur les lignes `y0..y1` (`nnf` et `err` ne contiennent qu'elles) :
    /// propagation depuis le voisin de gauche et celui du dessus (de droite et du dessous une passe
    /// sur deux), puis recherche aléatoire autour de la correspondance, dans des carrés de plus en
    /// plus petits.
    fn band(&self, y0: usize, y1: usize, iter: usize, iter_seed: u32, nnf: &mut [[i32; 2]], err: &mut [f32]) {
        let w = self.cost.t.w;
        let (sw, sh) = (self.cost.s.w as i32, self.cost.s.h as i32);
        let r = (self.cost.patch / 2) as i32;
        let forward = iter % 2 == 0;
        let q: i32 = if forward { 1 } else { -1 };
        for k in 0..y1 - y0 {
            let y = if forward { y0 + k } else { y1 - 1 - k };
            for m in 0..w {
                let x = if forward { m } else { w - 1 - m };
                let i = (y - y0) * w + x;
                let (xi, yi) = (x as i32, y as i32);
                if (forward && x > 0) || (!forward && x + 1 < w) {
                    let mut n = nnf[if forward { i - 1 } else { i + 1 }];
                    n[0] += q;
                    if (forward && n[0] < sw - r) || (!forward && n[0] >= r) {
                        self.try_patch(xi, yi, n, &mut nnf[i], &mut err[i]);
                    }
                }
                if (forward && y > y0) || (!forward && y + 1 < y1) {
                    let mut n = nnf[if forward { i - w } else { i + w }];
                    n[1] += q;
                    if (forward && n[1] < sh - r) || (!forward && n[1] >= r) {
                        self.try_patch(xi, yi, n, &mut nnf[i], &mut err[i]);
                    }
                }
                let mut seed = randi(((x as u32) | ((y as u32) << 11)) ^ iter_seed);
                let pix0 = nnf[i];
                for &rad in self.irad.iter().rev() {
                    let (tlx, tly) = ((pix0[0] - rad).max(r), (pix0[1] - rad).max(r));
                    let (brx, bry) = ((pix0[0] + rad).min(sw - r), (pix0[1] + rad).min(sh - r));
                    let rx = randi(seed);
                    let ry = randi(rx);
                    seed = ry;
                    let n = [tlx + (rx % (brx - tlx) as u32) as i32, tly + (ry % (bry - tly) as u32) as i32];
                    self.try_patch(xi, yi, n, &mut nnf[i], &mut err[i]);
                }
            }
        }
    }
}

/// PatchMatch avec uniformité (`patchmatch`) : améliore `nnf` en place ; `err` reçoit le coût de
/// chaque correspondance.
pub(crate) fn patchmatch(search: &Search, nnf: &mut [[i32; 2]], err: &mut [f32], rng: &mut Rng) {
    let (t, s) = (search.target, search.source);
    let patch = search.patch_size;
    let cost = Cost {
        t,
        s,
        weights: search.weights,
        row_weights: search.weights.iter().copied().cycle().take(patch * t.c).collect(),
        patch,
    };

    // Coût de départ de chaque correspondance.
    let rows_per_band = t.h.div_ceil(search.threads.max(1)).max(1);
    let eval_rows = |y0: usize, nnf: &[[i32; 2]], err: &mut [f32]| {
        for (i, e) in err.iter_mut().enumerate() {
            let (x, y) = ((i % t.w) as i32, (y0 + i / t.w) as i32);
            *e = cost.eval(x, y, nnf[i][0], nnf[i][1], f32::MAX);
        }
    };
    if search.threads <= 1 {
        eval_rows(0, nnf, err);
    } else {
        std::thread::scope(|scope| {
            let chunk = rows_per_band * t.w;
            for (i, (n, e)) in nnf.chunks(chunk).zip(err.chunks_mut(chunk)).enumerate() {
                let eval_rows = &eval_rows;
                scope.spawn(move || eval_rows(i * rows_per_band, n, e));
            }
        });
    }

    // Rayons de la recherche aléatoire : le plus grand côté du style, puis divisé par 2 jusqu'à 1.
    let mut irad = vec![s.w.max(s.h) as i32];
    while *irad.last().unwrap() != 1 {
        irad.push((0.5f64.powi(irad.len() as i32) * irad[0] as f64) as i32);
    }

    let counts: Vec<AtomicI32> = (0..s.w * s.h).map(|_| AtomicI32::new(0)).collect();
    let worker = Worker {
        omega: Omega { counts: &counts, w: s.w, patch },
        omega_best: (t.w * t.h) as f32 / (s.w * s.h) as f32 * (patch * patch) as f32,
        lambda: search.uniformity,
        irad: &irad,
        cost,
    };
    if worker.lambda != 0.0 {
        for &n in nnf.iter() {
            worker.omega.add(n, 1);
        }
    }

    // Bandes de lignes, une par fil, au moins 8 lignes chacune (comme l'original).
    let threads = search.threads.max(1);
    let tiles = if t.h.div_ceil(threads) > 8 { threads } else { t.h.div_ceil(8).max(1) };
    let tile_h = t.h / tiles;
    for iter in 0..search.iters {
        let iter_seed = rng.next_u32();
        if tiles == 1 {
            worker.band(0, t.h, iter, iter_seed, nnf, err);
            continue;
        }
        std::thread::scope(|scope| {
            let (mut nrest, mut erest) = (&mut *nnf, &mut *err);
            for tile in 0..tiles {
                let y0 = tile * tile_h;
                let y1 = if tile == tiles - 1 { t.h } else { (y0 + tile_h).min(t.h) };
                let (n, nr) = std::mem::take(&mut nrest).split_at_mut((y1 - y0) * t.w);
                let (e, er) = std::mem::take(&mut erest).split_at_mut((y1 - y0) * t.w);
                (nrest, erest) = (nr, er);
                let worker = &worker;
                scope.spawn(move || worker.band(y0, y1, iter, iter_seed, n, e));
            }
        });
    }
}
