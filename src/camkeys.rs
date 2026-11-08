//! Clés de caméra qu'on choisit, déplace, copie, colle et supprime, dans le montage et dans la
//! timeline (Anthony, 2026-10-18 : « je veux pouvoir sélectionner une clé de caméra en cliquant
//! dessus et pouvoir la copier-coller dans la timeline »).
//!
//! Même geste que les clés d'effets (`fxkeys.rs`) :
//! - un clic choisit le losange jaune (Ctrl : l'ajoute ou le retire ; Maj : toutes les clés
//!   d'entre le dernier cliqué et lui, d'un plan à l'autre dans le montage) et place la tête de
//!   lecture dessus ; les clés choisies sont cerclées du contour du thème ;
//! - au rectangle (Anthony, 2026-10-19 : « sélectionner les clés de caméra par une sélection par
//!   zone ») : un glisser qui part d'un espace vide de la piste Caméra trace un rectangle et
//!   choisit les clés qu'il touche ; Maj les ajoute au choix, Ctrl bascule celles qu'il touche,
//!   Échap rend le choix d'avant ; un clic dans le vide lâche le choix (`CamRect`, `pick_in_rect`,
//!   `rect_choice`) ;
//! - glisser un losange (Anthony, 2026-10-19 : « quand je tire une image clé, les autres clés
//!   suivent ; avec Maj, seule l'image clé se déplace ») : par défaut, la clé prise (ou la
//!   première des clés choisies) et TOUTES les clés d'après, dans son plan, glissent du même
//!   nombre d'images (« Décaler les clés de caméra ») ; les clés d'avant ne bougent pas, et la
//!   première ne passe pas par-dessus la clé d'avant (`clamp_ripple`) ; vers la droite, les clés
//!   d'après qui toucheraient la fin du plan s'y tassent, une image chacune (`moves`, partie 129 :
//!   une clé sur la dernière image ne bloque plus celles d'avant). Maj tenue (même en cours de
//!   geste) : seules les clés choisies bougent, et passent par-dessus les autres sans jamais se
//!   poser sur l'une d'elles (`clamp_free`, « Déplacer une clé de caméra »). Dans les deux cas, ni
//!   avant l'image 0 ni après la fin du plan, et la ligne d'état dit ce qui arrête la clé ; une
//!   étape d'annulation par geste, Échap l'abandonne. Un
//!   Maj + clic sans mouvement reste le choix d'une plage : c'est egui qui sépare le clic du
//!   glisser (seuil d'environ 6 points) ;
//! - Ctrl+C copie les clés choisies. Ctrl+V les INSÈRE juste après la clé sur laquelle est la tête
//!   de lecture (sinon la dernière clé d'avant elle ; sans clé avant, à la tête de lecture), en
//!   gardant leurs écarts : la première clé collée est à l'image d'après cette clé, et toutes les
//!   clés d'après sont repoussées de la longueur du bloc (écart de la dernière clé collée + 1),
//!   donc rien n'est écrasé (`paste_insert`, « Insérer des clés de caméra »). Maj+Ctrl+V colle
//!   « à la tête de lecture » (`edition.coller_a_la_tete`) : la première clé tombe exactement
//!   sur la tête, sans rien repousser, et remplace les clés qu'elle rencontre (`paste`, « Coller
//!   les clés de caméra à la tête de lecture »). Dans le montage, le bloc peut passer d'un plan à
//!   l'autre (une clé de caméra est comptée dans son plan, ses valeurs sont celles de la feuille) ;
//!   en animation, dans le plan courant seulement. Une clé qui tomberait après la fin du dernier
//!   plan (ou du plan courant) est laissée de côté, et dite ; si l'insertion repoussait une clé
//!   hors de son plan, rien n'est collé (le plan ne s'allonge pas). Suppr supprime les clés
//!   choisies, et rien sans clé choisie ;
//! - le clic droit (piste Caméra) a les mêmes commandes ; chaque modification est une étape
//!   d'annulation nommée.
//!
//! La logique est ici en fonctions pures (`choose`, `clamp_free`, `clamp_ripple`, `moves`, `shift`, `copy`,
//! `paste`, `paste_insert`, `delete`), testées dans `camkeys_tests.rs` ; `impl App` ne fait que la brancher sur la souris,
//! le clavier et l'historique.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

use crate::app::{App, Mode};
use crate::camera::{self, CameraKey};
use crate::document::Document;
use crate::history::Edit;

/// Une clé de caméra choisie : son plan (id) et son image dans ce plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CamKeyRef {
    pub panel: u64,
    pub frame: usize,
}

/// Une clé copiée : son écart (en images du montage) à la première clé copiée, et la clé.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CamClipKey {
    pub offset: usize,
    pub key: CameraKey,
}

/// Les clés de caméra copiées.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CamClip {
    pub keys: Vec<CamClipKey>,
}

/// Ce que donne un collage.
#[derive(Debug, PartialEq)]
pub(crate) struct Pasted {
    /// Les clés posées.
    pub placed: Vec<CamKeyRef>,
    /// Clés laissées de côté : après la fin du dernier plan, ou hors du plan courant (animation).
    pub skipped: usize,
}

// ---------------------------------------------------------------------- lecture

fn panel_index(doc: &Document, id: u64) -> Option<usize> {
    doc.panels.iter().position(|p| p.id == id)
}

/// La clé existe encore.
pub(crate) fn exists(doc: &Document, k: CamKeyRef) -> bool {
    panel_index(doc, k.panel).is_some_and(|pi| doc.panels[pi].camera.iter().any(|c| c.frame == k.frame))
}

