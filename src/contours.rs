//! Contours des éléments choisis (le trait médian de chacun, 1 px, en bleu) sans ralentir quand on
//! en choisit des centaines (Anthony, 2026-10-18 : « un ralentissement quand beaucoup d'objets sont
//! sélectionnés », au repos et pendant un geste).
//!
//! Avant, chaque image d'écran recopiait tous les points de tous les éléments choisis, les passait à
//! l'écran un à un, et laissait egui tracer chaque ligne (4 sommets et 18 indices par point,
//! renvoyés à la carte graphique à chaque image) : 14 ms de tracé et 22 Mo de maillage par image
//! pour 300 traits de 500 points, 54 ms pour 300 traits de 2000 points.
//!
//! Maintenant les contours sont faits une fois, pour la sélection et la vue du moment :
//! - ramenés à la résolution de l'écran (Douglas-Peucker à 0,45 px près ; plus gros si la sélection
//!   dépasse `MAX_POINTS` points, jusqu'à 12 px : on voit toujours où sont les traits) ;
//! - tracés en ruban de trois sommets par point (milieu opaque, bords transparents : une ligne
//!   d'1 px adoucie, deux fois moins de sommets que le tracé d'egui) dans un seul maillage gardé ;
//! - redessinés tels quels tant que la sélection et la vue sont les mêmes, et déplacés d'une
//!   transformation (une addition par sommet) quand la vue bouge ou que la sélection est déplacée,
//!   tournée ou agrandie : jamais refaits pendant un geste.
//!
//! Refaits seulement si la sélection ou le dessin change, ou si la vue a changé d'échelle de plus
//! d'un quart (l'épaisseur du trait suivrait) en dehors d'un geste, ou si la vue est en
//! perspective (le maillage ne suit plus une transformation).

use std::sync::Arc;

use eframe::egui::epaint::{Mesh, Vertex, WHITE_UV};
use eframe::egui::{pos2, Color32, Pos2, Shape};

use crate::vector::{Affine, Element, VectorDrawing};

/// Au plus ce nombre de points (après simplification) pour tous les contours ensemble.
pub const MAX_POINTS: usize = 40_000;
/// Au-delà, on s'arrête là même à la plus grosse tolérance (des milliers de petits éléments).
const HARD_MAX_POINTS: usize = 2 * MAX_POINTS;
/// Écart toléré à la simplification, en points d'écran, puis jusqu'où on peut le grossir.
const TOLERANCE: f32 = 0.45;
const TOLERANCE_MAX: f32 = 12.0;
/// Demi-largeur du ruban, en points d'écran.
const HALF_WIDTH: f32 = 1.0;
/// Échelle de la vue (depuis la construction) en dehors de laquelle le maillage est refait.
const SCALE_RANGE: (f32, f32) = (0.8, 1.25);

/// Quatre points du dessin dont la place à l'écran dit comment la vue (et le geste) ont déplacé le
/// maillage depuis sa construction.
pub const ANCHORS: [Pos2; 4] = [pos2(0.0, 0.0), pos2(1000.0, 0.0), pos2(0.0, 1000.0), pos2(1000.0, 1000.0)];

/// Ce qu'a coûté la construction, pour les essais et le suivi.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub lines: usize,
    pub points_in: usize,
    pub points_out: usize,
    pub tolerance: f32,
}

/// Les contours à l'écran d'une sélection.
pub struct Contours {
    key: u64,
    mesh: Arc<Mesh>,
    /// Où étaient `ANCHORS` à l'écran quand le maillage a été fait.
    anchor: [Pos2; 4],
    pub stats: Stats,
}

impl Contours {
    /// Construit les contours de `lines` (points du dessin) vus par `to_s` ; `anchor` : la place à
    /// l'écran de `ANCHORS` avec la même vue.
    pub fn build(key: u64, lines: &[Vec<Pos2>], to_s: impl Fn(Pos2) -> Pos2, anchor: [Pos2; 4], color: Color32) -> Contours {
        let mut screen: Vec<Vec<Pos2>> = lines.iter().filter(|l| l.len() >= 2).map(|l| l.iter().map(|&p| to_s(p)).collect()).collect();
        let points_in: usize = screen.iter().map(Vec::len).sum();
        let mut tolerance = TOLERANCE;
        loop {
            screen = screen.iter().map(|l| simplify(l, tolerance)).collect();
            let total: usize = screen.iter().map(Vec::len).sum();
            if total <= MAX_POINTS || tolerance >= TOLERANCE_MAX {
                break;
            }
            tolerance = (tolerance * 1.7).min(TOLERANCE_MAX);
        }
        let mut mesh = Mesh::default();
        let mut points_out = 0;
        let mut lines_out = 0;
        for line in &screen {
            if points_out >= HARD_MAX_POINTS {
                break;
            }
            let n = ribbon(&mut mesh, line, color);
            if n > 0 {
                points_out += n;
                lines_out += 1;
            }
        }
        Contours { key, mesh: Arc::new(mesh), anchor, stats: Stats { lines: lines_out, points_in, points_out, tolerance } }
    }

