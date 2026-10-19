//! Son : décodage des fichiers (WAV, MP3, FLAC, OGG), formes d'onde, et lecture.
//!
//! Tous les sons sont convertis à l'ouverture en stéréo à `RATE`. Le mixeur tourne dans le fil
//! de la carte son ; sa position sert d'horloge à la lecture de l'image, ce qui garde le son
//! et l'image synchronisés.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::document::Document;

/// Fréquence interne de tous les sons.
pub const RATE: u32 = 48_000;
/// Nombre d'échantillons résumés par une valeur de forme d'onde (200 valeurs par seconde).
pub const PEAK_BLOCK: usize = 240;

/// Son décodé : stéréo entrelacé à `RATE`, en 16 bits (moitié moins de mémoire qu'en flottant,
/// qualité d'un CD), avec sa forme d'onde résumée.
pub struct Pcm {
    pub data: Vec<i16>,
    /// Amplitude maximale (0 à 1) par bloc de `PEAK_BLOCK` échantillons.
    pub peaks: Vec<f32>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.data.len() / 2
    }

    /// Échantillon `i` (gauche, droite), entre -1 et 1.
    pub fn frame(&self, i: usize) -> (f32, f32) {
        const K: f32 = 1.0 / 32768.0;
        (self.data[2 * i] as f32 * K, self.data[2 * i + 1] as f32 * K)
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / RATE as f64
    }

    /// Mise au format interne : stéréo, `RATE` (interpolation linéaire).
    pub fn from_interleaved(src: &[f32], channels: usize, rate: u32) -> Pcm {
        let ch = channels.max(1);
        let n = src.len() / ch;
        let rate = rate.max(1);
        let at = |i: usize| -> (f32, f32) {
            let b = i.min(n.saturating_sub(1)) * ch;
            if ch == 1 {
                (src[b], src[b])
            } else {
                (src[b], src[b + 1])
            }
        };
        let out_n = if n == 0 {
            0
        } else if rate == RATE {
            n
        } else {
            ((n as u64 * RATE as u64) / rate as u64) as usize
        };
        let to_i16 = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        let mut data = Vec::with_capacity(out_n * 2);
        for j in 0..out_n {
            let t = j as f64 * rate as f64 / RATE as f64;
            let i0 = t.floor() as usize;
            let frac = (t - i0 as f64) as f32;
            let (l0, r0) = at(i0);
            let (l1, r1) = at(i0 + 1);
            data.push(to_i16(l0 + (l1 - l0) * frac));
            data.push(to_i16(r0 + (r1 - r0) * frac));
        }
        let peaks = data
            .chunks(PEAK_BLOCK * 2)
            .map(|c| (c.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0) as f32 / 32767.0).min(1.0))
            .collect();
        Pcm { data, peaks }
    }

    /// Amplitude maximale entre deux positions (en échantillons).
    pub fn peak_between(&self, from: usize, to: usize) -> f32 {
        let a = from / PEAK_BLOCK;
        let b = (to.max(from + 1)).div_ceil(PEAK_BLOCK).min(self.peaks.len());
        self.peaks.get(a..b).map_or(0.0, |s| s.iter().fold(0.0f32, |m, &v| m.max(v)))
    }
}

/// Décode un fichier son complet (`ext` aide à reconnaître le format).
pub fn decode(bytes: &[u8], ext: &str) -> Result<Pcm, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("format de son non reconnu ({e})"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("aucune piste audio dans ce fichier")?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(RATE);
    let mut channels = track.codec_params.channels.map_or(2, |c| c.count());
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("codec non géré ({e})"))?;
    let mut samples: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("lecture du son interrompue ({e})")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                rate = spec.rate;
                channels = spec.channels.count();
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                samples.extend_from_slice(sb.samples());
            }
            // Un paquet abîmé : on le saute, comme les lecteurs habituels.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("décodage impossible ({e})")),
        }
    }
    if samples.is_empty() {
        return Err("le fichier ne contient aucun son".into());
    }
    Ok(Pcm::from_interleaved(&samples, channels, rate))
}

// ---------------------------------------------------------------------------------- mixage

/// Un extrait prêt à jouer, en échantillons (à `RATE`) depuis le début du montage.
#[derive(Clone)]
pub struct MixClip {
    pub pcm: Arc<Pcm>,
    pub start: u64,
    /// Échantillons sautés au début du son.
    pub offset: u64,
    pub len: u64,
    /// Volume de la piste × volume général × volume de l'extrait.
    pub gain: f32,
    /// Fondus, enchaîné et clés de volume (`audiofx.rs`) ; rien : le son tel quel (le plus courant, et
    /// le plus rapide à mixer).
    pub env: Option<Arc<ClipEnv>>,
}

