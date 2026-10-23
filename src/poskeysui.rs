//! Les clés de position à l'écran et dans les gestes (Anthony, 2026-10-20) : le bouton « Clé de position »
//! des réglages de l'outil Transformer, les losanges verts sur la ligne du calque dans la timeline (les
//! choisir, les glisser, les supprimer, les lisser), les commandes du fichier de raccourcis. Le modèle
//! (pose entre les clés, adoucis) est dans `poskeys.rs`.
//!
//! La règle d'emploi :
//! - « Clé de position » (outil Transformer, ou la commande `calque.cle_position`) pose une clé à
//!   l'image courante avec la pose actuelle du calque. Avec les calques choisis (au moins deux), chacun
//!   reçoit sa clé.
//! - Un calque qui a des clés est animé : le déplacer, l'agrandir ou le tourner à une image pose ou met à
//!   jour la clé de cette image (il n'y a rien d'autre à faire pour l'animer) ; entre deux clés la pose va
//!   de l'une à l'autre. Sans clé, le calque garde sa pose fixe.
//! - « Lisser » : la clé de l'image courante arrive et repart lentement (ou les adoucis réglés à la main,
//!   en images, comme les clés de caméra).
//! - Dans la timeline : un clic sur un losange le choisit et y met la tête de lecture (Ctrl : en ajouter
//!   ou en retirer, Maj : ajouter celles d'entre), glisser le déplace (avec lui, les autres clés choisies
//!   du calque), Suppr le supprime, clic droit : lisser, supprimer. Les losanges lissés sont des ronds.
//! - Dans le montage (Anthony, 2026-10-20 : « ajoute le glisser dans le montage ») : les mêmes gestes sur
//!   les losanges verts, au milieu de la piste de chaque calque, de n'importe quel plan. Un geste qui part
//!   d'un losange passe avant les gestes du dessin dessous (poignées, glisser d'un dessin, choix au
//!   rectangle) ; les clés d'un seul plan à la fois sont choisies (le plan du losange pris devient courant).

use eframe::egui::{self, pos2, Color32, Painter, Pos2, Rect, RichText, Shape, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::history::Edit;
use crate::poskeys::{self, PosKey};

/// Demi-hauteur de la zone qui prend un losange à la souris.
pub(crate) const HIT_H: f32 = 7.0;

/// Les clés de position choisies (timeline et montage) et le geste en cours.
#[derive(Default)]
pub(crate) struct PosUi {
    /// Clés choisies : (id du calque, image du plan).
    pub sel: Vec<(u64, usize)>,
    /// La dernière clé cliquée : le départ d'un choix Maj + clic.
    pub anchor: Option<(u64, usize)>,
    pub drag: Option<PosDrag>,
}

/// L'image de clé la plus proche de l'abscisse `px`, à `tol` près au plus (la timeline et le montage s'en
/// servent). `x_of` : l'abscisse du milieu d'une image.
pub(crate) fn marker_near(frames: impl Iterator<Item = usize>, px: f32, x_of: impl Fn(usize) -> f32, tol: f32) -> Option<usize> {
    frames.map(|f| (f, (x_of(f) - px).abs())).filter(|&(_, d)| d <= tol).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(f, _)| f)
}

/// Des clés prises à la souris : les clés du calque telles qu'elles étaient, celles qui bougent, le point
/// pris, le décalage en cours, l'étape d'annulation de tout le geste.
pub(crate) struct PosDrag {
    layer: u64,
    before: Vec<PosKey>,
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
    // ------------------------------------------------------------------ les calques visés

    /// Les calques que les commandes de clés visent : les calques choisis qui bougent ensemble avec
    /// l'outil Transformer (au moins deux), sinon le calque courant s'il peut être transformé.
    fn pos_targets(&self) -> Vec<usize> {
        let peers = self.transform_peers();
        if !peers.is_empty() {
            return peers;
        }
        if self.layer().drawable() && !crate::folders::locked_by_folder(self.layers(), self.cur_layer) {
            vec![self.cur_layer]
        } else {
            Vec::new()
        }
    }

    // ------------------------------------------------------------------ les commandes

