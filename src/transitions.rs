//! Transitions du montage (Anthony, 2026-10-19) : fondu enchaîné, fondu au noir, fondu au blanc,
//! volet et noir, sur une coupe entre deux plans ou au début / à la fin du film.
//!
//! # Modèle de temps (choisi, à confirmer à l'écran)
//!
//! **La durée du film ne change pas** : une transition n'avance ni ne recule aucun plan (le son, les
//! commentaires et les repères restent calés). Elle se pose **autour de la coupe** entre le plan
//! sortant A et le plan entrant B, à l'intérieur de leurs images :
//!
//! - « Centrée sur la coupe » (par défaut) : `n / 2` images avant la coupe, `n - n / 2` après ;
//! - « Finit à la coupe » : les `n` images d'avant la coupe ;
//! - « Commence à la coupe » : les `n` images d'après.
//!
//! Dans la zone, A et B sont tous deux montrés. Un plan n'a pas d'images au-delà de ses bords : la
//! partie manquante est **tenue** (A garde sa dernière image après la coupe, B sa première avant
//! elle). Les poignées du rognage (la réserve de `rognage.rs`) ne sont pas lues : à faire (une
//! transition y gagnerait des images vivantes au lieu d'images tenues). La zone ne sort jamais du
//! plan sortant ni du plan entrant (une transition plus longue qu'un plan est raccourcie).
//!
//! Au début du film, « Fondu au noir / au blanc » est un fondu **depuis** la couleur (les `n`
//! premières images, la première est toute noire) ; à la fin, un fondu **vers** la couleur (les `n`
//! dernières, la dernière est toute noire). Sur une coupe, il passe par la couleur : A s'éteint dans
//! la première moitié, B s'allume dans la seconde.
//!
//! # Rendu
//!
//! Une image du film dans une zone est un `Mix` : quelles images de quels plans (`a`, `b`), le poids
//! de B (`m`), un volet éventuel et une couche de couleur opaque (`color`, `k`). La carte graphique
//! (`gpu.rs`, lecture à l'écran) et le calcul sur processeur (`blend_images`, export) lisent le même
//! `Mix`, donc le même résultat à l'arrondi près.
//!
//! # Fichier
//!
//! `transitions.json` à côté du manifeste, sans changer `VERSION` (même principe que
//! `commentaires.json` et `rognages.json`) : absent, il n'y a pas de transition ; un fichier illisible
//! ou d'une version plus récente n'empêche pas d'ouvrir le projet et n'est jamais effacé
//! (`project::write_sidecar`, copie `.illisible` quand un fichier neuf doit le remplacer). Les
//! transitions sont dans `Structure` : une étape d'annulation les rétablit avec le reste.

use std::collections::HashSet;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::document::Document;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "transitions.json";
const FORMAT: &str = "shot-transitions";
const VERSION: u32 = 1;
/// Durée d'une transition neuve, en images.
pub const DEFAULT_FRAMES: usize = 12;
/// La plus longue (un fichier abîmé ne fait pas calculer des milliers d'images doubles).
pub const MAX_FRAMES: usize = 240;

/// Sorte de transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// Fondu enchaîné : A se fond dans B.
    #[serde(rename = "enchaine")]
    Dissolve,
    /// Fondu au noir (par le noir sur une coupe ; depuis / vers le noir au bord du film).
    #[serde(rename = "noir")]
    Black,
    /// Fondu au blanc.
    #[serde(rename = "blanc")]
    White,
    /// Volet : B recouvre A de gauche à droite.
    #[serde(rename = "volet")]
    Wipe,
    /// Noir : les images de la zone sont toutes noires (coupe sèche vers le noir et retour).
    #[serde(rename = "coupe_noir")]
    CutBlack,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Dissolve, Kind::Black, Kind::White, Kind::Wipe, Kind::CutBlack];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Dissolve => "Fondu enchaîné",
            Kind::Black => "Fondu au noir",
            Kind::White => "Fondu au blanc",
            Kind::Wipe => "Volet",
            Kind::CutBlack => "Noir",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Kind::Dissolve => "Le plan sortant se fond dans le plan entrant.",
            Kind::Black => "Le plan sortant s'éteint dans le noir, puis le plan entrant s'allume (au bord du film : depuis ou vers le noir).",
            Kind::White => "Comme le fondu au noir, par le blanc.",
            Kind::Wipe => "Le plan entrant recouvre le plan sortant de gauche à droite.",
            Kind::CutBlack => "La coupe passe par des images toutes noires, sans fondu.",
        }
    }

    /// Sorte permise sur une coupe (entre deux plans) ou au bord du film.
    pub fn allowed(self, between_plans: bool) -> bool {
        between_plans || !matches!(self, Kind::Dissolve | Kind::Wipe)
    }

    /// La couleur de la couche (noir ou blanc).
    fn color(self) -> [u8; 3] {
        if self == Kind::White {
            [255, 255, 255]
        } else {
            [0, 0, 0]
        }
    }
}

