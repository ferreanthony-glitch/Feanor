use eframe::egui::{self, Color32, ColorImage, TextureFilter, TextureHandle, TextureOptions};

use crate::camera::Camera;
use crate::document::{CelId, Document, Panel};
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

/// Ce que voit la caméra `cam` : l'image finale (`doc.frame`). Chaque pixel prend la couleur de
/// la feuille au point filmé (interpolation bilinéaire) ; caméra centrée sans zoom ni rotation :
/// les pixels de la feuille, tels quels. Hors de la feuille : transparent.
pub fn camera_image(doc: &Document, plan: &Plan, cam: &Camera) -> Vec<Color32> {
    let full = composite_full(doc, plan);
    let [fw, fh] = doc.frame;
    let (w, h) = (doc.width as i64, doc.height as i64);
    let get = |x: i64, y: i64| -> [f32; 4] {
        if x < 0 || y < 0 || x >= w || y >= h {
            return [0.0; 4];
        }
        let c = full[(y * w + x) as usize];
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let mut out = Vec::with_capacity(fw * fh);
    for v in 0..fh {
        for u in 0..fw {
            let p = cam.to_stage(doc.frame, u as f32 + 0.5, v as f32 + 0.5);
            let (fx, fy) = (p.x - 0.5, p.y - 0.5);
            let (ix, iy) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - ix, fy - iy);
            let (ix, iy) = (ix as i64, iy as i64);
            let (a, b, c, d) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
            let mut px = [0u8; 4];
            for k in 0..4 {
                let top = a[k] + (b[k] - a[k]) * tx;
                let bottom = c[k] + (d[k] - c[k]) * tx;
                px[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
            }
            out.push(Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3]));
        }
    }
    out
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
pub fn panel_plan(panel: &Panel, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    Plan {
        paper,
        items: panel
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

/// Image réduite de ce que voit la caméra `cam` (moyenne de 4 échantillons par pixel).
pub fn thumbnail(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize) -> ColorImage {
    let [fw, fh] = doc.frame;
    let (sx, sy) = (fw as f32 / tw as f32, fh as f32 / th as f32);
    let mut pixels = Vec::with_capacity(tw * th);
    for y in 0..th {
        for x in 0..tw {
            let mut acc = [0u32; 4];
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let p = cam.to_stage(doc.frame, (x as f32 + ox) * sx, (y as f32 + oy) * sy);
                let c = if p.x < 0.0 || p.y < 0.0 || p.x >= doc.width as f32 || p.y >= doc.height as f32 {
                    Color32::TRANSPARENT
                } else {
                    composite_pixel(doc, plan, p.x as usize, p.y as usize)
                };
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
    /// Composition par la carte graphique (`gpu.rs`) : les changements lui sont aussi transmis.
    gpu: bool,
    gpu_all: bool,
    gpu_tiles: Vec<usize>,
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
            gpu: false,
            gpu_all: true,
            gpu_tiles: Vec::new(),
        }
    }

    pub fn set_gpu(&mut self, on: bool) {
        self.gpu = on;
    }

    pub fn invalidate_all(&mut self) {
        self.dirty_all = true;
        self.gpu_all = true;
    }

    pub fn invalidate_tiles(&mut self, tiles: &[usize]) {
        for &t in tiles {
            if let Some(d) = self.dirty.get_mut(t) {
                *d = true;
                self.any_dirty = true;
            }
        }
        if self.gpu {
            self.gpu_tiles.extend_from_slice(tiles);
        }
    }

    /// Changements à envoyer à la carte graphique depuis la dernière fois : tout, ou ces tuiles.
    pub fn take_gpu_dirty(&mut self) -> (bool, Vec<usize>) {
        let mut tiles = std::mem::take(&mut self.gpu_tiles);
        tiles.sort_unstable();
        tiles.dedup();
        (std::mem::take(&mut self.gpu_all), tiles)
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
