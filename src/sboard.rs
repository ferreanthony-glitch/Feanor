//! Lecture d'un projet Storyboard Pro (14a) : le fichier `.sboard` est du XML. On n'y lit que
//! ce qu'il décrit en clair — plans et panneaux dans l'ordre du film, leurs durées, leurs
//! légendes, le nom de leurs calques, la cadence, la taille de l'image et les pistes son. Les
//! dessins (`elements/…/*.tvg`) sont dans un format fermé : ils viennent d'un export fait par
//! Storyboard Pro lui-même (`sbscript`). Rien n'est jamais écrit dans le dossier du projet.
//!
//! Ce que dit le fichier (relevé sur de vrais projets, versions 25 et 27) :
//! - `timeline/scene@id` désigne la scène racine ; sa colonne de type 0 range les plans
//!   (`warpSeq`, dans l'ordre, `exposures="51-188"` en images du film comptées depuis 1) ;
//! - chaque plan range de même ses panneaux, dont la durée est leur nombre d'images ;
//! - le nom montré d'un plan et sa séquence sont dans `metas/meta/sceneInfo` ;
//! - les légendes d'un panneau sont dans `metas/meta/infoFields/field` (texte enrichi HTML),
//!   leurs noms dans `scenePanelInfoFieldList` du projet ;
//! - les calques d'un panneau sont ses modules `READ`, du premier plan au fond ;
//! - les pistes son sont les colonnes de type 1 de la scène racine ; un extrait
//!   (`soundSequence`) va de `startFrame` à `stopFrame` (comptés depuis 1, inclus) et joue le son
//!   depuis `clippingTimeStart` secondes jusqu'à `clippingTimeStop` (le premier atteint de la fin
//!   d'extrait ou de ce temps l'arrête). Ces temps sont ceux du son après ses effets de vitesse
//!   (vérifié : la durée d'un fichier divisée par le tempo donne `clippingTimeStop`).

use std::collections::HashMap;
use std::path::Path;

use crate::xml::{self, Element};

/// Un projet Storyboard Pro lu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbProject {
    pub title: String,
    pub fps: u32,
    /// Taille de l'image du projet (résolution), en pixels.
    pub size: [usize; 2],
    pub shots: Vec<SbShot>,
    pub tracks: Vec<SbTrack>,
    /// Ce qui n'a pas pu être lu tel quel.
    pub warnings: Vec<String>,
}

/// Un plan (« scene » dans Storyboard Pro).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbShot {
    pub id: String,
    pub name: String,
    /// Nom de sa séquence ; vide : aucune.
    pub sequence: String,
    /// Première image du plan dans le film, comptée depuis 1.
    pub start: usize,
    pub panels: Vec<SbPanel>,
}

/// Un panneau, avec ses légendes réparties dans les trois textes de Pelure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbPanel {
    pub id: String,
    pub frames: usize,
    pub dialogue: String,
    pub action: String,
    pub notes: String,
    /// Calques, du premier plan au fond.
    pub layers: Vec<SbLayer>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbLayer {
    pub name: String,
    pub vector: bool,
    /// Opacité, de 0 à 1.
    pub opacity: f32,
}

/// Une piste son.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbTrack {
    pub name: String,
    pub muted: bool,
    pub clips: Vec<SbClip>,
}

/// Un extrait de son.
#[derive(Clone, Debug, PartialEq)]
pub struct SbClip {
    /// Nom du fichier, dans le dossier `audio` du projet.
    pub file: String,
    /// Première et dernière image (incluse) dans le film, comptées depuis 1.
    pub start: usize,
    pub stop: usize,
    /// Temps du son (après ses effets de vitesse) au début de l'extrait, et temps où il s'arrête.
    pub from: f64,
    pub to: f64,
    /// Gain linéaire (1 : inchangé ; 1,995 = +6 dB, la plus forte valeur rencontrée).
    pub volume: f32,
    /// Changement de vitesse sans changer la hauteur (effet « WSOLATempo », 1 : aucun).
    pub tempo: f64,
    /// Changement de vitesse et de hauteur ensemble (effet « Resample », 1 : aucun).
    pub speed: f64,
    /// Une enveloppe de volume (non reprise).
    pub envelope: bool,
    /// Effets actifs non repris (réverbération, hauteur…).
    pub other_effects: Vec<String>,
}

