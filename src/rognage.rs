//! Rognage des plans dans le montage (Anthony, 2026-10-19 : « Je ne peux pas saisir le début d'un
//! shot pour le raccourcir en l'état »).
//!
//! Un plan commence à l'image 0 de ses calques, de sa caméra, de ses marques… : tout est compté
//! depuis le début du plan, et sa durée est celle de son contenu. Rogner son début ou sa fin ne
//! doit pourtant rien détruire (choix de SHOT : le montage se défait et se refait, comme sur une
//! table de montage). Ce qui sort du plan est donc rangé dans une **réserve** (`Reserve`), à côté
//! du plan, et revient quand on le rallonge :
//!
//! - `head` : les images d'avant la première image du plan, avec leurs dessins, leurs rôles, les
//!   clés de leurs effets, les clés de caméra, les marques d'image, les signets, les points
//!   d'entrée et de sortie et les commentaires qui s'y trouvaient ;
//! - `tail` : de même, pour les images d'après la dernière.
//!
//! Le plan vivant est toujours complet et cohérent (c'est ce que voient l'écran, l'export, le PDF) ;
//! la réserve n'est lue que par le rognage. Elle est rangée par numéro de plan (`Document::reserves`,
//! avec l'annulation, qui la rétablit comme le reste) et enregistrée dans `rognages.json`, à côté du
//! manifeste, sans changer `VERSION` : un projet sans ce fichier n'a pas de réserve (voir plus bas).
//!
//! Les clés de caméra et d'effets continuent leur mouvement à la coupure : rogner pose une clé au
//! bord du plan (celle que la courbe donnait à cet endroit) si le mouvement la traversait ; le
//! bord est noté dans la réserve, et la clé repart quand le contenu revient (rien ne s'accumule).
//!
//! Chaque opération renvoie ce qu'elle a vraiment fait (un plan garde toujours au moins une image).

use crate::camera::{self, Camera, CameraKey};
use crate::commentaires::Comment;
use crate::document::{CelId, Document, ImageMark, Panel};
use crate::fx::{Effect, FxKey};
use crate::opacitecles::OpKey;
use crate::poskeys::PosKey;
use crate::roles::Roles;

thread_local! {
    /// Les images rognées qu'une jonction de plans vient d'oublier (`forget_joint_reserves`).
    static OUBLIEES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Les images rognées oubliées par les jonctions depuis le dernier appel (le compteur repart de zéro).
pub(crate) fn images_oubliees() -> usize {
    OUBLIEES.with(|c| c.replace(0))
}

/// « 1 dessin », « 5 dessins » (la barre d'état dit ce qu'une suppression emporte).
pub(crate) fn dessins_texte(n: usize) -> String {
    if n == 1 { "1 dessin".into() } else { format!("{n} dessins") }
}

/// Durée la plus longue d'un plan (comme le champ Durée).
pub const MAX_PLAN: usize = 100_000;

/// Ce qu'on fait d'un plan dans le montage (gestes à la souris, commandes, menus).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rogner {
    /// Rogner (positif) ou rallonger (négatif) le début.
    Head(isize),
    /// Rallonger (positif) ou rogner (négatif) la fin.
    Tail(isize),
    /// Déplacer la coupe entre ce plan et le suivant (positif : vers la droite).
    Roll(isize),
    /// Faire glisser le contenu sous la fenêtre du plan (positif : le contenu part vers la droite).
    Slip(isize),
    /// Donner la durée exacte (en rognant ou en prolongeant la fin).
    Duration(usize),
    /// Reprendre tout le contenu rogné.
    RestoreAll,
    /// Oublier le contenu rogné.
    Forget,
}

/// Les clés d'un effet qui sont sorties du plan, et la clé posée au bord pour garder la courbe.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxPiece {
    /// Clés sorties, en images de la partie.
    pub keys: Vec<FxKey>,
    /// Valeurs de la clé ajoutée au bord du plan à la coupure (rien : aucune clé ajoutée).
    pub edge: Option<Vec<f32>>,
}

/// Ce qu'un calque a perdu d'un côté.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerPiece {
    /// Numéro du calque (il suit le calque, pas son rang).
    pub layer: u64,
    /// Dessins des images sorties (une partie plus courte : le reste est vide).
    pub frames: Vec<Option<CelId>>,
    pub roles: Roles,
    /// Un par effet du calque, dans l'ordre de sa pile.
    pub fx: Vec<FxPiece>,
    /// Clés de position et d'opacité sorties **par le début** (en images de la partie), et la clé posée au bord du plan à la
    /// coupure pour garder la courbe (`duree-cles`, 2026-11-07 : décaler les clés d'un plan ne doit en perdre aucune). Par la
    /// fin, ces clés restent sur le calque, au-delà de la dernière image : elles reprennent leur sens quand le plan se prolonge.
    pub pos: Vec<PosKey>,
    pub pos_edge: Option<PosKey>,
    pub op: Vec<OpKey>,
    pub op_edge: Option<OpKey>,
    /// Départ et fin de la trajectoire du calque avant la coupure du début, et ce qu'ils sont devenus (ramenés au bord du plan si la
    /// coupure les passe) : si personne n'y touche, la restitution rend exactement les premiers.
    pub traj_avant: Option<(usize, Option<usize>)>,
    pub traj_apres: Option<(usize, Option<usize>)>,
}

/// Une partie du plan hors champ : `len` images, comptées depuis 0 (au début, l'image `len - 1` est
/// celle qui touche le plan ; à la fin, l'image 0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Piece {
    pub len: usize,
    pub layers: Vec<LayerPiece>,
    pub camera: Vec<CameraKey>,
    /// Caméra de la clé ajoutée au bord du plan à la coupure.
    pub cam_edge: Option<Camera>,
    pub marks: Vec<ImageMark>,
    pub bookmarks: Vec<usize>,
    pub in_point: Option<usize>,
    pub out_point: Option<usize>,
    /// Commentaires sortis (leur plan est celui de la réserve).
    pub comments: Vec<Comment>,
    /// Cette partie a été rangée par un code qui décale aussi les clés de position et d'opacité du calque (`cut_head`) : à sa
    /// restitution, les clés vivantes avancent de `len`. Une partie lue d'un ancien `rognages.json` (faux) garde l'ancienne règle :
    /// ces clés n'avaient pas été décalées au rangement, elles ne le sont pas non plus au retour.
    pub keys_v2: bool,
}

/// Le contenu d'un plan qui est hors de lui, au début et à la fin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reserve {
    pub head: Piece,
    pub tail: Piece,
}

