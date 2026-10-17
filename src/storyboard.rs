//! Niveau storyboard : les plans montés bout à bout, leurs vignettes, leur durée,
//! leurs indications (action, dialogue, notes) et la lecture de tout le montage.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
};

use crate::app::{App, BoardView, Mode};
use crate::document::{Document, Panel};
use crate::handles::Handle;
use crate::history::Edit;
use crate::soundtrack::{ClipDrag, ClipEdge, MenuTarget};
use crate::theme;
use crate::timeline::CUR_FRAME;

pub(crate) enum MontageDrag {
    /// Bord droit d'un plan tiré : on change sa durée. Les calques sont recalculés depuis
    /// l'original à chaque mouvement, rien n'est perdu tant qu'on n'a pas relâché.
    Trim {
        panel: usize,
        original: Panel,
        start_len: usize,
        origin_x: f32,
        snap: Edit,
    },
    /// Plan déplacé dans le montage ; `target` est la place où il sera inséré.
    Move { panel: usize, target: usize },
    Scrub,
    /// Extrait de son déplacé, ou raccourci par un bord.
    Clip(ClipDrag),
}

pub(crate) const RULER_H: f32 = 18.0;
const SEQ_H: f32 = 17.0;
pub(crate) const BANDS_H: f32 = SEQ_H + 2.0;

pub fn timecode(frames: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    let s = frames / fps;
    format!("{}:{:02}:{:02}", s / 60, s % 60, frames % fps)
}

pub fn duration_label(frames: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    if frames >= fps {
        format!("{}s {:02}", frames / fps, frames % fps)
    } else {
        format!("{frames} i")
    }
}

/// « Plan 3 » : le numéro (ou le nom) du plan, pour les vignettes et les barres (13a).
pub(crate) fn panel_title(doc: &Document, index: usize) -> String {
    format!("Plan {}", doc.scene_label(doc.panels[index].scene))
}

/// « Séquence 1 — plan 3 » : pour la fiche et la barre d'état.
pub fn panel_long_title(doc: &Document, index: usize) -> String {
    let seq = doc.sequence_of(index);
    format!("Séquence {} — plan {}", doc.sequence_label(seq), doc.scene_label(doc.panels[index].scene))
}

impl App {
    // ------------------------------------------------------------------ opérations sur les plans