/// Où la zone se pose par rapport à la coupe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    #[default]
    #[serde(rename = "centree")]
    Centre,
    /// Les images d'avant la coupe.
    #[serde(rename = "avant")]
    Before,
    /// Les images d'après la coupe.
    #[serde(rename = "apres")]
    After,
}

impl Align {
    pub const ALL: [Align; 3] = [Align::Centre, Align::Before, Align::After];

    pub fn name(self) -> &'static str {
        match self {
            Align::Centre => "Centrée sur la coupe",
            Align::Before => "Finit à la coupe",
            Align::After => "Commence à la coupe",
        }
    }

    /// (images avant la coupe, images après) pour une durée de `n` images.
    pub fn split(self, n: usize) -> (usize, usize) {
        match self {
            Align::Centre => (n / 2, n - n / 2),
            Align::Before => (n, 0),
            Align::After => (0, n),
        }
    }

    /// Durée qui amène le bord de la zone (`left` : le bord gauche, sinon le droit) à l'image `edge`
    /// du film, la coupe étant à l'image `cut`. En centré, la zone reste centrée : la durée est le
    /// double de la distance de ce bord à la coupe.
    pub fn frames_for_edge(self, cut: usize, edge: usize, left: bool) -> usize {
        let d = if left { cut.saturating_sub(edge) } else { edge.saturating_sub(cut) };
        match self {
            Align::Centre => (d * 2).clamp(1, MAX_FRAMES),
            Align::Before => if left { d } else { 1 }.clamp(1, MAX_FRAMES),
            Align::After => if left { 1 } else { d }.clamp(1, MAX_FRAMES),
        }
    }
}

/// Une transition : la coupe qu'elle habille (les plans de part et d'autre, par leur numéro), sa
/// sorte, sa durée et son calage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub id: u64,
    /// Le plan sortant (avant la coupe) ; rien : début du film.
    #[serde(rename = "sortant", default)]
    pub out: Option<u64>,
    /// Le plan entrant (après la coupe) ; rien : fin du film.
    #[serde(rename = "entrant", default)]
    pub inn: Option<u64>,
    #[serde(rename = "sorte")]
    pub kind: Kind,
    #[serde(rename = "images")]
    pub frames: usize,
    #[serde(rename = "calage", default, skip_serializing_if = "is_centre")]
    pub align: Align,
}

fn is_centre(a: &Align) -> bool {
    *a == Align::Centre
}

impl Transition {
    pub fn between_plans(&self) -> bool {
        self.out.is_some() && self.inn.is_some()
    }
}

// ------------------------------------------------------------------------------------- zones

/// Un plan de part ou d'autre de la zone : son rang, sa première image dans le film, sa durée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Side {
    pub panel: usize,
    pub start: usize,
    pub len: usize,
}

/// Les images du film qu'une transition recouvre, [start, end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub id: u64,
    pub kind: Kind,
    /// L'image de la coupe (première image du plan entrant ; 0 au début du film, le total à la fin).
    pub cut: usize,
    pub start: usize,
    pub end: usize,
    pub out: Option<Side>,
    pub inn: Option<Side>,
    pub align: Align,
}

