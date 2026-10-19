//! Coloriage par zones (Anthony, 2026-10-19) : propager des aplats de couleur le long d'une
//! animation en retrouvant, d'une image à l'autre, les mêmes zones fermées.
//!
//! Pour l'animation « cel » : on colorie des images clés (le trait, et des aplats dans les zones
//! fermées, sur un calque de couleur) ; SHOT reporte ces couleurs sur les autres images en
//! appariant les zones, puis écrit de vrais aplats nets et modifiables sous le trait intact : des
//! formes pleines vectorielles sur un calque vectoriel, des aplats de pixels sur un calque bitmap.
//! Rien de flou ni de généré. Voir `docs/COLORIAGE.md` pour la méthode et ses limites.
//!
//! Ici, tout ce qui ne touche pas au document (il tourne sur un autre fil, `coloriageui.rs`) :
//! 1. `zones::segment` découpe chaque image du trait en zones fermées (en parallèle, gardées par
//!    version du dessin dans un `ZoneCache`) ;
//! 2. `read_colours` lit la couleur dominante de chaque zone d'une clé ;
//! 3. `match_zones` apparie les zones de deux images qui se suivent : déplacement du centre, rapport
//!    des aires, recouvrement, forme, cohérence des voisines, par l'algorithme hongrois ; une zone
//!    peut se couper ou se réunir ;
//! 4. `colorier` propage depuis la clé d'avant et celle d'après (la plus sûre l'emporte), décide ce
//!    qui est trop incertain (laissé vide ou marqué « à vérifier ») et note les doutes ;
//! 5. `fill_raster` et `fill_vector` écrivent les aplats.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::Color32;

use crate::raster::{premultiply, unpremultiply, Raster, TILE};
use crate::vector::{trace_contours, Element, VectorDrawing};
use crate::zones::{Segmentation, Zone, ZoneCache};

pub use crate::zones::Reglages as ReglagesZones;

/// Une couleur non prémultipliée : rouge, vert, bleu, opacité.
pub type Rgba = [u8; 4];

/// Ce que l'on fait d'une zone dont la couleur est trop incertaine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Incertain {
    /// Elle reste sans couleur.
    Laisser,
    /// Elle prend cette couleur de repère (« à vérifier »).
    Marquer([u8; 3]),
}

/// Couleur de repère d'une zone à vérifier.
pub const REPERE: [u8; 3] = [255, 0, 255];

/// Les réglages du coloriage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parametres {
    pub zones: ReglagesZones,
    pub incertain: Incertain,
    /// Confiance (de 0 à 1) en dessous de laquelle une zone est incertaine.
    pub seuil: f32,
}

impl Default for Parametres {
    fn default() -> Self {
        Parametres { zones: ReglagesZones::default(), incertain: Incertain::Laisser, seuil: 0.5 }
    }
}

/// Aire (en pixels) en dessous de laquelle une zone nouvelle n'est pas signalée (bruit de tracé).
pub const AIRE_MINI_RAPPORT: u32 = 40;

// ------------------------------------------------------------------------- conseiller de couleur

/// Ce qu'un conseiller de couleur reçoit pour une zone trop incertaine. (Les champs sont lus par
/// les conseillers, aucun n'existe encore dans SHOT.)
#[allow(dead_code)]
pub struct Question<'a> {
    /// Rang de l'image dans la demande.
    pub cadre: usize,
    pub segmentation: &'a Segmentation,
    pub zone: &'a Zone,
    /// La couleur que l'appariement proposait, et la confiance qu'on lui prête.
    pub proposition: Option<Rgba>,
    pub confiance: f32,
}

/// Le conseil : une couleur et la confiance qu'on lui accorde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conseil {
    pub couleur: Rgba,
    pub confiance: f32,
}

/// Un « conseiller de couleur » : un avis sur la couleur d'une zone que l'appariement n'a pas pu
/// trancher (nouvelle zone, appariement ambigu). Prévu pour un modèle génératif, plus tard, comme
/// simple indice : le résultat reste un aplat de la couleur conseillée, jamais des pixels générés.
/// Aujourd'hui, `SansConseil` ne dit rien.
pub trait ConseillerCouleur: Send + Sync {
    fn conseille(&self, question: &Question) -> Option<Conseil>;
}

/// Le conseiller qui n'a aucun avis (celui de SHOT pour l'instant).
pub struct SansConseil;

impl ConseillerCouleur for SansConseil {
    fn conseille(&self, _: &Question) -> Option<Conseil> {
        None
    }
}

// --------------------------------------------------------------------------------- appariement

/// Pourquoi on se fie (ou pas) à la couleur d'une zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Clé, ou bon appariement.
    Sure,
    /// Une autre zone aurait pu faire l'affaire.
    Ambiguous,
    /// La zone vient d'une zone coupée en deux.
    Split,
    /// Des zones de couleurs différentes se sont réunies.
    Merge,
    /// Les deux clés ne sont pas d'accord.
    Disagree,
    /// Aucune zone de l'image d'avant ne lui ressemble.
    New,
    /// La couleur vient du conseiller.
    Advisor,
}

/// Pourquoi une zone est signalée.
pub type Raison = Why;

impl Why {
    pub fn label(self) -> &'static str {
        match self {
            Why::Sure => "appariement douteux",
            Why::Ambiguous => "appariement ambigu",
            Why::Split => "zone coupée",
            Why::Merge => "zones réunies de couleurs différentes",
            Why::Disagree => "les deux clés se contredisent",
            Why::New => "nouvelle zone",
            Why::Advisor => "avis du conseiller",
        }
    }
}

