//! Cadres tracés par-dessus l'image (9d) : zones de sécurité et cadre d'un format de diffusion,
//! dans le cadre de la caméra. Ils servent à composer ; rien n'en passe dans le film.

use eframe::egui::{pos2, Rect};

/// Formats proposés pour le cadre réglable, largeur:hauteur (« Personnalisé » : n'importe lequel).
/// Le 2,39:1 est celui que pose l'objectif anamorphique de la caméra (`camera::ANAMORPHIC_FORMAT`).
pub const PRESETS: [[f32; 2]; 7] = [[16.0, 9.0], [9.0, 16.0], [4.0, 5.0], [1.0, 1.0], [4.0, 3.0], [1.85, 1.0], crate::camera::ANAMORPHIC_FORMAT];

/// Zones de sécurité, en part de la largeur et de la hauteur du cadre : action dans 93 %,
/// titres et textes dans 90 % (valeurs de la recommandation EBU R 95).
pub const ACTION_SAFE: f32 = 0.93;
pub const TITLE_SAFE: f32 = 0.90;

/// Format utilisable (fini, positif), ramené dans des bornes raisonnables ; sinon `None`.
pub fn sanitized(ratio: [f32; 2]) -> Option<[f32; 2]> {
    let ok = |v: f32| v.is_finite() && v > 0.0;
    (ok(ratio[0]) && ok(ratio[1])).then(|| ratio.map(|v| v.clamp(0.01, 1000.0)))
}

/// Le format est l'un des formats proposés.
pub fn is_preset(ratio: [f32; 2]) -> bool {
    PRESETS.contains(&ratio)
}

/// « 16:9 », ou « 2.39:1 » pour un rapport qui n'est pas entier.
pub fn label(ratio: [f32; 2]) -> String {
    let n = |v: f32| {
        if (v - v.round()).abs() < 1e-3 {
            format!("{}", v.round())
        } else {
            format!("{v:.2}")
        }
    };
    format!("{}:{}", n(ratio[0]), n(ratio[1]))
}

/// Le plus grand rectangle du format `ratio`, centré dans le cadre `frame` (en pixels du cadre).
pub fn format_rect(frame: [usize; 2], ratio: [f32; 2]) -> Rect {
    let (w, h) = (frame[0] as f32, frame[1] as f32);
    let r = ratio[0] / ratio[1];
    let (fw, fh) = if w / h > r { (h * r, h) } else { (w, w / r) };
    Rect::from_center_size(pos2(w / 2.0, h / 2.0), eframe::egui::vec2(fw, fh))
}

/// `inside` réduit à `part` de sa largeur et de sa hauteur, autour de son centre.
pub fn safe_rect(inside: Rect, part: f32) -> Rect {
    Rect::from_center_size(inside.center(), inside.size() * part)
}
