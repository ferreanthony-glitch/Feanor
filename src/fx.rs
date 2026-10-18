//! Pile d'effets d'un calque (13i), la « pile FX » de TVPaint 12, en effets vivants (choix
//! d'Anthony, 2026-10-16) : les effets s'appliquent à l'affichage et à l'export, les dessins ne
//! changent pas. Chaque effet a ses paramètres, et des clés (une image, tous les paramètres)
//! entre lesquelles ils varient en ligne droite ; avant la première clé et après la dernière,
//! ils valent ceux de cette clé. Premiers effets (choix d'Anthony) : flou gaussien et flou de
//! mouvement ; puis (2026-10-17) réglages de couleur, netteté, bruit, mosaïque, ombre portée et
//! lueur (`fxfilters.rs`), rangés par familles comme le menu Filtre de Photoshop.
//!
//! Le rendu passe par des dessins dérivés : le dessin filtré est calculé une fois par jeu de
//! paramètres et gardé dans un cache, sous un numéro à part (`fx_id`) ; les moteurs d'affichage
//! et d'export le lisent comme un dessin ordinaire (`shown`).

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::document::{Cel, CelId, Document};
use crate::raster::{Raster, TILE};

/// Un paramètre d'effet : nom, bornes, valeur par défaut, unité.
pub struct Param {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
}

/// La sorte d'un effet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    #[serde(rename = "flou_gaussien")]
    GaussianBlur,
    #[serde(rename = "flou_mouvement")]
    MotionBlur,
    // Réglages, netteté, bruit, mosaïque, ombre, lueur (`fxfilters.rs`, Anthony, 2026-10-17).
    #[serde(rename = "luminosite_contraste")]
    BrightnessContrast,
    #[serde(rename = "niveaux")]
    Levels,
    #[serde(rename = "teinte_saturation")]
    HueSaturation,
    #[serde(rename = "noir_et_blanc")]
    BlackWhite,
    #[serde(rename = "negatif")]
    Invert,
    #[serde(rename = "posterisation")]
    Posterize,
    #[serde(rename = "seuil")]
    Threshold,
    #[serde(rename = "nettete")]
    Sharpen,
    #[serde(rename = "bruit")]
    Noise,
    #[serde(rename = "mosaique")]
    Mosaic,
    #[serde(rename = "ombre_portee")]
    DropShadow,
    #[serde(rename = "lueur")]
    Glow,
    // Déformation et Rendu (`fxwarp.rs`, Anthony, 2026-10-17).
    #[serde(rename = "onde")]
    Wave,
    #[serde(rename = "ondulation")]
    Ripple,
    #[serde(rename = "tourbillon")]
    Twirl,
    #[serde(rename = "pincement")]
    Pinch,
    #[serde(rename = "nuages")]
    Clouds,
    #[serde(rename = "eclairage")]
    Lighting,
    #[serde(rename = "degrade")]
    Gradient,
}