/// Ce que montre une image du film dans une zone (voir l'en-tête du module).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    /// Plan et image du plan sortant, s'il se voit.
    pub a: Option<(usize, usize)>,
    /// Plan et image du plan entrant, s'il se voit.
    pub b: Option<(usize, usize)>,
    /// Volet (B recouvre A jusqu'à `m` de la largeur) au lieu d'un fondu.
    pub wipe: bool,
    /// Poids de B, de 0 (A seul) à 1 (B seul).
    pub m: f32,
    /// Couche de couleur opaque posée par-dessus, d'opacité `k`.
    pub color: [u8; 3],
    pub k: f32,
}

impl Zone {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn contains(&self, g: usize) -> bool {
        (self.start..self.end).contains(&g)
    }

    /// L'image `g` du film dans cette zone ; rien si elle n'y est pas.
    pub fn mix(&self, g: usize) -> Option<Mix> {
        if !self.contains(g) {
            return None;
        }
        let n = self.len();
        let i = g - self.start;
        // Une image du plan, tenue au-delà de ses bords.
        let held = |s: &Side| (s.panel, g.saturating_sub(s.start).min(s.len.saturating_sub(1)));
        let (a, b) = (self.out.as_ref().map(held), self.inn.as_ref().map(held));
        let color = self.kind.color();
        let mut mix = Mix { a, b, wipe: false, m: 0.0, color, k: 0.0 };
        match (self.kind, a, b) {
            (Kind::CutBlack, _, _) => {
                mix.k = 1.0;
                mix.color = [0, 0, 0];
                if a.is_none() {
                    mix.m = 1.0;
                } else {
                    mix.b = None;
                }
            }
            (Kind::Dissolve, Some(_), Some(_)) => mix.m = (i + 1) as f32 / (n + 1) as f32,
            (Kind::Wipe, Some(_), Some(_)) => {
                mix.wipe = true;
                mix.m = (i + 1) as f32 / (n + 1) as f32;
            }
            // Par la couleur, sur une coupe : A s'éteint, puis B s'allume.
            (_, Some(_), Some(_)) => {
                let t = (i + 1) as f32 / (n + 1) as f32;
                if t < 0.5 {
                    mix.b = None;
                    mix.k = 2.0 * t;
                } else {
                    mix.a = None;
                    mix.m = 1.0;
                    mix.k = 2.0 * (1.0 - t);
                }
            }
            // Début du film : le plan entrant s'allume, la première image est toute de couleur.
            (_, None, Some(_)) => {
                mix.m = 1.0;
                mix.k = 1.0 - i as f32 / n as f32;
            }
            // Fin du film : le plan sortant s'éteint, la dernière image est toute de couleur.
            (_, Some(_), None) => mix.k = (i + 1) as f32 / n as f32,
            (_, None, None) => return None,
        }
        Some(mix)
    }
}

/// Les zones des transitions du document, par ordre de début. Une transition dont les plans ne se
/// suivent plus (ils ont été déplacés) n'a pas de zone : elle est gardée mais ne joue pas.
pub fn zones(doc: &Document) -> Vec<Zone> {
    if doc.transitions.is_empty() {
        return Vec::new();
    }
    let mut starts = Vec::with_capacity(doc.panels.len());
    let mut at = 0;
    for p in &doc.panels {
        starts.push(at);
        at += p.frame_count();
    }
    let total = at;
    let index = |id: u64| doc.panels.iter().position(|p| p.id == id);
    let side = |i: usize| Side { panel: i, start: starts[i], len: doc.panels[i].frame_count() };
    let mut out = Vec::new();
    for t in &doc.transitions {
        let n = t.frames.clamp(1, MAX_FRAMES);
        let (o, i) = match (t.out, t.inn) {
            (Some(o), Some(i)) => match (index(o), index(i)) {
                (Some(o), Some(i)) if i == o + 1 => (Some(o), Some(i)),
                _ => continue,
            },
            (None, Some(i)) => match index(i) {
                Some(0) => (None, Some(0)),
                _ => continue,
            },
            (Some(o), None) => match index(o) {
                Some(o) if o + 1 == doc.panels.len() => (Some(o), None),
                _ => continue,
            },
            (None, None) => continue,
        };
        let (cut, start, end) = match (o, i) {
            (Some(o), Some(i)) => {
                let cut = starts[i];
                let (before, after) = t.align.split(n);
                let lo = starts[o];
                let hi = starts[i] + doc.panels[i].frame_count();
                (cut, cut.saturating_sub(before).max(lo), (cut + after).min(hi))
            }
            (None, Some(i)) => (0, 0, n.min(doc.panels[i].frame_count())),
            (Some(o), None) => (total, total.saturating_sub(n).max(starts[o]), total),
            (None, None) => continue,
        };
        if end <= start {
            continue;
        }
        out.push(Zone { id: t.id, kind: t.kind, cut, start, end, out: o.map(side), inn: i.map(side), align: t.align });
    }
    out.sort_by_key(|z| (z.start, z.id));
    out
}

