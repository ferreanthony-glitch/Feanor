//! Dessins vectoriels d'un projet Storyboard Pro (14a-2). Le script d'export lit chaque dessin
//! vectoriel avec l'interface de script étendue de Storyboard Pro (`Drawing.query.getStrokes`,
//! modèle vectoriel décrit dans sa documentation : arts, calques plans, traits en courbes de
//! Bézier, zones colorées à gauche et à droite des traits) et l'écrit en JSON, avec les
//! couleurs de la palette (`ColorManager.getColorDataWithId`). Pelure en refait des éléments
//! vectoriels modifiables :
//! - une zone colorée devient un remplissage dont les contours sont les traits qui la bordent
//!   (une couleur d'un seul côté) ; Pelure remplit pair-impair, le sens des traits ne compte pas ;
//! - un trait de crayon (couleur et épaisseur) devient un trait à épaisseur variable.
//!
//! Les coordonnées du dessin ne sont pas en pixels, et la caméra du panneau ou le placement du
//! calque les déplacent : on cale le dessin sur l'image du même calque exportée par Storyboard
//! Pro (mêmes boîtes, même échelle), puis on vérifie que le dessin recalculé recouvre bien
//! l'image (sens de l'axe vertical compris). Sinon, le calque garde l'image.

use std::collections::HashMap;

use serde_json::Value;

use crate::raster::Raster;
use crate::vector::{Element, VPoint, VectorDrawing};

/// Recouvrement minimal entre le dessin recalculé et l'image exportée pour garder les vecteurs.
pub const MIN_OVERLAP: f32 = 0.6;

/// Éléments du dessin dans ses propres coordonnées (axe vertical vers le haut, comme le modèle
/// de Storyboard Pro), du dessous au-dessus : arts du fond au premier plan, et dans chaque
/// calque les remplissages puis les traits de crayon.
pub fn raw_elements(json: &Value) -> Vec<Element> {
    let colors: HashMap<String, [f32; 4]> = json
        .get("colors")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let a = v.as_array()?;
                    let c = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(255.0) as f32;
                    Some((k.clone(), [c(0), c(1), c(2), c(3)]))
                })
                .collect()
        })
        .unwrap_or_default();
    let color_of = |id: Option<&str>| -> ([u8; 3], f32) {
        let c = id.and_then(|id| colors.get(id)).copied().unwrap_or([0.0, 0.0, 0.0, 255.0]);
        ([c[0] as u8, c[1] as u8, c[2] as u8], (c[3] / 255.0).clamp(0.0, 1.0))
    };
    let mut arts: Vec<&Value> = json.get("arts").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    arts.sort_by_key(|a| a.get("art").and_then(Value::as_i64).unwrap_or(0));
    let mut out = Vec::new();
    for art in arts {
        let layers = art.get("strokes").and_then(|s| s.get("layers")).and_then(Value::as_array);
        for layer in layers.into_iter().flatten() {
            layer_elements(layer, &color_of, &mut out);
        }
    }
    out
}

/// Un trait du modèle : ses jointures de départ et d'arrivée, sa ligne échantillonnée.
struct Edge {
    from: i64,
    to: i64,
    points: Vec<[f32; 2]>,
    closed: bool,
    left: Option<i64>,
    right: Option<i64>,
}

fn layer_elements(layer: &Value, color_of: &dyn Fn(Option<&str>) -> ([u8; 3], f32), out: &mut Vec<Element>) {
    let shaders: Vec<Option<&str>> = layer
        .get("shaders")
        .and_then(Value::as_array)
        .map(|s| s.iter().map(|v| v.get("colorId").and_then(Value::as_str)).collect())
        .unwrap_or_default();
    let thickness_paths: Vec<&Value> = layer.get("thicknessPaths").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let strokes: Vec<&Value> = layer.get("strokes").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let mut edges = Vec::with_capacity(strokes.len());
    let mut pencils = Vec::new();
    for s in &strokes {
        let path = bezier_points(s.get("path"));
        if path.len() < 2 {
            continue;
        }
        let int = |k: &str| s.get(k).and_then(Value::as_i64);
        edges.push(Edge {
            from: int("fromJoint").unwrap_or(-1),
            to: int("toJoint").unwrap_or(-2),
            points: path.clone(),
            closed: s.get("closed").and_then(Value::as_bool).unwrap_or(false),
            left: int("shaderLeft"),
            right: int("shaderRight"),
        });
        if let Some(pencil) = s.get("pencilColorId").and_then(Value::as_str) {
            let (color, opacity) = color_of(Some(pencil));
            let th = s.get("thickness");
            let min = th.and_then(|t| t.get("minThickness")).and_then(Value::as_f64).unwrap_or(1.0) as f32;
            let max = th.and_then(|t| t.get("maxThickness")).and_then(Value::as_f64).unwrap_or(min as f64) as f32;
            let keys = th
                .and_then(|t| t.get("thicknessPath"))
                .and_then(Value::as_u64)
                .and_then(|i| thickness_paths.get(i as usize))
                .and_then(|p| p.get("keys"))
                .and_then(Value::as_array);
            let n = path.len();
            let points = path
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
                    VPoint { x: p[0], y: p[1], r: radius_at(keys, t).unwrap_or((min + max) * 0.25).max(0.1) }
                })
                .collect();
            pencils.push(Element::Stroke { points, color, opacity, hardness: 1.0, brush: None });
        }
    }
    // Zones colorées : pour chaque couleur, les traits qui la bordent d'un seul côté.
    for (c, id) in shaders.iter().enumerate() {
        let c = c as i64;
        let border: Vec<&Edge> = edges.iter().filter(|e| (e.left == Some(c)) != (e.right == Some(c))).collect();
        let rings = chain(&border);
        if rings.is_empty() {
            continue;
        }
        let (color, opacity) = color_of(*id);
        out.push(Element::Fill { contours: rings, color, opacity });
    }
    out.extend(pencils);
}

