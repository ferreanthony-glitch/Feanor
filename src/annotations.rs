//! Annotations de la vue Vignettes (Anthony, 2026-10-21) : un stylo (rouge d'origine) et sa gomme, pour
//! montrer, corriger, entourer, flécher sur la planche — **sans aucun effet sur le storyboard**.
//!
//! # Ce que c'est
//!
//! - Un trait d'annotation passe **par-dessus tout** ce que montre la planche (cases, barres, bandes de note,
//!   marges, fond) et peut déborder n'importe où. Il n'est ni dans un plan, ni dans le film, ni dans les
//!   vignettes, ni dans les exports vidéo ou images, ni au montage : les exports ne le lisent jamais (le cliché
//!   du projet, `exportrender.rs`, ne le copie pas). Seul l'export de la planche peut l'inclure, si on le coche.
//! - Chaque trait est **attaché à la case la plus proche** de son centre. Ses points sont gardés *relativement à
//!   cette case* (en largeurs de case, rayon compris), pas en points de la planche : quand les colonnes changent,
//!   que les cases se déplacent ou grossissent, le trait les suit (`Annotations::points_planche`).
//!   La case est désignée par la vignette (le numéro du plan, ou de la marque d'image) et, pour une case vide après
//!   la dernière vignette, par le nombre de cases qui la séparent de cette vignette (`Ancre::apres`) : quand une
//!   case vide devient une vignette, le trait reste dans la même case.
//! - **Si la case disparaît** (plan supprimé, plans réunis, marque retirée) : le trait reste **attaché à sa voisine
//!   la plus proche** (parmi les vignettes qui restent), à la même place sur la planche à l'instant de la
//!   disparition (`rattacher_manquantes`). Il n'est jamais perdu : seule la gomme ou « Effacer les annotations »
//!   l'enlève. Si l'ancienne mise en page manque (projet qui s'ouvre sur un fichier d'une autre séance), il se
//!   rattache à la vignette de même rang que celle d'avant, à son décalage d'origine.
//! - La **gomme d'annotation** n'enlève que des annotations (le trait entier que touche le cercle) ; elle ne touche
//!   ni aux dessins, ni aux notes de la planche.
//! - **Fichier** : `annotations.json`, à côté du manifeste, sans changer `VERSION` du projet (même principe que
//!   `personnages.json` : absent, il n'y a pas d'annotation ; illisible ou d'une version plus récente, il
//!   n'empêche pas d'ouvrir le projet et n'est jamais effacé, `project::write_sidecar`). Un projet partagé l'écrit
//!   avec la structure : les équipiers voient les mêmes annotations. Les annotations sont dans `Structure` :
//!   l'annulation les rétablit avec le reste.
//! - Afficher / cacher et « Effacer les annotations » : `annotationsui.rs`. Cacher n'est qu'un affichage (rien
//!   n'est effacé).

use eframe::egui::{Pos2, Rect};
use serde::{Deserialize, Serialize};

use crate::board::{self, BoardLayout};
use crate::document::Document;
use crate::vector::VPoint;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "annotations.json";
const FORMAT: &str = "shot-annotations";
const VERSION: u32 = 1;
/// Bornes d'un fichier abîmé : au-delà, c'est coupé.
pub const MAX_TRACES: usize = 5000;
pub const MAX_POINTS: usize = 3000;
/// Rouge d'origine du stylo, et sa taille (points de la planche, pour une case de largeur normale).
pub const COULEUR_PAR_DEFAUT: [u8; 3] = [220, 38, 38];
pub const TAILLE_PAR_DEFAUT: f32 = 5.0;
pub const TAILLE_MIN: f32 = 1.0;
pub const TAILLE_MAX: f32 = 60.0;

