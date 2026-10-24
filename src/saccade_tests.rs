//! Essais de la postérisation temporelle de la caméra (« saccade », `camera::Saccade`, `Document::panel_camera`).
//!
//! Ce qu'ils gardent :
//! - avec N = 3, la caméra est constante sur chaque palier de 3 images et vaut la caméra douce du début du palier ;
//! - éteinte, la caméra est exactement la caméra douce (et la rallumer puis l'éteindre ne laisse aucune trace) ;
//! - aucune clé de caméra n'est posée, changée ni déplacée : tout se passe à l'évaluation ;
//! - la dernière clé est atteinte pile (le mouvement ne reste pas en deçà de son cadre final) ;
//! - un plan sans clé garde la caméra que le plan d'avant a **montrée** à sa dernière image ;
//! - les exports (`Rendu`) montrent les mêmes caméras que l'écran ;
//! - le réglage s'enregistre et se relit ; un projet d'avant, ou sans réglage, l'ouvre éteint ;
//! - couper un plan garde le réglage aux deux parties, réunir deux plans garde celui du premier ;
//! - l'outil Caméra règle les clés d'après la caméra douce, pas d'après un palier.

use std::sync::Arc;

use crate::camera::{Camera, CameraKey, Saccade};
use crate::document::Document;
use crate::export_tests::doc_with_drawing;
use crate::exportrender::{self, Papier, Rendu};
use crate::project;
use crate::render;

/// Un plan de `len` images dont la caméra glisse en ligne droite de x = 0 à x = 190 entre la première et la dernière image.
fn doc_en_mouvement(len: usize) -> Document {
    let mut doc = Document::with_frame(150, 90, 12);
    doc.panels[0].set_length(len);
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 0.0, ..c }), CameraKey::new(len - 1, Camera { x: 190.0, ..c })];
    doc
}

fn doux(doc: &Document, f: usize) -> Camera {
    crate::camera::camera_at(&doc.panels[0].camera, f, doc.default_camera())
}

#[test]
fn with_three_the_camera_is_constant_over_each_block_and_smooth_at_its_start() {
    let mut doc = doc_en_mouvement(20);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 3 };
    let derniere = 19;
    for f in 0..20 {
        let montree = doc.panel_camera(0, f);
        let debut = if f >= derniere { f } else { f - f % 3 };
        assert_eq!(montree, doux(&doc, debut), "image {f} : la caméra du début du palier ({debut})");
    }
    // Chaque palier de 3 images (jusqu'à la dernière clé) est constant, et deux paliers voisins diffèrent.
    for debut in (0..18).step_by(3) {
        let (a, b, c) = (doc.panel_camera(0, debut), doc.panel_camera(0, debut + 1), doc.panel_camera(0, debut + 2));
        assert_eq!((a, a), (b, c), "palier {debut}..{}", debut + 2);
        assert_ne!(a, doc.panel_camera(0, debut + 3), "le palier suivant bouge");
    }
    // La dernière clé est atteinte pile, et après elle la caméra ne bouge plus.
    assert_eq!(doc.panel_camera(0, 19), doc.panels[0].camera[1].cam);
}

#[test]
fn cadence_two_holds_every_other_frame_and_a_free_integer_is_kept() {
    let mut doc = doc_en_mouvement(13);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 2 };
    for f in (0..12).step_by(2) {
        assert_eq!(doc.panel_camera(0, f), doc.panel_camera(0, f + 1), "image {f}");
    }
    // Un entier libre : 7.
    doc.panels[0].saccade = Saccade { actif: true, cadence: 7 };
    assert_eq!(doc.panel_camera(0, 8), doux(&doc, 7));
    assert_eq!(doc.panel_camera(0, 13 - 1), doc.panels[0].camera[1].cam);
    // Moins de 2 n'existe pas : ramené à 2 ; un nombre énorme est borné.
    assert_eq!(Saccade { actif: true, cadence: 0 }.normalise().cadence, 2);
    assert_eq!(Saccade { actif: true, cadence: 1 }.normalise().cadence, 2);
    assert_eq!(Saccade { actif: true, cadence: usize::MAX }.normalise().cadence, crate::camera::SACCADE_MAX);
}

