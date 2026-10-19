//! Préréglages d'effets (Anthony, 2026-10-19 : « calques d'effets après des préréglages d'animation
//! d'effet comme des impact frames, glows, tremblements de caméra effectifs, timecode »).
//!
//! Un préréglage ne crée aucune notion nouvelle : il écrit un calque d'effet (`calqueffet.rs`) ou des
//! effets sur le calque courant, avec leurs clés posées à partir de la tête de lecture (impact,
//! lueur, flash, tremblement de l'image, vignettage, grain, aberration chromatique, flou de vitesse),
//! ou une incrustation (timecode). Chaque valeur reste ensuite réglable comme les autres : les clés
//! se déplacent, se copient, se suppriment dans le panneau Effets, la timeline et le montage.
//!
//! Les fonctions sont pures et déterministes (même préréglage, mêmes réglages, même graine : mêmes
//! clés) ; `prereglagesui.rs` les branche sur le document, l'historique et la fenêtre de réglage.
//! Les valeurs en pixels sont proportionnelles à la hauteur du cadre (réglées pour 1080 points) ;
//! l'ampleur des tremblements vient de `camfx.rs` (mêmes générateurs de bruit que les effets de
//! caméra), à régler à l'œil — aucun équivalent dans Storyboard Pro ni dans TVPaint n'a été
//! vérifié (à vérifier).

use crate::blend::BlendMode;
use crate::calqueffet::CalqueEffet;
use crate::camfx;
use crate::fx::{Effect, EffectKind};
use crate::incrustation::Incrustation;

/// Les préréglages proposés.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Genre {
    Impact,
    Lueur,
    FlashBlanc,
    FlashCouleur,
    Tremblement,
    Vignettage,
    Grain,
    Aberration,
    FlouVitesse,
    Timecode,
}

