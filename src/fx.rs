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
    // Tremblement du trait (`tremblement.rs`, Anthony, 2026-10-19), calques vectoriels seulement.
    #[serde(rename = "tremblement")]
    Boil,
    // Effets « Cinéma » (`fxcine.rs`, calques d'effets et préréglages d'effets, 2026-10-19).
    #[serde(rename = "flash")]
    Flash,
    #[serde(rename = "image_impact")]
    Impact,
    #[serde(rename = "lueur_diffuse")]
    Bloom,
    #[serde(rename = "secousse")]
    Shake,
    #[serde(rename = "vignettage")]
    Vignette,
    #[serde(rename = "grain")]
    Grain,
    #[serde(rename = "aberration")]
    Aberration,
    // Effets du trait (`fxtrait.rs`, Anthony, 2026-10-20), calques vectoriels seulement.
    #[serde(rename = "ecriture_trait")]
    Write,
    #[serde(rename = "crayonne")]
    Sketch,
    #[serde(rename = "respiration")]
    Breath,
    #[serde(rename = "ondulation_trait")]
    StrokeWave,
    #[serde(rename = "pointilles")]
    Dashes,
    #[serde(rename = "desagregation")]
    Dust,
    // Effets de matière du trait (`fxmatiere.rs`, Anthony, 2026-10-20) : le liseré, le trait sec et l'encre
    // qui bave marchent sur les deux sortes de calques ; le dégradé et la plume, sur le vectoriel seulement.
    #[serde(rename = "lisere")]
    Outline,
    #[serde(rename = "trait_sec")]
    Dry,
    #[serde(rename = "encre_bave")]
    Bleed,
    #[serde(rename = "degrade_trait")]
    StrokeGradient,
    #[serde(rename = "plume")]
    Nib,
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

/// Tremblement du trait (`tremblement.rs`) : les rangs sont ceux de `tremblement::INTENSITY`…
/// `REST`.
const BOIL: [Param; 7] = [
    Param { name: "Intensité", min: 0.0, max: 20.0, default: 3.0, unit: " px" },
    Param { name: "Fréquence", min: 0.0, max: 100.0, default: 55.0, unit: " %" },
    Param { name: "Variantes", min: 2.0, max: 6.0, default: 3.0, unit: "" },
    Param { name: "Vitesse", min: 1.0, max: 12.0, default: 2.0, unit: " img/var." },
    Param { name: "Épaisseur", min: 0.0, max: 100.0, default: 0.0, unit: " %" },
    Param { name: "Graine", min: 1.0, max: 999.0, default: 1.0, unit: "" },
    Param { name: "Animer à l'arrêt", min: 0.0, max: 1.0, default: 0.0, unit: "" },
];

