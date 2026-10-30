//! Effets sonores des extraits et des pistes (Anthony, 2026-10-19) : fondus d'entrée et de sortie,
//! fondu enchaîné entre deux extraits, volume par extrait, clés de volume, égaliseur à trois bandes
//! et écho ; solo des pistes.
//!
//! # Où ils agissent
//!
//! Un seul endroit construit ce qu'on entend : `build_mix`, qui transforme les pistes du document en
//! `MixClip` (voir `audio.rs`). La lecture, le son au défilement, le WAV du film et le son des
//! exports vidéo l'appellent tous : les effets y sont donc identiques. Le fil de la carte son ne
//! calcule que des gains (`ClipEnv::gain_at`, quelques multiplications par échantillon) ; le travail
//! lourd (égaliseur, écho) est fait **hors** de ce fil et hors du fil de l'interface :
//!
//! - l'égaliseur et l'écho refont le son de l'extrait en entier (`process`), sur un fil à part
//!   (`processed`), et le résultat est gardé (24 au plus) ; en attendant, on entend le son tel quel ;
//! - l'export et les essais attendent le résultat (`build_mix(doc, true)`) : le fichier est exact.
//!
//! # Modèle de temps
//!
//! Les durées des fondus et les clés sont en **images** du montage (comme tout le reste), comptées
//! depuis le début de l'extrait. Le fondu d'entrée / de sortie est linéaire en amplitude ; le fondu
//! enchaîné est en puissance égale (sinus / cosinus : le niveau perçu ne creuse pas au milieu).
//!
//! Les extraits d'une piste ne se chevauchent pas (`Document::track_free`). Le fondu enchaîné se
//! pose donc **sur la jonction de deux extraits qui se touchent** : il est centré sur la jonction et
//! utilise le son que chaque extrait a de l'autre côté de sa coupe (la « poignée » : le son existe au-delà
//! du bord d'un extrait raccourci). Sans poignée, le fondu se raccourcit, jusqu'à disparaître (coupe
//! sèche). L'écho prolonge un extrait qui va jusqu'au bout de son son (la queue de l'écho sonne
//! après la fin de l'extrait).
//!
//! # Fichier
//!
//! `effets_son.json` à côté du manifeste, sans changer `VERSION` (même principe que `transitions.json`),
//! rangé par numéro d'extrait et de piste. Les effets sont dans `SoundClip::fx` et `AudioTrack::solo`
//! (donc dans `Structure` : l'annulation les rétablit).

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::audio::{samples_per_frame, ClipEnv, MixClip, Pcm, RATE};
use crate::document::{AudioTrack, Document, SoundClip};

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "effets_son.json";
const FORMAT: &str = "shot-effets-son";
const VERSION: u32 = 1;
/// Gain maximal d'un extrait (+12 dB).
pub const MAX_GAIN: f32 = 4.0;
/// Réglage de chaque bande de l'égaliseur, en dB (de −12 à +12).
pub const EQ_RANGE: f32 = 12.0;
/// Fondu enchaîné posé par la commande, en images.
pub const DEFAULT_XFADE: usize = 12;
/// Plus longue queue d'écho, en secondes.
const MAX_TAIL: f32 = 4.0;
/// Fréquences de l'égaliseur : basses (étagère), médiums (cloche), aigus (étagère).
const EQ_FREQS: [f64; 3] = [200.0, 1000.0, 4000.0];

// ------------------------------------------------------------------------------------ modèle

/// Une clé de volume : à `t` images du début de l'extrait, le gain vaut `v` (1 = inchangé).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeKey {
    pub t: f32,
    pub v: f32,
}

/// Écho : le son revient `delay_ms` plus tard, `feedback` fois moins fort à chaque retour, mêlé au son
/// sec à raison de `mix` (0 : pas d'écho).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Echo {
    #[serde(rename = "niveau")]
    pub mix: f32,
    #[serde(rename = "delai_ms")]
    pub delay_ms: f32,
    #[serde(rename = "retour")]
    pub feedback: f32,
}

impl Default for Echo {
    fn default() -> Self {
        Echo { mix: 0.0, delay_ms: 250.0, feedback: 0.35 }
    }
}

