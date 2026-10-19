//! Export vers Storyboard Pro (14b-1 ; choix d'Anthony : partir de son projet modèle, dessins en
//! vecteurs si possible — ici en images, les vecteurs viendront en 14b-2). Fichier > Exporter
//! vers Storyboard Pro :
//! 1. Pelure prépare, dans un dossier de travail temporaire, une image PNG par calque et par
//!    image de storyboard (le cadre de la caméra au centre, à la première image de l'image de
//!    storyboard), un WAV par extrait de son (volume de sa piste appliqué) et le script de
//!    construction, données comprises ;
//! 2. il copie le projet modèle et y lance Storyboard Pro en mode batch avec ce script
//!    (`sbbatch`). Le script, qui n'utilise que des fonctions documentées de l'interface de
//!    script, ajoute un plan par plan de Pelure et un panneau par image de storyboard (durée,
//!    légendes), les calques (images importées, nom, visibilité, opacité), les séquences et les
//!    sons, enlève les plans et les pistes son du modèle, puis enregistre le projet sous le titre
//!    dans le dossier choisi.
//!
//! Le compte rendu dit ce qui n'est pas repris : l'animation dans une image de storyboard (seul
//! le premier dessin), la caméra, les modes de fusion, les dossiers de calques (mis à plat), les
//! pistes coupées (exportées quand même).

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::Color32;
use serde_json::{json, Value};

use crate::audio::Pcm;
use crate::document::{Document, LayerKind};
use crate::sbbatch::{self, Batch};

/// Nom du script de construction.
pub const SCRIPT: &str = "construit_pelure.js";
/// Avancement écrit par le script (nombre de plans construits).
pub const PROGRESS: &str = "avancement.txt";
/// Résultat écrit par le script.
pub const RESULT: &str = "resultat.txt";

/// Ce que Pelure envoie à Storyboard Pro.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbOut {
    pub title: String,
    pub fps: u32,
    pub size: [usize; 2],
    /// Séquences à créer, dans l'ordre (aucune si le projet n'en a qu'une, sans nom).
    pub sequences: Vec<OutSequence>,
    pub scenes: Vec<OutScene>,
    pub tracks: Vec<OutTrack>,
    /// Ce qui n'est pas repris tel quel.
    pub notes: Vec<String>,
}

/// Une séquence : du plan `first` au plan `last` (rangs dans `SbOut::scenes`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutSequence {
    pub name: String,
    pub first: usize,
    pub last: usize,
}

/// Un plan de Pelure, qui devient un plan de Storyboard Pro.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutScene {
    pub name: String,
    /// Rang du plan dans le document.
    pub plan: usize,
    pub panels: Vec<OutPanel>,
}

