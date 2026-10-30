//! Essais de « Stabilité et retouches de l'audit » (2026-10-30) : l'annulation des calques séparés n'est suivie d'aucune écriture
//! tardive, « Marquer l'envoi » n'enregistre pas en silence un projet à l'ancien nom, le calque vidéo propose de devenir un calque
//! d'animation, Échap ferme les fenêtres d'export, la fin d'un export s'efface seule, l'ancien `crayonnes.json` se met de côté.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{Event, Key, Modifiers};

use crate::document::{Document, Layer, LayerKind};
use crate::export::{self, ExportJob};
use crate::exportlayers::{self, LayersSettings, Porte};
use crate::messages::ActionMessage;
use crate::perf_tests::{banc, Banc};
use crate::project;

fn dossier(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-audit-{}-{nom}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

fn fichiers(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entrees) = std::fs::read_dir(dir) {
        for e in entrees.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.push(p.clone());
                out.extend(fichiers(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn echap() -> Event {
    Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

// ------------------------------------------------------------------------------------ A1 : annuler les calques séparés

#[test]
fn the_door_refuses_every_write_after_the_cleanup_and_the_cleanup_waits_for_a_write_in_progress() {
    let porte = Arc::new(Porte::default());
    // Une écriture en cours : le nettoyage l'attend.
    assert!(porte.entrer());
    let p = porte.clone();
    let nettoyage = std::thread::spawn(move || p.fermer_et_attendre());
    std::thread::sleep(Duration::from_millis(60));
    assert!(!nettoyage.is_finished(), "le nettoyage attend l'écriture en cours");
    porte.sortir();
    nettoyage.join().unwrap();
    // Porte fermée : plus aucune écriture ne commence.
    assert!(!porte.entrer());
}

#[test]
fn cancelling_the_layer_export_leaves_nothing_even_when_a_render_thread_finishes_later() {
    for tour in 0..4 {
        let dir = dossier(&format!("annule-{tour}"));
        let longue = crate::export_tests::doc_with_drawing(1, 30);
        let frames = export::frames_for(&longue, export::Range::Montage, 0);
        let info = exportlayers::plans_info(&longue, &[0], false);
        let mut job = exportlayers::job(&dir, info, frames, &LayersSettings::default(), false);
        let t = Instant::now();
        while job.outcome.is_none() && !matches!(job.progress(), export::Progress::Running(n, _) if n >= 1 + tour) {
            job.step(&longue, Duration::from_millis(8));
            std::thread::sleep(Duration::from_millis(1));
            assert!(t.elapsed() < Duration::from_secs(120));
        }
        job.cancel();
        assert!(fichiers(&dir).is_empty(), "tour {tour} : rien ne reste juste après : {:?}", fichiers(&dir));
        // Les fils de rendu qui finissaient une image n'écrivent plus rien après coup.
        std::thread::sleep(Duration::from_millis(1500));
        assert!(fichiers(&dir).is_empty(), "tour {tour} : une écriture tardive a laissé : {:?}", fichiers(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ------------------------------------------------------------------------------------ B1 : Marquer l'envoi

fn doc_dessine() -> Document {
    crate::export_tests::doc_with_drawing(2, 3)
}

fn attendre_envoi(b: &mut Banc) {
    let debut = Instant::now();
    while debut.elapsed() < Duration::from_secs(90) {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() && !b.app.save_en_cours() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("l'envoi ne se termine pas");
}

#[test]
fn marking_an_envoi_in_an_old_pelure_project_does_not_save_and_rename_it_silently() {
    let dir = dossier("envoi-pelure");
    let doc = doc_dessine();
    let ancien = dir.join("Film.pelure");
    project::save(&doc, &ancien, &mut Default::default()).expect("enregistrement");
    let mut b = banc(doc, false);
    b.app.path = Some(ancien.clone());
    b.app.modified = false;
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1 au réal", crate::envois::lire_date("20/10/2026").unwrap());
    attendre_envoi(&mut b);
    assert_eq!(b.app.doc.envois.list.len(), 1, "l'envoi est dans le projet");
    assert!(b.app.modified, "le projet reste à enregistrer");
    assert_eq!(b.app.path, Some(ancien.clone()), "le chemin ne change pas");
    assert!(ancien.exists() && !dir.join("Film.shot").exists(), "aucun .shot n'a été créé, le .pelure n'a pas été renommé");
    assert!(b.app.status.contains("Ctrl+S") && b.app.status.contains(".shot"), "le message dit ce qui se passera : {}", b.app.status);
    // Un projet déjà en .shot s'enregistre, lui, comme avant.
    let neuf = dir.join("Autre.shot");
    project::save(&b.app.doc, &neuf, &mut Default::default()).expect("enregistrement");
    b.app.path = Some(neuf.clone());
    b.app.doc.envois = Default::default();
    b.app.envois_marquer(&ctx, "v1", crate::envois::lire_date("20/10/2026").unwrap());
    attendre_envoi(&mut b);
    assert!(!b.app.modified, "un projet .shot est enregistré par l'envoi");
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------------------------ B2 : calque vidéo

#[test]
fn drawing_on_a_video_layer_offers_the_button_that_turns_it_into_an_animation_layer() {
    let dir = dossier("video");
    let mut doc = Document::with_frame(16, 16, 24);
    let clip = crate::video::VideoClip { id: crate::video::new_id(), name: "essai.mp4".into(), frames: 2, fps: 24.0, start: 0, ext: "png".into() };
    for k in 0..2 {
        image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255])).save(crate::video::frame_file(&dir, k, "png")).unwrap();
    }
    crate::video::register_dir(clip.id, dir.clone());
    let mut calque = Layer::new("Vidéo", LayerKind::Bitmap);
    calque.video = Some(clip);
    doc.panels[0].layers = vec![calque];
    let mut b = banc(doc, false);
    b.app.cur_layer = 0;
    assert!(!b.app.layer_editable());
    assert_eq!(b.app.action_du_message(), Some(ActionMessage::VideoEnAnimation));
    assert!(b.app.status.contains("calque d'animation"));
    b.app.lancer_action_message(ActionMessage::VideoEnAnimation);
    assert!(b.app.doc.panels[0].layers[0].video.is_none(), "le calque est devenu un calque d'animation");
    // Le bouton ne vaut que pour son message : un autre message l'efface.
    assert_eq!(b.app.action_du_message(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------------------------ B4 : Échap

#[test]
fn escape_closes_the_export_window_without_starting_anything() {
    let mut b = banc(doc_dessine(), false);
    b.app.export_hub = true;
    b.image(Vec::new(), 0.1);
    b.image(Vec::new(), 0.1);
    assert!(b.app.export_hub, "la fenêtre est ouverte");
    b.image(vec![echap()], 0.1);
    assert!(!b.app.export_hub, "Échap la ferme");
    assert!(b.app.export_job.is_none() && b.app.export_dialog.is_none(), "rien n'est lancé");
    // La fenêtre d'un export choisi se ferme aussi, sans rien lancer.
    b.app.open_export(export::Format::Mp4);
    b.image(Vec::new(), 0.1);
    b.image(Vec::new(), 0.1);
    assert!(b.app.export_dialog.is_some());
    b.image(vec![echap()], 0.1);
    assert!(b.app.export_dialog.is_none(), "Échap ferme la fenêtre d'export");
    assert!(b.app.export_job.is_none());
}

// ------------------------------------------------------------------------------------ B8 : fin d'export

fn export_fini(b: &mut Banc, dir: &Path, resultat: Result<(), String>) {
    let sortie = dir.join("sortie.txt");
    let message = format!("Exporté : {}", sortie.display());
    let job = ExportJob::task("Essai".into(), &sortie, message, move |_, _| resultat.clone());
    b.app.start_job(job);
    let debut = Instant::now();
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) {
        b.image(Vec::new(), 0.05);
        std::thread::sleep(Duration::from_millis(2));
        assert!(debut.elapsed() < Duration::from_secs(30));
    }
}

#[test]
fn the_end_of_a_successful_export_clears_itself_and_keeps_the_open_folder_button() {
    let dir = dossier("export-ok");
    let mut b = banc(doc_dessine(), false);
    export_fini(&mut b, &dir, Ok(()));
    b.image(Vec::new(), 0.1);
    assert!(b.app.export_job.is_some(), "la fenêtre de fin est là");
    for _ in 0..6 {
        b.image(Vec::new(), 1.0);
    }
    assert!(b.app.export_job.is_some(), "encore là après six secondes");
    for _ in 0..4 {
        b.image(Vec::new(), 1.0);
    }
    assert!(b.app.export_job.is_none(), "elle s'efface seule au bout de quelques secondes");
    assert!(matches!(b.app.action_du_message(), Some(ActionMessage::MontrerDossier(_))), "« Ouvrir le dossier » reste dans la barre d'état");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_end_of_a_failed_export_stays_until_clicked() {
    let dir = dossier("export-erreur");
    let mut b = banc(doc_dessine(), false);
    export_fini(&mut b, &dir, Err("disque plein".into()));
    for _ in 0..40 {
        b.image(Vec::new(), 1.0);
    }
    assert!(b.app.export_job.is_some(), "une erreur reste jusqu'au clic");
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------------------------ B5 : crayonnes.json

const CRAYONNES: &str = r#"{"format":"shot-crayonnes","version":1,"grille":[4,3],"taille":[640,360],"cases":[{"id":1,"traits":[{"x":1}],"utilisee":false}]}"#;

#[test]
fn the_old_crayonnes_warning_has_a_button_that_sets_the_file_aside_and_undoes() {
    let dir = dossier("crayonnes");
    let manifeste = dir.join("P.shot");
    project::save(&doc_dessine(), &manifeste, &mut Default::default()).expect("enregistrement");
    let fichier = dir.join("crayonnes.json");
    std::fs::write(&fichier, CRAYONNES).unwrap();
    let mut b = banc(Document::with_frame(16, 16, 24), false);
    b.app.finish_open(manifeste.clone(), project::load(&manifeste));
    assert!(b.app.status.starts_with("Ouvert avec") && b.app.status.contains("crayonnes.json"), "{}", b.app.status);
    let Some(action) = b.app.action_du_message() else { panic!("le message n'a pas de bouton") };
    assert_eq!(action, ActionMessage::CrayonnesDeCote(dir.clone()));
    b.app.lancer_action_message(action);
    assert!(!fichier.exists() && dir.join("crayonnes.json.ancien").exists(), "mis de côté, pas effacé");
    assert_eq!(std::fs::read_to_string(dir.join("crayonnes.json.ancien")).unwrap(), CRAYONNES, "octet pour octet");
    // Annuler le remet ; rouvert, l'avis revient.
    let Some(annuler) = b.app.action_du_message() else { panic!("pas de bouton Annuler") };
    assert_eq!(annuler, ActionMessage::CrayonnesRemettre(dir.clone()));
    b.app.lancer_action_message(annuler);
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), CRAYONNES);
    assert!(!dir.join("crayonnes.json.ancien").exists());
    // Mis de côté, l'avis ne revient plus à l'ouverture.
    b.app.crayonnes_de_cote(&dir);
    let rouvert = project::load(&manifeste).expect("ouverture");
    assert!(rouvert.warnings.iter().all(|w| !w.contains("crayonnes.json")), "{:?}", rouvert.warnings);
    let _ = std::fs::remove_dir_all(&dir);
}
