//! Outil Cutter (Anthony, 2026-10-17 : « ajoute l'outil cutter qui coupe les lignes vectorielles
//! qui se croisent »), raccourci Alt+T.
//!
//! Sur un calque vectoriel, on glisse en travers des bouts de traits à enlever (ou on clique sur
//! l'un d'eux) : chaque trait touché est coupé là où il croise les autres traits (et lui-même),
//! et seuls les morceaux que le geste touche disparaissent. Un trait qui ne croise rien part en
//! entier. Les zones remplies et les textes ne bougent pas.
//!
//! **Partie 169** (2026-11-18, Anthony : « On peut tout mettre ? ») : cinq réglages, retenus avec les réglages du poste
//! (`Settings::cutter`, jamais dans le projet) :
//! - **Écart toléré** : un trait qui s'arrête à moins de N pixels du dessin d'un autre trait compte comme s'il le croisait
//!   (le bout prolongé jusqu'à l'autre trait ; l'autre trait est coupé au point d'approche). 0 : comme avant.
//! - **Bouts effilés** : le bout laissé par une coupe s'affine sur une longueur réglable. 0 : net, comme avant.
//! - **Mode** : « Couper » (comme avant) ou « Nettoyer les dépassements » : dans la zone que le geste balaie (ce qu'il
//!   touche ou entoure), les bouts d'extrémité (entre le bout libre d'un trait et son dernier croisement) plus courts qu'une
//!   longueur choisie partent ; un morceau entre deux croisements ne part jamais.
//! - **Portée** : les lignes de coupe sont les traits du calque actif (comme avant) ou ceux de tous les calques vectoriels
//!   visibles (qui ne sont pas modifiés). Plusieurs calques ou clés choisis : le geste agit sur tous les dessins choisis, en une
//!   seule étape d'annulation (`cutter_multi`).
//! - **Geste** : « Glisser en travers » (comme avant) ou « Lasso » : on entoure, et les morceaux entièrement dedans partent ;
//!   en mode Dépassements, le lasso délimite la zone.
//!
//! Sans latence : les boîtes des traits sont calculées une fois par geste, et seuls les traits dont la boîte rencontre celle
//! d'un autre sont comparés (`cutter_tests::un_gros_dessin_se_coupe_vite`).

use eframe::egui::{Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::vector::{self, Affine, Element, VPoint, VectorDrawing};

/// Les morceaux plus courts (pixels du dessin) sont laissés de côté.
const MIN_PIECE: f32 = 0.5;
/// Le rayon au bout d'un effilé, en part du rayon du trait.
const POINTE: f32 = 0.12;

// ------------------------------------------------------------------ les réglages

/// Couper, ou nettoyer les dépassements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    #[serde(rename = "couper")]
    Couper,
    #[serde(rename = "depassements")]
    Depassements,
}

/// Contre quels traits on coupe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Portee {
    #[default]
    #[serde(rename = "calque")]
    Calque,
    #[serde(rename = "calques_visibles")]
    Visibles,
}

/// La forme du geste.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Geste {
    #[default]
    #[serde(rename = "travers")]
    Travers,
    #[serde(rename = "lasso")]
    Lasso,
}

/// Les réglages du Cutter (partie 169), réglages du poste.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglages {
    /// Écart toléré, en pixels du dessin (0 : seuls les vrais croisements comptent).
    #[serde(rename = "ecart_tolere")]
    pub ecart: f32,
    /// Longueur des bouts effilés, en pixels du dessin (0 : net).
    #[serde(rename = "bouts_effiles")]
    pub effile: f32,
    pub mode: Mode,
    /// Mode Dépassements : les bouts plus courts que tant de pixels du dessin partent.
    #[serde(rename = "depassement_max")]
    pub depassement: f32,
    pub portee: Portee,
    pub geste: Geste,
}

pub const ECART_MAX: f32 = 30.0;
pub const EFFILE_MAX: f32 = 80.0;
pub const DEPASSEMENT_MIN: f32 = 1.0;
pub const DEPASSEMENT_MAX: f32 = 100.0;

impl Default for Reglages {
    fn default() -> Self {
        Reglages { ecart: 0.0, effile: 0.0, mode: Mode::Couper, depassement: 12.0, portee: Portee::Calque, geste: Geste::Travers }
    }
}

