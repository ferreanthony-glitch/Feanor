//! Les mots et les programmes qui changent d'un système à l'autre (audit du 2026-11-21, le Mac entre dans la bêta) : le nom du
//! gestionnaire de fichiers dans les libellés (« l'Explorateur » sous Windows, « le Finder » sur Mac), la commande qui y montre un
//! fichier, celle qui ouvre un fichier texte, et le décalage de l'heure locale. Tout est décidé à la compilation (`cfg!`) : un
//! libellé reste un `&'static str`, utilisable dans la table des commandes.

use std::path::Path;

/// Quel système, pour les fonctions qui se testent sur les trois.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Systeme {
    Windows,
    Mac,
    Autre,
}

pub const ICI: Systeme = if cfg!(windows) {
    Systeme::Windows
} else if cfg!(target_os = "macos") {
    Systeme::Mac
} else {
    Systeme::Autre
};

/// « Montrer dans … » : le bouton qui ouvre le dossier d'un fichier écrit.
pub const MONTRER: &str = match ICI {
    Systeme::Windows => "Montrer dans l'explorateur",
    Systeme::Mac => "Montrer dans le Finder",
    Systeme::Autre => "Montrer dans le dossier",
};

/// « Afficher le projet dans … » (menu Fichier).
pub const AFFICHER_PROJET: &str = match ICI {
    Systeme::Windows => "Afficher le projet dans l'Explorateur",
    Systeme::Mac => "Afficher le projet dans le Finder",
    Systeme::Autre => "Afficher le dossier du projet",
};

/// Le même, dans la liste des raccourcis.
pub const AFFICHER_PROJET_LONG: &str = match ICI {
    Systeme::Windows => "Afficher le projet dans l'Explorateur Windows",
    Systeme::Mac => "Afficher le projet dans le Finder",
    Systeme::Autre => "Afficher le dossier du projet",
};

/// « Afficher dans … » (clic droit sur un onglet de projet).
pub const AFFICHER_DANS: &str = match ICI {
    Systeme::Windows => "Afficher dans l'Explorateur",
    Systeme::Mac => "Afficher dans le Finder",
    Systeme::Autre => "Afficher le dossier",
};

/// « Ouvrir le dossier dans … » (Bibliothèque).
pub const OUVRIR_DOSSIER: &str = match ICI {
    Systeme::Windows => "Ouvrir le dossier dans l'Explorateur",
    Systeme::Mac => "Ouvrir le dossier dans le Finder",
    Systeme::Autre => "Ouvrir le dossier",
};

/// Le même, dans la liste des raccourcis.
pub const OUVRIR_DOSSIER_LONG: &str = match ICI {
    Systeme::Windows => "Bibliothèque : ouvrir le dossier du fichier choisi dans l'Explorateur",
    Systeme::Mac => "Bibliothèque : ouvrir le dossier du fichier choisi dans le Finder",
    Systeme::Autre => "Bibliothèque : ouvrir le dossier du fichier choisi",
};

/// La fin de la phrase « Enregistré dans le dossier … » : où retrouver le projet.
pub const OU_RETROUVER: &str = match ICI {
    Systeme::Windows => "Fichier > Afficher le projet dans l'Explorateur.",
    Systeme::Mac => "Fichier > Afficher le projet dans le Finder.",
    Systeme::Autre => "Fichier > Afficher le dossier du projet.",
};

/// La bulle du bouton d'un message d'export.
pub const OUVRE_SUR_EXPORT: &str = match ICI {
    Systeme::Windows => "Ouvre l'explorateur sur ce qui vient d'être exporté.",
    Systeme::Mac => "Ouvre le Finder sur ce qui vient d'être exporté.",
    Systeme::Autre => "Ouvre le dossier de ce qui vient d'être exporté.",
};

/// Le programme et ses arguments pour montrer `chemin` dans le gestionnaire de fichiers : un fichier y est choisi quand le système
/// sait le faire (Windows, Mac), sinon son dossier s'ouvre. Sous Windows l'argument est « brut » (`raw_arg`) : `/select,"chemin"`.
pub fn commande_montrer(systeme: Systeme, chemin: &Path, est_fichier: bool) -> (&'static str, Vec<String>) {
    let texte = chemin.display().to_string();
    match systeme {
        Systeme::Windows if est_fichier => ("explorer.exe", vec![format!("/select,\"{texte}\"")]),
        Systeme::Windows => ("explorer.exe", vec![format!("\"{texte}\"")]),
        Systeme::Mac if est_fichier => ("open", vec!["-R".into(), texte]),
        Systeme::Mac => ("open", vec![texte]),
        Systeme::Autre => {
            let dossier = if est_fichier { chemin.parent().map(|d| d.display().to_string()).unwrap_or(texte) } else { texte };
            ("xdg-open", vec![dossier])
        }
    }
}

/// Le programme et ses arguments pour ouvrir un fichier texte à modifier, et le nom à écrire dans le message.
pub fn commande_editer(systeme: Systeme, chemin: &Path) -> (&'static str, Vec<String>, &'static str) {
    let texte = chemin.display().to_string();
    match systeme {
        Systeme::Windows => ("notepad.exe", vec![texte], "le Bloc-notes"),
        // `open -t` : l'éditeur de texte par défaut (TextEdit, sauf choix de l'utilisateur).
        Systeme::Mac => ("open", vec!["-t".into(), texte], "l'éditeur de texte"),
        Systeme::Autre => ("xdg-open", vec![texte], "l'éditeur de texte"),
    }
}
