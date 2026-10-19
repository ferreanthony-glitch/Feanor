//! Fenêtre « Exporter pour le montage (EDL / XML / OTIO) » (Fichier > Exporter, action
//! `fichier.exporter_montage`) : les formats, le dossier, le time code de départ, la séquence, puis
//! l'export (`editorialjob.rs`) et son compte rendu. Ce qui est écrit et comment : `editorial.rs`.
//!
//! Règle « sans latence » : l'interface ne calcule que les images (quelques-unes par image d'écran,
//! comme l'export du film) ; ffmpeg, le fil du son et l'écriture des fichiers sont à côté. Tant que
//! les images se calculent le document ne bouge pas (`EditorialWindow::rendering`, lu par
//! `App::modal_open`) ; pendant le son, on peut déjà travailler.

use std::path::{Path, PathBuf};

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::App;
use crate::editorial::{self, Media, Options, Plan};
use crate::editorialjob::{self, EditorialJob, Report, Stage};

/// L'état de la fenêtre.
pub(crate) struct EditorialWindow {
    opts: Options,
    /// Dossier où sera créé `<Titre>_montage`.
    parent: Option<PathBuf>,
    title: String,
    /// Ce qui partirait maintenant, ou pourquoi rien ne peut partir.
    plan: Result<Plan, String>,
    /// Ce qui a servi à le calculer (choix, plan courant, étape d'annulation, projet).
    key: Option<(Options, usize, u64, u64, String)>,
    job: Option<EditorialJob>,
    result: Option<Result<Report, String>>,
}

impl EditorialWindow {
    /// SHOT calcule encore les images : le projet ne doit pas changer.
    pub(crate) fn rendering(&self) -> bool {
        self.job.as_ref().is_some_and(|j| j.running() && j.stage == Stage::Video)
    }

    fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(EditorialJob::running)
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

impl App {
    /// Fichier > Exporter > Pour le montage.
    pub(crate) fn open_editorial(&mut self) {
        self.end_stroke();
        self.exactify_idle();
        self.playing = false;
        if self.editorial.as_ref().is_some_and(EditorialWindow::busy) {
            return;
        }
        let mut opts = Options::default();
        // ProRes si ce ffmpeg sait l'encoder, sinon H.264.
        if let Some((_, Ok(enc))) = self.ffmpeg_info() {
            if !enc.prores {
                opts.media = Media::H264;
            }
        }
        let title = match self.doc.properties.title.trim() {
            "" => self.base_name_or("SHOT"),
            t => t.to_string(),
        };
        let parent = self.settings.montage_export_dir.clone().or_else(|| self.path.as_ref().or(self.import_source.as_ref()).and_then(|p| p.parent()).map(Path::to_path_buf));
        self.editorial = Some(EditorialWindow { opts, parent, title, plan: Err(String::new()), key: None, job: None, result: None });
    }

    fn start_editorial(&mut self, w: &mut EditorialWindow) {
        let Ok(plan) = w.plan.clone() else { return };
        let Some(parent) = w.parent.clone() else { return };
        let Some((ffmpeg, Ok(enc))) = self.ffmpeg_info().cloned() else {
            w.result = Some(Err("ffmpeg introuvable ou inutilisable : c'est lui qui fabrique les vidéos.".into()));
            return;
        };
        self.exactify_idle();
        w.result = None;
        match EditorialJob::start(plan, w.opts.clone(), ffmpeg, enc, &parent) {
            Ok(job) => {
                w.job = Some(job);
                self.playing = false;
            }
            Err(e) => w.result = Some(Err(e)),
        }
    }

