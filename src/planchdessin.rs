//! Le crayon de la planche de références (Anthony, 2026-11-05 : « un outil de dessin dans la planche de références », sur
//! un canevas blanc). Un trait de crayon est un élément de la planche comme un autre (`Kind::Stroke`) : il se range
//! dans `Document::references`, donc dans la même étape d'annulation que le reste de la planche, dans le même fichier
//! (`references.json`) et avec les mêmes gestes (choisir, déplacer, agrandir, tourner, supprimer, emporté par un cadre).
//!
//! - **Vectoriel léger** : des points `[x, y, pression]` par rapport au centre du trait, en unités du tableau. Les traits
//!   suivent donc le zoom, et rien n'est tramé. L'épaisseur (`Reference::width`) est celle d'une pression pleine ; la
//!   pression du stylet l'amincit (`pressure_factor`), la souris donne toujours la pression pleine.
//! - **Par-dessus tout** : les traits sont peints après les images, les post-it et les cadres, et attrapés les premiers
//!   (`references::hit_test_near`), quel que soit leur rang dans la pile.
//! - **Rien qui grossisse** : les points sont partagés (`Arc`), la planche copie ses éléments à chaque image sans les
//!   recopier ; un trait hors de l'écran n'est pas peint, un trait peint saute les points plus proches que 0,75 point
//!   d'écran ; un trait n'est testé au clic que s'il est sous la souris (`stroke_hit`).
//! - **Fichier** : trois champs en plus (`points`, `encre`, `epaisseur`), tous `#[serde(default)]` et absents de tout autre
//!   élément. Le fichier passe en format 2 comme pour un post-it ; `project::VERSION` ne change pas.
//! - **Jamais ailleurs** : ces traits ne sont ni dans un plan, ni dans le film, ni dans un export ; ils ne sont pas non plus
//!   des annotations de la planche des Vignettes (`annotations.rs`).
//!
//! L'écran (outils, couleur, taille, gomme, gestes) est dans `planchdessinui.rs`.

use std::sync::Arc;

