//! Exports pour le montage (Anthony, 2026-10-20) : l'animatique découpée plan par plan, avec ses
//! durées, pour un monteur (Premiere Pro, DaVinci Resolve, Avid…).
//!
//! # Ce qui sort
//!
//! Un dossier `<Nom>_montage/` :
//!
//! ```text
//! <Nom>.edl              EDL CMX 3600 (image seulement)
//! <Nom>.xml              Final Cut Pro 7 XML (xmeml 4) : Premiere Pro et DaVinci Resolve l'importent
//! <Nom>.otio             OpenTimelineIO (JSON)
//! media/SQ01_PL012A.mov  un fichier vidéo par plan (ProRes 422 HQ, ou H.264), `editorialjob.rs`
//! audio/A1_Dialogues_01.wav  un fichier son par extrait, effets compris (XML et OTIO seulement)
//! LISEZ-MOI.txt          ce qui a été écrit, ce qui ne l'a pas été, ce qui reste à vérifier
//! ```
//!
//! Les fichiers de montage désignent les médias par des **chemins relatifs** au dossier (`media/…`),
//! ou absolus au choix. Les noms de clips sont ceux de la numérotation des plans
//! (`Document::plan_full_ref` : « SQ01_PL012A », voir `numerotation.rs`) ; un plan qui a plusieurs versions
//! (`planvers.rs`) y ajoute celle qui part : « SQ01_PL012A_V2 » (nom du clip et du fichier ; un plan à une seule
//! version garde son nom sans suffixe).
//!
//! Ce module est **pur** (pas d'interface, pas de fichier) : `build_plan` lit le document et décrit
//! l'export (`Plan`), `write_edl` / `write_xml` / `write_otio` / `readme` en font du texte. L'écriture
//! des médias et des fichiers est dans `editorialjob.rs`, la fenêtre dans `editorialui.rs`.
//!
//! # Temps
//!
//! - Tout est compté en **images du projet** (`Document::fps`, entier) ; le début de la ligne de temps
//!   est un time code de départ (01:00:00:00 d'office). Time code **sans saut d'images** (NDF) :
//!   `HH:MM:SS:FF`, FF < cadence. Les cadences non standard (12, 15, 48, 60…) sont écrites avec la même
//!   arithmétique (FF jusqu'à cadence - 1) ; l'EDL CMX 3600 n'est définie que jusqu'à 30 i/s.
//! - **NTSC** (option, pour 24, 30 et 60) : la cadence réelle est × 1000/1001 (23,976 / 29,97 / 59,94).
//!   Les numéros d'images ne changent pas, seuls changent le drapeau `ntsc` du xmeml, la cadence
//!   réelle de l'OTIO (24000/1001…) et celle des vidéos par plan (ffmpeg `24000/1001`). Le time code
//!   reste NDF (le 29,97 à sauts d'images n'est pas écrit). Le **son** reste à 48 kHz et n'est pas
//!   ralenti de 0,1 % : sur un film long, un son synchrone dérive donc de 0,1 % (3,6 s par heure) ;
//!   si la vidéo est tirée en NTSC, c'est au monteur de ralentir le son (à vérifier).
//!
//! # Poignées (transitions)
//!
//! Dans SHOT, une transition ne rallonge aucun plan : la partie qui manque à un plan est **tenue**
//! (`transitions.rs`). Pour qu'un monteur retrouve exactement la même image, le fichier vidéo d'un
//! plan qui participe à un fondu enchaîné contient des **poignées** : sa première image répétée avant
//! (`head`) et sa dernière après (`tail`), juste ce que le fondu mange. Le clip de la ligne de temps
//! ne montre que les images du plan (entrée = `head`, sortie = `head + len`), le fondu lit dans les
//! poignées.
//!
//! # Transitions écrites
//!
//! | Transition de SHOT | EDL | Final Cut XML | OTIO |
//! |---|---|---|---|
//! | Fondu enchaîné entre deux plans | `D nnn`, deux lignes sur le même événement | `transitionitem` « Cross Dissolve » | `Transition` `SMPTE_Dissolve` |
//! | Fondu au noir, début ou fin du film | dissolve depuis / vers `BL` | coupe (notée) | coupe (notée) |
//! | Fondu au blanc, volet, noir, fondu par le noir entre deux plans | coupe (notée) | coupe (notée) | coupe (notée) |
//!
//! Les coupes notées sont dites dans `LISEZ-MOI.txt` (et en commentaire de l'EDL) : on ne devine pas ce
//! que chaque logiciel ferait d'un effet qu'on ne sait pas lui décrire.
//!
//! # Formats : ce sur quoi on s'appuie (rien de plus)
//!
//! - **EDL CMX 3600** : `TITLE:`, `FCM: NON-DROP FRAME`, événements `NNN  REEL  V  C  src_in src_out
//!   rec_in rec_out` (champs séparés par des blancs), `D nnn` pour un fondu enchaîné de nnn images
//!   (événement à deux lignes : le plan sortant à longueur nulle, `C`, puis le plan entrant, `D nnn`),
//!   `BL` pour le noir, `* FROM CLIP NAME:` pour le nom du clip. Noms de bobine de 8 caractères au plus.
//! - **Final Cut Pro 7 XML (xmeml, version 4)** : `sequence` > `media` > `video` / `audio` > `track` >
//!   `clipitem` (`start` / `end` dans la séquence, `in` / `out` dans le fichier, en images) et
//!   `transitionitem` (`start`, `end`, `alignment`, `effect`), `file` (`pathurl`, `duration`,
//!   `timecode`). Chemins : `pathurl` relatif au dossier (ou `file://localhost/…` absolu).
//! - **OpenTimelineIO** (JSON, schémas `Timeline.1`, `Stack.1`, `Track.1`, `Clip.1`, `Gap.1`,
//!   `Transition.1`, `ExternalReference.1`, `TimeRange.1`, `RationalTime.1`) : `Clip.1` est lu par toutes
//!   les versions (les récentes le convertissent en `Clip.2`).
//!
//! Aucun de ces fichiers n'a été ouvert dans Premiere Pro, DaVinci Resolve ou Avid : voir
//! `docs/EXPORT_MONTAGE.md` (« à vérifier par Anthony »).