/// Effets « Cinéma » (`fxcine.rs`). Les rangs de l'image d'impact sont ceux de `fxcine::IMPACT_*`.
const FLASH: [Param; 5] = [
    Param { name: "Quantité", min: 0.0, max: 100.0, default: 100.0, unit: " %" },
    Param { name: "Rouge", min: 0.0, max: 255.0, default: 255.0, unit: "" },
    Param { name: "Vert", min: 0.0, max: 255.0, default: 255.0, unit: "" },
    Param { name: "Bleu", min: 0.0, max: 255.0, default: 255.0, unit: "" },
    Param { name: "Écran (éclaircit seulement)", min: 0.0, max: 1.0, default: 0.0, unit: "" },
];
const IMPACT: [Param; 12] = [
    Param { name: "Quantité", min: 0.0, max: 100.0, default: 100.0, unit: " %" },
    Param { name: "Mode (0 négatif, 1 noir et blanc, 2 inversé, 3 postérisé)", min: 0.0, max: 3.0, default: 0.0, unit: "" },
    Param { name: "Seuil", min: 0.0, max: 255.0, default: 128.0, unit: "" },
    Param { name: "Teinte : rouge", min: 0.0, max: 255.0, default: 230.0, unit: "" },
    Param { name: "Teinte : vert", min: 0.0, max: 255.0, default: 30.0, unit: "" },
    Param { name: "Teinte : bleu", min: 0.0, max: 255.0, default: 30.0, unit: "" },
    Param { name: "Force de la teinte", min: 0.0, max: 100.0, default: 0.0, unit: " %" },
    Param { name: "Éclat (rayons)", min: 0.0, max: 100.0, default: 0.0, unit: " %" },
    Param { name: "Lignes de vitesse", min: 0.0, max: 100.0, default: 0.0, unit: " %" },
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Graine", min: 1.0, max: 999.0, default: 1.0, unit: "" },
];
const BLOOM: [Param; 3] = [
    Param { name: "Rayon", min: 1.0, max: 300.0, default: 30.0, unit: " px" },
    Param { name: "Intensité", min: 0.0, max: 300.0, default: 100.0, unit: " %" },
    Param { name: "Seuil", min: 0.0, max: 255.0, default: 140.0, unit: "" },
];
const SHAKE: [Param; 5] = [
    Param { name: "Décalage x", min: -300.0, max: 300.0, default: 0.0, unit: " px" },
    Param { name: "Décalage y", min: -300.0, max: 300.0, default: 0.0, unit: " px" },
    Param { name: "Rotation", min: -20.0, max: 20.0, default: 0.0, unit: "°" },
    Param { name: "Zoom", min: 50.0, max: 200.0, default: 100.0, unit: " %" },
    Param { name: "Flou de bougé", min: 0.0, max: 100.0, default: 0.0, unit: " %" },
];
const VIGNETTE: [Param; 7] = [
    Param { name: "Intensité", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Rayon", min: 0.0, max: 150.0, default: 60.0, unit: " %" },
    Param { name: "Douceur", min: 1.0, max: 150.0, default: 60.0, unit: " %" },
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Largeur du cadre", min: 1.0, max: 200.0, default: 100.0, unit: " %" },
    Param { name: "Hauteur du cadre", min: 1.0, max: 200.0, default: 100.0, unit: " %" },
];
/// Le grain porte à la fin de ses valeurs l'image (comme le tremblement sa variante) : il change à
/// chaque image, même quand rien d'autre ne bouge.
const GRAIN: [Param; 3] = [
    Param { name: "Quantité", min: 0.0, max: 100.0, default: 25.0, unit: " %" },
    Param { name: "Taille", min: 1.0, max: 8.0, default: 1.0, unit: " px" },
    Param { name: "Graine", min: 1.0, max: 999.0, default: 1.0, unit: "" },
];
const ABERRATION: [Param; 3] = [
    Param { name: "Force", min: 0.0, max: 60.0, default: 6.0, unit: " px" },
    Param { name: "Centre x", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
    Param { name: "Centre y", min: 0.0, max: 100.0, default: 50.0, unit: " %" },
];

/// Comment se règle un paramètre dans le panneau Effets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamStyle {
    /// Un curseur.
    Slider,
    /// Un curseur à valeurs entières.
    Integer,
    /// Une case à cocher (0 ou 1).
    Switch,
    /// Un choix de plume (liste des réglages de pinceau ; ne s'anime pas).
    Nib,
}

/// Familles d'effets, dans l'ordre des menus (inspirées du menu Filtre et des réglages de
/// Photoshop).
pub const FAMILIES: [&str; 11] = ["Trait", "Matière", "Flou", "Déformation", "Réglages", "Bruit", "Pixéliser", "Rendu", "Netteté", "Styliser", "Cinéma"];

impl EffectKind {
    pub const ALL: [EffectKind; 40] = [
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
        EffectKind::Boil,
        EffectKind::Flash,
        EffectKind::Impact,
        EffectKind::Bloom,
        EffectKind::Shake,
        EffectKind::Vignette,
        EffectKind::Grain,
        EffectKind::Aberration,
        EffectKind::Write,
        EffectKind::Sketch,
        EffectKind::Breath,
        EffectKind::StrokeWave,
        EffectKind::Dashes,
        EffectKind::Dust,
        EffectKind::Outline,
        EffectKind::Dry,
        EffectKind::Bleed,
        EffectKind::StrokeGradient,
        EffectKind::Nib,
    ];

    /// L'effet est de la famille « Trait » ou « Matière » (`fxtrait.rs`, `fxmatiere.rs`) : calculé en
    /// arrière-plan, sans aperçu réduit ni passe sur la carte graphique, et une seule fois par calque.
    pub fn is_trait(self) -> bool {
        self.vector_stage() || matches!(self, EffectKind::Outline | EffectKind::Bleed)
    }

    /// L'effet lit le dessin vectoriel (traits, abscisse, tracé) et passe avant les autres effets de la pile
    /// (`fxtrait::render`) ; sans dessin vectoriel, ceux qui le peuvent agissent sur les pixels (le trait sec).
    pub fn vector_stage(self) -> bool {
        matches!(
            self,
            EffectKind::Write
                | EffectKind::Sketch
                | EffectKind::Breath
                | EffectKind::StrokeWave
                | EffectKind::Dashes
                | EffectKind::Dust
                | EffectKind::Dry
                | EffectKind::StrokeGradient
                | EffectKind::Nib
        )
    }

    /// L'effet se calcule hors de la feuille comme dedans : ses réglages ne dépendent ni du centre ni de la taille de la feuille (`effets_hors_de_la_feuille`).
    fn suit_la_toile(self) -> bool {
        !matches!(
            self,
            EffectKind::Wave | EffectKind::Ripple | EffectKind::Twirl | EffectKind::Pinch | EffectKind::Clouds | EffectKind::Lighting | EffectKind::Gradient | EffectKind::Flash | EffectKind::Impact | EffectKind::Shake | EffectKind::Vignette | EffectKind::Aberration
        )
    }

    /// L'effet ne touche que les tuiles dessinées : son coût ne dépend pas du rectangle qui les contient.
    fn par_tuile(self) -> bool {
        matches!(
            self,
            EffectKind::Boil | EffectKind::BrightnessContrast | EffectKind::Levels | EffectKind::HueSaturation | EffectKind::BlackWhite | EffectKind::Invert | EffectKind::Posterize | EffectKind::Threshold
        )
    }

    /// L'effet n'a de sens que sur un calque vectoriel (le tremblement du trait et les effets du trait qui
    /// lisent les traits). Le liseré, l'encre qui bave et le trait sec marchent aussi sur un bitmap.
    pub fn vector_only(self) -> bool {
        self == EffectKind::Boil || (self.vector_stage() && self != EffectKind::Dry)
    }

    /// Une seule fois par calque : le tremblement et les effets du trait et de la matière (les menus les
    /// cochent et les décochent comme un interrupteur).
    pub fn single(self) -> bool {
        self == EffectKind::Boil || self.is_trait()
    }

    /// Le réglage `k` de l'effet se règle-t-il par un curseur, un curseur entier ou une case ?
    pub fn param_style(self, k: usize) -> ParamStyle {
        use crate::fxmatiere as m;
        match (self, k) {
            (EffectKind::Outline, m::OUTLINE_BEHIND) | (EffectKind::StrokeGradient, m::GRAD_WHOLE | m::GRAD_LOOP) => ParamStyle::Switch,
            (EffectKind::Dry, m::DRY_SEED | m::DRY_EVERY | m::DRY_VARIANTS) | (EffectKind::Bleed, m::BLEED_SEED | m::BLEED_GRAIN) | (EffectKind::StrokeGradient, m::GRAD_COUNT) => ParamStyle::Integer,
            (EffectKind::Nib, m::NIB_ID) => ParamStyle::Nib,
            (EffectKind::Boil, crate::tremblement::VARIANTS | crate::tremblement::SPEED | crate::tremblement::SEED) => ParamStyle::Integer,
            (EffectKind::Boil, crate::tremblement::REST) => ParamStyle::Switch,
            (EffectKind::Write, 1 | 2 | 3 | 6) | (EffectKind::StrokeWave, 3) | (EffectKind::Dust, 7) => ParamStyle::Switch,
            (EffectKind::Sketch, 0 | 4 | 5 | 6) | (EffectKind::Breath, 2) | (EffectKind::Dust, 1 | 6) => ParamStyle::Integer,
            (EffectKind::Flash, 4) => ParamStyle::Switch,
            (EffectKind::Impact, crate::fxcine::IMPACT_MODE | crate::fxcine::IMPACT_SEED) => ParamStyle::Integer,
            (EffectKind::Grain, 1 | 2) => ParamStyle::Integer,
            _ => ParamStyle::Slider,
        }
    }

    /// Ce que fait l'effet, en une ou deux phrases (bulle du nom dans le panneau Effets) ; vide pour
    /// les effets classiques.
    pub fn hint(self) -> &'static str {
        match self {
            EffectKind::Boil => "Le trait « bout » comme dans WigglyPaint : le dessin est montré en quelques variantes légèrement déplacées, qui se suivent. Les dessins ne changent pas.",
            k if k.is_trait() => crate::fxtrait::hint(k),
            _ => "",
        }
    }

    /// Ce que règle le paramètre `k` (bulle de son nom) ; vide s'il n'y a rien à dire.
    pub fn param_hint(self, k: usize) -> &'static str {
        crate::fxtrait::param_hint(self, k)
    }

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
            EffectKind::Boil => "Tremblement du trait",
            EffectKind::Flash => "Flash",
            EffectKind::Impact => "Image d'impact",
            EffectKind::Bloom => "Lueur diffuse",
            EffectKind::Shake => "Secousse de l'image",
            EffectKind::Vignette => "Vignettage",
            EffectKind::Grain => "Grain de film",
            EffectKind::Aberration => "Aberration chromatique",
            EffectKind::Write => "Écriture du trait",
            EffectKind::Sketch => "Crayonné multiple",
            EffectKind::Breath => "Respiration du trait",
            EffectKind::StrokeWave => "Ondulation du trait",
            EffectKind::Dashes => "Pointillés qui défilent",
            EffectKind::Dust => "Désagrégation",
            EffectKind::Outline => "Liseré",
            EffectKind::Dry => "Trait sec",
            EffectKind::Bleed => "Encre qui bave",
            EffectKind::StrokeGradient => "Dégradé le long du trait",
            EffectKind::Nib => "Changer de plume",
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
            EffectKind::Flash
            | EffectKind::Impact
            | EffectKind::Bloom
            | EffectKind::Shake
            | EffectKind::Vignette
            | EffectKind::Grain
            | EffectKind::Aberration => "Cinéma",
            // (Le tremblement du trait est de la famille Trait depuis 2026-10-21 : seul le regroupement des
            // menus change, il s'enregistre par son nom de sorte, pas par sa famille.)
            EffectKind::Boil | EffectKind::Write | EffectKind::Sketch | EffectKind::Breath | EffectKind::StrokeWave | EffectKind::Dashes | EffectKind::Dust => "Trait",
            EffectKind::Outline | EffectKind::Dry | EffectKind::Bleed | EffectKind::StrokeGradient | EffectKind::Nib => "Matière",
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
            EffectKind::Boil => &BOIL,
            EffectKind::Flash => &FLASH,
            EffectKind::Impact => &IMPACT,
            EffectKind::Bloom => &BLOOM,
            EffectKind::Shake => &SHAKE,
            EffectKind::Vignette => &VIGNETTE,
            EffectKind::Grain => &GRAIN,
            EffectKind::Aberration => &ABERRATION,
            EffectKind::Write => &crate::fxtrait::WRITE,
            EffectKind::Sketch => &crate::fxtrait::SKETCH,
            EffectKind::Breath => &crate::fxtrait::BREATH,
            EffectKind::StrokeWave => &crate::fxtrait::WAVE,
            EffectKind::Dashes => &crate::fxtrait::DASH,
            EffectKind::Dust => &crate::fxtrait::DUST,
            EffectKind::Outline => &crate::fxmatiere::OUTLINE,
            EffectKind::Dry => &crate::fxmatiere::DRY,
            EffectKind::Bleed => &crate::fxmatiere::BLEED,
            EffectKind::StrokeGradient => &crate::fxmatiere::GRADIENT,
            EffectKind::Nib => &crate::fxmatiere::NIB,
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
            EffectKind::Boil => crate::tremblement::Settings::of(v).changes(),
            EffectKind::Flash | EffectKind::Impact | EffectKind::Vignette | EffectKind::Grain | EffectKind::Aberration => at(0) > 0.0,
            EffectKind::Bloom => at(1) > 0.0,
            EffectKind::Shake => at(0) != 0.0 || at(1) != 0.0 || at(2) != 0.0 || (at(3) - 100.0).abs() > 1e-3,
            EffectKind::Write => crate::fxtrait::WriteS::of(v).changes(),
            EffectKind::Sketch => crate::fxtrait::SketchS::of(v).changes(),
            EffectKind::Breath => crate::fxtrait::BreathS::of(v).changes(),
            EffectKind::StrokeWave => crate::fxtrait::WaveS::of(v).changes(),
            EffectKind::Dashes => true,
            EffectKind::Dust => crate::fxtrait::DustS::of(v).changes(),
            EffectKind::Outline => crate::fxmatiere::OutlineS::of(v).changes(),
            EffectKind::Dry => crate::fxmatiere::DryS::of(v).changes(),
            EffectKind::Bleed => crate::fxmatiere::BleedS::of(v).changes(),
            EffectKind::StrokeGradient => crate::fxmatiere::GradS::of(v).changes(),
            EffectKind::Nib => crate::fxmatiere::NibS::of(v).changes(),
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

    /// Change le paramètre `k` partout : dans les valeurs de l'effet et dans chacune de ses clés
    /// (un réglage qui ne s'anime pas, comme « Animer à l'arrêt »).
    pub fn set_param(&mut self, k: usize, v: f32) {
        let v = self.kind.params().get(k).map_or(v, |p| v.clamp(p.min, p.max));
        if let Some(x) = self.values.get_mut(k) {
            *x = v;
        }
        for key in &mut self.keys {
            if let Some(x) = key.values.get_mut(k) {
                *x = v;
            }
        }
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
    steps_at_with(fx, frame, None, true)
}

