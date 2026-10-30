//! Pelure d'oignon (3d-1) : les dessins voisins du calque courant, sous le dessin courant, de plus
//! en plus pâles avec la distance. Valeurs par défaut : 3 avant, 3 après, rouge avant,
//! vert après, teinté, sous le dessin. Réglages de l'application, retenus d'une séance à l'autre.
//!
//! 13f : tout ce que le panneau « Table lumineuse » de TVPaint 12 décrit (textes 000050050–124) :
//! colorisation avant et après chacune à part ; chaque image montrée ou cachée ; une opacité par
//! image et une opacité globale ; curseurs liés (opacité linéaire d'une image à l'autre) ;
//! « Devant » (par-dessus le dessin courant) ; modes instance, image, signet, marque d'image ;
//! réglages rangés sous un nom (le « Bin »). Le « Dégradé » (nom seulement dans les textes de
//! TVPaint) suit, au choix d'Anthony, celui de Photoshop : deux couleurs, de la plus proche à la
//! plus lointaine. Le mode « clé de rig » (calques Puppet) n'est pas repris.

use serde::{Deserialize, Serialize};

use crate::document::{CelId, Layer};

/// Ce que compte « 3 avant ».
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OnionMode {
    /// Les dessins différents les plus proches, quelle que soit leur durée d'exposition
    /// (« mode instance »).
    #[serde(rename = "dessins")]
    Drawings,
    /// Les images voisines dans la timeline : un dessin tenu longtemps peut n'en couvrir aucune
    /// autre ; un dessin vu à plusieurs de ces images n'est montré qu'une fois, au plus près.
    #[serde(rename = "images")]
    Frames,
    /// Les dessins aux signets du plan (13d) les plus proches.
    #[serde(rename = "signets")]
    Bookmarks,
    /// Les dessins portant une marque de couleur (13d) les plus proches.
    #[serde(rename = "marques")]
    Marks,
}

impl OnionMode {
    pub const ALL: [OnionMode; 4] = [OnionMode::Drawings, OnionMode::Frames, OnionMode::Bookmarks, OnionMode::Marks];

    pub fn label(self) -> &'static str {
        match self {
            OnionMode::Drawings => "Instance",
            OnionMode::Frames => "Image",
            OnionMode::Bookmarks => "Signet",
            OnionMode::Marks => "Marque de couleur",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            OnionMode::Drawings => "« 3 avant » = les 3 dessins précédents, quelle que soit leur durée.",
            OnionMode::Frames => "« 3 avant » = les 3 images précédentes : un dessin tenu longtemps cache les autres.",
            OnionMode::Bookmarks => "Seulement les dessins aux signets du plan.",
            OnionMode::Marks => "Seulement les dessins qui portent une marque de couleur.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnionSettings {
    pub enabled: bool,
    pub before: usize,
    pub after: usize,
    /// Opacité globale (13f) : multiplie celle de chaque image. Avant 13f : l'opacité du voisin le
    /// plus proche, ce qui revient au même avec les curseurs liés.
    pub opacity: f32,
    pub mode: OnionMode,
    /// Ancien réglage unique de colorisation (avant 13f) : lu une fois, repris des deux côtés.
    #[serde(skip_serializing)]
    pub tinted: Option<bool>,
    /// Coloriser les images d'avant, d'après (13f ; sinon : leurs vraies couleurs).
    pub tint_before: bool,
    pub tint_after: bool,
    pub before_color: [u8; 3],
    pub after_color: [u8; 3],
    /// Dégradé (13f, sur le modèle de Photoshop : un dégradé linéaire entre deux couleurs) :
    /// l'image la plus proche prend la couleur ci-dessus, la plus lointaine la seconde couleur,
    /// celles du milieu une couleur entre les deux.
    pub gradient_before: bool,
    pub gradient_after: bool,
    pub before_color2: [u8; 3],
    pub after_color2: [u8; 3],
    /// Chaque image d'avant, d'après (par rang, 0 = la plus proche) : montrée ou cachée (13f).
    pub show_before: Vec<bool>,
    pub show_after: Vec<bool>,
    /// Opacité de chaque image d'avant, d'après (par rang ; 13f).
    pub before_opacity: Vec<f32>,
    pub after_opacity: Vec<f32>,
    /// Curseurs liés (13f) : l'opacité décroît en ligne droite de la plus proche à la plus
    /// lointaine ; tirer un curseur les règle tous.
    pub linked: bool,
    /// Devant (13f) : les images de la pelure par-dessus le dessin courant.
    pub front: bool,
    /// Réglages rangés sous un nom (13f, le « Bin » de TVPaint).
    pub bin: Vec<OnionPreset>,
}

/// Des réglages de pelure rangés sous un nom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnionPreset {
    pub name: String,
    pub settings: OnionSettings,
}

