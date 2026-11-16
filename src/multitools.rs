//! La sélection sur plusieurs calques dans l'application (Anthony, 2026-10-19 : « Ajoute la
//! possibilité de sélectionner deux calques pour ensuite sélectionner les deux dessins de chaque
//! calque pour les transformer, les déplacer »). Le cœur — quels traits, quelles zones, comment
//! un geste s'applique à chaque calque — est dans `multisel.rs`, sans interface ; ici : la souris,
//! la boîte à l'écran, les commandes (Suppr, copier, couper, coller, miroir, flèches), l'annulation
//! et les feuilles flottantes qui rendent le geste sans latence.
//!
//! - Plusieurs calques choisis (Ctrl ou Maj + clic dans le panneau Calques, la timeline ou le
//!   montage) et l'outil Sélection : le lasso, le rectangle et « Tout sélectionner » prennent sur
//!   tous les calques choisis (`multi_wanted`).
//! - La boîte (`selbox.rs`) est sur la feuille : `sel_frame` la donne, comme pour un seul calque,
//!   et tout ce qui l'utilise (poignées, curseurs, champs chiffrés) marche sans changement.
//! - Un geste de la boîte = une étape d'annulation (`Edit::Group`) au nom du geste.
//! - L'outil Transformer, lui, déplace, tourne et agrandit les calques choisis ensemble
//!   (`layertools.rs`) ; l'outil Déformer et la Gomme les prennent tous aussi (`deformmulti.rs`, `gommemulti.rs`, partie 132).

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::{App, Tool};
use crate::contours::{self, Contours};
use crate::flottante::Flottante;
use crate::multisel::{self, MDrag, MultiSel, PartKind, Targets};
use crate::selbox::{self, BoxParams, Handle, SelFrame};
use crate::vector::Affine;

const ACCENT: Color32 = crate::theme::SELECTION_BOITE;
/// Opacité (sur 1) du halo des dessins d'autres clés, qu'on ne voit pas : plus pâle que celui du dessin courant.
const GHOST_ALPHA: f32 = 0.4;
/// À moins de tant de pixels d'écran du centre, le pivot tiré s'y remet.
const PIVOT_SNAP: f32 = 6.0;
/// Geste d'annulation des flèches : une flèche tenue = une étape.
const NUDGE_GESTURE: u64 = 0x6d75_6c74_696e_7564;

/// « 42 traits sur 3 calques », « 12 traits et 2 zones sur 6 dessins de 2 calques » : ce que « Tout sélectionner » a pris.
pub(crate) fn compte_tout(sel: &MultiSel) -> String {
    let s = |n: usize| if n > 1 { "s" } else { "" };
    let traits: usize = sel.parts.iter().filter(|p| matches!(p.kind, PartKind::Vector { .. })).map(multisel::Part::count).sum();
    let zones = sel.parts.iter().filter(|p| matches!(p.kind, PartKind::Bitmap { .. })).count();
    let mut calques: Vec<u64> = sel.parts.iter().map(|p| p.layer).collect();
    calques.sort_unstable();
    calques.dedup();
    let quoi = match (traits, zones) {
        (t, 0) => format!("{t} trait{}", s(t)),
        (0, z) => format!("{z} zone{}", s(z)),
        (t, z) => format!("{t} trait{} et {z} zone{}", s(t), s(z)),
    };
    let (n, m) = (sel.parts.len(), calques.len());
    if sel.by_keys {
        format!("{quoi} sur {n} dessin{} de {m} calque{}", s(n), s(m))
    } else {
        format!("{quoi} sur {m} calque{}", s(m))
    }
}

/// Les touches tenues font-elles le geste « tous les calques » (Ctrl+Maj avec
/// l'outil Sélection) ? Ctrl seul garde le lasso temporaire et la boîte autour du pivot, Maj seul
/// ajoute ou retire ; Alt est la pipette.
pub(crate) fn is_all_layers_gesture(m: &egui::Modifiers) -> bool {
    (m.command || m.ctrl) && m.shift && !m.alt
}

impl App {
    // ------------------------------------------------------------------ ce qui est choisi

    /// Le lasso prend-il sur plusieurs calques ? L'outil Sélection, et plusieurs calques choisis.
    pub(crate) fn multi_wanted(&self) -> bool {
        self.tool == Tool::Select && (self.chosen_layers().len() >= 2 || self.multi.sel.is_some() || self.keys_mode() || self.aplat_strate_selection())
    }

    /// L'outil Sélection travaille l'**aplat** du dessin (sa strate, `fond.rs`) : la sous-ligne de l'aplat du calque courant
    /// est la cible (Anthony, 2026-10-28). Le clic, le lasso, le rectangle et « Tout sélectionner » prennent alors les
    /// pixels de l'aplat, que la boîte déplace, met à l'échelle et tourne comme le trait ; le trait n'est pas touché. Ce
    /// chemin est celui de la sélection sur plusieurs calques (`multisel.rs`), qui sait déjà plusieurs clés d'un coup.
    /// (Un calque de couleur est un calque bitmap ordinaire : il n'a besoin de rien de plus.)
    pub(crate) fn aplat_strate_selection(&self) -> bool {
        self.tool == Tool::Select && self.aplat_en_cible()
    }

    /// Les clés choisies du plan courant, calque (id) et image de début : de quoi prendre leurs
    /// dessins ensemble. L'aplat étant visé, seules comptent les clés du calque dont c'est l'aplat : celles de son calque de
    /// trait aussi quand c'est un calque de couleur (elles se lisent sur la couleur qui les suit).
    fn chosen_key_drawings(&self) -> Vec<(u64, usize)> {
        let Some(panel) = self.doc.panels.get(self.cur_panel).map(|p| p.id) else { return Vec::new() };
        let keys = self.key_sel.iter().filter(|k| k.panel == panel).map(|k| (k.layer, k.start));
        let Some(cur) = self.layers().get(self.cur_layer) else { return keys.collect() };
        if self.aplat_strate_selection() {
            return keys.filter(|k| k.0 == cur.id).collect();
        }
        match cur.couleur.as_ref().filter(|c| c.est_lie()) {
            Some(c) if self.tool == Tool::Select => {
                let (own, line) = (cur.id, c.trait_id);
                let mut v: Vec<(u64, usize)> = keys.filter(|k| k.0 == own || k.0 == line).map(|k| (own, k.1)).collect();
                v.sort_unstable();
                v.dedup();
                v
            }
            _ => keys.collect(),
        }
    }

