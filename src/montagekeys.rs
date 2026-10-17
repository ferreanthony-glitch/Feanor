//! Clés du montage choisies à plusieurs (Anthony, 2026-10-17 : « choix et sélection de
//! plusieurs clés à la fois ») et leurs commandes.
//! - Un clic sur un dessin de la piste d'un calque le choisit seul ; Ctrl + clic l'ajoute ou le
//!   retire ; Maj + clic choisit toutes les clés de la piste entre la dernière cliquée et
//!   celle-ci, d'un plan à l'autre ; un clic à côté des dessins vide le choix. Ctrl+A : toutes
//!   les clés du calque courant dans le plan courant.
//! - Une clé choisie prise par son milieu emmène les autres clés choisies (`KeysDrag`) : toutes
//!   glissent du même nombre d'images, chacune prend sa place parmi les autres expositions de son
//!   calque (`shift_runs`), toutes gardent leur durée. Échap rend tout comme avant ; une étape
//!   d'annulation.
//! - Copier, coller, supprimer (la voisine remplace, ou un vide) et dupliquer sur place visent
//!   les clés choisies, sinon la clé à l'image courante du calque courant. Collées, les clés
//!   gardent leurs écarts et leurs durées, sur le calque courant à partir de l'image courante.

use crate::app::App;
use crate::document::{Cel, CelId, Layer, LayerKind};
use crate::handles::{duplicate_run, overwrite_run, remove_run, runs, shift_runs};
use crate::montagerows::{layer_on_track, TrackKey};

/// Une clé : plan (id), calque (id), image où commence son exposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KeyRef {
    pub panel: u64,
    pub layer: u64,
    pub start: usize,
}

/// Clés copiées, dans l'ordre du montage.
pub(crate) struct KeyClip {
    pub keys: Vec<ClipKey>,
}

/// Une clé copiée : son écart à la première (en images du montage), sa durée, son dessin copié
/// tel qu'il était, la sorte de son calque.
pub(crate) struct ClipKey {
    pub offset: usize,
    pub len: usize,
    pub cel: Cel,
    pub kind: LayerKind,
}

/// Plusieurs clés prises à la fois dans le montage : chaque calque touché tel qu'il était, l'image
/// du montage où on les a prises, le décalage en cours, l'étape d'annulation de tout le geste.
pub(crate) struct KeysDrag {
    layers: Vec<DragLayer>,
    grab: usize,
    delta: isize,
    snap: crate::history::Edit,
}

/// Un calque touché : plan (id), calque (id), son état d'avant, la durée du plan, le début de
/// ses clés prises.
struct DragLayer {
    panel: u64,
    layer: u64,
    before: Layer,
    plan_len: usize,
    starts: Vec<usize>,
}

/// Les clés `chosen` après un clic sur `key` (rien : à côté des dessins). `toggle` (Ctrl) :
/// l'ajoute ou la retire ; `range` (Maj) : les clés `between` (de la dernière cliquée à
/// celle-ci) ; sinon elle seule. Renvoie aussi la nouvelle clé de départ des Maj + clic.
pub(crate) fn choose(chosen: &[KeyRef], anchor: Option<KeyRef>, key: Option<KeyRef>, toggle: bool, range: Option<Vec<KeyRef>>) -> (Vec<KeyRef>, Option<KeyRef>) {
    let Some(k) = key else {
        // À côté des dessins : sans touche, le choix se vide.
        return if toggle || range.is_some() { (chosen.to_vec(), anchor) } else { (Vec::new(), anchor) };
    };
    if let Some(between) = range.filter(|_| anchor.is_some()) {
        return (between, anchor);
    }
    if toggle {
        let mut out = chosen.to_vec();
        match out.iter().position(|c| *c == k) {
            Some(i) => {
                out.remove(i);
            }
            None => out.push(k),
        }
        return (out, Some(k));
    }
    (vec![k], Some(k))
}

impl App {
    /// La clé du plan `pi`, calque `li`, qui contient l'image `f` du plan.
    pub(crate) fn key_ref(&self, pi: usize, li: usize, f: usize) -> Option<KeyRef> {
        let p = self.doc.panels.get(pi)?;
        let l = p.layers.get(li)?;
        l.frames.get(f).copied().flatten()?;
        Some(KeyRef { panel: p.id, layer: l.id, start: l.run_bounds(f).0 })
    }

