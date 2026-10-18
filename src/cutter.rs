//! Outil Cutter (Anthony, 2026-10-17 : « ajoute l'outil cutter qui coupe les lignes vectorielles
//! qui se croisent ») ; « Cutter Tool » de Storyboard Pro (Alt+T), dont le comportement exact est
//! à vérifier.
//!
//! Sur un calque vectoriel, on glisse en travers des bouts de traits à enlever (ou on clique sur
//! l'un d'eux) : chaque trait touché est coupé là où il croise les autres traits (et lui-même),
//! et seuls les morceaux que le geste touche disparaissent. Un trait qui ne croise rien part en
//! entier. Les zones remplies et les textes ne bougent pas.

use eframe::egui::{Pos2, Rect};

use crate::app::App;
use crate::document::LayerKind;
use crate::vector::{Element, VPoint, VectorDrawing};

/// Les morceaux plus courts (pixels du dessin) sont laissés de côté.
const MIN_PIECE: f32 = 0.5;

/// Croisement des segments `a`–`b` et `c`–`d` : fractions le long de chacun (0 à 1).
fn seg_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<(f32, f32)> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let q = c - a;
    let t = (q.x * s.y - q.y * s.x) / den;
    let u = (q.x * r.y - q.y * r.x) / den;
    const E: f32 = 1e-4;
    ((-E..=1.0 + E).contains(&t) && (-E..=1.0 + E).contains(&u)).then_some((t.clamp(0.0, 1.0), u.clamp(0.0, 1.0)))
}

/// Endroits (indice du segment + fraction) où la ligne `p` croise la ligne `q` ; `same` : c'est
/// la même ligne (ses segments voisins ne comptent pas).
pub fn crossings(p: &[Pos2], q: &[Pos2], same: bool) -> Vec<f32> {
    let mut out = Vec::new();
    for i in 0..p.len().saturating_sub(1) {
        let (a, b) = (p[i], p[i + 1]);
        let ab = Rect::from_two_pos(a, b);
        for j in 0..q.len().saturating_sub(1) {
            if same && j + 1 >= i && j <= i + 1 {
                continue;
            }
            let (c, d) = (q[j], q[j + 1]);
            if !ab.intersects(Rect::from_two_pos(c, d)) {
                continue;
            }
            if let Some((t, _)) = seg_cross(a, b, c, d) {
                out.push(i as f32 + t);
            }
        }
    }
    out
}

/// Point de la ligne à l'endroit `at` (indice du segment + fraction).
fn point_at(points: &[VPoint], at: f32) -> VPoint {
    let i = (at.floor().max(0.0) as usize).min(points.len().saturating_sub(2));
    let t = (at - i as f32).clamp(0.0, 1.0);
    let (a, b) = (points[i], points[(i + 1).min(points.len() - 1)]);
    VPoint { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, r: a.r + (b.r - a.r) * t }
}

