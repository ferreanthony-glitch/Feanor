//! Mathématiques de l'Espace 3D (`docs/ESPACE3D.md`) : vecteurs, matrices 4×4 en colonnes, rayons, prise de vue
//! (`View`), caméra d'orbite de la vue d'édition (`Orbite`) et les calculs des manipulateurs (déplacer, tourner,
//! mettre à l'échelle) et de l'aimantation. Rien ici ne dépend de l'interface : tout se teste sans fenêtre.
//!
//! Conventions : mètres, Y vers le haut, Z vers la caméra. Une rotation s'écrit en degrés [x, y, z] et s'applique
//! dans l'ordre Y puis X puis Z (R = Ry · Rx · Rz) : lacet, tangage, roulis.

pub type V3 = [f32; 3];
/// Matrice 4×4 en colonnes (l'élément ligne r, colonne c est `m[c * 4 + r]`).
pub type M4 = [f32; 16];

pub const IDENT: M4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}
pub fn normalize(a: V3) -> V3 {
    let l = length(a);
    if l < 1e-20 {
        [0.0, 1.0, 0.0]
    } else {
        scale(a, 1.0 / l)
    }
}

// ---------------------------------------------------------------------------------- matrices

/// `a · b` : on applique d'abord `b`, puis `a`.
pub fn mul(a: &M4, b: &M4) -> M4 {
    let mut m = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            m[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    m
}

pub fn translation(t: V3) -> M4 {
    let mut m = IDENT;
    m[12..15].copy_from_slice(&t);
    m
}

pub fn scaling(s: V3) -> M4 {
    let mut m = IDENT;
    m[0] = s[0];
    m[5] = s[1];
    m[10] = s[2];
    m
}

pub fn rot_x(deg: f32) -> M4 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut m = IDENT;
    m[5] = c;
    m[6] = s;
    m[9] = -s;
    m[10] = c;
    m
}

pub fn rot_y(deg: f32) -> M4 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut m = IDENT;
    m[0] = c;
    m[2] = -s;
    m[8] = s;
    m[10] = c;
    m
}

pub fn rot_z(deg: f32) -> M4 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut m = IDENT;
    m[0] = c;
    m[1] = s;
    m[4] = -s;
    m[5] = c;
    m
}

/// Rotation de `deg` degrés autour de l'axe `axis` (Rodrigues).
pub fn rot_axis(axis: V3, deg: f32) -> M4 {
    let a = normalize(axis);
    let (s, c) = deg.to_radians().sin_cos();
    let t = 1.0 - c;
    let mut m = IDENT;
    m[0] = t * a[0] * a[0] + c;
    m[1] = t * a[0] * a[1] + s * a[2];
    m[2] = t * a[0] * a[2] - s * a[1];
    m[4] = t * a[0] * a[1] - s * a[2];
    m[5] = t * a[1] * a[1] + c;
    m[6] = t * a[1] * a[2] + s * a[0];
    m[8] = t * a[0] * a[2] + s * a[1];
    m[9] = t * a[1] * a[2] - s * a[0];
    m[10] = t * a[2] * a[2] + c;
    m
}

/// Rotation [x, y, z] en degrés : Ry · Rx · Rz.
pub fn rotation(rot: V3) -> M4 {
    mul(&rot_y(rot[1]), &mul(&rot_x(rot[0]), &rot_z(rot[2])))
}

/// Position, rotation (degrés) et échelle : T · R · S.
pub fn trs(pos: V3, rot: V3, size: V3) -> M4 {
    mul(&translation(pos), &mul(&rotation(rot), &scaling(size)))
}

