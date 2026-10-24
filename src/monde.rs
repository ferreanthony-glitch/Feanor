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
//!
//! Ce qui fait refaire le hors-feuille (audit du 2026-10-23, point 7) : pas le plan entier, qui change à chaque image de
//! la lecture, du défilement, de la pelure d'oignon et de chaque réglage tiré, mais **ce que le plan pose hors de la
//! feuille** (`WorldKey`) : le papier et les seuls éléments qui peuvent y montrer un pixel (`contributes`), avec leur
//! dessin, leur opacité, leur teinte, leur mode de fusion, leur transformation. Un calque qui reste dans la feuille peut
//! changer de dessin, d'opacité ou de place sans que rien ne se refasse ici.
//!
//! Quand ce contenu change (autre plan, réglage d'un calque qui dépasse, tout a changé), le hors-feuille se refait **sur
//! un autre fil**, d'après un instantané du plan (les tuiles des dessins sont partagées, rien n'est copié) ; l'écran
//! garde l'image d'avant en attendant, et n'envoie à la texture que les tuiles qui ont changé. Un réglage tiré en continu
//! se suit au rythme de ces calculs (le suivant part à l'arrivée du précédent). La texture n'est refaite que si sa taille
//! change ; sinon, les tuiles écrites sont effacées une à une.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, ColorImage, TextureFilter, TextureHandle, TextureId, TextureOptions};

use crate::blend::BlendMode;
use crate::document::Document;
use crate::raster::{is_outer_id, outer_key, tiles_across, Raster, TILE, TILE_PIXELS};
use crate::render::{composite_world_tile, plan_bounds, plan_world_tiles, Area, FondPart, Plan, PlanItem};
use crate::vector::Affine;

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

/// Ce que le plan pose hors de la feuille : le papier, et les éléments qui peuvent y montrer un pixel (`contributes`),
/// dans leur ordre. Deux plans de même clé ont le même hors-feuille, pixel pour pixel.
#[derive(Clone, PartialEq)]
pub(crate) struct WorldKey {
    paper: Option<[u8; 3]>,
    items: Vec<PlanItem>,
}

/// La clé du hors-feuille du plan `plan`.
pub(crate) fn world_key(doc: &Document, plan: &Plan) -> WorldKey {
    WorldKey { paper: plan.paper, items: plan.items.iter().filter(|i| contributes(doc, i)).cloned().collect() }
}

