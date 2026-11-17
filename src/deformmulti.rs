//! L'outil Déformer sur plusieurs calques (Anthony, 2026-11-10 : « Les outils sélection, gomme, déformer et transformer
//! s'appliquent à plusieurs calques quand plusieurs calques sont sélectionnés. »).
//!
//! Avec plusieurs calques choisis, un clic sur le dessin ouvre **une seule** enveloppe (ou un seul cadre de perspective) autour
//! de ce que contiennent tous ces calques à l'image courante, sur la feuille (le repère commun). Tirer un coin, une poignée ronde
//! ou l'intérieur déforme tous les calques ensemble, comme un seul objet : chaque dessin est déformé là où on le voit, la
//! déformation de la feuille étant ramenée dans son repère (`D = T⁻¹ · E · T`, `T` : sa transformation, ses clés de position et
//! le défilement multiplan à cette image).
//!
//! - **Chacun selon son genre** : un dessin vectoriel déforme ses traits (au lâcher ; pendant le geste, sa feuille flottante est
//!   posée par un maillage que dessine la carte graphique, une place par calque) ; un dessin bitmap ses pixels (pendant le geste,
//!   l'aperçu rapide au plus proche voisin ; au lâcher, le résultat exact). Rien n'est rejoué pendant le geste ; le rendu exact
//!   des traits arrive d'un fil de travail (`rejeu.rs`).
//! - **Tout depuis l'original** : chaque geste de la séance repart des dessins d'origine (pas de perte d'un geste à l'autre).
//! - **Une étape d'annulation par geste**, pour tous les calques.
//! - **Calques laissés de côté** : cachés, verrouillés, vidéo, sans dessin ou vides à cette image ; la barre d'état les nomme.
//! - La séance se termine par Entrée, Échap, un clic hors du cadre, un autre outil, un autre mode, une autre image, un autre plan
//!   ou un autre choix de calques. Ni dans une case des Vignettes.
//! - Ce qui est choisi avec l'outil Sélection n'est pas repris : tout le dessin de chaque calque se déforme (pour une partie
//!   seulement, choisir un seul calque).

use eframe::egui::{self, vec2, Color32, Key, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::envelope::{self, Envelope};
use crate::envelopetools::DeformMode;
use crate::flottante::{Flottante, Pending};
use crate::history::Edit;
use crate::persp;
use crate::raster::Raster;
use crate::vector::{Affine, VectorDrawing};

/// Rayon de prise des coins et des poignées, en pixels d'écran.
const GRAB: f32 = 9.0;
const ACCENT: Color32 = crate::theme::POIGNEE_DEFORMER;

/// Ce que l'on tire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prise {
    Coin(usize),
    Poignee(usize),
    Tout,
}

/// La déformation, sur la feuille : l'enveloppe (mode Enveloppe) ou le cadre (mode Perspective).
#[derive(Clone, Copy, PartialEq)]
struct Forme {
    env: Envelope,
    quad: [Pos2; 4],
}

enum Sorte {
    /// Un dessin bitmap : l'original de la séance, et les deux feuilles de l'aperçu (ce qui reste, ce qui se déforme).
    Pixels { orig: Raster, base: Raster, sheet: Raster, shown: Rect, committed: envelope::Committed },
    /// Un dessin vectoriel : l'original de la séance et la place de sa feuille flottante.
    Traits { dessin: Box<VectorDrawing>, slot: usize },
}

/// Un calque de la séance.
struct Part {
    /// Son calque (id) et l'image où son dessin se prend (celle de sa clé).
    layer: u64,
    frame: usize,
    /// Le sceau du dessin après le dernier geste de la séance (`sceau`) : s'il change hors de la séance, celle-ci se termine sans
    /// rien écrire (son original serait périmé).
    sceau: u64,
    cel: CelId,
    /// Du dessin vers la feuille, et retour.
    to_stage: Affine,
    from_stage: Affine,
    /// Ce qui se déforme, dans le repère du dessin.
    src: Rect,
    /// Le dessin qu'on voit à l'image courante (sinon celui d'une autre clé choisie : aucun aperçu, il est déformé au lâcher).
    live: bool,
    sorte: Sorte,
}

