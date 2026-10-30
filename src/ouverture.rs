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
use crate::rejeu::Demande;

/// Un projet en cours d'ouverture.
pub struct OpenJob {
    pub path: PathBuf,
    progress: Arc<Progress>,
    handle: JoinHandle<(Result<Loaded, String>, Vec<Demande>)>,
}

impl OpenJob {
    /// Démarre la lecture de `path` ; `ctx` est réveillé à la fin pour que l'interface l'adopte. `exacts` : le rendu
    /// exact des dessins ouverts en rendu rapide est demandé dès l'adoption (réglage « traits vectoriels texturés ») ; leurs
    /// copies pour le rejeu se font alors ici, sur ce fil, pas sur celui de l'interface.
    pub fn start(path: PathBuf, ctx: &egui::Context, equipe: Option<crate::cooperationui::ParamsEquipe>, exacts: bool) -> Self {
        let progress = Arc::new(Progress::default());
        let (p, q, wake) = (path.clone(), progress.clone(), ctx.clone());
        let handle = std::thread::spawn(move || {
            let mut result = project::load_quick_progress(&p, &q);
            // Un film partagé : l'équipe se rejoint ici aussi, sur ce fil (ma fiche est écrite, celles des autres lues).
            if let (Ok(loaded), Some(equipe)) = (result.as_mut(), equipe) {
                crate::cooperationui::rejoindre_dans_le_fil(loaded, equipe, &p);
            }
            let demandes = match &result {
                Ok(loaded) if exacts => crate::rejeu::demandes(&loaded.doc, &loaded.approx),
                _ => Vec::new(),
            };
            wake.request_repaint();
            (result, demandes)
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

    /// Le résultat (à n'appeler qu'une fois `finished`), et les copies des dessins à rejouer.
    pub fn join(self) -> (Result<Loaded, String>, Vec<Demande>) {
        self.handle.join().unwrap_or_else(|_| (Err("la lecture du projet a planté".into()), Vec::new()))
    }
}

impl App {
    /// Lance l'ouverture de `path` sur un autre fil (une seule à la fois).
    pub(crate) fn start_open(&mut self, path: PathBuf) {
        if self.open_job.is_some() {
            return;
        }
        self.status = format!("Ouverture de {}…", path.display());
        // (Le film partagé a besoin de qui je suis : lu ici, sur le fil de l'interface, rien qu'en mémoire.)
        let equipe = Some(crate::cooperationui::ParamsEquipe { moi: self.identite_equipe(), delais: crate::cooperationui::delais_de(&self.settings.equipe), ctx: self.ctx.clone() });
        let exacts = self.settings.perf.traits_textures == crate::diagnostic::TraitsTextures::Exact;
        self.open_job = Some(OpenJob::start(path, &self.ctx, equipe, exacts));
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
        let _t = crate::perf::section("ouverture : adoption du projet");
        let (loaded, demandes) = job.join();
        self.finish_open_avec(path, loaded, demandes);
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

    /// Les dessins `ids` viennent de s'ouvrir en rendu rapide ; `demandes` : leurs copies pour le rejeu, faites sur le fil
    /// de la lecture (vide si le rendu exact n'était pas demandé à l'ouverture). Un film de 300 plans en a des milliers :
    /// elles entrent dans la file d'un coup, sans copie sur le fil de l'interface.
    pub(crate) fn rejeux_apres_ouverture(&mut self, ids: Vec<crate::document::CelId>, demandes: Vec<Demande>) {
        if demandes.is_empty() || self.settings.perf.traits_textures != crate::diagnostic::TraitsTextures::Exact {
            // (Le réglage a changé pendant la lecture : les copies ne servent pas.)
            crate::lacher::en_fond(demandes);
            for id in ids {
                self.rejeu_apres_ouverture(id);
            }
            return;
        }
        let copies: std::collections::HashSet<crate::document::CelId> = demandes.iter().map(|d| d.cel).collect();
        let (w, h) = (self.doc.width, self.doc.height);
        self.rejeu.submit_batch(demandes, w, h, false);
        for id in ids.into_iter().filter(|id| !copies.contains(id)) {
            self.rejeu_apres_ouverture(id);
        }
    }

    /// Le projet est lu (ou ne l'a pas pu être) : il devient le projet courant.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn finish_open(&mut self, path: PathBuf, loaded: Result<Loaded, String>) {
        self.finish_open_avec(path, loaded, Vec::new());
    }

    /// `finish_open`, avec les copies des dessins à rejouer faites sur le fil de la lecture (`OpenJob::start`).
    pub(crate) fn finish_open_avec(&mut self, path: PathBuf, loaded: Result<Loaded, String>, demandes: Vec<Demande>) {
        match loaded {
            Ok(loaded) => {
                if !self.charger_projet(loaded.doc) {
                    self.status = format!("« {} » n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.", path.display());
                    crate::lacher::en_fond(demandes);
                    return;
                }
                self.rejeux_apres_ouverture(loaded.approx, demandes);
                self.written = loaded.written;
                // Un film partagé : la session démarre (`cooperationui.rs`). Sans elle, le projet reste en lecture seule : son
                // chemin n'est pas retenu (un enregistrement ne doit jamais écrire un manifeste classique par-dessus la structure).
                let partage_ok = match (loaded.session, loaded.erreur_equipe, loaded.partage.is_some()) {
                    (Some(s), _, _) => {
                        self.partage = Some(s);
                        self.equipe_ui = Default::default();
                        true
                    }
                    (None, Some(e), _) => {
                        self.status = format!("Film partagé ouvert sans l'équipe : {e}. Il reste en lecture seule (Enregistrer sous en fait une copie à toi).");
                        false
                    }
                    (None, None, partage) => {
                        if partage {
                            self.status = "Film partagé ouvert sans l'équipe : il reste en lecture seule (Enregistrer sous en fait une copie à toi).".into();
                        }
                        !partage
                    }
                };
                let status_partage = if partage_ok { None } else { Some(self.status.clone()) };
                self.status = if loaded.warnings.is_empty() {
                    format!("Ouvert : {}", path.display())
                } else {
                    format!("Ouvert avec {} remarque{} : {}", loaded.warnings.len(), if loaded.warnings.len() > 1 { "s" } else { "" }, loaded.warnings.join(" ; "))
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
                if let Some(m) = status_partage {
                    self.status = m;
                    self.modified = false;
                } else {
                    // L'ancien `crayonnes.json` signalé : un bouton le met de côté (`crayonnesheritage.rs`).
                    if loaded.warnings.iter().any(|w| w.starts_with(crate::crayonnesheritage::FILE)) {
                        if let Some(dir) = path.parent() {
                            let msg = self.status.clone();
                            self.status_avec_action(msg, crate::messages::ActionMessage::CrayonnesDeCote(dir.to_path_buf()));
                        }
                    }
                    self.path = Some(path);
                }
            }
            Err(e) => self.status = format!("Impossible d'ouvrir le projet : {e}"),
        }
    }
}