impl SbClip {
    /// Images couvertes par l'extrait.
    pub fn frames(&self) -> usize {
        (self.stop + 1).saturating_sub(self.start)
    }
}

/// Lit le fichier `.sboard` `path` (lecture seule).
pub fn read(path: &Path) -> Result<SbProject, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture de « {} » impossible : {e}", path.display()))?;
    let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    parse(&String::from_utf8_lossy(&bytes), &stem)
}

/// Lit le texte XML d'un projet ; `fallback_title` sert si le projet n'a pas de titre.
pub fn parse(src: &str, fallback_title: &str) -> Result<SbProject, String> {
    let root = xml::parse(src)?;
    if root.name != "project" {
        return Err("ce fichier n'est pas un projet Storyboard Pro".into());
    }
    let mut sb = SbProject { fps: 24, size: [1920, 1080], ..Default::default() };
    if let Some(options) = root.child("options") {
        if let Some(f) = options.child("framerate").and_then(|f| f.attr("val")).and_then(|v| v.parse::<f64>().ok()) {
            sb.fps = (f.round() as u32).max(1);
        }
        if let Some((w, h)) = options.child("resolution").and_then(|r| r.attr("size")).and_then(pair) {
            sb.size = [w, h];
        }
    }
    sb.title = project_title(&root).unwrap_or_else(|| fallback_title.to_string());
    let fields = field_names(&root);

    let scenes: HashMap<&str, &Element> = root
        .child("scenes")
        .map(|s| s.children_named("scene").filter_map(|e| Some((e.attr("id")?, e))).collect())
        .unwrap_or_default();
    let top_id = root.path("timeline/scene").and_then(|s| s.attr("id")).ok_or("projet sans film (timeline vide)")?;
    let top = *scenes.get(top_id).ok_or("la scène racine du film est introuvable")?;

    for (id, range) in sequence_of(top) {
        let Some(shot) = scenes.get(id.as_str()) else {
            sb.warnings.push(format!("Plan {id} introuvable : sauté."));
            continue;
        };
        let info = meta(shot, "sceneInfo").and_then(|m| m.child("sceneInfo"));
        let name = info.and_then(|i| i.attr("name")).or(shot.attr("name")).unwrap_or("").to_string();
        let sequence = info.and_then(|i| i.attr("sequenceName")).unwrap_or("").to_string();
        let mut panels = Vec::new();
        for (pid, prange) in sequence_of(shot) {
            let Some(p) = scenes.get(pid.as_str()) else {
                sb.warnings.push(format!("Plan {name} : panneau {pid} introuvable, sauté."));
                continue;
            };
            let frames = prange
                .map(|(a, b)| (b + 1).saturating_sub(a))
                .or_else(|| p.attr("nbframes").and_then(|n| n.parse().ok()))
                .unwrap_or(1)
                .max(1);
            panels.push(read_panel(p, pid, frames, &fields));
        }
        if panels.is_empty() {
            sb.warnings.push(format!("Plan {name} : aucun panneau, sauté."));
            continue;
        }
        sb.shots.push(SbShot { id, name, sequence, start: range.map_or(0, |r| r.0), panels });
    }
    if sb.shots.is_empty() {
        return Err("le film de ce projet ne contient aucun panneau".into());
    }
    sb.tracks = read_tracks(top, &mut sb.warnings);
    Ok(sb)
}

