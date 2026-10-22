//! Planche de la vue Vignettes, sans interface : où se placent les vignettes, les barres de coupe
//! entre elles, les cases pâles où dessiner, les bandes de notes, et les notes (traits et textes)
//! posées sur la planche.
//!
//! La planche a ses propres coordonnées (des points d'écran au zoom 1). Les vignettes y sont
//! rangées comme sur des pages : une par image de storyboard (13a : le début de chaque plan, puis
//! chacune de ses marques d'image), de gauche à droite, `colonnes` par ligne (6 au plus). Une ligne ne se casse jamais à
//! un changement de plan : ce qui sépare deux vignettes qui se suivent, c'est l'écart (serré dans un même plan, large à une
//! coupe) et une courte barre verticale (grise : même plan ; noire : coupe).
//!
//! Vignettes simples (Anthony, 2026-10-21) : aucune bande de titre ; seul le numéro du plan, au-dessus
//! de sa première vignette ; sous chaque vignette, une bande de note (de 1 à 3 lignes ; la ligne
//! suivante descend d'autant). Une note libre reste où on l'a posée ; une note attachée à une vignette la suit,
//! à son échelle.
//!
//! **Papier infini et cases pâles** (Anthony, 2026-10-21) : plus de carte blanche, les cases sont posées sur le papier. Après la
//! dernière vignette, des **cases pâles** montrent où dessiner : au moins `pale_rows` rangées de `colonnes` cases en tout (trois
//! d'origine), et toujours **une rangée pâle après la dernière rangée occupée** (dessiner dans la dernière en fait apparaître une
//! autre). Dessiner dans une case pâle y fait le plan ; celles qui la précèdent deviennent des plans vides, pour garder l'ordre
//! de lecture.
//!
//! **L'empreinte d'un mouvement de caméra** (`vignettecam::empreinte`) : la première case d'un plan dont la caméra bouge prend la
//! place de ses cadres, à l'échelle des autres cases, **à leur vraie place** (Anthony, 2026-10-22 : un cadre peut tomber à
//! mi-rangée, entre les lignes) : elle occupe l'enveloppe de ses cadres, rangée par rangée, sans arrondi (`gabarit_de` : pour chaque
//! rangée que l'enveloppe touche, l'intervalle en x qu'elle y couvre). Les cases qui suivent se rangent **dans l'ordre de
//! lecture** dans les places libres, à côté et dessous de l'enveloppe, jamais dessus.
//!
//! **La case A reste en place quand on retouche ses autres cadres, et l'ordre de lecture ne se casse jamais** (Anthony, 2026-10-22) :
//! - **pendant un geste** de caméra, A reste exactement où elle était (`Reserve::pose`, `poser`) : l'enveloppe peut recouvrir un instant des
//!   cases d'avant, rien ne se range sous la main ;
//! - **au lâcher**, tout se range d'après les cadres, dans l'ordre de lecture (gauche à droite, rangée par rangée). La case se pose d'abord
//!   d'après sa seule vignette, sa place sans mouvement : si l'enveloppe ne recouvre rien d'avant (elle grandit vers la droite ou le bas,
//!   même au-delà de la rangée, sur le papier infini ; ou vers le haut ou la gauche sur du vide), **A ne bouge pas** ; si elle recouvrirait
//!   des cases d'avant, la case passe, une fois, à la première place où elle tient tout entière **après la précédente** : à droite sur la même
//!   rangée, sinon sur la suivante. Les cases d'avant ne bougent jamais ;
//! - une enveloppe qui dépasse la page en haut ou à gauche fait glisser toute la planche d'autant (`BoardLayout::decalage`) : la vue compense par
//!   le défilement, rien ne saute à l'écran.
//!//! Les rangées d'une planche dépendent de la hauteur de leurs notes, qui dépend de la place des cases : la place des rangées se
//! déduit par quelques passes (`layout_with`), qui s'arrêtent dès que rien ne change (une seule passe tant que toutes les notes
//! font une ligne).
//!
//! La place réservée peut être **retenue** (`Reserve`) : pendant un geste de caméra, l'étendue de l'enveloppe ne rétrécit
//! jamais, si bien que les autres cases ne tremblent pas quand le cadre va et vient ; au lâcher tout se range d'après les cadres.

use std::collections::HashMap;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::document::Document;
use crate::vector::VPoint;
use crate::vignettecam::{self, Empreinte, Place, Zone};

