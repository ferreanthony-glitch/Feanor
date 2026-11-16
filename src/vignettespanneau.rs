//! Le panneau « Vignettes » (Anthony, 2026-10-24) : une petite fenêtre modulable de l'espace Vignettes, à droite de la planche.
//!
//! - **Sens de lecture** : « En lignes » (d'origine : les cases de gauche à droite, puis la ligne suivante dessous, leur texte sous
//!   chaque case) ou « À la japonaise » (de haut en bas, puis la colonne suivante à droite, le texte à droite de chaque case, comme une
//!   feuille d'e-konte). Le réglage « cases par ligne » devient « cases par colonne ».
//! - **Textes à côté des cases** : l'action, le dialogue, les notes du plan (ceux de la fiche), chacun une case à cocher ; d'origine, les
//!   notes seules. Plusieurs champs montrés : chacun a son étiquette colorée.
//!
//! Ce sont des réglages de l'application (la disposition de la fenêtre, comme la taille des cases : `workspace::Layout`), retenus à
//! chaque changement. Rien n'est écrit dans le projet, rien ne s'annule : on ne fait que choisir ce qu'on voit.
//! La mise en page suit tout de suite (`board::Options::sens` et `textes`, `boardjapon.rs`).
//!
//! - **Calques** (Anthony, 2026-10-24) : un bloc tout en haut du panneau, qui liste les calques d'annotation de la planche (le dernier est au-dessus)
//!   puis « Dessin » (le dessin de la case) : œil par calque, un clic vise un calque (le stylo, la gomme et le lasso d'annotation y agissent), « Dessin »
//!   range les outils d'annotation, double-clic renomme, clic droit monte, descend, supprime ; **on tient un calque par son nom et on le dépose
//!   entre deux autres** pour le ranger (une étape d'annulation par dépôt). Ce bloc, lui, écrit dans le projet et s'annule
//!   (`annotationsui.rs`, `annotations.rs`) : ce sont les calques du projet, pas un réglage de la fenêtre.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::boardsens::{Champ, Sens};

