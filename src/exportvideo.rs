//! Le film « pro » (exports V1, 2026-10-30) : les formats de la fenêtre Film (MP4, ProRes 422 HQ, ProRes 4444 avec
//! transparence, DNxHR HQ), la taille et la cadence du fichier, et les informations gravées dans l'image (numéro de plan,
//! time code, séquence, dialogues). Les arguments de ffmpeg se calculent ici, sans lancer rien : ils se testent à sec.
//!
//! - **Taille** : `VideoSize` (projet, 4K UHD, 1080p, 720p, moitié, largeur libre). Les proportions sont gardées et les
//!   dimensions sont toujours paires (exigence des codecs).
//! - **Cadence** : `VideoRate` (celle du projet, ou 23,976 à 60). La conversion est faite par le filtre `fps` de ffmpeg : le
//!   projet ne change pas, des images sont répétées ou sautées pour garder la durée.
//! - **Informations gravées** : `BurnSettings`. Elles sont posées sur l'image finale comme une incrustation de calque
//!   (`incrustation::burn`), sans ajouter aucun calque au projet.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::export::{Encoders, Format};
use crate::incrustation::{self, Ancre, Boite, Incrustation, Quand, Source};

// -------------------------------------------------------------------------------------------- taille

/// La taille d'un film.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum VideoSize {
    /// Celle de l'image finale du projet.
    #[default]
    Project,
    /// 4K UHD : 3840 points de large.
    Uhd,
    /// 1080p : 1080 points de haut.
    Hd1080,
    /// 720p : 720 points de haut.
    Hd720,
    /// La moitié de l'image finale.
    Half,
    /// Une largeur en points.
    Width(u32),
}

impl VideoSize {
    /// Les choix de la fenêtre (la largeur libre vient avec son champ).
    pub const CHOICES: [VideoSize; 5] = [VideoSize::Project, VideoSize::Uhd, VideoSize::Hd1080, VideoSize::Hd720, VideoSize::Half];

    pub fn label(self) -> &'static str {
        match self {
            VideoSize::Project => "Taille du projet",
            VideoSize::Uhd => "4K UHD (3840 de large)",
            VideoSize::Hd1080 => "1080p (1080 de haut)",
            VideoSize::Hd720 => "720p (720 de haut)",
            VideoSize::Half => "50 % de la taille du projet",
            VideoSize::Width(_) => "Largeur libre",
        }
    }

    /// Largeur et hauteur du film pour une image finale de `frame` : proportions gardées, toujours paires, au moins 2.
    pub fn dimensions(self, frame: [usize; 2]) -> [usize; 2] {
        let [fw, fh] = [frame[0].max(1), frame[1].max(1)];
        let even = |v: f64| (((v / 2.0).round() as usize) * 2).max(2);
        match self {
            // (Comme avant : une taille impaire perd sa dernière ligne ou colonne.)
            VideoSize::Project => frame.map(|v| (v & !1).max(2)),
            VideoSize::Half => frame.map(|v| ((v / 2) & !1).max(2)),
            VideoSize::Uhd => [3840, even(fh as f64 * 3840.0 / fw as f64)],
            VideoSize::Hd1080 => [even(fw as f64 * 1080.0 / fh as f64), 1080],
            VideoSize::Hd720 => [even(fw as f64 * 720.0 / fh as f64), 720],
            VideoSize::Width(w) => {
                let w = (w as usize).clamp(2, 16384);
                [even(w as f64), even(fh as f64 * w as f64 / fw as f64)]
            }
        }
    }
}

// ------------------------------------------------------------------------------------------- cadence

/// La cadence d'un film.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum VideoRate {
    /// Celle du projet : aucune conversion.
    #[default]
    Project,
    F23976,
    F24,
    F25,
    F2997,
    F30,
    F50,
    F60,
}

impl VideoRate {
    pub const CHOICES: [VideoRate; 8] = [VideoRate::Project, VideoRate::F23976, VideoRate::F24, VideoRate::F25, VideoRate::F2997, VideoRate::F30, VideoRate::F50, VideoRate::F60];

