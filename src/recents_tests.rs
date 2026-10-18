//! Projets récents : en tête, sans doublon, dix au plus ; le nom montré.

use std::path::{Path, PathBuf};

use crate::recents::{label, remember, MAX};

#[test]
fn the_last_project_goes_first_without_duplicates() {
    let mut list: Vec<PathBuf> = Vec::new();
    remember(&mut list, Path::new(r"C:\Films\Bus\Bus.pelure"));
    remember(&mut list, Path::new(r"C:\Films\Car\Car.pelure"));
    remember(&mut list, Path::new(r"c:\films\bus\BUS.pelure"));
    assert_eq!(list.len(), 2, "le même projet, la casse ignorée");
    assert_eq!(list[0], PathBuf::from(r"c:\films\bus\BUS.pelure"));
    for i in 0..20 {
        remember(&mut list, &PathBuf::from(format!(r"C:\P{i}\P{i}.pelure")));
    }
    assert_eq!(list.len(), MAX);
    assert_eq!(list[0], PathBuf::from(r"C:\P19\P19.pelure"));
}

#[test]
fn a_recent_project_shows_its_name_and_place() {
    assert_eq!(label(Path::new(r"C:\Films\Bus\Bus.pelure")), r"Bus  —  C:\Films");
}

/// La présence des projets récents se vérifie sur un autre fil : inconnue, elle compte pour
/// « là » ; la réponse vient ensuite (et réveille l'interface si elle change) ; une nouvelle
/// vérification n'est faite qu'après le délai.
#[test]
fn the_recent_projects_presence_is_checked_off_the_interface_thread() {
    let dir = std::env::temp_dir().join(format!("shot-recents-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let here = dir.join("Ici.pelure");
    std::fs::write(&here, b"x").unwrap();
    let gone = dir.join("Parti").join("Parti.pelure");
    let ctx = eframe::egui::Context::default();
    let mut presence = crate::recents::Presence::default();
    // Avant toute réponse : présents.
    assert!(presence.exists(&here) && presence.exists(&gone));
    presence.refresh(&ctx, &[here.clone(), gone.clone()]);
    let t = std::time::Instant::now();
    while presence.exists(&gone) && t.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!presence.exists(&gone), "le projet absent doit finir par l'être pour l'interface");
    assert!(presence.exists(&here));
    // Dans le délai, rien n'est revérifié : un projet qui apparaît n'est pas vu tout de suite.
    std::fs::create_dir_all(gone.parent().unwrap()).unwrap();
    std::fs::write(&gone, b"x").unwrap();
    presence.refresh(&ctx, &[here.clone(), gone.clone()]);
    std::thread::sleep(std::time::Duration::from_millis(80));
    assert!(!presence.exists(&gone), "pas de nouvelle vérification avant 3 s");
    let _ = std::fs::remove_dir_all(&dir);
}