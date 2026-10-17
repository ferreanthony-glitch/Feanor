use eframe::egui::{self, vec2, Align, Align2, Color32, Layout, RichText};

use crate::app::{App, Mode, Tool};
use crate::brush::BrushSettings;
use crate::document::{Document, LayerKind};

/// Gestes à la souris et au stylet (hors fichier de raccourcis).
pub(crate) const MOUSE_GESTURES: [(&str, &str); 21] = [
    ("Clic droit sur une commande", "lui assigner un raccourci"),
    ("Alt + clic", "prélever une couleur"),
    ("Ctrl tenu sur le dessin", "le lasso le temps de le tenir (sauf avec le tampon de duplication)"),
    ("Touche d'un outil tenue", "l'outil le temps de la tenir, puis retour au précédent (brève : il reste)"),
    ("Molette", "zoom autour du curseur"),
    ("Maj + molette", "rotation de la vue"),
    ("Bouton du milieu + glisser", "déplacer la vue"),
    ("Double-clic sur un plan", "entrer dans le plan pour l'animer"),
    ("Sélection : glisser dans le vide", "lasso (Maj : ajouter / retirer)"),
    ("Sélection : poignées", "échelle (Maj : proportionnelle), rotation (Maj : par 15°)"),
    ("Timeline : glisser sur les cases", "choisir des images et des calques"),
    ("Timeline : glisser des images choisies", "les déplacer (Ctrl : copier, Maj : insérer)"),
    ("Timeline ou montage : barre sous un calque", "décaler tout le calque dans le temps"),
    ("Timeline ou montage : début ou fin d'un calque", "rogner ou allonger (tient le premier / dernier dessin)"),
    ("Timeline : bord droit d'un dessin", "étirer ou raccourcir son exposition (la suite glisse)"),
    ("Montage : limite entre deux dessins d'un calque", "allonger l'un, raccourcir l'autre"),
    ("Montage : milieu d'un dessin d'un calque", "le placer avant ou après un autre (chacun garde sa durée)"),
    ("Montage : double-clic sur la piste d'un calque", "nouveau dessin (clé) à cette image"),
    ("Montage : Ctrl ou Maj + clic sur des dessins", "choisir plusieurs clés (Ctrl : une de plus ou de moins ; Maj : celles d'entre)"),
    ("Timeline : prise au bout de la règle", "changer la durée du plan"),
    ("Échap ou Ctrl+Z pendant un glisser", "abandonner le geste"),
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

/// Réglages d'un pinceau : chaque curseur avec, au bout, l'interrupteur qui le concerne (12d) —
/// la pression du stylet à côté de la taille et de l'opacité, les pixels nets à côté de la dureté.
fn brush_ui(ui: &mut egui::Ui, s: &mut BrushSettings, vector: bool) {
    use crate::bulles::{bulle, interrupteur};
    use egui_phosphor::regular as ph;
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut s.size, 0.5..=400.0).logarithmic(true).text("Taille"));
        interrupteur(ui, &mut s.pressure_size, ph::PEN_NIB, "Taille selon la pression", "Plus on appuie avec le stylet, plus le trait est gros.", "");
    });
    if s.pressure_size {
        let resp = ui.add(egui::Slider::new(&mut s.min_size, 0.0..=1.0).text("Taille mini"));
        bulle(resp, "Taille mini", "Taille quand on effleure, en part de la taille.", "");
    }
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut s.opacity, 0.0..=1.0).text("Opacité"));
        interrupteur(ui, &mut s.pressure_opacity, ph::PEN_NIB, "Opacité selon la pression", "Plus on appuie avec le stylet, plus le trait couvre.", "");
    });
    if s.pressure_opacity {
        let resp = ui.add(egui::Slider::new(&mut s.min_opacity, 0.0..=1.0).text("Opacité mini"));
        bulle(resp, "Opacité mini", "Opacité quand on effleure, en part de l'opacité.", "");
    }
    ui.horizontal(|ui| {
        let resp = ui.add(egui::Slider::new(&mut s.power, 0.01..=1.0).text("Débit"));
        bulle(resp, "Débit", "Peinture déposée à chaque empreinte (le « Power » de TVPaint) ; l'opacité en reste le plafond.", "");
        interrupteur(ui, &mut s.build_up, ph::STACK, "Accumuler", "Repasser dans le même trait fonce, jusqu'à l'opacité. Éteint : le trait reste uniforme.", "");
    });
    ui.horizontal(|ui| {
        if (!s.aliased || vector) && s.profile.is_none() {
            ui.add(egui::Slider::new(&mut s.hardness, 0.0..=1.0).text("Dureté"));
        }
        if !vector {
            interrupteur(ui, &mut s.aliased, ph::CHECKERBOARD, "Pixels nets", "Sans anticrénelage : un bord net, pixel par pixel (sans dureté).", "");
        }
        // Profil de la pointe (moteur de dessin, étape 3) : il part de la dureté réglée.
        let mut on = s.profile.is_some();
        if interrupteur(ui, &mut on, ph::CHART_LINE, "Profil de la pointe", "Régler en courbe la couverture de la pointe, du centre au bord (remplace la dureté).", "").clicked() {
            s.profile = on.then(|| BrushSettings::profile_from_hardness(s.hardness));
        }
    });
    if let Some(profile) = &mut s.profile {
        ui.horizontal(|ui| {
            let resp = crate::dynui::curve_editor(ui, profile, "profil");
            bulle(resp, "Profil de la pointe", "Couverture du centre (à gauche) au bord (à droite). Glisser un point ; double-clic : ajouter ; clic droit : retirer.", "");
            ui.vertical(|ui| {
                let presets: [(&str, &str, &[[f32; 2]]); 3] = [
                    ("Dur", "Plein jusqu'au bord.", &[[0.0, 1.0], [0.9, 1.0], [1.0, 0.0]]),
                    ("Doux", "Plein au centre, chute douce jusqu'au bord.", &[[0.0, 1.0], [0.2, 0.95], [0.5, 0.5], [0.8, 0.05], [1.0, 0.0]]),
                    ("Diffus", "Peu de cœur, un large halo.", &[[0.0, 1.0], [0.15, 0.6], [0.4, 0.25], [0.7, 0.07], [1.0, 0.0]]),
                ];
                for (name, help, pts) in presets {
                    if ui.small_button(name).on_hover_text(help).clicked() {
                        *profile = crate::brush::Curve::from_points(pts);
                    }
                }
            });
        });
    }
    ui.add(egui::Slider::new(&mut s.spacing, 0.02..=1.0).text("Espacement"));
    // Lissage (moteur de dessin, étape 5) : son type, et le réglage qui va avec.
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("lissage_type").selected_text(s.smooth_kind.label()).width(120.0).show_ui(ui, |ui| {
            for k in crate::brush::SmoothKind::ALL {
                ui.selectable_value(&mut s.smooth_kind, k, k.label()).on_hover_text(k.help());
            }
        });
        match s.smooth_kind {
            crate::brush::SmoothKind::Follow => {
                ui.add(egui::Slider::new(&mut s.smoothing, 0.0..=0.95).text("Lissage"));
            }
            crate::brush::SmoothKind::PulledString => {
                ui.add(egui::Slider::new(&mut s.smooth_px, 0.0..=80.0).suffix(" px").text("Corde"));
            }
            crate::brush::SmoothKind::AveragePoints => {
                ui.add(egui::Slider::new(&mut s.smooth_points, 1..=40).text("Points"));
            }
            crate::brush::SmoothKind::AverageDistance => {
                ui.add(egui::Slider::new(&mut s.smooth_px, 0.0..=80.0).suffix(" px").text("Distance"));
            }
        }
    })
    .response
    .on_hover_text(s.smooth_kind.help());
    crate::dynui::shape_ui(ui, s);
    crate::dynui::wet_ui(ui, s, vector);
    crate::dynui::dynamics_ui(ui, s, vector);
}