const GAUSSIAN: [Param; 1] = [Param { name: "Rayon", min: 0.0, max: 200.0, default: 8.0, unit: " px" }];
const MOTION: [Param; 2] = [
    Param { name: "Longueur", min: 0.0, max: 400.0, default: 30.0, unit: " px" },
    Param { name: "Angle", min: -180.0, max: 180.0, default: 0.0, unit: "°" },
];
const BRIGHTNESS_CONTRAST: [Param; 2] = [
    Param { name: "Luminosité", min: -100.0, max: 100.0, default: 0.0, unit: "" },
    Param { name: "Contraste", min: -100.0, max: 100.0, default: 0.0, unit: "" },
];
const LEVELS: [Param; 3] = [
    Param { name: "Noir", min: 0.0, max: 254.0, default: 0.0, unit: "" },
    Param { name: "Blanc", min: 1.0, max: 255.0, default: 255.0, unit: "" },
    Param { name: "Gamma", min: 0.1, max: 5.0, default: 1.0, unit: "" },
];
const HUE_SATURATION: [Param; 3] = [
    Param { name: "Teinte", min: -180.0, max: 180.0, default: 0.0, unit: "°" },
    Param { name: "Saturation", min: -100.0, max: 100.0, default: 0.0, unit: "" },
    Param { name: "Luminosité", min: -100.0, max: 100.0, default: 0.0, unit: "" },
];
const AMOUNT: [Param; 1] = [Param { name: "Quantité", min: 0.0, max: 100.0, default: 100.0, unit: " %" }];
const POSTERIZE: [Param; 1] = [Param { name: "Niveaux", min: 2.0, max: 32.0, default: 4.0, unit: "" }];
const THRESHOLD: [Param; 1] = [Param { name: "Seuil", min: 0.0, max: 255.0, default: 128.0, unit: "" }];
const SHARPEN: [Param; 2] = [
    Param { name: "Gain", min: 0.0, max: 300.0, default: 100.0, unit: " %" },
    Param { name: "Rayon", min: 0.5, max: 20.0, default: 2.0, unit: " px" },
];
const NOISE: [Param; 1] = [Param { name: "Quantité", min: 0.0, max: 100.0, default: 20.0, unit: " %" }];
const MOSAIC: [Param; 1] = [Param { name: "Taille", min: 2.0, max: 100.0, default: 10.0, unit: " px" }];
const SHADOW: [Param; 4] = [
    Param { name: "Décalage x", min: -200.0, max: 200.0, default: 10.0, unit: " px" },
    Param { name: "Décalage y", min: -200.0, max: 200.0, default: 10.0, unit: " px" },
    Param { name: "Flou", min: 0.0, max: 100.0, default: 6.0, unit: " px" },
    Param { name: "Opacité", min: 0.0, max: 100.0, default: 60.0, unit: " %" },
];
const GLOW: [Param; 2] = [
    Param { name: "Rayon", min: 1.0, max: 100.0, default: 12.0, unit: " px" },
    Param { name: "Intensité", min: 0.0, max: 200.0, default: 100.0, unit: " %" },
];