pub fn point(m: &M4, p: V3) -> V3 {
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

pub fn dir(m: &M4, d: V3) -> V3 {
    [0, 1, 2].map(|r| m[r] * d[0] + m[4 + r] * d[1] + m[8 + r] * d[2])
}

/// Inverse d'une matrice affine (rotation, échelle, translation). Une échelle nulle donne l'identité.
pub fn inverse_affine(m: &M4) -> M4 {
    let (a, b, c) = ([m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]);
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-20 {
        return IDENT;
    }
    // Les lignes de l'inverse 3×3 sont b×c, c×a, a×b divisées par le déterminant.
    let r0 = scale(cross(b, c), 1.0 / det);
    let r1 = scale(cross(c, a), 1.0 / det);
    let r2 = scale(cross(a, b), 1.0 / det);
    let t = [m[12], m[13], m[14]];
    [
        r0[0], r1[0], r2[0], 0.0,
        r0[1], r1[1], r2[1], 0.0,
        r0[2], r1[2], r2[2], 0.0,
        -dot(r0, t), -dot(r1, t), -dot(r2, t), 1.0,
    ]
}

/// Rotation [x, y, z] en degrés (ordre de `rotation`) d'une matrice, sans tenir compte de l'échelle.
pub fn euler_of(m: &M4) -> V3 {
    let col = |c: usize| normalize([m[c * 4], m[c * 4 + 1], m[c * 4 + 2]]);
    let (cx, cy, cz) = (col(0), col(1), col(2));
    // r[ligne][colonne]
    let r = |row: usize, c: usize| [cx, cy, cz][c][row];
    let s = (-r(1, 2)).clamp(-1.0, 1.0);
    let rx = s.asin();
    if rx.cos().abs() > 1e-4 {
        [rx.to_degrees(), r(0, 2).atan2(r(2, 2)).to_degrees(), r(1, 0).atan2(r(1, 1)).to_degrees()]
    } else {
        [rx.to_degrees(), (-r(2, 0)).atan2(r(0, 0)).to_degrees(), 0.0]
    }
}

/// Les trois axes (colonnes normalisées) d'une matrice : où vont X, Y et Z.
pub fn axes_of(m: &M4) -> [V3; 3] {
    [0, 1, 2].map(|c| normalize([m[c * 4], m[c * 4 + 1], m[c * 4 + 2]]))
}

// ------------------------------------------------------------------------------------ rayons

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub o: V3,
    /// Direction, de longueur 1.
    pub d: V3,
}

impl Ray {
    pub fn new(o: V3, d: V3) -> Ray {
        Ray { o, d: normalize(d) }
    }

    pub fn at(&self, t: f32) -> V3 {
        add(self.o, scale(self.d, t))
    }

    /// Le même rayon dans le repère d'un objet (`inv` : l'inverse de sa matrice). La direction n'est
    /// pas renormalisée : les distances `t` restent celles du monde.
    pub fn to_local(&self, inv: &M4) -> (V3, V3) {
        (point(inv, self.o), dir(inv, self.d))
    }
}

/// Où le rayon coupe le plan passant par `p0` de normale `n` ; rien s'il lui est parallèle ou si le
/// plan est derrière.
pub fn ray_plane(ray: &Ray, p0: V3, n: V3) -> Option<f32> {
    let den = dot(ray.d, n);
    if den.abs() < 1e-7 {
        return None;
    }
    let t = dot(sub(p0, ray.o), n) / den;
    (t > 0.0).then_some(t)
}

/// Paramètre `t` du point de la droite `p0 + t · axis` (axis de longueur 1) le plus proche du rayon.
pub fn closest_on_axis(ray: &Ray, p0: V3, axis: V3) -> Option<f32> {
    let w = sub(p0, ray.o);
    let b = dot(axis, ray.d);
    let (d, e) = (dot(axis, w), dot(ray.d, w));
    let den = 1.0 - b * b;
    (den > 1e-6).then(|| (b * e - d) / den)
}

// ----------------------------------------------------------------------------- manipulateurs

/// Déplacement le long de l'axe `axis` (de longueur 1) qui passe par `origin`, quand le rayon passe
/// du départ `from` à `to`.
pub fn drag_along_axis(from: &Ray, to: &Ray, origin: V3, axis: V3) -> V3 {
    match (closest_on_axis(from, origin, axis), closest_on_axis(to, origin, axis)) {
        (Some(a), Some(b)) => scale(axis, b - a),
        _ => [0.0; 3],
    }
}

/// Déplacement dans le plan de normale `normal` qui passe par `origin`.
pub fn drag_in_plane(from: &Ray, to: &Ray, origin: V3, normal: V3) -> V3 {
    match (ray_plane(from, origin, normal), ray_plane(to, origin, normal)) {
        (Some(a), Some(b)) => sub(to.at(b), from.at(a)),
        _ => [0.0; 3],
    }
}

/// Angle (degrés, sens direct autour de `axis`) dont le rayon a tourné autour de `center`, vu dans
/// le plan perpendiculaire à l'axe.
pub fn drag_angle(from: &Ray, to: &Ray, center: V3, axis: V3) -> f32 {
    let (Some(a), Some(b)) = (ray_plane(from, center, axis), ray_plane(to, center, axis)) else { return 0.0 };
    let (va, vb) = (sub(from.at(a), center), sub(to.at(b), center));
    if length(va) < 1e-6 || length(vb) < 1e-6 {
        return 0.0;
    }
    dot(axis, cross(va, vb)).atan2(dot(va, vb)).to_degrees()
}

