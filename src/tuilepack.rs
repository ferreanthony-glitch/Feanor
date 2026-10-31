//! Tuiles comprimées en mémoire (chantier « mémoire d'un épisode », docs/ENDURANCE.md §6).
//!
//! Une tuile de pixels pèse 16 Ko ; un trait au crayon n'en remplit que quelques pour cent (le reste est transparent)
//! et un aplat n'a qu'une couleur. Ce module range une tuile dans un petit tampon, **sans aucune perte** : les mêmes
//! pixels reviennent, au bit près. Rien n'est écrit sur le disque, rien n'est oublié : le tampon est une copie exacte.
//!
//! Format (des mots de 32 bits, un pixel premultiplié RGBA chacun, dans l'ordre de la tuile) : une suite de morceaux.
//! Un morceau commence par un mot d'en-tête de 16 bits (petit-boutiste) :
//! - bit 15 à 1 : une **série** de `n + 1` pixels identiques (`n` = les 15 autres bits), suivie du pixel (4 octets) ;
//! - bit 15 à 0 : `n + 1` pixels **littéraux** qui suivent (4 octets chacun).
//!
//! Une série ne vaut la peine qu'à partir de 3 pixels (un en-tête de 2 octets + 4 octets contre 4 octets par pixel) ;
//! en dessous, les pixels restent dans le morceau littéral en cours.

use eframe::egui::Color32;

use crate::raster::TILE_PIXELS;

/// Un tampon plus gros que cela ne gagne presque rien sur les 16 384 octets d'une tuile : elle reste en pixels.
pub const MAX_OCTETS: usize = 12 * 1024;

/// Le plus de pixels d'un morceau (15 bits).
const MAX_MORCEAU: usize = 1 << 15;

/// Les octets de `pixels`, ou `None` si le résultat dépasserait `MAX_OCTETS` (une tuile qui ne se range pas).
pub fn pack(pixels: &[Color32; TILE_PIXELS]) -> Option<Box<[u8]>> {
    let px: &[u32] = bytemuck::cast_slice(&pixels[..]);
    let mut out: Vec<u8> = Vec::with_capacity(2048);
    let mut i = 0;
    let mut lit_debut = 0usize; // début du morceau littéral en cours
    let flush = |out: &mut Vec<u8>, from: usize, to: usize| {
        let mut a = from;
        while a < to {
            let n = (to - a).min(MAX_MORCEAU);
            out.extend_from_slice(&((n - 1) as u16).to_le_bytes());
            out.extend_from_slice(bytemuck::cast_slice(&px[a..a + n]));
            a += n;
        }
    };
    while i < px.len() {
        let v = px[i];
        let mut j = i + 1;
        while j < px.len() && px[j] == v {
            j += 1;
        }
        if j - i >= 3 {
            flush(&mut out, lit_debut, i);
            let mut a = i;
            while a < j {
                let n = (j - a).min(MAX_MORCEAU);
                out.extend_from_slice(&(0x8000u16 | (n - 1) as u16).to_le_bytes());
                out.extend_from_slice(&v.to_le_bytes());
                a += n;
            }
            lit_debut = j;
        }
        i = j;
        if out.len() > MAX_OCTETS {
            return None;
        }
    }
    flush(&mut out, lit_debut, px.len());
    (out.len() <= MAX_OCTETS).then(|| out.into_boxed_slice())
}

/// Les pixels que `pack` a rangés dans `octets`. Un tampon qui ne vient pas de `pack` n'existe pas (rien ne le lit d'ailleurs) :
/// ce qui ne remplit pas la tuile est transparent.
pub fn unpack(octets: &[u8]) -> Box<[Color32; TILE_PIXELS]> {
    let mut out: Box<[Color32; TILE_PIXELS]> = vec![Color32::TRANSPARENT; TILE_PIXELS].into_boxed_slice().try_into().expect("taille d'une tuile");
    unpack_into(octets, &mut out);
    out
}

/// Comme `unpack`, dans un tampon de pixels qu'on fournit (il est d'abord vidé).
pub fn unpack_into(octets: &[u8], out: &mut [Color32; TILE_PIXELS]) {
    out.fill(Color32::TRANSPARENT);
    let dst: &mut [u32] = bytemuck::cast_slice_mut(&mut out[..]);
    let (mut p, mut k) = (0usize, 0usize);
    while p + 2 <= octets.len() && k < dst.len() {
        let h = u16::from_le_bytes([octets[p], octets[p + 1]]);
        p += 2;
        let n = ((h & 0x7FFF) as usize + 1).min(dst.len() - k);
        if h & 0x8000 != 0 {
            let Some(b) = octets.get(p..p + 4) else { break };
            let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            p += 4;
            dst[k..k + n].fill(v);
        } else {
            let Some(b) = octets.get(p..p + 4 * n) else { break };
            for (d, c) in dst[k..k + n].iter_mut().zip(b.chunks_exact(4)) {
                *d = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            }
            p += 4 * n;
        }
        k += n;
    }
}
