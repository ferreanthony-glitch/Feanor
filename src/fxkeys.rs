//! Clés d'effets visibles et modifiables dans le montage et dans la timeline (Anthony,
//! 2026-10-18 : « quand un effet est appliqué à un calque et que des clés sont posées, elles ne
//! se voient pas dans la timeline ; je dois les voir, les déplacer, les copier-coller et les
//! supprimer »).
//!
//! Avant, les clés (13i) n'étaient dessinées que dans la timeline de l'animation, en petits
//! losanges collés au bord haut des dessins ; le montage, où Anthony travaille, ne les montrait
//! pas du tout, et une seule clé se prenait à la fois. Maintenant :
//! - un losange violet par clé de l'effet choisi dans le panneau Effets (Anthony, même jour :
//!   « les clés d'effets n'apparaissent que lorsque l'effet est sélectionné ») : seulement sur le
//!   calque de cet effet, en haut de sa piste, dans le montage comme dans la timeline ; aucun
//!   effet choisi, aucun losange. La case « Clés d'effets… » du panneau Effets est un
//!   interrupteur général ;
//! - un clic le choisit (Ctrl : l'ajoute ou le retire ; Maj : tous ceux d'entre l'ancre et lui, sur
//!   ce calque) et place la tête de lecture dessus ; le choix efface celui des dessins et des images ;
//! - glisser un losange (Anthony, 2026-10-19, mêmes règles que les clés de caméra) : par défaut la
//!   clé prise (ou la première des choisies) et TOUTES les clés d'après du même effet glissent du
//!   même nombre d'images (« Décaler les clés d'effet ») ; les clés d'avant ne bougent pas, la
//!   première ne passe pas par-dessus la clé d'avant (`clamp_ripple`). Maj tenue : seules les clés
//!   choisies bougent (« Déplacer une clé d'effet ») ; une clé qui tombe alors sur une autre du
//!   même effet la remplace, comme avant. Une étape d'annulation par geste, Échap l'abandonne ;
//! - Ctrl+C copie les clés choisies. Ctrl+V les insère après la clé de l'effet sur laquelle est la
//!   tête de lecture (sinon la dernière d'avant elle, sinon sur la tête), en gardant leurs écarts,
//!   et repousse les clés d'après de la longueur du bloc (`paste_insert_keys`, « Insérer des clés
//!   d'effet ») ; Maj+Ctrl+V (`edition.coller_a_la_tete`) les colle exactement sur la tête de lecture,
//!   sans rien repousser (`paste_keys`, « Coller les clés d'effet à la tête de lecture »). Elles
//!   vont sur l'effet choisi (de la même sorte que celui copié). Suppr les supprime ; le clic droit
//!   a les mêmes commandes. Suppr sans clé choisie ne fait rien ;
//! - une clé créée (panneau Effets) devient le choix ; changer d'effet choisi lâche le choix des clés.
//!
//! La logique est ici en fonctions pures (`move_keys`, `paste_keys`…), testées dans
//! `fxkeys_tests.rs` ; `impl App` en dessous ne fait que la brancher sur la souris, le clavier et
//! l'historique.

use eframe::egui::{self, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::{App, Mode};
use crate::fx::{Effect, EffectKind, FxKey};
use crate::history::Edit;

/// La couleur d'une clé d'effet : le violet du thème.
pub(crate) fn color() -> Color32 {
    crate::theme::effet()
}

/// Hauteur, depuis le haut d'une piste, de la zone où une clé d'effet se prend.
pub(crate) const STRIP: f32 = 14.0;

/// Une clé d'effet choisie : plan (id), calque (id), effet (rang dans sa pile), image du plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FxKeyRef {
    pub panel: u64,
    pub layer: u64,
    pub effect: usize,
    pub frame: usize,
}

/// Une clé copiée : son écart à la première clé copiée et ses paramètres.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FxClipKey {
    pub offset: usize,
    pub values: Vec<f32>,
}

/// Les clés d'effet copiées : celles d'un seul effet, dont on garde la sorte (les paramètres ne
/// vont qu'à un effet de la même sorte).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FxClip {
    pub kind: EffectKind,
    pub keys: Vec<FxClipKey>,
}

/// Ce que donne un collage.
#[derive(Debug, PartialEq)]
pub(crate) struct Pasted {
    /// Les images où des clés sont posées.
    pub placed: Vec<usize>,
    /// Clés laissées de côté : d'une autre sorte d'effet, ou après la fin du plan.
    pub skipped: usize,
}

/// L'effet dont les clés se montrent : le plan (id), le calque (id) et son rang dans la pile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FxSel {
    pub panel: u64,
    pub layer: u64,
    pub effect: usize,
}

// ---------------------------------------------------------------------- lecture

/// L'effet dont les clés se montrent sur le calque `layer` du plan `panel` (id) : `choice`, si
/// c'est un effet de ce calque, si l'interrupteur général `on` est mis, et si le calque peut avoir
/// des effets (ni dossier, ni vidéo).
pub(crate) fn shown(choice: Option<FxSel>, on: bool, panel: u64, layer: &crate::document::Layer) -> Option<FxSel> {
    let c = choice.filter(|c| on && c.panel == panel && c.layer == layer.id)?;
    (c.effect < layer.fx.len() && !layer.folder && layer.video.is_none()).then_some(c)
}

