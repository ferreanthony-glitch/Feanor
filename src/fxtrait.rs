//! Effets du trait (Anthony, 2026-10-20 : « le tremblement, j'adore ; encore des effets qui agissent
//! sur le trait lui-même »). Une famille de la pile d'effets d'un calque vectoriel, au même titre
//! que le tremblement (`tremblement.rs`) : réglages enregistrés avec le calque, clés, copier-coller,
//! calcul en arrière-plan, cache des dessins dérivés, export et vignettes.
//!
//! - **Écriture du trait** : le dessin se trace de lui-même, trait par trait (dans l'ordre où ils ont
//!   été dessinés) ou tous ensemble ; en sens inverse, il s'efface. Avancement animable.
//! - **Crayonné multiple** : chaque trait est redoublé de passes légèrement décalées, comme un
//!   dessin repris au crayon ; elles sont retirées au hasard toutes les N images.
//! - **Respiration** : l'épaisseur du trait pulse (amplitude, période), ou suit les clés.
//! - **Ondulation du trait** : une vague qui court le long de chaque trait.
//! - **Pointillés** : le trait devient une suite de tirets qui défilent.
//! - **Désagrégation** : le trait se défait en poussière, avancement animable.
//!
//! # Choix de conception
//!
//! - **Des effets sur les traits, pas sur l'image** : ils lisent la géométrie du dessin vectoriel
//!   (`VectorDrawing`), que le fil de fond reçoit avec le dessin (`fxcache::Job`), et rendent une
//!   image qui prend la place du dessin pour les effets d'après (flous, couleur…). Les dessins ne
//!   changent jamais.
//! - **Sur les pixels de chaque trait, pas sur un rejeu du moteur de brosses.** Chaque trait est
//!   d'abord rendu seul — depuis les pixels exacts que son cache garde (`vector.rs`) — puis transformé
//!   en pixels : la texture de la brosse suit la déformation. Le moteur n'est donc rejoué qu'une fois
//!   par trait (en arrière-plan, comme à l'ouverture d'un projet), jamais à chaque image : une gouache
//!   qui demande des secondes de rejeu coûte la même chose que tout autre trait à chaque image.
//! - **L'axe du trait** guide les transformations : il est rééchantillonné tous les 2 pixels
//!   (`PathIndex`), et une carte du « point de l'axe le plus proche » (`NearMap`, une seule fois par
//!   trait) donne à chaque pixel son abscisse curviligne, sa normale et le rayon du trait en ce
//!   point. Écriture et pointillés lisent l'abscisse ; l'ondulation déplace selon la normale ; la
//!   respiration remet le pixel à l'échelle autour de l'axe.
//! - **Ordre dans un trait** : respiration, ondulation, pointillés, écriture, désagrégation, puis
//!   crayonné (qui redouble le résultat). Les zones remplies et les textes ne sont pas des traits :
//!   ils restent tels quels, sauf si l'écriture ou la désagrégation a son réglage « zones remplies ».
//! - **Déterministe** : même image, mêmes pixels. Le hasard vient d'un hachage entier de (graine,
//!   trait, passe, variante) ; le travail se répartit sur les cœurs par morceaux de traits, mais les
//!   traits sont posés dans leur ordre, un à un, par le fil qui appelle.
//! - **Sans latence** : jamais sur le fil de l'interface (`fxcache.rs`). Un trait que l'effet ne
//!   touche pas (écriture : déjà tracé ou pas encore commencé) est posé tel quel depuis son cache ou
//!   sauté ; à 50 % d'une écriture « l'un après l'autre » un seul trait est vraiment transformé.
//! - **Réparti sur les cœurs** : les traits d'un morceau de 96 éléments sont transformés en parallèle
//!   (`parallele::par_map`), puis posés par bandes de tuiles, chaque bande recevant les couches dans
//!   l'ordre (`compose_banded`) : le même pixel qu'avec un seul fil. Le fil de fond laisse deux cœurs
//!   à l'interface (`render::thread_cap`).
//! - **Carte graphique** : ces effets lisent des traits, pas des pixels : ils restent un calcul de fond.
//!   Quand la pile mêle des effets du trait et des effets de la carte, elle est coupée en deux
//!   (`fx::hybrid_split`) : le fond calcule le dessin dérivé par les effets du trait, la carte passe le
//!   reste dessus (flou, couleur, tremblement…) à pleine résolution, à chaque image.
//! - Pas d'aperçu réduit (`fxproxy.rs`) : les longueurs sont en pixels de la feuille.
//! - **Limites** : les zones remplies et les textes ne sont pas des traits (réglage « zones remplies »
//!   de l'écriture et de la désagrégation) ; un fond sous le trait (`fond.rs`) reste fixe ; la forme d'un
//!   trait se lit sur ses pixels (une brosse très large qui se recoupe elle-même donne l'abscisse du
//!   point le plus proche de l'axe, pas celle de l'ordre où la plume a posé ses empreintes) ; seul le
//!   tremblement « anime à l'arrêt » (les autres suivent l'image courante).

use std::f32::consts::TAU;

use eframe::egui::Color32;

use crate::fx::{EffectKind, FxStep, Param};
use crate::raster::{Raster, TILE};
use crate::vector::{render_element, Element, VPoint, VectorDrawing};

// ---------------------------------------------------------------------- les réglages

/// Rang des réglages de l'écriture du trait (`fx::EffectKind::Write`).
pub const WRITE_PROGRESS: usize = 0;
pub const WRITE_ERASE: usize = 1;
pub const WRITE_TOGETHER: usize = 2;
pub const WRITE_BY_LENGTH: usize = 3;
pub const WRITE_EASE: usize = 4;
pub const WRITE_SOFT: usize = 5;
pub const WRITE_FILLS: usize = 6;

pub const SKETCH_PASSES: usize = 0;
pub const SKETCH_OFFSET: usize = 1;
pub const SKETCH_OPACITY: usize = 2;
pub const SKETCH_NERVE: usize = 3;
pub const SKETCH_VARIANTS: usize = 4;
pub const SKETCH_SPEED: usize = 5;
pub const SKETCH_SEED: usize = 6;
/// La variante montrée, ajoutée par `fx::steps_at` (comme celle du tremblement).
pub const SKETCH_VARIANT: usize = 7;

pub const BREATH_THICKNESS: usize = 0;
pub const BREATH_AMPLITUDE: usize = 1;
pub const BREATH_PERIOD: usize = 2;
pub const BREATH_WAVE: usize = 3;
/// L'image, ajoutée par `fx::steps_at` (l'effet bat avec le temps).
pub const BREATH_FRAME: usize = 4;

pub const WAVE_AMPLITUDE: usize = 0;
pub const WAVE_LENGTH: usize = 1;
pub const WAVE_SPEED: usize = 2;
pub const WAVE_FIXED: usize = 3;
pub const WAVE_FRAME: usize = 4;

pub const DASH_LENGTH: usize = 0;
pub const DASH_GAP: usize = 1;
pub const DASH_SPEED: usize = 2;
pub const DASH_SOFT: usize = 3;
pub const DASH_FRAME: usize = 4;

pub const DUST_PROGRESS: usize = 0;
pub const DUST_GRAIN: usize = 1;
pub const DUST_SPREAD: usize = 2;
pub const DUST_SENSE: usize = 3;
pub const DUST_ANGLE: usize = 4;
pub const DUST_DRIFT: usize = 5;
pub const DUST_SEED: usize = 6;
pub const DUST_FILLS: usize = 7;

pub(crate) const fn p(name: &'static str, min: f32, max: f32, default: f32, unit: &'static str) -> Param {
    Param { name, min, max, default, unit }
}

pub(crate) const WRITE: [Param; 7] = [
    p("Avancement", 0.0, 100.0, 60.0, " %"),
    p("Effacement (au lieu de l'écriture)", 0.0, 1.0, 0.0, ""),
    p("Tous les traits ensemble", 0.0, 1.0, 0.0, ""),
    p("Durée selon la longueur", 0.0, 1.0, 1.0, ""),
    p("Douceur du rythme", 0.0, 100.0, 0.0, " %"),
    p("Bord doux", 0.0, 60.0, 0.0, " px"),
    p("Zones remplies avec le trait", 0.0, 1.0, 0.0, ""),
];
pub(crate) const SKETCH: [Param; 7] = [
    p("Passes en plus", 1.0, 4.0, 2.0, ""),
    p("Décalage", 0.0, 20.0, 2.5, " px"),
    p("Opacité des passes", 0.0, 100.0, 60.0, " %"),
    p("Nervosité", 0.0, 100.0, 50.0, " %"),
    p("Variantes", 2.0, 6.0, 3.0, ""),
    p("Vitesse", 1.0, 12.0, 2.0, " img/var."),
    p("Graine", 1.0, 999.0, 1.0, ""),
];
pub(crate) const BREATH: [Param; 4] = [
    p("Épaisseur", 25.0, 300.0, 100.0, " %"),
    p("Amplitude", 0.0, 100.0, 35.0, " %"),
    p("Période", 2.0, 120.0, 24.0, " img"),
    p("Vague le long du trait", 0.0, 600.0, 0.0, " px"),
];
pub(crate) const WAVE: [Param; 4] = [
    p("Amplitude", 0.0, 40.0, 4.0, " px"),
    p("Longueur d'onde", 6.0, 400.0, 60.0, " px"),
    p("Vitesse", -60.0, 60.0, 4.0, " px/img"),
    p("Bouts fixes", 0.0, 1.0, 1.0, ""),
];
pub(crate) const DASH: [Param; 4] = [
    p("Tiret", 1.0, 300.0, 18.0, " px"),
    p("Intervalle", 1.0, 300.0, 12.0, " px"),
    p("Vitesse", -80.0, 80.0, 3.0, " px/img"),
    p("Bord doux", 0.0, 12.0, 1.0, " px"),
];
pub(crate) const DUST: [Param; 8] = [
    p("Avancement", 0.0, 100.0, 40.0, " %"),
    p("Taille des grains", 1.0, 24.0, 4.0, " px"),
    p("Dispersion", 0.0, 600.0, 80.0, " px"),
    p("Sens du trait", 0.0, 100.0, 50.0, " %"),
    p("Dérive : direction", -180.0, 180.0, -90.0, "°"),
    p("Dérive : force", 0.0, 300.0, 40.0, " px"),
    p("Graine", 1.0, 999.0, 1.0, ""),
    p("Zones remplies avec la poussière", 0.0, 1.0, 0.0, ""),
];

