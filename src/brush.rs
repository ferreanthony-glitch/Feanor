use std::collections::HashMap;

use eframe::egui::{Color32, Pos2};

use crate::document::CelId;
use crate::history::Edit;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};
use crate::vector::VPoint;

#[derive(Clone, Copy, PartialEq)]
pub struct BrushSettings {
    /// Diamètre en pixels.
    pub size: f32,
    pub opacity: f32,
    /// 0 = bord très flou, 1 = bord net.
    pub hardness: f32,
    /// Distance entre deux empreintes, en fraction du diamètre.
    pub spacing: f32,
    /// Stabilisation du trait (0 = aucune).
    pub smoothing: f32,
    pub pressure_size: bool,
    pub pressure_opacity: bool,
    /// Pixels francs, sans anticrénelage (crayon).
    pub aliased: bool,
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
        }
    }

    pub fn pencil() -> Self {
        Self {
            size: 2.0,
            opacity: 1.0,
            hardness: 1.0,
            spacing: 0.1,
            smoothing: 0.0,
            pressure_size: true,
            pressure_opacity: false,
            aliased: true,
        }
    }

    pub fn eraser() -> Self {
        Self {
            size: 24.0,
            opacity: 1.0,
            hardness: 0.7,
            spacing: 0.08,
            smoothing: 0.2,
            pressure_size: true,
            pressure_opacity: false,
            aliased: false,
        }
    }
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
        }
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
            pressure.clamp(0.0, 1.0).max(0.03)
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
        let mut flow = if s.pressure_opacity {
            pressure.clamp(0.0, 1.0)
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

        let x0 = (c.x - radius - 1.0).floor().max(0.0) as usize;
        let y0 = (c.y - radius - 1.0).floor().max(0.0) as usize;
        let x1 = (c.x + radius + 1.0).ceil().clamp(0.0, raster.width as f32) as usize;
        let y1 = (c.y + radius + 1.0).ceil().clamp(0.0, raster.height as f32) as usize;
        if x0 >= x1 || y0 >= y1 {
            return;
        }

        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                let idx = ty * raster.tiles_x + tx;
                if matches!(mode, StrokeMode::Erase) && raster.tiles[idx].is_none() {
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
                        let cov = coverage((dx * dx + dy * dy).sqrt(), radius, inner, s.aliased);
                        let v = cov * flow;
                        let i = (py - oy) * TILE + (px - ox);
                        if v <= mask[i] {
                            continue;
                        }
                        mask[i] = v;
                        let b = before.map_or(Color32::TRANSPARENT, |t| t[i]);
                        tile[i] = match mode {
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

fn erase(b: Color32, a: f32) -> Color32 {
    let k = 1.0 - a.clamp(0.0, 1.0);
    let ch = |d: u8| (d as f32 * k + 0.5) as u8;
    Color32::from_rgba_premultiplied(ch(b.r()), ch(b.g()), ch(b.b()), ch(b.a()))
}
