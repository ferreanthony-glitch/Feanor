//! Niveau storyboard : les plans montés bout à bout, leurs vignettes, leur durée,
//! leurs indications (action, dialogue, notes) et la lecture de tout le montage.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
};

use crate::app::App;
use crate::document::Document;
use crate::handles::Handle;
use crate::soundtrack::{ClipDrag, ClipEdge, MenuTarget};
use crate::theme;
use crate::timeline::cur_frame;

pub(crate) enum MontageDrag {
    /// Bord d'un plan tiré (début, fin, coupe entre deux plans) ou contenu glissé : le plan est
    /// refait depuis l'original à chaque mouvement, rien n'est perdu tant qu'on n'a pas relâché
    /// (`rognageui.rs`).
    Edge(crate::rognageui::EdgeDrag),
    /// Plan déplacé dans le montage (son identifiant : le montage d'un équipier peut arriver pendant le geste) ; `target` est la
    /// place où il sera inséré. `merge` : le plan sur lequel il est, par le milieu, quand on le lâcherait pour le fusionner
    /// avec lui (`fusionglisse.rs`) ; alors `target` ne compte pas.
    Move { panel: u64, target: usize, merge: Option<u64> },
    Scrub,
    /// Extrait de son déplacé, ou raccourci par un bord.
    Clip(ClipDrag),
}

