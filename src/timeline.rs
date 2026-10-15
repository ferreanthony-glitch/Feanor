use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};

use crate::app::{App, Mode};
use crate::document::LayerKind;
use crate::storyboard::duration_label;
use crate::theme;

const CELL_W: f32 = 16.0;
const ROW_H: f32 = 24.0;
const HEADER_H: f32 = 20.0;
const NAMES_W: f32 = 170.0;
/// Ligne du son sous les calques (forme d'onde du montage pendant le panneau).
const SOUND_H: f32 = 30.0;
/// Ligne des images clés de la caméra, sous les calques.
const CAM_H: f32 = 18.0;
const CAM_KEY: Color32 = theme::CAMERA;

pub(crate) const CUR_FRAME: Color32 = theme::IMAGE_COURANTE;

impl App {
    /// Hauteur de la ligne du son (rien s'il n'y a aucun son dans le projet).
    fn sound_row_h(&self) -> f32 {
        if self.doc.audio_tracks.iter().any(|t| !t.clips.is_empty()) {
            SOUND_H
        } else {
            0.0
        }
    }

    pub(crate) fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        self.timeline_toolbar(ui);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("timeline_v")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    self.timeline_names(ui);
                    egui::ScrollArea::horizontal()
                        .id_salt("timeline_h")
                        .auto_shrink([false, true])
                        .show(ui, |ui| self.timeline_grid(ui));
                });
            });
    }

    /// Barre de la timeline, sur une seule ligne (12c) : les commandes en icônes, leur nom, leur
    /// raccourci et une description dans une bulle (12d) ; si la fenêtre est trop étroite, la ligne défile au lieu de passer à la
    /// ligne.
    fn timeline_toolbar(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, interrupteur};
        use egui_phosphor::regular as ph;
        let count = self.frame_count();
        let fps = self.doc.fps;
        egui::ScrollArea::horizontal()
            .id_salt("barre_timeline")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let sep = |ui: &mut egui::Ui| {
                        ui.add_space(3.0);
                        ui.separator();
                        ui.add_space(3.0);
                    };
                    if self.bouton(ui, ph::ARROW_U_UP_LEFT, "Retour au storyboard", "Revenir au montage des panneaux.", "mode.basculer").clicked() {
                        self.set_mode(Mode::Storyboard);
                    }
                    let resp = ui.label(RichText::new(crate::storyboard::panel_title(&self.doc, self.cur_panel)).strong());
                    bulle(resp, &crate::storyboard::panel_long_title(&self.doc, self.cur_panel), "Le panneau que l'on anime.", "");
                    let resp = self.panel_length_field(ui, "");
                    bulle(resp, "Durée du panneau", &format!("{} ; glisser ou taper pour la changer.", duration_label(count, fps)), "");
                    sep(ui);

                    // Lecture.
                    if self.bouton(ui, ph::SKIP_BACK, "Première image", "Première image du panneau.", "image.premiere").clicked() {
                        self.goto_frame(0);
                    }
                    if self.bouton(ui, ph::CARET_LEFT, "Image précédente", "", "image.precedente").clicked() {
                        self.goto_frame(self.cur_frame.saturating_sub(1));
                    }
                    let play = if self.playing { ph::PAUSE } else { ph::PLAY };
                    if self.bouton(ui, play, "Lecture / pause", "Lire le panneau.", "lecture.basculer").clicked() {
                        self.toggle_play();
                    }
                    if self.bouton(ui, ph::CARET_RIGHT, "Image suivante", "", "image.suivante").clicked() {
                        self.goto_frame(self.cur_frame + 1);
                    }
                    if self.bouton(ui, ph::SKIP_FORWARD, "Dernière image", "Dernière image du panneau.", "image.derniere").clicked() {
                        self.goto_frame(count - 1);
                    }
                    interrupteur(ui, &mut self.looping, ph::REPEAT, "Lecture en boucle", "Reprendre au début une fois à la fin.", "");
                    let mut fps = self.doc.fps;
                    let resp = ui.add(egui::DragValue::new(&mut fps).range(1..=120).suffix(" i/s"));
                    let resp = bulle(resp, "Cadence", "Images par seconde, pour tout le projet.", "");
                    if self.edited(&resp) {
                        self.doc.fps = fps;
                    }
                    let resp = ui.label(RichText::new(format!("{} / {}", self.cur_frame + 1, count)).monospace());
                    bulle(resp, "Image courante", "Image courante / nombre d'images du panneau.", "");
                    sep(ui);

                    // Dessins et expositions.
                    if self.bouton(ui, ph::FILE_PLUS, "Nouveau dessin", "Un dessin vide après l'exposition courante.", "dessin.nouveau").clicked() {
                        self.new_image(false);
                    }
                    if self.bouton(ui, ph::COPY, "Dupliquer le dessin", "Une copie du dessin courant, après lui.", "dessin.dupliquer").clicked() {
                        self.new_image(true);
                    }
                    if self.bouton(ui, ph::PLUS, "Allonger l'exposition", "Tenir le dessin une image de plus.", "exposition.allonger").clicked() {
                        self.exposure_plus();
                    }
                    if self.bouton(ui, ph::MINUS, "Raccourcir l'exposition", "Tenir le dessin une image de moins.", "exposition.raccourcir").clicked() {
                        self.exposure_minus();
                    }
                    if self.bouton(ui, ph::TRASH, "Supprimer le dessin", "Retirer le dessin et ses expositions de la timeline.", "dessin.supprimer").clicked() {
                        self.delete_image();
                    }
                    if self.bouton(ui, ph::ERASER, "Vider le dessin", "Effacer tout ce qu'il contient ; il reste exposé.", "edition.supprimer").clicked() {
                        self.clear_image();
                    }
                    let resp = ui.add(egui::DragValue::new(&mut self.default_exposure).range(1..=12).prefix("en "));
                    bulle(resp, "Exposition des nouveaux dessins", "Nombre d'images que tient chaque nouveau dessin (« en 1 », « en 2 »…).", "");
                    sep(ui);

                    self.camera_keys_ui(ui);
                    sep(ui);

                    // Pelure d'oignon.
                    let key = self.raccourci("vue.pelure");
                    interrupteur(ui, &mut self.onion.enabled, ph::CIRCLES_THREE, "Pelure d'oignon", "Montrer les dessins d'avant et d'après.", &key);
                    let max = crate::onion::MAX_NEIGHBOURS;
                    let resp = ui.add(egui::DragValue::new(&mut self.onion.before).range(0..=max).prefix("‹ "));
                    bulle(resp, "Avant", "Nombre de dessins (ou d'images) montrés avant.", "");
                    let resp = ui.add(egui::DragValue::new(&mut self.onion.after).range(0..=max).suffix(" ›"));
                    bulle(resp, "Après", "Nombre de dessins (ou d'images) montrés après.", "");
                    let resp = ui.menu_button(RichText::new(ph::GEAR).size(crate::bulles::TAILLE), |ui| self.onion_settings_ui(ui)).response;
                    bulle(resp, "Réglages de la pelure", "Compter par dessin ou par image, opacité, couleurs.", "");
                });
            });
    }
    /// Réglages détaillés de la pelure d'oignon (menu de la timeline).
    fn onion_settings_ui(&mut self, ui: &mut egui::Ui) {
        use crate::onion::{OnionMode, OnionSettings};
        let s = &mut self.onion;
        ui.label(RichText::new("Compter").strong());
        ui.radio_value(&mut s.mode, OnionMode::Drawings, "par dessin : les dessins différents les plus proches")
            .on_hover_text("« 3 avant » = les 3 dessins précédents, quelle que soit leur durée.");
        ui.radio_value(&mut s.mode, OnionMode::Frames, "par image : les images voisines de la timeline")
            .on_hover_text("« 3 avant » = les 3 images précédentes : un dessin tenu longtemps cache les autres.");
        ui.separator();
        ui.add(egui::Slider::new(&mut s.opacity, 0.05..=1.0).text("opacité du plus proche"));
        ui.checkbox(&mut s.tinted, "Teinter (sinon : vraies couleurs)");
        ui.add_enabled_ui(s.tinted, |ui| {
            ui.horizontal(|ui| {
                ui.color_edit_button_srgb(&mut s.before_color);
                ui.label("avant");
                ui.add_space(12.0);
                ui.color_edit_button_srgb(&mut s.after_color);
                ui.label("après");
            });
        });
        ui.separator();
        if ui
            .button("Valeurs par défaut")
            .on_hover_text("3 avant en rouge, 3 après en vert, par dessin (tes préférences Storyboard Pro).")
            .clicked()
        {
            let enabled = s.enabled;
            *s = OnionSettings { enabled, ..OnionSettings::default() };
        }
    }

    fn timeline_names(&mut self, ui: &mut egui::Ui) {
        let rows = self.layers().len();
        let sound_h = self.sound_row_h();
        let size = vec2(NAMES_W, HEADER_H + rows as f32 * ROW_H + CAM_H + sound_h);
        let (resp, painter) = ui.allocate_painter(size, Sense::click());
        let r = resp.rect;
        let visuals = ui.visuals();
        painter.rect_filled(Rect::from_min_size(r.min, vec2(NAMES_W, HEADER_H)), 0.0, theme::REGLE);
        {
            let y = r.top() + HEADER_H + rows as f32 * ROW_H;
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(NAMES_W, CAM_H)).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 2.0, theme::RANGEE_SOMBRE);
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
        }
        if sound_h > 0.0 {
            let row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 2.0, theme::RANGEE_SOMBRE);
            painter.text(
                pos2(r.left() + 8.0, row.center().y),
                Align2::LEFT_CENTER,
                "Son (montage)",
                FontId::proportional(12.0),
                visuals.weak_text_color(),
            );
        }
        painter.text(
            pos2(r.left() + 6.0, r.top() + HEADER_H * 0.5),
            Align2::LEFT_CENTER,
            "Calques",
            FontId::proportional(11.0),
            visuals.weak_text_color(),
        );
        for row in 0..rows {
            let li = rows - 1 - row;
            let layer = &self.layers()[li];
            let y = r.top() + HEADER_H + row as f32 * ROW_H;
            let row_rect = Rect::from_min_size(pos2(r.left(), y), vec2(NAMES_W, ROW_H)).shrink2(vec2(0.0, 1.0));
            let bg = if li == self.cur_layer {
                visuals.selection.bg_fill
            } else {
                theme::ETIQUETTE
            };
            painter.rect_filled(row_rect, 2.0, bg);
            // Sur le bleu du calque courant, le texte passe en blanc (le seul lisible sur l'accent).
            let current = li == self.cur_layer;
            let text_color = match (layer.visible, current) {
                (true, true) => visuals.selection.stroke.color,
                (true, false) => visuals.text_color(),
                (false, _) => visuals.weak_text_color(),
            };
            painter.text(
                pos2(r.left() + ROW_H * 0.5, row_rect.center().y),
                Align2::CENTER_CENTER,
                if layer.visible { "👁" } else { "·" },
                FontId::proportional(13.0),
                text_color,
            );
            let badge = match layer.kind {
                _ if current => visuals.selection.stroke.color,
                LayerKind::Vector => theme::DESSIN_VECTORIEL,
                LayerKind::Bitmap => theme::DESSIN_BITMAP,
            };
            painter.text(
                pos2(r.left() + ROW_H + 2.0, row_rect.center().y),
                Align2::LEFT_CENTER,
                layer.kind.badge(),
                FontId::monospace(11.0),
                badge,
            );
            painter.text(
                pos2(r.left() + ROW_H + 16.0, row_rect.center().y),
                Align2::LEFT_CENTER,
                &layer.name,
                FontId::proportional(13.0),
                text_color,
            );
            // Cadenas à droite : plein si le calque est verrouillé, à peine visible sinon.
            painter.text(
                pos2(r.right() - ROW_H * 0.5, row_rect.center().y),
                Align2::CENTER_CENTER,
                "🔒",
                FontId::proportional(12.0),
                if layer.locked {
                    visuals.text_color()
                } else {
                    visuals.weak_text_color().gamma_multiply(0.35)
                },
            );
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let row = ((p.y - r.top() - HEADER_H) / ROW_H).floor();
                if row >= 0.0 && (row as usize) < rows {
                    let li = rows - 1 - row as usize;
                    if p.x < r.left() + ROW_H {
                        self.record_step();
                        let layer = &mut self.layers_mut()[li];
                        layer.visible = !layer.visible;
                    } else if p.x > r.right() - ROW_H {
                        self.toggle_layer_lock(li);
                    } else {
                        self.select_layer(li);
                    }
                }
            }
        }
    }

    fn timeline_grid(&mut self, ui: &mut egui::Ui) {
        let count = self.frame_count();
        let rows = self.layers().len();
        let visible_cols = (ui.available_width() / CELL_W) as usize + 1;
        let cols = (count + 48).max(self.cur_frame + 24).max(visible_cols);
        let sound_h = self.sound_row_h();
        let size = vec2(cols as f32 * CELL_W, HEADER_H + rows as f32 * ROW_H + CAM_H + sound_h);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let r = resp.rect;
        let visuals = ui.visuals().clone();
        let grid_top = r.top() + HEADER_H;
        // Règle des images.
        painter.rect_filled(Rect::from_min_size(r.min, vec2(size.x, HEADER_H)), 0.0, theme::REGLE);

        // Images clés de la caméra : losanges (ronds si le mouvement y est adouci), reliés par un
        // trait là où la caméra bouge.
        {
            let y = grid_top + rows as f32 * ROW_H;
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(size.x, CAM_H)).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 0.0, theme::RANGEE_SOMBRE);
            let keys = &self.panel().camera;
            let cx = |f: usize| r.left() + (f as f32 + 0.5) * CELL_W;
            if keys.len() >= 2 {
                painter.line_segment(
                    [pos2(cx(keys[0].frame), row.center().y), pos2(cx(keys[keys.len() - 1].frame), row.center().y)],
                    Stroke::new(2.0_f32, CAM_KEY.gamma_multiply(0.45)),
                );
            }
            for k in keys {
                let c = pos2(cx(k.frame), row.center().y);
                if k.smooth {
                    painter.circle_filled(c, 4.5, CAM_KEY);
                } else {
                    let d = 5.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)],
                        CAM_KEY,
                        Stroke::new(1.0_f32, Color32::from_black_alpha(140)),
                    ));
                }
            }
        }

        // Le son du montage pendant ce panneau (toutes pistes superposées), en lecture seule.
        if sound_h > 0.0 {
            let row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 0.0, theme::RANGEE_SOMBRE);
            let start = self.doc.panel_start(self.cur_panel);
            let left = r.left() - start as f32 * CELL_W;
            let fps = self.doc.fps;
            for track in &self.doc.audio_tracks {
                let color = if track.muted {
                    theme::TEXTE_DOUX
                } else {
                    theme::SON_ONDE
                };
                for clip in &track.clips {
                    let cr = Rect::from_min_max(
                        pos2(left + clip.start as f32 * CELL_W, row.top() + 2.0),
                        pos2(left + clip.end() as f32 * CELL_W, row.bottom() - 2.0),
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
                            CELL_W,
                            fps,
                            color,
                        );
                    }
                }
            }
        }

        // Fond des lignes, calque courant en surbrillance.
        for row in 0..rows {
            let li = rows - 1 - row;
            let y = grid_top + row as f32 * ROW_H;
            let rect = Rect::from_min_size(pos2(r.left(), y), vec2(size.x, ROW_H)).shrink2(vec2(0.0, 1.0));
            let bg = if li == self.cur_layer {
                visuals.selection.bg_fill.gamma_multiply(0.35)
            } else {
                theme::RANGEE
            };
            painter.rect_filled(rect, 0.0, bg);
        }

        // Graduations : une seconde = trait plus marqué.
        let fps = self.doc.fps.max(1) as usize;
        for c in 0..=cols {
            let x = r.left() + c as f32 * CELL_W;
            let color = if c % fps == 0 {
                theme::GRADUATION_SECONDE
            } else {
                theme::GRADUATION
            };
            painter.line_segment([pos2(x, grid_top), pos2(x, r.bottom())], Stroke::new(1.0_f32, color));
            if c < cols && (c == 0 || (c + 1) % 5 == 0) {
                painter.text(
                    pos2(x + CELL_W * 0.5, r.top() + HEADER_H * 0.5),
                    Align2::CENTER_CENTER,
                    (c + 1).to_string(),
                    FontId::monospace(9.0),
                    visuals.weak_text_color(),
                );
            }
        }

        // Dessins : un bloc par exposition, pastille au début du dessin.
        for row in 0..rows {
            let li = rows - 1 - row;
            let layer = &self.layers()[li];
            let y = grid_top + row as f32 * ROW_H;
            let mut f = 0;
            while f < layer.frames.len() {
                let (start, end) = layer.run_bounds(f);
                if let Some(id) = layer.frames[start] {
                    let block = Rect::from_min_max(
                        pos2(r.left() + start as f32 * CELL_W + 1.0, y + 4.0),
                        pos2(r.left() + end as f32 * CELL_W - 1.0, y + ROW_H - 4.0),
                    );
                    let empty = self.doc.cels.get(&id).is_none_or(|c| c.is_empty());
                    let mut fill = match (empty, layer.kind) {
                        (true, _) => theme::DESSIN_VIDE,
                        (false, LayerKind::Vector) => theme::DESSIN_VECTORIEL,
                        (false, LayerKind::Bitmap) => theme::DESSIN_BITMAP,
                    };
                    if !layer.visible {
                        fill = fill.gamma_multiply(0.5);
                    }
                    painter.rect_filled(block, 3.0, fill);
                    painter.circle_filled(
                        pos2(block.left() + CELL_W * 0.5 - 1.0, block.center().y),
                        2.5,
                        theme::FOND,
                    );
                    if end - start > 1 {
                        painter.line_segment(
                            [
                                pos2(block.left() + CELL_W - 1.0, block.center().y),
                                pos2(block.right() - 4.0, block.center().y),
                            ],
                            Stroke::new(1.0_f32, theme::ENTETE),
                        );
                    }
                }
                f = end;
            }
        }

        // Au-delà de la fin du plan.
        let end_x = r.left() + count as f32 * CELL_W;
        painter.rect_filled(
            Rect::from_min_max(pos2(end_x, grid_top), r.max),
            0.0,
            Color32::from_black_alpha(70),
        );
        painter.line_segment(
            [pos2(end_x, r.top()), pos2(end_x, r.bottom())],
            Stroke::new(1.5_f32, theme::TEXTE_DOUX),
        );

        // Image courante.
        let cx = r.left() + self.cur_frame as f32 * CELL_W;
        let column = Rect::from_min_size(pos2(cx, r.top()), vec2(CELL_W, size.y));
        painter.rect_filled(column, 0.0, CUR_FRAME.gamma_multiply(0.18));
        painter.rect_stroke(column, 0.0, Stroke::new(1.0_f32, CUR_FRAME), StrokeKind::Inside);
        painter.rect_filled(
            Rect::from_min_size(pos2(cx, r.top()), vec2(CELL_W, HEADER_H)),
            0.0,
            CUR_FRAME,
        );
        painter.text(
            pos2(cx + CELL_W * 0.5, r.top() + HEADER_H * 0.5),
            Align2::CENTER_CENTER,
            (self.cur_frame + 1).to_string(),
            FontId::monospace(9.0),
            Color32::BLACK,
        );

        // Clic et glisser : choisir l'image (et le calque au clic).
        if resp.clicked() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let col = ((p.x - r.left()) / CELL_W).floor().max(0.0) as usize;
                if col != self.cur_frame || self.playing {
                    self.set_frame_free(col);
                }
                if resp.clicked() || resp.drag_started() {
                    let row = ((p.y - grid_top) / ROW_H).floor();
                    if row >= 0.0 && (row as usize) < rows {
                        self.select_layer(rows - 1 - row as usize);
                    }
                }
            }
        }

        if self.follow_frame {
            let row = rows - 1 - self.cur_layer.min(rows - 1);
            let target = Rect::from_min_size(
                pos2(cx - CELL_W * 2.0, grid_top + row as f32 * ROW_H),
                vec2(CELL_W * 5.0, ROW_H),
            );
            ui.scroll_to_rect(target, None);
            self.follow_frame = false;
        }
    }
}
