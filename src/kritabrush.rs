//! Import des brosses Krita (`.kpp`) et MyPaint (`.myb`) (moteur de dessin, étape 10). Formats
//! ouverts, lus d'après leurs fichiers et le code publié de Krita et de libmypaint :
//! - `.kpp` : une image PNG dont le bloc de texte compressé `preset` porte le réglage en XML
//!   (`<Preset name paintopid><param name>…</param>`). Seul le moteur « paintbrush » (pinceau à
//!   empreintes) est repris. La pointe (`brush_definition`) : ronde (`auto_brush` : diamètre,
//!   rapport, fondu, espacement, angle) ou en image (`.gbr` / `.png`, cherchée dans le réglage
//!   puis dans le dossier `brushes` de Krita). Une option est allumée par `Pressure<Nom>` ; sa
//!   source et sa courbe sont dans `<Nom>Sensor` (`<params id="pressure"><curve>x,y;…</curve>`).
//! - `.myb` : JSON (`settings` → réglage → `base_value`, `inputs` → source → points). Repris :
//!   rayon (logarithmique), opacité, dureté, empreintes par rayon, ellipse, dispersion, et l'appui
//!   sur la taille et l'opacité.
//!
//! Devinés, dits dans le compte rendu : le fondu de Krita pris comme la dureté (1 = net) ; la
//! dispersion de Krita en diamètres.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::brush::{BrushSettings, Connection, Curve, Source};
use crate::brushtex::{BrushLibrary, GrayImage, Preset, Target};

/// Ce que l'import a fait.
#[derive(Debug, Default)]
pub struct KritaImport {
    pub presets: usize,
    pub tips: usize,
    pub notes: Vec<String>,
}

/// Le texte du bloc `preset` d'un `.kpp` (bloc PNG `zTXt`, `iTXt` ou `tEXt`).
pub fn kpp_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("ce n'est pas un réglage de brosse .kpp (PNG)".into());
    }
    let mut i = 8;
    while i + 12 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let kind = &bytes[i + 4..i + 8];
        let data = bytes.get(i + 8..i + 8 + len).ok_or("PNG tronqué")?;
        let key_end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        if &data[..key_end] == b"preset" {
            let inflate = |z: &[u8]| -> Result<String, String> {
                let mut s = String::new();
                flate2::read::ZlibDecoder::new(z).read_to_string(&mut s).map_err(|e| format!("réglage illisible ({e})"))?;
                Ok(s)
            };
            match kind {
                b"zTXt" => return inflate(data.get(key_end + 2..).ok_or("bloc tronqué")?),
                b"tEXt" => return Ok(String::from_utf8_lossy(&data[key_end + 1..]).into_owned()),
                b"iTXt" => {
                    // mot-clé \0, compressé (1 octet), méthode (1), langue \0, mot traduit \0, texte.
                    let flag = *data.get(key_end + 1).unwrap_or(&0);
                    let mut p = key_end + 3;
                    for _ in 0..2 {
                        p += data[p..].iter().position(|&b| b == 0).ok_or("bloc tronqué")? + 1;
                    }
                    return if flag == 1 { inflate(&data[p..]) } else { Ok(String::from_utf8_lossy(&data[p..]).into_owned()) };
                }
                _ => {}
            }
        }
        i += 12 + len;
    }
    Err("pas de réglage dans ce fichier".into())
}

/// Le nom, le moteur et les paramètres d'un réglage Krita (XML), plus ses ressources jointes
/// (nom de fichier → contenu).
pub struct KritaPreset {
    pub name: String,
    pub paintop: String,
    pub params: HashMap<String, String>,
    pub resources: HashMap<String, Vec<u8>>,
}

