//! Exports (3b) : film (MP4 H.264 ou MOV ProRes, encodé par ffmpeg), suite d'images PNG, son seul
//! (WAV).
//!
//! Les images sont calculées sur le fil de l'interface, quelques-unes par image d'écran : la
//! fenêtre reste vivante, et une fenêtre modale empêche de modifier le document pendant ce temps.
//! L'écriture des fichiers et l'encodage se font à côté (fils d'écriture, processus ffmpeg).

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::audio::{self, MixClip, MixState};
use crate::camera::Camera;
use crate::document::Document;
use crate::pdf;
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
    /// Storyboard PDF : planches A4 paysage, 6 panneaux par page.
    Pdf,
    /// GIF animé (ffmpeg : palette calculée sur tout le film, puis appliquée) : pour montrer, partager. Sans son.
    Gif,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::ProRes => "mov",
            Format::Png => "png",
            Format::Wav => "wav",
            Format::Pdf => "pdf",
            Format::Gif => "gif",
        }
    }

    pub fn is_video(self) -> bool {
        matches!(self, Format::Mp4 | Format::ProRes)
    }

    /// Les images passent par ffmpeg (les films et le GIF).
    pub fn uses_ffmpeg(self) -> bool {
        matches!(self, Format::Mp4 | Format::ProRes | Format::Gif)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Range {
    Montage,
    /// La séquence courante (13a : les scènes sont devenues des plans).
    Scene,
    /// Le plan courant.
    Panel,
    /// Les plans choisis (le plan courant et ceux qu'on a choisis avec lui : `App::board_selection`).
    Selection,
}

/// La taille d'un GIF : une largeur en pixels (jamais plus que l'image finale : on ne l'agrandit pas) ou
/// une part de l'image finale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GifSize {
    Width(u32),
    Percent(u32),
}

impl GifSize {
    /// Les choix de la fenêtre.
    pub const CHOICES: [GifSize; 4] = [GifSize::Width(480), GifSize::Width(720), GifSize::Percent(50), GifSize::Percent(100)];

    pub fn label(self) -> String {
        match self {
            GifSize::Width(w) => format!("{w} px de large"),
            GifSize::Percent(100) => "Image finale".to_string(),
            GifSize::Percent(p) => format!("{p} % de l'image finale"),
        }
    }

    /// Largeur et hauteur du GIF pour une image finale de `frame` : les proportions sont gardées, jamais plus
    /// grand que l'image finale, au moins 2 points.
    pub fn dimensions(self, frame: [usize; 2]) -> [usize; 2] {
        let [fw, fh] = frame;
        let w = match self {
            GifSize::Width(w) => (w as usize).min(fw),
            GifSize::Percent(p) => (fw as f64 * p.clamp(1, 100) as f64 / 100.0).round() as usize,
        }
        .clamp(2, fw.max(2));
        let h = ((fh as f64 * w as f64 / fw.max(1) as f64).round() as usize).max(2);
        [w, h]
    }
}

/// Les réglages propres au GIF.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GifSettings {
    pub size: GifSize,
    /// Images par seconde du GIF, au plus celles du projet (`gif_fps`).
    pub fps: u32,
    /// Il recommence quand il finit (sinon il joue une fois).
    pub looped: bool,
    /// Tramage : des dégradés plus doux avec 256 couleurs, au prix d'un fichier plus lourd.
    pub dither: bool,
}

impl Default for GifSettings {
    fn default() -> Self {
        Self { size: GifSize::Width(480), fps: 15, looped: true, dither: true }
    }
}

/// Cadences proposées pour un GIF (celle du projet s'y ajoute, et rien au-dessus).
pub const GIF_FPS_CHOICES: [u32; 7] = [6, 8, 10, 12, 15, 20, 24];

/// Les cadences permises pour un projet à `project_fps` : celles de la liste qui n'y dépassent pas, et la
/// sienne.
pub fn gif_fps_choices(project_fps: u32) -> Vec<u32> {
    let project_fps = project_fps.max(1);
    let mut out: Vec<u32> = GIF_FPS_CHOICES.into_iter().filter(|f| *f < project_fps).collect();
    out.push(project_fps);
    out
}