    /// Plusieurs clés sont choisies (dans le montage ou la timeline) : l'outil Sélection agit sur leurs
    /// dessins à tous, sans rien tenir. Un seul dessin, ou le geste Ctrl+Maj « tous les calques » :
    /// non.
    pub(crate) fn keys_mode(&self) -> bool {
        !self.multi.all_layers && self.chosen_key_drawings().len() >= 2
    }

    /// Les clés choisies du plan courant (calque, image de début), tous calques confondus.
    pub(crate) fn cles_du_plan(&self) -> Vec<(u64, usize)> {
        let Some(panel) = self.doc.panels.get(self.cur_panel).map(|p| p.id) else { return Vec::new() };
        self.key_sel.iter().filter(|k| k.panel == panel).map(|k| (k.layer, k.start)).collect()
    }

    /// Les dessins que visent la gomme et l'outil Déformer quand plusieurs dessins sont choisis (partie 132) : ceux des clés
    /// choisies du plan courant (dans la timeline ou le montage : plusieurs calques et plusieurs clés, chaque dessin une seule
    /// fois, même tenu sur plusieurs images), s'il y en a deux au moins ; sinon ceux des calques choisis à l'image courante.
    /// Vrai : par les clés.
    pub(crate) fn dessins_choisis(&self) -> (Targets, bool) {
        let keys = self.cles_du_plan();
        if keys.len() >= 2 {
            let t = multisel::targets_for_keys_in(&self.doc, self.cur_panel, self.cur_frame, &keys, false);
            if t.list.len() >= 2 {
                return (t, true);
            }
        }
        (multisel::targets(&self.doc, self.cur_panel, self.cur_frame, &self.chosen_layers()), false)
    }

    /// Plusieurs dessins sont réellement visés (calques ou clés choisis, une fois écartés les cachés, verrouillés, vidéo, sans
    /// dessin) : deux au moins. Avec un seul, la gomme et Déformer gardent le chemin d'un seul calque (sa sélection, son
    /// enveloppe).
    pub(crate) fn plusieurs_dessins_choisis(&self) -> bool {
        if self.chosen_layers().len() < 2 && self.cles_du_plan().len() < 2 {
            return false;
        }
        self.dessins_choisis().0.list.len() >= 2
    }

    /// Les dessins visés, en clair : « 15 dessins (5 clés sur 3 calques) », ou « 3 calques ».
    pub(crate) fn dessins_texte(t: &Targets, par_cles: bool) -> String {
        let mut calques: Vec<u64> = t.list.iter().map(|x| x.id).collect();
        calques.sort_unstable();
        calques.dedup();
        let (n, m) = (t.list.len(), calques.len());
        let s = |k: usize| if k > 1 { "s" } else { "" };
        if par_cles {
            format!("{n} dessin{} ({n} clé{} sur {m} calque{})", s(n), s(n), s(m))
        } else {
            format!("{n} calque{}", s(n))
        }
    }

    /// Les dessins des clés choisies, à prendre ensemble (`keys_mode`).
    fn key_targets(&self) -> Targets {
        multisel::targets_for_keys_in(&self.doc, self.cur_panel, self.cur_frame, &self.chosen_key_drawings(), self.aplat_strate_selection())
    }

    /// Une sélection neuve sur les cibles de ce moment : sur des clés ou sur des calques.
    fn new_multi_sel(&self, parts: Vec<multisel::Part>) -> MultiSel {
        let mut sel = MultiSel::new(self.cur_panel, self.cur_frame, parts);
        sel.by_keys = self.keys_mode();
        sel
    }

    /// Échap avec plusieurs clés choisies et l'outil Sélection : on revient au dessin courant seul
    /// (les clés sont lâchées, la sélection sur plusieurs dessins s'en va).
    pub(crate) fn keys_escape(&mut self, key: egui::Key) -> bool {
        if key != egui::Key::Escape || self.tool != Tool::Select || !self.keys_mode() || self.multi.drag.is_some() {
            return false;
        }
        self.drop_multi();
        self.key_sel.clear();
        self.key_anchor = None;
        self.status = "Clés lâchées : la sélection agit sur le dessin courant.".into();
        true
    }

    /// Les rangs de tous les calques du plan courant qui ont un dessin (les dossiers n'en ont pas :
    /// leurs calques comptent par eux-mêmes ; `multisel::targets` écarte les cachés, les verrouillés
    /// et les calques vidéo).
    pub(crate) fn all_layer_ranks(&self) -> Vec<usize> {
        (0..self.layers().len()).filter(|&li| !self.layers()[li].folder).collect()
    }

    /// Les calques que la sélection peut toucher à l'image courante, et ceux qu'elle laisse : les
    /// calques choisis, ou tous ceux du plan pendant un geste Ctrl+Maj (`Multi::all_layers`).
    pub(crate) fn multi_targets(&self) -> Targets {
        if self.keys_mode() {
            return self.key_targets();
        }
        // L'aplat est la cible : celui du calque courant seul.
        if self.aplat_strate_selection() {
            return multisel::targets_aplat(&self.doc, self.cur_panel, self.cur_frame, &[self.cur_layer]);
        }
        let ranks = if self.multi.all_layers { self.all_layer_ranks() } else { self.chosen_layers() };
        multisel::targets(&self.doc, self.cur_panel, self.cur_frame, &ranks)
    }

    /// Ctrl+Maj : les calques où la sélection a pris quelque chose deviennent les calques choisis
    /// (le calque courant reste le courant), et leurs clés à l'image courante, des clés choisies de
    /// la timeline et du montage. Le geste « tous les calques » est fini.
    pub(crate) fn multi_adopt_layers(&mut self) {
        let was_all = std::mem::take(&mut self.multi.all_layers);
        if !was_all {
            return;
        }
        let Some(sel) = self.multi.sel.as_ref() else { return };
        let ids: Vec<u64> = sel.parts.iter().map(|p| p.layer).collect();
        let cur_id = self.layers().get(self.cur_layer).map(|l| l.id);
        self.layer_sel = ids.iter().copied().filter(|id| Some(*id) != cur_id).collect();
        let (pi, f) = (self.cur_panel, self.cur_frame);
        let keys: Vec<crate::montagekeys::KeyRef> = ids
            .iter()
            .filter_map(|id| self.layers().iter().position(|l| l.id == *id))
            .filter_map(|li| self.key_ref(pi, li, f))
            .collect();
        self.key_anchor = keys.first().copied();
        self.key_sel = keys;
    }

