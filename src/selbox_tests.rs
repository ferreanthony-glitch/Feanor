//! La boîte de sélection (`selbox.rs`) : les parties sous le pointeur à tout
//! zoom, les transformations des gestes (échelle et rotation autour du pivot, inclinaison,
//! proportions), les valeurs chiffrées et l'annulation de chaque geste.

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::document::{Document, LayerKind};
use crate::flottante::Flottante;
use crate::history::{Edit, History, Nav};
use crate::selbox::*;
use crate::vector::{Affine, Element, VPoint};

fn near(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 1e-2
}

fn rect() -> Rect {
    Rect::from_min_max(pos2(100.0, 100.0), pos2(300.0, 200.0))
}

/// La boîte de `rect()` vue à l'écran avec un zoom, décalée de (50, 50).
fn screen_box(zoom: f32) -> ([Pos2; 4], Pos2) {
    let f = SelFrame { m: Affine::scale_about(pos2(0.0, 0.0), zoom, zoom).then(&Affine::translate(vec2(50.0, 50.0))), ..SelFrame::new(rect()) };
    (f.corners(), f.pivot_world())
}

fn hit(zoom: f32, p: Pos2) -> Option<Handle> {
    let (quad, pivot) = screen_box(zoom);
    hit_test(quad, pivot, p, &Grabs::default())
}

#[test]
fn every_part_of_the_box_is_reachable_at_every_zoom() {
    for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
        let (q, pivot) = screen_box(zoom);
        let (w, h) = (q[1].x - q[0].x, q[3].y - q[0].y);
        // Le pivot, au centre.
        assert_eq!(hit(zoom, pivot), Some(Handle::Pivot), "zoom {zoom}");
        // Les quatre coins, exactement, et un peu en dedans.
        for k in 0..4 {
            assert_eq!(hit(zoom, q[k]), Some(Handle::Corner(k)), "zoom {zoom}, coin {k}");
            let inward = pivot + (q[k] - pivot) * 0.9;
            let d = inward.distance(q[k]);
            if d <= 5.0 {
                assert_eq!(hit(zoom, inward), Some(Handle::Corner(k)), "zoom {zoom}, coin {k} un peu en dedans");
            }
        }
        // Les milieux des côtés.
        for k in 0..4 {
            let mid = q[k] + (q[(k + 1) % 4] - q[k]) * 0.5;
            assert_eq!(hit(zoom, mid), Some(Handle::Edge(k)), "zoom {zoom}, côté {k}");
        }
        // Un bord, au quart de sa longueur : incliner (loin des poignées).
        for k in 0..4 {
            let p = q[k] + (q[(k + 1) % 4] - q[k]) * 0.25;
            assert_eq!(hit(zoom, p), Some(Handle::Skew(k)), "zoom {zoom}, bord {k}");
        }
        // Dedans, ni près d'un bord ni du pivot : déplacer.
        let inside = pivot + vec2(0.15 * w, 0.1 * h);
        assert_eq!(hit(zoom, inside), Some(Handle::Move), "zoom {zoom}");
        // Juste dehors d'un coin, en diagonale : tourner.
        for (k, out) in [vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0)].into_iter().enumerate() {
            let p = q[k] + out * 8.0;
            assert_eq!(hit(zoom, p), Some(Handle::Rotate(k)), "zoom {zoom}, rotation au coin {k}");
        }
        // Loin de tout : rien.
        assert_eq!(hit(zoom, q[0] + vec2(-200.0, -200.0)), None, "zoom {zoom}");
        assert_eq!(hit(zoom, q[2] + vec2(GRAB_ROTATE + 20.0, 0.0)), None, "zoom {zoom}");
    }
}