pub(crate) fn at(v: &[f32], k: usize, default: f32) -> f32 {
    v.get(k).copied().filter(|x| x.is_finite()).unwrap_or(default)
}

pub(crate) fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// L'écriture du trait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WriteS {
    /// 0 à 1.
    pub progress: f32,
    pub erase: bool,
    pub together: bool,
    pub by_length: bool,
    pub ease: f32,
    pub soft: f32,
    pub fills: bool,
}

impl WriteS {
    pub fn of(v: &[f32]) -> WriteS {
        WriteS {
            progress: (at(v, WRITE_PROGRESS, 60.0) / 100.0).clamp(0.0, 1.0),
            erase: at(v, WRITE_ERASE, 0.0) >= 0.5,
            together: at(v, WRITE_TOGETHER, 0.0) >= 0.5,
            by_length: at(v, WRITE_BY_LENGTH, 1.0) >= 0.5,
            ease: (at(v, WRITE_EASE, 0.0) / 100.0).clamp(0.0, 1.0),
            soft: at(v, WRITE_SOFT, 0.0).clamp(0.0, 60.0),
            fills: at(v, WRITE_FILLS, 0.0) >= 0.5,
        }
    }

    /// L'avancement après le rythme : de la ligne droite (0 %) à la courbe en S (100 %).
    pub fn eased(&self) -> f32 {
        self.progress + (smooth(self.progress) - self.progress) * self.ease
    }

    /// L'écriture change-t-elle quelque chose ? (Écrite en entier, ou effacement à zéro : non.)
    pub fn changes(&self) -> bool {
        if self.erase { self.progress > 0.0005 } else { self.progress < 0.9995 }
    }
}

/// Le crayonné multiple.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchS {
    pub passes: usize,
    pub offset: f32,
    /// 0 à 1.
    pub opacity: f32,
    pub nerve: f32,
    pub variants: usize,
    pub speed: usize,
    pub seed: u32,
    pub variant: usize,
}

impl SketchS {
    pub fn of(v: &[f32]) -> SketchS {
        let variants = (at(v, SKETCH_VARIANTS, 3.0).round() as i64).clamp(2, 6) as usize;
        SketchS {
            passes: (at(v, SKETCH_PASSES, 2.0).round() as i64).clamp(1, 4) as usize,
            offset: at(v, SKETCH_OFFSET, 2.5).clamp(0.0, 20.0),
            opacity: (at(v, SKETCH_OPACITY, 60.0) / 100.0).clamp(0.0, 1.0),
            nerve: at(v, SKETCH_NERVE, 50.0).clamp(0.0, 100.0),
            variants,
            speed: (at(v, SKETCH_SPEED, 2.0).round() as i64).clamp(1, 12) as usize,
            seed: (at(v, SKETCH_SEED, 1.0).round() as i64).clamp(1, 999) as u32,
            variant: (at(v, SKETCH_VARIANT, 0.0).round().max(0.0) as usize) % variants,
        }
    }

    pub fn changes(&self) -> bool {
        self.offset >= 0.05 && self.opacity > 0.0
    }
}

/// La variante du crayonné à l'image `frame` (comme celle du tremblement).
pub fn sketch_variant(values: &[f32], frame: usize) -> usize {
    let s = SketchS::of(values);
    crate::tremblement::variant_at(frame, s.speed, s.variants)
}

/// La respiration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreathS {
    /// Facteur d'épaisseur de base (1 = inchangée).
    pub thickness: f32,
    /// 0 à 1.
    pub amplitude: f32,
    pub period: f32,
    pub wave: f32,
    pub frame: f32,
}

impl BreathS {
    pub fn of(v: &[f32]) -> BreathS {
        BreathS {
            thickness: (at(v, BREATH_THICKNESS, 100.0) / 100.0).clamp(0.25, 3.0),
            amplitude: (at(v, BREATH_AMPLITUDE, 35.0) / 100.0).clamp(0.0, 1.0),
            period: at(v, BREATH_PERIOD, 24.0).clamp(2.0, 120.0),
            wave: at(v, BREATH_WAVE, 0.0).clamp(0.0, 600.0),
            frame: at(v, BREATH_FRAME, 0.0).max(0.0),
        }
    }

    pub fn changes(&self) -> bool {
        (self.thickness - 1.0).abs() > 0.005 || self.amplitude > 0.005
    }

    /// L'épaisseur la plus grande qu'atteint le trait (facteur).
    fn widest(&self) -> f32 {
        self.thickness * (1.0 + self.amplitude)
    }
}

/// L'ondulation le long du trait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveS {
    pub amplitude: f32,
    pub length: f32,
    pub speed: f32,
    pub fixed: bool,
    pub frame: f32,
}

impl WaveS {
    pub fn of(v: &[f32]) -> WaveS {
        WaveS {
            amplitude: at(v, WAVE_AMPLITUDE, 4.0).clamp(0.0, 40.0),
            length: at(v, WAVE_LENGTH, 60.0).clamp(6.0, 400.0),
            speed: at(v, WAVE_SPEED, 4.0).clamp(-60.0, 60.0),
            fixed: at(v, WAVE_FIXED, 1.0) >= 0.5,
            frame: at(v, WAVE_FRAME, 0.0).max(0.0),
        }
    }

    pub fn changes(&self) -> bool {
        self.amplitude > 0.05
    }
}

/// Les pointillés qui défilent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DashS {
    pub dash: f32,
    pub gap: f32,
    pub speed: f32,
    pub soft: f32,
    pub frame: f32,
}

impl DashS {
    pub fn of(v: &[f32]) -> DashS {
        DashS {
            dash: at(v, DASH_LENGTH, 18.0).clamp(1.0, 300.0),
            gap: at(v, DASH_GAP, 12.0).clamp(1.0, 300.0),
            speed: at(v, DASH_SPEED, 3.0).clamp(-80.0, 80.0),
            soft: at(v, DASH_SOFT, 1.0).clamp(0.0, 12.0),
            frame: at(v, DASH_FRAME, 0.0).max(0.0),
        }
    }

    /// Le tiret à l'abscisse `s` du trait : 1 dans un tiret, 0 dans l'intervalle, un bord adouci
    /// d'au moins un pixel.
    pub fn factor(&self, s: f32) -> f32 {
        let period = self.dash + self.gap;
        let u = (s - self.speed * self.frame).rem_euclid(period);
        let e = self.soft.max(1.0);
        smooth(u / e) * smooth((self.dash - u) / e)
    }
}

/// La désagrégation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DustS {
    /// 0 à 1.
    pub progress: f32,
    pub grain: i32,
    pub spread: f32,
    pub sense: f32,
    pub angle: f32,
    pub drift: f32,
    pub seed: u32,
    pub fills: bool,
}

impl DustS {
    pub fn of(v: &[f32]) -> DustS {
        DustS {
            progress: (at(v, DUST_PROGRESS, 40.0) / 100.0).clamp(0.0, 1.0),
            grain: (at(v, DUST_GRAIN, 4.0).round() as i32).clamp(1, 24),
            spread: at(v, DUST_SPREAD, 80.0).clamp(0.0, 600.0),
            sense: (at(v, DUST_SENSE, 50.0) / 100.0).clamp(0.0, 1.0),
            angle: at(v, DUST_ANGLE, -90.0).clamp(-180.0, 180.0).to_radians(),
            drift: at(v, DUST_DRIFT, 40.0).clamp(0.0, 300.0),
            seed: (at(v, DUST_SEED, 1.0).round() as i64).clamp(1, 999) as u32,
            fills: at(v, DUST_FILLS, 0.0) >= 0.5,
        }
    }

    pub fn changes(&self) -> bool {
        self.progress > 0.0005
    }
}

/// Les effets du trait d'une pile, à une image : au plus un de chaque sorte (le premier).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stack {
    pub write: Option<WriteS>,
    pub sketch: Option<SketchS>,
    pub breath: Option<BreathS>,
    pub wave: Option<WaveS>,
    pub dash: Option<DashS>,
    pub dust: Option<DustS>,
    // Effets de matière qui lisent les traits (`fxmatiere.rs`).
    pub dry: Option<crate::fxmatiere::DryS>,
    pub gradient: Option<crate::fxmatiere::GradS>,
    pub nib: Option<crate::fxmatiere::NibS>,
}

