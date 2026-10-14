//! Tests de la sélection bitmap : zones, pixels soulevés, transformations.

use eframe::egui::{pos2, vec2, Color32, Rect};

use crate::pixsel::{self, PixelMask};
use crate::raster::Raster;
use crate::vector::Affine;

const RED: Color32 = Color32::from_rgb(220, 30, 30);

/// Image 100×80 avec un carré rouge plein de (10, 10) à (30, 30).
fn square() -> Raster {
    let mut r = Raster::new(100, 80);
    for y in 10..30 {
        for x in 10..30 {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % 64) * 64 + x % 64] = RED;
        }
    }
    r
}

fn rect_mask(r: Rect) -> PixelMask {
    PixelMask::from_polygons(&[pixsel::rect_polygon(r)], 100, 80).unwrap()
}

#[test]
fn affine_compose_and_invert() {
    let a = Affine::rotate_about(pos2(5.0, 5.0), 0.7).then(&Affine::scale_about(pos2(1.0, 2.0), 2.0, 0.5));
    let p = pos2(13.0, -4.0);
    let back = a.inverse().unwrap().apply(a.apply(p));
    assert!(back.distance(p) < 1e-3);
    let t = Affine::translate(vec2(3.0, 0.0)).then(&Affine::translate(vec2(0.0, 2.0)));
    assert_eq!(t.apply(pos2(0.0, 0.0)), pos2(3.0, 2.0));
    assert!(Affine::scale_about(pos2(0.0, 0.0), 0.0, 1.0).inverse().is_none());
}

#[test]
fn masks_from_polygons() {
    let m = rect_mask(Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0)));
    assert_eq!(m.at(15, 15), 1.0);
    assert_eq!(m.at(5, 5), 0.0);
    assert_eq!(m.at(20, 15), 0.0, "le bord droit est exclu");
    let n = rect_mask(Rect::from_min_max(pos2(40.0, 40.0), pos2(50.0, 45.0)));
    let u = m.union(&n);
    assert_eq!((u.at(15, 15), u.at(45, 42), u.at(30, 30)), (1.0, 1.0, 0.0));
    // Un triangle : moitié couverte sur la diagonale.
    let tri = PixelMask::from_polygons(&[vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(0.0, 10.0)]], 100, 80).unwrap();
    assert!((tri.at(4, 5) - 0.5).abs() < 0.2);
    assert!(PixelMask::from_polygons(&[vec![pos2(0.0, 0.0), pos2(1.0, 1.0)]], 100, 80).is_none());
}

#[test]
fn lift_then_stamp_in_place_restores() {
    let original = square();
    let mut r = original.clone();
    let mask = rect_mask(Rect::from_min_max(pos2(5.0, 5.0), pos2(40.0, 40.0)));
    let f = pixsel::lift(&mut r, &mask);
    assert_eq!(r.get(15, 15), Color32::TRANSPARENT, "les pixels sont soulevés");
    assert_eq!(f.pixels[(15 - 5) * f.w + (15 - 5)], RED);
    pixsel::stamp(&mut r, &f, &Affine::IDENTITY).unwrap();
    assert!(r.to_flat() == original.to_flat(), "reposés tels quels");
}

#[test]
fn integer_move_is_exact() {
    let mut r = square();
    let mask = rect_mask(Rect::from_min_max(pos2(10.0, 10.0), pos2(30.0, 30.0)));
    let f = pixsel::lift(&mut r, &mask);
    let placed = pixsel::stamp(&mut r, &f, &Affine::translate(vec2(50.0, 20.0))).unwrap();
    assert!(placed.contains(pos2(65.0, 45.0)));
    assert_eq!(r.get(60, 30), RED, "coin haut-gauche déplacé, sans flou");
    assert_eq!(r.get(79, 49), RED);
    assert_eq!(r.get(80, 50), Color32::TRANSPARENT);
    assert_eq!(r.get(59, 30), Color32::TRANSPARENT, "pas de bavure à gauche");
    assert_eq!(r.get(15, 15), Color32::TRANSPARENT, "l'ancienne place est vide");
}

#[test]
fn flip_and_scale() {
    let mut r = Raster::new(100, 80);
    let idx = r.tile_index_of(10, 10);
    r.tile_mut(idx)[10 * 64 + 10] = RED;
    let mask = rect_mask(Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0)));
    let f = pixsel::lift(&mut r, &mask);
    // Miroir horizontal autour du centre de la zone (x = 15) : le pixel 10 va en 19.
    pixsel::stamp(&mut r, &f, &Affine::scale_about(pos2(15.0, 15.0), -1.0, 1.0)).unwrap();
    assert_eq!(r.get(19, 10), RED);
    assert_eq!(r.get(10, 10), Color32::TRANSPARENT);
    // Agrandi deux fois depuis le coin : un pixel devient 2×2 (au centre, plein).
    let mut big = Raster::new(100, 80);
    pixsel::stamp(&mut big, &f, &Affine::scale_about(pos2(10.0, 10.0), 2.0, 2.0)).unwrap();
    assert!(big.get(10, 10).a() > 100 && big.get(11, 11).a() > 100);
    assert_eq!(big.get(14, 14), Color32::TRANSPARENT);
}

#[test]
fn keep_only_inside() {
    let mut r = square();
    let mask = rect_mask(Rect::from_min_max(pos2(20.0, 20.0), pos2(30.0, 30.0)));
    pixsel::keep_only(&mut r, &mask);
    assert_eq!(r.get(25, 25), RED);
    assert_eq!(r.get(15, 15), Color32::TRANSPARENT);
    let copy = pixsel::copy(&square(), &mask);
    assert_eq!(copy.pixels.iter().filter(|&&c| c == RED).count(), 100, "les 10×10 pixels de la zone");
    assert!(copy.pixels.iter().all(|&c| c == RED || c == Color32::TRANSPARENT));
}