impl Genre {
    pub const ALL: [Genre; 10] = [
        Genre::Impact,
        Genre::Lueur,
        Genre::FlashBlanc,
        Genre::FlashCouleur,
        Genre::Tremblement,
        Genre::Vignettage,
        Genre::Grain,
        Genre::Aberration,
        Genre::FlouVitesse,
        Genre::Timecode,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Genre::Impact => "Image d'impact (impact frame)",
            Genre::Lueur => "Lueur (glow)",
            Genre::FlashBlanc => "Flash blanc",
            Genre::FlashCouleur => "Flash de couleur",
            Genre::Tremblement => "Tremblement de l'image",
            Genre::Vignettage => "Vignettage",
            Genre::Grain => "Grain de film",
            Genre::Aberration => "Aberration chromatique",
            Genre::FlouVitesse => "Flou de vitesse",
            Genre::Timecode => "Timecode et incrustations",
        }
    }

    /// Nom du calque créé.
    pub fn layer_name(self) -> &'static str {
        match self {
            Genre::Impact => "Impact frame",
            Genre::Lueur => "Lueur",
            Genre::FlashBlanc => "Flash blanc",
            Genre::FlashCouleur => "Flash de couleur",
            Genre::Tremblement => "Tremblement",
            Genre::Vignettage => "Vignettage",
            Genre::Grain => "Grain de film",
            Genre::Aberration => "Aberration chromatique",
            Genre::FlouVitesse => "Flou de vitesse",
            Genre::Timecode => "Timecode",
        }
    }

    /// Ce que fait le préréglage, en une phrase (bulle).
    pub fn note(self) -> &'static str {
        match self {
            Genre::Impact => "Une à trois images très contrastées (négatif, noir et blanc), teintées en option, avec un éclat de rayons et des lignes de vitesse. Allumée et éteinte d'un coup.",
            Genre::Lueur => "Une copie floutée et éclaircie des parties claires, ajoutée par-dessus en écran. Pulsation en option.",
            Genre::FlashBlanc => "L'image monte au blanc et redescend, vite.",
            Genre::FlashCouleur => "L'image monte vers une couleur et redescend, vite.",
            Genre::Tremblement => "Secoue l'image déjà dessinée, tout ce qui est dessous : un décalage et une rotation par image tirés d'un bruit à graine (les mêmes générateurs que les effets de caméra), avec un flou de bougé en option.",
            Genre::Vignettage => "Assombrit les bords du cadre.",
            Genre::Grain => "Un grain qui change à chaque image.",
            Genre::Aberration => "Le rouge et le bleu s'écartent un peu du centre.",
            Genre::FlouVitesse => "Un flou dans une direction, qui monte puis retombe.",
            Genre::Timecode => "Du texte posé sur l'image finale : timecode HH:MM:SS:IM, numéro d'image, nom du plan, de la séquence, titre, date ou texte libre. Gravé dans l'export.",
        }
    }

    /// Identifiant de l'action qui l'ouvre (`keymap::ACTIONS`).
    pub fn action(self) -> &'static str {
        match self {
            Genre::Impact => "effet.impact",
            Genre::Lueur => "effet.lueur",
            Genre::FlashBlanc => "effet.flash_blanc",
            Genre::FlashCouleur => "effet.flash_couleur",
            Genre::Tremblement => "effet.tremblement",
            Genre::Vignettage => "effet.vignettage",
            Genre::Grain => "effet.grain",
            Genre::Aberration => "effet.aberration",
            Genre::FlouVitesse => "effet.flou_vitesse",
            Genre::Timecode => "effet.timecode",
        }
    }

    pub fn from_action(action: &str) -> Option<Genre> {
        Genre::ALL.into_iter().find(|g| g.action() == action)
    }

    /// Durée proposée, en images ; rien : jusqu'à la fin du plan.
    pub fn default_frames(self, fps: u32) -> Option<usize> {
        let secs = |s: f32| ((s * fps as f32).round() as usize).max(1);
        match self {
            Genre::Impact => Some(2),
            Genre::FlashBlanc | Genre::FlashCouleur => Some(secs(0.35)),
            Genre::Tremblement => Some(secs(camfx::Preset::Explosion.default_seconds())),
            Genre::FlouVitesse => Some(secs(0.5)),
            Genre::Lueur | Genre::Vignettage | Genre::Grain | Genre::Aberration | Genre::Timecode => None,
        }
    }

    /// Le préréglage se règle par une intensité.
    pub fn has_intensity(self) -> bool {
        self != Genre::Timecode
    }

    /// Il tire des nombres au hasard (graine).
    pub fn has_seed(self) -> bool {
        matches!(self, Genre::Impact | Genre::Tremblement | Genre::Grain)
    }

    /// Il a une couleur à choisir.
    pub fn has_color(self) -> bool {
        matches!(self, Genre::FlashCouleur)
    }

    /// Il écrit des effets (donc peut aller sur le calque courant) plutôt qu'une incrustation.
    pub fn is_effect(self) -> bool {
        self != Genre::Timecode
    }
}

/// Les réglages de la fenêtre d'un préréglage.
#[derive(Clone, Debug, PartialEq)]
pub struct Reglages {
    pub genre: Genre,
    /// 1 = 100 % : la force proposée.
    pub intensite: f32,
    /// Images ; rien : jusqu'à la fin du plan.
    pub duree: Option<usize>,
    pub couleur: [u8; 3],
    pub graine: u32,
    // Image d'impact.
    /// 0 négatif, 1 noir et blanc, 2 noir et blanc inversé, 3 postérisé.
    pub mode_impact: u8,
    pub teinte: bool,
    pub eclat: bool,
    pub lignes: bool,
    // Lueur.
    pub pulsation: bool,
    /// Période de la pulsation, en secondes.
    pub periode: f32,
    /// Luminance (0 à 255) à partir de laquelle ça brille.
    pub seuil: f32,
    // Tremblement.
    pub style: camfx::Preset,
    pub frequence: f32,
    pub flou_bouge: bool,
    // Flou de vitesse.
    pub angle: f32,
    // Timecode.
    pub incrustation: Incrustation,
    /// Sur le calque courant plutôt que sur un calque d'effet neuf (les préréglages d'effets).
    pub sur_calque: bool,
    /// Dans tous les plans du film (les préréglages qui durent jusqu'à la fin du plan).
    pub tous_les_plans: bool,
}

