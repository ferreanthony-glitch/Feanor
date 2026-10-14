use eframe::egui::{self, Color32, ColorImage, TextureFilter, TextureHandle, TextureOptions};

use crate::document::{CelId, Document, Shot};
use crate::raster::{tile_bounds, tiles_across, TILE, TILE_PIXELS};

/// Un dessin à superposer, avec son opacité et une teinte éventuelle (pelure d'oignon).
#[derive(Clone, PartialEq)]
pub struct PlanItem {
    pub cel: CelId,
    pub opacity: u8,
    pub tint: Option<[u8; 3]>,
}

/// Ce qu'il faut composer pour afficher une image, du fond vers le dessus.
/// Sert aussi de clé : tant que le plan ne change pas, l'image affichée reste valable.
#[derive(Clone, PartialEq)]
pub struct Plan {
    pub paper: Option<[u8; 3]>,
    pub items: Vec<PlanItem>,
}

fn mul(a: u32, b: u32) -> u32 {
    (a * b + 127) / 255
}

/// `src` par-dessus `dst` (couleurs prémultipliées).
pub fn over(dst: Color32, src: Color32, opacity: u8, tint: Option<[u8; 3]>) -> Color32 {
    let op = opacity as u32;
    let sa = mul(src.a() as u32, op);
    if sa == 0 {
        return dst;
    }
    let (sr, sg, sb) = match tint {
        None => (
            mul(src.r() as u32, op),
            mul(src.g() as u32, op),
            mul(src.b() as u32, op),
        ),
        Some(t) => (mul(t[0] as u32, sa), mul(t[1] as u32, sa), mul(t[2] as u32, sa)),
    };
    let inv = 255 - sa;
    Color32::from_rgba_premultiplied(
        (sr + mul(dst.r() as u32, inv)).min(255) as u8,
        (sg + mul(dst.g() as u32, inv)).min(255) as u8,
        (sb + mul(dst.b() as u32, inv)).min(255) as u8,
        (sa + mul(dst.a() as u32, inv)).min(255) as u8,
    )
}

fn paper_color(plan: &Plan) -> Color32 {
    match plan.paper {
        Some(p) => Color32::from_rgb(p[0], p[1], p[2]),
        None => Color32::TRANSPARENT,
    }
}

/// Compose la tuile `idx` dans `buf`, image pleine de la taille du document.
pub fn composite_tile(doc: &Document, plan: &Plan, idx: usize, buf: &mut [Color32]) {
    let w = doc.width;
    let (ox, oy, tw, th) = tile_bounds(doc.width, doc.height, idx);
    let base = paper_color(plan);
    for y in 0..th {
        let s = (oy + y) * w + ox;
        buf[s..s + tw].fill(base);
    }
    for item in &plan.items {
        let Some(tile) = doc.cels.get(&item.cel).and_then(|c| c.raster.tiles[idx].as_ref()) else {
            continue;
        };
        let tile: &[Color32; TILE_PIXELS] = tile;
        for y in 0..th {
            let s = (oy + y) * w + ox;
            let row = &tile[y * TILE..y * TILE + tw];
            for (d, &src) in buf[s..s + tw].iter_mut().zip(row) {
                if src.a() != 0 {
                    *d = over(*d, src, item.opacity, item.tint);
                }
            }
        }
    }
}

fn tile_count(doc: &Document) -> usize {
    tiles_across(doc.width) * doc.height.div_ceil(TILE)
}

pub fn composite_full(doc: &Document, plan: &Plan) -> Vec<Color32> {
    let mut buf = vec![Color32::TRANSPARENT; doc.width * doc.height];
    for idx in 0..tile_count(doc) {
        composite_tile(doc, plan, idx, &mut buf);
    }
    buf
}

pub fn composite_pixel(doc: &Document, plan: &Plan, x: usize, y: usize) -> Color32 {
    let mut c = paper_color(plan);
    for item in &plan.items {
        if let Some(cel) = doc.cels.get(&item.cel) {
            let s = cel.raster.get(x, y);
            if s.a() != 0 {
                c = over(c, s, item.opacity, item.tint);
            }
        }
    }
    c
}

