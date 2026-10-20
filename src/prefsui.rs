//! Menus plus courts (Anthony, 2026-10-18 : « certains menus contiennent trop de choses et noient
//! les informations principales ») : deux fenêtres reprennent ce qui encombrait le menu Fichier et
//! le menu Affichage.
//! - Exporter… : les sept exports d'un coup d'œil, chacun avec une ligne d'explication, et la case
//!   « PNG sans le papier » à côté d'eux ; chaque bouton lance l'export d'avant (rien ne change
//!   dans les exports eux-mêmes).
//! - Préférences… : les réglages de l'application (pas du projet) qui étaient éparpillés dans les
//!   menus : copie de secours, dernier projet rouvert, croix du système, panneaux ajustés,
//!   raccourcis clavier ; et la section « Performance » (`diagnosticui.rs`), les onze réglages que
//!   Aide > Diagnostic des performances propose.
//!
//! Apparence (en tête de Préférences…) : le thème et ses neuf couleurs (`theme.rs`).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::theme::ChoixTheme;

/// Les exports proposés : icône, nom, explication, action (ou `None` : la page HTML, sans action).
const EXPORTS: [(&str, &str, &str, Option<&str>); 9] = [
    (ph::FILM_STRIP, "Le film (vidéo)", "Le montage ou le plan en vidéo, avec le son.", Some("fichier.exporter_film")),
    (ph::IMAGES, "Des images (PNG)", "Une image PNG par image du film.", Some("fichier.exporter_images")),
    (ph::IMAGE, "L'image courante (PNG)", "Seulement l'image affichée.", Some("fichier.exporter_image_courante")),
    (ph::FILE_PDF, "Le storyboard (PDF)", "Les plans en planches, avec leurs textes.", Some("fichier.exporter_pdf")),
    (ph::BROWSER, "Le storyboard (page HTML + planches)", "Une page à ouvrir dans un navigateur, et ses images.", None),
    (ph::SPEAKER_HIGH, "Le son (WAV)", "Les pistes son mélangées.", Some("fichier.exporter_son")),
    (ph::EXPORT, "Vers Storyboard Pro", "Un projet .sboard fait à partir du modèle choisi.", Some("fichier.exporter_sboard")),
    (ph::FILM_REEL, "Pour le montage (EDL / XML / OTIO)", "Un fichier vidéo par plan et le montage pour Premiere, DaVinci Resolve ou Avid.", Some("fichier.exporter_montage")),
    (ph::EYEGLASSES, "Pour relecture (page HTML)", "Une page sans internet à envoyer au réalisateur : il écrit ses notes et dessine dessus, SHOT les relit ensuite.", Some("fichier.exporter_relecture")),
];

impl App {
    /// La fenêtre Exporter… (menu Fichier).
    pub(crate) fn export_hub_window(&mut self, ctx: &egui::Context) {
        if !self.export_hub {
            return;
        }
        let mut open = true;
        let mut run: Option<Option<&str>> = None;
        egui::Window::new("Exporter")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .show(ctx, |ui| {
                ui.set_width(420.0);
                for (icon, name, hint, action) in EXPORTS {
                    let text = RichText::new(format!("{icon}  {name}")).strong();
                    let resp = ui.add_sized([ui.available_width(), 26.0], egui::Button::new(text).frame(true));
                    if resp.clicked() {
                        run = Some(action);
                    }
                    ui.label(RichText::new(hint).small().weak());
                    ui.add_space(4.0);
                }
                ui.separator();
                ui.checkbox(&mut self.export_transparent, "Images PNG sans le papier (fond transparent)");
            });
        if let Some(action) = run {
            self.export_hub = false;
            match action {
                Some(a) => self.run_action(a),
                None => self.export_storyboard(),
            }
        } else {
            self.export_hub = open;
        }
    }