impl Reglages {
    /// Les réglages proposés pour `genre` à `fps` images par seconde.
    pub fn new(genre: Genre, fps: u32) -> Reglages {
        let style = camfx::Preset::Explosion;
        Reglages {
            genre,
            intensite: 1.0,
            duree: genre.default_frames(fps),
            couleur: if genre == Genre::FlashCouleur { [255, 90, 40] } else { [255, 255, 255] },
            graine: 1,
            mode_impact: 0,
            teinte: false,
            eclat: false,
            lignes: false,
            pulsation: false,
            periode: 1.0,
            seuil: 140.0,
            style,
            frequence: style.default_freq(),
            flou_bouge: false,
            angle: 0.0,
            incrustation: Incrustation::default(),
            sur_calque: false,
            tous_les_plans: false,
        }
    }

    /// Les mêmes réglages pour un autre préréglage : l'intensité, la graine et l'endroit restent ;
    /// la durée, la couleur et le reste repartent de ses valeurs.
    pub fn with_genre(&self, genre: Genre, fps: u32) -> Reglages {
        Reglages { intensite: self.intensite, graine: self.graine, sur_calque: self.sur_calque && genre.is_effect(), tous_les_plans: self.tous_les_plans, ..Reglages::new(genre, fps) }
    }

    /// Réglages ramenés dans des bornes sûres.
    pub fn sanitized(&self, fps: u32) -> Reglages {
        let ok = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Reglages {
            intensite: ok(self.intensite, 0.0, 3.0, 1.0),
            duree: self.duree.map(|d| d.clamp(1, 100_000)),
            graine: self.graine.clamp(1, 999),
            mode_impact: self.mode_impact.min(3),
            periode: ok(self.periode, 0.1, 30.0, 1.0),
            seuil: ok(self.seuil, 0.0, 255.0, 140.0),
            frequence: ok(self.frequence, 0.1, (fps as f32 / 2.0).max(1.0), 8.0),
            angle: ok(self.angle, -180.0, 180.0, 0.0),
            ..self.clone()
        }
    }
}

/// Où et comment le préréglage s'applique : le projet tel qu'il est à la tête de lecture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contexte {
    pub fps: u32,
    /// Taille du cadre (l'image finale), en pixels.
    pub cadre: [usize; 2],
    /// Image de départ (la tête de lecture) et nombre d'images du plan.
    pub debut: usize,
    pub images: usize,
    /// Centre du cadre de la caméra sur la feuille, et sa taille, en % de la feuille.
    pub centre: [f32; 2],
    pub taille: [f32; 2],
}

/// Un calque d'effet (ou des effets) prêt à poser.
#[derive(Clone, Debug, PartialEq)]
pub struct Construit {
    pub nom: String,
    pub effet: CalqueEffet,
    /// Les effets, avec leurs clés.
    pub fx: Vec<Effect>,
    /// Les images du calque : de `debut`, sur `longueur` images.
    pub debut: usize,
    pub longueur: usize,
    pub fusion: BlendMode,
    /// La fin a été coupée par la fin du plan.
    pub coupe: bool,
}

/// Un effet réglé : `values` partout, une clé à chaque image de `keys`.
fn keyed(kind: EffectKind, base: &[f32], keys: Vec<(usize, Vec<f32>)>) -> Effect {
    let mut e = Effect::new(kind);
    for (k, v) in base.iter().enumerate() {
        e.set_param(k, *v);
    }
    for (frame, values) in keys {
        e.set_key(frame, values);
    }
    e
}