const WAVE: [Param; 3] = [
    Param { name: "Amplitude", min: 0.0, max: 200.0, default: 10.0, unit: " px" },
    Param { name: "Longueur d'onde", min: 4.0, max: 1000.0, default: 120.0, unit: " px" },
    Param { name: "Direction", min: -180.0, max: 180.0, default: 0.0, unit: "°" },
];
const RIPPLE: [Param; 4] = [
    Param { name: "Amplitude", min: 0.0, max: 100.0, default: 8.0, unit: " px" },
    Param { name: "Longueur d'onde", min: 4.0, max: 500.0, default: 60.0, unit: " px" },
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
];
const TWIRL: [Param; 4] = [
    Param { name: "Angle", min: -720.0, max: 720.0, default: 90.0, unit: "°" },
    Param { name: "Rayon", min: 1.0, max: 200.0, default: 60.0, unit: " %" },
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
];
const PINCH: [Param; 4] = [
    Param { name: "Quantité", min: -100.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Rayon", min: 1.0, max: 200.0, default: 60.0, unit: " %" },
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
];
const CLOUDS: [Param; 3] = [
    Param { name: "Échelle", min: 2.0, max: 1000.0, default: 120.0, unit: " px" },
    Param { name: "Quantité", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Graine", min: 1.0, max: 999.0, default: 1.0, unit: "" },
];
const LIGHTING: [Param; 4] = [
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 35.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 35.0, unit: " %" },
    Param { name: "Rayon", min: 1.0, max: 300.0, default: 100.0, unit: " %" },
    Param { name: "Intensité", min: 0.0, max: 100.0, default: 40.0, unit: " %" },
];
const GRADIENT: [Param; 2] = [
    Param { name: "Direction", min: -180.0, max: 180.0, default: 90.0, unit: "°" },
    Param { name: "Force", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
];

/// Familles d'effets, dans l'ordre des menus (inspirées du menu Filtre et des réglages de
/// Photoshop).
pub const FAMILIES: [&str; 8] = ["Flou", "Déformation", "Réglages", "Bruit", "Pixéliser", "Rendu", "Netteté", "Styliser"];

impl EffectKind {
    pub const ALL: [EffectKind; 21] = [
        EffectKind::GaussianBlur,
        EffectKind::MotionBlur,
        EffectKind::BrightnessContrast,
        EffectKind::Levels,
        EffectKind::HueSaturation,
        EffectKind::BlackWhite,
        EffectKind::Invert,
        EffectKind::Posterize,
        EffectKind::Threshold,
        EffectKind::Sharpen,
        EffectKind::Noise,
        EffectKind::Mosaic,
        EffectKind::DropShadow,
        EffectKind::Glow,
        EffectKind::Wave,
        EffectKind::Ripple,
        EffectKind::Twirl,
        EffectKind::Pinch,
        EffectKind::Clouds,
        EffectKind::Lighting,
        EffectKind::Gradient,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EffectKind::GaussianBlur => "Flou gaussien",
            EffectKind::MotionBlur => "Flou de mouvement",
            EffectKind::BrightnessContrast => "Luminosité / contraste",
            EffectKind::Levels => "Niveaux",
            EffectKind::HueSaturation => "Teinte / saturation",
            EffectKind::BlackWhite => "Noir et blanc",
            EffectKind::Invert => "Négatif",
            EffectKind::Posterize => "Postérisation",
            EffectKind::Threshold => "Seuil",
            EffectKind::Sharpen => "Netteté",
            EffectKind::Noise => "Bruit",
            EffectKind::Mosaic => "Mosaïque",
            EffectKind::DropShadow => "Ombre portée",
            EffectKind::Glow => "Lueur",
            EffectKind::Wave => "Onde",
            EffectKind::Ripple => "Ondulation",
            EffectKind::Twirl => "Tourbillon",
            EffectKind::Pinch => "Pincement",
            EffectKind::Clouds => "Nuages",
            EffectKind::Lighting => "Éclairage",
            EffectKind::Gradient => "Dégradé",
        }
    }

    /// Sa famille (`FAMILIES`).
    pub fn family(self) -> &'static str {
        match self {
            EffectKind::GaussianBlur | EffectKind::MotionBlur => "Flou",
            EffectKind::BrightnessContrast
            | EffectKind::Levels
            | EffectKind::HueSaturation
            | EffectKind::BlackWhite
            | EffectKind::Invert
            | EffectKind::Posterize
            | EffectKind::Threshold => "Réglages",
            EffectKind::Sharpen => "Netteté",
            EffectKind::Noise => "Bruit",
            EffectKind::Mosaic => "Pixéliser",
            EffectKind::DropShadow | EffectKind::Glow => "Styliser",
            EffectKind::Wave | EffectKind::Ripple | EffectKind::Twirl | EffectKind::Pinch => "Déformation",
            EffectKind::Clouds | EffectKind::Lighting | EffectKind::Gradient => "Rendu",
        }
    }

    pub fn params(self) -> &'static [Param] {
        match self {
            EffectKind::GaussianBlur => &GAUSSIAN,
            EffectKind::MotionBlur => &MOTION,
            EffectKind::BrightnessContrast => &BRIGHTNESS_CONTRAST,
            EffectKind::Levels => &LEVELS,
            EffectKind::HueSaturation => &HUE_SATURATION,
            EffectKind::BlackWhite | EffectKind::Invert => &AMOUNT,
            EffectKind::Posterize => &POSTERIZE,
            EffectKind::Threshold => &THRESHOLD,
            EffectKind::Sharpen => &SHARPEN,
            EffectKind::Noise => &NOISE,
            EffectKind::Mosaic => &MOSAIC,
            EffectKind::DropShadow => &SHADOW,
            EffectKind::Glow => &GLOW,
            EffectKind::Wave => &WAVE,
            EffectKind::Ripple => &RIPPLE,
            EffectKind::Twirl => &TWIRL,
            EffectKind::Pinch => &PINCH,
            EffectKind::Clouds => &CLOUDS,
            EffectKind::Lighting => &LIGHTING,
            EffectKind::Gradient => &GRADIENT,
        }
    }

    /// Avec ces paramètres, l'effet change-t-il quelque chose ? (Sinon il ne coûte rien.)
    pub fn changes(self, v: &[f32]) -> bool {
        let at = |k: usize| v.get(k).copied().unwrap_or(0.0);
        match self {
            EffectKind::GaussianBlur => at(0) >= 0.5,
            EffectKind::MotionBlur => at(0) >= 1.0,
            EffectKind::BrightnessContrast => at(0) != 0.0 || at(1) != 0.0,
            EffectKind::Levels => at(0) > 0.0 || at(1) < 255.0 || (at(2) - 1.0).abs() > 1e-3,
            EffectKind::HueSaturation => at(0) != 0.0 || at(1) != 0.0 || at(2) != 0.0,
            EffectKind::BlackWhite | EffectKind::Invert | EffectKind::Noise => at(0) > 0.0,
            EffectKind::Posterize | EffectKind::Threshold => true,
            EffectKind::Sharpen => at(0) > 0.0,
            EffectKind::Mosaic => at(0) >= 2.0,
            EffectKind::DropShadow => at(3) > 0.0,
            EffectKind::Glow => at(1) > 0.0,
            EffectKind::Wave | EffectKind::Ripple => at(0) > 0.0,
            EffectKind::Twirl | EffectKind::Pinch => at(0) != 0.0,
            EffectKind::Clouds => at(1) > 0.0,
            EffectKind::Lighting => at(3) > 0.0,
            EffectKind::Gradient => at(1) > 0.0,
        }
    }

    fn defaults(self) -> Vec<f32> {
        self.params().iter().map(|p| p.default).collect()
    }
}