use std::path::Path;

use serde_json::{json, Value};

use crate::audiofx;
use crate::audio::RATE;
use crate::document::{Document, Sound, SoundClip};
use crate::transitions::{self, Align, Kind};

/// Format des fichiers vidéo par plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    /// MOV ProRes 422 HQ : pour le montage.
    ProRes,
    /// MP4 H.264 : plus léger.
    H264,
}

impl Media {
    pub fn extension(self) -> &'static str {
        match self {
            Media::ProRes => "mov",
            Media::H264 => "mp4",
        }
    }

    pub fn format(self) -> crate::export::Format {
        match self {
            Media::ProRes => crate::export::Format::ProRes,
            Media::H264 => crate::export::Format::Mp4,
        }
    }
}

/// Les choix de la fenêtre.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub edl: bool,
    pub xml: bool,
    pub otio: bool,
    /// Time code du début de la ligne de temps (`HH:MM:SS:FF`).
    pub start_tc: String,
    /// Cadence réelle × 1000/1001 (pour 24, 30 et 60 i/s).
    pub ntsc: bool,
    pub media: Media,
    /// Vidéos à moitié de la taille du cadre.
    pub half: bool,
    /// Tout le film (sinon : la séquence courante).
    pub whole_film: bool,
    /// Avec les pistes son (XML et OTIO).
    pub sound: bool,
    /// Chemins absolus dans le XML et l'OTIO (sinon : relatifs au dossier).
    pub absolute: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            edl: true,
            xml: true,
            otio: true,
            start_tc: "01:00:00:00".into(),
            ntsc: false,
            media: Media::ProRes,
            half: false,
            whole_film: true,
            sound: true,
            absolute: false,
        }
    }
}

// ----------------------------------------------------------------------------------- cadence, time code

/// La cadence de la ligne de temps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub fps: u32,
    pub ntsc: bool,
}

/// Le NTSC (× 1000/1001) existe pour 24, 30 et 60 i/s.
pub fn ntsc_possible(fps: u32) -> bool {
    matches!(fps, 24 | 30 | 60)
}

impl Rate {
    pub fn new(fps: u32, ntsc: bool) -> Rate {
        Rate { fps: fps.max(1), ntsc: ntsc && ntsc_possible(fps) }
    }

    /// Images par seconde réelles (23,976…).
    pub fn real(self) -> f64 {
        if self.ntsc {
            self.fps as f64 * 1000.0 / 1001.0
        } else {
            self.fps as f64
        }
    }

    /// La cadence comme ffmpeg la lit (`24000/1001`).
    pub fn ffmpeg(self) -> String {
        if self.ntsc {
            format!("{}/1001", self.fps * 1000)
        } else {
            self.fps.to_string()
        }
    }

    pub fn label(self) -> String {
        if self.ntsc {
            format!("{} i/s (NTSC)", format!("{:.3}", self.real()).replace('.', ","))
        } else {
            format!("{} i/s", self.fps)
        }
    }
}

/// `HH:MM:SS:FF` sans saut d'images.
pub fn timecode(frames: i64, fps: u32) -> String {
    let fps = fps.max(1) as i64;
    let f = frames.max(0);
    let s = f / fps;
    format!("{:02}:{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60, f % fps)
}

/// Lit `HH:MM:SS:FF` (le séparateur peut être `:`, `;` ou `.`) : le nombre d'images depuis zéro. Rien si
/// ce n'est pas un time code de cette cadence.
pub fn parse_timecode(text: &str, fps: u32) -> Option<i64> {
    let parts: Vec<&str> = text.trim().split([':', ';', '.']).collect();
    if parts.len() != 4 || parts.iter().any(|p| p.is_empty() || p.len() > 2 || !p.bytes().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let n: Vec<i64> = parts.iter().map(|p| p.parse().unwrap_or(-1)).collect();
    let fps = fps.max(1) as i64;
    if n[1] >= 60 || n[2] >= 60 || n[3] >= fps {
        return None;
    }
    Some(((n[0] * 60 + n[1]) * 60 + n[2]) * fps + n[3])
}

// ----------------------------------------------------------------------------------- noms

/// Le texte sans accents ni caractères hors ASCII (EDL, noms de fichiers).
pub fn ascii_fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'À' | 'Â' | 'Ä' | 'Á' | 'Ã' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'Î' | 'Ï' | 'Í' | 'Ì' => 'I',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => 'O',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'œ' => 'o',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '_',
        })
        .collect()
}

/// Un nom de fichier : lettres, chiffres, `_` et `-`.
pub fn file_safe(s: &str) -> String {
    let folded = ascii_fold(s.trim());
    let out: String = folded.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "sans_nom".into()
    } else {
        out
    }
}

// ----------------------------------------------------------------------------------- le plan d'export

/// Un plan : un clip de la ligne de temps et son fichier vidéo.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanClip {
    /// Rang du panneau dans le document.
    pub panel: usize,
    /// « SQ01_PL012A ».
    pub name: String,
    /// Fichier relatif au dossier : « media/SQ01_PL012A.mov ».
    pub file: String,
    /// Nom de bobine de l'EDL (8 caractères au plus).
    pub reel: String,
    /// Première image dans la ligne de temps (0 = début de l'export).
    pub start: usize,
    /// Images du plan.
    pub len: usize,
    /// Poignées tenues avant et après les images du plan, pour les fondus enchaînés.
    pub head: usize,
    pub tail: usize,
    /// « Séquence 1 — plan 12A » (nom lisible).
    pub title: String,
}

