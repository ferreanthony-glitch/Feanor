//! Dossiers de données de SHOT (réglages, raccourcis, brosses importées, rapports d'erreur, copies
//! de secours), et leur reprise depuis ceux de l'ancien nom du logiciel.
//!
//! Le logiciel s'appelait Pelure : ses données étaient dans `%APPDATA%\Pelure` et
//! `%LOCALAPPDATA%\Pelure`. Elles sont maintenant dans `...\SHOT`. La première fois qu'un dossier
//! `SHOT` est demandé, s'il n'existe pas encore et que l'ancien existe, son contenu y est copié
//! (jamais déplacé : l'ancien dossier n'est ni modifié ni supprimé, on peut revenir en arrière).
//! Si `SHOT` existe déjà, rien n'est copié : le choix de l'utilisateur prime.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Nom du dossier de données.
pub const NOM: &str = "SHOT";
/// Nom du dossier de l'ancien nom du logiciel (Pelure), repris une fois.
pub const ANCIEN_NOM: &str = "Pelure";

/// Dossiers parents déjà vérifiés pendant cette séance (la reprise ne se tente qu'une fois).
static VERIFIES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// `%APPDATA%\SHOT` (réglages, raccourcis, brosses importées).
pub fn roaming() -> Option<PathBuf> {
    dossier("APPDATA")
}

/// `%LOCALAPPDATA%\SHOT` (rapports d'erreur, copies de secours).
pub fn local() -> Option<PathBuf> {
    dossier("LOCALAPPDATA")
}

fn dossier(variable: &str) -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(bac) = BAC.with(|b| b.borrow().clone()) {
        return Some(sous(&bac));
    }
    Some(sous(Path::new(&std::env::var_os(variable)?)))
}

#[cfg(test)]
thread_local! {
    /// Essais : dossier qui tient lieu de `%APPDATA%` et de `%LOCALAPPDATA%` pour ce fil.
    static BAC: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Essais : les réglages, raccourcis et copies de ce fil s'écrivent dans `dossier` au lieu des
/// vrais dossiers de l'utilisateur (un essai qui fait tourner l'application ne touche pas aux siens).
#[cfg(test)]
pub fn bac_a_sable(dossier: &Path) {
    BAC.with(|b| *b.borrow_mut() = Some(dossier.to_path_buf()));
}

/// Le dossier SHOT dans `base` ; la première fois (par séance et par `base`), reprend l'ancien.
pub fn sous(base: &Path) -> PathBuf {
    let nouveau = base.join(NOM);
    let premiere_fois = match VERIFIES.lock() {
        Ok(mut vus) if !vus.iter().any(|b| b == base) => {
            vus.push(base.to_path_buf());
            true
        }
        _ => false,
    };
    if premiere_fois {
        // Une reprise ratée n'empêche pas de démarrer : SHOT repart alors avec des réglages neufs.
        let _ = reprendre(&base.join(ANCIEN_NOM), &nouveau);
    }
    nouveau
}

/// Copie `ancien` dans `nouveau` si `nouveau` n'existe pas et que `ancien` existe. Renvoie `true`
/// quand une copie a eu lieu. La copie se fait dans un dossier voisin, renommé à la fin : une
/// copie interrompue ne laisse pas un `nouveau` à moitié rempli (qui ne serait plus jamais
/// complété).
pub fn reprendre(ancien: &Path, nouveau: &Path) -> std::io::Result<bool> {
    if nouveau.exists() || !ancien.is_dir() {
        return Ok(false);
    }
    let mut nom = nouveau.file_name().unwrap_or_default().to_os_string();
    nom.push(".reprise");
    let provisoire = nouveau.with_file_name(nom);
    if provisoire.exists() {
        // Reste d'une copie interrompue, créé par nous.
        std::fs::remove_dir_all(&provisoire)?;
    }
    if let Err(e) = copier(ancien, &provisoire).and_then(|_| std::fs::rename(&provisoire, nouveau)) {
        let _ = std::fs::remove_dir_all(&provisoire);
        return Err(e);
    }
    Ok(true)
}

/// Copie récursive du contenu de `de` dans `vers` (créé).
fn copier(de: &Path, vers: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(vers)?;
    for entree in std::fs::read_dir(de)? {
        let entree = entree?;
        let cible = vers.join(entree.file_name());
        if entree.file_type()?.is_dir() {
            copier(&entree.path(), &cible)?;
        } else {
            std::fs::copy(entree.path(), &cible)?;
        }
    }
    Ok(())
}
