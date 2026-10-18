//! Import des brosses de TVPaint (moteur de dessin, étape 10) : un export de panneau `.tvpx`
//! (archive tar), un panneau `.bin` et son dossier `.data`, ou un réglage `.ini` seul. Lu d'après
//! les fichiers de TVPaint 12 et sa référence George (relevés par Fable) ; ce qui est deviné est
//! dit ici et dans le compte rendu.
//! - Un bouton du panneau (`N_text`, `N_cmd0`, `N_cmd1`…) : `CBTool:<uuid>` désigne une brosse
//!   perso (`<uuid>.ini` et sa pointe `<uuid>.dip`) ; `Tool:tv_penbrush …` (et airbrush, pencil,
//!   propelling) un outil de base avec ses paramètres ; `cmd: tv_paper …` son papier.
//! - Connexions (`c_size2`, `c_opacity2`, `c_angle2`, et `csize`, `cpower`, `cangle`, `caspect`
//!   des outils de base) : `code;N d1 d2 x1 y1 … xN yN`. Codes de la référence George : 10
//!   pression, 4 vitesse, 5 direction, 6 orientation (prise comme la direction), 7 fondu, 8
//!   hasard ; 11 altitude, 12 azimut, 13 à 15 molettes, 17 rotation ne sont pas lus par Pelure.
//!   Un code négatif retourne l'entrée (deviné).
//! - Profil (`profile "N d1 d2 x y …"`) : x du bord (0) au centre (1), retourné pour Pelure
//!   (deviné d'après les profils livrés, qui montent de (0, 0) à (1, 1)).
//! - Taille d'une brosse perso : `size` % de sa pointe ; pas : `stepval` % de la taille (déduit des
//!   fichiers : `step` = taille × `stepval`) ; dispersion : `jitter` ÷ 50 diamètres (deviné).
//! - Ordre des images d'une pointe animée (`anim`) : d'après l'ordre des libellés de TVPaint
//!   (deviné ; le code d'origine est noté dans le compte rendu).
//! - Papier : `Hardness` → grain, `Size` → taille, `Angle`, `Invert` ; son image est cherchée par
//!   son nom dans les papiers de l'installation de TVPaint (lecture seule).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::brush::{BrushSettings, Connection, Curve, PencilGrain, Source, TipOrder};
use crate::brushtex::{BrushLibrary, GrayImage, Preset, Target};

/// Ce que l'import a fait.
#[derive(Debug, Default)]
pub struct TvpImport {
    pub presets: usize,
    pub tips: usize,
    pub papers: usize,
    /// Ce qui n'a pas pu être repris tel quel.
    pub notes: Vec<String>,
}

/// Les fichiers d'un `.tvpx` (archive tar ustar) : nom (chemin dans l'archive) → contenu.
pub fn read_tar(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut files = HashMap::new();
    let mut i = 0;
    while i + 512 <= bytes.len() {
        let h = &bytes[i..i + 512];
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let text = |a: usize, b: usize| String::from_utf8_lossy(&h[a..b]).trim_end_matches('\0').to_string();
        let mut name = text(0, 100);
        let prefix = text(345, 500);
        if !prefix.is_empty() {
            name = format!("{prefix}/{name}");
        }
        let size_text = text(124, 136);
        let size = usize::from_str_radix(size_text.trim(), 8).map_err(|_| format!("archive illisible (taille « {size_text} »)"))?;
        let kind = h[156];
        let start = i + 512;
        let end = start + size;
        if end > bytes.len() {
            return Err("archive tronquée".into());
        }
        if kind == b'0' || kind == 0 {
            files.insert(name.replace('\\', "/"), bytes[start..end].to_vec());
        }
        i = start + size.div_ceil(512) * 512;
    }
    Ok(files)
}

