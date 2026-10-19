//! Crayonnés rapides (Anthony, 2026-10-19 : « un mode thumbnails » — « crayonnés rapides » : une
//! page de petites cases où l'on enchaîne des croquis très vite, puis chaque case devient un plan
//! du storyboard). Ce n'est PAS une vue en grille des plans existants : c'est une surface de dessin.
//!
//! - **Modèle** : une `Planche` (la planche de crayonnés du projet) est une suite de `Case`s, toutes
//!   au rapport du cadre du projet. Chaque case a son propre dessin : l'encre (un dessin vectoriel,
//!   des traits lisses : le crayon et la gomme) et, s'il y en a, de la peinture (un dessin bitmap : le
//!   pinceau). Elles sont rangées en pages de `colonnes × lignes` cases (4 × 3 au départ).
//! - **Un trait appartient à la case où il commence** : ses points sont notés dans le repère de cette
//!   case (ils peuvent en sortir) ; la case est un dessin à ses bornes, donc le trait est coupé à
//!   l'affichage ; au passage en plan, ce qui sort de la case est coupé de même (`clip_stroke`).
//! - **Sans latence** : même machinerie que le dessin (`StrokeState`) — l'aperçu du geste est peint
//!   dans l'image de la case à chaque point reçu ; au lâcher, le trait lisse prend la place de
//!   l'aperçu (`Cel::render_on_top`, un seul trait : jamais de rejeu du moteur sur le fil de
//!   l'interface). Les traits d'une case n'ont pas de pinceau texturé : leur rendu est exact et
//!   immédiat, sur ce fil comme à la relecture.
//! - **Fichier** : `crayonnes.json` à côté du manifeste (`project.rs`), sans changer `VERSION` :
//!   absent, il n'y a pas de planche ; une version qui ne le connaît pas le laisse en place. Écrit
//!   par `SaveJob` (sur l'autre fil), atomiquement, avec les protections de `docs/FIABILITE.md`
//!   (`write_sidecar` : jamais effacé s'il est illisible ou plus récent, mis de côté sous
//!   `.illisible` si un fichier neuf doit le remplacer).
//! - **Annulation** : la planche a sa propre pile (`PageHistory`) ; « Créer les plans » est une seule
//!   étape de l'historique du projet, et la pile de la planche s'y renvoie (`PEdit::Plans`).
//! - L'écran est dans `crayonnesui.rs`.

use std::ops::Range;

use eframe::egui::{pos2, vec2, Pos2, Rect};
use serde::{Deserialize, Serialize};

