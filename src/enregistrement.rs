//! Enregistrer sans figer l'interface (règle d'Anthony : le logiciel répond toujours, instantanément).
//!
//! `Ctrl+S`, « Enregistrer sous » et « Enregistrer » des boîtes « Modifications non enregistrées »
//! ne font plus écrire le fil de l'interface. Il ne fait que le **cliché** de ce qui doit être écrit
//! (`project::prepare_save` : le document sans ses dessins, et la copie des seuls dessins modifiés depuis
//! le dernier enregistrement ; aucun accès au disque), puis un autre fil lit le dossier, met le manifeste
//! et les annexes en JSON, encode les PNG, écrit les fichiers et rend compte ; `tick_save` adopte le
//! résultat à une image d'écran suivante. Avant : 270 ms de gel pour 50 dessins modifiés, 2,8 s pour un
//! premier enregistrement de 500 dessins ; puis 8 à 58 ms pour un film de 100 à 600 plans où rien
//! n'avait changé. Un dessin que le dossier a perdu à côté de SHOT (`SaveReport::manquants`) est réécrit
//! aussitôt, par un second passage, avant la suite.
//!
//! **Le projet n'est jamais marqué enregistré à tort.** Au cliché, `modified` passe à faux ; toute
//! modification qui suit le remet à vrai (c'est le compteur : un « vrai » vu à la fin de l'écriture
//! est une modification postérieure au cliché, que le fichier n'a pas). Si l'écriture échoue,
//! `modified` redevient vrai et une fenêtre dit pourquoi : rien n'est perdu.
//!
//! **Les suites** (Quitter, Fermer, Nouveau, Ouvrir…) attendent la fin de l'écriture : pendant qu'un
//! enregistrement dure, `App::request` retient la demande (`after_save`) et la rejoue à la fin
//! (modifié depuis : la boîte « Modifications non enregistrées » ; sinon, l'action). Au-delà de
//! 200 ms d'attente, une fenêtre montre l'avancement. Une fermeture de fenêtre pendant l'écriture
//! est refusée de la même façon, et `fermeture` attend le fil au cas où. Un échec annule la suite.
//!
//! **L'atomicité** est celle de `project::write_atomic` : chaque fichier est écrit à côté
//! (`<nom>.tmp`) puis prend sa place d'un renommage, le manifeste en dernier ; un plantage ou un
//! disque plein au milieu laisse l'ancien projet lisible. La copie de secours (`recovery.rs`) passe
//! par le même `SaveJob` ; elle ne démarre pas pendant qu'un enregistrement dure (elle recopierait
//! des dessins que le dossier du projet n'a pas encore).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::{App, Pending};
use crate::project::{self, Progress, SaveReport};

/// Attente à partir de laquelle une suite (Quitter…) montre une fenêtre d'avancement.
pub(crate) const DELAI_FENETRE: Duration = Duration::from_millis(200);

/// Un enregistrement qui s'écrit sur un autre fil.
pub(crate) struct Enregistrement {
    manifest: PathBuf,
    handle: JoinHandle<Result<SaveReport, String>>,
    progress: Arc<Progress>,
    started: Instant,
    /// Le projet (`App::doc_epoch`) dont c'est le cliché : un autre projet ouvert entre-temps ne
    /// reçoit ni chemin ni état « enregistré ».
    epoch: u64,
    /// Ce qui suit si tout va bien (la boîte « Modifications non enregistrées »).
    then: Option<Pending>,
}

impl Enregistrement {
    /// Dessins écrits et dessins à écrire.
    pub(crate) fn advance(&self) -> (usize, usize) {
        (self.progress.done.load(Ordering::Relaxed), self.progress.total.load(Ordering::Relaxed))
    }

    /// Le fil d'écriture a fini (son résultat est à adopter).
    pub(crate) fn fini(&self) -> bool {
        self.handle.is_finished()
    }

    /// Une action attend la fin de cette écriture (fermer l'onglet…).
    pub(crate) fn a_une_suite(&self) -> bool {
        self.then.is_some()
    }

    /// Attend la fin de l'écriture et jette son résultat (un onglet qui se ferme, l'application qui quitte :
    /// une écriture en cours n'est jamais coupée).
    pub(crate) fn attendre(self) {
        let _ = self.handle.join();
    }
}

/// Fils de l'écriture : la moitié des cœurs, pour que l'interface garde les siens.
fn threads() -> usize {
    (crate::parallele::coeurs() / 2).clamp(2, 8)
}

impl App {
    /// Un enregistrement s'écrit en ce moment.
    pub(crate) fn save_en_cours(&self) -> bool {
        self.saving.is_some()
    }

    pub(crate) fn save(&mut self) -> bool {
        self.save_then(None)
    }

