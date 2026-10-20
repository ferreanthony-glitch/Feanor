//! Exports (3b) : film (MP4 H.264 ou MOV ProRes, encodé par ffmpeg), GIF, suite d'images PNG, image seule,
//! storyboard PDF, page HTML, son seul (WAV).
//!
//! Aucune image n'est calculée sur le fil de l'interface, et le projet n'est pas verrouillé : l'export prend une
//! copie du projet au départ (`exportrender.rs` : le cliché, dont les dessins partagent leurs tuiles) et calcule
//! ses images sur d'autres fils, à une priorité inférieure à celle de l'interface. Un autre fil range les images
//! dans l'ordre à l'écriture (fichiers PNG, ffmpeg, PDF…). L'interface (`ExportJob::step`) ne fait que copier le
//! projet par petites tranches au départ, puis regarder où l'export en est.

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
use crate::exportrender::{self, Image, Papier, Pipeline, Pose, Rendu, SnapshotBuilder};
use crate::pdf;
use crate::render::Plan;

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
        // Sous la priorité normale : l'encodage (x264 prend tous les cœurs) ne passe pas avant l'interface.
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        c.creation_flags(CREATE_NO_WINDOW | if crate::parallele::priorite_export() { BELOW_NORMAL_PRIORITY_CLASS } else { 0 });
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn write_wav(path: &Path, clips: Vec<MixClip>, start: u64, len: u64) -> std::io::Result<()> {
    write_wav_until(path, clips, start, len, None).map(|_| ())
}

/// Comme `write_wav`, mais `cancel` est regardé entre deux morceaux : vrai, le mixage s'arrête (le fichier est
/// laissé tel quel, à effacer) et la fonction rend `false`.
pub fn write_wav_until(path: &Path, clips: Vec<MixClip>, start: u64, len: u64, cancel: Option<&AtomicBool>) -> std::io::Result<bool> {
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
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Ok(false);
        }
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
    f.flush()?;
    Ok(true)
}


// ------------------------------------------------------------------------ images de storyboard

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

// -------------------------------------------------------------------------------------- tâche

/// Écrit `bytes` sous `path` : d'abord dans `<path>.part`, puis renommé. Un disque plein, un fichier tenu ouvert
/// ailleurs ou un arrêt en route ne laissent ni demi-fichier ni fichier d'avant abîmé.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let part = part_of(path);
    std::fs::write(&part, bytes).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("{} : {e}", part.display())
    })?;
    if let Err(e) = std::fs::rename(&part, path) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("{} : {e}", path.display()));
    }
    Ok(())
}

/// `<nom>.pdf` -> `<nom>.pdf.part`.
pub(crate) fn part_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// L'image finale du plan `p`, image `f`, avec les incrustations gravées (timecode…) des calques
/// d'incrustation visibles qui ne sont pas réglés « Seulement à l'écran » (`incrustation.rs`). L'image
/// d'origine, gardée pour l'image suivante si rien n'a changé, n'est pas touchée.
pub(crate) fn burned(doc: &Document, p: usize, f: usize, img: Image) -> Image {
    if !crate::incrustation::any_for(doc, p, f, true) {
        return img;
    }
    let boxes = crate::incrustation::boxes_for(doc, p, f, true, crate::incrustation::now_secs());
    let mut pixels = (*img).clone();
    crate::incrustation::burn(&mut pixels, doc.frame, &boxes);
    Arc::new(pixels)
}

/// Images en attente d'écriture au plus (≈ 8 Mo chacune en 1920×1080, 33 Mo en 4K).
const QUEUE: usize = 4;

/// Comment s'appellent les fichiers d'une suite d'images.
#[derive(Clone, Debug)]
enum Naming {
    /// `<base>_0001.png`… dans le dossier.
    Numbered(String),
    /// Un seul fichier, ce chemin.
    Exact(PathBuf),
}

/// L'entrée du fil d'écriture : le passeur de `exportrender` la tient et la ferme en finissant.
enum Tx {
    Png(SyncSender<(usize, Image)>),
    Video(SyncSender<Image>),
    Collect(SyncSender<(usize, Image)>),
    /// Rien à écrire image par image (le son seul).
    Rien,
}

impl Tx {
    fn send(&self, index: usize, img: Image) -> Pose {
        let result = match self {
            Tx::Png(tx) | Tx::Collect(tx) => tx.try_send((index, img)).map_err(|e| match e {
                TrySendError::Full((_, img)) => Some(img),
                TrySendError::Disconnected(_) => None,
            }),
            Tx::Video(tx) => tx.try_send(img).map_err(|e| match e {
                TrySendError::Full(img) => Some(img),
                TrySendError::Disconnected(_) => None,
            }),
            Tx::Rien => Err(None),
        };
        match result {
            Ok(()) => Pose::Ok,
            Err(Some(img)) => Pose::Pleine(img),
            Err(None) => Pose::Cassee,
        }
    }
}