#[test]
fn hit_sizes_are_screen_pixels_not_drawing_pixels() {
    // Une poignée se prend à 9 px d'écran de son centre, quel que soit le zoom ; 14 px plus loin,
    // dehors, c'est la rotation, puis rien.
    for zoom in [0.5, 1.0, 4.0] {
        let (q, _) = screen_box(zoom);
        assert_eq!(hit(zoom, q[2] + vec2(GRAB_HANDLE * 0.7, GRAB_HANDLE * 0.7)), Some(Handle::Corner(2)), "zoom {zoom}");
        assert_eq!(hit(zoom, q[2] + vec2(GRAB_HANDLE + 6.0, 0.0)), Some(Handle::Rotate(2)), "zoom {zoom}");
        assert_eq!(hit(zoom, q[2] + vec2(GRAB_ROTATE * 1.5, GRAB_ROTATE * 1.5)), None, "zoom {zoom}");
    }
}

#[test]
fn the_skew_band_is_thin_and_does_not_steal_the_handles() {
    let (q, _) = screen_box(1.0);
    let top_quarter = q[0] + (q[1] - q[0]) * 0.25;
    // 3 px sous le bord du haut : toujours le bord ; 3 px au-dessus aussi ; 10 px plus bas : dedans.
    assert_eq!(hit(1.0, top_quarter + vec2(0.0, 3.0)), Some(Handle::Skew(0)));
    assert_eq!(hit(1.0, top_quarter - vec2(0.0, 3.0)), Some(Handle::Skew(0)));
    assert_eq!(hit(1.0, top_quarter + vec2(0.0, GRAB_SKEW + 5.0)), Some(Handle::Move));
    // Au-dessus de la bande, mais pas près d'un coin : rien ne se prend (la rotation ne vaut que près d'un coin).
    assert_eq!(hit(1.0, top_quarter - vec2(0.0, GRAB_SKEW + 5.0)), None);
    // Le milieu du bord est une poignée, pas une inclinaison.
    assert_eq!(hit(1.0, q[0] + (q[1] - q[0]) * 0.5), Some(Handle::Edge(0)));
}

#[test]
fn a_tilted_box_is_hit_in_its_own_axes() {
    // Boîte tournée de 30° : ses poignées sont sur ses côtés, pas sur ceux du rectangle droit.
    let f = SelFrame::new(rect()).then(&Affine::rotate_about(pos2(200.0, 150.0), 30f32.to_radians()));
    let (q, pivot) = (f.corners(), f.pivot_world());
    let g = Grabs::default();
    for k in 0..4 {
        assert_eq!(hit_test(q, pivot, q[k], &g), Some(Handle::Corner(k)));
        let mid = f.mids()[k];
        assert_eq!(hit_test(q, pivot, mid, &g), Some(Handle::Edge(k)));
        // Hors du coin, le long de la diagonale depuis le centre : tourner.
        let out = (q[k] - pivot).normalized() * 12.0;
        assert_eq!(hit_test(q, pivot, q[k] + out, &g), Some(Handle::Rotate(k)));
    }
    // Le coin haut-gauche du rectangle droit n'est plus une poignée.
    assert_ne!(hit_test(q, pivot, rect().left_top(), &g), Some(Handle::Corner(0)));
}

#[test]
fn tiny_boxes_keep_their_parts_reachable() {
    // Une boîte de 12 × 8 px : le pivot au centre, les coins aux coins, dedans rien d'absurde.
    let small = Rect::from_min_size(pos2(10.0, 10.0), vec2(12.0, 8.0));
    let f = SelFrame::new(small);
    let (q, pivot) = (f.corners(), f.pivot_world());
    let g = Grabs::default();
    assert_eq!(hit_test(q, pivot, pivot, &g), Some(Handle::Pivot));
    assert!(matches!(hit_test(q, pivot, q[0], &g), Some(Handle::Corner(0))));
    assert_eq!(hit_test(q, pivot, q[2] + vec2(9.0, 9.0), &g), Some(Handle::Rotate(2)));
}

fn mods() -> Mods {
    Mods::default()
}