/// Clés d'un effet allumé d'un coup : « allumé » de `debut` à `debut + longueur - 1`, « éteint » juste
/// avant et juste après (sur le calque courant, où rien d'autre ne l'éteint).
fn hard_keys(debut: usize, longueur: usize, on: impl Fn(usize) -> Vec<f32>, off: Vec<f32>, step: usize, bookends: bool) -> Vec<(usize, Vec<f32>)> {
    let mut keys = Vec::new();
    if bookends && debut > 0 {
        keys.push((debut - 1, off.clone()));
    }
    let mut rel = 0;
    while rel < longueur {
        keys.push((debut + rel, on(rel)));
        rel += step.max(1);
    }
    // (La dernière image allumée porte toujours une clé : sinon l'effet glisserait vers « éteint ».)
    if longueur > 1 && (longueur - 1) % step.max(1) != 0 {
        keys.push((debut + longueur - 1, on(longueur - 1)));
    }
    if bookends {
        keys.push((debut + longueur, off));
    }
    keys
}

/// Clés d'une montée puis d'une descente : 0 à `debut`, le sommet après `montee` images, 0 à la
/// dernière image.
fn pulse_keys(debut: usize, longueur: usize, peak: Vec<f32>, zero: Vec<f32>, bookends: bool) -> Vec<(usize, Vec<f32>)> {
    let montee = (longueur / 5).max(1).min(longueur.saturating_sub(1));
    let mut keys = Vec::new();
    if bookends && debut > 0 {
        keys.push((debut - 1, zero.clone()));
    }
    if montee > 0 {
        keys.push((debut, zero.clone()));
        keys.push((debut + montee, peak));
    } else {
        keys.push((debut, peak));
    }
    if longueur > montee + 1 {
        keys.push((debut + longueur - 1, zero.clone()));
    }
    if bookends {
        keys.push((debut + longueur, zero));
    }
    keys
}

/// Nombre de clés au plus pour une pulsation ou un tremblement (au-delà, elles s'espacent).
pub const MAX_KEYS: usize = camfx::MAX_KEYS;

