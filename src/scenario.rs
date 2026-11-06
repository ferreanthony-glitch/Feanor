//! Importer un scénario (Anthony, 2026-10-19) : lecture du texte d'un scénario en une suite
//! d'éléments (intitulés de scène, action, répliques, transitions, notes), sans rien savoir du
//! storyboard. `scenarioplans` en fait des séquences et des plans, `scenarioui` montre la fenêtre.
//!
//! Formats lus :
//! - **Fountain** (`.fountain`, `.spmd`) : intitulés `INT.` / `EXT.`, personnages en majuscules,
//!   répliques et parenthèses, transitions (`CUT TO:`), dialogues simultanés (`^`), notes `[[ ]]`,
//!   cimetière `/* */`, sections `#`, synopsis `=`, centré `> <`, éléments forcés (`.` `!` `@` `>` `~`),
//!   page de titre ;
//! - **texte brut** (`.txt`, et tout ce qui n'est pas reconnu) : les mêmes repères, déduits de la
//!   forme (intitulés, personnage en majuscules suivi de sa réplique, « NOM : réplique », texte copié
//!   d'un PDF avec ses numéros de page) ;
//! - **Final Draft** (`.fdx`, du XML) et **Word** (`.docx`, un zip, lu par `zipread`) : voir
//!   `scenariofdx`.
//!
//! Le PDF n'est pas lu (pas de lecteur de PDF hors connexion dans SHOT) : le message le dit et
//! propose d'exporter en `.fountain` ou `.txt`. Rien ici ne panique sur un fichier abîmé : une
//! erreur en français, sinon un `Script`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Plus gros fichier lu (un scénario de 120 pages pèse 200 Ko).
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Nom de la séquence qui reçoit ce qui précède le premier intitulé de scène.
pub const OPENING: &str = "Ouverture";

/// Le format dans lequel le fichier a été lu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Fountain,
    #[default]
    Texte,
    Fdx,
    Docx,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Fountain => "Fountain",
            Format::Texte => "Texte brut",
            Format::Fdx => "Scénario (.fdx)",
            Format::Docx => "Document (.docx)",
        }
    }
}

/// Une réplique : un personnage, ses indications (`V.O.`), ses parenthèses de jeu, son texte.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speech {
    pub name: String,
    /// Extensions du nom, sans parenthèses : `V.O.`, `O.S.` (« CONT'D » est retiré).
    pub tags: Vec<String>,
    /// Parenthèses de jeu, avec leurs parenthèses : `(bas)`.
    pub asides: Vec<String>,
    pub text: String,
    /// Dit en même temps que la réplique d'avant (Fountain : `^` après le nom).
    pub dual: bool,
}

impl Speech {
    /// La ligne du champ « dialogue » : `MARIE (V.O.) (bas) : Viens ici.` Le lecteur de sons par
    /// règles prend le nom avant les deux-points (parenthèses comprises dans le nom, retirées par
    /// lui) et lit le reste comme réplique : les indications de jeu ne sont donc pas prononcées.
    pub fn line(&self) -> String {
        let mut head = self.name.clone();
        for t in &self.tags {
            head.push_str(&format!(" ({t})"));
        }
        for a in &self.asides {
            head.push(' ');
            head.push_str(a);
        }
        let head = head.trim().to_string();
        if self.name.is_empty() {
            return self.text.clone();
        }
        if self.text.is_empty() {
            return format!("{head} :");
        }
        format!("{head} : {}", self.text)
    }

    /// Nombre de mots de la réplique (sans le nom ni les parenthèses de jeu).
    pub fn words(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

/// Ce qu'un élément du scénario est.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Intitulé de scène : `INT. CUISINE – NUIT`.
    Heading(String),
    /// Action (plusieurs lignes séparées par `\n`).
    Action(String),
    Dialogue(Speech),
    Transition(String),
    /// Titre de section (`# Acte I`).
    Section(String),
    /// Synopsis (`= Marie cherche sa clé`).
    Synopsis(String),
    /// Note seule sur sa ligne (`[[ à vérifier ]]`).
    Note(String),
}

/// Un élément et les notes `[[ ]]` qui étaient dans son texte.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// (Intitulé de scène seulement.) Le numéro de scène que le fichier écrit (« 12 », « 12A »), vide s'il n'en a pas. Jamais écrit s'il est vide :
    /// un scénario gardé avant ce champ se relit comme avant.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub numero: String,
    /// (Intitulé de scène seulement.) L'intitulé tel que le fichier l'écrit, sans numéro ni marques de mise en forme (« INT. JOUR - CUISINE ») ;
    /// vide pour un scénario gardé avant ce champ (le nom aux tirets longs sert alors).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ecriture: String,
}

