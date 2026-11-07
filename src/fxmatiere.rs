//! Effets de matière du trait (Anthony, 2026-10-20 : « une deuxième série d'effets qui agissent sur la
//! matière du trait »). La suite de `fxtrait.rs` : même famille d'effets d'un calque (réglages enregistrés
//! avec le calque, clés, copier-coller, calcul en arrière-plan, cache des dessins dérivés, export et
//! vignettes), cinq effets de plus.
//!
//! - **Liseré** : un contour d'une autre couleur autour du dessin (couleur, largeur, douceur, opacité ;
//!   derrière le trait, ou en anneau seulement).
//! - **Trait sec** : des trous et un bord qui s'effrite, comme une brosse presque sèche.
//! - **Encre qui bave** : un bord irrégulier qui bave un peu, comme de l'encre sur du papier buvard.
//! - **Dégradé le long du trait** : la couleur change du début à la fin de chaque trait (ou sur tout le
//!   dessin, dans l'ordre des traits), avec un défilement animable.
//! - **Changer de plume** : les traits sont redessinés avec une autre plume (n'importe quel réglage de
//!   pinceau), sans toucher aux traits d'origine ; « Appliquer pour de bon » les remplace vraiment.
//!
//! # Choix de conception
//!
//! - **Vectoriel et bitmap.** Le liseré et l'encre qui bave sont des opérations **sur les pixels** du
//!   calque : ils marchent donc sur les deux sortes de calques, sans différence. Le trait sec a deux
//!   formes : sur un calque **vectoriel**, les trous suivent chaque trait (abscisse le long du trait,
//!   hasard par trait) et le bord s'effrite en stries parallèles au trait ; sur un calque **bitmap**, une
//!   forme simplifiée — des plages creuses d'un bruit à basse fréquence (taille réglée par la longueur
//!   des trous) et le même bruit sur la lisière de l'encre (distance au bord). Le dégradé et le
//!   changement de plume ont besoin des traits (abscisse, tracé, appuis) : calques vectoriels seulement,
//!   grisés avec la raison sur un bitmap.
//! - **Distance exacte au bord.** Le liseré et l'encre se règlent en pixels : ils lisent une distance
//!   euclidienne exacte au bord de l'encre (transformée de Felzenszwalb et Huttenlocher, deux passes
//!   séparables, réparties sur les cœurs), pas un flou seuillé. Le bord de l'encre est le contour à 25 %
//!   de couverture ; la douceur du liseré est une chute d'au moins un pixel.
//! - **Déterministe.** Le hasard est un hachage entier de (graine, position, trait, variante) : jamais
//!   d'horloge, jamais de pixel qui dépende du découpage en tuiles. Même image, mêmes pixels, sur un fil ou
//!   plusieurs.
//! - **Sans latence.** Rien sur le fil de l'interface : ces effets passent par le même calcul de fond que
//!   ceux du trait (`fxcache.rs`, dessins dérivés), à l'écran comme à l'export et dans les vignettes. Le
//!   changement de plume ne rejoue le moteur de brosses qu'une fois par trait et par plume (les traits
//!   redessinés sont gardés, `NibCtx::restyle`) : le défilement du dégradé ou le trait sec animé ne
//!   rejouent rien.
//! - **Pas de passe sur la carte graphique** (décision de cette série, voir `docs/PERFORMANCE.md` §14) : une
//!   dilatation exacte par un disque n'est pas séparable, et un résultat à moins de deux niveaux du calcul
//!   sur processeur demanderait le même algorithme des deux côtés (propagation de distance en plusieurs
//!   passes). Les piles qui en contiennent restent un calcul de fond.
//! - **Une plume = un nom de réglage.** L'effet « Changer de plume » enregistre un numéro de 24 bits calculé
//!   d'après le nom du réglage (`name_id`), pas son rang dans la bibliothèque : ajouter ou ranger des
//!   brosses ne change pas le choix. La bibliothèque publie ses réglages à l'effet (`publish`).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use eframe::egui::Color32;

use crate::brush::BrushSettings;
use crate::fx::{Effect, EffectKind, Param};
use crate::fxtrait::{at, hash, p, scaled, smooth, unit, Dense, Geo};
use crate::raster::Raster;
use crate::vector::{Element, VPoint, VectorDrawing};

// ---------------------------------------------------------------------- les réglages

/// Rang des réglages du liseré (`EffectKind::Outline`).
pub const OUTLINE_WIDTH: usize = 0;
pub const OUTLINE_SOFT: usize = 1;
pub const OUTLINE_OPACITY: usize = 2;
/// Rouge, vert, bleu : trois réglages à la suite.
pub const OUTLINE_RGB: usize = 3;
pub const OUTLINE_BEHIND: usize = 6;

pub const DRY_FREQ: usize = 0;
pub const DRY_LENGTH: usize = 1;
pub const DRY_DEPTH: usize = 2;
pub const DRY_ROUGH: usize = 3;
pub const DRY_SEED: usize = 4;
pub const DRY_EVERY: usize = 5;
pub const DRY_VARIANTS: usize = 6;
/// La variante montrée, ajoutée par `fx::steps_at`.
pub const DRY_VARIANT: usize = 7;

pub const BLEED_SPREAD: usize = 0;
pub const BLEED_IRREG: usize = 1;
pub const BLEED_DARK: usize = 2;
pub const BLEED_HALO: usize = 3;
pub const BLEED_GRAIN: usize = 4;
pub const BLEED_SEED: usize = 5;

pub const GRAD_WHOLE: usize = 0;
pub const GRAD_COUNT: usize = 1;
/// Première couleur ; la suivante est trois rangs plus loin.
pub const GRAD_COLOR: usize = 2;
pub const GRAD_MIX: usize = 14;
pub const GRAD_SCROLL: usize = 15;
pub const GRAD_LOOP: usize = 16;
/// L'image, ajoutée par `fx::steps_at` quand le dégradé défile.
pub const GRAD_FRAME: usize = 17;

pub const NIB_ID: usize = 0;
pub const NIB_SIZE: usize = 1;
/// Signature du réglage de la plume, ajoutée par `fx::steps_at` (un autre réglage de même nom : un autre
/// dessin dérivé).
pub const NIB_SIG: usize = 2;

/// Numéro (24 bits) d'un réglage de pinceau d'après son nom : stable d'une séance à l'autre, exact dans un
/// réel de 32 bits.
pub const fn name_id(name: &str) -> u32 {
    let b = name.as_bytes();
    let mut h: u32 = 0x811c_9dc5;
    let mut i = 0;
    while i < b.len() {
        h ^= b[i] as u32;
        h = h.wrapping_mul(0x0100_0193);
        i += 1;
    }
    (h ^ (h >> 24)) & 0x00FF_FFFF
}

/// Le réglage de plume fabriqué par SHOT (une plume plate inclinée) : il est toujours là, même sans
/// bibliothèque de brosses.
pub const CALLIGRAPHY: &str = "Plume calligraphique";