    /// Pose une clé de position à l'image courante, avec la pose actuelle du calque.
    pub(crate) fn pos_key_here(&mut self) {
        if self.content_refuses() {
            return;
        }
        let targets = self.pos_targets();
        if targets.is_empty() {
            // (Le message dit pourquoi : verrouillé, position verrouillée, dossier…)
            self.layer_editable();
            return;
        }
        self.end_stroke();
        let f = self.cur_frame;
        self.named_step("Poser une clé de position", |app| app.record_step());
        for &li in &targets {
            let l = &mut self.layers_mut()[li];
            let t = l.pose(f);
            poskeys::set_key(&mut l.pos_keys, f, t);
        }
        let n = targets.len();
        self.modified = true;
        self.status = format!(
            "Clé de position posée à l'image {} sur {} calque{} : change d'image et déplace le calque pour l'animer.",
            self.frame_label(f),
            n,
            s(n)
        );
    }

    /// Supprime les clés de position choisies dans la timeline (quand elle a la main), sinon celles de
    /// l'image courante sur les calques visés.
    pub(crate) fn pos_keys_delete(&mut self) {
        self.pos_keys_delete_with(self.pos_keys_have_focus());
    }

    /// Supprime la clé de position de l'image courante (les calques visés), quel que soit le choix de la
    /// timeline : le bouton des réglages de l'outil Transformer.
    pub(crate) fn pos_keys_delete_here(&mut self) {
        self.pos_keys_delete_with(false);
    }

    fn pos_keys_delete_with(&mut self, chosen: bool) {
        if self.content_refuses() {
            return;
        }
        let f = self.cur_frame;
        let mut list: Vec<(u64, usize)> = if chosen {
            self.pos_ui.sel.iter().copied().filter(|&(id, fr)| self.layers().iter().any(|l| l.id == id && poskeys::key_at(&l.pos_keys, fr).is_some())).collect()
        } else {
            Vec::new()
        };
        if list.is_empty() {
            list = self.pos_targets().into_iter().filter(|&li| poskeys::key_at(&self.layers()[li].pos_keys, f).is_some()).map(|li| (self.layers()[li].id, f)).collect();
        }
        if list.is_empty() {
            self.status = "Pas de clé de position ici : rien n'est supprimé.".into();
            return;
        }
        self.end_stroke();
        self.named_step("Supprimer des clés de position", |app| app.record_step());
        for &(id, fr) in &list {
            let Some(l) = self.layers_mut().iter_mut().find(|l| l.id == id) else { continue };
            let here = l.pose(f);
            poskeys::remove_key(&mut l.pos_keys, fr);
            // La dernière clé partie, le calque garde la pose qu'il avait à cette image.
            if l.pos_keys.is_empty() {
                l.transform = here;
                l.fold_dz();
            }
        }
        self.pos_ui.sel.clear();
        self.renderer.invalidate_all();
        self.modified = true;
        let n = list.len();
        self.status = format!("{n} clé{} de position supprimée{}.", s(n), s(n));
    }

    /// Lisse (ou ne lisse plus) la clé de position de l'image courante : elle arrive et repart lentement.
    pub(crate) fn pos_key_smooth(&mut self) {
        if self.content_refuses() {
            return;
        }
        let f = self.cur_frame;
        let mut targets: Vec<usize> = self.pos_targets();
        targets.retain(|&li| poskeys::key_at(&self.layers()[li].pos_keys, f).is_some());
        if targets.is_empty() {
            self.status = "Pas de clé de position à cette image : pose-en une d'abord.".into();
            return;
        }
        self.end_stroke();
        // Une clé déjà adoucie (lissée ou réglée) ne l'est plus ; sinon elle est lissée.
        let eased = targets.iter().any(|&li| self.layers()[li].pos_keys.iter().any(|k| k.frame == f && k.is_eased()));
        self.named_step(if eased { "Ne plus lisser une clé de position" } else { "Lisser une clé de position" }, |app| app.record_step());
        for &li in &targets {
            poskeys::set_eases(&mut self.layers_mut()[li].pos_keys, f, !eased, 0.0, 0.0);
        }
        self.renderer.invalidate_all();
        self.modified = true;
        self.status = if eased { "Clé de position : vitesse constante.".into() } else { "Clé de position lissée : le calque y arrive et en repart lentement.".into() };
    }

