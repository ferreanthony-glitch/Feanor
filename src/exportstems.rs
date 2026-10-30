//! « Son par piste » (exports V1, 2026-10-30) : un fichier son par piste audible du montage (une piste muette, ou qu'une autre en solo
//! couvre, n'est pas écrite), plus, si on le veut, le mixage de toutes ; en WAV ou en AIFF, 16 ou 24 bits, toujours à 48 kHz stéréo.
//! Les fichiers portent le nom de la piste (accents gardés, caractères interdits de Windows remplacés).
//!
//! Chaque fichier a le volume de sa piste et le volume général, les fondus, les clés de volume, l'égaliseur et l'écho des extraits
//! (`audiofx::build_mix_where`) : il sonne comme la piste dans le mixage. Tous commencent au début de l'étendue exportée et ont la même
//! durée : on les pose les uns sous les autres dans un logiciel de son et ils tombent en place.
//!
//! Le mixage se fait sur un fil à part (`ExportJob::task`), sur une copie du projet : l'interface ne calcule rien. Annuler efface ce qui a
//! été écrit.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::audio::{self, MixClip, MixState};
use crate::audiofx;
use crate::document::Document;

/// La profondeur des échantillons.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Depth {
    #[default]
    Bits16,
    Bits24,
}

impl Depth {
    pub fn bits(self) -> u16 {
        match self {
            Depth::Bits16 => 16,
            Depth::Bits24 => 24,
        }
    }

    /// Octets par échantillon.
    pub fn bytes(self) -> usize {
        self.bits() as usize / 8
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Container {
    #[default]
    Wav,
    Aiff,
}

impl Container {
    pub fn extension(self) -> &'static str {
        match self {
            Container::Wav => "wav",
            Container::Aiff => "aiff",
        }
    }
}

/// Le format d'un fichier son : conteneur et profondeur (48 kHz stéréo toujours).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AudioFormat {
    pub container: Container,
    pub depth: Depth,
}

/// Les choix de la fenêtre « Son par piste ».
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StemsSettings {
    pub format: AudioFormat,
    /// En plus : le mixage de toutes les pistes qu'on entend.
    pub with_mix: bool,
}

impl Default for StemsSettings {
    fn default() -> Self {
        StemsSettings { format: AudioFormat::default(), with_mix: false }
    }
}

// ----------------------------------------------------------------------------------------- écriture

/// En-tête WAV (PCM, stéréo, 48 kHz) de `frames` échantillons par canal à `depth`.
pub fn wav_header(frames: u64, depth: Depth) -> Vec<u8> {
    let data = frames as u32 * 2 * depth.bytes() as u32;
    let block = 2 * depth.bytes() as u16;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&audio::RATE.to_le_bytes());
    h.extend_from_slice(&(audio::RATE * block as u32).to_le_bytes());
    h.extend_from_slice(&block.to_le_bytes());
    h.extend_from_slice(&depth.bits().to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    h
}

/// 48000 en nombre à virgule flottante étendu 80 bits (la fréquence d'un AIFF s'écrit ainsi).
const RATE_EXTENDED_48K: [u8; 10] = [0x40, 0x0E, 0xBB, 0x80, 0, 0, 0, 0, 0, 0];

/// En-tête AIFF (`FORM`, `COMM`, début de `SSND`) de `frames` échantillons par canal à `depth`.
pub fn aiff_header(frames: u64, depth: Depth) -> Vec<u8> {
    let data = frames as u32 * 2 * depth.bytes() as u32;
    let mut h = Vec::with_capacity(54);
    h.extend_from_slice(b"FORM");
    // « AIFF » (4) + COMM (8 + 18) + SSND (8 + 8 + données).
    h.extend_from_slice(&(4 + 26 + 16 + data).to_be_bytes());
    h.extend_from_slice(b"AIFFCOMM");
    h.extend_from_slice(&18u32.to_be_bytes());
    h.extend_from_slice(&2u16.to_be_bytes());
    h.extend_from_slice(&(frames as u32).to_be_bytes());
    h.extend_from_slice(&depth.bits().to_be_bytes());
    h.extend_from_slice(&RATE_EXTENDED_48K);
    h.extend_from_slice(b"SSND");
    h.extend_from_slice(&(8 + data).to_be_bytes());
    h.extend_from_slice(&0u32.to_be_bytes());
    h.extend_from_slice(&0u32.to_be_bytes());
    h
}

/// Un échantillon (entre -1 et 1) en entier signé de `depth` : 32767 ou 8388607 pour 1.
fn quantize(s: f32, depth: Depth) -> i32 {
    let max = match depth {
        Depth::Bits16 => 32767.0,
        Depth::Bits24 => 8_388_607.0,
    };
    (s.clamp(-1.0, 1.0) * max).round() as i32
}