#[test]
fn moving_translates_and_shift_locks_the_axis() {
    let f = SelFrame::new(rect());
    let d = gesture(Handle::Move, &f, pos2(150.0, 150.0), pos2(170.0, 155.0), mods());
    assert!(near(d.apply(pos2(1.0, 2.0)), pos2(21.0, 7.0)));
    let d = gesture(Handle::Move, &f, pos2(150.0, 150.0), pos2(170.0, 155.0), Mods { shift: true, ..mods() });
    assert!(near(d.apply(pos2(0.0, 0.0)), pos2(20.0, 0.0)), "plutôt à l'horizontale");
    let d = gesture(Handle::Move, &f, pos2(150.0, 150.0), pos2(152.0, 190.0), Mods { shift: true, ..mods() });
    assert!(near(d.apply(pos2(0.0, 0.0)), pos2(0.0, 40.0)), "plutôt à la verticale");
}

#[test]
fn a_corner_scales_about_the_opposite_corner_and_ctrl_about_the_pivot() {
    let f = SelFrame::new(rect());
    // Le coin bas-droite (300, 200) tiré en (400, 250) : la boîte devient 300 × 150 depuis (100, 100).
    let d = gesture(Handle::Corner(2), &f, pos2(300.0, 200.0), pos2(400.0, 250.0), mods());
    let after = f.then(&d);
    let c = after.corners();
    assert!(near(c[0], pos2(100.0, 100.0)), "le coin opposé ne bouge pas");
    assert!(near(c[2], pos2(400.0, 250.0)));
    // Avec Ctrl : autour du pivot (le centre, (200, 150)) : le coin va deux fois plus loin, symétriquement.
    let d = gesture(Handle::Corner(2), &f, pos2(300.0, 200.0), pos2(400.0, 250.0), Mods { ctrl: true, ..mods() });
    let after = f.then(&d);
    assert!(near(after.pivot_world(), pos2(200.0, 150.0)), "le pivot ne bouge pas");
    assert!(near(after.corners()[2], pos2(400.0, 250.0)));
    assert!(near(after.corners()[0], pos2(0.0, 50.0)));
}

#[test]
fn a_side_handle_scales_one_axis_only() {
    let f = SelFrame::new(rect());
    // Le côté droit (x = 300) tiré à x = 400 : la largeur passe de 200 à 300, la hauteur ne change pas.
    let d = gesture(Handle::Edge(1), &f, pos2(300.0, 150.0), pos2(400.0, 190.0), mods());
    let p = f.then(&d).params();
    assert!((p.width - 300.0).abs() < 1e-2 && (p.height - 100.0).abs() < 1e-2, "{p:?}");
    assert!(near(f.then(&d).corners()[0], pos2(100.0, 100.0)), "le côté gauche reste en place");
    // Le côté du haut (y = 100) tiré à y = 50 : la hauteur passe de 100 à 150, le côté du bas reste.
    let d = gesture(Handle::Edge(0), &f, pos2(200.0, 100.0), pos2(210.0, 50.0), mods());
    let p = f.then(&d).params();
    assert!((p.width - 200.0).abs() < 1e-2 && (p.height - 150.0).abs() < 1e-2, "{p:?}");
    assert!(near(f.then(&d).corners()[3], pos2(100.0, 200.0)));
}

#[test]
fn scaling_a_tilted_box_follows_its_axes() {
    let f = SelFrame::new(rect()).then(&Affine::rotate_about(pos2(200.0, 150.0), 30f32.to_radians()));
    let right = f.mids()[1];
    // Tirer le côté droit de 50 px le long de l'axe de la boîte : seule la largeur change.
    let axis = (f.corners()[1] - f.corners()[0]).normalized();
    let d = gesture(Handle::Edge(1), &f, right, right + axis * 50.0, mods());
    let (p0, p1) = (f.params(), f.then(&d).params());
    assert!((p1.width - (p0.width + 50.0)).abs() < 1e-2, "{p0:?} -> {p1:?}");
    assert!((p1.height - p0.height).abs() < 1e-2 && (p1.angle - p0.angle).abs() < 1e-2);
    // Le côté gauche n'a pas bougé.
    assert!(near(f.then(&d).mids()[3], f.mids()[3]));
}

