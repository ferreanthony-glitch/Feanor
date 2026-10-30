//! Dégradé (outil « Dégradé », Anthony, 2026-10-24 : « un outil dégradé, vecto et bitmap »).
//!
//! Ce module est le noyau, sans interface : le dégradé lui-même (`Degrade` : deux points, des pastilles de couleur, des
//! pastilles d'opacité, une forme), ses réglages (`Reglages`), les dégradés enregistrés (`Enregistre`) et son écriture dans
//! des pixels (`peindre`).
//!
//! * Le point `depart` est le début de l'axe (la couleur de la première pastille), `arrivee` en est la fin (la dernière
//!   pastille). Au-delà des deux points, la couleur reste celle du bout. Linéaire : la couleur ne dépend que de la position le
//!   long de l'axe. Radial : `depart` est le centre, `arrivee` un point du cercle d'arrivée. Angle : en éventail autour de
//!   `depart`, l'axe est la direction de départ. Réfléchi : le miroir de part et d'autre de `depart`. Losange : comme le radial,
//!   avec des diagonales.
//! * Les couleurs sont portées par des **pastilles** (position de 0 à 1, couleur, et un « milieu » : où l'on est à 50/50 entre
//!   cette pastille et la suivante), l'opacité par une **seconde série** de pastilles, indépendante. Au moins deux de chaque.
//! * Le **tramage** est un bruit ordonné déterministe (fonction du seul pixel) de moins d'un niveau, qui casse les marches
//!   visibles d'un dégradé en 8 bits ; absent (`tramage` faux), le rendu est exactement celui d'avant.
//! * Sur un calque vectoriel, le dégradé n'est jamais écrit en pixels dans les données : `Element::Fill` le garde
//!   (`gradient`), il se déplace avec la forme (`Degrade::transformed`) et se rend avec elle (`vector.rs`).
//! * Sur un calque bitmap, `peindre` écrit les pixels, par tuile, sans le moteur de pinceaux : un dégradé à 1920 × 1080 ne
//!   coûte que quelques millisecondes réparties sur les cœurs.
//! * Format (projet 44) : `Degrade` s'écrit avec `pastilles` et `transparence` ; un dégradé d'avant (deux couleurs, deux opacités :
//!   `couleur_depart`, `couleur_arrivee`, `opacite_depart`, `opacite_arrivee`) se lit comme deux pastilles de chaque, sans tramage.

use eframe::egui::{Color32, Pos2};
use serde::{Deserialize, Serialize};

use crate::pixsel::PixelMask;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};
use crate::stencil::Stencil;
use crate::vector::Affine;

/// Le plus de pastilles de chaque sorte (couleurs, opacités).
pub const MAX_PASTILLES: usize = 16;
/// Le milieu d'un segment reste entre ces bornes (une pente trop raide n'est pas un dégradé).
pub const MILIEU_MIN: f32 = 0.05;

/// La forme du dégradé.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Genre {
    #[default]
    #[serde(rename = "lineaire")]
    Lineaire,
    #[serde(rename = "radial")]
    Radial,
    /// En éventail autour du point d'appui, l'axe en donne la direction de départ.
    #[serde(rename = "angle")]
    Angle,
    /// Le miroir de part et d'autre du point d'appui.
    #[serde(rename = "reflechi")]
    Reflechi,
    /// Des losanges autour du point d'appui, l'arrivée en est une pointe.
    #[serde(rename = "losange")]
    Losange,
}

impl Genre {
    pub const TOUS: [Genre; 5] = [Genre::Lineaire, Genre::Radial, Genre::Angle, Genre::Reflechi, Genre::Losange];
}

fn demi() -> f32 {
    0.5
}

fn est_demi(m: &f32) -> bool {
    (*m - 0.5).abs() < 1e-6
}

/// Une pastille qui a une position et un milieu (couleur ou opacité).
pub trait Pastille: Copy {
    fn pos(&self) -> f32;
    fn set_pos(&mut self, p: f32);
    fn milieu(&self) -> f32;
    fn set_milieu(&mut self, m: f32);
}

/// Une pastille de couleur : où (0 à 1 le long de l'axe), quelle couleur, et le milieu du segment qui la suit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PastilleCouleur {
    #[serde(rename = "position")]
    pub pos: f32,
    #[serde(rename = "couleur")]
    pub rgb: [u8; 3],
    /// Où l'on est à 50/50 entre cette pastille et la suivante (0,5 : au milieu).
    #[serde(default = "demi", skip_serializing_if = "est_demi")]
    pub milieu: f32,
}

/// Une pastille d'opacité (0 : transparent, 1 : opaque).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PastilleOpacite {
    #[serde(rename = "position")]
    pub pos: f32,
    pub opacite: f32,
    #[serde(default = "demi", skip_serializing_if = "est_demi")]
    pub milieu: f32,
}

macro_rules! pastille_impl {
    ($t:ty) => {
        impl Pastille for $t {
            fn pos(&self) -> f32 {
                self.pos
            }
            fn set_pos(&mut self, p: f32) {
                self.pos = p;
            }
            fn milieu(&self) -> f32 {
                self.milieu
            }
            fn set_milieu(&mut self, m: f32) {
                self.milieu = m;
            }
        }
    };
}
pastille_impl!(PastilleCouleur);
pastille_impl!(PastilleOpacite);

