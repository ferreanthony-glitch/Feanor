use std::collections::HashMap;

use eframe::egui::{Color32, Pos2};
use serde::{Deserialize, Serialize};

use crate::brushtex::{grain_factor, BrushTextures};
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};
use crate::vector::VPoint;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushSettings {
    /// Diamètre en pixels.
    #[serde(rename = "taille")]
    pub size: f32,
    #[serde(rename = "opacite")]
    pub opacity: f32,
    /// 0 = bord très flou, 1 = bord net.
    #[serde(rename = "durete")]
    pub hardness: f32,
    /// Distance entre deux empreintes, en fraction du diamètre.
    #[serde(rename = "espacement")]
    pub spacing: f32,
    /// Stabilisation du trait (0 = aucune).
    #[serde(rename = "lissage")]
    pub smoothing: f32,
    #[serde(rename = "pression_taille")]
    pub pressure_size: bool,
    #[serde(rename = "pression_opacite")]
    pub pressure_opacity: bool,
    /// Pixels francs, sans anticrénelage (crayon).
    #[serde(rename = "pixels_nets")]
    pub aliased: bool,
    /// Taille (en part de la taille) et opacité quand on effleure, si elles suivent la pression.
    #[serde(rename = "taille_min")]
    pub min_size: f32,
    #[serde(rename = "opacite_min")]
    pub min_opacity: f32,
    /// Force du grain du papier (0 = papier lisse).
    pub grain: f32,
    /// Papier et pointe en image (numéros dans `BrushLibrary`) ; pas de pointe : pointe ronde.
    #[serde(rename = "papier")]
    pub paper: Option<u32>,
    #[serde(rename = "pointe")]
    pub tip: Option<u32>,
    /// Pointe aplatie (1 = ronde) et tournée de `angle` degrés, plus un hasard de ± `angle_jitter`/2.
    #[serde(rename = "rondeur")]
    pub roundness: f32,
    pub angle: f32,
    #[serde(rename = "angle_hasard")]
    pub angle_jitter: f32,
    /// Variation de taille au hasard, en part de la taille.
    #[serde(rename = "taille_hasard")]
    pub size_jitter: f32,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self::brush()
    }
}

impl BrushSettings {
    pub fn brush() -> Self {
        Self {
            size: 6.0,
            opacity: 1.0,
            hardness: 0.85,
            spacing: 0.08,
            smoothing: 0.3,
            pressure_size: true,
            pressure_opacity: false,
            aliased: false,
            min_size: 0.0,
            min_opacity: 0.0,
            grain: 0.0,
            paper: None,
            tip: None,
            roundness: 1.0,
            angle: 0.0,
            angle_jitter: 0.0,
            size_jitter: 0.0,
        }
    }

    pub fn pencil() -> Self {
        Self {
            size: 2.0,
            hardness: 1.0,
            spacing: 0.1,
            smoothing: 0.0,
            aliased: true,
            ..Self::brush()
        }
    }

    pub fn eraser() -> Self {
        Self {
            size: 24.0,
            hardness: 0.7,
            smoothing: 0.2,
            ..Self::brush()
        }
    }
}

/// Aperçu d'un réglage (grille des pinceaux) : un trait en vague tracé par le vrai moteur dans
/// une image `w`×`h`, effleuré aux bouts et appuyé au milieu. Renvoie la couverture (0 à 255)
/// de chaque pixel, ligne par ligne. La taille est ramenée à ce qui tient dans l'image ; une
/// gomme est montrée comme le trait qu'elle enlève.
pub fn stroke_preview(settings: &BrushSettings, textures: BrushTextures, w: usize, h: usize) -> Vec<u8> {
    let mut s = *settings;
    s.size = s.size.clamp(1.0, h as f32 * 0.5);
    s.smoothing = 0.0;
    let mut raster = Raster::new(w, h);
    let mut stroke = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true).with_textures(textures);
    let mut dirty = Vec::new();
    let margin = s.size * 0.5 + 2.0;
    let swing = (h as f32 * 0.5 - margin).max(0.0) * 0.7;
    let n = 64;
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let p = Pos2::new(
            margin + t * (w as f32 - 2.0 * margin),
            h as f32 * 0.5 - (t * std::f32::consts::TAU).sin() * swing,
        );
        stroke.add_point(&mut raster, p, (t * std::f32::consts::PI).sin().max(0.15), &mut dirty);
    }
    stroke.finish(&mut raster, &mut dirty);
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| raster.get(x, y).a()).collect()
}

#[derive(Clone, Copy)]
pub enum StrokeMode {
    Paint([u8; 3]),
    Erase,
}