/// Ce que montre l'image `g` du film, si une transition la recouvre (la première, si deux se
/// recouvrent).
pub fn mix_in(zones: &[Zone], g: usize) -> Option<(&Zone, Mix)> {
    zones.iter().find_map(|z| z.mix(g).map(|m| (z, m)))
}

// --------------------------------------------------------------------------- rendu processeur

#[inline]
fn lerp8(x: u8, y: u8, t: u32) -> u8 {
    ((x as u32 * (256 - t) + y as u32 * t + 128) >> 8) as u8
}

#[inline]
fn lerp_px(x: Color32, y: Color32, t: u32) -> Color32 {
    let (x, y) = (x.to_array(), y.to_array());
    Color32::from_rgba_premultiplied(lerp8(x[0], y[0], t), lerp8(x[1], y[1], t), lerp8(x[2], y[2], t), lerp8(x[3], y[3], t))
}

/// Le poids `m` (0 à 1) en 256es.
fn weight(m: f32) -> u32 {
    (m.clamp(0.0, 1.0) * 256.0).round() as u32
}

/// Mélange les images `a` et `b` (celles du `Mix`, calculées par l'appelant, à la taille `size`) :
/// le même calcul que la carte graphique, sur le processeur. Les couleurs sont prémultipliées.
pub fn blend_images(mix: &Mix, a: Option<&[Color32]>, b: Option<&[Color32]>, size: [usize; 2]) -> Vec<Color32> {
    let [w, h] = size;
    let n = w * h;
    let pick = |img: Option<&[Color32]>, i: usize| img.map_or(Color32::TRANSPARENT, |p| p[i]);
    let m = weight(mix.m);
    let k = weight(mix.k);
    let layer = Color32::from_rgba_premultiplied(mix.color[0], mix.color[1], mix.color[2], 255);
    // Colonne où le volet s'arrête : à gauche, B ; à droite, A.
    let edge = (mix.m.clamp(0.0, 1.0) * w as f32).round() as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = i % w.max(1);
        let base = match (mix.wipe, a.is_some(), b.is_some()) {
            (true, true, true) => {
                if x < edge {
                    pick(b, i)
                } else {
                    pick(a, i)
                }
            }
            (_, true, true) => lerp_px(pick(a, i), pick(b, i), m),
            (_, true, false) => pick(a, i),
            (_, false, true) => pick(b, i),
            (_, false, false) => Color32::TRANSPARENT,
        };
        out.push(if k == 0 { base } else { lerp_px(base, layer, k) });
    }
    out
}

// ------------------------------------------------------------------------------- opérations

/// Les deux plans de part et d'autre de la coupe `cut` (0 : début du film, `panels.len()` : fin).
pub fn cut_plans(doc: &Document, cut: usize) -> (Option<u64>, Option<u64>) {
    let out = cut.checked_sub(1).and_then(|i| doc.panels.get(i)).map(|p| p.id);
    let inn = doc.panels.get(cut).map(|p| p.id);
    (out, inn)
}