/// Une image de storyboard, qui devient un panneau.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutPanel {
    /// Première image dans le plan.
    pub start: usize,
    pub frames: usize,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
    /// Du fond au premier plan.
    pub layers: Vec<OutLayer>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutLayer {
    /// Rang du calque dans le plan.
    pub layer: usize,
    pub name: String,
    pub vector: bool,
    pub visible: bool,
    /// 0 à 1, dossier compris.
    pub opacity: f32,
    /// Le calque a un dessin à la première image du panneau.
    pub drawn: bool,
    /// Image du calque (chemin relatif au dossier des données), une fois calculée ; aucune :
    /// calque vide.
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutTrack {
    pub name: String,
    pub clips: Vec<OutClip>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutClip {
    /// Rangs de la piste et de l'extrait dans le document.
    pub track: usize,
    pub clip: usize,
    /// Première image du montage.
    pub start: usize,
    /// Fichier WAV (chemin relatif au dossier des données).
    pub file: String,
}

/// Le titre en nom de dossier : sans les caractères interdits par Windows, ni point ou espace au
/// bout ; « Projet SHOT » s'il ne reste rien.
pub fn safe_name(title: &str) -> String {
    let s: String = title.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() {
        "Projet SHOT".into()
    } else {
        s
    }
}

/// Ce qui partira vers Storyboard Pro (sans les images, calculées ensuite).
pub fn prepare(doc: &Document, title: &str) -> SbOut {
    let mut out = SbOut { title: title.trim().to_string(), fps: doc.fps, size: doc.frame, ..Default::default() };
    let (mut animated, mut camera, mut blends, mut folders, mut colours) = (0, 0, 0, false, 0);
    for (i, p) in doc.panels.iter().enumerate() {
        if !p.camera.is_empty() {
            camera += 1;
        }
        folders |= p.layers.iter().any(|l| l.folder);
        blends += p.layers.iter().filter(|l| !l.folder && l.blend != crate::blend::BlendMode::Normal).count();
        colours += p.layers.iter().filter(|l| l.couleur.is_some()).count();
        let mut scene = OutScene { name: doc.scene_label(p.scene), plan: i, panels: Vec::new() };
        for (id, start, end) in crate::board::plan_boards(doc, i) {
            if end <= start {
                continue;
            }
            let (action, dialogue, notes) = match p.marks.iter().find(|m| m.id == id) {
                Some(m) => (m.action.clone(), m.dialogue.clone(), m.notes.clone()),
                None => (p.action.clone(), p.dialogue.clone(), p.notes.clone()),
            };
            let mut layers = Vec::new();
            for (li, l) in p.layers.iter().enumerate() {
                if l.folder {
                    continue;
                }
                let first = crate::video::cel_at(l, start, doc.fps);
                if (start + 1..end).any(|f| crate::video::cel_at(l, f, doc.fps) != first) {
                    animated += 1;
                }
                let parent = crate::folders::parent_index(&p.layers, li).map(|fi| &p.layers[fi]);
                layers.push(OutLayer {
                    layer: li,
                    name: l.name.clone(),
                    vector: l.kind == LayerKind::Vector,
                    visible: l.visible && parent.is_none_or(|f| f.visible),
                    opacity: l.opacity.clamp(0.0, 1.0) * parent.map_or(1.0, |f| f.opacity.clamp(0.0, 1.0)),
                    drawn: first.is_some(),
                    image: None,
                });
            }
            scene.panels.push(OutPanel { start, frames: end - start, action, dialogue, notes, layers });
        }
        out.scenes.push(scene);
    }
    // Séquences : seulement si le projet en a plusieurs, ou une qui a un nom.
    let order = doc.sequence_order();
    let named = doc.sequences.iter().any(|s| order.contains(&s.id) && !s.name.trim().is_empty());
    if order.len() > 1 || named {
        for (k, scene) in out.scenes.iter().enumerate() {
            let seq = doc.sequence_of(scene.plan);
            let name = doc.sequence_label(seq);
            match out.sequences.last_mut() {
                Some(s) if s.name == name && doc.sequence_of(out.scenes[s.last].plan) == seq => s.last = k,
                _ => out.sequences.push(OutSequence { name, first: k, last: k }),
            }
        }
    }
    let mut muted = 0;
    for (ti, t) in doc.audio_tracks.iter().enumerate() {
        if t.clips.is_empty() {
            continue;
        }
        if t.muted {
            muted += 1;
        }
        let clips = t
            .clips
            .iter()
            .enumerate()
            .map(|(ci, c)| OutClip { track: ti, clip: ci, start: c.start, file: format!("sons/piste{:02}_{:03}.wav", ti + 1, ci + 1) })
            .collect();
        out.tracks.push(OutTrack { name: t.name.clone(), clips });
    }
    if animated > 0 {
        out.notes.push(format!("Animation : {animated} calque(s) changent de dessin pendant une image de storyboard ; seul le premier dessin est repris."));
    }
    if camera > 0 {
        out.notes.push(format!("Caméra : {camera} plan(s) ont des clés de caméra, non reprises (les calques sont vus par la caméra au centre)."));
    }
    if blends > 0 {
        out.notes.push(format!("Modes de fusion : {blends} calque(s) ne sont pas en mode normal ; ils arrivent en mode normal."));
    }
    if folders {
        out.notes.push("Dossiers de calques : mis à plat (leur visibilité et leur opacité passent à leurs calques).".into());
    }
    if colours > 0 {
        out.notes.push(format!("Calques de couleur : {colours} calque(s) exportés comme des calques bitmap ordinaires, sous leur trait (le lien trait / couleur de SHOT n'existe pas dans Storyboard Pro : à vérifier)."));
    }
    if muted > 0 {
        out.notes.push(format!("Son : {muted} piste(s) coupée(s) dans SHOT, exportées quand même (et pas coupées dans Storyboard Pro)."));
    }
    // Transitions et effets des sons (`transitions.rs`, `audiofx.rs`).
    let zones = crate::transitions::zones(doc).len();
    if zones > 0 {
        out.notes.push(format!("Transitions : {zones} transition(s) du montage non reprises (comment Storyboard Pro range les siennes : à vérifier) ; l'export du film les contient."));
    }
    let fx_clips = doc.audio_tracks.iter().flat_map(|t| &t.clips).filter(|c| !c.fx.is_default()).count();
    if fx_clips > 0 {
        out.notes.push(format!("Son : les effets de {fx_clips} extrait(s) (fondus, volume, clés, égaliseur, écho) sont appliqués dans les fichiers son exportés ; les fondus enchaînés entre extraits ne le sont pas."));
    }
    out
}

/// Nombres pour la fenêtre : plans, images de storyboard, calques avec dessin, extraits de son.
pub fn counts(out: &SbOut) -> (usize, usize, usize, usize) {
    let panels = out.scenes.iter().map(|s| s.panels.len()).sum();
    let drawn = out.scenes.iter().flat_map(|s| &s.panels).flat_map(|p| &p.layers).filter(|l| l.drawn).count();
    let clips = out.tracks.iter().map(|t| t.clips.len()).sum();
    (out.scenes.len(), panels, drawn, clips)
}

/// Une chaîne en littéral JavaScript (ECMAScript 3 : ni U+2028 ni U+2029 tels quels).
fn js(v: &Value) -> String {
    v.to_string().replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

/// Les données du script, en objet JavaScript.
pub fn script_data(out: &SbOut) -> Value {
    let plans: Vec<Value> = out
        .scenes
        .iter()
        .map(|s| {
            let panneaux: Vec<Value> = s
                .panels
                .iter()
                .map(|p| {
                    let calques: Vec<Value> = p
                        .layers
                        .iter()
                        .map(|l| {
                            json!({
                                "nom": l.name,
                                "vecteur": l.vector,
                                "visible": l.visible,
                                "opacite": (l.opacity * 100.0).round(),
                                "image": l.image.clone().unwrap_or_default(),
                            })
                        })
                        .collect();
                    json!({"duree": p.frames, "dialogue": p.dialogue, "action": p.action, "notes": p.notes, "calques": calques})
                })
                .collect();
            json!({"nom": s.name, "panneaux": panneaux})
        })
        .collect();
    let sequences: Vec<Value> = out.sequences.iter().map(|s| json!({"nom": s.name, "premier": s.first, "dernier": s.last})).collect();
    let pistes: Vec<Value> = out
        .tracks
        .iter()
        .map(|t| {
            let extraits: Vec<Value> = t.clips.iter().map(|c| json!({"fichier": c.file, "debut": c.start})).collect();
            json!({"nom": t.name, "extraits": extraits})
        })
        .collect();
    json!({"titre": out.title, "plans": plans, "sequences": sequences, "pistes": pistes})
}

/// Le script de construction : `data` est le dossier des données (images, sons, avancement,
/// résultat), `dest` le dossier du projet à enregistrer (qui ne doit pas exister).
pub fn script(out: &SbOut, data: &Path, dest: &Path) -> String {
    let path = |p: &Path| js(&Value::String(p.to_string_lossy().replace('\\', "/")));
    SCRIPT_TEXT.replace("@DONNEES@", &path(data)).replace("@DESTINATION@", &path(dest)).replace("@PELURE@", &js(&script_data(out)))
}

/// Ce que le script a écrit dans `RESULT`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildReport {
    /// Plans construits.
    pub scenes: usize,
    /// Enregistré dans le dossier choisi (`project.saveAs`).
    pub saved_as: bool,
    /// À défaut, enregistré sur place, dans la copie du modèle (`project.saveAll`).
    pub saved_all: bool,
    pub fps: Option<f64>,
    pub size: Option<[usize; 2]>,
    pub remarks: Vec<String>,
    pub error: Option<String>,
}

pub fn parse_result(text: &str) -> Result<BuildReport, String> {
    let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
    if lines.next().map(|l| l.split('\t').next()) != Some(Some("pelure-construction")) {
        return Err("le résultat du script est illisible".into());
    }
    let mut r = BuildReport::default();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "plans" => r.scenes = f.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
            "enregistre" => {
                r.saved_as = f.get(1) == Some(&"1");
                r.saved_all = f.get(2) == Some(&"1");
            }
            "projet" => {
                r.fps = f.get(1).and_then(|v| v.parse().ok());
                if let (Some(w), Some(h)) = (f.get(2).and_then(|v| v.parse().ok()), f.get(3).and_then(|v| v.parse().ok())) {
                    r.size = Some([w, h]);
                }
            }
            "remarque" if f.len() > 1 => r.remarks.push(f[1..].join(" ")),
            "erreur" if f.len() > 1 => r.error = Some(f[1..].join(" ")),
            _ => {}
        }
    }
    Ok(r)
}