    /// La boîte telle qu'on la voit (le geste en cours compris).
    pub(crate) fn multi_live_frame(&self) -> Option<SelFrame> {
        let sel = self.multi.sel.as_ref()?;
        if let Some(MDrag::Transform { frame, aff, .. }) = &self.multi.drag {
            return Some(frame.then(aff));
        }
        sel.base_frame(&self.doc)
    }

    /// « 3 calques, 42 éléments » pour la barre d'état et le panneau Réglages.
    pub(crate) fn multi_summary(&self) -> Option<String> {
        self.multi.sel.as_ref().map(MultiSel::summary)
    }

    // ------------------------------------------------------------------ la souris

    /// Un appui avec l'outil Sélection et plusieurs calques choisis : une poignée de la boîte, un
    /// trait, ou le début d'un lasso (ou d'un rectangle).
    /// `all` : Ctrl+Maj tenus : le geste prend sur tous les calques du
    /// plan à l'image courante, comme une nouvelle sélection (Maj seul garde son sens : ajouter ou
    /// retirer ; Ctrl seul, celui de la boîte, autour du pivot).
    pub(crate) fn multi_press(&mut self, stage: Pos2, screen: Pos2, shift: bool, all: bool) {
        self.end_stroke();
        self.playing = false;
        self.finish_field_edit();
        if self.content_refuses() {
            return;
        }
        // Une sélection sur un seul calque laisse la place — sauf avec Ctrl+Maj, qui cumule : elle
        // est reprise dans la sélection sur plusieurs calques (`selobjet.rs`).
        if all {
            self.carry_single_selection();
        }
        if self.vsel.is_some() || self.bsel.is_some() {
            self.vsel = None;
            self.drop_bitmap_selection();
        }
        // Un geste lâché dont la feuille attend encore le rendu exact : on le termine d'abord.
        self.multi_flush();
        // La boîte : une poignée, un bord, l'extérieur d'un coin, le pivot ou l'intérieur.
        // (Ctrl+Maj est un geste de choix : il ne prend jamais une poignée de la boîte, même quand le
        // trait visé est tout contre un de ses bords.)
        if let Some((handle, frame)) = self.box_press(screen, shift).filter(|_| !all) {
            self.multi.drag = Some(if handle == Handle::Pivot {
                // Le pivot déplacé est gardé avec la boîte.
                if let Some(sel) = self.multi.sel.as_mut() {
                    sel.keep_frame(&self.doc, frame);
                }
                MDrag::Pivot { start: stage, frame }
            } else {
                MDrag::Transform { handle, start: stage, frame, snaps: None, aff: Affine::IDENTITY, field: false }
            });
            return;
        }
        // Ctrl+Maj : tous les calques du plan, et la sélection se cumule : un objet s'ajoute, ou se
        // retire s'il était déjà choisi (Anthony, 2026-10-20 ; avant : nouvelle sélection).
        self.multi.all_layers = all;
        let shift = shift || all;
        let targets = self.multi_targets();
        let tol = 4.0 / self.view.zoom.max(1e-3);
        if let Some((t, item)) = multisel::hit_item(&self.doc, &targets, stage, tol) {
            let t = t.clone();
            let (panel, frame) = (self.cur_panel, self.cur_frame);
            if shift {
                let by_keys = self.keys_mode();
                let sel = self.multi.sel.get_or_insert_with(|| {
                    let mut s = MultiSel::new(panel, frame, Vec::new());
                    s.by_keys = by_keys;
                    s
                });
                sel.toggle_item(&t, item);
                if sel.parts.is_empty() {
                    self.multi.sel = None;
                } else {
                    self.status = format!("{} choisis.", sel.summary());
                }
                // Ctrl+Maj : les calques touchés deviennent les calques choisis ; le geste est fini.
                self.multi_adopt_layers();
                self.multi.all_layers = false;
                return;
            }
            let already = self
                .multi
                .sel
                .as_ref()
                .is_some_and(|s| s.parts.iter().any(|p| p.layer == t.id && matches!(&p.kind, PartKind::Vector { items } if items.contains(&item))));
            if !already {
                self.multi.sel = Some(self.new_multi_sel(vec![multisel::vector_part(&t, vec![item])]));
            }
            // Ctrl+Maj + clic sur un trait : son calque devient un calque choisi.
            self.multi_adopt_layers();
            // Un clic sur un trait choisi : on le tient, et il suit la souris.
            if let Some(frame) = self.multi_live_frame() {
                self.multi.drag = Some(MDrag::Transform { handle: Handle::Move, start: stage, frame, snaps: None, aff: Affine::IDENTITY, field: false });
            }
            return;
        }
        // Dans le vide : un lasso (ou un rectangle) ; sans Maj, l'ancienne sélection s'en va.
        if !shift {
            self.drop_multi();
        }
        self.multi.drag = Some(if self.select_rect {
            MDrag::Rect { from: stage, to: stage, add: shift }
        } else {
            MDrag::Lasso { points: vec![stage], add: shift }
        });
    }

    /// La souris est en `stage` (point de la feuille) : le lasso grandit, ou le geste de la boîte
    /// suit (`shift`, `ctrl` : les touches tenues maintenant, `selbox::gesture`).
    pub(crate) fn multi_drag(&mut self, stage: Pos2, shift: bool, ctrl: bool) {
        let zoom = self.view.zoom.max(1e-3);
        let mods = self.box_mods(shift, ctrl);
        match &mut self.multi.drag {
            Some(MDrag::Lasso { points, .. }) => {
                if points.last().is_none_or(|q| q.distance(stage) * zoom > 1.5) {
                    points.push(stage);
                }
            }
            Some(MDrag::Rect { to, .. }) => *to = stage,
            Some(MDrag::Transform { handle, start, frame, field: false, .. }) => {
                let aff = selbox::gesture(*handle, frame, *start, stage, mods);
                self.multi_set_gesture(aff);
            }
            Some(MDrag::Pivot { start, frame }) => {
                let (start, frame) = (*start, *frame);
                let local = selbox::move_pivot(&frame, start, stage, PIVOT_SNAP, zoom);
                if let Some(b) = self.multi.sel.as_mut().and_then(|s| s.boxed.as_mut()) {
                    b.frame.pivot = local;
                }
            }
            _ => {}
        }
    }

