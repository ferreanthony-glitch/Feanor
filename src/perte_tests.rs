//! Contre-vérification de 55b35a3 (N0) : aucune perte de données à la relecture d'un dessin.
//! - les traces écrites en flottants par d01fffe (même version de fichier 24) se relisent ;
//! - un dessin illisible n'est jamais supprimé par l'enregistrement suivant.

use crate::document::{Document, LayerKind};
use crate::project;
use crate::vector::Trace;

#[test]
fn traces_written_as_floats_by_the_earlier_build_still_load() {
    // Format de d01fffe : appuis en flottants, instants en secondes (−1 : inconnu).
    let old: Trace = serde_json::from_str(r#"{"appui":[0.5,1.0,0.25],"instants":[0.0,0.005,-1.0]}"#).unwrap();
    assert_eq!(old.pressure, vec![0.5, 1.0, 0.25]);
    assert_eq!(old.time, vec![0.0, 0.005, -1.0]);
    // Format quantifié d'aujourd'hui (entiers).
    let new: Trace = serde_json::from_str(r#"{"appui":[32768,65535],"instants":[0,50,4294967295]}"#).unwrap();
    assert!((new.pressure[0] - 0.5).abs() < 1e-4 && new.pressure[1] == 1.0);
    assert_eq!(new.time, vec![0.0, 0.005, -1.0]);
    // Et l'aller-retour d'aujourd'hui.
    let back: Trace = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
    assert_eq!(back.time, new.time);
    assert!(back.pressure.iter().zip(&new.pressure).all(|(a, b)| (a - b).abs() < 1e-4));
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn an_unreadable_drawing_is_not_deleted_by_the_next_save() {
    let dir = temp_dir("illisible");
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames = vec![Some(a); 48];
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let file = manifest.parent().unwrap().join("dessins").join(format!("{a}.json"));
    assert!(file.exists(), "dessin vectoriel enregistré");
    // Le fichier devient illisible (par exemple un format qu'on ne sait plus lire).
    let garbage = br#"[{"Stroke":{"trace":{"appui":"?"#;
    std::fs::write(&file, garbage).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.is_empty(), "avertissement");
    assert!(!loaded.doc.cels.contains_key(&a), "le dessin n'est pas dans le projet ouvert");
    // L'enregistrement suivant ne l'efface pas.
    let mut written = loaded.written;
    project::save(&loaded.doc, &manifest, &mut written).unwrap();
    assert!(file.exists(), "le fichier illisible est gardé");
    assert_eq!(std::fs::read(&file).unwrap(), garbage);
}
