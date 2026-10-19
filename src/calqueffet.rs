//! Calques d'effets (Anthony, 2026-10-19 : « création de calques d'effets après des préréglages
//! d'animation d'effet comme des impact frames, glows, tremblements de caméra effectifs, timecode »).
//!
//! Un calque d'effet est un calque ordinaire de la pile (`Layer::effet`), sans dessin à lui :
//! - **Ajustement** (comme un calque de réglage de Photoshop ou d'After Effects) : sa pile d'effets
//!   (`Layer::fx`, avec ses clés) s'applique à TOUT ce qui est dessous dans le plan, ou dans son
//!   dossier s'il est rangé dans un dossier. Il a une opacité, un mode de fusion, un œil, et des
//!   images comme les autres calques : il agit aux images où il a une exposition (un dessin vide
//!   tenu), en ligne droite entre ses clés d'effets.
//! - **Incrustation** (timecode, numéro d'image, nom du plan…) : du texte posé sur l'image finale,
//!   dans le cadre de la caméra (`incrustation.rs`).
//!
//! # Rendu
//!
//! Un calque d'ajustement actif pose dans le plan (`render::Plan`) un élément repère
//! (`FondPart::Reglage`, voir `marker`) juste après les éléments de ce qu'il couvre. `resolve`
//! remplace le repère et ce qu'il couvre par UN dessin : la composition de ce qui est dessous
//! (papier compris pour un calque au plus bas), passée par la pile d'effets, puis fondue sur elle
//! avec l'opacité et le mode du calque. Ce dessin dérivé est gardé par `fxcache.rs` comme ceux des
//! effets d'un calque : à l'écran il est demandé au fil de fond (jamais calculé sur le fil de
//! l'interface), à la taille de l'aperçu pendant la lecture et les gestes ; à l'export et dans les
//! essais il est calculé tout de suite, à pleine résolution (`Mode::Blocking`). En attendant le
//! résultat l'image se montre sans l'effet. La carte graphique et le processeur composent le même
//! dessin dérivé : ils ne peuvent pas différer.
//!
//! **Sur la carte graphique** (`gpufx.rs`, réglage allumé par défaut), l'écran ne résout pas les repères des
//! calques hors dossier dont toute la pile a des passes : la carte applique la pile à l'image composée jusque-là,
//! à pleine résolution, à chaque image, et pose le résultat avec l'opacité et le mode du calque. Le chemin
//! ci-dessus sert alors à l'export, aux vignettes, aux calques dans un dossier et quand le réglage est éteint.

use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui::{pos2, Color32};
use serde::{Deserialize, Serialize};

use crate::blend::{blend_over, BlendMode};
use crate::document::{Cel, CelId, Document, Layer, LayerKind, LayerTransform};
use crate::fx::FxStep;
use crate::incrustation::Incrustation;
use crate::raster::{empty_tile, tile_bounds, Raster, TILE, TILE_PIXELS};
use crate::render::{FondPart, Plan, PlanItem};

/// Ce que fait un calque d'effet.
#[derive(Clone, Debug, PartialEq)]
pub enum Contenu {
    /// La pile d'effets du calque s'applique à ce qui est dessous.
    Ajustement,
    /// Du texte posé sur l'image finale.
    Incrustation(Incrustation),
}

/// Le réglage d'un calque d'effet : ce qu'il fait, et le préréglage d'où il vient (pour le nom
/// affiché ; vide : un calque d'effet vide, réglé à la main).
#[derive(Clone, Debug, PartialEq)]
pub struct CalqueEffet {
    pub contenu: Contenu,
    pub prereglage: String,
}

impl CalqueEffet {
    pub fn ajustement(prereglage: &str) -> CalqueEffet {
        CalqueEffet { contenu: Contenu::Ajustement, prereglage: prereglage.to_string() }
    }

    pub fn incrustation(i: Incrustation, prereglage: &str) -> CalqueEffet {
        CalqueEffet { contenu: Contenu::Incrustation(i), prereglage: prereglage.to_string() }
    }
}

// ---------------------------------------------------------------------- enregistrement (format 27)

/// Un calque d'effet dans le manifeste du projet (`calque_effet` d'un calque, format 27). Les
/// effets, eux, sont ceux de la pile du calque (`effets`), comme pour tout calque.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entree {
    /// « ajustement » ou « incrustation ».
    pub sorte: String,
    /// Le préréglage d'où il vient (nom affiché).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prereglage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incrustation: Option<IncrustationEntree>,
}

