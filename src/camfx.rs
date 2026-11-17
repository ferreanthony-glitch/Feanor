//! Effets de caméra en préréglages : des clés de caméra posées toutes seules (tremblements,
//! explosion, zooms), à partir de la caméra telle qu'elle est (Anthony, 2026-10-18 : « ajouter des
//! effets de caméra en préréglage qui posent des clés automatiquement : tremblement de caméra
//! type found footage, explosions, zoom »).
//!
//! Un préréglage n'invente aucune notion nouvelle : il écrit des clés de caméra ordinaires, que
//! l'on déplace, copie, supprime ou règle ensuite comme les autres. Il part de la caméra de
//! chaque image (`base`, avec ses clés et ses adoucis) et lui ajoute un décalage : il ne remet
//! rien à zéro. Les valeurs d'amplitude, de fréquence et de durée sont des choix de SHOT, à régler
//! à l'œil.
//!
//! Tout est déterministe : même préréglage, mêmes réglages, même graine, mêmes clés. Le bruit est
//! une somme de sinus aux phases tirées de la graine (lisse, borné par l'intensité) ; les
//! secousses sont des oscillations amorties. Un tremblement continu part de zéro et y revient
//! (la dernière clé est neutre) ; l'explosion et la secousse commencent d'un coup et s'éteignent.
//! Les décalages se comptent en part de la largeur du cadre filmé (le tremblement a donc le même
//! effet à tous les zooms), les rotations en degrés, les zooms en facteur de la focale.
//!
//! Les fonctions sont pures (`Generator`, `plan`, `write`) ; `camfxui.rs` les branche sur le
//! document, l'historique et le panneau de la caméra.

use crate::camera::{self, Camera, CameraKey};

/// Les préréglages proposés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Caméra à l'épaule (found footage) : dérive lente au hasard, avec de petites secousses.
    Shoulder,
    /// Tremblement léger et continu.
    LightShake,
    /// Explosion / impact : forte secousse qui s'éteint, avec un coup de zoom en option.
    Explosion,
    /// Secousse rapide : un à-coup court.
    Jolt,
    /// Zoom avant, adouci au départ et à l'arrivée.
    ZoomIn,
    /// Zoom arrière, adouci.
    ZoomOut,
    /// Zoom choc (crash zoom) : un zoom avant brutal qui ralentit en arrivant.
    CrashZoom,
    /// Vertigo (dolly zoom) : la focale change, la caméra recule en même temps, le sujet (au plan
    /// de référence) garde sa taille et le fond se déforme.
    Vertigo,
}

