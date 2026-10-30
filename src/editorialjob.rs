//! La tâche de l'export pour le montage (`editorial.rs` décrit, ceci écrit).
//!
//! Trois travaux, qui ne bloquent jamais l'interface :
//!
//! 1. **Un fichier vidéo par plan**, l'un après l'autre : le projet est copié une fois au départ (le cliché,
//!    `exportrender.rs`), puis les images de chaque plan sont calculées sur d'autres fils, comme pour le film
//!    (`export::ExportJob`), et envoyées à un ffmpeg par plan (ProRes ou H.264, sans transitions : elles sont
//!    dans les fichiers de montage). Les poignées d'un fondu enchaîné sont des images tenues, que l'export
//!    d'images met en cache (elles ne sont pas recalculées). Le projet reste libre pendant ce temps.
//! 2. **Un fichier son par extrait**, sur un **fil à part** qui démarre en même temps (effets et
//!    égaliseur compris), avec son compteur d'avancement.
//! 3. À la fin, les **fichiers de montage** (EDL, XML, FCPXML, OTIO, `LISEZ-MOI.txt`) : quelques kilo-octets.
//!
//! Annuler (ou fermer SHOT) arrête ffmpeg et le fil du son, et **efface les fichiers déjà écrits par
//! cet export** (le dossier lui-même s'il l'a créé et qu'il est vide).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::document::Document;
use crate::editorial::{self, Options, Plan, PlanClip};
use crate::export::{self, Encoders, ExportJob, Progress};
use crate::exportrender::SnapshotBuilder;

/// Le compte rendu d'un export réussi.
#[derive(Clone, Debug)]
pub struct Report {
    pub dir: PathBuf,
    /// Les fichiers de montage écrits (EDL, XML, OTIO, LISEZ-MOI).
    pub files: Vec<PathBuf>,
    pub plans: usize,
    pub sounds: usize,
}

/// Où en est l'export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Les fichiers vidéo, plan par plan.
    Video,
    /// Les vidéos sont faites ; le fil du son finit.
    Sound,
    Done,
}

/// Le dossier d'un export : `<parent>/<Titre>_montage`.
pub fn export_dir_for(parent: &Path, title: &str) -> PathBuf {
    parent.join(format!("{}_montage", editorial::file_safe(title)))
}

/// Les images (plan, image) du fichier vidéo d'un plan : les poignées (première image tenue), le plan,
/// les poignées de fin (dernière image tenue).
pub fn clip_frames(clip: &PlanClip) -> Vec<(usize, usize)> {
    let p = clip.panel;
    let last = clip.len.saturating_sub(1);
    let mut v = Vec::with_capacity(clip.file_frames());
    v.extend((0..clip.head).map(|_| (p, 0)));
    v.extend((0..clip.len).map(|f| (p, f)));
    v.extend((0..clip.tail).map(|_| (p, last)));
    v
}

/// Le fil qui écrit les fichiers son.
struct SoundWorker {
    handle: Option<JoinHandle<Result<(), String>>>,
    done: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    /// Les fichiers écrits jusqu'ici (à effacer si l'on annule).
    written: Arc<Mutex<Vec<PathBuf>>>,
}

impl SoundWorker {
    fn start(jobs: Vec<(editorial::AudioSource, PathBuf)>) -> SoundWorker {
        let done = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let written = Arc::new(Mutex::new(Vec::new()));
        let (d, c, w) = (done.clone(), cancel.clone(), written.clone());
        let handle = std::thread::spawn(move || -> Result<(), String> {
            for (src, path) in jobs {
                if c.load(Ordering::Relaxed) {
                    return Err("annulé".into());
                }
                let bytes = editorial::wav_bytes(&src);
                if c.load(Ordering::Relaxed) {
                    return Err("annulé".into());
                }
                fs::write(&path, bytes).map_err(|e| format!("{} : {e}", path.display()))?;
                w.lock().unwrap_or_else(|e| e.into_inner()).push(path);
                d.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        });
        SoundWorker { handle: Some(handle), done, cancel, written }
    }

    fn finished(&self) -> bool {
        self.handle.as_ref().is_none_or(|h| h.is_finished())
    }

    fn join(&mut self) -> Result<(), String> {
        match self.handle.take() {
            Some(h) => h.join().unwrap_or_else(|_| Err("le fil du son s'est arrêté".into())),
            None => Ok(()),
        }
    }

    fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.join();
    }
}

/// Un export pour le montage en cours.
pub struct EditorialJob {
    pub plan: Plan,
    opts: Options,
    ffmpeg: PathBuf,
    enc: Encoders,
    pub dir: PathBuf,
    made_dir: bool,
    pub stage: Stage,
    current: Option<ExportJob>,
    /// Prochain plan dont le fichier est à faire, et images déjà écrites pour les précédents.
    next: usize,
    frames_done: usize,
    worker: Option<SoundWorker>,
    /// Fichiers vidéo écrits (à effacer si l'on annule).
    written: Vec<PathBuf>,
    /// Le cliché du projet, pris une fois pour tous les plans.
    builder: Option<SnapshotBuilder>,
    snapshot: Option<Arc<Document>>,
    /// Les dessins restés en rendu rapide, à refaire exacts dans le cliché.
    exact: Vec<crate::document::CelId>,
    pub started: Instant,
    pub outcome: Option<Result<Report, String>>,
}

