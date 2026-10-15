//! Calque vidéo dans l'interface (13k) : importer une vidéo ou une séquence d'images (sur un
//! autre fil, la barre d'état suit), avec le son de la vidéo sur une piste son calée sur le
//! calque ; régler le début du calque ; le transformer en calque d'animation. Voir `video.rs`.

use eframe::egui;

use crate::app::App;
use crate::document::{Layer, LayerKind};
use crate::video::{self, Source};

/// Extensions de vidéo proposées à l'import.
const VIDEO_EXTS: [&str; 9] = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "mpg", "mpeg", "wmv"];
/// Extensions d'images d'une séquence.
const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "bmp", "tga"];

/// Un import vidéo en cours, et où poser le calque : plan (id) et image.
pub(crate) struct VideoImport {
    job: video::ImportJob,
    panel: u64,
    at: usize,
}

impl App {
    /// Fichier > Importer une vidéo.
    pub(crate) fn import_video(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Importer une vidéo").add_filter("Vidéo", &VIDEO_EXTS).pick_file() else { return };
        self.start_video_import(Source::Video(path));
    }

    /// Fichier > Importer une séquence d'images : une image de la séquence suffit.
    pub(crate) fn import_sequence(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importer une séquence d'images (choisir une de ses images)")
            .add_filter("Images", &IMAGE_EXTS)
            .pick_file()
        else {
            return;
        };
        let files = video::sequence_files(&path);
        self.start_video_import(Source::Sequence(files));
    }

    /// Un fichier lâché sur la fenêtre : une vidéo devient un calque vidéo. Faux si ce n'est pas
    /// une vidéo.
    pub(crate) fn import_dropped_video(&mut self, path: &std::path::Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !VIDEO_EXTS.contains(&ext.as_str()) {
            return false;
        }
        self.start_video_import(Source::Video(path.to_path_buf()));
        true
    }

    fn start_video_import(&mut self, source: Source) {
        if self.video_import.is_some() {
            self.status = "Un import vidéo est déjà en cours.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.playing = false;
        let ffmpeg = crate::export::find_ffmpeg(self.settings.ffmpeg.as_deref());
        let job = video::start_import(ffmpeg, source, self.doc.frame, self.doc.fps);
        self.status = format!("Import de « {} »…", job.name);
        self.video_import = Some(VideoImport { job, panel: self.panel().id, at: self.cur_frame });
    }

    /// À chaque image d'affichage : l'import vidéo avance, ou se termine (le calque est créé).
    pub(crate) fn tick_video_import(&mut self, ctx: &egui::Context) {
        let Some(imp) = self.video_import.as_mut() else { return };
        let Some(result) = imp.job.finished() else {
            let n = imp.job.progress.load(std::sync::atomic::Ordering::Relaxed);
            self.status = format!("Import de « {} » : {n} images…", imp.job.name);
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        };
        let VideoImport { panel, at, .. } = self.video_import.take().expect("import en cours");
        match result {
            Err(e) => self.status = format!("Import impossible : {e}"),
            Ok(imported) => self.place_video(imported, panel, at),
        }
    }

    /// Pose la vidéo importée : un calque vidéo au-dessus du calque courant du plan `panel`, qui
    /// commence à l'image `at` ; le plan s'allonge s'il le faut (sauf storyboard verrouillé) ;
    /// le son de la vidéo va sur une piste son, calé sur le calque.
    fn place_video(&mut self, imported: video::Imported, panel: u64, at: usize) {
        let mut clip = imported.clip;
        video::register_dir(clip.id, video::temp_dir(clip.id));
        if let Some(i) = self.doc.panels.iter().position(|p| p.id == panel) {
            self.set_panel(i);
        }
        clip.start = at;
        let (name, fps) = (clip.name.clone(), self.doc.fps);
        let end = clip.end(fps);
        let snap = self.snapshot();
        let mut layer = Layer::new(name.clone(), LayerKind::Bitmap);
        layer.parent = self.layer().parent;
        layer.video = Some(clip.clone());
        let li = (self.cur_layer + 1).min(self.layers().len());
        self.layers_mut().insert(li, layer);
        self.cur_layer = li;
        if end > self.frame_count() && !self.doc.storyboard_locked {
            self.panel_mut().set_length(end);
        }
        self.commit_structure(snap);
        let mut msg = format!("Calque vidéo « {name} » : {} images à {:.3} i/s, de l'image {} à {}.", clip.frames, clip.fps, self.frame_label(at), self.frame_label(end - 1));
        if let Some(wav) = imported.sound {
            let start = self.doc.panel_start(self.cur_panel) + at;
            self.import_sound_path(&wav, None, Some(start));
            msg.push_str(" Son de la vidéo sur une piste son.");
        }
        self.status = msg;
    }

    /// Le calque vidéo courant devient un calque d'animation : chaque image de la vidéo devient
    /// un dessin (lourd en mémoire : environ 8 Mo par image HD).
    pub(crate) fn video_to_animation(&mut self) {
        let li = self.cur_layer;
        let Some(clip) = self.layer().video.clone() else {
            self.status = "Ce calque n'est pas un calque vidéo.".into();
            return;
        };
        if self.content_refuses() {
            return;
        }
        let Some(dir) = video::dir_of(clip.id) else {
            self.status = "Les images de cette vidéo sont introuvables.".into();
            return;
        };
        self.end_stroke();
        let snap = self.snapshot();
        let fps = self.doc.fps;
        let mut made: std::collections::HashMap<usize, crate::document::CelId> = std::collections::HashMap::new();
        let mut frames = Vec::with_capacity(clip.end(fps));
        for f in 0..clip.end(fps) {
            let cel = clip.index_at(f, fps).and_then(|k| {
                if let Some(&c) = made.get(&k) {
                    return Some(c);
                }
                let raster = video::read_frame(&self.doc, &video::frame_file(&dir, k, &clip.ext))?;
                let c = self.doc.new_cel(LayerKind::Bitmap);
                if let Some(cel) = self.doc.cels.get_mut(&c) {
                    cel.raster = raster;
                    cel.version += 1;
                }
                made.insert(k, c);
                Some(c)
            });
            frames.push(cel);
        }
        let layer = &mut self.layers_mut()[li];
        layer.video = None;
        layer.kind = LayerKind::Bitmap;
        layer.frames = frames;
        layer.trim();
        self.renderer.invalidate_all();
        self.commit_structure(snap);
        self.status = format!("Calque d'animation : {} dessins tirés de la vidéo.", made.len());
    }

    /// Le menu du calque vidéo `li` (ligne de la timeline).
    pub(crate) fn video_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(clip) = self.layers().get(li).and_then(|l| l.video.clone()) else { return };
        let fps = self.doc.fps;
        ui.label(egui::RichText::new(format!("« {} » : {} images à {:.3} i/s", clip.name, clip.frames, clip.fps)).weak());
        ui.horizontal(|ui| {
            ui.label("Commence à l'image");
            let mut start = clip.start;
            let first = self.doc.start_frame;
            let resp = ui.add(egui::DragValue::new(&mut start).range(0..=100_000).custom_formatter(move |v, _| (v as usize + first).to_string()));
            if self.edited(&resp) {
                if let Some(v) = self.layers_mut()[li].video.as_mut() {
                    v.start = start;
                }
            }
        });
        ui.label(egui::RichText::new(format!("Jusqu'à l'image {}.", self.frame_label(clip.end(fps).saturating_sub(1)))).weak());
        ui.separator();
        self.action_item(ui, "Transformer en calque d'animation", "calque.video_en_animation");
    }
}