impl Preset {
    pub const ALL: [Preset; 8] =
        [Preset::Shoulder, Preset::LightShake, Preset::Explosion, Preset::Jolt, Preset::ZoomIn, Preset::ZoomOut, Preset::CrashZoom, Preset::Vertigo];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Shoulder => "Caméra à l'épaule (found footage)",
            Preset::LightShake => "Tremblement léger",
            Preset::Explosion => "Explosion / impact",
            Preset::Jolt => "Secousse rapide",
            Preset::ZoomIn => "Zoom avant",
            Preset::ZoomOut => "Zoom arrière",
            Preset::CrashZoom => "Zoom choc (crash zoom)",
            Preset::Vertigo => "Vertigo (dolly zoom)",
        }
    }

    /// Le nom rangé dans le projet (format 52) : stable, sans accent, indépendant du libellé.
    pub fn nom(self) -> &'static str {
        match self {
            Preset::Shoulder => "epaule",
            Preset::LightShake => "tremblement",
            Preset::Explosion => "explosion",
            Preset::Jolt => "secousse",
            Preset::ZoomIn => "zoom_avant",
            Preset::ZoomOut => "zoom_arriere",
            Preset::CrashZoom => "zoom_choc",
            Preset::Vertigo => "vertigo",
        }
    }

    /// Le préréglage de ce nom (`nom`) ; rien pour un nom inconnu.
    pub fn du_nom(nom: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.nom() == nom)
    }

    /// Ce que fait le préréglage, en une phrase (bulle).
    pub fn note(self) -> &'static str {
        match self {
            Preset::Shoulder => "Dérive lente et irrégulière de la position et de la rotation, avec de petites secousses de temps en temps. Une clé toutes les 2 images.",
            Preset::LightShake => "Tremblement fin et continu, à peine visible. Une clé par image.",
            Preset::Explosion => "Forte secousse qui s'éteint en décroissance exponentielle, avec un coup de zoom en option. Une clé par image.",
            Preset::Jolt => "Un à-coup court, qui s'éteint vite. Une clé par image.",
            Preset::ZoomIn => "Resserre le cadre (la focale monte), doucement au départ et à l'arrivée. Deux clés.",
            Preset::ZoomOut => "Élargit le cadre (la focale baisse), doucement au départ et à l'arrivée. Deux clés.",
            Preset::CrashZoom => "Zoom avant brutal qui ralentit en arrivant. Deux clés.",
            Preset::Vertigo => "La focale monte et la caméra recule d'autant : le sujet au plan de référence garde sa taille, les calques en profondeur bougent. Deux douzaines de clés.",
        }
    }

    /// Un tremblement ou une secousse (intensité, fréquence, graine).
    pub fn is_shake(self) -> bool {
        matches!(self, Preset::Shoulder | Preset::LightShake | Preset::Explosion | Preset::Jolt)
    }

    /// Commence d'un coup (la caméra est calme juste avant) plutôt que de monter doucement.
    pub fn is_abrupt(self) -> bool {
        matches!(self, Preset::Explosion | Preset::Jolt | Preset::CrashZoom)
    }

    /// Durée proposée, en secondes.
    pub fn default_seconds(self) -> f32 {
        match self {
            Preset::Shoulder => 4.0,
            Preset::LightShake => 3.0,
            Preset::Explosion => 1.2,
            Preset::Jolt => 0.35,
            Preset::ZoomIn | Preset::ZoomOut => 2.0,
            Preset::CrashZoom => 0.3,
            Preset::Vertigo => 3.0,
        }
    }

    /// Fréquence proposée, en Hz (de la dérive pour la caméra à l'épaule, du tremblement sinon).
    pub fn default_freq(self) -> f32 {
        match self {
            Preset::Shoulder => 0.9,
            Preset::LightShake => 7.0,
            Preset::Explosion => 12.0,
            Preset::Jolt => 10.0,
            _ => 0.0,
        }
    }

    /// Facteur de zoom proposé (focale d'arrivée / focale de départ).
    pub fn default_factor(self) -> f32 {
        match self {
            Preset::ZoomIn | Preset::ZoomOut => 1.5,
            Preset::CrashZoom => 3.0,
            Preset::Vertigo => 2.0,
            _ => 1.0,
        }
    }

    /// Écart entre deux clés, en images, avant le plafond du nombre de clés.
    fn step(self) -> usize {
        match self {
            Preset::Shoulder => 2,
            Preset::LightShake | Preset::Explosion | Preset::Jolt => 1,
            _ => usize::MAX,
        }
    }
}

/// Les réglages d'un effet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub preset: Preset,
    /// 1 = 100 % : la force de départ du préréglage (tremblements).
    pub intensity: f32,
    /// Images entre la première et la dernière clé.
    pub duration: usize,
    /// Fréquence, en Hz (tremblements).
    pub freq: f32,
    /// Graine du hasard (tremblements).
    pub seed: u32,
    /// Focale d'arrivée / focale de départ (zooms, vertigo) ; au moins 1.
    pub zoom_factor: f32,
    /// Coup de zoom avec l'explosion.
    pub punch: bool,
}

/// Plafond du nombre de clés posées d'un coup (un effet très long espace ses clés).
pub const MAX_KEYS: usize = 1000;

impl Params {
    /// Les réglages proposés pour `preset` à `fps` images par seconde.
    pub fn new(preset: Preset, fps: u32) -> Params {
        Params {
            preset,
            intensity: 1.0,
            duration: ((preset.default_seconds() * fps as f32).round() as usize).max(1),
            freq: preset.default_freq(),
            seed: 1,
            zoom_factor: preset.default_factor(),
            punch: preset == Preset::Explosion,
        }
    }

    /// Les mêmes réglages pour un autre préréglage : durée, fréquence et facteur repartent de ses
    /// valeurs, l'intensité et la graine restent.
    pub fn with_preset(&self, preset: Preset, fps: u32) -> Params {
        Params { intensity: self.intensity, seed: self.seed, ..Params::new(preset, fps) }
    }