impl Reglages {
    pub fn est_defaut(&self) -> bool {
        *self == Reglages::default()
    }

    /// Les valeurs ramenées dans leurs bornes (un fichier écrit à la main).
    pub fn nettoyes(self) -> Reglages {
        let borne = |v: f32, a: f32, b: f32, d: f32| if v.is_finite() { v.clamp(a, b) } else { d };
        let d = Reglages::default();
        Reglages {
            ecart: borne(self.ecart, 0.0, ECART_MAX, d.ecart),
            effile: borne(self.effile, 0.0, EFFILE_MAX, d.effile),
            depassement: borne(self.depassement, DEPASSEMENT_MIN, DEPASSEMENT_MAX, d.depassement),
            ..self
        }
    }
}

// ------------------------------------------------------------------ la géométrie

/// Croisement des segments `a`–`b` et `c`–`d` : fractions le long de chacun (0 à 1).
fn seg_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<(f32, f32)> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let q = c - a;
    let t = (q.x * s.y - q.y * s.x) / den;
    let u = (q.x * r.y - q.y * r.x) / den;
    const E: f32 = 1e-4;
    ((-E..=1.0 + E).contains(&t) && (-E..=1.0 + E).contains(&u)).then_some((t.clamp(0.0, 1.0), u.clamp(0.0, 1.0)))
}

/// Endroits (indice du segment + fraction) où la ligne `p` croise la ligne `q` ; `same` : c'est
/// la même ligne (ses segments voisins ne comptent pas).
pub fn crossings(p: &[Pos2], q: &[Pos2], same: bool) -> Vec<f32> {
    crossings_dans(p, q, Rect::EVERYTHING, same)
}

/// `crossings`, la boîte de `q` étant `cadre_q` : un segment de `p` hors d'elle ne parcourt pas `q` (sans latence sur un gros
/// dessin, où chaque trait long rencontre la boîte de beaucoup d'autres).
fn crossings_dans(p: &[Pos2], q: &[Pos2], cadre_q: Rect, same: bool) -> Vec<f32> {
    let mut out = Vec::new();
    for i in 0..p.len().saturating_sub(1) {
        let (a, b) = (p[i], p[i + 1]);
        let ab = Rect::from_two_pos(a, b);
        if !ab.intersects(cadre_q) {
            continue;
        }
        for j in 0..q.len().saturating_sub(1) {
            if same && j + 1 >= i && j <= i + 1 {
                continue;
            }
            let (c, d) = (q[j], q[j + 1]);
            if !ab.intersects(Rect::from_two_pos(c, d)) {
                continue;
            }
            if let Some((t, _)) = seg_cross(a, b, c, d) {
                out.push(i as f32 + t);
            }
        }
    }
    out
}

/// Point de la ligne à l'endroit `at` (indice du segment + fraction).
fn point_at(points: &[VPoint], at: f32) -> VPoint {
    let i = (at.floor().max(0.0) as usize).min(points.len().saturating_sub(2));
    let t = (at - i as f32).clamp(0.0, 1.0);
    let (a, b) = (points[i], points[(i + 1).min(points.len() - 1)]);
    VPoint { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, r: a.r + (b.r - a.r) * t }
}

