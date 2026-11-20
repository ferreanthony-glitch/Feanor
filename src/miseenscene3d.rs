//! Partie 200 (`docs/INTERFACE.md` partie 200, étape 5 sur 5 de la maquette de l'onglet 3D) : **la mise en scène 3D**. Les données et
//! leurs calculs ; l'interface est dans `miseenscene3dui.rs`.
//!
//! - **Les lampes** (`Lampe3d`) : des lumières ponctuelles du décor d'une séquence, en plus du soleil et du ciel : une place, une
//!   couleur (Ampoule, Blanche, Bleutée, Rouge, ou choisie), une intensité, une portée en mètres (rien au-delà). Au plus
//!   `MAX_LAMPES` par séquence. Elles éclairent le décor dans les trois aspects (la carte graphique, `decor3dgpu::SHADER`, et le
//!   processeur, `Aspect::ombrer_au`, avec les mêmes formules : `LampesVues::lumiere`). **Elles ne portent pas d'ombres** (une lampe
//!   ponctuelle en demanderait six cartes d'ombres : hors du budget « sans latence ») et **n'éclairent pas les cartes** : les cartes ne
//!   sont pas éclairées par le soleil non plus, seulement assombries par les ombres (partie 185).
//! - **Les clés d'un objet** (`CleObjet`) : sa place (position, rotation autour de la verticale, échelle) à une image de la séquence,
//!   adoucie entre deux clés comme la maquette (`place_a` : début et fin lents). La règle des caméras de plan pour les retoucher
//!   (`poser`) : avec deux clés ou plus, déplacer l'objet règle la clé de l'image courante (créée au besoin) ; avec une, il règle
//!   celle-là ; sans clé, sa place.
//! - **Caché dans le plan** (`Objet3d::caches_dans`) : l'objet reste dans le décor, mais l'image de ces plans ne le montre pas.
//!
//! Tout est dans `decors3d.json` (version 2), en champs facultatifs : un fichier d'avant se lit (rien d'animé, aucune lampe, rien de
//! caché), et un FEANOR d'avant lit celui-ci (il ignore les clés qu'il ne connaît pas).

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::decor3d::{Decor3d, Place3d};
use crate::document::Document;
use crate::espace3dmath::{self as m, M4, V3};

/// Les lampes d'une séquence, au plus (la carte graphique les lit toutes à chaque pixel du décor : huit restent loin sous le coût d'une
/// passe, sans ombres).
pub const MAX_LAMPES: usize = 8;
/// Les teintes proposées (sRGB), celles de la maquette.
pub const TEINTES: [(&str, [u8; 3]); 4] = [("Ampoule", [0xff, 0xcf, 0x8a]), ("Blanche", [0xff, 0xff, 0xff]), ("Bleutée", [0x9f, 0xc2, 0xff]), ("Rouge", [0xff, 0x8a, 0x6a])];
/// Une lampe neuve : son intensité, sa portée (mètres), sa hauteur au-dessus du point visé par la vue.
pub const INTENSITE: f32 = 0.8;
pub const PORTEE: f32 = 5.5;
pub const HAUTEUR: f32 = 2.2;
/// Les bornes des réglages d'une lampe.
pub const INTENSITE_MAX: f32 = 2.0;
pub const PORTEE_MIN: f32 = 0.5;
pub const PORTEE_MAX: f32 = 30.0;

// ------------------------------------------------------------------------------------------------ les clés d'un objet

/// Une clé d'animation d'un objet du décor : sa place à l'image `image` de la séquence (0 : la première image de son premier plan).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CleObjet {
    pub image: usize,
    #[serde(flatten)]
    pub place: Place3d,
}

/// L'adoucissement entre deux clés : début et fin lents (`3t² − 2t³`, celui de la maquette et de la boussole, `onglet3d::adoucir`).
pub fn adoucir(t: f32) -> f32 {
    crate::onglet3d::adoucir(t)
}

