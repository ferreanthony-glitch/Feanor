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

/// Une case de 120 de large (le cadre de 160 fait 0,75 point de la planche par point de la feuille), une rangée de six cases.
fn place() -> Place {
    Place { card_w: 120.0, thumb_h: 67.5, row_w: 6.0 * 120.0 + 5.0 * 30.0, total: 21.0, haut: 18.0 }
}

fn empreinte_de(keys: &[(usize, f32, f32, f32)]) -> Empreinte {
    empreinte(&doc_avec(keys), 0, &place()).expect("une empreinte")
}

#[test]
fn a_plan_without_two_keys_has_no_movement_and_no_footprint() {
    let doc = doc_avec(&[]);
    assert!(cadres(&doc, 0).is_empty());
    assert!(empreinte(&doc, 0, &place()).is_none());
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0)]);
    assert!(empreinte(&doc, 0, &place()).is_none(), "une seule clé : la caméra est fixe");
}

#[test]
fn a_travelling_widens_the_box_just_as_much_as_needed_at_the_scale_of_the_cases() {
    // Une largeur de cadre vers la droite : A puis B côte à côte, deux cases de large (sans écart : même feuille).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    assert_eq!((e.bande_min, e.bande_max), (0, 0));
    assert_eq!(e.tuiles.len(), 1);
    assert_eq!(e.cadres[0].rect, Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 67.5)), "A est la case, à son échelle");
    assert!((e.cadres[1].rect.left() - 120.0).abs() < 1e-3 && (e.cadres[1].rect.width() - 120.0).abs() < 1e-3, "{:?}", e.cadres[1].rect);
    assert!((e.x_min - 0.0).abs() < 1e-3 && (e.x_max - 240.0).abs() < 1e-3);
    // Une demi-largeur : une case et demie, pas deux (juste ce qu'il faut).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 160.0, 45.0, 1.0)]);
    assert!((e.x_max - 180.0).abs() < 1e-3, "{}", e.x_max);
    // Vers la gauche : la case s'élargit à gauche de A.
    let e = empreinte_de(&[(0, 240.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]);
    assert!((e.x_min + 120.0).abs() < 1e-3 && (e.x_max - 120.0).abs() < 1e-3, "{} {}", e.x_min, e.x_max);
    // Trois cadres bout à bout : trois cases.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (12, 240.0, 45.0, 1.0), (24, 400.0, 45.0, 1.0)]);
    assert!((e.x_max - 360.0).abs() < 1e-3);
    assert_eq!(e.cadres.len(), 3);
}

#[test]
fn a_zoom_out_surrounds_the_case_and_its_height_is_limited_to_the_row() {
    // Un cadre un peu plus large (zoom 0,9) : B entoure A, sans être réduit.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.9)]);
    let b = &e.cadres[1];
    assert!(!b.reduit && b.bande == 0);
    assert!((b.rect.height() - 67.5 / 0.9).abs() < 1e-2 && b.rect.contains_rect(e.cadres[0].rect), "{:?}", b.rect);
    // Un zoom arrière de moitié : B voudrait faire 135 de haut, la limite est la case et sa bande de note (67,5 + 21).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.5)]);
    let b = &e.cadres[1];
    assert!(b.reduit, "limite de taille");
    assert!((b.rect.height() - 88.5).abs() < 1e-3, "{}", b.rect.height());
    assert!((b.rect.width() / b.rect.height() - 16.0 / 9.0).abs() < 1e-3, "les proportions du cadre restent");
    assert!(b.rect.top() >= -place().haut - 1e-3 && b.rect.bottom() <= 67.5 + place().total + 1e-3);
    assert_eq!(e.tuiles.len(), 1, "une seule rangée");
    // La tuile couvre A et B ; la part de la feuille qu'elle montre est à l'échelle des cases.
    let z = e.tuiles[0].zone;
    assert!((z.echelle - 0.75).abs() < 1e-5);
    assert!((z.taille.x - e.tuiles[0].rect.width() / 0.75).abs() < 1e-3);
    assert!((z.centre - pos2(80.0, 45.0)).length() < 1e-3, "centrée sur A comme B : {:?}", z.centre);
    // Un zoom avant : B est dedans A, dessiné dedans.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 2.0)]);
    let b = &e.cadres[1];
    assert!(e.cadres[0].rect.contains_rect(b.rect) && !b.reduit);
    assert!((b.rect.width() - 60.0).abs() < 1e-3);
    assert_eq!(e.tuiles[0].rect, e.cadres[0].rect, "la case ne grandit pas");
}