/// Endroit de la ligne le plus proche de `c`, et l'écart au bord du trait (négatif : dedans).
fn nearest(points: &[VPoint], c: Pos2) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for i in 0..points.len().saturating_sub(1) {
        let (a, b) = (points[i].pos(), points[i + 1].pos());
        let ab = b - a;
        let len2 = ab.length_sq();
        let t = if len2 > 0.0 { ((c - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let at = i as f32 + t;
        let gap = (a + ab * t).distance(c) - point_at(points, at).r;
        if best.is_none_or(|(_, g)| gap < g) {
            best = Some((at, gap));
        }
    }
    best
}

fn length(points: &[VPoint]) -> f32 {
    points.windows(2).map(|w| w[0].pos().distance(w[1].pos())).sum()
}

/// Le bout `k` (0 : le début, 1 : la fin) d'une ligne, et la direction dans laquelle elle y sort.
fn bout(pts: &[Pos2], k: usize) -> (Pos2, Vec2) {
    let n = pts.len();
    let (p, autres): (Pos2, Box<dyn Iterator<Item = &Pos2>>) = if k == 0 { (pts[0], Box::new(pts[1..].iter())) } else { (pts[n - 1], Box::new(pts[..n - 1].iter().rev())) };
    let dir = autres.map(|q| p - *q).find(|v| v.length() > 1e-4).map_or(Vec2::ZERO, |v| v.normalized());
    (p, dir)
}

/// L'endroit (`at`) de la ligne à la distance `s` (le long d'elle) de son bout `k`.
fn at_depuis_le_bout(pts: &[Pos2], k: usize, s: f32) -> f32 {
    let n = pts.len();
    let mut reste = s;
    if k == 1 {
        for i in (0..n - 1).rev() {
            let l = pts[i].distance(pts[i + 1]);
            if l >= reste {
                return i as f32 + 1.0 - if l > 0.0 { reste / l } else { 0.0 };
            }
            reste -= l;
        }
        0.0
    } else {
        for i in 0..n - 1 {
            let l = pts[i].distance(pts[i + 1]);
            if l >= reste {
                return i as f32 + if l > 0.0 { reste / l } else { 0.0 };
            }
            reste -= l;
        }
        (n - 1) as f32
    }
}

/// Les points de la ligne entre `from` et `to`.
fn troncon(pts: &[Pos2], from: f32, to: f32) -> Vec<Pos2> {
    let n = pts.len();
    let at = |a: f32| {
        let i = (a.floor().max(0.0) as usize).min(n.saturating_sub(2));
        let t = (a - i as f32).clamp(0.0, 1.0);
        pts[i] + (pts[(i + 1).min(n - 1)] - pts[i]) * t
    };
    let mut out = vec![at(from)];
    for (k, p) in pts.iter().enumerate().skip(from.floor() as usize + 1) {
        if k as f32 >= to {
            break;
        }
        out.push(*p);
    }
    out.push(at(to));
    out
}

/// Le bout `bout_p` d'une ligne (qui en sort selon `dir`, sa queue étant `queue`) s'arrête-t-il à moins de `ecart` de la ligne
/// `cible` ? L'endroit de `cible` où il la toucherait : là où le bout prolongé la rencontre, sinon son point le plus proche
/// (si la queue ne la croise pas déjà). `plage` : la part de `cible` qui compte.
fn approche(bout_p: Pos2, dir: Vec2, queue: &[Pos2], cible: &[Pos2], plage: (f32, f32), ecart: f32) -> Option<f32> {
    let dans = |at: f32| at >= plage.0 && at <= plage.1;
    if dir != Vec2::ZERO {
        let loin = bout_p + dir * ecart;
        let zone = Rect::from_two_pos(bout_p, loin);
        let mut best: Option<(f32, f32)> = None;
        for j in 0..cible.len().saturating_sub(1) {
            let (c, d) = (cible[j], cible[j + 1]);
            if !zone.intersects(Rect::from_two_pos(c, d)) {
                continue;
            }
            if let Some((t, u)) = seg_cross(bout_p, loin, c, d) {
                let at = j as f32 + u;
                if t > 1e-3 && dans(at) && best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, at));
                }
            }
        }
        if let Some((_, at)) = best {
            return Some(at);
        }
    }
    // Le point le plus proche, s'il est assez près (et que le bout ne touche pas déjà ce trait).
    let mut best: Option<(f32, f32)> = None;
    for j in 0..cible.len().saturating_sub(1) {
        let (a, b) = (cible[j], cible[j + 1]);
        let ab = b - a;
        let len2 = ab.length_sq();
        let t = if len2 > 0.0 { ((bout_p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let at = j as f32 + t;
        let dist = (a + ab * t).distance(bout_p);
        if dans(at) && dist <= ecart && best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, at));
        }
    }
    let (dist, at) = best?;
    if dist < 1e-3 {
        return None;
    }
    let sub = troncon(cible, plage.0, plage.1);
    crossings(queue, &sub, false).is_empty().then_some(at)
}

