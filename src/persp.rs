//! Déformer en perspective (5f), sans interface : homographie qui envoie un rectangle sur un
//! quadrilatère (ses quatre coins déplacés), appliquée à une image (échantillonnage inverse,
//! bilinéaire) ou aux points d'éléments vectoriels.
//!
//! Comme l'outil Perspective de Storyboard Pro 27 (DRAWING_PERSPECTIVE_TOOL_KEY) : on tire les
//! quatre coins. Son rendu exact n'a pas été comparé.

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::raster::{Raster, TILE};
use crate::vector::{Affine, Element, VPoint, VectorDrawing};

/// Homographie 3×3 (ligne par ligne, dernier terme 1) : x' = (a·x + b·y + c) / (g·x + h·y + 1).
pub type Homography = [f32; 9];

/// Image du carré unité (0,0), (1,0), (1,1), (0,1) sur les coins `q` (dans cet ordre).
pub fn square_to_quad(q: [Pos2; 4]) -> Option<Homography> {
    let [p0, p1, p2, p3] = q;
    let (dx1, dx2, dx3) = (p1.x - p2.x, p3.x - p2.x, p0.x - p1.x + p2.x - p3.x);
    let (dy1, dy2, dy3) = (p1.y - p2.y, p3.y - p2.y, p0.y - p1.y + p2.y - p3.y);
    if dx3.abs() < 1e-6 && dy3.abs() < 1e-6 {
        // Parallélogramme : transformation affine.
        return Some([p1.x - p0.x, p3.x - p0.x, p0.x, p1.y - p0.y, p3.y - p0.y, p0.y, 0.0, 0.0, 1.0]);
    }
    let den = dx1 * dy2 - dx2 * dy1;
    if den.abs() < 1e-9 {
        return None;
    }
    let g = (dx3 * dy2 - dx2 * dy3) / den;
    let h = (dx1 * dy3 - dx3 * dy1) / den;
    Some([
        p1.x - p0.x + g * p1.x,
        p3.x - p0.x + h * p3.x,
        p0.x,
        p1.y - p0.y + g * p1.y,
        p3.y - p0.y + h * p3.y,
        p0.y,
        g,
        h,
        1.0,
    ])
}

/// Produit de deux homographies : `a` après `b`.
pub fn mul(a: &Homography, b: &Homography) -> Homography {
    let mut m = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            m[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
        }
    }
    m
}

/// Le rectangle `r` sur les coins `q` (haut-gauche, haut-droite, bas-droite, bas-gauche).
pub fn rect_to_quad(r: Rect, q: [Pos2; 4]) -> Option<Homography> {
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return None;
    }
    let to_unit = [1.0 / r.width(), 0.0, -r.min.x / r.width(), 0.0, 1.0 / r.height(), -r.min.y / r.height(), 0.0, 0.0, 1.0];
    Some(mul(&square_to_quad(q)?, &to_unit))
}

pub fn apply(h: &Homography, p: Pos2) -> Pos2 {
    let w = h[6] * p.x + h[7] * p.y + h[8];
    let w = if w.abs() < 1e-9 { 1e-9_f32.copysign(w) } else { w };
    pos2((h[0] * p.x + h[1] * p.y + h[2]) / w, (h[3] * p.x + h[4] * p.y + h[5]) / w)
}

pub fn inverse(m: &Homography) -> Option<Homography> {
    let [a, b, c, d, e, f, g, h, i] = *m;
    let co = [e * i - f * h, -(d * i - f * g), d * h - e * g];
    let det = a * co[0] + b * co[1] + c * co[2];
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = [
        co[0],
        -(b * i - c * h),
        b * f - c * e,
        co[1],
        a * i - c * g,
        -(a * f - c * d),
        co[2],
        -(a * h - b * g),
        a * e - b * d,
    ];
    let s = inv[8];
    let k = if s.abs() > 1e-12 { 1.0 / s } else { 1.0 / det };
    Some(inv.map(|v| v * k))
}

/// Les quatre coins sont-ils dans le bon ordre (quadrilatère convexe, sans croisement) ?
pub fn is_convex(q: &[Pos2; 4]) -> bool {
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let s: Vec<f32> = (0..4).map(|i| cross(q[i], q[(i + 1) % 4], q[(i + 2) % 4])).collect();
    s.iter().all(|v| *v > 1e-3) || s.iter().all(|v| *v < -1e-3)
}

/// Boîte des pixels non transparents (au pixel près).
pub fn raster_bounds(r: &Raster) -> Option<Rect> {
    let mut b = Rect::NOTHING;
    for (idx, tile) in r.tiles.iter().enumerate() {
        let Some(tile) = tile else { continue };
        let (x0, y0, w, h) = r.tile_bounds(idx);
        for ly in 0..h {
            for lx in 0..w {
                if tile[ly * TILE + lx].a() > 0 {
                    let (x, y) = ((x0 + lx) as f32, (y0 + ly) as f32);
                    b = b.union(Rect::from_min_max(pos2(x, y), pos2(x + 1.0, y + 1.0)));
                }
            }
        }
    }
    b.is_positive().then_some(b)
}