/// Un trait en cours.
///
/// Chaque empreinte écrit dans un masque propre au trait (on garde le maximum), puis le pixel
/// est recalculé depuis son état d'avant le trait. Le chevauchement des empreintes ne
/// fonce donc pas le trait : une opacité de 50 % donne un trait uniforme à 50 %.
pub struct StrokeState {
    pub cel: CelId,
    /// Le trait vient d'un stylet (pression lue sur la tablette) plutôt que de la souris.
    pub pen: bool,
    /// Ligne médiane lissée et rayon en chaque point : ce que garde un trait vectoriel.
    pub path: Vec<VPoint>,
    mode: StrokeMode,
    settings: BrushSettings,
    before: HashMap<usize, Option<Tile>>,
    mask: HashMap<usize, Box<[f32; TILE_PIXELS]>>,
    prefix: Option<Edit>,
    last: Option<(Pos2, f32)>,
    smoothed: Option<Pos2>,
    raw: Option<(Pos2, f32)>,
    since_dab: f32,
    /// Pointe et papier en image (aucun : pointe ronde, papier lisse).
    textures: BrushTextures,
    /// Hasard (angle, taille des empreintes), le même à chaque fois pour un même trait.
    rng: u32,
    /// Préserver la transparence (13b) : le pinceau ne colore que ce qui est déjà dessiné.
    keep_alpha: bool,
    /// Pochoirs (13b) : où le trait peut peindre ; aucun : partout.
    stencil: Option<crate::stencil::Stencil>,
}

impl StrokeState {
    pub fn new(
        cel: CelId,
        mode: StrokeMode,
        settings: BrushSettings,
        prefix: Option<Edit>,
        pen: bool,
    ) -> Self {
        Self {
            cel,
            pen,
            path: Vec::new(),
            mode,
            settings,
            before: HashMap::new(),
            mask: HashMap::new(),
            prefix,
            last: None,
            smoothed: None,
            raw: None,
            since_dab: 0.0,
            textures: BrushTextures::default(),
            rng: 0x9E37_79B9,
            keep_alpha: false,
            stencil: None,
        }
    }

    /// Réglages du calque peint (13b) : préserver la transparence, pochoirs des autres calques.
    pub fn on_layer(mut self, keep_alpha: bool, stencil: Option<crate::stencil::Stencil>) -> Self {
        self.keep_alpha = keep_alpha;
        self.stencil = stencil;
        self
    }

    /// Bout du trait : le dernier point posé, la pointe réelle du stylet et son rayon. Le
    /// lissage (et l'espacement des empreintes) laissent un petit écart entre les deux, que
    /// l'affichage comble pour que le trait reste attaché à la pointe.
    pub fn tip(&self) -> Option<(Pos2, Pos2, f32)> {
        let (last, _) = self.last?;
        let (raw, pressure) = self.raw?;
        Some((last, raw, self.settings.size * 0.5 * self.size_factor(pressure)))
    }

    /// Couleur et opacité du trait à la pointe (aucune pour la gomme).
    pub fn tip_paint(&self) -> Option<([u8; 3], f32)> {
        let StrokeMode::Paint(rgb) = self.mode else { return None };
        let s = self.settings;
        let pressure = self.raw.map_or(1.0, |r| r.1);
        let flow = if s.pressure_opacity {
            s.min_opacity + (1.0 - s.min_opacity) * pressure.clamp(0.0, 1.0)
        } else {
            1.0
        };
        Some((rgb, (s.opacity * flow).clamp(0.0, 1.0)))
    }

    /// Pointe et papier du trait (pinceaux texturés).
    pub fn with_textures(mut self, textures: BrushTextures) -> Self {
        self.textures = textures;
        self
    }

