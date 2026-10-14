//! Tests des formes (5c) : courbes qui passent par le pointeur, polyligne, éléments produits.

use eframe::egui::{pos2, Pos2};

use crate::shapes::{self, LineMode, ShapeEdit};
use crate::vector::Element;

fn close(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 1e-3
}

#[test]
fn a_curve_passes_through_the_pointer() {
    let (a, b) = (pos2(0.0, 0.0), pos2(100.0, 0.0));
    let mut e = ShapeEdit::curve(a, b, LineMode::Curve);
    // Avant courbure : une droite.
    assert!(e.points().iter().all(|p| p.y.abs() < 1e-4));
    let p = pos2(30.0, 40.0);
    e.bend_to(p);
    let ShapeEdit::Curve { c1, c2, .. } = e.clone() else { panic!() };
    assert!(close(shapes::cubic_at(a, c1, c2, b, 0.5), p), "{:?}", shapes::cubic_at(a, c1, c2, b, 0.5));
    assert!(close(e.points()[0], a) && close(*e.points().last().unwrap(), b), "extrémités fixes");
    assert!(e.end_bend(), "une seule courbure");
}

#[test]
fn an_s_curve_takes_two_bends() {
    let (a, b) = (pos2(0.0, 0.0), pos2(90.0, 0.0));
    let mut e = ShapeEdit::curve(a, b, LineMode::SCurve);
    e.bend_to(pos2(45.0, -30.0));
    assert!(!e.end_bend(), "il en faut deux");
    let second = pos2(60.0, 25.0);
    e.bend_to(second);
    let ShapeEdit::Curve { c1, c2, .. } = e.clone() else { panic!() };
    assert!(close(shapes::cubic_at(a, c1, c2, b, 2.0 / 3.0), second));
    // Le S : la courbe passe d'un côté de la corde à l'autre.
    let pts = e.points();
    assert!(pts.iter().any(|p| p.y < -1.0) && pts.iter().any(|p| p.y > 1.0));
    assert!(e.end_bend());
    assert_eq!(LineMode::Straight.bends(), 0);
}

#[test]
fn snapping_and_squares() {
    let s = shapes::snap_angle(pos2(0.0, 0.0), pos2(10.0, 1.0));
    assert!(close(s, pos2(10.0498, 0.0)), "{s:?}");
    let d = shapes::snap_angle(pos2(0.0, 0.0), pos2(10.0, 9.0));
    assert!((d.x - d.y).abs() < 1e-3, "45° : {d:?}");
    assert_eq!(shapes::square_corner(pos2(0.0, 0.0), pos2(-3.0, 8.0)), pos2(-8.0, 8.0));
    let r = shapes::rect_points(pos2(10.0, 10.0), pos2(0.0, 5.0));
    assert_eq!(r.len(), 5);
    assert_eq!(r[0], r[4]);
    let e = shapes::ellipse_points(pos2(0.0, 0.0), pos2(20.0, 10.0));
    assert!(e.iter().all(|p| ((p.x - 10.0) / 10.0).powi(2) + ((p.y - 5.0) / 5.0).powi(2) - 1.0 < 1e-3));
}

#[test]
fn polyline_closing_and_elements() {
    let pts = [pos2(0.0, 0.0), pos2(50.0, 0.0), pos2(50.0, 40.0)];
    assert!(shapes::closes(&pts, pos2(3.0, -2.0), 5.0));
    assert!(!shapes::closes(&pts, pos2(9.0, 0.0), 5.0));
    assert!(!shapes::closes(&pts[..2], pos2(0.0, 0.0), 5.0), "il faut trois points");
    // Ouverte : un trait par les trois points.
    let Some(Element::Stroke { points, .. }) = shapes::element(&pts, false, false, 4.0, [1, 2, 3]) else { panic!() };
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].r, 2.0);
    // Fermée : le trait revient au départ.
    let Some(Element::Stroke { points, .. }) = shapes::element(&pts, true, false, 4.0, [1, 2, 3]) else { panic!() };
    assert_eq!(points.len(), 4);
    assert_eq!((points[3].x, points[3].y), (0.0, 0.0));
    // Fermée et pleine : une zone, sans point en double.
    let mut doubled = pts.to_vec();
    doubled.push(pos2(50.0, 40.0));
    doubled.push(pos2(0.0, 0.0));
    let Some(Element::Fill { contours, .. }) = shapes::element(&doubled, true, true, 4.0, [1, 2, 3]) else { panic!() };
    assert_eq!(contours[0].len(), 3);
    // Sans étendue : rien.
    assert!(shapes::element(&[pos2(5.0, 5.0), pos2(5.1, 5.1)], false, false, 4.0, [0; 3]).is_none());
    assert!(shapes::element(&[pos2(5.0, 5.0)], false, false, 4.0, [0; 3]).is_none());
}

#[test]
fn shape_tools_have_their_actions() {
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for action in ["outil.ligne", "outil.ligne_droite", "outil.ligne_courbe", "outil.ligne_s", "outil.rectangle", "outil.ellipse", "outil.polyligne"] {
        assert!(crate::keymap::action_info(action).is_some(), "{action}");
        assert!(map.warnings.iter().all(|w| !w.contains(action)), "{action}");
    }
    for tool in crate::app::Tool::ALL {
        assert!(crate::keymap::action_info(tool.action()).is_some(), "{}", tool.action());
    }
}