/// Les clés d'un fichier INI : section → clé → valeur. (Les fichiers de TVPaint commencent par
/// la marque UTF-8, BOM.) Les noms de section sont mis en minuscules : les panneaux livrés
/// écrivent `[toolbox]`, un panneau exporté par TVPaint 12 (`.tvpx`) écrit `[ToolBox]` (audit du
/// rendu, 2026-10-18 : l'export d'Anthony n'importait aucun outil).
pub fn read_ini(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut section = String::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_lowercase();
        } else if let Some((k, v)) = line.split_once('=') {
            out.entry(section.clone()).or_default().insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// Une connexion de TVPaint (`code;N d1 d2 x1 y1 …`) ; aucune si elle ne suit rien que Pelure
/// sache lire (le nom de la source non lue part dans `skipped`).
pub fn connection(text: &str, skipped: &mut Vec<String>) -> Option<Connection> {
    let (code, rest) = text.split_once(';')?;
    let code: i32 = code.trim().parse().ok()?;
    let source = match code.abs() {
        0 => return None,
        10 => Source::Pressure,
        4 => Source::Speed,
        5 | 6 => Source::Direction,
        7 => Source::Fade,
        8 => Source::Random,
        11 => {
            skipped.push("inclinaison (altitude)".into());
            return None;
        }
        12 => {
            skipped.push("azimut".into());
            return None;
        }
        13..=15 => {
            skipped.push("molette".into());
            return None;
        }
        17 => {
            skipped.push("rotation du stylet".into());
            return None;
        }
        other => {
            skipped.push(format!("source {other}"));
            return None;
        }
    };
    let curve = curve_points(rest).map(|p| Curve::from_points(&p)).unwrap_or_default();
    Some(Connection { source, invert: code < 0, curve, min: 0.0 })
}

/// Les points d'une courbe TVPaint `N d1 d2 x1 y1 … xN yN`, au plus `Curve::MAX` (pris à
/// intervalles réguliers s'il y en a plus).
pub fn curve_points(text: &str) -> Option<Vec<[f32; 2]>> {
    let nums: Vec<f32> = text.split_whitespace().filter_map(|v| v.parse().ok()).collect();
    let n = *nums.first()? as usize;
    let pts: Vec<[f32; 2]> = nums.get(3..3 + 2 * n)?.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    if pts.len() <= Curve::MAX {
        return Some(pts);
    }
    let last = pts.len() - 1;
    Some((0..Curve::MAX).map(|k| pts[k * last / (Curve::MAX - 1)]).collect())
}

/// Les paramètres d'une commande George (`tv_penbrush mode "color" size 10 …`) : nom → valeur.
pub fn george_args(cmd: &str) -> (String, HashMap<String, String>) {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in cmd.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                if !quoted {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    let name = tokens.first().cloned().unwrap_or_default();
    let mut args = HashMap::new();
    let mut k = 1;
    while k + 1 < tokens.len() {
        args.insert(tokens[k].to_lowercase(), tokens[k + 1].clone());
        k += 2;
    }
    (name, args)
}

/// Le papier d'un bouton (`tv_paper Hardness … Name "/Paper 33"`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TvpPaper {
    pub name: String,
    pub hardness: f32,
    pub size: f32,
    pub angle: f32,
    pub invert: bool,
}

pub fn paper_of(cmd: &str) -> Option<TvpPaper> {
    let (_, a) = george_args(cmd.trim_start_matches("cmd:").trim());
    let num = |k: &str, d: f32| a.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    if num("active", 1.0) == 0.0 {
        return None;
    }
    let name = a.get("name")?.trim_start_matches('/').to_string();
    Some(TvpPaper { name, hardness: num("hardness", 100.0), size: num("size", 1.0), angle: num("angle", 0.0), invert: num("invert", 0.0) != 0.0 })
}

/// Le réglage d'une brosse perso (`.ini` de TVPaint) ; `tip_size` : la taille de sa pointe
/// (pixels), `notes` reçoit ce qui n'est pas repris.
pub fn custom_brush(ini: &HashMap<String, HashMap<String, String>>, tip_size: Option<usize>, notes: &mut Vec<String>) -> BrushSettings {
    let empty = HashMap::new();
    let t = ini.get("tvp tool").unwrap_or(&empty);
    let num = |k: &str, d: f32| t.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
    let native = tip_size.map(|s| s as f32).unwrap_or_else(|| num("width", 10.0).max(num("height", 10.0)));
    let mut s = BrushSettings { pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() };
    s.size = (native * num("size", 100.0) / 100.0).clamp(0.5, 400.0);
    s.opacity = (num("opacity", 100.0) / 100.0).clamp(0.0, 1.0);
    s.angle = num("angle", 0.0);
    if num("stepon", 0.0) != 0.0 {
        s.spacing = (num("stepval", 8.0) / 100.0).clamp(0.02, 1.0);
    }
    s.scatter = (num("jitter", 0.0) / 50.0).clamp(0.0, 4.0);
    s.flip_x = num("flipx", 0.0) != 0.0;
    s.flip_y = num("flipy", 0.0) != 0.0;
    let mut skipped = Vec::new();
    if let Some(c) = t.get("c_size2").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.size = c;
    }
    if let Some(c) = t.get("c_opacity2").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.opacity = c;
    }
    if let Some(c) = t.get("c_angle2").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.angle = c;
    }
    for (k, what) in [("c_jitter2", "de dispersion"), ("c_shift2", "de décalage"), ("c_smear", "d'estompe")] {
        if t.get(k).is_some_and(|v| !v.starts_with("0;")) {
            notes.push(format!("connexion {what} non reprise"));
        }
    }
    for k in skipped {
        notes.push(format!("connexion à {k} non reprise"));
    }
    // L'estompe ne joue que dans les modes d'empreinte qui étalent (6 à 8, d'après George) : elle
    // devient le mélange de la peinture humide de Pelure (étape 11).
    if matches!(num("stamp", 1.0) as i32, 6..=8) {
        let smear = num("smear", 50.0);
        s.mix = (smear / 100.0).clamp(0.05, 1.0);
        notes.push(format!("estompe ({smear:.0}) prise comme le mélange de SHOT, à comparer"));
    }
    if num("shift", 0.0) > 0.0 {
        notes.push("décalage de couleur (shift) non repris".into());
    }
    if num("dry", 1.0) == 0.0 {
        notes.push("peinture qui ne sèche pas non reprise".into());
    }
    if num("projection", 0.0) != 0.0 {
        notes.push("projection selon l'inclinaison non reprise".into());
    }
    if num("mode", 0.0) != 0.0 {
        notes.push(format!("mode de dessin {} pris comme « couleur »", num("mode", 0.0)));
    }
    s
}