/// Les effets d'un extrait.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipFx {
    /// Volume de l'extrait (1 = inchangé).
    pub gain: f32,
    /// Fondu d'entrée et de sortie, en images.
    pub fade_in: usize,
    pub fade_out: usize,
    /// Fondu enchaîné avec l'extrait suivant (qui doit le toucher), en images ; 0 : coupe sèche.
    pub xfade: usize,
    /// Clés de volume, triées par `t`.
    pub keys: Vec<VolumeKey>,
    /// Égaliseur : basses, médiums, aigus, en dB.
    pub eq: [f32; 3],
    pub echo: Echo,
}

impl Default for ClipFx {
    fn default() -> Self {
        ClipFx { gain: 1.0, fade_in: 0, fade_out: 0, xfade: 0, keys: Vec::new(), eq: [0.0; 3], echo: Echo::default() }
    }
}

impl ClipFx {
    /// Aucun réglage (le fichier n'écrit rien pour cet extrait).
    pub fn is_default(&self) -> bool {
        *self == ClipFx::default()
    }

    /// Ni égaliseur ni écho : le son de l'extrait n'est pas refait.
    pub fn is_dry(&self) -> bool {
        self.eq.iter().all(|g| g.abs() < 0.05) && self.echo.mix < 0.005
    }

    /// Ramène chaque valeur dans ses bornes (un fichier abîmé, un champ tapé n'importe comment).
    pub fn sanitize(&mut self) {
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        self.gain = finite(self.gain, 1.0).clamp(0.0, MAX_GAIN);
        self.eq = self.eq.map(|g| finite(g, 0.0).clamp(-EQ_RANGE, EQ_RANGE));
        self.echo.mix = finite(self.echo.mix, 0.0).clamp(0.0, 1.0);
        self.echo.delay_ms = finite(self.echo.delay_ms, 250.0).clamp(20.0, 1500.0);
        self.echo.feedback = finite(self.echo.feedback, 0.35).clamp(0.0, 0.9);
        self.fade_in = self.fade_in.min(100_000);
        self.fade_out = self.fade_out.min(100_000);
        self.xfade = self.xfade.min(100_000);
        self.keys.retain(|k| k.t.is_finite() && k.v.is_finite());
        for k in &mut self.keys {
            k.t = k.t.clamp(0.0, 1.0e6);
            k.v = k.v.clamp(0.0, MAX_GAIN);
        }
        self.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        self.keys.truncate(512);
    }

    /// Le gain donné par les clés à `t` images (linéaire entre deux clés, tenu avant la première et
    /// après la dernière ; 1 sans clé).
    pub fn key_gain(&self, t: f32) -> f32 {
        let pts: Vec<(f64, f32)> = self.keys.iter().map(|k| (k.t as f64, k.v)).collect();
        envelope_at(&pts, t as f64)
    }

    /// Pose (ou déplace, si une clé est déjà à ce `t`) une clé. Renvoie son rang.
    pub fn set_key(&mut self, t: f32, v: f32) -> usize {
        let key = VolumeKey { t: t.max(0.0), v: v.clamp(0.0, MAX_GAIN) };
        if let Some(i) = self.keys.iter().position(|k| (k.t - key.t).abs() < 1e-3) {
            self.keys[i] = key;
            return i;
        }
        let at = self.keys.partition_point(|k| k.t < key.t);
        self.keys.insert(at, key);
        at
    }

    /// Déplace la clé `i` ; elle reste entre ses voisines (l'ordre des clés ne change pas).
    pub fn move_key(&mut self, i: usize, t: f32, v: f32) {
        if i >= self.keys.len() {
            return;
        }
        let lo = if i > 0 { self.keys[i - 1].t + 0.01 } else { 0.0 };
        let hi = if i + 1 < self.keys.len() { self.keys[i + 1].t - 0.01 } else { f32::MAX };
        self.keys[i] = VolumeKey { t: t.clamp(lo, hi.max(lo)), v: v.clamp(0.0, MAX_GAIN) };
    }
}

/// Le gain d'une courbe de clés (`(temps, gain)` triés) au temps `t` : linéaire entre deux clés, tenu
/// avant la première et après la dernière ; 1 s'il n'y a aucune clé.
pub fn envelope_at(keys: &[(f64, f32)], t: f64) -> f32 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return 1.0 };
    if t <= first.0 {
        return first.1;
    }
    if t >= last.0 {
        return last.1;
    }
    let i = keys.partition_point(|k| k.0 <= t);
    let (a, b) = (keys[i - 1], keys[i]);
    if b.0 <= a.0 {
        return b.1;
    }
    let u = ((t - a.0) / (b.0 - a.0)) as f32;
    a.1 + (b.1 - a.1) * u
}