/// Les réglages d'une incrustation, enregistrés.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IncrustationEntree {
    pub contenu: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub texte: String,
    pub compte: String,
    pub place: String,
    pub taille: f32,
    pub couleur: [u8; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boite: Option<[u8; 4]>,
    pub marge: f32,
    pub visible: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub police: String,
}

impl From<&CalqueEffet> for Entree {
    fn from(e: &CalqueEffet) -> Entree {
        match &e.contenu {
            Contenu::Ajustement => Entree { sorte: "ajustement".into(), prereglage: e.prereglage.clone(), incrustation: None },
            Contenu::Incrustation(i) => Entree {
                sorte: "incrustation".into(),
                prereglage: e.prereglage.clone(),
                incrustation: Some(IncrustationEntree {
                    contenu: i.source.name().into(),
                    texte: i.texte.clone(),
                    compte: i.portee.name().into(),
                    place: i.ancre.name().into(),
                    taille: i.taille,
                    couleur: i.couleur,
                    boite: i.fond,
                    marge: i.marge,
                    visible: i.quand.name().into(),
                    police: i.police.clone(),
                }),
            },
        }
    }
}

impl Entree {
    /// Le calque d'effet enregistré, ramené dans des bornes sûres (un projet abîmé ou d'une autre
    /// version : une incrustation qu'on ne comprend pas devient un timecode, un nom inconnu reprend la
    /// valeur proposée).
    pub fn into_model(self) -> CalqueEffet {
        let contenu = match (self.sorte.as_str(), self.incrustation) {
            ("incrustation", e) => {
                let d = Incrustation::default();
                Contenu::Incrustation(match e {
                    None => d,
                    Some(e) => {
                        let finite = |v: f32, lo: f32, hi: f32, dv: f32| if v.is_finite() { v.clamp(lo, hi) } else { dv };
                        Incrustation {
                            source: crate::incrustation::Source::from_name(&e.contenu).unwrap_or(d.source),
                            texte: e.texte.chars().take(500).collect(),
                            portee: crate::incrustation::Portee::from_name(&e.compte).unwrap_or(d.portee),
                            ancre: crate::incrustation::Ancre::from_name(&e.place).unwrap_or(d.ancre),
                            taille: finite(e.taille, 0.5, 60.0, d.taille),
                            couleur: e.couleur,
                            fond: e.boite,
                            marge: finite(e.marge, 0.0, 40.0, d.marge),
                            quand: crate::incrustation::Quand::from_name(&e.visible).unwrap_or(d.quand),
                            police: e.police,
                        }
                    }
                })
            }
            _ => Contenu::Ajustement,
        };
        CalqueEffet { contenu, prereglage: self.prereglage }
    }
}

impl Layer {
    /// Calque d'effet neuf : un calque bitmap sans dessin, ses images sont ses expositions.
    pub fn new_effect(name: impl Into<String>, effet: CalqueEffet) -> Layer {
        Layer { effet: Some(effet), ..Layer::new(name, LayerKind::Bitmap) }
    }

    /// Calque d'effet d'ajustement (pile d'effets sur ce qui est dessous).
    pub fn is_adjustment(&self) -> bool {
        matches!(self.effet, Some(CalqueEffet { contenu: Contenu::Ajustement, .. }))
    }

    /// L'incrustation d'un calque d'incrustation.
    pub fn overlay(&self) -> Option<&Incrustation> {
        match &self.effet {
            Some(CalqueEffet { contenu: Contenu::Incrustation(i), .. }) => Some(i),
            _ => None,
        }
    }

    /// Le calque d'effet agit à l'image `frame` : une exposition la couvre.
    pub fn effect_active(&self, frame: usize) -> bool {
        self.effet.is_some() && self.cel_at(frame).is_some()
    }
}

// ---------------------------------------------------------------------- le repère du plan

/// Ce que le repère d'un calque d'ajustement dit au plan (`FondPart::Reglage`).
#[derive(Clone, Debug, PartialEq)]
pub struct Reglage {
    /// Le calque d'effet (son numéro dans le plan).
    pub layer: u64,
    /// Rang, dans la liste des éléments du plan, du premier élément qu'il couvre ; il couvre tout
    /// ce qui va de là jusqu'à lui.
    pub from: usize,
    /// Le calque est hors dossier : le papier du plan est dans ce qu'il couvre.
    pub root: bool,
    /// Ses effets à cette image.
    pub steps: Vec<FxStep>,
    /// Opacité (0 à 255) et mode de fusion du calque.
    pub opacity: u8,
    pub blend: BlendMode,
}

