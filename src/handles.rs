//! Poignées de la timeline du plan (passe interface, demandée par Anthony le 2026-10-16) : on y
//! prend à la souris et on fait glisser dans le temps les marques d'image, les signets, les
//! points d'entrée et de sortie et les clés d'effets ; le bord droit d'un dessin étire ou
//! raccourcit son exposition (ce qui suit glisse d'autant). Une étape d'annulation par geste,
//! seulement si quelque chose a bougé ; Échap (ou Ctrl+Z) pendant le geste remet tout comme
//! avant. (Les clés de caméra se prenaient déjà : `camtools`.)
//!
//! Poignées de calque (chantier 1 de l'audit, choix d'Anthony le 2026-10-16) : la barre sous les
//! dessins d'un calque le décale dans le temps ; son début et sa fin se rognent ou s'allongent
//! (allonger tient le premier ou le dernier dessin). Ces poignées ne changent jamais la durée du
//! plan : elles s'arrêtent à sa fin, et un calque raccourci la laisse telle quelle. La durée se
//! change par sa propre poignée, au bout de la règle (chantier 4), qui prévient quand les plans
//! suivants se décalent par rapport au son.
//!
//! Depuis le montage (Anthony, 2026-10-17 : « tout doit pouvoir être fait depuis le banc de
//! montage ») : un double-clic sur la piste d'un calque y pose un nouveau dessin, une clé
//! d'animation (`key_at`) ; un dessin pris par son milieu se place avant ou après les autres,
//! chacun gardant sa durée (`reorder_run`, `Handle::Key`) ; la limite entre deux dessins se
//! déplace, l'un s'allonge, l'autre raccourcit (`move_boundary`, `Handle::Boundary`). Une clé se
//! colle (`overwrite_run`), se supprime (sa voisine la remplace ou elle laisse un vide,
//! `remove_run`), se duplique sur place (`duplicate_run`) ; le choix de plusieurs clés et ces
//! commandes sont dans `montagekeys.rs`.

use eframe::egui;

use crate::app::App;
use crate::document::{CelId, Document, ImageMark, Layer, Panel};
use crate::fx::FxKey;
use crate::history::Edit;

/// Ce que l'on tient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handle {
    /// Marque d'image du plan (son rang dans `Panel::marks`).
    Mark(usize),
    /// Signet (son rang dans `Cues::bookmarks`).
    Bookmark(usize),
    In,
    Out,
    /// Clé d'effet : calque, effet, rang de la clé.
    FxKey { layer: usize, effect: usize, key: usize },
    /// Bord droit du dessin exposé de `start` à `end` (exclu) sur le calque `layer`.
    Exposure { layer: usize, start: usize, end: usize },
    /// Début du calque (sa première image dessinée).
    LayerStart { layer: usize },
    /// Fin du calque (après sa dernière image dessinée).
    LayerEnd { layer: usize },
    /// Le calque entier, pris par sa barre à l'image `grab`.
    LayerMove { layer: usize, grab: usize },
    /// Fin du plan : sa durée.
    PlanEnd,
    /// Le dessin exposé de `start` à `end` (exclu) sur le calque `layer`, pris par son milieu à
    /// l'image `grab` : il se place avant ou après les autres (montage).
    Key { layer: usize, start: usize, end: usize, grab: usize },
    /// La limite, à l'image `at`, entre deux expositions du calque `layer` : l'une s'allonge,
    /// l'autre raccourcit (montage).
    Boundary { layer: usize, at: usize },
}

impl Handle {
    /// Une poignée qui change les dessins d'un calque : elle ne change pas la durée du plan.
    pub(crate) fn layer(self) -> Option<usize> {
        match self {
            Handle::Exposure { layer, .. }
            | Handle::LayerStart { layer }
            | Handle::LayerEnd { layer }
            | Handle::LayerMove { layer, .. }
            | Handle::Key { layer, .. }
            | Handle::Boundary { layer, .. } => Some(layer),
            _ => None,
        }
    }
}

/// Où l'on a cliqué droit dans la timeline du plan (en dehors des cases des calques, qui ont
/// leur menu des rôles et des images).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridMenu {
    /// La règle (l'image courante vient sous le clic).
    Ruler,
    /// La ligne de la caméra, ou son nom.
    Camera,
    /// Une clé d'effet (le calque et l'effet deviennent courants).
    FxKey,
    /// La ligne du son : la piste et l'extrait sous le clic, l'image du montage.
    Sound { track: usize, clip: Option<u64>, frame: usize },
    /// Le nom de la ligne du son.
    SoundHeader,
    /// Le nom de la ligne des notes.
    NotesHeader,
}

