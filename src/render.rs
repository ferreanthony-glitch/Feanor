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
    /// Remplissage au griffonnage d'un calque de couleur (`griffonnage.rs`) : cet élément montre le remplissage des
    /// griffonnages `cel` d'après les traits de ces sources (les dessins des sources à cette image). Rien pour un
    /// dessin ordinaire. (Le nom `ctg` est celui du calque CTG que ce remplissage a remplacé : `propager.rs` l'écrit
    /// encore.)
    pub ctg: Option<crate::griffonnage::GriffonInput>,
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
    /// Carte inclinée de l'Espace 3D (`espace3d.rs`) : le maillage pose le calque entier sur la feuille, ce n'est ni un
    /// geste en cours ni une feuille flottante (un calque d'ajustement au-dessus la voit comme un dessin posé).
    pub card: bool,
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
    /// La couleur unie d'un calque « Couleur unie » (`couleurunie.rs`) : toute la feuille de cette couleur, sous les dessins du calque.
    /// Son dessin (un seul pixel, étiré sur la feuille) est fait à la demande et gardé par couleur : aucun bitmap de la taille de la feuille.
    Unie([u8; 3]),
}

/// Le numéro d'élément de plan de la couleur unie : aucun dessin du projet n'a ce numéro.
pub const UNIE_CEL: CelId = (1 << 62) | 0x554e_4945;

/// L'élément de la couleur unie du calque `layer` (s'il en a une), à placer juste sous ses dessins. `opacity` : celle du calque à
/// cette image, déjà multipliée par celle de son dossier.
pub fn unie_item(layer: &crate::document::Layer, opacity: u8) -> Option<PlanItem> {
    let c = layer.unie.filter(|_| !layer.folder)?;
    Some(PlanItem { cel: UNIE_CEL, opacity, tint: None, transform: None, blend: layer.blend, fx: Vec::new(), ctg: None, fond: Some(FondPart::Unie(c)), pre: None })
}

/// L'élément du fond sous le trait du calque `layer` (s'il en a un), à placer juste sous `line`,
/// l'élément de son dessin.
pub fn fond_item(layer: &crate::document::Layer, line: &PlanItem) -> Option<PlanItem> {
    let r = layer.fond.filter(|_| !layer.folder && layer.video.is_none())?;
    Some(PlanItem { fx: Vec::new(), ctg: None, fond: Some(FondPart::Forme(r)), ..line.clone() })
}