/// Plan de composition d'une image d'un plan, sans pelure d'oignon (vignettes, export).
pub fn shot_plan(shot: &Shot, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    Plan {
        paper,
        items: shot
            .layers
            .iter()
            .filter(|l| l.visible)
            .filter_map(|l| {
                l.cel_at(frame).map(|cel| PlanItem {
                    cel,
                    opacity: (l.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
                    tint: None,
                })
            })
            .collect(),
    }
}

/// Image réduite (moyenne de 4 échantillons par pixel).
pub fn thumbnail(doc: &Document, plan: &Plan, tw: usize, th: usize) -> ColorImage {
    let (sx, sy) = (doc.width as f32 / tw as f32, doc.height as f32 / th as f32);
    let mut pixels = Vec::with_capacity(tw * th);
    for y in 0..th {
        for x in 0..tw {
            let mut acc = [0u32; 4];
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let px = (((x as f32 + ox) * sx) as usize).min(doc.width - 1);
                let py = (((y as f32 + oy) * sy) as usize).min(doc.height - 1);
                let c = composite_pixel(doc, plan, px, py);
                for (a, v) in acc.iter_mut().zip(c.to_array()) {
                    *a += v as u32;
                }
            }
            pixels.push(Color32::from_rgba_premultiplied(
                (acc[0] / 4) as u8,
                (acc[1] / 4) as u8,
                (acc[2] / 4) as u8,
                (acc[3] / 4) as u8,
            ));
        }
    }
    ColorImage {
        size: [tw, th],
        pixels,
    }
}

fn texture_options() -> TextureOptions {
    TextureOptions {
        magnification: TextureFilter::Nearest,
        minification: TextureFilter::Linear,
        ..TextureOptions::LINEAR
    }
}

/// Garde l'image composée à l'écran dans une texture et ne recompose que ce qui a changé.
pub struct Renderer {
    width: usize,
    height: usize,
    buffer: Vec<Color32>,
    texture: Option<TextureHandle>,
    plan: Option<Plan>,
    dirty_all: bool,
    dirty: Vec<bool>,
    any_dirty: bool,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            buffer: Vec::new(),
            texture: None,
            plan: None,
            dirty_all: true,
            dirty: Vec::new(),
            any_dirty: false,
        }
    }

    pub fn invalidate_all(&mut self) {
        self.dirty_all = true;
    }

    pub fn invalidate_tiles(&mut self, tiles: &[usize]) {
        for &t in tiles {
            if let Some(d) = self.dirty.get_mut(t) {
                *d = true;
                self.any_dirty = true;
            }
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, doc: &Document, plan: Plan) -> egui::TextureId {
        if self.width != doc.width || self.height != doc.height {
            self.width = doc.width;
            self.height = doc.height;
            self.buffer = vec![Color32::TRANSPARENT; doc.width * doc.height];
            self.dirty = vec![false; tile_count(doc)];
            self.texture = None;
            self.dirty_all = true;
        }
        if self.plan.as_ref() != Some(&plan) {
            self.plan = Some(plan);
            self.dirty_all = true;
        }
        let plan = self.plan.as_ref().expect("plan défini juste au-dessus");
        let options = texture_options();

        if self.dirty_all || self.texture.is_none() {
            for idx in 0..self.dirty.len() {
                composite_tile(doc, plan, idx, &mut self.buffer);
            }
            let image = ColorImage {
                size: [self.width, self.height],
                pixels: self.buffer.clone(),
            };
            match &mut self.texture {
                Some(t) => t.set(image, options),
                None => self.texture = Some(ctx.load_texture("canevas", image, options)),
            }
        } else if self.any_dirty {
            let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
            for idx in 0..self.dirty.len() {
                if !self.dirty[idx] {
                    continue;
                }
                composite_tile(doc, plan, idx, &mut self.buffer);
                let (ox, oy, tw, th) = tile_bounds(self.width, self.height, idx);
                x0 = x0.min(ox);
                y0 = y0.min(oy);
                x1 = x1.max(ox + tw);
                y1 = y1.max(oy + th);
            }
            let mut pixels = Vec::with_capacity((x1 - x0) * (y1 - y0));
            for y in y0..y1 {
                pixels.extend_from_slice(&self.buffer[y * self.width + x0..y * self.width + x1]);
            }
            if let Some(t) = &mut self.texture {
                t.set_partial(
                    [x0, y0],
                    ColorImage {
                        size: [x1 - x0, y1 - y0],
                        pixels,
                    },
                    options,
                );
            }
        }
        self.dirty.fill(false);
        self.any_dirty = false;
        self.dirty_all = false;
        self.texture.as_ref().expect("texture créée").id()
    }
}