/// Le rang de l'effet choisi `c` après l'échange des rangs `a` et `b`.
pub(crate) fn rank_after_swap(c: usize, a: usize, b: usize) -> usize {
    if c == a {
        b
    } else if c == b {
        a
    } else {
        c
    }
}

/// Le rang de l'effet choisi `c` après le retrait du rang `removed` : plus de choix s'il était
/// lui-même retiré, un rang de moins s'il venait après.
pub(crate) fn rank_after_remove(c: usize, removed: usize) -> Option<usize> {
    match c.cmp(&removed) {
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(c - 1),
        std::cmp::Ordering::Less => Some(c),
    }
}

/// Les images où l'effet a une clé, dans l'ordre (une par losange).
pub(crate) fn marker_frames(e: &Effect) -> Vec<usize> {
    let mut out: Vec<usize> = e.keys.iter().map(|k| k.frame).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Le losange le plus proche de l'abscisse `px` parmi ceux des images `frames` (`x_of` : le
/// centre d'une image), à `tol` points au plus.
pub(crate) fn marker_near(frames: &[usize], px: f32, x_of: impl Fn(usize) -> f32, tol: f32) -> Option<usize> {
    frames
        .iter()
        .map(|&f| (f, (px - x_of(f)).abs()))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(f, _)| f)
}

/// La clé de l'effet `e` (celui de `sel`) à l'image `frame`, en référence : vide s'il n'y en a pas.
pub(crate) fn refs_on(sel: FxSel, e: &Effect, frame: usize) -> Vec<FxKeyRef> {
    if e.key_at(frame).is_some() {
        vec![FxKeyRef { panel: sel.panel, layer: sel.layer, effect: sel.effect, frame }]
    } else {
        Vec::new()
    }
}

/// Toutes les clés de l'effet `e` (celui de `sel`) des images `a` à `b` (comprises).
pub(crate) fn keys_between(sel: FxSel, e: &Effect, a: usize, b: usize) -> Vec<FxKeyRef> {
    let (a, b) = (a.min(b), a.max(b));
    e.keys.iter().filter(|k| (a..=b).contains(&k.frame)).map(|k| FxKeyRef { panel: sel.panel, layer: sel.layer, effect: sel.effect, frame: k.frame }).collect()
}

// ---------------------------------------------------------------------- choix

/// Le choix après un clic sur un losange dont les clés sont `marker` : lui seul ; `toggle` (Ctrl) :
/// il s'ajoute, ou se retire s'il était entièrement choisi ; `range` (Maj) : les clés `between`
/// (de l'ancre à lui). Renvoie aussi la nouvelle ancre des Maj + clic.
pub(crate) fn choose(chosen: &[FxKeyRef], anchor: Option<FxKeyRef>, marker: &[FxKeyRef], toggle: bool, range: Option<Vec<FxKeyRef>>) -> (Vec<FxKeyRef>, Option<FxKeyRef>) {
    // La règle est la même pour les trois sortes de clés (`clescommun.rs`).
    crate::clescommun::choose_group(chosen, anchor, marker, toggle, range)
}

// ---------------------------------------------------------------------- déplacer

/// Le décalage `want` ramené dans ce que permettent les clés : aucune avant l'image 0, aucune
/// après la dernière image de son plan (sauf une qui y était déjà). `groups` : pour chaque calque
/// touché, les images de ses clés choisies et la durée de son plan.
pub(crate) fn clamp_delta(groups: &[(Vec<usize>, usize)], want: isize) -> isize {
    let lo = groups.iter().flat_map(|g| g.0.iter().copied()).min().map_or(0, |m| -(m as isize));
    let hi = groups
        .iter()
        .filter_map(|(frames, len)| frames.iter().copied().max().map(|m| (len.saturating_sub(1) as isize - m as isize).max(0)))
        .min()
        .unwrap_or(0);
    want.clamp(lo, hi.max(lo))
}

/// Les clés qui suivent le geste « décaler » : pour chaque effet touché, sa première clé choisie
/// et toutes ses clés d'après (choisies ou non). (effet, image), dans l'ordre.
pub(crate) fn ripple_keys(fx: &[Effect], keys: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (ei, e) in fx.iter().enumerate() {
        let Some(first) = keys.iter().filter(|k| k.0 == ei).map(|k| k.1).min() else { continue };
        out.extend(marker_frames(e).into_iter().filter(|f| *f >= first).map(|f| (ei, f)));
    }
    out
}

