//! Maillages de la scène 3D : les formes simples (cube, plan, cylindre, sphère, cône, escalier), le
//! mannequin (capsules articulées) et les modèles importés, dans le repère de l'objet. Chaque
//! maillage a ses normales (ombrage), des normales « de contour » lissées (pour tracer l'épaisseur
//! du trait autour de la silhouette) et ses arêtes vives (pour les dessiner).
//!
//! Dimensions de référence (avant la taille de l'objet) : le cube a 1 m de côté, centré ; le plan est
//! un carré de 1 m couché à y = 0 ; le cylindre, le cône et la sphère mesurent 1 m de diamètre et (sauf
//! la sphère) 1 m de haut, centrés. L'escalier est une boîte de 1 m dont le dessus monte par
//! marches, la plus basse du côté +Z. Le mannequin a ses pieds à y = 0 et regarde vers +Z.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::model3d::Model3d;
use crate::scene3d::{joint, Mannequin, Shape3d, JOINTS};
use crate::scene3dmath::{self as m, M4, V3};

/// Facettes d'un cylindre ou d'un cône, et de la sphère (parallèles, méridiens).
pub const SEGMENTS: usize = 24;
pub const SPHERE_RINGS: usize = 12;

/// Un maillage prêt à dessiner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidMesh {
    pub positions: Vec<V3>,
    pub normals: Vec<V3>,
    /// Normale lissée de chaque sommet (moyenne des normales des sommets confondus) : le trait de
    /// contour s'épaissit dans cette direction, sans se déchirer aux arêtes vives.
    pub outline: Vec<V3>,
    pub indices: Vec<u32>,
    /// Arêtes vives à tracer (segments de deux points).
    pub edges: Vec<[V3; 2]>,
    pub bounds: (V3, V3),
}

impl SolidMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    fn push_vertex(&mut self, p: V3, n: V3) -> u32 {
        self.positions.push(p);
        self.normals.push(n);
        (self.positions.len() - 1) as u32
    }

    /// Un triangle, tourné pour que sa face avant (sens trigonométrique vu de la normale) regarde du
    /// côté de `want`.
    fn triangle(&mut self, a: u32, b: u32, c: u32, want: V3) {
        let (pa, pb, pc) = (self.positions[a as usize], self.positions[b as usize], self.positions[c as usize]);
        let g = m::cross(m::sub(pb, pa), m::sub(pc, pa));
        if m::dot(g, want) >= 0.0 {
            self.indices.extend([a, b, c]);
        } else {
            self.indices.extend([a, c, b]);
        }
    }

    /// Un quadrilatère à plat (4 sommets neufs de normale `n`).
    fn quad(&mut self, p: [V3; 4], n: V3) {
        let i = p.map(|q| self.push_vertex(q, n));
        self.triangle(i[0], i[1], i[2], n);
        self.triangle(i[0], i[2], i[3], n);
    }

    fn edge(&mut self, a: V3, b: V3) {
        self.edges.push([a, b]);
    }

    /// Calcule les normales de contour et la boîte, et retire les arêtes en double.
    fn finish(mut self) -> SolidMesh {
        self.outline = smooth_normals(&self.positions, &self.normals);
        self.bounds = m::bounds_of(self.positions.iter().copied()).unwrap_or(([0.0; 3], [0.0; 3]));
        dedupe_edges(&mut self.edges);
        self
    }
}

fn quantized(p: V3) -> [i64; 3] {
    p.map(|v| (v * 1e4).round() as i64)
}

/// Moyenne des normales des sommets qui se trouvent au même endroit.
pub fn smooth_normals(positions: &[V3], normals: &[V3]) -> Vec<V3> {
    let mut sum: HashMap<[i64; 3], V3> = HashMap::with_capacity(positions.len());
    for (p, n) in positions.iter().zip(normals) {
        let e = sum.entry(quantized(*p)).or_insert([0.0; 3]);
        *e = m::add(*e, *n);
    }
    positions.iter().zip(normals).map(|(p, n)| sum.get(&quantized(*p)).map_or(*n, |s| m::normalize(*s))).collect()
}

