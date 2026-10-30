//! Choisir un objet au clic (Anthony, 2026-10-20 : « Ctrl pour sélection, puis Maj+Ctrl pour sélectionner
//! un autre objet et cumuler les sélections »).
//!
//! La règle, qui prolonge celles qui existaient :
//! - **Ctrl + clic** (le lasso temporaire de Ctrl, `springtools.rs`, ou l'outil Sélection) choisit
//!   l'objet sous la souris et remplace la sélection. Sur un calque vectoriel, l'objet est le trait ou
//!   la zone remplie (c'était déjà ainsi). Sur un calque bitmap, c'est **la forme d'un seul tenant** :
//!   les pixels dessinés qui se touchent (huit voisins) autour du pixel visé — ou du plus proche, à
//!   quatre pixels d'écran près, pour atteindre un trait fin. Un clic dans le vide désélectionne.
//!   Un glisser reste un lasso ou un rectangle : seul le clic sans glisser choisit un objet.
//! - **Ctrl + Maj + clic** ajoute l'objet à la sélection, sur n'importe quel calque du plan, ou le
//!   retire s'il y était déjà ; **Ctrl + Maj + lasso** ajoute tout ce qu'il prend, sur tous les calques
//!   (c'est le geste « tous les calques » de `multitools.rs`, qui jusqu'ici
//!   remplaçait la sélection). Une sélection faite sur un seul calque (trait ou lasso vectoriel) est
//!   reprise dans le cumul ; une zone bitmap soulevée est d'abord posée.
//! - **Maj + clic** (sans Ctrl) fait la même chose sur les calques choisis, comme avant.

use eframe::egui::{Pos2, Rect};

use crate::app::App;
use crate::multisel::{self, MultiSel, PartKind};
use crate::pixsel::PixelMask;
use crate::raster::Raster;
use crate::vector;

/// Au plus autant de pixels dans un objet (au-delà, c'est tout le dessin : on s'arrête là).
const MAX_PIXELS: usize = 40_000_000;

/// Le pixel dessiné (opaque, même un peu) le plus proche de `p` à `reach` pixels près (le pixel visé
/// d'abord).
fn nearest_drawn(raster: &Raster, p: Pos2, reach: f32) -> Option<(usize, usize)> {
    let (w, h) = (raster.width as i64, raster.height as i64);
    let (cx, cy) = (p.x.floor() as i64, p.y.floor() as i64);
    let r = reach.ceil().max(1.0) as i64;
    let mut best: Option<((usize, usize), f32)> = None;
    for y in (cy - r).max(0)..=(cy + r).min(h - 1) {
        for x in (cx - r).max(0)..=(cx + r).min(w - 1) {
            if raster.get(x as usize, y as usize).a() == 0 {
                continue;
            }
            let d = pos_dist(p, x, y);
            if d <= reach.max(0.75) && best.is_none_or(|(_, bd)| d < bd) {
                best = Some(((x as usize, y as usize), d));
            }
        }
    }
    best.map(|(xy, _)| xy)
}

/// Distance du point `p` au centre du pixel (`x`, `y`).
fn pos_dist(p: Pos2, x: i64, y: i64) -> f32 {
    ((p.x - (x as f32 + 0.5)).powi(2) + (p.y - (y as f32 + 0.5)).powi(2)).sqrt()
}

/// La forme d'un seul tenant (huit voisins) de pixels dessinés qui contient le pixel (`x`, `y`) de
/// `raster` : sa zone et ses contours (pixels du dessin). Rien si le pixel est vide.
pub fn object_at(raster: &Raster, x: usize, y: usize) -> Option<(PixelMask, Vec<Vec<Pos2>>)> {
    let (w, h) = (raster.width, raster.height);
    if x >= w || y >= h || raster.get(x, y).a() == 0 {
        return None;
    }
    let mut region = vec![false; w * h];
    let mut stack = vec![(x, y)];
    region[y * w + x] = true;
    let mut count = 1usize;
    while let Some((cx, cy)) = stack.pop() {
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (nx, ny) = (cx as i64 + dx, cy as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let i = ny as usize * w + nx as usize;
                if !region[i] && raster.get(nx as usize, ny as usize).a() > 0 {
                    region[i] = true;
                    count += 1;
                    stack.push((nx as usize, ny as usize));
                }
            }
        }
        if count > MAX_PIXELS {
            break;
        }
    }
    let mask = PixelMask::from_bools(&region, w, h)?;
    let outline = vector::trace_contours(&region, w, h).into_iter().map(|c| c.into_iter().map(|q| Pos2::new(q[0], q[1])).collect()).collect();
    Some((mask, outline))
}

