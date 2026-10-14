//! Vue du repère 3D (2d) : caméra en orbite autour d'un point, perspective dans le cadre de
//! l'image, fil de fer des arêtes utiles, arêtes cachées masquées (tampon de profondeur calculé
//! par Pelure). Le résultat est un ensemble de segments en pixels du cadre, posés sur la feuille
//! comme un guide : ils défilent avec la caméra 2D comme un décor dessiné au plan de référence.

use eframe::egui::{pos2, Pos2};

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
    pub fn to_camera(&self, p: V3) -> V3 {
        let (r, u, f) = self.basis();
        let d = sub(p, self.eye());
        [dot(d, r), dot(d, u), dot(d, f)]
    }

    /// Point du cadre (pixels, origine en haut à gauche) ; rien s'il est derrière l'œil.
    pub fn project(&self, p: V3, frame: [f32; 2]) -> Option<Pos2> {
        let c = self.to_camera(p);
        (c[2] > 1e-6).then(|| {
            let s = self.pixels_per_unit(frame[0]);
            pos2(frame[0] / 2.0 + c[0] / c[2] * s, frame[1] / 2.0 - c[1] / c[2] * s)
        })
    }

    /// Cadre tout le modèle (`bounds`), vu un peu de côté et d'au-dessus.
    pub fn fit(bounds: (V3, V3), frame: [f32; 2]) -> Cam3d {
        let (lo, hi) = bounds;
        let center = scale(add(lo, hi), 0.5);
        let radius = (dot(sub(hi, lo), sub(hi, lo)).sqrt() / 2.0).max(1e-3);
        let mut cam = Cam3d { target: center, azimuth: 35.0, elevation: 20.0, distance: 1.0, focal: 35.0 };
        let tan_h = 18.0 / cam.focal;
        let tan_v = tan_h * frame[1] / frame[0].max(1.0);
        let half = tan_h.min(tan_v).atan();
        cam.distance = radius / half.sin() * 1.1;
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

/// Tampon de profondeur : 1/z le plus grand (le plus proche) par pixel.
struct DepthBuffer {
    w: usize,
    h: usize,
    /// Pixels du tampon par pixel du cadre.
    k: f32,
    data: Vec<f32>,
}

impl DepthBuffer {
    fn new(frame: [f32; 2], max_w: usize) -> DepthBuffer {
        let k = (max_w as f32 / frame[0].max(1.0)).min(1.0);
        let (w, h) = (((frame[0] * k).ceil() as usize).max(1), ((frame[1] * k).ceil() as usize).max(1));
        DepthBuffer { w, h, k, data: vec![0.0; w * h] }
    }

    /// Triangle en pixels du tampon, avec 1/z à chaque sommet (interpolé linéairement à l'écran,
    /// ce qui est exact en perspective).
    fn triangle(&mut self, p: [(f32, f32, f32); 3]) {
        let (x0, x1) = (p.iter().map(|v| v.0).fold(f32::MAX, f32::min), p.iter().map(|v| v.0).fold(f32::MIN, f32::max));
        let (y0, y1) = (p.iter().map(|v| v.1).fold(f32::MAX, f32::min), p.iter().map(|v| v.1).fold(f32::MIN, f32::max));
        if x1 < 0.0 || y1 < 0.0 || x0 >= self.w as f32 || y0 >= self.h as f32 {
            return;
        }
        let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
        if area.abs() < 1e-9 {
            return;
        }
        let (xa, xb) = (x0.max(0.0).floor() as usize, (x1.ceil() as usize).min(self.w - 1));
        let (ya, yb) = (y0.max(0.0).floor() as usize, (y1.ceil() as usize).min(self.h - 1));
        for y in ya..=yb {
            let py = y as f32 + 0.5;
            for x in xa..=xb {
                let px = x as f32 + 0.5;
                let w0 = ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) / area;
                let w1 = ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let invz = w0 * p[0].2 + w1 * p[1].2 + w2 * p[2].2;
                let d = &mut self.data[y * self.w + x];
                if invz > *d {
                    *d = invz;
                }
            }
        }
    }

    /// Le plus lointain (1/z le plus petit) autour du pixel (x, y) : une arête au bord d'une
    /// surface n'est pas cachée par cette surface elle-même.
    fn farthest_around(&self, x: f32, y: f32) -> Option<f32> {
        let (cx, cy) = ((x * self.k) as isize, (y * self.k) as isize);
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
/// masquer les arêtes cachées par des faces plus proches.
pub fn wireframe(model: &Model3d, cam: &Cam3d, frame: [f32; 2], hidden: bool) -> Vec<[Pos2; 2]> {
    let pts: Vec<V3> = model.mesh.positions.iter().map(|&p| cam.to_camera(p)).collect();
    let near = (cam.distance * 0.002).max(1e-4);
    let s = cam.pixels_per_unit(frame[0]);
    let screen = |c: V3| (frame[0] / 2.0 + c[0] / c[2] * s, frame[1] / 2.0 - c[1] / c[2] * s);
    let depth = hidden.then(|| {
        let mut zb = DepthBuffer::new(frame, 1280);
        let k = zb.k;
        for t in &model.mesh.triangles {
            let v = t.map(|i| pts[i as usize]);
            // Coupe au plan proche : 0, 1 ou 2 triangles.
            let inside: Vec<V3> = {
                let mut poly = Vec::with_capacity(4);
                for i in 0..3 {
                    let (a, b) = (v[i], v[(i + 1) % 3]);
                    if a[2] >= near {
                        poly.push(a);
                    }
                    if (a[2] >= near) != (b[2] >= near) {
                        let t = (near - a[2]) / (b[2] - a[2]);
                        poly.push(add(a, scale(sub(b, a), t)));
                    }
                }
                poly
            };
            if inside.len() < 3 {
                continue;
            }
            let proj: Vec<(f32, f32, f32)> = inside
                .iter()
                .map(|&c| {
                    let (x, y) = screen(c);
                    (x * k, y * k, 1.0 / c[2])
                })
                .collect();
            for i in 1..proj.len() - 1 {
                zb.triangle([proj[0], proj[i], proj[i + 1]]);
            }
        }
        zb
    });
    let mut out = Vec::new();
    for e in &model.edges {
        let Some((a, b)) = clip_near(pts[e[0] as usize], pts[e[1] as usize], near) else { continue };
        let (pa, pb) = (screen(a), screen(b));
        let (pa, pb) = (pos2(pa.0, pa.1), pos2(pb.0, pb.1));
        if !(pa.x.is_finite() && pa.y.is_finite() && pb.x.is_finite() && pb.y.is_finite()) {
            continue;
        }
        let Some(zb) = &depth else {
            out.push([pa, pb]);
            continue;
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
    }
    out
}