/// Le projet construit.
#[derive(Clone, Debug)]
pub struct Built {
    /// Le `.sboard` enregistré.
    pub project: PathBuf,
    pub report: BuildReport,
}

/// Un son à écrire : le son, ses échantillons de `from` à `to`, son gain, et son fichier. Un extrait
/// qui a des effets (`audiofx.rs`) arrive déjà refait dans `baked` (fondus, volume, clés, égaliseur,
/// écho) : Storyboard Pro n'a pas nos effets, il reçoit le son tel qu'on l'entend.
struct SoundOut {
    pcm: Arc<Pcm>,
    from: usize,
    to: usize,
    gain: f32,
    file: String,
    baked: Option<Vec<[f32; 2]>>,
}

fn sounds_of(doc: &Document, out: &SbOut) -> Vec<SoundOut> {
    let mut v = Vec::new();
    for t in &out.tracks {
        for c in &t.clips {
            let Some(track) = doc.audio_tracks.get(c.track) else { continue };
            let Some(clip) = track.clips.get(c.clip) else { continue };
            let Some(sound) = doc.sounds.get(&clip.sound) else { continue };
            let (from, len) = crate::export::sample_span(doc.fps, clip.offset, clip.length);
            let baked = (!clip.fx.is_default()).then(|| crate::audiofx::bake_clip(doc, clip));
            v.push(SoundOut { pcm: sound.pcm.clone(), from: from as usize, to: (from + len) as usize, gain: track.volume, file: c.file.clone(), baked });
        }
    }
    v
}