/// Une séance de l'outil Déformer sur plusieurs calques.
pub(crate) struct DeformMulti {
    panel: usize,
    frame: usize,
    mode: DeformMode,
    /// Les calques choisis au début (ids) : un autre choix termine la séance.
    choisis: Vec<u64>,
    /// Les clés choisies du plan au début : un autre choix termine aussi la séance.
    cles: Vec<(u64, usize)>,
    /// La boîte commune, sur la feuille.
    src: Rect,
    forme: Forme,
    drag: Option<(Prise, Pos2, Forme)>,
    parts: Vec<Part>,
}

impl DeformMulti {
    /// La séance touche-t-elle l'un des calques `ids` ?
    pub(crate) fn touche(&self, ids: &[u64]) -> bool {
        self.parts.iter().any(|p| ids.contains(&p.layer))
    }

    /// Un geste est en cours sur le dessin `cel` (son image est l'aperçu).
    pub(crate) fn busy_on(&self, cel: CelId) -> bool {
        self.drag.is_some() && self.parts.iter().any(|p| p.cel == cel)
    }

    /// Où va le point `p` de la feuille.
    fn stage_map(&self) -> Option<Box<dyn Fn(Pos2) -> Pos2>> {
        let src = self.src;
        match self.mode {
            DeformMode::Envelope => {
                let env = self.forme.env;
                Some(Box::new(move |p| env.map(src, p)))
            }
            DeformMode::Perspective => {
                let h = persp::rect_to_quad(src, self.forme.quad)?;
                Some(Box::new(move |p| persp::apply(&h, p)))
            }
        }
    }

    /// L'agrandissement moyen de la déformation (l'épaisseur des traits le suit).
    fn echelle(&self) -> f32 {
        let aire = match self.mode {
            DeformMode::Envelope => self.forme.env.area(),
            DeformMode::Perspective => {
                let q = self.forme.quad;
                0.5 * (0..4).map(|i| q[i].x * q[(i + 1) % 4].y - q[(i + 1) % 4].x * q[i].y).sum::<f32>().abs()
            }
        };
        (aire / (self.src.width() * self.src.height()).max(1e-6)).sqrt()
    }

    /// Le tour de la déformation, sur la feuille (côtés échantillonnés).
    fn contour(&self) -> Vec<Pos2> {
        match self.mode {
            DeformMode::Envelope => (0..4).flat_map(|k| (0..16).map(move |i| (k, i as f32 / 16.0))).map(|(k, t)| bezier_at(&self.forme.env, k, t)).collect(),
            DeformMode::Perspective => self.forme.quad.to_vec(),
        }
    }
}

/// Le point `t` du côté `k` de l'enveloppe.
fn bezier_at(env: &Envelope, k: usize, t: f32) -> Pos2 {
    let p = env.edge(k);
    let s = 1.0 - t;
    (p[0].to_vec2() * (s * s * s) + p[1].to_vec2() * (3.0 * s * s * t) + p[2].to_vec2() * (3.0 * s * t * t) + p[3].to_vec2() * (t * t * t)).to_pos2()
}

/// Le sceau d'un dessin : ce qui change quand on y touche hors de la séance. Bitmap : sa version (seule la séance la change
/// pendant qu'elle est ouverte, et elle la note). Vectoriel : sa géométrie (le nombre d'éléments et leurs boîtes ; sa version
/// change aussi quand un rendu exact arrive, ce qui ne compte pas).
fn sceau(doc: &crate::document::Document, cel: CelId) -> u64 {
    let Some(c) = doc.cels.get(&cel) else { return 0 };
    match &c.vector {
        None => c.version,
        Some(d) => {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ d.elements.len() as u64;
            for e in &d.elements {
                let b = e.bounds();
                for v in [b.min.x, b.min.y, b.max.x, b.max.y] {
                    h ^= v.to_bits() as u64;
                    h = h.wrapping_mul(0x100_0000_01b3);
                }
            }
            h
        }
    }
}

/// Deux transformations égales (au millième près).
fn memes(a: &Affine, b: &Affine) -> bool {
    [a.a - b.a, a.b - b.b, a.c - b.c, a.d - b.d, (a.tx - b.tx) / 1000.0, (a.ty - b.ty) / 1000.0].iter().all(|v| v.abs() < 1e-3)
}