/// La case à laquelle un trait est attaché.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ancre {
    /// La vignette (le numéro du plan, ou de la marque d'image).
    #[serde(rename = "vignette")]
    pub card: u64,
    /// Nombre de cases entre cette vignette et la case visée : 0 pour la vignette elle-même, 1 pour la case qui la
    /// suit (une case vide, si c'est la dernière vignette)…
    #[serde(rename = "apres", default)]
    pub after: u32,
    /// Le rang de la case sur la planche au moment de l'attache (secours quand la vignette disparaît sans que
    /// l'ancienne mise en page soit connue).
    #[serde(rename = "rang", default)]
    pub rank: u32,
}

/// Un trait d'annotation : sa case, sa couleur, ses points relatifs à cette case.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub anchor: Ancre,
    pub color: [u8; 3],
    /// Points relatifs au coin haut gauche de l'image de la case, en largeurs d'image (rayon compris).
    pub points: Vec<VPoint>,
}

/// Les annotations d'un projet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotations {
    pub traces: Vec<Trace>,
}

// ------------------------------------------------------------------------------------ les cases

/// Les images de toutes les cases de la planche, dans l'ordre de lecture : les vignettes, puis les cases vides.
pub fn cases(layout: &BoardLayout) -> Vec<Rect> {
    layout.cards.iter().map(|c| c.thumb).chain(layout.empty.iter().copied()).collect()
}

/// L'image de la case désignée par `a` (son rang et son rectangle). Une case au-delà de la planche est la dernière.
pub fn case_de(layout: &BoardLayout, a: &Ancre) -> Option<(usize, Rect)> {
    let k = layout.cards.iter().position(|c| c.id == a.card)?;
    let all = cases(layout);
    let at = (k + a.after as usize).min(all.len().checked_sub(1)?);
    Some((at, all[at]))
}