use eframe::egui::{self, epaint::Vertex, vec2, Color32, Mesh, Painter, Pos2, Rect, Shape, Vec2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::references::{Kind, Reference, View};

/// L'encre d'origine : noir.
pub(crate) const INK_DEFAULT: [u8; 3] = [0, 0, 0];
/// Les encres proposées d'un clic (la dernière est un gris).
pub(crate) const INKS: [[u8; 3]; 8] = [[0, 0, 0], [214, 40, 40], [214, 102, 10], [28, 150, 72], [30, 110, 220], [124, 70, 200], [236, 72, 160], [120, 124, 132]];
/// Le nom de chaque encre (bulles).
pub(crate) const INK_NAMES: [&str; 8] = ["Noir", "Rouge", "Orange", "Vert", "Bleu", "Violet", "Rose", "Gris"];
/// Épaisseur d'origine du crayon, et ses bornes, en unités du tableau (à pression pleine).
pub(crate) const WIDTH_DEFAULT: f32 = 4.0;
pub(crate) const WIDTH_MIN: f32 = 0.5;
pub(crate) const WIDTH_MAX: f32 = 120.0;
/// Marge, en unités du tableau, dont on peut manquer un trait fin en cliquant dessus (la planche la remplace par 6 points d'écran).
pub(crate) const HIT_SLACK: f32 = 4.0;
/// Nombre de points d'un trait au plus (au-delà, le trait est éclairci en douceur).
pub(crate) const POINTS_MAX: usize = 6000;
/// Un point plus proche que ça du précédent, à l'écran, n'est pas peint.
const PAINT_STEP: f32 = 0.75;
/// Le blanc du canevas : celui de la planche dans tous les thèmes, en fenêtre à part et en plein écran.
pub(crate) const CANVAS: Color32 = Color32::WHITE;
/// Le bleu du contour de ce qui est choisi, et des poignées, sur le blanc du canevas (le contour du thème ne s'y verrait pas dans les thèmes sombres).
pub(crate) const SELECTION: Color32 = Color32::from_rgb(28, 112, 224);
/// Le texte de ce qui est posé sur le canevas (aide, légendes) : un gris sombre lisible sur le blanc.
pub(crate) const CANVAS_TEXT: Color32 = Color32::from_rgb(44, 46, 52);
/// Un texte plus discret, sur le blanc.
pub(crate) const CANVAS_TEXT_SOFT: Color32 = Color32::from_rgb(112, 116, 126);

// ---------------------------------------------------------------------- le fichier

pub(crate) fn points_empty(p: &Arc<Vec<[f32; 3]>>) -> bool {
    p.is_empty()
}

pub(crate) fn ink_is_default(ink: &[u8; 3]) -> bool {
    *ink == INK_DEFAULT
}

pub(crate) fn width_is_zero(w: &f32) -> bool {
    *w == 0.0
}

/// Les points partagés (`Arc`) s'écrivent en une seule chaîne : `"x y p x y p …"`, `x` et `y` en centièmes d'unité, `p` (la
/// pression) en centièmes, des entiers. Court et rapide à écrire (des centaines de traits ne pèsent pas sur l'enregistrement),
/// et sans la fonction `rc` de serde. À la lecture, un point illisible arrête la liste (le début du trait est gardé).
pub(crate) mod points_serde {
    use super::*;
    use std::fmt::Write;

    pub fn encode(points: &[[f32; 3]]) -> String {
        let mut s = String::with_capacity(points.len() * 14);
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            let _ = write!(s, "{} {} {}", (p[0] * 100.0).round() as i32, (p[1] * 100.0).round() as i32, (p[2] * 100.0).round() as i32);
        }
        s
    }

    pub fn decode(text: &str) -> Vec<[f32; 3]> {
        let mut out = Vec::new();
        let mut tokens = text.split_ascii_whitespace();
        while let (Some(x), Some(y), Some(p)) = (tokens.next(), tokens.next(), tokens.next()) {
            let (Ok(x), Ok(y), Ok(p)) = (x.parse::<i32>(), y.parse::<i32>(), p.parse::<i32>()) else { break };
            out.push([x as f32 / 100.0, y as f32 / 100.0, p as f32 / 100.0]);
            if out.len() >= 4 * POINTS_MAX {
                break;
            }
        }
        out
    }

    pub fn serialize<S: Serializer>(v: &Arc<Vec<[f32; 3]>>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<Vec<[f32; 3]>>, D::Error> {
        String::deserialize(d).map(|t| Arc::new(decode(&t)))
    }
}

/// Un élément lu qui n'est pas un trait n'a ni points, ni encre, ni épaisseur ; un trait lu est ramené à des valeurs sûres
/// (un fichier édité à la main ne casse rien) et reprend la taille que ses points demandent.
pub(crate) fn sanitize_stroke(r: &mut Reference) {
    if r.kind != Kind::Stroke {
        if !r.points.is_empty() {
            r.points = Arc::default();
        }
        r.ink = INK_DEFAULT;
        r.width = 0.0;
        return;
    }
    r.width = if r.width.is_finite() && r.width > 0.0 { r.width.clamp(WIDTH_MIN, WIDTH_MAX) } else { WIDTH_DEFAULT };
    let clean = |v: f32| if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.0 };
    let tidy = r.points.iter().any(|p| !p.iter().all(|v| v.is_finite()) || p[2] < 0.0 || p[2] > 1.0) || r.points.len() > POINTS_MAX;
    if tidy {
        let mut pts: Vec<[f32; 3]> = r.points.iter().filter(|p| p[0].is_finite() && p[1].is_finite()).map(|p| [clean(p[0]), clean(p[1]), if p[2].is_finite() { p[2].clamp(0.0, 1.0) } else { 1.0 }]).collect();
        if pts.len() > POINTS_MAX {
            pts = thin(&pts, POINTS_MAX);
        }
        r.points = Arc::new(pts);
    }
    r.size = stroke_size(&r.points, r.width);
}

/// Au plus `max` points, répartis sur tout le trait (le premier et le dernier restent).
fn thin(points: &[[f32; 3]], max: usize) -> Vec<[f32; 3]> {
    if points.len() <= max || max < 2 {
        return points.to_vec();
    }
    let last = points.len() - 1;
    (0..max).map(|i| points[i * last / (max - 1)]).collect()
}