/// La boîte où `map` envoie le rectangle `r` (une grille de 9 × 9 points, un peu élargie).
fn mapped_box(r: Rect, map: &impl Fn(Pos2) -> Pos2) -> Rect {
    let mut out = Rect::NOTHING;
    for j in 0..=8 {
        for i in 0..=8 {
            let p = egui::pos2(r.min.x + r.width() * i as f32 / 8.0, r.min.y + r.height() * j as f32 / 8.0);
            out = out.union(Rect::from_min_max(map(p), map(p)));
        }
    }
    out.expand(4.0)
}

impl App {
    /// L'outil Déformer agit-il sur plusieurs calques ? Deux calques choisis au moins, hors des Vignettes ; ou une séance est
    /// ouverte.
    pub(crate) fn deform_multi_voulu(&self) -> bool {
        self.tool == Tool::Perspective && !self.in_card && (self.plusieurs_dessins_choisis() || self.deform_multi.is_some())
    }

    /// La partie de la déformation sous le pointeur (position écran).
    pub(crate) fn deform_multi_prise(&self, screen: Pos2) -> Option<Prise> {
        let d = self.deform_multi.as_ref()?;
        let to_s = |p: Pos2| self.stage_to_screen(p);
        if d.mode == DeformMode::Envelope {
            if let Some(i) = (0..8).find(|&i| to_s(d.forme.env.handles[i]).distance(screen) <= GRAB) {
                return Some(Prise::Poignee(i));
            }
            if let Some(k) = (0..4).find(|&k| to_s(d.forme.env.corners[k]).distance(screen) <= GRAB) {
                return Some(Prise::Coin(k));
            }
        } else if let Some(k) = (0..4).find(|&k| to_s(d.forme.quad[k]).distance(screen) <= GRAB) {
            return Some(Prise::Coin(k));
        }
        let contour: Vec<Pos2> = d.contour().into_iter().map(to_s).collect();
        crate::vector::point_in_polygon(screen, &contour).then_some(Prise::Tout)
    }

    /// Appui avec l'outil Déformer et plusieurs calques choisis (`stage` : point de la feuille) : sur la déformation ouverte, on
    /// la tire ; ailleurs elle se termine, et une nouvelle s'ouvre autour du contenu de tous les calques choisis.
    pub(crate) fn deform_multi_press(&mut self, stage: Pos2, screen: Pos2) {
        self.playing = false;
        if let Some(prise) = self.deform_multi_prise(screen) {
            if let Some(d) = &mut self.deform_multi {
                d.drag = Some((prise, stage, d.forme));
            }
            return;
        }
        self.deform_multi = None;
        self.envelope = None;
        self.persp = None;
        self.end_stroke();
        if self.content_refuses() {
            return;
        }
        // Les sélections et les feuilles en attente : posées d'abord (les places des feuilles servent ici).
        self.drop_multi();
        self.multi_flush();
        self.flush_flottante();
        self.vsel = None;
        self.drop_bitmap_selection();
        let ranks = self.chosen_layers();
        let (mut targets, par_cles) = self.dessins_choisis();
        let mut parts = Vec::new();
        let mut src = Rect::NOTHING;
        let mut slot = 0;
        for t in &targets.list {
            if parts.iter().any(|p: &Part| p.cel == t.cel) {
                continue;
            }
            let Some(from_stage) = t.to_stage.inverse() else { continue };
            let Some(c) = self.doc.cels.get(&t.cel) else { continue };
            let (local, sorte) = match (&c.vector, t.vector) {
                (Some(d), true) => {
                    let b = d.elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
                    slot += 1;
                    (b, Sorte::Traits { dessin: Box::new(d.clone()), slot })
                }
                _ => {
                    let Some(b) = persp::raster_bounds(&c.raster) else {
                        targets.skipped.push((t.name.clone(), "vide à cette image"));
                        continue;
                    };
                    // (L'aperçu n'est fait que pour le dessin qu'on voit.)
                    let (base, sheet) = if t.live { envelope::split_rect(&c.raster, b) } else { (Raster::new(1, 1), Raster::new(1, 1)) };
                    (b, Sorte::Pixels { orig: c.raster.clone(), base, sheet, shown: b, committed: Default::default() })
                }
            };
            if !local.is_positive() || !local.is_finite() {
                targets.skipped.push((t.name.clone(), "vide à cette image"));
                if matches!(sorte, Sorte::Traits { .. }) {
                    slot -= 1;
                }
                continue;
            }
            src = src.union(t.to_stage.apply_rect(local));
            parts.push(Part { layer: t.id, frame: t.frame, sceau: sceau(&self.doc, t.cel), cel: t.cel, to_stage: t.to_stage, from_stage, src: local, live: t.live, sorte });
        }
        let laisses = targets.skipped_text();
        if parts.is_empty() || !src.is_positive() {
            let rien = "Déformer : rien à déformer sur les calques choisis à cette image.";
            self.status = match laisses {
                Some(l) => format!("{rien} {l}"),
                None => rien.into(),
            };
            return;
        }
        let choisis = ranks.iter().filter_map(|&li| self.layers().get(li).map(|l| l.id)).collect();
        let forme = Forme { env: Envelope::identity(src), quad: [src.left_top(), src.right_top(), src.right_bottom(), src.left_bottom()] };
        let n = parts.len();
        let cles = self.cles_du_plan();
        self.deform_multi = Some(DeformMulti { panel: self.cur_panel, frame: self.cur_frame, mode: self.deform_mode, choisis, cles, src, forme, drag: None, parts });
        let quoi = match self.deform_mode {
            DeformMode::Envelope => "Enveloppe",
            DeformMode::Perspective => "Perspective",
        };
        let sur = if par_cles { Self::dessins_texte(&targets, true) } else { format!("{n} calques") };
        let texte = format!("{quoi} sur {sur} : tire un coin (ou l'intérieur pour tout déplacer) ; tout se déforme ensemble. Entrée, Échap ou clic ailleurs : terminer ; Ctrl+Z annule chaque geste.");
        self.status = match laisses {
            Some(l) => format!("{texte} {l}"),
            None => texte,
        };
    }