/// La cadence du GIF ramenée à ce qui se peut : au moins 1, jamais au-dessus de celle du projet.
pub fn gif_fps(wanted: u32, project_fps: u32) -> u32 {
    wanted.clamp(1, project_fps.max(1))
}

/// Les images d'un GIF à `fps` images par seconde tirées des images `frames` du film à `project_fps` : à la
/// seconde `k / fps`, l'image du film qui s'affiche alors. Aucune image du film n'est calculée pour rien (le
/// GIF n'en garde qu'une sur deux à 12 images par seconde pour un film à 24), et la durée est celle du film.
pub fn gif_sample(frames: &[(usize, usize)], project_fps: u32, fps: u32) -> Vec<(usize, usize)> {
    let fps = gif_fps(fps, project_fps) as u64;
    let project_fps = project_fps.max(1) as u64;
    let n = frames.len() as u64;
    if n == 0 {
        return Vec::new();
    }
    // Autant d'images de GIF que la durée du film le permet (au moins une).
    let count = (n * fps).div_ceil(project_fps).max(1);
    (0..count).map(|k| frames[((k * project_fps / fps).min(n - 1)) as usize]).collect()
}

/// Arguments de ffmpeg pour un GIF : images brutes RGBA (déjà à la cadence du GIF : `gif_sample`) sur l'entrée
/// standard, redimensionnées à `size`, une palette calculée sur l'ensemble (`palettegen`) puis appliquée
/// (`paletteuse`) — sans quoi les couleurs d'un dessin se voient en aplats grossiers.
pub fn gif_args(frame: [usize; 2], fps: u32, s: &GifSettings, out: &Path) -> Vec<String> {
    let [w, h] = frame;
    let [ow, oh] = s.size.dimensions(frame);
    let text = |v: &str| v.to_string();
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostats", "-y"].map(text).to_vec();
    a.extend(["-f", "rawvideo", "-pix_fmt", "rgba"].map(text));
    a.extend([text("-s"), format!("{w}x{h}"), text("-framerate"), fps.to_string(), text("-i"), text("-")]);
    let dither = if s.dither { "sierra2_4a" } else { "none" };
    let scale = if [ow, oh] == [w, h] { String::new() } else { format!("scale={ow}:{oh}:flags=lanczos,") };
    a.extend([
        text("-vf"),
        format!("{scale}split[a][b];[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither={dither}:diff_mode=rectangle"),
    ]);
    // `-loop 0` : en boucle sans fin ; `-loop -1` : une seule fois.
    a.extend([text("-loop"), text(if s.looped { "0" } else { "-1" })]);
    a.push(out.to_string_lossy().into_owned());
    a
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
    /// Avec les transitions du montage (fondus, volet, noir : `transitions.rs`).
    pub transitions: bool,
    /// Storyboard PDF avec les commentaires de la timeline (non résolus), sous l'image où ils tombent.
    pub comments: bool,
    /// Taille, cadence, boucle et tramage du GIF (`Format::Gif`).
    pub gif: GifSettings,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: Format::Mp4,
            range: Range::Montage,
            half: false,
            sound: true,
            transparent: false,
            transitions: true,
            comments: false,
            gif: GifSettings::default(),
        }
    }
}

/// Images à exporter (panneau, image), dans l'ordre du montage (sans plans choisis : voir `frames_for_chosen`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn frames_for(doc: &Document, range: Range, panel: usize) -> Vec<(usize, usize)> {
    frames_for_chosen(doc, range, panel, &[])
}

