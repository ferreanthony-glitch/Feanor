use eframe::egui::{self, vec2, Align, Align2, Layout, RichText};

use crate::app::{App, Mode, Tool};
use crate::document::LayerKind;

/// Gestes à la souris et au stylet (hors fichier de raccourcis).
pub(crate) const MOUSE_GESTURES: [(&str, &str); 27] = [
    ("Clic droit sur une commande", "lui assigner un raccourci"),
    ("Alt + clic", "prélever une couleur"),
    ("Ctrl tenu sur le dessin", "le lasso le temps de le tenir"),
    ("Vignettes : Ctrl + glisser dans une case", "le lasso dans son dessin"),
    ("Vignettes : Ctrl + Maj + glisser dans une case", "le mouvement de caméra : déplacer un cadre, tirer un coin pour le zoom (Alt : un cadre de plus ; T tenue : pareil)"),
    ("Vignettes : Maj + glisser le cadre d'une case", "déplacer son plan (Alt en plus : en poser une copie)"),
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
    ("Montage : milieu d'un dessin d'un calque", "il emmène la suite de sa piste (Maj : seul ; Alt : le dupliquer, avec son propre dessin ; Ctrl+Maj : le placer avant ou après un autre, chacun garde sa durée)"),
    ("Montage : double-clic sur la piste d'un calque", "nouveau dessin (clé) à cette image"),
    ("Montage : Ctrl ou Maj + clic sur des dessins", "choisir plusieurs clés (Ctrl : une de plus ou de moins, sur n'importe quel calque ; Maj : le rectangle entre la clé de départ et celle-ci, tous calques d'entre compris)"),
    ("Montage : glisser dans le vide des pistes", "choisir toutes les clés que le rectangle touche, sur toutes les rangées qu'il couvre (Ctrl ou Maj : les ajouter)"),
    ("Montage : prendre une clé choisie", "toutes les clés choisies glissent, calque par calque (Maj : seules celles-ci ; Alt : les dupliquer ; Ctrl+Maj : les réordonner)"),
    ("Sélection : Ctrl+Maj + lasso, rectangle ou clic", "prendre sur tous les calques du plan à l'image courante ; leurs clés se choisissent aussi dans le montage"),
    ("Timeline : prise au bout de la règle", "changer la durée du plan"),
    ("Échap ou Ctrl+Z pendant un glisser", "abandonner le geste"),
];

/// Un menu de la barre qui parle d'un projet : sans projet ouvert, son titre est grisé et ne s'ouvre pas (`sansprojet.rs`).
/// Rend vrai quand le menu est à construire.
fn menu_actif(ui: &mut egui::Ui, sans_projet: bool, titre: &str) -> bool {
    if sans_projet {
        ui.add_enabled(false, egui::Button::new(titre).frame(false));
    }
    !sans_projet
}

/// Mode d'emploi de la boîte de sélection (Réglages de l'outil Sélection).
const BOX_NOTE: &str = "Boîte : glisser dedans pour déplacer (Maj : un seul axe) ; un coin ou le milieu d'un côté pour \
mettre à l'échelle, la poignée opposée restant en place (Maj : proportions ; Ctrl : l'autre point fixe, réglé par « Point \
fixe ») ; juste hors d'un coin pour tourner autour du pivot ou du coin opposé (réglé par « Rotation » ; Maj : par 15°) ; un bord entre les poignées pour incliner ; le point rond pour déplacer le pivot. \
Flèches : déplacer d'un pixel (Maj : dix).";

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

/// « (Alt+S) » d'un raccourci, pour une phrase ; rien s'il n'y en a pas. Les phrases d'aide lisent
/// ainsi la touche dans le fichier de raccourcis au lieu de l'écrire en dur.
pub(crate) fn between_parens(shortcut: &str) -> String {
    if shortcut.is_empty() {
        String::new()
    } else {
        format!(" ({shortcut})")
    }
}

impl App {
    /// Entrée de menu reliée à une action ; le raccourci affiché vient du fichier de raccourcis.
    /// Clic droit : lui assigner un raccourci.
    pub(crate) fn action_item(&mut self, ui: &mut egui::Ui, text: &str, action: &str) {
        let shortcut = self.keymap.label(action);
        // (Sans projet ouvert, une commande qui en vise un est grisée : `sansprojet.rs`.)
        let resp = ui.add_enabled(!self.action_bloquee(action), egui::Button::new(text).shortcut_text(&shortcut));
        if resp.clicked() {
            ui.close_menu();
            self.run_action(action);
        } else if resp.secondary_clicked() {
            ui.close_menu();
            self.start_assign(action);
        }
    }

