//! Empreintes de contenu : un nombre de 64 bits qui dit « c'est le même dessin », le même d'une
//! séance à l'autre et d'un poste à l'autre (`std::hash::DefaultHasher` ne le promet pas d'une version
//! de Rust à l'autre). Elles servent aux envois et aux versions de plans (`envois.rs`) : ce qui a changé
//! depuis un envoi, et les fichiers de `versions/` rangés par empreinte, un dessin qui n'a pas bougé
//! n'étant écrit qu'une fois.
//!
//! Ce n'est pas une signature : rien ne protège d'un fichier fabriqué pour tomber sur la même empreinte.
//! Pour deux dessins qui diffèrent par accident, la chance de se rencontrer est de 2^-64.
//!
//! # Ce que compte l'empreinte d'un dessin
//!
//! - **Bitmap** : la taille de la feuille et les pixels **tels que le PNG les garde** (couleur non
//!   prémultipliée), tuile par tuile ; une tuile allouée mais entièrement transparente compte comme
//!   une tuile absente (une gomme laisse de telles tuiles). Hacher ce que le PNG garde, et non la
//!   mémoire, évite qu'un dessin enregistré, rouvert, ne paraisse changé.
//! - **Vectoriel** : le dessin tel qu'il s'écrit (`serde_json`), pas son image : l'image est dérivée
//!   (elle peut être approchée jusqu'au rendu exact en arrière-plan, sans que le dessin change).
//! - **Strate Fond** (peinture et coupe), s'il y en a une non vide.
//!
//! # Coût
//!
//! Un dessin se copie en quelques microsecondes (`Donnees::de` : les tuiles sont partagées, `Arc`), et se
//! hache ailleurs que sur le fil de l'interface (`envoisui.rs`). Compter environ 4 µs par tuile pleine.

use eframe::egui::Color32;

use crate::document::{Cel, CelId, LayerKind, Panel};
use crate::raster::{unpremultiply, Raster, TILE_PIXELS};
use crate::vector::VectorDrawing;

const K: u64 = 0x9E37_79B9_7F4A_7C15;

#[inline]
fn melange(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Une empreinte en train de se faire : on lui donne des mots et des octets, `fin` rend le nombre.
#[derive(Clone)]
pub struct Empreinte {
    h: u64,
    n: u64,
}

impl Default for Empreinte {
    fn default() -> Self {
        Empreinte { h: 0x243F_6A88_85A3_08D3, n: 0 }
    }
}

impl Empreinte {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn mot(&mut self, w: u64) {
        self.h = melange((self.h ^ w).wrapping_add(K));
        self.n += 1;
    }

    /// Des octets, huit par huit ; la longueur compte (« ab » et « ab\0 » diffèrent).
    pub fn octets(&mut self, b: &[u8]) {
        let mut chunks = b.chunks_exact(8);
        for c in &mut chunks {
            self.mot(u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
        }
        let mut last = [0u8; 8];
        let rest = chunks.remainder();
        last[..rest.len()].copy_from_slice(rest);
        self.mot(u64::from_le_bytes(last));
        self.mot(b.len() as u64);
    }

    pub fn fin(self) -> u64 {
        // (Jamais 0 : zéro dit « pas d'empreinte » ailleurs.)
        melange(self.h ^ self.n.rotate_left(32)).max(1)
    }
}

/// L'empreinte d'une suite d'octets.
pub fn de_octets(b: &[u8]) -> u64 {
    let mut e = Empreinte::new();
    e.octets(b);
    e.fin()
}

/// Ajoute à `e` les pixels de `r` : taille, puis chaque tuile qui montre quelque chose (son rang et
/// ses pixels, non prémultipliés comme dans le PNG).
pub fn raster(e: &mut Empreinte, r: &Raster) {
    e.mot(r.width as u64);
    e.mot(r.height as u64);
    for (idx, tile) in r.tiles.iter().enumerate() {
        let Some(tile) = tile else { continue };
        if tile.iter().all(|c| c.a() == 0) {
            continue;
        }
        e.mot(idx as u64 + 1);
        let px = |c: &Color32| u32::from_le_bytes(unpremultiply(*c)) as u64;
        for pair in tile.chunks_exact(2) {
            e.mot(px(&pair[0]) | (px(&pair[1]) << 32));
        }
        debug_assert_eq!(TILE_PIXELS % 2, 0);
    }
}

/// Ce qu'il faut d'un dessin pour le hacher et l'écrire, copié à bas prix : les tuiles d'un bitmap sont
/// partagées, un dessin vectoriel est copié (son image dérivée ne l'est pas).
#[derive(Clone)]
pub struct Donnees {
    pub largeur: usize,
    pub hauteur: usize,
    pub contenu: Contenu,
    /// Strate Fond (peinture, coupe), s'il y en a une qui montre quelque chose.
    pub fond: Option<(Raster, Raster)>,
}

#[derive(Clone)]
pub enum Contenu {
    Bitmap(Raster),
    Vectoriel(VectorDrawing),
}

impl Donnees {
    pub fn de(cel: &Cel) -> Donnees {
        let contenu = match &cel.vector {
            Some(v) => Contenu::Vectoriel(v.clone()),
            None => Contenu::Bitmap(cel.raster.clone()),
        };
        let fond = cel.fond.as_ref().filter(|s| !s.is_empty()).map(|s| (s.peinture.raster.clone(), s.coupe.clone()));
        Donnees { largeur: cel.raster.width, hauteur: cel.raster.height, contenu, fond }
    }

    /// Un dessin vide du genre `kind` (un dessin qu'on ne trouve pas).
    pub fn vide(largeur: usize, hauteur: usize, kind: LayerKind) -> Donnees {
        let contenu = match kind {
            LayerKind::Vector => Contenu::Vectoriel(VectorDrawing::default()),
            LayerKind::Bitmap => Contenu::Bitmap(Raster::new(largeur, hauteur)),
        };
        Donnees { largeur, hauteur, contenu, fond: None }
    }

    pub fn vectoriel(&self) -> bool {
        matches!(self.contenu, Contenu::Vectoriel(_))
    }

    /// L'empreinte du dessin (voir plus haut ce qui compte).
    pub fn empreinte(&self) -> u64 {
        let mut e = Empreinte::new();
        e.mot(self.largeur as u64);
        e.mot(self.hauteur as u64);
        match &self.contenu {
            Contenu::Bitmap(r) => {
                e.mot(1);
                raster(&mut e, r);
            }
            Contenu::Vectoriel(v) => {
                e.mot(2);
                // (Un dessin qui ne s'écrit pas est un dessin vide : il ne peut pas s'enregistrer non plus.)
                e.octets(&serde_json::to_vec(v).unwrap_or_default());
            }
        }
        if let Some((peinture, coupe)) = &self.fond {
            e.mot(3);
            raster(&mut e, peinture);
            raster(&mut e, coupe);
        }
        e.fin()
    }
}

/// Les dessins que montrent les calques de `panel`, sans doublon.
pub fn dessins_du_plan(panel: &Panel) -> Vec<CelId> {
    let mut seen = std::collections::BTreeSet::new();
    for l in &panel.layers {
        seen.extend(l.frames.iter().flatten().copied());
    }
    seen.into_iter().collect()
}