/// Les éléments rangés dans la colonne de type 0 de `scene` (plans ou panneaux), dans l'ordre,
/// avec leurs images (première, dernière) comptées depuis 1.
fn sequence_of(scene: &Element) -> Vec<(String, Option<(usize, usize)>)> {
    let Some(columns) = scene.child("columns") else { return Vec::new() };
    let Some(col) = columns
        .children_named("column")
        .find(|c| c.attr("type") == Some("0") && c.child("warpSeq").is_some())
    else {
        return Vec::new();
    };
    col.children_named("warpSeq")
        .filter_map(|w| Some((w.attr("id")?.to_string(), w.attr("exposures").and_then(range))))
        .collect()
}

fn read_panel(p: &Element, id: String, frames: usize, fields: &HashMap<String, String>) -> SbPanel {
    let mut panel = SbPanel { id, frames, ..Default::default() };
    let mut notes = Vec::new();
    if let Some(info) = meta(p, "infoFields").and_then(|m| m.child("infoFields")) {
        for f in info.children_named("field") {
            let text = caption_text(f.attr("value").unwrap_or(""));
            if text.is_empty() {
                continue;
            }
            let fid = f.attr("id").unwrap_or("");
            let name = fields.get(fid).cloned().unwrap_or_else(|| default_field(fid).to_string());
            let lower = name.to_lowercase();
            if lower.starts_with("dialog") {
                append(&mut panel.dialogue, &text);
            } else if lower.starts_with("action") {
                append(&mut panel.action, &text);
            } else if lower == "notes" {
                notes.push(text);
            } else {
                notes.push(format!("{name} : {text}"));
            }
        }
    }
    panel.notes = notes.join("\n");
    if let Some(group) = p.child("rootgroup") {
        let mut modules = Vec::new();
        group.find_all("module", &mut modules);
        for m in modules.into_iter().filter(|m| m.attr("type") == Some("READ")) {
            let attrs = m.child("attrs");
            let val = |k: &str| attrs.and_then(|a| a.child(k)).and_then(|e| e.attr("val"));
            panel.layers.push(SbLayer {
                name: m.attr("name").unwrap_or("Calque").to_string(),
                vector: val("lineArtDrawingMode") == Some("VectorDrawingMode"),
                opacity: val("opacity").and_then(|v| v.parse::<f32>().ok()).map_or(1.0, |o| (o / 100.0).clamp(0.0, 1.0)),
            });
        }
    }
    panel
}

fn read_tracks(top: &Element, warnings: &mut Vec<String>) -> Vec<SbTrack> {
    let Some(columns) = top.child("columns") else { return Vec::new() };
    let mut tracks = Vec::new();
    for col in columns.children_named("column").filter(|c| c.attr("type") == Some("1")) {
        let name = col.attr("name").unwrap_or("Son").to_string();
        let mut clips = Vec::new();
        for s in col.children_named("soundSequence") {
            let num = |k: &str| s.attr(k).and_then(|v| v.parse::<f64>().ok());
            let (Some(file), Some(start), Some(stop)) = (s.attr("name"), num("startFrame"), num("stopFrame")) else {
                warnings.push(format!("Piste {name} : un extrait sans fichier ou sans images, sauté."));
                continue;
            };
            let mut clip = SbClip {
                file: file.to_string(),
                start: start.max(1.0) as usize,
                stop: stop.max(1.0) as usize,
                from: num("clippingTimeStart").unwrap_or(0.0).max(0.0),
                to: num("clippingTimeStop").unwrap_or(f64::INFINITY),
                volume: num("volume").unwrap_or(1.0) as f32,
                tempo: 1.0,
                speed: 1.0,
                envelope: s.child("envelope").is_some(),
                other_effects: Vec::new(),
            };
            for fx in s.children_named("soundEffect").filter(|e| e.attr("disabled") != Some("true")) {
                let first = fx.child("attrs").and_then(|a| a.children.first());
                let value = first.and_then(|v| v.attr("val")).and_then(|v| v.parse::<f64>().ok());
                match (fx.attr("name").unwrap_or(""), value) {
                    ("WSOLATempo", Some(v)) if v > 0.0 => clip.tempo *= v / 100.0,
                    ("Resample", Some(v)) if v > 0.0 => clip.speed *= v / 100.0,
                    (other, _) => clip.other_effects.push(other.to_string()),
                }
            }
            clips.push(clip);
        }
        tracks.push(SbTrack { name, muted: col.attr("disabled") == Some("true"), clips });
    }
    tracks
}