fn write_sounds(data: &Path, sounds: &[SoundOut]) -> Result<(), String> {
    if !sounds.is_empty() {
        std::fs::create_dir_all(data.join("sons")).map_err(|e| format!("dossier des sons impossible ({e})"))?;
    }
    for s in sounds {
        let mut samples = match &s.baked {
            Some(b) => b.clone(),
            None => crate::stretch::slice(&s.pcm, s.from, s.to),
        };
        crate::stretch::gain(&mut samples, s.gain);
        let (_, bytes) = crate::stretch::to_sound(&samples);
        let path = data.join(&s.file);
        std::fs::write(&path, bytes).map_err(|e| format!("{} : {e}", path.display()))?;
    }
    Ok(())
}

/// Construction par Storyboard Pro (sur un autre fil) : copie du modèle, sons, lancement du
/// script, puis le projet enregistré.
fn build(exe: &Path, template: &Path, work: &Path, dest: &Path, sounds: Vec<SoundOut>, set: &dyn Fn(&str), done: &AtomicUsize, cancel: &AtomicBool) -> Result<Built, String> {
    let src = template.parent().ok_or("dossier du projet modèle introuvable")?;
    let name = template.file_name().ok_or("nom du projet modèle introuvable")?;
    let copy = work.join("projet");
    let data = work.join("donnees");
    sbbatch::copy_project(src, &copy).map_err(|e| format!("copie du projet modèle impossible ({e})"))?;
    set("Écriture des sons…");
    write_sounds(&data, &sounds)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("export arrêté".into());
    }
    set("Storyboard Pro construit le projet…");
    let count = || std::fs::read_to_string(data.join(PROGRESS)).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
    sbbatch::launch(exe, &copy.join(name), &data.join(SCRIPT), &data.join("journal.txt"), cancel, done, &count)
        .map_err(|e| if e == "arrêté" { "export arrêté".to_string() } else { e })?;
    set("Vérification…");
    let tail = || sbbatch::journal_tail(&data);
    let text = std::fs::read(data.join(RESULT)).map_err(|_| format!("le script n'a pas fini. Journal de Storyboard Pro : {}", tail()))?;
    let report = parse_result(&String::from_utf8_lossy(&text))?;
    if let Some(e) = &report.error {
        return Err(format!("erreur du script : {e}. Journal : {}", tail()));
    }
    // Enregistré dans le dossier choisi ; à défaut, la copie enregistrée sur place y est copiée.
    if !report.saved_as {
        if !report.saved_all {
            return Err(format!("Storyboard Pro n'a pas enregistré le projet. Journal : {}", tail()));
        }
        set("Copie du projet dans le dossier choisi…");
        sbbatch::copy_project(&copy, dest).map_err(|e| format!("copie du projet construit impossible ({e})"))?;
    }
    let project = find_sboard(dest).ok_or_else(|| format!("aucun projet dans « {} ». Journal : {}", dest.display(), tail()))?;
    Ok(Built { project, report })
}

/// Le `.sboard` du dossier `dir`.
fn find_sboard(dir: &Path) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sboard")))
        .collect();
    v.sort();
    v.into_iter().next()
}

/// Une image de calque à écrire.
type Png = (PathBuf, Arc<Vec<Color32>>);

/// Où en est l'export.
pub enum Stage {
    /// Pelure calcule les images des calques (`next` sur le total).
    Images,
    /// Storyboard Pro travaille.
    Storyboard(Batch<Built>),
    Done(Result<Built, String>),
}

