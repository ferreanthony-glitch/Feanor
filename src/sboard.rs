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
    /// Table des éléments de dessin : identifiant -> (dossier, nom) ; le dessin `val` de l'élément
    /// est `elements/<dossier>/<nom>-<val>.tvg` (import 71).
    pub elements: HashMap<String, (String, String)>,
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
    /// Clés de la caméra du plan (import 71) : position du « peg » de la caméra, par image du plan.
    pub camera: Vec<SbCamKey>,
}

/// Une clé de la caméra : image du plan (depuis 0), position en champs (x droite, y haut) et z.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SbCamKey {
    pub frame: usize,
    pub pos: [f64; 3],
}

/// Un panneau, avec ses légendes réparties dans les trois textes de Pelure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbPanel {
    pub id: String,
    pub frames: usize,
    /// Nom du panneau (méta `panelInfo`), vide si aucun.
    pub name: String,
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
    /// Élément et nom du dessin exposé (`elementSeq@id`, `@val`) ; vides : calque sans dessin.
    pub element: String,
    pub drawing: String,
    pub visible: bool,
    pub locked: bool,
    /// Placement du calque dans le panneau : décalage en champs, échelle, rotation en degrés.
    pub offset: [f32; 2],
    pub scale: f32,
    pub angle: f32,
    /// Effets reliés au calque (noms des modules), non repris.
    pub effects: Vec<String>,
    /// Un réglage du calque est animé par une courbe (non repris), ou incliné (`skew`).
    pub animated: bool,
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

/// Lit un projet modèle (export vers Storyboard Pro, 14b) : comme `read`, mais un projet sans
/// aucun panneau (un modèle vide) se lit aussi.
pub fn read_template(path: &Path) -> Result<SbProject, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture de « {} » impossible : {e}", path.display()))?;
    let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    parse_any(&String::from_utf8_lossy(&bytes), &stem)
}

/// Lit le texte XML d'un projet ; `fallback_title` sert si le projet n'a pas de titre.
pub fn parse(src: &str, fallback_title: &str) -> Result<SbProject, String> {
    let sb = parse_any(src, fallback_title)?;
    if sb.shots.is_empty() {
        return Err("le film de ce projet ne contient aucun panneau".into());
    }
    Ok(sb)
}