/// Une poignée tenue : le plan (id), le plan d'avant (pour Échap et pour savoir si quelque chose
/// a bougé) et l'étape d'annulation de tout le geste.
pub(crate) struct HandleDrag {
    pub handle: Handle,
    panel: u64,
    before: Panel,
    snap: Edit,
}

/// `f` gardé entre les voisins `prev` et `next` (exclus) : l'ordre ne change pas.
pub fn between(f: usize, prev: Option<usize>, next: Option<usize>) -> usize {
    let lo = prev.map_or(0, |p| p + 1);
    let hi = next.map_or(usize::MAX, |n| n.saturating_sub(1));
    f.clamp(lo, hi.max(lo))
}

/// La marque `i` amenée vers l'image `frame` (entre ses voisines, jamais sur la première image
/// du plan, avant la fin `len`). Renvoie où elle est.
pub fn move_mark(marks: &mut [ImageMark], i: usize, frame: usize, len: usize) -> usize {
    let prev = i.checked_sub(1).map(|p| marks[p].frame).or(Some(0));
    let next = marks.get(i + 1).map(|m| m.frame).or(Some(len.max(2)));
    let f = between(frame, prev, next);
    marks[i].frame = f;
    f
}

/// Le signet `i` (signets triés) amené vers l'image `frame`, entre ses voisins.
pub fn move_bookmark(bookmarks: &mut [usize], i: usize, frame: usize, len: usize) -> usize {
    let prev = i.checked_sub(1).map(|p| bookmarks[p]);
    let next = bookmarks.get(i + 1).copied().or(Some(len.max(1)));
    let f = between(frame, prev, next);
    bookmarks[i] = f;
    f
}

/// La clé `i` d'un effet (clés triées) amenée vers l'image `frame`, entre ses voisines.
pub fn move_fx_key(keys: &mut [FxKey], i: usize, frame: usize) -> usize {
    let prev = i.checked_sub(1).map(|p| keys[p].frame);
    let next = keys.get(i + 1).map(|k| k.frame);
    let f = between(frame, prev, next);
    keys[i].frame = f;
    f
}

/// Le dessin exposé de `start` à `end` (exclu) dure maintenant `len` images (au moins une) ;
/// les images d'après, leurs rôles et les clés d'effets glissent d'autant.
pub fn set_run_length(layer: &mut Layer, start: usize, end: usize, len: usize) {
    let len = len.max(1);
    let old = end.saturating_sub(start);
    if start >= layer.frames.len() || old == 0 {
        return;
    }
    if len > old {
        let cel = layer.frames[start];
        layer.insert(end.min(layer.frames.len()), cel, len - old);
    } else if len < old {
        let a = start + len;
        let b = end.min(layer.frames.len());
        layer.frames.drain(a..b);
        layer.shift_frames(a, b - a, true);
    }
    layer.trim();
}

/// Le contenu d'un calque : de sa première image dessinée à la fin de sa dernière (exclue) ;
/// aucun s'il n'a pas de dessin.
pub fn content_bounds(layer: &Layer) -> Option<(usize, usize)> {
    let first = layer.frames.iter().position(Option::is_some)?;
    let last = layer.frames.iter().rposition(Option::is_some)?;
    Some((first, last + 1))
}

/// Le début du calque amené à l'image `to` (avant sa fin) : plus tôt, son premier dessin est
/// tenu dès `to` ; plus tard, les images d'avant `to` sont vidées. La suite ne bouge pas.
pub fn set_layer_start(layer: &mut Layer, to: usize) {
    let Some((a, b)) = content_bounds(layer) else { return };
    let to = to.min(b - 1);
    if to < a {
        let cel = layer.frames[a];
        for f in &mut layer.frames[to..a] {
            *f = cel;
        }
    } else if to > a {
        for f in &mut layer.frames[a..to] {
            *f = None;
        }
        layer.roles.retain(|f, _| *f < a || *f >= to);
    }
}

