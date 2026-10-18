//! Panneau « Feuille d'exposition » (13g-1) : la feuille du plan courant, une ligne par image,
//! en affichage occidental (voir `xsheet.rs`). Un clic sur une case va à cette image (et à ce
//! calque) ; la feuille suit l'image courante. L'en-tête reste en haut quand on fait défiler.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::theme;
use crate::xsheet::{camera_cells, layer_cells, story_rows, CamCell, Cell};

const ROW_H: f32 = 18.0;
const HEADER_H: f32 = 34.0;
const IMAGE_W: f32 = 52.0;
const TEXT_W: f32 = 150.0;
const AUDIO_W: f32 = 34.0;
const LAYER_W: f32 = 58.0;
const CAMERA_W: f32 = 40.0;

/// Une colonne de la feuille.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Image,
    Action,
    Dialogue,
    Audio,
    Layer(usize),
    Camera,
}

impl Col {
    fn width(self) -> f32 {
        match self {
            Col::Image => IMAGE_W,
            Col::Action | Col::Dialogue => TEXT_W,
            Col::Audio => AUDIO_W,
            Col::Layer(_) => LAYER_W,
            Col::Camera => CAMERA_W,
        }
    }
}

impl App {
    /// Les colonnes montrées, de gauche à droite : image, son, calques (dans l'ordre de la
    /// timeline, le plus haut à gauche, sans les dossiers ni les groupes cachés), caméra, puis
    /// les textes, plus larges, au bout (le panneau est souvent étroit).
    fn xsheet_columns(&self) -> Vec<Col> {
        let s = &self.xsheet;
        let mut cols = vec![Col::Image];
        if s.audio {
            cols.push(Col::Audio);
        }
        if s.layers {
            let layers = self.layers();
            for li in (0..layers.len()).rev() {
                let l = &layers[li];
                let group_shown = self.doc.groups.get(l.group as usize).is_none_or(|g| g.shown);
                if !l.folder && group_shown && (l.visible || s.hidden_layers) {
                    cols.push(Col::Layer(li));
                }
            }
        }
        if s.camera {
            cols.push(Col::Camera);
        }
        if s.action {
            cols.push(Col::Action);
        }
        if s.dialogue {
            cols.push(Col::Dialogue);
        }
        cols
    }

