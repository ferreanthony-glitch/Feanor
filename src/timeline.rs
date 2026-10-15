use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};

use crate::app::{App, Mode};
use crate::document::LayerKind;
use crate::layerheads::{row_at, rows_height, rows_of, Row, HEAD_W};
use crate::storyboard::duration_label;
use crate::theme;

const CELL_W: f32 = 16.0;
const ROW_H: f32 = crate::layerheads::ROW_SMALL;
const HEADER_H: f32 = 20.0;
/// Largeur de l'en-tête des calques (13b).
const NAMES_W: f32 = HEAD_W;
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
                    ui.add_space(2.0);
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
                    if self.bouton(ui, ph::ARROW_U_UP_LEFT, "Retour au storyboard", "Revenir au montage des plans.", "mode.basculer").clicked() {
                        self.set_mode(Mode::Storyboard);
                    }
                    let resp = ui.label(RichText::new(crate::storyboard::panel_title(&self.doc, self.cur_panel)).strong());
                    bulle(resp, &crate::storyboard::panel_long_title(&self.doc, self.cur_panel), "Le plan que l'on anime.", "");
                    let resp = self.panel_length_field(ui, "");
                    bulle(resp, "Durée du plan", &format!("{} ; glisser ou taper pour la changer.", duration_label(count, fps)), "");
                    sep(ui);

                    // Lecture.
                    if self.bouton(ui, ph::SKIP_BACK, "Première image", "Première image du plan.", "image.premiere").clicked() {
                        self.goto_frame(0);
                    }
                    if self.bouton(ui, ph::CARET_LEFT, "Image précédente", "", "image.precedente").clicked() {
                        self.goto_frame(self.cur_frame.saturating_sub(1));
                    }
                    let play = if self.playing { ph::PAUSE } else { ph::PLAY };
                    if self.bouton(ui, play, "Lecture / pause", "Lire le plan.", "lecture.basculer").clicked() {
                        self.toggle_play();
                    }
                    if self.bouton(ui, ph::CARET_RIGHT, "Image suivante", "", "image.suivante").clicked() {
                        self.goto_frame(self.cur_frame + 1);
                    }
                    if self.bouton(ui, ph::SKIP_FORWARD, "Dernière image", "Dernière image du plan.", "image.derniere").clicked() {
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
                    bulle(resp, "Image courante", "Image courante / nombre d'images du plan.", "");
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
                    // Images (13c) : opérations sur les images choisies (Maj + clic dans la grille).
                    let resp = ui.menu_button(RichText::new(ph::FILM_STRIP).size(crate::bulles::TAILLE), |ui| self.images_menu(ui)).response;
                    bulle(
                        resp,
                        "Images",
                        "Supprimer, couper, copier, coller, dupliquer, inverser, insérer… Maj + clic dans la grille pour choisir des images.",
                        "",
                    );
                    sep(ui);

                    self.camera_keys_ui(ui);
                    sep(ui);

                    // Marque d'image (13a) : une image de storyboard de plus dans le plan.
                    let here = self.panel().marks.iter().any(|m| m.frame == self.cur_frame);
                    let resp = self.bouton_etat(
                        ui,
                        here,
                        ph::FLAG_PENNANT,
                        "Marque d'image",
                        "Une image de storyboard de plus dans le plan, à l'image courante (vignettes, PDF). Clic : poser ou retirer.",
                        "marque.basculer",
                    );
                    if resp.clicked() {
                        self.toggle_mark();
                    }
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

    /// En-tête des calques (13b, `layerheads.rs`), puis les noms des lignes caméra et son.
    fn timeline_names(&mut self, ui: &mut egui::Ui) {
        let rows_h = rows_height(&rows_of(self.layers(), &self.doc.groups));
        let sound_h = self.sound_row_h();
        let r = self.timeline_heads(ui, HEADER_H, CAM_H + sound_h);
        let painter = ui.painter_at(r);
        let visuals = ui.visuals();
        {
            let y = r.top() + HEADER_H + rows_h;
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
    }

    fn timeline_grid(&mut self, ui: &mut egui::Ui) {
        let count = self.frame_count();
        // Lignes des calques, de hauteurs différentes (réduites ou complètes, 13b).
        let rows = rows_of(self.layers(), &self.doc.groups);
        let rows_h = rows_height(&rows);
        let visible_cols = (ui.available_width() / CELL_W) as usize + 1;
        let cols = (count + 48).max(self.cur_frame + 24).max(visible_cols);
        let sound_h = self.sound_row_h();
        let size = vec2(cols as f32 * CELL_W, HEADER_H + rows_h + CAM_H + sound_h);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let r = resp.rect;
        let visuals = ui.visuals().clone();
        let grid_top = r.top() + HEADER_H;
        // Règle des images.
        painter.rect_filled(Rect::from_min_size(r.min, vec2(size.x, HEADER_H)), 0.0, theme::REGLE);

        // Images clés de la caméra : losanges (ronds si le mouvement y est adouci), reliés par un
        // trait là où la caméra bouge.
        {
            let y = grid_top + rows_h;
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
            for (i, k) in keys.iter().enumerate() {
                let c = pos2(cx(k.frame), row.center().y);
                // La clé que l'on fait glisser : cerclée de blanc.
                let outline = if self.is_dragged_camera_key(self.cur_panel, i) {
                    Stroke::new(1.5_f32, Color32::WHITE)
                } else {
                    Stroke::new(1.0_f32, Color32::from_black_alpha(140))
                };
                if k.smooth {
                    painter.circle(c, 4.5, CAM_KEY, outline);
                } else {
                    let d = 5.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)],
                        CAM_KEY,
                        outline,
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
        for row in &rows {
            let li = row.li;
            let rect = Rect::from_min_size(pos2(r.left(), grid_top + row.top), vec2(size.x, row.h)).shrink2(vec2(0.0, 1.0));
            let bg = if li == self.cur_layer {
                visuals.selection.bg_fill.gamma_multiply(0.35)
            } else if self.is_chosen_layer(li) {
                visuals.selection.bg_fill.gamma_multiply(0.18)
            } else if self.layers()[li].folder {
                // Dossier (13b-5) : pas de dessin, une rangée plus sombre.
                theme::RANGEE_SOMBRE
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

        // Marques d'image (13a) : un triangle dans la règle et un trait au début de l'image.
        for m in &self.panel().marks {
            let x = r.left() + m.frame as f32 * CELL_W;
            painter.line_segment([pos2(x, grid_top), pos2(x, r.bottom())], Stroke::new(1.5_f32, theme::MARQUE));
            painter.add(egui::Shape::convex_polygon(
                vec![pos2(x, r.top() + 2.0), pos2(x + 7.0, r.top() + 2.0), pos2(x, r.top() + HEADER_H - 2.0)],
                theme::MARQUE,
                Stroke::NONE,
            ));
        }

        // Dessins : un bloc par exposition, pastille au début du dessin.
        for row in &rows {
            let layer = &self.layers()[row.li];
            let y = grid_top + row.top;
            // Avant la première image et après la dernière (pré / post-comportement, 13c) : les
            // images montrées, en blocs pâles bordés.
            let first = layer.frames.iter().position(Option::is_some);
            let last = layer.frames.iter().rposition(Option::is_some);
            if let (Some(first), Some(last)) = (first, last) {
                let base = match layer.kind {
                    LayerKind::Vector => theme::DESSIN_VECTORIEL,
                    LayerKind::Bitmap => theme::DESSIN_BITMAP,
                };
                let ranges = [(0, first), (last + 1, cols)];
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
                                pos2(r.left() + f as f32 * CELL_W + 1.0, y + 6.0),
                                pos2(r.left() + g as f32 * CELL_W - 1.0, y + row.h - 6.0),
                            );
                            painter.rect(block, 3.0, base.gamma_multiply(0.18), Stroke::new(1.0_f32, base.gamma_multiply(0.7)), StrokeKind::Inside);
                        }
                        f = g;
                    }
                }
            }
            let mut f = 0;
            while f < layer.frames.len() {
                let (start, end) = layer.run_bounds(f);
                if let Some(id) = layer.frames[start] {
                    let block = Rect::from_min_max(
                        pos2(r.left() + start as f32 * CELL_W + 1.0, y + 4.0),
                        pos2(r.left() + end as f32 * CELL_W - 1.0, y + row.h - 4.0),
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
                    // Nom d'instance (13c) dans le bloc, sinon le trait de l'exposition.
                    match layer.names.get(&id) {
                        Some(name) => {
                            painter.with_clip_rect(block.shrink(1.0)).text(
                                pos2(block.left() + CELL_W - 1.0, block.center().y),
                                Align2::LEFT_CENTER,
                                name,
                                FontId::proportional(10.0),
                                theme::FOND,
                            );
                        }
                        None if end - start > 1 => {
                            painter.line_segment(
                                [
                                    pos2(block.left() + CELL_W - 1.0, block.center().y),
                                    pos2(block.right() - 4.0, block.center().y),
                                ],
                                Stroke::new(1.0_f32, theme::ENTETE),
                            );
                        }
                        None => {}
                    }
                }
                f = end;
            }
        }

        // Double-clic sur un dessin : renommer son instance (13c) ; le champ se pose sur le bloc.
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let col = ((p.x - r.left()) / CELL_W).floor().max(0.0) as usize;
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
                Some(Rect::from_min_size(pos2(r.left() + start as f32 * CELL_W, grid_top + row.top + (row.h - 20.0) * 0.5), vec2(120.0, 20.0)))
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

        // Clés de caméra : on les prend et les fait glisser le long de leur rangée (12e).
        let cam_row = Rect::from_min_size(pos2(r.left(), grid_top + rows_h), vec2(size.x, CAM_H));
        let key_x = |f: usize| r.left() + (f as f32 + 0.5) * CELL_W;
        let key_under = |app: &App, p: Pos2| {
            cam_row.contains(p).then(|| crate::camtools::camera_key_at(&app.panel().camera, p.x, key_x, CELL_W * 0.5 + 2.0)).flatten()
        };
        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin());
            if let Some(k) = origin.and_then(|p| key_under(self, p)) {
                self.begin_camera_key_drag(self.cur_panel, k);
            }
        }
        // Tant qu'on tient une clé, la tête de lecture ne bouge pas.
        let holding_key = self.cam_key_drag.is_some();
        if holding_key {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                self.drag_camera_key(((p.x - r.left()) / CELL_W).floor().max(0.0) as usize);
            }
            if resp.drag_stopped() {
                self.end_camera_key_drag();
            }
        } else if resp.hover_pos().is_some_and(|p| key_under(self, p).is_some()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        // Clic et glisser : choisir l'image (et le calque au clic). Avec Maj : choisir les images
        // entre l'image courante et celle sous la souris, sur les calques choisis (13c-2).
        if resp.is_pointer_button_down_on() {
            self.images_focus = true;
            self.panel_focus = false;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        // Glisser les images choisies (13c-3) : on les prend dans leur cadre ; lâchées sans
        // touche elles sont déplacées, avec Ctrl copiées, avec Maj insérées.
        let col_of = |x: f32| ((x - r.left()) / CELL_W).floor().max(0.0) as usize;
        if resp.drag_started() && !holding_key {
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
                let mode = if m.command {
                    crate::images::DropMode::Copy
                } else if m.shift {
                    crate::images::DropMode::Insert
                } else {
                    crate::images::DropMode::Move
                };
                self.drop_images(c1 as isize - c0 as isize, mode);
            }
        }
        if (resp.clicked() || resp.dragged()) && !holding_key && self.image_drag.is_none() && !resp.drag_stopped() {
            if let Some(p) = resp.interact_pointer_pos() {
                let col = ((p.x - r.left()) / CELL_W).floor().max(0.0) as usize;
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

        // Images choisies : un cadre blanc sur les calques choisis.
        if let Some((a, b)) = self.frame_sel {
            for row in rows.iter().filter(|row| self.is_chosen_layer(row.li) && !self.layers()[row.li].folder) {
                let sel = Rect::from_min_max(
                    pos2(r.left() + a as f32 * CELL_W, grid_top + row.top + 1.0),
                    pos2(r.left() + b as f32 * CELL_W, grid_top + row.top + row.h - 1.0),
                );
                painter.rect(sel, 2.0, Color32::from_white_alpha(28), Stroke::new(1.5_f32, Color32::WHITE), StrokeKind::Inside);
                // Pendant un glisser : là où elles vont se poser.
                if let Some((c0, c1)) = self.image_drag.filter(|(c0, c1)| c0 != c1) {
                    let dx = (c1 as f32 - c0 as f32) * CELL_W;
                    let ghost = sel.translate(vec2(dx, 0.0));
                    painter.rect(ghost, 2.0, theme::MARQUE.gamma_multiply(0.25), Stroke::new(1.5_f32, theme::MARQUE), StrokeKind::Inside);
                }
            }
        }

        if self.follow_frame {
            let row = rows.iter().find(|r| r.li == self.cur_layer).copied().unwrap_or(Row { li: 0, top: 0.0, h: ROW_H });
            let target = Rect::from_min_size(pos2(cx - CELL_W * 2.0, grid_top + row.top), vec2(CELL_W * 5.0, row.h));
            ui.scroll_to_rect(target, None);
            self.follow_frame = false;
        }
    }
}
