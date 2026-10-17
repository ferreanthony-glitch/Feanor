//! Enveloppe de Bézier : neutre au départ, un coin emmène ses poignées, une poignée courbe son côté,
//! les dessins (vectoriels et bitmap) suivent.

use eframe::egui::{pos2, vec2, Color32, Rect};

use crate::envelope::{warp_drawing, warp_raster, Envelope};
use crate::raster::{Raster, TILE};
use crate::vector::{Element, VPoint, VectorDrawing};

fn src() -> Rect {
    Rect::from_min_max(pos2(100.0, 100.0), pos2(200.0, 160.0))
}

#[test]
fn the_starting_envelope_changes_nothing() {
    let e = Envelope::identity(src());
    for (u, v) in [(0.0, 0.0), (1.0, 1.0), (0.3, 0.7), (0.5, 0.5)] {
        let p = e.point(u, v);
        let want = pos2(100.0 + 100.0 * u, 100.0 + 60.0 * v);
        assert!(p.distance(want) < 1e-3, "({u}, {v}) : {p:?}");
    }
}

#[test]
fn a_corner_drags_its_handles_and_a_handle_bends_its_side() {
    let mut e = Envelope::identity(src());
    let before = e;
    e.move_corner(2, vec2(30.0, 20.0));
    assert_eq!(e.corners[2], pos2(230.0, 180.0));
    assert_eq!(e.handles[4], before.handles[4] + vec2(30.0, 20.0), "poignée du côté bas, près du coin");
    assert_eq!(e.handles[3], before.handles[3] + vec2(30.0, 20.0), "poignée du côté droit, près du coin");
    assert_eq!(e.corners[0], before.corners[0], "le coin opposé ne bouge pas");
    assert!(e.point(1.0, 1.0).distance(pos2(230.0, 180.0)) < 1e-3);
    // Les deux poignées du haut montées de 30 : le milieu du côté haut monte de 22,5 (3/4 de 30).
    let mut b = Envelope::identity(src());
    b.handles[0].y -= 30.0;
    b.handles[1].y -= 30.0;
    assert!((b.point(0.5, 0.0).y - (100.0 - 22.5)).abs() < 1e-3);
    assert!(b.point(0.0, 0.0).distance(pos2(100.0, 100.0)) < 1e-3, "les coins restent");
    assert!(b.point(0.5, 1.0).distance(pos2(150.0, 160.0)) < 1e-3, "le côté bas ne bouge pas");
}

#[test]
fn a_vector_stroke_bends_with_the_envelope() {
    let stroke = Element::Stroke {
        points: vec![VPoint { x: 100.0, y: 100.0, r: 2.0 }, VPoint { x: 200.0, y: 100.0, r: 2.0 }],
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
    };
    let d = VectorDrawing { elements: vec![stroke] };
    // Neutre : les mêmes points (plus serrés).
    let same = warp_drawing(&d, None, src(), &Envelope::identity(src()));
    let Element::Stroke { points, .. } = &same.elements[0] else { panic!() };
    assert!(points.len() > 2, "rendu plus dense pour pouvoir se courber");
    assert!(points.iter().all(|p| (p.y - 100.0).abs() < 1e-3));
    // Côté haut courbé : le trait le long du haut se courbe, ses bouts restent.
    let mut e = Envelope::identity(src());
    e.handles[0].y -= 30.0;
    e.handles[1].y -= 30.0;
    let bent = warp_drawing(&d, None, src(), &e);
    let Element::Stroke { points, .. } = &bent.elements[0] else { panic!() };
    let (first, last) = (points[0], points[points.len() - 1]);
    assert!(first.pos().distance(pos2(100.0, 100.0)) < 1e-3 && last.pos().distance(pos2(200.0, 100.0)) < 1e-3);
    let top = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!((top - 77.5).abs() < 1.0, "le milieu monté : {top}");
}

#[test]
fn a_bitmap_follows_the_envelope() {
    let mut r = Raster::new(256, 256);
    // Un carré rouge de 10 px au coin haut-gauche du rectangle.
    for y in 100..110 {
        for x in 100..110 {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgb(255, 0, 0);
        }
    }
    let same = warp_raster(&r, src(), &Envelope::identity(src()));
    assert_eq!(same.get(105, 105), Color32::from_rgb(255, 0, 0), "neutre : rien ne bouge");
    // Toute l'enveloppe déplacée de 40 vers la droite : le carré suit, sa place est vidée.
    let mut e = Envelope::identity(src());
    e.translate(vec2(40.0, 0.0));
    let moved = warp_raster(&r, src(), &e);
    assert_eq!(moved.get(145, 105), Color32::from_rgb(255, 0, 0));
    assert_eq!(moved.get(105, 105).a(), 0, "l'ancienne place est vide");
}

#[test]
fn only_the_chosen_elements_follow_the_envelope() {
    let line = |y: f32| Element::Stroke {
        points: vec![VPoint { x: 100.0, y, r: 2.0 }, VPoint { x: 200.0, y, r: 2.0 }],
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
    };
    let d = VectorDrawing { elements: vec![line(100.0), line(150.0)] };
    let mut e = Envelope::identity(src());
    e.translate(vec2(0.0, 40.0));
    let out = warp_drawing(&d, Some(&[1]), src(), &e);
    assert_eq!(out.elements[0], d.elements[0], "l'élément non choisi ne bouge pas");
    let Element::Stroke { points, .. } = &out.elements[1] else { panic!() };
    assert!(points.iter().all(|p| (p.y - 190.0).abs() < 1e-3), "le choisi suit l'enveloppe");
}
#[test]
fn only_the_chosen_zone_of_a_bitmap_moves() {
    use crate::envelope::warp_raster_masked;
    use crate::pixsel::PixelMask;
    let red = Color32::from_rgb(255, 0, 0);
    let mut r = Raster::new(256, 256);
    // Deux carrés : en (100, 100) choisi, en (150, 100) pas.
    for (x0, y0) in [(100, 100), (150, 100)] {
        for y in y0..y0 + 10 {
            for x in x0..x0 + 10 {
                let i = r.tile_index_of(x, y);
                r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = red;
            }
        }
    }
    let zone = Rect::from_min_max(pos2(95.0, 95.0), pos2(115.0, 115.0));
    let poly = vec![zone.left_top(), zone.right_top(), zone.right_bottom(), zone.left_bottom()];
    let mask = PixelMask::from_polygons(&[poly], 256, 256).expect("zone");
    let mut e = Envelope::identity(mask.bounds());
    e.translate(vec2(0.0, 60.0));
    let out = warp_raster_masked(&r, &mask, &e);
    assert_eq!(out.get(105, 165), red, "le carré choisi est descendu");
    assert_eq!(out.get(105, 105).a(), 0, "sa place est vide");
    assert_eq!(out.get(155, 105), red, "l'autre n'a pas bougé");
}