/// Un export vers Storyboard Pro en cours.
pub struct SbExportJob {
    pub out: SbOut,
    pub stage: Stage,
    exe: PathBuf,
    template: PathBuf,
    /// Dossier du projet à enregistrer.
    pub dest: PathBuf,
    pub work: PathBuf,
    /// Images à calculer : (plan, panneau, calque) dans `out`.
    tasks: Vec<(usize, usize, usize)>,
    pub next: usize,
    /// Mêmes pixels, même fichier.
    seen: HashMap<u64, String>,
    tx: Option<SyncSender<Png>>,
    pending: Option<Png>,
    sent: usize,
    written: Arc<AtomicUsize>,
    error: Arc<Mutex<Option<String>>>,
    pub started: Instant,
}

impl SbExportJob {
    /// Prépare l'export de `out` avec Storyboard Pro `exe`, à partir du projet modèle
    /// `template`, vers le dossier `dest` (qui ne doit pas exister).
    pub fn start(out: SbOut, exe: PathBuf, template: PathBuf, dest: PathBuf) -> Result<SbExportJob, String> {
        let work = sbbatch::work_dir(&safe_name(&out.title));
        let images = work.join("donnees").join("images");
        std::fs::create_dir_all(&images).map_err(|e| format!("dossier de travail impossible ({e})"))?;
        let tasks = out
            .scenes
            .iter()
            .enumerate()
            .flat_map(|(s, sc)| sc.panels.iter().enumerate().flat_map(move |(k, p)| p.layers.iter().enumerate().filter(|(_, l)| l.drawn).map(move |(j, _)| (s, k, j))))
            .collect();
        let (tx, rx) = sync_channel::<Png>(8);
        let rx = Arc::new(Mutex::new(rx));
        let written = Arc::new(AtomicUsize::new(0));
        let error = Arc::new(Mutex::new(None));
        let [w, h] = out.size;
        let n = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(2, 6);
        for _ in 0..n {
            let (rx, written, error) = (rx.clone(), written.clone(), error.clone());
            std::thread::spawn(move || loop {
                let msg = rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                let Ok((path, img)) = msg else { break };
                if let Err(e) = crate::io::write_png(&path, &img, w, h) {
                    *error.lock().unwrap_or_else(|e| e.into_inner()) = Some(format!("{} : {e}", path.display()));
                }
                written.fetch_add(1, Ordering::Relaxed);
            });
        }
        Ok(SbExportJob {
            out,
            stage: Stage::Images,
            exe,
            template,
            dest,
            work,
            tasks,
            next: 0,
            seen: HashMap::new(),
            tx: Some(tx),
            pending: None,
            sent: 0,
            written,
            error,
            started: Instant::now(),
        })
    }

    pub fn total_images(&self) -> usize {
        self.tasks.len()
    }

    /// Calcule des images pendant `budget` au plus ; une fois toutes écrites, lance Storyboard
    /// Pro. Le résultat arrive dans `stage`.
    pub fn step(&mut self, doc: &Document, budget: Duration) {
        match &self.stage {
            Stage::Images => {}
            Stage::Storyboard(b) => {
                if let Some(res) = b.finished() {
                    if res.is_ok() {
                        let _ = std::fs::remove_dir_all(&self.work);
                    }
                    self.stage = Stage::Done(res);
                }
                return;
            }
            Stage::Done(_) => return,
        }
        let t0 = Instant::now();
        while self.next < self.tasks.len() && t0.elapsed() < budget {
            let png = match self.pending.take() {
                Some(p) => Some(p),
                None => self.render(doc, self.next),
            };
            if let Some(png) = png {
                let Some(tx) = &self.tx else { return };
                match tx.try_send(png) {
                    Ok(()) => self.sent += 1,
                    Err(TrySendError::Full(p)) => {
                        // L'écriture est en retard : on garde l'image pour la prochaine fois.
                        self.pending = Some(p);
                        return;
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        self.fail("l'écriture des images s'est arrêtée".into());
                        return;
                    }
                }
            }
            self.next += 1;
        }
        if self.next < self.tasks.len() || self.written.load(Ordering::Relaxed) < self.sent {
            return;
        }
        self.tx = None;
        let error = self.error.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(e) = error {
            self.fail(format!("image non écrite : {e}"));
            return;
        }
        // Toutes les images sont écrites : le script, puis Storyboard Pro.
        let data = self.work.join("donnees");
        if let Err(e) = std::fs::write(data.join(SCRIPT), script(&self.out, &data, &self.dest)) {
            self.fail(format!("écriture du script impossible ({e})"));
            return;
        }
        let sounds = sounds_of(doc, &self.out);
        let (exe, template, work, dest) = (self.exe.clone(), self.template.clone(), self.work.clone(), self.dest.clone());
        let total = self.out.scenes.len();
        self.stage = Stage::Storyboard(Batch::spawn(self.work.clone(), total, "Copie du projet modèle…", move |set, done, cancel| {
            build(&exe, &template, &work, &dest, sounds, set, done, cancel)
        }));
    }

