//! Clés du montage choisies à plusieurs (Anthony, 2026-10-17 : « choix et sélection de
//! plusieurs clés à la fois ») et leurs commandes.
//! - Un clic sur un dessin de la piste d'un calque le choisit seul ; Ctrl + clic l'ajoute ou le
//!   retire, sur n'importe quel calque ; Maj + clic choisit le rectangle entre la clé de départ et
//!   celle-ci (les rangées d'entre comprises, d'un plan à l'autre, `keys_between`) ; un clic à côté
//!   des dessins vide le choix. Ctrl+A : toutes les clés du calque courant dans le plan courant.
//! - Au rectangle : un glisser qui part d'un espace vide des pistes de calques (ou avec Ctrl ou
//!   Maj, de n'importe où dans les pistes) choisit les clés que le rectangle touche, sur TOUTES les
//!   rangées qu'il couvre ; Maj les ajoute au choix, Ctrl bascule celles qu'il touche (comme les
//!   clés de caméra). Échap rend le choix d'avant (`KeyRect`). La barre d'état dit « 12 clés sur
//!   3 calques ».
//! - Une clé choisie prise par son milieu (ou la limite de deux expositions) emmène les autres clés
//!   choisies (`KeysDrag`). Mêmes règles que les clés de caméra et d'effets (Anthony, 2026-10-19) :
//!   par défaut la clé prise (la première des choisies de chaque calque) et TOUTES les clés d'après
//!   sur son calque glissent du même nombre d'images, chacune gardant sa durée : l'exposition
//!   d'avant s'allonge ou raccourcit (`KeyMove::Ripple`, « Décaler les clés »). Maj tenue : seules
//!   les clés choisies bougent, la suite reste en place (`KeyMove::Single`, « Déplacer une clé »).
//!   Alt tenue : l'ancien geste, les clés prennent leur place parmi les autres expositions
//!   (`KeyMove::Reorder`, `shift_runs`, « Réordonner les clés »). La durée du plan ne change
//!   jamais : un calque tenu jusqu'à la fin du plan le reste (sa dernière exposition absorbe le
//!   décalage), sinon le contenu s'arrête à la fin du plan. Comme dans Storyboard Pro, rien ne
//!   bouge pendant le geste : des fantômes suivent la souris et un repère montre où se pose la clé
//!   (`drop_view`, aussi pour une clé seule) ; elles se posent au lâcher. Échap rend tout comme
//!   avant ; une étape d'annulation. Refusé sous le verrou du storyboard.
//! - Copier, coller, supprimer (la voisine remplace, ou un vide) et dupliquer sur place visent
//!   les clés choisies, sinon la clé à l'image courante du calque courant. Ctrl+V INSÈRE les clés
//!   copiées après la clé sur laquelle est la tête de lecture du calque courant (sinon la dernière
//!   d'avant elle) : le bloc garde les écarts et les durées, et le contenu d'après est repoussé de
//!   la longueur du bloc (« Insérer des clés ») ; le plan ne s'allonge pas (la dernière exposition
//!   absorbe) et rien n'est collé si une clé entière sortirait du plan. Maj+Ctrl+V colle « à la
//!   tête de lecture » (`edition.coller_a_la_tete`) : le bloc commence exactement sur la tête, par-
//!   dessus ce qui y était, sans rien repousser (« Coller les clés à la tête de lecture »). Venues
//!   d'un seul calque, les clés vont sur le calque courant ; de plusieurs, chacune garde son calque
//!   RELATIVEMENT au plus haut calque copié (Anthony, 2026-10-19) : celui-ci va sur le calque
//!   courant, les autres sur les calques en dessous, dans le même ordre et avec les mêmes écarts
//!   (`target_rank`) ; ceux qui manquent sous la pile sont créés (`ensure_layers_below`). Avant,
//!   la règle cherchait un calque de même nom.
//! - Clé précédente / suivante (A / F) : la tête de lecture va au début de dessin
//!   d'avant ou d'après, tous les calques compris (`goto_key`).

