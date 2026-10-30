//! Deux options du pinceau et du crayon (Anthony, 2026-10-17 : « pour le pinceau j'ai besoin de
//! deux options : peint derrière et remplir la forme. Remplir la forme remplit par la couleur une
//! forme fermée ») :
//! - Peint derrière : la peinture passe sous ce qui est déjà dessiné sur le calque (`paint_under`).
//! - Remplir la forme : au lâcher, un trait fermé remplit son intérieur de sa couleur. Fermé :
//!   son bout revient près de son début, ou il se recoupe (une boucle qui dépasse) ; c'est alors
//!   la boucle qui se remplit (`closed_outline`). Le remplissage est sous le trait et par-dessus
//!   ce qui était dessiné avant (`fill_under_stroke`) ; avec « peint derrière », sous tout.
//!
//! 2026-10-23 (Anthony) : « Peint derrière » s'appelle **« Peint sur l'aplat »** (même effet pour l'instant, la clé
//! `peint_derriere` des réglages et l'action `pinceau.derriere` gardent leur nom) ; « Remplir la forme » n'est plus
//! proposé par l'interface (il faisait double emploi avec l'aplat sous le trait, `fond.rs`) : l'ancienne clé
//! `remplir_la_forme` se lit sans effet, l'action `pinceau.remplir` est un double de `pinceau.fond`. Le moteur garde
//! le remplissage (`closed_outline`, `fill_under_stroke`), testé, que l'application n'allume plus.

use eframe::egui::{Color32, Pos2};

/// Le contour à remplir d'un trait de ligne médiane `path` : tout le trait si son bout revient à
/// moins de `tol` de son début ; sinon la plus grande boucle qu'il fait en se recoupant (de la
/// fin vers le début) ; aucun si le trait reste ouvert.
pub fn closed_outline(path: &[Pos2], tol: f32) -> Option<Vec<Pos2>> {
    if path.len() < 3 {
        return None;
    }
    let length: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    let (first, last) = (path[0], path[path.len() - 1]);
    if first.distance(last) <= tol && length > 3.0 * tol.max(1.0) {
        return Some(path.to_vec());
    }
    // Le dernier segment qui en recoupe un plus ancien (pas son voisin) ; le plus ancien possible,
    // pour la plus grande boucle.
    let n = path.len() - 1;
    for j in (2..n).rev() {
        for i in 0..j - 1 {
            if let Some(x) = crossing(path[i], path[i + 1], path[j], path[j + 1]) {
                let mut loop_pts = Vec::with_capacity(j - i + 1);
                loop_pts.push(x);
                loop_pts.extend_from_slice(&path[i + 1..=j]);
                return (loop_pts.len() >= 3).then_some(loop_pts);
            }
        }
    }
    None
}

/// Point où les segments [a, b] et [c, d] se coupent, s'ils se coupent.
fn crossing(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<Pos2> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let q = c - a;
    let t = (q.x * s.y - q.y * s.x) / den;
    let u = (q.x * r.y - q.y * r.x) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a + r * t)
}

/// `top` posé sur `bottom` (couleurs prémultipliées).
pub fn over_premul(top: Color32, bottom: Color32) -> Color32 {
    let k = 1.0 - top.a() as f32 / 255.0;
    let ch = |t: u8, b: u8| (t as f32 + b as f32 * k + 0.5).min(255.0) as u8;
    Color32::from_rgba_premultiplied(ch(top.r(), bottom.r()), ch(top.g(), bottom.g()), ch(top.b(), bottom.b()), ch(top.a(), bottom.a()))
}

/// Peint derrière : la couleur `rgb` de couverture `a` (0 à 1) passe sous le pixel `b`.
pub fn paint_under(b: Color32, rgb: [u8; 3], a: f32) -> Color32 {
    over_premul(b, solid(rgb, a))
}

/// Le pixel `after` (le trait de couverture `stroke_a` posé sur `before`) avec le remplissage
/// `fill` glissé entre les deux : trait sur remplissage sur dessin d'avant. En prémultiplié,
/// après = trait + (1 − couverture) × avant ; on remplace « avant » par « remplissage sur avant ».
pub fn fill_under_stroke(after: Color32, before: Color32, stroke_a: f32, fill: Color32) -> Color32 {
    let k = 1.0 - stroke_a.clamp(0.0, 1.0);
    let filled = over_premul(fill, before);
    let ch = |a: u8, f: u8, b: u8| (a as f32 + k * (f as f32 - b as f32)).round().clamp(0.0, 255.0);
    let alpha = ch(after.a(), filled.a(), before.a());
    let c = |a: u8, f: u8, b: u8| ch(a, f, b).min(alpha) as u8;
    Color32::from_rgba_premultiplied(c(after.r(), filled.r(), before.r()), c(after.g(), filled.g(), before.g()), c(after.b(), filled.b(), before.b()), alpha as u8)
}

/// La couleur `rgb` de couverture `a` (0 à 1), prémultipliée.
pub fn solid(rgb: [u8; 3], a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let ch = |c: u8| (c as f32 * a + 0.5) as u8;
    Color32::from_rgba_premultiplied(ch(rgb[0]), ch(rgb[1]), ch(rgb[2]), (255.0 * a + 0.5) as u8)
}
