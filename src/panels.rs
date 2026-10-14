use eframe::egui::{self, vec2, Align, Align2, Color32, Layout, RichText, Sense, Stroke, StrokeKind};

use crate::app::{App, Mode, ShapeKind, Tool};
use crate::brush::BrushSettings;
use crate::document::{Document, LayerKind};

const PRESETS: [(&str, usize, usize); 5] = [
    ("HD 1920×1080", 1920, 1080),
    ("1280×720", 1280, 720),
    ("4K 3840×2160", 3840, 2160),
    ("Carré 1080", 1080, 1080),
    ("Vertical 1080×1920", 1080, 1920),
];

/// Gestes à la souris et au stylet (hors fichier de raccourcis).
const MOUSE_GESTURES: [(&str, &str); 7] = [
    ("Alt + clic", "prélever une couleur"),
    ("Molette", "zoom autour du curseur"),
    ("Maj + molette", "rotation de la vue"),
    ("Bouton du milieu + glisser", "déplacer la vue"),
    ("Double-clic sur un panneau", "entrer dans le panneau pour l'animer"),
    ("Sélection : glisser dans le vide", "lasso (Maj : ajouter / retirer)"),
    ("Sélection : poignées", "échelle (Maj : proportionnelle), rotation (Maj : par 15°)"),
];

fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let clicked = ui
        .add(egui::Button::new(label).shortcut_text(shortcut))
        .clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

fn brush_ui(ui: &mut egui::Ui, s: &mut BrushSettings, vector: bool) {
    ui.add(egui::Slider::new(&mut s.size, 0.5..=400.0).logarithmic(true).text("Taille"));
    ui.add(egui::Slider::new(&mut s.opacity, 0.0..=1.0).text("Opacité"));
    if !s.aliased || vector {
        ui.add(egui::Slider::new(&mut s.hardness, 0.0..=1.0).text("Dureté"));
    }
    ui.add(egui::Slider::new(&mut s.spacing, 0.02..=1.0).text("Espacement"));
    ui.add(egui::Slider::new(&mut s.smoothing, 0.0..=0.95).text("Lissage"));
    ui.checkbox(&mut s.pressure_size, "Taille selon la pression");
    ui.checkbox(&mut s.pressure_opacity, "Opacité selon la pression");
    if !vector {
        ui.checkbox(&mut s.aliased, "Pixels nets (sans anticrénelage)");
    }
}