use crate::app::App;
use crate::document::{Cel, CelId, Layer, LayerKind};
use crate::handles::{apply_key_move, clamp_key_delta, duplicate_run, key_preview, overwrite_run, remove_run, runs, Handle, KeyMove};
use crate::montagerows::layer_in_row;

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
/// tel qu'il était, la sorte et le nom de son calque, et le rang de son calque en partant du haut
/// de sa pile (pour le poser sur le même calque relatif, `target_rank`).
pub(crate) struct ClipKey {
    pub offset: usize,
    pub len: usize,
    pub cel: Cel,
    pub kind: LayerKind,
    pub name: String,
    pub rank: usize,
}

/// Plusieurs clés prises à la fois dans le montage : chaque calque touché tel qu'il était, l'image
/// du montage où on les a prises, le décalage en cours, l'étape d'annulation de tout le geste.
pub(crate) struct KeysDrag {
    layers: Vec<DragLayer>,
    grab: usize,
    delta: isize,
    /// Le geste en cours (touches tenues à la dernière image d'écran).
    mode: KeyMove,
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

/// Un choix au rectangle en cours : son coin de départ (à l'écran), le choix d'avant, et la touche
/// tenue au départ : Maj ajoute au choix, Ctrl bascule les clés touchées (comme pour les clés de
/// caméra, `clescommun::rect_choice`).
pub(crate) struct KeyRect {
    pub origin: eframe::egui::Pos2,
    pub before: Vec<KeyRef>,
    pub add: bool,
    pub toggle: bool,
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

/// Le nom de l'étape d'annulation d'un glissement de clés de dessins.
pub(crate) fn key_move_name(mode: KeyMove, several: bool) -> &'static str {
    match (mode, several) {
        (KeyMove::Ripple, _) => "Décaler les clés",
        (KeyMove::Single, false) => "Déplacer une clé",
        (KeyMove::Single, true) => "Déplacer des clés",
        (KeyMove::Reorder, _) => "Réordonner les clés",
    }
}

/// Les clés `chosen` après un clic sur `key` (rien : à côté des dessins). `toggle` (Ctrl) :
/// l'ajoute ou la retire ; `range` (Maj) : les clés `between` (de la dernière cliquée à
/// celle-ci) ; sinon elle seule. Renvoie aussi la nouvelle clé de départ des Maj + clic.
pub(crate) fn choose(chosen: &[KeyRef], anchor: Option<KeyRef>, key: Option<KeyRef>, toggle: bool, range: Option<Vec<KeyRef>>) -> (Vec<KeyRef>, Option<KeyRef>) {
    let Some(k) = key else {
        // À côté des dessins : sans touche, le choix se vide.
        return if toggle || range.is_some() { (chosen.to_vec(), anchor) } else { (Vec::new(), anchor) };
    };
    // La règle est la même pour les trois sortes de clés (`clescommun.rs`).
    crate::clescommun::choose_group(chosen, anchor, &[k], toggle, range)
}

/// Le rang (en partant du haut de la pile du plan) du calque où se pose une clé copiée du rang
/// `rank` : le plus haut calque copié (`min_rank`) va sur le calque courant (`cur_rank`), les
/// autres en dessous, dans le même ordre et avec les mêmes écarts (Anthony, 2026-10-19 : « chaque
/// clé garde son calque relativement au calque le plus haut copié »).
pub(crate) fn target_rank(cur_rank: usize, min_rank: usize, rank: usize) -> usize {
    cur_rank + rank.saturating_sub(min_rank)
}

/// Crée, au bas de la pile `layers` (du bas vers le haut), les calques qui manquent pour que tous
/// les rangs de `specs` (rang depuis le haut, nom du calque d'origine, sorte) existent. Un calque
/// créé reprend le nom et la sorte de la clé qui le vise. Renvoie le nombre de créés.
pub(crate) fn ensure_layers_below(layers: &mut Vec<Layer>, specs: &[(usize, String, LayerKind)]) -> usize {
    let need = specs.iter().map(|s| s.0 + 1).max().unwrap_or(0);
    let mut made = 0;
    while layers.len() < need {
        // Le rang du calque à créer est le nombre de calques qu'il y a déjà (0 : le plus haut).
        let rank = layers.len();
        let (name, kind) = specs
            .iter()
            .find(|s| s.0 == rank)
            .or_else(|| specs.first())
            .map_or(("Calque".to_string(), LayerKind::Vector), |s| (s.1.clone(), s.2));
        layers.insert(0, Layer::new(name, kind));
        made += 1;
    }
    made
}

/// Combien de clés et de calques une liste de clés couvre (« 12 clés sur 3 calques »).
pub(crate) fn key_counts(keys: &[KeyRef]) -> (usize, usize) {
    let mut layers: Vec<(u64, u64)> = keys.iter().map(|k| (k.panel, k.layer)).collect();
    layers.sort_unstable();
    layers.dedup();
    (keys.len(), layers.len())
}

/// « 12 clés sur 3 calques » (ou « 1 clé sur 1 calque »).
pub(crate) fn key_counts_text(keys: &[KeyRef]) -> String {
    let (n, m) = key_counts(keys);
    format!("{n} clé{} sur {m} calque{}", s(n), s(m))
}

/// Ce qui est choisi (donc éclairé) juste après qu'une clé ou une image a été ajoutée (Anthony,
/// 2026-10-18 : « la nouvelle clé doit être sélectionnée ») : elle seule, ni l'ancien choix ni
/// la clé qu'elle vient de couper. Dans le montage (storyboard) c'est la clé choisie de ses pistes ;
/// dans la timeline (animation) ce sont ses images, cadrées de blanc sur son calque.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NewKeyChoice {
    pub keys: Vec<KeyRef>,
    pub frames: Option<(usize, usize)>,
    /// Qui a la main pour copier, coller et supprimer : les clés (montage) ou les images (timeline).
    pub key_focus: bool,
    pub images_focus: bool,
}