/// Comme `steps_at`. `rest` : l'image virtuelle du repos (`tremblement::rest_frame`), que prennent
/// à la place de `frame` les tremblements qui animent à l'arrêt ; `vector` : le calque est
/// vectoriel (sinon les effets réservés au vectoriel sont laissés). Le tremblement porte à la fin
/// de ses valeurs la variante de l'image (`tremblement::VARIANT`) : c'est ce qui fait de chaque
/// variante un dessin dérivé à part, gardé sous son propre numéro (`fx_id`).
pub fn steps_at_with(fx: &[Effect], frame: usize, rest: Option<usize>, vector: bool) -> Vec<FxStep> {
    fx.iter()
        .filter(|e| e.enabled && (vector || !e.kind.vector_only()))
        .map(|e| {
            let mut values = e.values_at(frame);
            if e.kind == EffectKind::Boil {
                let variant = crate::tremblement::variant_for(&values, frame, rest);
                values.push(variant as f32);
            }
            // Le grain de film change à chaque image : l'image est la dernière de ses valeurs.
            if e.kind == EffectKind::Grain {
                values.push(frame as f32);
            }
            // Le crayonné se retire toutes les N images (sa variante) ; la respiration, l'ondulation et
            // les pointillés battent avec le temps (l'image).
            match e.kind {
                EffectKind::Sketch => values.push(crate::fxtrait::sketch_variant(&values, frame) as f32),
                EffectKind::Breath | EffectKind::StrokeWave | EffectKind::Dashes => values.push(frame as f32),
                // Le trait sec se retire toutes les N images (sa variante) ; le dégradé défile avec l'image
                // (sinon un dessin dérivé suffit pour toutes) ; la plume porte la signature de son réglage.
                EffectKind::Dry => values.push(crate::fxmatiere::dry_variant(&values, frame) as f32),
                EffectKind::StrokeGradient => {
                    let scrolls = values.get(crate::fxmatiere::GRAD_SCROLL).is_some_and(|s| s.abs() > 1e-6);
                    values.push(if scrolls { frame as f32 } else { 0.0 });
                }
                EffectKind::Nib => {
                    let id = crate::fxmatiere::NibS::of(&values).id;
                    values.push(crate::fxmatiere::nib_signature(id) as f32);
                }
                _ => {}
            }
            FxStep { kind: e.kind, values }
        })
        .filter(|s| s.kind.changes(&s.values))
        .collect()
}

/// Les effets du calque `layer` à l'image `frame` (voir `steps_at_with`) : le tremblement n'agit que
/// sur un calque vectoriel.
///
/// **Le tremblement par défaut** (Préférences > Dessin, `tremblement::Defaut`) s'ajoute ici, au rendu, en tête des effets d'un
/// calque vectoriel qui n'a **aucun** « Tremblement du trait » à lui (allumé ou non : un tremblement éteint à la main reste
/// éteint) : rien n'est écrit dans le document. Le bouton « fx » éteint du calque l'éteint aussi.
pub fn layer_steps(layer: &crate::document::Layer, frame: usize, rest: Option<usize>) -> Vec<FxStep> {
    // Bouton « fx » du calque éteint : aucun effet, la pile reste en place.
    if !layer.fx_enabled {
        return Vec::new();
    }
    let vector = layer.kind == crate::document::LayerKind::Vector;
    let mut steps = steps_at_with(&layer.fx, frame, rest, vector);
    if vector && !layer.folder && layer.effet.is_none() && !layer.fx.iter().any(|e| e.kind == EffectKind::Boil) {
        if let Some(step) = crate::tremblement::default_step(frame, rest) {
            steps.insert(0, step);
        }
    }
    steps
}

/// Un élément du plan dont la pile mêle des effets du trait (calculés en arrière-plan : ils lisent les
/// traits du dessin vectoriel, `fxtrait.rs`) et des effets qui ont une version sur la carte graphique
/// (`gpufx.rs`) : l'élément réduit aux effets du trait — dont le dessin dérivé devient le dessin d'origine
/// de la carte —, et les effets que la carte passe dessus, dans leur ordre. Rien pour une pile sans effet du
/// trait, sans autre effet, ou dont un autre effet n'a pas de passe ; rien non plus pour un remplissage
/// au griffonnage, un fond sous le trait ou une feuille flottante.
pub fn hybrid_split(item: &crate::render::PlanItem) -> Option<(crate::render::PlanItem, Vec<FxStep>)> {
    if item.ctg.is_some() || item.fond.is_some() || item.pre.is_some() {
        return None;
    }
    // (Seuls les effets qui lisent le dessin vectoriel passent d'abord : le liseré et l'encre qui bavent
    // sont des effets sur les pixels, qui gardent leur place dans la pile — et leur calcul sur processeur.)
    let (line, rest): (Vec<FxStep>, Vec<FxStep>) = item.fx.iter().cloned().partition(|s| s.kind.vector_stage());
    if line.is_empty() || !crate::gpufx::steps_supported(&rest) {
        return None;
    }
    Some((crate::render::PlanItem { fx: line, ..item.clone() }, rest))
}

/// Les mêmes effets sans le tremblement ni les effets du trait (le calque qu'on édite se montre tel
/// qu'il est : on dessine, on prend et on déplace ce qu'on voit).
pub fn without_boil(steps: Vec<FxStep>) -> Vec<FxStep> {
    steps.into_iter().filter(|s| !s.kind.vector_only() && !s.kind.vector_stage()).collect()
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
    // Tuile par tuile, ligne par ligne (les tuiles vides restent transparentes).
    let (tx0, ty0) = (x0 / TILE, y0 / TILE);
    let (tx1, ty1) = (x1.div_ceil(TILE), y1.div_ceil(TILE));
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            let idx = ty * raster.tiles_x + tx;
            let Some(tile) = raster.tiles[idx].as_ref() else { continue };
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            let (cx0, cx1) = (ox.max(x0), (ox + tw).min(x1));
            let (cy0, cy1) = (oy.max(y0), (oy + th).min(y1));
            for y in cy0..cy1 {
                let src = &tile[(y - oy) * TILE + (cx0 - ox)..(y - oy) * TILE + (cx1 - ox)];
                px[(y - y0) * w + (cx0 - x0)..(y - y0) * w + (cx1 - x0)].copy_from_slice(src);
            }
        }
    }
    Some(Region { x0, y0, w, h, px })
}

/// La zone remise dans une feuille vide de la taille de `like` (tuiles toutes transparentes
/// laissées vides).
pub(crate) fn into_raster(r: &Region, like: &Raster) -> Raster {
    let mut out = Raster::new(like.width, like.height);
    let (tx0, ty0) = (r.x0 / TILE, r.y0 / TILE);
    let (tx1, ty1) = ((r.x0 + r.w).div_ceil(TILE).min(out.tiles_x), (r.y0 + r.h).div_ceil(TILE));
    if tx0 >= tx1 || ty0 >= ty1 {
        return out;
    }
    let (width, height, tiles_x) = (out.width, out.height, out.tiles_x);
    // Une rangée de tuiles par morceau, réparties sur les cœurs.
    let rows = &mut out.tiles[ty0 * tiles_x..(ty1 * tiles_x).min(like.tiles.len())];
    crate::render::par_chunks(rows, tiles_x, |k, row| {
        let ty = ty0 + k;
        for tx in tx0..tx1 {
            let idx = ty * tiles_x + tx;
            let (ox, oy, tw, th) = crate::raster::tile_bounds(width, height, idx);
            // Ce que la zone couvre de cette tuile.
            let (cx0, cx1) = (ox.max(r.x0), (ox + tw).min(r.x0 + r.w));
            let (cy0, cy1) = (oy.max(r.y0), (oy + th).min(r.y0 + r.h));
            if cx0 >= cx1 || cy0 >= cy1 {
                continue;
            }
            let mut any = false;
            let mut tile = crate::raster::empty_tile();
            for y in cy0..cy1 {
                let src = &r.px[(y - r.y0) * r.w + (cx0 - r.x0)..(y - r.y0) * r.w + (cx1 - r.x0)];
                let dst = &mut tile[(y - oy) * TILE + (cx0 - ox)..(y - oy) * TILE + (cx1 - ox)];
                for (d, &c) in dst.iter_mut().zip(src) {
                    if c.a() != 0 {
                        any = true;
                        *d = c;
                    }
                }
            }
            if any {
                row[tx] = Some(tile);
            }
        }
    });
    out
}

