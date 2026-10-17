//! Import des brosses Photoshop (`.abr`, moteur de dessin, étape 10), d'après la structure que
//! lisent GIMP (`gimpbrush-load.c`) et Krita (`kis_abr_brush_collection.cpp`) ; aucun fichier
//! d'essai n'était sur le poste : à essayer sur une vraie brosse.
//! - Versions 1 et 2 : `version`, `nombre`, puis chaque brosse (`type`, `taille`) ; type 2 =
//!   échantillonnée : espacement, nom (version 2), bornes, profondeur, compression (0 brute, 1
//!   par plages : longueurs des lignes puis PackBits). Le type 1 (calculée) n'est pas décrit.
//! - Versions 6 et plus : `version`, `sous-version`, puis des sections `8BIM` + clé + longueur.
//!   `samp` : les pointes (identifiant, préambule de 10 ou 264 octets selon la sous-version,
//!   bornes, profondeur, compression) ; `desc` : le descripteur des préréglages (nom, diamètre,
//!   dureté, angle, rondeur, espacement, pointe liée, appui sur la taille et l'opacité), lu au
//!   mieux.
//! - Les pointes : 255 où elles déposent.

use std::collections::HashMap;

use crate::brush::{BrushSettings, Connection, Source};
use crate::brushtex::{BrushLibrary, GrayImage, Preset, Target};

/// Lecteur gros-boutiste.
struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let v = u16::from_be_bytes(self.b.get(self.p..self.p + 2)?.try_into().ok()?);
        self.p += 2;
        Some(v)
    }
    fn u32(&mut self) -> Option<u32> {
        let v = u32::from_be_bytes(self.b.get(self.p..self.p + 4)?.try_into().ok()?);
        self.p += 4;
        Some(v)
    }
    fn f64(&mut self) -> Option<f64> {
        let v = f64::from_be_bytes(self.b.get(self.p..self.p + 8)?.try_into().ok()?);
        self.p += 8;
        Some(v)
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let v = self.b.get(self.p..self.p + n)?;
        self.p += n;
        Some(v)
    }
    /// Texte UTF-16 (longueur en caractères, sur 4 octets), sans le zéro final.
    fn ucs2(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        let raw = self.bytes(n * 2)?;
        let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        Some(String::from_utf16_lossy(&units).trim_end_matches('\0').to_string())
    }
    /// Une clé de descripteur : longueur (0 : quatre lettres), puis le texte.
    fn key(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        let n = if n == 0 { 4 } else { n };
        Some(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
}

/// Une pointe échantillonnée : sa taille et son dépôt (0 à 1).
fn sampled(r: &mut Reader) -> Option<GrayImage> {
    let (top, left, bottom, right) = (r.u32()? as i32, r.u32()? as i32, r.u32()? as i32, r.u32()? as i32);
    let depth = r.u16()?;
    let compress = r.u8()?;
    let (w, h) = ((right - left).max(0) as usize, (bottom - top).max(0) as usize);
    if w == 0 || h == 0 || w * h > 25_000_000 || !(depth == 8 || depth == 16) {
        return None;
    }
    let bpp = depth as usize / 8;
    let row = w * bpp;
    let mut data = vec![0u8; row * h];
    if compress == 0 {
        data.copy_from_slice(r.bytes(row * h)?);
    } else {
        // Les longueurs compressées des lignes, puis chaque ligne en PackBits.
        let lens: Vec<usize> = (0..h).map(|_| r.u16().map(|v| v as usize)).collect::<Option<_>>()?;
        for (y, len) in lens.into_iter().enumerate() {
            let src = r.bytes(len)?;
            let out = &mut data[y * row..(y + 1) * row];
            let (mut i, mut o) = (0, 0);
            while i < src.len() && o < out.len() {
                let n = src[i] as i8;
                i += 1;
                if n >= 0 {
                    let k = (n as usize + 1).min(out.len() - o).min(src.len() - i);
                    out[o..o + k].copy_from_slice(&src[i..i + k]);
                    i += n as usize + 1;
                    o += k;
                } else if n != -128 {
                    let k = ((1 - n as i32) as usize).min(out.len() - o);
                    let v = *src.get(i)?;
                    i += 1;
                    out[o..o + k].fill(v);
                    o += k;
                }
            }
        }
    }
    let values = if bpp == 1 { data.iter().map(|&v| v as f32 / 255.0).collect() } else { data.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0).collect() };
    Some(GrayImage::new(w, h, values))
}

/// Une brosse lue : nom, pointe (ou aucune : ronde), réglage.
pub struct AbrBrush {
    pub name: String,
    pub tip: Option<GrayImage>,
    pub settings: BrushSettings,
}

fn base_settings() -> BrushSettings {
    BrushSettings { pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() }
}

