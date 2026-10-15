//! Outils des calques vectoriels : trait, gomme qui coupe, pot de peinture, formes,
//! sélection au lasso avec poignées (déplacer, mettre à l'échelle, tourner), presse-papiers.

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::app::{App, Tool};
use crate::brush::{StrokeMode, StrokeState};
use crate::document::{CelId, LayerKind};
use crate::fill;
use crate::history::Edit;
use crate::vector::{self, Affine, Element, VPoint, VectorDrawing};

/// Un trait vectoriel en cours : il se dessine en direct comme un trait bitmap,
/// puis devient un élément vectoriel au relâchement.
pub(crate) struct VectorStroke {
    pub cel: CelId,
    pub before: VectorDrawing,
    pub prefix: Option<Edit>,
    pub color: [u8; 3],
    pub opacity: f32,
    pub hardness: f32,
    /// Pinceau texturé (7b), textures désignées par leur clé de contenu.
    pub brush: Option<Box<crate::brush::BrushSettings>>,
}

pub(crate) struct VectorErase {
    pub cel: CelId,
    pub before: VectorDrawing,
    pub changed: bool,
    pub last: Option<Pos2>,
    pub pen: bool,
    /// Zone touchée par la gomme depuis le début du geste.
    pub region: Rect,
}

pub(crate) struct VSelection {
    pub cel: CelId,
    pub items: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handle {
    Move,
    /// Coins dans l'ordre : haut-gauche, haut-droite, bas-droite, bas-gauche.
    Corner(usize),
    /// Milieux des côtés : haut, droite, bas, gauche.
    Edge(usize),
    Rotate,
}

pub(crate) enum VDrag {
    Lasso(Vec<Pos2>),
    Transform {
        cel: CelId,
        handle: Handle,
        start: Pos2,
        bbox: Rect,
        before: VectorDrawing,
        originals: Vec<Element>,
        changed: bool,
    },
}

/// Rayon de prise des poignées, en pixels d'écran.
const HANDLE_GRAB: f32 = 9.0;

pub(crate) fn transform_for(handle: Handle, s: Pos2, p: Pos2, b: Rect, shift: bool) -> Affine {
    let ratio = |num: f32, den: f32| {
        if den.abs() < 1e-3 {
            return 1.0;
        }
        let r = num / den;
        if r.abs() < 0.01 {
            0.01f32.copysign(r)
        } else {
            r
        }
    };
    match handle {
        Handle::Move => {
            let mut d = p - s;
            if shift {
                if d.x.abs() > d.y.abs() {
                    d.y = 0.0;
                } else {
                    d.x = 0.0;
                }
            }
            Affine::translate(d)
        }
        Handle::Rotate => {
            let c = b.center();
            let mut a = (p - c).angle() - (s - c).angle();
            if shift {
                let step = 15f32.to_radians();
                a = (a / step).round() * step;
            }
            Affine::rotate_about(c, a)
        }
        Handle::Corner(k) => {
            let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()];
            let (corner, anchor) = (corners[k], corners[(k + 2) % 4]);
            let mut sx = ratio(p.x - anchor.x, corner.x - anchor.x);
            let mut sy = ratio(p.y - anchor.y, corner.y - anchor.y);
            if shift {
                let m = sx.abs().max(sy.abs());
                sx = m.copysign(sx);
                sy = m.copysign(sy);
            }
            Affine::scale_about(anchor, sx, sy)
        }
        Handle::Edge(k) => {
            let c = b.center();
            match k {
                0 => Affine::scale_about(pos2(c.x, b.max.y), 1.0, ratio(p.y - b.max.y, b.min.y - b.max.y)),
                1 => Affine::scale_about(pos2(b.min.x, c.y), ratio(p.x - b.min.x, b.max.x - b.min.x), 1.0),
                2 => Affine::scale_about(pos2(c.x, b.min.y), 1.0, ratio(p.y - b.min.y, b.max.y - b.min.y)),
                _ => Affine::scale_about(pos2(b.max.x, c.y), ratio(p.x - b.max.x, b.min.x - b.max.x), 1.0),
            }
        }
    }
}

