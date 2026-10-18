//! La boîte de sélection (`selbox.rs`) dans l'application : outil Sélection sur un calque
//! vectoriel (traits choisis) ou bitmap (zone), et outil Transformer sur une zone bitmap.
//!
//! - la boîte garde son orientation tant que la sélection ne change pas (`SelBox`) : après une
//!   rotation ou une inclinaison, ses poignées restent sur les côtés du dessin tourné ;
//! - les gestes (déplacer, échelle, rotation, inclinaison) passent tous par une `Affine` que la
//!   feuille flottante pose au zéro latence (`flottante.rs`) ; au lâcher, une étape d'annulation
//!   au nom du geste (« Déplacer », « Mettre à l'échelle », « Tourner », « Incliner ») ;
//! - les champs chiffrés du panneau Réglages (position, taille, angle, inclinaison) passent par
//!   le même chemin qu'un glissé : la valeur change, la feuille flottante suit, un seul lâcher.

use eframe::egui::{self, vec2, Color32, Context, CursorIcon, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::pixtools::BDrag;
use crate::selbox::{self, BoxParams, Handle, Mods, SelFrame};
use crate::vector::{Affine, Element, VectorDrawing};
use crate::vtools::VDrag;

const ACCENT: Color32 = Color32::from_rgb(70, 165, 255);
/// À moins de tant de pixels d'écran du centre, le pivot tiré s'y remet.
const PIVOT_SNAP: f32 = 6.0;

/// Ce que la boîte garde d'une image à l'autre. Un dessin vectoriel : le rectangle d'origine des
/// éléments choisis, la transformation qui l'amène où on le voit, et les bornes des éléments
/// quand elle a été posée (`sig` : si les éléments changent autrement, la boîte repart de zéro).
/// Une zone bitmap : seulement le pivot (la transformation est celle des pixels soulevés).
pub(crate) struct SelBox {
    pub cel: CelId,
    pub rect0: Rect,
    pub m: Affine,
    pub pivot: Option<Pos2>,
    pub sig: Rect,
    pub items: Vec<usize>,
}

fn close(a: Rect, b: Rect) -> bool {
    let near = |x: f32, y: f32| (x - y).abs() < 0.01;
    near(a.min.x, b.min.x) && near(a.min.y, b.min.y) && near(a.max.x, b.max.x) && near(a.max.y, b.max.y)
}

/// Quel champ chiffré a changé.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    X,
    Y,
    Width,
    Height,
    Angle,
    Skew,
}

impl Field {
    /// Le geste que ce champ remplace : le nom de l'étape d'annulation.
    fn handle(self) -> Handle {
        match self {
            Field::X | Field::Y => Handle::Move,
            Field::Width | Field::Height => Handle::Corner(2),
            Field::Angle => Handle::Rotate(0),
            Field::Skew => Handle::Skew(0),
        }
    }
}

/// Le texte d'état d'un geste : ce que la boîte vaut maintenant.
pub(crate) fn gesture_text(handle: Handle, p: &BoxParams) -> String {
    let n = |v: f32| format!("{v:.1}").replace('.', ",");
    match handle {
        Handle::Move | Handle::Pivot => format!("Déplacer : pivot en x {:.0}, y {:.0}.", p.x, p.y),
        Handle::Corner(_) | Handle::Edge(_) => format!("Mettre à l'échelle : {:.0} × {:.0} px.", p.width, p.height),
        Handle::Rotate(_) => format!("Tourner : {}°.", n(p.angle)),
        Handle::Skew(_) => format!("Incliner : {}°.", n(p.skew)),
    }
}

impl App {
    // ------------------------------------------------------------------ la boîte

    /// Bornes des éléments `items` du dessin `cel`.
    fn vector_bounds_of(&self, cel: CelId, items: &[usize]) -> Option<Rect> {
        let d = self.drawing(cel)?;
        let r = items.iter().filter_map(|&i| d.elements.get(i)).fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
        (r.is_finite() && r.width() >= 0.0).then_some(r)
    }