    /// Réglages ramenés dans des bornes sûres.
    pub fn sanitized(&self, fps: u32) -> Params {
        let ok = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Params {
            intensity: ok(self.intensity, 0.0, 10.0, 1.0),
            duration: self.duration.clamp(1, 100_000),
            freq: ok(self.freq, 0.0, (fps as f32 / 2.0).max(1.0), self.preset.default_freq()),
            zoom_factor: ok(self.zoom_factor, 1.0, 20.0, self.preset.default_factor().max(1.0)),
            ..*self
        }
    }
}

/// Une clé d'effet : le décalage ajouté à la caméra de base à l'image `frame` (comptée depuis le
/// début de l'effet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub frame: usize,
    /// Décalages de la position, en part de la largeur du cadre filmé (x vers la droite, y vers le bas).
    pub dx: f32,
    pub dy: f32,
    /// Rotation ajoutée, en degrés.
    pub angle: f32,
    /// Facteur de la focale (1 : inchangée).
    pub zoom: f32,
    /// Adoucis à poser sur la clé (images), pour les zooms.
    pub ease_in: f32,
    pub ease_out: f32,
}

/// Les bornes de ce qu'un effet peut ajouter à la caméra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Décalage de position, en part de la largeur du cadre filmé (par axe).
    pub pos: f32,
    /// Rotation, en degrés.
    pub angle: f32,
    /// Coup de zoom : le facteur reste dans 1 ± cette part.
    pub punch: f32,
}

// ---------------------------------------------------------------------- hasard et bruit

/// Générateur de nombres (splitmix64) : déterministe, sans dépendance.
struct Rng(u64);