/// Une clé : à l'image `frame` du calque, les paramètres valent `values`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxKey {
    #[serde(rename = "image")]
    pub frame: usize,
    #[serde(rename = "valeurs")]
    pub values: Vec<f32>,
}

/// Un effet de la pile d'un calque.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    #[serde(rename = "sorte")]
    pub kind: EffectKind,
    #[serde(rename = "actif", default = "yes")]
    pub enabled: bool,
    /// Les paramètres quand il n'y a pas de clé.
    #[serde(rename = "valeurs")]
    pub values: Vec<f32>,
    /// Les clés, dans l'ordre des images.
    #[serde(rename = "cles", default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<FxKey>,
}

fn yes() -> bool {
    true
}

impl Effect {
    pub fn new(kind: EffectKind) -> Effect {
        Effect { kind, enabled: true, values: kind.defaults(), keys: Vec::new() }
    }

    /// Les paramètres, bornés et complétés (un projet abîmé, un effet d'une autre version).
    fn sane(&self, values: &[f32]) -> Vec<f32> {
        self.kind
            .params()
            .iter()
            .enumerate()
            .map(|(k, p)| values.get(k).copied().filter(|v| v.is_finite()).unwrap_or(p.default).clamp(p.min, p.max))
            .collect()
    }

    /// Les paramètres à l'image `frame` : ceux des clés (en ligne droite entre deux clés), sinon
    /// les valeurs de l'effet.
    pub fn values_at(&self, frame: usize) -> Vec<f32> {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else { return self.sane(&self.values) };
        if frame <= first.frame {
            return self.sane(&first.values);
        }
        if frame >= last.frame {
            return self.sane(&last.values);
        }
        let k = self.keys.iter().position(|k| k.frame > frame).unwrap_or(self.keys.len() - 1);
        let (a, b) = (&self.keys[k - 1], &self.keys[k]);
        let t = (frame - a.frame) as f32 / (b.frame - a.frame).max(1) as f32;
        let (va, vb) = (self.sane(&a.values), self.sane(&b.values));
        va.iter().zip(&vb).map(|(x, y)| x + (y - x) * t).collect()
    }

    /// La clé de l'image `frame`, s'il y en a une.
    pub fn key_at(&self, frame: usize) -> Option<usize> {
        self.keys.iter().position(|k| k.frame == frame)
    }

    /// Pose (ou remplace) une clé à l'image `frame` avec `values`.
    pub fn set_key(&mut self, frame: usize, values: Vec<f32>) {
        let values = self.sane(&values);
        match self.keys.iter().position(|k| k.frame >= frame) {
            Some(i) if self.keys[i].frame == frame => self.keys[i].values = values,
            Some(i) => self.keys.insert(i, FxKey { frame, values }),
            None => self.keys.push(FxKey { frame, values }),
        }
    }

    /// Retire la clé de l'image `frame` ; faux s'il n'y en avait pas.
    pub fn remove_key(&mut self, frame: usize) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| k.frame != frame);
        self.keys.len() != before
    }
}

/// `n` images insérées à l'image `at` (`removed` : retirées) : les clés d'après suivent ; celles
/// des images retirées partent.
pub fn shift_keys(fx: &mut [Effect], at: usize, n: usize, removed: bool) {
    if n == 0 {
        return;
    }
    for e in fx {
        e.keys.retain(|k| !removed || k.frame < at || k.frame >= at + n);
        for k in &mut e.keys {
            if k.frame >= at {
                k.frame = if removed { k.frame - n } else { k.frame + n };
            }
        }
    }
}