    /// Une fois par image, avant d'agir : la boîte vectorielle ne vaut que pour la sélection et
    /// les éléments qu'elle a vus ; sinon elle repart du rectangle des éléments, droit.
    pub(crate) fn sync_sel_box(&mut self) {
        if let Some(sel) = &self.bsel {
            let (cel, rect0) = (sel.cel, sel.mask.bounds());
            if !self.sel_box.as_ref().is_some_and(|b| b.cel == cel && b.rect0 == rect0 && b.items.is_empty()) {
                self.sel_box = Some(SelBox { cel, rect0, m: Affine::IDENTITY, pivot: None, sig: rect0, items: Vec::new() });
            }
            return;
        }
        let Some(sel) = self.vsel.as_ref().filter(|s| !s.items.is_empty()) else {
            self.sel_box = None;
            return;
        };
        // Un geste en cours : les éléments n'ont pas bougé, la boîte non plus.
        if self.vdrag.is_some() && self.sel_box.as_ref().is_some_and(|b| b.cel == sel.cel && b.items == sel.items) {
            return;
        }
        let Some(now) = self.vector_bounds_of(sel.cel, &sel.items) else {
            self.sel_box = None;
            return;
        };
        if !self.sel_box.as_ref().is_some_and(|b| b.cel == sel.cel && b.items == sel.items && close(b.sig, now)) {
            self.sel_box = Some(SelBox { cel: sel.cel, rect0: now, m: Affine::IDENTITY, pivot: None, sig: now, items: sel.items.clone() });
        }
    }

    /// La boîte de la sélection telle qu'on la voit (le geste en cours compris), ou rien.
    pub(crate) fn sel_frame(&self) -> Option<SelFrame> {
        if let Some(sel) = &self.bsel {
            let rect0 = sel.mask.bounds();
            let pivot = self.sel_box.as_ref().filter(|b| b.cel == sel.cel && b.rect0 == rect0).and_then(|b| b.pivot);
            let m = sel.lifted.as_ref().map_or(Affine::IDENTITY, |l| l.aff);
            return Some(SelFrame { rect0, m, pivot });
        }
        let sel = self.vsel.as_ref().filter(|s| !s.items.is_empty())?;
        let mut frame = match self.sel_box.as_ref().filter(|b| b.cel == sel.cel && b.items == sel.items) {
            Some(b) => SelFrame { rect0: b.rect0, m: b.m, pivot: b.pivot },
            None => SelFrame::new(self.vector_bounds_of(sel.cel, &sel.items)?),
        };
        // Geste en cours : les éléments n'ont pas encore bougé, la boîte suit la feuille flottante.
        if let (Some(f), Some(VDrag::Transform { changed: true, .. })) = (&self.flottante, &self.vdrag) {
            if f.pending.is_none() {
                frame = frame.then(&f.aff);
            }
        }
        Some(frame)
    }

    /// Les coins de la boîte à l'écran et son pivot.
    fn frame_on_screen(&self, frame: &SelFrame) -> ([Pos2; 4], Pos2) {
        (frame.corners().map(|q| self.layer_to_screen(q)), self.layer_to_screen(frame.pivot_world()))
    }

    /// La partie de la boîte sous le pointeur (position écran).
    pub(crate) fn box_hit(&self, screen: Pos2) -> Option<Handle> {
        let frame = self.sel_frame()?;
        let (quad, pivot) = self.frame_on_screen(&frame);
        selbox::hit_test(quad, pivot, screen, &selbox::Grabs::default())
    }

    /// Comme `box_hit`, sans l'intérieur : une poignée, un bord, l'extérieur d'un coin ou le pivot.
    pub(crate) fn handle_at(&self, screen: Pos2) -> Option<Handle> {
        self.box_hit(screen).filter(|h| *h != Handle::Move)
    }

    /// Ce que prend un appui en `screen` : la partie de la boîte et la boîte. Maj + appui dedans
    /// n'est pas un déplacement (il ajoute au choix).
    pub(crate) fn box_press(&self, screen: Pos2, shift: bool) -> Option<(Handle, SelFrame)> {
        let frame = self.sel_frame()?;
        let hit = self.box_hit(screen)?;
        (!(hit == Handle::Move && shift)).then_some((hit, frame))
    }

    /// Touches et réglage du geste.
    pub(crate) fn box_mods(&self, shift: bool, ctrl: bool) -> Mods {
        Mods { shift, ctrl, keep: self.keep_proportions }
    }

    /// La partie de la boîte tenue en ce moment (curseur, poignée éclairée).
    fn active_handle(&self) -> Option<Handle> {
        match (&self.vdrag, &self.bdrag) {
            (Some(VDrag::Transform { handle, field: false, .. }), _) | (_, Some(BDrag::Transform { handle, field: false, .. })) => Some(*handle),
            (Some(VDrag::Pivot { .. }), _) | (_, Some(BDrag::Pivot { .. })) => Some(Handle::Pivot),
            _ => None,
        }
    }