    pub fn label(self) -> &'static str {
        match self {
            VideoRate::Project => "Celle du projet",
            VideoRate::F23976 => "23,976 images par seconde",
            VideoRate::F24 => "24 images par seconde",
            VideoRate::F25 => "25 images par seconde",
            VideoRate::F2997 => "29,97 images par seconde",
            VideoRate::F30 => "30 images par seconde",
            VideoRate::F50 => "50 images par seconde",
            VideoRate::F60 => "60 images par seconde",
        }
    }

    /// La cadence comme ffmpeg la lit ; `None` pour celle du projet.
    pub fn ffmpeg(self) -> Option<&'static str> {
        match self {
            VideoRate::Project => None,
            VideoRate::F23976 => Some("24000/1001"),
            VideoRate::F24 => Some("24"),
            VideoRate::F25 => Some("25"),
            VideoRate::F2997 => Some("30000/1001"),
            VideoRate::F30 => Some("30"),
            VideoRate::F50 => Some("50"),
            VideoRate::F60 => Some("60"),
        }
    }

    /// Images par seconde du film.
    pub fn fps(self, project: u32) -> f64 {
        match self.ffmpeg() {
            None => project as f64,
            Some(r) => rate_value(r),
        }
    }

    /// La conversion à demander à ffmpeg : rien si c'est déjà la cadence du projet.
    pub fn conversion(self, project: u32) -> Option<&'static str> {
        self.ffmpeg().filter(|_| (self.fps(project) - project as f64).abs() > 1e-9)
    }
}

/// La valeur d'une cadence écrite comme ffmpeg la lit (`24`, `24000/1001`).
pub fn rate_value(rate: &str) -> f64 {
    match rate.split_once('/') {
        Some((n, d)) => n.parse::<f64>().unwrap_or(24.0) / d.parse::<f64>().unwrap_or(1.0).max(1.0),
        None => rate.parse().unwrap_or(24.0),
    }
}

// ----------------------------------------------------------------------------------- arguments de ffmpeg

/// La plus petite image que l'encodeur H.264 de Windows accepte (largeur, hauteur).
const MF_MIN: [usize; 2] = [64, 36];

/// Tout ce qu'il faut pour écrire la ligne de commande d'un film.
pub struct VideoSpec<'a> {
    pub format: Format,
    pub enc: &'a Encoders,
    /// Les images brutes qu'on envoie à ffmpeg : l'image finale du projet.
    pub frame: [usize; 2],
    /// La taille du fichier (paire).
    pub out_size: [usize; 2],
    /// La cadence des images envoyées, comme ffmpeg la lit.
    pub rate_in: String,
    /// La cadence du fichier si elle diffère (filtre `fps`).
    pub rate_out: Option<String>,
    pub audio: Option<&'a Path>,
    pub target: &'a Path,
    /// DNxHR dans un fichier MXF plutôt que MOV.
    pub mxf: bool,
}