impl PastilleCouleur {
    pub fn new(pos: f32, rgb: [u8; 3]) -> Self {
        Self { pos, rgb, milieu: 0.5 }
    }
}

impl PastilleOpacite {
    pub fn new(pos: f32, opacite: f32) -> Self {
        Self { pos, opacite, milieu: 0.5 }
    }
}

/// La courbe d'un segment : `u` (0 à 1 dans le segment) devient 0,5 quand `u` vaut `m`. Une courbe rationnelle, qui vaut 0 en 0, 1 en 1,
/// et dont le retournement est la même courbe avec `1 - m` (inverser un dégradé n'en change pas la forme).
fn adoucir(u: f32, m: f32) -> f32 {
    let m = m.clamp(MILIEU_MIN, 1.0 - MILIEU_MIN);
    if (m - 0.5).abs() < 1e-6 {
        return u;
    }
    let u = u.clamp(0.0, 1.0);
    u * (1.0 - m) / (u * (1.0 - 2.0 * m) + m)
}

/// Le segment de `s` où tombe `t` (l'indice de sa première pastille) et la fraction adoucie dans ce segment. Avant la première
/// pastille : (0, 0) ; après la dernière : (avant-dernière, 1). Il faut deux pastilles au moins.
fn repere<T: Pastille>(s: &[T], t: f32) -> (usize, f32) {
    let n = s.len();
    if t <= s[0].pos() {
        return (0, 0.0);
    }
    if t >= s[n - 1].pos() {
        return (n - 2, 1.0);
    }
    let mut i = 0;
    while i + 2 < n && t >= s[i + 1].pos() {
        i += 1;
    }
    let (a, b) = (s[i].pos(), s[i + 1].pos());
    let u = if b - a <= 1e-9 { 1.0 } else { ((t - a) / (b - a)).clamp(0.0, 1.0) };
    (i, adoucir(u, s[i].milieu()))
}

/// La couleur (0 à 255, non prémultipliée) des pastilles `s` à la position `t`.
pub fn couleur_en(s: &[PastilleCouleur], t: f32) -> [f32; 3] {
    match s.len() {
        0 => [0.0; 3],
        1 => s[0].rgb.map(|v| v as f32),
        _ => {
            let (i, u) = repere(s, t);
            let (a, b) = (s[i].rgb, s[i + 1].rgb);
            [0, 1, 2].map(|k| a[k] as f32 + (b[k] as f32 - a[k] as f32) * u)
        }
    }
}

/// L'opacité (0 à 1) des pastilles `s` à la position `t`.
pub fn opacite_en(s: &[PastilleOpacite], t: f32) -> f32 {
    match s.len() {
        0 => 1.0,
        1 => s[0].opacite,
        _ => {
            let (i, u) = repere(s, t);
            s[i].opacite + (s[i + 1].opacite - s[i].opacite) * u
        }
    }
}

/// `s` retourné : la première pastille devient la dernière (positions et milieux compris).
fn inverser<T: Pastille>(s: &[T]) -> Vec<T> {
    let n = s.len();
    let mut out: Vec<T> = s.iter().rev().copied().collect();
    for p in &mut out {
        p.set_pos(1.0 - p.pos());
    }
    for j in 0..n {
        let m = if j + 1 < n { 1.0 - s[n - 2 - j].milieu() } else { 0.5 };
        out[j].set_milieu(m);
    }
    out
}

/// Insère `p` à sa place (par position) dans `v` ; son indice.
pub fn inserer<T: Pastille>(v: &mut Vec<T>, p: T) -> usize {
    let i = v.iter().position(|q| q.pos() > p.pos()).unwrap_or(v.len());
    v.insert(i, p);
    i
}

/// Les positions de `s` finies, de 0 à 1, rangées ; les milieux entre les bornes ; au moins deux pastilles et pas plus de `MAX_PASTILLES`.
fn pastilles_saines<T: Pastille>(s: &[T]) -> bool {
    (2..=MAX_PASTILLES).contains(&s.len())
        && s.iter().all(|p| p.pos().is_finite() && (0.0..=1.0).contains(&p.pos()) && p.milieu().is_finite() && (0.01..=0.99).contains(&p.milieu()))
        && s.windows(2).all(|w| w[0].pos() <= w[1].pos())
}

/// Un dégradé : sa forme, ses deux points (en pixels du dessin), ses pastilles de couleur et d'opacité, son tramage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "DegradeFichier", into = "DegradeFichier")]
pub struct Degrade {
    pub genre: Genre,
    pub depart: [f32; 2],
    pub arrivee: [f32; 2],
    pub couleurs: Vec<PastilleCouleur>,
    pub opacites: Vec<PastilleOpacite>,
    pub tramage: bool,
}

/// Ce qui s'écrit : les champs d'avant (deux couleurs, deux opacités) sont encore lus, jamais écrits.
#[derive(Serialize, Deserialize)]
struct DegradeFichier {
    genre: Genre,
    depart: [f32; 2],
    arrivee: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couleur_depart: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couleur_arrivee: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opacite_depart: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opacite_arrivee: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pastilles: Vec<PastilleCouleur>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    transparence: Vec<PastilleOpacite>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    tramage: bool,
}

