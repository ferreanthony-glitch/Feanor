//! Flips (13h), ceux de TVPaint 12 (textes 011074000–063, flips par rôle 012104150–159) :
//! feuilleter vite des images, comme on feuillette des dessins sur la table, pour juger un
//! mouvement. Un flip a :
//! - des limites autour de l'image de départ : le plan, le calque courant, l'entrée / sortie,
//!   les signets, les marques de couleur, un rôle, ou un nombre d'images à gauche et à droite ;
//! - des checkpoints, les images par lesquelles il passe : toutes, les instances du calque
//!   courant, les signets, les marques de couleur, les images d'un rôle ;
//! - une sorte :
//!   - basique : tant qu'on le tient, il parcourt ses checkpoints à N flips par seconde, en
//!     boucle ou non ;
//!   - directionnel : tant qu'on le tient, chaque déplacement de N pixels de la souris (à
//!     l'horizontale ou à la verticale) passe au checkpoint suivant ou précédent ;
//!   - manuel : deux touches vont au checkpoint précédent ou suivant.
//! - « Revenir au début » : relâché, le flip rend l'image de départ.
//!
//! C'est notre lecture des réglages que décrivent les textes de TVPaint ; « En dehors des
//! tenons » et « Multiple » (noms seulement) ne sont pas repris. Les flips sont un réglage de
//! l'application, comme chez TVPaint.

use serde::{Deserialize, Serialize};

use crate::document::{Layer, Panel};
use crate::roles::Role;

/// La sorte d'un flip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlipKind {
    #[serde(rename = "basique")]
    Basic,
    #[serde(rename = "directionnel")]
    Directional,
    #[serde(rename = "manuel")]
    Manual,
}

impl FlipKind {
    pub const ALL: [FlipKind; 3] = [FlipKind::Basic, FlipKind::Directional, FlipKind::Manual];

    pub fn label(self) -> &'static str {
        match self {
            FlipKind::Basic => "Basique",
            FlipKind::Directional => "Directionnel",
            FlipKind::Manual => "Manuel",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            FlipKind::Basic => "Tant qu'on le tient, il parcourt ses images à la vitesse choisie.",
            FlipKind::Directional => "Tant qu'on le tient, déplacer la souris passe d'une image à l'autre.",
            FlipKind::Manual => "Deux touches (ou deux boutons) vont à l'image précédente ou suivante.",
        }
    }
}

/// Les limites d'un flip, autour de l'image de départ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sorte")]
pub enum Limits {
    #[serde(rename = "plan")]
    Plan,
    /// De la première à la dernière image dessinée du calque courant.
    #[serde(rename = "calque")]
    Layer,
    #[serde(rename = "entree_sortie")]
    InOut,
    /// Entre le signet d'avant (ou sur) et celui d'après.
    #[serde(rename = "signets")]
    Bookmarks,
    /// Entre les dessins marqués (d'une couleur, ou de toutes) d'avant et d'après.
    #[serde(rename = "marques")]
    Marks { couleur: Option<u8> },
    /// Entre les images de ce rôle d'avant et d'après, sur le calque courant.
    #[serde(rename = "role")]
    Role { role: Role },
    /// Tant d'images à gauche et à droite.
    #[serde(rename = "personnalise")]
    Custom { gauche: usize, droite: usize },
}

/// Les images par lesquelles passe un flip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sorte")]
pub enum Checkpoints {
    #[serde(rename = "images")]
    Images,
    /// Le début de chaque dessin du calque courant.
    #[serde(rename = "instances")]
    Instances,
    #[serde(rename = "signets")]
    Bookmarks,
    #[serde(rename = "marques")]
    Marks { couleur: Option<u8> },
    #[serde(rename = "role")]
    Role { role: Role },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flip {
    pub name: String,
    pub kind: FlipKind,
    pub limits: Limits,
    pub checkpoints: Checkpoints,
    /// Flips (images parcourues) par seconde, flip basique.
    pub rate: f32,
    /// Boucler entre les limites.
    pub looping: bool,
    /// Revenir à l'image de départ une fois relâché.
    pub back_to_start: bool,
    /// Flip directionnel : à la verticale (sinon à l'horizontale), à rebours (sinon : vers la
    /// droite ou vers le bas = en avant), pixels de souris par image.
    pub vertical: bool,
    pub backward: bool,
    pub distance: f32,
}

impl Default for Flip {
    /// Le flip de TVPaint d'Anthony : manuel, limité au calque courant, de dessin en dessin,
    /// 30 par seconde, retour à l'image de départ.
    fn default() -> Self {
        Self {
            name: "Flip manuel".into(),
            kind: FlipKind::Manual,
            limits: Limits::Layer,
            checkpoints: Checkpoints::Instances,
            rate: 30.0,
            looping: false,
            back_to_start: true,
            vertical: false,
            backward: false,
            distance: 30.0,
        }
    }
}

/// Ce que regarde un flip : le plan, le calque courant, la longueur du plan.
pub struct FlipScope<'a> {
    pub panel: &'a Panel,
    pub layer: &'a Layer,
    pub len: usize,
}