/// Les arguments de ffmpeg : images brutes RGBA (non prémultipliées) sur l'entrée standard, son en WAV (facultatif).
pub fn video_args(s: &VideoSpec) -> Result<Vec<String>, String> {
    let [w, h] = s.frame;
    let [ow, oh] = s.out_size;
    let out_rate = s.rate_out.clone().unwrap_or_else(|| s.rate_in.clone());
    let fps = rate_value(&out_rate);
    let t = |v: &str| v.to_string();
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostats", "-y"].map(t).to_vec();
    a.extend(["-f", "rawvideo", "-pix_fmt", "rgba"].map(t));
    a.extend([t("-s"), format!("{w}x{h}"), t("-framerate"), s.rate_in.clone(), t("-i"), t("-")]);
    if let Some(wav) = s.audio {
        a.extend([t("-i"), wav.to_string_lossy().into_owned()]);
    }
    a.extend(["-map", "0:v:0"].map(t));
    if s.audio.is_some() {
        a.extend(["-map", "1:a:0"].map(t));
    }
    let pix = match s.format {
        Format::Mp4 => "yuv420p",
        Format::ProRes => "yuv422p10le",
        Format::ProRes4444 => "yuva444p10le",
        Format::DnxHr => "yuv422p",
        _ => return Err("format non vidéo".into()),
    };
    // Cadence d'abord (sur les images du projet), puis la taille, puis les couleurs : conversion et étiquette BT.709.
    let mut chain: Vec<String> = Vec::new();
    if s.rate_out.is_some() {
        chain.push(format!("fps={out_rate}"));
    }
    let cropped = [w & !1, h & !1].map(|v| v.max(2));
    if [ow, oh] == cropped {
        // Taille impaire : on retire la dernière ligne ou colonne plutôt que de rééchantillonner.
        chain.push(format!("crop={ow}:{oh}:0:0,scale=out_color_matrix=bt709:out_range=tv"));
    } else {
        let flags = if ow * oh < w * h { "area" } else { "lanczos" };
        chain.push(format!("scale={ow}:{oh}:flags={flags}:out_color_matrix=bt709:out_range=tv"));
    }
    chain.push(format!("format={pix}"));
    a.extend([t("-vf"), chain.join(",")]);
    a.extend(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709"].map(t));
    match s.format {
        Format::Mp4 => {
            // L'encodeur H.264 de Windows refuse les images plus petites que 64 × 36 : OpenH264, s'il est là, les prend.
            let h264 = match s.enc.h264 {
                Some("h264_mf") if (ow < MF_MIN[0] || oh < MF_MIN[1]) && s.enc.openh264 => Some("libopenh264"),
                other => other,
            };
            match h264 {
                Some("libx264") => a.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "18"].map(t)),
                Some(name) => {
                    // OpenH264 et Media Foundation se règlent au débit : ~0,25 bit par pixel.
                    let bits = (ow * oh) as f64 * fps * 0.25;
                    a.extend([t("-c:v"), t(name), t("-b:v"), format!("{}k", (bits / 1000.0).round())]);
                    if name == "libopenh264" {
                        a.extend(["-profile:v", "high"].map(t));
                    }
                    if name == "h264_mf" {
                        // L'encodeur de Windows : le mode « qualité » au plus haut (le débit n'est qu'une indication). Essayé avec
                        // le ffmpeg de Krita : le fichier est deux fois plus gros qu'au débit seul, et bien plus net.
                        a.extend(["-rate_control", "quality", "-quality", "100"].map(t));
                    }
                }
                None => return Err("ce ffmpeg ne sait pas encoder en H.264".into()),
            }
            if s.audio.is_some() {
                if !s.enc.aac {
                    return Err("ce ffmpeg ne sait pas encoder le son en AAC".into());
                }
                a.extend(["-c:a", "aac", "-b:a", "192k"].map(t));
            }
            a.extend(["-movflags", "+faststart"].map(t));
        }
        Format::ProRes | Format::ProRes4444 => {
            if !s.enc.prores {
                return Err("ce ffmpeg ne sait pas encoder en ProRes".into());
            }
            let profile = if s.format == Format::ProRes4444 { "4444" } else { "3" };
            a.extend(["-c:v", "prores_ks", "-profile:v", profile].map(t));
            if s.format == Format::ProRes4444 {
                a.extend(["-alpha_bits", "16"].map(t));
            }
            if s.audio.is_some() {
                a.extend(["-c:a", "pcm_s16le"].map(t));
            }
        }
        _ => {
            if !s.enc.dnxhd {
                return Err("ce ffmpeg ne sait pas encoder en DNxHR".into());
            }
            a.extend(["-c:v", "dnxhd", "-profile:v", "dnxhr_hq"].map(t));
            if s.audio.is_some() {
                a.extend(["-c:a", "pcm_s16le"].map(t));
            }
            if s.mxf {
                a.extend(["-f", "mxf"].map(t));
            }
        }
    }
    if s.audio.is_some() {
        a.push(t("-shortest"));
    }
    a.push(s.target.to_string_lossy().into_owned());
    Ok(a)
}

