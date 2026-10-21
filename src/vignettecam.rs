//! Les mouvements de caméra dessinés dans les Vignettes (Anthony, 2026-10-21), sans interface.
//!
//! **Ctrl + glisser le cadre d'une case** : le cadre d'origine reste **A** (le début du mouvement) ; un nouveau cadre
//! **B** apparaît là où on le dépose. Les clés de caméra du plan sont posées (A à la première image, B à la dernière ;
//! le modèle est celui de la timeline et du montage, vitesse constante) ; en recommençant depuis le dernier cadre, on
//! ajoute **C**, **D**… Les clés se répartissent régulièrement sur la durée du plan quand elles l'étaient déjà
//! (`est_repartie`) ; des clés posées à la main ailleurs (montage, timeline) sont respectées : la nouvelle va à la
//! dernière image, et la dernière clé, si elle y était déjà, recule à mi-chemin de la précédente et de la fin.
//!
//! **La case grandit** pour montrer toute la zone que couvrent les cadres : on part du cadre A (une case) et de
//! l'englobant de tous les cadres sur la feuille, et la case prend le nombre entier de cases qui le contient
//! (`span` : 2 × 1 pour un panoramique horizontal d'une largeur de cadre, 1 × 2 pour un vertical, 2 × 2…), au plus
//! les colonnes de la planche en largeur et `SPAN_MAX_H` en hauteur. Rien n'est enregistré dans le projet pour cela :
//! la taille se déduit des clés (en supprimer au montage fait rétrécir la case).
//!
//! Dans la grande case, la feuille se voit à l'échelle du cadre A (un cadre A = une case), centrée sur l'englobant
//! (`Zone`) : le décor se dessine n'importe où dans cette zone, hors de la feuille aussi (toile infinie).

use eframe::egui::{vec2, Pos2, Rect, Vec2};

use crate::camera::{Camera, CameraKey};
use crate::document::Document;
use crate::view::View;

/// Hauteur d'une case de mouvement, en cases, au plus (sa largeur est bornée par les colonnes de la planche).
pub const SPAN_MAX_H: usize = 3;

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

/// Le plus petit rectangle de la feuille qui contient tous les cadres (rien de fini : `Rect::NOTHING`).
pub fn englobant(cadres: &[Cadre]) -> Rect {
    let mut r = Rect::NOTHING;
    for c in cadres {
        for p in c.coins {
            if p.x.is_finite() && p.y.is_finite() {
                r.extend_with(p);
            }
        }
    }
    r
}

/// La taille de la case du plan `panel` en cases entières (largeur, hauteur), s'il a un mouvement de caméra (deux clés
/// au moins) : [1, 1] quand tous les cadres tiennent dans le cadre A.
pub fn span_du_mouvement(doc: &Document, panel: usize, max_cols: usize) -> Option<[usize; 2]> {
    let cads = cadres(doc, panel);
    let a = cads.first()?;
    let u = englobant(&cads);
    if !u.is_finite() {
        return None;
    }
    let r = taille_cadre(doc, &a.cam);
    // (Un millième de cadre de tolérance : deux cadres côte à côte qui se touchent tiennent en deux cases, pas trois.)
    let cases = |len: f32, unit: f32| (((len / unit.max(1e-3)) - 1e-3).ceil().max(1.0)) as usize;
    Some([cases(u.width(), r.x).min(max_cols.max(1)), cases(u.height(), r.y).min(SPAN_MAX_H)])
}

/// La taille de la case du plan `panel` en cases entières : [1, 1] sans mouvement.
#[cfg(test)]
pub fn span(doc: &Document, panel: usize, max_cols: usize) -> [usize; 2] {
    span_du_mouvement(doc, panel, max_cols).unwrap_or([1, 1])
}

/// La part de la feuille que montre une grande case : centrée sur l'englobant des cadres, à l'échelle du cadre A.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Le centre, sur la feuille.
    pub centre: Pos2,
    /// Sa taille, sur la feuille.
    pub taille: Vec2,
    /// Points de la planche par unité de la feuille : un cadre A occupe une largeur de case.
    pub echelle: f32,
}