/// Le décalage `want` ramené dans ce que permet le geste « décaler » : `groups` donne, pour chaque
/// effet touché, (toutes les images de ses clés, la première image choisie, la durée du plan). La
/// première clé choisie ne passe pas par-dessus la clé d'avant (qui ne bouge pas) ni l'image 0, la
/// dernière clé de l'effet reste dans son plan.
pub(crate) fn clamp_ripple(groups: &[(Vec<usize>, usize, usize)], want: isize) -> isize {
    if groups.is_empty() {
        return 0;
    }
    let (mut lo, mut hi) = (isize::MIN, isize::MAX);
    for (all, first, len) in groups {
        let floor = all.iter().copied().filter(|f| f < first).max().map_or(0, |p| p as isize + 1);
        lo = lo.max(floor - *first as isize);
        let last = all.iter().copied().max().unwrap_or(*first);
        hi = hi.min((len.saturating_sub(1) as isize - last as isize).max(0));
    }
    want.clamp(lo, hi.max(lo))
}

/// Les clés `keys` (effet, image) de la pile `fx` déplacées de `delta` images : chacune garde ses
/// paramètres, une clé qui en rencontre une autre du même effet (non choisie) la remplace, les
/// clés restent dans l'ordre des images. Renvoie où elles sont maintenant : (effet, image). Le
/// décalage doit être déjà ramené dans les bornes (`clamp_delta`) ; ici une image ne passe pas
/// sous 0.
pub(crate) fn move_keys(fx: &mut [Effect], keys: &[(usize, usize)], delta: isize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (ei, e) in fx.iter_mut().enumerate() {
        let frames: Vec<usize> = keys.iter().filter(|k| k.0 == ei).map(|k| k.1).collect();
        if frames.is_empty() {
            continue;
        }
        let moved: Vec<FxKey> = e.keys.iter().filter(|k| frames.contains(&k.frame)).cloned().collect();
        e.keys.retain(|k| !frames.contains(&k.frame));
        let mut landed: Vec<usize> = Vec::new();
        for k in moved {
            let frame = (k.frame as isize + delta).max(0) as usize;
            e.keys.retain(|x| x.frame != frame);
            e.keys.push(FxKey { frame, values: k.values });
            landed.push(frame);
        }
        e.keys.sort_by_key(|k| k.frame);
        out.extend(landed.into_iter().map(|f| (ei, f)));
    }
    out
}

// ---------------------------------------------------------------------- copier, coller, supprimer

/// Copie les clés de l'effet `e` aux images `frames`. Les écarts se comptent depuis la première
/// image copiée.
pub(crate) fn copy_keys(e: &Effect, frames: &[usize]) -> FxClip {
    let first = frames.iter().copied().min().unwrap_or(0);
    let mut keys: Vec<FxClipKey> = frames
        .iter()
        .filter_map(|&frame| e.keys.iter().find(|k| k.frame == frame).map(|k| FxClipKey { offset: frame - first, values: k.values.clone() }))
        .collect();
    keys.sort_by_key(|k| k.offset);
    FxClip { kind: e.kind, keys }
}

/// Colle `clip` sur l'effet `e`, à partir de l'image `at` d'un plan de `len` images : chaque clé
/// va à son écart ; elle remplace celle qui y était. Les clés d'une autre sorte d'effet que `e`,
/// ou qui tomberaient après la fin du plan, sont laissées de côté.
pub(crate) fn paste_keys(e: &mut Effect, clip: &FxClip, at: usize, len: usize) -> Pasted {
    let mut placed = Vec::new();
    let mut skipped = 0;
    for k in &clip.keys {
        let frame = at + k.offset;
        if e.kind == clip.kind && frame < len {
            e.set_key(frame, k.values.clone());
            placed.push(frame);
        } else {
            skipped += 1;
        }
    }
    Pasted { placed, skipped }
}

/// Ce que donne un collage par insertion.
#[derive(Debug, PartialEq)]
pub(crate) struct Inserted {
    pub pasted: Pasted,
    /// Clés d'après repoussées.
    pub pushed: usize,
}

/// L'image de la clé de l'effet `e` sur laquelle est la tête de lecture (`playhead`), sinon de la
/// dernière clé d'avant elle ; rien s'il n'y en a pas.
pub(crate) fn anchor_frame(e: &Effect, playhead: usize) -> Option<usize> {
    e.keys.iter().map(|k| k.frame).filter(|f| *f <= playhead).max()
}

/// Insère `clip` sur l'effet `e` après sa clé de la tête de lecture : la première clé collée va à
/// l'image d'après cette clé (la dernière clé d'avant la tête si aucune n'est dessus, la tête
/// elle-même s'il n'y en a pas avant), les autres gardent leurs écarts ; les clés de `e` à partir de
/// là sont repoussées de la longueur du bloc (de la première à la dernière clé collée, comprises) :
/// rien n'est écrasé. Les clés d'une autre sorte d'effet, ou après la fin du plan (`len`), sont
/// laissées de côté. Si une clé repoussée sortait du plan, rien n'est fait.
pub(crate) fn paste_insert_keys(e: &mut Effect, clip: &FxClip, playhead: usize, len: usize) -> Result<Inserted, crate::camkeys::NoRoom> {
    let nothing = || Ok(Inserted { pasted: Pasted { placed: Vec::new(), skipped: clip.keys.len() }, pushed: 0 });
    if e.kind != clip.kind {
        return nothing();
    }
    let at = anchor_frame(e, playhead).map_or(playhead, |a| a + 1);
    let fits: Vec<&FxClipKey> = clip.keys.iter().filter(|k| at + k.offset < len).collect();
    let Some(last) = fits.iter().map(|k| k.offset).max() else { return nothing() };
    let room = last + 1;
    if let Some(max) = e.keys.iter().map(|k| k.frame).filter(|f| *f >= at).max() {
        if max + room > len.saturating_sub(1) {
            return Err(crate::camkeys::NoRoom { short: max + room - len.saturating_sub(1) });
        }
    }
    let mut pushed = 0;
    for k in e.keys.iter_mut().filter(|k| k.frame >= at) {
        k.frame += room;
        pushed += 1;
    }
    let mut placed = Vec::new();
    for k in &fits {
        e.set_key(at + k.offset, k.values.clone());
        placed.push(at + k.offset);
    }
    Ok(Inserted { pasted: Pasted { placed, skipped: clip.keys.len() - fits.len() }, pushed })
}

