//! Couleurs (passe couleur, demandée par Anthony le 2026-10-16), sans interface : conversions
//! entre RVB, TSV et TSL, code hexadécimal, et les réglages du panneau Couleur gardés dans
//! `%APPDATA%\Pelure\reglages.toml` (table `[couleurs]`). Les couleurs ne vont pas dans le
//! projet : son format ne change pas.

use serde::{Deserialize, Serialize};

use crate::roue::WheelShape;

pub const NOIR: [u8; 3] = [0, 0, 0];
pub const BLANC: [u8; 3] = [255, 255, 255];
/// Couleur d'avant-plan au premier lancement : l'encre de Pelure depuis 6b.
pub const ENCRE: [u8; 3] = [24, 24, 28];

/// Teinte (0 à 360), saturation et valeur (0 à 1) d'une couleur.
pub fn rgb_to_hsv(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    [h, if max <= 0.0 { 0.0 } else { d / max }, max]
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [u8; 3] {
    let h = hsv[0].rem_euclid(360.0) / 60.0;
    let (s, v) = (hsv[1].clamp(0.0, 1.0), hsv[2].clamp(0.0, 1.0));
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|t| ((t + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// TSV → TSL (teinte, saturation, luminosité ; 0 à 360, 0 à 1, 0 à 1). La teinte est gardée
/// telle quelle ; au noir et au blanc, la saturation TSL vaut 0.
pub fn hsv_to_hsl(hsv: [f32; 3]) -> [f32; 3] {
    let [h, s, v] = [hsv[0], hsv[1].clamp(0.0, 1.0), hsv[2].clamp(0.0, 1.0)];
    let l = v * (1.0 - s / 2.0);
    let m = l.min(1.0 - l);
    [h, if m <= 1e-6 { 0.0 } else { ((v - l) / m).clamp(0.0, 1.0) }, l]
}

/// TSL → TSV. La teinte est gardée telle quelle ; au noir, la saturation TSV vaut 0.
pub fn hsl_to_hsv(hsl: [f32; 3]) -> [f32; 3] {
    let [h, s, l] = [hsl[0], hsl[1].clamp(0.0, 1.0), hsl[2].clamp(0.0, 1.0)];
    let v = l + s * l.min(1.0 - l);
    [h, if v <= 1e-6 { 0.0 } else { (2.0 * (1.0 - l / v)).clamp(0.0, 1.0) }, v]
}

#[cfg(test)]
pub fn rgb_to_hsl(c: [u8; 3]) -> [f32; 3] {
    hsv_to_hsl(rgb_to_hsv(c))
}

pub fn hsl_to_rgb(hsl: [f32; 3]) -> [u8; 3] {
    hsv_to_rgb(hsl_to_hsv(hsl))
}

/// TSV de `c`, en gardant ce que `c` ne dit pas : la teinte d'avant pour un gris (saturation
/// nulle), la teinte et la saturation d'avant pour le noir. Ainsi passer par un gris ou par le
/// noir ne fait pas perdre la teinte qu'on réglait.
pub fn hsv_following(c: [u8; 3], before: [f32; 3]) -> [f32; 3] {
    let mut hsv = rgb_to_hsv(c);
    if hsv[2] <= 0.0 {
        hsv[0] = before[0];
        hsv[1] = before[1];
    } else if hsv[1] <= 0.0 {
        hsv[0] = before[0];
    }
    hsv
}

/// Couleurs récentes : la plus récente d'abord, sans doublon, `RECENT_MAX` au plus.
pub const RECENT_MAX: usize = 24;

pub fn remember(recent: &mut Vec<[u8; 3]>, c: [u8; 3]) {
    recent.retain(|r| *r != c);
    recent.insert(0, c);
    recent.truncate(RECENT_MAX);
}

/// Les curseurs du panneau Couleur.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SliderMode {
    /// Teinte, saturation, valeur.
    #[default]
    #[serde(rename = "tsv")]
    Hsv,
    /// Rouge, vert, bleu (0 à 255).
    #[serde(rename = "rvb")]
    Rgb,
    /// Teinte, saturation, luminosité.
    #[serde(rename = "tsl")]
    Hsl,
}

/// Code hexadécimal « #RRGGBB ».
pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Lit un code hexadécimal : « #RRGGBB », « RRGGBB », ou la forme courte « #RGB » / « RGB »
/// (chaque chiffre doublé, comme en CSS) ; majuscules ou minuscules, espaces autour permis.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]),
        3 => Some([(v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF].map(|d| (d * 17) as u8)),
        _ => None,
    }
}

/// Ce que le panneau Couleur retient d'une séance à l'autre (`[couleurs]` dans `reglages.toml`).
/// Valeurs simples d'abord, tables ensuite (ordre du TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorSettings {
    /// Couleur d'avant-plan : celle avec laquelle les outils dessinent.
    #[serde(rename = "avant_plan")]
    pub front: [u8; 3],
    /// Couleur d'arrière-plan, que l'on échange avec l'avant-plan (comme dans Photoshop).
    #[serde(rename = "arriere_plan")]
    pub back: [u8; 3],
    /// Roue chromatique : montrée, et ce qu'il y a dans l'anneau (carré ou triangle).
    #[serde(rename = "roue_visible")]
    pub wheel_shown: bool,
    #[serde(rename = "roue")]
    pub wheel: WheelShape,
    /// Curseurs montrés : TSV, RVB ou TSL.
    #[serde(rename = "curseurs")]
    pub sliders: SliderMode,
    /// Dernières couleurs avec lesquelles on a dessiné, la plus récente d'abord.
    #[serde(rename = "recentes")]
    pub recent: Vec<[u8; 3]>,
}

impl Default for ColorSettings {
    fn default() -> Self {
        ColorSettings {
            front: ENCRE,
            back: BLANC,
            wheel_shown: true,
            wheel: WheelShape::Square,
            sliders: SliderMode::Hsv,
            recent: Vec::new(),
        }
    }
}
