//! Essais de la scène 3D de blocage : maillages des formes, mannequin, maths et manipulateurs,
//! aimantation, caméra du plateau (cadrage et projection comme la caméra 2D), rendu vers un calque
//! (taille, alignement), enregistrement (aller-retour et protections), annulation, et l'application
//! sans fenêtre (envoi vers un calque en arrière-plan, repos, carte graphique si la machine en a une).

use std::collections::HashMap;

use eframe::egui::{pos2, vec2, Rect};

use crate::camera::Camera;
use crate::document::Document;
use crate::perf_tests::{banc, banc_taille, se_reposer};
use crate::scene3d::{joint, Light, Mannequin, Pose, RenderArea, RenderSettings, RenderStyle, SceneCam, Scenes3d, Shape3d, Target, Xf, JOINTS};
use crate::scene3dgizmo::{self as gz, Gizmo, Handle, Mode, CAMERA_ID};
use crate::scene3dmath::{self as m, Ray, V3, View};
use crate::scene3dmesh::{self as mesh, SolidMesh, SEGMENTS, SPHERE_RINGS};
use crate::scene3drender::{self as render, Item, Opts};

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

fn near3(a: V3, b: V3, eps: f32) -> bool {
    (0..3).all(|k| near(a[k], b[k], eps))
}

// ------------------------------------------------------------------------------ maillages

/// Normales unitaires, indices valides, faces tournées du côté de leurs normales.
fn assert_sound(name: &str, s: &SolidMesh) {
    assert_eq!(s.positions.len(), s.normals.len(), "{name}");
    assert_eq!(s.positions.len(), s.outline.len(), "{name}");
    assert!(s.indices.len() % 3 == 0 && s.indices.iter().all(|&i| (i as usize) < s.positions.len()), "{name} : indices");
    for n in s.normals.iter().chain(&s.outline) {
        assert!(near(m::length(*n), 1.0, 1e-3), "{name} : normale non unitaire {n:?}");
    }
    for t in s.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| s.positions[i as usize]);
        let g = m::cross(m::sub(b, a), m::sub(c, a));
        if m::length(g) < 1e-9 {
            continue;
        }
        let avg = t.iter().fold([0.0; 3], |acc, &i| m::add(acc, s.normals[i as usize]));
        assert!(m::dot(g, avg) > 0.0, "{name} : une face est tournée à l'envers de sa normale");
    }
}

#[test]
fn simple_shapes_have_the_expected_vertex_counts_and_unit_normals() {
    let n = SEGMENTS;
    let cases: Vec<(&str, SolidMesh, usize, usize)> = vec![
        ("cube", mesh::cube(), 24, 12),
        ("plan", mesh::plane(), 4, 2),
        ("cylindre", mesh::cylinder(), 4 * n + 2, 4 * n),
        ("cône", mesh::cone(), 3 * n + 1, 2 * n),
        ("sphère", mesh::sphere(), (SPHERE_RINGS + 1) * (n + 1), 2 * n * (SPHERE_RINGS - 1)),
        ("escalier de 5", mesh::stairs(5), 16 * 5 + 8, 8 * 5 + 4),
        ("escalier de 12", mesh::stairs(12), 16 * 12 + 8, 8 * 12 + 4),
    ];
    for (name, s, vertices, triangles) in cases {
        assert_eq!(s.positions.len(), vertices, "{name} : sommets");
        assert_eq!(s.triangle_count(), triangles, "{name} : triangles");
        assert_sound(name, &s);
    }
}

#[test]
fn shapes_have_their_documented_size_and_outward_normals() {
    let c = mesh::cube();
    assert_eq!(c.bounds, ([-0.5; 3], [0.5; 3]));
    assert_eq!(c.edges.len(), 12);
    for (p, n) in c.positions.iter().zip(&c.normals) {
        assert!(m::dot(*p, *n) > 0.49, "la normale du cube regarde vers l'extérieur");
        assert!(n.iter().filter(|v| v.abs() > 0.99).count() == 1, "une normale de cube suit un axe");
    }
    let p = mesh::plane();
    assert!(p.normals.iter().all(|n| *n == [0.0, 1.0, 0.0]) && p.positions.iter().all(|v| v[1] == 0.0));
    assert_eq!(p.edges.len(), 4);
    let s = mesh::sphere();
    for (v, n) in s.positions.iter().zip(&s.normals) {
        assert!(near(m::length(*v), 0.5, 1e-4), "la sphère a 1 m de diamètre");
        assert!(near3(m::scale(*v, 2.0), *n, 1e-4), "la normale d'une sphère est radiale");
    }
    let cy = mesh::cylinder();
    assert!(near3(cy.bounds.0, [-0.5, -0.5, -0.5], 1e-4) && near3(cy.bounds.1, [0.5, 0.5, 0.5], 1e-4));
    assert!(cy.normals.iter().all(|n| n[1].abs() < 1e-4 || n[0].abs() + n[2].abs() < 1e-4), "flanc horizontal, disques verticaux");
    let co = mesh::cone();
    assert!(near(co.bounds.1[1], 0.5, 1e-5) && near(co.bounds.0[1], -0.5, 1e-5));
    assert!(co.positions.iter().filter(|p| p[1] > 0.49).all(|p| p[0].abs() < 1e-5 && p[2].abs() < 1e-5), "une seule pointe");
    // Escalier : la marche la plus haute est au fond (−Z) et touche le haut de la boîte.
    let st = mesh::stairs(5);
    assert_eq!(st.bounds, ([-0.5; 3], [0.5; 3]));
    let tread_y: Vec<f32> = st.positions.iter().zip(&st.normals).filter(|(_, n)| **n == [0.0, 1.0, 0.0]).map(|(p, _)| p[1]).collect();
    assert!(tread_y.iter().any(|y| near(*y, -0.5 + 1.0 / 5.0, 1e-5)) && tread_y.iter().any(|y| near(*y, 0.5, 1e-5)));
    let top_z: Vec<f32> = st.positions.iter().zip(&st.normals).filter(|(p, n)| **n == [0.0, 1.0, 0.0] && near(p[1], 0.5, 1e-5)).map(|(p, _)| p[2]).collect();
    assert!(top_z.iter().all(|z| *z <= -0.5 + 0.2 + 1e-5), "la marche la plus haute est au fond");
    // Le nombre de marches est borné.
    assert_eq!(mesh::stairs(0).positions.len(), 16 * 2 + 8);
}

#[test]
fn a_mesh_is_found_again_by_its_shape_and_a_pose_change_makes_a_new_one() {
    let models = HashMap::new();
    let mut lib = mesh::MeshLib::default();
    let (k1, a) = lib.get(&Shape3d::Cube, &models);
    let (k2, b) = lib.get(&Shape3d::Cube, &models);
    assert_eq!(k1, k2);
    assert!(std::sync::Arc::ptr_eq(&a, &b), "la même forme ne se refait pas");
    let standing = Shape3d::Mannequin(Box::new(Mannequin::new(Pose::Standing, 1.75)));
    let mut walking = Mannequin::new(Pose::Standing, 1.75);
    walking.set_pose(Pose::Walking);
    assert_ne!(mesh::key_of(&standing), mesh::key_of(&Shape3d::Mannequin(Box::new(walking))));
    assert_ne!(mesh::key_of(&Shape3d::Stairs { steps: 4 }), mesh::key_of(&Shape3d::Stairs { steps: 5 }));
}

// ------------------------------------------------------------------------------- mannequin

// Ordre des capsules (voir `scene3dmesh::raw_capsules`).
const PELVIS: usize = 0;
const HEAD: usize = 4;
const THIGH_L: usize = 11;
const SHIN_L: usize = 12;
const HAND_L: usize = 7;
const SHIN_R: usize = 15;

fn extent(caps: &[mesh::Capsule]) -> (f32, f32) {
    caps.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.a[1].min(c.b[1]) - c.r), hi.max(c.a[1].max(c.b[1]) + c.r)))
}

#[test]
fn the_standing_mannequin_has_the_asked_height_and_stands_on_the_floor() {
    for h in [0.5f32, 1.0, 1.75, 2.6] {
        let mq = Mannequin::new(Pose::Standing, h);
        let s = mesh::mannequin(&mq);
        assert_sound("mannequin", &s);
        let (lo, hi) = s.bounds;
        // (Le bas du maillage est un peu plus haut que celui de la capsule exacte : les sommets d'une
        // demi-sphère inclinée ne tombent pas pile sur son point le plus bas.)
        assert!(near(lo[1], 0.0, 0.006 * h), "les pieds sont au sol (taille {h}) : {}", lo[1]);
        assert!(near(hi[1] - lo[1], h, h * 0.005), "taille {h} : {}", hi[1] - lo[1]);
        assert!(near(lo[0], -hi[0], 0.01 * h), "gauche et droite sont symétriques");
        assert!(hi[2] > 0.0 && lo[2] < 0.0, "les pieds avancent vers +Z");
    }
}

