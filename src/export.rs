//! Exports (3b) : film (MP4 H.264 ou MOV ProRes, encodé par ffmpeg), suite d'images PNG, son seul
//! (WAV).
//!
//! Les images sont calculées sur le fil de l'interface, quelques-unes par image d'écran : la
//! fenêtre reste vivante, et une fenêtre modale empêche de modifier le document pendant ce temps.
//! L'écriture des fichiers et l'encodage se font à côté (fils d'écriture, processus ffmpeg).

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::audio::{self, MixClip, MixState};
use crate::camera::Camera;
use crate::document::Document;
use crate::render::{self, Plan};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// MP4 H.264 + AAC : pour regarder, envoyer.
    Mp4,
    /// MOV ProRes 422 HQ + PCM : pour un logiciel de montage.
    ProRes,
    /// Suite d'images PNG numérotées.
    Png,
    /// Son seul, WAV 48 kHz 16 bits stéréo.
    Wav,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::ProRes => "mov",
            Format::Png => "png",
            Format::Wav => "wav",
        }
    }

    pub fn is_video(self) -> bool {
        matches!(self, Format::Mp4 | Format::ProRes)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Range {
    Montage,
    Scene,
    Panel,
}

/// Choix de la fenêtre d'export.
#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub format: Format,
    pub range: Range,
    /// Vidéo à moitié de la taille du cadre (fichier de travail plus léger).
    pub half: bool,
    pub sound: bool,
    /// Images PNG sans le papier.
    pub transparent: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: Format::Mp4,
            range: Range::Montage,
            half: false,
            sound: true,
            transparent: false,
        }
    }
}

/// Images à exporter (panneau, image), dans l'ordre du montage.
pub fn frames_for(doc: &Document, range: Range, panel: usize) -> Vec<(usize, usize)> {
    let (a, b) = match range {
        Range::Montage => (0, doc.panels.len()),
        Range::Scene => doc.scene_run(panel),
        Range::Panel => (panel, panel + 1),
    };
    (a..b).flat_map(|p| (0..doc.panels[p].frame_count()).map(move |f| (p, f))).collect()
}

/// Taille de la vidéo : moitié si demandé, et toujours paire (exigence des codecs vidéo).
pub fn video_size(frame: [usize; 2], half: bool) -> [usize; 2] {
    let d = if half { 2 } else { 1 };
    frame.map(|v| ((v / d) & !1).max(2))
}

// ------------------------------------------------------------------------------------- ffmpeg

/// ffmpeg à utiliser : `PELURE_FFMPEG`, celui choisi dans Pelure, celui du PATH, puis ceux
/// installés avec d'autres logiciels (Krita) ou par winget.
pub fn find_ffmpeg(saved: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("PELURE_FFMPEG") {
        candidates.push(p.into());
    }
    if let Some(p) = saved {
        candidates.push(p.to_path_buf());
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join("ffmpeg.exe")));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(var).map(PathBuf::from) {
            candidates.push(root.join("Krita (x64)").join("bin").join("ffmpeg.exe"));
            candidates.push(root.join("ffmpeg").join("bin").join("ffmpeg.exe"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let packages = local.join("Microsoft").join("WinGet").join("Packages");
        for entry in std::fs::read_dir(packages).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy().starts_with("Gyan.FFmpeg") {
                for sub in std::fs::read_dir(entry.path()).into_iter().flatten().flatten() {
                    candidates.push(sub.path().join("bin").join("ffmpeg.exe"));
                }
            }
        }
    }
    candidates.push(PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"));
    candidates.into_iter().find(|p| p.is_file())
}

/// Encodeurs utiles proposés par un ffmpeg.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Encoders {
    /// Le meilleur encodeur H.264 disponible.
    pub h264: Option<&'static str>,
    pub prores: bool,
    pub aac: bool,
}

/// Lit la liste de `ffmpeg -encoders`.
pub fn parse_encoders(list: &str) -> Encoders {
    let has = |name: &str| list.lines().any(|l| l.split_whitespace().nth(1) == Some(name));
    Encoders {
        h264: ["libx264", "libopenh264", "h264_mf"].into_iter().find(|n| has(n)),
        prores: has("prores_ks"),
        aac: has("aac"),
    }
}