impl PlanClip {
    pub fn end(&self) -> usize {
        self.start + self.len
    }

    /// Images du fichier : les poignées et le plan.
    pub fn file_frames(&self) -> usize {
        self.head + self.len + self.tail
    }
}

/// Un fondu enchaîné entre le clip `a` et le clip `b` (`b == a + 1`), en images de la ligne de temps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dissolve {
    pub a: usize,
    pub b: usize,
    pub start: usize,
    pub end: usize,
    /// La coupe : première image du clip `b`.
    pub cut: usize,
    pub align: Align,
}

/// Un fondu au noir au début ou à la fin du film (écrit dans l'EDL seulement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fade {
    pub clip: usize,
    /// Vers le noir (fin du film) ; sinon depuis le noir (début).
    pub out: bool,
    pub start: usize,
    pub end: usize,
}

/// Ce que l'export n'a pas pu écrire tel quel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub text: String,
    /// Les formats qui perdent la transition : EDL, XML, OTIO.
    pub lost: [bool; 3],
}

/// Un extrait son dans la ligne de temps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioOut {
    /// Piste de sortie (1, 2…), parmi les pistes qu'on entend.
    pub track: usize,
    pub track_name: String,
    /// « A1_Dialogues_01 ».
    pub name: String,
    /// « audio/A1_Dialogues_01.wav ».
    pub file: String,
    pub start: usize,
    pub len: usize,
}

/// Ce qu'il faut pour écrire le son d'un extrait (hors du document : l'écriture se fait sur un fil).
#[derive(Clone)]
pub struct AudioSource {
    pub sound: Sound,
    pub fps: u32,
    /// L'extrait, déjà coupé à l'étendue exportée.
    pub clip: SoundClip,
    /// Volume de la piste × volume général.
    pub gain: f32,
}

/// L'export décrit : tout ce que les écrivains et la tâche d'export ont à savoir.
#[derive(Clone)]
pub struct Plan {
    pub title: String,
    pub rate: Rate,
    pub start_tc: i64,
    /// Taille des vidéos (paire), et cadre du projet (la taille des images qu'on envoie à ffmpeg).
    pub size: [usize; 2],
    pub frame: [usize; 2],
    pub media: Media,
    pub half: bool,
    pub total: usize,
    pub clips: Vec<PlanClip>,
    pub dissolves: Vec<Dissolve>,
    pub fades: Vec<Fade>,
    pub skipped: Vec<Skipped>,
    pub audio: Vec<AudioOut>,
    pub audio_sources: Vec<AudioSource>,
    /// Pistes son qu'on n'entend pas (muettes, ou une autre en solo) et qui ne sont pas écrites.
    pub silent_tracks: Vec<String>,
    /// Extraits dont le son manque.
    pub missing_sounds: usize,
    /// Cadence qui n'est pas celle de l'EDL CMX 3600 (plus de 30 i/s, ou moins de 24 hors 12 et 15).
    pub odd_rate: bool,
}

impl Plan {
    /// Les plans qui ont des poignées (leur fichier est plus long que le plan).
    pub fn handles(&self) -> usize {
        self.clips.iter().filter(|c| c.head + c.tail > 0).count()
    }

    /// Images à rendre en tout (poignées comprises).
    pub fn frames_to_render(&self) -> usize {
        self.clips.iter().map(PlanClip::file_frames).sum()
    }

    /// Images de la ligne de temps pour le time code (`rec`).
    pub fn rec(&self, frame: usize) -> String {
        timecode(self.start_tc + frame as i64, self.rate.fps)
    }

    /// Time code d'une image d'un fichier vidéo (qui commence à zéro).
    pub fn src(&self, frame: usize) -> String {
        timecode(frame as i64, self.rate.fps)
    }
}

/// Nom de bobine de l'EDL : « PL012A », au plus 8 caractères de A-Z, 0-9 et `_` ; à défaut « R00001 ».
fn reel_name(plan_ref: &str, index: usize) -> String {
    let r: String = plan_ref.chars().map(|c| if c == '.' { '_' } else { c.to_ascii_uppercase() }).collect();
    if r.len() <= 8 && r.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        r
    } else {
        format!("R{:05}", index + 1)
    }
}