/// La fin du calque amenée à `to` (après son début ; au plus `max`, la fin du plan, sauf si le
/// calque allait déjà plus loin) : plus loin, son dernier dessin est tenu jusque-là ; plus près,
/// les images d'après sont coupées.
pub fn set_layer_end(layer: &mut Layer, to: usize, max: usize) {
    let Some((a, b)) = content_bounds(layer) else { return };
    let to = to.clamp(a + 1, max.max(b));
    layer.frames.truncate(b);
    if to > b {
        let cel = layer.frames[b - 1];
        layer.frames.resize(to, cel);
    } else if to < b {
        layer.frames.truncate(to);
        layer.roles.retain(|f, _| *f < to);
    }
    layer.trim();
}

/// Le calque entier décalé de `delta` images, sans passer avant la première image du plan ni
/// après `max`, sa fin (sauf s'il allait déjà plus loin) : ses dessins, leurs rôles et les clés
/// d'effets suivent. Renvoie le décalage fait.
pub fn move_layer(layer: &mut Layer, delta: isize, max: usize) -> isize {
    let Some((a, b)) = content_bounds(layer) else { return 0 };
    let room = max.saturating_sub(b) as isize;
    let d = delta.clamp(-(a as isize), room);
    if d > 0 {
        layer.insert(0, None, d as usize);
    } else if d < 0 {
        let n = d.unsigned_abs();
        layer.frames.drain(0..n);
        layer.shift_frames(0, n, true);
    }
    layer.trim();
    d
}

/// Un dessin commence déjà à l'image `f` du calque : on n'y pose pas de clé.
pub fn key_start_taken(layer: &Layer, f: usize) -> bool {
    f < layer.frames.len() && layer.frames[f].is_some() && layer.run_bounds(f).0 == f
}

/// Un nouveau dessin `cel` posé à l'image `f` (une clé d'animation) : au milieu d'une
/// exposition, elle est coupée là et la suite montre le nouveau dessin ; sur une image vide (ou
/// après la fin du calque), il tient jusqu'au dessin suivant, au plus jusqu'à `max` (la fin du
/// plan). Rien si un dessin commence déjà à `f`. Renvoie la fin (exclue) du nouveau dessin.
pub fn key_at(layer: &mut Layer, f: usize, cel: CelId, max: usize) -> Option<usize> {
    if key_start_taken(layer, f) {
        return None;
    }
    let end = if f < layer.frames.len() && layer.frames[f].is_some() {
        layer.run_bounds(f).1
    } else {
        let mut e = f + 1;
        while e < max.max(f + 1) && layer.frames.get(e).copied().flatten().is_none() {
            e += 1;
        }
        e
    };
    for x in f..end {
        layer.set(x, Some(cel));
    }
    Some(end)
}

/// Les expositions du calque bout à bout, jusqu'à sa fin : (début, fin exclue, dessin) ; une
/// plage vide en est une aussi (dessin `None`).
pub fn runs(layer: &Layer) -> Vec<(usize, usize, Option<CelId>)> {
    let mut out = Vec::new();
    let mut f = 0;
    while f < layer.frames.len() {
        let (s, e) = layer.run_bounds(f);
        out.push((s, e, layer.frames[f]));
        f = e;
    }
    out
}

/// Rang que prendrait, parmi les autres expositions, le dessin exposé à partir de `start` s'il
/// commençait à `new_start` : le nombre d'autres expositions dont le milieu est avant le sien.
pub fn reorder_slot(layer: &Layer, start: usize, new_start: isize) -> usize {
    let all = runs(layer);
    let Some(&(s, e, _)) = all.iter().find(|r| r.0 == start) else { return 0 };
    let center = new_start as f32 + (e - s) as f32 * 0.5;
    all.iter().filter(|r| r.0 != s && (r.0 + r.1) as f32 * 0.5 < center).count()
}

/// Le dessin exposé à partir de `start` placé au rang `slot` parmi les autres expositions (0 :
/// en premier) : chacune garde sa durée, la durée du calque ne change pas, les rôles suivent
/// leurs images. Renvoie le nouveau début du dessin (rien s'il n'y a pas de dessin à `start`).
pub fn reorder_run(layer: &mut Layer, start: usize, slot: usize) -> Option<usize> {
    let mut order = runs(layer);
    let i = order.iter().position(|r| r.0 == start && r.2.is_some())?;
    let moved = order.remove(i);
    order.insert(slot.min(order.len()), moved);
    let mut frames = Vec::with_capacity(layer.frames.len());
    let mut roles = crate::roles::Roles::new();
    let mut new_start = start;
    for &(s, e, cel) in &order {
        let at = frames.len();
        if s == moved.0 {
            new_start = at;
        }
        roles.extend(layer.roles.range(s..e).map(|(f, r)| (at + (f - s), *r)));
        frames.extend(std::iter::repeat_n(cel, e - s));
    }
    layer.frames = frames;
    layer.roles = roles;
    Some(new_start)
}

