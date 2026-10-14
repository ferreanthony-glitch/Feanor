//! Pelure d'oignon (3d-1) : les dessins voisins du calque courant, sous le dessin courant, de plus
//! en plus pâles avec la distance. Valeurs par défaut reprises des préférences Storyboard Pro
//! d'Anthony (`docs/reference/sbpro_presets/STB_Preferences.xml`) : 3 avant, 3 après, rouge avant,
//! vert après, teinté, sous le dessin. Réglages de l'application, retenus d'une séance à l'autre.

use serde::{Deserialize, Serialize};

use crate::document::{CelId, Layer};

/// Ce que compte « 3 avant » : 3 dessins différents, ou 3 images de la timeline.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OnionMode {
    /// Les dessins différents les plus proches, quelle que soit leur durée d'exposition.
    #[serde(rename = "dessins")]
    Drawings,
    /// Les images voisines dans la timeline : un dessin tenu longtemps peut n'en couvrir aucune
    /// autre ; un dessin vu à plusieurs de ces images n'est montré qu'une fois, au plus près.
    #[serde(rename = "images")]
    Frames,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnionSettings {
    pub enabled: bool,
    pub before: usize,
    pub after: usize,
    /// Opacité du voisin le plus proche (les suivants pâlissent).
    pub opacity: f32,
    pub mode: OnionMode,
    /// Teinter les voisins (sinon : leurs vraies couleurs, en transparence).
    pub tinted: bool,
    pub before_color: [u8; 3],
    pub after_color: [u8; 3],
}

impl Default for OnionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            before: 3,
            after: 3,
            opacity: 0.45,
            mode: OnionMode::Drawings,
            tinted: true,
            before_color: [255, 0, 0],
            after_color: [0, 255, 0],
        }
    }
}

/// Plus grand nombre de voisins de chaque côté.
pub const MAX_NEIGHBOURS: usize = 12;

/// Voisins du dessin de l'image `frame` : (dessin, rang), rang 0 = le plus proche. Le dessin
/// courant et les images vides ne sont jamais montrés.
pub fn neighbours(layer: &Layer, frame: usize, s: &OnionSettings) -> (Vec<(CelId, usize)>, Vec<(CelId, usize)>) {
    let current = layer.cel_at(frame);
    let len = layer.frames.len();
    let mut before: Vec<(CelId, usize)> = Vec::new();
    let mut after: Vec<(CelId, usize)> = Vec::new();
    let add = |list: &mut Vec<(CelId, usize)>, f: usize, rank: usize| {
        if let Some(id) = layer.cel_at(f) {
            if Some(id) != current && !list.iter().any(|&(c, _)| c == id) {
                list.push((id, rank));
            }
        }
    };
    match s.mode {
        OnionMode::Drawings => {
            let mut f = frame.min(len);
            while f > 0 && before.len() < s.before {
                f -= 1;
                let rank = before.len();
                add(&mut before, f, rank);
            }
            let mut f = frame + 1;
            while f < len && after.len() < s.after {
                let rank = after.len();
                add(&mut after, f, rank);
                f += 1;
            }
        }
        OnionMode::Frames => {
            for d in 1..=s.before.min(frame) {
                add(&mut before, frame - d, d - 1);
            }
            for d in 1..=s.after {
                if frame + d < len {
                    add(&mut after, frame + d, d - 1);
                }
            }
        }
    }
    (before, after)
}

/// Opacité (0 à 255) du voisin de rang `k` sur `n` : `opacity` au plus près, puis de moins en
/// moins, jusqu'à `opacity / n` pour le plus lointain.
pub fn fade(opacity: f32, k: usize, n: usize) -> u8 {
    let n = n.max(1);
    (opacity.clamp(0.0, 1.0) * (n - k.min(n - 1)) as f32 / n as f32 * 255.0).round() as u8
}