impl From<DegradeFichier> for Degrade {
    fn from(f: DegradeFichier) -> Degrade {
        let couleurs = if !f.pastilles.is_empty() {
            f.pastilles
        } else {
            match (f.couleur_depart, f.couleur_arrivee) {
                (Some(a), Some(b)) => vec![PastilleCouleur::new(0.0, a), PastilleCouleur::new(1.0, b)],
                _ => Vec::new(),
            }
        };
        let opacites = if !f.transparence.is_empty() {
            f.transparence
        } else {
            vec![PastilleOpacite::new(0.0, f.opacite_depart.unwrap_or(1.0)), PastilleOpacite::new(1.0, f.opacite_arrivee.unwrap_or(1.0))]
        };
        Degrade { genre: f.genre, depart: f.depart, arrivee: f.arrivee, couleurs, opacites, tramage: f.tramage }
    }
}

impl From<Degrade> for DegradeFichier {
    fn from(g: Degrade) -> DegradeFichier {
        DegradeFichier {
            genre: g.genre,
            depart: g.depart,
            arrivee: g.arrivee,
            couleur_depart: None,
            couleur_arrivee: None,
            opacite_depart: None,
            opacite_arrivee: None,
            pastilles: g.couleurs,
            transparence: g.opacites,
            tramage: g.tramage,
        }
    }
}

/// Le seuil de tramage au pixel entier (`x`, `y`), de −0,5 (compris) à 0,5 (exclu) : un bruit de gradient entrelacé, qui ne dépend que
/// du pixel (même résultat à chaque rendu, aucun scintillement à la lecture).
pub fn seuil(x: i64, y: i64) -> f32 {
    let f = 0.067_110_56 * x as f32 + 0.005_837_15 * y as f32;
    let g = (f - f.floor()) * 52.982_918;
    (g - g.floor()) - 0.5
}

impl Degrade {
    /// Un dégradé de deux couleurs et deux opacités (sans tramage).
    pub fn simple(genre: Genre, depart: [f32; 2], arrivee: [f32; 2], c0: [u8; 3], c1: [u8; 3], a0: f32, a1: f32) -> Degrade {
        Degrade {
            genre,
            depart,
            arrivee,
            couleurs: vec![PastilleCouleur::new(0.0, c0), PastilleCouleur::new(1.0, c1)],
            opacites: vec![PastilleOpacite::new(0.0, a0), PastilleOpacite::new(1.0, a1)],
            tramage: false,
        }
    }

    /// La couleur de la première pastille.
    pub fn c0(&self) -> [u8; 3] {
        self.couleurs.first().map_or([0; 3], |p| p.rgb)
    }

    /// La couleur de la dernière pastille.
    pub fn c1(&self) -> [u8; 3] {
        self.couleurs.last().map_or([0; 3], |p| p.rgb)
    }

    /// L'opacité de la première pastille.
    pub fn a0(&self) -> f32 {
        self.opacites.first().map_or(1.0, |p| p.opacite)
    }

    /// L'opacité de la dernière pastille.
    pub fn a1(&self) -> f32 {
        self.opacites.last().map_or(1.0, |p| p.opacite)
    }

    /// Tout est fini et les opacités sont des opacités, les pastilles sont rangées : un dégradé lu d'un fichier abîmé ne passe pas
    /// le contrôle.
    pub fn is_sane(&self) -> bool {
        self.depart.iter().chain(self.arrivee.iter()).all(|v| v.is_finite())
            && pastilles_saines(&self.couleurs)
            && pastilles_saines(&self.opacites)
            && self.opacites.iter().all(|p| (0.0..=1.0).contains(&p.opacite))
    }

