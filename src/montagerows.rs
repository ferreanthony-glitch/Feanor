//! Pistes de la bande de montage (6f) : la caméra au-dessus des panneaux, les calques et le fond
//! de scène en dessous, puis les sons ; leurs noms dans la colonne de gauche.
//!
//! Chaque plan a ses propres calques, dans son propre ordre : la rangée r montre, sous chaque
//! plan, son r-ième calque en partant du haut de sa pile (Anthony, 2026-10-17 : « l'organisation
//! et l'ordre des calques indépendants d'un shot à un autre » ; au chantier 5, une piste par nom
//! de calque les alignait d'un plan à l'autre ; avant, 8 rangées au plus). Le nom de chaque
//! calque est dans sa case ; la colonne de gauche montre ceux du plan courant. Un calque de toute
//! la scène (fond) a sa rangée comme les autres, teinté « fond ».
//!
//! Les gestes de la timeline marchent dans les pistes (chantier 5) : le bord droit d'un dessin
//! étire son exposition, la barre sous les dessins décale le calque, ses bouts le rognent ou
//! l'allongent (`handles.rs`, une étape d'annulation par geste). Depuis le 2026-10-17 aussi : un
//! dessin pris par son milieu se place avant ou après les autres (chacun garde sa durée), la
//! limite entre deux dessins allonge l'un et raccourcit l'autre, un double-clic pose un nouveau
//! dessin (une clé d'animation) à l'image visée.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::document::{Behaviour, Layer};
use crate::handles::{content_bounds, Handle};
use crate::pistes::Lane;
use crate::soundtrack::LANE_GAP;
use crate::storyboard::{BANDS_H, RULER_H};
use crate::theme;

/// Hauteur de la piste Caméra et d'une piste de calque.
pub(crate) const CAM_H: f32 = 16.0;
/// Une piste de calque : haute depuis que le montage est le banc principal (Anthony,
/// 2026-10-17 : « pour rendre la saisie des clés plus confortable » ; 20, puis 26, puis 40).
pub(crate) const LAYER_H: f32 = 40.0;
/// Barre d'un calque sous ses dessins (chantier 5, comme dans la timeline) : on la prend pour
/// décaler le calque.
pub(crate) const BAR_H: f32 = 5.0;
/// Distance entre le bas d'une piste dépliée et la barre du calque : la place du liseré (3 points) du calque de couleur lié.
pub(crate) const BAR_OFF: f32 = 3.0;
/// Hauteur du liseré d'un calque de couleur lié, sous le bloc de son calque de trait.
const LISERE_H: f32 = 3.0;
/// La couleur des clés de caméra (celle du thème).
fn cam_color() -> Color32 {
    theme::camera()
}


/// Le calque d'un plan (`layers`, du bas vers le haut) à la rangée `row` du montage (0 : en
/// haut) : chaque plan montre ses calques dans son propre ordre, du haut de sa pile vers le bas
/// (Anthony, 2026-10-17 : « l'organisation et l'ordre des calques indépendants d'un shot à un
/// autre »). Un calque de couleur lié à un calque de trait n'a pas de rangée : un liseré sous le
/// bloc de son trait le montre (2026-10-28).
pub(crate) fn layer_in_row(layers: &[Layer], row: usize) -> Option<usize> {
    layers.iter().enumerate().rev().filter(|(_, l)| !l.keys_follow_line()).nth(row).map(|(k, _)| k)
}

/// La rangée du calque `li` (celle de son trait, pour un calque de couleur lié) : l'inverse de `layer_in_row`.
pub(crate) fn row_of_layer(layers: &[Layer], li: usize) -> Option<usize> {
    let l = layers.get(li)?;
    let li = if l.keys_follow_line() { crate::calquecouleur::trait_de(layers, li)? } else { li };
    Some(layers[li + 1..].iter().filter(|l| !l.keys_follow_line()).count())
}

/// Combien de rangées les calques de `layers` font (les calques de couleur liés n'en font pas).
pub(crate) fn row_count(layers: &[Layer]) -> usize {
    layers.iter().filter(|l| !l.keys_follow_line()).count()
}

/// Rangées de la bande, en distance depuis son haut.
pub(crate) struct Rows {
    pub block_h: f32,
    /// Les pistes de calques visibles, de haut en bas : dépliées ou repliées, sans celles d'un
    /// dossier fermé (`pistes.rs`).
    pub lanes: Vec<Lane>,
}

impl Rows {
    pub fn camera_top(&self) -> f32 {
        RULER_H + BANDS_H
    }
    pub fn blocks_top(&self) -> f32 {
        self.camera_top() + CAM_H
    }
    pub fn layers_top(&self) -> f32 {
        self.blocks_top() + self.block_h + 2.0
    }
    /// Hauteur de toutes les pistes de calques.
    pub fn layers_h(&self) -> f32 {
        self.lanes.last().map_or(0.0, |l| l.top + l.h)
    }
    /// Bas des pistes de calques.
    pub fn background_top(&self) -> f32 {
        self.layers_top() + self.layers_h()
    }
    /// La piste sous la hauteur `y` (depuis le haut des pistes de calques).
    pub fn lane_at(&self, y: f32) -> Option<&Lane> {
        self.lanes.iter().find(|l| y >= l.top && y < l.top + l.h)
    }
    /// La rangée sous la hauteur `y` (depuis le haut des pistes de calques), la plus proche si `y`
    /// est avant la première piste ou après la dernière (choix au rectangle).
    pub fn row_near(&self, y: f32) -> usize {
        // (Sans la ligne « N autres calques », qui n'est la rangée d'aucun calque : un rectangle de choix qui s'y termine s'arrête à la
        // dernière piste de calque.)
        let real = || self.lanes.iter().filter(|l| !l.is_more());
        real().find(|l| y >= l.top && y < l.top + l.h).or_else(|| if y < 0.0 { real().next() } else { real().last() }).map_or(0, |l| l.row)
    }
    pub fn lanes_top(&self) -> f32 {
        self.background_top() + LANE_GAP
    }
    /// Hauteur totale, pistes son comprises.
    pub fn height(&self, sound_tracks: usize) -> f32 {
        self.lanes_top() + sound_tracks as f32 * crate::montagehauteur::piste_son() + 2.0
    }
}


