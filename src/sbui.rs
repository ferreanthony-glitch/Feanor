//! Ouvrir un projet Storyboard Pro (14a). Fichier > Ouvrir (ou « Ouvrir un projet Storyboard
//! Pro… », ou glisser le `.sboard` sur la fenêtre) lit le projet (jamais modifié), puis :
//! - si Storyboard Pro est installé, l'export des dessins se fait tout seul (14a-2, `sbbatch` :
//!   copie du projet, Storyboard Pro en mode batch avec le script de Pelure), et l'import suit ;
//! - sinon, l'import se fait sans les dessins (les calques arrivent vides).
//!
//! L'import tourne sur un autre fil et remplace le projet ouvert (après avoir proposé de
//! l'enregistrer). On peut encore exporter les dessins à la main (« Créer le script… »). Voir
//! `sboard`, `sbscript`, `sbimport`, `sbvector`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::{App, Pending};
use crate::document::Document;
use crate::sbbatch::Batch;
use crate::sboard::{self, SbProject};
use crate::sbscript::{self, Drawings};

/// Résultat d'un import fini : le projet et ce qui n'a pas pu être repris.
type Built = (Document, Vec<String>);

/// L'état de la fenêtre.
#[derive(Default)]
pub(crate) struct SbWindow {
    sboard: Option<PathBuf>,
    project: Option<Result<SbProject, String>>,
    drawings_dir: Option<PathBuf>,
    drawings: Option<Result<Drawings, String>>,
    /// Script écrit (et copié) : on montre comment le lancer.
    script_written: bool,
    /// Export automatique des dessins en cours (14a-2).
    export: Option<Batch>,
    /// Pourquoi les dessins n'ont pas été repris (ajouté au compte rendu de l'import).
    drawings_note: Option<String>,
    /// Dossier de travail de l'export, effacé une fois l'import fait.
    work: Option<PathBuf>,
    /// Compte rendu du dernier import.
    report: Option<Vec<String>>,
    job: Option<SbJob>,
}

/// Un import en cours, sur un autre fil.
pub(crate) struct SbJob {
    progress: Arc<AtomicU32>,
    result: Arc<Mutex<Option<Built>>>,
    name: String,
}

impl SbWindow {
    fn busy(&self) -> bool {
        self.job.is_some() || self.export.is_some()
    }
}

impl App {
    /// Fichier > Ouvrir un projet Storyboard Pro.
    pub(crate) fn open_sb_window(&mut self) {
        let busy = self.sb_window.as_ref().is_some_and(SbWindow::busy);
        if busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new().set_title("Projet Storyboard Pro à ouvrir").add_filter("Projet Storyboard Pro", &["sboard"]).pick_file() {
            self.open_sboard(&path);
        }
    }