impl MixClip {
    /// Un extrait sans effet (essais).
    #[cfg(test)]
    pub fn plain(pcm: Arc<Pcm>, start: u64, offset: u64, len: u64, gain: f32) -> MixClip {
        MixClip { pcm, start, offset, len, gain, env: None }
    }

    /// Le gain à l'échantillon `rel` depuis le début de l'extrait (le volume fixe et l'enveloppe).
    #[inline]
    pub fn gain_at(&self, rel: u64) -> f32 {
        match &self.env {
            Some(env) => self.gain * env.gain_at(rel),
            None => self.gain,
        }
    }
}

/// Ce qui fait varier le volume d'un extrait dans le temps, en échantillons depuis son début (qui est
/// avancé de `lead` échantillons quand un fondu enchaîné commence avant la coupe). Le calcul est pur :
/// la lecture, le défilement et l'export l'appellent de la même façon.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipEnv {
    /// Échantillons du début de l'`MixClip` qui précèdent le début nominal de l'extrait.
    pub lead: u64,
    /// Durée nominale de l'extrait.
    pub nominal: u64,
    /// Fondus linéaires, comptés depuis le début et la fin nominaux (0 : aucun).
    pub fade_in: u64,
    pub fade_out: u64,
    /// Fondu enchaîné entrant : puissance égale sur les `xf_in` premiers échantillons.
    pub xf_in: u64,
    /// Fondu enchaîné sortant : à partir de l'échantillon `xf_out_at`, sur `xf_out` échantillons.
    pub xf_out_at: u64,
    pub xf_out: u64,
    /// Clés de volume : (échantillons depuis le début nominal, gain), triées.
    pub keys: Vec<(f64, f32)>,
}

impl ClipEnv {
    pub fn gain_at(&self, rel: u64) -> f32 {
        // Temps depuis le début nominal (négatif pendant l'avance d'un enchaîné).
        let t = rel as f64 - self.lead as f64;
        let mut g = 1.0f32;
        if self.fade_in > 0 {
            g *= crate::audiofx::fade_in_gain(t, self.fade_in);
        }
        if self.fade_out > 0 {
            g *= crate::audiofx::fade_out_gain(self.nominal as f64 - t, self.fade_out);
        }
        if self.xf_in > 0 && rel < self.xf_in {
            g *= crate::audiofx::crossfade_gains(rel as f64 / self.xf_in as f64).1;
        }
        if self.xf_out > 0 && rel >= self.xf_out_at {
            g *= crate::audiofx::crossfade_gains((rel - self.xf_out_at) as f64 / self.xf_out as f64).0;
        }
        if !self.keys.is_empty() {
            g *= crate::audiofx::envelope_at(&self.keys, t);
        }
        g
    }
}

/// Échantillons par image du film.
pub fn samples_per_frame(fps: u32) -> f64 {
    RATE as f64 / fps.max(1) as f64
}

/// Les extraits des pistes qu'on entend (muettes et solo : `audiofx::audible`), avec leurs effets.
/// Ne bloque jamais : un égaliseur ou un écho pas encore calculé (sur un fil à part) laisse entendre
/// le son tel quel en attendant.
pub fn mix_clips(doc: &Document) -> Vec<MixClip> {
    crate::audiofx::build_mix(doc, false)
}

/// Comme `mix_clips`, mais attend le résultat des effets calculés : pour l'export et les essais, où
/// le son doit être exact.
pub fn mix_clips_blocking(doc: &Document) -> Vec<MixClip> {
    crate::audiofx::build_mix(doc, true)
}

/// État partagé avec le fil de la carte son.
#[derive(Default)]
pub struct MixState {
    pub clips: Vec<MixClip>,
    pub playing: bool,
    /// Position de lecture, en échantillons à `RATE`.
    pub pos: f64,
    /// Lecture de `range.0` à `range.1` (exclu).
    pub range: (u64, u64),
    pub looping: bool,
    /// Court extrait à jouer une fois (son au défilement) : position et fin.
    pub scrub: Option<(f64, f64)>,
}