/// Supprime les clés `keys` (effet, image) de la pile `fx`. Renvoie combien étaient là.
pub(crate) fn delete_keys(fx: &mut [Effect], keys: &[(usize, usize)]) -> usize {
    keys.iter().filter(|&&(ei, frame)| fx.get_mut(ei).is_some_and(|e| e.remove_key(frame))).count()
}

// ---------------------------------------------------------------------- dessin

/// Les losanges d'une piste : `e` l'effet choisi du calque, `x_of` le centre d'une image, `y` leur
/// hauteur, `max` les images du plan, `visible` les abscisses à dessiner, `chosen` les images dont
/// les clés sont choisies (cerclées de blanc).
pub(crate) fn paint_markers(painter: &Painter, e: &Effect, x_of: impl Fn(usize) -> f32, y: f32, max: usize, visible: (f32, f32), chosen: &[usize]) {
    for f in marker_frames(e) {
        let x = x_of(f);
        if f >= max || x < visible.0 - 8.0 || x > visible.1 + 8.0 {
            continue;
        }
        paint_marker(painter, Pos2::new(x, y), chosen.contains(&f));
    }
}

/// Un losange : violet, cerclé de noir ; choisi : plus grand et cerclé de blanc.
fn paint_marker(painter: &Painter, c: Pos2, chosen: bool) {
    let d = if chosen { 6.0 } else { 4.5 };
    let outline = if chosen { Stroke::new(2.0_f32, crate::theme::contour()) } else { Stroke::new(1.0_f32, Color32::from_black_alpha(200)) };
    painter.add(Shape::convex_polygon(vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)], color(), outline));
}

// ---------------------------------------------------------------------- geste en cours

/// Des clés d'effets prises à la souris : chaque calque touché tel qu'il était (sa pile), le point
/// pris, le décalage en cours, l'étape d'annulation de tout le geste.
pub(crate) struct FxKeysDrag {
    layers: Vec<DragLayer>,
    grab: usize,
    delta: isize,
    /// Les clés d'après suivent (par défaut) ; faux : Maj tenue, seules les clés choisies bougent.
    ripple: bool,
    snap: Edit,
}

struct DragLayer {
    panel: u64,
    layer: u64,
    before: Vec<Effect>,
    keys: Vec<(usize, usize)>,
    plan_len: usize,
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

impl App {
    /// Des clés d'effets sont choisies et la bande (montage ou timeline) a la main : copier,
    /// coller et supprimer les visent.
    pub(crate) fn fx_keys_have_focus(&self) -> bool {
        (self.key_focus || self.images_focus) && !self.fx_key_sel.is_empty() && !self.fx_targets().is_empty()
    }

    /// (plan, calque) d'un couple d'identifiants.
    fn fx_layer_at(&self, panel: u64, layer: u64) -> Option<(usize, usize)> {
        let pi = self.doc.panels.iter().position(|p| p.id == panel)?;
        let li = self.doc.panels[pi].layers.iter().position(|l| l.id == layer)?;
        Some((pi, li))
    }

    // ------------------------------------------------------------------ l'effet choisi

    /// Choisit l'effet de rang `effect` dans la pile du calque courant : ses clés, et elles seules,
    /// se montrent dans le montage et la timeline. Changer d'effet lâche le choix des clés.
    pub(crate) fn select_fx(&mut self, effect: usize) {
        let new = self.doc.panels.get(self.cur_panel).and_then(|p| {
            let l = p.layers.get(self.cur_layer)?;
            (effect < l.fx.len()).then_some(FxSel { panel: p.id, layer: l.id, effect })
        });
        self.set_fx_choice(new);
    }

    /// Plus aucun effet choisi : plus de losange.
    pub(crate) fn deselect_fx(&mut self) {
        self.set_fx_choice(None);
    }

    fn set_fx_choice(&mut self, new: Option<FxSel>) {
        if self.fx_choice != new {
            self.fx_choice = new;
            self.fx_key_sel.clear();
            self.fx_key_anchor = None;
        }
    }

