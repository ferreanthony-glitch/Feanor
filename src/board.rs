//! Planche de la vue Vignettes (phase 7), sans interface : où se placent les vignettes, où
//! tombent les panneaux qu'on lâche, et les notes libres dessinées sur le fond.
//!
//! La planche a ses propres coordonnées (des points d'écran au zoom 1) : les vignettes y sont
//! rangées par scène, en lignes qui reviennent à la ligne à la largeur de la vue ; les notes y
//! restent où on les a dessinées, même quand les vignettes bougent (choix d'Anthony).

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::document::Document;
use crate::vector::VPoint;

/// Bande du titre (« Sc 1 · P 2 », durée) au-dessus de chaque vignette.
pub const HEADER_H: f32 = 18.0;
/// Écart entre deux vignettes.
pub const GAP: f32 = 18.0;
/// Titre d'une scène, au-dessus de ses vignettes.
pub const SCENE_H: f32 = 30.0;
pub const MARGIN: f32 = 28.0;
/// Bornes de la largeur d'une vignette.
pub const CARD_MIN: f32 = 100.0;
pub const CARD_MAX: f32 = 700.0;

/// Une vignette sur la planche : la carte entière (titre compris) et l'image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPlace {
    pub panel: usize,
    pub card: Rect,
    pub thumb: Rect,
}

/// Une scène sur la planche : ses panneaux [`start`, `end`) et l'endroit de son titre.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRow {
    pub scene: u64,
    pub start: usize,
    pub end: usize,
    pub title: Pos2,
    /// Ce que couvrent le titre et les vignettes de la scène.
    pub area: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardLayout {
    pub cards: Vec<CardPlace>,
    pub scenes: Vec<SceneRow>,
    pub size: Vec2,
}

/// Range les vignettes : une scène après l'autre, de gauche à droite, à la ligne quand la largeur
/// `wrap` est atteinte (au moins une vignette par ligne).
pub fn layout(doc: &Document, card_w: f32, wrap: f32) -> BoardLayout {
    let card_w = card_w.clamp(CARD_MIN, CARD_MAX);
    let thumb_h = card_w * doc.frame[1] as f32 / doc.frame[0].max(1) as f32;
    let card_h = HEADER_H + thumb_h;
    let mut out = BoardLayout::default();
    let mut y = MARGIN;
    let mut right: f32 = 0.0;
    let mut start = 0;
    while start < doc.panels.len() {
        let (s, e) = doc.scene_run(start);
        let title = pos2(MARGIN, y);
        y += SCENE_H;
        let mut x = MARGIN;
        for i in s..e {
            if x > MARGIN && x + card_w > wrap - MARGIN {
                x = MARGIN;
                y += card_h + GAP;
            }
            let card = Rect::from_min_size(pos2(x, y), vec2(card_w, card_h));
            let thumb = Rect::from_min_max(pos2(card.left(), card.top() + HEADER_H), card.max);
            out.cards.push(CardPlace { panel: i, card, thumb });
            right = right.max(card.right());
            x += card_w + GAP;
        }
        let bottom = y + card_h;
        out.scenes.push(SceneRow {
            scene: doc.panels[s].scene,
            start: s,
            end: e,
            title,
            area: Rect::from_min_max(title, pos2(right.max(title.x + card_w), bottom)),
        });
        y = bottom + GAP * 2.0;
        start = e;
    }
    out.size = vec2(right + MARGIN, y - GAP * 2.0 + MARGIN);
    out
}

/// Où vont des panneaux lâchés : dans la scène `scene`, devant le panneau `before` (indice dans
/// le film), ou à la fin de la scène. `line` : où montrer le repère (un trait vertical).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropTarget {
    pub scene: u64,
    pub before: Option<usize>,
    pub line: (Pos2, Pos2),
}

/// La place visée par le point `p` de la planche : la scène dont la zone est la plus proche en
/// hauteur, puis la ligne de vignettes la plus proche, puis entre deux vignettes de cette ligne.
pub fn drop_target(layout: &BoardLayout, p: Pos2) -> Option<DropTarget> {
    let dist = |r: &Rect| if p.y < r.top() { r.top() - p.y } else { (p.y - r.bottom()).max(0.0) };
    let row = layout.scenes.iter().min_by(|a, b| dist(&a.area).total_cmp(&dist(&b.area)))?;
    let cards: Vec<&CardPlace> = layout.cards.iter().filter(|c| (row.start..row.end).contains(&c.panel)).collect();
    let first = cards.first()?;
    // Ligne de vignettes (même haut) la plus proche du point.
    let line_top = cards
        .iter()
        .map(|c| c.card.top())
        .min_by(|a, b| (a + first.card.height() * 0.5 - p.y).abs().total_cmp(&(b + first.card.height() * 0.5 - p.y).abs()))?;
    let line: Vec<&&CardPlace> = cards.iter().filter(|c| c.card.top() == line_top).collect();
    let (top, bottom) = (line_top, line_top + first.card.height());
    let x_line = |x: f32| (pos2(x, top - 4.0), pos2(x, bottom + 4.0));
    match line.iter().find(|c| p.x < c.card.center().x) {
        Some(c) => Some(DropTarget { scene: row.scene, before: Some(c.panel), line: x_line(c.card.left() - GAP * 0.5) }),
        None => {
            let last = line.last()?;
            // Après la dernière vignette de la ligne : devant la première de la ligne suivante.
            let before = (last.panel + 1 < row.end).then_some(last.panel + 1);
            Some(DropTarget { scene: row.scene, before, line: x_line(last.card.right() + GAP * 0.5) })
        }
    }
}

/// Vignettes que touche le rectangle `r` (sélection au lasso rectangulaire).
pub fn cards_in(layout: &BoardLayout, r: Rect) -> Vec<usize> {
    layout.cards.iter().filter(|c| c.card.intersects(r)).map(|c| c.panel).collect()
}

/// Un trait de note sur la planche.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteStroke {
    pub points: Vec<VPoint>,
    pub color: [u8; 3],
}

/// Ce que couvre un trait (épaisseur comprise).
pub fn points_bounds(points: &[VPoint]) -> Rect {
    points.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0))))
}

impl NoteStroke {
    #[cfg(test)]
    pub fn bounds(&self) -> Rect {
        points_bounds(&self.points)
    }

    /// Le trait passe-t-il à moins de `radius` du point `p` ?
    pub fn touches(&self, p: Pos2, radius: f32) -> bool {
        match self.points.as_slice() {
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
}

/// Les notes de la planche (phase 7) : des traits libres, enregistrés avec le projet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub strokes: Vec<NoteStroke>,
}

impl Board {
    /// Gomme : retire les traits que touche le cercle (`p`, `radius`). Vrai si un trait est parti.
    pub fn erase_at(&mut self, p: Pos2, radius: f32) -> bool {
        let before = self.strokes.len();
        self.strokes.retain(|s| !s.touches(p, radius));
        self.strokes.len() != before
    }
}

/// Ajoute le point `p` à un trait en cours, sauf s'il est trop près du précédent (`min_step`).
pub fn push_note_point(points: &mut Vec<VPoint>, p: VPoint, min_step: f32) {
    match points.last_mut() {
        Some(last) if last.pos().distance(p.pos()) < min_step => last.r = last.r.max(p.r),
        _ => points.push(p),
    }
}