/// Les fils qui écrivent, et ce qu'il faut pour les arrêter.
enum Sink {
    /// Fils d'écriture PNG.
    Png { workers: Vec<JoinHandle<()>> },
    /// Processus ffmpeg, nourri par un fil d'écriture.
    Video {
        child: Child,
        writer: Option<JoinHandle<Result<(), String>>>,
        /// Ce que le fil d'écriture a rendu en finissant.
        written: Option<Result<(), String>>,
        stderr: Option<JoinHandle<String>>,
        /// Son mixé, à effacer à la fin.
        wav: Option<PathBuf>,
    },
    /// Fils qui réduisent chaque image (JPEG du PDF ou de la page de relecture, PNG de la page HTML…), puis un fil qui écrit le fichier
    /// quand elles sont toutes arrivées.
    Collect { finisher: Option<JoinHandle<Result<(), String>>>, stop: Arc<AtomicBool> },
    /// Rien n'écrit (pas encore lancé, ou le son seul).
    Rien,
}

impl Sink {
    /// Fin de l'écriture : `None` tant que ce n'est pas fini.
    fn poll(&mut self) -> Option<Result<(), String>> {
        match self {
            Sink::Png { workers } => {
                if workers.iter().any(|w| !w.is_finished()) {
                    return None;
                }
                for w in workers.drain(..) {
                    let _ = w.join();
                }
                Some(Ok(()))
            }
            Sink::Video { child, writer, written, stderr, wav } => {
                if written.is_none() && writer.as_ref().is_some_and(|w| w.is_finished()) {
                    *written = Some(writer.take().map_or(Ok(()), |w| w.join().unwrap_or(Err("écriture interrompue".into()))));
                }
                // Le fil d'écriture s'est arrêté sur une erreur : ffmpeg ne recevra plus rien, inutile de l'attendre.
                if matches!(written, Some(Err(_))) {
                    let _ = child.kill();
                }
                if written.is_none() && writer.is_some() {
                    return None;
                }
                let status = match child.try_wait() {
                    Ok(Some(s)) => s,
                    Ok(None) => return None,
                    Err(e) => return Some(Err(e.to_string())),
                };
                let written = written.take().unwrap_or(Ok(()));
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
            Sink::Collect { finisher, .. } => {
                if finisher.as_ref().is_some_and(|w| !w.is_finished()) {
                    return None;
                }
                Some(finisher.take().map_or(Ok(()), |w| w.join().unwrap_or(Err("écriture interrompue".into()))))
            }
            Sink::Rien => Some(Ok(())),
        }
    }

    /// Arrêt immédiat (annulation, fermeture). L'entrée du fil d'écriture est déjà fermée (le passeur est arrêté).
    fn abort(&mut self) {
        match self {
            Sink::Png { workers } => {
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
            Sink::Collect { finisher, stop } => {
                stop.store(true, Ordering::Relaxed);
                if let Some(w) = finisher.take() {
                    let _ = w.join();
                }
            }
            Sink::Rien => {}
        }
    }
}

/// Où en est un export.
pub enum Progress {
    /// Images passées à l'écriture sur le total.
    Running(usize, usize),
    /// Tout est calculé ; l'écriture ou l'encodage se termine.
    Finishing,
    Done,
}

/// Ce que fait l'export avant de lancer l'écriture : en général un fil de fond qui mixe le son avec le cliché du projet.
/// Rend un fichier à effacer si l'export s'arrête (le son mixé que lit ffmpeg).
type Prelude = Box<dyn FnOnce(&Document, &AtomicBool) -> Result<Option<PathBuf>, String> + Send>;

/// Lance l'écriture (ffmpeg…) quand le son mixé est prêt (son chemin) ; rend le fil d'écriture et son entrée.
type Launch = Box<dyn FnOnce(Option<PathBuf>) -> Result<(Sink, Tx), String> + Send>;

enum Stage {
    /// Rien n'est parti : le cliché du projet se prend au premier `step`.
    Neuf,
    /// Le cliché du projet se copie, par tranches de quelques millisecondes.
    Cliche(SnapshotBuilder),
    /// Le son se mixe sur un fil à part.
    Son(JoinHandle<Result<Option<PathBuf>, String>>),
    /// Les images se calculent.
    Rendu(Pipeline),
    Fini,
}

/// Ce que l'annulation d'un export efface.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Erase {
    /// Rien : les images déjà écrites d'une suite restent.
    Rien,
    /// Le fichier `.part` à côté de la cible : la cible, éventuellement d'un export d'avant, reste intacte.
    Partie,
    /// La cible elle-même : ffmpeg y écrit directement (un film inachevé n'est pas un film).
    Fichier,
}

/// Un export en cours. Le projet n'est lu qu'au départ (le cliché) : l'interface ne calcule aucune image, et le projet
/// reste libre pendant l'export.
pub struct ExportJob {
    pub title: String,
    /// Ce qu'on compte dans la progression (« Image », « Panneau »).
    pub unit: &'static str,
    /// Fichier ou dossier produit.
    pub target: PathBuf,
    frames: Arc<Vec<(usize, usize)>>,
    papier: Papier,
    graver: bool,
    explicite: Option<(Plan, Camera)>,
    /// Les zones des transitions du montage (`with_transitions`) ; vides : aucune transition.
    zones: Vec<crate::transitions::Zone>,
    snapshot: Option<Arc<Document>>,
    /// Les dessins restés en rendu rapide, à refaire exacts dans le cliché (`with_exact`).
    exact: Vec<crate::document::CelId>,
    prelude: Option<Prelude>,
    launch: Option<Launch>,
    stage: Stage,
    sink: Sink,
    tx: Option<Tx>,
    /// La première erreur d'un fil (écriture, rendu).
    fail: Arc<Mutex<Option<String>>>,
    cancel: Arc<AtomicBool>,
    /// Ce que l'annulation (ou un échec) efface.
    erase: Erase,
    /// L'écriture directe de la cible (ffmpeg) commence au lancement : `erase` passe alors à `Fichier`.
    erase_after_launch: bool,
    rendering_since: Option<Instant>,
    /// Le message de réussite, quand ce n'est pas « N images exportées ».
    done_text: Option<String>,
    pub started: Instant,
    /// Résultat : message de réussite ou d'erreur.
    pub outcome: Option<Result<String, String>>,
}

type Fail = Arc<Mutex<Option<String>>>;

/// Les fils d'écriture d'une suite d'images PNG : `n` fils qui lisent la même file.
fn png_writers(dir: PathBuf, naming: Naming, size: [usize; 2], fail: Fail) -> (SyncSender<(usize, Image)>, Vec<JoinHandle<()>>) {
    let (tx, rx) = sync_channel::<(usize, Image)>(QUEUE);
    let rx = Arc::new(Mutex::new(rx));
    let n = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(2, 6);
    let workers = (0..n)
        .map(|_| {
            let (rx, fail, dir, naming) = (rx.clone(), fail.clone(), dir.clone(), naming.clone());
            std::thread::spawn(move || {
                crate::parallele::baisser_la_priorite_ici();
                loop {
                    let msg = rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                    let Ok((i, img)) = msg else { break };
                    let path = match &naming {
                        Naming::Numbered(base) => dir.join(format!("{base}_{:04}.png", i + 1)),
                        Naming::Exact(p) => p.clone(),
                    };
                    // (D'abord sous un nom en `.part` : pas de demi-image si l'on s'arrête en route.)
                    let part = part_of(&path);
                    let written = crate::io::write_png(&part, &img, size[0], size[1]).and_then(|()| std::fs::rename(&part, &path).map_err(|e| e.to_string()));
                    if let Err(e) = written {
                        let _ = std::fs::remove_file(&part);
                        let mut f = fail.lock().unwrap_or_else(|e| e.into_inner());
                        if f.is_none() {
                            *f = Some(format!("{} : {e}", path.display()));
                        }
                        break;
                    }
                }
            })
        })
        .collect();
    (tx, workers)
}

/// Lance ffmpeg (images brutes sur son entrée standard) et le fil qui les lui écrit.
fn spawn_video(ffmpeg: &Path, args: &[String], wav: Option<PathBuf>) -> Result<(Sink, Tx), String> {
    let mut child = command(ffmpeg)
        .args(args)
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
        crate::parallele::baisser_la_priorite_ici();
        for img in rx {
            stdin
                .write_all(bytemuck::cast_slice(&img[..]))
                .map_err(|e| format!("ffmpeg ne reçoit plus les images ({e})"))?;
        }
        Ok(())
    });
    Ok((Sink::Video { child, writer: Some(writer), written: None, stderr: Some(stderr), wav }, Tx::Video(tx)))
}