// Les blocs de plans que la bande du montage a peints à sa dernière image (pour les essais, `fusionglisse_tests.rs`).
#[cfg(test)]
thread_local! {
    pub(crate) static BLOCS_DU_MONTAGE: std::cell::RefCell<Vec<Rect>> = const { std::cell::RefCell::new(Vec::new()) };
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

/// Les secondes de la règle du montage à peindre, de `lo` à `hi` compris, bornées par `last` : celles dont la graduation tombe
/// dans `clip` (en x), plus celles, à gauche, dont le texte (« 12:34 », à droite de la graduation) peut encore y entrer. La
/// graduation de la seconde `s` est à `left + s × fps × ppf`. Rien à peindre : `lo > hi`.
pub(crate) fn visible_seconds(clip: Rect, left: f32, fps: usize, ppf: f32, last: usize) -> (usize, usize) {
    // (Un texte de la règle fait moins de 60 points ; 120 de marge.)
    const MARGE: f32 = 120.0;
    let pas = fps as f32 * ppf;
    if pas.is_nan() || pas <= 0.0 || !clip.is_finite() {
        return (0, last);
    }
    let lo = ((clip.left() - MARGE - left) / pas).floor().max(0.0);
    let hi = ((clip.right() + 1.0 - left) / pas).ceil().max(0.0);
    if lo > last as f32 {
        return (1, 0);
    }
    (lo as usize, (hi.min(last as f32)) as usize)
}

/// `panel_title`, les numéros du film déjà trouvés (`Numeros` : un titre par bloc visible, à chaque image d'écran).
pub(crate) fn panel_title_n(doc: &Document, numeros: &crate::numerotation::Numeros, index: usize) -> String {
    format!("Plan {}", numeros.scene_label(doc, doc.panels[index].scene))
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
        self.structure_op_porte(true, op)
    }

    /// Comme `structure_op`. `montage` : l'opération change le montage (ordre, nombre, durée : il faut le jeton dans un film
    /// partagé) ; sinon elle ne change que le contenu du plan courant (une marque d'image : il faut tenir le plan).
    pub(crate) fn structure_op_porte(&mut self, montage: bool, op: impl FnOnce(&mut Document, usize) -> Option<usize>) -> bool {
        self.structure_op_avec(montage, false, op)
    }

    /// Comme `structure_op`, pour une opération qui copie, coupe, réunit ou déplace les dessins des plans (réunir, couper,
    /// dupliquer, coller, déplacer) : les sélections en cours se posent d'abord (`poser_les_selections`). Les autres (nouveau plan,
    /// marque d'image…) laissent une zone soulevée telle quelle : le dessin qu'elle déplace reste celui du plan.
    pub(crate) fn structure_op_dessins(&mut self, op: impl FnOnce(&mut Document, usize) -> Option<usize>) -> bool {
        self.structure_op_avec(true, true, op)
    }

    fn structure_op_avec(&mut self, montage: bool, poser: bool, op: impl FnOnce(&mut Document, usize) -> Option<usize>) -> bool {
        if montage && self.storyboard_refuses() || !montage && self.content_refuses() {
            return false;
        }
        self.end_stroke();
        // (Couper, réunir, dupliquer, ranger une version : les dessins copiés partent avec le fond de leur dernier trait.)
        self.attendre_fonds_de_traits();
        if poser {
            self.poser_les_selections();
        }
        self.playing = false;
        // « Report du décor » (Préférences > Vignettes) : lu par la jonction de deux plans (`decor.rs`).
        self.doc.report_decor = self.settings.vignettes.report_decor;
        // « Durée d'un nouveau plan » (Préférences > Montage) : lue par `make_panel`.
        self.doc.duree_plan_neuf = self.settings.new_plan_secs;
        let snap = self.snapshot();
        let before = self.panel().id;
        let layer_before = self.layer().id;
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
        // Le calque courant, par son identifiant quand il est encore là : une opération qui pose un calque en bas du plan (l'image
        // déposée, `deposer.rs`) décale les rangs, et le rang d'avant désignait le calque du dessous (le trait suivant y allait).
        self.cur_layer = self.layers().iter().position(|l| l.id == layer_before).unwrap_or_else(|| self.cur_layer.min(self.layers().len() - 1));
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
        self.structure_op_dessins(|d, i| Some(d.duplicate_scene_after(i)));
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
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau Pelure (`pasteimage`).
        self.ctx.copy_text(format!("{}plans copiés", crate::pasteimage::MARK));
        self.panel_clipboard = Some(clip);
        self.fx_clip_recent = false;
        self.fx_pile_recent = false;
        self.cam_clip_recent = false;
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
        let Some(original) = self.panel_clipboard.clone() else {
            self.status = "Aucun plan copié.".into();
            return;
        };
        // Copiés dans un autre onglet, d'une autre feuille : adaptés (`adaptation.rs`, `collage.rs`). L'original reste
        // dans le presse-papiers : on peut le coller ailleurs, à sa taille.
        let clip = match self.collage_preparer(crate::collage::Colle::Plans(original), crate::collage::Reprise::Plans) {
            crate::collage::Preparation::Pret(crate::collage::Colle::Plans(c)) => c,
            _ => return,
        };
        let n = clip.panels.len();
        let after = panel_title(&self.doc, self.cur_panel);
        if self.structure_op_dessins(|d, i| d.paste_plans(&clip, i)) {
            let end = (self.cur_panel + n).min(self.doc.panels.len());
            self.board_sel = self.doc.panels[self.cur_panel..end].iter().map(|p| p.id).collect();
            self.status = match n {
                1 => format!("Plan collé après le {}.", after.to_lowercase()),
                n => format!("{n} plans collés après le {}.", after.to_lowercase()),
            };
            // Copiés d'un projet d'une autre cadence, les images gardées et rien d'adapté : la durée change (24 images =
            // une seconde à 24 i/s, un peu moins à 25). Une copie adaptée a déjà son message (`collage.rs`).
            if clip.origine.cadence != self.doc.fps {
                self.status += &format!(" Attention : ils viennent d'un projet à {} i/s, celui-ci tourne à {} i/s : leur durée change.", clip.origine.cadence, self.doc.fps);
            }
        } else {
            self.status = crate::onglets::refus_de_taille(if n > 1 { "Ces plans viennent" } else { "Ce plan vient" }, clip.origine.feuille, [self.doc.width, self.doc.height]);
        }
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
        self.move_plans_before(ids, before);
    }

    /// Déplace les plans `ids` devant le plan `before` (aucun : à la fin) : l'opération du montage, que la vue Vignettes
    /// reprend telle quelle. Le plan courant reste courant. Vrai si l'ordre a changé.
    pub(crate) fn move_plans_before(&mut self, ids: &[u64], before: Option<u64>) -> bool {
        let current = self.panel().id;
        let ids = ids.to_vec();
        self.structure_op_dessins(|d, _| {
            d.move_plans(&ids, before)?;
            d.panels.iter().position(|p| p.id == current)
        })
    }

    /// Déplace le plan `from` à la place `slot` (0 = au début, nombre de plans = à la fin).
    pub(crate) fn move_panel(&mut self, from: usize, slot: usize) {
        let Some(id) = self.doc.panels.get(from).map(|p| p.id) else { return };
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
        if self.storyboard_refuses() || !self.partage_peut_toucher(&[self.panel().id]) {
            return;
        }
        let at = self.cur_frame;
        // Un plan qui a des versions rangées : la question « avec quelle partie ? » d'abord (`coupeversions.rs`).
        let plan = self.panel().id;
        if self.coupe_versions_necessaire(plan, at) {
            self.coupe_versions_ouvrir(plan, at, crate::coupeversions::Refaire::Courant);
            return;
        }
        if self.structure_op_dessins(|d, i| d.split_plan(i, at)) {
            self.status = format!("Plan coupé : la suite est le {}.", panel_title(&self.doc, self.cur_panel).to_lowercase());
        } else {
            self.status = "Place la tête de lecture dans le plan, après sa première image, pour le couper.".into();
        }
    }

    /// Réunit le plan courant et le suivant (de la même séquence) : le début du suivant devient
    /// une marque d'image.
    pub(crate) fn merge_next_panel(&mut self) {
        let voisins: Vec<u64> = self.doc.panels.iter().skip(self.cur_panel).take(2).map(|p| p.id).collect();
        if self.storyboard_refuses() || !self.partage_peut_toucher(&voisins) {
            return;
        }
        // Deux calques de même nom : la fenêtre le demande d'abord (`fusionnoms.rs`).
        let a = self.panel().id;
        self.jonction_demander(crate::fusionnoms::Jonction::Suivant { a, precedent: false });
    }

    /// Fusionne les plans choisis (deux ou plus, qui se suivent dans une même séquence) en un
    /// seul, sans repère aux jointures (`Document::join_plans`).
    pub(crate) fn merge_chosen_panels(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        let ids = self.board_selection();
        if !self.partage_peut_toucher(&ids) {
            return;
        }
        let n = ids.len();
        if n < 2 {
            self.status = "Choisis au moins deux plans (Ctrl ou Maj + clic dans le montage) pour les fusionner.".into();
            return;
        }
        // Deux calques de même nom : la fenêtre le demande d'abord (`fusionnoms.rs`).
        self.jonction_demander(crate::fusionnoms::Jonction::Choisis(ids));
    }

    /// Réunit le plan précédent (de la même séquence) et le plan courant.
    pub(crate) fn join_previous_scene(&mut self) {
        let voisins: Vec<u64> = self.doc.panels.iter().skip(self.cur_panel.saturating_sub(1)).take(2).map(|p| p.id).collect();
        if self.storyboard_refuses() || !self.partage_peut_toucher(&voisins) {
            return;
        }
        let Some(a) = self.cur_panel.checked_sub(1).and_then(|i| self.doc.panels.get(i)).map(|p| p.id) else {
            self.status = "Pas de plan précédent dans cette séquence.".into();
            return;
        };
        // Deux calques de même nom : la fenêtre le demande d'abord (`fusionnoms.rs`).
        self.jonction_demander(crate::fusionnoms::Jonction::Suivant { a, precedent: true });
    }

    /// Pose ou retire une marque d'image à l'image courante du plan (une image de storyboard de
    /// plus ou de moins dans les vignettes et le PDF).
    pub(crate) fn toggle_mark(&mut self) {
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        // Une marque posée fait une case de plus, et le dessin tenu par-dessus la marque y est copié (`cles_de_case`) : si c'est celui
        // d'une zone soulevée, la zone se pose d'abord (sinon elle continuerait sur le dessin d'avant la marque, et la copie
        // garderait les pixels à mi-chemin). Ailleurs, la zone reste soulevée : la marque ne touche pas à son dessin.
        let pose = frame > 0 && !self.panel().marks.iter().any(|m| m.frame == frame) && {
            let soulevee = self.bsel.as_ref().filter(|s| s.lifted.is_some()).map(|s| s.cel);
            soulevee.is_some_and(|c| self.layers().iter().any(|l| l.cel_at(frame) == Some(c) && l.cel_at(frame - 1) == Some(c)))
        };
        if pose {
            self.poser_les_selections();
        }
        let mut placed = None;
        let done = self.structure_op_porte(false, |d, i| {
            placed = d.toggle_mark(i, frame);
            // Une case de plus : une clé de dessin de plus sur chaque calque, à son image (`cases.rs`).
            if placed == Some(true) {
                d.cles_de_case(i, frame);
            }
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
        if self.storyboard_refuses() || !self.partage_peut_toucher(&[self.panel().id]) {
            return;
        }
        self.length_field_active = true;
        let id = self.panel().id;
        if self.length_origin.as_ref().is_none_or(|s| s.panel.id != id) {
            self.end_stroke();
            let snap = self.snapshot();
            self.history.push(snap);
            self.length_origin = Some(crate::rognageui::PlanBackup::capture(&self.doc, self.cur_panel));
        }
        let Some(origin) = &self.length_origin else { return };
        // Raccourcir garde le reste en réserve (`rognage.rs`) ; chaque valeur repart du plan d'avant.
        origin.restore(&mut self.doc);
        self.doc.set_duration(self.cur_panel, n);
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
        self.cancel_cam_keys_drag();
        self.cancel_handle_drag();
        self.trans_cancel_drag();
        match self.montage_drag.take() {
            Some(MontageDrag::Edge(d)) => self.cancel_edge_drag(d),
            Some(MontageDrag::Clip(d)) => self.doc.audio_tracks = d.before,
            _ => {}
        }
    }

    // ------------------------------------------------------------------ panneau du montage

    pub(crate) fn montage_panel(&mut self, ui: &mut egui::Ui) {
        if self.bande_film_vide(ui) {
            return;
        }
        ui.add_space(2.0);
        // Une seule ligne, comme la barre de la timeline (12c), qui ne défile pas : les commandes de
        // `barres.rs`, le reste sous « Plus… ».
        // (Dans l'espace Montage, la barre du travail de montage : coupes, rognage, transitions, son…)
        let barre = crate::barres::Barre::du_montage_dans(self.espace);
        ui.horizontal(|ui| {
            self.barre_ui(ui, barre);
            // Au bout de la ligne : la légende des clés et son filtre (`legendecles.rs`).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.legende_cles_ui(ui));
        });
        ui.separator();
        // Le zoom, petit, tout en bas de la bande.
        egui::TopBottomPanel::bottom("zoom_montage")
            .resizable(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 1)))
            .show_inside(ui, |ui| self.montage_zoom_bar(ui));
        // La piste du calque courant se déplie quand la souris est relâchée (`pistes.rs`).
        let pointer_down = ui.input(|i| i.pointer.any_down());
        self.pistes_follow_current(pointer_down);
        // Hauteur des panneaux : ce qui reste une fois la règle, les barres et les pistes placées.
        let rows = self.montage_rows(ui.available_height());
        // Quand la bande est trop basse pour toutes les pistes, elle défile : aucune piste ni bouton coupé, et la barre de
        // défilement se voit tout de suite (pas seulement au survol, `montagehauteur.rs`).
        ui.scope(|ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            #[allow(unused_variables)]
            let sortie = egui::ScrollArea::vertical()
                .id_salt("montage_v")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let headers = self.track_headers(ui, rows.lanes_top());
                        // La colonne dans une zone à elle : ses boutons, en nombre variable selon les
                        // calques du plan courant, ne changent pas l'identité de la bande qui suit
                        // (sinon un glisser de la tête de lecture se perdait en entrant dans un plan
                        // qui n'a pas autant de calques, 2026-10-17).
                        let mut column = ui.new_child(egui::UiBuilder::new().max_rect(headers).id_salt("colonne_montage"));
                        self.row_headers(&mut column, headers, &rows);
                        self.head_width_grip(&mut column, headers);
                        egui::ScrollArea::horizontal()
                            .id_salt("montage_h")
                            .auto_shrink([false, true])
                            .show(ui, |ui| self.montage_strip(ui, &rows));
                    });
                });
            #[cfg(test)]
            crate::montagehauteur::DEFILEMENT.with(|c| c.set((sortie.content_size.y, sortie.inner_rect.height())));
        });
    }

    /// La position dans le film et sa durée (affichage fixe de la barre du montage).
    pub(crate) fn montage_position_ui(&mut self, ui: &mut egui::Ui) {
        let (total, fps) = (self.doc.total_frames(), self.doc.fps);
        let resp = ui.label(RichText::new(format!("{} / {}", timecode(self.global_frame(), fps), timecode(total, fps))).monospace());
        crate::bulles::bulle(resp, "Position", "Image courante / durée du montage (minutes:secondes:images).", "");
    }

    /// Le menu « Plans » de la barre du montage (`barres.rs`) : dupliquer, copier, couper, réunir,
    /// marques d'image.
    pub(crate) fn plans_barre_menu(&mut self, ui: &mut egui::Ui) {
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
        let n = self.board_selection().len();
        if n > 1 {
            item(self, ui, &format!("Fusionner les {n} plans choisis"), "panneau.fusionner_choisis");
            item(self, ui, &format!("Nouvelle séquence avec les {n} plans choisis"), "sequence.nouvelle_avec_plans");
            item(self, ui, &format!("Changer la durée des {n} plans choisis…"), "montage.duree_plans");
        }
        item(self, ui, if n > 1 { "Retourner les plans choisis" } else { "Retourner le plan" }, "plan.retourner");
        ui.separator();
        item(self, ui, "Marque d'image ici (ajouter / retirer)", "marque.basculer");
    }

    /// Le zoom du montage, en bas de la bande et petit (Anthony, 2026-10-17 : « il faut le mettre
    /// en bas de la timeline, pas en haut, et le rendre plus petit ») : arrière, curseur, avant,
    /// tout voir ; Ctrl + molette sur la bande, Alt+1 / Alt+2 / Alt+3.
    fn montage_zoom_bar(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        use egui_phosphor::regular as ph;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().slider_width = 70.0;
            let small = |icon: &str| egui::Button::new(RichText::new(icon).size(11.0)).frame(false).min_size(vec2(16.0, 14.0));
            let (out, inn, all) = (self.raccourci("bande.zoom_arriere"), self.raccourci("bande.zoom_avant"), self.raccourci("bande.tout_voir"));
            if bulle(ui.add(small(ph::MAGNIFYING_GLASS_MINUS)), "Zoom arrière", "Ou Ctrl + molette sur la bande.", &out).clicked() {
                self.run_action("bande.zoom_arriere");
            }
            let resp = ui.add(egui::Slider::new(&mut self.montage_zoom, 0.5..=16.0).logarithmic(true).show_value(false));
            if resp.changed() {
                self.anchor_strip_zoom(crate::zoombande::Strip::Montage);
            }
            bulle(resp, "Zoom du montage", "Largeur d'une image dans la bande ; la tête de lecture reste en place.", "");
            if bulle(ui.add(small(ph::MAGNIFYING_GLASS_PLUS)), "Zoom avant", "Ou Ctrl + molette sur la bande.", &inn).clicked() {
                self.run_action("bande.zoom_avant");
            }
            if bulle(ui.add(small(ph::ARROWS_OUT_LINE_HORIZONTAL)), "Tout voir", "Tout le film dans la largeur de la bande.", &all).clicked() {
                self.run_action("bande.tout_voir");
            }
            // La hauteur des pistes son : petite, moyenne, grande (`montagehauteur.rs`).
            self.hauteur_pistes_bouton(ui);
        });
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
        item(self, ui, "Bruitages…", "vue.bruitages");
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

    /// Couleur de repère des plans choisis (ou du plan visé, qui est alors le seul choisi) : une
    /// étape d'annulation, rien si rien ne change.
    pub(crate) fn set_plan_color(&mut self, color: Option<u8>) {
        let ids = self.board_selection();
        let differs = self.doc.panels.iter().any(|p| ids.contains(&p.id) && p.color != color);
        if !differs {
            return;
        }
        self.record_step();
        self.doc.set_panels_color(&ids, color);
    }

    /// « Couleur du plan ▸ » (clic droit sur un plan du montage ou sur une vignette) : une pastille
    /// par couleur de repère, et « Aucune ».
    pub(crate) fn plan_color_menu(&mut self, ui: &mut egui::Ui) {
        let ids = self.board_selection();
        // La couleur commune des plans visés, pour cocher la pastille actuelle.
        let first = self.doc.panels.iter().find(|p| ids.contains(&p.id)).map(|p| p.color);
        let common = first.filter(|c| self.doc.panels.iter().filter(|p| ids.contains(&p.id)).all(|p| p.color == *c));
        ui.menu_button("Couleur du plan", |ui| {
            if ui.selectable_label(common == Some(None), "Aucune").clicked() {
                ui.close_menu();
                self.set_plan_color(None);
            }
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(168.0);
                for (k, (name, _)) in crate::marks::COLORS.iter().enumerate() {
                    let k = k as u8;
                    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                    let c = crate::marks::color(k);
                    ui.painter().rect_filled(rect.shrink(2.0), 4.0, c);
                    if common == Some(Some(k)) {
                        ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0_f32, ui.visuals().strong_text_color()), StrokeKind::Inside);
                    } else if resp.hovered() {
                        ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ui.visuals().text_color()), StrokeKind::Inside);
                    }
                    if resp.on_hover_text(*name).clicked() {
                        ui.close_menu();
                        self.set_plan_color(Some(k));
                    }
                }
            });
        });
    }

    /// Menu du clic droit sur un plan de la bande de montage.
    ///
    /// Une dizaine d'entrées au premier niveau (simplification des barres, 2026-10-21) : les gestes de tous les jours
    /// (animer, nouveau, dupliquer, copier, coller, supprimer), puis « Couper et réunir », « Version du plan », la couleur
    /// du plan et ses deux transitions (2026-10-28 : plus de sous-menu « Plus »).
    pub(crate) fn panel_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        if ui.button("Animer ce plan").clicked() {
            ui.close_menu();
            self.enter_animation(index);
        }
        // Film partagé : prendre, rendre, reprendre (`cooperationui.rs`), en une seule entrée.
        if self.partage.is_some() {
            ui.menu_button("Équipe", |ui| self.equipe_menu_items(ui, index));
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
        ui.menu_button("Couper et réunir", |ui| {
            if ui.button("Couper le plan à l'image courante").clicked() {
                ui.close_menu();
                self.split_current_panel();
            }
            if ui.button("Réunir avec le plan suivant").clicked() {
                ui.close_menu();
                self.merge_next_panel();
            }
            // Plusieurs plans choisis (Ctrl ou Maj + clic) : les fusionner en un seul.
            let n = self.board_selection().len();
            if n > 1 && ui.button(format!("Fusionner les {n} plans choisis")).on_hover_text("Ils doivent se suivre, dans une même séquence. Pas de repère aux jointures : leurs textes rejoignent ceux du plan.").clicked() {
                ui.close_menu();
                self.merge_chosen_panels();
            }
            // Plusieurs plans choisis : une séquence à eux (`sequenceplans.rs`).
            if n > 1 && ui.button(format!("Nouvelle séquence avec les {n} plans choisis")).on_hover_text("Ils quittent leur séquence et forment une séquence neuve, dans l'ordre du film.").clicked() {
                ui.close_menu();
                self.new_sequence_from_chosen();
            }
        });
        // Les versions du plan (`planversui.rs`) : les deux « Nouvelle version » (en dupliquant la version en cours, ou en
        // partant d'un plan vide), puis passer à, restaurer, supprimer, comparer.
        self.version_menu(ui, index, true);
        // Plus de sous-menu « Plus » (Anthony, 2026-10-28, copie d'écran annotée : « éviter le sur-développement ») : la
        // couleur et les deux transitions montent au premier niveau. « Rogner le plan » (sur la règle de la timeline et dans
        // le menu Storyboard) et « Importer un son au début du plan » (menu Son) quittent ce menu.
        ui.separator();
        self.plan_color_menu(ui);
        // Les coupes de part et d'autre du plan (`transitionsui.rs`).
        self.cut_menu_named(ui, index, "Transition au début du plan");
        self.cut_menu_named(ui, index + 1, "Transition à la fin du plan");
        if self.board_selection().len() > 1 {
            self.action_item(ui, "Changer la durée des plans choisis…", "montage.duree_plans");
        }
    }

    fn montage_strip(&mut self, ui: &mut egui::Ui, rows: &crate::montagerows::Rows) {
        // « Tout voir » : le film dans la largeur visible de la bande.
        if std::mem::take(&mut self.montage_fit) {
            let shown = self.doc.total_frames().max(self.doc.audio_end()).max(1);
            self.montage_zoom = ((ui.available_width() - 40.0) / shown as f32).clamp(0.5, 16.0);
        }
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
        // Un zoom a eu lieu à l'image d'écran d'avant : la bande défile pour que la tête de lecture
        // reste en place (`zoombande.rs`).
        let head = self.global_frame() as f32;
        self.follow_strip_anchor(ui, crate::zoombande::Strip::Montage, left, ppf, head);
        // Ctrl + molette sur la bande : zoom du montage (comme sur la grille de la timeline), autour
        // de la tête de lecture ; il compte à l'image d'écran suivante.
        if resp.hovered() {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                self.anchor_strip_zoom(crate::zoombande::Strip::Montage);
                self.montage_zoom = (self.montage_zoom * zoom).clamp(0.5, 16.0);
            }
        }
        let x_of = |g: usize| left + g as f32 * ppf;
        let bands_top = r.top() + RULER_H;
        let top = r.top() + rows.blocks_top();
        let lanes_top = r.top() + rows.lanes_top();
        let in_blocks = |p: Pos2| p.y >= top && p.y <= top + block_h;
        let visuals = ui.visuals().clone();
        // Les transitions posées sur les coupes (`transitionsui.rs`) : leurs formes, au milieu des plans.
        let badges = self.trans_badges(left, ppf, top + block_h * 0.5);
        // Un clic dans la bande lui donne la main : copier, coller, supprimer visent ses panneaux.
        if resp.is_pointer_button_down_on() {
            self.panel_focus = true;
            self.images_focus = false;
            self.trans_track_focus(ui.input(|i| i.pointer.press_origin()), &badges);
        }

        // Règle : une graduation par seconde, un repère lisible tous les ~60 pixels.
        painter.rect_filled(Rect::from_min_max(r.min, pos2(r.right(), bands_top)), 0.0, theme::regle());
        let label_every = ((60.0 / (fps as f32 * ppf)).ceil() as usize).max(1);
        // (Les seules secondes que la partie visible de la bande montre, plus un repère à gauche dont le texte peut y entrer :
        // tout le film coûtait un `format!` et un texte par repère, à chaque image d'écran.)
        let (s_lo, s_hi) = if crate::perf::ancien() { (0, shown / fps + 2) } else { visible_seconds(painter.clip_rect(), left, fps, ppf, shown / fps + 2) };
        for s in s_lo..=s_hi {
            let x = x_of(s * fps);
            let major = s % label_every == 0;
            painter.line_segment(
                [pos2(x, r.top() + if major { 3.0 } else { 10.0 }), pos2(x, bands_top)],
                Stroke::new(1.0_f32, theme::graduation_seconde()),
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

        // Plans. (Les numéros du film, trouvés une fois pour les titres et la barre des séquences.)
        let numeros = crate::numerotation::Numeros::pour_l_ecran(&self.doc);
        let deadline = std::time::Instant::now() + crate::thumbs::THUMB_TIME;
        let thumb_h = (block_h - 24.0).max(10.0);
        let aspect = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let mut blocks: Vec<Rect> = Vec::with_capacity(self.doc.panels.len());
        // Les repères « modifié depuis le dernier envoi » posés ce coup-ci : où, et à quel plan (`envoisui.rs`).
        let mut reperes: Vec<(Rect, u64)> = Vec::new();
        // Les pastilles de prise (film partagé) de ce coup-ci : la zone et le plan, pour la bulle.
        let mut prises_zones: Vec<(Rect, u64)> = Vec::new();
        // Les notes à traiter de chaque plan (`fildecisionui.rs`) : un seul passage sur les commentaires, et la zone de chaque pastille.
        let notes_marques = self.doc.plan_marks();
        let mut notes_zones: Vec<(Rect, crate::fildecision::PlanMark)> = Vec::new();
        // Les étiquettes de version « V2 » posées ce coup-ci (`pileversui.rs`).
        let mut etiquettes: Vec<crate::pileversui::EtiquetteBloc> = Vec::new();
        let mut start = 0;
        // (La souris tenue : un geste change peut-être un plan sans étape d'historique, les clés du bloc se recalculent alors.)
        let pointer_tenu = ui.input(|i| i.pointer.any_down());
        for i in 0..self.doc.panels.len() {
            let len = self.doc.panels[i].frame_count();
            let rect = Rect::from_min_max(
                pos2(x_of(start) + 1.0, top + 2.0),
                pos2(x_of(start + len) - 1.0, top + block_h),
            );
            blocks.push(rect);
            start += len;
            // Hors de la partie visible de la bande : ni dessin ni vignette (le bloc reste dans
            // `blocks`, les gestes s'en servent). Sans cela, six cents plans coûtaient six cents
            // vignettes cherchées et six cents titres mis en page à chaque image d'écran.
            if !rect.expand(8.0).intersects(painter.clip_rect()) {
                continue;
            }
            let selected = i == self.cur_panel;
            // Choisi avec lui (Ctrl ou Maj + clic) : un ton d'accent plus léger.
            let chosen = !selected && self.board_sel.contains(&self.doc.panels[i].id);
            let bg = if selected {
                theme::surface_active(crate::theme::accent(), 0.55)
            } else if chosen {
                theme::surface_active(crate::theme::accent(), 0.3)
            } else {
                theme::bloc()
            };
            // Plan coloré (couleur de repère) : fond teinté, puis le ton du plan courant ou choisi
            // par-dessus, et une bande pleine en haut ; le contour de sélection reste le même.
            if let Some(tag) = self.doc.panels[i].color {
                let c = crate::marks::color(tag);
                painter.rect_filled(rect, 4.0, theme::bloc().lerp_to_gamma(c, 0.42));
                if selected || chosen {
                    painter.rect_filled(rect, 4.0, theme::surface_active(crate::theme::accent(), if selected { 0.4 } else { 0.2 }));
                }
                let band = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 6.0));
                painter.rect_filled(band, CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, c);
            } else {
                painter.rect_filled(rect, 4.0, bg);
            }
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
            // Modifié (ou ajouté) depuis le dernier envoi : une pastille en haut à droite du bloc (une bande
            // ambre sur un bloc étroit).
            if let Some(etat) = self.envois_ui.etat(self.doc.panels[i].id) {
                reperes.push((crate::envoisui::peindre_repere_du_bloc(&clip, rect, etat), self.doc.panels[i].id));
            }
            // Des notes encore à traiter : une petite pastille sous celle-là (`fildecisionui.rs`).
            if let Some(m) = notes_marques.get(&self.doc.panels[i].id).filter(|m| m.counts.open() > 0) {
                if let Some(zone) = crate::fildecisionui::peindre_marque_du_bloc(&clip, rect, m) {
                    notes_zones.push((zone, *m));
                }
            }
            // Film partagé : qui tient le plan (pastille à sa couleur, en haut à gauche) et qui le regarde.
            if let Some(zone) = self.peindre_pastille(&clip, rect, self.doc.panels[i].id, 1.0) {
                prises_zones.push((zone, self.doc.panels[i].id));
            }
            let title = clip.layout_no_wrap(crate::planversui::titre_avec_version_n(&self.doc, &numeros, i), FontId::proportional(12.0), visuals.text_color());
            let duration = clip.layout_no_wrap(
                duration_label(len, self.doc.fps),
                FontId::monospace(10.0),
                visuals.weak_text_color(),
            );
            // La durée n'est affichée que si elle tient à côté du titre.
            if title.size().x + duration.size().x + 20.0 < rect.width() {
                let at = pos2(rect.right() - 7.0 - duration.size().x, rect.bottom() - 7.0 - duration.size().y);
                clip.galley(at, duration, visuals.weak_text_color());
            }
            let at = pos2(rect.left() + 5.0, rect.bottom() - 7.0 - title.size().y);
            clip.galley(at, title, visuals.text_color());
            // Les clés de tous les calques du plan, en petits traits le long du bas du bloc (`clesplan.rs`), de la couleur de leur calque ;
            // calculées une fois par changement du document.
            let cles = self.cles_blocs.pour(self.history.serial(), &self.doc.panels[i], pointer_tenu);
            crate::clesplan::peindre(&painter.with_clip_rect(rect.intersect(painter.clip_rect())), rect, &cles, ppf);
            // (Les marques d'image, les limites des cases du plan, ne se dessinent plus dans le montage : réunir deux plans en décochant une
            // coupe y laissait un repère orange qui ne sert à rien. Elles restent dans les données, la planche et la timeline du plan.)
            // La version du plan (« V2 »), près du haut, au milieu, quand il en a plusieurs : la survoler ouvre
            // la pile des versions (`pileversui.rs`).
            let a_repere = self.envois_ui.etat(self.doc.panels[i].id).is_some() && crate::envoisui::repere_en_pastille(rect);
            if let Some(e) = self.peindre_etiquette_version(&clip, rect, i, a_repere) {
                etiquettes.push(e);
            }
            if selected || chosen {
                painter.rect_stroke(
                    rect,
                    4.0,
                    Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, theme::contour()),
                    StrokeKind::Inside,
                );
            }
            // Poignées de rognage sur les bords : le début à gauche, la fin (la durée) à droite ;
            // un triangle dit qu'il reste du contenu rogné de ce côté (`rognage.rs`).
            let (hid_head, hid_tail) = self.doc.hidden(i);
            let edge_lit = |side: crate::rognageui::Side| {
                self.edge_sel.is_some_and(|e| {
                    e.panel == self.doc.panels[i].id && e.side == side
                        || (e.side == crate::rognageui::Side::Cut
                            && ((side == crate::rognageui::Side::Tail && e.panel == self.doc.panels[i].id)
                                || (side == crate::rognageui::Side::Head && i > 0 && e.panel == self.doc.panels[i - 1].id)))
                })
            };
            for (side, x0, hidden) in [
                (crate::rognageui::Side::Head, rect.left() + 1.0, hid_head),
                (crate::rognageui::Side::Tail, rect.right() - 4.0, hid_tail),
            ] {
                let lit = edge_lit(side);
                let color = if lit { theme::contour() } else { theme::poignee() };
                let w = if lit { 4.0 } else { 3.0 };
                painter.rect_filled(Rect::from_min_max(pos2(x0, rect.top() + 10.0), pos2(x0 + w, rect.bottom() - 10.0)), 1.0, color);
                if hidden > 0 {
                    let (x, dir) = if side == crate::rognageui::Side::Head { (rect.left() + 6.0, -1.0) } else { (rect.right() - 6.0, 1.0) };
                    let y = rect.center().y;
                    painter.add(egui::Shape::convex_polygon(
                        vec![pos2(x, y - 4.0), pos2(x, y + 4.0), pos2(x + dir * 5.0, y)],
                        theme::contour(),
                        Stroke::NONE,
                    ));
                }
            }
        }

        // (Les essais retrouvent les plans à l'écran par les blocs de la dernière image.)
        #[cfg(test)]
        BLOCS_DU_MONTAGE.with(|b| b.borrow_mut().clone_from(&blocks));
        // Barre des séquences au-dessus des plans (13a : un plan = un bloc, plus de barre de scène).
        let mut runs: Vec<(Rect, usize)> = Vec::new();
        // (La séquence de chaque plan et leur ordre, trouvés une fois : les chercher plan par plan
        // coûtait le carré du nombre de plans.)
        let seqs = self.doc.sequence_ids();
        let clip_bar = painter.clip_rect();
        let ancien_ordre = crate::perf::ancien().then(|| self.doc.sequence_order());
        // Durées cibles des séquences (`budget.rs`) : rien à calculer quand aucune n'est posée.
        let budgets = crate::budget::budgets_des_sequences(&self.doc);
        for (band, y0, h) in [(0, bands_top, SEQ_H)] {
            let mut i = 0;
            while i < self.doc.panels.len() {
                let (start, end) = if band == 0 {
                    let mut e = i + 1;
                    while e < seqs.len() && seqs[e] == seqs[i] {
                        e += 1;
                    }
                    (i, e)
                } else {
                    self.doc.scene_run(i)
                };
                let rect = Rect::from_min_max(
                    pos2(blocks[start].left(), y0 + 1.0),
                    pos2(blocks[end - 1].right(), y0 + h - 1.0),
                );
                // Hors de la partie visible : ni dessin ni étiquette (la barre reste dans `runs`, les gestes s'en servent).
                if !rect.intersects(clip_bar) && !crate::perf::ancien() {
                    runs.push((rect, start));
                    i = end;
                    continue;
                }
                let mut depasse = false;
                let (label, number) = if band == 0 {
                    let seq = seqs[start];
                    // (Essai de performance du code d'avant : le rang dans l'ordre trouvé une fois, comme il l'était.)
                    let number = match &ancien_ordre {
                        Some(order) => order.iter().position(|&s| s == seq).map_or(0, |k| k + 1),
                        None => numeros.sequence_number(&self.doc, seq),
                    };
                    let label = numeros.sequence_label(&self.doc, seq);
                    let mut texte = format!("Séquence {label}{}", crate::variantesui::suffixe_barre(&self.doc, seq));
                    if let Some(b) = budgets.get(&seq) {
                        texte += &format!(" · {}", b.texte(self.doc.fps));
                        depasse = b.depasse();
                    }
                    (texte, number)
                } else {
                    let scene = self.doc.panels[start].scene;
                    (format!("Scène {}", self.doc.scene_label(scene)), self.doc.scene_number(scene))
                };
                let current = (start..end).contains(&self.cur_panel);
                let fill = if current {
                    theme::surface_active(crate::theme::accent(), if band == 0 { 0.45 } else { 0.7 })
                } else {
                    theme::barre(band == 0, number % 2 == 1)
                };
                painter.rect_filled(rect, 3.0, fill);
                painter.with_clip_rect(rect.shrink(1.0)).text(
                    pos2(rect.left() + 5.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(if band == 0 { 10.0 } else { 11.0 }),
                    if depasse { visuals.error_fg_color } else { visuals.text_color() },
                );
                runs.push((rect, start));
                i = end;
            }
        }

        // Un clic sur la piste d'un calque donne la main à ses clés (copier, coller, supprimer) ;
        // ailleurs dans le montage, aux plans.
        if resp.is_pointer_button_down_on() {
            self.key_focus = ui.input(|i| i.pointer.press_origin()).is_some_and(|p| {
                (p.y < r.top() + rows.background_top() && self.layer_row_hit(p, r.top(), &blocks, rows).is_some())
                    || (!self.cam_key_sel.is_empty() && self.montage_camera_key_at(p, r.top(), &blocks, ppf, rows).is_some())
            });
        }
        // Caméra au-dessus des plans, calques dessous (6f), puis les pistes son. La poignée de
        // calque tenue (sur le plan courant) ou survolée s'éclaire (chantier 5).
        // Le losange vert (clé de position) sous la souris : il passe avant les poignées du dessin dessous.
        let hover_pos_key = resp.hover_pos().and_then(|p| self.montage_pos_key_at(p, r.top(), &blocks, ppf, rows));
        // Le losange gris clair (cle d'opacite), en bas de la piste : meme regle (`opacitecleselui.rs`).
        let hover_op_key = resp.hover_pos().and_then(|p| self.montage_op_key_at(p, r.top(), &blocks, ppf, rows));
        let lit = match &self.handle_drag {
            Some(d) => Some((self.cur_panel, d.handle, true)),
            None if self.montage_drag.is_none() && self.cam_keys_drag.is_none() && self.keys_drag.is_none() && self.fx_keys_drag.is_none() && self.pos_ui.drag.is_none() && self.op_ui.drag.is_none() => resp
                .hover_pos()
                .filter(|p| hover_pos_key.is_none() && hover_op_key.is_none() && self.montage_fx_key_at(*p, r.top(), &blocks, ppf, rows).is_none())
                .and_then(|p| self.montage_layer_handle_at(p, r.top(), &blocks, ppf, rows))
                .map(|(i, _, h)| (i, h, false)),
            None => None,
        };
        let drops = self.drop_view();
        self.paint_track_rows(&painter, r, &blocks, ppf, rows, lit, &drops);
        self.paint_lanes(&painter, r, left, ppf, lanes_top, theme::contour());
        self.paint_transitions(&painter, &badges);
        // Un bruitage glissé depuis le panneau Bruitages : il se pose là où on le lâche.
        self.bruitage_drop(ui, &resp, left, ppf, lanes_top);

        let panel_at = |x: f32| blocks.iter().position(|b| x >= b.left() - 1.0 && x <= b.right() + 1.0);
        // Le bord d'un plan sous le point : son début ou sa fin (`rognageui::edge_hit`).
        let edge_at = |p: Pos2| {
            if !in_blocks(p) {
                return None;
            }
            crate::rognageui::edge_hit(&blocks, p.x)
        };
        let frame_at = |x: f32| (((x - left) / ppf).floor().max(0.0) as usize).min(total - 1);
        // Sans limite à la fin des plans (on peut poser un son plus loin).
        let free_frame_at = |x: f32| ((x - left) / ppf).floor().max(0.0) as usize;

        // Commentaires (`commentairesui.rs`) : leurs fanions dans la règle, le mât au milieu de
        // l'image du film. Un fanion pris garde la souris : la bande ne fait rien d'autre.
        let ruler_rect = Rect::from_min_max(r.min, pos2(r.right(), bands_top));
        let comments = self.doc.comment_positions(crate::commentaires::Scope::Film, &self.comment_ui.filter, self.comment_ui.sel);
        let comment_pole = |g: usize| x_of(g) + ppf * 0.5;
        let comment_ptr = self.comment_pointer(
            ui,
            &resp,
            &crate::commentairesui::RulerView { ruler: ruler_rect, placed: &comments, pole_x: &comment_pole, frame_at: &frame_at, scope: crate::commentaires::Scope::Film, plain: true },
        );

        if let Some(p) = resp.hover_pos() {
            if let Some((_, phrase)) = reperes.iter().find(|(z, _)| z.contains(p)).and_then(|(_, plan)| self.envoi_repere(*plan)) {
                resp.clone().on_hover_text_at_pointer(phrase);
            }
            if let Some(phrase) = prises_zones.iter().find(|(z, _)| z.contains(p)).and_then(|(_, plan)| self.pastille_phrase(*plan)) {
                resp.clone().on_hover_text_at_pointer(phrase);
            }
            if let Some((_, m)) = notes_zones.iter().find(|(z, _)| z.contains(p)) {
                resp.clone().on_hover_text_at_pointer(crate::fildecisionui::marque_phrase(m));
            }
            let clip_edge = matches!(
                self.lane_hit(p, left, ppf, lanes_top),
                Some((_, Some((_, edge)))) if edge.is_resize()
            );
            let resizing = matches!(&self.montage_drag, Some(MontageDrag::Edge(d)) if d.kind != crate::rognageui::EdgeKind::Slip)
                || matches!(&self.montage_drag, Some(MontageDrag::Clip(d)) if d.edge.is_resize());
            if edge_at(p).is_some() || clip_edge || resizing {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
            // Une transition : ses bords s'étirent, sa bulle dit sa sorte et sa durée.
            if let Some((i, hit)) = crate::transitionsui::badge_hit(&badges, p) {
                if hit != crate::transitionsui::BadgeHit::Body {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                let tip = crate::transitionsui::badge_tip(&badges[i], self.doc.transition(badges[i].id));
                resp.clone().on_hover_text_at_pointer(tip);
            }
            if matches!(&self.montage_drag, Some(MontageDrag::Edge(d)) if d.kind == crate::rognageui::EdgeKind::Slip) {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
            // Un losange de clé d'effet se prend (`fxkeys.rs`).
            if self.montage_fx_key_at(p, r.top(), &blocks, ppf, rows).is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
            if let (Some((i, k, f)), None) = (hover_op_key, &self.op_ui.drag) {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
                let layer = self.doc.panels[i].layers[k].name.clone();
                let name = format!("Cle d'opacite, image {} ({layer})", self.frame_label(f));
                let tip = "Glisser pour la deplacer, avec les cles choisies du calque (Echap : abandonner) ; Ctrl + clic : ajouter ou retirer du choix ; Maj + clic : ajouter celles d'entre ; Suppr : la supprimer ; clic droit : lisser, supprimer.";
                resp.clone().on_hover_ui_at_pointer(|ui| crate::bulles::contenu(ui, &name, tip, ""));
            }
            // Un losange vert de clé de position se prend (`poskeysui.rs`) : sa bulle dit comment.
            if let (Some((i, k, f)), None) = (hover_pos_key, &self.pos_ui.drag) {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
                let layer = self.doc.panels[i].layers[k].name.clone();
                let name = format!("Clé de position, image {} ({layer})", self.frame_label(f));
                let tip = "Glisser pour la déplacer, avec les clés choisies du calque (Échap : abandonner) ; Ctrl + clic : ajouter ou retirer du choix ; Maj + clic : ajouter celles d'entre ; Suppr : la supprimer ; clic droit : lisser, supprimer.";
                resp.clone().on_hover_ui_at_pointer(|ui| crate::bulles::contenu(ui, &name, tip, ""));
            }
            // Poignées des pistes de calques (chantier 5) : les bords s'étirent, la barre se prend.
            match lit.filter(|_| self.handle_drag.is_none()).map(|(_, h, _)| h) {
                Some(Handle::LayerStart { .. } | Handle::LayerEnd { .. } | Handle::Exposure { .. } | Handle::Boundary { .. }) => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                }
                Some(Handle::LayerMove { .. } | Handle::Key { .. }) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                _ => {}
            }
        }

        // Une étiquette de version « V2 » prend le geste qui part d'elle : un clic ouvre la pile (pour un stylet
        // qui ne survole pas), ni glissement ni double-clic ne traversent jusqu'au plan dessous.
        let on_label = |p: Option<Pos2>| p.is_some_and(|p| etiquettes.iter().any(|e| e.zone.contains(p)));
        let label_press = on_label(ui.input(|i| i.pointer.press_origin()));
        let label_click = (resp.clicked() || resp.double_clicked()) && on_label(resp.interact_pointer_pos());
        if label_click && resp.clicked() {
            if let Some(plan) = resp.interact_pointer_pos().and_then(|p| etiquettes.iter().find(|e| e.zone.contains(p))).map(|e| e.plan) {
                self.pile_epingler(plan);
            }
        }
        // Une transition prend le geste qui part de sa forme (ses bords changent sa durée).
        let trans_took = resp.drag_started()
            && !comment_ptr.busy
            && self.trans_begin_drag(ui.input(|i| i.pointer.press_origin()).unwrap_or(r.center()), &badges);
        if self.trans.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                self.trans_drag_to(free_frame_at(p.x));
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.trans_end_drag();
            }
        }
        if resp.drag_started() && !comment_ptr.busy && !trans_took && !label_press {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(r.center());
            // Sans recentrer la bande : le bord tiré doit rester sous le pointeur.
            self.playing = false;
            // Un geste qui ne part pas d'un losange violet lâche le choix des clés d'effets.
            let fx_hit = self.montage_fx_key_at(origin, r.top(), &blocks, ppf, rows);
            if fx_hit.is_none() {
                self.fx_key_sel.clear();
            }
            // Un geste qui ne part pas d'un losange vert (clé de position) lâche leur choix ; celui qui part
            // d'un losange passe avant tout ce qui est dessous (`poskeysui.rs`).
            let pos_hit = self.montage_pos_key_at(origin, r.top(), &blocks, ppf, rows);
            if pos_hit.is_none() && self.pos_ui.drag.is_none() {
                self.pos_ui.sel.clear();
            }
            let op_hit = self.montage_op_key_at(origin, r.top(), &blocks, ppf, rows);
            if op_hit.is_none() && self.op_ui.drag.is_none() {
                self.op_ui.sel.clear();
            }
            // Un geste qui ne part pas d'un losange jaune lâche le choix des clés de caméra.
            let cam_hit = self.montage_camera_key_at(origin, r.top(), &blocks, ppf, rows).and_then(|(i, k)| Some((i, self.doc.panels[i].camera.get(k)?.frame)));
            // (Un geste qui part de la piste Caméra, hors d'un losange, trace un rectangle de choix.)
            let cam_row = (r.top() + rows.camera_top()..=r.top() + rows.camera_top() + crate::montagerows::CAM_H).contains(&origin.y);
            if cam_hit.is_none() && !cam_row {
                self.clear_cam_keys();
            }
            if let Some((i, f)) = cam_hit {
                // Une clé de caméra : elle emmène toutes les clés de caméra choisies (`camkeys.rs`).
                self.sel_clip = None;
                let grab = free_frame_at(origin.x);
                self.begin_cam_keys_drag(i, f, grab);
            } else if cam_row {
                // Un rectangle de choix des clés de caméra (`camkeys::CamRect`) : Maj ajoute au choix,
                // Ctrl bascule les clés touchées.
                self.sel_clip = None;
                let mods = ui.input(|i| i.modifiers);
                self.begin_cam_rect(origin, mods.shift, mods.command && !mods.shift);
            } else if let Some((_, Some((id, edge)))) = self.lane_hit(origin, left, ppf, lanes_top) {
                self.begin_clip_drag(id, edge, origin);
            } else if let Some((i, k, f)) = fx_hit {
                // Une clé d'effet : elle emmène toutes les clés d'effets choisies (`fxkeys.rs`).
                self.sel_clip = None;
                self.key_focus = true;
                let grab = free_frame_at(origin.x);
                self.begin_fx_keys_drag(i, k, f, grab);
            } else if let Some((i, k, f)) = op_hit {
                // Une cle d'opacite : elle emmene toutes les cles choisies du calque, une etape d'annulation.
                self.sel_clip = None;
                let grab = (((origin.x - x_of(self.doc.panel_start(i))) / ppf).floor().max(0.0)) as usize;
                self.begin_op_drag_in_montage(i, k, f, grab);
            } else if let Some((i, k, f)) = pos_hit {
                // Une clé de position : elle emmène toutes les clés choisies du calque, une étape d'annulation.
                self.sel_clip = None;
                let grab = (((origin.x - x_of(self.doc.panel_start(i))) / ppf).floor().max(0.0)) as usize;
                self.begin_pos_drag_in_montage(i, k, f, grab);
            } else if let Some(rect) = {
                // Un choix au rectangle : depuis un espace vide des pistes de calques, ou avec Ctrl
                // ou Maj depuis n'importe où dans les pistes (`montagekeys::KeyRect`).
                let mods = ui.input(|i| i.modifiers);
                let held = mods.command || mods.shift;
                let in_tracks = self.row_at(origin, r.top(), rows).is_some();
                let free = self.montage_layer_handle_at(origin, r.top(), &blocks, ppf, rows).is_none();
                // Maj ajoute au choix, Ctrl bascule les clés touchées (comme pour la caméra).
                (in_tracks && (held || free)).then(|| crate::montagekeys::KeyRect { origin, before: self.key_sel.clone(), add: mods.shift, toggle: mods.command && !mods.shift })
            } {
                self.sel_clip = None;
                self.key_focus = true;
                self.key_rect = Some(rect);
            } else if let Some((i, k, h)) = self.montage_layer_handle_at(origin, r.top(), &blocks, ppf, rows) {
                // Les gestes de la timeline dans les pistes de calques (chantier 5) : ce plan, ce
                // calque, et sa poignée. Une clé choisie parmi d'autres emmène toutes les choisies.
                self.sel_clip = None;
                let group = match h {
                    Handle::Key { grab, .. } => self.key_sel.len() > 1 && self.key_ref(i, k, grab).is_some_and(|key| self.key_sel.contains(&key)),
                    _ => false,
                };
                if group {
                    self.begin_keys_drag(free_frame_at(origin.x));
                } else {
                    self.set_panel(i);
                    self.select_layer(k);
                    self.begin_handle_drag(h);
                }
            } else {
                let drag = if !in_blocks(origin) {
                    MontageDrag::Scrub
                } else if self.doc.storyboard_locked && (edge_at(origin).is_some() || panel_at(origin.x).is_some()) {
                    // Storyboard verrouillé (13e) : ni durée ni place des plans ne bougent.
                    self.status = crate::projetui::STORYBOARD_LOCKED.into();
                    MontageDrag::Scrub
                } else if let Some((i, side)) = edge_at(origin) {
                    // Un bord de plan (`rognageui.rs`) : le début ou la fin, qui rognent sans rien
                    // détruire et font suivre les plans d'après (ripple) ; avec Alt, entre deux
                    // plans, la coupe se déplace (roll) et le total ne change pas.
                    let alt = ui.input(|inp| inp.modifiers.alt);
                    let (kind, plan) = match side {
                        crate::rognageui::Side::Tail if alt && i + 1 < self.doc.panels.len() => (crate::rognageui::EdgeKind::Roll, i),
                        crate::rognageui::Side::Head if alt && i > 0 => (crate::rognageui::EdgeKind::Roll, i - 1),
                        crate::rognageui::Side::Head => (crate::rognageui::EdgeKind::Head, i),
                        _ => (crate::rognageui::EdgeKind::Tail, i),
                    };
                    self.begin_edge_drag(kind, plan, origin.x).unwrap_or(MontageDrag::Scrub)
                } else if let Some(i) = panel_at(origin.x) {
                    if ui.input(|inp| inp.modifiers.alt) {
                        // Alt + glisser dans un plan : son contenu glisse sous sa fenêtre.
                        self.begin_edge_drag(crate::rognageui::EdgeKind::Slip, i, origin.x).unwrap_or(MontageDrag::Scrub)
                    } else {
                        // Un panneau qui n'était pas choisi part seul ; sinon tout le groupe le suit.
                        if !self.board_selection().contains(&self.doc.panels[i].id) {
                            self.board_sel.clear();
                        }
                        self.set_panel(i);
                        MontageDrag::Move { panel: self.doc.panels[i].id, target: i, merge: None }
                    }
                } else {
                    MontageDrag::Scrub
                };
                self.montage_drag = Some(drag);
            }
        }

        if self.cam_keys_drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                self.drag_cam_keys(free_frame_at(p.x));
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_cam_keys_drag();
            }
        } else if resp.hover_pos().is_some_and(|p| self.montage_camera_key_at(p, r.top(), &blocks, ppf, rows).is_some()) {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        // Choix au rectangle des clés de caméra : seule la largeur du rectangle compte (la piste
        // est basse), il se dessine pendant le geste.
        if let Some(o) = self.cam_rect.as_ref().map(|c| c.origin) {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let rect = Rect::from_two_pos(o, p);
                let y = r.top() + rows.camera_top() + crate::montagerows::CAM_H * 0.5;
                let band = rect.union(Rect::from_center_size(pos2(rect.center().x, y), vec2(0.0, crate::montagerows::CAM_H)));
                let picked = crate::camkeys::pick_in_rect(&self.montage_cam_diamonds(&blocks, ppf, y), band, crate::camkeys::DIAMOND_HALF);
                self.drag_cam_rect(&picked);
                crate::camkeys::paint_marquee(&painter, rect);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_cam_rect();
            }
        }
        if let Some(h) = self.handle_drag.as_ref().map(|d| d.handle) {
            let resize = matches!(h, Handle::LayerStart { .. } | Handle::LayerEnd { .. } | Handle::Exposure { .. } | Handle::Boundary { .. });
            ui.ctx().set_cursor_icon(if resize { CursorIcon::ResizeHorizontal } else { CursorIcon::Grabbing });
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                // L'image sous la souris, et le bord d'image le plus proche (expositions, bouts).
                let x = ((p.x - x_of(self.doc.panel_start(self.cur_panel))) / ppf).max(0.0);
                self.drag_handle(x.floor() as usize, x.round() as usize);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_handle_drag();
            }
        }

        // Choix au rectangle : les pistes et les images couvertes ; le rectangle se dessine.
        if let Some(o) = self.key_rect.as_ref().map(|k| k.origin) {
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let row = |y: f32| rows.row_near(y - r.top() - rows.layers_top());
                let (a, b) = (row(o.y.min(p.y)), row(o.y.max(p.y)));
                let (g0, g1) = (free_frame_at(o.x.min(p.x)), free_frame_at(o.x.max(p.x)));
                self.drag_key_rect(a..=b, g0, g1);
                let accent = theme::contour();
                painter.rect(Rect::from_two_pos(o, p), 2.0, accent.gamma_multiply(0.15), Stroke::new(1.5_f32, accent), StrokeKind::Inside);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.key_rect = None;
                let n = self.key_sel.len();
                self.status = format!(
                    "{} choisie{} : Ctrl+C, Ctrl+V (insérer), Maj+Ctrl+V (sur la tête), Suppr, Ctrl+D ; une clé choisie prise par son milieu décale la suite avec elle, calque par calque (Maj : seules les clés choisies ; Alt : les copier).",
                    crate::montagekeys::key_counts_text(&self.key_sel),
                    if n > 1 { "s" } else { "" }
                );
            }
        }
        // Plusieurs clés choisies prises à la fois (`montagekeys`).
        if self.keys_drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                self.drag_keys(free_frame_at(p.x));
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_keys_drag();
            }
        }
        // Des clés d'effets prises à la souris (`fxkeys.rs`).
        if self.fx_keys_drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                self.drag_fx_keys(free_frame_at(p.x));
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_fx_keys_drag();
            }
        }
        // Des clés de position prises à la souris (`poskeysui.rs`) : l'image est celle du plan du calque.
        if self.pos_ui.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                let at = (((p.x - x_of(self.doc.panel_start(self.cur_panel))) / ppf).floor().max(0.0)) as usize;
                self.drag_pos_keys(at);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_pos_drag();
            }
        }
        if self.op_ui.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                let at = (((p.x - x_of(self.doc.panel_start(self.cur_panel))) / ppf).floor().max(0.0)) as usize;
                self.drag_op_keys(at);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_op_drag();
            }
        }
        if resp.dragged() && !comment_ptr.busy && self.cam_keys_drag.is_none() && self.handle_drag.is_none() && self.keys_drag.is_none() && self.fx_keys_drag.is_none() && self.pos_ui.drag.is_none() && self.op_ui.drag.is_none() && self.key_rect.is_none() {
            if let Some(p) = resp.interact_pointer_pos() {
                match &mut self.montage_drag {
                    Some(MontageDrag::Scrub) => {
                        let g = frame_at(p.x);
                        if g != self.global_frame() {
                            self.goto_global(g);
                        }
                    }
                    Some(MontageDrag::Move { panel, .. }) => {
                        let panel = *panel;
                        let slot = blocks.iter().filter(|b| b.center().x < p.x).count();
                        // Le milieu d'un plan (dans la hauteur des blocs) : fusionner (`fusionglisse.rs`).
                        let seul = self.board_selection().len() <= 1;
                        let onto = if in_blocks(p) { crate::fusionglisse::plan_vise(&self.doc, &blocks, p, panel, seul) } else { None };
                        if let Some(MontageDrag::Move { target, merge, .. }) = &mut self.montage_drag {
                            (*target, *merge) = (slot, onto);
                        }
                    }
                    Some(MontageDrag::Edge(_)) => self.update_edge_drag(p.x),
                    Some(MontageDrag::Clip(_)) => self.update_clip_drag(p, ppf, lanes_top),
                    None => {}
                }
            }
        }

        if resp.drag_stopped() {
            match self.montage_drag.take() {
                Some(MontageDrag::Edge(d)) => self.finish_edge_drag(d),
                Some(MontageDrag::Move { panel, merge: Some(onto), .. }) => self.fusionner_en_glissant(panel, onto),
                Some(MontageDrag::Move { panel, target, merge: None }) => {
                    let ids = self.board_selection();
                    if ids.len() > 1 {
                        self.move_chosen_panels(&ids, target);
                    } else {
                        self.move_chosen_panels(&[panel], target);
                    }
                }
                Some(MontageDrag::Clip(d)) => self.finish_clip_drag(d),
                _ => {}
            }
        }

        let trans_clicked = resp.clicked() && !comment_ptr.busy && resp.interact_pointer_pos().is_some_and(|p| self.trans_click(p, &badges));
        if resp.clicked() && !comment_ptr.busy && !trans_clicked && !label_click {
            self.trans.sel = None;
            // Le bord choisi pour les flèches ne l'est que par un clic sur un bord (ci-dessous).
            self.edge_sel = None;
            if let Some(p) = resp.interact_pointer_pos() {
                let fx_hit = self.montage_fx_key_at(p, r.top(), &blocks, ppf, rows);
                if fx_hit.is_none() {
                    self.fx_key_sel.clear();
                }
                // Un clic qui ne tombe pas sur un losange vert lâche le choix des clés de position.
                let pos_hit = self.montage_pos_key_at(p, r.top(), &blocks, ppf, rows);
                if pos_hit.is_none() {
                    self.pos_ui.sel.clear();
                }
                let op_hit = self.montage_op_key_at(p, r.top(), &blocks, ppf, rows);
                if op_hit.is_none() {
                    self.op_ui.sel.clear();
                }
                let cam_hit = self.montage_camera_key_at(p, r.top(), &blocks, ppf, rows).and_then(|(i, k)| Some((i, self.doc.panels[i].camera.get(k)?.frame)));
                if cam_hit.is_none() {
                    self.clear_cam_keys();
                }
                if let Some((i, f)) = cam_hit {
                    // Un losange de clé de caméra : choisi (Ctrl : ajouté ou retiré ; Maj : ceux
                    // d'entre), la tête de lecture va dessus (`camkeys.rs`).
                    self.sel_clip = None;
                    let mods = ui.input(|i| i.modifiers);
                    self.click_cam_key(i, f, mods.command, mods.shift);
                } else if let Some((i, k, f)) = fx_hit {
                    // Un losange de clé d'effet : choisi (Ctrl : ajouté ou retiré ; Maj : ceux d'entre),
                    // la tête de lecture va dessus (`fxkeys.rs`).
                    self.sel_clip = None;
                    let mods = ui.input(|i| i.modifiers);
                    self.click_fx_marker(i, k, f, mods.command, mods.shift);
                } else if let Some((i, k, f)) = op_hit {
                    // Un losange gris clair de cle d'opacite : choisi comme les autres (`opacitecleselui.rs`).
                    self.sel_clip = None;
                    let mods = ui.input(|i| i.modifiers);
                    self.click_op_marker_in_montage(i, k, f, mods.command, mods.shift && !mods.command);
                } else if let Some((i, k, f)) = pos_hit {
                    // Un losange vert de clé de position : choisi (Ctrl : ajouté ou retiré ; Maj : ceux
                    // d'entre), la tête de lecture va dessus (`poskeysui.rs`).
                    self.sel_clip = None;
                    let mods = ui.input(|i| i.modifiers);
                    self.click_pos_marker_in_montage(i, k, f, mods.command, mods.shift && !mods.command);
                } else if let Some((i, k)) = self.layer_row_hit(p, r.top(), &blocks, rows) {
                    // Clic dans une piste de calque : ce panneau, cette image, ce calque ; la clé
                    // cliquée est choisie (Ctrl : ajoutée ou retirée ; Maj : celles d'entre).
                    self.sel_clip = None;
                    let f = (((p.x - x_of(self.doc.panel_start(i))) / ppf).floor().max(0.0)) as usize;
                    let key = self.key_ref(i, k, f);
                    let mods = ui.input(|i| i.modifiers);
                    self.goto_global(frame_at(p.x));
                    if self.cur_panel == i {
                        self.select_layer(k);
                    }
                    self.click_key(key, mods.command, mods.shift);
                } else if let Some((_, hit)) = self.lane_hit(p, left, ppf, lanes_top) {
                    // Clic dans une piste son : sélectionne l'extrait (ou rien) et y place la tête ;
                    // une clé de volume touchée est choisie (Suppr la retire).
                    self.sel_clip = hit.map(|(id, _)| id);
                    self.sfx.key = match hit {
                        Some((id, ClipEdge::Key(i))) => Some((id, i)),
                        _ => None,
                    };
                    self.goto_global(frame_at(p.x));
                } else {
                    self.sel_clip = None;
                    self.key_sel.clear();
                    let mods = ui.input(|i| i.modifiers);
                    // Un clic sur un bord de plan le choisit : les flèches le déplacent d'une
                    // image (Alt : la coupe entre deux plans, `rognageui.rs`).
                    self.edge_sel = None;
                    if let Some((i, side)) = edge_at(p) {
                        let (plan, side) = match side {
                            crate::rognageui::Side::Tail if mods.alt && i + 1 < self.doc.panels.len() => (i, crate::rognageui::Side::Cut),
                            crate::rognageui::Side::Head if mods.alt && i > 0 => (i - 1, crate::rognageui::Side::Cut),
                            s => (i, s),
                        };
                        self.select_edge(plan, side);
                    }
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
        if resp.double_clicked() && !comment_ptr.busy && !label_click {
            if let Some(p) = resp.interact_pointer_pos() {
                if crate::transitionsui::badge_hit(&badges, p).is_some() {
                    // Sur une transition : le double-clic n'entre pas dans le plan dessous.
                } else if let Some(i) = in_blocks(p).then(|| panel_at(p.x)).flatten() {
                    self.enter_animation(i);
                } else if let Some(&(_, start)) = runs.iter().find(|(rect, _)| rect.contains(p)) {
                    // Double-clic sur la barre d'une séquence : son nom s'écrit (une petite fenêtre : `planchesequencesui.rs`).
                    self.board_sel.clear();
                    self.select_panel(start);
                    let seq = self.doc.sequence_of(self.cur_panel);
                    self.titre_ouvrir_fenetre(seq);
                } else if self.montage_fx_key_at(p, r.top(), &blocks, ppf, rows).is_some() {
                    // Sur un losange de clé d'effet : le double-clic ne pose pas de clé de dessin.
                } else if self.montage_pos_key_at(p, r.top(), &blocks, ppf, rows).is_some() {
                    // Ni sur un losange vert de clé de position.
                } else if let Some((i, k)) = self.layer_row_hit(p, r.top(), &blocks, rows).filter(|_| p.y < r.top() + rows.background_top()) {
                    // Double-clic sur la piste d'un calque : une nouvelle clé à cette image, copie du
                    // dessin comme F5.
                    let f = (((p.x - x_of(self.doc.panel_start(i))) / ppf).floor().max(0.0)) as usize;
                    self.montage_new_key(i, k, f, true);
                } else if let Some((track, Some((id, _)))) = self.lane_hit(p, left, ppf, lanes_top) {
                    // Double-clic sur un extrait de son : une clé de volume là où l'on a cliqué.
                    self.double_click_clip(track, id, p, left, ppf, lanes_top);
                }
            }
        }

        // Clic droit : menu du panneau ou de la piste son visés (d'un fanion de commentaire d'abord).
        if let Some(id) = comment_ptr.menu {
            self.menu_target = Some(MenuTarget::Comment(id));
        } else if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.menu_target = if let Some(target) = self.trans_menu_target(p, &badges, &blocks, top, top + block_h) {
                    // Une transition ou une coupe entre deux plans (`transitionsui.rs`).
                    Some(target)
                } else if let Some((track, hit)) = self.lane_hit(p, left, ppf, lanes_top) {
                    let clip = hit.map(|(id, _)| id);
                    if clip.is_some() {
                        self.sel_clip = clip;
                    }
                    self.doc.audio_tracks.get(track).map(|t| MenuTarget::Lane {
                        track: t.id,
                        clip,
                        frame: free_frame_at(p.x),
                    })
                } else if in_blocks(p) {
                    panel_at(p.x).map(|i| {
                        if !self.board_selection().contains(&self.doc.panels[i].id) {
                            self.board_sel.clear();
                        }
                        // Les commandes de rognage du menu visent ce plan et la tête de lecture
                        // d'avant le clic (choisir le plan la ramène à son début).
                        self.trim_menu = Some((self.doc.panels[i].id, self.global_frame()));
                        self.select_panel(i);
                        MenuTarget::Panel(self.doc.panels[i].id)
                    })
                } else if let Some((i, f)) = self.montage_camera_key_at(p, r.top(), &blocks, ppf, rows).and_then(|(i, k)| Some((i, self.doc.panels[i].camera.get(k)?.frame))) {
                    // Un losange de clé de caméra : le menu de la caméra, qui vise les clés choisies
                    // (celle-ci seule si elle n'était pas choisie).
                    self.sel_clip = None;
                    let this = crate::camkeys::CamKeyRef { panel: self.doc.panels[i].id, frame: f };
                    if !self.cam_key_sel.contains(&this) {
                        self.click_cam_key(i, f, false, false);
                    }
                    Some(MenuTarget::Camera)
                } else if let Some((i, k, f)) = self.montage_fx_key_at(p, r.top(), &blocks, ppf, rows) {
                    // Un losange de clé d'effet : le menu des clés d'effets choisies (celui-ci seul s'il
                    // n'était pas choisi).
                    self.sel_clip = None;
                    let all_chosen = self.fx_marker_refs(i, k, f).iter().all(|c| self.fx_key_sel.contains(c));
                    if !all_chosen {
                        self.click_fx_marker(i, k, f, false, false);
                    }
                    Some(MenuTarget::FxKey)
                } else if let Some((i, k, f)) = self.montage_op_key_at(p, r.top(), &blocks, ppf, rows) {
                    // Un losange d'opacite : le menu des cles d'opacite (celui-ci seul s'il n'etait pas choisi).
                    self.sel_clip = None;
                    let id = self.doc.panels[i].layers[k].id;
                    if i != self.cur_panel || !self.op_ui.sel.contains(&(id, f)) {
                        self.click_op_marker_in_montage(i, k, f, false, false);
                    }
                    self.key_focus = true;
                    Some(MenuTarget::OpKey)
                } else if let Some((i, k, f)) = self.montage_pos_key_at(p, r.top(), &blocks, ppf, rows) {
                    // Un losange vert de clé de position : le menu des clés de position (celui-ci seul
                    // s'il n'était pas choisi).
                    self.sel_clip = None;
                    let id = self.doc.panels[i].layers[k].id;
                    if i != self.cur_panel || !self.pos_ui.sel.contains(&(id, f)) {
                        self.click_pos_marker_in_montage(i, k, f, false, false);
                    }
                    self.key_focus = true;
                    Some(MenuTarget::PosKey)
                } else if let Some((i, k)) = self.layer_row_hit(p, r.top(), &blocks, rows) {
                    // Une piste de calque : ce plan, cette image, ce calque, et son menu. Une clé
                    // qui n'était pas choisie l'est seule.
                    self.sel_clip = None;
                    let f = (((p.x - x_of(self.doc.panel_start(i))) / ppf).floor().max(0.0)) as usize;
                    if let Some(key) = self.key_ref(i, k, f).filter(|key| !self.key_sel.contains(key)) {
                        self.key_sel = vec![key];
                        self.key_anchor = Some(key);
                    }
                    self.goto_global(frame_at(p.x));
                    (self.cur_panel == i).then(|| {
                        self.select_layer(k);
                        MenuTarget::Layer(self.layers()[k].id)
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
            // Le plan, la piste ou le calque visés, retrouvés par leur identifiant dans le film d'aujourd'hui ; s'ils n'y sont plus, le
            // menu se ferme plutôt que de viser leur voisin.
            let plan_of = |app: &App, id: u64| app.doc.panels.iter().position(|p| p.id == id);
            let track_of = |app: &App, id: u64| app.doc.audio_tracks.iter().position(|t| t.id == id);
            let layer_of = |app: &App, id: u64| app.layers().iter().position(|l| l.id == id);
            let menu_open = resp.context_menu(|ui| match target {
                MenuTarget::Panel(id) => match plan_of(self, id) {
                    Some(i) => self.panel_menu(ui, i),
                    None => {
                        self.menu_target = None;
                        ui.close_menu();
                    }
                },
                MenuTarget::Comment(id) => self.comment_menu(ui, id),
                MenuTarget::Transition(id) => self.transition_menu(ui, id),
                MenuTarget::Cut(cut) => self.cut_menu(ui, cut),
                MenuTarget::Lane { track, clip, frame } => match track_of(self, track) {
                    Some(t) => self.lane_menu(ui, t, clip, frame),
                    None => {
                        self.menu_target = None;
                        ui.close_menu();
                    }
                },
                MenuTarget::Layer(id) => {
                    let Some(k) = layer_of(self, id) else {
                        self.menu_target = None;
                        ui.close_menu();
                        return;
                    };
                    // La piste d'un calque : les clés à l'image du clic, puis le menu du calque.
                    let resp = ui.add(egui::Button::new("Nouvelle clé ici (copie du dessin)").shortcut_text(self.keymap.label("dessin.nouvelle_cle")));
                    if crate::bulles::bulle(resp, "Nouvelle clé ici", "Elle part du dessin montré ici (sinon du dernier d'avant). Ou double-clic sur la piste.", "").clicked() {
                        ui.close_menu();
                        let (pi, f) = (self.cur_panel, self.cur_frame);
                        self.montage_new_key(pi, k, f, true);
                    }
                    if ui.add(egui::Button::new("Nouvelle clé vide ici").shortcut_text(self.keymap.label("dessin.nouvelle_cle_vide"))).clicked() {
                        ui.close_menu();
                        let (pi, f) = (self.cur_panel, self.cur_frame);
                        self.montage_new_key(pi, k, f, false);
                    }
                    self.key_focus = true;
                    // Les cinq commandes communes aux trois sortes de clés (`clescommun.rs`), puis
                    // ce qui est propre aux clés de dessins.
                    self.key_menu_core(ui, crate::clescommun::KeyKind::Drawing);
                    let n = self.key_sel.len().max(1);
                    let what = |one: &str, many: &str| if n > 1 { format!("{many} ({n})") } else { one.to_string() };
                    let dup = self.keymap.label("dessin.dupliquer");
                    let resp = ui.add(egui::Button::new(what("Dupliquer la clé sur place", "Dupliquer les clés sur place")).shortcut_text(dup));
                    if crate::bulles::bulle(resp, "Dupliquer sur place", "La copie prend la seconde moitié de l'exposition ; la suite ne bouge pas.", "").clicked() {
                        ui.close_menu();
                        self.duplicate_keys();
                    }
                    // Aplat intelligent (`aplatintel.rs`) : le calque de cette piste en a un.
                    if self.layers().get(k).and_then(|l| l.fond).is_some_and(|f| f.intelligent) {
                        let resp = ui.add(egui::Button::new(what("Colorier comme la clé précédente", "Colorier les clés comme la précédente")).shortcut_text(self.keymap.label("aplat.colorier_comme_precedente")));
                        if crate::bulles::bulle(resp, "Colorier comme la clé précédente", "Les clés choisies prennent les couleurs de la clé coloriée d'avant, zone par zone (en chaîne s'il y en a plusieurs).", "").clicked() {
                            ui.close_menu();
                            self.colorier_comme_precedente();
                        }
                    }
                    let resp = ui.button(what("Supprimer la clé en laissant un vide", "Supprimer les clés en laissant un vide"));
                    if crate::bulles::bulle(resp, "Laisser un vide", "Leurs images restent vides ; les autres clés ne bougent pas.", "").clicked() {
                        ui.close_menu();
                        self.delete_keys(true);
                    }
                    ui.separator();
                    // Tout ce qui touche au calque, comme au clic droit sur son nom.
                    let name = self.layers().get(k).map(|l| l.name.clone()).unwrap_or_default();
                    ui.menu_button(format!("Calque « {name} »"), |ui| self.montage_layer_menu(ui, k));
                }
                MenuTarget::Camera => self.grid_menu_ui(ui, crate::handles::GridMenu::Camera),
                MenuTarget::FxKey => self.fx_key_menu(ui),
                MenuTarget::PosKey => self.pos_key_menu(ui),
                MenuTarget::OpKey => self.op_key_menu(ui),
                MenuTarget::Sequence => {
                    if ui.button("Renommer la séquence…").clicked() {
                        ui.close_menu();
                        let seq = self.doc.sequence_of(self.cur_panel);
                        self.titre_ouvrir_fenetre(seq);
                    }
                    ui.separator();
                    self.action_item(ui, "Nouvelle séquence", "sequence.nouvelle");
                    self.action_item(ui, "Nouveau plan", "panneau.nouveau");
                    self.action_item(ui, "Nouveau plan avant le plan courant", "plan.nouveau_avant");
                    ui.separator();
                    self.variantes_sous_menu(ui);
                    ui.separator();
                    self.action_item(ui, "Verrouiller / déverrouiller le storyboard", "storyboard.verrouiller");
                }
                MenuTarget::Ruler => {
                    self.action_item(ui, "Marque d'image ici (ajouter / retirer)", "marque.basculer");
                    self.action_item(ui, "Couper le plan ici", "panneau.couper");
                    self.action_item(ui, "Signet du plan ici (poser / retirer)", "signet.basculer");
                    self.action_item(ui, "Commentaire ici", "commentaire.ajouter");
                    ui.separator();
                    self.action_item(ui, "Raccourcir le début du plan jusqu'ici", "montage.debut_a_tete");
                    self.action_item(ui, "Raccourcir la fin du plan jusqu'ici", "montage.fin_a_tete");
                    ui.separator();
                    self.action_item(ui, "Importer un son ici…", "son.importer");
                }
            });
            // Le menu est fermé : les commandes de rognage visent de nouveau le plan courant.
            if menu_open.is_none() && !resp.secondary_clicked() {
                self.trim_menu = None;
            }
        }

        // La pile des versions, au-dessus du bloc dont on survole l'étiquette (pas pendant un geste de la bande).
        let occupe = self.montage_drag.is_some()
            || self.handle_drag.is_some()
            || self.cam_keys_drag.is_some()
            || self.keys_drag.is_some()
            || self.fx_keys_drag.is_some()
            || self.pos_ui.drag.is_some()
            || self.op_ui.drag.is_some()
            || self.key_rect.is_some()
            || self.cam_rect.is_some()
            || self.trans.drag.is_some();
        self.pile_versions_ui(ui, &etiquettes, occupe, painter.clip_rect());

        // Tête de lecture et repère de déplacement.
        let gx = x_of(self.global_frame());
        painter.line_segment([pos2(gx, r.top()), pos2(gx, r.bottom())], Stroke::new(2.0_f32, cur_frame()));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(gx - 5.0, r.top()), pos2(gx + 5.0, r.top()), pos2(gx, r.top() + 7.0)],
            cur_frame(),
            Stroke::NONE,
        ));
        if let Some(MontageDrag::Move { target, merge, .. }) = &self.montage_drag {
            let cible = merge.and_then(|id| self.doc.panels.iter().position(|p| p.id == id)).and_then(|i| blocks.get(i));
            if let Some(b) = cible {
                // Lâché là : fusionné. Le plan visé s'entoure et le dit (au lieu du repère de place).
                painter.rect_filled(*b, 4.0, theme::contour().gamma_multiply(0.18));
                painter.rect_stroke(*b, 4.0, Stroke::new(3.0_f32, theme::contour()), StrokeKind::Inside);
                let galley = painter.layout_no_wrap("Fusionner".into(), FontId::proportional(13.0), theme::contour());
                let pill = Rect::from_center_size(b.center(), galley.size() + vec2(14.0, 6.0));
                painter.rect_filled(pill, 4.0, visuals.window_fill());
                painter.rect_stroke(pill, 4.0, Stroke::new(1.0_f32, theme::contour()), StrokeKind::Inside);
                painter.galley(pill.center() - galley.size() * 0.5, galley, theme::contour());
            } else {
                let x = match blocks.get(*target) {
                    Some(b) => b.left() - 1.0,
                    None => blocks.last().map_or(left, |b| b.right() + 1.0),
                };
                painter.line_segment(
                    [pos2(x, top - 2.0), pos2(x, top + block_h + 2.0)],
                    Stroke::new(3.0_f32, theme::contour()),
                );
            }
        }
        // Rognage en cours : le repère de la coupe dans le film d'avant le geste, et la durée.
        if matches!(self.montage_drag, Some(MontageDrag::Edge(_))) {
            self.paint_edge_drag(&painter, &blocks, &x_of, top, top + block_h, theme::contour());
        }
        // Les fanions des commentaires, par-dessus la tête de lecture.
        crate::commentairesui::paint_flags(&painter, ruler_rect, &comments, &comment_pole, self.comment_ui.sel, comment_ptr.hover);

        if self.following {
            ui.scroll_to_rect(
                Rect::from_center_size(pos2(gx, top + block_h * 0.5), vec2(ppf * 24.0, 10.0)),
                None,
            );
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
        let scene_number = self.doc.plan_code(scene);
        let seq_number = self.doc.sequence_code(seq);
        ui.add_space(4.0);
        ui.heading(crate::planversui::titre_long_avec_version(&self.doc, i));
        self.kitsu_etiquette(ui, i);
        // Chaque champ modifie une copie ; `edited` enregistre l'étape d'annulation avant qu'on
        // reporte la valeur (une étape par saisie).
        egui::Grid::new("fiche_panneau").num_columns(2).show(ui, |ui| {
            ui.label("Séquence");
            if let Some(mut name) = self.doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()) {
                let resp = ui
                    .add(
                        egui::TextEdit::singleline(&mut name)
                            .hint_text(seq_number)
                            .desired_width(crate::dockui::fit_w(ui, 150.0)),
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
                            .desired_width(crate::dockui::fit_w(ui, 150.0)),
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
            ui.label("Version");
            self.version_ligne(ui);
            ui.end_row();
            // Durées cibles (`budget.rs`) : la séquence du plan et le film, l'écart en rouge quand ça dépasse.
            let fps_budget = self.doc.fps;
            for (nom, budget) in [("Séquence", self.doc.budget_de_sequence(self.doc.sequence_of(i))), ("Film", self.doc.budget_du_film())] {
                let Some(b) = budget else { continue };
                ui.label(format!("Cible {}", nom.to_lowercase()));
                if b.depasse() {
                    ui.colored_label(ui.visuals().error_fg_color, format!("{} : trop long", b.texte(fps_budget)));
                } else {
                    ui.label(RichText::new(b.texte(fps_budget)).weak());
                }
                ui.end_row();
            }
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
            let etait_vide = text.trim().is_empty();
            let resp = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY),
            );
            if self.edited(&resp) {
                // Un premier texte dans ce champ : sa ligne se coche sous les cases des Vignettes, pour qu'il s'y voie (partie 106).
                if etait_vide && !text.trim().is_empty() {
                    use crate::boardsens::Champ;
                    self.vignettes_montrer_champ([Champ::Action, Champ::Dialogue, Champ::Notes][field]);
                }
                let p = &mut self.doc.panels[i];
                match p.marks.iter_mut().find(|m| Some(m.id) == mark) {
                    Some(m) => *[&mut m.action, &mut m.dialogue, &mut m.notes][field] = text,
                    None => *[&mut p.action, &mut p.dialogue, &mut p.notes][field] = text,
                }
            }
        }
    }
}