/// La limite entre deux expositions, à l'image `at`, amenée à l'image `to` : celle de gauche
/// s'allonge et celle de droite raccourcit d'autant, ou l'inverse (un dessin garde au moins une
/// image, une plage vide peut disparaître) ; la suite ne bouge pas. Le rôle posé au début de
/// l'exposition de droite la suit. Renvoie où est la limite.
pub fn move_boundary(layer: &mut Layer, at: usize, to: usize) -> usize {
    if at == 0 || at >= layer.frames.len() || layer.frames[at - 1] == layer.frames[at] {
        return at;
    }
    let (left, right) = (layer.frames[at - 1], layer.frames[at]);
    let lo = layer.run_bounds(at - 1).0 + usize::from(left.is_some());
    let hi = layer.run_bounds(at).1 - usize::from(right.is_some());
    let to = to.clamp(lo, hi.max(lo));
    if to == at {
        return at;
    }
    if to > at {
        layer.frames[at..to].fill(left);
    } else {
        layer.frames[to..at].fill(right);
    }
    let role = layer.roles.remove(&at);
    let (a, b) = (at.min(to), at.max(to));
    layer.roles.retain(|f, _| !(a..b).contains(f));
    if let Some(r) = role {
        layer.roles.insert(to, r);
    }
    layer.trim();
    to
}
/// Le dessin exposé à partir de `start` dupliqué sur place : la copie `cel` prend la seconde
/// moitié de son exposition, le dessin garde la première ; la suite ne bouge pas. Rien si
/// l'exposition n'a qu'une image. Renvoie où commence la copie.
pub fn duplicate_run(layer: &mut Layer, start: usize, cel: CelId) -> Option<usize> {
    layer.frames.get(start).copied().flatten()?;
    let (s, e) = layer.run_bounds(start);
    if e - s < 2 {
        return None;
    }
    let mid = s + (e - s).div_ceil(2);
    layer.frames[mid..e].fill(Some(cel));
    Some(mid)
}
/// Le dessin `cel` posé sur `n` images à partir de l'image `f` (une clé collée) : ce qui y était
/// est remplacé, la suite ne bouge pas ; les rôles de ces images partent.
pub fn overwrite_run(layer: &mut Layer, f: usize, n: usize, cel: CelId) {
    if n == 0 {
        return;
    }
    if f + n > layer.frames.len() {
        layer.frames.resize(f + n, None);
    }
    layer.frames[f..f + n].fill(Some(cel));
    layer.roles.retain(|x, _| !(f..f + n).contains(x));
}

/// La clé exposée à l'image `f` supprimée. `gap` : ses images restent vides, les autres clés ne
/// bougent pas. Sinon le dessin d'avant (juste avant elle) la remplace en tenant plus longtemps,
/// ou à défaut celui d'après, qui commence plus tôt ; sans voisin, un vide. Renvoie ses bornes
/// [début, fin), rien s'il n'y a pas de dessin à `f`.
pub fn remove_run(layer: &mut Layer, f: usize, gap: bool) -> Option<(usize, usize)> {
    if f >= layer.frames.len() || layer.frames[f].is_none() {
        return None;
    }
    let (s, e) = layer.run_bounds(f);
    let before = if s > 0 { layer.frames[s - 1] } else { None };
    let after = layer.frames.get(e).copied().flatten();
    let fill = if gap { None } else { before.or(after) };
    layer.frames[s..e].fill(fill);
    layer.roles.retain(|x, _| !(s..e).contains(x));
    // Le dessin d'après commence plus tôt : le rôle de son début le suit.
    if !gap && before.is_none() && after.is_some() {
        if let Some(r) = layer.roles.remove(&e) {
            layer.roles.insert(s, r);
        }
    }
    layer.trim();
    Some((s, e))
}

