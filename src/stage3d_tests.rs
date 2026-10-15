//! Tests du plateau en 3D (9e) : sa géométrie doit montrer ce que le rendu 2D filme.

use eframe::egui::{pos2, vec2, Rect};

use crate::camera::Camera;
use crate::document::{CameraSeen, Layer, LayerKind, LayerTransform};
use crate::stage3d::{self, Projector, World};
use crate::view3d::Cam3d;

/// Feuille plus grande que le cadre, comme dans un vrai projet.
fn world() -> World {
    World::new([2400, 1350], [1920, 1080])
}

fn screen() -> Rect {
    Rect::from_min_size(pos2(10.0, 20.0), vec2(1920.0, 1080.0))
}

#[test]
fn the_panel_camera_sees_each_plane_where_the_2d_render_puts_it() {
    let w = world();
    let c0 = pos2(1200.0, 675.0);
    for depth in [-0.5, 0.0, 0.7, 3.0] {
        for (ox, oy, dolly) in [(0.0, 0.0, 0.0), (150.0, -40.0, 0.0), (-80.0, 60.0, 300.0), (0.0, 0.0, -500.0)] {
            let cam = Camera { x: c0.x + ox, y: c0.y + oy, zoom: 1.0, angle: 0.0, dolly, tilt: 0.0, pan: 0.0 };
            let mut l = Layer::new("L", LayerKind::Bitmap);
            l.depth = depth;
            l.transform = LayerTransform { x: 30.0, y: -12.0, scale: 1.5, angle: 20.0 };
            let seen = l.transform_seen(&CameraSeen { offset: vec2(ox, oy), dolly, d0: w.d0 }).unwrap();
            for p in [pos2(100.0, 200.0), c0, pos2(2000.0, 1100.0)] {
                // Le point du dessin, posé sur son plan, vu par la caméra et ramené sur le plan de
                // référence : là où le rendu 2D (multiplan) l'affiche.
                let on_plane = w.at_depth(l.transform.affine(c0).apply(p), depth);
                let got = w.seen_on_reference(&cam, on_plane).unwrap();
                let want = seen.affine(c0).apply(p);
                assert!(got.distance(want) < 0.05, "profondeur {depth}, caméra ({ox}, {oy}, {dolly}) : {got:?} ≠ {want:?}");
            }
        }
    }
}

#[test]
fn looking_through_the_camera_frames_the_shot() {
    let w = world();
    // Avec bascule et panoramique (9f) : la vue 3D (repère calculé à part, `Cam3d::basis`) voit
    // les coins du cadre filmé — un trapèze sur la feuille — aux coins de l'écran.
    for (zoom, dolly, tilt, pan) in
        [(1.0, 0.0, 0.0, 0.0), (2.0, 0.0, 0.0, 0.0), (1.0, 400.0, 0.0, 0.0), (1.5, -300.0, 0.0, 0.0), (1.0, 0.0, 15.0, 0.0), (1.0, 0.0, 0.0, -30.0), (1.3, 200.0, -12.0, 25.0)]
    {
        let cam = Camera { x: 1300.0, y: 600.0, zoom, angle: 0.0, dolly, tilt, pan };
        let rect = screen();
        let proj = Projector::new(&w.look_through(&cam), rect);
        let corners = w.frame_corners(&cam);
        for (c, r) in corners.iter().zip([rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]) {
            let s = proj.project(*c).unwrap();
            assert!(s.distance(r) < 0.5, "zoom {zoom}, avancée {dolly}, bascule {tilt}, panoramique {pan} : {s:?} ≠ {r:?}");
        }
    }
}

