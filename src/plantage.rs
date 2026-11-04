//! Journal de plantage (Anthony, 2026-10-17 : « le logiciel crashe très vite lors de
//! l'utilisation de l'outil de propagation de la couleur ») : une erreur de programme (panic)
//! fermait SHOT (alors Pelure) sans laisser de trace (pas de console, rien dans le journal de
//! Windows). Le crochet écrit le message, l'endroit du code, le fil et la pile dans
//! `%LOCALAPPDATA%\SHOT\plantages\plantage-<secondes>.txt`, et la séance suivante le signale
//! dans la barre d'état. Le projet, lui, est rattrapé par la copie de secours (`recovery.rs`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Fichier qui désigne le dernier rapport, tant que la séance suivante ne l'a pas signalé.
const NEW: &str = "nouveau.txt";

/// Les dernières commandes et étapes d'annulation de la séance, pour le rapport : ce qu'on faisait juste avant
/// l'erreur (passe de débogage, 2026-10-22). Une écriture par commande ou par étape, jamais par image.
static DERNIERES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
const GARDEES: usize = 8;

/// Note une commande lancée ou une étape d'annulation (son nom). Sans jamais attendre : si le rapport est en train
/// de lire la liste, la note est perdue.
pub fn noter_action(quoi: &str) {
    if let Ok(mut d) = DERNIERES.try_lock() {
        if d.len() >= GARDEES {
            d.pop_front();
        }
        d.push_back(quoi.to_string());
    }
}

/// Les dernières actions notées, de la plus ancienne à la plus récente.
pub fn dernieres_actions() -> String {
    match DERNIERES.try_lock() {
        Ok(d) if !d.is_empty() => d.iter().map(String::as_str).collect::<Vec<_>>().join(" → "),
        Ok(_) => "(aucune)".into(),
        // (Une erreur pendant qu'on notait : la liste n'est pas lisible sans risque.)
        Err(_) => "(illisible)".into(),
    }
}

/// La carte graphique de l'affichage est perdue, et l'interface ne l'a pas encore dit (`carte_perdue_a_signaler`).
static CARTE_PERDUE: AtomicBool = AtomicBool::new(false);

/// La carte graphique de l'affichage est perdue (pilote réinitialisé, mise en veille…) : un rapport, et l'interface
/// lance une copie de secours à l'image suivante. (Le 2026-10-22, SHOT a tourné deux minutes sur une carte perdue
/// avant que l'affichage d'egui ne plante : le travail fait depuis la dernière copie de secours était perdu.)
pub fn note_device_lost(what: &str) {
    CARTE_PERDUE.store(true, Ordering::Relaxed);
    note_gpu_error(what);
}

/// Vrai une seule fois après la perte de la carte graphique.
pub fn carte_perdue_a_signaler() -> bool {
    CARTE_PERDUE.load(Ordering::Relaxed) && CARTE_PERDUE.swap(false, Ordering::Relaxed)
}

/// Dossier des rapports.
pub fn dir() -> Option<PathBuf> {
    crate::donnees::local().map(|d| d.join("plantages"))
}

/// Texte du rapport.
pub fn report_text(message: &str, location: &str, thread: &str, backtrace: &str) -> String {
    format!(
        "FEANOR {} : une erreur de programme (une erreur rattrapée, par exemple dans la propagation, ne ferme pas FEANOR).\n\nErreur : {message}\nEndroit : {location}\nFil : {thread}\nQuand : {} s (horloge Unix)\nDernières actions : {}\n\nPile :\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        crate::recovery::now_secs(),
        dernieres_actions(),
    )
}

/// Écrit le rapport dans `dir` et le désigne comme nouveau ; rend son chemin.
pub fn write_report(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let secs = crate::recovery::now_secs();
    // (Deux erreurs dans la même seconde — un fil de fond puis l'interface — gardent chacune leur rapport.)
    let mut path = dir.join(format!("plantage-{secs}.txt"));
    let mut k = 2;
    while path.exists() && k < 100 {
        path = dir.join(format!("plantage-{secs}-{k}.txt"));
        k += 1;
    }
    std::fs::write(&path, text)?;
    std::fs::write(dir.join(NEW), path.to_string_lossy().as_bytes())?;
    Ok(path)
}

/// Le rapport laissé par une séance précédente et pas encore signalé (il ne l'est qu'une fois).
pub fn take_new(dir: &Path) -> Option<PathBuf> {
    let marker = dir.join(NEW);
    let path = std::fs::read_to_string(&marker).ok()?;
    let _ = std::fs::remove_file(&marker);
    Some(PathBuf::from(path.trim()))
}

/// Une erreur de la carte graphique de l'affichage (sans plantage) : un rapport, le premier de la
/// séance seulement (une erreur qui revient à chaque image ne remplit pas le disque).
pub fn note_gpu_error(what: &str) {
    static NOTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    eprintln!("{what}");
    if NOTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if let Some(dir) = dir() {
        let _ = write_report(&dir, &report_text(what, "carte graphique (affichage)", "wgpu", "(sans pile)"));
    }
}

/// Pose le crochet : le message habituel (sur la console, s'il y en a une), puis le rapport.
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(sans message)".into());
        let location = info.location().map_or_else(|| "inconnu".into(), |l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let thread = std::thread::current().name().unwrap_or("sans nom").to_string();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        if let Some(dir) = dir() {
            let _ = write_report(&dir, &report_text(&message, &location, &thread, &backtrace));
        }
    }));
}