/// L'élément peut-il montrer un pixel hors de la feuille (ou dans la part d'une tuile de bord qui la dépasse) ? Non
/// pour une feuille posée par un maillage et un dessin d'une autre taille (`composite_world_tile` ne les pose que sur la
/// feuille, que le hors-feuille efface), un dessin vide, et un dessin dont tout ce qui est dessiné (transformé, avec la
/// marge de son lissage) reste dans les tuiles entières de la feuille. Un élément à effets ou à remplissage au griffonnage,
/// ou qui n'est pas un dessin du projet : oui, toujours (ce qu'il montre peut déborder de son dessin).
pub(crate) fn contributes(doc: &Document, item: &PlanItem) -> bool {
    let pivot = doc.stage_center();
    if item.warp(pivot).is_some() {
        return false;
    }
    if !item.fx.is_empty() || item.ctg.is_some() || matches!(item.fond, Some(FondPart::Reglage(_))) {
        return true;
    }
    let Some(cel) = doc.cels.get(&item.cel) else { return true };
    let mut range = cel.raster.drawn_tile_range();
    // Le fond sous le trait : sa forme suit le trait, ce qu'on y a peint est dans sa strate.
    if item.fond.is_some() {
        if let Some(s) = cel.fond.as_deref() {
            range = match (range, s.peinture.raster.drawn_tile_range()) {
                (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))),
                (a, b) => a.or(b),
            };
        }
    }
    let Some((tx0, ty0, tx1, ty1)) = range else { return false };
    let t = TILE as i64;
    let (w, h) = (doc.width as i64, doc.height as i64);
    let (x0, y0, x1, y1) = (tx0 as i64 * t, ty0 as i64 * t, (tx1 as i64 + 1) * t, (ty1 as i64 + 1) * t);
    match item.affine(pivot) {
        None => {
            if cel.raster.width != doc.width || cel.raster.height != doc.height {
                return false;
            }
            !(x0 >= 0 && y0 >= 0 && x1 <= w && y1 <= h)
        }
        Some(a) => {
            let p = |x: i64, y: i64| a.apply(egui::pos2(x as f32, y as f32));
            let pts = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
            let lo = pts.iter().fold((f32::MAX, f32::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
            let hi = pts.iter().fold((f32::MIN, f32::MIN), |(x, y), p| (x.max(p.x), y.max(p.y)));
            // (Le lissage lit un pixel autour du dessin : un pixel du dessin vaut jusqu'à `etire` pixels du monde.)
            let (o, ex, ey) = (a.apply(egui::pos2(0.0, 0.0)), a.apply(egui::pos2(1.0, 0.0)), a.apply(egui::pos2(0.0, 1.0)));
            let etire = (ex - o).length() + (ey - o).length();
            let m = 2.0 + 2.0 * etire;
            if ![lo.0, lo.1, hi.0, hi.1, m].iter().all(|v| v.is_finite()) {
                return true;
            }
            let r = Area::around((lo.0 - m, lo.1 - m), (hi.0 + m, hi.1 + m)).tile_aligned();
            !(r.x0 >= 0 && r.y0 >= 0 && r.x1 <= (w / t) * t && r.y1 <= (h / t) * t)
        }
    }
}

/// Un élément du plan figé pour le fil qui refait le hors-feuille : le dessin montré (ses tuiles sont partagées) et ce
/// qu'il faut pour le poser.
struct Couche {
    raster: Raster,
    inv: Option<Affine>,
    opacity: u8,
    tint: Option<[u8; 3]>,
    blend: BlendMode,
}

/// Le plan figé : ce qu'il faut au fil pour composer les tuiles du monde comme `composite_world_tile`.
struct Instantane {
    sheet: (usize, usize),
    paper: Color32,
    couches: Vec<Couche>,
}

impl Instantane {
    /// Figé sur le fil de l'interface : les éléments de la clé, chacun avec le dessin qu'il montre maintenant (effets
    /// compris, `fx::shown`), comme `composite_world_tile` les prend ; ceux qu'il ne pose que sur la feuille sont laissés.
    fn of(doc: &Document, key: &WorldKey) -> Instantane {
        let mut couches = Vec::new();
        for item in &key.items {
            let inv = crate::render::inverse_of(doc, item);
            let Some(cel) = crate::fx::shown(doc, item) else { continue };
            if inv.is_none() && (cel.raster.width != doc.width || cel.raster.height != doc.height) {
                continue;
            }
            couches.push(Couche { raster: cel.raster.clone(), inv, opacity: item.opacity, tint: item.tint, blend: item.blend });
        }
        Instantane { sheet: (doc.width, doc.height), paper: crate::render::paper_of(key.paper), couches }
    }

    /// La tuile du monde `key`, entière (le papier, puis les couches).
    fn tile(&self, key: (i32, i32)) -> Vec<Color32> {
        let t = TILE as i64;
        let origin = (key.0 as i64 * t, key.1 as i64 * t);
        let mut scratch = vec![self.paper; TILE_PIXELS];
        for c in &self.couches {
            crate::render::world_item_over(&c.raster, c.inv, c.opacity, c.tint, c.blend, key.0, key.1, &mut scratch, TILE, origin);
        }
        scratch
    }
}

/// La tuile du monde `key` composée (`scratch`, entière) telle que la texture la montre : sa part dans la feuille
/// effacée (la feuille a sa propre image), puis réduite d'un facteur `k`.
fn finish(mut scratch: Vec<Color32>, key: (i32, i32), sheet: (usize, usize), k: usize) -> Vec<Color32> {
    let t = TILE as i64;
    let (ox, oy) = (key.0 as i64 * t, key.1 as i64 * t);
    for y in 0..t {
        let wy = oy + y;
        if wy < 0 || wy >= sheet.1 as i64 {
            continue;
        }
        for x in 0..t {
            let wx = ox + x;
            if wx >= 0 && wx < sheet.0 as i64 {
                scratch[(y * t + x) as usize] = Color32::TRANSPARENT;
            }
        }
    }
    if k == 1 {
        return scratch;
    }
    let tp = TILE / k;
    let mut out = Vec::with_capacity(tp * tp);
    for dy in 0..tp {
        for dx in 0..tp {
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
            out.push(Color32::from_rgba_premultiplied((sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, (sum[3] / n) as u8));
        }
    }
    out
}

/// Un hors-feuille qui se refait sur un autre fil.
struct Job {
    key: WorldKey,
    area: Area,
    scale: usize,
    sheet: (usize, usize),
    tiles: usize,
    rx: Receiver<Vec<((i32, i32), Vec<Color32>)>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// La texture du monde hors de la feuille, et ce qu'il reste à y composer.
#[derive(Default)]
pub struct Monde {
    texture: Option<TextureHandle>,
    /// Taille de la texture (elle n'est refaite que si elle change).
    texture_size: [usize; 2],
    /// La zone du monde que la texture couvre (tuiles entières).
    area: Option<Area>,
    /// Facteur de réduction : un pixel de la texture vaut `scale` × `scale` pixels du monde.
    scale: usize,
    buffer: Vec<Color32>,
    /// Largeur de la texture.
    width: usize,
    /// Ce que montre la texture (la clé du plan qu'elle compose) ; rien : à refaire.
    key: Option<WorldKey>,
    sheet: (usize, usize),
    /// Tuiles à composer (la dernière sort la première) et leur ensemble.
    pending: Vec<(i32, i32)>,
    queued: HashSet<(i32, i32)>,
    /// Les tuiles écrites dans la texture depuis qu'elle a été vidée (à effacer quand le contenu change).
    written: HashSet<(i32, i32)>,
    /// Des tuiles ont changé pendant que la vue ne regardait pas hors de la feuille : tout est à refaire.
    stale: bool,
    /// Le hors-feuille qui se refait sur un autre fil, ce qui a changé depuis qu'il est parti (tout, ou ces tuiles).
    job: Option<Job>,
    job_outdated: bool,
    since_job: Vec<usize>,
    /// Tuiles composées depuis le début (mesures).
    composed: u64,
    /// Hors-feuilles refaits sur un autre fil depuis le début (mesures, essais).
    rebuilds: u64,
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

/// Les coordonnées de tuile d'un numéro de tuile d'un dessin d'une feuille de `width` pixels de large (fenêtre de la
/// feuille, ou hors-feuille).
fn tile_of(width: usize, id: usize) -> (i32, i32) {
    if is_outer_id(id) {
        outer_key(id)
    } else {
        let across = tiles_across(width).max(1);
        ((id % across) as i32, (id / across) as i32)
    }
}

/// Une image transparente de `n` pixels, sans la remplir pixel par pixel (de la mémoire mise à zéro).
fn transparent(n: usize) -> Vec<Color32> {
    bytemuck::zeroed_vec(n)
}

impl Monde {
    pub fn new() -> Self {
        Self { scale: 1, ..Default::default() }
    }

    /// Lâche la texture : le plan n'a plus rien hors de la feuille.
    fn release(&mut self) {
        *self = Self { composed: self.composed, rebuilds: self.rebuilds, scale: 1, ..Default::default() };
    }

    /// Octets que la texture et sa copie en mémoire prennent.
    pub fn bytes(&self) -> u64 {
        self.buffer.len() as u64 * 4 * if self.texture.is_some() { 2 } else { 1 }
    }

    /// Tuiles qui attendent d'être composées (sur ce fil, ou sur celui qui refait le hors-feuille).
    pub fn pending(&self) -> usize {
        self.pending.len() + self.job.as_ref().map_or(0, |j| j.tiles.max(1))
    }

    /// Le hors-feuille se refait sur un autre fil (l'écran montre celui d'avant).
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Tuiles composées depuis le début.
    pub fn composed(&self) -> u64 {
        self.composed
    }

    /// Hors-feuilles refaits sur un autre fil depuis le début.
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
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
        // Un hors-feuille refait sur l'autre fil est arrivé : il prend la place de l'ancien.
        self.poll();
        let stale = std::mem::take(&mut self.stale);
        let want = want.tile_aligned();
        let page = |lo: i64, hi: i64| (lo.div_euclid(PAGE) * PAGE, (hi + PAGE - 1).div_euclid(PAGE) * PAGE);
        let (px0, px1) = page(want.x0, want.x1);
        let (py0, py1) = page(want.y0, want.y1);
        let fresh = intersect(Area { x0: px0, y0: py0, x1: px1, y1: py1 }, bounds.tile_aligned());
        let fresh_scale = scale_for(fresh);
        let key = world_key(doc, plan);
        // La texture couvre-t-elle encore la vue, assez finement ? Sinon : autre zone, tout est à composer (ici, par
        // petits morceaux, comme toujours).
        let geometry_ok = self.texture.is_some() && self.sheet == (doc.width, doc.height) && self.area.is_some_and(|a| inside(a, want)) && fresh_scale >= self.scale;
        if !geometry_ok {
            self.reset_geometry(ctx, fresh, fresh_scale, (doc.width, doc.height));
            self.key = Some(key);
            // Les plus près du centre de la vue d'abord (la dernière sort la première).
            let (cx, cy) = ((view.x0 + view.x1) / 2, (view.y0 + view.y1) / 2);
            let mut tiles = plan_world_tiles(doc, plan, fresh);
            tiles.sort_by_key(|&(tx, ty)| std::cmp::Reverse((tx as i64 * TILE as i64 - cx).abs() + (ty as i64 * TILE as i64 - cy).abs()));
            self.queued = tiles.iter().copied().collect();
            self.pending = tiles;
        } else if let Some(job) = &self.job {
            // Un hors-feuille se refait : ce qui change d'ici son arrivée sera refait après lui.
            let same = job.key == key;
            if all || stale || !same {
                self.job_outdated = true;
            }
            self.since_job.extend_from_slice(ids);
        } else if all || stale || self.job_outdated || self.key.as_ref() != Some(&key) {
            // Le contenu a changé (autre plan, réglage d'un calque qui dépasse, tout) : refait sur un autre fil, l'image
            // d'avant reste à l'écran en attendant.
            self.start_job(ctx, doc, plan, key);
        } else if !ids.is_empty() {
            self.queue_ids(ids);
        }
        self.work(ctx, doc, plan);
        let texture = self.texture.as_ref()?;
        Some(Shown { id: texture.id(), area: self.area? })
    }

    /// Les tuiles touchées `ids` qui sont hors de la feuille et dans la zone : à composer d'abord.
    fn queue_ids(&mut self, ids: &[usize]) {
        let Some(area) = self.area else { return };
        let t = TILE as i64;
        let (w, h) = self.sheet;
        for &id in ids {
            let (tx, ty) = tile_of(w, id);
            let (x, y) = (tx as i64 * t, ty as i64 * t);
            let on_sheet = x >= 0 && y >= 0 && x + t <= w as i64 && y + t <= h as i64;
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

    /// Une autre zone (ou une autre finesse) : la texture n'est refaite que si sa taille change ; sinon les tuiles déjà
    /// écrites y sont effacées.
    fn reset_geometry(&mut self, ctx: &egui::Context, area: Area, k: usize, sheet: (usize, usize)) {
        self.job = None;
        self.job_outdated = false;
        self.since_job.clear();
        self.pending.clear();
        self.queued.clear();
        let (bw, bh) = (area.width() / k, area.height() / k);
        if self.texture.is_some() && self.texture_size == [bw, bh] && self.buffer.len() == bw * bh {
            let old: Vec<(i32, i32)> = self.written.drain().collect();
            let tp = TILE / self.scale;
            let blank = vec![Color32::TRANSPARENT; tp * tp];
            for &key in &old {
                self.put(key, &blank);
            }
            self.upload(old);
        } else {
            self.buffer = transparent(bw * bh);
            let image = ColorImage { size: [bw, bh], pixels: transparent(bw * bh) };
            match &mut self.texture {
                Some(t) => t.set(image, options()),
                None => self.texture = Some(ctx.load_texture("monde", image, options())),
            }
            self.texture_size = [bw, bh];
            self.written.clear();
        }
        self.area = Some(area);
        self.scale = k;
        self.width = bw;
        self.sheet = sheet;
    }

    /// Le hors-feuille de `key` part sur un autre fil (toutes les tuiles du plan dans la zone de la texture).
    fn start_job(&mut self, ctx: &egui::Context, doc: &Document, plan: &Plan, key: WorldKey) {
        let Some(area) = self.area else { return };
        let tiles = plan_world_tiles(doc, plan, area);
        let snap = Instantane::of(doc, &key);
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let (k, n) = (self.scale, tiles.len());
        let stop = cancel.clone();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("hors-feuille".into()).spawn(move || {
            let mut out: Vec<((i32, i32), Vec<Color32>)> = tiles.into_iter().map(|t| (t, Vec::new())).collect();
            // (La moitié des cœurs : l'interface et le dessin gardent les leurs.)
            crate::render::set_thread_cap((crate::parallele::coeurs() / 2).max(1));
            crate::render::par_chunks(&mut out, 4, |_, part| {
                for (t, px) in part.iter_mut() {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    *px = finish(snap.tile(*t), *t, snap.sheet, k);
                }
            });
            if !stop.load(Ordering::Relaxed) && tx.send(out).is_ok() {
                ctx.request_repaint();
            }
        });
        if spawned.is_err() {
            return;
        }
        self.job = Some(Job { key, area, scale: k, sheet: self.sheet, tiles: n, rx, cancel });
        self.job_outdated = false;
        self.since_job.clear();
        self.pending.clear();
        self.queued.clear();
        self.rebuilds += 1;
    }

    /// Le hors-feuille refait sur l'autre fil est-il arrivé ? Il remplace l'ancien : seules les tuiles qui changent vont
    /// à la texture ; celles d'avant qui n'existent plus sont effacées. Ce qui a été touché depuis son départ est recomposé.
    fn poll(&mut self) {
        let Some(job) = &self.job else { return };
        let done = match job.rx.try_recv() {
            Ok(done) => done,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                // (Le fil est mort : tout se refera.)
                self.job = None;
                self.job_outdated = true;
                return;
            }
        };
        let job = self.job.take().expect("hors-feuille en route");
        if Some(job.area) != self.area || job.scale != self.scale || job.sheet != self.sheet {
            self.job_outdated = true;
            return;
        }
        let new: HashMap<(i32, i32), Vec<Color32>> = done.into_iter().collect();
        let tp = TILE / self.scale;
        let blank = vec![Color32::TRANSPARENT; tp * tp];
        let mut touched = Vec::new();
        let gone: Vec<(i32, i32)> = self.written.iter().copied().filter(|k| !new.contains_key(k)).collect();
        for key in gone {
            self.put(key, &blank);
            self.written.remove(&key);
            touched.push(key);
        }
        for (key, px) in &new {
            if px.len() != tp * tp {
                continue;
            }
            self.composed += 1;
            if !self.same(*key, px) {
                self.put(*key, px);
                touched.push(*key);
            }
            self.written.insert(*key);
        }
        self.upload(touched);
        self.key = Some(job.key.clone());
        // Les tuiles touchées depuis son départ (un trait) : recomposées ici, comme d'ordinaire.
        let ids = std::mem::take(&mut self.since_job);
        self.queue_ids(&ids);
    }

    /// La tuile `key` de la copie de la texture a-t-elle déjà ces pixels ?
    fn same(&self, key: (i32, i32), px: &[Color32]) -> bool {
        let Some(area) = self.area else { return false };
        let (k, tp) = (self.scale, TILE / self.scale);
        let t = TILE as i64;
        let (bx, by) = (((key.0 as i64 * t - area.x0) / k as i64) as usize, ((key.1 as i64 * t - area.y0) / k as i64) as usize);
        (0..tp).all(|dy| self.buffer[(by + dy) * self.width + bx..(by + dy) * self.width + bx + tp] == px[dy * tp..(dy + 1) * tp])
    }

    /// Écrit la tuile `key` (déjà réduite) dans la copie de la texture.
    fn put(&mut self, key: (i32, i32), px: &[Color32]) {
        let Some(area) = self.area else { return };
        let (k, tp) = (self.scale, TILE / self.scale);
        let t = TILE as i64;
        let (bx, by) = (((key.0 as i64 * t - area.x0) / k as i64) as usize, ((key.1 as i64 * t - area.y0) / k as i64) as usize);
        for dy in 0..tp {
            let s = (by + dy) * self.width + bx;
            self.buffer[s..s + tp].copy_from_slice(&px[dy * tp..(dy + 1) * tp]);
        }
    }

    /// Envoie à la texture ces tuiles de sa copie (une bande par rangée de tuiles voisines).
    fn upload(&mut self, mut touched: Vec<(i32, i32)>) {
        let (Some(area), Some(texture)) = (self.area, self.texture.as_mut()) else { return };
        touched.sort_by_key(|&(tx, ty)| (ty, tx));
        touched.dedup();
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

    /// Compose des tuiles en attente, tant que le budget de l'image le permet, et envoie à la texture
    /// ce qui a changé.
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
        self.upload(touched);
    }

    /// Compose la tuile du monde `key` dans la copie de la texture : le plan complet sur cette tuile,
    /// puis sa part qui est dans la feuille effacée (la feuille a sa propre image), puis réduite.
    fn compose(&mut self, doc: &Document, plan: &Plan, key: (i32, i32)) {
        if self.area.is_none() {
            return;
        }
        let t = TILE as i64;
        let (ox, oy) = (key.0 as i64 * t, key.1 as i64 * t);
        let mut scratch = vec![Color32::TRANSPARENT; TILE_PIXELS];
        composite_world_tile(doc, plan, key.0, key.1, &mut scratch, TILE, (ox, oy));
        self.composed += 1;
        let px = finish(scratch, key, (doc.width, doc.height), self.scale);
        self.put(key, &px);
        self.written.insert(key);
    }
}