/// Hauteur de la ligne du numéro de plan, au-dessus de chaque vignette (réservée sur toute la ligne).
pub const LABEL_H: f32 = 20.0;
/// Espace entre une vignette et sa bande de note ; hauteur d'une ligne de note ; lignes affichées au plus.
pub const NOTE_GAP: f32 = 8.0;
pub const NOTE_LINE_H: f32 = 13.0;
pub const NOTE_MAX_LINES: usize = 3;
/// Espace sous la dernière ligne de note, avant la ligne suivante.
pub const ROW_GAP: f32 = 8.0;
/// Écart entre deux vignettes d'un même plan, et à une coupe, **à la largeur de référence** `REF_W` : les proportions de la
/// maquette validée (une vignette de 136, un écart de 10 dans un plan, de 30 à une coupe). Ils suivent la largeur des
/// vignettes (`gap_same`, `gap_cut`).
pub const REF_W: f32 = 240.0;
pub const GAP_SAME: f32 = 18.0;
pub const GAP_CUT: f32 = 53.0;
/// Barre entre deux vignettes : épaisseur (très pâle, dans un plan), épaisseur d'une coupe (noire), et part de la hauteur de la
/// vignette (un tiers).
pub const BAR_W: f32 = 1.5;
pub const BAR_CUT_W: f32 = 2.0;
pub const BAR_FRACTION: f32 = 1.0 / 3.0;
/// Le trait d'une vignette reste visible au-delà de son cadre, sur 12 % de sa largeur de chaque côté (la maquette 07 : une marge
/// de 12 %, que le premier essai avait réduite à 3 %). Elle dépasse l'écart entre deux vignettes (10 / 136 dans un plan, 30 / 136 à une
/// coupe) : la marge d'une case recouvre alors un peu la case voisine, et c'est le trait de la case peinte en dernier qui se voit
/// (l'ordre de la planche : toujours le même, rien ne clignote).
pub const RING_FRACTION: f32 = 0.12;

/// L'écart entre deux vignettes d'un même plan, pour des vignettes de `card_w` de large.
pub fn gap_same(card_w: f32) -> f32 {
    GAP_SAME * card_w / REF_W
}

/// L'écart à une coupe, pour des vignettes de `card_w` de large.
pub fn gap_cut(card_w: f32) -> f32 {
    GAP_CUT * card_w / REF_W
}
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
/// Rangées pâles au moins (réglage des Préférences), d'origine et au plus.
pub const MIN_ROWS_DEFAULT: usize = 3;
pub const MIN_ROWS_MAX: usize = 12;
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
        BoardSettings { card_w: 240.0, columns: 6, rows: 0 }
    }
}

/// Ce que le geste de caméra en cours retient de la disposition : l'étendue de l'enveloppe du plan `plan` ne rétrécit pas
/// tant qu'il dure (la boîte qu'elle a couverte, relative au coin haut-gauche de sa case A).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reserve {
    pub plan: u64,
    pub rect: Rect,
    /// Où est la case A (sa rangée, son x depuis le bord gauche des rangées) quand le geste commence : elle ne bouge pas tant qu'il
    /// dure, quitte à recouvrir les cases d'avant ; au lâcher la planche se range à nouveau, dans l'ordre de lecture.
    pub pose: Option<(usize, f32)>,
}

impl Reserve {
    /// Cette réserve, élargie à `autre` (elle ne rétrécit jamais ; la case A reste là où le geste l'a trouvée).
    pub fn avec(self, autre: Reserve) -> Reserve {
        Reserve { plan: self.plan, rect: self.rect.union(autre.rect), pose: self.pose.or(autre.pose) }
    }

    /// Ce que la case `k` de `layout` couvre, relativement à son image A (la case seule si sa caméra ne bouge pas), et où elle est.
    pub fn depuis(layout: &BoardLayout, k: usize) -> Option<Reserve> {
        let c = layout.cards.get(k)?;
        let rect = match layout.move_of(k) {
            Some(m) => m.tuile.rect.translate(-c.thumb.min.to_vec2()),
            None => Rect::from_min_size(Pos2::ZERO, c.thumb.size()),
        };
        Some(Reserve { plan: c.plan, rect, pose: Some((c.row, c.thumb.left() - layout.origine_x)) })
    }
}

/// Ce qui change la disposition sans changer le projet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    /// Les cases pâles : au moins ce nombre de rangées en tout (plus une rangée après la dernière occupée). Rien : pas de case
    /// pâle (les exports).
    pub pale_rows: Option<usize>,
    /// La place retenue pendant un geste de caméra.
    pub hold: Option<Reserve>,
}

impl Options {
    pub fn pale(rows: usize) -> Options {
        Options { pale_rows: Some(rows.clamp(1, MIN_ROWS_MAX)), hold: None }
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
    /// L'image (le cadre A pour un plan dont la caméra bouge : toujours une case).
    pub thumb: Rect,
    /// La bande de note sous l'image, de la hauteur de ses lignes affichées (sous l'enveloppe quand elle descend sous la case).
    pub note: Rect,
    /// La rangée de l'image (A).
    pub row: usize,
    /// La case d'un plan dont la caméra bouge (deux clés au moins) : son empreinte montre les cadres (`BoardLayout::moves`).
    pub mouvement: bool,
    /// Tout ce que la case couvre : l'image, plus sa tuile quand la caméra bouge.
    pub foot: Rect,
    /// Le bord droit de la place réservée à la case (la suivante commence un écart après).
    pub reach: f32,
    /// L'empreinte du mouvement dans `BoardLayout::moves`.
    pub mv: Option<usize>,
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

/// Un cadre d'un mouvement de caméra sur la planche.
#[derive(Clone, Debug, PartialEq)]
pub struct CadreBoard {
    pub rang: usize,
    /// Sa boîte englobante.
    pub rect: Rect,
    /// Ses quatre coins (tournés si la caméra l'est).
    pub coins: [Pos2; 4],
    pub reduit: bool,
}

/// La tuile de l'empreinte sur la planche : sa place, la part de la feuille qu'elle montre, et l'enveloppe en coordonnées de la
/// feuille (`vignettecam::Tuile`).
#[derive(Clone, Debug, PartialEq)]
pub struct TuileBoard {
    pub rect: Rect,
    pub zone: Zone,
    pub masque: Vec<Pos2>,
}

/// L'empreinte d'un mouvement de caméra, à sa place sur la planche.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveBoard {
    pub cadres: Vec<CadreBoard>,
    /// La zone de dessin : l'enveloppe convexe de la case A et de tous les cadres.
    pub enveloppe: Vec<Pos2>,
    pub tuile: TuileBoard,
}

impl MoveBoard {
    /// Le point `p` (planche) est-il dans la zone de dessin ?
    pub fn contient(&self, p: Pos2) -> bool {
        vignettecam::dans_polygone(&self.enveloppe, p)
    }

