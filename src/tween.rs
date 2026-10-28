//! Interpolation d'images (13l), « images intermédiaires » de TVPaint 12 (000020927/928,
//! 000025580, 000022168). Choix d'Anthony (2026-10-16), les deux :
//! - calque vectoriel : de vrais intervalles. L'élément de rang i du premier dessin passe à
//!   celui de rang i du second s'ils sont de même sorte (trait / trait, remplissage / remplissage
//!   au même nombre de contours) : points rééchantillonnés le long du trait, position, épaisseur,
//!   couleur et opacité en ligne droite ; un trait dessiné dans l'autre sens est retourné. Les
//!   éléments sans correspondant se fondent (l'un s'efface, l'autre apparaît) ;
//! - calque bitmap : un fondu entre les deux images, comme TVPaint.
//!
//! Le même calcul sert à étirer des images dans le temps et à changer la cadence du projet
//! (`resample`).

use eframe::egui::{pos2, Color32};

use crate::document::CelId;
use crate::raster::{Raster, TILE};
use crate::vector::{Element, Trace, VPoint, VectorDrawing};

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| lerp(a[i] as f32, b[i] as f32, t).round().clamp(0.0, 255.0) as u8)
}

/// `n` points régulièrement espacés le long de `points` (par la longueur du trait), avec, pour
/// chacun, l'appui et l'instant interpolés de `trace` si elle suit le chemin.
fn resample_traced(points: &[VPoint], trace: &Trace, n: usize) -> (Vec<VPoint>, Option<Vec<(f32, f64)>>) {
    if points.len() < 2 || n < 2 {
        return (vec![points.first().copied().unwrap_or(VPoint { x: 0.0, y: 0.0, r: 1.0 }); n.max(1)], None);
    }
    let mut acc = vec![0.0f32];
    for w in points.windows(2) {
        let d = w[0].pos().distance(w[1].pos());
        acc.push(acc.last().unwrap() + d);
    }
    let total = *acc.last().unwrap();
    let aligned = trace.aligned(points.len());
    let mut samples = Vec::with_capacity(if aligned { n } else { 0 });
    let pts = (0..n)
        .map(|k| {
            let s = total * k as f32 / (n - 1) as f32;
            let i = acc.partition_point(|&a| a <= s).clamp(1, points.len() - 1);
            let (a, b) = (points[i - 1], points[i]);
            let seg = (acc[i] - acc[i - 1]).max(1e-6);
            let t = ((s - acc[i - 1]) / seg).clamp(0.0, 1.0);
            if aligned {
                samples.extend(trace.sample(points.len(), i - 1, t));
            }
            VPoint { x: lerp(a.x, b.x, t), y: lerp(a.y, b.y, t), r: lerp(a.r, b.r, t) }
        })
        .collect();
    (pts, (aligned && samples.len() == n).then_some(samples))
}

fn resample_points(points: &[VPoint], n: usize) -> Vec<VPoint> {
    resample_traced(points, &Trace::default(), n).0
}

/// Même chose pour un contour fermé ([x, y]).
fn resample_contour(c: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    let mut pts: Vec<VPoint> = c.iter().map(|p| VPoint { x: p[0], y: p[1], r: 0.0 }).collect();
    if let Some(&first) = pts.first() {
        pts.push(first);
    }
    let mut out = resample_points(&pts, n + 1);
    out.pop();
    out.iter().map(|p| [p.x, p.y]).collect()
}

/// Un élément à l'opacité multipliée par `k` (pour les fondus).
fn faded(e: &Element, k: f32) -> Element {
    let mut e = e.clone();
    match &mut e {
        Element::Stroke { opacity, .. } | Element::Fill { opacity, .. } | Element::Text { opacity, .. } => *opacity *= k,
    }
    e
}

/// L'élément `a` devenu `b` à `t` (0 : a, 1 : b), s'ils se correspondent.
fn morph(a: &Element, b: &Element, t: f32) -> Option<Element> {
    match (a, b) {
        (
            Element::Stroke { points: pa, color: ca, opacity: oa, hardness: ha, brush, trace },
            Element::Stroke { points: pb, color: cb, opacity: ob, hardness: hb, trace: tb, .. },
        ) => {
            let n = pa.len().max(pb.len()).clamp(2, 512);
            let (ra, sa) = resample_traced(pa, trace, n);
            let (mut rb, mut sb) = resample_traced(pb, tb, n);
            // Trait dessiné dans l'autre sens : on le retourne.
            let (a0, a1, b0, b1) = (ra[0].pos(), ra[n - 1].pos(), rb[0].pos(), rb[n - 1].pos());
            if a0.distance(b1) + a1.distance(b0) < a0.distance(b0) + a1.distance(b1) {
                rb.reverse();
                if let Some(s) = &mut sb {
                    s.reverse();
                }
            }
            let points = ra.iter().zip(&rb).map(|(p, q)| VPoint { x: lerp(p.x, q.x, t), y: lerp(p.y, q.y, t), r: lerp(p.r, q.r, t) }).collect();
            // Les appuis suivent aussi, entre ceux des deux traits (les instants, s'ils sont connus
            // des deux côtés).
            let trace = match (sa, sb) {
                (Some(sa), Some(sb)) => {
                    let samples: Vec<(f32, f64)> = sa
                        .iter()
                        .zip(&sb)
                        .map(|(x, y)| (lerp(x.0, y.0, t), if x.1 >= 0.0 && y.1 >= 0.0 { x.1 + (y.1 - x.1) * t as f64 } else { -1.0 }))
                        .collect();
                    trace.with_samples(&samples)
                }
                (Some(sa), None) if t < 0.5 => trace.with_samples(&sa),
                _ => trace.without_samples(),
            };
            Some(Element::Stroke { points, color: lerp_rgb(*ca, *cb, t), opacity: lerp(*oa, *ob, t), hardness: lerp(*ha, *hb, t), brush: brush.clone(), trace })
        }
        (Element::Fill { contours: ka, color: ca, opacity: oa, gradient: ga }, Element::Fill { contours: kb, color: cb, opacity: ob, gradient: gb }) if ka.len() == kb.len() => {
            let contours = ka
                .iter()
                .zip(kb)
                .map(|(x, y)| {
                    let n = x.len().max(y.len()).clamp(3, 512);
                    let (rx, ry) = (resample_contour(x, n), resample_contour(y, n));
                    rx.iter().zip(&ry).map(|(p, q)| [lerp(p[0], q[0], t), lerp(p[1], q[1], t)]).collect()
                })
                .collect();
            // Deux dégradés de même genre se mélangent (axe, couleurs, opacités) ; sinon celui du plus proche.
            let gradient = match (ga, gb) {
                (Some(x), Some(y)) if x.genre == y.genre => Some(crate::degrade::Degrade::entre(x, y, t)),
                (None, None) => None,
                _ => *(if t < 0.5 { ga } else { gb }),
            };
            Some(Element::Fill { contours, color: lerp_rgb(*ca, *cb, t), opacity: lerp(*oa, *ob, t), gradient })
        }
        _ => None,
    }
}