impl App {
    /// L'action qui choisit un sens de lecture.
    pub(crate) fn sens_action(sens: Sens) -> &'static str {
        match sens {
            Sens::Lignes => "vignettes.sens_lignes",
            Sens::Japonais => "vignettes.sens_japonais",
        }
    }

    /// L'action qui montre ou cache un champ de texte.
    pub(crate) fn texte_action(champ: Champ) -> &'static str {
        match champ {
            Champ::Action => "vignettes.texte_action",
            Champ::Dialogue => "vignettes.texte_dialogue",
            Champ::Notes => "vignettes.texte_notes",
        }
    }

    /// Choisit le sens de lecture de la planche. La planche est refaite et remise à la taille de la vue (en lignes, à la largeur ; à la
    /// japonaise, à la hauteur : les colonnes s'ajoutent à droite).
    pub(crate) fn vignettes_regler_sens(&mut self, sens: Sens) {
        if self.layout.board_sens == sens {
            return;
        }
        self.layout.board_sens = sens;
        self.settings_a_ecrire = true;
        self.board_fit();
        self.status = format!("Vignettes : lecture {}.", sens.nom().to_lowercase());
    }

    /// Montre ou cache les numéros de plan (la touche `vignettes.numeros`) : sur la planche, et dans son export comme les textes. Un
    /// affichage : rien n'est écrit dans le projet ; retenu dans les réglages de l'application.
    pub(crate) fn vignettes_basculer_numeros(&mut self) {
        let on = !self.settings.vignettes.numeros_visibles;
        self.settings.vignettes.numeros_visibles = on;
        self.settings_a_ecrire = true;
        self.status = if on { "Vignettes : numéros de plan montrés.".into() } else { "Vignettes : numéros de plan cachés.".into() };
    }

    /// Montre ou cache un champ de texte à côté des cases.
    pub(crate) fn vignettes_basculer_texte(&mut self, champ: Champ) {
        let on = !self.layout.board_textes.montre(champ);
        self.layout.board_textes.regler(champ, on);
        self.settings_a_ecrire = true;
        self.status = format!("Vignettes : {} {}.", champ.nom().to_lowercase(), if on { "montré à côté des cases" } else { "caché" });
    }

    /// Un texte vient d'être saisi dans le champ `champ` (fiche du plan, projet qui s'ouvre) : sa ligne est cochée si elle ne l'était pas, pour
    /// que le texte se voie sous sa case (partie 106). Décocher ensuite la cache : rien ne la recoche avant une nouvelle saisie.
    pub(crate) fn vignettes_montrer_champ(&mut self, champ: Champ) {
        if self.layout.board_textes.montre(champ) {
            return;
        }
        self.layout.board_textes.regler(champ, true);
        self.settings_a_ecrire = true;
        self.status = format!("Vignettes : la ligne « {} » est cochée, pour montrer le texte sous les cases (panneau Planche pour la décocher).", champ.nom());
    }

    /// À l'ouverture d'un projet : les lignes de texte qui ont un texte quelque part dans le film (plan ou image) sont cochées si elles ne
    /// l'étaient pas. Aucune n'est décochée, et un projet sans texte ne change rien au choix fait.
    pub(crate) fn vignettes_montrer_les_textes_saisis(&mut self) {
        let remplis = crate::boardsens::champs_remplis(&self.doc);
        for champ in Champ::TOUS {
            if remplis.montre(champ) {
                self.vignettes_montrer_champ(champ);
            }
        }
    }

    /// La ligne « Annotations » du panneau (partie 160, Anthony : « Une seule ligne "Annotations" avec son œil, sans liste. Si tu te sers
    /// de plusieurs couches d'annotation, on les garde au clic droit. ») : l'œil montre ou cache les annotations (un affichage), un clic
    /// sur « Annotations » prend le stylo d'annotation, « Dessin de la case » rend la main au pinceau. Au clic droit de la ligne, les
    /// couches d'annotation d'un projet qui en a plusieurs : viser, montrer ou cacher, renommer, monter, descendre, supprimer, en
    /// ajouter une. Plus le mot « calque » dans les Vignettes.
    fn vignettes_calques(&mut self, ui: &mut egui::Ui) {
        let vise = self.vignettes.annot.outil.is_some();
        let montrees = self.vignettes.annot.visible;
        let traits: usize = self.doc.annotations.calques.iter().map(|c| self.doc.annotations.traits_du_calque(c.id)).sum();
        ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let eye = if montrees { ph::EYE } else { ph::EYE_SLASH };
                let resp = ui.add(egui::Button::new(RichText::new(eye)).frame(false).min_size(egui::vec2(22.0, 0.0)));
                let resp = crate::bulles::bulle(resp, if montrees { "Cacher les annotations" } else { "Montrer les annotations" }, "Un affichage : rien n'est effacé.", "");
                if resp.clicked() {
                    self.annot_basculer_visibles();
                }
                resp.context_menu(|ui| self.vignettes_couches_menu(ui));
                let largeur = ((ui.available_width() - 30.0) * 0.5).max(40.0);
                let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new(format!("{}  Annotations", ph::MARKER_CIRCLE)), vise)).selected(vise).min_size(egui::vec2(largeur, 0.0)));
                let resp = crate::bulles::bulle(resp, "Annotations", "Un clic prend le stylo d'annotation : on note par-dessus la planche, jamais dans les exports du dessin. Clic droit : les couches d'annotation.", "");
                if resp.clicked() {
                    if let Some(id) = self.annot_calque_actif() {
                        self.annot_viser_le_calque(id);
                    } else {
                        self.annot_choisir(crate::annotationsui::Outil::Stylo);
                    }
                }
                resp.context_menu(|ui| self.vignettes_couches_menu(ui));
                let largeur = (ui.available_width() - 26.0).max(40.0);
                let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new("Dessin de la case"), !vise)).selected(!vise).min_size(egui::vec2(largeur, 0.0)));
                let resp = crate::bulles::bulle(resp, "Dessin de la case", "Le dessin de la case : le pinceau, la gomme et le lasso de la barre d'outils y agissent. Un clic range les outils d'annotation.", "");
                if resp.clicked() {
                    self.annot_viser_le_dessin();
                }
                resp.context_menu(|ui| self.vignettes_couches_menu(ui));
                let resp = ui.label(RichText::new(traits.to_string()).small().weak()).on_hover_text("Annotations sur la planche.");
                resp.context_menu(|ui| self.vignettes_couches_menu(ui));
            });
        ui.add_space(2.0);
        ui.separator();
    }

    /// Le clic droit de la ligne « Annotations » : les couches d'annotation (la dernière est au-dessus), chacune avec son sous-menu.
    fn vignettes_couches_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Couches d'annotation").small().weak());
        let couches: Vec<(u64, String, bool, usize)> = self.doc.annotations.calques.iter().rev().map(|c| (c.id, c.nom.clone(), c.visible, self.doc.annotations.traits_du_calque(c.id))).collect();
        let vise = self.vignettes.annot.outil.is_some().then(|| self.annot_calque_actif()).flatten();
        let haut = couches.first().map(|c| c.0);
        let bas = couches.last().map(|c| c.0);
        if couches.is_empty() {
            ui.label(RichText::new("Aucune annotation pour l'instant.").small().weak());
        }
        for (id, nom, visible, traits) in couches {
            if self.vignettes.annot.renomme.as_ref().is_some_and(|r| r.0 == id) {
                ui.horizontal(|ui| self.vignettes_renommer_calque(ui, id));
                continue;
            }
            let titre = format!("{}  {nom} ({traits})", if vise == Some(id) { ph::PENCIL_SIMPLE } else if visible { ph::EYE } else { ph::EYE_SLASH });
            ui.menu_button(titre, |ui| {
                if ui.button("Viser cette couche").on_hover_text("Le stylo, la gomme et le lasso d'annotation y agissent.").clicked() {
                    ui.close_menu();
                    self.annot_viser_le_calque(id);
                }
                if ui.button(if visible { "Cacher" } else { "Montrer" }).on_hover_text("Un affichage : rien n'est effacé.").clicked() {
                    ui.close_menu();
                    self.annot_calque_basculer(id);
                }
                if ui.button("Renommer").clicked() {
                    self.vignettes.annot.renomme = Some((id, nom.clone()));
                }
                if ui.add_enabled(haut != Some(id), egui::Button::new("Monter")).clicked() {
                    ui.close_menu();
                    self.annot_calque_deplacer(id, true);
                }
                if ui.add_enabled(bas != Some(id), egui::Button::new("Descendre")).clicked() {
                    ui.close_menu();
                    self.annot_calque_deplacer(id, false);
                }
                ui.separator();
                if ui.button(format!("Supprimer (et ses {traits} annotation{})", if traits > 1 { "s" } else { "" })).clicked() {
                    ui.close_menu();
                    self.annot_calque_supprimer(id);
                }
            });
        }
        ui.separator();
        let action = "vignettes.annotation_calque_nouveau";
        let key = self.raccourci(action);
        if ui.add(egui::Button::new(format!("{}  Nouvelle couche d'annotation", ph::PLUS)).shortcut_text(key)).on_hover_text("Une couche de plus pour les annotations de la planche, au-dessus des autres : on y trace, on la montre ou on la cache à part.").clicked() {
            ui.close_menu();
            self.run_action(action);
        }
    }

    /// Le champ de renommage d'un calque d'annotation : Entrée (ou un clic ailleurs) valide, Échap abandonne.
    fn vignettes_renommer_calque(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some((_, texte)) = self.vignettes.annot.renomme.as_mut() else { return };
        let edit = egui::TextEdit::singleline(texte).desired_width((ui.available_width() - 4.0).max(40.0)).char_limit(60);
        let resp = ui.add(edit);
        if !resp.has_focus() && !resp.lost_focus() {
            resp.request_focus();
        }
        let (entree, echap) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        if echap {
            self.vignettes.annot.renomme = None;
        } else if entree || resp.lost_focus() {
            if let Some((_, nom)) = self.vignettes.annot.renomme.take() {
                self.annot_calque_renommer(id, &nom);
            }
        }
    }

    /// Le contenu du panneau.
    pub(crate) fn vignettes_panneau(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(crate::barres::ECART, crate::barres::ECART);
        self.vignettes_calques(ui);
        // Les séquences : lesquelles la planche montre, dans quel ordre (`planchesequencesui.rs`).
        self.vignettes_sequences(ui);
        ui.label(RichText::new("Sens de lecture").small().weak());
        ui.horizontal_wrapped(|ui| {
            for sens in Sens::TOUS {
                let on = self.layout.board_sens == sens;
                let icone = if sens.japonais() { ph::COLUMNS } else { ph::ROWS };
                let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new(format!("{icone} {}", sens.nom())), on)).selected(on));
                let action = Self::sens_action(sens);
                if self.aide(resp, sens.nom(), sens.description(), action).clicked() {
                    self.run_action(action);
                }
            }
        });
        let japonais = self.layout.board_sens.japonais();
        ui.horizontal(|ui| {
            let (nom, description) = if japonais {
                ("Cases par colonne", "Combien de cases dans une colonne avant la suivante, à droite (six au plus).")
            } else {
                ("Cases par ligne", "Combien de cases dans une ligne avant la suivante, dessous (six au plus).")
            };
            let resp = ui.add(egui::DragValue::new(&mut self.layout.board_columns).range(1..=crate::board::MAX_COLUMNS).speed(0.05).prefix(format!("{nom} : ")));
            crate::bulles::bulle(resp, nom, description, "");
        });
        ui.add_space(2.0);
        ui.label(RichText::new(if japonais { "Textes à droite des cases" } else { "Textes sous les cases" }).small().weak());
        for champ in Champ::TOUS {
            let mut on = self.layout.board_textes.montre(champ);
            let [r, g, b] = champ.encre();
            let resp = ui.checkbox(&mut on, RichText::new(champ.nom()).color(egui::Color32::from_rgb(r, g, b)));
            let action = Self::texte_action(champ);
            let description = if japonais {
                format!("Montrer le champ « {} » du plan à droite de chaque case (étiqueté quand plusieurs champs sont montrés).", champ.nom())
            } else {
                format!("Montrer le champ « {} » du plan sous chaque case (étiqueté quand plusieurs champs sont montrés).", champ.nom())
            };
            if self.aide(resp, champ.nom(), &description, action).changed() {
                self.vignettes_basculer_texte(champ);
            }
        }
        // Les numéros de plan : montrés d'origine ; la touche N les cache ou les montre (planche et export).
        let mut numeros = self.settings.vignettes.numeros_visibles;
        let resp = ui.checkbox(&mut numeros, "Numéros de plan");
        if self.aide(resp, "Numéros de plan", "Montrer le numéro de chaque plan dans la marge, au-dessus de sa première case (et dans l'export de la planche). Un affichage : rien n'est changé dans le projet.", "vignettes.numeros").changed() {
            self.vignettes_basculer_numeros();
        }
    }
}