use crate::brush::{BrushSettings, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushTextures;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::history::Edit;
use crate::raster::{Raster, Tile};
use crate::vector::{self, Affine, Element, VPoint, VectorDrawing};

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "crayonnes.json";
const FORMAT: &str = "shot-crayonnes";
const VERSION: u32 = 1;

/// Le grand côté d'une case, en pixels (l'autre suit le rapport du cadre du projet).
pub(crate) const BOX_LONG: usize = 640;
/// Les grilles proposées (colonnes, lignes) ; 4 × 3 au départ.
pub(crate) const GRIDS: [(usize, usize); 5] = [(2, 2), (3, 2), (4, 3), (4, 4), (6, 4)];
pub(crate) const DEFAULT_GRID: (usize, usize) = (4, 3);
/// Bornes d'un fichier abîmé : plus de cases, de traits, de texte ou de pixels que ça est coupé.
const MAX_CASES: usize = 2400;
const MAX_ELEMENTS: usize = 20_000;
pub(crate) const CAPTION_MAX: usize = 400;
const MAX_COORD: f32 = 100_000.0;
const MAX_RADIUS: f32 = 400.0;
const MAX_SIDE: usize = 4096;
/// Étapes d'annulation gardées.
const HISTORY_LIMIT: usize = 200;

// ------------------------------------------------------------------------------------ la planche

/// Ce qui change dans une case et doit être envoyé à l'écran (textures de l'interface).
#[derive(Clone, Debug, Default)]
pub(crate) struct Dirty {
    /// Tout est à renvoyer (case relue, annulation, vidée).
    pub all: bool,
    /// Tuiles de l'encre et de la peinture touchées depuis le dernier envoi.
    pub ink: Vec<usize>,
    pub paint: Vec<usize>,
}

impl Dirty {
    /// Ce qui est à envoyer, et la liste remise à zéro (tuiles triées, sans doublon).
    pub fn take(&mut self) -> Dirty {
        let mut d = std::mem::take(self);
        d.ink.sort_unstable();
        d.ink.dedup();
        d.paint.sort_unstable();
        d.paint.dedup();
        d
    }
}

/// Une case de la planche.
#[derive(Clone)]
pub(crate) struct Case {
    /// Identifiant, unique dans la planche (stable quand les cases changent de place).
    pub id: u64,
    /// La ligne sous la case : elle devient le texte d'action du plan.
    pub caption: String,
    /// Des plans ont été faits d'après cette case (la petite coche).
    pub used: bool,
    /// L'encre : un dessin vectoriel (traits lisses) aux dimensions de la case.
    pub ink: Cel,
    /// La peinture : un dessin bitmap (le pinceau) aux dimensions de la case.
    pub paint: Cel,
    pub dirty: Dirty,
    /// L'image de l'encre est à refaire d'après ses traits (case relue d'un fichier).
    stale: bool,
}

impl Case {
    fn blank(id: u64, size: [usize; 2]) -> Case {
        Case {
            id,
            caption: String::new(),
            used: false,
            ink: Cel::new(size[0], size[1], LayerKind::Vector),
            paint: Cel::bitmap(Raster::new(size[0], size[1])),
            dirty: Dirty::default(),
            stale: false,
        }
    }

    pub fn has_ink(&self) -> bool {
        self.ink.vector.as_ref().is_some_and(|v| !v.elements.is_empty())
    }

    /// De la peinture visible (une gomme laisse des tuiles vides : on regarde les pixels).
    pub fn has_paint(&self) -> bool {
        self.paint.raster.tiles.iter().flatten().any(|t| t.iter().any(|c| c.a() != 0))
    }

    pub fn has_drawing(&self) -> bool {
        self.has_ink() || self.has_paint()
    }

    /// Rien dedans, pas de légende, jamais servi.
    pub fn is_blank(&self) -> bool {
        !self.has_drawing() && self.caption.trim().is_empty() && !self.used
    }

    /// L'image de l'encre, refaite d'après ses traits si elle ne l'est pas encore.
    pub fn render_if_stale(&mut self) {
        if self.stale {
            self.ink.rerender();
            self.dirty.all = true;
            self.stale = false;
        }
    }

    fn clear_drawing(&mut self, size: [usize; 2]) {
        self.ink = Cel::new(size[0], size[1], LayerKind::Vector);
        self.paint = Cel::bitmap(Raster::new(size[0], size[1]));
        self.dirty = Dirty { all: true, ..Default::default() };
        self.stale = false;
    }
}

/// La planche de crayonnés du projet.
#[derive(Clone)]
pub(crate) struct Planche {
    /// Taille d'une case en pixels (au rapport du cadre du projet au premier usage).
    pub size: [usize; 2],
    /// Colonnes et lignes d'une page.
    pub grid: (usize, usize),
    pub cases: Vec<Case>,
    next_id: u64,
}

impl Default for Planche {
    fn default() -> Self {
        Planche { size: [0, 0], grid: DEFAULT_GRID, cases: Vec::new(), next_id: 1 }
    }
}

/// La taille d'une case pour un cadre `frame` : le grand côté fait `BOX_LONG`.
pub(crate) fn box_size(frame: [usize; 2]) -> [usize; 2] {
    let (w, h) = (frame[0].max(1) as f32, frame[1].max(1) as f32);
    if w >= h {
        [BOX_LONG, ((BOX_LONG as f32 * h / w).round() as usize).max(1)]
    } else {
        [((BOX_LONG as f32 * w / h).round() as usize).max(1), BOX_LONG]
    }
}

impl Planche {
    pub fn len(&self) -> usize {
        self.cases.len()
    }

    pub fn per_page(&self) -> usize {
        self.grid.0 * self.grid.1
    }

    pub fn page_count(&self) -> usize {
        self.cases.len().div_ceil(self.per_page().max(1)).max(1)
    }

    pub fn page_of(&self, index: usize) -> usize {
        index / self.per_page().max(1)
    }

    /// Les indices des cases de la page `page`.
    pub fn page_range(&self, page: usize) -> Range<usize> {
        let n = self.per_page().max(1);
        (page * n).min(self.cases.len())..((page + 1) * n).min(self.cases.len())
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.cases.iter().position(|c| c.id == id)
    }

    pub fn case_mut(&mut self, id: u64) -> Option<&mut Case> {
        self.cases.iter_mut().find(|c| c.id == id)
    }

    /// Rien à enregistrer : aucune case n'a de dessin, de légende ni de coche.
    pub fn is_blank(&self) -> bool {
        self.cases.iter().all(Case::is_blank)
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn blank_case(&mut self) -> Case {
        let id = self.fresh_id();
        Case::blank(id, self.size)
    }

    /// Prépare la planche à servir : une planche neuve reçoit sa première page, aux proportions du
    /// cadre `frame`. Une planche déjà faite ne change pas (ses cases gardent leur taille).
    pub fn ensure(&mut self, frame: [usize; 2]) {
        if self.cases.is_empty() || self.size[0] == 0 || self.size[1] == 0 {
            self.cases.clear();
            self.size = box_size(frame);
            for _ in 0..self.per_page() {
                let c = self.blank_case();
                self.cases.push(c);
            }
        }
    }

    /// Une page de cases vides à la fin ; renvoie l'indice de sa première case.
    pub fn add_page(&mut self) -> usize {
        // La dernière page est d'abord complétée : une page, c'est toujours `colonnes × lignes`.
        let first = self.cases.len().div_ceil(self.per_page()) * self.per_page();
        while self.cases.len() < first + self.per_page() {
            let c = self.blank_case();
            self.cases.push(c);
        }
        first
    }

    /// Une case vide juste après la case `index` (à la fin si `index` dépasse) ; renvoie son indice.
    pub fn insert_after(&mut self, index: usize) -> usize {
        let at = (index + 1).min(self.cases.len());
        let c = self.blank_case();
        self.cases.insert(at, c);
        at
    }

    /// Une case vide juste avant la case `index` ; renvoie son indice.
    pub fn insert_before(&mut self, index: usize) -> usize {
        let at = index.min(self.cases.len());
        let c = self.blank_case();
        self.cases.insert(at, c);
        at
    }

    /// Retire la case `index`. La dernière case qui reste est seulement vidée. Faux si `index`
    /// n'existe pas.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.cases.len() {
            return false;
        }
        if self.cases.len() == 1 {
            self.clear_case(0);
            self.cases[0].caption.clear();
            self.cases[0].used = false;
            return true;
        }
        self.cases.remove(index);
        true
    }

    /// La case `from` passe à la place `to` (les autres glissent d'un cran). Faux si rien ne change.
    pub fn move_case(&mut self, from: usize, to: usize) -> bool {
        if from >= self.cases.len() || to >= self.cases.len() || from == to {
            return false;
        }
        let c = self.cases.remove(from);
        self.cases.insert(to, c);
        true
    }

    /// Vide le dessin de la case (sa légende et sa coche restent).
    pub fn clear_case(&mut self, index: usize) -> bool {
        let size = self.size;
        match self.cases.get_mut(index) {
            Some(c) => {
                c.clear_drawing(size);
                true
            }
            None => false,
        }
    }

    /// Change la grille (colonnes, lignes) ; les cases ne bougent pas, les pages se recomptent.
    pub fn set_grid(&mut self, cols: usize, rows: usize) -> bool {
        let g = (cols.clamp(1, 8), rows.clamp(1, 8));
        if g == self.grid {
            return false;
        }
        self.grid = g;
        true
    }

    /// Pose ou retire la coche « utilisée » des cases `indices`.
    pub fn mark_used(&mut self, indices: &[usize], used: bool) {
        for &i in indices {
            if let Some(c) = self.cases.get_mut(i) {
                c.used = used;
            }
        }
    }

    /// Tout est à renvoyer à l'écran (après une annulation qui remplace la planche).
    pub fn invalidate(&mut self) {
        for c in &mut self.cases {
            c.dirty.all = true;
        }
    }

    /// Les indices des cases désignées par leurs identifiants, dans l'ordre de la planche.
    pub fn picks(&self, ids: &[u64]) -> Vec<usize> {
        self.cases.iter().enumerate().filter(|(_, c)| ids.contains(&c.id)).map(|(i, _)| i).collect()
    }

    /// Toutes les cases qui servent à quelque chose (un dessin ou une légende).
    pub fn filled(&self) -> Vec<usize> {
        self.cases.iter().enumerate().filter(|(_, c)| c.has_drawing() || !c.caption.trim().is_empty()).map(|(i, _)| i).collect()
    }
}

