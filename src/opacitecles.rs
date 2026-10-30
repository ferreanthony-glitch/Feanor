//! Clés d'opacité des calques (Anthony, 2026-10-29 : « On doit rajouter des clés d'opacité à SHOT »).
//!
//! Le modèle est celui des clés de position (`poskeys.rs`) : les clés vivent sur le calque (`Layer::opacity_keys`), en images du
//! plan, triées par image ; avant la première et après la dernière l'opacité ne bouge pas ; entre deux clés elle va de l'une à
//! l'autre en ligne droite, ralentie au départ et à l'arrivée selon les adoucis des deux clés (la fonction `camera::ease`, les
//! mêmes `smooth`, `ease_in`, `ease_out`). Sans clé, c'est l'opacité fixe du calque (`Layer::opacity`).
//!
//! Elles suivent les images quand on en insère ou en retire (`shift_keys`), la coupe d'un plan (`split_keys`) et la réunion de deux
//! plans (`join_keys`), comme les clés de position et d'effets.
//!
//! API pour un import : `Layer::set_opacity_at` pose une clé, `Layer::set_opacity_keys` en pose plusieurs d'un coup.

use serde::{Deserialize, Serialize};

use crate::camera::ease;

/// Une clé d'opacité : à l'image `frame` du plan, le calque a l'opacité `value` (0 à 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpKey {
    pub frame: usize,
    pub value: f32,
    /// Lissée : l'opacité y arrive et en repart lentement, sur la moitié de chaque intervalle.
    pub smooth: bool,
    /// Adoucis d'entrée (avant la clé) et de sortie (après), en images ; 0 : vitesse constante.
    pub ease_in: f32,
    pub ease_out: f32,
}

impl OpKey {
    pub fn new(frame: usize, value: f32) -> OpKey {
        OpKey { frame, value: clamp(value), smooth: false, ease_in: 0.0, ease_out: 0.0 }
    }

    fn eases(&self, span: f32) -> (f32, f32) {
        let pick = |v: f32| if v > 0.0 { v } else if self.smooth { span * 0.5 } else { 0.0 };
        (pick(self.ease_in), pick(self.ease_out))
    }

    /// La clé a-t-elle un adouci (lissée, ou entrée / sortie réglées) ?
    pub fn is_eased(&self) -> bool {
        self.smooth || self.ease_in > 0.0 || self.ease_out > 0.0
    }
}

/// Une opacité ramenée entre 0 et 1 (une valeur abîmée : 1).
pub fn clamp(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 }
}

/// Une clé d'opacité telle qu'elle s'écrit dans le projet (`cles_opacite` d'un calque, format 43).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entree {
    pub image: usize,
    pub valeur: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lisse: bool,
    #[serde(default, skip_serializing_if = "est_zero")]
    pub adoucir_entree: f32,
    #[serde(default, skip_serializing_if = "est_zero")]
    pub adoucir_sortie: f32,
}

fn est_zero(v: &f32) -> bool {
    *v == 0.0
}

impl From<&OpKey> for Entree {
    fn from(k: &OpKey) -> Entree {
        Entree { image: k.frame, valeur: k.value, lisse: k.smooth, adoucir_entree: k.ease_in, adoucir_sortie: k.ease_out }
    }
}

impl Entree {
    /// La clé du modèle, ou rien si une valeur est abîmée (pas un nombre).
    pub fn into_model(self) -> Option<OpKey> {
        if ![self.valeur, self.adoucir_entree, self.adoucir_sortie].iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(OpKey { frame: self.image, value: clamp(self.valeur), smooth: self.lisse, ease_in: self.adoucir_entree.max(0.0), ease_out: self.adoucir_sortie.max(0.0) })
    }
}

/// Des clés lues dans un projet : triées par image, une seule par image (la dernière écrite l'emporte).
pub fn from_entries(entries: Vec<Entree>) -> Vec<OpKey> {
    let mut keys = Vec::new();
    for k in entries.into_iter().filter_map(Entree::into_model) {
        put_key(&mut keys, k);
    }
    keys
}

/// L'opacité à l'image `f` d'un calque dont les clés sont `keys` (triées par image) ; sans clé, `base`.
pub fn value_at(keys: &[OpKey], f: usize, base: f32) -> f32 {
    match keys {
        [] => base,
        [k] => k.value,
        _ => match keys.iter().position(|k| k.frame > f) {
            None => keys[keys.len() - 1].value,
            Some(0) => keys[0].value,
            Some(i) => {
                let (a, b) = (&keys[i - 1], &keys[i]);
                let span = (b.frame - a.frame).max(1) as f32;
                let t = (f - a.frame) as f32 / span;
                let (_, out) = a.eases(span);
                let (inn, _) = b.eases(span);
                let k = ease(t, span, out, inn);
                clamp(a.value + (b.value - a.value) * k)
            }
        },
    }
}

/// Pose (ou remplace) la clé de l'image `frame`, en gardant les clés triées et l'adoucissement d'une clé remplacée.
pub fn set_key(keys: &mut Vec<OpKey>, frame: usize, value: f32) {
    match keys.iter().position(|k| k.frame >= frame) {
        Some(i) if keys[i].frame == frame => keys[i].value = clamp(value),
        Some(i) => keys.insert(i, OpKey::new(frame, value)),
        None => keys.push(OpKey::new(frame, value)),
    }
}