    /// Les quatre coins de chaque cadre.
    pub fn quads(&self) -> Vec<[Pos2; 4]> {
        self.cadres.iter().map(|c| c.coins).collect()
    }
}

/// La barre entre la vignette `after` et la suivante (la première case pâle après la dernière) : grise
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
    /// Les barres de coupe : une après chaque vignette, la dernière avant la première case pâle.
    pub bars: Vec<CutBar>,
    /// Les pages (cartes blanches des exports ; l'écran n'en peint plus : le papier est infini), et les cases pâles dans
    /// l'ordre de lecture, la première juste après la dernière vignette.
    pub pages: Vec<Rect>,
    pub empty: Vec<Rect>,
    /// Les empreintes des plans dont la caméra bouge (`CardPlace::mv`).
    pub moves: Vec<MoveBoard>,
    pub size: Vec2,
    /// De combien tout a glissé (vers la droite, vers le bas) parce qu'une enveloppe dépasse la page à gauche ou en haut : la
    /// vue compense (la case A ne bouge pas à l'écran).
    pub decalage: Vec2,
    /// Le bord gauche des rangées (le x des cases est compté depuis lui, `Reserve::pose`).
    pub origine_x: f32,
}

impl BoardLayout {
    pub fn card_of(&self, id: u64) -> Option<&CardPlace> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// Les zones de la case `k` qui montrent son dessin : l'image, ou l'enveloppe de son empreinte (des polygones convexes).
    pub fn zones_of(&self, k: usize) -> Vec<Vec<Pos2>> {
        match self.cards.get(k) {
            Some(c) => match c.mv.and_then(|m| self.moves.get(m)) {
                Some(m) if !m.enveloppe.is_empty() => vec![m.enveloppe.clone()],
                _ => vec![vignettecam::coins_de(c.thumb).to_vec()],
            },
            None => Vec::new(),
        }
    }

    /// L'empreinte de la case `k`, si sa caméra bouge.
    pub fn move_of(&self, k: usize) -> Option<&MoveBoard> {
        self.cards.get(k).and_then(|c| c.mv).and_then(|m| self.moves.get(m))
    }

    /// La case dont l'image ou l'enveloppe contient le point `p`.
    pub fn card_at(&self, p: Pos2) -> Option<usize> {
        (0..self.cards.len()).find(|&k| self.cards[k].thumb.contains(p) || self.move_of(k).is_some_and(|m| m.contient(p)))
    }

    /// La case dont l'empreinte entière (tout ce qu'elle couvre) et une marge de `marge` contiennent le point `p`.
    pub fn card_near(&self, p: Pos2, marge: f32) -> Option<usize> {
        self.card_at(p).or_else(|| self.cards.iter().position(|c| c.foot.expand(marge).contains(p)))
    }
}

/// Images de storyboard du plan `index` : (id de la vignette, première image, fin).
pub fn plan_boards(doc: &Document, index: usize) -> Vec<(u64, usize, usize)> {
    let p = &doc.panels[index];
    let n = p.frame_count();
    let starts: Vec<(u64, usize)> = std::iter::once((p.id, 0)).chain(p.marks.iter().filter(|m| m.frame < n).map(|m| (m.id, m.frame))).collect();
    (0..starts.len()).map(|k| (starts[k].0, starts[k].1, starts.get(k + 1).map_or(n, |s| s.1))).collect()
}

/// Range les vignettes (voir le début du fichier), avec une ligne de note par vignette et sans case pâle (comme les exports).
#[cfg(test)]
pub fn layout(doc: &Document, s: &BoardSettings) -> BoardLayout {
    layout_with(doc, s, &HashMap::new(), false, &Options::default())
}

/// Un intervalle occupé d'une rangée : de `a` à `b`, puis l'écart qui suit, en x depuis le bord gauche des rangées.
type Occupe = (f32, f32, f32);