    /// Un fichier lâché sur la fenêtre (ou ouvert) : un `.sboard` s'ouvre ; la liste écrite par
    /// le script (`pelure.txt`) choisit le dossier des dessins de la fenêtre. Faux si ce n'est
    /// ni l'un ni l'autre.
    pub(crate) fn import_dropped_sboard(&mut self, path: &Path) -> bool {
        let project = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("sboard"));
        let manifest = path.file_name().is_some_and(|n| n.eq_ignore_ascii_case(sbscript::MANIFEST));
        if !project && !manifest {
            return false;
        }
        if project {
            self.open_sboard(path);
            return true;
        }
        let w = self.sb_window.get_or_insert_with(SbWindow::default);
        if !w.busy() {
            w.drawings_dir = path.parent().map(Path::to_path_buf);
            read_drawings(w);
            w.report = None;
        }
        true
    }

    /// Ouvre le projet `path` : lecture, export des dessins par Storyboard Pro s'il est là,
    /// puis import.
    fn open_sboard(&mut self, path: &Path) {
        let w = self.sb_window.get_or_insert_with(SbWindow::default);
        if w.busy() {
            return;
        }
        w.project = Some(sboard::read(path));
        w.sboard = Some(path.to_path_buf());
        w.drawings = None;
        w.drawings_dir = None;
        w.drawings_note = None;
        w.report = None;
        let Some(Ok(sb)) = &w.project else { return };
        let total: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
        match crate::sbbatch::find_storyboard_pro() {
            Some(exe) => {
                let batch = Batch::start(exe, path.to_path_buf(), total);
                w.work = Some(batch.work.clone());
                w.export = Some(batch);
            }
            None => {
                w.drawings_note = Some("Storyboard Pro n'a pas été trouvé sur ce poste : les dessins ne sont pas repris (calques vides).".into());
                self.request(Pending::SbImport);
            }
        }
    }

    /// Lance l'import (le projet ouvert a été enregistré ou abandonné).
    pub(crate) fn start_sb_import(&mut self) {
        let Some(w) = self.sb_window.as_mut() else { return };
        let (Some(path), Some(Ok(sb))) = (w.sboard.clone(), w.project.as_ref()) else { return };
        let sb = sb.clone();
        let drawings = w.drawings.as_ref().and_then(|d| d.as_ref().ok()).cloned();
        let note = w.drawings_note.clone();
        // Le dossier de travail de l'export n'est plus utile une fois les dessins lus.
        let cleanup = w.work.take().filter(|_| drawings.is_some());
        let dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        let progress = Arc::new(AtomicU32::new(0));
        let result = Arc::new(Mutex::new(None));
        let (p, r) = (progress.clone(), result.clone());
        std::thread::spawn(move || {
            let report = |f: f32| p.store(f.to_bits(), Ordering::Relaxed);
            let (doc, mut warnings) = crate::sbimport::build(&sb, &dir, drawings.as_ref(), &report);
            if let Some(n) = note {
                warnings.insert(0, n);
            }
            if let Some(work) = cleanup {
                let _ = std::fs::remove_dir_all(work);
            }
            *r.lock().unwrap_or_else(|e| e.into_inner()) = Some((doc, warnings));
        });
        w.report = None;
        w.job = Some(SbJob { progress, result, name: sb_name(&path) });
        self.end_stroke();
        self.playing = false;
    }

    /// À chaque image d'affichage : la fenêtre, l'export et l'import qui avancent ou se
    /// terminent.
    pub(crate) fn sb_window_ui(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.sb_window.take() else { return };
        // Export fini : l'import suit, avec ou sans les dessins.
        let mut import = false;
        if let Some(res) = w.export.as_ref().and_then(Batch::finished) {
            w.export = None;
            match res {
                Ok(d) => {
                    w.drawings_dir = Some(d.dir.clone());
                    w.drawings = Some(Ok(d));
                }
                Err(e) => {
                    let kept = w.work.as_ref().map(|p| format!(" (dossier de travail gardé : {})", p.display())).unwrap_or_default();
                    w.drawings_note = Some(format!("Dessins non repris : {e}{kept}"));
                    w.drawings = Some(Err(e));
                }
            }
            import = true;
        }
        // Import fini : le projet remplace celui qui était ouvert.
        let done = w.job.as_ref().and_then(|j| j.result.lock().unwrap_or_else(|e| e.into_inner()).take());
        if let Some((doc, warnings)) = done {
            let name = w.job.take().map(|j| j.name).unwrap_or_default();
            let (plans, marks) = (doc.panels.len(), doc.panels.iter().map(|p| p.marks.len()).sum::<usize>());
            let sounds = doc.audio_tracks.iter().map(|t| t.clips.len()).sum::<usize>();
            if !self.charger_projet(doc) {
                self.status = format!("« {name} » n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.");
                w.report = Some(warnings);
                self.sb_window = Some(w);
                return;
            }
            self.modified = true;
            self.status = format!(
                "« {name} » ouvert : {plans} plan(s), {} image(s) de storyboard, {sounds} extrait(s) de son{}. « Enregistrer sous » pour le garder en projet SHOT.",
                plans + marks,
                if warnings.is_empty() { String::new() } else { format!(", {} remarque(s)", warnings.len()) }
            );
            w.report = Some(warnings);
        }
        if let Some(b) = &w.export {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
            let stage = b.stage.lock().unwrap_or_else(|e| e.into_inner()).clone();
            self.status = format!("{stage} {} / {} panneaux", b.done.load(Ordering::Relaxed), b.total);
        } else if let Some(j) = &w.job {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            self.status = format!("Ouverture de « {} » : {:.0} %", j.name, f32::from_bits(j.progress.load(Ordering::Relaxed)) * 100.0);
        }

        let mut open = true;
        egui::Window::new("Projet Storyboard Pro")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(520.0)
            .default_pos(ctx.screen_rect().center() - vec2(260.0, 220.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| import |= window_body(ui, &mut w, ctx));
        if open || import || w.busy() {
            self.sb_window = Some(w);
        }
        // Après avoir remis la fenêtre en place : l'import la lit.
        if import {
            self.request(Pending::SbImport);
        }
    }
}


