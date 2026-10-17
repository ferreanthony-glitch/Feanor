//! Cutter : les traits touchés par le geste sont coupés à leurs croisements, seuls les morceaux
//! touchés partent.

use eframe::egui::pos2;

use crate::cutter::{crossings, cut_strokes};
use crate::vector::{Element, VPoint, VectorDrawing};

fn stroke(points: &[(f32, f32)]) -> Element {
    Element::Stroke {
        points: points.iter().map(|&(x, y)| VPoint { x, y, r: 1.0 }).collect(),
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
    }
}

fn ends(e: &Element) -> ((f32, f32), (f32, f32)) {
    let Element::Stroke { points, .. } = e else { panic!("un trait") };
    let (a, b) = (points[0], points[points.len() - 1]);
    ((a.x.round(), a.y.round()), (b.x.round(), b.y.round()))
}

#[test]
fn crossings_are_found_along_the_line() {
    let h = [pos2(0.0, 50.0), pos2(100.0, 50.0)];
    let v = [pos2(30.0, 0.0), pos2(30.0, 100.0)];
    let c = crossings(&h, &v, false);
    assert_eq!(c.len(), 1);
    assert!((c[0] - 0.3).abs() < 1e-4, "{c:?}");
    // Une boucle : la ligne se croise elle-même (deux endroits sur elle).
    let lp = [pos2(0.0, 0.0), pos2(10.0, 10.0), pos2(10.0, 0.0), pos2(0.0, 10.0)];
    assert_eq!(crossings(&lp, &lp, true).len(), 2);
}

#[test]
fn an_overshoot_is_trimmed_at_the_crossing() {
    // Un trait horizontal, un vertical qui le dépasse de 20 en bas ; le geste coupe le dépassement.
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)])] };
    let region = cut_strokes(&mut d, &[pos2(20.0, 62.0), pos2(40.0, 62.0)], 2.0).expect("coupé");
    assert!(region.contains(pos2(30.0, 60.0)));
    assert_eq!(d.elements.len(), 2, "l'horizontal intact, le vertical raccourci");
    assert_eq!(ends(&d.elements[0]), ((0.0, 50.0), (100.0, 50.0)));
    assert_eq!(ends(&d.elements[1]), ((30.0, 0.0), (30.0, 50.0)), "arrêté au croisement");
}

#[test]
fn the_middle_piece_between_two_crossings_goes() {
    // Une ligne coupée par deux verticales ; le geste touche le milieu : restent les deux bouts.
    let mut d = VectorDrawing {
        elements: vec![
            stroke(&[(0.0, 50.0), (100.0, 50.0)]),
            stroke(&[(30.0, 0.0), (30.0, 100.0)]),
            stroke(&[(70.0, 0.0), (70.0, 100.0)]),
        ],
    };
    cut_strokes(&mut d, &[pos2(50.0, 40.0), pos2(50.0, 60.0)], 2.0).expect("coupé");
    let horizontal: Vec<_> = d.elements.iter().map(ends).filter(|(a, b)| a.1 == 50.0 && b.1 == 50.0).collect();
    assert_eq!(horizontal, vec![((0.0, 50.0), (30.0, 50.0)), ((70.0, 50.0), (100.0, 50.0))]);
    assert_eq!(d.elements.len(), 4, "les deux verticales restent entières");
}

#[test]
fn a_click_cuts_the_nearest_piece_and_a_lone_stroke_goes_whole() {
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)])] };
    // Clic sur le dépassement (à 0,5 du trait, rayon 1).
    cut_strokes(&mut d, &[pos2(30.5, 65.0)], 2.0).expect("coupé");
    assert_eq!(ends(&d.elements[1]), ((30.0, 0.0), (30.0, 50.0)));
    // Clic loin de tout : rien.
    assert!(cut_strokes(&mut d, &[pos2(500.0, 500.0)], 2.0).is_none());
    // Un trait qui ne croise rien part en entier.
    let mut lone = VectorDrawing { elements: vec![stroke(&[(0.0, 0.0), (50.0, 0.0)])] };
    cut_strokes(&mut lone, &[pos2(25.0, -5.0), pos2(25.0, 5.0)], 2.0).expect("coupé");
    assert!(lone.elements.is_empty());
}

#[test]
fn fills_and_untouched_strokes_stay() {
    let fill = Element::Fill { contours: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]], color: [9, 9, 9], opacity: 1.0 };
    let mut d = VectorDrawing { elements: vec![fill.clone(), stroke(&[(0.0, 200.0), (100.0, 200.0)])] };
    assert!(cut_strokes(&mut d, &[pos2(0.0, 5.0), pos2(20.0, 5.0)], 2.0).is_none(), "le geste ne touche aucun trait");
    assert_eq!(d.elements[0], fill);
    assert_eq!(d.elements.len(), 2);
}