#[test]
fn off_is_exactly_the_smooth_camera_and_no_key_is_touched() {
    let mut doc = doc_en_mouvement(20);
    let cles = doc.panels[0].camera.clone();
    let lisses: Vec<Camera> = (0..20).map(|f| doc.panel_camera(0, f)).collect();
    assert_eq!(lisses, (0..20).map(|f| doux(&doc, f)).collect::<Vec<_>>());
    doc.panels[0].saccade = Saccade { actif: true, cadence: 4 };
    assert_ne!((0..20).map(|f| doc.panel_camera(0, f)).collect::<Vec<_>>(), lisses, "allumée, le mouvement est par paliers");
    assert_eq!(doc.panels[0].camera, cles, "aucune clé posée ni changée");
    // La caméra douce de l'outil Caméra ne suit pas les paliers.
    for f in 0..20 {
        assert_eq!(doc.panel_camera_douce(0, f), lisses[f], "image {f}");
    }
    // Éteinte (la cadence reste), on retrouve le mouvement doux à l'identique.
    doc.panels[0].saccade.actif = false;
    assert_eq!(doc.panels[0].saccade.cadence, 4, "la cadence se souvient");
    assert_eq!((0..20).map(|f| doc.panel_camera(0, f)).collect::<Vec<_>>(), lisses);
    assert_eq!(doc.panels[0].camera, cles);
}

#[test]
fn without_two_keys_there_is_nothing_to_hold() {
    let mut doc = doc_en_mouvement(10);
    doc.panels[0].camera.truncate(1);
    let avant: Vec<Camera> = (0..10).map(|f| doc.panel_camera(0, f)).collect();
    doc.panels[0].saccade = Saccade { actif: true, cadence: 3 };
    assert_eq!((0..10).map(|f| doc.panel_camera(0, f)).collect::<Vec<_>>(), avant);
    doc.panels[0].camera.clear();
    assert_eq!(doc.panel_camera(0, 5), doc.default_camera());
}

#[test]
fn a_keyless_plan_keeps_the_camera_the_plan_before_showed_on_its_last_frame() {
    let mut doc = doc_en_mouvement(10);
    doc.add_panel_after(0);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 4 };
    assert!(doc.panels[1].camera.is_empty());
    // La dernière image (9) est après la dernière clé (9) : la caméra y est la caméra finale, pile.
    assert_eq!(doc.panel_camera(1, 3), doc.panels[0].camera[1].cam);
    // Si la dernière clé est plus loin que la fin du plan, la dernière image montre son palier (8), et le plan sans clé aussi.
    let c = doc.default_camera();
    doc.panels[0].camera[1] = CameraKey::new(30, Camera { x: 190.0, ..c });
    let montree = doc.panel_camera(0, 9);
    assert_eq!(montree, doux(&doc, 8));
    assert_eq!(doc.panel_camera(1, 0), montree, "le plan suivant reprend ce qu'on a vu");
    assert_eq!(doc.panel_camera_douce(1, 0), doux(&doc, 9), "l'édition lit la caméra douce");
}

#[test]
fn exports_show_the_held_cameras() {
    let mut doc = doc_with_drawing(1, 12);
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x - 20.0, ..c }), CameraKey::new(11, Camera { x: c.x + 20.0, zoom: c.zoom * 1.2, ..c })];
    doc.panels[0].saccade = Saccade { actif: true, cadence: 3 };
    let rendu = Rendu::new(exportrender::snapshot_of(&doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default());
    let mut differe_du_doux = 0;
    for f in 0..12 {
        let image = crate::fxcache::avec_privee(256 << 20, || rendu.image(0, f)).expect("image");
        let plan = render::panel_plan_at(&doc, 0, f, doc.render_paper());
        let montree = render::camera_image(&doc, &plan, &doc.panel_camera(0, f));
        assert!(*image == montree, "image {f} de l'export : la caméra montrée");
        let douce = render::camera_image(&doc, &plan, &doc.panel_camera_douce(0, f));
        if douce != montree {
            differe_du_doux += 1;
        }
    }
    assert!(differe_du_doux >= 6, "l'export est vraiment par paliers ({differe_du_doux} images diffèrent du mouvement doux)");
    // Deux images du même palier sont identiques (la caméra se tient).
    let (a, b) = (rendu.image(0, 3).unwrap(), rendu.image(0, 5).unwrap());
    assert!(*a == *b);
}

