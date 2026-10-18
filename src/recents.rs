//! Projets récents (Anthony, 2026-10-18, axe d'ergonomie 1) : Fichier > Projets récents, et le
//! dernier projet rouvert au lancement (sauf si on en ouvre un autre, ou si une séance interrompue
//! attend d'être récupérée). La liste est gardée dans les réglages (`projets_recents`).

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::app::{App, Pending};

/// Taille de la liste.
pub const MAX: usize = 10;

/// `path` passe en tête de la liste (sans doublon, la casse ignorée comme sous Windows).
pub fn remember(list: &mut Vec<PathBuf>, path: &Path) {
    let key = |p: &Path| p.to_string_lossy().to_lowercase();
    let k = key(path);
    list.retain(|p| key(p) != k);
    list.insert(0, path.to_path_buf());
    list.truncate(MAX);
}

/// Le nom montré : le projet, et le dossier qui contient son dossier.
pub fn label(path: &Path) -> String {
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let place = path.parent().and_then(Path::parent).map(|d| d.display().to_string()).unwrap_or_default();
    if place.is_empty() {
        name
    } else {
        format!("{name}  —  {place}")
    }
}

impl App {
    /// Le projet `path` vient d'être ouvert ou enregistré : en tête des récents.
    pub(crate) fn remember_project(&mut self, path: &Path) {
        remember(&mut self.settings.recent_projects, path);
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
    }

    /// Sous-menu Fichier > Projets récents.
    pub(crate) fn recent_menu(&mut self, ui: &mut egui::Ui) {
        let list = self.settings.recent_projects.clone();
        if list.is_empty() {
            ui.label(egui::RichText::new("Aucun projet récent.").weak());
        }
        for path in list {
            let exists = path.exists();
            let resp = ui.add_enabled(exists, egui::Button::new(label(&path))).on_hover_text(path.display().to_string());
            let resp = resp.on_disabled_hover_text(format!("Introuvable : {}", path.display()));
            if resp.clicked() {
                ui.close_menu();
                self.recent_to_open = Some(path);
                self.request(Pending::OpenRecent);
            }
        }
        ui.separator();
        let mut reopen = self.settings.reopen_last;
        if ui.checkbox(&mut reopen, "Rouvrir le dernier projet au lancement").changed() {
            self.settings.reopen_last = reopen;
            let _ = self.settings.save();
        }
        if ui.button("Effacer la liste").clicked() {
            ui.close_menu();
            self.settings.recent_projects.clear();
            let _ = self.settings.save();
        }
    }

    /// Au lancement sans projet demandé : le dernier, s'il existe encore et qu'aucune séance
    /// interrompue n'attend d'être récupérée.
    pub(crate) fn reopen_last_project(&mut self) {
        if !self.settings.reopen_last || !self.orphans.is_empty() {
            return;
        }
        if let Some(path) = self.settings.recent_projects.first().filter(|p| p.exists()).cloned() {
            self.open_path(path);
        }
    }
}