/// Numéro d'« origine » (dessin d'avant, pour garder le dernier résultat) d'un calque d'effet : hors
/// des numéros de dessins du projet.
fn origin_of(layer: u64) -> CelId {
    (1 << 62) | layer
}

/// Rang du calque le plus bas que le calque `li` couvre : le plus bas de son dossier, ou le tout
/// premier.
pub fn scope_low(layers: &[Layer], li: usize) -> usize {
    match crate::folders::parent_index(layers, li) {
        Some(fi) => crate::folders::children_of(layers, fi).first().copied().unwrap_or(li),
        None => 0,
    }
}

/// L'élément repère à poser dans le plan pour le calque `li` à l'image `frame` s'il agit (calque
/// d'ajustement, une exposition à cette image, au moins un effet qui change quelque chose). `starts`
/// : pour chaque calque d'en dessous, le nombre d'éléments déjà posés quand il a commencé.
/// `opacity` : celle du calque, multipliée par celle de son dossier (`folders::effect`).
pub fn marker(layers: &[Layer], li: usize, frame: usize, opacity: f32, starts: &[usize]) -> Option<PlanItem> {
    let l = layers.get(li)?;
    if !l.is_adjustment() || !l.effect_active(frame) {
        return None;
    }
    let steps = crate::fx::layer_steps(l, frame, None);
    if steps.is_empty() {
        return None;
    }
    let low = scope_low(layers, li);
    let reglage = Reglage {
        layer: l.id,
        from: starts.get(low).copied().unwrap_or(0),
        root: crate::folders::parent_index(layers, li).is_none(),
        steps,
        opacity: (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
        blend: l.blend,
    };
    Some(PlanItem {
        cel: origin_of(l.id),
        opacity: 255,
        tint: None,
        transform: None,
        blend: BlendMode::Normal,
        fx: Vec::new(),
        ctg: None,
        fond: Some(FondPart::Reglage(Arc::new(reglage))),
        pre: None,
    })
}

/// Le plan a-t-il un repère de calque d'effet à résoudre ?
pub fn has_markers(plan: &Plan) -> bool {
    plan.items.iter().any(|i| matches!(i.fond, Some(FondPart::Reglage(_))))
}

// ---------------------------------------------------------------------- la photo de ce qui est dessous

/// Un élément couvert, tel que la composition le lit : son dessin affiché (effets compris) et comment
/// il se pose.
#[derive(Clone)]
pub struct Low {
    pub raster: Raster,
    pub opacity: u8,
    pub tint: Option<[u8; 3]>,
    pub transform: Option<LayerTransform>,
    pub blend: BlendMode,
}

/// Tout ce qu'il faut pour calculer, sans le projet, le dessin d'un calque d'ajustement : les dessins
/// couverts (des copies légères : les tuiles sont partagées), le papier, et les effets. C'est ce
/// que reçoit le fil de fond (`fxcache::Job::base`).
pub struct Snapshot {
    pub width: usize,
    pub height: usize,
    pub paper: Option<[u8; 3]>,
    pub lows: Vec<Low>,
    pub steps: Vec<FxStep>,
    pub opacity: u8,
    pub blend: BlendMode,
}

impl Snapshot {
    /// Le dessin du calque d'ajustement, à l'échelle `1 >> level` : la composition, ses effets, puis
    /// la fusion avec la composition.
    pub fn render(&self, level: u8) -> Raster {
        let base = self.compose(level);
        let effected = crate::fx::apply_scaled(&base, &self.steps, level);
        blend_onto(&base, &effected, self.opacity, self.blend)
    }

    /// Ce qui est dessous, composé : le papier (s'il y en a un), puis chaque élément dans l'ordre,
    /// à l'échelle `1 / (1 << level)` (aperçu).
    pub fn compose(&self, level: u8) -> Raster {
        let f = 1usize << level;
        let (tw, th) = (self.width.div_ceil(f), self.height.div_ceil(f));
        let mut out = Raster::new(tw, th);
        let tiles_x = out.tiles_x.max(1);
        let base = self.paper.map_or(Color32::TRANSPARENT, |p| Color32::from_rgb(p[0], p[1], p[2]));
        let pivot = pos2(self.width as f32 / 2.0, self.height as f32 / 2.0);
        // Chaque élément à la taille de la composition (réduit, si besoin), avec sa transformation.
        struct Prepared<'a> {
            raster: Cow<'a, Raster>,
            inv: Option<crate::vector::Affine>,
            kx: f32,
            ky: f32,
            low: &'a Low,
        }
        let prepared: Vec<Prepared> = self
            .lows
            .iter()
            .filter_map(|low| {
                let r = &low.raster;
                let raster = if r.width == tw && r.height == th {
                    Cow::Borrowed(r)
                } else if level > 0 && r.width == self.width && r.height == self.height {
                    Cow::Owned(crate::fxproxy::downscale(r, level))
                } else {
                    Cow::Borrowed(r)
                };
                let inv = match low.transform {
                    Some(t) => Some(t.affine(pivot).inverse()?),
                    None => None,
                };
                let (kx, ky) = (raster.width as f32 / tw.max(1) as f32, raster.height as f32 / th.max(1) as f32);
                Some(Prepared { raster, inv, kx, ky, low })
            })
            .collect();
        let has_paper = self.paper.is_some();
        let fl = f as f32;
        crate::render::par_chunks(&mut out.tiles, tiles_x, |row, tiles| {
            for (tx, slot) in tiles.iter_mut().enumerate() {
                let idx = row * tiles_x + tx;
                let (ox, oy, w, h) = tile_bounds(tw, th, idx);
                let mut tile = empty_tile();
                let px: &mut [Color32; TILE_PIXELS] = &mut tile;
                if has_paper {
                    for y in 0..h {
                        px[y * TILE..y * TILE + w].fill(base);
                    }
                }
                for p in &prepared {
                    let low = p.low;
                    // Un dessin à la taille de la composition, posé sans transformation : tuile pour tuile.
                    if p.inv.is_none() && p.kx == 1.0 && p.ky == 1.0 {
                        let Some(src) = p.raster.tiles.get(idx).and_then(Option::as_ref) else { continue };
                        for y in 0..h {
                            for x in 0..w {
                                let s = src[y * TILE + x];
                                if s.a() != 0 {
                                    px[y * TILE + x] = blend_over(px[y * TILE + x], s, low.opacity, low.tint, low.blend, (ox + x, oy + y));
                                }
                            }
                        }
                        continue;
                    }
                    for y in 0..h {
                        for x in 0..w {
                            let (cx, cy) = ((ox + x) as f32 + 0.5, (oy + y) as f32 + 0.5);
                            let (sx, sy) = match &p.inv {
                                Some(inv) => {
                                    let q = inv.apply(pos2(cx * fl, cy * fl));
                                    (q.x / fl, q.y / fl)
                                }
                                None => (cx, cy),
                            };
                            let s = crate::render::sample(&p.raster, sx * p.kx, sy * p.ky);
                            if s.a() != 0 {
                                px[y * TILE + x] = blend_over(px[y * TILE + x], s, low.opacity, low.tint, low.blend, (ox + x, oy + y));
                            }
                        }
                    }
                }
                if has_paper || px.iter().any(|c| c.a() != 0) {
                    *slot = Some(tile);
                }
            }
        });
        out
    }
}