fn sb_name(path: &Path) -> String {
    path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

fn read_drawings(w: &mut SbWindow) {
    if let Some(dir) = &w.drawings_dir {
        w.drawings = Some(sbscript::read_manifest(dir));
    }
}

/// Le contenu de la fenêtre ; vrai si l'on demande l'import.
fn window_body(ui: &mut egui::Ui, w: &mut SbWindow, ctx: &egui::Context) -> bool {
    let warn = crate::theme::avertissement();
    match &w.project {
        Some(Ok(sb)) => {
            let panels: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
            let clips: usize = sb.tracks.iter().map(|t| t.clips.len()).sum();
            ui.label(RichText::new(format!("« {} »", sb.title)).strong());
            ui.label(format!(
                "{} plan(s), {panels} panneau(x), {}×{} à {} i/s, {} piste(s) son, {clips} extrait(s).",
                sb.shots.len(),
                sb.size[0],
                sb.size[1],
                sb.fps,
                sb.tracks.len()
            ));
            if let Some(p) = &w.sboard {
                ui.label(RichText::new(p.display().to_string()).small().weak());
            }
        }
        Some(Err(e)) => {
            ui.colored_label(warn, format!("Illisible : {e}"));
        }
        None => {
            ui.label("Aucun projet : Fichier > Importer > Un projet Storyboard Pro (.sboard)…");
        }
    }
    ui.separator();
    // L'export des dessins par Storyboard Pro.
    let mut stop = false;
    if let Some(b) = &w.export {
        let stage = b.stage.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let done = b.done.load(Ordering::Relaxed);
        ui.label(RichText::new(stage).strong());
        ui.add(egui::ProgressBar::new(if b.total > 0 { done as f32 / b.total as f32 } else { 0.0 }).text(format!("{done} / {} panneaux", b.total)));
        ui.label(
            RichText::new("Storyboard Pro travaille sur une copie du projet, sans fenêtre : le projet d'origine n'est pas touché. Les calques vectoriels restent vectoriels quand leurs traits ont pu être lus.")
                .weak()
                .small(),
        );
        stop = ui.button("Arrêter (ouvrir sans les dessins)").clicked();
    }
    if stop {
        if let Some(b) = &w.export {
            b.cancel();
        }
    }
    if let Some(j) = &w.job {
        ui.label(RichText::new("Ouverture du projet…").strong());
        ui.add(egui::ProgressBar::new(f32::from_bits(j.progress.load(Ordering::Relaxed))).show_percentage());
    }
    let mut import = false;
    // Les dessins à la main, sans lancer Storyboard Pro.
    ui.add_enabled_ui(!w.busy(), |ui| {
        egui::CollapsingHeader::new("Dessins à la main (sans lancer Storyboard Pro)").default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Le script s'écrit dans un dossier ; dans Storyboard Pro, projet ouvert : vue Script Editor, Sandbox, coller, Run. Puis « Relire » et « Ouvrir avec ces dessins ».").weak());
            ui.horizontal(|ui| {
                if ui.button("Créer le script…").on_hover_text("Choisir le dossier où iront les dessins : le script y est écrit, et copié.").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Dossier où Storyboard Pro exportera les dessins").pick_folder() {
                        let text = sbscript::script(&dir);
                        match std::fs::write(dir.join(sbscript::SCRIPT), &text) {
                            Ok(()) => {
                                ctx.copy_text(text);
                                w.script_written = true;
                                w.drawings_dir = Some(dir);
                                w.drawings = None;
                            }
                            Err(e) => w.drawings = Some(Err(format!("écriture du script impossible : {e}"))),
                        }
                    }
                }
                if ui.button("Choisir le dossier des dessins…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Dossier des dessins exportés").pick_folder() {
                        w.drawings_dir = Some(dir);
                        read_drawings(w);
                    }
                }
                if w.drawings_dir.is_some() && ui.button("Relire").clicked() {
                    read_drawings(w);
                }
            });
            if w.script_written {
                ui.label(format!("Script écrit ({}) et copié.", sbscript::SCRIPT));
            }
            match &w.drawings {
                Some(Ok(d)) => {
                    let images = d.panels.iter().flat_map(|p| &p.layers).filter(|l| l.file.is_some()).count();
                    let traits = d.panels.iter().flat_map(|p| &p.layers).filter(|l| l.traits.is_some()).count();
                    ui.label(format!("{} panneau(x), {images} image(s) de calque, {traits} dessin(s) vectoriel(s).", d.panels.len()));
                }
                Some(Err(e)) => {
                    ui.colored_label(warn, e.as_str());
                }
                None => {}
            }
            let ready = matches!(w.project, Some(Ok(_)));
            if ui.add_enabled(ready, egui::Button::new("Ouvrir avec ces dessins")).clicked() {
                w.drawings_note = None;
                import = true;
            }
        });
    });
    if let Some(report) = &w.report {
        ui.separator();
        if report.is_empty() {
            ui.label("Projet ouvert, sans remarque.");
        } else {
            ui.label(RichText::new(format!("Projet ouvert ; {} remarque(s) :", report.len())).strong());
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for r in report {
                    ui.label(format!("• {r}"));
                }
            });
        }
    }
    import
}
