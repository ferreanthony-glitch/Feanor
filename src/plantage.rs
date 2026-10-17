//! Journal de plantage (Anthony, 2026-10-17 : « le logiciel crashe très vite lors de
//! l'utilisation de l'outil de propagation de la couleur ») : une erreur de programme (panic)
//! fermait Pelure sans laisser de trace (pas de console, rien dans le journal de Windows). Le
//! crochet écrit le message, l'endroit du code, le fil et la pile dans
//! `%LOCALAPPDATA%\Pelure\plantages\plantage-<secondes>.txt`, et la séance suivante le signale
//! dans la barre d'état. Le projet, lui, est rattrapé par la copie de secours (`recovery.rs`).

use std::path::{Path, PathBuf};

/// Fichier qui désigne le dernier rapport, tant que la séance suivante ne l'a pas signalé.
const NEW: &str = "nouveau.txt";

/// Dossier des rapports.
pub fn dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Pelure").join("plantages"))
}

/// Texte du rapport.
pub fn report_text(message: &str, location: &str, thread: &str, backtrace: &str) -> String {
    format!(
        "Pelure {} s'est fermé sur une erreur.\n\nErreur : {message}\nEndroit : {location}\nFil : {thread}\nQuand : {} s (horloge Unix)\n\nPile :\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        crate::recovery::now_secs(),
    )
}

/// Écrit le rapport dans `dir` et le désigne comme nouveau ; rend son chemin.
pub fn write_report(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("plantage-{}.txt", crate::recovery::now_secs()));
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
