//! Rôles des images (13g-2), ceux de la feuille d'exposition de TVPaint 12 : clé, breakdown,
//! intervalle (à faire / complété), intervalle spécial (à faire / complété). Choix d'Anthony
//! (2026-10-15) : un rôle se pose sur une case (un calque, une image), même vide, et ne change
//! que si on le change (pas de « complété » automatique quand on dessine).
//!
//! Les rôles d'un calque sont rangés par image (`Layer::roles`) ; ils suivent les images quand on
//! en insère, en supprime, en inverse ou en déplace. Chacun a son symbole (propre à Pelure),
//! dans la timeline et la feuille d'exposition.

use std::collections::BTreeMap;

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Stroke};
use serde::{Deserialize, Serialize};

/// Le rôle d'une image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "cle")]
    Key,
    #[serde(rename = "breakdown")]
    Breakdown,
    #[serde(rename = "intervalle")]
    Inbetween,
    #[serde(rename = "intervalle_complete")]
    InbetweenDone,
    #[serde(rename = "intervalle_special")]
    Special,
    #[serde(rename = "intervalle_special_complete")]
    SpecialDone,
}

/// Les rôles d'un calque : image → rôle.
pub type Roles = BTreeMap<usize, Role>;

impl Role {
    pub const ALL: [Role; 6] = [Role::Key, Role::Breakdown, Role::Inbetween, Role::InbetweenDone, Role::Special, Role::SpecialDone];

    pub fn label(self) -> &'static str {
        match self {
            Role::Key => "Clé",
            Role::Breakdown => "Breakdown",
            Role::Inbetween => "Intervalle à faire",
            Role::InbetweenDone => "Intervalle complété",
            Role::Special => "Intervalle spécial à faire",
            Role::SpecialDone => "Intervalle spécial complété",
        }
    }

    /// Identifiant de l'action du clavier qui pose ce rôle.
    pub fn action(self) -> &'static str {
        match self {
            Role::Key => "role.cle",
            Role::Breakdown => "role.breakdown",
            Role::Inbetween => "role.intervalle",
            Role::InbetweenDone => "role.intervalle_complete",
            Role::Special => "role.intervalle_special",
            Role::SpecialDone => "role.intervalle_special_complete",
        }
    }

    fn color(self) -> Color32 {
        match self {
            // Les couleurs du thème : rouge, orange, bleu et violet restent lisibles sur les rangées.
            Role::Key => crate::theme::image_courante(),
            Role::Breakdown => crate::theme::marque(),
            Role::Inbetween | Role::InbetweenDone => crate::theme::dessin_vectoriel(),
            Role::Special | Role::SpecialDone => crate::theme::effet(),
        }
    }
}

/// `n` images insérées à l'image `at` (`removed` : retirées de `at` à `at + n`) : les rôles
/// d'après suivent ; ceux des images retirées partent.
pub fn shift(roles: &mut Roles, at: usize, n: usize, removed: bool) {
    if n == 0 {
        return;
    }
    let old = std::mem::take(roles);
    for (f, r) in old {
        let to = if f < at {
            Some(f)
        } else if !removed {
            Some(f + n)
        } else if f < at + n {
            None
        } else {
            Some(f - n)
        };
        if let Some(to) = to {
            roles.insert(to, r);
        }
    }
}

/// Les images de `a` à `b` (exclue) sont inversées : leurs rôles aussi.
pub fn reverse(roles: &mut Roles, a: usize, b: usize) {
    let inside: Vec<(usize, Role)> = roles.range(a..b).map(|(f, r)| (*f, *r)).collect();
    for (f, _) in &inside {
        roles.remove(f);
    }
    for (f, r) in inside {
        roles.insert(a + b - 1 - f, r);
    }
}

/// Les rôles des images de `a` à `b` (exclue), à partir de 0.
pub fn take(roles: &Roles, a: usize, b: usize) -> Vec<(usize, Role)> {
    roles.range(a..b).map(|(f, r)| (f - a, *r)).collect()
}

/// Pose `row` (rôles à partir de 0) à partir de l'image `to`, après avoir vidé les `len` images
/// de `to` à `to + len`.
pub fn put(roles: &mut Roles, to: usize, len: usize, row: &[(usize, Role)]) {
    let gone: Vec<usize> = roles.range(to..to + len).map(|(f, _)| *f).collect();
    for f in gone {
        roles.remove(&f);
    }
    for (k, r) in row {
        roles.insert(to + k, *r);
    }
}

/// Les rôles d'un plan coupé à l'image `at` : ceux d'après partent dans le second, ramenés à 0.
pub fn split_off(roles: &mut Roles, at: usize) -> Roles {
    let tail = roles.split_off(&at);
    tail.into_iter().map(|(f, r)| (f - at, r)).collect()
}

/// Dessine le symbole d'un rôle, centré sur `c`, de taille `s` (en points), à l'opacité
/// `alpha` (0 à 1). Clé : disque ; breakdown : triangle ; intervalle : cercle (plein une fois
/// complété) ; intervalle spécial : carré (plein une fois complété).
pub fn paint_symbol(painter: &egui::Painter, c: Pos2, s: f32, role: Role, alpha: f32) {
    let color = role.color().gamma_multiply(alpha.clamp(0.0, 1.0));
    let r = s / 2.0;
    let stroke = Stroke::new(1.5_f32, color);
    match role {
        Role::Key => {
            painter.circle_filled(c, r, color);
        }
        Role::Breakdown => {
            let pts = vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y + r * 0.8), pos2(c.x - r, c.y + r * 0.8)];
            painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        Role::Inbetween => {
            painter.circle_stroke(c, r - 0.75, stroke);
        }
        Role::InbetweenDone => {
            painter.circle_stroke(c, r - 0.75, stroke);
            painter.circle_filled(c, r * 0.45, color);
        }
        Role::Special => {
            painter.rect_stroke(egui::Rect::from_center_size(c, vec2(s - 1.5, s - 1.5)), 0.0, stroke, egui::StrokeKind::Middle);
        }
        Role::SpecialDone => {
            painter.rect_filled(egui::Rect::from_center_size(c, vec2(s - 1.0, s - 1.0)), 0.0, color);
        }
    }
}