impl Stack {
    /// Les effets du trait de `steps` qui changent quelque chose.
    pub fn of(steps: &[FxStep]) -> Stack {
        use crate::fxmatiere as m;
        let mut st = Stack::default();
        for s in steps {
            let v = &s.values;
            match s.kind {
                EffectKind::Write if st.write.is_none() => st.write = Some(WriteS::of(v)).filter(WriteS::changes),
                EffectKind::Sketch if st.sketch.is_none() => st.sketch = Some(SketchS::of(v)).filter(SketchS::changes),
                EffectKind::Breath if st.breath.is_none() => st.breath = Some(BreathS::of(v)).filter(BreathS::changes),
                EffectKind::StrokeWave if st.wave.is_none() => st.wave = Some(WaveS::of(v)).filter(WaveS::changes),
                EffectKind::Dashes if st.dash.is_none() => st.dash = Some(DashS::of(v)),
                EffectKind::Dust if st.dust.is_none() => st.dust = Some(DustS::of(v)).filter(DustS::changes),
                EffectKind::Dry if st.dry.is_none() => st.dry = Some(m::DryS::of(v)).filter(m::DryS::changes),
                EffectKind::StrokeGradient if st.gradient.is_none() => st.gradient = Some(m::GradS::of(v)).filter(m::GradS::changes),
                EffectKind::Nib if st.nib.is_none() => st.nib = Some(m::NibS::of(v)).filter(m::NibS::changes),
                _ => {}
            }
        }
        st
    }

    pub fn is_empty(&self) -> bool {
        self.write.is_none()
            && self.sketch.is_none()
            && self.breath.is_none()
            && self.wave.is_none()
            && self.dash.is_none()
            && self.dust.is_none()
            && self.dry.is_none()
            && self.gradient.is_none()
            && self.nib.is_none()
    }

    /// Un effet autre que l'écriture touche chaque trait.
    fn transforms(&self) -> bool {
        self.sketch.is_some()
            || self.breath.is_some()
            || self.wave.is_some()
            || self.dash.is_some()
            || self.dust.is_some()
            || self.dry.is_some()
            || self.gradient.is_some()
            || self.nib.is_some()
    }
}

// ---------------------------------------------------------------------- noms, bulles, préréglages

/// Les effets du trait, dans l'ordre où les menus et la rangée d'icônes les montrent.
pub const KINDS: [EffectKind; 6] = [EffectKind::Write, EffectKind::Sketch, EffectKind::Breath, EffectKind::StrokeWave, EffectKind::Dashes, EffectKind::Dust];

/// L'icône d'un effet du trait.
pub fn icon(kind: EffectKind) -> &'static str {
    use egui_phosphor::regular as ph;
    match kind {
        EffectKind::Write => ph::SIGNATURE,
        EffectKind::Sketch => ph::SCRIBBLE,
        EffectKind::Breath => ph::HEARTBEAT,
        EffectKind::StrokeWave => ph::WAVE_SAWTOOTH,
        EffectKind::Dashes => ph::DOTS_THREE,
        EffectKind::Dust => ph::GRAINS,
        // Le tremblement du trait (`tremblement.rs`), en tête du menu « Trait ».
        EffectKind::Boil => ph::WAVE_SINE,
        _ => crate::fxmatiere::icon(kind),
    }
}

/// L'action (`keymap.toml`) qui coche et décoche l'effet sur le calque courant.
pub fn action(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Write => "fx.ecriture",
        EffectKind::Sketch => "fx.crayonne",
        EffectKind::Breath => "fx.respiration",
        EffectKind::StrokeWave => "fx.ondulation_trait",
        EffectKind::Dashes => "fx.pointilles",
        EffectKind::Dust => "fx.desagregation",
        EffectKind::Boil => "fx.tremblement",
        _ => "",
    }
}

/// Les entrées du menu déroulant « Trait » : le tremblement du trait d'abord (il est de la même famille),
/// puis les effets du trait (`KINDS`). `KINDS` ne change pas : les enregistrements et les essais le suivent.
pub fn menu_kinds() -> [EffectKind; 7] {
    let [a, b, c, d, e, f] = KINDS;
    [EffectKind::Boil, a, b, c, d, e, f]
}

/// L'effet d'une action de `action`.
pub fn kind_of_action(action_id: &str) -> Option<EffectKind> {
    KINDS.into_iter().find(|k| action(*k) == action_id)
}

/// Ce que fait l'effet, en une ou deux phrases (bulles).
pub fn hint(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Write => "Le dessin se trace de lui-même, trait par trait, dans l'ordre où ils ont été dessinés (ou tous ensemble). Une clé d'avancement à 0 % puis une à 100 % : le dessin s'écrit. Au sens inverse, il s'efface.",
        EffectKind::Sketch => "Chaque trait est redoublé de passes un peu décalées, comme un dessin repris au crayon ; elles sont retirées au hasard toutes les N images. Le trait d'origine reste dessus, intact.",
        EffectKind::Breath => "L'épaisseur du trait pulse (amplitude, période), avec en option une vague qui court le long du trait. L'épaisseur de base suit aussi les clés.",
        EffectKind::StrokeWave => "Une vague court le long de chaque trait : il ondule de part et d'autre de sa ligne, les bouts restent en place.",
        EffectKind::Dashes => "Le trait devient une suite de tirets qui défilent le long de lui-même.",
        EffectKind::Dust => "Le trait se défait en poussière, grain par grain. Avancement animable : pour une disparition ou une transition. Le hasard est fixé par la graine.",
        _ => crate::fxmatiere::hint(kind),
    }
}

/// Ce que règle le paramètre `k` de l'effet.
pub fn param_hint(kind: EffectKind, k: usize) -> &'static str {
    match (kind, k) {
        (EffectKind::Write, WRITE_PROGRESS) => "La part du dessin déjà tracée. Une clé à 0 %, une à 100 % : le dessin s'écrit.",
        (EffectKind::Write, WRITE_ERASE) => "Coché, le dessin s'efface dans le même ordre au lieu de s'écrire (100 % : plus rien).",
        (EffectKind::Write, WRITE_TOGETHER) => "Coché, tous les traits avancent en même temps, chacun sur toute sa longueur. Décoché : l'un après l'autre.",
        (EffectKind::Write, WRITE_BY_LENGTH) => "Coché, un trait long prend plus de temps qu'un court (la plume va toujours à la même vitesse). Décoché : chaque trait prend le même temps. Sans effet quand les traits avancent ensemble.",
        (EffectKind::Write, WRITE_EASE) => "0 % : l'écriture va à vitesse constante. 100 % : elle démarre et finit doucement.",
        (EffectKind::Write, WRITE_SOFT) => "Longueur de la pointe qui se fond. 0 : coupure nette (adoucie d'un pixel).",
        (EffectKind::Write, WRITE_FILLS) => "Coché, les zones de couleur et les textes apparaissent en fondu à la fin de l'écriture (ou partent au début d'un effacement). Décoché : ils restent.",
        (EffectKind::Sketch, SKETCH_PASSES) => "Combien de passes en plus du trait d'origine.",
        (EffectKind::Sketch, SKETCH_OFFSET) => "Écart le plus grand des passes, en pixels de la feuille.",
        (EffectKind::Sketch, SKETCH_OPACITY) => "Opacité des passes : le trait d'origine reste plein.",
        (EffectKind::Sketch, SKETCH_NERVE) => "0 : les passes ondulent doucement le long du trait. 100 : elles tremblent par petits coups.",
        (EffectKind::Sketch, SKETCH_VARIANTS) => "Nombre de crayonnés différents qui se suivent.",
        (EffectKind::Sketch, SKETCH_SPEED) => "Images du projet pour chaque crayonné : 2 images à 24 i/s font douze changements par seconde.",
        (EffectKind::Sketch, SKETCH_SEED) => "Change le tirage au hasard : un autre crayonné.",
        (EffectKind::Breath, BREATH_THICKNESS) => "Épaisseur de base du trait, en pourcentage de l'épaisseur dessinée. Se met en clés.",
        (EffectKind::Breath, BREATH_AMPLITUDE) => "De combien l'épaisseur pulse autour de sa base. 0 : elle ne pulse pas.",
        (EffectKind::Breath, BREATH_PERIOD) => "Images pour une pulsation complète.",
        (EffectKind::Breath, BREATH_WAVE) => "Longueur d'une vague de pulsation qui court le long du trait. 0 : tout le trait respire ensemble.",
        (EffectKind::StrokeWave, WAVE_AMPLITUDE) => "De combien le trait s'écarte de sa ligne, en pixels.",
        (EffectKind::StrokeWave, WAVE_LENGTH) => "Distance entre deux crêtes de la vague, en pixels.",
        (EffectKind::StrokeWave, WAVE_SPEED) => "Pixels parcourus par la vague à chaque image. Négatif : elle va dans l'autre sens.",
        (EffectKind::StrokeWave, WAVE_FIXED) => "Coché, la vague s'éteint vers les bouts du trait, qui restent en place.",
        (EffectKind::Dashes, DASH_LENGTH) => "Longueur d'un tiret, en pixels le long du trait.",
        (EffectKind::Dashes, DASH_GAP) => "Longueur de l'intervalle entre deux tirets.",
        (EffectKind::Dashes, DASH_SPEED) => "Pixels parcourus par les tirets à chaque image. Négatif : ils reculent.",
        (EffectKind::Dashes, DASH_SOFT) => "Adoucit le bout des tirets (au moins un pixel).",
        (EffectKind::Dust, DUST_PROGRESS) => "La part du trait déjà défaite. Une clé à 0 %, une à 100 % : le trait part en poussière.",
        (EffectKind::Dust, DUST_GRAIN) => "Côté des grains, en pixels : petits, la poussière est fine.",
        (EffectKind::Dust, DUST_SPREAD) => "Jusqu'où les grains s'éparpillent, en pixels.",
        (EffectKind::Dust, DUST_SENSE) => "0 % : les grains partent au hasard. 100 % : le trait se défait du début vers la fin.",
        (EffectKind::Dust, DUST_ANGLE) => "Direction où le vent pousse la poussière (0° : vers la droite, -90° : vers le haut).",
        (EffectKind::Dust, DUST_DRIFT) => "Force du vent, en pixels.",
        (EffectKind::Dust, DUST_SEED) => "Change le tirage au hasard : une autre poussière.",
        (EffectKind::Dust, DUST_FILLS) => "Coché, les zones de couleur et les textes s'effacent peu à peu avec le trait. Décoché : ils restent.",
        (EffectKind::Boil, crate::tremblement::REST) => "Au repos, le dessin continue de trembler (l'écran se redessine à la cadence du tremblement). Éteint, l'image fixe montre la variante de l'image courante.",
        _ => crate::fxmatiere::param_hint(kind, k),
    }
}