/// Décrit l'export des plans choisis. `cur_panel` : le plan courant (la « séquence courante » est
/// celle de ce plan). Erreur si le time code de départ n'est pas lisible ou s'il n'y a rien à exporter.
pub fn build_plan(doc: &Document, opts: &Options, cur_panel: usize, title: &str) -> Result<Plan, String> {
    let fps = doc.fps.max(1);
    let rate = Rate::new(fps, opts.ntsc);
    let start_tc = parse_timecode(&opts.start_tc, fps).ok_or_else(|| format!("Time code de départ illisible : « {} » (attendu HH:MM:SS:FF, FF de 0 à {}).", opts.start_tc.trim(), fps - 1))?;
    if doc.panels.is_empty() {
        return Err("Le projet n'a aucun plan.".into());
    }
    let cur = cur_panel.min(doc.panels.len() - 1);
    let (a, b) = if opts.whole_film { (0, doc.panels.len()) } else { doc.sequence_run(cur) };
    let t0 = doc.panel_start(a);
    let size = crate::export::video_size(doc.frame, opts.half);

    // Les clips, les uns à la suite des autres.
    let mut clips: Vec<PlanClip> = Vec::new();
    let mut at = 0;
    for p in a..b {
        // Un plan qui a plusieurs versions porte celle qui part dans son nom : « SQ01_PL012A_V2 » (la bobine de
        // l'EDL reste « PL012A » : 8 caractères au plus).
        let versionne = doc.nombre_de_versions(doc.panels[p].id) > 1;
        let suffixe = if versionne { format!("_{}", crate::planvers::etiquette(doc.numero_de_version(doc.panels[p].id))) } else { String::new() };
        let name = format!("{}{suffixe}", doc.plan_full_ref(p));
        let ref_only = doc.plan_ref(doc.panels[p].scene);
        let len = doc.panels[p].frame_count();
        clips.push(PlanClip {
            panel: p,
            file: format!("media/{}.{}", file_safe(&name), opts.media.extension()),
            reel: reel_name(&ref_only, clips.len()),
            name,
            start: at,
            len,
            head: 0,
            tail: 0,
            title: if versionne { format!("{} · {}", crate::storyboard::panel_long_title(doc, p), crate::planvers::etiquette(doc.numero_de_version(doc.panels[p].id))) } else { crate::storyboard::panel_long_title(doc, p) },
        });
        at += len;
    }
    let total = at;
    let end_film = b == doc.panels.len();

    // Les transitions.
    let mut dissolves = Vec::new();
    let mut fades = Vec::new();
    let mut skipped: Vec<Skipped> = Vec::new();
    let clip_of = |panel: usize| panel.checked_sub(a).filter(|k| *k < clips.len());
    for z in transitions::zones(doc) {
        let (zs, ze, zc) = (z.start.saturating_sub(t0), z.end.saturating_sub(t0), z.cut.saturating_sub(t0));
        match (z.out, z.inn) {
            (Some(o), Some(i)) => {
                let (Some(ka), Some(kb)) = (clip_of(o.panel), clip_of(i.panel)) else { continue };
                if z.kind == Kind::Dissolve {
                    dissolves.push(Dissolve { a: ka, b: kb, start: zs, end: ze, cut: zc, align: z.align });
                } else {
                    skipped.push(Skipped {
                        text: format!("{} entre {} et {} (images {} à {}) : écrit comme une coupe.", z.kind.name(), clips[ka].name, clips[kb].name, zs, ze),
                        lost: [true, true, true],
                    });
                }
            }
            (None, Some(i)) => {
                let Some(k) = clip_of(i.panel).filter(|_| a == 0) else { continue };
                if z.kind == Kind::Black {
                    fades.push(Fade { clip: k, out: false, start: zs, end: ze });
                    skipped.push(Skipped { text: format!("Fondu depuis le noir au début ({} images) : écrit dans l'EDL seulement.", ze - zs), lost: [false, true, true] });
                } else {
                    skipped.push(Skipped { text: format!("{} au début du film : écrit comme une coupe.", z.kind.name()), lost: [true, true, true] });
                }
            }
            (Some(o), None) => {
                let Some(k) = clip_of(o.panel).filter(|_| end_film) else { continue };
                if z.kind == Kind::Black {
                    fades.push(Fade { clip: k, out: true, start: zs, end: ze });
                    skipped.push(Skipped { text: format!("Fondu vers le noir à la fin ({} images) : écrit dans l'EDL seulement.", ze - zs), lost: [false, true, true] });
                } else {
                    skipped.push(Skipped { text: format!("{} à la fin du film : écrit comme une coupe.", z.kind.name()), lost: [true, true, true] });
                }
            }
            (None, None) => {}
        }
    }
    // Poignées : ce que le fondu mange hors des images de chaque plan.
    for d in &dissolves {
        clips[d.a].tail = clips[d.a].tail.max(d.end.saturating_sub(d.cut));
        clips[d.b].head = clips[d.b].head.max(d.cut.saturating_sub(d.start));
    }

    // Le son : une piste de sortie par piste qu'on entend, un fichier par extrait.
    let mut audio = Vec::new();
    let mut audio_sources = Vec::new();
    let mut silent_tracks = Vec::new();
    let mut missing_sounds = 0;
    if opts.sound {
        let (f0, f1) = (t0, t0 + total);
        let mut out_track = 0;
        for track in &doc.audio_tracks {
            if !audiofx::audible(doc, track) {
                if !track.clips.is_empty() {
                    silent_tracks.push(track.name.clone());
                }
                continue;
            }
            let mut order: Vec<&SoundClip> = track.clips.iter().filter(|c| c.length > 0 && c.start < f1 && c.end() > f0).collect();
            order.sort_by_key(|c| c.start);
            if order.is_empty() {
                continue;
            }
            out_track += 1;
            for (k, c) in order.iter().enumerate() {
                let Some(sound) = doc.sounds.get(&c.sound) else {
                    missing_sounds += 1;
                    continue;
                };
                // L'extrait coupé à l'étendue exportée (ses fondus gardent ceux de l'extrait entier).
                let mut clip = (*c).clone();
                if clip.start < f0 {
                    let cut = f0 - clip.start;
                    clip.start = f0;
                    clip.offset += cut;
                    clip.length -= cut;
                }
                if clip.end() > f1 {
                    clip.length = f1 - clip.start;
                }
                let name = format!("A{}_{}_{:02}", out_track, file_safe(&track.name), k + 1);
                audio.push(AudioOut { track: out_track, track_name: track.name.clone(), file: format!("audio/{name}.wav"), name, start: clip.start - f0, len: clip.length });
                audio_sources.push(AudioSource { sound: sound.clone(), fps, clip, gain: track.volume * doc.master_volume });
            }
        }
    }

    Ok(Plan {
        title: if title.trim().is_empty() { "SHOT".into() } else { title.trim().to_string() },
        rate,
        start_tc,
        size,
        frame: doc.frame,
        media: opts.media,
        half: opts.half,
        total,
        clips,
        dissolves,
        fades,
        skipped,
        audio,
        audio_sources,
        silent_tracks,
        missing_sounds,
        odd_rate: fps > 30 || (fps < 24 && fps != 12 && fps != 15),
    })
}

