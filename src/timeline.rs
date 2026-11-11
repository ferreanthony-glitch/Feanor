use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::handles::{GridMenu, GridSelect, Handle};
use crate::layerheads::{row_at, rows_height, rows_of, Row, HEAD_W};
use crate::storyboard::duration_label;
use crate::theme;

/// Largeur d'une image dans la grille : par défaut, et bornes du zoom de la timeline (13d).
pub(crate) const CELL_W: f32 = 16.0;
pub(crate) const CELL_MIN: f32 = 3.0;
pub(crate) const CELL_MAX: f32 = 64.0;
const ROW_H: f32 = crate::layerheads::ROW_SMALL;
const HEADER_H: f32 = 20.0;
/// Largeur de l'en-tête des calques (13b).
const NAMES_W: f32 = HEAD_W;
/// Ligne du son sous les calques (forme d'onde du montage pendant le panneau).
const SOUND_H: f32 = 30.0;
/// Ligne des images clés de la caméra, sous les calques.
const CAM_H: f32 = 18.0;
/// Couleur des clés de caméra (celle du thème).
fn cam_key() -> Color32 {
    theme::camera()
}
/// Barre d'un calque, sous ses dessins (chantier 1) : on la prend pour décaler le calque, par
/// ses bouts pour le rogner ou l'allonger.
const BAR_H: f32 = 4.0;
/// Prise de la fin du plan, au bout de la règle (chantier 4).
const PLAN_END_W: f32 = 9.0;

// La prise de la fin du plan que la timeline a peinte à sa dernière image (pour les essais, `duree_cles_tests.rs`).
#[cfg(test)]
thread_local! {
    pub(crate) static PRISE_FIN_DU_PLAN: std::cell::Cell<Option<Rect>> = const { std::cell::Cell::new(None) };
}

/// Bas des dessins d'une ligne qui commence en `y` (la barre du calque est dessous).
fn body_bottom(y: f32, h: f32) -> f32 {
    y + h - BAR_H - 3.0
}

/// Colonnes [début, fin] de la grille qu'il faut peindre : celles que montre la zone `clip_left` à
/// `clip_right` (la grille commence en `origin`, `cell` de large par colonne), une de marge de
/// chaque côté, sans dépasser les `cols` colonnes de la grille.
pub(crate) fn visible_columns(clip_left: f32, clip_right: f32, origin: f32, cell: f32, cols: usize) -> (usize, usize) {
    let first = (((clip_left - origin) / cell).floor().max(0.0) as usize).saturating_sub(1);
    let last = (((clip_right - origin) / cell).ceil().max(0.0) as usize + 1).min(cols);
    (first.min(last), last)
}

/// Couleur de l'image courante (celle du thème).
pub(crate) fn cur_frame() -> Color32 {
    theme::image_courante()
}

impl App {
    /// Hauteur de la ligne du son (rien s'il n'y a aucun son dans le projet).
    fn sound_row_h(&self) -> f32 {
        if self.doc.audio_tracks.iter().any(|t| !t.clips.is_empty()) {
            SOUND_H
        } else {
            0.0
        }
    }

    /// Hauteur de la rangée des notes dessinées (13e) : seulement quand l'onglet Notes est ouvert.
    fn notes_row_h(&self) -> f32 {
        if self.bottom_tab == Some(crate::bottomtabs::BottomTab::Notes) {
            crate::notes::NOTES_H
        } else {
            0.0
        }
    }

