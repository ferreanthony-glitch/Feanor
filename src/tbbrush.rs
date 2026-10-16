//! Import des pinceaux d'un fichier de préréglages Toon Boom (Storyboard Pro, Harmony) :
//! XML `<penstyles>` avec des `<pen>` (réglages) et des `<texture>` (images de pointes et de
//! papiers, en hexadécimal). Le rendu reste celui de Pelure (visé : TVPaint) ; seuls les réglages
//! sont repris, avec quelques hypothèses sur les échelles, notées ici :
//! - taille : `maxValue` en pixels ; `minPercentage` = taille quand on effleure, en % ;
//! - dureté : lue sur une échelle de 0 à 5 (valeurs rencontrées : 2,47 et 5) — non vérifié ;
//! - espacement en % du diamètre ; lissage en % ;
//! - opacité et débit : leurs maximums se multiplient, de même leurs minimums sous la pression ;
//! - grain : le papier du pinceau, à pleine force ;
//! - un pinceau dont le nom contient « eraser » ou « gomme » va à la gomme.

use std::collections::HashMap;

use crate::brush::BrushSettings;
use crate::brushtex::{BrushLibrary, GrayImage, Preset, Target};

// ----------------------------------------------------------------------------- lecture XML

/// Élément XML : nom, attributs, enfants (le texte entre les balises n'est pas gardé).
#[derive(Debug, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
}

impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Descendant par un chemin de noms : `"a/b/c"`.
    pub fn path(&self, path: &str) -> Option<&Element> {
        path.split('/').try_fold(self, |e, name| e.children.iter().find(|c| c.name == name))
    }

    /// Attribut `value` au bout d'un chemin (la forme des fichiers Toon Boom).
    pub fn value(&self, path: &str) -> Option<&str> {
        self.path(path)?.attr("value")
    }

    pub fn num(&self, path: &str) -> Option<f32> {
        self.value(path)?.trim().parse().ok()
    }

    pub fn flag(&self, path: &str) -> Option<bool> {
        match self.value(path)? {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Lit un document XML simple (balises, attributs entre guillemets) ; ignore le prologue, le
/// DOCTYPE, les commentaires et le texte.
pub fn parse_xml(text: &str) -> Result<Element, String> {
    let bytes = text.as_bytes();
    let mut stack: Vec<Element> = vec![Element::default()];
    let mut i = 0;
    while let Some(off) = text[i..].find('<') {
        i += off;
        let rest = &text[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->").ok_or("commentaire non fermé")? + 3;
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            i += rest.find('>').ok_or("balise non fermée")? + 1;
            continue;
        }
        if let Some(close) = rest.strip_prefix("</") {
            let end = close.find('>').ok_or("balise fermante non fermée")?;
            let name = close[..end].trim();
            let done = stack.pop().ok_or("balise fermante en trop")?;
            if done.name != name {
                return Err(format!("balise </{name}> inattendue (ouverte : <{}>)", done.name));
            }
            stack.last_mut().ok_or("balise fermante en trop")?.children.push(done);
            i += 2 + end + 1;
            continue;
        }
        // Balise ouvrante : nom, attributs, fin « > » ou « /> ».
        let mut j = i + 1;
        let name_end = text[j..].find(|c: char| c.is_whitespace() || c == '>' || c == '/').ok_or("balise non fermée")? + j;
        let mut el = Element {
            name: text[j..name_end].to_string(),
            ..Default::default()
        };
        j = name_end;
        loop {
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            match bytes.get(j) {
                None => return Err(format!("balise <{}> non fermée", el.name)),
                Some(b'>') => {
                    stack.push(el);
                    j += 1;
                    break;
                }
                Some(b'/') => {
                    if bytes.get(j + 1) != Some(&b'>') {
                        return Err(format!("« / » inattendu dans <{}>", el.name));
                    }
                    stack.last_mut().ok_or("document vide")?.children.push(el);
                    j += 2;
                    break;
                }
                Some(_) => {
                    let eq = text[j..].find('=').ok_or("attribut sans valeur")? + j;
                    let key = text[j..eq].trim().to_string();
                    let mut k = eq + 1;
                    while k < bytes.len() && (bytes[k] as char).is_whitespace() {
                        k += 1;
                    }
                    let quote = *bytes.get(k).ok_or("attribut sans valeur")?;
                    if quote != b'"' && quote != b'\'' {
                        return Err(format!("valeur sans guillemets pour « {key} »"));
                    }
                    let end = text[k + 1..].find(quote as char).ok_or("guillemets non fermés")? + k + 1;
                    el.attrs.push((key, unescape(&text[k + 1..end])));
                    j = end + 1;
                }
            }
        }
        i = j;
    }
    if stack.len() != 1 {
        return Err(format!("balise <{}> non fermée", stack.last().map_or("", |e| e.name.as_str())));
    }
    let root = stack.pop().expect("racine");
    root.children.into_iter().next().ok_or_else(|| "document vide".to_string())
}

// ------------------------------------------------------------------------------- textures

/// Image d'un `bundledFile` : hexadécimal, en-tête de 18 octets (taille des données, largeur,
/// hauteur, bits par pixel sur 4 octets chacun, puis 2 octets), puis les lignes de pixels gris,
/// chacune complétée à un multiple de 4 octets. Valeurs rendues de 0 (noir) à 1 (blanc).
pub fn decode_bundled(hex: &str) -> Result<GrayImage, String> {
    let hex = hex.trim().as_bytes();
    if hex.len() % 2 != 0 {
        return Err("données hexadécimales de longueur impaire".into());
    }
    let digit = |c: u8| -> Result<u8, String> {
        (c as char).to_digit(16).map(|d| d as u8).ok_or_else(|| format!("caractère « {} » inattendu", c as char))
    };
    let bytes = hex
        .chunks_exact(2)
        .map(|p| Ok(digit(p[0])? << 4 | digit(p[1])?))
        .collect::<Result<Vec<u8>, String>>()?;
    if bytes.len() < 18 {
        return Err("texture trop courte".into());
    }
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("4 octets")) as usize;
    let (len, w, h, bpp) = (u32_at(0), u32_at(4), u32_at(8), u32_at(12));
    if bpp != 8 {
        return Err(format!("texture en {bpp} bits par pixel : seules les textures en gris (8 bits) sont lues"));
    }
    let stride = (w + 3) & !3;
    if w == 0 || h == 0 || len != stride * h || bytes.len() < 18 + len {
        return Err(format!("texture {w}×{h} incohérente ({len} octets annoncés)"));
    }
    let mut values = Vec::with_capacity(w * h);
    for y in 0..h {
        let row = &bytes[18 + y * stride..18 + y * stride + w];
        values.extend(row.iter().map(|&v| v as f32 / 255.0));
    }
    Ok(GrayImage::new(w, h, values))
}

// ------------------------------------------------------------------------------- pinceaux

/// Réglage Pelure d'un `<pen>`, avec les pointes et papiers déjà importés (numéros Toon Boom →
/// numéros Pelure).
pub fn pen_to_preset(pen: &Element, tips: &HashMap<String, u32>, papers: &HashMap<String, u32>, source: &str) -> Preset {
    let name = pen.value("name").unwrap_or("Pinceau").trim().to_string();
    let shape = "mainBrushShape";
    let pressure = |range: &str| pen.flag(&format!("{range}/valueRange/stylusFunction/unitaryFunction/enabled")).unwrap_or(false);
    let size_range = format!("{shape}/sizeRange");
    let (op_pressure, flow_pressure) = (pressure("opacityRange"), pressure("flowRange"));
    let pct = |path: &str, default: f32| pen.num(path).unwrap_or(default) / 100.0;
    let opacity = pct("opacityRange/maxValue", 100.0) * pct("flowRange/maxValue", 100.0);
    let at_rest = |p: bool, range: &str| if p { pct(&format!("{range}/minPercentage"), 0.0) } else { 1.0 };
    let tip = (pen.value("style") == Some("texture"))
        .then(|| pen.value(&format!("{shape}/brushTextureId")))
        .flatten()
        .and_then(|id| tips.get(id).copied());
    let paper = (pen.flag("paperTextureEnabled") == Some(true))
        .then(|| pen.value("paperTextureId"))
        .flatten()
        .and_then(|id| papers.get(id).copied());
    let lower = name.to_lowercase();
    let target = if lower.contains("eraser") || lower.contains("gomme") { Target::Eraser } else { Target::Brush };
    let settings = BrushSettings {
        size: pen.num(&format!("{size_range}/maxValue")).unwrap_or(6.0).clamp(0.5, 400.0),
        opacity: opacity.clamp(0.0, 1.0),
        power: 1.0,
        build_up: false,
        profile: None,
        hardness: (pen.num(&format!("{shape}/hardness")).unwrap_or(5.0) / 5.0).clamp(0.0, 1.0),
        spacing: pct(&format!("{shape}/spacing"), 8.0).clamp(0.02, 1.0),
        smoothing: pct("smooth", 30.0).clamp(0.0, 0.9),
        pressure_size: pressure(&size_range),
        pressure_opacity: op_pressure || flow_pressure,
        aliased: pen.flag("antialiasingEnabled") == Some(false),
        min_size: pct(&format!("{size_range}/minPercentage"), 0.0).clamp(0.0, 1.0),
        min_opacity: (at_rest(op_pressure, "opacityRange") * at_rest(flow_pressure, "flowRange")).clamp(0.0, 1.0),
        grain: if paper.is_some() { 1.0 } else { 0.0 },
        paper,
        tip,
        roundness: pct(&format!("{shape}/roundness"), 100.0).clamp(0.05, 1.0),
        angle: pen.num(&format!("{shape}/angle")).unwrap_or(0.0),
        angle_jitter: pen.num(&format!("{shape}/randomAngle")).unwrap_or(0.0).clamp(0.0, 360.0),
        size_jitter: pct(&format!("{size_range}/randomPercentage"), 0.0).clamp(0.0, 1.0),
        dynamics: Default::default(),
    };
    Preset {
        name,
        target,
        settings,
        source: source.to_string(),
    }
}

/// Ce que l'import a trouvé.
#[derive(Debug, Default)]
pub struct Imported {
    pub presets: usize,
    pub tips: usize,
    pub papers: usize,
    /// Textures illisibles, avec la raison.
    pub skipped: Vec<String>,
}

/// Ajoute à `lib` les pinceaux et textures du fichier `text` (`source` : son nom).
pub fn import(text: &str, source: &str, lib: &mut BrushLibrary) -> Result<Imported, String> {
    let root = parse_xml(text)?;
    if root.name != "penstyles" {
        return Err(format!("ce fichier n'est pas une liste de pinceaux Toon Boom (<{}> au lieu de <penstyles>)", root.name));
    }
    let mut report = Imported::default();
    let (mut tips, mut papers) = (HashMap::new(), HashMap::new());
    for t in root.all("texture") {
        let (Some(id), Some(data)) = (t.value("textureId"), t.value("bundledFile")) else { continue };
        let name = t.value("textureName").unwrap_or(id);
        match decode_bundled(data) {
            Ok(img) if t.value("textureCategory") == Some("paperTextureCategory") => {
                papers.insert(id.to_string(), lib.add_paper(name, img));
                report.papers += 1;
            }
            Ok(mut img) => {
                // Pointe : noir = couleur déposée, blanc = rien.
                for v in &mut img.values {
                    *v = 1.0 - *v;
                }
                tips.insert(id.to_string(), lib.add_tip(name, img));
                report.tips += 1;
            }
            Err(e) => report.skipped.push(format!("{name} : {e}")),
        }
    }
    for pen in root.all("pen") {
        lib.presets.push(pen_to_preset(pen, &tips, &papers, source));
        report.presets += 1;
    }
    if report.presets == 0 {
        return Err("aucun pinceau dans ce fichier".into());
    }
    Ok(report)
}
