//! Dessin vectoriel façon Storyboard Pro : des traits à épaisseur variable et des zones remplies
//! qui restent modifiables (sélection, déplacement, rotation, couleur, gomme qui coupe les traits).

use std::collections::HashMap;

use eframe::egui::{pos2, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::brush::{coverage, paint};
use crate::raster::{Raster, TILE};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VPoint {
    pub x: f32,
    pub y: f32,
    /// Rayon du trait en ce point.
    pub r: f32,
}

impl VPoint {
    pub fn pos(&self) -> Pos2 {
        pos2(self.x, self.y)
    }

    fn lerp(a: VPoint, b: VPoint, t: f32) -> VPoint {
        VPoint {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            r: a.r + (b.r - a.r) * t,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Element {
    /// Trait à épaisseur variable le long d'une ligne médiane.
    Stroke {
        points: Vec<VPoint>,
        color: [u8; 3],
        opacity: f32,
        hardness: f32,
    },
    /// Zone remplie, délimitée par un ou plusieurs contours (pair-impair : les trous restent vides).
    Fill {
        contours: Vec<Vec<[f32; 2]>>,
        color: [u8; 3],
        opacity: f32,
    },
}

impl Element {
    pub fn bounds(&self) -> Rect {
        let mut r = Rect::NOTHING;
        match self {
            Element::Stroke { points, .. } => {
                for p in points {
                    r = r.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0)));
                }
            }
            Element::Fill { contours, .. } => {
                for p in contours.iter().flatten() {
                    r.extend_with(pos2(p[0], p[1]));
                }
            }
        }
        r
    }

    pub fn set_color(&mut self, c: [u8; 3]) {
        match self {
            Element::Stroke { color, .. } | Element::Fill { color, .. } => *color = c,
        }
    }

    pub fn scale_width(&mut self, k: f32) {
        if let Element::Stroke { points, .. } = self {
            for p in points {
                p.r = (p.r * k).clamp(0.2, 200.0);
            }
        }
    }

    pub fn transformed(&self, a: &Affine) -> Element {
        match self {
            Element::Stroke {
                points,
                color,
                opacity,
                hardness,
            } => {
                let k = a.width_factor();
                Element::Stroke {
                    points: points
                        .iter()
                        .map(|p| {
                            let q = a.apply(p.pos());
                            VPoint { x: q.x, y: q.y, r: p.r * k }
                        })
                        .collect(),
                    color: *color,
                    opacity: *opacity,
                    hardness: *hardness,
                }
            }
            Element::Fill {
                contours,
                color,
                opacity,
            } => Element::Fill {
                contours: contours
                    .iter()
                    .map(|c| {
                        c.iter()
                            .map(|p| {
                                let q = a.apply(pos2(p[0], p[1]));
                                [q.x, q.y]
                            })
                            .collect()
                    })
                    .collect(),
                color: *color,
                opacity: *opacity,
            },
        }
    }

    /// Le point `p` touche-t-il l'élément (à `tol` près) ?
    pub fn hit(&self, p: Pos2, tol: f32) -> bool {
        if !self.bounds().expand(tol).contains(p) {
            return false;
        }
        match self {
            Element::Stroke { points, .. } => {
                if points.len() == 1 {
                    return points[0].pos().distance(p) <= points[0].r + tol;
                }
                points.windows(2).any(|s| {
                    let (d, t) = segment_distance(p, s[0].pos(), s[1].pos());
                    d <= s[0].r + (s[1].r - s[0].r) * t + tol
                })
            }
            Element::Fill { contours, .. } => inside_contours(p, contours),
        }
    }

    /// L'élément a-t-il au moins un point dans le lasso ?
    pub fn touches(&self, lasso: &[Pos2]) -> bool {
        match self {
            Element::Stroke { points, .. } => points.iter().any(|q| point_in_polygon(q.pos(), lasso)),
            Element::Fill { contours, .. } => contours
                .iter()
                .flatten()
                .any(|q| point_in_polygon(pos2(q[0], q[1]), lasso)),
        }
    }

    /// Lignes qui dessinent l'élément sélectionné à l'écran.
    pub fn outline(&self) -> Vec<Vec<Pos2>> {
        match self {
            Element::Stroke { points, .. } => vec![points.iter().map(VPoint::pos).collect()],
            Element::Fill { contours, .. } => contours
                .iter()
                .map(|c| {
                    let mut line: Vec<Pos2> = c.iter().map(|p| pos2(p[0], p[1])).collect();
                    if let Some(&first) = line.first() {
                        line.push(first);
                    }
                    line
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorDrawing {
    pub elements: Vec<Element>,
}

impl VectorDrawing {
    pub fn render(&self, width: usize, height: usize) -> Raster {
        let mut raster = Raster::new(width, height);
        for e in &self.elements {
            render_element(&mut raster, e);
        }
        raster
    }

    /// Élément le plus haut sous le point.
    pub fn hit_test(&self, p: Pos2, tol: f32) -> Option<usize> {
        (0..self.elements.len())
            .rev()
            .find(|&i| self.elements[i].hit(p, tol))
    }

    /// Simplifie les traits qui touchent `region` (les autres ne bougent pas d'un pixel).
    pub fn simplify_strokes_in(&mut self, tol: f32, region: Rect) {
        for e in &mut self.elements {
            if !e.bounds().intersects(region) {
                continue;
            }
            if let Element::Stroke { points, .. } = e {
                *points = simplify(points, tol);
            }
        }
    }

    /// Les remplissages au pot passent sous les traits, après les remplissages déjà là.
    pub fn fill_insert_index(&self) -> usize {
        self.elements
            .iter()
            .take_while(|e| matches!(e, Element::Fill { .. }))
            .count()
    }
}

/// Transformation affine : p' = (a·x + b·y + tx, c·x + d·y + ty).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn translate(v: Vec2) -> Self {
        Self {
            tx: v.x,
            ty: v.y,
            ..Self::IDENTITY
        }
    }

    pub fn scale_about(o: Pos2, sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx: o.x - sx * o.x,
            ty: o.y - sy * o.y,
        }
    }

    pub fn rotate_about(o: Pos2, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            a: c,
            b: -s,
            c: s,
            d: c,
            tx: o.x - (c * o.x - s * o.y),
            ty: o.y - (s * o.x + c * o.y),
        }
    }

    pub fn apply(&self, p: Pos2) -> Pos2 {
        pos2(
            self.a * p.x + self.b * p.y + self.tx,
            self.c * p.x + self.d * p.y + self.ty,
        )
    }

    /// Facteur appliqué aux épaisseurs (moyenne géométrique des échelles).
    pub fn width_factor(&self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    /// `self` puis `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        Affine {
            a: next.a * self.a + next.b * self.c,
            b: next.a * self.b + next.b * self.d,
            c: next.c * self.a + next.d * self.c,
            d: next.c * self.b + next.d * self.d,
            tx: next.a * self.tx + next.b * self.ty + next.tx,
            ty: next.c * self.tx + next.d * self.ty + next.ty,
        }
    }

    /// Transformation inverse (`None` si elle écrase tout sur une ligne).
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-8 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Affine {
            a,
            b,
            c,
            d,
            tx: -(a * self.tx + b * self.ty),
            ty: -(c * self.tx + d * self.ty),
        })
    }

    /// Rectangle englobant de `r` transformé.
    pub fn apply_rect(&self, r: Rect) -> Rect {
        Rect::from_points(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].map(|p| self.apply(p)))
    }
}

