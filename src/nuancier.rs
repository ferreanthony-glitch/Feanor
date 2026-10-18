//! Nuanciers personnels (passe couleur), sans interface : plusieurs nuanciers nommés, chacun une
//! suite de pastilles (couleur et nom) ; créer, renommer, dupliquer, supprimer ; ajouter, retirer,
//! nommer, remplacer et ranger les pastilles. Import et export aux formats de GIMP (`.gpl`, du
//! texte) et de Photoshop (`.aco`, binaire décrit par Adobe dans « Photoshop File Formats
//! Specification », section « Color Swatches »). Gardés dans `reglages.toml` (`[couleurs]`).

use std::path::Path;

use eframe::egui::{Pos2, Rect};
use serde::{Deserialize, Serialize};

use crate::couleur::{hsv_to_rgb, lab_to_rgb};

/// Nom du nuancier d'un premier lancement.
pub const NOM_DEFAUT: &str = "SHOT";
/// Son nom avant que le logiciel ne s'appelle SHOT : un nuancier gardé sous ce nom dans les
/// réglages est renommé à la lecture (`ColorSettings::sanitized`).
pub const NOM_ANCIEN: &str = "Pelure";

/// Une pastille : une couleur, et un nom facultatif (montré dans sa bulle).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swatch {
    #[serde(rename = "couleur")]
    pub color: [u8; 3],
    #[serde(default, rename = "nom", skip_serializing_if = "String::is_empty")]
    pub name: String,
}

impl Swatch {
    pub fn new(color: [u8; 3], name: &str) -> Swatch {
        Swatch { color, name: name.to_string() }
    }
}

/// Un nuancier nommé.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(default, rename = "pastilles")]
    pub swatches: Vec<Swatch>,
}

impl Palette {
    pub fn new(name: &str) -> Palette {
        Palette { name: name.to_string(), swatches: Vec::new() }
    }

    /// Le nuancier d'un premier lancement : les couleurs de l'ancien nuancier de SHOT (6b). Il
    /// s'appelait « Pelure » avant le changement de nom du logiciel (voir `NOM_ANCIEN`).
    pub fn pelure() -> Palette {
        let colors: [([u8; 3], &str); 10] = [
            ([24, 24, 28], "Encre"),
            ([255, 255, 255], "Blanc"),
            ([128, 128, 128], "Gris"),
            ([205, 45, 45], "Rouge"),
            ([240, 150, 40], "Orange"),
            ([245, 215, 60], "Jaune"),
            ([60, 160, 80], "Vert"),
            ([50, 110, 200], "Bleu"),
            ([130, 70, 170], "Violet"),
            ([235, 190, 160], "Peau"),
        ];
        Palette { name: NOM_DEFAUT.into(), swatches: colors.iter().map(|(c, n)| Swatch::new(*c, n)).collect() }
    }

    /// Rang de la première pastille de cette couleur.
    pub fn position(&self, c: [u8; 3]) -> Option<usize> {
        self.swatches.iter().position(|s| s.color == c)
    }

    /// Ajoute une pastille à la fin ; rien si la couleur y est déjà (son rang est rendu en
    /// `Err`).
    pub fn add(&mut self, c: [u8; 3], name: &str) -> Result<usize, usize> {
        if let Some(i) = self.position(c) {
            return Err(i);
        }
        self.swatches.push(Swatch::new(c, name));
        Ok(self.swatches.len() - 1)
    }
}

/// Déplace l'élément `from` dans la fente `slot` (0 : avant le premier, `len` : après le
/// dernier ; fentes comptées avant le déplacement). Renvoie son nouveau rang.
pub fn move_item<T>(v: &mut Vec<T>, from: usize, slot: usize) -> usize {
    if from >= v.len() {
        return from;
    }
    let to = (if slot > from { slot - 1 } else { slot }).min(v.len() - 1);
    let item = v.remove(from);
    v.insert(to, item);
    to
}

/// Fente visée en `p` parmi des pastilles posées en `rects` (dans l'ordre) : avant la pastille
/// la plus proche si l'on est sur sa moitié gauche, après sinon.
pub fn drop_slot(rects: &[Rect], p: Pos2) -> Option<usize> {
    let (j, r) = rects.iter().enumerate().min_by(|a, b| a.1.distance_sq_to_pos(p).total_cmp(&b.1.distance_sq_to_pos(p)))?;
    Some(if p.x < r.center().x { j } else { j + 1 })
}

/// Un nom pas encore pris : `base`, sinon « base 2 », « base 3 »…
pub fn unique_name(palettes: &[Palette], base: &str) -> String {
    let taken = |n: &str| palettes.iter().any(|p| p.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|k| format!("{base} {k}")).find(|n| !taken(n)).unwrap_or_default()
}

/// Nouveau nuancier vide, juste après `after` ; renvoie son rang.
pub fn create(palettes: &mut Vec<Palette>, after: usize) -> usize {
    let name = unique_name(palettes, "Nouveau nuancier");
    let at = (after + 1).min(palettes.len());
    palettes.insert(at, Palette::new(&name));
    at
}