    /// Où en est le point (`x`, `y`) du dégradé : 0 au départ, 1 à l'arrivée, borné.
    pub fn parametre(&self, x: f32, y: f32) -> f32 {
        let (dx, dy) = (self.arrivee[0] - self.depart[0], self.arrivee[1] - self.depart[1]);
        let (px, py) = (x - self.depart[0], y - self.depart[1]);
        let l2 = dx * dx + dy * dy;
        let t = match self.genre {
            Genre::Lineaire => {
                if l2 <= 1e-12 {
                    0.0
                } else {
                    (px * dx + py * dy) / l2
                }
            }
            Genre::Radial => {
                let r = l2.sqrt();
                if r <= 1e-6 {
                    0.0
                } else {
                    (px * px + py * py).sqrt() / r
                }
            }
            Genre::Angle => {
                if l2 <= 1e-12 || px * px + py * py <= 1e-12 {
                    0.0
                } else {
                    (py.atan2(px) - dy.atan2(dx)).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU
                }
            }
            Genre::Reflechi => {
                if l2 <= 1e-12 {
                    0.0
                } else {
                    ((px * dx + py * dy) / l2).abs()
                }
            }
            Genre::Losange => {
                let len = l2.sqrt();
                if len <= 1e-6 {
                    0.0
                } else {
                    let (u, v) = ((px * dx + py * dy) / len, (-px * dy + py * dx) / len);
                    (u.abs() + v.abs()) / len
                }
            }
        };
        if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// La couleur (non prémultipliée, 0 à 255) et l'opacité (0 à 1) à la position `t` (0 à 1) du dégradé.
    pub fn couleur_a(&self, t: f32) -> ([f32; 3], f32) {
        (couleur_en(&self.couleurs, t), opacite_en(&self.opacites, t).clamp(0.0, 1.0))
    }

    /// La couleur (non prémultipliée, 0 à 255) et l'opacité (0 à 1) au point (`x`, `y`).
    pub fn couleur(&self, x: f32, y: f32) -> ([f32; 3], f32) {
        self.couleur_a(self.parametre(x, y))
    }

    /// Le pixel prémultiplié au point (`x`, `y`) (le centre du pixel : `x + 0,5`), l'opacité multipliée par `k`. Sans tramage.
    pub fn pixel(&self, x: f32, y: f32, k: f32) -> Color32 {
        let (rgb, a) = self.couleur(x, y);
        premultiplie(rgb, (a * k).clamp(0.0, 1.0))
    }

    /// Le pixel prémultiplié du pixel entier (`x`, `y`) : le centre du pixel, avec le tramage si le dégradé en a un.
    pub fn pixel_en(&self, x: usize, y: usize, k: f32) -> Color32 {
        let (rgb, a) = self.couleur(x as f32 + 0.5, y as f32 + 0.5);
        let d = if self.tramage { seuil(x as i64, y as i64) } else { 0.0 };
        premultiplie_tr(rgb, (a * k).clamp(0.0, 1.0), d)
    }

    /// La couleur (0 à 255, arrondie, tramée) du pixel entier (`x`, `y`) et l'opacité (0 à 1) : pour le rendu vectoriel.
    pub fn couleur_pixel(&self, x: i64, y: i64) -> ([u8; 3], f32) {
        let (rgb, a) = self.couleur(x as f32 + 0.5, y as f32 + 0.5);
        let d = if self.tramage { seuil(x, y) } else { 0.0 };
        (rgb.map(|v| (v + d).round().clamp(0.0, 255.0) as u8), a)
    }

    /// Le dégradé à mi-chemin de `a` (`t` = 0) et `b` (`t` = 1), de même genre : axe, couleurs et opacités se mélangent
    /// (l'interpolation d'un plan à l'autre, `tween.rs`). Quand le nombre de pastilles diffère, les deux courbes sont mélangées sur
    /// l'ensemble de leurs positions.
    pub fn entre(a: &Degrade, b: &Degrade, t: f32) -> Degrade {
        let l = |x: f32, y: f32| x + (y - x) * t;
        let mix = |x: [u8; 3], y: [u8; 3]| [0, 1, 2].map(|k| l(x[k] as f32, y[k] as f32).round().clamp(0.0, 255.0) as u8);
        let couleurs = if a.couleurs.len() == b.couleurs.len() {
            a.couleurs
                .iter()
                .zip(&b.couleurs)
                .map(|(x, y)| PastilleCouleur { pos: l(x.pos, y.pos).clamp(0.0, 1.0), rgb: mix(x.rgb, y.rgb), milieu: l(x.milieu, y.milieu).clamp(0.01, 0.99) })
                .collect()
        } else {
            melanger_courbes_couleur(&a.couleurs, &b.couleurs, t)
        };
        let opacites = if a.opacites.len() == b.opacites.len() {
            a.opacites
                .iter()
                .zip(&b.opacites)
                .map(|(x, y)| PastilleOpacite { pos: l(x.pos, y.pos).clamp(0.0, 1.0), opacite: l(x.opacite, y.opacite).clamp(0.0, 1.0), milieu: l(x.milieu, y.milieu).clamp(0.01, 0.99) })
                .collect()
        } else {
            melanger_courbes_opacite(&a.opacites, &b.opacites, t)
        };
        Degrade {
            genre: a.genre,
            depart: [l(a.depart[0], b.depart[0]), l(a.depart[1], b.depart[1])],
            arrivee: [l(a.arrivee[0], b.arrivee[0]), l(a.arrivee[1], b.arrivee[1])],
            couleurs,
            opacites,
            tramage: a.tramage || b.tramage,
        }
    }

    /// Les mêmes bouts échangés : le départ prend la couleur d'arrivée et inversement (les pastilles et leurs milieux, les
    /// opacités aussi).
    pub fn reversed(&self) -> Degrade {
        Degrade { couleurs: inverser(&self.couleurs), opacites: inverser(&self.opacites), ..self.clone() }
    }

    /// Le dégradé dont les deux points passent par `f` (la forme se déplace, tourne, se déforme : son axe la suit).
    pub fn mapped(&self, f: impl Fn(Pos2) -> Pos2) -> Degrade {
        let (a, b) = (f(Pos2::new(self.depart[0], self.depart[1])), f(Pos2::new(self.arrivee[0], self.arrivee[1])));
        Degrade { depart: [a.x, a.y], arrivee: [b.x, b.y], ..self.clone() }
    }

    /// Le dégradé transformé par `a` : son axe suit la forme (le rayon d'un dégradé radial suit l'échelle).
    pub fn transformed(&self, a: &Affine) -> Degrade {
        self.mapped(|p| a.apply(p))
    }

    /// Le rectangle qui contient les deux points.
    pub fn axe(&self) -> (Pos2, Pos2) {
        (Pos2::new(self.depart[0], self.depart[1]), Pos2::new(self.arrivee[0], self.arrivee[1]))
    }

    /// L'angle de l'axe en degrés (0 : vers la droite, dans le sens des aiguilles d'une montre à l'écran), de −180 (exclu) à 180.
    pub fn angle_degres(&self) -> f32 {
        let (dx, dy) = (self.arrivee[0] - self.depart[0], self.arrivee[1] - self.depart[1]);
        let a = dy.atan2(dx).to_degrees();
        if a <= -180.0 {
            180.0
        } else {
            a
        }
    }

    /// La longueur de l'axe.
    pub fn longueur(&self) -> f32 {
        ((self.arrivee[0] - self.depart[0]).powi(2) + (self.arrivee[1] - self.depart[1]).powi(2)).sqrt()
    }
}

/// Deux courbes de couleur mélangées sur l'ensemble de leurs positions.
fn melanger_courbes_couleur(a: &[PastilleCouleur], b: &[PastilleCouleur], t: f32) -> Vec<PastilleCouleur> {
    let mut pos: Vec<f32> = a.iter().chain(b.iter()).map(|p| p.pos).collect();
    pos.sort_by(|x, y| x.total_cmp(y));
    pos.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    pos.truncate(MAX_PASTILLES);
    pos.into_iter()
        .map(|p| {
            let (x, y) = (couleur_en(a, p), couleur_en(b, p));
            PastilleCouleur::new(p, [0, 1, 2].map(|k| (x[k] + (y[k] - x[k]) * t).round().clamp(0.0, 255.0) as u8))
        })
        .collect()
}

/// Deux courbes d'opacité mélangées sur l'ensemble de leurs positions.
fn melanger_courbes_opacite(a: &[PastilleOpacite], b: &[PastilleOpacite], t: f32) -> Vec<PastilleOpacite> {
    let mut pos: Vec<f32> = a.iter().chain(b.iter()).map(|p| p.pos).collect();
    pos.sort_by(|x, y| x.total_cmp(y));
    pos.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    pos.truncate(MAX_PASTILLES);
    pos.into_iter()
        .map(|p| {
            let (x, y) = (opacite_en(a, p), opacite_en(b, p));
            PastilleOpacite::new(p, (x + (y - x) * t).clamp(0.0, 1.0))
        })
        .collect()
}

/// Couleur `rgb` (0 à 255, non prémultipliée) d'opacité `a`, en pixel prémultiplié.
pub fn premultiplie(rgb: [f32; 3], a: f32) -> Color32 {
    premultiplie_tr(rgb, a, 0.0)
}

/// Comme `premultiplie`, avec le seuil de tramage `d` (de −0,5 à 0,5) ajouté avant l'arrondi, aux couleurs et à l'opacité : jamais plus d'un
/// niveau d'écart, et une couleur jamais plus claire que l'opacité (le pixel reste un pixel prémultiplié valide).
pub fn premultiplie_tr(rgb: [f32; 3], a: f32, d: f32) -> Color32 {
    let c = |v: f32| (v * a + 0.5 + d).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(c(rgb[0]), c(rgb[1]), c(rgb[2]), (a * 255.0 + 0.5 + d).clamp(0.0, 255.0) as u8)
}

/// Comment le dégradé finit (le raccourci simple : l'ancien réglage « Fin »).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fin {
    /// Vers la couleur de fond (celle d'arrière-plan, le second carré du panneau Couleur).
    #[default]
    VersFond,
    /// Vers le transparent : la couleur d'avant-plan, de plus en plus transparente.
    VersTransparent,
}