impl EditorialJob {
    /// Prépare le dossier et lance le fil du son. Les images démarrent au premier `step`.
    pub fn start(plan: Plan, opts: Options, ffmpeg: PathBuf, enc: Encoders, parent: &Path) -> Result<EditorialJob, String> {
        if plan.clips.is_empty() {
            return Err("Aucun plan à exporter.".into());
        }
        match plan.media {
            editorial::Media::ProRes if !enc.prores => return Err("Ce ffmpeg ne sait pas encoder en ProRes : choisis H.264.".into()),
            editorial::Media::H264 if enc.h264.is_none() => return Err("Ce ffmpeg ne sait pas encoder en H.264.".into()),
            _ => {}
        }
        let dir = export_dir_for(parent, &plan.title);
        let made_dir = !dir.exists();
        fs::create_dir_all(dir.join("media")).map_err(|e| format!("{} : {e}", dir.display()))?;
        let worker = if plan.audio.is_empty() {
            None
        } else {
            fs::create_dir_all(dir.join("audio")).map_err(|e| format!("{} : {e}", dir.display()))?;
            let jobs = plan.audio_sources.iter().cloned().zip(plan.audio.iter()).map(|(src, a)| (src, dir.join(&a.file))).collect();
            Some(SoundWorker::start(jobs))
        };
        Ok(EditorialJob { plan, opts, ffmpeg, enc, dir, made_dir, stage: Stage::Video, current: None, next: 0, frames_done: 0, worker, written: Vec::new(), builder: None, snapshot: None, exact: Vec::new(), started: Instant::now(), outcome: None })
    }

    /// Images calculées et images à calculer (poignées comprises).
    pub fn frames(&self) -> (usize, usize) {
        let now = match (&self.current, self.plan.clips.get(self.next)) {
            (Some(job), Some(clip)) => match job.progress() {
                Progress::Running(n, _) => n,
                _ => clip.file_frames(),
            },
            _ => 0,
        };
        (self.frames_done + now, self.plan.frames_to_render())
    }

    /// Extraits son écrits et à écrire.
    pub fn sounds(&self) -> (usize, usize) {
        (self.worker.as_ref().map_or(self.plan.audio.len(), |w| w.done.load(Ordering::Relaxed)), self.plan.audio.len())
    }

    /// Ce que fait l'export, en une ligne.
    pub fn status(&self) -> String {
        match self.stage {
            Stage::Video => match self.plan.clips.get(self.next) {
                Some(c) => format!("Plan {} sur {} : {}", self.next + 1, self.plan.clips.len(), c.name),
                None => "Fin des vidéos…".into(),
            },
            Stage::Sound => {
                let (d, t) = self.sounds();
                format!("Son : {d} extrait(s) sur {t}")
            }
            Stage::Done => "Terminé".into(),
        }
    }

    pub fn running(&self) -> bool {
        self.outcome.is_none()
    }

    /// L'export copie encore le projet (les premières images d'écran) : il lit le projet affiché.
    pub fn needs_project(&self) -> bool {
        self.outcome.is_none() && self.stage == Stage::Video && self.snapshot.is_none() && self.builder.as_ref().is_none_or(SnapshotBuilder::reads_project)
    }

    /// Les dessins `ids` sont restés en rendu rapide : l'export les veut exacts (refaits dans le cliché, sur un autre fil).
    pub fn with_exact(mut self, ids: Vec<crate::document::CelId>) -> EditorialJob {
        self.exact = ids;
        self
    }

    /// Le cliché du projet, déjà pris (le paquet de livraison en prend un seul pour toutes ses tâches).
    pub fn with_snapshot(mut self, snapshot: Arc<Document>) -> EditorialJob {
        self.snapshot = Some(snapshot);
        self
    }

    /// Le cliché de l'export, quand il est pris.
    pub fn snapshot(&self) -> Option<Arc<Document>> {
        self.snapshot.clone()
    }