/// Image du montage de la clé.
pub(crate) fn global_of(doc: &Document, k: CamKeyRef) -> Option<usize> {
    panel_index(doc, k.panel).map(|pi| doc.panel_start(pi) + k.frame)
}

/// Les clés choisies qui existent encore, sans doublon, dans l'ordre du montage.
pub(crate) fn live(doc: &Document, sel: &[CamKeyRef]) -> Vec<CamKeyRef> {
    let mut out: Vec<(usize, CamKeyRef)> = Vec::new();
    for &k in sel {
        if let Some(g) = global_of(doc, k).filter(|_| exists(doc, k)) {
            if !out.iter().any(|(_, o)| *o == k) {
                out.push((g, k));
            }
        }
    }
    out.sort_by_key(|(g, _)| *g);
    out.into_iter().map(|(_, k)| k).collect()
}

/// Toutes les clés de caméra du montage entre les images `a` et `b` (comprises, dans n'importe
/// quel ordre), d'un plan à l'autre.
pub(crate) fn between(doc: &Document, a: CamKeyRef, b: CamKeyRef) -> Vec<CamKeyRef> {
    let (Some(ga), Some(gb)) = (global_of(doc, a), global_of(doc, b)) else { return Vec::new() };
    let (lo, hi) = (ga.min(gb), ga.max(gb));
    let mut out = Vec::new();
    let mut start = 0;
    for p in &doc.panels {
        for k in &p.camera {
            if (lo..=hi).contains(&(start + k.frame)) {
                out.push(CamKeyRef { panel: p.id, frame: k.frame });
            }
        }
        start += p.frame_count();
    }
    out
}

// ---------------------------------------------------------------------- choix

/// Le choix après un clic sur la clé `clicked` : elle seule ; `toggle` (Ctrl) : elle s'ajoute, ou
/// se retire si elle était choisie ; `range` (Maj) : les clés `between` (de l'ancre à elle), s'il y
/// a une ancre. Renvoie aussi la nouvelle ancre.
pub(crate) fn choose(chosen: &[CamKeyRef], anchor: Option<CamKeyRef>, clicked: CamKeyRef, toggle: bool, range: Option<Vec<CamKeyRef>>) -> (Vec<CamKeyRef>, Option<CamKeyRef>) {
    // La règle est la même pour les trois sortes de clés (`clescommun.rs`).
    crate::clescommun::choose_group(chosen, anchor, &[clicked], toggle, range)
}

// ---------------------------------------------------------------------- choix au rectangle

/// Un choix au rectangle en cours : son coin de départ (à l'écran), le choix d'avant (que Échap
/// rend), et la touche tenue au départ : Maj ajoute au choix, Ctrl bascule les clés touchées.
pub(crate) struct CamRect {
    pub origin: Pos2,
    pub before: Vec<CamKeyRef>,
    pub before_anchor: Option<CamKeyRef>,
    pub add: bool,
    pub toggle: bool,
}

/// Les clés dont le losange (centre donné, `half` de demi-diagonale) touche `rect`.
pub(crate) fn pick_in_rect(diamonds: &[(CamKeyRef, Pos2)], rect: Rect, half: f32) -> Vec<CamKeyRef> {
    diamonds.iter().filter(|(_, c)| Rect::from_center_size(*c, egui::vec2(2.0 * half, 2.0 * half)).intersects(rect)).map(|(k, _)| *k).collect()
}

/// Le choix une fois le rectangle tendu sur `picked` : sans touche, ces clés seules ; `add` (Maj) :
/// elles s'ajoutent au choix d'avant ; `toggle` (Ctrl) : celles du choix d'avant qu'il touche sont
/// retirées, les autres ajoutées.
pub(crate) fn rect_choice(before: &[CamKeyRef], picked: &[CamKeyRef], add: bool, toggle: bool) -> Vec<CamKeyRef> {
    // La règle est la même pour les clés de dessins et de caméra (`clescommun.rs`).
    crate::clescommun::rect_choice(before, picked, add, toggle)
}

/// Demi-diagonale d'un losange de clé de caméra à l'écran, en points.
pub(crate) const DIAMOND_HALF: f32 = 4.0;

/// Le rectangle de choix tel qu'on le voit : un voile du contour du thème, cerclé.
pub(crate) fn paint_marquee(painter: &egui::Painter, rect: Rect) {
    let accent = crate::theme::contour();
    painter.rect(rect, 2.0, accent.gamma_multiply(0.15), Stroke::new(1.5_f32, accent), egui::StrokeKind::Inside);
}

// ---------------------------------------------------------------------- déplacer

/// Les clés d'un plan prises ensemble : ses images choisies, toutes ses images clés, sa durée.
pub(crate) struct Group {
    pub chosen: Vec<usize>,
    pub all: Vec<usize>,
    pub len: usize,
}

/// Les images qui suivent le geste « décaler » : la première image choisie et toutes les images
/// clés d'après (choisies ou non), dans l'ordre.
pub(crate) fn ripple_frames(all: &[usize], chosen: &[usize]) -> Vec<usize> {
    let Some(first) = chosen.iter().copied().min() else { return Vec::new() };
    let mut out: Vec<usize> = all.iter().copied().filter(|f| *f >= first).collect();
    out.sort_unstable();
    out
}