pub fn parse_preset(xml: &str) -> Result<KritaPreset, String> {
    let root = crate::xml::parse(xml)?;
    let name = root.attr("name").unwrap_or("Brosse importée").to_string();
    let paintop = root.attr("paintopid").unwrap_or("").to_string();
    let params = root.children_named("param").filter_map(|p| Some((p.attr("name")?.to_string(), p.text.clone()))).collect();
    let mut resources = HashMap::new();
    let mut found = Vec::new();
    root.find_all("resource", &mut found);
    for r in found {
        if let (Some(file), Some(bytes)) = (r.attr("filename").or(r.attr("name")), base64(r.text.trim())) {
            resources.insert(file.to_string(), bytes);
        }
    }
    Ok(KritaPreset { name, paintop, params, resources })
}

/// Décodage base64 (ressources jointes à un réglage Krita 5).
pub fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    if clean.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut n = 0u32;
        for (k, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * k);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len().saturating_sub(1)]);
    }
    Some(out)
}

/// La source et la courbe d'un capteur Krita (`<params id="pressure"><curve>0,0;1,1;</curve>`,
/// ou une liste de capteurs, dont on prend le premier). Aucune : source que Pelure ne lit pas
/// (son nom dans `skipped`).
pub fn sensor(xml: &str, skipped: &mut Vec<String>) -> Option<Connection> {
    let root = crate::xml::parse(xml).ok()?;
    let node = if root.attr("id") == Some("sensorslist") { root.children.iter().find(|c| c.attr("id").is_some())?.clone() } else { root };
    let id = node.attr("id")?;
    let source = match id {
        "pressure" | "pressurein" => Source::Pressure,
        "speed" => Source::Speed,
        "drawingangle" => Source::Direction,
        "fade" | "distance" => Source::Fade,
        "fuzzy" | "fuzzystroke" => Source::Random,
        other => {
            skipped.push(other.to_string());
            return None;
        }
    };
    let curve = node
        .child("curve")
        .map(|c| c.text.split(';').filter_map(|p| p.split_once(',')).filter_map(|(x, y)| Some([x.trim().parse().ok()?, y.trim().parse().ok()?])).collect::<Vec<[f32; 2]>>())
        .filter(|v| v.len() >= 2)
        .map(|v| Curve::from_points(&v))
        .unwrap_or_default();
    Some(Connection { source, curve, ..Default::default() })
}

/// Une pointe `.gbr` de GIMP : en-tête (taille, version, largeur, hauteur, octets par pixel,
/// « GIMP », espacement), nom, pixels ; en gris de dépôt (1 où elle dépose).
pub fn read_gbr(b: &[u8]) -> Option<GrayImage> {
    let u = |i: usize| -> Option<usize> { Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?) as usize) };
    let (header, w, h, bpp) = (u(0)?, u(8)?, u(12)?, u(16)?);
    if w == 0 || h == 0 || !(bpp == 1 || bpp == 4) || w * h > 16_000_000 {
        return None;
    }
    let px = b.get(header..header + w * h * bpp)?;
    let values: Vec<f32> = if bpp == 4 {
        px.chunks_exact(4).map(|p| p[3] as f32 / 255.0).collect()
    } else {
        px.iter().map(|&v| v as f32 / 255.0).collect()
    };
    let mut g = GrayImage::new(w, h, values);
    // Un fond clair (bords à 1) : la brosse est en noir sur blanc, on retourne.
    if bpp == 1 {
        let border: Vec<f32> = (0..w).flat_map(|x| [g.get(x, 0), g.get(x, h - 1)]).collect();
        if border.iter().sum::<f32>() / border.len() as f32 > 0.5 {
            g.values.iter_mut().for_each(|v| *v = 1.0 - *v);
        }
    }
    Some(g)
}

