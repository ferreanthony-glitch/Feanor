//! Tests de la roue chromatique (passe couleur) : teinte sur l'anneau, carré et triangle
//! saturation / valeur (aller et retour), parties visées, maillages.

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::couleur::hsv_to_rgb;
use crate::roue::{Wheel, WheelShape, Zone};

fn wheel() -> Wheel {
    Wheel::in_rect(Rect::from_min_max(pos2(0.0, 0.0), pos2(202.0, 202.0)))
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn hues_turn_clockwise_from_red_at_the_top() {
    let w = wheel();
    let c = w.center;
    assert!(near(w.hue_at(pos2(c.x, c.y - 50.0)), 0.0, 1e-3), "rouge en haut");
    assert!(near(w.hue_at(pos2(c.x + 50.0, c.y)), 90.0, 1e-3), "à droite");
    assert!(near(w.hue_at(pos2(c.x, c.y + 50.0)), 180.0, 1e-3), "en bas");
    assert!(near(w.hue_at(pos2(c.x - 50.0, c.y)), 270.0, 1e-3), "à gauche");
    for hue in [0.0, 30.0, 59.5, 120.0, 211.0, 300.0, 359.0] {
        assert!(near(w.hue_at(w.ring_point(hue)), hue, 1e-2), "{hue}");
        assert_eq!(w.zone(w.ring_point(hue)), Some(Zone::Ring));
    }
}

#[test]
fn the_square_goes_both_ways() {
    let w = wheel();
    let r = w.square();
    assert!(r.width() < w.inner * 2.0_f32.sqrt(), "le carré tient dans l'anneau");
    assert_eq!(w.square_sv(r.left_top()), (0.0, 1.0), "en haut à gauche : blanc (sans saturation, pleine valeur)");
    assert_eq!(w.square_sv(r.right_top()), (1.0, 1.0), "en haut à droite : la teinte pure");
    assert_eq!(w.square_sv(r.left_bottom()).1, 0.0, "en bas : noir");
    for (s, v) in [(0.0, 0.0), (0.25, 0.75), (0.5, 0.5), (1.0, 0.1), (0.9, 1.0)] {
        let (s2, v2) = w.square_sv(w.square_point(s, v));
        assert!(near(s, s2, 1e-4) && near(v, v2, 1e-4), "({s}, {v}) → ({s2}, {v2})");
    }
    // Hors du carré : bornes.
    assert_eq!(w.square_sv(pos2(-100.0, -100.0)), (0.0, 1.0));
    assert_eq!(w.square_sv(pos2(500.0, 500.0)), (1.0, 0.0));
}

#[test]
fn the_triangle_has_hue_white_and_black_at_its_corners() {
    let w = wheel();
    for hue in [0.0, 75.0, 180.0, 333.0] {
        let [h, white, black] = w.triangle(hue);
        // La teinte pure pointe vers sa place sur l'anneau.
        assert!(near(w.hue_at(h), hue, 1e-2));
        assert!(w.triangle_point(hue, 1.0, 1.0).distance(h) < 1e-3);
        assert!(w.triangle_point(hue, 0.0, 1.0).distance(white) < 1e-3);
        assert!(w.triangle_point(hue, 0.3, 0.0).distance(black) < 1e-3, "à valeur nulle, tout est au noir");
        for (s, v) in [(0.5, 0.5), (0.2, 0.9), (0.95, 0.3), (0.0, 0.4), (1.0, 0.7)] {
            let (s2, v2) = w.triangle_sv(hue, w.triangle_point(hue, s, v));
            assert!(near(s, s2, 1e-3) && near(v, v2, 1e-3), "{hue} : ({s}, {v}) → ({s2}, {v2})");
        }
        // Au-delà des côtés : bornes, sans sortir de 0..1.
        let far = w.center + (h - w.center) * 3.0;
        let (s, v) = w.triangle_sv(hue, far);
        assert!((0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&v));
        assert_eq!(w.triangle_sv(hue, black), (0.0, 0.0));
    }
}

#[test]
fn a_press_aims_at_the_ring_the_inside_or_nothing() {
    let w = wheel();
    assert_eq!(w.zone(w.center), Some(Zone::Inside));
    assert_eq!(w.zone(w.square().right_top()), Some(Zone::Inside));
    assert_eq!(w.zone(pos2(0.0, 0.0)), None, "coin hors de la roue");
    for shape in [WheelShape::Square, WheelShape::Triangle] {
        let p = w.sv_point(shape, 40.0, 0.6, 0.7);
        let (s, v) = w.sv_at(shape, 40.0, p);
        assert!(near(s, 0.6, 1e-3) && near(v, 0.7, 1e-3), "{shape:?}");
        assert_eq!(w.zone(p), Some(Zone::Inside));
    }
}

// ------------------------------------------------------------ la roue à l'écran (egui sans fenêtre)

use eframe::egui::{self, vec2, Event, Modifiers, PointerButton, RawInput};

/// Une image d'egui avec ces événements, la roue au milieu ; renvoie son rectangle.
fn frame(ctx: &egui::Context, time: &mut f64, events: Vec<Event>, hsv: &mut [f32; 3], shape: WheelShape) -> Rect {
    *time += 1.0 / 60.0;
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))),
        time: Some(*time),
        events,
        ..Default::default()
    };
    let mut rect = Rect::NOTHING;
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| rect = crate::colorui::wheel_ui(ui, 200.0, hsv, shape).rect);
    });
    rect
}

