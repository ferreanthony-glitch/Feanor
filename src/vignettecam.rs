//! Les mouvements de caméra dessinés dans les Vignettes (Anthony, 2026-10-21), sans interface.
//!
//! **Ctrl + Maj + glisser dans une case** (ou T tenue) : le cadre d'origine reste **A** (le début du mouvement) ; un nouveau cadre **B** apparaît
//! là où on le dépose. Les clés de caméra du plan sont posées (A à la première image, B à la dernière ; le modèle est celui de
//! la timeline et du montage, vitesse constante). Les clés se répartissent régulièrement sur la durée du plan quand elles
//! l'étaient déjà (`est_repartie`) ; des clés posées à la main ailleurs (montage, timeline) sont respectées : la nouvelle va à
//! la dernière image, et la dernière clé, si elle y était déjà, recule à mi-chemin de la précédente et de la fin.
//!
//! **Règles du geste** (Anthony, 2026-10-21, « une fois la caméra bougée, je ne peux plus la rebouger ») :
//! - sur une case sans mouvement, le glissé crée B (déplacé, ou zoomé quand il part d'un coin) ;
//! - sur une case avec des cadres, le glissé **ne crée jamais rien** : dedans un cadre il le déplace (le plus petit qui contient
//!   le point l'emporte : B dans A, dedans B on déplace B, dans A hors de B on déplace A), à un coin il change son zoom ;
//! - un cadre de plus (C, D…) se demande : Ctrl + Maj + Alt + glisser (il part du dernier cadre) ou « Ajouter un cadre » au clic droit ;
//!   « Retirer ce cadre » au clic droit sur un cadre.
//!
//! **L'empreinte** (`empreinte`) : une case qui a un mouvement montre ses cadres à **la même échelle** que les autres cases, sans
//! jamais rapetisser son contenu, et **à leur vraie place les uns par rapport aux autres** (Anthony, 2026-10-22 : « laisse la
//! caméra entre les lignes » : un cadre peut tomber à mi-rangée, à cheval sur deux rangées, à n'importe quel décalage, sans
//! arrondi à une rangée, une colonne ou un huitième de case ; seul un léger aimant, très près d'un alignement, aide le geste :
//! `aimante`). Une caméra tournée ou penchée se voit tournée ou penchée : un cadre est le quadrilatère de ses quatre coins.
//! - **l'enveloppe** est l'enveloppe convexe de tous les coins des cadres (et de la case A) : la zone de A, de B… et de l'espace
//!   entre eux. C'est la **zone de dessin** : on y dessine comme dans une case, le dessin vit sur la feuille du plan (en
//!   coordonnées de la feuille) et les cadres ne sont que des fenêtres dessus : déplacer un cadre ne déplace pas le dessin ;
//! - **une seule tuile** (une vignette composée sur un fil de fond, le dessin de la feuille à l'échelle des cases) couvre
//!   l'enveloppe ; hors de l'enveloppe, la tuile ne montre que le trait qui déborde (comme la marge d'une case) ;
//! - ce que la planche réserve aux autres cases suit l'enveloppe, rangée par rangée (`board.rs`) ;
//! - les cadres trop grands (plus qu'une rangée) sont dessinés réduits ; un décalage plus grand qu'une rangée de colonnes ou
//!   que trois rangées est resserré (les cadres restent proches de A).
//!
//! **Les liens** (`liens`) : seulement les côtés de l'enveloppe qui relient deux cadres (la silhouette), jamais une ligne qui
//! traverse l'intérieur ; une flèche pour un travelling pur.
//!
//! Rien n'est enregistré dans le projet pour cela : tout se déduit des clés (en supprimer au montage fait rétrécir l'empreinte).

use eframe::egui::{vec2, Pos2, Rect, Vec2};

use crate::camera::{Camera, CameraKey};
use crate::document::Document;
use crate::view::View;

/// Un cadre du mouvement : une clé de caméra du plan, vue sur la feuille.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cadre {
    /// Son rang parmi les clés (0 : A).
    pub rang: usize,
    /// L'image du plan où la clé est posée.
    pub image: usize,
    pub cam: Camera,
    /// Ses quatre coins sur la feuille : haut-gauche, haut-droite, bas-droite, bas-gauche.
    pub coins: [Pos2; 4],
    /// Le centre du cadre sur la feuille (le point visé).
    pub centre: Pos2,
}

/// La lettre d'un cadre : A, B, C…
pub fn lettre(rang: usize) -> char {
    (b'A' + rang.min(25) as u8) as char
}

fn cadre_de(doc: &Document, rang: usize, k: &CameraKey) -> Cadre {
    let [fw, fh] = doc.frame;
    Cadre { rang, image: k.frame, cam: k.cam, coins: k.cam.corners(doc.frame), centre: k.cam.to_stage(doc.frame, fw as f32 / 2.0, fh as f32 / 2.0) }
}

/// Les cadres du plan `panel` : une clé chacun, dans l'ordre des images. Rien s'il a moins de deux clés (une caméra qui
/// ne bouge pas n'est pas un mouvement).
pub fn cadres(doc: &Document, panel: usize) -> Vec<Cadre> {
    let Some(p) = doc.panels.get(panel) else { return Vec::new() };
    if p.camera.len() < 2 {
        return Vec::new();
    }
    p.camera.iter().enumerate().map(|(i, k)| cadre_de(doc, i, k)).collect()
}