impl Element {
    pub fn new(kind: Kind) -> Element {
        Element { kind, notes: Vec::new(), numero: String::new(), ecriture: String::new() }
    }
}

/// Un scénario lu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Script {
    pub format: Format,
    pub title: String,
    pub author: String,
    pub elements: Vec<Element>,
    /// Ce qui n'a pas pu être repris ou qui mérite un mot, en français.
    pub warnings: Vec<String>,
}

impl Default for Script {
    fn default() -> Script {
        Script::empty(Format::Texte)
    }
}

impl Script {
    pub fn empty(format: Format) -> Script {
        Script { format, title: String::new(), author: String::new(), elements: Vec::new(), warnings: Vec::new() }
    }

    /// Nombre d'intitulés de scène.
    pub fn headings(&self) -> usize {
        self.elements.iter().filter(|e| matches!(e.kind, Kind::Heading(_))).count()
    }
}

/// Un personnage trouvé : son nom (majuscules), ses répliques et leurs mots.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub name: String,
    pub lines: usize,
    pub words: usize,
}

/// Les personnages qui parlent, dans l'ordre de leur première réplique. Un nom sans réplique
/// (action en majuscules prise pour un nom ?) n'y est pas ; les noms vides non plus.
pub fn characters(elements: &[Element]) -> Vec<Character> {
    let mut out: Vec<Character> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in elements {
        let Kind::Dialogue(s) = &e.kind else { continue };
        if s.name.is_empty() {
            continue;
        }
        let key = s.name.to_uppercase();
        let i = *index.entry(key).or_insert_with(|| {
            out.push(Character { name: s.name.clone(), lines: 0, words: 0 });
            out.len() - 1
        });
        out[i].lines += 1;
        out[i].words += s.words();
    }
    out
}

// ------------------------------------------------------------------------------------ fichiers