/// La méta `type` d'un élément (ses `metas/meta`).
fn meta<'a>(e: &'a Element, kind: &str) -> Option<&'a Element> {
    e.child("metas")?.children_named("meta").find(|m| m.attr("type") == Some(kind))
}

fn project_title(root: &Element) -> Option<String> {
    let m = root.child("metas")?.children_named("meta").find(|m| m.attr("name") == Some("projectTitle"))?;
    let t = m.child("string")?.attr("value")?.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Noms des champs de légende du projet, par numéro.
fn field_names(root: &Element) -> HashMap<String, String> {
    let mut lists = Vec::new();
    if let Some(m) = root.child("metas") {
        m.find_all("scenePanelInfoFieldList", &mut lists);
    }
    lists
        .first()
        .map(|l| {
            l.children_named("field")
                .filter_map(|f| Some((f.attr("id")?.to_string(), f.attr("name")?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// Champs d'un projet neuf de Storyboard Pro, si le projet ne les nomme pas.
fn default_field(id: &str) -> &'static str {
    match id {
        "1" => "Dialog",
        "2" => "Action Notes",
        "3" => "Slugging",
        _ => "Notes",
    }
}

fn append(dst: &mut String, text: &str) {
    if !dst.is_empty() {
        dst.push('\n');
    }
    dst.push_str(text);
}

/// « 51-188 » → (51, 188) ; « 7 » → (7, 7).
fn range(s: &str) -> Option<(usize, usize)> {
    let mut it = s.split('-').map(|v| v.trim().parse::<usize>());
    let a = it.next()?.ok()?;
    let b = match it.next() {
        Some(v) => v.ok()?,
        None => a,
    };
    Some((a, b.max(a)))
}

/// « 1920,1080 » → (1920, 1080).
fn pair(s: &str) -> Option<(usize, usize)> {
    let (a, b) = s.split_once(',')?;
    let (w, h) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// Texte d'une légende : le texte enrichi (HTML de Qt) devient du texte simple, un paragraphe
/// ou un retour à la ligne par ligne.
pub fn caption_text(html: &str) -> String {
    if !html.contains('<') {
        return decode_entities(html).trim().to_string();
    }
    let body = html.find("<body").map_or(html, |i| &html[i..]);
    let mut out = String::new();
    let mut rest = body;
    while let Some(lt) = rest.find('<') {
        // Un retour à la ligne entre les balises n'est qu'un blanc : les lignes viennent des
        // balises (`<br />`, fin de paragraphe).
        out.push_str(&decode_entities(&rest[..lt]).replace(['\n', '\r'], " "));
        let end = rest[lt..].find('>').map_or(rest.len(), |e| lt + e + 1);
        let tag = rest[lt + 1..end.saturating_sub(1).max(lt + 1)].trim().to_ascii_lowercase();
        if tag.starts_with("br") || tag == "/p" || tag.starts_with("/li") || tag.starts_with("/h") {
            out.push('\n');
        }
        rest = &rest[end..];
    }
    out.push_str(&decode_entities(rest).replace(['\n', '\r'], " "));
    let lines: Vec<&str> = out.split('\n').map(str::trim).collect();
    lines.join("\n").trim().to_string()
}

/// Entités HTML courantes (`&amp;`, `&nbsp;`, `&#233;`…).
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(semi) = tail.find(';').filter(|&k| k <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..semi];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
