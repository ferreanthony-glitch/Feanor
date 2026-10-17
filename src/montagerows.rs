//! Pistes de la bande de montage (6f) : la caméra au-dessus des panneaux, les calques et le fond
//! de scène en dessous, puis les sons ; leurs noms dans la colonne de gauche.
//!
//! Chaque plan a ses propres calques. Depuis le chantier 5, une piste par nom de calque : le
//! calque « Perso » de chaque plan est sur la même piste, où qu'il soit dans sa pile ; un plan
//! sans calque de ce nom y laisse un trou. Toutes les pistes sont montrées (avant : 8 au plus,
//! par rang). Le fond de scène tient une seule barre par scène.
//!
//! Les gestes de la timeline marchent dans les pistes (chantier 5) : le bord droit d'un dessin
//! étire son exposition, la barre sous les dessins décale le calque, ses bouts le rognent ou
//! l'allongent (`handles.rs`, une étape d'annulation par geste). Depuis le 2026-10-17 aussi : un
//! dessin pris par son milieu glisse dans le temps en gardant sa durée, un double-clic pose un
//! nouveau dessin (une clé d'animation) à l'image visée.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Visuals};

use crate::app::App;
use crate::document::{Layer, LayerKind};
use crate::handles::{content_bounds, Handle};
use crate::layersui::SCENE_WIDE;
use crate::soundtrack::{HEADERS_W, LANE_GAP, TRACK_H};
use crate::storyboard::{BANDS_H, RULER_H};
use crate::theme;

/// Hauteur de la piste Caméra et d'une piste de calque.
pub(crate) const CAM_H: f32 = 16.0;
pub(crate) const LAYER_H: f32 = 20.0;
/// Barre d'un calque sous ses dessins (chantier 5, comme dans la timeline) : on la prend pour
/// décaler le calque.
const BAR_H: f32 = 3.0;
const CAM_COLOR: Color32 = theme::CAMERA;

/// Une piste de calque du montage (chantier 5) : un nom de calque et, quand un plan a plusieurs
/// calques de ce nom, le rang de celui-ci parmi eux (0 : le plus haut).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrackKey {
    pub name: String,
    pub nth: usize,
}

impl TrackKey {
    /// Le nom affiché : « Perso », « Perso (2) » pour le deuxième calque « Perso » d'un plan.
    pub fn label(&self) -> String {
        if self.nth == 0 {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.nth + 1)
        }
    }
}

/// Les pistes de calques du montage, du haut vers le bas : chaque nom une fois (et chaque doublon
/// d'un même plan). On suit les plans dans l'ordre, chacun du haut de sa pile vers le bas : un nom
/// nouveau vient juste sous la piste du calque qui le précède dans son plan (tout en haut s'il est
/// le premier). Les calques de toute la scène (fond) n'ont pas de piste ici.
pub(crate) fn layer_tracks<'a>(plans: impl IntoIterator<Item = &'a [Layer]>) -> Vec<TrackKey> {
    let mut out: Vec<TrackKey> = Vec::new();
    for layers in plans {
        let mut after: Option<usize> = None;
        let mut seen: Vec<&str> = Vec::new();
        for l in layers.iter().rev().filter(|l| l.scene_wide.is_none()) {
            let nth = seen.iter().filter(|n| **n == l.name).count();
            seen.push(&l.name);
            match out.iter().position(|t| t.nth == nth && t.name == l.name) {
                Some(i) => after = Some(i),
                None => {
                    let at = after.map_or(0, |i| i + 1);
                    out.insert(at, TrackKey { name: l.name.clone(), nth });
                    after = Some(at);
                }
            }
        }
    }
    out
}

/// Le calque d'un plan (`layers`, du bas vers le haut) sur la piste `key`.
pub(crate) fn layer_on_track(layers: &[Layer], key: &TrackKey) -> Option<usize> {
    (0..layers.len()).rev().filter(|&k| layers[k].scene_wide.is_none() && layers[k].name == key.name).nth(key.nth)
}

/// Rangées de la bande, en distance depuis son haut.
pub(crate) struct Rows {
    /// Pistes de calques (calques propres aux plans), du haut vers le bas.
    pub tracks: Vec<TrackKey>,
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
        self.layers_top() + self.tracks.len() as f32 * LAYER_H
    }
    pub fn lanes_top(&self) -> f32 {
        self.background_top() + if self.background { LAYER_H } else { 0.0 } + LANE_GAP
    }
    /// Hauteur totale, pistes son comprises.
    pub fn height(&self, sound_tracks: usize) -> f32 {
        self.lanes_top() + sound_tracks as f32 * TRACK_H + 2.0
    }
}