pub(crate) const OUTLINE: [Param; 7] = [
    p("Largeur", 0.0, 60.0, 4.0, " px"),
    p("Douceur du bord", 0.0, 30.0, 1.0, " px"),
    p("Opacité", 0.0, 100.0, 100.0, " %"),
    p("Rouge", 0.0, 255.0, 255.0, ""),
    p("Vert", 0.0, 255.0, 255.0, ""),
    p("Bleu", 0.0, 255.0, 255.0, ""),
    p("Derrière le trait", 0.0, 1.0, 1.0, ""),
];
pub(crate) const DRY: [Param; 7] = [
    p("Trous (pour 100 px de trait)", 0.0, 30.0, 5.0, ""),
    p("Longueur d'un trou", 1.0, 150.0, 16.0, " px"),
    p("Profondeur des trous", 0.0, 100.0, 85.0, " %"),
    p("Bord qui s'effrite", 0.0, 100.0, 45.0, " %"),
    p("Graine", 1.0, 999.0, 1.0, ""),
    p("Changer toutes les", 0.0, 12.0, 0.0, " img"),
    p("Variantes", 2.0, 6.0, 3.0, ""),
];
pub(crate) const BLEED: [Param; 6] = [
    p("Étalement", 0.0, 12.0, 2.5, " px"),
    p("Irrégularité", 0.0, 100.0, 60.0, " %"),
    p("Bord plus foncé", 0.0, 100.0, 40.0, " %"),
    p("Force de la bavure", 0.0, 100.0, 55.0, " %"),
    p("Grain du papier", 1.0, 16.0, 4.0, " px"),
    p("Graine", 1.0, 999.0, 1.0, ""),
];
pub(crate) const GRADIENT: [Param; 17] = [
    p("Sur tout le dessin (dans l'ordre des traits)", 0.0, 1.0, 0.0, ""),
    p("Nombre de couleurs", 2.0, 4.0, 2.0, ""),
    p("Couleur 1 : rouge", 0.0, 255.0, 224.0, ""),
    p("Couleur 1 : vert", 0.0, 255.0, 64.0, ""),
    p("Couleur 1 : bleu", 0.0, 255.0, 48.0, ""),
    p("Couleur 2 : rouge", 0.0, 255.0, 48.0, ""),
    p("Couleur 2 : vert", 0.0, 255.0, 104.0, ""),
    p("Couleur 2 : bleu", 0.0, 255.0, 224.0, ""),
    p("Couleur 3 : rouge", 0.0, 255.0, 240.0, ""),
    p("Couleur 3 : vert", 0.0, 255.0, 200.0, ""),
    p("Couleur 3 : bleu", 0.0, 255.0, 48.0, ""),
    p("Couleur 4 : rouge", 0.0, 255.0, 56.0, ""),
    p("Couleur 4 : vert", 0.0, 255.0, 176.0, ""),
    p("Couleur 4 : bleu", 0.0, 255.0, 88.0, ""),
    p("Mélange avec la couleur d'origine", 0.0, 100.0, 100.0, " %"),
    p("Défilement", -50.0, 50.0, 0.0, " %/img"),
    p("Boucle", 0.0, 1.0, 0.0, ""),
];
pub(crate) const NIB: [Param; 2] = [
    p("Plume", 0.0, 16_777_215.0, name_id(CALLIGRAPHY) as f32, ""),
    p("Taille de la plume", 25.0, 300.0, 100.0, " %"),
];

/// Le réglage `k` de `kind` est le rouge d'une couleur (le vert et le bleu suivent) : l'interface montre
/// alors un sélecteur de couleur au lieu de trois curseurs.
pub fn color_start(kind: EffectKind, k: usize) -> Option<usize> {
    match (kind, k) {
        (EffectKind::Outline, OUTLINE_RGB) => Some(OUTLINE_RGB),
        (EffectKind::StrokeGradient, k) if (GRAD_COLOR..GRAD_COLOR + 12).contains(&k) && (k - GRAD_COLOR) % 3 == 0 => Some(k),
        _ => None,
    }
}

/// Le réglage `k` de `kind` est le vert ou le bleu d'une couleur (sauté par l'interface).
pub fn color_tail(kind: EffectKind, k: usize) -> bool {
    match kind {
        EffectKind::Outline => k == OUTLINE_RGB + 1 || k == OUTLINE_RGB + 2,
        EffectKind::StrokeGradient => (GRAD_COLOR..GRAD_COLOR + 12).contains(&k) && (k - GRAD_COLOR) % 3 != 0,
        _ => false,
    }
}

/// Le nom de la couleur qui commence au réglage `k`.
pub fn color_label(kind: EffectKind, k: usize) -> String {
    match kind {
        EffectKind::Outline => "Couleur".into(),
        _ => format!("Couleur {}", (k - GRAD_COLOR) / 3 + 1),
    }
}

fn byte(v: &[f32], k: usize, default: f32) -> u8 {
    at(v, k, default).round().clamp(0.0, 255.0) as u8
}

/// Le liseré.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlineS {
    pub width: f32,
    pub soft: f32,
    /// 0 à 1.
    pub opacity: f32,
    pub rgb: [u8; 3],
    pub behind: bool,
}

impl OutlineS {
    pub fn of(v: &[f32]) -> OutlineS {
        OutlineS {
            width: at(v, OUTLINE_WIDTH, 4.0).clamp(0.0, 60.0),
            soft: at(v, OUTLINE_SOFT, 1.0).clamp(0.0, 30.0),
            opacity: (at(v, OUTLINE_OPACITY, 100.0) / 100.0).clamp(0.0, 1.0),
            rgb: [byte(v, OUTLINE_RGB, 255.0), byte(v, OUTLINE_RGB + 1, 255.0), byte(v, OUTLINE_RGB + 2, 255.0)],
            behind: at(v, OUTLINE_BEHIND, 1.0) >= 0.5,
        }
    }

    pub fn changes(&self) -> bool {
        self.width >= 0.05 && self.opacity > 0.0
    }
}

/// Le trait sec.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DryS {
    /// Trous pour 100 pixels de trait.
    pub freq: f32,
    pub length: f32,
    /// 0 à 1.
    pub depth: f32,
    /// 0 à 1.
    pub rough: f32,
    pub seed: u32,
    /// Images pour chaque tirage (0 : toujours le même).
    pub every: usize,
    pub variants: usize,
    pub variant: usize,
}

impl DryS {
    pub fn of(v: &[f32]) -> DryS {
        let variants = (at(v, DRY_VARIANTS, 3.0).round() as i64).clamp(2, 6) as usize;
        DryS {
            freq: at(v, DRY_FREQ, 5.0).clamp(0.0, 30.0),
            length: at(v, DRY_LENGTH, 16.0).clamp(1.0, 150.0),
            depth: (at(v, DRY_DEPTH, 85.0) / 100.0).clamp(0.0, 1.0),
            rough: (at(v, DRY_ROUGH, 45.0) / 100.0).clamp(0.0, 1.0),
            seed: (at(v, DRY_SEED, 1.0).round() as i64).clamp(1, 999) as u32,
            every: (at(v, DRY_EVERY, 0.0).round() as i64).clamp(0, 12) as usize,
            variants,
            variant: (at(v, DRY_VARIANT, 0.0).round().max(0.0) as usize) % variants,
        }
    }

    pub fn changes(&self) -> bool {
        (self.freq > 0.01 && self.depth > 0.005) || self.rough > 0.005
    }

    /// Le tirage d'un trait de rang `rank` (le même pour toutes ses images tant que la variante ne change pas).
    fn stroke_seed(&self, rank: usize) -> u32 {
        hash(self.seed, rank as u32, self.variant as u32 + 1, 0xD1)
    }
}

/// La variante du trait sec à l'image `frame` (0 quand il ne se retire pas).
pub fn dry_variant(values: &[f32], frame: usize) -> usize {
    let s = DryS::of(values);
    if s.every == 0 {
        0
    } else {
        crate::tremblement::variant_at(frame, s.every, s.variants)
    }
}

/// L'encre qui bave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BleedS {
    pub spread: f32,
    /// 0 à 1.
    pub irreg: f32,
    pub dark: f32,
    pub halo: f32,
    pub grain: f32,
    pub seed: u32,
}