/// Couverture (0 à 1) d'un ou plusieurs polygones (règle pair-impair), anticrénelée, sur la partie
/// de l'image qu'ils recouvrent : (x0, y0, largeur, hauteur, couvertures ligne par ligne).
pub fn polygon_coverage(contours: &[Vec<[f32; 2]>], width: usize, height: usize) -> Option<(usize, usize, usize, usize, Vec<f32>)> {
    let pts: Vec<Pos2> = contours.iter().flatten().map(|p| pos2(p[0], p[1])).collect();
    let mut mask = Mask::new(Rect::from_points(&pts).expand(1.0), width, height)?;
    fill_even_odd(&mut mask, contours);
    for v in &mut mask.data {
        *v = v.min(1.0);
    }
    Some((mask.x0, mask.y0, mask.w, mask.h, mask.data))
}

// ---------------------------------------------------------------------------------- rendu

/// Couverture (0 à 1) d'un élément sur une zone de l'image, avant de la peindre.
struct Mask {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl Mask {
    fn new(bounds: Rect, width: usize, height: usize) -> Option<Self> {
        if !bounds.is_finite() || bounds.max.x < 0.0 || bounds.max.y < 0.0 {
            return None;
        }
        let x0 = bounds.min.x.floor().max(0.0) as usize;
        let y0 = bounds.min.y.floor().max(0.0) as usize;
        let x1 = (bounds.max.x.ceil() as usize).min(width);
        let y1 = (bounds.max.y.ceil() as usize).min(height);
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        Some(Self {
            x0,
            y0,
            w: x1 - x0,
            h: y1 - y0,
            data: vec![0.0; (x1 - x0) * (y1 - y0)],
        })
    }