/// La pile d'un plan coupé à l'image `at`, pour le second : les mêmes effets, avec les clés
/// d'après la coupe ramenées à 0 (l'effet garde la valeur qu'il avait à la coupe s'il n'en a
/// plus).
pub fn keys_from(fx: &[Effect], at: usize) -> Vec<Effect> {
    fx.iter()
        .map(|e| {
            let mut keys: Vec<FxKey> = e.keys.iter().filter(|k| k.frame >= at).map(|k| FxKey { frame: k.frame - at, values: k.values.clone() }).collect();
            if keys.is_empty() && !e.keys.is_empty() {
                keys.push(FxKey { frame: 0, values: e.values_at(at) });
            }
            Effect { keys, ..e.clone() }
        })
        .collect()
}

/// Un effet tel qu'il s'applique à une image : sa sorte et ses paramètres à cette image.
#[derive(Clone, Debug, PartialEq)]
pub struct FxStep {
    pub kind: EffectKind,
    pub values: Vec<f32>,
}

/// Les effets actifs de la pile qui changent quelque chose à l'image `frame`, dans l'ordre.
pub fn steps_at(fx: &[Effect], frame: usize) -> Vec<FxStep> {
    fx.iter()
        .filter(|e| e.enabled)
        .map(|e| FxStep { kind: e.kind, values: e.values_at(frame) })
        .filter(|s| s.kind.changes(&s.values))
        .collect()
}

/// Numéro du dessin dérivé : le dessin `cel` passé par `steps` (bit de poids fort mis : il ne
/// tombe jamais sur un vrai dessin).
pub fn fx_id(cel: CelId, steps: &[FxStep]) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cel.hash(&mut h);
    for s in steps {
        s.kind.hash(&mut h);
        for v in &s.values {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish() | (1 << 63)
}

// ---------------------------------------------------------------------- calcul des flous

/// Une zone de pixels prémultipliés, en ligne.
pub(crate) struct Region {
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
    pub px: Vec<Color32>,
}

/// Les pixels dessinés de `raster`, avec une marge de `margin` pixels (dans la feuille). Rien si
/// le dessin est vide.
pub(crate) fn region_of(raster: &Raster, margin: usize) -> Option<Region> {
    let mut lo = (usize::MAX, usize::MAX);
    let mut hi = (0, 0);
    for (idx, t) in raster.tiles.iter().enumerate() {
        if t.is_some() {
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            lo = (lo.0.min(ox), lo.1.min(oy));
            hi = (hi.0.max(ox + tw), hi.1.max(oy + th));
        }
    }
    if lo.0 == usize::MAX {
        return None;
    }
    let (x0, y0) = (lo.0.saturating_sub(margin), lo.1.saturating_sub(margin));
    let (x1, y1) = ((hi.0 + margin).min(raster.width), (hi.1 + margin).min(raster.height));
    let (w, h) = (x1 - x0, y1 - y0);
    let mut px = vec![Color32::TRANSPARENT; w * h];
    for (y, row) in px.chunks_mut(w).enumerate() {
        for (x, p) in row.iter_mut().enumerate() {
            *p = raster.get(x0 + x, y0 + y);
        }
    }
    Some(Region { x0, y0, w, h, px })
}

/// La zone remise dans une feuille vide de la taille de `like` (tuiles toutes transparentes
/// laissées vides).
pub(crate) fn into_raster(r: &Region, like: &Raster) -> Raster {
    let mut out = Raster::new(like.width, like.height);
    let (tx0, ty0) = (r.x0 / TILE, r.y0 / TILE);
    let (tx1, ty1) = ((r.x0 + r.w).div_ceil(TILE), (r.y0 + r.h).div_ceil(TILE));
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            let idx = ty * out.tiles_x + tx;
            let (ox, oy, tw, th) = out.tile_bounds(idx);
            let mut any = false;
            let mut tile = crate::raster::empty_tile();
            for y in 0..th {
                for x in 0..tw {
                    let (sx, sy) = (ox + x, oy + y);
                    if sx < r.x0 || sy < r.y0 || sx >= r.x0 + r.w || sy >= r.y0 + r.h {
                        continue;
                    }
                    let c = r.px[(sy - r.y0) * r.w + (sx - r.x0)];
                    if c.a() != 0 {
                        any = true;
                        tile[y * TILE + x] = c;
                    }
                }
            }
            if any {
                out.tiles[idx] = Some(tile);
            }
        }
    }
    out
}

