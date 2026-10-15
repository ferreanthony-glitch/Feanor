//! Vue du repère 3D (2d) : caméra en orbite autour d'un point, perspective dans le cadre de
//! l'image, fil de fer des arêtes utiles, arêtes cachées masquées (tampon de profondeur calculé
//! par Pelure). Le résultat est un ensemble de segments en pixels du cadre, posés sur la feuille
//! comme un guide : ils défilent avec la caméra 2D comme un décor dessiné au plan de référence.
//!
//! Un décor de production compte facilement un ou deux millions de triangles et des centaines de
//! milliers d'arêtes : les calculs sont répartis sur les cœurs du processeur, et le fil de fer est
//! tracé une fois dans une image (`rasterize`) plutôt que segment par segment à chaque affichage.

use eframe::egui::{pos2, Pos2, Rect};

use crate::model3d::Model3d;

/// Caméra du repère : elle regarde `target` depuis la distance `distance`, en faisant le tour
/// du modèle (`azimuth`, degrés, 0 = depuis +Z) à une hauteur de regard (`elevation`, degrés,
/// 90 = du dessus). Focale en mm pour un format de 36 mm de large (35 = grand-angle, 50 =
/// normal, 85 = serré).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam3d {
    pub target: [f32; 3],
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub focal: f32,
}

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, k: f32) -> V3 {
    a.map(|v| v * k)
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-20);
    scale(a, 1.0 / l)
}

/// Hauteur de regard maximale (au-delà, la caméra se retournerait).
pub const MAX_ELEVATION: f32 = 89.0;

impl Cam3d {
    /// Direction du point visé vers l'œil.
    fn dir(&self) -> V3 {
        let (a, e) = (self.azimuth.to_radians(), self.elevation.clamp(-MAX_ELEVATION, MAX_ELEVATION).to_radians());
        [e.cos() * a.sin(), e.sin(), e.cos() * a.cos()]
    }

    pub fn eye(&self) -> V3 {
        add(self.target, scale(self.dir(), self.distance))
    }

    /// Repère de la caméra : droite, haut, avant (vers le point visé).
    pub fn basis(&self) -> (V3, V3, V3) {
        let forward = scale(self.dir(), -1.0);
        let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
        let up = cross(right, forward);
        (right, up, forward)
    }

    /// Pixels du cadre par unité de x/z (largeur du cadre ↔ 36 mm de pellicule).
    pub fn pixels_per_unit(&self, frame_w: f32) -> f32 {
        frame_w / 2.0 * self.focal.max(1.0) / 18.0
    }

    /// Point dans le repère de la caméra : (droite, haut, profondeur devant l'œil).
    #[cfg(test)]
    pub fn to_camera(&self, p: V3) -> V3 {
        self.transform()(p)
    }

    /// `to_camera` avec le repère calculé une fois (pour beaucoup de points).
    fn transform(&self) -> impl Fn(V3) -> V3 + Sync {
        let (r, u, f) = self.basis();
        let eye = self.eye();
        move |p| {
            let d = sub(p, eye);
            [dot(d, r), dot(d, u), dot(d, f)]
        }
    }

    /// Point du cadre (pixels, origine en haut à gauche) ; rien s'il est derrière l'œil.
    #[cfg(test)]
    pub fn project(&self, p: V3, frame: [f32; 2]) -> Option<Pos2> {
        let c = self.to_camera(p);
        (c[2] > 1e-6).then(|| {
            let s = self.pixels_per_unit(frame[0]);
            pos2(frame[0] / 2.0 + c[0] / c[2] * s, frame[1] / 2.0 - c[1] / c[2] * s)
        })
    }

    /// Cadre tout le modèle (`bounds`), vu un peu de côté et d'au-dessus : la caméra recule juste
    /// assez pour que les 8 coins de la boîte tiennent dans le cadre (à 92 %). Un décor plat et
    /// large remplit ainsi le cadre, au lieu d'y tenir dans une sphère qui l'entoure.
    pub fn fit(bounds: (V3, V3), frame: [f32; 2]) -> Cam3d {
        Self::fit_at(bounds, frame, 35.0, 20.0)
    }