/// Pourquoi ce ffmpeg ne peut pas faire ce format (`None` : il le peut) : la raison à montrer à côté du choix grisé.
pub fn missing_reason(format: Format, enc: &Encoders) -> Option<&'static str> {
    match format {
        Format::Mp4 if enc.h264.is_none() => Some("ce ffmpeg ne sait pas encoder en H.264"),
        Format::ProRes if !enc.prores => Some("ce ffmpeg ne sait pas encoder en ProRes"),
        Format::ProRes4444 if !enc.prores => Some("ce ffmpeg ne sait pas encoder en ProRes"),
        Format::DnxHr if !enc.dnxhd => Some("ce ffmpeg ne sait pas encoder en DNxHR"),
        _ => None,
    }
}

/// Pourquoi ce format ne peut pas avoir cette taille (`None` : il le peut) : le DNxHR veut au moins 256 × 120 points.
pub fn size_problem(format: Format, size: [usize; 2]) -> Option<String> {
    (format == Format::DnxHr && (size[0] < 256 || size[1] < 120)).then(|| format!("le DNxHR demande au moins 256 × 120 points (ici {} × {}) : choisis une taille plus grande", size[0], size[1]))
}

/// Comme `size_problem`, pour ce ffmpeg : sans libx264 ni OpenH264, l'encodeur H.264 de Windows refuse les images plus petites que 64 × 36.
pub fn size_problem_for(format: Format, size: [usize; 2], enc: &Encoders) -> Option<String> {
    if format == Format::Mp4 && enc.h264 == Some("h264_mf") && !enc.openh264 && (size[0] < MF_MIN[0] || size[1] < MF_MIN[1]) {
        return Some(format!("l'encodeur H.264 de Windows demande au moins {} × {} points (ici {} × {}) : choisis une taille plus grande", MF_MIN[0], MF_MIN[1], size[0], size[1]));
    }
    size_problem(format, size)
}

/// Un avertissement quand le H.264 de ce ffmpeg n'est pas le meilleur (pas de libx264).
pub fn h264_warning(enc: &Encoders) -> Option<String> {
    match enc.h264 {
        Some("libx264") | None => None,
        Some("h264_mf") => Some("qualité H.264 réduite : ce ffmpeg n'a pas libx264, SHOT utilise l'encodeur H.264 de Windows (réglé au débit : fichier plus lourd à qualité égale). Pour la meilleure qualité, choisis un ffmpeg complet (ffmpeg.org).".into()),
        Some(other) => Some(format!("qualité H.264 réduite : ce ffmpeg n'a pas libx264, SHOT utilise {other} (réglé au débit : fichier plus lourd à qualité égale). Pour la meilleure qualité, choisis un ffmpeg complet (ffmpeg.org).")),
    }
}

// ------------------------------------------------------------------------------- informations gravées

/// Les informations posées dans l'image (« Graver les informations »).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BurnSettings {
    pub enabled: bool,
    /// Le numéro du plan (SQ01_PL012A), en haut à gauche.
    pub plan: bool,
    /// Le time code du film, en bas à droite.
    pub timecode: bool,
    /// Le nom de la séquence, en haut à droite.
    pub sequence: bool,
    /// L'action et le dialogue du plan, en bas de l'image, sur plusieurs lignes.
    pub texts: bool,
}

impl Default for BurnSettings {
    fn default() -> Self {
        BurnSettings { enabled: false, plan: true, timecode: true, sequence: true, texts: false }
    }
}

impl BurnSettings {
    /// Y a-t-il quelque chose à graver ?
    pub fn active(&self) -> bool {
        self.enabled && (self.plan || self.timecode || self.sequence || self.texts)
    }
}

