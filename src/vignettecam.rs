//! Les mouvements de caméra dessinés dans les Vignettes (Anthony, 2026-10-21), sans interface.
//!
//! **Ctrl + glisser dans une case** : le cadre d'origine reste **A** (le début du mouvement) ; un nouveau cadre **B** apparaît
//! là où on le dépose. Les clés de caméra du plan sont posées (A à la première image, B à la dernière ; le modèle est celui de
//! la timeline et du montage, vitesse constante). Les clés se répartissent régulièrement sur la durée du plan quand elles
//! l'étaient déjà (`est_repartie`) ; des clés posées à la main ailleurs (montage, timeline) sont respectées : la nouvelle va à
//! la dernière image, et la dernière clé, si elle y était déjà, recule à mi-chemin de la précédente et de la fin.
//!
//! **Règles du geste** (Anthony, 2026-10-21, « une fois la caméra bougée, je ne peux plus la rebouger ») :
//! - sur une case sans mouvement, le glissé crée B (déplacé, ou zoomé quand il part d'un coin) ;
//! - sur une case avec des cadres, le glissé **ne crée jamais rien** : dedans un cadre il le déplace (le plus petit qui contient
//!   le point l'emporte : B dans A, dedans B on déplace B, dans A hors de B on déplace A), à un coin il change son zoom ;
//! - un cadre de plus (C, D…) se demande : Ctrl + Alt + glisser (il part du dernier cadre) ou « Ajouter un cadre » au clic droit ;
//!   « Retirer ce cadre » au clic droit sur un cadre.
//!
//! **L'empreinte** (`empreinte`) : une case qui a un mouvement montre ses cadres à **la même échelle** que les autres cases, sans
//! jamais rapetisser son contenu. Elle prend de la place là où le mouvement va, juste ce qu'il faut :
//! - un travelling (horizontal) élargit la case dans le sens du mouvement ;
//! - un zoom arrière entoure la case d'un cadre B plus grand, dont la hauteur est **limitée** à celle d'une rangée (la case et sa
//!   bande de note) ; un zoom avant dessine B dedans ;
//! - un mouvement vers le bas (diagonale, panoramique vertical) met B dans la rangée suivante ; vers le haut, dans la précédente.
//!
//! Les cadres d'une même rangée partagent une **tuile** (une vignette composée sur un fil de fond, le dessin de la feuille à
//! l'échelle des cases) ; une rangée de plus, une tuile de plus. Rien n'est enregistré dans le projet pour cela : tout se
//! déduit des clés (en supprimer au montage fait rétrécir l'empreinte).

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

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

/// Le rectangle de la feuille que couvre un cadre (ses quatre coins), s'ils sont finis.
fn rect_feuille(c: &Cadre) -> Option<Rect> {
    let mut r = Rect::NOTHING;
    for p in c.coins {
        if !(p.x.is_finite() && p.y.is_finite()) {
            return None;
        }
        r.extend_with(p);
    }
    (r.width() > 1e-3 && r.height() > 1e-3).then_some(r)
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
    /// Ce dont une tuile peut dépasser la hauteur de la case : la bande de note (l'espace et ses lignes). Un cadre est limité à
    /// la hauteur d'une rangée : la case et sa bande de note.
    pub total: f32,
    /// Ce qui peut dépasser en haut de la case (la ligne du numéro de plan, moins un filet).
    pub haut: f32,
}

/// Un cadre du mouvement, tel qu'il se dessine sur la planche, **relatif à la case A** : x depuis le bord gauche de A, y depuis
/// le haut de l'image de sa rangée.
#[derive(Clone, Debug, PartialEq)]
pub struct CadreAff {
    pub rang: usize,
    pub rect: Rect,
    /// La rangée par rapport à celle de A : 0 la même, 1 la suivante, -1 la précédente.
    pub bande: i32,
    /// Le cadre est plus grand que la limite : il est dessiné réduit (« limite de taille »).
    pub reduit: bool,
}

/// Une tuile : le rectangle d'une rangée de l'empreinte (relatif comme `CadreAff`) et la part de la feuille qu'il montre.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuile {
    pub bande: i32,
    pub rect: Rect,
    pub zone: Zone,
}