fn dedupe_edges(edges: &mut Vec<[V3; 2]>) {
    let mut seen = std::collections::HashSet::new();
    edges.retain(|e| {
        let (a, b) = (quantized(e[0]), quantized(e[1]));
        seen.insert(if a <= b { (a, b) } else { (b, a) })
    });
}

// ---------------------------------------------------------------------------------- formes

pub fn cube() -> SolidMesh {
    let mut s = SolidMesh::default();
    let e = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for a in 0..3 {
        for sign in [-1.0f32, 1.0] {
            let n = m::scale(e[a], sign);
            let (u, v) = (e[(a + 1) % 3], e[(a + 2) % 3]);
            let c = m::scale(n, 0.5);
            let at = |x: f32, y: f32| m::add(c, m::add(m::scale(u, x * 0.5), m::scale(v, y * 0.5)));
            s.quad([at(-1.0, -1.0), at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0)], n);
        }
    }
    let corner = |i: usize| [if i & 1 == 0 { -0.5 } else { 0.5 }, if i & 2 == 0 { -0.5 } else { 0.5 }, if i & 4 == 0 { -0.5 } else { 0.5 }];
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0 {
                s.edge(corner(i), corner(i | bit));
            }
        }
    }
    s.finish()
}

pub fn plane() -> SolidMesh {
    let mut s = SolidMesh::default();
    let p = [[-0.5, 0.0, -0.5], [-0.5, 0.0, 0.5], [0.5, 0.0, 0.5], [0.5, 0.0, -0.5]];
    s.quad(p, [0.0, 1.0, 0.0]);
    for k in 0..4 {
        s.edge(p[k], p[(k + 1) % 4]);
    }
    s.finish()
}

fn ring_point(i: usize, segs: usize, r: f32, y: f32) -> V3 {
    let a = std::f32::consts::TAU * (i % segs) as f32 / segs as f32;
    [r * a.cos(), y, r * a.sin()]
}