#[test]
fn keep_proportions_keeps_the_ratio_with_shift_or_with_the_setting() {
    let f = SelFrame::new(rect());
    let start = pos2(300.0, 200.0);
    let to = pos2(500.0, 230.0);
    let ratio = |m: Mods| {
        let p = f.then(&gesture(Handle::Corner(2), &f, start, to, m)).params();
        p.width / p.height
    };
    assert!((ratio(Mods { shift: true, ..mods() }) - 2.0).abs() < 1e-3, "Maj : la forme est gardée");
    assert!((ratio(Mods { keep: true, ..mods() }) - 2.0).abs() < 1e-3, "le réglage : la forme est gardée");
    assert!((ratio(Mods { keep: true, shift: true, ..mods() }) - 2.0).abs() > 0.1, "le réglage et Maj se compensent");
    assert!((ratio(mods()) - 2.0).abs() > 0.1, "sans rien : libre");
    // Un côté qui garde les proportions agrandit aussi l'autre sens.
    let p = f.then(&gesture(Handle::Edge(1), &f, pos2(300.0, 150.0), pos2(400.0, 150.0), Mods { keep: true, ..mods() })).params();
    assert!((p.width / p.height - 2.0).abs() < 1e-3, "{p:?}");
}

#[test]
fn rotating_turns_about_the_pivot_and_shift_snaps_to_15_degrees() {
    let f = SelFrame::new(rect());
    let c = f.pivot_world();
    // De la droite du centre vers le bas du centre : un quart de tour (y vers le bas).
    let d = gesture(Handle::Rotate(2), &f, c + vec2(100.0, 0.0), c + vec2(0.0, 100.0), mods());
    assert!(near(d.apply(c + vec2(100.0, 0.0)), c + vec2(0.0, 100.0)));
    assert!(near(d.apply(c), c), "le pivot ne bouge pas");
    assert!((f.then(&d).params().angle - 90.0).abs() < 1e-2);
    // Maj : 37° devient 30°.
    let a = 37f32.to_radians();
    let d = gesture(Handle::Rotate(2), &f, c + vec2(100.0, 0.0), c + vec2(a.cos(), a.sin()) * 100.0, Mods { shift: true, ..mods() });
    assert!((f.then(&d).params().angle - 30.0).abs() < 1e-2);
    // Un pivot déplacé : la rotation tourne autour de lui.
    let g = SelFrame { pivot: Some(pos2(100.0, 100.0)), ..f };
    let p = g.pivot_world();
    let d = gesture(Handle::Rotate(0), &g, p + vec2(50.0, 0.0), p + vec2(0.0, 50.0), mods());
    assert!(near(d.apply(p), p));
    assert!(near(g.then(&d).pivot_world(), p), "le pivot suit la boîte");
    assert!(near(d.apply(rect().right_top()), p + vec2(0.0, 200.0)), "{:?}", d.apply(rect().right_top()));
}

