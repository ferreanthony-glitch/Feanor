//! Mesures seulement (`perf_tests::perf_monde`) : le hors-feuille de l'écran tel qu'il était avant la branche
//! `optim-vram-monde` (eb82649), pour comparer avant et après dans le même processus. Tout refait, sur le fil de
//! l'interface, à chaque changement du plan. Le texte d'origine suit.
//!
//! La toile infinie à l'écran : ce que le plan montre hors de la feuille.
//!
//! La feuille se compose comme toujours (carte graphique ou processeur, `gpu.rs`, `render.rs`) ; ce
//! module compose, par le processeur, le reste du monde visible : les tuiles dessinées hors de la
//! feuille, et la part des tuiles de bord qui dépasse. Il en fait une texture posée sous la feuille
//! (la feuille y est transparente), si bien que les deux se complètent pixel pour pixel, comme les
//! tuiles d'une même image composée.
//!
//! Règles :
//! - rien n'est composé tant que le plan n'a rien hors de la feuille (`plan_bounds`), ni là où la vue
//!   ne regarde pas ;
//! - seules les tuiles où un calque a dessiné sont composées (le papier est peint par l'écran, il ne
//!   coûte rien) ;
//! - la texture a une taille bornée (`MAX_SIDE` pixels de côté) : au-delà, elle est réduite d'un
//!   facteur 2, 4, 8… (moyenne des pixels) ;
//! - le travail est borné par image (`BUDGET`) : une vue très chargée se remplit en quelques images
//!   au lieu de figer l'écran, et les tuiles qu'on vient de toucher passent d'abord (un trait loin de
//!   la feuille apparaît tout de suite) ;
//! - la zone de la texture est alignée sur des pages de `PAGE` pixels : déplacer la vue ne refait
//!   rien tant qu'elle reste dans la zone.
#![allow(dead_code)]

use std::collections::HashSet;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, ColorImage, TextureFilter, TextureHandle, TextureId, TextureOptions};

use crate::document::Document;
use crate::raster::{is_outer_id, outer_key, tiles_across, TILE, TILE_PIXELS};
use crate::render::{composite_world_tile, plan_bounds, plan_world_tiles, Area, Plan};

/// Plus grand côté de la texture, en pixels d'écran (avant réduction).
pub const MAX_SIDE: i64 = 4096;
/// Les bords de la zone composée tombent sur des multiples de cette taille (pixels du monde).
const PAGE: i64 = 1024;
/// Temps de composition par image d'écran avant de reprendre à l'image suivante.
const BUDGET: Duration = Duration::from_millis(5);

/// Ce qu'il faut peindre : la texture et la zone du monde qu'elle couvre.
#[derive(Clone, Copy, Debug)]
pub struct Shown {
    pub id: TextureId,
    pub area: Area,
}

/// La texture du monde hors de la feuille, et ce qu'il reste à y composer.
#[derive(Default)]
pub struct Monde {
    texture: Option<TextureHandle>,
    /// La zone du monde que la texture couvre (tuiles entières).
    area: Option<Area>,
    /// Facteur de réduction : un pixel de la texture vaut `scale` × `scale` pixels du monde.
    scale: usize,
    buffer: Vec<Color32>,
    /// Largeur de la texture.
    width: usize,
    /// Le plan montré (le changer refait tout).
    plan: Option<Plan>,
    sheet: (usize, usize),
    /// Tuiles à composer (la dernière sort la première) et leur ensemble.
    pending: Vec<(i32, i32)>,
    queued: HashSet<(i32, i32)>,
    /// Des tuiles ont changé pendant que la vue ne regardait pas hors de la feuille : tout est à refaire.
    stale: bool,
    /// Tuiles composées depuis le début (mesures).
    composed: u64,
}

fn options() -> TextureOptions {
    TextureOptions { magnification: TextureFilter::Nearest, minification: TextureFilter::Linear, ..TextureOptions::LINEAR }
}

/// Le plus grand rectangle commun à `a` et `b` (vide si elles ne se touchent pas).
fn intersect(a: Area, b: Area) -> Area {
    Area { x0: a.x0.max(b.x0), y0: a.y0.max(b.y0), x1: a.x1.min(b.x1), y1: a.y1.min(b.y1) }
}

fn inside(outer: Area, inner: Area) -> bool {
    inner.x0 >= outer.x0 && inner.y0 >= outer.y0 && inner.x1 <= outer.x1 && inner.y1 <= outer.y1
}

/// Facteur de réduction pour que `area` tienne dans la texture (puissance de deux).
fn scale_for(area: Area) -> usize {
    let side = area.width().max(area.height()) as i64;
    let mut k = 1usize;
    while side / k as i64 > MAX_SIDE {
        k *= 2;
    }
    k
}