/// Pose la clé `key` en entier à son image, en gardant les clés triées ; elle remplace celle qui y était.
pub fn put_key(keys: &mut Vec<OpKey>, key: OpKey) {
    let key = OpKey { value: clamp(key.value), ..key };
    match keys.iter().position(|k| k.frame >= key.frame) {
        Some(i) if keys[i].frame == key.frame => keys[i] = key,
        Some(i) => keys.insert(i, key),
        None => keys.push(key),
    }
}

/// La clé de l'image `frame`, s'il y en a une.
pub fn key_at(keys: &[OpKey], frame: usize) -> Option<usize> {
    keys.iter().position(|k| k.frame == frame)
}

/// Les images des clés comprises entre `a` et `b` (bornes comprises), pour un choix Maj + clic ; `b` y est toujours.
pub fn frames_between(keys: &[OpKey], a: usize, b: usize) -> Vec<usize> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut out: Vec<usize> = keys.iter().map(|k| k.frame).filter(|f| (lo..=hi).contains(f)).collect();
    if !out.contains(&b) {
        out.push(b);
    }
    out
}

/// Retire la clé de l'image `frame` ; faux s'il n'y en avait pas.
pub fn remove_key(keys: &mut Vec<OpKey>, frame: usize) -> bool {
    let before = keys.len();
    keys.retain(|k| k.frame != frame);
    keys.len() != before
}

/// `n` images insérées à l'image `at` (`removed` : retirées de `at` à `at + n`) : les clés d'après suivent ; celles des images
/// retirées partent.
pub fn shift_keys(keys: &mut Vec<OpKey>, at: usize, n: usize, removed: bool) {
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

/// Les clés d'un plan coupé à l'image `at`, pour les deux parties. Le mouvement est gardé : chaque partie reçoit une clé à la
/// coupure (la première à l'image `at - 1`, la seconde à son image 0) si l'opacité y bougeait. `base` : l'opacité sans clé.
pub fn split_keys(keys: &[OpKey], at: usize, base: f32) -> (Vec<OpKey>, Vec<OpKey>) {
    if keys.len() <= 1 {
        return (keys.to_vec(), keys.iter().map(|k| OpKey { frame: 0, ..*k }).collect());
    }
    let mut first: Vec<OpKey> = keys.iter().filter(|k| k.frame < at).copied().collect();
    let mut second: Vec<OpKey> = keys.iter().filter(|k| k.frame >= at).map(|k| OpKey { frame: k.frame - at, ..*k }).collect();
    let before = value_at(keys, at.saturating_sub(1), base);
    let after = value_at(keys, at, base);
    if !second.is_empty() && first.last().is_none_or(|k| k.frame + 1 < at) {
        set_key(&mut first, at.saturating_sub(1), before);
    }
    if second.first().is_none_or(|k| k.frame > 0) {
        set_key(&mut second, 0, after);
    }
    (first, second)
}

/// Les clés d'un calque de `a_len` images suivi du même calque d'un autre plan : les siennes, puis celles de l'autre décalées. Une
/// partie sans clé garde son opacité fixe (`a_base`, `b_base`), avec des clés à son début et à sa fin. Rien si aucune des deux n'a de
/// clé et que les opacités sont les mêmes.
pub fn join_keys(a: &[OpKey], a_len: usize, a_base: f32, b: &[OpKey], b_len: usize, b_base: f32) -> Vec<OpKey> {
    if a.is_empty() && b.is_empty() && a_base == b_base {
        return Vec::new();
    }
    let fixed = |keys: &[OpKey], len: usize, base: f32| -> Vec<OpKey> {
        if keys.len() >= 2 {
            return keys.to_vec();
        }
        let v = value_at(keys, 0, base);
        let mut out = vec![OpKey::new(0, v)];
        if len > 1 {
            out.push(OpKey::new(len - 1, v));
        }
        out
    };
    let mut out = fixed(a, a_len, a_base);
    for k in fixed(b, b_len, b_base) {
        put_key(&mut out, OpKey { frame: k.frame + a_len, ..k });
    }
    out
}

/// Règle les adoucis de la clé de l'image `frame` : `smooth` (lissée), entrée et sortie en images. Faux s'il n'y a pas de clé là.
pub fn set_eases(keys: &mut [OpKey], frame: usize, smooth: bool, ease_in: f32, ease_out: f32) -> bool {
    let Some(k) = keys.iter_mut().find(|k| k.frame == frame) else { return false };
    (k.smooth, k.ease_in, k.ease_out) = (smooth, ease_in.max(0.0), ease_out.max(0.0));
    true
}

/// Les clés de `keys` dont l'image est dans `frames` déplacées de `delta` images (les autres restent) ; une clé qui arrive sur une
/// autre la remplace. `max` : dernière image possible (celle du plan).
pub fn moved(keys: &[OpKey], frames: &[usize], delta: isize, max: usize) -> Vec<OpKey> {
    let mut out: Vec<OpKey> = keys.iter().filter(|k| !frames.contains(&k.frame)).copied().collect();
    for k in keys.iter().filter(|k| frames.contains(&k.frame)) {
        let frame = (k.frame as isize + delta).clamp(0, max as isize) as usize;
        put_key(&mut out, OpKey { frame, ..*k });
    }
    out
}

/// Le plus grand décalage qui garde les clés `frames` dans le plan (images 0 à `max`).
pub fn clamp_delta(frames: &[usize], want: isize, max: usize) -> isize {
    let (Some(&lo), Some(&hi)) = (frames.iter().min(), frames.iter().max()) else { return 0 };
    want.clamp(-(lo as isize), max as isize - hi as isize)
}
