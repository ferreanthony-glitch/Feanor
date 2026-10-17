//! Lecture des images `.dip` de TVPaint (pointes des brosses, papiers anciens, icônes) :
//! le conteneur IFF « DEEP » (format public de l'époque Amiga, repris par TVPaint), lu d'après
//! les fichiers eux-mêmes (aucune spécification de TVPaint n'est publiée). Ce qui a été relevé :
//! - `FORM` … `DEEP`, blocs gros-boutistes, alignés sur deux octets ;
//! - `DGBL` : taille d'affichage, compression (1 : par plages), aspect ;
//! - `DPEL` : les composantes d'un pixel et leur profondeur (ex. bleu, vert, rouge, alpha en
//!   8 bits) ; types IFF-DEEP 1 rouge, 2 vert, 3 bleu, 4 alpha ; TVPaint écrit 17 pour l'alpha ;
//! - `DLOC` : la taille réelle de l'image ;
//! - `DBOD` : la première image, compressée par plages de pixels entiers (octet de commande
//!   n < 128 : n + 1 pixels tels quels ; n > 128 : le pixel suivant répété 257 − n fois) ;
//! - `DBAN` : chaque image suivante d'une brosse animée, en différences avec la précédente :
//!   les mêmes plages, plus `0x80 k` qui saute k pixels (k < 128) ou 256 − k pixels (k ≥ 128),
//!   et un `0x80` final ;
//! - `NIMA` : le nombre d'images ; `FRAT` : la cadence (en millièmes) ; `XS24` : une vignette.
//!
//! Vérifié sur les pointes de TVPaint 12 : chaque image se décode en exactement largeur × hauteur
//! pixels, en consommant tout son bloc.

/// Une image `.dip` : taille et images (une, ou plusieurs pour une brosse animée), en RVBA 8 bits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dip {
    pub width: usize,
    pub height: usize,
    pub frames: Vec<Vec<[u8; 4]>>,
    /// Images par seconde d'une brosse animée, si le fichier le dit.
    pub fps: Option<f32>,
}

fn be16(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}

/// Les blocs d'un conteneur IFF : (nom, contenu).
pub(crate) fn chunks(b: &[u8]) -> Result<Vec<(&str, &[u8])>, String> {
    if b.len() < 12 || &b[0..4] != b"FORM" || &b[8..12] != b"DEEP" {
        return Err("ce n'est pas une image DEEP (.dip)".into());
    }
    let mut out = Vec::new();
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = std::str::from_utf8(&b[i..i + 4]).map_err(|_| "bloc illisible")?;
        let len = be32(b, i + 4).ok_or("bloc tronqué")? as usize;
        let start = i + 8;
        let end = (start + len).min(b.len());
        out.push((id, &b[start..end]));
        i = start + len + (len & 1);
    }
    Ok(out)
}

/// Rang de chaque composante (rouge, vert, bleu, alpha) dans un pixel du fichier.
fn layout(dpel: &[u8]) -> Result<([Option<usize>; 4], usize), String> {
    let n = be32(dpel, 0).ok_or("DPEL tronqué")? as usize;
    let mut idx = [None; 4];
    for k in 0..n {
        let kind = be16(dpel, 4 + 4 * k).ok_or("DPEL tronqué")?;
        let depth = be16(dpel, 6 + 4 * k).ok_or("DPEL tronqué")?;
        if depth != 8 {
            return Err(format!("composante sur {depth} bits (seul le 8 bits est lu)"));
        }
        let slot = match kind {
            1 => 0,
            2 => 1,
            3 => 2,
            4 | 17 => 3,
            _ => continue,
        };
        idx[slot] = Some(k);
    }
    Ok((idx, n))
}

