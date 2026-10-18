//! Reprise des données de l'ancien nom (Pelure) dans le dossier SHOT.

use std::fs;
use std::path::PathBuf;

use crate::donnees::{reprendre, ANCIEN_NOM, NOM};

fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pelure-test-donnees-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Un ancien dossier garni : un fichier à la racine, un sous-dossier avec un fichier.
fn garnir(ancien: &PathBuf) {
    fs::create_dir_all(ancien.join("pinceaux")).unwrap();
    fs::write(ancien.join("reglages.toml"), "version = 1\n").unwrap();
    fs::write(ancien.join("pinceaux").join("pinceaux.json"), "{}").unwrap();
}

#[test]
fn les_noms_sont_ceux_du_logiciel() {
    assert_eq!(NOM, "SHOT");
    assert_eq!(ANCIEN_NOM, "Pelure");
}

#[test]
fn l_ancien_dossier_est_copie_sans_etre_touche() {
    let base = temp("copie");
    let (ancien, nouveau) = (base.join(ANCIEN_NOM), base.join(NOM));
    garnir(&ancien);
    assert!(reprendre(&ancien, &nouveau).unwrap());
    assert_eq!(fs::read_to_string(nouveau.join("reglages.toml")).unwrap(), "version = 1\n");
    assert_eq!(fs::read_to_string(nouveau.join("pinceaux").join("pinceaux.json")).unwrap(), "{}");
    // L'ancien est intact, et rien ne traîne à côté.
    assert_eq!(fs::read_to_string(ancien.join("reglages.toml")).unwrap(), "version = 1\n");
    assert!(ancien.join("pinceaux").join("pinceaux.json").exists());
    assert!(!base.join("SHOT.reprise").exists());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn un_dossier_shot_deja_la_n_est_pas_ecrase() {
    let base = temp("existe");
    let (ancien, nouveau) = (base.join(ANCIEN_NOM), base.join(NOM));
    garnir(&ancien);
    fs::create_dir_all(&nouveau).unwrap();
    fs::write(nouveau.join("reglages.toml"), "version = 2\n").unwrap();
    assert!(!reprendre(&ancien, &nouveau).unwrap());
    assert_eq!(fs::read_to_string(nouveau.join("reglages.toml")).unwrap(), "version = 2\n");
    assert!(!nouveau.join("pinceaux").exists(), "rien de l'ancien n'est mélangé au nouveau");
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn sans_ancien_dossier_rien_n_est_cree() {
    let base = temp("vide");
    let (ancien, nouveau) = (base.join(ANCIEN_NOM), base.join(NOM));
    assert!(!reprendre(&ancien, &nouveau).unwrap());
    assert!(!nouveau.exists());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn un_reste_de_copie_interrompue_est_refait() {
    let base = temp("reste");
    let (ancien, nouveau) = (base.join(ANCIEN_NOM), base.join(NOM));
    garnir(&ancien);
    fs::create_dir_all(base.join("SHOT.reprise")).unwrap();
    fs::write(base.join("SHOT.reprise").join("reste.txt"), "x").unwrap();
    assert!(reprendre(&ancien, &nouveau).unwrap());
    assert!(nouveau.join("reglages.toml").exists());
    assert!(!nouveau.join("reste.txt").exists());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn le_dossier_shot_reprend_l_ancien_une_seule_fois_par_seance() {
    let base = temp("sous");
    garnir(&base.join(ANCIEN_NOM));
    let d = crate::donnees::sous(&base);
    assert_eq!(d, base.join(NOM));
    assert!(d.join("reglages.toml").exists());
    // Le dossier SHOT effacé : la même séance ne recopie pas l'ancien une seconde fois.
    fs::remove_dir_all(&d).unwrap();
    assert_eq!(crate::donnees::sous(&base), d);
    assert!(!d.exists());
    // Sans ancien dossier, on obtient le chemin SHOT sans rien créer.
    let autre = temp("sous-neuf");
    assert_eq!(crate::donnees::sous(&autre), autre.join(NOM));
    assert!(!autre.join(NOM).exists());
    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&autre);
}
