//! Tampons (5e), sans interface.
//!
//! - **Tampon** (comme l'outil Stamp de Storyboard Pro 27, d'après ses réglages,
//!   `resources/editors/stampbrushprops.xml`) : des images posées le long du trait, à intervalles
//!   réguliers ; taille selon la pression (entre une taille minimale et maximale), rondeur et
//!   angle, espacement, variations au hasard de la taille, de l'angle et de l'espacement, miroirs,
//!   plusieurs images en cycle (dans l'ordre ou au hasard). Le rendu exact de Storyboard Pro n'a
//!   pas été comparé.
//! - **Tampon de duplication** (comme dans les logiciels de retouche) : on peint, à l'endroit du
//!   pinceau, ce qu'il y a à un décalage fixe (la source).

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{pos2, Color32, Pos2, Vec2};

use crate::brush::coverage;
use crate::raster::{premultiply, Raster, Tile, TILE};
use crate::render::over;

/// Image d'un tampon (pixels prémultipliés). `mask` : forme blanche que la couleur courante
/// colore (tampons de Pelure) ; sinon image en couleurs.
#[derive(Clone, Debug)]
pub struct StampImage {
    pub name: String,
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<Color32>,
    pub mask: bool,
}

impl StampImage {
    /// Image RGBA (non prémultipliée), par exemple un PNG importé.
    pub fn from_rgba(name: String, w: usize, h: usize, rgba: &[u8]) -> StampImage {
        let pixels = rgba.chunks_exact(4).map(|p| premultiply(p[0], p[1], p[2], p[3])).collect();
        StampImage { name, w, h, pixels, mask: false }
    }

