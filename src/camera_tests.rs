//! Tests de la caméra : images clés, cadre, image filmée, coupe et réunion de panneaux.

use eframe::egui::{pos2, Color32};

use crate::camera::{self, camera_at, Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind};
use crate::project;
use crate::render;

const RED: Color32 = Color32::from_rgb(220, 20, 20);

fn cam(x: f32, zoom: f32, angle: f32) -> Camera {
    Camera { x, y: 50.0, zoom, angle, dolly: 0.0 }
}

#[test]
fn keys_and_interpolation() {
    let default = Camera::centered(200, 100);
    assert_eq!(camera_at(&[], 7, default), default, "sans clé : au centre");
    let one = [CameraKey::new(3, cam(10.0, 1.0, 0.0))];
    assert_eq!(camera_at(&one, 0, default).x, 10.0, "une clé : caméra fixe");
    let keys = [
        CameraKey::new(0, cam(0.0, 1.0, 0.0)),
        CameraKey::new(10, cam(100.0, 4.0, 90.0)),
    ];
    let mid = camera_at(&keys, 5, default);
    assert!((mid.x - 50.0).abs() < 1e-4 && (mid.angle - 45.0).abs() < 1e-4);
    assert!((mid.zoom - 2.0).abs() < 1e-4, "zoom à vitesse régulière : 1 → 4 passe par 2");
    assert_eq!(camera_at(&keys, 30, default).x, 100.0, "après la dernière clé, la caméra reste");
    let mut ks = keys.to_vec();
    camera::set_key(&mut ks, 4, cam(7.0, 1.0, 0.0));
    camera::set_key(&mut ks, 10, cam(9.0, 1.0, 0.0));
    assert_eq!(ks.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 4, 10]);
    assert_eq!(ks[2].cam.x, 9.0, "une clé à la même image est remplacée");
}

#[test]
fn smooth_keys_slow_the_move_down() {
    let default = Camera::centered(200, 100);
    let mut keys = vec![CameraKey::new(0, cam(0.0, 1.0, 0.0)), CameraKey::new(8, cam(80.0, 1.0, 0.0))];
    let linear: Vec<f32> = (0..=8).map(|f| camera_at(&keys, f, default).x).collect();
    assert_eq!(linear[2], 20.0);
    keys[0].smooth = true;
    keys[1].smooth = true;
    let soft: Vec<f32> = (0..=8).map(|f| camera_at(&keys, f, default).x).collect();
    assert!(soft[1] < 10.0 && soft[7] > 70.0, "départ et arrivée ralentis : {soft:?}");
    assert!((soft[4] - 40.0).abs() < 1e-3, "même position à mi-chemin");
    assert_eq!((soft[0], soft[8]), (0.0, 80.0));
    // Adouci seulement à l'arrivée : départ franc.
    keys[0].smooth = false;
    let end_only: Vec<f32> = (0..=8).map(|f| camera_at(&keys, f, default).x).collect();
    assert!(end_only[1] > soft[1] && end_only[7] > 70.0);
}

#[test]
fn frame_corners_follow_zoom_and_rotation() {
    let c = Camera::centered(200, 120);
    let k = c.corners([100, 60]);
    assert_eq!(k, [pos2(50.0, 30.0), pos2(150.0, 30.0), pos2(150.0, 90.0), pos2(50.0, 90.0)]);
    let z = Camera { zoom: 2.0, ..c }.corners([100, 60]);
    assert_eq!((z[0], z[2]), (pos2(75.0, 45.0), pos2(125.0, 75.0)), "zoom 200 % : cadre deux fois plus petit");
    let r = Camera { angle: 90.0, ..c }.corners([100, 60]);
    assert!(r[0].distance(pos2(130.0, 10.0)) < 1e-3, "rotation de 90° : le coin haut-gauche passe en haut à droite");
}

