//! Essais des maths de l'Espace 3D (`espace3dmath.rs`, `espace3dgizmo.rs`, `espace3d.rs`) : placements, rayons, manipulateurs
//! (déplacer, tourner, échelle), aimantation, caméra du plan comme prise de vue, pyramide de la caméra, caméra d'orbite.

use eframe::egui::pos2;

use crate::camera::Camera;
use crate::espace3d::{SceneCam, Xf};
use crate::espace3dgizmo::{self as gz, Gizmo, Handle, Mode, CAMERA_ID};
use crate::espace3dmath::{self as m, Orbite, Ray, V3, View};

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

fn near3(a: V3, b: V3, eps: f32) -> bool {
    (0..3).all(|k| near(a[k], b[k], eps))
}
#[test]
fn a_placement_matrix_round_trips_through_its_inverse_and_its_euler_angles() {
    let xf = Xf { pos: [1.5, -2.0, 0.7], rot: [20.0, 130.0, -35.0], size: [2.0, 0.5, 3.0] };
    let mat = xf.matrix();
    let inv = m::inverse_affine(&mat);
    for p in [[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [-4.0, 0.5, 9.0]] {
        assert!(near3(m::point(&inv, m::point(&mat, p)), p, 1e-4));
    }
    // Les angles retrouvés donnent la même rotation.
    for rot in [[0.0, 0.0, 0.0], [20.0, 130.0, -35.0], [-70.0, -10.0, 170.0], [89.0, 0.0, 0.0], [10.0, 180.0, 5.0]] {
        let r = m::rotation(rot);
        let back = m::rotation(m::euler_of(&r));
        for k in 0..16 {
            assert!(near(r[k], back[k], 1e-3), "rotation {rot:?} : {:?} contre {:?}", m::euler_of(&r), rot);
        }
    }
    // Rotation d'un quart de tour autour de Y : +X part vers −Z.
    assert!(near3(m::dir(&m::rot_axis([0.0, 1.0, 0.0], 90.0), [1.0, 0.0, 0.0]), [0.0, 0.0, -1.0], 1e-5));
    assert!(near3(m::dir(&m::rot_y(90.0), [1.0, 0.0, 0.0]), [0.0, 0.0, -1.0], 1e-5), "rot_y et rot_axis disent la même chose");
}

// ----------------------------------------------------------------------- manipulateurs

fn side_view() -> (View, [f32; 2]) {
    let cam = SceneCam { pos: [0.0, 0.0, 10.0], pan: 0.0, tilt: 0.0, roll: 0.0, focal: 35.0 };
    (cam.view(800.0), [400.0, 300.0])
}

fn ray_through(view: &View, center: [f32; 2], p: V3) -> Ray {
    view.ray(view.project(p, center).expect("devant l'œil"), center)
}

#[test]
fn dragging_a_handle_moves_rotates_and_scales_by_the_amount_the_mouse_went() {
    let (v, c) = side_view();
    let origin = [0.0; 3];
    // Déplacer le long de X : la souris passe de l'origine au point (2, 0, 0).
    let d = m::drag_along_axis(&ray_through(&v, c, origin), &ray_through(&v, c, [2.0, 0.0, 0.0]), origin, [1.0, 0.0, 0.0]);
    assert!(near3(d, [2.0, 0.0, 0.0], 1e-3), "{d:?}");
    // Le long d'un axe, seul le point de l'axe le plus proche du rayon compte : un petit écart de la
    // souris hors de l'axe ne le change presque pas, et le déplacement reste sur l'axe.
    let d = m::drag_along_axis(&ray_through(&v, c, origin), &ray_through(&v, c, [2.0, 0.5, 0.0]), origin, [1.0, 0.0, 0.0]);
    assert!(near(d[0], 2.0, 0.02) && d[1] == 0.0 && d[2] == 0.0, "{d:?}");
    // Dans un plan.
    let d = m::drag_in_plane(&ray_through(&v, c, origin), &ray_through(&v, c, [1.0, -2.0, 0.0]), origin, [0.0, 0.0, 1.0]);
    assert!(near3(d, [1.0, -2.0, 0.0], 1e-3), "{d:?}");
    // Tourner autour de Z : de +X à +Y, un quart de tour direct.
    let a = m::drag_angle(&ray_through(&v, c, [1.0, 0.0, 0.0]), &ray_through(&v, c, [0.0, 1.0, 0.0]), origin, [0.0, 0.0, 1.0]);
    assert!(near(a, 90.0, 0.05), "{a}");
    let a = m::drag_angle(&ray_through(&v, c, [1.0, 0.0, 0.0]), &ray_through(&v, c, [1.0, -1.0, 0.0]), origin, [0.0, 0.0, 1.0]);
    assert!(near(a, -45.0, 0.05), "{a}");
    // Échelle : deux fois plus loin de l'origine, deux fois plus grand.
    let f = m::drag_scale_axis(&ray_through(&v, c, [1.0, 0.0, 0.0]), &ray_through(&v, c, [2.0, 0.0, 0.0]), origin, [1.0, 0.0, 0.0]);
    assert!(near(f, 2.0, 1e-3), "{f}");
    assert!((0.01..=100.0).contains(&m::scale_from_screen(10_000.0)) && m::scale_from_screen(-50.0) > 1.0, "glisser vers le haut agrandit");
}

#[test]
fn gizmo_gestures_apply_to_every_chosen_object() {
    let a = Xf { pos: [1.0, 0.0, 0.0], rot: [0.0; 3], size: [1.0; 3] };
    let b = Xf { pos: [-1.0, 0.0, 2.0], rot: [0.0, 30.0, 0.0], size: [2.0, 1.0, 1.0] };
    let both = vec![(1, a), (2, b)];
    let moved = gz::move_xfs(&both, [0.0, 1.0, 0.0]);
    assert!(near3(moved[0].1.pos, [1.0, 1.0, 0.0], 1e-6) && near3(moved[1].1.pos, [-1.0, 1.0, 2.0], 1e-6));
    assert_eq!(moved[1].1.rot, b.rot, "déplacer ne tourne pas");
    // Un quart de tour autour de Y, pivot à l'origine : les positions tournent, les objets aussi.
    let turned = gz::rotate_xfs(&both, [0.0; 3], [0.0, 1.0, 0.0], 90.0);
    assert!(near3(turned[0].1.pos, [0.0, 0.0, -1.0], 1e-5), "{:?}", turned[0].1.pos);
    assert!(near(turned[0].1.rot[1], 90.0, 1e-3) && near(turned[1].1.rot[1], 120.0, 1e-3), "{:?} {:?}", turned[0].1.rot, turned[1].1.rot);
    // Échelle d'un objet seul sur son axe, puis de tous ensemble autour d'un pivot.
    let one = gz::scale_xfs(&both[..1], [0.0; 3], Some(2), 3.0);
    assert!(near3(one[0].1.size, [1.0, 1.0, 3.0], 1e-6) && one[0].1.pos == a.pos);
    let all = gz::scale_xfs(&both, [1.0, 0.0, 0.0], None, 2.0);
    assert!(near3(all[0].1.pos, [1.0, 0.0, 0.0], 1e-6) && near3(all[1].1.pos, [-3.0, 0.0, 4.0], 1e-6) && near3(all[1].1.size, [4.0, 2.0, 2.0], 1e-6));
    assert!(near3(gz::pivot_of(&both), [0.0, 0.0, 1.0], 1e-6), "pivot : centre de la boîte des positions");
}

#[test]
fn gizmo_handles_are_found_at_the_pixel_where_they_are_drawn() {
    let (v, c) = side_view();
    let g = Gizmo { origin: [0.0; 3], axes: gz::axes_for(false, &Xf::default()), len: Gizmo::size_for(&v, [0.0; 3]), mode: Mode::Move };
    for k in 0..3 {
        let mid = m::scale(g.axes[k], g.len * 0.8);
        let px = v.project(mid, c).expect("visible");
        // (Z pointe vers l'œil : sa flèche est un point, que la poignée centrale emporte.)
        if k < 2 {
            assert_eq!(g.hit(&v, c, px, 6.0), Some(Handle::Axis(k)), "axe {k}");
        }
        assert_eq!(g.hit(&v, c, [px[0] + 300.0, px[1] + 300.0], 6.0), None);
    }
    // Un carré de plan se prend en dedans.
    let q = g.plane_quad(2);
    let inside = m::scale(m::add(q[0], q[2]), 0.5);
    assert_eq!(g.hit(&v, c, v.project(inside, c).expect("visible"), 6.0), Some(Handle::Plane(2)));
    // Tourner : les cercles ; échelle : cubes et centre.
    let r = Gizmo { mode: Mode::Rotate, ..g };
    let on_ring = r.ring_points(2, 48)[6];
    assert_eq!(r.hit(&v, c, v.project(on_ring, c).expect("visible"), 6.0), Some(Handle::Ring(2)));
    let s = Gizmo { mode: Mode::Scale, ..g };
    assert_eq!(s.hit(&v, c, c, 6.0), Some(Handle::Center));
    assert_eq!(Gizmo { mode: Mode::Select, ..g }.hit(&v, c, c, 6.0), None);
    // Local : les axes suivent l'objet.
    let tilted = Xf { rot: [0.0, 90.0, 0.0], ..Xf::default() };
    assert!(near3(gz::axes_for(true, &tilted)[0], [0.0, 0.0, -1.0], 1e-5) && gz::axes_for(false, &tilted)[0] == [1.0, 0.0, 0.0]);
}

#[test]
fn snapping_rounds_to_the_grid_and_the_steps() {
    assert!(near(m::snap(0.37, 0.25), 0.25, 1e-6) && near(m::snap(0.38, 0.25), 0.5, 1e-6) && near(m::snap(-0.4, 0.25), -0.5, 1e-6));
    assert_eq!(m::snap(1.234, 0.0), 1.234, "pas nul : inchangé");
    assert!(near(m::snap(52.0, 15.0), 45.0, 1e-5), "rotation par paliers de 15°");
    // Seuls les axes bougés sont ramenés à la grille.
    let p = m::snap_position([0.37, 0.61, 0.9], 0.25, [true, false, true]);
    assert!(near3(p, [0.25, 0.61, 1.0], 1e-6), "{p:?}");
}

#[test]
fn the_camera_is_moved_by_the_same_handles_as_an_object() {
    let cam = SceneCam { pos: [1.0, 2.0, 6.0], pan: 20.0, tilt: -10.0, roll: 5.0, focal: 50.0 };
    let back = gz::cam_from_xf(&cam, &gz::cam_xf(&cam));
    assert!(near3(back.pos, cam.pos, 1e-5) && near(back.pan, 20.0, 1e-4) && near(back.tilt, -10.0, 1e-4) && near(back.roll, 5.0, 1e-4) && back.focal == 50.0);
    // Tourner la caméra de 30° autour de la verticale du monde tourne sa visée de 30°.
    let turned = gz::rotate_xfs(&[(CAMERA_ID, gz::cam_xf(&cam))], cam.pos, [0.0, 1.0, 0.0], 30.0);
    let after = gz::cam_from_xf(&cam, &turned[0].1);
    let want = m::dir(&m::rot_axis([0.0, 1.0, 0.0], 30.0), cam.forward());
    assert!(near3(after.forward(), want, 1e-4), "{:?} contre {want:?}", after.forward());
    // Une caméra penchée vers le haut ne passe pas la verticale.
    assert!(SceneCam { tilt: 400.0, ..cam }.sanitized().tilt <= crate::espace3d::MAX_TILT);
}

// ------------------------------------------------------------------------------- caméra

#[test]
fn the_scene_camera_sees_the_frame_like_the_2d_camera_does() {
    // Le plan z = 0 de la scène est la feuille : les coins du cadre vus par la caméra du plateau,
    // posés sur ce plan, retombent sur les coins que la caméra 2D filme sur la feuille.
    let stage = [1280usize, 720];
    let frame = [640usize, 360];
    let k = SceneCam::meters_per_px(frame[0]);
    let cases = [
        Camera { x: 640.0, y: 360.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 },
        Camera { x: 700.0, y: 330.0, zoom: 2.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 },
        Camera { x: 600.0, y: 400.0, zoom: 1.0, angle: 0.0, dolly: 250.0, tilt: 0.0, pan: 0.0 },
        Camera { x: 640.0, y: 360.0, zoom: 1.5, angle: 20.0, dolly: -200.0, tilt: 0.0, pan: 0.0 },
        Camera { x: 640.0, y: 360.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 12.0, pan: 0.0 },
        Camera { x: 640.0, y: 360.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: -25.0 },
        Camera { x: 660.0, y: 340.0, zoom: 1.3, angle: -15.0, dolly: 120.0, tilt: -9.0, pan: 18.0 },
    ];
    for cam in cases {
        let sc = SceneCam::from_plan_camera(&cam, stage, frame);
        assert!(near(sc.focal, cam.focal(), 1e-3), "même focale");
        let view = sc.view(frame[0] as f32);
        let want = cam.corners(frame);
        let (w, h) = (frame[0] as f32, frame[1] as f32);
        let center = [w / 2.0, h / 2.0];
        for (i, px) in [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]].into_iter().enumerate() {
            let ray = view.ray(px, center);
            let t = m::ray_plane(&ray, [0.0; 3], [0.0, 0.0, 1.0]).expect("le rayon rencontre la feuille");
            let p = ray.at(t);
            let on_stage = pos2(p[0] / k + stage[0] as f32 / 2.0, -p[1] / k + stage[1] as f32 / 2.0);
            assert!(on_stage.distance(want[i]) < 0.6, "caméra {cam:?}, coin {i} : {on_stage:?} contre {:?}", want[i]);
        }
    }
}