/// La distance d'un point à un rectangle (0 dedans).
fn distance(r: Rect, p: Pos2) -> f32 {
    let dx = (r.left() - p.x).max(p.x - r.right()).max(0.0);
    let dy = (r.top() - p.y).max(p.y - r.bottom()).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// La case la plus proche du point `p` (planche) : son rang, son rectangle. En cas d'égalité, la première.
pub fn case_proche(layout: &BoardLayout, p: Pos2) -> Option<(usize, Rect)> {
    let all = cases(layout);
    all.iter().enumerate().map(|(i, r)| (i, *r, distance(*r, p))).min_by(|a, b| a.2.total_cmp(&b.2)).map(|(i, r, _)| (i, r))
}

/// L'ancre qui désigne la case de rang `rank`.
pub fn ancre_du_rang(layout: &BoardLayout, rank: usize) -> Option<Ancre> {
    let n = layout.cards.len();
    if n == 0 {
        return None;
    }
    let rank32 = rank.min(u32::MAX as usize) as u32;
    Some(if rank < n { Ancre { card: layout.cards[rank].id, after: 0, rank: rank32 } } else { Ancre { card: layout.cards[n - 1].id, after: (rank - (n - 1)).min(u32::MAX as usize) as u32, rank: rank32 } })
}

fn relatif(p: &VPoint, r: Rect) -> VPoint {
    let k = r.width().max(1e-3);
    VPoint { x: (p.x - r.left()) / k, y: (p.y - r.top()) / k, r: p.r / k }
}

fn absolu(p: &VPoint, r: Rect) -> VPoint {
    let k = r.width();
    VPoint { x: r.left() + p.x * k, y: r.top() + p.y * k, r: p.r * k }
}

fn centre(points: &[VPoint]) -> Option<Pos2> {
    let b = board::points_bounds(points);
    b.is_finite().then(|| b.center())
}

impl Annotations {
    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    /// Les points du trait `i` sur la planche (rien si sa vignette n'existe plus : voir `rattacher_manquantes`).
    pub fn points_planche(&self, layout: &BoardLayout, i: usize) -> Option<Vec<VPoint>> {
        let t = self.traces.get(i)?;
        let (_, r) = case_de(layout, &t.anchor)?;
        Some(t.points.iter().map(|p| absolu(p, r)).collect())
    }

    /// Ajoute un trait tracé sur la planche (`points` en coordonnées de la planche), attaché à la case la plus proche
    /// de son centre. Vrai si le trait est gardé.
    pub fn ajouter(&mut self, layout: &BoardLayout, points: &[VPoint], color: [u8; 3]) -> bool {
        if points.is_empty() || self.traces.len() >= MAX_TRACES || points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.r.is_finite())) {
            return false;
        }
        let Some(c) = centre(points) else { return false };
        let Some((rank, r)) = case_proche(layout, c) else { return false };
        let Some(anchor) = ancre_du_rang(layout, rank) else { return false };
        let points: Vec<VPoint> = reduire(points, MAX_POINTS).iter().map(|p| relatif(p, r)).collect();
        self.traces.push(Trace { anchor, color, points });
        true
    }

    /// Gomme : retire les traits que touche le cercle (`p`, `radius`, planche). Rend combien.
    pub fn effacer_en(&mut self, layout: &BoardLayout, p: Pos2, radius: f32) -> usize {
        let hit: Vec<usize> = (0..self.traces.len()).filter(|&i| self.points_planche(layout, i).is_some_and(|pts| board::touches(&pts, p, radius))).collect();
        for &i in hit.iter().rev() {
            self.traces.remove(i);
        }
        hit.len()
    }

    /// Un trait est-il sous le cercle (`p`, `radius`) ?
    pub fn touche(&self, layout: &BoardLayout, p: Pos2, radius: f32) -> bool {
        (0..self.traces.len()).any(|i| self.points_planche(layout, i).is_some_and(|pts| board::touches(&pts, p, radius)))
    }

    /// Les traits dont la vignette n'est plus sur la planche `apres` se rattachent à leur voisine la plus proche
    /// (voir le début du fichier). `avant` : la planche d'avant, d'où l'on sait où ils étaient. Rend combien de traits ont changé de case.
    pub fn rattacher_manquantes(&mut self, avant: Option<&BoardLayout>, apres: &BoardLayout) -> usize {
        if apres.cards.is_empty() {
            return 0;
        }
        let mut changes = 0;
        for t in &mut self.traces {
            if apres.cards.iter().any(|c| c.id == t.anchor.card) {
                continue;
            }
            // Là où le trait était sur l'ancienne planche, puis la vignette restante la plus proche de ce point.
            let was = avant.and_then(|b| case_de(b, &t.anchor).map(|(_, r)| (b, r)));
            let new = was.and_then(|(b, r)| {
                let pts: Vec<VPoint> = t.points.iter().map(|p| absolu(p, r)).collect();
                let c = centre(&pts)?;
                let near = b.cards.iter().filter(|k| apres.cards.iter().any(|a| a.id == k.id)).min_by(|x, y| distance(x.thumb, c).total_cmp(&distance(y.thumb, c)))?;
                let rank = apres.cards.iter().position(|a| a.id == near.id)?;
                // Les points se reportent sur la case voisine (même place sur l'ancienne planche), relativement à elle.
                let anchor = Ancre { card: near.id, after: 0, rank: rank as u32 };
                let points = pts.iter().map(|p| relatif(p, near.thumb)).collect::<Vec<_>>();
                Some((anchor, points))
            });
            match new {
                Some((anchor, points)) => {
                    t.anchor = anchor;
                    t.points = points;
                }
                None => {
                    // Pas d'ancienne planche : la vignette de même rang, au même décalage.
                    let rank = (t.anchor.rank as usize).min(apres.cards.len() - 1);
                    t.anchor = Ancre { card: apres.cards[rank].id, after: 0, rank: rank as u32 };
                }
            }
            changes += 1;
        }
        changes
    }

    /// Le rang des cases est mis à jour (pour que le secours de `rattacher_manquantes` reste proche de la réalité).
    /// Ne compte pas comme une modification du projet. Rend vrai si un rang a changé.
    pub fn noter_rangs(&mut self, layout: &BoardLayout) -> bool {
        let mut changed = false;
        for t in &mut self.traces {
            if let Some((rank, _)) = case_de(layout, &t.anchor) {
                if t.anchor.rank as usize != rank {
                    t.anchor.rank = rank as u32;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Ce que couvre l'ensemble des traits sur la planche (pour cadrer l'export).
    pub fn etendue(&self, layout: &BoardLayout) -> Option<Rect> {
        let mut out: Option<Rect> = None;
        for i in 0..self.traces.len() {
            if let Some(pts) = self.points_planche(layout, i) {
                let b = board::points_bounds(&pts);
                if b.is_finite() {
                    out = Some(out.map_or(b, |o| o.union(b)));
                }
            }
        }
        out
    }
}

/// Garde au plus `max` points d'un trait en prenant des points réguliers (le premier et le dernier restent).
pub fn reduire(points: &[VPoint], max: usize) -> Vec<VPoint> {
    if points.len() <= max.max(2) {
        return points.to_vec();
    }
    let max = max.max(2);
    (0..max).map(|k| points[(k * (points.len() - 1)) / (max - 1)]).collect()
}

/// Simplifie un trait en cours de tracé : retire les points à moins de `eps` de leur voisin gardé (le dernier reste).
pub fn epurer(points: &[VPoint], eps: f32) -> Vec<VPoint> {
    let mut out: Vec<VPoint> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_none_or(|l| (l.pos() - p.pos()).length() >= eps) {
            out.push(*p);
        }
    }
    if let (Some(last), Some(l)) = (points.last(), out.last().copied()) {
        if l != *last {
            out.push(*last);
        }
    }
    out
}

// ------------------------------------------------------------------------------------ le fichier

#[derive(Serialize, Deserialize)]
struct TraceFichier {
    ancre: Ancre,
    couleur: [u8; 3],
    points: Vec<[f32; 3]>,
}

#[derive(Serialize, Deserialize)]
struct Fichier {
    format: String,
    version: u32,
    traces: Vec<TraceFichier>,
}

fn arrondi(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
}

/// Le texte de `annotations.json` pour `doc` ; rien s'il n'y a pas d'annotation (le fichier part alors, s'il est
/// lisible : `write_sidecar`).
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.annotations.is_empty() {
        return Ok(None);
    }
    let traces = doc
        .annotations
        .traces
        .iter()
        .take(MAX_TRACES)
        .map(|t| TraceFichier { ancre: t.anchor, couleur: t.color, points: t.points.iter().take(MAX_POINTS).map(|p| [arrondi(p.x), arrondi(p.y), arrondi(p.r)]).collect() })
        .collect();
    let f = Fichier { format: FORMAT.into(), version: VERSION, traces };
    serde_json::to_vec_pretty(&f).map(Some).map_err(|e| e.to_string())
}

/// Les annotations lues dans le texte de `annotations.json`. Les traits sans point, ou dont un point n'est pas un
/// nombre, sont écartés ; rien de plus que `MAX_TRACES` traits de `MAX_POINTS` points.
pub(crate) fn parse(text: &str) -> Result<Annotations, String> {
    let f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier d'annotations".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    let traces = f
        .traces
        .into_iter()
        .filter_map(|t| {
            let points: Vec<VPoint> = t.points.into_iter().take(MAX_POINTS).filter(|p| p.iter().all(|v| v.is_finite())).map(|[x, y, r]| VPoint { x, y, r: r.clamp(1e-5, 50.0) }).collect();
            (!points.is_empty()).then_some(Trace { anchor: t.ancre, color: t.couleur, points })
        })
        .take(MAX_TRACES)
        .collect();
    Ok(Annotations { traces })
}

#[cfg(test)]
#[path = "annotations_tests.rs"]
mod tests;