/// Le décalage permis au geste « décaler » : toutes les clés d'après la première choisie glissent ensemble. Vers la
/// gauche, la première s'arrête juste après la clé d'avant (qui ne bouge pas) et à l'image 0. Vers la droite, les clés
/// d'après **se tassent contre la fin du plan** (`moves`) : la clé prise peut aller jusqu'à laisser une image à chacune.
///
/// (Avant le 8 novembre 2026, le bloc entier s'arrêtait dès que sa dernière clé touchait la fin du plan : une clé posée
/// sur la dernière image, comme en laissent un Ctrl sur la durée ou la Trajectoire, bloquait toutes celles d'avant.
/// Anthony : « je ne peux pas pousser la clé de caméra vers la droite ».)
pub(crate) fn clamp_ripple(groups: &[Group], want: isize) -> isize {
    let (mut lo, mut hi) = (isize::MIN, isize::MAX);
    for g in groups {
        let moving = ripple_frames(&g.all, &g.chosen);
        let Some(&first) = moving.first() else { continue };
        let below = g.all.iter().filter(|a| **a < first).max().map_or(0, |a| *a as isize + 1);
        lo = lo.max(below - first as isize);
        hi = hi.min(g.len as isize - moving.len() as isize - first as isize);
    }
    if lo == isize::MIN || hi == isize::MAX || lo > hi {
        return 0;
    }
    want.clamp(lo, hi)
}

/// Le décalage permis quand seules les clés choisies bougent (Maj) : aucune ne sort du plan, et une clé **peut passer
/// par-dessus** une clé qui n'est pas choisie, sans jamais tomber sur elle (elle s'arrête à côté tant que la souris est
/// dessus). Le décalage rendu est le plus proche de `want` qui ne pose aucune clé sur une autre ; à égalité, le plus
/// court.
pub(crate) fn clamp_free(groups: &[Group], want: isize) -> isize {
    let (mut lo, mut hi) = (isize::MIN, isize::MAX);
    for g in groups {
        for &f in &g.chosen {
            lo = lo.max(-(f as isize));
            hi = hi.min(g.len.saturating_sub(1) as isize - f as isize);
        }
    }
    if lo == isize::MIN || hi == isize::MAX || lo > hi {
        return 0;
    }
    let w = want.clamp(lo, hi);
    let libre = |d: isize| {
        groups.iter().all(|g| g.chosen.iter().all(|&f| {
            let t = f as isize + d;
            !g.all.iter().any(|&a| a as isize == t && !g.chosen.contains(&a))
        }))
    };
    let vers_zero = -w.signum();
    for k in 0..=(hi - lo) {
        for d in [w + vers_zero * k, w - vers_zero * k] {
            if (lo..=hi).contains(&d) && libre(d) {
                return d;
            }
        }
    }
    0
}

/// Les clés d'un plan qui bougent de `delta` images, et où elles arrivent : (image d'avant, image d'après), dans l'ordre.
/// Maj (`ripple` faux) : les clés choisies, du même décalage (ramené par `clamp_free`). Sinon : la première choisie et
/// toutes celles d'après ; celles qui passeraient la fin du plan s'y arrêtent, tassées à une image l'une de l'autre,
/// dans leur ordre : aucune ne disparaît ni n'en écrase une autre.
pub(crate) fn moves(g: &Group, delta: isize, ripple: bool) -> Vec<(usize, usize)> {
    let last = g.len.saturating_sub(1) as isize;
    let frames = if ripple { ripple_frames(&g.all, &g.chosen) } else { g.chosen.clone() };
    let mut out: Vec<(usize, usize)> = frames.iter().map(|&f| (f, (f as isize + delta).clamp(0, last) as usize)).collect();
    if ripple {
        let mut limit = last;
        for m in out.iter_mut().rev() {
            let t = (m.1 as isize).min(limit).max(0);
            m.1 = t as usize;
            limit = t - 1;
        }
    }
    out
}

/// Les clés `before` une fois les déplacements `mv` faits (`moves`), triées.
pub(crate) fn apply_moves(before: &[CameraKey], mv: &[(usize, usize)]) -> Vec<CameraKey> {
    let mut keys: Vec<CameraKey> = before.iter().map(|k| CameraKey { frame: mv.iter().find(|m| m.0 == k.frame).map_or(k.frame, |m| m.1), ..*k }).collect();
    keys.sort_by_key(|k| k.frame);
    keys
}

/// Les clés de `keys` aux images `frames` déplacées de `delta` images (le décalage doit être
/// ramené dans les bornes par `clamp_delta`). Renvoie leurs images maintenant.
#[cfg(test)]
pub(crate) fn shift(keys: &mut [CameraKey], frames: &[usize], delta: isize) -> Vec<usize> {
    let mut out = Vec::new();
    for k in keys.iter_mut().filter(|k| frames.contains(&k.frame)) {
        k.frame = (k.frame as isize + delta).max(0) as usize;
        out.push(k.frame);
    }
    keys.sort_by_key(|k| k.frame);
    out
}

// ---------------------------------------------------------------------- copier, coller, supprimer

/// Copie les clés choisies (`sel`) ; rien s'il n'y en a pas.
pub(crate) fn copy(doc: &Document, sel: &[CamKeyRef]) -> Option<CamClip> {
    let mut keys: Vec<(usize, CameraKey)> = live(doc, sel)
        .into_iter()
        .filter_map(|r| {
            let pi = panel_index(doc, r.panel)?;
            let key = doc.panels[pi].camera.iter().find(|k| k.frame == r.frame).copied()?;
            Some((doc.panel_start(pi) + r.frame, key))
        })
        .collect();
    keys.sort_by_key(|(g, _)| *g);
    let first = keys.first()?.0;
    Some(CamClip { keys: keys.into_iter().map(|(g, key)| CamClipKey { offset: g - first, key }).collect() })
}