    /// Supprime toutes les clés de position des calques visés : chacun garde la pose qu'il a à l'image
    /// courante.
    pub(crate) fn pos_keys_clear(&mut self) {
        if self.content_refuses() {
            return;
        }
        let targets: Vec<usize> = self.pos_targets().into_iter().filter(|&li| self.layers()[li].is_animated()).collect();
        if targets.is_empty() {
            self.status = "Ce calque n'est pas animé : pas de clé de position.".into();
            return;
        }
        self.end_stroke();
        let f = self.cur_frame;
        self.named_step("Supprimer les clés de position", |app| app.record_step());
        for &li in &targets {
            let l = &mut self.layers_mut()[li];
            l.transform = l.pose(f);
            l.pos_keys.clear();
            l.fold_dz();
        }
        self.pos_ui.sel.clear();
        self.renderer.invalidate_all();
        self.modified = true;
        let n = targets.len();
        self.status = format!("Clés de position supprimées sur {n} calque{} : il garde sa pose de cette image.", s(n));
    }

    // ------------------------------------------------------------------ les réglages de l'outil

    /// Les clés de position dans les réglages de l'outil Transformer : poser, lisser, supprimer, et les
    /// adoucis de la clé de l'image courante.
    pub(crate) fn pos_keys_ui(&mut self, ui: &mut egui::Ui) {
        let (li, f) = (self.cur_layer, self.cur_frame);
        let Some(layer) = self.layers().get(li) else { return };
        let keys = layer.pos_keys.len();
        let here = poskeys::key_at(&layer.pos_keys, f).map(|k| layer.pos_keys[k]);
        ui.separator();
        ui.label(RichText::new("Clés de position").strong());
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::DIAMOND, "Clé de position", "Poser une clé avec la pose du calque à cette image. Change d'image, déplace le calque : il s'anime, et va d'une clé à l'autre.", "calque.cle_position").clicked() {
                self.pos_key_here();
            }
            let (smooth_on, has) = (here.is_some_and(|k| k.is_eased()), here.is_some());
            ui.add_enabled_ui(has, |ui| {
                if self.bouton_etat(ui, smooth_on, ph::WAVE_SINE, "Lisser la clé", "Le calque arrive à cette clé et en repart lentement, au lieu d'aller à vitesse constante.", "calque.cle_position_lisser").clicked() {
                    self.pos_key_smooth();
                }
            });
            if self.bouton_si(ui, has, ph::TRASH, "Supprimer la clé d'ici", "Retire la clé de position de cette image.", "calque.cle_position_supprimer").clicked() {
                self.pos_keys_delete_here();
            }
            if self.bouton_si(ui, keys > 0, ph::ERASER, "Supprimer toutes les clés", "Le calque n'est plus animé : il garde la pose qu'il a à cette image.", "calque.cles_position_supprimer").clicked() {
                self.pos_keys_clear();
            }
        });
        let note = |ui: &mut egui::Ui, text: &str| {
            ui.label(RichText::new(text).small().weak());
        };
        if keys == 0 {
            note(ui, "Aucune clé : le calque garde sa pose fixe. Pose une clé, change d'image, déplace le calque.");
        } else {
            let at = match here {
                Some(_) => "une clé est posée à cette image".to_string(),
                None => "pas de clé à cette image : la déplacer en pose une".to_string(),
            };
            note(ui, &format!("{keys} clé{} de position ; {at}.", s(keys)));
        }
        // Les adoucis de la clé d'ici, en images.
        if let (Some(k), true) = (here, keys >= 2) {
            let (mut ease_in, mut ease_out) = (k.ease_in, k.ease_out);
            let mut changed = false;
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut ease_in).speed(0.2).range(0.0..=60.0).prefix("arrivée ").suffix(" i"));
                changed |= self.edited(&r);
                let r = ui.add(egui::DragValue::new(&mut ease_out).speed(0.2).range(0.0..=60.0).prefix("départ ").suffix(" i"));
                changed |= self.edited(&r);
            });
            if changed {
                let smooth = k.smooth && ease_in == 0.0 && ease_out == 0.0;
                poskeys::set_eases(&mut self.layers_mut()[li].pos_keys, f, smooth, ease_in, ease_out);
                self.renderer.invalidate_all();
            }
            note(ui, "Arrivée, départ : combien d'images le calque met à ralentir en arrivant à cette clé, à accélérer en repartant (0 : vitesse constante).");
        }
    }

    // ------------------------------------------------------------------ la timeline

    /// Des clés de position sont choisies dans la timeline ou le montage et il a la main (`images_focus` :
    /// la timeline ; `key_focus` : une piste de calque du montage) : Suppr les vise.
    pub(crate) fn pos_keys_have_focus(&self) -> bool {
        (self.images_focus || self.key_focus) && self.pos_ui.sel.iter().any(|&(id, fr)| self.layers().iter().any(|l| l.id == id && poskeys::key_at(&l.pos_keys, fr).is_some()))
    }

    /// Les losanges d'un calque : verts, ronds quand la clé est adoucie, cerclés de blanc quand ils sont
    /// choisis. `x_of` : l'abscisse du milieu d'une image ; `y` : leur hauteur ; `visible` : la zone vue.
    pub(crate) fn paint_pos_markers(&self, painter: &Painter, layer: &crate::document::Layer, x_of: impl Fn(usize) -> f32, y: f32, max: usize, visible: (f32, f32)) {
        // (Cachés par le filtre de la légende, `legendecles.rs` : ni dessinés ni saisis.)
        if !self.cle_vue(crate::legendecles::Sorte::Position) {
            return;
        }
        for k in &layer.pos_keys {
            let x = x_of(k.frame);
            if k.frame >= max || x < visible.0 - 8.0 || x > visible.1 + 8.0 {
                continue;
            }
            // Cerclé : choisi dans la timeline, ou posé sous la tête de lecture (la clé « d'ici » des commandes).
            let chosen = self.pos_ui.sel.contains(&(layer.id, k.frame)) || (k.frame == self.cur_frame && self.layers().get(self.cur_layer).is_some_and(|l| l.id == layer.id));
            paint_marker(painter, pos2(x, y), chosen, k.is_eased());
        }
    }

    /// La hauteur des losanges dans une ligne de calque qui commence en `top` et fait `h` de haut.
    pub(crate) fn pos_marker_y(top: f32, h: f32) -> f32 {
        top + h * 0.5
    }

    /// Le losange de position sous le point `p` de la timeline : (calque, image).
    pub(crate) fn pos_marker_in_timeline(&self, p: Pos2, r: Rect, cell: f32, rows: &[crate::layerheads::Row], grid_top: f32) -> Option<(usize, usize)> {
        let row = crate::layerheads::row_at(rows, p.y - grid_top)?;
        let layer = self.layers().get(row.li)?;
        if !self.cle_vue(crate::legendecles::Sorte::Position) || layer.pos_keys.is_empty() || (p.y - (grid_top + Self::pos_marker_y(row.top, row.h))).abs() > HIT_H {
            return None;
        }
        let tol = (cell * 0.5).max(5.0);
        marker_near(layer.pos_keys.iter().map(|k| k.frame), p.x, |f| r.left() + (f as f32 + 0.5) * cell, tol).map(|f| (row.li, f))
    }

    /// Clic sur le losange de l'image `frame` du calque `li` : il est choisi (`toggle` : ajouté ou
    /// retiré), la tête de lecture va dessus et le calque devient courant.
    pub(crate) fn click_pos_marker(&mut self, li: usize, frame: usize, toggle: bool) {
        self.click_pos_marker_with(li, frame, toggle, false);
    }

    /// Comme `click_pos_marker`, avec `range` (Maj + clic) : les clés du calque entre la dernière cliquée et
    /// celle-ci rejoignent le choix.
    pub(crate) fn click_pos_marker_with(&mut self, li: usize, frame: usize, toggle: bool, range: bool) {
        if !self.choose_pos_marker(li, frame, toggle, range) {
            return;
        }
        self.images_focus = true;
        self.panel_focus = false;
        self.key_focus = false;
        self.frame_sel = None;
        self.set_frame_free(frame);
        self.select_layer(li);
        self.say_pos_choice();
    }

    /// Le choix des clés de position : un clic le remplace, Ctrl ajoute ou retire, Maj ajoute celles d'entre.
    /// Faux si le calque n'existe plus.
    fn choose_pos_marker(&mut self, li: usize, frame: usize, toggle: bool, range: bool) -> bool {
        let Some(layer) = self.layers().get(li) else { return false };
        let id = layer.id;
        let k = (id, frame);
        let between = match self.pos_ui.anchor {
            Some((a, from)) if range && a == id => poskeys::frames_between(&layer.pos_keys, from, frame),
            _ => vec![frame],
        };
        if range {
            for f in between {
                if !self.pos_ui.sel.contains(&(id, f)) {
                    self.pos_ui.sel.push((id, f));
                }
            }
            self.pos_ui.anchor.get_or_insert(k);
        } else if toggle {
            match self.pos_ui.sel.iter().position(|x| *x == k) {
                Some(i) => {
                    self.pos_ui.sel.remove(i);
                }
                None => {
                    self.pos_ui.sel.push(k);
                    self.pos_ui.anchor = Some(k);
                }
            }
        } else {
            self.pos_ui.sel = vec![k];
            self.pos_ui.anchor = Some(k);
        }
        true
    }

    fn say_pos_choice(&mut self) {
        let n = self.pos_ui.sel.len();
        self.status = if n == 0 {
            "Plus aucune clé de position choisie.".into()
        } else {
            format!("{n} clé{} de position choisie{} : glisser pour décaler, Suppr pour supprimer ; clic droit : lisser.", s(n), s(n))
        };
    }

    // ------------------------------------------------------------------ le montage

    /// Clic sur le losange de l'image `frame` du calque `li` du plan `pi`, dans une piste du montage : comme
    /// dans la timeline (`click_pos_marker_with`), la tête de lecture va sur le losange (image du film) et
    /// ce plan devient courant. Les clés choisies sont d'un seul plan : prendre un losange d'un autre plan
    /// lâche le choix d'avant.
    pub(crate) fn click_pos_marker_in_montage(&mut self, pi: usize, li: usize, frame: usize, toggle: bool, range: bool) {
        if pi >= self.doc.panels.len() {
            return;
        }
        if pi != self.cur_panel {
            self.set_panel(pi);
            self.pos_ui.sel.clear();
            self.pos_ui.anchor = None;
        }
        if !self.choose_pos_marker(li, frame, toggle, range) {
            return;
        }
        // Les clés de position ont la main : elles remplacent le choix des autres clés (dessins, caméra, effets).
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
        self.say_pos_choice();
    }

    /// Commence à glisser, depuis le montage, les clés choisies en prenant le losange de l'image `frame` du
    /// calque `li` du plan `pi` ; `grab` : l'image du plan sous la souris (même repère que `drag_pos_keys`).
    /// Un losange pris hors du choix devient le choix. Faux si refusé (storyboard verrouillé…).
    pub(crate) fn begin_pos_drag_in_montage(&mut self, pi: usize, li: usize, frame: usize, grab: usize) -> bool {
        if pi >= self.doc.panels.len() {
            return false;
        }
        let id = self.doc.panels[pi].layers.get(li).map(|l| l.id);
        let taken = id.is_some_and(|id| pi == self.cur_panel && self.pos_ui.sel.contains(&(id, frame)));
        if !taken {
            self.click_pos_marker_in_montage(pi, li, frame, false, false);
        }
        self.key_focus = true;
        self.begin_pos_drag(li, frame, grab)
    }

    /// Échap pendant le geste : les clés du calque reviennent comme avant, rien n'est enregistré.
    pub(crate) fn cancel_pos_drag(&mut self) {
        let Some(d) = self.pos_ui.drag.take() else { return };
        for p in &mut self.doc.panels {
            if let Some(l) = p.layers.iter_mut().find(|l| l.id == d.layer) {
                l.pos_keys = d.before.clone();
            }
        }
        self.renderer.invalidate_all();
    }

    // ------------------------------------------------------------------ déplacer

    /// Commence à glisser les clés choisies, en prenant le losange de l'image `frame` du calque `li` ;
    /// `grab` : l'image sous la souris. Un losange pris hors du choix devient le choix.
    pub(crate) fn begin_pos_drag(&mut self, li: usize, frame: usize, grab: usize) -> bool {
        if self.content_refuses() {
            return false;
        }
        let Some(id) = self.layers().get(li).map(|l| l.id) else { return false };
        if !self.pos_ui.sel.contains(&(id, frame)) {
            self.click_pos_marker(li, frame, false);
        }
        let frames: Vec<usize> = self.pos_ui.sel.iter().filter(|(l, _)| *l == id).map(|&(_, f)| f).collect();
        if frames.is_empty() {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let before = self.layers()[li].pos_keys.clone();
        let snap = self.snapshot();
        let max = self.frame_count().saturating_sub(1);
        self.pos_ui.drag = Some(PosDrag { layer: id, before, frames, grab, delta: 0, max, snap });
        true
    }

    /// Les clés prises suivent la souris, à l'image `at` : le calque repart de ses clés d'avant le geste.
    pub(crate) fn drag_pos_keys(&mut self, at: usize) {
        let Some(mut d) = self.pos_ui.drag.take() else { return };
        let delta = poskeys::clamp_delta(&d.frames, at as isize - d.grab as isize, d.max);
        if delta != d.delta {
            d.delta = delta;
            if let Some(l) = self.layers_mut().iter_mut().find(|l| l.id == d.layer) {
                l.pos_keys = poskeys::moved(&d.before, &d.frames, delta, d.max);
            }
            // (La position du calque est dans le plan à composer : aucun pixel n'est renvoyé à la carte graphique.)
            self.renderer.invalidate_params();
            let n = d.frames.len();
            self.status = format!("{n} clé{} de position : {delta:+} image{}.", s(n), if delta.abs() > 1 { "s" } else { "" });
        }
        self.pos_ui.drag = Some(d);
    }

    /// Lâche les clés : une étape d'annulation, « Déplacer des clés de position », si quelque chose a
    /// bougé. Les clés déplacées restent choisies.
    pub(crate) fn end_pos_drag(&mut self) {
        let Some(d) = self.pos_ui.drag.take() else { return };
        if d.delta == 0 {
            return;
        }
        self.pos_ui.sel.retain(|(l, _)| *l != d.layer);
        self.pos_ui.sel.extend(d.frames.iter().map(|&f| (d.layer, (f as isize + d.delta).max(0) as usize)));
        self.modified = true;
        self.commit_named(d.snap, "Déplacer des clés de position");
    }

    // ------------------------------------------------------------------ le menu du clic droit

    /// Le menu du clic droit sur un losange de position.
    pub(crate) fn pos_key_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Lisser la clé de position", "calque.cle_position_lisser");
        self.action_item(ui, "Supprimer la clé de position", "calque.cle_position_supprimer");
        ui.separator();
        self.action_item(ui, "Clé de position ici (pose actuelle)", "calque.cle_position");
        self.action_item(ui, "Supprimer toutes les clés du calque", "calque.cles_position_supprimer");
    }
}

/// Un losange vert (la couleur des poignées du calque), cerclé de noir ; adouci : un rond ; choisi :
/// plus grand et cerclé de blanc.
fn paint_marker(painter: &Painter, c: Pos2, chosen: bool, eased: bool) {
    // Assez grands pour se voir sur les blocs de dessins (bleus ou verts) de la timeline et du montage.
    let d = if chosen { 7.0 } else { 5.5 };
    let (fill, edge) = (crate::theme::POIGNEE_CALQUE, if chosen { Color32::WHITE } else { Color32::from_black_alpha(190) });
    let stroke = Stroke::new(if chosen { 1.8_f32 } else { 1.2_f32 }, edge);
    if eased {
        painter.circle(c, d * 0.9, fill, stroke);
    } else {
        let pts = vec![pos2(c.x, c.y - d), pos2(c.x + d, c.y), pos2(c.x, c.y + d), pos2(c.x - d, c.y)];
        painter.add(Shape::convex_polygon(pts, fill, stroke));
    }
}