/// Les réglages de l'outil (Réglages, outil Dégradé).
///
/// Tant qu'on n'a pas touché aux pastilles, les couleurs sont **liées** aux carrés du panneau Couleur (`couleurs` vaut `None` :
/// avant-plan, puis arrière-plan ou avant-plan devenu transparent selon `fin`) et l'opacité aussi (`transparence` vaut `None`) :
/// l'usage simple ne change pas.
#[derive(Clone, Debug, PartialEq)]
pub struct Reglages {
    pub genre: Genre,
    pub fin: Fin,
    /// Opacité du dégradé, 0 à 1 (multiplie celle des pastilles d'opacité).
    pub opacite: f32,
    /// Échanger les deux bouts.
    pub inverse: bool,
    /// Remplir tout le calque (Alt tenu : la même chose, le temps du geste).
    pub tout_le_calque: bool,
    /// Le tramage (coché d'origine) : de quoi éviter les marches visibles en 8 bits.
    pub tramage: bool,
    /// Les pastilles de couleur à soi ; `None` : les carrés du panneau Couleur.
    pub couleurs: Option<Vec<PastilleCouleur>>,
    /// Les pastilles d'opacité à soi ; `None` : celles que `fin` donne (opaque, ou jusqu'au transparent).
    pub transparence: Option<Vec<PastilleOpacite>>,
}

impl Default for Reglages {
    fn default() -> Self {
        Self { genre: Genre::Lineaire, fin: Fin::VersFond, opacite: 1.0, inverse: false, tout_le_calque: false, tramage: true, couleurs: None, transparence: None }
    }
}

impl Reglages {
    /// Les pastilles de couleur telles qu'elles sont maintenant : les siennes, ou celles des carrés `avant` et `arriere`.
    pub fn couleurs_effectives(&self, avant: [u8; 3], arriere: [u8; 3]) -> Vec<PastilleCouleur> {
        match &self.couleurs {
            Some(c) => c.clone(),
            None => vec![PastilleCouleur::new(0.0, avant), PastilleCouleur::new(1.0, if self.fin == Fin::VersFond { arriere } else { avant })],
        }
    }