impl BleedS {
    pub fn of(v: &[f32]) -> BleedS {
        BleedS {
            spread: at(v, BLEED_SPREAD, 2.5).clamp(0.0, 12.0),
            irreg: (at(v, BLEED_IRREG, 60.0) / 100.0).clamp(0.0, 1.0),
            dark: (at(v, BLEED_DARK, 40.0) / 100.0).clamp(0.0, 1.0),
            halo: (at(v, BLEED_HALO, 55.0) / 100.0).clamp(0.0, 1.0),
            grain: at(v, BLEED_GRAIN, 4.0).clamp(1.0, 16.0),
            seed: (at(v, BLEED_SEED, 1.0).round() as i64).clamp(1, 999) as u32,
        }
    }

    pub fn changes(&self) -> bool {
        (self.spread >= 0.05 && self.halo > 0.0) || self.dark > 0.005
    }

    /// Jusqu'où la bavure s'étend au plus (pixels).
    fn reach(&self) -> f32 {
        self.spread * (1.0 + self.irreg)
    }
}

/// Le dégradé le long du trait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradS {
    pub whole: bool,
    pub count: usize,
    pub colors: [[f32; 3]; 4],
    /// 0 à 1.
    pub mix: f32,
    /// Part du dégradé parcourue à chaque image.
    pub scroll: f32,
    pub looped: bool,
    pub frame: f32,
}

impl GradS {
    pub fn of(v: &[f32]) -> GradS {
        let colors = std::array::from_fn(|i| {
            let k = GRAD_COLOR + 3 * i;
            let d = GRADIENT[k].default;
            let e = GRADIENT[k + 1].default;
            let f = GRADIENT[k + 2].default;
            [at(v, k, d).clamp(0.0, 255.0), at(v, k + 1, e).clamp(0.0, 255.0), at(v, k + 2, f).clamp(0.0, 255.0)]
        });
        GradS {
            whole: at(v, GRAD_WHOLE, 0.0) >= 0.5,
            count: (at(v, GRAD_COUNT, 2.0).round() as i64).clamp(2, 4) as usize,
            colors,
            mix: (at(v, GRAD_MIX, 100.0) / 100.0).clamp(0.0, 1.0),
            scroll: at(v, GRAD_SCROLL, 0.0).clamp(-50.0, 50.0) / 100.0,
            looped: at(v, GRAD_LOOP, 0.0) >= 0.5,
            frame: at(v, GRAD_FRAME, 0.0).max(0.0),
        }
    }

    pub fn changes(&self) -> bool {
        self.mix > 0.0
    }

    /// La couleur (0 à 255) à la position `t` du dégradé (0 : début, 1 : fin), défilement compris.
    pub fn color(&self, t: f32) -> [f32; 3] {
        let t = t + self.scroll * self.frame;
        let n = self.count;
        let (pos, next_wraps) = if self.looped { (t.rem_euclid(1.0) * n as f32, true) } else { (t.clamp(0.0, 1.0) * (n - 1) as f32, false) };
        let i = (pos.floor() as usize).min(if next_wraps { n - 1 } else { n - 2 });
        let f = (pos - i as f32).clamp(0.0, 1.0);
        let (a, b) = (self.colors[i], self.colors[if next_wraps { (i + 1) % n } else { i + 1 }]);
        std::array::from_fn(|c| a[c] + (b[c] - a[c]) * f)
    }
}

/// La plume de rechange.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NibS {
    pub id: u32,
    /// Facteur de la taille du réglage.
    pub size: f32,
    pub sig: u32,
}

impl NibS {
    pub fn of(v: &[f32]) -> NibS {
        NibS {
            id: at(v, NIB_ID, NIB[NIB_ID].default).round().clamp(0.0, 16_777_215.0) as u32,
            size: (at(v, NIB_SIZE, 100.0) / 100.0).clamp(0.25, 3.0),
            sig: at(v, NIB_SIG, 0.0).round().max(0.0) as u32,
        }
    }

    pub fn changes(&self) -> bool {
        self.id != 0
    }
}

// ---------------------------------------------------------------------- noms, bulles, préréglages

/// Les effets de matière, dans l'ordre où la rangée d'icônes et les menus les montrent.
pub const KINDS: [EffectKind; 5] = [EffectKind::Outline, EffectKind::Dry, EffectKind::Bleed, EffectKind::StrokeGradient, EffectKind::Nib];

pub fn icon(kind: EffectKind) -> &'static str {
    use egui_phosphor::regular as ph;
    match kind {
        EffectKind::Outline => ph::SELECTION,
        EffectKind::Dry => ph::PAINT_BRUSH_BROAD,
        EffectKind::Bleed => ph::DROP_HALF,
        EffectKind::StrokeGradient => ph::GRADIENT,
        EffectKind::Nib => ph::PEN_NIB,
        _ => "",
    }
}

/// L'action (`keymap.toml`) qui coche et décoche l'effet sur le calque courant.
pub fn action(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Outline => "fx.lisere",
        EffectKind::Dry => "fx.trait_sec",
        EffectKind::Bleed => "fx.encre_bave",
        EffectKind::StrokeGradient => "fx.degrade_trait",
        EffectKind::Nib => "fx.plume",
        _ => "",
    }
}

/// L'action qui remplace pour de bon les traits par ceux de la plume (« Appliquer pour de bon »).
pub const BAKE_ACTION: &str = "fx.plume_appliquer";

pub fn kind_of_action(action_id: &str) -> Option<EffectKind> {
    KINDS.into_iter().find(|k| action(*k) == action_id)
}

/// Pourquoi l'effet est grisé sur un calque bitmap (bulle des menus et de la rangée).
pub fn bitmap_why(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::StrokeGradient => {
            "Le dégradé suit le sens de chaque trait (début, fin, ordre de dessin) : il ne va que sur un calque vectoriel. Choisis un calque vectoriel, ou crée-en un (Calques > Nouveau calque > Vectoriel)."
        }
        _ => {
            "Changer de plume redessine les traits (leur tracé et leur pression) avec un autre pinceau : il ne va que sur un calque vectoriel. Choisis un calque vectoriel, ou crée-en un (Calques > Nouveau calque > Vectoriel)."
        }
    }
}

pub fn hint(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Outline => "Un contour d'une autre couleur autour du dessin, pour détacher un personnage du fond (largeur, douceur, couleur). Marche sur les calques vectoriels et bitmap.",
        EffectKind::Dry => "Le trait s'interrompt par endroits et son bord s'effrite, comme une brosse presque sèche. Vectoriel : les trous suivent chaque trait. Bitmap : version simplifiée, un bruit sur les bords et des plages creuses.",
        EffectKind::Bleed => "Le bord du trait bave un peu et devient irrégulier, comme de l'encre sur du papier buvard ; le trait reste lisible. Marche sur les calques vectoriels et bitmap.",
        EffectKind::StrokeGradient => "La couleur change du début à la fin de chaque trait, ou sur tout le dessin dans l'ordre des traits ; le défilement est animable. Calques vectoriels seulement.",
        EffectKind::Nib => "Redessine les traits avec une autre plume (calligraphie, feutre, pinceau encre, fusain…) en gardant leur tracé et leur pression. Les traits d'origine ne changent pas ; « Appliquer pour de bon » les remplace. Calques vectoriels seulement.",
        _ => "",
    }
}

