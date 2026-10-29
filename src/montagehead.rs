//! Colonne de gauche du montage : sa largeur et les réglages des calques (Anthony, 2026-10-17 :
//! « ajoute la possibilité d'étirer la fenêtre caméra et calques en ajoutant [à côté des noms]
//! les options de calques, options de fusion, opacité, option d'animation en boucle, etc. En
//! clic droit tout ce qui a trait aux calques. […] une barre d'étirage pour laisser la place
//! aux options »).
//! - Une barre au bord droit de la colonne l'élargit ou la rétrécit ; la largeur est retenue
//!   dans les réglages. Double-clic : tout montrer, ou revenir à la colonne étroite.
//! - La rangée d'un calque ne montre que l'œil, la pastille de sa sorte et le nom (`pistes::montage_head`, 2026-10-28) : l'opacité,
//!   le verrou, le mode de fusion, la pelure d'oignon et le reste sont au clic droit (`montage_layer_menu`) et dans le panneau
//!   Calques. La largeur sert aux noms longs.
//! - Clic droit sur la rangée : tout ce qui touche au calque (`montage_layer_menu`).


use eframe::egui::{self, pos2, CursorIcon, Rect, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::blend::BlendMode;
use crate::bulles::bulle;
use crate::document::{Behaviour, LayerKind};
use crate::soundtrack::HEADERS_W;

/// Largeur la plus petite de la colonne : celle d'avant (les noms seuls).
pub(crate) const HEAD_MIN: f32 = HEADERS_W;
/// Largeur la plus grande.
pub(crate) const HEAD_MAX: f32 = 520.0;
/// Largeur où tout se voit (double-clic sur la barre).
pub(crate) const HEAD_ALL: f32 = 330.0;
/// Largeur de la barre qu'on tire, au bord droit de la colonne.
const GRIP_W: f32 = 6.0;

/// Largeur de colonne permise (une valeur lue dans les réglages peut être n'importe quoi).
pub(crate) fn clamp_head_w(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(HEAD_MIN, HEAD_MAX)
    } else {
        HEAD_MIN
    }
}

/// Icône d'un pré / post-comportement.
pub(crate) fn behaviour_icon(b: Behaviour) -> &'static str {
    match b {
        Behaviour::Nothing => ph::MINUS,
        Behaviour::Loop => ph::REPEAT,
        Behaviour::PingPong => ph::ARROWS_LEFT_RIGHT,
        Behaviour::Hold => ph::PAUSE,
    }
}