/// Le son mixé de l'étendue, écrit en WAV (`start` et `len` en échantillons), pour que ffmpeg le lise à côté des images.
pub struct SoundSpan {
    pub start: u64,
    pub len: u64,
}

impl ExportJob {
    /// Suite d'images PNG `base_0001.png`… dans `dir`.
    pub fn images(dir: &Path, base: &str, frames: Vec<(usize, usize)>, size: [usize; 2], paper: bool) -> ExportJob {
        let fail: Fail = Arc::default();
        let (tx, workers) = png_writers(dir.to_path_buf(), Naming::Numbered(base.to_string()), size, fail.clone());
        let mut job = ExportJob::new(format!("Export de {} images", frames.len()), dir.to_path_buf(), frames, paper, Sink::Png { workers }, Tx::Png(tx), fail);
        job.erase = Erase::Rien;
        job
    }

    /// Une seule image PNG, `path` : le plan `plan` filmé par `cam` (celui que l'écran montre : `App::build_plan`), image
    /// `frame` du plan `panel`, avec les incrustations gravées.
    pub fn still(path: &Path, plan: Plan, cam: Camera, panel: usize, frame: usize, size: [usize; 2]) -> ExportJob {
        let fail: Fail = Arc::default();
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let (tx, workers) = png_writers(dir, Naming::Exact(path.to_path_buf()), size, fail.clone());
        let mut job = ExportJob::new("Export de l'image".to_string(), path.to_path_buf(), vec![(panel, frame)], true, Sink::Png { workers }, Tx::Png(tx), fail);
        job.explicite = Some((plan, cam));
        job.graver = true;
        job
    }