/// Le choix après l'ajout d'une clé (ou d'images) du plan `panel`, calque `layer`, images
/// `start` à `end` (exclue) ; `animation` : la timeline est montrée.
pub(crate) fn choice_for_new_key(animation: bool, panel: u64, layer: u64, start: usize, end: usize) -> NewKeyChoice {
    NewKeyChoice {
        keys: vec![KeyRef { panel, layer, start }],
        frames: animation.then_some((start, end.max(start + 1))),
        key_focus: !animation,
        images_focus: animation,
    }
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

/// Où s'insèrent des clés collées sur `layer` (Ctrl+V) : la fin de l'exposition sur laquelle est
/// la tête de lecture (`playhead`) ; sans dessin sous elle, la fin du dernier dessin d'avant ; sans
/// dessin avant, la tête elle-même. Toujours une limite entre deux expositions.
pub(crate) fn insert_point(layer: &Layer, playhead: usize) -> usize {
    if layer.frames.get(playhead).copied().flatten().is_some() {
        return layer.run_bounds(playhead).1;
    }
    let upto = playhead.min(layer.frames.len());
    match layer.frames[..upto].iter().rposition(Option::is_some) {
        Some(last) => layer.run_bounds(last).1,
        None => playhead,
    }
}

/// De combien d'images il manque de place pour ouvrir `n` images à l'image `at` de `layer` sans
/// qu'une clé d'après sorte du plan (`plan_len`) : la dernière exposition peut perdre sa queue,
/// mais son début doit rester dans le plan. 0 : ça tient.
pub(crate) fn gap_short(layer: &Layer, at: usize, n: usize, plan_len: usize) -> usize {
    let len = layer.frames.len();
    if at >= len {
        return 0;
    }
    let last_start = layer.run_bounds(len - 1).0;
    if last_start < at {
        return 0;
    }
    (last_start + n).saturating_sub(plan_len.saturating_sub(1))
}

/// Ouvre `n` images vides à l'image `at` : tout ce qui suit (dessins, rôles, clés d'effets) est
/// repoussé de `n`. Le plan ne s'allonge pas : ce qui dépasse sa fin est coupé (la queue de la
/// dernière exposition ; `gap_short` a vérifié qu'aucun début de clé n'est perdu).
pub(crate) fn open_gap(layer: &mut Layer, at: usize, n: usize, plan_len: usize) {
    if at >= layer.frames.len() || n == 0 {
        return;
    }
    layer.insert(at, None, n);
    layer.frames.truncate(plan_len);
    layer.roles.retain(|f, _| *f < plan_len);
    layer.trim();
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

    /// La clé (ou les images) qu'on vient d'ajouter sur le calque `li` du plan `pi`, images `start`
    /// à `end` (exclue), devient le choix et reçoit la main (`choice_for_new_key`). Toutes les
    /// commandes qui ajoutent une clé ou une image y passent : nouvelle clé (F4 / F5, double-clic,
    /// menu), nouveau dessin, dupliquer, insérer une image.
    pub(crate) fn select_new_key(&mut self, pi: usize, li: usize, start: usize, end: usize) {
        let Some((panel, layer)) = self.doc.panels.get(pi).and_then(|p| p.layers.get(li).map(|l| (p.id, l.id))) else { return };
        let c = choice_for_new_key(self.mode == crate::app::Mode::Animation, panel, layer, start, end);
        self.key_anchor = c.keys.first().copied();
        self.key_sel = c.keys;
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.clear_cam_keys();
        self.layer_sel.clear();
        self.frame_sel = c.frames;
        self.key_focus = c.key_focus;
        self.images_focus = c.images_focus;
        self.panel_focus = false;
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


    /// Un clic sur la clé `key` (rien : à côté des dessins) d'une piste du montage, avec Ctrl
    /// (`toggle` : sur n'importe quel calque) ou Maj (`range` : le rectangle entre la clé de départ
    /// et celle-ci, rangées d'entre comprises).
    pub(crate) fn click_key(&mut self, key: Option<KeyRef>, toggle: bool, range: bool) {
        let between = match (range, key, self.key_anchor) {
            (true, Some(k), Some(a)) => Some(self.keys_between(a, k)),
            _ => None,
        };
        let (sel, anchor) = choose(&self.key_sel, self.key_anchor, key, toggle, between);
        self.key_sel = sel;
        self.key_anchor = anchor;
        if self.key_sel.len() > 1 {
            self.status = format!(
                "{} choisies : glisser pour décaler (Maj : seules celles-ci), Ctrl+C, Ctrl+V (insérer), Maj+Ctrl+V (sur la tête), Suppr, Ctrl+D (dupliquer) ; clic droit pour le reste.",
                key_counts_text(&self.key_sel)
            );
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
        self.keys_drag = Some(KeysDrag { layers, grab, delta: 0, mode: self.key_move_now(), snap });
        true
    }

    /// Le geste que les touches tenues demandent : Alt, l'ancien (réordonner) ; Maj, la clé seule ;
    /// sinon les clés d'après suivent.
    pub(crate) fn key_move_now(&self) -> KeyMove {
        self.ctx.input(|i| {
            if i.modifiers.alt {
                KeyMove::Reorder
            } else if i.modifiers.shift {
                KeyMove::Single
            } else {
                KeyMove::Ripple
            }
        })
    }

    /// Les clés prises suivent la souris, à l'image `at` du montage : rien ne bouge encore, le
    /// montage montre fantômes et repères (`drop_view`). Les touches Maj et Alt se lisent à
    /// chaque image d'écran : on peut les presser ou les lâcher en cours de geste.
    pub(crate) fn drag_keys(&mut self, at: usize) {
        let mode = self.key_move_now();
        self.drag_keys_mode(at, mode);
    }

    pub(crate) fn drag_keys_mode(&mut self, at: usize, mode: KeyMove) {
        let Some(d) = self.keys_drag.as_mut() else { return };
        let want = at as isize - d.grab as isize;
        // Un seul décalage pour tous les calques : le plus serré décide.
        let delta = d.layers.iter().fold(want, |acc, l| clamp_key_delta(&l.before, &l.starts, acc, mode, l.plan_len));
        if delta == d.delta && mode == d.mode {
            return;
        }
        d.delta = delta;
        d.mode = mode;
        let n = d.layers.iter().map(|l| l.starts.len()).sum::<usize>();
        let marks = d.layers.iter().map(|l| key_preview(&l.before, &l.starts, delta, mode, l.plan_len).1.len()).sum::<usize>();
        let who = if n > 1 { format!("les {n} clés") } else { "la clé".to_string() };
        let img = format!("{delta:+} image{}", if delta.abs() > 1 { "s" } else { "" });
        self.status = match (mode, marks) {
            (_, 0) => "Lâcher ici : rien ne bouge.".to_string(),
            (KeyMove::Ripple, _) => format!("Lâcher : {img} pour {who} et tout ce qui suit sur sa piste (Maj : seule, Alt : réordonner)."),
            (KeyMove::Single, _) => format!("Lâcher : {img} pour {who} seule{} (Maj tenue), la suite reste en place.", if n > 1 { "s" } else { "" }),
            (KeyMove::Reorder, _) => format!("Lâcher : {who} se pose aux repères, durée gardée (Alt tenue)."),
        };
    }

    /// Lâche les clés : elles se posent (chaque calque repart de son état d'avant le geste) ;
    /// une étape d'annulation si quelque chose a bougé.
    pub(crate) fn end_keys_drag(&mut self) {
        let Some(d) = self.keys_drag.take() else { return };
        let mut sel = Vec::new();
        let mut changed = false;
        let several = d.layers.iter().map(|l| l.starts.len()).sum::<usize>() > 1;
        for dl in d.layers {
            let mut l = dl.before.clone();
            let new = apply_key_move(&mut l, &dl.starts, d.delta, d.mode, dl.plan_len);
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
            self.commit_as(d.snap, key_move_name(d.mode, several));
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
                .map(|l| {
                    let (ghosts, marks) = key_preview(&l.before, &l.starts, d.delta, d.mode, l.plan_len);
                    DropCell { panel: l.panel, layer: l.layer, ghosts, marks }
                })
                .collect();
        }
        if let Some(Handle::Key { layer, start, .. }) = self.handle_drag.as_ref().map(|h| h.handle) {
            let p = &self.doc.panels[self.cur_panel];
            if let Some(l) = p.layers.get(layer) {
                let (ghosts, marks) = key_preview(l, &[start], self.key_drag_delta, self.key_drag_mode, p.frame_count());
                return vec![DropCell { panel: p.id, layer: l.id, ghosts, marks }];
            }
        }
        Vec::new()
    }

    /// La rangée (0 : en haut) d'une clé dans la pile de son plan.
    fn key_row(&self, k: KeyRef) -> Option<usize> {
        let (pi, li, _, _) = self.locate(k)?;
        Some(self.doc.panels[pi].layers.len() - 1 - li)
    }

    /// Maj + clic : le rectangle entre la clé de départ `a` et la clé cliquée `b` : toutes les
    /// rangées d'entre (les leurs comprises), et sur chacune les clés dont le début est entre les
    /// deux images, d'un plan à l'autre. Sur une seule rangée, c'est la plage d'une piste.
    pub(crate) fn keys_between(&self, a: KeyRef, b: KeyRef) -> Vec<KeyRef> {
        let global = |k: KeyRef| self.locate(k).map(|(pi, _, s, _)| self.doc.panel_start(pi) + s);
        let (Some(ga), Some(gb), Some(ra), Some(rb)) = (global(a), global(b), self.key_row(a), self.key_row(b)) else { return vec![b] };
        let (lo, hi) = (ga.min(gb), ga.max(gb));
        let rows = ra.min(rb)..=ra.max(rb);
        let mut out = Vec::new();
        let mut start = 0;
        for p in &self.doc.panels {
            for row in rows.clone() {
                if let Some(li) = layer_in_row(&p.layers, row) {
                    let l = &p.layers[li];
                    for (s, _, cel) in runs(l) {
                        if cel.is_some() && (lo..=hi).contains(&(start + s)) {
                            out.push(KeyRef { panel: p.id, layer: l.id, start: s });
                        }
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
        let Some((before, add, toggle)) = self.key_rect.as_ref().map(|r| (r.before.clone(), r.add, r.toggle)) else { return };
        let picked = self.keys_in_rect(rows, g0, g1);
        self.key_sel = crate::clescommun::rect_choice(&before, &picked, add, toggle);
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
            let (kind, name, rank) = (layers[li].kind, layers[li].name.clone(), layers.len() - 1 - li);
            keys.push(ClipKey { offset: self.doc.panel_start(pi) + s - g0, len: e - s, cel, kind, name, rank });
        }
        keys.sort_by_key(|k| k.offset);
        let n = keys.len();
        self.key_clipboard = Some(KeyClip { keys });
        self.fx_clip_recent = false;
        self.cam_clip_recent = false;
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau Pelure (`pasteimage`).
        self.ctx.copy_text(format!("{}clés copiées", crate::pasteimage::MARK));
        self.status = format!("{n} clé{} copiée{} : Ctrl+V pour les insérer après la clé de la tête de lecture, Maj+Ctrl+V pour les coller sur la tête.", s(n), s(n));
    }

    /// Ctrl+V : insère les clés copiées après la clé sur laquelle est la tête de lecture, sur le
    /// calque courant (`insert_point`), chacune avec son écart et sa durée ; tout ce qui suit sur
    /// les calques touchés est repoussé de la longueur du bloc (`open_gap`).
    pub(crate) fn paste_keys(&mut self) {
        self.paste_keys_with(true);
    }

    /// Maj+Ctrl+V (`edition.coller_a_la_tete`) : colle les clés copiées exactement à partir de la
    /// tête de lecture, à la place de ce qui y était, sans rien repousser.
    pub(crate) fn paste_keys_at_head(&mut self) {
        self.paste_keys_with(false);
    }

    /// Colle les clés copiées, chacune avec son écart et sa durée (sans dépasser la fin du plan) ;
    /// de nouveaux dessins, copies des siens. Venues d'un seul calque : sur le calque courant. De
    /// plusieurs : chacune relativement au plus haut calque copié (`target_rank`), créé s'il manque. Les clés
    /// collées deviennent le choix. `insert` : après la clé de la tête de lecture, en repoussant la
    /// suite ; sinon à la tête de lecture, à la place de ce qui y était.
    fn paste_keys_with(&mut self, insert: bool) {
        if self.key_clipboard.is_none() || self.refuse_on_folder() {
            return;
        }
        let plan = self.frame_count();
        let f0 = if insert {
            self.layers().get(self.cur_layer).map_or(self.cur_frame, |l| insert_point(l, self.cur_frame))
        } else {
            self.cur_frame.min(plan.saturating_sub(1))
        };
        let (pi, cur) = (self.cur_panel, self.cur_layer);
        // Chaque clé garde son calque RELATIVEMENT au plus haut calque copié : celui-ci va sur le
        // calque courant, les autres sur les calques en dessous, dans le même ordre et avec les
        // mêmes écarts (`target_rank`). Des clés d'un seul calque vont sur le calque courant.
        let stack = self.doc.panels[pi].layers.len();
        let cur_rank = stack.saturating_sub(1 + cur);
        let min_rank = self.key_clipboard.as_ref().and_then(|c| c.keys.iter().map(|k| k.rank).min()).unwrap_or(0);
        // (image, durée, dessin, sorte, nom du calque d'origine, rang visé) des clés qui tiennent dans le plan.
        let wanted: Vec<(usize, usize, Cel, LayerKind, String, usize)> = self
            .key_clipboard
            .as_ref()
            .map(|c| {
                c.keys
                    .iter()
                    .filter(|k| f0 + k.offset < plan)
                    .map(|k| (f0 + k.offset, k.len.min(plan - f0 - k.offset).max(1), k.cel.clone(), k.kind, k.name.clone(), target_rank(cur_rank, min_rank, k.rank)))
                    .collect()
            })
            .unwrap_or_default();
        let total = self.key_clipboard.as_ref().map_or(0, |c| c.keys.len());
        if wanted.is_empty() {
            self.status = "Rien à coller ici : l'image courante est trop près de la fin du plan.".into();
            return;
        }
        // Longueur du bloc (de sa première image à la fin de sa dernière clé) et place pour le
        // repousser : le plan ne s'allonge pas, aucune clé d'après ne doit en sortir.
        let block = wanted.iter().map(|w| w.0 + w.1 - f0).max().unwrap_or(0);
        if insert {
            let layers = &self.doc.panels[pi].layers;
            let mut ranks: Vec<usize> = wanted.iter().map(|w| w.5).collect();
            ranks.sort_unstable();
            ranks.dedup();
            let short = ranks.iter().filter_map(|&r| layers.get(stack.checked_sub(1 + r)?)).map(|l| gap_short(l, f0, block, plan)).max().unwrap_or(0);
            if short > 0 {
                self.status = format!(
                    "Rien collé : les clés d'après sortiraient du plan ({short} image{} de trop). Maj+Ctrl+V colle sur la tête de lecture sans rien repousser.",
                    s(short)
                );
                return;
            }
        }
        self.end_stroke();
        let snap = self.snapshot();
        // Les calques qui manquent sous la pile sont créés (`ensure_layers_below`).
        let specs: Vec<(usize, String, LayerKind)> = wanted.iter().map(|w| (w.5, w.4.clone(), w.3)).collect();
        let cur_id = self.doc.panels[pi].layers.get(cur).map(|l| l.id);
        let created = ensure_layers_below(&mut self.doc.panels[pi].layers, &specs);
        let stack = self.doc.panels[pi].layers.len();
        let panel = self.doc.panels[pi].id;
        let mut pasted = Vec::new();
        let mut layers_used: Vec<u64> = Vec::new();
        let mut opened: Vec<u64> = Vec::new();
        for (f, n, cel, kind, _, rank) in wanted {
            let Some(li) = stack.checked_sub(1 + rank) else { continue };
            if self.doc.panels[pi].layers[li].kind != kind || self.doc.panels[pi].layers[li].folder {
                continue;
            }
            let id = self.doc.add_cel(cel);
            let l = &mut self.doc.panels[pi].layers[li];
            // Insertion : la place du bloc s'ouvre une fois par calque, avant sa première clé.
            if insert && !opened.contains(&l.id) {
                open_gap(l, f0, block, plan);
                opened.push(l.id);
            }
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
        // Le plan garde sa durée : la queue de la dernière exposition a pu être coupée.
        if self.doc.panels[pi].frame_count() < plan {
            self.doc.panels[pi].length = plan;
        }
        self.commit_as(snap, if insert { "Insérer des clés" } else { "Coller les clés à la tête de lecture" });
        let skipped = total - n;
        let mut msg = format!("{n} clé{} {} sur {m} calque{}", s(n), if insert { format!("insérée{}", s(n)) } else { format!("collée{}", s(n)) }, s(m));
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
            crate::calquecouleur::noter_copie(&mut self.doc, pi, li, src, copy);
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
