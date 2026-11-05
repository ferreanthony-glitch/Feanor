//! Outils des calques vectoriels : trait, gomme qui coupe, pot de peinture, formes,
//! sélection au lasso avec poignées (déplacer, mettre à l'échelle, tourner), presse-papiers.

use eframe::egui::{Color32, Pos2, Rect};

use crate::app::{App, Tool};
use crate::brush::{StrokeMode, StrokeState};
use crate::document::{CelId, LayerKind};
use crate::fill;
use crate::history::Edit;
use crate::selbox::{Handle, SelFrame};
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
    /// Le réglage complet, textures désignées par leur clé de contenu (7b, étape 5).
    pub brush: Option<Box<crate::brush::BrushSettings>>,
    /// Le toucher et la couleur B du moment : la trace du trait (étape 5).
    pub feel: crate::brush::Toucher,
    pub secondary: [u8; 3],
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

pub(crate) enum VDrag {
    Lasso(Vec<Pos2>),
    /// Un geste de la boîte de sélection (`selbox.rs`) : `frame`, la boîte au début du geste ;
    /// `field` : le geste vient d'un champ chiffré du panneau Réglages, pas de la souris.
    Transform {
        cel: CelId,
        handle: Handle,
        start: Pos2,
        frame: SelFrame,
        before: VectorDrawing,
        originals: Vec<Element>,
        changed: bool,
        field: bool,
    },
    /// Le pivot tiré.
    Pivot { start: Pos2, frame: SelFrame },
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

    /// Rectangle englobant de la sélection courante (bitmap ou vectorielle) telle qu'elle est
    /// dessinée. La boîte à poignées, elle, est `sel_frame` (`seltools.rs`).
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

    // ------------------------------------------------------------------ trait et gomme

