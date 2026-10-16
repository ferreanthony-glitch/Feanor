//! Pistes de la bande de montage (6f) : la caméra au-dessus des panneaux, les calques et le fond
//! de scène en dessous, puis les sons ; leurs noms dans la colonne de gauche.
//!
//! Chaque panneau a ses propres calques : la rangée r montre, sous chaque panneau, son r-ième
//! calque en partant du haut (comme dans le panneau Calques). Les noms de la colonne de gauche
//! sont ceux du panneau courant. Le fond de scène tient une seule barre par scène.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Visuals};

use crate::app::App;
use crate::document::{Layer, LayerKind, Panel};
use crate::layersui::SCENE_WIDE;
use crate::soundtrack::{HEADERS_W, LANE_GAP, TRACK_H};
use crate::storyboard::{BANDS_H, RULER_H};
use crate::theme;

/// Hauteur de la piste Caméra et d'une piste de calque.
pub(crate) const CAM_H: f32 = 16.0;
pub(crate) const LAYER_H: f32 = 18.0;
/// Au-delà, les calques du dessous ne sont pas montrés dans le montage.
const MAX_LAYER_ROWS: usize = 8;
const CAM_COLOR: Color32 = theme::CAMERA;

/// Rangées de la bande, en distance depuis son haut.
pub(crate) struct Rows {
    /// Pistes de calques (calques propres aux panneaux).
    pub layer_rows: usize,
    /// Une piste « Fond » (calques de toute la scène).
    pub background: bool,
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
    pub fn background_top(&self) -> f32 {
        self.layers_top() + self.layer_rows as f32 * LAYER_H
    }
    pub fn lanes_top(&self) -> f32 {
        self.background_top() + if self.background { LAYER_H } else { 0.0 } + LANE_GAP
    }
    /// Hauteur totale, pistes son comprises.
    pub fn height(&self, sound_tracks: usize) -> f32 {
        self.lanes_top() + sound_tracks as f32 * TRACK_H + 2.0
    }
}

/// Calques propres au panneau (sans ceux de toute la scène), du bas vers le haut.
fn own_layers(p: &Panel) -> Vec<usize> {
    (0..p.layers.len()).filter(|&k| p.layers[k].scene_wide.is_none()).collect()
}

/// Calque de la rangée `row` (0 = en haut) d'un panneau.
fn layer_in_row(p: &Panel, row: usize) -> Option<usize> {
    let own = own_layers(p);
    own.len().checked_sub(1 + row).map(|j| own[j])
}

impl App {
    /// Rangées pour une bande haute de `available` : les panneaux prennent la place qui reste.
    pub(crate) fn montage_rows(&self, available: f32) -> Rows {
        let layer_rows = self.doc.panels.iter().map(|p| own_layers(p).len()).max().unwrap_or(0).min(MAX_LAYER_ROWS);
        let background = self.doc.panels.iter().any(|p| p.layers.iter().any(|l| l.scene_wide.is_some()));
        let mut rows = Rows { layer_rows, background, block_h: 0.0 };
        let fixed = rows.height(self.doc.audio_tracks.len()) + 12.0;
        rows.block_h = (available - fixed).clamp(56.0, 170.0);
        rows
    }