impl MixState {
    /// Remplit `out` (stéréo entrelacé). `step` = échantillons internes par échantillon de sortie.
    pub fn render(&mut self, out: &mut [f32], step: f64) {
        for frame in out.chunks_exact_mut(2) {
            let (l, r) = if self.playing {
                let s = self.sample_at(self.pos);
                self.pos += step;
                let (start, end) = (self.range.0 as f64, self.range.1 as f64);
                if self.pos >= end {
                    if self.looping && end > start {
                        self.pos = start + (self.pos - end) % (end - start);
                    } else {
                        self.playing = false;
                        self.pos = end;
                    }
                }
                s
            } else if let Some((p, end)) = self.scrub {
                let s = self.sample_at(p);
                let next = p + step;
                self.scrub = (next < end).then_some((next, end));
                s
            } else {
                (0.0, 0.0)
            };
            frame[0] = l;
            frame[1] = r;
        }
    }

    fn sample_at(&self, pos: f64) -> (f32, f32) {
        let p = pos.max(0.0) as u64;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for c in &self.clips {
            if p >= c.start && p < c.start + c.len {
                let i = (p - c.start + c.offset) as usize;
                if i < c.pcm.frames() {
                    let (a, b) = c.pcm.frame(i);
                    let g = c.gain_at(p - c.start);
                    l += a * g;
                    r += b * g;
                }
            }
        }
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }
}

/// Sortie audio du système. Si elle est absente ou tombe en panne, la lecture continue sans son,
/// avec l'horloge de l'ordinateur.
pub struct AudioEngine {
    state: Arc<Mutex<MixState>>,
    failed: Arc<AtomicBool>,
    _stream: cpal::Stream,
}

impl AudioEngine {
    pub fn start() -> Result<Self, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("aucune sortie audio")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let state = Arc::new(Mutex::new(MixState::default()));
        let failed = Arc::new(AtomicBool::new(false));
        let format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let stream = match format {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config, channels, rate, state.clone(), failed.clone()),
            cpal::SampleFormat::I16 => build::<i16>(&device, &config, channels, rate, state.clone(), failed.clone()),
            cpal::SampleFormat::U16 => build::<u16>(&device, &config, channels, rate, state.clone(), failed.clone()),
            other => Err(format!("format de sortie audio non géré ({other:?})")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            state,
            failed,
            _stream: stream,
        })
    }

    pub fn ok(&self) -> bool {
        !self.failed.load(Ordering::Relaxed)
    }

    fn with<R>(&self, f: impl FnOnce(&mut MixState) -> R) -> Option<R> {
        self.state.lock().ok().map(|mut s| f(&mut s))
    }

    pub fn play(&self, clips: Vec<MixClip>, from: u64, range: (u64, u64), looping: bool) {
        self.with(|s| {
            s.clips = clips;
            s.pos = from as f64;
            s.range = range;
            s.looping = looping;
            s.scrub = None;
            s.playing = range.1 > range.0;
        });
    }

    pub fn stop(&self) {
        self.with(|s| s.playing = false);
    }

    /// Suit les changements faits pendant la lecture (extraits déplacés, piste muette, boucle).
    pub fn update(&self, clips: Vec<MixClip>, looping: bool) {
        self.with(|s| {
            s.clips = clips;
            s.looping = looping;
        });
    }

    pub fn is_playing(&self) -> bool {
        self.with(|s| s.playing).unwrap_or(false)
    }

    pub fn position(&self) -> f64 {
        self.with(|s| s.pos).unwrap_or(0.0)
    }

    /// Joue une fois le son de `len` échantillons à partir de `from` (son au défilement).
    pub fn scrub(&self, clips: Vec<MixClip>, from: u64, len: u64) {
        self.with(|s| {
            if !s.playing {
                s.clips = clips;
                s.scrub = Some((from as f64, (from + len) as f64));
            }
        });
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    rate: u32,
    state: Arc<Mutex<MixState>>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;
    let step = RATE as f64 / rate.max(1) as f64;
    let channels = channels.max(1);
    let mut buf: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels;
                buf.resize(frames * 2, 0.0);
                match state.lock() {
                    Ok(mut s) => s.render(&mut buf, step),
                    Err(_) => buf.fill(0.0),
                }
                for (i, frame) in data.chunks_exact_mut(channels).enumerate() {
                    let (l, r) = (buf[2 * i], buf[2 * i + 1]);
                    for (c, out) in frame.iter_mut().enumerate() {
                        let v = match (channels, c) {
                            (1, _) => (l + r) * 0.5,
                            (_, 0) => l,
                            (_, 1) => r,
                            _ => 0.0,
                        };
                        *out = T::from_sample(v);
                    }
                }
            },
            move |err| {
                failed.store(true, Ordering::Relaxed);
                eprintln!("sortie audio : {err}");
            },
            None,
        )
        .map_err(|e| e.to_string())
}