    pub(crate) fn xsheet_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let resp = ui.menu_button(RichText::new(ph::EYE).size(crate::bulles::TAILLE), |ui| self.xsheet_display_menu(ui)).response;
            bulle(resp, "Infos affichées", "Colonnes de la feuille, comportements, image courante, repères, trait de séparation.", "");
            ui.label(RichText::new(crate::storyboard::panel_title(&self.doc, self.cur_panel)).strong());
            ui.label(RichText::new(format!("{} images", self.frame_count())).weak());
        });
        let cols = self.xsheet_columns();
        let len = self.frame_count();
        let width: f32 = cols.iter().map(|c| c.width()).sum();
        egui::ScrollArea::both().id_salt("feuille_expo").auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
            let (rect, resp) = ui.allocate_exact_size(vec2(width, HEADER_H + len as f32 * ROW_H), Sense::click());
            let origin = rect.min;
            // La feuille suit l'image courante.
            if self.xsheet_seen_frame != Some(self.cur_frame) {
                self.xsheet_seen_frame = Some(self.cur_frame);
                let y = origin.y + HEADER_H + self.cur_frame as f32 * ROW_H;
                ui.scroll_to_rect(Rect::from_min_size(pos2(origin.x, y - HEADER_H), vec2(1.0, ROW_H + HEADER_H)), None);
            }
            let first = ((viewport.min.y - HEADER_H) / ROW_H).floor().max(0.0) as usize;
            let last = (((viewport.max.y - HEADER_H) / ROW_H).ceil().max(0.0) as usize + 1).min(len);
            let painter = ui.painter_at(ui.clip_rect());
            self.paint_xsheet_rows(&painter, origin, &cols, first, last);
            // L'en-tête, collé en haut de la partie visible.
            let header = Rect::from_min_size(pos2(origin.x, origin.y + viewport.min.y), vec2(width, HEADER_H));
            self.paint_xsheet_header(&painter, header, &cols);
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    self.click_xsheet(p, origin, header, &cols, len);
                }
            }
            // Clic droit sur une case de calque : le menu des rôles (13g-2).
            if resp.secondary_clicked() {
                self.xsheet_role_menu = false;
                if let Some(p) = resp.interact_pointer_pos() {
                    let row = ((p.y - origin.y - HEADER_H) / ROW_H).floor();
                    if let (Some(Col::Layer(li)), false, true) = (Self::xsheet_col_at(&cols, origin.x, p.x), header.contains(p), row >= 0.0 && (row as usize) < len) {
                        self.aim_cell(li, row as usize);
                        self.xsheet_seen_frame = Some(row as usize);
                        self.xsheet_role_menu = true;
                    }
                }
            }
            if self.xsheet_role_menu {
                resp.context_menu(|ui| self.roles_menu(ui));
            }
        });
    }

    /// Colonne sous `x` (en points de l'écran).
    fn xsheet_col_at(cols: &[Col], origin_x: f32, x: f32) -> Option<Col> {
        let mut left = origin_x;
        for c in cols {
            if x >= left && x < left + c.width() {
                return Some(*c);
            }
            left += c.width();
        }
        None
    }

    fn click_xsheet(&mut self, p: egui::Pos2, origin: egui::Pos2, header: Rect, cols: &[Col], len: usize) {
        let col = Self::xsheet_col_at(cols, origin.x, p.x);
        if let Some(Col::Layer(li)) = col {
            self.select_layer(li);
        }
        if header.contains(p) {
            return;
        }
        let row = ((p.y - origin.y - HEADER_H) / ROW_H).floor();
        if row >= 0.0 && (row as usize) < len {
            self.goto_frame(row as usize);
            self.xsheet_seen_frame = Some(row as usize);
        }
    }

    fn paint_xsheet_header(&self, painter: &egui::Painter, header: Rect, cols: &[Col]) {
        painter.rect_filled(header, 0.0, theme::entete());
        let font = FontId::proportional(11.0);
        let mut x = header.left();
        for c in cols {
            let r = Rect::from_min_size(pos2(x, header.top()), vec2(c.width(), HEADER_H));
            let (title, strong) = match *c {
                Col::Image => ("Image".to_string(), false),
                Col::Action => ("Action".to_string(), false),
                Col::Dialogue => ("Dialogues / Notes".to_string(), false),
                Col::Audio => ("Son".to_string(), false),
                Col::Camera => ("Caméra".to_string(), false),
                Col::Layer(li) => {
                    let l = &self.layers()[li];
                    // Couleur du groupe en haut de la colonne, comme dans la timeline.
                    if let Some(g) = self.doc.groups.get(l.group as usize) {
                        let [r0, g0, b0] = g.color;
                        painter.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 3.0)), 0.0, Color32::from_rgb(r0, g0, b0));
                    }
                    if li == self.cur_layer {
                        painter.rect_filled(r.shrink2(vec2(1.0, 3.0)).translate(vec2(0.0, 1.0)), 2.0, theme::accent());
                    }
                    (l.name.clone(), li == self.cur_layer)
                }
            };
            let color = if strong { theme::texte_fort() } else if matches!(c, Col::Layer(li) if !self.layers()[*li].visible) { theme::texte_doux() } else { theme::texte() };
            let galley = painter.layout(title, font.clone(), color, r.width() - 6.0);
            painter.with_clip_rect(r.shrink(2.0)).galley(pos2(r.left() + 3.0, r.center().y - galley.size().y / 2.0), galley, color);
            painter.line_segment([pos2(r.right(), r.top()), pos2(r.right(), r.bottom())], Stroke::new(1.0_f32, theme::bord()));
            x += c.width();
        }
        painter.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0_f32, theme::graduation_seconde()));
    }

    fn paint_xsheet_rows(&self, painter: &egui::Painter, origin: egui::Pos2, cols: &[Col], first: usize, last: usize) {
        let s = &self.xsheet;
        let len = self.frame_count();
        let panel = self.panel();
        let every = s.separator_every(self.doc.fps);
        let cues = &panel.cues;
        let row_rect = |f: usize, x: f32, w: f32| Rect::from_min_size(pos2(x, origin.y + HEADER_H + f as f32 * ROW_H), vec2(w, ROW_H));
        let width: f32 = cols.iter().map(|c| c.width()).sum();
        let small = FontId::proportional(11.0);
        let mono = FontId::monospace(11.0);
        // Fonds : rangées alternées par seconde, hors entrée / sortie plus sombres, image courante.
        for f in first..last {
            let r = row_rect(f, origin.x, width);
            let outside = s.cues && (cues.in_point.is_some_and(|a| f < a) || cues.out_point.is_some_and(|b| f > b));
            let fill = if outside { theme::fond() } else if (f / every) % 2 == 0 { theme::rangee() } else { theme::rangee_sombre() };
            painter.rect_filled(r, 0.0, fill);
            if s.playhead && f == self.cur_frame {
                painter.rect_filled(r, 0.0, theme::image_courante().gamma_multiply(0.35));
            }
        }
        // Données par colonne.
        let cells: Vec<(Col, Vec<Cell>)> = cols
            .iter()
            .filter_map(|c| match *c {
                Col::Layer(li) => Some((*c, layer_cells(&self.layers()[li], len, s.behaviours))),
                _ => None,
            })
            .collect();
        let cam = if s.camera { camera_cells(panel, len) } else { Vec::new() };
        let levels = if s.audio { crate::xsheet::audio_levels(&self.doc, self.cur_panel) } else { Vec::new() };
        let story = if s.action || s.dialogue { story_rows(panel) } else { Vec::new() };
        let mut x = origin.x;
        for c in cols {
            let w = c.width();
            match *c {
                Col::Image => {
                    for f in first..last {
                        let r = row_rect(f, x, w);
                        let strong = f % every == 0;
                        let color = if f == self.cur_frame { theme::texte_fort() } else if strong { theme::texte() } else { theme::texte_doux() };
                        painter.text(pos2(r.right() - 4.0, r.center().y), Align2::RIGHT_CENTER, self.frame_label(f), mono.clone(), color);
                        if s.cues {
                            if cues.bookmarks.contains(&f) {
                                painter.text(pos2(r.left() + 2.0, r.center().y), Align2::LEFT_CENTER, ph::BOOKMARK_SIMPLE, small.clone(), theme::camera());
                            }
                            for (at, mark) in [(cues.in_point, "["), (cues.out_point, "]")] {
                                if at == Some(f) {
                                    painter.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, mark, mono.clone(), theme::accent_texte());
                                }
                            }
                        }
                    }
                }
                Col::Action | Col::Dialogue => {
                    // Chaque texte occupe les lignes de son image de storyboard.
                    for (k, row) in story.iter().enumerate() {
                        let end = story.get(k + 1).map_or(len, |n| n.frame).max(row.frame + 1);
                        if end <= first || row.frame >= last {
                            continue;
                        }
                        let text = if *c == Col::Action {
                            row.action.clone()
                        } else {
                            [row.dialogue.as_str(), row.notes.as_str()].iter().filter(|t| !t.trim().is_empty()).copied().collect::<Vec<_>>().join("\n")
                        };
                        let r = Rect::from_min_max(row_rect(row.frame, x, w).min, row_rect(end - 1, x, w).max).shrink(3.0);
                        painter.line_segment([pos2(x, r.top() - 3.0), pos2(x + w, r.top() - 3.0)], Stroke::new(1.0_f32, theme::marque()));
                        if !text.trim().is_empty() {
                            let galley = painter.layout(text, small.clone(), theme::texte(), r.width());
                            painter.with_clip_rect(r).galley(r.min, galley, theme::texte());
                        }
                    }
                }
                Col::Audio => {
                    for f in first..last {
                        let r = row_rect(f, x, w);
                        let a = levels.get(f).copied().unwrap_or(0.0);
                        if a > 0.004 {
                            let half = (r.width() / 2.0 - 3.0) * a;
                            painter.rect_filled(Rect::from_center_size(r.center(), vec2((half * 2.0).max(1.0), r.height() - 4.0)), 1.0, theme::son_onde());
                        }
                    }
                }
                Col::Camera => {
                    for f in first..last {
                        let r = row_rect(f, x, w);
                        match cam.get(f) {
                            Some(CamCell::Key) => {
                                painter.text(r.center(), Align2::CENTER_CENTER, "◆", small.clone(), theme::camera());
                            }
                            Some(CamCell::Between) => {
                                painter.line_segment([pos2(r.center().x, r.top()), pos2(r.center().x, r.bottom())], Stroke::new(1.5_f32, theme::camera()));
                            }
                            _ => {}
                        }
                    }
                }
                Col::Layer(li) => {
                    let Some((_, list)) = cells.iter().find(|(cc, _)| *cc == *c) else { continue };
                    let kind_color = match self.layers()[li].kind {
                        crate::document::LayerKind::Vector => theme::dessin_vectoriel(),
                        crate::document::LayerKind::Bitmap => theme::dessin_bitmap(),
                    };
                    for f in first..last {
                        let r = row_rect(f, x, w);
                        match &list[f] {
                            Cell::Start { label, mark, .. } => {
                                if let Some(k) = mark {
                                    painter.rect_filled(Rect::from_min_size(pos2(r.left() + 2.0, r.top() + 3.0), vec2(4.0, r.height() - 6.0)), 1.0, crate::marks::color(*k));
                                }
                                let color = if li == self.cur_layer { theme::texte_fort() } else { theme::texte() };
                                let galley = painter.layout_no_wrap(label.clone(), mono.clone(), color);
                                painter.with_clip_rect(r.shrink(1.0)).galley(pos2(r.center().x - galley.size().x / 2.0, r.center().y - galley.size().y / 2.0), galley, color);
                            }
                            Cell::Hold => {
                                painter.line_segment([pos2(r.center().x, r.top()), pos2(r.center().x, r.bottom())], Stroke::new(2.0_f32, kind_color));
                            }
                            Cell::EmptyStart => {
                                painter.text(r.center(), Align2::CENTER_CENTER, "×", mono.clone(), theme::dessin_vide());
                            }
                            Cell::Behaviour { label } => {
                                painter.text(r.center(), Align2::CENTER_CENTER, format!("({label})"), mono.clone(), theme::texte_doux());
                            }
                            Cell::Blank => {}
                        }
                        // Rôle de l'image (13g-2), à droite de la case.
                        if let Some(&role) = self.layers()[li].roles.get(&f) {
                            crate::roles::paint_symbol(painter, pos2(r.right() - 7.0, r.center().y), 8.0, role, 1.0);
                        }
                    }
                }
            }
            // Trait de colonne.
            let top = origin.y + HEADER_H + first as f32 * ROW_H;
            let bottom = origin.y + HEADER_H + last as f32 * ROW_H;
            painter.line_segment([pos2(x + w, top), pos2(x + w, bottom)], Stroke::new(1.0_f32, theme::bord()));
            x += w;
        }
        // Traits entre les images ; épais toutes les `every` images.
        for f in first..=last {
            let y = origin.y + HEADER_H + f as f32 * ROW_H;
            let (stroke_w, color) = if f % every == 0 { (1.5_f32, theme::graduation_seconde()) } else { (1.0_f32, theme::graduation()) };
            painter.line_segment([pos2(origin.x, y), pos2(origin.x + width, y)], Stroke::new(stroke_w, color));
        }
    }

    /// Ce que montre la feuille (« Modifier infos affichées »).
    fn xsheet_display_menu(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.xsheet;
        ui.label(RichText::new("Général").strong());
        ui.checkbox(&mut s.playhead, "Image courante surlignée");
        ui.checkbox(&mut s.cues, "Points d'entrée et de sortie, signets");
        ui.horizontal(|ui| {
            ui.label("Trait épais toutes les");
            ui.add(egui::DragValue::new(&mut s.separator).range(0..=1000).custom_formatter(|v, _| if v == 0.0 { "secondes".into() } else { format!("{v:.0} images") }));
        });
        ui.separator();
        ui.label(RichText::new("Colonnes").strong());
        ui.checkbox(&mut s.action, "Action");
        ui.checkbox(&mut s.dialogue, "Dialogues / Notes");
        ui.checkbox(&mut s.audio, "Son");
        ui.checkbox(&mut s.layers, "Calques");
        ui.add_enabled(s.layers, egui::Checkbox::new(&mut s.hidden_layers, "Calques cachés aussi"));
        ui.add_enabled(s.layers, egui::Checkbox::new(&mut s.behaviours, "Pré- et post-comportements"));
        ui.checkbox(&mut s.camera, "Caméra");
        ui.separator();
        if ui.button("Valeurs par défaut").clicked() {
            *s = crate::xsheet::XSheetSettings::default();
        }
    }
}