    /// L'image du calque de la tâche `i`, à écrire (aucune : déjà écrite pareille, ou vide).
    fn render(&mut self, doc: &Document, i: usize) -> Option<Png> {
        let (s, k, j) = self.tasks[i];
        let (plan, start, layer) = {
            let sc = &self.out.scenes[s];
            (sc.plan, sc.panels[k].start, sc.panels[k].layers[j].layer)
        };
        let lp = crate::render::layer_plan_at(doc, plan, layer, start)?;
        let img = crate::render::camera_image(doc, &lp, &doc.default_camera());
        if img.iter().all(|c| c.a() == 0) {
            return None;
        }
        let mut h = DefaultHasher::new();
        bytemuck::cast_slice::<Color32, u8>(&img).hash(&mut h);
        let key = h.finish();
        if let Some(file) = self.seen.get(&key) {
            self.out.scenes[s].panels[k].layers[j].image = Some(file.clone());
            return None;
        }
        let file = format!("images/{:04}_{:03}_{:03}.png", s + 1, k + 1, j + 1);
        self.seen.insert(key, file.clone());
        self.out.scenes[s].panels[k].layers[j].image = Some(file.clone());
        Some((self.work.join("donnees").join(&file), Arc::new(img)))
    }

    fn fail(&mut self, e: String) {
        self.tx = None;
        self.stage = Stage::Done(Err(e));
    }

    /// Arrête l'export ; le dossier de travail est effacé.
    pub fn cancel(&mut self) {
        match &self.stage {
            Stage::Storyboard(b) => b.cancel(),
            Stage::Images => {
                self.tx = None;
                self.pending = None;
                self.stage = Stage::Done(Err("export arrêté".into()));
                let _ = std::fs::remove_dir_all(&self.work);
            }
            Stage::Done(_) => {}
        }
    }

    pub fn running(&self) -> bool {
        !matches!(self.stage, Stage::Done(_))
    }
}

/// Le script (ECMAScript 3, comme celui de l'import : l'objet `JSON` n'est pas sûr d'exister ;
/// les données sont écrites en littéral). Les fonctions sont celles de la documentation de
/// l'interface de script de Storyboard Pro : StoryboardManager, LayerManager, CaptionManager,
/// SoundTrackManager, project.
const SCRIPT_TEXT: &str = r#"// SHOT : construction d'un projet Storyboard Pro a partir d'un projet SHOT.
// Lance par SHOT en mode batch, sur une copie du projet modele. Les images, les sons,
// l'avancement et le resultat sont dans PELURE_DONNEES ; le projet est enregistre dans
// PELURE_DESTINATION.
var PELURE_DONNEES = @DONNEES@;
var PELURE_DESTINATION = @DESTINATION@;
var PELURE = @PELURE@;

function pelureNettoie(s)
{
  return String(s).replace(/[\t\r\n]/g, " ");
}

function pelureEcrit(nom, lignes)
{
  var f = new File(PELURE_DONNEES + "/" + nom);
  f.open(FileAccess.WriteOnly);
  for (var i = 0; i < lignes.length; ++i) f.writeLine(lignes[i]);
  f.close();
}

function pelureHtml(s)
{
  return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\r?\n/g, "<br/>");
}

// Les legendes du modele qui recoivent les textes de Pelure (ajoutees s'il ne les a pas).
function pelureLegendes(cm, remarques)
{
  var noms = new Array(0);
  var n = cm.numberOfPanelCaptions();
  for (var i = 0; i < n; ++i) noms.push(String(cm.nameOfPanelCaption(i)));
  var cherche = function (test, defaut)
  {
    for (var i = 0; i < noms.length; ++i) if (test(noms[i].toLowerCase())) return noms[i];
    if (cm.addPanelCaption(defaut)) { noms.push(defaut); return defaut; }
    remarques.push("legende " + defaut + " absente du modele et impossible a ajouter");
    return "";
  };
  return {
    dialogue: cherche(function (s) { return s.indexOf("dialog") >= 0; }, "Dialog"),
    action: cherche(function (s) { return s.indexOf("action") >= 0; }, "Action Notes"),
    notes: cherche(function (s) { return s.indexOf("note") >= 0 && s.indexOf("action") < 0; }, "Notes")
  };
}