/// `top` (le résultat des effets) posé sur `under` (la composition) avec l'opacité et le mode du
/// calque, pixel par pixel.
fn blend_onto(under: &Raster, top: &Raster, opacity: u8, mode: BlendMode) -> Raster {
    let mut out = under.clone();
    if opacity == 0 {
        return out;
    }
    let tiles_x = out.tiles_x.max(1);
    let (w, h) = (out.width, out.height);
    crate::render::par_chunks(&mut out.tiles, tiles_x, |row, tiles| {
        for (tx, slot) in tiles.iter_mut().enumerate() {
            let idx = row * tiles_x + tx;
            let Some(src) = top.tiles.get(idx).and_then(Option::as_ref) else { continue };
            let (ox, oy, tw, th) = tile_bounds(w, h, idx);
            let mut tile = slot.take().unwrap_or_else(empty_tile);
            {
                let px: &mut [Color32; TILE_PIXELS] = &mut tile;
                for y in 0..th {
                    for x in 0..tw {
                        let s = src[y * TILE + x];
                        if s.a() != 0 {
                            px[y * TILE + x] = blend_over(px[y * TILE + x], s, opacity, None, mode, (ox + x, oy + y));
                        }
                    }
                }
            }
            *slot = Some(tile);
        }
    });
    out
}

