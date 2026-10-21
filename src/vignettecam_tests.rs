//! Essais de la logique des mouvements de caméra des Vignettes (`vignettecam.rs`), sans interface.

use eframe::egui::{pos2, vec2};

use super::*;
use crate::camera::Camera;
use crate::document::Document;

fn doc_avec(keys: &[(usize, f32, f32, f32)]) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 25);
    for &(frame, x, y, zoom) in keys {
        let cam = Camera { x, y, zoom, ..Camera::centered(160, 90) };
        crate::camera::set_key(&mut doc.panels[0].camera, frame, cam);
    }
    doc
}

#[test]
fn a_plan_without_two_keys_has_no_movement_and_a_one_cell_box() {
    let doc = doc_avec(&[]);
    assert!(cadres(&doc, 0).is_empty());
    assert_eq!(span(&doc, 0, 4), [1, 1]);
    assert_eq!(span_du_mouvement(&doc, 0, 4), None);
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0)]);
    assert_eq!(span_du_mouvement(&doc, 0, 4), None, "une seule clé : la caméra est fixe");
}

#[test]
fn the_box_takes_the_whole_cells_that_hold_all_the_frames() {
    // Un panoramique d'une largeur de cadre : deux cases de large.
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]), 0, 6), [2, 1]);
    // Une demi-largeur : encore deux cases (la zone doit tout contenir).
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 160.0, 45.0, 1.0)]), 0, 6), [2, 1]);
    // Un vertical : deux rangées.
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 135.0, 1.0)]), 0, 6), [1, 2]);
    // Les deux.
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]), 0, 6), [2, 2]);
    // Trois cadres bout à bout.
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (12, 240.0, 45.0, 1.0), (24, 400.0, 45.0, 1.0)]), 0, 6), [3, 1]);
    // Pas de mouvement réel : tout tient dans A.
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]), 0, 6), [1, 1]);
    assert!(span_du_mouvement(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]), 0, 6).is_some());
    // Un zoom avant reste dans A ; un zoom arrière (cadre deux fois plus large) prend 2 × 2.
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 2.0)]), 0, 6), [1, 1]);
    assert_eq!(span(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.5)]), 0, 6), [2, 2]);
}

#[test]
fn the_box_is_capped_by_the_columns_and_the_height() {
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 1600.0, 1000.0, 1.0)]);
    assert_eq!(span(&doc, 0, 4), [4, SPAN_MAX_H]);
    assert_eq!(span(&doc, 0, 2), [2, SPAN_MAX_H]);
}

#[test]
fn the_zone_is_centred_on_the_frames_at_the_scale_of_frame_a() {
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    // Une case de 2 largeurs (240 de large la planche : A = 120 de large) et une hauteur : 68,75.
    let z = zone(&doc, 0, vec2(250.0, 67.5), 120.0).unwrap();
    assert!((z.echelle - 120.0 / 160.0).abs() < 1e-5);
    assert_eq!(z.centre, pos2(160.0, 45.0));
    assert!((z.taille.x - 250.0 / z.echelle).abs() < 1e-3);
    let case = eframe::egui::Rect::from_min_size(pos2(10.0, 20.0), vec2(250.0, 67.5));
    assert_eq!(z.vers_planche(case, z.centre), case.center());
    let p = pos2(100.0, 30.0);
    let r = z.vers_feuille(case, z.vers_planche(case, p));
    assert!((r - p).length() < 1e-3);
}

#[test]
fn keys_are_spread_evenly_over_the_plan() {
    assert_eq!(repartition(2, 25), vec![0, 24]);
    assert_eq!(repartition(3, 25), vec![0, 12, 24]);
    assert_eq!(repartition(4, 25), vec![0, 8, 16, 24]);
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0), (12, 160.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    assert!(est_repartie(&doc.panels[0].camera, 25));
    assert!(!est_repartie(&doc.panels[0].camera, 30));
}

#[test]
fn a_new_frame_makes_a_at_the_start_and_b_at_the_end_then_spreads_c() {
    let a = Camera::centered(160, 90);
    let b = a.shifted(160.0, 0.0);
    let keys = avec_nouveau_cadre(&[], a, b, 25).unwrap();
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 24]);
    assert_eq!((keys[0].cam, keys[1].cam), (a, b));
    assert!(!keys[1].smooth && keys[1].ease_in == 0.0, "vitesse constante, comme au montage");
    let c = b.shifted(160.0, 0.0);
    let keys = avec_nouveau_cadre(&keys, a, c, 25).unwrap();
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 12, 24]);
    assert_eq!((keys[0].cam, keys[1].cam, keys[2].cam), (a, b, c));
    let d = c.shifted(0.0, 90.0);
    let keys = avec_nouveau_cadre(&keys, a, d, 25).unwrap();
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 8, 16, 24]);
    assert_eq!(keys[3].cam, d);
}