    /// Les pastilles d'opacité telles qu'elles sont maintenant (sans l'opacité du réglage).
    pub fn transparence_effective(&self) -> Vec<PastilleOpacite> {
        match &self.transparence {
            Some(t) => t.clone(),
            None => vec![PastilleOpacite::new(0.0, 1.0), PastilleOpacite::new(1.0, if self.fin == Fin::VersFond { 1.0 } else { 0.0 })],
        }
    }

    /// Le dégradé du geste qui va de `depart` à `arrivee`, avec les couleurs `avant` (avant-plan) et `arriere`
    /// (arrière-plan).
    pub fn degrade(&self, avant: [u8; 3], arriere: [u8; 3], depart: Pos2, arrivee: Pos2) -> Degrade {
        let o = self.opacite.clamp(0.0, 1.0);
        let mut opacites = self.transparence_effective();
        for p in &mut opacites {
            p.opacite = (p.opacite * o).clamp(0.0, 1.0);
        }
        let g = Degrade {
            genre: self.genre,
            depart: [depart.x, depart.y],
            arrivee: [arrivee.x, arrivee.y],
            couleurs: self.couleurs_effectives(avant, arriere),
            opacites,
            tramage: self.tramage,
        };
        if self.inverse {
            g.reversed()
        } else {
            g
        }
    }

    /// Les réglages qui donnent le dégradé `g` (retrouver, avec l'outil Dégradé, celui d'une zone) : ses pastilles sont les siennes.
    pub fn depuis(g: &Degrade) -> Reglages {
        Reglages {
            genre: g.genre,
            fin: Fin::VersFond,
            opacite: 1.0,
            inverse: false,
            tout_le_calque: false,
            tramage: g.tramage,
            couleurs: Some(g.couleurs.clone()),
            transparence: Some(g.opacites.clone()),
        }
    }

    /// Reprend les couleurs, l'opacité et la fin d'un dégradé enregistré.
    pub fn appliquer(&mut self, e: &Enregistre) {
        self.couleurs = e.couleurs.clone();
        self.transparence = e.transparence.clone();
        self.fin = if e.vers_transparent { Fin::VersTransparent } else { Fin::VersFond };
        self.inverse = false;
    }

    /// Le dégradé courant, à enregistrer sous le nom `nom` (les couleurs liées le restent : « avant-plan vers transparent »).
    pub fn enregistrer(&self, nom: &str) -> Enregistre {
        Enregistre { nom: nom.to_string(), couleurs: self.couleurs.clone(), transparence: self.transparence.clone(), vers_transparent: self.fin == Fin::VersTransparent }
    }
}

/// Un dégradé enregistré (les réglages de l'application, pas le projet) : la liste de dégradés à soi.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enregistre {
    pub nom: String,
    /// Les pastilles de couleur ; absentes : liées aux carrés du panneau Couleur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub couleurs: Option<Vec<PastilleCouleur>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparence: Option<Vec<PastilleOpacite>>,
    /// Liées aux carrés : l'avant-plan devient transparent au lieu d'aller vers l'arrière-plan.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vers_transparent: bool,
}

impl Enregistre {
    /// Il est sain : des pastilles rangées, des opacités entre 0 et 1, un nom.
    pub fn est_sain(&self) -> bool {
        self.couleurs.as_deref().is_none_or(pastilles_saines)
            && self.transparence.as_deref().is_none_or(|t| pastilles_saines(t) && t.iter().all(|p| (0.0..=1.0).contains(&p.opacite)))
    }

    /// Le dégradé qu'il donne, sur l'axe unité de gauche à droite, avec les carrés `avant` et `arriere` (pour sa vignette).
    pub fn apercu(&self, avant: [u8; 3], arriere: [u8; 3]) -> Degrade {
        let mut r = Reglages::default();
        r.appliquer(self);
        r.tramage = false;
        r.degrade(avant, arriere, Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0))
    }
}

fn c(rgb: [u8; 3], pos: f32) -> PastilleCouleur {
    PastilleCouleur::new(pos, rgb)
}

/// Les dégradés du départ : simples, de quoi voir comment ça marche (et ils se rangent ou se suppriment comme les autres).
pub fn enregistres_d_origine() -> Vec<Enregistre> {
    vec![
        Enregistre { nom: "Noir vers blanc".into(), couleurs: Some(vec![c([0, 0, 0], 0.0), c([255, 255, 255], 1.0)]), transparence: None, vers_transparent: false },
        Enregistre { nom: "Avant-plan vers arrière-plan".into(), couleurs: None, transparence: None, vers_transparent: false },
        Enregistre { nom: "Avant-plan vers transparent".into(), couleurs: None, transparence: None, vers_transparent: true },
        Enregistre {
            nom: "Ciel".into(),
            couleurs: Some(vec![c([28, 58, 128], 0.0), c([92, 154, 224], 0.55), c([214, 232, 245], 1.0)]),
            transparence: None,
            vers_transparent: false,
        },
        Enregistre {
            nom: "Soleil couchant".into(),
            couleurs: Some(vec![c([52, 40, 96], 0.0), c([200, 84, 92], 0.5), c([250, 176, 80], 1.0)]),
            transparence: None,
            vers_transparent: false,
        },
        Enregistre {
            nom: "Ombre douce".into(),
            couleurs: Some(vec![c([0, 0, 0], 0.0), c([0, 0, 0], 1.0)]),
            transparence: Some(vec![PastilleOpacite::new(0.0, 0.7), PastilleOpacite::new(1.0, 0.0)]),
            vers_transparent: false,
        },
    ]
}