/// Ce qui empêche de poser quelque chose entre `a` et `b` (l'écart qui suit compris) dans la rangée `row` : la fin de l'intervalle
/// occupé qui le gêne. `deborde` : `a` est le bord gauche d'une enveloppe qui dépasse à gauche de sa case : elle peut mordre sur
/// l'écart de la case d'avant (il n'est là que pour séparer deux vignettes), pas sur la case elle-même.
fn bloque(occ: &[Vec<Occupe>], row: i64, a: f32, b: f32, deborde: bool) -> Option<f32> {
    let eps = 0.5;
    let fin = |iv: &Occupe| if deborde { iv.1 } else { iv.1 + iv.2 };
    occ.get(usize::try_from(row).ok()?)?.iter().filter(|iv| a < fin(iv) - eps && b > iv.0 + eps).map(fin).fold(None, |m, e| Some(m.map_or(e, |m: f32| m.max(e))))
}

/// Une case à poser : ses rangées (par rapport à celle de A) avec leur intervalle en x depuis le bord gauche de A.
#[derive(Clone, Debug)]
struct Gabarit {
    bandes: Vec<(i32, f32, f32)>,
    x_lo: f32,
    x_hi: f32,
    /// Rangées au-dessus de A (celles qui existent : une enveloppe qui dépasse le haut de la page n'en compte pas).
    au_dessus: usize,
    rangees: usize,
    /// La rangée la plus basse que la case touche, par rapport à celle de A.
    b_max: i32,
}

impl Gabarit {
    /// Une vignette seule : une rangée, sa largeur.
    fn unite(card_w: f32) -> Gabarit {
        Gabarit { bandes: vec![(0, 0.0, card_w)], x_lo: 0.0, x_hi: card_w, au_dessus: 0, rangees: 1, b_max: 0 }
    }
}

/// La première place libre, dans l'ordre de lecture, à partir de (`r_a`, `x_a`) : la rangée de A et le bord gauche de A. Une
/// case qui ne tient pas dans le reste de la rangée (ou de la page) passe tout entière plus loin, jamais coupée.
fn trouve(occ: &[Vec<Occupe>], g: &Gabarit, gap: f32, row_w: f32, page_cap: usize, depart: (usize, f32)) -> (usize, f32) {
    let (mut r_a, mut x_a) = depart;
    let paginee = page_cap != usize::MAX && g.rangees > 1 && g.rangees <= page_cap;
    loop {
        if paginee && r_a < g.au_dessus {
            (r_a, x_a) = (g.au_dessus, -g.x_lo);
            continue;
        }
        if x_a + g.x_lo < -1e-3 {
            x_a = -g.x_lo;
        }
        if x_a + g.x_hi > row_w + 1e-3 || (paginee && ((r_a - g.au_dessus) % page_cap) + g.rangees > page_cap) {
            (r_a, x_a) = (r_a + 1, -g.x_lo);
            continue;
        }
        let mut pousse = None;
        for &(b, lo, hi) in &g.bandes {
            if let Some(fin) = bloque(occ, r_a as i64 + b as i64, x_a + lo, x_a + hi + gap, lo < -1e-3) {
                pousse = Some(fin - lo);
                break;
            }
        }
        match pousse {
            Some(nx) => x_a = nx.max(x_a + 1e-3),
            None => return (r_a, x_a),
        }
    }
}

/// Note l'occupation d'une case posée (les rangées qui n'existent pas, au-dessus de la page, ne comptent pas).
fn occupe(occ: &mut Vec<Vec<Occupe>>, g: &Gabarit, gap: f32, r_a: usize, x_a: f32) {
    for &(b, lo, hi) in &g.bandes {
        let Ok(row) = usize::try_from(r_a as i64 + b as i64) else { continue };
        while occ.len() <= row {
            occ.push(Vec::new());
        }
        occ[row].push((x_a + lo, x_a + hi, gap));
    }
}

/// La case `g`, posée en (`r_a`, `x_a`), gênerait-elle ce qui est déjà noté ?
fn gene(occ: &[Vec<Occupe>], g: &Gabarit, gap: f32, r_a: usize, x_a: f32) -> bool {
    g.bandes.iter().any(|&(b, lo, hi)| bloque(occ, r_a as i64 + b as i64, x_a + lo, x_a + hi + gap, lo < -1e-3).is_some())
}