#[test]
fn every_preset_pose_keeps_the_feet_on_the_floor_and_changes_the_silhouette() {
    let h = 1.8;
    let stand = mesh::mannequin_capsules(&Mannequin::new(Pose::Standing, h));
    assert!(near(extent(&stand).0, 0.0, 1e-4));
    let mut keys = Vec::new();
    for pose in Pose::PRESETS {
        let mq = Mannequin::new(pose, h);
        let caps = mesh::mannequin_capsules(&mq);
        assert!(near(extent(&caps).0, 0.0, 1e-4), "{} : pieds au sol", pose.label());
        keys.push(mesh::key_of(&Shape3d::Mannequin(Box::new(mq))));
    }
    keys.sort_by_key(|k| format!("{k:?}"));
    keys.dedup();
    assert_eq!(keys.len(), Pose::PRESETS.len(), "quatre poses, quatre silhouettes");
}

#[test]
fn the_sitting_mannequin_is_lower_with_thighs_forward_and_shins_down() {
    let h = 1.8;
    let stand = mesh::mannequin_capsules(&Mannequin::new(Pose::Standing, h));
    let sit = mesh::mannequin_capsules(&Mannequin::new(Pose::Sitting, h));
    assert!(sit[PELVIS].a[1] < stand[PELVIS].a[1] - 0.2 * h, "assis, le bassin descend");
    let (hip, knee, ankle) = (sit[THIGH_L].a, sit[THIGH_L].b, sit[SHIN_L].b);
    assert!(knee[2] - hip[2] > 0.2 * h * 0.9, "la cuisse part vers l'avant : {hip:?} {knee:?}");
    assert!(near(knee[1], hip[1], 0.03 * h), "la cuisse est horizontale");
    assert!(near(ankle[2], knee[2], 0.03 * h) && ankle[1] < knee[1] - 0.2 * h * 0.9, "la jambe pend à la verticale : {knee:?} {ankle:?}");
}

#[test]
fn raised_arms_go_above_the_head_and_walking_spreads_the_legs() {
    let h = 1.8;
    let up = mesh::mannequin_capsules(&Mannequin::new(Pose::ArmsUp, h));
    assert!(up[HAND_L].b[1] > up[HEAD].a[1], "mains au-dessus de la tête");
    let stand = mesh::mannequin_capsules(&Mannequin::new(Pose::Standing, h));
    assert!(stand[HAND_L].b[1] < stand[HEAD].a[1] - 0.3 * h, "debout, les mains pendent");
    let walk = mesh::mannequin_capsules(&Mannequin::new(Pose::Walking, h));
    let (l, r) = (walk[SHIN_L].b, walk[SHIN_R].b);
    assert!((l[2] - r[2]).abs() > 0.1 * h, "pieds écartés d'avant en arrière : {l:?} {r:?}");
    assert!(l[2] * r[2] < 0.0 || (l[2] - r[2]).abs() > 0.2 * h, "un pied devant, l'autre derrière");
}

#[test]
fn a_joint_rotation_moves_only_what_hangs_from_it() {
    let mut mq = Mannequin::new(Pose::Standing, 1.8);
    let before = mesh::mannequin_capsules(&mq);
    mq.set_joint(joint::KNEE_L, [-60.0, 0.0, 0.0]);
    assert_eq!(mq.pose, Pose::Custom, "toucher une articulation rend la pose libre");
    let after = mesh::mannequin_capsules(&mq);
    // Le genou plié recule le pied gauche ; le droit, la tête et les cuisses ne bougent pas.
    assert!(after[SHIN_L].b[2] < before[SHIN_L].b[2] - 0.1, "le pied part en arrière");
    assert!(near3(after[SHIN_R].b, before[SHIN_R].b, 1e-4));
    assert!(near3(after[THIGH_L].b, before[THIGH_L].b, 1e-4));
    assert!(near3(after[HEAD].a, before[HEAD].a, 1e-4));
    // Hors bornes ou non fini : ramené dans des valeurs sûres.
    mq.set_joint(joint::HEAD, [f32::NAN, 1e9, -1e9]);
    assert_eq!(mq.joints[joint::HEAD], [0.0, 360.0, -360.0]);
    assert_eq!(mq.joints.len(), JOINTS);
    // Tout est proportionnel à la taille.
    let a = mesh::mannequin_capsules(&Mannequin::new(Pose::Walking, 1.0));
    let b = mesh::mannequin_capsules(&Mannequin::new(Pose::Walking, 2.0));
    for (x, y) in a.iter().zip(&b) {
        assert!(near3(m::scale(x.b, 2.0), y.b, 1e-3) && near(x.r * 2.0, y.r, 1e-4));
    }
}

// ----------------------------------------------------------------- transformations, rayons

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

#[test]
fn normals_stay_perpendicular_under_an_uneven_scale() {
    let mat = Xf { pos: [0.0; 3], rot: [0.0, 40.0, 0.0], size: [4.0, 1.0, 0.25] }.matrix();
    let nm = m::normal_matrix(&mat);
    // Une face inclinée : tangente et normale.
    let (tangent, normal) = ([1.0, 1.0, 0.0], m::normalize([1.0, -1.0, 0.0]));
    let (t2, n2) = (m::dir(&mat, tangent), m::dir(&nm, normal));
    assert!(m::dot(t2, n2).abs() < 1e-5, "la normale reste perpendiculaire à sa face : {}", m::dot(t2, n2));
}