/// L'élément du remplissage au griffonnage du calque de couleur `li` de `layers` à l'image `frame` (s'il en a un), à placer
/// juste sous `line`, l'élément de sa peinture libre : un dessin dérivé des griffonnages et des traits de ses sources
/// (`griffonnage.rs`), calculé en arrière-plan à l'écran. Il a les effets, la transformation et le mode de fusion du calque,
/// comme la peinture libre qu'il accompagne.
pub fn griffon_item(layers: &[crate::document::Layer], li: usize, frame: usize, line: &PlanItem) -> Option<PlanItem> {
    let (griffonnage, input) = crate::griffonnage::entree(layers, li, frame)?;
    Some(PlanItem { cel: griffonnage, ctg: Some(input), fond: None, ..line.clone() })
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
pub(crate) fn inverse_of(doc: &Document, item: &PlanItem) -> Option<Affine> {
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
    let feuille = Area::sheet(doc);
    plan_ink_bounds(doc, plan).map_or(feuille, |encre| feuille.union(&encre))
}

/// Où le plan a de l'encre : les tuiles dessinées de ses dessins, sur la feuille et autour d'elle, déplacées par la transformation de leur
/// calque (un dessin posé par un maillage : la feuille entière). Rien si rien n'est dessiné. La planche des Vignettes s'en sert pour montrer
/// le dessin d'une case au-delà de son cadre, jusqu'où il va (`thumbs.rs`, partie 131).
pub fn plan_ink_bounds(doc: &Document, plan: &Plan) -> Option<Area> {
    plan_ink_bounds_sauf(doc, plan, &[])
}

/// Les dessins que la case du plan `panel` à l'image `frame` **n'est pas seule à montrer** : ceux d'un calque Décor (un seul dessin pour
/// toutes les cases du plan, `decor.rs`), d'un calque partagé entre des plans (la planche-mère, `partage.rs`), d'un calque de toute la scène,
/// et ceux qu'une autre case du même plan montre aussi (un dessin tenu d'une marque d'image à l'autre).
///
/// Partie 131 : « le trait entier à sa case, montré sur la planche où qu'il aille » ne vaut que pour un dessin qu'**une seule case** montre.
/// Ceux-ci gardent la règle d'avant : chaque case en montre son cadre et sa marge ordinaire, un trait tracé sur la planche y est réparti case
/// par case (`horschamp.rs`), la gomme de la planche n'y touche pas (`gommedebord.rs`). Sinon chaque case qui les montre en répéterait le
/// débord autour d'elle, par-dessus ses voisines (le Décor d'une continuité, une planche-mère vue par trois caméras).
pub fn dessins_communs(doc: &Document, panel: usize, frame: usize) -> Vec<CelId> {
    let Some(p) = doc.panels.get(panel) else { return Vec::new() };
    // Les cases d'un plan commencent à sa première image et à ses marques d'image : celle-ci commence à `debut`.
    let debut = p.marks.iter().map(|m| m.frame).filter(|f| *f <= frame).max().unwrap_or(0);
    let mut communs = Vec::new();
    for l in &p.layers {
        let Some(cel) = l.cel_at(frame) else { continue };
        let partage = l.decor || l.partage.is_some() || l.scene_wide.is_some();
        let tenu = || std::iter::once(0).chain(p.marks.iter().map(|m| m.frame)).any(|f| f != debut && l.cel_at(f) == Some(cel));
        if (partage || tenu()) && !communs.contains(&cel) {
            communs.push(cel);
        }
    }
    communs
}

/// `plan_ink_bounds`, sans compter les dessins `sauf` (`dessins_communs` : ils ne font pas s'étendre la vignette d'une case).
pub fn plan_ink_bounds_sauf(doc: &Document, plan: &Plan, sauf: &[CelId]) -> Option<Area> {
    let mut encre: Option<Area> = None;
    let mut ajoute = |r: Area| encre = Some(encre.map_or(r, |a| a.union(&r)));
    let pivot = doc.stage_center();
    let t = TILE as i64;
    for item in &plan.items {
        if sauf.contains(&item.cel) {
            continue;
        }
        let Some(cel) = doc.cels.get(&item.cel) else { continue };
        let Some((tx0, ty0, tx1, ty1)) = cel.raster.drawn_tile_range() else { continue };
        if item.warp(pivot).is_some() {
            ajoute(Area::sheet(doc));
        }
        // (Ce que ses effets portent autour de l'encre, pour un dessin qui passe le bord : le halo d'un flou, le déplacement d'une secousse.)
        let m = crate::fx::portee_hors_feuille(item, cel);
        let (x0, y0, x1, y1) = ((tx0 as i64 * t) as f32 - m, (ty0 as i64 * t) as f32 - m, ((tx1 as i64 + 1) * t) as f32 + m, ((ty1 as i64 + 1) * t) as f32 + m);
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
        // (L'aplat sous le trait déborde du trait de son débord : une tuile de plus autour, quand le trait sort de la feuille.
        // Un trait qui reste dans la feuille a son aplat dans la feuille : rien de plus à composer.)
        let sort = r.x0 < 0 || r.y0 < 0 || r.x1 > doc.width as i64 || r.y1 > doc.height as i64;
        let r = if item.fond.is_some() && sort { Area { x0: r.x0 - t, y0: r.y0 - t, x1: r.x1 + t, y1: r.y1 + t } } else { r };
        ajoute(r);
    }
    encre
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
                let m = crate::fx::portee_hors_feuille(item, cel);
                let (x0, y0, x1, y1) = ((tx0 as i64 * t) as f32 - m, (ty0 as i64 * t) as f32 - m, ((tx1 as i64 + 1) * t) as f32 + m, ((ty1 as i64 + 1) * t) as f32 + m);
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
                // (L'aplat sous le trait déborde du trait de son débord : les tuiles voisines comptent aussi.)
                let around = if item.fond.is_some() { 1 } else { 0 };
                // Les effets d'un dessin qui passe le bord posent aussi autour de ses tuiles : déjà calculés, on lit leurs tuiles ; sinon, on les devine d'après leur portée.
                let pret = (!item.fx.is_empty() && crate::fx::cel_deborde(cel)).then(|| crate::fx::resultat_pret(doc, item)).flatten();
                let around = match &pret {
                    Some(_) => around,
                    None => (around as i64).max(crate::fxmonde::en_tuiles(crate::fx::portee_hors_feuille(item, cel)) as i64),
                };
                // (Une très grande portée : toute la zone autour de l'encre, bornée par ce que la vue compose, plutôt qu'un carré autour de chaque tuile.)
                if around > 16 {
                    let (ax0, ay0) = ((tx0 as i64 - around).max(area.x0.div_euclid(t)), (ty0 as i64 - around).max(area.y0.div_euclid(t)));
                    let (ax1, ay1) = ((tx1 as i64 + around).min((area.x1 - 1).div_euclid(t)), (ty1 as i64 + around).min((area.y1 - 1).div_euclid(t)));
                    for ty in ay0..=ay1 {
                        for tx in ax0..=ax1 {
                            if wanted(tx, ty) {
                                set.insert((tx as i32, ty as i32));
                            }
                        }
                    }
                    continue;
                }
                let raster = pret.as_ref().map_or(&cel.raster, |c| &c.raster);
                for id in raster.drawn_ids() {
                    let (tx, ty) = raster.key_of(id);
                    for (dx, dy) in (-around..=around).flat_map(|dy| (-around..=around).map(move |dx| (dx, dy))) {
                        let (nx, ny) = (tx as i64 + dx, ty as i64 + dy);
                        if wanted(nx, ny) {
                            set.insert((nx as i32, ny as i32));
                        }
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
            world_item_over(&cel.raster, Some(inv), item.opacity, item.tint, item.blend, tx, ty, buf, stride, origin);
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
        world_item_over(r, None, item.opacity, item.tint, item.blend, tx, ty, buf, stride, origin);
    }
}

/// Pose sur la tuile du monde (`tx`, `ty`) de `buf` (largeur `stride`, coin haut gauche au point `origin` du monde) un
/// dessin de la taille de la feuille, ou transformé (`inv` : du monde vers le dessin) : ce que `composite_world_tile`
/// fait pour un élément du plan qui n'est ni posé par un maillage ni d'une autre taille. Le hors-feuille de l'écran le
/// fait aussi sur un autre fil, avec les mêmes pixels (`monde.rs`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn world_item_over(raster: &Raster, inv: Option<Affine>, opacity: u8, tint: Option<[u8; 3]>, blend: BlendMode, tx: i32, ty: i32, buf: &mut [Color32], stride: usize, origin: (i64, i64)) {
    let t = TILE as i64;
    let (ox, oy) = (tx as i64 * t, ty as i64 * t);
    let at = |x: i64, y: i64| ((y - origin.1) as usize) * stride + (x - origin.0) as usize;
    if let Some(inv) = inv {
        for y in 0..t {
            let s = at(ox, oy + y);
            for (x, d) in buf[s..s + TILE].iter_mut().enumerate() {
                let p = inv.apply(egui::pos2((ox + x as i64) as f32 + 0.5, (oy + y) as f32 + 0.5));
                let src = sample(raster, p.x, p.y);
                if src.a() != 0 {
                    *d = blend_over(*d, src, opacity, tint, blend, (world_dither(ox + x as i64), world_dither(oy + y)));
                }
            }
        }
        return;
    }
    let Some(tile) = raster.tile_at(tx, ty) else { return };
    let tile: &[Color32; TILE_PIXELS] = tile;
    for y in 0..TILE {
        let s = at(ox, oy + y as i64);
        let row = &tile[y * TILE..y * TILE + TILE];
        for (x, (d, &src)) in buf[s..s + TILE].iter_mut().zip(row).enumerate() {
            if src.a() != 0 {
                *d = blend_over(*d, src, opacity, tint, blend, (world_dither(ox + x as i64), world_dither(oy + y as i64)));
            }
        }
    }
}

/// Le papier du plan (transparent s'il n'en a pas).
pub(crate) fn paper_of(paper: Option<[u8; 3]>) -> Color32 {
    paper_color(&Plan { paper, items: Vec::new() })
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
    // (Un export travaille sous la priorité de l'interface : ses fils aussi.)
    let basse = crate::parallele::priorite_basse();
    std::thread::scope(|s| {
        for group in groups {
            s.spawn(move || {
                crate::parallele::heriter_la_priorite(basse);
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
    /// Feuille posée par un maillage (carte inclinée de l'Espace 3D, enveloppe, perspective) : le dessin posé sur la
    /// feuille entière, calculé une fois ; il remplace le dessin de l'élément.
    laid: Option<Raster>,
}

fn resolve<'a>(doc: &'a Document, plan: &'a Plan) -> Vec<Resolved<'a>> {
    plan.items
        .iter()
        .filter_map(|item| {
            let cel = crate::fx::shown(doc, item)?;
            if let Some((warp, layer)) = item.warp(doc.stage_center()) {
                let laid = warp.rasterize(&cel.raster, layer.as_ref(), doc.width, doc.height);
                return Some(Resolved { item, cel, inv: None, kx: 1.0, ky: 1.0, same: true, laid: Some(laid) });
            }
            let (w, h) = (cel.raster.width, cel.raster.height);
            let kx = w as f32 / doc.width.max(1) as f32;
            let ky = h as f32 / doc.height.max(1) as f32;
            Some(Resolved { item, cel, inv: inverse_of(doc, item), kx, ky, same: w == doc.width && h == doc.height, laid: None })
        })
        .collect()
}

/// Le pixel (`x`, `y`) du monde (la feuille, ou la toile autour d'elle) : le papier `base`, puis
/// chaque élément.
fn pixel_of(layers: &[Resolved], base: Color32, x: i64, y: i64) -> Color32 {
    let mut c = base;
    for l in layers {
        let r = l.laid.as_ref().unwrap_or(&l.cel.raster);
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
///
/// **Sans les calques d'annotation du plan** (`Layer::annotation`, `calqueannot.rs`) : tout ce qui sort du film (exports, vignettes, lecture)
/// passe par ici, et des notes de réalisateur n'y ont pas leur place. `panel_plan_at_avec` les laisse venir quand on le demande.
pub fn panel_plan_at(doc: &Document, index: usize, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    panel_plan_at_avec(doc, index, frame, paper, false)
}

/// Comme `panel_plan_at`, avec ou sans les calques d'annotation du plan : `annotations` vrai, ils sont posés par-dessus tout le reste.
pub fn panel_plan_at_avec(doc: &Document, index: usize, frame: usize, paper: Option<[u8; 3]>, annotations: bool) -> Plan {
    let camera = doc.camera_seen(index, frame);
    plan_with_camera(&crate::espace3d::Monde::of(doc), &doc.panels[index], frame, paper, &camera, doc.repere_du_plan(index, frame).as_ref(), doc.fps, annotations)
}

/// L'image `frame` d'un plan rangé (une version qui n'est pas dans `doc.panels`), avec sa propre caméra : la
/// vignette d'une carte de la pile des versions.
pub fn stored_panel_plan(doc: &Document, panel: &Panel, frame: usize, paper: Option<[u8; 3]>) -> Plan {
    plan_with_camera(&crate::espace3d::Monde::of(doc), panel, frame, paper, &doc.camera_seen_du_plan_range(panel, frame), doc.repere_du_plan_range(panel, frame).as_ref(), doc.fps, false)
}

/// Le calque `li` du plan `index` seul, à l'image `frame`, tel que la caméra au centre le voit
/// (essais) : en mode normal, à pleine opacité et même caché (sa visibilité et son opacité sont
/// reprises à part), effets compris. Aucun : un dossier, ou rien à cette image.
#[cfg(test)]
pub fn layer_plan_at(doc: &Document, index: usize, li: usize, frame: usize) -> Option<Plan> {
    let panel = doc.panels.get(index)?;
    let l = panel.layers.get(li)?;
    // (Un calque d'annotation n'est pas un dessin du film : il n'entre pas dans ce plan.)
    if l.folder || l.effet.is_some() || l.annotation {
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
        ctg: None,
        fond: None,
        pre: None,
    };
    // Le remplissage au griffonnage (calque de couleur) et le fond sous le trait font partie de l'image du calque.
    Some(Plan { paper: None, items: griffon_item(&panel.layers, li, frame, &line).into_iter().chain(fond_item(l, &line)).chain([line]).collect() })
}

fn plan_with_camera(world: &crate::espace3d::Monde, panel: &Panel, frame: usize, paper: Option<[u8; 3]>, camera: &CameraSeen, repere: Option<&crate::espace3dmath::M4>, fps: u32, annotations: bool) -> Plan {
    plan_with_camera_layers(world, panel, frame, paper, camera, repere, fps, annotations).0
}

/// Les calques d'un plan que l'export des calques séparés (`exportlayers.rs`) écrit chacun à part, du plus bas au plus haut : ni dossier, ni
/// calque d'effet (il agit sur ce qui est dessous : il n'a pas d'image à lui), ni calque caché (lui ou son dossier), ni calque d'annotation
/// sauf si `annotations`.
pub fn separable_layers(panel: &Panel, annotations: bool) -> Vec<usize> {
    crate::calqueannot::ordre(&panel.layers, annotations)
        .into_iter()
        .filter(|&li| {
            let l = &panel.layers[li];
            !l.folder && l.effet.is_none() && crate::folders::effect(&panel.layers, li).is_some()
        })
        .collect()
}

/// Chaque calque séparable du plan `index` (`separable_layers`) seul, à l'image `frame`, sur fond transparent : (rang du calque, son plan) ;
/// un calque sans rien à cette image n'a pas de plan. Les éléments sont ceux de l'image entière (même caméra, mêmes réglages) : posés
/// l'un sur l'autre dans l'ordre, en mode normal, ils redonnent l'image du plan.
pub fn panel_layer_plans(doc: &Document, index: usize, frame: usize, annotations: bool) -> Vec<(usize, Plan)> {
    let camera = doc.camera_seen(index, frame);
    let panel = &doc.panels[index];
    let (plan, ranges) = plan_with_camera_layers(&crate::espace3d::Monde::of(doc), panel, frame, None, &camera, doc.repere_du_plan(index, frame).as_ref(), doc.fps, annotations);
    let keep = separable_layers(panel, annotations);
    ranges.into_iter().filter(|(li, r)| keep.contains(li) && !r.is_empty()).map(|(li, r)| (li, Plan { paper: None, items: plan.items[r].to_vec() })).collect()
}

/// Comme `plan_with_camera`, avec pour chaque calque traité (dans l'ordre de pose) la plage de ses éléments dans `Plan::items`.
fn plan_with_camera_layers(world: &crate::espace3d::Monde, panel: &Panel, frame: usize, paper: Option<[u8; 3]>, camera: &CameraSeen, repere: Option<&crate::espace3dmath::M4>, fps: u32, annotations: bool) -> (Plan, Vec<(usize, std::ops::Range<usize>)>) {
    let mut items: Vec<PlanItem> = Vec::new();
    // Pour chaque calque, le nombre d'éléments posés avant lui (un calque d'effet couvre ceux d'en dessous).
    let mut starts: Vec<usize> = vec![0; panel.layers.len()];
    let order = crate::calqueannot::ordre(&panel.layers, annotations);
    // Plan en Espace 3D (`docs/ESPACE-LIBRE.md`, A4) : du plus loin de la caméra au plus près ; sinon l'ordre de la pile.
    let order = if panel.espace { crate::espace3d::ordre_par_distance(world, &panel.layers, &order, frame, repere) } else { order };
    let mut portees = crate::calqueffet::Portees::new(panel.espace);
    // Les calques d'annotation du plan (`calqueannot.rs`) : absents, ou, si on les veut, posés après tous les autres, par-dessus tout.
    for &li in &order {
        let l = &panel.layers[li];
        starts[li] = items.len();
        portees.visiter(&panel.layers, li, items.len());
        // Caché (lui ou son dossier, 13b-5) : rien ; l'opacité du dossier s'ajoute.
        let Some(opacity) = crate::folders::effect_at(&panel.layers, li, frame) else { continue };
        // Calque d'effet (`calqueffet.rs`) : son repère, ou rien (une incrustation se pose sur l'image
        // finale, `incrustation.rs`).
        if l.effet.is_some() {
            items.extend(portees.marker(&panel.layers, li, frame, opacity, &starts));
            continue;
        }
        // Derrière la caméra : rien. Sinon l'affinité d'avant (carte à plat) ou le maillage d'une carte inclinée
        // de l'Espace 3D (`espace3d::place`).
        // (Plan en Espace 3D : `repere`, le changement de repère de sa caméra ; sinon rien, le chemin de toujours.)
        let Some(placed) = crate::espace3d::place_dans(world, l, frame, camera, repere) else { continue };
        // Couleur unie du calque (`couleurunie.rs`) : sous ses dessins, même s'il n'en a pas à cette image.
        items.extend(unie_item(l, (opacity * 255.0).round() as u8));
        // Calque vidéo (13k) : l'image de la vidéo qui tombe là.
        let Some(cel) = crate::video::cel_at(l, frame, fps) else { continue };
        let line = PlanItem {
            cel,
            opacity: (opacity * 255.0).round() as u8,
            tint: None,
            transform: placed.transform,
            blend: l.blend,
            fx: crate::fx::layer_steps(l, frame, None),
            ctg: None,
            fond: None,
            pre: placed.pre,
        };
        // Le remplissage au griffonnage (calque de couleur, `griffonnage.rs`) puis le fond sous le trait (`fond.rs`),
        // juste sous le dessin de son calque.
        items.extend(griffon_item(&panel.layers, li, frame, &line));
        items.extend(fond_item(l, &line));
        items.push(line);
    }
    let ranges = order.iter().enumerate().map(|(k, &li)| (li, starts[li]..order.get(k + 1).map_or(items.len(), |&next| starts[next]))).collect();
    (Plan { paper, items }, ranges)
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
    thumbnail_marge_until(doc, plan, cam, tw, th, 0, None, cancelled)
}

/// Comme `thumbnail_until`, avec `marge` pixels de plus tout autour du cadre (l'image fait `tw + 2 marge` sur `th + 2 marge`) :
/// là, seul le dessin est composé, sans le papier (transparent ailleurs). La planche des Vignettes s'en sert pour montrer un
/// peu du trait qui déborde de la case. `masque` : un polygone convexe (en points du cadre filmé) hors duquel le papier ne se voit pas
/// non plus, comme dans la marge : la tuile d'un mouvement de caméra ne montre le papier que dans son enveloppe.
#[allow(clippy::too_many_arguments)]
pub fn thumbnail_marge_until(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize, marge: usize, masque: Option<&[egui::Pos2]>, cancelled: &dyn Fn() -> bool) -> Option<ColorImage> {
    thumbnail_marges_until(doc, plan, cam, tw, th, [marge; 4], None, masque, cancelled)
}

/// Les marges d'une case de la planche, en pixels de sa vignette (`tw` × `th` pour le cadre) : **à gauche, en haut, à droite, en bas**. Au moins
/// `mini` de chaque côté (le débord des Préférences), et de quoi montrer **tout ce que le plan a de dessiné hors du cadre** (partie 131 : la
/// case montre son dessin où qu'il aille, le bord du cadre ne coupe rien sur la planche), jusqu'à `MARGE_MAX_CADRES` cadres de chaque côté et
/// `cote_max` pixels en tout (`VIGNETTE_MAX` à l'écran). `sauf` : les dessins que la case n'est pas seule à montrer (`dessins_communs`), qui
/// ne font pas s'étendre la vignette.
#[allow(clippy::too_many_arguments)]
pub fn marges_de_l_encre(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize, mini: usize, sauf: &[CelId], cote_max: usize) -> [usize; 4] {
    let egales = [mini; 4];
    if tw == 0 || th == 0 {
        return egales;
    }
    let Some(dehors) = debord_de_l_encre(doc, plan, cam, doc.frame, sauf) else { return egales };
    let [fw, fh] = doc.frame;
    let (kx, ky) = (tw as f32 / fw.max(1) as f32, th as f32 / fh.max(1) as f32);
    // Ce qui dépasse de chaque côté, en pixels de la vignette (un de plus : l'arrondi et le lissage du bord).
    let voulu = [dehors[0] * kx, dehors[1] * ky, dehors[2] * kx, dehors[3] * ky].map(|v| if v > 0.0 { v.min(1e7).ceil() as usize + 1 } else { 0 });
    let borne = |v: usize, cote: usize| v.max(mini).min((cote * MARGE_MAX_CADRES).max(mini));
    let mut m = [borne(voulu[0], tw), borne(voulu[1], th), borne(voulu[2], tw), borne(voulu[3], th)];
    // L'image entière garde une taille raisonnable : ce qui dépasse la marge ordinaire rétrécit, les deux côtés d'un même sens ensemble. **La
    // marge ordinaire, elle, reste** : avant la partie 131 l'image faisait déjà `cote + 2 mini`, quelle que soit sa taille (une tuile d'export).
    for (a, b, cote) in [(0, 2, tw), (1, 3, th)] {
        let place = cote_max.saturating_sub(cote + 2 * mini);
        let (ea, eb) = (m[a] - mini, m[b] - mini);
        let somme = ea + eb;
        if somme > place {
            m[a] = mini + ea * place / somme;
            m[b] = mini + eb * place / somme;
        }
    }
    m
}

/// Ce que le plan a de dessiné **hors du cadre** filmé par `cam` (un cadre de `frame` pixels), en pixels du cadre, de chaque côté : à gauche,
/// en haut, à droite, en bas (0 : rien ne dépasse de ce côté). D'après les tuiles dessinées de ses dessins : quelques microsecondes, aucun pixel
/// lu. Une caméra basculée ou en panoramique : la boîte de l'encre vue à travers elle ; si un de ses coins passe derrière la caméra, l'encre
/// peut se voir partout autour du cadre (une valeur infinie de chaque côté : les marges ont leur borne). Rien pour un plan sans encre.
/// `sauf` : les dessins qui ne comptent pas (`dessins_communs`).
pub fn debord_de_l_encre(doc: &Document, plan: &Plan, cam: &Camera, frame: [usize; 2], sauf: &[CelId]) -> Option<[f32; 4]> {
    let encre = plan_ink_bounds_sauf(doc, plan, sauf)?;
    let [fw, fh] = frame;
    if !cam.is_level() {
        let h = cam.homography_inverse(frame);
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (x, y) in [(encre.x0, encre.y0), (encre.x1, encre.y0), (encre.x1, encre.y1), (encre.x0, encre.y1)] {
            let (x, y) = (x as f32, y as f32);
            let w = h[6] * x + h[7] * y + h[8];
            let (u, v) = ((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w);
            if w <= 1e-6 || !u.is_finite() || !v.is_finite() {
                return Some([f32::INFINITY; 4]);
            }
            (x0, y0, x1, y1) = (x0.min(u), y0.min(v), x1.max(u), y1.max(v));
        }
        return Some([-x0, -y0, x1 - fw as f32, y1 - fh as f32].map(|v| v.max(0.0)));
    }
    // Du cadre filmé à la feuille : une similitude (zoom, roulis, décalage). Son inverse, d'après trois points.
    let (o, i, j) = (cam.to_stage(frame, 0.0, 0.0), cam.to_stage(frame, 1.0, 0.0), cam.to_stage(frame, 0.0, 1.0));
    let vers_feuille = Affine { a: i.x - o.x, b: j.x - o.x, c: i.y - o.y, d: j.y - o.y, tx: o.x, ty: o.y };
    let vers_cadre = vers_feuille.inverse()?;
    let coins = [(encre.x0, encre.y0), (encre.x1, encre.y0), (encre.x1, encre.y1), (encre.x0, encre.y1)].map(|(x, y)| vers_cadre.apply(egui::pos2(x as f32, y as f32)));
    if !coins.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        return None;
    }
    let (x0, y0) = coins.iter().fold((f32::MAX, f32::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
    let (x1, y1) = coins.iter().fold((f32::MIN, f32::MIN), |(x, y), p| (x.max(p.x), y.max(p.y)));
    Some([-x0, -y0, x1 - fw as f32, y1 - fh as f32].map(|v| v.max(0.0)))
}

/// Jusqu'où la vignette d'une case s'étend pour montrer le dessin hors de son cadre : ce nombre de cadres de chaque côté, au plus.
pub const MARGE_MAX_CADRES: usize = 4;
/// Le plus grand côté d'une vignette de case, marges comprises (pixels), à l'écran.
pub const VIGNETTE_MAX: usize = 4096;
/// Le plus grand côté de la marge d'une case dans l'export de la planche (pixels) : ses tuiles sont plus grandes qu'à l'écran.
pub const MARGE_EXPORT_MAX: usize = 12288;

/// Le plan sans les dessins `sauf` (`dessins_communs`) : ce qu'une case de la planche montre au-delà de sa marge ordinaire. Rien s'il n'y a
/// rien à retirer.
fn plan_propre(plan: &Plan, sauf: &[CelId]) -> Option<Plan> {
    plan.items.iter().any(|i| sauf.contains(&i.cel)).then(|| Plan { paper: plan.paper, items: plan.items.iter().filter(|i| !sauf.contains(&i.cel)).cloned().collect() })
}

/// Le point (`x`, `y`) d'une vignette de `tw` × `th` pixels de cadre, de marges `marges`, est-il **au-delà de la marge ordinaire** (`mini`) ?
fn au_dela(x: usize, y: usize, tw: usize, th: usize, marges: [usize; 4], mini: usize) -> bool {
    x + mini < marges[0] || x >= marges[0] + tw + mini || y + mini < marges[1] || y >= marges[1] + th + mini
}

/// Comme `thumbnail_marge_until`, avec une marge par côté (`marges` : à gauche, en haut, à droite, en bas ; l'image fait
/// `tw + gauche + droite` sur `th + haut + bas`). `propre` : les dessins que la case n'est pas seule à montrer (`dessins_communs`) et sa
/// marge ordinaire, en pixels ; **au-delà de cette marge, ces dessins ne sont pas composés** (partie 131 : seul ce qui est à la case seule
/// se voit loin de son cadre).
#[allow(clippy::too_many_arguments)]
pub fn thumbnail_marges_until(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize, marges: [usize; 4], propre: Option<(&[CelId], usize)>, masque: Option<&[egui::Pos2]>, cancelled: &dyn Fn() -> bool) -> Option<ColorImage> {
    #[cfg(test)]
    THUMBNAILS_HERE.with(|n| n.set(n.get() + 1));
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    let [fw, fh] = doc.frame;
    let (sx, sy) = (fw as f32 / tw as f32, fh as f32 / th as f32);
    let map = cam.frame_map(doc.frame);
    let (ow, oh) = (tw + marges[0] + marges[2], th + marges[1] + marges[3]);
    let (mx, m) = (marges[0] as f32, marges[1] as f32);
    let mut pixels = Vec::with_capacity(ow * oh);
    // (Les dessins affichés se cherchent une fois, pas à chaque échantillon : 80 000 par vignette.)
    let layers = resolve(doc, plan);
    // (Au-delà de la marge ordinaire : sans les dessins communs. Seulement si une marge va plus loin qu'elle.)
    let mini = propre.map_or(0, |p| p.1);
    let seul = propre.filter(|(_, mini)| marges.iter().any(|v| v > mini)).and_then(|(sauf, _)| plan_propre(plan, sauf));
    let loin = seul.as_ref().map(|p| resolve(doc, p));
    let paper = paper_color(plan);
    // Une case de la planche (avec sa marge) se lit comme le dessin vivant de la case courante (`gpu.rs`, `fs_display`) : plus de lectures quand on
    // réduit fort, moyennées en lumière linéaire. (En 4 lectures moyennées en gamma, un trait fin était deux fois plus foncé ici que dans
    // la case qu'on dessine : le dernier trait tracé y paraissait pâle à côté des autres, partie 111.)
    let comme_le_vivant = marges.iter().any(|v| *v > 0);
    let n: usize = if comme_le_vivant { ((sx.max(sy) / 2.5).ceil() as usize).clamp(2, 4) } else { 2 };
    let hauteurs: Vec<f32> = (0..n).map(|k| (k as f32 + 0.5) / n as f32).collect();
    let mut lus: Vec<Color32> = Vec::with_capacity(n * n);
    for y in 0..oh {
        if cancelled() {
            return None;
        }
        // (Le masque : à chaque hauteur d'échantillon, l'intervalle en x où le papier se voit.)
        let bandes: Vec<_> = hauteurs.iter().map(|oy| masque.map(|poly| crate::vignettecam::intervalle_a(poly, (y as f32 - m + oy) * sy))).collect();
        for x in 0..ow {
            lus.clear();
            let layers = match &loin {
                Some(l) if au_dela(x, y, tw, th, marges, mini) => l,
                _ => &layers,
            };
            for (j, oy) in hauteurs.iter().enumerate() {
                for ox in &hauteurs {
                    // (Dans la marge, le papier ne se voit pas : seulement ce qui est dessiné.)
                    let (fx, fy) = ((x as f32 - mx + ox) * sx, (y as f32 - m + oy) * sy);
                    let dans_masque = match bandes[j] {
                        None => true,
                        Some(None) => false,
                        Some(Some((lo, hi))) => fx >= lo && fx <= hi,
                    };
                    let base = if dans_masque && fx >= 0.0 && fy >= 0.0 && fx < fw as f32 && fy < fh as f32 { paper } else { Color32::TRANSPARENT };
                    // (Toile infinie : le cadre peut filmer hors de la feuille.)
                    lus.push(match map.at(fx, fy) {
                        Some(p) if p.x.abs() < WORLD as f32 && p.y.abs() < WORLD as f32 => pixel_of(layers, base, p.x.floor() as i64, p.y.floor() as i64),
                        _ => Color32::TRANSPARENT,
                    });
                }
            }
            pixels.push(if comme_le_vivant { moyenne_sur_le_papier(&lus) } else { moyenne_gamma(&lus) });
        }
    }
    Some(ColorImage {
        size: [ow, oh],
        pixels,
    })
}

/// La moyenne de pixels prémultipliés, en gamma.
fn moyenne_gamma(lus: &[Color32]) -> Color32 {
    let mut acc = [0u32; 4];
    for c in lus {
        for (a, v) in acc.iter_mut().zip(c.to_array()) {
            *a += v as u32;
        }
    }
    let n = lus.len().max(1) as u32;
    Color32::from_rgba_premultiplied((acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8, (acc[3] / n) as u8)
}

fn lineaire_de(v: u8) -> f32 {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| egui::ecolor::linear_f32_from_gamma_u8(i as u8)))[v as usize]
}

/// La moyenne de pixels prémultipliés telle qu'elle se voit **posée sur le papier blanc de la planche**, en lumière linéaire, comme le dessin
/// vivant. Ce qui n'est pas opaque (la marge d'une case) est rendu en couleur et opacité telles que, posé sur le papier par egui (qui mêle en
/// gamma), il donne exactement cette couleur : même trait dedans et dehors.
pub(crate) fn moyenne_sur_le_papier(lus: &[Color32]) -> Color32 {
    let Some(premier) = lus.first().copied() else { return Color32::TRANSPARENT };
    // (Le plus souvent : du papier, ou rien.)
    if matches!(premier.a(), 0 | 255) && lus.iter().all(|c| *c == premier) {
        return premier;
    }
    let (mut l, mut a) = ([0.0_f32; 3], 0u32);
    for c in lus {
        let reste = 255 - c.a();
        for (s, v) in l.iter_mut().zip([c.r(), c.g(), c.b()]) {
            *s += lineaire_de(v.saturating_add(reste));
        }
        a += u32::from(c.a());
    }
    let n = lus.len() as f32;
    let t = l.map(|v| egui::ecolor::gamma_u8_from_linear_f32(v / n));
    if a == 255 * lus.len() as u32 {
        return Color32::from_rgb(t[0], t[1], t[2]);
    }
    let bas = t[0].min(t[1]).min(t[2]);
    Color32::from_rgba_premultiplied(t[0] - bas, t[1] - bas, t[2] - bas, 255 - bas)
}

/// La marge seule d'une case de `tw` × `th` pixels : `marge` pixels tout autour, où se voit ce qui est dessiné (pas le papier) ; le dedans reste
/// transparent et ne coûte rien. L'export de la planche (les images, elles, viennent du rendu de l'export). `cancelled` est demandé entre deux lignes : vrai, le calcul s'arrête et rend `None`.
/// `propre` : comme `thumbnail_marges_until`.
#[allow(clippy::too_many_arguments)]
pub fn marge_seule_until(doc: &Document, plan: &Plan, cam: &Camera, tw: usize, th: usize, marges: [usize; 4], propre: Option<(&[CelId], usize)>, cancelled: &dyn Fn() -> bool) -> Option<ColorImage> {
    let resolved = crate::calqueffet::resolve_auto(doc, plan);
    let plan = &*resolved;
    let [fw, fh] = doc.frame;
    let (sx, sy) = (fw as f32 / tw.max(1) as f32, fh as f32 / th.max(1) as f32);
    let map = cam.frame_map(doc.frame);
    let (ow, oh) = (tw + marges[0] + marges[2], th + marges[1] + marges[3]);
    let (mx, m) = (marges[0] as f32, marges[1] as f32);
    let mut pixels = vec![Color32::TRANSPARENT; ow * oh];
    let layers = resolve(doc, plan);
    let mini = propre.map_or(0, |p| p.1);
    let seul = propre.filter(|(_, mini)| marges.iter().any(|v| v > mini)).and_then(|(sauf, _)| plan_propre(plan, sauf));
    let loin = seul.as_ref().map(|p| resolve(doc, p));
    for y in 0..oh {
        if cancelled() {
            return None;
        }
        for x in 0..ow {
            if (marges[0]..marges[0] + tw).contains(&x) && (marges[1]..marges[1] + th).contains(&y) {
                continue;
            }
            let layers = match &loin {
                Some(l) if au_dela(x, y, tw, th, marges, mini) => l,
                _ => &layers,
            };
            let lus = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)].map(|(ox, oy)| match map.at((x as f32 - mx + ox) * sx, (y as f32 - m + oy) * sy) {
                Some(p) if p.x.abs() < WORLD as f32 && p.y.abs() < WORLD as f32 => pixel_of(layers, Color32::TRANSPARENT, p.x.floor() as i64, p.y.floor() as i64),
                _ => Color32::TRANSPARENT,
            });
            pixels[y * ow + x] = moyenne_gamma(&lus);
        }
    }
    Some(ColorImage { size: [ow, oh], pixels })
}

/// Le dessin entier réduit à `max_w` pixels de large au plus (moyenne de 4 échantillons par
/// pixel) : texture des plans de la vue du plateau en 3D (9e).
pub fn cel_image(raster: &Raster, max_w: usize) -> ColorImage {
    let [tw, th] = cel_image_size(raster, max_w);
    cel_image_zone(raster, max_w, [0, 0, tw, th])
}

/// La taille de l'image que `cel_image` fait de ce dessin, au plus large de `max_w` pixels.
pub fn cel_image_size(raster: &Raster, max_w: usize) -> [usize; 2] {
    let (w, h) = (raster.width.max(1), raster.height.max(1));
    let tw = w.min(max_w.max(1));
    [tw, ((h as f32 * tw as f32 / w as f32).round() as usize).max(1)]
}

/// Un morceau de l'image de `cel_image` : la zone `[x0, y0, x1, y1]` de ses pixels (bornes hautes exclues), mêmes
/// pixels que l'image entière. Sert à rafraîchir seulement ce qu'un trait vient de changer (`espace3ddessin.rs`).
pub fn cel_image_zone(raster: &Raster, max_w: usize, zone: [usize; 4]) -> ColorImage {
    let (w, h) = (raster.width.max(1), raster.height.max(1));
    let [tw, th] = cel_image_size(raster, max_w);
    let (sx, sy) = (w as f32 / tw as f32, h as f32 / th as f32);
    let (x0, y0) = (zone[0].min(tw), zone[1].min(th));
    let (zw, zh) = (zone[2].min(tw).saturating_sub(x0), zone[3].min(th).saturating_sub(y0));
    let mut pixels = vec![Color32::TRANSPARENT; zw * zh];
    if pixels.is_empty() {
        return ColorImage { size: [zw, zh], pixels };
    }
    const ROWS: usize = 16;
    par_chunks(&mut pixels, zw * ROWS, |chunk, rows| {
        for (k, px) in rows.iter_mut().enumerate() {
            let (x, y) = (x0 + k % zw, y0 + chunk * ROWS + k / zw);
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
    ColorImage { size: [zw, zh], pixels }
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

    /// Des pixels ont pu changer partout (dessins, structure du plan) : tout se renvoie à la carte graphique et le
    /// hors-feuille se refait.
    /// L'image que le processeur a composée à la dernière mise à jour (pour les essais : ce que l'écran montre sans carte graphique).
    #[cfg(test)]
    pub(crate) fn composee(&self) -> &[Color32] {
        &self.buffer
    }

    pub fn invalidate_all(&mut self) {
        self.dirty_all = true;
        self.gpu_all = true;
        self.world_all = true;
    }

    /// Seul un réglage a changé, aucun pixel d'aucun dessin : opacité, mode de fusion, clé de position ou d'effet, exposition,
    /// fin du plan (ce qui est montré à cette image). Tout cela est dans le plan à composer (`Plan`), que la carte graphique
    /// compare à celui de l'image gardée (`ItemKey`) et le hors-feuille au sien (`monde.rs`) : rien n'est renvoyé à la carte,
    /// le hors-feuille ne se refait que si le réglage touche un calque qui dépasse de la feuille (`monde::WorldKey`), et
    /// alors sur un autre fil. Seule l'image du processeur (sans carte graphique) se recompose. Pendant
    /// qu'on tire une opacité, `invalidate_all` renvoyait à la carte toutes les tuiles de tous les dessins du plan, à chaque
    /// image du geste (+3 à 5 ms sur 15 calques de 40 tuiles, `perf_tests::perf_parameter_drag`).
    pub fn invalidate_params(&mut self) {
        self.dirty_all = true;
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