/// L'ordre des images d'une pointe animée (`anim` de TVPaint, d'après l'ordre de ses libellés :
/// deviné) ; aucun : pointe fixe.
pub fn anim_order(code: i32) -> Option<TipOrder> {
    match code {
        0 => None,
        5 | 11 => Some(TipOrder::Random),
        6 => Some(TipOrder::Pressure),
        8 => Some(TipOrder::Direction),
        _ => Some(TipOrder::Loop),
    }
}

/// Un outil de base de TVPaint (`tv_penbrush …`), ou aucun s'il n'est pas un outil de dessin.
pub fn base_tool(cmd: &str, notes: &mut Vec<String>) -> Option<BrushSettings> {
    let (name, a) = george_args(cmd.trim_start_matches("Tool:").trim());
    let num = |k: &str, d: f32| a.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
    let mut s = BrushSettings { pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() };
    match name.as_str() {
        "tv_penbrush" => {
            s.hardness = 1.0;
            s.aliased = num("aaliasing", 1.0) == 0.0;
        }
        "tv_airbrush" => s.hardness = 0.0,
        "tv_pencil" => s.hardness = 0.9,
        "tv_propelling" => {
            s.hardness = 1.0;
            s.aliased = true;
        }
        "tv_specialbrush" => {
            notes.push("outil spécial (estompe, décalage) non repris".into());
            return None;
        }
        _ => return None,
    }
    s.size = num("size", 6.0).clamp(0.5, 400.0);
    s.opacity = (num("opacity", 100.0) / 100.0).clamp(0.0, 1.0);
    let power = (num("power", 100.0) / 100.0).clamp(0.01, 1.0);
    s.power = power;
    s.build_up = power < 1.0;
    // `step` : pris en pour cent de la taille, comme le `stepval` des brosses perso (12,5 par
    // défaut dans les deux cas ; audit du rendu, 2026-10-18 : lu en pixels, le crayon 9,5 px
    // d'Anthony à `step 12.5` posait une empreinte par diamètre, un trait en chapelet). Deviné,
    // à vérifier dans TVPaint.
    if let Some(step) = a.get("step").and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.0) {
        s.spacing = (step / 100.0).clamp(0.02, 1.0);
    }
    if name == "tv_pencil" {
        if let Some(g) = a.get("grain").and_then(|v| v.parse::<f32>().ok()) {
            // Le menu « Grain » du crayon de TVPaint (Silk, Soft, Medium, Hard, dans l'ordre de ses
            // libellés — deviné, à vérifier) devient le grain propre du crayon de Pelure (rendu,
            // étape 3), dont le calcul est celui de Pelure.
            s.pencil_grain = match g as i32 {
                0 => PencilGrain::Silk,
                1 => PencilGrain::Soft,
                2 => PencilGrain::Medium,
                _ => PencilGrain::Hard,
            };
            notes.push(format!("grain du crayon {g:.0} pris comme « {} » de SHOT (ordre Silk, Soft, Medium, Hard deviné, à vérifier)", s.pencil_grain.label()));
        }
    }
    if let Some(aspect) = a.get("aspect").and_then(|v| v.parse::<f32>().ok()) {
        s.roundness = (aspect / 100.0).clamp(0.05, 1.0);
    }
    s.angle = num("angle", 0.0);
    let mut skipped = Vec::new();
    if let Some(c) = a.get("csize").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.size = c;
    }
    if let Some(c) = a.get("cpower").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.opacity = c;
    }
    if let Some(c) = a.get("cangle").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.angle = c;
    }
    if let Some(c) = a.get("caspect").and_then(|v| connection(v, &mut skipped)) {
        s.dynamics.roundness = c;
    }
    for k in skipped {
        notes.push(format!("connexion à {k} non reprise"));
    }
    if let Some(pts) = a.get("profile").and_then(|p| curve_points(p)) {
        // Du bord au centre chez TVPaint, du centre au bord chez Pelure.
        let flipped: Vec<[f32; 2]> = pts.iter().map(|p| [1.0 - p[0], p[1]]).collect();
        s.profile = Some(Curve::from_points(&flipped));
    }
    if num("dry", 1.0) == 0.0 {
        notes.push("peinture qui ne sèche pas non reprise".into());
    }
    Some(s)
}

