//! Clés du montage choisies à plusieurs (Anthony, 2026-10-17 : « choix et sélection de
//! plusieurs clés à la fois ») et leurs commandes.
//! - Un clic sur un dessin de la piste d'un calque le choisit seul ; Ctrl + clic l'ajoute ou le
//!   retire ; Maj + clic choisit toutes les clés de la rangée entre la dernière cliquée et
//!   celle-ci, d'un plan à l'autre ; un clic à côté des dessins vide le choix. Ctrl+A : toutes
//!   les clés du calque courant dans le plan courant.
//! - Au rectangle : un glisser qui part d'un espace vide des pistes de calques (ou avec Ctrl ou
//!   Maj, de n'importe où dans les pistes) choisit les clés que le rectangle touche ; avec Ctrl
//!   ou Maj, il les ajoute au choix. Échap rend le choix d'avant (`KeyRect`).
//! - Une clé choisie prise par son milieu emmène les autres clés choisies (`KeysDrag`) : toutes
//!   glissent du même nombre d'images, chacune prend sa place parmi les autres expositions de son
//!   calque (`shift_runs`), toutes gardent leur durée. Comme dans Storyboard Pro, rien ne bouge
//!   pendant le geste : un fantôme suit la souris et un repère montre avant ou après quelle clé
//!   elles se poseront (`drop_view`, aussi pour une clé seule) ; elles se posent au lâcher. Échap
//!   rend tout comme avant ; une étape d'annulation.
//! - Copier, coller, supprimer (la voisine remplace, ou un vide) et dupliquer sur place visent
//!   les clés choisies, sinon la clé à l'image courante du calque courant. Collées, les clés
//!   gardent leurs écarts et leurs durées à partir de l'image courante : venues d'un seul calque,
//!   sur le calque courant ; de plusieurs, chacune sur le calque de même nom du plan courant
//!   (créé s'il manque, à la même place dans la pile).
//! - Clé précédente / suivante (A / F) : la tête de lecture va au début de dessin
//!   d'avant ou d'après, tous les calques compris (`goto_key`).

