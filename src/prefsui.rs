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

    /// « Opacité du passe-partout » : le voile autour du cadre de la caméra quand « Estomper le hors-champ » est
    /// allumé (`camtools.rs`). De 0 % (rien) à 100 % (le hors-champ noir) ; retenu dans les réglages.
    /// Montage > Planche des Vignettes : le nombre de rangées de cases pâles qu'on voit toujours, pour montrer où dessiner. Une
    /// rangée de plus apparaît dès qu'on dessine dans la dernière.
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
    }

    pub(crate) fn passe_partout_pref_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Opacité du passe-partout");
            let mut v = f32::from(self.settings.passe_partout);
            let resp = ui.add(egui::Slider::new(&mut v, 0.0..=100.0).suffix(" %").fixed_decimals(0));
            let resp = crate::bulles::bulle(
                resp,
                "Opacité du passe-partout",
                "Le voile qui estompe ce qui est hors du cadre de la caméra, quand « Estomper le hors-champ » est allumé. 0 % : aucun voile ; 100 % : le hors-champ noir.",
                &self.raccourci("camera.masque"),
            );
            if resp.changed() {
                self.settings.passe_partout = v.round().clamp(0.0, 100.0) as u8;
            }
            if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                let _ = self.settings.save();
            }
            if ui.small_button("Rétablir").on_hover_text("Revenir à l'opacité d'origine (27 %).").clicked() {
                self.settings.passe_partout = crate::camtools::PASSE_PARTOUT_DEFAUT;
                let _ = self.settings.save();
            }
        });
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
            Groupe::Pistes => self.montage_prefs_ui(ui),
            Groupe::Vignettes => self.vignettes_prefs_ui(ui),
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