    /// Fait avancer l'export. Le projet n'est lu que pour le copier au départ (par tranches de quelques
    /// millisecondes) ; ensuite les images se calculent sur d'autres fils et cet appel ne fait que regarder.
    pub fn step(&mut self, doc: &Document, budget: Duration) {
        if self.outcome.is_some() {
            return;
        }
        let t0 = Instant::now();
        loop {
            match self.stage {
                Stage::Video => {
                    if self.snapshot.is_none() {
                        let b = self.builder.get_or_insert_with(|| {
                            let mut panels: Vec<usize> = self.plan.clips.iter().map(|c| c.panel).collect();
                            panels.sort_unstable();
                            panels.dedup();
                            SnapshotBuilder::new(doc, &panels).with_exact(std::mem::take(&mut self.exact))
                        });
                        if !b.step(doc, budget.min(Duration::from_millis(8))) {
                            return;
                        }
                        self.snapshot = self.builder.take().map(SnapshotBuilder::finish);
                    }
                    if self.current.is_none() {
                        let Some(clip) = self.plan.clips.get(self.next).cloned() else {
                            self.stage = Stage::Sound;
                            continue;
                        };
                        match self.start_clip(&clip) {
                            Ok(job) => self.current = Some(job),
                            Err(e) => return self.fail(e),
                        }
                    }
                    let left = budget.saturating_sub(t0.elapsed());
                    if let Some(job) = &mut self.current {
                        job.step(doc, left);
                    }
                    let finished = self.current.as_ref().and_then(|j| j.outcome.clone());
                    match finished {
                        None => return,
                        Some(Err(e)) => return self.fail(e),
                        Some(Ok(_)) => {
                            let clip = &self.plan.clips[self.next];
                            self.frames_done += clip.file_frames();
                            self.written.push(self.dir.join(&clip.file));
                            self.current = None;
                            self.next += 1;
                            if t0.elapsed() >= budget {
                                return;
                            }
                        }
                    }
                }
                Stage::Sound => {
                    if let Some(w) = &mut self.worker {
                        if !w.finished() {
                            return;
                        }
                        if let Err(e) = w.join() {
                            return self.fail(e);
                        }
                    }
                    match self.write_files() {
                        Ok(report) => {
                            self.stage = Stage::Done;
                            self.outcome = Some(Ok(report));
                        }
                        Err(e) => self.fail(e),
                    }
                    return;
                }
                Stage::Done => return,
            }
        }
    }

    fn start_clip(&self, clip: &PlanClip) -> Result<ExportJob, String> {
        let out = self.dir.join(&clip.file);
        let args = export::video_args_rate(self.plan.media.format(), &self.enc, self.plan.frame, &self.plan.rate.ffmpeg(), self.plan.half, None, &out)?;
        let job = ExportJob::video(&self.ffmpeg, args, &out, clip_frames(clip), None)?;
        Ok(match &self.snapshot {
            Some(s) => job.with_snapshot(s.clone()),
            None => job,
        })
    }

    /// Les fichiers de montage, une fois les médias écrits.
    fn write_files(&mut self) -> Result<Report, String> {
        let base = editorial::file_safe(&self.plan.title);
        let abs = if self.opts.absolute { Some(self.dir.canonicalize().unwrap_or_else(|_| self.dir.clone())) } else { None };
        let mut files = Vec::new();
        let put = |name: String, text: String, files: &mut Vec<PathBuf>| -> Result<(), String> {
            let path = self.dir.join(name);
            fs::write(&path, text).map_err(|e| format!("{} : {e}", path.display()))?;
            files.push(path);
            Ok(())
        };
        if self.opts.edl {
            put(format!("{base}.edl"), editorial::write_edl(&self.plan), &mut files)?;
        }
        if self.opts.xml {
            put(format!("{base}.xml"), editorial::write_xml(&self.plan, abs.as_deref()), &mut files)?;
        }
        if self.opts.otio {
            put(format!("{base}.otio"), editorial::write_otio(&self.plan, abs.as_deref()), &mut files)?;
        }
        if self.opts.fcpxml {
            // (Toujours des chemins absolus : Final Cut Pro X ne retrouve pas un média par un chemin relatif.)
            let dir = self.dir.canonicalize().unwrap_or_else(|_| self.dir.clone());
            put(format!("{base}.fcpxml"), editorial::write_fcpxml(&self.plan, &dir, self.opts.fcpxml_version), &mut files)?;
        }
        put("LISEZ-MOI.txt".into(), editorial::readme(&self.plan, &self.opts), &mut files)?;
        self.written.extend(files.iter().cloned());
        Ok(Report { dir: self.dir.clone(), files, plans: self.plan.clips.len(), sounds: self.plan.audio.len() })
    }

    fn fail(&mut self, error: String) {
        self.cleanup();
        self.stage = Stage::Done;
        self.outcome = Some(Err(error));
    }

    /// Arrête l'export et efface ce qu'il avait écrit.
    pub fn cancel(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.cleanup();
        self.stage = Stage::Done;
        self.outcome = Some(Err("Export annulé.".into()));
    }

    fn cleanup(&mut self) {
        if let Some(mut job) = self.current.take() {
            job.cancel();
        }
        if let Some(mut w) = self.worker.take() {
            w.stop();
            let files: Vec<PathBuf> = w.written.lock().unwrap_or_else(|e| e.into_inner()).clone();
            for f in files {
                let _ = fs::remove_file(f);
            }
        }
        for f in self.written.drain(..) {
            let _ = fs::remove_file(f);
        }
        // Les dossiers, s'ils sont vides (et le dossier de l'export, s'il a été créé par cet export).
        let _ = fs::remove_dir(self.dir.join("media"));
        let _ = fs::remove_dir(self.dir.join("audio"));
        if self.made_dir {
            let _ = fs::remove_dir(&self.dir);
        }
    }
}

impl Drop for EditorialJob {
    fn drop(&mut self) {
        // Fermeture de SHOT pendant un export : pas de ffmpeg ni de fil abandonné.
        if self.outcome.is_none() {
            self.cancel();
        }
    }
}