use crate::app::App;
use crate::document::{Cel, CelId, Layer, LayerKind};
use crate::handles::{drop_marks, duplicate_run, overwrite_run, remove_run, runs, shift_runs, Handle};
use crate::montagerows::{layer_in_row, layer_on_track, track_of, TrackKey};

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
/// tel qu'il était, la sorte de son calque, sa piste (nom du calque) et le rang de son calque
/// en partant du haut de sa pile (pour recréer les calques manquants dans le même ordre).
pub(crate) struct ClipKey {
    pub offset: usize,
    pub len: usize,
    pub cel: Cel,
    pub kind: LayerKind,
    pub track: TrackKey,
    pub rank: usize,
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

/// Un choix au rectangle en cours : son coin de départ (à l'écran), le choix d'avant, et s'il
/// s'y ajoute (Ctrl ou Maj).
pub(crate) struct KeyRect {
    pub origin: eframe::egui::Pos2,
    pub before: Vec<KeyRef>,
    pub add: bool,
}

/// Ce que montre le montage pendant qu'on glisse des clés, pour un calque d'un plan : les
/// fantômes des clés prises (images [début, fin) une fois glissées, dans le plan) et les repères
/// où elles se poseront.
pub(crate) struct DropCell {
    pub panel: u64,
    pub layer: u64,
    pub ghosts: Vec<(isize, isize)>,
    pub marks: Vec<usize>,
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

/// Avant de coller des clés venues de plusieurs calques : les calques de leurs pistes `wanted`
/// (piste, sorte, rang en partant du haut de leur pile d'origine) qui manquent dans `layers`
/// sont créés, du plus bas au plus haut de leur pile d'origine, chacun posé en haut (ils gardent
/// donc leur ordre) ; un deuxième calque du même nom va en bas. Renvoie le nombre de créés.
pub(crate) fn ensure_tracks(layers: &mut Vec<Layer>, wanted: &[(TrackKey, LayerKind, usize)]) -> usize {
    let mut missing: Vec<&(TrackKey, LayerKind, usize)> = Vec::new();
    for w in wanted {
        if layer_on_track(layers, &w.0).is_none() && !missing.iter().any(|m| m.0 == w.0) {
            missing.push(w);
        }
    }
    missing.sort_by_key(|m| std::cmp::Reverse(m.2));
    for (track, kind, _) in &missing {
        let layer = Layer::new(track.name.clone(), *kind);
        if track.nth > 0 {
            layers.insert(0, layer);
        } else {
            layers.push(layer);
        }
    }
    missing.len()
}

/// Les clés d'un plan avant son image `limit` : les images où commence un dessin, sur n'importe
/// lequel de ses calques (sauf dossiers et calques vidéo), dans l'ordre, sans doublon.
pub(crate) fn key_starts(layers: &[Layer], limit: usize) -> Vec<usize> {
    let mut out: Vec<usize> = layers
        .iter()
        .filter(|l| !l.folder && l.video.is_none())
        .flat_map(|l| {
            let fr = &l.frames;
            (0..fr.len().min(limit)).filter(move |&k| fr[k].is_some() && (k == 0 || fr[k - 1] != fr[k]))
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Parmi les clés `keys` (dans l'ordre), la première après `at`, ou la dernière avant.
pub(crate) fn step_key(keys: &[usize], at: usize, forward: bool) -> Option<usize> {
    if forward {
        keys.iter().copied().find(|&k| k > at)
    } else {
        keys.iter().rev().copied().find(|&k| k < at)
    }
}

impl App {
    /// Clé suivante (ou précédente), tous les calques compris (Anthony, 2026-10-17 : « passer
    /// d'une clé à une autre, tous calques compris ») : en storyboard, dans tout le montage, d'un
    /// plan à l'autre ; en animation, dans le plan.
    pub(crate) fn goto_key(&mut self, forward: bool) {
        let found = match self.mode {
            crate::app::Mode::Storyboard => {
                let keys: Vec<usize> = (0..self.doc.panels.len())
                    .flat_map(|i| {
                        let (start, p) = (self.doc.panel_start(i), &self.doc.panels[i]);
                        key_starts(&p.layers, p.frame_count()).into_iter().map(move |k| start + k)
                    })
                    .collect();
                let found = step_key(&keys, self.global_frame(), forward);
                if let Some(g) = found {
                    self.goto_global(g);
                }
                found.is_some()
            }
            crate::app::Mode::Animation => {
                let found = step_key(&key_starts(self.layers(), usize::MAX), self.cur_frame, forward);
                if let Some(f) = found {
                    self.set_frame_free(f);
                    self.follow_frame = true;
                }
                found.is_some()
            }
        };
        if !found {
            self.status = if forward { "Pas de clé après la tête de lecture." } else { "Pas de clé avant la tête de lecture." }.into();
        }
    }

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


    /// Un clic sur la rangée `row` du montage, sur la clé `key` (rien : à côté des dessins), avec
    /// Ctrl (`toggle`) ou Maj (`range`).
    pub(crate) fn click_key(&mut self, key: Option<KeyRef>, row: Option<usize>, toggle: bool, range: bool) {
        let between = match (range, key, self.key_anchor, row) {
            (true, Some(k), Some(a), Some(row)) => Some(self.keys_between(a, k, row)),
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

    /// Les clés prises suivent la souris, à l'image `at` du montage : rien ne bouge encore, le
    /// montage montre fantômes et repères (`drop_view`).
    pub(crate) fn drag_keys(&mut self, at: usize) {
        let Some(d) = self.keys_drag.as_mut() else { return };
        let delta = at as isize - d.grab as isize;
        if delta == d.delta {
            return;
        }
        d.delta = delta;
        let (n, marks) = (d.layers.iter().map(|l| l.starts.len()).sum::<usize>(), d.layers.iter().map(|l| drop_marks(&l.before, &l.starts, delta).len()).sum::<usize>());
        self.status = match marks {
            0 => format!("Lâcher ici : les {n} clés restent à leur place."),
            _ => format!("Lâcher : les {n} clés se posent aux repères, leur durée gardée."),
        };
    }

    /// Lâche les clés : elles se posent (chaque calque repart de son état d'avant le geste) ;
    /// une étape d'annulation si quelque chose a bougé.
    pub(crate) fn end_keys_drag(&mut self) {
        let Some(d) = self.keys_drag.take() else { return };
        let mut sel = Vec::new();
        let mut changed = false;
        for dl in d.layers {
            let mut l = dl.before.clone();
            let new = shift_runs(&mut l, &dl.starts, d.delta);
            changed |= l.frames != dl.before.frames;
            if let Some(pi) = self.doc.panels.iter().position(|p| p.id == dl.panel) {
                if let Some(li) = self.doc.panels[pi].layers.iter().position(|x| x.id == dl.layer) {
                    let p = &mut self.doc.panels[pi];
                    p.layers[li] = l;
                    // La durée du plan reste celle d'avant le geste.
                    if p.frame_count() < dl.plan_len {
                        p.length = dl.plan_len;
                    }
                }
            }
            sel.extend(new.into_iter().map(|start| KeyRef { panel: dl.panel, layer: dl.layer, start }));
        }
        self.key_sel = sel;
        if changed {
            self.history.push(d.snap);
            self.modified = true;
            self.renderer.invalidate_all();
        }
    }

    /// Échap pendant le geste : rien n'a bougé, le choix reste.
    pub(crate) fn cancel_keys_drag(&mut self) {
        self.keys_drag = None;
    }

    /// Fantômes et repères à montrer pendant qu'on glisse des clés (plusieurs, ou une seule).
    pub(crate) fn drop_view(&self) -> Vec<DropCell> {
        if let Some(d) = &self.keys_drag {
            return d
                .layers
                .iter()
                .map(|l| DropCell {
                    panel: l.panel,
                    layer: l.layer,
                    ghosts: l.starts.iter().map(|&s| (s as isize + d.delta, l.before.run_bounds(s).1 as isize + d.delta)).collect(),
                    marks: drop_marks(&l.before, &l.starts, d.delta),
                })
                .collect();
        }
        if let Some(Handle::Key { layer, start, end, .. }) = self.handle_drag.as_ref().map(|h| h.handle) {
            let p = &self.doc.panels[self.cur_panel];
            if let Some(l) = p.layers.get(layer) {
                let delta = self.key_drag_delta;
                return vec![DropCell { panel: p.id, layer: l.id, ghosts: vec![(start as isize + delta, end as isize + delta)], marks: drop_marks(l, &[start], delta) }];
            }
        }
        Vec::new()
    }

    /// Les clés de la rangée `row` entre `a` et `b` (comprises), d'un plan à l'autre.
    fn keys_between(&self, a: KeyRef, b: KeyRef, row: usize) -> Vec<KeyRef> {
        let global = |k: KeyRef| self.locate(k).map(|(pi, _, s, _)| self.doc.panel_start(pi) + s);
        let (Some(ga), Some(gb)) = (global(a), global(b)) else { return vec![b] };
        let (lo, hi) = (ga.min(gb), ga.max(gb));
        let mut out = Vec::new();
        let mut start = 0;
        for p in &self.doc.panels {
            if let Some(li) = layer_in_row(&p.layers, row) {
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

    /// Les clés des rangées `rows` qui touchent les images `g0` à `g1` (comprises) du montage.
    pub(crate) fn keys_in_rect(&self, rows: std::ops::RangeInclusive<usize>, g0: usize, g1: usize) -> Vec<KeyRef> {
        let mut out = Vec::new();
        let mut start = 0;
        for p in &self.doc.panels {
            for row in rows.clone() {
                if let Some(li) = layer_in_row(&p.layers, row) {
                    let l = &p.layers[li];
                    for (s, e, cel) in runs(l) {
                        if cel.is_some() && start + s <= g1 && start + e > g0 {
                            out.push(KeyRef { panel: p.id, layer: l.id, start: s });
                        }
                    }
                }
            }
            start += p.frame_count();
        }
        out
    }

    /// Le choix au rectangle suit la souris : les clés des rangées `rows`, images `g0` à `g1`.
    pub(crate) fn drag_key_rect(&mut self, rows: std::ops::RangeInclusive<usize>, g0: usize, g1: usize) {
        let Some((before, add)) = self.key_rect.as_ref().map(|r| (r.before.clone(), r.add)) else { return };
        let mut sel = if add { before } else { Vec::new() };
        for k in self.keys_in_rect(rows, g0, g1) {
            if !sel.contains(&k) {
                sel.push(k);
            }
        }
        self.key_sel = sel;
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
            let layers = &self.doc.panels[pi].layers;
            let (kind, track, rank) = (layers[li].kind, track_of(layers, li), layers.len() - 1 - li);
            keys.push(ClipKey { offset: self.doc.panel_start(pi) + s - g0, len: e - s, cel, kind, track, rank });
        }
        keys.sort_by_key(|k| k.offset);
        let n = keys.len();
        self.key_clipboard = Some(KeyClip { keys });
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau Pelure (`pasteimage`).
        self.ctx.copy_text(format!("{}clés copiées", crate::pasteimage::MARK));
        self.status = format!("{n} clé{} copiée{} : Ctrl+V pour coller à l'image courante d'un calque.", s(n), s(n));
    }

    /// Colle les clés copiées à partir de l'image courante, chacune avec son écart et sa durée
    /// (sans dépasser la fin du plan), à la place de ce qui y était ; de nouveaux dessins, copies
    /// des siens. Venues d'un seul calque : sur le calque courant. De plusieurs : chacune sur le
    /// calque de même nom du plan courant, créé s'il manque. Les clés collées deviennent le choix.
    pub(crate) fn paste_keys(&mut self) {
        if self.key_clipboard.is_none() || self.refuse_on_folder() {
            return;
        }
        let plan = self.frame_count();
        let f0 = self.cur_frame.min(plan.saturating_sub(1));
        let (pi, cur) = (self.cur_panel, self.cur_layer);
        // (image, durée, dessin, sorte, piste, rang) des clés qui tiennent dans le plan.
        let wanted: Vec<(usize, usize, Cel, LayerKind, TrackKey, usize)> = self
            .key_clipboard
            .as_ref()
            .map(|c| {
                c.keys
                    .iter()
                    .filter(|k| f0 + k.offset < plan)
                    .map(|k| (f0 + k.offset, k.len.min(plan - f0 - k.offset).max(1), k.cel.clone(), k.kind, k.track.clone(), k.rank))
                    .collect()
            })
            .unwrap_or_default();
        let total = self.key_clipboard.as_ref().map_or(0, |c| c.keys.len());
        let mut tracks: Vec<TrackKey> = Vec::new();
        for w in &wanted {
            if !tracks.contains(&w.4) {
                tracks.push(w.4.clone());
            }
        }
        let several = tracks.len() > 1;
        if wanted.is_empty() {
            self.status = "Rien à coller ici : l'image courante est trop près de la fin du plan.".into();
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        // Plusieurs calques : ceux qui manquent sont créés (`ensure_tracks`).
        let created = if several {
            let specs: Vec<(TrackKey, LayerKind, usize)> = wanted.iter().map(|w| (w.4.clone(), w.3, w.5)).collect();
            ensure_tracks(&mut self.doc.panels[pi].layers, &specs)
        } else {
            0
        };
        let panel = self.doc.panels[pi].id;
        let mut pasted = Vec::new();
        let mut layers_used: Vec<u64> = Vec::new();
        let cur_kind = self.doc.panels[pi].layers.get(cur).map(|l| l.kind);
        let cur_id = self.doc.panels[pi].layers.get(cur).map(|l| l.id);
        for (f, n, cel, kind, track, _) in wanted {
            let li = if several {
                layer_on_track(&self.doc.panels[pi].layers, &track)
            } else {
                cur_id.and_then(|id| self.doc.panels[pi].layers.iter().position(|l| l.id == id))
            };
            let Some(li) = li else { continue };
            let layer_kind = if several { self.doc.panels[pi].layers[li].kind } else { cur_kind.unwrap_or(kind) };
            if layer_kind != kind || self.doc.panels[pi].layers[li].folder {
                continue;
            }
            let id = self.doc.add_cel(cel);
            let l = &mut self.doc.panels[pi].layers[li];
            overwrite_run(l, f, n, id);
            if !layers_used.contains(&l.id) {
                layers_used.push(l.id);
            }
            pasted.push(KeyRef { panel, layer: l.id, start: f });
        }
        // Le calque courant reste le même (des calques ont pu être créés dessous).
        if let Some(i) = cur_id.and_then(|id| self.doc.panels[pi].layers.iter().position(|l| l.id == id)) {
            self.cur_layer = i;
        }
        let (n, m) = (pasted.len(), layers_used.len());
        if n == 0 && created == 0 {
            self.status = "Rien collé : le calque n'est pas de la même sorte que les clés (vectoriel ou bitmap).".into();
            return;
        }
        self.key_sel = pasted;
        self.vsel = None;
        self.commit_structure(snap);
        let skipped = total - n;
        let mut msg = format!("{n} clé{} collée{} sur {m} calque{}", s(n), s(n), s(m));
        if created > 0 {
            msg += &format!(" ({created} créé{})", s(created));
        }
        if skipped > 0 {
            msg += &format!(" ; {skipped} non (calque d'une autre sorte, ou après la fin du plan)");
        }
        self.status = msg + ".";
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