#[test]
fn skewing_slides_an_edge_along_itself_and_keeps_the_opposite_edge() {
    let f = SelFrame::new(rect());
    // Le bord du haut tiré de 40 px vers la droite : ses coins glissent de 40, le bord du bas reste.
    let d = gesture(Handle::Skew(0), &f, pos2(150.0, 100.0), pos2(190.0, 130.0), mods());
    let c = f.then(&d).corners();
    assert!(near(c[0], pos2(140.0, 100.0)) && near(c[1], pos2(340.0, 100.0)), "{c:?}");
    assert!(near(c[3], pos2(100.0, 200.0)) && near(c[2], pos2(300.0, 200.0)), "le bord du bas reste");
    // La hauteur ne change pas : c'est un cisaillement, pas une échelle.
    let p = f.then(&d).params();
    assert!((p.height - 100.0).abs() < 1e-2 && (p.width - 200.0).abs() < 1e-2, "{p:?}");
    assert!((p.skew - (-(40.0f32 / 100.0).atan().to_degrees())).abs() < 1e-2 || (p.skew.abs() - (40.0f32 / 100.0).atan().to_degrees()).abs() < 1e-2, "{p:?}");
    // Le bord de droite tiré de 30 px vers le bas : ses coins glissent de 30, celui de gauche reste.
    let d = gesture(Handle::Skew(1), &f, pos2(300.0, 150.0), pos2(310.0, 180.0), mods());
    let c = f.then(&d).corners();
    assert!(near(c[1], pos2(300.0, 130.0)) && near(c[2], pos2(300.0, 230.0)), "{c:?}");
    assert!(near(c[0], pos2(100.0, 100.0)) && near(c[3], pos2(100.0, 200.0)));
    // Ctrl : autour de la ligne du pivot (y = 150) : le bord du haut glisse de 40, le bas de −40 / ... la ligne du milieu reste.
    let d = gesture(Handle::Skew(0), &f, pos2(150.0, 100.0), pos2(170.0, 100.0), Mods { ctrl: true, ..mods() });
    let g = f.then(&d);
    assert!(near(g.pivot_world(), pos2(200.0, 150.0)), "le pivot reste en place");
    assert!((g.corners()[0].x - 120.0).abs() < 1e-2, "le haut glisse de 20 : {:?}", g.corners());
    assert!((g.corners()[3].x - 80.0).abs() < 1e-2, "le bas glisse de −20 : {:?}", g.corners());
}

#[test]
fn skewing_a_tilted_box_slides_along_its_own_edge() {
    let f = SelFrame::new(rect()).then(&Affine::rotate_about(pos2(200.0, 150.0), 30f32.to_radians()));
    let top_mid = f.mids()[0];
    let along = (f.corners()[1] - f.corners()[0]).normalized();
    let d = gesture(Handle::Skew(0), &f, top_mid, top_mid + along * 25.0, mods());
    let g = f.then(&d);
    // Le bord du haut a glissé de 25 px le long de lui-même ; celui du bas n'a pas bougé.
    assert!(near(g.corners()[0], f.corners()[0] + along * 25.0), "{:?}", g.corners());
    assert!(near(g.corners()[3], f.corners()[3]));
    assert!((g.corners()[1] - g.corners()[0]).length() - 200.0 < 1e-2, "le bord du haut garde sa longueur");
}

#[test]
fn the_pivot_follows_the_box_and_snaps_back_to_the_centre() {
    let f = SelFrame::new(rect());
    // Tiré de 60 px à droite : un pivot hors du centre.
    let local = move_pivot(&f, pos2(200.0, 150.0), pos2(260.0, 150.0), 6.0, 1.0).expect("pivot déplacé");
    assert!(near(local, pos2(260.0, 150.0)));
    let g = SelFrame { pivot: Some(local), ..f };
    // La boîte déplacée : le pivot suit.
    let moved = g.then(&Affine::translate(vec2(10.0, 5.0)));
    assert!(near(moved.pivot_world(), pos2(270.0, 155.0)));
    // Retombé près du centre (à 4 px d'écran) : il s'y remet.
    assert_eq!(move_pivot(&g, pos2(260.0, 150.0), pos2(203.0, 152.0), 6.0, 1.0), None);
}