    /// Storyboard > Plans : ce qui se fait sur les plans choisis.
    fn plans_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Nouveau plan avant le plan courant", "plan.nouveau_avant");
        self.action_item(ui, "Dupliquer le plan", "panneau.dupliquer");
        ui.separator();
        self.action_item(ui, "Copier les plans choisis", "panneau.copier");
        self.action_item(ui, "Coller les plans après le plan courant", "panneau.coller");
        self.action_item(ui, "Supprimer les plans choisis", "panneau.supprimer");
        ui.separator();
        self.action_item(ui, "Allonger le plan d'une image", "panneau.ajouter_image");
        self.action_item(ui, "Raccourcir le plan d'une image", "panneau.retirer_image");
        // Rogner : durée exacte, début et fin à la tête de lecture, contenu rogné (`rognageui.rs`).
        let cur = self.cur_panel;
        ui.menu_button("Rogner le plan", |ui| self.trim_menu_items(ui, cur));
        ui.separator();
        self.action_item(ui, "Couper le plan à l'image courante", "panneau.couper");
        self.action_item(ui, "Réunir avec le plan suivant", "panneau.fusionner_suivant");
        self.action_item(ui, "Réunir avec le plan précédent", "scene.joindre_precedente");
        self.action_item(ui, "Fusionner les plans choisis", "panneau.fusionner_choisis");
        self.action_item(ui, "Nouvelle séquence avec les plans choisis", "sequence.nouvelle_avec_plans");
        self.action_item(ui, "Changer la durée des plans choisis…", "montage.duree_plans");
        self.action_item(ui, "Retourner les plans choisis", "plan.retourner");
        ui.separator();
        self.action_item(ui, "Nouveau projet à partir des plans choisis", "projet.nouveau_depuis_plans");
    }

    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            // (Plus de logotype à gauche de Fichier : retiré à la demande d'Anthony le 2026-10-30.)
            // Menus courts (Anthony, 2026-10-18 : « certains menus contiennent trop de choses et
            // noient les informations principales ») : l'essentiel en premier niveau, le reste dans
            // un sous-menu ou une fenêtre (Exporter, Préférences, `prefsui.rs`) ; ce qui a déjà un
            // bouton toujours visible (vue sous le dessin) n'est plus répété. Toute commande reste
            // trouvable par la palette (Ctrl+K).
            // Sans projet ouvert (`sansprojet.rs`) : seuls Fichier et Aide servent ; les autres menus sont grisés.
            let sans_projet = self.sans_projet();
            ui.menu_button("Fichier", |ui| {
                self.action_item(ui, "Nouveau…", "fichier.nouveau");
                self.action_item(ui, "Ouvrir…", "fichier.ouvrir");
                ui.menu_button("Projets récents", |ui| self.recent_menu(ui));
                self.action_item(ui, "Enregistrer", "fichier.enregistrer");
                self.action_item(ui, "Enregistrer sous…", "fichier.enregistrer_sous");
                self.action_item(ui, "Afficher le projet dans l'Explorateur", "fichier.afficher_explorateur");
                self.action_item(ui, "Nouvel onglet", "onglet.nouveau");
                self.action_item(ui, "Fermer l'onglet", "fichier.fermer");
                self.action_item(ui, "Fermer les autres onglets", "onglet.fermer_autres");
                ui.separator();
                ui.menu_button("Importer", |ui| {
                    if ui.add_enabled(!sans_projet, egui::Button::new("Une image (calque bitmap)…")).clicked() {
                        ui.close_menu();
                        self.import_image();
                    }
                    self.action_item(ui, "Une vidéo (calque vidéo)…", "fichier.importer_video");
                    self.action_item(ui, "Une séquence d'images (calque vidéo)…", "fichier.importer_sequence");
                    self.action_item(ui, "Des images comme références…", "references.importer_image");
                    self.action_item(ui, "Une vidéo comme référence…", "references.importer_video");
                    ui.separator();
                    self.action_item(ui, "Un scénario, à garder dans le projet (Fountain, .fdx, .docx, texte)…", "fichier.importer_scenario");
                    self.action_item(ui, "Des notes de relecture (.json)…", "fichier.importer_notes");
                });
                self.action_item(ui, "Exporter…", "fichier.exporter");
                ui.separator();
                self.action_item(ui, "Réglages du projet…", "projet.proprietes");
                // Travailler à plusieurs (`cooperationui.rs`).
                if self.partage.is_some() {
                    self.action_item(ui, "Équipe…", "equipe.fenetre");
                } else {
                    self.action_item(ui, "Partager ce projet…", "fichier.partager");
                }
                self.action_item(ui, "Préférences…", "fichier.preferences");
                ui.separator();
                self.action_item(ui, "Quitter", "fichier.quitter");
            });
            if menu_actif(ui, sans_projet, "Édition") { ui.menu_button("Édition", |ui| {
                self.action_item(ui, "Annuler", "edition.annuler");
                self.action_item(ui, "Rétablir", "edition.retablir");
                self.action_item(ui, "Historique…", "vue.historique");
                ui.separator();
                self.action_item(ui, "Couper", "edition.couper");
                self.action_item(ui, "Copier", "edition.copier");
                self.action_item(ui, "Coller", "edition.coller");
                // « Coller sur place » fait la même chose que Coller (le collage pose déjà tout à sa
                // place) : il reste une commande de la palette, sans entrée de menu.
                self.action_item(ui, "Coller à la tête de lecture (clés)", "edition.coller_a_la_tete");
                self.action_item(ui, "Supprimer", "edition.supprimer");
                self.action_item(ui, "Vider le dessin", "dessin.vider");
                ui.separator();
                self.action_item(ui, "Tout sélectionner", "edition.tout_selectionner");
                self.action_item(ui, "Tout désélectionner", "edition.tout_deselectionner");
                ui.menu_button("Retourner la sélection", |ui| {
                    self.action_item(ui, "De gauche à droite", "selection.miroir_h");
                    self.action_item(ui, "De haut en bas", "selection.miroir_v");
                });
            }); }
            if menu_actif(ui, sans_projet, "Storyboard") { ui.menu_button("Storyboard", |ui| {
                self.action_item(ui, "Nouveau plan", "panneau.nouveau");
                self.action_item(ui, "Nouvelle séquence", "sequence.nouvelle");
                ui.separator();
                ui.separator();
                self.action_item(ui, "Aller à un plan…", "scene.aller_a");
                self.action_item(ui, "Fiche du plan", "panneau.proprietes");
                ui.menu_button("Plans", |ui| self.plans_menu(ui));
                self.action_item(ui, "Marque d'image ici (ajouter / retirer)", "marque.basculer");
                self.action_item(ui, "Nouveau commentaire ici", "commentaire.ajouter");
                self.action_item(ui, "Commentaires…", "vue.commentaires");
                ui.menu_button("Envois", |ui| {
                    self.action_item(ui, "Marquer un envoi…", "envoi.marquer");
                    self.action_item(ui, "Envois et plans modifiés…", "envoi.liste");
                    self.action_item(ui, "Plan modifié suivant", "envoi.modifie_suivant");
                    self.action_item(ui, "Plan modifié précédent", "envoi.modifie_precedent");
                    self.action_item(ui, "Choisir les plans modifiés", "envoi.modifies_choisir");
                    self.action_item(ui, "Journal des différences…", "envoi.journal");
                });
                ui.menu_button("Versions du plan", |ui| {
                    self.action_item(ui, "Nouvelle version, en dupliquant la version en cours", "version.nouvelle");
                    self.action_item(ui, "Nouvelle version, en partant d'un plan vide", "version.vide");
                    self.action_item(ui, "Version précédente", "version.precedente");
                    self.action_item(ui, "Version suivante", "version.suivante");
                    ui.separator();
                    self.action_item(ui, "Comparer le plan…", "comparer.ouvrir");
                    ui.separator();
                    self.purge_items(ui);
                });
                self.variantes_sous_menu(ui);
                ui.separator();
                let lock = if self.doc.storyboard_locked { "Déverrouiller le storyboard" } else { "Verrouiller le storyboard" };
                self.action_item(ui, lock, "storyboard.verrouiller");
                let numbering = if self.doc.numbering_locked { "Déverrouiller la numérotation des plans" } else { "Verrouiller la numérotation des plans" };
                self.action_item(ui, numbering, "numerotation.verrouiller");
                self.action_item(ui, "Renuméroter les plans…", "numerotation.renumeroter");
                ui.separator();
                let label = match self.mode {
                    Mode::Storyboard => "Animer ce plan",
                    Mode::Animation => "Retour au storyboard",
                };
                self.action_item(ui, label, "mode.basculer");
            }); }
            if menu_actif(ui, sans_projet, "Son") { ui.menu_button("Son", |ui| self.sound_menu(ui)); }
            if menu_actif(ui, sans_projet, "Calques") { ui.menu_button("Calques", |ui| self.layer_ops_menu(ui)); }
            if menu_actif(ui, sans_projet, "Animation") { ui.menu_button("Animation", |ui| {
                self.action_item(ui, "Lecture / pause", "lecture.basculer");
                ui.menu_button("Aller à", |ui| {
                    self.action_item(ui, "Image précédente", "image.precedente");
                    self.action_item(ui, "Image suivante", "image.suivante");
                    self.action_item(ui, "Première image du plan", "image.premiere");
                    self.action_item(ui, "Dernière image du plan", "image.derniere");
                });
                ui.separator();
                self.action_item(ui, "Nouveau dessin", "dessin.nouveau");
                self.action_item(ui, "Dupliquer le dessin", "dessin.dupliquer");
                self.action_item(ui, "Allonger l'exposition", "exposition.allonger");
                self.action_item(ui, "Raccourcir l'exposition", "exposition.raccourcir");
                self.action_item(ui, "Retirer le dessin de la timeline", "dessin.supprimer");
                ui.separator();
                ui.menu_button("Images", |ui| self.images_menu(ui));
                ui.separator();
                self.action_item(ui, "Colorier par zones…", "calque.colorier_zones");
                self.action_item(ui, "Propager la peinture du calque…", "calque.propager");
            }); }
            if menu_actif(ui, sans_projet, "Affichage") { ui.menu_button("Affichage", |ui| {
                // Zoom, rotation, miroir, pelure, table lumineuse : boutons sous le dessin.
                self.action_item(ui, "Ajuster à la fenêtre", "vue.ajuster");
                self.action_item(ui, "Zoom 100 %", "vue.reinit_zoom");
                self.action_item(ui, "Remettre la vue droite", "vue.reinit_rotation");
                ui.separator();
                ui.menu_button("Caméra", |ui| {
                    self.action_item(ui, "Vue à travers la caméra", "camera.vue");
                    self.action_item(ui, "Passe-partout", "camera.masque");
                    self.action_item(ui, "Zones de sécurité", "vue.cadres_securite");
                    ui.separator();
                    self.action_item(ui, "Réinitialiser la caméra du plan", "camera.reinitialiser");
                });
                ui.menu_button("Plateau", |ui| {
                    self.action_item(ui, "Espace 3D", "espace3d.basculer");
                    self.action_item(ui, "Vue de dessus (profondeur des calques)", "vue.dessus");
                });
                ui.menu_button("Guides", |ui| {
                    self.action_item(ui, "Afficher et utiliser", "guide.activer");
                    self.action_item(ui, "Aligner les traits", "guide.aligner");
                    self.action_item(ui, "Verrouiller", "guide.verrouiller");
                    self.action_item(ui, "Lignes de référence", "guide.lignes_reference");
                });
                ui.separator();
                let barre = if self.barre_perso_visible() { "Cacher les pastilles des brosses dédiées" } else { "Montrer les pastilles des brosses dédiées" };
                self.action_item(ui, barre, "perso.barre");
                self.workspace_menu(ui);
                self.action_item(ui, "Mode concentration (le dessin seul)", "vue.concentration");
                self.action_item(ui, "Plein écran", "vue.plein_ecran");
                self.action_item(ui, "Performance (temps de l'interface)", "vue.performance");
            }); }
            // Les fonctions à l'essai, rangées ensemble (Anthony, 2026-10-20), cachées d'origine (premier contact, 2026-10-30) : le menu
            // Labo ne paraît qu'avec Préférences > Général > « Afficher les fonctions expérimentales ». Ce qui était mûr (la propagation
            // de la peinture) est sorti à sa place, dans le menu Animation.
            if self.settings.experimental && menu_actif(ui, sans_projet, "Labo") { ui.menu_button("Labo", |ui| {
                ui.label(RichText::new("Fonctions expérimentales : elles peuvent encore changer.").weak());
                ui.separator();
                self.action_item(ui, "Suggestions sonores…", "vue.suggestions_sonores");
                self.action_item(ui, "Lire le storyboard (proposer des sons)", "suggestion.lire");
                ui.separator();
                // Mon panneau : l'essai d'un panneau à soi (commandes, brosses, outils perso ; `monpanneau.rs`).
                self.action_item(ui, "Mon panneau (essai)", crate::monpanneau::ACTION);
                ui.separator();
                let rec = if self.record_strokes.is_some() { "Arrêter d'enregistrer mes traits" } else { "Enregistrer mes traits (banc d'essai du moteur)" };
                self.action_item(ui, rec, "pinceau.enregistrer_traits");
            }); }
            ui.menu_button("Aide", |ui| {
                self.action_item(ui, "Palette de commandes…", "aide.palette");
                self.action_item(ui, "Raccourcis clavier…", "aide.raccourcis");
                self.action_item(ui, "Régler le stylet à ma main…", "aide.regler_stylet");
                self.action_item(ui, "Visite rapide « Premier jour »…", "aide.visite");
                self.action_item(ui, "Revoir les conseils", "aide.conseils");
                self.action_item(ui, "Régler FEANOR pour cette machine…", "aide.diagnostic");
                ui.separator();
                self.action_item(ui, "À propos de FEANOR…", "aide.a_propos");
            });

            // (Sans projet, il n'y a pas d'espace de travail à choisir : les trois mots ne paraissent pas.)
            if !sans_projet { ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Les trois espaces de travail restent écrits en toutes lettres, dans l'ordre du
                // travail (de gauche à droite : Vignettes, Storyboard, Montage) : on voit d'un coup
                // d'œil où l'on est. Passer de l'un à l'autre est instantané : le plan, l'image, la
                // sélection et le projet ne changent pas, seule la disposition change.
                // (Ce panneau se remplit de droite à gauche : le dernier posé est le premier à gauche.)
                use crate::workspace::Espace;
                let on = self.espace == Espace::Montage;
                let montage = ui.selectable_label(on, crate::bulles::encre_etat(RichText::new(" Montage ").strong(), on));
                let montage = self.aide(montage, "Montage", "Le film en grand, avec toutes ses pistes : couper, rogner, transitions, son, relecture. Tab : retour au storyboard.", "espace.montage");
                if on {
                    crate::theme::souligner_actif(ui, montage.rect);
                }
                if montage.clicked() && !on {
                    self.set_espace(Espace::Montage);
                }
                let on = self.espace == Espace::Storyboard;
                let board = ui.selectable_label(on, crate::bulles::encre_etat(RichText::new(" Storyboard ").strong(), on));
                let board = self.aide(board, "Storyboard", "Monter les plans et les séquences.", "mode.basculer");
                if on {
                    crate::theme::souligner_actif(ui, board.rect);
                }
                if board.clicked() && !on {
                    self.set_espace(Espace::Storyboard);
                }
                let on = self.espace == Espace::Vignettes;
                let vignettes = ui.selectable_label(on, crate::bulles::encre_etat(RichText::new(" Vignettes ").strong(), on));
                let vignettes = self.aide(vignettes, "Vignettes", "La planche seule : dessiner dans les cases, avec le pinceau, la couleur et les personnages. Ctrl + Maj + glisser un cadre : mouvement de caméra ; Ctrl + glisser : lasso ; Maj + Alt + glisser le cadre d'une case : copie du plan.", "espace.vignettes");
                if on {
                    crate::theme::souligner_actif(ui, vignettes.rect);
                }
                if vignettes.clicked() && !on {
                    self.set_espace(Espace::Vignettes);
                }
                // En animation (on y entre par un double-clic sur un plan), il n'y a pas d'onglet : ce bouton, à gauche des
                // onglets, ramène au storyboard, sur le même plan.
                if self.espace == Espace::Animation {
                    self.bouton_retour_storyboard(ui);
                }
            }); }
        });
    }

    /// Le bouton « Retour au storyboard » de la barre du haut, en animation : l'espace Storyboard, sur le même plan.
    pub(crate) fn bouton_retour_storyboard(&mut self, ui: &mut egui::Ui) -> egui::Response {
        use egui_phosphor::regular as ph;
        let resp = ui.button(RichText::new(format!("{} Retour au storyboard", ph::ARROW_BEND_UP_LEFT)).strong());
        let resp = self.aide(resp, "Retour au storyboard", "Quitter l'animation et revenir au storyboard, sur ce plan.", "mode.basculer");
        if resp.clicked() {
            self.set_espace(crate::workspace::Espace::Storyboard);
        }
        resp
    }

    /// Panneau Outils : une icône par outil ; nom, raccourci et description dans la bulle.
    pub(crate) fn tool_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(crate::barres::ECART, crate::barres::ECART);
            for tool in Tool::ALL {
                let icon = RichText::new(tool.icon()).size(crate::barres::ICONE);
                let resp = ui.add_sized(crate::barres::CASE, egui::SelectableLabel::new(self.tool == tool, icon));
                if self.aide(resp, tool.label(), tool.description(), tool.action()).clicked() {
                    self.set_tool(tool);
                }
            }
        });
        // Sur la planche des Vignettes : le stylo et la gomme d'annotation (`annotationsui.rs`).
        if self.board_shown() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(crate::barres::ECART, crate::barres::ECART);
                self.annot_outils_ligne(ui, crate::barres::CASE, crate::barres::ICONE);
            });
        }
    }
    /// « Plus de réglages » de l'outil courant : une section repliée (retenue outil par outil dans les réglages de
    /// l'application, `Panneau::plus_outils`) sous les deux ou trois réglages qui servent le plus (passe de
    /// simplicité, 2026-10-21). Le pinceau, le crayon et la gomme ont la leur, sous l'aperçu du trait
    /// (`brush_settings_ui`).
    pub(crate) fn plus_de_reglages(&mut self, ui: &mut egui::Ui, contenu: impl FnOnce(&mut App, &mut egui::Ui)) {
        // Sélection et Transformer : le point fixe des poignées et le centre des rotations, toujours à portée, juste
        // au-dessus (`point_fixe_ui`).
        if matches!(self.tool, Tool::Select | Tool::SelectColor | Tool::Transform) {
            self.point_fixe_ui(ui);
        }
        let outil = self.tool.action();
        let mut ouvert = self.pinceaux.plus_ouvert(outil);
        if crate::repli::entete(ui, &mut ouvert, "Plus de réglages", "Les réglages qui servent moins souvent, et le mode d'emploi de l'outil.") {
            self.pinceaux.regler_plus(outil, ouvert);
        }
        if ouvert {
            contenu(self, ui);
        }
    }

    /// Sous l'aperçu du trait du pinceau et du crayon, au-dessus de « Plus de réglages », toujours à portée (Anthony,
    /// 2026-10-23, puis 2026-10-28) : « Peindre derrière » (`paint_behind`, `brushfill`), « Peint sur l'aplat »
    /// (`paint_on_aplat`, `aplatmulti.rs` ; exclusifs l'un de l'autre) et l'aplat sous le trait du calque courant (le même
    /// bouton que sur sa ligne du panneau Calques, `fond_layer_button`). « Remplir la forme » n'existe plus : il faisait
    /// double emploi avec l'aplat.
    pub(crate) fn brush_extras_ui(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let behind_key = self.raccourci("pinceau.derriere");
        let on_key = self.raccourci("pinceau.sur_aplat");
        ui.horizontal(|ui| {
            let resp = crate::bulles::interrupteur_a(
                ui,
                &mut self.paint_behind,
                ph::SELECTION_BACKGROUND,
                "Peindre derrière",
                "La peinture passe sous ce qui est déjà dessiné sur le calque. Éteint « Peint sur l'aplat ».",
                &behind_key,
                "pinceau.derriere",
            );
            if resp.changed() && self.paint_behind {
                self.paint_on_aplat = false;
            }
            let resp = crate::bulles::interrupteur_a(
                ui,
                &mut self.paint_on_aplat,
                ph::SQUARE_HALF_BOTTOM,
                "Peint sur l'aplat",
                "Le pinceau et le crayon peignent directement dans l'aplat du dessin (le calque de couleur du calque, sinon l'aplat sous le trait, créé au besoin), pas sur le trait. Éteint « Peindre derrière ».",
                &on_key,
                "pinceau.sur_aplat",
            );
            if resp.changed() && self.paint_on_aplat {
                self.paint_behind = false;
            }
            let li = self.cur_layer;
            if let Some(resp) = self.fond_layer_button(ui, li, false) {
                if resp.clicked() {
                    self.toggle_fond(li);
                }
            }
        });
    }

    /// Panneau des réglages de l'outil courant.
    pub(crate) fn tool_options_panel(&mut self, ui: &mut egui::Ui) {
        // L'Espace 3D a ses propres réglages (`espace3dui.rs`).
        if self.espace3d.on && !self.camera_view {
            self.espace3d_options_panel(ui);
            return;
        }
        // Le stylo ou la gomme d'annotation de la planche a ses propres réglages (`annotationsui.rs`).
        if self.annot_actif() {
            self.annot_reglages_ui(ui);
            return;
        }
        let vector = self.layer().kind == LayerKind::Vector;
        ui.horizontal(|ui| {
            ui.heading(format!("{}  {}", self.tool.icon(), self.tool.label()));
            ui.label(
                RichText::new(if vector { "calque vectoriel" } else { "calque bitmap" })
                    .small()
                    .weak(),
            );
        });
        // Sous-ligne d'aplat choisie : « Aplat de <calque> » et ses réglages (couleur, débord, trous) d'abord ; l'outil et
        // sa brosse suivent, ce sont eux qui travaillent l'aplat.
        self.aplat_section_ui(ui);
        match self.tool {
            // Pinceau, crayon, gomme : la taille, l'opacité et le nom de la brosse en haut ; sous l'aperçu du trait,
            // « Peint sur l'aplat » et l'aplat du calque (pinceau et crayon), puis « Plus de réglages » (replié
            // d'office) : les catégories de la brosse et le mode d'emploi. Le mode « Griffonner » d'un calque de couleur
            // reste à portée. (Le tremblement du trait se règle dans le panneau Effets : plus de ligne ici, 2026-10-23.)
            Tool::Brush | Tool::Mixer => {
                if self.tool == Tool::Mixer {
                    // Le Mélangeur est le pinceau courant en « Lisser » : mêmes taille, dureté, espacement, grain.
                    note(ui, "Mélangeur : la brosse du pinceau, en « Lisser » : elle étale ce qui est peint sans poser de couleur. « Couleur et mélange » règle l'entraînement.");
                }
                self.brush_summary_ui(ui);
                self.brush_settings_ui(ui);
                // Calque de couleur : le mode « Griffonner » (`calquecouleurui.rs`).
                self.griffonner_options_ui(ui);
                if vector && self.pinceaux.reglages_ouverts {
                    let key = between_parens(&self.raccourci("outil.selection"));
                    note(ui, &format!("Le trait reste modifiable avec l'outil Sélection{key}, même texturé (pointe en \
                          image, grain du papier)."));
                }
            }
            Tool::Pencil => {
                self.brush_summary_ui(ui);
                self.brush_settings_ui(ui);
                self.griffonner_options_ui(ui);
                if vector && self.pinceaux.reglages_ouverts {
                    let key = between_parens(&self.raccourci("outil.selection"));
                    note(ui, &format!("Le trait reste modifiable avec l'outil Sélection{key}."));
                }
            }
            Tool::Eraser => {
                self.brush_summary_ui(ui);
                if vector {
                    crate::bulles::interrupteur(
                        ui,
                        &mut self.vector_erase_whole,
                        egui_phosphor::regular::PATH,
                        "Effacer des traits entiers",
                        "La gomme retire chaque trait touché en entier au lieu de le couper.",
                        "",
                    );
                    if self.pinceaux.reglages_ouverts {
                        note(
                            ui,
                            "Sur un calque vectoriel, la gomme coupe les traits. Les zones remplies \
                             s'effacent en mode « traits entiers » ou par la sélection.",
                        );
                    }
                }
                self.brush_settings_ui(ui);
                self.griffonner_options_ui(ui);
            }
            // Le pot de peinture : la tolérance et d'où viennent les contours en haut ; le débord et le
            // mode d'emploi dans « Plus de réglages ».
            Tool::Fill => {
                self.griffonner_options_ui(ui);
                ui.add(egui::Slider::new(&mut self.fill.tolerance, 0.0..=1.0).text("Tolérance"));
                ui.horizontal(|ui| {
                    use egui_phosphor::regular as ph;
                    ui.label("Contours pris sur");
                    let sample = &mut self.fill.sample_all;
                    crate::bulles::choix(ui, sample, false, ph::STACK_SIMPLE, "Le calque courant", "Les contours du seul calque où l'on remplit.", "");
                    crate::bulles::choix(ui, sample, true, ph::STACK, "Tous les calques visibles", "Trait sur un calque, couleurs sur un autre.", "");
                });
                self.plus_de_reglages(ui, |app, ui| {
                    ui.add(egui::Slider::new(&mut app.fill.expand, 0..=4).text("Débord (px)"));
                    if vector {
                        note(ui, "Sur un trait ou une zone déjà peinte : change sa couleur.");
                    } else {
                        note(
                            ui,
                            "Astuce : trait sur un calque, couleurs sur un calque en dessous, \
                             contours pris sur tous les calques.",
                        );
                    }
                });
            }
            Tool::Gradient => self.degrade_options_ui(ui),
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline => self.shape_options_ui(ui),
            Tool::Text => self.text_options_ui(ui),
            Tool::Perspective => self.deform_options_ui(ui),
            Tool::Grab | Tool::Pull => self.sculpt_options_ui(ui),
            Tool::Select => self.selection_ui(ui, true),
            Tool::SelectColor => self.color_select_ui(ui),
            Tool::Cutter => {
                ui.label("Glisse en travers des bouts de traits à enlever, ou clique sur l'un d'eux.");
                note(
                    ui,
                    "Calque vectoriel : chaque trait touché est coupé là où il croise les autres traits (et lui-même) ; \
                     seuls les morceaux touchés partent. Un trait qui ne croise rien part en entier. Ctrl+Z annule.",
                );
            }
            Tool::Picker => {
                ui.label("Clique sur le dessin pour prélever une couleur.");
                note(ui, "Avec les autres outils : Alt + clic.");
            }
            Tool::Pan => {
                ui.label("Glisse pour déplacer la vue.");
                note(ui, "Sinon : Espace + glisser, ou bouton du milieu.");
            }
            Tool::Transform => {
                let peers = self.transform_peers().len();
                if peers >= 2 {
                    ui.label(RichText::new(format!("{peers} calques choisis : ils se déplacent, tournent et grandissent ensemble.")).strong());
                    note(ui, "Un coin agrandit tous les calques en gardant les proportions ; l'étirement par un côté se fait sur un seul calque. Ctrl+R remet tous les calques choisis en place. Les calques cachés ou verrouillés restent où ils sont.");
                }
                // Position, taille et angle en haut ; les clés de position (animer le calque) et le mode d'emploi dans
                // « Plus de réglages ».
                self.transform_fields(ui);
                self.plus_de_reglages(ui, |app, ui| {
                    app.pos_keys_ui(ui);
                    note(
                        ui,
                        "Glisser dans la boîte : déplacer le calque (Maj : à l'horizontale ou à la \
                         verticale). Coin : agrandir ou réduire, le coin opposé restant en place (Maj : en \
                         gardant les proportions ; Ctrl : l'autre point fixe, réglé par « Point fixe »). Milieu \
                         d'un côté : étirer dans un seul sens. Poignée ronde : tourner autour du centre ; juste \
                         hors d'un coin : tourner autour du centre ou du coin opposé (« Rotation ») ; Maj : par 15°. \
                         Pour courber le dessin, l'outil Déformer a son mode Enveloppe. \
                         Les dessins ne sont pas modifiés : on peut annuler la transformation à tout moment.",
                    );
                });
            }
            Tool::Camera => self.camera_panel(ui),
        }
        // (Un mode d'emploi de plus : seulement quand « Plus de réglages » est déplié.)
        if self.pinceaux.reglages_ouverts && self.current_brush().is_some_and(|b| b.pressure_size || b.pressure_opacity) {
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

    /// Options de l'outil Sélection sur un calque bitmap (`lasso` : avec le choix lasso /
    /// rectangle et son mode d'emploi ; sans pour la sélection par couleur).
    fn bitmap_selection_ui(&mut self, ui: &mut egui::Ui, lasso: bool) {
        use crate::bulles::choix;
        use egui_phosphor::regular as ph;
        if lasso {
            ui.horizontal(|ui| {
                choix(ui, &mut self.select_rect, false, ph::LASSO, "Lasso", "Entourer la zone à main levée.", "");
                choix(ui, &mut self.select_rect, true, ph::RECTANGLE_DASHED, "Rectangle", "Tirer un rectangle.", "");
            });
        }
        let has = self.bsel.is_some();
        ui.label(match self.bsel.as_ref().map(|s| s.lifted.is_some()) {
            None => "Rien de sélectionné.",
            Some(false) => "Zone sélectionnée.",
            Some(true) => "Zone soulevée : elle sera posée en quittant la sélection.",
        });
        // En haut : ce qui sert le plus. Une commande qui ne peut pas servir (rien de sélectionné, rien de copié) se
        // cache au lieu de se griser (2026-10-21).
        ui.horizontal_wrapped(|ui| {
            if self.bouton(ui, ph::SELECTION_ALL, "Tout sélectionner", "Tout le dessin.", "edition.tout_selectionner").clicked() {
                self.bitmap_select_all();
            }
            if has && self.bouton(ui, ph::COPY, "Copier", "Copier la zone.", "edition.copier").clicked() {
                self.bitmap_copy();
            }
            if has && self.bouton(ui, ph::SCISSORS, "Couper", "Copier la zone et l'effacer.", "edition.couper").clicked() {
                self.bitmap_cut();
            }
            if self.pixel_clipboard.is_some() && self.bouton(ui, ph::CLIPBOARD, "Coller", "Coller la zone copiée.", "edition.coller").clicked() {
                self.bitmap_paste();
            }
            if has && self.bouton(ui, ph::PAINT_BUCKET, "Remplir", "Remplir la zone de la couleur courante (le calque verrouillé refuse ; « préserver la transparence » ne change que la couleur de ce qui est dessiné).", "selection.remplir").clicked() {
                self.bitmap_fill_selection();
            }
        });
        self.plus_de_reglages(ui, |app, ui| {
            if has {
                ui.horizontal_wrapped(|ui| {
                    if app.bouton(ui, ph::FLIP_HORIZONTAL, "Miroir horizontal", "Retourner la zone de gauche à droite.", "selection.miroir_h").clicked() {
                        app.bitmap_flip(true);
                    }
                    if app.bouton(ui, ph::FLIP_VERTICAL, "Miroir vertical", "Retourner la zone de haut en bas.", "selection.miroir_v").clicked() {
                        app.bitmap_flip(false);
                    }
                    if app.bouton(ui, ph::ERASER, "Effacer dedans", "Effacer ce qui est dans la zone.", "").clicked() {
                        app.bitmap_erase(true);
                    }
                    if app.bouton(ui, ph::SELECTION_INVERSE, "Effacer autour", "Effacer tout ce qui est hors de la zone.", "").clicked() {
                        app.bitmap_erase(false);
                    }
                    if app.bouton(ui, ph::CHECK, "Poser", "Reposer la zone et quitter la sélection (Échap).", "").clicked() {
                        app.drop_bitmap_selection();
                    }
                });
                app.box_fields_ui(ui);
            }
            if lasso {
                note(ui, BOX_NOTE);
                note(ui, "Glisser dans le vide : lasso ou rectangle (Maj : ajouter). Un clic sur un dessin choisit sa forme (Ctrl+Maj + clic : en ajouter une autre, sur tous les calques). Échap ou autre outil : poser la zone.");
            }
        });
    }

    pub(crate) fn selection_ui(&mut self, ui: &mut egui::Ui, lasso: bool) {
        use egui_phosphor::regular as ph;
        // Plusieurs calques choisis, ou une sélection qui les couvre : `multitools.rs`.
        if lasso && (self.multi_wanted() || self.multi.sel.is_some()) {
            self.multi_selection_ui(ui, lasso);
            return;
        }
        if self.layer().kind != LayerKind::Vector {
            self.bitmap_selection_ui(ui, lasso);
            return;
        }
        let n = self.vsel.as_ref().map_or(0, |s| s.items.len());
        ui.label(if n == 0 {
            "Rien de sélectionné.".to_string()
        } else {
            format!("{n} élément(s) sélectionné(s).")
        });
        let has = n > 0;
        // En haut : ce qui sert le plus. Une commande qui ne peut pas servir (rien de sélectionné, rien de copié) se
        // cache au lieu de se griser (2026-10-21).
        ui.horizontal_wrapped(|ui| {
            if self.bouton(ui, ph::SELECTION_ALL, "Tout sélectionner", "Tous les éléments du dessin.", "edition.tout_selectionner").clicked() {
                self.select_all();
            }
            if has && self.bouton(ui, ph::COPY, "Copier", "", "edition.copier").clicked() {
                self.copy_selection();
            }
            if !self.clipboard.is_empty() && self.bouton(ui, ph::CLIPBOARD, "Coller", "", "edition.coller").clicked() {
                self.paste();
            }
            if has && self.bouton(ui, ph::TRASH, "Supprimer", "", "edition.supprimer").clicked() {
                self.delete_selection();
            }
        });
        self.plus_de_reglages(ui, |app, ui| {
            if has {
                ui.horizontal_wrapped(|ui| {
                    if app.bouton(ui, ph::DROP, "Couleur courante", "Donner la couleur courante aux éléments choisis.", "").clicked() {
                        app.recolor_selection();
                    }
                    if app.bouton(ui, ph::PLUS_CIRCLE, "Trait plus épais", "", "").clicked() {
                        app.scale_selection_width(1.25);
                    }
                    if app.bouton(ui, ph::MINUS_CIRCLE, "Trait plus fin", "", "").clicked() {
                        app.scale_selection_width(0.8);
                    }
                    if app.bouton(ui, ph::FLIP_HORIZONTAL, "Miroir horizontal", "Retourner de gauche à droite.", "selection.miroir_h").clicked() {
                        app.flip_selection(true);
                    }
                    if app.bouton(ui, ph::FLIP_VERTICAL, "Miroir vertical", "Retourner de haut en bas.", "selection.miroir_v").clicked() {
                        app.flip_selection(false);
                    }
                    if app.bouton(ui, ph::ARROW_LINE_UP, "Devant", "Passer devant les autres éléments du dessin.", "").clicked() {
                        app.order_selection(true);
                    }
                    if app.bouton(ui, ph::ARROW_LINE_DOWN, "Derrière", "Passer derrière les autres éléments du dessin.", "").clicked() {
                        app.order_selection(false);
                    }
                });
                app.box_fields_ui(ui);
            }
            if lasso {
                note(ui, BOX_NOTE);
                note(ui, "Clic : un élément. Glisser dans le vide : lasso. Maj : ajouter / retirer. Ctrl+Maj : prendre sur tous les calques du plan (pas seulement les calques choisis).");
            }
        });
    }
    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        // Sans projet ouvert : ni format, ni zoom (il n'y a rien à mesurer), seulement le message (`sansprojet.rs`).
        if self.sans_projet() {
            ui.horizontal(|ui| {
                if self.status != crate::sansprojet::STATUT_DE_DEPART {
                    self.status_message(ui);
                } else {
                    // (La barre garde sa hauteur : rien ne bouge quand un message paraît.)
                    ui.allocate_space(vec2(0.0, ui.spacing().interact_size.y));
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            // À gauche, seulement ce qui sert : le format du film et le zoom. L'espace de travail est l'onglet allumé,
            // le plan est écrit sur le dessin et dans l'en-tête du montage ou de la timeline, la rotation et le
            // miroir sont des icônes allumées sous le dessin, la position du curseur n'intéresse personne ici.
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
                ui.label(RichText::new(hand).color(crate::theme::accent_texte())).on_hover_text(
                    "Ce panneau a la main (encadré en bleu) : les raccourcis, copier, coller et supprimer le visent. Un clic dans le dessin la rend au dessin.",
                );
                ui.separator();
            }
            ui.label(format!("{}×{} · {} i/s", self.doc.frame[0], self.doc.frame[1], self.doc.fps)).on_hover_text(format!(
                "Image finale {}×{} px à {} images par seconde ; feuille de dessin {}×{} px (Réglages du film).",
                self.doc.frame[0], self.doc.frame[1], self.doc.fps, self.doc.width, self.doc.height
            ));
            // Sélection sur plusieurs calques : « 3 calques, 42 éléments ».
            if let Some(summary) = self.multi_summary() {
                ui.separator();
                ui.label(RichText::new(summary).color(crate::theme::accent_texte())).on_hover_text(
                    "Ce qui est choisi sur les calques choisis : une seule boîte, un seul geste, une seule étape d'annulation.",
                );
            }
            ui.separator();
            ui.label(format!("{:.0} %", self.view.zoom * self.ppp * 100.0)).on_hover_text("Zoom de la vue (la loupe, sous le dessin, le règle).");
            self.fusion_status_ui(ui);
            self.save_status_ui(ui);
            self.recovery_status_ui(ui);
            self.cooperation_status_ui(ui);
            // Pendant la lecture : la cadence vraiment affichée (orange si la machine ne suit pas).
            if let (true, Some((rate, skipped))) = (self.playing, self.play_meter.rate) {
                ui.separator();
                let mut text = format!("Lecture {} i/s", format!("{rate:.1}").replace('.', ","));
                if skipped > 0 {
                    let s = if skipped > 1 { "s" } else { "" };
                    text += &format!(" · {skipped} image{s} sautée{s}");
                }
                let slow = skipped > 0 || rate < self.doc.fps as f32 * 0.95;
                // (Pastille braise devant un texte neutre quand la machine ne suit pas.)
                if slow {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, crate::theme::braise());
                }
                ui.label(RichText::new(text)).on_hover_text(
                    "Images réellement affichées pendant la dernière seconde de lecture. Si la machine \
                     ne suit pas, des images sont sautées pour rester calé sur le son.",
                );
            }
            ui.separator();
            self.status_message(ui);
        });
    }

    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        // Messages : historique et notifications (`messages.rs`).
        self.track_messages(ctx);
        self.messages_ui(ctx);
        // Exporter… et Préférences… (`prefsui.rs`).
        self.export_hub_window(ctx);
        self.preferences_window(ctx);
        // Renuméroter les plans : la confirmation ; Exporter pour le montage : sa fenêtre.
        self.renumber_window(ctx);
        self.editorial_window(ctx);
        // Fiche technique (tableur) : sa fenêtre (`ficheui.rs`).
        self.fiche_window(ctx);
        // Pipeline de production : liste des plans, paquet de livraison, noms et production, modèles d'épisode.
        self.liste_plans_window(ctx);
        self.paquet_window(ctx);
        self.kitsu_window(ctx);
        self.noms_window(ctx);
        self.modele_episode_window(ctx);
        // Vignettes : « Effacer les annotations ? » (`annotationsui.rs`).
        self.annot_fenetre(ctx);
        // Vignettes : les images déposées sur la planche (`deposer.rs`).
        self.depot_tick(ctx);
        // Exporter la planche des vignettes et le color script (`plancheui.rs`, `colorscriptui.rs`).
        self.planche_fenetre(ctx);
        self.colorscript_fenetre(ctx);
        // Relecture par le réalisateur : exporter la page, importer ses notes (`relectureui.rs`).
        self.relecture_windows(ctx);
        // Suggestions sonores : confirmation d'envoi en ligne, « Modifier », saisie d'une clé.
        self.suggestions_windows(ctx);
        // À propos de SHOT (`aproposui.rs`).
        self.about_window_ui(ctx);
        // Diagnostic des performances et sa proposition du premier lancement (`diagnosticui.rs`).
        self.diagnostic_window(ctx);
        self.diagnostic_offer(ctx);
        // Écran d'accueil, sans projet ouvert : la page est la zone de travail (`accueil.rs`) ; ici, il se range quand un projet arrive.
        self.accueil_suivi();
        self.new_layer_window(ctx);
        // Préréglages d'effets (`prereglagesui.rs`).
        self.effect_dialog_window(ctx);
        self.groups_window_ui(ctx);
        self.properties_window_ui(ctx);
        self.onion_window_ui(ctx);
        self.flips_window_ui(ctx);

        self.tween_window_ui(ctx);
        // Importer un scénario (`scenarioui.rs`).
        self.scenario_window_ui(ctx);
        self.brush_import_window(ctx);
        self.smart_rename_window(ctx);
        self.goto_window(ctx);
        self.duration_dialog_ui(ctx);
        // Changer la durée des plans choisis (`dureeplans.rs`) ; réunir deux plans qui ont un calque de même nom (`fusionnoms.rs`).
        self.plans_duration_dialog_ui(ctx);
        self.fusion_noms_ui(ctx);
        self.coupe_versions_ui(ctx);
        self.palette_window(ctx);
        // Nouveau projet (`nouveauui.rs`).
        self.new_project_window(ctx);

        if let Some(action) = self.pending.take() {
            let mut choice = None;
            // (Avec plusieurs onglets, la boîte dit de quel projet elle parle.)
            let nom = crate::onglets::nom_de(self.path.as_deref(), self.import_source.as_deref(), false);
            // (Ce qui a changé, pour qu'on voie tout de suite si la question est juste : le dernier geste de l'historique.)
            let dernier = self.history.names().0.last().map(|s| (*s).to_string());
            egui::Window::new("Modifications non enregistrées")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Le projet « {nom} » a été modifié depuis le dernier enregistrement."));
                    if let Some(d) = &dernier {
                        ui.label(RichText::new(format!("Dernier changement : « {d} ».")).small().weak());
                    }
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
                // L'écriture se fait sur un autre fil : l'action suit à sa fin (`enregistrement.rs`).
                Some(0) => self.enregistrer_puis(action),
                Some(1) => self.perform(action),
                Some(_) => {}
                None => self.pending = Some(action),
            }
        }

        // Raccourcis : l'assignation en cours et la liste (raccourcisui.rs).
        self.shortcut_windows(ctx);
    }
}