    /// La transformation `aff` (sur la feuille) du geste en cours. À la première image : chaque
    /// calque vectoriel reçoit sa feuille flottante (le dessin sans la sélection, et la sélection
    /// seule), chaque zone bitmap soulève ses pixels. Ensuite, seule la transformation change :
    /// rien à refaire, rien à renvoyer à la carte graphique.
    pub(crate) fn multi_set_gesture(&mut self, aff: Affine) {
        let Some(MDrag::Transform { handle, frame, snaps, aff: now, .. }) = &mut self.multi.drag else { return };
        let (handle, frame) = (*handle, *frame);
        *now = aff;
        let Some(sel) = self.multi.sel.as_mut() else { return };
        if snaps.is_none() {
            *snaps = Some(multisel::begin_gesture(&mut self.doc, sel, &mut self.multi.floats));
        }
        let Some(snaps) = snaps.as_ref() else { return };
        let dirty = multisel::set_gesture(&mut self.doc, sel, &mut self.multi.floats, snaps, &aff, handle == Handle::Move);
        if !dirty.is_empty() {
            self.renderer.invalidate_tiles(&dirty);
            self.modified = true;
        }
        let text = crate::seltools::gesture_text(handle, &frame.then(&aff).params());
        self.status = format!("{} ({}).", text.trim_end_matches('.'), sel.summary());
    }

    /// Le bouton est relâché : le lasso prend, ou le geste est posé.
    pub(crate) fn multi_finish_drag(&mut self) {
        let Some(drag) = self.multi.drag.take() else { return };
        match drag {
            MDrag::Lasso { points, add } => {
                if self.big_enough(&points) {
                    self.multi_select_polygon(&points, add);
                } else if points.first().is_some_and(|&p| self.multi_pick_object(p, add)) {
                    // (Un clic sur un dessin bitmap : l'objet, `selobjet.rs`.)
                } else if !add {
                    self.multi.sel = None;
                }
            }
            MDrag::Rect { from, to, add } => {
                let r = Rect::from_two_pos(from, to);
                if r.width().min(r.height()) * self.view.zoom >= 3.0 {
                    self.multi_select_polygon(&crate::pixsel::rect_polygon(r), add);
                } else if self.multi_pick_object(from, add) {
                } else if !add {
                    self.multi.sel = None;
                }
            }
            MDrag::Pivot { .. } => {}
            MDrag::Transform { handle, frame, snaps, aff, .. } => {
                if let Some(snaps) = snaps {
                    self.multi_commit(handle, frame, snaps, aff);
                }
            }
        }
        // Ctrl+Maj ne vaut que pour le geste qui vient de finir.
        self.multi.all_layers = false;
    }

    /// Un lasso d'au moins trois points et quatre pixels d'écran.
    fn big_enough(&self, points: &[Pos2]) -> bool {
        let screen: Vec<Pos2> = points.iter().map(|q| self.stage_to_screen(*q)).collect();
        let extent = Rect::from_points(&screen);
        points.len() >= 3 && extent.width().max(extent.height()) >= 4.0
    }

    /// Ce que prend le polygone `poly` (sur la feuille) sur les calques choisis.
    pub(crate) fn multi_select_polygon(&mut self, poly: &[Pos2], add: bool) {
        let targets = self.multi_targets();
        let parts = multisel::select_polygon(&self.doc, &targets, poly);
        let left = targets.skipped_text();
        if parts.is_empty() {
            if !add {
                self.multi.sel = None;
            }
            let none = if targets.list.is_empty() { "Aucun des calques choisis ne peut être sélectionné." } else { "Rien dans le lasso sur les calques choisis." };
            self.status = match left {
                Some(l) => format!("{none} {l}"),
                None => none.into(),
            };
            return;
        }
        match self.multi.sel.as_mut().filter(|_| add) {
            Some(sel) => sel.merge(parts),
            None => self.multi.sel = Some(self.new_multi_sel(parts)),
        }
        let all = self.multi.all_layers;
        self.multi_adopt_layers();
        let summary = self.multi_summary().unwrap_or_default();
        let keys = if all && !self.key_sel.is_empty() { format!(" Clés de ces calques choisies dans le montage et la timeline ({}).", crate::montagekeys::key_counts_text(&self.key_sel)) } else { String::new() };
        self.status = match left {
            Some(l) => format!("{summary} choisis. {l}{keys}"),
            None => format!("{summary} choisis.{keys}"),
        };
    }

    /// « Tout sélectionner » avec plusieurs calques choisis : tout ce que contient le dessin de
    /// chacun à l'image courante.
    pub(crate) fn multi_select_all(&mut self) {
        self.end_stroke();
        if self.content_refuses() {
            return;
        }
        self.vsel = None;
        self.drop_bitmap_selection();
        self.multi_flush();
        let targets = self.multi_targets();
        let parts = multisel::select_everything(&self.doc, &targets);
        let left = targets.skipped_text();
        if parts.is_empty() {
            self.multi.sel = None;
            let none = if targets.list.is_empty() { "Aucun des calques choisis ne peut être sélectionné." } else { "Rien à sélectionner : les dessins des calques choisis sont vides." };
            self.status = match left {
                Some(l) => format!("{none} {l}"),
                None => none.into(),
            };
            return;
        }
        self.multi.sel = Some(self.new_multi_sel(parts));
        self.tool = Tool::Select;
        // « 42 traits sur 3 calques » (ou « … sur 6 dessins de 2 calques », par les clés ; « et 2 zones » pour les calques bitmap).
        let compte = self.multi.sel.as_ref().map(compte_tout).unwrap_or_default();
        self.status = match left {
            Some(l) => format!("Tout sélectionné : {compte}. {l}"),
            None => format!("Tout sélectionné : {compte}."),
        };
    }

    // ------------------------------------------------------------------ poser un geste