/// Les coordonnées de tuile d'un numéro de tuile d'un dessin (fenêtre de la feuille, ou hors-feuille).
fn tile_of(doc: &Document, id: usize) -> (i32, i32) {
    if is_outer_id(id) {
        outer_key(id)
    } else {
        let across = tiles_across(doc.width).max(1);
        ((id % across) as i32, (id / across) as i32)
    }
}

impl Monde {
    pub fn new() -> Self {
        Self { scale: 1, ..Default::default() }
    }

    /// Lâche la texture : le plan n'a plus rien hors de la feuille.
    fn release(&mut self) {
        *self = Self { composed: self.composed, scale: 1, ..Default::default() };
    }

    /// Octets que la texture et sa copie en mémoire prennent.
    pub fn bytes(&self) -> u64 {
        self.buffer.len() as u64 * 4 * if self.texture.is_some() { 2 } else { 1 }
    }

    /// Tuiles qui attendent d'être composées.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Tuiles composées depuis le début.
    pub fn composed(&self) -> u64 {
        self.composed
    }

    /// La zone du monde de la texture et son facteur de réduction.
    pub fn coverage(&self) -> Option<(Area, usize)> {
        self.area.map(|a| (a, self.scale))
    }

    /// Le pixel du monde (`x`, `y`) tel que la texture le montre (transparent dans la feuille et là
    /// où rien n'est encore composé), si la texture le couvre.
    pub fn pixel(&self, x: i64, y: i64) -> Option<Color32> {
        let area = self.area?;
        if !area.contains(x, y) {
            return None;
        }
        let k = self.scale as i64;
        self.buffer.get((((y - area.y0) / k) as usize) * self.width + ((x - area.x0) / k) as usize).copied()
    }

    /// Met la texture à jour pour la zone `view` du monde et la renvoie, ou rien s'il n'y a rien à
    /// montrer hors de la feuille. `all` : tout a pu changer ; `ids` : numéros des tuiles touchées
    /// depuis l'image précédente (`Renderer::take_world_dirty`).
    pub fn update(&mut self, ctx: &egui::Context, doc: &Document, plan: &Plan, view: Area, all: bool, ids: &[usize]) -> Option<Shown> {
        // (Un repère de calque d'ajustement n'a rien à faire hors de la feuille : ses effets ne passent que
        // sur la feuille.)
        let is_marker = |i: &crate::render::PlanItem| matches!(i.fond, Some(crate::render::FondPart::Reglage(_)));
        let without;
        let plan: &Plan = if plan.items.iter().any(is_marker) {
            without = Plan { items: plan.items.iter().filter(|i| !is_marker(i)).cloned().collect(), ..plan.clone() };
            &without
        } else {
            plan
        };
        let sheet = Area::sheet(doc);
        let bounds = plan_bounds(doc, plan);
        if bounds == sheet {
            self.release();
            return None;
        }
        let want = intersect(view, bounds);
        let beyond = want.width() > 0 && want.height() > 0 && !inside(sheet, want);
        if !beyond {
            // (Ce qui change pendant qu'on ne regarde pas hors de la feuille se refait au retour.)
            self.stale |= all || !ids.is_empty();
            return None;
        }
        let stale = std::mem::take(&mut self.stale);
        let want = want.tile_aligned();
        // Il faut tout refaire : autre plan ou autre feuille, tout a changé, ou la vue sort de la zone
        // (ou demande une texture plus fine).
        let page = |lo: i64, hi: i64| (lo.div_euclid(PAGE) * PAGE, (hi + PAGE - 1).div_euclid(PAGE) * PAGE);
        let (px0, px1) = page(want.x0, want.x1);
        let (py0, py1) = page(want.y0, want.y1);
        let fresh = intersect(Area { x0: px0, y0: py0, x1: px1, y1: py1 }, bounds.tile_aligned());
        let fresh_scale = scale_for(fresh);
        let keep = self.texture.is_some()
            && self.sheet == (doc.width, doc.height)
            && self.plan.as_ref() == Some(plan)
            && !all
            && !stale
            && self.area.is_some_and(|a| inside(a, want))
            && fresh_scale >= self.scale;
        if !keep {
            let (area, k) = (fresh, fresh_scale);
            let (bw, bh) = (area.width() / k, area.height() / k);
            if self.area == Some(area) && self.scale == k && self.buffer.len() == bw * bh {
                self.buffer.fill(Color32::TRANSPARENT);
            } else {
                self.buffer = vec![Color32::TRANSPARENT; bw * bh];
            }
            self.area = Some(area);
            self.scale = k;
            self.width = bw;
            self.plan = Some(plan.clone());
            self.sheet = (doc.width, doc.height);
            let image = ColorImage { size: [bw, bh], pixels: self.buffer.clone() };
            match &mut self.texture {
                Some(t) => t.set(image, options()),
                None => self.texture = Some(ctx.load_texture("monde", image, options())),
            }
            // Les plus près du centre de la vue d'abord (la dernière sort la première).
            let (cx, cy) = ((view.x0 + view.x1) / 2, (view.y0 + view.y1) / 2);
            let mut tiles = plan_world_tiles(doc, plan, area);
            tiles.sort_by_key(|&(tx, ty)| std::cmp::Reverse((tx as i64 * TILE as i64 - cx).abs() + (ty as i64 * TILE as i64 - cy).abs()));
            self.queued = tiles.iter().copied().collect();
            self.pending = tiles;
        } else if !ids.is_empty() {
            let area = self.area.expect("zone de la texture");
            let t = TILE as i64;
            for &id in ids {
                let (tx, ty) = tile_of(doc, id);
                let (x, y) = (tx as i64 * t, ty as i64 * t);
                let on_sheet = x >= 0 && y >= 0 && x + t <= doc.width as i64 && y + t <= doc.height as i64;
                if on_sheet || x >= area.x1 || x + t <= area.x0 || y >= area.y1 || y + t <= area.y0 {
                    continue;
                }
                let key = (tx, ty);
                // (Une tuile qu'on vient de toucher passe avant le reste.)
                if !self.queued.insert(key) {
                    if let Some(i) = self.pending.iter().position(|&k| k == key) {
                        self.pending.swap_remove(i);
                    }
                }
                self.pending.push(key);
            }
        }
        self.work(ctx, doc, plan);
        let texture = self.texture.as_ref()?;
        Some(Shown { id: texture.id(), area: self.area? })
    }