    /// La partie tenue, ou à défaut celle qui est sous le pointeur.
    pub(crate) fn active_or_hit_handle(&self, screen: Pos2) -> Option<Handle> {
        self.active_handle().or_else(|| self.box_hit(screen))
    }

    /// Le pivot tiré en `p` (depuis `start`, la boîte étant `frame`).
    pub(crate) fn drag_pivot(&mut self, frame: &SelFrame, start: Pos2, p: Pos2) {
        let zoom = self.layer_zoom();
        let local = selbox::move_pivot(frame, start, p, PIVOT_SNAP, zoom);
        if let Some(b) = &mut self.sel_box {
            b.pivot = local;
        }
    }

    /// « Réinitialiser le pivot » : il revient au centre de la boîte.
    pub(crate) fn reset_pivot(&mut self) {
        if let Some(b) = &mut self.sel_box {
            b.pivot = None;
        }
        self.status = "Pivot remis au centre de la boîte.".into();
    }

    /// Après un geste lâché ou une flèche : la boîte (vectorielle) devient `frame`, et les éléments
    /// `items` ont maintenant leurs bornes d'après.
    pub(crate) fn set_sel_box(&mut self, cel: CelId, items: &[usize], frame: SelFrame) {
        if self.bsel.is_some() {
            return;
        }
        let sig = self.vector_bounds_of(cel, items).unwrap_or(frame.rect0);
        self.sel_box = Some(SelBox { cel, rect0: frame.rect0, m: frame.m, pivot: frame.pivot, sig, items: items.to_vec() });
    }

    /// La boîte vectorielle déplacée de `aff` sans autre changement (flèches).
    pub(crate) fn move_sel_box(&mut self, aff: &Affine) {
        let Some(frame) = self.sel_frame().filter(|_| self.bsel.is_none()) else { return };
        let Some(sel) = &self.vsel else { return };
        let (cel, items) = (sel.cel, sel.items.clone());
        self.set_sel_box(cel, &items, frame.then(aff));
    }

    /// Enregistre ce que fait `f` sous le nom `name` (panneau Historique).
    pub(crate) fn named_step(&mut self, name: &str, f: impl FnOnce(&mut Self)) {
        let before = self.history.context.replace(name.to_string());
        f(self);
        self.history.context = before;
    }

    // ------------------------------------------------------------------ le geste vectoriel

    /// Un appui sur une partie de la boîte vectorielle : le geste commence.
    pub(crate) fn start_box_gesture(&mut self, cel: CelId, handle: Handle, p: Pos2, frame: SelFrame) {
        self.finish_field_edit();
        if handle == Handle::Pivot {
            self.vdrag = Some(VDrag::Pivot { start: p, frame });
            return;
        }
        let Some(sel) = &self.vsel else { return };
        let Some(d) = self.drawing(cel) else { return };
        let originals = sel.items.iter().filter_map(|&i| d.elements.get(i).cloned()).collect();
        let before = d.clone();
        self.vdrag = Some(VDrag::Transform { cel, handle, start: p, frame, before, originals, changed: false, field: false });
    }

    /// La transformation `aff` du geste vectoriel en cours : à la première image, la feuille
    /// flottante (le dessin sans la sélection, et la sélection seule, faits une fois) ; ensuite
    /// seule la transformation change (rien à refaire, rien à renvoyer à la carte graphique).
    pub(crate) fn set_vector_gesture(&mut self, aff: Affine) {
        let Some(VDrag::Transform { cel, handle, frame, changed, .. }) = &mut self.vdrag else { return };
        let (cel, handle, frame, first) = (*cel, *handle, *frame, !*changed);
        *changed = true;
        if first {
            let items = self.vsel.as_ref().map(|s| s.items.clone()).unwrap_or_default();
            self.flottante = crate::flottante::Flottante::lift(&mut self.doc, cel, &items);
        }
        if let Some(f) = self.flottante.as_mut() {
            f.aff = aff;
        }
        self.status = gesture_text(handle, &frame.then(&aff).params());
    }