/// Le point `p` est-il dans le polygone `poly` (fermé) ?
fn dedans(p: Pos2, poly: &[Pos2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Tout le morceau est-il dans le polygone (fermé) du geste ? (Les segments longs sont vérifiés tous les 2 pixels.)
fn tout_dedans(pts: &[VPoint], poly: &[Pos2], cadre: Rect) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let test = |p: Pos2| cadre.contains(p) && dedans(p, poly);
    if !pts.iter().all(|p| test(p.pos())) {
        return false;
    }
    pts.windows(2).all(|w| {
        let (a, b) = (w[0].pos(), w[1].pos());
        let k = ((a.distance(b) / 2.0).ceil() as usize).clamp(1, 64);
        (1..k).all(|s| test(a + (b - a) * (s as f32 / k as f32)))
    })
}

/// Le geste (une ligne) touche-t-il le morceau : il le croise, ou passe à moins de `tol` de son bord ?
fn touche(pts: &[VPoint], geste: &[Pos2], tol: f32) -> bool {
    let ligne: Vec<Pos2> = pts.iter().map(|p| p.pos()).collect();
    if !crossings(&ligne, geste, false).is_empty() {
        return true;
    }
    geste.iter().any(|g| nearest(pts, *g).is_some_and(|(_, gap)| gap <= tol))
}

/// Affine le morceau vers son début (`debut`) et/ou sa fin (`fin`) sur `l` pixels : des points sont ajoutés dans la longueur de
/// l'effilé, leur rayon décroît jusqu'à `POINTE` fois le rayon au bout.
fn effiler(pts: &[VPoint], debut: bool, fin: bool, l: f32) -> Vec<VPoint> {
    let total = length(pts);
    if total <= 0.0 || l <= 0.0 || (!debut && !fin) {
        return pts.to_vec();
    }
    let l = l.min(if debut && fin { total * 0.5 } else { total * 0.95 });
    let mut cum = vec![0.0f32];
    for w in pts.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + w[0].pos().distance(w[1].pos()));
    }
    let mut abscisses: Vec<f32> = cum.clone();
    const PAS: usize = 8;
    for k in 0..=PAS {
        let s = l * k as f32 / PAS as f32;
        if debut {
            abscisses.push(s);
        }
        if fin {
            abscisses.push(total - s);
        }
    }
    abscisses.sort_by(f32::total_cmp);
    abscisses.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    let a_l = |s: f32| -> VPoint {
        let i = cum.partition_point(|&c| c <= s).clamp(1, pts.len() - 1) - 1;
        let seg = cum[i + 1] - cum[i];
        let t = if seg > 0.0 { ((s - cum[i]) / seg).clamp(0.0, 1.0) } else { 0.0 };
        let (a, b) = (pts[i], pts[i + 1]);
        VPoint { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, r: a.r + (b.r - a.r) * t }
    };
    let facteur = |s: f32| -> f32 {
        let mut f: f32 = 1.0;
        for (actif, d) in [(debut, s), (fin, total - s)] {
            if actif && d < l {
                let t = (d / l).clamp(0.0, 1.0);
                f = f.min(POINTE + (1.0 - POINTE) * t * (2.0 - t));
            }
        }
        f
    };
    abscisses.into_iter().map(|s| {
        let mut p = a_l(s);
        p.r *= facteur(s);
        p
    }).collect()
}

/// Ce que reçoit une coupe.
pub struct Coupe<'a> {
    /// Le geste (points du dessin) : une ligne en travers, ou le tour d'un lasso.
    pub geste: &'a [Pos2],
    /// Portée d'un clic, et marge du geste (pixels du dessin).
    pub tol: f32,
    pub reglages: &'a Reglages,
    /// D'autres lignes de coupe (les traits des autres calques visibles, dans le repère de ce dessin) : elles coupent, on ne les
    /// modifie pas.
    pub autres: &'a [Vec<Pos2>],
}

/// Coupe les traits que touche le geste `gesture` (points du dessin) à leurs croisements, et
/// retire les morceaux touchés. Un geste sans longueur (un clic) touche le trait le plus proche à
/// `tol` près. Rend la zone changée, ou rien si aucun trait n'a été touché. (Les réglages d'origine.)
#[cfg(test)]
pub fn cut_strokes(d: &mut VectorDrawing, gesture: &[Pos2], tol: f32) -> Option<Rect> {
    couper(d, &Coupe { geste: gesture, tol, reglages: &Reglages::default(), autres: &[] }).map(|(r, _)| r)
}