/// Versions 1 et 2.
fn read_v12(r: &mut Reader, version: u16, notes: &mut Vec<String>) -> Option<Vec<AbrBrush>> {
    let count = r.u16()?;
    let mut out = Vec::new();
    for k in 0..count {
        let kind = r.u16()?;
        let size = r.u32()? as usize;
        let next = r.p + size;
        if kind == 2 {
            r.u32()?;
            let spacing = r.u16()?;
            let name = if version == 2 { r.ucs2().unwrap_or_default() } else { String::new() };
            r.u8()?;
            r.bytes(8)?;
            if let Some(tip) = sampled(r) {
                let mut s = base_settings();
                s.size = tip.width.max(tip.height) as f32;
                s.spacing = (spacing as f32 / 100.0).clamp(0.02, 1.0);
                let name = if name.is_empty() { format!("Brosse {}", k + 1) } else { name };
                out.push(AbrBrush { name, tip: Some(tip), settings: s });
            }
        } else {
            notes.push(format!("brosse {} : type {kind} (calculée) non décrit, sautée", k + 1));
        }
        r.p = next;
    }
    Some(out)
}

/// Une valeur d'un descripteur Photoshop.
#[derive(Clone, Debug)]
pub enum Value {
    Num(f64),
    Unit(String, f64),
    Text(String),
    Bool(bool),
    Enum(String),
    List(Vec<Value>),
    Obj(HashMap<String, Value>),
    Other,
}

impl Value {
    fn obj(&self) -> Option<&HashMap<String, Value>> {
        if let Value::Obj(m) = self { Some(m) } else { None }
    }
    fn num(&self) -> Option<f64> {
        match self {
            Value::Num(v) | Value::Unit(_, v) => Some(*v),
            _ => None,
        }
    }
}

/// Un descripteur : nom, classe, puis ses éléments (clé, type, valeur).
fn descriptor(r: &mut Reader) -> Option<HashMap<String, Value>> {
    r.ucs2()?;
    r.key()?;
    let n = r.u32()?;
    let mut m = HashMap::new();
    for _ in 0..n {
        let k = r.key()?;
        let v = value(r)?;
        m.insert(k.trim().to_string(), v);
    }
    Some(m)
}

fn value(r: &mut Reader) -> Option<Value> {
    let ty = String::from_utf8_lossy(r.bytes(4)?).into_owned();
    Some(match ty.as_str() {
        "Objc" | "GlbO" => Value::Obj(descriptor(r)?),
        "VlLs" => {
            let n = r.u32()?;
            Value::List((0..n).map(|_| value(r)).collect::<Option<_>>()?)
        }
        "doub" => Value::Num(r.f64()?),
        "UntF" => {
            let unit = String::from_utf8_lossy(r.bytes(4)?).into_owned();
            Value::Unit(unit, r.f64()?)
        }
        "long" => Value::Num(r.u32()? as i32 as f64),
        "bool" => Value::Bool(r.u8()? != 0),
        "TEXT" => Value::Text(r.ucs2()?),
        "enum" => {
            r.key()?;
            Value::Enum(r.key()?)
        }
        "type" | "GlbC" => {
            r.ucs2()?;
            r.key()?;
            Value::Other
        }
        "tdta" => {
            let n = r.u32()? as usize;
            r.bytes(n)?;
            Value::Other
        }
        _ => return None,
    })
}

/// La connexion d'une dynamique de Photoshop (objet `brVr` : `bVTy` 2 = pression, 1 = fondu,
/// d'après les lecteurs libres ; `Mnm ` : le minimum en %).
fn dynamic(v: Option<&Value>) -> Option<Connection> {
    let m = v?.obj()?;
    let kind = m.get("bVTy")?.num()? as i32;
    let source = match kind {
        2 => Source::Pressure,
        1 => Source::Fade,
        _ => return None,
    };
    let min = m.get("Mnm").and_then(Value::num).unwrap_or(0.0) as f32 / 100.0;
    Some(Connection { source, min: min.clamp(0.0, 1.0), ..Default::default() })
}

