//! Bibliothèque de bruitages (Anthony, 2026-10-19) : les dossiers de sons de l'utilisateur, listés
//! avec une recherche, une durée et une écoute au clic, pour les poser dans le montage.
//!
//! Rien ne pèse sur le fil de l'interface :
//!
//! - les dossiers se parcourent sur un fil à part (`Bibliotheque::scan`), par paquets que l'interface
//!   reprend à chaque image ; la bibliothèque se parcourt à la première ouverture du panneau et à la
//!   demande, jamais au lancement de SHOT ;
//! - la durée d'un son se lit sur un autre fil, **seulement pour les lignes que l'on voit**
//!   (`Sonde`) : l'en-tête du fichier suffit (symphonia), sans décoder le son ; un MP3 sans durée
//!   annoncée est décodé (s'il est petit) ;
//! - l'écoute décode le son sur un fil à part, puis le joue par la sortie audio de SHOT (`Ecoute`,
//!   sur un `Sortie` : un faux se glisse dans les essais, qui ne produisent aucun son).
//!
//! Les dossiers sont un réglage de l'application (`reglages.toml`, section `[bruitages]`) : ils
//! suivent l'utilisateur d'un projet à l'autre. Un son posé dans le montage est **copié dans le
//! projet** comme tout son importé (`soundtrack::import_sound_path`) ; la bibliothèque n'est pas
//! enregistrée avec le projet.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::audio::{self, MixClip, Pcm};
use crate::project::SOUND_EXTS;

/// Le plus de sons gardés (un dossier de disque entier ne fait pas grossir la liste sans fin).
pub const MAX_ENTRIES: usize = 50_000;
/// Profondeur des sous-dossiers parcourus.
pub const MAX_DEPTH: usize = 8;
/// Taille au-delà de laquelle on ne décode pas un fichier pour en lire la durée.
const PROBE_DECODE_MAX: u64 = 16 << 20;
/// Demandes de durée en vol au plus.
const PROBE_QUEUE: usize = 48;

/// Les réglages de la bibliothèque, dans `reglages.toml`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BruitagesReglages {
    /// Les dossiers de sons de l'utilisateur.
    #[serde(rename = "dossiers", skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<PathBuf>,
}

impl BruitagesReglages {
    /// Ajoute un dossier (une fois). Faux s'il y était déjà, ou s'il en contient un autre déjà là
    /// (les sons seraient listés deux fois).
    pub fn add(&mut self, dir: PathBuf) -> bool {
        if self.folders.iter().any(|f| same(f, &dir) || dir.starts_with(f)) {
            return false;
        }
        // Un dossier qui en contient d'autres les remplace.
        self.folders.retain(|f| !f.starts_with(&dir));
        self.folders.push(dir);
        true
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.folders.len() {
            self.folders.remove(index);
        }
    }
}

fn same(a: &Path, b: &Path) -> bool {
    a == b || (a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()))
}

/// Un son de la bibliothèque.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    /// Nom du fichier sans son extension.
    pub name: String,
    /// Le dossier de la bibliothèque (son rang dans les réglages) et le chemin depuis lui.
    pub folder: usize,
    pub rel: String,
    pub bytes: u64,
}

impl Entry {
    /// Le texte où chercher : sous-dossier et nom, en minuscules.
    fn haystack(&self) -> String {
        format!("{} {}", self.rel, self.name).to_lowercase()
    }
}