impl Default for OnionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            before: 3,
            after: 3,
            opacity: 0.45,
            mode: OnionMode::Drawings,
            tinted: None,
            tint_before: true,
            tint_after: true,
            before_color: [255, 0, 0],
            after_color: [0, 255, 0],
            gradient_before: false,
            gradient_after: false,
            before_color2: [255, 200, 0],
            after_color2: [0, 140, 255],
            show_before: vec![true; MAX_NEIGHBOURS],
            show_after: vec![true; MAX_NEIGHBOURS],
            before_opacity: vec![1.0; MAX_NEIGHBOURS],
            after_opacity: vec![1.0; MAX_NEIGHBOURS],
            linked: true,
            front: false,
            bin: Vec::new(),
        }
    }
}

/// Plus grand nombre de voisins de chaque côté.
pub const MAX_NEIGHBOURS: usize = 12;

/// Part de l'opacité de l'image de rang `k` sur `n` quand les curseurs sont liés : 1 au plus
/// près, puis en ligne droite jusqu'à 1/n pour la plus lointaine.
pub fn ramp(k: usize, n: usize) -> f32 {
    let n = n.max(1);
    (n - k.min(n - 1)) as f32 / n as f32
}

impl OnionSettings {
    /// Reprend l'ancien réglage de colorisation (avant 13f) et complète les listes par rang.
    pub fn migrate(&mut self) {
        if let Some(t) = self.tinted.take() {
            self.tint_before = t;
            self.tint_after = t;
        }
        for list in [&mut self.show_before, &mut self.show_after] {
            list.resize(MAX_NEIGHBOURS, true);
        }
        for list in [&mut self.before_opacity, &mut self.after_opacity] {
            list.resize(MAX_NEIGHBOURS, 1.0);
        }
    }

    /// Opacité propre (0 à 1, sans l'opacité globale) de l'image de rang `k` d'avant ou d'après.
    pub fn rank_opacity(&self, after: bool, k: usize) -> f32 {
        let (ops, n) = if after { (&self.after_opacity, self.after) } else { (&self.before_opacity, self.before) };
        let v = if self.linked { ops.first().copied().unwrap_or(1.0) * ramp(k, n) } else { ops.get(k).copied().unwrap_or(1.0) };
        v.clamp(0.0, 1.0)
    }

    /// Règle l'opacité de l'image de rang `k` ; liés, les curseurs bougent ensemble (la plus
    /// proche est réglée pour que la ligne passe par `v` au rang `k`).
    pub fn set_rank_opacity(&mut self, after: bool, k: usize, v: f32) {
        let n = if after { self.after } else { self.before };
        let linked = self.linked;
        let ops = if after { &mut self.after_opacity } else { &mut self.before_opacity };
        ops.resize(MAX_NEIGHBOURS, 1.0);
        if linked {
            ops[0] = (v / ramp(k, n)).clamp(0.0, 1.0);
        } else if k < ops.len() {
            ops[k] = v.clamp(0.0, 1.0);
        }
    }

    /// Lie ou délie les curseurs ; délier garde les opacités qu'on voyait.
    pub fn set_linked(&mut self, linked: bool) {
        if !linked && self.linked {
            for after in [false, true] {
                let values: Vec<f32> = (0..MAX_NEIGHBOURS).map(|k| self.rank_opacity(after, k)).collect();
                if after {
                    self.after_opacity = values;
                } else {
                    self.before_opacity = values;
                }
            }
        }
        self.linked = linked;
    }

    /// L'image de rang `k` d'avant ou d'après est-elle montrée ?
    pub fn shown(&self, after: bool, k: usize) -> bool {
        let list = if after { &self.show_after } else { &self.show_before };
        list.get(k).copied().unwrap_or(true)
    }

    /// Montre (ou cache) toutes les images, des deux côtés (« Activer / Désactiver tout »).
    pub fn show_all(&mut self, on: bool) {
        self.show_before = vec![on; MAX_NEIGHBOURS];
        self.show_after = vec![on; MAX_NEIGHBOURS];
    }