    /// À chaque image d'affichage : la fenêtre, et l'export qui avance.
    pub(crate) fn editorial_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.editorial.take() else { return };
        if let Some(job) = &mut w.job {
            job.step(&self.doc, std::time::Duration::from_millis(40));
            let (done, total) = job.frames();
            match job.stage {
                Stage::Video => {
                    ctx.request_repaint();
                    self.status = format!("Export pour le montage : {} (image {done} / {total})", job.status());
                }
                Stage::Sound => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    self.status = format!("Export pour le montage : {}", job.status());
                }
                Stage::Done => {}
            }
        }
        if w.job.as_ref().is_some_and(|j| !j.running()) {
            // (Le compte rendu est copié, pas retiré : l'abandon d'un export sans compte rendu l'annule.)
            let job = w.job.take().expect("export");
            let outcome = job.outcome.clone().unwrap_or_else(|| Err("Export annulé.".into()));
            self.status = match &outcome {
                Ok(r) => format!("Exporté pour le montage : {}", r.dir.display()),
                Err(e) => format!("Export pour le montage : {e}"),
            };
            w.result = Some(outcome);
        }
        // Ce qui partirait maintenant : recalculé quand un choix, le plan courant ou le projet change.
        if w.job.is_none() {
            let key = (w.opts.clone(), self.cur_panel, self.history.serial(), self.doc_epoch, w.title.clone());
            if w.key.as_ref() != Some(&key) {
                w.plan = editorial::build_plan(&self.doc, &w.opts, self.cur_panel, &w.title);
                w.key = Some(key);
            }
        }

        let mut open = true;
        let mut start = false;
        egui::Window::new("Exporter pour le montage")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(540.0)
            .default_pos(ctx.screen_rect().center() - vec2(270.0, 260.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| start = self.editorial_body(ui, &mut w));
        if start {
            self.start_editorial(&mut w);
        }
        if !open {
            if let Some(job) = &mut w.job {
                job.cancel();
            }
        }
        if open || w.busy() {
            self.editorial = Some(w);
        }
    }

    /// Le contenu de la fenêtre ; vrai si l'on demande l'export.
    fn editorial_body(&mut self, ui: &mut egui::Ui, w: &mut EditorialWindow) -> bool {
        let warn = crate::theme::avertissement();
        let busy = w.busy();
        let fps = self.doc.fps;
        ui.label(
            RichText::new(
                "L'animatique découpée plan par plan, pour un monteur : un fichier vidéo par plan, et le montage en EDL (CMX 3600), Final Cut XML (Premiere Pro, DaVinci Resolve) ou OpenTimelineIO. Les clips portent les numéros des plans (SQ01_PL012A).",
            )
            .weak()
            .small(),
        );
        ui.separator();
        ui.add_enabled_ui(!busy, |ui| {
            egui::Grid::new("editorial_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
                ui.label("Fichiers de montage");
                ui.vertical(|ui| {
                    ui.checkbox(&mut w.opts.edl, "EDL CMX 3600 (.edl) : l'image seulement");
                    ui.checkbox(&mut w.opts.xml, "Final Cut Pro 7 XML (.xml) : Premiere Pro, DaVinci Resolve");
                    ui.checkbox(&mut w.opts.otio, "OpenTimelineIO (.otio)");
                });
                ui.end_row();

                ui.label("Dossier");
                ui.horizontal(|ui| {
                    match &w.parent {
                        Some(d) => ui.label(short_path(d)).on_hover_text(d.display().to_string()),
                        None => ui.colored_label(warn, "à choisir"),
                    };
                    if ui.button("Choisir…").clicked() {
                        let mut d = rfd::FileDialog::new().set_title("Dossier où créer l'export pour le montage");
                        if let Some(dir) = &w.parent {
                            d = d.set_directory(dir);
                        }
                        if let Some(p) = d.pick_folder() {
                            self.settings.montage_export_dir = Some(p.clone());
                            if let Err(e) = self.settings.save() {
                                self.status = format!("Réglages non enregistrés : {e}");
                            }
                            w.parent = Some(p);
                        }
                    }
                });
                ui.end_row();

                ui.label("Titre");
                ui.add(egui::TextEdit::singleline(&mut w.title).desired_width(260.0));
                ui.end_row();

                ui.label("Étendue");
                ui.vertical(|ui| {
                    let (a, b) = self.doc.sequence_run(self.cur_panel.min(self.doc.panels.len() - 1));
                    ui.radio_value(&mut w.opts.whole_film, true, format!("Tout le film ({} plans)", self.doc.panels.len()));
                    ui.radio_value(&mut w.opts.whole_film, false, format!("La séquence courante ({} plans)", b - a));
                });
                ui.end_row();

                ui.label("Time code de départ");
                ui.horizontal(|ui| {
                    let valid = editorial::parse_timecode(&w.opts.start_tc, fps).is_some();
                    let edit = egui::TextEdit::singleline(&mut w.opts.start_tc).desired_width(100.0).hint_text("01:00:00:00");
                    let edit = if valid { edit } else { edit.text_color(warn) };
                    ui.add(edit).on_hover_text(format!("HH:MM:SS:FF, FF de 0 à {} (sans saut d'images). 01:00:00:00 est l'usage pour une ligne de temps de montage.", fps - 1));
                    if editorial::ntsc_possible(fps) {
                        ui.checkbox(&mut w.opts.ntsc, "NTSC").on_hover_text(format!(
                            "Cadence réelle × 1000/1001 ({} au lieu de {fps}). Les numéros d'images ne changent pas ; le son reste à 48 kHz.",
                            editorial::Rate::new(fps, true).label()
                        ));
                    }
                });
                ui.end_row();

                ui.label("Vidéos");
                ui.vertical(|ui| {
                    ui.radio_value(&mut w.opts.media, Media::ProRes, "ProRes 422 HQ (.mov) : pour le montage");
                    ui.radio_value(&mut w.opts.media, Media::H264, "H.264 (.mp4) : plus léger");
                    ui.checkbox(&mut w.opts.half, "À moitié de la taille de l'image");
                });
                ui.end_row();

                ui.label("Son");
                ui.checkbox(&mut w.opts.sound, "Avec les pistes son (XML et OTIO)");
                ui.end_row();

                ui.label("Chemins");
                ui.checkbox(&mut w.opts.absolute, "Absolus dans le XML et l'OTIO (sinon relatifs au dossier)");
                ui.end_row();
            });
        });
        ui.separator();