impl Reserve {
    /// Applique `f` aux caméras de ce contenu rogné (un changement de feuille).
    pub(crate) fn map_cameras(&mut self, f: impl Fn(&Camera) -> Camera) {
        for piece in [&mut self.head, &mut self.tail] {
            for k in &mut piece.camera {
                k.cam = f(&k.cam);
            }
            if let Some(c) = piece.cam_edge.as_mut() {
                *c = f(c);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.len == 0 && self.tail.len == 0
    }

    /// Les dessins gardés dans la réserve (ils restent dans le projet à l'enregistrement).
    pub fn cels(&self) -> impl Iterator<Item = CelId> + '_ {
        [&self.head, &self.tail].into_iter().flat_map(|p| p.layers.iter()).flat_map(|l| l.frames.iter().flatten().copied())
    }
}

// ------------------------------------------------------------------------ clés de caméra

/// Une clé de caméra est « simple » : sans adouci (c'est une clé que le rognage a posée).
fn plain(k: &CameraKey) -> bool {
    !k.smooth && k.ease_in == 0.0 && k.ease_out == 0.0
}

/// Coupe les clés au début, à l'image `c` : (sorties, gardées ramenées à 0, clé posée au bord).
fn cam_cut_head(keys: &[CameraKey], c: usize, default: Camera) -> (Vec<CameraKey>, Vec<CameraKey>, Option<Camera>) {
    let out: Vec<CameraKey> = keys.iter().filter(|k| k.frame < c).copied().collect();
    let mut kept: Vec<CameraKey> = keys.iter().filter(|k| k.frame >= c).map(|k| CameraKey { frame: k.frame - c, ..*k }).collect();
    let mut edge = None;
    if !out.is_empty() && kept.first().is_none_or(|k| k.frame > 0) {
        let cam = camera::camera_at(keys, c, default);
        kept.insert(0, CameraKey::new(0, cam));
        edge = Some(cam);
    }
    (out, kept, edge)
}

/// Coupe les clés à la fin : le plan garde [0, c) ; (gardées, sorties ramenées à 0, clé du bord).
fn cam_cut_tail(keys: &[CameraKey], c: usize, default: Camera) -> (Vec<CameraKey>, Vec<CameraKey>, Option<Camera>) {
    let mut kept: Vec<CameraKey> = keys.iter().filter(|k| k.frame < c).copied().collect();
    let out: Vec<CameraKey> = keys.iter().filter(|k| k.frame >= c).map(|k| CameraKey { frame: k.frame - c, ..*k }).collect();
    let mut edge = None;
    if !out.is_empty() && kept.last().is_none_or(|k| k.frame + 1 < c) {
        let cam = camera::camera_at(keys, c - 1, default);
        camera::set_key(&mut kept, c - 1, cam);
        edge = Some(cam);
    }
    (kept, out, edge)
}

// ------------------------------------------------------------------------ clés d'effets

fn fx_cut_head(e: &mut Effect, c: usize) -> FxPiece {
    if e.keys.is_empty() {
        return FxPiece::default();
    }
    let v = e.values_at(c);
    let keys: Vec<FxKey> = e.keys.iter().filter(|k| k.frame < c).cloned().collect();
    e.keys.retain(|k| k.frame >= c);
    for k in &mut e.keys {
        k.frame -= c;
    }
    let mut edge = None;
    if !keys.is_empty() && e.keys.first().is_none_or(|k| k.frame > 0) {
        e.keys.insert(0, FxKey { frame: 0, values: v.clone() });
        edge = Some(v);
    }
    FxPiece { keys, edge }
}

fn fx_cut_tail(e: &mut Effect, c: usize) -> FxPiece {
    if e.keys.is_empty() {
        return FxPiece::default();
    }
    let v = e.values_at(c - 1);
    let keys: Vec<FxKey> = e.keys.iter().filter(|k| k.frame >= c).map(|k| FxKey { frame: k.frame - c, values: k.values.clone() }).collect();
    e.keys.retain(|k| k.frame < c);
    let mut edge = None;
    if !keys.is_empty() && e.keys.last().is_none_or(|k| k.frame + 1 < c) {
        e.keys.push(FxKey { frame: c - 1, values: v.clone() });
        edge = Some(v);
    }
    FxPiece { keys, edge }
}

// ------------------------------------------------------------------------ clés de position et d'opacité

/// Coupe les clés de position au début, à l'image `c` : (sorties, gardées ramenées à 0, clé posée au bord).
fn pos_cut_head(keys: &[PosKey], c: usize, base: crate::document::LayerTransform) -> (Vec<PosKey>, Vec<PosKey>, Option<PosKey>) {
    let out: Vec<PosKey> = keys.iter().filter(|k| k.frame < c).copied().collect();
    let mut kept: Vec<PosKey> = keys.iter().filter(|k| k.frame >= c).map(|k| PosKey { frame: k.frame - c, ..*k }).collect();
    let mut edge = None;
    if !out.is_empty() && kept.first().is_none_or(|k| k.frame > 0) {
        let key = PosKey::new(0, crate::poskeys::pose_at(keys, c, base));
        kept.insert(0, key);
        edge = Some(key);
    }
    (out, kept, edge)
}

/// Comme `pos_cut_head`, pour les clés d'opacité.
fn op_cut_head(keys: &[OpKey], c: usize, base: f32) -> (Vec<OpKey>, Vec<OpKey>, Option<OpKey>) {
    let out: Vec<OpKey> = keys.iter().filter(|k| k.frame < c).copied().collect();
    let mut kept: Vec<OpKey> = keys.iter().filter(|k| k.frame >= c).map(|k| OpKey { frame: k.frame - c, ..*k }).collect();
    let mut edge = None;
    if !out.is_empty() && kept.first().is_none_or(|k| k.frame > 0) {
        let key = OpKey::new(0, crate::opacitecles::value_at(keys, c, base));
        kept.insert(0, key);
        edge = Some(key);
    }
    (out, kept, edge)
}

/// Remet les clés de position et d'opacité sorties par le début : la clé du bord s'en va si personne n'y a touché, les clés
/// restantes avancent de `len` images, les clés sorties reprennent leur place devant.
fn keys_absorb_head(l: &mut crate::document::Layer, lp: Option<&LayerPiece>, len: usize, v2: bool) {
    if !v2 {
        return;
    }
    if let Some(e) = lp.and_then(|lp| lp.pos_edge) {
        if l.pos_keys.first() == Some(&e) {
            l.pos_keys.remove(0);
        }
    }
    for k in &mut l.pos_keys {
        k.frame += len;
    }
    if let Some(lp) = lp {
        let mut keys = lp.pos.clone();
        keys.append(&mut l.pos_keys);
        l.pos_keys = keys;
    }
    if let Some(e) = lp.and_then(|lp| lp.op_edge) {
        if l.opacity_keys.first() == Some(&e) {
            l.opacity_keys.remove(0);
        }
    }
    for k in &mut l.opacity_keys {
        k.frame += len;
    }
    if let Some(lp) = lp {
        let mut keys = lp.op.clone();
        keys.append(&mut l.opacity_keys);
        l.opacity_keys = keys;
    }
    // La trajectoire (départ et fin, en images du plan) suit les clés. Si personne n'y a touché depuis la coupure, elle reprend
    // exactement son départ et sa fin d'avant ; sinon elle avance de `len` comme tout le reste.
    if let Some(t) = l.trajectoire.as_mut() {
        let maintenant = (t.depart, t.fin);
        match lp.and_then(|lp| lp.traj_apres.zip(lp.traj_avant)) {
            Some((apres, avant)) if apres == maintenant => (t.depart, t.fin) = avant,
            _ => {
                t.depart += len;
                t.fin = t.fin.map(|f| f + len);
            }
        }
    }
}

fn fx_absorb_head(e: &mut Effect, piece: Option<&FxPiece>, len: usize) {
    if let Some(edge) = piece.and_then(|p| p.edge.as_ref()) {
        if e.keys.first().is_some_and(|k| k.frame == 0 && &k.values == edge) {
            e.keys.remove(0);
        }
    }
    for k in &mut e.keys {
        k.frame += len;
    }
    if let Some(p) = piece {
        let mut keys = p.keys.clone();
        keys.append(&mut e.keys);
        e.keys = keys;
    }
}

fn fx_absorb_tail(e: &mut Effect, piece: Option<&FxPiece>, n: usize) {
    let Some(p) = piece else { return };
    if let Some(edge) = p.edge.as_ref() {
        if e.keys.last().is_some_and(|k| k.frame + 1 == n && &k.values == edge) {
            e.keys.pop();
        }
    }
    e.keys.extend(p.keys.iter().map(|k| FxKey { frame: k.frame + n, values: k.values.clone() }));
}

// ------------------------------------------------------------------------ couper

/// Sort les `c` premières images du plan (1 <= c < durée) : le plan commence `c` images plus tard.
/// Le plan doit être complet (sa réserve du début reprise avant).
fn cut_head(p: &mut Panel, comments: &mut Vec<Comment>, c: usize, default: Camera) -> Piece {
    let n = p.frame_count();
    let mut piece = Piece { len: c, keys_v2: true, ..Default::default() };
    for l in &mut p.layers {
        if let Some(v) = l.video.as_mut() {
            v.start -= c as i64;
        }
        if l.folder {
            continue;
        }
        if l.scene_wide.is_some() {
            // Un calque de toute la scène suit la durée du plan (`sync_scene_layers`).
            let k = l.frames.len().min(c);
            l.frames.drain(..k);
            l.trim();
            continue;
        }
        let mut lp = LayerPiece { layer: l.id, ..Default::default() };
        let k = l.frames.len().min(c);
        lp.frames = l.frames.drain(..k).collect();
        lp.roles = l.roles.range(..c).map(|(f, r)| (*f, *r)).collect();
        l.roles = l.roles.range(c..).map(|(f, r)| (f - c, *r)).collect();
        lp.fx = l.fx.iter_mut().map(|e| fx_cut_head(e, c)).collect();
        (lp.pos, l.pos_keys, lp.pos_edge) = pos_cut_head(&l.pos_keys, c, l.transform);
        (lp.op, l.opacity_keys, lp.op_edge) = op_cut_head(&l.opacity_keys, c, l.opacity);
        if let Some(t) = l.trajectoire.as_mut() {
            lp.traj_avant = Some((t.depart, t.fin));
            t.depart = t.depart.saturating_sub(c);
            t.fin = t.fin.map(|f| f.saturating_sub(c)).filter(|&f| f > t.depart);
            lp.traj_apres = Some((t.depart, t.fin));
        }
        l.trim();
        piece.layers.push(lp);
    }
    let (out, kept, edge) = cam_cut_head(&p.camera, c, default);
    (piece.camera, p.camera, piece.cam_edge) = (out, kept, edge);
    // L'effet de caméra vivant suit les images, comme les clés (partie 129) ; commencé dans la partie coupée, il part du début.
    if let Some(v) = p.effet_camera.as_mut() {
        v.start = v.start.saturating_sub(c);
    }
    // Une marque posée sur la première image gardée serait l'image de départ du plan : elle sort.
    let (hidden, shown): (Vec<ImageMark>, Vec<ImageMark>) = std::mem::take(&mut p.marks).into_iter().partition(|m| m.frame <= c);
    piece.marks = hidden;
    p.marks = shown.into_iter().map(|m| ImageMark { frame: m.frame - c, ..m }).collect();
    let cues = &mut p.cues;
    let (hidden, shown): (Vec<usize>, Vec<usize>) = std::mem::take(&mut cues.bookmarks).into_iter().partition(|&b| b < c);
    piece.bookmarks = hidden;
    cues.bookmarks = shown.into_iter().map(|b| b - c).collect();
    piece.in_point = cues.in_point.filter(|&f| f < c);
    cues.in_point = cues.in_point.filter(|&f| f >= c).map(|f| f - c);
    piece.out_point = cues.out_point.filter(|&f| f < c);
    cues.out_point = cues.out_point.filter(|&f| f >= c).map(|f| f - c);
    // Les notes dessinées suivent le temps : elles glissent avec les images (hors du plan, elles
    // ne se voient plus et reviennent avec le contenu).
    for note in &mut cues.notes {
        for pt in &mut note.points {
            pt[0] -= c as f32;
        }
    }
    let id = p.id;
    let mut kept = Vec::with_capacity(comments.len());
    for mut cm in comments.drain(..) {
        if cm.panel != id {
            kept.push(cm);
        } else if cm.frame < c {
            piece.comments.push(cm);
        } else {
            cm.frame -= c;
            kept.push(cm);
        }
    }
    *comments = kept;
    p.length = n - c;
    piece
}

/// Sort les images du plan après la `c`-ième (1 <= c < durée) : le plan s'arrête à l'image `c - 1`.
fn cut_tail(p: &mut Panel, comments: &mut Vec<Comment>, c: usize, default: Camera) -> Piece {
    let n = p.frame_count();
    let mut piece = Piece { len: n - c, ..Default::default() };
    for l in &mut p.layers {
        if l.folder {
            continue;
        }
        if l.scene_wide.is_some() {
            l.frames.truncate(c);
            l.trim();
            continue;
        }
        let mut lp = LayerPiece { layer: l.id, ..Default::default() };
        if l.frames.len() > c {
            lp.frames = l.frames.split_off(c);
        }
        lp.roles = l.roles.split_off(&c).into_iter().map(|(f, r)| (f - c, r)).collect();
        lp.fx = l.fx.iter_mut().map(|e| fx_cut_tail(e, c)).collect();
        l.trim();
        piece.layers.push(lp);
    }
    let (kept, out, edge) = cam_cut_tail(&p.camera, c, default);
    (p.camera, piece.camera, piece.cam_edge) = (kept, out, edge);
    let (hidden, shown): (Vec<ImageMark>, Vec<ImageMark>) = std::mem::take(&mut p.marks).into_iter().partition(|m| m.frame >= c);
    piece.marks = hidden.into_iter().map(|m| ImageMark { frame: m.frame - c, ..m }).collect();
    p.marks = shown;
    let cues = &mut p.cues;
    let (hidden, shown): (Vec<usize>, Vec<usize>) = std::mem::take(&mut cues.bookmarks).into_iter().partition(|&b| b >= c);
    piece.bookmarks = hidden.into_iter().map(|b| b - c).collect();
    cues.bookmarks = shown;
    piece.in_point = cues.in_point.filter(|&f| f >= c).map(|f| f - c);
    cues.in_point = cues.in_point.filter(|&f| f < c);
    piece.out_point = cues.out_point.filter(|&f| f >= c).map(|f| f - c);
    cues.out_point = cues.out_point.filter(|&f| f < c);
    let id = p.id;
    let mut kept = Vec::with_capacity(comments.len());
    for mut cm in comments.drain(..) {
        if cm.panel == id && cm.frame >= c {
            cm.frame -= c;
            piece.comments.push(cm);
        } else {
            kept.push(cm);
        }
    }
    *comments = kept;
    p.length = c;
    piece
}

// ------------------------------------------------------------------------ reprendre

/// Remet dans le plan toute la partie du début : le plan commence `piece.len` images plus tôt.
fn absorb_head(p: &mut Panel, comments: &mut Vec<Comment>, piece: Piece) {
    let len = piece.len;
    if len == 0 {
        return;
    }
    let keys_v2 = piece.keys_v2;
    let n = p.frame_count();
    for l in &mut p.layers {
        if let Some(v) = l.video.as_mut() {
            v.start += len as i64;
        }
        if l.folder {
            continue;
        }
        if l.scene_wide.is_some() {
            if let Some(&Some(first)) = l.frames.first() {
                l.frames.splice(0..0, std::iter::repeat_n(Some(first), len));
            }
            continue;
        }
        let lp = piece.layers.iter().find(|x| x.layer == l.id);
        let mut frames: Vec<Option<CelId>> = Vec::with_capacity(len + l.frames.len());
        if let Some(lp) = lp {
            frames.extend_from_slice(&lp.frames);
        }
        frames.resize(len, None);
        frames.append(&mut l.frames);
        l.frames = frames;
        let mut roles: Roles = lp.map(|lp| lp.roles.clone()).unwrap_or_default();
        roles.extend(l.roles.iter().map(|(f, r)| (f + len, *r)));
        l.roles = roles;
        for (i, e) in l.fx.iter_mut().enumerate() {
            fx_absorb_head(e, lp.and_then(|lp| lp.fx.get(i)), len);
        }
        keys_absorb_head(l, lp, len, keys_v2);
        l.trim();
    }
    let mut live = std::mem::take(&mut p.camera);
    if let (Some(cam), Some(k0)) = (piece.cam_edge, live.first()) {
        if k0.frame == 0 && k0.cam == cam && plain(k0) {
            live.remove(0);
        }
    }
    p.camera = piece.camera;
    p.camera.extend(live.into_iter().map(|k| CameraKey { frame: k.frame + len, ..k }));
    if let Some(v) = p.effet_camera.as_mut() {
        v.start += len;
    }
    let mut marks = piece.marks;
    marks.extend(std::mem::take(&mut p.marks).into_iter().map(|m| ImageMark { frame: m.frame + len, ..m }));
    p.marks = marks;
    let cues = &mut p.cues;
    let mut bookmarks = piece.bookmarks;
    bookmarks.extend(cues.bookmarks.iter().map(|b| b + len));
    cues.bookmarks = bookmarks;
    cues.in_point = cues.in_point.map(|f| f + len).or(piece.in_point);
    cues.out_point = cues.out_point.map(|f| f + len).or(piece.out_point);
    for note in &mut cues.notes {
        for pt in &mut note.points {
            pt[0] += len as f32;
        }
    }
    let id = p.id;
    for cm in comments.iter_mut().filter(|c| c.panel == id) {
        cm.frame += len;
    }
    comments.extend(piece.comments);
    p.length = n + len;
}

/// Remet dans le plan toute la partie de la fin.
fn absorb_tail(p: &mut Panel, comments: &mut Vec<Comment>, piece: Piece) {
    let len = piece.len;
    if len == 0 {
        return;
    }
    let n = p.frame_count();
    for l in &mut p.layers {
        if l.folder || l.scene_wide.is_some() {
            continue;
        }
        let Some(lp) = piece.layers.iter().find(|x| x.layer == l.id) else { continue };
        if !lp.frames.is_empty() {
            l.frames.resize(n, None);
            l.frames.extend_from_slice(&lp.frames);
        }
        l.roles.extend(lp.roles.iter().map(|(f, r)| (f + n, *r)));
        for (i, e) in l.fx.iter_mut().enumerate() {
            fx_absorb_tail(e, lp.fx.get(i), n);
        }
        l.trim();
    }
    if let (Some(cam), Some(k)) = (piece.cam_edge, p.camera.last()) {
        if k.frame + 1 == n && k.cam == cam && plain(k) {
            p.camera.pop();
        }
    }
    p.camera.extend(piece.camera.iter().map(|k| CameraKey { frame: k.frame + n, ..*k }));
    p.marks.extend(piece.marks.into_iter().map(|m| ImageMark { frame: m.frame + n, ..m }));
    p.marks.sort_by_key(|m| m.frame);
    p.marks.dedup_by_key(|m| m.frame);
    let cues = &mut p.cues;
    cues.bookmarks.extend(piece.bookmarks.iter().map(|b| b + n));
    cues.bookmarks.sort_unstable();
    cues.bookmarks.dedup();
    cues.in_point = cues.in_point.or(piece.in_point.map(|f| f + n));
    cues.out_point = cues.out_point.or(piece.out_point.map(|f| f + n));
    comments.extend(piece.comments.into_iter().map(|c| Comment { frame: c.frame + n, ..c }));
    p.length = n + len;
}

/// Allonge le plan de `k` images au début, en tenant le premier dessin de chaque calque (`hold`) ou en laissant ces images vides :
/// tout ce qui tient aux images (clés, marques, signets, commentaires) suit le contenu.
fn pad_head(p: &mut Panel, comments: &mut [Comment], k: usize, hold: bool) {
    let n = p.frame_count();
    for l in &mut p.layers {
        if let Some(v) = l.video.as_mut() {
            v.start += k as i64;
        }
        if l.folder || l.scene_wide.is_some() {
            continue;
        }
        let first = if hold { l.frames.first().copied().flatten() } else { None };
        // (La trajectoire — départ et fin, en images du plan — suit les dessins et les clés par `Layer::shift_frames`.)
        l.insert(0, first, k);
        l.trim();
    }
    for key in &mut p.camera {
        key.frame += k;
    }
    if let Some(v) = p.effet_camera.as_mut() {
        v.start += k;
    }
    for m in &mut p.marks {
        m.frame += k;
    }
    let cues = &mut p.cues;
    for b in &mut cues.bookmarks {
        *b += k;
    }
    cues.in_point = cues.in_point.map(|f| f + k);
    cues.out_point = cues.out_point.map(|f| f + k);
    for note in &mut cues.notes {
        for pt in &mut note.points {
            pt[0] += k as f32;
        }
    }
    let id = p.id;
    for cm in comments.iter_mut().filter(|c| c.panel == id) {
        cm.frame += k;
    }
    p.length = n + k;
}

/// La réserve du début de `r` ramenée à `h` images (0 : tout revient).
fn set_hidden_head(p: &mut Panel, comments: &mut Vec<Comment>, r: &mut Reserve, default: Camera, h: usize) {
    let piece = std::mem::take(&mut r.head);
    absorb_head(p, comments, piece);
    let h = h.min(p.frame_count().saturating_sub(1));
    if h > 0 {
        r.head = cut_head(p, comments, h, default);
    }
}

/// La réserve de la fin de `r` ramenée à `t` images.
fn set_hidden_tail(p: &mut Panel, comments: &mut Vec<Comment>, r: &mut Reserve, default: Camera, t: usize) {
    let piece = std::mem::take(&mut r.tail);
    absorb_tail(p, comments, piece);
    let n = p.frame_count();
    let t = t.min(n.saturating_sub(1));
    if t > 0 {
        r.tail = cut_tail(p, comments, n - t, default);
    }
}

impl Document {
    /// Images cachées au début et à la fin du plan `pi` (sa réserve).
    pub fn hidden(&self, pi: usize) -> (usize, usize) {
        self.panels
            .get(pi)
            .and_then(|p| self.reserves.get(&p.id))
            .map_or((0, 0), |r| (r.head.len, r.tail.len))
    }

