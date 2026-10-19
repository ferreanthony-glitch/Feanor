//! Fenêtres d'export (3b) : choix (contenu, étendue, taille, son), puis progression.
//! Toutes deux modales : rien ne bouge dans le document pendant un export.

use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText};

use crate::app::App;
use crate::audio;
use crate::export::{self, ExportJob, ExportSettings, Format, Progress, Range};
use crate::storyboard::duration_label;

fn warn() -> Color32 {
    crate::theme::avertissement()
}

fn good() -> Color32 {
    crate::theme::succes()
}

impl App {
    /// Ouvre la fenêtre d'export, sur le contenu `format`.
    pub(crate) fn open_export(&mut self, format: Format) {
        self.end_stroke();
        self.playing = false;
        if self.export_job.is_some() {
            return;
        }
        let mut s = self.export_last.clone();
        s.format = format;
        s.transparent = self.export_transparent;
        self.export_dialog = Some(s);
    }

    /// ffmpeg et ses encodeurs, cherchés une fois (et après un choix à la main).
    fn ffmpeg_info(&mut self) -> Option<&(PathBuf, Result<export::Encoders, String>)> {
        if self.ffmpeg.is_none() {
            let found = export::find_ffmpeg(self.settings.ffmpeg.as_deref()).map(|p| {
                let enc = export::probe(&p);
                (p, enc)
            });
            self.ffmpeg = Some(found);
        }
        self.ffmpeg.as_ref().and_then(|f| f.as_ref())
    }

