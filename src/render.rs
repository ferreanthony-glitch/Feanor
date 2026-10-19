use eframe::egui::{self, Color32, ColorImage, TextureFilter, TextureHandle, TextureOptions};

use crate::blend::{blend_over, BlendMode};
use crate::camera::Camera;
use crate::document::{CameraSeen, CelId, Document, LayerTransform, Panel};
use crate::raster::{tile_bounds, tiles_across, Raster, TILE, TILE_PIXELS};
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

/// Ce que montre un élément de fond.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FondPart {
    /// La forme déduite du trait, de ces réglages, moins ce qu'on en a coupé, avec ce qu'on a
    /// peint dans le fond, dans la forme (recalculée quand le dessin change ; refaite tuile par
    /// tuile pendant qu'on peint dedans, `fx::fond_patch`).
    Forme(crate::fond::FondReglages),
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
    let get = |x: i64, y: i64| -> [f32; 4] {
        if x < 0 || y < 0 || x >= raster.width as i64 || y >= raster.height as i64 {
            return [0.0; 4];
        }
        let c = raster.get(x as usize, y as usize);
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

/// Traite `buf` par morceaux de `chunk` éléments, répartis sur les cœurs du processeur
/// (`f` reçoit le numéro du morceau et le morceau).
pub(crate) fn par_chunks<T: Send>(buf: &mut [T], chunk: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
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

/// Ce que voit la caméra `cam` : l'image finale (`doc.frame`). Chaque pixel prend la couleur de
/// la feuille au point filmé (interpolation bilinéaire) ; caméra centrée sans zoom ni rotation :
/// les pixels de la feuille, tels quels. Hors de la feuille : transparent.
///
/// Seule la zone filmée de la feuille est composée (en tuiles entières, avec une marge pour
/// l'interpolation), et le travail est réparti sur les cœurs du processeur.
pub fn camera_image(doc: &Document, plan: &Plan, cam: &Camera) -> Vec<Color32> {
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
    if w == 0 || h == 0 || hi.0 + 2.0 < 0.0 || hi.1 + 2.0 < 0.0 || lo.0 - 2.0 >= w as f32 || lo.1 - 2.0 >= h as f32 {
        return out;
    }
    let clamp = |v: f32, max: usize| (v.max(0.0) as usize).min(max - 1);
    let (tx0, ty0) = (clamp(lo.0 - 2.0, w) / TILE, clamp(lo.1 - 2.0, h) / TILE);
    let (tx1, ty1) = (clamp(hi.0 + 2.0, w) / TILE, clamp(hi.1 + 2.0, h) / TILE);
    let (rx, ry) = (tx0 * TILE, ty0 * TILE);
    let rw = ((tx1 + 1) * TILE).min(w) - rx;
    let rh = ((ty1 + 1) * TILE).min(h) - ry;
    let tiles_x = tiles_across(w);

    // Composer la zone, une bande de tuiles par morceau.
    let mut region = vec![Color32::TRANSPARENT; rw * rh];
    par_chunks(&mut region, TILE * rw, |band, buf| {
        let ty = ty0 + band;
        for tx in tx0..=tx1 {
            composite_tile_into(doc, plan, ty * tiles_x + tx, buf, rw, (rx, ty * TILE));
        }
    });

    // Échantillonner ce que filme la caméra. Hors de la zone (donc hors de la feuille, grâce à la
    // marge) : transparent.
    let region = &region;
    let get = |x: i64, y: i64| -> [f32; 4] {
        let (x, y) = (x - rx as i64, y - ry as i64);
        if x < 0 || y < 0 || x >= rw as i64 || y >= rh as i64 {
            return [0.0; 4];
        }
        let c = region[y as usize * rw + x as usize];
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
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
    pixel_of(&resolve(doc, plan), paper_color(plan), x, y)
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

/// Le pixel (`x`, `y`) de la feuille : le papier `base`, puis chaque élément.
fn pixel_of(layers: &[Resolved], base: Color32, x: usize, y: usize) -> Color32 {
    let mut c = base;
    for l in layers {
        let r = &l.cel.raster;
        let s = match &l.inv {
            Some(inv) => {
                let p = inv.apply(egui::pos2(x as f32 + 0.5, y as f32 + 0.5));
                sample(r, p.x * l.kx, p.y * l.ky)
            }
            None if l.same => r.get(x, y),
            None => {
                let (rx, ry) = (((x as f32 + 0.5) * l.kx) as usize, ((y as f32 + 0.5) * l.ky) as usize);
                if rx < r.width && ry < r.height { r.get(rx, ry) } else { Color32::TRANSPARENT }
            }
        };
        if s.a() != 0 {
            c = blend_over(c, s, l.item.opacity, l.item.tint, l.item.blend, (x, y));
        }
    }
    c
}

/// Plan de l'image `frame` du panneau `index` tel que la caméra le filme (vignettes, export) :
/// les calques en profondeur défilent selon le décalage de la caméra (multiplan).
pub fn panel_plan_at(doc: &Document, index: usize, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    plan_with_camera(&doc.panels[index], frame, paper, &doc.camera_seen(index, frame), doc.fps)
}

/// Le calque `li` du plan `index` seul, à l'image `frame`, tel que la caméra au centre le voit
/// (export vers Storyboard Pro, 14b) : en mode normal, à pleine opacité et même caché (sa
/// visibilité et son opacité sont reprises à part), effets compris. Aucun : un dossier, ou rien
/// à cette image.
pub fn layer_plan_at(doc: &Document, index: usize, li: usize, frame: usize) -> Option<Plan> {
    let panel = doc.panels.get(index)?;
    let l = panel.layers.get(li)?;
    if l.folder {
        return None;
    }
    let cel = crate::video::cel_at(l, frame, doc.fps)?;
    let t = l.transform;
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
    Plan {
        paper,
        items: panel
            .layers
            .iter()
            .enumerate()
            .filter_map(|(li, l)| {
                // Caché (lui ou son dossier, 13b-5) : rien ; l'opacité du dossier s'ajoute.
                let opacity = crate::folders::effect(&panel.layers, li)?;
                let t = l.transform_seen(camera)?;
                // Calque vidéo (13k) : l'image de la vidéo qui tombe là.
                crate::video::cel_at(l, frame, fps).map(|cel| PlanItem {
                    cel,
                    opacity: (opacity * 255.0).round() as u8,
                    tint: None,
                    transform: (!t.is_identity()).then_some(t),
                    blend: l.blend,
                    fx: crate::fx::layer_steps(l, frame, None),
                    ctg: crate::ctg::input(&panel.layers, li, frame),
                    fond: None,
                    pre: None,
                })
                .map(|line| (li, line))
            })
            // Le fond sous le trait (`fond.rs`), juste sous le dessin de son calque.
            .flat_map(|(li, line)| fond_item(&panel.layers[li], &line).into_iter().chain([line]))
            .collect(),
    }
}

/// Image réduite de ce que voit la caméra `cam` (moyenne de 4 échantillons par pixel).
pub fn thumbnail(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize) -> ColorImage {
    let [fw, fh] = doc.frame;
    let (sx, sy) = (fw as f32 / tw as f32, fh as f32 / th as f32);
    let map = cam.frame_map(doc.frame);
    let mut pixels = Vec::with_capacity(tw * th);
    // (Les dessins affichés se cherchent une fois, pas à chaque échantillon : 80 000 par vignette.)
    let layers = resolve(doc, plan);
    let base = paper_color(plan);
    for y in 0..th {
        for x in 0..tw {
            let mut acc = [0u32; 4];
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let c = match map.at((x as f32 + ox) * sx, (y as f32 + oy) * sy) {
                    Some(p) if p.x >= 0.0 && p.y >= 0.0 && p.x < doc.width as f32 && p.y < doc.height as f32 => {
                        pixel_of(&layers, base, p.x as usize, p.y as usize)
                    }
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
    ColorImage {
        size: [tw, th],
        pixels,
    }
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

    /// Un fond arrivé de l'arrière-plan (`fx::fond_poll`) remplace celui d'avant : le plan est le
    /// même, l'image composée par le processeur est à refaire (la carte graphique suit les versions
    /// des dessins dérivés toute seule).
    pub fn invalidate_composed(&mut self) {
        self.dirty_all = true;
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
