//! « Aller à un plan » (Ctrl+F ; Anthony, 2026-10-18 : « fais
//! les 5 rapides ») : une petite fenêtre où l'on tape le nom d'un plan (« 3 », « 12A »), ou
//! « séquence/plan » (« 2/3 ») si plusieurs séquences ont un plan de ce nom, ou à défaut son rang
//! dans tout le storyboard. Entrée : on y va ; Échap : on ferme.

use eframe::egui::{self, Align2};

use crate::app::App;

/// Le plan désigné par `query` parmi `plans` (étiquettes de séquence et de plan, dans l'ordre du
/// storyboard) : d'abord un plan de ce nom (celui de la séquence `prefer` s'il y en a un), puis
/// « séquence/plan », puis le rang (1 : le premier plan).
pub fn find_plan(plans: &[(String, String)], query: &str, prefer: &str) -> Option<usize> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    let named: Vec<usize> = (0..plans.len()).filter(|&i| same(&plans[i].1, q)).collect();
    if let Some(&i) = named.iter().find(|&&i| same(&plans[i].0, prefer)).or(named.first()) {
        return Some(i);
    }
    if let Some((seq, plan)) = q.split_once(['/', ' ']) {
        if let Some(i) = (0..plans.len()).find(|&i| same(&plans[i].0, seq) && same(&plans[i].1, plan)) {
            return Some(i);
        }
    }
    q.parse::<usize>().ok().filter(|&n| n >= 1 && n <= plans.len()).map(|n| n - 1)
}

impl App {
    /// La fenêtre « Aller à un plan », si elle est ouverte.
    pub(crate) fn goto_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.goto_plan.take() else { return };
        let (mut keep, mut go) = (true, false);
        let mut not_found = false;
        egui::Window::new("Aller à un plan")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 80.0])
            .show(ctx, |ui| {
                ui.label("Nom du plan (3, 12A), séquence/plan (2/3), ou son rang dans le storyboard.");
                let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(220.0));
                if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    go = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    keep = false;
                }
                ui.horizontal(|ui| {
                    go |= ui.button("Aller").clicked();
                    if ui.button("Fermer").clicked() {
                        keep = false;
                    }
                });
            });
        if go {
            let plans: Vec<(String, String)> = (0..self.doc.panels.len())
                .map(|i| (self.doc.sequence_label(self.doc.sequence_of(i)), self.doc.scene_label(self.doc.panels[i].scene)))
                .collect();
            let prefer = self.doc.sequence_label(self.doc.sequence_of(self.cur_panel));
            match find_plan(&plans, &text, &prefer) {
                Some(i) => {
                    self.set_panel(i);
                    self.status = format!("{}.", crate::storyboard::panel_long_title(&self.doc, i));
                    keep = false;
                }
                None => not_found = true,
            }
        }
        if not_found {
            self.status = format!("Aucun plan « {} ».", text.trim());
        }
        if keep {
            self.goto_plan = Some(text);
        }
    }
}