/// Rayon d'un trait de crayon à la position `t` (0 à 1) : la moitié de l'épaisseur, moyenne des
/// côtés gauche et droit.
fn radius_at(keys: Option<&Vec<Value>>, t: f32) -> Option<f32> {
    let keys = keys?;
    let k: Vec<(f32, f32)> = keys
        .iter()
        .filter_map(|k| {
            let f = |n: &str| k.get(n).and_then(Value::as_f64).map(|v| v as f32);
            Some((f("t")?, (f("leftThickness").unwrap_or(0.0) + f("rightThickness").unwrap_or(0.0)) * 0.25))
        })
        .collect();
    let first = *k.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    for w in k.windows(2) {
        if t <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-6);
            return Some(w[0].1 + (w[1].1 - w[0].1) * (t - w[0].0) / span);
        }
    }
    k.last().map(|l| l.1)
}

/// Points d'une courbe de Bézier du modèle : des points sur la courbe, entre lesquels zéro, un
/// ou deux points de contrôle (segment droit, quadratique, cubique).
pub fn bezier_points(path: Option<&Value>) -> Vec<[f32; 2]> {
    let Some(pts) = path.and_then(Value::as_array) else { return Vec::new() };
    let pts: Vec<([f32; 2], bool)> = pts
        .iter()
        .filter_map(|p| {
            let x = p.get("x")?.as_f64()? as f32;
            let y = p.get("y")?.as_f64()? as f32;
            Some(([x, y], p.get("onCurve").and_then(Value::as_bool).unwrap_or(true)))
        })
        .collect();
    let Some(&(first, _)) = pts.first() else { return Vec::new() };
    let mut out = vec![first];
    let mut i = 0;
    while i + 1 < pts.len() {
        let Some(j) = (i + 1..pts.len()).find(|&j| pts[j].1) else { break };
        let a = pts[i].0;
        let b = pts[j].0;
        let ctrl: Vec<[f32; 2]> = pts[i + 1..j].iter().map(|p| p.0).collect();
        let len = std::iter::once(a).chain(ctrl.iter().copied()).chain(std::iter::once(b)).collect::<Vec<_>>().windows(2).map(|w| dist(w[0], w[1])).sum::<f32>();
        let steps = ((len / 40.0).ceil() as usize).clamp(1, 32);
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            out.push(match ctrl.len() {
                0 => lerp(a, b, t),
                1 => {
                    let (p, q) = (lerp(a, ctrl[0], t), lerp(ctrl[0], b, t));
                    lerp(p, q, t)
                }
                _ => {
                    let (c1, c2) = (ctrl[0], ctrl[ctrl.len() - 1]);
                    let (p, q, r) = (lerp(a, c1, t), lerp(c1, c2, t), lerp(c2, b, t));
                    lerp(lerp(p, q, t), lerp(q, r, t), t)
                }
            });
        }
        i = j;
    }
    out
}

fn lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Les traits bout à bout, par leurs jointures, en contours fermés. En pair-impair, la façon
/// d'apparier les traits à une jointure ne change pas la zone remplie : seul compte l'ensemble
/// des traits.
fn chain(edges: &[&Edge]) -> Vec<Vec<[f32; 2]>> {
    let mut rings = Vec::new();
    let mut used = vec![false; edges.len()];
    for (i, e) in edges.iter().enumerate() {
        if e.closed || e.from == e.to {
            used[i] = true;
            rings.push(e.points.clone());
        }
    }
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let first = edges[start].from;
        let mut cur = edges[start].to;
        let mut ring = edges[start].points.clone();
        while cur != first {
            let Some(k) = (0..edges.len()).find(|&k| !used[k] && (edges[k].from == cur || edges[k].to == cur)) else { break };
            used[k] = true;
            let e = edges[k];
            if e.from == cur {
                ring.extend(e.points.iter().skip(1));
                cur = e.to;
            } else {
                ring.extend(e.points.iter().rev().skip(1));
                cur = e.from;
            }
        }
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }
    rings
}