impl App {
    /// Rangées pour une bande haute de `available` : les panneaux prennent la place qui reste.
    pub(crate) fn montage_rows(&self, available: f32) -> Rows {
        // (La hauteur des pistes son : celle du réglage, montagehauteur.rs.)
        self.poser_hauteur_pistes();
        let count = self.doc.panels.iter().map(|p| row_count(&p.layers)).max().unwrap_or(0);
        let lanes = crate::pistes::lanes_in(&self.doc.panels, self.cur_panel, self.cur_layer, count, self.pistes_auto_open(), &self.pistes_replis());
        let mut rows = Rows { block_h: 0.0, lanes };
        let fixed = rows.height(self.doc.audio_tracks.len()) + 12.0;
        rows.block_h = (available - fixed).clamp(56.0, 170.0);
        rows
    }

    /// Noms des pistes au-dessus des pistes son, dans la colonne `rect` ; un clic sur un calque
    /// le rend courant.
    pub(crate) fn row_headers(&mut self, ui: &mut egui::Ui, rect: Rect, rows: &Rows) {
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals().clone();
        let lane = |y: f32, h: f32| Rect::from_min_size(pos2(rect.left() + 2.0, rect.top() + y + 1.0), vec2(rect.width() - 8.0, h - 2.0));
        let label = |lane: Rect, text: &str, color: Color32| {
            painter.rect_filled(lane, 3.0, theme::etiquette());
            painter.with_clip_rect(lane.shrink(1.0)).text(
                pos2(lane.left() + 6.0, lane.center().y),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(12.0),
                color,
            );
        };
        let cam = lane(rows.camera_top(), CAM_H);
        label(cam, "Caméra", visuals.text_color());
        // Clic droit sur « Caméra » : le menu de la caméra (passe interface).
        ui.interact(cam, ui.id().with("nom_piste_camera"), Sense::click()).context_menu(|ui| self.grid_menu_ui(ui, crate::handles::GridMenu::Camera));
        let header = lane(rows.blocks_top(), rows.block_h);
        painter.rect_filled(header, 3.0, theme::etiquette());
        self.project_header(ui, header);
        // Les calques du plan courant, dans son ordre : clic, il devient courant ; glisser, il monte
        // ou descend (dans ce plan seulement) ; clic droit, son menu (passe interface).
        let names: Vec<(Lane, String, Option<usize>)> = {
            let panel = &self.doc.panels[self.cur_panel];
            rows.lanes
                .iter()
                .map(|&lane| {
                    let k = layer_in_row(&panel.layers, lane.row);
                    (lane, k.map_or(String::new(), |k| panel.layers[k].name.clone()), k)
                })
                .collect()
        };
        let mut pick = None;
        let cur_row = row_of_layer(self.layers(), self.cur_layer);
        let mut grid: Vec<crate::layerheads::Row> = Vec::new();
        // Le menu du clic droit d'une rangée se montre au milieu des rangées : « Supprimer » un dossier emmène ses calques, et les
        // rangées d'après lisaient des rangs qui n'existaient plus (passe de débogage, 2026-10-22). On s'arrête dès que les calques
        // ont changé ; l'image suivante refait les rangées.
        let avant = self.empreinte_calques();
        let mut tient = true;
        for (track, name, item) in names {
            if !self.calques_tiennent(&avant, item.unwrap_or(usize::MAX)) {
                tient = false;
                ui.ctx().request_repaint();
                break;
            }
            let row = track.row;
            let r = lane(rows.layers_top() + track.top, track.h);
            if track.is_more() {
                self.more_line(ui, &painter, r);
                continue;
            }
            let Some(k) = item else {
                label(r, &name, visuals.weak_text_color());
                ui.interact(r, ui.id().with(("piste_calque_absente", row)), Sense::hover())
                    .on_hover_text("Le plan courant a moins de calques : cette rangée est à d'autres plans.");
                continue;
            };
            grid.push(crate::layerheads::Row { li: k, top: track.top, h: track.h });
            // (Le calque de couleur courant est « actif » par la rangée de son trait.)
            let current = Some(row) == cur_row;
            // Un calque de toute la scène : son nom teinté « fond ».
            let wide = self.doc.panels[self.cur_panel].layers[k].scene_wide.is_some();
            let color = match (wide, current) {
                (true, _) => theme::decor(),
                (false, true) => visuals.strong_text_color(),
                (false, false) => visuals.text_color(),
            };
            // Le fond de la rangée (choisir, glisser, menu), puis le nom et les réglages qui tiennent
            // dans la largeur de la colonne, au-dessus (`montagehead.rs`).
            painter.rect_filled(r, 3.0, theme::etiquette());
            // (La sorte du calque, vectoriel, bitmap…, est la pastille de couleur de l'en-tête : `montage_head`.)
            if current {
                painter.rect_stroke(r, 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Inside);
            }
            // Choisi avec d'autres (Ctrl ou Maj + clic) : surligné plus légèrement.
            if !current && self.is_chosen_layer(k) {
                painter.rect_filled(r, 3.0, crate::theme::surface_active(crate::theme::accent(), 0.3));
                painter.rect_stroke(r, 3.0, Stroke::new(1.0_f32, crate::theme::accent()), StrokeKind::Inside);
            }
            let resp = ui.interact(r, ui.id().with(("piste_calque", row)), Sense::click_and_drag());
            if resp.drag_started() {
                self.montage_layer_drag = Some(k);
            }
            let resp = resp.on_hover_text(
                "Calque du plan courant : clic, il devient actif (la piste se déplie) ; double-clic sur le nom, le renommer ; double-clic à côté, replier ou déplier la piste ; glisser, il monte ou descend ; clic droit, tout ce qui touche au calque (opacité, verrou, fusion…).",
            );
            if resp.clicked() || resp.secondary_clicked() {
                pick = Some((k, resp.secondary_clicked()));
            }
            if let (true, Some(p)) = (resp.double_clicked(), resp.interact_pointer_pos()) {
                let name_rect = crate::pistes::head_layout(r, track.folded).name;
                if self.double_click_is_on_name(ui, p, name_rect, &name, crate::pistes::name_size(track.folded)) {
                    let id = self.doc.panels[self.cur_panel].layers[k].id;
                    self.layer_rename = Some(crate::layersui::LayerRename { in_montage: true, ..crate::layersui::LayerRename::new(id, &name, false) });
                } else {
                    self.toggle_track_fold(row, false);
                }
            }
            resp.context_menu(|ui| self.montage_layer_menu(ui, k));
            self.montage_head(ui, r, k, color, track.folded);
        }
        // (Les calques ont changé pendant les rangées : le glissé et le clic de cette image visaient les rangs d'avant.)
        tient &= self.calques_tiennent(&avant, usize::MAX);
        if !tient {
            ui.ctx().request_repaint();
            return;
        }
        if let Some(from) = self.montage_layer_drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let base = rect.top() + rows.layers_top();
            let pointer = ui.input(|i| i.pointer.interact_pos());
            if let Some(p) = pointer {
                let slot = grid.iter().filter(|g| g.top + g.h * 0.5 < p.y - base).count();
                let y = base + grid.get(slot).map_or(grid.last().map_or(0.0, |g| g.top + g.h), |g| g.top);
                painter.line_segment([pos2(rect.left() + 2.0, y), pos2(rect.right() - 6.0, y)], Stroke::new(2.0_f32, theme::contour()));
            }
            if ui.input(|i| !i.pointer.any_down()) {
                self.montage_layer_drag = None;
                if let Some(p) = pointer {
                    let to = crate::layerheads::drop_index(&grid, from, p.y - base);
                    if to != from && !self.content_refuses() {
                        self.move_layer_to(from, to);
                    }
                }
            }
        }
        if let Some((k, secondary)) = pick {
            // Ctrl : ajouté au choix ou retiré ; Maj : ceux d'entre ; clic droit sur un calque déjà
            // choisi : le choix reste (pour fusionner les calques choisis).
            let m = ui.input(|i| i.modifiers);
            if !(secondary && self.is_chosen_layer(k)) {
                self.click_layer_head(k, m.command && !secondary, m.shift && !secondary);
            }
        }
    }

    /// La ligne « ▾ N autres calques » (mode « cachés ») : un clic montre les autres calques en bandes fines, un autre les cache.
    fn more_line(&mut self, ui: &mut egui::Ui, painter: &Painter, r: Rect) {
        use egui_phosphor::regular as ph;
        let count = self.doc.panels.iter().map(|p| row_count(&p.layers)).max().unwrap_or(0);
        let n = crate::pistes::others_count(&self.doc.panels, self.cur_panel, self.cur_layer, count, self.pistes_auto_open(), &self.pistes_replis());
        let shown = self.pistes.autres_montres;
        painter.rect_filled(r, 3.0, theme::etiquette());
        let text = match (shown, n) {
            (false, 1) => format!("{} 1 autre calque", ph::CARET_DOWN),
            (false, n) => format!("{} {n} autres calques", ph::CARET_DOWN),
            (true, _) => format!("{} Cacher les autres", ph::CARET_UP),
        };
        let resp = ui.interact(r, ui.id().with("autres_calques"), Sense::click());
        let ink = if resp.hovered() { theme::texte_fort() } else { ui.visuals().weak_text_color() };
        painter.with_clip_rect(r).text(pos2(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, text, FontId::proportional(10.5), ink);
        let resp = resp.on_hover_text("Les calques non actifs sont cachés (Préférences > Montage). Un clic les montre en bandes fines, un autre les cache de nouveau.");
        if resp.clicked() {
            self.pistes.autres_montres = !shown;
        }
    }

    /// À la place du mot « Storyboard », en face des plans (choix d'Anthony, 2026-10-16, comme
    /// la colonne de gauche du panneau Projet de TVPaint) : les commandes des plans et le nom du
    /// projet, qui ouvre ses réglages.
    fn project_header(&mut self, ui: &mut egui::Ui, r: Rect) {
        use egui_phosphor::regular as ph;
        let inner = r.shrink2(vec2(4.0, 3.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).id_salt("entete_projet").layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(r.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing = vec2(3.0, 3.0);
        let ui = &mut child;
        let mut run: Option<&str> = None;
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::PLUS_SQUARE, "Nouveau plan", "Après le plan courant, dans la même séquence.", "panneau.nouveau").clicked() {
                run = Some("panneau.nouveau");
            }
            if self.bouton(ui, ph::ARROW_LINE_LEFT, "Nouveau plan avant", "Avant le plan courant, dans la même séquence.", "plan.nouveau_avant").clicked() {
                run = Some("plan.nouveau_avant");
            }
            if self.bouton(ui, ph::COPY_SIMPLE, "Dupliquer le plan", "Une copie du plan courant, juste après lui.", "panneau.dupliquer").clicked() {
                run = Some("panneau.dupliquer");
            }
        });
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::TRASH, "Supprimer les plans", "Les plans choisis, sinon le plan courant.", "panneau.supprimer").clicked() {
                run = Some("panneau.supprimer");
            }
            if self.bouton(ui, ph::STACK_PLUS, "Nouvelle séquence", "Après la séquence courante, avec un plan.", "sequence.nouvelle").clicked() {
                run = Some("sequence.nouvelle");
            }
            let locked = self.doc.storyboard_locked;
            let icon = if locked { ph::LOCK } else { ph::LOCK_OPEN };
            let what = "Verrouillé : l'ordre, le nombre et la durée des plans, leurs dessins et leurs calques ne changent plus.";
            if self.bouton_etat(ui, locked, icon, "Verrouiller le storyboard", what, "storyboard.verrouiller").clicked() {
                run = Some("storyboard.verrouiller");
            }
        });
        let title = self.doc.properties.title.trim().to_string();
        let name = if title.is_empty() { self.base_name_or("Sans titre") } else { title };
        let plans = self.doc.panels.len();
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::GEAR_SIX, "Réglages du projet", "Propriétés, résolution, cadence.", "projet.proprietes").clicked() {
                run = Some("projet.proprietes");
            }
            let resp = ui.add(egui::Label::new(egui::RichText::new(&name).small().strong()).truncate().sense(Sense::click()));
            if resp.on_hover_text(format!("{name} : {plans} plan(s). Clic : réglages du projet.")).clicked() {
                run = Some("projet.proprietes");
            }
        });
        if let Some(action) = run {
            self.run_action(action);
        }
    }

    /// Pistes Caméra, calques et fond, dans la bande `r` ; `blocks` : les panneaux à l'écran ;
    /// `lit` : la poignée de calque survolée ou tenue (vrai : tenue), et son plan ; `drops` : les
    /// clés qu'on glisse (fantômes et repères, `montagekeys::drop_view`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_track_rows(
        &self,
        painter: &Painter,
        r: Rect,
        blocks: &[Rect],
        ppf: f32,
        rows: &Rows,
        lit: Option<(usize, Handle, bool)>,
        drops: &[crate::montagekeys::DropCell],
    ) {
        let top = r.top();
        let lane = |y: f32, h: f32| Rect::from_min_max(pos2(r.left(), top + y + 1.0), pos2(r.right(), top + y + h - 1.0));

        // Caméra : une clé = un losange ; un mouvement = une barre de la première à la dernière.
        let cam = lane(rows.camera_top(), CAM_H);
        painter.rect_filled(cam, 0.0, theme::rangee_sombre());
        let cam_vues = self.cle_vue(crate::legendecles::Sorte::Camera);
        for (i, b) in blocks.iter().enumerate() {
            // (Cachées par le filtre de la légende : la piste reste, sans losanges.)
            let keys: &[crate::camera::CameraKey] = if cam_vues { &self.doc.panels[i].camera } else { &[] };
            let x0 = b.left() - 1.0;
            let y = cam.center().y;
            if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
                if keys.len() >= 2 {
                    let bar = Rect::from_min_max(
                        pos2(x0 + first.frame as f32 * ppf + ppf * 0.5, y - 1.5),
                        pos2(x0 + last.frame as f32 * ppf + ppf * 0.5, y + 1.5),
                    );
                    painter.rect_filled(bar, 1.0, cam_color().gamma_multiply(0.5));
                }
            }
            for key in keys.iter() {
                let c = pos2(x0 + key.frame as f32 * ppf + ppf * 0.5, y);
                if c.x < painter.clip_rect().left() - 8.0 || c.x > painter.clip_rect().right() + 8.0 {
                    continue;
                }
                // Une clé choisie (ou que l'on fait glisser) : plus grande, cerclée du contour du thème.
                self.paint_cam_diamond(painter, c, self.cam_key_chosen(i, key.frame), cam_color(), 4.0, Stroke::NONE);
            }
        }

        // Les calques de chaque plan, dans son ordre.
        let current_plan = &self.doc.panels[self.cur_panel.min(self.doc.panels.len() - 1)];
        for track in &rows.lanes {
            let row = track.row;
            let lr = lane(rows.layers_top() + track.top, track.h);
            painter.rect_filled(lr, 0.0, theme::rangee_sombre());
            if track.is_more() {
                continue;
            }
            // Le nom de la rangée : celui du calque du plan courant, que la colonne de gauche écrit. Un bloc n'écrit son nom que s'il en diffère.
            let row_name = layer_in_row(&current_plan.layers, row).map(|k| current_plan.layers[k].name.as_str());
            for (i, b) in blocks.iter().enumerate() {
                let cell = Rect::from_min_max(pos2(b.left(), lr.top()), pos2(b.right(), lr.bottom()));
                if !painter.clip_rect().intersects(cell) {
                    continue;
                }
                let p = &self.doc.panels[i];
                let Some(k) = layer_in_row(&p.layers, row) else { continue };
                let current = i == self.cur_panel && k == self.cur_layer;
                let hot = lit.filter(|(pi, h, _)| *pi == i && h.layer() == Some(k)).map(|(_, h, held)| (h, held));
                let chosen: Vec<usize> = self.key_sel.iter().filter(|c| c.panel == p.id && c.layer == p.layers[k].id).map(|c| c.start).collect();
                let show_name = row_name != Some(p.layers[k].name.as_str());
                paint_layer_cell(painter, cell, &p.layers[k], &p.layers, ppf, current, hot, &chosen, track.folded, show_name);
                let x0 = cell.left() - 1.0;
                let visible = (painter.clip_rect().left(), painter.clip_rect().right());
                if track.folded {
                    // Bande fine : les clés de position et d'effets en petits traits.
                    self.paint_ticks(painter, p, k, |f| x0 + f as f32 * ppf + ppf * 0.5, cell, visible);
                } else {
                    // Clés d'effets (13i) : un losange violet en haut de la piste, par image clée.
                    self.paint_fx_markers(painter, p.id, &p.layers[k], |f| x0 + f as f32 * ppf + ppf * 0.5, cell.top() + 8.0, p.frame_count(), visible);
                    // Clés de position (`poskeysui.rs`) : les losanges verts au milieu de la piste, comme dans la
                    // timeline (Anthony : « les clés de position d'un calque n'apparaissent pas sur le calque »).
                    if !p.layers[k].pos_keys.is_empty() {
                        self.paint_pos_markers(painter, &p.layers[k], |f| x0 + f as f32 * ppf + ppf * 0.5, cell.center().y, p.frame_count(), visible);
                    }
                    // Cles d'opacite (`opacitecleselui.rs`) : les losanges gris clair en bas de la piste.
                    if !p.layers[k].opacity_keys.is_empty() {
                        self.paint_op_markers(painter, &p.layers[k], |f| x0 + f as f32 * ppf + ppf * 0.5, cell.bottom() - 5.5, p.frame_count(), visible);
                    }
                }
                // Clés qu'on glisse : leur fantôme suit la souris, un repère montre où elles se
                // poseront (un repère entre deux panneaux).
                if let Some(d) = drops.iter().find(|d| d.panel == p.id && d.layer == p.layers[k].id) {
                    paint_drop(painter, cell, ppf, d);
                }
            }
        }
    }

    /// Les clés de position (vertes, moitié basse) et d'effets (violettes, moitié haute) du calque `k` du plan `p`, en petits traits sur
    /// toute la hauteur d'une bande fine `cell`. Les mêmes conditions que dans une piste dépliée (`paint_fx_markers`,
    /// `paint_pos_markers`) ; on ne les prend pas à la souris (un clic choisit le calque).
    pub(crate) fn paint_ticks(&self, painter: &Painter, p: &crate::document::Panel, k: usize, x_of: impl Fn(usize) -> f32, cell: Rect, visible: (f32, f32)) {
        let layer = &p.layers[k];
        let max = p.frame_count();
        let on_screen = |x: f32| x >= visible.0 - 4.0 && x <= visible.1 + 4.0;
        let (top, bottom) = (cell.top() + 1.0, cell.bottom() - 1.0);
        let middle = (top + bottom) * 0.5;
        let tick = |x: f32, y0: f32, y1: f32, color: Color32, on: bool| {
            let w = if on { 2.5 } else { 1.5 };
            painter.rect_filled(Rect::from_min_max(pos2(x - w * 0.5, y0), pos2(x + w * 0.5, y1)), 0.0, if on { theme::contour() } else { color });
        };
        if let Some(sel) = self.fx_shown(p.id, layer) {
            let chosen = self.fx_chosen_frames(p.id, layer.id);
            for f in crate::fxkeys::marker_frames(&layer.fx[sel.effect]) {
                let x = x_of(f);
                if f < max && on_screen(x) {
                    tick(x, top, middle, crate::fxkeys::color(), chosen.contains(&f));
                }
            }
        }
        let current_layer = self.layers().get(self.cur_layer).is_some_and(|l| l.id == layer.id);
        // (Les positions cachées par le filtre de la légende, `legendecles.rs`, ne se dessinent pas.)
        let positions: &[crate::poskeys::PosKey] = if self.cle_vue(crate::legendecles::Sorte::Position) { &layer.pos_keys } else { &[] };
        for key in positions {
            let x = x_of(key.frame);
            if key.frame < max && on_screen(x) {
                let on = self.pos_ui.sel.contains(&(layer.id, key.frame)) || (key.frame == self.cur_frame && current_layer);
                tick(x, middle, bottom, theme::POIGNEE_CALQUE, on);
            }
        }
        // Les cles d'opacite : un trait court tout en bas (cachees par le filtre de la legende comme les autres).
        if self.cle_vue(crate::legendecles::Sorte::Opacite) {
            for key in &layer.opacity_keys {
                let x = x_of(key.frame);
                if key.frame < max && on_screen(x) {
                    let on = self.op_ui.sel.contains(&(layer.id, key.frame)) || (key.frame == self.cur_frame && current_layer);
                    tick(x, bottom - 3.0, bottom, theme::CLE_OPACITE, on);
                }
            }
        }
    }

    /// Clé de caméra sous le point `p` dans la piste Caméra : (panneau, clé). `top` : haut de la
    /// bande ; `blocks` : les panneaux à l'écran.
    pub(crate) fn montage_camera_key_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize)> {
        let y = p.y - top;
        // (Cachées par le filtre de la légende, `legendecles.rs` : ni dessinées ni saisies.)
        if !self.cle_vue(crate::legendecles::Sorte::Camera) || y < rows.camera_top() || y > rows.camera_top() + CAM_H {
            return None;
        }
        let tol = (ppf * 0.5).max(5.0);
        blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| p.x >= b.left() - tol - 1.0 && p.x <= b.right() + tol + 1.0)
            .find_map(|(i, b)| {
                let x0 = b.left() - 1.0;
                crate::camtools::camera_key_at(&self.doc.panels[i].camera, p.x, |f| x0 + f as f32 * ppf + ppf * 0.5, tol).map(|k| (i, k))
            })
    }

    /// Losange de clé d'effet sous le point `p`, en haut d'une piste de calque : (plan, calque,
    /// image du plan). `top` : haut de la bande ; `blocks` : les panneaux à l'écran. Il passe avant
    /// les poignées du dessin dessous (`fxkeys.rs`).
    pub(crate) fn montage_fx_key_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize, usize)> {
        self.fx_choice.filter(|_| self.fx_keys_shown)?;
        let y = p.y - top;
        if y < rows.layers_top() {
            return None;
        }
        let lane = rows.lane_at(y - rows.layers_top())?;
        if (y - rows.layers_top()) - lane.top > lane.fx_hit_max() {
            return None;
        }
        let row = lane.row;
        let tol = (ppf * 0.5).max(5.0);
        blocks.iter().enumerate().filter(|(_, b)| p.x >= b.left() - 1.0 - tol && p.x <= b.right() + 1.0 + tol).find_map(|(i, b)| {
            let panel = &self.doc.panels[i];
            let k = layer_in_row(&panel.layers, row)?;
            let l = &panel.layers[k];
            let sel = self.fx_shown(panel.id, l)?;
            let x0 = b.left() - 1.0;
            let f = crate::fxkeys::marker_near(&crate::fxkeys::marker_frames(&l.fx[sel.effect]), p.x, |f| x0 + f as f32 * ppf + ppf * 0.5, tol)?;
            (f < panel.frame_count()).then_some((i, k, f))
        })
    }

    /// Losange vert de clé de position sous le point `p`, au milieu d'une piste de calque : (plan, calque,
    /// image du plan). `top` : haut de la bande ; `blocks` : les panneaux à l'écran. Il passe avant les
    /// poignées et le dessin dessous (`poskeysui.rs`) ; la bande des losanges d'effets, en haut de la piste
    /// (`fxkeys::STRIP`), reste aux losanges violets.
    pub(crate) fn montage_pos_key_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize, usize)> {
        let y = p.y - top;
        if y < rows.layers_top() || !self.cle_vue(crate::legendecles::Sorte::Position) {
            return None;
        }
        let lane = rows.lane_at(y - rows.layers_top())?;
        let in_row = (y - rows.layers_top()) - lane.top;
        if in_row <= lane.fx_hit_max() || (in_row - lane.pos_y()).abs() > lane.pos_hit() {
            return None;
        }
        let row = lane.row;
        let tol = (ppf * 0.5).max(5.0);
        blocks.iter().enumerate().filter(|(_, b)| p.x >= b.left() - 1.0 - tol && p.x <= b.right() + 1.0 + tol).find_map(|(i, b)| {
            let panel = &self.doc.panels[i];
            let k = layer_in_row(&panel.layers, row)?;
            let keys = &panel.layers[k].pos_keys;
            let x0 = b.left() - 1.0;
            let shown = keys.iter().map(|key| key.frame).filter(|&f| f < panel.frame_count());
            crate::poskeysui::marker_near(shown, p.x, |f| x0 + f as f32 * ppf + ppf * 0.5, tol).map(|f| (i, k, f))
        })
    }

    /// Losange gris clair de clé d'opacité sous le point `p`, en bas d'une piste de calque dépliée : (plan, calque, image du plan).
    /// Il passe avant les poignées et le dessin dessous (`opacitecleselui.rs`).
    pub(crate) fn montage_op_key_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize, usize)> {
        let y = p.y - top;
        if y < rows.layers_top() || !self.cle_vue(crate::legendecles::Sorte::Opacite) {
            return None;
        }
        let lane = rows.lane_at(y - rows.layers_top())?;
        if lane.folded || lane.is_more() {
            return None;
        }
        let in_row = (y - rows.layers_top()) - lane.top;
        if (in_row - (lane.h - 5.5)).abs() > crate::opacitecleselui::HIT_H {
            return None;
        }
        let row = lane.row;
        let tol = (ppf * 0.5).max(5.0);
        blocks.iter().enumerate().filter(|(_, b)| p.x >= b.left() - 1.0 - tol && p.x <= b.right() + 1.0 + tol).find_map(|(i, b)| {
            let panel = &self.doc.panels[i];
            let k = layer_in_row(&panel.layers, row)?;
            let keys = &panel.layers[k].opacity_keys;
            let x0 = b.left() - 1.0;
            let shown = keys.iter().map(|key| key.frame).filter(|&f| f < panel.frame_count());
            crate::poskeysui::marker_near(shown, p.x, |f| x0 + f as f32 * ppf + ppf * 0.5, tol).map(|f| (i, k, f))
        })
    }

    /// Calque visé dans les pistes de calques : (panneau, calque). `top` : haut de la bande.
    pub(crate) fn layer_row_hit(&self, p: Pos2, top: f32, blocks: &[Rect], rows: &Rows) -> Option<(usize, usize)> {
        let row = self.row_at(p, top, rows)?;
        let i = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0)?;
        Some((i, layer_in_row(&self.doc.panels[i].layers, row)?))
    }

    /// La rangée de calques sous le point `p` (`top` : haut de la bande).
    pub(crate) fn row_at(&self, p: Pos2, top: f32, rows: &Rows) -> Option<usize> {
        let y = p.y - top;
        if y < rows.layers_top() {
            return None;
        }
        rows.lane_at(y - rows.layers_top()).map(|l| l.row)
    }

    /// Poignée de calque sous le point `p`, dans les pistes de calques (chantier 5, les gestes de
    /// la timeline) : le début ou la fin du calque, sa barre (en bas de la piste), le bord droit
    /// d'un dessin. Tout contre la limite de deux plans, celui sous le point d'abord, puis son
    /// voisin. Renvoie (plan, calque, poignée). `top` : haut de la bande.
    pub(crate) fn montage_layer_handle_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize, Handle)> {
        let y = p.y - top;
        if y < rows.layers_top() {
            return None;
        }
        let lane = rows.lane_at(y - rows.layers_top())?;
        let row = lane.row;
        let lane_top = top + rows.layers_top() + lane.top;
        let tol = (ppf * 0.3).clamp(3.0, 6.0);
        let strict = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0);
        let near = blocks.iter().enumerate().filter(|(i, b)| Some(*i) != strict && p.x >= b.left() - 1.0 - tol && p.x <= b.right() + 1.0 + tol).map(|(i, _)| i);
        strict.into_iter().chain(near).find_map(|i| {
            let layers = &self.doc.panels[i].layers;
            let k = layer_in_row(layers, row)?;
            cell_handle_in(&layers[k], k, p, blocks[i].left() - 1.0, lane_top, ppf, tol, lane.bar_from()).map(|h| (i, k, h))
        })
    }
}