#[test]
fn a_diagonal_and_a_tilt_put_b_on_the_next_row_and_an_upward_move_on_the_previous() {
    // Un vertical d'une hauteur de cadre : B sous A, dans la rangée suivante, en x comme A.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 135.0, 1.0)]);
    assert_eq!((e.bande_min, e.bande_max), (0, 1));
    assert_eq!(e.cadres[1].bande, 1);
    assert!((e.cadres[1].rect.left() - 0.0).abs() < 1e-3 && (e.cadres[1].rect.top() - 0.0).abs() < 1e-3, "aligné sur l'image de sa rangée : {:?}", e.cadres[1].rect);
    assert_eq!(e.tuiles.len(), 2);
    assert_eq!(e.tuiles.len(), 2);
    // La tuile de la rangée de B montre la feuille là où B regarde.
    assert_eq!(e.tuiles[1].zone.centre, pos2(80.0, 135.0));
    // Une diagonale : en bas et à droite.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]);
    assert_eq!(e.cadres[1].bande, 1);
    assert!((e.cadres[1].rect.left() - 120.0).abs() < 1e-3);
    assert!((e.x_max - 240.0).abs() < 1e-3);
    // Vers le haut : la rangée précédente.
    let e = empreinte_de(&[(0, 80.0, 135.0, 1.0), (24, 80.0, 45.0, 1.0)]);
    assert_eq!((e.bande_min, e.bande_max), (-1, 0));
    // Un petit décalage vertical reste dans la rangée (la bande de note le permet).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 55.0, 1.0)]);
    assert_eq!(e.cadres[1].bande, 0);
    assert!(e.cadres[1].rect.bottom() > 67.5 && e.cadres[1].rect.bottom() <= 67.5 + place().total + 1e-3);
}

#[test]
fn the_footprint_is_never_wider_than_a_row() {
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 5000.0, 45.0, 1.0)]);
    assert!(e.x_max - e.x_min <= place().row_w + 1e-3, "{} {}", e.x_min, e.x_max);
    let etroit = Place { row_w: 120.0, ..place() };
    let e = empreinte(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 400.0, 45.0, 0.5)]), 0, &etroit).unwrap();
    assert!(e.x_max - e.x_min <= 120.0 + 1e-3, "une colonne : pas de travelling");
    assert!(e.cadres.iter().all(|c| c.rect.width() <= 120.0 + 1e-3));
}

#[test]
fn the_links_follow_the_kind_of_move() {
    let l = |keys: &[(usize, f32, f32, f32)]| {
        let e = empreinte_de(keys);
        // (Chaque rangée de plus est un pas plus bas sur la planche.)
        liens(&e.cadres.iter().map(|c| (c.rect.translate(vec2(0.0, c.bande as f32 * 95.0)), c.bande)).collect::<Vec<_>>(), 120.0)
    };
    // Un travelling : une flèche au-dessus, de A vers B.
    let v = l(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    assert!(matches!(v[..], [Lien::FlecheDessus { de, vers }] if de.x < vers.x && de.y < 0.0));
    // Un zoom : les coins de A reliés à ceux de B.
    let v = l(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.9)]);
    assert!(matches!(v[..], [Lien::Coins(_)]));
    // Un panoramique vertical : une flèche vers le bas.
    let v = l(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 135.0, 1.0)]);
    assert!(matches!(v[..], [Lien::FlecheVerticale { de, vers }] if de.y < vers.y));
    // Une diagonale : une boîte en perspective.
    let v = l(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]);
    assert!(matches!(v[..], [Lien::Coins(_)]));
    // Deux cadres qui se confondent : rien à relier.
    assert!(l(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]).is_empty());
}

#[test]
fn the_zone_of_a_tile_maps_the_sheet_onto_the_board_at_the_scale_of_the_cases() {
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    let z = e.tuiles[0].zone;
    // La tuile, posée sur la planche à (10, 20), montre la feuille : le point central du cadre A est au centre de A.
    let tuile = Rect::from_min_size(pos2(10.0, 20.0), e.tuiles[0].rect.size());
    let vue = z.vue(tuile, vec2(160.0, 90.0));
    assert!((vue.zoom - 0.75).abs() < 1e-5, "{}", vue.zoom);
    assert_eq!(z.centre, pos2(160.0, 45.0));
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
fn removing_a_frame_respreads_the_keys_and_removing_b_of_two_removes_the_move() {
    let a = Camera::centered(160, 90);
    let deux = vec![CameraKey::new(0, a), CameraKey::new(24, a.shifted(160.0, 0.0))];
    let out = sans_le_cadre(&deux, 1, 25).unwrap();
    assert_eq!(out.len(), 1, "il ne reste que A : plus de mouvement");
    assert_eq!(out[0].cam, a);
    let trois = vec![CameraKey::new(0, a), CameraKey::new(12, a.shifted(160.0, 0.0)), CameraKey::new(24, a.shifted(320.0, 0.0))];
    let out = sans_le_cadre(&trois, 1, 25).unwrap();
    assert_eq!(out.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 24], "les clés restantes se répartissent");
    assert_eq!(out[1].cam, trois[2].cam);
    let out = sans_le_cadre(&trois, 0, 25).unwrap();
    assert_eq!(out[0].cam, trois[1].cam, "retirer A : B devient le début");
    // Des clés posées à la main restent où elles sont.
    let main = vec![CameraKey::new(0, a), CameraKey::new(5, a.shifted(50.0, 0.0)), CameraKey::new(24, a.shifted(80.0, 0.0))];
    assert_eq!(sans_le_cadre(&main, 1, 25).unwrap().iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 24]);
    assert!(sans_le_cadre(&main, 3, 25).is_none());
}