/// La coupe avec ses réglages : la zone changée et le nombre de morceaux enlevés, ou rien.
pub fn couper(d: &mut VectorDrawing, c: &Coupe) -> Option<(Rect, usize)> {
    let r = c.reglages;
    let gesture = c.geste;
    let tol = c.tol;
    let n_el = d.elements.len();
    // Les lignes (les traits de ce dessin, puis les autres lignes de coupe) et leurs boîtes, une fois.
    let mut lignes: Vec<Option<(Vec<Pos2>, Rect)>> = d
        .elements
        .iter()
        .map(|e| match e {
            Element::Stroke { points, .. } if points.len() >= 2 => {
                let l: Vec<Pos2> = points.iter().map(|p| p.pos()).collect();
                let b = Rect::from_points(&l).expand(0.5);
                Some((l, b))
            }
            _ => None,
        })
        .collect();
    for l in c.autres.iter().filter(|l| l.len() >= 2) {
        lignes.push(Some((l.clone(), Rect::from_points(l).expand(0.5))));
    }
    let cadres: Vec<Rect> = d.elements.iter().map(Element::bounds).collect();
    let longueur_geste: f32 = gesture.windows(2).map(|w| w[0].distance(w[1])).sum();
    let click = gesture.len() < 2 || longueur_geste <= tol;
    let lasso = r.geste == Geste::Lasso && !click;
    let cadre_geste = Rect::from_points(gesture);
    let reach = cadre_geste.expand(tol);
    // Un clic : le seul trait le plus proche (le plus haut à égalité), s'il est à portée.
    let clicked: Option<(usize, f32)> = if click {
        let p = gesture.first().copied()?;
        let mut best: Option<(usize, f32, f32)> = None;
        for (i, e) in d.elements.iter().enumerate() {
            if let Element::Stroke { points, .. } = e {
                if let Some((at, gap)) = nearest(points, p) {
                    if gap <= tol && best.is_none_or(|(_, _, g)| gap <= g) {
                        best = Some((i, at, gap));
                    }
                }
            }
        }
        best.map(|(i, at, _)| (i, at))
    } else {
        None
    };
    let ecart = r.ecart.max(0.0);
    let mut region = Rect::NOTHING;
    let mut enleves = 0;
    let mut out = Vec::with_capacity(n_el);
    for (i, e) in d.elements.iter().enumerate() {
        let (Some((line, cadre_ligne)), Element::Stroke { points, color, opacity, hardness, brush, trace }) = (&lignes[i], e) else {
            out.push(e.clone());
            continue;
        };
        let bounds = cadres[i];
        let concerne = if click { clicked.is_some_and(|(k, _)| k == i) } else { bounds.intersects(reach) };
        if !concerne {
            out.push(e.clone());
            continue;
        }
        // Mode Couper en travers (ou un clic) : où le geste touche ce trait.
        let hits: Vec<f32> = if click {
            clicked.filter(|(k, _)| *k == i).map(|(_, at)| vec![at]).unwrap_or_default()
        } else if !lasso && r.mode == Mode::Couper {
            crossings(line, gesture, false)
        } else {
            Vec::new()
        };
        if r.mode == Mode::Couper && !lasso && hits.is_empty() {
            out.push(e.clone());
            continue;
        }
        // Où ce trait croise les autres, et lui-même.
        let mut cuts: Vec<f32> = crossings(line, line, true);
        for (j, other) in lignes.iter().enumerate() {
            if let Some((other, b)) = other.as_ref().filter(|_| j != i) {
                if b.intersects(*cadre_ligne) {
                    cuts.extend(crossings_dans(line, other, *b, false));
                }
            }
        }
        let end = (points.len() - 1) as f32;
        // L'écart toléré : les bouts des autres lignes qui s'arrêtent près de ce trait le coupent au point d'approche ; un bout
        // de ce trait qui s'arrête près d'une ligne n'est plus un bout libre.
        let mut libre = [true, true];
        if ecart > 0.0 {
            let pres = cadre_ligne.expand(ecart);
            for (j, other) in lignes.iter().enumerate() {
                let Some((other, _)) = other.as_ref().filter(|_| j != i) else { continue };
                for k in 0..2 {
                    let (p, dir) = bout(other, k);
                    if !pres.contains(p) {
                        continue;
                    }
                    let queue = troncon(other, if k == 0 { 0.0 } else { at_depuis_le_bout(other, 1, 2.0 * ecart) }, if k == 0 { at_depuis_le_bout(other, 0, 2.0 * ecart) } else { (other.len() - 1) as f32 });
                    if let Some(at) = approche(p, dir, &queue, line, (0.0, end), ecart) {
                        cuts.push(at);
                    }
                }
            }
            for (k, l) in libre.iter_mut().enumerate() {
                let (p, dir) = bout(line, k);
                let queue_at = at_depuis_le_bout(line, k, 2.0 * ecart);
                let queue = if k == 0 { troncon(line, 0.0, queue_at) } else { troncon(line, queue_at, end) };
                let pres_de = |b: &Rect| b.expand(ecart).contains(p);
                let ailleurs = lignes.iter().enumerate().any(|(j, o)| {
                    o.as_ref().filter(|(_, b)| j != i && pres_de(b)).is_some_and(|(o, _)| approche(p, dir, &queue, o, (0.0, (o.len() - 1) as f32), ecart).is_some())
                });
                if ailleurs {
                    *l = false;
                    continue;
                }
                // Le trait qui revient presque sur lui-même (une boucle pas tout à fait fermée) : hors de sa queue.
                let loin = at_depuis_le_bout(line, k, 2.0 * ecart + 2.0);
                let plage = if k == 0 { (loin, end) } else { (0.0, loin) };
                if plage.1 > plage.0 {
                    if let Some(at) = approche(p, dir, &queue, line, plage, ecart) {
                        cuts.push(at);
                        *l = false;
                    }
                }
            }
        }
        cuts.retain(|&c| c > 1e-3 && c < end - 1e-3);
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        let mut bornes = vec![0.0];
        bornes.extend(cuts);
        bornes.push(end);
        let n_morceaux = bornes.len() - 1;
        let morceaux: Vec<(f32, f32, Vec<VPoint>, Option<Vec<(f32, f64)>>)> = bornes
            .windows(2)
            .map(|w| {
                let (kept, samples) = vector::piece_traced(points, trace, w[0], w[1]);
                (w[0], w[1], kept, samples)
            })
            .collect();
        let retire: Vec<bool> = morceaux
            .iter()
            .enumerate()
            .map(|(k, (from, to, kept, _))| {
                let touche_le = || {
                    if click {
                        hits.iter().any(|&h| h >= *from && h <= *to)
                    } else {
                        touche(kept, gesture, tol) || tout_dedans(kept, gesture, cadre_geste)
                    }
                };
                match r.mode {
                    Mode::Couper if click || !lasso => hits.iter().any(|&h| h >= *from && h <= *to),
                    Mode::Couper => tout_dedans(kept, gesture, cadre_geste),
                    Mode::Depassements => {
                        let bout_libre = n_morceaux >= 2 && ((k == 0 && libre[0]) || (k + 1 == n_morceaux && libre[1]));
                        bout_libre && length(kept) < r.depassement && touche_le()
                    }
                }
            })
            .collect();
        if !retire.iter().any(|&x| x) {
            out.push(e.clone());
            continue;
        }
        for (k, (_, _, kept, samples)) in morceaux.into_iter().enumerate() {
            if retire[k] {
                enleves += 1;
                continue;
            }
            if kept.len() < 2 || length(&kept) < MIN_PIECE {
                continue;
            }
            // Le bout laissé par une coupe (son voisin est parti) s'affine.
            let debut = k > 0 && retire[k - 1];
            let fin = k + 1 < n_morceaux && retire[k + 1];
            if r.effile > 0.0 && (debut || fin) {
                // (Les appuis enregistrés ne suivent plus les points ajoutés : le rendu reprend l'appui d'après les rayons. Pas
                // d'allègement : il aplatirait l'effilé d'un trait fin sur toute sa longueur.)
                let pts = effiler(&kept, debut, fin, r.effile);
                out.push(Element::Stroke { points: pts, color: *color, opacity: *opacity, hardness: *hardness, brush: brush.clone(), trace: trace.without_samples() });
            } else {
                out.push(vector::piece_stroke(kept, samples, *color, *opacity, *hardness, brush, trace));
            }
        }
        region = region.union(bounds);
    }
    if region == Rect::NOTHING {
        return None;
    }
    d.elements = out;
    Some((region, enleves))
}