/// La forme sous (ou tout près de) `p` : voir `object_at`. `reach` : la portée du clic, en pixels du dessin.
pub fn object_near(raster: &Raster, p: Pos2, reach: f32) -> Option<(PixelMask, Vec<Vec<Pos2>>)> {
    let (x, y) = nearest_drawn(raster, p, reach)?;
    object_at(raster, x, y)
}

/// Le rectangle englobant d'un masque, pour les messages.
fn size_text(mask: &PixelMask) -> String {
    let r: Rect = mask.bounds();
    format!("{}×{} px", r.width().round(), r.height().round())
}

impl App {
    /// Un clic sans glisser avec l'outil Sélection sur un calque bitmap, en `p` (point du dessin) :
    /// l'objet sous la souris devient la zone choisie. `add` (Maj) : il s'ajoute à la zone, ou en est
    /// retiré s'il y était. Vrai si un objet a été trouvé (sinon l'appelant désélectionne).
    pub(crate) fn bitmap_pick_object(&mut self, p: Pos2, add: bool) -> bool {
        let Some(cel) = self.cel_here() else { return false };
        let reach = 4.0 / self.layer_zoom().max(1e-3);
        let Some(c) = self.doc.cels.get(&cel) else { return false };
        let Some((mask, outline)) = object_near(&c.raster, p, reach) else { return false };
        let n = size_text(&mask);
        let keep = add && self.bsel.as_ref().is_some_and(|s| s.cel == cel && s.lifted.is_none());
        match self.bsel.as_mut().filter(|_| keep) {
            Some(sel) => {
                // Déjà choisi (le pixel visé est dans la zone) : on le retire, sinon on l'ajoute.
                let inside = nearest_drawn(&self.doc.cels[&cel].raster, p, reach).is_some_and(|(x, y)| sel.mask.at(x, y) > 0.0);
                if inside {
                    sel.mask = sel.mask.without(&mask);
                    sel.outline = rebuild_outline(&sel.mask, self.doc.width, self.doc.height);
                    if sel.mask.is_empty() {
                        self.bsel = None;
                    }
                    self.status = format!("Objet retiré de la sélection ({n}).");
                } else {
                    sel.mask = sel.mask.union(&mask);
                    sel.outline.extend(outline);
                    self.status = format!("Objet ajouté à la sélection ({n}).");
                }
            }
            None => {
                self.drop_bitmap_selection();
                self.bsel = Some(crate::pixtools::BSelection { cel, mask, outline, lifted: None });
                self.status = format!("Objet choisi ({n}). Maj + clic : en ajouter un autre.");
            }
        }
        true
    }

    /// Ctrl+Maj : la sélection faite sur le seul calque courant (traits d'un dessin vectoriel, ou
    /// zone bitmap pas encore soulevée) entre dans la sélection sur plusieurs calques, pour que le
    /// geste suivant s'y ajoute au lieu de la remplacer.
    pub(crate) fn carry_single_selection(&mut self) {
        let (panel, frame) = (self.cur_panel, self.cur_frame);
        let targets = multisel::targets(&self.doc, panel, frame, &[self.cur_layer]);
        let Some(t) = targets.list.first() else { return };
        let part = if let Some(v) = self.vsel.as_ref().filter(|v| v.cel == t.cel && !v.items.is_empty() && v.items.iter().all(|&i| i < t.elements)) {
            multisel::vector_part(t, v.items.clone())
        } else if let Some(b) = self.bsel.as_ref().filter(|b| b.cel == t.cel && b.lifted.is_none()) {
            multisel::bitmap_part(t, b.mask.clone(), b.outline.clone(), None)
        } else {
            return;
        };
        match self.multi.sel.as_mut().filter(|s| (s.panel, s.frame) == (panel, frame)) {
            Some(s) => s.merge(vec![part]),
            None => self.multi.sel = Some(MultiSel::new(panel, frame, vec![part])),
        }
    }