pub fn cylinder() -> SolidMesh {
    let n = SEGMENTS;
    let mut s = SolidMesh::default();
    // Flanc : 2 sommets par facette (bas, haut), normale radiale.
    let base = s.positions.len() as u32;
    for i in 0..n {
        let r = ring_point(i, n, 1.0, 0.0);
        s.push_vertex(ring_point(i, n, 0.5, -0.5), [r[0], 0.0, r[2]]);
        s.push_vertex(ring_point(i, n, 0.5, 0.5), [r[0], 0.0, r[2]]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (b0, t0, b1, t1) = (base + 2 * i as u32, base + 2 * i as u32 + 1, base + 2 * j as u32, base + 2 * j as u32 + 1);
        let mid = m::normalize(m::add(s.normals[b0 as usize], s.normals[b1 as usize]));
        s.triangle(b0, b1, t1, mid);
        s.triangle(b0, t1, t0, mid);
    }
    // Deux disques.
    for (y, ny) in [(0.5, 1.0), (-0.5, -1.0)] {
        let c = s.push_vertex([0.0, y, 0.0], [0.0, ny, 0.0]);
        let ring: Vec<u32> = (0..n).map(|i| s.push_vertex(ring_point(i, n, 0.5, y), [0.0, ny, 0.0])).collect();
        for i in 0..n {
            s.triangle(c, ring[i], ring[(i + 1) % n], [0.0, ny, 0.0]);
        }
        for i in 0..n {
            s.edge(ring_point(i, n, 0.5, y), ring_point(i + 1, n, 0.5, y));
        }
    }
    s.finish()
}

pub fn cone() -> SolidMesh {
    let n = SEGMENTS;
    let mut s = SolidMesh::default();
    let slope = |a: f32| m::normalize([a.cos(), 0.5, a.sin()]);
    let angle = |i: f32| std::f32::consts::TAU * i / n as f32;
    let base = s.positions.len() as u32;
    for i in 0..n {
        s.push_vertex(ring_point(i, n, 0.5, -0.5), slope(angle(i as f32)));
        // Le sommet de chaque facette porte la normale du milieu de la facette.
        s.push_vertex([0.0, 0.5, 0.0], slope(angle(i as f32 + 0.5)));
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (b0, a0, b1) = (base + 2 * i as u32, base + 2 * i as u32 + 1, base + 2 * j as u32);
        let mid = m::normalize(m::add(s.normals[b0 as usize], s.normals[b1 as usize]));
        s.triangle(b0, b1, a0, mid);
    }
    let c = s.push_vertex([0.0, -0.5, 0.0], [0.0, -1.0, 0.0]);
    let ring: Vec<u32> = (0..n).map(|i| s.push_vertex(ring_point(i, n, 0.5, -0.5), [0.0, -1.0, 0.0])).collect();
    for i in 0..n {
        s.triangle(c, ring[i], ring[(i + 1) % n], [0.0, -1.0, 0.0]);
        s.edge(ring_point(i, n, 0.5, -0.5), ring_point(i + 1, n, 0.5, -0.5));
    }
    s.finish()
}

pub fn sphere() -> SolidMesh {
    let (rings, segs) = (SPHERE_RINGS, SEGMENTS);
    let mut s = SolidMesh::default();
    for j in 0..=rings {
        let theta = std::f32::consts::PI * j as f32 / rings as f32;
        for i in 0..=segs {
            let phi = std::f32::consts::TAU * i as f32 / segs as f32;
            let n = [theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin()];
            s.push_vertex(m::scale(n, 0.5), n);
        }
    }
    let at = |j: usize, i: usize| (j * (segs + 1) + i) as u32;
    for j in 0..rings {
        for i in 0..segs {
            let (a, b, c, d) = (at(j, i), at(j, i + 1), at(j + 1, i + 1), at(j + 1, i));
            let want = m::normalize(m::add(m::add(s.normals[a as usize], s.normals[c as usize]), m::add(s.normals[b as usize], s.normals[d as usize])));
            if j > 0 {
                s.triangle(a, d, b, want);
            }
            if j + 1 < rings {
                s.triangle(b, d, c, want);
            }
        }
    }
    s.finish()
}

/// Escalier de `steps` marches : la plus basse du côté +Z, la plus haute au fond (−Z).
pub fn stairs(steps: u32) -> SolidMesh {
    let n = steps.clamp(2, 64) as usize;
    let d = 1.0 / n as f32;
    let mut s = SolidMesh::default();
    let top = |i: usize| -0.5 + (i + 1) as f32 * d;
    let zf = |i: usize| 0.5 - i as f32 * d;
    for i in 0..n {
        let (z0, z1, y0, y1) = (zf(i), zf(i) - d, -0.5 + i as f32 * d, top(i));
        // Dessus de la marche et contremarche (face tournée vers +Z).
        s.quad([[-0.5, y1, z0], [0.5, y1, z0], [0.5, y1, z1], [-0.5, y1, z1]], [0.0, 1.0, 0.0]);
        s.quad([[-0.5, y0, z0], [0.5, y0, z0], [0.5, y1, z0], [-0.5, y1, z0]], [0.0, 0.0, 1.0]);
        // Flancs.
        s.quad([[0.5, -0.5, z0], [0.5, -0.5, z1], [0.5, y1, z1], [0.5, y1, z0]], [1.0, 0.0, 0.0]);
        s.quad([[-0.5, -0.5, z0], [-0.5, -0.5, z1], [-0.5, y1, z1], [-0.5, y1, z0]], [-1.0, 0.0, 0.0]);
    }
    s.quad([[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5]], [0.0, 0.0, -1.0]);
    s.quad([[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
    // Arêtes : le profil sur chaque flanc, et les arêtes des marches en travers.
    for x in [-0.5, 0.5] {
        let mut prev = [x, -0.5, 0.5];
        for i in 0..n {
            let (z0, z1, y1) = (zf(i), zf(i) - d, top(i));
            let up = [x, y1, z0];
            s.edge(prev, up);
            s.edge(up, [x, y1, z1]);
            prev = [x, y1, z1];
        }
        s.edge(prev, [x, -0.5, -0.5]);
        s.edge([x, -0.5, -0.5], [x, -0.5, 0.5]);
    }
    for i in 0..n {
        let (z0, z1, y0, y1) = (zf(i), zf(i) - d, -0.5 + i as f32 * d, top(i));
        s.edge([-0.5, y1, z0], [0.5, y1, z0]);
        s.edge([-0.5, y1, z1], [0.5, y1, z1]);
        s.edge([-0.5, y0, z0], [0.5, y0, z0]);
    }
    s.finish()
}

// -------------------------------------------------------------------------------- mannequin

/// Un os du mannequin : une capsule de `a` à `b` et de rayon `r`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub a: V3,
    pub b: V3,
    pub r: f32,
}

// Proportions, en fraction de la taille (environ 7 têtes et demie).
const THIGH: f32 = 0.245;
const SHIN: f32 = 0.245;
const ANKLE_Y: f32 = 0.04;
const HIP_Y: f32 = ANKLE_Y + SHIN + THIGH;
const HIP_X: f32 = 0.055;
const TORSO: f32 = 0.31;
const SHOULDER_X: f32 = 0.112;
const SHOULDER_DROP: f32 = 0.025;
const NECK: f32 = 0.04;
const HEAD_R: f32 = 0.062;
const UPPER_ARM: f32 = 0.17;
const FOREARM: f32 = 0.15;
const HAND: f32 = 0.075;
const FOOT: V3 = [0.0, -0.025, 0.11];

/// Rotation d'une articulation. `down` : l'os pend vers le bas (bras, jambe) ; sinon il monte
/// (colonne, tête). `side` : +1 pour le côté gauche du personnage (vers +X), −1 pour le droit.
/// Angles [avant+, torsion, écarter+] : voir `Pose::joints`.
fn joint_matrix(j: [f32; 3], down: bool, side: f32) -> M4 {
    let (x, z, twist) = if down { (-j[0], j[2] * side, j[1] * side) } else { (j[0], -j[2], j[1]) };
    // Torsion d'abord (autour de l'os), puis l'écart, puis le balancement avant-arrière.
    m::mul(&m::rot_x(x), &m::mul(&m::rot_z(z), &m::rot_y(twist)))
}

/// Les capsules du mannequin dans son repère (pieds à y = 0, regard vers +Z), à sa taille.
pub fn mannequin_capsules(mq: &Mannequin) -> Vec<Capsule> {
    let raw = raw_capsules(&mq.joints);
    // La taille debout fixe l'échelle : la pose « Debout » mesure exactement `height`.
    let (lo, hi) = capsule_extent(&raw_capsules(&crate::scene3d::Pose::Standing.joints()));
    let k = mq.height / (hi - lo).max(1e-6);
    let (low, _) = capsule_extent(&raw);
    let shift = [0.0, -low * k, 0.0];
    raw.into_iter().map(|c| Capsule { a: m::add(m::scale(c.a, k), shift), b: m::add(m::scale(c.b, k), shift), r: c.r * k }).collect()
}

fn capsule_extent(caps: &[Capsule]) -> (f32, f32) {
    caps.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.a[1].min(c.b[1]) - c.r), hi.max(c.a[1].max(c.b[1]) + c.r)))
}