/// Les traits d'un dessin, ramenés par `vers` dans un autre repère : des lignes de coupe.
fn lignes_de(d: &VectorDrawing, vers: &Affine) -> Vec<Vec<Pos2>> {
    d.elements
        .iter()
        .filter_map(|e| match e {
            Element::Stroke { points, .. } if points.len() >= 2 => Some(points.iter().map(|p| vers.apply(p.pos())).collect()),
            _ => None,
        })
        .collect()
}

impl App {
    /// Appui avec le cutter : le geste commence (calque vectoriel seulement).
    pub(crate) fn cutter_press(&mut self, p: Pos2) {
        if !self.layer_editable() {
            return;
        }
        // Aplat (`fondui.rs`) : un clic retire de l'aplat la zone fermée par le trait, sur un
        // calque bitmap comme vectoriel.
        if self.aplat_en_cible() {
            let avant = self.history.serial();
            self.fond_zone_at(p, true);
            // (Plusieurs clés choisies : la même zone se retire de l'aplat des autres, `aplatmulti.rs`.)
            self.aplat_autres_cles(p, true, avant);
            return;
        }
        if self.layer().kind != LayerKind::Vector {
            self.status = "Le cutter coupe les traits d'un calque vectoriel.".into();
            return;
        }
        self.end_stroke();
        self.playing = false;
        self.cut_gesture = Some(vec![p]);
    }