    /// Où est la clé `k` : (plan, calque, début, fin), si elle est toujours là.
    fn locate(&self, k: KeyRef) -> Option<(usize, usize, usize, usize)> {
        let pi = self.doc.panels.iter().position(|p| p.id == k.panel)?;
        let li = self.doc.panels[pi].layers.iter().position(|l| l.id == k.layer)?;
        let l = &self.doc.panels[pi].layers[li];
        l.frames.get(k.start).copied().flatten()?;
        let (s, e) = l.run_bounds(k.start);
        (s == k.start).then_some((pi, li, s, e))
    }


    /// Un clic sur la piste `track` du montage, sur la clé `key` (rien : à côté des dessins),
    /// avec Ctrl (`toggle`) ou Maj (`range`).
    pub(crate) fn click_key(&mut self, key: Option<KeyRef>, track: Option<&TrackKey>, toggle: bool, range: bool) {
        let between = match (range, key, self.key_anchor, track) {
            (true, Some(k), Some(a), Some(t)) => Some(self.keys_between(a, k, t)),
            _ => None,
        };
        let (sel, anchor) = choose(&self.key_sel, self.key_anchor, key, toggle, between);
        self.key_sel = sel;
        self.key_anchor = anchor;
        if self.key_sel.len() > 1 {
            self.status = format!("{} clés choisies : Ctrl+C, Ctrl+V, Suppr, Ctrl+D (dupliquer) ; clic droit pour le reste.", self.key_sel.len());
        }
    }