/// La zone de l'image d'avant dont vient une zone.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub from: u16,
    pub conf: f32,
    pub why: Why,
    /// Autres zones d'avant réunies dans celle-ci.
    pub also: Vec<u16>,
}

const UNMATCHED: f64 = 0.55;
const MAX_PAIR_COST: f64 = 1.1;
const CANDIDATES: usize = 8;
const HUNGARIAN_MAX: usize = 500;

/// Hongrois (Kuhn-Munkres, O(n³)) sur une matrice carrée : la colonne de chaque ligne.
pub(crate) fn hungarian(a: &[Vec<f64>]) -> Vec<usize> {
    let n = a.len();
    let inf = f64::INFINITY;
    let (mut u, mut v) = (vec![0.0; n + 1], vec![0.0; n + 1]);
    let (mut p, mut way) = (vec![0usize; n + 1], vec![0usize; n + 1]);
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0;
        let mut minv = vec![inf; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let (i0, mut delta, mut j1) = (p[j0], inf, 0);
            for j in 1..=n {
                if !used[j] {
                    let cur = a[i0 - 1][j - 1] - u[i0] - v[j];
                    if cur < minv[j] {
                        minv[j] = cur;
                        way[j] = j0;
                    }
                    if minv[j] < delta {
                        delta = minv[j];
                        j1 = j;
                    }
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    let mut ans = vec![0usize; n];
    for j in 1..=n {
        if p[j] != 0 {
            ans[p[j] - 1] = j - 1;
        }
    }
    ans
}

/// Les paires choisies (a, b) parmi les coûts `costs` (par b : les (a, coût) candidats) : un
/// appariement un à un qui peut laisser des zones seules (coût `UNMATCHED` chacune).
fn assign(na: usize, nb: usize, costs: &[Vec<(usize, f64)>]) -> Vec<Option<usize>> {
    let mut by_b: Vec<Option<usize>> = vec![None; nb];
    let n = na + nb;
    if n > HUNGARIAN_MAX {
        // Trop de zones pour le hongrois : les meilleures paires d'abord.
        let mut pairs: Vec<(f64, usize, usize)> = costs.iter().enumerate().flat_map(|(b, c)| c.iter().map(move |&(a, cost)| (cost, a, b))).collect();
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut taken = vec![false; na];
        for (cost, a, b) in pairs {
            if cost <= MAX_PAIR_COST && !taken[a] && by_b[b].is_none() {
                taken[a] = true;
                by_b[b] = Some(a);
            }
        }
        return by_b;
    }
    const BIG: f64 = 1e6;
    let mut m = vec![vec![BIG; n]; n];
    for (b, c) in costs.iter().enumerate() {
        for &(a, cost) in c {
            m[a][b] = cost;
        }
    }
    for a in 0..na {
        m[a][nb + a] = UNMATCHED;
    }
    for b in 0..nb {
        m[na + b][b] = UNMATCHED;
    }
    for row in m.iter_mut().skip(na) {
        for x in row.iter_mut().skip(nb) {
            *x = 0.0;
        }
    }
    let cols = hungarian(&m);
    for a in 0..na {
        let c = cols[a];
        if c < nb && m[a][c] < BIG {
            by_b[c] = Some(a);
        }
    }
    by_b
}

/// Les rangs des `k` plus petites valeurs (sans tout trier : des centaines de zones par image).
fn smallest(mut v: Vec<(f64, usize)>, k: usize) -> Vec<usize> {
    if v.len() > k {
        v.select_nth_unstable_by(k, |p, q| p.0.total_cmp(&q.0));
        v.truncate(k);
    }
    v.into_iter().map(|p| p.1).collect()
}

/// Le nombre (estimé) de pixels de la zone `zb` de `b` qui étaient dans la zone `ida` de `a`, `a`
/// décalée de (`dx`, `dy`).
fn overlap(a: &Segmentation, za: &Zone, b: &Segmentation, zb: &Zone, dx: i32, dy: i32) -> f32 {
    let ida = za.id;
    // Les boîtes décalées ne se touchent pas : rien en commun.
    let (ax0, ay0, ax1, ay1) = (za.bbox[0] as i64 + dx as i64, za.bbox[1] as i64 + dy as i64, za.bbox[2] as i64 + dx as i64, za.bbox[3] as i64 + dy as i64);
    let (x0, y0, x1, y1) = ((zb.bbox[0] as i64).max(ax0), (zb.bbox[1] as i64).max(ay0), (zb.bbox[2] as i64).min(ax1), (zb.bbox[3] as i64).min(ay1));
    if x0 > x1 || y0 > y1 {
        return 0.0;
    }
    let box_area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as usize;
    let stride: i64 = if box_area <= 20_000 {
        1
    } else if box_area <= 160_000 {
        2
    } else {
        3
    };
    let mut n = 0u32;
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            if b.label_at(x, y) == zb.id && a.label_at(x - dx as i64, y - dy as i64) == ida {
                n += 1;
            }
            x += stride;
        }
        y += stride;
    }
    (n as i64 * stride * stride) as f32
}

/// Le déplacement de tout le dessin d'une image à l'autre : celui qui superpose le mieux les
/// silhouettes (la plus grande partie des zones qui ne sont pas le dehors).
fn global_shift(a: &Segmentation, b: &Segmentation) -> (f32, f32) {
    let weighted = |s: &Segmentation| {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for z in s.inner() {
            sx += z.centroid.0 as f64 * z.area as f64;
            sy += z.centroid.1 as f64 * z.area as f64;
            n += z.area as f64;
        }
        (sx / n.max(1.0), sy / n.max(1.0))
    };
    let (ca, cb) = (weighted(a), weighted(b));
    let union = a.inner().fold([u32::MAX, u32::MAX, 0, 0], |r, z| [r[0].min(z.bbox[0]), r[1].min(z.bbox[1]), r[2].max(z.bbox[2]), r[3].max(z.bbox[3])]);
    if union[0] == u32::MAX {
        return (0.0, 0.0);
    }
    let outside_a: Vec<bool> = a.zones.iter().map(|z| z.outside).collect();
    let outside_b: Vec<bool> = b.zones.iter().map(|z| z.outside).collect();
    let score = |t: (i32, i32), stride: usize| -> u32 {
        let mut n = 0u32;
        let mut y = union[1] as i64;
        while y <= union[3] as i64 {
            let mut x = union[0] as i64;
            while x <= union[2] as i64 {
                let la = a.label_at(x, y);
                if la != 0 && !outside_a[la as usize - 1] {
                    let lb = b.label_at(x + t.0 as i64, y + t.1 as i64);
                    if lb != 0 && !outside_b[lb as usize - 1] {
                        n += 1;
                    }
                }
                x += stride as i64;
            }
            y += stride as i64;
        }
        n
    };
    let start = [(0, 0), ((cb.0 - ca.0).round() as i32, (cb.1 - ca.1).round() as i32)];
    let mut best = start[0];
    let mut best_score = score(best, 8);
    for &c in &start[1..] {
        let s = score(c, 8);
        if s > best_score {
            (best, best_score) = (c, s);
        }
    }
    for (stride, step) in [(8usize, 4i32), (4, 2), (3, 1)] {
        let mut current = score(best, stride);
        for _ in 0..24 {
            let mut moved = false;
            for (dx, dy) in [(step, 0), (-step, 0), (0, step), (0, -step), (step, step), (-step, -step), (step, -step), (-step, step)] {
                let t = (best.0 + dx, best.1 + dy);
                let s = score(t, stride);
                if s > current {
                    (best, current, moved) = (t, s, true);
                }
            }
            if !moved {
                break;
            }
        }
    }
    (best.0 as f32, best.1 as f32)
}

/// Le coût de l'appariement de `za` (d'avant, attendue en `pred`) avec `zb`.
fn pair_cost(za: &Zone, zb: &Zone, pred: (f32, f32), iou: f32, cons: Option<f32>) -> f64 {
    let r = 0.5 * (za.radius() + zb.radius()) + 6.0;
    let (dx, dy) = (pred.0 - zb.centroid.0, pred.1 - zb.centroid.1);
    let dpos = ((dx * dx + dy * dy).sqrt() / r).min(2.5) as f64;
    let darea = (zb.area as f64 / za.area.max(1) as f64).ln().abs().min(2.0);
    let dshape = ((zb.hu[0] / za.hu[0].max(1e-9)).ln().abs() + 4.0 * (zb.hu[1] - za.hu[1]).abs()).min(2.0);
    let neighbours = match cons {
        Some(k) => 0.3 * (1.0 - k as f64),
        None => 0.15,
    };
    0.35 * dpos + 0.30 * darea + 0.55 * (1.0 - iou as f64) + 0.25 * dshape + neighbours
}

/// La confiance qu'inspire un coût d'appariement.
fn confidence(cost: f64) -> f32 {
    (1.0 - (cost - 0.15).max(0.0) / 0.9).clamp(0.0, 1.0) as f32
}

/// Apparie les zones de `b` à celles de `a` (l'image d'avant) : pour chaque zone de `b` (dans
/// l'ordre de `b.zones`), la zone de `a` dont elle vient, si une lui ressemble assez. Le dehors n'est
/// jamais apparié.
pub fn match_zones(a: &Segmentation, b: &Segmentation) -> Vec<Option<Link>> {
    let mut links: Vec<Option<Link>> = vec![None; b.zones.len()];
    let za: Vec<&Zone> = a.inner().collect();
    let zb: Vec<&Zone> = b.inner().collect();
    if za.is_empty() || zb.is_empty() {
        return links;
    }
    let (na, nb) = (za.len(), zb.len());
    let index_a: HashMap<u16, usize> = za.iter().enumerate().map(|(i, z)| (z.id, i)).collect();
    let index_b: HashMap<u16, usize> = zb.iter().enumerate().map(|(i, z)| (z.id, i)).collect();
    let t = global_shift(a, b);
    // Première ronde : le dessin entier a bougé de `t`, ou la zone n'a pas bougé (un décor fixe) ;
    // dans les deux rondes, on essaie aussi chaque paire « alignée » (la zone d'avant glissée sur
    // son éventuelle pareille), au prix d'un écart au mouvement attendu.
    let first_guesses: Vec<(f32, f32)> = if t.0.abs() + t.1.abs() > 2.0 { vec![t, (0.0, 0.0)] } else { vec![t] };
    let mut disp: Vec<(f32, f32)> = vec![t; na];
    let mut previous: Option<Vec<Option<usize>>> = None;
    let mut last: Option<(Vec<Vec<(usize, f64)>>, HashMap<(usize, usize), f32>, Vec<Option<usize>>)> = None;
    for round in 0..2 {
        let mut costs: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nb];
        let mut overlaps: HashMap<(usize, usize), f32> = HashMap::new();
        for (j, zj) in zb.iter().enumerate() {
            // Les hypothèses de déplacement de chaque zone d'avant ; la première ronde essaie
            // `first_guesses` pour toutes, la seconde la prévision de chacune.
            let guess = |i: usize, h: usize| if round == 0 { first_guesses[h] } else { disp[i] };
            let hypotheses = if round == 0 { first_guesses.len() } else { 1 };
            // Les zones d'avant les plus proches de l'endroit où l'on attendait celle-ci, et les
            // trois qui lui ressemblent le plus par l'aire et la forme.
            let mut near: Vec<usize> = Vec::new();
            for h in 0..hypotheses {
                let by_distance: Vec<(f64, usize)> = (0..na)
                    .map(|i| {
                        let g = guess(i, h);
                        let (dx, dy) = (za[i].centroid.0 + g.0 - zj.centroid.0, za[i].centroid.1 + g.1 - zj.centroid.1);
                        ((dx * dx + dy * dy) as f64, i)
                    })
                    .collect();
                near.extend(smallest(by_distance, if h == 0 { CANDIDATES } else { CANDIDATES / 2 }));
            }
            let alike: Vec<(f64, usize)> = (0..na)
                .map(|i| ((zj.area as f64 / za[i].area.max(1) as f64).ln().abs() + (zj.hu[0] / za[i].hu[0].max(1e-9)).ln().abs(), i))
                .collect();
            near.extend(smallest(alike, 3));
            near.sort_unstable();
            near.dedup();
            for &i in &near {
                // Cohérence des voisines : celles d'avant déjà appariées doivent l'être à des voisines
                // de celle-ci.
                let cons = previous.as_ref().and_then(|prev| {
                    let (mut seen, mut ok) = (0u32, 0u32);
                    for &(n, _) in &za[i].neighbours {
                        let Some(&ni) = index_a.get(&n) else { continue };
                        let Some(m) = prev.iter().position(|x| *x == Some(ni)) else { continue };
                        seen += 1;
                        if zj.neighbours.iter().any(|&(id, _)| index_b.get(&id) == Some(&m)) {
                            ok += 1;
                        }
                    }
                    (seen > 0).then(|| ok as f32 / seen as f32).or(Some(0.5))
                });
                let mut best: Option<(f64, f32)> = None;
                let mut plain: Option<(f64, f32)> = None;
                let reach = 0.5 * (za[i].radius() + zj.radius()) + 6.0;
                let aligned = (zj.centroid.0 - za[i].centroid.0, zj.centroid.1 - za[i].centroid.1);
                for h in 0..=hypotheses {
                    // (La dernière hypothèse est l'alignement des centres : sans écart de position,
                    // mais le mouvement s'écarte de celui qu'on attendait.)
                    let (g, penalty) = if h < hypotheses {
                        (guess(i, h), 0.0)
                    } else {
                        let expected = guess(i, 0);
                        let off = ((aligned.0 - expected.0).powi(2) + (aligned.1 - expected.1).powi(2)).sqrt() / reach;
                        (aligned, 0.35 * 0.5 * off.min(2.5) as f64)
                    };
                    let ov = overlap(a, za[i], b, zj, g.0.round() as i32, g.1.round() as i32);
                    let union = za[i].area as f32 + zj.area as f32 - ov;
                    let iou = if union > 0.0 { (ov / union).clamp(0.0, 1.0) } else { 0.0 };
                    let pred = (za[i].centroid.0 + g.0, za[i].centroid.1 + g.1);
                    let cost = pair_cost(za[i], zj, pred, iou, cons) + penalty;
                    if best.is_none_or(|b| cost < b.0) {
                        best = Some((cost, ov));
                    }
                    // (Le recouvrement qui sert à voir une zone coupée ou réunies est celui d'un
                    // vrai déplacement du dessin, pas celui de l'alignement des centres.)
                    if h < hypotheses && plain.is_none_or(|b| cost < b.0) {
                        plain = Some((cost, ov));
                    }
                }
                if let (Some((cost, _)), Some((_, ov))) = (best, plain) {
                    overlaps.insert((i, j), ov);
                    costs[j].push((i, cost));
                }
            }
        }
        let chosen = assign(na, nb, &costs);
        if round == 0 {
            // Où va chaque zone d'avant : comme l'a montré son appariement s'il est bon, sinon
            // comme ses voisines appariées, sinon comme le dessin entier.
            let moved: Vec<Option<((f32, f32), f64)>> = {
                let mut m = vec![None; na];
                for (j, c) in chosen.iter().enumerate() {
                    if let Some(i) = *c {
                        let cost = costs[j].iter().find(|p| p.0 == i).map_or(f64::INFINITY, |p| p.1);
                        m[i] = Some(((zb[j].centroid.0 - za[i].centroid.0, zb[j].centroid.1 - za[i].centroid.1), cost));
                    }
                }
                m
            };
            for i in 0..na {
                if let Some((d, cost)) = moved[i] {
                    if cost <= 0.7 {
                        disp[i] = d;
                        continue;
                    }
                }
                let (mut sx, mut sy, mut sw) = (0.0f32, 0.0f32, 0.0f32);
                for &(n, len) in &za[i].neighbours {
                    if let Some((d, _)) = index_a.get(&n).and_then(|&ni| moved[ni]) {
                        sx += d.0 * len as f32;
                        sy += d.1 * len as f32;
                        sw += len as f32;
                    }
                }
                disp[i] = if sw > 0.0 { (sx / sw, sy / sw) } else { t };
            }
            previous = Some(chosen.clone());
        }
        last = Some((costs, overlaps, chosen));
    }
    let Some((costs, overlaps, chosen)) = last else { return links };
    // Les liens un à un, avec la confiance du coût et le doute des autres choix possibles.
    let mut by_a: Vec<Vec<(usize, f64)>> = vec![Vec::new(); na];
    for (j, c) in costs.iter().enumerate() {
        for &(i, cost) in c {
            by_a[i].push((j, cost));
        }
    }
    let mut taken_a = vec![false; na];
    for (j, c) in chosen.iter().enumerate() {
        let Some(i) = *c else { continue };
        let Some(&(_, cost)) = costs[j].iter().find(|(x, _)| *x == i) else { continue };
        if cost > MAX_PAIR_COST {
            continue;
        }
        let alt_b = costs[j].iter().filter(|(x, _)| *x != i).map(|p| p.1).fold(f64::INFINITY, f64::min);
        let alt_a = by_a[i].iter().filter(|(x, _)| *x != j).map(|p| p.1).fold(f64::INFINITY, f64::min);
        let alt = alt_a.min(alt_b);
        let margin = if alt.is_finite() { (alt - cost).max(0.0) } else { f64::INFINITY };
        let factor = if margin.is_finite() { (0.5 + 2.5 * margin).clamp(0.5, 1.0) } else { 1.0 };
        taken_a[i] = true;
        let why = if factor < 0.85 { Why::Ambiguous } else { Why::Sure };
        links[zb[j].id as usize - 1] = Some(Link { from: za[i].id, conf: confidence(cost) * factor as f32, why, also: Vec::new() });
    }
    // Une zone sans pareille qui était, pour l'essentiel, dans une zone d'avant : zone coupée.
    // (Un appariement peu sûr passe aussi pour une coupure : l'une des moitiés a pris la zone d'avant
    // à son compte, mais la zone d'avant était plus grande qu'elle.)
    for j in 0..nb {
        let slot = zb[j].id as usize - 1;
        if links[slot].as_ref().is_some_and(|l| l.conf >= 0.5) {
            continue;
        }
        let best = costs[j]
            .iter()
            .map(|&(i, _)| (i, overlaps.get(&(i, j)).copied().unwrap_or(0.0) / zb[j].area.max(1) as f32))
            .max_by(|p, q| p.1.total_cmp(&q.1));
        if let Some((i, frac)) = best.filter(|&(_, f)| f >= 0.6) {
            let conf = 0.55 + 0.1 * ((frac - 0.6) / 0.4).clamp(0.0, 1.0);
            if links[slot].as_ref().is_none_or(|l| l.conf < conf) {
                links[slot] = Some(Link { from: za[i].id, conf, why: Why::Split, also: Vec::new() });
            }
        }
    }
    // Une zone d'avant sans pareille qui est, pour l'essentiel, dans une zone d'après : réunion.
    for i in 0..na {
        if taken_a[i] {
            continue;
        }
        let best = by_a[i]
            .iter()
            .map(|&(j, _)| (j, overlaps.get(&(i, j)).copied().unwrap_or(0.0) / za[i].area.max(1) as f32))
            .max_by(|p, q| p.1.total_cmp(&q.1));
        if let Some((j, _)) = best.filter(|&(_, f)| f >= 0.6) {
            if let Some(l) = links[zb[j].id as usize - 1].as_mut() {
                if l.from != za[i].id {
                    l.also.push(za[i].id);
                }
            }
        }
    }
    links
}