    /// Noms des pistes au-dessus des pistes son, dans la colonne `rect` ; un clic sur un calque
    /// le rend courant.
    pub(crate) fn row_headers(&mut self, ui: &mut egui::Ui, rect: Rect, rows: &Rows) {
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals().clone();
        let lane = |y: f32, h: f32| Rect::from_min_size(pos2(rect.left() + 2.0, rect.top() + y + 1.0), vec2(HEADERS_W - 8.0, h - 2.0));
        let label = |lane: Rect, text: &str, color: Color32| {
            painter.rect_filled(lane, 3.0, theme::ETIQUETTE);
            painter.with_clip_rect(lane.shrink(1.0)).text(
                pos2(lane.left() + 6.0, lane.center().y),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(11.0),
                color,
            );
        };
        label(lane(rows.camera_top(), CAM_H), "Caméra", visuals.text_color());
        let header = lane(rows.blocks_top(), rows.block_h);
        painter.rect_filled(header, 3.0, theme::ETIQUETTE);
        self.project_header(ui, header);
        let panel = &self.doc.panels[self.cur_panel];
        let mut pick = None;
        for row in 0..rows.layer_rows {
            let r = lane(rows.layers_top() + row as f32 * LAYER_H, LAYER_H);
            let Some(k) = layer_in_row(panel, row) else {
                label(r, "", visuals.weak_text_color());
                continue;
            };
            let current = k == self.cur_layer;
            label(r, &panel.layers[k].name, if current { visuals.strong_text_color() } else { visuals.text_color() });
            if current {
                painter.rect_stroke(r, 3.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
            }
            let resp = ui.interact(r, ui.id().with(("piste_calque", row)), Sense::click());
            if resp.on_hover_text("Calque du plan courant : clic pour le rendre courant").clicked() {
                pick = Some(k);
            }
        }
        if rows.background {
            let r = lane(rows.background_top(), LAYER_H);
            label(r, "Fond (scène)", SCENE_WIDE);
            let wide = panel.layers.iter().position(|l| l.scene_wide.is_some());
            if wide.is_some_and(|k| k == self.cur_layer) {
                painter.rect_stroke(r, 3.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
            }
            let resp = ui.interact(r, ui.id().with("piste_fond"), Sense::click());
            if resp.on_hover_text("Calques de toute la scène : le même dessin dans chaque panneau").clicked() {
                pick = wide;
            }
        }
        if let Some(k) = pick {
            self.select_layer(k);
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

    /// Pistes Caméra, calques et fond, dans la bande `r` ; `blocks` : les panneaux à l'écran.
    pub(crate) fn paint_track_rows(&self, painter: &Painter, r: Rect, blocks: &[Rect], ppf: f32, rows: &Rows, visuals: &Visuals) {
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

        // Calques propres aux panneaux.
        for row in 0..rows.layer_rows {
            let lr = lane(rows.layers_top() + row as f32 * LAYER_H, LAYER_H);
            painter.rect_filled(lr, 0.0, theme::RANGEE_SOMBRE);
            for (i, b) in blocks.iter().enumerate() {
                let p = &self.doc.panels[i];
                let Some(k) = layer_in_row(p, row) else { continue };
                let cell = Rect::from_min_max(pos2(b.left(), lr.top()), pos2(b.right(), lr.bottom()));
                if painter.clip_rect().intersects(cell) {
                    let current = i == self.cur_panel && k == self.cur_layer;
                    paint_layer_cell(painter, cell, &p.layers[k], ppf, current, visuals);
                }
            }
        }

        // Fond : une barre par scène qui en a un.
        if rows.background {
            let lr = lane(rows.background_top(), LAYER_H);
            painter.rect_filled(lr, 0.0, theme::RANGEE_SOMBRE);
            let mut i = 0;
            while i < blocks.len() {
                let (s, e) = self.doc.scene_run(i);
                let wide: Vec<&Layer> = self.doc.panels[s].layers.iter().filter(|l| l.scene_wide.is_some()).collect();
                if !wide.is_empty() {
                    let cell = Rect::from_min_max(pos2(blocks[s].left(), lr.top()), pos2(blocks[e - 1].right(), lr.bottom()));
                    let drawn = wide.iter().any(|l| l.frames.iter().any(Option::is_some));
                    let visible = wide.iter().any(|l| l.visible);
                    let fill = match (drawn, visible) {
                        (true, true) => SCENE_WIDE.gamma_multiply(0.6),
                        (true, false) => SCENE_WIDE.gamma_multiply(0.25),
                        _ => theme::CONTROLE,
                    };
                    painter.rect_filled(cell, 2.0, fill);
                    let names: Vec<&str> = wide.iter().map(|l| l.name.as_str()).collect();
                    painter.with_clip_rect(cell.shrink(1.0)).text(
                        pos2(cell.left() + 4.0, cell.center().y),
                        Align2::LEFT_CENTER,
                        names.join(", "),
                        FontId::proportional(10.0),
                        theme::TEXTE_FORT,
                    );
                    let current = (s..e).contains(&self.cur_panel)
                        && self.doc.panels[self.cur_panel].layers.get(self.cur_layer).is_some_and(|l| l.scene_wide.is_some());
                    if current {
                        painter.rect_stroke(cell, 2.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
                    }
                }
                i = e;
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

    /// Calque visé dans les pistes de calques ou de fond : (panneau, calque). `top` : haut de la bande.
    pub(crate) fn layer_row_hit(&self, p: Pos2, top: f32, blocks: &[Rect], rows: &Rows) -> Option<(usize, usize)> {
        let y = p.y - top;
        let in_layers = y >= rows.layers_top() && y < rows.background_top();
        let in_background = rows.background && y >= rows.background_top() && y < rows.background_top() + LAYER_H;
        if !in_layers && !in_background {
            return None;
        }
        let i = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0)?;
        let panel = &self.doc.panels[i];
        let k = if in_layers {
            layer_in_row(panel, ((y - rows.layers_top()) / LAYER_H) as usize)?
        } else {
            panel.layers.iter().position(|l| l.scene_wide.is_some())?
        };
        Some((i, k))
    }
}

/// Un calque sous un panneau : un bloc par dessin exposé (tenu sur plusieurs images), son nom.
fn paint_layer_cell(painter: &Painter, cell: Rect, layer: &Layer, ppf: f32, current: bool, visuals: &Visuals) {
    painter.rect_filled(cell, 2.0, theme::ETIQUETTE);
    let fill = match layer.kind {
        LayerKind::Vector => theme::CALQUE_VECTORIEL,
        LayerKind::Bitmap => theme::CALQUE_BITMAP,
    };
    let fill = if layer.visible { fill } else { fill.gamma_multiply(0.35) };
    let x0 = cell.left() - 1.0;
    let mut f = 0;
    while f < layer.frames.len() {
        let (s, e) = layer.run_bounds(f);
        if layer.frames[f].is_some() {
            let seg = Rect::from_min_max(
                pos2((x0 + s as f32 * ppf).max(cell.left()), cell.top() + 2.0),
                pos2((x0 + e as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - 2.0),
            );
            if seg.width() > 0.0 {
                painter.rect_filled(seg, 1.0, fill);
            }
        }
        f = e;
    }
    if cell.width() > 28.0 {
        let text = if layer.visible { theme::TEXTE_FORT } else { theme::TEXTE_DOUX };
        painter.with_clip_rect(cell.shrink(1.0)).text(
            pos2(cell.left() + 4.0, cell.center().y),
            Align2::LEFT_CENTER,
            &layer.name,
            FontId::proportional(10.0),
            text,
        );
    }
    if current {
        painter.rect_stroke(cell, 2.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
    }
}