#[test]
fn keys_placed_by_hand_are_respected() {
    let a = Camera::centered(160, 90);
    // A à 0, B à 5 (loin de la fin) : C va à la dernière image, rien d'autre ne bouge.
    let k = vec![CameraKey::new(0, a), CameraKey::new(5, a.shifted(50.0, 0.0))];
    let out = avec_nouveau_cadre(&k, a, a.shifted(100.0, 0.0), 25).unwrap();
    assert_eq!(out.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 5, 24]);
    // Des clés à la main dont la dernière est à la fin : elle recule à mi-chemin, la nouvelle prend la fin.
    let k = vec![CameraKey::new(0, a), CameraKey::new(6, a.shifted(50.0, 0.0)), CameraKey::new(24, a.shifted(80.0, 0.0))];
    let out = avec_nouveau_cadre(&k, a, a.shifted(100.0, 0.0), 25).unwrap();
    assert_eq!(out.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 6, 15, 24]);
    // Un plan d'une image ne porte pas de mouvement.
    assert!(avec_nouveau_cadre(&[], a, a, 1).is_none());
}

#[test]
fn the_grab_picks_corners_edges_and_the_inside_of_the_last_frame() {
    let carre = |x: f32, y: f32, w: f32, h: f32| [pos2(x, y), pos2(x + w, y), pos2(x + w, y + h), pos2(x, y + h)];
    // Case sans mouvement : le seul cadre est la vignette.
    let un = [carre(0.0, 0.0, 100.0, 60.0)];
    assert_eq!(choisit(&un, true, pos2(50.0, 30.0), 6.0).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&un, true, pos2(98.0, 2.0), 6.0), Some(Choix { prise: Prise::NouveauZoom, rang: 0, coin: 1 }));
    assert_eq!(choisit(&un, true, pos2(150.0, 30.0), 6.0), None);
    // Deux cadres : A à gauche, B décalé.
    let deux = [carre(0.0, 0.0, 100.0, 60.0), carre(60.0, 0.0, 100.0, 60.0)];
    assert_eq!(choisit(&deux, false, pos2(0.0, 30.0), 4.0).map(|c| c.prise), Some(Prise::Deplace(0)), "bord de A");
    assert_eq!(choisit(&deux, false, pos2(160.0, 60.0), 4.0).map(|c| c.prise), Some(Prise::Zoom(1)), "coin de B");
    assert_eq!(choisit(&deux, false, pos2(130.0, 30.0), 4.0).map(|c| c.prise), Some(Prise::Nouveau), "dedans B : C naît");
    assert_eq!(choisit(&deux, false, pos2(30.0, 30.0), 4.0), None, "dedans A seul : rien");
}

#[test]
fn the_geste_moves_and_zooms_from_the_keys_before_it_and_never_accumulates() {
    let frame = [160, 90];
    let a = Camera::centered(160, 90);
    let k0 = vec![CameraKey::new(0, a), CameraKey::new(24, a.shifted(160.0, 0.0))];
    let centre = centre_de(frame, &k0[1].cam);
    let coin = coin_de(frame, &k0[1].cam, 1);
    for _ in 0..3 {
        let out = applique(frame, &k0, a, Prise::Deplace(1), vec2(10.0, 5.0), coin, centre, 25).unwrap();
        assert_eq!((out[1].cam.x, out[1].cam.y), (250.0, 50.0));
        assert_eq!(out[0].cam, a);
    }
    // Tirer le coin vers l'extérieur élargit (zoom plus petit).
    let out = applique(frame, &k0, a, Prise::Zoom(1), (coin - centre).normalized() * 40.0, coin, centre, 25).unwrap();
    assert!(out[1].cam.zoom < 1.0, "{}", out[1].cam.zoom);
    // Un nouveau cadre depuis le dernier.
    let out = applique(frame, &k0, a, Prise::Nouveau, vec2(160.0, 0.0), coin, centre, 25).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out[2].cam.x, 400.0);
}

#[test]
fn step_names_follow_the_gesture() {
    assert_eq!(nom_etape(Prise::Nouveau, 0), "Mouvement de caméra");
    assert_eq!(nom_etape(Prise::NouveauZoom, 0), "Mouvement de caméra");
    assert_eq!(nom_etape(Prise::Nouveau, 2), "Cadre C");
    assert_eq!(nom_etape(Prise::Nouveau, 3), "Cadre D");
    assert_eq!(nom_etape(Prise::Deplace(1), 2), "Cadre B");
    assert_eq!(nom_etape(Prise::Zoom(1), 2), "Zoom du cadre B");
    assert_eq!(lettre(0), 'A');
}