/// Un préréglage du trait : des effets tout réglés, avec leurs clés à partir de la tête de lecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Le dessin s'écrit en une seconde.
    Appearance,
    /// Un crayonné qui bouge, avec un léger tremblement.
    NervousSketch,
    /// Le dessin part en poussière en une seconde.
    Dust,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Appearance, Preset::NervousSketch, Preset::Dust];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Appearance => "Apparition écrite",
            Preset::NervousSketch => "Croquis nerveux",
            Preset::Dust => "Poussière",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Preset::Appearance => "Le dessin s'écrit trait par trait en une seconde, à partir de l'image courante (écriture du trait, clés à 0 % puis 100 %).",
            Preset::NervousSketch => "Chaque trait est redoublé de deux passes qui se retirent toutes les deux images, avec un léger tremblement du trait par-dessus.",
            Preset::Dust => "Le dessin part en poussière en une seconde, à partir de l'image courante (désagrégation, clés à 0 % puis 100 %).",
        }
    }

    pub fn action(self) -> &'static str {
        match self {
            Preset::Appearance => "fx.preset_apparition",
            Preset::NervousSketch => "fx.preset_croquis",
            Preset::Dust => "fx.preset_poussiere",
        }
    }

    pub fn from_action(action_id: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.action() == action_id)
    }

    /// Les effets du préréglage pour un plan de `images` images à `fps` images par seconde, la tête de
    /// lecture à l'image `start`.
    pub fn build(self, fps: u32, start: usize, images: usize) -> Vec<crate::fx::Effect> {
        use crate::fx::Effect;
        let with = |kind: EffectKind, set: &[(usize, f32)]| {
            let mut e = Effect::new(kind);
            for &(k, v) in set {
                e.set_param(k, v);
            }
            e
        };
        // Une seconde, sans dépasser le plan.
        let end = (start + fps.max(2) as usize).min(images.saturating_sub(1)).max(start + 1);
        let keyed = |kind: EffectKind, k: usize, set: &[(usize, f32)]| {
            let mut e = with(kind, set);
            let (mut from, mut to) = (e.values.clone(), e.values.clone());
            from[k] = 0.0;
            to[k] = 100.0;
            e.set_key(start, from);
            e.set_key(end, to);
            e
        };
        match self {
            Preset::Appearance => vec![keyed(EffectKind::Write, WRITE_PROGRESS, &[(WRITE_SOFT, 6.0), (WRITE_EASE, 35.0)])],
            Preset::NervousSketch => vec![
                with(EffectKind::Sketch, &[(SKETCH_PASSES, 2.0), (SKETCH_OFFSET, 3.0), (SKETCH_OPACITY, 55.0), (SKETCH_NERVE, 70.0), (SKETCH_SPEED, 2.0)]),
                with(EffectKind::Boil, &[(crate::tremblement::INTENSITY, 1.5), (crate::tremblement::FREQUENCY, 60.0)]),
            ],
            Preset::Dust => vec![keyed(EffectKind::Dust, DUST_PROGRESS, &[(DUST_GRAIN, 3.0), (DUST_SPREAD, 140.0), (DUST_SENSE, 70.0), (DUST_ANGLE, -70.0), (DUST_DRIFT, 60.0)])],
        }
    }
}

// ---------------------------------------------------------------------- le hasard

pub(crate) fn mix(h: u32) -> u32 {
    crate::tremblement::mix(h)
}

pub(crate) fn hash(seed: u32, a: u32, b: u32, k: u32) -> u32 {
    mix(seed ^ mix(a.wrapping_mul(0x8DA6_B343) ^ mix(b.wrapping_mul(0xD816_3841) ^ mix(k.wrapping_mul(0x9E37_79B9)))))
}

/// Un nombre de [0, 1) tiré d'un hachage.
pub(crate) fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

// ---------------------------------------------------------------------- pixels d'un trait

/// Les pixels d'un trait (prémultipliés) sur le rectangle [x0, x0 + w) × [y0, y0 + h) de la feuille.
#[derive(Clone)]
pub(crate) struct Dense {
    pub(crate) x0: i32,
    pub(crate) y0: i32,
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) px: Vec<Color32>,
}

impl Dense {
    fn blank(x0: i32, y0: i32, w: usize, h: usize) -> Dense {
        Dense { x0, y0, w, h, px: vec![Color32::TRANSPARENT; w * h] }
    }

    /// La boîte exacte de l'encre de `r` (tuiles et pixels vides ignorés).
    fn from_raster(r: &Raster) -> Option<Dense> {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        for (idx, tile) in r.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            let (ox, oy, tw, th) = r.tile_bounds(idx);
            for y in 0..th {
                let row = &tile[y * TILE..y * TILE + tw];
                if let Some(a) = row.iter().position(|c| c.a() != 0) {
                    let b = row.iter().rposition(|c| c.a() != 0).unwrap_or(a);
                    x0 = x0.min(ox + a);
                    x1 = x1.max(ox + b + 1);
                    y0 = y0.min(oy + y);
                    y1 = y1.max(oy + y + 1);
                }
            }
        }
        if x0 == usize::MAX {
            return None;
        }
        let mut d = Dense::blank(x0 as i32, y0 as i32, x1 - x0, y1 - y0);
        for (idx, tile) in r.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            let (ox, oy, tw, th) = r.tile_bounds(idx);
            let (cx0, cx1) = (ox.max(x0), (ox + tw).min(x1));
            let (cy0, cy1) = (oy.max(y0), (oy + th).min(y1));
            if cx0 >= cx1 || cy0 >= cy1 {
                continue;
            }
            for y in cy0..cy1 {
                let src = &tile[(y - oy) * TILE + (cx0 - ox)..(y - oy) * TILE + (cx1 - ox)];
                let at = (y - y0) * d.w + (cx0 - x0);
                d.px[at..at + src.len()].copy_from_slice(src);
            }
        }
        Some(d)
    }

    fn bounds(&self) -> (i32, i32, i32, i32) {
        (self.x0, self.y0, self.x0 + self.w as i32, self.y0 + self.h as i32)
    }

    fn get(&self, x: i32, y: i32) -> Color32 {
        let (dx, dy) = (x - self.x0, y - self.y0);
        if dx < 0 || dy < 0 || dx >= self.w as i32 || dy >= self.h as i32 {
            return Color32::TRANSPARENT;
        }
        self.px[dy as usize * self.w + dx as usize]
    }

    /// La couleur prémultipliée au point (x, y) de la feuille, lue entre ses quatre pixels voisins
    /// (comme `fxwarp::sample`).
    fn sample(&self, x: f32, y: f32) -> [f32; 4] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i32, iy as i32);
        let t = |c: Color32| [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32];
        let (lx, ly) = (ix - self.x0, iy - self.y0);
        let (a, b, c, d) = if lx >= 0 && ly >= 0 && (lx as usize) + 1 < self.w && (ly as usize) + 1 < self.h {
            // (Les quatre pixels sont dans le rectangle : pas de contrôle de bords.)
            let i = ly as usize * self.w + lx as usize;
            (t(self.px[i]), t(self.px[i + 1]), t(self.px[i + self.w]), t(self.px[i + self.w + 1]))
        } else {
            (t(self.get(ix, iy)), t(self.get(ix + 1, iy)), t(self.get(ix, iy + 1)), t(self.get(ix + 1, iy + 1)))
        };
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            top + (bottom - top) * ty
        })
    }
}