/// Colle `clip` à l'image `at` du montage : chaque clé va à son écart, dans le plan qui a cette
/// image, et remplace la clé qui y était. `only` : un seul plan (son rang) accepte les clés. Les
/// clés après la fin du dernier plan, ou hors du plan permis, sont laissées de côté. Un plan sans
/// clé qui en reçoit après son début garde d'abord sa caméra de continuité en place (comme une clé
/// posée à la main).
pub(crate) fn paste(doc: &mut Document, clip: &CamClip, at: usize, only: Option<usize>) -> Pasted {
    let starts = plan_ranges(doc);
    let (targets, skipped) = resolve_targets(&starts, clip, at, only);
    add_homes(doc, &targets);
    let mut placed = Vec::new();
    for (pi, frame, key) in targets {
        camera::put_key(&mut doc.panels[pi].camera, CameraKey { frame, ..key });
        placed.push(CamKeyRef { panel: doc.panels[pi].id, frame });
    }
    Pasted { placed, skipped }
}

/// (début dans le montage, durée) de chaque plan.
fn plan_ranges(doc: &Document) -> Vec<(usize, usize)> {
    let mut s = 0;
    doc.panels
        .iter()
        .map(|p| {
            let r = (s, p.frame_count());
            s += r.1;
            r
        })
        .collect()
}

/// Où va chaque clé de `clip` collée à l'image `at` du montage : (plan, image du plan, clé), et
/// combien sont laissées de côté (après la fin du dernier plan, ou hors du plan `only`).
fn resolve_targets(starts: &[(usize, usize)], clip: &CamClip, at: usize, only: Option<usize>) -> (Vec<(usize, usize, CameraKey)>, usize) {
    let mut targets = Vec::new();
    let mut skipped = 0;
    for ck in &clip.keys {
        let g = at + ck.offset;
        match starts.iter().position(|&(s, len)| g >= s && g < s + len) {
            Some(pi) if only.is_none_or(|o| o == pi) => targets.push((pi, g - starts[pi].0, ck.key)),
            _ => skipped += 1,
        }
    }
    (targets, skipped)
}

/// Les caméras de continuité, avant d'écrire quoi que ce soit : un plan sans clé qui en reçoit
/// après son début garde d'abord sa caméra d'alors, en clé à l'image 0.
fn add_homes(doc: &mut Document, targets: &[(usize, usize, CameraKey)]) {
    let mut firsts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for t in targets {
        firsts.entry(t.0).and_modify(|f| *f = (*f).min(t.1)).or_insert(t.1);
    }
    let homes: Vec<(usize, camera::Camera)> =
        firsts.into_iter().filter(|&(pi, f)| doc.panels[pi].camera.is_empty() && f > 0).map(|(pi, _)| (pi, doc.panel_camera_douce(pi, 0))).collect();
    for (pi, cam) in homes {
        doc.panels[pi].camera.push(CameraKey::new(0, cam));
    }
}

/// L'image de la clé sur laquelle est la tête de lecture (`playhead`, image du plan), sinon la
/// dernière clé d'avant elle ; rien s'il n'y en a pas.
pub(crate) fn anchor_frame(keys: &[CameraKey], playhead: usize) -> Option<usize> {
    keys.iter().map(|k| k.frame).filter(|f| *f <= playhead).max()
}

/// Ce que donne un collage par insertion.
#[derive(Debug, PartialEq)]
pub(crate) struct Inserted {
    pub pasted: Pasted,
    /// Clés d'après repoussées.
    pub pushed: usize,
}

/// Une insertion qui repousserait des clés hors de leur plan : rien n'est fait. `short` : de
/// combien d'images il manque de place, au plus juste.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NoRoom {
    pub short: usize,
}

/// Insère `clip` après la clé qui a la tête de lecture (`playhead`, image du montage) : la
/// première clé collée va à l'image d'après cette clé (la dernière clé d'avant la tête si aucune
/// n'est dessus, la tête elle-même s'il n'y en a pas avant, dans le plan de la tête), les autres
/// gardent leurs écarts. Dans chaque plan qui reçoit des clés, toutes les clés à partir de la
/// première collée sont repoussées de la longueur de ce qu'il reçoit (de sa première à sa dernière
/// clé collée, comprises) : rien n'est écrasé. Même règles de plans que `paste` (`only`, clés laissées
/// de côté, caméra de continuité). Si une clé d'après sortait de son plan, rien n'est fait.
pub(crate) fn paste_insert(doc: &mut Document, clip: &CamClip, playhead: usize, only: Option<usize>) -> Result<Inserted, NoRoom> {
    let empty = || Ok(Inserted { pasted: Pasted { placed: Vec::new(), skipped: clip.keys.len() }, pushed: 0 });
    let starts = plan_ranges(doc);
    let Some(head) = starts.iter().position(|&(s, len)| playhead >= s && playhead < s + len) else { return empty() };
    let local = playhead - starts[head].0;
    let at_local = match anchor_frame(&doc.panels[head].camera, local) {
        Some(a) => a + 1,
        None => local,
    };
    if at_local >= starts[head].1 {
        return empty();
    }
    let (targets, skipped) = resolve_targets(&starts, clip, starts[head].0 + at_local, only);
    // Par plan : première et dernière images collées.
    let mut spans: std::collections::BTreeMap<usize, (usize, usize)> = std::collections::BTreeMap::new();
    for t in &targets {
        spans.entry(t.0).and_modify(|s| *s = (s.0.min(t.1), s.1.max(t.1))).or_insert((t.1, t.1));
    }
    // Y a-t-il la place ? La dernière clé repoussée doit rester dans son plan.
    let mut short = 0;
    for (&pi, &(first, last)) in &spans {
        let room = last - first + 1;
        if let Some(max) = doc.panels[pi].camera.iter().map(|k| k.frame).filter(|f| *f >= first).max() {
            short = short.max((max + room).saturating_sub(starts[pi].1 - 1));
        }
    }
    if short > 0 {
        return Err(NoRoom { short });
    }
    add_homes(doc, &targets);
    let mut pushed = 0;
    for (&pi, &(first, last)) in &spans {
        let room = last - first + 1;
        for k in doc.panels[pi].camera.iter_mut().filter(|k| k.frame >= first) {
            k.frame += room;
            pushed += 1;
        }
    }
    let mut placed = Vec::new();
    for (pi, frame, key) in targets {
        camera::put_key(&mut doc.panels[pi].camera, CameraKey { frame, ..key });
        placed.push(CamKeyRef { panel: doc.panels[pi].id, frame });
    }
    Ok(Inserted { pasted: Pasted { placed, skipped }, pushed })
}

