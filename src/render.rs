use std::borrow::Cow;

use eframe::egui::{self, Color32, ColorImage, TextureFilter, TextureHandle, TextureOptions};

use crate::blend::{blend_over, BlendMode};
use crate::camera::Camera;
use crate::document::{CameraSeen, CelId, Document, LayerTransform, Panel};
use crate::raster::{tile_bounds, tiles_across, Raster, TILE, TILE_PIXELS, WORLD};
use crate::vector::Affine;

/// Un dessin à superposer, avec son opacité, une teinte éventuelle (pelure d'oignon) et la
/// transformation de son calque (aucune : le dessin tel quel sur la feuille).
#[derive(Clone, PartialEq)]
pub struct PlanItem {
    pub cel: CelId,
    pub opacity: u8,
    pub tint: Option<[u8; 3]>,
    pub transform: Option<LayerTransform>,
    /// Mode de fusion de son calque (8).
    pub blend: BlendMode,
    /// Effets de la pile du calque à cette image (13i) ; vide : le dessin tel quel.
    pub fx: Vec<crate::fx::FxStep>,
    /// Calque CTG (13j) : les dessins de ses sources à cette image (le dessin est rempli d'après
    /// leurs traits) ; rien pour un calque ordinaire.
    pub ctg: Option<crate::ctg::CtgInput>,
    /// Fond sous le trait (`fond.rs`) : cet élément montre le fond du dessin `cel`, pas son trait.
    pub fond: Option<FondPart>,
    /// Feuille flottante (`flottante.rs`) : transformation du geste en cours (déplacer, tourner,
    /// agrandir une sélection vectorielle), appliquée au dessin avant celle du calque. Le GPU
    /// n'a que ce paramètre à changer à chaque image ; aucun pixel n'est refait ni envoyé.
    pub pre: Option<Pre>,
}

/// La transformation d'un geste sur une feuille flottante (`PlanItem::pre`).
#[derive(Clone, PartialEq, Debug)]
pub struct Pre {
    /// Du dessin (ses pixels) vers la feuille, avant la transformation du calque.
    pub aff: Affine,
    /// Taille de l'image du dessin quand ce n'est pas celle de la feuille : la feuille flottante
    /// d'une sélection garde ce qui dépasse de la feuille (`flottante.rs`), elle est plus grande
    /// (`aff` compte alors sa marge). Rien : la taille de la feuille.
    pub dims: Option<[usize; 2]>,
    /// Enveloppe ou perspective (`warp.rs`) : la feuille est posée par ce maillage, pas par `aff`
    /// (le maillage compte la marge de la feuille).
    pub warp: Option<std::sync::Arc<crate::warp::Warp>>,
}

impl PlanItem {
    /// La transformation du dessin vers la feuille (celle du geste, puis celle du calque), `pivot`
    /// étant le centre de la feuille ; `None` : le dessin tel quel.
    pub fn affine(&self, pivot: egui::Pos2) -> Option<Affine> {
        match (&self.pre, &self.transform) {
            (None, None) => None,
            (Some(p), None) => Some(p.aff),
            (None, Some(t)) => Some(t.affine(pivot)),
            (Some(p), Some(t)) => Some(p.aff.then(&t.affine(pivot))),
        }
    }

    /// Le maillage qui pose la feuille (enveloppe, perspective) et la transformation du calque à
    /// lui appliquer ensuite.
    pub fn warp(&self, pivot: egui::Pos2) -> Option<(&crate::warp::Warp, Option<Affine>)> {
        let w = self.pre.as_ref()?.warp.as_deref()?;
        Some((w, self.transform.as_ref().map(|t| t.affine(pivot))))
    }
}

/// Ce que montre un élément qui n'est pas le dessin de son numéro : le fond d'un dessin, ou le
/// repère d'un calque d'effet.
#[derive(Clone, PartialEq, Debug)]
pub enum FondPart {
    /// La forme déduite du trait, de ces réglages, moins ce qu'on en a coupé, avec ce qu'on a
    /// peint dans le fond, dans la forme (recalculée quand le dessin change ; refaite tuile par
    /// tuile pendant qu'on peint dedans, `fx::fond_patch`).
    Forme(crate::fond::FondReglages),
    /// Le repère d'un calque d'effet d'ajustement (`calqueffet.rs`) : il se résout, avec les éléments
    /// qu'il couvre, en un dessin dérivé (`calqueffet::resolve`). Jamais composé tel quel.
    Reglage(std::sync::Arc<crate::calqueffet::Reglage>),
}

/// L'élément du fond sous le trait du calque `layer` (s'il en a un), à placer juste sous `line`,
/// l'élément de son dessin.
pub fn fond_item(layer: &crate::document::Layer, line: &PlanItem) -> Option<PlanItem> {
    let r = layer.fond.filter(|_| !layer.folder && layer.ctg.is_none() && layer.video.is_none())?;
    Some(PlanItem { fx: Vec::new(), ctg: None, fond: Some(FondPart::Forme(r)), ..line.clone() })
}