impl FlipScope<'_> {
    /// L'image `f` porte-t-elle un dessin marqué (de la couleur `k`, ou de toutes) au début de
    /// son exposition ?
    fn marked(&self, f: usize, k: Option<u8>) -> bool {
        let Some(id) = self.layer.cel_at(f) else { return false };
        let start = f == 0 || self.layer.cel_at(f - 1) != Some(id);
        start && self.layer.color_marks.get(&id).is_some_and(|m| k.is_none_or(|k| *m == k))
    }

    fn role_at(&self, f: usize, r: Role) -> bool {
        self.layer.roles.get(&f) == Some(&r)
    }

    /// Les images [a, b) entre lesquelles le flip parti de `start` se déplace.
    pub fn limits(&self, limits: Limits, start: usize) -> (usize, usize) {
        let len = self.len.max(1);
        let start = start.min(len - 1);
        // Entre la dernière image qui vérifie `stop` jusqu'à `start` compris et la première
        // après (comprise) ; le plan entier de chaque côté sinon.
        let around = |stop: &dyn Fn(usize) -> bool| {
            let a = (0..=start).rev().find(|&f| stop(f)).unwrap_or(0);
            let b = (start + 1..len).find(|&f| stop(f)).map_or(len, |f| f + 1);
            (a, b)
        };
        match limits {
            Limits::Plan => (0, len),
            Limits::Layer => {
                let first = self.layer.frames.iter().position(Option::is_some);
                let last = self.layer.frames.iter().rposition(Option::is_some);
                match (first, last) {
                    (Some(a), Some(b)) => (a.min(len - 1), (b + 1).min(len)),
                    _ => (0, len),
                }
            }
            Limits::InOut => crate::cues::play_range(&self.panel.cues, len),
            Limits::Bookmarks => around(&|f| self.panel.cues.bookmarks.contains(&f)),
            Limits::Marks { couleur } => around(&|f| self.marked(f, couleur)),
            Limits::Role { role } => around(&|f| self.role_at(f, role)),
            Limits::Custom { gauche, droite } => (start.saturating_sub(gauche), (start + droite + 1).min(len)),
        }
    }

    /// Les checkpoints d'un flip parti de `start`, dans l'ordre.
    pub fn points(&self, flip: &Flip, start: usize) -> Vec<usize> {
        let (a, b) = self.limits(flip.limits, start);
        (a..b)
            .filter(|&f| match flip.checkpoints {
                Checkpoints::Images => true,
                Checkpoints::Instances => self.layer.cel_at(f).is_some() && (f == 0 || self.layer.cel_at(f - 1) != self.layer.cel_at(f)),
                Checkpoints::Bookmarks => self.panel.cues.bookmarks.contains(&f),
                Checkpoints::Marks { couleur } => self.marked(f, couleur),
                Checkpoints::Role { role } => self.role_at(f, role),
            })
            .collect()
    }
}

/// Le checkpoint où est l'image `frame` : le dernier jusqu'à elle, sinon le premier.
pub fn index_of(points: &[usize], frame: usize) -> usize {
    points.iter().rposition(|&p| p <= frame).unwrap_or(0)
}

/// Un pas de `dir` (±1) depuis le checkpoint `i` ; en boucle, on repart de l'autre bout,
/// sinon on reste au bout.
pub fn step(points: &[usize], i: usize, dir: i32, looping: bool) -> usize {
    let n = points.len();
    if n == 0 {
        return 0;
    }
    let j = i as i64 + dir as i64;
    if looping {
        j.rem_euclid(n as i64) as usize
    } else {
        j.clamp(0, n as i64 - 1) as usize
    }
}

/// Flip manuel : le checkpoint d'après l'image `frame` (`dir` > 0) ou d'avant ; au bout, on
/// repart de l'autre bout en boucle, sinon on reste au dernier. Rien s'il n'y a pas de
/// checkpoint.
pub fn next_point(points: &[usize], frame: usize, dir: i32, looping: bool) -> Option<usize> {
    let (first, last) = (*points.first()?, *points.last()?);
    Some(if dir > 0 {
        points.iter().copied().find(|&p| p > frame).unwrap_or(if looping { first } else { last })
    } else {
        points.iter().rev().copied().find(|&p| p < frame).unwrap_or(if looping { last } else { first })
    })
}

/// Le checkpoint d'un flip basique `elapsed` secondes après son départ au checkpoint `i0`.
pub fn played(points: &[usize], i0: usize, elapsed: f64, rate: f32, looping: bool) -> usize {
    let n = points.len();
    if n == 0 {
        return 0;
    }
    let k = (elapsed * rate.max(0.1) as f64).floor() as usize;
    if looping {
        (i0 + k) % n
    } else {
        (i0 + k).min(n - 1)
    }
}

/// Les flips de Pelure (réglage de l'application).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlipSettings {
    pub flips: Vec<Flip>,
    pub current: usize,
}

impl Default for FlipSettings {
    fn default() -> Self {
        Self { flips: vec![Flip::default()], current: 0 }
    }
}

impl FlipSettings {
    /// Le flip courant (il y en a toujours un).
    pub fn current(&self) -> Flip {
        self.flips.get(self.current).cloned().unwrap_or_default()
    }
}