    pub fn key(&self) -> u64 {
        self.key
    }

    /// Les sommets (essais).
    pub fn vertex_count(&self) -> usize {
        self.mesh.vertices.len()
    }

    pub fn index_count(&self) -> usize {
        self.mesh.indices.len()
    }

    /// La transformation qui amène le maillage où `now` (la place à l'écran de `ANCHORS`) le met
    /// maintenant ; `None` si la vue n'est plus une transformation affine (perspective).
    pub fn transform_to(&self, now: &[Pos2; 4]) -> Option<Affine> {
        let t = fit(&self.anchor, now)?;
        // Le quatrième point doit tomber où il tombe : sinon la vue n'est pas affine.
        ((t.apply(self.anchor[3]) - now[3]).length() < 0.5).then_some(t)
    }

    /// Le maillage a-t-il trop changé d'échelle pour qu'on le garde (l'épaisseur du trait suivrait) ?
    pub fn too_scaled(t: &Affine) -> bool {
        let s = t.width_factor();
        !(SCALE_RANGE.0..=SCALE_RANGE.1).contains(&s)
    }

    /// Le maillage déplacé par `t` : le même, sans rien copier, si `t` ne change rien.
    pub fn shape(&self, t: &Affine) -> Shape {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        if near(t.a, 1.0) && near(t.d, 1.0) && near(t.b, 0.0) && near(t.c, 0.0) && near(t.tx, 0.0) && near(t.ty, 0.0) {
            return Shape::Mesh(self.mesh.clone());
        }
        let mut m = Mesh { indices: self.mesh.indices.clone(), vertices: Vec::with_capacity(self.mesh.vertices.len()), texture_id: self.mesh.texture_id };
        m.vertices.extend(self.mesh.vertices.iter().map(|v| Vertex { pos: t.apply(v.pos), ..*v }));
        Shape::Mesh(Arc::new(m))
    }
}

/// La transformation affine qui envoie `from[0..3]` sur `to[0..3]` (`None` : points alignés ou
/// pas finis).
fn fit(from: &[Pos2; 4], to: &[Pos2; 4]) -> Option<Affine> {
    let (u, v) = (from[1] - from[0], from[2] - from[0]);
    let (big_u, big_v) = (to[1] - to[0], to[2] - to[0]);
    let det = u.x * v.y - u.y * v.x;
    if det.abs() < 1e-6 || !det.is_finite() {
        return None;
    }
    // M = [U V] · [u v]⁻¹
    let a = (big_u.x * v.y - big_v.x * u.y) / det;
    let b = (-big_u.x * v.x + big_v.x * u.x) / det;
    let c = (big_u.y * v.y - big_v.y * u.y) / det;
    let d = (-big_u.y * v.x + big_v.y * u.x) / det;
    let t = Affine { a, b, c, d, tx: 0.0, ty: 0.0 };
    let o = t.apply(from[0]);
    let t = Affine { tx: to[0].x - o.x, ty: to[0].y - o.y, ..t };
    [t.a, t.b, t.c, t.d, t.tx, t.ty].iter().all(|v| v.is_finite()).then_some(t)
}

/// Douglas-Peucker : la ligne sans les points qui s'écartent de moins de `tol` du segment qui
/// les entoure. Les deux extrémités restent.
pub fn simplify(pts: &[Pos2], tol: f32) -> Vec<Pos2> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let ab = pb - pa;
        let len = ab.length();
        let (mut far, mut at) = (0.0f32, a);
        for (i, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let ap = *p - pa;
            // (Extrémités confondues : la distance au point ; sinon à la droite.)
            let dist = if len < 1e-6 { ap.length() } else { (ap.x * ab.y - ap.y * ab.x).abs() / len };
            if dist > far {
                (far, at) = (dist, i);
            }
        }
        if far > tol {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    pts.iter().zip(&keep).filter(|(_, &k)| k).map(|(p, _)| *p).collect()
}

