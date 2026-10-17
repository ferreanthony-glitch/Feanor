//! Pinceaux texturés (3d-2) : une pointe en image (niveaux de gris) et le grain du papier, comme
//! un crayon ou une craie sur un papier à grain. Rendu visé : celui de TVPaint (choix d'Anthony),
//! dont on reprend l'idée d'un grain fixe sur la feuille — sans en connaître le calcul exact.
//!
//! Le grain ne suit pas le trait : il est attaché au dessin, si bien que repasser au même endroit
//! retrouve les mêmes creux. Appuyer plus fort fait entrer la couleur dans les creux.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::brush::BrushSettings;

/// Image en niveaux de gris, valeurs de 0 à 1. Pointe : quantité de couleur déposée (1 = pleine).
/// Papier : relief (1 = sommet, qui prend la couleur en premier).
#[derive(Clone, Debug, PartialEq)]
pub struct GrayImage {
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
}

impl GrayImage {
    pub fn new(width: usize, height: usize, values: Vec<f32>) -> GrayImage {
        assert_eq!(values.len(), width * height, "taille de l'image");
        GrayImage { width, height, values }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.width + x]
    }

    /// Valeur au point (`x`, `y`) en texels, interpolée ; hors de l'image : 0.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let get = |x: i64, y: i64| {
            if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
                0.0
            } else {
                self.values[y as usize * self.width + x as usize]
            }
        };
        let top = get(ix, iy) + (get(ix + 1, iy) - get(ix, iy)) * tx;
        let bottom = get(ix, iy + 1) + (get(ix + 1, iy + 1) - get(ix, iy + 1)) * tx;
        top + (bottom - top) * ty
    }

    /// Valeur répétée en motif (papier).
    pub fn tiled(&self, x: usize, y: usize) -> f32 {
        self.values[(y % self.height) * self.width + x % self.width]
    }

    /// Valeur interpolée au point (`x`, `y`) de l'image répétée sans fin (papier mis à
    /// l'échelle ou tourné).
    pub fn sample_wrapped(&self, x: f32, y: f32) -> f32 {
        let (w, h) = (self.width as i64, self.height as i64);
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let get = |x: i64, y: i64| self.values[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize];
        let top = get(ix, iy) + (get(ix + 1, iy) - get(ix, iy)) * tx;
        let bottom = get(ix, iy + 1) + (get(ix + 1, iy + 1) - get(ix, iy + 1)) * tx;
        top + (bottom - top) * ty
    }

    /// Une pointe importée d'une image, réduite à `max` pixels de côté au plus : 1 où elle dépose.
    /// Une image transparente donne sa transparence ; une image opaque, son noir (le noir dépose).
    pub fn tip_from_image(img: &image::DynamicImage, max: u32) -> GrayImage {
        let img = if img.width() > max || img.height() > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img.clone() };
        let rgba = img.to_rgba8();
        let transparent = rgba.pixels().any(|p| p.0[3] < 250);
        let values = rgba
            .pixels()
            .map(|p| {
                if transparent {
                    p.0[3] as f32 / 255.0
                } else {
                    1.0 - (0.2126 * p.0[0] as f32 + 0.7152 * p.0[1] as f32 + 0.0722 * p.0[2] as f32) / 255.0
                }
            })
            .collect();
        GrayImage::new(rgba.width() as usize, rgba.height() as usize, values)
    }

    /// Le gris d'une image (luminance, 0 à 1), réduite à `max` pixels de côté au plus : un
    /// papier importé.
    pub fn from_image(img: &image::DynamicImage, max: u32) -> GrayImage {
        let img = if img.width() > max || img.height() > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img.clone() };
        let g = img.to_luma8();
        GrayImage::new(g.width() as usize, g.height() as usize, g.pixels().map(|p| p.0[0] as f32 / 255.0).collect())
    }

    /// Image moitié moins grande (moyenne de 2 × 2 texels).
    pub fn half(&self) -> GrayImage {
        let (w, h) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut values = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let (x0, y0) = ((2 * x).min(self.width - 1), (2 * y).min(self.height - 1));
                let (x1, y1) = ((2 * x + 1).min(self.width - 1), (2 * y + 1).min(self.height - 1));
                values.push((self.get(x0, y0) + self.get(x1, y0) + self.get(x0, y1) + self.get(x1, y1)) / 4.0);
            }
        }
        GrayImage::new(w, h, values)
    }

    /// Valeurs étirées sur toute la plage 0..1 (un papier peu contrasté marque quand même).
    pub fn normalized(mut self) -> GrayImage {
        let lo = self.values.iter().copied().fold(f32::MAX, f32::min);
        let hi = self.values.iter().copied().fold(f32::MIN, f32::max);
        if hi - lo > 1e-6 {
            for v in &mut self.values {
                *v = (*v - lo) / (hi - lo);
            }
        }
        self
    }
}

