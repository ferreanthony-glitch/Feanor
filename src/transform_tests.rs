//! Tests de la transformation d'un calque entier (phase 2c-1).

use eframe::egui::{pos2, Color32};

use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::project;
use crate::render;

const RED: Color32 = Color32::from_rgb(220, 20, 20);

fn one_pixel_doc(x: usize, y: usize) -> Document {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].layers = vec![Layer::new("B", LayerKind::Bitmap)];
    let b = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&b).unwrap().raster;
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % 64) * 64 + x % 64] = RED;
    doc.panels[0].layers[0].frames = vec![Some(b)];
    doc
}

#[test]
fn affine_and_pivoting() {
    let c = pos2(32.0, 24.0);
    assert_eq!(LayerTransform::default().affine(c).apply(pos2(3.0, 4.0)), pos2(3.0, 4.0));
    let moved = LayerTransform { x: 5.0, y: -2.0, ..Default::default() };
    assert_eq!(moved.affine(c).apply(pos2(3.0, 4.0)), pos2(8.0, 2.0));
    // Échelle 2 autour du centre de la feuille : le centre ne bouge pas.
    let big = LayerTransform { scale: 2.0, ..Default::default() };
    assert_eq!(big.affine(c).apply(c), c);
    assert_eq!(big.affine(c).apply(pos2(33.0, 24.0)), pos2(34.0, 24.0));
    // Tourner ou agrandir autour d'un autre point le laisse en place.
    let q = pos2(10.0, 10.0);
    let t = moved.around(c, q, 1.5, 30.0);
    let p_on_q = moved.affine(c).inverse().unwrap().apply(q);
    assert!(t.affine(c).apply(p_on_q).distance(q) < 1e-3);
    assert!((t.scale - 1.5).abs() < 1e-6 && (t.angle - 30.0).abs() < 1e-6);
}

#[test]
fn transformed_layers_are_composited_where_they_go() {
    let mut doc = one_pixel_doc(10, 10);
    doc.panels[0].layers[0].transform = LayerTransform { x: 5.0, y: 3.0, ..Default::default() };
    let plan = render::panel_plan(&doc.panels[0], 0, None);
    let full = render::composite_full(&doc, &plan);
    assert_eq!(full[13 * 64 + 15], RED, "le pixel suit le calque");
    assert_eq!(full[10 * 64 + 10], Color32::TRANSPARENT);
    assert_eq!(render::composite_pixel(&doc, &plan, 15, 13), RED);
    // Demi-tour autour du centre (32, 24) : (10.5, 10.5) va en (53.5, 37.5).
    doc.panels[0].layers[0].transform = LayerTransform { angle: 180.0, ..Default::default() };
    let plan = render::panel_plan(&doc.panels[0], 0, None);
    let full = render::composite_full(&doc, &plan);
    assert!(full[37 * 64 + 53].a() > 200);
}

#[test]
fn transforms_are_saved_and_block_merging() {
    let dir = std::env::temp_dir().join(format!("pelure-test-transfo-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    let mut doc = one_pixel_doc(1, 1);
    doc.panels[0].layers.push(Layer::new("V", LayerKind::Vector));
    let t = LayerTransform { x: 12.5, y: -3.0, scale: 0.75, angle: 20.0 };
    doc.panels[0].layers[1].transform = t;
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[1].transform, t);
    assert!(back.panels[0].layers[0].transform.is_identity());
    let mut d = back;
    assert!(d.merge_layer_down(0, 1).is_err(), "pas de fusion d'un calque transformé");
    std::fs::remove_dir_all(&dir).ok();
}
