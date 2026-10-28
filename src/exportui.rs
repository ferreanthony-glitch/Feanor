//! Fenêtres d'export (3b) : choix (contenu, étendue, taille, son), puis progression.
//! Le choix est modal ; la progression ne l'est pas : l'export travaille sur une copie du projet, sur d'autres fils
//! (`exportrender.rs`), et le projet reste libre pendant ce temps.

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::{self, vec2, Align2, Color32, RichText};

use egui_phosphor::regular as ph;

use crate::app::App;
use crate::audio;
use crate::bulles;
use crate::export::{self, ExportJob, ExportSettings, Format, Progress, Range, SoundSpan};
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
        if !self.export_libre() {
            return;
        }
        let mut s = self.export_last.clone();
        s.format = format;
        s.transparent = self.export_transparent;
        s.annotations = self.settings.export_annotations;
        self.export_dialog = Some(s);
    }

    /// On peut lancer un export : aucun ne tourne (un export fini, dont la fenêtre est restée, est refermé). Sinon la barre d'état le dit.
    pub(crate) fn export_libre(&mut self) -> bool {
        match &self.export_job {
            Some(j) if j.outcome.is_none() => {
                self.status = "Un export est déjà en cours : attends sa fin, ou arrête-le.".into();
                false
            }
            Some(j) => {
                self.status = match &j.outcome {
                    Some(Ok(msg)) => msg.clone(),
                    Some(Err(e)) => e.clone(),
                    None => String::new(),
                };
                self.export_job = None;
                self.export_projet = None;
                true
            }
            None => true,
        }
    }

    /// ffmpeg et ses encodeurs, cherchés une fois (et après un choix à la main).
    pub(crate) fn ffmpeg_info(&mut self) -> Option<&(PathBuf, Result<export::Encoders, String>)> {
        if self.ffmpeg.is_none() {
            let found = export::find_ffmpeg(self.settings.ffmpeg.as_deref()).map(|p| {
                let enc = export::probe(&p);
                (p, enc)
            });
            self.ffmpeg = Some(found);
        }
        self.ffmpeg.as_ref().and_then(|f| f.as_ref())
    }

    pub(crate) fn choose_ffmpeg(&mut self) {
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
        let chosen = self.board_selection();
        // Les plans choisis ne se proposent que là où le son ne compte pas (des plans qui ne se suivent pas n'ont
        // pas un son d'un seul tenant) : le GIF, les images, le storyboard PDF.
        let by_selection = matches!(s.format, Format::Gif | Format::Png | Format::Pdf);
        if s.range == Range::Selection && !by_selection {
            s.range = Range::Panel;
        }
        let count = |range| export::frames_for_chosen(&self.doc, range, self.cur_panel, &chosen).len();
        let ranges = [
            (Range::Montage, "Tout le montage", count(Range::Montage)),
            (Range::Scene, "Séquence courante", count(Range::Scene)),
            (Range::Panel, "Plan courant", count(Range::Panel)),
            (Range::Selection, if chosen.len() > 1 { "Plans choisis" } else { "Plans choisis (aucun autre que le plan courant)" }, count(Range::Selection)),
        ];
        // Le son de l'étendue choisie (pour griser « avec le son » s'il n'y en a pas).
        let frames = export::frames_for_chosen(&self.doc, s.range, self.cur_panel, &chosen);
        let first = frames.first().map_or(0, |&(p, f)| self.doc.panel_start(p) + f);
        let (start, len) = export::sample_span(fps, first, frames.len());
        let sound_here = export::has_sound(&audio::mix_clips(&self.doc), start, len);
        let ffmpeg = if s.format.uses_ffmpeg() { self.ffmpeg_info().cloned() } else { None };
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
                    ui.radio_value(&mut s.format, Format::Gif, "GIF animé : pour montrer, partager (sans le son)");
                    ui.radio_value(&mut s.format, Format::Png, "Images PNG (une par image)");
                    ui.radio_value(&mut s.format, Format::Wav, "Son seul (WAV)");
                    ui.radio_value(&mut s.format, Format::Pdf, "Storyboard PDF (A4 paysage, 6 images par page)");
                });
                ui.end_row();
                ui.label("Étendue");
                ui.vertical(|ui| {
                    for (range, label, n) in ranges {
                        if range == Range::Selection && !by_selection {
                            continue;
                        }
                        ui.radio_value(&mut s.range, range, format!("{label} ({})", duration_label(n, fps)));
                    }
                });
                ui.end_row();
                // GIF : taille, cadence (au plus celle du projet), boucle, tramage.
                if s.format == Format::Gif {
                    ui.label("Taille");
                    ui.vertical(|ui| {
                        for size in export::GifSize::CHOICES {
                            let [w, h] = size.dimensions([fw, fh]);
                            ui.radio_value(&mut s.gif.size, size, format!("{} ({w}×{h})", size.label()));
                        }
                    });
                    ui.end_row();
                    ui.label("Cadence");
                    ui.horizontal(|ui| {
                        s.gif.fps = export::gif_fps(s.gif.fps, fps);
                        egui::ComboBox::from_id_salt("gif_cadence")
                            .selected_text(format!("{} images par seconde", s.gif.fps))
                            .show_ui(ui, |ui| {
                                for f in export::gif_fps_choices(fps) {
                                    ui.selectable_value(&mut s.gif.fps, f, format!("{f} images par seconde"));
                                }
                            })
                            .response
                            .on_hover_text("Jamais plus que la cadence du projet. Moins d'images : un fichier plus léger.");
                        let n = export::gif_sample(&frames, fps, s.gif.fps).len();
                        ui.label(RichText::new(format!("{n} images")).weak());
                    });
                    ui.end_row();
                    ui.label("Lecture");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut s.gif.looped, "Tourne en boucle");
                        ui.checkbox(&mut s.gif.dither, "Tramage (dégradés plus doux, fichier plus lourd)");
                    });
                    ui.end_row();
                }
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
                // Les commentaires de la timeline, écrits sous l'image où ils tombent (`export::storyboard_of_with`).
                if s.format == Format::Pdf {
                    ui.label("Commentaires");
                    ui.add_enabled_ui(!self.doc.comments.is_empty(), |ui| {
                        ui.checkbox(&mut s.comments, "Avec les commentaires (les non résolus)");
                    });
                    ui.end_row();
                }
                if s.format == Format::Png {
                    ui.label("Fond");
                    ui.checkbox(&mut s.transparent, "Sans le papier (fond transparent)");
                    ui.end_row();
                }
                // Les calques d'annotation des plans (`calqueannot.rs`) : jamais dans le film d'office, posés par-dessus tout si on les veut.
                if s.format != Format::Wav {
                    ui.label("Annotations");
                    crate::bulles::bulle(
                        ui.checkbox(&mut s.annotations, "Avec les annotations"),
                        "Avec les annotations",
                        "Pose les calques d'annotation des plans par-dessus l'image. Décoché d'office : des notes de réalisateur ne font pas partie du film. Retenu d'une séance à l'autre.",
                        "",
                    );
                    ui.end_row();
                }
                // Les fondus, volets et noirs du montage (`transitions.rs`) : calculés comme le reste
                // de l'export, image par image.
                if (s.format.is_video() || matches!(s.format, Format::Png | Format::Gif)) && has_transitions {
                    ui.label("Transitions");
                    ui.checkbox(&mut s.transitions, "Avec les transitions du montage");
                    ui.end_row();
                }
            });
            if s.format.uses_ffmpeg() {
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
                go = crate::theme::bouton_principal(ui, true, "Exporter…").clicked();
                close = ui.button("Annuler").clicked();
            });
            ui.add_space(4.0);
            ui.label(RichText::new("L'export utilise le projet tel qu'il est au départ : tu pourras continuer à travailler pendant qu'il tourne.").small().color(crate::theme::texte_doux()));
        });
        if choose {
            self.choose_ffmpeg();
        }
        // Le choix « Avec les annotations » est retenu par poste de travail, tout de suite (avec les réglages de l'application).
        if s.format != Format::Wav && self.settings.export_annotations != s.annotations {
            self.settings.export_annotations = s.annotations;
            self.settings_a_ecrire = true;
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

    /// Lance l'export choisi : demande où écrire, puis démarre. Aucune image ne se calcule ici : l'export copie le
    /// projet (le cliché) et travaille sur d'autres fils ; le projet reste libre.
    fn start_export(&mut self, s: &ExportSettings, frames: Vec<(usize, usize)>, first: usize, sound: bool) {
        let base = self.base_name();
        let fps = self.doc.fps;
        match s.format {
            Format::Wav => {
                let Some(path) = save_dialog(&base, Format::Wav) else { return };
                let (start, len) = export::sample_span(fps, first, frames.len());
                // (Le son exporté attend l'égaliseur et l'écho : `mix_clips_blocking`, sur un fil à part.)
                self.start_job(ExportJob::sound(&path, SoundSpan { start, len }));
            }
            Format::Png => {
                let Some(dir) = Self::pick_folder("Dossier où exporter les images") else { return };
                let job = ExportJob::images(&dir, &base, frames, self.doc.frame, !s.transparent).avec_annotations(s.annotations);
                self.start_job(if s.transitions { job.with_transitions(&self.doc) } else { job });
            }
            Format::Pdf => {
                let Some(out) = save_dialog(&base, Format::Pdf) else { return };
                let (shots, boards) = export::storyboard_of_with(&self.doc, &export::panels_of(&frames), s.comments);
                self.start_job(ExportJob::pdf(&out, &base, shots, boards, self.doc.frame).avec_annotations(s.annotations));
            }
            Format::Gif => {
                let Some((ffmpeg, Ok(_))) = self.ffmpeg_info().cloned() else {
                    self.status = "Export impossible : ffmpeg introuvable ou inutilisable.".into();
                    return;
                };
                let Some(out) = save_dialog(&base, Format::Gif) else { return };
                // Seules les images que le GIF garde sont calculées (à 12 images par seconde sur un film à 24, une sur deux).
                let rate = export::gif_fps(s.gif.fps, fps);
                let kept = export::gif_sample(&frames, fps, rate);
                let args = export::gif_args(self.doc.frame, rate, &s.gif, &out);
                match ExportJob::gif(&ffmpeg, args, &out, kept) {
                    Ok(job) => {
                        let job = job.avec_annotations(s.annotations);
                        self.start_job(if s.transitions { job.with_transitions(&self.doc) } else { job })
                    }
                    Err(e) => self.status = format!("Export impossible : {e}"),
                }
            }
            Format::Mp4 | Format::ProRes => {
                let Some((ffmpeg, Ok(enc))) = self.ffmpeg_info().cloned() else {
                    self.status = "Export impossible : ffmpeg introuvable ou inutilisable.".into();
                    return;
                };
                let Some(out) = save_dialog(&base, s.format) else { return };
                // Les arguments de ffmpeg sont vérifiés tout de suite (encodeur absent…) ; le son est mixé ensuite, sur un fil
                // à part, dans un fichier temporaire que ffmpeg lit à côté des images.
                if let Err(e) = export::video_args(s.format, &enc, self.doc.frame, fps, s.half, None, &out) {
                    self.status = format!("Export impossible : {e}");
                    return;
                }
                let span = sound.then(|| {
                    let (start, len) = export::sample_span(fps, first, frames.len());
                    SoundSpan { start, len }
                });
                let (format, frame, half, target) = (s.format, self.doc.frame, s.half, out.clone());
                let job = ExportJob::video_with_sound(&ffmpeg, move |wav| export::video_args(format, &enc, frame, fps, half, wav, &target), &out, frames, span).avec_annotations(s.annotations);
                self.start_job(if s.transitions { job.with_transitions(&self.doc) } else { job });
            }
        }
    }

    /// Démarre un export : il copie le projet affiché (les premières tranches tout de suite), puis tourne tout seul.
    pub(crate) fn start_job(&mut self, mut job: ExportJob) {
        // (Traits texturés restés en rendu rapide : l'export les veut exacts, il les refait dans sa copie, sur un autre fil.)
        job.set_exact(self.rejeu_idle.clone());
        job.step(&self.doc, Duration::from_millis(8));
        self.export_projet = Some((self.onglets.get(self.actif).map_or(0, |t| t.id), self.base_name_or("Sans titre")));
        self.export_job = Some(job);
    }

    /// Le projet de l'export en cours n'est pas celui qu'on regarde.
    fn export_d_un_autre_projet(&self) -> Option<&str> {
        let (id, nom) = self.export_projet.as_ref()?;
        (self.onglets.get(self.actif).map(|t| t.id) != Some(*id)).then_some(nom.as_str())
    }

    /// La fenêtre de l'export en cours : pas modale, le projet reste libre (l'export travaille sur sa copie).
    fn export_progress_window(&mut self, ctx: &egui::Context) {
        let autre = self.export_d_un_autre_projet().map(str::to_string);
        let Some(job) = &mut self.export_job else { return };
        let mut cancel = false;
        let mut dismiss = false;
        let mut reveal = false;
        egui::Window::new("Export en cours")
            .id(egui::Id::new("progression_export"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-16.0, -40.0))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.horizontal(|ui| {
                    ui.strong(&job.title);
                    if let Some(nom) = &autre {
                        ui.label(RichText::new(format!("· « {nom} »")).weak());
                    }
                });
                ui.add_space(4.0);
                match job.progress() {
                    Progress::Running(done, total) => {
                        let phase = job.phase();
                        let progress = if done == 0 && !phase.starts_with("Calcul") { 0.0 } else { done as f32 / total.max(1) as f32 };
                        ui.add(egui::ProgressBar::new(progress).show_percentage());
                        let mut text = if done == 0 && !phase.is_empty() && !phase.starts_with("Calcul") { phase.to_string() } else { format!("{} {done} sur {total}", job.unit) };
                        if let Some(rest) = job.remaining() {
                            let s = rest.as_secs();
                            text += &format!(" · reste environ {}:{:02}", s / 60, s % 60);
                        }
                        ui.label(text);
                        ui.label(RichText::new("L'export utilise le projet tel qu'il était au départ : tu peux continuer à travailler.").small().color(crate::theme::texte_doux()));
                        ui.add_space(4.0);
                        cancel = bouton_arreter(ui);
                    }
                    Progress::Finishing => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Finalisation…");
                        });
                        ui.add_space(4.0);
                        cancel = bouton_arreter(ui);
                    }
                    Progress::Done => {
                        match &job.outcome {
                            Some(Ok(msg)) => {
                                let s = job.started.elapsed().as_secs();
                                ui.colored_label(good(), format!("Terminé en {}:{:02}.", s / 60, s % 60));
                                ui.label(msg);
                            }
                            Some(Err(e)) => {
                                ui.colored_label(warn(), e);
                            }
                            None => {}
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if matches!(job.outcome, Some(Ok(_))) {
                                reveal = bulles::bulle(ui.button(format!("{} Montrer dans l'explorateur", ph::FOLDER_OPEN)), "Montrer dans l'explorateur", "Ouvre le dossier du fichier exporté.", "").clicked();
                            }
                            dismiss = bulles::bulle(ui.button(format!("{} Fermer", ph::CHECK)), "Fermer", "Ferme cette fenêtre (l'export est fini).", "").clicked();
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
            self.export_projet = None;
        }
    }
}

/// Le bouton qui arrête l'export (icône, nom, bulle au bout d'une seconde).
fn bouton_arreter(ui: &mut egui::Ui) -> bool {
    bulles::bulle(ui.button(format!("{} Arrêter", ph::X)), "Arrêter l'export", "Arrête l'export et efface ce qu'il a commencé d'écrire (un export fini plus tôt n'est pas touché).", "").clicked()
}

fn save_dialog(base: &str, format: Format) -> Option<PathBuf> {
    let ext = format.extension();
    let (title, filter) = match format {
        Format::Mp4 => ("Exporter le film", "Vidéo MP4"),
        Format::ProRes => ("Exporter le film", "Vidéo MOV (ProRes)"),
        Format::Pdf => ("Exporter le storyboard", "Document PDF"),
        Format::Gif => ("Exporter un GIF animé", "Image GIF animée"),
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