/// Couleur du dessin au point (`x`, `y`) de ses pixels (interpolation bilinéaire, prémultipliée).
pub fn sample(raster: &Raster, x: f32, y: f32) -> Color32 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (ix, iy) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - ix, fy - iy);
    let (ix, iy) = (ix as i64, iy as i64);
    // (Toile infinie : le monde entier, hors de la feuille aussi ; vide hors des tuiles dessinées.)
    let get = |x: i64, y: i64| -> [f32; 4] {
        let c = raster.get_i(x, y);
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let (a, b, c, d) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
    let mut px = [0u8; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bottom = c[k] + (d[k] - c[k]) * tx;
        px[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
    }
    Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3])
}

/// Transformation inverse (feuille → dessin) d'un dessin transformé.
fn inverse_of(doc: &Document, item: &PlanItem) -> Option<Affine> {
    item.affine(doc.stage_center()).and_then(|a| a.inverse())
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
    composite_tile_into(doc, plan, idx, buf, doc.width, (0, 0));
}

/// Compose la tuile `idx` dans `buf`, une zone de la feuille de largeur `stride` dont le coin
/// haut-gauche est au point `origin` de la feuille.
fn composite_tile_into(doc: &Document, plan: &Plan, idx: usize, buf: &mut [Color32], stride: usize, origin: (usize, usize)) {
    let w = stride;
    let (ox, oy, tw, th) = tile_bounds(doc.width, doc.height, idx);
    let at = |x: usize, y: usize| (y - origin.1) * w + (x - origin.0);
    let base = paper_color(plan);
    for y in 0..th {
        let s = at(ox, oy + y);
        buf[s..s + tw].fill(base);
    }
    for item in &plan.items {
        // Feuille déformée par un maillage (enveloppe, perspective) : calcul de secours, la carte
        // graphique dessine les mêmes triangles.
        if let Some((warp, layer)) = item.warp(doc.stage_center()) {
            let Some(cel) = crate::fx::shown(doc, item) else { continue };
            // (D'abord dans une zone à part : deux triangles voisins peuvent poser le même pixel.)
            let mut laid = vec![Color32::TRANSPARENT; tw * th];
            warp.fill(&cel.raster, layer.as_ref(), (ox, oy, tw, th), |x, y, src| laid[(y - oy) * tw + (x - ox)] = src);
            for y in 0..th {
                let s = at(ox, oy + y);
                for (x, (d, &src)) in buf[s..s + tw].iter_mut().zip(&laid[y * tw..(y + 1) * tw]).enumerate() {
                    if src.a() != 0 {
                        *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (ox + x, oy + y));
                    }
                }
            }
            continue;
        }
        // Calque transformé : chaque pixel va chercher le point correspondant du dessin.
        if let Some(inv) = inverse_of(doc, item) {
            let Some(cel) = crate::fx::shown(doc, item) else { continue };
            for y in 0..th {
                let s = at(ox, oy + y);
                for (x, d) in buf[s..s + tw].iter_mut().enumerate() {
                    let p = inv.apply(egui::pos2((ox + x) as f32 + 0.5, (oy + y) as f32 + 0.5));
                    let src = sample(&cel.raster, p.x, p.y);
                    if src.a() != 0 {
                        *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (ox + x, oy + y));
                    }
                }
            }
            continue;
        }
        let Some(cel) = crate::fx::shown(doc, item) else { continue };
        // Un dessin d'une autre taille que la feuille (aperçu réduit d'un effet, dessin d'un projet
        // abîmé) : lu à l'échelle, comme `composite_pixel` ; ses tuiles ne sont pas celles de la feuille.
        let r = &cel.raster;
        if r.width != doc.width || r.height != doc.height {
            let (kx, ky) = (r.width as f32 / doc.width.max(1) as f32, r.height as f32 / doc.height.max(1) as f32);
            for y in 0..th {
                let s = at(ox, oy + y);
                for (x, d) in buf[s..s + tw].iter_mut().enumerate() {
                    let src = r.get((((ox + x) as f32 + 0.5) * kx) as usize, (((oy + y) as f32 + 0.5) * ky) as usize);
                    if src.a() != 0 {
                        *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (ox + x, oy + y));
                    }
                }
            }
            continue;
        }
        let Some(tile) = cel.raster.tiles.get(idx).and_then(Option::as_ref) else {
            continue;
        };
        let tile: &[Color32; TILE_PIXELS] = tile;
        for y in 0..th {
            let s = at(ox, oy + y);
            let row = &tile[y * TILE..y * TILE + tw];
            for (x, (d, &src)) in buf[s..s + tw].iter_mut().zip(row).enumerate() {
                if src.a() != 0 {
                    *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (ox + x, oy + y));
                }
            }
        }
    }
}

/// Un rectangle du monde, en pixels entiers : [x0, x1) × [y0, y1). Les coordonnées vont de
/// `-WORLD` à `WORLD` (toile infinie).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
}

impl Area {
    pub fn width(&self) -> usize {
        (self.x1 - self.x0).max(0) as usize
    }