    /// Le rang de l'effet choisi dans la pile du calque courant (il est choisi, sur ce calque).
    pub(crate) fn fx_index(&self) -> Option<usize> {
        let c = self.fx_choice?;
        let p = self.doc.panels.get(self.cur_panel)?;
        let l = p.layers.get(self.cur_layer)?;
        (c.panel == p.id && c.layer == l.id && c.effect < l.fx.len()).then_some(c.effect)
    }

    /// L'effet dont les clés se montrent sur le calque `layer` du plan `panel` (id) : celui qui est
    /// choisi, si c'est sur ce calque, si la case du panneau Effets est cochée, et si le calque
    /// peut avoir des effets.
    pub(crate) fn fx_shown(&self, panel: u64, layer: &crate::document::Layer) -> Option<FxSel> {
        shown(self.fx_choice, self.fx_keys_shown, panel, layer)
    }

    /// Les clés de l'effet montré à l'image `frame` du calque `li` du plan `pi` (le losange).
    pub(crate) fn fx_marker_refs(&self, pi: usize, li: usize, frame: usize) -> Vec<FxKeyRef> {
        let Some(p) = self.doc.panels.get(pi) else { return Vec::new() };
        let Some(l) = p.layers.get(li) else { return Vec::new() };
        self.fx_shown(p.id, l).map_or_else(Vec::new, |sel| refs_on(sel, &l.fx[sel.effect], frame))
    }

    /// Les clés choisies qui sont encore là et visibles, par calque : (plan, calque, [(effet, image)]).
    pub(crate) fn fx_targets(&self) -> Vec<(usize, usize, Vec<(usize, usize)>)> {
        let mut out: Vec<(usize, usize, Vec<(usize, usize)>)> = Vec::new();
        for k in &self.fx_key_sel {
            let Some((pi, li)) = self.fx_layer_at(k.panel, k.layer) else { continue };
            let layer = &self.doc.panels[pi].layers[li];
            if self.fx_shown(k.panel, layer).is_none_or(|s| s.effect != k.effect) || layer.fx.get(k.effect).is_none_or(|e| e.key_at(k.frame).is_none()) {
                continue;
            }
            match out.iter_mut().find(|t| t.0 == pi && t.1 == li) {
                Some(t) => t.2.push((k.effect, k.frame)),
                None => out.push((pi, li, vec![(k.effect, k.frame)])),
            }
        }
        out.sort_by_key(|t| (t.0, t.1));
        out
    }

    /// Donne la main aux clés d'effets choisies : elles remplacent le choix des dessins et des images.
    fn give_fx_keys_focus(&mut self) {
        self.clear_cam_keys();
        self.key_sel.clear();
        self.key_anchor = None;
        self.frame_sel = None;
        self.panel_focus = false;
        // (Les clés qu'on vient de créer ou de coller ont la main, pas le panneau Effets : `fxcopie.rs`.)
        self.fx_pane_focus = false;
        if self.mode == Mode::Storyboard {
            self.key_focus = true;
        } else {
            self.images_focus = true;
        }
    }

    /// Clic sur le losange de l'image `frame` du calque `li` du plan `pi` : voir `choose`. La tête
    /// de lecture va sur le losange et le calque devient courant.
    pub(crate) fn click_fx_marker(&mut self, pi: usize, li: usize, frame: usize, toggle: bool, range: bool) {
        let Some((marker, between, name)) = self.doc.panels.get(pi).and_then(|p| {
            let l = p.layers.get(li)?;
            let shown = self.fx_shown(p.id, l)?;
            let e = &l.fx[shown.effect];
            let marker = refs_on(shown, e, frame);
            let between = match (range, self.fx_key_anchor) {
                (true, Some(a)) if a.panel == p.id && a.layer == l.id => Some(keys_between(shown, e, a.frame, frame)),
                _ => None,
            };
            Some((marker, between, e.kind.label()))
        }) else {
            return;
        };
        if marker.is_empty() {
            return;
        }
        let (sel, anchor) = choose(&self.fx_key_sel, self.fx_key_anchor, &marker, toggle, between);
        self.fx_key_sel = sel;
        self.fx_key_anchor = anchor;
        self.give_fx_keys_focus();
        if self.mode == Mode::Storyboard {
            let g = self.doc.panel_start(pi) + frame;
            self.goto_global(g);
        } else {
            self.set_frame_free(frame);
        }
        self.select_layer(li);
        let n = self.fx_key_sel.len();
        self.status = if n == 0 {
            "Plus aucune clé d'effet choisie.".into()
        } else {
            format!("{n} clé{} d'effet choisie{} (image {} : {name}) : glisser pour décaler (Maj : cette clé seule), Ctrl+C, Ctrl+V insère après la clé (Maj+Ctrl+V : sur la tête), Suppr ; clic droit pour le reste.", s(n), s(n), self.frame_label(frame))
        };
    }