/// La taille, sur la feuille, du cadre de la caméra `cam` (le cadre final divisé par le grossissement).
pub fn taille_cadre(doc: &Document, cam: &Camera) -> Vec2 {
    let m = cam.magnification(doc.frame).max(1e-3);
    vec2(doc.frame[0] as f32 / m, doc.frame[1] as f32 / m)
}

/// Les quatre coins sont-ils des points de la feuille ?
fn coins_finis(q: &[Pos2; 4]) -> bool {
    q.iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

/// Le rectangle de la feuille que couvre un cadre (ses quatre coins), s'ils sont finis.
fn rect_feuille(c: &Cadre) -> Option<Rect> {
    if !coins_finis(&c.coins) {
        return None;
    }
    let r = boite(&c.coins);
    (r.width() > 1e-3 && r.height() > 1e-3).then_some(r)
}

/// La boîte englobante de points.
pub fn boite(points: &[Pos2]) -> Rect {
    points.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)))
}

// ------------------------------------------------------------------ la géométrie des polygones convexes

/// Le sens du polygone : positif ou négatif selon qu'il tourne dans un sens ou dans l'autre (aire signée, doublée).
fn tour(poly: &[Pos2]) -> f32 {
    (0..poly.len()).map(|i| poly[i].x * poly[(i + 1) % poly.len()].y - poly[(i + 1) % poly.len()].x * poly[i].y).sum()
}

/// Le point est-il dans le polygone convexe (bords compris), quel que soit son sens ?
pub fn dans_polygone(poly: &[Pos2], p: Pos2) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let sens = tour(poly).signum();
    (0..poly.len()).all(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        sens * ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)) >= -1e-4
    })
}

/// La distance d'un point au bord d'un polygone (jamais négative, même dedans).
#[cfg_attr(not(test), allow(dead_code))]
pub fn distance_bord(poly: &[Pos2], p: Pos2) -> f32 {
    (0..poly.len()).map(|i| distance_segment(poly[i], poly[(i + 1) % poly.len()], p)).fold(f32::MAX, f32::min)
}

/// Deux polygones convexes se recouvrent-ils (d'une épaisseur plus grande que `eps`) ? Théorème de l'axe séparateur.
#[cfg(test)]
pub fn se_recouvrent(a: &[Pos2], b: &[Pos2], eps: f32) -> bool {
    for poly in [a, b] {
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let axe = vec2(q.y - p.y, p.x - q.x);
            let n = axe.length();
            if n < 1e-9 {
                continue;
            }
            let axe = axe / n;
            let proj = |s: &[Pos2]| s.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v.to_vec2().dot(axe)), hi.max(v.to_vec2().dot(axe))));
            let ((a0, a1), (b0, b1)) = (proj(a), proj(b));
            if a1 <= b0 + eps || b1 <= a0 + eps {
                return false;
            }
        }
    }
    true
}

/// L'intervalle en x du polygone convexe à la hauteur `y` (rien s'il n'y passe pas).
pub fn intervalle_a(poly: &[Pos2], y: f32) -> Option<(f32, f32)> {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.y - y) * (b.y - y) > 0.0 {
            continue;
        }
        if (b.y - a.y).abs() < 1e-9 {
            lo = lo.min(a.x.min(b.x));
            hi = hi.max(a.x.max(b.x));
        } else {
            let x = a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y);
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