pub fn param_hint(kind: EffectKind, k: usize) -> &'static str {
    match (kind, k) {
        (EffectKind::Outline, OUTLINE_WIDTH) => "Épaisseur du liseré à partir du bord du trait, en pixels de la feuille.",
        (EffectKind::Outline, OUTLINE_SOFT) => "Largeur du fondu du bord extérieur. 0 : bord net (adouci d'un pixel).",
        (EffectKind::Outline, OUTLINE_OPACITY) => "Opacité du liseré. Le trait, lui, reste tel quel.",
        (EffectKind::Outline, OUTLINE_RGB) => "Couleur du liseré.",
        (EffectKind::Outline, OUTLINE_BEHIND) => "Coché, le liseré passe sous le trait : un trait un peu transparent laisse voir sa couleur. Décoché, c'est un anneau autour du trait, sans rien dessous.",
        (EffectKind::Dry, DRY_FREQ) => "Combien de trous pour 100 pixels de trait, en moyenne. 0 : pas de trou (seul le bord s'effrite).",
        (EffectKind::Dry, DRY_LENGTH) => "Longueur moyenne d'un trou, en pixels le long du trait. Sur un calque bitmap : la taille des plages creuses.",
        (EffectKind::Dry, DRY_DEPTH) => "À quel point les trous sont vides. 100 % : plus d'encre ; moins : quelques poils de brosse passent.",
        (EffectKind::Dry, DRY_ROUGH) => "Part du bord du trait qui s'effrite, en stries parallèles au trait. 0 : bord intact.",
        (EffectKind::Dry, DRY_SEED) => "Change le tirage au hasard : d'autres trous aux autres endroits.",
        (EffectKind::Dry, DRY_EVERY) => "Images pour chaque tirage : les trous changent de place toutes les N images, comme le tremblement. 0 : toujours les mêmes.",
        (EffectKind::Dry, DRY_VARIANTS) => "Nombre de tirages différents qui se suivent (si « Changer toutes les » n'est pas 0).",
        (EffectKind::Bleed, BLEED_SPREAD) => "Jusqu'où l'encre bave autour du trait, en pixels.",
        (EffectKind::Bleed, BLEED_IRREG) => "0 : la bavure suit le trait de façon régulière. 100 % : elle s'étend très inégalement, par taches.",
        (EffectKind::Bleed, BLEED_DARK) => "Le bord du trait fonce et s'effrite un peu, là où l'encre s'accumule. 0 : bord inchangé.",
        (EffectKind::Bleed, BLEED_HALO) => "Opacité de la bavure près du trait. 100 % : presque aussi foncée que le trait.",
        (EffectKind::Bleed, BLEED_GRAIN) => "Taille des taches de la bavure, comme le grain du papier, en pixels.",
        (EffectKind::Bleed, BLEED_SEED) => "Change le tirage au hasard : une autre bavure.",
        (EffectKind::StrokeGradient, GRAD_WHOLE) => "Coché, le dégradé court sur tout le dessin, d'un trait au suivant dans l'ordre où ils ont été dessinés. Décoché : chaque trait refait tout le dégradé.",
        (EffectKind::StrokeGradient, GRAD_COUNT) => "Combien de couleurs ont les arrêts du dégradé (2 à 4).",
        (EffectKind::StrokeGradient, GRAD_MIX) => "Part de la couleur du dégradé. 100 % : la couleur du trait est remplacée ; moins : elle se mêle à celle d'origine.",
        (EffectKind::StrokeGradient, GRAD_SCROLL) => "Part du dégradé parcourue à chaque image : le dégradé défile le long des traits. Négatif : dans l'autre sens.",
        (EffectKind::StrokeGradient, GRAD_LOOP) => "Coché, le dégradé revient à sa première couleur au bout (utile avec le défilement). Décoché : les bouts gardent la première et la dernière couleur.",
        (EffectKind::StrokeGradient, k) if (GRAD_COLOR..GRAD_COLOR + 12).contains(&k) => "Couleur de cet arrêt du dégradé.",
        (EffectKind::Nib, NIB_ID) => "Le réglage de pinceau qui redessine les traits : un des réglages de la bibliothèque, ou la plume calligraphique de FEANOR.",
        (EffectKind::Nib, NIB_SIZE) => "Taille de la plume, en pourcentage de celle du réglage. La pression des traits d'origine règle l'épaisseur, comme à la main.",
        _ => "",
    }
}

/// Un préréglage de matière : des effets tout réglés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Une encre qui bave, au bord foncé.
    ChinaInk,
    /// Un feutre qui s'use : la plume du feutre et des trous.
    WornFelt,
    /// Un liseré blanc autour du dessin.
    WhiteOutline,
    /// Un arc-en-ciel d'un trait à l'autre.
    Rainbow,
    /// La plume calligraphique.
    CalligraphyPen,
}

impl Preset {
    pub const ALL: [Preset; 5] = [Preset::ChinaInk, Preset::WornFelt, Preset::WhiteOutline, Preset::Rainbow, Preset::CalligraphyPen];

    pub fn label(self) -> &'static str {
        match self {
            Preset::ChinaInk => "Encre de Chine",
            Preset::WornFelt => "Feutre usé",
            Preset::WhiteOutline => "Contour blanc",
            Preset::Rainbow => "Arc-en-ciel",
            Preset::CalligraphyPen => "Plume calligraphique",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Preset::ChinaInk => "Le bord du trait bave un peu et fonce, comme de l'encre de Chine sur du papier (encre qui bave). Calques vectoriels et bitmap.",
            Preset::WornFelt => "Le dessin est repris au feutre (changer de plume) et le trait s'interrompt par endroits, comme un feutre presque sec (trait sec). Calques vectoriels seulement.",
            Preset::WhiteOutline => "Un liseré blanc de 4 pixels autour du dessin, derrière le trait, pour détacher un personnage du fond. Calques vectoriels et bitmap.",
            Preset::Rainbow => "Les traits prennent toutes les couleurs de l'arc-en-ciel, de la couleur du premier trait à celle du dernier (dégradé sur tout le dessin). Calques vectoriels seulement.",
            Preset::CalligraphyPen => "Les traits sont redessinés avec une plume plate inclinée : pleins et déliés selon la direction et la pression (changer de plume). Calques vectoriels seulement.",
        }
    }

    pub fn action(self) -> &'static str {
        match self {
            Preset::ChinaInk => "fx.preset_encre_chine",
            Preset::WornFelt => "fx.preset_feutre_use",
            Preset::WhiteOutline => "fx.preset_contour_blanc",
            Preset::Rainbow => "fx.preset_arc_en_ciel",
            Preset::CalligraphyPen => "fx.preset_plume_calligraphique",
        }
    }

    pub fn from_action(action_id: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.action() == action_id)
    }

    /// Le préréglage a besoin d'un calque vectoriel.
    pub fn vector_only(self) -> bool {
        self.build().iter().any(|e| e.kind.vector_only())
    }

    /// Les effets du préréglage (pas de clé : la matière ne s'écrit pas dans le temps).
    pub fn build(self) -> Vec<Effect> {
        let with = |kind: EffectKind, set: &[(usize, f32)]| {
            let mut e = Effect::new(kind);
            for &(k, v) in set {
                e.set_param(k, v);
            }
            e
        };
        match self {
            Preset::ChinaInk => vec![with(
                EffectKind::Bleed,
                &[(BLEED_SPREAD, 2.0), (BLEED_IRREG, 65.0), (BLEED_DARK, 60.0), (BLEED_HALO, 50.0), (BLEED_GRAIN, 3.0)],
            )],
            Preset::WornFelt => vec![
                with(EffectKind::Nib, &[(NIB_ID, name_id("Feutre") as f32), (NIB_SIZE, 100.0)]),
                with(EffectKind::Dry, &[(DRY_FREQ, 7.0), (DRY_LENGTH, 12.0), (DRY_DEPTH, 70.0), (DRY_ROUGH, 55.0)]),
            ],
            Preset::WhiteOutline => vec![with(EffectKind::Outline, &[(OUTLINE_WIDTH, 4.0), (OUTLINE_SOFT, 1.0)])],
            Preset::Rainbow => vec![with(
                EffectKind::StrokeGradient,
                &[
                    (GRAD_WHOLE, 1.0),
                    (GRAD_COUNT, 4.0),
                    (GRAD_COLOR, 230.0),
                    (GRAD_COLOR + 1, 50.0),
                    (GRAD_COLOR + 2, 50.0),
                    (GRAD_COLOR + 3, 240.0),
                    (GRAD_COLOR + 4, 200.0),
                    (GRAD_COLOR + 5, 40.0),
                    (GRAD_COLOR + 6, 60.0),
                    (GRAD_COLOR + 7, 180.0),
                    (GRAD_COLOR + 8, 80.0),
                    (GRAD_COLOR + 9, 60.0),
                    (GRAD_COLOR + 10, 100.0),
                    (GRAD_COLOR + 11, 220.0),
                ],
            )],
            Preset::CalligraphyPen => vec![with(EffectKind::Nib, &[(NIB_ID, name_id(CALLIGRAPHY) as f32), (NIB_SIZE, 100.0)])],
        }
    }
}