impl Rng {
    fn new(seed: u32, salt: u64) -> Rng {
        Rng(((seed as u64) << 20) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Dans [0, 1[.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

const TAU: f32 = std::f32::consts::TAU;

/// Bruit lisse dans [-1, 1] : trois sinus de fréquences voisines, aux phases tirées au sort.
#[derive(Clone, Copy, Debug)]
struct Wave {
    phase: [f32; 3],
}

impl Wave {
    const RATIO: [f32; 3] = [1.0, 1.93, 3.31];
    const WEIGHT: [f32; 3] = [1.0, 0.55, 0.3];

    fn new(rng: &mut Rng) -> Wave {
        Wave { phase: [rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU)] }
    }

    fn at(&self, freq: f32, t: f32) -> f32 {
        let sum: f32 = Self::WEIGHT.iter().sum();
        (0..3).map(|k| Self::WEIGHT[k] * (TAU * freq * Self::RATIO[k] * t + self.phase[k]).sin()).sum::<f32>() / sum
    }
}

/// 0 en 0, 1 en 1, sans à-coup aux deux bouts.
fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Une secousse de la caméra à l'épaule.
#[derive(Clone, Copy, Debug)]
struct Jolt {
    /// Instant, en secondes.
    t0: f32,
    dir: (f32, f32),
    mag: f32,
    sign: f32,
}

// Amplitudes à 100 % (part de la largeur du cadre filmé ; degrés).
const SHOULDER_X: f32 = 0.012;
const SHOULDER_Y: f32 = 0.009;
const SHOULDER_ANGLE: f32 = 0.7;
const SHOULDER_JOLT: f32 = 0.025;
const SHOULDER_JOLT_ANGLE: f32 = 1.2;
const LIGHT_X: f32 = 0.004;
const LIGHT_Y: f32 = 0.0035;
const LIGHT_ANGLE: f32 = 0.15;
const EXPLOSION_POS: f32 = 0.045;
const EXPLOSION_ANGLE: f32 = 2.5;
const EXPLOSION_PUNCH: f32 = 0.06;
const JOLT_POS: f32 = 0.03;
const JOLT_ANGLE: f32 = 1.5;
/// Durée de vie d'une secousse de la caméra à l'épaule, et sa fréquence (Hz).
const JOLT_DECAY: f32 = 0.12;
const JOLT_FREQ: f32 = 5.0;

/// Calcule les clés d'un effet, image par image.
#[derive(Clone, Debug)]
pub struct Generator {
    p: Params,
    fps: f32,
    step: usize,
    waves: [Wave; 3],
    dir: (f32, f32),
    phase: f32,
    jolts: Vec<Jolt>,
}

impl Generator {
    pub fn new(p: &Params, fps: u32) -> Generator {
        let p = p.sanitized(fps);
        let fps_f = fps.max(1) as f32;
        // Le vertigo pose deux douzaines de clés : la distance se mêle en ligne droite entre deux
        // clés alors que le zoom se mêle en proportion, l'écart reste sous 0,1 % de la taille du sujet.
        let base_step = if p.preset == Preset::Vertigo { p.duration.div_ceil(24) } else { p.preset.step() };
        let step = base_step.max(p.duration.div_ceil(MAX_KEYS)).max(1);
        let mut rng = Rng::new(p.seed, p.preset as u64 + 1);
        let waves = [Wave::new(&mut rng), Wave::new(&mut rng), Wave::new(&mut rng)];
        let theta = rng.range(0.0, TAU);
        let phase = rng.range(0.0, TAU);
        let mut jolts = Vec::new();
        if p.preset == Preset::Shoulder {
            let total = p.duration as f32 / fps_f;
            let mut t = rng.range(0.5, 1.5);
            // (Aucune secousse dans la dernière seconde : l'effet finit calme.)
            while t < total - 1.0 {
                let a = rng.range(0.0, TAU);
                let snapped = (t * fps_f / step as f32).round() * step as f32 / fps_f;
                jolts.push(Jolt { t0: snapped, dir: (a.cos(), a.sin()), mag: rng.range(0.6, 1.0), sign: if rng.unit() < 0.5 { -1.0 } else { 1.0 } });
                t += rng.range(0.9, 2.4);
            }
        }
        Generator { p, fps: fps_f, step, waves, dir: (theta.cos(), theta.sin()), phase, jolts }
    }

    pub fn params(&self) -> &Params {
        &self.p
    }

    /// Les images (depuis le début de l'effet) où l'effet pose une clé : 0, de `step` en `step`, et la dernière.
    pub fn frames(&self) -> Vec<usize> {
        let mut out: Vec<usize> = (0..self.p.duration).step_by(self.step).collect();
        out.push(self.p.duration);
        out
    }

    /// Les bornes de ce que l'effet ajoute à la caméra.
    pub fn bounds(&self) -> Bounds {
        let i = self.p.intensity;
        match self.p.preset {
            Preset::Shoulder => Bounds {
                pos: i * (SHOULDER_X.max(SHOULDER_Y) + SHOULDER_JOLT * 1.01),
                angle: i * (SHOULDER_ANGLE + SHOULDER_JOLT_ANGLE * 1.01),
                punch: 0.0,
            },
            Preset::LightShake => Bounds { pos: i * LIGHT_X.max(LIGHT_Y), angle: i * LIGHT_ANGLE, punch: 0.0 },
            Preset::Explosion => Bounds { pos: i * EXPLOSION_POS, angle: i * EXPLOSION_ANGLE, punch: if self.p.punch { i * EXPLOSION_PUNCH } else { 0.0 } },
            Preset::Jolt => Bounds { pos: i * JOLT_POS, angle: i * JOLT_ANGLE, punch: 0.0 },
            _ => Bounds { pos: 0.0, angle: 0.0, punch: 0.0 },
        }
    }

    /// Le décalage à l'image `frame` de l'effet (0 = sa première clé). Au-delà de la durée, le
    /// résultat est celui de la dernière image.
    pub fn at(&self, frame: usize) -> Sample {
        let frame = frame.min(self.p.duration);
        let last = frame == self.p.duration;
        let t = frame as f32 / self.fps;
        let total = self.p.duration as f32 / self.fps;
        let neutral = Sample { frame, dx: 0.0, dy: 0.0, angle: 0.0, zoom: 1.0, ease_in: 0.0, ease_out: 0.0 };
        let i = self.p.intensity;
        let f = self.p.freq;
        let [wx, wy, wa] = self.waves;
        match self.p.preset {
            Preset::Shoulder | Preset::LightShake => {
                if last {
                    return neutral;
                }
                let ramp = (0.3_f32).min(total / 3.0).max(1e-3);
                let env = smoothstep(t / ramp) * smoothstep((total - t) / ramp);
                let (ax, ay, aa) = if self.p.preset == Preset::Shoulder { (SHOULDER_X, SHOULDER_Y, SHOULDER_ANGLE) } else { (LIGHT_X, LIGHT_Y, LIGHT_ANGLE) };
                let mut s = Sample {
                    dx: i * ax * env * wx.at(f, t),
                    dy: i * ay * env * wy.at(f, t),
                    angle: i * aa * env * wa.at(f, t),
                    ..neutral
                };
                for j in &self.jolts {
                    let dt = t - j.t0;
                    if (0.0..1.5).contains(&dt) {
                        let k = (-dt / JOLT_DECAY).exp() * (TAU * JOLT_FREQ * dt).cos() * j.mag * i;
                        s.dx += SHOULDER_JOLT * k * j.dir.0;
                        s.dy += SHOULDER_JOLT * k * j.dir.1;
                        s.angle += SHOULDER_JOLT_ANGLE * k * j.sign;
                    }
                }
                s
            }
            Preset::Explosion | Preset::Jolt => {
                if last {
                    return neutral;
                }
                let (pos, rot, tau) = if self.p.preset == Preset::Explosion {
                    (EXPLOSION_POS, EXPLOSION_ANGLE, total / 4.0)
                } else {
                    (JOLT_POS, JOLT_ANGLE, total / 3.0)
                };
                let env = (-t / tau.max(1e-3)).exp();
                let main = (TAU * f * t).cos();
                let zoom = if self.p.preset == Preset::Explosion && self.p.punch { 1.0 + EXPLOSION_PUNCH * i * env } else { 1.0 };
                Sample {
                    dx: i * pos * env * (0.75 * self.dir.0 * main + 0.25 * wx.at(f, t)),
                    dy: i * pos * env * (0.75 * self.dir.1 * main + 0.25 * wy.at(f, t)),
                    angle: i * rot * env * (0.6 * (TAU * f * 0.7 * t + self.phase).cos() + 0.4 * wa.at(f, t)),
                    zoom,
                    ..neutral
                }
            }
            Preset::ZoomIn | Preset::ZoomOut | Preset::CrashZoom | Preset::Vertigo => {
                let factor = self.p.zoom_factor;
                let end = if self.p.preset == Preset::ZoomOut { 1.0 / factor } else { factor };
                let dur = self.p.duration as f32;
                let (out, inn) = self.zoom_eases();
                let zoom = if last {
                    end
                } else {
                    end.powf(camera::ease(frame as f32 / dur, dur, out, inn))
                };
                // (Le vertigo a l'adoucissement dans ses clés ; les zooms le posent sur leurs deux clés.)
                let (ease_in, ease_out) = if self.p.preset == Preset::Vertigo || (0 < frame && !last) {
                    (0.0, 0.0)
                } else if frame == 0 {
                    (0.0, out)
                } else {
                    (inn, 0.0)
                };
                Sample { zoom, ease_in, ease_out, ..neutral }
            }
        }
    }

    /// Adoucis (départ, arrivée) du mouvement d'un zoom, en images.
    fn zoom_eases(&self) -> (f32, f32) {
        let dur = self.p.duration as f32;
        match self.p.preset {
            Preset::CrashZoom => (0.0, dur),
            _ => (dur * 0.5, dur * 0.5),
        }
    }

    /// Le vertigo garde la taille du sujet : la caméra recule avec la focale.
    pub fn keeps_subject_size(&self) -> bool {
        self.p.preset == Preset::Vertigo
    }

    /// L'effet commence d'un coup (une clé calme juste avant).
    pub fn abrupt(&self) -> bool {
        self.p.preset.is_abrupt()
    }
}

// ---------------------------------------------------------------------- plan et écriture

/// Une clé à poser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannedKey {
    /// Image du plan.
    pub frame: usize,
    pub cam: Camera,
    /// Adoucis à poser (sinon ceux de la clé, s'il y en a déjà une).
    pub ease_in: Option<f32>,
    pub ease_out: Option<f32>,
}

/// Ce qu'un effet va écrire dans les clés d'un plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub keys: Vec<PlannedKey>,
    /// Clé du début du plan, si le plan n'avait aucune clé et que l'effet commence plus tard : la
    /// caméra de continuité y reste en place jusque-là.
    pub home: Option<(usize, Camera)>,
    /// Clé calme juste avant un effet qui commence d'un coup.
    pub before: Option<(usize, Camera)>,
    /// L'effet a été coupé par la fin du plan.
    pub truncated: bool,
}

impl Plan {
    /// Dernière image touchée.
    pub fn last_frame(&self) -> Option<usize> {
        self.keys.last().map(|k| k.frame)
    }
}

/// Prépare les clés de l'effet `g` posé à partir de l'image `start` d'un plan de `plan_len`
/// images. `base` : la caméra du plan avant l'effet ; `extra` : images de clés déjà là, à garder
/// (elles reçoivent aussi le décalage) ; `has_keys` : le plan avait déjà des clés.
pub fn plan(g: &Generator, frame: [usize; 2], start: usize, plan_len: usize, extra: &[usize], has_keys: bool, base: &dyn Fn(usize) -> Camera) -> Plan {
    let mut out = Plan { keys: Vec::new(), home: None, before: None, truncated: false };
    let Some(last_allowed) = plan_len.checked_sub(1).filter(|&l| start <= l) else { return out };
    let duration = g.params().duration;
    let mut rels = g.frames();
    rels.extend(extra.iter().filter(|&&f| f > start && f < start + duration).map(|&f| f - start));
    rels.sort_unstable();
    rels.dedup();
    if start + duration > last_allowed {
        out.truncated = true;
        rels.retain(|&r| start + r <= last_allowed);
        rels.push(last_allowed - start);
        rels.dedup();
    }
    let d0 = camera::ref_distance(frame);
    for rel in rels {
        let s = g.at(rel);
        let at = start + rel;
        let b = base(at);
        let width = frame[0] as f32 / b.magnification(frame).max(1e-3);
        let mut mul = s.zoom;
        if g.keeps_subject_size() {
            // La distance suit la focale : mul ne sort ni des bornes du zoom ni de celles de la distance.
            let dist = b.distance(frame);
            let lo = (0.05 / b.zoom).max(0.05 * d0 / dist);
            let hi = (50.0 / b.zoom).min(20.0 * d0 / dist);
            mul = mul.clamp(lo, hi.max(lo));
        }
        let zoom = (b.zoom * mul).clamp(0.05, 50.0);
        let dolly = if g.keeps_subject_size() { d0 - b.distance(frame) * (zoom / b.zoom) } else { b.dolly };
        let cam = Camera { x: b.x + s.dx * width, y: b.y + s.dy * width, angle: b.angle + s.angle, zoom, dolly, ..b }.limited(frame);
        out.keys.push(PlannedKey { frame: at, cam, ease_in: (s.ease_in > 0.0).then_some(s.ease_in), ease_out: (s.ease_out > 0.0).then_some(s.ease_out) });
    }
    if g.abrupt() && start > 0 {
        out.before = Some((start - 1, base(start - 1)));
    }
    if !has_keys && start > 0 {
        out.home = Some((0, base(0)));
    }
    out
}

// ---------------------------------------------------------------------- l'effet vivant

/// Un effet de caméra **vivant** (Anthony, 2026-11-08 : « les effets de caméra doivent être appliqués et visibles
/// immédiatement sans création de clés ») : un réglage du plan, rangé dans le projet, calculé quand la caméra est
/// évaluée (`Document::panel_camera`). Aucune clé n'est écrite ; « Convertir en clés » les pose (`cuire`).
///
/// La règle qui garantit « la cuisson donne la même image » : l'effet vivant **est** la cuisson, faite à la volée. La
/// caméra montrée est celle qu'auraient les clés cuites (`cuire`), interpolées et postérisées comme des clés
/// ordinaires ; convertir ne fait qu'écrire ces clés-là.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vivant {
    /// Préréglage, intensité, durée (en images), fréquence, graine, facteur de zoom, coup de zoom.
    pub params: Params,
    /// Image du plan où l'effet commence.
    pub start: usize,
}

impl Vivant {
    /// Les réglages ramenés dans leurs bornes.
    pub fn sanitized(&self, fps: u32) -> Vivant {
        Vivant { params: self.params.sanitized(fps), start: self.start }
    }