// ------------------------------------------------------------------------------------ navigation

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Move {
    Next,
    Prev,
    Left,
    Right,
    Up,
    Down,
}

/// La case où mène un déplacement depuis la case `i` d'une planche de `len` cases rangées par pages
/// de `cols` colonnes. Tab et les flèches droite / gauche passent à la case voisine, de page en
/// page ; haut et bas changent de ligne (la ligne d'une page suit celle de la page d'avant). Au bout,
/// on reste où l'on est.
pub(crate) fn navigate(i: usize, len: usize, cols: usize, mv: Move) -> usize {
    if len == 0 {
        return 0;
    }
    let i = i.min(len - 1);
    let cols = cols.max(1);
    match mv {
        Move::Next | Move::Right => (i + 1).min(len - 1),
        Move::Prev | Move::Left => i.saturating_sub(1),
        Move::Down if i + cols < len => i + cols,
        Move::Down => i,
        Move::Up if i >= cols => i - cols,
        Move::Up => i,
    }
}

/// Le choix des cases après un clic sur le numéro de la case `clicked` : un clic choisit cette case
/// seule, Ctrl ajoute ou retire, Maj prend toutes celles entre la dernière cliquée (`anchor`) et
/// celle-ci. Renvoie les identifiants choisis et la nouvelle case d'ancrage.
pub(crate) fn choose(selected: &[u64], anchor: Option<usize>, ids: &[u64], clicked: usize, ctrl: bool, shift: bool) -> (Vec<u64>, Option<usize>) {
    let Some(&id) = ids.get(clicked) else { return (selected.to_vec(), anchor) };
    if shift {
        let from = anchor.unwrap_or(clicked).min(ids.len() - 1);
        let (a, b) = (from.min(clicked), from.max(clicked));
        return (ids[a..=b].to_vec(), Some(from));
    }
    if ctrl {
        let mut v = selected.to_vec();
        match v.iter().position(|&x| x == id) {
            Some(k) => {
                v.remove(k);
            }
            None => v.push(id),
        }
        return (v, Some(clicked));
    }
    (vec![id], Some(clicked))
}

// ------------------------------------------------------------------------------------ disposition

/// Une case de la page à l'écran : le cadre du dessin, la ligne de légende dessous, et le numéro
/// (petite étiquette dans le coin du cadre).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell {
    pub frame: Rect,
    pub caption: Rect,
    pub badge: Rect,
}

/// Espace entre les cases, hauteur de la ligne de légende et de l'étiquette du numéro, en points.
pub(crate) const GAP: f32 = 14.0;
pub(crate) const CAPTION_H: f32 = 24.0;
pub(crate) const BADGE_H: f32 = 20.0;

/// Les `cols × rows` cases d'une page dans `avail`, toutes au rapport `aspect` (largeur ÷ hauteur),
/// aussi grandes que possible, la grille centrée.
pub(crate) fn layout(avail: Rect, cols: usize, rows: usize, aspect: f32) -> Vec<Cell> {
    let (cols, rows) = (cols.max(1), rows.max(1));
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 16.0 / 9.0 };
    let by_width = (avail.width() - GAP * (cols as f32 + 1.0)) / cols as f32;
    let by_height = (avail.height() - GAP * (rows as f32 + 1.0) - CAPTION_H * rows as f32) / rows as f32 * aspect;
    let w = by_width.min(by_height).max(8.0);
    let h = w / aspect;
    let total = vec2(cols as f32 * w + (cols as f32 - 1.0) * GAP, rows as f32 * (h + CAPTION_H) + (rows as f32 - 1.0) * GAP);
    let origin = avail.min + ((avail.size() - total) * 0.5).max(vec2(0.0, 0.0));
    let mut cells = Vec::with_capacity(cols * rows);
    for slot in 0..cols * rows {
        let (c, r) = (slot % cols, slot / cols);
        let min = origin + vec2(c as f32 * (w + GAP), r as f32 * (h + CAPTION_H + GAP));
        let frame = Rect::from_min_size(min, vec2(w, h));
        let caption = Rect::from_min_size(frame.left_bottom() + vec2(0.0, 2.0), vec2(w, CAPTION_H - 2.0));
        let badge = Rect::from_min_size(frame.min, vec2((BADGE_H + 8.0).min(w * 0.5), BADGE_H));
        cells.push(Cell { frame, caption, badge });
    }
    cells
}