    /// Nombre au hasard entre -0,5 et 0,5.
    fn jitter(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) - 0.5
    }

    pub fn add_point(&mut self, raster: &mut Raster, pos: Pos2, pressure: f32, dirty: &mut Vec<usize>) {
        self.raw = Some((pos, pressure));
        let follow = 1.0 - self.settings.smoothing.clamp(0.0, 0.95);
        let target = match self.smoothed {
            Some(s) => s + (pos - s) * follow,
            None => pos,
        };
        self.smoothed = Some(target);
        self.record(target, pressure);
        self.move_to(raster, target, pressure, dirty);
    }

    fn record(&mut self, p: Pos2, pressure: f32) {
        let r = self.settings.size * 0.5 * self.size_factor(pressure);
        self.path.push(VPoint { x: p.x, y: p.y, r });
    }

    /// Termine le trait (en rattrapant le retard du lissage) et renvoie de quoi l'annuler.
    pub fn finish(&mut self, raster: &mut Raster, dirty: &mut Vec<usize>) -> Option<Edit> {
        if let (Some((p, pr)), Some(s)) = (self.raw, self.smoothed) {
            if p != s {
                self.move_to(raster, p, pr, dirty);
                self.record(p, pr);
            }
        }
        let tiles: Vec<_> = std::mem::take(&mut self.before).into_iter().collect();
        let pixels = (!tiles.is_empty()).then_some(Edit::Tiles {
            cel: self.cel,
            tiles,
        });
        match (self.prefix.take(), pixels) {
            (Some(pre), Some(px)) => Some(Edit::Group(vec![pre, px])),
            (Some(pre), None) => Some(pre),
            (None, px) => px,
        }
    }

    fn size_factor(&self, pressure: f32) -> f32 {
        if self.settings.pressure_size {
            let min = self.settings.min_size.clamp(0.0, 1.0);
            (min + (1.0 - min) * pressure.clamp(0.0, 1.0)).max(0.03)
        } else {
            1.0
        }
    }

    fn step(&self, pressure: f32) -> f32 {
        let diameter = self.settings.size * self.size_factor(pressure);
        (diameter * self.settings.spacing).max(0.3)
    }

    /// Pose des empreintes régulièrement espacées du dernier point jusqu'à `to`.
    fn move_to(&mut self, raster: &mut Raster, to: Pos2, pressure: f32, dirty: &mut Vec<usize>) {
        let Some((from, p0)) = self.last else {
            self.dab(raster, to, pressure, dirty);
            self.last = Some((to, pressure));
            self.since_dab = 0.0;
            return;
        };
        let dist = from.distance(to);
        if dist < 1e-3 {
            self.last = Some((from, pressure));
            return;
        }
        let mut travelled = 0.0;
        loop {
            let pr = p0 + (pressure - p0) * (travelled / dist);
            let need = (self.step(pr) - self.since_dab).max(0.0);
            if travelled + need > dist {
                self.since_dab += dist - travelled;
                break;
            }
            travelled += need;
            let t = travelled / dist;
            self.dab(raster, from + (to - from) * t, p0 + (pressure - p0) * t, dirty);
            self.since_dab = 0.0;
        }
        self.last = Some((to, pressure));
    }

    fn dab(&mut self, raster: &mut Raster, c: Pos2, pressure: f32, dirty: &mut Vec<usize>) {
        let s = self.settings;
        let mode = self.mode;
        let mut radius = s.size * 0.5 * self.size_factor(pressure);
        if s.size_jitter > 0.0 {
            radius *= (1.0 + s.size_jitter * 2.0 * self.jitter()).max(0.05);
        }
        let mut flow = if s.pressure_opacity {
            let min = s.min_opacity.clamp(0.0, 1.0);
            min + (1.0 - min) * pressure.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if s.aliased {
            radius = radius.max(0.71);
        } else if radius < 0.75 {
            // Plus fin qu'un pixel : on éclaircit au lieu d'amincir.
            flow *= radius / 0.75;
            radius = 0.75;
        }
        if flow <= 0.0 {
            return;
        }
        let inner = (radius * s.hardness.clamp(0.0, 1.0)).min(radius - 0.5).max(0.0);
        // Pointe aplatie ou tournée : on mesure chaque pixel dans le repère de la pointe.
        let angle = (s.angle + s.angle_jitter * self.jitter()).to_radians();
        let (sin, cos) = angle.sin_cos();
        let squash = s.roundness.clamp(0.05, 1.0);
        let shaped = squash < 1.0 || self.textures.tip.is_some();
        let tip = self.textures.tip.clone();
        let tip = tip.as_ref().map(|t| t.level(radius * 2.0));
        let paper = self.textures.paper.clone().filter(|_| s.grain > 0.0);
        // Une pointe en image tournée déborde du cercle : on couvre le carré qui la contient.
        let reach = if tip.is_some() { radius * std::f32::consts::SQRT_2 } else { radius };

        let x0 = (c.x - reach - 1.0).floor().max(0.0) as usize;
        let y0 = (c.y - reach - 1.0).floor().max(0.0) as usize;
        let x1 = (c.x + reach + 1.0).ceil().clamp(0.0, raster.width as f32) as usize;
        let y1 = (c.y + reach + 1.0).ceil().clamp(0.0, raster.height as f32) as usize;
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        // Couverture d'un pixel par l'empreinte, à (dx, dy) de son centre.
        let footprint = |dx: f32, dy: f32| -> f32 {
            let (lx, ly) = if shaped {
                (dx * cos + dy * sin, (-dx * sin + dy * cos) / squash)
            } else {
                (dx, dy)
            };
            match tip {
                Some(img) => {
                    let u = (lx / (2.0 * radius) + 0.5) * img.width as f32;
                    let v = (ly / (2.0 * radius) + 0.5) * img.height as f32;
                    img.sample(u, v)
                }
                None => coverage((lx * lx + ly * ly).sqrt(), radius, inner, s.aliased),
            }
        };

        // Préserver la transparence : on ne peint que sur ce qui est déjà dessiné (13b).
        let keep_alpha = self.keep_alpha && matches!(mode, StrokeMode::Paint(_));
        let stencil = self.stencil.as_ref();
        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                let idx = ty * raster.tiles_x + tx;
                if (matches!(mode, StrokeMode::Erase) || keep_alpha) && raster.tiles[idx].is_none() {
                    continue;
                }
                if stencil.is_some_and(|st| st.blocks(idx)) {
                    continue;
                }
                let (ox, oy) = (tx * TILE, ty * TILE);
                if !self.before.contains_key(&idx) {
                    self.before.insert(idx, raster.tiles[idx].clone());
                }
                let before = self.before[&idx].as_deref();
                let mask = self
                    .mask
                    .entry(idx)
                    .or_insert_with(|| Box::new([0.0; TILE_PIXELS]));
                let tile = raster.tile_mut(idx);
                let mut changed = false;
                for py in y0.max(oy)..y1.min(oy + TILE) {
                    let dy = py as f32 + 0.5 - c.y;
                    for px in x0.max(ox)..x1.min(ox + TILE) {
                        let dx = px as f32 + 0.5 - c.x;
                        let cov = footprint(dx, dy);
                        if cov <= 0.0 {
                            continue;
                        }
                        // Grain du papier, fixe sur le dessin.
                        let grain = paper.as_ref().map_or(1.0, |p| grain_factor(p.tiled(px, py), pressure, s.grain));
                        let i = (py - oy) * TILE + (px - ox);
                        let through = stencil.map_or(1.0, |st| st.at(idx, i));
                        let v = cov * flow * grain * through;
                        if v <= mask[i] {
                            continue;
                        }
                        mask[i] = v;
                        let b = before.map_or(Color32::TRANSPARENT, |t| t[i]);
                        tile[i] = match mode {
                            StrokeMode::Paint(rgb) if keep_alpha => paint_over(b, rgb, v * s.opacity),
                            StrokeMode::Paint(rgb) => paint(b, rgb, v * s.opacity),
                            StrokeMode::Erase => erase(b, v * s.opacity),
                        };
                        changed = true;
                    }
                }
                if changed {
                    dirty.push(idx);
                }
            }
        }
    }
}

