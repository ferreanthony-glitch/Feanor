//! Enregistrer sur un autre fil (`enregistrement.rs`) : `modified` reste juste quand on modifie
//! pendant l'écriture, Quitter attend, un échec ne perd rien, l'écriture est atomique.

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::Color32;

use crate::app::Pending;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::perf_tests::{banc, Banc};
use crate::project;

fn petit_projet(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(k)[0] = Color32::from_rgba_premultiplied(200, 10, 10, 255);
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

fn dossier(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-enreg-{}-{nom}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

fn banc_enregistre(nom: &str) -> (Banc, PathBuf, PathBuf) {
    let dir = dossier(nom);
    let mut b = banc(petit_projet(6), false);
    let manifeste = dir.join("Essai.pelure");
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    (b, dir, manifeste)
}

/// Le dessin de la première image, modifié comme un trait : pixel changé, version et projet modifié.
fn modifier(b: &mut Banc, rouge: u8) -> CelId {
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    let cel = b.app.doc.cels.get_mut(&id).expect("dessin");
    cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(rouge, 10, 10, 255);
    cel.version += 1;
    b.app.modified = true;
    id
}

/// Le rouge du premier pixel du premier dessin tel que le fichier le garde.
fn rouge_sur_disque(manifeste: &PathBuf) -> u8 {
    let loaded = project::load(manifeste).expect("projet lisible");
    let id = loaded.doc.panels[0].layers[0].frames[0].expect("dessin");
    loaded.doc.cels[&id].raster.get(0, 0).r()
}

/// Des images jusqu'à la fin de l'écriture (l'interface continue de tourner pendant ce temps).
fn pomper(b: &mut Banc) {
    for _ in 0..600 {
        if !b.app.save_en_cours() {
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("l'enregistrement ne se termine pas");
}

#[test]
fn a_background_save_leaves_modified_alone_when_nothing_changed_since_the_snapshot() {
    let (mut b, dir, manifeste) = banc_enregistre("propre");
    assert!(b.app.save());
    assert!(b.app.save_en_cours(), "l'écriture est partie sur un autre fil");
    pomper(&mut b);
    assert!(!b.app.modified, "rien n'a changé depuis le cliché : enregistré");
    assert_eq!(b.app.path, Some(manifeste.clone()));
    assert_eq!(rouge_sur_disque(&manifeste), 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_background_save_keeps_modified_when_the_project_changes_during_the_write() {
    let (mut b, dir, manifeste) = banc_enregistre("concurrent");
    assert!(b.app.save());
    // Un trait pendant l'écriture : le projet n'est plus celui du fichier.
    modifier(&mut b, 90);
    pomper(&mut b);
    assert!(b.app.modified, "le trait posé pendant l'écriture n'est pas dans le fichier");
    assert_eq!(rouge_sur_disque(&manifeste), 200, "le fichier est le cliché, pas ce qui a changé après");
    // Le trait est écrit à l'enregistrement suivant ; alors, tout est à jour.
    assert!(b.app.save());
    pomper(&mut b);
    assert!(!b.app.modified);
    assert_eq!(rouge_sur_disque(&manifeste), 90);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_save_asked_during_the_write_follows_it() {
    let (mut b, dir, manifeste) = banc_enregistre("file");
    assert!(b.app.save());
    modifier(&mut b, 120);
    assert!(b.app.save(), "demandé pendant l'écriture : mis en file");
    pomper(&mut b);
    assert!(!b.app.modified, "les deux enregistrements sont faits");
    assert_eq!(rouge_sur_disque(&manifeste), 120);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_waits_for_the_save_to_finish() {
    let (mut b, dir, manifeste) = banc_enregistre("quitter");
    assert!(b.app.save());
    b.app.request(Pending::Quit);
    assert!(b.app.save_en_cours() || b.app.allow_close, "la demande est retenue ou déjà satisfaite");
    if b.app.save_en_cours() {
        assert!(!b.app.allow_close, "on ne quitte pas pendant que le projet s'écrit");
        assert!(b.app.after_save.is_some());
    }
    pomper(&mut b);
    // L'écriture est finie et rien n'a changé depuis : la demande de quitter est refaite et passe.
    assert!(b.app.allow_close, "à la fin de l'écriture, Quitter a lieu");
    assert!(!b.app.modified);
    assert_eq!(rouge_sur_disque(&manifeste), 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_asks_again_when_the_project_changed_during_the_write() {
    let (mut b, dir, manifeste) = banc_enregistre("quitter-modifie");
    assert!(b.app.save());
    b.app.request(Pending::Quit);
    modifier(&mut b, 60);
    pomper(&mut b);
    assert!(!b.app.allow_close, "le projet a changé pendant l'écriture : on ne quitte pas sans demander");
    assert!(b.app.pending.is_some(), "la boîte « Modifications non enregistrées » s'ouvre");
    assert_eq!(rouge_sur_disque(&manifeste), 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_exit_waits_for_a_running_write() {
    let (mut b, dir, manifeste) = banc_enregistre("fermeture");
    assert!(b.app.save());
    b.app.fermeture();
    assert!(!b.app.save_en_cours(), "la fermeture a attendu l'écriture");
    assert_eq!(rouge_sur_disque(&manifeste), 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_save_button_of_the_unsaved_changes_box_runs_the_action_afterwards() {
    let (mut b, dir, _) = banc_enregistre("suite");
    b.app.request(Pending::Quit);
    assert!(b.app.pending.is_some(), "modifié : la boîte demande");
    // « Enregistrer » dans la boîte : l'écriture, puis l'action.
    let action = b.app.pending.take().expect("action");
    assert!(b.app.save_then(Some(action)));
    assert!(!b.app.allow_close, "pas avant la fin de l'écriture");
    pomper(&mut b);
    assert!(b.app.allow_close, "l'action suit l'enregistrement");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_save_keeps_the_project_modified_and_cancels_what_followed() {
    let (mut b, dir, manifeste) = banc_enregistre("echec");
    assert!(b.app.save());
    pomper(&mut b);
    assert!(!b.app.modified);
    let avant = std::fs::read(&manifeste).expect("manifeste");
    // Le manifeste ne peut plus être écrit (son fichier provisoire est occupé par un dossier) : les
    // dessins passent, puis l'échec.
    std::fs::create_dir_all(dir.join("Essai.pelure.tmp")).expect("obstacle");
    modifier(&mut b, 33);
    assert!(b.app.save());
    b.app.request(Pending::Quit);
    assert!(b.app.after_save.is_some());
    pomper(&mut b);
    assert!(b.app.modified, "l'échec garde le projet modifié");
    assert!(b.app.save_error.is_some(), "une fenêtre dit pourquoi");
    assert!(b.app.status.starts_with("Échec de l'enregistrement"), "{}", b.app.status);
    assert!(!b.app.allow_close && b.app.after_save.is_none(), "Quitter est annulé");
    assert_eq!(std::fs::read(&manifeste).expect("manifeste"), avant, "l'ancien manifeste est intact");
    assert!(project::load(&manifeste).is_ok(), "le projet reste lisible");
    // L'obstacle levé, l'enregistrement suivant réécrit tout et réussit.
    std::fs::remove_dir_all(dir.join("Essai.pelure.tmp")).expect("obstacle levé");
    assert!(b.app.save());
    pomper(&mut b);
    assert!(!b.app.modified);
    assert_eq!(rouge_sur_disque(&manifeste), 33);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_finishing_after_another_project_opened_does_not_touch_the_new_one() {
    let (mut b, dir, manifeste) = banc_enregistre("autre-projet");
    assert!(b.app.save());
    // Un autre projet prend la place (ouverture par glisser-déposer, par exemple).
    b.app.load_doc(petit_projet(2));
    pomper(&mut b);
    assert_eq!(b.app.path, None, "le nouveau projet n'est pas à ce chemin");
    assert!(!b.app.modified);
    assert_eq!(rouge_sur_disque(&manifeste), 200, "l'ancien projet est bien écrit");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_atomic_write_survives_a_failure_halfway() {
    let dir = dossier("atomique");
    let fichier = dir.join("bon.json");
    project::write_atomic(&fichier, b"version 1").expect("premier");
    assert_eq!(std::fs::read(&fichier).unwrap(), b"version 1");
    // Écriture impossible (le fichier provisoire est occupé) : l'ancien contenu reste, entier.
    let tmp = dir.join("bon.json.tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    assert!(project::write_atomic(&fichier, b"version 2 qui ne passera pas").is_err());
    assert_eq!(std::fs::read(&fichier).unwrap(), b"version 1", "l'ancien fichier n'est pas touché");
    std::fs::remove_dir_all(&tmp).unwrap();
    // Remplacement impossible (la destination est un dossier) : erreur, pas de reste provisoire.
    let cible = dir.join("dossier.json");
    std::fs::create_dir_all(&cible).unwrap();
    assert!(project::write_atomic(&cible, b"x").is_err());
    assert!(!dir.join("dossier.json.tmp").exists(), "le fichier provisoire est effacé après l'échec");
    // Et quand tout va bien, aucun fichier provisoire ne reste.
    project::write_atomic(&fichier, b"version 3").unwrap();
    assert_eq!(std::fs::read(&fichier).unwrap(), b"version 3");
    assert!(!tmp.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_save_cut_before_the_manifest_leaves_the_previous_project_readable() {
    let dir = dossier("coupe");
    let manifeste = dir.join("Essai.pelure");
    let mut doc = petit_projet(4);
    let mut written = Default::default();
    project::save(&doc, &manifeste, &mut written).expect("premier");
    let avant = std::fs::read(&manifeste).unwrap();
    // Un dessin change, mais l'écriture est coupée au manifeste.
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(5, 5, 5, 255);
    cel.version += 1;
    std::fs::create_dir_all(dir.join("Essai.pelure.tmp")).unwrap();
    assert!(project::save(&doc, &manifeste, &mut written).is_err());
    assert!(written.is_empty(), "après un échec, tout sera réécrit");
    assert_eq!(std::fs::read(&manifeste).unwrap(), avant);
    let loaded = project::load(&manifeste).expect("l'ancien projet se lit");
    assert_eq!(loaded.doc.panels[0].layers[0].frames.len(), 4);
    let _ = std::fs::remove_dir_all(&dir);
}