// ---------------------------------------------------------------------- résolution du plan

/// Comment résoudre un plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Export, enregistrement, essais : calcul tout de suite, à pleine résolution, exact.
    Blocking,
    /// Écran : jamais de calcul sur le fil appelant. Ce qui n'est pas prêt est demandé au fil de
    /// fond (priorité `prio`, 0 : l'image affichée) ; en attendant, l'image se montre sans l'effet
    /// (ou, pendant un aperçu, avec le dernier résultat du même calque si c'est la même image).
    Display { level: u8, prio: u32 },
    /// Images qui viennent : on demande leurs dessins (priorité `prio`), on ne rend que ceux déjà prêts.
    Ahead { level: u8, prio: u32 },
}

/// Pixels au plus d'un dessin de calque d'effet pendant l'aperçu (le million : à ce prix la composition,
/// la lueur, la secousse et la fusion d'une image tiennent dans les 41 ms d'une image à 24 i/s sur le
/// fil de fond, mesure `effetsperf_tests`).
const PROXY_PIXELS: usize = 1_000_000;

/// Le niveau d'aperçu d'un calque d'effet. Il traite l'image entière, effets lourds compris, et la
/// lecture doit tenir 24 images par seconde : pendant l'aperçu (`level` > 0, ou un geste ou la lecture à
/// un zoom élevé), au moins la moitié de la résolution, et plus bas tant que le dessin dépasse
/// `PROXY_PIXELS` (une feuille 4K : le quart). Au repos, la pleine résolution.
pub(crate) fn proxy_level(level: u8, width: usize, height: usize) -> u8 {
    if level == 0 && !crate::fx::drafting() {
        return 0;
    }
    let mut l = level.max(1);
    while l < 3 && (width >> l) * (height >> l) > PROXY_PIXELS {
        l += 1;
    }
    l
}

/// Résout les repères du plan comme l'écran, l'export ou les vignettes le veulent : calcul tout de
/// suite, sauf dans une portée « douce » (vignettes de l'écran, `fxcache::soft_scope`).
pub fn resolve_auto<'a>(doc: &Document, plan: &'a Plan) -> Cow<'a, Plan> {
    if !has_markers(plan) {
        return Cow::Borrowed(plan);
    }
    if crate::fxcache::soft_active() {
        resolve(doc, plan, Mode::Display { level: 0, prio: SOFT_PRIO })
    } else {
        resolve(doc, plan, Mode::Blocking)
    }
}

/// Priorité des calculs demandés pour les vignettes (après l'image affichée et les suivantes).
const SOFT_PRIO: u32 = 100;

/// Le plan, ses repères de calques d'effets remplacés chacun, avec ce qu'il couvre, par le dessin
/// dérivé (`bake`). Un repère dont le dessin n'est pas prêt est retiré : ce qu'il couvrait reste.
pub fn resolve<'a>(doc: &Document, plan: &'a Plan, mode: Mode) -> Cow<'a, Plan> {
    if !has_markers(plan) {
        return Cow::Borrowed(plan);
    }
    let mut out: Vec<PlanItem> = Vec::with_capacity(plan.items.len());
    // Pour chaque élément du plan d'origine, la place qu'il prend dans `out` à son arrivée.
    let mut at: Vec<usize> = Vec::with_capacity(plan.items.len());
    for item in &plan.items {
        at.push(out.len());
        let Some(FondPart::Reglage(r)) = &item.fond else {
            out.push(item.clone());
            continue;
        };
        let start = at.get(r.from).copied().unwrap_or(0).min(out.len());
        let paper = if r.root { plan.paper } else { None };
        if let Some(derived) = bake(doc, r, &out[start..], paper, mode) {
            out.truncate(start);
            out.push(derived);
        }
    }
    Cow::Owned(Plan { paper: plan.paper, items: out })
}

/// Un élément couvert, résolu : son dessin affiché, le numéro sous lequel il est gardé.
struct Covered<'a> {
    item: &'a PlanItem,
    id: CelId,
    cel: crate::fx::Shown<'a>,
}