/// La case de la page sous le point `pos` (le cadre du dessin seulement).
pub(crate) fn hit(cells: &[Cell], pos: Pos2) -> Option<usize> {
    cells.iter().position(|c| c.frame.contains(pos))
}

/// Un point de l'écran dans le repère de la case dessinée dans `frame` (taille `size` en pixels).
/// Il peut sortir de la case : un trait qui en sort reste à la case où il a commencé.
pub(crate) fn to_local(frame: Rect, size: [usize; 2], pos: Pos2) -> Pos2 {
    let k = vec2(size[0] as f32 / frame.width().max(1e-3), size[1] as f32 / frame.height().max(1e-3));
    pos2((pos.x - frame.min.x) * k.x, (pos.y - frame.min.y) * k.y)
}

// ------------------------------------------------------------------------------------ coupe

/// Le trait `points` coupé au rectangle `rect` : les morceaux qui sont dedans, en gardant l'interpolation
/// des rayons. Un trait qui sort et rentre donne deux morceaux ; un trait entièrement dehors, rien.
pub(crate) fn clip_stroke(points: &[VPoint], rect: Rect) -> Vec<Vec<VPoint>> {
    let inside = |p: &VPoint| rect.contains(p.pos());
    let mut out: Vec<Vec<VPoint>> = Vec::new();
    let mut run: Vec<VPoint> = Vec::new();
    let flush = |run: &mut Vec<VPoint>, out: &mut Vec<Vec<VPoint>>| {
        if !run.is_empty() {
            out.push(std::mem::take(run));
        }
    };
    match points {
        [] => return out,
        [p] => {
            if inside(p) {
                out.push(vec![*p]);
            }
            return out;
        }
        _ => {}
    }
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        match clip_segment(a, b, rect) {
            Some((t0, t1)) => {
                let lerp = |t: f32| VPoint { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, r: a.r + (b.r - a.r) * t };
                let (p, q) = (if t0 <= 0.0 { a } else { lerp(t0) }, if t1 >= 1.0 { b } else { lerp(t1) });
                // Le segment prolonge le morceau en cours s'il en part (même point de départ).
                if run.last().is_none_or(|l| (l.x - p.x).abs() > 1e-3 || (l.y - p.y).abs() > 1e-3) {
                    flush(&mut run, &mut out);
                    run.push(p);
                }
                run.push(q);
                if t1 < 1.0 {
                    flush(&mut run, &mut out);
                }
            }
            None => flush(&mut run, &mut out),
        }
    }
    flush(&mut run, &mut out);
    out
}

/// La part [t0, t1] du segment a → b qui est dans `rect` (Liang–Barsky), si elle existe.
fn clip_segment(a: VPoint, b: VPoint, rect: Rect) -> Option<(f32, f32)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-dx, a.x - rect.min.x), (dx, rect.max.x - a.x), (-dy, a.y - rect.min.y), (dy, rect.max.y - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return None;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return None;
                }
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

// ------------------------------------------------------------------------------------ le geste

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Outil {
    /// Trait lisse de la case (vectoriel).
    Crayon,
    /// Pinceau du logiciel, peint dans l'image de la case (bitmap).
    Pinceau,
    /// Gomme : coupe les traits et efface la peinture.
    Gomme,
}

/// Ce qu'il faut savoir du style choisi dans la barre pour tracer.
#[derive(Clone)]
pub(crate) struct Style {
    pub color: [u8; 3],
    /// Diamètre du crayon, en pixels de la case ; la gomme et le pinceau en dérivent.
    pub size: f32,
    /// Le pinceau du logiciel (sa taille est remplacée par `size`) et ses textures.
    pub brush: BrushSettings,
    pub textures: BrushTextures,
    pub toucher: Toucher,
    /// Pixels d'écran par pixel de la case (lissage mesuré à l'écran).
    pub zoom: f32,
}

impl Style {
    #[cfg(test)]
    pub fn simple(color: [u8; 3], size: f32) -> Style {
        Style { color, size, brush: BrushSettings::brush(), textures: BrushTextures::default(), toucher: Toucher::default(), zoom: 1.0 }
    }
}

/// Le crayon d'une case : dur, uni, sans texture (le trait lisse qui en sort se rejoue exactement).
pub(crate) fn pencil_settings(size: f32) -> BrushSettings {
    BrushSettings { size: size.max(0.5), opacity: 1.0, hardness: 1.0, pressure_opacity: false, min_opacity: 0.0, aliased: false, ..BrushSettings::pencil() }
}

/// La gomme d'une case : trois fois la taille du crayon.
pub(crate) fn eraser_settings(size: f32) -> BrushSettings {
    BrushSettings { size: (size * 3.0).max(2.0), ..BrushSettings::eraser() }
}

struct EraseCircle {
    radius: f32,
    last: Option<Pos2>,
    changed: bool,
    region: Rect,
}

/// Un geste en cours sur une case.
pub(crate) struct Live {
    /// La case où le geste a commencé : à elle seule il appartient.
    pub case: u64,
    pub tool: Outil,
    pub pen: bool,
    stroke: Option<StrokeState>,
    erase: Option<EraseCircle>,
    before: Option<VectorDrawing>,
    color: [u8; 3],
    opacity: f32,
    hardness: f32,
}