    /// Applique une opération de montage du modèle (avec annulation) et rend courant le plan
    /// qu'elle désigne. `None` = opération refusée.
    pub(crate) fn structure_op(&mut self, op: impl FnOnce(&mut Document, usize) -> Option<usize>) -> bool {
        if self.storyboard_refuses() {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let before = self.panel().id;
        let board_before = self.board_layout();
        let Some(index) = op(&mut self.doc, self.cur_panel) else {
            return false;
        };
        // Les notes attachées à une image qui n'est plus là restent sur la planche, libres.
        self.release_removed_notes(&board_before);
        if self.doc.panels[index].id != before {
            self.cur_frame = 0;
        }
        self.cur_panel = index;
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
        self.vsel = None;
        self.follow_frame = true;
        self.commit_structure(snap);
        true
    }

    /// Nouveau plan juste après le plan courant, dans la même séquence.
    pub(crate) fn new_panel(&mut self) {
        if self.structure_op(|d, i| Some(d.add_scene_after(i))) {
            self.status = format!("Nouveau {}.", panel_title(&self.doc, self.cur_panel).to_lowercase());
        }
    }

    /// Nouvelle séquence (avec un plan) après la séquence courante.
    pub(crate) fn new_sequence(&mut self) {
        if self.structure_op(|d, i| Some(d.add_sequence_after(i))) {
            let seq = self.doc.sequence_of(self.cur_panel);
            self.status = format!("Nouvelle séquence {}.", self.doc.sequence_label(seq));
        }
    }

    /// Copie du plan courant, juste après lui.
    pub(crate) fn duplicate_panel(&mut self) {
        self.structure_op(|d, i| Some(d.duplicate_scene_after(i)));
    }

    // ------------------------------------------------------------------ plans choisis (12e)

    /// Le montage (ou la planche) a la main : copier, coller et supprimer y visent les plans
    /// choisis. Dans les deux modes depuis que le montage se voit aussi en animation (chantier 6) ;
    /// un clic dans la timeline ou dans le dessin lui reprend la main.
    pub(crate) fn panels_have_focus(&self) -> bool {
        self.panel_focus
    }

    /// Copie les plans choisis (le plan courant en fait toujours partie).
    pub(crate) fn copy_panels(&mut self) {
        let ids = self.board_selection();
        let Some(clip) = self.doc.copy_panels(&ids) else { return };
        self.status = match clip.panels.len() {
            1 => format!("{} copié.", panel_title(&self.doc, self.cur_panel)),
            n => format!("{n} plans copiés."),
        };
        self.panel_clipboard = Some(clip);
    }

    /// Copie les plans choisis, puis les retire.
    pub(crate) fn cut_panels(&mut self) {
        self.copy_panels();
        let ids = self.board_selection();
        self.delete_panels(&ids);
    }

    /// Colle les plans copiés juste après le plan courant ; ils deviennent les plans choisis.
    pub(crate) fn paste_panels(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        let Some(clip) = self.panel_clipboard.take() else {
            self.status = "Aucun plan copié.".into();
            return;
        };
        let n = clip.panels.len();
        let after = panel_title(&self.doc, self.cur_panel);
        if self.structure_op(|d, i| d.paste_plans(&clip, i)) {
            let end = (self.cur_panel + n).min(self.doc.panels.len());
            self.board_sel = self.doc.panels[self.cur_panel..end].iter().map(|p| p.id).collect();
            self.status = match n {
                1 => format!("Plan collé après le {}.", after.to_lowercase()),
                n => format!("{n} plans collés après le {}.", after.to_lowercase()),
            };
        } else {
            self.status = "Ces plans viennent d'un projet d'une autre taille : ils ne se collent pas ici.".into();
        }
        self.panel_clipboard = Some(clip);
    }

    /// Supprime les plans choisis (une étape d'annulation).
    pub(crate) fn delete_chosen_panels(&mut self) {
        let ids = self.board_selection();
        self.delete_panels(&ids);
    }

    /// Déplace les plans `ids` à la place `slot` de la bande (voir `Document::move_plans`) ; le
    /// plan courant reste courant.
    fn move_chosen_panels(&mut self, ids: &[u64], slot: usize) {
        let before = self.doc.panels.get(slot).map(|p| p.id);
        let current = self.panel().id;
        let ids = ids.to_vec();
        self.structure_op(|d, _| {
            d.move_plans(&ids, before)?;
            d.panels.iter().position(|p| p.id == current)
        });
    }

    /// Déplace le plan `from` à la place `slot` (0 = au début, nombre de plans = à la fin).
    pub(crate) fn move_panel(&mut self, from: usize, slot: usize) {
        let id = self.doc.panels[from].id;
        self.move_chosen_panels(&[id], slot);
    }

    pub(crate) fn shift_panel(&mut self, later: bool) {
        let i = self.cur_panel;
        if later && i + 1 < self.doc.panels.len() {
            self.move_panel(i, i + 2);
        } else if !later && i > 0 {
            self.move_panel(i, i - 1);
        }
    }

    /// Coupe le plan courant à l'image courante (F5) : la suite devient un nouveau plan.
    pub(crate) fn split_current_panel(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        let at = self.cur_frame;
        if self.structure_op(|d, i| d.split_plan(i, at)) {
            self.status = format!("Plan coupé : la suite est le {}.", panel_title(&self.doc, self.cur_panel).to_lowercase());
        } else {
            self.status = "Place la tête de lecture dans le plan, après sa première image, pour le couper.".into();
        }
    }

    /// Réunit le plan courant et le suivant (de la même séquence) : le début du suivant devient
    /// une marque d'image.
    pub(crate) fn merge_next_panel(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        if self.structure_op(|d, i| d.join_next_plan(i)) {
            self.status = format!("Plans réunis : {}.", panel_title(&self.doc, self.cur_panel));
        } else {
            self.status = "Pas de plan suivant dans cette séquence.".into();
        }
    }

    /// Réunit le plan précédent (de la même séquence) et le plan courant.
    pub(crate) fn join_previous_scene(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        if self.structure_op(|d, i| d.join_next_plan(i.checked_sub(1)?)) {
            self.status = format!("Plans réunis : {}.", panel_title(&self.doc, self.cur_panel));
        } else {
            self.status = "Pas de plan précédent dans cette séquence.".into();
        }
    }

    /// Pose ou retire une marque d'image à l'image courante du plan (une image de storyboard de
    /// plus ou de moins dans les vignettes et le PDF).
    pub(crate) fn toggle_mark(&mut self) {
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let mut placed = None;
        let done = self.structure_op(|d, i| {
            placed = d.toggle_mark(i, frame);
            placed.map(|_| i)
        });
        self.cur_frame = frame;
        self.status = match (done, placed) {
            (true, Some(true)) => format!("Marque d'image à l'image {} : une image de storyboard de plus.", frame + 1),
            (true, _) => format!("Marque d'image retirée de l'image {}.", frame + 1),
            _ => "La première image est déjà le début du plan : place-toi plus loin pour poser une marque.".into(),
        };
    }

    /// Plan précédent (ou suivant).
    pub(crate) fn step_scene(&mut self, forward: bool) {
        let target = if forward {
            (self.cur_panel + 1 < self.doc.panels.len()).then_some(self.cur_panel + 1)
        } else {
            self.cur_panel.checked_sub(1)
        };
        if let Some(i) = target {
            self.select_panel(i);
        }
    }
    /// Durée du plan courant. Chaque valeur repart du plan tel qu'il était au début du geste
    /// (raccourcir puis rallonger ne perd rien), avec une seule étape d'annulation par geste.
    pub(crate) fn edit_panel_length(&mut self, n: usize) {
        if self.storyboard_refuses() {
            return;
        }
        self.length_field_active = true;
        let id = self.panel().id;
        if self.length_origin.as_ref().is_none_or(|s| s.id != id) {
            self.end_stroke();
            let snap = self.snapshot();
            self.history.push(snap);
            self.length_origin = Some(self.panel().clone());
        }
        let Some(origin) = &self.length_origin else { return };
        let mut panel = origin.clone();
        panel.set_length(n);
        *self.panel_mut() = panel;
        self.vsel = None;
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Champ de saisie de la durée du plan courant (fiche du plan, timeline).
    pub(crate) fn panel_length_field(&mut self, ui: &mut egui::Ui, prefix: &str) -> egui::Response {
        let mut len = self.frame_count();
        let resp = ui.add(
            egui::DragValue::new(&mut len)
                .range(1..=100_000)
                .prefix(prefix)
                .suffix(" i")
                .update_while_editing(false),
        );
        if resp.dragged() || resp.has_focus() {
            self.length_field_active = true;
        }
        if resp.changed() {
            self.edit_panel_length(len);
        }
        resp
    }

    /// Fin du geste sur la durée quand plus aucun champ de durée n'est utilisé.
    pub(crate) fn end_length_gesture_if_idle(&mut self) {
        if !std::mem::take(&mut self.length_field_active) {
            self.length_origin = None;
        }
    }

    /// Abandonne un glissé en cours dans la bande de montage (la durée revient à l'original).
    pub(crate) fn cancel_montage_drag(&mut self) {
        self.cancel_camera_key_drag();
        self.cancel_handle_drag();
        match self.montage_drag.take() {
            Some(MontageDrag::Trim { panel, original, .. }) => {
                if self.doc.panels.get(panel).is_some_and(|s| s.id == original.id) {
                    self.doc.panels[panel] = original;
                    self.renderer.invalidate_all();
                }
            }
            Some(MontageDrag::Clip(d)) => self.doc.audio_tracks = d.before,
            _ => {}
        }
    }

    // ------------------------------------------------------------------ panneau du montage

    pub(crate) fn montage_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        let total = self.doc.total_frames();
        let fps = self.doc.fps;
        // Une seule ligne, comme la barre de la timeline (12c) : elle défile si la fenêtre est étroite.
        egui::ScrollArea::horizontal()
            .id_salt("barre_montage")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| ui.horizontal(|ui| self.montage_toolbar(ui, total, fps)));
        ui.separator();
        // Hauteur des panneaux : ce qui reste une fois la règle, les barres et les pistes placées.
        let rows = self.montage_rows(ui.available_height());
        egui::ScrollArea::vertical()
            .id_salt("montage_v")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let headers = self.track_headers(ui, rows.lanes_top());
                    self.row_headers(ui, headers, &rows);
                    egui::ScrollArea::horizontal()
                        .id_salt("montage_h")
                        .auto_shrink([false, true])
                        .show(ui, |ui| self.montage_strip(ui, &rows));
                });
            });
    }

    fn montage_toolbar(&mut self, ui: &mut egui::Ui, total: usize, fps: u32) {
        use crate::bulles::{bulle, etiquette, TAILLE};
        use egui_phosphor::regular as ph;
        ui.spacing_mut().item_spacing.x = 3.0;
        let sep = |ui: &mut egui::Ui| {
            ui.add_space(3.0);
            ui.separator();
            ui.add_space(3.0);
        };

        // Lecture.
        if self.bouton(ui, ph::SKIP_BACK, "Début du montage", "Revenir à la première image du film.", "image.premiere").clicked() {
            self.goto_global(0);
        }
        let play = if self.playing { ph::PAUSE } else { ph::PLAY };
        if self.bouton(ui, play, "Lire le montage / pause", "Tout le film à la suite (animatique), avec le son.", "lecture.basculer").clicked() {
            self.toggle_play_scope(true);
        }
        let key = self.raccourci("lecture.boucle");
        crate::bulles::interrupteur_a(ui, &mut self.looping, ph::REPEAT, "Lecture en boucle", "Reprendre au début une fois à la fin.", &key, "lecture.boucle");
        let resp = ui.label(RichText::new(format!("{} / {}", timecode(self.global_frame(), fps), timecode(total, fps))).monospace());
        bulle(resp, "Position", "Image courante / durée du montage (minutes:secondes:images).", "");
        sep(ui);

        // Plans et séquences.
        if self.bouton(ui, ph::PLUS_SQUARE, "Nouveau plan", "Après le plan courant, dans la même séquence.", "panneau.nouveau").clicked() {
            self.new_panel();
        }
        if self.bouton(ui, ph::STACK_PLUS, "Nouvelle séquence", "Après la séquence courante, avec un plan.", "sequence.nouvelle").clicked() {
            self.new_sequence();
        }
        if self
            .bouton(ui, ph::BEZIER_CURVE, "Nouveau calque", "Un calque vectoriel dans le plan courant, au-dessus du calque courant : une piste de plus.", "calque.nouveau_vectoriel")
            .clicked()
        {
            self.add_layer(crate::document::LayerKind::Vector);
        }
        // Plans choisis (Ctrl ou Maj + clic dans la bande) : copier, coller, supprimer (12e).
        let n = self.board_selection().len();
        let chosen = if n > 1 { format!("Les {n} plans choisis.") } else { "Le plan courant ; Ctrl ou Maj + clic pour en choisir plusieurs.".to_string() };
        if self.bouton(ui, ph::COPY, "Copier les plans", &chosen, "edition.copier").clicked() {
            self.copy_panels();
        }
        let can_paste = self.panel_clipboard.is_some();
        if self.bouton_si(ui, can_paste, ph::CLIPBOARD, "Coller les plans", "Après le plan courant, dans sa séquence.", "edition.coller").clicked() {
            self.paste_panels();
        }
        if self.bouton(ui, ph::TRASH, "Supprimer les plans", &chosen, "edition.supprimer").clicked() {
            self.delete_chosen_panels();
        }
        let resp = ui
            .menu_button(RichText::new(ph::DOTS_THREE).size(TAILLE), |ui| {
                let item = |app: &mut App, ui: &mut egui::Ui, text: &str, action: &str| {
                    let key = app.keymap.label(action);
                    if ui.add(egui::Button::new(text).shortcut_text(key)).clicked() {
                        ui.close_menu();
                        app.run_action(action);
                    }
                };
                item(self, ui, "Nouveau plan avant le plan courant", "plan.nouveau_avant");
                item(self, ui, "Dupliquer le plan", "panneau.dupliquer");
                ui.separator();
                self.panel_clipboard_items(ui);
                ui.separator();
                item(self, ui, "Couper le plan à l'image courante", "panneau.couper");
                item(self, ui, "Réunir le plan et le suivant", "panneau.fusionner_suivant");
                item(self, ui, "Réunir avec le plan précédent", "scene.joindre_precedente");
                ui.separator();
                item(self, ui, "Marque d'image ici (ajouter / retirer)", "marque.basculer");
            })
            .response;
        bulle(resp, "Plans", "Dupliquer, copier, couper, réunir, marques d'image.", "");
        if self.bouton(ui, ph::CARET_DOUBLE_LEFT, "Avancer le plan", "Le placer plus tôt dans le montage.", "plan.avancer").clicked() {
            self.shift_panel(false);
        }
        if self.bouton(ui, ph::CARET_DOUBLE_RIGHT, "Reculer le plan", "Le placer plus tard dans le montage.", "plan.reculer").clicked() {
            self.shift_panel(true);
        }
        let locked = self.doc.storyboard_locked;
        let icon = if locked { ph::LOCK } else { ph::LOCK_OPEN };
        let what = "Verrouillé : l'ordre, le nombre et la durée des plans, leurs dessins et leurs calques ne changent plus.";
        if self.bouton_etat(ui, locked, icon, "Verrouiller le storyboard", what, "storyboard.verrouiller").clicked() {
            self.toggle_storyboard_lock();
        }
        sep(ui);

        // Son.
        if self
            .bouton(
                ui,
                ph::MUSIC_NOTES_PLUS,
                "Importer un son",
                "WAV, MP3, FLAC ou OGG, à l'image courante ; on peut aussi lâcher le fichier sur la fenêtre.",
                "son.importer",
            )
            .clicked()
        {
            self.import_sound(None, None);
        }
        let resp = ui.menu_button(RichText::new(ph::SPEAKER_HIGH).size(TAILLE), |ui| self.sound_menu(ui)).response;
        bulle(resp, "Son", "Pistes son, son au défilement et pendant la lecture.", "");
        sep(ui);

        // Le montage se voit aussi en animation (chantier 6) : le bouton fait alors revenir.
        let (icon, name, what) = match self.mode {
            Mode::Storyboard => (ph::FILM_SLATE, "Animer ce plan", "Entrer dans le plan courant pour l'animer (ou double-clic sur le plan)."),
            Mode::Animation => (ph::ARROW_U_UP_LEFT, "Retour au storyboard", "Le dessin montre de nouveau le storyboard (Vue dessin ou vignettes)."),
        };
        if self.bouton(ui, icon, name, what, "mode.basculer").clicked() {
            match self.mode {
                Mode::Storyboard => self.enter_animation(self.cur_panel),
                Mode::Animation => self.set_mode(Mode::Storyboard),
            }
        }
        sep(ui);

        // Vue : dessin du plan, ou vignettes de toutes les images de storyboard.
        let key = self.raccourci("vue.planches");
        crate::bulles::choix_a(ui, &mut self.board_view, BoardView::Scene, ph::PAINT_BRUSH, "Vue dessin", "Dessiner le plan courant.", &key, "vue.planches");
        crate::bulles::choix_a(ui, &mut self.board_view, BoardView::Panels, ph::SQUARES_FOUR, "Vue vignettes", "Toutes les images de storyboard en planche : le début de chaque plan, puis ses marques d'image.", &key, "vue.planches");
        sep(ui);
        etiquette(ui, ph::MAGNIFYING_GLASS, "Zoom du montage", "");
        ui.spacing_mut().slider_width = 80.0;
        let resp = ui.add(egui::Slider::new(&mut self.montage_zoom, 0.5..=16.0).logarithmic(true).show_value(false));
        bulle(resp, "Zoom du montage", "Largeur d'une image dans la bande.", "");
    }

    /// Menu « Son… » de la barre du montage.
    pub(crate) fn sound_menu(&mut self, ui: &mut egui::Ui) {
        let item = |app: &mut App, ui: &mut egui::Ui, text: &str, action: &str| {
            let key = app.keymap.label(action);
            if ui.add(egui::Button::new(text).shortcut_text(key)).clicked() {
                ui.close_menu();
                app.run_action(action);
            }
        };
        item(self, ui, "Importer un son…", "son.importer");
        item(self, ui, "Nouvelle piste son", "son.nouvelle_piste");
        item(self, ui, "Couper le son à l'image courante", "son.couper");
        if let Some(id) = self.sel_clip.filter(|&id| self.doc.clip(id).is_some()) {
            if ui.button("Supprimer l'extrait sélectionné").clicked() {
                ui.close_menu();
                self.delete_clip(id);
            }
        }
        ui.separator();
        let on = self.sound_on;
        if ui.checkbox(&mut self.sound_on, "Son pendant la lecture").changed() {
            self.sound_on = on;
            self.run_action("son.lecture");
        }
        let scrub = self.sound_scrub;
        if ui.checkbox(&mut self.sound_scrub, "Son au défilement").changed() {
            self.sound_scrub = scrub;
            self.run_action("son.defilement");
        }
    }

    /// Copier, coller, supprimer les panneaux choisis : entrées des menus du montage et des
    /// vignettes (12e). Les raccourcis montrés sont ceux qui valent quand le montage a la main.
    pub(crate) fn panel_clipboard_items(&mut self, ui: &mut egui::Ui) {
        let n = self.board_selection().len();
        let what = |verb: &str| if n > 1 { format!("{verb} les {n} plans choisis") } else { format!("{verb} le plan") };
        let copy = self.raccourci("edition.copier");
        if ui.add(egui::Button::new(what("Copier")).shortcut_text(copy)).clicked() {
            ui.close_menu();
            self.copy_panels();
        }
        let paste = self.raccourci("edition.coller");
        let label = match self.panel_clipboard.as_ref().map(|c| c.panels.len()) {
            Some(k) if k > 1 => format!("Coller {k} plans après le plan courant"),
            _ => "Coller après le plan courant".to_string(),
        };
        if ui.add_enabled(self.panel_clipboard.is_some(), egui::Button::new(label).shortcut_text(paste)).clicked() {
            ui.close_menu();
            self.paste_panels();
        }
        let delete = self.raccourci("edition.supprimer");
        if ui.add(egui::Button::new(what("Supprimer")).shortcut_text(delete)).clicked() {
            ui.close_menu();
            self.delete_chosen_panels();
        }
    }

    /// Menu du clic droit sur un plan de la bande de montage.
    fn panel_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        if ui.button("Animer ce plan").clicked() {
            ui.close_menu();
            self.enter_animation(index);
        }
        if ui.button("Nouveau plan après").clicked() {
            ui.close_menu();
            self.new_panel();
        }
        if ui.button("Dupliquer le plan").clicked() {
            ui.close_menu();
            self.duplicate_panel();
        }
        ui.separator();
        self.panel_clipboard_items(ui);
        ui.separator();
        if ui.button("Couper le plan à l'image courante").clicked() {
            ui.close_menu();
            self.split_current_panel();
        }
        if ui.button("Réunir avec le plan suivant").clicked() {
            ui.close_menu();
            self.merge_next_panel();
        }
        ui.separator();
        if ui.button("Importer un son au début du plan…").clicked() {
            ui.close_menu();
            let at = self.doc.panel_start(index.min(self.doc.panels.len() - 1));
            self.import_sound(None, Some(at));
        }
    }

    fn montage_strip(&mut self, ui: &mut egui::Ui, rows: &crate::montagerows::Rows) {
        let block_h = rows.block_h;
        let ppf = self.montage_zoom;
        let fps = self.doc.fps.max(1) as usize;
        let total = self.doc.total_frames();
        // Le son peut dépasser la fin des plans : on le montre en entier.
        let shown = total.max(self.doc.audio_end());
        let n_tracks = self.doc.audio_tracks.len();
        if self.sel_clip.is_some_and(|id| self.doc.clip(id).is_none()) {
            self.sel_clip = None;
        }
        let width = (shown as f32 * ppf + 260.0).max(ui.available_width());
        let height = rows.height(n_tracks);
        let (resp, painter) = ui.allocate_painter(vec2(width, height), Sense::click_and_drag());
        let r = resp.rect;
        let left = r.left() + 6.0;
        let x_of = |g: usize| left + g as f32 * ppf;
        let bands_top = r.top() + RULER_H;
        let top = r.top() + rows.blocks_top();
        let lanes_top = r.top() + rows.lanes_top();
        let in_blocks = |p: Pos2| p.y >= top && p.y <= top + block_h;
        let visuals = ui.visuals().clone();
        // Un clic dans la bande lui donne la main : copier, coller, supprimer visent ses panneaux.
        if resp.is_pointer_button_down_on() {
            self.panel_focus = true;
            self.images_focus = false;
        }

        // Règle : une graduation par seconde, un repère lisible tous les ~60 pixels.
        painter.rect_filled(Rect::from_min_max(r.min, pos2(r.right(), bands_top)), 0.0, theme::REGLE);
        let label_every = ((60.0 / (fps as f32 * ppf)).ceil() as usize).max(1);
        for s in 0..=(shown / fps + 2) {
            let x = x_of(s * fps);
            let major = s % label_every == 0;
            painter.line_segment(
                [pos2(x, r.top() + if major { 3.0 } else { 10.0 }), pos2(x, bands_top)],
                Stroke::new(1.0_f32, theme::GRADUATION_SECONDE),
            );
            if major {
                painter.text(
                    pos2(x + 3.0, r.top() + 2.0),
                    Align2::LEFT_TOP,
                    format!("{}:{:02}", s / 60, s % 60),
                    FontId::monospace(10.0),
                    visuals.weak_text_color(),
                );
            }
        }

        // Plans.
        let deadline = std::time::Instant::now() + crate::thumbs::THUMB_TIME;
        let thumb_h = (block_h - 24.0).max(10.0);
        let aspect = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let mut blocks: Vec<Rect> = Vec::with_capacity(self.doc.panels.len());
        let mut start = 0;
        for i in 0..self.doc.panels.len() {
            let len = self.doc.panels[i].frame_count();
            let rect = Rect::from_min_max(
                pos2(x_of(start) + 1.0, top + 2.0),
                pos2(x_of(start + len) - 1.0, top + block_h),
            );
            blocks.push(rect);
            start += len;
            let selected = i == self.cur_panel;
            // Choisi avec lui (Ctrl ou Maj + clic) : un ton d'accent plus léger.
            let chosen = !selected && self.board_sel.contains(&self.doc.panels[i].id);
            let bg = if selected {
                visuals.selection.bg_fill.gamma_multiply(0.55)
            } else if chosen {
                visuals.selection.bg_fill.gamma_multiply(0.3)
            } else {
                theme::BLOC
            };
            painter.rect_filled(rect, 4.0, bg);
            let clip = painter.with_clip_rect(rect.shrink(2.0).intersect(r));
            if let Some(tex) = self.thumbs.texture(ui.ctx(), &self.doc, i, crate::thumbs::STRIP_WIDTH, deadline) {
                let tr = Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(thumb_h * aspect, thumb_h));
                clip.image(
                    tex,
                    tr,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let title = clip.layout_no_wrap(panel_title(&self.doc, i), FontId::proportional(12.0), visuals.text_color());
            let duration = clip.layout_no_wrap(
                duration_label(len, self.doc.fps),
                FontId::monospace(10.0),
                visuals.weak_text_color(),
            );
            // La durée n'est affichée que si elle tient à côté du titre.
            if title.size().x + duration.size().x + 20.0 < rect.width() {
                let at = pos2(rect.right() - 7.0 - duration.size().x, rect.bottom() - 4.0 - duration.size().y);
                clip.galley(at, duration, visuals.weak_text_color());
            }
            let at = pos2(rect.left() + 5.0, rect.bottom() - 4.0 - title.size().y);
            clip.galley(at, title, visuals.text_color());
            // Marques d'image (13a) : chacune, une image de storyboard de plus dans le plan.
            for m in self.doc.panels[i].marks.iter().filter(|m| m.frame < len) {
                let x = rect.left() - 1.0 + m.frame as f32 * ppf;
                clip.line_segment([pos2(x, rect.top() + 2.0), pos2(x, rect.bottom() - 2.0)], Stroke::new(1.0_f32, theme::MARQUE));
                clip.add(egui::Shape::convex_polygon(
                    vec![pos2(x - 4.0, rect.top() + 2.0), pos2(x + 4.0, rect.top() + 2.0), pos2(x, rect.top() + 8.0)],
                    theme::MARQUE,
                    Stroke::NONE,
                ));
            }
            if selected || chosen {
                painter.rect_stroke(
                    rect,
                    4.0,
                    Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, visuals.selection.stroke.color),
                    StrokeKind::Inside,
                );
            }
            // Poignée de durée sur le bord droit.
            painter.rect_filled(
                Rect::from_min_max(pos2(rect.right() - 4.0, rect.top() + 10.0), pos2(rect.right() - 1.0, rect.bottom() - 10.0)),
                1.0,
                theme::POIGNEE,
            );
        }

        // Barre des séquences au-dessus des plans (13a : un plan = un bloc, plus de barre de scène).
        let mut runs: Vec<(Rect, usize)> = Vec::new();
        for (band, y0, h) in [(0, bands_top, SEQ_H)] {
            let mut i = 0;
            while i < self.doc.panels.len() {
                let (start, end) = if band == 0 {
                    self.doc.sequence_run(i)
                } else {
                    self.doc.scene_run(i)
                };
                let rect = Rect::from_min_max(
                    pos2(blocks[start].left(), y0 + 1.0),
                    pos2(blocks[end - 1].right(), y0 + h - 1.0),
                );
                let (label, number) = if band == 0 {
                    let seq = self.doc.sequence_of(start);
                    (format!("Séquence {}", self.doc.sequence_label(seq)), self.doc.sequence_number(seq))
                } else {
                    let scene = self.doc.panels[start].scene;
                    (format!("Scène {}", self.doc.scene_label(scene)), self.doc.scene_number(scene))
                };
                let current = (start..end).contains(&self.cur_panel);
                let fill = if current {
                    visuals.selection.bg_fill.gamma_multiply(if band == 0 { 0.45 } else { 0.7 })
                } else {
                    theme::barre(band == 0, number % 2 == 1)
                };
                painter.rect_filled(rect, 3.0, fill);
                painter.with_clip_rect(rect.shrink(1.0)).text(
                    pos2(rect.left() + 5.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(if band == 0 { 10.0 } else { 11.0 }),
                    visuals.text_color(),
                );
                runs.push((rect, start));
                i = end;
            }
        }

        // Caméra au-dessus des plans, calques dessous (6f), puis les pistes son. La poignée de
        // calque tenue (sur le plan courant) ou survolée s'éclaire (chantier 5).
        let lit = match &self.handle_drag {
            Some(d) => Some((self.cur_panel, d.handle)),
            None if self.montage_drag.is_none() && self.cam_key_drag.is_none() => resp
                .hover_pos()
                .and_then(|p| self.montage_layer_handle_at(p, r.top(), &blocks, ppf, rows))
                .map(|(i, _, h)| (i, h)),
            None => None,
        };
        self.paint_track_rows(&painter, r, &blocks, ppf, rows, &visuals, lit);
        self.paint_lanes(&painter, r, left, ppf, lanes_top, visuals.selection.stroke.color);

        let panel_at = |x: f32| blocks.iter().position(|b| x >= b.left() - 1.0 && x <= b.right() + 1.0);
        let edge_at = |p: Pos2| {
            if !in_blocks(p) {
                return None;
            }
            blocks.iter().position(|b| (p.x - b.right()).abs() <= 5.0)
        };
        let frame_at = |x: f32| (((x - left) / ppf).floor().max(0.0) as usize).min(total - 1);
        // Marque d'image sous le point (son triangle en haut du plan) : (plan, rang de la marque).
        let mark_at = |app: &App, p: Pos2| {
            let i = blocks.iter().position(|b| p.x >= b.left() - 5.0 && p.x <= b.right() + 5.0 && p.y >= b.top() && p.y <= b.top() + 12.0)?;
            let b = blocks[i];
            let m = app.doc.panels[i].marks.iter().position(|m| (p.x - (b.left() - 1.0 + m.frame as f32 * ppf)).abs() <= 5.0)?;
            Some((i, m))
        };
        // Sans limite à la fin des plans (on peut poser un son plus loin).
        let free_frame_at = |x: f32| ((x - left) / ppf).floor().max(0.0) as usize;

        if let Some(p) = resp.hover_pos() {
            let clip_edge = matches!(
                self.lane_hit(p, left, ppf, lanes_top),
                Some((_, Some((_, ClipEdge::Left | ClipEdge::Right))))
            );
            let resizing = matches!(
                &self.montage_drag,
                Some(MontageDrag::Trim { .. })
            ) || matches!(&self.montage_drag, Some(MontageDrag::Clip(d)) if d.edge != ClipEdge::Move);
            if edge_at(p).is_some() || clip_edge || resizing {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
            // Poignées des pistes de calques (chantier 5) : les bords s'étirent, la barre se prend.
            match lit.filter(|_| self.handle_drag.is_none()).map(|(_, h)| h) {
                Some(Handle::LayerStart { .. } | Handle::LayerEnd { .. } | Handle::Exposure { .. }) => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                }
                Some(Handle::LayerMove { .. } | Handle::Key { .. }) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                _ => {}
            }
        }

        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(r.center());
            // Sans recentrer la bande : le bord tiré doit rester sous le pointeur.
            self.playing = false;
            if let Some((i, k)) = self.montage_camera_key_at(origin, r.top(), &blocks, ppf, rows) {
                self.begin_camera_key_drag(i, k);
            } else if let Some((_, Some((id, edge)))) = self.lane_hit(origin, left, ppf, lanes_top) {
                self.begin_clip_drag(id, edge, origin);
            } else if let Some((i, m)) = mark_at(self, origin) {
                // Une marque d'image se prend aussi dans le montage (passe interface).
                self.set_panel(i);
                self.begin_handle_drag(crate::handles::Handle::Mark(m));
            } else if let Some((i, k, h)) = self.montage_layer_handle_at(origin, r.top(), &blocks, ppf, rows) {
                // Les gestes de la timeline dans les pistes de calques (chantier 5) : ce plan, ce
                // calque, et sa poignée.
                self.sel_clip = None;
                self.set_panel(i);
                self.select_layer(k);
                self.begin_handle_drag(h);
            } else {
                let drag = if !in_blocks(origin) {
                    MontageDrag::Scrub
                } else if self.doc.storyboard_locked && (edge_at(origin).is_some() || panel_at(origin.x).is_some()) {
                    // Storyboard verrouillé (13e) : ni durée ni place des plans ne bougent.
                    self.status = crate::projetui::STORYBOARD_LOCKED.into();
                    MontageDrag::Scrub
                } else if let Some(i) = edge_at(origin) {
                    self.set_panel(i);
                    MontageDrag::Trim {
                        panel: i,
                        original: self.doc.panels[i].clone(),
                        start_len: self.doc.panels[i].frame_count(),
                        origin_x: origin.x,
                        snap: self.snapshot(),
                    }
                } else if let Some(i) = panel_at(origin.x) {
                    // Un panneau qui n'était pas choisi part seul ; sinon tout le groupe le suit.
                    if !self.board_selection().contains(&self.doc.panels[i].id) {
                        self.board_sel.clear();
                    }
                    self.set_panel(i);
                    MontageDrag::Move { panel: i, target: i }
                } else {
                    MontageDrag::Scrub
                };
                self.montage_drag = Some(drag);
            }
        }

        if self.cam_key_drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                if let Some(i) = self.doc.panels.iter().position(|q| self.is_dragged_camera_key_panel(q.id)) {
                    let start = self.doc.panel_start(i);
                    self.drag_camera_key(((p.x - x_of(start)) / ppf).floor().max(0.0) as usize);
                }
            }
            if resp.drag_stopped() {
                self.end_camera_key_drag();
            }
        } else if resp.hover_pos().is_some_and(|p| self.montage_camera_key_at(p, r.top(), &blocks, ppf, rows).is_some()) {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        if let Some(h) = self.handle_drag.as_ref().map(|d| d.handle) {
            let resize = matches!(h, Handle::LayerStart { .. } | Handle::LayerEnd { .. } | Handle::Exposure { .. });
            ui.ctx().set_cursor_icon(if resize { CursorIcon::ResizeHorizontal } else { CursorIcon::Grabbing });
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                // L'image sous la souris, et le bord d'image le plus proche (expositions, bouts).
                let x = ((p.x - x_of(self.doc.panel_start(self.cur_panel))) / ppf).max(0.0);
                self.drag_handle(x.floor() as usize, x.round() as usize);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_handle_drag();
            }
        } else if self.cam_key_drag.is_none() && resp.hover_pos().is_some_and(|p| mark_at(self, p).is_some()) {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        if resp.dragged() && self.cam_key_drag.is_none() && self.handle_drag.is_none() {
            if let Some(p) = resp.interact_pointer_pos() {
                match &mut self.montage_drag {
                    Some(MontageDrag::Scrub) => {
                        let g = frame_at(p.x);
                        if g != self.global_frame() {
                            self.goto_global(g);
                        }
                    }
                    Some(MontageDrag::Move { target, .. }) => {
                        *target = blocks.iter().filter(|b| b.center().x < p.x).count();
                    }
                    Some(MontageDrag::Trim { .. }) => self.update_trim(p.x),
                    Some(MontageDrag::Clip(_)) => self.update_clip_drag(p, ppf, lanes_top),
                    None => {}
                }
            }
        }

        if resp.drag_stopped() {
            match self.montage_drag.take() {
                Some(MontageDrag::Trim {
                    snap,
                    original,
                    panel,
                    ..
                }) => {
                    let changed = self
                        .doc
                        .panels
                        .get(panel)
                        .is_some_and(|s| s.id == original.id && s.frame_count() != original.frame_count());
                    if changed {
                        self.history.push(snap);
                        self.modified = true;
                    }
                }
                Some(MontageDrag::Move { panel, target }) => {
                    let ids = self.board_selection();
                    if ids.len() > 1 {
                        self.move_chosen_panels(&ids, target);
                    } else {
                        self.move_panel(panel, target);
                    }
                }
                Some(MontageDrag::Clip(d)) => self.finish_clip_drag(d),
                _ => {}
            }
        }

        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if let Some((i, k)) = self.layer_row_hit(p, r.top(), &blocks, rows) {
                    // Clic dans une piste de calque : ce panneau, cette image, ce calque.
                    self.sel_clip = None;
                    self.goto_global(frame_at(p.x));
                    if self.cur_panel == i {
                        self.select_layer(k);
                    }
                } else if let Some((_, hit)) = self.lane_hit(p, left, ppf, lanes_top) {
                    // Clic dans une piste son : sélectionne l'extrait (ou rien) et y place la tête.
                    self.sel_clip = hit.map(|(id, _)| id);
                    self.goto_global(frame_at(p.x));
                } else {
                    self.sel_clip = None;
                    let mods = ui.input(|i| i.modifiers);
                    let block = in_blocks(p).then(|| panel_at(p.x)).flatten();
                    // Clic sur une barre de scène ou de séquence : son premier panneau.
                    match (runs.iter().find(|(rect, _)| rect.contains(p)), block) {
                        (Some(&(_, start)), _) => {
                            self.board_sel.clear();
                            self.select_panel(start);
                        }
                        // Ctrl + clic : ajouter ou retirer un panneau du choix ; Maj + clic : une suite.
                        (None, Some(i)) if mods.command => {
                            let id = self.doc.panels[i].id;
                            match self.board_sel.iter().position(|s| *s == id) {
                                Some(k) => {
                                    self.board_sel.remove(k);
                                }
                                None if i != self.cur_panel => self.board_sel.push(id),
                                None => {}
                            }
                        }
                        (None, Some(i)) if mods.shift => {
                            let (a, b) = (self.cur_panel.min(i), self.cur_panel.max(i));
                            self.board_sel = self.doc.panels[a..=b].iter().map(|p| p.id).collect();
                        }
                        (None, block) => {
                            if block.is_some() {
                                self.board_sel.clear();
                            }
                            self.goto_global(frame_at(p.x));
                        }
                    }
                }
            }
        }
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if let Some(i) = in_blocks(p).then(|| panel_at(p.x)).flatten() {
                    self.enter_animation(i);
                } else if let Some((i, k)) = self.layer_row_hit(p, r.top(), &blocks, rows).filter(|_| p.y < r.top() + rows.background_top()) {
                    // Double-clic sur la piste d'un calque : un nouveau dessin (une clé) à cette image.
                    let f = (((p.x - x_of(self.doc.panel_start(i))) / ppf).floor().max(0.0)) as usize;
                    self.montage_new_key(i, k, f);
                }
            }
        }

        // Clic droit : menu du panneau ou de la piste son visés.
        if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.menu_target = if let Some((track, hit)) = self.lane_hit(p, left, ppf, lanes_top) {
                    let clip = hit.map(|(id, _)| id);
                    if clip.is_some() {
                        self.sel_clip = clip;
                    }
                    Some(MenuTarget::Lane {
                        track,
                        clip,
                        frame: free_frame_at(p.x),
                    })
                } else if in_blocks(p) {
                    panel_at(p.x).map(|i| {
                        if !self.board_selection().contains(&self.doc.panels[i].id) {
                            self.board_sel.clear();
                        }
                        self.select_panel(i);
                        MenuTarget::Panel(i)
                    })
                } else if let Some((i, k)) = self.layer_row_hit(p, r.top(), &blocks, rows) {
                    // Une piste de calque : ce plan, cette image, ce calque, et son menu.
                    self.sel_clip = None;
                    self.goto_global(frame_at(p.x));
                    (self.cur_panel == i).then(|| {
                        self.select_layer(k);
                        MenuTarget::Layer(k)
                    })
                } else if let Some(&(_, start)) = runs.iter().find(|(rect, _)| rect.contains(p)) {
                    self.board_sel.clear();
                    self.select_panel(start);
                    Some(MenuTarget::Sequence)
                } else if p.y < bands_top {
                    self.goto_global(frame_at(p.x));
                    Some(MenuTarget::Ruler)
                } else if (r.top() + rows.camera_top()..=r.top() + rows.camera_top() + crate::montagerows::CAM_H).contains(&p.y) {
                    self.goto_global(frame_at(p.x));
                    Some(MenuTarget::Camera)
                } else {
                    None
                };
            }
        }
        if let Some(target) = self.menu_target {
            resp.context_menu(|ui| match target {
                MenuTarget::Panel(i) => self.panel_menu(ui, i),
                MenuTarget::Lane { track, clip, frame } => self.lane_menu(ui, track, clip, frame),
                MenuTarget::Layer(k) => {
                    // La piste d'un calque : une clé à l'image du clic, puis le menu du calque.
                    if ui.button("Nouveau dessin ici (clé)").on_hover_text("Ou double-clic sur la piste.").clicked() {
                        ui.close_menu();
                        let (pi, f) = (self.cur_panel, self.cur_frame);
                        self.montage_new_key(pi, k, f);
                    }
                    ui.separator();
                    self.layer_menu(ui, k, false);
                }
                MenuTarget::Camera => self.grid_menu_ui(ui, crate::handles::GridMenu::Camera),
                MenuTarget::Sequence => {
                    self.action_item(ui, "Nouvelle séquence", "sequence.nouvelle");
                    self.action_item(ui, "Nouveau plan", "panneau.nouveau");
                    self.action_item(ui, "Nouveau plan avant le plan courant", "plan.nouveau_avant");
                    ui.separator();
                    self.action_item(ui, "Verrouiller / déverrouiller le storyboard", "storyboard.verrouiller");
                }
                MenuTarget::Ruler => {
                    self.action_item(ui, "Marque d'image ici (ajouter / retirer)", "marque.basculer");
                    self.action_item(ui, "Couper le plan ici", "panneau.couper");
                    self.action_item(ui, "Signet du plan ici (poser / retirer)", "signet.basculer");
                    ui.separator();
                    self.action_item(ui, "Importer un son ici…", "son.importer");
                }
            });
        }

        // Tête de lecture et repère de déplacement.
        let gx = x_of(self.global_frame());
        painter.line_segment([pos2(gx, r.top()), pos2(gx, r.bottom())], Stroke::new(2.0_f32, CUR_FRAME));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(gx - 5.0, r.top()), pos2(gx + 5.0, r.top()), pos2(gx, r.top() + 7.0)],
            CUR_FRAME,
            Stroke::NONE,
        ));
        if let Some(MontageDrag::Move { target, .. }) = &self.montage_drag {
            let x = match blocks.get(*target) {
                Some(b) => b.left() - 1.0,
                None => blocks.last().map_or(left, |b| b.right() + 1.0),
            };
            painter.line_segment(
                [pos2(x, top - 2.0), pos2(x, top + block_h + 2.0)],
                Stroke::new(3.0_f32, visuals.selection.stroke.color),
            );
        }

        if self.following {
            ui.scroll_to_rect(
                Rect::from_center_size(pos2(gx, top + block_h * 0.5), vec2(ppf * 24.0, 10.0)),
                None,
            );
        }
    }

    fn update_trim(&mut self, x: f32) {
        let ppf = self.montage_zoom;
        let fps = self.doc.fps;
        let Some(MontageDrag::Trim {
            panel,
            original,
            start_len,
            origin_x,
            ..
        }) = &self.montage_drag
        else {
            return;
        };
        let n = ((*start_len as f32) + ((x - *origin_x) / ppf).round()).max(1.0) as usize;
        let i = *panel;
        if self.doc.panels.get(i).is_none_or(|s| s.id != original.id) {
            self.montage_drag = None;
            return;
        }
        let old = original.frame_count();
        let mut s = original.clone();
        s.set_length(n);
        self.doc.panels[i] = s;
        self.renderer.invalidate_all();
        self.status = format!("{} : {}", panel_long_title(&self.doc, i), duration_label(n, fps));
        // Les plans suivants se décalent par rapport au son (chantier 4).
        if let Some(note) = crate::handles::sound_shift_note(&self.doc, i, old, n) {
            self.status = format!("{}. {note}", self.status);
        }
    }

    // ------------------------------------------------------------------ fiche du plan

    pub(crate) fn panel_inspector(&mut self, ui: &mut egui::Ui) {
        let i = self.cur_panel;
        let fps = self.doc.fps;
        // Image de storyboard sous la tête de lecture (13a) : le début du plan ou une marque d'image.
        let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let mark = self.doc.board_mark_at(i, f).map(|m| m.id);
        let boards = 1 + self.doc.panels[i].marks.len();
        let k = mark.and_then(|id| self.doc.panels[i].marks.iter().position(|m| m.id == id)).map_or(1, |p| p + 2);
        let scene = self.doc.panels[i].scene;
        let seq = self.doc.sequence_of(i);
        let scene_number = self.doc.scene_number(scene).to_string();
        let seq_number = self.doc.sequence_number(seq).to_string();
        ui.add_space(4.0);
        ui.heading(panel_long_title(&self.doc, i));
        // Chaque champ modifie une copie ; `edited` enregistre l'étape d'annulation avant qu'on
        // reporte la valeur (une étape par saisie).
        egui::Grid::new("fiche_panneau").num_columns(2).show(ui, |ui| {
            ui.label("Séquence");
            if let Some(mut name) = self.doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()) {
                let resp = ui
                    .add(
                        egui::TextEdit::singleline(&mut name)
                            .hint_text(seq_number)
                            .desired_width(150.0),
                    )
                    .on_hover_text("Vide = numéro automatique");
                if self.edited(&resp) {
                    if let Some(s) = self.doc.sequence_mut(seq) {
                        s.name = name;
                    }
                }
            }
            ui.end_row();
            ui.label("Plan");
            if let Some(mut name) = self.doc.scene(scene).map(|s| s.name.clone()) {
                let resp = ui
                    .add(
                        egui::TextEdit::singleline(&mut name)
                            .hint_text(scene_number)
                            .desired_width(150.0),
                    )
                    .on_hover_text("Numéro ou nom du plan (vide = numéro automatique)");
                if self.edited(&resp) {
                    if let Some(s) = self.doc.scene_mut(scene) {
                        s.name = name;
                    }
                }
            }
            ui.end_row();
            ui.label("Image");
            ui.label(format!("{k} sur {boards}"))
                .on_hover_text("Images de storyboard du plan : son début, puis chacune de ses marques d'image. Les textes ci-dessous sont ceux de l'image où est la tête de lecture.");
            ui.end_row();
            ui.label("Durée");
            ui.horizontal(|ui| {
                self.panel_length_field(ui, "");
                ui.label(RichText::new(duration_label(self.frame_count(), fps)).weak());
            });
            ui.end_row();
        });
        ui.label("Caméra").on_hover_text("Décalage du centre, zoom, rotation (outil Caméra pour la régler à la souris)");
        self.camera_fields(ui);
        for (label, field, rows) in [("Action", 0, 3), ("Dialogue", 1, 2), ("Notes", 2, 2)] {
            let p = &self.doc.panels[i];
            let mut text = match p.marks.iter().find(|m| Some(m.id) == mark) {
                Some(m) => [&m.action, &m.dialogue, &m.notes][field].clone(),
                None => [&p.action, &p.dialogue, &p.notes][field].clone(),
            };
            ui.label(label);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY),
            );
            if self.edited(&resp) {
                let p = &mut self.doc.panels[i];
                match p.marks.iter_mut().find(|m| Some(m.id) == mark) {
                    Some(m) => *[&mut m.action, &mut m.dialogue, &mut m.notes][field] = text,
                    None => *[&mut p.action, &mut p.dialogue, &mut p.notes][field] = text,
                }
            }
        }
    }
}
