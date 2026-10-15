//! Planche de la vue Vignettes (phases 7 et 8), sans interface : où se placent les vignettes, où
//! tombent les plans qu'on lâche, et les notes (traits et textes) posées sur la planche.
//!
//! La planche a ses propres coordonnées (des points d'écran au zoom 1). Les vignettes y sont
//! rangées comme sur des pages de vignettes : une par image de storyboard (13a : le début de
//! chaque plan, puis chacune de ses marques d'image), de gauche à droite (un plan peut revenir à
//! la ligne), `colonnes` par ligne (6 au plus), `lignes` par page (0 : une seule page). Une note libre reste où on l'a posée (choix d'Anthony) ; une note attachée à une
//! vignette la suit, à son échelle.

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::document::Document;
use crate::vector::VPoint;

/// Bande du titre (numéro, « Plan 3 », durée) au-dessus de chaque vignette.
pub const HEADER_H: f32 = 18.0;
/// Marge autour des pages, sur la planche ; marge intérieure d'une page ; écart entre deux pages.
pub const MARGIN: f32 = 28.0;
pub const PAGE_PAD: f32 = 22.0;
pub const PAGE_GAP: f32 = 36.0;
/// Bornes de la largeur d'une vignette.
pub const CARD_MIN: f32 = 100.0;
pub const CARD_MAX: f32 = 700.0;
/// Au plus 6 vignettes par ligne (demande d'Anthony).
pub const MAX_COLUMNS: usize = 6;
pub const MAX_ROWS: usize = 12;
pub const MAX_SPACING: f32 = 120.0;

/// Réglages de la disposition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardSettings {
    pub card_w: f32,
    pub columns: usize,
    /// Lignes par page ; 0 : une seule page, aussi longue qu'il faut.
    pub rows: usize,
    pub spacing: f32,
    /// Chaque plan commence une nouvelle ligne.
    pub scene_breaks: bool,
}

impl Default for BoardSettings {
    fn default() -> Self {
        BoardSettings { card_w: 240.0, columns: 4, rows: 4, spacing: 18.0, scene_breaks: false }
    }
}

/// Une vignette sur la planche (13a : une par image de storyboard) : le plan (indice, id), la
/// vignette (`id` : celui du plan pour sa première image, sinon celui de la marque d'image), les
/// images du plan qu'elle couvre [`frame`, `end`), sa scène, la carte entière (titre compris) et
/// l'image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPlace {
    pub panel: usize,
    pub plan: u64,
    pub id: u64,
    pub frame: usize,
    pub end: usize,
    pub scene: u64,
    pub card: Rect,
    pub thumb: Rect,
}