/// Ajoute au maillage le ruban de `line` : trois sommets par point (bord, milieu, bord), le milieu
/// à la couleur `color`, les bords transparents. Rend le nombre de points posés (0 : trop court).
fn ribbon(mesh: &mut Mesh, line: &[Pos2], color: Color32) -> usize {
    // Sans deux points confondus à la suite.
    let mut pts: Vec<Pos2> = Vec::with_capacity(line.len());
    for &p in line {
        if p.is_finite() && pts.last().is_none_or(|q| q.distance_sq(p) > 1e-6) {
            pts.push(p);
        }
    }
    let n = pts.len();
    if n < 2 {
        return 0;
    }
    let base = mesh.vertices.len() as u32;
    let normal = |a: Pos2, b: Pos2| {
        let d = (b - a).normalized();
        eframe::egui::vec2(-d.y, d.x)
    };
    for i in 0..n {
        let before = if i > 0 { Some(normal(pts[i - 1], pts[i])) } else { None };
        let after = if i + 1 < n { Some(normal(pts[i], pts[i + 1])) } else { None };
        // La normale d'un coin : la moyenne des deux, allongée pour garder la largeur (au plus 2 fois).
        let offset = match (before, after) {
            (Some(a), Some(b)) => {
                let m = a + b;
                if m.length_sq() < 1e-6 {
                    b
                } else {
                    let m = m.normalized();
                    m / m.dot(a).max(0.5)
                }
            }
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => unreachable!("au moins deux points"),
        } * HALF_WIDTH;
        for (pos, color) in [(pts[i] + offset, Color32::TRANSPARENT), (pts[i], color), (pts[i] - offset, Color32::TRANSPARENT)] {
            mesh.vertices.push(Vertex { pos, uv: WHITE_UV, color });
        }
    }
    for i in 0..(n as u32 - 1) {
        let (l0, c0, r0) = (base + 3 * i, base + 3 * i + 1, base + 3 * i + 2);
        let (l1, c1, r1) = (l0 + 3, c0 + 3, r0 + 3);
        mesh.indices.extend_from_slice(&[l0, c0, c1, l0, c1, l1, c0, r0, r1, c0, r1, c1]);
    }
    n
}

impl crate::app::App {
    /// Les contours des éléments `items` du dessin `cel`, tels qu'ils se voient maintenant
    /// (`moved` : le geste en cours sur la sélection, en coordonnées du dessin ; `gesture` : un
    /// geste est en cours, les contours ne sont alors jamais refaits, seulement déplacés). Gardés
    /// d'une image à l'autre : voir le haut de ce fichier.
    pub(crate) fn selection_contours(&self, cel: u64, items: &[usize], moved: Affine, gesture: bool, color: Color32) -> Option<Shape> {
        let c = self.doc.cels.get(&cel)?;
        let d = c.vector.as_ref()?;
        let key = fingerprint(cel, c.version, d, items);
        let to_s = |p: Pos2| self.layer_to_screen(moved.apply(p));
        let now = ANCHORS.map(to_s);
        let mut slot = self.sel_contours.borrow_mut();
        let reuse = slot.as_ref().filter(|k| k.key() == key).and_then(|k| k.transform_to(&now)).filter(|t| gesture || !Contours::too_scaled(t));
        if let (Some(t), Some(k)) = (reuse, slot.as_ref()) {
            return Some(k.shape(&t));
        }
        let built = Contours::build(key, &lines_of(d, items), to_s, now, color);
        let shape = built.shape(&Affine::IDENTITY);
        *slot = Some(built);
        Some(shape)
    }
}

/// Les lignes qui dessinent les éléments `items` de `d` (points du dessin).
pub fn lines_of(d: &VectorDrawing, items: &[usize]) -> Vec<Vec<Pos2>> {
    items.iter().filter_map(|&i| d.elements.get(i)).flat_map(Element::outline).filter(|l| l.len() >= 2).collect()
}

/// Une empreinte de la sélection, pour savoir si les contours gardés valent encore : les éléments
/// choisis, leur nombre de points, quelques-uns de leurs points, et la version du dessin. Coûte un
/// élément, pas un point.
pub fn fingerprint(cel: u64, version: u64, d: &VectorDrawing, items: &[usize]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut put = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    put(cel);
    put(version);
    put(items.len() as u64);
    let bits = |x: f32, y: f32| ((x.to_bits() as u64) << 32) | y.to_bits() as u64;
    for &i in items {
        put(i as u64);
        match d.elements.get(i) {
            Some(Element::Stroke { points, .. }) => {
                put(points.len() as u64);
                for k in [0, points.len() / 4, points.len() / 2, 3 * points.len() / 4, points.len().saturating_sub(1)] {
                    if let Some(p) = points.get(k) {
                        put(bits(p.x, p.y));
                    }
                }
            }
            Some(Element::Fill { contours, .. }) => {
                put(contours.len() as u64);
                if let Some(c) = contours.first() {
                    put(c.len() as u64);
                    for p in [c.first(), c.get(c.len() / 2), c.last()].into_iter().flatten() {
                        put(bits(p[0], p[1]));
                    }
                }
            }
            Some(Element::Text { text, size, place, .. }) => {
                put(text.len() as u64);
                put(size.to_bits() as u64);
                for pair in place.chunks(2) {
                    put(bits(pair[0], pair[1]));
                }
            }
            None => put(u64::MAX),
        }
    }
    h
}