#[test]
fn camera_image_through_zoom_and_rotation() {
    let mut doc = Document::with_frame(20, 10, 24);
    doc.panels[0].layers = vec![Layer::new("B", LayerKind::Bitmap)];
    let b = doc.new_cel(LayerKind::Bitmap);
    // Pixel rouge en haut à gauche du cadre (feuille 40×20, cadre de (10, 5) à (30, 15)).
    let r = &mut doc.cels.get_mut(&b).unwrap().raster;
    let idx = r.tile_index_of(10, 5);
    r.tile_mut(idx)[5 * 64 + 10] = RED;
    doc.panels[0].layers[0].frames = vec![Some(b)];
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    let straight = render::camera_image(&doc, &plan, &doc.default_camera());
    assert_eq!(straight[0], RED, "caméra centrée : pixel pour pixel");
    let turned = render::camera_image(&doc, &plan, &Camera { angle: 180.0, ..doc.default_camera() });
    assert_eq!(turned[20 * 10 - 1], RED, "demi-tour : le coin haut-gauche passe en bas à droite");
    let zoomed = render::camera_image(&doc, &plan, &Camera { zoom: 0.5, ..doc.default_camera() });
    // Zoom 50 % : le cadre couvre toute la feuille ; le pixel (10, 5) arrive en (5, 2.5).
    assert!(zoomed[2 * 20 + 5].a() > 0 || zoomed[3 * 20 + 5].a() > 0);
}

#[test]
fn splitting_and_merging_keep_the_camera_move() {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].length = 11;
    let a = Camera { x: 0.0, ..doc.default_camera() };
    let b = Camera { x: 100.0, ..doc.default_camera() };
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(10, b)];
    let before: Vec<f32> = (0..11).map(|f| doc.panel_camera(0, f).x).collect();
    doc.split_panel(0, 5).unwrap();
    let split: Vec<f32> = (0..5)
        .map(|f| doc.panel_camera(0, f).x)
        .chain((0..6).map(|f| doc.panel_camera(1, f).x))
        .collect();
    let close = |a: &[f32], b: &[f32]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-3);
    assert!(close(&split, &before), "chaque partie garde son bout du mouvement : {split:?}");
    doc.merge_with_next(0).unwrap();
    let merged: Vec<f32> = (0..11).map(|f| doc.panel_camera(0, f).x).collect();
    assert!(close(&merged, &before), "réunis : le mouvement d'origine : {merged:?}");
    // Un panneau sans caméra réuni à un panneau animé garde la caméra qu'il montrait (héritée du
    // panneau suivant : sa première clé).
    let mut doc = Document::new(64, 48, 24);
    doc.add_panel_after(0);
    let c = Camera { x: 7.0, ..doc.default_camera() };
    doc.panels[1].camera = vec![CameraKey::new(0, c), CameraKey::new(10, b)];
    let n0 = doc.panels[0].frame_count();
    assert_eq!(doc.panel_camera(0, 3), c, "avant : la caméra où le panneau d'après la prend");
    doc.merge_with_next(0).unwrap();
    assert_eq!(doc.panel_camera(0, 0), c);
    assert_eq!(doc.panel_camera(0, n0 - 1), c);
    assert_eq!(doc.panel_camera(0, n0).x, 7.0);
}

#[test]
fn a_panel_without_keys_keeps_the_camera_of_its_neighbour() {
    let mut doc = Document::new(400, 300, 24);
    for i in 0..2 {
        doc.add_panel_after(i);
    }
    let s2 = doc.add_scene_after(2);
    let a = Camera { x: 10.0, ..doc.default_camera() };
    let b = Camera { x: 90.0, zoom: 2.0, ..doc.default_camera() };
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(10, b)];
    // Le panneau 1 n'a pas de clé : il garde la caméra où le panneau 0 l'a laissée (pas le centre).
    assert_eq!(doc.panel_camera(1, 0), b);
    assert_eq!(doc.panel_camera(1, 30), b);
    // Le panneau 2 non plus : il prend la même, de proche en proche.
    assert_eq!(doc.panel_camera(2, 5), b);
    // Une autre scène ne hérite pas : caméra au centre.
    assert_eq!(doc.panel_camera(s2, 0), doc.default_camera());
    // Le premier panneau sans clé prend la caméra du suivant.
    doc.panels[0].camera.clear();
    doc.panels[1].camera = vec![CameraKey::new(0, a)];
    assert_eq!(doc.panel_camera(0, 12), a);
    // Les vignettes et l'export suivent (même caméra pour le plan et la vignette).
    let seen = doc.camera_seen(2, 0);
    assert_eq!(seen.offset.x, a.x - 200.0);
}