/// Le texte du bas de l'image à l'image `f` du plan `p` : l'action et le dialogue de la case qui s'y montre (ou du plan),
/// coupés en lignes d'au plus `max_chars` lettres, quatre lignes au plus.
pub fn texts_at(doc: &Document, p: usize, f: usize, max_chars: usize) -> String {
    let Some(panel) = doc.panels.get(p) else { return String::new() };
    let (action, dialogue) = crate::board::plan_boards(doc, p)
        .into_iter()
        .find(|&(_, a, b)| f >= a && f < b)
        .and_then(|(id, ..)| panel.marks.iter().find(|m| m.id == id))
        .map_or((panel.action.as_str(), panel.dialogue.as_str()), |m| (m.action.as_str(), m.dialogue.as_str()));
    let mut lines: Vec<String> = Vec::new();
    for (label, text) in [("Action", action), ("Dialogue", dialogue)] {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let wrapped = wrap_chars(&format!("{label} : {text}"), max_chars);
        lines.extend(wrapped);
    }
    if lines.len() > 4 {
        lines.truncate(4);
        if let Some(last) = lines.last_mut() {
            *last = format!("{}…", last.trim_end_matches(['.', ' ']));
        }
    }
    lines.join("\n")
}

/// Coupe `text` en lignes de `max` lettres au plus, aux espaces ; les retours à la ligne sont gardés.
pub fn wrap_chars(text: &str, max: usize) -> Vec<String> {
    let max = max.max(8);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let extra = if line.is_empty() { 0 } else { 1 };
            if !line.is_empty() && line.chars().count() + extra + word.chars().count() > max {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    while out.len() > 1 && out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

fn preset(source: Source, ancre: Ancre) -> Incrustation {
    Incrustation { source, ancre, taille: 3.6, marge: 2.5, quand: Quand::Toujours, ..Incrustation::default() }
}

/// Les boîtes à graver sur l'image `f` du plan `p`, de l'image finale du projet (`doc.frame`).
pub fn boxes(doc: &Document, p: usize, f: usize, s: &BurnSettings) -> Vec<Boite> {
    if !s.active() || p >= doc.panels.len() {
        return Vec::new();
    }
    let now = incrustation::now_secs();
    let mut out = Vec::new();
    let mut put = |inc: &Incrustation, text: String| {
        if let Some(b) = incrustation::render_box(inc, &text, doc.frame) {
            out.push(b);
        }
    };
    if s.plan {
        let inc = preset(Source::NomPlan, Ancre::HautGauche);
        // (Le nom du modèle de noms du projet s'il en a un, `nommage.rs` ; sinon « SQ01_PL012A » comme toujours.)
        put(&inc, if doc.properties.modele_noms.trim().is_empty() { doc.plan_full_ref(p) } else { doc.nom_de_plan(p) });
    }
    if s.sequence {
        let inc = preset(Source::NomSequence, Ancre::HautDroite);
        put(&inc, incrustation::text_for(&inc, doc, p, f, now));
    }
    if s.timecode {
        let inc = preset(Source::Timecode, Ancre::BasDroite);
        put(&inc, incrustation::text_for(&inc, doc, p, f, now));
    }
    if s.texts {
        let mut inc = preset(Source::Libre, Ancre::BasCentre);
        inc.taille = 3.2;
        inc.marge = if s.timecode { 8.0 } else { 3.0 };
        let size = inc.taille / 100.0 * doc.frame[1] as f32;
        let max_chars = ((doc.frame[0] as f32 * 0.86) / (size * 0.52).max(1.0)) as usize;
        put(&inc, texts_at(doc, p, f, max_chars));
    }
    out
}

/// L'image `img` (prémultipliée) avec les informations gravées ; l'image d'origine, qui peut être partagée, n'est pas touchée.
pub fn burn_into(doc: &Document, p: usize, f: usize, s: &BurnSettings, img: &[eframe::egui::Color32]) -> Vec<eframe::egui::Color32> {
    let mut pixels = img.to_vec();
    let boxes = boxes(doc, p, f, s);
    if !boxes.is_empty() {
        incrustation::burn(&mut pixels, doc.frame, &boxes);
    }
    pixels
}