impl CardPlace {
    /// La première vignette de son plan.
    pub fn starts_plan(&self) -> bool {
        self.frame == 0
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardLayout {
    pub cards: Vec<CardPlace>,
    /// Les pages (fond blanc), et les cases vides de la dernière page.
    pub pages: Vec<Rect>,
    pub empty: Vec<Rect>,
    pub size: Vec2,
}

impl BoardLayout {
    pub fn card_of(&self, id: u64) -> Option<&CardPlace> {
        self.cards.iter().find(|c| c.id == id)
    }
}

/// Images de storyboard du plan `index` : (id de la vignette, première image, fin).
pub fn plan_boards(doc: &Document, index: usize) -> Vec<(u64, usize, usize)> {
    let p = &doc.panels[index];
    let n = p.frame_count();
    let starts: Vec<(u64, usize)> = std::iter::once((p.id, 0)).chain(p.marks.iter().filter(|m| m.frame < n).map(|m| (m.id, m.frame))).collect();
    (0..starts.len()).map(|k| (starts[k].0, starts[k].1, starts.get(k + 1).map_or(n, |s| s.1))).collect()
}

/// Range les vignettes sur des pages (voir le début du fichier). Ne dépend pas de la taille de la
/// vue : les notes libres restent à leur place quand la fenêtre change.
pub fn layout(doc: &Document, s: &BoardSettings) -> BoardLayout {
    let cols = s.columns.clamp(1, MAX_COLUMNS);
    let card_w = s.card_w.clamp(CARD_MIN, CARD_MAX);
    let gap = s.spacing.clamp(0.0, MAX_SPACING);
    let thumb_h = card_w * doc.frame[1] as f32 / doc.frame[0].max(1) as f32;
    let card_h = HEADER_H + thumb_h;
    // Ligne et colonne de chaque vignette ; « plan à la ligne » : chaque plan commence une ligne.
    let mut boards: Vec<(usize, u64, usize, usize)> = Vec::new();
    for i in 0..doc.panels.len() {
        boards.extend(plan_boards(doc, i).into_iter().map(|(id, frame, end)| (i, id, frame, end)));
    }
    let mut slots: Vec<(usize, usize)> = Vec::with_capacity(boards.len());
    let (mut row, mut col) = (0, 0);
    for (k, &(_, _, frame, _)) in boards.iter().enumerate() {
        let new_plan = k > 0 && s.scene_breaks && frame == 0;
        if k > 0 && (col == cols || new_plan) {
            row += 1;
            col = 0;
        }
        slots.push((row, col));
        col += 1;
    }
    let rows_used = row + 1;
    let per_page = if s.rows == 0 { rows_used } else { s.rows.min(MAX_ROWS) };
    let pages = rows_used.div_ceil(per_page);
    let page_w = PAGE_PAD * 2.0 + cols as f32 * card_w + (cols - 1) as f32 * gap;
    let page_h = PAGE_PAD * 2.0 + per_page as f32 * card_h + (per_page - 1) as f32 * gap;
    let page_top = |p: usize| MARGIN + p as f32 * (page_h + PAGE_GAP);
    let cell = |r: usize, c: usize| {
        let (p, r) = (r / per_page, r % per_page);
        let min = pos2(MARGIN + PAGE_PAD + c as f32 * (card_w + gap), page_top(p) + PAGE_PAD + r as f32 * (card_h + gap));
        Rect::from_min_size(min, vec2(card_w, card_h))
    };
    let mut out = BoardLayout::default();
    for (&(i, id, frame, end), &(r, c)) in boards.iter().zip(&slots) {
        let card = cell(r, c);
        let thumb = Rect::from_min_max(pos2(card.left(), card.top() + HEADER_H), card.max);
        let p = &doc.panels[i];
        out.cards.push(CardPlace { panel: i, plan: p.id, id, frame, end, scene: p.scene, card, thumb });
    }
    out.pages = (0..pages).map(|p| Rect::from_min_size(pos2(MARGIN, page_top(p)), vec2(page_w, page_h))).collect();
    // Cases vides : la fin de la dernière ligne, puis les lignes restantes de la dernière page.
    for c in col..cols {
        out.empty.push(cell(row, c));
    }
    for r in row + 1..pages * per_page {
        for c in 0..cols {
            out.empty.push(cell(r, c));
        }
    }
    out.size = vec2(MARGIN * 2.0 + page_w, page_top(pages) - PAGE_GAP + MARGIN);
    out
}

/// Où vont des plans lâchés : devant le plan `before` (indice dans le film), ou à la fin.
/// `line` : où montrer le repère (un trait vertical).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropTarget {
    pub before: Option<usize>,
    pub line: (Pos2, Pos2),
}

/// La place visée par le point `p` : à côté du plan de la vignette la plus proche — devant lui si
/// l'on vise la moitié gauche de sa première vignette, après lui sinon.
pub fn drop_target(layout: &BoardLayout, p: Pos2, gap: f32) -> Option<DropTarget> {
    let k = (0..layout.cards.len()).min_by(|&a, &b| {
        layout.cards[a].card.distance_to_pos(p).total_cmp(&layout.cards[b].card.distance_to_pos(p))
    })?;
    let c = &layout.cards[k];
    let line = |card: Rect, x: f32| (pos2(x, card.top() - 4.0), pos2(x, card.bottom() + 4.0));
    if c.starts_plan() && p.x < c.card.center().x {
        return Some(DropTarget { before: Some(c.panel), line: line(c.card, c.card.left() - gap * 0.5) });
    }
    let last = layout.cards.iter().rev().find(|x| x.panel == c.panel).unwrap_or(c);
    let next = layout.cards.iter().find(|x| x.panel == c.panel + 1).map(|x| x.panel);
    Some(DropTarget { before: next, line: line(last.card, last.card.right() + gap * 0.5) })
}

/// Plans dont une vignette touche le rectangle `r` (sélection au lasso rectangulaire), sans
/// doublon, dans l'ordre du film.
pub fn cards_in(layout: &BoardLayout, r: Rect) -> Vec<usize> {
    let mut plans: Vec<usize> = layout.cards.iter().filter(|c| c.card.intersects(r)).map(|c| c.panel).collect();
    plans.dedup();
    plans
}
// ---------------------------------------------------------------- notes

/// Un trait de note. Libre : points en coordonnées de la planche. Attaché (`anchor` : id du
/// panneau) : points relatifs au coin de la vignette, en largeurs de vignette (rayons compris).
#[derive(Clone, Debug, PartialEq)]
pub struct NoteStroke {
    pub points: Vec<VPoint>,
    pub color: [u8; 3],
    pub anchor: Option<u64>,
}

/// Un texte de note : coin haut gauche, taille des lettres ; mêmes règles qu'un trait pour
/// l'attache.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteText {
    pub pos: Pos2,
    pub text: String,
    pub size: f32,
    pub color: [u8; 3],
    pub anchor: Option<u64>,
}