// Un texte de legende. Avec des retours a la ligne ou des signes < > & : en HTML (les legendes
// sont en HTML dans le projet) ; si Storyboard Pro l'a pris pour du texte (balises echappees a
// la relecture), il est redonne en texte simple.
function pelureLegende(cm, nom, panelId, texte)
{
  if (!nom || !texte) return;
  if (!/[<>&\r\n]/.test(texte)) { cm.setPanelCaptionText(nom, panelId, texte); return; }
  cm.setPanelCaptionText(nom, panelId, pelureHtml(texte));
  var lu = String(cm.textOfPanelCaption(nom, panelId));
  if (lu.indexOf("&lt;br") >= 0) cm.setPanelCaptionText(nom, panelId, texte);
}

function pelureNoms(lm, panelId)
{
  var out = new Array(0);
  var n = lm.numberOfLayers(panelId);
  for (var i = 0; i < n; ++i) out.push(String(lm.layerName(panelId, i)));
  return out;
}

// Rang du calque apparu entre deux listes de noms.
function pelureNouveau(avant, apres)
{
  for (var i = 0; i < apres.length; ++i) if (i >= avant.length || apres[i] != avant[i]) return i;
  return -1;
}

// Les calques d'un panneau, du fond au premier plan : chacun prend d'abord un nom provisoire
// ("pelure~rang"), pour le retrouver ; les calques vides que le panneau neuf avait deja s'en
// vont ; puis chacun prend son nom, sa visibilite et son opacite.
function pelureCalques(sb, lm, panelId, calques, remarques, ou)
{
  var faits = 0;
  for (var j = 0; j < calques.length; ++j)
  {
    var c = calques[j];
    var avant = pelureNoms(lm, panelId);
    var ok = false;
    try
    {
      if (c.image) ok = lm.importImageAsLayer(panelId, PELURE_DONNEES + "/" + c.image, "Composite");
      else if (avant.length > 0 && c.vecteur) ok = lm.addVectorLayer(panelId, 0, true, "pelure~" + j);
      else if (avant.length > 0) ok = lm.addBitmapLayer(panelId, 0, true, "pelure~" + j);
    }
    catch (e) { remarques.push(ou + ", calque " + c.nom + " : " + e); }
    var i = ok ? pelureNouveau(avant, pelureNoms(lm, panelId)) : -1;
    if (i < 0) { remarques.push(ou + ", calque " + c.nom + " non cree"); continue; }
    lm.renameLayer(panelId, i, "pelure~" + j);
    ++faits;
  }
  if (faits == 0) return;
  for (var i = lm.numberOfLayers(panelId) - 1; i >= 0; --i)
  {
    var nom = String(lm.layerName(panelId, i));
    if (nom.indexOf("pelure~") != 0 && lm.isEmpty(panelId, i)) lm.deleteLayer(panelId, i);
  }
  var debut = sb.panelStartFrame(panelId);
  for (var i = lm.numberOfLayers(panelId) - 1; i >= 0; --i)
  {
    var nom = String(lm.layerName(panelId, i));
    if (nom.indexOf("pelure~") != 0) continue;
    var c = calques[parseInt(nom.substring(7), 10)];
    if (!c) continue;
    lm.renameLayer(panelId, i, c.nom);
    if (!c.visible) lm.setLayerVisible(panelId, i, false);
    if (c.opacite < 100)
    {
      try { lm.setLayerOpacity(panelId, i, c.opacite, debut); }
      catch (e) { remarques.push(ou + ", opacite du calque " + c.nom + " : " + e); }
    }
  }
}

