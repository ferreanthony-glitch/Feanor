//! Les essais ne laissent plus rien dans le dossier temporaire de Windows (2026-11-17).
//!
//! Des centaines d'essais créent leurs dossiers par `std::env::temp_dir()` (`shot-…`, `pelure-test-…`, `feanor-…`) sans
//! les effacer : 652 000 dossiers, 12 Go, le 2026-11-16, le disque C: plein. Plutôt que de retoucher chaque essai, le
//! processus d'essai, avant `main`, dirige `TMP` et `TEMP` vers un dossier qui n'est qu'à lui,
//! `%TEMP%\feanor-essais-<processus>` : tout ce que les essais (et le code du programme qu'ils appellent :
//! `pelure-videos`, `shot-essais-…`, `pelure-export-….wav`…) mettent dans le dossier temporaire y va. Il est effacé quand
//! le processus finit, que les essais passent ou non ; celui d'un passage tué (délai de `scripts/verifier.ps1`, Ctrl+C)
//! l'est au démarrage du passage suivant, une fois son processus fini, et par `scripts/verifier.ps1` à sa fin.
//!
//! Le vrai programme n'est pas touché : ce module n'existe que dans les essais (`#[cfg(test)]`). Hors de Windows, rien
//! ne change (les essais gardent le dossier temporaire du système).

use std::path::PathBuf;
use std::sync::OnceLock;

/// Début du nom des dossiers des passages d'essai, dans le dossier temporaire du système.
#[cfg(windows)]
const PREFIXE: &str = "feanor-essais-";
/// Posée pour les processus que lance un essai (`memoirevideo_tests` relance l'exécutable des essais) : ils gardent le
/// dossier de leur parent, qui l'effacera.
#[cfg(windows)]
const MARQUE: &str = "FEANOR_ESSAIS_TEMP";

/// Le dossier de ce passage, quand c'est ce processus qui l'a créé (c'est lui qui l'efface).
#[cfg(windows)]
static RACINE: OnceLock<PathBuf> = OnceLock::new();

/// Le dossier temporaire du système, celui d'avant la redirection.
static SYSTEME: OnceLock<PathBuf> = OnceLock::new();

/// Le dossier temporaire du système, qui survit au processus d'essai : pour les bancs manuels (`#[ignore]`) dont un
/// processus fabrique ce que les suivants relisent (`shot-endurance`, `shot-lourd`).
pub(crate) fn dossier_systeme() -> PathBuf {
    SYSTEME.get().cloned().unwrap_or_else(std::env::temp_dir)
}

// Appelé par la bibliothèque C de Windows avant `main` (les fonctions de la section `.CRT$XCU`, comme les constructeurs
// globaux du C++) : aucun essai n'a encore lu le dossier temporaire.
#[cfg(windows)]
#[used]
#[unsafe(link_section = ".CRT$XCU")]
static AVANT_MAIN: extern "C" fn() = preparer;

#[cfg(windows)]
unsafe extern "C" {
    fn atexit(f: extern "C" fn()) -> i32;
}

#[cfg(windows)]
extern "C" fn preparer() {
    // (Aucun `unwrap` ici : une panique avant `main` arrêterait les essais sans un mot.)
    if let Some(racine) = std::env::var_os(MARQUE) {
        let racine = PathBuf::from(racine);
        let _ = SYSTEME.set(racine.parent().map(PathBuf::from).unwrap_or(racine));
        return;
    }
    let systeme = std::env::temp_dir();
    let _ = SYSTEME.set(systeme.clone());
    menage(&systeme);
    let racine = systeme.join(format!("{PREFIXE}{}", std::process::id()));
    // (Un dossier du même numéro vient d'un ancien processus : les numéros de processus resservent.)
    let _ = std::fs::remove_dir_all(&racine);
    if std::fs::create_dir_all(&racine).is_err() {
        return;
    }
    std::env::set_var("TMP", &racine);
    std::env::set_var("TEMP", &racine);
    std::env::set_var(MARQUE, &racine);
    let _ = RACINE.set(racine);
    // SAFETY : `effacer` est une fonction `extern "C"` sans argument, qui ne panique pas.
    unsafe {
        atexit(effacer);
    }
}

/// À la sortie du processus (essais finis, réussis ou non) : le dossier du passage part, avec tout ce qu'il contient.
#[cfg(windows)]
extern "C" fn effacer() {
    if let Some(racine) = RACINE.get() {
        let _ = std::fs::remove_dir_all(racine);
    }
}

/// Efface les dossiers des passages d'avant dont le processus est fini ; ceux d'un passage qui tourne encore (un autre
/// dossier de travail, une autre session) restent.
#[cfg(windows)]
fn menage(systeme: &std::path::Path) {
    let Ok(entrees) = std::fs::read_dir(systeme) else { return };
    let moi = std::process::id();
    for e in entrees.flatten() {
        let nom = e.file_name();
        let Some(pid) = nom.to_str().and_then(|n| n.strip_prefix(PREFIXE)).and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if pid != moi && !processus_vivant(pid) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// Le processus `pid` tourne-t-il encore ? (Dans le doute — accès refusé —, oui : son dossier reste.)
#[cfg(windows)]
fn processus_vivant(pid: u32) -> bool {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(acces: u32, heriter: i32, pid: u32) -> *mut c_void;
        fn GetExitCodeProcess(processus: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(objet: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_INVALID_PARAMETER: u32 = 87;
    // SAFETY : appels de l'API de Windows avec des arguments valides ; le descripteur ouvert est refermé.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return GetLastError() != ERROR_INVALID_PARAMETER;
        }
        let mut code = 0u32;
        let lu = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        lu == 0 || code == STILL_ACTIVE
    }
}

#[cfg(windows)]
#[test]
fn les_essais_ecrivent_dans_le_dossier_de_leur_passage() {
    let systeme = dossier_systeme();
    let temp = std::env::temp_dir();
    assert_ne!(temp, systeme, "le dossier temporaire des essais n'est pas celui du système");
    assert!(temp.starts_with(&systeme), "{} est dans {}", temp.display(), systeme.display());
    let nom = temp.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    assert!(nom.starts_with(PREFIXE), "{nom}");
    assert!(temp.is_dir());
}