impl App {
    pub(crate) fn drawing(&self, cel: CelId) -> Option<&VectorDrawing> {
        self.doc.cels.get(&cel)?.vector.as_ref()
    }

    pub(crate) fn drawing_mut(&mut self, cel: CelId) -> Option<&mut VectorDrawing> {
        self.doc.cels.get_mut(&cel)?.vector.as_mut()
    }

    /// La sélection ne vaut que pour le dessin affiché ; sinon on l'oublie.
    pub(crate) fn validate_selection(&mut self) {
        let Some(sel) = &self.vsel else { return };
        let count = self.drawing(sel.cel).map_or(0, |d| d.elements.len());
        let valid = Some(sel.cel) == self.cel_here()
            && !sel.items.is_empty()
            && sel.items.iter().all(|&i| i < count);
        if !valid && self.vdrag.is_none() {
            self.vsel = None;
        }
    }

    /// Rectangle de la sélection courante (bitmap ou vectorielle), pour les poignées.
    pub(crate) fn selection_bounds(&self) -> Option<Rect> {
        if self.bsel.is_some() {
            return self.bitmap_selection_bounds();
        }
        let sel = self.vsel.as_ref()?;
        let d = self.drawing(sel.cel)?;
        let r = sel
            .items
            .iter()
            .filter_map(|&i| d.elements.get(i))
            .fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
        (r.is_finite() && r.width() >= 0.0).then_some(r)
    }

    /// Poignées de la sélection, en coordonnées du dessin.
    pub(crate) fn handle_points(&self, b: Rect) -> Vec<(Handle, Pos2)> {
        let c = b.center();
        let up = 28.0 / self.layer_zoom();
        vec![
            (Handle::Rotate, pos2(c.x, b.min.y - up)),
            (Handle::Corner(0), b.left_top()),
            (Handle::Corner(1), b.right_top()),
            (Handle::Corner(2), b.right_bottom()),
            (Handle::Corner(3), b.left_bottom()),
            (Handle::Edge(0), pos2(c.x, b.min.y)),
            (Handle::Edge(1), pos2(b.max.x, c.y)),
            (Handle::Edge(2), pos2(c.x, b.max.y)),
            (Handle::Edge(3), pos2(b.min.x, c.y)),
        ]
    }

    /// Poignée sous le pointeur (position écran), le cas échéant.
    pub(crate) fn handle_at(&self, screen: Pos2) -> Option<Handle> {
        let b = self.selection_bounds()?;
        self.handle_points(b)
            .into_iter()
            .find(|(_, q)| self.layer_to_screen(*q).distance(screen) <= HANDLE_GRAB)
            .map(|(h, _)| h)
    }

    // ------------------------------------------------------------------ trait et gomme