impl Live {
    /// Le bout du trait à l'écran : (dernier point posé, pointe, rayon) en pixels de la case, et la
    /// couleur et l'opacité. Rien pour la gomme.
    pub fn tip(&self) -> Option<((Pos2, Pos2, f32), ([u8; 3], f32))> {
        let s = self.stroke.as_ref()?;
        Some((s.tip()?, s.tip_paint()?))
    }

    /// Le geste reçoit un point (dans le repère de la case, qu'il peut dépasser).
    pub fn to(&mut self, planche: &mut Planche, p: Pos2, pressure: f32, time: f64) {
        let Some(case) = planche.case_mut(self.case) else { return };
        match self.tool {
            Outil::Crayon => {
                if let Some(s) = self.stroke.as_mut() {
                    s.add_point_at(&mut case.ink.raster, p, pressure, time, &mut case.dirty.ink);
                }
            }
            Outil::Pinceau => {
                if let Some(s) = self.stroke.as_mut() {
                    s.add_point_at(&mut case.paint.raster, p, pressure, time, &mut case.dirty.paint);
                }
            }
            Outil::Gomme => {
                if let Some(s) = self.stroke.as_mut() {
                    s.add_point_at(&mut case.paint.raster, p, pressure, time, &mut case.dirty.paint);
                }
                if let Some(e) = self.erase.as_mut() {
                    let Some(d) = case.ink.vector.as_mut() else { return };
                    // On gomme le long du segment parcouru, pas seulement aux points reçus.
                    let from = e.last.unwrap_or(p);
                    let steps = ((from.distance(p) / (e.radius * 0.5).max(0.5)).ceil() as usize).max(1);
                    let swept = Rect::from_two_pos(from, p).expand(e.radius + 1.0);
                    let region = d.elements.iter().map(|el| el.bounds().expand(1.5)).filter(|b| b.intersects(swept)).fold(Rect::NOTHING, |r, b| r.union(b));
                    let mut changed = false;
                    for k in 1..=steps {
                        let q = from + (p - from) * (k as f32 / steps as f32);
                        changed |= vector::erase_circle(d, q, e.radius, false);
                    }
                    e.last = Some(p);
                    if changed {
                        e.changed = true;
                        e.region = e.region.union(region);
                        let tiles = case.ink.rerender_region(region.expand(2.0));
                        case.dirty.ink.extend(tiles);
                    }
                }
            }
        }
    }

    /// Le geste est fini : le trait lisse prend la place de l'aperçu. Renvoie de quoi l'annuler.
    pub fn end(mut self, planche: &mut Planche) -> Option<PEdit> {
        let id = self.case;
        let case = planche.case_mut(id)?;
        match self.tool {
            Outil::Crayon => {
                let mut stroke = self.stroke.take()?;
                let mut dirty = Vec::new();
                let edit = stroke.finish(&mut case.ink.raster, &mut dirty);
                case.dirty.ink.extend(dirty);
                // Les pixels d'avant l'aperçu : l'image sans lui.
                let tiles = match edit {
                    Some(Edit::Tiles { tiles, .. }) => tiles,
                    _ => Vec::new(),
                };
                let points = vector::simplify(&std::mem::take(&mut stroke.path), 0.25);
                if points.is_empty() {
                    let changed: Vec<usize> = tiles.iter().map(|t| t.0).collect();
                    for (i, t) in tiles {
                        case.ink.raster.tiles[i] = t;
                    }
                    case.dirty.ink.extend(changed);
                    return None;
                }
                let element = Element::Stroke { points, color: self.color, opacity: self.opacity, hardness: self.hardness, brush: None, trace: Default::default() };
                let before = self.before.take().unwrap_or_default();
                case.ink.vector.get_or_insert_with(Default::default).elements.push(element.clone());
                let changed = case.ink.render_on_top(tiles, &element);
                case.dirty.ink.extend(changed);
                Some(PEdit::Ink { case: id, drawing: before })
            }
            Outil::Pinceau => {
                let mut stroke = self.stroke.take()?;
                let mut dirty = Vec::new();
                let edit = stroke.finish(&mut case.paint.raster, &mut dirty);
                case.dirty.paint.extend(dirty);
                case.paint.version += 1;
                match edit {
                    Some(Edit::Tiles { tiles, .. }) => Some(PEdit::Paint { case: id, tiles }),
                    _ => None,
                }
            }
            Outil::Gomme => {
                let mut edits = Vec::new();
                if let Some(mut stroke) = self.stroke.take() {
                    let mut dirty = Vec::new();
                    let edit = stroke.finish(&mut case.paint.raster, &mut dirty);
                    case.dirty.paint.extend(dirty);
                    case.paint.version += 1;
                    if let Some(Edit::Tiles { tiles, .. }) = edit {
                        edits.push(PEdit::Paint { case: id, tiles });
                    }
                }
                if let Some(e) = self.erase.take() {
                    if e.changed {
                        // Un trait de moins qu'un rendu complet l'aurait fait : on refait la zone touchée.
                        let tiles = case.ink.rerender_region(e.region.expand(2.0));
                        case.dirty.ink.extend(tiles);
                        edits.push(PEdit::Ink { case: id, drawing: self.before.take().unwrap_or_default() });
                    }
                }
                match edits.len() {
                    0 => None,
                    1 => edits.pop(),
                    _ => Some(PEdit::Group(edits)),
                }
            }
        }
    }
}