// --------------------------------------------------------------------------------- les couleurs

/// La couleur d'une zone, avec la confiance qu'on lui prête.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub rgba: Option<Rgba>,
    pub conf: f32,
    pub why: Why,
}

impl Slot {
    fn new_zone() -> Slot {
        Slot { rgba: None, conf: 0.0, why: Why::New }
    }
}

/// La couleur dominante de chaque zone de `seg` dans le dessin de couleurs `colour` (dans l'ordre de
/// `seg.zones`) : celle qui couvre au moins le tiers de la zone ; sinon, ou si le dessin est vide à
/// cet endroit, la zone n'est pas coloriée.
pub fn read_colours(seg: &Segmentation, colour: &Raster) -> Vec<Option<Rgba>> {
    let mut buckets: Vec<Vec<(Rgba, u32)>> = vec![Vec::new(); seg.zones.len()];
    let mut last = vec![0usize; seg.zones.len()];
    for (idx, t) in colour.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = colour.tile_bounds(idx);
        if ox + tw <= seg.x0 || oy + th <= seg.y0 || ox >= seg.x0 + seg.bw || oy >= seg.y0 + seg.bh {
            continue;
        }
        for y in 0..th {
            for x in 0..tw {
                let c = t[y * TILE + x];
                if c.a() < 16 {
                    continue;
                }
                let l = seg.label_at((ox + x) as i64, (oy + y) as i64);
                if l == 0 {
                    continue;
                }
                let zi = l as usize - 1;
                if seg.zones[zi].outside {
                    continue;
                }
                let rgba = unpremultiply(c);
                let b = &mut buckets[zi];
                if b.get(last[zi]).is_some_and(|e| e.0 == rgba) {
                    b[last[zi]].1 += 1;
                } else if let Some(k) = b.iter().position(|e| e.0 == rgba) {
                    b[k].1 += 1;
                    last[zi] = k;
                } else {
                    b.push((rgba, 1));
                    last[zi] = b.len() - 1;
                }
            }
        }
    }
    seg.zones
        .iter()
        .zip(buckets)
        .map(|(z, b)| {
            let (rgba, n) = b.into_iter().max_by_key(|e| e.1)?;
            (!z.outside && n as f32 >= 0.35 * z.area as f32).then_some(rgba)
        })
        .collect()
}