    /// Le geste vectoriel est lâché : la géométrie des éléments reçoit la transformation, le rendu
    /// exact arrive d'un fil de travail, une étape d'annulation au nom du geste.
    pub(crate) fn commit_vector_gesture(&mut self, cel: CelId, handle: Handle, frame: SelFrame, before: VectorDrawing, originals: Vec<Element>, aff: Affine) {
        let items = self.vsel.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        crate::flottante::Flottante::apply_geometry(&mut self.doc, cel, &items, &originals, &aff);
        self.set_sel_box(cel, &items, frame.then(&aff));
        if let Some(f) = self.flottante.as_mut() {
            f.pending = Some(crate::flottante::Pending { items, originals });
        }
        self.rejeu_cel(cel);
        self.named_step(handle.step_name(), |app| app.push_vector(cel, before, None));
    }

    // ------------------------------------------------------------------ les champs chiffrés

    /// Un champ chiffré a changé : la boîte devient celle de `params`. Le premier changement
    /// ouvre un geste (comme un appui sur une poignée), les suivants le prolongent ; il se ferme
    /// quand plus aucun champ n'a la main (`finish_field_edit_if_idle`).
    fn apply_box_params(&mut self, live: SelFrame, params: BoxParams, field: Field) {
        let handle = field.handle();
        self.field_edit_alive = true;
        // Le geste en cours est-il celui des champs ?
        let open = matches!(&self.vdrag, Some(VDrag::Transform { field: true, .. })) || matches!(&self.bdrag, Some(BDrag::Transform { field: true, .. }));
        if !open {
            if !self.layer_editable() {
                return;
            }
            if self.bsel.is_some() {
                if !self.lift_selection() {
                    return;
                }
                let frame = self.sel_frame().unwrap_or(live);
                self.note_bitmap_step(handle);
                self.bdrag = Some(BDrag::Transform { handle, start: Pos2::ZERO, frame, field: true });
            } else {
                // Un geste lâché dont la feuille attend encore son rendu : on le termine d'abord.
                self.flush_flottante();
                let Some(cel) = self.vsel.as_ref().map(|s| s.cel) else { return };
                self.start_box_gesture(cel, handle, Pos2::ZERO, live);
                if let Some(VDrag::Transform { field, .. }) = &mut self.vdrag {
                    *field = true;
                }
            }
        }
        let frame0 = match (&self.vdrag, &self.bdrag) {
            (Some(VDrag::Transform { frame, field: true, .. }), _) | (_, Some(BDrag::Transform { frame, field: true, .. })) => *frame,
            _ => return,
        };
        // Les valeurs sont celles de la boîte vue ; la boîte de départ du geste s'y ramène.
        let Some(target) = live.with_params(&params) else { return };
        if self.bsel.is_some() {
            self.place_floating(target.m);
            self.status = gesture_text(handle, &params);
        } else if let Some(d) = frame0.delta_to(&target) {
            self.set_vector_gesture(d);
        }
    }

    /// Le geste des champs est fini : les éléments reçoivent la transformation (une étape
    /// d'annulation), ou la zone bitmap garde sa place.
    pub(crate) fn finish_field_edit(&mut self) {
        if matches!(&self.vdrag, Some(VDrag::Transform { field: true, .. })) {
            self.finish_vdrag(false);
        }
        if matches!(&self.bdrag, Some(BDrag::Transform { field: true, .. })) {
            self.bdrag = None;
        }
        self.field_edit_alive = false;
    }

    /// Début d'image : un champ qui n'a plus la main termine son geste.
    pub(crate) fn finish_field_edit_if_idle(&mut self) {
        let open = matches!(&self.vdrag, Some(VDrag::Transform { field: true, .. })) || matches!(&self.bdrag, Some(BDrag::Transform { field: true, .. }));
        if open && !std::mem::take(&mut self.field_edit_alive) {
            self.finish_field_edit();
        }
    }