    /// Choisit toutes les clés de l'effet choisi (Ctrl+A après un clic sur un losange violet, ou
    /// « Choisir toutes les clés de l'effet » du clic droit) : comme pour les clés de caméra et
    /// de dessins (`clescommun.rs`).
    pub(crate) fn choose_all_fx_keys(&mut self) {
        let all = self.fx_choice.and_then(|c| {
            let (pi, li) = self.fx_layer_at(c.panel, c.layer)?;
            let layer = &self.doc.panels[pi].layers[li];
            let shown = self.fx_shown(c.panel, layer)?;
            Some(keys_between(shown, &layer.fx[shown.effect], 0, usize::MAX))
        });
        let Some(all) = all.filter(|a| !a.is_empty()) else {
            self.status = "Pas de clé d'effet à choisir : choisir un effet qui a des clés dans le panneau Effets.".into();
            return;
        };
        self.fx_key_anchor = all.first().copied();
        self.fx_key_sel = all;
        self.give_fx_keys_focus();
        let n = self.fx_key_sel.len();
        self.status = format!("{n} clé{} d'effet choisie{} (tout l'effet).", s(n), s(n));
    }

    /// Une clé d'effet vient d'être créée (panneau Effets) : elle seule est choisie.
    pub(crate) fn select_new_fx_key(&mut self, li: usize, effect: usize, frame: usize) {
        let Some((panel, layer)) = self.doc.panels.get(self.cur_panel).and_then(|p| p.layers.get(li).map(|l| (p.id, l.id))) else { return };
        let k = FxKeyRef { panel, layer, effect, frame };
        self.fx_key_sel = vec![k];
        self.fx_key_anchor = Some(k);
        self.give_fx_keys_focus();
    }

    // ------------------------------------------------------------------ déplacer