/// Supprime les clés choisies. Renvoie combien étaient là.
pub(crate) fn delete(doc: &mut Document, sel: &[CamKeyRef]) -> usize {
    let mut n = 0;
    for k in live(doc, sel) {
        if let Some(pi) = panel_index(doc, k.panel) {
            let keys = &mut doc.panels[pi].camera;
            if let Some(i) = keys.iter().position(|c| c.frame == k.frame) {
                keys.remove(i);
                n += 1;
            }
        }
    }
    n
}

// ---------------------------------------------------------------------- geste en cours

/// Des clés de caméra prises à la souris : chaque plan touché tel qu'il était, le point pris (image
/// du montage), le décalage en cours, l'étape d'annulation de tout le geste.
pub(crate) struct CamKeysDrag {
    panels: Vec<DragPanel>,
    grab: usize,
    delta: isize,
    /// Le décalage que demande la souris (avant les bornes) : de quoi dire pourquoi la clé s'arrête.
    want: isize,
    /// Les clés d'après suivent (par défaut) ; faux : Maj tenue, seules les clés choisies bougent.
    ripple: bool,
    snap: Edit,
}

struct DragPanel {
    panel: u64,
    before: Vec<CameraKey>,
    chosen: Vec<usize>,
    len: usize,
    /// Où sont allées les clés au dernier pas du geste : (image d'avant, image d'après) (`moves`).
    moved: Vec<(usize, usize)>,
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

impl App {
    /// Des clés de caméra sont choisies et la bande (montage ou timeline) a la main : copier,
    /// coller et supprimer les visent.
    pub(crate) fn cam_keys_have_focus(&self) -> bool {
        (self.key_focus || self.images_focus) && !self.cam_targets().is_empty()
    }

    /// Les clés choisies qui existent encore.
    pub(crate) fn cam_targets(&self) -> Vec<CamKeyRef> {
        live(&self.doc, &self.cam_key_sel)
    }

    /// La clé `frame` du plan de rang `pi` est choisie.
    pub(crate) fn cam_key_chosen(&self, pi: usize, frame: usize) -> bool {
        !self.cam_key_sel.is_empty() && self.doc.panels.get(pi).is_some_and(|p| self.cam_key_sel.contains(&CamKeyRef { panel: p.id, frame }))
    }

    /// Lâche le choix des clés de caméra.
    pub(crate) fn clear_cam_keys(&mut self) {
        self.cam_key_sel.clear();
        self.cam_key_anchor = None;
    }

    /// Donne la main aux clés de caméra choisies : elles remplacent le choix des dessins, des
    /// images et des clés d'effets.
    fn give_cam_keys_focus(&mut self) {
        self.key_sel.clear();
        self.key_anchor = None;
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.frame_sel = None;
        self.panel_focus = false;
        if self.mode == Mode::Storyboard {
            self.key_focus = true;
        } else {
            self.images_focus = true;
        }
    }

    /// Enregistre l'étape `snap` sous le nom `name` (le panneau Historique le montre).
    pub(crate) fn commit_as(&mut self, snap: Edit, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.commit_structure(snap);
        self.history.context = before;
    }

    /// Une étape d'annulation nommée, à enregistrer AVANT le changement.
    pub(crate) fn record_named_step(&mut self, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.record_step();
        self.history.context = before;
    }

    /// Clic sur la clé de l'image `frame` du plan de rang `pi` : voir `choose`. La tête de lecture
    /// va sur la clé.
    pub(crate) fn click_cam_key(&mut self, pi: usize, frame: usize, toggle: bool, range: bool) {
        let Some(panel) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        let clicked = CamKeyRef { panel, frame };
        if !exists(&self.doc, clicked) {
            return;
        }
        let between_keys = match (range, self.cam_key_anchor) {
            (true, Some(a)) => Some(between(&self.doc, a, clicked)),
            _ => None,
        };
        let (sel, anchor) = choose(&self.cam_key_sel, self.cam_key_anchor, clicked, toggle, between_keys);
        self.cam_key_sel = sel;
        self.cam_key_anchor = anchor;
        self.give_cam_keys_focus();
        if pi == self.cur_panel && self.mode == Mode::Animation {
            self.set_frame_free(frame);
        } else {
            let g = self.doc.panel_start(pi) + frame;
            self.goto_global(g);
        }
        let n = self.cam_key_sel.len();
        self.status = if n == 0 {
            "Plus aucune clé de caméra choisie.".into()
        } else {
            format!(
                "{n} clé{} de caméra choisie{} (image {}) : glisser pour décaler (Maj : cette clé seule), Ctrl+C, Ctrl+V insère après la clé (Maj+Ctrl+V : sur la tête), Suppr ; clic droit pour le reste.",
                s(n),
                s(n),
                self.frame_label(frame)
            )
        };
    }

