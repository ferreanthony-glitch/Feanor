//! Où s'ouvrent les fenêtres de fichiers des exports (Anthony, 2026-10-30, « petits livrables ») : dans le dossier du projet quand un projet est ouvert,
//! au lieu du dernier dossier que le système a retenu (souvent sans rapport avec le film).
//!
//! La règle, par sorte d'export (« fiche », « planche », « pdf »…) :
//! - un projet est ouvert : le dernier dossier choisi **pour cette sorte** s'il est dans le dossier du projet, sinon le dossier du projet ;
//! - aucun projet (rien d'enregistré) : on ne propose rien, la fenêtre s'ouvre où le système le veut.
//!
//! « Enregistrer sous » crée un dossier au nom choisi : la fenêtre s'ouvre donc à côté du dossier du projet (son parent), pas dedans.
//!
//! Tout passe par ici : `App::dialogue_enregistrer` (un fichier), `App::dialogue_dossier` (un dossier), `App::dialogue_pour_enregistrer_sous`. Les
//! dossiers retenus ne vivent que le temps de la séance (rien n'est écrit dans les réglages).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::app::App;

/// Le dernier dossier choisi pour chaque sorte d'export (cette séance).
static DERNIERS: Mutex<Option<HashMap<&'static str, PathBuf>>> = Mutex::new(None);

/// Le dossier où ouvrir la fenêtre : celui du projet, ou le dernier de cette sorte s'il est dedans. Rien si aucun projet n'est ouvert (ou si
/// son dossier n'existe plus).
pub(crate) fn dossier_de_depart(projet: Option<&Path>, dernier: Option<&Path>) -> Option<PathBuf> {
    let projet = projet.filter(|d| d.is_dir())?;
    match dernier {
        Some(d) if d.starts_with(projet) && d.is_dir() => Some(d.to_path_buf()),
        _ => Some(projet.to_path_buf()),
    }
}

/// Le dernier dossier retenu pour `sorte`.
pub(crate) fn dernier(sorte: &str) -> Option<PathBuf> {
    DERNIERS.lock().ok()?.as_ref()?.get(sorte).cloned()
}

/// Retient `dossier` pour `sorte`.
pub(crate) fn retenir(sorte: &'static str, dossier: &Path) {
    if let Ok(mut g) = DERNIERS.lock() {
        g.get_or_insert_with(HashMap::new).insert(sorte, dossier.to_path_buf());
    }
}

/// Choisit un dossier : la fenêtre s'ouvre dans le dossier du projet (`projet`) ou, s'il est dedans, dans le dernier choisi pour `sorte` ; sans projet, dans
/// `repli` (un dossier gardé dans un réglage) s'il existe. Pour un appelant qui tient déjà `self` : il donne le dossier du projet lui-même.
pub(crate) fn choisir_dossier(sorte: &'static str, titre: &str, projet: Option<&Path>, repli: Option<&Path>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(titre);
    let voulu = dossier_de_depart(projet, dernier(sorte).as_deref()).or_else(|| repli.filter(|p| p.is_dir()).map(Path::to_path_buf));
    if let Some(dir) = voulu {
        d = d.set_directory(dir);
    }
    let dossier = d.pick_folder()?;
    retenir(sorte, &dossier);
    Some(dossier)
}

/// Un nom de fichier sûr pour `rfd` : jamais de séparateur de dossier.
fn nom_seul(nom: &str) -> String {
    nom.chars().map(|c| if matches!(c, '/' | '\\') { '_' } else { c }).collect()
}

impl App {
    /// Le dossier du projet ouvert : celui du manifeste (ou du fichier importé).
    pub(crate) fn dossier_du_projet(&self) -> Option<PathBuf> {
        self.path.as_ref().or(self.import_source.as_ref()).and_then(|p| p.parent()).map(Path::to_path_buf)
    }

    /// Le dossier où ouvrir une fenêtre de fichiers pour `sorte`.
    pub(crate) fn dossier_pour(&self, sorte: &str) -> Option<PathBuf> {
        dossier_de_depart(self.dossier_du_projet().as_deref(), dernier(sorte).as_deref())
    }

    /// Choisit où écrire un fichier : `filtre` = (nom du type, extensions), `nom` = le nom proposé.
    pub(crate) fn dialogue_enregistrer(&self, sorte: &'static str, titre: &str, filtre: (&str, &[&str]), nom: &str) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new().set_title(titre).add_filter(filtre.0, filtre.1).set_file_name(nom_seul(nom));
        if let Some(dir) = self.dossier_pour(sorte) {
            d = d.set_directory(dir);
        }
        let path = d.save_file()?;
        if let Some(p) = path.parent() {
            retenir(sorte, p);
        }
        Some(path)
    }

    /// Choisit un dossier ; `depart` : le dossier que la fenêtre garde dans un réglage, utilisé seulement quand aucun projet n'est ouvert.
    pub(crate) fn dialogue_dossier(&self, sorte: &'static str, titre: &str, depart: Option<&Path>) -> Option<PathBuf> {
        choisir_dossier(sorte, titre, self.dossier_du_projet().as_deref(), depart)
    }

    /// Le dossier où proposer « Enregistrer sous » : à côté du dossier du projet (un dossier à ce nom sera créé).
    pub(crate) fn dossier_pour_enregistrer_sous(&self) -> Option<PathBuf> {
        let projet = self.dossier_du_projet()?;
        projet.parent().filter(|p| p.is_dir()).map(Path::to_path_buf).or(Some(projet).filter(|p| p.is_dir()))
    }
}
