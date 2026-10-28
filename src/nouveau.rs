//! Fenêtre « Nouveau projet » (Anthony, 2026-10-18) : la partie sans interface. Des préréglages
//! d'animation (vidéo, réseaux, papier), les unités (px, mm, cm, in) converties avec la
//! résolution, les valeurs retenues d'un projet à l'autre et les préréglages de l'utilisateur.
//!
//! Le projet ne garde que des pixels : la résolution (dpi) sert à lire le papier et les unités, et
//! reste dans les réglages de l'application (`settings.rs`), pas dans le fichier du projet.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::document::{BackgroundKind, Document};

/// Taille minimale et maximale du cadre, en pixels (la feuille de dessin en fait le double).
pub const MIN_PX: usize = 16;
pub const MAX_PX: usize = 4096;
/// Résolution minimale et maximale, en points par pouce.
pub const MIN_DPI: u32 = 36;
pub const MAX_DPI: u32 = 1200;
/// Millimètres dans un pouce.
const MM_PER_INCH: f64 = 25.4;

/// Unité des champs largeur et hauteur ; le projet, lui, garde des pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[default]
    #[serde(rename = "px")]
    Px,
    #[serde(rename = "mm")]
    Mm,
    #[serde(rename = "cm")]
    Cm,
    #[serde(rename = "in")]
    In,
}

impl Unit {
    pub const ALL: [Unit; 4] = [Unit::Px, Unit::Mm, Unit::Cm, Unit::In];

    pub fn label(self) -> &'static str {
        match self {
            Unit::Px => "px",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::In => "in",
        }
    }

    /// Décimales montrées (le pixel est entier).
    pub fn decimals(self) -> usize {
        match self {
            Unit::Px => 0,
            Unit::Mm => 1,
            Unit::Cm => 2,
            Unit::In => 3,
        }
    }

    /// Pas d'un glisser sur le champ.
    pub fn speed(self) -> f64 {
        match self {
            Unit::Px => 1.0,
            Unit::Mm => 0.5,
            Unit::Cm => 0.05,
            Unit::In => 0.01,
        }
    }

    /// Pixels par unité à la résolution `dpi`.
    fn px_per_unit(self, dpi: u32) -> f64 {
        let dpi = f64::from(dpi.max(1));
        match self {
            Unit::Px => 1.0,
            Unit::Mm => dpi / MM_PER_INCH,
            Unit::Cm => dpi / MM_PER_INCH * 10.0,
            Unit::In => dpi,
        }
    }

    /// Une longueur en pixels, dans cette unité.
    pub fn from_px(self, px: usize, dpi: u32) -> f64 {
        px as f64 / self.px_per_unit(dpi)
    }

    /// Une longueur de cette unité, en pixels (arrondie, gardée entre `MIN_PX` et `MAX_PX`).
    pub fn to_px(self, value: f64, dpi: u32) -> usize {
        let px = (value * self.px_per_unit(dpi)).round();
        if px.is_nan() {
            return MIN_PX;
        }
        (px.clamp(MIN_PX as f64, MAX_PX as f64)) as usize
    }
}

/// Famille d'un préréglage, pour grouper la liste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Video,
    Social,
    Paper,
    User,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Video => "Vidéo",
            Category::Social => "Réseaux",
            Category::Paper => "Papier",
            Category::User => "Mes préréglages",
        }
    }
}

/// Taille d'un préréglage : des pixels, ou des millimètres (le papier, lu à la résolution).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Px(usize, usize),
    /// Paysage (le grand côté d'abord).
    Mm(f64, f64),
}

/// Un préréglage d'origine : un nom, une taille et une cadence.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub category: Category,
    pub name: &'static str,
    pub size: Size,
    pub fps: u32,
}