/// Fondu d'entrée linéaire : 0 au début, 1 après `len` échantillons (1 sans fondu).
pub fn fade_in_gain(t: f64, len: u64) -> f32 {
    if len == 0 || t >= len as f64 {
        1.0
    } else {
        (t.max(0.0) / len as f64) as f32
    }
}

/// Fondu de sortie linéaire : `remaining` échantillons avant la fin de l'extrait.
pub fn fade_out_gain(remaining: f64, len: u64) -> f32 {
    fade_in_gain(remaining, len)
}

/// Fondu enchaîné en puissance égale : `u` de 0 à 1 ; l'entrant monte en sinus, le sortant descend en
/// cosinus (la somme des carrés vaut 1).
pub fn crossfade_gains(u: f64) -> (f32, f32) {
    let a = u.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2;
    (a.cos() as f32, a.sin() as f32)
}

/// Le niveau que les effets d'un extrait donnent à l'image `t` (depuis son début) : volume, fondus et
/// clés, pour la feuille d'exposition (le niveau du son image par image).
pub fn level_in_frame(fx: &ClipFx, t: f32, length: usize) -> f32 {
    let t = t + 0.5;
    let fade_in = if fx.fade_in > 0 && t < fx.fade_in as f32 { t / fx.fade_in as f32 } else { 1.0 };
    let remaining = length as f32 - t;
    let fade_out = if fx.fade_out > 0 && remaining < fx.fade_out as f32 { remaining.max(0.0) / fx.fade_out as f32 } else { 1.0 };
    fx.gain * fade_in * fade_out * fx.key_gain(t)
}

// ---------------------------------------------------------------------------- son : traitement

/// Filtre du second ordre (formes de R. Bristow-Johnson, « Audio EQ Cookbook »).
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 3]) -> Biquad {
        Biquad { b: [b[0] / a[0], b[1] / a[0], b[2] / a[0]], a: [a[1] / a[0], a[2] / a[0]], z: [0.0; 2] }
    }

    /// Étagère des graves (`low`) ou des aigus, de `db` dB à partir de `f0`.
    fn shelf(low: bool, fs: f64, f0: f64, db: f64) -> Biquad {
        let a = 10f64.powf(db / 40.0);
        let w = 2.0 * std::f64::consts::PI * f0 / fs;
        let (sin, cos) = w.sin_cos();
        // Pente 1 : alpha = sin(w) / 2 * sqrt(2).
        let alpha = sin / 2.0 * 2f64.sqrt();
        let k = 2.0 * a.sqrt() * alpha;
        if low {
            Biquad::new(
                [a * ((a + 1.0) - (a - 1.0) * cos + k), 2.0 * a * ((a - 1.0) - (a + 1.0) * cos), a * ((a + 1.0) - (a - 1.0) * cos - k)],
                [(a + 1.0) + (a - 1.0) * cos + k, -2.0 * ((a - 1.0) + (a + 1.0) * cos), (a + 1.0) + (a - 1.0) * cos - k],
            )
        } else {
            Biquad::new(
                [a * ((a + 1.0) + (a - 1.0) * cos + k), -2.0 * a * ((a - 1.0) + (a + 1.0) * cos), a * ((a + 1.0) + (a - 1.0) * cos - k)],
                [(a + 1.0) - (a - 1.0) * cos + k, 2.0 * ((a - 1.0) - (a + 1.0) * cos), (a + 1.0) - (a - 1.0) * cos - k],
            )
        }
    }

    /// Cloche autour de `f0`, de `db` dB.
    fn peak(fs: f64, f0: f64, db: f64, q: f64) -> Biquad {
        let a = 10f64.powf(db / 40.0);
        let w = 2.0 * std::f64::consts::PI * f0 / fs;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        Biquad::new([1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a])
    }

    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        // Forme transposée II.
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// Le son refait avec l'égaliseur puis l'écho de `fx`. Plus long que l'original de la queue de l'écho.
/// Calcul pur, sans temps réel : c'est le même pour l'écoute et pour l'export.
pub fn process(pcm: &Pcm, fx: &ClipFx) -> Pcm {
    let n = pcm.frames();
    let fs = RATE as f64;
    // Les canaux, en flottants.
    let mut chans: [Vec<f32>; 2] = [Vec::with_capacity(n), Vec::with_capacity(n)];
    for i in 0..n {
        let (l, r) = pcm.frame(i);
        chans[0].push(l);
        chans[1].push(r);
    }
    // Égaliseur : trois filtres en série par canal (une bande à 0 dB est sautée).
    let filters = || -> Vec<Biquad> {
        let mut v = Vec::new();
        for (band, &db) in fx.eq.iter().enumerate() {
            if db.abs() < 0.05 {
                continue;
            }
            let db = db as f64;
            v.push(match band {
                0 => Biquad::shelf(true, fs, EQ_FREQS[0], db),
                1 => Biquad::peak(fs, EQ_FREQS[1], db, 0.8),
                _ => Biquad::shelf(false, fs, EQ_FREQS[2], db),
            });
        }
        v
    };
    for data in chans.iter_mut() {
        let mut fl = filters();
        if fl.is_empty() {
            continue;
        }
        for s in data.iter_mut() {
            let mut x = *s as f64;
            for f in fl.iter_mut() {
                x = f.run(x);
            }
            *s = x as f32;
        }
    }
    // Écho : un retour retardé, avec contre-réaction, mêlé au son sec.
    let echo = fx.echo;
    let mut tail = 0;
    if echo.mix >= 0.005 {
        let delay = ((echo.delay_ms.clamp(20.0, 1500.0) / 1000.0) * RATE as f32) as usize;
        let fb = echo.feedback.clamp(0.0, 0.9);
        // Retours audibles : jusqu'à −60 dB, au plus MAX_TAIL secondes.
        let repeats = if fb < 0.01 { 1 } else { ((0.001f32.ln() / fb.ln()).ceil() as usize).max(1) };
        tail = (delay * repeats).min((MAX_TAIL * RATE as f32) as usize);
        for data in chans.iter_mut() {
            data.resize(n + tail, 0.0);
            echo_channel(data, delay, fb, echo.mix);
        }
    }
    let mut flat = Vec::with_capacity((n + tail) * 2);
    for i in 0..n + tail {
        flat.push(chans[0][i]);
        flat.push(chans[1][i]);
    }
    Pcm::from_interleaved(&flat, 2, RATE)
}