/// Lit le fichier `path` (le format se déduit de l'extension, puis du contenu). `progress` reçoit
/// l'avancement (0 à 1).
pub fn read_file(path: &Path, progress: &dyn Fn(f32)) -> Result<Script, String> {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let meta = std::fs::metadata(path).map_err(|e| format!("Impossible de lire « {name} » : {e}."))?;
    if !meta.is_file() {
        return Err(format!("« {name} » n'est pas un fichier."));
    }
    if meta.len() > MAX_BYTES {
        return Err(format!("« {name} » est trop gros pour un scénario ({} Mo, au plus {} Mo).", meta.len() / (1024 * 1024), MAX_BYTES / (1024 * 1024)));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Impossible de lire « {name} » : {e}."))?;
    progress(0.05);
    let ext = path.extension().map_or_else(String::new, |e| e.to_string_lossy().to_ascii_lowercase());
    parse_bytes(&bytes, &ext, progress)
}

/// Lit les octets d'un fichier d'extension `ext` (en minuscules, sans point). Un plantage du
/// lecteur lui-même (qui ne devrait pas arriver) est rattrapé et devient une erreur.
pub fn parse_bytes(bytes: &[u8], ext: &str, progress: &dyn Fn(f32)) -> Result<Script, String> {
    let run = std::panic::AssertUnwindSafe(|| parse_bytes_inner(bytes, ext, progress));
    match std::panic::catch_unwind(run) {
        Ok(r) => r,
        Err(_) => Err("Ce fichier a mis le lecteur de scénarios en échec (erreur inattendue). Exportez le scénario en .fountain ou .txt et réessayez.".into()),
    }
}

const PDF_MESSAGE: &str = "Les fichiers PDF ne sont pas lus (FEANOR n'a pas de lecteur de PDF hors connexion). Exporte le scénario en .fountain ou .txt depuis ton logiciel d'écriture, ou copiez le texte du PDF dans un fichier .txt.";

fn parse_bytes_inner(bytes: &[u8], ext: &str, progress: &dyn Fn(f32)) -> Result<Script, String> {
    if bytes.is_empty() {
        return Err("Le fichier est vide.".into());
    }
    if bytes.starts_with(b"%PDF") || ext == "pdf" {
        return Err(PDF_MESSAGE.into());
    }
    match ext {
        "doc" => return Err("Le format .doc (ancien) n'est pas lu : enregistrez le scénario en .docx, .fountain ou .txt.".into()),
        "rtf" => return Err("Le format .rtf n'est pas lu : enregistrez le scénario en .docx, .fountain ou .txt.".into()),
        "odt" | "pages" => return Err(format!("Le format .{ext} n'est pas lu : enregistrez le scénario en .docx, .fountain ou .txt.")),
        _ => {}
    }
    let zip = bytes.starts_with(b"PK\x03\x04");
    let mut script = if ext == "docx" || (zip && ext != "fdx") {
        crate::scenariofdx::parse_docx(bytes, progress)?
    } else {
        let text = decode_text(bytes);
        if looks_binary(&text) {
            return Err("Ce fichier ne ressemble pas à du texte : ce n'est pas un scénario lisible (formats lus : .fountain, .txt, .fdx, .docx).".into());
        }
        let head = text.trim_start();
        if ext == "fdx" || head.starts_with("<?xml") || head.starts_with("<FinalDraft") {
            crate::scenariofdx::parse_fdx(&text, progress)?
        } else if matches!(ext, "fountain" | "spmd" | "ftn") {
            parse_fountain(&text, progress)
        } else {
            parse_text(&text, TextOptions::default(), progress)
        }
    };
    let has_content = script.elements.iter().any(|e| matches!(e.kind, Kind::Heading(_) | Kind::Action(_) | Kind::Dialogue(_) | Kind::Transition(_)));
    if !has_content {
        return Err(format!(
            "Aucun scénario reconnu dans ce fichier ({} lu, mais ni scène, ni action, ni réplique). Vérifiez le format, ou exportez en .fountain ou .txt.",
            script.format.label()
        ));
    }
    if script.headings() == 0 {
        script.warnings.push("Aucun intitulé de scène (INT. / EXT.) reconnu : tout le texte forme une seule séquence.".into());
    }
    progress(1.0);
    Ok(script)
}

/// Le texte d'un fichier : UTF-8 (avec ou sans marque), UTF-16 (avec marque), sinon Windows-1252
/// (les fichiers Word et Final Draft anciens).
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.len() >= 2 && (bytes[0..2] == [0xFF, 0xFE] || bytes[0..2] == [0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [u16; 32] = [
        0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018,
        0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
    ];
    match b {
        0x80..=0x9F => char::from_u32(HIGH[(b - 0x80) as usize] as u32).unwrap_or('?'),
        _ => b as char,
    }
}

/// Beaucoup de caractères nuls ou de contrôle : un fichier binaire.
fn looks_binary(text: &str) -> bool {
    let sample: Vec<char> = text.chars().take(8192).collect();
    if sample.is_empty() {
        return false;
    }
    let bad = sample.iter().filter(|c| **c == '\u{0}' || (c.is_control() && !matches!(**c, '\n' | '\r' | '\t' | '\u{c}'))).count();
    bad * 50 > sample.len()
}

// ------------------------------------------------------------------------------------ repères

fn is_upper(s: &str) -> bool {
    s.chars().any(|c| c.is_alphabetic()) && !s.chars().any(|c| c.is_lowercase())
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Retire les marques de mise en forme de Fountain : `*italique*`, `**gras**`, `_souligné_`.
pub fn strip_emphasis(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        i += 1;
        match c {
            '*' => {
                // « 3 * 4 » reste tel quel.
                if prev.is_some_and(char::is_whitespace) && next.is_none_or(char::is_whitespace) {
                    out.push(c);
                }
            }
            '_' => {
                if prev.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphanumeric) {
                    out.push(c);
                }
            }
            // Un caractère échappé (`\*`) reste, sans sa barre.
            '\\' if next.is_some_and(|n| matches!(n, '*' | '_' | '\\' | '[' | ']' | '#' | '=' | '>' | '<' | '.' | '~' | '@' | '!')) => {
                out.push(chars[i]);
                i += 1;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Début d'un intitulé de scène : (préfixe, ne vaut que tout en majuscules).
/// Un préfixe suivi d'un point est sûr ; suivi d'une espace, il faut que la ligne soit en majuscules
/// (« Est-ce que… » ou « Ext » ne font pas un intitulé).
const HEADING_PREFIXES: [&str; 8] = ["INT./EXT", "INT/EXT", "EXT./INT", "EXT/INT", "I/E", "INT", "EXT", "EST"];

/// Un intitulé de scène lu : son numéro de scène tel que le fichier l'écrit (vide s'il n'en a pas), l'intitulé tel qu'il est écrit
/// (« INT. JOUR - CUISINE » : mêmes mots, mêmes tirets, sans numéro de scène ni marques de mise en forme) et son nom, aux tirets longs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnTete {
    pub numero: String,
    pub ecriture: String,
    pub nom: String,
}

impl EnTete {
    /// L'élément du scénario qui le garde.
    pub fn element(self, notes: Vec<String>) -> Element {
        Element { kind: Kind::Heading(self.nom), notes, numero: self.numero, ecriture: self.ecriture }
    }
}

/// Le texte d'un intitulé de scène (sans numéro de scène ni point forcé) si la ligne en est un.
pub fn heading_of(line: &str) -> Option<String> {
    en_tete_of(line).map(|h| h.nom)
}

/// L'intitulé de scène d'une ligne, avec son numéro de scène et son écriture d'origine, si la ligne en est un.
pub fn en_tete_of(line: &str) -> Option<EnTete> {
    let mut t = line.trim();
    if t.is_empty() || t.chars().count() > 200 {
        return None;
    }
    let forced = t.starts_with('.') && !t.starts_with("..");
    let mut numero = String::new();
    if forced {
        t = t[1..].trim();
    } else {
        // Numéro de scène en tête : « 12. INT. CUISINE » ou « 12A INT. CUISINE ».
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 {
            let mut rest = &t[digits..];
            let mut fin = digits;
            // Une lettre de bis : « 12A ».
            if rest.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && rest[1..].starts_with(['.', ' ', '\t']) {
                rest = &rest[1..];
                fin += 1;
            }
            let rest = rest.trim_start_matches(['.', ' ', '\t']);
            if rest.len() < t.len() && is_heading_start(rest) {
                numero = t[..fin].to_string();
                t = rest;
            }
        }
        if !is_heading_start(t) {
            return None;
        }
    }
    if t.is_empty() {
        return None;
    }
    let h = en_tete_depuis(t, numero);
    (!h.nom.is_empty()).then_some(h)
}

/// Un intitulé de scène dont `numero` est le numéro déjà lu en tête de ligne (ou l'attribut du fichier), vide s'il n'y en a pas : le
/// numéro de fin de ligne (`#12#`, ou « 12 » après plusieurs espaces) le remplace alors, s'il y en a un.
pub fn en_tete_depuis(t: &str, numero: String) -> EnTete {
    let mut name = t.trim().to_string();
    let mut en_fin = String::new();
    if name.ends_with('#') {
        if let Some(open) = name[..name.len() - 1].rfind('#') {
            let marque = name[open + 1..name.len() - 1].trim();
            if !marque.is_empty() && marque.chars().count() <= 10 && !marque.contains(char::is_whitespace) {
                en_fin = marque.to_string();
            }
            name.truncate(open);
        }
    }
    let trimmed = name.trim_end();
    if let Some(sp) = trimmed.rfind(['\t', ' ']) {
        let last = &trimmed[sp + 1..];
        let gap = trimmed[..sp].ends_with([' ', '\t']);
        if gap && !last.is_empty() && last.len() <= 4 && last.chars().next().is_some_and(|c| c.is_ascii_digit()) && last.chars().all(|c| c.is_ascii_alphanumeric()) {
            if en_fin.is_empty() {
                en_fin = last.to_string();
            }
            name = trimmed[..sp].to_string();
        }
    }
    let ecriture = collapse_spaces(&strip_emphasis(&name));
    let nom = ecriture.replace(" -- ", " – ").replace(" - ", " – ").replace(" — ", " – ");
    let numero = if numero.trim().is_empty() { en_fin } else { numero.trim().to_string() };
    EnTete { numero, ecriture, nom }
}
fn is_heading_start(t: &str) -> bool {
    let up = t.to_uppercase();
    for p in HEADING_PREFIXES {
        if let Some(rest) = up.strip_prefix(p) {
            if rest.starts_with('.') {
                return true;
            }
            if (rest.starts_with(' ') || rest.starts_with('\t')) && is_upper(t) {
                return true;
            }
        }
    }
    for word in ["INTÉRIEUR", "EXTÉRIEUR", "INTERIEUR", "EXTERIEUR"] {
        if let Some(rest) = up.strip_prefix(word) {
            if (rest.starts_with(' ') || rest.starts_with('.') || rest.starts_with('-')) && is_upper(t) {
                return true;
            }
        }
    }
    false
}

/// Une transition : `CUT TO:`, `FONDU AU NOIR.`, `COUPÉ À :`… (en majuscules, courte).
pub fn is_transition(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.chars().count() > 45 || !is_upper(t) {
        return false;
    }
    let u = t.to_uppercase();
    if u.ends_with("TO:") || u.ends_with("TO :") || (u.starts_with("COUP") && (u.ends_with("À :") || u.ends_with("A :"))) {
        return true;
    }
    const STARTS: [&str; 15] =
        ["FADE", "FONDU", "CUT TO", "COUPE", "COUPÉ", "DISSOLVE", "ENCHAINÉ", "ENCHAÎNÉ", "ENCHAINE", "SMASH", "MATCH CUT", "IRIS", "VOLET", "WIPE", "SUR NOIR"];
    STARTS.iter().any(|s| {
        u.strip_prefix(s).is_some_and(|rest| !rest.chars().next().is_some_and(char::is_alphabetic))
            && (u.ends_with('.') || u.ends_with(':') || u.split_whitespace().count() <= 4)
    })
}

/// Un nom de personnage lu sur une ligne : `MARIE`, `MARIE (V.O.)`, `MARIE ^`, `@Mc Donald`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub name: String,
    pub tags: Vec<String>,
    pub dual: bool,
}

/// « CONT'D » et ses cousins : la suite d'une réplique, pas une indication de son.
fn is_continued(tag: &str) -> bool {
    let k: String = tag.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase();
    matches!(k.as_str(), "CONTD" | "CONT" | "CONTINUED" | "CONTINUING" | "SUITE" | "SUITED")
}

/// Étiquettes en majuscules qui ouvrent une consigne, jamais un personnage.
const NOT_NAMES: [&str; 20] = [
    "SFX", "FX", "VFX", "SON", "SONS", "BRUITAGE", "BRUITAGES", "AMBIANCE", "AMBIANCES", "MUSIQUE", "NOTE", "NOTES", "ACTION", "TITRE", "NB", "DÉCOR",
    "DECOR", "LIEU", "TRANSITION", "SYNOPSIS",
];

/// Lit une ligne de personnage. `forced` : `@` ou type déjà connu (Final Draft) : toute casse
/// passe. Sinon il faut des majuscules, peu de mots et pas de ponctuation de phrase.
pub fn parse_cue(raw: &str, forced: bool) -> Option<Cue> {
    let mut s = raw.trim();
    let mut forced = forced;
    if let Some(r) = s.strip_prefix('@') {
        s = r.trim_start();
        forced = true;
    }
    let mut dual = false;
    if let Some(r) = s.strip_suffix('^') {
        s = r.trim_end();
        dual = true;
    }
    let (name_part, mut rest) = match s.find('(') {
        Some(i) => (s[..i].trim(), &s[i..]),
        None => (s, ""),
    };
    let mut tags = Vec::new();
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let inner = rest.strip_prefix('(')?;
        let close = inner.find(')')?;
        let tag = collapse_spaces(&inner[..close]);
        if !tag.is_empty() && !is_continued(&tag) {
            tags.push(tag);
        }
        rest = &inner[close + 1..];
    }
    let name = collapse_spaces(&name_part.replace(':', " "));
    if name.is_empty() || !name.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    if !forced {
        let words = name.split_whitespace().count();
        if !is_upper(&name) || words > 5 || name.chars().count() > 40 || name.ends_with(['.', '!', '?', ',', ';']) {
            return None;
        }
        // Les étiquettes de consignes (« SFX : … ») ne sont pas des personnages.
        if NOT_NAMES.contains(&name.as_str()) {
            return None;
        }
    }
    Some(Cue { name, tags, dual })
}

fn speech_from(cue: Cue, rest: &[String]) -> Speech {
    let mut text = Vec::new();
    let mut asides = Vec::new();
    for l in rest {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with('(') && t.ends_with(')') {
            asides.push(collapse_spaces(t));
        } else {
            text.push(t.to_string());
        }
    }
    Speech { name: cue.name, tags: cue.tags, asides, text: strip_emphasis(&collapse_spaces(&text.join(" "))), dual: cue.dual }
}

// ------------------------------------------------------------------------------------ Fountain

const NOTE_OPEN: char = '\u{E000}';
const NOTE_CLOSE: char = '\u{E001}';

/// Retire les cimetières `/* … */` (un cimetière non refermé emporte la fin du texte).
fn strip_boneyard(text: &str, warnings: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(j) => rest = &rest[i + 2 + j + 2..],
            None => {
                warnings.push("Un commentaire « /* … */ » n'est pas refermé : la fin du texte est ignorée.".into());
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Une note `[[ ]]` ne contient pas de ligne vide.
fn has_blank_line(s: &str) -> bool {
    let lines: Vec<&str> = s.split('\n').collect();
    lines.len() > 2 && lines[1..lines.len() - 1].iter().any(|l| l.trim().is_empty())
}

/// Sort les notes `[[ ]]` du texte : chacune devient une marque (`NOTE_OPEN` n `NOTE_CLOSE`).
fn lift_notes(text: &str, notes: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("[[") {
        let after = &rest[i + 2..];
        let end = after.find("]]").filter(|&e| !has_blank_line(&after[..e]));
        match end {
            Some(e) => {
                out.push_str(&rest[..i]);
                notes.push(collapse_spaces(&after[..e]));
                out.push(NOTE_OPEN);
                out.push_str(&(notes.len() - 1).to_string());
                out.push(NOTE_CLOSE);
                rest = &after[e + 2..];
            }
            None => {
                out.push_str(&rest[..i + 2]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Une ligne sans ses marques de notes, et les notes qu'elle portait.
struct Ln {
    text: String,
    notes: Vec<String>,
}

impl Ln {
    fn new(line: &str, notes: &[String]) -> Ln {
        if !line.contains(NOTE_OPEN) {
            return Ln { text: line.trim_end().to_string(), notes: Vec::new() };
        }
        let mut text = String::new();
        let mut found = Vec::new();
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            if c != NOTE_OPEN {
                text.push(c);
                continue;
            }
            let mut digits = String::new();
            for d in chars.by_ref() {
                if d == NOTE_CLOSE {
                    break;
                }
                digits.push(d);
            }
            if let Some(n) = digits.parse::<usize>().ok().and_then(|i| notes.get(i)) {
                if !n.is_empty() {
                    found.push(n.clone());
                }
            }
        }
        let had = !text.trim().is_empty();
        let text = if had { text.trim_end().to_string() } else { String::new() };
        // Une note au milieu d'une phrase laisse une double espace.
        let text = if had && text.contains("  ") && !text.starts_with(' ') { collapse_spaces(&text) } else { text };
        Ln { text, notes: found }
    }

    fn blank(&self) -> bool {
        self.text.trim().is_empty() && self.notes.is_empty()
    }

    fn only_notes(&self) -> bool {
        self.text.trim().is_empty() && !self.notes.is_empty()
    }
}

const TITLE_KEYS: [&str; 24] = [
    "title", "credit", "author", "authors", "source", "draft date", "date", "contact", "copyright", "notes", "revision", "watermark", "font", "tl", "tr", "bl",
    "br", "header", "footer", "titre", "auteur", "auteurs", "crédit", "version",
];

fn title_key(line: &str) -> Option<(String, String)> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let (k, v) = line.split_once(':')?;
    let key = k.trim().to_lowercase();
    TITLE_KEYS.contains(&key.as_str()).then(|| (key, v.trim().to_string()))
}

/// La page de titre (« Title: … », « Author: … ») au début du texte : (première ligne après elle,
/// titre, auteur).
fn title_page(lines: &[Ln]) -> Option<(usize, String, String)> {
    let first = lines.iter().position(|l| !l.blank())?;
    title_key(&lines[first].text)?;
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut i = first;
    while i < lines.len() && !lines[i].blank() {
        let t = &lines[i].text;
        match title_key(t) {
            Some((k, v)) => entries.push((k, v)),
            None => {
                if let Some(last) = entries.last_mut() {
                    if !last.1.is_empty() {
                        last.1.push(' ');
                    }
                    last.1.push_str(t.trim());
                }
            }
        }
        i += 1;
    }
    let get = |keys: &[&str]| {
        entries
            .iter()
            .find(|(k, v)| keys.contains(&k.as_str()) && !v.is_empty())
            .map(|(_, v)| collapse_spaces(&strip_emphasis(v)))
            .unwrap_or_default()
    };
    Some((i, get(&["title", "titre"]), get(&["author", "authors", "auteur", "auteurs"])))
}

/// Lit un scénario Fountain.
pub fn parse_fountain(text: &str, progress: &dyn Fn(f32)) -> Script {
    let mut script = Script::empty(Format::Fountain);
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let text = strip_boneyard(&text, &mut script.warnings);
    let mut notes = Vec::new();
    let text = lift_notes(&text, &mut notes);
    let lines: Vec<Ln> = text.split('\n').map(|l| Ln::new(l, &notes)).collect();
    let mut i = 0;
    if let Some((end, title, author)) = title_page(&lines) {
        script.title = title;
        script.author = author;
        i = end;
    }
    let total = lines.len().max(1);
    let mut out: Vec<Element> = Vec::new();
    let mut last_report = 0;
    while i < lines.len() {
        if lines[i].blank() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < lines.len() && !lines[j].blank() {
            j += 1;
        }
        // Les notes du paragraphe ; les lignes qui n'ont que des notes n'en font pas partie.
        let mut para_notes: Vec<String> = Vec::new();
        let mut content: Vec<String> = Vec::new();
        for l in &lines[i..j] {
            para_notes.extend(l.notes.iter().cloned());
            if !l.only_notes() {
                content.push(l.text.clone());
            }
        }
        let before = out.len();
        if content.is_empty() {
            out.extend(para_notes.drain(..).map(|n| Element::new(Kind::Note(n))));
        } else {
            fountain_paragraph(&content, &mut out);
            if let Some(first) = out.get_mut(before) {
                first.notes = para_notes;
            }
        }
        i = j;
        if i - last_report > 400 {
            last_report = i;
            progress(0.1 + 0.85 * i as f32 / total as f32);
        }
    }
    script.elements = out;
    script
}

/// Les éléments d'un paragraphe de Fountain (lignes sans ligne vide entre elles).
fn fountain_paragraph(lines: &[String], out: &mut Vec<Element>) {
    let mut action: Vec<String> = Vec::new();
    let flush = |action: &mut Vec<String>, out: &mut Vec<Element>| {
        if !action.is_empty() {
            let text = strip_emphasis(&action.join("\n"));
            out.push(Element::new(Kind::Action(text)));
            action.clear();
        }
    };
    let mut k = 0;
    // Vrai tant qu'on est au début d'un bloc (après une ligne vide ou un élément d'une ligne) :
    // seul endroit où un nom de personnage est reconnu.
    let mut at_start = true;
    while k < lines.len() {
        let line = lines[k].trim();
        let is_last = k + 1 == lines.len();
        if line.len() >= 3 && line.chars().all(|c| c == '=') {
            k += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            flush(&mut action, out);
            out.push(Element::new(Kind::Section(collapse_spaces(&strip_emphasis(rest.trim_start_matches('#').trim())))));
            k += 1;
            at_start = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix('=') {
            flush(&mut action, out);
            out.push(Element::new(Kind::Synopsis(collapse_spaces(&strip_emphasis(rest.trim())))));
            k += 1;
            at_start = true;
            continue;
        }
        if at_start {
            if let Some(h) = en_tete_of(line) {
                flush(&mut action, out);
                out.push(h.element(Vec::new()));
                k += 1;
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix('>') {
            flush(&mut action, out);
            let rest = rest.trim();
            match rest.strip_suffix('<') {
                Some(inner) => out.push(Element::new(Kind::Action(collapse_spaces(&strip_emphasis(inner.trim()))))),
                None => out.push(Element::new(Kind::Transition(collapse_spaces(&strip_emphasis(rest))))),
            }
            k += 1;
            at_start = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix('!') {
            action.push(rest.trim().to_string());
            k += 1;
            at_start = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix('~') {
            action.push(format!("♪ {}", rest.trim()));
            k += 1;
            at_start = false;
            continue;
        }
        if at_start && is_last && is_transition(line) {
            flush(&mut action, out);
            out.push(Element::new(Kind::Transition(collapse_spaces(line))));
            k += 1;
            continue;
        }
        if at_start && !is_last {
            if let Some(cue) = parse_cue(line, false) {
                flush(&mut action, out);
                let sp = speech_from(cue, &lines[k + 1..]);
                out.push(Element::new(Kind::Dialogue(sp)));
                return;
            }
        }
        action.push(collapse_spaces(line));
        k += 1;
        at_start = false;
    }
    flush(&mut action, out);
}

// ------------------------------------------------------------------------------------ texte brut

/// Réglages de la lecture du texte brut.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextOptions {
    /// Chaque ligne est un paragraphe (texte d'un document Word : un paragraphe = une ligne).
    pub line_is_paragraph: bool,
}

enum State {
    Idle,
    Action(Vec<String>),
    Speech(Cue, Vec<String>),
}

fn leave(state: &mut State, out: &mut Vec<Element>) {
    match std::mem::replace(state, State::Idle) {
        State::Idle => {}
        State::Action(lines) => out.push(Element::new(Kind::Action(collapse_lines(&lines)))),
        State::Speech(cue, lines) => out.push(Element::new(Kind::Dialogue(speech_from(cue, &lines)))),
    }
}

fn collapse_lines(lines: &[String]) -> String {
    lines.iter().map(|l| collapse_spaces(l)).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Largeur d'indentation d'une ligne (une tabulation vaut quatre espaces).
fn indent_of(line: &str) -> usize {
    let mut n = 0;
    for c in line.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    n
}

/// Ce que le texte copié d'un PDF ajoute à chaque page : numéros, « (SUITE) »…
fn is_page_furniture(t: &str) -> bool {
    let core = t.trim_matches(|c: char| !c.is_alphanumeric());
    if core.is_empty() {
        return true;
    }
    if core.len() <= 4 && core.chars().all(|c| c.is_ascii_digit()) && (t.ends_with('.') || t.starts_with('-') || t.starts_with('(') || t.len() == core.len()) {
        return true;
    }
    let k: String = core.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase();
    matches!(k.as_str(), "CONTINUED" | "CONTINUE" | "SUITE" | "MORE" | "CONTD" | "SUITEPAGESUIVANTE")
}

/// Une ligne qui commence par une étiquette de consigne (`SFX :`, `Musique :`, `Note :`…).
fn is_directive(t: &str) -> bool {
    t.split_once([':', '：']).is_some_and(|(head, _)| NOT_NAMES.contains(&head.trim().to_uppercase().as_str()))
}

/// Cherche « NOM : réplique » (nom en majuscules) : le personnage et la réplique.
fn inline_speech(t: &str) -> Option<(Cue, String)> {
    let i = t.find([':', '：'])?;
    let (head, tail) = (&t[..i], t[i..].trim_start_matches([':', '：']).trim());
    if tail.is_empty() {
        return None;
    }
    let cue = parse_cue(head, false)?;
    if cue.name.split_whitespace().count() > 4 {
        return None;
    }
    Some((cue, tail.to_string()))
}

/// Lit un scénario en texte brut (déduit des intitulés, des noms en majuscules et de l'indentation).
pub fn parse_text(text: &str, opts: TextOptions, progress: &dyn Fn(f32)) -> Script {
    let mut script = Script::empty(Format::Texte);
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n").replace('\u{c}', "\n");
    let raw: Vec<&str> = text.split('\n').collect();
    let total = raw.len().max(1);
    let mut out: Vec<Element> = Vec::new();
    let mut state = State::Idle;
    for (i, line) in raw.iter().enumerate() {
        if i % 500 == 499 {
            progress(0.1 + 0.85 * i as f32 / total as f32);
        }
        let t = line.trim();
        if t.is_empty() {
            leave(&mut state, &mut out);
            continue;
        }
        if is_page_furniture(t) {
            continue;
        }
        let indent = indent_of(line);
        if let Some(h) = en_tete_of(t) {
            leave(&mut state, &mut out);
            out.push(h.element(Vec::new()));
            continue;
        }
        if !matches!(state, State::Speech(..)) && is_transition(t) {
            leave(&mut state, &mut out);
            out.push(Element::new(Kind::Transition(collapse_spaces(t))));
            continue;
        }
        // Une consigne (« SFX : porte qui grince ») est de l'action, jamais une réplique.
        let directive = is_directive(t);
        if directive && matches!(state, State::Speech(..)) {
            leave(&mut state, &mut out);
        }
        // « NOM : réplique » sur une seule ligne.
        if let Some((cue, first)) = inline_speech(t).filter(|_| !directive) {
            leave(&mut state, &mut out);
            state = State::Speech(cue, vec![first]);
            continue;
        }
        // Le nom d'un personnage : majuscules, suivi tout de suite d'une ligne de texte.
        let next = raw.get(i + 1).map(|l| l.trim()).filter(|l| !l.is_empty());
        let next_is_text = next.is_some_and(|n| heading_of(n).is_none() && !is_page_furniture(n) && !is_transition(n));
        if next_is_text && !directive {
            if let Some(cue) = parse_cue(t, false) {
                // Dans une réplique, un nom en majuscules est un nouveau personnage s'il n'est pas
                // plus à gauche que la réplique (un cri en majuscules sans ponctuation est rare).
                let new_cue = match &state {
                    State::Speech(..) => true,
                    _ => cue.name.split_whitespace().count() <= 4 || indent >= 8,
                };
                if new_cue {
                    leave(&mut state, &mut out);
                    state = State::Speech(cue, Vec::new());
                    continue;
                }
            }
        }
        if opts.line_is_paragraph && matches!(state, State::Action(_)) {
            leave(&mut state, &mut out);
        }
        if matches!(state, State::Idle) {
            state = State::Action(vec![t.to_string()]);
        } else if let State::Speech(_, lines) | State::Action(lines) = &mut state {
            lines.push(t.to_string());
        }
    }
    leave(&mut state, &mut out);
    script.elements = out;
    script
}
