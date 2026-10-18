//! Colonne de gauche du montage : sa largeur et les réglages des calques (Anthony, 2026-10-17 :
//! « ajoute la possibilité d'étirer la fenêtre caméra et calques en ajoutant [à côté des noms]
//! les options de calques, options de fusion, opacité, option d'animation en boucle, etc. En
//! clic droit tout ce qui a trait aux calques. […] une barre d'étirage pour laisser la place
//! aux options »).
//! - Une barre au bord droit de la colonne l'élargit ou la rétrécit ; la largeur est retenue
//!   dans les réglages. Double-clic : tout montrer, ou revenir à la colonne étroite.
//! - La rangée d'un calque montre ce qui tient dans sa largeur : en haut, le nom, puis afficher,
//!   verrouiller, pelure d'oignon et le menu ⋮ ; en bas, l'opacité, le mode de fusion, puis ce
//!   que fait le calque après sa dernière image (rien, boucle, va-et-vient, maintenir : le
//!   post-comportement, 13c). Le reste (avant la première image, pochoir, groupe…) est au clic
//!   droit.
//! - Clic droit sur la rangée (ou ⋮) : tout ce qui touche au calque (`montage_layer_menu`).

use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::blend::BlendMode;
use crate::bulles::bulle;
use crate::document::{Behaviour, LayerKind};
use crate::layerheads::{blend_combo, icon_at};
use crate::soundtrack::HEADERS_W;

/// Largeur la plus petite de la colonne : celle d'avant (les noms seuls).
pub(crate) const HEAD_MIN: f32 = HEADERS_W;
/// Largeur la plus grande.
pub(crate) const HEAD_MAX: f32 = 520.0;
/// Largeur où tout se voit (double-clic sur la barre).
pub(crate) const HEAD_ALL: f32 = 330.0;
/// Largeur de la barre qu'on tire, au bord droit de la colonne.
const GRIP_W: f32 = 6.0;
/// Place gardée au nom avant les icônes.
const NAME_MIN: f32 = 56.0;
const ICON_W: f32 = 19.0;
const MENU_W: f32 = 16.0;
const BAR_W: f32 = 100.0;
const BLEND_W: f32 = 96.0;
const AFTER_W: f32 = 104.0;
/// Largeur de rangée à partir de laquelle viennent l'opacité (dessous) et les icônes.
const OPTIONS_FROM: f32 = 120.0;

/// Largeur de colonne permise (une valeur lue dans les réglages peut être n'importe quoi).
pub(crate) fn clamp_head_w(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(HEAD_MIN, HEAD_MAX)
    } else {
        HEAD_MIN
    }
}

/// Où va chaque chose dans la rangée d'un calque, selon sa largeur.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadLayout {
    pub name: Rect,
    /// Afficher, verrouiller, pelure d'oignon : autant qu'il en tient, dans cet ordre.
    pub icons: Vec<Rect>,
    pub menu: Option<Rect>,
    pub opacity: Option<Rect>,
    pub blend: Option<Rect>,
    /// Après la dernière image (le post-comportement).
    pub after: Option<Rect>,
}