impl App {
    /// Entrée de menu reliée à une action ; le raccourci affiché vient du fichier de raccourcis.
    fn action_item(&mut self, ui: &mut egui::Ui, text: &str, action: &str) {
        let shortcut = self.keymap.label(action);
        if menu_item(ui, text, &shortcut) {
            self.run_action(action);
        }
    }

    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Fichier", |ui| {
                self.action_item(ui, "Nouveau…", "fichier.nouveau");
                self.action_item(ui, "Ouvrir…", "fichier.ouvrir");
                self.action_item(ui, "Enregistrer", "fichier.enregistrer");
                self.action_item(ui, "Enregistrer sous…", "fichier.enregistrer_sous");
                ui.separator();
                if menu_item(ui, "Importer une image (calque bitmap)…", "") {
                    self.import_image();
                }
                ui.separator();
                if menu_item(ui, "Exporter l'image courante (PNG)…", "") {
                    self.export_frame();
                }
                if menu_item(ui, "Exporter le panneau (séquence PNG)…", "") {
                    self.export_panel_sequence();
                }
                if menu_item(ui, "Exporter tout le montage (séquence PNG)…", "") {
                    self.export_montage_sequence();
                }
                if menu_item(ui, "Exporter le storyboard (page HTML + planches)…", "") {
                    self.export_storyboard();
                }
                ui.checkbox(&mut self.export_transparent, "Séquences sans le papier (fond transparent)");
                ui.separator();
                self.action_item(ui, "Quitter", "fichier.quitter");
            });
            ui.menu_button("Édition", |ui| {
                self.action_item(ui, "Annuler", "edition.annuler");
                self.action_item(ui, "Rétablir", "edition.retablir");
                ui.separator();
                self.action_item(ui, "Copier la sélection", "edition.copier");
                self.action_item(ui, "Couper la sélection", "edition.couper");
                self.action_item(ui, "Coller", "edition.coller");
                self.action_item(ui, "Tout sélectionner", "edition.tout_selectionner");
                self.action_item(ui, "Tout désélectionner", "edition.tout_deselectionner");
                ui.separator();
                self.action_item(ui, "Supprimer la sélection ou effacer le dessin", "edition.supprimer");
            });
            ui.menu_button("Storyboard", |ui| {
                self.action_item(ui, "Nouveau panneau", "panneau.nouveau");
                self.action_item(ui, "Nouvelle scène", "scene.nouvelle");
                self.action_item(ui, "Nouvelle séquence", "sequence.nouvelle");
                ui.separator();
                self.action_item(ui, "Dupliquer le panneau", "panneau.dupliquer");
                self.action_item(ui, "Supprimer le panneau", "panneau.supprimer");
                self.action_item(ui, "Allonger le panneau d'une image", "panneau.ajouter_image");
                self.action_item(ui, "Raccourcir le panneau d'une image", "panneau.retirer_image");
                ui.separator();
                self.action_item(ui, "Couper le panneau à l'image courante", "panneau.couper");
                self.action_item(ui, "Réunir le panneau et le suivant", "panneau.fusionner_suivant");
                ui.separator();
                self.action_item(ui, "Dupliquer la scène", "scene.dupliquer");
                self.action_item(ui, "Supprimer la scène", "scene.supprimer");
                self.action_item(ui, "Couper la scène au panneau courant", "scene.couper");
                self.action_item(ui, "Joindre la scène à la précédente", "scene.joindre_precedente");
                ui.separator();
                self.action_item(ui, "Panneau précédent", "panneau.precedent");
                self.action_item(ui, "Panneau suivant", "panneau.suivant");
                self.action_item(ui, "Scène précédente", "scene.precedente");
                self.action_item(ui, "Scène suivante", "scene.suivante");
                ui.separator();
                let label = match self.mode {
                    Mode::Storyboard => "Animer ce panneau",
                    Mode::Animation => "Retour au storyboard",
                };
                self.action_item(ui, label, "mode.basculer");
                self.action_item(ui, "Vue dessin / vignettes", "vue.planches");
            });
            ui.menu_button("Son", |ui| self.sound_menu(ui));
            ui.menu_button("Calques", |ui| {
                self.action_item(ui, "Nouveau calque vectoriel", "calque.nouveau_vectoriel");
                self.action_item(ui, "Nouveau calque bitmap", "calque.nouveau_bitmap");
                self.action_item(ui, "Supprimer le calque", "calque.supprimer");
                ui.separator();
                self.action_item(ui, "Copier le calque", "calque.copier");
                self.action_item(ui, "Coller le calque", "calque.coller");
                ui.separator();
                let lock = if self.layer().locked { "Déverrouiller le calque" } else { "Verrouiller le calque" };
                self.action_item(ui, lock, "calque.verrouiller");
                self.action_item(ui, "Fusionner avec le calque du dessous", "calque.fusionner_dessous");
                ui.separator();
                self.action_item(ui, "Calque du dessus", "calque.dessus");
                self.action_item(ui, "Calque du dessous", "calque.dessous");
            });
            ui.menu_button("Animation", |ui| {
                self.action_item(ui, "Lecture / pause", "lecture.basculer");
                self.action_item(ui, "Image précédente", "image.precedente");
                self.action_item(ui, "Image suivante", "image.suivante");
                self.action_item(ui, "Première image du panneau", "image.premiere");
                self.action_item(ui, "Dernière image du panneau", "image.derniere");
                ui.separator();
                self.action_item(ui, "Nouveau dessin", "dessin.nouveau");
                self.action_item(ui, "Dupliquer le dessin", "dessin.dupliquer");
                self.action_item(ui, "Allonger l'exposition", "exposition.allonger");
                self.action_item(ui, "Raccourcir l'exposition", "exposition.raccourcir");
                self.action_item(ui, "Retirer le dessin de la timeline", "dessin.supprimer");
            });
            ui.menu_button("Affichage", |ui| {
                self.action_item(ui, "Ajuster à la fenêtre", "vue.ajuster");
                self.action_item(ui, "Zoom 100 %", "vue.reinit_zoom");
                self.action_item(ui, "Zoom avant", "vue.zoom_avant");
                self.action_item(ui, "Zoom arrière", "vue.zoom_arriere");
                self.action_item(ui, "Tourner (sens horaire)", "vue.rotation_horaire");
                self.action_item(ui, "Tourner (sens antihoraire)", "vue.rotation_antihoraire");
                self.action_item(ui, "Remettre la vue droite", "vue.reinit_rotation");
                self.action_item(ui, "Miroir", "vue.miroir");
                self.action_item(ui, "Pelure d'oignon", "vue.pelure");
                self.action_item(ui, "Table lumineuse", "vue.table_lumineuse");
                ui.separator();
                self.action_item(ui, "Vue à travers la caméra", "camera.vue");
                self.action_item(ui, "Estomper le hors-champ", "camera.masque");
                self.action_item(ui, "Réinitialiser la caméra du panneau", "camera.reinitialiser");
                ui.separator();
                self.action_item(ui, "Guide : afficher et utiliser", "guide.activer");
                self.action_item(ui, "Guide : aligner les traits", "guide.aligner");
                self.action_item(ui, "Guide : verrouiller", "guide.verrouiller");
                self.action_item(ui, "Guide : lignes de référence", "guide.lignes_reference");
                ui.separator();
                self.action_item(ui, "Plein écran", "vue.plein_ecran");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Couleur du papier");
                    let mut paper = self.doc.paper;
                    let resp = ui.color_edit_button_srgb(&mut paper);
                    if self.edited(&resp) {
                        self.doc.paper = paper;
                        self.renderer.invalidate_all();
                    }
                });
            });
            ui.menu_button("Aide", |ui| {
                self.action_item(ui, "Raccourcis clavier", "aide.raccourcis");
                if menu_item(ui, "Modifier les raccourcis…", "") {
                    self.edit_keymap();
                }
                if menu_item(ui, "Recharger les raccourcis", "") {
                    self.reload_keymap();
                }
                ui.separator();
                ui.label(RichText::new(format!("Affichage : {}", self.display_info)).small().weak());
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let animation = ui
                    .selectable_label(self.mode == Mode::Animation, RichText::new(" Animation ").strong())
                    .on_hover_text("Animer le panneau courant (Tab)");
                if animation.clicked() && self.mode != Mode::Animation {
                    self.set_mode(Mode::Animation);
                }
                let board = ui
                    .selectable_label(self.mode == Mode::Storyboard, RichText::new(" Storyboard ").strong())
                    .on_hover_text("Monter les panneaux (Tab)");
                if board.clicked() && self.mode != Mode::Storyboard {
                    self.set_mode(Mode::Storyboard);
                }
            });
        });
    }

    pub(crate) fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Outils");
        egui::Grid::new("grille_outils")
            .num_columns(2)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (i, tool) in Tool::ALL.into_iter().enumerate() {
                    let shortcut = self.keymap.label(tool.action());
                    let hint = if shortcut.is_empty() {
                        "Pas de raccourci".to_string()
                    } else {
                        format!("Raccourci : {shortcut}")
                    };
                    let mut text = egui::text::LayoutJob::default();
                    let style = ui.style().clone();
                    RichText::new(tool.label()).append_to(&mut text, &style, egui::FontSelection::Default, Align::Center);
                    RichText::new(format!("  {shortcut}")).small().weak().append_to(
                        &mut text,
                        &style,
                        egui::FontSelection::Default,
                        Align::Center,
                    );
                    if ui
                        .add_sized([108.0, 26.0], egui::SelectableLabel::new(self.tool == tool, text))
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.end_stroke();
                        self.tool = tool;
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });

        ui.separator();
        ui.heading("Couleur");
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size = vec2(60.0, 26.0);
            ui.color_edit_button_srgb(&mut self.color);
            if ui.button("+ Nuancier").clicked() && !self.swatches.contains(&self.color) {
                self.swatches.push(self.color);
            }
        });
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            for (i, c) in self.swatches.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(19.0, 19.0), Sense::click());
                ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
                let border = if *c == self.color {
                    Stroke::new(2.0_f32, Color32::WHITE)
                } else {
                    Stroke::new(1.0_f32, Color32::from_gray(70))
                };
                ui.painter().rect_stroke(rect, 3.0, border, StrokeKind::Inside);
                let resp = resp.on_hover_text("Clic : choisir · clic droit : retirer");
                if resp.clicked() {
                    self.color = *c;
                }
                if resp.secondary_clicked() {
                    remove = Some(i);
                }
            }
        });
        if let Some(i) = remove {
            self.swatches.remove(i);
        }

        ui.separator();
        let vector = self.layer().kind == LayerKind::Vector;
        ui.horizontal(|ui| {
            ui.heading(self.tool.label());
            ui.label(
                RichText::new(if vector { "calque vectoriel" } else { "calque bitmap" })
                    .small()
                    .weak(),
            );
        });
        match self.tool {
            Tool::Brush => {
                brush_ui(ui, &mut self.brush, vector);
                if vector {
                    note(ui, "Le trait reste modifiable avec l'outil Sélection (V).");
                }
            }
            Tool::Pencil => {
                brush_ui(ui, &mut self.pencil, vector);
                if vector {
                    note(ui, "Le trait reste modifiable avec l'outil Sélection (V).");
                }
            }
            Tool::Eraser => {
                brush_ui(ui, &mut self.eraser, vector);
                if vector {
                    ui.checkbox(&mut self.vector_erase_whole, "Effacer des traits entiers");
                    note(
                        ui,
                        "Sur un calque vectoriel, la gomme coupe les traits. Les zones remplies \
                         s'effacent en mode « traits entiers » ou par la sélection.",
                    );
                }
            }
            Tool::Fill => {
                ui.add(egui::Slider::new(&mut self.fill.tolerance, 0.0..=1.0).text("Tolérance"));
                ui.add(egui::Slider::new(&mut self.fill.expand, 0..=4).text("Débord (px)"));
                ui.label("Contours pris sur :");
                ui.radio_value(&mut self.fill.sample_all, false, "le calque courant");
                ui.radio_value(&mut self.fill.sample_all, true, "tous les calques visibles");
                if vector {
                    note(ui, "Sur un trait ou une zone déjà peinte : change sa couleur.");
                } else {
                    note(
                        ui,
                        "Astuce : trait sur un calque, couleurs sur un calque en dessous, \
                         contours pris sur tous les calques.",
                    );
                }
            }
            Tool::Shape => {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.shape.kind, ShapeKind::Line, "Ligne");
                    ui.selectable_value(&mut self.shape.kind, ShapeKind::Rect, "Rectangle");
                    ui.selectable_value(&mut self.shape.kind, ShapeKind::Ellipse, "Ellipse");
                });
                ui.add(egui::Slider::new(&mut self.shape.width, 0.5..=100.0).logarithmic(true).text("Épaisseur"));
                if self.shape.kind != ShapeKind::Line {
                    ui.checkbox(&mut self.shape.filled, "Forme pleine");
                }
                note(ui, "Maj : carré, cercle, ou angle par pas de 15°.");
            }
            Tool::Select => self.selection_ui(ui),
            Tool::Picker => {
                ui.label("Clique sur le dessin pour prélever une couleur.");
                note(ui, "Avec les autres outils : Alt + clic.");
            }
            Tool::Pan => {
                ui.label("Glisse pour déplacer la vue.");
                note(ui, "Sinon : Espace + glisser, ou bouton du milieu.");
            }
            Tool::Camera => {
                self.camera_fields(ui);
                let was = self.camera_view;
                ui.checkbox(&mut self.camera_view, "Vue à travers la caméra");
                if was && !self.camera_view {
                    self.fit_pending = true;
                }
                ui.checkbox(&mut self.camera_mask, "Estomper le hors-champ");
                note(
                    ui,
                    "Glisser dans le cadre : déplacer (Maj : à l'horizontale ou à la verticale). Coin : \
                     zoomer. Poignée ronde : tourner (Maj : par 15°). La caméra appartient au panneau.",
                );
            }
        }
        if self.current_brush().is_some_and(|b| b.pressure_size || b.pressure_opacity) {
            note(ui, "La pression vient du stylet (Windows Ink). À la souris, elle vaut 100 %.");
        }

        ui.separator();
        ui.heading("Vue");
        ui.horizontal(|ui| {
            if ui.button("Ajuster").clicked() {
                self.fit_pending = true;
            }
            if ui.button("100 %").clicked() {
                self.zoom_actual();
            }
            if ui.selectable_label(self.view.flip, "Miroir").clicked() {
                self.flip_view();
            }
        });
        let mut degrees = self.view.rotation.to_degrees();
        if ui
            .add(egui::Slider::new(&mut degrees, -180.0..=180.0).text("Rotation").suffix("°"))
            .changed()
        {
            let delta = degrees.to_radians() - self.view.rotation;
            let (rect, size) = (self.canvas_rect, self.canvas_size());
            self.view.rotate_around(rect, size, rect.center(), delta);
        }
        ui.checkbox(&mut self.light_table, "Table lumineuse")
            .on_hover_text("Les autres calques du panneau sont estompés ; le calque courant reste net.");
        if self.light_table {
            ui.add(
                egui::Slider::new(&mut self.light_table_strength, 0.05..=0.8)
                    .text("autres")
                    .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
            );
        }

        ui.separator();
        ui.heading("Guides");
        self.guides_ui(ui);
    }

    /// Options de l'outil Sélection sur un calque bitmap.
    fn bitmap_selection_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.select_rect, false, "Lasso");
            ui.selectable_value(&mut self.select_rect, true, "Rectangle");
        });
        let has = self.bsel.is_some();
        ui.label(match self.bsel.as_ref().map(|s| s.lifted.is_some()) {
            None => "Rien de sélectionné.",
            Some(false) => "Zone sélectionnée.",
            Some(true) => "Zone soulevée : elle sera posée en quittant la sélection.",
        });
        ui.add_enabled_ui(has, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Miroir H").clicked() {
                    self.bitmap_flip(true);
                }
                if ui.button("Miroir V").clicked() {
                    self.bitmap_flip(false);
                }
                if ui.button("Copier").clicked() {
                    self.bitmap_copy();
                }
                if ui.button("Couper").clicked() {
                    self.bitmap_cut();
                }
                if ui.button("Effacer dedans").clicked() {
                    self.bitmap_erase(true);
                }
                if ui.button("Effacer autour").clicked() {
                    self.bitmap_erase(false);
                }
                if ui.button("Poser").on_hover_text("Repose la zone et quitte la sélection (Échap)").clicked() {
                    self.drop_bitmap_selection();
                }
            });
        });
        ui.horizontal(|ui| {
            if ui.button("Tout sélectionner").clicked() {
                self.bitmap_select_all();
            }
            if ui
                .add_enabled(self.pixel_clipboard.is_some(), egui::Button::new("Coller"))
                .clicked()
            {
                self.bitmap_paste();
            }
        });
        note(
            ui,
            "Glisser : lasso ou rectangle (Maj : ajouter). Dans la zone : déplacer. Poignées : \
             échelle (Maj : proportionnelle), rotation (Maj : par 15°). Échap ou autre outil : poser.",
        );
    }

    fn selection_ui(&mut self, ui: &mut egui::Ui) {
        if self.layer().kind != LayerKind::Vector {
            self.bitmap_selection_ui(ui);
            return;
        }
        let n = self.vsel.as_ref().map_or(0, |s| s.items.len());
        ui.label(if n == 0 {
            "Rien de sélectionné.".to_string()
        } else {
            format!("{n} élément(s) sélectionné(s).")
        });
        ui.add_enabled_ui(n > 0, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Couleur courante").clicked() {
                    self.recolor_selection();
                }
                if ui.button("Plus épais").clicked() {
                    self.scale_selection_width(1.25);
                }
                if ui.button("Plus fin").clicked() {
                    self.scale_selection_width(0.8);
                }
                if ui.button("Miroir H").clicked() {
                    self.flip_selection(true);
                }
                if ui.button("Miroir V").clicked() {
                    self.flip_selection(false);
                }
                if ui.button("Devant").clicked() {
                    self.order_selection(true);
                }
                if ui.button("Derrière").clicked() {
                    self.order_selection(false);
                }
                if ui.button("Copier").clicked() {
                    self.copy_selection();
                }
                if ui.button("Supprimer").clicked() {
                    self.delete_selection();
                }
            });
        });
        ui.horizontal(|ui| {
            if ui.button("Tout sélectionner").clicked() {
                self.select_all();
            }
            if ui
                .add_enabled(!self.clipboard.is_empty(), egui::Button::new("Coller"))
                .clicked()
            {
                self.paste();
            }
        });
        note(
            ui,
            "Clic : un élément. Glisser dans le vide : lasso. Maj : ajouter / retirer. \
             Poignées : échelle (Maj : proportionnelle) et rotation (Maj : par 15°).",
        );
    }

    pub(crate) fn right_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("panneau_droit")
            .show(ui, |ui| {
                self.panel_inspector(ui);
                ui.separator();
                self.layers_panel(ui);
            });
    }

    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Calques");
        ui.horizontal(|ui| {
            if ui.button("+ Vectoriel").on_hover_text("Traits modifiables, façon Storyboard Pro").clicked() {
                self.add_layer(LayerKind::Vector);
            }
            if ui.button("+ Bitmap").on_hover_text("Peinture en pixels, façon TVPaint").clicked() {
                self.add_layer(LayerKind::Bitmap);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Dupliquer").clicked() {
                self.duplicate_layer();
            }
            if ui.button("Supprimer").clicked() {
                self.delete_layer();
            }
            if ui.button("Monter").clicked() {
                self.move_layer(true);
            }
            if ui.button("Descendre").clicked() {
                self.move_layer(false);
            }
        });
        let hint = |app: &App, text: &str, action: &str| {
            let key = app.keymap.label(action);
            if key.is_empty() {
                text.to_string()
            } else {
                format!("{text} ({key})")
            }
        };
        ui.horizontal(|ui| {
            if ui.button("Copier").on_hover_text(hint(self, "Copier le calque", "calque.copier")).clicked() {
                self.copy_layer();
            }
            if ui
                .add_enabled(self.layer_clipboard.is_some(), egui::Button::new("Coller"))
                .on_hover_text(hint(self, "Coller le calque copié au-dessus du calque courant", "calque.coller"))
                .clicked()
            {
                self.paste_layer();
            }
            if ui
                .add_enabled(self.cur_layer > 0, egui::Button::new("Fusionner"))
                .on_hover_text(hint(self, "Fusionner avec le calque du dessous", "calque.fusionner_dessous"))
                .clicked()
            {
                self.merge_layer_down();
            }
        });
        ui.add_space(4.0);

        let mut select = None;
        let mut toggle = None;
        let mut lock = None;
        let lock_hint = hint(self, "Verrouiller / déverrouiller : on ne peut plus dessiner sur le calque", "calque.verrouiller");
        for li in (0..self.layers().len()).rev() {
            let layer = &self.layers()[li];
            ui.horizontal(|ui| {
                let eye = if layer.visible { "👁" } else { "  " };
                if ui
                    .add(egui::Button::new(eye).min_size(vec2(26.0, 22.0)))
                    .on_hover_text("Afficher / masquer")
                    .clicked()
                {
                    toggle = Some(li);
                }
                let padlock = if layer.locked { "🔒" } else { "  " };
                if ui
                    .add(egui::Button::new(padlock).min_size(vec2(26.0, 22.0)))
                    .on_hover_text(&lock_hint)
                    .clicked()
                {
                    lock = Some(li);
                }
                ui.label(RichText::new(layer.kind.badge()).monospace().weak())
                    .on_hover_text(match layer.kind {
                        LayerKind::Vector => "Calque vectoriel",
                        LayerKind::Bitmap => "Calque bitmap",
                    });
                let text = if layer.visible {
                    RichText::new(&layer.name)
                } else {
                    RichText::new(&layer.name).weak()
                };
                if ui
                    .add_sized(
                        [ui.available_width(), 22.0],
                        egui::SelectableLabel::new(li == self.cur_layer, text),
                    )
                    .clicked()
                {
                    select = Some(li);
                }
            });
        }
        if let Some(li) = toggle {
            self.record_step();
            let layer = &mut self.layers_mut()[li];
            layer.visible = !layer.visible;
        }
        if let Some(li) = lock {
            self.toggle_layer_lock(li);
        }
        if let Some(li) = select {
            self.select_layer(li);
        }

        // Réglages du calque courant : on modifie des copies, `edited` enregistre l'annulation.
        ui.separator();
        let li = self.cur_layer;
        let layer = &self.layers()[li];
        let (mut name, mut opacity, mut visible) = (layer.name.clone(), layer.opacity, layer.visible);
        ui.horizontal(|ui| {
            ui.label("Nom");
            let resp = ui.text_edit_singleline(&mut name);
            if self.edited(&resp) {
                self.layers_mut()[li].name = name;
            }
        });
        let resp = ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Opacité"));
        if self.edited(&resp) {
            self.layers_mut()[li].opacity = opacity;
        }
        let resp = ui.checkbox(&mut visible, "Visible");
        if self.edited(&resp) {
            self.layers_mut()[li].visible = visible;
        }
    }

    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let where_ = match self.mode {
                Mode::Storyboard => format!("Storyboard · {}", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
                Mode::Animation => format!("Animation · {}", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
            };
            ui.label(RichText::new(where_).strong());
            ui.separator();
            ui.label(format!(
                "Cadre {}×{} · {} i/s",
                self.doc.frame[0], self.doc.frame[1], self.doc.fps
            ))
            .on_hover_text(format!(
                "Image finale {}×{} px ; feuille de dessin {}×{} px",
                self.doc.frame[0], self.doc.frame[1], self.doc.width, self.doc.height
            ));
            ui.separator();
            ui.label(format!("Zoom {:.0} %", self.view.zoom * self.ppp * 100.0));
            let degrees = self.view.rotation.to_degrees();
            if degrees.abs() > 0.05 {
                ui.label(format!("Rotation {degrees:.0}°"));
            }
            if self.view.flip {
                ui.label("Miroir");
            }
            if let Some(p) = self.hover_canvas {
                ui.separator();
                ui.label(format!("x {:.0}  y {:.0}", p.x, p.y));
            }
            ui.separator();
            ui.label(&self.status);
        });
    }

    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut d) = self.new_dialog.take() {
            let mut keep = true;
            let mut create = false;
            egui::Window::new("Nouveau projet")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (label, w, h) in PRESETS {
                            if ui.selectable_label(d.width == w && d.height == h, label).clicked() {
                                d.width = w;
                                d.height = h;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    // Le cadre : taille de l'image finale. La feuille de dessin fait le double, pour
                    // dessiner hors champ (limite : 4096 px de cadre, soit 8192 px de feuille).
                    egui::Grid::new("grille_nouveau").num_columns(2).show(ui, |ui| {
                        ui.label("Largeur du cadre");
                        ui.add(egui::DragValue::new(&mut d.width).range(16..=4096).suffix(" px"));
                        ui.end_row();
                        ui.label("Hauteur du cadre");
                        ui.add(egui::DragValue::new(&mut d.height).range(16..=4096).suffix(" px"));
                        ui.end_row();
                        ui.label("Cadence");
                        ui.add(egui::DragValue::new(&mut d.fps).range(1..=120).suffix(" i/s"));
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Créer").clicked() {
                            create = true;
                        }
                        if ui.button("Annuler").clicked() {
                            keep = false;
                        }
                    });
                });
            if create {
                self.load_doc(Document::with_frame(d.width, d.height, d.fps));
                self.status = format!(
                    "Nouveau projet : cadre {}×{} à {} i/s, feuille de dessin {}×{}.",
                    d.width,
                    d.height,
                    d.fps,
                    d.width * 2,
                    d.height * 2
                );
            } else if keep {
                self.new_dialog = Some(d);
            }
        }

        if let Some(action) = self.pending.take() {
            let mut choice = None;
            egui::Window::new("Modifications non enregistrées")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Le projet a été modifié depuis le dernier enregistrement.");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Enregistrer").clicked() {
                            choice = Some(0);
                        }
                        if ui.button("Ne pas enregistrer").clicked() {
                            choice = Some(1);
                        }
                        if ui.button("Annuler").clicked() {
                            choice = Some(2);
                        }
                    });
                });
            match choice {
                Some(0) => {
                    if self.save() {
                        self.perform(action);
                    }
                }
                Some(1) => self.perform(action),
                Some(_) => {}
                None => self.pending = Some(action),
            }
        }

        if self.show_help {
            let mut open = true;
            let mut edit = false;
            let mut reload = false;
            egui::Window::new("Raccourcis clavier")
                .open(&mut open)
                .resizable(true)
                .default_size([640.0, 560.0])
                .default_pos(ctx.screen_rect().center() - vec2(320.0, 280.0))
                .show(ctx, |ui| {
                    let file = match &self.keymap.user_file {
                        Some(p) => format!("Fichier : {}", p.display()),
                        None => "Raccourcis par défaut (aucun fichier personnel).".into(),
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(file).small());
                        edit = ui.button("Modifier…").clicked();
                        reload = ui.button("Recharger").clicked();
                    });
                    for w in &self.keymap.warnings {
                        ui.colored_label(Color32::from_rgb(240, 170, 60), format!("⚠ {w}"));
                    }
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("grille_raccourcis")
                            .num_columns(3)
                            .striped(true)
                            .show(ui, |ui| {
                                for b in &self.keymap.bindings {
                                    let info = crate::keymap::action_info(&b.action);
                                    let mut keys = b.combos.iter().map(|c| c.label()).collect::<Vec<_>>();
                                    keys.dedup();
                                    // Un raccourci en réserve peut être un geste qu'on ne sait pas encore lire.
                                    if keys.is_empty() {
                                        keys = b.keys.clone();
                                    }
                                    let keys = if keys.is_empty() { "—".to_string() } else { keys.join("  ou  ") };
                                    let why = if b.reserved { "non implémenté" } else { "désactivé" };
                                    let mut label = info.map_or(b.action.clone(), |i| i.label.to_string());
                                    match b.context {
                                        Some(Mode::Storyboard) => label += " (storyboard)",
                                        Some(Mode::Animation) => label += " (animation)",
                                        None => {}
                                    }
                                    if b.active {
                                        ui.label(RichText::new(keys).strong());
                                        ui.label(label);
                                    } else {
                                        ui.label(RichText::new(keys).weak());
                                        ui.label(RichText::new(format!("{label} — {why}")).weak());
                                    }
                                    ui.label(RichText::new(&b.source).small().weak());
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(RichText::new("Souris et stylet").strong());
                        for (geste, effet) in MOUSE_GESTURES {
                            ui.label(format!("{geste} : {effet}"));
                        }
                    });
                });
            self.show_help = open;
            if edit {
                self.edit_keymap();
            }
            if reload {
                self.reload_keymap();
            }
        }
    }
}