/// Flou en boîte horizontal de rayon `r` sur chaque ligne de `px` (largeur `w`), hors champ
/// transparent.
fn box_rows(px: &mut [Color32], w: usize, r: usize) {
    if r == 0 || w == 0 {
        return;
    }
    let size = (2 * r + 1) as u32;
    // Division par `size` (arrondie au plus proche) par une multiplication : exacte pour les
    // sommes de ce flou (au plus 255 × 801 + 400).
    let recip = (1u64 << 32) / size as u64 + 1;
    let half = size / 2;
    let div = move |v: u32| (((v + half) as u64 * recip) >> 32) as u8;
    crate::render::par_chunks(px, w * 16, |_, rows| {
        let mut line = vec![Color32::TRANSPARENT; w];
        for row in rows.chunks_mut(w) {
            line.copy_from_slice(row);
            let mut acc = [0u32; 4];
            for c in line.iter().take(r.min(w - 1) + 1) {
                acc[0] += c.r() as u32;
                acc[1] += c.g() as u32;
                acc[2] += c.b() as u32;
                acc[3] += c.a() as u32;
            }
            for x in 0..w {
                row[x] = Color32::from_rgba_premultiplied(div(acc[0]), div(acc[1]), div(acc[2]), div(acc[3]));
                if x + r + 1 < w {
                    let c = line[x + r + 1];
                    acc[0] += c.r() as u32;
                    acc[1] += c.g() as u32;
                    acc[2] += c.b() as u32;
                    acc[3] += c.a() as u32;
                }
                if x >= r {
                    let c = line[x - r];
                    acc[0] -= c.r() as u32;
                    acc[1] -= c.g() as u32;
                    acc[2] -= c.b() as u32;
                    acc[3] -= c.a() as u32;
                }
            }
        }
    });
}

/// Division par `size` utilisée par `box_rows` (pour les tests : exacte comme `(v + size/2) / size`).
#[cfg(test)]
pub(crate) fn box_div_exact(size: u32) -> bool {
    let recip = (1u64 << 32) / size as u64 + 1;
    let half = size / 2;
    (0..=255 * size).all(|v| (((v + half) as u64 * recip) >> 32) as u32 == (v + half) / size)
}

/// Le tableau `px` (largeur `w`, hauteur `h`) transposé : `out[x * h + y] = px[y * w + x]`. Par
/// bandes de 16 colonnes, réparties sur les cœurs : les lignes de cache sont lues en entier.
fn transpose(px: &[Color32], w: usize, h: usize) -> Vec<Color32> {
    const BAND: usize = 16;
    let mut out = vec![Color32::TRANSPARENT; w * h];
    if w == 0 || h == 0 {
        return out;
    }
    crate::render::par_chunks(&mut out, h * BAND, |k, band| {
        let x0 = k * BAND;
        let n = band.len() / h;
        for y in 0..h {
            let src = &px[y * w + x0..y * w + x0 + n];
            for (i, &c) in src.iter().enumerate() {
                band[i * h + y] = c;
            }
        }
    });
    out
}

/// Rayons des trois flous en boîte qui approchent un flou gaussien d'écart type `sigma`.
pub(crate) fn boxes_for(sigma: f32) -> [usize; 3] {
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
    apply_scaled(raster, steps, 0)
}

/// Comme `apply`, sur un dessin réduit `1 << level` fois (aperçu pendant la lecture et les
/// gestes, `fxproxy.rs`) : les paramètres en pixels sont réduits d'autant. `level` 0 : exact.
pub fn apply_scaled(raster: &Raster, steps: &[FxStep], level: u8) -> Raster {
    apply_with(raster, None, steps, level)
}

/// Comme `apply_scaled`, avec le dessin vectoriel d'où vient `raster` : les effets du trait
/// (`fxtrait.rs`) le lisent et passent d'abord, quelle que soit leur place dans la pile ; les autres
/// effets suivent, dans l'ordre, sur l'image qu'ils ont faite. Sans dessin vectoriel (ou en aperçu
/// réduit, `level` > 0 : ils ne le supportent pas, `fxproxy::proxyable`), les effets du trait sont
/// laissés.
pub fn apply_with(raster: &Raster, vector: Option<&crate::vector::VectorDrawing>, steps: &[FxStep], level: u8) -> Raster {
    let mut cur: Option<Raster> = None;
    let on_vector = vector.is_some() && level == 0;
    if let (Some(v), 0) = (vector, level) {
        let line: Vec<FxStep> = steps.iter().filter(|s| s.kind.vector_stage()).cloned().collect();
        if !line.is_empty() {
            cur = Some(crate::fxtrait::render(v, raster.width, raster.height, &line));
        }
    }
    // Ce que les effets sur les pixels lisent hors de la feuille : les traits du dessin passés par les effets du trait, s'ils y ont laissé quelque chose
    // (`fxtrait::render` pose sur la feuille ce qu'il transforme) ; sinon le dessin, sauf si un effet du trait a voulu cacher des traits.
    let cache_des_traits = steps.iter().any(|s| matches!(s.kind, EffectKind::Write | EffectKind::Dust));
    let source_monde: Option<Raster> = match &cur {
        Some(c) if !c.outer.is_empty() || cache_des_traits => Some(c.clone()),
        _ => None,
    };
    // (Sans dessin vectoriel, les effets qui ne vivent que sur les traits sont laissés ; le trait sec, lui,
    // agit sur les pixels.)
    let sur_les_pixels: Vec<&FxStep> = steps.iter().filter(|s| !(s.kind.vector_stage() && (on_vector || s.kind.vector_only()))).collect();
    for s in &sur_les_pixels {
        // (Le premier effet lit le dessin lui-même : pas de copie d'avance.)
        let next = un_effet(cur.as_ref().unwrap_or(raster), s, level, (0, 0));
        cur = Some(next);
    }
    let mut out = cur.unwrap_or_else(|| raster.clone());
    if level == 0 {
        effets_hors_de_la_feuille(&mut out, source_monde.as_ref().unwrap_or(raster), &sur_les_pixels);
    }
    out
}

/// Marge autour de ce qui est dessiné hors de la feuille, et bande de la feuille lue le long de son bord (en tuiles) : ce qu'un effet peut étaler ou aller lire.
const MARGE_MONDE: i32 = 4;
/// Au-delà de ces pixels, le dessin hors de la feuille est gardé sans les effets qui lisent tout leur rectangle (flou, lueur…) : un point perdu loin de la feuille
/// leur coûtait 4 à 5 octets par pixel (507 Mo et 363 ms à 11 000 pixels de là). Rien n'est jamais retiré. Le tremblement et les réglages de couleur vont tuile par tuile : aucune borne.
const MONDE_DENSE_PIXELS: usize = 1 << 25;

/// Les effets ne connaissent que la feuille (`Raster::tiles`) : **ce qui est dessiné au-delà disparaissait de l'image dérivée**, et un calque à effets
/// (le tremblement par défaut, sur tout calque vectoriel) se voyait coupé net au bord de la feuille, au repos, partout où l'image dérivée sert : case,
/// Storyboard, export (partie 118). La feuille reste calculée comme toujours, au pixel près ; ce qui la dépasse l'est ici, sur une image à part, faite
/// de ce hors-feuille et de la bande de la feuille qui le touche, le tremblement gardant son champ de la feuille (le trait ne saute pas au bord). Les
/// effets réglés par rapport à la feuille (centre, pourcentages) ne s'y appliquent pas : ce qu'ils auraient déplacé reste à sa place, tel quel.
fn effets_hors_de_la_feuille(out: &mut Raster, base: &Raster, steps: &[&FxStep]) {
    if !base.has_beyond() {
        return;
    }
    let Some((tx0, ty0, tx1, ty1)) = base.drawn_tile_range() else { return };
    let (ox, oy) = (tx0 - MARGE_MONDE, ty0 - MARGE_MONDE);
    let (nx, ny) = ((tx1 - tx0 + 1 + 2 * MARGE_MONDE) as usize, (ty1 - ty0 + 1 + 2 * MARGE_MONDE) as usize);
    let (mut image, origine) = (Raster::new(nx * TILE, ny * TILE), (ox as i64 * TILE as i64, oy as i64 * TILE as i64));
    let (cols, rows) = (base.tiles_x as i32, base.tiles_y() as i32);
    let pres_du_bord = |tx: i32, ty: i32| tx < MARGE_MONDE || ty < MARGE_MONDE || tx >= cols - MARGE_MONDE || ty >= rows - MARGE_MONDE;
    for id in base.drawn_ids() {
        let (tx, ty) = base.key_of(id);
        if !base.in_window(tx, ty) || pres_du_bord(tx, ty) {
            image.tiles[(ty - oy) as usize * nx + (tx - ox) as usize] = base.slot(id).cloned();
        }
    }
    let assez_petite = image.width.saturating_mul(image.height) <= MONDE_DENSE_PIXELS;
    for s in steps.iter().filter(|s| s.kind.suit_la_toile() && (assez_petite || s.kind.par_tuile())) {
        image = un_effet(&image, s, 0, origine);
    }
    // Le tremblement seul de la feuille ne lit pas au-delà d'elle : à son bord, le trait aurait une encoche. Les tuiles du bord viennent donc d'ici, qui
    // voit des deux côtés, avec le même champ (le reste de la feuille lui est identique). Seulement pour un dessin qui déborde.
    let tremble_seul = matches!(steps, [s] if s.kind == EffectKind::Boil);
    let au_bord = |tx: i32, ty: i32| tx == 0 || ty == 0 || tx == cols - 1 || ty == rows - 1;
    for (k, tile) in image.tiles.into_iter().enumerate() {
        let (tx, ty) = (ox + (k % nx) as i32, oy + (k / nx) as i32);
        if tremble_seul && out.in_window(tx, ty) && au_bord(tx, ty) {
            out.replace_slot(out.id_of(tx, ty), tile);
            continue;
        }
        let Some(tile) = tile else { continue };
        // (Dans la feuille, le calcul de la feuille reste : seul ce qui dépasse d'une tuile de bord est pris.)
        out.absorb_beyond(tx, ty, tile);
    }
}