    /// Le pointeur bouge (`stage` : point de la feuille) ; vrai si la déformation est tirée.
    pub(crate) fn deform_multi_drag(&mut self, stage: Pos2) -> bool {
        let Some(d) = &mut self.deform_multi else { return false };
        let Some((prise, start, f0)) = d.drag else { return false };
        let delta = stage - start;
        let mut f = f0;
        match (d.mode, prise) {
            (DeformMode::Envelope, Prise::Coin(k)) => f.env.move_corner(k, delta),
            (DeformMode::Envelope, Prise::Poignee(i)) => f.env.handles[i] = f0.env.handles[i] + delta,
            (DeformMode::Envelope, Prise::Tout) => f.env.translate(delta),
            (DeformMode::Perspective, Prise::Coin(k)) => f.quad[k] = f0.quad[k] + delta,
            (DeformMode::Perspective, _) => f.quad = f0.quad.map(|c| c + delta),
        }
        // Un cadre qui se croise ne se déforme pas : on garde le dernier correct.
        if (d.mode == DeformMode::Perspective && !persp::is_convex(&f.quad)) || f == d.forme {
            return true;
        }
        d.forme = f;
        self.deform_multi_apercu();
        true
    }

    /// L'aperçu de la déformation du moment, sur chaque calque, sans rien rejouer.
    fn deform_multi_apercu(&mut self) {
        let Some(mut d) = self.deform_multi.take() else { return };
        let Some(m) = d.stage_map() else {
            self.deform_multi = Some(d);
            return;
        };
        let cells = if d.mode == DeformMode::Envelope { 8 } else { 24 };
        for part in d.parts.iter_mut().filter(|p| p.live) {
            let (to, from, cel, src) = (part.to_stage, part.from_stage, part.cel, part.src);
            let map = |p: Pos2| from.apply(m(to.apply(p)));
            match &mut part.sorte {
                Sorte::Traits { dessin, slot } => {
                    let slot = *slot;
                    if !self.multi.floats.iter().any(|f| f.slot == slot && f.src == cel) {
                        // (Une feuille d'un autre geste sur cette place s'en va d'abord.)
                        if let Some(i) = self.multi.floats.iter().position(|f| f.slot == slot) {
                            let old = self.multi.floats.remove(i);
                            Flottante::release_slot(&mut self.doc, old.slot);
                        }
                        let all: Vec<usize> = (0..dessin.elements.len()).collect();
                        if let Some(f) = Flottante::lift_from_slot(&mut self.doc, cel, dessin, &all, slot) {
                            self.multi.floats.push(f);
                        }
                    }
                    if let Some(f) = self.multi.floats.iter_mut().find(|f| f.slot == slot && f.src == cel) {
                        f.pending = None;
                        f.set_warp(src, cells, map);
                    }
                }
                Sorte::Pixels { base, sheet, shown, .. } => {
                    let mut img = base.clone();
                    img.composite_over(&envelope::warp_raster_fast(sheet, src, map));
                    let vu = mapped_box(src, &map);
                    let region = shown.union(vu).union(src).expand(2.0);
                    *shown = vu.union(src);
                    let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
                    let tiles = crate::vector::tiles_in(&c.raster, region);
                    c.raster = img;
                    c.version += 1;
                    self.renderer.invalidate_tiles(&tiles);
                }
            }
        }
        self.deform_multi = Some(d);
        self.modified = true;
    }