    /// Même cadrage, vu sous l'angle donné (degrés : `azimuth` autour, `elevation` au-dessus).
    pub fn fit_at(bounds: (V3, V3), frame: [f32; 2], azimuth: f32, elevation: f32) -> Cam3d {
        let (lo, hi) = bounds;
        let center = scale(add(lo, hi), 0.5);
        let radius = (dot(sub(hi, lo), sub(hi, lo)).sqrt() / 2.0).max(1e-3);
        let mut cam = Cam3d { target: center, azimuth, elevation, distance: 1.0, focal: 35.0 };
        let (r, u, f) = cam.basis();
        let s = cam.pixels_per_unit(frame[0]);
        let (hw, hh) = ((frame[0] * 0.46).max(1.0), (frame[1] * 0.46).max(1.0));
        // Un coin à (x, y) du centre et `ahead` plus loin que lui, vu à la distance D : sa
        // profondeur est D + ahead, et il tient si |x|·s ≤ hw·(D + ahead).
        let mut distance = radius * 1e-3;
        for i in 0..8 {
            let corner = [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] });
            let v = sub(corner, center);
            let ahead = dot(v, f);
            distance = distance
                .max(dot(v, r).abs() * s / hw - ahead)
                .max(dot(v, u).abs() * s / hh - ahead)
                .max(radius * 0.01 - ahead);
        }
        cam.distance = distance;
        cam
    }

    /// Tourner autour du point visé.
    pub fn orbit(&mut self, d_azimuth: f32, d_elevation: f32) {
        self.azimuth = (self.azimuth + d_azimuth).rem_euclid(360.0);
        self.elevation = (self.elevation + d_elevation).clamp(-MAX_ELEVATION, MAX_ELEVATION);
    }

    /// Glisser la vue de (`dx`, `dy`) pixels du cadre : le point visé suit la souris.
    pub fn pan(&mut self, dx: f32, dy: f32, frame_w: f32) {
        let (r, u, _) = self.basis();
        let k = self.distance / self.pixels_per_unit(frame_w);
        self.target = add(self.target, add(scale(r, -dx * k), scale(u, dy * k)));
    }

    /// Avancer (`factor` < 1) ou reculer (> 1).
    pub fn dolly(&mut self, factor: f32) {
        self.distance = (self.distance * factor).max(1e-4);
    }
}

/// Coupe le segment [a, b] (repère caméra) au plan proche `near` ; rien s'il est tout derrière.
fn clip_near(a: V3, b: V3, near: f32) -> Option<(V3, V3)> {
    match (a[2] >= near, b[2] >= near) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (ina, _) => {
            let t = (near - a[2]) / (b[2] - a[2]);
            let m = add(a, scale(sub(b, a), t));
            Some(if ina { (a, m) } else { (m, b) })
        }
    }
}

/// Nombre de fils de calcul pour les gros modèles.
fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16)
}

/// `items.map(f)`, réparti sur plusieurs fils quand il y a beaucoup d'éléments.
fn par_map<T: Sync, U: Send + Copy + Default>(items: &[T], f: impl Fn(&T) -> U + Sync) -> Vec<U> {
    let mut out = vec![U::default(); items.len()];
    let chunk = items.len().div_ceil(threads()).max(4096);
    std::thread::scope(|sc| {
        for (src, dst) in items.chunks(chunk).zip(out.chunks_mut(chunk)) {
            let f = &f;
            sc.spawn(move || {
                for (s, d) in src.iter().zip(dst) {
                    *d = f(s);
                }
            });
        }
    });
    out
}

/// Tampon de profondeur : 1/z le plus grand (le plus proche) par pixel.
struct DepthBuffer {
    w: usize,
    h: usize,
    /// Coin de la zone couverte, en pixels du cadre.
    origin: Pos2,
    /// Pixels du tampon par pixel du cadre.
    k: f32,
    data: Vec<f32>,
}

/// Triangle (pixels du tampon, 1/z à chaque sommet, interpolé linéairement à l'écran, ce qui est
/// exact en perspective) dans la bande de lignes `y0..y0 + data.len() / w` du tampon.
fn raster_triangle(data: &mut [f32], w: usize, y0: usize, p: [[f32; 3]; 3]) {
    let y1 = y0 + data.len() / w;
    let (x0, x1) = (p[0][0].min(p[1][0]).min(p[2][0]), p[0][0].max(p[1][0]).max(p[2][0]));
    let (ya, yb) = (p[0][1].min(p[1][1]).min(p[2][1]), p[0][1].max(p[1][1]).max(p[2][1]));
    if x1 < 0.0 || yb < y0 as f32 || x0 >= w as f32 || ya >= y1 as f32 {
        return;
    }
    let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let (xa, xb) = (x0.max(0.0).floor() as usize, (x1.ceil() as usize).min(w - 1));
    let (ys, ye) = ((ya.max(y0 as f32).floor() as usize).max(y0), (yb.ceil() as usize).min(y1 - 1));
    for y in ys..=ye {
        let py = y as f32 + 0.5;
        let row = &mut data[(y - y0) * w..(y - y0 + 1) * w];
        for (x, d) in row.iter_mut().enumerate().take(xb + 1).skip(xa) {
            let px = x as f32 + 0.5;
            let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) / area;
            let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let invz = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
            if invz > *d {
                *d = invz;
            }
        }
    }
}