/// Les couleurs des zones de l'image qui suit : chaque zone prend celle de la zone dont elle vient.
fn propagate(prev: &[Slot], links: &[Option<Link>]) -> Vec<Slot> {
    links
        .iter()
        .map(|l| match l {
            None => Slot::new_zone(),
            Some(l) => {
                let s = prev.get(l.from as usize - 1).copied().unwrap_or_else(Slot::new_zone);
                let mut slot = Slot { rgba: s.rgba, conf: s.conf.min(l.conf), why: if l.conf < s.conf { l.why } else { s.why } };
                // Des zones de couleurs différentes réunies : on ne sait pas laquelle garder.
                let differs = l.also.iter().any(|&o| prev.get(o as usize - 1).is_some_and(|p| p.rgba.is_some() && p.rgba != s.rgba));
                if differs {
                    slot.conf = slot.conf.min(0.4);
                    slot.why = Why::Merge;
                }
                slot
            }
        })
        .collect()
}

/// La couleur finale d'une zone qui a deux candidates (celle de la clé d'avant, `f`, et celle de la
/// clé d'après, `g`) : la plus sûre l'emporte.
fn combine(f: Option<&Slot>, g: Option<&Slot>, closer_to_f: bool) -> Slot {
    match (f, g) {
        (Some(a), None) => *a,
        (None, Some(b)) => *b,
        (None, None) => Slot::new_zone(),
        (Some(a), Some(b)) if a.rgba == b.rgba => Slot { rgba: a.rgba, conf: 1.0 - (1.0 - a.conf) * (1.0 - b.conf), why: if a.conf >= b.conf { a.why } else { b.why } },
        (Some(a), Some(b)) => {
            let (best, other) = if (a.conf - b.conf).abs() < 0.1 {
                if closer_to_f {
                    (a, b)
                } else {
                    (b, a)
                }
            } else if a.conf > b.conf {
                (a, b)
            } else {
                (b, a)
            };
            let conf = (best.conf - 0.6 * other.conf).max(0.0);
            Slot { rgba: best.rgba, conf, why: if other.conf > 0.3 { Why::Disagree } else { best.why } }
        }
    }
}