    /// Lâcher : chaque calque reçoit la déformation depuis son dessin d'origine, en une seule étape d'annulation. Les traits
    /// gardent leur feuille déformée jusqu'à l'arrivée de leur rendu exact (en arrière-plan).
    pub(crate) fn deform_multi_release(&mut self) {
        let Some(mut d) = self.deform_multi.take() else { return };
        let Some((_, _, f0)) = d.drag.take() else {
            self.deform_multi = Some(d);
            return;
        };
        // Revenu à son point de départ (ou « Remettre droit » sur un cadre déjà droit) : rien à poser, mais l'aperçu du geste
        // s'en va (la feuille des traits attend son rendu, les pixels reprennent l'image d'avant le geste).
        let Some(m) = d.stage_map().filter(|_| f0 != d.forme) else {
            self.deform_multi_restaurer(&mut d);
            self.deform_multi = Some(d);
            return;
        };
        let k = d.echelle();
        let mut edits: Vec<Edit> = Vec::new();
        for part in &mut d.parts {
            let (to, from, cel, src) = (part.to_stage, part.from_stage, part.cel, part.src);
            let map = |p: Pos2| from.apply(m(to.apply(p)));
            match &mut part.sorte {
                Sorte::Pixels { orig, committed, .. } => {
                    let new = envelope::warp_raster_with(orig, src, map);
                    let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
                    // (« Avant » : l'image d'avant CE geste, comme pour un seul calque, `Committed`.)
                    let Some((before, changed)) = committed.step(orig, &c.raster, &new) else { continue };
                    c.raster = new;
                    c.version += 1;
                    self.renderer.invalidate_tiles(&changed);
                    edits.push(Edit::Tiles { cel, tiles: before });
                }
                Sorte::Traits { dessin, slot } => {
                    let new = envelope::warp_drawing_with(dessin, None, src, map, k);
                    let Some(dd) = self.drawing_mut(cel) else { continue };
                    let before = std::mem::replace(dd, new);
                    self.rejeu_cel(cel);
                    edits.push(Edit::Vector { cel, drawing: before });
                    // La feuille déformée reste affichée jusqu'à l'arrivée du rendu exact.
                    if let Some(f) = self.multi.floats.iter_mut().find(|f| f.slot == *slot && f.src == cel) {
                        f.pending = Some(Pending { items: Vec::new(), originals: Vec::new() });
                    }
                }
            }
        }
        let nom = match d.mode {
            DeformMode::Envelope => "Enveloppe",
            DeformMode::Perspective => "Perspective",
        };
        let n = d.parts.len();
        for part in &mut d.parts {
            part.sceau = sceau(&self.doc, part.cel);
        }
        self.deform_multi = Some(d);
        if !edits.is_empty() {
            let avant = self.history.context.replace(format!("{nom} ({n} calques)"));
            self.history.push(Edit::Group(edits));
            self.history.context = avant;
            self.modified = true;
        }
    }

    /// Un geste qui ne pose rien : l'aperçu s'en va. La feuille de chaque dessin vectoriel attend le rendu exact de son dessin
    /// (qui est redemandé) ; chaque dessin bitmap qu'on voit reprend l'image d'avant le geste (le résultat du geste d'avant, ou
    /// l'original).
    fn deform_multi_restaurer(&mut self, d: &mut DeformMulti) {
        for part in d.parts.iter_mut().filter(|p| p.live) {
            let cel = part.cel;
            match &mut part.sorte {
                Sorte::Traits { slot, .. } => {
                    if let Some(f) = self.multi.floats.iter_mut().find(|f| f.slot == *slot && f.src == cel && f.pending.is_none()) {
                        f.pending = Some(Pending { items: Vec::new(), originals: Vec::new() });
                        self.rejeu_cel(cel);
                    }
                }
                Sorte::Pixels { orig, committed, shown, .. } => {
                    let img = committed.current(orig).clone();
                    let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
                    let tiles = crate::vector::tiles_in(&c.raster, shown.union(part.src).expand(2.0));
                    c.raster = img;
                    c.version += 1;
                    *shown = part.src;
                    self.renderer.invalidate_tiles(&tiles);
                }
            }
            part.sceau = sceau(&self.doc, cel);
        }
    }

