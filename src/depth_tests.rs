//! Tests de la profondeur des calques : le défilement multiplan (phase 2c-2).

use eframe::egui::{vec2, Color32, Vec2};

use crate::camera::{Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::project;
use crate::render;

const RED: Color32 = Color32::from_rgb(220, 20, 20);

/// Feuille 64×48 avec un point rouge en (10, 10) et une caméra fixe décalée de (8, 0).
fn doc_with_camera(depth: f32) -> Document {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].layers = vec![Layer::new("B", LayerKind::Bitmap)];
    let b = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&b).unwrap().raster;
    let idx = r.tile_index_of(10, 10);
    r.tile_mut(idx)[10 * 64 + 10] = RED;
    doc.panels[0].layers[0].frames = vec![Some(b)];
    doc.panels[0].layers[0].depth = depth;
    let cam = Camera { x: 40.0, ..doc.default_camera() };
    doc.panels[0].camera = vec![CameraKey::new(0, cam)];
    doc
}

fn red_at(doc: &Document) -> Option<(usize, usize)> {
    let plan = render::panel_plan_at(doc, 0, 0, None);
    let full = render::composite_full(doc, &plan);
    let i = full.iter().position(|c| c.r() > 200 && c.a() > 200)?;
    Some((i % 64, i / 64))
}

#[test]
fn perspective_and_seen_transform() {
    let mut l = Layer::new("L", LayerKind::Bitmap);
    assert_eq!(l.perspective(), 1.0);
    assert_eq!(l.transform_seen(vec2(30.0, 0.0)), l.transform, "plan de référence : pas de décalage");
    l.depth = 1.0;
    assert_eq!(l.perspective(), 0.5);
    assert_eq!(l.transform_seen(Vec2::ZERO), l.transform, "caméra au centre : rien ne bouge");
    let t = l.transform_seen(vec2(30.0, -10.0));
    assert_eq!((t.x, t.y), (15.0, -5.0), "deux fois plus loin : suit la caméra à moitié");
    // Premier plan : défile plus vite que la caméra, dans l'autre sens.
    l.depth = -0.5;
    let t = l.transform_seen(vec2(10.0, 0.0));
    assert!((t.x + 10.0).abs() < 1e-4);
    // La profondeur ne touche ni l'échelle ni la rotation du calque.
    l.transform = LayerTransform { x: 3.0, scale: 2.0, angle: 45.0, ..Default::default() };
    let t = l.transform_seen(vec2(10.0, 0.0));
    assert_eq!((t.scale, t.angle), (2.0, 45.0));
    assert!((t.x - (3.0 - 10.0)).abs() < 1e-4);
    // Trop près : bornée pour ne pas diviser par zéro.
    l.depth = -5.0;
    assert!(l.perspective().is_finite());
}

#[test]
fn layers_scroll_with_the_camera_according_to_depth() {
    // Caméra décalée de +8 : le plan de référence ne bouge pas sur la feuille…
    assert_eq!(red_at(&doc_with_camera(0.0)), Some((10, 10)));
    // …un décor deux fois plus loin la suit à moitié (+4)…
    assert_eq!(red_at(&doc_with_camera(1.0)), Some((14, 10)));
    // …un premier plan deux fois plus proche part dans l'autre sens (−8).
    assert_eq!(red_at(&doc_with_camera(-0.5)), Some((2, 10)));
    // Caméra au centre : aucun décalage, quelle que soit la profondeur.
    let mut doc = doc_with_camera(3.0);
    doc.panels[0].camera.clear();
    assert_eq!(red_at(&doc), Some((10, 10)));
}

#[test]
fn depth_is_saved_and_blocks_merging() {
    let dir = std::env::temp_dir().join(format!("pelure-test-profondeur-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    let mut doc = doc_with_camera(2.5);
    doc.panels[0].layers.push(Layer::new("Devant", LayerKind::Bitmap));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].depth, 2.5);
    assert_eq!(back.panels[0].layers[1].depth, 0.0);
    let mut d = back;
    assert!(d.merge_layer_down(0, 1).is_err(), "pas de fusion entre deux profondeurs");
    d.panels[0].layers[0].depth = 0.0;
    assert!(d.merge_layer_down(0, 1).is_ok());
    std::fs::remove_dir_all(&dir).ok();
}
