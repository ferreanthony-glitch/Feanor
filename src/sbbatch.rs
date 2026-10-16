//! Export automatique des dessins d'un projet Storyboard Pro (14a-2, demandé par Anthony : « il
//! faut automatiser le script à l'ouverture du fichier »). À l'ouverture d'un `.sboard`, Pelure :
//! 1. copie le dossier du projet dans un dossier temporaire (sans `logs/` ni verrous) : Storyboard
//!    Pro écrit dans le dossier du projet qu'il ouvre, le projet d'origine n'est donc jamais
//!    touché ;
//! 2. y lance Storyboard Pro en mode batch avec le script d'export (`sbscript`), avec la syntaxe
//!    de sa documentation : `StoryboardPro -scene <projet>.sboard -batch -compile <script>.js` ;
//! 3. suit l'avancement (un dossier d'images par panneau exporté) ; « Arrêter » arrête Storyboard
//!    Pro ;
//! 4. lit la liste écrite par le script. Ce que Storyboard Pro a écrit est gardé dans
//!    `journal.txt` (montré si l'export échoue).
//!
//! `PELURE_STORYBOARD_PRO` peut désigner l'exécutable ; `PELURE_SANS_STORYBOARD_PRO` désactive le
//! lancement (essais de Pelure).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::sbscript::{self, Drawings};

/// L'exécutable de Storyboard Pro le plus récent installé, s'il y en a un (et si le lancement
/// n'est pas désactivé).
pub fn find_storyboard_pro() -> Option<PathBuf> {
    if std::env::var_os("PELURE_SANS_STORYBOARD_PRO").is_some() {
        return None;
    }
    if let Some(p) = std::env::var_os("PELURE_STORYBOARD_PRO").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        let Some(base) = std::env::var_os(var).map(|b| PathBuf::from(b).join("Toon Boom Animation")) else { continue };
        let Ok(entries) = std::fs::read_dir(&base) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let exe = e.path().join("win64").join("bin").join("StoryboardPro.exe");
            if name.starts_with("Toon Boom Storyboard Pro") && exe.is_file() {
                found.push(exe);
            }
        }
    }
    // « … Pro 27 » passe avant « … Pro 25 » : le numéro de version le plus haut.
    found.sort_by_key(|p| std::cmp::Reverse(version_of(p)));
    found.into_iter().next()
}

