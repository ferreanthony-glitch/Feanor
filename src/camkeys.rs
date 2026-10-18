//! Clés de caméra qu'on choisit, déplace, copie, colle et supprime, dans le montage et dans la
//! timeline (Anthony, 2026-10-18 : « je veux pouvoir sélectionner une clé de caméra en cliquant
//! dessus et pouvoir la copier-coller dans la timeline »).
//!
//! Même geste que les clés d'effets (`fxkeys.rs`) :
//! - un clic choisit le losange jaune (Ctrl : l'ajoute ou le retire ; Maj : toutes les clés
//!   d'entre le dernier cliqué et lui, d'un plan à l'autre dans le montage) et place la tête de
//!   lecture dessus ; les clés choisies sont cerclées du contour du thème ;
//! - glisser un losange emmène toutes les clés choisies du même nombre d'images (une étape
//!   d'annulation par geste, Échap l'abandonne). Comme avant, une clé ne passe pas par-dessus une
//!   clé qui n'est pas choisie (ni avant l'image 0, ni après la fin de son plan) ;
//! - Ctrl+C copie les clés choisies, Ctrl+V les colle à la tête de lecture en gardant leurs écarts :
//!   dans le montage, elles peuvent passer d'un plan à l'autre (une clé de caméra est comptée
//!   dans son plan, ses valeurs sont celles de la feuille) ; en animation, dans le plan courant
//!   seulement. Une clé qui tomberait après la fin du dernier plan (ou du plan courant) est
//!   laissée de côté, et dite. Suppr supprime les clés choisies, et rien sans clé choisie ;
//! - le clic droit (piste Caméra) a les mêmes commandes ; chaque modification est une étape
//!   d'annulation nommée.
//!
//! La logique est ici en fonctions pures (`choose`, `clamp_delta`, `shift`, `copy`, `paste`,
//! `delete`), testées dans `camkeys_tests.rs` ; `impl App` ne fait que la brancher sur la souris,
//! le clavier et l'historique.

use eframe::egui::{self, Color32, Pos2, Stroke};

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
    if let Some(between) = range.filter(|_| anchor.is_some()) {
        return (between, anchor);
    }
    if toggle {
        let mut out = chosen.to_vec();
        match out.iter().position(|c| *c == clicked) {
            Some(i) => {
                out.remove(i);
            }
            None => out.push(clicked),
        }
        return (out, Some(clicked));
    }
    (vec![clicked], Some(clicked))
}

// ---------------------------------------------------------------------- déplacer

/// Les clés d'un plan prises ensemble : ses images choisies, toutes ses images clés, sa durée.
pub(crate) struct Group {
    pub chosen: Vec<usize>,
    pub all: Vec<usize>,
    pub len: usize,
}

/// Le décalage `want` ramené dans ce que permettent les clés : aucune ne passe avant l'image 0,
/// après la dernière image de son plan, ni par-dessus une clé du plan qui n'est pas choisie.
pub(crate) fn clamp_delta(groups: &[Group], want: isize) -> isize {
    let (mut lo, mut hi) = (isize::MIN, isize::MAX);
    for g in groups {
        for &f in &g.chosen {
            let below = g.all.iter().filter(|a| **a < f && !g.chosen.contains(a)).max().map_or(0, |a| *a as isize + 1);
            let above = g.all.iter().filter(|a| **a > f && !g.chosen.contains(a)).min().map_or(g.len.saturating_sub(1) as isize, |a| *a as isize - 1);
            lo = lo.max(below - f as isize);
            hi = hi.min(above.max(f as isize) - f as isize);
        }
    }
    if lo > hi {
        return 0;
    }
    if lo == isize::MIN || hi == isize::MAX {
        return 0;
    }
    want.clamp(lo, hi)
}

/// Les clés de `keys` aux images `frames` déplacées de `delta` images (le décalage doit être
/// ramené dans les bornes par `clamp_delta`). Renvoie leurs images maintenant.
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
    let starts: Vec<(usize, usize)> = {
        let mut s = 0;
        doc.panels
            .iter()
            .map(|p| {
                let r = (s, p.frame_count());
                s += r.1;
                r
            })
            .collect()
    };
    let mut targets: Vec<(usize, usize, CameraKey)> = Vec::new();
    let mut skipped = 0;
    for ck in &clip.keys {
        let g = at + ck.offset;
        match starts.iter().position(|&(s, len)| g >= s && g < s + len) {
            Some(pi) if only.is_none_or(|o| o == pi) => targets.push((pi, g - starts[pi].0, ck.key)),
            _ => skipped += 1,
        }
    }
    // Les caméras de continuité, avant d'écrire quoi que ce soit.
    let mut firsts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for t in &targets {
        firsts.entry(t.0).and_modify(|f| *f = (*f).min(t.1)).or_insert(t.1);
    }
    let homes: Vec<(usize, camera::Camera)> =
        firsts.into_iter().filter(|&(pi, f)| doc.panels[pi].camera.is_empty() && f > 0).map(|(pi, _)| (pi, doc.panel_camera(pi, 0))).collect();
    for (pi, cam) in homes {
        doc.panels[pi].camera.push(CameraKey::new(0, cam));
    }
    let mut placed = Vec::new();
    for (pi, frame, key) in targets {
        camera::put_key(&mut doc.panels[pi].camera, CameraKey { frame, ..key });
        placed.push(CamKeyRef { panel: doc.panels[pi].id, frame });
    }
    Pasted { placed, skipped }
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
    snap: Edit,
}

