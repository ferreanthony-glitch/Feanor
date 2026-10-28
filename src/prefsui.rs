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
//! Apparence : le thème et ses neuf couleurs (`theme.rs`).
//!
//! La fenêtre Préférences… est rangée en neuf onglets (2026-10-21, `prefsonglets.rs` : la table des réglages) :
//! une liste verticale à gauche (icône et nom), le contenu de l'onglet à droite, un champ de recherche en haut qui
//! montre, sur tous les onglets, les groupes de réglages qui répondent. Chaque onglet est une suite de groupes (un
//! titre court, un trait, les réglages) ; les réglages jouent tout de suite, comme avant, et s'écrivent dans les
//! mêmes clés de `reglages.toml` (plus `onglet_preferences`, l'onglet ouvert).

use eframe::egui::{self, vec2, Align, Layout, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::prefsonglets::{self as table, Groupe, Onglet};
use crate::theme::{self, ChoixTheme};

/// Taille de départ de la fenêtre (plus large que haute) et plus petite taille permise.
pub(crate) const FENETRE: [f32; 2] = [800.0, 540.0];
const FENETRE_MIN: [f32; 2] = [560.0, 320.0];
/// Largeur de la liste des onglets.
const LISTE_L: f32 = 156.0;

/// Ce qu'un groupe demande à la fenêtre une fois dessiné : une action à lancer, un jeu de raccourcis à proposer.
#[derive(Default)]
struct Sortie {
    action: Option<&'static str>,
    jeu: Option<crate::keymap::PresetId>,
}

/// Le texte de la recherche, gardé dans la mémoire d'egui (il ne vaut que pour la fenêtre ouverte).
fn id_recherche() -> egui::Id {
    egui::Id::new("prefs_recherche")
}

pub(crate) fn recherche(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(id_recherche())).unwrap_or_default()
}

pub(crate) fn definir_recherche(ctx: &egui::Context, texte: &str) {
    ctx.data_mut(|d| d.insert_temp(id_recherche(), texte.to_string()));
}

/// Le titre d'un groupe : un texte court, un trait dessous.
fn titre_groupe(ui: &mut egui::Ui, groupe: Groupe, premier: bool) {
    if !premier {
        ui.add_space(14.0);
    }
    ui.label(RichText::new(groupe.titre()).strong().size(14.0).color(theme::texte_fort()));
    ui.separator();
}