/// Taille par défaut d'un texte de note, en points de la planche.
pub const NOTE_TEXT_SIZE: f32 = 16.0;

/// Une note de la planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoteRef {
    Stroke(usize),
    Text(usize),
}

/// Les notes de la planche, enregistrées avec le projet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub strokes: Vec<NoteStroke>,
    pub texts: Vec<NoteText>,
}

/// Ce que couvre un trait (épaisseur comprise).
pub fn points_bounds(points: &[VPoint]) -> Rect {
    points.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0))))
}

/// Le trait passe-t-il à moins de `radius` du point `p` ?
pub fn touches(points: &[VPoint], p: Pos2, radius: f32) -> bool {
    match points {
        [] => false,
        [a] => a.pos().distance(p) <= radius + a.r,
        pts => pts.windows(2).any(|w| {
            let (a, b) = (w[0].pos(), w[1].pos());
            let ab = b - a;
            let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            (a + ab * t).distance(p) <= radius + w[0].r.max(w[1].r)
        }),
    }
}

/// Repère d'une vignette : son coin et sa largeur (une note attachée s'y rapporte).
fn frame_of(layout: &BoardLayout, anchor: Option<u64>) -> Option<(Pos2, f32)> {
    match anchor {
        None => Some((Pos2::ZERO, 1.0)),
        Some(id) => layout.card_of(id).map(|c| (c.card.min, c.card.width())),
    }
}

impl Board {
    /// Les points d'un trait sur la planche (rien si sa vignette n'existe plus).
    pub fn stroke_points(&self, layout: &BoardLayout, i: usize) -> Option<Vec<VPoint>> {
        let s = self.strokes.get(i)?;
        let (o, k) = frame_of(layout, s.anchor)?;
        Some(s.points.iter().map(|p| VPoint { x: o.x + p.x * k, y: o.y + p.y * k, r: p.r * k }).collect())
    }

    /// Coin et taille d'un texte sur la planche.
    pub fn text_place(&self, layout: &BoardLayout, i: usize) -> Option<(Pos2, f32)> {
        let t = self.texts.get(i)?;
        let (o, k) = frame_of(layout, t.anchor)?;
        Some((o + t.pos.to_vec2() * k, t.size * k))
    }

    /// Ajoute un trait tracé sur la planche, attaché à la vignette `anchor` s'il y en a une.
    pub fn add_stroke(&mut self, layout: &BoardLayout, points: Vec<VPoint>, color: [u8; 3], anchor: Option<u64>) {
        let mut s = NoteStroke { points, color, anchor: None };
        if let Some((o, k)) = anchor.and_then(|id| frame_of(layout, Some(id))) {
            s.points = s.points.iter().map(|p| VPoint { x: (p.x - o.x) / k, y: (p.y - o.y) / k, r: p.r / k }).collect();
            s.anchor = anchor;
        }
        self.strokes.push(s);
    }

    /// Ajoute un texte posé en `pos` (planche), de taille `size`, attaché ou non.
    pub fn add_text(&mut self, layout: &BoardLayout, pos: Pos2, text: String, size: f32, color: [u8; 3], anchor: Option<u64>) {
        let mut t = NoteText { pos, text, size, color, anchor: None };
        if let Some((o, k)) = anchor.and_then(|id| frame_of(layout, Some(id))) {
            t.pos = ((pos - o) / k).to_pos2();
            t.size = size / k;
            t.anchor = anchor;
        }
        self.texts.push(t);
    }

    /// Déplace les notes `sel` de `delta` (planche).
    pub fn translate(&mut self, layout: &BoardLayout, sel: &[NoteRef], delta: Vec2) {
        for r in sel {
            match *r {
                NoteRef::Stroke(i) => {
                    let Some(s) = self.strokes.get_mut(i) else { continue };
                    let Some((_, k)) = frame_of(layout, s.anchor) else { continue };
                    for p in &mut s.points {
                        p.x += delta.x / k;
                        p.y += delta.y / k;
                    }
                }
                NoteRef::Text(i) => {
                    let Some(t) = self.texts.get_mut(i) else { continue };
                    let Some((_, k)) = frame_of(layout, t.anchor) else { continue };
                    t.pos += delta / k;
                }
            }
        }
    }