/// Copie du nuancier `i`, juste après lui ; renvoie son rang.
pub fn duplicate(palettes: &mut Vec<Palette>, i: usize) -> Option<usize> {
    let mut copy = palettes.get(i)?.clone();
    copy.name = unique_name(palettes, &format!("{} (copie)", copy.name));
    palettes.insert(i + 1, copy);
    Some(i + 1)
}

/// Supprime le nuancier `i` ; il en reste toujours un (vide si c'était le dernier). Renvoie le
/// rang du nuancier à montrer ensuite.
pub fn delete(palettes: &mut Vec<Palette>, i: usize) -> usize {
    if i < palettes.len() {
        palettes.remove(i);
    }
    if palettes.is_empty() {
        palettes.push(Palette::new("Nuancier"));
    }
    i.saturating_sub(1).min(palettes.len() - 1)
}

/// Renomme le nuancier `i` (nom sans espaces autour) ; refusé si vide ou déjà pris.
pub fn rename(palettes: &mut [Palette], i: usize, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("un nuancier a besoin d'un nom".into());
    }
    if palettes.iter().enumerate().any(|(k, p)| k != i && p.name == name) {
        return Err(format!("un nuancier s'appelle déjà « {name} »"));
    }
    let p = palettes.get_mut(i).ok_or("nuancier introuvable")?;
    p.name = name.to_string();
    Ok(())
}

// ------------------------------------------------------------ GIMP (.gpl)

/// Nuancier au format de GIMP : « GIMP Palette », « Name: », « Columns: », « # », puis une ligne
/// par couleur : rouge, vert, bleu (0 à 255), tabulation, nom.
pub fn to_gpl(p: &Palette) -> String {
    let mut out = format!("GIMP Palette\nName: {}\nColumns: 8\n#\n", p.name.replace(['\r', '\n'], " "));
    for s in &p.swatches {
        let [r, g, b] = s.color;
        out += &format!("{r:3} {g:3} {b:3}");
        let name = s.name.replace(['\r', '\n', '\t'], " ");
        if !name.is_empty() {
            out += &format!("\t{name}");
        }
        out.push('\n');
    }
    out
}

/// Ce qu'a donné un import : le nuancier (son nom, s'il en avait un) et le nombre de couleurs
/// laissées de côté (lignes illisibles, espaces colorimétriques inconnus).
#[derive(Debug, PartialEq)]
pub struct Imported {
    pub palette: Palette,
    pub skipped: usize,
}

/// Lit un nuancier GIMP. Le nom d'une couleur est le reste de sa ligne (jusqu'à une tabulation
/// de plus, que certains logiciels ajoutent) ; « Untitled », le nom par défaut de GIMP, compte
/// comme pas de nom.
pub fn parse_gpl(text: &str) -> Result<Imported, String> {
    let mut lines = text.trim_start_matches('\u{feff}').lines().map(str::trim_end).filter(|l| !l.trim().is_empty());
    if !lines.next().is_some_and(|l| l.trim().starts_with("GIMP Palette")) {
        return Err("ce n'est pas un nuancier GIMP (il manque « GIMP Palette » en tête)".into());
    }
    let mut out = Imported { palette: Palette::new(""), skipped: 0 };
    for line in lines {
        let t = line.trim_start();
        if t.starts_with('#') {
            continue;
        }
        if let Some(name) = t.strip_prefix("Name:") {
            out.palette.name = name.trim().to_string();
            continue;
        }
        if t.starts_with("Columns:") {
            continue;
        }
        let mut rest = t;
        let mut rgb = [0u8; 3];
        let mut ok = true;
        for v in &mut rgb {
            rest = rest.trim_start();
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            match rest[..end].parse::<u8>() {
                Ok(x) => *v = x,
                Err(_) => ok = false,
            }
            rest = &rest[end..];
        }
        if !ok {
            out.skipped += 1;
            continue;
        }
        let name = rest.trim_start().split('\t').next().unwrap_or("").trim();
        let name = if name == "Untitled" { "" } else { name };
        out.palette.swatches.push(Swatch::new(rgb, name));
    }
    Ok(out)
}

// ------------------------------------------------------------ Photoshop (.aco)

/// Nuancier au format de Photoshop : la section de la version 1 (couleurs seules), puis celle de
/// la version 2 (les mêmes, avec leurs noms), comme Photoshop l'écrit. Nombres en 16 bits,
/// octet de poids fort d'abord ; couleurs en RVB (espace 0, de 0 à 65535).
pub fn to_aco(p: &Palette) -> Vec<u8> {
    let n = p.swatches.len().min(u16::MAX as usize);
    let mut out = Vec::new();
    let put16 = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_be_bytes());
    for version in [1u16, 2] {
        put16(&mut out, version);
        put16(&mut out, n as u16);
        for s in &p.swatches[..n] {
            put16(&mut out, 0);
            for c in s.color {
                put16(&mut out, c as u16 * 257);
            }
            put16(&mut out, 0);
            if version == 2 {
                let name: Vec<u16> = s.name.encode_utf16().chain(std::iter::once(0)).collect();
                out.extend_from_slice(&(name.len() as u32).to_be_bytes());
                for u in name {
                    put16(&mut out, u);
                }
            }
        }
    }
    out
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.data.get(self.pos..self.pos + 2).ok_or("fichier .aco tronqué")?;
        self.pos += 2;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(((self.u16()? as u32) << 16) | self.u16()? as u32)
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
}