    fn stamp(&mut self, cx: f32, cy: f32, radius: f32, hardness: f32) {
        let (mut radius, mut flow) = (radius, 1.0);
        if radius < 0.75 {
            flow = (radius / 0.75).max(0.0);
            radius = 0.75;
        }
        let inner = (radius * hardness.clamp(0.0, 1.0)).min(radius - 0.5).max(0.0);
        let lx0 = ((cx - radius - 1.0).floor() - self.x0 as f32).max(0.0) as usize;
        let ly0 = ((cy - radius - 1.0).floor() - self.y0 as f32).max(0.0) as usize;
        let lx1 = (((cx + radius + 1.0).ceil() - self.x0 as f32).max(0.0) as usize).min(self.w);
        let ly1 = (((cy + radius + 1.0).ceil() - self.y0 as f32).max(0.0) as usize).min(self.h);
        for ly in ly0..ly1 {
            let dy = (self.y0 + ly) as f32 + 0.5 - cy;
            for lx in lx0..lx1 {
                let dx = (self.x0 + lx) as f32 + 0.5 - cx;
                let v = coverage((dx * dx + dy * dy).sqrt(), radius, inner, false) * flow;
                let m = &mut self.data[ly * self.w + lx];
                if v > *m {
                    *m = v;
                }
            }
        }
    }

    fn composite(&self, raster: &mut Raster, rgb: [u8; 3], opacity: f32) {
        for ly in 0..self.h {
            for lx in 0..self.w {
                let m = self.data[ly * self.w + lx];
                if m <= 0.0 {
                    continue;
                }
                let (x, y) = (self.x0 + lx, self.y0 + ly);
                let idx = raster.tile_index_of(x, y);
                let i = (y % TILE) * TILE + x % TILE;
                let tile = raster.tile_mut(idx);
                tile[i] = paint(tile[i], rgb, m.min(1.0) * opacity);
            }
        }
    }
}

pub fn render_element(raster: &mut Raster, e: &Element) {
    render_element_clipped(raster, e, Rect::EVERYTHING);
}

/// Dessine l'élément dans `clip` seulement (bords en pixels entiers) : chaque pixel du rectangle
/// reçoit exactement ce qu'un rendu complet lui donnerait.
pub fn render_element_clipped(raster: &mut Raster, e: &Element, clip: Rect) {
    let Some(mut mask) = Mask::new(e.bounds().expand(1.5).intersect(clip), raster.width, raster.height) else {
        return;
    };
    match e {
        Element::Stroke {
            points,
            color,
            opacity,
            hardness,
        } => {
            stamp_path(&mut mask, points, *hardness);
            mask.composite(raster, *color, *opacity);
        }
        Element::Fill {
            contours,
            color,
            opacity,
        } => {
            fill_even_odd(&mut mask, contours);
            mask.composite(raster, *color, *opacity);
        }
    }
}

fn stamp_path(mask: &mut Mask, points: &[VPoint], hardness: f32) {
    let Some(first) = points.first() else { return };
    mask.stamp(first.x, first.y, first.r, hardness);
    for s in points.windows(2) {
        let (a, b) = (s[0], s[1]);
        let len = a.pos().distance(b.pos());
        let step = (a.r.min(b.r) * 0.25).clamp(0.3, 3.0);
        let n = (len / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let p = VPoint::lerp(a, b, k as f32 / n as f32);
            mask.stamp(p.x, p.y, p.r, hardness);
        }
    }
}

/// Remplissage pair-impair, anticrénelé : 4 lignes de balayage par pixel et couverture
/// horizontale exacte.
fn fill_even_odd(mask: &mut Mask, contours: &[Vec<[f32; 2]>]) {
    const SUB: usize = 4;
    let mut edges = Vec::new();
    for c in contours.iter().filter(|c| c.len() >= 3) {
        for i in 0..c.len() {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            if a[1] != b[1] {
                edges.push((a[0], a[1], b[0], b[1]));
            }
        }
    }
    let ox = mask.x0 as f32;
    let mut xs: Vec<f32> = Vec::new();
    for ly in 0..mask.h {
        let row_y = (mask.y0 + ly) as f32;
        let row = &mut mask.data[ly * mask.w..(ly + 1) * mask.w];
        for s in 0..SUB {
            let sy = row_y + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for &(x0, y0, x1, y1) in &edges {
                if (y0 <= sy) != (y1 <= sy) {
                    xs.push(x0 + (sy - y0) * (x1 - x0) / (y1 - y0) - ox);
                }
            }
            xs.sort_by(|a, b| a.total_cmp(b));
            for pair in xs.chunks_exact(2) {
                add_span(row, pair[0], pair[1], 1.0 / SUB as f32);
            }
        }
    }
}

fn add_span(row: &mut [f32], a: f32, b: f32, weight: f32) {
    let a = a.max(0.0);
    let b = b.min(row.len() as f32);
    if b <= a {
        return;
    }
    let first = a.floor() as usize;
    let last = (b.ceil() as usize).min(row.len());
    for (i, v) in row.iter_mut().enumerate().take(last).skip(first) {
        let lo = (i as f32).max(a);
        let hi = ((i + 1) as f32).min(b);
        if hi > lo {
            *v += (hi - lo) * weight;
        }
    }
}

/// Rectangle couvert par un trait de points `path` (cercles de rayon `r`), anticrénelage compris.
pub fn path_bounds(path: &[VPoint]) -> Rect {
    path.iter()
        .fold(Rect::NOTHING, |r, p| r.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0 + 3.0))))
}

