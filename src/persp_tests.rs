//! Tests de la déformation en perspective (5f).

use eframe::egui::{pos2, Pos2, Rect};

use crate::persp;
use crate::raster::{premultiply, Raster};
use crate::vector::{Element, VPoint, VectorDrawing};

fn close(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 1e-2
}

#[test]
fn corners_go_where_they_are_pulled() {
    let r = Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 70.0));
    let q = [pos2(30.0, 25.0), pos2(90.0, 25.0), pos2(130.0, 90.0), pos2(0.0, 90.0)];
    let h = persp::rect_to_quad(r, q).unwrap();
    for (c, t) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip(q) {
        assert!(close(persp::apply(&h, c), t), "{c:?} → {:?} (attendu {t:?})", persp::apply(&h, c));
    }
    // Inverse : on revient au point de départ.
    let inv = persp::inverse(&h).unwrap();
    let p = pos2(47.0, 33.0);
    assert!(close(persp::apply(&inv, persp::apply(&h, p)), p));
    // Une droite reste droite : le milieu d'un bord tombe sur le bord d'arrivée.
    let m = persp::apply(&h, pos2(60.0, 20.0));
    assert!((m.y - 25.0).abs() < 1e-3);
    // Parallélogramme : cas affine, le centre va au centre.
    let par = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(14.0, 10.0), pos2(4.0, 10.0)];
    let a = persp::rect_to_quad(Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), par).unwrap();
    assert!(close(persp::apply(&a, pos2(0.5, 0.5)), pos2(7.0, 5.0)));
}

#[test]
fn crossed_quads_are_refused() {
    let ok = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0), pos2(0.0, 10.0)];
    assert!(persp::is_convex(&ok));
    let crossed = [pos2(0.0, 0.0), pos2(10.0, 10.0), pos2(10.0, 0.0), pos2(0.0, 10.0)];
    assert!(!persp::is_convex(&crossed));
    let flat = [pos2(0.0, 0.0), pos2(5.0, 0.0), pos2(10.0, 0.0), pos2(0.0, 10.0)];
    assert!(!persp::is_convex(&flat));
}

fn square(x0: usize, y0: usize, x1: usize, y1: usize) -> Raster {
    let mut r = Raster::new(200, 200);
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % 64) * 64 + x % 64] = premultiply(0, 0, 200, 255);
        }
    }
    r
}

#[test]
fn an_image_is_put_in_perspective() {
    let r = square(40, 40, 120, 120);
    let b = persp::raster_bounds(&r).unwrap();
    assert_eq!(b, Rect::from_min_max(pos2(40.0, 40.0), pos2(120.0, 120.0)));
    // Coins en place : rien ne change.
    let same = persp::warp_raster(&r, b, [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()]).unwrap();
    for (x, y) in [(41, 41), (80, 80), (118, 118), (30, 30), (130, 80)] {
        assert_eq!(same.get(x, y), r.get(x, y), "({x}, {y})");
    }
    // Haut resserré (trapèze) : les coins du haut sont vides, le bas reste plein.
    let q = [pos2(70.0, 40.0), pos2(90.0, 40.0), pos2(120.0, 120.0), pos2(40.0, 120.0)];
    let t = persp::warp_raster(&r, b, q).unwrap();
    assert_eq!(t.get(45, 45).a(), 0, "coin du haut vidé");
    assert_eq!(t.get(80, 45).a(), 255, "haut au milieu");
    assert_eq!(t.get(45, 115).a(), 255, "bas plein");
    assert_eq!(persp::raster_bounds(&t).unwrap().min.y, 40.0);
}

#[test]
fn vector_points_follow() {
    let d = VectorDrawing {
        elements: vec![
            Element::Stroke {
                points: vec![VPoint { x: 0.0, y: 0.0, r: 2.0 }, VPoint { x: 100.0, y: 100.0, r: 2.0 }],
                color: [0, 0, 0],
                opacity: 1.0,
                hardness: 1.0,
                brush: None,
                trace: Default::default(),
            },
            Element::Fill { contours: vec![vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]]], color: [1, 1, 1], opacity: 1.0 },
        ],
    };
    let src = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
    let q = [pos2(10.0, 0.0), pos2(90.0, 0.0), pos2(100.0, 100.0), pos2(0.0, 100.0)];
    let all = persp::warp_drawing(&d, None, src, q).unwrap();
    let Element::Stroke { points, .. } = &all.elements[0] else { panic!() };
    assert!(close(points[0].pos(), pos2(10.0, 0.0)) && close(points[1].pos(), pos2(100.0, 100.0)));
    let Element::Fill { contours, .. } = &all.elements[1] else { panic!() };
    assert!(close(pos2(contours[0][1][0], contours[0][1][1]), pos2(90.0, 0.0)));
    // Seulement la sélection : le reste ne bouge pas.
    let some = persp::warp_drawing(&d, Some(&[1]), src, q).unwrap();
    assert_eq!(some.elements[0], d.elements[0]);
    assert_ne!(some.elements[1], d.elements[1]);
}