/// Une couleur d'un fichier .aco en RVB, selon son espace : 0 RVB, 1 TSV (« HSB »), 2 CMJN
/// (0 = 100 % d'encre ; conversion simple, sans profil), 7 Lab (L de 0 à 10000, a et b signés
/// en centièmes ; blanc D50), 8 niveaux de gris (0 à 10000, 10000 = noir, lu comme GIMP le
/// lit). Les autres (nuanciers de fabricants) : `None`.
pub fn aco_color(space: u16, w: u16, x: u16, y: u16, z: u16) -> Option<[u8; 3]> {
    let f = |v: u16| v as f32 / 65535.0;
    let byte = |t: f32| (t * 255.0).round().clamp(0.0, 255.0) as u8;
    match space {
        0 => Some([w, x, y].map(|v| byte(f(v)))),
        1 => Some(hsv_to_rgb([f(w) * 360.0, f(x), f(y)])),
        2 => Some([w, x, y].map(|v| byte(f(v) * f(z)))),
        7 => Some(lab_to_rgb(w as f32 / 100.0, x as i16 as f32 / 100.0, y as i16 as f32 / 100.0)),
        8 => Some([byte(1.0 - w.min(10000) as f32 / 10000.0); 3]),
        _ => None,
    }
}

fn read_section(r: &mut Reader, version: u16) -> Result<Imported, String> {
    if version != 1 && version != 2 {
        return Err(format!("fichier .aco de version {version} inconnue"));
    }
    let mut out = Imported { palette: Palette::new(""), skipped: 0 };
    let n = r.u16()?;
    for _ in 0..n {
        let space = r.u16()?;
        let (w, x, y, z) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
        let mut name = String::new();
        if version == 2 {
            let len = r.u32()? as usize;
            if len * 2 > r.remaining() {
                return Err("nom de couleur tronqué dans le fichier .aco".into());
            }
            let units: Vec<u16> = (0..len).map(|_| r.u16()).collect::<Result<_, _>>()?;
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            name = String::from_utf16_lossy(&units[..end]);
        }
        match aco_color(space, w, x, y, z) {
            Some(c) => out.palette.swatches.push(Swatch::new(c, &name)),
            None => out.skipped += 1,
        }
    }
    Ok(out)
}

/// Lit un nuancier Photoshop : la section de la version 2 (avec les noms) si elle suit celle de
/// la version 1, sinon la première.
pub fn parse_aco(data: &[u8]) -> Result<Imported, String> {
    let mut r = Reader { data, pos: 0 };
    let version = r.u16()?;
    let first = read_section(&mut r, version)?;
    if version == 1 && r.remaining() >= 4 && r.u16() == Ok(2) {
        if let Ok(named) = read_section(&mut r, 2) {
            return Ok(named);
        }
    }
    Ok(first)
}

// ------------------------------------------------------------ fichiers

/// Lit un fichier de nuancier : `.gpl` (GIMP) ou `.aco` (Photoshop), d'après son extension, ou
/// d'après son contenu si l'extension est autre. Sans nom dans le fichier, le nuancier prend
/// celui du fichier.
pub fn read_file(path: &Path) -> Result<Imported, String> {
    let data = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let gpl = match ext.as_str() {
        "gpl" => true,
        "aco" => false,
        _ => data.trim_ascii_start().starts_with(b"GIMP Palette") || data.starts_with("\u{feff}GIMP".as_bytes()),
    };
    let mut imported = if gpl { parse_gpl(&String::from_utf8_lossy(&data))? } else { parse_aco(&data)? };
    if imported.palette.name.trim().is_empty() {
        imported.palette.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Nuancier".into());
    }
    Ok(imported)
}

/// Écrit un nuancier : en `.aco` si le chemin finit par `.aco`, sinon en `.gpl` (ajouté s'il
/// manque). Renvoie le chemin écrit.
pub fn write_file(path: &Path, p: &Palette) -> Result<std::path::PathBuf, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let (path, data) = match ext.as_str() {
        "aco" => (path.to_path_buf(), to_aco(p)),
        "gpl" => (path.to_path_buf(), to_gpl(p).into_bytes()),
        _ => {
            let mut s = path.as_os_str().to_owned();
            s.push(".gpl");
            (std::path::PathBuf::from(s), to_gpl(p).into_bytes())
        }
    };
    std::fs::write(&path, data).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(path)
}

/// Nom de fichier proposé pour un nuancier (sans les caractères que Windows refuse).
pub fn file_name(p: &Palette) -> String {
    let base: String = p.name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let base = base.trim().trim_end_matches('.');
    if base.is_empty() { "nuancier".to_string() } else { base.to_string() }
}