    pub fn height(&self) -> usize {
        (self.y1 - self.y0).max(0) as usize
    }

    /// La feuille du document.
    pub fn sheet(doc: &Document) -> Area {
        Area { x0: 0, y0: 0, x1: doc.width as i64, y1: doc.height as i64 }
    }

    /// Le plus petit rectangle de tuiles entières qui contient `self`.
    pub fn tile_aligned(&self) -> Area {
        let t = TILE as i64;
        Area { x0: self.x0.div_euclid(t) * t, y0: self.y0.div_euclid(t) * t, x1: (self.x1 + t - 1).div_euclid(t) * t, y1: (self.y1 + t - 1).div_euclid(t) * t }
    }

    pub fn union(&self, o: &Area) -> Area {
        Area { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }

    pub fn contains(&self, x: i64, y: i64) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    /// Le rectangle des flottants `lo` à `hi`, arrondi vers l'extérieur et ramené dans le monde.
    pub fn around(lo: (f32, f32), hi: (f32, f32)) -> Area {
        let w = WORLD as f32;
        let c = |v: f32, up: bool| (if up { v.ceil() } else { v.floor() }).clamp(-w, w) as i64;
        Area { x0: c(lo.0, false), y0: c(lo.1, false), x1: c(hi.0, true), y1: c(hi.1, true) }
    }
}

/// Où le plan peut montrer autre chose que du papier : la feuille, et les tuiles dessinées des
/// dessins du plan (hors de la feuille aussi), déplacées par la transformation de leur calque. Rien
/// d'autre ne se compose : partout ailleurs, c'est le papier.
pub fn plan_bounds(doc: &Document, plan: &Plan) -> Area {
    let mut area = Area::sheet(doc);
    let pivot = doc.stage_center();
    let t = TILE as i64;
    for item in &plan.items {
        let Some(cel) = doc.cels.get(&item.cel) else { continue };
        let Some((tx0, ty0, tx1, ty1)) = cel.raster.drawn_tile_range() else { continue };
        let (x0, y0, x1, y1) = ((tx0 as i64 * t) as f32, (ty0 as i64 * t) as f32, ((tx1 as i64 + 1) * t) as f32, ((ty1 as i64 + 1) * t) as f32);
        let r = match item.affine(pivot) {
            Some(a) => {
                let pts = [egui::pos2(x0, y0), egui::pos2(x1, y0), egui::pos2(x1, y1), egui::pos2(x0, y1)].map(|p| a.apply(p));
                let lo = pts.iter().fold((f32::MAX, f32::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
                let hi = pts.iter().fold((f32::MIN, f32::MIN), |(x, y), p| (x.max(p.x), y.max(p.y)));
                if ![lo.0, lo.1, hi.0, hi.1].iter().all(|v| v.is_finite()) {
                    continue;
                }
                Area::around((lo.0 - 2.0, lo.1 - 2.0), (hi.0 + 2.0, hi.1 + 2.0))
            }
            None => Area::around((x0, y0), (x1, y1)),
        };
        area = area.union(&r);
    }
    area
}

/// Les tuiles du monde de `area` où le plan peut montrer autre chose que du papier et qui ne sont
/// pas entièrement dans la feuille : celles des dessins (hors de la feuille, et les bords qui
/// débordent), et, pour un calque transformé, toutes celles que son rectangle déplacé touche.
/// Ce que l'écran compose en plus de la feuille (`monde.rs`).
pub fn plan_world_tiles(doc: &Document, plan: &Plan, area: Area) -> Vec<(i32, i32)> {
    let t = TILE as i64;
    let (w, h) = (doc.width as i64, doc.height as i64);
    let pivot = doc.stage_center();
    let mut set = std::collections::BTreeSet::new();
    let wanted = |tx: i64, ty: i64| {
        let inside = tx >= 0 && ty >= 0 && (tx + 1) * t <= w && (ty + 1) * t <= h;
        !inside && tx * t < area.x1 && (tx + 1) * t > area.x0 && ty * t < area.y1 && (ty + 1) * t > area.y0
    };
    for item in &plan.items {
        let Some(cel) = doc.cels.get(&item.cel) else { continue };
        if item.warp(pivot).is_some() {
            continue;
        }
        let Some((tx0, ty0, tx1, ty1)) = cel.raster.drawn_tile_range() else { continue };
        match item.affine(pivot) {
            Some(a) => {
                let (x0, y0, x1, y1) = ((tx0 as i64 * t) as f32, (ty0 as i64 * t) as f32, ((tx1 as i64 + 1) * t) as f32, ((ty1 as i64 + 1) * t) as f32);
                let pts = [egui::pos2(x0, y0), egui::pos2(x1, y0), egui::pos2(x1, y1), egui::pos2(x0, y1)].map(|p| a.apply(p));
                let lo = pts.iter().fold((f32::MAX, f32::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
                let hi = pts.iter().fold((f32::MIN, f32::MIN), |(x, y), p| (x.max(p.x), y.max(p.y)));
                if ![lo.0, lo.1, hi.0, hi.1].iter().all(|v| v.is_finite()) {
                    continue;
                }
                let r = Area::around((lo.0 - 2.0, lo.1 - 2.0), (hi.0 + 2.0, hi.1 + 2.0)).tile_aligned();
                let (ax0, ay0, ax1, ay1) = (r.x0.max(area.x0).div_euclid(t), r.y0.max(area.y0).div_euclid(t), (r.x1.min(area.x1) + t - 1).div_euclid(t), (r.y1.min(area.y1) + t - 1).div_euclid(t));
                for ty in ay0..ay1 {
                    for tx in ax0..ax1 {
                        if wanted(tx, ty) {
                            set.insert((tx as i32, ty as i32));
                        }
                    }
                }
            }
            None => {
                for id in cel.raster.drawn_ids() {
                    let (tx, ty) = cel.raster.key_of(id);
                    if wanted(tx as i64, ty as i64) {
                        set.insert((tx, ty));
                    }
                }
            }
        }
    }
    set.into_iter().collect()
}
/// Compose la tuile du monde (`tx`, `ty`), en entier (soixante-quatre pixels de côté, y compris la
/// bande d'une tuile de bord qui dépasse la feuille), dans `buf` : une zone de largeur `stride` dont
/// le coin haut gauche est au point `origin` du monde. Hors de la feuille, ce qu'on y a dessiné
/// (`Raster::outer`) ; les feuilles déformées par un maillage (enveloppe, perspective) restent à la
/// feuille.
pub(crate) fn composite_world_tile(doc: &Document, plan: &Plan, tx: i32, ty: i32, buf: &mut [Color32], stride: usize, origin: (i64, i64)) {
    let t = TILE as i64;
    let (ox, oy) = (tx as i64 * t, ty as i64 * t);
    let at = |x: i64, y: i64| ((y - origin.1) as usize) * stride + (x - origin.0) as usize;
    let base = paper_color(plan);
    for y in 0..t {
        let s = at(ox, oy + y);
        buf[s..s + TILE].fill(base);
    }
    let in_window = tx >= 0 && ty >= 0 && ox < doc.width as i64 && oy < doc.height as i64;
    for item in &plan.items {
        if let Some((warp, layer)) = item.warp(doc.stage_center()) {
            if !in_window {
                continue;
            }
            let Some(cel) = crate::fx::shown(doc, item) else { continue };
            let (tw, th) = (TILE.min(doc.width - ox as usize), TILE.min(doc.height - oy as usize));
            let mut laid = vec![Color32::TRANSPARENT; tw * th];
            warp.fill(&cel.raster, layer.as_ref(), (ox as usize, oy as usize, tw, th), |x, y, src| laid[(y - oy as usize) * tw + (x - ox as usize)] = src);
            for y in 0..th {
                let s = at(ox, oy + y as i64);
                for (x, (d, &src)) in buf[s..s + tw].iter_mut().zip(&laid[y * tw..(y + 1) * tw]).enumerate() {
                    if src.a() != 0 {
                        *d = blend_over(*d, src, item.opacity, item.tint, item.blend, ((ox + x as i64) as usize, (oy + y as i64) as usize));
                    }
                }
            }
            continue;
        }
        // Calque transformé : chaque pixel va chercher le point correspondant du dessin.
        if let Some(inv) = inverse_of(doc, item) {
            let Some(cel) = crate::fx::shown(doc, item) else { continue };
            for y in 0..t {
                let s = at(ox, oy + y);
                for (x, d) in buf[s..s + TILE].iter_mut().enumerate() {
                    let p = inv.apply(egui::pos2((ox + x as i64) as f32 + 0.5, (oy + y) as f32 + 0.5));
                    let src = sample(&cel.raster, p.x, p.y);
                    if src.a() != 0 {
                        *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (world_dither(ox + x as i64), world_dither(oy + y)));
                    }
                }
            }
            continue;
        }
        let Some(cel) = crate::fx::shown(doc, item) else { continue };
        let r = &cel.raster;
        // Un dessin d'une autre taille que la feuille (aperçu réduit d'un effet) : lu à l'échelle,
        // sur la feuille seulement.
        if r.width != doc.width || r.height != doc.height {
            if !in_window {
                continue;
            }
            let (kx, ky) = (r.width as f32 / doc.width.max(1) as f32, r.height as f32 / doc.height.max(1) as f32);
            for y in 0..t {
                let s = at(ox, oy + y);
                for (x, d) in buf[s..s + TILE].iter_mut().enumerate() {
                    let (wx, wy) = (ox + x as i64, oy + y);
                    if wx >= doc.width as i64 || wy >= doc.height as i64 {
                        continue;
                    }
                    let src = r.get((((wx as f32) + 0.5) * kx) as usize, (((wy as f32) + 0.5) * ky) as usize);
                    if src.a() != 0 {
                        *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (wx as usize, wy as usize));
                    }
                }
            }
            continue;
        }
        let Some(tile) = r.tile_at(tx, ty) else { continue };
        let tile: &[Color32; TILE_PIXELS] = tile;
        for y in 0..TILE {
            let s = at(ox, oy + y as i64);
            let row = &tile[y * TILE..y * TILE + TILE];
            for (x, (d, &src)) in buf[s..s + TILE].iter_mut().zip(row).enumerate() {
                if src.a() != 0 {
                    *d = blend_over(*d, src, item.opacity, item.tint, item.blend, (world_dither(ox + x as i64), world_dither(oy + y as i64)));
                }
            }
        }
    }
}

/// La position d'un pixel du monde pour les motifs qui en dépendent (le tramage du mode « Dissoudre ») :
/// sur la feuille, la même que toujours ; hors d'elle, le reste de la division par 4 096 (les
/// motifs se répètent, ils n'ont pas besoin d'un entier signé).
fn world_dither(v: i64) -> usize {
    v.rem_euclid(1 << 20) as usize
}

fn tile_count(doc: &Document) -> usize {
    tiles_across(doc.width) * doc.height.div_ceil(TILE)
}

/// Compose le plan sur la zone `area` du monde (toile infinie) : une image `area.width() ×
/// area.height()`, papier compris. Les tuiles du monde sont réparties sur les cœurs, une rangée de
/// tuiles par morceau.
pub fn composite_area(doc: &Document, plan: &Plan, area: Area) -> Vec<Color32> {
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    let (w, h) = (area.width(), area.height());
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let al = area.tile_aligned();
    let (aw, ah) = (al.width(), al.height());
    let mut buf = vec![Color32::TRANSPARENT; aw * ah];
    let t = TILE as i64;
    par_chunks(&mut buf, TILE * aw, |band, rows| {
        let ty = (al.y0 / t) as i32 + band as i32;
        for tx in (al.x0 / t) as i32..(al.x1 / t) as i32 {
            composite_world_tile(doc, plan, tx, ty, rows, aw, (al.x0, ty as i64 * t));
        }
    });
    if al == area {
        return buf;
    }
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        let s = ((area.y0 - al.y0) as usize + y) * aw + (area.x0 - al.x0) as usize;
        out.extend_from_slice(&buf[s..s + w]);
    }
    out
}

pub fn composite_full(doc: &Document, plan: &Plan) -> Vec<Color32> {
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    let mut buf = vec![Color32::TRANSPARENT; doc.width * doc.height];
    for idx in 0..tile_count(doc) {
        composite_tile(doc, plan, idx, &mut buf);
    }
    buf
}

/// Traite `buf` par morceaux de `chunk` éléments, répartis sur les cœurs du processeur
/// (`f` reçoit le numéro du morceau et le morceau).
pub(crate) fn par_chunks<T: Send>(buf: &mut [T], chunk: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let mut threads = crate::parallele::coeurs().clamp(1, 16);
    // Un travail de fond (effets calculés en arrière-plan) laisse des cœurs libres à l'interface.
    let cap = THREAD_CAP.with(|c| c.get());
    if cap > 0 {
        threads = threads.min(cap);
    }
    let mut groups: Vec<Vec<(usize, &mut [T])>> = (0..threads).map(|_| Vec::new()).collect();
    // En alternance : les zones chargées (souvent groupées) se répartissent entre les fils.
    for (i, c) in buf.chunks_mut(chunk.max(1)).enumerate() {
        groups[i % threads].push((i, c));
    }
    groups.retain(|g| !g.is_empty());
    let f = &f;
    // Un seul groupe : pas la peine de lancer un fil.
    if groups.len() <= 1 {
        for group in groups {
            for (i, c) in group {
                f(i, c);
            }
        }
        return;
    }
    std::thread::scope(|s| {
        for group in groups {
            s.spawn(move || {
                for (i, c) in group {
                    f(i, c);
                }
            });
        }
    });
}

thread_local! {
    /// Nombre de fils que `par_chunks` peut lancer depuis ce fil (0 : autant que de cœurs).
    static THREAD_CAP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Limite à `cap` le nombre de fils que `par_chunks` lance depuis le fil appelant (le fil de fond
/// des effets s'en sert pour ne pas affamer l'interface).
pub(crate) fn set_thread_cap(cap: usize) {
    THREAD_CAP.with(|c| c.set(cap));
}

/// La limite de fils de ce fil de fond (0 : aucune), pour les travaux qui répartissent eux-mêmes
/// leur travail (`fxtrait.rs`).
pub(crate) fn thread_cap() -> usize {
    THREAD_CAP.with(|c| c.get())
}

/// Ce que voit la caméra `cam` : l'image finale (`doc.frame`). Chaque pixel prend la couleur de
/// la feuille au point filmé (interpolation bilinéaire) ; caméra centrée sans zoom ni rotation :
/// les pixels de la feuille, tels quels. Hors de la feuille (toile infinie) : ce qu'on y a dessiné,
/// sur le papier (transparent si le plan n'a pas de papier).
///
/// Seule la zone filmée est composée, et seulement là où il y a autre chose que du papier (la
/// feuille et ce que les dessins ont hors d'elle), en tuiles entières avec une marge pour
/// l'interpolation ; le travail est réparti sur les cœurs du processeur.
pub fn camera_image(doc: &Document, plan: &Plan, cam: &Camera) -> Vec<Color32> {
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    let [fw, fh] = doc.frame;
    let mut out = vec![Color32::TRANSPARENT; fw * fh];
    let (w, h) = (doc.width, doc.height);
    let map = cam.frame_map(doc.frame);
    let corners = cam.corners(doc.frame);
    let mut lo = corners.iter().fold((f32::MAX, f32::MAX), |(x, y), c| (x.min(c.x), y.min(c.y)));
    let mut hi = corners.iter().fold((f32::MIN, f32::MIN), |(x, y), c| (x.max(c.x), y.max(c.y)));
    // Caméra tournée qui frôle l'horizon : la zone filmée n'est plus bornée par ses coins.
    if !cam.sees_reference(doc.frame) {
        (lo, hi) = ((0.0, 0.0), (w as f32, h as f32));
    }
    if w == 0 || h == 0 || ![lo.0, lo.1, hi.0, hi.1].iter().all(|v| v.is_finite()) {
        return out;
    }
    // Où il y a autre chose que du papier : la feuille et ce que les dessins ont autour d'elle.
    let busy = plan_bounds(doc, plan);
    let want = Area::around((lo.0 - 2.0, lo.1 - 2.0), (hi.0 + 2.0, hi.1 + 2.0));
    let seen = Area { x0: want.x0.max(busy.x0), y0: want.y0.max(busy.y0), x1: want.x1.min(busy.x1), y1: want.y1.min(busy.y1) };
    let base = paper_color(plan);
    let area = if seen.x1 > seen.x0 && seen.y1 > seen.y0 { seen.tile_aligned() } else { Area { x0: 0, y0: 0, x1: 0, y1: 0 } };
    let (rw, rh) = (area.width(), area.height());
    // Composer la zone, une bande de tuiles par morceau.
    let region = composite_area(doc, plan, area);

    // Échantillonner ce que filme la caméra. Hors de la zone : le papier.
    let region = &region;
    let get = |x: i64, y: i64| -> [f32; 4] {
        let c = if area.contains(x, y) { region[(y - area.y0) as usize * rw + (x - area.x0) as usize] } else { base };
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let _ = rh;
    const ROWS: usize = 16;
    par_chunks(&mut out, fw * ROWS, |chunk, rows| {
        for (k, px_out) in rows.iter_mut().enumerate() {
            let (u, v) = (k % fw, chunk * ROWS + k / fw);
            let Some(p) = map.at(u as f32 + 0.5, v as f32 + 0.5) else { continue };
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
            *px_out = Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3]);
        }
    });
    out
}

pub fn composite_pixel(doc: &Document, plan: &Plan, x: usize, y: usize) -> Color32 {
    composite_pixel_i(doc, plan, x as i64, y as i64)
}

/// Un élément du plan dont le dessin affiché (effets compris) et la transformation inverse sont
/// déjà trouvés : de quoi composer des milliers de pixels sans refaire ce travail pour chacun.
/// Le dessin peut être plus petit que la feuille : pendant qu'une vignette attend son effet,
/// `fx::shown` rend le dernier résultat gardé, qui peut être un aperçu réduit (`fxproxy.rs`) ; on
/// le lit à l'échelle (`kx`, `ky` : dessin sur feuille ; le lire comme une feuille entière sortait
/// du dessin et fermait SHOT au lancement, 2026-10-18).
struct Resolved<'a> {
    item: &'a PlanItem,
    cel: crate::fx::Shown<'a>,
    inv: Option<Affine>,
    kx: f32,
    ky: f32,
    /// Le dessin a la taille de la feuille.
    same: bool,
}

fn resolve<'a>(doc: &'a Document, plan: &'a Plan) -> Vec<Resolved<'a>> {
    plan.items
        .iter()
        .filter_map(|item| {
            let cel = crate::fx::shown(doc, item)?;
            let (w, h) = (cel.raster.width, cel.raster.height);
            let kx = w as f32 / doc.width.max(1) as f32;
            let ky = h as f32 / doc.height.max(1) as f32;
            Some(Resolved { item, cel, inv: inverse_of(doc, item), kx, ky, same: w == doc.width && h == doc.height })
        })
        .collect()
}

/// Le pixel (`x`, `y`) du monde (la feuille, ou la toile autour d'elle) : le papier `base`, puis
/// chaque élément.
fn pixel_of(layers: &[Resolved], base: Color32, x: i64, y: i64) -> Color32 {
    let mut c = base;
    for l in layers {
        let r = &l.cel.raster;
        let s = match &l.inv {
            Some(inv) => {
                let p = inv.apply(egui::pos2(x as f32 + 0.5, y as f32 + 0.5));
                sample(r, p.x * l.kx, p.y * l.ky)
            }
            None if l.same => r.get_i(x, y),
            None if x < 0 || y < 0 => Color32::TRANSPARENT,
            None => {
                let (rx, ry) = (((x as f32 + 0.5) * l.kx) as usize, ((y as f32 + 0.5) * l.ky) as usize);
                if rx < r.width && ry < r.height { r.get(rx, ry) } else { Color32::TRANSPARENT }
            }
        };
        if s.a() != 0 {
            c = blend_over(c, s, l.item.opacity, l.item.tint, l.item.blend, (world_dither(x), world_dither(y)));
        }
    }
    c
}

/// Le pixel (`x`, `y`) du monde, hors de la feuille aussi (la pipette sur la toile infinie).
pub fn composite_pixel_i(doc: &Document, plan: &Plan, x: i64, y: i64) -> Color32 {
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    pixel_of(&resolve(doc, plan), paper_color(plan), x, y)
}

/// Plan de l'image `frame` du panneau `index` tel que la caméra le filme (vignettes, export) :
/// les calques en profondeur défilent selon le décalage de la caméra (multiplan).
pub fn panel_plan_at(doc: &Document, index: usize, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    plan_with_camera(&doc.panels[index], frame, paper, &doc.camera_seen(index, frame), doc.fps)
}

/// L'image `frame` d'un plan rangé (une version qui n'est pas dans `doc.panels`), avec sa propre caméra : la
/// vignette d'une carte de la pile des versions.
pub fn stored_panel_plan(doc: &Document, panel: &Panel, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    plan_with_camera(panel, frame, paper, &doc.camera_seen_du_plan_range(panel, frame), doc.fps)
}

/// Le calque `li` du plan `index` seul, à l'image `frame`, tel que la caméra au centre le voit
/// (export vers Storyboard Pro, 14b) : en mode normal, à pleine opacité et même caché (sa
/// visibilité et son opacité sont reprises à part), effets compris. Aucun : un dossier, ou rien
/// à cette image.
pub fn layer_plan_at(doc: &Document, index: usize, li: usize, frame: usize) -> Option<Plan> {
    let panel = doc.panels.get(index)?;
    let l = panel.layers.get(li)?;
    if l.folder || l.effet.is_some() {
        return None;
    }
    let cel = crate::video::cel_at(l, frame, doc.fps)?;
    let t = l.pose(frame);
    let line = PlanItem {
        cel,
        opacity: 255,
        tint: None,
        transform: (!t.is_identity()).then_some(t),
        blend: BlendMode::Normal,
        fx: crate::fx::layer_steps(l, frame, None),
        ctg: crate::ctg::input(&panel.layers, li, frame),
        fond: None,
        pre: None,
    };
    // Le fond sous le trait fait partie de l'image du calque.
    Some(Plan { paper: None, items: fond_item(l, &line).into_iter().chain([line]).collect() })
}

fn plan_with_camera(panel: &Panel, frame: usize, paper: Option<[u8; 3]>, camera: &CameraSeen, fps: u32) -> Plan {
    let mut items: Vec<PlanItem> = Vec::new();
    // Pour chaque calque, le nombre d'éléments posés avant lui (un calque d'effet couvre ceux d'en dessous).
    let mut starts: Vec<usize> = Vec::with_capacity(panel.layers.len());
    for (li, l) in panel.layers.iter().enumerate() {
        starts.push(items.len());
        // Caché (lui ou son dossier, 13b-5) : rien ; l'opacité du dossier s'ajoute.
        let Some(opacity) = crate::folders::effect(&panel.layers, li) else { continue };
        // Calque d'effet (`calqueffet.rs`) : son repère, ou rien (une incrustation se pose sur l'image
        // finale, `incrustation.rs`).
        if l.effet.is_some() {
            items.extend(crate::calqueffet::marker(&panel.layers, li, frame, opacity, &starts));
            continue;
        }
        let Some(t) = l.transform_seen_at(camera, frame) else { continue };
        // Calque vidéo (13k) : l'image de la vidéo qui tombe là.
        let Some(cel) = crate::video::cel_at(l, frame, fps) else { continue };
        let line = PlanItem {
            cel,
            opacity: (opacity * 255.0).round() as u8,
            tint: None,
            transform: (!t.is_identity()).then_some(t),
            blend: l.blend,
            fx: crate::fx::layer_steps(l, frame, None),
            ctg: crate::ctg::input(&panel.layers, li, frame),
            fond: None,
            pre: None,
        };
        // Le fond sous le trait (`fond.rs`), juste sous le dessin de son calque.
        items.extend(fond_item(l, &line));
        items.push(line);
    }
    Plan { paper, items }
}

/// Image réduite de ce que voit la caméra `cam` (moyenne de 4 échantillons par pixel).
pub fn thumbnail(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize) -> ColorImage {
    thumbnail_until(doc, plan, cam, tw, th, &|| false).expect("jamais interrompue")
}

#[cfg(test)]
thread_local! {
    /// Vignettes composées par ce fil (essais : l'interface n'en compose aucune, `thumbs.rs`).
    pub(crate) static THUMBNAILS_HERE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Comme `thumbnail`, mais `cancelled` est demandé entre deux lignes : vrai, le calcul s'arrête et
/// rend `None` (une vignette que plus personne n'attend, `thumbs.rs`).
pub fn thumbnail_until(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize, cancelled: &dyn Fn() -> bool) -> Option<ColorImage> {
    #[cfg(test)]
    THUMBNAILS_HERE.with(|n| n.set(n.get() + 1));
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    let [fw, fh] = doc.frame;
    let (sx, sy) = (fw as f32 / tw as f32, fh as f32 / th as f32);
    let map = cam.frame_map(doc.frame);
    let mut pixels = Vec::with_capacity(tw * th);
    // (Les dessins affichés se cherchent une fois, pas à chaque échantillon : 80 000 par vignette.)
    let layers = resolve(doc, plan);
    let base = paper_color(plan);
    for y in 0..th {
        if cancelled() {
            return None;
        }
        for x in 0..tw {
            let mut acc = [0u32; 4];
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                // (Toile infinie : le cadre peut filmer hors de la feuille.)
                let c = match map.at((x as f32 + ox) * sx, (y as f32 + oy) * sy) {
                    Some(p) if p.x.abs() < WORLD as f32 && p.y.abs() < WORLD as f32 => pixel_of(&layers, base, p.x.floor() as i64, p.y.floor() as i64),
                    _ => Color32::TRANSPARENT,
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
    Some(ColorImage {
        size: [tw, th],
        pixels,
    })
}

/// Le dessin entier réduit à `max_w` pixels de large au plus (moyenne de 4 échantillons par
/// pixel) : texture des plans de la vue du plateau en 3D (9e).
pub fn cel_image(raster: &Raster, max_w: usize) -> ColorImage {
    let (w, h) = (raster.width.max(1), raster.height.max(1));
    let tw = w.min(max_w.max(1));
    let th = ((h as f32 * tw as f32 / w as f32).round() as usize).max(1);
    let (sx, sy) = (w as f32 / tw as f32, h as f32 / th as f32);
    let mut pixels = vec![Color32::TRANSPARENT; tw * th];
    const ROWS: usize = 16;
    par_chunks(&mut pixels, tw * ROWS, |chunk, rows| {
        for (k, px) in rows.iter_mut().enumerate() {
            let (x, y) = (k % tw, chunk * ROWS + k / tw);
            let mut acc = [0u32; 4];
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let c = sample(raster, (x as f32 + ox) * sx, (y as f32 + oy) * sy);
                for (a, v) in acc.iter_mut().zip(c.to_array()) {
                    *a += v as u32;
                }
            }
            *px = Color32::from_rgba_premultiplied((acc[0] / 4) as u8, (acc[1] / 4) as u8, (acc[2] / 4) as u8, (acc[3] / 4) as u8);
        }
    });
    ColorImage { size: [tw, th], pixels }
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
    /// Ce qui a changé depuis la dernière image pour le hors-feuille (toile infinie, `monde.rs`) : tout,
    /// ou ces tuiles (numéros de la fenêtre et du hors-feuille).
    world_all: bool,
    world_tiles: Vec<usize>,
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
            world_all: true,
            world_tiles: Vec::new(),
        }
    }

    pub fn set_gpu(&mut self, on: bool) {
        self.gpu = on;
    }

    pub fn invalidate_all(&mut self) {
        self.dirty_all = true;
        self.gpu_all = true;
        self.world_all = true;
    }

    /// Un fond arrivé de l'arrière-plan (`fx::fond_poll`) remplace celui d'avant : le plan est le
    /// même, l'image composée par le processeur est à refaire (la carte graphique suit les versions
    /// des dessins dérivés toute seule).
    pub fn invalidate_composed(&mut self) {
        self.dirty_all = true;
        self.world_all = true;
    }

    pub fn invalidate_tiles(&mut self, tiles: &[usize]) {
        if !self.world_all {
            self.world_tiles.extend_from_slice(tiles);
            // (Au-delà de quelques milliers de tuiles, tout refaire coûte moins que les suivre.)
            if self.world_tiles.len() > 8192 {
                self.world_all = true;
                self.world_tiles.clear();
            }
        }
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
    /// Changements à montrer hors de la feuille depuis la dernière fois : tout, ou ces tuiles.
    pub fn take_world_dirty(&mut self) -> (bool, Vec<usize>) {
        let mut tiles = std::mem::take(&mut self.world_tiles);
        tiles.sort_unstable();
        tiles.dedup();
        (std::mem::take(&mut self.world_all), tiles)
    }

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
        // (Les repères de calques d'effets sont déjà résolus par l'écran, `calqueffet::resolve` ; sinon, ici. Les
        // dessins dérivés sont tenus jusqu'à la fin de la composition.)
        let crate::calqueffet::Resolu { plan: resolved, tenus: _tenus } = crate::calqueffet::resolve_auto(doc, &plan);
        let plan = match resolved {
            Cow::Borrowed(_) => plan,
            Cow::Owned(resolved) => resolved,
        };
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
