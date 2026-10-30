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
use crate::exportcamera;
use crate::exportlayers;
use crate::exportstems::{self, Container, Depth};
use crate::exportstills::{self, StillFormat, StillsPer};
use crate::exportvideo::{self, VideoRate, VideoSize, VideoSpec};
use crate::pdfmise::{self, Paper};
use crate::storyboard::duration_label;

/// Échap ferme la fenêtre d'export sans rien lancer (sauf s'il ferme d'abord une liste déroulante ouverte). `Modal::should_close` le
/// fait déjà quand la fenêtre est « la plus haute » ; ceci le garantit aussi quand une autre couche d'`egui` lui dispute ce titre.
pub(crate) fn echap_ferme(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) && !ctx.memory(|m| m.any_popup_open())
}

/// Secondes pendant lesquelles la fenêtre d'un export réussi reste affichée (« Fermer » la ferme avant).
const FINI_AFFICHE: f64 = 8.0;

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
        self.settings.exports.apply(&mut s);
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
        let by_selection = matches!(s.format, Format::Gif | Format::Png | Format::Pdf | Format::Stills | Format::Layers | Format::Camera);
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
                // Le format : seulement ceux de la famille ouverte (le film en a quatre) ; un format que ce ffmpeg ne sait pas faire est
                // grisé, avec la raison.
                let family = family_of(s.format);
                if family.len() == 1 {
                    ui.label("Contenu");
                    ui.label(RichText::new(format_label(s.format)).strong());
                    ui.end_row();
                }
                if family.len() > 1 {
                    let encoders = match &ffmpeg {
                        Some((_, Ok(enc))) => Some(enc.clone()),
                        _ => None,
                    };
                    ui.label("Format");
                    ui.vertical(|ui| {
                        for &f in family {
                            let reason = encoders.as_ref().and_then(|e| exportvideo::missing_reason(f, e));
                            ui.add_enabled_ui(reason.is_none(), |ui| ui.radio_value(&mut s.format, f, format_label(f)));
                            if let Some(r) = reason {
                                ui.label(RichText::new(format!("Grisé : {r}.")).small().color(warn()));
                            }
                        }
                    });
                    ui.end_row();
                }
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
                        for size in VideoSize::CHOICES {
                            let [w, h] = size.dimensions([fw, fh]);
                            ui.radio_value(&mut s.size, size, format!("{} ({w}×{h})", size.label()));
                        }
                        let free = matches!(s.size, VideoSize::Width(_));
                        ui.horizontal(|ui| {
                            if ui.radio(free, "Largeur libre").clicked() && !free {
                                s.size = VideoSize::Width(1280);
                            }
                            if let VideoSize::Width(w) = &mut s.size {
                                ui.add(egui::DragValue::new(w).range(2..=16384).suffix(" px").speed(4.0));
                                let [ow, oh] = VideoSize::Width(*w).dimensions([fw, fh]);
                                ui.label(RichText::new(format!("{ow}×{oh}")).weak());
                            }
                        });
                    });
                    ui.end_row();
                    let enc_ici = match &ffmpeg {
                        Some((_, Ok(enc))) => Some(enc.clone()),
                        _ => None,
                    };
                    if let Some(p) = exportvideo::size_problem_for(s.format, s.size.dimensions([fw, fh]), &enc_ici.unwrap_or_default()) {
                        ui.label("");
                        ui.colored_label(warn(), p);
                        ui.end_row();
                    }
                    ui.label("Cadence");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("film_cadence")
                            .selected_text(if s.rate == VideoRate::Project { format!("{fps} images par seconde (le projet)") } else { s.rate.label().to_string() })
                            .show_ui(ui, |ui| {
                                for r in VideoRate::CHOICES {
                                    let text = if r == VideoRate::Project { format!("{fps} images par seconde (le projet)") } else { r.label().to_string() };
                                    ui.selectable_value(&mut s.rate, r, text);
                                }
                            })
                            .response
                            .on_hover_text("Le projet ne change pas : le film est converti à l'écriture, des images sont répétées ou sautées pour garder la durée.");
                    });
                    ui.end_row();
                    if s.format == Format::DnxHr {
                        ui.label("Fichier");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut s.mxf, false, "MOV");
                            ui.radio_value(&mut s.mxf, true, "MXF");
                        });
                        ui.end_row();
                    }
                    ui.label("Infos");
                    ui.vertical(|ui| {
                        bulles::bulle(
                            ui.checkbox(&mut s.burn.enabled, "Graver les informations dans l'image"),
                            "Graver les informations",
                            "Pose dans l'image du film le numéro du plan, le time code, le nom de la séquence et, si tu veux, l'action et le dialogue. Rien n'est ajouté au projet.",
                            "",
                        );
                        ui.add_enabled_ui(s.burn.enabled, |ui| {
                            ui.indent("film_infos", |ui| {
                                ui.checkbox(&mut s.burn.plan, "Numéro du plan (en haut à gauche)");
                                ui.checkbox(&mut s.burn.sequence, "Nom de la séquence (en haut à droite)");
                                ui.checkbox(&mut s.burn.timecode, "Time code (en bas à droite)");
                                ui.checkbox(&mut s.burn.texts, "Action et dialogue (en bas, sur plusieurs lignes)");
                            });
                        });
                    });
                    ui.end_row();
                    if s.format.has_alpha() {
                        ui.label("Fond");
                        ui.label(RichText::new("Transparent : le papier du projet n'est pas dans le film.").small());
                        ui.end_row();
                    }
                    ui.label("Son");
                    if sound_here {
                        ui.checkbox(&mut s.sound, "Avec le son du montage");
                    } else {
                        ui.label(RichText::new("Aucun son dans cette étendue.").weak());
                    }
                    ui.end_row();
                }
                // Le storyboard PDF à la carte : page, cases, textes, en-tête.
                if s.format == Format::Pdf {
                    ui.label("Page");
                    ui.vertical(|ui| {
                        ui.label(RichText::new(s.pdf.label()).small().weak());
                        ui.horizontal(|ui| {
                            for p in Paper::ALL {
                                ui.radio_value(&mut s.pdf.paper, p, p.label());
                            }
                            ui.separator();
                            ui.radio_value(&mut s.pdf.landscape, true, "Paysage");
                            ui.radio_value(&mut s.pdf.landscape, false, "Portrait");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Cases par page");
                            egui::ComboBox::from_id_salt("pdf_cases")
                                .selected_text(s.pdf.per_page_ok().to_string())
                                .show_ui(ui, |ui| {
                                    for n in pdfmise::PER_PAGE_CHOICES {
                                        let (c, r) = pdfmise::grid(n, s.pdf.landscape);
                                        ui.selectable_value(&mut s.pdf.per_page, n, format!("{n}  ({c} × {r})"));
                                    }
                                });
                        });
                    });
                    ui.end_row();
                    ui.label("Textes");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut s.pdf.number, "Numéro et titre du plan");
                        ui.checkbox(&mut s.pdf.duration, "Durée");
                        ui.checkbox(&mut s.pdf.action, "Action");
                        ui.checkbox(&mut s.pdf.dialogue, "Dialogue");
                        ui.checkbox(&mut s.pdf.notes, "Notes");
                    });
                    ui.end_row();
                    ui.label("Pages");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut s.pdf.header, "Titre du film et date en haut de chaque page");
                        ui.checkbox(&mut s.pdf.page_numbers, "Numéros de page");
                    });
                    ui.end_row();
                }
                // Les calques séparés : avec ou sans caméra ; ce qui partirait.
                if s.format == Format::Layers {
                    ui.label("Caméra");
                    ui.vertical(|ui| {
                        ui.radio_value(&mut s.layers.with_camera, true, "Caméra appliquée (la taille de l'image finale, comme dans le film)");
                        ui.radio_value(&mut s.layers.with_camera, false, "Sans caméra : la feuille entière de chaque calque");
                    });
                    ui.end_row();
                    ui.label("Calques");
                    let plans = export::panels_of(&frames);
                    let info = exportlayers::plans_info(&self.doc, &plans, s.annotations);
                    let files = exportlayers::file_count(&info, &frames);
                    ui.vertical(|ui| {
                        if files == 0 {
                            ui.colored_label(warn(), "Aucun calque visible à exporter dans cette étendue.");
                        } else {
                            let layers: usize = info.iter().map(|i| i.layers.len()).sum();
                            ui.label(format!("{} plan{}, {layers} calque{}, {files} images.", info.len(), if info.len() > 1 { "s" } else { "" }, if layers > 1 { "s" } else { "" }));
                        }
                        let effects = exportlayers::effect_layers(&self.doc, &plans);
                        if effects > 0 {
                            ui.label(RichText::new(format!("{effects} calque{} d'effet pas écrit{} : il agit sur ce qui est dessous, il n'a pas d'image à lui.", if effects > 1 { "s" } else { "" }, if effects > 1 { "s" } else { "" })).small().color(warn()));
                        }
                        ui.label(RichText::new("Un dossier par plan (SQ01_PL012A), une suite PNG par calque : 01_Fond_0001.png…").small().weak());
                    });
                    ui.end_row();
                }
                // Les clés de caméra : la caméra de chaque image, les CSV.
                if s.format == Format::Camera {
                    ui.label("Fichiers");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut s.camera.per_frame, "Avec la caméra de chaque image (déjà interpolée)");
                        ui.checkbox(&mut s.camera.csv, "Aussi en CSV (un tableur, un script)");
                        ui.label(RichText::new("Le JSON et son format sont décrits dans docs/EXPORT_MONTAGE.md.").small().weak());
                    });
                    ui.end_row();
                }
                // Le son par piste : conteneur, profondeur, mixage ; les pistes qui partiraient.
                if s.format == Format::Stems {
                    ui.label("Fichiers");
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut s.stems.format.container, Container::Wav, "WAV");
                            ui.radio_value(&mut s.stems.format.container, Container::Aiff, "AIFF");
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut s.stems.format.depth, Depth::Bits16, "16 bits");
                            ui.radio_value(&mut s.stems.format.depth, Depth::Bits24, "24 bits");
                            ui.label(RichText::new("48 kHz, stéréo").weak());
                        });
                        ui.checkbox(&mut s.stems.with_mix, "Avec le mixage de toutes les pistes");
                    });
                    ui.end_row();
                    ui.label("Pistes");
                    let files = exportstems::plan(&self.doc, first, frames.len(), &s.stems, &self.base_name());
                    if files.is_empty() {
                        ui.colored_label(warn(), "Aucune piste audible avec un extrait dans cette étendue (une piste muette n'est pas écrite).");
                    } else {
                        ui.vertical(|ui| {
                            for f in &files {
                                ui.label(RichText::new(format!("{}  ({})", f.file, f.name)).small());
                            }
                        });
                    }
                    ui.end_row();
                }
                // Une image par plan ou par case.
                if s.format == Format::Stills {
                    ui.label("Images");
                    ui.vertical(|ui| {
                        ui.radio_value(&mut s.stills.per, StillsPer::Plan, "Une par plan (sa première image)");
                        ui.radio_value(&mut s.stills.per, StillsPer::Case, "Une par case (le plan et chacune de ses marques d'image)");
                    });
                    ui.end_row();
                    ui.label("Fichier");
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut s.stills.format, StillFormat::Png, "PNG");
                            ui.radio_value(&mut s.stills.format, StillFormat::Jpeg, "JPEG");
                        });
                        if s.stills.format == StillFormat::Png {
                            ui.checkbox(&mut s.stills.transparent, "Sans le papier (fond transparent)");
                        } else {
                            ui.add(egui::Slider::new(&mut s.stills.quality, 50..=100).text("qualité"));
                        }
                    });
                    ui.end_row();
                    ui.label("Taille");
                    ui.vertical(|ui| {
                        for size in VideoSize::CHOICES {
                            let [w, h] = exportstills::still_size(size, [fw, fh]);
                            ui.radio_value(&mut s.stills.size, size, format!("{} ({w}×{h})", size.label()));
                        }
                        let free = matches!(s.stills.size, VideoSize::Width(_));
                        ui.horizontal(|ui| {
                            if ui.radio(free, "Largeur libre").clicked() && !free {
                                s.stills.size = VideoSize::Width(1280);
                            }
                            if let VideoSize::Width(w) = &mut s.stills.size {
                                ui.add(egui::DragValue::new(w).range(2..=16384).suffix(" px").speed(4.0));
                            }
                        });
                    });
                    ui.end_row();
                    ui.label("Noms");
                    ui.label(RichText::new("Comme l'export pour le montage : SQ01_PL012A (et _C02, _C03… pour les cases d'après).").small().weak());
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
                if s.format.has_images() {
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
                        ui.label(RichText::new(format!("ffmpeg : {}", path.display())).small());
                        if let Some(m) = exportvideo::missing_reason(s.format, enc) {
                            ui.colored_label(warn(), m);
                        }
                        // Pas de libx264 : le MP4 sort quand même, mais de moins bonne qualité.
                        if s.format == Format::Mp4 {
                            if let Some(w) = exportvideo::h264_warning(enc) {
                                ui.colored_label(warn(), w);
                            }
                        }
                    }
                    Some((path, Err(e))) => {
                        ui.colored_label(warn(), format!("{} : {e}", path.display()));
                    }
                    None => {
                        ui.colored_label(
                            warn(),
                            "ffmpeg introuvable. C'est lui qui fabrique la vidéo : installe-le (il vient par \
                             exemple avec un logiciel de dessin), ou indique où il est.",
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
            ui.add_space(4.0);
            ui.label(RichText::new("L'export utilise le projet tel qu'il est au départ : tu pourras continuer à travailler pendant qu'il tourne.").small().color(crate::theme::texte_doux()));
        });
        if choose {
            self.choose_ffmpeg();
        }
        // Le choix « Avec les annotations » est retenu par poste de travail, tout de suite (avec les réglages de l'application).
        if s.format.has_images() && self.settings.export_annotations != s.annotations {
            self.settings.export_annotations = s.annotations;
            self.settings_a_ecrire = true;
        }
        if go {
            self.export_last = s.clone();
            // Les choix de film, de PDF et d'images par plan sont retenus par poste de travail (réglages de l'application).
            let prefs = export::ExportPrefs::of(&s);
            if self.settings.exports != prefs {
                self.settings.exports = prefs;
                self.settings_a_ecrire = true;
            }
            if s.format == Format::Png {
                self.export_transparent = s.transparent;
            }
            self.start_export(&s, frames, first, sound_here && s.sound);
        } else if !close && !modal.should_close() && !echap_ferme(ctx) {
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
                let Some(path) = save_dialog(&base, Format::Wav, false) else { return };
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
                let Some(out) = save_dialog(&base, Format::Pdf, false) else { return };
                let (shots, boards) = export::storyboard_of_with(&self.doc, &export::panels_of(&frames), s.comments);
                self.start_job(ExportJob::pdf_layout(&out, &base, shots, boards, self.doc.frame, s.pdf).avec_annotations(s.annotations));
            }
            Format::Stills => {
                let Some(dir) = Self::pick_folder("Dossier où exporter les images par plan") else { return };
                let items = exportstills::items(&self.doc, &export::panels_of(&frames), s.stills.per);
                if items.is_empty() {
                    self.status = "Aucun plan dans cette étendue.".into();
                    return;
                }
                self.start_job(exportstills::job(&dir, &items, self.doc.frame, &s.stills, s.annotations));
            }
            Format::Gif => {
                let Some((ffmpeg, Ok(_))) = self.ffmpeg_info().cloned() else {
                    self.status = "Export impossible : ffmpeg introuvable ou inutilisable.".into();
                    return;
                };
                let Some(out) = save_dialog(&base, Format::Gif, false) else { return };
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
            Format::Mp4 | Format::ProRes | Format::ProRes4444 | Format::DnxHr => {
                let Some((ffmpeg, Ok(enc))) = self.ffmpeg_info().cloned() else {
                    self.status = "Export impossible : ffmpeg introuvable ou inutilisable.".into();
                    return;
                };
                let mxf = s.format == Format::DnxHr && s.mxf;
                if let Some(p) = exportvideo::size_problem_for(s.format, s.size.dimensions(self.doc.frame), &enc) {
                    self.status = format!("Export impossible : {p}.");
                    return;
                }
                let Some(out) = save_dialog(&base, s.format, mxf) else { return };
                let frame = self.doc.frame;
                let (format, size, rate_in, rate_out) = (s.format, s.size.dimensions(frame), fps.to_string(), s.rate.conversion(fps).map(str::to_string));
                let spec_for = move |enc: &export::Encoders, wav: Option<&std::path::Path>, target: &std::path::Path| {
                    exportvideo::video_args(&VideoSpec { format, enc, frame, out_size: size, rate_in: rate_in.clone(), rate_out: rate_out.clone(), audio: wav, target, mxf })
                };
                // Les arguments de ffmpeg sont vérifiés tout de suite (encodeur absent…) ; le son est mixé ensuite, sur un fil
                // à part, dans un fichier temporaire que ffmpeg lit à côté des images.
                if let Err(e) = spec_for(&enc, None, &out) {
                    self.status = format!("Export impossible : {e}");
                    return;
                }
                let span = sound.then(|| {
                    let (start, len) = export::sample_span(fps, first, frames.len());
                    SoundSpan { start, len }
                });
                let target = out.clone();
                let job = ExportJob::video_with_sound_alpha(&ffmpeg, move |wav| spec_for(&enc, wav, &target), &out, frames, span, s.format.has_alpha())
                    .avec_annotations(s.annotations)
                    .avec_infos(s.burn);
                self.start_job(if s.transitions { job.with_transitions(&self.doc) } else { job });
            }
            Format::Stems => {
                let files = exportstems::plan(&self.doc, first, frames.len(), &s.stems, &base);
                if files.is_empty() {
                    self.status = "Aucune piste audible avec un extrait dans cette étendue.".into();
                    return;
                }
                let Some(dir) = Self::pick_folder("Dossier où exporter le son par piste") else { return };
                let (start, len) = export::sample_span(fps, first, frames.len());
                let (format, d) = (s.stems.format, dir.clone());
                let message = format!("{} fichier{} son exporté{} : {}", files.len(), if files.len() > 1 { "s" } else { "" }, if files.len() > 1 { "s" } else { "" }, dir.display());
                self.start_job(ExportJob::task("Export du son par piste".into(), &dir, message, move |snap, cancel| exportstems::write_all(snap, &d, &files, start, len, format, cancel).map(|_| ())));
            }
            Format::Layers => {
                let info = exportlayers::plans_info(&self.doc, &export::panels_of(&frames), s.annotations);
                if exportlayers::file_count(&info, &frames) == 0 {
                    self.status = "Aucun calque visible à exporter dans cette étendue.".into();
                    return;
                }
                let Some(dir) = Self::pick_folder("Dossier où exporter les calques séparés") else { return };
                self.start_job(exportlayers::job(&dir, info, frames, &s.layers, s.annotations));
            }
            Format::Camera => {
                let Some(out) = save_dialog(&base, Format::Camera, false) else { return };
                let plans = export::panels_of(&frames);
                let (title, settings, target) = (self.base_name_or("SHOT"), s.camera, out.clone());
                let message = format!("Clés de caméra exportées ({} plan{}) : {}", plans.len(), if plans.len() > 1 { "s" } else { "" }, out.display());
                self.start_job(ExportJob::task("Export des clés de caméra".into(), &out, message, move |snap, _| {
                    let e = exportcamera::build(snap, &plans, &title, &settings);
                    exportcamera::write(&e, &target, &settings).map(|_| ())
                }));
            }
        }
    }

    /// Démarre un export : il copie le projet affiché (les premières tranches tout de suite), puis tourne tout seul.
    pub(crate) fn start_job(&mut self, mut job: ExportJob) {
        // (Traits texturés restés en rendu rapide : l'export les veut exacts, il les refait dans sa copie, sur un autre fil.)
        self.ctx.data_mut(|d| d.remove::<f64>(egui::Id::new("export_fini_le")));
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
                        // Une réussite s'efface d'elle-même au bout de quelques secondes (le pointeur sur la fenêtre la garde) ;
                        // une erreur reste jusqu'au clic. Le message et « Ouvrir le dossier » restent dans la barre d'état.
                        if matches!(job.outcome, Some(Ok(_))) {
                            let now = ui.ctx().input(|i| i.time);
                            let id = egui::Id::new("export_fini_le");
                            let depuis = if ui.rect_contains_pointer(ui.max_rect()) {
                                ui.ctx().data_mut(|d| d.insert_temp(id, now));
                                now
                            } else {
                                ui.ctx().data_mut(|d| *d.get_temp_mut_or_insert_with(id, || now))
                            };
                            if now - depuis >= FINI_AFFICHE {
                                dismiss = true;
                            } else {
                                ui.ctx().request_repaint_after(Duration::from_millis(250));
                            }
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if matches!(job.outcome, Some(Ok(_))) {
                                reveal = bulles::bulle(ui.button(format!("{} Montrer dans l'explorateur", ph::FOLDER_OPEN)), "Montrer dans l'explorateur", "Ouvre le dossier du fichier exporté.", "").clicked();
                            }
                            dismiss |= bulles::bulle(ui.button(format!("{} Fermer", ph::CHECK)), "Fermer", "Ferme cette fenêtre (l'export est fini).", "").clicked();
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
            ctx.data_mut(|d| d.remove::<f64>(egui::Id::new("export_fini_le")));
            if let Some(job) = self.export_job.take() {
                match &job.outcome {
                    Some(Ok(msg)) => self.status_avec_action(msg.clone(), crate::messages::ActionMessage::MontrerDossier(job.target.clone())),
                    Some(Err(e)) => self.status = e.clone(),
                    None => self.status = String::new(),
                }
            }
            self.export_projet = None;
        }
    }
}

