//! Fenêtre « Importer un projet Storyboard Pro » (14a). On choisit le `.sboard` (lu, jamais
//! modifié), puis, pour avoir les dessins, le dossier où le script de Pelure les a exportés
//! (bouton « Créer le script… » : le script y est écrit et copié, prêt à coller dans Storyboard
//! Pro). L'import se fait sur un autre fil et remplace le projet ouvert (après avoir proposé de
//! l'enregistrer). Voir `sboard`, `sbscript`, `sbimport`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, vec2, Align2, Color32, RichText};

use crate::app::{App, Pending};
use crate::document::Document;
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

impl App {
    /// Fichier > Importer un projet Storyboard Pro.
    pub(crate) fn open_sb_window(&mut self) {
        if self.sb_window.is_none() {
            self.sb_window = Some(SbWindow::default());
        }
        let w = self.sb_window.as_mut().expect("fenêtre ouverte");
        if w.sboard.is_none() && w.job.is_none() {
            pick_sboard(w);
        }
    }

    /// Un fichier lâché sur la fenêtre : un `.sboard` ouvre la fenêtre d'import avec ce projet ;
    /// la liste écrite par le script (`pelure.txt`) y choisit le dossier des dessins. Faux si ce
    /// n'est ni l'un ni l'autre.
    pub(crate) fn import_dropped_sboard(&mut self, path: &Path) -> bool {
        let project = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("sboard"));
        let manifest = path.file_name().is_some_and(|n| n.eq_ignore_ascii_case(sbscript::MANIFEST));
        if !project && !manifest {
            return false;
        }
        let w = self.sb_window.get_or_insert_with(SbWindow::default);
        if w.job.is_some() {
            return true;
        }
        if project {
            w.project = Some(sboard::read(path));
            w.sboard = Some(path.to_path_buf());
        } else {
            w.drawings_dir = path.parent().map(Path::to_path_buf);
            read_drawings(w);
        }
        w.report = None;
        true
    }

    /// Lance l'import (le projet ouvert a été enregistré ou abandonné).
    pub(crate) fn start_sb_import(&mut self) {
        let Some(w) = self.sb_window.as_mut() else { return };
        let (Some(path), Some(Ok(sb))) = (w.sboard.clone(), w.project.as_ref()) else { return };
        let sb = sb.clone();
        let drawings = w.drawings.as_ref().and_then(|d| d.as_ref().ok()).cloned();
        let dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        let progress = Arc::new(AtomicU32::new(0));
        let result = Arc::new(Mutex::new(None));
        let (p, r) = (progress.clone(), result.clone());
        std::thread::spawn(move || {
            let report = |f: f32| p.store(f.to_bits(), Ordering::Relaxed);
            let built = crate::sbimport::build(&sb, &dir, drawings.as_ref(), &report);
            *r.lock().unwrap_or_else(|e| e.into_inner()) = Some(built);
        });
        w.report = None;
        w.job = Some(SbJob { progress, result, name: sb_name(&path) });
        self.end_stroke();
        self.playing = false;
    }

    /// À chaque image d'affichage : la fenêtre, et l'import qui avance ou se termine.
    pub(crate) fn sb_window_ui(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.sb_window.take() else { return };
        // Import fini : le projet remplace celui qui était ouvert.
        let done = w.job.as_ref().and_then(|j| j.result.lock().unwrap_or_else(|e| e.into_inner()).take());
        if let Some((doc, warnings)) = done {
            let name = w.job.take().map(|j| j.name).unwrap_or_default();
            let (plans, marks) = (doc.panels.len(), doc.panels.iter().map(|p| p.marks.len()).sum::<usize>());
            let sounds = doc.audio_tracks.iter().map(|t| t.clips.len()).sum::<usize>();
            self.load_doc(doc);
            self.modified = true;
            self.status = format!(
                "« {name} » importé : {plans} plan(s), {} image(s) de storyboard, {sounds} extrait(s) de son{}. « Enregistrer sous » pour le garder.",
                plans + marks,
                if warnings.is_empty() { String::new() } else { format!(", {} remarque(s)", warnings.len()) }
            );
            w.report = Some(warnings);
        }
        if let Some(j) = &w.job {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            self.status = format!("Import de « {} » : {:.0} %", j.name, f32::from_bits(j.progress.load(Ordering::Relaxed)) * 100.0);
        }

        let mut open = true;
        let mut import = false;
        egui::Window::new("Importer un projet Storyboard Pro")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(520.0)
            .default_pos(ctx.screen_rect().center() - vec2(260.0, 220.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| import = window_body(ui, &mut w, ctx));
        if open || import || w.job.is_some() {
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

fn pick_sboard(w: &mut SbWindow) {
    let Some(path) = rfd::FileDialog::new().set_title("Projet Storyboard Pro à importer").add_filter("Projet Storyboard Pro", &["sboard"]).pick_file() else {
        return;
    };
    w.project = Some(sboard::read(&path));
    w.sboard = Some(path);
    w.report = None;
}

fn read_drawings(w: &mut SbWindow) {
    if let Some(dir) = &w.drawings_dir {
        w.drawings = Some(sbscript::read_manifest(dir));
    }
}

/// Le contenu de la fenêtre ; vrai si l'on demande l'import.
fn window_body(ui: &mut egui::Ui, w: &mut SbWindow, ctx: &egui::Context) -> bool {
    let busy = w.job.is_some();
    let warn = Color32::from_rgb(240, 170, 60);
    ui.add_enabled_ui(!busy, |ui| {
        ui.label(RichText::new("1. Projet").strong());
        ui.horizontal(|ui| {
            let text = w.sboard.as_ref().map_or("Aucun projet choisi.".into(), |p| p.display().to_string());
            ui.label(text);
            if ui.button("Choisir…").clicked() {
                pick_sboard(w);
            }
        });
        match &w.project {
            Some(Ok(sb)) => {
                let panels: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
                let clips: usize = sb.tracks.iter().map(|t| t.clips.len()).sum();
                ui.label(format!(
                    "« {} » : {} plan(s), {panels} panneau(x), {}×{} à {} i/s, {} piste(s) son, {clips} extrait(s).",
                    sb.title,
                    sb.shots.len(),
                    sb.size[0],
                    sb.size[1],
                    sb.fps,
                    sb.tracks.len()
                ));
                ui.label(RichText::new("Le fichier est seulement lu : il n'est jamais modifié.").weak());
            }
            Some(Err(e)) => {
                ui.colored_label(warn, format!("Illisible : {e}"));
            }
            None => {}
        }
        ui.add_space(8.0);
        ui.label(RichText::new("2. Dessins (facultatif)").strong());
        ui.label(
            RichText::new(
                "Les dessins d'un projet Storyboard Pro sont dans un format fermé : Storyboard Pro les exporte lui-même, \
                 calque par calque, avec un script écrit par Pelure. Sans eux, les calques arrivent vides.",
            )
            .weak(),
        );
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
            if w.drawings_dir.is_some() && ui.button("Relire").on_hover_text("Relire le dossier après l'export.").clicked() {
                read_drawings(w);
            }
        });
        if let Some(dir) = &w.drawings_dir {
            ui.label(format!("Dossier : {}", dir.display()));
        }
        if w.script_written {
            ui.label(format!(
                "Script écrit ({}) et copié. Dans Storyboard Pro, le projet ouvert : vue Script Editor, choisir Sandbox, \
                 coller, puis Run. L'export peut durer plusieurs minutes. Ensuite : « Relire ».",
                sbscript::SCRIPT
            ));
        }
        match &w.drawings {
            Some(Ok(d)) => {
                let images = d.panels.iter().flat_map(|p| &p.layers).filter(|l| l.file.is_some()).count();
                ui.label(format!("{} panneau(x) exporté(s), {images} image(s) de calque.", d.panels.len()));
            }
            Some(Err(e)) => {
                ui.colored_label(warn, e.as_str());
            }
            None => {}
        }
        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("La caméra n'est pas reprise. L'import remplace le projet ouvert.").weak());
    });
    let mut import = false;
    ui.horizontal(|ui| {
        let ready = !busy && matches!(w.project, Some(Ok(_)));
        if ui.add_enabled(ready, egui::Button::new("Importer")).clicked() {
            import = true;
        }
        if let Some(j) = &w.job {
            ui.add(egui::ProgressBar::new(f32::from_bits(j.progress.load(Ordering::Relaxed))).show_percentage());
        }
    });
    if let Some(report) = &w.report {
        ui.separator();
        if report.is_empty() {
            ui.label("Import terminé, sans remarque.");
        } else {
            ui.label(RichText::new(format!("Import terminé ; {} remarque(s) :", report.len())).strong());
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for r in report {
                    ui.label(format!("• {r}"));
                }
            });
        }
    }
    import
}