impl App {
    /// La barre qui étire la colonne `column` (son bord droit), posée après tout le reste de la
    /// colonne pour rester au-dessus.
    pub(crate) fn head_width_grip(&mut self, ui: &mut egui::Ui, column: Rect) {
        let grip = Rect::from_min_max(pos2(column.right() - GRIP_W, column.top()), pos2(column.right(), column.bottom()));
        let resp = ui.interact(grip, ui.id().with("etirer_colonne_montage"), Sense::click_and_drag());
        let hot = resp.hovered() || resp.dragged();
        let color = if hot { ui.visuals().selection.stroke.color } else { ui.visuals().widgets.noninteractive.bg_stroke.color };
        let x = column.right() - GRIP_W * 0.5;
        ui.painter().line_segment([pos2(x, column.top()), pos2(x, column.bottom())], Stroke::new(if hot { 2.0_f32 } else { 1.0 }, color));
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        if resp.dragged() {
            self.montage_head_w = clamp_head_w(self.montage_head_w + resp.drag_delta().x);
        }
        let resp = bulle(
            resp,
            "Largeur de la colonne",
            "Tirer pour élargir la colonne et lire les noms longs des calques. Double-clic : l'élargir, ou revenir à la colonne étroite.",
            "",
        );
        if resp.double_clicked() {
            self.montage_head_w = if self.montage_head_w < HEAD_ALL - 1.0 { HEAD_ALL } else { HEAD_MIN };
        }
        if resp.drag_stopped() || resp.double_clicked() {
            self.settings.montage_head_w = self.montage_head_w;
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
    }

    /// Tout ce qui touche au calque `li` du plan courant (clic droit dans la colonne du montage,
    /// son ⋮, ou « Calque » au clic droit d'une piste) : ses réglages, avant / après ses images,
    /// son groupe, le renommer, le dupliquer, le supprimer, puis toutes les commandes des calques.
    ///
    /// Une dizaine d'entrées au premier niveau (simplification des barres, 2026-10-21) : les quatre gestes du calque,
    /// l'aplat, les effets, puis « Réglages du calque » (afficher, opacité, mode de fusion, pelure d'oignon, pochoir,
    /// profondeur, groupe), « Animation » et « Plus » (pistes, propagation, coloriage, vidéo, calque de couleur, toutes
    /// les commandes des calques). Rien n'est retiré : tout y est, un sous-menu plus loin. (Le menu du panneau Calques,
    /// lui, est `layer_full_menu` avec `in_montage` faux.)
    pub(crate) fn montage_layer_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(l) = self.layers().get(li) else {
            ui.close_menu();
            return;
        };
        let (folder, video) = (l.folder, l.video.is_some());
        self.rename_item(ui, li, false, true);
        self.duplicate_lock_delete_items(ui, li);
        // Plusieurs calques choisis (Ctrl ou Maj + clic) : les fusionner.
        let chosen = self.chosen_layers().len();
        if chosen > 1 && ui.button(format!("Fusionner les {chosen} calques choisis")).clicked() {
            ui.close_menu();
            self.merge_chosen_layers();
        }
        ui.separator();
        // Aplat sous le trait (`fondui.rs`) : « Ajouter un aplat » / « Retirer l'aplat ».
        self.aplat_item(ui, li);
        // Les effets par familles (un sous-menu) et le tremblement du trait.
        self.layer_fx_items(ui, li);
        ui.menu_button("Réglages du calque", |ui| self.montage_layer_settings(ui, li));
        if !folder {
            ui.menu_button("Animation (avant, après, automatismes)", |ui| self.behaviour_items(ui, li));
        }
        ui.menu_button("Plus", |ui| {
            // Replier / déplier les pistes du montage (`pistes.rs`).
            self.pistes_menu(ui, li);
            self.propagation_item(ui, li);
            self.coloriage_item(ui, li);
            if video {
                ui.menu_button("Calque vidéo", |ui| self.video_menu(ui, li));
            }
            // Calque de couleur sous le trait (`calquecouleurui.rs`) : en ajouter un, ou ses réglages (griffonner, sources).
            if !folder && !video {
                ui.menu_button("Calque de couleur", |ui| self.couleur_menu(ui, li));
            }
            ui.menu_button("Toutes les commandes des calques", |ui| self.layer_ops_menu_from(ui, crate::layerops::OpsFrom::LayerMenu));
        });
    }