/// Lance un programme sans ouvrir de console (Pelure est une application fenêtrée).
fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

pub fn probe(ffmpeg: &Path) -> Result<Encoders, String> {
    let out = command(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffmpeg ne démarre pas ({e})"))?;
    Ok(parse_encoders(&String::from_utf8_lossy(&out.stdout)))
}

/// Arguments de ffmpeg : images brutes RGBA sur l'entrée standard, son en WAV (facultatif).
pub fn video_args(
    format: Format,
    enc: &Encoders,
    frame: [usize; 2],
    fps: u32,
    half: bool,
    audio: Option<&Path>,
    out: &Path,
) -> Result<Vec<String>, String> {
    let [w, h] = frame;
    let [ow, oh] = video_size(frame, half);
    let s = |v: &str| v.to_string();
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostats", "-y"].map(s).to_vec();
    a.extend(["-f", "rawvideo", "-pix_fmt", "rgba"].map(s));
    a.extend([s("-s"), format!("{w}x{h}"), s("-framerate"), fps.to_string(), s("-i"), s("-")]);
    if let Some(wav) = audio {
        a.extend([s("-i"), wav.to_string_lossy().into_owned()]);
    }
    a.extend(["-map", "0:v:0"].map(s));
    if audio.is_some() {
        a.extend(["-map", "1:a:0"].map(s));
    }
    let pix = match format {
        Format::Mp4 => "yuv420p",
        Format::ProRes => "yuv422p10le",
        _ => return Err("format non vidéo".into()),
    };
    // Couleurs : conversion et étiquette BT.709 (la norme HD que supposent les lecteurs).
    let resize = if half {
        format!("scale={ow}:{oh}:flags=area:out_color_matrix=bt709:out_range=tv")
    } else {
        // Taille impaire : on retire la dernière ligne ou colonne plutôt que de rééchantillonner.
        format!("crop={ow}:{oh}:0:0,scale=out_color_matrix=bt709:out_range=tv")
    };
    a.extend([s("-vf"), format!("{resize},format={pix}")]);
    a.extend(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709"].map(s));
    match format {
        Format::Mp4 => {
            match enc.h264 {
                Some("libx264") => a.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "18"].map(s)),
                Some(name) => {
                    // OpenH264 et Media Foundation se règlent au débit : ~0,25 bit par pixel.
                    let rate = (ow * oh) as f64 * fps as f64 * 0.25;
                    a.extend([s("-c:v"), s(name), s("-b:v"), format!("{}k", (rate / 1000.0).round())]);
                    if name == "libopenh264" {
                        a.extend(["-profile:v", "high"].map(s));
                    }
                }
                None => return Err("ce ffmpeg ne sait pas encoder en H.264".into()),
            }
            if audio.is_some() {
                if !enc.aac {
                    return Err("ce ffmpeg ne sait pas encoder le son en AAC".into());
                }
                a.extend(["-c:a", "aac", "-b:a", "192k"].map(s));
            }
            a.extend(["-movflags", "+faststart"].map(s));
        }
        _ => {
            if !enc.prores {
                return Err("ce ffmpeg ne sait pas encoder en ProRes".into());
            }
            a.extend(["-c:v", "prores_ks", "-profile:v", "3"].map(s));
            if audio.is_some() {
                a.extend(["-c:a", "pcm_s16le"].map(s));
            }
        }
    }
    if audio.is_some() {
        a.push(s("-shortest"));
    }
    a.push(out.to_string_lossy().into_owned());
    Ok(a)
}

// ---------------------------------------------------------------------------------------- son