    /// Choisit toutes les clés de caméra du plan courant.
    pub(crate) fn choose_all_cam_keys(&mut self) {
        let Some(p) = self.doc.panels.get(self.cur_panel) else { return };
        let all: Vec<CamKeyRef> = p.camera.iter().map(|k| CamKeyRef { panel: p.id, frame: k.frame }).collect();
        if all.is_empty() {
            self.status = "Ce plan n'a pas de clé de caméra.".into();
            return;
        }
        self.cam_key_anchor = all.first().copied();
        self.cam_key_sel = all;
        self.give_cam_keys_focus();
        let n = self.cam_key_sel.len();
        self.status = format!("{n} clé{} de caméra choisie{} (tout le plan).", s(n), s(n));
    }

    /// Les clés qui viennent d'être posées (un collage, un effet) deviennent le choix.
    pub(crate) fn choose_cam_keys(&mut self, keys: Vec<CamKeyRef>) {
        self.cam_key_anchor = keys.first().copied();
        self.cam_key_sel = keys;
        self.give_cam_keys_focus();
    }

    // ------------------------------------------------------------------ choix au rectangle

    /// Les losanges de clés de caméra du montage à l'écran : `blocks` les plans, `ppf` la largeur
    /// d'une image, `y` le milieu de la piste Caméra.
    pub(crate) fn montage_cam_diamonds(&self, blocks: &[Rect], ppf: f32, y: f32) -> Vec<(CamKeyRef, Pos2)> {
        let mut out = Vec::new();
        for (p, b) in self.doc.panels.iter().zip(blocks) {
            let x0 = b.left() - 1.0;
            out.extend(p.camera.iter().map(|k| (CamKeyRef { panel: p.id, frame: k.frame }, egui::pos2(x0 + k.frame as f32 * ppf + ppf * 0.5, y))));
        }
        out
    }

    /// Commence un choix au rectangle depuis `origin`, un espace vide de la piste Caméra. Maj
    /// ajoute au choix d'avant, Ctrl bascule les clés touchées ; sans touche, le choix repart de zéro.
    pub(crate) fn begin_cam_rect(&mut self, origin: Pos2, add: bool, toggle: bool) {
        self.end_stroke();
        self.playing = false;
        self.cam_rect = Some(CamRect { origin, before: self.cam_key_sel.clone(), before_anchor: self.cam_key_anchor, add, toggle });
    }

    /// Le rectangle suit la souris : le choix devient ce qu'il touche (`picked`) avec le choix d'avant.
    pub(crate) fn drag_cam_rect(&mut self, picked: &[CamKeyRef]) {
        let Some((before, add, toggle)) = self.cam_rect.as_ref().map(|r| (r.before.clone(), r.add, r.toggle)) else { return };
        let sel = rect_choice(&before, picked, add, toggle);
        self.cam_key_anchor = sel.last().copied();
        self.cam_key_sel = sel;
        if !self.cam_key_sel.is_empty() {
            self.give_cam_keys_focus();
        }
    }

    /// Lâche le rectangle : le choix reste, et la barre d'état le dit.
    pub(crate) fn end_cam_rect(&mut self) {
        if self.cam_rect.take().is_none() {
            return;
        }
        let n = self.cam_targets().len();
        if n > 0 {
            self.give_cam_keys_focus();
        }
        self.status = if n == 0 {
            "Plus aucune clé de caméra choisie.".into()
        } else {
            format!("{n} clé{} de caméra choisie{} : glisser pour décaler (Maj : seules celles-ci), Ctrl+C, Ctrl+V insère après la clé (Maj+Ctrl+V : sur la tête), Suppr.", s(n), s(n))
        };
    }

    /// Échap pendant le rectangle : le choix d'avant revient.
    pub(crate) fn cancel_cam_rect(&mut self) {
        if let Some(r) = self.cam_rect.take() {
            self.cam_key_sel = r.before;
            self.cam_key_anchor = r.before_anchor;
        }
    }

    // ------------------------------------------------------------------ déplacer