    /// « Remettre droit » : la déformation revient à la boîte (une étape d'annulation).
    pub(crate) fn deform_multi_reset(&mut self) {
        let Some(d) = &mut self.deform_multi else { return };
        let s = d.src;
        d.drag = Some((Prise::Tout, Pos2::ZERO, d.forme));
        d.forme = Forme { env: Envelope::identity(s), quad: [s.left_top(), s.right_top(), s.right_bottom(), s.left_bottom()] };
        self.deform_multi_apercu();
        self.deform_multi_release();
    }

    /// Termine la séance (un geste en cours est d'abord lâché).
    pub(crate) fn deform_multi_fermer(&mut self) {
        if self.deform_multi.as_ref().is_some_and(|d| d.drag.is_some()) {
            self.deform_multi_release();
        }
        self.deform_multi = None;
    }

    /// Termine la séance sans rien écrire : un geste en cours rend à chaque dessin son image d'avant le geste (partie 136 : un
    /// calque de la séance vient d'être verrouillé).
    pub(crate) fn deform_multi_abandonner(&mut self) {
        if let Some(mut d) = self.deform_multi.take() {
            if d.drag.take().is_some() {
                self.deform_multi_restaurer(&mut d);
            }
        }
        self.status = "Déformation des calques choisis terminée : un de leurs calques est verrouillé.".into();
    }

    /// Entrée ou Échap terminent la séance ; vrai si la touche a servi.
    pub(crate) fn deform_multi_key(&mut self, key: Key) -> bool {
        if self.deform_multi.is_none() || !matches!(key, Key::Enter | Key::Escape) {
            return false;
        }
        self.deform_multi_fermer();
        self.status = "Déformation des calques choisis terminée.".into();
        true
    }

    /// Une fois par image : un autre outil, un autre mode, une autre image, un autre plan ou un autre choix de calques
    /// termine la séance.
    pub(crate) fn check_deform_multi(&mut self) {
        let Some(d) = &self.deform_multi else { return };
        let choisis: Vec<u64> = self.chosen_layers().iter().filter_map(|&li| self.layers().get(li).map(|l| l.id)).collect();
        let ok = self.tool == Tool::Perspective
            && !self.in_card
            && self.deform_mode == d.mode
            && self.cur_panel == d.panel
            && self.cur_frame == d.frame
            && choisis == d.choisis
            && self.cles_du_plan() == d.cles;
        if !ok {
            self.deform_multi_fermer();
            return;
        }
        // Un calque de la séance verrouillé (cadenas, dossier verrouillé) : elle se ferme sans rien écrire.
        if let Some(panel) = self.doc.panels.get(d.panel) {
            let verrou = d.parts.iter().any(|p| panel.layers.iter().position(|l| l.id == p.layer).is_some_and(|li| panel.layers[li].locked || crate::folders::locked_by_folder(&panel.layers, li)));
            if verrou {
                self.deform_multi_abandonner();
                return;
            }
        }
        // Un dessin de la séance a changé hors d'elle (une clé posée ou effacée, une position, un trait) : son original est
        // périmé. La séance se termine sans rien écrire, pour ne rien perdre de ce changement. (Pendant un geste, l'aperçu change
        // les dessins : on regarde au lâcher.)
        if d.drag.is_some() {
            return;
        }
        let Some(panel) = self.doc.panels.get(d.panel) else { return };
        let change = d.parts.iter().any(|p| {
            let Some(li) = panel.layers.iter().position(|l| l.id == p.layer) else { return true };
            panel.layers[li].cel_at(p.frame) != Some(p.cel)
                || crate::multisel::layer_to_stage(&self.doc, d.panel, p.frame, li).is_none_or(|t| !memes(&t, &p.to_stage))
                || sceau(&self.doc, p.cel) != p.sceau
        });
        if change {
            self.deform_multi = None;
            self.status = "Déformation des calques choisis terminée : un de leurs dessins a changé depuis l'ouverture du cadre.".into();
        }
    }