/// Flou en boîte horizontal de rayon `r` sur chaque ligne de `px` (largeur `w`), hors champ
/// transparent.
fn box_rows(px: &mut [Color32], w: usize, r: usize) {
    if r == 0 || w == 0 {
        return;
    }
    let size = (2 * r + 1) as u32;
    crate::render::par_chunks(px, w * 16, |_, rows| {
        let mut line = vec![Color32::TRANSPARENT; w];
        for row in rows.chunks_mut(w) {
            line.copy_from_slice(row);
            let mut acc = [0u32; 4];
            let add = |acc: &mut [u32; 4], c: Color32, sign: bool| {
                for (k, v) in c.to_array().iter().enumerate() {
                    if sign {
                        acc[k] += *v as u32;
                    } else {
                        acc[k] -= *v as u32;
                    }
                }
            };
            for c in line.iter().take(r.min(w - 1) + 1) {
                add(&mut acc, *c, true);
            }
            for x in 0..w {
                let m = |v: u32| ((v + size / 2) / size) as u8;
                row[x] = Color32::from_rgba_premultiplied(m(acc[0]), m(acc[1]), m(acc[2]), m(acc[3]));
                if x + r + 1 < w {
                    add(&mut acc, line[x + r + 1], true);
                }
                if x >= r {
                    add(&mut acc, line[x - r], false);
                }
            }
        }
    });
}

fn transpose(px: &[Color32], w: usize, h: usize) -> Vec<Color32> {
    let mut out = vec![Color32::TRANSPARENT; w * h];
    for y in 0..h {
        for x in 0..w {
            out[x * h + y] = px[y * w + x];
        }
    }
    out
}

/// Rayons des trois flous en boîte qui approchent un flou gaussien d'écart type `sigma`.
fn boxes_for(sigma: f32) -> [usize; 3] {
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut wl = ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - 3.0 * (wl * wl) as f32 - 12.0 * wl as f32 - 9.0) / (-4.0 * wl as f32 - 4.0)).round() as i64;
    std::array::from_fn(|i| (((if (i as i64) < m { wl } else { wu }) - 1) / 2).max(0) as usize)
}

/// Flou gaussien : le rayon est la portée visible du flou (trois écarts types).
pub(crate) fn gaussian(raster: &Raster, radius: f32) -> Raster {
    let margin = radius.ceil() as usize + 2;
    let Some(mut r) = region_of(raster, margin) else { return Raster::new(raster.width, raster.height) };
    let boxes = boxes_for(radius / 3.0);
    for b in boxes {
        box_rows(&mut r.px, r.w, b);
    }
    let mut t = transpose(&r.px, r.w, r.h);
    for b in boxes {
        box_rows(&mut t, r.h, b);
    }
    r.px = transpose(&t, r.h, r.w);
    into_raster(&r, raster)
}

/// Flou de mouvement : chaque point est la moyenne du dessin le long d'un segment de `length`
/// pixels centré sur lui, dans la direction `angle` (degrés, 0 = horizontale, sens horaire à
/// l'écran).
fn motion(raster: &Raster, length: f32, angle: f32) -> Raster {
    let margin = (length / 2.0).ceil() as usize + 2;
    let Some(r) = region_of(raster, margin) else { return Raster::new(raster.width, raster.height) };
    let (s, c) = angle.to_radians().sin_cos();
    let n = (length.ceil() as usize).clamp(2, 64);
    let offsets: Vec<(f32, f32)> = (0..n).map(|i| {
        let t = (i as f32 / (n - 1) as f32 - 0.5) * length;
        (t * c, t * s)
    }).collect();
    let (w, h) = (r.w, r.h);
    let src = &r.px;
    let get = |x: i64, y: i64| -> [f32; 4] {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            return [0.0; 4];
        }
        let p = src[y as usize * w + x as usize];
        [p.r() as f32, p.g() as f32, p.b() as f32, p.a() as f32]
    };
    let mut out = vec![Color32::TRANSPARENT; w * h];
    crate::render::par_chunks(&mut out, w * 8, |chunk, rows| {
        for (k, o) in rows.iter_mut().enumerate() {
            let (x, y) = ((k % w) as f32, (chunk * 8 + k / w) as f32);
            let mut acc = [0.0f32; 4];
            for (dx, dy) in &offsets {
                let (fx, fy) = (x + dx, y + dy);
                let (ix, iy) = (fx.floor(), fy.floor());
                let (tx, ty) = (fx - ix, fy - iy);
                let (ix, iy) = (ix as i64, iy as i64);
                let (a, b, cc, d) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
                for q in 0..4 {
                    let top = a[q] + (b[q] - a[q]) * tx;
                    let bottom = cc[q] + (d[q] - cc[q]) * tx;
                    acc[q] += top + (bottom - top) * ty;
                }
            }
            let m = |v: f32| (v / n as f32).round().clamp(0.0, 255.0) as u8;
            *o = Color32::from_rgba_premultiplied(m(acc[0]), m(acc[1]), m(acc[2]), m(acc[3]));
        }
    });
    into_raster(&Region { px: out, ..r }, raster)
}