/// Le dessin affiché d'un élément couvert, selon le mode ; rien s'il n'est pas prêt.
fn covered<'a>(doc: &'a Document, item: &'a PlanItem, mode: Mode) -> Option<Covered<'a>> {
    match mode {
        Mode::Blocking => {
            let cel = crate::fx::shown(doc, item)?;
            Some(Covered { item, id: crate::fx::shown_id(item), cel })
        }
        Mode::Display { level, prio } => {
            let d = crate::fx::display_with(doc, item, level, prio)?;
            if !d.ready {
                crate::fxcache::mark_incomplete_if_soft();
                return None;
            }
            Some(Covered { item, id: d.id, cel: d.cel })
        }
        Mode::Ahead { level, prio } => {
            crate::fx::prefetch(doc, std::slice::from_ref(item), prio, level);
            match crate::fx::ready(doc, item, level) {
                Some((id, cel)) => Some(Covered { item, id, cel: crate::fx::Shown::Fx(cel) }),
                None => {
                    // Un dessin sans effet : celui du projet (rien à attendre).
                    let src = doc.cels.get(&item.cel).filter(|_| crate::fx::shown_id(item) == item.cel)?;
                    Some(Covered { item, id: item.cel, cel: crate::fx::Shown::Plain(src) })
                }
            }
        }
    }
}

fn hash_transform(t: &Option<LayerTransform>, h: &mut DefaultHasher) {
    match t {
        None => 0u8.hash(h),
        Some(t) => {
            1u8.hash(h);
            for v in [t.x, t.y, t.scale, t.angle, t.stretch_x, t.stretch_y] {
                v.to_bits().hash(h);
            }
        }
    }
}

fn hash_steps(steps: &[FxStep], h: &mut DefaultHasher) {
    for s in steps {
        s.kind.hash(h);
        for v in &s.values {
            v.to_bits().hash(h);
        }
    }
}

/// Signature de CE QUE le calque d'ajustement montre, sans le niveau d'aperçu ni l'état de calcul des
/// effets d'en dessous : les dessins couverts (numéros et versions), comment ils se posent, les effets
/// du calque. Elle sert de version au dessin dérivé : un résultat d'une autre taille, de la même
/// signature, est un bon remplaçant en attendant.
fn content_signature(doc: &Document, r: &Reglage, lower: &[PlanItem], paper: Option<[u8; 3]>) -> u64 {
    let mut h = DefaultHasher::new();
    (doc.width, doc.height, paper).hash(&mut h);
    hash_steps(&r.steps, &mut h);
    (r.opacity, r.blend).hash(&mut h);
    for item in lower {
        item.cel.hash(&mut h);
        doc.cels.get(&item.cel).map(|c| c.version).hash(&mut h);
        (item.opacity, item.tint, item.blend).hash(&mut h);
        hash_transform(&item.transform, &mut h);
        hash_steps(&item.fx, &mut h);
        item.ctg.hash(&mut h);
        if let Some(ctg) = &item.ctg {
            for s in &ctg.sources {
                doc.cels.get(s).map(|c| c.version).hash(&mut h);
            }
        }
        item.fond.as_ref().map(|f| match f {
            FondPart::Forme(r) => format!("{r:?}"),
            FondPart::Reglage(r) => format!("{r:?}"),
        }).hash(&mut h);
    }
    h.finish()
}