    /// Enregistre ; `then` est l'action à faire ensuite si tout va bien. Rend vrai si
    /// l'enregistrement est lancé (ou en file derrière celui qui dure).
    pub(crate) fn save_then(&mut self, then: Option<Pending>) -> bool {
        self.end_stroke();
        match self.path.clone() {
            // Un projet à l'ancien nom (`Bus/Bus.pelure`) s'enregistre en `Bus/Bus.shot`, dans le même dossier : seul le
            // manifeste change de nom, l'ancien part une fois le nouveau écrit (`project::retirer_ancien_manifeste`). Un
            // film partagé garde le sien (des équipiers peuvent avoir une version d'avant, `docs/COOPERATION.md`).
            Some(p) => {
                let cible = if self.partage.is_none() { project::manifeste_shot(&p) } else { p };
                self.start_save(cible, then)
            }
            None => self.save_as_then(then),
        }
    }

    /// `manifest` est-il le projet ouvert, sous son nom ou sous son nouveau nom (`Bus.pelure` → `Bus.shot`) ? Ses
    /// dessins sont alors déjà dans le dossier.
    fn meme_projet(&self, manifest: &std::path::Path) -> bool {
        self.path.as_deref().is_some_and(|p| p == manifest || (self.partage.is_none() && project::manifeste_shot(p) == manifest))
    }

    /// « Enregistrer sous » : on choisit un nom, SHOT crée le dossier du projet à ce nom.
    pub(crate) fn save_as(&mut self) -> bool {
        self.save_as_then(None)
    }

    pub(crate) fn save_as_then(&mut self, then: Option<Pending>) -> bool {
        self.end_stroke();
        // Sans nom de fichier : le titre du projet (un projet importé, 14a), sinon « Mon film ».
        let title: String = self.doc.properties.title.trim().chars().map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect();
        let name = format!("{}.{}", self.base_name_or(if title.is_empty() { "Mon film" } else { &title }), project::EXTENSION);
        let mut dialog = rfd::FileDialog::new()
            .set_title("Enregistrer le projet (un dossier à ce nom sera créé)")
            .add_filter("Projet SHOT", &[project::EXTENSION])
            .set_file_name(name);
        if let Some(dir) = self.import_source.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        let Some(chosen) = dialog.save_file() else {
            return false;
        };
        let (_, manifest) = project::target_for(&chosen);
        // (Le projet ouvert sous son ancien nom, `Bus/Bus.pelure`, enregistré sous `Bus/Bus.shot` : c'est lui, pas un autre.)
        let courant = self.path.as_deref().map(|p| if self.partage.is_none() { project::manifeste_shot(p) } else { p.to_path_buf() });
        if let Err(e) = project::check_target(&manifest, courant.as_deref()) {
            self.status = format!("Enregistrement annulé : {e}");
            return false;
        }
        self.start_save(manifest, then)
    }

    /// Enregistre dans `manifest` et attend la fin (création d'un projet neuf, déjà minuscule).
    pub(crate) fn save_to_blocking(&mut self, manifest: PathBuf) -> bool {
        self.start_save(manifest, None) && self.wait_save()
    }

    /// Le cliché, sur le fil de l'interface, puis l'écriture sur un autre fil. Si un enregistrement
    /// dure encore, celui-ci le suit (un seul à la fois).
    pub(crate) fn start_save(&mut self, manifest: PathBuf, then: Option<Pending>) -> bool {
        if self.saving.is_some() {
            self.save_queue = Some((manifest, then));
            self.status = "Enregistrement en cours : le suivant partira à sa fin.".into();
            return true;
        }
        // (Une zone bitmap qu'on vient de transformer : le dessin reçoit d'abord ses pixels à leur place, `pixtools.rs`.)
        self.flush_bitmap_float();
        // (De même, l'aplat posé sur les autres clés en arrière-plan, `aplatmulti.rs` : quelques dixièmes de seconde au plus.)
        self.attendre_aplat_multi();
        // Film partagé enregistré à sa place : seulement ce qu'on tient (`projet_partage.rs`). Ailleurs (« Enregistrer sous »), c'est
        // une copie à plat, comme un projet ordinaire.
        let partage_ici = self.partage.is_some() && self.path.as_ref() == Some(&manifest);
        // Dernière modification (13e, propriétés du projet) : dans un film partagé, c'est l'affaire du montage.
        if !partage_ici || self.partage.as_ref().is_some_and(|s| s.tiens(crate::verrous::Cible::Montage)) {
            self.doc.properties.modified = crate::recovery::now_secs();
        }
        if !self.meme_projet(&manifest) {
            // Nouveau dossier : tous les dessins y seront écrits.
            self.written.clear();
        }
        if partage_ici {
            self.partage_avant_enregistrement();
        }
        let prepare = match self.partage.as_mut().filter(|_| partage_ici) {
            Some(s) => {
                // (Une suite attend — quitter, fermer : ce qui n'est pas écrit serait perdu, on écrit même une prise en confirmation.)
                let ctx = s.contexte_ecriture(&self.doc, then.is_some() || self.after_save.is_some());
                project::partage_io::preparer(&self.doc, &manifest, &mut self.written, ctx)
            }
            // (Un `.shot` : l'ancien `.pelure` du même projet part après lui, s'il est là — conversion, ou reste d'un
            // plantage au milieu d'une conversion.)
            None => project::prepare_save(&self.doc, &manifest, &mut self.written, None).map(|job| match project::manifeste_jumeau(&manifest).filter(|j| project::est_ancien_manifeste(j)) {
                Some(ancien) => job.retirer_apres(ancien),
                None => job,
            }),
        };
        let job = match prepare {
            Ok(job) => job,
            Err(e) => {
                self.written.clear();
                // (Une seconde passe ou une file qui échoue : le dossier n'a pas tout, le projet reste à enregistrer.)
                self.modified = true;
                self.status = format!("Échec de l'enregistrement : {e}");
                self.save_error = Some(e);
                return false;
            }
        };
        // Le cliché est pris : ce qui change à partir d'ici remet `modified` à vrai.
        self.modified = false;
        let progress = Arc::new(Progress::default());
        let (q, wake) = (progress.clone(), self.ctx.clone());
        let handle = std::thread::spawn(move || {
            let result = job.run_with(Some(&q), threads());
            wake.request_repaint();
            result
        });
        self.status = "Enregistrement…".into();
        self.saving = Some(Enregistrement { manifest, handle, progress, started: Instant::now(), epoch: self.doc_epoch, then });
        true
    }