/// Les fantômes des clés qu'on glisse (cadres translucides, coupés au plan) et les repères où
/// elles se poseront (un trait épais sur la hauteur de la piste, avec deux pointes).
fn paint_drop(painter: &Painter, cell: Rect, ppf: f32, d: &crate::montagekeys::DropCell) {
    let x0 = cell.left() - 1.0;
    let accent = theme::contour();
    let clip = painter.with_clip_rect(cell.expand(1.0).intersect(painter.clip_rect()));
    for &(s, e) in &d.ghosts {
        let ghost = Rect::from_min_max(pos2(x0 + s as f32 * ppf, cell.top() + 2.0), pos2(x0 + e as f32 * ppf - 1.0, cell.bottom() - 2.0));
        clip.rect(ghost, 2.0, accent.gamma_multiply(0.25), Stroke::new(1.5_f32, accent), StrokeKind::Inside);
    }
    for &m in &d.marks {
        let x = x0 + m as f32 * ppf;
        clip.line_segment([pos2(x, cell.top()), pos2(x, cell.bottom())], Stroke::new(3.0_f32, theme::contour()));
        let t = 5.0;
        clip.add(egui::Shape::convex_polygon(vec![pos2(x - t, cell.top()), pos2(x + t, cell.top()), pos2(x, cell.top() + t)], theme::contour(), Stroke::NONE));
        clip.add(egui::Shape::convex_polygon(vec![pos2(x - t, cell.bottom()), pos2(x, cell.bottom() - t), pos2(x + t, cell.bottom())], theme::contour(), Stroke::NONE));
    }
}