/// Le son d'un extrait en 16 bits stéréo à 48 kHz, effets compris (`audiofx::bake_sound`), volume de la
/// piste appliqué : l'en-tête WAV puis les échantillons.
pub fn wav_bytes(src: &AudioSource) -> Vec<u8> {
    let samples = audiofx::bake_sound(&src.sound, src.fps, &src.clip);
    let mut out = Vec::with_capacity(44 + samples.len() * 4);
    out.extend_from_slice(&crate::export::wav_header(samples.len() as u32));
    for s in &samples {
        for ch in s {
            out.extend_from_slice(&(((ch * src.gain).clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
    }
    out
}

// ----------------------------------------------------------------------------------- EDL

/// Une ligne d'événement de l'EDL.
struct Line {
    reel: String,
    chan: &'static str,
    /// `None` : coupe (`C`) ; `Some(n)` : fondu enchaîné de `n` images (`D nnn`).
    dissolve: Option<usize>,
    src_in: usize,
    src_out: usize,
    rec_in: usize,
    rec_out: usize,
    clip_name: String,
}

fn edl_line(plan: &Plan, event: usize, l: &Line, out: &mut String) {
    let trans = match l.dissolve {
        None => "C        ".to_string(),
        Some(n) => format!("D    {:03} ", n),
    };
    out.push_str(&format!(
        "{:03}  {:<8} {:<6}{}{} {} {} {}\n",
        event,
        l.reel,
        l.chan,
        trans,
        plan.src(l.src_in),
        plan.src(l.src_out),
        plan.rec(l.rec_in),
        plan.rec(l.rec_out)
    ));
    if !l.clip_name.is_empty() {
        out.push_str(&format!("* FROM CLIP NAME: {}\n", ascii_fold(&l.clip_name)));
    }
}

/// L'EDL CMX 3600 (image seulement).
pub fn write_edl(plan: &Plan) -> String {
    let mut s = String::new();
    s.push_str(&format!("TITLE: {}\n", ascii_fold(&plan.title)));
    s.push_str("FCM: NON-DROP FRAME\n\n");
    s.push_str(&format!("* SHOT : {} plan(s), {} images, {}.\n", plan.clips.len(), plan.total, plan.rate.label()));
    if plan.rate.ntsc {
        s.push_str("* Cadence NTSC : les numeros d'images sont ceux du projet, time code sans saut d'images.\n");
    }
    if plan.odd_rate {
        s.push_str(&format!("* Attention : {} i/s n'est pas une cadence de l'EDL CMX 3600 (24, 25, 30).\n", plan.rate.fps));
    }
    for k in &plan.skipped {
        if k.lost[0] {
            s.push_str(&format!("* {}\n", ascii_fold(&k.text)));
        }
    }
    s.push_str("* Les poignees des fondus enchaines (images tenues) sont dans les fichiers video : les entrees des plans les sautent.\n\n");

    let n = plan.clips.len();
    let dis_after = |k: usize| plan.dissolves.iter().find(|d| d.a == k);
    let fade_in = |k: usize| plan.fades.iter().find(|f| f.clip == k && !f.out);
    let fade_out = |k: usize| plan.fades.iter().find(|f| f.clip == k && f.out);
    // L'image du fichier du clip `k` qui est à l'image `rec` de la ligne de temps (poignée comprise).
    let src_at_signed = |k: usize, rec: usize| (plan.clips[k].head as i64 + rec as i64 - plan.clips[k].start as i64).max(0) as usize;
    let play_out = |k: usize| -> usize {
        if let Some(d) = dis_after(k) {
            d.start
        } else if let Some(f) = fade_out(k) {
            f.start
        } else {
            plan.clips[k].end()
        }
    };
    let cut_line = |k: usize, from: usize, to: usize| Line {
        reel: plan.clips[k].reel.clone(),
        chan: "V",
        dissolve: None,
        src_in: src_at_signed(k, from),
        src_out: src_at_signed(k, to),
        rec_in: from,
        rec_out: to,
        clip_name: plan.clips[k].name.clone(),
    };
    let black_line = |at: usize, len: usize, dissolve: Option<usize>| Line {
        reel: "BL".into(),
        chan: "V",
        dissolve,
        src_in: 0,
        src_out: len,
        rec_in: at,
        rec_out: at + len,
        clip_name: String::new(),
    };

    let mut event = 0;
    let mut incoming = vec![false; n];
    for k in 0..n {
        // Fondu depuis le noir : le noir d'abord (longueur nulle), puis le plan, en fondu.
        if let Some(f) = fade_in(k) {
            event += 1;
            let zero = black_line(f.start, 0, None);
            edl_line(plan, event, &zero, &mut s);
            let mut l = cut_line(k, f.start, play_out(k));
            l.dissolve = Some(f.end - f.start);
            edl_line(plan, event, &l, &mut s);
            incoming[k] = true;
        }
        if !incoming[k] {
            event += 1;
            edl_line(plan, event, &cut_line(k, plan.clips[k].start, play_out(k)), &mut s);
        }
        if let Some(d) = dis_after(k) {
            // Le plan sortant, à longueur nulle, puis le plan entrant en fondu de `n` images ; le fondu
            // commence à `d.start` et le plan entrant y entre dans sa poignée.
            event += 1;
            let kb = d.b;
            edl_line(plan, event, &cut_line(k, d.start, d.start), &mut s);
            let mut l = cut_line(kb, d.start, play_out(kb));
            l.dissolve = Some(d.end - d.start);
            edl_line(plan, event, &l, &mut s);
            incoming[kb] = true;
        }
        if let Some(f) = fade_out(k) {
            event += 1;
            let zero = cut_line(k, f.start, f.start);
            edl_line(plan, event, &zero, &mut s);
            edl_line(plan, event, &black_line(f.start, f.end - f.start, Some(f.end - f.start)), &mut s);
        }
    }
    if plan.clips.is_empty() {
        s.push_str("* (aucun plan)\n");
    }
    s
}

// ----------------------------------------------------------------------------------- XML (xmeml)

struct Xml {
    s: String,
    depth: usize,
}

fn escape(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

impl Xml {
    fn new() -> Xml {
        Xml { s: String::new(), depth: 0 }
    }

    fn pad(&mut self) {
        for _ in 0..self.depth {
            self.s.push_str("  ");
        }
    }

    fn open(&mut self, tag: &str) {
        self.pad();
        self.s.push_str(&format!("<{tag}>\n"));
        self.depth += 1;
    }

    fn open_attr(&mut self, tag: &str, attrs: &str) {
        self.pad();
        if attrs.is_empty() {
            self.s.push_str(&format!("<{tag}>\n"));
        } else {
            self.s.push_str(&format!("<{tag} {attrs}>\n"));
        }
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.pad();
        self.s.push_str(&format!("</{tag}>\n"));
    }

    fn leaf(&mut self, tag: &str, text: impl std::fmt::Display) {
        self.pad();
        self.s.push_str(&format!("<{tag}>{}</{tag}>\n", escape(&text.to_string())));
    }

    fn empty(&mut self, tag: &str, attrs: &str) {
        self.pad();
        self.s.push_str(&format!("<{tag} {attrs}/>\n"));
    }

    fn rate(&mut self, r: Rate) {
        self.open("rate");
        self.leaf("timebase", r.fps);
        self.leaf("ntsc", if r.ntsc { "TRUE" } else { "FALSE" });
        self.close("rate");
    }

    fn timecode(&mut self, r: Rate, frame: i64) {
        self.open("timecode");
        self.rate(r);
        self.leaf("string", timecode(frame, r.fps));
        self.leaf("frame", frame);
        self.leaf("displayformat", "NDF");
        self.close("timecode");
    }
}

/// `file://localhost/C:/dossier/media/x.mov` (caractères à protéger en %XX).
pub fn file_url(path: &Path) -> String {
    let p = path.to_string_lossy().replace('\\', "/");
    let p = p.strip_prefix("//?/").unwrap_or(&p);
    let mut out = String::from("file://localhost/");
    for b in p.trim_start_matches('/').bytes() {
        if b.is_ascii_alphanumeric() || b"/:._-~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Le chemin d'un média tel qu'écrit dans les fichiers : relatif, ou absolu sous `abs`.
fn media_path(rel: &str, abs: Option<&Path>) -> String {
    match abs {
        Some(dir) => file_url(&dir.join(rel)),
        None => rel.to_string(),
    }
}

fn alignment(a: Align) -> &'static str {
    match a {
        Align::Centre => "center",
        Align::Before => "end",
        Align::After => "start",
    }
}

fn xml_video_file(x: &mut Xml, plan: &Plan, c: &PlanClip, id: &str, abs: Option<&Path>) {
    x.open_attr("file", &format!("id=\"{id}\""));
    x.leaf("name", c.file.rsplit('/').next().unwrap_or(&c.file));
    x.leaf("pathurl", media_path(&c.file, abs));
    x.rate(plan.rate);
    x.leaf("duration", c.file_frames());
    x.timecode(plan.rate, 0);
    x.open("media");
    x.open("video");
    x.open("samplecharacteristics");
    x.rate(plan.rate);
    x.leaf("width", plan.size[0]);
    x.leaf("height", plan.size[1]);
    x.leaf("anamorphic", "FALSE");
    x.leaf("pixelaspectratio", "square");
    x.leaf("fielddominance", "none");
    x.close("samplecharacteristics");
    x.close("video");
    x.close("media");
    x.close("file");
}

/// Le Final Cut Pro 7 XML (xmeml 4). `abs` : dossier de l'export, pour des chemins absolus.
pub fn write_xml(plan: &Plan, abs: Option<&Path>) -> String {
    let mut x = Xml::new();
    x.s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n");
    x.open_attr("xmeml", "version=\"4\"");
    x.open_attr("sequence", "id=\"sequence-1\"");
    x.leaf("name", &plan.title);
    x.leaf("duration", plan.total);
    x.rate(plan.rate);
    x.timecode(plan.rate, plan.start_tc);
    x.open("media");

    // Vidéo : une piste.
    x.open("video");
    x.open("format");
    x.open("samplecharacteristics");
    x.rate(plan.rate);
    x.leaf("width", plan.size[0]);
    x.leaf("height", plan.size[1]);
    x.leaf("anamorphic", "FALSE");
    x.leaf("pixelaspectratio", "square");
    x.leaf("fielddominance", "none");
    x.close("samplecharacteristics");
    x.close("format");
    x.open("track");
    for (k, c) in plan.clips.iter().enumerate() {
        x.open_attr("clipitem", &format!("id=\"clipitem-{}\"", k + 1));
        x.leaf("name", &c.name);
        x.leaf("enabled", "TRUE");
        x.leaf("duration", c.file_frames());
        x.rate(plan.rate);
        x.leaf("start", c.start);
        x.leaf("end", c.end());
        x.leaf("in", c.head);
        x.leaf("out", c.head + c.len);
        xml_video_file(&mut x, plan, c, &format!("file-{}", k + 1), abs);
        x.open("sourcetrack");
        x.leaf("mediatype", "video");
        x.leaf("trackindex", 1);
        x.close("sourcetrack");
        x.open("comments");
        x.leaf("mastercomment1", &c.title);
        x.close("comments");
        x.close("clipitem");
        if let Some(d) = plan.dissolves.iter().find(|d| d.a == k) {
            x.open("transitionitem");
            x.leaf("start", d.start);
            x.leaf("end", d.end);
            x.leaf("alignment", alignment(d.align));
            x.rate(plan.rate);
            x.open("effect");
            x.leaf("name", "Cross Dissolve");
            x.leaf("effectid", "Cross Dissolve");
            x.leaf("effectcategory", "Dissolve");
            x.leaf("effecttype", "transition");
            x.leaf("mediatype", "video");
            x.close("effect");
            x.close("transitionitem");
        }
    }
    x.close("track");
    x.close("video");

    // Son : chaque piste de SHOT devient deux pistes (voie gauche, voie droite) du même fichier stéréo.
    if !plan.audio.is_empty() {
        x.open("audio");
        x.leaf("numOutputChannels", 2);
        x.open("format");
        x.open("samplecharacteristics");
        x.leaf("depth", 16);
        x.leaf("samplerate", RATE);
        x.close("samplecharacteristics");
        x.close("format");
        let tracks = plan.audio.iter().map(|a| a.track).max().unwrap_or(0);
        for t in 1..=tracks {
            for channel in 1..=2 {
                x.open_attr("track", "");
                for (k, a) in plan.audio.iter().enumerate().filter(|(_, a)| a.track == t) {
                    x.open_attr("clipitem", &format!("id=\"clipitem-a{}-{}-{}\"", t, channel, k + 1));
                    x.leaf("name", &a.name);
                    x.leaf("enabled", "TRUE");
                    x.leaf("duration", a.len);
                    x.rate(plan.rate);
                    x.leaf("start", a.start);
                    x.leaf("end", a.start + a.len);
                    x.leaf("in", 0);
                    x.leaf("out", a.len);
                    if channel == 1 {
                        x.open_attr("file", &format!("id=\"file-a{}\"", k + 1));
                        x.leaf("name", a.file.rsplit('/').next().unwrap_or(&a.file));
                        x.leaf("pathurl", media_path(&a.file, abs));
                        x.rate(plan.rate);
                        x.leaf("duration", a.len);
                        x.open("media");
                        x.open("audio");
                        x.open("samplecharacteristics");
                        x.leaf("depth", 16);
                        x.leaf("samplerate", RATE);
                        x.close("samplecharacteristics");
                        x.leaf("channelcount", 2);
                        x.close("audio");
                        x.close("media");
                        x.close("file");
                    } else {
                        x.empty("file", &format!("id=\"file-a{}\"", k + 1));
                    }
                    x.open("sourcetrack");
                    x.leaf("mediatype", "audio");
                    x.leaf("trackindex", channel);
                    x.close("sourcetrack");
                    x.close("clipitem");
                }
                x.close("track");
            }
        }
        x.close("audio");
    }
    x.close("media");
    x.close("sequence");
    x.close("xmeml");
    x.s
}

// ----------------------------------------------------------------------------------- OpenTimelineIO

fn rt(value: f64, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate, "value": value })
}

fn tr(start: usize, duration: usize, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "TimeRange.1", "start_time": rt(start as f64, rate), "duration": rt(duration as f64, rate) })
}

fn otio_reference(file: &str, name: &str, frames: usize, rate: f64, abs: Option<&Path>, extra: Value) -> Value {
    json!({
        "OTIO_SCHEMA": "ExternalReference.1",
        "name": name,
        "available_range": tr(0, frames, rate),
        "target_url": media_path(file, abs),
        "metadata": extra,
    })
}

/// L'OpenTimelineIO (JSON).
pub fn write_otio(plan: &Plan, abs: Option<&Path>) -> String {
    let rate = plan.rate.real();
    // Piste vidéo : les clips à la suite, un fondu enchaîné entre les deux plans qu'il lie.
    let mut video = Vec::new();
    for (k, c) in plan.clips.iter().enumerate() {
        video.push(json!({
            "OTIO_SCHEMA": "Clip.1",
            "name": c.name,
            "source_range": tr(c.head, c.len, rate),
            "media_reference": otio_reference(&c.file, c.file.rsplit('/').next().unwrap_or(&c.file), c.file_frames(), rate, abs, json!({})),
            "effects": [],
            "markers": [],
            "metadata": { "shot": { "plan": c.name, "titre": c.title, "debut_ligne_de_temps": c.start, "poignee_avant": c.head, "poignee_apres": c.tail } },
        }));
        if let Some(d) = plan.dissolves.iter().find(|d| d.a == k) {
            video.push(json!({
                "OTIO_SCHEMA": "Transition.1",
                "name": "Fondu enchaîné",
                "transition_type": "SMPTE_Dissolve",
                "in_offset": rt((d.cut - d.start) as f64, rate),
                "out_offset": rt((d.end - d.cut) as f64, rate),
                "metadata": {},
            }));
        }
    }
    let mut tracks = vec![json!({
        "OTIO_SCHEMA": "Track.1", "name": "V1", "kind": "Video", "children": video,
        "source_range": null, "effects": [], "markers": [], "metadata": {},
    })];
    // Pistes son : des extraits et des vides.
    let n_tracks = plan.audio.iter().map(|a| a.track).max().unwrap_or(0);
    for t in 1..=n_tracks {
        let mut items = Vec::new();
        let mut at = 0;
        for a in plan.audio.iter().filter(|a| a.track == t) {
            if a.start > at {
                items.push(json!({ "OTIO_SCHEMA": "Gap.1", "name": "", "source_range": tr(0, a.start - at, rate), "effects": [], "markers": [], "metadata": {} }));
            }
            items.push(json!({
                "OTIO_SCHEMA": "Clip.1",
                "name": a.name,
                "source_range": tr(0, a.len, rate),
                "media_reference": otio_reference(&a.file, a.file.rsplit('/').next().unwrap_or(&a.file), a.len, rate, abs, json!({ "echantillonnage": RATE, "voies": 2 })),
                "effects": [],
                "markers": [],
                "metadata": { "shot": { "piste": a.track_name } },
            }));
            at = a.start + a.len;
        }
        let name = plan.audio.iter().find(|a| a.track == t).map_or(String::new(), |a| a.track_name.clone());
        tracks.push(json!({
            "OTIO_SCHEMA": "Track.1", "name": format!("A{t}"), "kind": "Audio", "children": items,
            "source_range": null, "effects": [], "markers": [], "metadata": { "shot": { "piste": name } },
        }));
    }
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": plan.title,
        "global_start_time": rt(plan.start_tc as f64, rate),
        "tracks": { "OTIO_SCHEMA": "Stack.1", "name": "tracks", "children": tracks, "source_range": null, "effects": [], "markers": [], "metadata": {} },
        "metadata": { "shot": {
            "cadence": plan.rate.fps, "ntsc": plan.rate.ntsc, "taille": plan.size, "images": plan.total,
            "timecode_depart": timecode(plan.start_tc, plan.rate.fps),
        } },
    });
    serde_json::to_string_pretty(&timeline).expect("écriture de l'OTIO") + "\n"
}