    /// Le geste est lâché : la géométrie des traits reçoit la transformation (leur feuille reste
    /// affichée jusqu'au rendu exact, qui vient d'un fil de travail), les zones bitmap gardent leur
    /// place, et tout forme une seule étape d'annulation au nom du geste.
    fn multi_commit(&mut self, handle: Handle, frame: SelFrame, snaps: Vec<multisel::Snap>, aff: Affine) {
        let Some(sel) = self.multi.sel.as_mut() else { return };
        let edits = multisel::commit_gesture(&mut self.doc, sel, &mut self.multi.floats, snaps, &aff);
        sel.keep_frame(&self.doc, frame.then(&aff));
        self.multi_replay();
        if !edits.is_empty() {
            self.named_step(handle.step_name(), |app| app.history.push(crate::history::Edit::Group(edits)));
            self.modified = true;
        }
    }

    /// Demande le rendu exact des dessins vectoriels de la sélection (en arrière-plan).
    fn multi_replay(&mut self) {
        let cels: Vec<_> = self.multi.sel.iter().flat_map(|s| s.parts.iter()).filter(|p| matches!(p.kind, PartKind::Vector { .. })).map(|p| p.cel).collect();
        for cel in cels {
            self.rejeu_cel(cel);
        }
    }

    /// Les champs chiffrés de la boîte ont changé : la boîte devient celle de `params`. Le premier
    /// changement ouvre un geste, les suivants le prolongent ; il se ferme quand plus aucun champ
    /// n'a la main (`finish_field_edit_if_idle`).
    pub(crate) fn multi_apply_params(&mut self, live: SelFrame, params: BoxParams, handle: Handle) {
        let open = matches!(&self.multi.drag, Some(MDrag::Transform { field: true, .. }));
        if !open {
            if self.content_refuses() {
                return;
            }
            self.multi_flush();
            self.multi.drag = Some(MDrag::Transform { handle, start: Pos2::ZERO, frame: live, snaps: None, aff: Affine::IDENTITY, field: true });
        }
        let Some(MDrag::Transform { frame: frame0, field: true, .. }) = &self.multi.drag else { return };
        let frame0 = *frame0;
        let Some(target) = live.with_params(&params) else { return };
        if let Some(d) = frame0.delta_to(&target) {
            self.multi_set_gesture(d);
        }
    }

    /// Un geste d'un seul coup (flèches, miroir) : saisie, pose et lâché, puis le rendu exact.
    /// `grouped` : les appuis successifs de la même touche se joignent en une étape.
    fn multi_instant(&mut self, a: Affine, name: &str, grouped: bool, keep_box: bool) {
        if self.content_refuses() {
            return;
        }
        self.multi_flush();
        let Some(sel) = self.multi.sel.as_mut() else { return };
        let Some(frame0) = sel.base_frame(&self.doc) else { return };
        let (edits, dirty) = multisel::run_gesture(&mut self.doc, sel, &mut self.multi.floats, &a, true);
        if keep_box {
            sel.keep_frame(&self.doc, frame0.then(&a));
        } else {
            sel.boxed = None;
        }
        if !dirty.is_empty() {
            self.renderer.invalidate_tiles(&dirty);
        }
        self.multi_replay();
        if grouped {
            self.gesture_group(NUDGE_GESTURE, name, edits);
        } else if !edits.is_empty() {
            self.named_step(name, |app| app.history.push(crate::history::Edit::Group(edits)));
            self.modified = true;
        }
    }

    /// Les flèches : la sélection bouge de `d` pixels de la feuille (Maj : dix).
    pub(crate) fn multi_nudge(&mut self, d: Vec2) {
        self.multi_instant(Affine::translate(d), "Déplacer", true, true);
        let summary = self.multi_summary().unwrap_or_default();
        self.status = format!("{summary} déplacés de {} px (flèches ; Maj + flèche : {} px).", d.length().round(), crate::nudge::BIG_STEP);
    }

    /// Miroir de toute la sélection autour du centre de sa boîte.
    pub(crate) fn multi_flip(&mut self, horizontal: bool) {
        let Some(c) = self.multi.sel.as_ref().and_then(|s| s.stage_bounds(&self.doc)).map(|b| b.center()) else { return };
        let a = if horizontal { Affine::scale_about(c, -1.0, 1.0) } else { Affine::scale_about(c, 1.0, -1.0) };
        self.multi_instant(a, "Retourner", false, false);
        let summary = self.multi_summary().unwrap_or_default();
        self.status = format!("{summary} retournés.");
    }

    // ------------------------------------------------------------------ copier, couper, coller

    pub(crate) fn multi_copy(&mut self) -> bool {
        let Some(sel) = &self.multi.sel else { return false };
        let clip = multisel::copy_selection(&self.doc, sel);
        if clip.items.is_empty() {
            self.status = "Rien à copier dans la sélection.".into();
            return false;
        }
        let text = format!("{}{}", crate::pasteimage::MARK, sel.summary());
        self.status = format!("{} ({}).", clip.summary(), sel.summary());
        // Un texte dans le presse-papiers du système : sans lui, Ctrl+V n'arrive pas jusqu'à nous.
        self.ctx.copy_text(text);
        self.multi_clip = Some(clip);
        self.collage.origines.multi = Some(crate::adaptation::Origine::de(&self.doc));
        true
    }

    pub(crate) fn multi_cut(&mut self) {
        if self.multi_copy() {
            self.multi_delete();
        }
    }