fn parse_any(src: &str, fallback_title: &str) -> Result<SbProject, String> {
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
    let sequences = sequences_exist(&root);

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
        let sequence = if sequences { info.and_then(|i| i.attr("sequenceName")).unwrap_or("").to_string() } else { String::new() };
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
        let camera = read_camera(shot);
        sb.shots.push(SbShot { id, name, sequence, start: range.map_or(0, |r| r.0), panels, camera });
    }
    sb.tracks = read_tracks(top, &mut sb.warnings);
    if let Some(els) = root.child("elements") {
        for e in els.children_named("element") {
            if let (Some(id), Some(folder)) = (e.attr("id"), e.attr("elementFolder")) {
                sb.elements.insert(id.to_string(), (folder.to_string(), e.attr("elementName").unwrap_or("Draw").to_string()));
            }
        }
    }
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
    panel.name = meta(p, "panelInfo").and_then(|m| m.child("panelInfo")).and_then(|i| i.attr("name")).unwrap_or("").to_string();
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
        let cols: HashMap<&str, &Element> = p.child("columns").map(|c| c.children_named("column").filter_map(|c| Some((c.attr("name")?, c))).collect()).unwrap_or_default();
        let blurs: Vec<&Element> = modules.iter().copied().filter(|m| m.attr("type").is_some_and(|t| t != "READ" && t != "PEG" && t != "COMPOSITE" && t != "CAMERA")).collect();
        let links: Vec<(&str, &str)> = group.child("linkedlist").map(|l| l.children_named("link").filter_map(|k| Some((k.attr("out")?, k.attr("in")?))).collect()).unwrap_or_default();
        for m in modules.iter().copied().filter(|m| m.attr("type") == Some("READ")) {
            let attrs = m.child("attrs");
            let val = |k: &str| attrs.and_then(|a| a.child(k)).and_then(|e| e.attr("val"));
            let sub = |a: &str, b: &str| attrs.and_then(|x| x.child(a)).and_then(|x| x.child(b)).and_then(|e| e.attr("val")).and_then(|v| v.parse::<f32>().ok());
            let seq = attrs
                .and_then(|a| a.path("drawing/element"))
                .and_then(|e| e.attr("col"))
                .and_then(|c| cols.get(c))
                .and_then(|c| c.child("elementSeq"));
            let name = m.attr("name").unwrap_or("Calque");
            let effects = links.iter().filter(|(o, _)| *o == name).filter_map(|(_, i)| blurs.iter().find(|b| b.attr("name") == Some(i)).map(|b| format!("{}", b.attr("type").unwrap_or("?")))).collect();
            panel.layers.push(SbLayer {
                element: seq.and_then(|s| s.attr("id")).unwrap_or("").to_string(),
                drawing: seq.and_then(|s| s.attr("val")).unwrap_or("").to_string(),
                visible: m.path("options/disabled").and_then(|d| d.attr("val")) != Some("true"),
                locked: m.path("options/locked").and_then(|d| d.attr("val")) == Some("true"),
                offset: [sub("offset", "x").unwrap_or(0.0), sub("offset", "y").unwrap_or(0.0)],
                scale: sub("scale", "xy").unwrap_or(1.0),
                angle: sub("rotation", "anglez").unwrap_or(0.0),
                effects,
                animated: attrs.is_some_and(|a| a.children.iter().filter(|c| c.name != "drawing").any(|c| has_col(c))),
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

/// Clés de la caméra d'un plan : le chemin 3D du `PEG` de la caméra (colonne de type 2), à
/// l'image `lockedInTime` de chaque point (depuis 1) ; sinon la position fixe du `PEG`. Unités :
/// champs (à vérifier, `docs/IMPORT-SBP.md`).
fn read_camera(shot: &Element) -> Vec<SbCamKey> {
    let mut modules = Vec::new();
    if let Some(g) = shot.child("rootgroup") {
        g.find_all("module", &mut modules);
    }
    let Some(peg) = modules.iter().find(|m| m.attr("type") == Some("PEG") && m.attr("name").is_some_and(|n| n.starts_with("Camera"))) else { return Vec::new() };
    let pos = peg.path("attrs/position");
    let fixed = |k: &str| pos.and_then(|p| p.child(k)).and_then(|e| e.attr("val")).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let base = [fixed("x"), fixed("y"), fixed("z")];
    let col = pos.and_then(|p| p.child("attr3dpath")).and_then(|e| e.attr("col"));
    let mut keys = Vec::new();
    if let (Some(col), Some(columns)) = (col, shot.child("columns")) {
        if let Some(points) = columns.children_named("column").find(|c| c.attr("name") == Some(col)).and_then(|c| c.path("path3D/points")) {
            for pt in points.children_named("pt") {
                let v: Vec<f64> = pt.attr("val").unwrap_or("").split(',').filter_map(|s| s.trim().parse().ok()).collect();
                let at = pt.attr("lockedInTime").and_then(|s| s.parse::<usize>().ok());
                if let (3, Some(at)) = (v.len(), at) {
                    keys.push(SbCamKey { frame: at.saturating_sub(1), pos: [v[0], v[1], v[2]] });
                }
            }
        }
    }
    if keys.is_empty() && base != [0.0, 0.0, 0.0] {
        keys.push(SbCamKey { frame: 0, pos: base });
    }
    keys
}

fn has_col(e: &Element) -> bool {
    e.attr("col").is_some() || e.children.iter().any(has_col)
}

/// La méta `type` d'un élément (ses `metas/meta`).
fn meta<'a>(e: &'a Element, kind: &str) -> Option<&'a Element> {
    e.child("metas")?.children_named("meta").find(|m| m.attr("type") == Some(kind))
}

/// Le projet a-t-il des séquences ? La méta `sequenceExists` vaut `true` quand il en a ; vide,
/// le `sequenceName` des plans n'est qu'une valeur par défaut (« 0 », « 0_A »… dans les projets
/// NAJI et BRAWLIDAYS partie 1, où Storyboard Pro ne montre aucune séquence). Sans cette méta
/// (projets plus anciens), on garde les noms. Lu dans `analyse-sbpro` (2026-10-17), à confirmer
/// dans Storyboard Pro.
fn sequences_exist(root: &Element) -> bool {
    let Some(m) = root.child("metas").and_then(|m| m.children_named("meta").find(|m| m.attr("name") == Some("sequenceExists"))) else {
        return true;
    };
    m.child("bool").and_then(|b| b.attr("value")) == Some("true")
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
