//! Planche de la vue Vignettes, sans interface : où se placent les vignettes, les barres de coupe
//! entre elles, les cases vides à dessiner, les bandes de notes, et les notes (traits et textes)
//! posées sur la planche.
//!
//! La planche a ses propres coordonnées (des points d'écran au zoom 1). Les vignettes y sont
//! rangées comme sur des pages : une par image de storyboard (13a : le début de chaque plan, puis
//! chacune de ses marques d'image), de gauche à droite, `colonnes` par ligne (6 au plus), `lignes`
//! par page (0 : une seule page). Une ligne ne se casse jamais à un changement de plan : ce qui
//! sépare deux vignettes qui se suivent, c'est l'écart (serré dans un même plan, large à une coupe)
//! et une courte barre verticale (grise : même plan ; noire : coupe).
//!
//! Vignettes simples (Anthony, 2026-10-21) : aucune bande de titre ; seul le numéro du plan, au-dessus
//! de sa première vignette ; sous chaque vignette, une bande de note (de 1 à 3 lignes ; la ligne
//! suivante descend d'autant) ; après la dernière vignette, des cases vides où l'on dessine pour faire
//! le plan suivant. Une note libre reste où on l'a posée ; une note attachée à une vignette la suit,
//! à son échelle.

use std::collections::HashMap;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::document::Document;
use crate::vector::VPoint;

/// Hauteur de la ligne du numéro de plan, au-dessus de chaque vignette (réservée sur toute la ligne).
pub const LABEL_H: f32 = 16.0;
/// Espace entre une vignette et sa bande de note ; hauteur d'une ligne de note ; lignes affichées au plus.
pub const NOTE_GAP: f32 = 3.0;
pub const NOTE_LINE_H: f32 = 13.0;
pub const NOTE_MAX_LINES: usize = 3;
/// Espace sous la dernière ligne de note, avant la ligne suivante.
pub const ROW_GAP: f32 = 8.0;
/// Écart entre deux vignettes d'un même plan, et à une coupe.
pub const GAP_SAME: f32 = 10.0;
pub const GAP_CUT: f32 = 30.0;
/// Barre de coupe : épaisseur, et part de la hauteur de la vignette (un tiers).
pub const BAR_W: f32 = 1.5;
pub const BAR_FRACTION: f32 = 1.0 / 3.0;
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
/// Borne de l'écart enregistré dans les réglages d'avant les Vignettes simples (ils n'en tiennent plus compte).
pub const MAX_SPACING: f32 = 120.0;

/// Réglages de la disposition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardSettings {
    pub card_w: f32,
    pub columns: usize,
    /// Lignes par page ; 0 : une seule page, aussi longue qu'il faut.
    pub rows: usize,
}

impl Default for BoardSettings {
    fn default() -> Self {
        BoardSettings { card_w: 240.0, columns: 4, rows: 4 }
    }
}

/// Une vignette sur la planche (13a : une par image de storyboard) : le plan (indice, id), la
/// vignette (`id` : celui du plan pour sa première image, sinon celui de la marque d'image), les
/// images du plan qu'elle couvre [`frame`, `end`), sa scène, la carte (ligne du numéro comprise), l'image
/// et la bande de note dessous.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPlace {
    pub panel: usize,
    pub plan: u64,
    pub id: u64,
    pub frame: usize,
    pub end: usize,
    pub scene: u64,
    /// Le numéro du plan (au-dessus) et l'image : le repère des notes attachées de la planche.
    pub card: Rect,
    pub thumb: Rect,
    /// La bande de note sous l'image, de la hauteur de ses lignes affichées.
    pub note: Rect,
    pub row: usize,
    pub col: usize,
}

impl CardPlace {
    /// La première vignette de son plan.
    pub fn starts_plan(&self) -> bool {
        self.frame == 0
    }

    /// La ligne du numéro de plan, au-dessus de l'image.
    pub fn label(&self) -> Rect {
        Rect::from_min_max(self.card.min, pos2(self.card.right(), self.thumb.top()))
    }
}

