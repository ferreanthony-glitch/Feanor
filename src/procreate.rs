//! Import des brosses Procreate (`.brush` : une brosse ; `.brushset` : un jeu ; moteur de
//! dessin, étape 10). Une brosse est une archive zip : `Brush.archive` (liste de propriétés
//! binaire d'Apple, « bplist00 », au format NSKeyedArchiver), `Shape.png` (la pointe) et
//! `Grain.png` (le grain). Un jeu met chaque brosse dans son dossier. Le format bplist est
//! publié par Apple (CFBinaryPList) ; les noms des réglages viennent des relevés de la
//! communauté (aucune documentation de Procreate) et leurs échelles sont devinées : dites dans
//! le compte rendu. Aucune brosse Procreate n'était sur le poste : à essayer sur une vraie.

use std::collections::HashMap;

use crate::brush::{BrushSettings, Connection, Source};
use crate::brushtex::{BrushLibrary, GrayImage, Preset, Target};
use crate::zipread::Zip;

/// Un objet d'une liste de propriétés.
#[derive(Clone, Debug, PartialEq)]
pub enum Plist {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Str(String),
    Data(Vec<u8>),
    Uid(usize),
    Array(Vec<Plist>),
    Dict(Vec<(Plist, Plist)>),
}

impl Plist {
    pub fn get(&self, key: &str) -> Option<&Plist> {
        match self {
            Plist::Dict(items) => items.iter().find(|(k, _)| matches!(k, Plist::Str(s) if s == key)).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            Plist::Int(v) => Some(*v as f64),
            Plist::Real(v) => Some(*v),
            Plist::Bool(b) => Some(*b as i32 as f64),
            _ => None,
        }
    }
}

/// Lit une liste de propriétés binaire (« bplist00 ») ; renvoie l'objet du haut.
pub fn read_bplist(b: &[u8]) -> Result<Plist, String> {
    if b.len() < 40 || &b[..8] != b"bplist00" {
        return Err("ce n'est pas une liste de propriétés binaire".into());
    }
    let t = &b[b.len() - 32..];
    let off_size = t[6] as usize;
    let ref_size = t[7] as usize;
    let be = |s: &[u8]| s.iter().fold(0u64, |a, &x| (a << 8) | x as u64);
    let count = be(&t[8..16]) as usize;
    let top = be(&t[16..24]) as usize;
    let table = be(&t[24..32]) as usize;
    // (Une table annoncée tout au bout de l'espace des nombres faisait déborder l'addition : la vérification
    // passait, et la lecture sortait du fichier.)
    if off_size == 0 || ref_size == 0 || count > 10_000_000 || table.checked_add(count * off_size).is_none_or(|end| end > b.len()) {
        return Err("liste de propriétés illisible".into());
    }
    let offsets: Vec<usize> = (0..count).map(|i| be(&b[table + i * off_size..table + (i + 1) * off_size]) as usize).collect();
    let mut seen = vec![false; count];
    object(b, &offsets, ref_size, top, &mut seen, 0)
}

fn object(b: &[u8], offsets: &[usize], ref_size: usize, i: usize, seen: &mut [bool], depth: usize) -> Result<Plist, String> {
    let bad = || "liste de propriétés illisible".to_string();
    if depth > 64 || i >= offsets.len() {
        return Err(bad());
    }
    let p = offsets[i];
    let marker = *b.get(p).ok_or_else(bad)?;
    let (kind, low) = (marker >> 4, (marker & 0x0F) as usize);
    let be = |s: &[u8]| s.iter().fold(0u64, |a, &x| (a << 8) | x as u64);
    // Longueur d'un objet : dans les quatre bits bas, ou un entier qui suit (0xF).
    let len_at = |p: usize| -> Result<(usize, usize), String> {
        if low != 0x0F {
            return Ok((low, p + 1));
        }
        let m = *b.get(p + 1).ok_or_else(bad)?;
        let n = 1usize << (m & 0x0F);
        Ok((be(b.get(p + 2..p + 2 + n).ok_or_else(bad)?) as usize, p + 2 + n))
    };
    Ok(match kind {
        0x0 => match marker {
            0x08 => Plist::Bool(false),
            0x09 => Plist::Bool(true),
            _ => Plist::Null,
        },
        0x1 => {
            let n = 1usize << low;
            let raw = b.get(p + 1..p + 1 + n).ok_or_else(bad)?;
            let v = be(raw);
            Plist::Int(if n == 8 { v as i64 } else if n == 4 { v as u32 as i64 } else { v as i64 })
        }
        0x2 => {
            let n = 1usize << low;
            let raw = b.get(p + 1..p + 1 + n).ok_or_else(bad)?;
            Plist::Real(if n == 4 { f32::from_be_bytes(raw.try_into().map_err(|_| bad())?) as f64 } else { f64::from_be_bytes(raw.try_into().map_err(|_| bad())?) })
        }
        0x3 => Plist::Real(f64::from_be_bytes(b.get(p + 1..p + 9).ok_or_else(bad)?.try_into().map_err(|_| bad())?)),
        0x4 => {
            let (n, s) = len_at(p)?;
            Plist::Data(b.get(s..s + n).ok_or_else(bad)?.to_vec())
        }
        0x5 => {
            let (n, s) = len_at(p)?;
            Plist::Str(String::from_utf8_lossy(b.get(s..s + n).ok_or_else(bad)?).into_owned())
        }
        0x6 => {
            let (n, s) = len_at(p)?;
            let raw = b.get(s..s + 2 * n).ok_or_else(bad)?;
            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            Plist::Str(String::from_utf16_lossy(&units))
        }
        0x8 => Plist::Uid(be(b.get(p + 1..p + 2 + low).ok_or_else(bad)?) as usize),
        0xA | 0xC | 0xD => {
            if seen[i] {
                return Err(bad());
            }
            seen[i] = true;
            let (n, s) = len_at(p)?;
            let refs = |k: usize| -> Result<usize, String> { Ok(be(b.get(s + k * ref_size..s + (k + 1) * ref_size).ok_or_else(bad)?) as usize) };
            let out = if kind == 0xD {
                // (Le nombre d'entrées vient du fichier : une réserve à sa mesure pouvait demander des téraoctets.)
                let mut items = Vec::with_capacity(n.min(4096));
                for k in 0..n {
                    let key = object(b, offsets, ref_size, refs(k)?, seen, depth + 1)?;
                    let val = object(b, offsets, ref_size, refs(n + k)?, seen, depth + 1)?;
                    items.push((key, val));
                }
                Plist::Dict(items)
            } else {
                Plist::Array((0..n).map(|k| object(b, offsets, ref_size, refs(k)?, seen, depth + 1)).collect::<Result<_, _>>()?)
            };
            seen[i] = false;
            out
        }
        _ => return Err(bad()),
    })
}