/// Où `peindre` écrit.
pub enum Portee<'a> {
    /// Toute la feuille.
    Tout,
    /// Une zone trouvée comme le fait le pot de peinture (`fill.rs`) : `region` est la zone, `grown` la zone élargie du
    /// débord. Dans la zone, le dégradé remplace ce qui est là (de quoi recolorer un aplat) ; dans le débord, il passe
    /// *sous* l'existant, pour ne pas abîmer le bord anticrénelé du trait.
    Zone { region: &'a [bool], grown: &'a [bool] },
    /// Une sélection (couverture de 0 à 1 par pixel, bords anticrénelés) : le dégradé se pose par-dessus, dans la
    /// mesure de la couverture.
    Masque(&'a PixelMask),
}

/// `top` (prémultiplié) posé sur `under` (prémultiplié) : `under` passe dessous.
fn derriere(top: Color32, under: Color32) -> Color32 {
    let inv = 255 - top.a() as u32;
    let ch = |t: u8, u: u8| (t as u32 + (u as u32 * inv + 127) / 255).min(255) as u8;
    Color32::from_rgba_premultiplied(ch(top.r(), under.r()), ch(top.g(), under.g()), ch(top.b(), under.b()), ch(top.a(), under.a()))
}

/// Le pixel (`x`, `y`) de `dst` après la pose du dégradé avec la couverture `cover` (0 à 1) : par-dessus ; sur un calque
/// qui préserve la transparence (`garde_alpha`), la couleur remplace celle du pixel dans la mesure de la couverture et
/// de l'opacité du dégradé, et le vide reste vide (`remplirsel::filled_pixel`).
fn pose(dst: Color32, g: &Degrade, x: usize, y: usize, cover: f32, garde_alpha: bool) -> Color32 {
    let (rgb, a) = g.couleur(x as f32 + 0.5, y as f32 + 0.5);
    let c = (a * cover).clamp(0.0, 1.0);
    if c <= 0.0 {
        return dst;
    }
    let d = if g.tramage { seuil(x as i64, y as i64) } else { 0.0 };
    if garde_alpha {
        let q = |v: f32| (v + d).round().clamp(0.0, 255.0) as u8;
        return crate::remplirsel::filled_pixel(dst, [q(rgb[0]), q(rgb[1]), q(rgb[2])], c, true);
    }
    crate::render::over(dst, premultiplie_tr(rgb, c, d), 255, None)
}

/// Les pixels de la tuile `idx` après le dégradé ; `None` si rien n'y change.
fn tuile_peinte(raster: &Raster, idx: usize, portee: &Portee, g: &Degrade, garde_alpha: bool, pochoir: Option<&Stencil>) -> Option<Tile> {
    let (ox, oy, tw, th) = raster.tile_bounds(idx);
    let w = raster.width;
    let ancienne = raster.tiles[idx].as_ref();
    let mut px: [Color32; TILE_PIXELS] = match ancienne {
        Some(t) => **t,
        None => [Color32::TRANSPARENT; TILE_PIXELS],
    };
    let mut change = false;
    for y in 0..th {
        for x in 0..tw {
            let (gx, gy) = (ox + x, oy + y);
            let i = y * TILE + x;
            let facteur = pochoir.map_or(1.0, |st| st.at(idx, i));
            if facteur <= 0.0 {
                continue;
            }
            let dst = px[i];
            let out = match portee {
                Portee::Tout => pose(dst, g, gx, gy, facteur, garde_alpha),
                Portee::Masque(m) => {
                    let cover = m.at(gx, gy) * facteur;
                    if cover <= 0.0 {
                        continue;
                    }
                    pose(dst, g, gx, gy, cover, garde_alpha)
                }
                Portee::Zone { region, grown } => {
                    let k = gy * w + gx;
                    if !grown[k] {
                        continue;
                    }
                    let src = g.pixel_en(gx, gy, 1.0);
                    if region[k] {
                        src
                    } else {
                        derriere(dst, src)
                    }
                }
            };
            if out != dst {
                px[i] = out;
                change = true;
            }
        }
    }
    change.then(|| Tile::new(px))
}

/// Écrit le dégradé `g` dans `raster` (la feuille), dans `portee` : rend les tuiles d'avant (pour l'annulation, une seule
/// étape). Les tuiles se calculent en parallèle ; rien n'est écrit dans une tuile que le dégradé ne change pas.
pub fn peindre(raster: &mut Raster, portee: &Portee, g: &Degrade, garde_alpha: bool, pochoir: Option<&Stencil>) -> Vec<(usize, Option<Tile>)> {
    let w = raster.width;
    // Les tuiles que la portée peut toucher.
    let candidates: Vec<usize> = (0..raster.tiles.len())
        .filter(|&idx| {
            if pochoir.is_some_and(|st| st.blocks(idx)) {
                return false;
            }
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            match portee {
                Portee::Tout => true,
                Portee::Masque(m) => {
                    let b = m.bounds();
                    (ox as f32) < b.max.x && ((ox + tw) as f32) > b.min.x && (oy as f32) < b.max.y && ((oy + th) as f32) > b.min.y
                }
                Portee::Zone { grown, .. } => (0..th).any(|y| {
                    let s = (oy + y) * w + ox;
                    s + tw <= grown.len() && grown[s..s + tw].iter().any(|&g| g)
                }),
            }
        })
        .collect();
    let calculees: Vec<Option<Tile>> = if candidates.len() >= 8 {
        crate::parallele::par_map(&candidates, 16, |_, &idx| tuile_peinte(raster, idx, portee, g, garde_alpha, pochoir))
    } else {
        candidates.iter().map(|&idx| tuile_peinte(raster, idx, portee, g, garde_alpha, pochoir)).collect()
    };
    let mut avant = Vec::new();
    for (idx, neuve) in candidates.into_iter().zip(calculees) {
        if let Some(t) = neuve {
            avant.push((idx, raster.tiles[idx].clone()));
            raster.tiles[idx] = Some(t);
        }
    }
    avant
}