impl Document {
    /// La transition posée sur cette coupe, s'il y en a une.
    pub fn transition_on(&self, out: Option<u64>, inn: Option<u64>) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.out == out && t.inn == inn)
    }

    pub fn transition(&self, id: u64) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.id == id)
    }

    /// La coupe (0 à `panels.len()`) la plus proche de l'image `g` du film.
    pub fn nearest_cut(&self, g: usize) -> usize {
        let mut best = (0, usize::MAX);
        let mut at = 0usize;
        for c in 0..=self.panels.len() {
            let d = at.abs_diff(g);
            if d < best.1 {
                best = (c, d);
            }
            if let Some(p) = self.panels.get(c) {
                at += p.frame_count();
            }
        }
        best.0
    }

    /// Pose une transition de sorte `kind` sur la coupe `cut`, ou change celle qui y est (sa durée
    /// et son calage restent). Renvoie son numéro ; rien si la sorte n'y est pas permise ou si la
    /// coupe n'existe pas.
    pub fn set_transition(&mut self, cut: usize, kind: Kind) -> Option<u64> {
        if cut > self.panels.len() {
            return None;
        }
        let (out, inn) = cut_plans(self, cut);
        if !kind.allowed(out.is_some() && inn.is_some()) {
            return None;
        }
        if let Some(t) = self.transitions.iter_mut().find(|t| t.out == out && t.inn == inn) {
            t.kind = kind;
            return Some(t.id);
        }
        let id = self.alloc();
        self.transitions.push(Transition { id, out, inn, kind, frames: DEFAULT_FRAMES, align: Align::Centre });
        Some(id)
    }

    pub fn remove_transition(&mut self, id: u64) -> bool {
        let n = self.transitions.len();
        self.transitions.retain(|t| t.id != id);
        self.transitions.len() != n
    }

    /// Les transitions dont un plan n'existe plus disparaissent (avec leur plan ; l'annulation les
    /// ramène avec lui, `Structure`).
    pub(crate) fn prune_transitions(&mut self) {
        if self.transitions.is_empty() {
            return;
        }
        let ids: HashSet<u64> = self.panels.iter().map(|p| p.id).collect();
        self.transitions.retain(|t| t.out.is_none_or(|o| ids.contains(&o)) && t.inn.is_none_or(|i| ids.contains(&i)));
    }

    /// Ce que montre l'image `g` du film (voir `mix_in`) ; rien sans transition.
    pub fn mix_at(&self, g: usize) -> Option<(Zone, Mix)> {
        if self.transitions.is_empty() {
            return None;
        }
        let zones = zones(self);
        mix_in(&zones, g).map(|(z, m)| (*z, m))
    }
}

// --------------------------------------------------------------------------------- fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(default)]
    transitions: Vec<Transition>,
}

/// Le contenu de `transitions.json` pour `doc` ; rien s'il n'y a aucune transition (le fichier est
/// alors retiré). Celles d'un plan qui n'existe plus ne sont pas écrites.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let ids: HashSet<u64> = doc.panels.iter().map(|p| p.id).collect();
    let order = |t: &Transition| t.inn.or(t.out).and_then(|id| doc.panels.iter().position(|p| p.id == id)).unwrap_or(usize::MAX);
    let mut list: Vec<Transition> = doc
        .transitions
        .iter()
        .filter(|t| t.out.is_none_or(|o| ids.contains(&o)) && t.inn.is_none_or(|i| ids.contains(&i)))
        .cloned()
        .collect();
    if list.is_empty() {
        return Ok(None);
    }
    // Dans l'ordre du film : le fichier se lit, et deux enregistrements se ressemblent.
    list.sort_by_key(|t| (order(t), t.id));
    let sidecar = Sidecar { format: FORMAT.into(), version: VERSION, transitions: list };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les transitions lues dans le texte de `transitions.json`. Numéros uniques et non nuls (un double
/// en reçoit un neuf), durées bornées, une seule transition par coupe, aucune sans plan.
pub(crate) fn parse(text: &str) -> Result<Vec<Transition>, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de transitions".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.transitions.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    let mut seen = HashSet::new();
    let mut cuts = HashSet::new();
    let mut out = Vec::with_capacity(sidecar.transitions.len());
    for mut t in sidecar.transitions {
        if (t.out.is_none() && t.inn.is_none()) || !cuts.insert((t.out, t.inn)) {
            continue;
        }
        if t.id == 0 || !seen.insert(t.id) {
            t.id = next;
            next += 1;
            seen.insert(t.id);
        }
        t.frames = t.frames.clamp(1, MAX_FRAMES);
        if !t.kind.allowed(t.between_plans()) {
            t.kind = Kind::Black;
        }
        out.push(t);
    }
    Ok(out)
}