    /// Vidéo encodée par ffmpeg (`args` : voir `video_args`), avec le son `wav` s'il y en a (déjà écrit).
    pub fn video(ffmpeg: &Path, args: Vec<String>, out: &Path, frames: Vec<(usize, usize)>, wav: Option<PathBuf>) -> Result<ExportJob, String> {
        let (sink, tx) = spawn_video(ffmpeg, &args, wav)?;
        let mut job = ExportJob::new(format!("Export de la vidéo ({} images)", frames.len()), out.to_path_buf(), frames, true, sink, tx, Arc::default());
        job.erase = Erase::Fichier;
        Ok(job)
    }

    /// Comme `video`, mais le son est mixé par l'export lui-même, sur un fil à part, avant que ffmpeg parte : `args(wav)`
    /// donne les arguments de ffmpeg une fois le chemin du son connu. Rien ne s'attend sur le fil de l'interface.
    pub fn video_with_sound(
        ffmpeg: &Path,
        args: impl FnOnce(Option<&Path>) -> Result<Vec<String>, String> + Send + 'static,
        out: &Path,
        frames: Vec<(usize, usize)>,
        sound: Option<SoundSpan>,
    ) -> ExportJob {
        let ffmpeg = ffmpeg.to_path_buf();
        let launch: Launch = Box::new(move |wav| {
            let a = args(wav.as_deref())?;
            spawn_video(&ffmpeg, &a, wav)
        });
        let mut job = ExportJob::new(format!("Export de la vidéo ({} images)", frames.len()), out.to_path_buf(), frames, true, Sink::Rien, Tx::Rien, Arc::default());
        job.launch = Some(launch);
        job.tx = None;
        // (Le film n'est effacé à l'arrêt que si ffmpeg est parti : un ffmpeg qui ne démarre pas laisse le fichier d'avant.)
        job.erase = Erase::Rien;
        job.erase_after_launch = true;
        if let Some(s) = sound {
            job.prelude = Some(Box::new(move |snap, cancel| {
                let path = std::env::temp_dir().join(format!("pelure-export-{}-{}.wav", std::process::id(), NEXT_WAV.fetch_add(1, Ordering::Relaxed)));
                match write_wav_until(&path, audio::mix_clips_blocking(snap), s.start, s.len, Some(cancel)) {
                    Ok(true) => Ok(Some(path)),
                    Ok(false) => {
                        let _ = std::fs::remove_file(&path);
                        Err("annulé".into())
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&path);
                        Err(format!("Échec du mixage du son : {e}"))
                    }
                }
            }));
        }
        job
    }

    /// Le son seul de l'étendue, en WAV, mixé sur un fil à part (aucune image).
    pub fn sound(out: &Path, span: SoundSpan) -> ExportJob {
        let mut job = ExportJob::new("Export du son".to_string(), out.to_path_buf(), Vec::new(), true, Sink::Rien, Tx::Rien, Arc::default());
        let target = out.to_path_buf();
        job.done_text = Some(format!("Son exporté : {}", out.display()));
        job.prelude = Some(Box::new(move |snap, cancel| {
            let part = part_of(&target);
            match write_wav_until(&part, audio::mix_clips_blocking(snap), span.start, span.len, Some(cancel)) {
                Ok(true) => {}
                Ok(false) => {
                    let _ = std::fs::remove_file(&part);
                    return Err("annulé".into());
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    return Err(format!("Échec de l'export du son : {e}"));
                }
            }
            if cancel.load(Ordering::Relaxed) {
                let _ = std::fs::remove_file(&part);
                return Err("annulé".into());
            }
            std::fs::rename(&part, &target).map_err(|e| {
                let _ = std::fs::remove_file(&part);
                format!("Échec de l'export du son : {e}")
            })?;
            Ok(None)
        }));
        job
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
                write_atomic(&path, &bytes)
            },
        );
        job.unit = "Image de storyboard";
        job
    }

    /// Un export qui garde chaque image calculée sous une autre forme et écrit le tout à la fin
    /// (un PDF, une page de relecture) : `encode(rang, pixels)` s'exécute sur quelques fils à part, image
    /// après image, `finish(toutes)` une fois qu'elles sont toutes arrivées. Annuler ou fermer SHOT
    /// arrête les fils et efface `out` (`finish` ne s'exécute pas).
    pub fn collect<T, E, F>(title: String, unit: &'static str, out: &Path, frames: Vec<(usize, usize)>, encode: E, finish: F) -> ExportJob
    where
        T: Send + 'static,
        E: Fn(usize, &[Color32]) -> Result<T, String> + Send + Sync + 'static,
        F: FnOnce(Vec<T>) -> Result<(), String> + Send + 'static,
    {
        ExportJob::collect_with(title, unit, out, frames, encode, finish, || {})
    }

    /// Comme `collect` ; `cleanup` s'exécute si l'export s'arrête sans avoir fini (annulé, raté) : il efface ce que
    /// `encode` a écrit en route.
    pub fn collect_with<T, E, F, C>(title: String, unit: &'static str, out: &Path, frames: Vec<(usize, usize)>, encode: E, finish: F, cleanup: C) -> ExportJob
    where
        T: Send + 'static,
        E: Fn(usize, &[Color32]) -> Result<T, String> + Send + Sync + 'static,
        F: FnOnce(Vec<T>) -> Result<(), String> + Send + 'static,
        C: FnOnce() + Send + 'static,
    {
        let (tx, rx) = sync_channel::<(usize, Image)>(QUEUE);
        let rx = Arc::new(Mutex::new(rx));
        let stop = Arc::new(AtomicBool::new(false));
        let fail: Fail = Arc::default();
        let n = frames.len();
        let slots: Arc<Mutex<Vec<Option<T>>>> = Arc::new(Mutex::new((0..n).map(|_| None).collect()));
        let encode = Arc::new(encode);
        let encoders: Vec<JoinHandle<Result<(), String>>> = (0..exportrender::fils_de_rendu().0.clamp(1, 4))
            .map(|_| {
                let (rx, stop, slots, encode, fail) = (rx.clone(), stop.clone(), slots.clone(), encode.clone(), fail.clone());
                std::thread::spawn(move || -> Result<(), String> {
                    crate::parallele::baisser_la_priorite_ici();
                    loop {
                        let msg = rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        let Ok((i, img)) = msg else { return Ok(()) };
                        if stop.load(Ordering::Relaxed) {
                            return Err("annulé".into());
                        }
                        if i >= n {
                            continue;
                        }
                        match encode(i, &img) {
                            Ok(t) => slots.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(t),
                            Err(e) => {
                                let mut f = fail.lock().unwrap_or_else(|e| e.into_inner());
                                if f.is_none() {
                                    *f = Some(e.clone());
                                }
                                stop.store(true, Ordering::Relaxed);
                                return Err(e);
                            }
                        }
                    }
                })
            })
            .collect();
        let stopped = stop.clone();
        let finisher = std::thread::spawn(move || -> Result<(), String> {
            let result = (|| {
                let mut error: Option<String> = None;
                for e in encoders {
                    match e.join() {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            error.get_or_insert(e);
                        }
                        Err(_) => {
                            error.get_or_insert("écriture interrompue".into());
                        }
                    }
                }
                if let Some(e) = error {
                    return Err(e);
                }
                if stopped.load(Ordering::Relaxed) {
                    return Err("annulé".into());
                }
                let all: Vec<T> = std::mem::take(&mut *slots.lock().unwrap_or_else(|e| e.into_inner())).into_iter().collect::<Option<_>>().ok_or("image manquante")?;
                finish(all)
            })();
            if result.is_err() {
                cleanup();
            }
            result
        });
        let mut job = ExportJob::new(title, out.to_path_buf(), frames, true, Sink::Collect { finisher: Some(finisher), stop }, Tx::Collect(tx), fail);
        job.unit = unit;
        job
    }

    /// La page HTML du storyboard (une planche PNG par image de storyboard, 960 pixels de large, et `storyboard.html`
    /// dans `dir`) : le papier est toujours celui du projet, sans transition ni incrustation (comme avant).
    pub fn storyboard_page(dir: &Path, title: &str, frames: Vec<(usize, usize)>, items: Vec<PlancheHtml>, frame: [usize; 2]) -> ExportJob {
        let n = items.len();
        let (d_encode, d_finish, d_clean) = (dir.to_path_buf(), dir.to_path_buf(), dir.to_path_buf());
        let name = title.to_string();
        let written: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
        let (w_encode, w_clean) = (written.clone(), written.clone());
        let page = dir.join("storyboard.html");
        let mut job = ExportJob::collect_with(
            format!("Export de la page du storyboard ({n} images)"),
            "Image de storyboard",
            &page,
            frames,
            move |k, img| {
                let file = format!("planche_{:03}.png", k + 1);
                let path = d_encode.join(&file);
                w_encode.lock().unwrap_or_else(|e| e.into_inner()).push(path.clone());
                crate::io::write_png_scaled(&path, img, frame[0], frame[1], 960).map_err(|e| format!("{} : {e}", path.display()))?;
                Ok(file)
            },
            move |files: Vec<String>| write_atomic(&d_finish.join("storyboard.html"), storyboard_html(&name, &items, &files).as_bytes()),
            move || {
                for p in w_clean.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
                    let _ = std::fs::remove_file(p);
                }
                let _ = std::fs::remove_file(part_of(&d_clean.join("storyboard.html")));
            },
        );
        job.papier = Papier::Feuille;
        job.graver = false;
        job.done_text = Some(format!("Storyboard exporté : {}", dir.join("storyboard.html").display()));
        job
    }

    fn new(title: String, target: PathBuf, frames: Vec<(usize, usize)>, paper: bool, sink: Sink, tx: Tx, fail: Fail) -> ExportJob {
        ExportJob {
            unit: "Image",
            title,
            target,
            frames: Arc::new(frames),
            papier: if paper { Papier::Fond } else { Papier::Aucun },
            graver: true,
            explicite: None,
            zones: Vec::new(),
            snapshot: None,
            exact: Vec::new(),
            prelude: None,
            launch: None,
            stage: Stage::Neuf,
            sink,
            tx: Some(tx),
            fail,
            cancel: Arc::new(AtomicBool::new(false)),
            erase: Erase::Partie,
            erase_after_launch: false,
            rendering_since: None,
            done_text: None,
            started: Instant::now(),
            outcome: None,
        }
    }

    /// Où en est l'export : les images passées à l'écriture sur le total ; tant que le projet se copie ou que le son se
    /// mixe, zéro.
    pub fn progress(&self) -> Progress {
        let total = self.frames.len();
        if self.outcome.is_some() {
            return Progress::Done;
        }
        match &self.stage {
            Stage::Rendu(p) if p.emitted() >= total => Progress::Finishing,
            Stage::Rendu(p) => Progress::Running(p.emitted(), total),
            _ => Progress::Running(0, total),
        }
    }

    /// L'export n'a pas encore fini de copier le projet : il lit encore le projet affiché (il faut que ce soit le même, et
    /// qu'il ne change pas d'onglet).
    pub fn needs_project(&self) -> bool {
        self.outcome.is_none()
            && match &self.stage {
                Stage::Neuf => self.snapshot.is_none(),
                Stage::Cliche(b) => b.reads_project(),
                _ => false,
            }
    }

    /// Ce que l'export fait en ce moment, en quelques mots.
    pub fn phase(&self) -> &'static str {
        match &self.stage {
            Stage::Neuf | Stage::Cliche(_) => "Copie du projet…",
            Stage::Son(_) => "Mixage du son…",
            Stage::Rendu(_) => "Calcul des images…",
            Stage::Fini => "",
        }
    }

    /// Applique les transitions du montage à cet export (`transitions.rs`) : les images qu'une zone
    /// recouvre sont le mélange des deux plans, calculé sur le processeur, comme le reste de l'export (la
    /// carte graphique ne sert qu'à l'écran).
    pub fn with_transitions(mut self, doc: &Document) -> ExportJob {
        self.zones = crate::transitions::zones(doc);
        self
    }

    /// Les dessins `ids` du projet sont restés en rendu rapide (traits texturés, `App::rejeu_idle`) : l'export les veut exacts.
    /// Ils sont refaits dans le cliché, sur un autre fil ; le projet affiché garde son rendu rapide.
    pub fn set_exact(&mut self, ids: Vec<crate::document::CelId>) {
        self.exact = ids;
    }

    /// Le cliché du projet, déjà pris (l'export pour le montage en fait un pour tous ses fichiers).
    pub fn with_snapshot(mut self, snapshot: Arc<Document>) -> ExportJob {
        self.snapshot = Some(snapshot);
        self
    }

    /// Le cliché de l'export, quand il est pris.
    #[cfg(test)]
    pub fn snapshot(&self) -> Option<Arc<Document>> {
        self.snapshot.clone()
    }

    /// Fait avancer l'export. `doc` n'est lu que pour copier le projet au départ (quelques millisecondes au plus par
    /// appel, `budget` borné) ; ensuite l'export tourne sur son cliché, sur d'autres fils, et cet appel ne fait que
    /// regarder où il en est.
    pub fn step(&mut self, doc: &Document, budget: Duration) {
        if self.outcome.is_some() {
            return;
        }
        let failure = self.fail.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(e) = failure {
            self.stop_with(Err(e));
            return;
        }
        match &self.stage {
            Stage::Neuf => match self.snapshot.clone() {
                Some(s) => self.after_snapshot(s),
                None => {
                    let panels = exportrender::panels_needed(&self.frames, &self.zones);
                    self.stage = Stage::Cliche(SnapshotBuilder::new(doc, &panels).with_exact(std::mem::take(&mut self.exact)));
                    self.advance_snapshot(doc, budget);
                }
            },
            Stage::Cliche(_) => self.advance_snapshot(doc, budget),
            Stage::Son(h) => {
                if h.is_finished() {
                    let Stage::Son(h) = std::mem::replace(&mut self.stage, Stage::Fini) else { return };
                    match h.join() {
                        Ok(Ok(wav)) => match self.snapshot.clone() {
                            Some(s) => self.launch_render(s, wav),
                            None => self.stop_with(Err("cliché du projet manquant".into())),
                        },
                        Ok(Err(e)) => self.stop_with(Err(e)),
                        Err(_) => self.stop_with(Err("le mixage du son a échoué".into())),
                    }
                }
            }
            Stage::Rendu(p) => {
                if p.finished() {
                    self.conclude();
                }
            }
            Stage::Fini => {}
        }
    }

    fn advance_snapshot(&mut self, doc: &Document, budget: Duration) {
        let Stage::Cliche(b) = &mut self.stage else { return };
        if b.step(doc, budget.min(Duration::from_millis(8))) {
            let Stage::Cliche(b) = std::mem::replace(&mut self.stage, Stage::Fini) else { return };
            let snapshot = b.finish();
            self.snapshot = Some(snapshot.clone());
            self.after_snapshot(snapshot);
        }
    }

    fn after_snapshot(&mut self, snapshot: Arc<Document>) {
        let Some(prelude) = self.prelude.take() else {
            return self.launch_render(snapshot, None);
        };
        let cancel = self.cancel.clone();
        let started = std::thread::Builder::new().name("export-son".into()).spawn(move || {
            crate::parallele::baisser_la_priorite_ici();
            prelude(&snapshot, &cancel)
        });
        match started {
            Ok(h) => self.stage = Stage::Son(h),
            Err(e) => self.stop_with(Err(format!("impossible de lancer l'export ({e})"))),
        }
    }

    /// Le cliché est pris (et le son mixé) : l'écriture démarre, puis les fils de rendu.
    fn launch_render(&mut self, snapshot: Arc<Document>, wav: Option<PathBuf>) {
        if let Some(launch) = self.launch.take() {
            match launch(wav) {
                Ok((sink, tx)) => {
                    self.sink = sink;
                    self.tx = Some(tx);
                    if self.erase_after_launch {
                        self.erase = Erase::Fichier;
                    }
                }
                Err(e) => return self.stop_with(Err(e)),
            }
        }
        let Some(tx) = self.tx.take() else {
            return self.stop_with(Err("l'écriture n'a pas démarré".into()));
        };
        let mut rendu = Rendu::new(snapshot, self.papier, self.graver, self.zones.clone(), self.cancel.clone());
        if let Some((plan, cam)) = self.explicite.clone() {
            rendu = rendu.avec_plan(plan, cam);
        }
        self.rendering_since = Some(Instant::now());
        let pipeline = Pipeline::start(Arc::new(rendu), self.frames.clone(), Box::new(move |i, img| tx.send(i, img)), self.fail.clone(), QUEUE);
        self.stage = Stage::Rendu(pipeline);
    }

    /// Toutes les images sont parties (ou le passeur s'est arrêté) : on attend les fils d'écriture.
    fn conclude(&mut self) {
        let Stage::Rendu(p) = &self.stage else { return };
        let (sent, total) = (p.emitted(), self.frames.len());
        let Some(result) = self.sink.poll() else { return };
        // (Les fils de rendu encore là — le passeur s'est arrêté avant la fin — s'en vont.)
        self.cancel.store(true, Ordering::Relaxed);
        self.stage = Stage::Fini;
        let what = if self.unit == "Image de storyboard" { "images de storyboard exportées" } else { "images exportées" };
        self.outcome = Some(match result {
            Ok(()) if sent < total => Err("écriture interrompue".into()),
            Ok(()) => Ok(self.done_text.clone().unwrap_or_else(|| format!("{total} {what} : {}", self.target.display()))),
            Err(e) => Err(e),
        });
    }

    /// Arrête tout et note `outcome` : fils de rendu, passeur, écriture ; ce que l'export a commencé d'écrire est effacé.
    fn stop_with(&mut self, outcome: Result<String, String>) {
        self.cancel.store(true, Ordering::Relaxed);
        match std::mem::replace(&mut self.stage, Stage::Fini) {
            Stage::Rendu(mut p) => p.cancel(),
            Stage::Son(h) => {
                // Le mixage ne s'interrompt pas en route : un fil à part attend sa fin et efface ce qu'il a écrit.
                let _ = std::thread::Builder::new().name("export-nettoyage".into()).spawn(move || {
                    if let Ok(Ok(Some(path))) = h.join() {
                        let _ = std::fs::remove_file(path);
                    }
                });
            }
            _ => {}
        }
        self.prelude = None;
        self.launch = None;
        self.tx = None;
        self.sink.abort();
        match self.erase {
            Erase::Rien => {}
            Erase::Partie => {
                let _ = std::fs::remove_file(part_of(&self.target));
            }
            Erase::Fichier => {
                let _ = std::fs::remove_file(&self.target);
            }
        }
        self.outcome = Some(outcome);
    }

    /// Arrête l'export ; un film, un PDF ou un son inachevé est effacé.
    pub fn cancel(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.stop_with(Err("Export annulé.".into()));
    }

    /// Temps restant estimé, d'après le rythme jusqu'ici.
    pub fn remaining(&self) -> Option<Duration> {
        let Stage::Rendu(p) = &self.stage else { return None };
        let (done, total) = (p.emitted(), self.frames.len());
        (done >= 4 && total > done).then(|| {
            let per = self.rendering_since.map_or(0.0, |t| t.elapsed().as_secs_f64()) / done as f64;
            Duration::from_secs_f64(per * (total - done) as f64)
        })
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        // Fermeture de SHOT pendant un export : on n'abandonne pas un ffmpeg derrière soi.
        if self.outcome.is_none() {
            self.cancel();
        }
    }
}