/// Tuiles de l'image recouvertes par `rect`.
pub fn tiles_in(raster: &Raster, rect: Rect) -> Vec<usize> {
    if !rect.is_finite()
        || rect.max.x < 0.0
        || rect.max.y < 0.0
        || rect.min.x >= raster.width as f32
        || rect.min.y >= raster.height as f32
    {
        return Vec::new();
    }
    let x0 = rect.min.x.max(0.0) as usize / TILE;
    let y0 = rect.min.y.max(0.0) as usize / TILE;
    let x1 = (rect.max.x as usize).min(raster.width - 1) / TILE;
    let y1 = (rect.max.y as usize).min(raster.height - 1) / TILE;
    let tiles_x = raster.tiles_x;
    (y0..=y1)
        .flat_map(|ty| (x0..=x1).map(move |tx| ty * tiles_x + tx))
        .collect()
}

// ---------------------------------------------------------------------------------- géométrie

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> (f32, f32) {
    let ab = b - a;
    let len2 = ab.length_sq();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((a + ab * t).distance(p), t)
}

pub fn point_in_polygon(p: Pos2, poly: &[Pos2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

fn inside_contours(p: Pos2, contours: &[Vec<[f32; 2]>]) -> bool {
    let mut inside = false;
    for c in contours {
        let n = c.len();
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            if (a[1] > p.y) != (b[1] > p.y) && p.x < a[0] + (p.y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Douglas-Peucker : indique quels points garder.
fn rdp_keep(n: usize, tol: f32, dist: impl Fn(usize, usize, usize) -> f32) -> Vec<bool> {
    let mut keep = vec![false; n];
    if n == 0 {
        return keep;
    }
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0, n - 1)];
    while let Some((s, e)) = stack.pop() {
        if e <= s + 1 {
            continue;
        }
        let mut best = (0.0f32, s);
        for i in s + 1..e {
            let d = dist(i, s, e);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    keep
}

fn dist3(p: VPoint, a: VPoint, b: VPoint) -> f32 {
    let ab = [b.x - a.x, b.y - a.y, b.r - a.r];
    let ap = [p.x - a.x, p.y - a.y, p.r - a.r];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let d = [ap[0] - ab[0] * t, ap[1] - ab[1] * t, ap[2] - ab[2] * t];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Allège un trait sans le déformer de plus de `tol` pixels (épaisseur comprise).
pub fn simplify(points: &[VPoint], tol: f32) -> Vec<VPoint> {
    let mut pts: Vec<VPoint> = Vec::with_capacity(points.len());
    for p in points {
        if pts
            .last()
            .is_none_or(|q| (q.x - p.x).abs() + (q.y - p.y).abs() > 1e-3)
        {
            pts.push(*p);
        }
    }
    if pts.len() <= 2 {
        return pts;
    }
    let keep = rdp_keep(pts.len(), tol, |i, s, e| dist3(pts[i], pts[s], pts[e]));
    pts.into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect()
}

fn simplify_open(pts: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    let p = |i: usize| pos2(pts[i][0], pts[i][1]);
    let keep = rdp_keep(pts.len(), tol, |i, s, e| segment_distance(p(i), p(s), p(e)).0);
    pts.iter()
        .zip(keep)
        .filter_map(|(q, k)| k.then_some(*q))
        .collect()
}

fn simplify_closed(pts: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    let n = pts.len();
    if n < 4 {
        return pts.to_vec();
    }
    let d2 = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let far = (1..n)
        .max_by(|&i, &j| d2(pts[i], pts[0]).total_cmp(&d2(pts[j], pts[0])))
        .unwrap_or(n / 2);
    let mut out = simplify_open(&pts[..=far], tol);
    let mut back = pts[far..].to_vec();
    back.push(pts[0]);
    let back = simplify_open(&back, tol);
    out.pop();
    out.extend_from_slice(&back[..back.len() - 1]);
    out
}

/// Contours (bords de pixels) d'une zone, simplifiés : la forme vectorielle d'un remplissage.
pub fn trace_contours(mask: &[bool], w: usize, h: usize) -> Vec<Vec<[f32; 2]>> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x0 == usize::MAX {
        return Vec::new();
    }
    let inside = |x: i64, y: i64| {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && mask[y as usize * w + x as usize]
    };
    // Chaque bord entre un pixel de la zone et un pixel extérieur devient une arête orientée.
    let mut next: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !mask[y * w + x] {
                continue;
            }
            let (x, y) = (x as i64, y as i64);
            if !inside(x, y - 1) {
                next.entry((x, y)).or_default().push((x + 1, y));
            }
            if !inside(x + 1, y) {
                next.entry((x + 1, y)).or_default().push((x + 1, y + 1));
            }
            if !inside(x, y + 1) {
                next.entry((x + 1, y + 1)).or_default().push((x, y + 1));
            }
            if !inside(x - 1, y) {
                next.entry((x, y + 1)).or_default().push((x, y));
            }
        }
    }
    let mut loops = Vec::new();
    let starts: Vec<(i64, i64)> = next.keys().copied().collect();
    for s in starts {
        while let Some(first) = next.get_mut(&s).and_then(|v| v.pop()) {
            let mut path = vec![s];
            let mut cur = first;
            while cur != s {
                path.push(cur);
                match next.get_mut(&cur).and_then(|v| v.pop()) {
                    Some(n) => cur = n,
                    None => break,
                }
            }
            loops.push(path);
        }
    }
    loops
        .into_iter()
        .filter_map(|lp| {
            let n = lp.len();
            let corners: Vec<[f32; 2]> = (0..n)
                .filter(|&i| {
                    let (a, b, c) = (lp[(i + n - 1) % n], lp[i], lp[(i + 1) % n]);
                    (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0) != 0
                })
                .map(|i| [lp[i].0 as f32, lp[i].1 as f32])
                .collect();
            if corners.len() < 3 {
                return None;
            }
            let simple = simplify_closed(&corners, 0.7);
            (simple.len() >= 3).then_some(simple)
        })
        .collect()
}

fn densify(points: &[VPoint], step: f32) -> Vec<VPoint> {
    let mut out = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            let a = points[i - 1];
            let n = (a.pos().distance(p.pos()) / step).ceil() as usize;
            for k in 1..n {
                out.push(VPoint::lerp(a, *p, k as f32 / n as f32));
            }
        }
        out.push(*p);
    }
    out
}

/// Gomme vectorielle : coupe les traits sous le cercle (ou les retire entiers si `whole`).
/// Les zones remplies ne sont retirées qu'en mode `whole`.
pub fn erase_circle(d: &mut VectorDrawing, c: Pos2, radius: f32, whole: bool) -> bool {
    let mut changed = false;
    let mut out = Vec::with_capacity(d.elements.len());
    for e in std::mem::take(&mut d.elements) {
        if !e.bounds().expand(radius).contains(c) {
            out.push(e);
            continue;
        }
        let cut: Option<Vec<Element>> = match &e {
            Element::Stroke {
                points,
                color,
                opacity,
                hardness,
            } => {
                if whole {
                    e.hit(c, radius).then(Vec::new)
                } else {
                    let mut pieces: Vec<Vec<VPoint>> = Vec::new();
                    let mut cur: Vec<VPoint> = Vec::new();
                    let mut removed = false;
                    for p in densify(points, (radius * 0.3).max(0.4)) {
                        if p.pos().distance(c) <= radius {
                            removed = true;
                            if !cur.is_empty() {
                                pieces.push(std::mem::take(&mut cur));
                            }
                        } else {
                            cur.push(p);
                        }
                    }
                    if !cur.is_empty() {
                        pieces.push(cur);
                    }
                    removed.then(|| {
                        pieces
                            .into_iter()
                            .filter(|p| p.len() >= 2)
                            .map(|points| Element::Stroke {
                                points,
                                color: *color,
                                opacity: *opacity,
                                hardness: *hardness,
                            })
                            .collect()
                    })
                }
            }
            Element::Fill { .. } => (whole && e.hit(c, 0.0)).then(Vec::new),
        };
        match cut {
            Some(parts) => {
                changed = true;
                out.extend(parts);
            }
            None => out.push(e),
        }
    }
    d.elements = out;
    changed
}