/// Décode des plages de pixels (`DBOD`, ou `DBAN` avec les sauts) dans `out` (pixels bruts de
/// `bpp` octets). Renvoie le nombre de pixels écrits ou sautés.
pub(crate) fn runs(data: &[u8], out: &mut [u8], bpp: usize, delta: bool) -> usize {
    let total = out.len() / bpp;
    let (mut p, mut q) = (0usize, 0usize);
    while p < data.len() && q < total {
        let c = data[p];
        p += 1;
        if c == 0x80 {
            if !delta {
                continue;
            }
            let Some(&k) = data.get(p) else { break };
            p += 1;
            q += if k < 0x80 { k as usize } else { 256 - k as usize };
        } else if c < 0x80 {
            let n = (c as usize + 1).min(total - q);
            let bytes = n * bpp;
            if p + bytes > data.len() {
                break;
            }
            out[q * bpp..q * bpp + bytes].copy_from_slice(&data[p..p + bytes]);
            p += (c as usize + 1) * bpp;
            q += n;
        } else {
            let n = (257 - c as usize).min(total - q);
            if p + bpp > data.len() {
                break;
            }
            for k in 0..n {
                out[(q + k) * bpp..(q + k + 1) * bpp].copy_from_slice(&data[p..p + bpp]);
            }
            p += bpp;
            q += n;
        }
    }
    q.min(total)
}

/// Lit un fichier `.dip`.
pub fn read(bytes: &[u8]) -> Result<Dip, String> {
    let chunks = chunks(bytes)?;
    let get = |name: &str| chunks.iter().find(|c| c.0 == name).map(|c| c.1);
    let dgbl = get("DGBL").ok_or("bloc DGBL absent")?;
    let compression = be16(dgbl, 4).ok_or("DGBL tronqué")?;
    let (w, h) = match get("DLOC") {
        Some(d) => (be16(d, 0).ok_or("DLOC tronqué")? as usize, be16(d, 2).ok_or("DLOC tronqué")? as usize),
        None => (be16(dgbl, 0).ok_or("DGBL tronqué")? as usize, be16(dgbl, 2).ok_or("DGBL tronqué")? as usize),
    };
    if w == 0 || h == 0 || w * h > 64_000_000 {
        return Err(format!("taille impossible : {w}×{h}"));
    }
    let (idx, bpp) = layout(get("DPEL").ok_or("bloc DPEL absent")?)?;
    let fps = get("FRAT").and_then(|f| be32(f, 0)).map(|v| v as f32 / 1000.0);
    let mut raw = vec![0u8; w * h * bpp];
    let mut frames = Vec::new();
    let to_rgba = |raw: &[u8]| -> Vec<[u8; 4]> {
        raw.chunks_exact(bpp)
            .map(|px| {
                let c = |k: usize, default: u8| idx[k].map_or(default, |i| px[i]);
                [c(0, 0), c(1, 0), c(2, 0), c(3, 255)]
            })
            .collect()
    };
    for (name, data) in &chunks {
        match *name {
            "DBOD" => {
                if compression == 0 {
                    let n = raw.len().min(data.len());
                    raw[..n].copy_from_slice(&data[..n]);
                } else if compression == 1 {
                    runs(data, &mut raw, bpp, false);
                } else {
                    return Err(format!("compression {compression} non lue"));
                }
                frames.push(to_rgba(&raw));
            }
            "DBAN" if !frames.is_empty() => {
                runs(data, &mut raw, bpp, true);
                frames.push(to_rgba(&raw));
            }
            _ => {}
        }
    }
    if frames.is_empty() {
        return Err("aucune image (bloc DBOD absent)".into());
    }
    Ok(Dip { width: w, height: h, frames, fps })
}

/// Les images d'une brosse côte à côte, en gris de dépôt (0 à 1, 1 où elle dépose) : une pointe
/// en plusieurs cases pour Pelure (`BrushSettings::tip_cells`). L'alpha fait le dépôt ; une
/// image sans aucune transparence prend son noir.
pub fn tip_strip(d: &Dip) -> crate::brushtex::GrayImage {
    let n = d.frames.len().max(1);
    let opaque = d.frames.iter().all(|f| f.iter().all(|p| p[3] == 255));
    let mut values = vec![0.0f32; d.width * n * d.height];
    for (k, f) in d.frames.iter().enumerate() {
        for y in 0..d.height {
            for x in 0..d.width {
                let p = f[y * d.width + x];
                let v = if opaque { 1.0 - (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0 } else { p[3] as f32 / 255.0 };
                values[y * d.width * n + k * d.width + x] = v;
            }
        }
    }
    crate::brushtex::GrayImage::new(d.width * n, d.height, values)
}