    /// Dernière image touchée par l'effet dans un plan de `plan_len` images (coupée par la fin du plan).
    pub fn last_frame(&self, plan_len: usize) -> usize {
        (self.start + self.params.duration).min(plan_len.saturating_sub(1))
    }
}

/// Les clés de caméra d'un plan une fois l'effet `v` cuit dedans : `keys` (ses clés), `base` (la caméra du plan sans
/// l'effet, continuité comprise). Le même calcul sert à l'affichage (effet vivant) et à « Convertir en clés ». Rend
/// aussi ce que l'effet a posé (`Plan`), rien si l'effet commence après la fin du plan.
pub fn cuire(v: &Vivant, fps: u32, frame: [usize; 2], plan_len: usize, keys: &[CameraKey], base: &dyn Fn(usize) -> Camera) -> Option<(Vec<CameraKey>, Plan)> {
    let g = Generator::new(&v.params, fps);
    let extra: Vec<usize> = keys.iter().map(|k| k.frame).collect();
    let plan = plan(&g, frame, v.start, plan_len, &extra, !keys.is_empty(), base);
    plan.last_frame()?;
    let mut out = keys.to_vec();
    write(&mut out, &plan);
    Some((out, plan))
}

/// Comme `cuire`, sans le `Plan`, et gardé en mémoire : la caméra est évaluée bien des fois par image d'écran (cadre,
/// vignettes, montage, lecture), le calcul ne se refait que si quelque chose a changé. La clé de la mémoire porte tout
/// ce dont dépend le résultat : l'effet, la cadence, le cadre, la durée du plan, ses clés, et (plan sans clé) la caméra
/// de continuité, qui est alors la même à toutes les images.
pub fn cuire_vite(v: &Vivant, fps: u32, frame: [usize; 2], plan_len: usize, keys: &[CameraKey], base: &dyn Fn(usize) -> Camera) -> Option<Vec<CameraKey>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let cam = |h: &mut std::collections::hash_map::DefaultHasher, c: &Camera| {
        for x in [c.x, c.y, c.zoom, c.angle, c.dolly, c.tilt, c.pan] {
            x.to_bits().hash(h);
        }
    };
    let p = &v.params;
    (p.preset as u8, p.intensity.to_bits(), p.duration, p.freq.to_bits(), p.seed, p.zoom_factor.to_bits(), p.punch, v.start).hash(&mut h);
    (fps, frame, plan_len, keys.len()).hash(&mut h);
    for k in keys {
        (k.frame, k.smooth, k.ease_in.to_bits(), k.ease_out.to_bits()).hash(&mut h);
        cam(&mut h, &k.cam);
    }
    if keys.is_empty() {
        cam(&mut h, &base(0));
    }
    let id = h.finish();
    // (Une table par fil, jusqu'à `MEMOIRE_MAX` cuissons : une planche de centaines de plans à effet ne recalcule pas en
    // boucle. Pleine, elle repart de zéro : les plans affichés se recalculent une fois.)
    type Table = std::collections::HashMap<u64, Option<std::rc::Rc<Vec<CameraKey>>>>;
    thread_local! {
        static MEMOIRE: std::cell::RefCell<Table> = std::cell::RefCell::new(Table::new());
    }
    if let Some(hit) = MEMOIRE.with(|m| m.borrow().get(&id).cloned()) {
        return hit.map(|r| (*r).clone());
    }
    let out = cuire(v, fps, frame, plan_len, keys, base).map(|(k, _)| std::rc::Rc::new(k));
    MEMOIRE.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= MEMOIRE_MAX {
            m.clear();
        }
        m.insert(id, out.clone());
    });
    out.map(|r| (*r).clone())
}

