//! Sélection sur plusieurs calques (Anthony, 2026-10-19 : « Ajoute la possibilité de sélectionner
//! deux calques pour ensuite sélectionner les deux dessins de chaque calque pour les transformer,
//! les déplacer »).
//!
//! Avec plusieurs calques choisis (Ctrl ou Maj + clic dans le panneau Calques, la timeline ou le
//! montage), le lasso de l'outil Sélection prend, sur chacun, ce qui est dans le lasso : les traits
//! d'un calque vectoriel, la zone d'un calque bitmap. La boîte de sélection (`selbox.rs`) entoure
//! le tout, et chaque geste (déplacer, échelle, rotation, inclinaison, champs chiffrés, flèches,
//! miroir) s'applique à toutes les parties autour du même pivot.
//!
//! Tout se passe sur la feuille (le repère commun des calques) : la boîte et le geste y sont
//! faits, puis ramenés dans le repère de chaque calque par sa propre transformation, `D = T⁻¹·A·T`
//! (`layer_delta`) : un calque décalé, tourné ou en profondeur suit le geste là où on le voit.
//!
//! Ce fichier ne connaît ni la souris ni l'écran : il travaille sur le document, pour qu'on puisse
//! tout vérifier sans interface (`multisel_tests.rs`). `multitools.rs` y branche l'application.
//!
//! - **Sans latence** : pendant un geste, chaque partie vectorielle a sa feuille flottante
//!   (`flottante.rs`, une place par calque) : le dessin sans la sélection, et la sélection seule
//!   avec la transformation du geste (la carte graphique n'a qu'un paramètre à changer par
//!   calque) ; jamais de rejeu du moteur des brosses. Une zone bitmap soulève ses pixels une fois,
//!   puis les repose à chaque image (comme `pixtools.rs`). Au lâcher, la géométrie des traits
//!   reçoit la transformation et le rendu exact vient d'un fil de travail.
//! - **Annulation** : un geste sur N calques est une seule étape (`Edit::Group`), au nom du geste.
//! - Les calques cachés, verrouillés, sans dessin à l'image courante, les dossiers et les calques
//!   vidéo sont laissés de côté (`targets`), avec la raison.

use eframe::egui::{pos2, Pos2, Rect};

use crate::document::{CelId, Document, LayerKind};
use crate::flottante::{Flottante, Pending};
use crate::history::Edit;
use crate::pixsel::{self, Floating, PixelMask};
use crate::raster::{Raster, Tile, TILE};
use crate::selbox::{Handle, SelFrame};
use crate::vector::{self, Affine, Element, VectorDrawing};

/// Un calque que la sélection peut toucher à l'image courante.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub id: u64,
    pub name: String,
    pub cel: CelId,
    pub vector: bool,
    /// Le nombre d'éléments de son dessin vectoriel.
    pub elements: usize,
    /// Du dessin du calque vers la feuille, tel qu'on le voit à cette image.
    pub to_stage: Affine,
}

