//! Glisser des clés de dessins avant ou après une autre clé du même calque (partie 137, Anthony 2026-11-10 : « j'aimerais pouvoir
//! décaler des clés d'images en les faisant glisser avant ou après une autre clé d'image. Actuellement ça ne marche pas »).
//!
//! - **Où.** Dans les pistes de calques du montage (la timeline du Storyboard) et dans la timeline d'Animation. Pas dans les Vignettes.
//! - **Quand.** Sans touche, la souris lâchée sur un AUTRE dessin du calque de la clé prise : les clés prises s'insèrent avant ce
//!   dessin si la souris est dans sa première moitié, après lui sinon (`insertion_at`). Ailleurs (sur la clé elle-même, sur un vide,
//!   après la fin du calque), l'ancien geste reste : dans le montage, la clé et la suite glissent (`KeyMove::Ripple`) ; dans la
//!   timeline, les images se posent là où on les lâche (`DropMode::Move`). Les touches gardent leur geste (Maj, Alt, Ctrl, Ctrl+Maj).
//! - **Comment.** On insère, on n'écrase pas (`insert_keys_at`) : les clés prises sortent de leur place, celles d'entre l'ancienne et
//!   la nouvelle place se décalent pour la leur laisser, chaque dessin garde sa durée, la durée du calque et celle du plan ne changent
//!   pas. Plusieurs clés prises sur un calque se posent ensemble, dans leur ordre. Sur plusieurs calques, chaque calque se réordonne de
//!   son côté : sa limite d'insertion est à la même distance de sa première clé prise que sur le calque de la clé tenue. Les rôles des
//!   images suivent leur dessin ; les clés d'effets, de position et d'opacité du calque restent à leur image (elles sont au calque,
//!   pas au dessin). Un calque verrouillé ou masqué est laissé de côté, avec un message (`laisse_de_cote`).
//! - **Pendant le geste.** Rien n'est posé : une ligne d'insertion montre la limite (la même que le repère de « Réordonner »), la
//!   barre d'état dit « avant l'image 12 » ou « après l'image 9 ». Calcul léger (des listes d'expositions) : rien n'est rejoué.
//! - **Au lâcher.** Une étape d'annulation ; la barre d'état : « 3 clés déplacées avant l'image 12 ».

use crate::app::App;
use crate::document::{CelId, Layer};
use crate::handles::runs;

/// Le calque est laissé de côté par ce geste : verrouillé ou masqué.
pub(crate) fn laisse_de_cote(l: &Layer) -> bool {
    l.locked || !l.visible
}

/// La limite (une image) où s'insèrent les clés qui commencent aux images `starts` quand la souris est à l'image `pointer` du plan :
/// sur un autre dessin du calque (pas une des clés prises), avant lui dans sa première moitié, après lui dans la seconde. Rien
/// ailleurs, ni si l'insertion ne changerait rien (l'ancien geste reprend la main).
pub(crate) fn insertion_at(layer: &Layer, starts: &[usize], pointer: usize) -> Option<usize> {
    if starts.is_empty() || pointer >= layer.frames.len() {
        return None;
    }
    layer.frames[pointer]?;
    let (s, e) = layer.run_bounds(pointer);
    if starts.contains(&s) {
        return None;
    }
    let x = if 2 * pointer + 1 < s + e { s } else { e };
    let mut probe = layer.clone();
    insert_keys_at(&mut probe, starts, x);
    (probe.frames != layer.frames).then_some(x)
}

/// Les clés qui commencent aux images `starts` sorties de leur place et posées ensemble, dans leur ordre, à la limite `x` (image du
/// calque tel qu'il est) : chaque autre exposition (dessin ou vide) reste avant la limite si son milieu y est, sinon elle passe après.
/// Durées gardées, durée du calque inchangée, rôles des images avec leur dessin. Renvoie le nouveau début de chaque clé, dans l'ordre
/// de `starts` (une clé qui n'en est pas une garde son image).
pub(crate) fn insert_keys_at(layer: &mut Layer, starts: &[usize], x: usize) -> Vec<usize> {
    let all = runs(layer);
    let picked = |r: &(usize, usize, Option<CelId>)| r.2.is_some() && starts.contains(&r.0);
    let moved: Vec<(usize, usize, Option<CelId>)> = all.iter().filter(|r| picked(r)).copied().collect();
    if moved.is_empty() {
        return starts.to_vec();
    }
    let rest: Vec<(usize, usize, Option<CelId>)> = all.iter().filter(|r| !picked(r)).copied().collect();
    let cut = rest.iter().filter(|r| r.0 + r.1 <= 2 * x).count();
    let order = rest[..cut].iter().map(|r| (*r, false)).chain(moved.iter().map(|r| (*r, true))).chain(rest[cut..].iter().map(|r| (*r, false)));
    let mut frames = Vec::with_capacity(layer.frames.len());
    let mut roles = crate::roles::Roles::new();
    let mut landed: Vec<(usize, usize)> = Vec::new();
    for ((s, e, cel), is_moved) in order {
        let at = frames.len();
        if is_moved {
            landed.push((s, at));
        }
        roles.extend(layer.roles.range(s..e).map(|(f, r)| (at + (f - s), *r)));
        frames.extend(std::iter::repeat_n(cel, e - s));
    }
    layer.frames = frames;
    layer.roles = roles;
    layer.trim();
    starts.iter().map(|s| landed.iter().find(|m| m.0 == *s).map_or(*s, |m| m.1)).collect()
}