/// Boîte (x0, y0, x1, y1) des pixels assez opaques d'une image.
fn alpha_box(r: &Raster) -> Option<(f32, f32, f32, f32)> {
    let mut b: Option<(usize, usize, usize, usize)> = None;
    for (idx, t) in r.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = r.tile_bounds(idx);
        for y in 0..th {
            for x in 0..tw {
                if t[y * crate::raster::TILE + x].a() > 32 {
                    let (px, py) = (ox + x, oy + y);
                    b = Some(match b {
                        None => (px, py, px, py),
                        Some((a, c, d, e)) => (a.min(px), c.min(py), d.max(px), e.max(py)),
                    });
                }
            }
        }
    }
    b.map(|(a, c, d, e)| (a as f32, c as f32, d as f32 + 1.0, e as f32 + 1.0))
}

/// Part commune des pixels opaques des deux images (intersection sur union), un pixel sur deux.
fn overlap(a: &Raster, b: &Raster) -> f32 {
    let (mut inter, mut union) = (0usize, 0usize);
    let (w, h) = (a.width.min(b.width), a.height.min(b.height));
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let (p, q) = (a.get(x, y).a() > 32, b.get(x, y).a() > 32);
            inter += (p && q) as usize;
            union += (p || q) as usize;
        }
    }
    if union == 0 {
        0.0
    } else {
        inter as f32 / union as f32
    }
}

/// Le dessin `raw` calé sur l'image `target` (une feuille de Pelure) : mêmes boîtes, même
/// échelle sur les deux axes, axe vertical dans le sens qui recouvre le mieux l'image. Rien si
/// les proportions ne correspondent pas (dessin tourné, déformé) ou si le recouvrement est
/// trop faible : le calque gardera l'image.
pub fn fit_to_image(raw: &[Element], target: &Raster) -> Option<Fit> {
    let (tx0, ty0, tx1, ty1) = alpha_box(target)?;
    let rb = raw.iter().fold(egui_rect_nothing(), |r, e| r.union(e.bounds()));
    if !rb.is_positive() {
        return None;
    }
    let (sx, sy) = ((tx1 - tx0) / rb.width(), (ty1 - ty0) / rb.height());
    let s = (sx + sy) * 0.5;
    if !s.is_finite() || s <= 0.0 || (sx - sy).abs() / s > 0.2 {
        return None;
    }
    let mut best: Option<Fit> = None;
    for y_up in [true, false] {
        let place = |p: [f32; 2]| -> [f32; 2] {
            let x = tx0 + (p[0] - rb.min.x) * s;
            let y = if y_up { ty0 + (rb.max.y - p[1]) * s } else { ty0 + (p[1] - rb.min.y) * s };
            [x, y]
        };
        let placed: Vec<Element> = raw
            .iter()
            .map(|e| match e {
                Element::Stroke { points, color, opacity, hardness, brush } => Element::Stroke {
                    points: points
                        .iter()
                        .map(|p| {
                            let q = place([p.x, p.y]);
                            VPoint { x: q[0], y: q[1], r: (p.r * s).max(0.3) }
                        })
                        .collect(),
                    color: *color,
                    opacity: *opacity,
                    hardness: *hardness,
                    brush: brush.clone(),
                },
                Element::Fill { contours, color, opacity } => Element::Fill {
                    contours: contours.iter().map(|c| c.iter().map(|p| place(*p)).collect()).collect(),
                    color: *color,
                    opacity: *opacity,
                },
                other => other.clone(),
            })
            .collect();
        let drawing = VectorDrawing { elements: placed };
        let rendered = drawing.render(target.width, target.height);
        let score = overlap(&rendered, target);
        if best.as_ref().is_none_or(|b| score > b.score) {
            best = Some(Fit { elements: drawing.elements, score, scale: s, y_up, origin: place([0.0, 0.0]) });
        }
    }
    best.filter(|f| f.score >= MIN_OVERLAP)
}

/// Un dessin calé (`fit_to_image`) : ses éléments sur la feuille, le recouvrement obtenu, et
/// la mesure du calage, qui servira à l'export des vecteurs (14b-2) : pixels de la feuille par
/// unité de Storyboard Pro, sens de l'axe vertical, et où tombe l'origine (0, 0) sur la feuille.
pub struct Fit {
    pub elements: Vec<Element>,
    pub score: f32,
    pub scale: f32,
    pub y_up: bool,
    pub origin: [f32; 2],
}

fn egui_rect_nothing() -> eframe::egui::Rect {
    eframe::egui::Rect::NOTHING
}