// ----------------------------------------------------------------------------- la demande

/// Une image à traiter : un dessin du calque de trait (ses images consécutives n'en font qu'une).
pub struct Cadre {
    /// Le dessin du calque de trait, avec sa version (pour le cache).
    pub cel: u64,
    pub version: u64,
    pub ligne: Raster,
    /// Si l'image est une clé : le dessin du calque de couleur.
    pub cle: Option<Raster>,
    /// L'image est à colorier (ni une clé, ni hors de la plage : celles-là ne servent qu'à relier
    /// les clés aux images à colorier).
    pub cible: bool,
}

/// Tout ce qu'il faut pour colorier, relevé sur le document au départ. Les images se suivent dans
/// le temps.
pub struct Demande {
    pub cadres: Vec<Cadre>,
    pub parametres: Parametres,
}

/// Un doute : une zone qu'on n'a pas pu colorier sûrement.
#[derive(Clone, Debug, PartialEq)]
pub struct Doute {
    /// Rang de l'image dans la demande.
    pub cadre: usize,
    pub zone: u16,
    pub centre: (f32, f32),
    pub aire: u32,
    pub raison: Raison,
    pub confiance: f32,
}

/// Une image coloriée : les couleurs des zones (dans l'ordre de `seg.zones`).
pub struct ImageColoree {
    pub cadre: usize,
    pub seg: Arc<Segmentation>,
    pub couleurs: Vec<Option<Rgba>>,
    pub doutes: Vec<Doute>,
}