impl App {
    /// Rangées pour une bande haute de `available` : les panneaux prennent la place qui reste.
    pub(crate) fn montage_rows(&self, available: f32) -> Rows {
        let tracks = layer_tracks(self.doc.panels.iter().map(|p| p.layers.as_slice()));
        let background = self.doc.panels.iter().any(|p| p.layers.iter().any(|l| l.scene_wide.is_some()));
        let mut rows = Rows { tracks, background, block_h: 0.0 };
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
        let cam = lane(rows.camera_top(), CAM_H);
        label(cam, "Caméra", visuals.text_color());
        // Clic droit sur « Caméra » : le menu de la caméra (passe interface).
        ui.interact(cam, ui.id().with("nom_piste_camera"), Sense::click()).context_menu(|ui| self.grid_menu_ui(ui, crate::handles::GridMenu::Camera));
        let header = lane(rows.blocks_top(), rows.block_h);
        painter.rect_filled(header, 3.0, theme::ETIQUETTE);
        self.project_header(ui, header);
        // Une piste par nom de calque (chantier 5). Celles du plan courant : clic, son calque
        // devient courant ; glisser, il monte ou descend ; clic droit, son menu (passe interface).
        let names: Vec<(usize, String, Option<usize>)> = {
            let panel = &self.doc.panels[self.cur_panel];
            rows.tracks.iter().enumerate().map(|(row, t)| (row, t.label(), layer_on_track(&panel.layers, t))).collect()
        };
        let mut pick = None;
        let mut grid: Vec<crate::layerheads::Row> = Vec::new();
        for (row, name, item) in names {
            let r = lane(rows.layers_top() + row as f32 * LAYER_H, LAYER_H);
            let Some(k) = item else {
                label(r, &name, visuals.weak_text_color());
                ui.interact(r, ui.id().with(("piste_calque_absente", row)), Sense::hover())
                    .on_hover_text("Le plan courant n'a pas de calque de ce nom.");
                continue;
            };
            grid.push(crate::layerheads::Row { li: k, top: row as f32 * LAYER_H, h: LAYER_H });
            let current = k == self.cur_layer;
            label(r, &name, if current { visuals.strong_text_color() } else { visuals.text_color() });
            if current {
                painter.rect_stroke(r, 3.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
            }
            let resp = ui.interact(r, ui.id().with(("piste_calque", row)), Sense::click_and_drag());
            if resp.drag_started() {
                self.montage_layer_drag = Some(k);
            }
            let resp = resp.on_hover_text("Calque du plan courant : clic, il devient courant ; glisser, il monte ou descend ; clic droit, son menu.");
            if resp.clicked() || resp.secondary_clicked() {
                pick = Some(k);
            }
            resp.context_menu(|ui| self.layer_menu(ui, k, false));
        }
        if let Some(from) = self.montage_layer_drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let base = rect.top() + rows.layers_top();
            let pointer = ui.input(|i| i.pointer.interact_pos());
            if let Some(p) = pointer {
                let slot = grid.iter().filter(|g| g.top + g.h * 0.5 < p.y - base).count();
                let y = base + grid.get(slot).map_or(grid.len() as f32 * LAYER_H, |g| g.top);
                painter.line_segment([pos2(rect.left() + 2.0, y), pos2(rect.left() + HEADERS_W - 6.0, y)], Stroke::new(2.0_f32, Color32::WHITE));
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
        let panel = &self.doc.panels[self.cur_panel];
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

    /// Pistes Caméra, calques et fond, dans la bande `r` ; `blocks` : les panneaux à l'écran ;
    /// `lit` : la poignée de calque survolée ou tenue, et son plan.
    pub(crate) fn paint_track_rows(&self, painter: &Painter, r: Rect, blocks: &[Rect], ppf: f32, rows: &Rows, visuals: &Visuals, lit: Option<(usize, Handle)>) {
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

        // Calques propres aux plans : une piste par nom (chantier 5).
        for (row, key) in rows.tracks.iter().enumerate() {
            let lr = lane(rows.layers_top() + row as f32 * LAYER_H, LAYER_H);
            painter.rect_filled(lr, 0.0, theme::RANGEE_SOMBRE);
            for (i, b) in blocks.iter().enumerate() {
                let cell = Rect::from_min_max(pos2(b.left(), lr.top()), pos2(b.right(), lr.bottom()));
                if !painter.clip_rect().intersects(cell) {
                    continue;
                }
                let p = &self.doc.panels[i];
                let Some(k) = layer_on_track(&p.layers, key) else { continue };
                let current = i == self.cur_panel && k == self.cur_layer;
                let hot = lit.filter(|(pi, h)| *pi == i && h.layer() == Some(k)).map(|(_, h)| h);
                paint_layer_cell(painter, cell, &p.layers[k], ppf, current, hot, visuals);
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
            layer_on_track(&panel.layers, rows.tracks.get(((y - rows.layers_top()) / LAYER_H) as usize)?)?
        } else {
            panel.layers.iter().position(|l| l.scene_wide.is_some())?
        };
        Some((i, k))
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
        let key = rows.tracks.get(row)?;
        let lane_top = top + rows.layers_top() + row as f32 * LAYER_H;
        let tol = (ppf * 0.3).clamp(2.0, 5.0);
        let strict = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0);
        let near = blocks.iter().enumerate().filter(|(i, b)| Some(*i) != strict && p.x >= b.left() - 1.0 - tol && p.x <= b.right() + 1.0 + tol).map(|(i, _)| i);
        strict.into_iter().chain(near).find_map(|i| {
            let layers = &self.doc.panels[i].layers;
            let k = layer_on_track(layers, key)?;
            cell_handle(&layers[k], k, p, blocks[i].left() - 1.0, lane_top, ppf, tol).map(|h| (i, k, h))
        })
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
    let f = ((p.x - x0) / ppf).round().max(0.0) as usize;
    if f >= 1 && near(x(f)) {
        let (s, e) = layer.run_bounds(f - 1);
        if e == f && layer.frames.get(s).is_some_and(Option::is_some) {
            return Some(Handle::Exposure { layer: k, start: s, end: e });
        }
    }
    // Le milieu d'un dessin : il glisse dans le temps, sa durée gardée.
    let grab = ((p.x - x0) / ppf).floor().max(0.0) as usize;
    if layer.frames.get(grab).is_some_and(Option::is_some) {
        let (s, e) = layer.run_bounds(grab);
        return Some(Handle::Key { layer: k, start: s, end: e, grab });
    }
    None
}

/// Un calque sous un plan : un bloc par dessin exposé (tenu sur plusieurs images), sa barre
/// dessous (chantier 5), son nom. `hot` : sa poignée survolée ou tenue.
fn paint_layer_cell(painter: &Painter, cell: Rect, layer: &Layer, ppf: f32, current: bool, hot: Option<Handle>, visuals: &Visuals) {
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
                pos2((x0 + e as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - 3.0 - BAR_H),
            );
            if seg.width() > 0.0 {
                painter.rect_filled(seg, 1.0, fill);
                // Le dessin survolé (on le prend par son milieu pour le déplacer) : cerclé de blanc.
                if matches!(hot, Some(Handle::Key { start, .. }) if start == s) {
                    painter.rect_stroke(seg, 1.0, Stroke::new(1.5_f32, Color32::WHITE), StrokeKind::Inside);
                }
            }
        }
        f = e;
    }
    // La barre du calque, de son premier dessin à la fin du dernier ; ses bouts plus clairs. Elle
    // s'éclaire sous la souris et pendant le geste (comme dans la timeline).
    if let Some((a, b)) = content_bounds(layer).filter(|_| !layer.folder && layer.video.is_none()) {
        let bar = Rect::from_min_max(
            pos2((x0 + a as f32 * ppf + 1.0).max(cell.left()), cell.bottom() - 2.0 - BAR_H),
            pos2((x0 + b as f32 * ppf - 1.0).min(cell.right()), cell.bottom() - 2.0),
        );
        if bar.width() > 0.0 {
            let base = match layer.kind {
                LayerKind::Vector => theme::DESSIN_VECTORIEL,
                LayerKind::Bitmap => theme::DESSIN_BITMAP,
            };
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
    // Un bord pris (début, fin, exposition) : un trait blanc sur la hauteur de la piste.
    let edge = match hot {
        Some(Handle::LayerStart { .. }) => content_bounds(layer).map(|(a, _)| a),
        Some(Handle::LayerEnd { .. }) => content_bounds(layer).map(|(_, b)| b),
        // Pendant le geste, le dessin a déjà sa nouvelle durée.
        Some(Handle::Exposure { start, .. }) => (start < layer.frames.len()).then(|| layer.run_bounds(start).1),
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
            FontId::proportional(10.0),
            text,
        );
    }
    if current {
        painter.rect_stroke(cell, 2.0, Stroke::new(1.5_f32, visuals.selection.stroke.color), StrokeKind::Inside);
    }
}