#[test]
fn params_round_trip_and_fields_edit_one_thing_at_a_time() {
    let f = SelFrame::new(rect());
    let p = f.params();
    assert_eq!((p.x, p.y, p.width, p.height), (200.0, 150.0, 200.0, 100.0));
    assert!(p.angle.abs() < 1e-4 && p.skew.abs() < 1e-4);
    let back = f.with_params(&p).expect("boîte");
    assert!(near(back.corners()[0], f.corners()[0]) && near(back.corners()[2], f.corners()[2]));
    // Une boîte tournée, inclinée, mise à l'échelle : les valeurs se relisent identiques.
    let g = f.then(&Affine::scale_about(pos2(200.0, 150.0), 1.5, 0.8)).then(&Affine::rotate_about(pos2(180.0, 120.0), 0.6));
    let q = g.params();
    let again = g.with_params(&q).expect("boîte").params();
    assert!((again.angle - q.angle).abs() < 1e-3 && (again.width - q.width).abs() < 1e-2 && (again.height - q.height).abs() < 1e-2 && (again.skew - q.skew).abs() < 1e-3, "{q:?} / {again:?}");
    // Changer l'angle seul : le pivot, la taille et l'inclinaison ne bougent pas.
    let h = g.with_params(&BoxParams { angle: 10.0, ..q }).expect("boîte");
    let r = h.params();
    assert!((r.angle - 10.0).abs() < 1e-3 && (r.width - q.width).abs() < 1e-2 && (r.height - q.height).abs() < 1e-2 && (r.skew - q.skew).abs() < 1e-3);
    assert!(near(h.pivot_world(), g.pivot_world()));
    // L'inclinaison seule.
    let k = f.with_params(&BoxParams { skew: 20.0, ..f.params() }).expect("boîte");
    let r = k.params();
    assert!((r.skew - 20.0).abs() < 1e-3 && (r.width - 200.0).abs() < 1e-2 && (r.height - 100.0).abs() < 1e-2 && r.angle.abs() < 1e-3);
    assert!(near(k.pivot_world(), f.pivot_world()), "le pivot reste en place");
    // La position : la boîte se déplace, rien d'autre.
    let m = f.with_params(&BoxParams { x: 500.0, y: 40.0, ..f.params() }).expect("boîte");
    assert!(near(m.pivot_world(), pos2(500.0, 40.0)) && (m.params().width - 200.0).abs() < 1e-3);
    // L'écart entre deux boîtes est la transformation à appliquer.
    let d = f.delta_to(&m).expect("écart");
    assert!(near(f.then(&d).corners()[0], m.corners()[0]) && near(f.then(&d).corners()[2], m.corners()[2]));
}

#[test]
fn a_mirrored_box_keeps_its_mirror_when_a_field_changes() {
    let f = SelFrame::new(rect()).then(&Affine::scale_about(pos2(200.0, 150.0), -1.0, 1.0));
    let p = f.params();
    assert!((p.width - 200.0).abs() < 1e-3 && (p.height - 100.0).abs() < 1e-3, "{p:?}");
    let g = f.with_params(&BoxParams { width: 300.0, ..p }).expect("boîte");
    // Le coin haut-gauche de rect() est toujours à droite du haut-droit : le miroir est resté.
    assert!(g.corners()[0].x > g.corners()[1].x, "{:?}", g.corners());
    assert!((g.params().width - 300.0).abs() < 1e-2);
}

// ------------------------------------------------------------------ annulation de chaque geste