/// Cinématique directe, pour un mannequin de hauteur 1 dont le bassin est à l'origine du sol.
fn raw_capsules(j: &[[f32; 3]; JOINTS]) -> Vec<Capsule> {
    let at = |f: &M4, p: V3| m::point(f, p);
    let mut caps = Vec::with_capacity(16);
    let cap = |caps: &mut Vec<Capsule>, a: V3, b: V3, r: f32| caps.push(Capsule { a, b, r });
    let pelvis = m::translation([0.0, HIP_Y, 0.0]);
    cap(&mut caps, at(&pelvis, [-HIP_X, 0.0, 0.0]), at(&pelvis, [HIP_X, 0.0, 0.0]), 0.055);
    let spine = m::mul(&pelvis, &joint_matrix(j[joint::SPINE], false, 1.0));
    cap(&mut caps, at(&pelvis, [0.0; 3]), at(&spine, [0.0, TORSO, 0.0]), 0.072);
    let shoulder_y = TORSO - SHOULDER_DROP;
    cap(&mut caps, at(&spine, [-SHOULDER_X, shoulder_y, 0.0]), at(&spine, [SHOULDER_X, shoulder_y, 0.0]), 0.036);
    // Cou et tête.
    let neck_base = m::mul(&spine, &m::translation([0.0, TORSO, 0.0]));
    let head = m::mul(&neck_base, &joint_matrix(j[joint::HEAD], false, 1.0));
    cap(&mut caps, at(&neck_base, [0.0; 3]), at(&head, [0.0, NECK, 0.0]), 0.026);
    let center = at(&head, [0.0, NECK + HEAD_R * 0.95, 0.0]);
    cap(&mut caps, center, center, HEAD_R);
    // Bras.
    for (side, sh, el) in [(1.0, joint::SHOULDER_L, joint::ELBOW_L), (-1.0, joint::SHOULDER_R, joint::ELBOW_R)] {
        let shoulder = m::mul(&spine, &m::mul(&m::translation([side * SHOULDER_X, shoulder_y, 0.0]), &joint_matrix(j[sh], true, side)));
        let elbow_at = at(&shoulder, [0.0, -UPPER_ARM, 0.0]);
        cap(&mut caps, at(&shoulder, [0.0; 3]), elbow_at, 0.032);
        let elbow = m::mul(&shoulder, &m::mul(&m::translation([0.0, -UPPER_ARM, 0.0]), &joint_matrix(j[el], true, side)));
        let wrist = at(&elbow, [0.0, -FOREARM, 0.0]);
        cap(&mut caps, elbow_at, wrist, 0.026);
        cap(&mut caps, wrist, at(&elbow, [0.0, -(FOREARM + HAND), 0.0]), 0.022);
    }
    // Jambes.
    for (side, hp, kn) in [(1.0, joint::HIP_L, joint::KNEE_L), (-1.0, joint::HIP_R, joint::KNEE_R)] {
        let hip = m::mul(&pelvis, &m::mul(&m::translation([side * HIP_X, 0.0, 0.0]), &joint_matrix(j[hp], true, side)));
        let knee_at = at(&hip, [0.0, -THIGH, 0.0]);
        cap(&mut caps, at(&hip, [0.0; 3]), knee_at, 0.052);
        let knee = m::mul(&hip, &m::mul(&m::translation([0.0, -THIGH, 0.0]), &joint_matrix(j[kn], true, side)));
        let ankle = at(&knee, [0.0, -SHIN, 0.0]);
        cap(&mut caps, knee_at, ankle, 0.04);
        cap(&mut caps, ankle, m::add(ankle, m::dir(&knee, FOOT)), 0.032);
    }
    caps
}