/// Les calques utilisables, et ceux qu'on laisse de côté (nom, raison).
#[derive(Default)]
pub(crate) struct Targets {
    pub list: Vec<Target>,
    pub skipped: Vec<(String, &'static str)>,
}

impl Targets {
    /// « Calques laissés de côté : Encre (verrouillé), Fond (masqué). »
    pub fn skipped_text(&self) -> Option<String> {
        if self.skipped.is_empty() {
            return None;
        }
        let list: Vec<String> = self.skipped.iter().map(|(n, why)| format!("{n} ({why})")).collect();
        Some(format!("Laissé{} de côté : {}.", if list.len() > 1 { "s" } else { "" }, list.join(", ")))
    }
}

/// Transformation du calque `li` du plan `panel` à l'image `frame`, de son dessin vers la feuille :
/// la sienne, plus le défilement multiplan si le calque est en profondeur (`Layer::transform_seen`).
/// Rien si le calque est derrière la caméra.
pub(crate) fn layer_to_stage(doc: &Document, panel: usize, frame: usize, li: usize) -> Option<Affine> {
    let layer = doc.panels.get(panel)?.layers.get(li)?;
    let seen = doc.camera_seen(panel, frame);
    Some(layer.transform_seen(&seen)?.affine(doc.stage_center()))
}

/// Les calques `chosen` (rangs) que la sélection peut toucher. Un dossier n'a pas de dessin : ses
/// calques comptent seulement s'ils sont choisis eux-mêmes.
pub(crate) fn targets(doc: &Document, panel: usize, frame: usize, chosen: &[usize]) -> Targets {
    let mut out = Targets::default();
    let Some(p) = doc.panels.get(panel) else { return out };
    let layers = &p.layers;
    for &li in chosen {
        let Some(l) = layers.get(li) else { continue };
        if l.folder {
            continue;
        }
        let mut skip = |why: &'static str| out.skipped.push((l.name.clone(), why));
        if l.video.is_some() {
            skip("vidéo");
        } else if crate::folders::effect(layers, li).is_none() {
            skip("masqué");
        } else if l.locked || crate::folders::locked_by_folder(layers, li) || l.position_locked {
            skip("verrouillé");
        } else {
            let Some(cel) = l.cel_at(frame).filter(|c| doc.cels.contains_key(c)) else {
                skip("sans dessin à cette image");
                continue;
            };
            let Some(to_stage) = layer_to_stage(doc, panel, frame, li) else {
                skip("derrière la caméra");
                continue;
            };
            let vector = l.kind == LayerKind::Vector && doc.cels[&cel].vector.is_some();
            let elements = doc.cels[&cel].vector.as_ref().map_or(0, |d| d.elements.len());
            out.list.push(Target { id: l.id, name: l.name.clone(), cel, vector, elements, to_stage });
        }
    }
    out
}

/// Les pixels soulevés d'une zone bitmap : l'image d'origine garde ce qu'on a soulevé (chaque
/// transformation repart d'elle, tourner deux fois ne floute pas deux fois), `base` est le dessin
/// sans eux, `aff` ce qui les a placés où on les voit (dans le repère du dessin).
pub(crate) struct Lift {
    pub floating: Floating,
    pub base: Raster,
    pub aff: Affine,
    /// Zone où les pixels sont posés en ce moment.
    pub placed: Option<Rect>,
}

pub(crate) enum PartKind {
    /// Des traits d'un dessin vectoriel (rangs des éléments).
    Vector { items: Vec<usize> },
    /// Une zone d'un dessin bitmap : son masque et ses contours (repère du dessin, à l'origine).
    Bitmap { mask: PixelMask, outline: Vec<Vec<Pos2>>, lift: Option<Lift> },
}

/// La part d'un calque dans la sélection.
pub(crate) struct Part {
    pub layer: u64,
    pub name: String,
    pub cel: CelId,
    /// Le nombre d'éléments du dessin vectoriel quand les traits ont été pris : s'il change (un
    /// trait dessiné, une gomme), les rangs choisis ne valent plus et la sélection s'en va.
    pub elements: usize,
    pub to_stage: Affine,
    pub kind: PartKind,
}

impl Part {
    /// Le nombre d'éléments : les traits choisis, ou 1 pour une zone.
    pub fn count(&self) -> usize {
        match &self.kind {
            PartKind::Vector { items } => items.len(),
            PartKind::Bitmap { .. } => 1,
        }
    }

    /// La part, telle qu'on la voit, dans le repère du dessin.
    pub fn layer_bounds(&self, doc: &Document) -> Option<Rect> {
        match &self.kind {
            PartKind::Vector { items } => {
                let d = doc.cels.get(&self.cel)?.vector.as_ref()?;
                let r = items.iter().filter_map(|&i| d.elements.get(i)).fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
                (r.is_finite() && r.width() >= 0.0).then_some(r)
            }
            PartKind::Bitmap { mask, lift, .. } => {
                let b = mask.bounds();
                Some(lift.as_ref().map_or(b, |l| l.aff.apply_rect(b)))
            }
        }
    }

    /// La part, telle qu'on la voit, sur la feuille.
    pub fn stage_bounds(&self, doc: &Document) -> Option<Rect> {
        self.layer_bounds(doc).map(|b| self.to_stage.apply_rect(b))
    }
}

/// La boîte gardée d'une image à l'autre (comme `SelBox`) : le cadre, tel que le dernier geste l'a
/// laissé, et les bornes de la sélection à ce moment ; si elles changent autrement, la boîte repart
/// droite.
pub(crate) struct BoxState {
    pub frame: SelFrame,
    pub sig: Rect,
}

fn close(a: Rect, b: Rect) -> bool {
    let near = |x: f32, y: f32| (x - y).abs() < 0.01;
    near(a.min.x, b.min.x) && near(a.min.y, b.min.y) && near(a.max.x, b.max.x) && near(a.max.y, b.max.y)
}

/// Ce qui est choisi sur les calques choisis, au plan et à l'image où on l'a pris.
pub(crate) struct MultiSel {
    pub panel: usize,
    pub frame: usize,
    pub parts: Vec<Part>,
    pub boxed: Option<BoxState>,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}

impl MultiSel {
    pub fn new(panel: usize, frame: usize, parts: Vec<Part>) -> MultiSel {
        MultiSel { panel, frame, parts, boxed: None }
    }

