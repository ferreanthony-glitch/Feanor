//! Fenêtre « Exporter vers Storyboard Pro » (14b-1) : le projet modèle (choisi une fois, gardé
//! dans les réglages), le dossier et le titre, puis l'export (`sbexport`) et son compte rendu.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::App;
use crate::sbexport::{self, Built, SbExportJob, SbOut, Stage};

/// L'état de la fenêtre.
pub(crate) struct SbExportWindow {
    title: String,
    /// Ce qui partirait maintenant (calculé à l'ouverture de la fenêtre).
    preview: SbOut,
    /// Le projet modèle lu : sa description et ses écarts avec le projet ; ou pourquoi il est
    /// illisible.
    template_info: Option<(PathBuf, Result<(String, Vec<String>), String>)>,
    job: Option<SbExportJob>,
    result: Option<Result<Built, String>>,
    /// Ce qui n'est pas repris tel quel, pour le dernier export.
    notes: Vec<String>,
    /// Dossier de travail gardé après un échec (journal de Storyboard Pro).
    kept: Option<PathBuf>,
}

impl SbExportWindow {
    /// Pelure calcule encore les images : le projet ne doit pas changer.
    pub(crate) fn rendering(&self) -> bool {
        self.job.as_ref().is_some_and(|j| matches!(j.stage, Stage::Images))
    }

    fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(SbExportJob::running)
    }
}

/// Un chemin en court (« …\dossier\nom »), le chemin complet en bulle.
fn short_path(p: &Path) -> String {
    let parts: Vec<String> = p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.len() <= 3 {
        p.display().to_string()
    } else {
        format!("…\\{}", parts[parts.len() - 2..].join("\\"))
    }
}

/// Le projet modèle, comparé au projet Pelure.
fn template_info(path: &Path, doc_frame: [usize; 2], doc_fps: u32) -> Result<(String, Vec<String>), String> {
    let sb = crate::sboard::read_template(path)?;
    let panels: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
    let desc = format!("{}×{} à {} i/s", sb.size[0], sb.size[1], sb.fps);
    let mut warnings = Vec::new();
    let [w, h] = doc_frame;
    let [tw, th] = sb.size;
    if tw * h != th * w {
        warnings.push(format!("Le projet SHOT est en {w}×{h} : les images ne seront pas aux proportions du modèle ({tw}×{th})."));
    } else if [tw, th] != doc_frame {
        warnings.push(format!("Le projet SHOT est en {w}×{h}, le modèle en {tw}×{th} : Storyboard Pro mettra les images à sa taille."));
    }
    if sb.fps != doc_fps {
        warnings.push(format!("Le projet SHOT est à {doc_fps} i/s, le modèle à {} i/s : les durées en images sont gardées, pas en secondes.", sb.fps));
    }
    if panels > 0 {
        warnings.push(format!("Le modèle a {} plan(s), {panels} panneau(x) : ils ne seront pas gardés (seuls ses réglages servent).", sb.shots.len()));
    }
    Ok((desc, warnings))
}

impl App {
    /// Fichier > Exporter vers Storyboard Pro.
    pub(crate) fn open_sb_export(&mut self) {
        self.end_stroke();
        if self.sb_export.as_ref().is_some_and(SbExportWindow::busy) {
            return;
        }
        let title = match self.doc.properties.title.trim() {
            "" => self.base_name_or("Projet SHOT"),
            t => t.to_string(),
        };
        let preview = sbexport::prepare(&self.doc, &title);
        self.sb_export = Some(SbExportWindow { title, preview, template_info: None, job: None, result: None, notes: Vec::new(), kept: None });
    }

    fn start_sb_export(&mut self, w: &mut SbExportWindow) {
        let (Some(template), Some(dir)) = (self.settings.sboard_template.clone(), self.export_dir()) else { return };
        let Some(exe) = crate::sbbatch::find_storyboard_pro() else { return };
        let out = sbexport::prepare(&self.doc, &w.title);
        let dest = dir.join(sbexport::safe_name(&w.title));
        w.notes = out.notes.clone();
        w.result = None;
        w.kept = None;
        match SbExportJob::start(out, exe, template, dest) {
            Ok(job) => {
                w.job = Some(job);
                self.playing = false;
            }
            Err(e) => w.result = Some(Err(e)),
        }
    }

    /// Dossier où va le projet : le dernier choisi, sinon celui du projet ouvert.
    fn export_dir(&self) -> Option<PathBuf> {
        self.settings
            .sboard_export_dir
            .clone()
            .or_else(|| self.path.as_ref().or(self.import_source.as_ref()).and_then(|p| p.parent()).map(Path::to_path_buf))
    }