/// Un effet sur les pixels de `src`. `origine` : la place dans le monde du pixel (0, 0) de `src`, quand ce n'est pas la feuille (`effets_hors_de_la_feuille`).
fn un_effet(src: &Raster, s: &FxStep, level: u8, origine: (i64, i64)) -> Raster {
    use crate::fxfilters as f;
    {
        let values = crate::fxproxy::scaled_values(s.kind, &s.values, level);
        let v = &values;
        match s.kind {
            EffectKind::GaussianBlur => gaussian(src, v[0]),
            EffectKind::MotionBlur => motion(src, v[0], v[1]),
            EffectKind::BrightnessContrast => f::brightness_contrast(src, v[0], v[1]),
            EffectKind::Levels => f::levels(src, v[0], v[1], v[2]),
            EffectKind::HueSaturation => f::hue_saturation(src, v[0], v[1], v[2]),
            EffectKind::BlackWhite => f::black_white(src, v[0]),
            EffectKind::Invert => f::invert(src, v[0]),
            EffectKind::Posterize => f::posterize(src, v[0]),
            EffectKind::Threshold => f::threshold(src, v[0]),
            EffectKind::Sharpen => f::sharpen(src, v[0], v[1]),
            EffectKind::Noise => f::noise_scaled(src, v[0], level),
            EffectKind::Mosaic => f::mosaic(src, v[0]),
            EffectKind::DropShadow => f::drop_shadow(src, v[0], v[1], v[2], v[3]),
            EffectKind::Glow => f::glow(src, v[0], v[1]),
            EffectKind::Wave => crate::fxwarp::wave(src, v[0], v[1], v[2]),
            EffectKind::Ripple => crate::fxwarp::ripple(src, v[0], v[1], v[2], v[3]),
            EffectKind::Twirl => crate::fxwarp::twirl(src, v[0], v[1], v[2], v[3]),
            EffectKind::Pinch => crate::fxwarp::pinch(src, v[0], v[1], v[2], v[3]),
            EffectKind::Clouds => crate::fxwarp::clouds(src, v[0], v[1], v[2]),
            EffectKind::Lighting => crate::fxwarp::lighting(src, v[0], v[1], v[2], v[3]),
            EffectKind::Gradient => crate::fxwarp::gradient(src, v[0], v[1]),
            EffectKind::Boil => crate::tremblement::boil_depuis(src, v, level, origine),
            EffectKind::Flash => crate::fxcine::flash(src, v[0], [v[1], v[2], v[3]], v[4] >= 0.5),
            EffectKind::Impact => crate::fxcine::impact(src, v),
            EffectKind::Bloom => crate::fxcine::bloom(src, v[0], v[1], v[2]),
            EffectKind::Shake => crate::fxcine::shake(src, v[0], v[1], v[2], v[3], v[4]),
            EffectKind::Vignette => crate::fxcine::vignette(src, v[0], v[1], v[2], v[3], v[4], v[5], v[6]),
            EffectKind::Grain => crate::fxcine::grain(src, v[0], v[1], v[2], v.get(3).copied().unwrap_or(0.0), level),
            EffectKind::Aberration => crate::fxcine::aberration(src, v[0], v[1], v[2]),
            // Effets de matière sur les pixels (`fxmatiere.rs`).
            EffectKind::Outline => crate::fxmatiere::outline(src, v),
            EffectKind::Bleed => crate::fxmatiere::bleed(src, v),
            EffectKind::Dry => crate::fxmatiere::dry_pixels(src, v),
            // (Les effets du trait sont passés plus haut.)
            EffectKind::Write | EffectKind::Sketch | EffectKind::Breath | EffectKind::StrokeWave | EffectKind::Dashes | EffectKind::Dust | EffectKind::StrokeGradient | EffectKind::Nib => unreachable!("effet du trait"),
        }
    }
}

// ---------------------------------------------------------------------- cache des dessins dérivés

// Les dessins dérivés sont gardés par `fxcache.rs` (réserve en octets, calcul de fond).
static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Vide le cache des dessins dérivés (mesures et tests).
#[cfg(test)]
pub(crate) fn clear_cache() {
    crate::fxcache::clear();
}

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
/// au griffonnage, sinon un numéro propre à ce dessin, ces effets et ces sources.
pub fn shown_id(item: &crate::render::PlanItem) -> CelId {
    // Fond sous le trait (`fond.rs`) : un numéro propre au fond de ce dessin.
    if let Some(crate::render::FondPart::Forme(r)) = &item.fond {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        item.cel.hash(&mut h);
        "fond".hash(&mut h);
        r.hash(&mut h);
        return h.finish() | (1 << 63);
    }
    // Couleur unie (`couleurunie.rs`) : un numéro propre à la couleur.
    if let Some(crate::render::FondPart::Unie(c)) = &item.fond {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        "unie".hash(&mut h);
        c.hash(&mut h);
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

/// Le dessin d'un élément du plan tel qu'il s'affiche : effets (13i) et remplissage au griffonnage
/// (`griffonnage.rs`) compris.
///
/// Calcul bloquant et exact (export, vignettes enregistrées, tests), sauf en mode doux
/// (`fxcache::soft_scope`, vignettes de l'écran) : les effets non encore calculés sont alors
/// demandés au fil de fond et remplacés par leur dernier résultat. À l'écran (`differe`), un remplissage au
/// griffonnage qui n'est plus à jour est lui aussi demandé au fil de fond : le remplissage d'avant reste affiché, ou
/// rien tant qu'il n'y en a pas (jamais les griffonnages eux-mêmes, ni un calcul sur le fil de l'interface).
pub fn shown<'a>(doc: &'a Document, item: &crate::render::PlanItem) -> Option<Shown<'a>> {
    if crate::fxcache::soft_active() {
        // (Une vignette passe après l'image affichée.)
        let d = display_with(doc, item, 0, THUMB_PRIO)?;
        if !d.ready {
            crate::fxcache::mark_incomplete();
        }
        return Some(d.cel);
    }
    shown_blocking(doc, item)
}

fn shown_blocking<'a>(doc: &'a Document, item: &crate::render::PlanItem) -> Option<Shown<'a>> {
    // Remplissage au griffonnage affiché à l'écran : jamais calculé sur le fil de l'interface.
    if item.ctg.is_some() && item.fond.is_none() && DIFFERE.with(std::cell::Cell::get) && doc.cels.contains_key(&item.cel) {
        return display_with(doc, item, 0, 0).map(|d| d.cel);
    }
    match &item.fond {
        None => shown_with(doc, item.cel, &item.fx, item.ctg.as_ref()),
        Some(crate::render::FondPart::Forme(r)) => fond_shown(doc, item, r),
        // Le repère d'un calque d'effet non résolu (`calqueffet::resolve`) ne montre rien.
        Some(crate::render::FondPart::Reglage(_)) => None,
        Some(crate::render::FondPart::Unie(c)) => Some(Shown::Fx(unie_cel(*c))),
    }
}

/// Le dessin d'une couleur unie (`couleurunie.rs`) : un seul pixel de cette couleur, que la composition étire sur toute la feuille (comme
/// l'aperçu réduit d'un effet) ; gardé par couleur (les huit dernières). Aucun bitmap de la taille de la feuille.
pub fn unie_cel(c: [u8; 3]) -> Arc<Cel> {
    static GARDEES: Mutex<Vec<([u8; 3], Arc<Cel>)>> = Mutex::new(Vec::new());
    let mut v = GARDEES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = v.iter().position(|(couleur, _)| *couleur == c) {
        let e = v.remove(k);
        let cel = e.1.clone();
        v.push(e);
        return cel;
    }
    let mut raster = Raster::new(1, 1);
    raster.tile_mut(0).fill(Color32::from_rgb(c[0], c[1], c[2]));
    let cel = Arc::new(Cel { raster, vector: None, version: 1, fond: None });
    v.push((c, cel.clone()));
    if v.len() > 8 {
        v.remove(0);
    }
    cel
}

/// Un dessin pour l'affichage : ce qu'il faut montrer maintenant, sous quel numéro il est gardé
/// (celui de l'élément du plan s'il est prêt ; un autre, celui du dernier résultat, ou le dessin
/// lui-même, tant que le fil de fond calcule), et s'il est prêt.
pub struct Display<'a> {
    pub cel: Shown<'a>,
    pub id: CelId,
    pub ready: bool,
}