    /// Les champs de la boîte, dans Réglages : position du pivot, taille, angle, inclinaison, les
    /// proportions et le pivot. Compact : trois lignes.
    pub(crate) fn box_fields_ui(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let Some(live) = self.sel_frame() else { return };
        let p0 = live.params();
        let mut p = p0;
        let mut touched: Option<Field> = None;
        let mut alive = false;
        let mut field = |ui: &mut egui::Ui, v: &mut f32, which: Field, prefix: &str, suffix: &str, speed: f64, range: std::ops::RangeInclusive<f32>, tip: (&str, &str)| {
            let r = ui.add(egui::DragValue::new(v).speed(speed).range(range).prefix(prefix).suffix(suffix).max_decimals(1));
            let changed = r.changed();
            alive |= r.has_focus() || r.dragged();
            crate::bulles::bulle(r, tip.0, tip.1, "");
            if changed {
                touched = Some(which);
            }
        };
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            field(ui, &mut p.x, Field::X, "x ", " px", 1.0, -20000.0..=20000.0, ("Position x", "Où est le pivot de la sélection (par défaut son centre), en pixels du dessin."));
            field(ui, &mut p.y, Field::Y, "y ", " px", 1.0, -20000.0..=20000.0, ("Position y", "Où est le pivot de la sélection (par défaut son centre), en pixels du dessin."));
        });
        ui.horizontal_wrapped(|ui| {
            field(ui, &mut p.width, Field::Width, "L ", " px", 1.0, 1.0..=20000.0, ("Largeur", "Largeur de la boîte, le long de son bord du haut. Le pivot ne bouge pas."));
            field(ui, &mut p.height, Field::Height, "H ", " px", 1.0, 1.0..=20000.0, ("Hauteur", "Hauteur de la boîte, à angle droit de son bord du haut. Le pivot ne bouge pas."));
            let link = if self.keep_proportions { ph::LINK } else { ph::LINK_BREAK };
            crate::bulles::interrupteur(
                ui,
                &mut self.keep_proportions,
                link,
                "Garder les proportions",
                "Mettre à l'échelle en gardant la forme : par une poignée comme par la largeur et la hauteur. Maj, en tirant, fait l'inverse.",
                "",
            );
        });
        ui.horizontal_wrapped(|ui| {
            field(ui, &mut p.angle, Field::Angle, "", "°", 0.5, -180.0..=180.0, ("Angle", "Rotation de la boîte autour du pivot, en degrés."));
            field(ui, &mut p.skew, Field::Skew, "inclin. ", "°", 0.5, -selbox::SKEW_MAX..=selbox::SKEW_MAX, ("Inclinaison", "Cisaillement le long du bord du haut, en degrés."));
            let moved = self.sel_box.as_ref().is_some_and(|b| b.pivot.is_some());
            if crate::bulles::icone_si(ui, moved, ph::CROSSHAIR, "Réinitialiser le pivot", "Le pivot revient au centre de la boîte.", "").clicked() {
                self.reset_pivot();
            }
        });
        if alive {
            self.field_edit_alive = true;
        }
        let Some(which) = touched else { return };
        // Garder les proportions : la largeur et la hauteur changent ensemble.
        if self.keep_proportions {
            match which {
                Field::Width if p0.width > 1e-3 => p.height = p0.height * p.width / p0.width,
                Field::Height if p0.height > 1e-3 => p.width = p0.width * p.height / p0.height,
                _ => {}
            }
        }
        self.apply_box_params(live, p, which);
    }

    // ------------------------------------------------------------------ l'écran

    /// Vrai quand la boîte répond à la souris (outils de sélection, ou Transformer sur une zone).
    pub(crate) fn box_active(&self) -> bool {
        self.tool.is_select() || (self.tool == Tool::Transform && self.bsel.is_some())
    }

    /// Dessine la boîte : le cadre, les huit poignées carrées et le pivot rond.
    pub(crate) fn paint_selection_box(&self, painter: &Painter) {
        let Some(frame) = self.sel_frame() else { return };
        let (quad, pivot) = self.frame_on_screen(&frame);
        painter.add(Shape::closed_line(quad.to_vec(), Stroke::new(1.0_f32, ACCENT)));
        if !self.box_active() {
            return;
        }
        let hover = painter
            .ctx()
            .pointer_hover_pos()
            .filter(|p| painter.clip_rect().contains(*p) || self.active_handle().is_some())
            .and_then(|p| self.active_handle().or_else(|| self.box_hit(p)));
        let lit = |h: Handle| hover == Some(h);
        let mids = [0, 1, 2, 3].map(|k| quad[k] + (quad[(k + 1) % 4] - quad[k]) * 0.5);
        for (k, c) in quad.iter().enumerate() {
            let fill = if lit(Handle::Corner(k)) { ACCENT } else { Color32::WHITE };
            painter.rect(selbox::handle_rect(*c, 8.0), 1.0, fill, Stroke::new(1.5_f32, ACCENT), StrokeKind::Middle);
        }
        for (k, c) in mids.iter().enumerate() {
            let fill = if lit(Handle::Edge(k)) { ACCENT } else { Color32::WHITE };
            painter.rect(selbox::handle_rect(*c, 7.0), 1.0, fill, Stroke::new(1.5_f32, ACCENT), StrokeKind::Middle);
        }
        // Le bord survolé (incliner) s'épaissit.
        if let Some(Handle::Skew(k)) = hover {
            painter.line_segment([quad[k], quad[(k + 1) % 4]], Stroke::new(2.5_f32, ACCENT));
        }
        let fill = if lit(Handle::Pivot) { ACCENT } else { Color32::WHITE };
        painter.circle(pivot, 4.5, fill, Stroke::new(1.5_f32, ACCENT));
        painter.circle_filled(pivot, 1.5, if lit(Handle::Pivot) { Color32::WHITE } else { ACCENT });
    }

    /// Le curseur au-dessus de la boîte ; faux si la souris n'est sur aucune de ses parties.
    /// Redimensionner : les flèches du système, selon la direction de la poignée à l'écran (la
    /// boîte peut être tournée) ; tourner et incliner : un petit dessin, le système n'a pas ces
    /// curseurs.
    pub(crate) fn box_cursor(&self, ctx: &Context, painter: &Painter, screen: Pos2) -> bool {
        let Some(frame) = self.sel_frame() else { return false };
        let (quad, _) = self.frame_on_screen(&frame);
        let handle = self.active_handle().or_else(|| self.box_hit(screen));
        let Some(handle) = handle else { return false };
        let resize = |v: Option<Vec2>| match v.map(selbox::resize_cursor_axis) {
            Some(0) => CursorIcon::ResizeHorizontal,
            Some(1) => CursorIcon::ResizeNwSe,
            Some(2) => CursorIcon::ResizeVertical,
            Some(_) => CursorIcon::ResizeNeSw,
            None => CursorIcon::Default,
        };
        match handle {
            Handle::Move => ctx.set_cursor_icon(CursorIcon::Move),
            Handle::Pivot => ctx.set_cursor_icon(CursorIcon::Crosshair),
            Handle::Corner(_) | Handle::Edge(_) => ctx.set_cursor_icon(resize(selbox::handle_direction(quad, handle))),
            Handle::Skew(k) => {
                ctx.set_cursor_icon(CursorIcon::None);
                let dir = (quad[(k + 1) % 4] - quad[k]).normalized();
                paint_double_arrow(painter, screen, if dir.is_finite() { dir } else { vec2(1.0, 0.0) });
            }
            Handle::Rotate(_) => {
                ctx.set_cursor_icon(CursorIcon::None);
                paint_rotate_glyph(painter, screen);
            }
        }
        true
    }
}

