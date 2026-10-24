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
//!   range les outils d'annotation, double-clic renomme, clic droit monte, descend, supprime. Ce bloc, lui, écrit dans le projet et s'annule
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

    /// Montre ou cache un champ de texte à côté des cases.
    pub(crate) fn vignettes_basculer_texte(&mut self, champ: Champ) {
        let on = !self.layout.board_textes.montre(champ);
        self.layout.board_textes.regler(champ, on);
        self.settings_a_ecrire = true;
        self.status = format!("Vignettes : {} {}.", champ.nom().to_lowercase(), if on { "montré à côté des cases" } else { "caché" });
    }

    /// Le bloc « Calques » du panneau : les calques d'annotation de la planche (le dernier est au-dessus), puis « Dessin » (le dessin de la
    /// case) tout en bas. Un clic vise un calque : le stylo, la gomme et le lasso d'annotation y agissent ; « Dessin » range ces outils, les
    /// outils de dessin agissent de nouveau sur la case. L'œil montre ou cache le calque ; double-clic : renommer ; clic droit : monter,
    /// descendre, supprimer. Ce bloc est dans le panneau « Planche » plutôt que dans un panneau à lui : il n'a de sens que sur la planche, comme
    /// ses autres réglages, et chaque disposition de fenêtre déjà enregistrée le montre sans avoir à le chercher.
    fn vignettes_calques(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Calques").small().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = ui.add(egui::Button::new(RichText::new(ph::PLUS)).min_size(egui::vec2(22.0, 0.0)));
                let action = "vignettes.annotation_calque_nouveau";
                if self.aide(resp, "Nouveau calque d'annotation", "Un calque de plus pour les annotations de la planche, au-dessus des autres : on y trace, on le montre ou on le cache à part.", action).clicked() {
                    self.run_action(action);
                }
            });
        });
        let calques: Vec<(u64, String, bool, usize)> = self.doc.annotations.calques.iter().rev().map(|c| (c.id, c.nom.clone(), c.visible, self.doc.annotations.traits_du_calque(c.id))).collect();
        let vise = self.vignettes.annot.outil.is_some().then(|| self.annot_calque_actif()).flatten();
        let haut = calques.first().map(|c| c.0);
        let bas = calques.last().map(|c| c.0);
        if calques.is_empty() {
            ui.label(RichText::new("Aucun calque d'annotation : le premier trait en crée un.").small().weak());
        }
        for (id, nom, visible, traits) in calques {
            ui.push_id(("calque_annotation", id), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let eye = if visible { ph::EYE } else { ph::EYE_SLASH };
                    let resp = ui.add(egui::Button::new(RichText::new(eye)).frame(false).min_size(egui::vec2(22.0, 0.0)));
                    if crate::bulles::bulle(resp, if visible { "Cacher le calque" } else { "Montrer le calque" }, "Un affichage : rien n'est effacé.", "").clicked() {
                        self.annot_calque_basculer(id);
                    }
                    if self.vignettes.annot.renomme.as_ref().is_some_and(|r| r.0 == id) {
                        self.vignettes_renommer_calque(ui, id);
                        return;
                    }
                    let on = vise == Some(id);
                    let texte = if visible { RichText::new(&nom) } else { RichText::new(&nom).weak() };
                    let largeur = (ui.available_width() - 26.0).max(40.0);
                    let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(texte, on)).selected(on).min_size(egui::vec2(largeur, 0.0)));
                    let resp = crate::bulles::bulle(resp, &nom, "Un clic vise ce calque : le stylo, la gomme et le lasso d'annotation y agissent. Double-clic : renommer. Clic droit : monter, descendre, supprimer.", "");
                    if resp.clicked() {
                        self.annot_viser_le_calque(id);
                    }
                    if resp.double_clicked() {
                        self.vignettes.annot.renomme = Some((id, nom.clone()));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Renommer").clicked() {
                            ui.close_menu();
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
                    ui.label(RichText::new(traits.to_string()).small().weak());
                });
            });
        }
        // Le dessin de la case : tout en bas, sous les annotations.
        let on = self.vignettes.annot.outil.is_none();
        let courant = self.layer().name.clone();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.add_space(25.0);
            let largeur = (ui.available_width() - 26.0).max(40.0);
            let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new("Dessin"), on)).selected(on).min_size(egui::vec2(largeur, 0.0)));
            let resp = crate::bulles::bulle(resp, "Dessin", &format!("Le dessin de la case : le pinceau, la gomme et le lasso de la barre d'outils y agissent (calque « {courant} »). Un clic range les outils d'annotation."), "");
            if resp.clicked() {
                self.annot_viser_le_dessin();
            }
        });
        ui.add_space(2.0);
        ui.separator();
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
    }
}