    /// À chaque image d'écran : la fenêtre d'erreur, la fenêtre d'avancement d'une suite qui attend,
    /// et l'adoption du résultat dès que l'écriture est finie.
    pub(crate) fn tick_save(&mut self, ctx: &egui::Context) {
        self.save_error_window(ctx);
        let Some(job) = &self.saving else { return };
        if !job.handle.is_finished() {
            if self.after_save.is_some() && job.started.elapsed() >= DELAI_FENETRE {
                let (done, total) = job.advance();
                let name = job.manifest.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                egui::Window::new("Enregistrement")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label(format!("Enregistrement de « {name} »…"));
                        let fraction = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                        ui.add(egui::ProgressBar::new(fraction).desired_width(280.0).text(if total == 0 { "préparation".to_string() } else { format!("{done} / {total} dessins") }));
                    });
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        self.finish_job();
        ctx.request_repaint();
    }

    /// Attend la fin de l'écriture (et des suivantes en file) ; rend faux si l'une a échoué.
    pub(crate) fn wait_save(&mut self) -> bool {
        let mut ok = true;
        while self.saving.is_some() {
            ok = self.finish_job();
        }
        ok
    }

    /// Joint le fil d'écriture et adopte son résultat.
    pub(crate) fn finish_job(&mut self) -> bool {
        let Some(Enregistrement { manifest, handle, epoch, then, .. }) = self.saving.take() else { return true };
        let result = handle.join().unwrap_or_else(|_| Err("l'écriture a planté".into()));
        self.finish_save(manifest, epoch, then, result)
    }

