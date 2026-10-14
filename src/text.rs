//! Texte (5d) : polices installées sur le poste, mise en page simple (lignes alignées à gauche,
//! sans crénage), contours des lettres en pixels. Lecture des polices par ttf-parser.
//!
//! Une police absente (projet ouvert sur un autre poste) est remplacée par la police intégrée à
//! Pelure (`FALLBACK`, celle de l'interface) : le texte reste lisible et modifiable.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use eframe::egui::{self, pos2, vec2, Rect};
use ttf_parser::{name_id, Face, GlyphId, OutlineBuilder, Tag};

/// Police intégrée.
pub const FALLBACK: &str = "Ubuntu Light";

/// Lettres mises en page.
#[derive(Debug)]
pub struct TextLayout {
    /// Contours de chaque lettre, en pixels ; origine en haut à gauche de la première ligne, y
    /// vers le bas.
    pub glyphs: Vec<Vec<Vec<[f32; 2]>>>,
    /// Boîte des lignes : largeur de la plus longue, hauteur de toutes.
    pub bounds: Rect,
    /// Police réellement utilisée.
    pub font: String,
}

#[derive(Clone)]
struct FontFile {
    path: PathBuf,
    index: u32,
}

#[derive(Default)]
struct Registry {
    scanned: bool,
    files: HashMap<String, FontFile>,
    names: Vec<String>,
    data: HashMap<PathBuf, Arc<Vec<u8>>>,
    layouts: HashMap<(String, String, u32), Arc<TextLayout>>,
}

fn registry() -> &'static Mutex<Registry> {
    static R: OnceLock<Mutex<Registry>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn fallback_data() -> Arc<Vec<u8>> {
    static D: OnceLock<Arc<Vec<u8>>> = OnceLock::new();
    D.get_or_init(|| {
        let defs = egui::FontDefinitions::default();
        Arc::new(defs.font_data.get("Ubuntu-Light").map(|f| f.font.to_vec()).unwrap_or_default())
    })
    .clone()
}

/// Dossiers des polices de Windows : celles du système, puis celles installées pour l'utilisateur.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(w) = std::env::var_os("WINDIR") {
        dirs.push(PathBuf::from(w).join("Fonts"));
    }
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

/// Nom complet (« Arial Bold ») de chaque police d'un fichier, sans le lire en entier : l'en-tête,
/// puis la seule table des noms.
fn scan_file(path: &Path) -> Vec<(String, u32)> {
    let Ok(mut file) = std::fs::File::open(path) else { return Vec::new() };
    let mut head = Vec::new();
    if file.by_ref().take(64 * 1024).read_to_end(&mut head).is_err() {
        return Vec::new();
    }
    let count = ttf_parser::fonts_in_collection(&head).unwrap_or(1).min(64);
    let mut out = Vec::new();
    for index in 0..count {
        let Ok(raw) = ttf_parser::RawFace::parse(&head, index) else { continue };
        let Some(rec) = raw.table_records.into_iter().find(|r| r.tag == Tag::from_bytes(b"name")) else { continue };
        let mut buf = vec![0u8; rec.length as usize];
        if file.seek(SeekFrom::Start(rec.offset as u64)).is_err() || file.read_exact(&mut buf).is_err() {
            continue;
        }
        if let Some(name) = ttf_parser::name::Table::parse(&buf).and_then(|t| full_name(&t)) {
            out.push((name, index));
        }
    }
    out
}

/// Nom complet, en anglais de préférence.
fn full_name(table: &ttf_parser::name::Table) -> Option<String> {
    let names = table.names;
    let mut best: Option<(u8, String)> = None;
    for i in 0..names.len() {
        let Some(n) = names.get(i) else { continue };
        if n.name_id != name_id::FULL_NAME || !n.is_unicode() {
            continue;
        }
        let Some(s) = n.to_string().filter(|s| !s.trim().is_empty()) else { continue };
        let rank = if n.language_id == 0x0409 { 0 } else { 1 };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, s));
        }
    }
    best.map(|(_, s)| s)
}

fn scan(reg: &mut Registry) {
    if reg.scanned {
        return;
    }
    reg.scanned = true;
    for dir in font_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                continue;
            }
            for (name, index) in scan_file(&path) {
                reg.files.entry(name).or_insert(FontFile { path: path.clone(), index });
            }
        }
    }
    let mut names: Vec<String> = reg.files.keys().cloned().collect();
    names.push(FALLBACK.into());
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    reg.names = names;
}

/// Noms des polices disponibles (la première fois : lecture des dossiers de polices).
pub fn font_names() -> Vec<String> {
    let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    scan(&mut reg);
    reg.names.clone()
}

/// Lit les polices sur un autre fil, pour que l'outil Texte n'attende pas la première fois.
pub fn scan_in_background() {
    std::thread::spawn(|| {
        font_names();
    });
}