/// Comme `frames_for` ; `chosen` : les numéros des plans choisis (pour `Range::Selection`), pris dans l'ordre du
/// montage. Sans plan choisi, la sélection est le plan courant.
pub fn frames_for_chosen(doc: &Document, range: Range, panel: usize, chosen: &[u64]) -> Vec<(usize, usize)> {
    let (a, b) = match range {
        Range::Montage => (0, doc.panels.len()),
        // Un plan est une scène (13a) : « Scene » est la séquence courante.
        Range::Scene => doc.sequence_run(panel),
        Range::Panel => (panel, panel + 1),
        Range::Selection => {
            let picked: Vec<usize> = (0..doc.panels.len()).filter(|&p| chosen.contains(&doc.panels[p].id)).collect();
            let picked = if picked.is_empty() { vec![panel] } else { picked };
            return picked.into_iter().flat_map(|p| (0..doc.panels[p].frame_count()).map(move |f| (p, f))).collect();
        }
    };
    (a..b).flat_map(|p| (0..doc.panels[p].frame_count()).map(move |f| (p, f))).collect()
}

/// Taille de la vidéo : moitié si demandé, et toujours paire (exigence des codecs vidéo).
pub fn video_size(frame: [usize; 2], half: bool) -> [usize; 2] {
    let d = if half { 2 } else { 1 };
    frame.map(|v| ((v / d) & !1).max(2))
}

// ------------------------------------------------------------------------------------- ffmpeg

/// ffmpeg à utiliser : `PELURE_FFMPEG`, celui choisi dans SHOT, celui du PATH, puis ceux
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
    video_args_rate(format, enc, frame, &fps.to_string(), half, audio, out)
}

/// Comme `video_args`, la cadence écrite comme ffmpeg la lit (`24`, `24000/1001` : les vidéos par plan
/// de l'export pour le montage en NTSC, `editorial.rs`).
pub fn video_args_rate(
    format: Format,
    enc: &Encoders,
    frame: [usize; 2],
    rate: &str,
    half: bool,
    audio: Option<&Path>,
    out: &Path,
) -> Result<Vec<String>, String> {
    let fps: f64 = match rate.split_once('/') {
        Some((n, d)) => n.parse::<f64>().unwrap_or(24.0) / d.parse::<f64>().unwrap_or(1.0).max(1.0),
        None => rate.parse().unwrap_or(24.0),
    };
    let [w, h] = frame;
    let [ow, oh] = video_size(frame, half);
    let s = |v: &str| v.to_string();
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostats", "-y"].map(s).to_vec();
    a.extend(["-f", "rawvideo", "-pix_fmt", "rgba"].map(s));
    a.extend([s("-s"), format!("{w}x{h}"), s("-framerate"), rate.to_string(), s("-i"), s("-")]);
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
                    let bits = (ow * oh) as f64 * fps * 0.25;
                    a.extend([s("-c:v"), s(name), s("-b:v"), format!("{}k", (bits / 1000.0).round())]);
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

/// L'image finale du plan `p`, image `f`, avec les incrustations gravées (timecode…) des calques
/// d'incrustation visibles qui ne sont pas réglés « Seulement à l'écran » (`incrustation.rs`). L'image
/// d'origine, gardée pour l'image suivante si rien n'a changé, n'est pas touchée.
fn burned(doc: &Document, p: usize, f: usize, img: Image) -> Image {
    if !crate::incrustation::any_for(doc, p, f, true) {
        return img;
    }
    let boxes = crate::incrustation::boxes_for(doc, p, f, true, crate::incrustation::now_secs());
    let mut pixels = (*img).clone();
    crate::incrustation::burn(&mut pixels, doc.frame, &boxes);
    Arc::new(pixels)
}

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
    /// Storyboard PDF : un fil réduit chaque image en JPEG, puis écrit le fichier à la fin.
    Pdf {
        tx: Option<SyncSender<(usize, Image)>>,
        writer: Option<JoinHandle<Result<(), String>>>,
        cancelled: Arc<AtomicBool>,
    },
}

/// Largeur des images du PDF, en pixels : ~220 points par pouce dans une case de 3,5 pouces.
const PDF_IMAGE_WIDTH: u32 = 800;

/// Image d'un panneau pour le PDF : sur fond blanc, réduite, en JPEG.
pub fn board_image(img: &[Color32], size: [usize; 2]) -> Result<pdf::BoardImage, String> {
    let (width, height, jpeg) = jpeg_on_white(img, size, PDF_IMAGE_WIDTH, 88)?;
    Ok(pdf::BoardImage { width, height, jpeg })
}