/// Les réglages d'une archive NSKeyedArchiver : l'objet racine (`$top` → `root`), avec chaque
/// valeur résolue dans `$objects` (nombres, textes ; `$null` : absent).
pub fn keyed_root(p: &Plist) -> Option<HashMap<String, Plist>> {
    let objects = match p.get("$objects")? {
        Plist::Array(a) => a,
        _ => return None,
    };
    let root = match p.get("$top")?.get("root")? {
        Plist::Uid(u) => objects.get(*u)?,
        other => other,
    };
    let Plist::Dict(items) = root else { return None };
    let resolve = |v: &Plist| -> Plist {
        match v {
            Plist::Uid(u) => match objects.get(*u) {
                Some(Plist::Str(s)) if s == "$null" => Plist::Null,
                Some(o) => o.clone(),
                None => Plist::Null,
            },
            o => o.clone(),
        }
    };
    Some(items.iter().filter_map(|(k, v)| match k {
        Plist::Str(k) => Some((k.clone(), resolve(v))),
        _ => None,
    }).collect())
}

/// Le réglage d'une brosse Procreate (échelles devinées, dites dans `notes`).
pub fn brush_settings(keys: &HashMap<String, Plist>, notes: &mut Vec<String>) -> BrushSettings {
    let num = |k: &str| keys.get(k).and_then(Plist::num).map(|v| v as f32);
    let mut s = BrushSettings { pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() };
    // Taille et opacité : la valeur réglée (0 à 1) dans les bornes de la brosse.
    let max = num("maxSize").unwrap_or(1.0).max(0.01);
    s.size = (num("paintSize").unwrap_or(0.1) * max * 100.0).clamp(0.5, 400.0);
    s.opacity = num("paintOpacity").unwrap_or(1.0).clamp(0.0, 1.0) * num("maxOpacity").unwrap_or(1.0).clamp(0.0, 1.0);
    if let Some(v) = num("plotSpacing") {
        s.spacing = v.clamp(0.02, 1.0);
    }
    s.scatter = num("plotJitter").unwrap_or(0.0).clamp(0.0, 4.0);
    s.scatter_along = num("plotJitterLongitudinal").unwrap_or(0.0).clamp(0.0, 4.0);
    s.angle_jitter = (num("shapeScatter").unwrap_or(0.0) * 360.0).clamp(0.0, 360.0);
    s.count = num("shapeCount").unwrap_or(1.0).round().clamp(1.0, 16.0) as u32;
    s.roundness = num("shapeRoundness").unwrap_or(1.0).clamp(0.05, 1.0);
    s.angle = num("shapeAngle").unwrap_or(0.0) * 360.0;
    if num("shapeRotation").unwrap_or(0.0).abs() >= 0.5 {
        s.dynamics.angle = Connection { source: Source::Direction, invert: num("shapeRotation").unwrap_or(0.0) < 0.0, ..Default::default() };
    }
    if let Some(v) = num("dynamicsPressureSize").filter(|v| *v > 0.0) {
        s.dynamics.size = Connection { source: Source::Pressure, min: (1.0 - v).clamp(0.0, 1.0), ..Default::default() };
    }
    if let Some(v) = num("dynamicsPressureOpacity").filter(|v| *v > 0.0) {
        s.dynamics.opacity = Connection { source: Source::Pressure, min: (1.0 - v).clamp(0.0, 1.0), ..Default::default() };
    }
    if let Some(v) = num("dynamicsSpeedSize").filter(|v| v.abs() > 0.01) {
        s.dynamics.size = Connection { source: Source::Speed, invert: v > 0.0, min: (1.0 - v.abs()).clamp(0.0, 1.0), ..Default::default() };
    }
    s.grain = num("grainDepth").unwrap_or(1.0).clamp(0.0, 1.0);
    s.paper_scale = num("textureScale").unwrap_or(1.0).clamp(0.1, 8.0);
    if num("textureApplication").unwrap_or(1.0) == 0.0 {
        notes.push("grain qui suit le trait (« Moving ») pris comme grain fixe".into());
    }
    if let Some(m) = num("dynamicsMix").filter(|m| *m > 0.0) {
        s.mix = m.clamp(0.0, 1.0);
        notes.push("mélange humide (Wet Mix) pris comme le mélange de FEANOR, à comparer".into());
    }
    if let Some(l) = num("dynamicsLoad").filter(|l| *l > 0.0 && *l < 1.0) {
        s.charge = l.clamp(0.0, 1.0);
    }
    if keys.get("dualBlendMode").is_some_and(|v| v.num().is_some_and(|n| n != 0.0)) {
        notes.push("double brosse non reprise".into());
    }
    s
}