impl App {
    /// Entrée de menu reliée à une action ; le raccourci affiché vient du fichier de raccourcis.
    /// Clic droit : lui assigner un raccourci.
    pub(crate) fn action_item(&mut self, ui: &mut egui::Ui, text: &str, action: &str) {
        let shortcut = self.keymap.label(action);
        let resp = ui.add(egui::Button::new(text).shortcut_text(&shortcut));
        if resp.clicked() {
            ui.close_menu();
            self.run_action(action);
        } else if resp.secondary_clicked() {
            ui.close_menu();
            self.start_assign(action);
        }
    }

    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Fichier", |ui| {
                self.action_item(ui, "Nouveau…", "fichier.nouveau");
                self.action_item(ui, "Ouvrir…", "fichier.ouvrir");
                self.action_item(ui, "Ouvrir un projet Storyboard Pro (.sboard)…", "fichier.importer_sboard");
                self.action_item(ui, "Enregistrer", "fichier.enregistrer");
                self.action_item(ui, "Enregistrer sous…", "fichier.enregistrer_sous");
                self.action_item(ui, "Nouveau projet à partir des plans choisis", "projet.nouveau_depuis_plans");
                self.action_item(ui, "Réglages du projet (propriétés, résolution, cadence)…", "projet.proprietes");
                self.action_item(ui, "Raccourcis clavier…", "aide.raccourcis");
                ui.separator();
                if menu_item(ui, "Importer une image (calque bitmap)…", "") {
                    self.import_image();
                }
                self.action_item(ui, "Importer une vidéo (calque vidéo)…", "fichier.importer_video");
                self.action_item(ui, "Importer une séquence d'images (calque vidéo)…", "fichier.importer_sequence");
                self.action_item(ui, "Importer un modèle 3D (glTF) comme repère…", "fichier.importer_modele3d");
                ui.separator();
                self.action_item(ui, "Exporter le film (vidéo)…", "fichier.exporter_film");
                self.action_item(ui, "Exporter des images (PNG)…", "fichier.exporter_images");
                self.action_item(ui, "Exporter le son (WAV)…", "fichier.exporter_son");
                self.action_item(ui, "Exporter le storyboard (PDF)…", "fichier.exporter_pdf");
                self.action_item(ui, "Exporter vers Storyboard Pro…", "fichier.exporter_sboard");
                self.action_item(ui, "Exporter l'image courante (PNG)…", "fichier.exporter_image_courante");
                if menu_item(ui, "Exporter le storyboard (page HTML + planches)…", "") {
                    self.export_storyboard();
                }
                ui.checkbox(&mut self.export_transparent, "Images PNG sans le papier (fond transparent)");
                ui.separator();
                self.autosave_menu_ui(ui);
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
                ui.separator();
                // Couleurs et nuanciers (passe couleur, `colorui.rs`).
                self.color_menu(ui);
            });
            ui.menu_button("Storyboard", |ui| {
                self.action_item(ui, "Nouveau plan", "panneau.nouveau");
                self.action_item(ui, "Nouveau plan avant le plan courant", "plan.nouveau_avant");
                self.action_item(ui, "Nouvelle séquence", "sequence.nouvelle");
                ui.separator();
                let lock = if self.doc.storyboard_locked { "Déverrouiller le storyboard" } else { "Verrouiller le storyboard" };
                self.action_item(ui, lock, "storyboard.verrouiller");
                ui.separator();
                self.action_item(ui, "Dupliquer le plan", "panneau.dupliquer");
                self.action_item(ui, "Copier les plans choisis", "panneau.copier");
                self.action_item(ui, "Coller les plans après le plan courant", "panneau.coller");
                self.action_item(ui, "Supprimer les plans choisis", "panneau.supprimer");
                self.action_item(ui, "Allonger le plan d'une image", "panneau.ajouter_image");
                self.action_item(ui, "Raccourcir le plan d'une image", "panneau.retirer_image");
                ui.separator();
                self.action_item(ui, "Couper le plan à l'image courante", "panneau.couper");
                self.action_item(ui, "Réunir le plan et le suivant", "panneau.fusionner_suivant");
                self.action_item(ui, "Réunir avec le plan précédent", "scene.joindre_precedente");
                self.action_item(ui, "Fusionner les plans choisis", "panneau.fusionner_choisis");
                ui.separator();
                self.action_item(ui, "Marque d'image ici (ajouter / retirer)", "marque.basculer");
                ui.separator();
                self.action_item(ui, "Plan précédent", "panneau.precedent");
                self.action_item(ui, "Plan suivant", "panneau.suivant");
                ui.separator();
                let label = match self.mode {
                    Mode::Storyboard => "Animer ce plan",
                    Mode::Animation => "Retour au storyboard",
                };
                self.action_item(ui, label, "mode.basculer");
                self.action_item(ui, "Vue dessin / vignettes", "vue.planches");
            });
            ui.menu_button("Son", |ui| self.sound_menu(ui));
            ui.menu_button("Calques", |ui| self.layer_ops_menu(ui));
            ui.menu_button("Animation", |ui| {
                self.action_item(ui, "Lecture / pause", "lecture.basculer");
                self.action_item(ui, "Image précédente", "image.precedente");
                self.action_item(ui, "Image suivante", "image.suivante");
                self.action_item(ui, "Première image du plan", "image.premiere");
                self.action_item(ui, "Dernière image du plan", "image.derniere");
                ui.separator();
                self.action_item(ui, "Nouveau dessin", "dessin.nouveau");
                self.action_item(ui, "Dupliquer le dessin", "dessin.dupliquer");
                self.action_item(ui, "Allonger l'exposition", "exposition.allonger");
                self.action_item(ui, "Raccourcir l'exposition", "exposition.raccourcir");
                self.action_item(ui, "Retirer le dessin de la timeline", "dessin.supprimer");
                ui.separator();
                ui.menu_button("Images", |ui| self.images_menu(ui));
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
                self.action_item(ui, "Zones de sécurité", "vue.cadres_securite");
                self.action_item(ui, "Plateau en 3D", "vue.3d");
                self.action_item(ui, "Vue de dessus (profondeur des calques)", "vue.dessus");
                self.action_item(ui, "Réinitialiser la caméra du plan", "camera.reinitialiser");
                ui.separator();
                self.action_item(ui, "Guide : afficher et utiliser", "guide.activer");
                self.action_item(ui, "Guide : aligner les traits", "guide.aligner");
                self.action_item(ui, "Guide : verrouiller", "guide.verrouiller");
                self.action_item(ui, "Guide : lignes de référence", "guide.lignes_reference");
                ui.separator();
                self.workspace_menu(ui);
                self.action_item(ui, "Plein écran", "vue.plein_ecran");
                if ui
                    .checkbox(&mut self.settings.system_cursor, "Croix du système sur le pinceau")
                    .on_hover_text(
                        "Affiche la croix de Windows à la pointe, en plus du cercle du pinceau : elle suit \
                         le stylet sans retard (le cercle, dessiné par Pelure, arrive une image plus tard).",
                    )
                    .changed()
                {
                    let _ = self.settings.save();
                }
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
                self.action_item(ui, "Raccourcis clavier…", "aide.raccourcis");
                let rec = if self.record_strokes.is_some() { "Arrêter d'enregistrer mes traits" } else { "Enregistrer mes traits (banc d'essai du moteur)" };
                self.action_item(ui, rec, "pinceau.enregistrer_traits");
                if menu_item(ui, "Modifier le fichier de raccourcis à la main…", "") {
                    self.edit_keymap();
                }
                if menu_item(ui, "Recharger les raccourcis", "") {
                    self.reload_keymap();
                }
                ui.separator();
                ui.label(RichText::new(format!("Affichage : {}", self.display_info)).small().weak());
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Les deux niveaux restent écrits en toutes lettres : on voit d'un coup d'œil où
                // l'on est.
                let animation = ui.selectable_label(self.mode == Mode::Animation, RichText::new(" Animation ").strong());
                let animation = self.aide(animation, "Animation", "Animer le plan courant, image par image.", "mode.basculer");
                if animation.clicked() && self.mode != Mode::Animation {
                    self.set_mode(Mode::Animation);
                }
                let board = ui.selectable_label(self.mode == Mode::Storyboard, RichText::new(" Storyboard ").strong());
                let board = self.aide(board, "Storyboard", "Monter les plans et les séquences.", "mode.basculer");
                if board.clicked() && self.mode != Mode::Storyboard {
                    self.set_mode(Mode::Storyboard);
                }
            });
        });
    }

    /// Panneau Outils : une icône par outil ; nom, raccourci et description dans la bulle.
    pub(crate) fn tool_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            for tool in Tool::ALL {
                let icon = RichText::new(tool.icon()).size(21.0);
                let resp = ui.add_sized([38.0, 34.0], egui::SelectableLabel::new(self.tool == tool, icon));
                if self.aide(resp, tool.label(), tool.description(), tool.action()).clicked() {
                    self.set_tool(tool);
                }
            }
        });
    }
    /// Panneau des réglages de l'outil courant.
    pub(crate) fn tool_options_panel(&mut self, ui: &mut egui::Ui) {
        let vector = self.layer().kind == LayerKind::Vector;
        ui.horizontal(|ui| {
            ui.heading(format!("{}  {}", self.tool.icon(), self.tool.label()));
            ui.label(
                RichText::new(if vector { "calque vectoriel" } else { "calque bitmap" })
                    .small()
                    .weak(),
            );
        });
        // Peint derrière, remplir la forme (`brushfill`) : en haut, pour le pinceau et le crayon.
        if matches!(self.tool, Tool::Brush | Tool::Pencil) {
            use egui_phosphor::regular as ph;
            let (behind_key, fill_key) = (self.raccourci("pinceau.derriere"), self.raccourci("pinceau.remplir"));
            ui.horizontal(|ui| {
                crate::bulles::interrupteur_a(
                    ui,
                    &mut self.paint_behind,
                    ph::SELECTION_BACKGROUND,
                    "Peint derrière",
                    "La peinture passe sous ce qui est déjà dessiné sur le calque.",
                    &behind_key,
                    "pinceau.derriere",
                );
                crate::bulles::interrupteur_a(
                    ui,
                    &mut self.fill_shapes,
                    ph::PAINT_BUCKET,
                    "Remplir la forme",
                    "Au lâcher, un trait fermé (son bout revient près de son début, ou il se recoupe) remplit son intérieur de sa couleur, sous le trait. Sur un calque vectoriel, la zone reste modifiable.",
                    &fill_key,
                    "pinceau.remplir",
                );
            });
        }
        // Réglages nommés, papier et grain : pinceau et crayon sur les deux sortes de calques (le
        // trait vectoriel texturé reste modifiable, 7b) ; la gomme sur un calque bitmap.
        if matches!(self.tool, Tool::Brush | Tool::Pencil) || (self.tool == Tool::Eraser && !vector) {
            self.brush_library_ui(ui);
        }
        match self.tool {
            Tool::Brush => {
                brush_ui(ui, &mut self.brush, vector);
                if vector {
                    note(ui, "Le trait reste modifiable avec l'outil Sélection (V), même texturé (pointe en \
                              image, grain du papier).");
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
                    crate::bulles::interrupteur(
                        ui,
                        &mut self.vector_erase_whole,
                        egui_phosphor::regular::PATH,
                        "Effacer des traits entiers",
                        "La gomme retire chaque trait touché en entier au lieu de le couper.",
                        "",
                    );
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
                ui.horizontal(|ui| {
                    use egui_phosphor::regular as ph;
                    ui.label("Contours pris sur");
                    let sample = &mut self.fill.sample_all;
                    crate::bulles::choix(ui, sample, false, ph::STACK_SIMPLE, "Le calque courant", "Les contours du seul calque où l'on remplit.", "");
                    crate::bulles::choix(ui, sample, true, ph::STACK, "Tous les calques visibles", "Trait sur un calque, couleurs sur un autre.", "");
                });
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
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline => self.shape_options_ui(ui),
            Tool::Text => self.text_options_ui(ui),
            Tool::Stamp => self.stamp_options_ui(ui),
            Tool::Clone => self.clone_options_ui(ui),
            Tool::Perspective => self.persp_options_ui(ui),
            Tool::Select => self.selection_ui(ui),
            Tool::Picker => {
                ui.label("Clique sur le dessin pour prélever une couleur.");
                note(ui, "Avec les autres outils : Alt + clic.");
            }
            Tool::Pan => {
                ui.label("Glisse pour déplacer la vue.");
                note(ui, "Sinon : Espace + glisser, ou bouton du milieu.");
            }
            Tool::Transform => {
                self.transform_fields(ui);
                note(
                    ui,
                    "Glisser dans la boîte : déplacer le calque (Maj : à l'horizontale ou à la \
                     verticale). Coin : agrandir ou réduire. Poignée ronde : tourner (Maj : par 15°). \
                     Les dessins ne sont pas modifiés : on peut annuler la transformation à tout moment.",
                );
            }
            Tool::Ref3d => self.ref3d_panel(ui),
            Tool::Camera => self.camera_panel(ui),
        }
        if self.current_brush().is_some_and(|b| b.pressure_size || b.pressure_opacity) {
            note(ui, "La pression vient du stylet (Windows Ink). À la souris, elle vaut 100 %.");
        }
    }

    /// Panneau Vue : cadrage, zoom, miroir, rotation, table lumineuse.
    pub(crate) fn view_panel(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::ARROWS_OUT, "Ajuster à la fenêtre", "Toute la feuille dans la vue.", "vue.ajuster").clicked() {
                self.fit_pending = true;
            }
            if self.bouton(ui, ph::MAGNIFYING_GLASS, "Zoom 100 %", "Un pixel du dessin pour un pixel de l'écran.", "vue.reinit_zoom").clicked() {
                self.zoom_actual();
            }
            if self.bouton_etat(ui, self.view.flip, ph::FLIP_HORIZONTAL, "Miroir de la vue", "Voir le dessin retourné ; le dessin ne change pas.", "vue.miroir").clicked() {
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
        ui.horizontal(|ui| {
            let key = self.raccourci("vue.table_lumineuse");
            crate::bulles::interrupteur_a(ui, &mut self.light_table, ph::LIGHTBULB, "Table lumineuse", "Les autres calques du plan estompés ; le calque courant reste net.", &key, "vue.table_lumineuse");
            if self.light_table {
                let resp = ui.add(
                    egui::Slider::new(&mut self.light_table_strength, 0.05..=0.8)
                        .text("autres")
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
                crate::bulles::bulle(resp, "Autres calques", "Combien ils restent visibles.", "");
            }
        });
        // Cadres de composition (9d), aussi dans la barre au-dessus du dessin.
        ui.separator();
        ui.horizontal(|ui| {
            let key = self.raccourci("vue.cadres_securite");
            crate::bulles::interrupteur_a(ui, &mut self.layout.safe_frames, ph::SELECTION, "Zones de sécurité", "Action dans 93 % du cadre, titres dans 90 % (EBU R 95).", &key, "vue.cadres_securite");
            self.frame_format_ui(ui);
        });
    }

    /// Options de l'outil Sélection sur un calque bitmap.
    fn bitmap_selection_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::choix;
        use egui_phosphor::regular as ph;
        ui.horizontal(|ui| {
            choix(ui, &mut self.select_rect, false, ph::LASSO, "Lasso", "Entourer la zone à main levée.", "");
            choix(ui, &mut self.select_rect, true, ph::RECTANGLE_DASHED, "Rectangle", "Tirer un rectangle.", "");
        });
        let has = self.bsel.is_some();
        ui.label(match self.bsel.as_ref().map(|s| s.lifted.is_some()) {
            None => "Rien de sélectionné.",
            Some(false) => "Zone sélectionnée.",
            Some(true) => "Zone soulevée : elle sera posée en quittant la sélection.",
        });
        ui.horizontal_wrapped(|ui| {
            if self.bouton_si(ui, has, ph::FLIP_HORIZONTAL, "Miroir horizontal", "Retourner la zone de gauche à droite.", "selection.miroir_h").clicked() {
                self.bitmap_flip(true);
            }
            if self.bouton_si(ui, has, ph::FLIP_VERTICAL, "Miroir vertical", "Retourner la zone de haut en bas.", "selection.miroir_v").clicked() {
                self.bitmap_flip(false);
            }
            if self.bouton_si(ui, has, ph::COPY, "Copier", "Copier la zone.", "edition.copier").clicked() {
                self.bitmap_copy();
            }
            if self.bouton_si(ui, has, ph::SCISSORS, "Couper", "Copier la zone et l'effacer.", "edition.couper").clicked() {
                self.bitmap_cut();
            }
            if self.bouton_si(ui, has, ph::ERASER, "Effacer dedans", "Effacer ce qui est dans la zone.", "").clicked() {
                self.bitmap_erase(true);
            }
            if self.bouton_si(ui, has, ph::SELECTION_INVERSE, "Effacer autour", "Effacer tout ce qui est hors de la zone.", "").clicked() {
                self.bitmap_erase(false);
            }
            if self.bouton_si(ui, has, ph::CHECK, "Poser", "Reposer la zone et quitter la sélection (Échap).", "").clicked() {
                self.drop_bitmap_selection();
            }
            ui.separator();
            if self.bouton(ui, ph::SELECTION_ALL, "Tout sélectionner", "Tout le dessin.", "edition.tout_selectionner").clicked() {
                self.bitmap_select_all();
            }
            let can_paste = self.pixel_clipboard.is_some();
            if self.bouton_si(ui, can_paste, ph::CLIPBOARD, "Coller", "Coller la zone copiée.", "edition.coller").clicked() {
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
        use egui_phosphor::regular as ph;
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
        let has = n > 0;
        ui.horizontal_wrapped(|ui| {
            if self.bouton_si(ui, has, ph::DROP, "Couleur courante", "Donner la couleur courante aux éléments choisis.", "").clicked() {
                self.recolor_selection();
            }
            if self.bouton_si(ui, has, ph::PLUS_CIRCLE, "Trait plus épais", "", "").clicked() {
                self.scale_selection_width(1.25);
            }
            if self.bouton_si(ui, has, ph::MINUS_CIRCLE, "Trait plus fin", "", "").clicked() {
                self.scale_selection_width(0.8);
            }
            if self.bouton_si(ui, has, ph::FLIP_HORIZONTAL, "Miroir horizontal", "Retourner de gauche à droite.", "selection.miroir_h").clicked() {
                self.flip_selection(true);
            }
            if self.bouton_si(ui, has, ph::FLIP_VERTICAL, "Miroir vertical", "Retourner de haut en bas.", "selection.miroir_v").clicked() {
                self.flip_selection(false);
            }
            if self.bouton_si(ui, has, ph::ARROW_LINE_UP, "Devant", "Passer devant les autres éléments du dessin.", "").clicked() {
                self.order_selection(true);
            }
            if self.bouton_si(ui, has, ph::ARROW_LINE_DOWN, "Derrière", "Passer derrière les autres éléments du dessin.", "").clicked() {
                self.order_selection(false);
            }
            if self.bouton_si(ui, has, ph::COPY, "Copier", "", "edition.copier").clicked() {
                self.copy_selection();
            }
            if self.bouton_si(ui, has, ph::TRASH, "Supprimer", "", "edition.supprimer").clicked() {
                self.delete_selection();
            }
            ui.separator();
            if self.bouton(ui, ph::SELECTION_ALL, "Tout sélectionner", "Tous les éléments du dessin.", "edition.tout_selectionner").clicked() {
                self.select_all();
            }
            let can_paste = !self.clipboard.is_empty();
            if self.bouton_si(ui, can_paste, ph::CLIPBOARD, "Coller", "", "edition.coller").clicked() {
                self.paste();
            }
        });
        note(
            ui,
            "Clic : un élément. Glisser dans le vide : lasso. Maj : ajouter / retirer. \
             Poignées : échelle (Maj : proportionnelle) et rotation (Maj : par 15°).",
        );
    }
    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let where_ = match self.mode {
                Mode::Storyboard => format!("Storyboard · {}", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
                Mode::Animation => format!("Animation · {}", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
            };
            ui.label(RichText::new(where_).strong());
            // Le panneau qui a la main : copier, coller, supprimer et les raccourcis le visent.
            let hand = if self.key_focus {
                Some("raccourcis des clés")
            } else if self.panels_have_focus() {
                Some("raccourcis du montage")
            } else if self.images_focus {
                Some("raccourcis de la timeline")
            } else {
                None
            };
            if let Some(hand) = hand {
                ui.label(RichText::new(hand).color(ui.visuals().selection.stroke.color)).on_hover_text(
                    "Ce panneau a la main (trait de couleur en haut) : les raccourcis, copier, coller et supprimer le visent. Un clic dans le dessin la rend au dessin.",
                );
            }
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
            self.recovery_status_ui(ui);
            // Pendant la lecture : la cadence vraiment affichée (orange si la machine ne suit pas).
            if let (true, Some((rate, skipped))) = (self.playing, self.play_meter.rate) {
                ui.separator();
                let mut text = format!("Lecture {} i/s", format!("{rate:.1}").replace('.', ","));
                if skipped > 0 {
                    let s = if skipped > 1 { "s" } else { "" };
                    text += &format!(" · {skipped} image{s} sautée{s}");
                }
                let slow = skipped > 0 || rate < self.doc.fps as f32 * 0.95;
                let color = if slow { Color32::from_rgb(240, 170, 60) } else { ui.visuals().text_color() };
                ui.label(RichText::new(text).color(color)).on_hover_text(
                    "Images réellement affichées pendant la dernière seconde de lecture. Si la machine \
                     ne suit pas, des images sont sautées pour rester calé sur le son.",
                );
            }
            ui.separator();
            ui.label(&self.status);
        });
    }

    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        self.new_layer_window(ctx);
        self.groups_window_ui(ctx);
        self.properties_window_ui(ctx);
        self.onion_window_ui(ctx);
        self.flips_window_ui(ctx);
        self.fx_window_ui(ctx);
        self.tween_window_ui(ctx);
        self.sb_window_ui(ctx);
        self.brush_import_window(ctx);
        self.sb_export_ui(ctx);
        self.smart_rename_window(ctx);
        if let Some(mut d) = self.new_dialog.take() {
            let mut keep = true;
            let mut create = false;
            egui::Window::new("Nouveau projet")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (label, w, h) in crate::projetui::RESOLUTIONS {
                            let resp = ui.selectable_label(d.width == w && d.height == h, label).on_hover_text(format!("{w}×{h}"));
                            if resp.clicked() {
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
                self.load_doc(Document::with_frame(d.width, d.height, d.fps).created_now());
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

        // Raccourcis : l'assignation en cours et la liste (raccourcisui.rs).
        self.shortcut_windows(ctx);
    }
}