    /// « Réglages du calque » du menu du montage : afficher, opacité, mode de fusion, pelure d'oignon du calque, puis
    /// préserver la transparence, pochoir, position verrouillée, profondeur et groupe.
    fn montage_layer_settings(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        let (visible, onion, keep_alpha, stencil, negative, fixed, opacity, blend, folder) =
            (l.visible, l.onion, l.keep_alpha, l.stencil, l.stencil_negative, l.position_locked, l.opacity, l.blend, l.folder);
        if ui.checkbox(&mut visible.clone(), "Afficher").clicked() {
            self.toggle_layer_visible(li);
        }
        if !folder && ui.checkbox(&mut onion.clone(), "Pelure d'oignon du calque").clicked() {
            self.set_layer_flag(li, |l| l.onion = !onion);
        }
        let mut pct = opacity * 100.0;
        let resp = ui.horizontal(|ui| {
            ui.label("Opacité");
            ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").fixed_decimals(0))
        });
        if self.edited(&resp.inner) {
            self.layers_mut()[li].opacity = (pct / 100.0).clamp(0.0, 1.0);
            self.renderer.invalidate_params();
        }
        if !folder {
            ui.menu_button(format!("Mode de fusion : {}", blend.label()), |ui| {
                for m in BlendMode::ALL {
                    if BlendMode::FAMILY_STARTS.contains(&m) {
                        ui.separator();
                    }
                    if ui.radio(m == blend, m.label()).on_hover_text(m.hint()).clicked() {
                        ui.close_menu();
                        self.set_layer_flag(li, |l| l.blend = m);
                    }
                }
            });
            if ui.checkbox(&mut keep_alpha.clone(), "Préserver la transparence").on_hover_text("Le pinceau ne colore que ce qui est déjà dessiné : le vide reste vide.").clicked() {
                self.set_layer_flag(li, |l| l.keep_alpha = !keep_alpha);
            }
            if ui.checkbox(&mut stencil.clone(), "Pochoir").on_hover_text("Le dessin de ce calque limite où l'on peint sur les autres calques.").clicked() {
                self.set_layer_flag(li, |l| l.stencil = !stencil);
            }
            if ui
                .add_enabled(stencil, egui::Checkbox::new(&mut negative.clone(), "Pochoir négatif"))
                .on_hover_text("Positif : on ne peint que dans la forme du pochoir. Négatif : seulement en dehors.")
                .clicked()
            {
                self.set_layer_flag(li, |l| l.stencil_negative = !negative);
            }
            if ui
                .checkbox(&mut fixed.clone(), "Verrouiller la position")
                .on_hover_text("L'outil Transformer et le déplacement en 3D ne bougent plus le calque, et on ne peut plus dessiner dessus.")
                .clicked()
            {
                self.set_layer_flag(li, |l| l.position_locked = !fixed);
            }
        }
        self.depth_item(ui, li);
        ui.menu_button("Groupe", |ui| self.group_menu(ui, li));
    }

    /// Le même menu, depuis le montage (`in_montage`) ou le panneau Calques. Dans le panneau, rien
    /// de ce qu'il montre déjà (Anthony, 2026-10-17 : « ne doit pas faire doublon avec les options
    /// déjà disponibles dans la fenêtre de calques ») : ni afficher, ni opacité, ni mode de fusion
    /// (son en-tête), ni dupliquer, supprimer, verrouiller (ses boutons et son verrou) ; le nom s'y
    /// renomme. Le sous-menu des commandes ne répète pas non plus ce que le menu a déjà
    /// (`OpsFrom`).
    ///
    /// « Propager la peinture… » (`propagerui.rs`) pour le calque `li` : toujours dans le menu,
    /// grisé avec la raison quand ce calque ne peut pas être propagé (Anthony, 2026-10-17 : « je
    /// ne trouve plus la propagation de couleur » : l'entrée n'apparaissait que sur un calque
    /// bitmap ayant un autre calque à suivre, et le calque par défaut est vectoriel).
    pub(crate) fn propagation_item(&mut self, ui: &mut egui::Ui, li: usize) {
        let blocker = match self.layers().get(li) {
            None => Some("Aucun calque."),
            Some(l) if l.video.is_some() => Some("C'est un calque vidéo : peins sur un calque bitmap au-dessus, puis propage ce calque-là."),
            Some(l) if l.folder || l.kind != LayerKind::Bitmap => {
                Some("La propagation se fait sur un calque bitmap : crée-en un (Calques > Nouveau calque > Bitmap) au-dessus de la vidéo ou de l'animation, et peins dessus.")
            }
            Some(_) if crate::propager::guides(&self.doc, self.cur_panel, li).is_empty() => Some("Rien à suivre : il faut un autre calque dans le plan (vidéo, animation…)."),
            Some(_) => None,
        };
        if blocker.is_none() && li == self.cur_layer {
            // L'entrée de `propagerui` (elle agit sur le calque courant).
            self.propagation_menu(ui, li);
            return;
        }
        ui.separator();
        let shortcut = self.keymap.label("calque.propager");
        let resp = ui.add_enabled(blocker.is_none(), egui::Button::new("Propager la peinture…").shortcut_text(&shortcut));
        if let Some(why) = blocker {
            resp.on_disabled_hover_text(why);
        } else if resp.clicked() {
            ui.close_menu();
            self.select_layer(li);
            self.run_action("calque.propager");
        }
    }