    /// Commence à glisser les clés choisies, prises à l'image `grab` du montage. Faux si refusé
    /// (storyboard verrouillé) ou si aucune n'est plus là.
    pub(crate) fn begin_keys_drag(&mut self, grab: usize) -> bool {
        if self.content_refuses() {
            return false;
        }
        let mut layers: Vec<DragLayer> = Vec::new();
        for k in self.key_sel.clone() {
            let Some((pi, li, s, _)) = self.locate(k) else { continue };
            match layers.iter_mut().find(|d| d.panel == k.panel && d.layer == k.layer) {
                Some(d) => d.starts.push(s),
                None => {
                    let p = &self.doc.panels[pi];
                    layers.push(DragLayer { panel: k.panel, layer: k.layer, before: p.layers[li].clone(), plan_len: p.frame_count(), starts: vec![s] });
                }
            }
        }
        if layers.is_empty() {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.keys_drag = Some(KeysDrag { layers, grab, delta: 0, snap });
        true
    }

    /// Les clés prises suivent la souris, à l'image `at` du montage : chaque calque repart de son
    /// état d'avant le geste.
    pub(crate) fn drag_keys(&mut self, at: usize) {
        let Some(d) = self.keys_drag.as_mut() else { return };
        let delta = at as isize - d.grab as isize;
        if delta == d.delta {
            return;
        }
        d.delta = delta;
        let work: Vec<(u64, u64, Layer, usize, Vec<usize>)> = d.layers.iter().map(|l| (l.panel, l.layer, l.before.clone(), l.plan_len, l.starts.clone())).collect();
        let mut sel = Vec::new();
        for (panel, layer, mut l, plan_len, starts) in work {
            let new = shift_runs(&mut l, &starts, delta);
            let Some(pi) = self.doc.panels.iter().position(|p| p.id == panel) else { continue };
            let Some(li) = self.doc.panels[pi].layers.iter().position(|x| x.id == layer) else { continue };
            let p = &mut self.doc.panels[pi];
            p.layers[li] = l;
            // La durée du plan reste celle d'avant le geste.
            if p.frame_count() < plan_len {
                p.length = plan_len;
            }
            sel.extend(new.into_iter().map(|start| KeyRef { panel, layer, start }));
        }
        let n = sel.len();
        self.key_sel = sel;
        self.renderer.invalidate_all();
        self.status = format!("{n} clés glissées de {delta:+} image{} : chacune prend sa place parmi les autres, sa durée gardée.", if delta.abs() > 1 { "s" } else { "" });
    }

    /// Lâche les clés : une étape d'annulation si quelque chose a bougé.
    pub(crate) fn end_keys_drag(&mut self) {
        let Some(d) = self.keys_drag.take() else { return };
        let changed = d.layers.iter().any(|l| {
            self.doc.panels.iter().find(|p| p.id == l.panel).and_then(|p| p.layers.iter().find(|x| x.id == l.layer)).is_some_and(|x| x.frames != l.before.frames)
        });
        if changed {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    /// Échap pendant le geste : chaque calque revient comme avant, le choix aussi.
    pub(crate) fn cancel_keys_drag(&mut self) {
        let Some(d) = self.keys_drag.take() else { return };
        let mut sel = Vec::new();
        for l in d.layers {
            let Some(pi) = self.doc.panels.iter().position(|p| p.id == l.panel) else { continue };
            if let Some(li) = self.doc.panels[pi].layers.iter().position(|x| x.id == l.layer) {
                self.doc.panels[pi].layers[li] = l.before;
            }
            sel.extend(l.starts.into_iter().map(|start| KeyRef { panel: l.panel, layer: l.layer, start }));
        }
        self.key_sel = sel;
        self.renderer.invalidate_all();
    }

    /// Les clés de la piste `track` entre `a` et `b` (comprises), d'un plan à l'autre.
    fn keys_between(&self, a: KeyRef, b: KeyRef, track: &TrackKey) -> Vec<KeyRef> {
        let global = |k: KeyRef| self.locate(k).map(|(pi, _, s, _)| self.doc.panel_start(pi) + s);
        let (Some(ga), Some(gb)) = (global(a), global(b)) else { return vec![b] };
        let (lo, hi) = (ga.min(gb), ga.max(gb));
        let mut out = Vec::new();
        let mut start = 0;
        for p in &self.doc.panels {
            if let Some(li) = layer_on_track(&p.layers, track) {
                let l = &p.layers[li];
                for (s, _, cel) in runs(l) {
                    if cel.is_some() && (lo..=hi).contains(&(start + s)) {
                        out.push(KeyRef { panel: p.id, layer: l.id, start: s });
                    }
                }
            }
            start += p.frame_count();
        }
        out
    }

    /// Ctrl+A après un clic sur une piste : toutes les clés du calque courant dans le plan courant.
    pub(crate) fn choose_all_keys(&mut self) {
        let (panel, l) = (self.doc.panels[self.cur_panel].id, self.layer());
        let (layer, name) = (l.id, l.name.clone());
        let keys: Vec<KeyRef> = runs(l).into_iter().filter(|r| r.2.is_some()).map(|r| KeyRef { panel, layer, start: r.0 }).collect();
        self.key_sel = keys;
        self.status = format!("{} clé(s) de « {name} » choisies.", self.key_sel.len());
    }

    /// Les clés visées : les choisies (toujours là), sinon celle de l'image courante du calque
    /// courant ; (plan, calque, début, fin), dans l'ordre du montage.
    fn target_keys(&mut self) -> Vec<(usize, usize, usize, usize)> {
        let mut out: Vec<(usize, usize, usize, usize)> = self.key_sel.iter().filter_map(|k| self.locate(*k)).collect();
        if out.is_empty() {
            let (f, l) = (self.cur_frame, self.layer());
            if l.frames.get(f).copied().flatten().is_some() {
                let (s, e) = l.run_bounds(f);
                out.push((self.cur_panel, self.cur_layer, s, e));
            }
        }
        out.sort_by_key(|&(pi, li, s, _)| (pi, li, s));
        out.dedup();
        if out.is_empty() {
            self.status = "Pas de clé ici : cliquer sur un dessin de la piste d'un calque.".into();
        }
        out
    }

    /// Le dessin d'une clé.
    fn key_cel(&self, pi: usize, li: usize, s: usize) -> Option<CelId> {
        self.doc.panels[pi].layers[li].frames.get(s).copied().flatten()
    }

    /// Copie les clés visées (Ctrl+C après un clic sur une piste).
    pub(crate) fn copy_keys(&mut self) {
        let targets = self.target_keys();
        let Some(&(p0, _, s0, _)) = targets.iter().min_by_key(|&&(pi, _, s, _)| self.doc.panel_start(pi) + s) else { return };
        let g0 = self.doc.panel_start(p0) + s0;
        let mut keys = Vec::new();
        for &(pi, li, s, e) in &targets {
            let Some(cel) = self.key_cel(pi, li, s).and_then(|id| self.doc.cels.get(&id)).cloned() else { continue };
            let kind = self.doc.panels[pi].layers[li].kind;
            keys.push(ClipKey { offset: self.doc.panel_start(pi) + s - g0, len: e - s, cel, kind });
        }
        keys.sort_by_key(|k| k.offset);
        let n = keys.len();
        self.key_clipboard = Some(KeyClip { keys });
        self.status = format!("{n} clé{} copiée{} : Ctrl+V pour coller à l'image courante d'un calque.", s(n), s(n));
    }

    /// Colle les clés copiées sur le calque courant, à partir de l'image courante : chacune avec
    /// son écart et sa durée (sans dépasser la fin du plan), à la place de ce qui y était ; de
    /// nouveaux dessins, copies des siens. Les clés collées deviennent le choix.
    pub(crate) fn paste_keys(&mut self) {
        if self.key_clipboard.is_none() || self.refuse_on_folder() {
            return;
        }
        let kind = self.layer().kind;
        let plan = self.frame_count();
        let f0 = self.cur_frame.min(plan.saturating_sub(1));
        let li = self.cur_layer;
        let wanted: Vec<(usize, usize, Cel)> = self
            .key_clipboard
            .as_ref()
            .map(|c| c.keys.iter().filter(|k| k.kind == kind && f0 + k.offset < plan).map(|k| (f0 + k.offset, k.len.min(plan - f0 - k.offset).max(1), k.cel.clone())).collect())
            .unwrap_or_default();
        let total = self.key_clipboard.as_ref().map_or(0, |c| c.keys.len());
        if wanted.is_empty() {
            self.status = "Rien à coller ici (calque d'une autre sorte, ou après la fin du plan).".into();
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let (panel, layer) = (self.doc.panels[self.cur_panel].id, self.layer().id);
        let mut pasted = Vec::new();
        for (f, n, cel) in wanted {
            let id = self.doc.add_cel(cel);
            overwrite_run(&mut self.layers_mut()[li], f, n, id);
            pasted.push(KeyRef { panel, layer, start: f });
        }
        let n = pasted.len();
        self.key_sel = pasted;
        self.vsel = None;
        self.commit_structure(snap);
        let skipped = total - n;
        self.status = if skipped > 0 {
            format!("{n} clé{} collée{} ; {skipped} non (calque d'une autre sorte, ou après la fin du plan).", s(n), s(n))
        } else {
            format!("{n} clé{} collée{}.", s(n), s(n))
        };
    }

    /// Supprime les clés visées : la voisine remplace chacune (celle d'avant, sinon celle
    /// d'après), ou (`gap`) elles laissent un vide et les autres clés ne bougent pas.
    pub(crate) fn delete_keys(&mut self, gap: bool) {
        if self.refuse_on_folder() {
            return;
        }
        let targets = self.target_keys();
        if targets.is_empty() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        // De la dernière à la première : une clé remplacée par sa voisine choisie, puis cette
        // voisine supprimée à son tour, laisse la place à celle d'avant elles deux.
        for &(pi, li, s, _) in targets.iter().rev() {
            remove_run(&mut self.doc.panels[pi].layers[li], s, gap);
        }
        let n = targets.len();
        self.key_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = match gap {
            true => format!("{n} clé{} supprimée{} : leurs images restent vides.", s(n), s(n)),
            false => format!("{n} clé{} supprimée{} : les voisines les remplacent.", s(n), s(n)),
        };
    }

    /// Duplique sur place les clés visées (Ctrl+D après un clic sur une piste) : la copie prend
    /// la seconde moitié de l'exposition de chacune, la suite ne bouge pas.
    pub(crate) fn duplicate_keys(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
        let targets = self.target_keys();
        if targets.is_empty() {
            return;
        }
        if targets.iter().all(|&(_, _, s, e)| e - s < 2) {
            self.status = "Une clé d'une seule image ne se duplique pas sur place : l'allonger d'abord (sa limite avec la suivante).".into();
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let mut done = Vec::new();
        for &(pi, li, s, e) in targets.iter().rev() {
            if e - s < 2 {
                continue;
            }
            let Some(src) = self.key_cel(pi, li, s) else { continue };
            let copy = self.doc.duplicate_cel(src);
            if let Some(mid) = duplicate_run(&mut self.doc.panels[pi].layers[li], s, copy) {
                let (p, l) = (&self.doc.panels[pi], &self.doc.panels[pi].layers[li]);
                done.push(KeyRef { panel: p.id, layer: l.id, start: mid });
            }
        }
        let (n, skipped) = (done.len(), targets.len() - done.len());
        self.key_sel = done;
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!(
            "{n} clé{} dupliquée{} sur place (la copie prend la seconde moitié de l'exposition){}.",
            s(n),
            s(n),
            if skipped > 0 { format!(" ; {skipped} d'une seule image, laissée{} telle{}", s(skipped), s(skipped)) } else { String::new() }
        );
    }
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}