    /// « 3 calques, 42 éléments » (et « 2 zones » pour les calques bitmap).
    pub fn summary(&self) -> String {
        let elements: usize = self.parts.iter().filter(|p| matches!(p.kind, PartKind::Vector { .. })).map(Part::count).sum();
        let zones = self.parts.iter().filter(|p| matches!(p.kind, PartKind::Bitmap { .. })).count();
        let mut s = plural(self.parts.len(), "calque", "calques");
        if elements > 0 || zones == 0 {
            s += &format!(", {}", plural(elements, "élément", "éléments"));
        }
        if zones > 0 {
            s += &format!("{} {}", if elements > 0 { " et" } else { "," }, plural(zones, "zone", "zones"));
        }
        s
    }

    pub fn has_bitmap(&self) -> bool {
        self.parts.iter().any(|p| matches!(p.kind, PartKind::Bitmap { .. }))
    }

    pub fn has_layer(&self, id: u64) -> bool {
        self.parts.iter().any(|p| p.layer == id)
    }

    /// « Encre (3), Couleur (2) » : où est la sélection, calque par calque.
    pub fn per_layer(&self) -> String {
        self.parts.iter().map(|p| format!("{} ({})", p.name, p.count())).collect::<Vec<_>>().join(", ")
    }

    pub fn has_cel(&self, cel: CelId) -> bool {
        self.parts.iter().any(|p| p.cel == cel)
    }

    /// Les bornes de toute la sélection, sur la feuille.
    pub fn stage_bounds(&self, doc: &Document) -> Option<Rect> {
        let r = self.parts.iter().filter_map(|p| p.stage_bounds(doc)).fold(Rect::NOTHING, |r, b| r.union(b));
        (r.is_finite() && r.width() >= 0.0).then_some(r)
    }

    /// La boîte, telle que les gestes l'ont laissée, ou droite autour de la sélection.
    pub fn base_frame(&self, doc: &Document) -> Option<SelFrame> {
        let now = self.stage_bounds(doc)?;
        Some(match &self.boxed {
            Some(b) if close(b.sig, now) => b.frame,
            _ => SelFrame::new(now),
        })
    }

    /// Garde `frame` comme la boîte de la sélection telle qu'elle est maintenant.
    pub fn keep_frame(&mut self, doc: &Document, frame: SelFrame) {
        self.boxed = self.stage_bounds(doc).map(|sig| BoxState { frame, sig });
    }

    /// Remet à jour le passage des dessins à la feuille (un calque a pu bouger entre-temps).
    pub fn refresh_maps(&mut self, doc: &Document) {
        for p in &mut self.parts {
            let li = doc.panels.get(self.panel).and_then(|pn| pn.layers.iter().position(|l| l.id == p.layer));
            if let Some(m) = li.and_then(|li| layer_to_stage(doc, self.panel, self.frame, li)) {
                p.to_stage = m;
            }
        }
    }

    /// Ajoute `extra` à la sélection (Maj) : les traits d'un même calque se réunissent, les zones
    /// aussi (tant que les pixels de la première ne sont pas soulevés).
    pub fn merge(&mut self, extra: Vec<Part>) {
        for new in extra {
            let Some(old) = self.parts.iter_mut().find(|p| p.layer == new.layer && p.cel == new.cel) else {
                self.parts.push(new);
                continue;
            };
            match (&mut old.kind, new.kind) {
                (PartKind::Vector { items }, PartKind::Vector { items: more }) => {
                    for i in more {
                        if !items.contains(&i) {
                            items.push(i);
                        }
                    }
                }
                (PartKind::Bitmap { mask, outline, lift: None }, PartKind::Bitmap { mask: more, outline: lines, .. }) => {
                    *mask = mask.union(&more);
                    outline.extend(lines);
                }
                _ => {}
            }
        }
        self.boxed = None;
    }