/// La zone de la case du plan `panel`, dont l'image mesure `case` sur la planche (`card_w` : la largeur d'une case).
/// Rien si le plan n'a pas de mouvement.
pub fn zone(doc: &Document, panel: usize, case: Vec2, card_w: f32) -> Option<Zone> {
    let cads = cadres(doc, panel);
    let a = cads.first()?;
    let u = englobant(&cads);
    if !u.is_finite() {
        return None;
    }
    let echelle = card_w / taille_cadre(doc, &a.cam).x.max(1e-3);
    Some(Zone { centre: u.center(), taille: case / echelle, echelle })
}

impl Zone {
    /// Un point de la feuille sur la planche, `case` étant l'image de la case.
    pub fn vers_planche(&self, case: Rect, p: Pos2) -> Pos2 {
        case.center() + (p - self.centre) * self.echelle
    }

    /// Le point de la feuille sous un point de la planche.
    #[cfg(test)]
    pub fn vers_feuille(&self, case: Rect, p: Pos2) -> Pos2 {
        self.centre + (p - case.center()) / self.echelle
    }

    /// La vue du dessin qui montre exactement cette zone dans `rect` (la grande case, à l'écran).
    pub fn vue(&self, rect: Rect, stage: Vec2) -> View {
        let zoom = rect.width() / self.taille.x.max(1e-3);
        View { zoom, rotation: 0.0, flip: false, pan: -(self.centre.to_vec2() - stage * 0.5) * zoom, persp: None }
    }
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

fn distance_segment(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance(p)
}

/// Ce que saisit un appui en `p`. `cadres` : les quatre coins de chaque cadre dans le repère de `p` (la planche) ;
/// `simple` : la case n'a pas encore de mouvement, son seul cadre (A) est celle de la vignette entière. `tol` : la
/// distance (même repère) à laquelle un coin ou un bord se saisit.
///
/// - case sans mouvement : un coin tiré fait un nouveau cadre de la taille changée, ailleurs dedans un nouveau cadre
///   déplacé ; A reste ;
/// - case avec mouvement : un coin tiré change la taille de **ce** cadre, son bord le déplace, et dedans le dernier
///   cadre un nouveau cadre naît (déplacé depuis lui) ; ailleurs, rien.
pub fn choisit(cadres: &[[Pos2; 4]], simple: bool, p: Pos2, tol: f32) -> Option<Choix> {
    if cadres.is_empty() {
        return None;
    }
    let coin_proche = |q: &[Pos2; 4]| q.iter().enumerate().map(|(i, c)| (i, c.distance(p))).filter(|(_, d)| *d <= tol).min_by(|a, b| a.1.total_cmp(&b.1));
    if simple {
        let q = &cadres[0];
        if let Some((c, _)) = coin_proche(q) {
            return Some(Choix { prise: Prise::NouveauZoom, rang: 0, coin: c });
        }
        return dans_quad(q, p).then_some(Choix { prise: Prise::Nouveau, rang: 0, coin: 0 });
    }
    // Les coins d'abord (le plus proche ; à égalité, le dernier cadre), puis les bords.
    let mut best: Option<(f32, usize, usize)> = None;
    for (r, q) in cadres.iter().enumerate() {
        if let Some((c, d)) = coin_proche(q) {
            if best.is_none_or(|(bd, _, _)| d <= bd) {
                best = Some((d, r, c));
            }
        }
    }
    if let Some((_, r, c)) = best {
        return Some(Choix { prise: Prise::Zoom(r), rang: r, coin: c });
    }
    let mut edge: Option<(f32, usize)> = None;
    for (r, q) in cadres.iter().enumerate() {
        let d = (0..4).map(|i| distance_segment(q[i], q[(i + 1) % 4], p)).fold(f32::MAX, f32::min);
        if d <= tol && edge.is_none_or(|(bd, _)| d <= bd) {
            edge = Some((d, r));
        }
    }
    if let Some((_, r)) = edge {
        return Some(Choix { prise: Prise::Deplace(r), rang: r, coin: 0 });
    }
    let last = cadres.len() - 1;
    dans_quad(&cadres[last], p).then_some(Choix { prise: Prise::Nouveau, rang: last, coin: 0 })
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