pub(crate) fn coverage(d: f32, radius: f32, inner: f32, aliased: bool) -> f32 {
    if aliased {
        return if d <= radius { 1.0 } else { 0.0 };
    }
    let edge = (radius - d + 0.5).clamp(0.0, 1.0);
    if edge <= 0.0 || d <= inner {
        return edge;
    }
    let t = ((d - inner) / (radius - inner).max(1e-3)).clamp(0.0, 1.0);
    edge.min(1.0 - t * t * (3.0 - 2.0 * t))
}

pub(crate) fn paint(b: Color32, rgb: [u8; 3], a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let ia = 1.0 - a;
    let ch = |c: u8, d: u8| (c as f32 * a + d as f32 * ia + 0.5) as u8;
    Color32::from_rgba_premultiplied(
        ch(rgb[0], b.r()),
        ch(rgb[1], b.g()),
        ch(rgb[2], b.b()),
        (255.0 * a + b.a() as f32 * ia + 0.5) as u8,
    )
}

/// Comme `paint`, mais l'opacité du pixel ne change pas (préserver la transparence, 13b) : seule
/// sa couleur va vers `rgb`. Un pixel vide reste vide.
pub(crate) fn paint_over(b: Color32, rgb: [u8; 3], a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let ia = 1.0 - a;
    let k = b.a() as f32 / 255.0;
    let ch = |c: u8, d: u8| ((c as f32 * k * a + d as f32 * ia + 0.5) as u8).min(b.a());
    Color32::from_rgba_premultiplied(ch(rgb[0], b.r()), ch(rgb[1], b.g()), ch(rgb[2], b.b()), b.a())
}

fn erase(b: Color32, a: f32) -> Color32 {
    let k = 1.0 - a.clamp(0.0, 1.0);
    let ch = |d: u8| (d as f32 * k + 0.5) as u8;
    Color32::from_rgba_premultiplied(ch(b.r()), ch(b.g()), ch(b.b()), ch(b.a()))
}