/// Écart (par canal, opacité comprise) entre deux pixels voisins en dessous duquel la variation est « douce » : une pente de
/// dégradé ou de peinture, pas un trait. Un dégradé noir → blanc posé sur 26 pixels ou plus passe sous ce seuil ; le bord
/// d'un trait (un saut de plusieurs dizaines de niveaux d'un pixel à l'autre) ne le passe pas.
pub const PAS_DOUX: i32 = 10;

fn proche(a: Color32, b: Color32, tol: i32) -> bool {
    (a.r() as i32 - b.r() as i32).abs() <= tol
        && (a.g() as i32 - b.g() as i32).abs() <= tol
        && (a.b() as i32 - b.b() as i32).abs() <= tol
        && (a.a() as i32 - b.a() as i32).abs() <= tol
}

/// Le pixel `i` est « lisse » : aucun de ses quatre voisins n'en diffère de plus de `PAS_DOUX`. Un pixel du bord d'un trait ne
/// l'est jamais (il a un voisin de l'autre côté du saut), même quand son bord anticrénelé varie doucement le long du trait.
fn lisse(src: &[Color32], w: usize, h: usize, i: usize) -> bool {
    let (x, y, p) = (i % w, i / w, src[i]);
    (x == 0 || proche(src[i - 1], p, PAS_DOUX)) && (x + 1 == w || proche(src[i + 1], p, PAS_DOUX)) && (y == 0 || proche(src[i - w], p, PAS_DOUX)) && (y + 1 == h || proche(src[i + w], p, PAS_DOUX))
}

/// La zone fermée où l'on appuie, pour le dégradé : celle du pot de peinture (les pixels de la couleur du point d'appui, à
/// `tol` près, de proche en proche), **plus** les pixels lisses qu'on atteint de proche en proche (un dégradé, une peinture
/// douce), plus **une couche** de pixels voisins de la même couleur (le bord de la zone, collé au trait). Le trait la borne :
/// un saut de couleur d'un pixel à l'autre. Sur une couleur unie, c'est la zone du pot de peinture, au pixel près. Sur un
/// dégradé, ce n'est plus une bande de couleurs voisines : la zone va jusqu'au trait.
pub fn zone_douce(src: &[Color32], w: usize, h: usize, sx: usize, sy: usize, tol: u8) -> Vec<bool> {
    let mut mask = crate::fill::flood(src, w, h, sx, sy, tol);
    let cible = src[sy * w + sx];
    let tol = tol as i32;
    let voisins4 = |i: usize| {
        let (x, y) = (i % w, i / w);
        [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)]
    };
    // Les pixels de la zone qui touchent un voisin hors de la zone : de là, la zone continue sur les pixels lisses.
    let frontiere = |mask: &[bool]| -> Vec<usize> { (0..w * h).filter(|&i| mask[i] && voisins4(i).into_iter().flatten().any(|j| !mask[j])).collect() };
    let mut pile = frontiere(&mask);
    while let Some(i) = pile.pop() {
        for j in voisins4(i).into_iter().flatten() {
            if !mask[j] && (proche(src[j], cible, tol) || lisse(src, w, h, j)) {
                mask[j] = true;
                pile.push(j);
            }
        }
    }
    // Une couche de plus, en huit voisins : les pixels de la zone collés au trait ne sont pas lisses (le trait est leur voisin)
    // et seraient restés de l'ancienne couleur. Une seule couche : la zone ne glisse pas le long du bord du trait.
    let mut ajouts = Vec::new();
    for i in frontiere(&mask) {
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if !mask[j] && proche(src[j], src[i], PAS_DOUX) {
                    ajouts.push(j);
                }
            }
        }
    }
    for j in ajouts {
        mask[j] = true;
    }
    mask
}

/// Le dégradé en couleurs d'une grille de `n` × `n` cases sur le rectangle `de`..`a` : de quoi peindre l'aperçu d'un geste
/// avec un maillage de sommets colorés (un sommet par angle de case, prémultiplié).
pub fn sommets(g: &Degrade, de: Pos2, a: Pos2, nx: usize, ny: usize, k: f32) -> Vec<Color32> {
    let mut out = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            let p = Pos2::new(de.x + (a.x - de.x) * i as f32 / nx.max(1) as f32, de.y + (a.y - de.y) * j as f32 / ny.max(1) as f32);
            out.push(g.pixel(p.x, p.y, k));
        }
    }
    out
}