// ---------------------------------------------------------------------- le hasard (bruit de valeur)

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Bruit de valeur lissé, de [0, 1), à la position (x, y) (unités de la maille).
fn vnoise(seed: u32, x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (smooth(x - fx), smooth(y - fy));
    let (ix, iy) = (fx as i32 as u32, fy as i32 as u32);
    let c = |dx: u32, dy: u32| unit(hash(seed, ix.wrapping_add(dx), iy.wrapping_add(dy), 7));
    lerp(lerp(c(0, 0), c(1, 0), tx), lerp(c(0, 1), c(1, 1), tx), ty)
}

/// Trois octaves de bruit de valeur, de [0, 1) ; `cell` : la plus grande maille, en pixels.
fn fbm(seed: u32, x: f32, y: f32, cell: f32) -> f32 {
    let (u, v) = (x / cell, y / cell);
    0.6 * vnoise(seed, u, v) + 0.3 * vnoise(seed ^ 0x55, u * 2.3 + 17.0, v * 2.3 + 5.0) + 0.1 * vnoise(seed ^ 0xAA, u * 5.1 + 3.0, v * 5.1 + 11.0)
}

// ---------------------------------------------------------------------- distance exacte au bord

/// Couverture (sur 255) à partir de laquelle un pixel est de l'encre : le bord de l'encre est le contour à 25 %.
pub(crate) const SITE_ALPHA: u8 = 64;
/// « Infini » des distances au carré.
pub(crate) const FAR: f32 = 1e18;

/// Distance au carré au site le plus proche, et ce site (rang du pixel), pour chaque pixel d'un
/// rectangle ; `(FAR, u32::MAX)` quand il n'y a aucun site.
pub(crate) struct Dist {
    pub(crate) v: Vec<(f32, u32)>,
}

impl Dist {
    fn d2(&self, i: usize) -> f32 {
        self.v[i].0
    }
}

/// Transformée en distance d'une dimension (Felzenszwalb et Huttenlocher) : `d[q] = min_p ((q - p)² + f[p])`,
/// `arg[q]` le `p` qui donne le minimum. `v` (n) et `z` (n + 1) : de la place de travail.
fn dt1(f: &[f32], d: &mut [f32], arg: &mut [u32], v: &mut [u32], z: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let fq = f[q] + (q * q) as f32;
        let s = loop {
            let p = v[k] as usize;
            let s = (fq - (f[p] + (p * p) as f32)) / (2.0 * (q - p) as f32);
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break s;
            }
        };
        k += 1;
        v[k] = q as u32;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k] as usize;
        let dq = q as f32 - p as f32;
        d[q] = dq * dq + f[p];
        arg[q] = p as u32;
    }
}

/// Les distances de tous les pixels d'un rectangle `w` × `h` aux pixels `site`. Deux passes séparables
/// (colonnes, puis lignes), réparties sur les cœurs ; chaque colonne et chaque ligne se calcule seule : le
/// résultat ne dépend pas du nombre de fils.
pub(crate) fn distance(w: usize, h: usize, site: &[bool]) -> Dist {
    const COLS: usize = 32;
    const ROWS: usize = 8;
    if w == 0 || h == 0 || !site.iter().any(|&s| s) {
        return Dist { v: vec![(FAR, u32::MAX); w * h] };
    }
    // Colonnes : stockage par colonnes (chaque colonne se suit en mémoire).
    let mut col: Vec<(f32, u32)> = vec![(FAR, 0); w * h];
    crate::render::par_chunks(&mut col, h * COLS, |ci, chunk| {
        let n = h;
        let (mut f, mut d, mut arg) = (vec![0.0f32; n], vec![0.0f32; n], vec![0u32; n]);
        let (mut v, mut z) = (vec![0u32; n], vec![0.0f32; n + 1]);
        for (j, column) in chunk.chunks_mut(h).enumerate() {
            let x = ci * COLS + j;
            for y in 0..h {
                f[y] = if site[y * w + x] { 0.0 } else { FAR };
            }
            dt1(&f, &mut d, &mut arg, &mut v, &mut z);
            for y in 0..h {
                column[y] = (d[y], arg[y]);
            }
        }
    });
    // Lignes.
    let mut out: Vec<(f32, u32)> = vec![(FAR, u32::MAX); w * h];
    let col = &col;
    crate::render::par_chunks(&mut out, w * ROWS, |ri, chunk| {
        let n = w;
        let (mut f, mut d, mut arg) = (vec![0.0f32; n], vec![0.0f32; n], vec![0u32; n]);
        let (mut v, mut z) = (vec![0u32; n], vec![0.0f32; n + 1]);
        for (j, row) in chunk.chunks_mut(w).enumerate() {
            let y = ri * ROWS + j;
            for x in 0..w {
                f[x] = col[x * h + y].0;
            }
            dt1(&f, &mut d, &mut arg, &mut v, &mut z);
            for x in 0..w {
                if d[x] < FAR * 0.5 {
                    let sx = arg[x] as usize;
                    let sy = col[sx * h + y].1 as usize;
                    row[x] = (d[x], (sy * w + sx) as u32);
                }
            }
        }
    });
    Dist { v: out }
}

// ---------------------------------------------------------------------- pixels

/// La zone de pixels de `src` (avec une marge de `margin` pixels) ; rien si le dessin est vide.
fn region(src: &Raster, margin: usize) -> Option<crate::fx::Region> {
    crate::fx::region_of(src, margin)
}

/// `a` composé sur `b`, prémultipliés (chaque canal sur 255, en réels).
fn over_f(a: [f32; 4], b: [f32; 4]) -> Color32 {
    let k = 1.0 - a[3] / 255.0;
    let ch = |i: usize| (a[i] + b[i] * k).round().clamp(0.0, 255.0);
    let alpha = ch(3);
    Color32::from_rgba_premultiplied(ch(0).min(alpha) as u8, ch(1).min(alpha) as u8, ch(2).min(alpha) as u8, alpha as u8)
}