/// L'enveloppe convexe de points marqués (chaîne monotone d'Andrew) : sans points alignés, les points confondus (à un millième
/// près) n'en font qu'un dont le marque réunit les leurs. Les points alignés écartés perdent leur marque : un côté de l'enveloppe
/// relie des sommets, pas tous les coins qu'il traverse.
fn enveloppe_marquee(points: &[(Pos2, u64)]) -> Vec<(Pos2, u64)> {
    let mut pts: Vec<(Pos2, u64)> = points.iter().copied().filter(|(p, _)| p.x.is_finite() && p.y.is_finite()).collect();
    pts.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
    let mut uniques: Vec<(Pos2, u64)> = Vec::with_capacity(pts.len());
    for (p, m) in pts {
        match uniques.last_mut() {
            Some(l) if (l.0 - p).length() < 1e-3 => l.1 |= m,
            _ => uniques.push((p, m)),
        }
    }
    if uniques.len() < 3 {
        return uniques;
    }
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    const EPS: f32 = 1e-2;
    let mut hull: Vec<(Pos2, u64)> = Vec::with_capacity(uniques.len() * 2);
    for &p in &uniques {
        while hull.len() >= 2 && cross(hull[hull.len() - 2].0, hull[hull.len() - 1].0, p.0) <= EPS {
            hull.pop();
        }
        hull.push(p);
    }
    let bas = hull.len() + 1;
    for &p in uniques.iter().rev().skip(1) {
        while hull.len() >= bas && cross(hull[hull.len() - 2].0, hull[hull.len() - 1].0, p.0) <= EPS {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}


/// L'enveloppe convexe de points.
pub fn enveloppe_convexe(points: &[Pos2]) -> Vec<Pos2> {
    enveloppe_marquee(&points.iter().map(|p| (*p, 1)).collect::<Vec<_>>()).into_iter().map(|(p, _)| p).collect()
}

// ------------------------------------------------------------------ l'empreinte

/// La place dont dispose une empreinte, en points de la planche.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// La largeur d'une case, et sa hauteur.
    pub card_w: f32,
    pub thumb_h: f32,
    /// La largeur d'une rangée de la planche : une empreinte n'est jamais plus large.
    pub row_w: f32,
    /// Ce dont une tuile peut dépasser la hauteur de la case : la bande de note (l'espace, la marge de débord et ses lignes). Un cadre est limité à
    /// la hauteur d'une rangée : la case et sa bande de note.
    pub total: f32,
    /// La hauteur la plus grande que prend toute l'empreinte (trois rangées).
    pub max_h: f32,
}

/// Un cadre du mouvement, tel qu'il se dessine sur la planche, **relatif à la case A** : x et y depuis le coin haut-gauche de A.
#[derive(Clone, Debug, PartialEq)]
pub struct CadreAff {
    pub rang: usize,
    /// Sa boîte englobante.
    pub rect: Rect,
    /// Ses quatre coins : haut-gauche, haut-droite, bas-droite, bas-gauche (tournés si la caméra l'est).
    pub coins: [Pos2; 4],
    /// Le cadre est plus grand que la limite : il est dessiné réduit (« limite de taille »).
    pub reduit: bool,
}

/// La tuile : le rectangle qui couvre l'enveloppe (relatif comme `CadreAff`), la part de la feuille qu'il montre, et
/// l'enveloppe en coordonnées de la feuille (hors d'elle, la tuile ne garde que le trait qui déborde).
#[derive(Clone, Debug, PartialEq)]
pub struct Tuile {
    pub rect: Rect,
    pub zone: Zone,
    pub masque: Vec<Pos2>,
}

/// L'empreinte d'une case qui a un mouvement de caméra.
#[derive(Clone, Debug, PartialEq)]
pub struct Empreinte {
    pub cadres: Vec<CadreAff>,
    /// L'enveloppe convexe de la case A et de tous les cadres : la zone de dessin.
    pub enveloppe: Vec<Pos2>,
    pub tuile: Tuile,
    /// Ce que la caméra balaie, en polygones convexes (relatifs à A comme `cadres`) : l'ouverture du passe-partout (`balayage`).
    pub balayage: Vec<Vec<Pos2>>,
    /// L'étendue, relative au coin haut-gauche de A : la boîte de l'enveloppe (x_min ≤ 0 ≤ largeur de la case ≤ x_max…).
    pub x_min: f32,
    pub x_max: f32,
    pub y_min: f32,
    pub y_max: f32,
}

/// L'empreinte du plan `panel` (rien s'il n'a pas de mouvement). Les cadres sont à l'échelle de la case : un cadre A fait une
/// largeur de case. Chaque cadre est à sa vraie place par rapport à A (même quand il tombe à mi-rangée) ; seuls un cadre
/// plus grand qu'une rangée (réduit) et un décalage trop grand pour la planche (resserré) s'écartent de la feuille.
pub fn empreinte(doc: &Document, panel: usize, p: &Place) -> Option<Empreinte> {
    let cads = cadres(doc, panel);
    let a = cads.first()?;
    let ra = rect_feuille(a)?;
    let echelle = p.card_w / taille_cadre(doc, &a.cam).x.max(1e-3);
    let (cw, ch) = (p.card_w, p.thumb_h);
    let thumb = Rect::from_min_size(Pos2::ZERO, vec2(cw, ch));
    let centre_thumb = thumb.center();
    // Pour chaque cadre : ses coins sur la feuille (A à l'identique pour un cadre qui ne voit plus la feuille), son centre, et le
    // facteur qui le réduit à la limite de taille.
    struct Source {
        rang: usize,
        coins: [Pos2; 4],
        centre: Pos2,
        f: f32,
    }
    let sources: Vec<Source> = cads
        .iter()
        .map(|c| {
            let (coins, cam) = if rect_feuille(c).is_some() { (c.coins, c.cam) } else { (a.coins, a.cam) };
            let centre = boite(&coins).center();
            // (La taille d'un cadre droit est celle du cadre, tourné ou non ; un cadre penché, sa boîte.)
            let taille = if cam.is_level() { taille_cadre(doc, &cam) * echelle } else { boite(&coins).size() * echelle };
            let f = ((p.thumb_h + p.total) / taille.y.max(1e-3)).min(p.row_w / taille.x.max(1e-3)).min(1.0);
            Source { rang: c.rang, coins, centre, f }
        })
        .collect();
    // (Un cadre d'une caméra droite qui ne tourne pas est exactement un rectangle : A est alors exactement la case.)
    let droit = |c: &Cadre| c.cam.is_level() && c.cam.angle == 0.0;
    let construit = |g: f32| -> Vec<[Pos2; 4]> {
        sources
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 && droit(a) && s.f >= 1.0 {
                    return coins_de(thumb);
                }
                let d = (s.centre - ra.center()) * echelle * g;
                s.coins.map(|c| centre_thumb + d + (c - s.centre) * echelle * s.f)
            })
            .collect()
    };
    let etendue = |q: &[[Pos2; 4]]| q.iter().flat_map(|q| q.iter().copied()).fold(thumb, |r, p| r.union(Rect::from_min_max(p, p)));
    // Pas plus large qu'une rangée, pas plus haut que trois : les décalages se resserrent (la largeur est convexe en `g` : la corde
    // de 0 à 1 la majore, une seule résolution suffit).
    let (e0, e1) = (etendue(&construit(0.0)), etendue(&construit(1.0)));
    let limite = |e0: f32, e1: f32, max: f32| if e1 <= max + 1e-3 { 1.0 } else if e1 - e0 > 1e-6 { ((max - e0) / (e1 - e0)).clamp(0.0, 1.0) } else { 0.0 };
    let g = limite(e0.width(), e1.width(), p.row_w).min(limite(e0.height(), e1.height(), p.max_h));
    let quads = construit(g);
    // Ce que la caméra balaie entre deux clés (une rotation, un zoom, un mouvement penché sortent de l'enveloppe des deux cadres).
    let pose = |coins: [Pos2; 4], f: f32| -> [Pos2; 4] {
        let centre = boite(&coins).center();
        let d = (centre - ra.center()) * echelle * g;
        coins.map(|c| centre_thumb + d + (c - centre) * echelle * f)
    };
    let (balayage, supplement) = balayage(doc, &cads, &sources.iter().map(|s| s.f).collect::<Vec<_>>(), &quads, pose);
    let mut points: Vec<Pos2> = quads.iter().flat_map(|q| q.iter().copied()).collect();
    points.extend(supplement);
    points.extend(coins_de(thumb));
    let enveloppe = enveloppe_convexe(&points);
    let boite_e = boite(&enveloppe);
    let cadres_aff: Vec<CadreAff> = quads.iter().zip(&sources).map(|(q, s)| CadreAff { rang: s.rang, rect: boite(q), coins: *q, reduit: s.f < 1.0 }).collect();
    // La tuile : sa place sur la feuille. (Ancrée sur A : un point de la planche est le point de la feuille qui lui correspond.)
    let sur_la_feuille = |q: Pos2| ra.center() + (q - centre_thumb) / echelle;
    let zone = Zone { centre: sur_la_feuille(boite_e.center()), taille: boite_e.size() / echelle, echelle };
    let masque: Vec<Pos2> = enveloppe.iter().map(|q| sur_la_feuille(*q)).collect();
    Some(Empreinte {
        cadres: cadres_aff,
        enveloppe,
        tuile: Tuile { rect: boite_e, zone, masque },
        balayage,
        x_min: boite_e.left(),
        x_max: boite_e.right(),
        y_min: boite_e.top(),
        y_max: boite_e.bottom(),
    })
}