/// Les fichiers d'un panneau : son `.bin` et ceux de son dossier `.data` (nom simple → contenu).
pub struct Panel {
    pub bin: String,
    pub data: HashMap<String, Vec<u8>>,
}

impl Panel {
    /// Le panneau d'un `.tvpx`.
    pub fn from_tvpx(bytes: &[u8]) -> Result<Panel, String> {
        let files = read_tar(bytes)?;
        let (bin_name, bin) = files.iter().find(|(n, _)| n.ends_with(".bin")).ok_or("aucun panneau dans l'archive")?;
        let dir = bin_name.trim_end_matches(".bin").to_string() + ".data/";
        let data = files.iter().filter_map(|(n, b)| n.strip_prefix(&dir).map(|s| (s.to_string(), b.clone()))).collect();
        Ok(Panel { bin: String::from_utf8_lossy(bin).into_owned(), data })
    }

    /// Le panneau d'un `.bin` et de son dossier `.data` voisin.
    pub fn from_bin(path: &Path) -> Result<Panel, String> {
        let bin = std::fs::read(path).map_err(|e| format!("lecture impossible : {e}"))?;
        let dir = path.with_extension("data");
        let mut data = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if let Ok(b) = std::fs::read(e.path()) {
                    data.insert(e.file_name().to_string_lossy().into_owned(), b);
                }
            }
        }
        Ok(Panel { bin: String::from_utf8_lossy(&bin).into_owned(), data })
    }
}

