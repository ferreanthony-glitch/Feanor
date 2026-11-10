//! Les clés d'opacité à l'écran et dans les gestes (Anthony, 2026-10-29) : le losange « Créer une clé » à côté de l'opacité du
//! panneau Calques, les losanges gris clair en bas de la ligne du calque dans la timeline (les choisir, les glisser, les supprimer,
//! les lisser), les commandes du fichier de raccourcis. Le modèle (valeur entre les clés, adoucis) est dans `opacitecles.rs` ; la
//! mécanique est celle des clés de position (`poskeysui.rs`), sans second système : mêmes gestes, mêmes adoucis.
//!
//! La règle d'emploi :
//! - « Créer une clé » (panneau Calques, ou la commande `calque.cle_opacite`) pose une clé à l'image courante avec l'opacité actuelle
//!   du calque.
//! - Un calque qui a des clés d'opacité est animé : changer son opacité à une image pose ou met à jour la clé de cette image ; entre
//!   deux clés l'opacité va de l'une à l'autre. Sans clé, le calque garde son opacité fixe.
//! - Dans la timeline : un clic sur un losange le choisit et y met la tête de lecture (Ctrl : en ajouter ou en retirer, Maj : ajouter
//!   celles d'entre), glisser le déplace (avec lui, les autres clés choisies du calque), Suppr le supprime, clic droit : lisser,
//!   supprimer. Les losanges lissés sont des ronds.
//! - Dans le montage : les mêmes losanges sont dessinés sur la piste du calque (`montagerows.rs`).