/// Ce que prend sur la planche une case dont l'enveloppe est `poly` (relative au coin haut-gauche de A) : pour chaque rangée
/// qu'elle touche, l'intervalle en x qu'elle y couvre. `rh(r)` : la hauteur de la rangée `r` (estimée), `r0` la rangée de A. `note` :
/// la bande de note de A quand elle descend sous l'enveloppe. Les rangées se suivent sans trou (l'écart sous une rangée est à elle).
fn gabarit_de(poly: &[Pos2], note: Option<Rect>, rh: &dyn Fn(i64) -> f32, r0: usize) -> Gabarit {
    let boite = vignettecam::boite(poly);
    let (y_min, y_max) = (boite.top(), note.map_or(boite.bottom(), |n| boite.bottom().max(n.bottom())));
    let etendue = |y0: f32, y1: f32| -> Option<(f32, f32)> {
        let mut lo_hi: Option<(f32, f32)> = None;
        let big = 1e6;
        let morceau = crate::viewbar::clip_to_rect(poly, Rect::from_min_max(pos2(-big, y0), pos2(big, y1)));
        if morceau.len() >= 3 {
            let b = vignettecam::boite(&morceau);
            if b.height() > 0.25 {
                lo_hi = Some((b.left(), b.right()));
            }
        }
        if let Some(n) = note.filter(|n| n.bottom().min(y1) - n.top().max(y0) > 0.25) {
            lo_hi = Some(lo_hi.map_or((n.left(), n.right()), |(lo, hi)| (lo.min(n.left()), hi.max(n.right()))));
        }
        lo_hi
    };
    let mut bandes: Vec<(i32, f32, f32)> = Vec::new();
    // Vers le bas : la rangée de A commence à -LABEL_H (sa ligne de numéro) au-dessus de l'image.
    let (mut b, mut haut) = (0i32, -LABEL_H);
    while b < 24 {
        let bas = haut + rh(r0 as i64 + b as i64);
        if b > 0 && haut >= y_max - 0.25 {
            break;
        }
        if let Some((lo, hi)) = etendue(haut, bas).filter(|_| r0 as i64 + b as i64 >= 0) {
            bandes.push((b, lo, hi));
        }
        haut = bas;
        b += 1;
    }
    // Vers le haut.
    let (mut b, mut bas) = (-1i32, -LABEL_H);
    while b > -24 {
        let haut = bas - rh(r0 as i64 + b as i64);
        if bas <= y_min + 0.25 {
            break;
        }
        if r0 as i64 + b as i64 >= 0 {
            if let Some((lo, hi)) = etendue(haut, bas) {
                bandes.push((b, lo, hi));
            }
        }
        bas = haut;
        b -= 1;
    }
    bandes.sort_by_key(|b| b.0);
    let b_min = bandes.iter().map(|b| b.0).min().unwrap_or(0).min(0);
    let b_max = bandes.iter().map(|b| b.0).max().unwrap_or(0).max(0);
    let x_lo = bandes.iter().map(|b| b.1).fold(0.0, f32::min);
    let x_hi = bandes.iter().map(|b| b.2).fold(0.0, f32::max);
    Gabarit { bandes, x_lo, x_hi, au_dessus: (-b_min) as usize, rangees: (b_max - b_min + 1) as usize, b_max }
}

/// Où chaque case se pose (voir le début du fichier). `prints[k]` : la case `k` a une empreinte (son gabarit complet est dans
/// `gabarits[k]`) ; `unite` : celui d'une vignette seule.
struct Pose {
    placed: Vec<(usize, f32)>,
    occ: Vec<Vec<Occupe>>,
    cursor: (usize, f32),
}