    /// Couleur de l'image de rang `k` d'avant ou d'après ; rien si ce côté n'est pas colorisé.
    pub fn tint_of(&self, after: bool, k: usize) -> Option<[u8; 3]> {
        let (on, gradient, c1, c2, n) = if after {
            (self.tint_after, self.gradient_after, self.after_color, self.after_color2, self.after)
        } else {
            (self.tint_before, self.gradient_before, self.before_color, self.before_color2, self.before)
        };
        if !on {
            return None;
        }
        if !gradient || n < 2 {
            return Some(c1);
        }
        let t = k.min(n - 1) as f32 / (n - 1) as f32;
        Some(std::array::from_fn(|i| (c1[i] as f32 + (c2[i] as f32 - c1[i] as f32) * t).round() as u8))
    }

    /// Opacité finale (0 à 255) de l'image de rang `k` : globale × la sienne.
    pub fn alpha(&self, after: bool, k: usize) -> u8 {
        (self.opacity.clamp(0.0, 1.0) * self.rank_opacity(after, k) * 255.0).round() as u8
    }

    /// Range les réglages courants sous `name` (remplace un rangement du même nom).
    pub fn store(&mut self, name: &str) {
        let mut settings = self.clone();
        settings.bin.clear();
        let preset = OnionPreset { name: name.to_string(), settings };
        match self.bin.iter_mut().find(|p| p.name == name) {
            Some(p) => *p = preset,
            None => self.bin.push(preset),
        }
    }

    /// Reprend les réglages rangés sous `name` (la pelure reste allumée ou éteinte, le Bin reste).
    pub fn recall(&mut self, name: &str) -> bool {
        let Some(stored) = self.bin.iter().find(|p| p.name == name).map(|p| p.settings.clone()) else { return false };
        let (enabled, bin) = (self.enabled, std::mem::take(&mut self.bin));
        *self = OnionSettings { enabled, bin, ..stored };
        self.migrate();
        true
    }
}

/// Images de pelure au plus de chaque côté (réglage du diagnostic des performances ; sans plafond,
/// le maximum de la pelure).
static CAP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(MAX_NEIGHBOURS);

pub fn set_cap(cap: usize) {
    CAP.store(cap.min(MAX_NEIGHBOURS), std::sync::atomic::Ordering::Relaxed);
}

/// Voisins du dessin de l'image `frame` : (dessin, rang), rang 0 = le plus proche. Le dessin
/// courant et les images vides ne sont jamais montrés. `bookmarks` : les signets du plan (mode
/// signet).
pub fn neighbours(layer: &Layer, frame: usize, s: &OnionSettings, bookmarks: &[usize]) -> (Vec<(CelId, usize)>, Vec<(CelId, usize)>) {
    neighbours_within(layer, frame, s, bookmarks, CAP.load(std::sync::atomic::Ordering::Relaxed))
}

/// Comme `neighbours`, avec au plus `cap` images de chaque côté : le plafond du diagnostic des
/// performances borne les voisins montrés, sans toucher au réglage de la pelure.
pub fn neighbours_within(layer: &Layer, frame: usize, s: &OnionSettings, bookmarks: &[usize], cap: usize) -> (Vec<(CelId, usize)>, Vec<(CelId, usize)>) {
    let (max_before, max_after) = (s.before.min(cap), s.after.min(cap));
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
            while f > 0 && before.len() < max_before {
                f -= 1;
                let rank = before.len();
                add(&mut before, f, rank);
            }
            let mut f = frame + 1;
            while f < len && after.len() < max_after {
                let rank = after.len();
                add(&mut after, f, rank);
                f += 1;
            }
        }
        OnionMode::Frames => {
            for d in 1..=max_before.min(frame) {
                add(&mut before, frame - d, d - 1);
            }
            for d in 1..=max_after {
                if frame + d < len {
                    add(&mut after, frame + d, d - 1);
                }
            }
        }
        // Seulement certaines images : les signets, ou celles dont le dessin porte une marque.
        OnionMode::Bookmarks | OnionMode::Marks => {
            let stop = |f: usize| match s.mode {
                OnionMode::Bookmarks => bookmarks.contains(&f),
                _ => layer.cel_at(f).is_some_and(|id| layer.color_marks.contains_key(&id)),
            };
            for f in (0..frame.min(len)).rev().filter(|&f| stop(f)) {
                if before.len() >= max_before {
                    break;
                }
                let rank = before.len();
                add(&mut before, f, rank);
            }
            for f in (frame + 1..len).filter(|&f| stop(f)) {
                if after.len() >= max_after {
                    break;
                }
                let rank = after.len();
                add(&mut after, f, rank);
            }
        }
    }
    (before, after)
}