#[test]
fn the_frustum_follows_the_lens_and_the_frame_proportions() {
    for focal in [14.0f32, 35.0, 50.0, 135.0] {
        let cam = SceneCam { focal, ..SceneCam::default() };
        let frame = [1920usize, 1080];
        let d = 4.0;
        let c = cam.corners_at(d, frame);
        let width = m::length(m::sub(c[1], c[0]));
        let height = m::length(m::sub(c[3], c[0]));
        assert!(near(width, d * 36.0 / focal, 1e-3), "largeur à {d} m avec {focal} mm : {width}");
        assert!(near(height / width, 1080.0 / 1920.0, 1e-4), "proportions du cadre");
        // L'angle de champ horizontal est celui de la caméra 2D.
        let half = (width / 2.0 / d).atan().to_degrees() * 2.0;
        assert!(near(half, crate::camera::angle_of_view(focal), 0.01) && near(cam.angle_of_view(), half, 0.01));
        // Les coins sont devant la caméra et se projettent sur ceux de l'image.
        let v = cam.view(frame[0] as f32);
        let ctr = [960.0, 540.0];
        for (p, want) in c.iter().zip([[0.0, 0.0], [1920.0, 0.0], [1920.0, 1080.0], [0.0, 1080.0]]) {
            let px = v.project(*p, ctr).expect("devant");
            assert!(near(px[0], want[0], 0.05) && near(px[1], want[1], 0.05), "{px:?} contre {want:?}");
        }
    }
    // Zoom 2 de la caméra 2D = focale doublée = moitié moins de largeur filmée.
    let w35 = SceneCam { focal: 35.0, ..SceneCam::default() }.corners_at(5.0, [100, 100]);
    let w70 = SceneCam { focal: 70.0, ..SceneCam::default() }.corners_at(5.0, [100, 100]);
    assert!(near(m::length(m::sub(w35[1], w35[0])) / 2.0, m::length(m::sub(w70[1], w70[0])), 1e-4));
    // Les objectifs proposés par le plan sont ceux de la caméra du plateau.
    for p in crate::camera::FOCAL_PRESETS.iter().filter(|p| !p.anamorphic) {
        assert!((crate::espace3d::MIN_FOCAL..=crate::espace3d::MAX_FOCAL).contains(&p.mm));
    }
}