/// Combien de cadres intermédiaires on pose entre deux clés qui ne se contentent pas de glisser (rotation, mouvement penché).
const ECHANTILLONS: usize = 12;

/// Deux caméras ne diffèrent-elles que par leur place et leur zoom (le balayage est alors celui des deux cadres) ?
fn glisse_seulement(a: &Camera, b: &Camera) -> bool {
    [(a.angle, b.angle), (a.dolly, b.dolly), (a.tilt, b.tilt), (a.pan, b.pan)].iter().all(|(p, q)| (p - q).abs() < 1e-4)
}

/// L'enveloppe convexe de plusieurs cadres.
fn coque_de(quads: &[[Pos2; 4]]) -> Vec<Pos2> {
    enveloppe_convexe(&quads.iter().flat_map(|q| q.iter().copied()).collect::<Vec<_>>())
}

/// Ce que balaie le cadre de la caméra de clé en clé, en polygones convexes (relatifs à A, comme `quads`) : une seule pièce entre deux clés
/// qui se contentent de glisser et de zoomer (l'enveloppe des deux cadres est exactement ce qui est balayé quand la taille ne change pas) ;
/// sinon `ECHANTILLONS` cadres intermédiaires, par groupes de trois (la rotation ou le penchement ne s'arrondit pas dans l'enveloppe des
/// deux cadres). Rend aussi les coins des cadres intermédiaires qui sortent de l'enveloppe des clés (pour agrandir l'enveloppe). Rien de tout
/// cela ne dépend du temps : le calcul se refait quand la caméra change.
fn balayage(doc: &Document, cads: &[Cadre], facteurs: &[f32], quads: &[[Pos2; 4]], pose: impl Fn([Pos2; 4], f32) -> [Pos2; 4]) -> (Vec<Vec<Pos2>>, Vec<Pos2>) {
    let (mut pieces, mut supplement) = (Vec::new(), Vec::new());
    for i in 0..quads.len().saturating_sub(1) {
        let (a, b) = (&cads[i], &cads[i + 1]);
        if glisse_seulement(&a.cam, &b.cam) || rect_feuille(a).is_none() || rect_feuille(b).is_none() {
            pieces.push(coque_de(&[quads[i], quads[i + 1]]));
            continue;
        }
        let mut suite = vec![quads[i]];
        for j in 1..ECHANTILLONS {
            let t = j as f32 / ECHANTILLONS as f32;
            let coins = Camera::lerp(&a.cam, &b.cam, t).corners(doc.frame);
            if !coins_finis(&coins) {
                continue;
            }
            let q = pose(coins, facteurs[i] + (facteurs[i + 1] - facteurs[i]) * t);
            supplement.extend(q);
            suite.push(q);
        }
        suite.push(quads[i + 1]);
        let mut debut = 0;
        while debut + 1 < suite.len() {
            let fin = (debut + 2).min(suite.len() - 1);
            pieces.push(coque_de(&suite[debut..=fin]));
            debut = fin;
        }
    }
    (pieces.into_iter().filter(|p| p.len() >= 3).collect(), supplement)
}

/// Comment relier deux cadres qui se suivent (en bleu, fin).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lien {
    /// Une flèche fine du centre d'un cadre à celui du suivant (A vers B, B vers C…) : elle suit le mouvement, droite et discrète.
    Fleche { de: Pos2, vers: Pos2 },
    /// Un côté de l'enveloppe qui relie deux cadres différents : un bout de la silhouette.
    Bord(Pos2, Pos2),
}