/// L'empreinte d'une case qui a un mouvement de caméra.
#[derive(Clone, Debug, PartialEq)]
pub struct Empreinte {
    pub cadres: Vec<CadreAff>,
    /// Une tuile par rangée occupée, de la plus haute à la plus basse.
    pub tuiles: Vec<Tuile>,
    pub bande_min: i32,
    pub bande_max: i32,
    /// L'étendue en x, relative au bord gauche de A (x_min ≤ 0, x_max ≥ largeur de la case).
    pub x_min: f32,
    pub x_max: f32,
}

/// La rangée d'un cadre dont le centre est décalé de `dy` (points de la planche) par rapport à A : la même rangée tant que
/// le décalage tient dans la place laissée par la bande de note, sinon la précédente ou la suivante.
fn bande_de(dy: f32, p: &Place) -> i32 {
    if dy.abs() <= p.total {
        0
    } else {
        dy.signum() as i32
    }
}

/// L'empreinte du plan `panel` (rien s'il n'a pas de mouvement). Les cadres sont à l'échelle de la case : un cadre A fait une
/// largeur de case.
pub fn empreinte(doc: &Document, panel: usize, p: &Place) -> Option<Empreinte> {
    let cads = cadres(doc, panel);
    let a = cads.first()?;
    let ra = rect_feuille(a)?;
    let echelle = p.card_w / taille_cadre(doc, &a.cam).x.max(1e-3);
    let (cw, ch) = (p.card_w, p.thumb_h);
    let mut cadres_aff = vec![CadreAff { rang: 0, rect: Rect::from_min_size(Pos2::ZERO, vec2(cw, ch)), bande: 0, reduit: false }];
    let mut sources = vec![ra.center()];
    for c in cads.iter().skip(1) {
        // (Un cadre sans coins finis, une caméra qui ne voit plus la feuille : A, à l'identique.)
        let r = rect_feuille(c).unwrap_or(ra);
        let d = (r.center() - ra.center()) * echelle;
        let mut taille = r.size() * echelle;
        let f = ((ch + p.total) / taille.y.max(1e-3)).min(p.row_w / taille.x.max(1e-3)).min(1.0);
        let reduit = f < 1.0;
        taille = (taille * f).max(vec2(6.0, 6.0));
        let bande = bande_de(d.y, p);
        let cx = (cw * 0.5 + d.x).clamp(cw - p.row_w + taille.x * 0.5, p.row_w - taille.x * 0.5);
        let cy = if bande == 0 {
            (ch * 0.5 + d.y).clamp(-p.haut + taille.y * 0.5, ch + p.total - taille.y * 0.5)
        } else {
            ch * 0.5
        };
        cadres_aff.push(CadreAff { rang: c.rang, rect: Rect::from_center_size(pos2(cx, cy), taille), bande, reduit });
        sources.push(r.center());
    }
    // Pas plus large qu'une rangée : on resserre les décalages, au pire tous les cadres sont centrés sur A.
    let largeur = |v: &[CadreAff]| {
        let (a, b) = v.iter().fold((0.0_f32, cw), |(lo, hi), c| (lo.min(c.rect.left()), hi.max(c.rect.right())));
        b - a
    };
    if largeur(&cadres_aff) > p.row_w + 1e-3 {
        let w = largeur(&cadres_aff);
        for facteur in [((p.row_w - cw) / (w - cw).max(1e-3)).clamp(0.0, 1.0), 0.0] {
            for c in cadres_aff.iter_mut().skip(1) {
                let cx = cw * 0.5 + (c.rect.center().x - cw * 0.5) * facteur;
                c.rect = Rect::from_center_size(pos2(cx, c.rect.center().y), c.rect.size());
            }
            if largeur(&cadres_aff) <= p.row_w + 1e-3 {
                break;
            }
        }
    }
    let bande_min = cadres_aff.iter().map(|c| c.bande).min().unwrap_or(0);
    let bande_max = cadres_aff.iter().map(|c| c.bande).max().unwrap_or(0);
    let mut tuiles = Vec::new();
    for bande in bande_min..=bande_max {
        let membres: Vec<usize> = (0..cadres_aff.len()).filter(|&i| cadres_aff[i].bande == bande).collect();
        let Some(&premier) = membres.first() else { continue };
        let rect = membres.iter().fold(Rect::NOTHING, |r, &i| r.union(cadres_aff[i].rect));
        // La tuile s'ancre sur son premier cadre : son centre sur la planche est celui de son centre sur la feuille.
        let centre = sources[premier] + (rect.center() - cadres_aff[premier].rect.center()) / echelle;
        tuiles.push(Tuile { bande, rect, zone: Zone { centre, taille: rect.size() / echelle, echelle } });
    }
    let x_min = tuiles.iter().map(|t| t.rect.left()).fold(0.0, f32::min);
    let x_max = tuiles.iter().map(|t| t.rect.right()).fold(cw, f32::max);
    Some(Empreinte { cadres: cadres_aff, tuiles, bande_min, bande_max, x_min, x_max })
}