#[test]
fn rays_hit_triangles_and_boxes() {
    let (a, b, c) = ([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
    let t = m::ray_triangle([0.5, 0.5, 5.0], [0.0, 0.0, -1.0], a, b, c).expect("touche");
    assert!(near(t, 5.0, 1e-5));
    assert!(m::ray_triangle([1.8, 1.8, 5.0], [0.0, 0.0, -1.0], a, b, c).is_none(), "hors du triangle");
    assert!(m::ray_triangle([0.5, 0.5, 5.0], [0.0, 0.0, 1.0], a, b, c).is_none(), "le triangle est derrière");
    let (t0, t1) = m::ray_box([0.0, 0.0, 5.0], [0.0, 0.0, -1.0], [-1.0; 3], [1.0; 3]).expect("touche");
    assert!(near(t0, 4.0, 1e-5) && near(t1, 6.0, 1e-5));
    assert!(m::ray_box([3.0, 0.0, 5.0], [0.0, 0.0, -1.0], [-1.0; 3], [1.0; 3]).is_none());
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
fn snapping_rounds_to_the_grid_the_steps_and_the_floor() {
    assert!(near(m::snap(0.37, 0.25), 0.25, 1e-6) && near(m::snap(0.38, 0.25), 0.5, 1e-6) && near(m::snap(-0.4, 0.25), -0.5, 1e-6));
    assert_eq!(m::snap(1.234, 0.0), 1.234, "pas nul : inchangé");
    assert!(near(m::snap(52.0, 15.0), 45.0, 1e-5), "rotation par paliers de 15°");
    // Seuls les axes bougés sont ramenés à la grille.
    let p = m::snap_position([0.37, 0.61, 0.9], 0.25, [true, false, true]);
    assert!(near3(p, [0.25, 0.61, 1.0], 1e-6), "{p:?}");
    // Le sol : un objet à 8 cm au-dessus descend, à 8 cm en dessous monte, à 1 m reste en l'air.
    assert!(near(m::floor_snap(0.08, 0.15).expect("proche"), -0.08, 1e-6) && near(m::floor_snap(-0.08, 0.15).expect("proche"), 0.08, 1e-6));
    assert!(m::floor_snap(1.0, 0.15).is_none());
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
    assert!(SceneCam { tilt: 400.0, ..cam }.sanitized().tilt <= crate::scene3d::MAX_TILT);
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
        assert!((crate::scene3d::MIN_FOCAL..=crate::scene3d::MAX_FOCAL).contains(&p.mm));
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
    // La matrice pour la carte graphique envoie le point visé au centre, et un point derrière l'œil
    // hors du volume.
    let mat = v.clip_matrix([1000.0, 600.0], 0.05, 2000.0);
    let target = m::add(cam.pos, m::scale(cam.forward(), 5.0));
    let clip = [0, 1, 2, 3].map(|r| mat[r] * target[0] + mat[4 + r] * target[1] + mat[8 + r] * target[2] + mat[12 + r]);
    assert!(near(clip[0] / clip[3], 0.0, 1e-4) && near(clip[1] / clip[3], 0.0, 1e-4), "le centre de l'image : {clip:?}");
    assert!((0.0..1.0).contains(&(clip[2] / clip[3])));
}

// --------------------------------------------------------------------- rendu vers un calque

fn scene_with_cube(panel: u64, seq: u64, cam: SceneCam) -> Scenes3d {
    let mut s = Scenes3d::default();
    let id = s.add(panel, seq, Target::Plan, Shape3d::Cube, [0.0; 3]).expect("cube");
    s.find_mut(panel, seq, id).expect("cube").color = [220, 40, 40];
    s.plan_mut(panel).cam = cam;
    s.plan_mut(panel).render = RenderSettings { style: RenderStyle::Color, lines: false, line_px: 2.0, area: RenderArea::Sheet };
    s
}

fn level_cam() -> SceneCam {
    // Le cube est posé sur le sol (centre à 0,5 m) : la caméra regarde son centre de face.
    SceneCam { pos: [0.0, 0.5, 5.0], pan: 0.0, tilt: 0.0, roll: 0.0, focal: 35.0 }
}

fn opaque_bounds(rgba: &[u8], size: [usize; 2], threshold: u8) -> Option<[usize; 4]> {
    let mut b: Option<[usize; 4]> = None;
    for y in 0..size[1] {
        for x in 0..size[0] {
            if rgba[(y * size[0] + x) * 4 + 3] >= threshold {
                b = Some(match b {
                    None => [x, y, x, y],
                    Some(o) => [o[0].min(x), o[1].min(y), o[2].max(x), o[3].max(y)],
                });
            }
        }
    }
    b
}

#[test]
fn the_plate_has_the_size_of_the_sheet_or_the_frame_and_sits_where_the_frame_is() {
    let (stage, frame) = ([640usize, 360], [320usize, 180]);
    let mut s = scene_with_cube(7, 1, level_cam());
    let models = HashMap::new();
    // Toute la feuille.
    let plate = render::render_plate(&s, 7, 1, &models, stage, frame, 1, 2);
    assert_eq!((plate.size, plate.origin, plate.rgba.len()), (stage, [0, 0], 640 * 360 * 4));
    assert_eq!(render::frame_origin(stage, frame), [160, 90], "le cadre est au milieu de la feuille");
    // Le cube est au centre du cadre, donc au centre de la feuille.
    let bounds = opaque_bounds(&plate.rgba, plate.size, 128).expect("le cube est rendu");
    let (cx, cy) = ((bounds[0] + bounds[2]) as f32 / 2.0 + 0.5, (bounds[1] + bounds[3]) as f32 / 2.0 + 0.5);
    assert!(near(cx, 320.0, 1.0) && near(cy, 180.0, 1.0), "centré sur le cadre : {cx} {cy}");
    // Le cadre seul : l'image fait la taille du cadre et se pose au bon endroit de la feuille.
    s.plan_mut(7).render.area = RenderArea::Frame;
    let framed = render::render_plate(&s, 7, 1, &models, stage, frame, 1, 2);
    assert_eq!((framed.size, framed.origin, framed.rgba.len()), (frame, [160, 90], 320 * 180 * 4));
    let raster = render::plate_raster(&framed, stage);
    assert_eq!((raster.width, raster.height), (640, 360), "le dessin du calque a la taille de la feuille");
    assert!(raster.get(320, 180).a() == 255, "le centre du cube est opaque");
    assert!(raster.get(20, 20).a() == 0 && raster.get(620, 340).a() == 0, "hors du cadre : transparent");
    // Même image que celle de toute la feuille, au même endroit (au pixel près).
    let sheet = render::plate_raster(&plate, stage);
    let mut differ = 0;
    for y in 90..270 {
        for x in 160..480 {
            if sheet.get(x, y) != raster.get(x, y) {
                differ += 1;
            }
        }
    }
    assert!(differ < 40, "{differ} pixels diffèrent entre le cadre seul et la feuille entière");
}

#[test]
fn the_cube_lands_on_the_pixels_its_projection_gives() {
    let frame = [320usize, 180];
    let stage = [640usize, 360];
    let cam = level_cam();
    let s = scene_with_cube(3, 1, cam);
    let plate = render::render_plate(&s, 3, 1, &HashMap::new(), stage, frame, 2, 2);
    let view = cam.view(frame[0] as f32);
    let center = [320.0, 180.0];
    let projected: Vec<[f32; 2]> = m::corners_of([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]).iter().map(|c| view.project(*c, center).expect("devant")).collect();
    let lo = [projected.iter().map(|p| p[0]).fold(f32::MAX, f32::min), projected.iter().map(|p| p[1]).fold(f32::MAX, f32::min)];
    let hi = [projected.iter().map(|p| p[0]).fold(f32::MIN, f32::max), projected.iter().map(|p| p[1]).fold(f32::MIN, f32::max)];
    let got = opaque_bounds(&plate.rgba, plate.size, 40).expect("rendu");
    assert!(near(got[0] as f32, lo[0], 1.6) && near(got[1] as f32, lo[1], 1.6), "coin haut-gauche {got:?} contre {lo:?}");
    assert!(near(got[2] as f32 + 1.0, hi[0], 1.6) && near(got[3] as f32 + 1.0, hi[1], 1.6), "coin bas-droit {got:?} contre {hi:?}");
    // Ce que dit la focale : à 35 mm, le cube (1 m) à 4,5 m de la caméra fait 311 · 1 / 4,5 pixels.
    let width = (got[2] + 1 - got[0]) as f32;
    assert!(near(width, 320.0 / 2.0 * 35.0 / 18.0 / 4.5, 1.6), "largeur {width}");
}

#[test]
fn a_longer_lens_fills_more_of_the_frame() {
    let (stage, frame) = ([640usize, 360], [320usize, 180]);
    let mut widths = Vec::new();
    for focal in [24.0f32, 35.0, 70.0] {
        let s = scene_with_cube(1, 1, SceneCam { focal, ..level_cam() });
        let p = render::render_plate(&s, 1, 1, &HashMap::new(), stage, frame, 1, 2);
        let b = opaque_bounds(&p.rgba, p.size, 128).expect("rendu");
        widths.push(b[2] - b[0]);
    }
    assert!(widths[0] < widths[1] && widths[1] < widths[2], "{widths:?}");
    assert!(near(widths[2] as f32 / widths[1] as f32, 2.0, 0.1), "70 mm = deux fois 35 mm : {widths:?}");
}

#[test]
fn the_styles_shade_in_colour_in_grey_or_with_outlines() {
    let (stage, frame) = ([320usize, 180], [320usize, 180]);
    let models = HashMap::new();
    let mut s = scene_with_cube(1, 1, level_cam());
    let px = |rgba: &[u8], x: usize, y: usize| {
        let i = (y * stage[0] + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    };
    // Couleurs : le cube rouge reste rouge.
    let color = render::render_plate(&s, 1, 1, &models, stage, frame, 1, 2);
    let c = px(&color.rgba, 160, 90);
    assert!(c[0] > c[1] + 40 && c[3] == 255, "{c:?}");
    // Gris : tous les pixels pris sont gris.
    s.plan_mut(1).render.style = RenderStyle::Gray;
    let gray = render::render_plate(&s, 1, 1, &models, stage, frame, 1, 2);
    for p in gray.rgba.chunks_exact(4).filter(|p| p[3] == 255) {
        assert!(p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1, "{p:?}");
    }
    // Contours : aplat clair, trait sombre à la silhouette.
    s.plan_mut(1).render.style = RenderStyle::Outline;
    let outline = render::render_plate(&s, 1, 1, &models, stage, frame, 2, 2);
    let inside = px(&outline.rgba, 160, 90);
    assert!(inside[0] > 200 && inside[3] == 255, "aplat clair : {inside:?}");
    let b = opaque_bounds(&outline.rgba, outline.size, 128).expect("rendu");
    let row: Vec<[u8; 4]> = (b[0]..=b[2]).map(|x| px(&outline.rgba, x, 90)).collect();
    assert!(row.first().expect("ligne")[0] < 90 && row.last().expect("ligne")[0] < 90, "un trait sombre aux deux bords : {:?} {:?}", row.first(), row.last());
    assert!(outline.rgba.chunks_exact(4).filter(|p| p[3] > 0 && p[0] < 80).count() > 100, "des pixels de trait");
    // Un objet gris dans une scène en couleurs.
    s.plan_mut(1).render.style = RenderStyle::Color;
    let id = s.plan(1).expect("plan").objects[0].id;
    s.find_mut(1, 1, id).expect("cube").gray = true;
    let one_gray = render::render_plate(&s, 1, 1, &models, stage, frame, 1, 2);
    let g = px(&one_gray.rgba, 160, 90);
    assert!(g[0].abs_diff(g[1]) <= 1, "{g:?}");
}

#[test]
fn what_is_behind_is_hidden_and_the_light_shades_the_faces() {
    // Deux cubes en file : le plus proche cache l'autre.
    let (stage, frame) = ([320usize, 180], [320usize, 180]);
    let mut s = scene_with_cube(1, 1, level_cam());
    let far = s.add(1, 1, Target::Plan, Shape3d::Cube, [0.0, 0.0, -3.0]).expect("cube");
    s.find_mut(1, 1, far).expect("cube").color = [30, 30, 220];
    let p = render::render_plate(&s, 1, 1, &HashMap::new(), stage, frame, 1, 2);
    let i = (90 * stage[0] + 160) * 4;
    assert!(p.rgba[i] > p.rgba[i + 2] + 40, "le cube rouge, devant, cache le bleu");
    // Une lumière de face éclaire la face avant plus qu'une lumière de dos.
    let lit = |azimuth: f32| {
        let mut s2 = scene_with_cube(1, 1, level_cam());
        s2.plan_mut(1).light = Light { azimuth, elevation: 10.0, intensity: 1.0, ambient: 0.2 };
        let p = render::render_plate(&s2, 1, 1, &HashMap::new(), stage, frame, 1, 2);
        p.rgba[(90 * stage[0] + 160) * 4]
    };
    assert!(lit(0.0) > lit(180.0) + 60, "{} contre {}", lit(0.0), lit(180.0));
}

#[test]
fn objects_behind_or_beside_the_camera_do_not_break_the_render() {
    let (stage, frame) = ([160usize, 90], [160usize, 90]);
    let mut s = scene_with_cube(1, 1, level_cam());
    // Un grand plan qui traverse le plan proche, un cube derrière la caméra, un autre énorme.
    s.add(1, 1, Target::Plan, Shape3d::Plane, [0.0, 0.0, 4.9]).expect("plan");
    s.add(1, 1, Target::Plan, Shape3d::Cube, [0.0, 0.0, 20.0]).expect("cube");
    let big = s.add(1, 1, Target::Plan, Shape3d::Sphere, [0.0; 3]).expect("sphère");
    s.find_mut(1, 1, big).expect("sphère").xf.size = [500.0; 3];
    let p = render::render_plate(&s, 1, 1, &HashMap::new(), stage, frame, 2, 2);
    assert_eq!(p.rgba.len(), 160 * 90 * 4);
}

// ----------------------------------------------------------------------------- rangement

fn rich_scenes(model: u64) -> Scenes3d {
    let mut s = Scenes3d::default();
    let (panel, seq) = (11, 2);
    s.add(panel, seq, Target::Plan, Shape3d::Cube, [1.0, 0.0, -2.0]).expect("cube");
    let mq = s.add(panel, seq, Target::Plan, Shape3d::Mannequin(Box::new(Mannequin::new(Pose::Walking, 1.62))), [0.0; 3]).expect("mannequin");
    if let Some(o) = s.find_mut(panel, seq, mq) {
        if let Shape3d::Mannequin(m) = &mut o.shape {
            m.set_joint(joint::HEAD, [12.0, -5.0, 3.0]);
        }
        o.gray = true;
        o.name = "Léa « la grande »".into();
    }
    let st = s.add(panel, seq, Target::Plan, Shape3d::Stairs { steps: 9 }, [3.0, 0.0, 0.0]).expect("escalier");
    s.add(panel, seq, Target::Plan, Shape3d::Model { id: model }, [0.0; 3]).expect("modèle");
    let sphere = s.add(panel, seq, Target::Plan, Shape3d::Sphere, [0.0; 3]).expect("sphère");
    s.group(panel, seq, &[st, sphere]).expect("groupe");
    s.add(panel, seq, Target::Decor, Shape3d::Plane, [0.0; 3]).expect("sol");
    s.add(panel, seq, Target::Decor, Shape3d::Cylinder, [-3.0, 0.0, -3.0]).expect("colonne");
    let plan = s.plan_mut(panel);
    plan.cam = SceneCam { pos: [0.5, 1.6, 7.0], pan: -12.0, tilt: 3.0, roll: 1.5, focal: 50.0 };
    plan.light = Light { azimuth: 120.0, elevation: 35.0, intensity: 1.2, ambient: 0.25 };
    plan.render = RenderSettings { style: RenderStyle::Outline, lines: true, line_px: 3.0, area: RenderArea::Frame };
    plan.use_decor = false;
    plan.bake_cel = Some(77);
    plan.follow = false;
    s
}

fn triangle_model(doc: &mut Document) -> u64 {
    doc.add_model("triangle.glb".into(), crate::model3d::Mesh3d { positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], triangles: vec![[0, 1, 2]] })
}

fn project_dir(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = crate::fiabilite_fixture::dossier(name);
    let manifest = dir.join("Film").join("Film.pelure");
    (dir, manifest)
}

#[test]
fn scenes_survive_a_save_and_a_reload() {
    let (dir, manifest) = project_dir("scenes3d-aller-retour");
    let mut doc = Document::with_frame(160, 90, 24);
    let model = triangle_model(&mut doc);
    let (panel, seq) = (doc.panels[0].id, doc.sequence_of(0));
    // Les numéros du plan et de la séquence du projet.
    let mut scenes = rich_scenes(model);
    let plan = scenes.plans.remove(&11).expect("plan");
    scenes.plans.insert(panel, plan);
    let decor = scenes.decors.remove(&2).expect("décor");
    scenes.decors.insert(seq, decor);
    scenes.touch();
    doc.scenes3d = scenes.clone();
    crate::project::save(&doc, &manifest, &mut Default::default()).expect("enregistrement");
    assert!(manifest.parent().expect("dossier").join(crate::scene3dstore::FILE).exists());
    let loaded = crate::project::load(&manifest).expect("lecture");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.doc.scenes3d, scenes, "les scènes sont rendues telles quelles");
    assert_eq!(loaded.doc.scenes3d.plan(panel).expect("plan").objects.len(), 5);
    // Le modèle que la scène montre est enregistré, même si aucun repère 3D ne le cite.
    assert!(loaded.doc.models.contains_key(&model), "le modèle de la scène est gardé");
    // Un deuxième tour donne les mêmes octets.
    let first = std::fs::read(manifest.parent().expect("dossier").join(crate::scene3dstore::FILE)).expect("fichier");
    let mut again = loaded;
    crate::project::save(&again.doc, &manifest, &mut again.written).expect("enregistrement");
    assert_eq!(std::fs::read(manifest.parent().expect("dossier").join(crate::scene3dstore::FILE)).expect("fichier"), first);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_scene_means_no_file_and_removing_the_scenes_removes_the_file() {
    let (dir, manifest) = project_dir("scenes3d-vide");
    let mut doc = Document::with_frame(160, 90, 24);
    let file = manifest.parent().expect("dossier").join(crate::scene3dstore::FILE);
    let mut written = Default::default();
    crate::project::save(&doc, &manifest, &mut written).expect("enregistrement");
    assert!(!file.exists(), "un projet sans scène n'a pas de scenes3d.json");
    let panel = doc.panels[0].id;
    doc.scenes3d.add(panel, 1, Target::Plan, Shape3d::Cube, [0.0; 3]);
    crate::project::save(&doc, &manifest, &mut written).expect("enregistrement");
    assert!(file.exists());
    doc.scenes3d = Scenes3d::default();
    crate::project::save(&doc, &manifest, &mut written).expect("enregistrement");
    assert!(!file.exists(), "plus de scène : le fichier part");
    // Une scène vide (jamais touchée) ne s'écrit pas non plus.
    doc.scenes3d.plan_mut(panel);
    crate::project::save(&doc, &manifest, &mut written).expect("enregistrement");
    assert!(!file.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_newer_or_damaged_scene_file_is_reported_and_never_erased() {
    use crate::fiabilite_fixture::{arbre, ecrire_json, lire_json};
    for (name, damage) in [("plus-recent", 0), ("abime", 1)] {
        let (dir, manifest) = project_dir(&format!("scenes3d-{name}"));
        let mut doc = Document::with_frame(160, 90, 24);
        let panel = doc.panels[0].id;
        doc.scenes3d.add(panel, 1, Target::Plan, Shape3d::Cube, [0.0; 3]);
        crate::project::save(&doc, &manifest, &mut Default::default()).expect("enregistrement");
        let root = manifest.parent().expect("dossier");
        let file = root.join(crate::scene3dstore::FILE);
        if damage == 0 {
            let mut v = lire_json(&file);
            v["version"] = serde_json::Value::from(2);
            v["champ_d_une_version_future"] = serde_json::Value::from("à garder");
            ecrire_json(&file, &v);
        } else {
            let bytes = std::fs::read(&file).expect("fichier");
            std::fs::write(&file, &bytes[..bytes.len() / 2]).expect("écriture");
        }
        let before = arbre(root);
        let mut loaded = crate::project::load(&manifest).expect("le projet s'ouvre sans ses scènes");
        assert!(loaded.warnings.iter().any(|w| w.contains(crate::scene3dstore::FILE) && w.contains("scènes 3D")), "{name} : {:?}", loaded.warnings);
        assert!(loaded.doc.scenes3d.is_empty());
        assert_eq!(arbre(root), before, "ouvrir ne touche à aucun fichier");
        // Enregistrer sans scène : le fichier illisible reste.
        crate::project::save(&loaded.doc, &manifest, &mut loaded.written).expect("enregistrement");
        assert_eq!(arbre(root).get(crate::scene3dstore::FILE), before.get(crate::scene3dstore::FILE), "{name} : le fichier a été effacé ou réécrit");
        // Avec une scène neuve : elle s'écrit, l'ancien fichier est mis de côté.
        loaded.doc.scenes3d.add(panel, 1, Target::Plan, Shape3d::Sphere, [0.0; 3]);
        crate::project::save(&loaded.doc, &manifest, &mut loaded.written).expect("enregistrement");
        let aside = root.join(format!("{}.illisible", crate::scene3dstore::FILE));
        assert!(aside.exists(), "{name} : l'ancien fichier est gardé à côté");
        assert_eq!(crate::fiabilite_fixture::empreinte(&std::fs::read(&aside).expect("fichier")), *before.get(crate::scene3dstore::FILE).expect("fichier d'origine"));
        let again = crate::project::load(&manifest).expect("relecture");
        assert!(again.warnings.is_empty() && again.doc.scenes3d.plan(panel).is_some_and(|p| p.objects.len() == 1), "{name} : {:?}", again.warnings);
        assert!(std::fs::read_dir(root).expect("dossier").flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")), "aucun .tmp ne traîne");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_hand_written_scene_file_is_made_safe() {
    let json = r#"{
        "format": "shot-scenes3d", "version": 1,
        "plans": [{ "plan": 5, "scene": { "objets": [
            { "id": 4, "nom": "A", "forme": "escalier", "marches": 9999, "placement": { "position": [1e30, 0, 0], "taille": [0, 1, -0.0] }, "couleur": [1,2,3] },
            { "id": 4, "nom": "B", "forme": "cube", "placement": { "position": [0, 0, 0], "taille": [1, 1, 1] }, "couleur": [1,2,3] },
            { "id": 0, "nom": "C", "forme": "mannequin", "taille": 99, "pose": "debout", "articulations": [[1e9,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0]], "placement": { "position": [0, 0, 0], "taille": [1, 1, 1] }, "couleur": [1,2,3] }
        ], "camera": { "position": [0, 0, 5], "panoramique": 0, "bascule": 1000, "focale": 0.001 } } }]
    }"#;
    let s = crate::scene3dstore::parse(json).expect("lisible");
    let objects = &s.plan(5).expect("plan").objects;
    assert_eq!(objects.len(), 3);
    let ids: Vec<u64> = objects.iter().map(|o| o.id).collect();
    assert!(ids.iter().all(|i| *i != 0) && ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2], "numéros uniques : {ids:?}");
    assert_eq!(objects[0].shape, Shape3d::Stairs { steps: crate::scene3d::MAX_STEPS });
    assert!(objects[0].xf.pos[0] <= 1e5 && objects[0].xf.size.iter().all(|v| v.abs() >= 1e-3), "{:?}", objects[0].xf);
    match &objects[2].shape {
        Shape3d::Mannequin(mq) => assert!(mq.height <= Mannequin::MAX_HEIGHT && mq.joints[0][0] <= 360.0),
        other => panic!("{other:?}"),
    }
    let cam = s.plan(5).expect("plan").cam;
    assert!(cam.tilt <= crate::scene3d::MAX_TILT && cam.focal >= crate::scene3d::MIN_FOCAL);
    assert!(s.next_id > *ids.iter().max().expect("numéros"));
    // Pas une scène, ou d'une autre sorte.
    assert!(crate::scene3dstore::parse("{}").is_err() && crate::scene3dstore::parse(r#"{"format":"autre","version":1}"#).is_err());
    assert!(crate::scene3dstore::parse(r#"{"format":"shot-scenes3d","version":2}"#).unwrap_err().contains("plus récente"));
}

// ------------------------------------------------------------------ édition et annulation

#[test]
fn objects_are_added_duplicated_grouped_and_deleted_by_number() {
    let mut s = Scenes3d::default();
    let (p, q) = (1, 1);
    let a = s.add(p, q, Target::Plan, Shape3d::Cube, [0.0; 3]).expect("a");
    let b = s.add(p, q, Target::Plan, Shape3d::Cube, [2.0, 0.0, 0.0]).expect("b");
    let d = s.add(p, q, Target::Decor, Shape3d::Plane, [0.0; 3]).expect("décor");
    assert!(a != b && b != d, "un numéro désigne un seul objet, plan et décor confondus");
    assert_eq!(s.find(p, q, a).expect("a").0.name, "Cube 1");
    assert_eq!(s.find(p, q, b).expect("b").0.name, "Cube 2");
    // Les objets sont posés sur le sol.
    assert!(near(s.find(p, q, a).expect("a").0.xf.pos[1], 0.5, 1e-6), "un cube de 1 m repose sur le sol");
    assert!(near(s.find(p, q, d).expect("sol").0.xf.pos[1], 0.0, 1e-6));
    let mq = s.add(p, q, Target::Plan, Shape3d::Mannequin(Box::new(Mannequin::new(Pose::Standing, 1.7))), [0.0; 3]).expect("mannequin");
    assert!(near(s.find(p, q, mq).expect("mannequin").0.xf.pos[1], 0.0, 1e-6), "le mannequin a les pieds au sol");
    // Groupe : choisir un objet choisit le groupe.
    let g = s.group(p, q, &[a, b]).expect("groupe");
    assert_eq!(s.expand_groups(p, q, &[a]).len(), 2, "le groupe {g} suit");
    assert!(s.group(p, q, &[a]).is_none(), "un groupe veut deux objets");
    // Copier un groupe : un groupe neuf.
    let copies = s.duplicate(p, q, &[a, b], [0.0, 0.0, 1.0]);
    assert_eq!(copies.len(), 2);
    let ga = s.find(p, q, copies[0]).expect("copie").0.group;
    assert!(ga.is_some() && ga != Some(g) && ga == s.find(p, q, copies[1]).expect("copie").0.group);
    assert!(near(s.find(p, q, copies[0]).expect("copie").0.xf.pos[2], 1.0, 1e-6));
    assert!(s.find(p, q, copies[0]).expect("copie").0.name.ends_with("copie"));
    assert_eq!(s.ungroup(p, q, &copies), 2);
    // Supprimer.
    assert_eq!(s.delete(p, q, &[a, b]), 2);
    assert!(s.find(p, q, a).is_none() && s.find(p, q, d).is_some());
    // Un plan voit son décor (sauf s'il ne l'utilise pas) puis ses objets.
    assert_eq!(s.visible_objects(p, q).len(), 1 + 1 + 2);
    s.plan_mut(p).use_decor = false;
    assert_eq!(s.visible_objects(p, q).len(), 1 + 2);
    // Un autre plan de la même séquence voit le même décor.
    assert_eq!(s.visible_objects(99, q).len(), 1);
    assert_eq!(s.visible_objects(99, 98).len(), 0);
}

#[test]
fn the_revision_changes_with_every_edit_and_is_unique() {
    let mut s = Scenes3d::default();
    let r0 = s.rev;
    s.add(1, 1, Target::Plan, Shape3d::Cube, [0.0; 3]);
    let r1 = s.rev;
    s.touch();
    let r2 = s.rev;
    assert!(r0 != r1 && r1 != r2 && r0 != r2);
    // Deux scènes différentes n'ont jamais le même numéro.
    let mut t = Scenes3d::default();
    t.touch();
    assert_ne!(t.rev, s.rev);
}

#[test]
fn undo_restores_the_scenes_with_a_french_step_name() {
    let doc = Document::with_frame(320, 180, 24);
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("scene3d.ajouter_cube");
    b.app.run_action("scene3d.ajouter_mannequin");
    assert!(b.app.scene3d.on, "ajouter un objet ouvre la scène 3D");
    let (panel, seq) = b.app.s3_ids();
    assert_eq!(b.app.doc.scenes3d.list(panel, seq, Target::Plan).len(), 2);
    let names = b.app.history.names().0.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(names[names.len() - 2..], ["Ajouter : cube".to_string(), "Ajouter : mannequin".to_string()], "{names:?}");
    let two = b.app.doc.scenes3d.clone();
    // Supprimer, annuler, rétablir.
    let ids: Vec<u64> = b.app.doc.scenes3d.list(panel, seq, Target::Plan).iter().map(|o| o.id).collect();
    b.app.scene3d.sel = vec![ids[0]];
    b.app.run_action("scene3d.supprimer");
    assert_eq!(b.app.doc.scenes3d.list(panel, seq, Target::Plan).len(), 1);
    assert_eq!(b.app.history.names().0.last().map(|s| s.to_string()), Some("Supprimer des objets 3D".to_string()));
    b.app.undo();
    assert_eq!(b.app.doc.scenes3d, two, "annuler rend les deux objets, tels qu'ils étaient");
    assert_ne!(b.app.doc.scenes3d.rev, 0);
    b.app.redo();
    assert_eq!(b.app.doc.scenes3d.list(panel, seq, Target::Plan).len(), 1);
    // Dupliquer, grouper.
    b.app.scene3d.sel = vec![b.app.doc.scenes3d.list(panel, seq, Target::Plan)[0].id];
    b.app.run_action("scene3d.dupliquer");
    assert_eq!(b.app.doc.scenes3d.list(panel, seq, Target::Plan).len(), 2);
    assert_eq!(b.app.history.names().0.last().map(|s| s.to_string()), Some("Dupliquer des objets 3D".to_string()));
    let all: Vec<u64> = b.app.doc.scenes3d.list(panel, seq, Target::Plan).iter().map(|o| o.id).collect();
    b.app.scene3d.sel = all;
    b.app.run_action("scene3d.grouper");
    assert!(b.app.doc.scenes3d.list(panel, seq, Target::Plan).iter().all(|o| o.group.is_some()));
    b.app.undo();
    assert!(b.app.doc.scenes3d.list(panel, seq, Target::Plan).iter().all(|o| o.group.is_none()));
}

#[test]
fn a_handle_drag_snaps_to_the_grid_and_the_floor_and_is_one_undo_step() {
    let doc = Document::with_frame(320, 180, 24);
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("scene3d.ajouter_cube");
    let (panel, seq) = b.app.s3_ids();
    let id = b.app.doc.scenes3d.list(panel, seq, Target::Plan)[0].id;
    b.app.scene3d.sel = vec![id];
    b.app.scene3d.mode = Mode::Move;
    let cam = crate::view3d::Cam3d { target: [0.0, 0.5, 0.0], azimuth: 0.0, elevation: 0.0, distance: 8.0, focal: 35.0 };
    let view = crate::scene3dview::orbit_view(&cam, 800.0);
    let center = [400.0, 300.0];
    let start = view.project([0.0, 0.5, 0.0], center).expect("visible");
    let gizmo = b.app.s3_gizmo(&view).expect("un manipulateur");
    let before = b.app.history.names().0.len();
    let ray = view.ray(start, center);
    b.app.s3_begin(Handle::Axis(0), Mode::Move, gizmo, ray, start, "Déplacer dans l'espace 3D");
    // Un glisser de 1,37 m vers la droite : aimanté au pas de 0,25 m.
    let to = view.project([1.37, 0.5, 0.0], center).expect("visible");
    b.app.s3_drag(&view, center, to);
    let x = b.app.doc.scenes3d.find(panel, seq, id).expect("cube").0.xf.pos;
    assert!(near(x[0], 1.25, 1e-3), "aimanté à la grille : {x:?}");
    assert!(near(x[1], 0.5, 1e-4), "l'axe X ne change pas la hauteur");
    // Sans aimantation : la valeur exacte.
    b.app.scene3d.snap = false;
    b.app.s3_drag(&view, center, to);
    let x = b.app.doc.scenes3d.find(panel, seq, id).expect("cube").0.xf.pos;
    assert!(near(x[0], 1.37, 0.02), "{x:?}");
    b.app.s3_finish_drag();
    let names = b.app.history.names().0.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(names.len(), before + 1, "tout le glisser est une seule étape : {names:?}");
    assert_eq!(names.last().map(String::as_str), Some("Déplacer dans l'espace 3D"));
    b.app.undo();
    assert!(near(b.app.doc.scenes3d.find(panel, seq, id).expect("cube").0.xf.pos[0], 0.0, 1e-5));
    // Le sol : monté de quelques centimètres, le cube redescend sur le sol (même sans grille).
    b.app.scene3d.snap = false;
    b.app.scene3d.snap_floor = true;
    let gizmo = b.app.s3_gizmo(&view).expect("un manipulateur");
    b.app.s3_begin(Handle::Axis(1), Mode::Move, gizmo, view.ray(start, center), start, "Déplacer dans la scène 3D");
    let up = view.project([0.0, 0.5 + 0.07, 0.0], center).expect("visible");
    b.app.s3_drag(&view, center, up);
    let y = b.app.doc.scenes3d.find(panel, seq, id).expect("cube").0.xf.pos[1];
    assert!(near(y, 0.5, 1e-3), "le cube se pose sur le sol : {y}");
    b.app.s3_finish_drag();
}

// ------------------------------------------------------------- l'application sans fenêtre

fn wait_for(b: &mut crate::perf_tests::Banc, what: &str, mut done: impl FnMut(&mut crate::perf_tests::Banc) -> bool) {
    let start = std::time::Instant::now();
    while !done(b) {
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "trop long : {what}");
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.image(Vec::new(), 0.05);
    }
}

#[test]
fn sending_the_scene_makes_a_normal_layer_that_follows_the_scene_in_the_background() {
    let doc = Document::with_frame(160, 90, 24);
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    let layers_before = b.app.layers().len();
    b.app.run_action("scene3d.ajouter_cube");
    b.app.run_action("scene3d.envoyer");
    // Le calque est là tout de suite (vide), tout en bas, verrouillé ; le rendu vient après.
    let (panel, _) = b.app.s3_ids();
    assert_eq!(b.app.layers().len(), layers_before + 1);
    let layer = &b.app.layers()[0];
    assert_eq!((layer.name.as_str(), layer.locked, layer.kind), (crate::scene3dui::LAYER_NAME, true, crate::document::LayerKind::Bitmap));
    let cel = b.app.doc.scenes3d.plan(panel).and_then(|p| p.bake_cel).expect("lien vers le dessin");
    assert_eq!(layer.frames.iter().flatten().next(), Some(&cel));
    assert_eq!(b.app.history.names().0.last().copied(), Some("Figer les objets 3D dans un calque de fond"));
    // L'image se calcule sur un autre fil : l'interface n'attend pas.
    let t = std::time::Instant::now();
    b.image(Vec::new(), 0.05);
    assert!(t.elapsed() < std::time::Duration::from_secs(5), "une image d'interface ne doit pas attendre le rendu");
    wait_for(&mut b, "le premier rendu", |b| b.app.scene3d.baked >= 1);
    let (w, h) = (b.app.doc.width, b.app.doc.height);
    let raster = &b.app.doc.cels[&cel].raster;
    assert_eq!((raster.width, raster.height), (w, h), "le dessin est à la taille de la feuille");
    assert!(raster.get(w / 2, h / 2).a() == 255, "le cube est au centre");
    assert!(b.app.doc.cels[&cel].version >= 1);
    // Au repos, rien ne demande d'afficher : plus de calcul, plus de réveil.
    let calm = se_reposer(&mut b, 40);
    assert!(calm < 40, "l'application finit par se reposer ({calm} images)");
    let baked = b.app.scene3d.baked;
    for _ in 0..10 {
        let m = b.image(Vec::new(), 0.5);
        assert!(m.reveil >= std::time::Duration::from_millis(900), "rien à refaire : pas de réveil ({:?})", m.reveil);
    }
    assert_eq!(b.app.scene3d.baked, baked, "rien n'a changé : pas de nouveau rendu");
    // Changer la scène relance le rendu, sans toucher au document autrement.
    let id = b.app.doc.scenes3d.list(panel, 1, Target::Plan).first().map(|o| o.id).expect("cube");
    let xf = Xf { pos: [1.0, 0.5, 0.0], ..b.app.doc.scenes3d.find(panel, b.app.s3_ids().1, id).expect("cube").0.xf };
    b.app.s3_set_xfs(&[(id, xf)]);
    wait_for(&mut b, "le rendu suivant", |b| b.app.scene3d.baked >= baked + 1);
    let raster = &b.app.doc.cels[&cel].raster;
    assert!(raster.get(w / 2, h / 2).a() < 255 || raster.get(w / 2 + 20, h / 2).a() > 0, "la scène a bougé dans le calque");
    // Renvoyer remet à jour le même calque.
    let n = b.app.layers().len();
    b.app.run_action("scene3d.envoyer");
    assert_eq!(b.app.layers().len(), n, "le calque existe déjà : pas de second calque");
}

#[test]
fn the_background_image_waits_while_the_user_is_busy_and_never_arrives_stale() {
    let doc = Document::with_frame(160, 90, 24);
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("scene3d.ajouter_cube");
    // Pendant la lecture (ou un trait, ou un geste dans la vue) : le rendu ne démarre pas.
    b.app.playing = true;
    b.app.run_action("scene3d.envoyer");
    b.app.playing = true;
    for _ in 0..5 {
        b.image(Vec::new(), 0.05);
    }
    assert!(b.app.scene3d.job.is_none() && b.app.scene3d.baked == 0, "rien ne démarre pendant la lecture");
    b.app.playing = false;
    wait_for(&mut b, "le rendu", |b| b.app.scene3d.baked >= 1);
    // Une image finie pendant qu'on est occupé attend, puis se pose si elle est à jour, sinon est jetée.
    let (panel, seq) = b.app.s3_ids();
    let id = b.app.doc.scenes3d.list(panel, seq, Target::Plan)[0].id;
    let base = b.app.scene3d.baked;
    let xf = Xf { pos: [0.5, 0.5, 0.0], ..Xf::default() };
    b.app.s3_set_xfs(&[(id, xf)]);
    b.image(Vec::new(), 0.05);
    b.app.playing = true; // « occupé » (un trait ou un geste dans la vue 3D compterait de même)
    let start = std::time::Instant::now();
    while b.app.scene3d.ready.is_none() && b.app.scene3d.job.is_some() && start.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.image(Vec::new(), 0.05);
    }
    assert_eq!(b.app.scene3d.baked, base, "pas posée tant qu'on est occupé");
    b.app.s3_set_xfs(&[(id, Xf { pos: [-0.5, 0.5, 0.0], ..Xf::default() })]);
    b.app.playing = false;
    wait_for(&mut b, "l'image à jour", |b| b.app.scene3d.baked > base);
    assert_eq!(b.app.scene3d.applied.as_ref().map(|k| k.rev), Some(b.app.doc.scenes3d.rev), "l'image posée est celle de la scène d'aujourd'hui");
}

#[test]
fn the_scene_view_runs_in_the_application_without_a_graphics_card() {
    let mut doc = Document::with_frame(320, 180, 24);
    let panel = doc.panels[0].id;
    let seq = doc.sequence_of(0);
    for k in 0..8 {
        doc.scenes3d.add(panel, seq, Target::Plan, Shape3d::Cube, [k as f32 - 4.0, 0.0, 0.0]);
    }
    doc.scenes3d.add(panel, seq, Target::Decor, Shape3d::Plane, [0.0; 3]);
    let mut b = banc(doc, false);
    b.app.run_action("scene3d.basculer");
    assert!(b.app.scene3d.on);
    b.chauffer(6);
    assert!(b.app.scene3d.cam.is_some(), "la vue est cadrée sur la scène");
    // Les réglages de la scène, tous les objets, la caméra, la lumière : un tour d'interface.
    let first = b.app.doc.scenes3d.list(panel, seq, Target::Plan)[0].id;
    b.app.scene3d.sel = vec![first];
    b.chauffer(3);
    b.app.scene3d.sel = vec![CAMERA_ID];
    b.app.scene3d.through_camera = true;
    b.chauffer(3);
    b.app.scene3d.through_camera = false;
    b.app.scene3d.mode = Mode::Rotate;
    b.chauffer(3);
    // Quitter : retour au dessin.
    b.app.run_action("scene3d.basculer");
    assert!(!b.app.scene3d.on);
    b.chauffer(3);
    // Un seul espace 3D : ses trois noms (l'ancien plateau en 3D, l'ancienne scène 3D, le nouveau) mènent au même.
    b.app.run_action("vue.3d");
    assert!(b.app.scene3d.on, "l'ancien nom du plateau en 3D ouvre l'Espace 3D");
    b.app.run_action("espace3d.basculer");
    assert!(!b.app.scene3d.on);
    b.app.run_action("scene3d.basculer");
    assert!(b.app.scene3d.on);
}

#[test]
fn the_mouse_picks_drags_and_orbits_in_the_view() {
    use eframe::egui::{Event, Modifiers, PointerButton};
    let mut doc = Document::with_frame(320, 180, 24);
    let (panel, seq) = (doc.panels[0].id, doc.sequence_of(0));
    let cube = doc.scenes3d.add(panel, seq, Target::Plan, Shape3d::Cube, [0.0; 3]).expect("cube");
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.run_action("scene3d.basculer");
    b.app.scene3d.show_grid = false;
    b.app.scene3d.mode = Mode::Select;
    b.chauffer(5);
    let rect = b.app.canvas_rect;
    let at = |b: &crate::perf_tests::Banc, w: V3| {
        let cam = b.app.scene3d.cam.expect("vue");
        let view = crate::scene3dview::orbit_view(&cam, rect.width());
        let p = view.project(w, [rect.width() / 2.0, rect.height() / 2.0]).expect("visible");
        pos2(rect.left() + p[0], rect.top() + p[1])
    };
    let button = |pos, button, pressed| Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE };
    // Un clic sur le cube le choisit ; un clic dans le vide ne choisit plus rien.
    let p = at(&b, [0.0, 0.5, 0.0]);
    b.image(vec![Event::PointerMoved(p)], 0.02);
    b.image(vec![button(p, PointerButton::Primary, true)], 0.02);
    b.image(vec![button(p, PointerButton::Primary, false)], 0.02);
    b.chauffer(2);
    assert_eq!(b.app.scene3d.sel, vec![cube], "le clic choisit le cube");
    assert!(b.app.history.names().0.is_empty(), "choisir n'est pas une étape d'annulation");
    let empty = pos2(rect.left() + 40.0, rect.top() + 60.0);
    b.image(vec![Event::PointerMoved(empty)], 0.02);
    b.image(vec![button(empty, PointerButton::Primary, true)], 0.02);
    b.image(vec![button(empty, PointerButton::Primary, false)], 0.02);
    b.chauffer(2);
    assert!(b.app.scene3d.sel.is_empty(), "un clic dans le vide lâche le choix");
    // Glisser le cube : il suit la souris sur le sol, d'un seul geste d'annulation.
    let to = at(&b, [2.0, 0.5, 0.0]);
    b.image(vec![Event::PointerMoved(p)], 0.02);
    b.image(vec![button(p, PointerButton::Primary, true)], 0.02);
    for k in 1..=8 {
        let t = k as f32 / 8.0;
        b.image(vec![Event::PointerMoved(p + (to - p) * t)], 0.02);
    }
    b.image(vec![button(to, PointerButton::Primary, false)], 0.02);
    b.chauffer(2);
    let x = b.app.doc.scenes3d.find(panel, seq, cube).expect("cube").0.xf.pos;
    assert!(x[0] > 1.5 && x[0] < 2.6 && near(x[1], 0.5, 1e-3), "le cube a suivi la souris : {x:?}");
    assert_eq!(b.app.history.names().0, vec!["Déplacer dans l'espace 3D"]);
    b.app.undo();
    assert!(near(b.app.doc.scenes3d.find(panel, seq, cube).expect("cube").0.xf.pos[0], 0.0, 1e-5));
    // Clic droit glissé : la vue tourne, sans rien changer à la scène (ni étape d'annulation).
    let az = b.app.scene3d.cam.expect("vue").azimuth;
    b.image(vec![Event::PointerMoved(empty)], 0.02);
    b.image(vec![button(empty, PointerButton::Secondary, true)], 0.02);
    for k in 1..=5 {
        b.image(vec![Event::PointerMoved(empty + vec2(20.0 * k as f32, 0.0))], 0.02);
    }
    b.image(vec![button(empty + vec2(100.0, 0.0), PointerButton::Secondary, false)], 0.02);
    assert!((b.app.scene3d.cam.expect("vue").azimuth - az).abs() > 5.0, "la vue a tourné");
    // La molette avance.
    let distance = b.app.scene3d.cam.expect("vue").distance;
    b.image(vec![Event::PointerMoved(p)], 0.02);
    let wheel = b.molette(vec2(0.0, 120.0), Modifiers::NONE);
    b.image(wheel, 0.02);
    assert!(b.app.scene3d.cam.expect("vue").distance < distance, "la molette rapproche");
    assert_eq!(b.app.history.names().0.len(), 0, "naviguer n'est pas une étape d'annulation");
}

#[test]
fn the_gpu_viewport_draws_what_the_cpu_render_draws() {
    let mut doc = Document::with_frame(320, 180, 24);
    let (panel, seq) = (doc.panels[0].id, doc.sequence_of(0));
    doc.scenes3d.plan_mut(panel).render = RenderSettings { style: RenderStyle::Color, lines: false, line_px: 2.0, area: RenderArea::Sheet };
    let cube = doc.scenes3d.add(panel, seq, Target::Plan, Shape3d::Cube, [0.0; 3]).expect("cube");
    doc.scenes3d.add(panel, seq, Target::Plan, Shape3d::Sphere, [1.6, 0.0, 0.5]);
    doc.scenes3d.add(panel, seq, Target::Plan, Shape3d::Mannequin(Box::new(Mannequin::new(Pose::Walking, 1.7))), [-1.8, 0.0, 0.0]);
    let mut b = banc(doc, true);
    if b.app.gpu.is_none() {
        eprintln!("scene3d_tests : pas de carte graphique sur cette machine, essai sauté");
        return;
    }
    b.app.run_action("scene3d.basculer");
    // (Les cartes sont cachées : cet essai compare les objets, pas les calques du plan.)
    b.app.scene3d.show_cards = false;
    b.app.scene3d.show_grid = false;
    b.app.scene3d.sel.clear();
    b.chauffer(6);
    let gpu = b.app.scene3d.gpu.as_ref().expect("ressources de la carte graphique");
    assert_eq!(gpu.drawn(), 3, "trois objets dessinés");
    assert_eq!(gpu.mesh_count(), 3);
    let shot = b.capturer().expect("capture");
    // Pour voir l'image : SHOT_SCENE3D_IMAGE=<dossier> écrit la capture de l'écran (PNG).
    if let Ok(dir) = std::env::var("SHOT_SCENE3D_IMAGE") {
        let _ = image::save_buffer(std::path::Path::new(&dir).join("scene3d-ecran.png"), &shot, 1920, 1080, image::ColorType::Rgba8);
    }
    let rect: Rect = b.app.canvas_rect;
    let cam = b.app.scene3d.cam.expect("vue");
    let view = crate::scene3dview::orbit_view(&cam, rect.width());
    let (w, h) = (rect.width() as usize, rect.height() as usize);
    let (plan_light, plan_render) = {
        let p = b.app.doc.scenes3d.plan(panel).cloned().unwrap_or_default();
        (p.light, p.render)
    };
    let items: Vec<Item> = {
        let mut lib = mesh::MeshLib::default();
        render::items_of(&b.app.doc.scenes3d, panel, seq, &b.app.doc.models, &mut lib)
    };
    let cpu = render::render(
        &items,
        &Opts { view, size: [w, h], center: [w as f32 / 2.0, h as f32 / 2.0], light: plan_light, style: plan_render.style, lines: false, line_px: 2.0, ss: 4, paper: None, threads: 2 },
    );
    // Les silhouettes (objet ou fond) concordent presque partout.
    let bg = (crate::scene3dgpu::BACKGROUND[0] * 255.0).round() as i32;
    let (mut total, mut differ, mut covered, mut gpu_only, mut cpu_only) = (0usize, 0usize, 0usize, 0usize, 0usize);
    // (Sans les lignes du haut et du bas, où la vue écrit ses légendes.)
    for y in 24..h - 30 {
        for x in 2..w - 2 {
            let (sx, sy) = (rect.left() as usize + x, rect.top() as usize + y);
            let g = &shot[(sy * 1920 + sx) * 4..(sy * 1920 + sx) * 4 + 3];
            // (La caméra du plateau est tracée en orange par-dessus : le rendu vers un calque ne la montre pas.)
            if g[0] as i32 - g[2] as i32 > 22 && g[0] as i32 - g[1] as i32 > 14 {
                continue;
            }
            let gpu_on = (g[0] as i32 - bg).abs() > 14 || (g[2] as i32 - (crate::scene3dgpu::BACKGROUND[2] * 255.0).round() as i32).abs() > 14;
            // (La carte graphique lisse les bords sur 4 échantillons : un pixel couvert au quart se voit.)
            let cpu_on = cpu[(y * w + x) * 4 + 3] >= 48;
            total += 1;
            covered += cpu_on as usize;
            differ += (gpu_on != cpu_on) as usize;
            gpu_only += (gpu_on && !cpu_on) as usize;
            cpu_only += (cpu_on && !gpu_on) as usize;
        }
    }
    eprintln!("GPU/CPU : {total} pixels, {covered} couverts, {gpu_only} seulement GPU, {cpu_only} seulement CPU, rect {rect:?}");
    assert!(covered > total / 40, "la scène occupe de la place à l'écran ({covered} sur {total})");
    assert!((differ as f32) < total as f32 * 0.005, "{differ} pixels sur {total} diffèrent entre la carte graphique et le processeur");
    let _ = cube;
}

// --------------------------------------------------------------------- voir ce que ça donne

/// `SHOT_SCENE3D_IMAGE=<dossier> cargo test --release scene3d_tests::images -- --ignored` : écrit des
/// captures de la vue d'édition (carte graphique) et des rendus vers un calque, pour les regarder.
#[test]
#[ignore]
fn images_of_the_scene_views() {
    let Ok(dir) = std::env::var("SHOT_SCENE3D_IMAGE") else {
        eprintln!("SHOT_SCENE3D_IMAGE n'est pas défini : rien à écrire");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let mut doc = Document::with_frame(480, 270, 24);
    let (panel, seq) = (doc.panels[0].id, doc.sequence_of(0));
    let s = &mut doc.scenes3d;
    s.add(panel, seq, Target::Decor, Shape3d::Plane, [0.0; 3]);
    s.add(panel, seq, Target::Decor, Shape3d::Stairs { steps: 6 }, [3.2, 0.0, -2.0]);
    s.add(panel, seq, Target::Decor, Shape3d::Cylinder, [-3.4, 0.0, -2.5]);
    let wall = s.add(panel, seq, Target::Decor, Shape3d::Plane, [0.0, 1.5, -5.0]).expect("mur");
    if let Some(o) = s.find_mut(panel, seq, wall) {
        o.xf.rot = [90.0, 0.0, 0.0];
        o.xf.size = [12.0, 1.0, 3.0];
        o.color = [175, 160, 140];
    }
    let table = s.add(panel, seq, Target::Plan, Shape3d::Cube, [0.4, 0.0, -1.0]).expect("table");
    if let Some(o) = s.find_mut(panel, seq, table) {
        o.xf.size = [1.4, 0.75, 0.8];
        o.xf.pos[1] = 0.375;
        o.color = [150, 110, 80];
    }
    for (k, (pose, x, z, turn)) in [(Pose::Standing, -1.6, 0.4, 20.0), (Pose::Sitting, 0.4, -0.2, 180.0), (Pose::Walking, 1.9, 0.9, -35.0), (Pose::ArmsUp, -0.6, 1.4, 0.0)].into_iter().enumerate() {
        let id = s.add(panel, seq, Target::Plan, Shape3d::Mannequin(Box::new(Mannequin::new(pose, 1.7))), [x, 0.0, z]).expect("mannequin");
        if let Some(o) = s.find_mut(panel, seq, id) {
            o.xf.rot[1] = turn;
            o.color = [[205, 150, 120], [120, 160, 205], [180, 205, 120], [215, 190, 110]][k];
            if pose == Pose::Sitting {
                o.xf.pos[1] = 0.45;
            }
        }
    }
    s.plan_mut(panel).cam = SceneCam { pos: [0.6, 1.5, 6.2], pan: -3.0, tilt: -3.0, roll: 0.0, focal: 35.0 };
    let mut b = banc(doc, true);
    let Some(_) = b.app.gpu.as_ref() else {
        eprintln!("pas de carte graphique : rien à écrire");
        return;
    };
    let save = |name: &str, rgba: &[u8], w: u32, h: u32| {
        let _ = image::save_buffer(dir.join(name), rgba, w, h, image::ColorType::Rgba8);
    };
    b.app.run_action("scene3d.basculer");
    b.app.scene3d.mode = Mode::Move;
    let ids: Vec<u64> = b.app.doc.scenes3d.list(panel, seq, Target::Plan).iter().map(|o| o.id).collect();
    b.app.scene3d.sel = vec![ids[ids.len() - 2]];
    b.chauffer(6);
    save("1-edition-deplacer.png", &b.capturer().expect("capture"), 1920, 1080);
    b.app.scene3d.mode = Mode::Rotate;
    b.chauffer(3);
    save("2-edition-tourner.png", &b.capturer().expect("capture"), 1920, 1080);
    b.app.scene3d.sel = vec![CAMERA_ID];
    b.app.scene3d.mode = Mode::Move;
    b.chauffer(3);
    save("3-edition-camera.png", &b.capturer().expect("capture"), 1920, 1080);
    b.app.scene3d.sel.clear();
    b.app.scene3d.through_camera = true;
    b.chauffer(3);
    save("4-par-la-camera.png", &b.capturer().expect("capture"), 1920, 1080);
    // Le bas du panneau Réglages (on y fait défiler).
    b.app.scene3d.through_camera = false;
    b.app.scene3d.sel = vec![ids[ids.len() - 2]];
    let at = b.souris(1650.0, 300.0);
    b.image(at, 0.02);
    for _ in 0..6 {
        let wheel = b.molette(eframe::egui::vec2(0.0, -300.0), eframe::egui::Modifiers::NONE);
        b.image(wheel, 0.02);
    }
    b.chauffer(2);
    save("8-reglages-bas.png", &b.capturer().expect("capture"), 1920, 1080);
    // Les rendus vers un calque, posés sur du papier blanc.
    for (style, name) in [(RenderStyle::Color, "5-rendu-couleurs.png"), (RenderStyle::Gray, "6-rendu-gris.png"), (RenderStyle::Outline, "7-rendu-contours.png")] {
        let mut scenes = b.app.doc.scenes3d.clone();
        scenes.plan_mut(panel).render = RenderSettings { style, lines: true, line_px: 2.0, area: RenderArea::Frame };
        let plate = render::render_plate(&scenes, panel, seq, &b.app.doc.models, [b.app.doc.width, b.app.doc.height], b.app.doc.frame, 3, 0);
        let mut flat = plate.rgba.clone();
        for p in flat.chunks_exact_mut(4) {
            let a = p[3] as f32 / 255.0;
            for c in &mut p[..3] {
                *c = (*c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
            }
            p[3] = 255;
        }
        save(name, &flat, plate.size[0] as u32, plate.size[1] as u32);
    }
}

// ------------------------------------------------------------------------------- mesures

/// `cargo test --release scene3d_tests::perf -- --ignored --nocapture`
#[test]
#[ignore]
fn perf_100_primitives() {
    use std::time::Instant;
    let mut doc = Document::with_frame(1920, 1080, 24);
    let (panel, seq) = (doc.panels[0].id, doc.sequence_of(0));
    let shapes = Shape3d::basics();
    for k in 0..100 {
        let shape = shapes[k % shapes.len()].clone();
        doc.scenes3d.add(panel, seq, Target::Plan, shape, [(k % 10) as f32 * 1.5 - 7.0, 0.0, (k / 10) as f32 * -1.5]);
    }
    doc.scenes3d.plan_mut(panel).cam = SceneCam { pos: [0.0, 4.0, 9.0], tilt: -18.0, ..SceneCam::default() };
    let stage = [doc.width, doc.height];
    let t = Instant::now();
    let mut lib = mesh::MeshLib::default();
    let items = render::items_of(&doc.scenes3d, panel, seq, &doc.models, &mut lib);
    println!("PERF scène 3D : {} objets, maillages construits en {:.1} ms", items.len(), t.elapsed().as_secs_f64() * 1e3);
    for (label, area) in [("cadre 1920x1080", RenderArea::Frame), ("feuille 3840x2160", RenderArea::Sheet)] {
        doc.scenes3d.plan_mut(panel).render.area = area;
        let t = Instant::now();
        let plate = render::render_plate(&doc.scenes3d, panel, seq, &doc.models, stage, doc.frame, 2, 0);
        println!("PERF scène 3D : rendu vers un calque, {label}, sur-échantillonnage 2 : {:.0} ms ({} Mo)", t.elapsed().as_secs_f64() * 1e3, plate.rgba.len() / 1_000_000);
    }
    // Choisir à la souris, sur 100 objets.
    let mut b = banc(doc, true);
    b.app.run_action("scene3d.basculer");
    b.chauffer(6);
    let ray = Ray::new([0.0, 4.0, 9.0], [0.0, -0.3, -1.0]);
    let t = Instant::now();
    for _ in 0..20 {
        let _ = b.app.s3_pick(&ray);
    }
    println!("PERF scène 3D : choisir un objet à la souris (100 objets) : {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / 20.0);
    let frames: Vec<f64> = (0..60).map(|k| b.image(vec![eframe::egui::Event::PointerMoved(pos2(900.0 + k as f32, 500.0))], 1.0 / 60.0).total()).collect();
    let mean = frames.iter().sum::<f64>() / frames.len() as f64;
    println!("PERF scène 3D : vue d'édition (carte graphique : {}), 100 objets : {:.2} ms par image (max {:.2})", b.app.gpu.is_some(), mean, frames.iter().cloned().fold(0.0, f64::max));
    let idle = se_reposer(&mut b, 60);
    println!("PERF scène 3D : repos atteint après {idle} images");
}
