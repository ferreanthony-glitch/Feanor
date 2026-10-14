use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};

use crate::app::{App, Mode};
use crate::document::LayerKind;
use crate::storyboard::duration_label;

const CELL_W: f32 = 16.0;
const ROW_H: f32 = 24.0;
const HEADER_H: f32 = 20.0;
const NAMES_W: f32 = 170.0;
/// Ligne du son sous les calques (forme d'onde du montage pendant le panneau).
const SOUND_H: f32 = 30.0;

pub(crate) const CUR_FRAME: Color32 = Color32::from_rgb(255, 96, 72);

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

    fn timeline_toolbar(&mut self, ui: &mut egui::Ui) {
        let count = self.frame_count();
        let fps = self.doc.fps;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(RichText::new("< Storyboard").strong())
                .on_hover_text("Revenir au montage des panneaux (Tab)")
                .clicked()
            {
                self.set_mode(Mode::Storyboard);
            }
            let title = crate::storyboard::panel_long_title(&self.doc, self.cur_panel);
            ui.label(RichText::new(title).strong());
            self.panel_length_field(ui, "durée ")
                .on_hover_text(duration_label(count, fps));
            ui.separator();

            if ui.button("⏮").on_hover_text("Première image (Origine)").clicked() {
                self.goto_frame(0);
            }
            if ui.button("⏪").on_hover_text("Image précédente (flèche gauche)").clicked() {
                self.goto_frame(self.cur_frame.saturating_sub(1));
            }
            let play = if self.playing { "⏸ Pause" } else { "▶ Lecture" };
            if ui.button(play).on_hover_text("Lecture du panneau en boucle (Entrée)").clicked() {
                self.toggle_play();
            }
            if ui.button("⏩").on_hover_text("Image suivante (flèche droite)").clicked() {
                self.goto_frame(self.cur_frame + 1);
            }
            if ui.button("⏭").on_hover_text("Dernière image (Fin)").clicked() {
                self.goto_frame(count - 1);
            }
            ui.checkbox(&mut self.looping, "Boucle");
            let mut fps = self.doc.fps;
            let resp = ui.add(egui::DragValue::new(&mut fps).range(1..=120).suffix(" i/s"));
            if self.edited(&resp) {
                self.doc.fps = fps;
            }
            ui.label(format!("Image {} / {}", self.cur_frame + 1, count));

            ui.separator();
            if ui
                .button("Nouvelle image")
                .on_hover_text("Insère un dessin vide après l'exposition courante (Inser)")
                .clicked()
            {
                self.new_image(false);
            }
            if ui.button("Dupliquer").on_hover_text("Copie du dessin courant (Ctrl+D)").clicked() {
                self.new_image(true);
            }
            if ui.button("+ Expo").on_hover_text("Tenir le dessin une image de plus (+)").clicked() {
                self.exposure_plus();
            }
            if ui.button("− Expo").on_hover_text("Tenir le dessin une image de moins (−)").clicked() {
                self.exposure_minus();
            }
            if ui
                .button("Supprimer")
                .on_hover_text("Retire le dessin et ses expositions de la timeline (Ctrl+Suppr)")
                .clicked()
            {
                self.delete_image();
            }
            if ui.button("Effacer").on_hover_text("Vide le dessin (Suppr)").clicked() {
                self.clear_image();
            }
            ui.add(
                egui::DragValue::new(&mut self.default_exposure)
                    .range(1..=12)
                    .prefix("nouveaux dessins sur "),
            )
            .on_hover_text("Nombre d'images que tient chaque nouveau dessin (1 = « en 1 », 2 = « en 2 »…)");

            ui.separator();
            ui.checkbox(&mut self.onion.enabled, "Pelure d'oignon (O)");
            ui.add(egui::DragValue::new(&mut self.onion.before).range(0..=6).prefix("avant "));
            ui.add(egui::DragValue::new(&mut self.onion.after).range(0..=6).prefix("après "));
            ui.add(
                egui::Slider::new(&mut self.onion.opacity, 0.05..=1.0)
                    .show_value(false)
                    .text("opacité"),
            );
        });
    }

    fn timeline_names(&mut self, ui: &mut egui::Ui) {
        let rows = self.layers().len();
        let sound_h = self.sound_row_h();
        let size = vec2(NAMES_W, HEADER_H + rows as f32 * ROW_H + sound_h);
        let (resp, painter) = ui.allocate_painter(size, Sense::click());
        let r = resp.rect;
        let visuals = ui.visuals();
        if sound_h > 0.0 {
            let row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 2.0, Color32::from_gray(36));
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
                visuals.faint_bg_color
            };
            painter.rect_filled(row_rect, 2.0, bg);
            let text_color = if layer.visible {
                visuals.text_color()
            } else {
                visuals.weak_text_color()
            };
            painter.text(
                pos2(r.left() + ROW_H * 0.5, row_rect.center().y),
                Align2::CENTER_CENTER,
                if layer.visible { "👁" } else { "·" },
                FontId::proportional(13.0),
                text_color,
            );
            let badge = match layer.kind {
                LayerKind::Vector => Color32::from_rgb(70, 165, 255),
                LayerKind::Bitmap => Color32::from_gray(150),
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
        let size = vec2(cols as f32 * CELL_W, HEADER_H + rows as f32 * ROW_H + sound_h);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let r = resp.rect;
        let visuals = ui.visuals().clone();
        let grid_top = r.top() + HEADER_H;

        // Le son du montage pendant ce panneau (toutes pistes superposées), en lecture seule.
        if sound_h > 0.0 {
            let row = Rect::from_min_max(pos2(r.left(), r.bottom() - sound_h), r.max).shrink2(vec2(0.0, 1.0));
            painter.rect_filled(row, 0.0, Color32::from_gray(32));
            let start = self.doc.panel_start(self.cur_panel);
            let left = r.left() - start as f32 * CELL_W;
            let fps = self.doc.fps;
            for track in &self.doc.audio_tracks {
                let color = if track.muted {
                    Color32::from_gray(90)
                } else {
                    Color32::from_rgb(120, 190, 215)
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
                visuals.faint_bg_color
            };
            painter.rect_filled(rect, 0.0, bg);
        }

        // Graduations : une seconde = trait plus marqué.
        let fps = self.doc.fps.max(1) as usize;
        for c in 0..=cols {
            let x = r.left() + c as f32 * CELL_W;
            let color = if c % fps == 0 {
                Color32::from_gray(85)
            } else {
                Color32::from_gray(52)
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
                        (true, _) => Color32::from_rgb(92, 96, 108),
                        (false, LayerKind::Vector) => Color32::from_rgb(96, 168, 220),
                        (false, LayerKind::Bitmap) => Color32::from_rgb(120, 146, 210),
                    };
                    if !layer.visible {
                        fill = fill.gamma_multiply(0.5);
                    }
                    painter.rect_filled(block, 3.0, fill);
                    painter.circle_filled(
                        pos2(block.left() + CELL_W * 0.5 - 1.0, block.center().y),
                        2.5,
                        Color32::from_gray(25),
                    );
                    if end - start > 1 {
                        painter.line_segment(
                            [
                                pos2(block.left() + CELL_W - 1.0, block.center().y),
                                pos2(block.right() - 4.0, block.center().y),
                            ],
                            Stroke::new(1.0_f32, Color32::from_gray(40)),
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
            Stroke::new(1.5_f32, Color32::from_gray(140)),
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