    pub(crate) fn begin_vector_stroke(&mut self, p: Pos2, pressure: f32, pen: bool) {
        let Some(mut settings) = self.current_brush().copied() else {
            return;
        };
        settings.aliased = false;
        if self.tool == Tool::Pencil {
            settings.hardness = 1.0;
        }
        let (cel, prefix) = self.ensure_cel();
        let before = self.drawing(cel).cloned().unwrap_or_default();
        // Pinceau texturé : l'aperçu prend ses textures, le trait gardera le réglage (7b).
        let brush = self.brushes.for_vector(&settings).map(Box::new);
        let textures = if brush.is_some() { self.brushes.textures(&settings) } else { Default::default() };
        let mut stroke = StrokeState::new(cel, StrokeMode::Paint(self.color), settings, None, pen).with_textures(textures);
        let mut dirty = Vec::new();
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            stroke.add_point(&mut c.raster, p, pressure, &mut dirty);
        }
        self.renderer.invalidate_tiles(&dirty);
        self.stroke = Some(stroke);
        self.vstroke = Some(VectorStroke {
            cel,
            before,
            prefix,
            color: self.color,
            opacity: settings.opacity,
            hardness: settings.hardness,
            brush,
        });
        self.vsel = None;
        self.modified = true;
    }

    /// Pose le trait vectoriel tracé. `preview` : l'état des tuiles avant l'aperçu dessiné pendant
    /// le geste.
    pub(crate) fn commit_vector_stroke(
        &mut self,
        vs: VectorStroke,
        path: Vec<VPoint>,
        preview: Option<Vec<(usize, Option<crate::raster::Tile>)>>,
    ) {
        let points = vector::simplify(&path, 0.25);
        if points.is_empty() {
            // Rien à garder : on efface l'aperçu.
            if let (Some(tiles), Some(c)) = (preview, self.doc.cels.get_mut(&vs.cel)) {
                let changed: Vec<usize> = tiles.iter().map(|t| t.0).collect();
                for (i, t) in tiles {
                    c.raster.tiles[i] = t;
                }
                self.renderer.invalidate_tiles(&changed);
            }
            return;
        }
        let element = Element::Stroke {
            points,
            color: vs.color,
            opacity: vs.opacity,
            hardness: vs.hardness,
            brush: vs.brush,
        };
        if let Some(d) = self.drawing_mut(vs.cel) {
            d.elements.push(element.clone());
        }
        match (preview, self.doc.cels.get_mut(&vs.cel)) {
            // Le nouveau trait est le dernier élément, donc dessus tout le reste : on remet les
            // pixels d'avant l'aperçu et on le dessine par-dessus. Même image qu'un rendu complet,
            // pour le prix du seul trait.
            (Some(tiles), Some(c)) => {
                let changed = c.render_on_top(tiles, &element);
                self.renderer.invalidate_tiles(&changed);
            }
            // Sans aperçu connu : on refait la zone du trait.
            _ => {
                let region = element.bounds().union(vector::path_bounds(&path));
                self.refresh_cel_region(vs.cel, region);
            }
        }
        self.push_vector(vs.cel, vs.before, vs.prefix);
    }

    pub(crate) fn begin_vector_erase(&mut self, p: Pos2, pen: bool) {
        let Some(cel) = self.cel_here() else { return };
        let before = self.drawing(cel).cloned().unwrap_or_default();
        self.verase = Some(VectorErase {
            cel,
            before,
            changed: false,
            last: None,
            pen,
            region: Rect::NOTHING,
        });
        self.vsel = None;
        let pressure = if pen { self.pen_pressure } else { 1.0 };
        self.erase_to(p, pressure);
    }

    pub(crate) fn erase_to(&mut self, p: Pos2, pressure: f32) {
        let s = self.eraser;
        let factor = if s.pressure_size { pressure.clamp(0.05, 1.0) } else { 1.0 };
        let radius = (s.size * 0.5 * factor).max(0.5);
        let whole = self.vector_erase_whole;
        let Some(ve) = self.verase.as_mut() else { return };
        let Some(cel) = self.doc.cels.get_mut(&ve.cel) else { return };
        let Some(d) = cel.vector.as_mut() else { return };
        // On gomme le long du segment parcouru, pas seulement aux points reçus.
        let from = ve.last.unwrap_or(p);
        let steps = ((from.distance(p) / (radius * 0.5).max(0.5)).ceil() as usize).max(1);
        // Seuls les éléments que touche le passage de la gomme peuvent changer, et ce qu'il en
        // reste tient dans leur ancien rectangle : on ne refait que cette zone.
        let swept = Rect::from_two_pos(from, p).expand(radius + 1.0);
        let region = d
            .elements
            .iter()
            .map(|e| e.bounds().expand(1.5))
            .filter(|b| b.intersects(swept))
            .fold(Rect::NOTHING, |r, b| r.union(b));
        let mut changed = false;
        for k in 1..=steps {
            let q = from + (p - from) * (k as f32 / steps as f32);
            changed |= vector::erase_circle(d, q, radius, whole);
        }
        ve.last = Some(p);
        if changed {
            ve.changed = true;
            ve.region = ve.region.union(region);
            let tiles = cel.rerender_region(region.expand(2.0));
            self.renderer.invalidate_tiles(&tiles);
        }
    }

    pub(crate) fn finish_vector_erase(&mut self) {
        let Some(ve) = self.verase.take() else { return };
        if !ve.changed {
            return;
        }
        // Les morceaux coupés sont simplifiés ; le reste du dessin n'a pas bougé.
        if let Some(d) = self.drawing_mut(ve.cel) {
            d.simplify_strokes_in(0.25, ve.region);
        }
        self.refresh_cel_region(ve.cel, ve.region);
        self.push_vector(ve.cel, ve.before, None);
    }

    pub(crate) fn erase_pen(&self) -> Option<bool> {
        self.verase.as_ref().map(|v| v.pen)
    }

    // ------------------------------------------------------------------ pot de peinture

    /// Sur un trait ou une zone déjà peinte : change sa couleur. Ailleurs : crée une zone
    /// remplie (sous les traits) à partir des contours trouvés.
    pub(crate) fn fill_vector(&mut self, p: Pos2) {
        let tol = 2.0 / self.layer_zoom();
        if let Some(cel) = self.cel_here() {
            if let Some(i) = self.drawing(cel).and_then(|d| d.hit_test(p, tol)) {
                let color = self.color;
                let Some(d) = self.drawing_mut(cel) else { return };
                let before = d.clone();
                d.elements[i].set_color(color);
                self.refresh_cel(cel);
                self.push_vector(cel, before, None);
                return;
            }
        }
        let (w, h) = (self.doc.width, self.doc.height);
        let source = self.fill_source();
        let tolerance = (self.fill.tolerance * 255.0).round() as u8;
        let region = fill::flood(&source, w, h, p.x as usize, p.y as usize, tolerance);
        let grown = fill::dilate(&region, w, h, self.fill.expand);
        let contours = vector::trace_contours(&grown, w, h);
        if contours.is_empty() {
            return;
        }
        let (cel, prefix) = self.ensure_cel();
        let color = self.color;
        let Some(d) = self.drawing_mut(cel) else { return };
        let before = d.clone();
        let at = d.fill_insert_index();
        d.elements.insert(
            at,
            Element::Fill {
                contours,
                color,
                opacity: 1.0,
            },
        );
        // L'insertion décale les éléments : une sélection garderait de mauvais indices.
        self.vsel = None;
        self.refresh_cel(cel);
        self.push_vector(cel, before, prefix);
    }

    // (Formes : voir `shapetools.rs`.)

    // ------------------------------------------------------------------ sélection

    pub(crate) fn select_press(&mut self, p: Pos2, screen: Pos2, shift: bool) {
        if !self.layer().drawable() {
            self.layer_editable();
            return;
        }
        if self.layer().kind == LayerKind::Bitmap {
            self.bitmap_select_press(p, screen, shift);
            return;
        }
        self.end_stroke();
        self.playing = false;
        let Some(cel) = self.cel_here() else {
            self.vdrag = Some(VDrag::Lasso(vec![p]));
            return;
        };
        if let Some(b) = self.selection_bounds() {
            let handle = self
                .handle_at(screen)
                .or_else(|| (!shift && b.contains(p)).then_some(Handle::Move));
            if let Some(handle) = handle {
                self.start_transform(cel, handle, p, b);
                return;
            }
        }
        let tol = 4.0 / self.layer_zoom();
        match self.drawing(cel).and_then(|d| d.hit_test(p, tol)) {
            Some(i) => {
                let sel = self.vsel.get_or_insert(VSelection { cel, items: Vec::new() });
                if shift {
                    match sel.items.iter().position(|&x| x == i) {
                        Some(k) => {
                            sel.items.remove(k);
                        }
                        None => sel.items.push(i),
                    }
                    return;
                }
                if !sel.items.contains(&i) {
                    sel.items = vec![i];
                }
                if let Some(b) = self.selection_bounds() {
                    self.start_transform(cel, Handle::Move, p, b);
                }
            }
            None => {
                if !shift {
                    self.vsel = None;
                }
                self.vdrag = Some(VDrag::Lasso(vec![p]));
            }
        }
    }

    fn start_transform(&mut self, cel: CelId, handle: Handle, p: Pos2, bbox: Rect) {
        let Some(sel) = &self.vsel else { return };
        let Some(d) = self.drawing(cel) else { return };
        let originals = sel.items.iter().filter_map(|&i| d.elements.get(i).cloned()).collect();
        let before = d.clone();
        self.vdrag = Some(VDrag::Transform {
            cel,
            handle,
            start: p,
            bbox,
            before,
            originals,
            changed: false,
        });
    }

    pub(crate) fn select_drag(&mut self, p: Pos2, shift: bool) {
        let zoom = self.layer_zoom();
        match &mut self.vdrag {
            Some(VDrag::Lasso(points)) => {
                if points.last().is_none_or(|q| q.distance(p) * zoom > 1.5) {
                    points.push(p);
                }
            }
            Some(VDrag::Transform {
                handle,
                start,
                bbox,
                originals,
                changed,
                ..
            }) => {
                let aff = transform_for(*handle, *start, p, *bbox, shift);
                *changed = true;
                let Some(sel) = &self.vsel else { return };
                let Some(cel) = self.doc.cels.get_mut(&sel.cel) else { return };
                let Some(d) = cel.vector.as_mut() else { return };
                // On ne refait que là où étaient les éléments déplacés et là où ils arrivent.
                let mut region = Rect::NOTHING;
                for (k, &i) in sel.items.iter().enumerate() {
                    if let (Some(dst), Some(src)) = (d.elements.get_mut(i), originals.get(k)) {
                        region = region.union(dst.bounds().expand(1.5));
                        *dst = src.transformed(&aff);
                        region = region.union(dst.bounds().expand(1.5));
                    }
                }
                let tiles = cel.rerender_region(region.expand(2.0));
                self.renderer.invalidate_tiles(&tiles);
            }
            None => {}
        }
    }

    pub(crate) fn finish_vdrag(&mut self, shift: bool) {
        let Some(drag) = self.vdrag.take() else { return };
        match drag {
            VDrag::Lasso(points) => {
                let Some(cel) = self.cel_here() else { return };
                let screen: Vec<Pos2> = points.iter().map(|q| self.layer_to_screen(*q)).collect();
                let extent = Rect::from_points(&screen);
                if points.len() < 3 || extent.width().max(extent.height()) < 4.0 {
                    return;
                }
                let Some(d) = self.drawing(cel) else { return };
                let hits: Vec<usize> = d
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.touches(&points))
                    .map(|(i, _)| i)
                    .collect();
                let sel = self.vsel.get_or_insert(VSelection { cel, items: Vec::new() });
                if !shift {
                    sel.items.clear();
                }
                for i in hits {
                    if !sel.items.contains(&i) {
                        sel.items.push(i);
                    }
                }
            }
            VDrag::Transform {
                cel, before, changed, ..
            } => {
                if changed {
                    self.push_vector(cel, before, None);
                }
            }
        }
    }

    /// Modifie les éléments sélectionnés, en une étape d'annulation.
    fn edit_selection(&mut self, f: impl FnOnce(&mut VectorDrawing, &mut Vec<usize>)) {
        self.end_stroke();
        let Some(sel) = self.vsel.as_mut() else { return };
        let cel = sel.cel;
        let Some(d) = self.doc.cels.get_mut(&cel).and_then(|c| c.vector.as_mut()) else { return };
        let before = d.clone();
        f(d, &mut sel.items);
        self.refresh_cel(cel);
        self.push_vector(cel, before, None);
    }

    pub(crate) fn delete_selection(&mut self) {
        self.edit_selection(|d, items| {
            let mut idx = items.clone();
            idx.sort_unstable();
            for i in idx.into_iter().rev() {
                if i < d.elements.len() {
                    d.elements.remove(i);
                }
            }
            items.clear();
        });
    }

    /// Garde seulement les éléments sélectionnés (« effacer hors de la sélection »).
    pub(crate) fn delete_unselected(&mut self) {
        self.edit_selection(|d, items| {
            let mut idx = items.clone();
            idx.sort_unstable();
            idx.dedup();
            idx.retain(|&i| i < d.elements.len());
            d.elements = idx.iter().map(|&i| d.elements[i].clone()).collect();
            *items = (0..d.elements.len()).collect();
        });
    }

    pub(crate) fn recolor_selection(&mut self) {
        let color = self.color;
        self.edit_selection(|d, items| {
            for &i in items.iter() {
                if let Some(e) = d.elements.get_mut(i) {
                    e.set_color(color);
                }
            }
        });
    }

    pub(crate) fn scale_selection_width(&mut self, k: f32) {
        self.edit_selection(|d, items| {
            for &i in items.iter() {
                if let Some(e) = d.elements.get_mut(i) {
                    e.scale_width(k);
                }
            }
        });
    }

    pub(crate) fn flip_selection(&mut self, horizontal: bool) {
        let Some(b) = self.selection_bounds() else { return };
        let aff = if horizontal {
            Affine::scale_about(b.center(), -1.0, 1.0)
        } else {
            Affine::scale_about(b.center(), 1.0, -1.0)
        };
        self.edit_selection(|d, items| {
            for &i in items.iter() {
                if let Some(e) = d.elements.get_mut(i) {
                    *e = e.transformed(&aff);
                }
            }
        });
    }

    /// Passe la sélection au premier plan (`front`) ou à l'arrière-plan du dessin.
    pub(crate) fn order_selection(&mut self, front: bool) {
        self.edit_selection(|d, items| {
            let mut idx = items.clone();
            idx.sort_unstable();
            idx.retain(|&i| i < d.elements.len());
            let mut picked: Vec<Element> = idx.iter().rev().map(|&i| d.elements.remove(i)).collect();
            picked.reverse();
            let n = picked.len();
            if front {
                let start = d.elements.len();
                d.elements.extend(picked);
                *items = (start..start + n).collect();
            } else {
                d.elements.splice(0..0, picked);
                *items = (0..n).collect();
            }
        });
    }

    pub(crate) fn select_all(&mut self) {
        if self.layer().kind != LayerKind::Vector {
            self.bitmap_select_all();
            return;
        }
        let Some(cel) = self.cel_here() else { return };
        let n = self.drawing(cel).map_or(0, |d| d.elements.len());
        if n > 0 {
            self.end_stroke();
            self.vsel = Some(VSelection {
                cel,
                items: (0..n).collect(),
            });
            self.tool = Tool::Select;
        }
    }

    pub(crate) fn copy_selection(&mut self) -> bool {
        let Some(sel) = &self.vsel else { return false };
        let Some(d) = self.drawing(sel.cel) else { return false };
        let items: Vec<Element> = sel.items.iter().filter_map(|&i| d.elements.get(i).cloned()).collect();
        self.status = format!("{} élément(s) copié(s).", items.len());
        // Un texte dans le presse-papiers du système : sans lui, Ctrl+V n'arrive pas jusqu'à nous.
        self.ctx
            .copy_text(format!("Pelure : {} élément(s) vectoriel(s)", items.len()));
        self.clipboard = items;
        true
    }

    pub(crate) fn cut_selection(&mut self) {
        if self.copy_selection() {
            self.delete_selection();
        }
    }

    pub(crate) fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        if self.layer().kind != LayerKind::Vector {
            self.status = "Le collage se fait dans un calque vectoriel.".into();
            return;
        }
        self.end_stroke();
        if !self.layer_editable() {
            return;
        }
        let (cel, prefix) = self.ensure_cel();
        let Some(d) = self.doc.cels.get_mut(&cel).and_then(|c| c.vector.as_mut()) else { return };
        let before = d.clone();
        let start = d.elements.len();
        d.elements.extend(self.clipboard.iter().cloned());
        let n = self.clipboard.len();
        self.refresh_cel(cel);
        self.push_vector(cel, before, prefix);
        self.vsel = Some(VSelection {
            cel,
            items: (start..start + n).collect(),
        });
        self.tool = Tool::Select;
    }

    /// Couleur d'aperçu des formes en cours de tracé.
    pub(crate) fn preview_color(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}
