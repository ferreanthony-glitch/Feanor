//! Vue du plateau en 3D (9e) : les calques du panneau sont des plans posés à leur profondeur, la
//! caméra du panneau est une pyramide de vision, un sol quadrillé donne l'échelle. On tourne
//! autour (orbite), on glisse la vue, on avance ; l'outil Déplacer bouge un calque dans son plan
//! ou en profondeur, ou la caméra.
//!
//! Le monde est celui du multiplan (`Layer::transform_seen`), en pixels de la feuille : x vers la
//! droite, y vers le haut, z vers la caméra, origine au centre de la feuille. Le plan de
//! référence (profondeur 0) est z = 0 et la caméra de référence est en z = D (distance de
//! référence). Un calque de profondeur d est en z = −D·d, agrandi de (1 + d) autour de l'axe de la
//! caméra de référence : vu par la caméra du panneau, il tombe là où le rendu 2D le met.

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::camera::{ref_distance, Camera};
use crate::document::Layer;
use crate::view3d::Cam3d;

pub type V3 = [f32; 3];

/// Plus près de l'œil que ça (pixels de la feuille), un point n'est pas dessiné.
pub const NEAR: f32 = 1.0;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Point entre `a` (t = 0) et `b` (t = 1).
pub fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Le plateau d'un panneau : taille de la feuille, cadre, distance de référence.
#[derive(Clone, Copy, Debug)]
pub struct World {
    pub stage: [f32; 2],
    pub frame: [usize; 2],
    pub d0: f32,
}

impl World {
    pub fn new(stage: [usize; 2], frame: [usize; 2]) -> World {
        World { stage: [stage[0] as f32, stage[1] as f32], frame, d0: ref_distance(frame) }
    }

    /// Agrandissement d'un plan de profondeur `depth` (bornée comme pour le rendu 2D).
    fn spread(depth: f32) -> f32 {
        1.0 + depth.max(Layer::MIN_DEPTH)
    }

    /// Hauteur z du plan de profondeur `depth`.
    pub fn plane_z(&self, depth: f32) -> f32 {
        -self.d0 * depth.max(Layer::MIN_DEPTH)
    }

    /// Profondeur du plan de hauteur `z`.
    pub fn depth_of(&self, z: f32) -> f32 {
        (-z / self.d0).max(Layer::MIN_DEPTH)
    }

    /// Point `p` de la feuille (pixels), posé sur le plan de profondeur `depth`.
    pub fn at_depth(&self, p: Pos2, depth: f32) -> V3 {
        let k = Self::spread(depth);
        [(p.x - self.stage[0] / 2.0) * k, -(p.y - self.stage[1] / 2.0) * k, self.plane_z(depth)]
    }

    /// Point de la feuille qui, posé sur le plan de profondeur `depth`, est en `w`.
    pub fn to_stage(&self, w: V3, depth: f32) -> Pos2 {
        let k = Self::spread(depth);
        pos2(w[0] / k + self.stage[0] / 2.0, -w[1] / k + self.stage[1] / 2.0)
    }

    /// Déplacement sur la feuille d'un glisser de `from` à `to` dans le plan de profondeur `depth`.
    pub fn stage_delta(&self, from: V3, to: V3, depth: f32) -> Vec2 {
        let k = Self::spread(depth);
        vec2((to[0] - from[0]) / k, -(to[1] - from[1]) / k)
    }

    /// Coins de la feuille sur le plan de profondeur `depth` : haut-gauche, haut-droite,
    /// bas-droite, bas-gauche.
    pub fn plane_corners(&self, depth: f32) -> [V3; 4] {
        let [w, h] = self.stage;
        [pos2(0.0, 0.0), pos2(w, 0.0), pos2(w, h), pos2(0.0, h)].map(|p| self.at_depth(p, depth))
    }

    /// Œil de la caméra du panneau.
    pub fn eye(&self, cam: &Camera) -> V3 {
        [cam.x - self.stage[0] / 2.0, -(cam.y - self.stage[1] / 2.0), self.d0 - cam.dolly]
    }

    /// Coins du cadre de la caméra sur le plan de référence.
    pub fn frame_corners(&self, cam: &Camera) -> [V3; 4] {
        cam.corners(self.frame).map(|p| self.at_depth(p, 0.0))
    }

    /// Le point `w` vu par la caméra du panneau, ramené sur le plan de référence (pixels de la
    /// feuille) : là où le rendu 2D le montre. Rien s'il n'est pas devant la caméra.
    #[cfg(test)]
    pub fn seen_on_reference(&self, cam: &Camera, w: V3) -> Option<Pos2> {
        let e = self.eye(cam);
        let dz = w[2] - e[2];
        if dz > -1e-3 {
            return None;
        }
        let t = -e[2] / dz;
        Some(self.to_stage(lerp3(e, w, t), 0.0))
    }

    /// Vue 3D placée à l'œil de la caméra du panneau, qui regarde droit devant avec sa focale :
    /// elle voit le cadre (rotation de la caméra mise à part).
    pub fn look_through(&self, cam: &Camera) -> Cam3d {
        let eye = self.eye(cam);
        let distance = (self.d0 - cam.dolly).max(1.0);
        Cam3d { target: [eye[0], eye[1], eye[2] - distance], azimuth: 0.0, elevation: 0.0, distance, focal: cam.focal() }
    }
}

/// Projection d'une vue 3D dans une zone de l'écran.
#[derive(Clone, Copy, Debug)]
pub struct Projector {
    right: V3,
    up: V3,
    forward: V3,
    eye: V3,
    scale: f32,
    center: Pos2,
}