    /// Colle sur les calques choisis ce que la copie avait pris sur eux, à la même place. Avec un seul calque en face (un seul
    /// calque choisi, sans plusieurs clés choisies) et aucun calque de la copie parmi eux : tout ce qui est de sa sorte s'y colle.
    /// Ce qui n'a pas de calque en face est laissé de côté, et la barre d'état le dit.
    pub(crate) fn multi_paste(&mut self) {
        if self.content_refuses() {
            return;
        }
        let Some(original) = self.multi_clip.clone() else { return };
        // D'un projet d'une autre feuille : adaptée, elle garde sa place dans le cadre (`adaptation.rs`).
        let mut clip = match self.collage_preparer(crate::collage::Colle::Multi(original), crate::collage::Reprise::Multi) {
            crate::collage::Preparation::Pret(crate::collage::Colle::Multi(c)) => c,
            _ => return,
        };
        // (Une copie n'est pas une part du trait d'origine : numéros neufs, partie 125.)
        for item in &mut clip.items {
            if let crate::multisel::ClipWhat::Vector(els) = &mut item.1 {
                crate::vector::renumeroter(els);
            }
        }
        self.end_stroke();
        self.multi_flush();
        self.drop_multi();
        self.vsel = None;
        self.drop_bitmap_selection();
        let targets = self.multi_targets();
        // Copiée dans un autre projet, aucun numéro de calque ne correspond : chaque calque copié va sur le premier
        // calque choisi de même sorte (vectoriel pour des éléments, bitmap pour des pixels) encore libre, dans l'ordre ;
        // ce qui n'a pas de calque en face est laissé de côté.
        if !clip.items.iter().any(|(l, _)| targets.list.iter().any(|t| t.id == *l)) {
            if let [seul] = targets.list.as_slice() {
                // Un seul calque en face (on est revenu à un seul calque, ou on colle sur un autre) : tout ce qui est de sa sorte
                // s'y colle, comme une copie d'un seul calque.
                for item in &mut clip.items {
                    item.0 = if matches!(item.1, crate::multisel::ClipWhat::Vector(_)) == seul.vector { seul.id } else { 0 };
                }
            } else {
                let mut pris = vec![false; targets.list.len()];
                for item in &mut clip.items {
                    let vectoriel = matches!(item.1, crate::multisel::ClipWhat::Vector(_));
                    let place = targets.list.iter().enumerate().find(|(i, t)| !pris[*i] && t.vector == vectoriel);
                    item.0 = match place {
                        Some((i, t)) => {
                            pris[i] = true;
                            t.id
                        }
                        None => 0,
                    };
                }
            }
        }
        // Plusieurs morceaux de la copie pour un même calque (deux clés d'un même calque copiées ensemble, ou regroupés ci-dessus) :
        // un seul morceau, les éléments à la suite — une seule part par dessin, une seule étape d'annulation par dessin. Des
        // pixels ne se regroupent pas : le premier morceau seul est gardé.
        let mut regroupes: Vec<(u64, crate::multisel::ClipWhat)> = Vec::new();
        let mut laisses = 0usize;
        for (calque, quoi) in std::mem::take(&mut clip.items) {
            if !targets.list.iter().any(|t| t.id == calque) {
                laisses += 1;
                continue;
            }
            match (regroupes.iter_mut().find(|(c, _)| *c == calque), quoi) {
                (Some((_, crate::multisel::ClipWhat::Vector(deja))), crate::multisel::ClipWhat::Vector(els)) => deja.extend(els),
                (Some(_), _) => laisses += 1,
                (None, quoi) => regroupes.push((calque, quoi)),
            }
        }
        clip.items = regroupes;
        let (parts, edits) = multisel::paste_clip(&mut self.doc, &targets, &clip);
        if parts.is_empty() {
            let pourquoi = targets.skipped_text().map(|t| format!(" {t}")).unwrap_or_default();
            self.status = format!("Rien à coller : les calques de la copie ne sont pas parmi les calques choisis, ou ils ne peuvent pas être modifiés.{pourquoi}");
            return;
        }
        self.renderer.invalidate_all();
        self.multi.sel = Some(self.new_multi_sel(parts));
        self.tool = Tool::Select;
        self.named_step("Coller", |app| app.history.push(crate::history::Edit::Group(edits)));
        self.modified = true;
        let summary = self.multi_summary().unwrap_or_default();
        let reste = match laisses {
            0 => String::new(),
            1 => " Un morceau de la copie n'avait pas de calque choisi en face : laissé de côté.".to_string(),
            n => format!(" {n} morceaux de la copie n'avaient pas de calque choisi en face : laissés de côté."),
        };
        self.status = format!("{summary} collés : déplace-les.{reste}");
    }

    /// Suppr : ce qui est choisi sur chaque calque s'efface, en une seule étape.
    pub(crate) fn multi_delete(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.multi_flush();
        let Some(sel) = self.multi.sel.take() else { return };
        let edits = multisel::delete_selection(&mut self.doc, &sel);
        self.renderer.invalidate_all();
        let summary = sel.summary();
        if !edits.is_empty() {
            self.named_step("Supprimer", |app| app.history.push(crate::history::Edit::Group(edits)));
            self.modified = true;
        }
        self.status = format!("{summary} supprimés.");
    }

    // ------------------------------------------------------------------ la vie de la sélection

    /// Oublie la sélection sur plusieurs calques (les feuilles lâchées attendent leur rendu).
    pub(crate) fn drop_multi(&mut self) {
        if matches!(&self.multi.drag, Some(MDrag::Transform { .. })) {
            self.multi_finish_drag();
        }
        self.multi.drag = None;
        self.multi.sel = None;
    }

    /// Avant une annulation ou un rétablissement : les feuilles lâchées sont terminées, la
    /// sélection s'en va (elle ne serait plus celle des dessins refaits).
    pub(crate) fn multi_before_history(&mut self) {
        // (La séance de Déformer sur plusieurs calques aussi : ses dessins d'origine ne seraient plus ceux du projet.)
        self.deform_multi_fermer();
        self.drop_multi();
        self.multi_flush();
    }

    /// Un autre projet, une autre taille de feuille : tout s'en va, sans toucher aux dessins.
    pub(crate) fn multi_cancel(&mut self) {
        self.multi.drag = None;
        self.multi.sel = None;
        for f in std::mem::take(&mut self.multi.floats) {
            Flottante::release_slot(&mut self.doc, f.slot);
        }
        *self.multi.contours.borrow_mut() = None;
        *self.multi.ghosts.borrow_mut() = None;
    }

    /// Termine tout de suite les gestes lâchés dont la feuille attend le rendu exact : les
    /// éléments reprennent leurs pixels en cache suivis, l'image du dessin est refaite là où
    /// quelque chose a changé, la feuille s'en va. (Les gestes en cours gardent la leur.)
    pub(crate) fn multi_flush(&mut self) {
        let (done, rest): (Vec<Flottante>, Vec<Flottante>) = std::mem::take(&mut self.multi.floats).into_iter().partition(|f| f.pending.is_some());
        self.multi.floats = rest;
        for f in done {
            self.settle_float(f, false);
        }
    }

    /// Le rendu exact du dessin `cel` est arrivé (`adopted`) ou n'est plus bon : les feuilles
    /// lâchées sur ce dessin s'en vont, après avoir refait son image à l'ancienne dans le second cas.
    pub(crate) fn multi_settle(&mut self, cel: crate::document::CelId, adopted: bool) {
        let (done, rest): (Vec<Flottante>, Vec<Flottante>) = std::mem::take(&mut self.multi.floats).into_iter().partition(|f| f.src == cel && f.pending.is_some());
        self.multi.floats = rest;
        for f in done {
            self.settle_float(f, adopted);
        }
    }