/// Changer la durée du plan `i` de `old` à `new` images décale les plans suivants : s'il y a
/// du son à partir de là, la phrase qui prévient (le son, lui, ne bouge pas).
pub fn sound_shift_note(doc: &Document, i: usize, old: usize, new: usize) -> Option<String> {
    if old == new || i >= doc.panels.len() {
        return None;
    }
    let at = doc.panel_start(i) + old.min(new);
    let sound_after = doc.audio_tracks.iter().any(|t| t.clips.iter().any(|c| c.end() > at));
    let d = new as isize - old as isize;
    sound_after.then(|| {
        format!(
            "Les plans suivants se décalent de {d:+} image{} par rapport au son, qui reste en place.",
            if d.abs() > 1 { "s" } else { "" }
        )
    })
}

impl App {
    /// Prend une poignée du plan courant (refusé sous le verrou du storyboard, sauf les signets
    /// et les points d'entrée et de sortie).
    pub(crate) fn begin_handle_drag(&mut self, handle: Handle) -> bool {
        let refused = match handle {
            Handle::Mark(_) | Handle::PlanEnd => self.storyboard_refuses(),
            Handle::FxKey { .. }
            | Handle::Exposure { .. }
            | Handle::LayerStart { .. }
            | Handle::LayerEnd { .. }
            | Handle::LayerMove { .. }
            | Handle::Key { .. }
            | Handle::Boundary { .. } => self.content_refuses(),
            _ => false,
        };
        if refused {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let before = self.panel().clone();
        self.handle_drag = Some(HandleDrag { handle, panel: before.id, before, snap });
        true
    }

    /// La poignée tenue suit la souris : `frame`, l'image sous elle ; `edge`, le bord d'image le
    /// plus proche (pour une exposition).
    pub(crate) fn drag_handle(&mut self, frame: usize, edge: usize) {
        let Some(d) = &self.handle_drag else { return };
        let (handle, id) = (d.handle, d.panel);
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == id) else {
            self.handle_drag = None;
            return;
        };
        let len = self.doc.panels[pi].frame_count();
        let label = |app: &App, f: usize| app.frame_label(f);
        let status = match handle {
            Handle::Mark(i) => {
                let f = move_mark(&mut self.doc.panels[pi].marks, i, frame, len);
                format!("Marque d'image : image {}.", label(self, f))
            }
            Handle::Bookmark(i) => {
                let f = move_bookmark(&mut self.doc.panels[pi].cues.bookmarks, i, frame, len);
                format!("Signet : image {}.", label(self, f))
            }
            // L'entrée reste avant la sortie, et inversement (jamais sur la même image, comme
            // quand on les pose) ; la sortie est la dernière image jouée.
            Handle::In => {
                let cues = &mut self.doc.panels[pi].cues;
                let last = cues.out_point.map_or(len.saturating_sub(1), |o| o.saturating_sub(1));
                let f = frame.min(last);
                cues.in_point = Some(f);
                format!("Point d'entrée : image {}.", label(self, f))
            }
            Handle::Out => {
                let cues = &mut self.doc.panels[pi].cues;
                let first = cues.in_point.map_or(0, |i| i + 1);
                let f = frame.min(len.saturating_sub(1)).max(first);
                cues.out_point = Some(f);
                format!("Point de sortie : image {}.", label(self, f))
            }
            Handle::FxKey { layer, effect, key } => {
                let Some(e) = self.doc.panels[pi].layers.get_mut(layer).and_then(|l| l.fx.get_mut(effect)) else { return };
                let f = move_fx_key(&mut e.keys, key, frame);
                self.renderer.invalidate_all();
                format!("Clé d'effet : image {}.", label(self, f))
            }
            Handle::Exposure { layer, start, end } => {
                let Some((mut l, plan_len)) = self.layer_before(layer) else { return };
                // Ce qui suit glisse d'autant, sans dépasser la fin du plan.
                let room = plan_len.saturating_sub(l.frames.len());
                let want = edge.saturating_sub(start).max(1);
                let n = want.min(end - start + room);
                set_run_length(&mut l, start, end, n);
                self.put_layer(pi, layer, l, plan_len);
                let s = if n > 1 { "s" } else { "" };
                if want > n {
                    format!("Exposition : {n} image{s} (fin du plan : l'allonger d'abord par la poignée au bout de la règle).")
                } else {
                    format!("Exposition : {n} image{s}.")
                }
            }
            Handle::LayerStart { layer } => {
                let Some((mut l, plan_len)) = self.layer_before(layer) else { return };
                set_layer_start(&mut l, edge);
                let a = content_bounds(&l).map_or(0, |(a, _)| a);
                self.put_layer(pi, layer, l, plan_len);
                format!("Début du calque : image {}.", label(self, a))
            }
            Handle::LayerEnd { layer } => {
                let Some((mut l, plan_len)) = self.layer_before(layer) else { return };
                let reached = edge > plan_len;
                set_layer_end(&mut l, edge, plan_len);
                let b = content_bounds(&l).map_or(0, |(_, b)| b);
                self.put_layer(pi, layer, l, plan_len);
                let end = format!("Fin du calque : image {}", label(self, b.saturating_sub(1)));
                if reached {
                    format!("{end} (fin du plan : l'allonger d'abord par la poignée au bout de la règle).")
                } else {
                    format!("{end}.")
                }
            }
            Handle::LayerMove { layer, grab } => {
                let Some((mut l, plan_len)) = self.layer_before(layer) else { return };
                let d = move_layer(&mut l, frame as isize - grab as isize, plan_len);
                self.put_layer(pi, layer, l, plan_len);
                format!("Calque décalé de {d:+} image{}.", if d.abs() > 1 { "s" } else { "" })
            }
            Handle::Key { layer, start, end, grab } => {
                let Some((mut l, plan_len)) = self.layer_before(layer) else { return };
                let slot = reorder_slot(&l, start, frame as isize - (grab as isize - start as isize));
                let s = reorder_run(&mut l, start, slot).unwrap_or(start);
                self.put_layer(pi, layer, l, plan_len);
                let n = end - start;
                format!("Dessin placé images {} à {} ({n} image{}, sa durée est gardée).", label(self, s), label(self, s + n - 1), if n > 1 { "s" } else { "" })
            }
            Handle::Boundary { layer, at } => {
                let Some((mut l, plan_len)) = self.layer_before(layer) else { return };
                let to = move_boundary(&mut l, at, edge);
                let before = to - l.run_bounds(to.saturating_sub(1)).0;
                let after = if to < l.frames.len() { l.run_bounds(to).1 - to } else { 0 };
                self.put_layer(pi, layer, l, plan_len);
                format!("Expositions : {before} image{} avant la limite, {after} après.", if before > 1 { "s" } else { "" })
            }
            Handle::PlanEnd => {
                let Some(before) = self.handle_drag.as_ref().map(|d| d.before.clone()) else { return };
                let (old, n) = (before.frame_count(), edge.max(1));
                let mut p = before;
                p.set_length(n);
                self.doc.panels[pi] = p;
                self.renderer.invalidate_all();
                let base = format!("Durée du plan : {}", crate::storyboard::duration_label(n, self.doc.fps));
                match sound_shift_note(&self.doc, pi, old, n) {
                    Some(note) => format!("{base}. {note}"),
                    None => format!("{base}."),
                }
            }
        };
        self.status = status;
    }