    /// Commence à glisser les clés choisies, en prenant la clé de l'image `frame` du plan de rang
    /// `pi` ; `grab` : l'image du montage sous la souris (même repère que `drag_cam_keys`). Une clé
    /// prise hors du choix devient le choix. Faux si rien n'est pris.
    pub(crate) fn begin_cam_keys_drag(&mut self, pi: usize, frame: usize, grab: usize) -> bool {
        let Some(panel) = self.doc.panels.get(pi).map(|p| p.id) else { return false };
        if !self.cam_key_sel.contains(&CamKeyRef { panel, frame }) {
            self.click_cam_key(pi, frame, false, false);
        }
        let targets = self.cam_targets();
        if targets.is_empty() {
            return false;
        }
        let mut panels: Vec<DragPanel> = Vec::new();
        for k in &targets {
            if let Some(d) = panels.iter_mut().find(|d| d.panel == k.panel) {
                d.chosen.push(k.frame);
                continue;
            }
            let Some(p) = self.doc.panels.iter().find(|p| p.id == k.panel) else { continue };
            panels.push(DragPanel { panel: k.panel, before: p.camera.clone(), chosen: vec![k.frame], len: p.frame_count(), moved: Vec::new() });
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.cam_keys_drag = Some(CamKeysDrag { panels, grab, delta: 0, want: 0, ripple: true, snap });
        true
    }

    /// Les clés prises suivent la souris, à l'image `at` du montage : chaque plan repart de ses
    /// clés d'avant le geste. Maj tenue : seules les clés choisies bougent (on peut la presser ou
    /// la lâcher en cours de geste) ; sinon les clés d'après les suivent.
    pub(crate) fn drag_cam_keys(&mut self, at: usize) {
        let ripple = !self.ctx.input(|i| i.modifiers.shift);
        self.drag_cam_keys_mode(at, ripple);
    }

    pub(crate) fn drag_cam_keys_mode(&mut self, at: usize, ripple: bool) {
        let Some(mut d) = self.cam_keys_drag.take() else { return };
        let groups: Vec<Group> =
            d.panels.iter().map(|p| Group { chosen: p.chosen.clone(), all: p.before.iter().map(|k| k.frame).collect(), len: p.len }).collect();
        let want = at as isize - d.grab as isize;
        let delta = if ripple { clamp_ripple(&groups, want) } else { clamp_free(&groups, want) };
        if delta != d.delta || ripple != d.ripple || want != d.want {
            d.delta = delta;
            d.ripple = ripple;
            d.want = want;
            let (mut tassees, mut au_bout, mut au_debut) = (false, false, false);
            for (dp, g) in d.panels.iter_mut().zip(&groups) {
                let Some(pi) = panel_index(&self.doc, dp.panel) else { continue };
                let mv = moves(g, delta, ripple);
                tassees |= mv.iter().any(|&(a, b)| (b as isize) < a as isize + delta);
                au_bout |= mv.iter().any(|&(a, b)| b + 1 >= g.len && dp.chosen.contains(&a)) || (ripple && mv.last().is_some_and(|m| m.1 + 1 >= g.len));
                au_debut |= mv.iter().any(|&(_, b)| b == 0);
                self.doc.panels[pi].camera = apply_moves(&dp.before, &mv);
                dp.moved = mv;
            }
            let n: usize = d.panels.iter().map(|p| p.chosen.len()).sum();
            let who = if ripple { "et les clés d'après".to_string() } else { "seule, Maj tenue".to_string() };
            // Pourquoi elle s'arrête, quand elle s'arrête (Anthony ne voyait pas pourquoi la clé ne venait pas).
            let pourquoi = if want > delta && ripple {
                " : la fin du plan l'arrête (les clés d'après sont tassées contre elle, une image chacune)"
            } else if tassees {
                " ; les clés d'après se tassent contre la fin du plan"
            } else if want > delta && au_bout {
                " : la fin du plan l'arrête"
            } else if want < delta && ripple {
                " : la clé d'avant l'arrête (Maj tenue : seule cette clé bouge, et passe par-dessus)"
            } else if want < delta && au_debut {
                " : le début du plan l'arrête"
            } else if want != delta {
                " : une autre clé est sur cette image, elle attend à côté"
            } else {
                ""
            };
            self.status = format!("{n} clé{} de caméra {who} : {delta:+} image{}{pourquoi}.", s(n), if delta.abs() > 1 { "s" } else { "" });
        }
        self.cam_keys_drag = Some(d);
    }

    /// Lâche les clés : une étape d'annulation, « Décaler les clés de caméra » (les clés d'après
    /// ont suivi) ou « Déplacer une clé de caméra » (Maj tenue), si quelque chose a bougé.
    pub(crate) fn end_cam_keys_drag(&mut self) {
        let Some(d) = self.cam_keys_drag.take() else { return };
        if d.delta == 0 {
            return;
        }
        let mut sel = Vec::new();
        for dp in &d.panels {
            sel.extend(dp.chosen.iter().map(|&f| CamKeyRef { panel: dp.panel, frame: dp.moved.iter().find(|m| m.0 == f).map_or(f, |m| m.1) }));
        }
        let several = sel.len() > 1;
        self.cam_key_anchor = sel.first().copied();
        self.cam_key_sel = sel;
        let name = if d.ripple {
            "Décaler les clés de caméra"
        } else if several {
            "Déplacer des clés de caméra"
        } else {
            "Déplacer une clé de caméra"
        };
        self.commit_as(d.snap, name);
    }

    /// Échap pendant le geste : les clés reviennent comme avant, le choix reste.
    pub(crate) fn cancel_cam_keys_drag(&mut self) {
        let Some(d) = self.cam_keys_drag.take() else { return };
        for dp in d.panels {
            if let Some(pi) = panel_index(&self.doc, dp.panel) {
                self.doc.panels[pi].camera = dp.before;
            }
        }
    }

    // ------------------------------------------------------------------ copier, coller, supprimer

    /// Copie les clés de caméra choisies.
    pub(crate) fn copy_cam_keys(&mut self) {
        let Some(clip) = copy(&self.doc, &self.cam_key_sel) else {
            self.status = "Pas de clé de caméra choisie : cliquer sur un losange jaune de la piste Caméra.".into();
            return;
        };
        let n = clip.keys.len();
        self.cam_key_clipboard = Some(clip);
        self.collage.origines.camera = Some(crate::adaptation::Origine::de(&self.doc));
        self.cam_clip_recent = true;
        self.fx_clip_recent = false;
        self.fx_pile_recent = false;
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau SHOT (`pasteimage`).
        self.ctx.copy_text(format!("{}clés de caméra copiées", crate::pasteimage::MARK));
        self.status = format!("{n} clé{} de caméra copiée{} : Ctrl+V pour les insérer après la clé de la tête de lecture, Maj+Ctrl+V pour les coller sur la tête.", s(n), s(n));
    }

    /// Ctrl+V : insère les clés de caméra copiées après la clé de la tête de lecture (voir
    /// `paste_insert`) ; les clés d'après sont repoussées. Elles deviennent le choix.
    pub(crate) fn paste_cam_keys(&mut self) {
        let Some(clip) = self.cam_key_clipboard.clone() else {
            self.status = "Aucune clé de caméra copiée.".into();
            return;
        };
        // D'un projet d'une autre feuille ou cadence : adaptées (adaptation.rs, collage.rs).
        let clip = match self.collage_preparer(crate::collage::Colle::CamKeys(clip), crate::collage::Reprise::CamKeys { tete: false }) {
            crate::collage::Preparation::Pret(crate::collage::Colle::CamKeys(c)) => c,
            _ => return,
        };
        let (at, only) = (self.global_frame(), (self.mode == Mode::Animation).then_some(self.cur_panel));
        self.end_stroke();
        let snap = self.snapshot();
        let result = match paste_insert(&mut self.doc, &clip, at, only) {
            Ok(r) => r,
            Err(NoRoom { short }) => {
                self.status = format!(
                    "Rien collé : les clés d'après sortiraient du plan ({short} image{} de trop). Maj+Ctrl+V colle sur la tête de lecture sans rien repousser.",
                    if short > 1 { "s" } else { "" }
                );
                return;
            }
        };
        let pushed = result.pushed;
        let result = result.pasted;
        if result.placed.is_empty() {
            self.status = if only.is_some() {
                "Rien collé : la clé de la tête de lecture est trop près de la fin du plan (en animation, les clés restent dans le plan courant).".into()
            } else {
                "Rien collé : la clé de la tête de lecture est trop près de la fin du plan.".into()
            };
            return;
        }
        let n = result.placed.len();
        let note = match (result.skipped, only.is_some()) {
            (0, _) => String::new(),
            (k, true) => format!(" ; {k} laissée{} de côté (hors du plan courant)", s(k)),
            (k, false) => format!(" ; {k} laissée{} de côté (après la fin du montage)", s(k)),
        };
        let push = if pushed > 0 { format!(", {pushed} clé{} d'après repoussée{}", s(pushed), s(pushed)) } else { String::new() };
        self.status = format!("{n} clé{} de caméra insérée{}{push}{note}.", s(n), s(n));
        self.choose_cam_keys(result.placed);
        self.commit_as(snap, "Insérer des clés de caméra");
    }

    /// Maj+Ctrl+V : colle les clés de caméra copiées à la tête de lecture, la première exactement
    /// dessus, sans rien repousser (ce qu'elles rencontrent est remplacé) ; elles deviennent le choix.
    pub(crate) fn paste_cam_keys_at_head(&mut self) {
        let Some(clip) = self.cam_key_clipboard.clone() else {
            self.status = "Aucune clé de caméra copiée.".into();
            return;
        };
        let clip = match self.collage_preparer(crate::collage::Colle::CamKeys(clip), crate::collage::Reprise::CamKeys { tete: true }) {
            crate::collage::Preparation::Pret(crate::collage::Colle::CamKeys(c)) => c,
            _ => return,
        };
        let (at, only) = (self.global_frame(), (self.mode == Mode::Animation).then_some(self.cur_panel));
        self.end_stroke();
        let snap = self.snapshot();
        let result = paste(&mut self.doc, &clip, at, only);
        if result.placed.is_empty() {
            self.status = if only.is_some() {
                "Rien collé : l'image courante est trop près de la fin du plan (en animation, les clés restent dans le plan courant).".into()
            } else {
                "Rien collé : l'image courante est trop près de la fin du montage.".into()
            };
            return;
        }
        let n = result.placed.len();
        let note = match (result.skipped, only.is_some()) {
            (0, _) => String::new(),
            (k, true) => format!(" ; {k} laissée{} de côté (hors du plan courant)", s(k)),
            (k, false) => format!(" ; {k} laissée{} de côté (après la fin du montage)", s(k)),
        };
        self.status = format!("{n} clé{} de caméra collée{}{note}.", s(n), s(n));
        self.choose_cam_keys(result.placed);
        self.commit_as(snap, "Coller les clés de caméra à la tête de lecture");
    }

    /// Supprime les clés de caméra choisies (rien sans clé choisie).
    pub(crate) fn delete_cam_keys(&mut self) {
        let targets = self.cam_targets();
        if targets.is_empty() {
            self.status = "Pas de clé de caméra choisie : rien n'est supprimé.".into();
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let n = delete(&mut self.doc, &targets);
        self.clear_cam_keys();
        self.status = format!("{n} clé{} de caméra supprimée{}.", s(n), s(n));
        self.commit_as(snap, "Supprimer des clés de caméra");
    }

    /// Les commandes des clés de caméra choisies, pour le menu de la piste Caméra (montage et timeline).
    pub(crate) fn cam_key_menu_items(&mut self, ui: &mut egui::Ui) {
        // Les cinq commandes communes aux trois sortes de clés (`clescommun.rs`).
        self.key_menu_core(ui, crate::clescommun::KeyKind::Camera);
    }

    /// Un losange de clé de caméra dessiné : plus grand et cerclé du contour du thème s'il est choisi.
    pub(crate) fn paint_cam_diamond(&self, painter: &egui::Painter, c: Pos2, chosen: bool, fill: Color32, size: f32, plain_outline: Stroke) {
        let d = if chosen { size + 1.5 } else { size };
        let outline = if chosen { Stroke::new(2.0_f32, crate::theme::contour()) } else { plain_outline };
        painter.add(egui::Shape::convex_polygon(
            vec![c + egui::vec2(0.0, -d), c + egui::vec2(d, 0.0), c + egui::vec2(0.0, d), c + egui::vec2(-d, 0.0)],
            fill,
            outline,
        ));
    }
}