    fn settle_float(&mut self, f: Flottante, adopted: bool) {
        // Une déformation sur plusieurs calques (`deformmulti.rs`) : la feuille déformée est posée sur le reste, comme
        // `flush_flottante` le fait pour un seul calque.
        if !adopted {
            if let Some(warp) = &f.warp {
                let tiles = Flottante::flatten_warp_slot(&mut self.doc, f.src, f.slot, warp);
                self.renderer.invalidate_tiles(&tiles);
            }
        }
        Flottante::release_slot(&mut self.doc, f.slot);
        if adopted {
            return;
        }
        if let Some(p) = f.pending {
            let tiles = Flottante::apply_preview(&mut self.doc, f.src, &p.items, &p.originals, &f.aff);
            self.renderer.invalidate_tiles(&tiles);
        }
    }

    /// Une fois par image, avant d'agir : la sélection ne vaut que pour le plan, l'image et les
    /// calques choisis où on l'a prise ; les zones bitmap n'existent qu'avec l'outil Sélection.
    pub(crate) fn multi_sync(&mut self) {
        let Some(sel) = self.multi.sel.as_ref() else { return };
        // Un geste de la boîte en cours : rien ne change sous la main.
        if matches!(&self.multi.drag, Some(MDrag::Transform { .. })) {
            return;
        }
        let chosen: Vec<u64> = self.chosen_layers().iter().filter_map(|&li| self.layers().get(li).map(|l| l.id)).collect();
        // Sur des clés : leurs calques comptent comme choisis ; une seule clé (ou Échap) : fini.
        let keyed: Vec<u64> = if self.keys_mode() { self.chosen_key_drawings().iter().map(|k| k.0).collect() } else { Vec::new() };
        // La cible (le trait ou l'aplat) a changé depuis la prise : la sélection n'est plus la bonne.
        let aplat = self.aplat_strate_selection();
        let valid = sel.still_valid(&self.doc, self.cur_panel, self.cur_frame)
            && sel.parts.iter().all(|p| p.aplat == aplat)
            && sel.parts.iter().all(|p| chosen.contains(&p.layer) || keyed.contains(&p.layer))
            && !(sel.by_keys && !self.keys_mode());
        if !valid {
            self.multi.sel = None;
            return;
        }
        let bitmap_here = self.tool == Tool::Select;
        let doc = &self.doc;
        if let Some(sel) = self.multi.sel.as_mut() {
            if !bitmap_here && sel.has_bitmap() {
                // Les zones bitmap n'existent qu'avec l'outil Sélection ; les traits restent.
                sel.parts.retain(|p| matches!(p.kind, PartKind::Vector { .. }));
                sel.boxed = None;
                if sel.parts.is_empty() {
                    self.multi.sel = None;
                }
                return;
            }
            sel.refresh_maps(doc);
        }
    }

    // ------------------------------------------------------------------ l'écran

    /// Le lasso en cours, les contours des traits choisis, le contour des zones et la boîte.
    /// Rend vrai quand la boîte commune est tracée (rien d'autre à montrer pour la sélection).
    pub(crate) fn paint_multi_overlay(&self, painter: &Painter) -> bool {
        let to_s = |p: Pos2| self.stage_to_screen(p);
        let lasso: Option<Vec<Pos2>> = match &self.multi.drag {
            Some(MDrag::Lasso { points, .. }) => Some(points.iter().map(|p| to_s(*p)).collect()),
            Some(MDrag::Rect { from, to, .. }) => {
                let mut pts: Vec<Pos2> = crate::pixsel::rect_polygon(Rect::from_two_pos(*from, *to)).into_iter().map(to_s).collect();
                pts.extend(pts.first().copied());
                Some(pts)
            }
            _ => None,
        };
        if let Some(shape) = lasso.and_then(|pts| contours::lasso_shape(&pts)) {
            painter.add(shape);
        }
        let Some(sel) = &self.multi.sel else { return false };
        if self.multi_live_frame().is_none() {
            return false;
        }
        // Geste en cours : les traits n'ont pas bougé, leurs contours suivent la feuille.
        let (moved, gesture) = match &self.multi.drag {
            Some(MDrag::Transform { aff, snaps: Some(_), .. }) => (*aff, true),
            _ => (Affine::IDENTITY, false),
        };
        // Les dessins qu'on voit : le halo plein ; ceux des autres clés (qu'on ne voit pas) : des
        // fantômes plus pâles, qui suivent le geste.
        for ghost in [false, true] {
            if let Some(shape) = self.multi_contours(sel, moved, gesture, ghost) {
                painter.add(shape);
            }
        }
        // Contour des zones bitmap : un trait orange d'1 px (plus pâle pour un autre dessin).
        for part in &sel.parts {
            if let PartKind::Bitmap { outline, lift, .. } = &part.kind {
                let aff = lift.as_ref().map_or(Affine::IDENTITY, |l| l.aff);
                // (Les pixels d'un autre dessin ne sont posés qu'au lâcher : son contour suit le geste d'ici là.)
                let extra = if part.live { Affine::IDENTITY } else { moved };
                let color = if part.live { ACCENT } else { ACCENT.gamma_multiply(GHOST_ALPHA) };
                for poly in outline {
                    let mut pts: Vec<Pos2> = poly.iter().map(|q| to_s(extra.apply(part.to_stage.apply(aff.apply(*q))))).collect();
                    if let Some(&first) = pts.first() {
                        pts.push(first);
                    }
                    painter.add(Shape::line(pts, Stroke::new(1.0_f32, color)));
                }
            }
        }        self.paint_selection_box(painter);
        true
    }