/// Les exports proposés : icône, nom, explication, action (ou `None` : la page HTML, sans action).
const EXPORTS: [(&str, &str, &str, Option<&str>); 13] = [
    (ph::FILM_STRIP, "Le film (vidéo)", "Le montage ou le plan en vidéo, avec le son.", Some("fichier.exporter_film")),
    (ph::GIF, "Un GIF animé", "Le film, le plan courant ou les plans choisis, en petit GIF à partager (sans le son).", Some("fichier.exporter_gif")),
    (ph::IMAGES, "Des images (PNG)", "Une image PNG par image du film.", Some("fichier.exporter_images")),
    (ph::IMAGE, "L'image courante (PNG)", "Seulement l'image affichée.", Some("fichier.exporter_image_courante")),
    (ph::FILE_PDF, "Le storyboard (PDF)", "Les plans en planches, avec leurs textes.", Some("fichier.exporter_pdf")),
    (ph::BROWSER, "Le storyboard (page HTML + planches)", "Une page à ouvrir dans un navigateur, et ses images.", None),
    (ph::SPEAKER_HIGH, "Le son (WAV)", "Les pistes son mélangées.", Some("fichier.exporter_son")),
    (ph::EXPORT, "Vers Storyboard Pro", "Un projet .sboard fait à partir du modèle choisi.", Some("fichier.exporter_sboard")),
    (ph::FILM_REEL, "Pour le montage (EDL / XML / OTIO)", "Un fichier vidéo par plan et le montage pour Premiere, DaVinci Resolve ou Avid.", Some("fichier.exporter_montage")),
    (ph::TABLE, "Fiche technique (tableur)", "Un plan par ligne, en classeur XLSX ou en CSV : durées, time codes, cadrage, mouvement de caméra, textes, sons, vignettes.", Some("fichier.exporter_fiche")),
    (ph::EYEGLASSES, "Pour relecture (page HTML)", "Une page sans internet à envoyer au réalisateur : il écrit ses notes et dessine dessus, SHOT les relit ensuite.", Some("fichier.exporter_relecture")),
    (ph::SQUARES_FOUR, "Planche des vignettes (PNG, JPEG, PDF)", "Toute la planche des Vignettes en une grande image ou en PDF : numéros, notes, barres, cadres de caméra, annotations, marie-louise.", Some("fichier.exporter_planche")),
    (ph::PALETTE, "Color script (PNG, PDF)", "Une petite image par plan, rangées par séquence sur un fond uni : la couleur du film d'un coup d'œil.", Some("fichier.exporter_colorscript")),
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
        ui.horizontal(|ui| {
            ui.label("Thème");
            let actuel = self.settings.theme.choix;
            egui::ComboBox::from_id_salt("theme_choix")
                .selected_text(actuel.nom())
                .show_ui(ui, |ui| {
                    for choix in ChoixTheme::TOUS {
                        if ui.selectable_label(actuel == choix, choix.nom()).clicked() {
                            self.settings.theme.choisir(choix);
                        }
                    }
                })
                .response
                .on_hover_text("Gris sombre, Gris clair, Sombre ou Personnalisé (avec tes neuf couleurs).");
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

    /// Vignettes > Planche des Vignettes : le nombre de rangées de cases pâles qu'on voit toujours (pour montrer où dessiner), le nombre de
    /// cases par rangée, et le débord du trait hors des cases. Les rangées et les colonnes sont des réglages de la disposition
    /// (`Layout`, retenus dans `reglages.toml` comme avant) ; le débord est `ReglagesVignettes::debord`.
    pub(crate) fn vignettes_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Rangées de cases pâles");
            let mut n = self.layout.board_min_rows;
            let resp = ui.add(egui::DragValue::new(&mut n).range(1..=crate::board::MIN_ROWS_MAX).speed(0.05));
            let resp = crate::bulles::bulle(
                resp,
                "Rangées de cases pâles",
                "Les cases pâles de la planche des Vignettes montrent où dessiner : au moins ce nombre de rangées en tout (trois d'origine). Dès qu'on dessine dans la dernière rangée, une rangée pâle de plus apparaît.",
                "",
            );
            if resp.changed() {
                self.layout.board_min_rows = n.clamp(1, crate::board::MIN_ROWS_MAX);
            }
        });
        ui.label(RichText::new("Une rangée pâle de plus apparaît dès qu'on dessine dans la dernière.").small().weak());
        ui.horizontal(|ui| {
            ui.label("Cases par rangée");
            let mut n = self.layout.board_columns;
            let resp = ui.add(egui::DragValue::new(&mut n).range(1..=crate::board::MAX_COLUMNS).speed(0.05));
            let resp = crate::bulles::bulle(resp, "Cases par rangée", "Les cases d'une rangée de la planche (six au plus). C'est aussi le réglage « Colonnes » en haut de la planche.", "");
            if resp.changed() {
                self.layout.board_columns = n.clamp(1, crate::board::MAX_COLUMNS);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Débord du trait hors des cases");
            let mut v = self.settings.vignettes.debord;
            let resp = ui.add(egui::Slider::new(&mut v, 0..=crate::vignettesreglages::DEBORD_MAX).suffix(" %"));
            let resp = crate::bulles::bulle(
                resp,
                "Débord du trait hors des cases",
                "Ce qu'on voit du trait au-delà du cadre d'une case, de chaque côté, en pour cent de sa largeur (12 % d'origine). Seule la case où l'on dessine laisse déborder tout son trait ; les autres sont coupées net, à quelques points près.",
                "",
            );
            if resp.changed() {
                self.settings.vignettes.debord = v;
                self.settings_a_ecrire = true;
            }
        });
    }

    /// Vignettes > Cadres de caméra : l'aimant (points d'écran, 0 : éteint) et la couleur des cadres A, B, C… (à l'écran et dans l'export).
    pub(crate) fn cadres_camera_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Aimant des cadres de caméra");
            let mut v = self.settings.vignettes.aimant;
            let resp = ui.add(egui::DragValue::new(&mut v).range(0..=crate::vignettesreglages::AIMANT_MAX).speed(0.1).suffix(" points"));
            let resp = crate::bulles::bulle(
                resp,
                "Aimant des cadres de caméra",
                "À quelle distance d'un alignement (colonnes et rangées de la planche, bords et centres des autres cadres) un cadre qu'on déplace se colle, en points d'écran. 6 d'origine ; 0 : pas d'aimant.",
                "",
            );
            if resp.changed() {
                self.settings.vignettes.aimant = v;
                self.settings_a_ecrire = true;
            }
        });
        ui.label(RichText::new("0 : pas d'aimant, le cadre suit le pointeur partout.").small().weak());
        ui.horizontal(|ui| {
            ui.label("Couleur des cadres de caméra");
            let mut c = self.settings.vignettes.couleur_cadre;
            let resp = ui.color_edit_button_srgb(&mut c);
            let resp = crate::bulles::bulle(resp, "Couleur des cadres de caméra", "La couleur fine des cadres A, B, C…, de leurs lettres et de la flèche, sur la planche et dans l'export de la planche. Bleu #378ADD d'origine.", "");
            if resp.changed() {
                self.settings.vignettes.couleur_cadre = c;
                self.settings_a_ecrire = true;
            }
        });
    }

    /// Vignettes > Décor : le report du décor à la jonction de deux plans, la couleur et la taille des trois brosses du décor, et de quoi
    /// tout rétablir.
    pub(crate) fn decor_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let mut report = self.settings.vignettes.report_decor;
        let resp = ui.checkbox(&mut report, "Report du décor");
        let resp = crate::bulles::bulle(
            resp,
            "Report du décor",
            "Quand la barre entre deux cases est décochée (les cases sont un seul plan), la case suivante montre le même décor, et seulement lui. Éteint, la case suivante repart d'un décor vide. Ne joue qu'au moment où l'on décoche la barre.",
            "",
        );
        if resp.changed() {
            self.settings.vignettes.report_decor = report;
            self.settings_a_ecrire = true;
        }
        ui.horizontal(|ui| {
            ui.label("Calque Décor");
            let mut bitmap = self.settings.vignettes.decor_bitmap;
            let a = ui.selectable_value(&mut bitmap, false, "Vectoriel");
            let a = crate::bulles::bulle(a, "Décor vectoriel", "Le calque Décor que les brosses du décor créent dans un plan est vectoriel : ses traits restent des traits qu'on peut sélectionner et déformer. D'origine.", "");
            let b = ui.selectable_value(&mut bitmap, true, "Bitmap");
            let b = crate::bulles::bulle(b, "Décor bitmap", "Le calque Décor que les brosses du décor créent dans un plan est un dessin de pixels, comme celui des anciens projets.", "");
            if a.changed() || b.changed() {
                self.settings.vignettes.decor_bitmap = bitmap;
                self.settings_a_ecrire = true;
            }
        });
        ui.label(RichText::new("Ne change pas le calque Décor d'un plan qui en a déjà un : il garde sa sorte et reçoit toujours les traits du décor.").small().weak());
        ui.horizontal(|ui| {
            ui.label("Couleur du décor");
            let mut c = self.settings.vignettes.couleur_decor;
            let resp = ui.color_edit_button_srgb(&mut c);
            let resp = crate::bulles::bulle(resp, "Couleur du décor", "La couleur des trois brosses du décor, un gris-bleu clair d'origine (#8FA3B8) pour que le décor se lise à part des personnages.", "");
            if resp.changed() {
                self.settings.vignettes.couleur_decor = c;
                self.settings_a_ecrire = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Taille des brosses du décor");
            for g in crate::decor::Genre::ALL {
                let mut t = self.settings.vignettes.taille_decor(g);
                let nom = match g {
                    crate::decor::Genre::Crayon => "Crayon",
                    crate::decor::Genre::Feutre => "Feutre",
                    crate::decor::Genre::Aplat => "Aplat",
                };
                ui.label(RichText::new(nom).weak());
                let resp = ui.add(egui::DragValue::new(&mut t).range(0.5..=400.0).speed(0.3).max_decimals(0).suffix(" px"));
                let resp = crate::bulles::bulle(resp, g.nom(), "Sa taille, en pixels du dessin. Elle se pose en choisissant la brosse dans la colonne des pastilles.", "");
                if resp.changed() {
                    self.settings.vignettes.tailles_decor[g.rang()] = t;
                    self.settings_a_ecrire = true;
                }
            }
        });
        if ui.small_button("Rétablir les réglages des Vignettes").on_hover_text("Remet le débord, l'aimant, les couleurs, le report et les tailles des brosses du décor à leurs valeurs d'origine (pas les rangées ni les cases par rangée).").clicked() {
            self.settings.vignettes = Default::default();
            self.settings_a_ecrire = true;
        }
    }

    /// Dessin > Caméra : le passe-partout, ce qui couvre le hors-champ autour du cadre de la caméra quand « Passe-partout »
    /// est allumé (`camtools.rs`) : sa couleur (noir d'origine) et son opacité (de 0 %, rien, à 100 %, opaque, d'origine).
    /// Un seul réglage : il remplace l'ancien voile gris translucide. Retenu dans les réglages.
    pub(crate) fn passe_partout_pref_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Couleur du passe-partout");
            let mut c = self.settings.passe_partout_couleur;
            let resp = ui.color_edit_button_srgb(&mut c);
            let resp = crate::bulles::bulle(
                resp,
                "Couleur du passe-partout",
                "La couleur qui couvre ce qui est hors du cadre de la caméra, quand « Passe-partout » est allumé : noir d'origine. Elle ne touche pas aux exports.",
                &self.raccourci("camera.masque"),
            );
            if resp.changed() {
                self.settings.passe_partout_couleur = c;
                self.settings_a_ecrire = true;
            }
            // Le cadre de format (Cadrage > Format) : gris d'origine depuis le 2026-10-28 (Anthony : il était rose). Sur la même
            // ligne que le passe-partout : le groupe Caméra ne grandit pas.
            ui.add_space(12.0);
            ui.label("Cadre de format");
            let mut c = self.settings.cadre_format_couleur;
            let resp = ui.color_edit_button_srgb(&mut c);
            let resp = crate::bulles::bulle(
                resp,
                "Couleur du cadre de format",
                "Le cadre du format de diffusion (Cadrage > Format) tracé dans le cadre de la caméra : gris d'origine. Il ne touche pas aux exports. Clic droit : revenir au gris.",
                "",
            );
            if resp.secondary_clicked() {
                c = crate::settings::CADRE_FORMAT_COULEUR;
            }
            if c != self.settings.cadre_format_couleur {
                self.settings.cadre_format_couleur = c;
                self.settings_a_ecrire = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Opacité du passe-partout");
            let mut v = f32::from(self.settings.passe_partout);
            let resp = ui.add(egui::Slider::new(&mut v, 0.0..=100.0).suffix(" %").fixed_decimals(0));
            let resp = crate::bulles::bulle(
                resp,
                "Opacité du passe-partout",
                "À quel point le passe-partout cache le hors-champ. 100 % : opaque, rien ne se voit autour du cadre (d'origine) ; 0 % : rien n'est couvert.",
                &self.raccourci("camera.masque"),
            );
            if resp.changed() {
                self.settings.passe_partout = v.round().clamp(0.0, 100.0) as u8;
            }
            if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                let _ = self.settings.save();
            }
            if ui.small_button("Rétablir").on_hover_text("Revenir au passe-partout d'origine : noir, opaque.").clicked() {
                self.settings.passe_partout = crate::camtools::PASSE_PARTOUT_DEFAUT;
                self.settings.passe_partout_couleur = crate::camtools::PASSE_PARTOUT_COULEUR;
                let _ = self.settings.save();
            }
        });
    }

    /// Dessin > Tremblement des traits : le tremblement par défaut de tous les calques vectoriels (`tremblement::Defaut`) :
    /// allumé ou non, son intensité (pixels) et sa fréquence (doux à nerveux). Un calque qui a son propre « Tremblement du trait »
    /// (panneau Effets) garde le sien. Joue à l'écran, dans les vignettes et dans les exports ; n'écrit rien dans les projets.
    pub(crate) fn tremblement_defaut_pref_ui(&mut self, ui: &mut egui::Ui) {
        let avant = self.settings.tremblement_defaut;
        let mut d = avant;
        let resp = ui.checkbox(&mut d.actif, "Tremblement du trait par défaut");
        crate::bulles::bulle(
            resp,
            "Tremblement du trait par défaut",
            "Tous les traits vectoriels tremblent un peu, comme redessinés à la main, sans rien ajouter aux calques : un calque qui a son propre « Tremblement du trait » (panneau Effets) garde le sien. Les exports le montrent aussi. Rien n'est écrit dans les projets.",
            "",
        );
        // Les réglages à gauche, l'aperçu à droite (il bouge quand « Animer à l'arrêt » est allumé).
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.add_enabled_ui(d.actif, |ui| {
                    egui::Grid::new("tremblement_defaut_reglages").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.label("Intensité du tremblement par défaut");
                        let resp = ui.add(egui::Slider::new(&mut d.intensite, 0.0..=crate::tremblement::MAX_INTENSITY).suffix(" px").max_decimals(1));
                        crate::bulles::bulle(resp, "Intensité du tremblement par défaut", "De combien de pixels de la feuille un trait peut bouger au plus (2 px d'origine).", "");
                        ui.end_row();
                        ui.label("Fréquence du tremblement par défaut");
                        let resp = ui.add(egui::Slider::new(&mut d.frequence, 0.0..=100.0).suffix(" %").fixed_decimals(0));
                        crate::bulles::bulle(resp, "Fréquence du tremblement par défaut", "0 % : le trait ondule doucement ; 100 % : nerveux, par petites vagues (55 % d'origine).", "");
                        ui.end_row();
                        ui.label("Variantes du tremblement par défaut");
                        let resp = ui.add(egui::Slider::new(&mut d.variantes, crate::tremblement::MIN_VARIANTS..=crate::tremblement::MAX_VARIANTS));
                        crate::bulles::bulle(resp, "Variantes du tremblement par défaut", "Combien de dessins légèrement différents se suivent avant de recommencer (3 d'origine). Plus il y en a, moins le trait se répète.", "");
                        ui.end_row();
                        ui.label("Vitesse du tremblement par défaut");
                        let resp = ui.add(egui::Slider::new(&mut d.vitesse, 1..=crate::tremblement::MAX_SPEED).suffix(" img/var."));
                        crate::bulles::bulle(resp, "Vitesse du tremblement par défaut", "Combien d'images du projet pour chaque variante : 1, le trait change à chaque image ; 2, une image sur deux (d'origine, comme un dessin à la main).", "");
                        ui.end_row();
                        ui.label("Épaisseur du tremblement par défaut");
                        let resp = ui.add(egui::Slider::new(&mut d.epaisseur, 0.0..=100.0).suffix(" %").fixed_decimals(0));
                        crate::bulles::bulle(resp, "Épaisseur du tremblement par défaut", "De combien le trait s'épaissit et s'amincit d'une variante à l'autre, sur son bord (0 % d'origine : seulement le déplacement).", "");
                        ui.end_row();
                        ui.label("Graine du tremblement par défaut");
                        let resp = ui.add(egui::DragValue::new(&mut d.graine).range(1..=999).speed(0.2));
                        crate::bulles::bulle(resp, "Graine du tremblement par défaut", "Le hasard du tremblement : une autre graine, un autre mouvement, aussi doux ou nerveux (1 d'origine).", "");
                        ui.end_row();
                    });
                    let resp = ui.checkbox(&mut d.repos, "Animer à l'arrêt (tremblement par défaut)");
                    crate::bulles::bulle(resp, "Animer à l'arrêt", "Allumé, le trait continue de trembler quand on s'arrête sur une image, à la cadence du réglage (l'écran se repeint à cette cadence seulement). Éteint (d'origine), une image fixe reste immobile : rien ne coûte au repos.", "");
                });
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Aperçu").small().weak());
                crate::tremblementapercu::apercu_ui(ui, &d, self.doc.fps);
            });
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(!d.est_defaut(), egui::Button::new("Rétablir")).on_hover_text("Éteint, 2 px, 55 %, 3 variantes, 2 images par variante, épaisseur 0 %, graine 1, immobile à l'arrêt.").clicked() {
                d = Default::default();
            }
            ui.label(RichText::new("Le trait tremble d'une image à l'autre. Un calque qui a son propre « Tremblement du trait » (panneau Effets) garde le sien.").small().weak());
        });
        if d != avant {
            self.settings.tremblement_defaut = d;
            self.settings_a_ecrire = true;
        }
    }

    /// Pose le tremblement par défaut là où le rendu le lit (`tremblement::set_defaut`) dès que le réglage change : les vignettes
    /// et l'image de l'écran sont à refaire ; les dessins dérivés, eux, ont leur propre numéro par réglage (rien à vider).
    pub(crate) fn appliquer_tremblement_defaut(&mut self, ctx: &egui::Context) {
        let voulu = self.settings.tremblement_defaut.actif.then(|| self.settings.tremblement_defaut.sanitized());
        if crate::tremblement::defaut() != voulu {
            crate::tremblement::set_defaut(voulu);
            self.thumbs.clear();
            self.layer_thumbs.clear();
            self.renderer.invalidate_all();
            ctx.request_repaint();
        }
    }

    /// La fenêtre Préférences… (menu Fichier) : une liste d'onglets à gauche, le contenu à droite, la recherche en
    /// haut. Redimensionnable, plus large que haute au départ.
    pub(crate) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.prefs_window {
            return;
        }
        let ecran = ctx.screen_rect();
        let taille = vec2((ecran.width() - 40.0).min(FENETRE[0]).max(FENETRE_MIN[0]), (ecran.height() - 100.0).min(FENETRE[1]).max(FENETRE_MIN[1]));
        let mut open = true;
        let mut sortie = Sortie::default();
        egui::Window::new("Préférences")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(taille)
            .min_size(FENETRE_MIN)
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(egui::pos2(ecran.center().x, ecran.top() + 60.0))
            .show(ctx, |ui| self.preferences_contenu(ui, &mut sortie));
        self.prefs_window = open;
        if let Some(p) = sortie.jeu {
            self.request_preset(ctx, p);
        }
        if let Some(a) = sortie.action {
            self.run_action(a);
        }
    }

    /// Le contenu de la fenêtre : la recherche, la liste des onglets, l'onglet (ou les réponses à la recherche).
    fn preferences_contenu(&mut self, ui: &mut egui::Ui, sortie: &mut Sortie) {
        let ctx = ui.ctx().clone();
        let mut q = recherche(&ctx);
        let mut onglet = self.settings.prefs_onglet;

        egui::TopBottomPanel::top("prefs_haut").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4))).show_separator_line(false).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                crate::bulles::etiquette(ui, ph::MAGNIFYING_GLASS, "Chercher un réglage", "Tape un mot (couleur, cache, clé, équipe…) : les groupes de réglages qui répondent s'affichent, de tous les onglets.");
                let large = (ui.available_width() - 34.0).max(80.0);
                let champ = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Chercher un réglage…").desired_width(large));
                crate::bulles::bulle(champ, "Chercher un réglage", "Un ou plusieurs mots, avec ou sans accents. Les onglets qui répondent montrent le nombre de réglages trouvés.", "");
                if !q.is_empty() && crate::bulles::icone(ui, ph::X, "Effacer la recherche", "Revenir à l'onglet ouvert.", "").clicked() {
                    q.clear();
                }
            });
        });

        let cherche = !q.trim().is_empty();
        let trouves = table::chercher(&q);

        egui::SidePanel::left("prefs_onglets").resizable(false).exact_width(LISTE_L).frame(egui::Frame::new().inner_margin(egui::Margin { right: 8, ..Default::default() })).show_inside(ui, |ui| {
            ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                ui.spacing_mut().button_padding = vec2(8.0, 5.0);
                for o in Onglet::TOUS {
                    let n: usize = trouves.iter().filter(|t| t.onglet == o).map(|t| t.libelles.len()).sum();
                    let on = !cherche && onglet == o;
                    let mut nom = format!("{}  {}", o.icone(), o.nom());
                    if cherche && n > 0 {
                        nom = format!("{nom}  ({n})");
                    }
                    let mut texte = RichText::new(nom);
                    if cherche && n == 0 {
                        texte = texte.color(theme::texte_doux());
                    } else if cherche {
                        texte = texte.color(theme::accent_texte());
                    }
                    let resp = ui.selectable_label(on, texte);
                    if crate::bulles::bulle(resp, o.nom(), o.resume(), "").clicked() {
                        onglet = o;
                        q.clear();
                    }
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new("Les réglages jouent tout de suite.").small().weak());
        });

        egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin { left: 10, ..Default::default() })).show_inside(ui, |ui| {
            let sel = if cherche { None } else { Some(onglet) };
            egui::ScrollArea::vertical().id_salt(("prefs_defile", sel)).auto_shrink([false, false]).show(ui, |ui| {
                if !cherche {
                    ui.label(RichText::new(format!("{}  {}", onglet.icone(), onglet.nom())).strong().size(18.0).color(theme::texte_fort()));
                    ui.label(RichText::new(onglet.resume()).small().weak());
                    ui.add_space(6.0);
                    for (i, g) in table::groupes_de(onglet).into_iter().enumerate() {
                        titre_groupe(ui, g, i == 0);
                        self.groupe_ui(ui, g, sortie);
                    }
                    return;
                }
                if trouves.is_empty() {
                    ui.label(RichText::new(format!("Aucun réglage ne répond à « {} ».", q.trim())).weak());
                    return;
                }
                ui.label(RichText::new(format!("{} groupe(s) de réglages répondent à « {} ».", trouves.len(), q.trim())).small().weak());
                for (i, t) in trouves.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(14.0);
                    }
                    ui.horizontal(|ui| {
                        let lien = ui.small_button(format!("{}  {} \u{203a}", t.onglet.icone(), t.onglet.nom()));
                        if crate::bulles::bulle(lien, &format!("Aller à l'onglet {}", t.onglet.nom()), "Ferme la recherche et ouvre l'onglet.", "").clicked() {
                            onglet = t.onglet;
                            q.clear();
                        }
                        // (Sur une ligne, coupée par des points de suspension : la fenêtre ne s'élargit pas pour elle.)
                        let trouve = t.libelles.join(", ");
                        ui.add(egui::Label::new(RichText::new(format!("trouvé : {trouve}")).small().color(theme::accent_texte())).truncate()).on_hover_text(trouve);
                    });
                    titre_groupe(ui, t.groupe, true);
                    self.groupe_ui(ui, t.groupe, sortie);
                }
            });
        });

        if onglet != self.settings.prefs_onglet {
            self.settings.prefs_onglet = onglet;
            let _ = self.settings.save();
        }
        if q != recherche(&ctx) {
            definir_recherche(&ctx, &q);
        }
    }

    /// Un groupe de réglages : le même dessin dans son onglet et dans les réponses à la recherche.
    fn groupe_ui(&mut self, ui: &mut egui::Ui, groupe: Groupe, sortie: &mut Sortie) {
        match groupe {
            Groupe::Theme => self.theme_prefs_ui(ui),
            Groupe::Panneaux => {
                let resp = ui.checkbox(&mut self.settings.panes_fit, "Ajuster les panneaux à leur contenu").on_hover_text(
                    "Les réglages de l'outil, les guides, la fiche et les effets prennent la hauteur de ce \
                     qu'ils montrent. Un bord tiré à la main reste où on l'a mis.",
                );
                if resp.changed() {
                    let _ = self.settings.save();
                }
                let bouton = ui.button("Réajuster les panneaux");
                if crate::bulles::bulle(bouton, "Réajuster les panneaux", "Remet chaque panneau à la hauteur de son contenu, même ceux dont tu avais tiré le bord à la main.", &self.raccourci("vue.reajuster_panneaux")).clicked() {
                    sortie.action = Some("vue.reajuster_panneaux");
                }
            }
            Groupe::Ouverture => {
                if ui
                    .checkbox(&mut self.settings.reopen_last, "Rouvrir le dernier projet au lancement")
                    .on_hover_text("Avec plusieurs onglets ouverts à la fermeture, ils reviennent tous, dans le même ordre.")
                    .changed()
                {
                    let _ = self.settings.save();
                }
                if ui
                    .checkbox(&mut self.settings.open_in_new_tab, "Ouvrir dans un nouvel onglet")
                    .on_hover_text("Ouvrir, Nouveau, un projet récent ou un import prennent un onglet à eux. Décoché, ils remplacent le projet affiché (après la question habituelle).")
                    .changed()
                {
                    let _ = self.settings.save();
                }
                ui.horizontal_wrapped(|ui| {
                    use crate::nouveau::OuvreSur;
                    ui.label("Un nouveau projet s'ouvre sur");
                    let avant = self.settings.new_project.ouvre_sur;
                    ui.radio_value(&mut self.settings.new_project.ouvre_sur, OuvreSur::Vignettes, "Vignettes")
                        .on_hover_text("La planche des Vignettes : tu dessines dans les Cases (d'origine).");
                    ui.radio_value(&mut self.settings.new_project.ouvre_sur, OuvreSur::Storyboard, "Storyboard")
                        .on_hover_text("L'espace Storyboard, avec le plan et sa timeline.");
                    if avant != self.settings.new_project.ouvre_sur {
                        let _ = self.settings.save();
                    }
                });
            }
            Groupe::Secours => self.autosave_menu_ui(ui),
            Groupe::Collage => self.collage_prefs_ui(ui),
            Groupe::Stylet => {
                let resp = ui.checkbox(&mut self.settings.system_cursor, "Croix du système sur le pinceau").on_hover_text(
                    "Affiche la croix de Windows à la pointe, en plus du cercle du pinceau : elle suit le \
                     stylet sans retard (le cercle, dessiné par SHOT, arrive une image plus tard).",
                );
                if resp.changed() {
                    let _ = self.settings.save();
                }
                ui.label(RichText::new("Le toucher du stylet se règle en bas des réglages du pinceau.").small().weak());
            }
            Groupe::Camera => self.passe_partout_pref_ui(ui),
            Groupe::Tremblement => self.tremblement_defaut_pref_ui(ui),
            Groupe::Poignees => {
                use crate::selbox::{CentreRotation, PointFixe};
                let avant = (self.settings.point_fixe, self.settings.rotation_autour);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Point fixe des poignées");
                    ui.radio_value(&mut self.settings.point_fixe, PointFixe::Opposee, "Poignée opposée")
                        .on_hover_text("Un coin ou un côté tiré (échelle, inclinaison) garde la poignée opposée en place.");
                    ui.radio_value(&mut self.settings.point_fixe, PointFixe::Centre, "Centre")
                        .on_hover_text("Un coin ou un côté tiré met à l'échelle autour du centre de la boîte (le pivot, s'il a été déplacé).");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Rotation autour de");
                    ui.radio_value(&mut self.settings.rotation_autour, CentreRotation::Pivot, "Pivot")
                        .on_hover_text("Juste hors d'un coin, la boîte tourne autour du pivot (le centre, sauf s'il a été déplacé).");
                    ui.radio_value(&mut self.settings.rotation_autour, CentreRotation::CoinOppose, "Coin opposé")
                        .on_hover_text("Juste hors d'un coin, la boîte tourne autour du coin opposé.");
                });
                ui.label(RichText::new("Ctrl tenu pendant le geste : l'autre point fixe. Aussi dans les réglages des outils Sélection et Transformer.").small().weak());
                if avant != (self.settings.point_fixe, self.settings.rotation_autour) {
                    let _ = self.settings.save();
                }
            }
            Groupe::Pistes => self.montage_prefs_ui(ui),
            Groupe::PlansNeufs => self.new_plan_duration_pref_ui(ui),
            Groupe::Vignettes => self.vignettes_prefs_ui(ui),
            Groupe::CadresCamera => self.cadres_camera_prefs_ui(ui),
            Groupe::Decor => self.decor_prefs_ui(ui),
            Groupe::Raccourcis => {
                sortie.jeu = crate::raccourcisui::preset_combo(ui, "jeu_raccourcis_prefs", self.settings.keymap_preset);
                ui.label(RichText::new(crate::raccourcisui::MERGE_RULE).small().weak());
                ui.horizontal_wrapped(|ui| {
                    let b = ui.button("Voir et changer les raccourcis…");
                    if crate::bulles::bulle(b, "Voir et changer les raccourcis", "La liste de toutes les commandes, avec une recherche et l'assignation d'une touche.", &self.raccourci("aide.raccourcis")).clicked() {
                        sortie.action = Some("aide.raccourcis");
                    }
                    let b = ui.button("Modifier le fichier à la main…");
                    if crate::bulles::bulle(b, "Modifier le fichier à la main", "Ouvre ton fichier personnel de raccourcis dans l'éditeur de texte.", "").clicked() {
                        self.edit_keymap();
                    }
                    let b = ui.button("Recharger");
                    if crate::bulles::bulle(b, "Recharger", "Relit le fichier de raccourcis après l'avoir modifié à la main.", "").clicked() {
                        self.reload_keymap();
                    }
                });
            }
            Groupe::Programmes => self.programmes_prefs_ui(ui, sortie),
            Groupe::Commentaires => {
                ui.horizontal(|ui| {
                    ui.label("Signer mes commentaires :");
                    let hint = crate::commentaires::author_name("");
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.settings.comment_author).hint_text(hint).desired_width(160.0));
                    let resp = crate::bulles::bulle(resp, "Signature des commentaires", "Le nom écrit sur les commentaires de la timeline. Vide : le nom de l'utilisateur Windows.", "");
                    if resp.lost_focus() {
                        let _ = self.settings.save();
                    }
                });
                ui.label(RichText::new("Vide : le nom de l'utilisateur Windows.").small().weak());
            }
            Groupe::Equipe => self.equipe_prefs_ui(ui),
            Groupe::Generation => self.generation_prefs_ui(ui),
            Groupe::Performance => self.performance_prefs_ui(ui),
            Groupe::Affichage => {
                ui.label(RichText::new(format!("Affichage : {}", self.display_info)).small().weak());
            }
            Groupe::Echelle => self.echelle_prefs_ui(ui),
            Groupe::ImagesAccueil => self.images_accueil_prefs_ui(ui),
            Groupe::Conseils => self.conseils_prefs_ui(ui),
            Groupe::StyletMain => {
                let b = ui.button("Régler le stylet à ma main…");
                if crate::bulles::bulle(b, "Régler le stylet à ma main", "Trace trois traits (léger, moyen, appuyé) : SHOT règle la courbe de pression du stylet sur ta main, après un aperçu et ta confirmation. Avec la souris, l'appui est toujours de 100 %.", &self.raccourci("aide.regler_stylet")).clicked() {
                    sortie.action = Some("aide.regler_stylet");
                }
            }
        }
    }

    /// Export > Programmes et modèles : ffmpeg et le projet modèle de Storyboard Pro, choisis une fois (ils se
    /// choisissaient aussi dans les fenêtres d'export, qui gardent leur bouton), et l'accès à la fenêtre Exporter.
    fn programmes_prefs_ui(&mut self, ui: &mut egui::Ui, sortie: &mut Sortie) {
        let ffmpeg = self.settings.ffmpeg.clone();
        let modele = self.settings.sboard_template.clone();
        egui::Grid::new("prefs_programmes").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            let l = ui.label("ffmpeg (vidéo, GIF, son)");
            crate::bulles::bulle(l, "ffmpeg", "Le programme qui fabrique les vidéos, les GIF et le son des exports. Par défaut SHOT le cherche tout seul à chaque export.", "");
            ui.horizontal(|ui| {
                match &ffmpeg {
                    Some(p) => ui.label(p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())).on_hover_text(p.display().to_string()),
                    None => ui.label(RichText::new("cherché tout seul").weak()),
                };
                let b = ui.button("Choisir…");
                if crate::bulles::bulle(b, "Choisir ffmpeg", "Indique où est ffmpeg.exe si SHOT ne le trouve pas.", "").clicked() {
                    self.choose_ffmpeg();
                }
                if ffmpeg.is_some() {
                    let b = ui.button("Automatique");
                    if crate::bulles::bulle(b, "Chercher tout seul", "Oublie le choix fait à la main : SHOT cherche ffmpeg à chaque export.", "").clicked() {
                        self.settings.ffmpeg = None;
                        self.ffmpeg = None;
                        let _ = self.settings.save();
                    }
                }
            });
            ui.end_row();
            let l = ui.label("Modèle Storyboard Pro");
            crate::bulles::bulle(l, "Modèle Storyboard Pro", "Un projet Storyboard Pro vide, à tes réglages (résolution, cadence, légendes) : l'export vers Storyboard Pro part d'une copie. Choisi une fois, gardé.", "");
            ui.horizontal(|ui| {
                match &modele {
                    Some(t) => ui.label(t.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned())).on_hover_text(t.display().to_string()),
                    None => ui.label(RichText::new("à choisir").weak()),
                };
                let b = ui.button("Choisir…");
                if crate::bulles::bulle(b, "Choisir le modèle", "Choisis le fichier .sboard vide qui sert de modèle.", "").clicked() {
                    let mut d = rfd::FileDialog::new().set_title("Projet modèle de Storyboard Pro").add_filter("Projet Storyboard Pro", &["sboard"]);
                    if let Some(dir) = modele.as_ref().and_then(|t| t.parent()).and_then(|p| p.parent()) {
                        d = d.set_directory(dir);
                    }
                    if let Some(p) = d.pick_file() {
                        self.settings.sboard_template = Some(p);
                        let _ = self.settings.save();
                    }
                }
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        let b = ui.button(format!("{}  Ouvrir la fenêtre Exporter…", ph::EXPORT));
        if crate::bulles::bulle(b, "Ouvrir la fenêtre Exporter", "Les exports (film, GIF, images, PDF, son…) : le dossier, le format et les colonnes se choisissent dans chaque fenêtre d'export, et SHOT les retient.", &self.raccourci("fichier.exporter")).clicked() {
            sortie.action = Some("fichier.exporter");
        }
    }
}