fn stroke(points: &[(f32, f32)]) -> Element {
    Element::Stroke {
        points: points.iter().map(|&(x, y)| VPoint { x, y, r: 2.0 }).collect(),
        color: [20, 20, 20],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

fn nav() -> Nav {
    Nav { panel: 0, layer: 0 }
}

#[test]
fn every_gesture_is_one_named_undo_step_that_undoes_and_redoes_exactly() {
    let gestures: [(Handle, (Pos2, Pos2), &str); 5] = [
        (Handle::Move, (pos2(150.0, 150.0), pos2(190.0, 170.0)), "Déplacer"),
        (Handle::Corner(2), (pos2(300.0, 200.0), pos2(400.0, 260.0)), "Mettre à l'échelle"),
        (Handle::Edge(1), (pos2(300.0, 150.0), pos2(380.0, 150.0)), "Mettre à l'échelle"),
        (Handle::Rotate(2), (pos2(300.0, 200.0), pos2(250.0, 260.0)), "Tourner"),
        (Handle::Skew(0), (pos2(150.0, 100.0), pos2(190.0, 100.0)), "Incliner"),
    ];
    for (handle, (from, to), name) in gestures {
        let mut doc = Document::new(400, 300, 24);
        let cel = doc.new_cel(LayerKind::Vector);
        let elements = vec![stroke(&[(100.0, 100.0), (300.0, 100.0), (300.0, 200.0)]), stroke(&[(120.0, 190.0), (280.0, 130.0)])];
        doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = elements;
        let items = [0usize, 1];
        let frame = SelFrame::new(doc.cels[&cel].vector.as_ref().unwrap().elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds())));
        let before = doc.cels[&cel].vector.clone().unwrap();
        let originals: Vec<Element> = before.elements.clone();
        let aff = gesture(handle, &frame, from, to, Mods::default());
        Flottante::apply_geometry(&mut doc, cel, &items, &originals, &aff);
        let after = doc.cels[&cel].vector.clone().unwrap();
        assert!(after.elements.iter().zip(&before.elements).any(|(a, b)| !a.same_as(b)), "{name} : le geste change quelque chose");
        // La boîte d'après contient les éléments d'après (le cadre transformé enferme chaque point).
        let boxed = frame.then(&aff);
        for e in &after.elements {
            let Element::Stroke { points, .. } = e else { unreachable!() };
            let inv = boxed.m.inverse().expect("inversible");
            for p in points {
                let local = inv.apply(pos2(p.x, p.y));
                assert!(frame.rect0.expand(0.05).contains(local), "{name} : {p:?} sort de la boîte");
            }
        }
        let mut history = History::default();
        // (Comme `App::named_step` : le nom du geste est le contexte de l'étape.)
        let ctx = history.context.replace(handle.step_name().to_string());
        history.push(Edit::Vector { cel, drawing: before.clone() });
        history.context = ctx;
        assert_eq!(history.names().0, vec![name], "{name} : une étape, au nom du geste");
        let mut n = nav();
        assert!(history.undo(&mut doc, &mut n));
        let undone = doc.cels[&cel].vector.clone().unwrap();
        assert!(undone.elements.iter().zip(&before.elements).all(|(a, b)| a.same_as(b)), "{name} : annuler rend le dessin d'avant");
        assert_eq!(history.names().1, vec![name]);
        assert!(history.redo(&mut doc, &mut n));
        let redone = doc.cels[&cel].vector.clone().unwrap();
        assert!(redone.elements.iter().zip(&after.elements).all(|(a, b)| a.same_as(b)), "{name} : rétablir rend le dessin d'après");
    }
}

#[test]
fn step_names_are_the_four_french_verbs() {
    assert_eq!(Handle::Move.step_name(), "Déplacer");
    assert_eq!(Handle::Corner(0).step_name(), "Mettre à l'échelle");
    assert_eq!(Handle::Edge(3).step_name(), "Mettre à l'échelle");
    assert_eq!(Handle::Rotate(1).step_name(), "Tourner");
    assert_eq!(Handle::Skew(2).step_name(), "Incliner");
}

#[test]
fn resize_cursors_follow_the_screen_direction_of_the_handle() {
    // Rectangle droit : un coin « \ » (haut-gauche, bas-droite), un coin « / », un côté vertical, un horizontal.
    let (q, _) = screen_box(1.0);
    let axis = |h| resize_cursor_axis(handle_direction(q, h).expect("direction"));
    assert_eq!(axis(Handle::Corner(0)), 1);
    assert_eq!(axis(Handle::Corner(2)), 1);
    assert_eq!(axis(Handle::Corner(1)), 3);
    assert_eq!(axis(Handle::Corner(3)), 3);
    assert_eq!(axis(Handle::Edge(0)), 2);
    assert_eq!(axis(Handle::Edge(1)), 0);
    // Tournée d'un quart de tour, le côté du haut devient horizontal à l'écran.
    let f = SelFrame::new(rect()).then(&Affine::rotate_about(pos2(200.0, 150.0), 90f32.to_radians()));
    assert_eq!(resize_cursor_axis(handle_direction(f.corners(), Handle::Edge(0)).expect("direction")), 0);
}