static NEXT_WAV: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// ------------------------------------------------------------------------------ la page HTML

/// Une planche de la page HTML du storyboard.
#[derive(Clone, Debug, Default)]
pub struct PlancheHtml {
    /// L'intertitre qui ouvre une séquence (« Séquence 2 »), s'il y en a un avant cette planche.
    pub sequence: Option<String>,
    /// Ce qui précède l'intertitre : c'est la première séquence (pas de `</main>` à fermer).
    pub premiere: bool,
    pub title: String,
    pub duration: String,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

/// Les planches de la page HTML : une par image de storyboard de `panels`, avec l'intertitre des séquences.
pub fn storyboard_items(doc: &Document, panels: &[usize]) -> (Vec<(usize, usize)>, Vec<PlancheHtml>) {
    let (frames, boards) = storyboard_of(doc, panels);
    let items = frames
        .iter()
        .zip(boards)
        .map(|(&(i, f), board)| {
            // Intertitre au début de chaque séquence.
            let sequence = (f == 0 && doc.sequence_run(i).0 == i).then(|| doc.sequence_label(doc.sequence_of(i)));
            PlancheHtml { sequence, premiere: i == 0, title: board.title, duration: board.duration, action: board.action, dialogue: board.dialogue, notes: board.notes }
        })
        .collect();
    (frames, items)
}

/// Le texte de la page HTML : `files` donne le fichier image de chaque planche.
pub fn storyboard_html(title: &str, items: &[PlancheHtml], files: &[String]) -> String {
    use crate::io::html_escape;
    let title = html_escape(title);
    let mut html = format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><title>{title}</title><style>\
         body{{font-family:system-ui,sans-serif;margin:24px;color:#222}}\
         h1{{font-size:20px}}main{{display:grid;grid-template-columns:repeat(2,1fr);gap:24px}}\
         section{{border:1px solid #ccc;padding:10px;break-inside:avoid}}\
         img{{width:100%;border:1px solid #999}}h2{{font-size:15px;margin:8px 0 2px}}\
         .dur{{color:#777;font-size:12px;margin:0 0 6px}}p{{font-size:13px;margin:4px 0}}\
         .label{{font-weight:600}}h2.seq{{font-size:18px;margin:28px 0 10px;break-after:avoid}}\
         </style></head><body><h1>{title} — storyboard</h1>"
    );
    for (item, file) in items.iter().zip(files) {
        if let Some(seq) = &item.sequence {
            if !item.premiere {
                html += "</main>";
            }
            html += &format!("<h2 class=\"seq\">Séquence {}</h2><main>", html_escape(seq));
        }
        html += &format!("<section><img src=\"{file}\"><h2>{}</h2><p class=\"dur\">{}</p>", html_escape(&item.title), item.duration);
        for (label, text) in [("Action", &item.action), ("Dialogue", &item.dialogue), ("Notes", &item.notes)] {
            if !text.trim().is_empty() {
                html += &format!("<p><span class=\"label\">{label} :</span> {}</p>", html_escape(text));
            }
        }
        html += "</section>";
    }
    html += "</main></body></html>";
    html
}