struct DragPanel {
    panel: u64,
    before: Vec<CameraKey>,
    chosen: Vec<usize>,
    len: usize,
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
                "{n} clé{} de caméra choisie{} (image {}) : glisser pour déplacer, Ctrl+C, Ctrl+V à l'image courante, Suppr ; clic droit pour le reste.",
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
            panels.push(DragPanel { panel: k.panel, before: p.camera.clone(), chosen: vec![k.frame], len: p.frame_count() });
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.cam_keys_drag = Some(CamKeysDrag { panels, grab, delta: 0, snap });
        true
    }

    /// Les clés prises suivent la souris, à l'image `at` du montage : chaque plan repart de ses
    /// clés d'avant le geste.
    pub(crate) fn drag_cam_keys(&mut self, at: usize) {
        let Some(mut d) = self.cam_keys_drag.take() else { return };
        let groups: Vec<Group> =
            d.panels.iter().map(|p| Group { chosen: p.chosen.clone(), all: p.before.iter().map(|k| k.frame).collect(), len: p.len }).collect();
        let delta = clamp_delta(&groups, at as isize - d.grab as isize);
        if delta != d.delta {
            d.delta = delta;
            for dp in &d.panels {
                let Some(pi) = panel_index(&self.doc, dp.panel) else { continue };
                let mut keys = dp.before.clone();
                shift(&mut keys, &dp.chosen, delta);
                self.doc.panels[pi].camera = keys;
            }
            let n: usize = d.panels.iter().map(|p| p.chosen.len()).sum();
            self.status = format!("{n} clé{} de caméra : {delta:+} image{}.", s(n), if delta.abs() > 1 { "s" } else { "" });
        }
        self.cam_keys_drag = Some(d);
    }

    /// Lâche les clés : une étape d'annulation, « Déplacer des clés de caméra », si quelque chose a bougé.
    pub(crate) fn end_cam_keys_drag(&mut self) {
        let Some(d) = self.cam_keys_drag.take() else { return };
        if d.delta == 0 {
            return;
        }
        let mut sel = Vec::new();
        for dp in &d.panels {
            sel.extend(dp.chosen.iter().map(|&f| CamKeyRef { panel: dp.panel, frame: (f as isize + d.delta).max(0) as usize }));
        }
        self.cam_key_anchor = sel.first().copied();
        self.cam_key_sel = sel;
        self.commit_as(d.snap, "Déplacer des clés de caméra");
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
        self.cam_clip_recent = true;
        self.fx_clip_recent = false;
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau SHOT (`pasteimage`).
        self.ctx.copy_text(format!("{}clés de caméra copiées", crate::pasteimage::MARK));
        self.status = format!("{n} clé{} de caméra copiée{} : Ctrl+V pour les coller à l'image courante.", s(n), s(n));
    }

    /// Colle les clés de caméra copiées à l'image courante ; elles deviennent le choix.
    pub(crate) fn paste_cam_keys(&mut self) {
        let Some(clip) = self.cam_key_clipboard.clone() else {
            self.status = "Aucune clé de caméra copiée.".into();
            return;
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
        self.commit_as(snap, "Coller des clés de caméra");
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
        let n = self.cam_targets().len();
        let what = |one: &str, many: &str| if n > 1 { format!("{many} ({n})") } else { one.to_string() };
        let key = |app: &App, a: &str| app.keymap.label(a);
        if ui.add_enabled(n > 0, egui::Button::new(what("Copier la clé de caméra", "Copier les clés de caméra")).shortcut_text(key(self, "edition.copier"))).clicked() {
            ui.close_menu();
            self.copy_cam_keys();
        }
        let can_paste = self.cam_key_clipboard.is_some();
        if ui
            .add_enabled(can_paste, egui::Button::new("Coller les clés de caméra ici").shortcut_text(key(self, "edition.coller")))
            .on_hover_text("À l'image courante, en gardant l'écart entre les clés ; dans le montage, d'un plan à l'autre.")
            .clicked()
        {
            ui.close_menu();
            self.paste_cam_keys();
        }
        if ui.add_enabled(n > 0, egui::Button::new(what("Supprimer la clé de caméra", "Supprimer les clés de caméra")).shortcut_text(key(self, "edition.supprimer"))).clicked() {
            ui.close_menu();
            self.delete_cam_keys();
        }
        if ui.button("Choisir toutes les clés de caméra du plan").clicked() {
            ui.close_menu();
            self.choose_all_cam_keys();
        }
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