/// Un trait noir sous un trait blanc : lisible sur tous les fonds.
fn outlined(painter: &Painter, points: Vec<Pos2>, width: f32) {
    painter.add(Shape::line(points.clone(), Stroke::new(width + 2.0, Color32::from_black_alpha(200))));
    painter.add(Shape::line(points, Stroke::new(width, Color32::WHITE)));
}

/// Une pointe de flèche en `tip`, dirigée selon `dir`.
fn arrow_head(painter: &Painter, tip: Pos2, dir: Vec2) {
    let side = vec2(-dir.y, dir.x);
    let back = tip - dir * 6.0;
    let tri = vec![tip + dir * 1.0, back + side * 3.5, back - side * 3.5];
    painter.add(Shape::convex_polygon(tri.clone(), Color32::from_black_alpha(200), Stroke::new(1.5_f32, Color32::from_black_alpha(200))));
    painter.add(Shape::convex_polygon(tri, Color32::WHITE, Stroke::NONE));
}

/// Curseur d'inclinaison : une double flèche parallèle au bord.
fn paint_double_arrow(painter: &Painter, at: Pos2, dir: Vec2) {
    let half = 10.0;
    outlined(painter, vec![at - dir * (half - 3.0), at + dir * (half - 3.0)], 1.5);
    arrow_head(painter, at + dir * half, dir);
    arrow_head(painter, at - dir * half, -dir);
}

/// Curseur de rotation : un arc fléché.
fn paint_rotate_glyph(painter: &Painter, at: Pos2) {
    let r = 8.0;
    let (a0, a1) = (-2.4_f32, 2.3_f32);
    let arc: Vec<Pos2> = (0..=18).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / 18.0;
        at + vec2(a.cos(), a.sin()) * r
    }).collect();
    outlined(painter, arc, 1.5);
    // La pointe au bout de l'arc, dans le sens du tracé.
    let tip = at + vec2(a1.cos(), a1.sin()) * r;
    let dir = vec2(-a1.sin(), a1.cos());
    arrow_head(painter, tip + dir * 3.0, dir);
}