/// Le dessin vectoriel entre `a` et `b` à `t` (0 : a, 1 : b).
pub fn tween_vector(a: &VectorDrawing, b: &VectorDrawing, t: f32) -> VectorDrawing {
    let t = t.clamp(0.0, 1.0);
    let mut elements = Vec::new();
    for i in 0..a.elements.len().max(b.elements.len()) {
        match (a.elements.get(i), b.elements.get(i)) {
            (Some(x), Some(y)) => match morph(x, y, t) {
                Some(m) => elements.push(m),
                None => {
                    elements.push(faded(x, 1.0 - t));
                    elements.push(faded(y, t));
                }
            },
            (Some(x), None) => elements.push(faded(x, 1.0 - t)),
            (None, Some(y)) => elements.push(faded(y, t)),
            (None, None) => {}
        }
    }
    elements.retain(|e| match e {
        Element::Stroke { opacity, .. } | Element::Fill { opacity, .. } | Element::Text { opacity, .. } => *opacity > 0.001,
    });
    VectorDrawing { elements }
}

/// Le fondu entre deux images bitmap à `t` (couleurs prémultipliées).
pub fn blend_raster(a: &Raster, b: &Raster, t: f32) -> Raster {
    let t = t.clamp(0.0, 1.0);
    let mut out = Raster::new(a.width, a.height);
    for idx in 0..out.tiles.len() {
        let (ta, tb) = (a.tiles.get(idx).and_then(|x| x.as_ref()), b.tiles.get(idx).and_then(|x| x.as_ref()));
        if ta.is_none() && tb.is_none() {
            continue;
        }
        let tile = out.tile_mut(idx);
        for k in 0..TILE * TILE {
            let ca = ta.map_or(Color32::TRANSPARENT, |x| x[k]);
            let cb = tb.map_or(Color32::TRANSPARENT, |x| x[k]);
            let m = |x: u8, y: u8| lerp(x as f32, y as f32, t).round() as u8;
            tile[k] = Color32::from_rgba_premultiplied(m(ca.r(), cb.r()), m(ca.g(), cb.g()), m(ca.b(), cb.b()), m(ca.a(), cb.a()));
        }
    }
    out
}

/// Ce qu'il faut à une image du calque rééchantillonné : un dessin existant, rien, ou un
/// intervalle entre deux dessins à `t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    Keep(Option<CelId>),
    Tween(CelId, CelId, f32),
}

/// Les `m` images qui remplacent les images `frames` étirées dans le temps : l'image j tombe
/// au temps j·n/m des images d'origine. Entre deux dessins différents qui se suivent, un
/// intervalle si `interpolate` (sinon le dessin d'avant est tenu). Les temps se règlent au
/// 1/64 d'image (deux images qui tombent au même endroit partagent leur intervalle).
pub fn resample(frames: &[Option<CelId>], m: usize, interpolate: bool) -> Vec<Slot> {
    let n = frames.len();
    if n == 0 {
        return vec![Slot::Keep(None); m];
    }
    (0..m)
        .map(|j| {
            let x = j as f64 * n as f64 / m.max(1) as f64;
            let q = (x * 64.0).round() / 64.0;
            let k = (q.floor() as usize).min(n - 1);
            let frac = (q - k as f64) as f32;
            match (frames[k], frames.get(k + 1).copied().flatten()) {
                (Some(a), Some(b)) if interpolate && a != b && frac > 0.0 => Slot::Tween(a, b, frac),
                (a, _) => Slot::Keep(a),
            }
        })
        .collect()
}

/// Une image du plan à une autre cadence : `f` × `to` / `from`, arrondie.
pub fn rescale_frame(f: usize, from: u32, to: u32) -> usize {
    ((f as f64) * to as f64 / from.max(1) as f64).round() as usize
}

/// Le premier point du premier trait d'un dessin vectoriel.
pub fn first_point(d: &VectorDrawing) -> Option<eframe::egui::Pos2> {
    d.elements.iter().find_map(|e| match e {
        Element::Stroke { points, .. } => points.first().map(|p| pos2(p.x, p.y)),
        _ => None,
    })
}