fn px_f(c: Color32) -> [f32; 4] {
    [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
}

// ---------------------------------------------------------------------- liseré

/// Jusqu'où le liseré lit autour d'un pixel (`fxmonde.rs`).
pub(crate) fn portee_du_lisere(v: &[f32]) -> f32 {
    let s = OutlineS::of(v);
    s.width + s.soft.max(1.0) * 0.5 + 4.0
}

/// Jusqu'où l'encre qui bave lit autour d'un pixel (`fxmonde.rs`).
pub(crate) fn portee_de_la_bavure(v: &[f32]) -> f32 {
    BleedS::of(v).reach() + 5.0
}

/// Le liseré de `src` (calque vectoriel ou bitmap : il ne lit que les pixels). Rien d'encré : `src`.
pub fn outline(src: &Raster, v: &[f32]) -> Raster {
    let s = OutlineS::of(v);
    if !s.changes() {
        return src.clone();
    }
    let soft = s.soft.max(1.0);
    let reach = s.width + soft * 0.5;
    let Some(mut r) = region(src, reach.ceil() as usize + 3) else { return src.clone() };
    let (w, h) = (r.w, r.h);
    let site: Vec<bool> = r.px.iter().map(|c| c.a() >= SITE_ALPHA).collect();
    let dist = distance(w, h, &site);
    let rgb = s.rgb.map(|c| c as f32);
    let dist = &dist;
    crate::render::par_chunks(&mut r.px, w * 16, |ci, chunk| {
        for (k, px) in chunk.iter_mut().enumerate() {
            let d2 = dist.d2(ci * w * 16 + k);
            if d2 > FAR * 0.5 {
                continue;
            }
            // Distance au bord de l'encre (négative dans l'encre), fondu centré sur la largeur.
            let d = d2.sqrt() - 0.5;
            let cov = smooth((s.width + soft * 0.5 - d) / soft);
            if cov <= 0.0 {
                continue;
            }
            let a_s = px.a() as f32 / 255.0;
            // En anneau : rien sous l'encre (d2 == 0), et sous le fondu du bord du trait de moins en moins.
            let o = cov * s.opacity * if s.behind { 1.0 } else if d2 == 0.0 { 0.0 } else { 1.0 - a_s };
            if o <= 0.0 {
                continue;
            }
            *px = over_f(px_f(*px), [rgb[0] * o, rgb[1] * o, rgb[2] * o, 255.0 * o]);
        }
    });
    crate::fx::into_raster(&r, src)
}

// ---------------------------------------------------------------------- encre qui bave

/// L'encre qui bave de `src` : un halo irrégulier autour de l'encre (de sa couleur, plus pâle), le bord de
/// l'encre plus foncé et un peu effrité. Le cœur du trait reste lisible.
pub fn bleed(src: &Raster, v: &[f32], origine: (i64, i64)) -> Raster {
    let s = BleedS::of(v);
    if !s.changes() {
        return src.clone();
    }
    let Some(mut r) = region(src, s.reach().ceil() as usize + 4) else { return src.clone() };
    let (w, h) = (r.w, r.h);
    let site: Vec<bool> = r.px.iter().map(|c| c.a() >= SITE_ALPHA).collect();
    let outer = distance(w, h, &site);
    // Le bord intérieur : distance de l'encre à ce qui n'en est pas.
    let inner = (s.dark > 0.005 || s.irreg > 0.0).then(|| distance(w, h, &site.iter().map(|b| !b).collect::<Vec<bool>>()));
    let src_px = r.px.clone();
    // (Le bruit de l'effritement se lit au point du monde : il continue à travers le bord de la feuille.)
    let (x0, y0) = ((r.x0 as i64 + origine.0) as i32, (r.y0 as i64 + origine.1) as i32);
    let (outer, inner, src_px) = (&outer, &inner, &src_px);
    let seed = hash(s.seed, 0, 0, 0xB1);
    crate::render::par_chunks(&mut r.px, w * 16, |ci, chunk| {
        for (k, px) in chunk.iter_mut().enumerate() {
            let i = ci * w * 16 + k;
            let (ix, iy) = (x0 + (i % w) as i32, y0 + (i / w) as i32);
            let (fx, fy) = (ix as f32 + 0.5, iy as f32 + 0.5);
            let mut core = px_f(*px);
            let (d2, site_i) = outer.v[i];
            // Le bord de l'encre : plus foncé, un peu effrité.
            if core[3] > 0.0 {
                let rim = if let Some(inner) = inner {
                    if d2 == 0.0 {
                        let di = inner.d2(i).min(FAR).sqrt();
                        (1.0 - (di - 0.5) / 2.2).clamp(0.0, 1.0)
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };
                if rim > 0.0 {
                    let darker = 1.0 - 0.55 * s.dark * rim;
                    let ragged = 1.0 - 0.35 * s.irreg * rim * vnoise(seed ^ 0x3C, fx / (s.grain * 0.5), fy / (s.grain * 0.5));
                    // (La couverture baisse avec l'effritement ; la couleur, avec le foncé.)
                    core = [core[0] * darker * ragged, core[1] * darker * ragged, core[2] * darker * ragged, core[3] * ragged];
                }
            }
            // La bavure : sous le trait, dans la couleur de l'encre la plus proche.
            let mut halo = [0.0f32; 4];
            if d2 > 0.0 && d2 < FAR * 0.5 && s.spread >= 0.05 && s.halo > 0.0 {
                let d = d2.sqrt() - 0.5;
                let n = fbm(seed, fx, fy, s.grain * 2.0);
                let n = ((n - 0.5) * 1.7 + 0.5).clamp(0.0, 1.0);
                let reach = s.spread * (1.0 - s.irreg + s.irreg * 2.0 * n);
                if d < reach && reach > 0.05 {
                    let u = (d / reach).clamp(0.0, 1.0);
                    let body = (1.0 - u).powf(1.4);
                    let near = src_px[site_i as usize];
                    let na = near.a() as f32;
                    if na > 0.0 {
                        let a = s.halo * body * na / 255.0;
                        let k = a * 255.0 / na;
                        halo = [near.r() as f32 * k, near.g() as f32 * k, near.b() as f32 * k, 255.0 * a];
                    }
                }
            }
            if core[3] > 0.0 || halo[3] > 0.0 {
                *px = over_f(core, halo);
            }
        }
    });
    crate::fx::into_raster(&r, src)
}

// ---------------------------------------------------------------------- trait sec

/// Le trait sec d'un calque **bitmap** (forme simplifiée) : des plages creuses d'un bruit à basse
/// fréquence, et le bord de l'encre qui s'effrite. Sur un calque vectoriel, c'est `dry_stroke`.
pub fn dry_pixels(src: &Raster, v: &[f32], origine: (i64, i64)) -> Raster {
    let s = DryS::of(v);
    if !s.changes() {
        return src.clone();
    }
    let Some(mut r) = region(src, 2) else { return src.clone() };
    let (w, h) = (r.w, r.h);
    let (x0, y0) = ((r.x0 as i64 + origine.0) as i32, (r.y0 as i64 + origine.1) as i32);
    let seed = hash(s.seed, 0, s.variant as u32 + 1, 0xD2);
    let inner = (s.rough > 0.005).then(|| distance(w, h, &r.px.iter().map(|c| c.a() < SITE_ALPHA).collect::<Vec<bool>>()));
    let inner = &inner;
    // Part de la feuille creuse : les trous (pour 100 px) multipliés par leur longueur.
    let holes = s.freq > 0.01 && s.depth > 0.005;
    let phi = (s.freq / 100.0 * s.length * 0.75).clamp(0.02, 0.85);
    let threshold = 0.5 + 0.2 * ((1.0 - phi) / phi).ln() / 1.7;
    let edge_reach = 0.8 + 2.2 * s.rough;
    let cell = s.length.max(2.0);
    crate::render::par_chunks(&mut r.px, w * 16, |ci, chunk| {
        for (k, px) in chunk.iter_mut().enumerate() {
            if px.a() == 0 {
                continue;
            }
            let i = ci * w * 16 + k;
            let (ix, iy) = (x0 + (i % w) as i32, y0 + (i / w) as i32);
            let (fx, fy) = (ix as f32 + 0.5, iy as f32 + 0.5);
            // Du bruit fin, de stries et de grains : de quoi effriter.
            let nn = 0.55 * vnoise(seed ^ 0x11, fx / 1.1, fy / 1.1) + 0.45 * unit(hash(seed, ix as u32, iy as u32, 9));
            let mut f = 1.0f32;
            if let Some(inner) = inner {
                let rim = if px.a() >= SITE_ALPHA {
                    (1.0 - (inner.d2(i).min(FAR).sqrt() - 0.5) / edge_reach).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                f *= smooth(((nn - s.rough * rim * 0.85) / 0.25 + 1.0).clamp(0.0, 1.0));
            }
            if holes {
                let g = fbm(seed ^ 0x77, fx, fy, cell);
                let gap = smooth((g - threshold) / 0.1 + 0.5);
                f *= 1.0 - s.depth * gap * (1.0 - 0.6 * nn * nn);
            }
            *px = scaled(*px, f.max(0.0));
        }
    });
    crate::fx::into_raster(&r, src)
}

/// Les trous d'un trait sur son axe : pour chaque point rééchantillonné, à quel point le trait est creux
/// (0 à `depth`). Les trous arrivent au hasard (une case de longueur fixe tous les ~75/fréquence pixels,
/// trois fois sur quatre un trou y est, à une place et d'une longueur tirées au hasard).
fn gap_profile(total: f32, step: f32, s: &DryS, seed: u32) -> Vec<f32> {
    let n = (total / step).ceil() as usize + 2;
    let mut out = vec![0.0f32; n];
    if s.freq <= 0.01 || s.depth <= 0.005 {
        return out;
    }
    let cell = (75.0 / s.freq).max(3.0);
    let cells = (total / cell).ceil() as usize + 2;
    for k in 0..cells {
        let kk = k as u32;
        if unit(hash(seed, kk, 0, 1)) >= 0.75 {
            continue;
        }
        let centre = (k as f32 + 0.2 + 0.6 * unit(hash(seed, kk, 0, 2))) * cell;
        let half = 0.5 * s.length * (0.45 + 1.1 * unit(hash(seed, kk, 0, 3)));
        let depth = s.depth * (0.6 + 0.4 * unit(hash(seed, kk, 0, 4)));
        let edge = (0.35 * half).max(1.5);
        let i0 = (((centre - half - edge) / step).floor().max(0.0)) as usize;
        let i1 = ((((centre + half + edge) / step).ceil()) as usize).min(n - 1);
        for i in i0..=i1 {
            let d = (i as f32 * step - centre).abs();
            let g = smooth((half - d) / edge + 0.5) * depth;
            if g > out[i] {
                out[i] = g;
            }
        }
    }
    out
}

/// Le trait sec d'un trait vectoriel, sur ses pixels `cur` (avec son axe `geo`) : des trous le long du
/// trait où passent quelques poils, et le bord qui s'effrite en stries parallèles au trait.
pub(crate) fn dry_stroke(cur: &mut Dense, geo: &Geo, s: &DryS, rank: usize) {
    let seed = s.stroke_seed(rank);
    let (total, step) = (geo.path.total, geo.path.step.max(0.5));
    let gaps = gap_profile(total, step, s, seed);
    let last = gaps.len() - 1;
    for y in 0..cur.h {
        for x in 0..cur.w {
            let c = cur.px[y * cur.w + x];
            if c.a() == 0 {
                continue;
            }
            let (ix, iy) = (cur.x0 + x as i32, cur.y0 + y as i32);
            let Some(hit) = geo.hit(ix, iy) else { continue };
            let (fx, fy) = (ix as f32 + 0.5, iy as f32 + 0.5);
            let off = (fx - hit.cx) * hit.nx + (fy - hit.cy) * hit.ny;
            // Le trou, lu entre les deux points de l'axe qui encadrent l'abscisse.
            let u = (hit.s / step).clamp(0.0, last as f32);
            let (i, t) = (u.floor() as usize, u - u.floor());
            let gap = lerp(gaps[i], gaps[(i + 1).min(last)], t);
            // Les stries : un bruit étiré le long du trait (des poils de brosse).
            let bristle = vnoise(seed ^ 0x1111, off / 1.25, hit.s / 36.0);
            let speck = unit(hash(seed, ix as u32, iy as u32, 5));
            let nn = 0.8 * bristle + 0.2 * speck;
            let mut f = 1.0 - gap * (1.0 - 0.6 * bristle * bristle);
            if s.rough > 0.005 {
                let edge = smooth(((off.abs() / hit.r.max(0.5)) - 0.35) / 0.65);
                f *= smooth(((nn - s.rough * edge * 0.85) / 0.25 + 1.0).clamp(0.0, 1.0));
            }
            cur.px[y * cur.w + x] = scaled(c, f.max(0.0));
        }
    }
}

// ---------------------------------------------------------------------- dégradé

/// Le dégradé d'un trait vectoriel, sur ses pixels `cur` : la couleur de chaque pixel change selon son
/// abscisse sur le trait (ou, sur tout le dessin, selon le rang du trait puis l'abscisse). La couverture
/// (la forme, la texture de la brosse) ne change pas.
pub(crate) fn gradient_stroke(cur: &mut Dense, geo: &Geo, g: &GradS, rank: usize, strokes: usize) {
    let total = geo.path.total;
    let n = strokes.max(1) as f32;
    for y in 0..cur.h {
        for x in 0..cur.w {
            let c = cur.px[y * cur.w + x];
            if c.a() == 0 {
                continue;
            }
            let Some(hit) = geo.hit(cur.x0 + x as i32, cur.y0 + y as i32) else { continue };
            let along = if total > 1e-3 { (hit.s / total).clamp(0.0, 1.0) } else { 0.5 };
            let t = if g.whole { (rank as f32 + along) / n } else { along };
            let col = g.color(t);
            let a = c.a() as f32;
            let k = 255.0 / a;
            let orig = [c.r() as f32 * k, c.g() as f32 * k, c.b() as f32 * k];
            let out = |i: usize| {
                let v = (orig[i] + (col[i] - orig[i]) * g.mix).clamp(0.0, 255.0);
                (v * a / 255.0).round().min(a) as u8
            };
            cur.px[y * cur.w + x] = Color32::from_rgba_premultiplied(out(0), out(1), out(2), c.a());
        }
    }
}

// ---------------------------------------------------------------------- les plumes

struct NibEntry {
    id: u32,
    name: String,
    /// Le réglage tel qu'un trait vectoriel le garde (pointe et papier désignés par leur clé de contenu).
    settings: BrushSettings,
    sig: u32,
}

static NIBS: RwLock<Vec<NibEntry>> = RwLock::new(Vec::new());

/// La plume calligraphique de SHOT : une plume plate inclinée, qui suit la pression.
pub fn calligraphy() -> BrushSettings {
    BrushSettings { size: 12.0, roundness: 0.22, angle: 38.0, spacing: 0.04, pressure_size: true, min_size: 0.3, ..BrushSettings::ink() }
}

/// Une signature de 24 bits d'un réglage de plume.
fn signature(s: &BrushSettings) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    if let Ok(bytes) = serde_json::to_vec(s) {
        for b in bytes {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
    }
    (h ^ (h >> 24)) & 0x00FF_FFFF
}

/// La bibliothèque de pinceaux publie ses réglages de pinceau (nom, réglage tel qu'un trait vectoriel le
/// garde) : l'effet « Changer de plume » les lit depuis le calcul de fond, qui n'a pas accès à
/// l'application. La plume calligraphique de SHOT est toujours là.
pub fn publish(brushes: &[(String, BrushSettings)]) {
    let mut list: Vec<NibEntry> = Vec::with_capacity(brushes.len() + 1);
    let mut add = |name: &str, settings: BrushSettings| {
        let id = name_id(name);
        if id != 0 && !list.iter().any(|e| e.id == id) {
            list.push(NibEntry { id, name: name.to_string(), sig: signature(&settings), settings });
        }
    };
    add(CALLIGRAPHY, calligraphy());
    for (name, s) in brushes {
        add(name, *s);
    }
    if let Ok(mut nibs) = NIBS.write() {
        *nibs = list;
    }
}

/// Les noms des plumes que l'effet peut choisir, avec leur numéro (la plume calligraphique d'abord).
pub fn nib_choices(brushes: &[String]) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = vec![(name_id(CALLIGRAPHY), CALLIGRAPHY.to_string())];
    for name in brushes {
        let id = name_id(name);
        if id != 0 && !out.iter().any(|(i, _)| *i == id) {
            out.push((id, name.clone()));
        }
    }
    out
}

/// Le réglage de la plume `id` et sa signature. Rien publié encore (un essai, un démarrage rapide) : les
/// pinceaux de SHOT sont fabriqués et publiés d'abord.
pub fn nib_settings(id: u32) -> Option<(BrushSettings, u32)> {
    let find = || NIBS.read().ok().and_then(|l| l.iter().find(|e| e.id == id).map(|e| (e.settings, e.sig)));
    if let Some(found) = find() {
        return Some(found);
    }
    static BUILTIN: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));
    {
        let mut done = BUILTIN.lock().ok()?;
        if !*done && NIBS.read().ok()?.is_empty() {
            *done = true;
            crate::brushtex::BrushLibrary::builtin().share();
        }
    }
    find()
}