    /// Le geste du cutter suit le pointeur ; faux s'il n'y a pas de geste en cours.
    pub(crate) fn cutter_drag(&mut self, p: Pos2) -> bool {
        let zoom = self.layer_zoom();
        let Some(g) = &mut self.cut_gesture else { return false };
        if g.last().is_none_or(|q| q.distance(p) * zoom > 1.5) {
            g.push(p);
        }
        true
    }

    /// Les lignes de coupe des autres calques vectoriels visibles du plan courant à l'image `frame` (pas le dessin `sauf`),
    /// ramenées dans le repère du dessin qui va de la feuille par `depuis_feuille`. Rien hors de la portée « tous les calques
    /// visibles », ni dans une case des Vignettes.
    fn lignes_visibles(&self, frame: usize, sauf: &[CelId], depuis_feuille: &Affine) -> Vec<Vec<Pos2>> {
        if self.settings.cutter.portee != Portee::Visibles || self.in_card {
            return Vec::new();
        }
        let Some(panel) = self.doc.panels.get(self.cur_panel) else { return Vec::new() };
        let mut out = Vec::new();
        for (li, l) in panel.layers.iter().enumerate() {
            if l.folder || l.kind != LayerKind::Vector || l.video.is_some() || crate::folders::effect(&panel.layers, li).is_none() {
                continue;
            }
            let Some(cel) = l.cel_at(frame).filter(|c| !sauf.contains(c)) else { continue };
            let Some(d) = self.doc.cels.get(&cel).and_then(|c| c.vector.as_ref()) else { continue };
            let Some(to_stage) = crate::multisel::layer_to_stage(&self.doc, self.cur_panel, frame, li) else { continue };
            out.extend(lignes_de(d, &to_stage.then(depuis_feuille)));
        }
        out
    }

    /// Au relâché : on coupe (une étape d'annulation).
    pub(crate) fn finish_cut(&mut self) {
        let Some(gesture) = self.cut_gesture.take() else { return };
        let tol = 4.0 / self.layer_zoom();
        // Plusieurs calques ou clés choisis : tous les dessins choisis (« la sélection agit sur tout »).
        if !self.in_card && self.plusieurs_dessins_choisis() {
            self.cutter_multi(&gesture, tol);
            return;
        }
        let Some(cel) = self.cel_here() else { return };
        let autres = match crate::multisel::layer_to_stage(&self.doc, self.cur_panel, self.cur_frame, self.cur_layer).and_then(|a| a.inverse()) {
            Some(inv) => self.lignes_visibles(self.cur_frame, &[cel], &inv),
            None => Vec::new(),
        };
        let reglages = self.settings.cutter;
        let Some(d) = self.drawing_mut(cel) else { return };
        let before = d.clone();
        match couper(d, &Coupe { geste: &gesture, tol, reglages: &reglages, autres: &autres }) {
            Some((region, _)) => {
                // Les indices des éléments ont changé.
                self.vsel = None;
                self.refresh_cel_region(cel, region);
                self.push_vector(cel, before, None);
                self.status = match reglages.mode {
                    Mode::Couper => "Traits coupés.".into(),
                    Mode::Depassements => "Dépassements nettoyés.".into(),
                };
            }
            None => self.status = self.rien_a_couper().into(),
        }
    }