    /// Le plan `pi` et sa réserve, prêts à être changés ensemble.
    fn with_reserve<R>(&mut self, pi: usize, f: impl FnOnce(&mut Panel, &mut Vec<Comment>, &mut Reserve, Camera) -> R) -> R {
        let default = self.default_camera();
        let id = self.panels[pi].id;
        let mut r = self.reserves.remove(&id).unwrap_or_default();
        let out = f(&mut self.panels[pi], &mut self.comments, &mut r, default);
        if !r.is_empty() {
            self.reserves.insert(id, r);
        }
        out
    }

    /// Rogne le début du plan `pi` de `delta` images (positif : le plan commence plus tard, des
    /// images sortent dans la réserve ; négatif : il commence plus tôt, la réserve revient, puis
    /// le premier dessin est tenu). Renvoie ce qui a été fait : un plan garde au moins une image.
    pub fn trim_head(&mut self, pi: usize, delta: isize) -> isize {
        if pi >= self.panels.len() || delta == 0 {
            return 0;
        }
        self.with_reserve(pi, |p, comments, r, default| {
            let n = p.frame_count();
            let h = r.head.len;
            if delta > 0 {
                let d = (delta as usize).min(n - 1);
                if d > 0 {
                    set_hidden_head(p, comments, r, default, h + d);
                }
                d as isize
            } else {
                let k = delta.unsigned_abs().min(MAX_PLAN.saturating_sub(n));
                let back = k.min(h);
                if back > 0 {
                    set_hidden_head(p, comments, r, default, h - back);
                }
                if k > back {
                    pad_head(p, comments, k - back, true);
                }
                -(k as isize)
            }
        })
    }