/// Poignée du calque `layer` (rang `k`) sous le point `p` ; `x0` : l'abscisse de la première
/// image de son plan, `lane_top` : le haut de sa piste.
#[cfg(test)]
pub(crate) fn cell_handle(layer: &Layer, k: usize, p: Pos2, x0: f32, lane_top: f32, ppf: f32, tol: f32) -> Option<Handle> {
    cell_handle_in(layer, k, p, x0, lane_top, ppf, tol, crate::pistes::full_lane().bar_from())
}

/// Comme `cell_handle`, dans une piste dont la barre du calque se prend à `bar_from` points sous
/// son haut (`Lane::bar_from` : piste dépliée ou repliée).
#[allow(clippy::too_many_arguments)]
pub(crate) fn cell_handle_in(layer: &Layer, k: usize, p: Pos2, x0: f32, lane_top: f32, ppf: f32, tol: f32, bar_from: f32) -> Option<Handle> {
    if layer.folder || layer.video.is_some() || layer.keys_follow_line() {
        return None;
    }
    let x = |f: usize| x0 + f as f32 * ppf;
    let near = |at: f32| (p.x - at).abs() <= tol;
    let (a, b) = content_bounds(layer)?;
    if near(x(a)) {
        return Some(Handle::LayerStart { layer: k });
    }
    if near(x(b)) {
        return Some(Handle::LayerEnd { layer: k });
    }
    let in_bar = p.y >= lane_top + bar_from;
    if in_bar && p.x > x(a) && p.x < x(b) {
        let grab = ((p.x - x0) / ppf).floor().max(0.0) as usize;
        return Some(Handle::LayerMove { layer: k, grab });
    }
    // La limite entre deux expositions : l'une s'allonge, l'autre raccourcit.
    let f = ((p.x - x0) / ppf).round().max(0.0) as usize;
    if f >= 1 && near(x(f)) && f < layer.frames.len() && layer.frames[f - 1] != layer.frames[f] {
        return Some(Handle::Boundary { layer: k, at: f });
    }
    // Le milieu d'un dessin : il se place avant ou après les autres.
    let grab = ((p.x - x0) / ppf).floor().max(0.0) as usize;
    if layer.frames.get(grab).is_some_and(Option::is_some) {
        let (s, e) = layer.run_bounds(grab);
        return Some(Handle::Key { layer: k, start: s, end: e, grab });
    }
    None
}