/// Les chiffres d'un coloriage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bilan {
    pub images: usize,
    pub zones_coloriees: usize,
    pub zones_incertaines: usize,
}

/// Applique `f` à chaque élément, sur plusieurs fils ; les résultats dans l'ordre.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let threads = std::thread::available_parallelism().map_or(2, |p| p.get()).min(8).min(n);
    if threads <= 1 {
        return items.iter().map(&f).collect();
    }
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                let r = f(&items[i]);
                out.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    out.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().flatten().collect()
}

/// Images traitées ensemble (la découpe se fait en parallèle sur ce paquet).
const BATCH: usize = 16;

/// Colorie : `emit` reçoit chaque image à colorier dès que ses deux clés ont parlé. `progress`
/// reçoit (fait, total). Toute la suite s'arrête (erreur) dès que `cancel` passe à vrai.
pub fn colorier(
    d: &Demande,
    cache: &ZoneCache,
    conseiller: &dyn ConseillerCouleur,
    cancel: &AtomicBool,
    progress: &(dyn Fn(usize, usize) + Sync),
    emit: &mut dyn FnMut(ImageColoree),
) -> Result<Bilan, String> {
    let n = d.cadres.len();
    let p = d.parametres;
    let stop = || if cancel.load(Ordering::Relaxed) { Err("arrêté".to_string()) } else { Ok(()) };
    let keys: Vec<usize> = (0..n).filter(|&i| d.cadres[i].cle.is_some()).collect();
    if keys.is_empty() {
        return Err("aucune clé : colorie au moins une image sur le calque de couleur".into());
    }
    if !d.cadres.iter().any(|c| c.cible) {
        return Err("aucune image à colorier dans la plage".into());
    }
    let total = n * 2;
    let done = AtomicUsize::new(0);
    let tick = |k: usize| {
        let v = done.fetch_add(k, Ordering::Relaxed) + k;
        progress(v.min(total), total);
    };
    let seg = |i: usize| cache.get(d.cadres[i].cel, d.cadres[i].version, &d.cadres[i].ligne, &p.zones);
    let is_key = |i: usize| d.cadres[i].cle.is_some();

    // Les couleurs des clés.
    let key_slots: HashMap<usize, Vec<Slot>> = keys
        .iter()
        .map(|&k| {
            let s = seg(k);
            let colours = read_colours(&s, d.cadres[k].cle.as_ref().expect("clé"));
            (k, colours.into_iter().map(|rgba| Slot { rgba, conf: 1.0, why: Why::Sure }).collect())
        })
        .collect();

    // Les suites d'images entre deux clés qui se suivent, avant la première clé, après la
    // dernière. Chacune se fait d'un bloc (découpe, appariement, couleurs, aplats) : la mémoire ne
    // garde que ses images.
    let mut intervals: Vec<(usize, usize)> = Vec::new();
    if keys[0] > 0 {
        intervals.push((0, keys[0]));
    }
    for w in keys.windows(2) {
        if w[1] > w[0] + 1 {
            intervals.push((w[0], w[1]));
        }
    }
    let last_key = *keys.last().unwrap_or(&0);
    if last_key + 1 < n {
        intervals.push((last_key, n - 1));
    }
    let mut bilan = Bilan::default();
    for (lo, hi) in intervals {
        let (fwd_ok, bwd_ok) = (is_key(lo), is_key(hi));
        // 1. Les appariements dont on a besoin, par paquets : on découpe d'abord les images (en
        //    parallèle), puis on apparie. (i → i+1 : avant ; i+1 → i : arrière.)
        let mut forward: HashMap<usize, Vec<Option<Link>>> = HashMap::new();
        let mut backward: HashMap<usize, Vec<Option<Link>>> = HashMap::new();
        let mut start = lo;
        while start < hi {
            stop()?;
            let end = (start + BATCH).min(hi);
            let frames: Vec<usize> = (start..=end).collect();
            par_map(&frames, |&i| {
                seg(i);
            });
            tick(frames.len() - 1);
            let mut tasks: Vec<(usize, bool)> = Vec::new();
            for i in start..end {
                if fwd_ok && !is_key(i + 1) {
                    tasks.push((i, true));
                }
                if bwd_ok && !is_key(i) {
                    tasks.push((i, false));
                }
            }
            let results = par_map(&tasks, |&(i, fwd)| if fwd { match_zones(&seg(i), &seg(i + 1)) } else { match_zones(&seg(i + 1), &seg(i)) });
            for (&(i, fwd), links) in tasks.iter().zip(results) {
                if fwd {
                    forward.insert(i, links);
                } else {
                    backward.insert(i, links);
                }
            }
            start = end;
        }
        // 2. Vers l'avant depuis la clé d'avant, vers l'arrière depuis celle d'après.
        let mut ahead: Vec<Option<Vec<Slot>>> = vec![None; hi - lo + 1];
        let mut behind: Vec<Option<Vec<Slot>>> = vec![None; hi - lo + 1];
        if fwd_ok {
            let mut cur = key_slots[&lo].clone();
            for i in lo + 1..=hi {
                let Some(links) = forward.get(&(i - 1)).filter(|_| !is_key(i)) else { break };
                cur = propagate(&cur, links);
                ahead[i - lo] = Some(cur.clone());
            }
        }
        if bwd_ok {
            let mut cur = key_slots[&hi].clone();
            for i in (lo..hi).rev() {
                let Some(links) = backward.get(&i).filter(|_| !is_key(i)) else { break };
                cur = propagate(&cur, links);
                behind[i - lo] = Some(cur.clone());
            }
        }
        // 3. Les images à colorier : la clé la plus sûre l'emporte.
        for i in lo..=hi {
            if is_key(i) || !d.cadres[i].cible {
                continue;
            }
            stop()?;
            let s = seg(i);
            let closer_to_f = if fwd_ok && bwd_ok { i - lo <= hi - i } else { fwd_ok };
            let (couleurs, doutes) = decide(i, &s, ahead[i - lo].as_deref(), behind[i - lo].as_deref(), closer_to_f, &p, conseiller, &mut bilan);
            bilan.images += 1;
            emit(ImageColoree { cadre: i, seg: s, couleurs, doutes });
            tick(1);
        }
    }
    // Une clé à réécrire (aplats dans un autre calque) : ses couleurs, telles qu'on les lit.
    for &k in &keys {
        if !d.cadres[k].cible {
            continue;
        }
        stop()?;
        let s = seg(k);
        let couleurs: Vec<Option<Rgba>> = key_slots[&k].iter().map(|slot| slot.rgba).collect();
        bilan.zones_coloriees += couleurs.iter().flatten().count();
        bilan.images += 1;
        emit(ImageColoree { cadre: k, seg: s, couleurs, doutes: Vec::new() });
        tick(1);
    }
    progress(total, total);
    Ok(bilan)
}