fn poser(gabarits: &[Gabarit], unite: &Gabarit, prints: &[bool], forces: &[Option<(usize, f32)>], gap_after: &dyn Fn(usize) -> f32, row_w: f32, page_cap: usize) -> Pose {
    let mut occ: Vec<Vec<Occupe>> = Vec::new();
    let mut placed: Vec<(usize, f32)> = Vec::with_capacity(gabarits.len());
    let mut cursor = (0usize, 0.0_f32);
    for (k, g) in gabarits.iter().enumerate() {
        let gap = gap_after(k);
        let (r, x) = if let Some(pose) = forces[k] {
            // Le geste de caméra de ce plan est en cours : sa case A reste exactement où elle était, quitte à recouvrir les cases
            // d'avant jusqu'au lâcher.
            pose
        } else if prints[k] {
            // Une case qui a une empreinte se pose d'abord d'après sa seule vignette (sa place ne dépend pas de ses cadres tant que
            // l'enveloppe ne gêne rien : elle peut déborder à droite et en bas sur le papier infini, ou en haut et à gauche sur du
            // vide) ; si l'enveloppe recouvrirait des cases d'avant, la case passe à la première place où elle tient tout entière,
            // après la précédente : l'ordre de lecture ne se casse jamais (Anthony, 2026-10-22).
            let (r, x) = trouve(&occ, unite, gap, row_w, page_cap, cursor);
            if gene(&occ, g, gap, r, x) {
                trouve(&occ, g, gap, row_w, page_cap, (cursor.0, cursor.1 - g.x_lo))
            } else {
                (r, x)
            }
        } else {
            trouve(&occ, g, gap, row_w, page_cap, (cursor.0, cursor.1 - g.x_lo))
        };
        occupe(&mut occ, g, gap, r, x);
        placed.push((r, x));
        cursor = (r, x + g.x_hi + gap);
    }
    Pose { placed, occ, cursor }
}
/// Comme `layout`. `lines` : le nombre de lignes de note affichées (1 à 3) de chaque vignette, par son
/// identifiant (absente : une ligne). `last_joins` : la première case pâle, si on la remplit, fait une
/// image de plus du dernier plan (barre grise devant elle) et non un plan neuf (barre noire).
/// Ne dépend pas de la taille de la vue : les notes libres restent à leur place quand la fenêtre change.
pub fn layout_with(doc: &Document, s: &BoardSettings, lines: &HashMap<u64, usize>, last_joins: bool, opt: &Options) -> BoardLayout {
    let cols = s.columns.clamp(1, MAX_COLUMNS);
    let card_w = s.card_w.clamp(CARD_MIN, CARD_MAX);
    let thumb_h = card_w * doc.frame[1] as f32 / doc.frame[0].max(1) as f32;
    let mut boards: Vec<(usize, u64, usize, usize)> = Vec::new();
    for i in 0..doc.panels.len() {
        boards.extend(plan_boards(doc, i).into_iter().map(|(id, frame, end)| (i, id, frame, end)));
    }
    let n = boards.len();
    let page_cap = if s.rows == 0 { usize::MAX } else { s.rows.min(MAX_ROWS).max(1) };
    let note_lines = |k: usize| lines.get(&boards[k].1).copied().unwrap_or(1).clamp(1, NOTE_MAX_LINES);
    // L'écart après la vignette `k` : serré dans un plan, large à une coupe ; après une case pâle, large.
    let joined_after = |k: usize| match boards.get(k + 1) {
        Some(next) => next.2 > 0,
        None => last_joins,
    };
    let (g_same, g_cut) = (gap_same(card_w), gap_cut(card_w));
    let gap_after = |k: usize| if joined_after(k) { g_same } else { g_cut };
    let row_w = cols as f32 * card_w + (cols - 1) as f32 * g_cut;

    // Les empreintes (la première image d'un plan dont la caméra bouge).
    let mut prints: Vec<Option<Empreinte>> = Vec::with_capacity(n);
    for (k, b) in boards.iter().enumerate() {
        let total = NOTE_GAP + note_lines(k) as f32 * NOTE_LINE_H;
        let place = Place { card_w, thumb_h, row_w, total, max_h: 3.0 * (LABEL_H + thumb_h + total + ROW_GAP) };
        prints.push(if b.2 == 0 { vignettecam::empreinte(doc, b.0, &place) } else { None });
    }
    // Ce qu'elles réservent : l'enveloppe (élargie à ce que le geste en cours a déjà couvert), et la bande de note de A quand elle
    // descend sous l'enveloppe.
    let polys: Vec<Option<Vec<Pos2>>> = prints
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let e = e.as_ref()?;
            let hold = opt.hold.filter(|h| h.plan == doc.panels[boards[k].0].id);
            Some(match hold {
                None => e.enveloppe.clone(),
                Some(h) => {
                    let mut pts = e.enveloppe.clone();
                    pts.extend(vignettecam::coins_de(h.rect));
                    let tenue = vignettecam::enveloppe_convexe(&pts);
                    // (Jamais plus large qu'une rangée : sinon la case ne tiendrait nulle part.)
                    if vignettecam::boite(&tenue).width() > row_w { e.enveloppe.clone() } else { tenue }
                }
            })
        })
        .collect();
    let notes: Vec<Option<Rect>> = prints
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let e = e.as_ref()?;
            (e.bas_a > thumb_h + 1e-3).then(|| Rect::from_min_size(pos2(0.0, e.bas_a + NOTE_GAP), vec2(card_w, note_lines(k) as f32 * NOTE_LINE_H)))
        })
        .collect();
    let a_print: Vec<bool> = prints.iter().map(Option::is_some).collect();
    // Le geste de caméra en cours : la case A du plan reste où elle était (`Reserve::pose`).
    let forces: Vec<Option<(usize, f32)>> = (0..n).map(|k| opt.hold.filter(|h| h.plan == doc.panels[boards[k].0].id && boards[k].2 == 0).and_then(|h| h.pose)).collect();
    let unite = Gabarit::unite(card_w);
    let nominal = LABEL_H + thumb_h + NOTE_GAP + NOTE_LINE_H + ROW_GAP;
    let hauteur_rangee = |lignes: usize| LABEL_H + thumb_h + NOTE_GAP + lignes as f32 * NOTE_LINE_H + ROW_GAP;

    // Où chaque case se pose. Les rangées touchées par une enveloppe dépendent de la hauteur des rangées, qui dépend des notes des
    // cases posées : on repart des hauteurs de la passe d'avant (nominales d'abord) jusqu'à ce que rien ne change.
    let mut estimees: Vec<f32> = Vec::new();
    let mut rangs: Vec<usize> = vec![0; n];
    let mut pose = Pose { placed: Vec::new(), occ: Vec::new(), cursor: (0, 0.0) };
    let mut gabarits: Vec<Gabarit> = Vec::new();
    let mut most: Vec<usize> = Vec::new();
    for passe in 0..5 {
        let hauteur_estimee = |r: i64| usize::try_from(r).ok().and_then(|r| estimees.get(r).copied()).unwrap_or(nominal);
        gabarits = (0..n).map(|k| polys[k].as_ref().map_or_else(|| unite.clone(), |poly| gabarit_de(poly, notes[k], &hauteur_estimee, rangs[k]))).collect();
        pose = poser(&gabarits, &unite, &a_print, &forces, &gap_after, row_w, page_cap);
        // Les notes les plus hautes de chaque rangée (celles des cases dont l'image est dans la rangée).
        let derniere = pose.placed.iter().zip(&gabarits).map(|(p, g)| p.0 + g.b_max as usize).max().unwrap_or(0);
        most = vec![1usize; derniere + 1];
        for (k, p) in pose.placed.iter().enumerate() {
            most[p.0] = most[p.0].max(note_lines(k));
        }
        if !a_print.iter().any(|p| *p) {
            break;
        }
        let reelles: Vec<f32> = most.iter().map(|&l| hauteur_rangee(l)).collect();
        let egal = |a: &[f32], b: &[f32]| (0..a.len().max(b.len())).all(|r| (a.get(r).copied().unwrap_or(nominal) - b.get(r).copied().unwrap_or(nominal)).abs() < 1e-3);
        let stable = egal(&reelles, &estimees) && rangs == pose.placed.iter().map(|p| p.0).collect::<Vec<_>>();
        estimees = reelles;
        rangs = pose.placed.iter().map(|p| p.0).collect();
        if stable {
            break;
        }
        let _ = passe;
    }
    let (placed, occ, cursor) = (pose.placed, pose.occ, pose.cursor);
    let bottom = placed.iter().zip(&gabarits).map(|(p, g)| p.0 + g.b_max as usize + 1).max().unwrap_or(0);

    // Les cases pâles : ce qui reste de la rangée courante et les rangées suivantes, au moins `pale_rows` en tout et une rangée
    // après la dernière occupée.
    let mut occ = occ;
    let mut pale: Vec<(usize, f32)> = Vec::new();
    let rows_n = match opt.pale_rows {
        Some(min_rows) => {
            let total = min_rows.max(bottom + 1).max(cursor.0 + 1);
            let mut cur = cursor;
            loop {
                let (r, x) = trouve(&occ, &unite, g_cut, row_w, usize::MAX, cur);
                if r >= total {
                    break;
                }
                occupe(&mut occ, &unite, g_cut, r, x);
                pale.push((r, x));
                cur = (r, x + card_w + g_cut);
            }
            total
        }
        None => bottom.max(1),
    };
    let per_page = if page_cap == usize::MAX { rows_n } else { page_cap };
    let pages = rows_n.div_ceil(per_page).max(1);

    // Hauteur de chaque ligne (ses notes les plus hautes : celles des cases dont l'image est dans la ligne), puis haut des
    // lignes et des pages.
    let row_h: Vec<f32> = (0..rows_n).map(|r| hauteur_rangee(most.get(r).copied().unwrap_or(1))).collect();
    let rows_of = |p: usize| p * per_page..((p + 1) * per_page).min(rows_n);
    let page_h: Vec<f32> = (0..pages).map(|p| PAGE_PAD * 2.0 + row_h[rows_of(p)].iter().sum::<f32>() - ROW_GAP).collect();
    let page_w = PAGE_PAD * 2.0 + row_w;
    let mut page_top = Vec::with_capacity(pages);
    let mut y = MARGIN;
    for h in &page_h {
        page_top.push(y);
        y += h + PAGE_GAP;
    }
    let mut row_top = Vec::with_capacity(rows_n);
    for p in 0..pages {
        let mut y = page_top[p] + PAGE_PAD;
        for r in rows_of(p) {
            row_top.push(y);
            y += row_h[r];
        }
    }

    // Une enveloppe qui dépasse la page à gauche ou en haut : tout glisse d'autant (la vue compense, la case A ne bouge pas à l'écran).
    let mut left = MARGIN + PAGE_PAD;
    let mut decalage = Vec2::ZERO;
    for (k, &(r, x_a)) in placed.iter().enumerate() {
        if let Some(e) = &prints[k] {
            let (gauche, haut) = (left + x_a + e.x_min, row_top[r] + LABEL_H + e.y_min);
            decalage.x = decalage.x.max(MARGIN - gauche);
            decalage.y = decalage.y.max(MARGIN - haut);
        }
    }
    left += decalage.x;
    for t in row_top.iter_mut().chain(page_top.iter_mut()) {
        *t += decalage.y;
    }

    let mut out = BoardLayout { decalage, origine_x: left, ..Default::default() };
    for (k, &(r, x_a)) in placed.iter().enumerate() {
        let x = left + x_a;
        let top = row_top[r] + LABEL_H;
        let bottom = top + thumb_h;
        let thumb = Rect::from_min_size(pos2(x, top), vec2(card_w, thumb_h));
        let (i, id, frame, end) = boards[k];
        let p = &doc.panels[i];
        let mut foot = thumb;
        let mut mv = None;
        let mut note_top = bottom + NOTE_GAP;
        if let Some(e) = &prints[k] {
            let au = vec2(x, top);
            let cadres: Vec<CadreBoard> = e.cadres.iter().map(|c| CadreBoard { rang: c.rang, rect: c.rect.translate(au), coins: c.coins.map(|q| q + au), reduit: c.reduit }).collect();
            let tuile = TuileBoard { rect: e.tuile.rect.translate(au), zone: e.tuile.zone, masque: e.tuile.masque.clone() };
            foot = foot.union(tuile.rect);
            note_top = top + e.bas_a.max(thumb_h) + NOTE_GAP;
            mv = Some(out.moves.len());
            out.moves.push(MoveBoard { cadres, enveloppe: e.enveloppe.iter().map(|q| *q + au).collect(), tuile });
        }
        let card = Rect::from_min_max(pos2(x, row_top[r]), pos2(x + card_w, bottom));
        let note = Rect::from_min_size(pos2(x, note_top), vec2(card_w, note_lines(k) as f32 * NOTE_LINE_H));
        let reach = left + x_a + gabarits[k].x_hi;
        out.cards.push(CardPlace { panel: i, plan: p.id, id, frame, end, scene: p.scene, card, thumb, note, row: r, mouvement: mv.is_some(), foot, reach, mv });
        let gap = gap_after(k);
        let zone = Rect::from_min_max(pos2(reach, thumb.top()), pos2(reach + gap, thumb.bottom()));
        let cut = !joined_after(k);
        let bar = Rect::from_center_size(pos2(reach + gap * 0.5, thumb.center().y), vec2(if cut { BAR_CUT_W } else { BAR_W }, thumb_h * BAR_FRACTION));
        out.bars.push(CutBar { after: k, cut, bar, zone });
    }
    for &(r, x_a) in &pale {
        out.empty.push(Rect::from_min_size(pos2(left + x_a, row_top[r] + LABEL_H), vec2(card_w, thumb_h)));
    }
    out.pages = (0..pages).map(|p| Rect::from_min_size(pos2(MARGIN, page_top[p]), vec2(page_w + decalage.x, page_h[p]))).collect();
    // (Une enveloppe qui dépasse à droite ou en bas : la page et la planche s'agrandissent.)
    let mut size = vec2(MARGIN * 2.0 + page_w + decalage.x, page_top[pages - 1] + page_h[pages - 1] + MARGIN);
    for c in out.cards.iter().filter(|c| c.mouvement) {
        size = size.max(vec2(c.foot.right() + MARGIN, c.foot.bottom() + MARGIN));
        for pg in out.pages.iter_mut().filter(|pg| pg.intersects(c.foot)) {
            *pg = pg.union(c.foot.expand2(vec2(PAGE_PAD, PAGE_PAD)).intersect(Rect::from_min_max(pos2(MARGIN, MARGIN), pos2(f32::MAX, f32::MAX))));
        }
    }
    out.size = size;
    out
}