/// La place que donnent les clés `cles` (triées) à l'image `image` : avant la première, la première ; après la dernière, la dernière ;
/// entre deux, adoucie. Rien sans clé.
pub fn place_a(cles: &[CleObjet], image: usize) -> Option<Place3d> {
    let (premiere, derniere) = (cles.first()?, cles.last()?);
    if cles.len() == 1 || image <= premiere.image {
        return Some(premiere.place);
    }
    if image >= derniere.image {
        return Some(derniere.place);
    }
    let i = cles.iter().position(|k| k.image > image)?;
    let (a, b) = (&cles[i - 1], &cles[i]);
    let t = adoucir((image - a.image) as f32 / (b.image - a.image).max(1) as f32);
    let l = |x: f32, y: f32| x + (y - x) * t;
    Some(Place3d {
        position: [0, 1, 2].map(|k| l(a.place.position[k], b.place.position[k])),
        rotation_y: l(a.place.rotation_y, b.place.rotation_y),
        echelle: l(a.place.echelle_sure(), b.place.echelle_sure()),
    })
}

/// Pose (ou remplace) la clé de l'image `image`, les clés gardées triées.
pub fn poser_cle(cles: &mut Vec<CleObjet>, image: usize, place: Place3d) {
    match cles.iter().position(|k| k.image >= image) {
        Some(i) if cles[i].image == image => cles[i].place = place,
        Some(i) => cles.insert(i, CleObjet { image, place }),
        None => cles.push(CleObjet { image, place }),
    }
}

/// **La règle des caméras de plan** pour une place `place` retouchée à l'image `image` : deux clés ou plus, la clé de cette image
/// (créée au besoin) ; une, celle-là ; aucune : rien (la place de l'objet suffit). Vrai si une clé a été créée.
pub fn retoucher(cles: &mut Vec<CleObjet>, image: usize, place: Place3d) -> bool {
    match cles.len() {
        0 => false,
        1 => {
            cles[0].place = place;
            false
        }
        _ => {
            let neuve = !cles.iter().any(|k| k.image == image);
            poser_cle(cles, image, place);
            neuve
        }
    }
}

/// Des clés lues d'un fichier : finies, triées, une par image.
pub fn cles_valides(mut cles: Vec<CleObjet>) -> Vec<CleObjet> {
    cles.retain(|k| k.place.position.iter().all(|v| v.is_finite()) && k.place.rotation_y.is_finite() && k.place.echelle.is_finite());
    cles.sort_by_key(|k| k.image);
    cles.dedup_by_key(|k| k.image);
    cles
}

// ------------------------------------------------------------------------------------------------ les lampes

/// Une lampe du décor d'une séquence : une lumière ponctuelle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lampe3d {
    pub id: u64,
    pub nom: String,
    /// Sa place (mètres, dans le repère du décor).
    pub position: V3,
    /// Sa couleur (sRGB).
    #[serde(default = "ampoule")]
    pub couleur: [u8; 3],
    #[serde(default = "intensite")]
    pub intensite: f32,
    /// Sa portée (mètres) : elle n'éclaire rien au-delà.
    #[serde(default = "portee")]
    pub portee: f32,
    /// Éteinte par son œil dans « Scène 3D ».
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cache: bool,
}

fn ampoule() -> [u8; 3] {
    TEINTES[0].1
}
fn intensite() -> f32 {
    INTENSITE
}
fn portee() -> f32 {
    PORTEE
}

impl Lampe3d {
    pub fn neuve(id: u64, nom: String, position: V3) -> Lampe3d {
        Lampe3d { id, nom, position, couleur: ampoule(), intensite: INTENSITE, portee: PORTEE, cache: false }
    }

    /// Ses réglages dans les bornes (un fichier abîmé ne fait rien d'étrange).
    pub fn bornee(mut self) -> Lampe3d {
        let fin = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.position = self.position.map(|v| fin(v, -1e4, 1e4, 0.0));
        self.intensite = fin(self.intensite, 0.0, INTENSITE_MAX, INTENSITE);
        self.portee = fin(self.portee, PORTEE_MIN, PORTEE_MAX, PORTEE);
        self
    }