    pub(crate) fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        if self.bande_film_vide(ui) {
            return;
        }
        ui.add_space(2.0);
        self.timeline_toolbar(ui);
        ui.separator();
        // Onglets du bas (13e) tout en bas, la grille dans la place qui reste.
        egui::TopBottomPanel::bottom("onglets_timeline")
            .resizable(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 2)))
            .show_inside(ui, |ui| self.bottom_tabs(ui));
        // Trop basse pour tous les calques : elle défile, et la barre se voit sans le survol (`montagehauteur.rs`).
        ui.scope(|ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::vertical()
                .id_salt("timeline_v")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        self.timeline_names(ui);
                        ui.add_space(2.0);
                        egui::ScrollArea::horizontal()
                            .id_salt("timeline_h")
                            .auto_shrink([false, true])
                            .show(ui, |ui| self.timeline_grid(ui));
                    });
                });
        });
    }

    /// Barre de la timeline, sur une seule ligne qui ne défile pas (`barres.rs`) : le plan, le
    /// transport, les dessins, la caméra et la pelure ; le reste sous « Plus… ». Les commandes en
    /// icônes ont leur nom, leur raccourci et une description dans une bulle (12d).
    fn timeline_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.barre_ui(ui, crate::barres::Barre::Timeline);
            // Au bout de la ligne : la légende des clés et son filtre (`legendecles.rs`).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.legende_cles_ui(ui));
        });
    }

    /// Le titre du plan que l'on anime et sa durée (affichage fixe de la barre).
    pub(crate) fn timeline_plan_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        let (count, fps) = (self.frame_count(), self.doc.fps);
        let resp = ui.label(RichText::new(crate::storyboard::panel_title(&self.doc, self.cur_panel)).strong());
        bulle(resp, &crate::storyboard::panel_long_title(&self.doc, self.cur_panel), "Le plan que l'on anime.", "");
        let resp = self.panel_length_field(ui, "");
        bulle(resp, "Durée du plan", &format!("{} ; glisser ou taper pour la changer.", duration_label(count, fps)), "");
    }

    /// L'image courante (13d), où l'on tape un numéro ou un time code pour y aller, et la dernière
    /// image du plan (affichage fixe de la barre).
    pub(crate) fn timeline_image_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        let count = self.frame_count();
        let (start, fps, how) = (self.doc.start_frame, self.doc.fps, self.time_display);
        let mut f = self.cur_frame as f64;
        let resp = ui.add(
            egui::DragValue::new(&mut f)
                .range(0.0..=(count.max(1) - 1) as f64)
                .speed(0.2)
                .custom_formatter(move |v, _| crate::cues::frame_label(v.max(0.0) as usize, start, fps, how))
                .custom_parser(move |s| crate::cues::parse_frame(s, start, fps, how).map(|f| f as f64)),
        );
        let resp = bulle(resp, "Image courante", "Glisser, ou taper un numéro (ou un time code) pour y aller.", "");
        if resp.changed() {
            self.goto_frame(f.max(0.0) as usize);
        }
        let resp = ui.label(RichText::new(format!("/ {}", self.frame_label(count.max(1) - 1))).monospace());
        bulle(resp, "Dernière image du plan", "", "");
    }
    /// En-tête des calques (13b, `layerheads.rs`), puis les noms des lignes caméra et son.
    fn timeline_names(&mut self, ui: &mut egui::Ui) {
        let rows_h = rows_height(&rows_of(self.layers(), &self.doc.groups));
        let sound_h = self.sound_row_h();
        let notes_h = self.notes_row_h();
        let r = self.timeline_heads(ui, HEADER_H, CAM_H + notes_h + sound_h);
        let painter = ui.painter_at(r);
        let visuals = ui.visuals().clone();
        // Clic droit sur le nom d'une ligne : son menu (passe interface).
        let menu_on = |app: &mut App, ui: &mut egui::Ui, row: Rect, salt: &str, m: GridMenu| {
            let resp = ui.interact(row, ui.id().with(("nom_ligne", salt)), Sense::click());
            resp.context_menu(|ui| app.grid_menu_ui(ui, m));
        };
        if notes_h > 0.0 {
            let row = Rect::from_min_size(pos2(r.left(), r.top() + HEADER_H + rows_h + CAM_H), vec2(NAMES_W, notes_h)).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 2.0, theme::rangee_sombre());
            painter.text(pos2(r.left() + 8.0, row.center().y), Align2::LEFT_CENTER, "Notes (jamais exportées)", FontId::proportional(11.0), visuals.weak_text_color());
            menu_on(self, ui, row, "notes", GridMenu::NotesHeader);
        }
        {
            let y = r.top() + HEADER_H + rows_h;
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(NAMES_W, CAM_H)).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 2.0, theme::rangee_sombre());
            let n = self.panel().camera.len();
            let label = match n {
                0 => "Caméra".to_string(),
                1 => "Caméra (fixe)".to_string(),
                _ => format!("Caméra ({n} clés)"),
            };
            painter.text(
                pos2(r.left() + 8.0, row.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(11.0),
                visuals.weak_text_color(),
            );
            menu_on(self, ui, row, "camera", GridMenu::Camera);
        }
        if sound_h > 0.0 {
            let row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 2.0, theme::rangee_sombre());
            painter.text(
                pos2(r.left() + 8.0, row.center().y),
                Align2::LEFT_CENTER,
                "Son (montage)",
                FontId::proportional(12.0),
                visuals.weak_text_color(),
            );
            menu_on(self, ui, row, "son", GridMenu::SoundHeader);
        }
    }

    fn timeline_grid(&mut self, ui: &mut egui::Ui) {
        // Largeur d'une image : le zoom de la timeline (13d).
        // « Adapter » (13d) : les images demandées tiennent dans la largeur visible.
        let view_w = ui.clip_rect().width().max(50.0);
        let fit = self.timeline_fit.take();
        if let Some((a, b)) = fit {
            self.timeline_cell = (view_w / (b.saturating_sub(a)).max(1) as f32).clamp(CELL_MIN, CELL_MAX);
        }
        let cell = self.timeline_cell.clamp(CELL_MIN, CELL_MAX);
        let count = self.frame_count();
        // Lignes des calques, de hauteurs différentes (réduites ou complètes, 13b).
        let rows = rows_of(self.layers(), &self.doc.groups);
        let rows_h = rows_height(&rows);
        let visible_cols = (ui.available_width() / cell) as usize + 1;
        let cols = (count + 48).max(self.cur_frame + 24).max(visible_cols);
        let sound_h = self.sound_row_h();
        let notes_h = self.notes_row_h();
        let size = vec2(cols as f32 * cell, HEADER_H + rows_h + CAM_H + notes_h + sound_h);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let r = resp.rect;
        if let Some((a, b)) = fit {
            let target = Rect::from_min_max(pos2(r.left() + a as f32 * cell, r.top()), pos2(r.left() + b as f32 * cell, r.top() + HEADER_H));
            ui.scroll_to_rect(target, Some(egui::Align::Min));
        }
        // Un zoom a eu lieu à l'image d'écran d'avant : la grille défile pour que la tête de lecture
        // reste en place (`zoombande.rs`).
        let head = self.cur_frame as f32 + 0.5;
        self.follow_strip_anchor(ui, crate::zoombande::Strip::Timeline, r.left(), cell, head);
        // Ctrl + molette sur la grille : zoom de la timeline, autour de la tête de lecture.
        if resp.hovered() {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                self.anchor_strip_zoom(crate::zoombande::Strip::Timeline);
                self.timeline_cell = (self.timeline_cell * zoom).clamp(CELL_MIN, CELL_MAX);
            }
        }
        let visuals = ui.visuals().clone();
        let grid_top = r.top() + HEADER_H;
        // Colonnes visibles [c0, c1] : le reste du plan n'est ni parcouru ni peint (à 2 000 images et
        // 15 calques, la grille entière coûtait 16 ms par image d'écran).
        let clip = painter.clip_rect();
        let (c0, c1) = visible_columns(clip.left(), clip.right(), r.left(), cell, cols);
        // Poignée sous la souris ou tenue : sa barre ou sa prise s'éclaire.
        let lit = self
            .handle_drag
            .as_ref()
            .map(|d| d.handle)
            .or_else(|| resp.hover_pos().and_then(|p| self.timeline_handle_at(p, r, cell, &rows, grid_top)));
        // Règle des images.
        painter.rect_filled(Rect::from_min_size(r.min, vec2(size.x, HEADER_H)), 0.0, theme::regle());

        // Images clés de la caméra : losanges (ronds si le mouvement y est adouci), reliés par un
        // trait là où la caméra bouge.
        {
            let y = grid_top + rows_h;
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(size.x, CAM_H)).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 0.0, theme::rangee_sombre());
            // (Cachées par le filtre de la légende, `legendecles.rs` : la rangée reste, sans losanges.)
            let keys: &[crate::camera::CameraKey] = if self.cle_vue(crate::legendecles::Sorte::Camera) { &self.panel().camera } else { &[] };
            let cx = |f: usize| r.left() + (f as f32 + 0.5) * cell;
            if keys.len() >= 2 {
                painter.line_segment(
                    [pos2(cx(keys[0].frame), row.center().y), pos2(cx(keys[keys.len() - 1].frame), row.center().y)],
                    Stroke::new(2.0_f32, cam_key().gamma_multiply(0.45)),
                );
            }
            // Les poignées de lissage (lissage.rs) : petits ronds creux, sous les losanges.
            let ease_hot = match self.ease_drag.as_ref() {
                Some(d) => keys.iter().position(|k| k.frame == d.frame).map(|i| (i, d.cote)),
                None => resp.hover_pos().filter(|p| row.contains(*p)).and_then(|p| crate::lissage::sous_pointeur(keys, p.x, &cx, cell, crate::lissage::tol_cle(cell))),
            };
            crate::lissage::peindre(&painter, keys, row.center().y, &cx, cell, cam_key(), theme::rangee_sombre(), ease_hot);
            for k in keys.iter() {
                let c = pos2(cx(k.frame), row.center().y);
                if c.x < painter.clip_rect().left() - 8.0 || c.x > painter.clip_rect().right() + 8.0 {
                    continue;
                }
                // Une clé choisie (ou que l'on fait glisser) : plus grande, cerclée du contour du thème.
                let chosen = self.cam_key_chosen(self.cur_panel, k.frame);
                let plain = Stroke::new(1.0_f32, Color32::from_black_alpha(140));
                self.paint_cam_diamond(&painter, c, chosen, cam_key(), 5.0, plain);
            }
        }

        // Le son du montage pendant ce panneau (toutes pistes superposées), en lecture seule.
        if sound_h > 0.0 {
            let row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 0.0, theme::rangee_sombre());
            let start = self.doc.panel_start(self.cur_panel);
            let left = r.left() - start as f32 * cell;
            let fps = self.doc.fps;
            // Un bruitage glissé depuis le panneau Bruitages se pose ici (`bruitagesui.rs`).
            self.bruitage_drop_row(ui, row, left, cell);
            for track in &self.doc.audio_tracks {
                // (Muette, ou une autre piste en solo : estompée.)
                let color = if !crate::audiofx::audible(&self.doc, track) {
                    theme::texte_doux()
                } else {
                    theme::son_onde()
                };
                for clip in &track.clips {
                    let cr = Rect::from_min_max(
                        pos2(left + clip.start as f32 * cell, row.top() + 2.0),
                        pos2(left + clip.end() as f32 * cell, row.bottom() - 2.0),
                    )
                    .intersect(row);
                    if !cr.is_positive() {
                        continue;
                    }
                    if let Some(sound) = self.doc.sounds.get(&clip.sound) {
                        crate::soundtrack::paint_wave(
                            &painter.with_clip_rect(cr),
                            Rect::from_min_max(pos2(cr.left(), row.top() + 2.0), pos2(cr.right(), row.bottom() - 2.0)),
                            &sound.pcm,
                            clip,
                            left,
                            cell,
                            fps,
                            color,
                        );
                    }
                }
            }
        }

        // Fond des lignes, calque courant en surbrillance.
        for row in &rows {
            let li = row.li;
            let rect = Rect::from_min_size(pos2(r.left(), grid_top + row.top), vec2(size.x, row.h)).shrink2(vec2(0.0, 1.0));
            let bg = if li == self.cur_layer {
                theme::surface_active(crate::theme::accent(), 0.35)
            } else if self.is_chosen_layer(li) {
                theme::surface_active(crate::theme::accent(), 0.18)
            } else if self.layers()[li].folder {
                // Dossier (13b-5) : pas de dessin, une rangée plus sombre.
                theme::rangee_sombre()
            } else {
                theme::rangee()
            };
            painter.rect_filled(rect, 0.0, bg);
        }

        // Graduations : une seconde = trait plus marqué.
        let fps = self.doc.fps.max(1) as usize;
        for c in c0..=c1 {
            let x = r.left() + c as f32 * cell;
            let color = if c % fps == 0 {
                theme::graduation_seconde()
            } else {
                theme::graduation()
            };
            painter.line_segment([pos2(x, grid_top), pos2(x, r.bottom())], Stroke::new(1.0_f32, color));
            // Numéros (13d) : assez espacés pour se lire (toutes les 5 images au zoom normal), ou
            // chaque seconde (ou plus) en time code.
            let time_code = self.time_display == crate::cues::TimeDisplay::TimeCode;
            let step = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000].into_iter().find(|s| *s as f32 * cell >= 70.0).unwrap_or(2000);
            let seconds = [1, 2, 5, 10, 30, 60].into_iter().find(|s| (*s * fps) as f32 * cell >= 80.0).unwrap_or(120);
            let shown = if time_code { c % (fps * seconds) == 0 } else { c == 0 || (c + 1) % step == 0 };
            if c < cols && shown {
                painter.text(
                    pos2(x + if time_code { 2.0 } else { cell * 0.5 }, r.top() + HEADER_H * 0.5),
                    if time_code { Align2::LEFT_CENTER } else { Align2::CENTER_CENTER },
                    self.frame_label(c),
                    FontId::monospace(9.0),
                    visuals.weak_text_color(),
                );
            }
        }

        // Points d'entrée et de sortie (13d) : crochets dans la règle, le reste du plan assombri.
        let cues = self.panel().cues.clone();
        if cues.in_point.is_some() || cues.out_point.is_some() {
            let (a, b) = self.plan_play_range();
            let shade = Color32::from_black_alpha(80);
            painter.rect_filled(Rect::from_min_max(pos2(r.left(), grid_top), pos2(r.left() + a as f32 * cell, r.bottom())), 0.0, shade);
            painter.rect_filled(Rect::from_min_max(pos2(r.left() + b as f32 * cell, grid_top), r.max), 0.0, shade);
            let bracket = Stroke::new(2.0_f32, theme::image_courante());
            if let Some(f) = cues.in_point {
                let x = r.left() + f as f32 * cell + 1.0;
                painter.line_segment([pos2(x, r.top() + 2.0), pos2(x, r.top() + HEADER_H - 2.0)], bracket);
                painter.line_segment([pos2(x, r.top() + 2.0), pos2(x + 5.0, r.top() + 2.0)], bracket);
                painter.line_segment([pos2(x, r.top() + HEADER_H - 2.0), pos2(x + 5.0, r.top() + HEADER_H - 2.0)], bracket);
            }
            if let Some(f) = cues.out_point {
                let x = r.left() + (f + 1) as f32 * cell - 1.0;
                painter.line_segment([pos2(x, r.top() + 2.0), pos2(x, r.top() + HEADER_H - 2.0)], bracket);
                painter.line_segment([pos2(x, r.top() + 2.0), pos2(x - 5.0, r.top() + 2.0)], bracket);
                painter.line_segment([pos2(x, r.top() + HEADER_H - 2.0), pos2(x - 5.0, r.top() + HEADER_H - 2.0)], bracket);
            }
        }
        // Signets (13d) : un petit fanion bleu en bas de la règle.
        let bookmark = theme::accent_texte();
        for &f in &cues.bookmarks {
            let x = r.left() + (f as f32 + 0.5) * cell;
            painter.add(egui::Shape::convex_polygon(
                vec![pos2(x - 4.0, r.top() + HEADER_H - 1.0), pos2(x + 4.0, r.top() + HEADER_H - 1.0), pos2(x, r.top() + HEADER_H - 7.0)],
                bookmark,
                Stroke::NONE,
            ));
            painter.line_segment([pos2(x, grid_top), pos2(x, r.bottom())], Stroke::new(1.0_f32, bookmark.gamma_multiply(0.5)));
        }

        // Marques d'image (13a) : un triangle dans la règle et un trait au début de l'image.
        for m in &self.panel().marks {
            let x = r.left() + m.frame as f32 * cell;
            painter.line_segment([pos2(x, grid_top), pos2(x, r.bottom())], Stroke::new(1.5_f32, theme::marque()));
            painter.add(egui::Shape::convex_polygon(
                vec![pos2(x, r.top() + 2.0), pos2(x + 7.0, r.top() + 2.0), pos2(x, r.top() + HEADER_H - 2.0)],
                theme::marque(),
                Stroke::NONE,
            ));
        }

        // Dessins : un bloc par exposition, pastille au début du dessin ; miniature du dessin
        // dans les lignes complètes (13d), posée après la boucle (elle a besoin de `self`).
        let mut thumbs: Vec<(Rect, crate::document::CelId, bool)> = Vec::new();
        for row in &rows {
            let layer = &self.layers()[row.li];
            let y = grid_top + row.top;
            // Calque vidéo (13k) : une bande de film de sa durée, avec son nom.
            if let Some(v) = &layer.video {
                let (a, b) = (v.first_frame(), v.end(self.doc.fps).min(cols));
                if a < b {
                    let block = Rect::from_min_max(pos2(r.left() + a as f32 * cell + 1.0, y + 4.0), pos2(r.left() + b as f32 * cell - 1.0, y + row.h - 4.0));
                    let fill = if layer.visible { theme::son_extrait() } else { theme::son_extrait().gamma_multiply(0.5) };
                    painter.rect_filled(block, 3.0, fill);
                    // Perforations, comme une pellicule.
                    let mut x = block.left() + 4.0;
                    while x < block.right() - 4.0 {
                        for yy in [block.top() + 2.0, block.bottom() - 4.0] {
                            painter.rect_filled(Rect::from_min_size(pos2(x, yy), vec2(3.0, 2.0)), 0.5, theme::sur_calque().gamma_multiply(0.6));
                        }
                        x += 8.0;
                    }
                    painter.with_clip_rect(block.shrink(1.0)).text(
                        pos2(block.left() + 6.0, block.center().y),
                        Align2::LEFT_CENTER,
                        format!("{}  {}", egui_phosphor::regular::FILM_STRIP, v.name),
                        FontId::proportional(10.0),
                        theme::sur_calque(),
                    );
                }
                continue;
            }
            let content = layer.frames.iter().position(Option::is_some).zip(layer.frames.iter().rposition(Option::is_some));
            // Avant la première image et après la dernière (pré / post-comportement, 13c) : les
            // images montrées, en blocs pâles bordés.
            let first = layer.frames.iter().position(Option::is_some);
            let last = layer.frames.iter().rposition(Option::is_some);
            if let (Some(first), Some(last)) = (first, last) {
                let base = theme::couleur_du_calque(layer);
                // (Les parties visibles seulement.)
                let ranges = [(c0, first.min(c1 + 1)), ((last + 1).max(c0), cols.min(c1 + 1))];
                for (a, b) in ranges {
                    let mut f = a;
                    while f < b {
                        let shown = layer.shown_at(f);
                        let mut g = f + 1;
                        while g < b && layer.shown_at(g) == shown {
                            g += 1;
                        }
                        if shown.is_some() {
                            let block = Rect::from_min_max(
                                pos2(r.left() + f as f32 * cell + 1.0, y + 5.0),
                                pos2(r.left() + g as f32 * cell - 1.0, body_bottom(y, row.h) - 2.0),
                            );
                            painter.rect(block, 3.0, base.gamma_multiply(0.18), Stroke::new(1.0_f32, base.gamma_multiply(0.7)), StrokeKind::Inside);
                        }
                        f = g;
                    }
                }
            }
            // Depuis l'exposition qui couvre la première colonne visible, jusqu'à la dernière.
            let mut f = if c0 < layer.frames.len() { layer.run_bounds(c0).0 } else { layer.frames.len() };
            // Le rang de la clé depuis le début du plan (pas depuis la première colonne vue) : l'alternance des couleurs ne bouge pas quand on défile.
            let mut rang = crate::cles_alternees::rang_de(layer, f);
            while f < layer.frames.len().min(c1 + 1) {
                let (start, end) = layer.run_bounds(f);
                // Instance vide entre deux dessins (13d) : une croix, si on le veut.
                let inside = content.is_some_and(|(a, b)| start > a && end <= b);
                if layer.frames[start].is_none() && self.empty_x && inside {
                    let cross = Rect::from_min_max(
                        pos2(r.left() + start as f32 * cell + 2.0, y + 5.0),
                        pos2(r.left() + end as f32 * cell - 2.0, body_bottom(y, row.h) - 2.0),
                    );
                    let stroke = Stroke::new(1.0_f32, theme::texte_doux().gamma_multiply(0.6));
                    painter.rect_stroke(cross, 2.0, stroke, StrokeKind::Inside);
                    painter.line_segment([cross.left_top(), cross.right_bottom()], stroke);
                    painter.line_segment([cross.left_bottom(), cross.right_top()], stroke);
                }
                if let Some(id) = layer.frames[start] {
                    let block = Rect::from_min_max(
                        pos2(r.left() + start as f32 * cell + 1.0, y + 3.0),
                        pos2(r.left() + end as f32 * cell - 1.0, body_bottom(y, row.h)),
                    );
                    // (Un dessin de couleur sans peinture mais avec des griffonnages n'est pas vide : il remplit des zones.)
                    let empty = self.doc.cels.get(&id).is_none_or(|c| c.is_empty())
                        && layer.couleur.as_ref().and_then(|c| c.griffonnages.get(&id)).and_then(|g| self.doc.cels.get(g)).is_none_or(|c| c.is_empty());
                    // (Un calque d'effet n'a que des dessins vides : ses images sont cyan.)
                    let mut fill = if empty && layer.effet.is_none() { theme::dessin_vide() } else { theme::couleur_du_calque(layer) };
                    // Une clé sur deux un peu plus claire : on voit où chacune commence et s'arrête (`cles_alternees.rs`).
                    fill = crate::cles_alternees::alternee(fill, rang);
                    rang += 1;
                    if !layer.visible {
                        fill = fill.gamma_multiply(0.5);
                    }
                    painter.rect_filled(block, 3.0, fill);
                    painter.circle_filled(
                        pos2(block.left() + cell * 0.5 - 1.0, block.center().y),
                        2.5,
                        theme::encre(),
                    );
                    // Marque de couleur (13d) : un trait de sa couleur en haut du bloc.
                    if let Some(&k) = layer.color_marks.get(&id) {
                        let strip = Rect::from_min_size(block.min, vec2(block.width(), 4.0));
                        painter.rect_filled(strip, 2.0, crate::marks::color(k));
                    }
                    // Miniature (13d) : dans une ligne complète, si le bloc est assez large.
                    let mut text_x = block.left() + cell - 1.0;
                    // (Seulement les blocs que l'on voit : une miniature par bloc du plan entier, c'était
                    // des centaines de dessins échantillonnés à chaque image d'écran.)
                    if self.timeline_thumbs && row.h >= 40.0 && block.width() >= cell + 26.0 && block.intersects(painter.clip_rect()) {
                        let h = block.height() - 6.0;
                        let w = (h * 16.0 / 9.0).min(block.width() - cell - 4.0);
                        let at = Rect::from_min_size(pos2(block.left() + cell, block.top() + 3.0), vec2(w, h));
                        thumbs.push((at, id, layer.visible));
                        text_x = at.right() + 3.0;
                    }
                    // Nom d'instance (13c) dans le bloc, sinon le trait de l'exposition.
                    match layer.names.get(&id) {
                        Some(name) => {
                            painter.with_clip_rect(block.shrink(1.0)).text(
                                pos2(text_x, block.center().y),
                                Align2::LEFT_CENTER,
                                name,
                                FontId::proportional(10.0),
                                theme::encre(),
                            );
                        }
                        None if end - start > 1 && text_x < block.right() - 6.0 => {
                            painter.line_segment(
                                [pos2(text_x, block.center().y), pos2(block.right() - 4.0, block.center().y)],
                                Stroke::new(1.0_f32, theme::encre()),
                            );
                        }
                        _ => {}
                    }
                }
                f = end;
            }
        }
        // Barre de chaque calque (chantier 1) : de son premier dessin à la fin du dernier ; ses
        // bouts sont plus clairs. Elle s'éclaire sous la souris et pendant le geste.
        for row in &rows {
            let layer = &self.layers()[row.li];
            if layer.folder || layer.video.is_some() {
                continue;
            }
            let Some((a, b)) = crate::handles::content_bounds(layer) else { continue };
            let y = grid_top + row.top;
            let bar = Rect::from_min_max(
                pos2(r.left() + a as f32 * cell + 1.0, body_bottom(y, row.h) + 1.0),
                pos2(r.left() + b.min(cols) as f32 * cell - 1.0, body_bottom(y, row.h) + 1.0 + BAR_H),
            );
            if !bar.is_positive() {
                continue;
            }
            let base = theme::couleur_du_calque(layer);
            let on = |h: Option<Handle>| match h {
                Some(Handle::LayerMove { layer, .. }) => layer == row.li,
                _ => false,
            };
            let (start_lit, end_lit) = (lit == Some(Handle::LayerStart { layer: row.li }), lit == Some(Handle::LayerEnd { layer: row.li }));
            let fill = if on(lit) { base } else { base.gamma_multiply(0.5) };
            painter.rect_filled(bar, 1.5, fill);
            let cap = (bar.width() * 0.5).min(4.0);
            for (x0, x1, hot) in [(bar.left(), bar.left() + cap, start_lit), (bar.right() - cap, bar.right(), end_lit)] {
                let c = if hot { theme::contour() } else { theme::texte_fort().gamma_multiply(0.7) };
                painter.rect_filled(Rect::from_min_max(pos2(x0, bar.top()), pos2(x1, bar.bottom())), 1.0, c);
            }
            // Un bout pris : un trait sur toute la hauteur de la ligne, là où il est.
            for (hot, x) in [(start_lit, bar.left() - 1.0), (end_lit, bar.right() + 1.0)] {
                if hot {
                    painter.line_segment([pos2(x, y + 1.0), pos2(x, y + row.h - 1.0)], Stroke::new(1.5_f32, theme::contour()));
                }
            }
        }
        for (at, id, visible) in thumbs {
            if let Some(t) = self.layer_thumb(ui.ctx(), id) {
                let tint = if visible { Color32::WHITE } else { Color32::from_gray(120) };
                painter.image(t.id(), at, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                painter.rect_stroke(at, 0.0, Stroke::new(1.0_f32, Color32::from_black_alpha(90)), StrokeKind::Inside);
            }
        }

        // Rôles des images (13g-2) : un symbole en bas de la case, sur une pastille sombre.
        let view = self.roles_view;
        if view.shown {
            let s = (cell - 3.0).clamp(4.0, 9.0);
            for row in &rows {
                if view.current_only && row.li != self.cur_layer {
                    continue;
                }
                for (&f, &role) in self.layers()[row.li].roles.range(0..cols) {
                    let c = pos2(r.left() + (f as f32 + 0.5) * cell, grid_top + row.top + row.h - s * 0.5 - 3.0);
                    painter.circle_filled(c, s * 0.5 + 1.5, theme::encre().gamma_multiply(view.opacity));
                    crate::roles::paint_symbol(&painter, c, s, role, view.opacity);
                }
            }
        }

        // Clés d'effets (13i) : un losange violet en haut de la ligne du calque, cerclé de blanc
        // quand il est choisi ; seulement les clés de l'effet choisi dans le panneau Effets, sur
        // son calque (`fxkeys.rs`).
        if self.fx_keys_shown {
            let panel = self.panel().id;
            let visible = (ui.clip_rect().left(), ui.clip_rect().right());
            for row in &rows {
                let layer = &self.layers()[row.li];
                self.paint_fx_markers(&painter, panel, layer, |f| r.left() + (f as f32 + 0.5) * cell, grid_top + row.top + 8.0, cols, visible);
            }
        }
        // Clés de position (`poskeysui.rs`) : des losanges verts au milieu de la ligne du calque qu'elles
        // animent (ronds quand la clé est adoucie).
        {
            let visible = (ui.clip_rect().left(), ui.clip_rect().right());
            for row in &rows {
                let layer = &self.layers()[row.li];
                if !layer.pos_keys.is_empty() {
                    self.paint_pos_markers(&painter, layer, |f| r.left() + (f as f32 + 0.5) * cell, grid_top + Self::pos_marker_y(row.top, row.h), cols, visible);
                }
            }
        }

        // Cles d'opacite (`opacitecleselui.rs`) : des losanges gris clair en bas de la ligne du calque dont l'opacite est animee.
        {
            let visible = (ui.clip_rect().left(), ui.clip_rect().right());
            for row in &rows {
                let layer = &self.layers()[row.li];
                if !layer.opacity_keys.is_empty() {
                    self.paint_op_markers(&painter, layer, |f| r.left() + (f as f32 + 0.5) * cell, grid_top + Self::op_marker_y(row.top, row.h), cols, visible);
                }
            }
        }

        // Commentaires (`commentairesui.rs`) : leurs fanions dans la règle du plan, le mât au
        // milieu de l'image. Un fanion pris garde la souris : la grille ne fait rien d'autre.
        let ruler_rect = Rect::from_min_size(r.min, vec2(size.x, HEADER_H));
        let plan_id = self.panel().id;
        let comments = self.doc.comment_positions(crate::commentaires::Scope::Plan(plan_id), &self.comment_ui.filter, self.comment_ui.sel);
        let comment_pole = |f: usize| r.left() + (f as f32 + 0.5) * cell;
        let comment_frame = |x: f32| ((x - r.left()) / cell).floor().max(0.0) as usize;
        let plain_ruler = resp.interact_pointer_pos().is_none_or(|p| self.timeline_handle_at(p, r, cell, &rows, grid_top).is_none());
        let comment_ptr = self.comment_pointer(
            ui,
            &resp,
            &crate::commentairesui::RulerView { ruler: ruler_rect, placed: &comments, pole_x: &comment_pole, frame_at: &comment_frame, scope: crate::commentaires::Scope::Plan(plan_id), plain: plain_ruler },
        );
        let comment_busy = comment_ptr.busy;

        // La clé de caméra sous le point `p` de la rangée Caméra : son image dans le plan.
        let cam_row = Rect::from_min_size(pos2(r.left(), grid_top + rows_h), vec2(size.x, CAM_H));
        let key_x = |f: usize| r.left() + (f as f32 + 0.5) * cell;
        let key_under = |app: &App, p: Pos2| {
            (cam_row.contains(p) && app.cle_vue(crate::legendecles::Sorte::Camera)).then(|| crate::camtools::camera_key_at(&app.panel().camera, p.x, key_x, cell * 0.5 + 2.0)).flatten()
        };
        let cam_frame_under = |app: &App, p: Pos2| key_under(app, p).and_then(|k| app.panel().camera.get(k).map(|k| k.frame));
        // La poignée de lissage sous le point p (la clé passe avant) : (rang de la clé, côté).
        let ease_under = |app: &App, p: Pos2| {
            (cam_row.contains(p) && app.cle_vue(crate::legendecles::Sorte::Camera)).then(|| crate::lissage::sous_pointeur(&app.panel().camera, p.x, &key_x, cell, crate::lissage::tol_cle(cell))).flatten()
        };

        // Clic droit sur une case de calque : elle est visée (sauf dans le choix), puis le menu
        // des rôles et des images. Ailleurs (passe interface) : la règle, une clé d'effet, la
        // caméra, le son ont leur menu (`handles::GridMenu`). Un fanion de commentaire a le sien.
        if let Some(id) = comment_ptr.menu {
            self.role_menu = false;
            self.grid_menu = Some(GridMenu::Comment(id));
        } else if resp.secondary_clicked() {
            self.role_menu = false;
            self.grid_menu = None;
            if let Some(p) = resp.interact_pointer_pos() {
                let col = ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                let y = p.y - grid_top;
                if p.y < grid_top {
                    self.set_frame_free(col);
                    self.grid_menu = Some(GridMenu::Ruler);
                } else if y < rows_h {
                    if let Some((layer, f)) = self.fx_marker_in_timeline(p, r, cell, &rows, grid_top) {
                        // Un losange qui n'était pas choisi l'est seul ; le menu vise le choix.
                        if !self.fx_marker_refs(self.cur_panel, layer, f).iter().all(|k| self.fx_key_sel.contains(k)) {
                            self.click_fx_marker(self.cur_panel, layer, f, false, false);
                        }
                        self.grid_menu = Some(GridMenu::FxKey);
                    } else if let Some((layer, f)) = self.op_marker_in_timeline(p, r, cell, &rows, grid_top) {
                        // Un losange d'opacite qui n'etait pas choisi l'est seul ; le menu vise le choix.
                        if !self.op_ui.sel.contains(&(self.layers()[layer].id, f)) {
                            self.click_op_marker(layer, f, false, false);
                        }
                        self.grid_menu = Some(GridMenu::OpKey);
                    } else if let Some((layer, f)) = self.pos_marker_in_timeline(p, r, cell, &rows, grid_top) {
                        // Un losange vert qui n'était pas choisi l'est seul ; le menu vise le choix.
                        if !self.pos_ui.sel.contains(&(self.layers()[layer].id, f)) {
                            self.click_pos_marker(layer, f, false);
                        }
                        self.grid_menu = Some(GridMenu::PosKey);
                    } else if let Some(row) = row_at(&rows, y).filter(|row| !self.layers()[row.li].folder) {
                        self.aim_cell(row.li, col);
                        self.role_menu = true;
                    }
                } else if y < rows_h + CAM_H {
                    // Un losange de clé de caméra qui n'était pas choisi l'est seul ; le menu vise le choix.
                    match cam_frame_under(self, p) {
                        Some(f) => {
                            if !self.cam_key_chosen(self.cur_panel, f) {
                                self.click_cam_key(self.cur_panel, f, false, false);
                            }
                        }
                        None => self.set_frame_free(col),
                    }
                    self.grid_menu = Some(GridMenu::Camera);
                } else if sound_h > 0.0 && p.y >= r.bottom() - sound_h {
                    let g = self.doc.panel_start(self.cur_panel) + col;
                    let hit = self.doc.audio_tracks.iter().enumerate().find_map(|(t, tr)| tr.clips.iter().find(|c| c.start <= g && g < c.end()).map(|c| (t, c.id)));
                    let (track, clip) = hit.map_or((0, None), |(t, id)| (t, Some(id)));
                    self.grid_menu = Some(GridMenu::Sound { track, clip, frame: g });
                }
            }
        }
        if self.role_menu || self.grid_menu.is_some() {
            resp.context_menu(|ui| match self.grid_menu {
                Some(m) => self.grid_menu_ui(ui, m),
                None => {
                    ui.menu_button("Rôle", |ui| self.roles_menu(ui));
                    ui.separator();
                    self.images_menu(ui);
                }
            });
        }

        // Double-clic sur un dessin : renommer son instance (13c) ; le champ se pose sur le bloc.
        if resp.double_clicked() && !comment_busy {
            if let Some(p) = resp.interact_pointer_pos() {
                let col = ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                if let Some(row) = row_at(&rows, p.y - grid_top) {
                    if let Some(cel) = self.layers()[row.li].cel_at(col) {
                        self.start_instance_rename(row.li, cel);
                    }
                }
            }
        }
        let renaming = self.instance_rename.as_ref().map(|n| (n.layer, n.cel));
        if let Some((layer_id, cel)) = renaming {
            let found = rows.iter().find(|row| self.layers()[row.li].id == layer_id).and_then(|row| {
                let l = &self.layers()[row.li];
                let start = l.frames.iter().position(|f| *f == Some(cel))?;
                Some(Rect::from_min_size(pos2(r.left() + start as f32 * cell, grid_top + row.top + (row.h - 20.0) * 0.5), vec2(120.0, 20.0)))
            });
            match found {
                Some(at) => {
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(at).id_salt("nom_instance"));
                    let n = self.instance_rename.as_mut().expect("renommage en cours");
                    let mut out = egui::TextEdit::singleline(&mut n.name).desired_width(at.width()).show(&mut child);
                    let field = out.response.clone();
                    if !n.focused {
                        field.request_focus();
                        let len = n.name.chars().count();
                        out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(len),
                        )));
                        out.state.store(ui.ctx(), field.id);
                        n.focused = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.instance_rename = None;
                    } else if field.lost_focus() {
                        self.commit_instance_rename();
                    }
                }
                None => self.instance_rename = None,
            }
        }

        // Au-delà de la fin du plan.
        let end_x = r.left() + count as f32 * cell;
        painter.rect_filled(
            Rect::from_min_max(pos2(end_x, grid_top), r.max),
            0.0,
            Color32::from_black_alpha(70),
        );
        let end_lit = lit == Some(Handle::PlanEnd);
        painter.line_segment(
            [pos2(end_x, r.top()), pos2(end_x, r.bottom())],
            Stroke::new(if end_lit { 2.0_f32 } else { 1.5_f32 }, if end_lit { theme::contour() } else { theme::texte_doux() }),
        );
        // La prise de la fin du plan, au bout de la règle (chantier 4) : on la tire pour changer
        // la durée du plan.
        let tab = Rect::from_min_size(pos2(end_x, r.top() + 2.0), vec2(PLAN_END_W - 2.0, HEADER_H - 4.0));
        // (Les essais retrouvent la prise de la fin du plan à l'écran par ce rectangle.)
        #[cfg(test)]
        PRISE_FIN_DU_PLAN.with(|c| c.set(Some(tab)));
        painter.rect_filled(tab, 2.0, if end_lit { theme::contour() } else { theme::texte_doux() });
        for dy in [-3.0, 0.0, 3.0] {
            painter.line_segment(
                [pos2(tab.left() + 2.0, tab.center().y + dy), pos2(tab.right() - 2.0, tab.center().y + dy)],
                Stroke::new(1.0_f32, theme::regle()),
            );
        }

        // Image courante.
        let cx = r.left() + self.cur_frame as f32 * cell;
        let column = Rect::from_min_size(pos2(cx, r.top()), vec2(cell, size.y));
        painter.rect_filled(column, 0.0, cur_frame().gamma_multiply(0.18));
        painter.rect_stroke(column, 0.0, Stroke::new(1.0_f32, cur_frame()), StrokeKind::Inside);
        painter.rect_filled(
            Rect::from_min_size(pos2(cx, r.top()), vec2(cell, HEADER_H)),
            0.0,
            cur_frame(),
        );
        painter.text(
            pos2(cx + cell * 0.5, r.top() + HEADER_H * 0.5),
            Align2::CENTER_CENTER,
            // En time code, seule l'image dans la seconde tient dans la case.
            match self.time_display {
                crate::cues::TimeDisplay::TimeCode => {
                    ((self.cur_frame + self.doc.start_frame).saturating_sub(1) % self.doc.fps.max(1) as usize).to_string()
                }
                _ => self.frame_label(self.cur_frame),
            },
            FontId::monospace(9.0),
            theme::encre(),
        );
        // Les fanions des commentaires, par-dessus la case de la tête de lecture.
        crate::commentairesui::paint_flags(&painter, ruler_rect, &comments, &comment_pole, self.comment_ui.sel, comment_ptr.hover);

        // Notes dessinées (13e) : leur rangée sous la caméra ; un geste qui y commence ne bouge
        // ni la tête de lecture ni les images.
        let notes_row = Rect::from_min_size(pos2(r.left(), grid_top + rows_h + CAM_H), vec2(size.x, notes_h));
        let in_notes = notes_h > 0.0 && ui.input(|i| i.pointer.press_origin()).is_some_and(|p| notes_row.contains(p));
        if notes_h > 0.0 {
            self.notes_row(ui, &painter, notes_row.shrink2(vec2(0.0, 1.0)), r.left(), cell, &resp);
        }

        // Son (13e) : Ctrl + glisser l'onde décale l'extrait sous la souris (comme dans TVPaint).
        let sound_row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max);
        let origin = ui.input(|i| i.pointer.press_origin());
        if sound_h > 0.0 && resp.drag_started() && ui.input(|i| i.modifiers.command) {
            if let Some(p) = origin.filter(|p| sound_row.contains(*p)) {
                let g = self.doc.panel_start(self.cur_panel) + ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                let hit = self.doc.audio_tracks.iter().enumerate().find_map(|(t, tr)| {
                    tr.clips.iter().find(|c| c.start <= g && g < c.end()).map(|c| (t, c.id))
                });
                if let Some((t, id)) = hit {
                    self.begin_clip_drag(id, crate::soundtrack::ClipEdge::Move, p);
                    if self.montage_drag.is_some() {
                        self.timeline_sound_track = Some(t);
                    }
                }
            }
        }
        if let Some(t) = self.timeline_sound_track {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                self.drag_timeline_sound(t, p, cell);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.timeline_sound_track = None;
                self.end_timeline_sound_drag();
            }
        }
        let in_notes = in_notes || self.timeline_sound_track.is_some();

        // Clés de caméra : on les choisit, les prend et les fait glisser le long de leur rangée
        // (12e, `camkeys.rs`).
        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin());
            let hit = origin.and_then(|p| cam_frame_under(self, p));
            // Un geste qui part d'un espace vide de la rangée Caméra (hors poignée : la fin du
            // plan) trace un rectangle de choix ; ailleurs, il lâche le choix des clés.
            let ease_hit = origin.filter(|_| hit.is_none() && !in_notes && !comment_busy).and_then(|p| ease_under(self, p));
            let rect_from = origin
                .filter(|p| hit.is_none() && ease_hit.is_none() && cam_row.contains(*p) && !in_notes && !comment_busy)
                .filter(|p| self.timeline_handle_at(*p, r, cell, &rows, grid_top).is_none());
            if hit.is_none() && rect_from.is_none() && ease_hit.is_none() {
                self.clear_cam_keys();
            }
            if let Some((i, cote)) = ease_hit {
                // Une poignée de lissage (lissage.rs) : elle règle l'adouci de sa clé, sans toucher au choix.
                if let Some(f) = self.panel().camera.get(i).map(|k| k.frame) {
                    self.begin_ease_drag(self.cur_panel, f, cote);
                }
            } else if let (Some(f), Some(p)) = (hit, origin) {
                let grab = self.doc.panel_start(self.cur_panel) + ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                self.begin_cam_keys_drag(self.cur_panel, f, grab);
            } else if let Some(p) = rect_from {
                let mods = ui.input(|i| i.modifiers);
                self.begin_cam_rect(p, mods.shift, mods.command && !mods.shift);
            }
        }
        // Le rectangle de choix des clés de caméra : seule sa largeur compte (la rangée est basse).
        let marquee = self.cam_rect.is_some();
        if let Some(o) = self.cam_rect.as_ref().map(|c| c.origin) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let rect = Rect::from_two_pos(o, p);
                let band = rect.union(Rect::from_center_size(pos2(rect.center().x, cam_row.center().y), vec2(0.0, CAM_H)));
                let panel = self.panel();
                let diamonds: Vec<_> = panel.camera.iter().map(|k| (crate::camkeys::CamKeyRef { panel: panel.id, frame: k.frame }, pos2(key_x(k.frame), cam_row.center().y))).collect();
                let picked = crate::camkeys::pick_in_rect(&diamonds, band, crate::camkeys::DIAMOND_HALF);
                self.drag_cam_rect(&picked);
                crate::camkeys::paint_marquee(&painter, rect);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_cam_rect();
            }
        }
        // Tant qu'on tient une clé, la tête de lecture ne bouge pas.
        let holding_ease = self.ease_drag.is_some();
        let holding_key = self.cam_keys_drag.is_some() || holding_ease;
        if holding_ease {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            if let (Some(p), Some((_, f))) = (resp.interact_pointer_pos().filter(|_| resp.dragged()), self.ease_drag_target()) {
                self.drag_ease(p.x, key_x(f), cell);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_ease_drag();
            }
        } else if holding_key {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let at = self.doc.panel_start(self.cur_panel) + ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                self.drag_cam_keys(at);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_cam_keys_drag();
            }
        } else if resp.hover_pos().is_some_and(|p| key_under(self, p).is_some()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        } else if resp.hover_pos().is_some_and(|p| ease_under(self, p).is_some()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        // Poignées (passe interface, `handles.rs`) : marques, signets, entrée et sortie dans la
        // règle ; clés d'effets et bord droit des dessins dans les lignes des calques. La case de
        // la tête de lecture, dans la règle, prend la tête (Anthony, 2026-10-17 : posée sur
        // l'entrée ou la sortie, elle ne se laissait plus glisser : la prise passait avant).
        let on_playhead = |p: Pos2| p.y >= r.top() && p.y < grid_top && p.x >= cx - 1.0 && p.x < cx + cell + 1.0;
        // Un clic ou un geste qui ne part pas d'un losange violet lâche le choix des clés d'effets.
        if (resp.clicked() || resp.drag_started()) && !self.fx_key_sel.is_empty() {
            let at = if resp.clicked() { resp.interact_pointer_pos() } else { ui.input(|i| i.pointer.press_origin()) };
            if at.is_none_or(|p| self.fx_marker_in_timeline(p, r, cell, &rows, grid_top).is_none()) {
                self.fx_key_sel.clear();
            }
        }
        // Les cles d'opacite (`opacitecleselui.rs`) : de meme, avec les losanges gris clair.
        if (resp.clicked() || resp.drag_started()) && !self.op_ui.sel.is_empty() && self.op_ui.drag.is_none() {
            let at = if resp.clicked() { resp.interact_pointer_pos() } else { ui.input(|i| i.pointer.press_origin()) };
            if at.is_none_or(|p| self.op_marker_in_timeline(p, r, cell, &rows, grid_top).is_none()) {
                self.op_ui.sel.clear();
            }
        }
        if resp.drag_started() && !holding_key && !in_notes && !comment_busy {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()).filter(|p| !on_playhead(*p)) {
                if let Some((li, f)) = self.op_marker_in_timeline(p, r, cell, &rows, grid_top) {
                    self.begin_op_drag(li, f, ((p.x - r.left()) / cell).floor().max(0.0) as usize);
                }
            }
        }
        // Les clés de position (`poskeysui.rs`) : un clic ou un geste qui ne part pas d'un losange vert lâche
        // le choix ; un glisser qui part d'un losange prend les clés choisies.
        if (resp.clicked() || resp.drag_started()) && !self.pos_ui.sel.is_empty() && self.pos_ui.drag.is_none() {
            let at = if resp.clicked() { resp.interact_pointer_pos() } else { ui.input(|i| i.pointer.press_origin()) };
            if at.is_none_or(|p| self.pos_marker_in_timeline(p, r, cell, &rows, grid_top).is_none()) {
                self.pos_ui.sel.clear();
            }
        }
        if resp.drag_started() && !holding_key && !in_notes && !comment_busy {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()).filter(|p| !on_playhead(*p)) {
                if let Some((li, f)) = self.pos_marker_in_timeline(p, r, cell, &rows, grid_top).filter(|_| self.op_ui.drag.is_none()) {
                    self.begin_pos_drag(li, f, ((p.x - r.left()) / cell).floor().max(0.0) as usize);
                }
            }
        }
        if resp.drag_started() && !holding_key && !in_notes && !comment_busy && self.pos_ui.drag.is_none() && self.op_ui.drag.is_none() {
            if let Some((p, h)) = ui
                .input(|i| i.pointer.press_origin())
                .filter(|p| !on_playhead(*p))
                .and_then(|p| self.timeline_handle_at(p, r, cell, &rows, grid_top).map(|h| (p, h)))
            {
                match h {
                    // Une clé d'effet emmène tout le choix (`fxkeys.rs`), une étape d'annulation.
                    Handle::FxKey { layer, effect, key } => {
                        if let Some(f) = self.layers().get(layer).and_then(|l| l.fx.get(effect)).and_then(|e| e.keys.get(key)).map(|k| k.frame) {
                            let grab = ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                            self.begin_fx_keys_drag(self.cur_panel, layer, f, grab);
                        }
                    }
                    h => {
                        self.begin_handle_drag(h);
                    }
                }
            }
        }
        if self.fx_keys_drag.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                self.drag_fx_keys(((p.x - r.left()) / cell).floor().max(0.0) as usize);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_fx_keys_drag();
            }
        }
        if self.op_ui.drag.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                self.drag_op_keys(((p.x - r.left()) / cell).floor().max(0.0) as usize);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_op_drag();
            }
        }
        if self.pos_ui.drag.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                self.drag_pos_keys(((p.x - r.left()) / cell).floor().max(0.0) as usize);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_pos_drag();
            }
        }
        let cursor_for = |h: Handle, held: bool| match h {
            Handle::Exposure { .. } | Handle::In | Handle::Out | Handle::LayerStart { .. } | Handle::LayerEnd { .. } | Handle::PlanEnd => {
                egui::CursorIcon::ResizeHorizontal
            }
            _ if held => egui::CursorIcon::Grabbing,
            _ => egui::CursorIcon::Grab,
        };
        if let Some(h) = self.handle_drag.as_ref().map(|d| d.handle) {
            ui.ctx().set_cursor_icon(cursor_for(h, true));
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let x = ((p.x - r.left()) / cell).max(0.0);
                self.drag_handle(x.floor() as usize, x.round() as usize);
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_handle_drag();
            }
        } else if !holding_key && self.image_drag.is_none() && self.fx_keys_drag.is_none() && self.pos_ui.drag.is_none() && self.op_ui.drag.is_none() {
            // Un losange vert se prend (`poskeysui.rs`), comme dans le montage.
            if resp.hover_pos().is_some_and(|p| !on_playhead(p) && (self.op_marker_in_timeline(p, r, cell, &rows, grid_top).is_some() || self.pos_marker_in_timeline(p, r, cell, &rows, grid_top).is_some())) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            } else if let Some(h) = resp.hover_pos().filter(|p| !on_playhead(*p)).and_then(|p| self.timeline_handle_at(p, r, cell, &rows, grid_top)) {
                ui.ctx().set_cursor_icon(cursor_for(h, false));
                // La bulle de la poignée de durée : Ctrl (les clés suivent la poignée).
                if h == Handle::PlanEnd {
                    resp.clone().on_hover_ui_at_pointer(|ui| crate::bulles::contenu(ui, crate::handles::BULLE_DUREE_NOM, crate::handles::BULLE_DUREE_AIDE, ""));
                }
            }
        }
        let holding_key = holding_key || marquee || self.handle_drag.is_some() || self.fx_keys_drag.is_some() || self.pos_ui.drag.is_some() || self.op_ui.drag.is_some() || comment_busy;

        // Clic et glisser : choisir l'image (et le calque au clic). Avec Maj : choisir les images
        // entre l'image courante et celle sous la souris, sur les calques choisis (13c-2).
        if resp.is_pointer_button_down_on() {
            self.images_focus = true;
            self.panel_focus = false;
            self.key_focus = false;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        // Glisser les images choisies (13c-3) : on les prend dans leur cadre ; lâchées sans
        // touche elles sont déplacées, avec Ctrl ou Alt copiées (des dessins à elles), avec Maj insérées.
        let col_of = |x: f32| ((x - r.left()) / cell).floor().max(0.0) as usize;
        if resp.drag_started() && !holding_key && !in_notes {
            if let (Some((a, b)), Some(p)) = (self.frame_sel, ui.input(|i| i.pointer.press_origin())) {
                let c = col_of(p.x);
                let on_chosen = row_at(&rows, p.y - grid_top).is_some_and(|row| self.is_chosen_layer(row.li));
                if c >= a && c < b && on_chosen {
                    self.image_drag = Some((c, c));
                }
            }
        }
        if let Some((c0, _)) = self.image_drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                self.image_drag = Some((c0, col_of(p.x)));
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                let (c0, c1) = self.image_drag.take().unwrap_or((c0, c0));
                let m = ui.input(|i| i.modifiers);
                let mode = crate::images::DropMode::de_touches(&m);
                // Sans touche, lâchées sur un autre dessin du calque pointé : les clés prises s'insèrent avant ou après lui (partie 137).
                let pointed = ui.input(|i| i.pointer.latest_pos()).and_then(|p| row_at(&rows, p.y - grid_top)).map(|row| row.li).filter(|&li| self.is_chosen_layer(li)).unwrap_or(self.cur_layer);
                match (mode, self.image_insertion(pointed, c1)) {
                    (crate::images::DropMode::Move, Some(d)) => self.insert_chosen_images(pointed, d),
                    _ => self.drop_images(c1 as isize - c0 as isize, mode),
                }
            }
        }
        // Choisir au rectangle (chantier 3) : un glisser qui part d'une case d'un calque (hors
        // poignée et hors des images déjà choisies) choisit les images et les calques couverts ;
        // la tête de lecture reste sur la case du départ. Glisser dans la règle fait défiler.
        if resp.drag_started() && !holding_key && !in_notes && self.image_drag.is_none() && !shift {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                let y = p.y - grid_top;
                if let Some(k) = rows.iter().position(|row| y >= row.top && y < row.top + row.h) {
                    let col = col_of(p.x);
                    let (before_frames, before_layers) = (self.frame_sel, self.layer_sel.clone());
                    self.set_frame_free(col);
                    self.layer_sel.clear();
                    self.select_layer(rows[k].li);
                    self.frame_sel = Some((col, col + 1));
                    self.grid_select = Some(GridSelect { col, row: k, before_frames, before_layers });
                }
            }
        }
        if let Some((c0, k0)) = self.grid_select.as_ref().map(|g| (g.col, g.row)) {
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let c = col_of(p.x);
                let y = (p.y - grid_top).clamp(0.0, (rows_h - 0.5).max(0.0));
                let k = rows.iter().position(|row| y >= row.top && y < row.top + row.h).unwrap_or(k0);
                self.frame_sel = Some((c0.min(c), c0.max(c) + 1));
                let (lo, hi) = (k0.min(k), k0.max(k));
                let cur = self.cur_layer;
                let ids: Vec<u64> = rows[lo..=hi]
                    .iter()
                    .map(|row| row.li)
                    .filter(|&li| li != cur && !self.layers()[li].folder)
                    .map(|li| self.layers()[li].id)
                    .collect();
                self.layer_sel = ids;
            }
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.grid_select = None;
                let (a, b) = self.image_range();
                let (n, m) = (b - a, self.image_layers().len());
                self.status = format!(
                    "{n} image{} × {m} calque{} choisis : les glisser pour les déplacer (sur un autre dessin : avant ou après lui ; Ctrl ou Alt : copier, Maj : insérer), Suppr pour les supprimer, Échap pendant le geste pour l'abandonner.",
                    if n > 1 { "s" } else { "" },
                    if m > 1 { "s" } else { "" }
                );
            }
        }
        // Clic sur un losange violet : il est choisi (Ctrl : ajouté ou retiré ; Maj : tous ceux
        // d'entre le dernier cliqué et lui), la tête de lecture va dessus.
        // Clic sur un losange vert (clé de position) : il est choisi (Ctrl : ajouté ou retiré), la tête de
        // lecture va dessus (`poskeysui.rs`).
        let mut pos_clicked = false;
        // Clic sur un losange d'opacite (`opacitecleselui.rs`) : comme les autres.
        if resp.clicked() && !holding_key && !in_notes {
            if let Some((layer, f)) = resp.interact_pointer_pos().and_then(|p| self.op_marker_in_timeline(p, r, cell, &rows, grid_top)) {
                let m = ui.input(|i| i.modifiers);
                self.click_op_marker(layer, f, m.command, m.shift && !m.command);
                pos_clicked = true;
            }
        }
        if resp.clicked() && !holding_key && !in_notes && !pos_clicked {
            if let Some((layer, f)) = resp.interact_pointer_pos().and_then(|p| self.pos_marker_in_timeline(p, r, cell, &rows, grid_top)) {
                let m = ui.input(|i| i.modifiers);
                self.click_pos_marker_with(layer, f, m.command, m.shift && !m.command);
                pos_clicked = true;
            }
        }
        let mut fx_clicked = false;
        if resp.clicked() && !holding_key && !in_notes && !pos_clicked {
            if let Some((layer, f)) = resp.interact_pointer_pos().and_then(|p| self.fx_marker_in_timeline(p, r, cell, &rows, grid_top)) {
                let m = ui.input(|i| i.modifiers);
                self.click_fx_marker(self.cur_panel, layer, f, m.command, m.shift);
                fx_clicked = true;
            }
        }
        // Clic sur un losange jaune de la rangée Caméra : la clé est choisie (Ctrl : ajoutée ou
        // retirée ; Maj : toutes celles d'entre), la tête de lecture va dessus (`camkeys.rs`).
        // Un clic ailleurs lâche le choix.
        let mut cam_clicked = false;
        if resp.clicked() && !holding_key && !in_notes {
            if let Some(p) = resp.interact_pointer_pos() {
                match cam_frame_under(self, p) {
                    Some(f) => {
                        let m = ui.input(|i| i.modifiers);
                        self.click_cam_key(self.cur_panel, f, m.command, m.shift);
                        cam_clicked = true;
                    }
                    // (Un clic sur une poignée de lissage ne fait rien : seul son glisser règle.)
                    None if ease_under(self, p).is_some() => cam_clicked = true,
                    None => self.clear_cam_keys(),
                }
            }
        }
        if (resp.clicked() || resp.dragged()) && !fx_clicked && !pos_clicked && !cam_clicked && !holding_key && self.image_drag.is_none() && self.grid_select.is_none() && !resp.drag_stopped() && !in_notes {
            if let Some(p) = resp.interact_pointer_pos() {
                let col = ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                if shift {
                    let (a, b) = (col.min(self.cur_frame), col.max(self.cur_frame) + 1);
                    self.frame_sel = Some((a, b));
                } else {
                    if resp.clicked() || resp.drag_started() {
                        self.frame_sel = None;
                    }
                    if col != self.cur_frame || self.playing {
                        self.set_frame_free(col);
                    }
                    if resp.clicked() || resp.drag_started() {
                        if let Some(row) = row_at(&rows, p.y - grid_top) {
                            self.select_layer(row.li);
                        }
                    }
                }
            }
        }

        // Pendant un glisser sans touche sur un autre dessin : la ligne d'insertion sur chaque calque choisi (partie 137).
        let insertion = match self.image_drag.filter(|(c0, c1)| c0 != c1) {
            Some((_, c1)) if ui.input(|i| i.modifiers.is_none()) => {
                let pointed = ui.input(|i| i.pointer.latest_pos()).and_then(|p| row_at(&rows, p.y - grid_top)).map(|row| row.li).filter(|&li| self.is_chosen_layer(li)).unwrap_or(self.cur_layer);
                self.image_insertion(pointed, c1).map(|d| self.image_insertion_lines(d))
            }
            _ => None,
        };
        if let Some(lines) = &insertion {
            for &(li, x) in lines {
                if let Some(row) = rows.iter().find(|row| row.li == li) {
                    let px = r.left() + x as f32 * cell;
                    let (top, bottom) = (grid_top + row.top, grid_top + row.top + row.h);
                    painter.line_segment([pos2(px, top), pos2(px, bottom)], Stroke::new(3.0_f32, theme::contour()));
                    let t = 4.0;
                    painter.add(egui::Shape::convex_polygon(vec![pos2(px - t, top), pos2(px + t, top), pos2(px, top + t)], theme::contour(), Stroke::NONE));
                    painter.add(egui::Shape::convex_polygon(vec![pos2(px - t, bottom), pos2(px, bottom - t), pos2(px + t, bottom)], theme::contour(), Stroke::NONE));
                }
            }
        }
        // Images choisies : un cadre blanc sur les calques choisis.
        if let Some((a, b)) = self.frame_sel {
            for row in rows.iter().filter(|row| self.is_chosen_layer(row.li) && !self.layers()[row.li].folder) {
                let sel = Rect::from_min_max(
                    pos2(r.left() + a as f32 * cell, grid_top + row.top + 1.0),
                    pos2(r.left() + b as f32 * cell, grid_top + row.top + row.h - 1.0),
                );
                painter.rect(sel, 2.0, theme::contour().gamma_multiply(0.11), Stroke::new(1.5_f32, theme::contour()), StrokeKind::Inside);
                // Pendant un glisser : là où elles vont se poser.
                if let Some((c0, c1)) = self.image_drag.filter(|(c0, c1)| c0 != c1 && insertion.is_none()) {
                    let dx = (c1 as f32 - c0 as f32) * cell;
                    let ghost = sel.translate(vec2(dx, 0.0));
                    painter.rect(ghost, 2.0, theme::marque().gamma_multiply(0.25), Stroke::new(1.5_f32, theme::marque()), StrokeKind::Inside);
                }
            }
        }

        if self.following {
            let row = rows.iter().find(|r| r.li == self.cur_layer).copied().unwrap_or(Row { li: 0, top: 0.0, h: ROW_H });
            let target = Rect::from_min_size(pos2(cx - cell * 2.0, grid_top + row.top), vec2(cell * 5.0, row.h));
            ui.scroll_to_rect(target, None);
        }
    }

    /// Poignée sous le point `p` de la grille (`r` : la grille, `cell` : largeur d'une image).
    /// Dans la règle : la fin du plan (sa prise au bout), un signet (en bas), l'entrée ou la
    /// sortie, une marque d'image. Dans une ligne de calque : une clé d'effet (en haut), le début
    /// ou la fin du calque, sa barre (en bas), le bord droit d'un dessin.
    fn timeline_handle_at(&self, p: Pos2, r: Rect, cell: f32, rows: &[Row], grid_top: f32) -> Option<Handle> {
        let x = |f: usize| r.left() + f as f32 * cell;
        let near = |at: f32, tol: f32| (p.x - at).abs() <= tol;
        let panel = self.panel();
        let end_x = x(panel.frame_count());
        let rows_h = rows_height(rows);
        // La fin du plan : sa prise dans la règle, ou son trait dans la ligne de la caméra.
        let in_ruler = p.y >= r.top() && p.y < grid_top;
        let in_cam = p.y >= grid_top + rows_h && p.y < grid_top + rows_h + CAM_H;
        if (in_ruler && p.x >= end_x - 1.0 && p.x <= end_x + PLAN_END_W) || (in_cam && near(end_x, 4.0)) {
            return Some(Handle::PlanEnd);
        }
        if in_ruler {
            if p.y >= grid_top - 9.0 {
                if let Some(i) = panel.cues.bookmarks.iter().position(|&f| near(x(f) + cell * 0.5, (cell * 0.5).max(5.0))) {
                    return Some(Handle::Bookmark(i));
                }
            }
            if panel.cues.in_point.is_some_and(|f| near(x(f) + 2.0, 5.0)) {
                return Some(Handle::In);
            }
            if panel.cues.out_point.is_some_and(|f| near(x(f + 1) - 2.0, 5.0)) {
                return Some(Handle::Out);
            }
            return panel.marks.iter().position(|m| p.x >= x(m.frame) - 3.0 && p.x <= x(m.frame) + 8.0).map(Handle::Mark);
        }
        let row = row_at(rows, p.y - grid_top)?;
        let layer = self.layers().get(row.li)?;
        // (Un calque de couleur suit les images de son trait : pas de poignée pour les déplacer.)
        if layer.folder || layer.video.is_some() || layer.keys_follow_line() {
            return None;
        }
        if let Some((li, f)) = self.fx_marker_in_timeline(p, r, cell, rows, grid_top) {
            if let Some((effect, key)) = layer.fx.iter().enumerate().find_map(|(e, fx)| fx.key_at(f).map(|k| (e, k))) {
                return Some(Handle::FxKey { layer: li, effect, key });
            }
        }
        let tol = (cell * 0.3).clamp(2.0, 5.0);
        // Le début et la fin du calque, puis sa barre (chantier 1).
        if let Some((a, b)) = crate::handles::content_bounds(layer) {
            if near(x(a), tol) {
                return Some(Handle::LayerStart { layer: row.li });
            }
            if near(x(b), tol) {
                return Some(Handle::LayerEnd { layer: row.li });
            }
            let in_bar = p.y - grid_top - row.top >= body_bottom(0.0, row.h);
            if in_bar && p.x > x(a) && p.x < x(b) {
                let grab = ((p.x - r.left()) / cell).floor().max(0.0) as usize;
                return Some(Handle::LayerMove { layer: row.li, grab });
            }
        }
        let b = ((p.x - r.left()) / cell).round().max(0.0) as usize;
        if b >= 1 && near(x(b), tol) {
            let (s, e) = layer.run_bounds(b - 1);
            if e == b && layer.frames.get(s).is_some_and(Option::is_some) {
                return Some(Handle::Exposure { layer: row.li, start: s, end: e });
            }
        }
        None
    }
}