    /// Les contours des traits choisis de tous les calques, sur la feuille, gardés d'une image à
    /// l'autre (`contours.rs`) : refaits seulement si la sélection ou un dessin change, jamais
    /// pendant un geste (seulement déplacés).
    fn multi_contours(&self, sel: &MultiSel, moved: Affine, gesture: bool, ghost: bool) -> Option<Shape> {
        let mut key: u64 = if ghost { 0x9e37_79b9_7f4a_7c15 } else { 0xcbf2_9ce4_8422_2325 };
        let mut any = false;
        for p in sel.parts.iter().filter(|p| p.live != ghost) {
            let PartKind::Vector { items } = &p.kind else { continue };
            let c = self.doc.cels.get(&p.cel)?;
            let d = c.vector.as_ref()?;
            any = true;
            for v in [contours::fingerprint(p.cel, c.version, d, items), p.to_stage.a.to_bits() as u64, p.to_stage.d.to_bits() as u64, p.to_stage.tx.to_bits() as u64, p.to_stage.ty.to_bits() as u64] {
                key ^= v;
                key = key.wrapping_mul(0x100_0000_01b3);
            }
        }
        if !any {
            return None;
        }
        let to_s = |p: Pos2| self.stage_to_screen(moved.apply(p));
        let now = contours::ANCHORS.map(to_s);
        let mut slot = if ghost { self.multi.ghosts.borrow_mut() } else { self.multi.contours.borrow_mut() };
        let reuse = slot.as_ref().filter(|k| k.key() == key).and_then(|k| k.transform_to(&now)).filter(|t| gesture || !Contours::too_scaled(t));
        if let (Some(t), Some(k)) = (reuse, slot.as_ref()) {
            return Some(k.shape(&t));
        }
        let mut lines: Vec<contours::RadiusLine> = Vec::new();
        for p in sel.parts.iter().filter(|p| p.live != ghost) {
            if let PartKind::Vector { items } = &p.kind {
                if let Some(d) = self.doc.cels.get(&p.cel).and_then(|c| c.vector.as_ref()) {
                    // Les rayons suivent l'échelle du calque sur la feuille.
                    let k = p.to_stage.a.hypot(p.to_stage.c);
                    lines.extend(contours::radius_lines_of(d, items).into_iter().map(|l| contours::RadiusLine { points: l.points.into_iter().map(|q| p.to_stage.apply(q)).collect(), radii: l.radii.into_iter().map(|r| r * k).collect() }));
                }
            }
        }
        let color = if ghost { ACCENT.gamma_multiply(GHOST_ALPHA) } else { ACCENT };
        let built = Contours::build_halo(key, &lines, to_s, self.view.zoom, now, color, ghost);
        let shape = built.shape(&Affine::IDENTITY);
        *slot = Some(built);
        Some(shape)
    }

    /// Réglages de l'outil Sélection avec plusieurs calques choisis.
    pub(crate) fn multi_selection_ui(&mut self, ui: &mut egui::Ui, lasso: bool) {
        use crate::bulles::choix;
        use egui_phosphor::regular as ph;
        let targets = self.multi_targets();
        let chosen = self.chosen_layers().len();
        if self.aplat_strate_selection() {
            let nom = self.layers().get(self.cur_layer).map(|l| l.name.clone()).unwrap_or_default();
            if self.keys_mode() {
                let (n, m) = (self.chosen_key_drawings().len(), targets.list.len());
                ui.label(egui::RichText::new(format!("Aplat de {nom} : {n} clés, le geste s'applique aux aplats de {m} dessins.")).strong());
                ui.label(egui::RichText::new("Échap : revenir à l'aplat du dessin courant seul.").small().weak());
            } else {
                ui.label(egui::RichText::new(format!("Aplat de {nom} : la sélection prend ses pixels, pas le trait.")).strong());
            }
        } else if self.keys_mode() {
            let (n, m) = (self.chosen_key_drawings().len(), targets.list.len());
            ui.label(egui::RichText::new(format!("{n} clés : le geste s'applique aux {m} dessins.")).strong());
            ui.label(egui::RichText::new("Échap : revenir au dessin courant seul.").small().weak());
        } else {
            let plural = if chosen > 1 { "calques choisis : le lasso prend sur tous" } else { "calque choisi" };
            ui.label(egui::RichText::new(format!("{chosen} {plural}.")).strong());
        }
        if let Some(left) = targets.skipped_text() {
            ui.label(egui::RichText::new(left).small().weak());
        }
        if lasso {
            ui.horizontal(|ui| {
                choix(ui, &mut self.select_rect, false, ph::LASSO, "Lasso", "Entourer la zone à main levée.", "");
                choix(ui, &mut self.select_rect, true, ph::RECTANGLE_DASHED, "Rectangle", "Tirer un rectangle.", "");
            });
        }
        let summary = self.multi_summary();
        let has = summary.is_some();
        ui.label(match &summary {
            Some(s) => format!("{s} sélectionnés."),
            None => "Rien de sélectionné.".to_string(),
        });
        if let Some(sel) = &self.multi.sel {
            ui.label(egui::RichText::new(sel.per_layer()).small().weak());
        }
        ui.horizontal_wrapped(|ui| {
            if self.bouton_si(ui, has, ph::FLIP_HORIZONTAL, "Miroir horizontal", "Retourner de gauche à droite, tous les calques ensemble.", "selection.miroir_h").clicked() {
                self.multi_flip(true);
            }
            if self.bouton_si(ui, has, ph::FLIP_VERTICAL, "Miroir vertical", "Retourner de haut en bas, tous les calques ensemble.", "selection.miroir_v").clicked() {
                self.multi_flip(false);
            }
            if self.bouton_si(ui, has, ph::COPY, "Copier", "Copier ce qui est choisi sur chaque calque.", "edition.copier").clicked() {
                self.multi_copy();
            }
            if self.bouton_si(ui, has, ph::SCISSORS, "Couper", "Copier, puis effacer.", "edition.couper").clicked() {
                self.multi_cut();
            }
            if self.bouton_si(ui, has, ph::TRASH, "Supprimer", "Effacer ce qui est choisi sur chaque calque, en une étape.", "edition.supprimer").clicked() {
                self.multi_delete();
            }
            ui.separator();
            if self.bouton(ui, ph::SELECTION_ALL, "Tout sélectionner", "Tout ce que contiennent les dessins des calques choisis.", "edition.tout_selectionner").clicked() {
                self.multi_select_all();
            }
            let can_paste = self.multi_clip.is_some();
            if self.bouton_si(ui, can_paste, ph::CLIPBOARD, "Coller", "Coller sur chaque calque ce qu'il avait copié, à la même place.", "edition.coller").clicked() {
                self.multi_paste();
            }
        });
        if has {
            self.box_fields_ui(ui);
        }
        // (Le mode d'emploi est dans l'animation « Plusieurs calques dans une seule boîte » et le registre des explications.)
        self.avertir_laisses_de_cote(ui);
    }
}