/// Mixe `clips` entre `start` et `start + len` (échantillons) et l'écrit en `format` sous `path` (d'abord sous `<path>.part`, puis
/// renommé). `cancel` est regardé entre deux morceaux : vrai, le fichier est effacé et la fonction rend `false`.
pub fn write_audio_until(path: &Path, clips: Vec<MixClip>, start: u64, len: u64, format: AudioFormat, cancel: Option<&AtomicBool>) -> std::io::Result<bool> {
    let part = crate::export::part_of(path);
    let result = (|| -> std::io::Result<bool> {
        let mut f = BufWriter::new(std::fs::File::create(&part)?);
        f.write_all(&match format.container {
            Container::Wav => wav_header(len, format.depth),
            Container::Aiff => aiff_header(len, format.depth),
        })?;
        let mut mix = MixState { clips, playing: true, pos: start as f64, range: (start, start + len), looping: false, scrub: None };
        let mut buf = vec![0f32; 2 * 4800];
        let mut bytes = Vec::with_capacity(6 * 4800);
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
                let v = quantize(*s, format.depth);
                match (format.container, format.depth) {
                    (Container::Wav, Depth::Bits16) => bytes.extend_from_slice(&(v as i16).to_le_bytes()),
                    (Container::Wav, Depth::Bits24) => bytes.extend_from_slice(&v.to_le_bytes()[..3]),
                    (Container::Aiff, Depth::Bits16) => bytes.extend_from_slice(&(v as i16).to_be_bytes()),
                    (Container::Aiff, Depth::Bits24) => bytes.extend_from_slice(&v.to_be_bytes()[1..]),
                }
            }
            f.write_all(&bytes)?;
            left -= n as u64;
        }
        f.flush()?;
        Ok(true)
    })();
    match result {
        Ok(true) => std::fs::rename(&part, path).map(|()| true).inspect_err(|_| {
            let _ = std::fs::remove_file(&part);
        }),
        other => {
            let _ = std::fs::remove_file(&part);
            other
        }
    }
}

// ------------------------------------------------------------------------------------------ le plan

/// Un fichier à écrire : la piste (rang dans `Document::audio_tracks`, rien pour le mixage) et son nom de fichier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StemFile {
    pub track: Option<usize>,
    pub name: String,
    pub file: String,
}

/// Un nom de fichier fait d'un nom de piste : les accents sont gardés, les caractères interdits de Windows (`<>:"/\|?*` et les
/// commandes) deviennent « _ », sans point ni blanc à la fin ; à défaut `fallback`.
pub fn file_name_of(name: &str, fallback: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_string();
    let upper = cleaned.to_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (upper.len() == 4 && (upper.starts_with("COM") || upper.starts_with("LPT")) && upper.ends_with(|c: char| c.is_ascii_digit()));
    if cleaned.is_empty() || reserved {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// Les fichiers du son par piste pour les images `first .. first + frames` du montage : une piste qu'on entend et qui a un extrait dans
/// l'étendue donne un fichier ; le mixage vient en dernier si `s.with_mix` (`base` : le nom du projet). Deux noms jamais identiques.
pub fn plan(doc: &Document, first: usize, frames: usize, s: &StemsSettings, base: &str) -> Vec<StemFile> {
    let (f0, f1) = (first, first + frames);
    let ext = s.format.container.extension();
    let mut out: Vec<StemFile> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    let mut unique = |name: String| -> String {
        let mut candidate = name.clone();
        let mut n = 1;
        while used.iter().any(|u| u.to_lowercase() == candidate.to_lowercase()) {
            n += 1;
            candidate = format!("{name}_{n}");
        }
        used.push(candidate.clone());
        candidate
    };
    for (i, t) in doc.audio_tracks.iter().enumerate() {
        if !audiofx::audible(doc, t) {
            continue;
        }
        let sounds = t.clips.iter().any(|c| c.length > 0 && c.start < f1 && c.end() > f0 && doc.sounds.contains_key(&c.sound));
        if !sounds {
            continue;
        }
        let stem = unique(file_name_of(&t.name, &format!("Piste {}", i + 1)));
        out.push(StemFile { track: Some(i), name: t.name.clone(), file: format!("{stem}.{ext}") });
    }
    if s.with_mix && !out.is_empty() {
        let stem = unique(file_name_of(&format!("{base}_mix"), "Mix"));
        out.push(StemFile { track: None, name: "Mixage".into(), file: format!("{stem}.{ext}") });
    }
    out
}

/// Écrit les fichiers `files` dans `dir`, pour les échantillons `start .. start + len` ; rend le message de réussite. Les fichiers déjà
/// écrits sont effacés si l'on annule ou si un fichier ne s'écrit pas.
pub fn write_all(doc: &Document, dir: &Path, files: &[StemFile], start: u64, len: u64, format: AudioFormat, cancel: &AtomicBool) -> Result<Vec<PathBuf>, String> {
    let mut written: Vec<PathBuf> = Vec::new();
    for f in files {
        if cancel.load(Ordering::Relaxed) {
            written.iter().for_each(|p| drop(std::fs::remove_file(p)));
            return Err("annulé".into());
        }
        let clips = match f.track {
            Some(i) => audiofx::build_mix_where(doc, true, |k, _| k == i),
            None => audio::mix_clips_blocking(doc),
        };
        let path = dir.join(&f.file);
        match write_audio_until(&path, clips, start, len, format, Some(cancel)) {
            Ok(true) => written.push(path),
            Ok(false) => {
                written.iter().for_each(|p| drop(std::fs::remove_file(p)));
                return Err("annulé".into());
            }
            Err(e) => {
                written.iter().for_each(|p| drop(std::fs::remove_file(p)));
                return Err(format!("{} : {e}", path.display()));
            }
        }
    }
    Ok(written)
}