/// L'opacité (0 à 255) du trait d'une flèche de mouvement : discrète.
pub const FLECHE_ALPHA: u8 = 170;

/// Les deux bouts du petit chevron d'une flèche qui arrive en `vers` en allant dans la direction `u` (unitaire), de `taille` de long.
pub fn chevron(vers: Pos2, u: Vec2, taille: f32) -> [Pos2; 2] {
    let tourne = |a: f32| vec2(u.x * a.cos() - u.y * a.sin(), u.x * a.sin() + u.y * a.cos());
    [vers - tourne(0.45) * taille, vers - tourne(-0.45) * taille]
}

/// Les liens entre les cadres (`quads` : leurs quatre coins, tous dans le même repère ; `thumb` : la case A, qui fait partie de
/// l'enveloppe). Les côtés de l'enveloppe qui relient deux cadres différents (Anthony, 2026-10-22 : pas de ligne qui traverse
/// l'intérieur : la silhouette seule), plus une flèche fine d'un cadre au suivant (2026-11-06 : A vers B, B vers C, quel que soit le
/// mouvement : un travelling, une diagonale, un zoom qui se déplace).
pub fn liens(quads: &[[Pos2; 4]], thumb: Rect) -> Vec<Lien> {
    let mut out = Vec::new();
    // Entre deux centres, raccourcie à chaque bout pour ne pas s'appuyer sur la lettre ni sur le trait des cadres. Un cadre qui en
    // confond un autre, ou un zoom sur place (rien à suivre), n'a pas de flèche.
    let distincts = cadres_distincts(quads);
    let centres: Vec<Pos2> = quads.iter().zip(&distincts).filter(|(_, d)| **d).map(|(q, _)| boite(q).center()).collect();
    for w in centres.windows(2) {
        let d = w[1] - w[0];
        let long = d.length();
        if long < 6.0 {
            continue;
        }
        let u = d / long;
        let rogne = (long * 0.12).clamp(2.0, 9.0);
        out.push(Lien::Fleche { de: w[0] + u * rogne, vers: w[1] - u * rogne });
    }
    // L'enveloppe de tous les coins (la case A comprise, elle est le cadre A) : chaque côté dont les deux bouts n'appartiennent pas
    // à un même cadre relie deux cadres.
    let mut points: Vec<(Pos2, u64)> = coins_de(thumb).iter().map(|p| (*p, 1)).collect();
    for (i, q) in quads.iter().enumerate() {
        points.extend(q.iter().map(|p| (*p, 1_u64 << i.min(62))));
    }
    let h = enveloppe_marquee(&points);
    for k in 0..h.len() {
        let (p, q) = (h[k], h[(k + 1) % h.len()]);
        if h.len() >= 3 && p.1 & q.1 == 0 {
            out.push(Lien::Bord(p.0, q.0));
        }
    }
    out
}

/// Les cadres qu'on dessine : un cadre qui se confond avec un cadre déjà dessiné (ses quatre coins à moins de `SEUIL_CONFONDU` près : le
/// tremblé d'une caméra à la main, des dizaines de clés presque au même endroit) ne se redessine pas, ni sa lettre : elles
/// s'empilaient en un petit carré bleu (Anthony, 2026-10-22).
pub fn cadres_distincts(quads: &[[Pos2; 4]]) -> Vec<bool> {
    let mut montres: Vec<usize> = Vec::new();
    quads
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let confondu = montres.iter().any(|&j| (0..4).all(|c| quads[j][c].distance(q[c]) < SEUIL_CONFONDU));
            if !confondu {
                montres.push(i);
            }
            !confondu
        })
        .collect()
}

/// Deux cadres dont les coins sont à moins de ce nombre de points de la planche l'un de l'autre n'en font qu'un à l'écran.
pub const SEUIL_CONFONDU: f32 = 2.5;

/// La part de la feuille que montre une tuile : centrée sur `centre`, à l'échelle des cases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Le centre, sur la feuille.
    pub centre: Pos2,
    /// Sa taille, sur la feuille.
    pub taille: Vec2,
    /// Points de la planche par unité de la feuille : un cadre A occupe une largeur de case.
    pub echelle: f32,
}

impl Zone {
    /// La vue du dessin qui montre exactement cette zone dans `rect` (la tuile, à l'écran).
    pub fn vue(&self, rect: Rect, stage: Vec2) -> View {
        let zoom = rect.width() / self.taille.x.max(1e-3);
        View { zoom, rotation: 0.0, flip: false, pan: -(self.centre.to_vec2() - stage * 0.5) * zoom, persp: None }
    }
}

// ------------------------------------------------------------------ l'aimant

/// (La distance, en points d'écran, à laquelle l'aimant d'un alignement prend un cadre : très près seulement, 6 d'origine
/// (Anthony, 2026-10-22 : « garde l'aimant, mais seulement quand c'est très proche d'un alignement avec les lignes et les colonnes »).
/// C'est un réglage, Préférences > Vignettes, 0 : pas d'aimant : `vignettesreglages::ReglagesVignettes::aimant`.)

/// Les alignements qui attirent un cadre qu'on déplace, relatifs au coin haut-gauche de A, en points de la planche : des
/// colonnes (x) et des rangées (y) de la planche, les bords et le centre de A et des autres cadres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grille {
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
}