    fn rien_a_couper(&self) -> &'static str {
        match (self.settings.cutter.mode, self.settings.cutter.geste) {
            (Mode::Couper, Geste::Travers) => "Rien à couper : glisse en travers d'un bout de trait.",
            (Mode::Couper, Geste::Lasso) => "Rien à couper : entoure entièrement les bouts à enlever.",
            (Mode::Depassements, _) => "Aucun dépassement assez court dans la zone (Cutter : « Dépassements enlevés jusqu'à »).",
        }
    }

    /// Le geste sur tous les dessins choisis (plusieurs calques, ou plusieurs clés) : chacun reçoit le geste dans son repère,
    /// tel qu'on le voit ; une seule étape d'annulation ; le compte dans la barre d'état.
    fn cutter_multi(&mut self, gesture: &[Pos2], tol: f32) {
        let Some(courant) = crate::multisel::layer_to_stage(&self.doc, self.cur_panel, self.cur_frame, self.cur_layer) else { return };
        let (targets, par_cles) = self.dessins_choisis();
        let reglages = self.settings.cutter;
        // (Le dessin qu'on voit coupe toujours, choisi ou non.)
        let mut liste: Vec<(CelId, Affine, usize, bool)> = Vec::new();
        if let Some(c) = self.cel_here().filter(|c| !targets.list.iter().any(|t| t.cel == *c)) {
            if self.layer().kind == LayerKind::Vector {
                liste.push((c, Affine::IDENTITY, self.cur_frame, true));
            }
        }
        for t in targets.list.iter().filter(|t| t.vector) {
            let Some(inv) = t.to_stage.inverse() else { continue };
            liste.push((t.cel, courant.then(&inv), t.frame, t.live));
        }
        let choisis: Vec<CelId> = liste.iter().map(|x| x.0).collect();
        let mut edits = Vec::new();
        let mut enleves = 0;
        let mut dessins = 0;
        for (cel, vers, frame, live) in liste {
            let g: Vec<Pos2> = gesture.iter().map(|p| vers.apply(*p)).collect();
            let k = vers.width_factor().max(1e-3);
            // (Les lignes de coupe des autres calques : de la feuille vers ce dessin.)
            let depuis_feuille = courant.inverse().map(|ci| ci.then(&vers));
            let autres = depuis_feuille.map(|a| self.lignes_visibles(frame, &choisis, &a)).unwrap_or_default();
            let Some(d) = self.doc.cels.get_mut(&cel).and_then(|c| c.vector.as_mut()) else { continue };
            let avant = d.clone();
            let Some((region, n)) = couper(d, &Coupe { geste: &g, tol: tol * k, reglages: &reglages, autres: &autres }) else { continue };
            enleves += n;
            dessins += 1;
            if let Some(c) = self.doc.cels.get_mut(&cel) {
                c.version += 1;
            }
            if live {
                self.refresh_cel_region(cel, region);
            } else {
                self.rejeu_cel(cel);
            }
            edits.push(Edit::Vector { cel, drawing: avant });
        }
        if edits.is_empty() {
            self.status = self.rien_a_couper().into();
            return;
        }
        self.vsel = None;
        let avant = self.history.context.replace("Cutter".to_string());
        self.history.push(Edit::Group(edits));
        self.history.context = avant;
        self.modified = true;
        let quoi = Self::dessins_texte(&targets, par_cles);
        let s = |n: usize| if n > 1 { "s" } else { "" };
        let mut texte = format!("Cutter sur {quoi} : {enleves} bout{} enlevé{} dans {dessins} dessin{}.", s(enleves), s(enleves), s(dessins));
        if let Some(l) = targets.skipped_text() {
            texte = format!("{texte} {l}");
        }
        self.status = texte;
    }
}