/// L'écho d'un canal, sur place : `data` est le son sec (suivi de silence pour la queue).
/// `y[i] = sec[i] + niveau × w[i]`, `w[i] = sec[i − D] + retour × w[i − D]`.
fn echo_channel(data: &mut [f32], delay: usize, feedback: f32, mix: f32) {
    let dry = data.to_vec();
    let mut wet = vec![0.0f32; data.len()];
    for i in 0..data.len() {
        let (d, w) = if i >= delay { (dry[i - delay], wet[i - delay]) } else { (0.0, 0.0) };
        wet[i] = d + feedback * w;
        data[i] = dry[i] + mix * wet[i];
    }
}

// ------------------------------------------------------- son : résultats gardés (hors des fils)

struct Slot {
    src: Arc<Pcm>,
    key: [u32; 6],
    state: SlotState,
}

enum SlotState {
    Pending,
    Ready(Arc<Pcm>),
}

/// Les sons refaits, du plus ancien au plus récent.
static CACHE: Mutex<Vec<Slot>> = Mutex::new(Vec::new());
const CACHE_LEN: usize = 24;

fn cache_key(fx: &ClipFx) -> [u32; 6] {
    [fx.eq[0].to_bits(), fx.eq[1].to_bits(), fx.eq[2].to_bits(), fx.echo.mix.to_bits(), fx.echo.delay_ms.to_bits(), fx.echo.feedback.to_bits()]
}