/// Versions 6 et plus : pointes (`samp`) et préréglages (`desc`).
fn read_v6(r: &mut Reader, sub: u16, notes: &mut Vec<String>) -> Option<Vec<AbrBrush>> {
    let mut tips: Vec<(String, GrayImage)> = Vec::new();
    let mut presets: Option<HashMap<String, Value>> = None;
    while r.p + 12 <= r.b.len() {
        if r.bytes(4)? != b"8BIM" {
            break;
        }
        let key = String::from_utf8_lossy(r.bytes(4)?).into_owned();
        let len = r.u32()? as usize;
        let end = r.p + len;
        match key.as_str() {
            "samp" => {
                while r.p + 4 < end {
                    let size = r.u32()? as usize;
                    let next = r.p + size.div_ceil(4) * 4;
                    let id_len = r.u8()? as usize;
                    let id = String::from_utf8_lossy(r.bytes(id_len)?).into_owned();
                    // Préambule : le reste des 37 octets de l'identifiant, puis 10 ou 264 octets.
                    r.p += 36usize.saturating_sub(id_len) + if sub == 1 { 10 } else { 264 };
                    if let Some(t) = sampled(r) {
                        tips.push((id, t));
                    }
                    r.p = next;
                }
            }
            "desc" => {
                r.u32()?;
                presets = descriptor(r);
            }
            _ => {}
        }
        r.p = end.div_ceil(2) * 2;
    }
    let mut out = Vec::new();
    let list = presets.as_ref().and_then(|p| match p.get("Brsh") {
        Some(Value::List(l)) => Some(l.clone()),
        _ => None,
    });
    let Some(list) = list else {
        // Sans préréglages : une brosse par pointe.
        for (k, (_, t)) in tips.into_iter().enumerate() {
            let mut s = base_settings();
            s.size = t.width.max(t.height) as f32;
            out.push(AbrBrush { name: format!("Brosse {}", k + 1), tip: Some(t), settings: s });
        }
        return Some(out);
    };
    let mut used = vec![false; tips.len()];
    for (k, item) in list.iter().enumerate() {
        let Some(p) = item.obj() else { continue };
        let name = match p.get("Nm") {
            Some(Value::Text(t)) => t.clone(),
            _ => format!("Brosse {}", k + 1),
        };
        let mut s = base_settings();
        let brush = p.get("Brsh").and_then(Value::obj);
        let get = |k: &str| brush.and_then(|b| b.get(k)).and_then(Value::num);
        let mut tip = None;
        if let Some(Value::Text(id)) = brush.and_then(|b| b.get("sampledData")) {
            if let Some(i) = tips.iter().position(|(t, _)| t == id || t.trim_start_matches('$') == id.as_str()) {
                used[i] = true;
                tip = Some(tips[i].1.clone());
            }
        }
        s.size = get("Dmtr").map(|v| v as f32).or(tip.as_ref().map(|t| t.width.max(t.height) as f32)).unwrap_or(20.0).clamp(0.5, 400.0);
        s.hardness = get("Hrdn").map_or(1.0, |v| (v / 100.0) as f32).clamp(0.0, 1.0);
        s.angle = get("Angl").unwrap_or(0.0) as f32;
        s.roundness = get("Rndn").map_or(1.0, |v| (v / 100.0) as f32).clamp(0.05, 1.0);
        s.spacing = get("Spcn").map_or(0.25, |v| (v / 100.0) as f32).clamp(0.02, 1.0);
        if let Some(c) = dynamic(p.get("szVr")) {
            s.dynamics.size = c;
        }
        if let Some(c) = dynamic(p.get("opVr")) {
            s.dynamics.opacity = c;
        }
        if matches!(p.get("useScatter"), Some(Value::Bool(true))) {
            notes.push(format!("{name} : dispersion non reprise"));
        }
        if matches!(p.get("useTexture"), Some(Value::Bool(true))) {
            notes.push(format!("{name} : texture non reprise"));
        }
        if matches!(p.get("useDualBrush"), Some(Value::Bool(true))) {
            notes.push(format!("{name} : double pointe non reprise"));
        }
        out.push(AbrBrush { name, tip, settings: s });
    }
    let unused = used.iter().filter(|u| !**u).count();
    if unused > 0 {
        notes.push(format!("{unused} pointe(s) sans préréglage, ignorée(s)"));
    }
    Some(out)
}

/// Les brosses d'un `.abr` ; `notes` reçoit ce qui n'est pas repris.
pub fn read(bytes: &[u8], notes: &mut Vec<String>) -> Result<Vec<AbrBrush>, String> {
    let mut r = Reader { b: bytes, p: 0 };
    let version = r.u16().ok_or("fichier vide")?;
    let list = match version {
        1 | 2 => read_v12(&mut r, version, notes),
        6..=10 => {
            let sub = r.u16().ok_or("fichier tronqué")?;
            read_v6(&mut r, sub, notes)
        }
        v => return Err(format!("version {v} de .abr non lue")),
    };
    list.ok_or_else(|| "brosse Photoshop illisible (fichier tronqué ou d'une forme inconnue)".into())
}

/// Importe un `.abr` dans la bibliothèque. Renvoie (brosses, pointes, remarques).
pub fn import(bytes: &[u8], source: &str, lib: &mut BrushLibrary) -> Result<(usize, usize, Vec<String>), String> {
    let mut notes = Vec::new();
    let brushes = read(bytes, &mut notes)?;
    let mut tips = 0;
    let n = brushes.len();
    for mut b in brushes {
        if let Some(t) = b.tip.take() {
            b.settings.tip = Some(lib.add_tip(&b.name, t));
            tips += 1;
        }
        lib.presets.push(Preset { name: b.name, target: Target::Brush, settings: b.settings, source: source.to_string() });
    }
    Ok((n, tips, notes))
}