    /// Commence à glisser les clés choisies, en prenant le losange de l'image `frame` du calque
    /// `li` du plan `pi` ; `grab` : l'image sous la souris (même repère que `drag_fx_keys`). Un
    /// losange pris hors du choix devient le choix. Faux si refusé (storyboard verrouillé).
    pub(crate) fn begin_fx_keys_drag(&mut self, pi: usize, li: usize, frame: usize, grab: usize) -> bool {
        if self.content_refuses() {
            return false;
        }
        let marker = self.fx_marker_refs(pi, li, frame);
        let in_choice = !marker.is_empty() && marker.iter().all(|k| self.fx_key_sel.contains(k));
        if !in_choice {
            self.click_fx_marker(pi, li, frame, false, false);
        }
        let layers: Vec<DragLayer> = self
            .fx_targets()
            .into_iter()
            .map(|(pi, li, keys)| {
                let (p, l) = (&self.doc.panels[pi], &self.doc.panels[pi].layers[li]);
                DragLayer { panel: p.id, layer: l.id, before: l.fx.clone(), keys, plan_len: p.frame_count() }
            })
            .collect();
        if layers.is_empty() {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.fx_keys_drag = Some(FxKeysDrag { layers, grab, delta: 0, ripple: true, snap });
        true
    }

    /// Les clés prises suivent la souris, à l'image `at` : chaque calque repart de sa pile d'avant
    /// le geste. Maj tenue (même en cours de geste) : seules les clés choisies bougent ; sinon les
    /// clés d'après du même effet les suivent.
    pub(crate) fn drag_fx_keys(&mut self, at: usize) {
        let ripple = !self.ctx.input(|i| i.modifiers.shift);
        self.drag_fx_keys_mode(at, ripple);
    }

    pub(crate) fn drag_fx_keys_mode(&mut self, at: usize, ripple: bool) {
        let Some(mut d) = self.fx_keys_drag.take() else { return };
        let want = at as isize - d.grab as isize;
        let delta = if ripple {
            // Par effet touché : toutes ses clés, sa première clé choisie, la durée du plan.
            let mut groups: Vec<(Vec<usize>, usize, usize)> = Vec::new();
            for dl in &d.layers {
                for (ei, e) in dl.before.iter().enumerate() {
                    if let Some(first) = dl.keys.iter().filter(|k| k.0 == ei).map(|k| k.1).min() {
                        groups.push((marker_frames(e), first, dl.plan_len));
                    }
                }
            }
            clamp_ripple(&groups, want)
        } else {
            let groups: Vec<(Vec<usize>, usize)> = d.layers.iter().map(|l| (l.keys.iter().map(|k| k.1).collect(), l.plan_len)).collect();
            clamp_delta(&groups, want)
        };
        if delta != d.delta || ripple != d.ripple {
            d.delta = delta;
            d.ripple = ripple;
            for dl in &d.layers {
                let Some((pi, li)) = self.fx_layer_at(dl.panel, dl.layer) else { continue };
                let mut fx = dl.before.clone();
                let moving = if ripple { ripple_keys(&dl.before, &dl.keys) } else { dl.keys.clone() };
                move_keys(&mut fx, &moving, delta);
                self.doc.panels[pi].layers[li].fx = fx;
            }
            self.renderer.invalidate_all();
            let n: usize = d.layers.iter().map(|l| l.keys.len()).sum();
            let who = if ripple { "et les clés d'après" } else { "seule, Maj tenue" };
            self.status = format!("{n} clé{} d'effet {who} : {delta:+} image{}.", s(n), if delta.abs() > 1 { "s" } else { "" });
        }
        self.fx_keys_drag = Some(d);
    }

    /// Lâche les clés : une étape d'annulation, « Décaler les clés d'effet » (les clés d'après ont
    /// suivi) ou « Déplacer une clé d'effet » (Maj tenue), si quelque chose a bougé.
    pub(crate) fn end_fx_keys_drag(&mut self) {
        let Some(d) = self.fx_keys_drag.take() else { return };
        if d.delta == 0 {
            return;
        }
        let mut sel = Vec::new();
        for dl in &d.layers {
            sel.extend(dl.keys.iter().map(|&(effect, frame)| FxKeyRef {
                panel: dl.panel,
                layer: dl.layer,
                effect,
                frame: (frame as isize + d.delta).max(0) as usize,
            }));
        }
        let several = sel.len() > 1;
        self.fx_key_anchor = sel.first().copied();
        self.fx_key_sel = sel;
        let name = if d.ripple {
            "Décaler les clés d'effet"
        } else if several {
            "Déplacer des clés d'effet"
        } else {
            "Déplacer une clé d'effet"
        };
        self.commit_named(d.snap, name);
    }

    /// Échap pendant le geste : les piles reviennent comme avant, le choix reste.
    pub(crate) fn cancel_fx_keys_drag(&mut self) {
        let Some(d) = self.fx_keys_drag.take() else { return };
        for dl in d.layers {
            if let Some((pi, li)) = self.fx_layer_at(dl.panel, dl.layer) {
                self.doc.panels[pi].layers[li].fx = dl.before;
            }
        }
        self.renderer.invalidate_all();
    }

    /// Enregistre l'étape `snap` sous le nom `name` (le panneau Historique le montre).
    pub(crate) fn commit_named(&mut self, snap: Edit, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.commit_structure(snap);
        self.history.context = before;
    }

    // ------------------------------------------------------------------ copier, coller, supprimer

    /// Copie les clés d'effets choisies.
    pub(crate) fn copy_fx_keys(&mut self) {
        let targets = self.fx_targets();
        let Some((pi, li, keys)) = targets.first() else {
            self.status = "Pas de clé d'effet choisie : choisir un effet dans le panneau Effets, puis cliquer sur un losange violet de la piste de son calque.".into();
            return;
        };
        let Some(e) = keys.first().and_then(|k| self.doc.panels[*pi].layers[*li].fx.get(k.0)) else { return };
        let clip = copy_keys(e, &keys.iter().map(|k| k.1).collect::<Vec<_>>());
        let (n, kind) = (clip.keys.len(), clip.kind.label());
        self.fx_key_clipboard = Some(clip);
        self.collage.origines.effets_cles = Some(crate::adaptation::Origine::de(&self.doc));
        self.fx_clip_recent = true;
        self.cam_clip_recent = false;
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau SHOT (`pasteimage`).
        self.ctx.copy_text(format!("{}clés d'effets copiées", crate::pasteimage::MARK));
        self.status = format!("{n} clé{} d'effet copiée{} : Ctrl+V pour les insérer après la clé de la tête de lecture, Maj+Ctrl+V pour les coller sur la tête, sur l'effet choisi ({kind}).", s(n), s(n));
    }

    /// Ctrl+V : insère les clés d'effets copiées sur l'effet choisi (de la même sorte que celui
    /// copié), après sa clé de la tête de lecture ; les clés d'après sont repoussées.
    pub(crate) fn paste_fx_keys(&mut self) {
        self.paste_fx_keys_with(true);
    }

    /// Maj+Ctrl+V : colle les clés d'effets copiées exactement sur la tête de lecture, sans rien
    /// repousser (ce qu'elles rencontrent est remplacé).
    pub(crate) fn paste_fx_keys_at_head(&mut self) {
        self.paste_fx_keys_with(false);
    }

    /// Colle les clés d'effets copiées, sur l'effet choisi ; elles deviennent le choix.
    /// `insert` : après la clé de la tête de lecture, en repoussant la suite ; sinon sur la tête.
    fn paste_fx_keys_with(&mut self, insert: bool) {
        let Some(clip) = self.fx_key_clipboard.clone() else {
            self.status = "Aucune clé d'effet copiée.".into();
            return;
        };
        if self.refuse_on_folder() {
            return;
        }
        // D'un projet d'une autre feuille ou cadence : adaptées (adaptation.rs, collage.rs).
        let clip = match self.collage_preparer(crate::collage::Colle::FxKeys(clip), crate::collage::Reprise::FxKeys { insert }) {
            crate::collage::Preparation::Pret(crate::collage::Colle::FxKeys(c)) => c,
            _ => return,
        };
        let Some(sel) = self.fx_choice.filter(|c| self.fx_keys_shown && self.fx_layer_at(c.panel, c.layer).is_some()) else {
            self.status = "Pas d'effet choisi : en choisir un dans le panneau Effets (et garder cochée la case des clés d'effets) pour y coller les clés.".into();
            return;
        };
        let Some((pi, li)) = self.fx_layer_at(sel.panel, sel.layer) else { return };
        let Some(kind) = self.doc.panels[pi].layers[li].fx.get(sel.effect).map(|e| e.kind) else { return };
        if pi != self.cur_panel {
            self.status = "L'effet choisi est dans un autre plan : se placer dans ce plan pour y coller les clés.".into();
            return;
        }
        if kind != clip.kind {
            self.status = format!("Rien collé : les clés copiées sont celles d'un effet « {} », l'effet choisi est « {} ».", clip.kind.label(), kind.label());
            return;
        }
        let (at, len) = (self.cur_frame, self.frame_count());
        self.end_stroke();
        let snap = self.snapshot();
        let effect = &mut self.doc.panels[pi].layers[li].fx[sel.effect];
        let (result, pushed) = if insert {
            match paste_insert_keys(effect, &clip, at, len) {
                Ok(r) => (r.pasted, r.pushed),
                Err(crate::camkeys::NoRoom { short }) => {
                    self.status = format!(
                        "Rien collé : les clés d'après sortiraient du plan ({short} image{} de trop). Maj+Ctrl+V colle sur la tête de lecture sans rien repousser.",
                        if short > 1 { "s" } else { "" }
                    );
                    return;
                }
            }
        } else {
            (paste_keys(effect, &clip, at, len), 0)
        };
        if result.placed.is_empty() {
            self.status = "Rien collé : l'image courante est trop près de la fin du plan.".into();
            return;
        }
        let sel_keys: Vec<FxKeyRef> = result.placed.iter().map(|&frame| FxKeyRef { panel: sel.panel, layer: sel.layer, effect: sel.effect, frame }).collect();
        self.fx_key_anchor = sel_keys.first().copied();
        self.fx_key_sel = sel_keys;
        self.give_fx_keys_focus();
        let n = result.placed.len();
        let verb = if insert { "insérée" } else { "collée" };
        let push = if pushed > 0 { format!(", {pushed} clé{} d'après repoussée{}", s(pushed), s(pushed)) } else { String::new() };
        let skipped = if result.skipped > 0 { format!(" ; {} laissée{} de côté (après la fin du plan)", result.skipped, s(result.skipped)) } else { String::new() };
        self.status = format!("{n} clé{} d'effet {verb}{}{push}{skipped}.", s(n), s(n));
        self.renderer.invalidate_all();
        self.commit_named(snap, if insert { "Insérer des clés d'effet" } else { "Coller les clés d'effet à la tête de lecture" });
    }

    /// Supprime les clés d'effets choisies (rien sans clé choisie).
    pub(crate) fn delete_fx_keys(&mut self) {
        let targets = self.fx_targets();
        if targets.is_empty() {
            self.status = "Pas de clé d'effet choisie : rien n'est supprimé.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let mut n = 0;
        for (pi, li, keys) in &targets {
            n += delete_keys(&mut self.doc.panels[*pi].layers[*li].fx, keys);
        }
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.status = format!("{n} clé{} d'effet supprimée{}.", s(n), s(n));
        self.commit_named(snap, "Supprimer des clés d'effet");
    }

    /// Le menu du clic droit sur un losange (montage et timeline).
    pub(crate) fn fx_key_menu(&mut self, ui: &mut egui::Ui) {
        // Les cinq commandes communes aux trois sortes de clés (`clescommun.rs`), puis ce qui est
        // propre aux effets.
        self.key_menu_core(ui, crate::clescommun::KeyKind::Effect);
        ui.separator();
        self.action_item(ui, "Pile d'effets du calque…", "fx.panneau");
        self.action_item(ui, "Clés d'effets (montrer / cacher)", "fx.cles_timeline");
    }

    /// Les images des clés choisies du calque `layer` du plan `panel` (pour cercler les losanges).
    pub(crate) fn fx_chosen_frames(&self, panel: u64, layer: u64) -> Vec<usize> {
        self.fx_key_sel.iter().filter(|k| k.panel == panel && k.layer == layer).map(|k| k.frame).collect()
    }

    /// Les losanges de la piste d'un calque : ceux de l'effet choisi, si c'est un effet de ce
    /// calque et que la case du panneau Effets les montre.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_fx_markers(&self, painter: &Painter, panel: u64, layer: &crate::document::Layer, x_of: impl Fn(usize) -> f32, y: f32, max: usize, visible: (f32, f32)) {
        let Some(sel) = self.fx_shown(panel, layer) else { return };
        paint_markers(painter, &layer.fx[sel.effect], x_of, y, max, visible, &self.fx_chosen_frames(panel, layer.id));
    }

    /// Le losange sous le point `p` dans la timeline de l'animation : (calque, image).
    pub(crate) fn fx_marker_in_timeline(&self, p: Pos2, r: Rect, cell: f32, rows: &[crate::layerheads::Row], grid_top: f32) -> Option<(usize, usize)> {
        let row = crate::layerheads::row_at(rows, p.y - grid_top)?;
        if p.y - grid_top - row.top > STRIP {
            return None;
        }
        let layer = self.layers().get(row.li)?;
        let sel = self.fx_shown(self.panel().id, layer)?;
        let tol = (cell * 0.5).max(5.0);
        let f = marker_near(&marker_frames(&layer.fx[sel.effect]), p.x, |f| r.left() + (f as f32 + 0.5) * cell, tol)?;
        Some((row.li, f))
    }
}
