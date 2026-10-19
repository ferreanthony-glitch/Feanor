//! Dessins d'une autre taille que la feuille : l'aperçu réduit d'un effet qui calcule encore
//! (`fxproxy.rs`), une feuille flottante élargie, une image de vidéo, un dessin d'un projet abîmé.
//! SHOT ne se lançait plus le 2026-10-18 parce qu'une vignette lisait un tel dessin comme une feuille
//! entière (`raster.rs:57`, index hors des tuiles). Lire dehors donne maintenant du transparent, et
//! chaque composition lit à l'échelle.

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::raster::Raster;
use crate::render;

#[test]
fn reading_outside_a_drawing_is_transparent() {
    let r = Raster::new(100, 70);
    for (x, y) in [(100, 0), (0, 70), (99, 70), (1_000_000, 3), (3, 1_000_000), (usize::MAX, usize::MAX), (usize::MAX, 0)] {
        assert_eq!(r.get(x, y), Color32::TRANSPARENT, "({x}, {y})");
    }
    // Une tuile de bord déborde du dessin (100 n'est pas un multiple de 64) : rien n'en sort non plus.
    let mut r = Raster::new(100, 70);
    r.tile_mut(1).fill(Color32::RED);
    assert_eq!(r.get(99, 5), Color32::RED);
    assert_eq!(r.get(100, 5), Color32::TRANSPARENT);
    assert_eq!(r.get(120, 5), Color32::TRANSPARENT);
}

/// Un dessin plus petit (ou plus grand) que la feuille passe par toutes les compositions sans sortir
/// de ses tuiles, et s'y retrouve à l'échelle.
#[test]
fn drawings_of_another_size_compose_without_panic_in_every_path() {
    for (cw, ch) in [(150, 150), (300, 300), (100, 600), (700, 90), (1, 1)] {
        for transforme in [false, true] {
            let mut doc = Document::new(300, 300, 24);
            let cel = doc.new_cel(LayerKind::Bitmap);
            let c = doc.cels.get_mut(&cel).expect("dessin");
            c.raster = Raster::new(cw, ch);
            for idx in 0..c.raster.tiles.len() {
                c.raster.tile_mut(idx).fill(Color32::from_rgb(255, 0, 0));
            }
            let p = &mut doc.panels[0];
            p.layers[0] = Layer::new("Dessin", LayerKind::Bitmap);
            p.layers[0].set(0, Some(cel));
            if transforme {
                p.layers[0].transform.x = 20.0;
                p.layers[0].transform.angle = 10.0;
            }
            let plan = render::panel_plan_at(&doc, 0, 0, Some([255, 255, 255]));
            let tout = render::composite_full(&doc, &plan);
            assert_eq!(tout.len(), 300 * 300);
            let _ = render::composite_pixel(&doc, &plan, 299, 299);
            let _ = render::composite_pixel(&doc, &plan, 0, 0);
            let vignette = render::thumbnail(&doc, &plan, &doc.default_camera(), 16, 16);
            assert_eq!(vignette.pixels.len(), 256);
            let _ = render::camera_image(&doc, &plan, &doc.default_camera());
        }
    }
}

/// La moitié de la feuille, lue à l'échelle : le dessin remplit toute la feuille (et non son quart).
#[test]
fn a_half_size_drawing_is_stretched_over_the_whole_sheet() {
    let mut doc = Document::new(300, 300, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let c = doc.cels.get_mut(&cel).expect("dessin");
    c.raster = Raster::new(150, 150);
    for idx in 0..c.raster.tiles.len() {
        c.raster.tile_mut(idx).fill(Color32::from_rgb(255, 0, 0));
    }
    doc.panels[0].layers[0] = Layer::new("Dessin", LayerKind::Bitmap);
    doc.panels[0].layers[0].set(0, Some(cel));
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    let tout = render::composite_full(&doc, &plan);
    for (x, y) in [(0, 0), (299, 299), (150, 20), (20, 280)] {
        assert_eq!(tout[y * 300 + x], Color32::from_rgb(255, 0, 0), "({x}, {y})");
    }
    assert_eq!(render::composite_pixel(&doc, &plan, 299, 0), Color32::from_rgb(255, 0, 0));
}
