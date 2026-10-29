//! Images collées dans un dessin vectoriel (Anthony, 2026-10-29 : « une image collée est collée sur un calque vectoriel :
//! l'image devient un objet vectoriel, sauf si elle est collée sur un calque bitmap »).
//!
//! Une image collée sur un calque vectoriel est un objet du dessin de la clé (`Element::Image`) : un rectangle texturé, avec
//! son placement (déplacement, échelle, rotation) et son opacité. Elle se sélectionne, se déplace, se transforme, se supprime
//! et se réordonne comme les traits et les zones. Aucun calque n'est créé. Collée sur un calque bitmap, elle reste des pixels
//! (`pasteimage.rs`).
//!
//! # Où sont les pixels
//!
//! Pas dans le dessin : l'objet ne porte que l'**empreinte** du contenu (`empreinte`), la taille et le placement. Les pixels
//! sont dans un magasin commun à tous les onglets (`obtenir`, `partager`), comme les textures des pinceaux ; un même contenu
//! collé dix fois n'y est qu'une fois. Le fichier du projet garde chaque image **une seule fois**, en PNG, dans
//! `images/<empreinte>.png` (un fichier ne change jamais sous la même empreinte : il n'est écrit que s'il manque, et effacé quand
//! plus aucun dessin ne le cite). À l'ouverture, `images/` est lu avec les textures. Une image dont le fichier manque ne se
//! montre pas (son rectangle reste sélectionnable).
//!
//! # Sans latence
//!
//! Une grande image (au-delà de `GRANDE` pixels) est préparée sur un fil de fond (réduction à `COTE_MAX`, empreinte) : l'interface
//! continue de vivre, le collage se pose quand c'est prêt (`collage.rs`). L'encodage PNG se fait à l'enregistrement, sur le fil
//! d'écriture.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use eframe::egui::Color32;

use crate::raster::{Raster, TILE};
use crate::render::over;
use crate::vector::{Affine, Element};

/// Dossier des images du projet.
pub(crate) const DOSSIER: &str = "images";

/// Plus grand côté gardé d'une image collée : au-delà, elle est réduite au collage.
pub const COTE_MAX: usize = 4096;

/// Au-dessus de cette surface (pixels), la préparation d'une image collée passe par un fil de fond.
pub const GRANDE: usize = 1_000_000;