/// Numéro du dessin dérivé de `item` pour l'aperçu de niveau `level` (0 : pleine résolution,
/// `shown_id`).
fn display_id(item: &crate::render::PlanItem, level: u8) -> CelId {
    if level == 0 {
        return shown_id(item);
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    shown_id(item).hash(&mut h);
    "aperçu".hash(&mut h);
    level.hash(&mut h);
    h.finish() | (1 << 63)
}

/// Ce que l'affichage demande pour `item` au niveau `level` : le dessin d'origine, et le niveau
/// réellement servi (l'aperçu n'est admis que sans transformation, pour des effets qui le suivent).
/// Un remplissage au griffonnage (`item.ctg`) se demande aussi, toujours en pleine résolution : son dessin d'origine est
/// le dessin de griffonnages. Rien si l'élément n'est pas un dessin passé par des effets seuls (fond, vidéo : calcul
/// bloquant d'avant).
fn wanted<'a>(doc: &'a Document, item: &crate::render::PlanItem, level: u8) -> Option<(&'a Cel, u8)> {
    if item.fond.is_some() {
        return None;
    }
    if item.ctg.is_some() {
        return doc.cels.get(&item.cel).map(|src| (src, 0));
    }
    if item.fx.is_empty() {
        return None;
    }
    let src = doc.cels.get(&item.cel)?;
    let level = if level > 0 && item.transform.is_none() && item.pre.is_none() && crate::fxproxy::proxyable(&item.fx) { level } else { 0 };
    Some((src, level))
}

/// Le dessin de `item` pour l'écran, sans jamais calculer d'effet sur le fil appelant : prêt, ou
/// demandé au fil de fond et remplacé en attendant par le dernier résultat du même dessin, ou le
/// dessin sans effet (puis l'écran est redessiné quand le résultat arrive). `level` : aperçu
/// réduit `1 << level` fois (0 : pleine résolution).
pub fn display<'a>(doc: &'a Document, item: &crate::render::PlanItem, level: u8) -> Option<Display<'a>> {
    display_with(doc, item, level, 0)
}

/// Priorité des calculs demandés pour les vignettes (après l'image affichée et les suivantes).
const THUMB_PRIO: u32 = 100;

pub(crate) fn display_with<'a>(doc: &'a Document, item: &crate::render::PlanItem, level: u8, prio: u32) -> Option<Display<'a>> {
    let Some((src, level)) = wanted(doc, item, level) else {
        return Some(Display { cel: shown_blocking(doc, item)?, id: shown_id(item), ready: true });
    };
    if let Some((id, cel)) = lookup(doc, item, src, level) {
        return Some(Display { cel: Shown::Fx(cel), id, ready: true });
    }
    // (Pendant un geste ou la lecture, les vignettes attendent : le calcul de fond sert l'image
    // affichée ; elles se refont à la fin du geste, `draft_level`.)
    if prio < THUMB_PRIO || !draft_active() {
        let (id, version) = (display_id(item, level), version_of(doc, item, src));
        // (Un remplissage déjà demandé n'est pas redemandé — ses sources ne sont pas recopiées à chaque image d'écran —,
        // mais sa demande est renouvelée : sans cela elle vieillirait et serait abandonnée.)
        if !(item.ctg.is_some() && crate::fxcache::renew(id, version, prio)) {
            crate::fxcache::request(job_for(doc, item, src, id, level, prio));
        }
    }
    if item.ctg.is_some() {
        GRIFFON_ATTENDU.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // (Un tremblement n'a rien à gagner d'un résultat d'avant la dernière retouche du dessin : il
    // ferait disparaître le trait qu'on vient de poser ; il montre alors le dessin tel quel.
    // L'autre variante du même dessin, elle, est un bon remplaçant.)
    let boil = item.fx.iter().any(|s| s.kind.vector_only() || s.kind.vector_stage());
    // (Un aperçu réduit n'a rien hors de la feuille : il ne remplace pas un dessin qui en a, le trait y serait coupé, partie 118.)
    let sans_debord = |c: &Cel| c.raster.width != src.raster.width && !src.raster.outer.is_empty();
    Some(match crate::fxcache::fallback(item.cel) {
        Some((last, cel)) if (!boil || cel.version == src.version) && !sans_debord(&cel) => Display { cel: Shown::Fx(cel), id: last, ready: false },
        // Un remplissage qui n'a jamais été calculé ne montre rien (surtout pas ses griffonnages, des taches de couleur).
        _ if item.ctg.is_some() => Display { cel: Shown::Fx(cel_vide(src.raster.width, src.raster.height)), id: display_id(item, level), ready: false },
        _ => Display { cel: Shown::Plain(src), id: item.cel, ready: false },
    })
}

/// Un dessin vide de `w` × `h` pixels, gardé : ce que montre un remplissage au griffonnage en attendant son premier calcul. Un par
/// taille (les quatre dernières demandées) : deux projets de tailles différentes, ouverts ensemble, ne se volaient pas le seul dessin
/// gardé, et un dessin vide qui change de numéro à chaque image se prenait pour un remplissage arrivé.
pub(crate) fn cel_vide(w: usize, h: usize) -> Arc<Cel> {
    static VIDES: Mutex<Vec<Arc<Cel>>> = Mutex::new(Vec::new());
    let mut v = VIDES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = v.iter().position(|c| c.raster.width == w && c.raster.height == h) {
        // (Le dernier servi à la fin : ce sont les plus anciens qui partent.)
        let c = v.remove(k);
        v.push(c.clone());
        return c;
    }
    let c = Arc::new(Cel { raster: Raster::new(w, h), vector: None, version: 0, fond: None });
    v.push(c.clone());
    if v.len() > 4 {
        v.remove(0);
    }
    c
}

/// Un remplissage au griffonnage affiché attendait le fil de fond.
static GRIFFON_ATTENDU: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Le nombre de calculs de fond finis la dernière fois qu'on a regardé (`griffon_poll`).
static GRIFFON_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// À chaque image d'écran : un remplissage au griffonnage attendait le fil de fond, qui a fini un calcul depuis — l'image
/// composée par le processeur est à refaire (la carte graphique relit les dessins dérivés à chaque image). Vrai alors.
pub fn griffon_poll() -> bool {
    let generation = crate::fxcache::generation();
    if GRIFFON_GEN.swap(generation, std::sync::atomic::Ordering::Relaxed) == generation {
        return false;
    }
    GRIFFON_ATTENDU.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// La version d'un remplissage au griffonnage : celles du dessin de griffonnages (`version`) et des dessins de ses sources.
fn griffon_version(doc: &Document, version: u64, input: &crate::griffonnage::GriffonInput) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    version.hash(&mut h);
    for s in &input.sources {
        doc.cels.get(s).map_or(0, |c| c.version).hash(&mut h);
    }
    h.finish()
}

/// La version du dessin dérivé de `item` : celle de son dessin d'origine `src`, ou, pour un remplissage au griffonnage,
/// celle qui compte aussi ses sources.
fn version_of(doc: &Document, item: &crate::render::PlanItem, src: &Cel) -> u64 {
    match &item.ctg {
        None => src.version,
        Some(input) => griffon_version(doc, src.version, input),
    }
}

/// Le dessin dérivé de `item` s'il est gardé au niveau `level`, sinon en pleine résolution (déjà
/// calculée au repos : plus nette, autant la montrer), avec son numéro.
fn lookup(doc: &Document, item: &crate::render::PlanItem, src: &Cel, level: u8) -> Option<(CelId, Arc<Cel>)> {
    let both = [level, 0];
    let levels = if level == 0 { &both[..1] } else { &both[..] };
    let version = version_of(doc, item, src);
    levels.iter().find_map(|&l| {
        let id = display_id(item, l);
        crate::fxcache::get(id, version).map(|c| (id, c))
    })
}

fn job_for(doc: &Document, item: &crate::render::PlanItem, src: &Cel, id: CelId, level: u8, prio: u32) -> crate::fxcache::Job {
    // Un remplissage au griffonnage : les traits de ses sources partent avec la demande.
    let griffon = item.ctg.as_ref().map(|input| {
        let lines = input.sources.iter().filter_map(|s| doc.cels.get(s)).map(|c| Arc::new(c.raster.clone())).collect();
        (lines, input.gap)
    });
    crate::fxcache::Job {
        id,
        version: version_of(doc, item, src),
        griffon,
        src: crate::fxcache::source(item.cel, src),
        vector: if item.fx.iter().any(|s| s.kind.vector_stage()) { crate::fxcache::vector_source(item.cel, src) } else { None },
        steps: item.fx.clone(),
        level,
        prio,
        epoch: crate::fxcache::epoch(),
        origin: item.cel,
        seq: 0,
        world: crate::fxcache::world(),
        base: None,
    }
}