fn button(p: Pos2, which: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: which, pressed, modifiers: Modifiers::NONE }
}

/// Appuie en `from`, glisse jusqu'à `to` en quelques pas, lâche.
fn drag(ctx: &egui::Context, time: &mut f64, from: Pos2, to: Pos2, hsv: &mut [f32; 3], shape: WheelShape) {
    frame(ctx, time, vec![Event::PointerMoved(from)], hsv, shape);
    frame(ctx, time, vec![button(from, PointerButton::Primary, true)], hsv, shape);
    for k in 1..=6 {
        let p = from + (to - from) * (k as f32 / 6.0);
        frame(ctx, time, vec![Event::PointerMoved(p)], hsv, shape);
    }
    frame(ctx, time, vec![button(to, PointerButton::Primary, false)], hsv, shape);
    frame(ctx, time, vec![Event::PointerGone], hsv, shape);
    *time += 1.0;
}

fn screen_wheel() -> (egui::Context, f64, Wheel) {
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut hsv = [0.0, 0.0, 0.0];
    let rect = frame(&ctx, &mut time, Vec::new(), &mut hsv, WheelShape::Square);
    // egui ne voit pas le tout premier clic d'un contexte neuf : un clic dans le vide.
    let empty = pos2(390.0, 290.0);
    frame(&ctx, &mut time, vec![Event::PointerMoved(empty)], &mut hsv, WheelShape::Square);
    frame(&ctx, &mut time, vec![button(empty, PointerButton::Primary, true)], &mut hsv, WheelShape::Square);
    frame(&ctx, &mut time, vec![button(empty, PointerButton::Primary, false)], &mut hsv, WheelShape::Square);
    time += 1.0;
    (ctx, time, Wheel::in_rect(rect))
}