    /// Change la fin du plan `pi` de `delta` images (positif : le plan se prolonge, la réserve
    /// revient d'abord, puis le dernier dessin est tenu ; négatif : des images sortent dans la
    /// réserve). Renvoie ce qui a été fait.
    pub fn trim_tail(&mut self, pi: usize, delta: isize) -> isize {
        if pi >= self.panels.len() || delta == 0 {
            return 0;
        }
        self.with_reserve(pi, |p, comments, r, default| {
            let n = p.frame_count();
            let t = r.tail.len;
            if delta > 0 {
                let k = (delta as usize).min(MAX_PLAN.saturating_sub(n));
                let back = k.min(t);
                if back > 0 {
                    set_hidden_tail(p, comments, r, default, t - back);
                }
                if k > back {
                    let n = p.frame_count();
                    p.set_length(n + k - back);
                }
                k as isize
            } else {
                let d = delta.unsigned_abs().min(n - 1);
                if d > 0 {
                    set_hidden_tail(p, comments, r, default, t + d);
                }
                -(d as isize)
            }
        })
    }

    /// Donne au plan `pi` la durée `n` en rognant ou en prolongeant sa fin. Renvoie la durée faite.
    pub fn set_duration(&mut self, pi: usize, n: usize) -> usize {
        let Some(old) = self.panels.get(pi).map(Panel::frame_count) else { return 0 };
        let n = n.clamp(1, MAX_PLAN);
        self.trim_tail(pi, n as isize - old as isize);
        self.panels[pi].frame_count()
    }