/// Les couleurs finales des zones de l'image `i` (de la découpe `s`) d'après les candidates venues de
/// la clé d'avant (`ahead`) et de celle d'après (`behind`) ; le conseiller est consulté sur les zones
/// incertaines ; les doutes sont notés, et les zones incertaines laissées vides ou marquées.
#[allow(clippy::too_many_arguments)]
fn decide(
    i: usize,
    s: &Segmentation,
    ahead: Option<&[Slot]>,
    behind: Option<&[Slot]>,
    closer_to_f: bool,
    p: &Parametres,
    conseiller: &dyn ConseillerCouleur,
    bilan: &mut Bilan,
) -> (Vec<Option<Rgba>>, Vec<Doute>) {
    let mut couleurs: Vec<Option<Rgba>> = vec![None; s.zones.len()];
    let mut doutes = Vec::new();
    for z in s.inner() {
        let zi = z.id as usize - 1;
        let mut slot = combine(ahead.and_then(|v| v.get(zi)), behind.and_then(|v| v.get(zi)), closer_to_f);
        let doubtful = |slot: &Slot| slot.conf < p.seuil && (slot.rgba.is_some() || slot.why == Why::New);
        if doubtful(&slot) {
            let question = Question { cadre: i, segmentation: s, zone: z, proposition: slot.rgba, confiance: slot.conf };
            if let Some(c) = conseiller.conseille(&question).filter(|c| c.confiance >= p.seuil) {
                slot = Slot { rgba: Some(c.couleur), conf: c.confiance, why: Why::Advisor };
            }
        }
        if doubtful(&slot) {
            bilan.zones_incertaines += 1;
            if z.area >= AIRE_MINI_RAPPORT {
                doutes.push(Doute { cadre: i, zone: z.id, centre: z.centroid, aire: z.area, raison: slot.why, confiance: slot.conf });
                if let Incertain::Marquer(c) = p.incertain {
                    couleurs[zi] = Some([c[0], c[1], c[2], 255]);
                }
            }
        } else if let Some(c) = slot.rgba {
            couleurs[zi] = Some(c);
            bilan.zones_coloriees += 1;
        }
    }
    (couleurs, doutes)
}