    /// Forme blanche de transparence `alpha` (0 à 1).
    pub fn from_alpha(name: &str, w: usize, h: usize, alpha: &[f32]) -> StampImage {
        let pixels = alpha.iter().map(|a| Color32::from_white_alpha((a.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
        StampImage { name: name.to_string(), w, h, pixels, mask: true }
    }

    /// Pixel de l'image en (u, v) (pixels de l'image, bilinéaire ; transparent au-dehors).
    pub fn sample(&self, u: f32, v: f32) -> Color32 {
        let (x, y) = (u - 0.5, v - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let get = |xi: f32, yi: f32| -> [f32; 4] {
            if xi < 0.0 || yi < 0.0 || xi >= self.w as f32 || yi >= self.h as f32 {
                return [0.0; 4];
            }
            let c = self.pixels[yi as usize * self.w + xi as usize];
            [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
        };
        let (a, b, c, d) = (get(x0, y0), get(x0 + 1.0, y0), get(x0, y0 + 1.0), get(x0 + 1.0, y0 + 1.0));
        let mix = |k: usize| {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bottom = c[k] + (d[k] - c[k]) * fx;
            (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8
        };
        Color32::from_rgba_premultiplied(mix(0), mix(1), mix(2), mix(3))
    }
}

/// Ordre des images d'un tampon qui en a plusieurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    InOrder,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StampSettings {
    /// Largeur la plus grande d'un tampon (pression pleine), en pixels.
    pub size: f32,
    /// Taille minimale (pression nulle), en part de la plus grande.
    pub min_size: f32,
    /// Espacement entre deux tampons, en part de leur taille.
    pub spacing: f32,
    /// Angle (degrés) et rondeur (1 = image telle quelle, moins = aplatie).
    pub angle: f32,
    pub roundness: f32,
    /// Variations au hasard : taille et espacement (part), angle (degrés, écart total).
    pub random_size: f32,
    pub random_angle: f32,
    pub random_spacing: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub opacity: f32,
    /// Colorer aussi les images en couleurs avec la couleur courante.
    pub tint: bool,
    pub cycle: Cycle,
    /// Chaque trait reprend le cycle à la première image.
    pub reset_cycle: bool,
}

impl Default for StampSettings {
    fn default() -> Self {
        StampSettings {
            size: 80.0,
            min_size: 0.5,
            spacing: 1.0,
            angle: 0.0,
            roundness: 1.0,
            random_size: 0.3,
            random_angle: 40.0,
            random_spacing: 0.2,
            flip_x: false,
            flip_y: false,
            opacity: 1.0,
            tint: false,
            cycle: Cycle::InOrder,
            reset_cycle: true,
        }
    }
}

/// Un tampon posé : centre, largeur (pixels), angle (radians), image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dab {
    pub center: Pos2,
    pub size: f32,
    pub angle: f32,
    pub image: usize,
}

/// Les tampons d'un trait, au fur et à mesure.
pub struct StampTrail {
    settings: StampSettings,
    images: usize,
    rng: u64,
    next_image: usize,
    last: Option<Pos2>,
    /// Chemin restant avant le prochain tampon.
    to_next: f32,
}

impl StampTrail {
    /// `images` : nombre d'images du tampon ; `first` : première image (cycle continué).
    pub fn new(settings: StampSettings, images: usize, seed: u64, first: usize) -> StampTrail {
        StampTrail { settings, images: images.max(1), rng: seed | 1, next_image: first, last: None, to_next: 0.0 }
    }

    /// Nombre au hasard entre 0 et 1 (suite reproductible).
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Image du cycle à reprendre au trait suivant.
    pub fn next_image(&self) -> usize {
        self.next_image
    }

    fn base_size(&self, pressure: f32) -> f32 {
        let s = &self.settings;
        s.size * (s.min_size + (1.0 - s.min_size) * pressure.clamp(0.0, 1.0))
    }

    fn gap(&mut self, pressure: f32) -> f32 {
        let jitter = 1.0 + self.settings.random_spacing * (self.rand() * 2.0 - 1.0);
        (self.settings.spacing * self.base_size(pressure) * jitter).max(1.0)
    }

    fn dab(&mut self, at: Pos2, pressure: f32) -> Dab {
        let s = self.settings;
        let size = (self.base_size(pressure) * (1.0 + s.random_size * (self.rand() * 2.0 - 1.0))).max(1.0);
        let angle = (s.angle + s.random_angle * (self.rand() - 0.5)).to_radians();
        let image = match s.cycle {
            Cycle::InOrder => {
                let i = self.next_image % self.images;
                self.next_image = (i + 1) % self.images;
                i
            }
            Cycle::Random => ((self.rand() * self.images as f32) as usize).min(self.images - 1),
        };
        Dab { center: at, size, angle, image }
    }

    /// Le trait passe par `p` : tampons à poser depuis le point précédent.
    pub fn add_point(&mut self, p: Pos2, pressure: f32) -> Vec<Dab> {
        let Some(mut from) = self.last else {
            self.last = Some(p);
            let d = self.dab(p, pressure);
            self.to_next = self.gap(pressure);
            return vec![d];
        };
        let mut out = Vec::new();
        let mut left = from.distance(p);
        while left >= self.to_next {
            let dir = (p - from) / from.distance(p).max(1e-6);
            let at = from + dir * self.to_next;
            left -= self.to_next;
            from = at;
            out.push(self.dab(at, pressure));
            self.to_next = self.gap(pressure);
        }
        self.to_next -= left;
        self.last = Some(p);
        out
    }
}

/// Garde la tuile `idx` telle qu'elle était avant le geste (une fois).
fn keep(raster: &Raster, before: &mut HashMap<usize, Option<Tile>>, idx: usize) {
    before.entry(idx).or_insert_with(|| raster.tiles[idx].clone());
}

/// Pose un tampon dans le dessin ; renvoie les tuiles touchées.
pub fn place(
    raster: &mut Raster,
    img: &StampImage,
    dab: &Dab,
    s: &StampSettings,
    color: [u8; 3],
    before: &mut HashMap<usize, Option<Tile>>,
) -> Vec<usize> {
    if img.w == 0 || img.h == 0 {
        return Vec::new();
    }
    let k = dab.size / img.w as f32;
    let height = img.h as f32 * k * s.roundness.clamp(0.01, 1.0);
    let reach = 0.5 * (dab.size * dab.size + height * height).sqrt() + 1.0;
    let (sin, cos) = dab.angle.sin_cos();
    let opacity = (s.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    let tint = (img.mask || s.tint).then_some(color);
    let x0 = (dab.center.x - reach).floor().max(0.0) as usize;
    let y0 = (dab.center.y - reach).floor().max(0.0) as usize;
    let x1 = ((dab.center.x + reach).ceil().max(0.0) as usize).min(raster.width);
    let y1 = ((dab.center.y + reach).ceil().max(0.0) as usize).min(raster.height);
    let mut dirty = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let d = pos2(x as f32 + 0.5, y as f32 + 0.5) - dab.center;
            // Dans le repère du tampon (tourné, à l'échelle, aplati, retourné).
            let (lx, ly) = (d.x * cos + d.y * sin, -d.x * sin + d.y * cos);
            let mut u = lx / k;
            let mut v = ly / (k * s.roundness.clamp(0.01, 1.0));
            if s.flip_x {
                u = -u;
            }
            if s.flip_y {
                v = -v;
            }
            let src = img.sample(u + img.w as f32 * 0.5, v + img.h as f32 * 0.5);
            if src.a() == 0 {
                continue;
            }
            let idx = raster.tile_index_of(x, y);
            keep(raster, before, idx);
            let i = (y % TILE) * TILE + x % TILE;
            let tile = raster.tile_mut(idx);
            tile[i] = over(tile[i], src, opacity, tint);
            if dirty.last() != Some(&idx) && !dirty.contains(&idx) {
                dirty.push(idx);
            }
        }
    }
    dirty
}

// ------------------------------------------------------------------------------ duplication

/// Ce que copie le tampon de duplication : le calque courant, ou tous les calques visibles,
/// tels qu'ils étaient au début du trait.
pub enum CloneSource {
    Layer(Raster),
    Composite { pixels: Vec<Color32>, width: usize, height: usize },
}

impl CloneSource {
    fn get(&self, x: isize, y: isize) -> Color32 {
        match self {
            CloneSource::Layer(r) => {
                if x < 0 || y < 0 || x as usize >= r.width || y as usize >= r.height {
                    Color32::TRANSPARENT
                } else {
                    r.get(x as usize, y as usize)
                }
            }
            CloneSource::Composite { pixels, width, height } => {
                if x < 0 || y < 0 || x as usize >= *width || y as usize >= *height {
                    Color32::TRANSPARENT
                } else {
                    pixels[y as usize * width + x as usize]
                }
            }
        }
    }
}

/// Un trait de tampon de duplication : chaque pixel reçoit, une seule fois (pas d'accumulation
/// d'un passage à l'autre), le pixel de la source décalé de `offset`.
pub struct CloneStroke {
    source: CloneSource,
    /// Décalage du pinceau vers la source (pixels entiers : copie exacte).
    offset: (isize, isize),
    radius: f32,
    hardness: f32,
    opacity: f32,
    /// Couverture déjà donnée à chaque pixel, par tuile.
    cover: HashMap<usize, Vec<f32>>,
    pub before: HashMap<usize, Option<Tile>>,
    last: Option<Pos2>,
}

impl CloneStroke {
    pub fn new(source: CloneSource, offset: Vec2, size: f32, hardness: f32, opacity: f32) -> CloneStroke {
        CloneStroke {
            source,
            offset: (offset.x.round() as isize, offset.y.round() as isize),
            radius: (size * 0.5).max(0.5),
            hardness: hardness.clamp(0.0, 1.0),
            opacity: opacity.clamp(0.0, 1.0),
            cover: HashMap::new(),
            before: HashMap::new(),
            last: None,
        }
    }

    /// Le pinceau passe par `p` ; renvoie les tuiles touchées.
    pub fn add_point(&mut self, raster: &mut Raster, p: Pos2) -> Vec<usize> {
        let step = (self.radius * 0.25).max(0.5);
        let points: Vec<Pos2> = match self.last {
            None => vec![p],
            Some(a) => {
                let n = (a.distance(p) / step).ceil().max(1.0) as usize;
                (1..=n).map(|i| a + (p - a) * (i as f32 / n as f32)).collect()
            }
        };
        self.last = Some(p);
        let mut dirty = Vec::new();
        for c in points {
            self.dab(raster, c, &mut dirty);
        }
        dirty
    }

    fn dab(&mut self, raster: &mut Raster, c: Pos2, dirty: &mut Vec<usize>) {
        let r = self.radius;
        let inner = (r * self.hardness).min(r - 0.5).max(0.0);
        let x0 = (c.x - r - 1.0).floor().max(0.0) as usize;
        let y0 = (c.y - r - 1.0).floor().max(0.0) as usize;
        let x1 = ((c.x + r + 1.0).ceil().max(0.0) as usize).min(raster.width);
        let y1 = ((c.y + r + 1.0).ceil().max(0.0) as usize).min(raster.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let d = pos2(x as f32 + 0.5, y as f32 + 0.5).distance(c);
                let v = coverage(d, r, inner, false);
                if v <= 0.0 {
                    continue;
                }
                let idx = raster.tile_index_of(x, y);
                let i = (y % TILE) * TILE + x % TILE;
                let cover = self.cover.entry(idx).or_insert_with(|| vec![0.0; TILE * TILE]);
                if v <= cover[i] {
                    continue;
                }
                cover[i] = v;
                keep(raster, &mut self.before, idx);
                let original = self.before[&idx].as_ref().map_or(Color32::TRANSPARENT, |t| t[i]);
                let src = self.source.get(x as isize + self.offset.0, y as isize + self.offset.1);
                let tile = raster.tile_mut(idx);
                tile[i] = over(original, src, (v * self.opacity * 255.0).round() as u8, None);
                if !dirty.contains(&idx) {
                    dirty.push(idx);
                }
            }
        }
    }
}

// ------------------------------------------------------------------------- tampons intégrés

const SIDE: usize = 128;

fn polygon_mask(name: &str, contour: Vec<[f32; 2]>) -> StampImage {
    let mut alpha = vec![0.0; SIDE * SIDE];
    if let Some((x0, y0, w, h, data)) = crate::vector::polygon_coverage(&[contour], SIDE, SIDE) {
        for y in 0..h {
            for x in 0..w {
                alpha[(y0 + y) * SIDE + x0 + x] = data[y * w + x];
            }
        }
    }
    StampImage::from_alpha(name, SIDE, SIDE, &alpha)
}

/// Tampons fournis avec Pelure : des formes blanches, colorées par la couleur courante.
pub fn builtin() -> Vec<Arc<StampImage>> {
    let c = SIDE as f32 * 0.5;
    let mut out = Vec::new();
    // Point doux.
    let soft: Vec<f32> = (0..SIDE * SIDE)
        .map(|i| {
            let d = pos2((i % SIDE) as f32 + 0.5, (i / SIDE) as f32 + 0.5).distance(pos2(c, c)) / (c - 2.0);
            let t = (1.0 - d).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect();
    out.push(StampImage::from_alpha("Point doux", SIDE, SIDE, &soft));
    // Feuille : pointue aux deux bouts, couchée.
    let leaf: Vec<[f32; 2]> = (0..=64)
        .map(|k| {
            let t = k as f32 / 64.0 * std::f32::consts::TAU;
            let x = t.cos();
            let half = 0.42 * (1.0 - x * x).max(0.0).powf(0.7);
            [c + x * (c - 4.0), c + t.sin().signum() * half * (c - 4.0)]
        })
        .collect();
    out.push(polygon_mask("Feuille", leaf));
    // Brin d'herbe : fin, courbé, pointu en haut.
    let mut blade: Vec<[f32; 2]> = Vec::new();
    for k in 0..=20 {
        let t = k as f32 / 20.0;
        let bend = 18.0 * t * t;
        blade.push([c - 7.0 * (1.0 - t) + bend, SIDE as f32 - 4.0 - t * (SIDE as f32 - 8.0)]);
    }
    for k in (0..=20).rev() {
        let t = k as f32 / 20.0;
        let bend = 18.0 * t * t;
        blade.push([c + 7.0 * (1.0 - t) + bend, SIDE as f32 - 4.0 - t * (SIDE as f32 - 8.0)]);
    }
    out.push(polygon_mask("Brin d'herbe", blade));
    // Étoile à cinq branches.
    let star: Vec<[f32; 2]> = (0..10)
        .map(|k| {
            let t = k as f32 / 10.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let r = if k % 2 == 0 { c - 4.0 } else { (c - 4.0) * 0.42 };
            [c + r * t.cos(), c + r * t.sin()]
        })
        .collect();
    out.push(polygon_mask("Étoile", star));
    // Caillou : contour irrégulier (toujours le même).
    let pebble: Vec<[f32; 2]> = (0..24)
        .map(|k| {
            let t = k as f32 / 24.0 * std::f32::consts::TAU;
            let r = (c - 10.0) * (0.82 + 0.1 * (3.0 * t).sin() + 0.06 * (5.0 * t + 1.0).cos());
            [c + r * t.cos(), c + r * t.sin() * 0.75]
        })
        .collect();
    out.push(polygon_mask("Caillou", pebble));
    // Nuage : des disques mêlés.
    let puffs = [(0.0, 0.1, 0.36), (-0.42, 0.18, 0.26), (0.42, 0.2, 0.27), (-0.18, -0.2, 0.3), (0.22, -0.16, 0.28)];
    let cloud: Vec<f32> = (0..SIDE * SIDE)
        .map(|i| {
            let p = pos2((i % SIDE) as f32 + 0.5, (i / SIDE) as f32 + 0.5);
            puffs
                .iter()
                .map(|&(dx, dy, r)| {
                    let d = p.distance(pos2(c + dx * c, c + dy * c));
                    (r * c - d + 0.5).clamp(0.0, 1.0)
                })
                .fold(0.0f32, f32::max)
        })
        .collect();
    out.push(StampImage::from_alpha("Nuage", SIDE, SIDE, &cloud));
    out.into_iter().map(Arc::new).collect()
}