    /// Compose des tuiles en attente, tant que le budget de l'image le permet, et envoie à la texture
    /// ce qui a changé (une bande par rangée de tuiles voisines).
    fn work(&mut self, ctx: &egui::Context, doc: &Document, plan: &Plan) {
        let started = Instant::now();
        let mut touched: Vec<(i32, i32)> = Vec::new();
        while let Some(key) = self.pending.pop() {
            self.queued.remove(&key);
            self.compose(doc, plan, key);
            touched.push(key);
            if touched.len() % 8 == 0 && started.elapsed() > BUDGET {
                break;
            }
        }
        if !self.pending.is_empty() {
            ctx.request_repaint();
        }
        let (Some(area), Some(texture)) = (self.area, self.texture.as_mut()) else { return };
        touched.sort_by_key(|&(tx, ty)| (ty, tx));
        let (k, tp) = (self.scale, TILE / self.scale);
        let mut i = 0;
        while i < touched.len() {
            let (tx0, ty) = touched[i];
            let mut n = 1;
            while i + n < touched.len() && touched[i + n] == (tx0 + n as i32, ty) {
                n += 1;
            }
            let (x0, y0) = (((tx0 as i64 * TILE as i64 - area.x0) / k as i64) as usize, ((ty as i64 * TILE as i64 - area.y0) / k as i64) as usize);
            let mut pixels = Vec::with_capacity(n * tp * tp);
            for y in y0..y0 + tp {
                pixels.extend_from_slice(&self.buffer[y * self.width + x0..y * self.width + x0 + n * tp]);
            }
            texture.set_partial([x0, y0], ColorImage { size: [n * tp, tp], pixels }, options());
            i += n;
        }
    }

    /// Compose la tuile du monde `key` dans la copie de la texture : le plan complet sur cette tuile,
    /// puis sa part qui est dans la feuille effacée (la feuille a sa propre image), puis réduite.
    fn compose(&mut self, doc: &Document, plan: &Plan, key: (i32, i32)) {
        let Some(area) = self.area else { return };
        let t = TILE as i64;
        let (ox, oy) = (key.0 as i64 * t, key.1 as i64 * t);
        let mut scratch = vec![Color32::TRANSPARENT; TILE_PIXELS];
        composite_world_tile(doc, plan, key.0, key.1, &mut scratch, TILE, (ox, oy));
        for y in 0..t {
            let wy = oy + y;
            if wy < 0 || wy >= doc.height as i64 {
                continue;
            }
            for x in 0..t {
                let wx = ox + x;
                if wx >= 0 && wx < doc.width as i64 {
                    scratch[(y * t + x) as usize] = Color32::TRANSPARENT;
                }
            }
        }
        self.composed += 1;
        let (k, tp) = (self.scale, TILE / self.scale);
        let (bx, by) = (((ox - area.x0) / k as i64) as usize, ((oy - area.y0) / k as i64) as usize);
        for dy in 0..tp {
            for dx in 0..tp {
                let out = &mut self.buffer[(by + dy) * self.width + bx + dx];
                if k == 1 {
                    *out = scratch[dy * TILE + dx];
                    continue;
                }
                let mut sum = [0u32; 4];
                for sy in 0..k {
                    for sx in 0..k {
                        let c = scratch[(dy * k + sy) * TILE + dx * k + sx];
                        for (s, v) in sum.iter_mut().zip(c.to_array()) {
                            *s += v as u32;
                        }
                    }
                }
                let n = (k * k) as u32;
                *out = Color32::from_rgba_premultiplied((sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, (sum[3] / n) as u8);
            }
        }
    }
}