#[test]
fn ease_in_and_out_in_frames() {
    // Sans adouci : en ligne droite.
    assert!((camera::ease(0.25, 8.0, 0.0, 0.0) - 0.25).abs() < 1e-6);
    // Sortie sur 4 images d'un mouvement de 8 : départ lent, puis vitesse constante, arrivée franche.
    let curve: Vec<f32> = (0..=8).map(|f| camera::ease(f as f32 / 8.0, 8.0, 4.0, 0.0)).collect();
    assert_eq!((curve[0], curve[8]), (0.0, 1.0));
    assert!(curve.windows(2).all(|w| w[1] >= w[0]), "toujours en avant : {curve:?}");
    assert!(curve[1] < 1.0 / 8.0, "départ ralenti");
    let (late1, late2) = (curve[7] - curve[6], curve[8] - curve[7]);
    assert!((late1 - late2).abs() < 1e-4, "vitesse constante à la fin");
    // Entrée et sortie qui se chevauchent : partagées, la courbe reste continue.
    let both: Vec<f32> = (0..=10).map(|f| camera::ease(f as f32 / 10.0, 10.0, 8.0, 8.0)).collect();
    assert!((both[5] - 0.5).abs() < 1e-4 && both.windows(2).all(|w| w[1] >= w[0]));
    // Dans les clés : la sortie de la première, l'entrée de la seconde.
    let default = Camera::centered(200, 100);
    let mut keys = vec![CameraKey::new(0, cam(0.0, 1.0, 0.0)), CameraKey::new(10, cam(100.0, 1.0, 0.0))];
    keys[1].ease_in = 5.0;
    let x: Vec<f32> = (0..=10).map(|f| camera_at(&keys, f, default).x).collect();
    assert!(x[9] > 90.0 && x[10] - x[9] < x[2] - x[1], "arrivée ralentie : {x:?}");
}

#[test]
fn dolly_moves_the_camera_into_the_set() {
    let frame = [1000, 500];
    let d0 = camera::ref_distance(frame);
    let c = Camera::centered(2000, 1000);
    assert!((c.magnification(frame) - 1.0).abs() < 1e-6);
    // À mi-chemin du plan de référence : il paraît deux fois plus grand.
    let half = Camera { dolly: d0 / 2.0, ..c };
    assert!((half.magnification(frame) - 2.0).abs() < 1e-4);
    assert!((half.distance(frame) - d0 / 2.0).abs() < 1e-3);
    assert!((Camera { zoom: 2.0, ..c }.focal() - 70.0).abs() < 1e-4);
    // Un calque deux fois plus loin grossit moins : sur l'écran, 2 × 2/3 = 4/3 au lieu de 2.
    let mut far = Layer::new("Fond", LayerKind::Bitmap);
    far.depth = 1.0;
    let seen = crate::document::CameraSeen { offset: eframe::egui::Vec2::ZERO, dolly: d0 / 2.0, d0 };
    let t = far.transform_seen(&seen).unwrap();
    assert!((t.scale - 2.0 / 3.0).abs() < 1e-4, "{t:?}");
    // Le plan de référence ne bouge pas (c'est le cadre qui se resserre).
    let mut mid = Layer::new("Perso", LayerKind::Bitmap);
    mid.depth = 0.0;
    assert_eq!(mid.transform_seen(&seen), Some(mid.transform));
    // Un premier plan que la caméra a dépassé : on ne le voit plus.
    let mut near = Layer::new("Feuillage", LayerKind::Bitmap);
    near.depth = -0.6;
    assert!(near.transform_seen(&seen).is_none());
    // Sans avancée : le défilement multiplan d'avant.
    let pan = crate::document::CameraSeen { offset: eframe::egui::vec2(30.0, 0.0), dolly: 0.0, d0 };
    let t = far.transform_seen(&pan).unwrap();
    assert!((t.x - 15.0).abs() < 1e-4 && (t.scale - 1.0).abs() < 1e-6);
}

#[test]
fn cameras_are_saved_and_follow_the_stage() {
    let dir = std::env::temp_dir().join(format!("pelure-test-camera-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("C.pelure");
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].camera = vec![
        CameraKey::new(0, Camera { x: 10.0, y: 12.0, zoom: 1.5, angle: -12.5, dolly: 0.0 }),
        CameraKey::new(20, Camera { x: 40.0, y: 30.0, zoom: 0.8, angle: 3.0, dolly: 6.5 }),
    ];
    doc.panels[0].camera[1].smooth = true;
    doc.panels[0].camera[0].ease_out = 4.0;
    doc.panels[0].camera[1].ease_in = 2.5;
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].camera, doc.panels[0].camera);
    // Agrandir la feuille décale les caméras avec le dessin.
    let mut big = back;
    big.enlarge_stage(128, 96);
    assert_eq!((big.panels[0].camera[0].cam.x, big.panels[0].camera[0].cam.y), (42.0, 36.0));
    std::fs::remove_dir_all(&dir).ok();
}