    /// Donne au plan `pi` la durée `n` **en gardant les clés à la même distance de la fin** (Ctrl sur la poignée de fin,
    /// Anthony, 2026-11-07) : toutes les clés du plan (dessins, caméra et leurs poignées de lissage, position, opacité, effets,
    /// marques, signets) se décalent du même nombre d'images que la poignée, le rythme ne change pas. C'est le début du
    /// plan qui change au lieu de la fin : plus long, ce qui avait été rogné au début revient d'abord, puis les nouvelles
    /// images sont **vides** (la première clé de dessin glisse donc vraiment, de 0 à 4 s dans l'exemple d'Anthony : clés à
    /// 4, 5, 6, 7 s) ; plus court, ce qui passerait avant la première image part dans la réserve du début et revient en
    /// rallongeant, rien n'est perdu. Renvoie la durée faite.
    pub fn slide_plan_end(&mut self, pi: usize, n: usize) -> usize {
        let Some(old) = self.panels.get(pi).map(Panel::frame_count) else { return 0 };
        let n = n.clamp(1, MAX_PLAN);
        if n < old {
            self.trim_head(pi, (old - n) as isize);
        } else if n > old {
            self.with_reserve(pi, |p, comments, r, default| {
                let k = (n - old).min(MAX_PLAN.saturating_sub(old));
                let h = r.head.len;
                let back = k.min(h);
                if back > 0 {
                    set_hidden_head(p, comments, r, default, h - back);
                }
                if k > back {
                    pad_head(p, comments, k - back, false);
                }
            });
        }
        self.panels[pi].frame_count()
    }

    /// Déplace la coupe entre le plan `pi` et le suivant de `delta` images (positif : vers la
    /// droite, le premier s'allonge et le second perd son début) ; la durée des deux ensemble ne
    /// change pas. Renvoie ce qui a été fait.
    pub fn roll(&mut self, pi: usize, delta: isize) -> isize {
        if pi + 1 >= self.panels.len() {
            return 0;
        }
        let (a, b) = (self.panels[pi].frame_count(), self.panels[pi + 1].frame_count());
        let max_a = MAX_PLAN.saturating_sub(a);
        let max_b = MAX_PLAN.saturating_sub(b);
        let d = delta.clamp(-(a as isize - 1).min(max_b as isize), (b as isize - 1).min(max_a as isize));
        if d == 0 {
            return 0;
        }
        self.trim_tail(pi, d);
        self.trim_head(pi + 1, d);
        d
    }

    /// Fait glisser le contenu du plan `pi` sous sa fenêtre sans changer sa durée ni sa place :
    /// `delta` positif, on voit des images plus anciennes (le contenu part vers la droite, comme
    /// quand on le tire à la souris) ; négatif, des images plus récentes. Limité à ce que la
    /// réserve contient. Renvoie ce qui a été fait.
    pub fn slip(&mut self, pi: usize, delta: isize) -> isize {
        let (h, t) = self.hidden(pi);
        let d = delta.clamp(-(t as isize), h as isize);
        if d == 0 {
            return 0;
        }
        // Les images qui rentrent d'un côté sont celles qui sortent de l'autre : d'abord le plan
        // s'allonge (la réserve revient), puis il se raccourcit de l'autre côté.
        if d > 0 {
            self.trim_head(pi, -d);
            self.trim_tail(pi, -d);
        } else {
            self.trim_tail(pi, -d);
            self.trim_head(pi, -d);
        }
        d
    }