#[test]
fn a_view_projects_and_casts_rays_consistently() {
    let cam = SceneCam { pos: [1.0, 2.0, 7.0], pan: 15.0, tilt: -8.0, roll: 10.0, focal: 28.0 };
    let v = cam.view(1000.0);
    let c = [500.0, 300.0];
    for p in [[0.0, 0.0, 0.0], [2.0, 1.0, -3.0], [-1.0, 3.0, 1.0]] {
        let px = v.project(p, c).expect("devant");
        let ray = v.ray(px, c);
        let t = m::dot(m::sub(p, ray.o), ray.d);
        assert!(near3(ray.at(t), p, 1e-3), "le rayon du pixel passe par le point");
    }
    assert!(v.project([0.0, 0.0, 20.0], c).is_none(), "derrière la caméra : rien");
}


#[test]
fn the_orbit_camera_turns_around_its_target_and_fits_a_box() {
    let mut cam = Orbite { target: [1.0, 0.5, -2.0], azimuth: 0.0, elevation: 0.0, distance: 8.0, focal: 35.0 };
    // De face : l'œil est devant la cible (+Z), la vue regarde vers −Z.
    assert!(near3(cam.eye(), [1.0, 0.5, 6.0], 1e-4));
    let (right, up, forward) = cam.basis();
    assert!(near3(right, [1.0, 0.0, 0.0], 1e-5) && near3(up, [0.0, 1.0, 0.0], 1e-5) && near3(forward, [0.0, 0.0, -1.0], 1e-5));
    // Tourner de 90° : l'œil passe à droite, toujours à 8 m de la cible, tourné vers elle.
    cam.orbit(90.0, 0.0);
    assert!(near3(cam.eye(), [9.0, 0.5, -2.0], 1e-3), "{:?}", cam.eye());
    assert!(near(m::length(m::sub(cam.eye(), cam.target)), 8.0, 1e-4));
    // La hauteur de regard ne passe pas la verticale.
    cam.orbit(0.0, 500.0);
    assert!(cam.elevation <= crate::espace3dmath::MAX_ELEVATION);
    // Glisser la vue déplace la cible ; avancer rapproche l'œil.
    let before = cam.target;
    cam.pan(30.0, 0.0, 800.0);
    assert!(m::length(m::sub(cam.target, before)) > 1e-3);
    let d = cam.distance;
    cam.dolly(0.5);
    assert!(near(cam.distance, d * 0.5, 1e-5));
    // Le cadrage tient les 8 coins de la boîte dans la zone.
    let bounds = ([-3.0, 0.0, -3.0], [3.0, 2.0, 3.0]);
    let fit = Orbite::fit_at(bounds, [800.0, 600.0], 30.0, 22.0);
    let view = fit.view(800.0);
    for i in 0..8 {
        let corner = [if i & 1 == 0 { -3.0 } else { 3.0 }, if i & 2 == 0 { 0.0 } else { 2.0 }, if i & 4 == 0 { -3.0 } else { 3.0 }];
        let p = view.project(corner, [400.0, 300.0]).expect("devant l'œil");
        assert!((0.0..800.0).contains(&p[0]) && (0.0..600.0).contains(&p[1]), "le coin {corner:?} sort de la zone : {p:?}");
    }
}