/// Ce qu'il y a sous un point de la planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// La barre après la vignette `k` (la dernière : avant la première case pâle).
    Bar(usize),
    /// L'image de la vignette `k` (ou une tuile de son empreinte).
    Thumb(usize),
    /// La bande de note de la vignette `k`.
    Note(usize),
    /// La case pâle `e` (0 : la première, juste après la dernière vignette).
    Empty(usize),
    Nothing,
}

/// Ce qui est sous le point `p` (planche). Les barres d'abord, élargies à `bar_min_w` si leur écart est plus
/// étroit (pour qu'une barre reste facile à toucher quand la planche est petite à l'écran) ; puis les images,
/// les notes, les cases pâles.
pub fn hit(layout: &BoardLayout, p: Pos2, bar_min_w: f32) -> Hit {
    if let Some(b) = layout.bars.iter().find(|b| b.zone.expand2(vec2(((bar_min_w - b.zone.width()) * 0.5).max(0.0), 0.0)).contains(p)) {
        return Hit::Bar(b.after);
    }
    if let Some(k) = layout.card_at(p) {
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
        let mut consider = |d: f32, inside: bool, small: f32| {
            let limit = if inside { tol.min(small * 0.25) } else { tol };
            if d <= limit && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((k, d));
            }
        };
        match layout.move_of(k) {
            Some(m) if !m.enveloppe.is_empty() => {
                let b = vignettecam::boite(&m.enveloppe);
                consider(vignettecam::distance_bord(&m.enveloppe, p), m.contient(p), b.width().min(b.height()));
            }
            _ => consider(edge_distance(c.thumb, p), c.thumb.contains(p), c.thumb.width().min(c.thumb.height())),
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
            Some(prev) if prev.row == c.row => (prev.reach + c.thumb.left()) * 0.5,
            _ => c.thumb.left() - PAGE_PAD * 0.5,
        };
        out.push(DropSlot { before: Some(c.plan), x, top: c.thumb.top(), bottom: c.thumb.bottom() });
    }
    if let Some(last) = layout.cards.last() {
        // Après la dernière vignette : dans l'écart avant la première case pâle si elle est sur la même ligne.
        let x = match layout.empty.first() {
            Some(e) if (e.top() - last.thumb.top()).abs() < 1.0 => (last.reach + e.left()) * 0.5,
            _ => last.reach + PAGE_PAD * 0.5,
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
    let mut plans: Vec<usize> = layout.cards.iter().filter(|c| c.card.intersects(r) || c.foot.intersects(r)).map(|c| c.panel).collect();
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