        // Ce qui partirait.
        match &w.plan {
            Ok(plan) => {
                let kinds = [w.opts.edl, w.opts.xml, w.opts.otio].iter().filter(|b| **b).count();
                ui.label(format!(
                    "{} plan(s), {}, {} · {} · vidéos {}×{}",
                    plan.clips.len(),
                    crate::storyboard::duration_label(plan.total, fps),
                    plan.rate.label(),
                    editorial::timecode(plan.start_tc, fps),
                    plan.size[0],
                    plan.size[1]
                ));
                let mut extra = Vec::new();
                if !plan.dissolves.is_empty() {
                    extra.push(format!("{} fondu(s) enchaîné(s) écrit(s), avec poignées dans {} fichier(s)", plan.dissolves.len(), plan.handles()));
                }
                if !plan.audio.is_empty() {
                    extra.push(format!("{} extrait(s) son sur {} piste(s)", plan.audio.len(), plan.audio.iter().map(|a| a.track).max().unwrap_or(0)));
                }
                if !extra.is_empty() {
                    ui.label(RichText::new(extra.join(" · ")).small());
                }
                if !self.doc.numbering_locked {
                    ui.colored_label(warn, "La numérotation des plans est suivie : les noms des clips changent si un plan s'ajoute ou se déplace. Réglages du projet > Numérotation des plans > Verrouillée les fige.");
                }
                if !plan.skipped.is_empty() {
                    egui::CollapsingHeader::new(format!("Transitions pas écrites telles quelles ({})", plan.skipped.len())).default_open(false).show(ui, |ui| {
                        for k in &plan.skipped {
                            ui.label(format!("• {}", k.text));
                        }
                    });
                }
                if kinds == 0 {
                    ui.colored_label(warn, "Aucun format de montage coché : seuls les fichiers vidéo (et LISEZ-MOI.txt) seront écrits.");
                }
                if plan.odd_rate {
                    ui.colored_label(warn, format!("{} i/s n'est pas une cadence classique de l'EDL (24, 25, 30).", plan.rate.fps));
                }
            }
            Err(e) if e.is_empty() => {}
            Err(e) => {
                ui.colored_label(warn, e);
            }
        }
        if let Some(parent) = &w.parent {
            let dir = editorialjob::export_dir_for(parent, &w.title);
            if dir.exists() && !busy && w.job.is_none() {
                ui.colored_label(warn, format!("« {} » existe déjà : les fichiers du même nom seront remplacés.", short_path(&dir))).on_hover_text(dir.display().to_string());
            } else {
                ui.label(RichText::new(format!("Tout sera écrit dans « {} ».", short_path(&dir))).small()).on_hover_text(dir.display().to_string());
            }
        }