/// Le dossier `Resources` de TVPaint, s'il est là (`PELURE_TVPAINT` peut le désigner).
pub fn tvpaint_resources() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PELURE_TVPAINT").map(PathBuf::from) {
        return p.is_dir().then_some(p);
    }
    let base = PathBuf::from(std::env::var_os("ProgramFiles")?).join("TVPaint Developpement");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&base).ok()?.flatten().map(|e| e.path().join("Resources")).filter(|p| p.is_dir()).collect();
    found.sort();
    found.pop()
}

/// L'image du papier `name` dans les papiers de TVPaint (PNG, ou `.dip`), en relief gris.
pub fn find_paper(res: &Path, name: &str) -> Option<GrayImage> {
    let papers = res.join("papers");
    let mut dirs = vec![papers.clone()];
    if let Ok(rd) = std::fs::read_dir(&papers) {
        dirs.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    for dir in dirs {
        for ext in ["png", "dip"] {
            let path = dir.join(format!("{name}.{ext}"));
            let Ok(bytes) = std::fs::read(&path) else { continue };
            if ext == "png" {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    return Some(GrayImage::from_image(&img, 1024));
                }
            } else if let Ok(d) = crate::dip::read(&bytes) {
                let f = &d.frames[0];
                let values = f.iter().map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0).collect();
                return Some(GrayImage::new(d.width, d.height, values));
            }
        }
    }
    None
}

