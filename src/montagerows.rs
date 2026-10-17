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

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Visuals};

use crate::app::App;
use crate::document::{Behaviour, Layer};
use crate::handles::{content_bounds, Handle};
use crate::layersui::SCENE_WIDE;
use crate::soundtrack::{LANE_GAP, TRACK_H};
use crate::storyboard::{BANDS_H, RULER_H};
use crate::theme;

/// Hauteur de la piste Caméra et d'une piste de calque.
pub(crate) const CAM_H: f32 = 16.0;
/// Une piste de calque : haute depuis que le montage est le banc principal (Anthony,
/// 2026-10-17 : « pour rendre la saisie des clés plus confortable » ; 20, puis 26, puis 40).
pub(crate) const LAYER_H: f32 = 40.0;
/// Barre d'un calque sous ses dessins (chantier 5, comme dans la timeline) : on la prend pour
/// décaler le calque.
const BAR_H: f32 = 5.0;
const CAM_COLOR: Color32 = theme::CAMERA;

/// Une piste de calque du montage (chantier 5) : un nom de calque et, quand un plan a plusieurs
/// calques de ce nom, le rang de celui-ci parmi eux (0 : le plus haut).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrackKey {
    pub name: String,
    pub nth: usize,
}


/// Le calque d'un plan (`layers`, du bas vers le haut) à la rangée `row` du montage (0 : en
/// haut) : chaque plan montre ses calques dans son propre ordre, du haut de sa pile vers le bas
/// (Anthony, 2026-10-17 : « l'organisation et l'ordre des calques indépendants d'un shot à un
/// autre »).
pub(crate) fn layer_in_row(layers: &[Layer], row: usize) -> Option<usize> {
    layers.len().checked_sub(row + 1)
}
/// La piste du calque `li` d'un plan (`layers`, du bas vers le haut) : son nom, et son rang parmi
/// les calques du même nom en partant du haut.
pub(crate) fn track_of(layers: &[Layer], li: usize) -> TrackKey {
    let name = layers[li].name.clone();
    let nth = layers[li + 1..].iter().filter(|l| l.name == name).count();
    TrackKey { name, nth }
}

/// Le calque d'un plan (`layers`, du bas vers le haut) sur la piste `key`.
pub(crate) fn layer_on_track(layers: &[Layer], key: &TrackKey) -> Option<usize> {
    (0..layers.len()).rev().filter(|&k| layers[k].name == key.name).nth(key.nth)
}

/// Rangées de la bande, en distance depuis son haut.
pub(crate) struct Rows {
    /// Rangées de calques : autant que le plan qui en a le plus.
    pub count: usize,
    pub block_h: f32,
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
    /// Bas des pistes de calques.
    pub fn background_top(&self) -> f32 {
        self.layers_top() + self.count as f32 * LAYER_H
    }
    pub fn lanes_top(&self) -> f32 {
        self.background_top() + LANE_GAP
    }
    /// Hauteur totale, pistes son comprises.
    pub fn height(&self, sound_tracks: usize) -> f32 {
        self.lanes_top() + sound_tracks as f32 * TRACK_H + 2.0
    }
}