impl Planche {
    /// Début d'un geste sur la case `index` (le point `p` est dans le repère de la case). Le premier
    /// point est posé. Rien si la case n'existe pas.
    #[allow(clippy::too_many_arguments)]
    pub fn begin(&mut self, index: usize, tool: Outil, style: &Style, p: Pos2, pressure: f32, pen: bool, time: f64) -> Option<Live> {
        let case = self.cases.get_mut(index)?;
        case.render_if_stale();
        let id = case.id;
        let mut live = Live { case: id, tool, pen, stroke: None, erase: None, before: None, color: style.color, opacity: 1.0, hardness: 1.0 };
        match tool {
            Outil::Crayon => {
                let s = pencil_settings(style.size);
                live.opacity = s.opacity;
                live.hardness = s.hardness;
                live.before = Some(case.ink.vector.clone().unwrap_or_default());
                live.stroke = Some(StrokeState::new(id, StrokeMode::Paint(style.color), s, None, pen).with_feel(style.toucher).with_zoom(style.zoom));
            }
            Outil::Pinceau => {
                let s = BrushSettings { size: style.size.max(0.5), ..style.brush };
                live.stroke = Some(
                    StrokeState::new(id, StrokeMode::Paint(style.color), s, None, pen).with_textures(style.textures.clone()).with_feel(style.toucher).with_zoom(style.zoom),
                );
            }
            Outil::Gomme => {
                let s = eraser_settings(style.size);
                if case.has_ink() {
                    live.before = Some(case.ink.vector.clone().unwrap_or_default());
                    live.erase = Some(EraseCircle { radius: s.size * 0.5, last: None, changed: false, region: Rect::NOTHING });
                }
                if case.has_paint() {
                    live.stroke = Some(StrokeState::new(id, StrokeMode::Erase, s, None, pen).with_feel(style.toucher).with_zoom(style.zoom));
                }
            }
        }
        live.to(self, p, pressure, time);
        Some(live)
    }
}

// ------------------------------------------------------------------------------------ annulation

/// Une étape d'annulation de la planche : l'état d'avant. L'appliquer rend l'état qu'elle remplace
/// (la même opération annule et rétablit).
pub(crate) enum PEdit {
    /// L'encre d'une case avant la modification.
    Ink { case: u64, drawing: VectorDrawing },
    /// Les tuiles de la peinture d'une case avant la modification.
    Paint { case: u64, tiles: Vec<(usize, Option<Tile>)> },
    /// Toute la planche (ordre, légendes, cases ajoutées ou retirées, vidées, grille).
    Whole(Box<Planche>),
    Group(Vec<PEdit>),
    /// Des plans ont été créés d'après des cases : les coches d'avant, et le nombre d'étapes à
    /// annuler de l'historique du projet juste après (l'étape de la création y est la dernière tant
    /// que ce nombre est le même et que son nom est `PLANS_STEP`).
    Plans { used: Vec<(u64, bool)>, depth: usize },
}

/// Le nom de l'étape d'annulation du projet que fait « Créer les plans ».
pub(crate) const PLANS_STEP: &str = "Créer les plans des crayonnés";

impl PEdit {
    pub fn apply(self, p: &mut Planche) -> PEdit {
        match self {
            PEdit::Ink { case, drawing } => {
                let Some(c) = p.case_mut(case) else { return PEdit::Group(Vec::new()) };
                let old = std::mem::replace(c.ink.vector.get_or_insert_with(Default::default), drawing);
                c.ink.rerender();
                c.stale = false;
                c.dirty.all = true;
                PEdit::Ink { case, drawing: old }
            }
            PEdit::Paint { case, tiles } => {
                let Some(c) = p.case_mut(case) else { return PEdit::Group(Vec::new()) };
                let mut old = Vec::with_capacity(tiles.len());
                for (i, t) in tiles {
                    if let Some(slot) = c.paint.raster.tiles.get_mut(i) {
                        old.push((i, std::mem::replace(slot, t)));
                        c.dirty.paint.push(i);
                    }
                }
                c.paint.version += 1;
                PEdit::Paint { case, tiles: old }
            }
            PEdit::Whole(w) => {
                let old = std::mem::replace(p, *w);
                p.invalidate();
                PEdit::Whole(Box::new(old))
            }
            PEdit::Group(v) => PEdit::Group(v.into_iter().rev().map(|e| e.apply(p)).collect()),
            PEdit::Plans { used, depth } => {
                let mut now = Vec::with_capacity(used.len());
                for (id, flag) in used {
                    if let Some(c) = p.case_mut(id) {
                        now.push((id, std::mem::replace(&mut c.used, flag)));
                    }
                }
                PEdit::Plans { used: now, depth }
            }
        }
    }

    fn plans_depth(&self) -> Option<usize> {
        match self {
            PEdit::Plans { depth, .. } => Some(*depth),
            _ => None,
        }
    }
}

/// Ce qu'une annulation (ou un rétablissement) a fait.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Undone {
    Nothing,
    /// Une étape de la planche a été défaite.
    Page,
    /// Des plans avaient été faits : les coches sont revenues, et l'étape du projet avait ce nombre
    /// d'étapes à annuler après elle (à défaire ou refaire dans le projet si elle y est toujours).
    Plans(usize),
}

#[derive(Default)]
pub(crate) struct PageHistory {
    undo: Vec<PEdit>,
    redo: Vec<PEdit>,
}

impl PageHistory {
    pub fn push(&mut self, edit: PEdit) {
        self.undo.push(edit);
        self.redo.clear();
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, p: &mut Planche) -> Undone {
        let Some(edit) = self.undo.pop() else { return Undone::Nothing };
        let depth = edit.plans_depth();
        self.redo.push(edit.apply(p));
        depth.map_or(Undone::Page, Undone::Plans)
    }

    pub fn redo(&mut self, p: &mut Planche) -> Undone {
        let Some(edit) = self.redo.pop() else { return Undone::Nothing };
        let depth = edit.plans_depth();
        self.undo.push(edit.apply(p));
        depth.map_or(Undone::Page, Undone::Plans)
    }
}