/// En-tête WAV : PCM 16 bits stéréo à 48 kHz, `frames` échantillons par canal.
pub fn wav_header(frames: u32) -> [u8; 44] {
    let data = frames * 4;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&2u16.to_le_bytes());
    h[24..28].copy_from_slice(&audio::RATE.to_le_bytes());
    h[28..32].copy_from_slice(&(audio::RATE * 4).to_le_bytes());
    h[32..34].copy_from_slice(&4u16.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

/// Échantillons (à 48 kHz) couverts par les images `start..start + count` du montage.
pub fn sample_span(fps: u32, start: usize, count: usize) -> (u64, u64) {
    let spf = audio::samples_per_frame(fps);
    let a = (start as f64 * spf).round() as u64;
    let b = ((start + count) as f64 * spf).round() as u64;
    (a, b - a)
}

/// Des extraits sonnent-ils entre les échantillons `start` et `start + len` ?
pub fn has_sound(clips: &[MixClip], start: u64, len: u64) -> bool {
    clips.iter().any(|c| c.start < start + len && c.start + c.len > start)
}

/// Mixe les extraits entre `start` et `start + len` (échantillons) et l'écrit en WAV.
pub fn write_wav(path: &Path, clips: Vec<MixClip>, start: u64, len: u64) -> std::io::Result<()> {
    let mut f = BufWriter::new(std::fs::File::create(path)?);
    f.write_all(&wav_header(len as u32))?;
    let mut mix = MixState {
        clips,
        playing: true,
        pos: start as f64,
        range: (start, start + len),
        looping: false,
        scrub: None,
    };
    let mut buf = vec![0f32; 2 * 4800];
    let mut bytes = Vec::with_capacity(4 * 4800);
    let mut left = len;
    while left > 0 {
        let n = left.min(4800) as usize;
        let out = &mut buf[..2 * n];
        mix.render(out, 1.0);
        bytes.clear();
        for s in out.iter() {
            bytes.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
        f.write_all(&bytes)?;
        left -= n as u64;
    }
    f.flush()
}

// -------------------------------------------------------------------------------------- tâche

type Image = Arc<Vec<Color32>>;

/// Où partent les images calculées.
enum Sink {
    /// Fils d'écriture PNG : (numéro de l'image, image).
    Png {
        tx: Option<SyncSender<(usize, Image)>>,
        workers: Vec<JoinHandle<()>>,
        error: Arc<Mutex<Option<String>>>,
    },
    /// Processus ffmpeg, nourri par un fil d'écriture.
    Video {
        child: Child,
        tx: Option<SyncSender<Image>>,
        writer: Option<JoinHandle<Result<(), String>>>,
        stderr: Option<JoinHandle<String>>,
        /// Son mixé, à effacer à la fin.
        wav: Option<PathBuf>,
    },
}

enum Sent {
    Ok,
    Full(Image),
    Broken,
}

impl Sink {
    fn send(&mut self, index: usize, img: Image) -> Sent {
        let result = match self {
            Sink::Png { tx: Some(tx), .. } => tx.try_send((index, img)).map_err(|e| match e {
                TrySendError::Full((_, img)) => Some(img),
                TrySendError::Disconnected(_) => None,
            }),
            Sink::Video { tx: Some(tx), .. } => tx.try_send(img).map_err(|e| match e {
                TrySendError::Full(img) => Some(img),
                TrySendError::Disconnected(_) => None,
            }),
            _ => Err(None),
        };
        match result {
            Ok(()) => Sent::Ok,
            Err(Some(img)) => Sent::Full(img),
            Err(None) => Sent::Broken,
        }
    }

    /// Plus d'images : les fils finissent d'écrire ce qu'ils ont reçu.
    fn close(&mut self) {
        match self {
            Sink::Png { tx, .. } => *tx = None,
            Sink::Video { tx, .. } => *tx = None,
        }
    }

    /// Fin de l'écriture : `None` tant que ce n'est pas fini.
    fn poll(&mut self) -> Option<Result<(), String>> {
        match self {
            Sink::Png { workers, error, .. } => {
                if workers.iter().any(|w| !w.is_finished()) {
                    return None;
                }
                for w in workers.drain(..) {
                    let _ = w.join();
                }
                Some(match error.lock().unwrap().take() {
                    Some(e) => Err(e),
                    None => Ok(()),
                })
            }
            Sink::Video { child, writer, stderr, wav, .. } => {
                if writer.as_ref().is_some_and(|w| !w.is_finished()) {
                    return None;
                }
                let status = match child.try_wait() {
                    Ok(Some(s)) => s,
                    Ok(None) => return None,
                    Err(e) => return Some(Err(e.to_string())),
                };
                let written = writer.take().map_or(Ok(()), |w| w.join().unwrap_or(Err("écriture interrompue".into())));
                let message = stderr.take().and_then(|r| r.join().ok()).unwrap_or_default();
                if let Some(w) = wav.take() {
                    let _ = std::fs::remove_file(w);
                }
                Some(if status.success() && written.is_ok() {
                    Ok(())
                } else {
                    let detail = message.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
                    Err(match (detail.is_empty(), written) {
                        (false, _) => format!("ffmpeg : {detail}"),
                        (true, Err(e)) => e,
                        (true, Ok(())) => format!("ffmpeg s'est arrêté ({status})"),
                    })
                })
            }
        }
    }

    /// Arrêt immédiat (annulation, fermeture).
    fn abort(&mut self) {
        self.close();
        match self {
            Sink::Png { workers, .. } => {
                for w in workers.drain(..) {
                    let _ = w.join();
                }
            }
            Sink::Video { child, writer, stderr, wav, .. } => {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(w) = writer.take() {
                    let _ = w.join();
                }
                if let Some(r) = stderr.take() {
                    let _ = r.join();
                }
                if let Some(w) = wav.take() {
                    let _ = std::fs::remove_file(w);
                }
            }
        }
    }
}

/// Où en est un export.
pub enum Progress {
    /// Images calculées sur le total.
    Running(usize, usize),
    /// Tout est calculé ; l'écriture ou l'encodage se termine.
    Finishing,
    Done,
}

/// Un export en cours.
pub struct ExportJob {
    pub title: String,
    /// Fichier ou dossier produit.
    pub target: PathBuf,
    frames: Vec<(usize, usize)>,
    paper: bool,
    next: usize,
    /// Dernière image calculée : une image tenue (même plan, même caméra) n'est pas recalculée.
    last: Option<(Plan, Camera, Image)>,
    /// Image calculée qui attend une place dans la file d'écriture.
    pending: Option<Image>,
    sink: Sink,
    pub started: Instant,
    /// Résultat : message de réussite ou d'erreur.
    pub outcome: Option<Result<String, String>>,
}

/// Images en attente d'écriture au plus (≈ 8 Mo chacune en 1920×1080).
const QUEUE: usize = 8;

impl ExportJob {
    /// Suite d'images PNG `base_0001.png`… dans `dir`.
    pub fn images(dir: &Path, base: &str, frames: Vec<(usize, usize)>, size: [usize; 2], paper: bool) -> ExportJob {
        let (tx, rx) = sync_channel::<(usize, Image)>(QUEUE);
        let rx = Arc::new(Mutex::new(rx));
        let error = Arc::new(Mutex::new(None));
        let n = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(2, 6);
        let workers = (0..n)
            .map(|_| {
                let (rx, error, dir, base) = (rx.clone(), error.clone(), dir.to_path_buf(), base.to_string());
                std::thread::spawn(move || loop {
                    let msg = rx.lock().unwrap().recv();
                    let Ok((i, img)) = msg else { break };
                    let path = dir.join(format!("{base}_{:04}.png", i + 1));
                    if let Err(e) = crate::io::write_png(&path, &img, size[0], size[1]) {
                        *error.lock().unwrap() = Some(format!("{} : {e}", path.display()));
                        break;
                    }
                })
            })
            .collect();
        ExportJob::new(
            format!("Export de {} images", frames.len()),
            dir.to_path_buf(),
            frames,
            paper,
            Sink::Png {
                tx: Some(tx),
                workers,
                error,
            },
        )
    }

    /// Vidéo encodée par ffmpeg (`args` : voir `video_args`), avec le son `wav` s'il y en a.
    pub fn video(ffmpeg: &Path, args: Vec<String>, out: &Path, frames: Vec<(usize, usize)>, wav: Option<PathBuf>) -> Result<ExportJob, String> {
        let mut child = command(ffmpeg)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg ne démarre pas ({e})"))?;
        let mut stdin = child.stdin.take().ok_or("pas d'entrée vers ffmpeg")?;
        let mut err = child.stderr.take().ok_or("pas de sortie d'erreur de ffmpeg")?;
        let stderr = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let (tx, rx) = sync_channel::<Image>(QUEUE);
        let writer = std::thread::spawn(move || -> Result<(), String> {
            for img in rx {
                stdin
                    .write_all(bytemuck::cast_slice(&img[..]))
                    .map_err(|e| format!("ffmpeg ne reçoit plus les images ({e})"))?;
            }
            Ok(())
        });
        Ok(ExportJob::new(
            format!("Export de la vidéo ({} images)", frames.len()),
            out.to_path_buf(),
            frames,
            true,
            Sink::Video {
                child,
                tx: Some(tx),
                writer: Some(writer),
                stderr: Some(stderr),
                wav,
            },
        ))
    }

    fn new(title: String, target: PathBuf, frames: Vec<(usize, usize)>, paper: bool, sink: Sink) -> ExportJob {
        ExportJob {
            title,
            target,
            frames,
            paper,
            next: 0,
            last: None,
            pending: None,
            sink,
            started: Instant::now(),
            outcome: None,
        }
    }

    pub fn progress(&self) -> Progress {
        if self.outcome.is_some() {
            Progress::Done
        } else if self.next < self.frames.len() {
            Progress::Running(self.next, self.frames.len())
        } else {
            Progress::Finishing
        }
    }

    fn render(&mut self, doc: &Document, i: usize) -> Image {
        let (p, f) = self.frames[i];
        let plan = render::panel_plan_at(doc, p, f, self.paper.then_some(doc.paper));
        let cam = doc.panel_camera(p, f);
        if let Some((lp, lc, img)) = &self.last {
            if *lp == plan && *lc == cam {
                return img.clone();
            }
        }
        let img: Image = Arc::new(render::camera_image(doc, &plan, &cam));
        self.last = Some((plan, cam, img.clone()));
        img
    }

    /// Calcule des images pendant `budget` au plus, puis rend la main à l'interface.
    pub fn step(&mut self, doc: &Document, budget: Duration) {
        if self.outcome.is_some() {
            return;
        }
        let t0 = Instant::now();
        while self.next < self.frames.len() {
            if t0.elapsed() >= budget {
                return;
            }
            let img = match self.pending.take() {
                Some(img) => img,
                None => self.render(doc, self.next),
            };
            match self.sink.send(self.next, img) {
                Sent::Ok => self.next += 1,
                Sent::Full(img) => {
                    // L'écriture est en retard : on garde l'image pour la prochaine fois.
                    self.pending = Some(img);
                    return;
                }
                Sent::Broken => {
                    // Le fil d'écriture s'est arrêté sur une erreur : on va la lire.
                    self.next = self.frames.len();
                    break;
                }
            }
        }
        self.sink.close();
        if let Some(result) = self.sink.poll() {
            let n = self.frames.len();
            self.outcome = Some(result.map(|()| format!("{n} images exportées : {}", self.target.display())));
        }
    }

    /// Arrête l'export ; une vidéo inachevée est effacée.
    pub fn cancel(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.sink.abort();
        if matches!(self.sink, Sink::Video { .. }) {
            let _ = std::fs::remove_file(&self.target);
        }
        self.outcome = Some(Err("Export annulé.".into()));
    }

    /// Temps restant estimé, d'après le rythme jusqu'ici.
    pub fn remaining(&self) -> Option<Duration> {
        (self.next >= 4).then(|| {
            let per = self.started.elapsed().as_secs_f64() / self.next as f64;
            Duration::from_secs_f64(per * (self.frames.len() - self.next) as f64)
        })
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        // Fermeture de Pelure pendant un export : on n'abandonne pas un ffmpeg derrière soi.
        if self.outcome.is_none() {
            self.cancel();
        }
    }
}