impl App {
    /// Rangées pour une bande haute de `available` : les panneaux prennent la place qui reste.
    pub(crate) fn montage_rows(&self, available: f32) -> Rows {
        let count = self.doc.panels.iter().map(|p| p.layers.len()).max().unwrap_or(0);
        let mut rows = Rows { count, block_h: 0.0 };
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
            painter.rect_filled(lane, 3.0, theme::ETIQUETTE);
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
        painter.rect_filled(header, 3.0, theme::ETIQUETTE);
        self.project_header(ui, header);
        // Les calques du plan courant, dans son ordre : clic, il devient courant ; glisser, il monte
        // ou descend (dans ce plan seulement) ; clic droit, son menu (passe interface).
        let names: Vec<(usize, String, Option<usize>)> = {
            let panel = &self.doc.panels[self.cur_panel];
            (0..rows.count)
                .map(|row| {
                    let k = layer_in_row(&panel.layers, row);
                    (row, k.map_or(String::new(), |k| panel.layers[k].name.clone()), k)
                })
                .collect()
        };
        let mut pick = None;
        let mut grid: Vec<crate::layerheads::Row> = Vec::new();
        for (row, name, item) in names {
            let r = lane(rows.layers_top() + row as f32 * LAYER_H, LAYER_H);
            let Some(k) = item else {
                label(r, &name, visuals.weak_text_color());
                ui.interact(r, ui.id().with(("piste_calque_absente", row)), Sense::hover())
                    .on_hover_text("Le plan courant a moins de calques : cette rangée est à d'autres plans.");
                continue;
            };
            grid.push(crate::layerheads::Row { li: k, top: row as f32 * LAYER_H, h: LAYER_H });
            let current = k == self.cur_layer;
            // Un calque de toute la scène : son nom teinté « fond ».
            let wide = self.doc.panels[self.cur_panel].layers[k].scene_wide.is_some();
            let color = match (wide, current) {
                (true, _) => SCENE_WIDE,
                (false, true) => visuals.strong_text_color(),
                (false, false) => visuals.text_color(),
            };
            // Le fond de la rangée (choisir, glisser, menu), puis le nom et les réglages qui tiennent
            // dans la largeur de la colonne, au-dessus (`montagehead.rs`).
            painter.rect_filled(r, 3.0, theme::ETIQUETTE);
            // La bande de sa sorte (bitmap vert, vectoriel bleu, CTG rose), au bord gauche.
            let l = &self.doc.panels[self.cur_panel].layers[k];
            if !l.folder {
                let band = Rect::from_min_size(r.left_top() + vec2(0.0, 3.0), vec2(3.0, r.height() - 6.0));
                painter.rect_filled(band, 1.0, theme::couleur_calque(l.kind, l.ctg.is_some()));
            }
            if current {
                painter.rect_stroke(r, 3.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
            }
            // Choisi avec d'autres (Ctrl ou Maj + clic) : surligné plus légèrement.
            if !current && self.is_chosen_layer(k) {
                painter.rect_filled(r, 3.0, visuals.selection.bg_fill.gamma_multiply(0.3));
            }
            let resp = ui.interact(r, ui.id().with(("piste_calque", row)), Sense::click_and_drag());
            if resp.drag_started() {
                self.montage_layer_drag = Some(k);
            }
            let resp = resp.on_hover_text(
                "Calque du plan courant : clic, il devient courant ; double-clic sur le nom, le renommer ; glisser, il monte ou descend ; clic droit, tout ce qui touche au calque. Tirer le bord droit de la colonne : ses réglages.",
            );
            if resp.clicked() || resp.secondary_clicked() {
                pick = Some((k, resp.secondary_clicked()));
            }
            if resp.double_clicked() && resp.interact_pointer_pos().is_some_and(|p| crate::montagehead::head_layout(r).name.contains(p)) {
                let id = self.doc.panels[self.cur_panel].layers[k].id;
                self.layer_rename = Some(crate::layersui::LayerRename { in_montage: true, ..crate::layersui::LayerRename::new(id, &name, false) });
            }
            resp.context_menu(|ui| self.montage_layer_menu(ui, k));
            self.montage_layer_head(ui, r, k, color);
        }
        if let Some(from) = self.montage_layer_drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let base = rect.top() + rows.layers_top();
            let pointer = ui.input(|i| i.pointer.interact_pos());
            if let Some(p) = pointer {
                let slot = grid.iter().filter(|g| g.top + g.h * 0.5 < p.y - base).count();
                let y = base + grid.get(slot).map_or(grid.len() as f32 * LAYER_H, |g| g.top);
                painter.line_segment([pos2(rect.left() + 2.0, y), pos2(rect.right() - 6.0, y)], Stroke::new(2.0_f32, Color32::WHITE));
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
        visuals: &Visuals,
        lit: Option<(usize, Handle, bool)>,
        drops: &[crate::montagekeys::DropCell],
    ) {
        let top = r.top();
        let lane = |y: f32, h: f32| Rect::from_min_max(pos2(r.left(), top + y + 1.0), pos2(r.right(), top + y + h - 1.0));

        // Caméra : une clé = un losange ; un mouvement = une barre de la première à la dernière.
        let cam = lane(rows.camera_top(), CAM_H);
        painter.rect_filled(cam, 0.0, theme::RANGEE_SOMBRE);
        for (i, b) in blocks.iter().enumerate() {
            let keys = &self.doc.panels[i].camera;
            let x0 = b.left() - 1.0;
            let y = cam.center().y;
            if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
                if keys.len() >= 2 {
                    let bar = Rect::from_min_max(
                        pos2(x0 + first.frame as f32 * ppf + ppf * 0.5, y - 1.5),
                        pos2(x0 + last.frame as f32 * ppf + ppf * 0.5, y + 1.5),
                    );
                    painter.rect_filled(bar, 1.0, CAM_COLOR.gamma_multiply(0.5));
                }
            }
            for (k, key) in keys.iter().enumerate() {
                let c = pos2(x0 + key.frame as f32 * ppf + ppf * 0.5, y);
                let d = 4.0;
                // La clé que l'on fait glisser : cerclée de blanc.
                let outline = if self.is_dragged_camera_key(i, k) { Stroke::new(1.5_f32, Color32::WHITE) } else { Stroke::NONE };
                painter.add(egui::Shape::convex_polygon(
                    vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)],
                    CAM_COLOR,
                    outline,
                ));
            }
        }