#[test]
fn picking_takes_the_nearest_drawing_under_the_pointer() {
    let w = world();
    let cam = Camera { x: 1200.0, y: 675.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
    let rect = screen();
    let proj = Projector::new(&w.look_through(&cam), rect);
    let (o, d) = proj.ray(rect.center());
    let planes = [(0, 2.0), (1, 0.0), (2, -0.5)];
    // Dessin partout : le premier plan l'emporte.
    assert_eq!(stage3d::pick(&w, o, d, &planes, |_, _| true), Some(2));
    // Rien de dessiné là sur le premier plan : le suivant, touché au centre de sa feuille.
    let found = stage3d::pick(&w, o, d, &planes, |i, p| {
        if i == 1 {
            assert!(p.distance(pos2(1200.0, 675.0)) < 0.01, "{p:?}");
        }
        i != 2
    });
    assert_eq!(found, Some(1));
    // Rien de dessiné nulle part : la feuille la plus proche.
    assert_eq!(stage3d::pick(&w, o, d, &planes, |_, _| false), Some(2));
    // Hors de toutes les feuilles : rien.
    let in_sheet = |_: usize, p: eframe::egui::Pos2| (0.0..2400.0).contains(&p.x) && (0.0..1350.0).contains(&p.y);
    assert_eq!(stage3d::pick(&w, [9000.0, 0.0, 1000.0], [0.0, 0.0, -1.0], &planes, in_sheet), None);
}

#[test]
fn moving_in_a_plane_and_in_depth() {
    let w = world();
    // Dans son plan : un plan lointain est agrandi, le même geste le décale moins sur la feuille.
    let a = w.at_depth(pos2(1000.0, 500.0), 1.0);
    let b = w.at_depth(pos2(1100.0, 450.0), 1.0);
    let moved = w.stage_delta(a, b, 1.0);
    assert!((moved - vec2(100.0, -50.0)).length() < 1e-3, "{moved:?}");
    for d in [-0.5, 0.0, 2.5] {
        assert!((w.depth_of(w.plane_z(d)) - d).abs() < 1e-5);
    }
    assert_eq!(w.depth_of(w.d0 * 5.0), Layer::MIN_DEPTH, "jamais derrière la caméra");

    // En profondeur, vu de côté : la souris suit le point le long de l'axe.
    let rect = screen();
    let side = Cam3d { target: [0.0, 0.0, -w.d0], azimuth: 90.0, elevation: 0.0, distance: 5.0 * w.d0, focal: 35.0 };
    let proj = Projector::new(&side, rect);
    let at = [0.0, 0.0, -w.d0];
    let s = proj.project(at).unwrap();
    let dz = stage3d::z_motion(&proj, at, s, s + vec2(100.0, 0.0));
    assert!(dz < 0.0, "vers la droite, vu de ce côté : vers le fond");
    let there = proj.project([0.0, 0.0, -w.d0 + dz]).unwrap();
    assert!(there.distance(s + vec2(100.0, 0.0)) < 0.5, "{there:?}");
    // Vu dans l'axe : monter éloigne.
    let front = Cam3d { target: [0.0, 0.0, 0.0], azimuth: 0.0, elevation: 0.0, distance: 3.0 * w.d0, focal: 35.0 };
    let proj = Projector::new(&front, rect);
    let s = proj.project([0.0, 0.0, 0.0]).unwrap();
    assert!(stage3d::z_motion(&proj, [0.0, 0.0, 0.0], s, s - vec2(0.0, 50.0)) < 0.0);
}

#[test]
fn segments_behind_the_eye_are_cut() {
    let front = Cam3d { target: [0.0, 0.0, 0.0], azimuth: 0.0, elevation: 0.0, distance: 1000.0, focal: 35.0 };
    let proj = Projector::new(&front, screen());
    // Œil en z = 1000 : un segment qui passe derrière lui est coupé, un segment tout derrière disparaît.
    let [a, b] = proj.segment([0.0, 100.0, 0.0], [0.0, 100.0, 5000.0]).unwrap();
    assert!(a.is_finite() && b.is_finite());
    assert!(proj.segment([0.0, 0.0, 2000.0], [10.0, 0.0, 3000.0]).is_none());
    assert!(proj.project([0.0, 0.0, 1500.0]).is_none());
}

#[test]
fn grid_steps_are_round() {
    assert_eq!(stage3d::grid_step(1200.0), 100.0);
    assert_eq!(stage3d::grid_step(1300.0), 200.0);
    assert_eq!(stage3d::grid_step(5000.0), 500.0);
    assert_eq!(stage3d::grid_step(3.0), 1.0);
    assert_eq!(stage3d::bounds(std::iter::empty()), ([0.0; 3], [0.0; 3]));
}