#[test]
fn a_ray_meets_a_plane_and_a_card_is_picked_where_it_is_drawn() {
    let ray = Ray::new([0.0, 0.0, 5.0], [0.0, 0.0, -1.0]);
    assert!(near(m::ray_plane(&ray, [0.0; 3], [0.0, 0.0, 1.0]).expect("touche"), 5.0, 1e-5));
    assert!(m::ray_plane(&Ray::new([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]), [0.0; 3], [0.0, 0.0, 1.0]).is_none(), "le plan est derrière");
    assert!(m::ray_plane(&Ray::new([0.0, 0.0, 5.0], [1.0, 0.0, 0.0]), [0.0; 3], [0.0, 0.0, 1.0]).is_none(), "parallèle");
    let world = crate::espace3d::Monde::new([320, 180], [320, 180]);
    let xf = world.card_xf(&crate::document::LayerTransform::default(), 0.0);
    let (t, p) = world.pick_card(&xf, &ray).expect("la carte est touchée au centre");
    assert!(near(t, 5.0, 1e-4) && near(p.x, 160.0, 0.05) && near(p.y, 90.0, 0.05), "{t} {p:?}");
    assert!(world.pick_card(&xf, &Ray::new([4.0, 0.0, 5.0], [0.0, 0.0, -1.0])).is_none(), "hors de la carte");
    let _ = pos2(0.0, 0.0);
}