function pelureConstruit()
{
  var sb = new StoryboardManager();
  var lm = new LayerManager();
  var cm = new CaptionManager();
  var sm = new SoundTrackManager();
  var remarques = new Array(0);
  var lignes = new Array(0);
  lignes.push("pelure-construction\t1");
  var plans = new Array(0);
  var enregistre = false;
  var tout = false;
  try
  {
    // Ce que le modele avait deja.
    var anciens = new Array(0);
    var n0 = sb.numberOfScenesInProject();
    for (var i = 0; i < n0; ++i) anciens.push(sb.sceneInProject(i));
    var pistes0 = new Array(0);
    var np = sm.numberOfSoundTracks();
    for (var i = 0; i < np; ++i) pistes0.push(sm.nameOfSoundTrack(i));
    var legendes = pelureLegendes(cm, remarques);

    for (var s = 0; s < PELURE.plans.length; ++s)
    {
      var plan = PELURE.plans[s];
      // Un nom refuse (deja pris par un plan du modele) : un nom provisoire, corrige plus bas.
      var avant = sb.numberOfScenesInProject();
      sb.appendScene(plan.nom);
      if (sb.numberOfScenesInProject() == avant) sb.appendScene("shot" + (s + 1));
      if (sb.numberOfScenesInProject() == avant)
      {
        remarques.push("plan " + plan.nom + " non cree");
        plans.push("");
        continue;
      }
      var sceneId = sb.sceneInProject(sb.numberOfScenesInProject() - 1);
      plans.push(sceneId);
      var panelId = sb.panelInScene(sceneId, 0);
      for (var k = 0; k < plan.panneaux.length; ++k)
      {
        var p = plan.panneaux[k];
        if (k > 0) panelId = sb.insertPanel(true, panelId, "" + (k + 1));
        if (!panelId) { remarques.push("plan " + plan.nom + " : panneau " + (k + 1) + " non cree"); break; }
        sb.setPanelDuration(panelId, p.duree);
        var ou = "plan " + plan.nom + ", panneau " + (k + 1);
        pelureCalques(sb, lm, panelId, p.calques, remarques, ou);
        pelureLegende(cm, legendes.dialogue, panelId, p.dialogue);
        pelureLegende(cm, legendes.action, panelId, p.action);
        pelureLegende(cm, legendes.notes, panelId, p.notes);
      }
      pelureEcrit("avancement.txt", ["" + (s + 1)]);
      MessageLog.trace("SHOT : plan " + (s + 1) + " / " + PELURE.plans.length);
    }

    // Les plans du modele s'en vont ; les plans de Pelure prennent leurs noms, maintenant
    // libres.
    for (var i = 0; i < anciens.length; ++i)
      if (!sb.deleteScene(anciens[i])) remarques.push("plan du modele " + anciens[i] + " non supprime");
    var faits = new Array(0);
    for (var s = 0; s < plans.length; ++s)
    {
      if (!plans[s]) continue;
      faits.push(plans[s]);
      var voulu = PELURE.plans[s].nom;
      if (String(sb.nameOfScene(plans[s])) == voulu) continue;
      var nom = sb.getValidSceneName(plans[s], voulu);
      if (!nom || !sb.renameScene(plans[s], nom)) remarques.push("plan " + voulu + " : nom refuse (" + sb.nameOfScene(plans[s]) + ")");
    }

    for (var q = 0; q < PELURE.sequences.length; ++q)
    {
      var sq = PELURE.sequences[q];
      if (!plans[sq.premier] || !plans[sq.dernier]) { remarques.push("sequence " + sq.nom + " non creee (plan manquant)"); continue; }
      var id = sb.createSequence(plans[sq.premier], plans[sq.dernier]);
      if (!id) { remarques.push("sequence " + sq.nom + " non creee"); continue; }
      if (sq.nom && !sb.renameSequence(id, sq.nom)) remarques.push("sequence " + sq.nom + " : nom refuse");
    }

    for (var i = 0; i < pistes0.length; ++i) sm.deleteSoundTrack(pistes0[i]);
    if (faits.length > 0)
    {
      var base = sb.panelStartFrame(sb.panelInScene(faits[0], 0));
      for (var t = 0; t < PELURE.pistes.length; ++t)
      {
        var col = sm.addSoundTrack();
        var ex = PELURE.pistes[t].extraits;
        for (var c = 0; c < ex.length; ++c)
          if (!sm.importSoundBuffer(col, PELURE_DONNEES + "/" + ex[c].fichier, base + ex[c].debut))
            remarques.push("son " + ex[c].fichier + " non importe");
      }
    }

    try { enregistre = project.saveAs(PELURE_DESTINATION); }
    catch (e) { remarques.push("enregistrement sous : " + e); }
    if (!enregistre)
    {
      try { tout = project.saveAll(); }
      catch (e) { remarques.push("enregistrement : " + e); }
    }
  }
  catch (e)
  {
    lignes.push("erreur\t" + pelureNettoie(e));
  }
  var n = 0;
  for (var s = 0; s < plans.length; ++s) if (plans[s]) ++n;
  lignes.push("plans\t" + n);
  lignes.push("enregistre\t" + (enregistre ? 1 : 0) + "\t" + (tout ? 1 : 0));
  lignes.push("projet\t" + project.getFrameRate() + "\t" + project.defaultResolutionX() + "\t" + project.defaultResolutionY());
  for (var i = 0; i < remarques.length; ++i) lignes.push("remarque\t" + pelureNettoie(remarques[i]));
  pelureEcrit("resultat.txt", lignes);
  MessageLog.trace("SHOT : " + n + " plans construits");
}

pelureConstruit();
"#;