        // Les calques de chaque plan, dans son ordre.
        for row in 0..rows.count {
            let lr = lane(rows.layers_top() + row as f32 * LAYER_H, LAYER_H);
            painter.rect_filled(lr, 0.0, theme::RANGEE_SOMBRE);
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
                paint_layer_cell(painter, cell, &p.layers[k], ppf, current, hot, &chosen, visuals);
                // Clés qu'on glisse : leur fantôme suit la souris, un repère montre où elles se
                // poseront (comme le repère de Storyboard Pro entre deux panneaux).
                if let Some(d) = drops.iter().find(|d| d.panel == p.id && d.layer == p.layers[k].id) {
                    paint_drop(painter, cell, ppf, d, visuals);
                }
            }
        }
    }

    /// Clé de caméra sous le point `p` dans la piste Caméra : (panneau, clé). `top` : haut de la
    /// bande ; `blocks` : les panneaux à l'écran.
    pub(crate) fn montage_camera_key_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize)> {
        let y = p.y - top;
        if y < rows.camera_top() || y > rows.camera_top() + CAM_H {
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

    /// Calque visé dans les pistes de calques : (panneau, calque). `top` : haut de la bande.
    pub(crate) fn layer_row_hit(&self, p: Pos2, top: f32, blocks: &[Rect], rows: &Rows) -> Option<(usize, usize)> {
        let row = self.row_at(p, top, rows)?;
        let i = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0)?;
        Some((i, layer_in_row(&self.doc.panels[i].layers, row)?))
    }

    /// La rangée de calques sous le point `p` (`top` : haut de la bande).
    pub(crate) fn row_at(&self, p: Pos2, top: f32, rows: &Rows) -> Option<usize> {
        let y = p.y - top;
        if y < rows.layers_top() || y >= rows.background_top() {
            return None;
        }
        Some(((y - rows.layers_top()) / LAYER_H) as usize).filter(|&r| r < rows.count)
    }

    /// Poignée de calque sous le point `p`, dans les pistes de calques (chantier 5, les gestes de
    /// la timeline) : le début ou la fin du calque, sa barre (en bas de la piste), le bord droit
    /// d'un dessin. Tout contre la limite de deux plans, celui sous le point d'abord, puis son
    /// voisin. Renvoie (plan, calque, poignée). `top` : haut de la bande.
    pub(crate) fn montage_layer_handle_at(&self, p: Pos2, top: f32, blocks: &[Rect], ppf: f32, rows: &Rows) -> Option<(usize, usize, Handle)> {
        let y = p.y - top;
        if y < rows.layers_top() || y >= rows.background_top() {
            return None;
        }
        let row = ((y - rows.layers_top()) / LAYER_H) as usize;
        if row >= rows.count {
            return None;
        }
        let lane_top = top + rows.layers_top() + row as f32 * LAYER_H;
        let tol = (ppf * 0.3).clamp(3.0, 6.0);
        let strict = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0);
        let near = blocks.iter().enumerate().filter(|(i, b)| Some(*i) != strict && p.x >= b.left() - 1.0 - tol && p.x <= b.right() + 1.0 + tol).map(|(i, _)| i);
        strict.into_iter().chain(near).find_map(|i| {
            let layers = &self.doc.panels[i].layers;
            let k = layer_in_row(layers, row)?;
            cell_handle(&layers[k], k, p, blocks[i].left() - 1.0, lane_top, ppf, tol).map(|h| (i, k, h))
        })
    }
}

