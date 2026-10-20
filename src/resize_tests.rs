//! Taille de l'image (passe interface, Fichier > Réglages du projet) : le cadre seul, ou tout
//! mis à l'échelle.

use eframe::egui::Color32;

use crate::camera::{Camera, CameraKey};
use crate::document::{CelId, Document, LayerKind};
use crate::raster::TILE;

/// Un projet 64×32 (feuille 128×64) dont le dessin est un carré de 8 pixels au centre.
fn project() -> (Document, CelId) {
    let mut doc = Document::with_frame(64, 32, 24);
    let c = doc.new_cel(LayerKind::Bitmap);
    let cel = doc.cels.get_mut(&c).unwrap();
    for y in 28..36 {
        for x in 60..68 {
            let i = cel.raster.tile_index_of(x, y);
            cel.raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgb(200, 0, 0);
        }
    }
    doc.panels[0].layers[0].kind = LayerKind::Bitmap;
    doc.panels[0].layers[0].frames = vec![Some(c)];
    doc.panels[0].layers[0].transform.x = 10.0;
    doc.panels[0].camera.push(CameraKey::new(0, Camera::centered(128, 64)));
    (doc, c)
}

#[test]
fn frame_only_keeps_drawings_around_the_centre() {
    let (mut doc, c) = project();
    doc.resize_frame(128, 64, false);
    assert_eq!(doc.frame, [128, 64]);
    assert_eq!((doc.width, doc.height), (256, 128), "la feuille reste le double du cadre");
    // Le carré reste à 4 pixels à gauche et au-dessus du centre.
    let r = &doc.cels[&c].raster;
    assert_eq!(r.get(124, 60).a(), 255);
    assert_eq!(r.get(123, 60).a(), 0);
    assert_eq!(doc.panels[0].layers[0].transform.x, 10.0, "le placement des calques ne change pas");
    assert_eq!(doc.panels[0].camera[0].cam.x, 128.0, "la caméra suit le centre");
    // Un cadre plus petit : la feuille ne rapetisse pas.
    doc.resize_frame(32, 16, false);
    assert_eq!((doc.frame, doc.width), ([32, 16], 256));
}

#[test]
fn scaling_fills_the_new_frame() {
    let (mut doc, c) = project();
    doc.resize_frame(128, 64, true);
    assert_eq!((doc.frame, doc.width, doc.height), ([128, 64], 256, 128));
    let r = &doc.cels[&c].raster;
    // Le carré a doublé : de 120 à 136 (bords adoucis par le filtre).
    assert_eq!(r.get(128, 64).a(), 255);
    assert!(r.get(121, 64).a() > 100 && r.get(134, 64).a() > 100);
    assert_eq!(r.get(110, 64).a(), 0);
    assert_eq!(r.get(146, 64).a(), 0);
    assert_eq!(doc.panels[0].layers[0].transform.x, 20.0);
    assert_eq!(doc.panels[0].camera[0].cam.x, 128.0);
    // Autres proportions : le dessin remplit (le plus grand des deux rapports), sans déformation.
    let (mut doc, _) = project();
    doc.resize_frame(64, 64, true);
    assert_eq!(doc.frame, [64, 64]);
    assert_eq!((doc.width, doc.height), (256, 128), "rapport 2 (la hauteur) sur les deux axes");
}

#[test]
fn scaling_uses_the_rule_of_pasting_from_another_size_so_effects_and_the_infinite_canvas_follow() {
    // Changer la taille avec mise à l'échelle et coller depuis un projet d'une autre taille (`adaptation.rs`) ont une seule
    // règle : les paramètres d'effets en pixels et ce qui est dessiné hors de la feuille suivent aussi.
    let (mut doc, c) = project();
    let i = doc.cels[&c].raster.id_of(-1, -1);
    doc.cels.get_mut(&c).unwrap().raster.tile_mut(i)[63 * TILE + 63] = Color32::from_rgb(0, 200, 0);
    let mut flou = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
    flou.values[0] = 4.0;
    doc.panels[0].layers[0].fx = vec![flou];
    doc.resize_frame(128, 64, true);
    assert_eq!(doc.panels[0].layers[0].fx[0].values[0], 8.0, "le rayon du flou est en pixels");
    let r = &doc.cels[&c].raster;
    assert!(r.has_beyond(), "le dessin hors de la feuille est gardé");
    assert!((-3..=-1).any(|d| r.get_i(d, d).a() > 0), "(-1, -1) devient (-2, -2) : le pixel hors feuille a suivi");
}