// ------------------------------------------------------------------------------------ plans

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Placement {
    /// Les plans suivent le plan courant, dans sa séquence.
    #[default]
    AfterCurrent,
    /// Une séquence neuve, après la séquence du plan courant.
    NewSequence,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PlanOptions {
    pub placement: Placement,
    /// Durée de chaque plan, en secondes (2 s au départ).
    pub seconds: f32,
}

impl Default for PlanOptions {
    fn default() -> Self {
        PlanOptions { placement: Placement::AfterCurrent, seconds: 2.0 }
    }
}

/// Les plans faits.
pub(crate) struct Created {
    /// Leurs numéros, dans l'ordre des cases.
    pub ids: Vec<u64>,
    /// Le rang du premier dans le montage.
    pub first: usize,
}

/// Du repère d'une case (taille `size`) à la feuille du projet : mise à l'échelle uniforme pour que
/// la case remplisse le cadre `frame` (sa place sur la feuille), centrée.
pub(crate) fn fit(size: [usize; 2], frame: Rect) -> Affine {
    let (w, h) = (size[0].max(1) as f32, size[1].max(1) as f32);
    let s = (frame.width() / w).min(frame.height() / h);
    Affine {
        a: s,
        d: s,
        tx: frame.min.x + (frame.width() - w * s) * 0.5,
        ty: frame.min.y + (frame.height() - h * s) * 0.5,
        ..Affine::IDENTITY
    }
}

/// Les traits de l'encre d'une case, coupés à la case puis mis à l'échelle de la feuille. Un trait
/// reste un trait (vectoriel), sa largeur suit l'échelle.
pub(crate) fn scaled_ink(drawing: &VectorDrawing, size: [usize; 2], to: &Affine) -> Vec<Element> {
    let bounds = Rect::from_min_size(Pos2::ZERO, vec2(size[0] as f32, size[1] as f32));
    let mut out = Vec::new();
    for e in &drawing.elements {
        match e {
            Element::Stroke { points, color, opacity, hardness, .. } => {
                for piece in clip_stroke(points, bounds) {
                    let stroke = Element::Stroke { points: piece, color: *color, opacity: *opacity, hardness: *hardness, brush: None, trace: Default::default() };
                    out.push(stroke.transformed(to));
                }
            }
            other => out.push(other.transformed(to)),
        }
    }
    out
}

/// Fait un plan de chaque case `picks` (rangs dans la planche, dans l'ordre) : le dessin de la case
/// va sur un calque « Crayonné » du plan, mis du cadre de la case au cadre du projet (traits
/// vectoriels mis à l'échelle, peinture bitmap rééchantillonnée), sa légende dans le texte d'action,
/// la durée voulue. Chaque plan a sa scène ; ils s'insèrent ensemble, après le plan `current` (dans
/// sa séquence) ou dans une séquence neuve. Le document ne garde pas d'étape d'annulation : l'appelant
/// la prend avant (`App::snapshot`). Rien si `picks` est vide.
pub(crate) fn create_plans(doc: &mut Document, planche: &Planche, picks: &[usize], current: usize, opts: &PlanOptions) -> Option<Created> {
    if picks.is_empty() || doc.panels.is_empty() {
        return None;
    }
    let current = current.min(doc.panels.len() - 1);
    let frames = ((opts.seconds.max(0.0) * doc.fps as f32).round() as usize).clamp(1, crate::rognage::MAX_PLAN);
    let (seq, at) = match opts.placement {
        Placement::AfterCurrent => (doc.sequence_of(current), doc.scene_run(current).1),
        Placement::NewSequence => {
            let end = doc.sequence_run(current).1;
            (doc.new_sequence(), end)
        }
    };
    let to = fit(planche.size, doc.frame_rect());
    let mut panels = Vec::new();
    for &i in picks {
        let Some(case) = planche.cases.get(i) else { continue };
        let scene = doc.new_scene(seq);
        let mut panel = doc.make_panel(scene);
        panel.length = frames;
        panel.action = case.caption.trim().to_string();
        let mut below: Vec<Layer> = Vec::new();
        // L'encre : un calque vectoriel « Crayonné » (il existe même si la case est vide).
        let mut crayon = Layer::new("Crayonné", LayerKind::Vector);
        if let Some(drawing) = case.ink.vector.as_ref().filter(|d| !d.elements.is_empty()) {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Vector);
            cel.vector = Some(VectorDrawing { elements: scaled_ink(drawing, planche.size, &to) });
            cel.rerender();
            let id = doc.add_cel(cel);
            crayon.frames = vec![Some(id); frames];
        }
        below.push(crayon);
        // La peinture : un calque bitmap à part, rééchantillonnée.
        if case.has_paint() {
            let moved = case.paint.raster.scaled(doc.width, doc.height, to.a).placed_in(doc.width, doc.height, to.tx.round().max(0.0) as usize, to.ty.round().max(0.0) as usize);
            let id = doc.add_cel(Cel::bitmap(moved));
            let mut paint = Layer::new("Crayonné (pinceau)", LayerKind::Bitmap);
            paint.frames = vec![Some(id); frames];
            below.push(paint);
        }
        // Le calque « Crayonné » (et la peinture) sous le calque de dessin vide du plan.
        panel.layers.splice(0..0, below);
        panels.push(panel);
    }
    if panels.is_empty() {
        return None;
    }
    let ids = panels.iter().map(|p| p.id).collect();
    let first = doc.insert_plans(at, panels);
    Some(Created { ids, first })
}

