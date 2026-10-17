//! Retourner : un calque sur place, un plan autour du milieu du cadre, sa caméra avec lui.

use eframe::egui::pos2;

use crate::camera::Camera;
use crate::document::LayerTransform;
use crate::flip::{flipped_in_place, mirror_camera_x, mirror_transform_x};

#[test]
fn a_plan_flip_mirrors_every_point_of_a_transformed_layer() {
    let (pivot, axis) = (pos2(1920.0, 1080.0), 1920.0);
    let t = LayerTransform { x: 40.0, y: -10.0, scale: 1.3, angle: 25.0, stretch_x: 1.5, stretch_y: 0.8 };
    let m = mirror_transform_x(t, pivot, axis);
    for p in [pos2(1000.0, 700.0), pos2(2500.0, 1300.0), pos2(1920.0, 1080.0)] {
        let (a, b) = (t.affine(pivot).apply(p), m.affine(pivot).apply(p));
        assert!((b.x - (2.0 * axis - a.x)).abs() < 1e-2 && (b.y - a.y).abs() < 1e-2, "{p:?} : {a:?} → {b:?}");
    }
    // Deux fois : retour à l'original.
    let back = mirror_transform_x(m, pivot, axis);
    assert!((back.x - t.x).abs() < 1e-3 && (back.angle - t.angle).abs() < 1e-4 && back.stretch_x == t.stretch_x);
}

#[test]
fn a_layer_flips_in_place() {
    let pivot = pos2(100.0, 50.0);
    let center = pos2(130.0, 60.0);
    let t = LayerTransform { angle: 10.0, ..Default::default() };
    let f = flipped_in_place(t, pivot, center, true);
    assert_eq!(f.stretch_x, -1.0);
    assert!(f.affine(pivot).apply(center).distance(t.affine(pivot).apply(center)) < 1e-3, "le milieu du contenu reste en place");
    let v = flipped_in_place(t, pivot, center, false);
    assert_eq!((v.stretch_x, v.stretch_y), (1.0, -1.0));
    // Un étirement négatif reste négatif quand on le borne.
    assert_eq!(LayerTransform::clamp_stretch(-500.0), -100.0);
    assert_eq!(LayerTransform::clamp_stretch(0.0), 0.01);
}

#[test]
fn the_camera_follows_the_flipped_plan() {
    let c = Camera { x: 2000.0, y: 900.0, zoom: 1.5, angle: 12.0, dolly: 30.0, tilt: 5.0, pan: -8.0 };
    let m = mirror_camera_x(c, 1920.0);
    assert_eq!((m.x, m.y, m.zoom, m.angle, m.dolly, m.tilt, m.pan), (1840.0, 900.0, 1.5, -12.0, 30.0, 5.0, 8.0));
}