#[test]
fn clicking_and_dragging_in_the_wheel_sets_the_color() {
    let (ctx, mut time, w) = screen_wheel();
    // Un clic dans l'anneau, à droite : la teinte 90° ; saturation et valeur ne bougent pas.
    let mut hsv = [10.0, 0.5, 0.6];
    let right = w.center + vec2((w.inner + w.outer) * 0.5, 0.0);
    drag(&ctx, &mut time, right, right, &mut hsv, WheelShape::Square);
    assert!(near(hsv[0], 90.0, 0.5), "{hsv:?}");
    assert_eq!((hsv[1], hsv[2]), (0.5, 0.6));
    // Glisser depuis le carré jusque loin dehors : la teinte ne bouge pas, le repère reste au
    // bord du carré (en haut à droite : saturée, pleine valeur).
    drag(&ctx, &mut time, w.center, pos2(395.0, 2.0), &mut hsv, WheelShape::Square);
    assert!(near(hsv[0], 90.0, 0.5));
    assert_eq!((hsv[1], hsv[2]), (1.0, 1.0));
    // Glisser depuis l'anneau jusqu'au centre et en bas : la teinte suit la direction.
    drag(&ctx, &mut time, right, w.center + vec2(0.0, 20.0), &mut hsv, WheelShape::Square);
    assert!(near(hsv[0], 180.0, 0.5), "{hsv:?}");
    assert_eq!((hsv[1], hsv[2]), (1.0, 1.0));
    // Dans le triangle : sa saturation et sa valeur.
    let target = w.triangle_point(hsv[0], 0.3, 0.8);
    drag(&ctx, &mut time, target, target, &mut hsv, WheelShape::Triangle);
    assert!(near(hsv[1], 0.3, 0.02) && near(hsv[2], 0.8, 0.02), "{hsv:?}");
}

#[test]
fn a_right_click_in_the_wheel_leaves_the_color() {
    let (ctx, mut time, w) = screen_wheel();
    let mut hsv = [10.0, 0.5, 0.6];
    let right = w.center + vec2((w.inner + w.outer) * 0.5, 0.0);
    frame(&ctx, &mut time, vec![Event::PointerMoved(right)], &mut hsv, WheelShape::Square);
    frame(&ctx, &mut time, vec![button(right, PointerButton::Secondary, true)], &mut hsv, WheelShape::Square);
    frame(&ctx, &mut time, vec![button(right, PointerButton::Secondary, false)], &mut hsv, WheelShape::Square);
    assert_eq!(hsv, [10.0, 0.5, 0.6]);
}

fn vertex_color(mesh: &eframe::egui::Mesh, p: Pos2) -> Color32 {
    mesh.vertices.iter().min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p))).unwrap().color
}

#[test]
fn meshes_carry_the_exact_colors() {
    let w = wheel();
    let hue = 200.0;
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    for mesh in [w.ring_mesh(96), w.square_mesh(hue, 24), w.triangle_mesh(hue, 24)] {
        assert!(!mesh.indices.is_empty() && mesh.indices.len() % 3 == 0);
        assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()));
    }
    let ring = w.ring_mesh(96);
    assert_eq!(ring.vertices.len(), 97 * 2);
    assert_eq!(vertex_color(&ring, w.ring_point(120.0)), Color32::from_rgb(0, 255, 0), "vert au tiers du tour");
    let square = w.square_mesh(hue, 24);
    assert_eq!(square.vertices.len(), 25 * 25);
    assert_eq!(vertex_color(&square, w.square().right_top()), rgb(hsv_to_rgb([hue, 1.0, 1.0])));
    assert_eq!(vertex_color(&square, w.square().left_top()), Color32::WHITE);
    assert_eq!(vertex_color(&square, w.square().left_bottom()), Color32::BLACK);
    assert_eq!(vertex_color(&square, w.square_point(0.5, 0.5)), rgb(hsv_to_rgb([hue, 0.5, 0.5])));
    let tri = w.triangle_mesh(hue, 24);
    assert_eq!(tri.vertices.len(), 25 * 26 / 2);
    assert_eq!(tri.indices.len() / 3, 24 * 24, "n² petits triangles");
    let [h, white, black] = w.triangle(hue);
    assert_eq!(vertex_color(&tri, h), rgb(hsv_to_rgb([hue, 1.0, 1.0])));
    assert_eq!(vertex_color(&tri, white), Color32::WHITE);
    assert_eq!(vertex_color(&tri, black), Color32::BLACK);
}
