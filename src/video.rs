//! Calque vidéo (13k), celui de TVPaint 12 (000020117–119, 000024902, 011085035). Choix
//! d'Anthony (2026-10-16) : la vidéo est lue depuis le fichier, légère en mémoire ; on importe
//! aussi une séquence d'images et le son de la vidéo.
//!
//! À l'import, ffmpeg décode la vidéo une fois en images JPEG à la taille du cadre (une séquence
//! d'images est mise à la taille du cadre, en PNG) dans un dossier à elle : celui du projet
//! (`videos/<id>/`) une fois enregistré, un dossier temporaire avant. Le calque n'a pas de
//! dessins : à chaque image du plan, il lit l'image de la vidéo qui tombe là (selon la cadence
//! de la vidéo et celle du projet), à la demande, gardée dans un petit cache. « Transformer en
//! calque d'animation » en fait des dessins, pour dessiner dessus.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde::{Deserialize, Serialize};

use crate::document::{Cel, CelId, Document};
use crate::raster::{Raster, TILE};

/// La vidéo d'un calque vidéo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoClip {
    /// Nom de son dossier d'images (`videos/<id>/`).
    pub id: u64,
    /// Nom du fichier importé (ou de la première image de la séquence).
    pub name: String,
    /// Nombre d'images de la vidéo.
    pub frames: usize,
    /// Cadence de la vidéo (images par seconde).
    pub fps: f64,
    /// Image du plan où commence la vidéo (négative : la vidéo a commencé avant le plan, dont le
    /// début a été rogné, `rognage.rs`).
    pub start: i64,
    /// Extension des images (`jpg` pour une vidéo, `png` pour une séquence).
    pub ext: String,
}

impl VideoClip {
    /// Image de la vidéo montrée à l'image `frame` du plan (cadence du projet `fps`), s'il y en
    /// a une.
    pub fn index_at(&self, frame: usize, fps: u32) -> Option<usize> {
        let t = usize::try_from(frame as i64 - self.start).ok()?;
        let k = (t as f64 * self.fps / fps.max(1) as f64 + 1e-6).floor() as usize;
        (k < self.frames).then_some(k)
    }

    /// Nombre d'images du plan que dure la vidéo.
    pub fn plan_frames(&self, fps: u32) -> usize {
        (self.frames as f64 * fps.max(1) as f64 / self.fps.max(0.001) - 1e-6).ceil().max(1.0) as usize
    }

    /// Image du plan juste après la fin de la vidéo.
    pub fn end(&self, fps: u32) -> usize {
        (self.start + self.plan_frames(fps) as i64).max(0) as usize
    }

    /// Image du plan où commence la vidéo, ramenée à 0 si elle commence avant le plan.
    pub fn first_frame(&self) -> usize {
        self.start.max(0) as usize
    }
}

/// Fichier de l'image `k` (à partir de 0) d'une vidéo dans `dir`.
pub fn frame_file(dir: &Path, k: usize, ext: &str) -> PathBuf {
    dir.join(format!("{:06}.{ext}", k + 1))
}

/// Un numéro neuf pour une vidéo importée (heure et compteur : pas de collision entre projets).
pub fn new_id() -> u64 {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros() as u64);
    (t << 8 | (COUNT.fetch_add(1, Ordering::Relaxed) as u64 & 0xff)) & !(3 << 62)
}

/// Dossier temporaire des images d'une vidéo pas encore enregistrée dans un projet.
pub fn temp_dir(id: u64) -> PathBuf {
    std::env::temp_dir().join("pelure-videos").join(id.to_string())
}

// ---------------------------------------------------------------------- où sont les images

/// Dossier des images de chaque vidéo de la session.
static DIRS: Mutex<Option<HashMap<u64, PathBuf>>> = Mutex::new(None);

/// Les images de la vidéo `id` sont dans `dir`.
pub fn register_dir(id: u64, dir: PathBuf) {
    DIRS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id, dir);
}

/// Dossier des images de la vidéo `id`.
pub fn dir_of(id: u64) -> Option<PathBuf> {
    DIRS.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(&id).cloned()
}

/// Images de vidéo vues dans un plan : numéro de dessin → (vidéo, image, extension).
static FRAMES: Mutex<Option<HashMap<CelId, (u64, usize, String)>>> = Mutex::new(None);

/// Numéro de dessin de l'image `k` de la vidéo `clip` (bit 62 mis : ni un vrai dessin, ni un
/// dessin dérivé d'effets) ; retenu pour que les moteurs le retrouvent (`shown`).
pub fn frame_id(clip: &VideoClip, k: usize) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    clip.id.hash(&mut h);
    k.hash(&mut h);
    let id = (h.finish() & !(3 << 62)) | (1 << 62);
    FRAMES.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).entry(id).or_insert_with(|| (clip.id, k, clip.ext.clone()));
    id
}