/// Ajoute une capsule au maillage : un tube et deux demi-sphères, `SEGMENTS / 2` facettes autour.
fn capsule_into(s: &mut SolidMesh, c: &Capsule) {
    let segs = 10usize;
    let rings = 4usize;
    let axis = m::sub(c.b, c.a);
    let u = if m::length(axis) < 1e-6 { [0.0, 1.0, 0.0] } else { m::normalize(axis) };
    let helper = if u[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let v = m::normalize(m::cross(u, helper));
    let w = m::cross(u, v);
    // Profil : du pôle de a à l'équateur de a, puis de l'équateur de b au pôle de b.
    let mut profile: Vec<(f32, f32, V3)> = Vec::new(); // (rayon, décalage sur l'axe, d'où il part)
    for k in 0..=rings {
        let phi = -std::f32::consts::FRAC_PI_2 * (1.0 - k as f32 / rings as f32);
        profile.push((phi.cos(), phi.sin(), c.a));
    }
    for k in 0..=rings {
        let phi = std::f32::consts::FRAC_PI_2 * k as f32 / rings as f32;
        profile.push((phi.cos(), phi.sin(), c.b));
    }
    let base = s.positions.len() as u32;
    for &(rho, off, origin) in &profile {
        for i in 0..=segs {
            let a = std::f32::consts::TAU * i as f32 / segs as f32;
            let radial = m::add(m::scale(v, a.cos()), m::scale(w, a.sin()));
            let n = m::add(m::scale(radial, rho), m::scale(u, off));
            s.push_vertex(m::add(origin, m::scale(n, c.r)), n);
        }
    }
    let at = |row: usize, i: usize| base + (row * (segs + 1) + i) as u32;
    for row in 0..profile.len() - 1 {
        for i in 0..segs {
            let (a, b, cc, d) = (at(row, i), at(row, i + 1), at(row + 1, i + 1), at(row + 1, i));
            let want = m::normalize(m::add(m::add(s.normals[a as usize], s.normals[cc as usize]), m::add(s.normals[b as usize], s.normals[d as usize])));
            // Aux pôles, un côté du quadrilatère est de longueur nulle.
            let gap = |p: u32, q: u32| m::length(m::sub(s.positions[p as usize], s.positions[q as usize]));
            let (flat_top, flat_bottom) = (gap(a, b) < 1e-7, gap(cc, d) < 1e-7);
            if !flat_top {
                s.triangle(a, b, d, want);
            }
            if !flat_bottom {
                s.triangle(b, cc, d, want);
            }
        }
    }
}

pub fn mannequin(mq: &Mannequin) -> SolidMesh {
    let mut s = SolidMesh::default();
    for c in mannequin_capsules(mq) {
        capsule_into(&mut s, &c);
    }
    s.finish()
}

// ------------------------------------------------------------------------ modèle importé

/// Au-delà de ce nombre de triangles, l'affichage d'édition se contente de la boîte du modèle.
pub const MODEL_VIEW_LIMIT: usize = 400_000;
/// Arêtes d'un modèle importé gardées pour le tracé.
const MODEL_EDGES: usize = 60_000;

pub fn model(m3: &Model3d) -> SolidMesh {
    let mesh = &m3.mesh;
    let mut s = SolidMesh::default();
    // Normales lissées par sommet (moyenne des faces voisines), normales plates pour l'ombrage.
    let mut smooth = vec![[0.0f32; 3]; mesh.positions.len()];
    let tri_normal = |t: &[u32; 3]| {
        let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
        m::normalize(m::cross(m::sub(b, a), m::sub(c, a)))
    };
    for t in &mesh.triangles {
        let n = tri_normal(t);
        for &i in t {
            smooth[i as usize] = m::add(smooth[i as usize], n);
        }
    }
    for t in &mesh.triangles {
        let n = tri_normal(t);
        let ids = t.map(|i| {
            let id = s.push_vertex(mesh.positions[i as usize], n);
            s.outline.push(m::normalize(smooth[i as usize]));
            id
        });
        s.indices.extend(ids);
    }
    for e in m3.edges.iter().take(MODEL_EDGES) {
        s.edge(mesh.positions[e[0] as usize], mesh.positions[e[1] as usize]);
    }
    s.bounds = m3.bounds;
    s
}

/// Une boîte à la place d'un trop gros modèle (affichage d'édition seulement).
pub fn bounding_box(bounds: (V3, V3)) -> SolidMesh {
    let mut s = cube();
    let (lo, hi) = bounds;
    let center = m::scale(m::add(lo, hi), 0.5);
    let size = m::sub(hi, lo);
    let f = |p: &mut V3| *p = m::add(center, m::mul_each(*p, size));
    s.positions.iter_mut().for_each(f);
    for e in &mut s.edges {
        f(&mut e[0]);
        f(&mut e[1]);
    }
    s.bounds = bounds;
    s
}

// ---------------------------------------------------------------------------- bibliothèque

/// Ce qui identifie un maillage : la forme et ses paramètres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeshKey {
    Cube,
    Plane,
    Cylinder,
    Sphere,
    Cone,
    Stairs(u32),
    /// Empreinte de la taille et des articulations.
    Mannequin(u64),
    Model(u64),
}