    /// Un clic sans glisser avec l'outil Sélection sur plusieurs calques, en `stage` (point de la
    /// feuille) : la forme dessinée sous la souris, sur le calque bitmap le plus haut qui en a une.
    /// `add` : elle s'ajoute à la sélection ou en est retirée. Vrai si une forme a été trouvée.
    pub(crate) fn multi_pick_object(&mut self, stage: Pos2, add: bool) -> bool {
        let targets = self.multi_targets();
        let tol = 4.0 / self.view.zoom.max(1e-3);
        let (panel, frame) = (self.cur_panel, self.cur_frame);
        for t in targets.list.iter().rev().filter(|t| !t.vector) {
            let Some(inv) = t.to_stage.inverse() else { continue };
            // (Les pixels du dessin, ou ceux de sa strate d'aplat quand l'aplat est la cible.)
            let Some(raster) = multisel::raster_of(&self.doc, t.cel, t.aplat) else { continue };
            let local = inv.apply(stage);
            let Some((mask, outline)) = object_near(raster, local, tol * inv.width_factor()) else { continue };
            let n = size_text(&mask);
            let (px, py) = nearest_drawn(raster, local, tol * inv.width_factor()).unwrap_or((0, 0));
            if !add {
                self.multi.sel = Some(MultiSel::new(panel, frame, vec![multisel::bitmap_part(t, mask, outline, None)]));
                self.status = format!("Objet choisi sur « {} » ({n}). Ctrl+Maj + clic : en ajouter un autre.", t.name);
            } else {
                let sel = self.multi.sel.get_or_insert_with(|| MultiSel::new(panel, frame, Vec::new()));
                let existing = sel.parts.iter().position(|p| p.layer == t.id && p.cel == t.cel && matches!(p.kind, PartKind::Bitmap { .. }));
                let inside = existing.is_some_and(|k| matches!(&sel.parts[k].kind, PartKind::Bitmap { mask: m, .. } if m.at(px, py) > 0.0));
                match (existing, inside) {
                    (Some(k), true) => {
                        let (w, h) = (self.doc.width, self.doc.height);
                        if let PartKind::Bitmap { mask: m, outline: o, lift: None } = &mut sel.parts[k].kind {
                            *m = m.without(&mask);
                            *o = rebuild_outline(m, w, h);
                            if m.is_empty() {
                                sel.parts.remove(k);
                            }
                        }
                        sel.boxed = None;
                        self.status = format!("Objet retiré de la sélection ({n}).");
                    }
                    _ => {
                        sel.merge(vec![multisel::bitmap_part(t, mask, outline, None)]);
                        self.status = format!("Objet ajouté à la sélection ({n}).");
                    }
                }
                if sel.parts.is_empty() {
                    self.multi.sel = None;
                }
            }
            self.multi_adopt_layers();
            self.multi.all_layers = false;
            return true;
        }
        false
    }
}

/// Les contours d'un masque (pour l'afficher après un retrait).
fn rebuild_outline(mask: &PixelMask, w: usize, h: usize) -> Vec<Vec<Pos2>> {
    let mut region = vec![false; w * h];
    for y in mask.y0..(mask.y0 + mask.h).min(h) {
        for x in mask.x0..(mask.x0 + mask.w).min(w) {
            region[y * w + x] = mask.at(x, y) > 0.0;
        }
    }
    vector::trace_contours(&region, w, h).into_iter().map(|c| c.into_iter().map(|q| Pos2::new(q[0], q[1])).collect()).collect()
}
