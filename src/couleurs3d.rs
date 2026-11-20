//! Partie 199 (Anthony : « donner une couleur à un objet 3D pour le rendre plus identifiable dans un storyboard ») : les couleurs
//! d'identification des objets du décor 3D, et les trois aspects du décor (`docs/INTERFACE.md` partie 199).
//!
//! - **La palette** : dix couleurs nommées, celles de la maquette (`PALETTE`), plus « Aucune ». Elles se distinguent par leur teinte
//!   et aussi par leur clarté (du brun et du violet, sombres, au jaune et au gris clair). Un objet garde le rang de sa couleur
//!   (`Objet3d::couleur`, `decors3d.json`) ; un objet neuf n'en a pas.
//! - **Les aspects** (`Rendu`) : **Matières** (le rendu d'avant : les couleurs et les textures du modèle), **Couleurs** (chaque objet
//!   dans sa couleur d'identification, éclairé à plat, un gris neutre sans couleur), **Argile** (tout en une argile grise claire,
//!   avec la lumière et les ombres). Celui de la vue (onglet 3D, Espace 3D, vue à côté) est un réglage du poste
//!   (`Settings::aspect_vue_3d`) ; celui des images des plans (le dessin, la planche, le montage, les vignettes, l'export) est celui
//!   du décor de la séquence, « Dans les plans » (`Aspect::rendu`, `decors3d.json`).
//! - **Une teinte par objet** (`Teinte`) : ce que la carte graphique (`decor3dgpu.rs`) et le rendu par le processeur
//!   (`decor3drendu.rs`) font d'un objet dans un aspect. Les maillages ne changent pas : seule la couleur de base de chaque matière,
//!   et la texture, qui ne garde que sa découpe (le feuillage d'un arbre reste découpé).

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Les dix couleurs d'identification (sRGB), dans l'ordre de la maquette.
pub const PALETTE: [[u8; 3]; 10] = [
    [0xe0, 0x6c, 0x6c],
    [0xf0, 0xa3, 0x4a],
    [0xe3, 0xcf, 0x57],
    [0x8c, 0xc6, 0x6a],
    [0x46, 0xb3, 0xa6],
    [0x5b, 0x9b, 0xe0],
    [0x9a, 0x7e, 0xe0],
    [0xde, 0x7b, 0xb4],
    [0xb0, 0x8a, 0x62],
    [0xd2, 0xd2, 0xd2],
];
/// Leurs noms.
pub const NOMS: [&str; 10] = ["Rouge", "Orange", "Jaune", "Vert", "Sarcelle", "Bleu", "Violet", "Rose", "Brun", "Gris clair"];
/// Le nom du choix « pas de couleur ».
pub const AUCUNE: &str = "Aucune";
/// L'argile (sRGB) : un gris clair un peu chaud.
pub const ARGILE: [u8; 3] = [0xbd, 0xb7, 0xad];
/// Un objet sans couleur, dans l'aspect Couleurs : un gris neutre.
pub const SANS: [u8; 3] = [0xa3, 0xa3, 0xa3];
/// La part de lumière « à plat » de l'aspect Couleurs : la couleur d'identification se lit d'un coup d'œil, la forme reste.
pub const PLAT: f32 = 0.55;

/// La couleur `k` de la palette (rien : hors palette).
pub fn couleur(k: u8) -> Option<[u8; 3]> {
    PALETTE.get(k as usize).copied()
}

/// Son nom ; « Aucune » sans couleur.
pub fn nom(k: Option<u8>) -> &'static str {
    k.and_then(|k| NOMS.get(k as usize).copied()).unwrap_or(AUCUNE)
}

pub fn color32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// sRGB (0–255) vers linéaire (0–1).
pub fn lineaire(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}

/// Comment le décor se montre : ses matières, les couleurs d'identification, ou l'argile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rendu {
    #[default]
    #[serde(rename = "matieres")]
    Matieres,
    #[serde(rename = "couleurs")]
    Couleurs,
    #[serde(rename = "argile")]
    Argile,
}

impl Rendu {
    pub const TOUS: [Rendu; 3] = [Rendu::Matieres, Rendu::Couleurs, Rendu::Argile];

    pub fn nom(self) -> &'static str {
        match self {
            Rendu::Matieres => "Matières",
            Rendu::Couleurs => "Couleurs",
            Rendu::Argile => "Argile",
        }
    }

    pub fn aide(self) -> &'static str {
        match self {
            Rendu::Matieres => "Les couleurs et les textures des modèles, comme ils sont faits.",
            Rendu::Couleurs => "Chaque objet dans sa couleur d'identification, éclairé à plat ; sans couleur, un gris neutre. Pour reconnaître les objets d'un coup d'œil.",
            Rendu::Argile => "Tout en une argile grise claire, avec la lumière et les ombres : les volumes seuls.",
        }
    }

    pub fn est_matieres(&self) -> bool {
        *self == Rendu::Matieres
    }
}

/// Ce qu'un objet devient dans un aspect : ses matières ; une couleur à plat (linéaire) ; l'argile.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Teinte {
    #[default]
    Matieres,
    Plate([f32; 3]),
    Argile,
}

impl Teinte {
    /// La teinte d'un objet de couleur `k` dans l'aspect `rendu`.
    pub fn de(rendu: Rendu, k: Option<u8>) -> Teinte {
        match rendu {
            Rendu::Matieres => Teinte::Matieres,
            Rendu::Couleurs => Teinte::Plate(lineaire(k.and_then(couleur).unwrap_or(SANS))),
            Rendu::Argile => Teinte::Argile,
        }
    }

    /// La couleur de base (linéaire) d'une matière de couleur `base` dans cette teinte, si la teinte la remplace (son alpha est
    /// gardé), et si la texture garde sa couleur (vrai) ou seulement sa découpe (faux).
    pub fn base(self, base: [f32; 4]) -> ([f32; 4], bool) {
        match self {
            Teinte::Matieres => (base, true),
            Teinte::Plate(c) => ([c[0], c[1], c[2], base[3]], false),
            Teinte::Argile => {
                let a = lineaire(ARGILE);
                ([a[0], a[1], a[2], base[3]], false)
            }
        }
    }

    /// Éclairée à plat (l'aspect Couleurs).
    pub fn plate(self) -> bool {
        matches!(self, Teinte::Plate(_))
    }
}