/// Le son de l'extrait avec son égaliseur et son écho. Sans réglage, le son lui-même. `wait` faux
/// (lecture) : si le résultat n'est pas encore prêt, il se calcule sur un fil à part et on rend le son
/// tel quel en attendant ; `wait` vrai (export, essais) : on attend le résultat.
pub fn processed(pcm: &Arc<Pcm>, fx: &ClipFx, wait: bool) -> Arc<Pcm> {
    if fx.is_dry() {
        return pcm.clone();
    }
    let key = cache_key(fx);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(slot) = cache.iter().find(|s| Arc::ptr_eq(&s.src, pcm) && s.key == key) {
        if let SlotState::Ready(done) = &slot.state {
            return done.clone();
        }
        if !wait {
            return pcm.clone();
        }
    }
    if wait {
        drop(cache);
        let done = Arc::new(process(pcm, fx));
        store(pcm, key, done.clone());
        return done;
    }
    // Un seul calcul à la fois par son : on ne lance pas un fil par valeur d'un curseur tiré ; la
    // valeur finale est demandée à l'image suivante.
    if cache.iter().any(|s| Arc::ptr_eq(&s.src, pcm) && matches!(s.state, SlotState::Pending)) {
        return pcm.clone();
    }
    cache.push(Slot { src: pcm.clone(), key, state: SlotState::Pending });
    if cache.len() > CACHE_LEN {
        if let Some(old) = cache.iter().position(|s| matches!(s.state, SlotState::Ready(_))) {
            cache.remove(old);
        }
    }
    drop(cache);
    let (src, fx) = (pcm.clone(), fx.clone());
    std::thread::spawn(move || {
        let done = Arc::new(process(&src, &fx));
        store(&src, cache_key(&fx), done);
    });
    pcm.clone()
}

fn store(src: &Arc<Pcm>, key: [u32; 6], done: Arc<Pcm>) {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match cache.iter_mut().find(|s| Arc::ptr_eq(&s.src, src) && s.key == key) {
        Some(slot) => slot.state = SlotState::Ready(done),
        None => cache.push(Slot { src: src.clone(), key, state: SlotState::Ready(done) }),
    }
    while cache.len() > CACHE_LEN {
        match cache.iter().position(|s| matches!(s.state, SlotState::Ready(_))) {
            Some(old) => {
                cache.remove(old);
            }
            None => break,
        }
    }
}

/// Le son refait est-il prêt (ou inutile) pour ce réglage ? (Essais.)
#[cfg(test)]
pub fn is_ready(pcm: &Arc<Pcm>, fx: &ClipFx) -> bool {
    if fx.is_dry() {
        return true;
    }
    let key = cache_key(fx);
    let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.iter().any(|s| Arc::ptr_eq(&s.src, pcm) && s.key == key && matches!(s.state, SlotState::Ready(_)))
}

// ------------------------------------------------------------------------- ce qu'on entend

/// Les pistes qu'on entend : les pistes en solo s'il y en a (même muettes), sinon celles qui ne sont
/// pas muettes.
pub fn audible(doc: &Document, track: &AudioTrack) -> bool {
    if doc.audio_tracks.iter().any(|t| t.solo) {
        track.solo
    } else {
        !track.muted
    }
}

/// La poignée du fondu enchaîné entre `a` et `b` (deux extraits qui se touchent sur la même piste),
/// en échantillons de chaque côté de la jonction : la moitié du fondu demandé, réduite par le son qui
/// existe de l'autre côté de chaque coupe et par la durée des extraits. 0 : coupe sèche.
pub fn junction_handle(doc: &Document, a: &SoundClip, b: &SoundClip) -> u64 {
    if a.fx.xfade == 0 || a.end() != b.start || a.length == 0 || b.length == 0 {
        return 0;
    }
    let (Some(sa), Some(_)) = (doc.sounds.get(&a.sound), doc.sounds.get(&b.sound)) else { return 0 };
    let spf = samples_per_frame(doc.fps);
    let wanted = (a.fx.xfade as f64 * spf / 2.0) as u64;
    let a_tail = (sa.pcm.frames() as u64).saturating_sub(((a.offset + a.length) as f64 * spf) as u64);
    let b_head = (b.offset as f64 * spf) as u64;
    let (la, lb) = ((a.length as f64 * spf) as u64, (b.length as f64 * spf) as u64);
    wanted.min(a_tail).min(b_head).min(la).min(lb)
}

/// Le fondu enchaîné réel, en images, du clip `i` de la piste avec le suivant (0 : aucun).
pub fn junction_frames(doc: &Document, track: &AudioTrack, i: usize) -> f64 {
    let (Some(a), Some(b)) = (track.clips.get(i), track.clips.get(i + 1)) else { return 0.0 };
    2.0 * junction_handle(doc, a, b) as f64 / samples_per_frame(doc.fps)
}

/// Ce qu'on entend : les extraits des pistes audibles, effets compris. `wait` : attendre l'égaliseur
/// et l'écho (export, essais) au lieu de les calculer en arrière-plan.
pub fn build_mix(doc: &Document, wait: bool) -> Vec<MixClip> {
    build_mix_where(doc, wait, |_, t| audible(doc, t))
}