/// Les fantômes des clés qu'on glisse (cadres translucides, coupés au plan) et les repères où
/// elles se poseront (un trait épais sur la hauteur de la piste, avec deux pointes).
fn paint_drop(painter: &Painter, cell: Rect, ppf: f32, d: &crate::montagekeys::DropCell, visuals: &Visuals) {
    let x0 = cell.left() - 1.0;
    let accent = visuals.selection.stroke.color;
    let clip = painter.with_clip_rect(cell.expand(1.0).intersect(painter.clip_rect()));
    for &(s, e) in &d.ghosts {
        let ghost = Rect::from_min_max(pos2(x0 + s as f32 * ppf, cell.top() + 2.0), pos2(x0 + e as f32 * ppf - 1.0, cell.bottom() - 2.0));
        clip.rect(ghost, 2.0, accent.gamma_multiply(0.25), Stroke::new(1.5_f32, accent), StrokeKind::Inside);
    }
    for &m in &d.marks {
        let x = x0 + m as f32 * ppf;
        clip.line_segment([pos2(x, cell.top()), pos2(x, cell.bottom())], Stroke::new(3.0_f32, Color32::WHITE));
        let t = 5.0;
        clip.add(egui::Shape::convex_polygon(vec![pos2(x - t, cell.top()), pos2(x + t, cell.top()), pos2(x, cell.top() + t)], Color32::WHITE, Stroke::NONE));
        clip.add(egui::Shape::convex_polygon(vec![pos2(x - t, cell.bottom()), pos2(x, cell.bottom() - t), pos2(x + t, cell.bottom())], Color32::WHITE, Stroke::NONE));
    }
}