impl Projector {
    pub fn new(cam: &Cam3d, rect: Rect) -> Projector {
        let (right, up, forward) = cam.basis();
        Projector { right, up, forward, eye: cam.eye(), scale: cam.pixels_per_unit(rect.width()), center: rect.center() }
    }

    /// Point dans le repère de la vue : (droite, haut, profondeur devant l'œil).
    pub fn to_view(&self, p: V3) -> V3 {
        let d = sub(p, self.eye);
        [dot(d, self.right), dot(d, self.up), dot(d, self.forward)]
    }

    /// Point de l'écran d'un point du repère de la vue (devant l'œil).
    pub fn screen_of(&self, c: V3) -> Pos2 {
        let z = c[2].max(1e-6);
        pos2(self.center.x + c[0] / z * self.scale, self.center.y - c[1] / z * self.scale)
    }

    /// Point de l'écran ; rien s'il est trop près de l'œil ou derrière.
    pub fn project(&self, p: V3) -> Option<Pos2> {
        let c = self.to_view(p);
        (c[2] >= NEAR).then(|| self.screen_of(c))
    }

    /// Segment de l'écran, coupé là où il passe derrière l'œil.
    pub fn segment(&self, a: V3, b: V3) -> Option<[Pos2; 2]> {
        let (mut ca, mut cb) = (self.to_view(a), self.to_view(b));
        match (ca[2] >= NEAR, cb[2] >= NEAR) {
            (false, false) => return None,
            (true, false) => cb = lerp3(ca, cb, (NEAR - ca[2]) / (cb[2] - ca[2])),
            (false, true) => ca = lerp3(cb, ca, (NEAR - cb[2]) / (ca[2] - cb[2])),
            (true, true) => {}
        }
        Some([self.screen_of(ca), self.screen_of(cb)])
    }

    /// Rayon de l'œil à travers le point `screen` : origine et direction.
    pub fn ray(&self, screen: Pos2) -> (V3, V3) {
        let (x, y) = ((screen.x - self.center.x) / self.scale, -(screen.y - self.center.y) / self.scale);
        let dir = [0, 1, 2].map(|k| self.right[k] * x + self.up[k] * y + self.forward[k]);
        (self.eye, dir)
    }

    pub fn eye(&self) -> V3 {
        self.eye
    }
}

/// Où le rayon (`origin`, `dir`) coupe le plan z = `z` (parallèle à la feuille), et à quelle
/// distance le long du rayon. Rien s'il lui est parallèle ou si le plan est derrière.
pub fn ray_plane_z(origin: V3, dir: V3, z: f32) -> Option<(V3, f32)> {
    if dir[2].abs() < 1e-9 {
        return None;
    }
    let t = (z - origin[2]) / dir[2];
    (t > 0.0).then(|| ([origin[0] + dir[0] * t, origin[1] + dir[1] * t, z], t))
}

/// Le plan touché le premier par le rayon parmi `planes` (numéro, profondeur) : sur son dessin
/// (`on_drawing` : numéro, point de la feuille), sinon dans sa feuille. Rien s'il n'en touche aucun.
pub fn pick(world: &World, origin: V3, dir: V3, planes: &[(usize, f32)], on_drawing: impl Fn(usize, Pos2) -> bool) -> Option<usize> {
    let stage = Rect::from_min_size(Pos2::ZERO, vec2(world.stage[0], world.stage[1]));
    let mut drawn: Option<(f32, usize)> = None;
    let mut sheet: Option<(f32, usize)> = None;
    for &(i, depth) in planes {
        let Some((hit, t)) = ray_plane_z(origin, dir, world.plane_z(depth)) else { continue };
        let p = world.to_stage(hit, depth);
        let nearer = |best: Option<(f32, usize)>| best.is_none_or(|(bt, _)| t < bt);
        if on_drawing(i, p) && nearer(drawn) {
            drawn = Some((t, i));
        }
        if stage.contains(p) && nearer(sheet) {
            sheet = Some((t, i));
        }
    }
    drawn.or(sheet).map(|(_, i)| i)
}

/// Avancée en z (monde) d'un glisser de `from` à `to` sur l'écran, le long de l'axe z qui passe
/// par `at` : vers le fond, z diminue. Si l'on regarde dans l'axe, monter éloigne.
pub fn z_motion(proj: &Projector, at: V3, from: Pos2, to: Pos2) -> f32 {
    let d = sub(at, proj.eye());
    let dist = dot(d, d).sqrt().max(1.0);
    let step = dist * 0.1;
    let m = to - from;
    if let (Some(a), Some(b)) = (proj.project(at), proj.project([at[0], at[1], at[2] - step])) {
        let v = b - a;
        if v.length() > 2.0 {
            return -m.dot(v) / v.length_sq() * step;
        }
    }
    m.y * dist / proj.scale
}

/// Bornes de points du monde (pour cadrer la vue).
pub fn bounds(points: impl IntoIterator<Item = V3>) -> (V3, V3) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo[0] > hi[0] {
        return ([0.0; 3], [0.0; 3]);
    }
    (lo, hi)
}

/// Pas du quadrillage du sol : une valeur ronde (1, 2 ou 5 × 10ⁿ) proche de `span` / 12.
pub fn grid_step(span: f32) -> f32 {
    let raw = (span / 12.0).max(1.0);
    let p = 10f32.powi(raw.log10().floor() as i32);
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * p).find(|s| *s >= raw).unwrap_or(10.0 * p)
}