/// Visite les tuiles de la feuille `w` × `h` que `d` touche et qui portent de l'encre : le numéro de
/// la tuile, son origine, et le rectangle de `d` qu'elle contient.
fn for_blocks(d: &Dense, w: usize, h: usize, mut f: impl FnMut(usize, (usize, usize), i32, i32, i32, i32)) {
    let tiles_x = crate::raster::tiles_across(w);
    let (x0, y0, x1, y1) = d.bounds();
    let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(w as i32), y1.min(h as i32));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for ty in y0 as usize / TILE..=(y1 as usize - 1) / TILE {
        for tx in x0 as usize / TILE..=(x1 as usize - 1) / TILE {
            if let Some((bx0, by0, bx1, by1)) = block_of(d, tx * TILE, ty * TILE, w, h) {
                f(ty * tiles_x + tx, (tx * TILE, ty * TILE), bx0, by0, bx1, by1);
            }
        }
    }
}

/// Le rectangle de `d` que couvre la tuile d'origine (`ox`, `oy`) d'une feuille `w` × `h`, s'il y a de
/// l'encre dedans.
fn block_of(d: &Dense, ox: usize, oy: usize, w: usize, h: usize) -> Option<Region> {
    let (x0, y0, x1, y1) = d.bounds();
    let (bx0, bx1) = (x0.max(ox as i32), x1.min((ox + TILE).min(w) as i32));
    let (by0, by1) = (y0.max(oy as i32), y1.min((oy + TILE).min(h) as i32));
    if bx0 >= bx1 || by0 >= by1 {
        return None;
    }
    let any = (by0..by1).any(|y| {
        let row = (y - d.y0) as usize * d.w;
        d.px[row + (bx0 - d.x0) as usize..row + (bx1 - d.x0) as usize].iter().any(|c| c.a() != 0)
    });
    any.then_some((bx0, by0, bx1, by1))
}

/// `d` posé par-dessus `out`, à l'opacité `opacity`.
fn over_dense(out: &mut Raster, d: &Dense, opacity: u8) {
    let (w, h) = (out.width, out.height);
    for_blocks(d, w, h, |idx, origin, bx0, by0, bx1, by1| over_block(out.tile_mut(idx), d, origin, (bx0, by0, bx1, by1), opacity));
}

fn over_block(tile: &mut [Color32; crate::raster::TILE_PIXELS], d: &Dense, origin: (usize, usize), block: Region, opacity: u8) {
    for y in block.1..block.3 {
        for x in block.0..block.2 {
            let s = d.px[(y - d.y0) as usize * d.w + (x - d.x0) as usize];
            if s.a() != 0 {
                let at = (y as usize - origin.1) * TILE + (x as usize - origin.0);
                tile[at] = crate::render::over(tile[at], s, opacity, None);
            }
        }
    }
}

/// Les couches `layers` posées dans l'ordre sur `out`, bande de tuiles par bande de tuiles, les bandes
/// réparties sur les cœurs : chaque pixel reçoit les mêmes couches dans le même ordre que si elles
/// étaient posées l'une après l'autre (même image, quel que soit le nombre de fils).
fn compose_banded(out: &mut Raster, layers: &[Layer]) {
    let (w, h, tiles_x) = (out.width, out.height, out.tiles_x.max(1));
    crate::render::par_chunks(&mut out.tiles, tiles_x, |ty, row| {
        let oy = ty * TILE;
        for layer in layers {
            match layer {
                Layer::Plain(_) => {}
                Layer::Sheet(sheet, opacity) => {
                    for (tx, slot) in row.iter_mut().enumerate() {
                        let Some(src) = sheet.tiles.get(ty * tiles_x + tx).and_then(Option::as_ref) else { continue };
                        if src.iter().all(|c| c.a() == 0) {
                            continue;
                        }
                        let dst: &mut [Color32; crate::raster::TILE_PIXELS] = slot.get_or_insert_with(crate::raster::empty_tile);
                        for (d, &s) in dst.iter_mut().zip(src.iter()) {
                            if s.a() != 0 {
                                *d = crate::render::over(*d, s, *opacity, None);
                            }
                        }
                    }
                }
                Layer::Dense(d, opacity) => {
                    let (x0, _, x1, _) = d.bounds();
                    if x1 <= 0 || x0 >= w as i32 {
                        continue;
                    }
                    for tx in (x0.max(0) as usize / TILE)..=((x1.min(w as i32) as usize - 1) / TILE).min(tiles_x - 1) {
                        if let Some(block) = block_of(d, tx * TILE, oy, w, h) {
                            let dst: &mut [Color32; crate::raster::TILE_PIXELS] = row[tx].get_or_insert_with(crate::raster::empty_tile);
                            over_block(dst, d, (tx * TILE, oy), block, *opacity);
                        }
                    }
                }
            }
        }
    });
}

/// `src` (même taille) posé par-dessus `out`, à l'opacité `opacity`.
fn over_sheet(out: &mut Raster, src: &Raster, opacity: u8) {
    if opacity == 255 {
        out.composite_over(src);
        return;
    }
    for (idx, tile) in src.tiles.iter().enumerate() {
        let Some(tile) = tile else { continue };
        if idx >= out.tiles.len() {
            break;
        }
        let dst = out.tile_mut(idx);
        for (d, &s) in dst.iter_mut().zip(tile.iter()) {
            if s.a() != 0 {
                *d = crate::render::over(*d, s, opacity, None);
            }
        }
    }
}

/// Une couleur prémultipliée en flottants, remise en pixel (la couverture arrondie, la couleur sous
/// elle : comme `fxwarp`).
fn to_px(c: [f32; 4]) -> Color32 {
    let a = c[3].round().clamp(0.0, 255.0);
    let ch = |v: f32| v.round().clamp(0.0, a) as u8;
    Color32::from_rgba_premultiplied(ch(c[0]), ch(c[1]), ch(c[2]), a as u8)
}

/// Le pixel multiplié par `f` (0 à 1).
pub(crate) fn scaled(c: Color32, f: f32) -> Color32 {
    if f >= 0.999 {
        return c;
    }
    let m = |v: u8| (v as f32 * f).round() as u8;
    Color32::from_rgba_premultiplied(m(c.r()), m(c.g()), m(c.b()), m(c.a()))
}

// ---------------------------------------------------------------------- l'axe d'un trait

/// Un point de l'axe : sa place, l'abscisse curviligne et la tangente.
#[derive(Clone, Copy, Debug)]
struct Sample {
    x: f32,
    y: f32,
    s: f32,
    tx: f32,
    ty: f32,
    /// Rayon du trait en ce point.
    r: f32,
}

/// L'axe d'un trait rééchantillonné tous les `STEP` pixels (au plus 4 097 points).
pub(crate) struct PathIndex {
    samples: Vec<Sample>,
    pub(crate) total: f32,
    /// Écart des points (pixels).
    pub(crate) step: f32,
}

const STEP: f32 = 2.0;

impl PathIndex {
    fn build(points: &[VPoint]) -> PathIndex {
        let mut cum = Vec::with_capacity(points.len());
        let mut acc = 0.0f32;
        cum.push(0.0f32);
        for w in points.windows(2) {
            acc += (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
            cum.push(acc);
        }
        let total = acc;
        let Some(first) = points.first() else {
            return PathIndex { samples: Vec::new(), total: 0.0, step: STEP };
        };
        if total < 1e-3 {
            return PathIndex { samples: vec![Sample { x: first.x, y: first.y, s: 0.0, tx: 1.0, ty: 0.0, r: first.r }], total: 0.0, step: STEP };
        }
        let step = (total / 4096.0).max(STEP);
        let n = (total / step).ceil() as usize + 1;
        let mut samples: Vec<Sample> = Vec::with_capacity(n);
        let mut j = 0usize;
        for k in 0..n {
            let s = (k as f32 * step).min(total);
            while j + 2 < points.len() && cum[j + 1] < s {
                j += 1;
            }
            let (a, b) = (points[j], points[(j + 1).min(points.len() - 1)]);
            let seg = cum[(j + 1).min(points.len() - 1)] - cum[j];
            let t = if seg > 1e-6 { ((s - cum[j]) / seg).clamp(0.0, 1.0) } else { 0.0 };
            samples.push(Sample { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, s, tx: 1.0, ty: 0.0, r: a.r + (b.r - a.r) * t });
        }
        // Tangentes : différences centrales sur deux points de chaque côté.
        let last = samples.len() - 1;
        let mut prev = (1.0f32, 0.0f32);
        for i in 0..samples.len() {
            let (a, b) = (samples[i.saturating_sub(2)], samples[(i + 2).min(last)]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = dx.hypot(dy);
            if len > 1e-4 {
                prev = (dx / len, dy / len);
            }
            samples[i].tx = prev.0;
            samples[i].ty = prev.1;
        }
        PathIndex { samples, total, step }
    }

    /// La boîte de l'axe : (x0, y0, x1, y1).
    fn bounds(&self) -> (f32, f32, f32, f32) {
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for s in &self.samples {
            b = (b.0.min(s.x), b.1.min(s.y), b.2.max(s.x), b.3.max(s.y));
        }
        b
    }
}

/// Ce que la carte donne en un pixel : l'abscisse du point de l'axe le plus proche (le pixel projeté
/// sur l'axe), ce point, la normale et le rayon du trait.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub(crate) s: f32,
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    pub(crate) nx: f32,
    pub(crate) ny: f32,
    /// Rayon du trait au point de l'axe.
    pub(crate) r: f32,
}

/// Pour chaque pixel d'un rectangle, le point de l'axe le plus proche à moins de `radius` pixels
/// (dessiné point par point : chaque point de l'axe écrit les pixels de son disque qui sont plus
/// près de lui que du précédent).
struct NearMap {
    x0: i32,
    y0: i32,
    w: usize,
    h: usize,
    idx: Vec<u32>,
    /// Écart des points qui ont dessiné la carte.
    stride: f32,
}

impl NearMap {
    fn build(path: &PathIndex, region: (i32, i32, i32, i32), radius: f32) -> NearMap {
        let (x0, y0, x1, y1) = region;
        let (w, h) = ((x1 - x0).max(0) as usize, (y1 - y0).max(0) as usize);
        let mut idx = vec![u32::MAX; w * h];
        let mut best = vec![f32::MAX; w * h];
        let every = ((radius / 16.0).ceil() as usize).max(1);
        let r2 = radius * radius;
        let n = path.samples.len();
        let mut visit = |i: usize| {
            let sm = path.samples[i];
            let (ya, yb) = (((sm.y - radius - 0.5).ceil() as i32).max(y0), ((sm.y + radius - 0.5).floor() as i32).min(y1 - 1));
            for py in ya..=yb {
                let dy = py as f32 + 0.5 - sm.y;
                let dy2 = dy * dy;
                if dy2 > r2 {
                    continue;
                }
                let half = (r2 - dy2).sqrt();
                let (xa, xb) = (((sm.x - half - 0.5).ceil() as i32).max(x0), ((sm.x + half - 0.5).floor() as i32).min(x1 - 1));
                let row = (py - y0) as usize * w;
                for px in xa..=xb {
                    let dx = px as f32 + 0.5 - sm.x;
                    let d = dx * dx + dy2;
                    let slot = row + (px - x0) as usize;
                    if d < best[slot] {
                        best[slot] = d;
                        idx[slot] = i as u32;
                    }
                }
            }
        };
        for i in (0..n).step_by(every) {
            visit(i);
        }
        if n > 0 && (n - 1) % every != 0 {
            visit(n - 1);
        }
        NearMap { x0, y0, w, h, idx, stride: path.step * every as f32 }
    }