pub fn key_of(shape: &Shape3d) -> MeshKey {
    match shape {
        Shape3d::Cube => MeshKey::Cube,
        Shape3d::Plane => MeshKey::Plane,
        Shape3d::Cylinder => MeshKey::Cylinder,
        Shape3d::Sphere => MeshKey::Sphere,
        Shape3d::Cone => MeshKey::Cone,
        Shape3d::Stairs { steps } => MeshKey::Stairs(*steps),
        Shape3d::Mannequin(mq) => {
            // (Le hachage par défaut a des clés fixes : la même empreinte à chaque lancement.)
            let mut h = std::collections::hash_map::DefaultHasher::new();
            mq.height.to_bits().hash(&mut h);
            for j in &mq.joints {
                for v in j {
                    v.to_bits().hash(&mut h);
                }
            }
            MeshKey::Mannequin(h.finish())
        }
        Shape3d::Model { id } => MeshKey::Model(*id),
    }
}

/// Construit le maillage d'une forme. Un modèle importé introuvable donne un cube.
pub fn build(shape: &Shape3d, models: &HashMap<u64, Arc<Model3d>>) -> SolidMesh {
    match shape {
        Shape3d::Cube => cube(),
        Shape3d::Plane => plane(),
        Shape3d::Cylinder => cylinder(),
        Shape3d::Sphere => sphere(),
        Shape3d::Cone => cone(),
        Shape3d::Stairs { steps } => stairs(*steps),
        Shape3d::Mannequin(mq) => mannequin(mq),
        Shape3d::Model { id } => match models.get(id) {
            Some(md) => model(md),
            None => cube(),
        },
    }
}