/// La barre entre la vignette `after` et la suivante (la première case vide après la dernière) : grise
/// quand elles sont du même plan, noire à une coupe. Au bout d'une ligne, elle est après la dernière
/// vignette de la ligne et dit la même chose de la première de la ligne suivante.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutBar {
    pub after: usize,
    pub cut: bool,
    /// Le trait visible (un tiers de la hauteur de la vignette).
    pub bar: Rect,
    /// La zone où l'on clique : tout l'écart, sur la hauteur de la vignette.
    pub zone: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardLayout {
    pub cards: Vec<CardPlace>,
    /// Les barres de coupe : une après chaque vignette, la dernière avant la première case vide.
    pub bars: Vec<CutBar>,
    /// Les pages (fond blanc), et les cases vides (leur image) qui suivent la dernière vignette.
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

/// Range les vignettes sur des pages (voir le début du fichier), avec une ligne de note par vignette.
#[cfg(test)]
pub fn layout(doc: &Document, s: &BoardSettings) -> BoardLayout {
    layout_with(doc, s, &HashMap::new(), false)
}

/// Comme `layout`. `lines` : le nombre de lignes de note affichées (1 à 3) de chaque vignette, par son
/// identifiant (absente : une ligne). `last_joins` : la première case vide, si on la remplit, fait une
/// image de plus du dernier plan (barre grise devant elle) et non un plan neuf (barre noire).
/// Ne dépend pas de la taille de la vue : les notes libres restent à leur place quand la fenêtre change.
pub fn layout_with(doc: &Document, s: &BoardSettings, lines: &HashMap<u64, usize>, last_joins: bool) -> BoardLayout {
    let cols = s.columns.clamp(1, MAX_COLUMNS);
    let card_w = s.card_w.clamp(CARD_MIN, CARD_MAX);
    let thumb_h = card_w * doc.frame[1] as f32 / doc.frame[0].max(1) as f32;
    let mut boards: Vec<(usize, u64, usize, usize)> = Vec::new();
    for i in 0..doc.panels.len() {
        boards.extend(plan_boards(doc, i).into_iter().map(|(id, frame, end)| (i, id, frame, end)));
    }
    let n = boards.len();
    // Au moins une case vide après la dernière vignette ; la dernière page est complétée.
    let mut rows_used = (n + 1).div_ceil(cols);
    let per_page = if s.rows == 0 { rows_used } else { s.rows.min(MAX_ROWS) };
    rows_used = rows_used.div_ceil(per_page) * per_page;
    let pages = rows_used / per_page;
    let note_lines = |slot: usize| boards.get(slot).map_or(1, |b| lines.get(&b.1).copied().unwrap_or(1).clamp(1, NOTE_MAX_LINES));
    // L'écart après la case `slot` : serré dans un plan, large à une coupe ; après une case vide, large.
    let joined_after = |slot: usize| match boards.get(slot + 1) {
        Some(next) => next.2 > 0,
        None => slot + 1 == n && last_joins,
    };
    let gap_after = |slot: usize| if joined_after(slot) { GAP_SAME } else { GAP_CUT };

    // Hauteur de chaque ligne (ses notes les plus hautes), puis haut des lignes et des pages.
    let row_h: Vec<f32> = (0..rows_used)
        .map(|r| {
            let most = (r * cols..(r + 1) * cols).map(note_lines).max().unwrap_or(1);
            LABEL_H + thumb_h + NOTE_GAP + most as f32 * NOTE_LINE_H + ROW_GAP
        })
        .collect();
    let page_h: Vec<f32> = (0..pages).map(|p| PAGE_PAD * 2.0 + row_h[p * per_page..(p + 1) * per_page].iter().sum::<f32>() - ROW_GAP).collect();
    let page_w = PAGE_PAD * 2.0 + cols as f32 * card_w + (cols - 1) as f32 * GAP_CUT;
    let mut page_top = Vec::with_capacity(pages);
    let mut y = MARGIN;
    for h in &page_h {
        page_top.push(y);
        y += h + PAGE_GAP;
    }
    let mut row_top = Vec::with_capacity(rows_used);
    for p in 0..pages {
        let mut y = page_top[p] + PAGE_PAD;
        for r in p * per_page..(p + 1) * per_page {
            row_top.push(y);
            y += row_h[r];
        }
    }

    let mut out = BoardLayout::default();
    let mut x = MARGIN + PAGE_PAD;
    for slot in 0..rows_used * cols {
        let (r, c) = (slot / cols, slot % cols);
        if c == 0 {
            x = MARGIN + PAGE_PAD;
        }
        let card = Rect::from_min_size(pos2(x, row_top[r]), vec2(card_w, LABEL_H + thumb_h));
        let thumb = Rect::from_min_max(pos2(card.left(), card.top() + LABEL_H), card.max);
        if let Some(&(i, id, frame, end)) = boards.get(slot) {
            let note = Rect::from_min_size(pos2(card.left(), thumb.bottom() + NOTE_GAP), vec2(card_w, note_lines(slot) as f32 * NOTE_LINE_H));
            let p = &doc.panels[i];
            out.cards.push(CardPlace { panel: i, plan: p.id, id, frame, end, scene: p.scene, card, thumb, note, row: r, col: c });
        } else {
            out.empty.push(thumb);
        }
        if slot < n {
            let gap = gap_after(slot);
            let zone = Rect::from_min_max(pos2(card.right(), thumb.top()), pos2(card.right() + gap, thumb.bottom()));
            let bar = Rect::from_center_size(pos2(card.right() + gap * 0.5, thumb.center().y), vec2(BAR_W, thumb_h * BAR_FRACTION));
            out.bars.push(CutBar { after: slot, cut: !joined_after(slot), bar, zone });
        }
        x += card_w + gap_after(slot);
    }
    out.pages = (0..pages).map(|p| Rect::from_min_size(pos2(MARGIN, page_top[p]), vec2(page_w, page_h[p]))).collect();
    out.size = vec2(MARGIN * 2.0 + page_w, page_top[pages - 1] + page_h[pages - 1] + MARGIN);
    out
}

/// Ce qu'il y a sous un point de la planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// La barre après la vignette `k` (la dernière : avant la première case vide).
    Bar(usize),
    /// L'image de la vignette `k`.
    Thumb(usize),
    /// La bande de note de la vignette `k`.
    Note(usize),
    /// La case vide `e` (0 : la première, juste après la dernière vignette).
    Empty(usize),
    Nothing,
}