/// Le dessin dérivé d'un calque d'ajustement : `lower` (ce qu'il couvre) passé par ses effets, fondu
/// sur lui. Rend l'élément qui le montre (son numéro gardé par `fxcache`), ou rien : il n'est pas prêt
/// (alors ce qu'il couvre reste tel quel).
fn bake(doc: &Document, r: &Reglage, lower: &[PlanItem], paper: Option<[u8; 3]>, mode: Mode) -> Option<PlanItem> {
    if lower.is_empty() && paper.is_none() {
        return None;
    }
    // La feuille flottante d'une sélection en cours de transformation n'est pas un dessin gardé : pendant
    // le geste, l'effet attend.
    if lower.iter().any(|i| i.pre.is_some()) {
        crate::fxcache::mark_incomplete_if_soft();
        return None;
    }
    // L'aperçu d'un calque d'effet est plus bas que celui des effets d'un calque (`proxy_level`). Au repos,
    // la pleine résolution prend la place (`fx::draft_level`).
    let mode = match mode {
        Mode::Display { level, prio } => Mode::Display { level: proxy_level(level, doc.width, doc.height), prio },
        Mode::Ahead { level, prio } => Mode::Ahead { level: proxy_level(level, doc.width, doc.height), prio },
        Mode::Blocking => Mode::Blocking,
    };
    let (level, prio) = match mode {
        Mode::Blocking => (0, 0),
        Mode::Display { level, prio } | Mode::Ahead { level, prio } => (level, prio),
    };
    let covers: Vec<Covered> = lower.iter().map(|i| covered(doc, i, mode)).collect::<Option<_>>()?;
    let sig = content_signature(doc, r, lower, paper);
    let mut h = DefaultHasher::new();
    (sig, level, paper).hash(&mut h);
    for c in &covers {
        // (Le numéro sous lequel l'élément est gardé, sa version : le contenu qu'on lit vraiment.)
        (c.id, c.cel.version, c.cel.raster.width).hash(&mut h);
    }
    let id = h.finish() | (1 << 63);
    let origin = origin_of(r.layer);
    let item = |cel: CelId| PlanItem {
        cel,
        opacity: 255,
        tint: None,
        transform: None,
        blend: BlendMode::Normal,
        fx: Vec::new(),
        ctg: None,
        fond: None,
        pre: None,
    };
    if let Some(cel) = crate::fxcache::get(id, sig) {
        let _ = cel;
        return Some(item(id));
    }
    let snapshot = || {
        Arc::new(Snapshot {
            width: doc.width,
            height: doc.height,
            paper,
            lows: covers
                .iter()
                .map(|c| Low { raster: c.cel.raster.clone(), opacity: c.item.opacity, tint: c.item.tint, transform: c.item.transform, blend: c.item.blend })
                .collect(),
            steps: r.steps.clone(),
            opacity: r.opacity,
            blend: r.blend,
        })
    };
    match mode {
        Mode::Blocking => {
            let base = snapshot();
            crate::fxcache::get_or_compute(id, sig, Some(origin), || Cel { raster: base.render(0), vector: None, version: sig, fond: None });
            Some(item(id))
        }
        Mode::Display { .. } | Mode::Ahead { .. } => {
            // (Pendant un geste ou la lecture, les vignettes attendent : le fil de fond sert l'image affichée.)
            if prio < SOFT_PRIO || !crate::fx::drafting() {
                crate::fxcache::request(crate::fxcache::Job {
                    id,
                    version: sig,
                    src: Arc::new(Raster::new(0, 0)),
                    steps: Vec::new(),
                    level,
                    prio,
                    epoch: crate::fxcache::epoch(),
                    origin,
                    seq: 0,
                    world: crate::fxcache::world(),
                    base: Some(snapshot()),
                });
            }
            // En attendant : le dernier résultat de ce calque s'il montre la même chose (une autre taille
            // d'aperçu, un aperçu qui s'achève), sinon l'image sans l'effet.
            if matches!(mode, Mode::Display { .. }) {
                if let Some((last, cel)) = crate::fxcache::fallback(origin).filter(|(_, c)| c.version == sig) {
                    let _ = cel;
                    return Some(item(last));
                }
            }
            crate::fxcache::mark_incomplete_if_soft();
            None
        }
    }
}

/// Les dessins dérivés prêts des calques d'effets du plan, à envoyer d'avance à la carte graphique
/// (images qui viennent) ; leurs calculs sont demandés au fil de fond, priorité `prio`.
pub fn ahead(doc: &Document, plan: &Plan, prio: u32, level: u8) -> Vec<(CelId, Arc<Cel>)> {
    if !has_markers(plan) {
        return Vec::new();
    }
    let resolved = resolve(doc, plan, Mode::Ahead { level, prio });
    resolved
        .items
        .iter()
        .filter(|i| i.cel & (1 << 63) != 0 && i.fx.is_empty() && !doc.cels.contains_key(&i.cel))
        .filter_map(|i| crate::fxcache::get_any(i.cel).map(|(_, c)| (i.cel, c)))
        .collect()
}

/// Le dessin dérivé gardé sous le numéro `id` (un élément résolu d'un calque d'effet), s'il y est.
pub fn derived(id: CelId) -> Option<Arc<Cel>> {
    if id & (1 << 63) == 0 {
        return None;
    }
    crate::fxcache::get_any(id).map(|(_, c)| c)
}