    /// Fait `op` sur le plan `pi` ; refusé (avec la phrase à montrer) quand le storyboard est
    /// verrouillé, comme les autres changements de durée. Renvoie ce qui a été fait, en images.
    pub fn rogner(&mut self, pi: usize, op: Rogner) -> Result<isize, &'static str> {
        if self.storyboard_locked {
            return Err(crate::projetui::STORYBOARD_LOCKED);
        }
        if pi >= self.panels.len() {
            return Ok(0);
        }
        Ok(match op {
            Rogner::Head(d) => self.trim_head(pi, d),
            Rogner::Tail(d) => self.trim_tail(pi, d),
            Rogner::Roll(d) => self.roll(pi, d),
            Rogner::Slip(d) => self.slip(pi, d),
            Rogner::Duration(n) => {
                let old = self.panels[pi].frame_count();
                self.set_duration(pi, n) as isize - old as isize
            }
            Rogner::RestoreAll => {
                let (h, t) = self.restore_all(pi);
                (h + t) as isize
            }
            Rogner::Forget => {
                let (h, t) = self.forget_reserve(pi);
                (h + t) as isize
            }
        })
    }

    /// Reprend tout ce qui était rogné (la réserve revient, le plan retrouve son contenu).
    pub fn restore_all(&mut self, pi: usize) -> (usize, usize) {
        let (h, t) = self.hidden(pi);
        if pi >= self.panels.len() || (h == 0 && t == 0) {
            return (0, 0);
        }
        self.with_reserve(pi, |p, comments, r, default| {
            set_hidden_head(p, comments, r, default, 0);
            set_hidden_tail(p, comments, r, default, 0);
        });
        (h, t)
    }

    /// Oublie la réserve du plan `pi` (le rognage devient définitif). Renvoie les images oubliées.
    pub fn forget_reserve(&mut self, pi: usize) -> (usize, usize) {
        let Some(id) = self.panels.get(pi).map(|p| p.id) else { return (0, 0) };
        // Ce qui sort du film reste récupérable : les images oubliées vont à la corbeille (`corbeille.rs`).
        if let Some(r) = self.reserves.get(&id).cloned() {
            let p = self.panels[pi].clone();
            self.jeter_images(&p, id, &r.head, "Contenu rogné oublié");
            self.jeter_images(&p, id, &r.tail, "Contenu rogné oublié");
        }
        self.reserves.remove(&id).map_or((0, 0), |r| (r.head.len, r.tail.len))
    }

    /// La fin rognée du plan `id` n'a plus de sens (le plan est coupé) : oubliée.
    pub(crate) fn forget_tail_reserve(&mut self, id: u64) {
        if let Some(r) = self.reserves.get_mut(&id) {
            r.tail = Piece::default();
            if r.is_empty() {
                self.reserves.remove(&id);
            }
        }
    }

    /// Les plans `a` et `b` (qui le suit) sont réunis : la fin rognée de `a` et le début rogné de `b` se retrouveraient au
    /// milieu du plan réuni, où rien ne les tient (la réserve n'a qu'un début et une fin) : ils sont oubliés, et leur nombre
    /// d'images est compté pour que le geste le dise (`images_oubliees`). Le début rogné de `a` et la fin rognée de `b` (qui
    /// deviennent ceux du plan réuni : `move_tail_reserve`) restent.
    pub(crate) fn forget_joint_reserves(&mut self, a: u64, b: &Panel) {
        let a = a;
        let b_id = b.id;
        let perdu = self.reserves.get(&a).map_or(0, |r| r.tail.len) + self.reserves.get(&b_id).map_or(0, |r| r.head.len);
        OUBLIEES.with(|c| c.set(c.get() + perdu));
        // Ce qui sort du film reste récupérable : ces images vont à la corbeille (`corbeille.rs`).
        if let (Some(pa), Some(r)) = (self.panels.iter().find(|p| p.id == a).cloned(), self.reserves.get(&a).map(|r| r.tail.clone())) {
            self.jeter_images(&pa, a, &r, "Images rognées au bord d'une jonction de plans");
        }
        if let Some(r) = self.reserves.get(&b_id).map(|r| r.head.clone()) {
            self.jeter_images(b, a, &r, "Images rognées au bord d'une jonction de plans");
        }
        let b = b_id;
        self.forget_tail_reserve(a);
        if let Some(r) = self.reserves.get_mut(&b) {
            r.head = Piece::default();
        }
    }

    /// La fin rognée du plan `from` devient celle du plan `to` (la seconde moitié d'un plan coupé, le plan réuni) : ses calques
    /// changent de numéro d'après `rename` (ancien numéro, nouveau), ses commentaires de plan. Un calque que `rename` ne
    /// connaît pas garde son numéro.
    pub(crate) fn move_tail_reserve(&mut self, from: u64, to: u64, rename: &std::collections::HashMap<u64, u64>) {
        let Some(r) = self.reserves.get_mut(&from) else { return };
        let mut tail = std::mem::take(&mut r.tail);
        if r.is_empty() {
            self.reserves.remove(&from);
        }
        if tail.len == 0 {
            return;
        }
        for lp in &mut tail.layers {
            if let Some(n) = rename.get(&lp.layer) {
                lp.layer = *n;
            }
        }
        for c in &mut tail.comments {
            c.panel = to;
        }
        self.reserves.entry(to).or_default().tail = tail;
    }

    /// Change la caméra de chaque clé rangée en réserve (la feuille change de taille).
    pub(crate) fn map_reserve_cameras(&mut self, f: impl Fn(&Camera) -> Camera) {
        for r in self.reserves.values_mut() {
            r.map_cameras(&f);
        }
    }

    /// Les réserves des plans qui existent encore (les autres ne s'enregistrent pas).
    pub fn live_reserves(&self) -> impl Iterator<Item = (usize, &Reserve)> {
        self.panels.iter().enumerate().filter_map(|(i, p)| self.reserves.get(&p.id).map(|r| (i, r)))
    }
}

// ------------------------------------------------------------------------ durée tapée

/// Lit une durée tapée : un nombre d'images (« 36 »), secondes et images (« 1:12 » ou « 1s12 »),
/// heures:minutes:secondes:images (« 0:00:01:12 »). Précédée de + ou -, elle s'ajoute à `current`
/// ou s'en retranche. Rien si le texte n'est pas une durée.
pub fn parse_duration(text: &str, fps: u32, current: usize) -> Option<usize> {
    let text = text.trim();
    let (sign, body) = match text.chars().next()? {
        '+' => (1_i64, &text[1..]),
        '-' => (-1_i64, &text[1..]),
        _ => (0_i64, text),
    };
    let body = body.trim();
    let fps = fps.max(1) as i64;
    let frames: i64 = if body.contains(':') {
        let parts: Vec<i64> = body.split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
        parts.iter().rev().enumerate().fold(0, |acc, (k, v)| {
            acc + match k {
                0 => *v,
                1 => v * fps,
                2 => v * fps * 60,
                _ => v * fps * 3600,
            }
        })
    } else if let Some((s, f)) = body.split_once(['s', 'S']) {
        let s: i64 = s.trim().parse().ok()?;
        let f: i64 = if f.trim().is_empty() { 0 } else { f.trim().parse().ok()? };
        s * fps + f
    } else {
        body.parse().ok()?
    };
    if frames < 0 {
        return None;
    }
    let total = if sign == 0 { frames } else { current as i64 + sign * frames };
    (total >= 1).then_some(total.min(MAX_PLAN as i64) as usize)
}

// ------------------------------------------------------------------------ aimantation