/// Nombre de cuissons gardées par fil (`cuire_vite`).
pub const MEMOIRE_MAX: usize = 4096;

/// Écrit `plan` dans les clés d'un plan : les clés posées remplacent celles de la même image (en
/// gardant leurs adoucis, sauf ceux que l'effet pose), les autres restent.
pub fn write(keys: &mut Vec<CameraKey>, plan: &Plan) {
    if let (true, Some((frame, cam))) = (keys.is_empty(), plan.home) {
        keys.push(CameraKey::new(frame, cam));
    }
    if let Some((frame, cam)) = plan.before {
        if !keys.iter().any(|k| k.frame == frame) {
            camera::put_key(keys, CameraKey::new(frame, cam));
        }
    }
    for k in &plan.keys {
        match keys.iter_mut().find(|x| x.frame == k.frame) {
            Some(x) => {
                x.cam = k.cam;
                if let Some(e) = k.ease_in {
                    x.ease_in = e;
                    x.smooth = false;
                }
                if let Some(e) = k.ease_out {
                    x.ease_out = e;
                    x.smooth = false;
                }
            }
            None => {
                let mut key = CameraKey::new(k.frame, k.cam);
                key.ease_in = k.ease_in.unwrap_or(0.0);
                key.ease_out = k.ease_out.unwrap_or(0.0);
                camera::put_key(keys, key);
            }
        }
    }
}