    /// Attache les notes `sel` à la vignette `id` (`None` : les rend libres), sans qu'elles bougent.
    pub fn set_anchor(&mut self, layout: &BoardLayout, sel: &[NoteRef], id: Option<u64>) {
        let Some((no, nk)) = frame_of(layout, id) else { return };
        for r in sel {
            match *r {
                NoteRef::Stroke(i) => {
                    let Some(pts) = self.stroke_points(layout, i) else { continue };
                    let s = &mut self.strokes[i];
                    s.points = pts.iter().map(|p| VPoint { x: (p.x - no.x) / nk, y: (p.y - no.y) / nk, r: p.r / nk }).collect();
                    s.anchor = id;
                }
                NoteRef::Text(i) => {
                    let Some((pos, size)) = self.text_place(layout, i) else { continue };
                    let t = &mut self.texts[i];
                    t.pos = ((pos - no) / nk).to_pos2();
                    t.size = size / nk;
                    t.anchor = id;
                }
            }
        }
    }

    /// Rend libres, à leur place, les notes attachées aux panneaux `ids` (avant de les retirer).
    pub fn release(&mut self, layout: &BoardLayout, ids: &[u64]) {
        let sel: Vec<NoteRef> = (0..self.strokes.len())
            .filter(|&i| self.strokes[i].anchor.is_some_and(|a| ids.contains(&a)))
            .map(NoteRef::Stroke)
            .chain((0..self.texts.len()).filter(|&i| self.texts[i].anchor.is_some_and(|a| ids.contains(&a))).map(NoteRef::Text))
            .collect();
        self.set_anchor(layout, &sel, None);
    }

    /// Retire les notes `sel`.
    pub fn remove(&mut self, sel: &[NoteRef]) {
        let mut strokes: Vec<usize> = sel.iter().filter_map(|r| if let NoteRef::Stroke(i) = r { Some(*i) } else { None }).collect();
        let mut texts: Vec<usize> = sel.iter().filter_map(|r| if let NoteRef::Text(i) = r { Some(*i) } else { None }).collect();
        strokes.sort_unstable();
        strokes.dedup();
        texts.sort_unstable();
        texts.dedup();
        for i in strokes.into_iter().rev() {
            if i < self.strokes.len() {
                self.strokes.remove(i);
            }
        }
        for i in texts.into_iter().rev() {
            if i < self.texts.len() {
                self.texts.remove(i);
            }
        }
    }

    /// Gomme : retire les traits que touche le cercle (`p`, `radius`). Vrai si un trait est parti.
    pub fn erase_at(&mut self, layout: &BoardLayout, p: Pos2, radius: f32) -> bool {
        let hit: Vec<NoteRef> = (0..self.strokes.len())
            .filter(|&i| self.stroke_points(layout, i).is_some_and(|pts| touches(&pts, p, radius)))
            .map(NoteRef::Stroke)
            .collect();
        self.remove(&hit);
        !hit.is_empty()
    }

    /// Traits que touche le rectangle `r` (lasso).
    pub fn strokes_in(&self, layout: &BoardLayout, r: Rect) -> Vec<NoteRef> {
        (0..self.strokes.len())
            .filter(|&i| self.stroke_points(layout, i).is_some_and(|pts| points_bounds(&pts).intersects(r)))
            .map(NoteRef::Stroke)
            .collect()
    }

    /// Le trait le plus haut sous le point `p` (à `tol` près).
    pub fn stroke_at(&self, layout: &BoardLayout, p: Pos2, tol: f32) -> Option<usize> {
        (0..self.strokes.len()).rev().find(|&i| self.stroke_points(layout, i).is_some_and(|pts| touches(&pts, p, tol)))
    }

    /// Recolore les notes `sel`.
    pub fn recolor(&mut self, sel: &[NoteRef], color: [u8; 3]) {
        for r in sel {
            match *r {
                NoteRef::Stroke(i) => {
                    if let Some(s) = self.strokes.get_mut(i) {
                        s.color = color;
                    }
                }
                NoteRef::Text(i) => {
                    if let Some(t) = self.texts.get_mut(i) {
                        t.color = color;
                    }
                }
            }
        }
    }
}

/// La vignette la plus proche du rectangle `r` (pour attacher une note).
pub fn nearest_card(layout: &BoardLayout, r: Rect) -> Option<u64> {
    let c = r.center();
    layout.cards.iter().min_by(|a, b| a.card.distance_to_pos(c).total_cmp(&b.card.distance_to_pos(c))).map(|c| c.id)
}

/// Ajoute le point `p` à un trait en cours, sauf s'il est trop près du précédent (`min_step`).
pub fn push_note_point(points: &mut Vec<VPoint>, p: VPoint, min_step: f32) {
    match points.last_mut() {
        Some(last) if last.pos().distance(p.pos()) < min_step => last.r = last.r.max(p.r),
        _ => points.push(p),
    }
}
