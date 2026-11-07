//! Clés de position des calques (Anthony, 2026-10-20 : « Outil Transformer : clé de position dans les
//! réglages pour pouvoir animer, déplacer une forme. Possible de lisser les clés »).
//!
//! Un calque a une pose (`LayerTransform` : décalage, échelle, étirement, rotation, autour du centre de
//! la feuille). Sans clé, c'est la pose fixe du calque (`Layer::transform`). Avec des clés, c'est la pose
//! que donnent les clés à l'image : avant la première et après la dernière le calque ne bouge pas ; entre
//! deux clés la pose va de l'une à l'autre en ligne droite, ralentie au départ et à l'arrivée selon les
//! adoucis des deux clés — le modèle des clés de caméra (`camera.rs` : `smooth`, `ease_in`,
//! `ease_out`, la fonction `ease`).
//!
//! Les clés vivent sur le calque (`Layer::pos_keys`), en images du plan. Elles suivent les images quand on
//! en insère ou en retire (`shift_keys`), la coupe d'un plan (`split_keys`) et la réunion de deux plans
//! (`join_keys`) comme les clés d'effets et de caméra.

use serde::{Deserialize, Serialize};

use crate::camera::ease;
use crate::document::LayerTransform;

/// Une clé de position telle qu'elle s'écrit dans le projet (`cles_position` d'un calque, format 33).
/// Les valeurs par défaut (échelle et étirements à 1, rien d'adouci) ne s'écrivent pas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entree {
    pub image: usize,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "un", skip_serializing_if = "est_un")]
    pub echelle: f32,
    #[serde(default, skip_serializing_if = "est_zero")]
    pub rotation: f32,
    #[serde(default = "un", skip_serializing_if = "est_un")]
    pub etirement_x: f32,
    #[serde(default = "un", skip_serializing_if = "est_un")]
    pub etirement_y: f32,
    /// Inclinaison de la carte dans l'Espace 3D, en degrés (format 35) et écart de profondeur de la clé.
    #[serde(default, skip_serializing_if = "est_zero")]
    pub inclinaison_x: f32,
    #[serde(default, skip_serializing_if = "est_zero")]
    pub inclinaison_y: f32,
    #[serde(default, skip_serializing_if = "est_zero")]
    pub ecart_profondeur: f32,
    /// Clé lissée (la pose y arrive et en repart lentement).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lisse: bool,
    /// Adoucis d'entrée et de sortie, en images.
    #[serde(default, skip_serializing_if = "est_zero")]
    pub adoucir_entree: f32,
    #[serde(default, skip_serializing_if = "est_zero")]
    pub adoucir_sortie: f32,
    /// Clé tenue (format 49, l'outil Trajectoire en « clés d'images ») : la pose reste celle de cette clé jusqu'à la suivante, sans
    /// interpolation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tenue: bool,
}

fn un() -> f32 {
    1.0
}
fn est_un(v: &f32) -> bool {
    *v == 1.0
}
fn est_zero(v: &f32) -> bool {
    *v == 0.0
}

impl From<&PosKey> for Entree {
    fn from(k: &PosKey) -> Entree {
        Entree {
            image: k.frame,
            x: k.t.x,
            y: k.t.y,
            echelle: k.t.scale,
            rotation: k.t.angle,
            etirement_x: k.t.stretch_x,
            etirement_y: k.t.stretch_y,
            inclinaison_x: k.t.tilt_x,
            inclinaison_y: k.t.tilt_y,
            ecart_profondeur: k.t.dz,
            lisse: k.smooth,
            adoucir_entree: k.ease_in,
            adoucir_sortie: k.ease_out,
            tenue: k.tenue,
        }
    }
}

impl Entree {
    /// La clé du modèle, ou rien si une valeur est abîmée (pas un nombre).
    pub fn into_model(self) -> Option<PosKey> {
        let vals = [
            self.x,
            self.y,
            self.echelle,
            self.rotation,
            self.etirement_x,
            self.etirement_y,
            self.inclinaison_x,
            self.inclinaison_y,
            self.ecart_profondeur,
            self.adoucir_entree,
            self.adoucir_sortie,
        ];
        if !vals.iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(PosKey {
            frame: self.image,
            t: LayerTransform {
                x: self.x,
                y: self.y,
                scale: self.echelle.clamp(0.01, 100.0),
                angle: self.rotation,
                stretch_x: LayerTransform::clamp_stretch(self.etirement_x),
                stretch_y: LayerTransform::clamp_stretch(self.etirement_y),
                tilt_x: LayerTransform::clamp_tilt(self.inclinaison_x),
                tilt_y: LayerTransform::clamp_tilt(self.inclinaison_y),
                dz: self.ecart_profondeur.clamp(-1000.0, 1000.0),
            },
            smooth: self.lisse,
            ease_in: self.adoucir_entree.max(0.0),
            ease_out: self.adoucir_sortie.max(0.0),
            tenue: self.tenue,
        })
    }
}