/// Le bouton qui arrête l'export (icône, nom, bulle au bout d'une seconde).
fn bouton_arreter(ui: &mut egui::Ui) -> bool {
    bulles::bulle(ui.button(format!("{} Arrêter", ph::X)), "Arrêter l'export", "Arrête l'export et efface ce qu'il a commencé d'écrire (un export fini plus tôt n'est pas touché).", "").clicked()
}

/// Les formats qu'une fenêtre d'export propose ensemble (celui qu'on a ouvert et ses voisins).
fn family_of(format: Format) -> &'static [Format] {
    match format {
        Format::Mp4 | Format::ProRes | Format::ProRes4444 | Format::DnxHr => &[Format::Mp4, Format::ProRes, Format::ProRes4444, Format::DnxHr],
        Format::Gif => &[Format::Gif],
        Format::Png => &[Format::Png],
        Format::Wav => &[Format::Wav],
        Format::Pdf => &[Format::Pdf],
        Format::Stills => &[Format::Stills],
        Format::Stems => &[Format::Stems],
        Format::Layers => &[Format::Layers],
        Format::Camera => &[Format::Camera],
    }
}

/// Le nom d'un format dans la liste de la fenêtre, avec à quoi il sert.
fn format_label(format: Format) -> &'static str {
    match format {
        Format::Mp4 => "MP4 (H.264) : pour regarder, envoyer",
        Format::ProRes => "MOV ProRes 422 HQ : pour le montage",
        Format::ProRes4444 => "MOV ProRes 4444 : fond transparent, pour superposer le film",
        Format::DnxHr => "DNxHR HQ (MOV ou MXF) : pour le montage",
        Format::Gif => "GIF animé : pour montrer, partager (sans le son)",
        Format::Png => "Images PNG (une par image)",
        Format::Wav => "Son mixé (WAV)",
        Format::Pdf => "Storyboard PDF",
        Format::Stills => "Images par plan",
        Format::Stems => "Son par piste",
        Format::Layers => "Calques séparés",
        Format::Camera => "Clés de caméra",
    }
}

fn save_dialog(base: &str, format: Format, mxf: bool) -> Option<PathBuf> {
    let ext = if mxf { "mxf" } else { format.extension() };
    let (title, filter) = match format {
        Format::Mp4 => ("Exporter le film", "Vidéo MP4"),
        Format::ProRes => ("Exporter le film", "Vidéo MOV (ProRes)"),
        Format::ProRes4444 => ("Exporter le film", "Vidéo MOV (ProRes 4444)"),
        Format::DnxHr => ("Exporter le film", if mxf { "Vidéo MXF (DNxHR)" } else { "Vidéo MOV (DNxHR)" }),
        Format::Pdf => ("Exporter le storyboard", "Document PDF"),
        Format::Camera => ("Exporter les clés de caméra", "Données JSON"),
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