/// Importe les brosses d'un panneau dans la bibliothèque (provenance `source`) ; `res` : le
/// dossier `Resources` de TVPaint, pour les papiers.
pub fn import_panel(panel: &Panel, source: &str, lib: &mut BrushLibrary, res: Option<&Path>) -> TvpImport {
    let mut report = TvpImport::default();
    let ini = read_ini(&panel.bin);
    let empty = HashMap::new();
    let tb = ini.get("toolbox").unwrap_or(&empty);
    let mut buttons: Vec<usize> = tb.keys().filter_map(|k| k.split_once('_').and_then(|(n, _)| n.parse().ok())).collect();
    buttons.sort_unstable();
    buttons.dedup();
    let mut papers: HashMap<String, Option<u32>> = HashMap::new();
    let mut skipped_buttons = 0;
    for b in buttons {
        let get = |k: &str| tb.get(&format!("{b}_{k}"));
        let name = get("text").cloned().unwrap_or_else(|| format!("Brosse {b}"));
        let count: usize = get("count").and_then(|v| v.parse().ok()).unwrap_or(1);
        let cmds: Vec<&String> = (0..count.max(1)).filter_map(|k| get(&format!("cmd{k}"))).collect();
        let mut notes = Vec::new();
        let mut settings = None;
        for cmd in &cmds {
            if let Some(uuid) = cmd.strip_prefix("CBTool:") {
                let uuid = uuid.trim();
                let Some(ini_bytes) = panel.data.get(&format!("{uuid}.ini")) else {
                    notes.push("réglage de la brosse absent".into());
                    continue;
                };
                let ini = read_ini(&String::from_utf8_lossy(ini_bytes));
                let tip = panel.data.get(&format!("{uuid}.dip")).and_then(|d| crate::dip::read(d).ok());
                let mut s = custom_brush(&ini, tip.as_ref().map(|d| d.width.max(d.height)), &mut notes);
                if let Some(mut d) = tip {
                    let tool = ini.get("tvp tool");
                    let anim: i32 = tool.and_then(|t| t.get("anim")).and_then(|v| v.parse().ok()).unwrap_or(0);
                    let order = anim_order(anim);
                    // Sans animation, TVPaint pose l'image courante de la pointe (`currentimage`).
                    if order.is_none() && d.frames.len() > 1 {
                        let cur: usize = tool.and_then(|t| t.get("currentimage")).and_then(|v| v.parse().ok()).unwrap_or(0);
                        let keep = d.frames.swap_remove(cur.min(d.frames.len() - 1));
                        d.frames = vec![keep];
                    }
                    // Au plus 64 images (pointe trop large sinon) : prises à intervalles réguliers.
                    if d.frames.len() > 64 {
                        let n = d.frames.len();
                        d.frames = (0..64).map(|k| d.frames[k * (n - 1) / 63].clone()).collect();
                    }
                    let frames = d.frames.len();
                    let id = lib.add_tip(&name, crate::dip::tip_strip(&d));
                    report.tips += 1;
                    s.tip = Some(id);
                    if let (Some(order), true) = (order, frames > 1) {
                        s.tip_cells = frames as u32;
                        s.tip_order = order;
                        notes.push(format!("pointe animée de {frames} images (mode TVPaint {anim} pris comme « {} », à vérifier)", order.label()));
                    }
                } else {
                    notes.push("image de la pointe absente : pointe ronde".into());
                }
                settings = Some(s);
            } else if let Some(t) = cmd.strip_prefix("Tool:") {
                if let Some(s) = base_tool(t, &mut notes) {
                    settings = Some(s);
                }
            } else if cmd.contains("tv_paper") {
                if let (Some(p), Some(s)) = (paper_of(cmd), settings.as_mut()) {
                    let id = *papers.entry(p.name.clone()).or_insert_with(|| {
                        let img = res.and_then(|r| find_paper(r, &p.name))?;
                        report.papers += 1;
                        Some(lib.add_paper(&p.name, img))
                    });
                    match id {
                        Some(id) => {
                            s.paper = Some(id);
                            s.grain = (p.hardness / 100.0).clamp(0.0, 1.0);
                            s.paper_scale = p.size.clamp(0.1, 8.0);
                            s.paper_angle = p.angle;
                            s.paper_invert = p.invert;
                        }
                        None => notes.push(format!("papier « {} » introuvable", p.name)),
                    }
                }
            }
        }
        let Some(settings) = settings else {
            skipped_buttons += 1;
            continue;
        };
        let lower = name.to_lowercase();
        let target = if lower.contains("eraser") || lower.contains("gomme") { Target::Eraser } else { Target::Brush };
        lib.presets.push(Preset { name: name.clone(), target, settings, source: source.to_string() });
        report.presets += 1;
        notes.sort();
        notes.dedup();
        if !notes.is_empty() {
            report.notes.push(format!("{name} : {}", notes.join(", ")));
        }
    }
    if skipped_buttons > 0 {
        report.notes.push(format!("{skipped_buttons} bouton(s) sans outil de dessin (commandes, raccourcis) ignoré(s)"));
    }
    report
}

/// Importe un réglage `.ini` seul (avec sa pointe `.dip` du même nom à côté).
pub fn import_ini(path: &Path, source: &str, lib: &mut BrushLibrary) -> Result<TvpImport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("lecture impossible : {e}"))?;
    let stem = path.file_stem().map_or("brosse".into(), |s| s.to_string_lossy().into_owned());
    let mut data = HashMap::new();
    data.insert(format!("{stem}.ini"), text.into_bytes());
    if let Ok(d) = std::fs::read(path.with_extension("dip")) {
        data.insert(format!("{stem}.dip"), d);
    }
    let bin = format!("[toolbox]\n0_cmd0=CBTool:{stem}\n0_count=1\n0_text={stem}\n");
    Ok(import_panel(&Panel { bin, data }, source, lib, tvpaint_resources().as_deref()))
}