impl DepthBuffer {
    /// Tampon sur la zone `area` (pixels du cadre), au plus `max_w` pixels de large.
    fn new(area: Rect, max_w: usize) -> DepthBuffer {
        let k = (max_w as f32 / area.width().max(1.0)).min(1.0);
        let (w, h) = (((area.width() * k).ceil() as usize).max(1), ((area.height() * k).ceil() as usize).max(1));
        DepthBuffer { w, h, origin: area.min, k, data: vec![0.0; w * h] }
    }

    /// Le plus lointain (1/z le plus petit) autour du point (x, y) du cadre : une arête au bord
    /// d'une surface n'est pas cachée par cette surface elle-même.
    fn farthest_around(&self, x: f32, y: f32) -> Option<f32> {
        let (cx, cy) = (((x - self.origin.x) * self.k).floor() as isize, ((y - self.origin.y) * self.k).floor() as isize);
        let mut m: Option<f32> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (px, py) = (cx + dx, cy + dy);
                if px < 0 || py < 0 || px >= self.w as isize || py >= self.h as isize {
                    continue;
                }
                let v = self.data[py as usize * self.w + px as usize];
                m = Some(m.map_or(v, |m: f32| m.min(v)));
            }
        }
        m
    }
}

/// Fil de fer du modèle vu par `cam`, en segments en pixels du cadre `frame`. `hidden` :
/// masquer les arêtes cachées par des faces plus proches. (Pelure utilise `wireframe_in`, sur
/// toute la feuille.)
#[cfg(test)]
pub fn wireframe(model: &Model3d, cam: &Cam3d, frame: [f32; 2], hidden: bool) -> Vec<[Pos2; 2]> {
    let area = Rect::from_min_max(Pos2::ZERO, pos2(frame[0], frame[1]));
    wireframe_in(model, cam, frame, area, hidden)
}

