//! Tests de la vue de dessus (phase 2c-3) : sa géométrie doit dire ce que filme la caméra.

use eframe::egui::vec2;

use crate::document::{Layer, LayerKind};
use crate::topview::{depth_of, distance, field_edges, half_field, world_x};

/// Caméra décalée de `offset`, sans avancée.
fn pan(offset: eframe::egui::Vec2) -> crate::document::CameraSeen {
    crate::document::CameraSeen { offset, dolly: 0.0, d0: 1000.0, grossissement: 1.0, angle: 0.0 }
}

#[test]
fn distance_and_depth_go_both_ways() {
    for z in [-0.9, -0.3, 0.0, 0.5, 1.0, 7.25, 1000.0] {
        assert!((depth_of(distance(z)) - z).abs() < 1e-4, "z = {z}");
    }
    assert_eq!(distance(0.0), 1.0, "plan de référence à 1");
    assert_eq!(depth_of(0.0), Layer::MIN_DEPTH, "pas plus près que la borne");
    assert_eq!(depth_of(5000.0), 1000.0);
}

#[test]
fn top_view_matches_the_multiplane_shift() {
    // Vu par la caméra (sténopé), un point du calque doit tomber là où le défilement multiplan le met.
    let c0 = 960.0;
    for z in [-0.5, 0.0, 1.0, 3.0] {
        let mut l = Layer::new("L", LayerKind::Bitmap);
        l.depth = z;
        let d = distance(z);
        for cam_x in [960.0, 1210.0, 500.0] {
            let seen = l.transform_seen(&pan(vec2(cam_x - c0, 0.0))).unwrap();
            for x in [100.0, 960.0, 1700.0] {
                // Multiplan : position du point dans le cadre, par rapport au centre de la caméra.
                let in_frame = x + seen.x - cam_x;
                // Vue de dessus : même point, projeté depuis la caméra.
                let projected = (world_x(x, c0, d) - (cam_x - c0)) / d;
                assert!((in_frame - projected).abs() < 1e-3, "z {z}, caméra {cam_x}, x {x}");
            }
        }
    }
}

#[test]
fn the_field_turns_with_the_pan() {
    let (unit, zoom) = (1920.0, 1.5);
    // Sans panoramique : les bords du champ de toujours.
    let [(a, da), (b, db)] = field_edges(100.0, 0.2, 3.0, zoom, 0.0, unit);
    let half = half_field(unit, zoom, 3.0 - 0.2);
    assert!((a - (100.0 - half)).abs() < 0.01 && (b - (100.0 + half)).abs() < 0.01);
    assert!((da - 3.0).abs() < 1e-4 && (db - 3.0).abs() < 1e-4);
    // Panoramique à droite : les deux bords partent à droite, le champ garde son ouverture.
    let [(a2, _), (b2, _)] = field_edges(100.0, 0.2, 3.0, zoom, 15.0, unit);
    assert!(a2 > a && b2 > b);
    // Très fort : le bord droit ne monte plus jusqu'en haut, il est arrêté sur sa direction.
    let [_, (_, d_far)] = field_edges(0.0, 0.0, 2.0, 1.0, 80.0, unit);
    assert!(d_far < 2.0);
}

#[test]
fn field_of_view_edges_are_the_frame_edges() {
    // Cadre de 1920 px, zoom 2 : la caméra voit 960 px du plan de référence, et D fois plus loin
    // à la distance D.
    let (fw, zoom) = (1920.0, 2.0);
    assert_eq!(half_field(fw, zoom, 1.0), 480.0);
    assert_eq!(half_field(fw, zoom, 3.0), 1440.0);
    // Un point du calque à z = 2 juste au bord du champ est au bord du cadre dans l'image.
    let (c0, cam_x, d) = (960.0, 1100.0, distance(2.0));
    let mut l = Layer::new("L", LayerKind::Bitmap);
    l.depth = 2.0;
    let edge_world = (cam_x - c0) + half_field(fw, zoom, d);
    let x = edge_world / d + c0;
    let seen = l.transform_seen(&pan(vec2(cam_x - c0, 0.0))).unwrap();
    assert!(((x + seen.x - cam_x) - fw / (2.0 * zoom)).abs() < 1e-3);
}