/// Comment relier deux cadres qui se suivent (en bleu, fin).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lien {
    /// Une flèche au-dessus de la rangée, de gauche à droite ou l'inverse (un travelling).
    FlecheDessus { de: Pos2, vers: Pos2 },
    /// Une flèche vers le bas ou vers le haut, d'un cadre à celui d'une autre rangée (un panoramique vertical).
    FlecheVerticale { de: Pos2, vers: Pos2 },
    /// Les coins d'un cadre reliés à ceux du suivant (un zoom, une diagonale : une boîte en perspective).
    Coins([(Pos2, Pos2); 4]),
}

/// Les liens entre les cadres qui se suivent. `rects` : leur rectangle et leur rangée sur la planche (tous dans le même repère).
pub fn liens(rects: &[(Rect, i32)], largeur_case: f32) -> Vec<Lien> {
    let mut out = Vec::new();
    for w in rects.windows(2) {
        let ((a, ba), (b, bb)) = (w[0], w[1]);
        let zoome = (b.width() / a.width().max(1e-3) - 1.0).abs() > 0.06;
        let coins = |a: Rect, b: Rect| Lien::Coins([(a.left_top(), b.left_top()), (a.right_top(), b.right_top()), (a.right_bottom(), b.right_bottom()), (a.left_bottom(), b.left_bottom())]);
        if ba == bb {
            if zoome {
                out.push(coins(a, b));
            } else if (a.center() - b.center()).length() > 4.0 {
                let y = a.top().min(b.top()) - 5.0;
                out.push(Lien::FlecheDessus { de: pos2(a.center().x, y), vers: pos2(b.center().x, y) });
            }
        } else if (a.center().x - b.center().x).abs() < 0.15 * largeur_case && !zoome {
            let (de, vers) = if bb > ba { (pos2(a.center().x, a.bottom()), pos2(b.center().x, b.top())) } else { (pos2(a.center().x, a.top()), pos2(b.center().x, b.bottom())) };
            out.push(Lien::FlecheVerticale { de, vers });
        } else {
            out.push(coins(a, b));
        }
    }
    out
}

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

fn distance_segment(a: Pos2, b: Pos2, p: Pos2) -> f32 {
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
    // Les coins d'abord : le plus proche ; à égalité, le cadre le plus petit.
    let mut best: Option<(f32, f32, usize, usize)> = None;
    for (r, q) in cadres.iter().enumerate() {
        if let Some((c, d)) = coin_proche(q) {
            let a = aire(q);
            if best.is_none_or(|(bd, ba, _, _)| d < bd - 1e-3 || ((d - bd).abs() <= 1e-3 && a < ba)) {
                best = Some((d, a, r, c));
            }
        }
    }
    if let Some((_, _, r, c)) = best {
        return Some(Choix { prise: Prise::Zoom(r), rang: r, coin: c });
    }
    // Dedans : le plus petit cadre qui contient le point (à aire égale, le dernier).
    let mut dedans: Option<(f32, usize)> = None;
    for (r, q) in cadres.iter().enumerate() {
        if dans_quad(q, p) {
            let a = aire(q);
            if dedans.is_none_or(|(ba, _)| a <= ba) {
                dedans = Some((a, r));
            }
        }
    }
    if let Some((_, r)) = dedans {
        return Some(Choix { prise: Prise::Deplace(r), rang: r, coin: 0 });
    }
    // Hors de tous (l'empreinte entre les cadres, la marge) : le plus proche.
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
