//! Interpolation d'images (13l) : intervalles vectoriels, fondus, rééchantillonnage dans le temps.

use eframe::egui::Color32;

use crate::raster::Raster;
use crate::tween::{self, Slot};
use crate::vector::{Element, VPoint, VectorDrawing};

fn stroke(pts: &[(f32, f32)], r: f32) -> Element {
    Element::Stroke { points: pts.iter().map(|&(x, y)| VPoint { x, y, r }).collect(), color: [0, 0, 0], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() }
}

fn points(e: &Element) -> Vec<(f32, f32, f32)> {
    match e {
        Element::Stroke { points, .. } => points.iter().map(|p| (p.x, p.y, p.r)).collect(),
        _ => panic!("un trait"),
    }
}

#[test]
fn a_stroke_moves_from_one_drawing_to_the_other() {
    let a = VectorDrawing { elements: vec![stroke(&[(0.0, 10.0), (100.0, 10.0)], 2.0)] };
    // Même trait plus bas, plus épais, dessiné en trois points.
    let b = VectorDrawing { elements: vec![stroke(&[(0.0, 30.0), (50.0, 30.0), (100.0, 30.0)], 6.0)] };
    let mid = tween::tween_vector(&a, &b, 0.5);
    assert_eq!(mid.elements.len(), 1);
    let p = points(&mid.elements[0]);
    assert_eq!(p.len(), 3, "le plus grand nombre de points des deux");
    for (x, y, r) in &p {
        assert!((y - 20.0).abs() < 1e-3 && (r - 4.0).abs() < 1e-3, "à mi-chemin : {x}, {y}, {r}");
    }
    assert!((p[1].0 - 50.0).abs() < 1e-3, "points répartis le long du trait");
    assert_eq!(tween::first_point(&tween::tween_vector(&a, &b, 0.0)).map(|q| q.y), Some(10.0));
}

#[test]
fn a_stroke_drawn_the_other_way_is_turned_round() {
    let a = VectorDrawing { elements: vec![stroke(&[(0.0, 0.0), (100.0, 0.0)], 2.0)] };
    let b = VectorDrawing { elements: vec![stroke(&[(100.0, 20.0), (0.0, 20.0)], 2.0)] };
    let p = points(&tween::tween_vector(&a, &b, 0.5).elements[0]);
    assert!((p[0].0 - 0.0).abs() < 1e-3 && (p[0].1 - 10.0).abs() < 1e-3, "le début reste à gauche : {:?}", p[0]);
}

#[test]
fn unmatched_elements_fade() {
    let a = VectorDrawing { elements: vec![stroke(&[(0.0, 0.0), (10.0, 0.0)], 1.0)] };
    let b = VectorDrawing {
        elements: vec![stroke(&[(0.0, 5.0), (10.0, 5.0)], 1.0), Element::Fill { contours: vec![vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]]], color: [255, 0, 0], opacity: 1.0 }],
    };
    let mid = tween::tween_vector(&a, &b, 0.25);
    assert_eq!(mid.elements.len(), 2);
    match &mid.elements[1] {
        Element::Fill { opacity, .. } => assert!((opacity - 0.25).abs() < 1e-5, "il apparaît : {opacity}"),
        _ => panic!("le remplissage"),
    }
    // Un trait face à un remplissage : l'un s'efface, l'autre apparaît.
    let c = VectorDrawing { elements: vec![b.elements[1].clone()] };
    assert_eq!(tween::tween_vector(&a, &c, 0.5).elements.len(), 2);
    assert_eq!(tween::tween_vector(&a, &c, 1.0).elements.len(), 1, "à la fin, seul le second reste");
}

#[test]
fn bitmap_images_cross_fade() {
    let mut a = Raster::new(8, 8);
    let mut b = Raster::new(8, 8);
    a.tile_mut(0)[0] = Color32::from_rgba_premultiplied(200, 0, 0, 200);
    b.tile_mut(0)[0] = Color32::from_rgba_premultiplied(0, 0, 100, 100);
    let mid = tween::blend_raster(&a, &b, 0.5);
    assert_eq!(mid.get(0, 0), Color32::from_rgba_premultiplied(100, 0, 50, 150));
    assert_eq!(mid.get(1, 1), Color32::TRANSPARENT);
}

#[test]
fn images_stretched_in_time() {
    let (a, b) = (1, 2);
    let frames = [Some(a), Some(a), Some(b), Some(b)];
    let s = tween::resample(&frames, 8, true);
    assert_eq!(s[0], Slot::Keep(Some(a)));
    assert_eq!(s[1], Slot::Keep(Some(a)), "entre deux mêmes dessins : tenu");
    assert_eq!(s[3], Slot::Tween(a, b, 0.5), "entre A et B : un intervalle");
    assert_eq!(s[7], Slot::Keep(Some(b)));
    assert_eq!(tween::resample(&frames, 8, false)[3], Slot::Keep(Some(a)), "sans interpolation : tenu");
    assert_eq!(tween::resample(&frames, 2, true), vec![Slot::Keep(Some(a)), Slot::Keep(Some(b))], "raccourci");
    assert_eq!(tween::resample(&[Some(a), None, Some(b)], 6, true)[1], Slot::Keep(Some(a)), "pas d'intervalle avec une image vide");
}

#[test]
fn frames_at_another_frame_rate() {
    assert_eq!(tween::rescale_frame(48, 24, 25), 50);
    assert_eq!(tween::rescale_frame(24, 24, 12), 12);
    assert_eq!(tween::rescale_frame(0, 24, 30), 0);
}