/// Le dessin `raster` passé par les effets `steps`, dans l'ordre.
pub fn apply(raster: &Raster, steps: &[FxStep]) -> Raster {
    if steps.is_empty() {
        return raster.clone();
    }
    let mut cur = raster.clone();
    for s in steps {
        use crate::fxfilters as f;
        let v = &s.values;
        cur = match s.kind {
            EffectKind::GaussianBlur => gaussian(&cur, v[0]),
            EffectKind::MotionBlur => motion(&cur, v[0], v[1]),
            EffectKind::BrightnessContrast => f::brightness_contrast(&cur, v[0], v[1]),
            EffectKind::Levels => f::levels(&cur, v[0], v[1], v[2]),
            EffectKind::HueSaturation => f::hue_saturation(&cur, v[0], v[1], v[2]),
            EffectKind::BlackWhite => f::black_white(&cur, v[0]),
            EffectKind::Invert => f::invert(&cur, v[0]),
            EffectKind::Posterize => f::posterize(&cur, v[0]),
            EffectKind::Threshold => f::threshold(&cur, v[0]),
            EffectKind::Sharpen => f::sharpen(&cur, v[0], v[1]),
            EffectKind::Noise => f::noise(&cur, v[0]),
            EffectKind::Mosaic => f::mosaic(&cur, v[0]),
            EffectKind::DropShadow => f::drop_shadow(&cur, v[0], v[1], v[2], v[3]),
            EffectKind::Glow => f::glow(&cur, v[0], v[1]),
            EffectKind::Wave => crate::fxwarp::wave(&cur, v[0], v[1], v[2]),
            EffectKind::Ripple => crate::fxwarp::ripple(&cur, v[0], v[1], v[2], v[3]),
            EffectKind::Twirl => crate::fxwarp::twirl(&cur, v[0], v[1], v[2], v[3]),
            EffectKind::Pinch => crate::fxwarp::pinch(&cur, v[0], v[1], v[2], v[3]),
            EffectKind::Clouds => crate::fxwarp::clouds(&cur, v[0], v[1], v[2]),
            EffectKind::Lighting => crate::fxwarp::lighting(&cur, v[0], v[1], v[2], v[3]),
            EffectKind::Gradient => crate::fxwarp::gradient(&cur, v[0], v[1]),
        };
    }
    cur
}

// ---------------------------------------------------------------------- cache des dessins dérivés

struct Entry {
    version: u64,
    cel: Arc<Cel>,
    used: u64,
}

/// Dessins dérivés gardés : les plus anciens partent au-delà de `CACHE_MAX`.
const CACHE_MAX: usize = 48;
static CACHE: Mutex<BTreeMap<CelId, Entry>> = Mutex::new(BTreeMap::new());
static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Un dessin tel qu'il s'affiche : le vrai, ou le dérivé de ses effets.
pub enum Shown<'a> {
    Plain(&'a Cel),
    Fx(Arc<Cel>),
}

impl std::ops::Deref for Shown<'_> {
    type Target = Cel;
    fn deref(&self) -> &Cel {
        match self {
            Shown::Plain(c) => c,
            Shown::Fx(c) => c,
        }
    }
}

/// Numéro du dessin dérivé d'un élément du plan : le dessin lui-même sans effet ni remplissage
/// CTG, sinon un numéro propre à ce dessin, ces effets et ces sources.
pub fn shown_id(item: &crate::render::PlanItem) -> CelId {
    // Fond sous le trait (`fond.rs`) : un numéro propre au fond de ce dessin.
    if let Some(part) = &item.fond {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        item.cel.hash(&mut h);
        "fond".hash(&mut h);
        match part {
            crate::render::FondPart::Forme(r) => r.hash(&mut h),
            crate::render::FondPart::Peinture => "peinture".hash(&mut h),
        }
        return h.finish() | (1 << 63);
    }
    match &item.ctg {
        None if item.fx.is_empty() => item.cel,
        None => fx_id(item.cel, &item.fx),
        Some(input) => {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            fx_id(item.cel, &item.fx).hash(&mut h);
            "ctg".hash(&mut h);
            input.hash(&mut h);
            h.finish() | (1 << 63)
        }
    }
}