// ------------------------------------------------------------------------------- les aplats

/// Les aplats de `couleurs` en pixels : chaque zone prend sa couleur, sous le trait compris.
pub fn fill_raster(seg: &Segmentation, couleurs: &[Option<Rgba>]) -> Raster {
    let mut out = Raster::new(seg.width, seg.height);
    let colours: Vec<Option<Color32>> = couleurs.iter().map(|c| c.map(|c| premultiply(c[0], c[1], c[2], c[3]))).collect();
    for ly in 0..seg.bh {
        let row = &seg.lab[ly * seg.bw..(ly + 1) * seg.bw];
        for (lx, &l) in row.iter().enumerate() {
            if l == 0 {
                continue;
            }
            if let Some(Some(c)) = colours.get(l as usize - 1) {
                let (x, y) = (seg.x0 + lx, seg.y0 + ly);
                let i = out.tile_index_of(x, y);
                out.tile_mut(i)[(y % TILE) * TILE + x % TILE] = *c;
            }
        }
    }
    out
}

/// Les aplats de `couleurs` en formes pleines vectorielles : un contour simplifié par zone, qui
/// passe d'un pixel sur les zones voisines (pas de liseré blanc entre deux aplats qui se touchent).
pub fn fill_vector(seg: &Segmentation, couleurs: &[Option<Rgba>]) -> VectorDrawing {
    let mut elements = Vec::new();
    for (zi, c) in couleurs.iter().enumerate() {
        let Some(c) = c else { continue };
        let id = zi as u16 + 1;
        let Some((mx, my, mw, mh, mask)) = seg.mask_of(id, 1) else { continue };
        let mut grown = mask.clone();
        for y in 0..mh {
            for x in 0..mw {
                let i = y * mw + x;
                if mask[i] {
                    continue;
                }
                let near = [(x > 0).then(|| i - 1), (x + 1 < mw).then(|| i + 1), (y > 0).then(|| i - mw), (y + 1 < mh).then(|| i + mw)];
                if near.into_iter().flatten().any(|q| mask[q]) {
                    // Pas dans le dehors : l'aplat ne dépasse pas du contour extérieur du trait.
                    let l = seg.label_at((mx + x) as i64, (my + y) as i64);
                    if l != 0 && !seg.zones[l as usize - 1].outside {
                        grown[i] = true;
                    }
                }
            }
        }
        let contours: Vec<Vec<[f32; 2]>> = trace_contours(&grown, mw, mh).into_iter().map(|lp| lp.into_iter().map(|p| [p[0] + mx as f32, p[1] + my as f32]).collect()).collect();
        if contours.is_empty() {
            continue;
        }
        elements.push(Element::Fill { contours, color: [c[0], c[1], c[2]], opacity: c[3] as f32 / 255.0 });
    }
    VectorDrawing { elements }
}