/// Le dessin que montre le calque `layer` à l'image `frame` (cadence du projet `fps`) : l'image
/// de sa vidéo pour un calque vidéo, sinon son dessin (pré / post-comportements compris).
pub fn cel_at(layer: &crate::document::Layer, frame: usize, fps: u32) -> Option<CelId> {
    match &layer.video {
        Some(v) => v.index_at(frame, fps).map(|k| frame_id(v, k)),
        None => layer.shown_at(frame),
    }
}

/// Images de vidéo gardées en mémoire : les plus anciennes partent au-delà de `CACHE_MAX`.
const CACHE_MAX: usize = 24;
static CACHE: Mutex<Vec<(CelId, Arc<Cel>)>> = Mutex::new(Vec::new());

/// Lit une image (fichier) et la pose au centre du cadre d'une feuille `doc`.
pub fn read_frame(doc: &Document, path: &Path) -> Option<Raster> {
    let img = image::open(path).ok()?.to_rgba8();
    let (iw, ih) = (img.width() as usize, img.height() as usize);
    let frame = doc.frame_rect();
    let x0 = (frame.min.x as i64 + (frame.width() as i64 - iw as i64) / 2).max(0) as usize;
    let y0 = (frame.min.y as i64 + (frame.height() as i64 - ih as i64) / 2).max(0) as usize;
    let mut out = Raster::new(doc.width, doc.height);
    for (y, row) in img.rows().enumerate() {
        let sy = y0 + y;
        if sy >= doc.height {
            break;
        }
        for (x, p) in row.enumerate() {
            let sx = x0 + x;
            if sx >= doc.width || p[3] == 0 {
                continue;
            }
            let idx = out.tile_index_of(sx, sy);
            out.tile_mut(idx)[(sy % TILE) * TILE + sx % TILE] = crate::raster::premultiply(p[0], p[1], p[2], p[3]);
        }
    }
    Some(out)
}

/// L'image de vidéo `cel` (un numéro de `frame_id`), lue à la demande et gardée un moment.
pub fn shown(doc: &Document, cel: CelId) -> Option<Arc<Cel>> {
    let (id, k, ext) = FRAMES.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(&cel).cloned()?;
    {
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = cache.iter().position(|(c, _)| *c == cel) {
            let entry = cache.remove(i);
            let found = entry.1.clone();
            cache.push(entry);
            return Some(found);
        }
    }
    let raster = read_frame(doc, &frame_file(&dir_of(id)?, k, &ext))?;
    let loaded = Arc::new(Cel { raster, vector: None, version: 1, fond: None });
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.push((cel, loaded.clone()));
    if cache.len() > CACHE_MAX {
        cache.remove(0);
    }
    Some(loaded)
}

// ---------------------------------------------------------------------- import

/// Cadence lue dans ce que ffmpeg écrit sur une vidéo (« …, 23.98 fps, … »).
pub fn parse_fps(stderr: &str) -> Option<f64> {
    stderr.lines().filter(|l| l.contains("Video:")).find_map(|l| {
        let words: Vec<&str> = l.split(|c: char| c == ',' || c.is_whitespace()).filter(|w| !w.is_empty()).collect();
        words.windows(2).find(|w| w[1] == "fps").and_then(|w| w[0].parse::<f64>().ok()).filter(|v| *v > 0.0)
    })
}

/// Nom d'image numérotée : (préfixe, numéro, extension), par exemple « plan_0012.png ».
pub fn split_numbered(name: &str) -> Option<(String, u64, String)> {
    let (stem, ext) = name.rsplit_once('.')?;
    let digits = stem.len() - stem.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let (prefix, number) = stem.split_at(stem.len() - digits);
    Some((prefix.to_string(), number.parse().ok()?, ext.to_ascii_lowercase()))
}

/// La séquence d'images dont `pick` fait partie : les fichiers du même dossier de même préfixe
/// et même extension, numérotés, dans l'ordre des numéros.
pub fn sequence_files(pick: &Path) -> Vec<PathBuf> {
    let Some((prefix, _, ext)) = pick.file_name().and_then(|n| n.to_str()).and_then(split_numbered) else { return vec![pick.to_path_buf()] };
    let dir = pick.parent().unwrap_or(Path::new("."));
    let mut found: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let (p, n, x) = split_numbered(&name)?;
            (p == prefix && x == ext).then(|| (n, e.path()))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

/// Ce qu'on importe.
pub enum Source {
    Video(PathBuf),
    Sequence(Vec<PathBuf>),
}

/// Ce qu'un import a produit : la vidéo (commençant à l'image 0) et le son extrait, s'il y en a.
pub struct Imported {
    pub clip: VideoClip,
    pub sound: Option<PathBuf>,
}

/// Un import en cours, sur un autre fil.
pub struct ImportJob {
    pub name: String,
    pub progress: Arc<AtomicUsize>,
    pub cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Imported, String>>>,
}

impl ImportJob {
    /// Le résultat, une fois l'import fini.
    pub fn finished(&mut self) -> Option<Result<Imported, String>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        let handle = self.handle.take()?;
        Some(handle.join().unwrap_or_else(|_| Err("l'import s'est arrêté".into())))
    }
}