/// La signature de la plume `id` (0 si elle n'existe pas ou n'est pas encore publiée).
pub fn nib_signature(id: u32) -> u32 {
    NIBS.read().ok().and_then(|l| l.iter().find(|e| e.id == id).map(|e| e.sig)).unwrap_or(0)
}

/// Le nom de la plume `id`, si elle est publiée (messages et essais).
pub fn nib_name(id: u32) -> Option<String> {
    NIBS.read().ok().and_then(|l| l.iter().find(|e| e.id == id).map(|e| e.name.clone()))
}

/// La plume qui redessine les traits d'un calcul : son réglage final, et les traits déjà redessinés.
pub struct NibCtx {
    settings: BrushSettings,
    /// Signature du réglage final (taille comprise).
    key: u64,
}

/// Traits redessinés gardés : (signature du trait, signature de la plume) → trait redessiné. Le rendu de
/// chaque trait (ses pixels) est gardé avec lui ; il est rejoué une fois, pas à chaque image.
static RESTYLED: LazyLock<Mutex<HashMap<(u64, u64), Arc<Element>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Au plus autant de traits redessinés gardés (leurs pixels sont aussi bornés par le budget du cache des
/// traits, `vector.rs`).
const RESTYLED_MAX: usize = 12_000;

impl NibCtx {
    pub fn resolve(n: &NibS) -> Option<NibCtx> {
        let (mut settings, _) = nib_settings(n.id)?;
        settings.size = (settings.size * n.size).clamp(0.5, 400.0);
        let mut key: u64 = 0xcbf2_9ce4_8422_2325;
        if let Ok(bytes) = serde_json::to_vec(&settings) {
            for b in bytes {
                key ^= b as u64;
                key = key.wrapping_mul(0x0100_0000_01b3);
            }
        }
        Some(NibCtx { settings, key })
    }

    /// Le trait `e` redessiné avec la plume : le même tracé, la même couleur, les mêmes appuis et instants.
    /// Rien pour une zone remplie ou un texte.
    pub fn build(&self, e: &Element) -> Option<Element> {
        let Element::Stroke { points, color, opacity, hardness, trace, .. } = e else { return None };
        // Le rayon de chaque point est celui que la plume dessine (la boîte du trait en dépend : un rayon
        // plus petit que la plume la couperait). Avec des appuis : taille de la plume × sa réponse à l'appui
        // (au plancher du toucher près) ; sans : le rayon d'origine, que la plume ne dépasse pas.
        let half = self.settings.size * 0.5;
        let connection = self.settings.size_connection();
        let floor = trace.feel.unwrap_or_default().size_floor;
        let aligned = trace.pressure.len() == points.len();
        let points: Vec<VPoint> = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let r = match trace.pressure.get(i).filter(|_| aligned) {
                    Some(&pressure) if pressure >= 0.0 => half * connection.map_or(1.0, |c| c.response(pressure)).max(floor) * 1.1,
                    _ => p.r.min(half * 1.1),
                };
                VPoint { r: r.max(0.3), ..*p }
            })
            .collect();
        Some(Element::Stroke {
            points,
            color: *color,
            opacity: *opacity,
            hardness: *hardness,
            brush: Some(Box::new(self.settings)),
            trace: trace.without_cache(),
        })
    }

    /// Comme `build`, gardé : le même trait avec la même plume est redessiné une seule fois.
    pub fn restyle(&self, e: &Element) -> Option<Arc<Element>> {
        let key = (stroke_key(e)?, self.key);
        if let Some(found) = RESTYLED.lock().ok().and_then(|m| m.get(&key).cloned()) {
            return Some(found);
        }
        let built = Arc::new(self.build(e)?);
        if let Ok(mut m) = RESTYLED.lock() {
            if m.len() >= RESTYLED_MAX {
                m.clear();
            }
            m.insert(key, built.clone());
        }
        Some(built)
    }
}