/// Numéro de version d'un chemin d'installation (« … Storyboard Pro 27\win64\… » → 27).
pub fn version_of(exe: &Path) -> u32 {
    exe.ancestors()
        .filter_map(|a| a.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .find(|n| n.starts_with("Toon Boom Storyboard Pro"))
        .and_then(|n| n.rsplit(' ').next().and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

/// Arguments de la ligne de commande du mode batch.
pub fn batch_args(scene: &Path, script: &Path) -> Vec<std::ffi::OsString> {
    vec!["-scene".into(), scene.as_os_str().into(), "-batch".into(), "-compile".into(), script.as_os_str().into()]
}

/// Copie le dossier du projet `src` dans `dest`, sans le dossier `logs` ni les fichiers de
/// verrou (`.lock`). Renvoie le nombre d'octets copiés.
pub fn copy_project(src: &Path, dest: &Path) -> std::io::Result<u64> {
    copy_dir(src, dest, true)
}

fn copy_dir(src: &Path, dest: &Path, top: bool) -> std::io::Result<u64> {
    std::fs::create_dir_all(dest)?;
    let mut total = 0;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        let lower = name.to_string_lossy().to_lowercase();
        if e.file_type()?.is_dir() {
            if !(top && lower == "logs") {
                total += copy_dir(&e.path(), &dest.join(&name), false)?;
            }
        } else if !lower.ends_with(".lock") {
            total += std::fs::copy(e.path(), dest.join(&name))?;
        }
    }
    Ok(total)
}

/// Nombre de panneaux déjà exportés (un dossier par panneau sous `images`).
fn count_exported(dest: &Path) -> usize {
    std::fs::read_dir(dest.join("images")).map_or(0, |r| r.flatten().filter(|e| e.path().is_dir()).count())
}

/// La fin du journal de Storyboard Pro, pour un message d'erreur.
fn journal_tail(dest: &Path) -> String {
    let text = std::fs::read(dest.join("journal.txt")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(4)..].join(" / ");
    if tail.is_empty() {
        "(vide)".into()
    } else {
        tail
    }
}

/// Un export en cours, sur un autre fil.
pub struct Batch {
    /// L'étape en cours, à montrer.
    pub stage: Arc<Mutex<String>>,
    /// Panneaux exportés jusqu'ici, sur `total`.
    pub done: Arc<AtomicUsize>,
    pub total: usize,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<Drawings, String>>>>,
    /// Dossier de travail (copie du projet, dessins exportés, journal).
    pub work: PathBuf,
}

impl Batch {
    /// Lance l'export du projet `sboard` (`total` : nombre de panneaux du projet) avec
    /// Storyboard Pro `exe`, dans un dossier temporaire neuf.
    pub fn start(exe: PathBuf, sboard: PathBuf, total: usize) -> Batch {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let stem = sboard.file_stem().map_or_else(|| "projet".into(), |s| s.to_string_lossy().into_owned());
        let safe: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        let work = std::env::temp_dir().join("pelure-storyboard-pro").join(format!("{safe}-{stamp}"));
        let batch = Batch {
            stage: Arc::new(Mutex::new("Copie du projet…".into())),
            done: Arc::new(AtomicUsize::new(0)),
            total,
            cancel: Arc::new(AtomicBool::new(false)),
            result: Arc::new(Mutex::new(None)),
            work: work.clone(),
        };
        let (stage, done, cancel, result) = (batch.stage.clone(), batch.done.clone(), batch.cancel.clone(), batch.result.clone());
        std::thread::spawn(move || {
            let set = |s: &str| *stage.lock().unwrap_or_else(|e| e.into_inner()) = s.to_string();
            let res = run(&exe, &sboard, &work, &set, &done, &cancel);
            *result.lock().unwrap_or_else(|e| e.into_inner()) = Some(res);
        });
        batch
    }

    /// Arrête l'export (Storyboard Pro est arrêté).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Le résultat, une fois l'export fini.
    pub fn finished(&self) -> Option<Result<Drawings, String>> {
        self.result.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

fn run(exe: &Path, sboard: &Path, work: &Path, set: &dyn Fn(&str), done: &AtomicUsize, cancel: &AtomicBool) -> Result<Drawings, String> {
    let src = sboard.parent().ok_or("dossier du projet introuvable")?;
    let name = sboard.file_name().ok_or("nom du projet introuvable")?;
    let copy = work.join("projet");
    copy_project(src, &copy).map_err(|e| format!("copie du projet impossible ({e})"))?;
    if cancel.load(Ordering::Relaxed) {
        return Err("export arrêté".into());
    }
    let dest = work.join("dessins");
    std::fs::create_dir_all(&dest).map_err(|e| format!("dossier des dessins impossible ({e})"))?;
    let script = dest.join(sbscript::SCRIPT);
    std::fs::write(&script, sbscript::script(&dest)).map_err(|e| format!("écriture du script impossible ({e})"))?;
    set("Storyboard Pro exporte les dessins…");
    let log = std::fs::File::create(dest.join("journal.txt")).map_err(|e| format!("journal impossible ({e})"))?;
    let log2 = log.try_clone().map_err(|e| format!("journal impossible ({e})"))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(batch_args(&copy.join(name), &script)).stdin(std::process::Stdio::null()).stdout(log).stderr(log2);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Pas de fenêtre de console.
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| format!("Storyboard Pro ne démarre pas ({e})"))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("export arrêté".into());
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(format!("Storyboard Pro : {e}")),
        }
        done.store(count_exported(&dest), Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    done.store(count_exported(&dest), Ordering::Relaxed);
    set("Lecture des dessins…");
    sbscript::read_manifest(&dest).map_err(|e| format!("{e} Journal de Storyboard Pro : {}", journal_tail(&dest)))
}