    /// Le trait `item` du calque `target` : retiré s'il était choisi, ajouté sinon (Maj + clic).
    pub fn toggle_item(&mut self, target: &Target, item: usize) {
        if let Some(k) = self.parts.iter().position(|p| p.layer == target.id && p.cel == target.cel) {
            if let PartKind::Vector { items } = &mut self.parts[k].kind {
                match items.iter().position(|&x| x == item) {
                    Some(at) => {
                        items.remove(at);
                    }
                    None => items.push(item),
                }
                if items.is_empty() {
                    self.parts.remove(k);
                }
            }
        } else {
            self.parts.push(vector_part(target, vec![item]));
        }
        self.boxed = None;
    }

    /// Cette sélection est-elle encore la bonne ? Même plan et même image, et les dessins
    /// choisis existent encore avec leurs éléments.
    pub fn still_valid(&self, doc: &Document, panel: usize, frame: usize) -> bool {
        if (self.panel, self.frame) != (panel, frame) || self.parts.is_empty() {
            return false;
        }
        let Some(p) = doc.panels.get(panel) else { return false };
        self.parts.iter().all(|part| {
            let Some(layer) = p.layers.iter().find(|l| l.id == part.layer) else { return false };
            if layer.cel_at(frame) != Some(part.cel) {
                return false;
            }
            match (&part.kind, doc.cels.get(&part.cel)) {
                (PartKind::Vector { items }, Some(c)) => {
                    let n = c.vector.as_ref().map_or(0, |d| d.elements.len());
                    n == part.elements && !items.is_empty() && items.iter().all(|&i| i < n)
                }
                (PartKind::Bitmap { .. }, Some(c)) => c.vector.is_none(),
                _ => false,
            }
        })
    }
}

pub(crate) fn vector_part(t: &Target, items: Vec<usize>) -> Part {
    Part { layer: t.id, name: t.name.clone(), cel: t.cel, elements: t.elements, to_stage: t.to_stage, kind: PartKind::Vector { items } }
}

/// La part bitmap d'un calque : sa zone (`outline`, `mask`), pixels pas encore soulevés.
fn bitmap_part(t: &Target, mask: PixelMask, outline: Vec<Vec<Pos2>>, lift: Option<Lift>) -> Part {
    Part { layer: t.id, name: t.name.clone(), cel: t.cel, elements: 0, to_stage: t.to_stage, kind: PartKind::Bitmap { mask, outline, lift } }
}

/// Ce que fait un geste de la boîte sur un calque : la transformation `a` (sur la feuille) vue
/// depuis le dessin de ce calque, `T⁻¹ · A · T` (`T` : du dessin à la feuille).
pub(crate) fn layer_delta(to_stage: &Affine, a: &Affine) -> Option<Affine> {
    Some(to_stage.then(a).then(&to_stage.inverse()?))
}

/// Un polygone de la feuille, ramené dans le repère du dessin d'un calque.
pub(crate) fn polygon_in_layer(to_stage: &Affine, poly: &[Pos2]) -> Option<Vec<Pos2>> {
    let inv = to_stage.inverse()?;
    Some(poly.iter().map(|&p| inv.apply(p)).collect())
}

/// Le rectangle des tuiles non vides d'un dessin bitmap (à la tuile près).
pub(crate) fn raster_content_rect(raster: &Raster) -> Option<Rect> {
    let mut r = Rect::NOTHING;
    for (idx, t) in raster.tiles.iter().enumerate() {
        if t.is_some() {
            let (x, y, w, h) = raster.tile_bounds(idx);
            r = r.union(Rect::from_min_max(pos2(x as f32, y as f32), pos2((x + w) as f32, (y + h) as f32)));
        }
    }
    (r.is_finite() && r.width() > 0.0).then_some(r)
}

/// La zone touche-t-elle au moins un pixel dessiné ?
fn mask_hits_pixels(raster: &Raster, mask: &PixelMask) -> bool {
    let (x1, y1) = (mask.x0 + mask.w, mask.y0 + mask.h);
    if mask.w == 0 || mask.h == 0 {
        return false;
    }
    for ty in (mask.y0 / TILE)..=((y1 - 1) / TILE) {
        for tx in (mask.x0 / TILE)..=((x1 - 1) / TILE).min(raster.tiles_x.saturating_sub(1)) {
            let idx = ty * raster.tiles_x + tx;
            let Some(Some(tile)) = raster.tiles.get(idx) else { continue };
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            for y in oy.max(mask.y0)..(oy + th).min(y1) {
                for x in ox.max(mask.x0)..(ox + tw).min(x1) {
                    if mask.data[(y - mask.y0) * mask.w + (x - mask.x0)] > 0.0 && tile[(y - oy) * TILE + (x - ox)].a() > 0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Les parts que prend le polygone `poly` (sur la feuille) : sur chaque calque vectoriel, les
/// traits qui ont un point dedans ; sur chaque calque bitmap, la zone, s'il y a des pixels dedans.
pub(crate) fn select_polygon(doc: &Document, targets: &Targets, poly: &[Pos2]) -> Vec<Part> {
    let mut parts = Vec::new();
    for t in &targets.list {
        let Some(local) = polygon_in_layer(&t.to_stage, poly) else { continue };
        let Some(cel) = doc.cels.get(&t.cel) else { continue };
        if t.vector {
            let Some(d) = cel.vector.as_ref() else { continue };
            let items: Vec<usize> = d.elements.iter().enumerate().filter(|(_, e)| e.touches(&local)).map(|(i, _)| i).collect();
            if !items.is_empty() {
                parts.push(vector_part(t, items));
            }
        } else if let Some(mask) = PixelMask::from_polygons(std::slice::from_ref(&local), doc.width, doc.height) {
            if mask_hits_pixels(&cel.raster, &mask) {
                parts.push(bitmap_part(t, mask, vec![local], None));
            }
        }
    }
    parts
}

/// Tout ce que contiennent les dessins choisis : tous les traits, et pour un calque bitmap le
/// rectangle de ses pixels.
pub(crate) fn select_everything(doc: &Document, targets: &Targets) -> Vec<Part> {
    let mut parts = Vec::new();
    for t in &targets.list {
        let Some(cel) = doc.cels.get(&t.cel) else { continue };
        if t.vector {
            let n = cel.vector.as_ref().map_or(0, |d| d.elements.len());
            if n > 0 {
                parts.push(vector_part(t, (0..n).collect()));
            }
        } else if let Some(r) = raster_content_rect(&cel.raster) {
            let poly = pixsel::rect_polygon(r);
            if let Some(mask) = PixelMask::from_polygons(std::slice::from_ref(&poly), doc.width, doc.height) {
                parts.push(bitmap_part(t, mask, vec![poly], None));
            }
        }
    }
    parts
}

/// Le trait du calque le plus haut sous le point `p` de la feuille (tolérance `tol` en pixels de la
/// feuille) : le calque et le rang de l'élément.
pub(crate) fn hit_item<'a>(doc: &Document, targets: &'a Targets, p: Pos2, tol: f32) -> Option<(&'a Target, usize)> {
    for t in targets.list.iter().rev().filter(|t| t.vector) {
        let Some(inv) = t.to_stage.inverse() else { continue };
        let local_tol = tol * inv.width_factor();
        let hit = doc.cels.get(&t.cel).and_then(|c| c.vector.as_ref()).and_then(|d| d.hit_test(inv.apply(p), local_tol));
        if let Some(i) = hit {
            return Some((t, i));
        }
    }
    None
}

// ---------------------------------------------------------------------------------- les gestes

/// Ce qu'il faut garder d'une part au début d'un geste pour l'annuler et le poser.
pub(crate) struct Snap {
    pub part: usize,
    /// Le dessin vectoriel d'avant, et l'état des éléments choisis.
    pub before: Option<VectorDrawing>,
    pub originals: Vec<Element>,
    /// L'image bitmap d'avant, et où étaient les pixels soulevés.
    pub before_raster: Option<Raster>,
    pub aff0: Affine,
}

/// Place de la feuille flottante de la part `k` (la 0 est celle d'une sélection sur un seul calque).
pub(crate) fn slot_of(k: usize) -> usize {
    1 + k
}

/// Le premier changement d'un geste : chaque part vectorielle reçoit sa feuille flottante (le
/// dessin sans elle, elle seule), chaque zone bitmap soulève ses pixels. Ne rejoue jamais le
/// moteur des brosses : tout vient des pixels en cache.
pub(crate) fn begin_gesture(doc: &mut Document, sel: &mut MultiSel, floats: &mut Vec<Flottante>) -> Vec<Snap> {
    for f in floats.drain(..) {
        Flottante::release_slot(doc, f.slot);
    }
    let mut snaps = Vec::new();
    for (k, part) in sel.parts.iter_mut().enumerate() {
        match &mut part.kind {
            PartKind::Vector { items } => {
                let Some(d) = doc.cels.get(&part.cel).and_then(|c| c.vector.as_ref()) else { continue };
                let before = d.clone();
                let originals: Vec<Element> = items.iter().filter_map(|&i| d.elements.get(i).cloned()).collect();
                if let Some(f) = Flottante::lift_slot(doc, part.cel, items, slot_of(k)) {
                    floats.push(f);
                }
                snaps.push(Snap { part: k, before: Some(before), originals, before_raster: None, aff0: Affine::IDENTITY });
            }
            PartKind::Bitmap { mask, lift, .. } => {
                let Some(c) = doc.cels.get_mut(&part.cel) else { continue };
                let before = c.raster.clone();
                if lift.is_none() {
                    let floating = pixsel::lift(&mut c.raster, mask);
                    let base = c.raster.clone();
                    let placed = pixsel::stamp(&mut c.raster, &floating, &Affine::IDENTITY);
                    *lift = Some(Lift { floating, base, aff: Affine::IDENTITY, placed });
                }
                let aff0 = lift.as_ref().map_or(Affine::IDENTITY, |l| l.aff);
                snaps.push(Snap { part: k, before: None, originals: Vec::new(), before_raster: Some(before), aff0 });
            }
        }
    }
    snaps
}

/// Pose les pixels soulevés avec `aff` (à la place de la précédente) : les tuiles touchées.
fn place_lift(c: &mut crate::document::Cel, l: &mut Lift, aff: Affine, dirty: &mut Vec<usize>) {
    if let Some(r) = l.placed {
        for idx in vector::tiles_in(&c.raster, r) {
            c.raster.tiles[idx] = l.base.tiles[idx].clone();
            dirty.push(idx);
        }
    }
    l.placed = pixsel::stamp(&mut c.raster, &l.floating, &aff);
    if let Some(r) = l.placed {
        dirty.extend(vector::tiles_in(&c.raster, r));
    }
    l.aff = aff;
    c.version += 1;
}

/// Chaque image du geste : la transformation `a` (sur la feuille) devient celle de chaque feuille
/// flottante, et les zones bitmap sont reposées. `snap` : les déplacements des zones bitmap
/// tombent sur des pixels entiers (l'image reste nette). Rend les tuiles à rafraîchir.
pub(crate) fn set_gesture(doc: &mut Document, sel: &mut MultiSel, floats: &mut [Flottante], snaps: &[Snap], a: &Affine, snap: bool) -> Vec<usize> {
    let mut dirty = Vec::new();
    for s in snaps {
        let part = &mut sel.parts[s.part];
        let Some(mut delta) = layer_delta(&part.to_stage, a) else { continue };
        match &mut part.kind {
            PartKind::Vector { .. } => {
                if let Some(f) = floats.iter_mut().find(|f| f.slot == slot_of(s.part)) {
                    f.aff = delta;
                }
            }
            PartKind::Bitmap { lift: Some(l), .. } => {
                if snap {
                    delta.tx = delta.tx.round();
                    delta.ty = delta.ty.round();
                }
                if let Some(c) = doc.cels.get_mut(&part.cel) {
                    place_lift(c, l, s.aff0.then(&delta), &mut dirty);
                }
            }
            PartKind::Bitmap { .. } => {}
        }
    }
    dirty
}

/// Le geste est lâché : la géométrie des traits reçoit la transformation (leur feuille reste
/// affichée jusqu'au rendu exact), les zones bitmap gardent leur place. Rend les étapes
/// d'annulation, qui forment ensemble une seule étape.
pub(crate) fn commit_gesture(doc: &mut Document, sel: &MultiSel, floats: &mut [Flottante], snaps: Vec<Snap>, a: &Affine) -> Vec<Edit> {
    let mut edits = Vec::new();
    for s in snaps {
        let part = &sel.parts[s.part];
        let Some(delta) = layer_delta(&part.to_stage, a) else { continue };
        match &part.kind {
            PartKind::Vector { items } => {
                Flottante::apply_geometry(doc, part.cel, items, &s.originals, &delta);
                if let Some(f) = floats.iter_mut().find(|f| f.slot == slot_of(s.part)) {
                    f.aff = delta;
                    f.pending = Some(Pending { items: items.clone(), originals: s.originals });
                }
                if let Some(before) = s.before {
                    edits.push(Edit::Vector { cel: part.cel, drawing: before });
                }
            }
            PartKind::Bitmap { .. } => {
                let (Some(before), Some(c)) = (s.before_raster, doc.cels.get(&part.cel)) else { continue };
                let tiles = changed_tiles(&before, &c.raster);
                if !tiles.is_empty() {
                    edits.push(Edit::Tiles { cel: part.cel, tiles });
                }
            }
        }
    }
    edits
}

/// Les tuiles de `before` qui ne sont plus celles de `now` (de quoi annuler).
pub(crate) fn changed_tiles(before: &Raster, now: &Raster) -> Vec<(usize, Option<Tile>)> {
    (0..now.tiles.len().min(before.tiles.len())).filter(|&i| before.tiles[i] != now.tiles[i]).map(|i| (i, before.tiles[i].clone())).collect()
}

/// Un geste en une fois (flèches, miroir, essais) : saisie, pose et lâcher. Les feuilles flottantes
/// restent, en attente du rendu exact.
pub(crate) fn run_gesture(doc: &mut Document, sel: &mut MultiSel, floats: &mut Vec<Flottante>, a: &Affine, snap: bool) -> (Vec<Edit>, Vec<usize>) {
    let snaps = begin_gesture(doc, sel, floats);
    let dirty = set_gesture(doc, sel, floats, &snaps, a, snap);
    (commit_gesture(doc, sel, floats, snaps, a), dirty)
}

/// Efface la sélection : les traits sont retirés, les zones bitmap vidées. Une seule étape.
pub(crate) fn delete_selection(doc: &mut Document, sel: &MultiSel) -> Vec<Edit> {
    let mut edits = Vec::new();
    for part in &sel.parts {
        let Some(c) = doc.cels.get_mut(&part.cel) else { continue };
        match &part.kind {
            PartKind::Vector { items } => {
                let Some(d) = c.vector.as_mut() else { continue };
                let before = d.clone();
                let mut idx = items.clone();
                idx.sort_unstable();
                idx.dedup();
                for i in idx.into_iter().rev() {
                    if i < d.elements.len() {
                        d.elements.remove(i);
                    }
                }
                c.rerender();
                edits.push(Edit::Vector { cel: part.cel, drawing: before });
            }
            PartKind::Bitmap { mask, lift, .. } => {
                let before = c.raster.clone();
                match lift {
                    // Les pixels soulevés étaient les seuls de la zone : le dessin sans eux.
                    Some(l) => c.raster = l.base.clone(),
                    None => {
                        pixsel::lift(&mut c.raster, mask);
                    }
                }
                let tiles = changed_tiles(&before, &c.raster);
                c.version += 1;
                if !tiles.is_empty() {
                    edits.push(Edit::Tiles { cel: part.cel, tiles });
                }
            }
        }
    }
    edits
}

// -------------------------------------------------------------------------- copier et coller

/// Ce qu'une part a copié.
pub(crate) enum ClipWhat {
    Vector(Vec<Element>),
    Pixels(Floating),
}

/// Ce qu'a copié la sélection sur plusieurs calques : par calque, ses éléments ou ses pixels.
pub(crate) struct MultiClip {
    pub items: Vec<(u64, ClipWhat)>,
}

impl MultiClip {
    pub fn summary(&self) -> String {
        let n = self.items.len();
        format!("{} copié{}", plural(n, "calque", "calques"), if n > 1 { "s" } else { "" })
    }
}

/// Les pixels d'une zone bitmap, tels qu'affichés (déplacés s'ils ont été soulevés).
fn part_pixels(doc: &Document, part: &Part) -> Option<Floating> {
    let PartKind::Bitmap { mask, lift, .. } = &part.kind else { return None };
    let c = doc.cels.get(&part.cel)?;
    let f = match lift {
        None => pixsel::copy(&c.raster, mask),
        Some(l) => {
            let mut r = Raster::new(c.raster.width, c.raster.height);
            let placed = pixsel::stamp(&mut r, &l.floating, &l.aff)?;
            let m = PixelMask::from_polygons(&[pixsel::rect_polygon(placed)], r.width, r.height)?;
            pixsel::lift(&mut r, &m)
        }
    };
    (!f.is_empty()).then_some(f)
}

pub(crate) fn copy_selection(doc: &Document, sel: &MultiSel) -> MultiClip {
    let mut items = Vec::new();
    for part in &sel.parts {
        match &part.kind {
            PartKind::Vector { items: idx } => {
                let Some(d) = doc.cels.get(&part.cel).and_then(|c| c.vector.as_ref()) else { continue };
                let elements: Vec<Element> = idx.iter().filter_map(|&i| d.elements.get(i).cloned()).collect();
                if !elements.is_empty() {
                    items.push((part.layer, ClipWhat::Vector(elements)));
                }
            }
            PartKind::Bitmap { .. } => {
                if let Some(f) = part_pixels(doc, part) {
                    items.push((part.layer, ClipWhat::Pixels(f)));
                }
            }
        }
    }
    MultiClip { items }
}

/// Colle sur les calques de `targets` ce que la copie y avait pris, à la même place : chaque
/// calque reçoit ses éléments (ou ses pixels), et ce qui est collé devient la sélection. Rend les
/// parts collées et les étapes d'annulation.
pub(crate) fn paste_clip(doc: &mut Document, targets: &Targets, clip: &MultiClip) -> (Vec<Part>, Vec<Edit>) {
    let (mut parts, mut edits) = (Vec::new(), Vec::new());
    for (layer, what) in &clip.items {
        let Some(t) = targets.list.iter().find(|t| t.id == *layer) else { continue };
        let (w, h) = (doc.width, doc.height);
        let Some(c) = doc.cels.get_mut(&t.cel) else { continue };
        match what {
            ClipWhat::Vector(elements) if t.vector => {
                let Some(d) = c.vector.as_mut() else { continue };
                let before = d.clone();
                let start = d.elements.len();
                d.elements.extend(elements.iter().cloned());
                c.rerender();
                edits.push(Edit::Vector { cel: t.cel, drawing: before });
                // (Le dessin a maintenant plus d'éléments : la part les compte.)
                let mut part = vector_part(t, (start..start + elements.len()).collect());
                part.elements = start + elements.len();
                parts.push(part);
            }
            ClipWhat::Pixels(f) if !t.vector => {
                let before = c.raster.clone();
                let base = c.raster.clone();
                let placed = pixsel::stamp(&mut c.raster, f, &Affine::IDENTITY);
                c.version += 1;
                let poly = pixsel::rect_polygon(f.bounds());
                let Some(mask) = PixelMask::from_polygons(std::slice::from_ref(&poly), w, h) else { continue };
                let tiles = changed_tiles(&before, &c.raster);
                if !tiles.is_empty() {
                    edits.push(Edit::Tiles { cel: t.cel, tiles });
                }
                let lift = Lift { floating: f.clone(), base, aff: Affine::IDENTITY, placed };
                parts.push(bitmap_part(t, mask, vec![poly], Some(lift)));
            }
            _ => {}
        }
    }
    (parts, edits)
}

// ------------------------------------------------------------------------ l'état de l'application

/// Un geste de la souris (ou d'un champ chiffré) sur la sélection, en coordonnées de la feuille.
pub(crate) enum MDrag {
    Lasso { points: Vec<Pos2>, add: bool },
    Rect { from: Pos2, to: Pos2, add: bool },
    /// Un geste de la boîte : `frame`, la boîte au début ; `snaps` : de quoi poser et annuler, pris
    /// au premier changement ; `aff` : la transformation du moment ; `field` : un champ chiffré.
    Transform { handle: Handle, start: Pos2, frame: SelFrame, snaps: Option<Vec<Snap>>, aff: Affine, field: bool },
    /// Le pivot tiré.
    Pivot { start: Pos2, frame: SelFrame },
}

/// Tout l'état de la sélection sur plusieurs calques dans l'application.
#[derive(Default)]
pub(crate) struct Multi {
    pub sel: Option<MultiSel>,
    pub drag: Option<MDrag>,
    /// Les feuilles flottantes d'un geste (en cours, ou lâché et en attente du rendu exact).
    pub floats: Vec<Flottante>,
    pub clip: Option<MultiClip>,
    /// Les contours des traits choisis, gardés d'une image à l'autre (`contours.rs`).
    pub contours: std::cell::RefCell<Option<crate::contours::Contours>>,
    /// Ctrl+Maj tenus au début du geste (comme dans Storyboard Pro) : le lasso, le rectangle ou le
    /// clic prennent sur TOUS les calques du plan à l'image courante, pas seulement les calques
    /// choisis. Une fois le geste fini, les calques touchés deviennent les calques choisis.
    pub all_layers: bool,
}

impl Multi {
    pub fn float_for(&self, cel: CelId) -> Option<&Flottante> {
        self.floats.iter().find(|f| f.src == cel)
    }

    /// Un geste de la boîte est en cours sur ce dessin : son image est celle du geste.
    pub fn busy_on(&self, cel: CelId) -> bool {
        matches!(&self.drag, Some(MDrag::Transform { .. })) && self.sel.as_ref().is_some_and(|s| s.has_cel(cel))
    }

    /// Des feuilles lâchées attendent le rendu exact de leur dessin.
    pub fn has_pending(&self) -> bool {
        self.floats.iter().any(|f| f.pending.is_some())
    }

}
