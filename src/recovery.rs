//! Sauvegarde de secours et récupération après plantage (4a).
//!
//! Chaque séance de Pelure a son dossier dans `%LOCALAPPDATA%\Pelure\secours\<séance>` :
//! - `verrou` : tenu ouvert (sans partage) tant que Pelure tourne ; tant qu'on ne peut pas
//!   l'effacer, la séance est vivante (un autre Pelure ouvert à côté, par exemple) ;
//! - `seance.json` : le projet d'origine, son titre, l'heure de la dernière copie ;
//! - `projet\secours.pelure` : une copie du projet, écrite toutes les N minutes s'il a changé,
//!   sur un autre fil (on continue à dessiner pendant ce temps).
//!
//! Le vrai projet n'est jamais touché : seul « Enregistrer » l'écrit. Fermeture normale (même
//! « Ne pas enregistrer ») : le dossier est effacé. Plantage : il reste, et la séance suivante
//! propose d'ouvrir la copie.

use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::document::{CelId, Document};
use crate::project::{self, Reuse, SaveReport};

const LOCK: &str = "verrou";
const INFO: &str = "seance.json";
const COPY: &str = "projet";
const MANIFEST: &str = "secours.pelure";

/// Fiche d'une séance.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    /// Titre du projet (nom du fichier, ou « Sans titre »).
    #[serde(rename = "titre")]
    pub title: String,
    /// Projet d'origine, s'il avait déjà été enregistré.
    #[serde(rename = "projet")]
    pub project: Option<PathBuf>,
    /// Heure de la dernière copie (secondes depuis 1970).
    #[serde(rename = "copie")]
    pub copied: Option<u64>,
}

/// Une séance interrompue dont on peut récupérer la copie.
#[derive(Clone, Debug)]
pub struct Orphan {
    pub dir: PathBuf,
    pub info: SessionInfo,
    pub manifest: PathBuf,
}

impl Orphan {
    /// Le projet d'origine a-t-il été enregistré après la dernière copie ? (La copie n'apporte
    /// alors sans doute rien.)
    pub fn project_is_newer(&self) -> bool {
        let saved = self
            .info
            .project
            .as_ref()
            .and_then(|p| fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        matches!((saved, self.info.copied), (Some(s), Some(c)) if s > c)
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Dossier de toutes les séances.
pub fn root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Pelure").join("secours"))
}

/// Fichier ouvert sans partage : ni un autre programme ni Pelure ne peut l'effacer tant qu'il
/// est ouvert.
fn open_lock(path: &Path) -> std::io::Result<File> {
    let mut o = fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(0);
    }
    o.open(path)
}

/// Séances interrompues (verrou libre) de `root`, sauf `own` : celles qui ont une copie sont
/// renvoyées, les autres (plantage avant toute copie) sont effacées.
pub fn orphans(root: &Path, own: Option<&Path>) -> Vec<Orphan> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root).into_iter().flatten().flatten() {
        let dir = entry.path();
        if !dir.is_dir() || Some(dir.as_path()) == own {
            continue;
        }
        let lock = dir.join(LOCK);
        if lock.exists() && fs::remove_file(&lock).is_err() {
            continue; // séance vivante
        }
        let manifest = dir.join(COPY).join(MANIFEST);
        if !manifest.exists() {
            let _ = fs::remove_dir_all(&dir);
            continue;
        }
        let info = fs::read_to_string(dir.join(INFO))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| SessionInfo {
                title: "Projet".into(),
                ..Default::default()
            });
        found.push(Orphan { dir, info, manifest });
    }
    found.sort_by_key(|o| std::cmp::Reverse(o.info.copied));
    found
}

/// Ce qu'il faut savoir du document pour faire une copie.
pub struct Snapshot<'a> {
    pub doc: &'a Document,
    /// Change à chaque modification (numéro de l'historique).
    pub serial: u64,
    /// Le document diffère du projet enregistré.
    pub modified: bool,
    pub title: String,
    pub project: Option<PathBuf>,
    /// Dessins déjà écrits dans le projet d'origine (recopiés au lieu d'être réencodés).
    pub project_written: &'a HashMap<CelId, u64>,
}

/// La séance en cours.
pub struct Recovery {
    pub dir: PathBuf,
    _lock: File,
    written: HashMap<CelId, u64>,
    job: Option<JoinHandle<Result<SaveReport, String>>>,
    /// Numéro de l'historique au moment de la dernière copie lancée.
    copied_serial: Option<u64>,
    last_try: Instant,
    pub info: SessionInfo,
    /// Dernière erreur d'écriture (affichée une fois).
    pub error: Option<String>,
}

impl Recovery {
    /// Ouvre le dossier de la séance dans `root`.
    pub fn start(root: &Path) -> Result<Recovery, String> {
        let name = format!("{}-{}", now_secs(), std::process::id());
        let dir = root.join(name);
        fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        let lock = open_lock(&dir.join(LOCK)).map_err(|e| format!("{} : {e}", dir.display()))?;
        Ok(Recovery {
            dir,
            _lock: lock,
            written: HashMap::new(),
            job: None,
            copied_serial: None,
            last_try: Instant::now(),
            info: SessionInfo::default(),
            error: None,
        })
    }

    fn write_info(&self) {
        if let Ok(text) = serde_json::to_string_pretty(&self.info) {
            let _ = fs::write(self.dir.join(INFO), text);
        }
    }

    /// Une copie s'écrit en ce moment.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// À chaque image : termine la copie en cours, ou en lance une si `interval` est passé et
    /// que le document a changé. `hold` : ne pas lancer maintenant (trait en cours…).
    /// Renvoie vrai quand une copie vient de se terminer.
    pub fn tick(&mut self, snap: Snapshot, interval: Option<Duration>, hold: bool) -> bool {
        if let Some(job) = &self.job {
            if !job.is_finished() {
                return false;
            }
            let result = self.job.take().expect("copie en cours").join().unwrap_or(Err("copie interrompue".into()));
            return match result {
                Ok(_) => {
                    self.info.copied = Some(now_secs());
                    self.write_info();
                    true
                }
                Err(e) => {
                    // Tout sera réécrit la prochaine fois.
                    self.written.clear();
                    self.copied_serial = None;
                    self.error = Some(e);
                    false
                }
            };
        }
        let Some(interval) = interval else { return false };
        if hold || self.last_try.elapsed() < interval || !snap.modified || self.copied_serial == Some(snap.serial) {
            return false;
        }
        self.start_copy(snap);
        false
    }

    /// Lance une copie tout de suite (sans attendre le délai).
    pub fn start_copy(&mut self, snap: Snapshot) {
        if self.job.is_some() {
            return;
        }
        self.last_try = Instant::now();
        self.info.title = snap.title;
        self.info.project = snap.project;
        let manifest = self.dir.join(COPY).join(MANIFEST);
        let reuse = self.info.project.as_deref().map(|p| Reuse {
            manifest_path: p,
            written: snap.project_written,
        });
        match project::prepare_save(snap.doc, &manifest, &mut self.written, reuse) {
            Ok(job) => {
                self.copied_serial = Some(snap.serial);
                self.job = Some(std::thread::spawn(move || job.run()));
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Fermeture normale : on attend la copie en cours, puis on efface le dossier de la séance.
    pub fn close(mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.join();
        }
        let dir = self.dir.clone();
        drop(self);
        let _ = fs::remove_dir_all(dir);
    }
}