    /// Le résultat de l'écriture : l'état du projet, puis la suite.
    fn finish_save(&mut self, manifest: PathBuf, epoch: u64, then: Option<Pending>, result: Result<SaveReport, String>) -> bool {
        // (Un autre projet a pris la place pendant l'écriture : ce qu'on a écrit est bien écrit, mais
        // le projet d'à présent n'est ni à ce chemin, ni enregistré.)
        let same = epoch == self.doc_epoch;
        let folder = manifest.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match result {
            Ok(mut report) => {
                let manquants = std::mem::take(&mut report.manquants);
                // Le projet est un dossier à son nom (comme un projet Storyboard Pro) : on le dit, le
                // fichier n'est pas là où l'on pensait l'avoir mis (Anthony, 2026-10-17 : « quand
                // j'enregistre un fichier Pelure, il n'apparaît pas dans l'explorateur »).
                self.status = format!(
                    "Enregistré dans le dossier « {folder} » : {} ({} dessin(s) écrit(s){}). Fichier > Afficher le projet dans l'Explorateur.",
                    manifest.display(),
                    report.written,
                    if report.removed > 0 { format!(", {} inutilisé(s) effacé(s)", report.removed) } else { String::new() }
                );
                if same {
                    // Film partagé : les bases de comparaison avancent, les prises annoncent ce qui est écrit ; une copie
                    // à plat ailleurs détache le projet du partage.
                    match (self.partage.as_mut(), report.partage.as_ref()) {
                        (Some(s), Some(r)) => s.enregistrement_fini(r, &self.doc),
                        (Some(_), None) => self.lacher_la_session(),
                        _ => {}
                    }
                    // Les instantanés d'envois sont dans le dossier : la mémoire qu'ils tenaient est rendue, et
                    // « Enregistrer sous » reprendra `versions/` de ce dossier-ci (`envois.rs`).
                    self.doc.envois.liberer(&report.snapshots);
                    self.doc.envois.dossier = manifest.parent().map(std::path::Path::to_path_buf);
                    // Un projet passé de `.pelure` à `.shot` : on le dit ; les récents (et le « dernier projet ») ne
                    // gardent que le nouveau nom (`recents::remember`), la séance et le secours suivent `path`.
                    if self.path.as_deref().is_some_and(|p| p != manifest && project::est_ancien_manifeste(p) && project::manifeste_shot(p) == manifest) {
                        let nom = manifest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        self.status.push_str(&format!(" Le fichier du projet s'appelle maintenant « {nom} » (l'ancien « .pelure » est remplacé ; dessins et dossier inchangés)."));
                    }
                    self.remember_project(&manifest);
                    self.path = Some(manifest.clone());
                    self.import_source = None;
                    // (Le projet a un nom et un chemin : la session le retient, `onglets.rs`.)
                    self.session_a_ecrire = true;
                    // Des dessins déjà écrits manquaient dans le dossier (effacés à côté de SHOT) : le cliché n'avait pas
                    // leurs pixels. Ils repartent tout de suite, avec la suite qui attendait (`SaveReport::manquants`).
                    if !manquants.is_empty() {
                        for id in &manquants {
                            self.written.remove(id);
                        }
                        return self.start_save(manifest, then);
                    }
                }
            }
            Err(e) => {
                if same {
                    // Rien n'est sûr de ce que le dossier contient : tout sera réécrit la prochaine fois.
                    self.written.clear();
                    self.modified = true;
                }
                self.status = format!("Échec de l'enregistrement : {e}");
                self.save_error = Some(e);
                self.save_queue = None;
                self.after_save = None;
                return false;
            }
        }
        if !same {
            self.save_queue = None;
            self.after_save = None;
            return true;
        }
        // Un enregistrement demandé pendant celui-ci : seulement s'il y a quelque chose à écrire.
        if let Some((next, next_then)) = self.save_queue.take() {
            if self.modified || self.path.as_ref() != Some(&next) {
                return self.start_save(next, next_then.or(then));
            }
            self.status = "Enregistré (rien de plus à écrire).".into();
            return self.after_saved(next_then.or(then));
        }
        self.after_saved(then)
    }

    /// La suite d'un enregistrement réussi : l'action de la boîte « Modifications non enregistrées »
    /// (redemandée si le projet a encore changé), ou celle d'une demande qui attendait.
    fn after_saved(&mut self, then: Option<Pending>) -> bool {
        if let Some(action) = then {
            // (Modifié pendant l'écriture : la boîte demande à nouveau.)
            self.request(action);
        }
        if let Some(action) = self.after_save.take() {
            self.request(action);
        }
        true
    }

    /// L'écriture a échoué : pourquoi, et que rien n'est perdu.
    fn save_error_window(&mut self, ctx: &egui::Context) {
        let Some(error) = self.save_error.clone() else { return };
        let mut close = false;
        let mut retry = false;
        egui::Window::new("Enregistrement impossible")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label("Le projet n'a pas pu être enregistré :");
                ui.label(egui::RichText::new(&error).strong());
                ui.add_space(4.0);
                ui.label("Il reste marqué comme modifié et rien n'est perdu dans SHOT. Libère de la place, vérifie les droits d'écriture du dossier, puis enregistre à nouveau.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    retry = ui.button("Réessayer").clicked();
                    close = ui.button("Fermer").clicked();
                });
            });
        if retry || close {
            self.save_error = None;
        }
        if retry {
            self.save();
        }
    }

    /// Dans la barre d'état : « Enregistrement… 12 / 50 » tant que l'écriture dure.
    pub(crate) fn save_status_ui(&self, ui: &mut egui::Ui) {
        let Some(job) = &self.saving else { return };
        let (done, total) = job.advance();
        ui.separator();
        // (Pas de roue qui tourne : elle repeindrait l'écran à chaque image ; le compte se rafraîchit
        // dix fois par seconde, `tick_save`.)
        let text = if total == 0 { "Enregistrement…".to_string() } else { format!("Enregistrement… {done} / {total}") };
        ui.label(egui::RichText::new(text).small()).on_hover_text("SHOT écrit le projet pendant que tu continues : rien ne se fige. Fermer attend la fin de l'écriture.");
    }

    /// `fermeture` : au cas où une écriture dure encore, on l'attend (on ne perd jamais un projet).
    pub(crate) fn finish_save_on_exit(&mut self) {
        let _ = self.wait_save();
    }
}