/// Facteur d'échelle quand le rayon passe du départ `from` à `to`, le long de l'axe qui passe par
/// `origin` : le rapport des distances à l'origine. Borné pour qu'un objet ne s'inverse pas.
pub fn drag_scale_axis(from: &Ray, to: &Ray, origin: V3, axis: V3) -> f32 {
    match (closest_on_axis(from, origin, axis), closest_on_axis(to, origin, axis)) {
        (Some(a), Some(b)) if a.abs() > 1e-4 => (b / a).clamp(0.01, 100.0),
        _ => 1.0,
    }
}

/// Facteur d'échelle d'un glisser d'écran vers le haut (`dy` < 0) ou le bas, pour l'échelle
/// uniforme.
pub fn scale_from_screen(dy: f32) -> f32 {
    (-dy * 0.01).exp().clamp(0.01, 100.0)
}

/// Valeur ramenée au multiple de `step` le plus proche (`step` ≤ 0 : inchangée).
pub fn snap(v: f32, step: f32) -> f32 {
    if step > 0.0 {
        (v / step).round() * step
    } else {
        v
    }
}

/// Position aimantée à la grille de pas `step`, composante par composante. `axes` dit lesquelles
/// (celles que le geste a bougées : on ne ramène pas à la grille l'axe qu'on n'a pas touché).
pub fn snap_position(p: V3, step: f32, axes: [bool; 3]) -> V3 {
    [0, 1, 2].map(|k| if axes[k] { snap(p[k], step) } else { p[k] })
}

/// Boîte englobante (min, max) de points.
pub fn bounds_of(points: impl IntoIterator<Item = V3>) -> Option<(V3, V3)> {
    let mut it = points.into_iter();
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| ([0, 1, 2].map(|k| lo[k].min(p[k])), [0, 1, 2].map(|k| hi[k].max(p[k])))))
}

// ------------------------------------------------------------------------------- caméra (vue)

/// Une prise de vue : où est l'œil, comment il est tourné, quelle focale, et à quelle largeur
/// d'image (en pixels) correspondent les 36 mm de la pellicule. Une seule définition sert à la
/// vue d'édition (carte graphique), au rendu vers un calque (processeur) et à la caméra du plan :
/// ce qu'on voit en éditant est ce qui sera dessiné.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: V3,
    pub right: V3,
    pub up: V3,
    /// Vers où l'œil regarde.
    pub forward: V3,
    /// Focale en mm (pellicule de 36 mm de large).
    pub focal: f32,
    /// Largeur de l'image (pixels) qui représente la pellicule de 36 mm.
    pub ref_width: f32,
    /// Projection orthographique (les vues Dessus, Face… de l'Espace 3D, `espace3dvues.rs`) : la largeur, en unités du monde,
    /// que montrent les `ref_width` pixels. Les rayons sont alors parallèles (direction `forward`, origine sur le plan de
    /// l'image qui passe par `eye`) et la focale ne sert pas. Rien : la perspective.
    pub ortho: Option<f32>,
}

impl View {
    /// Pixels de l'image par unité de x/z (en perspective) ; en orthographique, pixels par unité du monde.
    pub fn px_per_unit(&self) -> f32 {
        match self.ortho {
            Some(w) => self.ref_width.max(1.0) / w.max(1e-6),
            None => self.ref_width / 2.0 * self.focal.max(1.0) / 18.0,
        }
    }

    /// Point dans le repère de l'œil : (droite, haut, profondeur devant l'œil).
    pub fn to_view(&self, p: V3) -> V3 {
        let d = sub(p, self.eye);
        [dot(d, self.right), dot(d, self.up), dot(d, self.forward)]
    }

    /// Point de l'image (pixels, origine en haut à gauche, `center` : le point de l'image où l'on
    /// regarde) ; rien s'il est derrière l'œil.
    pub fn project(&self, p: V3, center: [f32; 2]) -> Option<[f32; 2]> {
        let c = self.to_view(p);
        let s = self.px_per_unit();
        if self.ortho.is_some() {
            return (c[2] > 1e-6).then(|| [center[0] + c[0] * s, center[1] - c[1] * s]);
        }
        (c[2] > 1e-6).then(|| [center[0] + c[0] / c[2] * s, center[1] - c[1] / c[2] * s])
    }

    /// Rayon de l'œil à travers le pixel `px` (en orthographique : parallèle à `forward`, depuis le plan de l'image).
    pub fn ray(&self, px: [f32; 2], center: [f32; 2]) -> Ray {
        let s = self.px_per_unit();
        let (x, y) = ((px[0] - center[0]) / s, -(px[1] - center[1]) / s);
        if self.ortho.is_some() {
            return Ray::new(add(self.eye, add(scale(self.right, x), scale(self.up, y))), self.forward);
        }
        let d = [0, 1, 2].map(|k| self.right[k] * x + self.up[k] * y + self.forward[k]);
        Ray::new(self.eye, d)
    }