/// Le décalage qui rapproche la valeur la plus proche d'une cible de celle-ci, si elle est à `seuil` près ; sinon rien. L'aimant
/// est **doux** : l'écart `d` à la cible devient `d × |d| / seuil` (la valeur colle à la cible tout près d'elle, et rejoint le
/// pointeur au bord du seuil), si bien que le cadre ne saute jamais en entrant dans l'aimant ni en en sortant.
fn rapproche(valeurs: [f32; 3], cibles: &[f32], seuil: f32) -> f32 {
    let mut meilleur: Option<f32> = None;
    for v in valeurs {
        for c in cibles {
            let d = c - v;
            if d.abs() <= seuil && meilleur.is_none_or(|m| d.abs() < m.abs()) {
                meilleur = Some(d);
            }
        }
    }
    meilleur.map_or(0.0, |d| d * (1.0 - d.abs() / seuil.max(1e-6)))
}

/// Le déplacement `delta` d'un cadre dont la boîte de départ est `rect` (points de la planche), corrigé pour que son bord ou son
/// centre tombe sur un alignement de `grille` s'il en est à `seuil` près, chaque axe pour lui. Partout ailleurs, `delta` tel quel :
/// aucun arrondi.
pub fn aimante(rect: Rect, delta: Vec2, grille: &Grille, seuil: f32) -> Vec2 {
    let r = rect.translate(delta);
    vec2(delta.x + rapproche([r.left(), r.center().x, r.right()], &grille.xs, seuil), delta.y + rapproche([r.top(), r.center().y, r.bottom()], &grille.ys, seuil))
}

// ------------------------------------------------------------------ les clés
/// Les images de `n` clés réparties régulièrement sur un plan de `len` images : la première au début, la dernière à
/// la fin.
pub fn repartition(n: usize, len: usize) -> Vec<usize> {
    match n {
        0 => Vec::new(),
        1 => vec![0],
        _ => (0..n).map(|k| (k as f32 * len.saturating_sub(1) as f32 / (n - 1) as f32).round() as usize).collect(),
    }
}

/// Les clés sont-elles réparties comme `repartition` le fait (celles que ce geste pose) ?
pub fn est_repartie(keys: &[CameraKey], len: usize) -> bool {
    keys.len() >= 2 && repartition(keys.len(), len).iter().zip(keys).all(|(f, k)| *f == k.frame)
}

/// Sur combien d'images se répartissent les cadres d'un mouvement : la durée de la **première case** du plan (`span` : jusqu'à sa
/// deuxième case, une marque d'image) quand elle peut porter deux clés, sinon tout le plan (`total`). Le dernier cadre tombe donc à la fin
/// de la case qui porte le mouvement : les cases qui la suivent dans le plan, en continuité, montrent le dernier cadre (la caméra tient sa
/// dernière clé), à l'écran des Vignettes comme à la lecture.
pub fn etendue_du_mouvement(span: usize, total: usize) -> usize {
    if span >= 2 {
        span.min(total)
    } else {
        total
    }
}

/// Les clés `keys`, posées d'ordinaire sur tout le plan (`total` images : les clés que ce geste posait avant que le mouvement ne tienne dans
/// la première case), rangées sur les `len` images de la première case quand elles étaient réparties régulièrement sur tout le plan. Des clés
/// posées à la main, ou déjà rangées, restent comme elles sont.
pub fn recale(keys: &[CameraKey], len: usize, total: usize) -> Vec<CameraKey> {
    if len >= total || len < keys.len() || !est_repartie(keys, total) || est_repartie(keys, len) {
        return keys.to_vec();
    }
    let mut out = keys.to_vec();
    for (k, f) in out.iter_mut().zip(repartition(keys.len(), len)) {
        k.frame = f;
    }
    out
}

/// Les clés du plan une fois un nouveau cadre `nouveau` ajouté après le dernier. `a` : la caméra du début quand le plan
/// n'a pas encore de mouvement (sa clé, ou la caméra qu'il montre). Rien si le plan est trop court pour porter une clé
/// de plus (une image par clé au moins).
pub fn avec_nouveau_cadre(keys: &[CameraKey], a: Camera, nouveau: Camera, len: usize) -> Option<Vec<CameraKey>> {
    if len < 2 {
        return None;
    }
    if keys.len() < 2 {
        return Some(vec![CameraKey::new(0, a), CameraKey::new(len - 1, nouveau)]);
    }
    let mut out = keys.to_vec();
    if est_repartie(keys, len) {
        if len < keys.len() + 1 {
            return None;
        }
        let frames = repartition(keys.len() + 1, len);
        for (k, f) in out.iter_mut().zip(&frames) {
            k.frame = *f;
        }
        out.push(CameraKey::new(frames[keys.len()], nouveau));
        return Some(out);
    }
    // Des clés posées à la main : elles restent où elles sont, la nouvelle va à la dernière image.
    let last = len - 1;
    let n = out.len();
    if out[n - 1].frame >= last {
        let prev = out[n - 2].frame;
        let mid = prev + (last - prev) / 2;
        if mid <= prev || mid >= last {
            return None;
        }
        out[n - 1].frame = mid;
    }
    out.push(CameraKey::new(last, nouveau));
    Some(out)
}

/// Les clés du plan une fois le cadre `rang` retiré. Retirer B d'un mouvement à deux cadres enlève le mouvement : il ne reste
/// qu'une clé. Des clés réparties régulièrement le restent. Rien si le cadre n'existe pas.
pub fn sans_le_cadre(keys: &[CameraKey], rang: usize, len: usize) -> Option<Vec<CameraKey>> {
    if rang >= keys.len() {
        return None;
    }
    let repartie = est_repartie(keys, len);
    let mut out = keys.to_vec();
    out.remove(rang);
    if repartie && out.len() >= 2 {
        let frames = repartition(out.len(), len);
        for (k, f) in out.iter_mut().zip(frames) {
            k.frame = f;
        }
    }
    Some(out)
}

