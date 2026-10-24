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

    /// Le contenu du panneau.
    pub(crate) fn vignettes_panneau(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(crate::barres::ECART, crate::barres::ECART);
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
