//! La caméra du plan **dans l'espace** : son œil, ses axes, le rayon d'un point de l'image, et la visée (partie 172, le lot 3
//! des préréglages : travelling latéral, orbite, grue, survol, et les trajectoires toutes faites qui gardent leur sujet dans
//! le cadre).
//!
//! Les mêmes conventions que `Camera::homography` et `espace3d::Monde::repere_camera`, comptées en **pixels de la feuille** :
//! x vers la droite, y vers le haut (l'inverse de la feuille), z vers l'arrière de la caméra de référence (le plan de référence
//! est en z = 0, la caméra de référence en z = d0, la distance de référence `camera::ref_distance`). L'œil de la caméra est
//! en (x, −y, d0 − avancée), sans la borne de `Camera::distance` : elle peut passer le plan de référence.
//!
//! Une caméra du plan tourne sur elle-même par son panoramique (autour de la verticale) puis sa bascule (autour de son axe
//! horizontal) ; son roulis (`angle`) tourne le cadre dans l'image. Ses axes dans l'espace sont donc, comme dans
//! `repere_camera` : droite (cp, 0, sp), haut (−sp·st, ct, cp·st), arrière (−sp·ct, −st, cp·ct) ; elle regarde vers −arrière.

use crate::camera::{self, Camera};

pub type V3 = [f32; 3];