// ------------------------------------------------------------------------------------ fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    /// Colonnes et lignes d'une page.
    grille: [usize; 2],
    /// Taille d'une case en pixels.
    taille: [usize; 2],
    #[serde(default)]
    cases: Vec<CaseEntry>,
}

#[derive(Serialize, Deserialize)]
struct CaseEntry {
    id: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    legende: String,
    #[serde(default, skip_serializing_if = "is_false")]
    utilisee: bool,
    /// L'encre : les traits lisses de la case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    traits: Vec<Element>,
    /// La peinture : un PNG (RGBA) en base 64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peinture: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Le contenu de `crayonnes.json` pour `doc` ; rien s'il n'y a rien à garder (le fichier est alors
/// retiré, s'il est lisible : `write_sidecar`).
#[allow(dead_code)]
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    bytes_of(&doc.crayonnes)
}

/// Comme `to_bytes`, pour une planche (l'écriture se fait sur un autre fil, avec une copie).
pub(crate) fn bytes_of(planche: &Planche) -> Result<Option<Vec<u8>>, String> {
    if planche.is_blank() {
        return Ok(None);
    }
    let mut cases = Vec::with_capacity(planche.cases.len());
    for c in &planche.cases {
        let peinture = if c.has_paint() { Some(b64_encode(&crate::io::encode_png(&c.paint.raster.to_rgba(), c.paint.raster.width, c.paint.raster.height)?)) } else { None };
        cases.push(CaseEntry {
            id: c.id,
            legende: c.caption.clone(),
            utilisee: c.used,
            traits: c.ink.vector.as_ref().map(|v| v.elements.clone()).unwrap_or_default(),
            peinture,
        });
    }
    let sidecar = Sidecar { format: FORMAT.into(), version: VERSION, grille: [planche.grid.0, planche.grid.1], taille: planche.size, cases };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Un trait relu : valeurs finies et raisonnables, sans pinceau ni trace (les traits d'une case sont
/// lisses). Rien pour un élément absurde.
fn sane(e: Element) -> Option<Element> {
    let coord = |v: f32| v.is_finite() && v.abs() <= MAX_COORD;
    match e {
        Element::Stroke { points, color, opacity, hardness, .. } => {
            if points.is_empty() || !points.iter().all(|p| coord(p.x) && coord(p.y) && p.r.is_finite() && (0.0..=MAX_RADIUS).contains(&p.r)) || !opacity.is_finite() || !hardness.is_finite() {
                return None;
            }
            Some(Element::Stroke { points, color, opacity: opacity.clamp(0.0, 1.0), hardness: hardness.clamp(0.0, 1.0), brush: None, trace: Default::default() })
        }
        Element::Fill { contours, color, opacity } => {
            if !contours.iter().flatten().all(|p| coord(p[0]) && coord(p[1])) || !opacity.is_finite() {
                return None;
            }
            Some(Element::Fill { contours, color, opacity: opacity.clamp(0.0, 1.0) })
        }
        // Pas de texte dans les cases.
        Element::Text { .. } => None,
    }
}

/// La planche lue dans le texte de `crayonnes.json`. Identifiants uniques et non nuls (un doublon
/// reçoit un identifiant neuf), légendes bornées, traits absurdes écartés.
pub(crate) fn parse(text: &str) -> Result<Planche, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de crayonnés".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let [w, h] = sidecar.taille;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("taille de case absurde ({w} × {h})"));
    }
    let (cols, rows) = (sidecar.grille[0], sidecar.grille[1]);
    if cols == 0 || rows == 0 || cols > 8 || rows > 8 {
        return Err(format!("grille absurde ({cols} × {rows})"));
    }
    if sidecar.cases.len() > MAX_CASES {
        return Err(format!("trop de cases ({})", sidecar.cases.len()));
    }
    let mut planche = Planche { size: [w, h], grid: (cols, rows), cases: Vec::with_capacity(sidecar.cases.len()), next_id: 1 };
    planche.next_id = sidecar.cases.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    let mut seen = std::collections::HashSet::new();
    for entry in sidecar.cases {
        let mut id = entry.id;
        if id == 0 || !seen.insert(id) {
            id = planche.fresh_id();
            seen.insert(id);
        }
        let mut case = Case::blank(id, [w, h]);
        case.caption = entry.legende.chars().take(CAPTION_MAX).collect();
        case.used = entry.utilisee;
        let elements: Vec<Element> = entry.traits.into_iter().take(MAX_ELEMENTS).filter_map(sane).collect();
        if !elements.is_empty() {
            case.ink.vector = Some(VectorDrawing { elements });
            case.stale = true;
        }
        if let Some(png) = entry.peinture.as_deref().and_then(b64_decode) {
            if let Some(raster) = decode_paint(&png, [w, h]) {
                case.paint = Cel::bitmap(raster);
            }
        }
        case.dirty.all = true;
        planche.cases.push(case);
    }
    Ok(planche)
}

/// La peinture d'une case, d'un PNG : seulement à la taille de la case.
fn decode_paint(png: &[u8], size: [usize; 2]) -> Option<Raster> {
    use image::ImageDecoder;
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(png)).ok()?;
    let (w, h) = decoder.dimensions();
    if (w as usize, h as usize) != (size[0], size[1]) {
        return None;
    }
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.into_rgba8();
    Some(Raster::from_rgba(size[0], size[1], img.as_raw()))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base 64 (alphabet standard, avec « = »).
pub(crate) fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Décode du base 64 ; rien si le texte n'en est pas.
pub(crate) fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if clean.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for quad in clean.chunks(4) {
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 {
            return None;
        }
        let mut n = 0u32;
        for (k, &c) in quad.iter().enumerate() {
            let v = if k >= 4 - pad { 0 } else { value(c)? };
            n = n << 6 | v;
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}