use eframe::egui::{self, pos2, Color32, Painter, Pos2, Rect, RichText, Shape, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::history::Edit;
use crate::opacitecles::{self as ok, OpKey};

/// Demi-hauteur de la zone qui prend un losange à la souris.
pub(crate) const HIT_H: f32 = 5.0;

/// Les clés d'opacité choisies (timeline) et le geste en cours.
#[derive(Default)]
pub(crate) struct OpUi {
    /// Clés choisies : (id du calque, image du plan).
    pub sel: Vec<(u64, usize)>,
    /// La dernière clé cliquée : le départ d'un choix Maj + clic.
    pub anchor: Option<(u64, usize)>,
    pub drag: Option<OpDrag>,
}

/// Des clés prises à la souris : les clés du calque telles qu'elles étaient, celles qui bougent, le point pris, le décalage en cours,
/// l'étape d'annulation de tout le geste.
pub(crate) struct OpDrag {
    layer: u64,
    before: Vec<OpKey>,
    frames: Vec<usize>,
    grab: usize,
    delta: isize,
    max: usize,
    snap: Edit,
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

impl App {
    // ------------------------------------------------------------------ les commandes

    /// Pose une clé d'opacité à l'image courante du calque courant, avec son opacité actuelle.
    pub(crate) fn op_key_here(&mut self) {
        if self.content_refuses() {
            return;
        }
        let Some(layer) = self.layers().get(self.cur_layer) else { return };
        let f = self.cur_frame;
        let v = layer.opacity_at(f);
        let had = ok::key_at(&layer.opacity_keys, f).is_some();
        self.end_stroke();
        self.named_step("Poser une clé d'opacité", |app| app.record_step());
        let li = self.cur_layer;
        self.layers_mut()[li].put_opacity_key(f, v);
        self.renderer.invalidate_params();
        self.modified = true;
        self.status = if had {
            format!("Il y avait déjà une clé d'opacité à l'image {} : elle garde {} %.", self.frame_label(f), (v * 100.0).round())
        } else {
            format!("Clé d'opacité posée à l'image {} ({} %) : change d'image et règle l'opacité pour l'animer.", self.frame_label(f), (v * 100.0).round())
        };
    }

    /// Supprime les clés d'opacité choisies dans la timeline (quand elle a la main), sinon celle de l'image courante.
    pub(crate) fn op_keys_delete(&mut self) {
        self.op_keys_delete_with(self.op_keys_have_focus());
    }

    fn op_keys_delete_with(&mut self, chosen: bool) {
        if self.content_refuses() {
            return;
        }
        let f = self.cur_frame;
        let mut list: Vec<(u64, usize)> = if chosen {
            self.op_ui.sel.iter().copied().filter(|&(id, fr)| self.layers().iter().any(|l| l.id == id && ok::key_at(&l.opacity_keys, fr).is_some())).collect()
        } else {
            Vec::new()
        };
        if list.is_empty() {
            if let Some(l) = self.layers().get(self.cur_layer).filter(|l| ok::key_at(&l.opacity_keys, f).is_some()) {
                list.push((l.id, f));
            }
        }
        if list.is_empty() {
            self.status = "Pas de clé d'opacité ici : rien n'est supprimé.".into();
            return;
        }
        self.end_stroke();
        self.named_step("Supprimer des clés d'opacité", |app| app.record_step());
        for &(id, fr) in &list {
            let Some(l) = self.layers_mut().iter_mut().find(|l| l.id == id) else { continue };
            let here = l.opacity_at(f);
            ok::remove_key(&mut l.opacity_keys, fr);
            // La dernière clé partie, le calque garde l'opacité qu'il avait à cette image.
            if l.opacity_keys.is_empty() {
                l.opacity = here;
            }
        }
        self.op_ui.sel.clear();
        self.renderer.invalidate_params();
        self.modified = true;
        let n = list.len();
        self.status = format!("{n} clé{} d'opacité supprimée{}.", s(n), s(n));
    }

    /// Lisse (ou ne lisse plus) la clé d'opacité de l'image courante : l'opacité y arrive et en repart lentement.
    pub(crate) fn op_key_smooth(&mut self) {
        if self.content_refuses() {
            return;
        }
        let f = self.cur_frame;
        let Some(layer) = self.layers().get(self.cur_layer).filter(|l| ok::key_at(&l.opacity_keys, f).is_some()) else {
            self.status = "Pas de clé d'opacité à cette image : pose-en une d'abord.".into();
            return;
        };
        // Une clé déjà adoucie (lissée ou réglée) ne l'est plus ; sinon elle est lissée.
        let eased = layer.opacity_keys.iter().any(|k| k.frame == f && k.is_eased());
        self.end_stroke();
        self.named_step(if eased { "Ne plus lisser une clé d'opacité" } else { "Lisser une clé d'opacité" }, |app| app.record_step());
        let li = self.cur_layer;
        ok::set_eases(&mut self.layers_mut()[li].opacity_keys, f, !eased, 0.0, 0.0);
        self.renderer.invalidate_params();
        self.modified = true;
        self.status = if eased { "Clé d'opacité : vitesse constante.".into() } else { "Clé d'opacité lissée : l'opacité y arrive et en repart lentement.".into() };
    }

    /// Supprime toutes les clés d'opacité du calque courant : il garde l'opacité qu'il a à l'image courante.
    pub(crate) fn op_keys_clear(&mut self) {
        if self.content_refuses() {
            return;
        }
        if !self.layers().get(self.cur_layer).is_some_and(|l| l.is_opacity_animated()) {
            self.status = "L'opacité de ce calque n'est pas animée : pas de clé d'opacité.".into();
            return;
        }
        self.end_stroke();
        let (f, li) = (self.cur_frame, self.cur_layer);
        self.named_step("Supprimer les clés d'opacité", |app| app.record_step());
        let l = &mut self.layers_mut()[li];
        l.opacity = l.opacity_at(f);
        l.opacity_keys.clear();
        self.op_ui.sel.clear();
        self.renderer.invalidate_params();
        self.modified = true;
        self.status = "Clés d'opacité supprimées : le calque garde l'opacité de cette image.".into();
    }

    // ------------------------------------------------------------------ le panneau Calques

    /// Les boutons des clés d'opacité à côté du réglage d'opacité du panneau Calques : « Créer une clé » (un losange, plein quand une
    /// clé est posée à cette image), supprimer la clé d'ici, supprimer toutes les clés.
    pub(crate) fn op_keys_buttons(&mut self, ui: &mut egui::Ui) {
        let (li, f) = (self.cur_layer, self.cur_frame);
        let Some(layer) = self.layers().get(li) else { return };
        let keys = layer.opacity_keys.len();
        let here = ok::key_at(&layer.opacity_keys, f).is_some();
        if self
            .bouton_etat(ui, here, ph::DIAMOND, "Créer une clé", "Pose une clé d'opacité à l'image courante, avec l'opacité actuelle du calque. Change d'image et règle l'opacité : elle s'anime, et va d'une clé à l'autre.", "calque.cle_opacite")
            .clicked()
        {
            // (Plusieurs calques choisis : une clé sur chacun, `choixtout.rs`.)
            self.run_action("calque.cle_opacite");
        }
        if keys > 0 {
            if self.bouton_si(ui, here, ph::TRASH, "Supprimer la clé d'ici", "Retire la clé d'opacité de cette image.", "calque.cle_opacite_supprimer").clicked() {
                self.op_keys_delete_here();
            }
            if self.bouton(ui, ph::ERASER, "Supprimer toutes les clés d'opacité", "L'opacité n'est plus animée : le calque garde celle qu'il a à cette image.", "calque.cles_opacite_supprimer").clicked() {
                self.op_keys_clear();
            }
            ui.label(RichText::new(format!("{keys} clé{}", s(keys))).small().weak());
        }
    }

    /// Supprime la clé de l'image courante, quel que soit le choix de la timeline (le bouton du panneau).
    fn op_keys_delete_here(&mut self) {
        self.op_keys_delete_with(false);
    }

    // ------------------------------------------------------------------ la timeline

    /// Des clés d'opacité sont choisies dans la timeline et elle a la main : Suppr les vise.
    pub(crate) fn op_keys_have_focus(&self) -> bool {
        (self.images_focus || self.key_focus) && self.op_ui.sel.iter().any(|&(id, fr)| self.layers().iter().any(|l| l.id == id && ok::key_at(&l.opacity_keys, fr).is_some()))
    }

    /// Les losanges d'un calque : gris clair, ronds quand la clé est adoucie, cerclés de blanc quand ils sont choisis. `x_of` :
    /// l'abscisse du milieu d'une image ; `y` : leur hauteur ; `visible` : la zone vue.
    pub(crate) fn paint_op_markers(&self, painter: &Painter, layer: &crate::document::Layer, x_of: impl Fn(usize) -> f32, y: f32, max: usize, visible: (f32, f32)) {
        // (Cachés par le filtre de la légende, `legendecles.rs` : ni dessinés ni saisis.)
        if !self.cle_vue(crate::legendecles::Sorte::Opacite) {
            return;
        }
        for k in &layer.opacity_keys {
            let x = x_of(k.frame);
            if k.frame >= max || x < visible.0 - 8.0 || x > visible.1 + 8.0 {
                continue;
            }
            let chosen = self.op_ui.sel.contains(&(layer.id, k.frame)) || (k.frame == self.cur_frame && self.layers().get(self.cur_layer).is_some_and(|l| l.id == layer.id));
            paint_marker(painter, pos2(x, y), chosen, k.is_eased());
        }
    }

    /// La hauteur des losanges dans une ligne de calque qui commence en `top` et fait `h` de haut : en bas de la ligne.
    pub(crate) fn op_marker_y(top: f32, h: f32) -> f32 {
        top + h - 5.5
    }

    /// Le losange d'opacité sous le point `p` de la timeline : (calque, image).
    pub(crate) fn op_marker_in_timeline(&self, p: Pos2, r: Rect, cell: f32, rows: &[crate::layerheads::Row], grid_top: f32) -> Option<(usize, usize)> {
        let row = crate::layerheads::row_at(rows, p.y - grid_top)?;
        let layer = self.layers().get(row.li)?;
        if !self.cle_vue(crate::legendecles::Sorte::Opacite) || layer.opacity_keys.is_empty() || (p.y - (grid_top + Self::op_marker_y(row.top, row.h))).abs() > HIT_H {
            return None;
        }
        let tol = (cell * 0.5).max(5.0);
        crate::poskeysui::marker_near(layer.opacity_keys.iter().map(|k| k.frame), p.x, |f| r.left() + (f as f32 + 0.5) * cell, tol).map(|f| (row.li, f))
    }

    /// Clic sur le losange de l'image `frame` du calque `li` : il est choisi (`toggle` : ajouté ou retiré ; `range` : Maj + clic, les
    /// clés d'entre la dernière cliquée et celle-ci), la tête de lecture va dessus et le calque devient courant.
    pub(crate) fn click_op_marker(&mut self, li: usize, frame: usize, toggle: bool, range: bool) {
        if !self.choose_op_marker(li, frame, toggle, range) {
            return;
        }
        self.images_focus = true;
        self.panel_focus = false;
        self.key_focus = false;
        self.frame_sel = None;
        self.set_frame_free(frame);
        self.select_layer(li);
        self.say_op_choice();
    }

    /// Le choix des clés d'opacité : un clic le remplace, Ctrl ajoute ou retire, Maj ajoute celles d'entre. Faux si le calque n'existe plus.
    fn choose_op_marker(&mut self, li: usize, frame: usize, toggle: bool, range: bool) -> bool {
        let Some(layer) = self.layers().get(li) else { return false };
        let id = layer.id;
        let k = (id, frame);
        if range {
            let between = match self.op_ui.anchor {
                Some((a, from)) if a == id => ok::frames_between(&layer.opacity_keys, from, frame),
                _ => vec![frame],
            };
            for f in between {
                if !self.op_ui.sel.contains(&(id, f)) {
                    self.op_ui.sel.push((id, f));
                }
            }
            self.op_ui.anchor.get_or_insert(k);
        } else if toggle {
            match self.op_ui.sel.iter().position(|x| *x == k) {
                Some(i) => {
                    self.op_ui.sel.remove(i);
                }
                None => {
                    self.op_ui.sel.push(k);
                    self.op_ui.anchor = Some(k);
                }
            }
        } else {
            self.op_ui.sel = vec![k];
            self.op_ui.anchor = Some(k);
        }
        // Les clés d'opacité ont la main : elles remplacent le choix des clés de position.
        self.pos_ui.sel.clear();
        true
    }

    fn say_op_choice(&mut self) {
        let n = self.op_ui.sel.len();
        self.status = if n == 0 {
            "Plus aucune clé d'opacité choisie.".into()
        } else {
            format!("{n} clé{} d'opacité choisie{} : glisser pour décaler, Suppr pour supprimer ; clic droit : lisser.", s(n), s(n))
        };
    }

    // ------------------------------------------------------------------ le montage

    /// Clic sur le losange de l'image `frame` du calque `li` du plan `pi`, dans une piste du montage : comme dans la timeline, la tête
    /// de lecture va sur le losange (image du film) et ce plan devient courant. Les clés choisies sont d'un seul plan.
    pub(crate) fn click_op_marker_in_montage(&mut self, pi: usize, li: usize, frame: usize, toggle: bool, range: bool) {
        if pi >= self.doc.panels.len() {
            return;
        }
        if pi != self.cur_panel {
            self.set_panel(pi);
            self.op_ui.sel.clear();
            self.op_ui.anchor = None;
        }
        if !self.choose_op_marker(li, frame, toggle, range) {
            return;
        }
        // Les clés d'opacité remplacent le choix des autres clés (dessins, caméra, effets).
        self.clear_cam_keys();
        self.key_sel.clear();
        self.key_anchor = None;
        self.fx_key_sel.clear();
        self.frame_sel = None;
        self.panel_focus = false;
        self.images_focus = false;
        self.key_focus = true;
        let g = self.doc.panel_start(pi) + frame;
        self.goto_global(g);
        self.select_layer(li);
        self.say_op_choice();
    }

    /// Commence à glisser, depuis le montage, les clés choisies en prenant le losange de l'image `frame` du calque `li` du plan `pi` ;
    /// `grab` : l'image du plan sous la souris. Un losange pris hors du choix devient le choix.
    pub(crate) fn begin_op_drag_in_montage(&mut self, pi: usize, li: usize, frame: usize, grab: usize) -> bool {
        if pi >= self.doc.panels.len() {
            return false;
        }
        let id = self.doc.panels[pi].layers.get(li).map(|l| l.id);
        let taken = id.is_some_and(|id| pi == self.cur_panel && self.op_ui.sel.contains(&(id, frame)));
        if !taken {
            self.click_op_marker_in_montage(pi, li, frame, false, false);
        }
        self.key_focus = true;
        self.begin_op_drag(li, frame, grab)
    }

    // ------------------------------------------------------------------ déplacer

    /// Commence à glisser les clés choisies, en prenant le losange de l'image `frame` du calque `li` ; `grab` : l'image sous la souris.
    /// Un losange pris hors du choix devient le choix.
    pub(crate) fn begin_op_drag(&mut self, li: usize, frame: usize, grab: usize) -> bool {
        if self.content_refuses() {
            return false;
        }
        let Some(id) = self.layers().get(li).map(|l| l.id) else { return false };
        if !self.op_ui.sel.contains(&(id, frame)) {
            self.click_op_marker(li, frame, false, false);
        }
        let frames: Vec<usize> = self.op_ui.sel.iter().filter(|(l, _)| *l == id).map(|&(_, f)| f).collect();
        if frames.is_empty() {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let before = self.layers()[li].opacity_keys.clone();
        let snap = self.snapshot();
        let max = self.frame_count().saturating_sub(1);
        self.op_ui.drag = Some(OpDrag { layer: id, before, frames, grab, delta: 0, max, snap });
        true
    }

    /// Échap pendant le geste : les clés du calque reviennent comme avant, rien n'est enregistré.
    pub(crate) fn cancel_op_drag(&mut self) {
        let Some(d) = self.op_ui.drag.take() else { return };
        for p in &mut self.doc.panels {
            if let Some(l) = p.layers.iter_mut().find(|l| l.id == d.layer) {
                l.opacity_keys = d.before.clone();
            }
        }
        self.renderer.invalidate_params();
    }

    /// Les clés prises suivent la souris, à l'image `at` : le calque repart de ses clés d'avant le geste.
    pub(crate) fn drag_op_keys(&mut self, at: usize) {
        let Some(mut d) = self.op_ui.drag.take() else { return };
        let delta = ok::clamp_delta(&d.frames, at as isize - d.grab as isize, d.max);
        if delta != d.delta {
            d.delta = delta;
            if let Some(l) = self.layers_mut().iter_mut().find(|l| l.id == d.layer) {
                l.opacity_keys = ok::moved(&d.before, &d.frames, delta, d.max);
            }
            // (L'opacité est dans le plan à composer : aucun pixel n'est renvoyé à la carte graphique.)
            self.renderer.invalidate_params();
            let n = d.frames.len();
            self.status = format!("{n} clé{} d'opacité : {delta:+} image{}.", s(n), if delta.abs() > 1 { "s" } else { "" });
        }
        self.op_ui.drag = Some(d);
    }

    /// Lâche les clés : une étape d'annulation, « Déplacer des clés d'opacité », si quelque chose a bougé.
    pub(crate) fn end_op_drag(&mut self) {
        let Some(d) = self.op_ui.drag.take() else { return };
        if d.delta == 0 {
            return;
        }
        self.op_ui.sel.retain(|(l, _)| *l != d.layer);
        self.op_ui.sel.extend(d.frames.iter().map(|&f| (d.layer, (f as isize + d.delta).max(0) as usize)));
        self.modified = true;
        self.commit_named(d.snap, "Déplacer des clés d'opacité");
    }

    // ------------------------------------------------------------------ le menu du clic droit

    /// Le menu du clic droit sur un losange d'opacité.
    pub(crate) fn op_key_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Lisser la clé d'opacité", "calque.cle_opacite_lisser");
        self.action_item(ui, "Supprimer la clé d'opacité", "calque.cle_opacite_supprimer");
        ui.separator();
        self.action_item(ui, "Clé d'opacité ici (opacité actuelle)", "calque.cle_opacite");
        self.action_item(ui, "Supprimer toutes les clés d'opacité du calque", "calque.cles_opacite_supprimer");
    }
}

/// Un losange gris clair, cerclé de noir ; adouci : un rond ; choisi : plus grand et cerclé de blanc.
pub(crate) fn paint_marker(painter: &Painter, c: Pos2, chosen: bool, eased: bool) {
    let d = if chosen { 5.5 } else { 4.5 };
    let (fill, edge) = (crate::theme::CLE_OPACITE, if chosen { Color32::WHITE } else { Color32::from_black_alpha(190) });
    let stroke = Stroke::new(if chosen { 1.8_f32 } else { 1.2_f32 }, edge);
    if eased {
        painter.circle(c, d * 0.9, fill, stroke);
    } else {
        let pts = vec![pos2(c.x, c.y - d), pos2(c.x + d, c.y), pos2(c.x, c.y + d), pos2(c.x - d, c.y)];
        painter.add(Shape::convex_polygon(pts, fill, stroke));
    }
}