    /// Apparence : le thème (Gris sombre, Gris clair, Sombre, Personnalisé) et les neuf couleurs de
    /// l'interface. Tout change aussitôt ; modifier une couleur passe en « Personnalisé », en
    /// partant du thème courant. Le réglage est enregistré par `App::update` une fois le curseur
    /// du sélecteur de couleur relâché.
    fn theme_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Apparence").strong());
        ui.horizontal(|ui| {
            ui.label("Thème");
            let actuel = self.settings.theme.choix;
            egui::ComboBox::from_id_salt("theme_choix").selected_text(actuel.nom()).show_ui(ui, |ui| {
                for choix in ChoixTheme::TOUS {
                    if ui.selectable_label(actuel == choix, choix.nom()).clicked() {
                        self.settings.theme.choisir(choix);
                    }
                }
            });
            if ui
                .button("Rétablir")
                .on_hover_text("Revenir au thème par défaut, Gris sombre (les couleurs personnalisées sont aussi remises à zéro).")
                .clicked()
            {
                self.settings.theme.retablir();
            }
        });
        let avant = self.settings.theme.couleurs();
        let mut couleurs = avant;
        egui::Grid::new("theme_couleurs").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            for (nom, aide, rvb) in couleurs.champs() {
                ui.label(nom).on_hover_text(aide);
                ui.color_edit_button_srgb(rvb).on_hover_text(aide);
                ui.end_row();
            }
        });
        if couleurs != avant {
            self.settings.theme.modifier(couleurs);
        }
        ui.label(RichText::new("Changer une couleur passe en « Personnalisé » ; le survol, le texte secondaire et les couleurs de la timeline s'en déduisent.").small().weak());
    }

    /// La fenêtre Préférences… (menu Fichier).
    pub(crate) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.prefs_window {
            return;
        }
        let mut open = true;
        let mut action: Option<&str> = None;
        let mut jeu: Option<crate::keymap::PresetId> = None;
        egui::Window::new("Préférences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .show(ctx, |ui| {
                ui.set_width(440.0);
                // Défile quand la fenêtre dépasse l'écran (la section Génération sonore l'allonge). Une
                // fenêtre non redimensionnable ne donne à son contenu que la hauteur qu'il avait à
                // l'image d'avant : `min_scrolled_height` lui donne tout de suite la hauteur voulue, et
                // `auto_shrink` la ramène à celle du contenu quand il est plus court.
                let hauteur = (ctx.screen_rect().height() - 170.0).max(240.0);
                egui::ScrollArea::vertical().max_height(hauteur).min_scrolled_height(hauteur).auto_shrink([false, true]).show(ui, |ui| {
                self.theme_prefs_ui(ui);
                ui.add_space(6.0);
                ui.label(RichText::new("Projets").strong());
                self.autosave_menu_ui(ui);
                if ui.checkbox(&mut self.settings.reopen_last, "Rouvrir le dernier projet au lancement").changed() {
                    let _ = self.settings.save();
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Commentaires").strong());
                ui.horizontal(|ui| {
                    ui.label("Signer mes commentaires :");
                    let hint = crate::commentaires::author_name("");
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.settings.comment_author).hint_text(hint).desired_width(160.0));
                    if resp.lost_focus() {
                        let _ = self.settings.save();
                    }
                });
                ui.label(RichText::new("Vide : le nom de l'utilisateur Windows.").small().weak());
                ui.add_space(6.0);
                ui.label(RichText::new("Dessin").strong());
                let resp = ui.checkbox(&mut self.settings.system_cursor, "Croix du système sur le pinceau").on_hover_text(
                    "Affiche la croix de Windows à la pointe, en plus du cercle du pinceau : elle suit le \
                     stylet sans retard (le cercle, dessiné par SHOT, arrive une image plus tard).",
                );
                if resp.changed() {
                    let _ = self.settings.save();
                }
                ui.label(RichText::new("Le toucher du stylet se règle en bas des réglages du pinceau.").small().weak());
                ui.add_space(6.0);
                ui.label(RichText::new("Panneaux").strong());
                let resp = ui.checkbox(&mut self.settings.panes_fit, "Ajuster les panneaux à leur contenu").on_hover_text(
                    "Les réglages de l'outil, les guides, la fiche et les effets prennent la hauteur de ce \
                     qu'ils montrent. Un bord tiré à la main reste où on l'a mis.",
                );
                if resp.changed() {
                    let _ = self.settings.save();
                }
                if ui.button("Réajuster les panneaux").clicked() {
                    action = Some("vue.reajuster_panneaux");
                }
                ui.add_space(6.0);
                // Performance (`diagnosticui.rs`) : les réglages que Aide > Diagnostic des
                // performances propose, un par un.
                self.performance_prefs_ui(ui);
                ui.add_space(6.0);
                // Génération sonore (`genprefsui.rs`) : le fournisseur de chaque rôle, les clés.
                self.generation_prefs_ui(ui);
                ui.add_space(6.0);
                ui.label(RichText::new("Raccourcis clavier").strong());
                jeu = crate::raccourcisui::preset_combo(ui, "jeu_raccourcis_prefs", self.settings.keymap_preset);
                ui.label(RichText::new(crate::raccourcisui::MERGE_RULE).small().weak());
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Voir et changer les raccourcis…").clicked() {
                        action = Some("aide.raccourcis");
                    }
                    if ui.button("Modifier le fichier à la main…").clicked() {
                        self.edit_keymap();
                    }
                    if ui.button("Recharger").clicked() {
                        self.reload_keymap();
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new(format!("Affichage : {}", self.display_info)).small().weak());
                });
            });
        self.prefs_window = open;
        if let Some(p) = jeu {
            self.request_preset(ctx, p);
        }
        if let Some(a) = action {
            self.run_action(a);
        }
    }
}