    fn hit(&self, path: &PathIndex, ix: i32, iy: i32) -> Option<Hit> {
        let (dx, dy) = (ix - self.x0, iy - self.y0);
        if dx < 0 || dy < 0 || dx >= self.w as i32 || dy >= self.h as i32 {
            return None;
        }
        let i = self.idx[dy as usize * self.w + dx as usize];
        if i == u32::MAX {
            return None;
        }
        let i = i as usize;
        let sm = path.samples[i];
        let (px, py) = (ix as f32 + 0.5, iy as f32 + 0.5);
        let u = (px - sm.x) * sm.tx + (py - sm.y) * sm.ty;
        let last = path.samples.len() - 1;
        let u = u.clamp(if i == 0 { 0.0 } else { -self.stride }, if i == last { 0.0 } else { self.stride });
        Some(Hit { s: (sm.s + u).clamp(0.0, path.total), cx: sm.x + u * sm.tx, cy: sm.y + u * sm.ty, nx: -sm.ty, ny: sm.tx, r: sm.r })
    }
}

/// L'axe d'un trait et sa carte, pour les effets qui lisent l'abscisse ou la normale.
pub(crate) struct Geo {
    pub(crate) path: PathIndex,
    near: NearMap,
}

impl Geo {
    pub(crate) fn hit(&self, ix: i32, iy: i32) -> Option<Hit> {
        self.near.hit(&self.path, ix, iy)
    }
}

type Region = (i32, i32, i32, i32);

fn grown(r: Region, by: f32, sheet: (usize, usize)) -> Region {
    let g = by.ceil() as i32;
    (r.0 - g, r.1 - g, r.2 + g, r.3 + g).clamp_to(sheet)
}

trait ClampTo {
    fn clamp_to(self, sheet: (usize, usize)) -> Self;
}

impl ClampTo for Region {
    fn clamp_to(self, sheet: (usize, usize)) -> Region {
        (self.0.max(0), self.1.max(0), self.2.min(sheet.0 as i32), self.3.min(sheet.1 as i32))
    }
}

/// Chaque pixel de `region` qui a un point de l'axe à portée va chercher sa couleur dans `src` au
/// point que `from` lui donne (aucun : transparent).
fn warp_dense(src: &Dense, region: Region, geo: &Geo, from: &dyn Fn(&Hit, f32, f32) -> Option<(f32, f32)>) -> Dense {
    let (w, h) = ((region.2 - region.0).max(0) as usize, (region.3 - region.1).max(0) as usize);
    let mut out = Dense::blank(region.0, region.1, w, h);
    for iy in region.1..region.3 {
        for ix in region.0..region.2 {
            let Some(hit) = geo.hit(ix, iy) else { continue };
            let (px, py) = (ix as f32 + 0.5, iy as f32 + 0.5);
            let Some((sx, sy)) = from(&hit, px, py) else { continue };
            let c = src.sample(sx, sy);
            if c[3] >= 0.5 {
                out.px[(iy - region.1) as usize * w + (ix - region.0) as usize] = to_px(c);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------- le rythme de l'écriture

/// Quand chaque trait s'écrit : le trait de rang `i` occupe, dans la durée 0 à 1 de l'écriture,
/// l'intervalle [début, début + durée] ; son point d'abscisse `s` est tracé à
/// `début + durée × s / longueur`.
struct Timing {
    start: Vec<f32>,
    span: Vec<f32>,
    len: Vec<f32>,
    /// Longueur (en temps de l'écriture) du bord doux de chaque trait.
    phi: Vec<f32>,
    /// L'avancement, élargi du bord doux pour que 0 % soit vide et 100 % entier.
    p_eff: f32,
    erase: bool,
}

impl Timing {
    fn new(w: &WriteS, lengths: &[f32]) -> Timing {
        let n = lengths.len();
        let weights: Vec<f32> = lengths.iter().map(|&l| if w.together { 1.0 } else if w.by_length { l.max(6.0) } else { 1.0 }).collect();
        let total: f32 = if w.together { 1.0 } else { weights.iter().sum::<f32>().max(1e-3) };
        let (mut start, mut span, mut len, mut phi) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        let mut acc = 0.0f32;
        let front = w.soft.max(1.0);
        for (i, &l) in lengths.iter().enumerate() {
            let (s0, sp) = if w.together { (0.0, 1.0) } else { (acc / total, weights[i] / total) };
            acc += weights[i];
            start.push(s0);
            span.push(sp);
            len.push(l);
            phi.push((front * sp / l.max(1.0)).max(1e-6));
        }
        let phi_max = phi.iter().copied().fold(0.0f32, f32::max);
        Timing { start, span, len, phi, p_eff: w.eased() * (1.0 + phi_max), erase: w.erase }
    }

    /// Le trait est entièrement visible.
    fn whole(&self, i: usize) -> bool {
        if self.erase { self.p_eff <= self.start[i] } else { self.p_eff >= self.start[i] + self.span[i] + self.phi[i] }
    }

    /// Le trait est entièrement caché.
    fn hidden(&self, i: usize) -> bool {
        if self.erase { self.p_eff >= self.start[i] + self.span[i] + self.phi[i] } else { self.p_eff <= self.start[i] }
    }

    /// La part visible du point d'abscisse `s` du trait `i` (0 à 1).
    fn factor(&self, i: usize, s: f32) -> f32 {
        let frac = if self.len[i] >= 1.0 { (s / self.len[i]).clamp(0.0, 1.0) } else { 0.0 };
        let t = self.start[i] + self.span[i] * frac;
        let f = smooth((self.p_eff - t) / self.phi[i]);
        if self.erase { 1.0 - f } else { f }
    }
}

// ---------------------------------------------------------------------- le rendu

/// Ce qu'un élément pose sur la feuille.
enum Layer<'a> {
    /// L'élément tel qu'il est (posé depuis son cache).
    Plain(&'a Element),
    /// Un élément rendu seul, à l'opacité donnée.
    Sheet(Raster, u8),
    Dense(Dense, u8),
}

/// Le travail d'un élément.
enum Job<'a> {
    Plain(&'a Element),
    /// Zone remplie ou texte à l'opacité donnée.
    Faded(&'a Element, u8),
    /// Un trait de rang `rank` (parmi les traits) à transformer.
    Stroke(&'a Element, usize),
}

/// Le dessin `vector` passé par les effets du trait de `steps`, sur une feuille `w` × `h`.
pub fn render(vector: &VectorDrawing, w: usize, h: usize, steps: &[FxStep]) -> Raster {
    let mut stack = Stack::of(steps);
    // La plume choisie (`fxmatiere::NibCtx`) : introuvable (pinceau supprimé), l'effet ne fait rien.
    let nib = stack.nib.and_then(|n| crate::fxmatiere::NibCtx::resolve(&n));
    if nib.is_none() {
        stack.nib = None;
    }
    let mut out = Raster::new(w, h);
    if stack.is_empty() {
        for e in &vector.elements {
            render_element(&mut out, e);
        }
        return out;
    }
    let lengths: Vec<f32> = vector
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Stroke { points, .. } => Some(points.windows(2).map(|s| (s[1].x - s[0].x).hypot(s[1].y - s[0].y)).sum::<f32>()),
            _ => None,
        })
        .collect();
    let timing = stack.write.as_ref().map(|wr| Timing::new(wr, &lengths));
    // L'opacité des zones remplies et des textes : suivent l'écriture ou la poussière si on le demande.
    let fill_opacity = {
        let mut a = 1.0f32;
        if let Some(wr) = stack.write.filter(|wr| wr.fills) {
            let p = wr.eased();
            a *= if wr.erase { 1.0 - smooth(p / 0.25) } else { smooth((p - 0.75) / 0.25) };
        }
        if let Some(d) = stack.dust.filter(|d| d.fills) {
            a *= 1.0 - d.progress;
        }
        (a * 255.0).round() as u8
    };
    let mut jobs: Vec<Job> = Vec::with_capacity(vector.elements.len());
    let mut rank = 0usize;
    for e in &vector.elements {
        match e {
            Element::Stroke { .. } => {
                let k = rank;
                rank += 1;
                if let Some(t) = &timing {
                    if t.hidden(k) {
                        continue;
                    }
                    if t.whole(k) && !stack.transforms() {
                        jobs.push(Job::Plain(e));
                        continue;
                    }
                }
                jobs.push(Job::Stroke(e, k));
            }
            _ if fill_opacity == 255 => jobs.push(Job::Plain(e)),
            _ if fill_opacity == 0 => {}
            _ => jobs.push(Job::Faded(e, fill_opacity)),
        }
    }
    let max = match crate::render::thread_cap() {
        0 => 16,
        cap => cap,
    };
    // Quand un effet transforme chaque trait, tout se pose par bandes de tuiles réparties sur les cœurs
    // (les éléments tels quels sont alors rendus seuls, eux aussi) ; sinon (l'écriture seule : presque
    // tout est posé tel quel depuis le cache) les éléments se posent l'un après l'autre.
    let banded = stack.transforms();
    let ctx = Ctx { stack, timing, w, h, banded, nib, strokes: lengths.len() };
    // Des morceaux de 96 éléments au plus, et d'au plus `CHUNK_PIXELS` pixels de couches en vol (une boîte de
    // trait par couche) : de longs traits en diagonale ne remplissent pas la mémoire.
    let passes = 1 + ctx.stack.sketch.map_or(0, |s| s.passes);
    let cost = |job: &Job| match job {
        Job::Stroke(e, _) => {
            let b = e.bounds();
            ((b.width().max(1.0) * b.height().max(1.0)) as usize).min(w * h) * passes
        }
        _ => 1,
    };
    let mut start = 0;
    while start < jobs.len() {
        let (mut end, mut pixels) = (start, 0usize);
        while end < jobs.len() && end - start < 96 && (pixels < CHUNK_PIXELS || end == start) {
            pixels += cost(&jobs[end]);
            end += 1;
        }
        let chunk = &jobs[start..end];
        start = end;
        let layers: Vec<Layer> = crate::parallele::par_map(chunk, max, |_, job| run(job, &ctx)).into_iter().flatten().collect();
        if banded {
            compose_banded(&mut out, &layers);
            continue;
        }
        for layer in layers {
            match layer {
                Layer::Plain(e) => render_element(&mut out, e),
                Layer::Sheet(s, opacity) => over_sheet(&mut out, &s, opacity),
                Layer::Dense(d, opacity) => over_dense(&mut out, &d, opacity),
            }
        }
    }
    out
}

/// Pixels de couches (`Dense`) qu'un morceau d'éléments a le droit de tenir à la fois : 48 millions, 192 Mo.
const CHUNK_PIXELS: usize = 48 << 20;

struct Ctx {
    stack: Stack,
    timing: Option<Timing>,
    w: usize,
    h: usize,
    /// Les couches se posent par bandes (pas d'élément posé tel quel).
    banded: bool,
    /// La plume qui redessine les traits (`fxmatiere::NibCtx`).
    nib: Option<crate::fxmatiere::NibCtx>,
    /// Combien de traits compte le dessin (le dégradé sur tout le dessin).
    strokes: usize,
}

fn run<'a>(job: &Job<'a>, ctx: &Ctx) -> Vec<Layer<'a>> {
    match job {
        Job::Plain(e) if ctx.banded => {
            let mut sheet = Raster::new(ctx.w, ctx.h);
            render_element(&mut sheet, e);
            vec![Layer::Sheet(sheet, 255)]
        }
        Job::Plain(e) => {
            // (Le rendu seul d'un trait sans cache, ici et en parallèle, le remplit : la pose, à la
            // suite, n'est plus qu'une copie.)
            if !e.cache_ready() {
                let mut scratch = Raster::new(ctx.w, ctx.h);
                render_element(&mut scratch, e);
            }
            vec![Layer::Plain(e)]
        }
        Job::Faded(e, opacity) => {
            let mut sheet = Raster::new(ctx.w, ctx.h);
            render_element(&mut sheet, e);
            vec![Layer::Sheet(sheet, *opacity)]
        }
        Job::Stroke(e, rank) => stroke(e, *rank, ctx),
    }
}

/// Un trait passé par tous les effets.
fn stroke<'a>(e: &'a Element, rank: usize, ctx: &Ctx) -> Vec<Layer<'a>> {
    let Element::Stroke { points, .. } = e else { return Vec::new() };
    let (w, h) = (ctx.w, ctx.h);
    let st = &ctx.stack;
    let mut sheet = Raster::new(w, h);
    // Une autre plume : le même tracé et les mêmes appuis, redessinés (et gardés) par la plume choisie.
    let nibbed = ctx.nib.as_ref().and_then(|n| n.restyle(e));
    render_element(&mut sheet, nibbed.as_deref().unwrap_or(e));
    let write = ctx.timing.as_ref().filter(|t| !t.whole(rank));
    if write.is_none() && !st.transforms() {
        return vec![Layer::Plain(e)];
    }
    let Some(src) = Dense::from_raster(&sheet) else { return Vec::new() };
    drop(sheet);
    // Le sens du trait, de son premier point au dernier.
    let dir = match (points.first(), points.last()) {
        (Some(a), Some(b)) if (b.x - a.x).hypot(b.y - a.y) > 1e-3 => {
            let len = (b.x - a.x).hypot(b.y - a.y);
            ((b.x - a.x) / len, (b.y - a.y) / len)
        }
        _ => (1.0, 0.0),
    };
    let sheet_size = (w, h);

    // L'axe, sa carte, et jusqu'où chaque effet pousse l'encre.
    let needs_path = write.is_some() || st.breath.is_some() || st.wave.is_some() || st.dash.is_some() || st.dry.is_some() || st.gradient.is_some() || st.dust.is_some_and(|d| d.sense > 0.0);
    let geo = needs_path.then(|| {
        let path = PathIndex::build(points);
        let pb = path.bounds();
        let (sx0, sy0, sx1, sy1) = src.bounds();
        let margin = (pb.0 - sx0 as f32).max(sx1 as f32 - pb.2).max(pb.1 - sy0 as f32).max(sy1 as f32 - pb.3).max(1.0) + 1.0;
        let breath_grow = st.breath.map_or(0.0, |b| (b.widest() - 1.0).max(0.0) * margin * 1.5);
        let wave_grow = st.wave.map_or(0.0, |wv| wv.amplitude + 1.0);
        let radius = margin * 1.5 + breath_grow + wave_grow + 1.0;
        let region = grown(src.bounds(), breath_grow + wave_grow + 1.0, sheet_size);
        let near = NearMap::build(&path, region, radius);
        (Geo { path, near }, breath_grow, wave_grow)
    });

    let mut cur = src;
    if let (Some(b), Some((geo, breath_grow, _))) = (st.breath, &geo) {
        let region = grown(cur.bounds(), *breath_grow, sheet_size);
        let phase = b.frame / b.period;
        cur = warp_dense(&cur, region, geo, &|hit, px, py| {
            let mut k = b.thickness;
            if b.amplitude > 0.0 {
                let along = if b.wave >= 1.0 { hit.s / b.wave } else { 0.0 };
                k *= 1.0 + b.amplitude * (TAU * (phase - along)).sin();
            }
            let k = k.max(0.05);
            if (k - 1.0).abs() < 1e-6 {
                return Some((px, py));
            }
            Some((hit.cx + (px - hit.cx) / k, hit.cy + (py - hit.cy) / k))
        });
    }
    if let (Some(wv), Some((geo, _, wave_grow))) = (st.wave, &geo) {
        let region = grown(cur.bounds(), *wave_grow, sheet_size);
        let total = geo.path.total;
        let moved = wv.speed * wv.frame;
        cur = warp_dense(&cur, region, geo, &|hit, px, py| {
            let env = if wv.fixed { smooth(hit.s.min(total - hit.s) / (wv.length * 0.5)) } else { 1.0 };
            let d = wv.amplitude * env * (TAU * (hit.s - moved) / wv.length).sin();
            Some((px - hit.nx * d, py - hit.ny * d))
        });
    }
    if let (Some(d), Some((geo, ..))) = (st.dash, &geo) {
        mask(&mut cur, geo, |_, s| d.factor(s));
    }
    // Effets de matière (`fxmatiere.rs`) : le trait sec, puis la couleur du dégradé.
    if let (Some(d), Some((geo, ..))) = (st.dry, &geo) {
        crate::fxmatiere::dry_stroke(&mut cur, geo, &d, rank);
    }
    if let (Some(g), Some((geo, ..))) = (st.gradient, &geo) {
        crate::fxmatiere::gradient_stroke(&mut cur, geo, &g, rank, ctx.strokes);
    }
    if let (Some(t), Some((geo, ..))) = (write, &geo) {
        mask(&mut cur, geo, |_, s| t.factor(rank, s));
    }
    let mut layers = Vec::new();
    let mut dust_sheet = None;
    if let Some(d) = st.dust {
        dust_sheet = Some(dust(&cur, geo.as_ref().map(|g| &g.0), &d, rank, w, h));
    }
    // Le crayonné redouble ce que les autres effets ont fait.
    if let Some(sk) = st.sketch {
        let base = match &dust_sheet {
            Some(sheet) => Dense::from_raster(sheet),
            None => Some(cur.clone()),
        };
        if let Some(base) = base {
            let opacity = (sk.opacity * 255.0).round() as u8;
            for pass in sketch_passes(&base, dir, rank, &sk, sheet_size) {
                layers.push(Layer::Dense(pass, opacity));
            }
        }
    }
    match dust_sheet {
        Some(sheet) => layers.push(Layer::Sheet(sheet, 255)),
        None => layers.push(Layer::Dense(cur, 255)),
    }
    layers
}

/// Multiplie chaque pixel de `d` par `f(pixel, abscisse)`.
fn mask(d: &mut Dense, geo: &Geo, f: impl Fn((i32, i32), f32) -> f32) {
    for y in 0..d.h {
        for x in 0..d.w {
            let c = d.px[y * d.w + x];
            if c.a() == 0 {
                continue;
            }
            let (ix, iy) = (d.x0 + x as i32, d.y0 + y as i32);
            let Some(hit) = geo.hit(ix, iy) else { continue };
            d.px[y * d.w + x] = scaled(c, f((ix, iy), hit.s));
        }
    }
}

/// Les passes du crayonné : le trait `d` relu à travers un champ de déplacement doux, propre au
/// trait, à la passe et à la variante (une translation de l'ensemble, plus un tremblement lent).
fn sketch_passes(d: &Dense, dir: (f32, f32), rank: usize, s: &SketchS, sheet: (usize, usize)) -> Vec<Dense> {
    use crate::tremblement::{Field, Lattice};
    let region = grown(d.bounds(), s.offset + 2.0, sheet);
    let (w, h) = ((region.2 - region.0).max(0) as usize, (region.3 - region.1).max(0) as usize);
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let occupied = occupancy(d, region, s.offset + 2.0);
    let cell = (crate::tremblement::wavelength(s.nerve) / 2.0).max(2.0);
    let mut out = Vec::with_capacity(s.passes);
    for pass in 0..s.passes {
        let seed = hash(s.seed, rank as u32, pass as u32, s.variant as u32 + 1);
        // Une translation de la passe, surtout de travers (un trait repris à côté de l'autre, d'un côté
        // puis de l'autre), un peu dans le sens du trait ; plus un tremblement lent. Au plus `offset`.
        let side = if pass % 2 == 0 { 1.0 } else { -1.0 };
        let across = side * (0.35 + 0.65 * unit(mix(seed ^ 0xA5A5))) * 0.6 * s.offset;
        let along = (unit(mix(seed ^ 0x5A5A)) * 2.0 - 1.0) * 0.25 * s.offset;
        let (bx, by) = (-dir.1 * across + dir.0 * along, dir.0 * across + dir.1 * along);
        let field = Field::raw(0.35 * s.offset, cell, mix(seed ^ 0x1234_5678));
        let lattice = Lattice::covering(&field, region.0 as f32, region.2 as f32 + 1.0, region.1 as f32, region.3 as f32 + 1.0);
        let mut layer = Dense::blank(region.0, region.1, w, h);
        for cy in 0..occupied.rows {
            for cx in 0..occupied.cols {
                if !occupied.cells[cy * occupied.cols + cx] {
                    continue;
                }
                for y in (cy * CELL)..((cy + 1) * CELL).min(h) {
                    for x in (cx * CELL)..((cx + 1) * CELL).min(w) {
                        let (px, py) = ((region.0 + x as i32) as f32 + 0.5, (region.1 + y as i32) as f32 + 0.5);
                        let [fx, fy, _] = lattice.at(&field, px, py);
                        let c = d.sample(px - bx - fx, py - by - fy);
                        if c[3] >= 0.5 {
                            layer.px[y * w + x] = to_px(c);
                        }
                    }
                }
            }
        }
        out.push(layer);
    }
    out
}

/// Côté (pixels) des cases de la carte d'occupation.
const CELL: usize = 4;

/// Les cases de `region` à moins de `reach` pixels de l'encre de `d`.
struct Occupancy {
    cols: usize,
    rows: usize,
    cells: Vec<bool>,
}

fn occupancy(d: &Dense, region: Region, reach: f32) -> Occupancy {
    let (w, h) = ((region.2 - region.0).max(0) as usize, (region.3 - region.1).max(0) as usize);
    let (cols, rows) = (w.div_ceil(CELL), h.div_ceil(CELL));
    let mut ink = vec![false; cols * rows];
    for y in 0..d.h {
        let cy = (d.y0 + y as i32 - region.1).max(0) as usize / CELL;
        for x in 0..d.w {
            if d.px[y * d.w + x].a() != 0 {
                let cx = (d.x0 + x as i32 - region.0).max(0) as usize / CELL;
                ink[(cy * cols + cx).min(cols * rows - 1)] = true;
            }
        }
    }
    // Élargi de `reach` pixels : un maximum en ligne, puis en colonne.
    let grow = (reach / CELL as f32).ceil() as usize;
    let mut wide = vec![false; cols * rows];
    for cy in 0..rows {
        for cx in 0..cols {
            if ink[cy * cols + cx] {
                for xx in cx.saturating_sub(grow)..=(cx + grow).min(cols - 1) {
                    wide[cy * cols + xx] = true;
                }
            }
        }
    }
    let mut cells = vec![false; cols * rows];
    for cy in 0..rows {
        for cx in 0..cols {
            if wide[cy * cols + cx] {
                for yy in cy.saturating_sub(grow)..=(cy + grow).min(rows - 1) {
                    cells[yy * cols + cx] = true;
                }
            }
        }
    }
    Occupancy { cols, rows, cells }
}

/// La désagrégation du trait `d` : chaque pixel appartient à un grain (carré de la grille de la
/// feuille) qui part à son heure, dans sa direction, plus la dérive ; il s'éparpille en pixels et
/// s'efface en fin de course.
fn dust(d: &Dense, geo: Option<&Geo>, s: &DustS, rank: usize, w: usize, h: usize) -> Raster {
    let mut out = Raster::new(w, h);
    let total = geo.map_or(1.0, |g| g.path.total.max(1.0));
    let drift = (s.angle.cos() * s.drift, s.angle.sin() * s.drift);
    for y in 0..d.h {
        for x in 0..d.w {
            let c = d.px[y * d.w + x];
            if c.a() == 0 {
                continue;
            }
            let (ix, iy) = (d.x0 + x as i32, d.y0 + y as i32);
            let (gx, gy) = (ix.div_euclid(s.grain), iy.div_euclid(s.grain));
            let hg = hash(s.seed, gx as u32, gy as u32, rank as u32);
            let rnd = unit(hg);
            let sn = match (s.sense > 0.0, geo) {
                (true, Some(g)) => g.hit(ix, iy).map_or(rnd, |hit| hit.s / total),
                _ => rnd,
            };
            let onset = rnd + (sn - rnd) * s.sense;
            // Un grain part entre 0 et 70 % de la durée et met 30 % de la durée à s'en aller : à 100 % tout est parti.
            let u = ((s.progress - 0.7 * onset) / 0.3).clamp(0.0, 1.0);
            let (mut tx, mut ty, mut keep) = (ix, iy, 1.0f32);
            if u > 0.0 {
                let heading = unit(mix(hg ^ 1)) * TAU;
                let speed = 0.4 + 0.6 * unit(mix(hg ^ 2));
                // Au départ le grain part d'un bloc ; en route ses pixels se séparent.
                let jitter = 1.0 + (unit(hash(s.seed ^ 0x9E37, ix as u32, iy as u32, rank as u32)) - 0.5) * 0.7 * smooth(u * 1.5);
                let dist = s.spread * speed * jitter * u.powf(1.3);
                tx = (ix as f32 + heading.cos() * dist + drift.0 * u * u).round() as i32;
                ty = (iy as f32 + heading.sin() * dist + drift.1 * u * u).round() as i32;
                keep = 1.0 - smooth((u - 0.55) / 0.45);
            }
            if keep <= 0.004 || tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                continue;
            }
            let idx = out.tile_index_of(tx as usize, ty as usize);
            let tile = out.tile_mut(idx);
            let at = (ty as usize % TILE) * TILE + tx as usize % TILE;
            tile[at] = crate::render::over(tile[at], scaled(c, keep), 255, None);
        }
    }
    out
}