/// Comme `build_mix`, pour les pistes que `keep` (rang de la piste, la piste) retient : le son d'une seule piste (`exportstems.rs`),
/// avec son volume, ses fondus, ses clés, son égaliseur et son écho.
pub fn build_mix_where(doc: &Document, wait: bool, keep: impl Fn(usize, &AudioTrack) -> bool) -> Vec<MixClip> {
    let spf = samples_per_frame(doc.fps);
    let mut out = Vec::new();
    for track in doc.audio_tracks.iter().enumerate().filter(|(i, t)| keep(*i, t)).map(|(_, t)| t) {
        let track_gain = track.volume * doc.master_volume;
        let mut order: Vec<usize> = (0..track.clips.len()).collect();
        order.sort_by_key(|&i| track.clips[i].start);
        for (rank, &ci) in order.iter().enumerate() {
            let c = &track.clips[ci];
            let Some(sound) = doc.sounds.get(&c.sound) else { continue };
            let prev = rank.checked_sub(1).map(|r| &track.clips[order[r]]);
            let next = order.get(rank + 1).map(|&n| &track.clips[n]);
            let lead = prev.map_or(0, |p| junction_handle(doc, p, c));
            let tail = next.map_or(0, |n| junction_handle(doc, c, n));
            let pcm = processed(&sound.pcm, &c.fx, wait);
            let start = (c.start as f64 * spf) as u64;
            let offset = (c.offset as f64 * spf) as u64;
            let nominal = (c.length as f64 * spf) as u64;
            let mut len = nominal + lead + tail;
            // L'écho d'un extrait qui va jusqu'au bout de son son : sa queue sonne après la coupe.
            if tail == 0 && pcm.frames() > sound.pcm.frames() && offset + nominal + 2 >= sound.pcm.frames() as u64 {
                len = (pcm.frames() as u64).saturating_sub(offset + lead).max(len);
            }
            let fx = &c.fx;
            let (fade_in, fade_out) = (if lead > 0 { 0 } else { (fx.fade_in as f64 * spf) as u64 }, if tail > 0 { 0 } else { (fx.fade_out as f64 * spf) as u64 });
            let keys: Vec<(f64, f32)> = fx.keys.iter().map(|k| (k.t as f64 * spf, k.v)).collect();
            let env = (lead > 0 || tail > 0 || fade_in > 0 || fade_out > 0 || !keys.is_empty()).then(|| {
                Arc::new(ClipEnv {
                    lead,
                    nominal,
                    fade_in,
                    fade_out,
                    xf_in: 2 * lead,
                    xf_out_at: lead + nominal - tail,
                    xf_out: 2 * tail,
                    keys,
                })
            });
            out.push(MixClip { pcm, start: start.saturating_sub(lead), offset: offset.saturating_sub(lead), len, gain: track_gain * fx.gain, env });
        }
    }
    out
}

/// Les échantillons d'un extrait avec ses effets : gain, fondus, clés, égaliseur et écho ; pas
/// d'enchaîné (chaque extrait est écrit seul, sans recouvrement) ; le volume de la piste est à part.
#[cfg(test)]
pub fn bake_clip(doc: &Document, clip: &SoundClip) -> Vec<[f32; 2]> {
    let Some(sound) = doc.sounds.get(&clip.sound) else { return Vec::new() };
    bake_sound(sound, doc.fps, clip)
}

/// Comme `bake_clip`, sans le document : le son et la cadence suffisent (l'export pour le montage
/// l'appelle sur un fil à part, `editorial.rs`).
pub fn bake_sound(sound: &crate::document::Sound, fps: u32, clip: &SoundClip) -> Vec<[f32; 2]> {
    let spf = samples_per_frame(fps);
    let pcm = processed(&sound.pcm, &clip.fx, true);
    let (from, len) = crate::export::sample_span(fps, clip.offset, clip.length);
    let mut out = crate::stretch::slice(&pcm, from as usize, (from + len) as usize);
    let keys: Vec<(f64, f32)> = clip.fx.keys.iter().map(|k| (k.t as f64 * spf, k.v)).collect();
    let env = ClipEnv {
        lead: 0,
        nominal: len,
        fade_in: (clip.fx.fade_in as f64 * spf) as u64,
        fade_out: (clip.fx.fade_out as f64 * spf) as u64,
        xf_in: 0,
        xf_out_at: 0,
        xf_out: 0,
        keys,
    };
    for (i, s) in out.iter_mut().enumerate() {
        let g = env.gain_at(i as u64) * clip.fx.gain;
        s[0] *= g;
        s[1] *= g;
    }
    out
}