    fn choose_ffmpeg(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Où est ffmpeg.exe ?")
            .add_filter("ffmpeg", &["exe"])
            .pick_file()
        else {
            return;
        };
        self.settings.ffmpeg = Some(path);
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglage non enregistré : {e}");
        }
        self.ffmpeg = None;
    }

    pub(crate) fn export_windows(&mut self, ctx: &egui::Context) {
        self.export_dialog_window(ctx);
        self.export_progress_window(ctx);
    }

    fn export_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut s) = self.export_dialog.take() else { return };
        let fps = self.doc.fps;
        let [fw, fh] = self.doc.frame;
        let count = |range| export::frames_for(&self.doc, range, self.cur_panel).len();
        let ranges = [
            (Range::Montage, "Tout le montage", count(Range::Montage)),
            (Range::Scene, "Séquence courante", count(Range::Scene)),
            (Range::Panel, "Plan courant", count(Range::Panel)),
        ];
        // Le son de l'étendue choisie (pour griser « avec le son » s'il n'y en a pas).
        let frames = export::frames_for(&self.doc, s.range, self.cur_panel);
        let first = frames.first().map_or(0, |&(p, f)| self.doc.panel_start(p) + f);
        let (start, len) = export::sample_span(fps, first, frames.len());
        let sound_here = export::has_sound(&audio::mix_clips(&self.doc), start, len);
        let ffmpeg = if s.format.is_video() { self.ffmpeg_info().cloned() } else { None };
        let has_transitions = !self.doc.transitions.is_empty();

        let mut go = false;
        let mut close = false;
        let mut choose = false;
        let modal = egui::Modal::new(egui::Id::new("fenetre_export")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Exporter");
            ui.add_space(6.0);
            egui::Grid::new("grille_export").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Contenu");
                ui.vertical(|ui| {
                    ui.radio_value(&mut s.format, Format::Mp4, "Film MP4 (H.264) : pour regarder, envoyer");
                    ui.radio_value(&mut s.format, Format::ProRes, "Film MOV (ProRes 422 HQ) : pour le montage");
                    ui.radio_value(&mut s.format, Format::Png, "Images PNG (une par image)");
                    ui.radio_value(&mut s.format, Format::Wav, "Son seul (WAV)");
                    ui.radio_value(&mut s.format, Format::Pdf, "Storyboard PDF (A4 paysage, 6 images par page)");
                });
                ui.end_row();
                ui.label("Étendue");
                ui.vertical(|ui| {
                    for (range, label, n) in ranges {
                        ui.radio_value(&mut s.range, range, format!("{label} ({})", duration_label(n, fps)));
                    }
                });
                ui.end_row();
                if s.format.is_video() {
                    ui.label("Taille");
                    ui.vertical(|ui| {
                        let [w, h] = export::video_size([fw, fh], false);
                        let [hw, hh] = export::video_size([fw, fh], true);
                        ui.radio_value(&mut s.half, false, format!("Image finale ({w}×{h})"));
                        ui.radio_value(&mut s.half, true, format!("Moitié ({hw}×{hh}), fichier plus léger"));
                    });
                    ui.end_row();
                    ui.label("Son");
                    if sound_here {
                        ui.checkbox(&mut s.sound, "Avec le son du montage");
                    } else {
                        ui.label(RichText::new("Aucun son dans cette étendue.").weak());
                    }
                    ui.end_row();
                }
                if s.format == Format::Png {
                    ui.label("Fond");
                    ui.checkbox(&mut s.transparent, "Sans le papier (fond transparent)");
                    ui.end_row();
                }
                // Les fondus, volets et noirs du montage (`transitions.rs`) : calculés comme le reste
                // de l'export, image par image.
                if (s.format.is_video() || s.format == Format::Png) && has_transitions {
                    ui.label("Transitions");
                    ui.checkbox(&mut s.transitions, "Avec les transitions du montage");
                    ui.end_row();
                }
            });
            if s.format.is_video() {
                ui.add_space(6.0);
                ui.separator();
                match &ffmpeg {
                    Some((path, Ok(enc))) => {
                        let missing = match s.format {
                            Format::Mp4 if enc.h264.is_none() => Some("ce ffmpeg ne sait pas encoder en H.264"),
                            Format::ProRes if !enc.prores => Some("ce ffmpeg ne sait pas encoder en ProRes"),
                            _ => None,
                        };
                        ui.label(RichText::new(format!("ffmpeg : {}", path.display())).small());
                        if let Some(m) = missing {
                            ui.colored_label(warn(), m);
                        }
                    }
                    Some((path, Err(e))) => {
                        ui.colored_label(warn(), format!("{} : {e}", path.display()));
                    }
                    None => {
                        ui.colored_label(
                            warn(),
                            "ffmpeg introuvable. C'est lui qui fabrique la vidéo : installe-le (il vient par \
                             exemple avec Krita), ou indique où il est.",
                        );
                    }
                }
                choose = ui.button("Choisir un autre ffmpeg…").clicked();
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                go = ui.button("Exporter…").clicked();
                close = ui.button("Annuler").clicked();
            });
        });
        if choose {
            self.choose_ffmpeg();
        }
        if go {
            self.export_last = s.clone();
            if s.format == Format::Png {
                self.export_transparent = s.transparent;
            }
            self.start_export(&s, frames, first, sound_here && s.sound);
        } else if !close && !modal.should_close() {
            self.export_dialog = Some(s);
        }
    }

    /// Lance l'export choisi : demande où écrire, puis démarre (ou écrit directement le son).
    fn start_export(&mut self, s: &ExportSettings, frames: Vec<(usize, usize)>, first: usize, sound: bool) {
        // (Traits texturés restés en rendu rapide : l'export les veut exacts.)
        self.exactify_idle();
        let base = self.base_name();
        let fps = self.doc.fps;
        match s.format {
            Format::Wav => {
                let Some(path) = save_dialog(&base, Format::Wav) else { return };
                let (start, len) = export::sample_span(fps, first, frames.len());
                // (Le son exporté attend l'égaliseur et l'écho : `mix_clips_blocking`.)
                self.status = match export::write_wav(&path, audio::mix_clips_blocking(&self.doc), start, len) {
                    Ok(()) => format!("Son exporté : {}", path.display()),
                    Err(e) => format!("Échec de l'export du son : {e}"),
                };
            }
            Format::Png => {
                let Some(dir) = Self::pick_folder("Dossier où exporter les images") else { return };
                let job = ExportJob::images(&dir, &base, frames, self.doc.frame, !s.transparent);
                self.export_job = Some(if s.transitions { job.with_transitions(&self.doc) } else { job });
            }
            Format::Pdf => {
                let Some(out) = save_dialog(&base, Format::Pdf) else { return };
                let (shots, boards) = export::storyboard_of(&self.doc, &export::panels_of(&frames));
                self.export_job = Some(ExportJob::pdf(&out, &base, shots, boards, self.doc.frame));
            }
            Format::Mp4 | Format::ProRes => {
                let Some((ffmpeg, Ok(enc))) = self.ffmpeg_info().cloned() else {
                    self.status = "Export impossible : ffmpeg introuvable ou inutilisable.".into();
                    return;
                };
                let Some(out) = save_dialog(&base, s.format) else { return };
                // Le son est mixé d'avance dans un fichier temporaire, que ffmpeg lit à côté des images.
                let wav = if sound {
                    let path = std::env::temp_dir().join(format!("pelure-export-{}.wav", std::process::id()));
                    let (start, len) = export::sample_span(fps, first, frames.len());
                    if let Err(e) = export::write_wav(&path, audio::mix_clips_blocking(&self.doc), start, len) {
                        self.status = format!("Échec du mixage du son : {e}");
                        return;
                    }
                    Some(path)
                } else {
                    None
                };
                let args = match export::video_args(s.format, &enc, self.doc.frame, fps, s.half, wav.as_deref(), &out) {
                    Ok(a) => a,
                    Err(e) => {
                        self.status = format!("Export impossible : {e}");
                        return;
                    }
                };
                match ExportJob::video(&ffmpeg, args, &out, frames, wav) {
                    Ok(job) => self.export_job = Some(if s.transitions { job.with_transitions(&self.doc) } else { job }),
                    Err(e) => self.status = format!("Export impossible : {e}"),
                }
            }
        }
    }

    fn export_progress_window(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.export_job else { return };
        let mut cancel = false;
        let mut dismiss = false;
        let mut reveal = false;
        egui::Modal::new(egui::Id::new("progression_export")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(&job.title);
            ui.add_space(6.0);
            match job.progress() {
                Progress::Running(done, total) => {
                    ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).show_percentage());
                    let mut text = format!("{} {done} sur {total}", job.unit);
                    if let Some(rest) = job.remaining() {
                        let s = rest.as_secs();
                        text += &format!(" · reste environ {}:{:02}", s / 60, s % 60);
                    }
                    ui.label(text);
                    ui.add_space(6.0);
                    cancel = ui.button("Annuler").clicked();
                }
                Progress::Finishing => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Finalisation…");
                    });
                    ui.add_space(6.0);
                    cancel = ui.button("Annuler").clicked();
                }
                Progress::Done => {
                    match &job.outcome {
                        Some(Ok(msg)) => {
                            let s = job.started.elapsed().as_secs();
                            ui.colored_label(good(),format!("Terminé en {}:{:02}.", s / 60, s % 60));
                            ui.label(msg);
                        }
                        Some(Err(e)) => {
                            ui.colored_label(warn(), e);
                        }
                        None => {}
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if matches!(job.outcome, Some(Ok(_))) {
                            reveal = ui.button("Montrer dans l'explorateur").clicked();
                        }
                        dismiss = ui.button("Fermer").clicked();
                    });
                }
            }
        });
        if cancel {
            job.cancel();
        }
        if reveal {
            reveal_in_explorer(&job.target);
        }
        if dismiss {
            if let Some(job) = self.export_job.take() {
                self.status = match &job.outcome {
                    Some(Ok(msg)) => msg.clone(),
                    Some(Err(e)) => e.clone(),
                    None => String::new(),
                };
            }
        }
    }
}

fn save_dialog(base: &str, format: Format) -> Option<PathBuf> {
    let ext = format.extension();
    let (title, filter) = match format {
        Format::Mp4 => ("Exporter le film", "Vidéo MP4"),
        Format::ProRes => ("Exporter le film", "Vidéo MOV (ProRes)"),
        Format::Pdf => ("Exporter le storyboard", "Document PDF"),
        _ => ("Exporter le son", "Son WAV"),
    };
    let mut path = rfd::FileDialog::new()
        .set_title(title)
        .add_filter(filter, &[ext])
        .set_file_name(format!("{base}.{ext}"))
        .save_file()?;
    if path.extension().is_none() {
        path.set_extension(ext);
    }
    Some(path)
}

/// Ouvre l'explorateur Windows sur le fichier (sélectionné) ou le dossier.
pub(crate) fn reveal_in_explorer(path: &std::path::Path) {
    let mut c = std::process::Command::new("explorer.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if path.is_file() {
            c.raw_arg(format!("/select,\"{}\"", path.display()));
        } else {
            c.raw_arg(format!("\"{}\"", path.display()));
        }
    }
    let _ = c.spawn();
}
