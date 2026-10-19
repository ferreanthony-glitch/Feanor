//! Tests de la découpe en zones fermées (`zones.rs`).

use crate::coloriage_tests::{art, closed, ellipse, rect, Shape, BLUE, H, RED, W};
use crate::raster::Raster;
use crate::zones::{segment, Reglages};

const R: Reglages = Reglages { trous: 6, debord: 3 };

fn inner_count(shapes: &[Shape], r: &Reglages) -> usize {
    segment(&art(W, H, 3.0, shapes), r).inner().count()
}

#[test]
fn a_closed_shape_is_a_zone_and_the_outside_is_marked() {
    let seg = segment(&art(W, H, 3.0, &[closed(rect(200.0, 150.0, 120.0, 80.0, 0.0), RED)]), &R);
    assert_eq!(seg.zones.len(), 2, "le dehors et l'intérieur");
    let inner = seg.zone_at(200, 150).expect("l'intérieur");
    let outer = seg.zones.iter().find(|z| z.outside).expect("le dehors");
    assert!(!inner.outside);
    // L'intérieur : 120 × 80 moins le trait, plus le débord sous le trait.
    assert!((8_500..=10_500).contains(&inner.area), "{}", inner.area);
    // Son centre et sa boîte.
    assert!((inner.centroid.0 - 200.0).abs() < 1.0 && (inner.centroid.1 - 150.0).abs() < 1.0, "{:?}", inner.centroid);
    assert!(inner.bbox[0] <= 142 && inner.bbox[0] >= 136 && inner.bbox[2] >= 258 && inner.bbox[2] <= 264, "{:?}", inner.bbox);
    // Le dehors est voisin de l'intérieur, et inversement.
    assert!(inner.neighbours.iter().any(|&(n, _)| n == outer.id));
    assert!(outer.neighbours.iter().any(|&(n, _)| n == inner.id));
    assert!(inner.perimeter > 380);
    // Le trait lui-même fait partie de l'intérieur sur `debord` pixels, mais pas du dehors.
    assert_eq!(seg.label_at(140, 150), inner.id, "sous le trait, côté intérieur");
    assert_eq!(seg.label_at(135, 150), outer.id);
    assert_eq!(seg.label_at(137, 150), outer.id, "hors du trait : le dehors, jusqu'au trait");
}

#[test]
fn an_empty_drawing_has_no_zone() {
    let seg = segment(&Raster::new(W, H), &R);
    assert!(seg.zones.is_empty());
    assert_eq!(seg.label_at(10, 10), 0);
}

#[test]
fn a_zone_smaller_than_the_gap_closing_survives() {
    // Un carré de 16 pixels de côté : plus petit que la fermeture (2 × 6) autour de lui.
    let shapes = [closed(rect(100.0, 100.0, 16.0, 16.0, 0.0), RED), closed(rect(300.0, 150.0, 100.0, 100.0, 0.0), BLUE)];
    let seg = segment(&art(W, H, 3.0, &shapes), &R);
    let tiny = seg.zone_at(100, 100).expect("la petite zone");
    assert!(!tiny.outside && tiny.area >= 100, "{}", tiny.area);
    assert_eq!(seg.inner().count(), 2);
    assert_ne!(seg.label_at(100, 100), seg.label_at(300, 150));
}

#[test]
fn a_gap_is_closed_up_to_twice_the_closing_distance() {
    let mut ring = closed(ellipse(200.0, 150.0, 50.0, 50.0, 0.0), RED);
    // 48 côtés pour 314 pixels : 6,5 pixels chacun ; en retirer un laisse un trou de 3,5 pixels.
    ring.skip = vec![5];
    assert_eq!(inner_count(&[ring.clone()], &Reglages { trous: 6, debord: 3 }), 1, "fermé");
    assert_eq!(inner_count(&[ring.clone()], &Reglages { trous: 3, debord: 3 }), 1, "trou de 3,5 : fermé à 2 × 3");
    assert_eq!(inner_count(&[ring], &Reglages { trous: 1, debord: 3 }), 0, "trop large pour 2 × 1 : l'intérieur fuit");
    // Un trou plus large (deux côtés : 10 pixels) ne se ferme qu'avec une fermeture plus forte.
    let mut wide = closed(ellipse(200.0, 150.0, 50.0, 50.0, 0.0), RED);
    wide.skip = vec![5, 6];
    assert_eq!(inner_count(&[wide.clone()], &Reglages { trous: 6, debord: 3 }), 1);
    assert_eq!(inner_count(&[wide], &Reglages { trous: 2, debord: 3 }), 0);
}

#[test]
fn neighbouring_zones_know_each_other() {
    // Deux rectangles qui partagent un côté : un seul trait entre eux.
    let shapes = [closed(rect(150.0, 150.0, 100.0, 100.0, 0.0), RED), closed(rect(250.0, 150.0, 100.0, 100.0, 0.0), BLUE)];
    let seg = segment(&art(W, H, 3.0, &shapes), &R);
    let (a, b) = (seg.zone_at(150, 150).unwrap(), seg.zone_at(250, 150).unwrap());
    let shared = a.neighbours.iter().find(|&&(n, _)| n == b.id).expect("voisines").1;
    assert!(shared > 80, "{shared}");
    assert!(b.neighbours.iter().any(|&(n, l)| n == a.id && l == shared));
    // Le trait qui les sépare est aux deux zones : aucun pixel n'est laissé de côté.
    assert_ne!(seg.label_at(200, 150), 0);
}

#[test]
fn the_shape_descriptor_does_not_change_when_the_shape_moves_turns_or_grows() {
    let hu = |shape: Vec<(f32, f32)>| {
        let seg = segment(&art(W, H, 3.0, &[closed(shape, RED)]), &R);
        let hu = seg.inner().next().unwrap().hu;
        hu
    };
    let base = hu(rect(150.0, 150.0, 120.0, 60.0, 0.0));
    let moved = hu(rect(250.0, 140.0, 120.0, 60.0, 0.0));
    let turned = hu(rect(200.0, 150.0, 120.0, 60.0, 37.0));
    let grown = hu(rect(200.0, 150.0, 150.0, 75.0, 0.0));
    let round = hu(ellipse(200.0, 150.0, 60.0, 60.0, 0.0));
    for (name, other) in [("déplacée", moved), ("tournée", turned), ("agrandie", grown)] {
        assert!((other[0] / base[0] - 1.0).abs() < 0.08, "{name} : {other:?} contre {base:?}");
    }
    // Un rond n'a pas la forme d'un rectangle allongé.
    assert!((round[0] / base[0] - 1.0).abs() > 0.1, "{round:?} {base:?}");
}

#[test]
fn the_frame_closes_a_shape_cut_by_it() {
    // Un corps coupé par le bord gauche de l'image : son intérieur est une zone fermée par le cadre.
    let shape = Shape { pts: vec![(0.0, 60.0), (120.0, 60.0), (120.0, 240.0), (0.0, 240.0)], closed: false, fill: None, skip: Vec::new() };
    let seg = segment(&art(W, H, 3.0, &[shape]), &R);
    let body = seg.zone_at(60, 150).expect("le corps");
    assert!(!body.outside, "le bord de l'image ferme la zone");
    assert_ne!(seg.label_at(300, 150), seg.label_at(60, 150));
}