/// Une image PNG en gris : la luminance (Procreate : le blanc dépose, pour la pointe ; le
/// relief, pour le grain).
fn gray(png: &[u8]) -> Option<GrayImage> {
    let img = image::load_from_memory(png).ok()?;
    let rgba = img.to_rgba8();
    let transparent = rgba.pixels().any(|p| p.0[3] < 250);
    let values = rgba.pixels().map(|p| if transparent { p.0[3] as f32 / 255.0 } else { (0.2126 * p.0[0] as f32 + 0.7152 * p.0[1] as f32 + 0.0722 * p.0[2] as f32) / 255.0 }).collect();
    let mut g = GrayImage::new(rgba.width() as usize, rgba.height() as usize, values);
    if g.width > 2048 || g.height > 2048 {
        g = GrayImage::from_image(&image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(g.width as u32, g.height as u32, |x, y| image::Luma([(g.get(x as usize, y as usize) * 255.0) as u8]))), 2048);
    }
    Some(g)
}

/// Importe une brosse (`.brush`) ou un jeu (`.brushset`) ; renvoie (brosses, pointes, papiers,
/// remarques).
pub fn import(bytes: &[u8], source: &str, lib: &mut BrushLibrary) -> Result<(usize, usize, usize, Vec<String>), String> {
    let zip = Zip::open(bytes)?;
    // Chaque brosse : le dossier de son Brush.archive (vide pour un .brush seul).
    let mut dirs: Vec<String> = zip.names().filter(|n| n.ends_with("Brush.archive") && !n.contains("Reset/") && !n.contains("Sub01/")).map(|n| n.trim_end_matches("Brush.archive").to_string()).collect();
    dirs.sort();
    if dirs.is_empty() {
        return Err("aucune brosse dans ce fichier".into());
    }
    let (mut n, mut tips, mut papers, mut notes) = (0, 0, 0, Vec::new());
    for dir in dirs {
        let Some(archive) = zip.read(&format!("{dir}Brush.archive")) else { continue };
        let keys = match read_bplist(&archive).ok().and_then(|p| keyed_root(&p)) {
            Some(k) => k,
            None => {
                notes.push(format!("{dir} : réglages illisibles"));
                continue;
            }
        };
        let name = match keys.get("name") {
            Some(Plist::Str(s)) if !s.is_empty() => s.clone(),
            _ => format!("Brosse importée {}", n + 1),
        };
        let mut brush_notes = Vec::new();
        let mut s = brush_settings(&keys, &mut brush_notes);
        match zip.read(&format!("{dir}Shape.png")).and_then(|b| gray(&b)) {
            Some(g) => {
                s.tip = Some(lib.add_tip(&name, g));
                tips += 1;
            }
            None if keys.get("bundledShapePath").is_some_and(|v| *v != Plist::Null) => brush_notes.push("pointe intégrée au logiciel d'origine, absente du fichier : pointe ronde".into()),
            None => {}
        }
        match zip.read(&format!("{dir}Grain.png")).and_then(|b| gray(&b)) {
            Some(g) => {
                s.paper = Some(lib.add_paper(&name, g));
                papers += 1;
            }
            None => s.grain = 0.0,
        }
        lib.presets.push(Preset { name: name.clone(), target: Target::Brush, settings: s, source: source.to_string() });
        n += 1;
        if !brush_notes.is_empty() {
            notes.push(format!("{name} : {}", brush_notes.join(", ")));
        }
    }
    if n > 0 {
        notes.insert(0, "Échelles de taille et d'espacement devinées (aucune documentation) : à comparer dans le logiciel d'origine.".into());
    }
    Ok((n, tips, papers, notes))
}