/// Le cadre qu'ajoute « Ajouter un cadre » : celui de `base` décalé d'une demi-largeur de cadre vers la droite (vers la gauche
/// s'il n'y a plus de place), qu'on prend ensuite pour le glisser où l'on veut.
pub fn cadre_suivant(frame: [usize; 2], base: Camera, taille: Vec2) -> Camera {
    let droite = base.shifted(taille.x * 0.5, 0.0).limited(frame);
    if droite != base.limited(frame) {
        droite
    } else {
        base.shifted(-taille.x * 0.5, 0.0).limited(frame)
    }
}

// ------------------------------------------------------------------ le geste

/// Ce que le geste fait au moment de l'appui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prise {
    /// Un nouveau cadre, déplacé depuis le dernier.
    Nouveau,
    /// Un nouveau cadre, de la taille du dernier changée (coin tiré).
    NouveauZoom,
    /// Le cadre de rang `n` est déplacé.
    Deplace(usize),
    /// Le cadre de rang `n` est agrandi ou resserré (coin tiré).
    Zoom(usize),
}

/// Ce que l'appui a saisi : la prise, le cadre visé et, pour un zoom, son coin (0 à 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choix {
    pub prise: Prise,
    pub rang: usize,
    pub coin: usize,
}

fn dans_quad(q: &[Pos2; 4], p: Pos2) -> bool {
    let mut signe = 0.0_f32;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let c = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if c.abs() < 1e-6 {
            continue;
        }
        if signe == 0.0 {
            signe = c.signum();
        } else if signe != c.signum() {
            return false;
        }
    }
    true
}

/// L'aire d'un quadrilatère (formule des lacets).
fn aire(q: &[Pos2; 4]) -> f32 {
    (0..4).map(|i| q[i].x * q[(i + 1) % 4].y - q[(i + 1) % 4].x * q[i].y).sum::<f32>().abs() * 0.5
}

/// La distance d'un point à un quadrilatère : zéro dedans.
fn distance_quad(q: &[Pos2; 4], p: Pos2) -> f32 {
    if dans_quad(q, p) {
        0.0
    } else {
        (0..4).map(|i| distance_segment(q[i], q[(i + 1) % 4], p)).fold(f32::MAX, f32::min)
    }
}

pub(crate) fn distance_segment(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance(p)
}

/// Ce que saisit un appui en `p`. `cadres` : les quatre coins de chaque cadre dans le repère de `p` (la planche) ;
/// `simple` : la case n'a pas encore de mouvement, son seul cadre (A) est celle de la vignette entière. `tol` : la
/// distance (même repère) à laquelle un coin se saisit. `ajoute` : Alt est tenue, on veut un cadre de plus.
///
/// Un appui saisit **toujours** quelque chose : le glissé d'une case, de son empreinte ou de sa marge est un geste de caméra,
/// jamais un déplacement de plan.
/// - case sans mouvement : un coin tiré fait un nouveau cadre de la taille changée, ailleurs un nouveau cadre déplacé ; A reste ;
/// - case avec mouvement : un coin tiré change la taille de **ce** cadre, dedans un cadre on le déplace (le plus petit qui contient
///   le point), hors de tous on déplace le plus proche. Rien ne naît : avec `ajoute`, un nouveau cadre part du dernier
///   (déplacé, ou zoomé quand l'appui est à un de ses coins).
pub fn choisit(cadres: &[[Pos2; 4]], simple: bool, p: Pos2, tol: f32, ajoute: bool) -> Option<Choix> {
    if cadres.is_empty() {
        return None;
    }
    let coin_proche = |q: &[Pos2; 4]| q.iter().enumerate().map(|(i, c)| (i, c.distance(p))).filter(|(_, d)| *d <= tol).min_by(|a, b| a.1.total_cmp(&b.1));
    if simple {
        let q = &cadres[0];
        if let Some((c, _)) = coin_proche(q) {
            return Some(Choix { prise: Prise::NouveauZoom, rang: 0, coin: c });
        }
        return Some(Choix { prise: Prise::Nouveau, rang: 0, coin: 0 });
    }
    let last = cadres.len() - 1;
    if ajoute {
        return Some(match coin_proche(&cadres[last]) {
            Some((c, _)) => Choix { prise: Prise::NouveauZoom, rang: last, coin: c },
            None => Choix { prise: Prise::Nouveau, rang: last, coin: 0 },
        });
    }
    // Les cadres en jeu : ceux qui contiennent le point ou dont un coin est à portée. Le plus petit l'emporte, à aire égale le dernier :
    // dans B, près d'un coin de A (B dans A, ou B recouvrant le coin de A), c'est B qu'on prend. Dans ce cadre, un coin à portée change
    // sa taille, sinon on le déplace. (Les coins d'un cadre plus grand ne passent jamais devant l'intérieur d'un plus petit.)
    let mut pris: Option<(f32, usize)> = None;
    for (r, q) in cadres.iter().enumerate() {
        if coin_proche(q).is_some() || dans_quad(q, p) {
            let a = aire(q);
            // (À un millième près, deux cadres de même taille ont la même aire : le dernier l'emporte.)
            if pris.is_none_or(|(ba, _)| a <= ba * 1.001) {
                pris = Some((a, r));
            }
        }
    }
    if let Some((_, mut r)) = pris {
        // A est la case : un coin de A tiré ne la rétrécit pas (Anthony, 2026-10-22 : « lors d'un zoom, c'est le cadre A qui rétrécit »). Quand le
        // point est aussi dans un autre cadre (ou près de son coin) (B qui entoure A après un zoom arrière, dont le coin est tout près de celui
        // de A), c'est ce cadre-là qu'on prend, le plus petit des autres. Le coin de A reste saisissable quand rien d'autre n'est là.
        if r == 0 && coin_proche(&cadres[0]).is_some() {
            let autre = (1..cadres.len()).filter(|&i| dans_quad(&cadres[i], p) || coin_proche(&cadres[i]).is_some()).min_by(|&i, &j| aire(&cadres[i]).total_cmp(&aire(&cadres[j])).then(j.cmp(&i)));
            if let Some(o) = autre {
                r = o;
            }
        }
        return Some(match coin_proche(&cadres[r]) {
            Some((c, _)) => Choix { prise: Prise::Zoom(r), rang: r, coin: c },
            None => Choix { prise: Prise::Deplace(r), rang: r, coin: 0 },
        });
    }    // Hors de tous (l'empreinte entre les cadres, la marge) : le plus proche.
    let (r, _) = cadres.iter().enumerate().map(|(r, q)| (r, distance_quad(q, p))).min_by(|a, b| a.1.total_cmp(&b.1))?;
    Some(Choix { prise: Prise::Deplace(r), rang: r, coin: 0 })
}