/// Ce qui est sous le point `p` (planche). Les barres d'abord, élargies à `bar_min_w` si leur écart est plus
/// étroit (pour qu'une barre reste facile à toucher quand la planche est petite à l'écran) ; puis les images,
/// les notes, les cases vides.
pub fn hit(layout: &BoardLayout, p: Pos2, bar_min_w: f32) -> Hit {
    if let Some(b) = layout.bars.iter().find(|b| b.zone.expand2(vec2(((bar_min_w - b.zone.width()) * 0.5).max(0.0), 0.0)).contains(p)) {
        return Hit::Bar(b.after);
    }
    if let Some(k) = layout.cards.iter().position(|c| c.thumb.contains(p)) {
        return Hit::Thumb(k);
    }
    if let Some(k) = layout.cards.iter().position(|c| c.note.contains(p)) {
        return Hit::Note(k);
    }
    match layout.empty.iter().position(|e| e.contains(p)) {
        Some(e) => Hit::Empty(e),
        None => Hit::Nothing,
    }
}

// ---------------------------------------------------------------- déplacer un plan (Maj + cadre)

/// Distance d'un point au filet d'un rectangle : dedans, jusqu'au bord le plus proche ; dehors, jusqu'au rectangle.
fn edge_distance(r: Rect, p: Pos2) -> f32 {
    let (dx, dy) = ((r.left() - p.x).max(p.x - r.right()).max(0.0), (r.top() - p.y).max(p.y - r.bottom()).max(0.0));
    if dx > 0.0 || dy > 0.0 {
        (dx * dx + dy * dy).sqrt()
    } else {
        (p.x - r.left()).min(r.right() - p.x).min(p.y - r.top()).min(r.bottom() - p.y)
    }
}

/// La vignette dont le **cadre** (le filet de l'image, à `tol` près de chaque côté) est sous le point `p`
/// (planche), s'il y en a une : la plus proche si deux cadres voisins se touchent. Dedans, la bande ne mange
/// jamais plus d'un quart de la plus petite côte : le centre reste à l'intérieur (la sélection).
pub fn frame_hit(layout: &BoardLayout, p: Pos2, tol: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (k, c) in layout.cards.iter().enumerate() {
        let inner = tol.min(c.thumb.width().min(c.thumb.height()) * 0.25);
        let d = edge_distance(c.thumb, p);
        let limit = if c.thumb.contains(p) { inner } else { tol };
        if d <= limit && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((k, d));
        }
    }
    best.map(|(k, _)| k)
}

