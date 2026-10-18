//! Panneau Historique (Anthony, 2026-10-18, axe d'ergonomie 7) : les étapes d'annulation par leur
//! nom (la commande, l'outil ou le panneau qui les a faites, `History::context`) ; un clic sur une
//! étape ramène le document juste après elle, en annulant ou en rétablissant jusque-là. Les étapes
//! annulées restent en gris sous la dernière, tant qu'une nouvelle modification ne les remplace pas.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;

/// Le nom d'une étape faite par une commande : son nom, sans la précision entre parenthèses.
pub fn step_name(label: &str) -> String {
    label.split(" (").next().unwrap_or(label).trim().to_string()
}

/// Combien d'étapes rétablir (positif) ou annuler (négatif) pour se trouver juste après l'étape
/// `target` (0 : avant la première), quand `done` étapes sont faites et `undone` annulées.
pub fn steps_to(done: usize, undone: usize, target: usize) -> isize {
    target.min(done + undone) as isize - done as isize
}

impl App {
    /// Le panneau Historique.
    pub(crate) fn history_panel(&mut self, ui: &mut egui::Ui) {
        let (done, undone) = self.history.names();
        let done: Vec<String> = done.into_iter().map(str::to_string).collect();
        let undone: Vec<String> = undone.into_iter().map(str::to_string).collect();
        let n = done.len();
        ui.horizontal(|ui| {
            self.command_button(ui, ph::ARROW_COUNTER_CLOCKWISE, "edition.annuler", "Annuler", "Défait la dernière étape.", None);
            self.command_button(ui, ph::ARROW_CLOCKWISE, "edition.retablir", "Rétablir", "Refait la dernière étape annulée.", None);
            let text = match undone.len() {
                0 => format!("{n} étape(s)"),
                u => format!("{n} étape(s), {u} annulée(s)"),
            };
            ui.label(RichText::new(text).small().weak());
        });
        ui.separator();
        let mut target = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            let first = ui.selectable_label(n == 0, RichText::new("Début de l'historique").italics());
            if first.on_hover_text("Annule toutes les étapes gardées (200 au plus).").clicked() {
                target = Some(0);
            }
            for (i, name) in done.iter().enumerate() {
                let resp = ui.selectable_label(i + 1 == n, format!("{}.  {name}", i + 1));
                if resp.on_hover_text("Clic : revenir juste après cette étape (les suivantes sont annulées).").clicked() {
                    target = Some(i + 1);
                }
            }
            for (k, name) in undone.iter().enumerate() {
                let resp = ui.selectable_label(false, RichText::new(format!("{}.  {name}", n + k + 1)).weak().italics());
                if resp.on_hover_text("Annulée. Clic : la rétablir (et celles d'avant).").clicked() {
                    target = Some(n + k + 1);
                }
            }
            if n == 0 && undone.is_empty() {
                ui.label(RichText::new("Rien à annuler pour l'instant : chaque modification s'ajoute ici.").small().weak());
            }
        });
        if let Some(t) = target {
            self.go_to_step(t);
        }
    }

    /// Le document juste après l'étape `target` de l'historique (0 : avant la première).
    pub(crate) fn go_to_step(&mut self, target: usize) {
        let (done, undone) = self.history.names();
        let d = steps_to(done.len(), undone.len(), target);
        if d == 0 {
            return;
        }
        // Comme Annuler et Rétablir : un texte en cours est posé, une déformation abandonnée.
        self.finish_text_edit();
        self.persp = None;
        self.envelope = None;
        for _ in 0..d.unsigned_abs() {
            if d < 0 {
                self.undo();
            } else {
                self.redo();
            }
        }
        self.status = match d {
            d if d < 0 => format!("Historique : {} étape(s) annulée(s).", -d),
            d => format!("Historique : {d} étape(s) rétablie(s)."),
        };
    }
}
