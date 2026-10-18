//! Projets récents (Anthony, 2026-10-18, axe d'ergonomie 1) : Fichier > Projets récents, et le
//! dernier projet rouvert au lancement (sauf si on en ouvre un autre, ou si une séance interrompue
//! attend d'être récupérée). La liste est gardée dans les réglages (`projets_recents`).

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::app::{App, Pending};

/// Taille de la liste.
pub const MAX: usize = 10;

/// Les projets récents existent-ils encore ? Un `exists` par projet à chaque image d'écran (le menu
/// ouvert, l'écran d'accueil) bloque l'interface tant que le disque répond : un projet sur un
/// serveur éteint la figeait des secondes à chaque mouvement de souris. La vérification se fait
/// sur un autre fil, au plus toutes les 3 secondes ; en attendant (ou si elle traîne), le projet
/// passe pour présent.
#[derive(Default)]
pub struct Presence {
    known: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<PathBuf, bool>>>,
    checked: Option<std::time::Instant>,
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Presence {
    /// Délai entre deux vérifications.
    const EVERY: std::time::Duration = std::time::Duration::from_secs(3);

    /// Lance la vérification de `paths` sur un autre fil si la dernière date de plus de 3 s (et
    /// qu'aucune n'est en cours) ; `ctx` est réveillé si une réponse change.
    pub fn refresh(&mut self, ctx: &egui::Context, paths: &[PathBuf]) {
        use std::sync::atomic::Ordering;
        if self.checked.is_some_and(|t| t.elapsed() < Self::EVERY) || self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        self.checked = Some(std::time::Instant::now());
        let (known, busy, ctx, paths) = (self.known.clone(), self.busy.clone(), ctx.clone(), paths.to_vec());
        std::thread::spawn(move || {
            let mut changed = false;
            for p in paths {
                let now = p.exists();
                let before = known.lock().unwrap_or_else(|e| e.into_inner()).insert(p, now);
                changed |= before != Some(now);
            }
            busy.store(false, Ordering::Release);
            if changed {
                ctx.request_repaint();
            }
        });
    }

    /// Le projet est-il là ? (Inconnu : oui, jusqu'à la réponse.)
    pub fn exists(&self, path: &Path) -> bool {
        self.known.lock().unwrap_or_else(|e| e.into_inner()).get(path).copied().unwrap_or(true)
    }
}

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
        self.recent_presence.refresh(ui.ctx(), &list);
        for path in list {
            let exists = self.recent_presence.exists(&path);
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