/// Construit le préréglage `r` à partir de `ctx`. Rien si l'image de départ est après la fin du plan.
pub fn build(r: &Reglages, ctx: &Contexte) -> Option<Construit> {
    let r = r.sanitized(ctx.fps);
    let debut = ctx.debut;
    let restant = ctx.images.checked_sub(debut).filter(|&n| n > 0)?;
    let demande = r.duree.unwrap_or(restant).max(1);
    let longueur = demande.min(restant);
    let coupe = r.duree.is_some() && demande > restant;
    let bookends = r.sur_calque;
    let k = ctx.cadre[1] as f32 / 1080.0;
    let i = r.intensite;
    let pct = |v: f32| (v * i).clamp(0.0, 100.0);
    let step = longueur.div_ceil(MAX_KEYS).max(1);
    let (effet, fx): (CalqueEffet, Vec<Effect>) = match r.genre {
        Genre::Impact => {
            let on = vec![
                pct(100.0),
                r.mode_impact as f32,
                128.0,
                r.couleur[0] as f32,
                r.couleur[1] as f32,
                r.couleur[2] as f32,
                if r.teinte { 70.0 } else { 0.0 },
                if r.eclat { pct(70.0) } else { 0.0 },
                if r.lignes { pct(70.0) } else { 0.0 },
                ctx.centre[0],
                ctx.centre[1],
                r.graine as f32,
            ];
            let off = [vec![0.0], on[1..].to_vec()].concat();
            let keys = hard_keys(debut, longueur, |_| on.clone(), off, 1, bookends);
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Impact, &on, keys)])
        }
        Genre::Lueur => {
            let radius = (30.0 * k).clamp(1.0, 300.0);
            let values = |m: f32| vec![radius, (100.0 * i * m).clamp(0.0, 300.0), r.seuil];
            let off = vec![radius, 0.0, r.seuil];
            let keys = if r.pulsation {
                let period = (r.periode * ctx.fps as f32).max(2.0);
                // Une clé par image (ou plus espacées pour un très long effet) : 1 ± 50 % d'intensité.
                hard_keys(debut, longueur, |rel| values(1.0 + 0.5 * (std::f32::consts::TAU * rel as f32 / period).sin()), off, step, bookends)
            } else if r.duree.is_none() || !bookends {
                vec![(debut, values(1.0))]
            } else {
                hard_keys(debut, longueur, |_| values(1.0), off, longueur, bookends)
            };
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Bloom, &values(1.0), keys)])
        }
        Genre::FlashBlanc | Genre::FlashCouleur => {
            let color = if r.genre == Genre::FlashBlanc { [255, 255, 255] } else { r.couleur };
            let at = |amount: f32| vec![amount, color[0] as f32, color[1] as f32, color[2] as f32, 0.0];
            let keys = pulse_keys(debut, longueur, at(pct(100.0)), at(0.0), bookends);
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Flash, &at(pct(100.0)), keys)])
        }
        Genre::Tremblement => {
            let params = camfx::Params {
                preset: r.style,
                intensity: i.max(0.01),
                duration: longueur,
                freq: r.frequence,
                seed: r.graine,
                zoom_factor: 1.0,
                punch: r.style == camfx::Preset::Explosion,
            };
            let g = camfx::Generator::new(&params, ctx.fps);
            let width = ctx.cadre[0] as f32;
            let blur = if r.flou_bouge { 70.0 } else { 0.0 };
            let at = |rel: usize| {
                let s = g.at(rel);
                vec![s.dx * width, s.dy * width, s.angle, 100.0 * s.zoom, blur]
            };
            let neutral = vec![0.0, 0.0, 0.0, 100.0, blur];
            let mut keys: Vec<(usize, Vec<f32>)> = Vec::new();
            if bookends && debut > 0 {
                keys.push((debut - 1, neutral.clone()));
            }
            for rel in g.frames() {
                if debut + rel < ctx.images || (bookends && rel == longueur) {
                    keys.push((debut + rel, at(rel)));
                }
            }
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Shake, &neutral, keys)])
        }
        Genre::Vignettage => {
            let at = vec![pct(50.0), 60.0, 60.0, ctx.centre[0], ctx.centre[1], ctx.taille[0], ctx.taille[1]];
            let off = [vec![0.0], at[1..].to_vec()].concat();
            let keys = if r.duree.is_none() || !bookends { vec![(debut, at.clone())] } else { hard_keys(debut, longueur, |_| at.clone(), off, longueur, bookends) };
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Vignette, &at, keys)])
        }
        Genre::Grain => {
            let size = k.round().clamp(1.0, 8.0);
            let at = vec![pct(25.0), size, r.graine as f32];
            let off = vec![0.0, size, r.graine as f32];
            let keys = if r.duree.is_none() || !bookends { vec![(debut, at.clone())] } else { hard_keys(debut, longueur, |_| at.clone(), off, longueur, bookends) };
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Grain, &at, keys)])
        }
        Genre::Aberration => {
            let at = vec![(6.0 * k * i).clamp(0.0, 60.0), 50.0, 50.0];
            let off = vec![0.0, 50.0, 50.0];
            let keys = if r.duree.is_none() || !bookends { vec![(debut, at.clone())] } else { hard_keys(debut, longueur, |_| at.clone(), off, longueur, bookends) };
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::Aberration, &at, keys)])
        }
        Genre::FlouVitesse => {
            let at = |len: f32| vec![len, r.angle];
            let peak = (80.0 * k * i).clamp(0.0, 400.0);
            let keys = pulse_keys(debut, longueur, at(peak), at(0.0), bookends);
            (CalqueEffet::ajustement(r.genre.label()), vec![keyed(EffectKind::MotionBlur, &at(peak), keys)])
        }
        Genre::Timecode => (CalqueEffet::incrustation(r.incrustation.clone(), r.genre.label()), Vec::new()),
    };
    // Un préréglage qui s'écrit sur le calque courant n'a pas de calque : la fusion est celle du calque.
    let fusion = BlendMode::Normal;
    Some(Construit { nom: r.genre.layer_name().to_string(), effet, fx, debut, longueur, fusion, coupe })
}