/// Où l'on peut poser des plans déplacés : devant un plan (`before`), ou tout à la fin (`None`). Le repère est un
/// trait vertical (`x`, de `top` à `bottom`, en coordonnées de la planche) dans l'écart qui précède la première
/// vignette de ce plan. Jamais à l'intérieur d'un plan : seules les premières vignettes ont un repère.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropSlot {
    pub before: Option<u64>,
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

/// Les endroits où poser un plan : devant chaque plan, et après le dernier.
pub fn drop_slots(layout: &BoardLayout) -> Vec<DropSlot> {
    let mut out = Vec::new();
    for (k, c) in layout.cards.iter().enumerate() {
        if !c.starts_plan() {
            continue;
        }
        let x = match k.checked_sub(1).map(|j| &layout.cards[j]) {
            Some(prev) if prev.row == c.row => (prev.thumb.right() + c.thumb.left()) * 0.5,
            _ => c.thumb.left() - GAP_CUT * 0.5,
        };
        out.push(DropSlot { before: Some(c.plan), x, top: c.thumb.top(), bottom: c.thumb.bottom() });
    }
    if let Some(last) = layout.cards.last() {
        // Après la dernière vignette : dans l'écart avant la première case vide si elle est sur la même ligne.
        let x = match layout.empty.first() {
            Some(e) if (e.top() - last.thumb.top()).abs() < 1.0 => (last.thumb.right() + e.left()) * 0.5,
            _ => last.thumb.right() + GAP_CUT * 0.5,
        };
        out.push(DropSlot { before: None, x, top: last.thumb.top(), bottom: last.thumb.bottom() });
    }
    out
}

/// L'ordre des plans (`order`) change-t-il si l'on pose `moving` devant `before` ? (Le même calcul que
/// `Document::move_plans`, sans le document.) Faux : poser là ne ferait rien.
pub fn drop_changes_order(order: &[u64], moving: &[u64], before: Option<u64>) -> bool {
    let anchor = before.and_then(|b| {
        let i = order.iter().position(|id| *id == b)?;
        order[i..].iter().find(|id| !moving.contains(id)).copied()
    });
    let kept: Vec<u64> = order.iter().copied().filter(|id| !moving.contains(id)).collect();
    let moved: Vec<u64> = order.iter().copied().filter(|id| moving.contains(id)).collect();
    let at = anchor.and_then(|a| kept.iter().position(|id| *id == a)).unwrap_or(kept.len());
    let mut after = kept;
    after.splice(at..at, moved);
    after != order
}

/// L'endroit de pose le plus proche du point `p` (planche) pour les plans `moving`. Rien si cet endroit-là ne
/// changerait rien (devant un plan qu'on déplace, ou devant celui qui le suit déjà) : lâcher près de sa place, c'est
/// la garder, et aucun repère ne se montre.
pub fn nearest_slot(slots: &[DropSlot], order: &[u64], moving: &[u64], p: Pos2) -> Option<DropSlot> {
    let dist = |s: &DropSlot| {
        let y = p.y.clamp(s.top, s.bottom);
        ((p.x - s.x).powi(2) + (p.y - y).powi(2)).sqrt()
    };
    slots
        .iter()
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
        .filter(|s| s.before.is_none_or(|b| !moving.contains(&b)) && drop_changes_order(order, moving, s.before))
        .copied()
}

/// Plans dont une vignette touche le rectangle `r` (sélection au lasso rectangulaire), sans
/// doublon, dans l'ordre du film.
pub fn cards_in(layout: &BoardLayout, r: Rect) -> Vec<usize> {
    let mut plans: Vec<usize> = layout.cards.iter().filter(|c| c.card.intersects(r)).map(|c| c.panel).collect();
    plans.dedup();
    plans
}

/// Taille (en points de la planche) du texte d'une note de `len` caractères : un peu plus petit
/// quand elle s'allonge.
pub fn note_font_size(len: usize) -> f32 {
    match len {
        0..=40 => 11.0,
        41..=90 => 10.0,
        _ => 9.5,
    }
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