        // ffmpeg.
        let ffmpeg = self.ffmpeg_info().cloned();
        let mut ffmpeg_ok = false;
        match &ffmpeg {
            Some((path, Ok(enc))) => {
                let missing = match w.opts.media {
                    Media::ProRes if !enc.prores => Some("ce ffmpeg ne sait pas encoder en ProRes : choisis H.264"),
                    Media::H264 if enc.h264.is_none() => Some("ce ffmpeg ne sait pas encoder en H.264"),
                    _ => None,
                };
                ffmpeg_ok = missing.is_none();
                ui.label(RichText::new(format!("ffmpeg : {}", short_path(path))).small()).on_hover_text(path.display().to_string());
                if let Some(m) = missing {
                    ui.colored_label(warn, m);
                }
            }
            Some((path, Err(e))) => {
                ui.colored_label(warn, format!("{} : {e}", path.display()));
            }
            None => {
                ui.colored_label(warn, "ffmpeg introuvable. C'est lui qui fabrique les vidéos : installe-le (il vient par exemple avec Krita), ou indique où il est.");
            }
        }
        if !busy && ui.small_button("Choisir un autre ffmpeg…").clicked() {
            self.choose_ffmpeg();
        }
        ui.separator();

        let tc_ok = editorial::parse_timecode(&w.opts.start_tc, fps).is_some();
        let ready = !busy && ffmpeg_ok && tc_ok && w.parent.is_some() && w.plan.is_ok() && !w.title.trim().is_empty();
        let mut start = false;
        match &w.job {
            Some(job) => {
                let (done, total) = job.frames();
                ui.label(RichText::new(job.status()).strong());
                ui.add(egui::ProgressBar::new(if total > 0 { done as f32 / total as f32 } else { 0.0 }).text(format!("images {done} / {total}")));
                let (sd, st) = job.sounds();
                if st > 0 {
                    ui.add(egui::ProgressBar::new(sd as f32 / st as f32).text(format!("son {sd} / {st}")));
                }
                if let Some(rest) = job_remaining(job) {
                    ui.label(RichText::new(format!("reste environ {}:{:02}", rest / 60, rest % 60)).small().weak());
                }
                if ui.button("Arrêter").on_hover_text("Arrête l'export et efface ce qu'il a déjà écrit.").clicked() {
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
            Some(Ok(r)) => {
                ui.separator();
                ui.label(RichText::new(format!("Terminé : {} plan(s), {} extrait(s) son.", r.plans, r.sounds)).strong().color(crate::theme::succes()));
                ui.label(RichText::new(short_path(&r.dir)).small()).on_hover_text(r.dir.display().to_string());
                for f in &r.files {
                    ui.label(RichText::new(format!("• {}", f.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()))).small());
                }
                ui.label(RichText::new("L'import dans Premiere Pro, DaVinci Resolve et Avid est à vérifier : LISEZ-MOI.txt dit quoi regarder.").small().weak());
                if ui.button("Ouvrir le dossier").clicked() {
                    crate::exportui::reveal_in_explorer(&r.dir);
                }
            }
            Some(Err(e)) => {
                ui.separator();
                ui.colored_label(warn, e);
            }
            None => {}
        }
        start
    }
}

/// Temps restant estimé, en secondes (d'après les images faites).
fn job_remaining(job: &EditorialJob) -> Option<u64> {
    let (done, total) = job.frames();
    (done >= 8 && total > done).then(|| (job.started.elapsed().as_secs_f64() / done as f64 * (total - done) as f64) as u64)
}