// ----------------------------------------------------------------------------------- LISEZ-MOI

/// Le texte de `LISEZ-MOI.txt` : ce qui est dans le dossier, ce qui n'a pas pu être écrit, ce qui reste à
/// vérifier.
pub fn readme(plan: &Plan, opts: &Options) -> String {
    let mut s = String::new();
    s.push_str(&format!("{} : export pour le montage (SHOT)\n", plan.title));
    s.push_str(&"=".repeat(40));
    s.push_str("\n\n");
    s.push_str(&format!(
        "{} plan(s), {} images, {}, {}x{}. Time code de départ : {}.\n\n",
        plan.clips.len(),
        plan.total,
        plan.rate.label(),
        plan.size[0],
        plan.size[1],
        timecode(plan.start_tc, plan.rate.fps)
    ));
    s.push_str("Dans ce dossier\n---------------\n");
    if opts.edl {
        s.push_str("- *.edl : EDL CMX 3600, l'image seulement (une bobine par plan, noms de clips en commentaire).\n");
    }
    if opts.xml {
        s.push_str("- *.xml : Final Cut Pro 7 XML (xmeml), à importer comme une timeline dans Premiere Pro ou DaVinci Resolve.\n");
    }
    if opts.otio {
        s.push_str("- *.otio : OpenTimelineIO (JSON), pour les logiciels et les outils qui le lisent (par exemple otioconvert).\n");
    }
    s.push_str(&format!("- media/ : un fichier {} par plan, nommé comme le plan (SQ01_PL012A = séquence 1, plan 12A).\n", if plan.media == Media::ProRes { "ProRes 422 HQ (.mov)" } else { "H.264 (.mp4)" }));
    if !plan.audio.is_empty() {
        s.push_str("- audio/ : un fichier WAV 48 kHz stéréo par extrait son, effets de SHOT compris (XML et OTIO).\n");
    }
    s.push_str("\nLes chemins des médias sont ");
    s.push_str(if opts.absolute { "absolus (là où l'export a été fait).\n" } else { "relatifs : gardez les fichiers de montage à côté de media/ et audio/.\n" });
    if plan.handles() > 0 {
        s.push_str(&format!(
            "\nPoignées : {} plan(s) participent à un fondu enchaîné ; leur fichier contient quelques images tenues avant et après le plan (la première / la dernière image répétée), que le fondu lit. Le clip de la ligne de temps ne montre que les images du plan.\n",
            plan.handles()
        ));
    }
    if !plan.skipped.is_empty() {
        s.push_str("\nTransitions pas écrites telles quelles\n--------------------------------------\n");
        for k in &plan.skipped {
            let lost: Vec<&str> = ["EDL", "XML", "OTIO"].iter().zip(k.lost).filter(|(_, l)| *l).map(|(n, _)| *n).collect();
            s.push_str(&format!("- {} (perdu dans : {}).\n", k.text, lost.join(", ")));
        }
    }
    if !plan.silent_tracks.is_empty() {
        s.push_str(&format!("\nPistes son pas écrites (muettes, ou une autre est en solo) : {}.\n", plan.silent_tracks.join(", ")));
    }
    if plan.missing_sounds > 0 {
        s.push_str(&format!("\n{} extrait(s) son sans fichier dans le projet : sautés.\n", plan.missing_sounds));
    }
    if plan.audio.is_empty() {
        s.push_str("\nSon : aucun extrait dans l'étendue exportée (ou « sans le son »).\n");
    } else {
        s.push_str("\nSon : chaque extrait est écrit seul, avec son volume, ses fondus, ses clés de volume, son égaliseur et son écho ; les fondus enchaînés entre extraits ne sont pas écrits (l'EDL ne porte pas le son).\n");
    }
    if plan.rate.ntsc {
        s.push_str("\nNTSC : cadence réelle × 1000/1001. Les numéros d'images sont ceux du projet. Le son reste à 48 kHz, sans être ralenti de 0,1 % : à ajuster dans le logiciel de montage si le film est long.\n");
    }
    if plan.odd_rate {
        s.push_str(&format!("\nCadence {} i/s : hors des cadences classiques de l'EDL (24, 25, 30) ; le XML et l'OTIO la portent sans difficulté connue, à vérifier.\n", plan.rate.fps));
    }
    s.push_str(
        "\nÀ vérifier par Anthony (rien de cela n'a été ouvert dans Premiere Pro, DaVinci Resolve ni Avid)\n\
         ------------------------------------------------------------------------------------------\n\
         - L'import de l'EDL, du XML et de l'OTIO : durées, coupes et noms des clips, time code de départ.\n\
         - Les chemins relatifs du XML et de l'OTIO (sinon : refaire l'export avec « chemins absolus »).\n\
         - Les fondus enchaînés : position (centrée, avant ou après la coupe) et durée.\n\
         - Le son : un extrait stéréo apparaît-il sur deux pistes (XML) ? Les volumes ?\n",
    );
    s
}
