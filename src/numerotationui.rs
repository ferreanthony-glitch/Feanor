//! Numérotation des plans : l'interface (`numerotation.rs` pour les règles).
//!
//! - Réglages du projet > Réglages > « Numérotation des plans » : Suivie / Verrouillée, et le bouton
//!   « Renuméroter… » (actif en numérotation verrouillée).
//! - Commandes `numerotation.verrouiller` (bascule) et `numerotation.renumeroter` (ouvre la
//!   confirmation) : menu Storyboard, « Plus… » de la barre du montage, palette Ctrl+K.
//! - Chaque changement est une étape d'annulation nommée.

use eframe::egui::{self, RichText};

use crate::app::App;

impl App {
    /// Un projet qui naît (Nouveau projet, accueil, scénario, plans choisis) : numérotation verrouillée si le réglage des
    /// Préférences le veut (coché d'origine), suivie sinon. Ne sert jamais à un projet qui existe déjà.
    pub(crate) fn numerotation_du_projet_neuf(&self, doc: &mut crate::document::Document) {
        if self.settings.numbering_locked_new {
            doc.lock_numbering();
        } else {
            doc.unlock_numbering();
        }
    }

    /// Préférences > Montage > Plans neufs : « Verrouiller la numérotation des projets neufs ».
    pub(crate) fn numerotation_neufs_pref_ui(&mut self, ui: &mut egui::Ui) {
        let avant = self.settings.numbering_locked_new;
        let resp = ui.checkbox(&mut self.settings.numbering_locked_new, "Verrouiller la numérotation des projets neufs");
        crate::bulles::bulle(
            resp,
            "Numérotation des projets neufs",
            "Coché : un plan inséré entre le 2 et le 3 devient le 2A, comme en studio (les autres gardent leur numéro). Décoché : les plans se renumérotent dans l'ordre du film. Ne change pas les projets qui existent ; « Renuméroter les plans… » (menu Storyboard) repart de 1 quand tu veux.",
            "",
        );
        if avant != self.settings.numbering_locked_new {
            let _ = self.settings.save();
        }
    }

    /// Verrouille la numérotation (les numéros d'aujourd'hui deviennent ceux des plans) ou la rend
    /// suivie. Une étape d'annulation.
    pub(crate) fn toggle_numbering_lock(&mut self) {
        self.end_stroke();
        self.montage_drag = None;
        self.record_step();
        if self.doc.numbering_locked {
            self.doc.unlock_numbering();
            self.status = "Numérotation suivie : les plans se numérotent dans l'ordre du film.".into();
        } else {
            self.doc.lock_numbering();
            self.status = "Numérotation verrouillée : un plan inséré entre le 12 et le 13 sera le 12A, un plan supprimé laisse un trou.".into();
        }
    }

    /// Renumérote 1, 2, 3… dans l'ordre du film (après confirmation). Une étape d'annulation.
    pub(crate) fn renumber_plans(&mut self) {
        if !self.doc.numbering_locked {
            self.status = "La numérotation est suivie : les plans sont déjà numérotés dans l'ordre du film.".into();
            return;
        }
        self.end_stroke();
        self.montage_drag = None;
        let before = self.history.context.replace("Renuméroter les plans".to_string());
        self.record_step();
        self.history.context = before;
        let n = self.doc.renumber();
        self.status = match n {
            0 => "Les plans étaient déjà numérotés dans l'ordre.".into(),
            1 => "Renuméroté : 1 plan a changé de numéro (Ctrl+Z pour revenir).".into(),
            n => format!("Renuméroté : {n} plans ont changé de numéro (Ctrl+Z pour revenir)."),
        };
    }

    /// La confirmation de « Renuméroter ».
    pub(crate) fn renumber_window(&mut self, ctx: &egui::Context) {
        if !self.renumber_confirm {
            return;
        }
        // Combien de numéros changeraient : on compare le numéro actuel à celui du rang.
        let changes = self.doc.scene_order_planche().iter().enumerate().filter(|(k, id)| self.doc.plan_code(**id) != (*k + 1).to_string()).count();
        let mut yes = false;
        let mut no = false;
        let modal = egui::Modal::new(egui::Id::new("renumeroter_plans")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Renuméroter les plans ?");
            ui.add_space(6.0);
            if self.doc.numbering_locked {
                ui.label(format!(
                    "Les plans et les séquences prennent les numéros 1, 2, 3… dans l'ordre du film ({changes} plan{} changent de numéro).",
                    if changes > 1 { "s" } else { "" }
                ));
                ui.label(RichText::new("Les exports déjà faits pour le montage portent les anciens numéros : à refaire.").weak());
                ui.label(RichText::new("Ctrl+Z revient en arrière.").weak());
            } else {
                ui.label("La numérotation est suivie : les plans sont déjà numérotés dans l'ordre du film.");
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                yes = ui.add_enabled(self.doc.numbering_locked, egui::Button::new("Renuméroter")).clicked();
                no = ui.button("Annuler").clicked();
            });
        });
        if yes {
            self.renumber_confirm = false;
            self.renumber_plans();
        } else if no || modal.should_close() {
            self.renumber_confirm = false;
        }
    }

    /// La section « Numérotation des plans » de Réglages du projet > Réglages.
    pub(crate) fn numbering_section(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Numérotation des plans").strong());
        let locked = self.doc.numbering_locked;
        ui.horizontal(|ui| {
            if ui
                .selectable_label(!locked, "Suivie")
                .on_hover_text("Les plans se numérotent 1, 2, 3… dans l'ordre du film. Insérer un plan fait glisser les suivants.")
                .clicked()
                && locked
            {
                self.run_action("numerotation.verrouiller");
            }
            if ui
                .selectable_label(locked, "Verrouillée")
                .on_hover_text("Chaque plan garde son numéro. Inséré entre le 12 et le 13, un plan devient le 12A ; supprimé, il laisse un trou ; déplacé, il garde son numéro.")
                .clicked()
                && !locked
            {
                self.run_action("numerotation.verrouiller");
            }
            let resp = ui.add_enabled(locked, egui::Button::new("Renuméroter…"));
            let resp = crate::bulles::bulle(resp, "Renuméroter les plans", "Remet des numéros propres, 1, 2, 3… dans l'ordre du film (après confirmation, une étape d'annulation).", "");
            if resp.clicked() {
                self.run_action("numerotation.renumeroter");
            }
        });
        let text = if locked {
            "Verrouillée : les numéros ne bougent plus (12, 12A, 13…). Un plan supprimé laisse un trou ; son numéro n'est pas redonné."
        } else {
            "Suivie : le numéro d'un plan est son rang dans le film."
        };
        ui.label(RichText::new(text).small().weak());
    }
}