/// Les positions (en images du film, bords d'images) où un bord rogné s'aimante : les bords des
/// plans, la tête de lecture, les clés de caméra et d'effets (leur image et la suivante).
pub fn snap_points(doc: &Document, playhead: usize) -> Vec<usize> {
    let mut pts = vec![0, playhead];
    let mut start = 0;
    for p in &doc.panels {
        let len = p.frame_count();
        pts.push(start);
        pts.push(start + len);
        for k in &p.camera {
            pts.extend([start + k.frame, start + k.frame + 1]);
        }
        for l in &p.layers {
            for e in &l.fx {
                for k in &e.keys {
                    pts.extend([start + k.frame, start + k.frame + 1]);
                }
            }
        }
        start += len;
    }
    pts.sort_unstable();
    pts.dedup();
    pts
}

/// Le point de `pts` (trié) le plus proche de `pos` (en images, fractionnaire) à `within` images
/// ou moins.
pub fn snap_to(pts: &[usize], pos: f32, within: f32) -> Option<usize> {
    let i = pts.partition_point(|&p| (p as f32) < pos);
    [i.checked_sub(1), Some(i)]
        .into_iter()
        .flatten()
        .filter_map(|k| pts.get(k).copied())
        .map(|p| (p, (p as f32 - pos).abs()))
        .filter(|&(_, d)| d <= within)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(p, _)| p)
}

// ------------------------------------------------------------------------ fichier

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "rognages.json";
const FORMAT: &str = "shot-rognages";
const VERSION: u32 = 1;