/// Le cadre qu'un appui en `p` vise, pour le menu du clic droit : celui qui contient le point (le plus petit), sinon rien.
pub fn cadre_sous(cadres: &[[Pos2; 4]], p: Pos2) -> Option<usize> {
    cadres.iter().enumerate().filter(|(_, q)| dans_quad(q, p)).min_by(|a, b| aire(a.1).total_cmp(&aire(b.1)).then(b.0.cmp(&a.0))).map(|(r, _)| r)
}

/// La caméra d'un cadre après le geste : déplacé de `delta` (feuille), ou de taille changée en tirant le coin `coin` (la
/// feuille) autour de `centre`. Une caméra tournée reste dans les limites où elle voit encore la feuille.
pub fn camera_geste(base: Camera, prise: Prise, delta: Vec2, coin: Pos2, centre: Pos2, frame: [usize; 2]) -> Camera {
    let cam = match prise {
        Prise::Nouveau | Prise::Deplace(_) => base.shifted(delta.x, delta.y),
        Prise::NouveauZoom | Prise::Zoom(_) => {
            // Éloigner le coin du centre élargit le cadre (moins de zoom), comme avec l'outil Caméra.
            let ratio = coin.distance(centre).max(1.0) / (coin + delta).distance(centre).max(1.0);
            Camera { zoom: (base.zoom * ratio).clamp(0.05, 50.0), ..base }
        }
    };
    cam.limited(frame)
}

/// Les clés du plan après le geste, depuis celles d'avant le geste (`keys0`) : jamais d'accumulation d'une image à l'autre.
#[allow(clippy::too_many_arguments)]
pub fn applique(frame: [usize; 2], keys0: &[CameraKey], a: Camera, prise: Prise, delta: Vec2, coin: Pos2, centre: Pos2, len: usize) -> Option<Vec<CameraKey>> {
    match prise {
        Prise::Deplace(i) | Prise::Zoom(i) => {
            let k = keys0.get(i)?;
            let mut out = keys0.to_vec();
            out[i].cam = camera_geste(k.cam, prise, delta, coin, centre, frame);
            Some(out)
        }
        Prise::Nouveau | Prise::NouveauZoom => {
            let base = if keys0.len() >= 2 { keys0[keys0.len() - 1].cam } else { a };
            avec_nouveau_cadre(keys0, a, camera_geste(base, prise, delta, coin, centre, frame), len)
        }
    }
}

/// Le nom de l'étape d'annulation d'un geste ; `avant` : le nombre de cadres avant lui.
pub fn nom_etape(prise: Prise, avant: usize) -> String {
    match prise {
        Prise::Nouveau | Prise::NouveauZoom if avant < 2 => "Mouvement de caméra".into(),
        Prise::Nouveau | Prise::NouveauZoom => format!("Cadre {}", lettre(avant)),
        Prise::Deplace(i) => format!("Cadre {}", lettre(i)),
        Prise::Zoom(i) => format!("Zoom du cadre {}", lettre(i)),
    }
}

/// Le coin `c` (0 à 3) du cadre de la caméra `cam`, sur la feuille.
pub fn coin_de(frame: [usize; 2], cam: &Camera, c: usize) -> Pos2 {
    cam.corners(frame)[c.min(3)]
}

/// Les quatre coins d'un rectangle, dans l'ordre des cadres (haut-gauche, haut-droite, bas-droite, bas-gauche).
pub fn coins_de(r: Rect) -> [Pos2; 4] {
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
}

/// Le centre d'un cadre sur la feuille (le point visé).
pub fn centre_de(frame: [usize; 2], cam: &Camera) -> Pos2 {
    cam.to_stage(frame, frame[0] as f32 / 2.0, frame[1] as f32 / 2.0)
}

#[cfg(test)]
#[path = "vignettecam_tests.rs"]
mod tests;