/// Le dessin d'un élément du plan tel qu'il s'affiche : effets (13i) et remplissage CTG (13j)
/// compris.
pub fn shown<'a>(doc: &'a Document, item: &crate::render::PlanItem) -> Option<Shown<'a>> {
    match item.fond {
        None => shown_with(doc, item.cel, &item.fx, item.ctg.as_ref()),
        // Ce qu'on peint dans le fond, en direct (la strate elle-même).
        Some(crate::render::FondPart::Peinture) => doc.cels.get(&item.cel)?.fond.as_deref().map(Shown::Plain),
        Some(crate::render::FondPart::Forme(r)) => fond_shown(doc, item, &r),
    }
}

/// Le fond sous le trait du dessin de `item` : sa forme et ce qu'on y a peint (calculé une fois,
/// puis gardé tant que le dessin, sa strate Fond et les réglages ne changent pas).
fn fond_shown<'a>(doc: &'a Document, item: &crate::render::PlanItem, r: &crate::fond::FondReglages) -> Option<Shown<'a>> {
    let src = doc.cels.get(&item.cel)?;
    let id = shown_id(item);
    let version = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        src.version.hash(&mut h);
        src.fond.as_ref().map_or(0, |f| f.version + 1).hash(&mut h);
        h.finish()
    };
    let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = cache.get_mut(&id).filter(|e| e.version == version) {
        e.used = tick;
        return Some(Shown::Fx(e.cel.clone()));
    }
    let (w, h) = (src.raster.width, src.raster.height);
    let cover = crate::fond::silhouette(&src.raster, r);
    let raster = crate::fond::compose(&cover, w, h, r.couleur, src.fond.as_ref().map(|f| &f.raster));
    let derived = Arc::new(Cel { raster, vector: None, version, fond: None });
    cache.insert(id, Entry { version, cel: derived.clone(), used: tick });
    if cache.len() > CACHE_MAX {
        if let Some(old) = cache.iter().min_by_key(|(_, e)| e.used).map(|(k, _)| *k) {
            cache.remove(&old);
        }
    }
    Some(Shown::Fx(derived))
}

/// Le dessin `cel`, rempli d'après `ctg` (dessins des sources, trous) s'il est d'un calque CTG, puis passé par
/// les effets `steps` (calculé une fois, puis gardé tant qu'aucun de ces dessins ne change).
pub fn shown_with<'a>(doc: &'a Document, cel: CelId, steps: &[FxStep], ctg: Option<&crate::ctg::CtgInput>) -> Option<Shown<'a>> {
    // Pas un dessin du projet : une image de calque vidéo (13k), lue à la demande.
    let Some(src) = doc.cels.get(&cel) else { return crate::video::shown(doc, cel).map(Shown::Fx) };
    if steps.is_empty() && ctg.is_none() {
        return Some(Shown::Plain(src));
    }
    let item = crate::render::PlanItem {
        cel,
        opacity: 255,
        tint: None,
        transform: None,
        blend: Default::default(),
        fx: steps.to_vec(),
        ctg: ctg.cloned(),
        fond: None,
    };
    let id = shown_id(&item);
    // Version : celle du dessin, et celles des sources du remplissage.
    let version = match ctg {
        None => src.version,
        Some(input) => {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            src.version.hash(&mut h);
            for s in &input.sources {
                doc.cels.get(s).map_or(0, |c| c.version).hash(&mut h);
            }
            h.finish()
        }
    };
    let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = cache.get_mut(&id).filter(|e| e.version == version) {
        e.used = tick;
        return Some(Shown::Fx(e.cel.clone()));
    }
    let base = match ctg {
        None => src.raster.clone(),
        Some(input) => {
            let rasters: Vec<&Raster> = input.sources.iter().filter_map(|s| doc.cels.get(s)).map(|c| &c.raster).collect();
            crate::ctg::fill(&rasters, &src.raster, input.gap as usize)
        }
    };
    let derived = Arc::new(Cel { raster: apply(&base, steps), vector: None, version, fond: None });
    cache.insert(id, Entry { version, cel: derived.clone(), used: tick });
    if cache.len() > CACHE_MAX {
        if let Some(old) = cache.iter().min_by_key(|(_, e)| e.used).map(|(k, _)| *k) {
            cache.remove(&old);
        }
    }
    Some(Shown::Fx(derived))
}