/// Lance un programme sans ouvrir de console.
fn command(program: &Path) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn count_frames(dir: &Path, ext: &str) -> usize {
    std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext)).count()
}

/// Lance l'import de `source` en images à la taille du cadre `frame` (ffmpeg : celui de
/// l'export, nécessaire pour une vidéo). Une séquence prend la cadence du projet `fps`.
pub fn start_import(ffmpeg: Option<PathBuf>, source: Source, frame: [usize; 2], fps: u32) -> ImportJob {
    let progress = Arc::new(AtomicUsize::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let name = match &source {
        Source::Video(p) => p.file_name().map_or("vidéo".into(), |n| n.to_string_lossy().into_owned()),
        Source::Sequence(files) => files.first().and_then(|p| p.file_name()).map_or("séquence".into(), |n| n.to_string_lossy().into_owned()),
    };
    let (p, c, n) = (progress.clone(), cancel.clone(), name.clone());
    let handle = std::thread::spawn(move || -> Result<Imported, String> {
        let id = new_id();
        let dir = temp_dir(id);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        let [fw, fh] = frame;
        match source {
            Source::Video(path) => {
                let ffmpeg = ffmpeg.ok_or("ffmpeg introuvable : choisis-le dans la fenêtre d'export (Fichier > Exporter le film)")?;
                let mut child = command(&ffmpeg)
                    .args(["-hide_banner", "-y", "-i"])
                    .arg(&path)
                    .args(["-vf", &format!("scale={fw}:{fh}:force_original_aspect_ratio=decrease"), "-q:v", "3"])
                    .arg(dir.join("%06d.jpg"))
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("ffmpeg ne démarre pas ({e})"))?;
                // ffmpeg écrit beaucoup sur sa sortie d'erreur : on la lit à côté.
                let mut err = child.stderr.take();
                let reader = std::thread::spawn(move || {
                    let mut s = String::new();
                    if let Some(e) = err.as_mut() {
                        use std::io::Read;
                        let _ = e.read_to_string(&mut s);
                    }
                    s
                });
                loop {
                    if c.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        return Err("import annulé".into());
                    }
                    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                        let log = reader.join().unwrap_or_default();
                        let frames = count_frames(&dir, "jpg");
                        if !status.success() || frames == 0 {
                            let last = log.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
                            return Err(format!("ffmpeg n'a pas pu lire « {n} » ({last})"));
                        }
                        let fps = parse_fps(&log).unwrap_or(fps as f64);
                        p.store(frames, Ordering::Relaxed);
                        // Le son de la vidéo (s'il y en a un).
                        let wav = dir.join("son.wav");
                        let sound = command(&ffmpeg)
                            .args(["-hide_banner", "-y", "-i"])
                            .arg(&path)
                            .args(["-vn", "-ac", "2", "-ar", "48000", "-c:a", "pcm_s16le"])
                            .arg(&wav)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status()
                            .ok()
                            .filter(|s| s.success())
                            .and_then(|_| std::fs::metadata(&wav).ok())
                            .filter(|m| m.len() > 1024)
                            .map(|_| wav);
                        return Ok(Imported { clip: VideoClip { id, name: n, frames, fps, start: 0, ext: "jpg".into() }, sound });
                    }
                    p.store(count_frames(&dir, "jpg"), Ordering::Relaxed);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
            Source::Sequence(files) => {
                for (k, file) in files.iter().enumerate() {
                    if c.load(Ordering::Relaxed) {
                        return Err("import annulé".into());
                    }
                    let img = image::open(file).map_err(|e| format!("{} : {e}", file.display()))?;
                    let img = if img.width() as usize > fw || img.height() as usize > fh {
                        img.resize(fw as u32, fh as u32, image::imageops::FilterType::Triangle)
                    } else {
                        img
                    };
                    img.to_rgba8().save(frame_file(&dir, k, "png")).map_err(|e| e.to_string())?;
                    p.store(k + 1, Ordering::Relaxed);
                }
                Ok(Imported { clip: VideoClip { id, name: n, frames: files.len(), fps: fps as f64, start: 0, ext: "png".into() }, sound: None })
            }
        }
    });
    ImportJob { name, progress, cancel, handle: Some(handle) }
}

/// Recopie les images de la vidéo `clip` (là où elles sont) dans `to`, celles qui y manquent.
/// Renvoie le nombre de fichiers écrits.
pub fn copy_frames(clip: &VideoClip, from: &Path, to: &Path) -> Result<usize, String> {
    if from == to {
        return Ok(0);
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{} : {e}", to.display()))?;
    let mut n = 0;
    for k in 0..clip.frames {
        let (src, dst) = (frame_file(from, k, &clip.ext), frame_file(to, k, &clip.ext));
        if !dst.exists() {
            std::fs::copy(&src, &dst).map_err(|e| format!("{} : {e}", src.display()))?;
            n += 1;
        }
    }
    Ok(n)
}