/// Données de la police `name` (ou de la police intégrée) : nom utilisé, octets, numéro dans le fichier.
fn font_data(reg: &mut Registry, name: &str) -> (String, Arc<Vec<u8>>, u32) {
    if name != FALLBACK {
        scan(reg);
        if let Some(f) = reg.files.get(name).cloned() {
            let data = match reg.data.get(&f.path) {
                Some(d) => Some(d.clone()),
                None => std::fs::read(&f.path).ok().map(|b| {
                    let d = Arc::new(b);
                    reg.data.insert(f.path.clone(), d.clone());
                    d
                }),
            };
            if let Some(d) = data {
                return (name.to_string(), d, f.index);
            }
        }
    }
    (FALLBACK.to_string(), fallback_data(), 0)
}

/// Mise en page de `text` dans la police `font`, `size` pixels par em (gardée en mémoire).
pub fn layout(text: &str, font: &str, size: f32) -> Arc<TextLayout> {
    let size = if size.is_finite() { size.clamp(1.0, 2000.0) } else { 48.0 };
    let key = (text.to_string(), font.to_string(), size.to_bits());
    let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(l) = reg.layouts.get(&key) {
        return l.clone();
    }
    let (used, data, index) = font_data(&mut reg, font);
    let laid = match Face::parse(&data, index) {
        Ok(face) => compute(&face, text, size, used),
        Err(_) => {
            let fb = fallback_data();
            match Face::parse(&fb, 0) {
                Ok(face) => compute(&face, text, size, FALLBACK.into()),
                Err(_) => TextLayout { glyphs: Vec::new(), bounds: Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, size)), font: FALLBACK.into() },
            }
        }
    };
    let laid = Arc::new(laid);
    if reg.layouts.len() > 256 {
        reg.layouts.clear();
    }
    reg.layouts.insert(key, laid.clone());
    laid
}

fn compute(face: &Face, text: &str, size: f32, font: String) -> TextLayout {
    let k = size / face.units_per_em().max(1) as f32;
    let ascent = face.ascender() as f32 * k;
    let descent = face.descender() as f32 * k;
    let gap = face.line_gap() as f32 * k;
    let line_height = ascent - descent + gap;
    let space = face.glyph_index(' ').and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0) as f32 * k;
    let mut glyphs = Vec::new();
    let mut width: f32 = 0.0;
    let lines: Vec<&str> = text.split('\n').collect();
    for (li, line) in lines.iter().enumerate() {
        let baseline = ascent + li as f32 * line_height;
        let mut x = 0.0;
        for ch in line.chars() {
            if ch == '\t' {
                x += space * 4.0;
                continue;
            }
            if ch.is_control() {
                continue;
            }
            let gid = face.glyph_index(ch).or_else(|| face.glyph_index('?')).unwrap_or(GlyphId(0));
            let mut b = Flatten { k, ox: x, oy: baseline, contours: Vec::new(), cur: Vec::new(), last: (0.0, 0.0) };
            face.outline_glyph(gid, &mut b);
            b.close();
            if !b.contours.is_empty() {
                glyphs.push(b.contours);
            }
            x += face.glyph_hor_advance(gid).unwrap_or(0) as f32 * k;
        }
        width = width.max(x);
    }
    let height = lines.len() as f32 * line_height - gap;
    TextLayout { glyphs, bounds: Rect::from_min_size(pos2(0.0, 0.0), vec2(width, height.max(1.0))), font }
}

/// Contours d'une lettre, courbes découpées en segments, en pixels.
struct Flatten {
    k: f32,
    ox: f32,
    oy: f32,
    contours: Vec<Vec<[f32; 2]>>,
    cur: Vec<[f32; 2]>,
    last: (f32, f32),
}

impl Flatten {
    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        [self.ox + x * self.k, self.oy - y * self.k]
    }

    /// Nombre de segments pour une courbe d'environ `len` unités de police.
    fn steps(&self, len: f32) -> usize {
        ((len * self.k / 3.0).ceil() as usize).clamp(2, 24)
    }
}

impl OutlineBuilder for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.cur.push(self.point(x, y));
        self.last = (x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.cur.push(self.point(x, y));
        self.last = (x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        let n = self.steps((x1 - x0).hypot(y1 - y0) + (x - x1).hypot(y - y1));
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let px = u * u * x0 + 2.0 * u * t * x1 + t * t * x;
            let py = u * u * y0 + 2.0 * u * t * y1 + t * t * y;
            self.cur.push(self.point(px, py));
        }
        self.last = (x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        let n = self.steps((x1 - x0).hypot(y1 - y0) + (x2 - x1).hypot(y2 - y1) + (x - x2).hypot(y - y2));
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.cur.push(self.point(a * x0 + b * x1 + c * x2 + d * x, a * y0 + b * y1 + c * y2 + d * y));
        }
        self.last = (x, y);
    }

    fn close(&mut self) {
        let c = std::mem::take(&mut self.cur);
        if c.len() >= 3 {
            self.contours.push(c);
        }
    }
}
