//! Retour de terrain d'Anthony (2026-10-24, branche `retours-terrain`) : l'aplat qui perd des morceaux (le remplissage d'une clé neuve, le
//! remplissage d'avant qui disparaît pendant le recalcul).

use std::sync::Arc;

use eframe::egui::{pos2, Color32};

use crate::app::Tool;
use crate::fx;
use crate::fxcache::monde;
use crate::griffonnage_tests::{app_griffonne, arc_of, doc_fill, is_red, put, trace, wait_new};

// ------------------------------------------------------------------------------------ 3. l'aplat qui perd des morceaux

#[test]
fn a_fill_stays_on_screen_while_it_is_recomputed_even_after_many_other_fills_were_shown() {
    monde::ecrire();
    let (mut doc, lines, _g, item) = doc_fill(8_600_000);
    let first = fx::differe(|| arc_of(&doc, &item));
    let ready = wait_new(&doc, &item, &first);
    assert_eq!(ready.raster.get(20, 30), Color32::RED, "le remplissage de la zone");
    // Vingt autres remplissages passent à l'écran (les vignettes d'un film en montrent bien plus).
    for k in 0..20u64 {
        let (autre, _, _, it) = doc_fill(8_610_000 + 10 * k);
        let vide = fx::differe(|| arc_of(&autre, &it));
        wait_new(&autre, &it, &vide);
    }
    // Le trait change : l'écran garde le remplissage d'avant, il ne se vide pas le temps du calcul.
    for y in 11..40 {
        put(&mut doc.cels.get_mut(&lines).expect("trait").raster, 22, y, Color32::BLACK);
    }
    doc.cels.get_mut(&lines).expect("trait").version += 1;
    let montre = fx::differe(|| arc_of(&doc, &item));
    assert!(Arc::ptr_eq(&montre, &ready), "le remplissage d'avant reste affiché (il a été vidé : {})", montre.raster.is_blank());
    let nouveau = wait_new(&doc, &item, &ready);
    assert_eq!(nouveau.raster.get(30, 30), Color32::RED);
}

#[test]
fn a_new_key_made_by_copy_shows_the_fill_of_its_source_until_its_own_is_computed() {
    monde::ecrire();
    let (mut app, _a) = app_griffonne("aplat-cle-neuve", 0.0, 8_700_000);
    trace(&mut app, pos2(180.0, 150.0), pos2(190.0, 150.0));
    let ligne = app.layers().iter().position(|l| l.name == "Trait").expect("calque de trait");
    // L'image 0 montre son remplissage.
    let plan0 = app.build_plan(0, false, false);
    let vide = fx::differe(|| arc_of(&app.doc, &plan0.items[0]));
    wait_new(&app.doc, &plan0.items[0], &vide);
    assert!(is_red(fx::differe(|| crate::render::composite_pixel(&app.doc, &plan0, 170, 180))), "l'image 0 montre sa couleur");
    // Une nouvelle clé, copie du dessin, à l'image 6 : son remplissage n'est pas calculé, l'écran montre celui de sa source.
    app.montage_new_key(0, ligne, 6, true);
    let plan6 = app.build_plan(6, false, false);
    let pixel = fx::differe(|| crate::render::composite_pixel(&app.doc, &plan6, 170, 180));
    assert!(is_red(pixel), "la couleur ne disparaît pas à la création de la clé : {:?}", pixel.to_array());
    // Retouchée tout de suite (une cloison), la nouvelle clé garde un remplissage à l'écran en attendant le calcul.
    app.select_layer(ligne);
    app.cur_frame = 6;
    app.tool = Tool::Brush;
    app.color = [0, 0, 0];
    trace(&mut app, pos2(240.0, 90.0), pos2(240.0, 210.0));
    let plan6 = app.build_plan(6, false, false);
    let pixel = fx::differe(|| crate::render::composite_pixel(&app.doc, &plan6, 170, 180));
    assert!(is_red(pixel), "la couleur reste pendant le recalcul : {:?}", pixel.to_array());
}

#[test]
fn the_blank_fill_shown_before_the_first_computation_is_kept_for_each_size() {
    monde::ecrire();
    let a = fx::cel_vide(64, 64);
    let _autre_taille = fx::cel_vide(400, 300);
    let b = fx::cel_vide(64, 64);
    assert!(Arc::ptr_eq(&a, &b), "un seul dessin vide par taille : deux projets ouverts ne se les volent pas");
}