/// Les préréglages d'origine, dans l'ordre de la liste.
///
/// « SD PAL 720×576 » n'y est pas : ses pixels ne sont pas carrés (4:3 sur 5:4), et SHOT ne
/// travaille qu'en pixels carrés ; 768×576 en est la version carrée.
pub const PRESETS: [Preset; 13] = [
    Preset { category: Category::Video, name: "HD 1080", size: Size::Px(1920, 1080), fps: 24 },
    Preset { category: Category::Video, name: "HD 720", size: Size::Px(1280, 720), fps: 24 },
    Preset { category: Category::Video, name: "UHD 4K", size: Size::Px(3840, 2160), fps: 24 },
    Preset { category: Category::Video, name: "2K DCI", size: Size::Px(2048, 1080), fps: 24 },
    Preset { category: Category::Video, name: "Scope 2,39", size: Size::Px(2048, 858), fps: 24 },
    Preset { category: Category::Video, name: "SD PAL 4:3", size: Size::Px(768, 576), fps: 25 },
    Preset { category: Category::Social, name: "Vertical 9:16", size: Size::Px(1080, 1920), fps: 24 },
    Preset { category: Category::Social, name: "Carré 1:1", size: Size::Px(1080, 1080), fps: 24 },
    Preset { category: Category::Social, name: "Portrait 4:5", size: Size::Px(1080, 1350), fps: 24 },
    Preset { category: Category::Paper, name: "A4", size: Size::Mm(297.0, 210.0), fps: 24 },
    Preset { category: Category::Paper, name: "A3", size: Size::Mm(420.0, 297.0), fps: 24 },
    Preset { category: Category::Paper, name: "A5", size: Size::Mm(210.0, 148.0), fps: 24 },
    Preset { category: Category::Paper, name: "US Letter", size: Size::Mm(279.4, 215.9), fps: 24 },
];

/// Ce qu'un préréglage donne une fois posé : taille en pixels et résolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    pub width: usize,
    pub height: usize,
    pub dpi: u32,
    /// La résolution a dû baisser pour que le papier tienne dans `MAX_PX`.
    pub capped: bool,
}

impl Preset {
    /// « 1920×1080 » ou « 297×210 mm ».
    pub fn detail(&self) -> String {
        match self.size {
            Size::Px(w, h) => format!("{w}×{h}"),
            Size::Mm(w, h) => format!("{}×{} mm", trim_num(w), trim_num(h)),
        }
    }

    pub fn is_paper(&self) -> bool {
        matches!(self.size, Size::Mm(..))
    }

    /// La taille en pixels (paysage). Le papier se lit à la résolution `dpi` ; s'il dépasse
    /// `MAX_PX`, la résolution baisse juste assez pour qu'il tienne (A3 à 300 dpi ferait 4961 px).
    pub fn apply(&self, dpi: u32) -> Applied {
        match self.size {
            Size::Px(width, height) => Applied { width, height, dpi, capped: false },
            Size::Mm(w, h) => {
                let mut dpi = dpi.clamp(MIN_DPI, MAX_DPI);
                let long_px = |dpi: u32| (w * f64::from(dpi) / MM_PER_INCH).round() as usize;
                let mut capped = false;
                if long_px(dpi) > MAX_PX {
                    capped = true;
                    // La plus haute résolution entière dont le grand côté tient.
                    dpi = (((MAX_PX as f64) * MM_PER_INCH / w).floor() as u32).clamp(MIN_DPI, MAX_DPI);
                    while dpi > MIN_DPI && long_px(dpi) > MAX_PX {
                        dpi -= 1;
                    }
                }
                Applied { width: Unit::Mm.to_px(w, dpi), height: Unit::Mm.to_px(h, dpi), dpi, capped }
            }
        }
    }
}

/// Un nombre sans « .0 » inutile : 297 → « 297 », 279,4 → « 279,4 ».
fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}").replace('.', ",")
    }
}

/// Préréglage de l'utilisateur, gardé dans les réglages de l'application (pas dans les projets).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserPreset {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "largeur")]
    pub width: usize,
    #[serde(rename = "hauteur")]
    pub height: usize,
    #[serde(rename = "cadence")]
    pub fps: u32,
    #[serde(rename = "resolution")]
    pub dpi: u32,
}

/// Ajoute un préréglage à la liste ; le même nom (casse ignorée) est remplacé. Un nom vide est
/// refusé.
pub fn save_user_preset(list: &mut Vec<UserPreset>, preset: UserPreset) -> bool {
    let name = preset.name.trim().to_string();
    if name.is_empty() {
        return false;
    }
    let preset = UserPreset { name, ..preset };
    match list.iter_mut().find(|p| p.name.to_lowercase() == preset.name.to_lowercase()) {
        Some(old) => *old = preset,
        None => list.push(preset),
    }
    true
}

/// Retire le préréglage de l'utilisateur qui porte ce nom ; ceux d'origine ne sont pas dans la
/// liste, donc on ne peut pas les effacer.
pub fn delete_user_preset(list: &mut Vec<UserPreset>, name: &str) -> bool {
    let before = list.len();
    list.retain(|p| p.name != name);
    list.len() != before
}