/// Un autre projet s'ouvre : les dessins dérivés de l'ancien sont oubliés (ses dessins portent les
/// mêmes numéros et les mêmes versions que ceux du nouveau).
pub fn forget_all() {
    crate::fxcache::forget_all();
    COVERS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// Comme `forget_all`, mais rend les dessins dérivés oubliés au lieu de les lâcher ici : l'appelant les fait rendre sur
/// un autre fil (`lacher.rs`, autre projet qui prend la place).
pub(crate) fn forget_all_rendre() -> crate::fxcache::Reserve {
    let r = crate::fxcache::mettre_de_cote();
    COVERS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    r
}

/// Demande d'avance les dessins dérivés de `items` (les images qui viennent, en lecture),
/// `prio` : leur rang dans l'avenir (1 : la suivante). Rien pour ce qui est déjà gardé.
pub fn prefetch(doc: &Document, items: &[crate::render::PlanItem], prio: u32, level: u8) {
    for item in items {
        let Some((src, level)) = wanted(doc, item, level) else { continue };
        if lookup(doc, item, src, level).is_some() {
            continue;
        }
        crate::fxcache::request(job_for(doc, item, src, display_id(item, level), level, prio));
    }
}

/// Le dessin dérivé de `item` s'il est déjà calculé (pour l'envoyer d'avance à la carte
/// graphique), avec le numéro sous lequel il est gardé.
pub fn ready(doc: &Document, item: &crate::render::PlanItem, level: u8) -> Option<(CelId, Arc<Cel>)> {
    let (src, level) = wanted(doc, item, level)?;
    lookup(doc, item, src, level)
}

/// Début d'une image d'écran : l'horloge des demandes avance, et le fil de fond sait quel écran
/// redessiner quand il a fini.
pub fn begin_frame(ctx: &eframe::egui::Context) {
    crate::fxcache::begin_frame(ctx);
    // L'aperçu vient de finir : les vignettes qui ont attendu se refont.
    let active = draft_active();
    if DRAFT_ON.swap(active, std::sync::atomic::Ordering::Relaxed) && !active {
        crate::fxcache::bump_generation();
    }
}

/// Dernier mouvement de l'image affichée ou d'un curseur d'effet (millisecondes depuis le
/// lancement) ; l'aperçu dure `DRAFT_MS` après lui.
static DRAFT_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LAST_SHOWN: Mutex<Option<(usize, usize)>> = Mutex::new(None);
const DRAFT_MS: u64 = 250;

pub(crate) fn now_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let real = START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64;
    #[cfg(test)]
    let real = real + CLOCK_SKEW.load(std::sync::atomic::Ordering::Relaxed);
    real
}

/// Essais : avance l'horloge de l'aperçu sans attendre (un essai qui dort 330 ms pour passer les
/// 250 ms de l'aperçu dépend de l'ordonnancement de la machine ; celui-ci non).
#[cfg(test)]
static CLOCK_SKEW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn advance_clock(ms: u64) {
    CLOCK_SKEW.fetch_add(ms, std::sync::atomic::Ordering::Relaxed);
}

/// Essais : l'aperçu d'une application qui vient de tourner (défilement, lecture : 250 ms
/// d'aperçu devant elle) ne déborde pas sur l'essai suivant (`fxcache::monde::ecrire`).
#[cfg(test)]
pub(crate) fn reset_draft() {
    DRAFT_UNTIL.store(0, std::sync::atomic::Ordering::Relaxed);
    DRAFT_ON.store(false, std::sync::atomic::Ordering::Relaxed);
    *LAST_SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Un curseur d'effet bouge : l'image passe en aperçu le temps du geste.
pub fn note_interaction(ctx: &eframe::egui::Context) {
    DRAFT_UNTIL.store(now_ms() + DRAFT_MS, std::sync::atomic::Ordering::Relaxed);
    // (Quand le geste finit, l'image se redessine en pleine résolution, même sans autre événement.)
    ctx.request_repaint_after(std::time::Duration::from_millis(DRAFT_MS + 50));
}

/// Niveau d'aperçu à utiliser pour afficher l'image (`panel`, `frame`) à `zoom` pixels d'écran par
/// pixel de la feuille : 0 (pleine résolution) au repos, réduit pendant la lecture, quand l'image
/// affichée vient de changer (défilement) et le temps d'un geste sur un effet.
pub fn draft_level(ctx: &eframe::egui::Context, panel: usize, frame: usize, playing: bool, zoom: f32) -> u8 {
    let now = now_ms();
    let moved = {
        let mut last = LAST_SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        let moved = *last != Some((panel, frame));
        *last = Some((panel, frame));
        moved
    };
    if playing || moved {
        DRAFT_UNTIL.store(now + DRAFT_MS, std::sync::atomic::Ordering::Relaxed);
    }
    if now < DRAFT_UNTIL.load(std::sync::atomic::Ordering::Relaxed) {
        // Une fois le mouvement fini, l'image se redessine en pleine résolution.
        ctx.request_repaint_after(std::time::Duration::from_millis(DRAFT_MS + 50));
        crate::fxproxy::level_with_floor(zoom, DRAFT_MIN.load(std::sync::atomic::Ordering::Relaxed))
    } else {
        0
    }
}

/// Niveau d'aperçu minimal pendant un mouvement (Aide > Diagnostic des performances, « effets en
/// aperçu réduit ») : 0, celui du zoom seul ; 1, au moins la moitié de la résolution même zoomé.
static DRAFT_MIN: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_draft_min(level: u8) {
    DRAFT_MIN.store(level.min(2), std::sync::atomic::Ordering::Relaxed);
}

static DRAFT_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// L'image est-elle en aperçu (lecture, défilement, geste sur un effet) ?
fn draft_active() -> bool {
    now_ms() < DRAFT_UNTIL.load(std::sync::atomic::Ordering::Relaxed)
}

/// L'image est-elle en aperçu ? (Pour les calques d'effets, `calqueffet.rs` : leur dessin est celui de
/// toute l'image, il passe un cran plus bas que les effets d'un calque pendant l'aperçu.)
pub(crate) fn drafting() -> bool {
    draft_active()
}

/// Formes de fond gardées (`fond::silhouette`, pour refaire le fond tuile par tuile pendant qu'on
/// peint dedans) : numéro du fond → version du dessin, forme, dernier usage.
type Cover = (u64, Arc<crate::fond::Couverture>, u64);
static COVERS: Mutex<BTreeMap<CelId, Cover>> = Mutex::new(BTreeMap::new());
const COVERS_MAX: usize = 16;

/// Essais : pose une forme de fond gardée, comme celle qu'un dessin laisse après avoir été vu.
#[cfg(test)]
pub(crate) fn garder_une_forme_pour_essai(id: CelId, version: u64) {
    COVERS.lock().unwrap_or_else(|e| e.into_inner()).insert(id, (version, Arc::new(crate::fond::Couverture::depuis_feuille(4, 4, vec![255; 16])), 0));
}

/// Essais : combien de formes de fond sont gardées.
#[cfg(test)]
pub(crate) fn formes_de_fond_gardees() -> usize {
    COVERS.lock().unwrap_or_else(|e| e.into_inner()).len()
}

/// La forme du fond `id` du dessin `src` (calculée une fois par version du dessin).
fn cover_of(id: CelId, src: &Cel, r: &crate::fond::FondReglages, tick: u64) -> Arc<crate::fond::Couverture> {
    let mut covers = COVERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = covers.get_mut(&id).filter(|e| e.0 == src.version) {
        e.2 = tick;
        return e.1.clone();
    }
    // (Toile infinie : la forme suit le trait partout où il est, hors de la feuille aussi.)
    let cover = Arc::new(crate::fond::silhouette_monde(&src.raster, r));
    covers.insert(id, (src.version, cover.clone(), tick));
    if covers.len() > COVERS_MAX {
        if let Some(old) = covers.iter().min_by_key(|(_, e)| e.2).map(|(k, _)| *k) {
            covers.remove(&old);
        }
    }
    cover
}

/// Le fond sous le trait du dessin de `item` : sa forme, moins ce qu'on en a coupé, et ce qu'on y
/// a peint (calculé une fois, puis gardé tant que le dessin, sa strate Fond et les réglages ne
/// changent pas).
fn fond_shown<'a>(doc: &'a Document, item: &crate::render::PlanItem, r: &crate::fond::FondReglages) -> Option<Shown<'a>> {
    let src = doc.cels.get(&item.cel)?;
    let id = shown_id(item);
    let version = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        src.version.hash(&mut h);
        src.fond.as_ref().map_or(0, |s| s.peinture.version + 1).hash(&mut h);
        h.finish()
    };
    let differe = DIFFERE.with(std::cell::Cell::get);
    if let Some((gardee, cel)) = crate::fxcache::get_any(id) {
        if gardee == version {
            // Revenu à une version déjà prête (annulation) : le fond attendu ne vaut plus.
            if let Some(fil) = FIL.get() {
                let mut fil = fil.lock().unwrap_or_else(|e| e.into_inner());
                if fil.pending_version(id).is_some_and(|p| p != version) {
                    fil.cancel(id);
                }
            }
            return Some(Shown::Fx(cel));
        }
        // À l'écran : le fond d'avant en attendant le nouveau (calculé en arrière-plan).
        if differe {
            demander_fond(id, version, src, r);
            return Some(Shown::Fx(cel));
        }
    }
    // Calcul direct (export, enregistrement, premier affichage) : ce qu'on attendait ne vaut plus. (Pas pour un
    // fil d'export : sa copie du projet n'est pas celle que l'écran attend.)
    if !crate::fxcache::privee_active() {
        if let Some(fil) = FIL.get() {
            fil.lock().unwrap_or_else(|e| e.into_inner()).cancel(id);
        }
    }
    let derived = crate::fxcache::get_or_compute(id, version, None, || {
        let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (w, h) = (src.raster.width, src.raster.height);
        let cover = cover_of(id, src, r, tick);
        let strate = src.fond.as_deref();
        let raster = crate::fond::compose_monde(&cover, w, h, r.couleur, strate.map(|s| &s.peinture.raster), strate.map(|s| &s.coupe));
        Cel { raster, vector: None, version, fond: None }
    });
    Some(Shown::Fx(derived))
}