/// « avant l'image 12 » ou « après l'image 9 » : où se posent les clés `starts` insérées à la limite `x`, dit par le dessin voisin
/// (non pris) du calque tel qu'il est. `label` : le numéro d'une image tel que l'écran l'écrit.
pub(crate) fn place_text(layer: &Layer, starts: &[usize], x: usize, label: impl Fn(usize) -> String) -> String {
    let autres: Vec<(usize, usize, Option<CelId>)> = runs(layer).into_iter().filter(|r| r.2.is_some() && !starts.contains(&r.0)).collect();
    if x > 0 && autres.iter().any(|r| r.1 == x) {
        return format!("après l'image {}", label(x - 1));
    }
    match autres.iter().find(|r| r.0 >= x) {
        Some(r) => format!("avant l'image {}", label(r.0)),
        None if x > 0 => format!("après l'image {}", label(x - 1)),
        None => "au début".to_string(),
    }
}

/// Les images [a, b) couvrent des clés entières du calque : elles commencent et finissent à une limite entre deux expositions.
fn aligned(layer: &Layer, a: usize, b: usize) -> bool {
    let f = &layer.frames;
    let start_ok = a == 0 || a >= f.len() || f[a - 1] != f[a];
    let end_ok = b >= f.len() || b == 0 || f[b - 1] != f[b];
    start_ok && end_ok
}

/// Les clés de dessins qui commencent dans les images [a, b) du calque.
fn starts_in(layer: &Layer, a: usize, b: usize) -> Vec<usize> {
    runs(layer).into_iter().filter(|r| r.2.is_some() && r.0 >= a && r.0 < b).map(|r| r.0).collect()
}

impl App {
    /// Glisser des images choisies dans la timeline, lâchées à l'image `col` sur le calque `pointed` : si elles couvrent des clés
    /// entières de chaque calque choisi et que la souris est sur un autre dessin du calque pointé, le décalage de la limite
    /// d'insertion depuis la première clé prise (le même pour chaque calque). Sinon rien : l'ancien geste reste.
    pub(crate) fn image_insertion(&self, pointed: usize, col: usize) -> Option<isize> {
        let (a, b) = self.image_range();
        let layers = self.image_layers();
        if !layers.contains(&pointed) {
            return None;
        }
        if layers.iter().any(|&li| !aligned(&self.layers()[li], a, b)) {
            return None;
        }
        let l = &self.layers()[pointed];
        let starts = starts_in(l, a, b);
        let first = *starts.first()?;
        let x = insertion_at(l, &starts, col)?;
        Some(x as isize - first as isize)
    }

    /// Les lignes d'insertion à montrer pendant le glisser : (calque, image de la limite), pour chaque calque choisi qui a des clés prises.
    pub(crate) fn image_insertion_lines(&self, delta: isize) -> Vec<(usize, usize)> {
        let (a, b) = self.image_range();
        self.image_layers()
            .into_iter()
            .filter_map(|li| {
                let first = *starts_in(&self.layers()[li], a, b).first()?;
                Some((li, (first as isize + delta).max(0) as usize))
            })
            .collect()
    }

    /// Lâcher des images choisies sur un autre dessin (`image_insertion`) : sur chaque calque choisi, ses clés prises s'insèrent à sa
    /// limite (`insert_keys_at`), une étape d'annulation ; les calques verrouillés ou masqués restent tels quels.
    pub(crate) fn insert_chosen_images(&mut self, pointed: usize, delta: isize) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let (a, b) = self.image_range();
        let snap = self.snapshot();
        let mut de_cote = Vec::new();
        let mut n = 0usize;
        let mut new_first = None;
        let mut place = String::new();
        for li in self.image_layers() {
            let l = &self.layers()[li];
            let starts = starts_in(l, a, b);
            let Some(&first) = starts.first() else { continue };
            if laisse_de_cote(l) {
                de_cote.push(l.name.clone());
                continue;
            }
            let x = (first as isize + delta).max(0) as usize;
            if li == pointed {
                let before = l.clone();
                place = place_text(&before, &starts, x, |f| self.frame_label(f));
            }
            let layer = &mut self.layers_mut()[li];
            let before = layer.frames.clone();
            let landed = insert_keys_at(layer, &starts, x);
            if layer.frames != before {
                n += starts.len();
            }
            if li == pointed {
                new_first = landed.first().copied();
            }
        }
        let mut msg = if n > 0 { format!("{n} clé{} déplacée{} {place}.", s(n), s(n)) } else { "Rien n'a bougé.".to_string() };
        if !de_cote.is_empty() {
            msg += &format!(" Laissé{} de côté (verrouillé ou masqué) : {}.", s(de_cote.len()), de_cote.iter().map(|n| format!("« {n} »")).collect::<Vec<_>>().join(", "));
        }
        if n == 0 {
            self.status = msg;
            return;
        }
        if let Some(f) = new_first {
            self.frame_sel = Some((f, f + (b - a)));
            self.cur_frame = f;
        }
        self.vsel = None;
        self.commit_as(snap, crate::montagekeys::key_move_name(crate::handles::KeyMove::Insert, n > 1));
        self.status = msg;
    }
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}
