//! Ouverture d'un projet sur un autre fil (règle d'Anthony : rien ne bloque l'interface).
//!
//! Lire un grand projet prend des secondes (un PNG de feuille entière à décoder par dessin, ou un
//! trait vectoriel à rendre) : l'interface restait figée tout ce temps, et au lancement la fenêtre
//! n'apparaissait qu'après. Maintenant `App::open_path` démarre un fil (`project::load_quick_progress`,
//! qui lit les dessins sur plusieurs cœurs) et l'interface montre l'avancement ; `tick_open`, à
//! chaque image d'écran, adopte le projet dès qu'il est lu.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use eframe::egui;

use crate::app::App;
use crate::project::{self, Loaded, Progress};

/// Un projet en cours d'ouverture.
pub struct OpenJob {
    pub path: PathBuf,
    progress: Arc<Progress>,
    handle: JoinHandle<Result<Loaded, String>>,
}

impl OpenJob {
    /// Démarre la lecture de `path` ; `ctx` est réveillé à la fin pour que l'interface l'adopte.
    pub fn start(path: PathBuf, ctx: &egui::Context) -> Self {
        let progress = Arc::new(Progress::default());
        let (p, q, wake) = (path.clone(), progress.clone(), ctx.clone());
        let handle = std::thread::spawn(move || {
            let result = project::load_quick_progress(&p, &q);
            wake.request_repaint();
            result
        });
        OpenJob { path, progress, handle }
    }

    /// Dessins lus et dessins à lire.
    pub fn advance(&self) -> (usize, usize) {
        (self.progress.done.load(Ordering::Relaxed), self.progress.total.load(Ordering::Relaxed))
    }

    pub fn finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Le résultat (à n'appeler qu'une fois `finished`).
    pub fn join(self) -> Result<Loaded, String> {
        self.handle.join().unwrap_or_else(|_| Err("la lecture du projet a planté".into()))
    }
}

impl App {
    /// Lance l'ouverture de `path` sur un autre fil (une seule à la fois).
    pub(crate) fn start_open(&mut self, path: PathBuf) {
        if self.open_job.is_some() {
            return;
        }
        self.status = format!("Ouverture de {}…", path.display());
        self.open_job = Some(OpenJob::start(path, &self.ctx));
    }

    /// À chaque image d'écran : la fenêtre d'avancement, et l'adoption du projet une fois lu.
    pub(crate) fn tick_open(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.open_job else { return };
        if !job.finished() {
            let (done, total) = job.advance();
            let name = job.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            egui::Window::new("Ouverture")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("Ouverture de « {name} »…"));
                    let fraction = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    ui.add(egui::ProgressBar::new(fraction).desired_width(280.0).text(if total == 0 { "lecture du manifeste".to_string() } else { format!("{done} / {total} dessins") }));
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        let job = self.open_job.take().expect("ouverture en cours");
        let path = job.path.clone();
        self.finish_open(path, job.join());
    }

    /// Le dessin `id` vient de s'ouvrir en rendu rapide : son rendu exact se demande selon le
    /// réglage « traits vectoriels texturés » (`diagnostic.rs`).
    pub(crate) fn rejeu_apres_ouverture(&mut self, id: crate::document::CelId) {
        match self.settings.perf.traits_textures {
            // (Derrière tout geste de l'utilisateur : le chargement attend son tour.)
            crate::diagnostic::TraitsTextures::Exact => self.rejeu_with(id, false),
            // Rendu rapide jusqu'au repos, ou pour tout sauf l'image affichée
            // (`App::rejeu_idle_tick`, `warm_current_cel`).
            _ => self.rejeu_idle.push(id),
        }
    }

    /// Le projet est lu (ou ne l'a pas pu être) : il devient le projet courant.
    pub(crate) fn finish_open(&mut self, path: PathBuf, loaded: Result<Loaded, String>) {
        match loaded {
            Ok(loaded) => {
                self.load_doc(loaded.doc);
                for id in loaded.approx {
                    self.rejeu_apres_ouverture(id);
                }
                self.written = loaded.written;
                self.status = if loaded.warnings.is_empty() {
                    format!("Ouvert : {}", path.display())
                } else {
                    format!("Ouvert avec {} problème(s) : {}", loaded.warnings.len(), loaded.warnings.join(" ; "))
                };
                if loaded.converted {
                    // Rien n'est réécrit sur le disque avant « Enregistrer ».
                    self.modified = true;
                    self.status.push_str(
                        " — Ancien format : la feuille de dessin a été agrandie autour du cadre (même cadrage). \
                         « Enregistrer » le passe au nouveau format.",
                    );
                }
                self.remember_project(&path);
                self.path = Some(path);
            }
            Err(e) => self.status = format!("Impossible d'ouvrir le projet : {e}"),
        }
    }
}