/// Des clés lues dans un projet : triées par image, une seule par image (la dernière écrite l'emporte).
pub fn from_entries(entries: Vec<Entree>) -> Vec<PosKey> {
    let mut keys = Vec::new();
    for k in entries.into_iter().filter_map(Entree::into_model) {
        put_key(&mut keys, k);
    }
    keys
}

/// Une clé de position : à l'image `frame` du plan, le calque a la pose `t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PosKey {
    pub frame: usize,
    pub t: LayerTransform,
    /// Lissée : la pose y arrive et en repart lentement, sur la moitié de chaque intervalle.
    pub smooth: bool,
    /// Adoucis d'entrée (avant la clé) et de sortie (après), en images ; 0 : vitesse constante.
    pub ease_in: f32,
    pub ease_out: f32,
    /// Tenue : la pose reste celle de cette clé jusqu'à la suivante, sans interpolation (l'outil Trajectoire en « clés d'images »).
    pub tenue: bool,
}

impl PosKey {
    pub fn new(frame: usize, t: LayerTransform) -> PosKey {
        PosKey { frame, t, smooth: false, ease_in: 0.0, ease_out: 0.0, tenue: false }
    }

    /// Adoucis effectifs sur un intervalle de `span` images : ceux qu'on a réglés, sinon (clé lissée)
    /// un demi-intervalle.
    fn eases(&self, span: f32) -> (f32, f32) {
        let pick = |v: f32| if v > 0.0 { v } else if self.smooth { span * 0.5 } else { 0.0 };
        (pick(self.ease_in), pick(self.ease_out))
    }

    /// La clé a-t-elle un adouci (lissée, ou entrée / sortie réglées) ?
    pub fn is_eased(&self) -> bool {
        self.smooth || self.ease_in > 0.0 || self.ease_out > 0.0
    }
}

/// Un étirement ramené dans ses bornes (le signe, qui dit « retourné », est gardé).
fn lerp_stretch(a: f32, b: f32, k: f32) -> f32 {
    LayerTransform::clamp_stretch(a + (b - a) * k)
}

/// Pose entre `a` et `b` à la fraction `k` (0 : `a`, 1 : `b`).
pub fn lerp(a: &LayerTransform, b: &LayerTransform, k: f32) -> LayerTransform {
    let m = |x: f32, y: f32| x + (y - x) * k;
    LayerTransform {
        x: m(a.x, b.x),
        y: m(a.y, b.y),
        scale: m(a.scale, b.scale).clamp(0.01, 100.0),
        angle: m(a.angle, b.angle),
        stretch_x: lerp_stretch(a.stretch_x, b.stretch_x, k),
        stretch_y: lerp_stretch(a.stretch_y, b.stretch_y, k),
        tilt_x: m(a.tilt_x, b.tilt_x),
        tilt_y: m(a.tilt_y, b.tilt_y),
        dz: m(a.dz, b.dz),
    }
}

/// La pose à l'image `f` d'un calque dont les clés sont `keys` (triées par image) ; sans clé, `base`.
pub fn pose_at(keys: &[PosKey], f: usize, base: LayerTransform) -> LayerTransform {
    match keys {
        [] => base,
        [k] => k.t,
        _ => match keys.iter().position(|k| k.frame > f) {
            None => keys[keys.len() - 1].t,
            Some(0) => keys[0].t,
            Some(i) => {
                let (a, b) = (&keys[i - 1], &keys[i]);
                // Une clé tenue : sa pose jusqu'à la suivante, sans interpolation.
                if a.tenue {
                    return a.t;
                }
                let span = (b.frame - a.frame).max(1) as f32;
                let t = (f - a.frame) as f32 / span;
                let (_, out) = a.eases(span);
                let (inn, _) = b.eases(span);
                lerp(&a.t, &b.t, ease(t, span, out, inn))
            }
        },
    }
}

/// Pose (ou remplace) la clé de l'image `frame`, en gardant les clés triées et l'adoucissement d'une clé
/// remplacée.
pub fn set_key(keys: &mut Vec<PosKey>, frame: usize, t: LayerTransform) {
    match keys.iter().position(|k| k.frame >= frame) {
        Some(i) if keys[i].frame == frame => keys[i].t = t,
        Some(i) => keys.insert(i, PosKey::new(frame, t)),
        None => keys.push(PosKey::new(frame, t)),
    }
}

/// Pose la clé `key` en entier à son image, en gardant les clés triées ; elle remplace celle qui y était.
pub fn put_key(keys: &mut Vec<PosKey>, key: PosKey) {
    match keys.iter().position(|k| k.frame >= key.frame) {
        Some(i) if keys[i].frame == key.frame => keys[i] = key,
        Some(i) => keys.insert(i, key),
        None => keys.push(key),
    }
}

/// La clé de l'image `frame`, s'il y en a une.
pub fn key_at(keys: &[PosKey], frame: usize) -> Option<usize> {
    keys.iter().position(|k| k.frame == frame)
}

/// Les images des clés comprises entre `a` et `b` (dans un sens ou dans l'autre, bornes comprises), pour un
/// choix Maj + clic ; `b` y est toujours, même sans clé.
pub fn frames_between(keys: &[PosKey], a: usize, b: usize) -> Vec<usize> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut out: Vec<usize> = keys.iter().map(|k| k.frame).filter(|f| (lo..=hi).contains(f)).collect();
    if !out.contains(&b) {
        out.push(b);
    }
    out
}

