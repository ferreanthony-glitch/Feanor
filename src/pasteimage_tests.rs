//! Image collée du presse-papiers de Windows : taille ajustée au cadre, pixels convertis.

use eframe::egui::Color32;

use crate::pasteimage::{est_copie_interne, fit_size, to_floating, MARK};

#[test]
fn a_big_image_shrinks_to_the_frame_without_distortion() {
    assert_eq!(fit_size(800, 600, 1920, 1080), (800, 600), "jamais agrandie");
    assert_eq!(fit_size(3840, 2160, 1920, 1080), (1920, 1080));
    assert_eq!(fit_size(1000, 4000, 1920, 1080), (270, 1080), "la hauteur décide");
    assert_eq!(fit_size(0, 10, 100, 100), (0, 10));
}

#[test]
fn pixels_are_premultiplied_and_placed() {
    // Deux pixels : rouge opaque, blanc à moitié transparent.
    let rgba = [255, 0, 0, 255, 255, 255, 255, 128];
    let f = to_floating(&rgba, 2, 1, 10, 20);
    assert_eq!((f.x0, f.y0, f.w, f.h), (10, 20, 2, 1));
    assert_eq!(f.pixels[0], Color32::from_rgb(255, 0, 0));
    assert_eq!(f.pixels[1], Color32::from_rgba_premultiplied(128, 128, 128, 128), "prémultiplié comme les images importées");
}

#[test]
fn an_image_without_any_alpha_is_opaque() {
    // Certains logiciels écrivent des pixels sans opacité : l'image serait invisible.
    let rgba = [10, 20, 30, 0, 40, 50, 60, 0];
    let f = to_floating(&rgba, 2, 1, 0, 0);
    assert_eq!(f.pixels, vec![Color32::from_rgb(10, 20, 30), Color32::from_rgb(40, 50, 60)]);
}

#[test]
fn shot_marks_its_own_copies() {
    // Les copies de FEANOR (zone bitmap, éléments vectoriels) commencent par la marque : leur
    // collage n'est pas pris pour une image venue d'ailleurs.
    assert!("FEANOR : zone bitmap 10×10".starts_with(MARK));
    assert!(est_copie_interne("FEANOR : 3 élément(s) vectoriel(s)"));
    // Une copie faite par une ancienne version (Pelure, SHOT) est reconnue aussi.
    assert!(est_copie_interne("Pelure : zone bitmap 10×10"));
    assert!(est_copie_interne("SHOT : images copiées"));
    assert!(!est_copie_interne("un texte copié ailleurs"));
}