/// Ce que montrent les valeurs actuelles : un préréglage reconnu, ou « Personnalisé ».
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Builtin(usize),
    User(usize),
    Custom,
}

impl Choice {
    pub const CUSTOM_LABEL: &'static str = "Personnalisé";
}

/// Le préréglage que décrivent ces valeurs (taille et cadence). Le papier se reconnaît dans les
/// deux orientations, à la résolution `dpi` ; ceux de l'utilisateur d'abord.
pub fn detect(v: &Values, user: &[UserPreset]) -> Choice {
    // (La résolution n'entre pas dans la comparaison : elle ne change pas la taille en pixels.)
    if let Some(i) = user.iter().position(|p| p.width == v.width && p.height == v.height && p.fps == v.fps) {
        return Choice::User(i);
    }
    for (i, p) in PRESETS.iter().enumerate() {
        if p.fps != v.fps {
            continue;
        }
        let a = p.apply(v.dpi);
        let same = match p.size {
            Size::Px(..) => (a.width, a.height) == (v.width, v.height),
            Size::Mm(..) => (a.width, a.height) == (v.width, v.height) || (a.height, a.width) == (v.width, v.height),
        };
        if same {
            return Choice::Builtin(i);
        }
    }
    Choice::Custom
}

/// Pose les valeurs d'un préréglage d'origine. Un papier garde l'orientation en cours.
pub fn apply_builtin(v: &mut Values, index: usize) -> Applied {
    let p = &PRESETS[index];
    let portrait = v.height > v.width && p.is_paper();
    let a = p.apply(v.dpi);
    let (w, h) = if portrait { (a.height, a.width) } else { (a.width, a.height) };
    v.width = w;
    v.height = h;
    v.fps = p.fps;
    v.dpi = a.dpi;
    a
}

/// Pose les valeurs d'un préréglage de l'utilisateur.
pub fn apply_user(v: &mut Values, p: &UserPreset) {
    v.width = p.width.clamp(MIN_PX, MAX_PX);
    v.height = p.height.clamp(MIN_PX, MAX_PX);
    v.fps = p.fps.clamp(1, 120);
    v.dpi = p.dpi.clamp(MIN_DPI, MAX_DPI);
}

/// Formes reconnues pour « Proportions » (paysage), avec leur nom.
const KNOWN_RATIOS: [(f64, &str, &str); 11] = [
    (1.0, "1:1", "1:1"),
    (5.0 / 4.0, "5:4", "4:5"),
    (4.0 / 3.0, "4:3", "3:4"),
    (3.0 / 2.0, "3:2", "2:3"),
    (16.0 / 10.0, "16:10", "10:16"),
    (16.0 / 9.0, "16:9", "9:16"),
    (1.85, "1,85:1", "1:1,85"),
    (2.0, "2:1", "1:2"),
    (2.39, "2,39:1", "1:2,39"),
    (21.0 / 9.0, "21:9", "9:21"),
    (std::f64::consts::SQRT_2, "1,41:1 (papier A)", "1:1,41 (papier A)"),
];

/// Proportions d'un cadre : « 16:9 », « 9:16 », « 2,39:1 » quand elles sont proches (moins de 0,5 %
/// d'écart), sinon la fraction simple ou « 1,33:1 ».
pub fn ratio_name(w: usize, h: usize) -> String {
    let (w, h) = (w.max(1), h.max(1));
    let r = w as f64 / h as f64;
    let long = r.max(1.0 / r);
    for (known, wide, tall) in KNOWN_RATIOS {
        if (long / known - 1.0).abs() < 0.005 {
            return if r >= 1.0 { wide } else { tall }.to_string();
        }
    }
    let text = crate::projetui::ratio_label(w, h);
    if r < 1.0 && text.ends_with(":1") {
        // « 0,42:1 » se lit mal : un cadre en hauteur s'écrit « 1:2,4 ».
        return format!("1:{:.2}", 1.0 / r).replace('.', ",");
    }
    text
}