/// La taille du bloc d'un trait : de quoi contenir ses points de part et d'autre de son centre, épaisseur comprise.
fn stroke_size(points: &[[f32; 3]], width: f32) -> [u32; 2] {
    let (mut mx, mut my) = (0.0_f32, 0.0_f32);
    for p in points {
        mx = mx.max(p[0].abs());
        my = my.max(p[1].abs());
    }
    [(2.0 * mx + width).ceil().clamp(8.0, 65_536.0) as u32, (2.0 * my + width).ceil().clamp(8.0, 65_536.0) as u32]
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------- un trait

/// L'épaisseur relative d'un point selon sa pression : le fin du stylet est un cinquième de l'épaisseur pleine.
pub(crate) fn pressure_factor(pressure: f32) -> f32 {
    0.2 + 0.8 * pressure.clamp(0.0, 1.0)
}

impl Reference {
    /// Un trait de crayon neuf, de numéro `id`, fait des échantillons `(point du tableau, pression)` : il est éclairci (aucun point à
    /// moins de `min_step` du précédent), centré sur ce qu'il couvre, et prend l'encre et l'épaisseur données. Rien s'il n'y a
    /// aucun échantillon.
    pub(crate) fn stroke(id: u64, samples: &[(Pos2, f32)], ink: [u8; 3], width: f32, min_step: f32) -> Option<Reference> {
        let first = samples.first()?;
        let mut kept: Vec<(Pos2, f32)> = vec![*first];
        for s in &samples[1..] {
            if kept.last().is_some_and(|k| (k.0 - s.0).length() >= min_step) {
                kept.push(*s);
            }
        }
        // (Le dernier point est toujours là : le trait finit où la main a fini.)
        if let (Some(last), true) = (samples.last(), kept.len() > 1) {
            let n = kept.len();
            kept[n - 1] = *last;
        }
        let width = width.clamp(WIDTH_MIN, WIDTH_MAX);
        let mut area = Rect::from_min_max(kept[0].0, kept[0].0);
        for (p, _) in &kept {
            area.extend_with(*p);
        }
        let center = area.center();
        let points: Vec<[f32; 3]> = kept.iter().map(|(p, pr)| [round2(p.x - center.x), round2(p.y - center.y), round2(pr.clamp(0.0, 1.0))]).collect();
        let points = if points.len() > POINTS_MAX { thin(&points, POINTS_MAX) } else { points };
        let mut r = Reference::image(id, "", "", [8, 8], Pos2::new(round2(center.x), round2(center.y)));
        r.kind = Kind::Stroke;
        r.scale = 1.0;
        r.ink = ink;
        r.width = width;
        r.size = stroke_size(&points, width);
        r.points = Arc::new(points);
        Some(r)
    }

    /// Le trait est-il sous le point `p` du tableau, à `slack` unités près ? Seul le tracé compte (pas le rectangle autour).
    pub(crate) fn stroke_hit(&self, p: Pos2, slack: f32) -> bool {
        let (h, l) = (self.half(), self.to_local(p));
        if l.x.abs() > h.x + slack || l.y.abs() > h.y + slack {
            return false;
        }
        let s = self.scale.max(1e-4);
        let (q, tol) = (l / s, slack / s);
        let reach = self.width * 0.5 + tol;
        let pts = &self.points;
        match pts.len() {
            0 => false,
            1 => (vec2(pts[0][0], pts[0][1]) - q).length() <= reach,
            _ => pts.windows(2).any(|w| {
                let (a, b) = (vec2(w[0][0], w[0][1]), vec2(w[1][0], w[1][1]));
                distance_to_segment(q, a, b) <= reach
            }),
        }
    }

    /// L'encre du trait, en couleur d'écran.
    pub(crate) fn ink_color(&self, alpha: f32) -> Color32 {
        Color32::from_rgba_unmultiplied(self.ink[0], self.ink[1], self.ink[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= 1e-9 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

// ---------------------------------------------------------------------- le dessin

/// Peint un trait de crayon : `pts` sont les points d'écran et la demi-épaisseur de chacun (en points d'écran).
/// Un ruban de triangles (coins limités), un rond à chaque bout ; un trait réduit à un point est un rond. Aucun
/// travail par image au-delà de ce qui se voit : les points plus proches que `PAINT_STEP` sont sautés.
pub(crate) fn paint_polyline(painter: &Painter, pts: &[(Pos2, f32)], color: Color32) {
    let Some(first) = pts.first() else { return };
    let mut kept: Vec<(Pos2, f32)> = Vec::with_capacity(pts.len().min(512));
    kept.push(*first);
    for (i, p) in pts.iter().enumerate().skip(1) {
        let last = i + 1 == pts.len();
        if last || (p.0 - kept[kept.len() - 1].0).length() >= PAINT_STEP {
            kept.push(*p);
        }
    }
    if kept.len() == 1 || (kept.len() == 2 && (kept[0].0 - kept[1].0).length() < 0.05) {
        painter.add(Shape::circle_filled(kept[0].0, kept[0].1, color));
        return;
    }
    let dirs: Vec<Vec2> = kept
        .windows(2)
        .map(|w| {
            let d = (w[1].0 - w[0].0).normalized();
            if d.is_finite() {
                d
            } else {
                vec2(1.0, 0.0)
            }
        })
        .collect();
    let mut mesh = Mesh::default();
    mesh.vertices.reserve(kept.len() * 2);
    mesh.indices.reserve(kept.len() * 6);
    for (i, (p, hw)) in kept.iter().enumerate() {
        let d_prev = dirs[i.saturating_sub(1).min(dirs.len() - 1)];
        let d_next = dirs[i.min(dirs.len() - 1)];
        let t = (d_prev + d_next).normalized();
        let t = if t.is_finite() { t } else { d_next };
        let normal = vec2(-t.y, t.x);
        // Coin : le décalage grandit quand le trait tourne, jusqu'au double.
        let n_prev = vec2(-d_prev.y, d_prev.x);
        let miter = 1.0 / normal.dot(n_prev).abs().max(0.5);
        let off = normal * (*hw * miter);
        mesh.vertices.push(Vertex { pos: *p + off, uv: egui::epaint::WHITE_UV, color });
        mesh.vertices.push(Vertex { pos: *p - off, uv: egui::epaint::WHITE_UV, color });
        if i > 0 {
            let k = (i as u32 - 1) * 2;
            mesh.indices.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    painter.add(Shape::mesh(mesh));
    let (a, b) = (kept[0], kept[kept.len() - 1]);
    painter.add(Shape::circle_filled(a.0, a.1, color));
    painter.add(Shape::circle_filled(b.0, b.1, color));
}

/// Peint le trait `r` sur la planche : `view` et `rect` donnent le passage du tableau à l'écran.
pub(crate) fn paint_stroke(painter: &Painter, r: &Reference, view: &View, rect: Rect, alpha: f32) {
    let (s, c) = r.rotation.to_radians().sin_cos();
    let center = r.center();
    let zoom = view.zoom;
    let to_screen = |p: &[f32; 3]| {
        let v = vec2(p[0], p[1]) * r.scale;
        let board = center + vec2(v.x * c - v.y * s, v.x * s + v.y * c);
        let hw = (r.width * r.scale * pressure_factor(p[2]) * zoom * 0.5).max(0.6);
        (view.to_screen(rect, board), hw)
    };
    let pts: Vec<(Pos2, f32)> = r.points.iter().map(to_screen).collect();
    paint_polyline(painter, &pts, r.ink_color(alpha));
}

/// Peint le trait en cours (pas encore dans la planche) : `samples` sont des points du tableau avec leur pression.
pub(crate) fn paint_live(painter: &Painter, samples: &[(Pos2, f32)], ink: [u8; 3], width: f32, view: &View, rect: Rect, alpha: f32) {
    let pts: Vec<(Pos2, f32)> = samples.iter().map(|(p, pr)| (view.to_screen(rect, *p), (width * pressure_factor(*pr) * view.zoom * 0.5).max(0.6))).collect();
    paint_polyline(painter, &pts, Color32::from_rgba_unmultiplied(ink[0], ink[1], ink[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8));
}

/// L'encre d'un texte posé sur le blanc du canevas : celle du thème est claire dans les thèmes sombres, et la teinte d'un
/// post-it (jaune, rose…) serait illisible sur du blanc. La teinte est assombrie, pas remplacée.
pub(crate) fn ink_on_canvas(tint: [u8; 3]) -> [u8; 3] {
    tint.map(|v| (v as f32 * 0.42).round() as u8)
}