    /// Le cadre (ou l'enveloppe, sa grille et ses poignées), sur la feuille.
    pub(crate) fn paint_deform_multi_overlay(&self, painter: &egui::Painter) {
        let Some(d) = &self.deform_multi else { return };
        let to_s = |p: Pos2| self.stage_to_screen(p);
        let grid = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.45));
        if let Some(m) = d.stage_map() {
            for k in 1..4 {
                let t = k as f32 / 4.0;
                let (x, y) = (d.src.min.x + d.src.width() * t, d.src.min.y + d.src.height() * t);
                let v: Vec<Pos2> = (0..=24).map(|j| to_s(m(egui::pos2(x, d.src.min.y + d.src.height() * j as f32 / 24.0)))).collect();
                let w: Vec<Pos2> = (0..=24).map(|j| to_s(m(egui::pos2(d.src.min.x + d.src.width() * j as f32 / 24.0, y)))).collect();
                painter.add(Shape::line(v, grid));
                painter.add(Shape::line(w, grid));
            }
        }
        let contour: Vec<Pos2> = d.contour().into_iter().map(to_s).collect();
        painter.add(Shape::closed_line(contour.clone(), Stroke::new(2.5_f32, Color32::from_black_alpha(120))));
        painter.add(Shape::closed_line(contour, Stroke::new(1.0_f32, ACCENT)));
        let coins = match d.mode {
            DeformMode::Envelope => {
                let e = &d.forme.env;
                for k in 0..4 {
                    let (a, b) = (to_s(e.corners[k]), to_s(e.corners[(k + 1) % 4]));
                    let (h0, h1) = (to_s(e.handles[2 * k]), to_s(e.handles[2 * k + 1]));
                    painter.line_segment([a, h0], Stroke::new(1.0_f32, ACCENT));
                    painter.line_segment([b, h1], Stroke::new(1.0_f32, ACCENT));
                    for h in [h0, h1] {
                        painter.circle(h, 4.5, Color32::WHITE, Stroke::new(1.5_f32, ACCENT));
                    }
                }
                e.corners
            }
            DeformMode::Perspective => d.forme.quad,
        };
        for c in coins.map(to_s) {
            painter.rect(Rect::from_center_size(c, vec2(9.0, 9.0)), 1.0, Color32::WHITE, Stroke::new(1.5_f32, ACCENT), StrokeKind::Middle);
        }
    }

    /// Curseur au-dessus de la déformation ouverte (aucun : pas de séance).
    pub(crate) fn deform_multi_cursor(&self, screen: Pos2) -> Option<egui::CursorIcon> {
        let d = self.deform_multi.as_ref()?;
        Some(match (d.drag.is_some(), self.deform_multi_prise(screen)) {
            (true, _) => egui::CursorIcon::Grabbing,
            (false, Some(Prise::Coin(_) | Prise::Poignee(_))) => egui::CursorIcon::Grab,
            (false, Some(Prise::Tout)) => egui::CursorIcon::Move,
            (false, None) => egui::CursorIcon::Crosshair,
        })
    }

    /// Réglages de l'outil Déformer avec plusieurs calques choisis : ce qui se déforme, « Remettre droit » et « Terminer ».
    pub(crate) fn deform_multi_ui(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let n = self.chosen_layers().len();
        let editing = self.deform_multi.is_some();
        ui.label(egui::RichText::new(format!("{n} calques choisis : la déformation les prend tous, comme un seul objet.")).strong());
        ui.horizontal(|ui| {
            if crate::bulles::icone_si(ui, editing, ph::ARROW_COUNTER_CLOCKWISE, "Remettre droit", "Les coins et les poignées reviennent à leur place.", "").clicked() {
                self.deform_multi_reset();
            }
            if crate::bulles::icone_si(ui, editing, ph::CHECK, "Terminer", "Garder la déformation et quitter (Entrée).", "").clicked() {
                self.deform_multi_fermer();
            }
        });
        // (Le mode d'emploi est dans l'animation « L'enveloppe » et le registre des explications, partie 153.)
        self.avertir_laisses_de_cote(ui);
    }
}