fn plus(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn moins(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn fois(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn scalaire(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norme(a: V3) -> f32 {
    scalaire(a, a).sqrt()
}

/// L'œil de la caméra, en pixels de la feuille (`d0` : la distance de référence, `camera::ref_distance`).
pub fn oeil(c: &Camera, d0: f32) -> V3 {
    [c.x, -c.y, d0 - c.dolly]
}

/// La même caméra, l'œil posé en `e` (son orientation, sa focale et son roulis ne changent pas).
pub fn avec_oeil(c: &Camera, e: V3, d0: f32) -> Camera {
    Camera { x: e[0], y: -e[1], dolly: d0 - e[2], ..*c }
}

/// Les axes de la caméra dans l'espace : sa droite, son haut, son arrière (elle regarde vers −arrière).
pub fn axes(c: &Camera) -> (V3, V3, V3) {
    let (st, ct) = c.tilt.to_radians().sin_cos();
    let (sp, cp) = c.pan.to_radians().sin_cos();
    ([cp, 0.0, sp], [-sp * st, ct, cp * st], [-sp * ct, -st, cp * ct])
}

/// La droite et le bas **de l'image** dans l'espace, roulis compris : un pas d'un pixel de l'image vers la droite (vers le bas)
/// déplace le point visé au plan de la caméra dans cette direction.
pub fn axes_de_l_image(c: &Camera) -> (V3, V3) {
    let (droite, haut, _) = axes(c);
    let (sr, cr) = c.angle.to_radians().sin_cos();
    (moins(fois(droite, cr), fois(haut, sr)), fois(plus(fois(droite, sr), fois(haut, cr)), -1.0))
}

/// La direction (non normée) du rayon de la caméra qui passe par le point (`u`, `v`) de l'image finale `frame` (pixels).
pub fn rayon(c: &Camera, frame: [usize; 2], u: f32, v: f32) -> V3 {
    let f = c.zoom.max(0.01) * camera::ref_distance(frame);
    let (du, dv) = (u - frame[0] as f32 / 2.0, v - frame[1] as f32 / 2.0);
    let (sr, cr) = c.angle.to_radians().sin_cos();
    let (a, b) = (cr * du - sr * dv, sr * du + cr * dv);
    let (droite, haut, arriere) = axes(c);
    plus(plus(fois(droite, a), fois(haut, -b)), fois(arriere, -f))
}

/// Où la caméra voit le point `p` de l'espace : (u, v) en pixels de l'image finale ; rien s'il est derrière elle (ou dans son
/// œil).
pub fn image_de(c: &Camera, frame: [usize; 2], p: V3) -> Option<[f32; 2]> {
    let d0 = camera::ref_distance(frame);
    let f = c.zoom.max(0.01) * d0;
    let q = moins(p, oeil(c, d0));
    let (droite, haut, arriere) = axes(c);
    let (x, y, z) = (scalaire(q, droite), scalaire(q, haut), scalaire(q, arriere));
    if z > -1e-3 * d0 {
        return None;
    }
    let (a, b) = (x * f / -z, y * f / z);
    let (sr, cr) = c.angle.to_radians().sin_cos();
    let (du, dv) = (cr * a + sr * b, -sr * a + cr * b);
    Some([du + frame[0] as f32 / 2.0, dv + frame[1] as f32 / 2.0])
}

/// Le point que la caméra voit au point (`u`, `v`) de l'image, **au plan de référence** (z = 0 : là où les dessins sont tels
/// qu'ils sont dessinés) ; si son rayon ne le rencontre pas devant elle, le point à sa distance de référence le long du rayon.
pub fn point_vu(c: &Camera, frame: [usize; 2], u: f32, v: f32) -> V3 {
    let d0 = camera::ref_distance(frame);
    let e = oeil(c, d0);
    let r = rayon(c, frame, u, v);
    if e[2] > 1e-3 * d0 && r[2] < -1e-6 {
        return plus(e, fois(r, -e[2] / r[2]));
    }
    let n = norme(r).max(1e-9);
    plus(e, fois(r, c.distance(frame) / n))
}

/// Un angle (degrés) ramené au plus près de `pres` (à un tour près).
fn au_plus_pres(a: f32, pres: f32) -> f32 {
    a - ((a - pres) / 360.0).round() * 360.0
}

/// **La visée** : la même caméra (son œil, sa focale, son roulis), son panoramique et sa bascule changés pour qu'elle voie le
/// point `p` de l'espace au point (`u`, `v`) de l'image. Des deux bascules possibles, la plus proche de la sienne. Rien si
/// c'est impossible (le point est dans son œil, ou plus haut ou plus bas que ce que ce point de l'image peut viser).
pub fn viser(c: &Camera, frame: [usize; 2], p: V3, u: f32, v: f32) -> Option<Camera> {
    let d0 = camera::ref_distance(frame);
    let d = moins(p, oeil(c, d0));
    let nd = norme(d);
    if !(nd > 1e-4 * d0) {
        return None;
    }
    let d = fois(d, 1.0 / nd);
    // Le rayon de (u, v) dans le repère de la caméra (avant bascule et panoramique) : (a, −b, −f), normé.
    let f = c.zoom.max(0.01) * d0;
    let (du, dv) = (u - frame[0] as f32 / 2.0, v - frame[1] as f32 / 2.0);
    let (sr, cr) = c.angle.to_radians().sin_cos();
    let (a, b) = (cr * du - sr * dv, sr * du + cr * dv);
    let w = [a, -b, -f];
    let w = fois(w, 1.0 / norme(w));
    // La bascule : la hauteur du rayon, ct·wy − st·wz = R·cos(t + φ), doit être celle de la direction voulue.
    let r = w[1].hypot(w[2]);
    if !(r > 1e-6) || d[1].abs() > r {
        return None;
    }
    let phi = w[2].atan2(w[1]);
    let base = (d[1] / r).clamp(-1.0, 1.0).acos();
    let t = [base - phi, -base - phi].map(|t| au_plus_pres(t.to_degrees(), c.tilt)).into_iter().min_by(|x, y| (x - c.tilt).abs().total_cmp(&(y - c.tilt).abs()))?;
    let (st, ct) = t.to_radians().sin_cos();
    let (vx, vz) = (w[0], st * w[1] + ct * w[2]);
    if vx.hypot(vz) < 1e-6 || d[0].hypot(d[2]) < 1e-6 {
        // (Le point est juste au-dessus ou en dessous : le panoramique ne compte plus, on garde le sien.)
        return Some(Camera { tilt: t, ..*c });
    }
    let pan = (d[2].atan2(d[0]) - vz.atan2(vx)).to_degrees();
    Some(Camera { tilt: t, pan: au_plus_pres(pan, c.pan), ..*c })
}

/// **L'orbite** : la caméra entière (son œil et son regard) tournée de `degres` autour de la verticale qui passe par `p`.
/// Positif : elle passe à droite du sujet (et regarde vers la gauche) ; `p` reste au même endroit de l'image.
pub fn tourner_autour(c: &Camera, frame: [usize; 2], p: V3, degres: f32) -> Camera {
    let d0 = camera::ref_distance(frame);
    let e = oeil(c, d0);
    let (o_x, o_z) = (e[0] - p[0], e[2] - p[2]);
    // (Le panoramique tourne le regard dans le sens où atan2(z, x) croît : l'œil tourne dans le même sens, de −degres.)
    let (s, k) = (-degres).to_radians().sin_cos();
    let e2 = [p[0] + o_x * k - o_z * s, e[1], p[2] + o_x * s + o_z * k];
    Camera { pan: c.pan - degres, ..avec_oeil(c, e2, d0) }
}

/// L'œil déplacé de `d` (pixels de la feuille, dans l'espace).
pub fn deplacer(c: &Camera, frame: [usize; 2], d: V3) -> Camera {
    let d0 = camera::ref_distance(frame);
    avec_oeil(c, plus(oeil(c, d0), d), d0)
}

/// La direction de la caméra **à plat** : vers où elle regarde, sans sa bascule (un pas en avant au-dessus du sol).
pub fn devant_a_plat(c: &Camera) -> V3 {
    let (sp, cp) = c.pan.to_radians().sin_cos();
    [sp, 0.0, -cp]
}