// ---------------------------------------------------------------------- fond en arrière-plan

thread_local! {
    static DIFFERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

static FIL: std::sync::OnceLock<Mutex<crate::fondfil::FondFil>> = std::sync::OnceLock::new();
/// Compte les fonds adoptés (pour rafraîchir ce qui en dépend : vignettes, image composée).
static FOND_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Exécute `f` en laissant les fonds se calculer en arrière-plan (fil de l'interface) : un fond
/// qui n'est plus à jour montre sa version d'avant, et la nouvelle le remplace à son arrivée
/// (`fond_poll`). Hors de là, partout (export, enregistrement, essais), le fond est exact et
/// calculé tout de suite.
pub fn differe<R>(f: impl FnOnce() -> R) -> R {
    struct Garde(bool);
    impl Drop for Garde {
        fn drop(&mut self) {
            DIFFERE.with(|d| d.set(self.0));
        }
    }
    let _garde = Garde(DIFFERE.with(|d| d.replace(true)));
    f()
}

/// Demande en arrière-plan le fond `id` de la version `version` du dessin `src` (sauf si c'est
/// déjà demandé).
fn demander_fond(id: CelId, version: u64, src: &Cel, r: &crate::fond::FondReglages) {
    let forme = COVERS.lock().unwrap_or_else(|e| e.into_inner()).get(&id).filter(|e| e.0 == src.version).map(|e| e.1.clone());
    let mut fil = FIL.get_or_init(|| Mutex::new(crate::fondfil::FondFil::new())).lock().unwrap_or_else(|e| e.into_inner());
    if fil.pending_version(id) == Some(version) {
        return;
    }
    let strate = src.fond.as_deref();
    fil.submit(crate::fondfil::Demande {
        id,
        version,
        src_version: src.version,
        reglages: *r,
        width: src.raster.width,
        height: src.raster.height,
        trait_: forme.is_none().then(|| src.raster.clone()),
        forme,
        peinture: strate.map(|s| s.peinture.raster.clone()),
        coupe: strate.map(|s| s.coupe.clone()),
    });
}

/// Adopte les fonds arrivés et encore valables ; vrai si l'un d'eux remplace un fond affiché
/// (il faut alors redessiner).
pub fn fond_poll() -> bool {
    let Some(fil) = FIL.get() else { return false };
    let done = fil.lock().unwrap_or_else(|e| e.into_inner()).poll();
    if done.is_empty() {
        return false;
    }
    for d in done {
        let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        {
            let mut covers = COVERS.lock().unwrap_or_else(|e| e.into_inner());
            covers.insert(d.id, (d.src_version, d.forme, tick));
            if covers.len() > COVERS_MAX {
                if let Some(old) = covers.iter().min_by_key(|(_, e)| e.2).map(|(k, _)| *k) {
                    covers.remove(&old);
                }
            }
        }
        let cel = Arc::new(Cel { raster: d.raster, vector: None, version: d.version, fond: None });
        crate::fxcache::put(d.id, d.version, cel);
    }
    FOND_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    true
}

/// Un fond est-il en cours de calcul ?
pub fn fond_pending() -> bool {
    FIL.get().is_some_and(|f| f.lock().unwrap_or_else(|e| e.into_inner()).has_pending())
}

/// Autre projet : plus rien à attendre, et les dessins dérivés gardés (dont les fonds, qu'on
/// montre en attendant le nouveau) ne sont plus ceux de ce projet.
pub fn fond_cancel_all() {
    if let Some(f) = FIL.get() {
        f.lock().unwrap_or_else(|e| e.into_inner()).cancel_all();
    }
    forget_all();
}

/// Le projet affiché s'en va (onglets de projets) : les fonds en calcul sont abandonnés, les formes de
/// fond gardées vidées, et les dessins dérivés rangés dans la réserve rendue — l'onglet les reprend à
/// son retour (`fxcache::rendre`), sans rien recalculer.
pub(crate) fn mettre_de_cote() -> crate::fxcache::Reserve {
    if let Some(f) = FIL.get() {
        f.lock().unwrap_or_else(|e| e.into_inner()).cancel_all();
    }
    COVERS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    crate::fxcache::mettre_de_cote()
}

/// Nombre de fonds adoptés depuis le début.
pub fn fond_epoch() -> u64 {
    FOND_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
}

/// Essais : comme si un fond venait d'être adopté (`fond_poll`), pour un dessin d'ailleurs.
#[cfg(test)]
pub(crate) fn fond_adopte_ailleurs() {
    FOND_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Pendant qu'on peint ou qu'on gomme dans la strate Fond du dessin `cel` (fond aux réglages
/// `r`) : le fond affiché refait sur les tuiles `tiles` seulement, sans changer de version, pour
/// le voir en direct (la version change au lâcher, et tout est refait).
pub fn fond_patch(doc: &Document, cel: CelId, r: &crate::fond::FondReglages, tiles: &[usize]) {
    let Some(src) = doc.cels.get(&cel) else { return };
    let item = crate::render::PlanItem {
        cel,
        opacity: 255,
        tint: None,
        transform: None,
        blend: Default::default(),
        fx: Vec::new(),
        ctg: None,
        fond: Some(crate::render::FondPart::Forme(*r)),
        pre: None,
    };
    let id = shown_id(&item);
    let Some(cover) = COVERS.lock().unwrap_or_else(|e| e.into_inner()).get(&id).filter(|e| e.0 == src.version).map(|e| e.1.clone()) else { return };
    crate::fxcache::update(id, |old| {
        let mut raster = old.raster.clone();
        let strate = src.fond.as_deref();
        let n = raster.tiles.len();
        // (Les tuiles de la feuille par leur rang, celles d'hors de la feuille par leur numéro codé : on peint dans l'aplat
        // partout.)
        for &idx in tiles.iter().filter(|&&i| crate::raster::is_outer_id(i) || i < n) {
            crate::fond::compose_tuile_monde(&mut raster, idx, &cover, r.couleur, strate.map(|s| &s.peinture.raster), strate.map(|s| &s.coupe));
        }
        Arc::new(Cel { raster, vector: None, version: old.version, fond: None })
    });
}

/// Le dessin `cel`, rempli d'après `ctg` (dessins des sources, trous) s'il est un dessin de griffonnages
/// (`griffonnage.rs`), puis passé par les effets `steps` (calculé une fois, puis gardé tant qu'aucun de ces dessins ne
/// change). Calcul bloquant : à l'écran, `display_with` passe par le fil de fond.
pub fn shown_with<'a>(doc: &'a Document, cel: CelId, steps: &[FxStep], ctg: Option<&crate::griffonnage::GriffonInput>) -> Option<Shown<'a>> {
    // Pas un dessin du projet : le dessin dérivé d'un calque d'effet (`calqueffet.rs`), ou une image
    // de calque vidéo (13k), lue à la demande.
    let Some(src) = doc.cels.get(&cel) else {
        return crate::calqueffet::derived(cel).map(Shown::Fx).or_else(|| crate::video::shown(doc, cel).map(Shown::Fx));
    };
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
        pre: None,
    };
    let id = shown_id(&item);
    // Version : celle du dessin, et celles des sources du remplissage.
    let version = match ctg {
        None => src.version,
        Some(input) => griffon_version(doc, src.version, input),
    };
    let derived = crate::fxcache::get_or_compute(id, version, ctg.is_none().then_some(cel), || {
        let raster = match ctg {
            // (Sans remplissage, `apply` lit le dessin lui-même : pas de copie.)
            None => apply_with(&src.raster, src.vector.as_ref(), steps, 0),
            Some(input) => {
                let rasters: Vec<&Raster> = input.sources.iter().filter_map(|s| doc.cels.get(s)).map(|c| &c.raster).collect();
                apply(&crate::griffonnage::fill(&rasters, &src.raster, input.gap as usize), steps)
            }
        };
        Cel { raster, vector: None, version, fond: None }
    });
    Some(Shown::Fx(derived))
}