/// Les maillages déjà construits, pour ne pas les refaire à chaque image.
#[derive(Default)]
pub struct MeshLib {
    map: HashMap<MeshKey, Arc<SolidMesh>>,
}

impl MeshLib {
    pub fn get(&mut self, shape: &Shape3d, models: &HashMap<u64, Arc<Model3d>>) -> (MeshKey, Arc<SolidMesh>) {
        let key = key_of(shape);
        if let Some(mesh) = self.map.get(&key) {
            return (key, mesh.clone());
        }
        // Un mannequin dont on tire une articulation change de maillage à chaque image : on ne
        // garde pas des centaines de poses.
        if self.map.len() > 128 {
            self.map.retain(|k, _| !matches!(k, MeshKey::Mannequin(_)));
        }
        let mesh = Arc::new(build(shape, models));
        self.map.insert(key, mesh.clone());
        (key, mesh)
    }

    /// Comme `get`, pour l'affichage d'édition : un trop gros modèle importé est remplacé par sa boîte.
    pub fn get_for_view(&mut self, shape: &Shape3d, models: &HashMap<u64, Arc<Model3d>>) -> (MeshKey, Arc<SolidMesh>) {
        let (key, mesh) = self.get(shape, models);
        if let MeshKey::Model(id) = key {
            if mesh.triangle_count() > MODEL_VIEW_LIMIT {
                let proxy = MeshKey::Model(id | 1 << 63);
                let boxed = self.map.entry(proxy).or_insert_with(|| Arc::new(bounding_box(mesh.bounds))).clone();
                return (proxy, boxed);
            }
        }
        (key, mesh)
    }
}

/// Boîte (min, max) d'un objet dans le monde.
pub fn world_bounds(mesh: &SolidMesh, matrix: &M4) -> (V3, V3) {
    let (lo, hi) = mesh.bounds;
    m::bounds_of(m::corners_of(lo, hi).iter().map(|c| m::point(matrix, *c))).unwrap_or(([0.0; 3], [0.0; 3]))
}