fn dossier_neuf(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-saccade-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Change à la main la cadence de tous les objets `saccade` d'un manifeste ; renvoie combien il y en a.
fn regle_cadence(v: &mut serde_json::Value, cadence: u64) -> usize {
    match v {
        serde_json::Value::Object(m) => {
            let mut n = 0;
            if let Some(s) = m.get_mut("saccade") {
                s["cadence"] = serde_json::json!(cadence);
                n += 1;
            }
            n + m.iter_mut().map(|(_, x)| regle_cadence(x, cadence)).sum::<usize>()
        }
        serde_json::Value::Array(a) => a.iter_mut().map(|x| regle_cadence(x, cadence)).sum(),
        _ => 0,
    }
}

#[test]
fn the_setting_is_saved_and_old_projects_open_with_it_off() {
    let dir = dossier_neuf("projet");
    let manifest = dir.join("S.pelure");
    let mut doc = doc_en_mouvement(20);
    doc.add_scene_after(0);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 5 };
    // Éteint mais avec une cadence à lui : gardé aussi.
    doc.panels[1].saccade = Saccade { actif: false, cadence: 3 };
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].saccade, Saccade { actif: true, cadence: 5 });
    assert_eq!(back.panels[1].saccade, Saccade { actif: false, cadence: 3 });
    assert_eq!(back.panels[0].camera, doc.panels[0].camera, "les clés n'ont pas bougé");
    assert_eq!((0..20).map(|f| back.panel_camera(0, f)).collect::<Vec<_>>(), (0..20).map(|f| doc.panel_camera(0, f)).collect::<Vec<_>>());

    // Sans réglage (éteint, cadence 2) : rien n'est écrit, et le projet s'ouvre éteint, comme un projet d'avant.
    let mut doc2 = doc_en_mouvement(20);
    project::save(&doc2, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(!text.contains("saccade"), "rien d'écrit quand il n'y a pas de réglage");
    assert!(text.contains(&format!("\"version\": {}", project::VERSION)));
    let old = text.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 36");
    std::fs::write(&manifest, old).unwrap();
    let back2 = project::load(&manifest).unwrap().doc;
    assert_eq!(back2.panels[0].saccade, Saccade::default());
    assert!(!back2.panels[0].saccade.actif);

    // Une cadence abîmée (0) ou démesurée est ramenée dans ses bornes à la lecture.
    doc2.panels[0].saccade = Saccade { actif: true, cadence: 4 };
    project::save(&doc2, &manifest, &mut Default::default()).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    assert_eq!(regle_cadence(&mut json, 0), 1);
    std::fs::write(&manifest, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    assert_eq!(project::load(&manifest).unwrap().doc.panels[0].saccade, Saccade { actif: true, cadence: 2 });
    regle_cadence(&mut json, 9_999_999);
    std::fs::write(&manifest, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    assert_eq!(project::load(&manifest).unwrap().doc.panels[0].saccade.cadence, crate::camera::SACCADE_MAX);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cutting_a_plan_keeps_the_setting_on_both_parts_and_joining_keeps_the_first_ones() {
    let mut doc = doc_en_mouvement(20);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 3 };
    let second = doc.split_panel(0, 8).expect("coupé");
    assert_eq!(doc.panels[0].saccade, Saccade { actif: true, cadence: 3 });
    assert_eq!(doc.panels[second].saccade, Saccade { actif: true, cadence: 3 });
    // Réunis : le réglage du premier plan, et les clés (douces) sont celles d'avant la coupe.
    doc.panels[second].saccade = Saccade { actif: true, cadence: 5 };
    doc.merge_with_next(0).expect("réunis");
    assert_eq!(doc.panels[0].saccade, Saccade { actif: true, cadence: 3 });
    // Réunir d'abord, par `append_next` : les caméras tenues aux bords sont les caméras douces.
    let mut doc = doc_en_mouvement(20);
    doc.add_panel_after(0);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 4 };
    let attendu = doux(&doc, 19);
    doc.append_next(0);
    assert_eq!(doc.panels[0].camera.iter().find(|k| k.frame == 19).map(|k| k.cam.x), Some(attendu.x), "la clé de fin est celle du mouvement doux");
}

#[test]
fn a_stored_plan_is_evaluated_with_its_own_setting() {
    let mut doc = doc_en_mouvement(20);
    doc.panels[0].saccade = Saccade { actif: true, cadence: 3 };
    let p = doc.panels[0].clone();
    assert_eq!(doc.camera_du_plan_range(&p, 10), doux(&doc, 9));
    let mut q = p.clone();
    q.saccade.actif = false;
    assert_eq!(doc.camera_du_plan_range(&q, 10), doux(&doc, 10));
}