/// Le réglage d'un préréglage Krita (« paintbrush ») ; `tip` : la pointe en image trouvée.
pub fn krita_settings(p: &KritaPreset, tip: Option<&GrayImage>, notes: &mut Vec<String>) -> BrushSettings {
    let get = |k: &str| p.params.get(k).map(String::as_str);
    let on = |k: &str| get(k) == Some("true");
    let num = |k: &str, d: f32| get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d);
    let mut s = BrushSettings { pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() };
    if let Some(def) = get("brush_definition").and_then(|d| crate::xml::parse(d).ok()) {
        let a = |k: &str| def.attr(k).and_then(|v| v.parse::<f32>().ok());
        s.spacing = a("spacing").unwrap_or(0.1).clamp(0.02, 1.0);
        s.angle = a("angle").unwrap_or(0.0).to_degrees();
        match def.attr("type") {
            Some("auto_brush") => {
                if let Some(m) = def.child("MaskGenerator") {
                    let ma = |k: &str| m.attr(k).and_then(|v| v.parse::<f32>().ok());
                    s.size = ma("diameter").unwrap_or(10.0).clamp(0.5, 400.0);
                    s.roundness = ma("ratio").unwrap_or(1.0).clamp(0.05, 1.0);
                    s.hardness = ma("hfade").unwrap_or(1.0).clamp(0.0, 1.0);
                    if m.attr("type") == Some("rect") {
                        notes.push("pointe carrée prise ronde".into());
                    }
                }
            }
            Some(_) => {
                let scale = a("scale").unwrap_or(1.0);
                s.size = tip.map_or(20.0, |t| t.width.max(t.height) as f32 * scale).clamp(0.5, 400.0);
                if tip.is_none() {
                    notes.push(format!("pointe en image « {} » introuvable : pointe ronde", def.attr("filename").unwrap_or("?")));
                }
            }
            None => {}
        }
    }
    s.opacity = num("OpacityValue", 1.0).clamp(0.0, 1.0);
    // Le Flow de Krita est le dépôt de chaque empreinte, qui s'accumule dans le trait sous le
    // plafond de l'opacité (rendu des brosses, étape 2).
    s.power = num("FlowValue", 1.0).clamp(0.01, 1.0);
    s.build_up = s.power < 1.0;
    let mut skipped = Vec::new();
    if on("PressureSize") {
        if let Some(c) = get("SizeSensor").and_then(|x| sensor(x, &mut skipped)) {
            s.dynamics.size = c;
        }
    }
    if on("PressureOpacity") {
        if let Some(c) = get("OpacitySensor").and_then(|x| sensor(x, &mut skipped)) {
            s.dynamics.opacity = c;
        }
    } else if on("PressureFlow") {
        if let Some(c) = get("FlowSensor").and_then(|x| sensor(x, &mut skipped)) {
            s.dynamics.opacity = c;
            notes.push("débit selon une source pris comme l'opacité".into());
        }
    }
    if on("PressureRotation") {
        if let Some(c) = get("RotationSensor").and_then(|x| sensor(x, &mut skipped)) {
            s.dynamics.angle = c;
        }
    }
    // L'option « Mix » de Krita mêle la couleur d'avant-plan et celle d'arrière-plan.
    if on("PressureMix") {
        if let Some(c) = get("MixSensor").and_then(|x| sensor(x, &mut skipped)) {
            s.dynamics.color = c;
        }
    }
    if on("PressureScatter") {
        s.scatter = num("ScatterValue", 0.0).clamp(0.0, 4.0);
    }
    for k in skipped {
        notes.push(format!("capteur « {k} » non repris"));
    }
    if on("Texture/Pattern/Enabled") {
        notes.push("texture (motif) non reprise".into());
    }
    if on("MaskingBrush/Enabled") {
        notes.push("double pointe non reprise".into());
    }
    s
}

/// Importe un `.kpp` ; `brushes_dir` : le dossier `brushes` de Krita, pour les pointes en image.
pub fn import_kpp(bytes: &[u8], source: &str, lib: &mut BrushLibrary, brushes_dir: Option<&Path>) -> Result<KritaImport, String> {
    import_kpp_with(bytes, source, lib, &|f| brushes_dir.and_then(|d| std::fs::read(d.join(f)).ok()))
}