/// Les pixels d'une image collée : RVBA **non prémultipliés**, ligne par ligne (la forme que le PNG garde).
pub struct Pixels {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

static MAGASIN: LazyLock<RwLock<HashMap<u64, Arc<Pixels>>>> = LazyLock::new(Default::default);

/// L'empreinte du contenu : la même image donne toujours la même empreinte.
pub fn empreinte(w: usize, h: usize, rgba: &[u8]) -> u64 {
    let mut h64: u64 = 0xcbf2_9ce4_8422_2325 ^ ((w as u64) << 32) ^ h as u64;
    let mut chunks = rgba.chunks_exact(8);
    for c in &mut chunks {
        let v = u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
        h64 = (h64.rotate_left(23) ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    for &b in chunks.remainder() {
        h64 = (h64.rotate_left(8) ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h64 ^ (h64 >> 29)
}

/// Les pixels de l'image d'empreinte `key`, s'ils sont dans le magasin.
pub fn obtenir(key: u64) -> Option<Arc<Pixels>> {
    MAGASIN.read().ok()?.get(&key).cloned()
}

/// L'image d'empreinte `key` est dans le magasin.
pub fn a_deja(key: u64) -> bool {
    MAGASIN.read().is_ok_and(|m| m.contains_key(&key))
}

/// Met l'image dans le magasin (sans rien changer si l'empreinte y est déjà).
pub fn partager(key: u64, px: Arc<Pixels>) {
    if let Ok(mut m) = MAGASIN.write() {
        m.entry(key).or_insert(px);
    }
}

/// Essais : l'image n'est plus dans le magasin (comme à l'ouverture d'une autre séance).
#[cfg(test)]
pub fn oublier(key: u64) {
    if let Ok(mut m) = MAGASIN.write() {
        m.remove(&key);
    }
}

/// Une image collée prête : (empreinte, largeur, hauteur). Réduite à `COTE_MAX` si elle est plus grande, puis mise dans le magasin.
/// C'est ce que fait le fil de fond d'une grande image.
pub fn preparer(w: usize, h: usize, rgba: Vec<u8>) -> Option<(u64, usize, usize)> {
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return None;
    }
    // Une image dont aucun pixel n'a d'opacité (certains logiciels n'en écrivent pas) est opaque.
    let mut rgba = rgba;
    rgba.truncate(w * h * 4);
    if rgba.chunks_exact(4).all(|p| p[3] == 0) {
        for p in rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
    let (w, h, rgba) = if w.max(h) > COTE_MAX {
        let (tw, th) = crate::pasteimage::fit_size(w, h, COTE_MAX, COTE_MAX);
        let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba)?;
        (tw, th, image::imageops::resize(&img, tw as u32, th as u32, image::imageops::FilterType::Triangle).into_raw())
    } else {
        (w, h, rgba)
    };
    let key = empreinte(w, h, &rgba);
    partager(key, Arc::new(Pixels { w, h, rgba }));
    Some((key, w, h))
}

/// Le PNG de l'image, tel que `images/` le garde.
pub fn png(px: &Pixels) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(px.w as u32, px.h as u32, px.rgba.clone()).ok_or("image de taille invalide")?;
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Le fichier d'une image dans le dossier d'un projet.
pub fn fichier(dir: &Path, key: u64) -> PathBuf {
    dir.join(DOSSIER).join(format!("{key:016x}.png"))
}

fn cle_du_fichier(path: &Path) -> Option<u64> {
    if path.extension().and_then(|e| e.to_str()) != Some("png") {
        return None;
    }
    u64::from_str_radix(path.file_stem()?.to_str()?, 16).ok()
}

/// Les empreintes des images citées par des éléments.
pub fn citees<'a>(elements: impl IntoIterator<Item = &'a Element>) -> BTreeSet<u64> {
    elements.into_iter().filter_map(|e| if let Element::Image { key, .. } = e { Some(*key) } else { None }).collect()
}

/// Lit les images du projet et les met dans le magasin. `nouvelles_seulement` : un plan relu seul ne relit pas celles qu'il a déjà.
pub fn charger_dossier(dir: &Path, warnings: &mut Vec<String>, nouvelles_seulement: bool) {
    let Ok(entries) = std::fs::read_dir(dir.join(DOSSIER)) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(key) = cle_du_fichier(&path) else { continue };
        if nouvelles_seulement && a_deja(key) {
            continue;
        }
        match image::open(&path) {
            Ok(img) => {
                let img = img.to_rgba8();
                let (w, h) = (img.width() as usize, img.height() as usize);
                partager(key, Arc::new(Pixels { w, h, rgba: img.into_raw() }));
            }
            Err(e) => warnings.push(format!("image collée {} illisible ({e}) : l'image ne se montre pas", path.display())),
        }
    }
}

/// Efface les images du dossier que plus aucun dessin ne cite. Rend combien sont parties.
pub fn effacer_inutiles(dir: &Path, utilisees: &BTreeSet<u64>) -> usize {
    let Ok(entries) = std::fs::read_dir(dir.join(DOSSIER)) else { return 0 };
    let mut partis = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(key) = cle_du_fichier(&path) {
            if !utilisees.contains(&key) && std::fs::remove_file(&path).is_ok() {
                partis += 1;
            }
        }
    }
    partis
}

/// Une image rendue : le contenu `px` posé sur `raster` avec `place` (pixel de l'image → pixel du dessin), à `opacity`, dans la zone
/// (x0, y0, largeur, hauteur) seulement. Échantillonnage bilinéaire ; une image réduite de plus de moitié prend la moyenne de
/// plusieurs points par pixel (pas de crénelage), et ses bords se fondent (le pixel à moitié couvert est à moitié transparent).
pub fn poser(raster: &mut Raster, px: &Pixels, place: &Affine, opacity: f32, zone: (i64, i64, usize, usize)) {
    let Some(inv) = place.inverse() else { return };
    let op = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    if op == 0 || px.w == 0 || px.h == 0 {
        return;
    }
    let (x0, y0, w, h) = zone;
    let reduction = place.width_factor();
    let n: usize = if reduction > 0.0 && reduction < 0.5 { ((1.0 / reduction).ceil() as usize).clamp(2, 3) } else { 1 };
    let t = TILE as i64;
    for ly in 0..h as i64 {
        let y = y0 + ly;
        for lx in 0..w as i64 {
            let x = x0 + lx;
            let mut acc = [0.0f32; 4];
            for sy in 0..n {
                for sx in 0..n {
                    let dx = (sx as f32 + 0.5) / n as f32;
                    let dy = (sy as f32 + 0.5) / n as f32;
                    let p = inv.apply(eframe::egui::pos2(x as f32 + dx, y as f32 + dy));
                    let s = echantillon(px, p.x - 0.5, p.y - 0.5);
                    for k in 0..4 {
                        acc[k] += s[k];
                    }
                }
            }
            let k = 1.0 / (n * n) as f32;
            let a = (acc[3] * k).round();
            if a < 1.0 {
                continue;
            }
            let c = |v: f32| (v * k).round().clamp(0.0, a) as u8;
            let src = Color32::from_rgba_premultiplied(c(acc[0]), c(acc[1]), c(acc[2]), a.min(255.0) as u8);
            let idx = raster.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
            let i = y.rem_euclid(t) as usize * TILE + x.rem_euclid(t) as usize;
            let tile = raster.tile_mut(idx);
            tile[i] = over(tile[i], src, op, None);
        }
    }
}

/// Un pixel de l'image (prémultiplié, 0 à 255) ; transparent hors de l'image.
fn pixel(px: &Pixels, x: i64, y: i64) -> [f32; 4] {
    if x < 0 || y < 0 || x >= px.w as i64 || y >= px.h as i64 {
        return [0.0; 4];
    }
    let i = (y as usize * px.w + x as usize) * 4;
    let a = px.rgba[i + 3] as f32;
    let m = a / 255.0;
    [px.rgba[i] as f32 * m, px.rgba[i + 1] as f32 * m, px.rgba[i + 2] as f32 * m, a]
}

/// Bilinéaire autour du point (u, v) (le centre du pixel (i, j) est en (i, j) ici).
fn echantillon(px: &Pixels, u: f32, v: f32) -> [f32; 4] {
    let (fu, fv) = (u.floor(), v.floor());
    let (tu, tv) = (u - fu, v - fv);
    let (iu, iv) = (fu as i64, fv as i64);
    let a = pixel(px, iu, iv);
    let b = pixel(px, iu + 1, iv);
    let c = pixel(px, iu, iv + 1);
    let d = pixel(px, iu + 1, iv + 1);
    let mut out = [0.0; 4];
    for k in 0..4 {
        out[k] = (a[k] * (1.0 - tu) + b[k] * tu) * (1.0 - tv) + (c[k] * (1.0 - tu) + d[k] * tu) * tv;
    }
    out
}
