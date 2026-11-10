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
                // (Un projet converti `.pelure` → `.shot` est encore là sous son nouveau nom : `open_path` le trouve.)
                let now = crate::project::manifeste_present(&p);
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

/// `path` passe en tête de la liste (sans doublon, la casse ignorée comme sous Windows). Un projet n'y est qu'une
/// fois sous ses deux noms : `Bus.shot` remplace `Bus.pelure` (projet converti, `project::cle_de_projet`).
pub fn remember(list: &mut Vec<PathBuf>, path: &Path) {
    let key = |p: &Path| crate::project::cle_de_projet(p);
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
        // Déjà en tête : rien ne change, les réglages ne sont pas réécrits (l'écriture continue passe ici après chaque trait, `continu.rs`).
        if self.settings.recent_projects.first().is_some_and(|p| p == path) {
            return;
        }
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
    ///
    /// Le projet est-il encore là ? Un `exists` sur le fil de l'interface, avant la première image,
    /// gelait SHOT le temps que répond un disque réseau éteint ou un disque externe qui se réveille
    /// (fenêtre noire, puis rien). La question est posée sur un autre fil ; `tick_reopen` ouvre le
    /// projet à une image d'écran suivante, si rien d'autre n'a été ouvert entre-temps.
    ///
    /// Les onglets (`onglets.rs`) : ce sont ceux de la séance d'avant qui reviennent (tous les projets qui
    /// étaient ouverts, l'onglet affiché à la fin) ; une séance sans onglets retenus rouvre le dernier projet.
    pub(crate) fn reopen_last_project(&mut self) {
        // (La séance d'avant s'est finie sans projet ouvert : on démarre sur l'accueil, `sansprojet.rs`.)
        if !self.settings.reopen_last || !self.orphans.is_empty() || self.settings.session_vide {
            return;
        }
        let mut candidats = self.settings.session_tabs.clone();
        if candidats.is_empty() {
            candidats.extend(self.settings.recent_projects.first().cloned());
        }
        if candidats.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, a_sonder) = (self.ctx.clone(), candidats.clone());
        std::thread::spawn(move || {
            // (Un projet converti depuis, ou un plantage au milieu d'une conversion : le manifeste à ouvrir, sous son nom
            // d'aujourd'hui, `project::manifeste_a_ouvrir`.)
            let presents: Vec<PathBuf> = a_sonder.into_iter().map(|p| crate::project::manifeste_a_ouvrir(&p)).filter(|p| p.exists()).collect();
            let _ = tx.send(presents);
            ctx.request_repaint();
        });
        self.reopen_probe = Some((candidats, rx));
    }

    /// À chaque image d'écran : la réponse de `reopen_last_project`, s'il en attend une.
    pub(crate) fn tick_reopen(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let Some((candidats, rx)) = &self.reopen_probe else { return };
        match rx.try_recv() {
            Err(TryRecvError::Empty) => {}
            Ok(presents) => {
                // Celui qui était affiché à la fin de la séance d'avant, s'il est encore là.
                let cle = |p: &Path| crate::project::cle_de_projet(p);
                let affiche = self
                    .settings
                    .session_tabs
                    .get(self.settings.session_active)
                    .and_then(|a| presents.iter().find(|p| cle(p) == cle(a)))
                    .cloned()
                    .or_else(|| presents.first().cloned());
                let _ = candidats;
                self.reopen_probe = None;
                // (Anthony a déjà ouvert, créé ou dessiné quelque chose : on ne lui prend pas la main.)
                if !presents.is_empty() && self.path.is_none() && !self.modified && self.open_job.is_none() && self.saving.is_none() && self.new_dialog.is_none() && self.sans_projet() {
                    self.rouvrir_ces_projets(presents, affiche);
                }
            }
            Err(TryRecvError::Disconnected) => self.reopen_probe = None,
        }
    }
}