/// Importe tous les réglages d'un paquet Krita (`.bundle`, une archive zip) : leurs pointes en
/// image viennent du paquet.
pub fn import_bundle(bytes: &[u8], source: &str, lib: &mut BrushLibrary) -> Result<KritaImport, String> {
    let zip = crate::zipread::Zip::open(bytes)?;
    let mut names: Vec<String> = zip.names().filter(|n| n.starts_with("paintoppresets/") && n.to_lowercase().ends_with(".kpp")).map(String::from).collect();
    names.sort();
    let mut report = KritaImport::default();
    for n in names {
        let Some(b) = zip.read(&n) else { continue };
        match import_kpp_with(&b, source, lib, &|f| zip.find(f)) {
            Ok(r) => {
                report.presets += r.presets;
                report.tips += r.tips;
                report.notes.extend(r.notes);
            }
            Err(e) => report.notes.push(format!("{n} : {e}")),
        }
    }
    Ok(report)
}

/// Importe un `.kpp` ; `find_tip` donne le contenu d'une pointe en image d'après son nom de
/// fichier (dossier des brosses, paquets).
pub fn import_kpp_with(bytes: &[u8], source: &str, lib: &mut BrushLibrary, find_tip: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<KritaImport, String> {
    let p = parse_preset(&kpp_text(bytes)?)?;
    let mut report = KritaImport::default();
    if p.paintop != "paintbrush" {
        report.notes.push(format!("{} : moteur « {} » non repris", p.name, p.paintop));
        return Ok(report);
    }
    // La pointe en image : jointe au réglage, sinon dans le dossier des brosses de Krita.
    let tip_file = p.params.get("brush_definition").and_then(|d| crate::xml::parse(d).ok()).and_then(|d| d.attr("filename").map(String::from));
    let tip = tip_file.as_ref().and_then(|f| {
        let bytes = p.resources.get(f).cloned().or_else(|| find_tip(f))?;
        if f.to_lowercase().ends_with(".gbr") {
            read_gbr(&bytes)
        } else {
            image::load_from_memory(&bytes).ok().map(|i| GrayImage::tip_from_image(&i, 2048))
        }
    });
    let mut notes = Vec::new();
    let mut s = krita_settings(&p, tip.as_ref(), &mut notes);
    if let Some(t) = tip {
        s.tip = Some(lib.add_tip(&p.name, t));
        report.tips += 1;
    }
    let target = if p.params.get("EraserMode").map(String::as_str) == Some("true") { Target::Eraser } else { Target::Brush };
    lib.presets.push(Preset { name: p.name.clone(), target, settings: s, source: source.to_string() });
    report.presets += 1;
    if !notes.is_empty() {
        report.notes.push(format!("{} : {}", p.name, notes.join(", ")));
    }
    Ok(report)
}

/// Un réglage MyPaint (`.myb`).
pub fn myb_settings(json: &serde_json::Value, notes: &mut Vec<String>) -> Result<BrushSettings, String> {
    let set = json.get("settings").ok_or("pas de réglages de brosse .myb")?;
    let base = |k: &str, d: f64| set.get(k).and_then(|v| v.get("base_value")).and_then(|v| v.as_f64()).unwrap_or(d) as f32;
    let input = |k: &str, src: &str| -> Option<Vec<[f32; 2]>> {
        let pts = set.get(k)?.get("inputs")?.get(src)?.as_array()?;
        let v: Vec<[f32; 2]> = pts.iter().filter_map(|p| Some([p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32])).collect();
        (v.len() >= 2).then_some(v)
    };
    let mut s = BrushSettings { pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() };
    let log_r = base("radius_logarithmic", 2.0);
    // La taille suit l'appui : l'entrée s'ajoute au logarithme du rayon.
    let factor = |pts: &[[f32; 2]], x: f32| -> f32 {
        let c = pts.windows(2).find(|w| x <= w[1][0]).unwrap_or(&pts[pts.len() - 2..]);
        let t = if c[1][0] > c[0][0] { ((x - c[0][0]) / (c[1][0] - c[0][0])).clamp(0.0, 1.0) } else { 1.0 };
        (c[0][1] + (c[1][1] - c[0][1]) * t).exp()
    };
    match input("radius_logarithmic", "pressure") {
        Some(pts) => {
            let xs: Vec<f32> = (0..Curve::MAX).map(|k| k as f32 / (Curve::MAX - 1) as f32).collect();
            let f: Vec<f32> = xs.iter().map(|&x| factor(&pts, x)).collect();
            let max = f.iter().cloned().fold(f32::MIN, f32::max).max(1e-3);
            s.size = (2.0 * log_r.exp() * max).clamp(0.5, 400.0);
            s.dynamics.size = Connection { source: Source::Pressure, curve: Curve::from_points(&xs.iter().zip(&f).map(|(&x, &v)| [x, v / max]).collect::<Vec<_>>()), ..Default::default() };
        }
        None => s.size = (2.0 * log_r.exp()).clamp(0.5, 400.0),
    }
    // `opaque` est le dépôt de chaque empreinte, composée par-dessus les autres (libmypaint) : un
    // débit qui s'accumule dans le trait, pas un plafond (rendu des brosses, étape 2 : pris comme
    // plafond, le Pencil 2B importé était presque invisible).
    let opaque = base("opaque", 1.0).clamp(0.01, 1.0);
    s.opacity = 1.0;
    s.power = opaque;
    s.build_up = opaque < 1.0;
    if let Some(pts) = input("opaque_multiply", "pressure").or_else(|| input("opaque", "pressure")) {
        let max = pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max).max(1e-3);
        s.dynamics.opacity = Connection { source: Source::Pressure, curve: Curve::from_points(&pts.iter().map(|p| [p[0], (p[1] / max).clamp(0.0, 1.0)]).collect::<Vec<_>>()), ..Default::default() };
    }
    s.hardness = base("hardness", 0.8).clamp(0.0, 1.0);
    let per_radius = base("dabs_per_actual_radius", 2.0).max(0.1);
    s.spacing = (1.0 / (2.0 * per_radius)).clamp(0.02, 1.0);
    let ratio = base("elliptical_dab_ratio", 1.0).max(1.0);
    s.roundness = (1.0 / ratio).clamp(0.05, 1.0);
    s.angle = base("elliptical_dab_angle", 90.0) - 90.0;
    s.scatter = (base("offset_by_random", 0.0) / 2.0).clamp(0.0, 4.0);
    if base("smudge", 0.0) > 0.0 {
        s.mix = base("smudge", 0.0).clamp(0.0, 1.0);
        notes.push("estompe (smudge) prise comme le mélange de SHOT, à comparer".into());
    }
    if input("hardness", "pressure").is_some() {
        notes.push("dureté selon l'appui non reprise".into());
    }
    for k in ["speed1", "speed2", "random", "tilt_declination", "tilt_ascension", "direction"] {
        if ["radius_logarithmic", "opaque", "opaque_multiply"].iter().any(|s| input(s, k).is_some()) {
            notes.push(format!("entrée « {k} » non reprise"));
        }
    }
    Ok(s)
}

/// Importe un `.myb` (nom : celui du fichier).
pub fn import_myb(text: &str, name: &str, source: &str, lib: &mut BrushLibrary) -> Result<KritaImport, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("réglage .myb illisible ({e})"))?;
    let mut notes = Vec::new();
    let s = myb_settings(&json, &mut notes)?;
    let eraser = json.get("settings").and_then(|v| v.get("eraser")).and_then(|v| v.get("base_value")).and_then(|v| v.as_f64()).unwrap_or(0.0) > 0.5;
    lib.presets.push(Preset { name: name.to_string(), target: if eraser { Target::Eraser } else { Target::Brush }, settings: s, source: source.to_string() });
    let mut report = KritaImport { presets: 1, ..Default::default() };
    if !notes.is_empty() {
        report.notes.push(format!("{name} : {}", notes.join(", ")));
    }
    Ok(report)
}