/// Couleur bilinéaire de l'image `r` au point (`x`, `y`) (aussi pour `envelope.rs`).
pub(crate) fn sample(r: &Raster, x: f32, y: f32) -> Color32 {
    let (x, y) = (x - 0.5, y - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let get = |xi: f32, yi: f32| -> [f32; 4] {
        if xi < 0.0 || yi < 0.0 || xi >= r.width as f32 || yi >= r.height as f32 {
            return [0.0; 4];
        }
        let c = r.get(xi as usize, yi as usize);
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let (a, b, c, d) = (get(x0, y0), get(x0 + 1.0, y0), get(x0, y0 + 1.0), get(x0 + 1.0, y0 + 1.0));
    let mix = |k: usize| {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_premultiplied(mix(0), mix(1), mix(2), mix(3))
}

/// L'image `original` dont le rectangle `src` est posé sur les coins `quad` : le reste de
/// l'image ne bouge pas, le rectangle d'origine est vidé.
pub fn warp_raster(original: &Raster, src: Rect, quad: [Pos2; 4]) -> Option<Raster> {
    let h = rect_to_quad(src, quad)?;
    let inv = inverse(&h)?;
    let mut out = original.clone();
    // Vider le rectangle d'origine.
    let (x0, y0) = (src.min.x.floor().max(0.0) as usize, src.min.y.floor().max(0.0) as usize);
    let (x1, y1) = ((src.max.x.ceil().max(0.0) as usize).min(out.width), (src.max.y.ceil().max(0.0) as usize).min(out.height));
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = out.tile_index_of(x, y);
            if out.tiles[idx].is_some() {
                out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = Color32::TRANSPARENT;
            }
        }
    }
    // Chaque pixel du quadrilatère prend le pixel d'origine correspondant.
    let b = Rect::from_points(&quad);
    let (qx0, qy0) = (b.min.x.floor().max(0.0) as usize, b.min.y.floor().max(0.0) as usize);
    let (qx1, qy1) = ((b.max.x.ceil().max(0.0) as usize).min(out.width), (b.max.y.ceil().max(0.0) as usize).min(out.height));
    let inside = src.expand(0.5);
    for y in qy0..qy1 {
        for x in qx0..qx1 {
            let s = apply(&inv, pos2(x as f32 + 0.5, y as f32 + 0.5));
            if !inside.contains(s) {
                continue;
            }
            let c = sample(original, s.x, s.y);
            if c.a() == 0 {
                continue;
            }
            let idx = out.tile_index_of(x, y);
            out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
        }
    }
    Some(out)
}

/// Un élément vectoriel déformé par `h` (traits : leurs points ; épaisseur à l'échelle moyenne
/// `k` ; texte : la transformation affine la plus proche, prise sur sa boîte).
pub fn warp_element(e: &Element, h: &Homography, k: f32) -> Element {
    warp_element_with(e, &|p| apply(h, p), k)
}

/// `warp_element` avec une déformation quelconque `f` (l'enveloppe de Bézier, `envelope.rs`).
pub fn warp_element_with(e: &Element, f: &impl Fn(Pos2) -> Pos2, k: f32) -> Element {
    let map = |x: f32, y: f32| {
        let q = f(pos2(x, y));
        [q.x, q.y]
    };
    match e {
        Element::Stroke { points, color, opacity, hardness, brush, trace } => Element::Stroke {
            points: points
                .iter()
                .map(|p| {
                    let [x, y] = map(p.x, p.y);
                    VPoint { x, y, r: (p.r * k).clamp(0.2, 200.0) }
                })
                .collect(),
            color: *color,
            opacity: *opacity,
            hardness: *hardness,
            brush: brush.as_ref().map(|b| Box::new(crate::brush::BrushSettings { size: b.size * k, ..**b })),
            trace: trace.without_cache(),
        },
        Element::Fill { contours, color, opacity } => Element::Fill {
            contours: contours.iter().map(|c| c.iter().map(|p| map(p[0], p[1])).collect()).collect(),
            color: *color,
            opacity: *opacity,
        },
        Element::Text { text, font, size, color, opacity, place } => {
            let mut out = Element::Text { text: text.clone(), font: font.clone(), size: *size, color: *color, opacity: *opacity, place: *place };
            if let Some((corners, _)) = e.text_shape() {
                // Coins haut-gauche, haut-droite, bas-gauche déformés : l'affine qui les y envoie.
                let (o, x, y) = (f(corners[0]), f(corners[1]), f(corners[3]));
                let a0 = Affine::from_array(*place);
                if let Some(inv) = a0.inverse() {
                    let (lo, lx, ly) = (inv.apply(corners[0]), inv.apply(corners[1]), inv.apply(corners[3]));
                    let (w, hh) = ((lx.x - lo.x).max(1e-3), (ly.y - lo.y).max(1e-3));
                    let fit = Affine {
                        a: (x.x - o.x) / w,
                        b: (y.x - o.x) / hh,
                        c: (x.y - o.y) / w,
                        d: (y.y - o.y) / hh,
                        tx: 0.0,
                        ty: 0.0,
                    };
                    let fit = Affine { tx: o.x - (fit.a * lo.x + fit.b * lo.y), ty: o.y - (fit.c * lo.x + fit.d * lo.y), ..fit };
                    if let Element::Text { place, .. } = &mut out {
                        *place = fit.to_array();
                    }
                }
            }
            out
        }
    }
}

/// Le dessin vectoriel `original` dont les éléments `items` (tous si `None`), compris dans
/// `src`, sont posés sur les coins `quad`.
pub fn warp_drawing(original: &VectorDrawing, items: Option<&[usize]>, src: Rect, quad: [Pos2; 4]) -> Option<VectorDrawing> {
    let h = rect_to_quad(src, quad)?;
    let area = |q: &[Pos2; 4]| {
        0.5 * (0..4).map(|i| q[i].x * q[(i + 1) % 4].y - q[(i + 1) % 4].x * q[i].y).sum::<f32>().abs()
    };
    let k = (area(&quad) / (src.width() * src.height()).max(1e-6)).sqrt();
    let mut out = original.clone();
    for (i, e) in out.elements.iter_mut().enumerate() {
        if items.is_none_or(|list| list.contains(&i)) {
            *e = warp_element(e, &h, k);
        }
    }
    Some(out)
}