/// Ce qui fait le tracé d'un trait (points, couleur, appuis, instants, toucher) : pas son pinceau, que la
/// plume remplace.
fn stroke_key(e: &Element) -> Option<u64> {
    let Element::Stroke { points, color, opacity, hardness, trace, .. } = e else { return None };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u64| {
        h ^= b;
        h = h.wrapping_mul(0x0100_0000_01b3);
        h ^= h >> 29;
    };
    eat(points.len() as u64);
    for p in points {
        eat(p.x.to_bits() as u64 | ((p.y.to_bits() as u64) << 32));
        eat(p.r.to_bits() as u64);
    }
    eat(u32::from_le_bytes([color[0], color[1], color[2], 0]) as u64);
    eat(opacity.to_bits() as u64 | ((hardness.to_bits() as u64) << 32));
    eat(trace.pressure.len() as u64);
    for v in &trace.pressure {
        eat(v.to_bits() as u64);
    }
    eat(trace.time.len() as u64);
    for t in &trace.time {
        eat(t.to_bits());
    }
    if let Some(b) = trace.secondary {
        eat(u32::from_le_bytes([b[0], b[1], b[2], 1]) as u64);
    }
    if let Some(f) = &trace.feel {
        if let Ok(bytes) = serde_json::to_vec(f) {
            for b in bytes {
                eat(b as u64);
            }
        }
    }
    Some(h)
}

/// Remplace pour de bon la plume de tous les traits de `drawing` (zones remplies et textes : tels quels).
/// Renvoie combien de traits ont changé.
pub fn bake_nib(drawing: &mut VectorDrawing, nib: &NibCtx) -> usize {
    let mut changed = 0;
    for e in &mut drawing.elements {
        if let Some(arc) = nib.restyle(e) {
            *e = (*arc).clone();
            changed += 1;
        }
    }
    changed
}