mod file {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize, Default)]
    pub struct CamFile {
        #[serde(rename = "image", default)]
        pub frame: usize,
        pub x: f32,
        pub y: f32,
        pub zoom: f32,
        pub rotation: f32,
        #[serde(default)]
        pub avance: f32,
        #[serde(default)]
        pub bascule: f32,
        #[serde(default)]
        pub panoramique: f32,
        #[serde(default)]
        pub doux: bool,
        #[serde(default)]
        pub adoucir_entree: f32,
        #[serde(default)]
        pub adoucir_sortie: f32,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub struct FxFile {
        #[serde(default)]
        pub cles: Vec<FxKey>,
        #[serde(default)]
        pub bord: Option<Vec<f32>>,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub struct LayerFile {
        /// Rang du calque dans le plan.
        pub rang: usize,
        /// Expositions : (dessin, images).
        #[serde(default)]
        pub expositions: Vec<(Option<u64>, usize)>,
        #[serde(default)]
        pub roles: Roles,
        #[serde(default)]
        pub effets: Vec<FxFile>,
        /// Clés de position et d'opacité sorties par le début, et la clé du bord (rien d'écrit quand il n'y en a pas : les projets
        /// d'avant gardent leur fichier tel quel).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub cles_position: Vec<crate::poskeys::Entree>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bord_position: Option<crate::poskeys::Entree>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub cles_opacite: Vec<crate::opacitecles::Entree>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bord_opacite: Option<crate::opacitecles::Entree>,
        /// Départ et fin de la trajectoire du calque avant la coupure du début, et après (rien : pas de trajectoire).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trajectoire_avant: Option<(usize, Option<usize>)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trajectoire_apres: Option<(usize, Option<usize>)>,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub struct MarkFile {
        pub id: u64,
        pub image: usize,
        #[serde(default)]
        pub couleur: u8,
        #[serde(default)]
        pub action: String,
        #[serde(default)]
        pub dialogue: String,
        #[serde(default)]
        pub notes: String,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub struct PieceFile {
        #[serde(default)]
        pub images: usize,
        #[serde(default)]
        pub calques: Vec<LayerFile>,
        #[serde(default)]
        pub camera: Vec<CamFile>,
        #[serde(default)]
        pub camera_bord: Option<CamFile>,
        #[serde(default)]
        pub marques: Vec<MarkFile>,
        #[serde(default)]
        pub signets: Vec<usize>,
        #[serde(default)]
        pub entree: Option<usize>,
        #[serde(default)]
        pub sortie: Option<usize>,
        #[serde(default)]
        pub commentaires: Vec<Comment>,
        /// Marqueur : la partie a été écrite par un code qui range aussi les clés de position et d'opacité (absent des anciens
        /// fichiers : faux, ancienne règle).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub cles_v2: bool,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub struct PlanFile {
        pub plan: u64,
        #[serde(default)]
        pub debut: PieceFile,
        #[serde(default)]
        pub fin: PieceFile,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Sidecar {
        pub format: String,
        pub version: u32,
        #[serde(default)]
        pub plans: Vec<PlanFile>,
    }
}

fn cam_file(frame: usize, c: &Camera, key: Option<&CameraKey>) -> file::CamFile {
    file::CamFile {
        frame,
        x: c.x,
        y: c.y,
        zoom: c.zoom,
        rotation: c.angle,
        avance: c.dolly,
        bascule: c.tilt,
        panoramique: c.pan,
        doux: key.is_some_and(|k| k.smooth),
        adoucir_entree: key.map_or(0.0, |k| k.ease_in),
        adoucir_sortie: key.map_or(0.0, |k| k.ease_out),
    }
}

fn cam_of(c: &file::CamFile) -> Option<Camera> {
    let v = [c.x, c.y, c.zoom, c.rotation, c.avance, c.bascule, c.panoramique, c.adoucir_entree, c.adoucir_sortie];
    v.iter().all(|x| x.is_finite()).then(|| Camera {
        x: c.x,
        y: c.y,
        zoom: c.zoom.clamp(0.05, 50.0),
        angle: c.rotation,
        dolly: c.avance,
        tilt: c.bascule.clamp(-85.0, 85.0),
        pan: c.panoramique.clamp(-85.0, 85.0),
    })
}

fn exposures(frames: &[Option<CelId>]) -> Vec<(Option<u64>, usize)> {
    let mut out: Vec<(Option<u64>, usize)> = Vec::new();
    for &f in frames {
        match out.last_mut() {
            Some(last) if last.0 == f => last.1 += 1,
            _ => out.push((f, 1)),
        }
    }
    out
}

fn piece_file(piece: &Piece, panel: &Panel) -> file::PieceFile {
    file::PieceFile {
        images: piece.len,
        calques: piece
            .layers
            .iter()
            .filter_map(|lp| {
                let rang = panel.layers.iter().position(|l| l.id == lp.layer)?;
                Some(file::LayerFile {
                    rang,
                    expositions: exposures(&lp.frames),
                    roles: lp.roles.clone(),
                    effets: lp.fx.iter().map(|f| file::FxFile { cles: f.keys.clone(), bord: f.edge.clone() }).collect(),
                    cles_position: lp.pos.iter().map(Into::into).collect(),
                    bord_position: lp.pos_edge.as_ref().map(Into::into),
                    cles_opacite: lp.op.iter().map(Into::into).collect(),
                    bord_opacite: lp.op_edge.as_ref().map(Into::into),
                    trajectoire_avant: lp.traj_avant,
                    trajectoire_apres: lp.traj_apres,
                })
            })
            .collect(),
        camera: piece.camera.iter().map(|k| cam_file(k.frame, &k.cam, Some(k))).collect(),
        camera_bord: piece.cam_edge.as_ref().map(|c| cam_file(0, c, None)),
        marques: piece
            .marks
            .iter()
            .map(|m| file::MarkFile { id: m.id, image: m.frame, couleur: m.color, action: m.action.clone(), dialogue: m.dialogue.clone(), notes: m.notes.clone() })
            .collect(),
        signets: piece.bookmarks.clone(),
        entree: piece.in_point,
        sortie: piece.out_point,
        commentaires: piece.comments.clone(),
        cles_v2: piece.keys_v2,
    }
}

/// Le contenu de `rognages.json` pour `doc` ; rien s'il n'y a aucune réserve (le fichier est alors
/// retiré). Une réserve de calque supprimé, ou de plan disparu, n'est pas écrite.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let plans: Vec<file::PlanFile> = doc
        .live_reserves()
        .filter(|(_, r)| !r.is_empty())
        .map(|(i, r)| file::PlanFile { plan: doc.panels[i].id, debut: piece_file(&r.head, &doc.panels[i]), fin: piece_file(&r.tail, &doc.panels[i]) })
        .collect();
    if plans.is_empty() {
        return Ok(None);
    }
    let sidecar = file::Sidecar { format: FORMAT.into(), version: VERSION, plans };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Le contenu rogné d'un seul plan, au format de `rognages.json` : un plan partagé le porte dans son propre fichier
/// (`projet_partage.rs`).
pub(crate) fn plan_value(panel: &Panel, r: &Reserve) -> Result<serde_json::Value, String> {
    let plan = file::PlanFile { plan: panel.id, debut: piece_file(&r.head, panel), fin: piece_file(&r.tail, panel) };
    serde_json::to_value(file::Sidecar { format: FORMAT.into(), version: VERSION, plans: vec![plan] }).map_err(|e| e.to_string())
}

/// Le fichier lu, pas encore rangé dans un document (les calques y sont par rang).
pub(crate) struct Loaded(Vec<file::PlanFile>);

impl Loaded {
    /// Plusieurs fichiers de plans en un (les plans d'un projet partagé).
    pub(crate) fn fusion(parts: Vec<Loaded>) -> Loaded {
        Loaded(parts.into_iter().flat_map(|l| l.0).collect())
    }

    /// Les dessins cités (à charger avec les autres).
    pub(crate) fn cels(&self) -> impl Iterator<Item = CelId> + '_ {
        self.0.iter().flat_map(|p| [&p.debut, &p.fin]).flat_map(|f| f.calques.iter()).flat_map(|l| l.expositions.iter()).filter_map(|e| e.0)
    }

    /// Les réserves lues pour ces plans (hors du film : ceux d'une variante rangée, `variantes.rs`), qui ne sont pas
    /// dans un document ; `missing` : dessins illisibles. Les réserves vides sont laissées.
    pub(crate) fn pour_plans(self, panels: &[&Panel], missing: &std::collections::BTreeSet<CelId>) -> Vec<(u64, Reserve)> {
        let mut out = Vec::new();
        for plan in self.0 {
            let Some(panel) = panels.iter().find(|p| p.id == plan.plan) else { continue };
            let r = Reserve { head: piece_of(plan.debut, panel, missing), tail: piece_of(plan.fin, panel, missing) };
            if !r.is_empty() {
                out.push((plan.plan, r));
            }
        }
        out
    }

    /// Range les réserves dans `doc` (dont les plans sont chargés) ; `missing` : dessins illisibles.
    pub(crate) fn install(self, doc: &mut Document, missing: &std::collections::BTreeSet<CelId>) {
        for plan in self.0 {
            let Some(panel) = doc.panels.iter().find(|p| p.id == plan.plan) else { continue };
            let head = piece_of(plan.debut, panel, missing);
            let tail = piece_of(plan.fin, panel, missing);
            let r = Reserve { head, tail };
            if !r.is_empty() {
                // Les numéros des marques et des commentaires rangés restent à eux.
                let ids: Vec<u64> = [&r.head, &r.tail]
                    .into_iter()
                    .flat_map(|p| p.marks.iter().map(|m| m.id).chain(p.comments.iter().map(|c| c.id)))
                    .collect();
                doc.reserves.insert(plan.plan, r);
                for id in ids {
                    doc.reserve_id(id);
                }
            }
        }
    }
}

fn piece_of(f: file::PieceFile, panel: &Panel, missing: &std::collections::BTreeSet<CelId>) -> Piece {
    let len = f.images.min(MAX_PLAN);
    if len == 0 {
        return Piece::default();
    }
    let layers = f
        .calques
        .into_iter()
        .filter_map(|l| {
            let layer = panel.layers.get(l.rang)?;
            let frames: Vec<Option<CelId>> = l
                .expositions
                .iter()
                .flat_map(|(c, n)| std::iter::repeat_n(c.filter(|id| !missing.contains(id)), (*n).min(MAX_PLAN)))
                .take(len)
                .collect();
            Some(LayerPiece {
                layer: layer.id,
                frames,
                roles: l.roles.into_iter().filter(|(f, _)| *f < len).collect(),
                fx: l.effets.into_iter().map(|e| FxPiece { keys: e.cles, edge: e.bord }).collect(),
                pos: crate::poskeys::from_entries(l.cles_position),
                pos_edge: l.bord_position.and_then(crate::poskeys::Entree::into_model),
                op: crate::opacitecles::from_entries(l.cles_opacite),
                op_edge: l.bord_opacite.and_then(crate::opacitecles::Entree::into_model),
                traj_avant: l.trajectoire_avant,
                traj_apres: l.trajectoire_apres,
            })
        })
        .collect();
    let camera = f
        .camera
        .iter()
        .filter_map(|k| cam_of(k).map(|cam| CameraKey { frame: k.frame, cam, smooth: k.doux, ease_in: k.adoucir_entree.max(0.0), ease_out: k.adoucir_sortie.max(0.0) }))
        .collect();
    Piece {
        len,
        layers,
        camera,
        cam_edge: f.camera_bord.as_ref().and_then(cam_of),
        marks: f
            .marques
            .into_iter()
            .map(|m| ImageMark { id: m.id, frame: m.image, color: m.couleur, action: m.action, dialogue: m.dialogue, notes: m.notes })
            .collect(),
        bookmarks: f.signets,
        in_point: f.entree,
        out_point: f.sortie,
        comments: f.commentaires.into_iter().map(|c| Comment { panel: panel.id, ..c }).collect(),
        keys_v2: f.cles_v2,
    }
}

/// Lit le texte de `rognages.json`.
pub(crate) fn parse(text: &str) -> Result<Loaded, String> {
    let sidecar: file::Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de rognages".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    Ok(Loaded(sidecar.plans))
}

/// Les noms d'une étape d'annulation de rognage.
pub mod noms {
    pub const DEBUT_COURT: &str = "Raccourcir le début";
    pub const DEBUT_LONG: &str = "Allonger le début";
    pub const FIN_COURTE: &str = "Raccourcir la fin";
    pub const FIN_LONGUE: &str = "Allonger la fin";
    pub const COUPE: &str = "Déplacer la coupe";
    pub const GLISSER: &str = "Glisser le contenu";
    pub const DUREE: &str = "Durée du plan";
    pub const DUREE_CLES_LONGUE: &str = "Allonger le plan, clés décalées";
    pub const DUREE_CLES_COURTE: &str = "Raccourcir le plan, clés décalées";
    pub const RETABLIR: &str = "Rétablir le contenu rogné";
    pub const OUBLIER: &str = "Oublier le contenu rogné";
}