/// Valeurs de la fenêtre : le format du projet et ses couleurs. Retenues d'un projet à l'autre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Values {
    #[serde(rename = "largeur")]
    pub width: usize,
    #[serde(rename = "hauteur")]
    pub height: usize,
    #[serde(rename = "cadence")]
    pub fps: u32,
    /// Résolution du papier et des unités ; seulement dans les réglages, jamais dans un projet.
    #[serde(rename = "resolution")]
    pub dpi: u32,
    #[serde(rename = "unite")]
    pub unit: Unit,
    /// Papier opaque (sinon : arrière-plan transparent).
    #[serde(rename = "papier_opaque")]
    pub paper_on: bool,
    #[serde(rename = "papier")]
    pub paper: [u8; 3],
    /// Couleur des fonds sous le trait.
    #[serde(rename = "fond_sous_le_trait")]
    pub fond: [u8; 3],
    /// Durée du premier plan, en secondes.
    #[serde(rename = "duree_premier_plan")]
    pub first_seconds: f32,
}

impl Default for Values {
    fn default() -> Self {
        Values { width: 1920, height: 1080, fps: 24, dpi: 300, unit: Unit::Px, paper_on: true, paper: [255, 255, 255], fond: [255, 255, 255], first_seconds: 2.0 }
    }
}

impl Values {
    /// Largeur et hauteur échangées (portrait ↔ paysage).
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.width, &mut self.height);
    }

    /// Nombre d'images du premier plan.
    pub fn first_frames(&self) -> usize {
        ((f64::from(self.first_seconds) * f64::from(self.fps)).round() as usize).max(1)
    }

    /// Les valeurs rentrées dans leurs limites.
    pub fn clamp(&mut self) {
        self.width = self.width.clamp(MIN_PX, MAX_PX);
        self.height = self.height.clamp(MIN_PX, MAX_PX);
        self.fps = self.fps.clamp(1, 120);
        self.dpi = self.dpi.clamp(MIN_DPI, MAX_DPI);
        self.first_seconds = if self.first_seconds.is_finite() { self.first_seconds.clamp(0.1, 600.0) } else { 2.0 };
    }
}

/// Réglages de la fenêtre dans `reglages.toml` : les dernières valeurs, le dernier dossier et les
/// préréglages de l'utilisateur.
/// La page sur laquelle un projet neuf s'ouvre (Préférences > Général > « Un nouveau projet s'ouvre sur »). Un réglage de ce
/// poste, pas du projet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OuvreSur {
    /// La planche des Vignettes (d'origine).
    #[default]
    #[serde(rename = "vignettes")]
    Vignettes,
    /// L'espace Storyboard, avec la timeline.
    #[serde(rename = "storyboard")]
    Storyboard,
}

impl OuvreSur {
    pub fn est_defaut(&self) -> bool {
        *self == OuvreSur::Vignettes
    }

    pub fn espace(self) -> crate::workspace::Espace {
        match self {
            OuvreSur::Vignettes => crate::workspace::Espace::Vignettes,
            OuvreSur::Storyboard => crate::workspace::Espace::Storyboard,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewProjectSettings {
    /// La page où le projet créé s'ouvre.
    #[serde(rename = "ouvre_sur", skip_serializing_if = "OuvreSur::est_defaut")]
    pub ouvre_sur: OuvreSur,
    #[serde(rename = "dernieres_valeurs")]
    pub values: Values,
    #[serde(rename = "dossier", skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    #[serde(rename = "prereglages", skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<UserPreset>,
}

/// Nom de dossier et de fichier sûr pour Windows : sans `<>:"/\|?*`, sans point ni espace à la fin.
pub fn file_stem(name: &str) -> String {
    let s: String = name.trim().chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    s.trim_end_matches(['.', ' ']).to_string()
}

/// Dossier proposé la première fois : Documents, sinon le dossier de l'utilisateur.
pub fn default_folder() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home {
        Some(h) if h.join("Documents").is_dir() => h.join("Documents"),
        Some(h) => h,
        None => PathBuf::from("."),
    }
}

/// Le projet neuf de ces valeurs : cadre, cadence, papier, couleur des fonds sous le trait, durée
/// du premier plan et titre.
pub fn build_document(v: &Values, title: &str) -> Document {
    let mut v = v.clone();
    v.clamp();
    let mut doc = Document::with_frame(v.width, v.height, v.fps).created_now();
    doc.paper = v.paper;
    doc.background = if v.paper_on { BackgroundKind::Color } else { BackgroundKind::None };
    let default_fond = crate::fond::FondReglages::default().couleur;
    doc.properties.fond = (v.fond != default_fond).then_some(v.fond);
    doc.properties.title = title.trim().to_string();
    let frames = v.first_frames();
    if let Some(panel) = doc.panels.first_mut() {
        panel.length = frames;
    }
    doc
}