/// Une image du film en JPEG (qualité `quality`) sur fond blanc, réduite à `max_width` de large au plus :
/// (largeur, hauteur, octets). Le PDF et la page de relecture s'en servent.
pub fn jpeg_on_white(img: &[Color32], size: [usize; 2], max_width: u32, quality: u8) -> Result<(u32, u32, Vec<u8>), String> {
    let [w, h] = size;
    let rgb: Vec<u8> = img
        .iter()
        .flat_map(|c| {
            // Couleurs prémultipliées posées sur du blanc.
            let white = 255 - c.a();
            [c.r().saturating_add(white), c.g().saturating_add(white), c.b().saturating_add(white)]
        })
        .collect();
    let mut image = image::RgbImage::from_raw(w as u32, h as u32, rgb).ok_or("image invalide")?;
    if image.width() > max_width {
        let nh = ((image.height() as f32 * max_width as f32 / image.width() as f32).round() as u32).max(1);
        image = image::imageops::resize(&image, max_width, nh, image::imageops::FilterType::Lanczos3);
    }
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
        .encode(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok((image.width(), image.height(), jpeg))
}

/// Panneaux (une fois chacun, dans l'ordre) des images `frames`.
pub fn panels_of(frames: &[(usize, usize)]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for &(p, _) in frames {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

/// Images de storyboard des plans `panels` (13a) : le début de chaque plan, puis chacune de ses
/// marques d'image ; pour chacune, l'image à rendre (plan, image du plan) et ce que le PDF en
/// écrit (ses textes à elle).
pub fn storyboard_of(doc: &Document, panels: &[usize]) -> (Vec<(usize, usize)>, Vec<pdf::Board>) {
    storyboard_of_with(doc, panels, false)
}

/// Comme `storyboard_of` ; `with_comments` : les commentaires de la timeline qui tombent sur chaque
/// image de storyboard sont écrits dessous (`pdf::Board::comments`).
pub fn storyboard_of_with(doc: &Document, panels: &[usize], with_comments: bool) -> (Vec<(usize, usize)>, Vec<pdf::Board>) {
    let mut frames = Vec::new();
    let mut boards = Vec::new();
    for &i in panels {
        let p = &doc.panels[i];
        let len = p.frame_count();
        let spans = crate::board::plan_boards(doc, i);
        let last = spans.len().saturating_sub(1);
        for (k, (id, start, end)) in spans.into_iter().enumerate() {
            let (action, dialogue, notes) = match p.marks.iter().find(|m| m.id == id) {
                Some(m) => (m.action.clone(), m.dialogue.clone(), m.notes.clone()),
                None => (p.action.clone(), p.dialogue.clone(), p.notes.clone()),
            };
            let mut title = crate::storyboard::panel_long_title(doc, i);
            if k > 0 {
                title = format!("{title} · image {}", k + 1);
            }
            // Les commentaires de ce plan qui se montrent entre `start` et `end` (sur la dernière image du plan
            // s'ils sont plus loin : `Document::comment_spot`).
            let comments = if with_comments {
                let mut here: Vec<&crate::commentaires::Comment> = doc
                    .comments
                    .iter()
                    .filter(|c| c.panel == p.id && !c.done)
                    .filter(|c| {
                        let at = c.frame.min(len.saturating_sub(1));
                        at >= start && (at < end || k == last)
                    })
                    .collect();
                here.sort_by_key(|c| (c.frame, c.id));
                here.into_iter().map(|c| (c.author.clone(), c.text.clone())).collect()
            } else {
                Vec::new()
            };
            frames.push((i, start));
            boards.push(pdf::Board {
                title,
                duration: crate::storyboard::duration_label(end - start, doc.fps),
                sequence: doc.sequence_label(doc.sequence_of(i)),
                action,
                dialogue,
                notes,
                comments,
            });
        }
    }
    (frames, boards)
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
            Sink::Pdf { tx: Some(tx), .. } => tx.try_send((index, img)).map_err(|e| match e {
                TrySendError::Full((_, img)) => Some(img),
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
            Sink::Pdf { tx, .. } => *tx = None,
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
            Sink::Pdf { writer, .. } => {
                if writer.as_ref().is_some_and(|w| !w.is_finished()) {
                    return None;
                }
                Some(writer.take().map_or(Ok(()), |w| w.join().unwrap_or(Err("écriture interrompue".into()))))
            }
        }
    }

    /// Arrêt immédiat (annulation, fermeture).
    fn abort(&mut self) {
        // Avant de fermer la file : le fil du PDF ne doit pas prendre la fin pour un export fini.
        if let Sink::Pdf { cancelled, .. } = self {
            cancelled.store(true, Ordering::Relaxed);
        }
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
            Sink::Pdf { writer, .. } => {
                if let Some(w) = writer.take() {
                    let _ = w.join();
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
    /// Ce qu'on compte dans la progression (« Image », « Panneau »).
    pub unit: &'static str,
    /// Fichier ou dossier produit.
    pub target: PathBuf,
    frames: Vec<(usize, usize)>,
    paper: bool,
    next: usize,
    /// Dernières images calculées : une image tenue (même plan, même caméra) n'est pas recalculée.
    /// Quatre, pour qu'une transition qui tient une image d'un côté ne la recalcule pas à chaque
    /// image de l'autre.
    cache: Vec<(Plan, Camera, Image)>,
    /// Les zones des transitions et la première image de chaque plan (`with_transitions`) ; vides :
    /// aucune transition.
    zones: Vec<crate::transitions::Zone>,
    starts: Vec<usize>,
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

    /// GIF animé encodé par ffmpeg (`args` : voir `gif_args`) : les images `frames` sont déjà celles du GIF
    /// (`gif_sample`). Même tâche de fond, même progression et même annulation que le film (le GIF inachevé est
    /// effacé).
    pub fn gif(ffmpeg: &Path, args: Vec<String>, out: &Path, frames: Vec<(usize, usize)>) -> Result<ExportJob, String> {
        let n = frames.len();
        let mut job = ExportJob::video(ffmpeg, args, out, frames, None)?;
        job.title = format!("Export du GIF ({n} images)");
        Ok(job)
    }

    /// Storyboard PDF `out` : une planche par image de storyboard de `boards`, rendue à l'image
    /// `frames` correspondante (plan, image du plan ; voir `storyboard_of`).
    pub fn pdf(out: &Path, title: &str, frames: Vec<(usize, usize)>, boards: Vec<pdf::Board>, frame: [usize; 2]) -> ExportJob {
        ExportJob::pdf_with(out, title, frames, boards, frame, false)
    }

    /// Comme `pdf` ; `notes_area` : la version à imprimer pour relire (`pdf::storyboard_with`).
    pub fn pdf_with(out: &Path, title: &str, frames: Vec<(usize, usize)>, boards: Vec<pdf::Board>, frame: [usize; 2], notes_area: bool) -> ExportJob {
        let n = boards.len();
        let (path, name) = (out.to_path_buf(), title.to_string());
        let aspect = frame[0] as f32 / frame[1].max(1) as f32;
        let mut job = ExportJob::collect(
            format!("Export du storyboard ({n} images)"),
            "Image de storyboard",
            out,
            frames,
            move |_, img| board_image(img, frame),
            move |images: Vec<pdf::BoardImage>| {
                let bytes = pdf::storyboard_with(&name, &boards, &images, aspect, notes_area);
                std::fs::write(&path, bytes).map_err(|e| format!("{} : {e}", path.display()))
            },
        );
        job.unit = "Image de storyboard";
        job
    }

    /// Un export qui garde chaque image calculée sous une autre forme et écrit le tout à la fin
    /// (un PDF, une page de relecture) : `encode(rang, pixels)` s'exécute sur un fil à part, image
    /// après image, `finish(toutes)` une fois qu'elles sont toutes arrivées. Annuler ou fermer SHOT
    /// arrête le fil et efface `out` (`finish` ne s'exécute pas).
    pub fn collect<T, E, F>(title: String, unit: &'static str, out: &Path, frames: Vec<(usize, usize)>, encode: E, finish: F) -> ExportJob
    where
        T: Send + 'static,
        E: Fn(usize, &[Color32]) -> Result<T, String> + Send + 'static,
        F: FnOnce(Vec<T>) -> Result<(), String> + Send + 'static,
    {
        let (tx, rx) = sync_channel::<(usize, Image)>(QUEUE);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let n = frames.len();
        let writer = std::thread::spawn(move || -> Result<(), String> {
            let mut done: Vec<Option<T>> = (0..n).map(|_| None).collect();
            for (i, img) in rx {
                if stop.load(Ordering::Relaxed) {
                    return Err("annulé".into());
                }
                if i < n {
                    done[i] = Some(encode(i, &img)?);
                }
            }
            if stop.load(Ordering::Relaxed) {
                return Err("annulé".into());
            }
            let all: Vec<T> = done.into_iter().collect::<Option<_>>().ok_or("image manquante")?;
            finish(all)
        });
        let mut job = ExportJob::new(title, out.to_path_buf(), frames, true, Sink::Pdf { tx: Some(tx), writer: Some(writer), cancelled });
        job.unit = unit;
        job
    }

    fn new(title: String, target: PathBuf, frames: Vec<(usize, usize)>, paper: bool, sink: Sink) -> ExportJob {
        ExportJob {
            unit: "Image",
            title,
            target,
            frames,
            paper,
            next: 0,
            cache: Vec::new(),
            zones: Vec::new(),
            starts: Vec::new(),
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

    /// Applique les transitions du montage à cet export (`transitions.rs`) : les images qu'une zone
    /// recouvre sont le mélange des deux plans, calculé ici sur le processeur, comme le reste de
    /// l'export (la carte graphique ne sert qu'à l'écran).
    pub fn with_transitions(mut self, doc: &Document) -> ExportJob {
        self.zones = crate::transitions::zones(doc);
        if !self.zones.is_empty() {
            self.starts = (0..doc.panels.len()).map(|i| doc.panel_start(i)).collect();
        }
        self
    }

    /// L'image d'un plan (caméra comprise), recalculée seulement si elle a changé.
    fn render_plain(&mut self, doc: &Document, p: usize, f: usize) -> Image {
        // Arrière-plan (13e) : aucun ou damier → image transparente.
        let plan = render::panel_plan_at(doc, p, f, if self.paper { doc.render_paper() } else { None });
        let cam = doc.panel_camera(p, f);
        // (Le cache garde l'image sans incrustation : le timecode change à chaque image, `burned` la
        // grave sur la copie qu'on rend.)
        if let Some(at) = self.cache.iter().position(|(lp, lc, _)| *lp == plan && *lc == cam) {
            let entry = self.cache.remove(at);
            let img = entry.2.clone();
            self.cache.push(entry);
            return burned(doc, p, f, img);
        }
        let img: Image = Arc::new(render::camera_image(doc, &plan, &cam));
        self.cache.push((plan, cam, img.clone()));
        if self.cache.len() > 4 {
            self.cache.remove(0);
        }
        burned(doc, p, f, img)
    }

    fn render(&mut self, doc: &Document, i: usize) -> Image {
        let (p, f) = self.frames[i];
        let mix = match self.starts.get(p) {
            Some(&start) => crate::transitions::mix_in(&self.zones, start + f).map(|(_, m)| m),
            None => None,
        };
        let Some(mix) = mix else {
            return self.render_plain(doc, p, f);
        };
        let a = mix.a.map(|(p, f)| self.render_plain(doc, p, f));
        let b = mix.b.map(|(p, f)| self.render_plain(doc, p, f));
        Arc::new(crate::transitions::blend_images(&mix, a.as_deref().map(|v| &v[..]), b.as_deref().map(|v| &v[..]), doc.frame))
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
            let what = if self.unit == "Image de storyboard" { "images de storyboard exportées" } else { "images exportées" };
            self.outcome = Some(result.map(|()| format!("{n} {what} : {}", self.target.display())));
        }
    }

    /// Arrête l'export ; une vidéo inachevée est effacée.
    pub fn cancel(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.sink.abort();
        if matches!(self.sink, Sink::Video { .. } | Sink::Pdf { .. }) {
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