/// Un calque sous un plan : un bloc par dessin exposé (tenu sur plusieurs images), sa barre
/// dessous (chantier 5), son nom s'il diffère de celui de sa rangée (`show_name`), le liseré de ses calques de couleur liés
/// (cherchés dans `stack`, la pile du plan). `hot` : sa poignée survolée ou tenue (vrai : tenue) ; `chosen` : début des clés choisies.
/// Une bande fine (`folded`, `pistes.rs`) : les dessins en barres de la couleur du calque, un peu atténuée, sans barre ni nom ni liseré.
#[allow(clippy::too_many_arguments)]
fn paint_layer_cell(painter: &Painter, cell: Rect, layer: &Layer, stack: &[Layer], ppf: f32, current: bool, hot: Option<(Handle, bool)>, chosen: &[usize], folded: bool, show_name: bool) {
    let (hot, held) = (hot.map(|h| h.0), hot.is_some_and(|h| h.1));
    let (pad, bar_h, bar_off, name_size) = if folded { (1.0, 0.0, 0.0, 9.0) } else { (2.0, BAR_H, BAR_OFF, 12.0) };
    let seg_off = if folded { 1.0 } else { bar_off + bar_h + 1.0 };
    painter.rect_filled(cell, 2.0, theme::etiquette());
    // Un calque de toute la scène : ses dessins teintés « fond ».
    let fill = match (layer.scene_wide.is_some(), folded) {
        (true, _) => theme::calque_decor(),
        (false, true) => theme::couleur_du_calque(layer).gamma_multiply(0.7),
        (false, false) => theme::fond_du_calque(layer),
    };
    let fill = if layer.visible { fill } else { fill.gamma_multiply(0.35) };
    let x0 = cell.left() - 1.0;
    let mut f = 0;
    while f < layer.frames.len() {
        let (s, e) = layer.run_bounds(f);
        if layer.frames[f].is_some() {
            let seg = Rect::from_min_max(
                pos2((x0 + s as f32 * ppf).max(cell.left()), cell.top() + pad),
                pos2((x0 + e as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - seg_off),
            );
            if seg.width() > 0.0 {
                painter.rect_filled(seg, 1.0, fill);
                // Une clé choisie : cerclée de la couleur du choix ; survolée (on la prend par son
                // milieu pour la déplacer) : de blanc.
                if chosen.contains(&s) {
                    painter.rect_stroke(seg, 1.0, Stroke::new(2.0_f32, theme::contour()), StrokeKind::Inside);
                }
                if matches!(hot, Some(Handle::Key { start, .. }) if start == s) {
                    painter.rect_stroke(seg, 1.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Inside);
                }
            }
        }
        f = e;
    }
    // Avant la première image et après la dernière : ce que le calque y montre (boucle,
    // va-et-vient, maintenir ; 13c), en pâle jusqu'au bord du plan, avec son icône.
    if let Some((a, b)) = content_bounds(layer).filter(|_| !layer.folder && layer.video.is_none()) {
        let (top, bottom) = (cell.top() + pad, cell.bottom() - seg_off);
        for (how, from, to) in [(layer.pre, cell.left(), x0 + a as f32 * ppf - 1.0), (layer.post, x0 + b as f32 * ppf, cell.right())] {
            let band = Rect::from_min_max(pos2(from.max(cell.left()), top), pos2(to.min(cell.right()), bottom));
            if how == Behaviour::Nothing || band.width() < 2.0 {
                continue;
            }
            painter.rect_filled(band, 1.0, fill.gamma_multiply(0.3));
            if band.width() > 14.0 && !folded {
                let icon = crate::montagehead::behaviour_icon(how);
                painter.with_clip_rect(band).text(pos2(band.right() - 3.0, band.top() + 1.0), Align2::RIGHT_TOP, icon, FontId::proportional(11.0), theme::sur_calque().gamma_multiply(0.8));
            }
        }
    }
    // La barre du calque, de son premier dessin à la fin du dernier ; ses bouts plus clairs. Elle
    // s'éclaire sous la souris et pendant le geste (comme dans la timeline).
    if let Some((a, b)) = content_bounds(layer).filter(|_| !folded && !layer.folder && layer.video.is_none()) {
        let bar = Rect::from_min_max(
            pos2((x0 + a as f32 * ppf + 1.0).max(cell.left()), cell.bottom() - bar_off - bar_h),
            pos2((x0 + b as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - bar_off),
        );
        if bar.width() > 0.0 {
            let base = theme::couleur_du_calque(layer);
            let moving = matches!(hot, Some(Handle::LayerMove { .. }));
            painter.rect_filled(bar, 1.0, if moving { base } else { base.gamma_multiply(0.5) });
            let cap = (bar.width() * 0.5).min(3.0);
            let (start_lit, end_lit) = (matches!(hot, Some(Handle::LayerStart { .. })), matches!(hot, Some(Handle::LayerEnd { .. })));
            for (l, r, on) in [(bar.left(), bar.left() + cap, start_lit), (bar.right() - cap, bar.right(), end_lit)] {
                let c = if on { theme::contour() } else { theme::texte_fort().gamma_multiply(0.7) };
                painter.rect_filled(Rect::from_min_max(pos2(l, bar.top()), pos2(r, bar.bottom())), 1.0, c);
            }
        }
    }
    // Un bord pris (début, fin, limite entre deux dessins) : un trait blanc sur la hauteur de la
    // piste (pendant le geste, la limite a bougé : le dessin le montre).
    let edge = match hot {
        Some(Handle::LayerStart { .. }) => content_bounds(layer).map(|(a, _)| a),
        Some(Handle::LayerEnd { .. }) => content_bounds(layer).map(|(_, b)| b),
        Some(Handle::Boundary { at, .. }) if !held => Some(at),
        _ => None,
    };
    if let Some(f) = edge {
        let x = x0 + f as f32 * ppf;
        if x >= cell.left() - 1.0 && x <= cell.right() + 1.0 {
            painter.line_segment([pos2(x, cell.top()), pos2(x, cell.bottom())], Stroke::new(1.5_f32, theme::contour()));
        }
    }
    // Les calques de couleur liés à ce calque de trait (l'aplat) n'ont pas de piste : un liseré de leur couleur (rose) sous le bloc, tant
    // que la piste est dépliée ; rien quand elle est repliée (leurs clés comptent dans celles du bloc du plan).
    if !folded && layer.couleur.is_none() && !layer.folder {
        let linked: Vec<&Layer> = stack.iter().filter(|c| c.couleur.as_ref().is_some_and(|cc| cc.trait_id == layer.id)).collect();
        let band_h = LISERE_H / linked.len().max(1) as f32;
        for (n, c) in linked.iter().enumerate() {
            let ink = if c.visible { theme::couleur_du_calque(c) } else { theme::couleur_du_calque(c).gamma_multiply(0.35) };
            let y0 = cell.bottom() - LISERE_H + n as f32 * band_h;
            let mut f = 0;
            while f < c.frames.len() {
                let (s, e) = c.run_bounds(f);
                if c.frames[f].is_some() {
                    let seg = Rect::from_min_max(pos2((x0 + s as f32 * ppf).max(cell.left()), y0), pos2((x0 + e as f32 * ppf - 1.0).min(cell.right()), y0 + band_h));
                    if seg.width() > 0.0 {
                        painter.rect_filled(seg, 0.0, ink);
                    }
                }
                f = e;
            }
        }
    }
    if cell.width() > 28.0 && show_name && !folded {
        let text = if layer.visible { theme::sur_calque() } else { theme::texte_doux() };
        painter.with_clip_rect(cell.shrink(1.0)).text(
            pos2(cell.left() + 4.0, (cell.top() + pad + cell.bottom() - seg_off) * 0.5),
            Align2::LEFT_CENTER,
            &layer.name,
            FontId::proportional(name_size),
            text,
        );
    }
    if current {
        // Le calque courant : un cadre autour de sa case. Quand des clés y sont choisies, ce sont
        // elles qui portent le blanc (la nouvelle clé seule, par exemple) : le cadre du calque s'efface.
        let stroke = if chosen.is_empty() {
            Stroke::new(1.5_f32, theme::contour())
        } else {
            Stroke::new(1.0_f32, theme::contour().gamma_multiply(0.3))
        };
        painter.rect_stroke(cell, 2.0, stroke, StrokeKind::Inside);
    }
}