    pub(crate) fn begin_vector_stroke(&mut self, p: Pos2, pressure: f32, pen: bool) {
        let Some(mut settings) = self.current_brush() else {
            return;
        };
        settings.aliased = false;
        // « La couleur se mélange » lit les pixels : sur un calque vectoriel, elle ne vaut pas (la case est grisée).
        settings.mix_off = true;
        if self.tool == Tool::Pencil {
            settings.hardness = 1.0;
        }
        let (cel, prefix) = self.ensure_cel();
        let before = self.drawing(cel).cloned().unwrap_or_default();
        // L'aperçu prend les textures du réglage, le trait gardera le réglage complet (7b, étape 5).
        let brush = self.brushes.for_vector(&settings).map(Box::new);
        let textures = self.brushes.textures(&settings);
        // Peint sur l'aplat : l'aperçu passe déjà sous les autres traits.
        let mut stroke = StrokeState::new(cel, StrokeMode::Paint(self.color), settings, None, pen)
            .with_textures(textures)
            .with_feel(self.toucher)
            .with_zoom(self.layer_zoom())
            .with_secondary(self.couleurs.prefs.back)
            .with_options(self.paint_behind, false, 0.0)
            // Toile infinie : l'aperçu du trait suit le stylet hors de la feuille aussi.
            .on_infinite_canvas();
        let mut dirty = Vec::new();
        let time = self.input_time;
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            stroke.add_point_at(&mut c.raster, p, pressure, time, &mut dirty);
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
            feel: self.toucher,
            secondary: self.couleurs.prefs.back,
        });
        self.vsel = None;
        self.modified = true;
    }

    /// Pose le trait vectoriel tracé. `preview` : l'état des tuiles avant l'aperçu dessiné pendant
    /// le geste. `pressure` et `time` : l'appui et l'instant de chaque point de `path` (la trace,
    /// étape 5) ; avec un réglage, le chemin est gardé tel quel (le simplifier déplacerait les
    /// empreintes rejouées).
    pub(crate) fn commit_vector_stroke(
        &mut self,
        vs: VectorStroke,
        path: Vec<VPoint>,
        pressure: Vec<f32>,
        time: Vec<f64>,
        preview: Option<Vec<(usize, Option<crate::raster::Tile>)>>,
    ) {
        let replayed = vs.brush.is_some() && pressure.len() == path.len();
        let points = if replayed { path.clone() } else { vector::simplify(&path, 0.25) };
        if points.is_empty() {
            // Rien à garder : on efface l'aperçu.
            if let (Some(tiles), Some(c)) = (preview, self.doc.cels.get_mut(&vs.cel)) {
                let changed: Vec<usize> = tiles.iter().map(|t| t.0).collect();
                for (i, t) in tiles {
                    c.raster.replace_slot(i, t);
                }
                self.renderer.invalidate_tiles(&changed);
            }
            return;
        }
        let trace = if replayed {
            vector::Trace { pressure, time, feel: Some(vs.feel), secondary: Some(vs.secondary), cache: Default::default() }
        } else {
            vector::Trace::default()
        };
        let element = Element::Stroke {
            points,
            color: vs.color,
            opacity: vs.opacity,
            hardness: vs.hardness,
            brush: vs.brush,
            trace,
        };
        // (« Remplir la forme », qui posait ici une zone de la couleur du trait sous lui, n'existe plus depuis le
        // 2026-10-23 : l'aplat sous le trait du calque fait ce travail.)
        let behind = self.paint_behind && self.tool != Tool::Eraser;
        if let Some(d) = self.drawing_mut(vs.cel) {
            // Peint sur l'aplat : le trait sous tous les autres éléments du dessin.
            let at = if behind { 0 } else { d.elements.len() };
            d.elements.insert(at, element.clone());
        }
        // Un trait au réglage complet se rejoue par le moteur, empreinte par empreinte : cela
        // coûte autant que de le tracer (44 ms pour 600 points de gouache, 174 ms pour 2400), donc
        // jamais sur le fil de l'interface (règle « sans latence »). L'aperçu du geste, déjà posé
        // dans l'image du dessin, reste affiché ; le rendu exact arrive d'un fil de travail
        // (`rejeu.rs`) qui pose le trait en cache et remplace l'image.
        if replayed {
            if let Some(c) = self.doc.cels.get_mut(&vs.cel) {
                c.version += 1;
            }
            self.push_vector(vs.cel, vs.before, vs.prefix);
            self.rejeu_cel(vs.cel);
            return;
        }
        // Sous les autres : on refait la zone du trait (le raccourci ci-dessous suppose le trait seul, dessus tout le
        // reste).
        if behind {
            let region = element.bounds().union(vector::path_bounds(&path));
            self.refresh_cel_region(vs.cel, region);
            self.push_vector(vs.cel, vs.before, vs.prefix);
            return;
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
        // Les morceaux coupés ont été refaits par `erase_circle` (un trait lisse simplifié, un trait
        // rejoué avec ses appuis) ; le reste du dessin n'a pas bougé d'un pixel.
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
                gradient: None,
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
        // Un seul calque : la sélection sur plusieurs calques s'efface (`multitools.rs`).
        self.drop_multi();
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
        self.finish_field_edit();
        // La boîte de la sélection : une poignée, un bord, l'extérieur d'un coin, le pivot ou
        // l'intérieur (`selbox.rs`).
        if let Some((handle, frame)) = self.box_press(screen, shift) {
            self.start_box_gesture(cel, handle, p, frame);
            return;
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
                // La sélection vient de changer : sa boîte est recalculée avant le geste.
                self.sync_sel_box();
                if let Some(frame) = self.sel_frame() {
                    self.start_box_gesture(cel, Handle::Move, p, frame);
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

    /// Le pointeur est en `p` (point du dessin) : le lasso grandit, ou le geste de la boîte suit
    /// (`shift`, `ctrl` : les touches tenues maintenant, `selbox::gesture`).
    pub(crate) fn select_drag(&mut self, p: Pos2, shift: bool, ctrl: bool) {
        let zoom = self.layer_zoom();
        let mods = self.box_mods(shift, ctrl);
        match &mut self.vdrag {
            Some(VDrag::Lasso(points)) => {
                if points.last().is_none_or(|q| q.distance(p) * zoom > 1.5) {
                    points.push(p);
                }
            }
            Some(VDrag::Transform { handle, start, frame, field: false, .. }) => {
                let aff = crate::selbox::gesture(*handle, frame, *start, p, mods);
                self.set_vector_gesture(aff);
            }
            Some(VDrag::Pivot { start, frame }) => {
                let (start, frame) = (*start, *frame);
                self.drag_pivot(&frame, start, p);
            }
            _ => {}
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
            VDrag::Pivot { .. } => {}
            VDrag::Transform {
                cel, handle, frame, before, originals, changed, ..
            } => {
                let aff = self.flottante.as_ref().map(|f| f.aff);
                if let (true, Some(aff)) = (changed, aff) {
                    // La transformation du geste passe à la géométrie des éléments choisis, sans
                    // rien refaire : la feuille flottante reste affichée, et le rendu exact (le
                    // moteur rejoué sur leur nouvelle place) arrive d'un fil de travail quelques
                    // instants après (`poll_rejeu`), sans rien bloquer. Une étape d'annulation, au
                    // nom du geste.
                    self.commit_vector_gesture(cel, handle, frame, before, originals, aff);
                } else {
                    self.flottante = None;
                    crate::flottante::Flottante::release(&mut self.doc);
                }
            }
        }
    }

    /// Un geste lâché dont la feuille flottante attend encore le rendu exact : l'utilisateur agit
    /// (clic, touche) avant qu'il arrive, on met d'abord l'image du dessin à jour, à l'ancienne,
    /// pour que tout ce qui suit parte d'une image juste.
    pub(crate) fn flush_flottante_on_input(&mut self, ctx: &eframe::egui::Context) {
        let multi_pending = self.multi.has_pending();
        // Une zone bitmap transformée dont le rendu exact se calcule encore (aucun geste en cours) : le dessin n'a pas encore ses
        // pixels à leur place ; un clic ou une touche le mettent d'abord à jour (`flush_bitmap_float`) — sauf un clic sur la boîte
        // de la zone, qui reprend la feuille pour un nouveau geste.
        let bitmap_waiting = self.bdrag.is_none() && self.bsel.as_ref().and_then(|s| s.lifted.as_ref()).is_some_and(|l| l.stale);
        if !self.flottante.as_ref().is_some_and(|f| f.pending.is_some()) && !multi_pending && !bitmap_waiting {
            return;
        }
        use eframe::egui::{Event, Key};
        // Une enveloppe ou une perspective lâchée, dont la séance continue : reprendre un coin, ou
        // Entrée / Échap pour finir, ne touchent pas au dessin ; la feuille reste (et sert au geste
        // suivant, `App::float_for_warp`).
        let session = self.flottante.as_ref().is_some_and(|f| f.for_warp) && (self.envelope.is_some() || self.persp.is_some());
        let acts = ctx.input(|i| {
            let grabs = session && i.pointer.interact_pos().is_some_and(|p| self.envelope_grip(p).is_some() || self.persp_grip(p).is_some());
            let on_keys = |e: &Event| match e {
                Event::Key { key: Key::Enter | Key::Escape, pressed: true, .. } if session => false,
                Event::Key { pressed: true, .. } | Event::Text(_) | Event::Paste(_) | Event::Cut | Event::Copy => true,
                _ => false,
            };
            let on_box = bitmap_waiting && i.pointer.interact_pos().is_some_and(|p| self.box_hit(p).is_some());
            (i.pointer.any_pressed() && !grabs && !on_box) || i.events.iter().any(on_keys)
        });
        if acts {
            self.flush_flottante();
            self.multi_flush();
            self.flush_bitmap_float();
        }
    }

    /// Termine tout de suite le travail d'un geste lâché : les éléments reprennent leurs pixels en
    /// cache suivis, l'image du dessin est refaite là où quelque chose a changé, la feuille s'en va.
    /// (Enveloppe ou perspective : le maillage est posé sur le reste par le processeur,
    /// `Flottante::flatten_warp`.)
    pub(crate) fn flush_flottante(&mut self) {
        if !self.flottante.as_ref().is_some_and(|f| f.pending.is_some()) {
            return;
        }
        let Some(f) = self.flottante.take() else { return };
        if let Some(warp) = &f.warp {
            let tiles = crate::flottante::Flottante::flatten_warp(&mut self.doc, f.src, warp);
            self.renderer.invalidate_tiles(&tiles);
        }
        crate::flottante::Flottante::release(&mut self.doc);
        if let Some(p) = f.pending.filter(|_| !f.for_warp) {
            let tiles = crate::flottante::Flottante::apply_preview(&mut self.doc, f.src, &p.items, &p.originals, &f.aff);
            self.renderer.invalidate_tiles(&tiles);
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
        // Les éléments changent de côté : la boîte repart droite autour d'eux.
        self.sel_box = None;
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
        // Plusieurs calques choisis : tout ce que contient leur dessin de l'image courante.
        // (Ou l'aplat du calque courant, quand sa sous-ligne est choisie : ses pixels, sur une ou plusieurs clés.)
        if self.chosen_layers().len() >= 2 || self.keys_mode() || self.aplat_strate_selection() {
            self.multi_select_all();
            return;
        }
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
            .copy_text(format!("{}{} élément(s) vectoriel(s)", crate::pasteimage::MARK, items.len()));
        self.clipboard = items;
        self.collage.origines.elements = Some(crate::adaptation::Origine::de(&self.doc));
        // (La dernière copie est celle-ci : plus celle d'une sélection sur plusieurs calques.)
        self.multi_clip = None;
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
        // D'un projet d'une autre feuille : adaptés, ils gardent leur place dans le cadre (adaptation.rs).
        let items = match self.collage_preparer(crate::collage::Colle::Elements(self.clipboard.clone()), crate::collage::Reprise::Elements) {
            crate::collage::Preparation::Pret(crate::collage::Colle::Elements(e)) => e,
            _ => return,
        };
        self.end_stroke();
        if !self.layer_editable() {
            return;
        }
        let (cel, prefix) = self.ensure_cel();
        let Some(d) = self.doc.cels.get_mut(&cel).and_then(|c| c.vector.as_mut()) else { return };
        let before = d.clone();
        let start = d.elements.len();
        let n = items.len();
        d.elements.extend(items);
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