/// Comme `wireframe`, avec les arêtes cachées calculées sur la zone `area` (pixels du cadre ;
/// par exemple toute la feuille autour du cadre). Hors de cette zone, rien ne cache rien.
pub fn wireframe_in(model: &Model3d, cam: &Cam3d, frame: [f32; 2], area: Rect, hidden: bool) -> Vec<[Pos2; 2]> {
    let to_cam = cam.transform();
    let pts: Vec<V3> = par_map(&model.mesh.positions, |&p| to_cam(p));
    let near = (cam.distance * 0.002).max(1e-4);
    let s = cam.pixels_per_unit(frame[0]);
    let screen = |c: V3| (frame[0] / 2.0 + c[0] / c[2] * s, frame[1] / 2.0 - c[1] / c[2] * s);
    let depth = hidden.then(|| {
        // Environ 1300 pixels de tampon sur la largeur du cadre.
        let max_w = (1300.0 * area.width() / frame[0].max(1.0)).clamp(64.0, 4096.0) as usize;
        let mut zb = DepthBuffer::new(area, max_w);
        let (k, w, o) = (zb.k, zb.w, zb.origin);
        // Sommets devant le plan proche, en pixels du tampon (les autres : NaN, triangle à couper).
        let scr: Vec<[f32; 3]> = par_map(&pts, |c| {
            if c[2] >= near {
                let (x, y) = screen(*c);
                [(x - o.x) * k, (y - o.y) * k, 1.0 / c[2]]
            } else {
                [f32::NAN; 3]
            }
        });
        // Chaque fil remplit sa bande de lignes du tampon avec tous les triangles.
        let rows = zb.h.div_ceil(threads()).max(8);
        let (scr, pts) = (&scr, &pts);
        std::thread::scope(|sc| {
            for (band, data) in zb.data.chunks_mut(rows * w).enumerate() {
                sc.spawn(move || {
                    let y0 = band * rows;
                    for t in &model.mesh.triangles {
                        let v = t.map(|i| scr[i as usize]);
                        if !(v[0][0].is_nan() || v[1][0].is_nan() || v[2][0].is_nan()) {
                            raster_triangle(data, w, y0, v);
                            continue;
                        }
                        // Coupe au plan proche : 0, 1 ou 2 triangles.
                        let c = t.map(|i| pts[i as usize]);
                        let mut poly: Vec<[f32; 3]> = Vec::with_capacity(4);
                        for i in 0..3 {
                            let (a, b) = (c[i], c[(i + 1) % 3]);
                            if a[2] >= near {
                                let (x, y) = screen(a);
                                poly.push([(x - o.x) * k, (y - o.y) * k, 1.0 / a[2]]);
                            }
                            if (a[2] >= near) != (b[2] >= near) {
                                let m = add(a, scale(sub(b, a), (near - a[2]) / (b[2] - a[2])));
                                let (x, y) = screen(m);
                                poly.push([(x - o.x) * k, (y - o.y) * k, 1.0 / m[2]]);
                            }
                        }
                        for i in 1..poly.len().saturating_sub(1) {
                            raster_triangle(data, w, y0, [poly[0], poly[i], poly[i + 1]]);
                        }
                    }
                });
            }
        });
        zb
    });
    // Arêtes, par paquets sur plusieurs fils.
    let edge = |e: &[u32; 2], out: &mut Vec<[Pos2; 2]>| {
        let Some((a, b)) = clip_near(pts[e[0] as usize], pts[e[1] as usize], near) else { return };
        let (pa, pb) = (screen(a), screen(b));
        let (pa, pb) = (pos2(pa.0, pa.1), pos2(pb.0, pb.1));
        if !(pa.x.is_finite() && pa.y.is_finite() && pb.x.is_finite() && pb.y.is_finite()) {
            return;
        }
        let Some(zb) = &depth else {
            out.push([pa, pb]);
            return;
        };
        // Le long de l'arête, pixel par pixel du tampon : visible tant qu'aucune face n'est devant.
        let (ia, ib) = (1.0 / a[2], 1.0 / b[2]);
        let steps = ((pa.distance(pb) * zb.k).ceil() as usize).clamp(1, 20_000);
        let mut run: Option<Pos2> = None;
        let mut prev = pa;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let q = pa + (pb - pa) * t;
            let invz = ia + (ib - ia) * t;
            let visible = match zb.farthest_around(q.x, q.y) {
                Some(d) => invz >= d * 0.996,
                None => true, // hors du tampon (hors du cadre) : rien ne le cache
            };
            match (visible, run) {
                (true, None) => run = Some(q),
                (false, Some(start)) => {
                    out.push([start, prev]);
                    run = None;
                }
                _ => {}
            }
            prev = q;
        }
        if let Some(start) = run {
            out.push([start, pb]);
        }
    };
    let chunk = model.edges.len().div_ceil(threads()).max(2048);
    let parts: Vec<Vec<[Pos2; 2]>> = std::thread::scope(|sc| {
        let handles: Vec<_> = model
            .edges
            .chunks(chunk)
            .map(|edges| {
                let edge = &edge;
                sc.spawn(move || {
                    let mut out = Vec::new();
                    for e in edges {
                        edge(e, &mut out);
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    parts.concat()
}

/// Aperçu rapide pendant qu'on fait tourner la vue : les `limit` plus longues arêtes, sans
/// masquer les arêtes cachées.
pub fn preview(model: &Model3d, cam: &Cam3d, frame: [f32; 2], limit: usize) -> Vec<[Pos2; 2]> {
    let to_cam = cam.transform();
    let near = (cam.distance * 0.002).max(1e-4);
    let s = cam.pixels_per_unit(frame[0]);
    let screen = |c: V3| pos2(frame[0] / 2.0 + c[0] / c[2] * s, frame[1] / 2.0 - c[1] / c[2] * s);
    model
        .edges
        .iter()
        .take(limit)
        .filter_map(|e| {
            let (a, b) = clip_near(to_cam(model.mesh.positions[e[0] as usize]), to_cam(model.mesh.positions[e[1] as usize]), near)?;
            let (pa, pb) = (screen(a), screen(b));
            (pa.x.is_finite() && pa.y.is_finite() && pb.x.is_finite() && pb.y.is_finite()).then_some([pa, pb])
        })
        .collect()
}

/// Coupe le segment [a, b] au rectangle [lo, hi] (Liang-Barsky) ; rien s'il est dehors.
fn clip_rect(a: Pos2, b: Pos2, lo: Pos2, hi: Pos2) -> Option<(Pos2, Pos2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-d.x, a.x - lo.x), (d.x, hi.x - a.x), (-d.y, a.y - lo.y), (d.y, hi.y - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

/// Trace les segments (pixels du cadre, multipliés par `zoom`) dans un masque de couverture de
/// `size` pixels : 255 sur le trait, lissé sur les bords, épaisseur `width` pixels du masque.
pub fn rasterize(segs: &[[Pos2; 2]], size: [usize; 2], zoom: f32, width: f32) -> Vec<u8> {
    let [w, h] = size;
    let mut mask = vec![0u8; w * h];
    if w == 0 || h == 0 {
        return mask;
    }
    let r = (width * 0.5).max(0.3);
    // Bandes de lignes, chacune avec les segments qui la touchent ; les fils se les partagent au
    // fur et à mesure (un décor ramassé au milieu de l'image n'occupe pas qu'un seul fil).
    const ROWS: usize = 16;
    let bands = h.div_ceil(ROWS);
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); bands];
    for (i, s) in segs.iter().enumerate() {
        let (a, b) = (s[0] * zoom, s[1] * zoom);
        let (ya, yb) = (a.y.min(b.y) - r - 1.0, a.y.max(b.y) + r + 1.0);
        let (xa, xb) = (a.x.min(b.x) - r - 1.0, a.x.max(b.x) + r + 1.0);
        // (Les comparaisons écartent aussi les valeurs non finies.)
        if !(yb >= 0.0 && ya < h as f32 && xb >= 0.0 && xa < w as f32) {
            continue;
        }
        let first = (ya.max(0.0) as usize / ROWS).min(bands - 1);
        let last = (yb.min(h as f32 - 1.0) as usize / ROWS).min(bands - 1);
        for bucket in &mut buckets[first..=last] {
            bucket.push(i as u32);
        }
    }
    let work = std::sync::Mutex::new(mask.chunks_mut(ROWS * w).enumerate().zip(&buckets));
    std::thread::scope(|sc| {
        for _ in 0..threads() {
            sc.spawn(|| loop {
                let next = work.lock().ok().and_then(|mut it| it.next());
                let Some(((band, data), bucket)) = next else { break };
                let y0 = band * ROWS;
                let y1 = y0 + data.len() / w;
                let (lo, hi) = (pos2(-r - 1.0, y0 as f32 - r - 1.0), pos2(w as f32 + r + 1.0, y1 as f32 + r + 1.0));
                for &i in bucket {
                    let s = segs[i as usize];
                    let Some((a, b)) = clip_rect(s[0] * zoom, s[1] * zoom, lo, hi) else { continue };
                    let d = b - a;
                    let len2 = d.length_sq();
                    let mut plot = |x: isize, y: isize| {
                        if x < 0 || y < y0 as isize || x >= w as isize || y >= y1 as isize {
                            return;
                        }
                        let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let t = if len2 > 0.0 { ((p - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
                        let dist = p.distance(a + d * t);
                        let cover = (r + 0.5 - dist).clamp(0.0, 1.0);
                        if cover > 0.0 {
                            let px = &mut data[(y as usize - y0) * w + x as usize];
                            *px = (*px).max((cover * 255.0).round() as u8);
                        }
                    };
                    let reach = (r + 1.0).ceil() as isize;
                    // Le long de l'axe principal, une petite colonne de pixels en travers.
                    if d.x.abs() >= d.y.abs() {
                        let (from, to) = (a.x.min(b.x).floor() as isize - reach, a.x.max(b.x).ceil() as isize + reach);
                        for x in from..=to {
                            let t = if d.x != 0.0 { ((x as f32 + 0.5 - a.x) / d.x).clamp(0.0, 1.0) } else { 0.0 };
                            let yc = (a.y + d.y * t).floor() as isize;
                            for y in yc - reach..=yc + reach {
                                plot(x, y);
                            }
                        }
                    } else {
                        let (from, to) = (a.y.min(b.y).floor() as isize - reach, a.y.max(b.y).ceil() as isize + reach);
                        for y in from..=to {
                            let t = ((y as f32 + 0.5 - a.y) / d.y).clamp(0.0, 1.0);
                            let xc = (a.x + d.x * t).floor() as isize;
                            for x in xc - reach..=xc + reach {
                                plot(x, y);
                            }
                        }
                    }
                }
            });
        }
    });
    drop(work);
    mask
}