    /// À chaque image d'affichage : la fenêtre, et l'export qui avance.
    pub(crate) fn sb_export_ui(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.sb_export.take() else { return };
        if let Some(job) = &mut w.job {
            job.step(&self.doc, std::time::Duration::from_millis(40));
            match &job.stage {
                Stage::Images => {
                    ctx.request_repaint();
                    self.status = format!("Export vers Storyboard Pro : images des calques {} / {}", job.next, job.total_images());
                }
                Stage::Storyboard(b) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(300));
                    let stage = b.stage.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    self.status = format!("Export vers Storyboard Pro : {stage} {} / {} plans", b.done.load(Ordering::Relaxed), b.total);
                }
                Stage::Done(_) => {}
            }
        }
        if w.job.as_ref().is_some_and(|j| !j.running()) {
            let job = w.job.take().expect("export");
            if let Stage::Done(res) = job.stage {
                self.status = match &res {
                    Ok(b) => format!("Exporté vers Storyboard Pro : {}", b.project.display()),
                    Err(e) => format!("Export vers Storyboard Pro : {e}"),
                };
                if res.is_err() && job.work.exists() {
                    w.kept = Some(job.work.clone());
                }
                w.result = Some(res);
            }
        }
        // Le modèle est relu quand il change.
        if let Some(t) = self.settings.sboard_template.clone() {
            if w.template_info.as_ref().is_none_or(|(p, _)| *p != t) {
                let info = template_info(&t, self.doc.frame, self.doc.fps);
                w.template_info = Some((t, info));
            }
        }

        let mut open = true;
        let mut start = false;
        egui::Window::new("Exporter vers Storyboard Pro")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(540.0)
            .default_pos(ctx.screen_rect().center() - vec2(270.0, 240.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| start = self.sb_export_body(ui, &mut w));
        if start {
            self.start_sb_export(&mut w);
        }
        if !open {
            if let Some(job) = &mut w.job {
                job.cancel();
            }
        }
        if open || w.busy() {
            self.sb_export = Some(w);
        }
    }

    /// Le contenu de la fenêtre ; vrai si l'on demande l'export.
    fn sb_export_body(&mut self, ui: &mut egui::Ui, w: &mut SbExportWindow) -> bool {
        let warn = crate::theme::avertissement();
        let busy = w.busy();
        let exe = crate::sbbatch::find_storyboard_pro();
        let (plans, panels, drawn, clips) = sbexport::counts(&w.preview);
        ui.label(format!("{plans} plan(s), {panels} image(s) de storyboard (une par panneau), {drawn} image(s) de calque, {clips} extrait(s) de son."));
        ui.label(
            RichText::new("Chaque plan devient un plan, chaque image de storyboard un panneau (durée, légendes), chaque calque un calque (son dessin en image, son nom, sa visibilité, son opacité). Les sons suivent, et les séquences.")
                .weak()
                .small(),
        );
        ui.separator();
        let mut changed_settings = false;
        ui.add_enabled_ui(!busy, |ui| {
            egui::Grid::new("sb_export_grille").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Projet modèle");
                ui.horizontal(|ui| {
                    match &self.settings.sboard_template {
                        Some(t) => ui.label(t.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned())).on_hover_text(t.display().to_string()),
                        None => ui.colored_label(warn, "à choisir"),
                    };
                    let tip = "Un projet Storyboard Pro vide, à vos réglages (résolution, cadence, légendes) : l'export part d'une copie. Choisi une fois, gardé.";
                    if ui.button("Choisir…").on_hover_text(tip).clicked() {
                        let mut d = rfd::FileDialog::new().set_title("Projet modèle de Storyboard Pro").add_filter("Projet Storyboard Pro", &["sboard"]);
                        if let Some(dir) = self.settings.sboard_template.as_ref().and_then(|t| t.parent()).and_then(|p| p.parent()) {
                            d = d.set_directory(dir);
                        }
                        if let Some(p) = d.pick_file() {
                            self.settings.sboard_template = Some(p);
                            changed_settings = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Dossier");
                ui.horizontal(|ui| {
                    match self.export_dir() {
                        Some(d) => ui.label(short_path(&d)).on_hover_text(d.display().to_string()),
                        None => ui.colored_label(warn, "à choisir"),
                    };
                    if ui.button("Choisir…").clicked() {
                        let mut d = rfd::FileDialog::new().set_title("Dossier où enregistrer le projet Storyboard Pro");
                        if let Some(dir) = self.export_dir() {
                            d = d.set_directory(dir);
                        }
                        if let Some(p) = d.pick_folder() {
                            self.settings.sboard_export_dir = Some(p);
                            changed_settings = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Titre");
                ui.add(egui::TextEdit::singleline(&mut w.title).desired_width(300.0));
                ui.end_row();
            });
        });
        if changed_settings {
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
        if let Some((_, info)) = &w.template_info {
            match info {
                Ok((desc, warnings)) => {
                    ui.label(RichText::new(format!("Modèle : {desc}.")).small());
                    for x in warnings {
                        ui.colored_label(warn, x);
                    }
                }
                Err(e) => {
                    ui.colored_label(warn, format!("Modèle illisible : {e}"));
                }
            }
        }
        let dest = self.export_dir().map(|d| d.join(sbexport::safe_name(&w.title)));
        let exists = dest.as_ref().is_some_and(|d| d.exists());
        if let Some(d) = &dest {
            if exists && !busy {
                ui.colored_label(warn, format!("« {} » existe déjà : choisir un autre titre ou un autre dossier.", short_path(d))).on_hover_text(d.display().to_string());
            } else {
                ui.label(RichText::new(format!("Le projet sera enregistré dans « {} ».", short_path(d))).small()).on_hover_text(d.display().to_string());
            }
        }
        if exe.is_none() {
            ui.colored_label(warn, "Storyboard Pro n'a pas été trouvé sur ce poste : l'export a besoin de lui.");
        }
        if !w.preview.notes.is_empty() && w.job.is_none() && w.result.is_none() {
            egui::CollapsingHeader::new(format!("Ce qui n'est pas repris tel quel ({})", w.preview.notes.len())).default_open(false).show(ui, |ui| {
                for n in &w.preview.notes {
                    ui.label(format!("• {n}"));
                }
            });
        }
        ui.separator();
        let template_ok = matches!(&w.template_info, Some((_, Ok(_))));
        let ready = !busy && exe.is_some() && template_ok && dest.is_some() && !exists && !w.title.trim().is_empty();
        let mut start = false;
        match &w.job {
            Some(job) => {
                let (label, done, total) = match &job.stage {
                    Stage::Images => ("SHOT calcule les images des calques…".to_string(), job.next, job.total_images()),
                    Stage::Storyboard(b) => (b.stage.lock().unwrap_or_else(|e| e.into_inner()).clone(), b.done.load(Ordering::Relaxed), b.total),
                    Stage::Done(_) => (String::new(), 0, 0),
                };
                ui.label(RichText::new(label).strong());
                let unit = if matches!(job.stage, Stage::Images) { "images" } else { "plans" };
                ui.add(egui::ProgressBar::new(if total > 0 { done as f32 / total as f32 } else { 0.0 }).text(format!("{done} / {total} {unit}")));
                ui.label(RichText::new("Storyboard Pro travaille sans fenêtre, sur une copie du modèle : le modèle n'est pas touché.").weak().small());
                if ui.button("Arrêter").clicked() {
                    if let Some(job) = &mut w.job {
                        job.cancel();
                    }
                }
            }
            None => {
                start = ui.add_enabled(ready, egui::Button::new("Exporter")).clicked();
            }
        }
        match &w.result {
            Some(Ok(b)) => {
                ui.separator();
                ui.label(RichText::new(format!("Projet enregistré : {}", short_path(&b.project))).strong()).on_hover_text(b.project.display().to_string());
                if ui.button("Ouvrir le dossier").clicked() {
                    if let Some(dir) = b.project.parent() {
                        let _ = std::process::Command::new("explorer").arg(dir).spawn();
                    }
                }
                let remarks: Vec<&String> = w.notes.iter().chain(&b.report.remarks).collect();
                if !remarks.is_empty() {
                    ui.label(format!("{} remarque(s) :", remarks.len()));
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for r in remarks {
                            ui.label(format!("• {r}"));
                        }
                    });
                }
            }
            Some(Err(e)) => {
                ui.separator();
                ui.colored_label(warn, format!("Export non fait : {e}"));
                if let Some(k) = &w.kept {
                    ui.label(RichText::new(format!("Dossier de travail gardé (journal de Storyboard Pro) : {}", short_path(k))).small()).on_hover_text(k.display().to_string());
                }
            }
            None => {}
        }
        start
    }
}