/// Pointe en image, avec ses réductions successives : une pointe de quelques pixels prend une
/// version réduite (moyennée) plutôt que quelques texels isolés de la grande image.
#[derive(Debug)]
pub struct Tip {
    levels: Vec<GrayImage>,
}

impl Tip {
    pub fn new(image: GrayImage) -> Tip {
        let mut levels = vec![image];
        while levels.last().is_some_and(|l| l.width.min(l.height) > 4) {
            let next = levels.last().expect("au moins un niveau").half();
            levels.push(next);
        }
        Tip { levels }
    }

    pub fn full(&self) -> &GrayImage {
        &self.levels[0]
    }

    /// La plus petite réduction encore au moins aussi large que la pointe à l'écran.
    pub fn level(&self, diameter: f32) -> &GrayImage {
        self.levels
            .iter()
            .rev()
            .find(|l| l.width as f32 >= diameter)
            .unwrap_or(&self.levels[0])
    }
}

/// Part de la couleur qui reste sur un point du papier de relief `height` : les sommets d'abord,
/// les creux quand on appuie (`pressure`) ; `grain` = 0 : papier lisse, 1 : grain marqué.
pub fn grain_factor(height: f32, pressure: f32, grain: f32) -> f32 {
    if grain <= 0.0 {
        return 1.0;
    }
    // Seuil : bas quand on appuie (seuls les creux profonds restent blancs), haut quand on
    // effleure (seuls les sommets prennent la couleur).
    let t = 0.15 + 0.6 * (1.0 - pressure.clamp(0.0, 1.0));
    let fill = smoothstep(t - 0.12, t + 0.12, height);
    1.0 - grain.clamp(0.0, 1.0) * (1.0 - fill)
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Papier fabriqué, répété sans raccord : un bruit fin (le « grain » d'un papier à dessin),
/// adouci sur `blur` texels (plus = grain plus gros), puis réparti uniformément de 0 à 1 — si
/// bien qu'un seuil de 0,3 laisse 30 % de creux, quelle que soit la texture.
pub fn procedural_paper(size: usize, blur: usize, seed: u32) -> GrayImage {
    // Mélange de MurmurHash3 (`fmix32`) : pas de motif visible entre texels voisins.
    let fmix = |mut h: u32| {
        h ^= h >> 16;
        h = h.wrapping_mul(0x85EB_CA6B);
        h ^= h >> 13;
        h = h.wrapping_mul(0xC2B2_AE35);
        h ^ (h >> 16)
    };
    let hash = |x: usize, y: usize| -> f32 {
        let h = fmix(fmix(seed ^ (x as u32).wrapping_mul(0x9E37_79B1)) ^ (y as u32).wrapping_mul(0x85EB_CA77));
        (h >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut values: Vec<f32> = (0..size * size).map(|i| hash(i % size, i / size)).collect();
    // Flou en boîte, deux passes horizontales et verticales (proche d'un flou gaussien), en
    // bouclant sur les bords pour que le motif se raccorde.
    let r = blur as isize;
    for _ in 0..2 {
        for vertical in [false, true] {
            let src = values.clone();
            for y in 0..size {
                for x in 0..size {
                    let mut sum = 0.0;
                    for d in -r..=r {
                        let (sx, sy) = if vertical { (x as isize, y as isize + d) } else { (x as isize + d, y as isize) };
                        let (sx, sy) = (sx.rem_euclid(size as isize) as usize, sy.rem_euclid(size as isize) as usize);
                        sum += src[sy * size + sx];
                    }
                    values[y * size + x] = sum / (2 * r + 1) as f32;
                }
            }
        }
    }
    // Répartition uniforme : chaque texel prend son rang.
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let n = (values.len() - 1).max(1) as f32;
    let mut ranked = vec![0.0f32; values.len()];
    for (rank, &i) in order.iter().enumerate() {
        ranked[i] = rank as f32 / n;
    }
    GrayImage::new(size, size, ranked)
}

/// Textures d'un trait, prêtes à l'emploi.
#[derive(Clone, Default)]
pub struct BrushTextures {
    pub tip: Option<Arc<Tip>>,
    pub paper: Option<Arc<GrayImage>>,
}

/// Outil auquel s'applique un réglage de pinceau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Target {
    #[serde(rename = "pinceau")]
    Brush,
    #[serde(rename = "gomme")]
    Eraser,
}

/// Un réglage de pinceau nommé.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "outil")]
    pub target: Target,
    #[serde(rename = "reglages")]
    pub settings: BrushSettings,
    /// « Pelure », ou le fichier d'où il a été importé.
    pub source: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum TextureKind {
    #[serde(rename = "pointe")]
    Tip,
    #[serde(rename = "papier")]
    Paper,
}

impl TextureKind {
    /// Nom dans les fichiers de textures d'un projet.
    pub fn file_word(self) -> &'static str {
        match self {
            TextureKind::Tip => "pointe",
            TextureKind::Paper => "papier",
        }
    }
}

// ---------------------------------------------------------------- textures partagées (7b)

/// Image en octets (0 à 255), telle qu'elle est écrite dans un fichier.
pub fn quantized(img: &GrayImage) -> Vec<u8> {
    img.values.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// Clé d'une texture d'après son contenu (sorte, taille, octets) : la même image a la même clé
/// d'une séance et d'une machine à l'autre. Le bit de poids fort est mis : une clé ne ressemble
/// jamais à un numéro de la bibliothèque de pinceaux.
pub fn content_key(kind: TextureKind, img: &GrayImage) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    let mut eat = |b: u8| {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    };
    eat(kind as u8);
    for b in (img.width as u32).to_le_bytes().into_iter().chain((img.height as u32).to_le_bytes()) {
        eat(b);
    }
    for b in quantized(img) {
        eat(b);
    }
    h | 0x8000_0000
}

/// Textures que voient les traits vectoriels texturés, par clé de contenu : celles de la
/// bibliothèque de pinceaux et celles des projets ouverts. Partagées, parce que le rendu d'un
/// dessin vectoriel n'a pas accès à l'application.
#[derive(Default)]
struct Shared {
    tips: HashMap<u32, Arc<Tip>>,
    papers: HashMap<u32, Arc<GrayImage>>,
    /// Numéro dans la bibliothèque → clé de contenu.
    from_library: HashMap<(TextureKind, u32), u32>,
}

static SHARED: LazyLock<RwLock<Shared>> = LazyLock::new(Default::default);

/// Clé de contenu de la texture `id` de la bibliothèque (après `BrushLibrary::share`).
pub fn shared_key(kind: TextureKind, id: u32) -> Option<u32> {
    SHARED.read().ok()?.from_library.get(&(kind, id)).copied()
}

/// Les textures d'un réglage dont `tip` et `paper` sont des clés de contenu (celles qui manquent
/// sont ignorées : pointe ronde, papier lisse).
pub fn shared_textures(s: &BrushSettings) -> BrushTextures {
    let Ok(shared) = SHARED.read() else { return BrushTextures::default() };
    BrushTextures {
        tip: s.tip.and_then(|k| shared.tips.get(&k)).cloned(),
        paper: s.paper.and_then(|k| shared.papers.get(&k)).cloned(),
    }
}

/// L'image d'une texture partagée (pour l'écrire dans un projet).
pub fn shared_image(kind: TextureKind, key: u32) -> Option<GrayImage> {
    let shared = SHARED.read().ok()?;
    match kind {
        TextureKind::Tip => shared.tips.get(&key).map(|t| t.full().clone()),
        TextureKind::Paper => shared.papers.get(&key).map(|p| p.as_ref().clone()),
    }
}

/// Ajoute une texture lue dans un projet, sous sa clé (sans remplacer celle déjà là).
pub fn share_texture(kind: TextureKind, key: u32, img: GrayImage) {
    let Ok(mut shared) = SHARED.write() else { return };
    match kind {
        TextureKind::Tip => {
            shared.tips.entry(key).or_insert_with(|| Arc::new(Tip::new(img)));
        }
        TextureKind::Paper => {
            shared.papers.entry(key).or_insert_with(|| Arc::new(img));
        }
    }
}

/// Pointes, papiers et réglages nommés. Ceux de Pelure sont fabriqués au démarrage ; ceux
/// importés sont gardés dans %APPDATA%\Pelure\pinceaux.
pub struct BrushLibrary {
    tips: HashMap<u32, (String, Arc<Tip>)>,
    papers: HashMap<u32, (String, Arc<GrayImage>)>,
    pub presets: Vec<Preset>,
    next_id: u32,
}

/// Papiers de Pelure.
pub const PAPER_FINE: u32 = 1;
pub const PAPER_COARSE: u32 = 2;
/// Numéros des textures importées : à partir d'ici.
const FIRST_IMPORTED: u32 = 100;

#[derive(Serialize, Deserialize)]
struct SavedTexture {
    id: u32,
    #[serde(rename = "nom")]
    name: String,
    #[serde(rename = "type")]
    kind: TextureKind,
    #[serde(rename = "fichier")]
    file: String,
}

#[derive(Serialize, Deserialize, Default)]
struct SavedLibrary {
    textures: Vec<SavedTexture>,
    #[serde(rename = "reglages")]
    presets: Vec<Preset>,
}

impl BrushLibrary {
    /// Les pinceaux de Pelure (sans les importés).
    pub fn builtin() -> BrushLibrary {
        let mut lib = BrushLibrary {
            tips: HashMap::new(),
            papers: HashMap::new(),
            presets: Vec::new(),
            next_id: FIRST_IMPORTED,
        };
        lib.papers.insert(PAPER_FINE, ("Grain fin".into(), Arc::new(procedural_paper(256, 0, 11))));
        lib.papers.insert(PAPER_COARSE, ("Grain gros".into(), Arc::new(procedural_paper(256, 1, 29))));
        let pelure = |name: &str, target, settings| Preset {
            name: name.into(),
            target,
            settings,
            source: "Pelure".into(),
        };
        let base = BrushSettings::brush();
        lib.presets = vec![
            pelure("Pinceau rond", Target::Brush, base),
            pelure(
                "Crayon à papier",
                Target::Brush,
                BrushSettings {
                    size: 4.0,
                    hardness: 0.9,
                    spacing: 0.06,
                    smoothing: 0.2,
                    min_size: 0.35,
                    pressure_opacity: true,
                    min_opacity: 0.15,
                    grain: 0.85,
                    paper: Some(PAPER_FINE),
                    ..base
                },
            ),
            pelure(
                "Craie grasse",
                Target::Brush,
                BrushSettings {
                    size: 18.0,
                    hardness: 0.6,
                    spacing: 0.05,
                    smoothing: 0.25,
                    min_size: 0.5,
                    pressure_opacity: true,
                    min_opacity: 0.3,
                    grain: 1.0,
                    paper: Some(PAPER_COARSE),
                    ..base
                },
            ),
            pelure(
                "Encre",
                Target::Brush,
                BrushSettings {
                    size: 5.0,
                    hardness: 1.0,
                    spacing: 0.05,
                    smoothing: 0.4,
                    min_size: 0.1,
                    ..base
                },
            ),
            pelure("Gomme", Target::Eraser, BrushSettings::eraser()),
            pelure(
                "Gomme à grain",
                Target::Eraser,
                BrushSettings {
                    grain: 0.8,
                    paper: Some(PAPER_FINE),
                    pressure_opacity: true,
                    min_opacity: 0.2,
                    ..BrushSettings::eraser()
                },
            ),
        ];
        lib
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_tip(&mut self, name: &str, image: GrayImage) -> u32 {
        let id = self.take_id();
        self.tips.insert(id, (name.into(), Arc::new(Tip::new(image))));
        id
    }

    pub fn add_paper(&mut self, name: &str, image: GrayImage) -> u32 {
        let id = self.take_id();
        self.papers.insert(id, (name.into(), Arc::new(image.normalized())));
        id
    }

    /// Met les textures de la bibliothèque à la disposition des traits vectoriels (au démarrage
    /// et après un import).
    pub fn share(&self) {
        let Ok(mut shared) = SHARED.write() else { return };
        for (&id, (_, tip)) in &self.tips {
            let key = content_key(TextureKind::Tip, tip.full());
            shared.tips.entry(key).or_insert_with(|| tip.clone());
            shared.from_library.insert((TextureKind::Tip, id), key);
        }
        for (&id, (_, paper)) in &self.papers {
            let key = content_key(TextureKind::Paper, paper);
            shared.papers.entry(key).or_insert_with(|| paper.clone());
            shared.from_library.insert((TextureKind::Paper, id), key);
        }
    }

    /// Le réglage `s` (numéros de la bibliothèque) pour un trait vectoriel : pointe et papier
    /// désignés par leur clé de contenu. Aucun : le réglage n'a pas de texture.
    pub fn for_vector(&self, s: &BrushSettings) -> Option<BrushSettings> {
        // Un profil de pointe se rejoue aussi (moteur de dessin, étape 3).
        let textured = s.tip.is_some() || (s.paper.is_some() && s.grain > 0.0) || s.profile.is_some();
        if !textured {
            return None;
        }
        // Une texture pas encore partagée (bibliothèque changée depuis) : on partage à nouveau.
        let key = |kind, id: Option<u32>| {
            id.and_then(|id| shared_key(kind, id).or_else(|| {
                self.share();
                shared_key(kind, id)
            }))
        };
        Some(BrushSettings { tip: key(TextureKind::Tip, s.tip), paper: key(TextureKind::Paper, s.paper), ..*s })
    }

    /// Les textures d'un réglage (celles qui manquent sont ignorées).
    pub fn textures(&self, s: &BrushSettings) -> BrushTextures {
        BrushTextures {
            tip: s.tip.and_then(|id| self.tips.get(&id)).map(|t| t.1.clone()),
            paper: s.paper.and_then(|id| self.papers.get(&id)).map(|p| p.1.clone()),
        }
    }

    /// Papiers, par numéro.
    pub fn papers(&self) -> Vec<(u32, String)> {
        let mut v: Vec<(u32, String)> = self.papers.iter().map(|(&id, (n, _))| (id, n.clone())).collect();
        v.sort_by_key(|p| p.0);
        v
    }

    pub fn tip_name(&self, id: u32) -> Option<&str> {
        self.tips.get(&id).map(|t| t.0.as_str())
    }

    /// Dossier des pinceaux importés.
    pub fn user_dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Pelure").join("pinceaux"))
    }

    /// Les pinceaux de Pelure, plus ceux importés et gardés dans `dir`.
    pub fn load(dir: Option<&Path>) -> BrushLibrary {
        let mut lib = BrushLibrary::builtin();
        let Some(dir) = dir else { return lib };
        let Ok(text) = std::fs::read_to_string(dir.join("pinceaux.json")) else { return lib };
        let Ok(saved) = serde_json::from_str::<SavedLibrary>(&text) else { return lib };
        for t in saved.textures {
            let Ok(img) = image::open(dir.join(&t.file)) else { continue };
            let gray = img.to_luma8();
            let values = gray.as_raw().iter().map(|&v| v as f32 / 255.0).collect();
            let image = GrayImage::new(gray.width() as usize, gray.height() as usize, values);
            match t.kind {
                TextureKind::Tip => {
                    lib.tips.insert(t.id, (t.name, Arc::new(Tip::new(image))));
                }
                TextureKind::Paper => {
                    lib.papers.insert(t.id, (t.name, Arc::new(image.normalized())));
                }
            }
            lib.next_id = lib.next_id.max(t.id + 1);
        }
        lib.presets.extend(saved.presets);
        lib.share();
        lib
    }

    /// Garde dans `dir` les pinceaux importés et leurs textures.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut saved = SavedLibrary::default();
        let images = self
            .tips
            .iter()
            .map(|(&id, (name, tip))| (id, name, TextureKind::Tip, tip.full()))
            .chain(self.papers.iter().map(|(&id, (name, p))| (id, name, TextureKind::Paper, p.as_ref())))
            .filter(|t| t.0 >= FIRST_IMPORTED);
        for (id, name, kind, img) in images {
            let file = format!("texture_{id}.png");
            let bytes: Vec<u8> = img.values.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
            image::save_buffer(dir.join(&file), &bytes, img.width as u32, img.height as u32, image::ExtendedColorType::L8)
                .map_err(|e| e.to_string())?;
            saved.textures.push(SavedTexture { id, name: name.clone(), kind, file });
        }
        saved.presets = self.presets.iter().filter(|p| p.source != "Pelure").cloned().collect();
        let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("pinceaux.json"), text).map_err(|e| e.to_string())
    }
}