/// Retire la clé de l'image `frame` ; faux s'il n'y en avait pas.
pub fn remove_key(keys: &mut Vec<PosKey>, frame: usize) -> bool {
    let before = keys.len();
    keys.retain(|k| k.frame != frame);
    keys.len() != before
}

/// `n` images insérées à l'image `at` (`removed` : retirées de `at` à `at + n`) : les clés d'après
/// suivent ; celles des images retirées partent.
pub fn shift_keys(keys: &mut Vec<PosKey>, at: usize, n: usize, removed: bool) {
    if n == 0 || keys.is_empty() {
        return;
    }
    keys.retain(|k| !removed || k.frame < at || k.frame >= at + n);
    for k in keys.iter_mut() {
        if k.frame >= at {
            k.frame = if removed { k.frame - n } else { k.frame + n };
        }
    }
}

/// Les clés d'un plan coupé à l'image `at`, pour les deux parties. Le mouvement est gardé : chaque
/// partie reçoit une clé à la coupure (la première à l'image `at - 1`, la seconde à son image 0) si le
/// calque y bougeait. `base` : la pose du calque sans clé.
pub fn split_keys(keys: &[PosKey], at: usize, base: LayerTransform) -> (Vec<PosKey>, Vec<PosKey>) {
    if keys.len() <= 1 {
        return (keys.to_vec(), keys.iter().map(|k| PosKey { frame: 0, ..*k }).collect());
    }
    let mut first: Vec<PosKey> = keys.iter().filter(|k| k.frame < at).copied().collect();
    let mut second: Vec<PosKey> = keys.iter().filter(|k| k.frame >= at).map(|k| PosKey { frame: k.frame - at, ..*k }).collect();
    let before = pose_at(keys, at.saturating_sub(1), base);
    let after = pose_at(keys, at, base);
    if !second.is_empty() && first.last().is_none_or(|k| k.frame + 1 < at) {
        set_key(&mut first, at.saturating_sub(1), before);
    }
    if second.first().is_none_or(|k| k.frame > 0) {
        set_key(&mut second, 0, after);
    }
    (first, second)
}

/// Les clés d'un calque de `a_len` images suivi du même calque d'un autre plan : les siennes, puis celles
/// de l'autre décalées. Une partie sans clé garde sa pose fixe (`a_base`, `b_base`), avec des clés à son
/// début et à sa fin. Rien si aucune des deux n'a de clé et que les poses sont les mêmes.
pub fn join_keys(a: &[PosKey], a_len: usize, a_base: LayerTransform, b: &[PosKey], b_len: usize, b_base: LayerTransform) -> Vec<PosKey> {
    if a.is_empty() && b.is_empty() && a_base == b_base {
        return Vec::new();
    }
    let fixed = |keys: &[PosKey], len: usize, base: LayerTransform| -> Vec<PosKey> {
        if keys.len() >= 2 {
            return keys.to_vec();
        }
        let t = pose_at(keys, 0, base);
        let mut v = vec![PosKey::new(0, t)];
        if len > 1 {
            v.push(PosKey::new(len - 1, t));
        }
        v
    };
    let mut out = fixed(a, a_len, a_base);
    for k in fixed(b, b_len, b_base) {
        put_key(&mut out, PosKey { frame: k.frame + a_len, ..k });
    }
    out
}

/// Règle les adoucis de la clé de l'image `frame` : `smooth` (lissée), entrée et sortie en images.
/// Faux s'il n'y a pas de clé à cette image.
pub fn set_eases(keys: &mut [PosKey], frame: usize, smooth: bool, ease_in: f32, ease_out: f32) -> bool {
    let Some(k) = keys.iter_mut().find(|k| k.frame == frame) else { return false };
    (k.smooth, k.ease_in, k.ease_out) = (smooth, ease_in.max(0.0), ease_out.max(0.0));
    true
}

/// Les clés de `keys` dont l'image est dans `frames` déplacées de `delta` images (les autres restent) ;
/// une clé qui arrive sur une autre la remplace. `max` : dernière image possible (celle du plan).
pub fn moved(keys: &[PosKey], frames: &[usize], delta: isize, max: usize) -> Vec<PosKey> {
    let mut out: Vec<PosKey> = keys.iter().filter(|k| !frames.contains(&k.frame)).copied().collect();
    for k in keys.iter().filter(|k| frames.contains(&k.frame)) {
        let frame = (k.frame as isize + delta).clamp(0, max as isize) as usize;
        put_key(&mut out, PosKey { frame, ..*k });
    }
    out
}

/// Le plus grand décalage, vers la gauche ou vers la droite, qui garde les clés `frames` dans le plan
/// (images 0 à `max`).
pub fn clamp_delta(frames: &[usize], want: isize, max: usize) -> isize {
    let (Some(&lo), Some(&hi)) = (frames.iter().min(), frames.iter().max()) else { return 0 };
    want.clamp(-(lo as isize), max as isize - hi as isize)
}