/// Poignée du calque `layer` (rang `k`) sous le point `p` ; `x0` : l'abscisse de la première
/// image de son plan, `lane_top` : le haut de sa piste.
pub(crate) fn cell_handle(layer: &Layer, k: usize, p: Pos2, x0: f32, lane_top: f32, ppf: f32, tol: f32) -> Option<Handle> {
    if layer.folder || layer.video.is_some() {
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
    let in_bar = p.y >= lane_top + LAYER_H - 3.0 - BAR_H - 1.0;
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
/// dessous (chantier 5), son nom. `hot` : sa poignée survolée ou tenue (vrai : tenue) ;
/// `chosen` : début des clés choisies.
#[allow(clippy::too_many_arguments)]
fn paint_layer_cell(painter: &Painter, cell: Rect, layer: &Layer, ppf: f32, current: bool, hot: Option<(Handle, bool)>, chosen: &[usize], visuals: &Visuals) {
    let (hot, held) = (hot.map(|h| h.0), hot.is_some_and(|h| h.1));
    painter.rect_filled(cell, 2.0, theme::ETIQUETTE);
    // Un calque de toute la scène : ses dessins teintés « fond ».
    let fill = match (layer.scene_wide.is_some(), layer.kind) {
        (true, _) => SCENE_WIDE,
        (false, kind) => theme::fond_calque(kind, layer.ctg.is_some()),
    };
    let fill = if layer.visible { fill } else { fill.gamma_multiply(0.35) };
    let x0 = cell.left() - 1.0;
    let mut f = 0;
    while f < layer.frames.len() {
        let (s, e) = layer.run_bounds(f);
        if layer.frames[f].is_some() {
            let seg = Rect::from_min_max(
                pos2((x0 + s as f32 * ppf).max(cell.left()), cell.top() + 2.0),
                pos2((x0 + e as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - 3.0 - BAR_H),
            );
            if seg.width() > 0.0 {
                painter.rect_filled(seg, 1.0, fill);
                // Une clé choisie : cerclée de la couleur du choix ; survolée (on la prend par son
                // milieu pour la déplacer) : de blanc.
                if chosen.contains(&s) {
                    painter.rect_stroke(seg, 1.0, Stroke::new(2.0_f32, visuals.selection.stroke.color), StrokeKind::Inside);
                }
                if matches!(hot, Some(Handle::Key { start, .. }) if start == s) {
                    painter.rect_stroke(seg, 1.0, Stroke::new(1.5_f32, Color32::WHITE), StrokeKind::Inside);
                }
            }
        }
        f = e;
    }
    // Avant la première image et après la dernière : ce que le calque y montre (boucle,
    // va-et-vient, maintenir ; 13c), en pâle jusqu'au bord du plan, avec son icône.
    if let Some((a, b)) = content_bounds(layer).filter(|_| !layer.folder && layer.video.is_none()) {
        let (top, bottom) = (cell.top() + 2.0, cell.bottom() - 3.0 - BAR_H);
        for (how, from, to) in [(layer.pre, cell.left(), x0 + a as f32 * ppf - 1.0), (layer.post, x0 + b as f32 * ppf, cell.right())] {
            let band = Rect::from_min_max(pos2(from.max(cell.left()), top), pos2(to.min(cell.right()), bottom));
            if how == Behaviour::Nothing || band.width() < 2.0 {
                continue;
            }
            painter.rect_filled(band, 1.0, fill.gamma_multiply(0.3));
            if band.width() > 14.0 {
                let icon = crate::montagehead::behaviour_icon(how);
                painter.with_clip_rect(band).text(pos2(band.right() - 3.0, band.top() + 1.0), Align2::RIGHT_TOP, icon, FontId::proportional(11.0), theme::TEXTE_FORT.gamma_multiply(0.8));
            }
        }
    }
    // La barre du calque, de son premier dessin à la fin du dernier ; ses bouts plus clairs. Elle
    // s'éclaire sous la souris et pendant le geste (comme dans la timeline).
    if let Some((a, b)) = content_bounds(layer).filter(|_| !layer.folder && layer.video.is_none()) {
        let bar = Rect::from_min_max(
            pos2((x0 + a as f32 * ppf + 1.0).max(cell.left()), cell.bottom() - 2.0 - BAR_H),
            pos2((x0 + b as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - 2.0),
        );
        if bar.width() > 0.0 {
            let base = theme::couleur_calque(layer.kind, layer.ctg.is_some());
            let moving = matches!(hot, Some(Handle::LayerMove { .. }));
            painter.rect_filled(bar, 1.0, if moving { base } else { base.gamma_multiply(0.5) });
            let cap = (bar.width() * 0.5).min(3.0);
            let (start_lit, end_lit) = (matches!(hot, Some(Handle::LayerStart { .. })), matches!(hot, Some(Handle::LayerEnd { .. })));
            for (l, r, on) in [(bar.left(), bar.left() + cap, start_lit), (bar.right() - cap, bar.right(), end_lit)] {
                let c = if on { Color32::WHITE } else { theme::TEXTE_FORT.gamma_multiply(0.7) };
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
            painter.line_segment([pos2(x, cell.top()), pos2(x, cell.bottom())], Stroke::new(1.5_f32, Color32::WHITE));
        }
    }
    if cell.width() > 28.0 {
        let text = if layer.visible { theme::TEXTE_FORT } else { theme::TEXTE_DOUX };
        painter.with_clip_rect(cell.shrink(1.0)).text(
            pos2(cell.left() + 4.0, cell.center().y),
            Align2::LEFT_CENTER,
            &layer.name,
            FontId::proportional(12.0),
            text,
        );
    }
    if current {
        painter.rect_stroke(cell, 2.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
    }
}