// --------------------------------------------------------------------------------- fichier

#[derive(Serialize, Deserialize, Default)]
struct ClipEntry {
    #[serde(rename = "extrait")]
    clip: u64,
    #[serde(rename = "volume", default = "one", skip_serializing_if = "is_one")]
    gain: f32,
    #[serde(rename = "fondu_entree", default, skip_serializing_if = "is_zero")]
    fade_in: usize,
    #[serde(rename = "fondu_sortie", default, skip_serializing_if = "is_zero")]
    fade_out: usize,
    #[serde(rename = "enchaine", default, skip_serializing_if = "is_zero")]
    xfade: usize,
    #[serde(rename = "cles", default, skip_serializing_if = "Vec::is_empty")]
    keys: Vec<VolumeKey>,
    #[serde(rename = "egaliseur", default, skip_serializing_if = "is_flat")]
    eq: [f32; 3],
    #[serde(rename = "echo", default, skip_serializing_if = "is_no_echo")]
    echo: Echo,
}

#[derive(Serialize, Deserialize)]
struct TrackEntry {
    #[serde(rename = "piste")]
    track: u64,
    #[serde(rename = "solo", default)]
    solo: bool,
}

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(rename = "extraits", default)]
    clips: Vec<ClipEntry>,
    #[serde(rename = "pistes", default)]
    tracks: Vec<TrackEntry>,
}

fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_zero(v: &usize) -> bool {
    *v == 0
}
fn is_flat(v: &[f32; 3]) -> bool {
    v.iter().all(|g| *g == 0.0)
}
fn is_no_echo(e: &Echo) -> bool {
    e.mix == 0.0
}

/// Le contenu de `effets_son.json` pour `doc` ; rien s'il n'y a aucun effet ni solo (le fichier part).
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let mut clips = Vec::new();
    let mut tracks = Vec::new();
    for t in &doc.audio_tracks {
        if t.solo {
            tracks.push(TrackEntry { track: t.id, solo: true });
        }
        for c in t.clips.iter().filter(|c| !c.fx.is_default()) {
            let fx = &c.fx;
            // (L'écho garde ses réglages même éteint, tant que le niveau n'est pas à zéro.)
            clips.push(ClipEntry { clip: c.id, gain: fx.gain, fade_in: fx.fade_in, fade_out: fx.fade_out, xfade: fx.xfade, keys: fx.keys.clone(), eq: fx.eq, echo: fx.echo });
        }
    }
    if clips.is_empty() && tracks.is_empty() {
        return Ok(None);
    }
    let sidecar = Sidecar { format: FORMAT.into(), version: VERSION, clips, tracks };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les effets lus dans le texte de `effets_son.json`, à poser sur les extraits et les pistes. Les
/// valeurs sont ramenées dans leurs bornes ; un numéro inconnu est ignoré à l'application.
pub(crate) fn parse(text: &str) -> Result<Loaded, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier d'effets de son".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut seen = HashSet::new();
    let clips = sidecar
        .clips
        .into_iter()
        .filter(|e| seen.insert(e.clip))
        .map(|e| {
            let mut fx = ClipFx { gain: e.gain, fade_in: e.fade_in, fade_out: e.fade_out, xfade: e.xfade, keys: e.keys, eq: e.eq, echo: e.echo };
            fx.sanitize();
            (e.clip, fx)
        })
        .collect();
    let solo = sidecar.tracks.into_iter().filter(|t| t.solo).map(|t| t.track).collect();
    Ok(Loaded { clips, solo })
}

/// Ce que dit `effets_son.json`.
pub(crate) struct Loaded {
    pub clips: Vec<(u64, ClipFx)>,
    pub solo: Vec<u64>,
}

impl Loaded {
    /// Pose les effets sur les extraits et les pistes du document (rien pour un numéro inconnu).
    pub(crate) fn install(self, doc: &mut Document) {
        for (id, fx) in self.clips {
            if let Some((t, c)) = doc.find_clip(id) {
                doc.audio_tracks[t].clips[c].fx = fx;
            }
        }
        for id in self.solo {
            if let Some(t) = doc.audio_tracks.iter_mut().find(|t| t.id == id) {
                t.solo = true;
            }
        }
    }
}