/// La rangée `r` d'un calque : en haut le nom et les icônes, en bas les réglages qui tiennent.
/// Sans réglages en bas, tout est centré dans la hauteur (comme la colonne étroite d'avant).
pub(crate) fn head_layout(r: Rect) -> HeadLayout {
    let w = r.width();
    let left = r.left() + 6.0;
    // La colonne étroite d'avant garde son nom seul (et le menu ⋮) ; les réglages viennent quand
    // on l'élargit.
    let opacity = w >= OPTIONS_FROM;
    let (y1, h1) = if opacity { (r.top() + 2.0, 18.0) } else { (r.center().y - 9.0, 18.0) };
    let menu = (w >= 6.0 + NAME_MIN + MENU_W + 2.0).then(|| Rect::from_min_size(pos2(r.right() - MENU_W - 2.0, y1), vec2(MENU_W, h1)));
    let icons_right = menu.map_or(r.right() - 2.0, |m| m.left());
    let room = icons_right - (left + NAME_MIN);
    let n = ((room / ICON_W).floor().max(0.0) as usize).min(3);
    let icons: Vec<Rect> = (0..n).map(|k| Rect::from_min_size(pos2(icons_right - (n - k) as f32 * ICON_W, y1), vec2(ICON_W, h1))).collect();
    let name_right = icons.first().map_or(icons_right, |i| i.left()) - 2.0;
    let name = Rect::from_min_max(pos2(left, y1), pos2(name_right.max(left), y1 + h1));
    let y2 = r.top() + 21.0;
    let line2 = |x: f32, width: f32| Rect::from_min_size(pos2(x, y2), vec2(width, 16.0));
    let (mut blend, mut after) = (None, None);
    let opacity = opacity.then(|| line2(left, BAR_W.min(w - 12.0)));
    if let Some(bar) = opacity {
        let x = bar.right() + 4.0;
        if x + BLEND_W + 4.0 <= r.right() {
            let b = line2(x, BLEND_W);
            if b.right() + 4.0 + AFTER_W + 4.0 <= r.right() {
                after = Some(line2(b.right() + 4.0, AFTER_W));
            }
            blend = Some(b);
        }
    }
    HeadLayout { name, icons, menu, opacity, blend, after }
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
            "Tirer pour élargir la colonne et voir les réglages des calques (opacité, fusion, boucle…). Double-clic : tout montrer, ou revenir à la colonne étroite.",
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

    /// Ce qui va dans la rangée `r` du calque `li` (plan courant), une fois le fond de la rangée
    /// posé (ses clics, glissés et menu) : le nom (ou son champ quand on le renomme) et les
    /// réglages qui tiennent (`head_layout`).
    pub(crate) fn montage_layer_head(&mut self, ui: &mut egui::Ui, r: Rect, li: usize, name_color: Color32) {
        let Some(l) = self.layers().get(li) else { return };
        let (id, name, visible, locked, onion, opacity, blend, post, folder) =
            (l.id, l.name.clone(), l.visible, l.locked, l.onion, l.opacity, l.blend, l.post, l.folder);
        let current = li == self.cur_layer;
        let at = head_layout(r);
        // Dans une zone à part : les boutons posés (`ui.put`) ne poussent pas la bande du montage,
        // qui suit la colonne.
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("tete_calque_montage", id)));
        let ui = &mut child;
        let painter = ui.painter_at(r);
        let renaming = self.layer_rename.as_ref().is_some_and(|n| n.id == id && n.in_montage);
        if renaming {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(at.name.expand2(vec2(0.0, 1.0))).id_salt(("nom_calque_montage", id)));
            let done = self.layer_rename_field(&mut child, at.name.width());
            self.finish_layer_rename(done);
        } else {
            painter.with_clip_rect(at.name).text(pos2(at.name.left(), at.name.center().y), Align2::LEFT_CENTER, &name, FontId::proportional(12.0), name_color);
        }
        let fg = ui.visuals().text_color();
        let off = fg.gamma_multiply(0.35);
        if let Some(slot) = at.icons.first() {
            self.eye_button(ui, *slot, li, visible, if visible { fg } else { off });
        }
        if let Some(slot) = at.icons.get(1) {
            self.lock_button(ui, *slot, li, locked, if locked { fg } else { off });
        }
        if let Some(slot) = at.icons.get(2).filter(|_| !folder) {
            let resp = icon_at(ui, *slot, ph::CIRCLES_THREE, if onion { fg } else { off });
            let resp = bulle(
                resp,
                "Pelure d'oignon du calque",
                "Quand la pelure d'oignon est allumée, montrer aussi les dessins voisins de ce calque, même s'il n'est pas le calque courant.",
                "",
            );
            if resp.clicked() {
                self.set_layer_flag(li, |l| l.onion = !l.onion);
            }
        }
        if let Some(slot) = at.menu {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(slot).id_salt(("options_calque_montage", id)));
            let button = egui::Button::new(RichText::new(ph::DOTS_THREE_VERTICAL).size(13.0).color(fg)).frame(false);
            let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.montage_layer_menu(ui, li)).response;
            bulle(resp, "Tout ce qui touche au calque", "Réglages, avant / après ses images, groupe, renommer, dupliquer, supprimer… (aussi : clic droit sur la rangée).", "");
        }
        if let Some(bar) = at.opacity {
            self.opacity_bar(ui, &painter, bar, r, li, egui::Id::new(("opacite_montage", id)), opacity, current, "Opacité du calque", "Cliquer ou tirer dans la barre.");
        }
        if folder {
            return;
        }
        if let Some(rect) = at.blend {
            if let Some(mode) = blend_combo(ui, rect, egui::Id::new(("fusion_montage", id)), blend) {
                self.set_layer_flag(li, |l| l.blend = mode);
            }
        }
        if let Some(rect) = at.after {
            let mut chosen = post;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("apres_montage_cadre", id)).layout(Layout::left_to_right(Align::Center)));
            child.spacing_mut().interact_size.y = 16.0;
            child.spacing_mut().button_padding = vec2(4.0, 0.0);
            let resp = egui::ComboBox::from_id_salt(("apres_montage", id))
                .selected_text(RichText::new(format!("{} {}", behaviour_icon(post), post.label())).size(11.0))
                .width(rect.width() - 4.0)
                .show_ui(&mut child, |ui| {
                    for b in Behaviour::ALL {
                        ui.selectable_value(&mut chosen, b, format!("{} {}", behaviour_icon(b), b.label()));
                    }
                })
                .response;
            bulle(
                resp,
                "Après la dernière image",
                "Ce que montre le calque après son dernier dessin, jusqu'à la fin du plan : rien, ses images en boucle, en va-et-vient, ou la dernière maintenue. Avant la première image : clic droit.",
                "",
            );
            if chosen != post {
                self.set_layer_flag(li, |l| l.post = chosen);
            }
        }
    }

    /// Tout ce qui touche au calque `li` du plan courant (clic droit dans la colonne du montage,
    /// son ⋮, ou « Calque » au clic droit d'une piste) : ses réglages, avant / après ses images,
    /// son groupe, le renommer, le dupliquer, le supprimer, puis toutes les commandes des calques.
    pub(crate) fn montage_layer_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        self.layer_full_menu(ui, li, true);
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
                Some("La propagation se fait sur un calque bitmap : crée-en un (Calques > Nouveau calque bitmap) au-dessus de la vidéo ou de l'animation, et peins dessus.")
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
        let (name, visible, onion, keep_alpha, stencil, negative, fixed, opacity, blend, folder) =
            (l.name.clone(), l.visible, l.onion, l.keep_alpha, l.stencil, l.stencil_negative, l.position_locked, l.opacity, l.blend, l.folder);
        let (ctg, video) = (l.ctg.is_some(), l.video.is_some());
        ui.label(RichText::new(format!("Calque « {name} »")).weak().small());
        // Fonctions simples.
        self.rename_item(ui, li, false, in_montage);
        if in_montage {
            self.duplicate_lock_delete_items(ui, li);
        }
        // Plusieurs calques choisis (Ctrl ou Maj + clic) : les fusionner (Anthony, 2026-10-17). Dans
        // le panneau Calques, c'est son bouton Fusionner.
        let chosen = self.chosen_layers().len();
        if in_montage && chosen > 1 && ui.button(format!("Fusionner les {chosen} calques choisis")).clicked() {
            ui.close_menu();
            self.merge_chosen_layers();
        }
        // Ce qui sert souvent (menu court, Anthony, 2026-10-18) : afficher, pelure, opacité, mode
        // de fusion (dans le panneau Calques, son en-tête les a), les effets non destructifs par
        // familles, la propagation de la peinture ; les autres réglages d'état et d'animation dans
        // deux sous-menus.
        ui.separator();
        if in_montage && ui.checkbox(&mut visible.clone(), "Afficher").clicked() {
            self.set_layer_flag(li, |l| l.visible = !visible);
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
                self.renderer.invalidate_all();
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
        self.propagation_item(ui, li);
        // Fonctions d'état, puis d'animation (Anthony, 2026-10-17 : « en premier les fonctions
        // simples, puis les fonctions d'état, ensuite les fonctions d'animation »).
        ui.separator();
        ui.menu_button("Options du calque", |ui| {
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
        if ctg {
            ui.menu_button("Calque CTG", |ui| self.ctg_menu(ui, li));
        }
        if video {
            ui.menu_button("Calque vidéo", |ui| self.video_menu(ui, li));
        }
        let (label, from) = if in_montage {
            ("Toutes les commandes des calques", crate::layerops::OpsFrom::LayerMenu)
        } else {
            ("Autres commandes des calques", crate::layerops::OpsFrom::LayersPanel)
        };
        ui.menu_button(label, |ui| self.layer_ops_menu_from(ui, from));
    }
}