#[test]
fn the_added_frame_sits_half_a_frame_beside_the_last() {
    let a = Camera::centered(160, 90);
    let t = Vec2::new(160.0, 90.0);
    let c = cadre_suivant([160, 90], a.shifted(0.0, 0.0), t);
    assert!(c.x > a.x && c.y == a.y && c.zoom == a.zoom, "{c:?}");
}

fn carre(x: f32, y: f32, w: f32, h: f32) -> [Pos2; 4] {
    [pos2(x, y), pos2(x + w, y), pos2(x + w, y + h), pos2(x, y + h)]
}

#[test]
fn the_grab_on_a_plain_case_makes_b_moved_or_zoomed_from_a_corner_wherever_it_starts() {
    let un = [carre(0.0, 0.0, 100.0, 60.0)];
    assert_eq!(choisit(&un, true, pos2(50.0, 30.0), 6.0, false).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&un, true, pos2(98.0, 2.0), 6.0, false), Some(Choix { prise: Prise::NouveauZoom, rang: 0, coin: 1 }));
    // (Même hors de la case : sa marge, son empreinte — un appui de caméra prend toujours quelque chose.)
    assert_eq!(choisit(&un, true, pos2(150.0, 30.0), 6.0, false).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&[], true, pos2(0.0, 0.0), 6.0, false), None);
}

#[test]
fn a_grab_inside_an_existing_frame_moves_that_frame_and_the_smallest_one_wins() {
    // B décalé à droite, recouvrant la moitié de A.
    let deux = [carre(0.0, 0.0, 100.0, 60.0), carre(60.0, 0.0, 100.0, 60.0)];
    assert_eq!(choisit(&deux, false, pos2(130.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(1)), "dedans B : on déplace B, rien ne naît");
    assert_eq!(choisit(&deux, false, pos2(30.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(0)), "dedans A, hors de B : on déplace A");
    // B dans A : dedans B on déplace B, dans A hors de B on déplace A.
    let dedans = [carre(0.0, 0.0, 100.0, 60.0), carre(30.0, 15.0, 40.0, 30.0)];
    assert_eq!(choisit(&dedans, false, pos2(50.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(1)));
    assert_eq!(choisit(&dedans, false, pos2(10.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(0)));
    // Un coin de n'importe quel cadre change son zoom.
    assert_eq!(choisit(&deux, false, pos2(160.0, 60.0), 4.0, false), Some(Choix { prise: Prise::Zoom(1), rang: 1, coin: 2 }), "coin de B");
    assert_eq!(choisit(&deux, false, pos2(0.0, 0.0), 4.0, false), Some(Choix { prise: Prise::Zoom(0), rang: 0, coin: 0 }), "coin de A");
    // Hors de tous les cadres : le plus proche, jamais rien.
    assert_eq!(choisit(&deux, false, pos2(200.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(1)));
    assert_eq!(choisit(&deux, false, pos2(-30.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(0)));
}

#[test]
fn alt_asks_for_the_next_frame_from_the_last_one() {
    let deux = [carre(0.0, 0.0, 100.0, 60.0), carre(60.0, 0.0, 100.0, 60.0)];
    // Où que l'appui tombe, avec Alt : un nouveau cadre déplacé depuis le dernier, ou zoomé à son coin.
    assert_eq!(choisit(&deux, false, pos2(30.0, 30.0), 4.0, true), Some(Choix { prise: Prise::Nouveau, rang: 1, coin: 0 }));
    assert_eq!(choisit(&deux, false, pos2(130.0, 30.0), 4.0, true).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&deux, false, pos2(160.0, 60.0), 4.0, true), Some(Choix { prise: Prise::NouveauZoom, rang: 1, coin: 2 }));
}

#[test]
fn the_frame_under_a_right_click_is_the_smallest_that_contains_the_point() {
    let dedans = [carre(0.0, 0.0, 100.0, 60.0), carre(30.0, 15.0, 40.0, 30.0)];
    assert_eq!(cadre_sous(&dedans, pos2(50.0, 30.0)), Some(1));
    assert_eq!(cadre_sous(&dedans, pos2(10.0, 30.0)), Some(0));
    assert_eq!(cadre_sous(&dedans, pos2(150.0, 30.0)), None);
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
    // Déplacer A (le cadre du début) : B ne bouge pas.
    let out = applique(frame, &k0, a, Prise::Deplace(0), vec2(10.0, 5.0), coin, centre, 25).unwrap();
    assert_eq!((out[0].cam.x, out[0].cam.y), (90.0, 50.0));
    assert_eq!(out[1].cam, k0[1].cam);
    assert_eq!(out.len(), 2, "rien ne naît");
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