/// Endroit de la ligne le plus proche de `c`, et l'écart au bord du trait (négatif : dedans).
fn nearest(points: &[VPoint], c: Pos2) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for i in 0..points.len().saturating_sub(1) {
        let (a, b) = (points[i].pos(), points[i + 1].pos());
        let ab = b - a;
        let len2 = ab.length_sq();
        let t = if len2 > 0.0 { ((c - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let at = i as f32 + t;
        let gap = (a + ab * t).distance(c) - point_at(points, at).r;
        if best.is_none_or(|(_, g)| gap < g) {
            best = Some((at, gap));
        }
    }
    best
}

/// Le morceau de la ligne entre `from` et `to`.
fn piece(points: &[VPoint], from: f32, to: f32) -> Vec<VPoint> {
    let mut out = vec![point_at(points, from)];
    let first = from.floor() as usize + 1;
    for k in first..points.len() {
        if k as f32 >= to {
            break;
        }
        out.push(points[k]);
    }
    out.push(point_at(points, to));
    out
}

fn length(points: &[VPoint]) -> f32 {
    points.windows(2).map(|w| w[0].pos().distance(w[1].pos())).sum()
}

/// Coupe les traits que touche le geste `gesture` (points du dessin) à leurs croisements, et
/// retire les morceaux touchés. Un geste sans longueur (un clic) touche le trait le plus proche à
/// `tol` près. Rend la zone changée, ou rien si aucun trait n'a été touché.
pub fn cut_strokes(d: &mut VectorDrawing, gesture: &[Pos2], tol: f32) -> Option<Rect> {
    let click = gesture.len() < 2 || gesture.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>() <= tol;
    let lines: Vec<Option<Vec<Pos2>>> = d
        .elements
        .iter()
        .map(|e| match e {
            Element::Stroke { points, .. } if points.len() >= 2 => Some(points.iter().map(|p| p.pos()).collect()),
            _ => None,
        })
        .collect();
    let reach = Rect::from_points(gesture).expand(tol);
    // Un clic : le seul trait le plus proche (le plus haut à égalité), s'il est à portée.
    let clicked: Option<(usize, f32)> = if click {
        let c = gesture.first().copied()?;
        let mut best: Option<(usize, f32, f32)> = None;
        for (i, e) in d.elements.iter().enumerate() {
            if let Element::Stroke { points, .. } = e {
                if let Some((at, gap)) = nearest(points, c) {
                    if gap <= tol && best.is_none_or(|(_, _, g)| gap <= g) {
                        best = Some((i, at, gap));
                    }
                }
            }
        }
        best.map(|(i, at, _)| (i, at))
    } else {
        None
    };
    let mut region = Rect::NOTHING;
    let mut out = Vec::with_capacity(d.elements.len());
    for (i, e) in d.elements.iter().enumerate() {
        let (Some(line), Element::Stroke { points, color, opacity, hardness, brush }) = (&lines[i], e) else {
            out.push(e.clone());
            continue;
        };
        let bounds = e.bounds();
        let hits: Vec<f32> = if click {
            clicked.filter(|(k, _)| *k == i).map(|(_, at)| vec![at]).unwrap_or_default()
        } else if bounds.intersects(reach) {
            crossings(line, gesture, false)
        } else {
            Vec::new()
        };
        if hits.is_empty() {
            out.push(e.clone());
            continue;
        }
        // Où ce trait croise les autres, et lui-même.
        let mut cuts: Vec<f32> = crossings(line, line, true);
        for (j, other) in lines.iter().enumerate() {
            if let Some(other) = other.as_ref().filter(|_| j != i && d.elements[j].bounds().intersects(bounds)) {
                cuts.extend(crossings(line, other, false));
            }
        }
        let end = (points.len() - 1) as f32;
        cuts.retain(|&c| c > 1e-3 && c < end - 1e-3);
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        let mut bounds_list = vec![0.0];
        bounds_list.extend(cuts);
        bounds_list.push(end);
        for w in bounds_list.windows(2) {
            let (from, to) = (w[0], w[1]);
            if hits.iter().any(|&h| h >= from && h <= to) {
                continue;
            }
            let kept = piece(points, from, to);
            if kept.len() >= 2 && length(&kept) >= MIN_PIECE {
                out.push(Element::Stroke { points: kept, color: *color, opacity: *opacity, hardness: *hardness, brush: brush.clone() });
            }
        }
        region = region.union(bounds);
    }
    if region == Rect::NOTHING {
        return None;
    }
    d.elements = out;
    Some(region)
}

impl App {
    /// Appui avec le cutter : le geste commence (calque vectoriel seulement).
    pub(crate) fn cutter_press(&mut self, p: Pos2) {
        if !self.layer_editable() {
            return;
        }
        // Strate Fond (`fondui.rs`) : un clic retire du fond la zone fermée par le trait, sur un
        // calque bitmap comme vectoriel.
        if self.paint_fond && self.layer().fond.is_some() {
            self.fond_zone_at(p, true);
            return;
        }
        if self.layer().kind != LayerKind::Vector {
            self.status = "Le cutter coupe les traits d'un calque vectoriel.".into();
            return;
        }
        self.end_stroke();
        self.playing = false;
        self.cut_gesture = Some(vec![p]);
    }

    /// Le geste du cutter suit le pointeur ; faux s'il n'y a pas de geste en cours.
    pub(crate) fn cutter_drag(&mut self, p: Pos2) -> bool {
        let zoom = self.layer_zoom();
        let Some(g) = &mut self.cut_gesture else { return false };
        if g.last().is_none_or(|q| q.distance(p) * zoom > 1.5) {
            g.push(p);
        }
        true
    }

    /// Au relâché : on coupe (une étape d'annulation).
    pub(crate) fn finish_cut(&mut self) {
        let Some(gesture) = self.cut_gesture.take() else { return };
        let Some(cel) = self.cel_here() else { return };
        let tol = 4.0 / self.layer_zoom();
        let Some(d) = self.drawing_mut(cel) else { return };
        let before = d.clone();
        match cut_strokes(d, &gesture, tol) {
            Some(region) => {
                // Les indices des éléments ont changé.
                self.vsel = None;
                self.refresh_cel_region(cel, region);
                self.push_vector(cel, before, None);
                self.status = "Traits coupés.".into();
            }
            None => self.status = "Rien à couper : glisse en travers d'un bout de trait.".into(),
        }
    }
}