/// Un fichier est-il un son qu'on sait lire (par son extension) ?
pub fn is_sound(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| SOUND_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Les sons de `dir` et de ses sous-dossiers, dans l'ordre (d'abord les fichiers d'un dossier, par
/// nom, puis ses sous-dossiers). `send` reçoit des paquets ; faux rendu si `limit` a coupé la liste.
pub fn scan_dir(dir: &Path, folder: usize, cancel: &AtomicBool, limit: usize, mut send: impl FnMut(Vec<Entry>)) -> bool {
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut batch = Vec::new();
    let mut count = 0;
    while let Some((here, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(read) = std::fs::read_dir(&here) else { continue };
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        for e in read.flatten() {
            let path = e.path();
            let Ok(kind) = e.file_type() else { continue };
            let hidden = e.file_name().to_string_lossy().starts_with('.');
            if hidden {
                continue;
            }
            if kind.is_dir() && depth < MAX_DEPTH {
                dirs.push(path);
            } else if kind.is_file() && is_sound(&path) {
                files.push((path, e.metadata().map_or(0, |m| m.len())));
            }
        }
        files.sort_by(|a, b| natural_key(&a.0).cmp(&natural_key(&b.0)));
        dirs.sort_by(|a, b| natural_key(a).cmp(&natural_key(b)));
        for (path, bytes) in files {
            if count >= limit {
                send(std::mem::take(&mut batch));
                return true;
            }
            let rel = path.parent().and_then(|p| p.strip_prefix(dir).ok()).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            batch.push(Entry { path, name, folder, rel, bytes });
            count += 1;
            if batch.len() >= 200 {
                send(std::mem::take(&mut batch));
            }
        }
        // (Le dernier sous-dossier doit sortir en premier de la pile pour garder l'ordre.)
        for d in dirs.into_iter().rev() {
            stack.push((d, depth + 1));
        }
    }
    send(batch);
    false
}

/// Clé de tri d'un nom : minuscules, pour que « Bam » et « bang » se rangent ensemble.
fn natural_key(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Les mots de `query` (séparés par des espaces) sont-ils tous dans `entry` ?
pub fn matches(entry: &Entry, query: &str) -> bool {
    let hay = entry.haystack();
    query.split_whitespace().all(|w| hay.contains(&w.to_lowercase()))
}

// ------------------------------------------------------------------------------ parcours

enum ScanMsg {
    Found(Vec<Entry>),
    Done { truncated: bool },
}

struct Scan {
    rx: Receiver<ScanMsg>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Scan {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

// ----------------------------------------------------------------------------------- durée

/// La durée d'un fichier son, en secondes : l'en-tête suffit le plus souvent ; sinon, un petit
/// fichier est décodé.
pub fn probe_seconds(path: &Path) -> Option<f64> {
    use symphonia::core::codecs::CODEC_TYPE_NULL;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let file = std::fs::File::open(path).ok()?;
    let size = file.metadata().map_or(0, |m| m.len());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let from_header = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .ok()
        .and_then(|p| {
            let t = p.format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?;
            let (n, rate) = (t.codec_params.n_frames?, t.codec_params.sample_rate?);
            (rate > 0).then(|| n as f64 / rate as f64)
        });
    if from_header.is_some() || size > PROBE_DECODE_MAX {
        return from_header;
    }
    let bytes = std::fs::read(path).ok()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    audio::decode(&bytes, &ext).ok().map(|p| p.seconds())
}

/// Lit les durées sur un fil à part, pour les seules lignes qu'on demande.
pub struct Sonde {
    tx: Sender<PathBuf>,
    rx: Receiver<(PathBuf, Option<f64>)>,
    asked: HashSet<PathBuf>,
    pub seconds: HashMap<PathBuf, Option<f64>>,
    in_flight: usize,
}

impl Default for Sonde {
    fn default() -> Self {
        Self::with(probe_seconds)
    }
}

impl Sonde {
    /// Avec une autre façon de lire la durée (les essais).
    pub fn with(probe: fn(&Path) -> Option<f64>) -> Sonde {
        let (tx, jobs) = channel::<PathBuf>();
        let (done, rx) = channel();
        std::thread::spawn(move || {
            for path in jobs {
                let s = probe(&path);
                if done.send((path, s)).is_err() {
                    break;
                }
            }
        });
        Sonde { tx, rx, asked: HashSet::new(), seconds: HashMap::new(), in_flight: 0 }
    }

    /// Demande la durée de `path` (une seule fois, et pas plus de `PROBE_QUEUE` à la fois : une liste
    /// qu'on fait défiler vite ne les empile pas toutes).
    pub fn want(&mut self, path: &Path) {
        if self.in_flight >= PROBE_QUEUE || self.seconds.contains_key(path) || !self.asked.insert(path.to_path_buf()) {
            return;
        }
        self.in_flight += 1;
        let _ = self.tx.send(path.to_path_buf());
    }

    /// Reprend les durées lues ; vrai s'il y en a de nouvelles.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((path, s)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            self.asked.remove(&path);
            self.seconds.insert(path, s);
            any = true;
        }
        any
    }

    pub fn get(&self, path: &Path) -> Option<Option<f64>> {
        self.seconds.get(path).copied()
    }
}

/// « 0,42 s », « 1:05 », « — » (durée pas encore lue, ou illisible).
pub fn duration_text(seconds: Option<Option<f64>>) -> String {
    match seconds {
        None => String::new(),
        Some(None) => "—".into(),
        Some(Some(s)) if s < 10.0 => format!("{s:.2} s").replace('.', ","),
        Some(Some(s)) if s < 60.0 => format!("{s:.1} s").replace('.', ","),
        Some(Some(s)) => format!("{}:{:02}", s as u64 / 60, s as u64 % 60),
    }
}

// ----------------------------------------------------------------------------------- écoute

/// Où part le son de l'écoute : la sortie audio de SHOT, ou un faux dans les essais.
pub trait Sortie {
    fn jouer(&mut self, pcm: Arc<Pcm>);
    fn arreter(&mut self);
    /// Le son joué par cette écoute sonne-t-il encore ?
    fn sonne(&self) -> bool;
}

/// Décode un fichier en entier (sur un fil à part). Remplaçable dans les essais.
pub type Decodeur = fn(&Path) -> Result<Arc<Pcm>, String>;

pub fn decode_file(path: &Path) -> Result<Arc<Pcm>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture impossible ({e})"))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    audio::decode(&bytes, &ext).map(Arc::new)
}

enum Etat {
    Repos,
    Charge { path: PathBuf, rx: Receiver<Result<Arc<Pcm>, String>> },
    Joue { path: PathBuf },
}

/// L'écoute d'un son, un seul à la fois : un clic joue, un second clic sur le même arrête.
pub struct Ecoute {
    etat: Etat,
    decodeur: Decodeur,
}

impl Default for Ecoute {
    fn default() -> Self {
        Ecoute { etat: Etat::Repos, decodeur: decode_file }
    }
}

impl Ecoute {
    #[cfg(test)]
    pub fn avec(decodeur: Decodeur) -> Ecoute {
        Ecoute { etat: Etat::Repos, decodeur }
    }

    /// Le son qu'on charge ou qu'on entend.
    pub fn courant(&self) -> Option<&Path> {
        match &self.etat {
            Etat::Repos => None,
            Etat::Charge { path, .. } | Etat::Joue { path } => Some(path),
        }
    }

    pub fn charge(&self) -> bool {
        matches!(self.etat, Etat::Charge { .. })
    }

    pub fn joue(&self) -> bool {
        matches!(self.etat, Etat::Joue { .. })
    }

    /// Un clic sur un son : l'écouter ; sur celui qu'on entend déjà : l'arrêter.
    pub fn basculer(&mut self, path: &Path, sortie: &mut dyn Sortie) {
        if self.courant() == Some(path) {
            self.arreter(sortie);
            return;
        }
        self.arreter(sortie);
        let (tx, rx) = channel();
        let (p, decode) = (path.to_path_buf(), self.decodeur);
        std::thread::spawn(move || {
            let _ = tx.send(decode(&p));
        });
        self.etat = Etat::Charge { path: path.to_path_buf(), rx };
    }

    pub fn arreter(&mut self, sortie: &mut dyn Sortie) {
        if matches!(self.etat, Etat::Joue { .. }) {
            sortie.arreter();
        }
        self.etat = Etat::Repos;
    }

    /// À chaque image d'écran : le son décodé est joué, la fin du son rend le repos. Renvoie le
    /// message d'une erreur de lecture, s'il y en a une.
    pub fn suivre(&mut self, sortie: &mut dyn Sortie) -> Option<String> {
        match &self.etat {
            Etat::Charge { path, rx } => match rx.try_recv() {
                Ok(Ok(pcm)) => {
                    let path = path.clone();
                    sortie.jouer(pcm);
                    self.etat = Etat::Joue { path };
                    None
                }
                Ok(Err(e)) => {
                    let message = format!("« {} » : {e}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                    self.etat = Etat::Repos;
                    Some(message)
                }
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    self.etat = Etat::Repos;
                    Some("l'écoute s'est interrompue".into())
                }
            },
            Etat::Joue { .. } => {
                if !sortie.sonne() {
                    self.etat = Etat::Repos;
                }
                None
            }
            Etat::Repos => None,
        }
    }
}

/// Ce que joue la sortie audio pour l'écoute : le son entier, une fois, à plein volume.
pub fn extrait_ecoute(pcm: Arc<Pcm>) -> (Vec<MixClip>, u64) {
    let len = pcm.frames() as u64;
    (vec![MixClip::plain(pcm, 0, 0, len, 1.0)], len)
}

// ------------------------------------------------------------------------------ bibliothèque

/// La liste des sons, la recherche, le son choisi et l'écoute.
pub struct Bibliotheque {
    pub entries: Vec<Entry>,
    scan: Option<Scan>,
    /// Le parcours a été lancé au moins une fois (il se fait à la première ouverture du panneau).
    pub started: bool,
    /// La liste a été coupée à `MAX_ENTRIES`.
    pub truncated: bool,
    pub search: String,
    /// Le son choisi (son chemin).
    pub selected: Option<PathBuf>,
    pub sonde: Sonde,
    pub ecoute: Ecoute,
    /// Lignes qui passent le filtre (rangs dans `entries`), refaites quand la recherche ou la liste change.
    filtered: Vec<usize>,
    filter_key: (String, usize),
}

impl Default for Bibliotheque {
    fn default() -> Self {
        Bibliotheque {
            entries: Vec::new(),
            scan: None,
            started: false,
            truncated: false,
            search: String::new(),
            selected: None,
            sonde: Sonde::default(),
            ecoute: Ecoute::default(),
            filtered: Vec::new(),
            filter_key: (String::new(), usize::MAX),
        }
    }
}

impl Bibliotheque {
    /// (Re)lance le parcours des dossiers sur un fil à part ; la liste se remplit au fil des images.
    pub fn scan(&mut self, folders: &[PathBuf]) {
        self.started = true;
        self.entries.clear();
        self.truncated = false;
        self.filter_key = (String::new(), usize::MAX);
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (flag, dirs) = (cancel.clone(), folders.to_vec());
        std::thread::spawn(move || {
            let mut left = MAX_ENTRIES;
            let mut truncated = false;
            for (i, dir) in dirs.iter().enumerate() {
                let mut found = 0;
                let cut = scan_dir(dir, i, &flag, left, |batch| {
                    found += batch.len();
                    if !batch.is_empty() {
                        let _ = tx.send(ScanMsg::Found(batch));
                    }
                });
                left = left.saturating_sub(found);
                if cut || left == 0 {
                    truncated = true;
                    break;
                }
                if flag.load(Ordering::Relaxed) {
                    return;
                }
            }
            let _ = tx.send(ScanMsg::Done { truncated });
        });
        self.scan = Some(Scan { rx, cancel });
    }

    /// Le parcours est-il en cours ?
    pub fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Reprend ce que le parcours a trouvé, et les durées lues ; vrai si quelque chose a changé.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.sonde.poll();
        let mut finished = false;
        if let Some(scan) = &self.scan {
            while let Ok(msg) = scan.rx.try_recv() {
                match msg {
                    ScanMsg::Found(batch) => {
                        self.entries.extend(batch);
                        changed = true;
                    }
                    ScanMsg::Done { truncated } => {
                        self.truncated = truncated;
                        finished = true;
                        changed = true;
                    }
                }
            }
        }
        if finished {
            self.scan = None;
        }
        changed
    }

    /// Les rangs des sons qui passent la recherche (gardés tant que ni elle ni la liste ne changent).
    pub fn visible(&mut self) -> &[usize] {
        let key = (self.search.clone(), self.entries.len());
        if key != self.filter_key {
            self.filtered = (0..self.entries.len()).filter(|&i| matches(&self.entries[i], &self.search)).collect();
            self.filter_key = key;
        }
        &self.filtered
    }

    /// Les durées des lignes `rows` (rangs dans la liste visible), demandées à la sonde.
    pub fn want_durations(&mut self, rows: std::ops::Range<usize>) {
        self.visible();
        for r in rows {
            if let Some(&i) = self.filtered.get(r) {
                let path = self.entries[i].path.clone();
                self.sonde.want(&path);
            }
        }
    }
}