    /// Ce qui fait sa lumière, dans l'empreinte de l'image du décor (`decor3d::contenu_a`).
    pub fn empreinte<H: Hasher>(&self, h: &mut H) {
        self.position.map(f32::to_bits).hash(h);
        self.couleur.hash(h);
        self.intensite.to_bits().hash(h);
        self.portee.to_bits().hash(h);
    }

    /// Le nom de sa teinte, si c'en est une des proposées.
    pub fn teinte(&self) -> Option<&'static str> {
        TEINTES.iter().find(|t| t.1 == self.couleur).map(|t| t.0)
    }
}

/// Des lampes lues d'un fichier : dans leurs bornes, au plus `MAX_LAMPES`.
pub fn lampes_valides(mut v: Vec<Lampe3d>) -> Vec<Lampe3d> {
    v.truncate(MAX_LAMPES);
    v.into_iter().map(Lampe3d::bornee).collect()
}

/// Les lampes allumées d'un décor, prêtes pour le rendu (`Aspect::lampes`) : place (dans le repère où l'on regarde) et portée ; couleur
/// linéaire fois l'intensité.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LampesVues {
    pub n: u32,
    pub pos: [[f32; 4]; MAX_LAMPES],
    pub col: [[f32; 4]; MAX_LAMPES],
}

impl LampesVues {
    /// La lumière (linéaire) que les lampes donnent au point `p` de normale `n` : les formules du shader (`decor3dgpu`, `lampes`).
    pub fn lumiere(&self, p: V3, n: V3) -> [f32; 3] {
        let mut s = [0.0f32; 3];
        for i in 0..(self.n as usize).min(MAX_LAMPES) {
            let [x, y, z, r] = self.pos[i];
            let l = m::sub([x, y, z], p);
            let d = m::length(l);
            let a = (1.0 - d / r.max(0.01)).clamp(0.0, 1.0);
            let k = m::dot(n, m::scale(l, 1.0 / d.max(1e-4))).max(0.0) * a * a;
            for c in 0..3 {
                s[c] += self.col[i][c] * k;
            }
        }
        s
    }
}

/// Les lampes allumées du décor `d`, passées dans le repère `repere` (celui d'un plan en Espace 3D, sinon l'identité).
pub fn lampes_vues(d: &Decor3d, repere: &M4) -> LampesVues {
    let mut v = LampesVues::default();
    for l in d.lampes.iter().filter(|l| !l.cache).take(MAX_LAMPES) {
        let p = m::point(repere, l.position);
        let c = crate::couleurs3d::lineaire(l.couleur);
        let i = v.n as usize;
        v.pos[i] = [p[0], p[1], p[2], l.portee];
        v.col[i] = [c[0] * l.intensite, c[1] * l.intensite, c[2] * l.intensite, 0.0];
        v.n += 1;
    }
    v
}

// ------------------------------------------------------------------------------------------------ la séquence

impl Document {
    /// L'image de la séquence de l'image `frame` du plan `index` : depuis la première image de son premier plan (les clés des objets
    /// du décor y sont posées).
    pub fn image_de_sequence(&self, index: usize, frame: usize) -> usize {
        if index >= self.panels.len() {
            return frame;
        }
        let (a, _) = self.sequence_run(index);
        self.panels[a..index].iter().map(crate::document::Panel::frame_count).sum::<usize>() + frame.min(self.panels[index].frame_count().saturating_sub(1))
    }

    /// Le plan de la séquence du plan `index` qui montre l'image `image` de la séquence, et son image (la dernière image de la
    /// séquence au-delà).
    pub fn plan_de_l_image(&self, index: usize, image: usize) -> (usize, usize) {
        let (a, z) = self.sequence_run(index);
        let mut reste = image;
        for pi in a..z {
            let n = self.panels[pi].frame_count();
            if reste < n {
                return (pi, reste);
            }
            reste -= n;
        }
        (z - 1, self.panels[z - 1].frame_count().saturating_sub(1))
    }
}