    /// La vue regarde en orthographique.
    pub fn is_ortho(&self) -> bool {
        self.ortho.is_some()
    }

    /// Le point du monde sous le pixel `px`, dans le plan face à la vue qui passe par `at` (glisser « dans le plan de
    /// l'écran », à la profondeur de l'objet).
    pub fn screen_to_plane(&self, px: [f32; 2], center: [f32; 2], at: V3) -> Option<V3> {
        let ray = self.ray(px, center);
        let den = dot(ray.d, self.forward);
        if den.abs() < 1e-7 {
            return None;
        }
        let t = dot(sub(at, ray.o), self.forward) / den;
        t.is_finite().then(|| ray.at(t))
    }
}

// ------------------------------------------------------------------- caméra d'orbite

/// Hauteur de regard maximale de la vue d'édition (au-delà, la caméra se retournerait).
pub const MAX_ELEVATION: f32 = 89.0;

/// La caméra de la vue d'édition de l'Espace 3D : elle regarde `target` depuis la distance `distance`, en faisant le
/// tour (`azimuth`, degrés, 0 = depuis +Z) à une hauteur de regard (`elevation`, degrés, 90 = du dessus). Focale en mm
/// pour une pellicule de 36 mm de large.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbite {
    pub target: V3,
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub focal: f32,
}

impl Orbite {
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

    /// Pixels de la zone par unité de x/z (largeur de la zone ↔ 36 mm de pellicule).
    pub fn pixels_per_unit(&self, width: f32) -> f32 {
        width / 2.0 * self.focal.max(1.0) / 18.0
    }

    /// La prise de vue de cette caméra, pour une zone de `width` pixels.
    pub fn view(&self, width: f32) -> View {
        let (right, up, forward) = self.basis();
        View { eye: self.eye(), right, up, forward, focal: self.focal, ref_width: width, ortho: None }
    }

    /// Cadre toute la boîte `bounds`, vue sous l'angle donné (degrés : `azimuth` autour, `elevation` au-dessus) : la
    /// caméra recule juste assez pour que les 8 coins tiennent dans la zone (à 92 %).
    pub fn fit_at(bounds: (V3, V3), frame: [f32; 2], azimuth: f32, elevation: f32) -> Orbite {
        let (lo, hi) = bounds;
        let center = scale(add(lo, hi), 0.5);
        let radius = (dot(sub(hi, lo), sub(hi, lo)).sqrt() / 2.0).max(1e-3);
        let mut cam = Orbite { target: center, azimuth, elevation, distance: 1.0, focal: 35.0 };
        let (r, u, f) = cam.basis();
        let s = cam.pixels_per_unit(frame[0]);
        let (hw, hh) = ((frame[0] * 0.46).max(1.0), (frame[1] * 0.46).max(1.0));
        // Un coin à (x, y) du centre et `ahead` plus loin que lui, vu à la distance D : sa profondeur est D + ahead, et
        // il tient si |x|·s ≤ hw·(D + ahead).
        let mut distance = radius * 1e-3;
        for i in 0..8 {
            let corner = [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] });
            let v = sub(corner, center);
            let ahead = dot(v, f);
            distance = distance.max(dot(v, r).abs() * s / hw - ahead).max(dot(v, u).abs() * s / hh - ahead).max(radius * 0.01 - ahead);
        }
        cam.distance = distance;
        cam
    }

    /// Tourner autour du point visé.
    pub fn orbit(&mut self, d_azimuth: f32, d_elevation: f32) {
        self.azimuth = (self.azimuth + d_azimuth).rem_euclid(360.0);
        self.elevation = (self.elevation + d_elevation).clamp(-MAX_ELEVATION, MAX_ELEVATION);
    }

    /// Glisser la vue de (`dx`, `dy`) pixels : le point visé suit la souris.
    pub fn pan(&mut self, dx: f32, dy: f32, width: f32) {
        let (r, u, _) = self.basis();
        let k = self.distance / self.pixels_per_unit(width);
        self.target = add(self.target, add(scale(r, -dx * k), scale(u, dy * k)));
    }

    /// Avancer (`factor` < 1) ou reculer (> 1).
    pub fn dolly(&mut self, factor: f32) {
        self.distance = (self.distance * factor).max(1e-4);
    }
}