    /// Nouveau dessin (clé d'animation) posé depuis le montage : plan `pi`, calque `li`, image
    /// `f` du plan. Le plan, le calque et l'image deviennent courants : on dessine la clé tout
    /// de suite. Une étape d'annulation.
    pub(crate) fn montage_new_key(&mut self, pi: usize, li: usize, f: usize) {
        if pi >= self.doc.panels.len() {
            return;
        }
        self.set_panel(pi);
        self.select_layer(li);
        if self.refuse_on_folder() {
            return;
        }
        let len = self.doc.panels[pi].frame_count();
        let f = f.min(len.saturating_sub(1));
        let Some((kind, video, taken)) = self.doc.panels[pi].layers.get(li).map(|l| (l.kind, l.video.is_some(), key_start_taken(l, f))) else { return };
        if video {
            self.status = "Un calque vidéo n'a pas de dessins.".into();
            return;
        }
        if taken {
            self.cur_frame = f;
            self.status = "Un dessin commence déjà ici : double-cliquer plus loin dans son exposition pour poser une nouvelle clé.".into();
            return;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let cel = self.doc.new_cel(kind);
        let end = key_at(&mut self.doc.panels[pi].layers[li], f, cel, len).unwrap_or(f + 1);
        self.cur_frame = f;
        self.vsel = None;
        self.commit_structure(snap);
        let name = self.doc.panels[pi].layers[li].name.clone();
        self.status = format!("Nouveau dessin sur « {name} » : images {} à {}. On peut le dessiner tout de suite.", self.frame_label(f), self.frame_label(end - 1));
    }

    /// Le calque `layer` tel qu'il était au début du geste, et la durée du plan d'alors.
    fn layer_before(&self, layer: usize) -> Option<(Layer, usize)> {
        let d = self.handle_drag.as_ref()?;
        Some((d.before.layers.get(layer)?.clone(), d.before.frame_count()))
    }

    /// Pose le calque changé par une poignée : le plan garde sa durée d'avant le geste.
    fn put_layer(&mut self, pi: usize, layer: usize, l: Layer, plan_len: usize) {
        let p = &mut self.doc.panels[pi];
        p.layers[layer] = l;
        if p.frame_count() < plan_len {
            p.length = plan_len;
        }
        self.renderer.invalidate_all();
    }

    /// Lâche la poignée : une étape d'annulation si quelque chose a bougé.
    pub(crate) fn end_handle_drag(&mut self) {
        let Some(d) = self.handle_drag.take() else { return };
        let Some(p) = self.doc.panels.iter().find(|p| p.id == d.panel) else { return };
        let changed = match d.handle {
            Handle::Mark(_) => p.marks != d.before.marks,
            Handle::Bookmark(_) | Handle::In | Handle::Out => p.cues != d.before.cues,
            Handle::PlanEnd => p.frame_count() != d.before.frame_count(),
            h => {
                let layer = h.layer().or(match h {
                    Handle::FxKey { layer, .. } => Some(layer),
                    _ => None,
                });
                layer.is_some_and(|layer| {
                    p.layers.get(layer).map(|l| (&l.frames, &l.fx, &l.roles)) != d.before.layers.get(layer).map(|l| (&l.frames, &l.fx, &l.roles))
                })
            }
        };
        if changed {
            self.history.push(d.snap);
            self.modified = true;
            // La note du son reste lisible après le geste ; la durée vue est à jour.
            self.plan_len_seen = Some((d.panel, p.frame_count()));
        } else if let Some(p) = self.doc.panels.iter_mut().find(|p| p.id == d.panel) {
            // Rien n'a changé : le plan reprend exactement son état (durée minimale comprise).
            *p = d.before;
        }
    }

    /// Le menu d'un clic droit dans la timeline du plan (hors cases des calques). Chaque entrée
    /// est une commande : un clic droit dessus lui donne un raccourci.
    pub(crate) fn grid_menu_ui(&mut self, ui: &mut egui::Ui, m: GridMenu) {
        match m {
            GridMenu::Ruler => {
                self.action_item(ui, "Signet ici (poser / retirer)", "signet.basculer");
                self.action_item(ui, "Marque d'image ici (poser / retirer)", "marque.basculer");
                ui.separator();
                self.action_item(ui, "Point d'entrée ici", "entree.poser");
                self.action_item(ui, "Point de sortie ici", "sortie.poser");
                self.action_item(ui, "Enlever l'entrée et la sortie", "entree_sortie.enlever");
                ui.separator();
                self.action_item(ui, "Adapter la timeline au plan", "timeline.adapter_plan");
                self.action_item(ui, "Timeline : tout voir", "timeline.tout_voir");
            }
            GridMenu::Camera => {
                self.action_item(ui, "Image clé de caméra ici", "camera.cle_ici");
                self.action_item(ui, "Retirer la clé de caméra d'ici", "camera.cle_supprimer");
                self.action_item(ui, "Adoucir le mouvement à cette clé", "camera.cle_adoucir");
                ui.separator();
                self.action_item(ui, "Clé au début du plan", "camera.cle_debut");
                self.action_item(ui, "Clé à la fin du plan", "camera.cle_fin");
                self.action_item(ui, "Adapter la timeline aux clés", "timeline.adapter_cles");
                ui.separator();
                self.action_item(ui, "Vue à travers la caméra", "camera.vue");
                self.action_item(ui, "Réinitialiser la caméra", "camera.reinitialiser");
            }
            GridMenu::FxKey => {
                self.action_item(ui, "Supprimer cette clé d'effet", "fx.cle_supprimer");
                self.action_item(ui, "Pile d'effets du calque…", "fx.panneau");
                self.action_item(ui, "Clés d'effets dans la timeline (montrer / cacher)", "fx.cles_timeline");
            }
            GridMenu::Sound { track, clip, frame } => self.lane_menu(ui, track, clip, frame),
            GridMenu::SoundHeader => {
                self.action_item(ui, "Importer un son…", "son.importer");
                self.action_item(ui, "Nouvelle piste son", "son.nouvelle_piste");
                self.action_item(ui, "Couper le son à l'image courante", "son.couper");
                ui.separator();
                self.action_item(ui, "Son au défilement (activer / couper)", "son.defilement");
                self.action_item(ui, "Son pendant la lecture (activer / couper)", "son.lecture");
                self.action_item(ui, "Onglet Son (pistes, volumes)", "onglet.son");
            }
            GridMenu::NotesHeader => {
                self.action_item(ui, "Onglet Notes (crayon, gomme, couleurs)", "onglet.notes");
            }
        }
    }

    /// Abandonne la poignée : le plan revient comme avant.
    pub(crate) fn cancel_handle_drag(&mut self) {
        let Some(d) = self.handle_drag.take() else { return };
        if let Some(p) = self.doc.panels.iter_mut().find(|p| p.id == d.panel) {
            *p = d.before;
            self.renderer.invalidate_all();
        }
    }

    /// Un glisser est en cours : poignée, clé de caméra, montage (durée, place d'un plan, son),
    /// images choisies, choix au rectangle, ligne de calque.
    pub(crate) fn dragging(&self) -> bool {
        self.handle_drag.is_some()
            || self.cam_key_drag.is_some()
            || self.montage_drag.is_some()
            || self.image_drag.is_some()
            || self.grid_select.is_some()
            || self.timeline_sound_track.is_some()
            || self.head_drag.is_some()
            || self.panel_layer_drag.is_some()
            || self.montage_layer_drag.is_some()
    }

    /// Échap (ou Ctrl+Z) pendant un glisser (chantier 2) : tout revient comme avant le geste ;
    /// rien n'est enregistré. Le choix fait au rectangle reste celui d'avant.
    pub(crate) fn cancel_drags(&mut self) {
        self.cancel_montage_drag();
        self.image_drag = None;
        if let Some(g) = self.grid_select.take() {
            self.frame_sel = g.before_frames;
            self.layer_sel = g.before_layers;
        }
        self.timeline_sound_track = None;
        self.head_drag = None;
        self.panel_layer_drag = None;
        self.montage_layer_drag = None;
        self.status = "Geste abandonné : tout revient comme avant.".into();
    }

    /// Après l'interface, une fois par image d'écran : un glisser dont la vue n'est plus montrée
    /// (bouton déjà relâché) ne reste pas en cours, sinon il bloquerait les raccourcis.
    pub(crate) fn drop_stale_drags(&mut self, pointer_down: bool) {
        if pointer_down {
            return;
        }
        self.grid_select = None;
        self.image_drag = None;
        self.head_drag = None;
        self.panel_layer_drag = None;
        self.montage_layer_drag = None;
        if self.handle_drag.is_some() {
            self.end_handle_drag();
        }
    }

    /// Une fois par image d'écran (chantier 4) : si la durée du plan courant a changé hors d'un
    /// glisser (touche +, dessin après la fin, collage…) et que des plans suivants se décalent par
    /// rapport au son, la barre d'état le dit.
    pub(crate) fn watch_plan_length(&mut self) {
        let Some(p) = self.doc.panels.get(self.cur_panel) else { return };
        let now = (p.id, p.frame_count());
        if self.dragging() {
            return;
        }
        if let Some((id, old)) = self.plan_len_seen {
            if id == now.0 && old != now.1 {
                if let Some(note) = sound_shift_note(&self.doc, self.cur_panel, old, now.1) {
                    let title = crate::storyboard::panel_title(&self.doc, self.cur_panel);
                    self.status = format!("{title} : {} images. {note}", now.1);
                }
            }
        }
        self.plan_len_seen = Some(now);
    }
}

/// Choix d'images au rectangle (chantier 3) : la case du clic, et le choix d'avant (rendu par
/// Échap).
pub(crate) struct GridSelect {
    pub col: usize,
    pub row: usize,
    pub before_frames: Option<(usize, usize)>,
    pub before_layers: Vec<u64>,
}