    /// Dans l'ordre (Anthony, 2026-10-17 : « en premier les fonctions simples, puis les fonctions
    /// d'état, ensuite les fonctions d'animation ») : renommer, dupliquer, verrouiller, supprimer ;
    /// afficher, pelure, transparence, pochoir, position, profondeur, groupe ; avant / après ses
    /// images, cassure et création automatiques ; effets (opacité, mode, effets non destructifs
    /// par familles, fenêtre Effets, propagation) ; puis le reste.
    pub(crate) fn layer_full_menu(&mut self, ui: &mut egui::Ui, li: usize, in_montage: bool) {
        let Some(l) = self.layers().get(li) else {
            ui.close_menu();
            return;
        };
        let (visible, onion, keep_alpha, stencil, negative, fixed, opacity, blend, folder) =
            (l.visible, l.onion, l.keep_alpha, l.stencil, l.stencil_negative, l.position_locked, l.opacity, l.blend, l.folder);
        let video = l.video.is_some();
        // (Pas de titre « Calque « nom » » : une ligne de moins, le menu s'ouvre sur le calque cliqué ; ses entrées sont
        // la dizaine du clic droit, simplification des barres, 2026-10-21.)
        // Fonctions simples.
        self.rename_item(ui, li, false, in_montage);
        if in_montage {
            self.duplicate_lock_delete_items(ui, li);
        }
        // Aplat sous le trait (`fondui.rs`) : « Ajouter un aplat » / « Retirer l'aplat », partout où le menu s'ouvre.
        self.aplat_item(ui, li);
        // Copier / coller le calque (ou les calques choisis) : dans le panneau Calques seulement ; Ctrl+C et Ctrl+V font de même
        // quand le panneau a la main (2026-10-29).
        if !in_montage {
            let n = self.chosen_layers().len();
            let (copier, titre) = if n > 1 { (format!("Copier les {n} calques choisis"), "Copier, coller les calques") } else { ("Copier le calque".to_string(), "Copier, coller le calque") };
            ui.menu_button(titre, |ui| {
                self.action_item(ui, &copier, "calque.copier");
                self.action_item(ui, "Coller le calque", "calque.coller");
            });
        }
        // Plusieurs calques choisis (Ctrl ou Maj + clic) : les fusionner (Anthony, 2026-10-17). Dans
        // le panneau Calques, c'est son bouton Fusionner.
        let chosen = self.chosen_layers().len();
        if in_montage && chosen > 1 && ui.button(format!("Fusionner les {chosen} calques choisis")).clicked() {
            ui.close_menu();
            self.merge_chosen_layers();
        }
        // Replier / déplier les pistes du montage (`pistes.rs`).
        if in_montage {
            self.pistes_menu(ui, li);
        }
        // Ce qui sert souvent (menu court, Anthony, 2026-10-18) : afficher, pelure, opacité, mode
        // de fusion (dans le panneau Calques, son en-tête les a), les effets non destructifs par
        // familles, la propagation de la peinture ; les autres réglages d'état et d'animation dans
        // deux sous-menus.
        ui.separator();
        if in_montage && ui.checkbox(&mut visible.clone(), "Afficher").clicked() {
            let _ = visible;
            self.toggle_layer_visible(li);
        }
        if !folder && ui.checkbox(&mut onion.clone(), "Pelure d'oignon du calque").clicked() {
            self.set_layer_flag(li, |l| l.onion = !onion);
        }
        if in_montage {
            let mut pct = opacity * 100.0;
            let resp = ui.horizontal(|ui| {
                ui.label("Opacité");
                ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").fixed_decimals(0))
            });
            if self.edited(&resp.inner) {
                self.layers_mut()[li].opacity = (pct / 100.0).clamp(0.0, 1.0);
                self.renderer.invalidate_params();
            }
        }
        if !folder {
            if in_montage {
                ui.menu_button(format!("Mode de fusion : {}", blend.label()), |ui| {
                    for m in BlendMode::ALL {
                        if BlendMode::FAMILY_STARTS.contains(&m) {
                            ui.separator();
                        }
                        if ui.radio(m == blend, m.label()).on_hover_text(m.hint()).clicked() {
                            ui.close_menu();
                            self.set_layer_flag(li, |l| l.blend = m);
                        }
                    }
                });
            }
            // Les effets par familles, puis le tremblement du trait (`tremblement.rs`) à découvert :
            // grisé avec la raison sur un calque bitmap (`fxui::offer`).
            self.layer_fx_items(ui, li);
        }
        // (Propager la peinture et Colorier par zones sont dans « Autres commandes des calques », en dernier.)
        // Fonctions d'état, puis d'animation (Anthony, 2026-10-17 : « en premier les fonctions
        // simples, puis les fonctions d'état, ensuite les fonctions d'animation »).
        ui.separator();
        ui.menu_button("Réglages du calque", |ui| {
            if !folder {
                if ui
                    .checkbox(&mut keep_alpha.clone(), "Préserver la transparence")
                    .on_hover_text("Le pinceau ne colore que ce qui est déjà dessiné : le vide reste vide.")
                    .clicked()
                {
                    self.set_layer_flag(li, |l| l.keep_alpha = !keep_alpha);
                }
                if ui.checkbox(&mut stencil.clone(), "Pochoir").on_hover_text("Le dessin de ce calque limite où l'on peint sur les autres calques.").clicked() {
                    self.set_layer_flag(li, |l| l.stencil = !stencil);
                }
                if ui
                    .add_enabled(stencil, egui::Checkbox::new(&mut negative.clone(), "Pochoir négatif"))
                    .on_hover_text("Positif : on ne peint que dans la forme du pochoir. Négatif : seulement en dehors.")
                    .clicked()
                {
                    self.set_layer_flag(li, |l| l.stencil_negative = !negative);
                }
                if ui
                    .checkbox(&mut fixed.clone(), "Verrouiller la position")
                    .on_hover_text("L'outil Transformer et le déplacement en 3D ne bougent plus le calque, et on ne peut plus dessiner dessus.")
                    .clicked()
                {
                    self.set_layer_flag(li, |l| l.position_locked = !fixed);
                }
            }
            self.depth_item(ui, li);
            ui.menu_button("Groupe", |ui| self.group_menu(ui, li));
        });
        if !folder {
            ui.menu_button("Animation (avant, après, automatismes)", |ui| self.behaviour_items(ui, li));
        }
        if video {
            ui.menu_button("Calque vidéo", |ui| self.video_menu(ui, li));
        }
        // Calque de couleur sous le trait (`calquecouleurui.rs`) : en ajouter un, ou ses réglages (griffonner, sources).
        if !folder && !video {
            ui.menu_button("Calque de couleur", |ui| self.couleur_menu(ui, li));
        }
        let (label, from) = if in_montage {
            ("Toutes les commandes des calques", crate::layerops::OpsFrom::LayerMenu)
        } else {
            ("Autres commandes des calques", crate::layerops::OpsFrom::LayersPanel)
        };
        ui.menu_button(label, |ui| {
            self.propagation_item(ui, li);
            self.coloriage_item(ui, li);
            ui.separator();
            self.layer_ops_menu_from(ui, from);
        });
    }
}
