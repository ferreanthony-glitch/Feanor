//! Essais de l'Espace 3D (`espace3d.rs`, `docs/ESPACE3D.md`) : la pose d'une carte et son chemin inverse, l'accord avec
//! le multiplan d'avant, la carte inclinée (géométrie, parallaxe, processeur contre carte graphique, vignette), les clés,
//! l'enregistrement (format 35) et les anciens projets.

use eframe::egui::{pos2, vec2, Color32};

use crate::camera::{Camera, CameraKey};
use crate::perf_tests::{banc_taille, Banc};
use crate::scene3d::{Shape3d, Target};
use crate::scene3dgizmo::{self as gz, Handle, Mode, CAMERA_ID};
use crate::scene3dmath as m;
use crate::view3d::Cam3d;
use crate::document::{CameraSeen, Document, Layer, LayerKind, LayerTransform};
use crate::espace3d::{self, Monde};
use crate::poskeys::{self, PosKey};
use crate::project;
use crate::raster::TILE;
use crate::render;
use crate::scene3d::Xf;

const RED: Color32 = Color32::from_rgb(220, 20, 20);
const GREEN: Color32 = Color32::from_rgb(20, 200, 40);
const BLUE: Color32 = Color32::from_rgb(30, 60, 230);

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

fn near3(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
    (0..3).all(|k| near(a[k], b[k], eps))
}

/// Un rectangle plein [x0, x1) × [y0, y1) dans le dessin `cel`.
fn fill_rect(doc: &mut Document, cel: crate::document::CelId, x0: usize, y0: usize, x1: usize, y1: usize, color: Color32) {
    let r = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = color;
        }
    }
}

/// Un projet 320 × 180 (feuille = cadre) dont chaque calque a un dessin plein de la couleur donnée, dans le rectangle
/// donné, à la profondeur donnée.
fn doc_with_cards(cards: &[(Color32, [usize; 4], f32)]) -> Document {
    let mut doc = Document::new(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for (k, (color, r, depth)) in cards.iter().enumerate() {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        fill_rect(&mut doc, cel, r[0], r[1], r[2], r[3], *color);
        layer.frames = vec![Some(cel); 12];
        layer.depth = *depth;
        doc.panels[0].layers.push(layer);
    }
    doc
}

fn sheet(doc: &Document, frame: usize) -> Vec<Color32> {
    let plan = render::panel_plan_at(doc, 0, frame, None);
    render::composite_full(doc, &plan)
}

/// Le centre de gravité des pixels de la couleur `c` (à 40 niveaux près) d'une image de largeur `w`.
fn centroid(img: &[Color32], w: usize, c: Color32) -> Option<(f32, f32)> {
    let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
    for (i, p) in img.iter().enumerate() {
        let close = |a: u8, b: u8| (a as i32 - b as i32).abs() < 40;
        if p.a() > 200 && close(p.r(), c.r()) && close(p.g(), c.g()) && close(p.b(), c.b()) {
            sx += (i % w) as f64 + 0.5;
            sy += (i / w) as f64 + 0.5;
            n += 1.0;
        }
    }
    (n > 0.0).then(|| ((sx / n) as f32, (sy / n) as f32))
}

/// La boîte (x0, y0, x1, y1) des pixels opaques.
fn bbox(img: &[Color32], w: usize) -> Option<(usize, usize, usize, usize)> {
    let mut b: Option<(usize, usize, usize, usize)> = None;
    for (i, p) in img.iter().enumerate() {
        if p.a() > 128 {
            let (x, y) = (i % w, i / w);
            b = Some(b.map_or((x, y, x + 1, y + 1), |(a, bb, c, d)| (a.min(x), bb.min(y), c.max(x + 1), d.max(y + 1))));
        }
    }
    b
}

/// Hauteur (pixels opaques) de la colonne `x`.
fn column_height(img: &[Color32], w: usize, h: usize, x: usize) -> usize {
    (0..h).filter(|&y| img[y * w + x].a() > 128).count()
}

fn pose(x: f32, y: f32) -> LayerTransform {
    LayerTransform { x, y, scale: 1.2, angle: 20.0, stretch_x: 1.1, stretch_y: 0.9, ..Default::default() }
}

// ------------------------------------------------------------------------------------ la pose d'une carte

#[test]
fn a_card_pose_round_trips_through_the_world_placement() {
    let w = Monde::new([640, 360], [320, 180]);
    for (t, d) in [
        (LayerTransform::default(), 0.0),
        (pose(30.0, -20.0), 1.5),
        (LayerTransform { tilt_x: 25.0, tilt_y: -40.0, ..pose(-12.0, 8.0) }, -0.4),
        (LayerTransform { stretch_x: -1.3, ..pose(5.0, 5.0) }, 3.0),
    ] {
        let xf = w.card_xf(&t, d);
        let (back, d2) = w.card_from_xf(&t, d, &xf);
        assert!(near(d, d2, 1e-4), "profondeur {d} → {d2}");
        for (a, b) in [(t.x, back.x), (t.y, back.y), (t.scale, back.scale), (t.angle, back.angle), (t.stretch_x, back.stretch_x), (t.stretch_y, back.stretch_y), (t.tilt_x, back.tilt_x), (t.tilt_y, back.tilt_y)] {
            assert!(near(a, b, 2e-3 * a.abs().max(1.0)), "{t:?} → {back:?}");
        }
    }
}

#[test]
fn moving_a_card_in_z_keeps_its_world_size_and_position() {
    let w = Monde::new([640, 360], [320, 180]);
    let t = pose(30.0, -20.0);
    let xf = w.card_xf(&t, 1.0);
    // Reculée de 2 m : la taille dans le monde ne change pas (elle rapetisse à l'écran), x et y du monde non plus.
    let farther = Xf { pos: [xf.pos[0], xf.pos[1], xf.pos[2] - 2.0], ..xf };
    let (t2, d2) = w.card_from_xf(&t, 1.0, &farther);
    assert!(d2 > 1.0);
    let again = w.card_xf(&t2, d2);
    assert!(near3(again.pos, farther.pos, 1e-3), "{:?} contre {:?}", again.pos, farther.pos);
    assert!(near3(again.size, farther.size, 1e-3), "{:?} contre {:?}", again.size, farther.size);
    assert!(near3(again.rot, farther.rot, 1e-3));
}

#[test]
fn a_one_axis_scale_changes_the_stretch_and_a_uniform_one_the_scale() {
    let w = Monde::new([640, 360], [320, 180]);
    let t = pose(0.0, 0.0);
    let xf = w.card_xf(&t, 0.5);
    let wide = Xf { size: [xf.size[0] * 1.5, xf.size[1], 1.0], ..xf };
    let (a, _) = w.card_from_xf(&t, 0.5, &wide);
    assert!(near(a.scale, t.scale, 1e-4) && near(a.stretch_x, t.stretch_x * 1.5, 1e-3) && near(a.stretch_y, t.stretch_y, 1e-4), "{a:?}");
    let big = Xf { size: [xf.size[0] * 2.0, xf.size[1] * 2.0, 1.0], ..xf };
    let (b, _) = w.card_from_xf(&t, 0.5, &big);
    assert!(near(b.scale, t.scale * 2.0, 1e-3) && near(b.stretch_x, t.stretch_x, 1e-5) && near(b.stretch_y, t.stretch_y, 1e-5), "{b:?}");
}

#[test]
fn the_card_is_the_flat_multiplan_layer_seen_from_the_eye() {
    // Le cœur de la fusion : le monde 3D et le multiplan d'avant donnent le même point de la feuille.
    let w = Monde::new([320, 180], [320, 180]);
    let pivot = pos2(160.0, 90.0);
    for depth in [-0.5f32, 0.0, 0.8, 2.0, 6.0] {
        for (offset, dolly) in [(vec2(0.0, 0.0), 0.0f32), (vec2(25.0, -10.0), 0.0), (vec2(-40.0, 15.0), 60.0), (vec2(10.0, 5.0), -80.0)] {
            let mut layer = Layer::new("L", LayerKind::Bitmap);
            layer.depth = depth;
            layer.transform = pose(30.0, -20.0);
            let seen = CameraSeen { offset, dolly, d0: w.d0_px };
            let Some(flat) = layer.transform_seen_at(&seen, 0) else { continue };
            let affine = flat.affine(pivot);
            let xf = w.card_xf(&layer.pose(0), layer.depth_at(0));
            let mx = xf.matrix();
            for p in [pos2(0.0, 0.0), pos2(320.0, 0.0), pos2(320.0, 180.0), pos2(37.0, 121.0)] {
                let want = affine.apply(p);
                let world = crate::scene3dmath::point(&mx, [p.x / 320.0 - 0.5, -(p.y / 180.0 - 0.5), 0.0]);
                let got = w.to_sheet(w.eye_seen(&seen), world).expect("devant l'œil");
                assert!((got - want).length() < 0.05, "profondeur {depth}, caméra {offset:?}/{dolly} : {got:?} contre {want:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------- la caméra, une seule

#[test]
fn the_plan_camera_is_the_3d_camera_in_both_directions() {
    let w = Monde::new([640, 360], [320, 180]);
    for cam in [
        Camera::centered(640, 360),
        Camera { x: 300.0, y: 200.0, zoom: 2.0, angle: 5.0, dolly: 80.0, tilt: -7.0, pan: 12.0 },
        Camera { x: 400.0, y: 100.0, zoom: 0.8, angle: -12.0, dolly: -150.0, tilt: 20.0, pan: -30.0 },
    ] {
        let xf = w.camera_xf(&cam);
        let back = w.camera_from_xf(&cam, &xf);
        for (a, b) in [(cam.x, back.x), (cam.y, back.y), (cam.dolly, back.dolly), (cam.pan, back.pan), (cam.tilt, back.tilt), (cam.angle, back.angle), (cam.zoom, back.zoom)] {
            assert!(near(a, b, 1e-2 * a.abs().max(1.0)), "{cam:?} → {back:?}");
        }
        // L'œil de la scène 3D est celui du plan.
        assert!(near3(w.scene_cam(&cam).pos, w.eye(&cam), 1e-4));
        // Et l'ancienne caméra de blocage donne une caméra du plan qui voit la même chose.
        let sc = w.scene_cam(&cam);
        let plan = w.plan_camera_of(&sc);
        assert!(near3(w.eye(&plan), sc.pos, 1e-3) && near(plan.pan, sc.pan, 1e-3) && near(plan.tilt, sc.tilt, 1e-3) && near(plan.angle, sc.roll, 1e-3), "{plan:?}");
    }
}

// ----------------------------------------------------------------------------- la carte inclinée, rendue

#[test]
fn flat_cards_are_composed_exactly_as_before() {
    // Le plan d'une image est celui d'avant l'Espace 3D : même éléments, mêmes affinités (donc mêmes pixels).
    let mut doc = doc_with_cards(&[(RED, [10, 10, 80, 60], -0.4), (GREEN, [100, 40, 200, 120], 1.5), (BLUE, [20, 100, 60, 170], 0.0)]);
    doc.panels[0].layers[1].transform = pose(15.0, -8.0);
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 180.0, y: 80.0, dolly: 30.0, ..doc.default_camera() })];
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    let camera = doc.camera_seen(0, 0);
    let legacy: Vec<_> = doc.panels[0].layers.iter().filter_map(|l| l.transform_seen_at(&camera, 0).map(|t| (l.cel_at(0).expect("dessin"), (!t.is_identity()).then_some(t)))).collect();
    assert_eq!(plan.items.len(), legacy.len());
    for (item, (cel, transform)) in plan.items.iter().zip(&legacy) {
        assert_eq!((item.cel, item.transform, item.pre.is_none()), (*cel, *transform, true));
    }
}

#[test]
fn a_tilted_card_is_foreshortened_and_projected_where_the_geometry_says() {
    // Un rectangle au milieu de la carte, pour que rien ne sorte de la feuille.
    let mut doc = doc_with_cards(&[(RED, [80, 45, 240, 135], 0.0)]);
    assert_eq!(bbox(&sheet(&doc, 0), 320), Some((80, 45, 240, 135)), "à plat : le rectangle tel quel");
    let w = Monde::of(&doc);
    let projected = |doc: &Document| -> (f32, f32, f32, f32) {
        let layer = &doc.panels[0].layers[0];
        let xf = w.card_xf(&layer.pose(0), layer.depth_at(0));
        let eye = w.eye_seen(&doc.camera_seen(0, 0));
        let mx = xf.matrix();
        let pts: Vec<_> = [(80.0, 45.0), (240.0, 45.0), (240.0, 135.0), (80.0, 135.0)]
            .iter()
            .map(|&(x, y)| w.to_sheet(eye, crate::scene3dmath::point(&mx, [x / 320.0 - 0.5, -(y / 180.0 - 0.5), 0.0])).expect("devant l'œil"))
            .collect();
        let f = |g: fn(f32, f32) -> f32, init: f32, v: fn(&eframe::egui::Pos2) -> f32| pts.iter().map(v).fold(init, g);
        (f(f32::min, f32::MAX, |p| p.x), f(f32::min, f32::MAX, |p| p.y), f(f32::max, f32::MIN, |p| p.x), f(f32::max, f32::MIN, |p| p.y))
    };
    // Inclinaison Y positive : le bord droit s'éloigne, il est plus court que le gauche.
    doc.panels[0].layers[0].transform.tilt_y = 40.0;
    let img = sheet(&doc, 0);
    let (x0, y0, x1, y1) = bbox(&img, 320).expect("carte inclinée");
    let (left, right) = (column_height(&img, 320, 180, x0 + 2), column_height(&img, 320, 180, x1 - 3));
    assert!(left > right + 15, "le bord gauche ({left}) est plus haut que le droit ({right})");
    let (px0, py0, px1, py1) = projected(&doc);
    for (got, want) in [(x0, px0), (y0, py0), (x1, px1), (y1, py1)] {
        assert!((got as f32 - want).abs() <= 1.5, "boîte vue ({x0}, {y0}, {x1}, {y1}) contre coins projetés ({px0}, {py0}, {px1}, {py1})");
    }
    // Inclinaison X positive : le haut vient vers la caméra, il est plus large que le bas.
    doc.panels[0].layers[0].transform = LayerTransform { tilt_x: 35.0, ..Default::default() };
    let img = sheet(&doc, 0);
    let (_, y0, _, y1) = bbox(&img, 320).expect("carte inclinée");
    let row = |y: usize| (0..320).filter(|&x| img[y * 320 + x].a() > 128).count();
    assert!(row(y0 + 2) > row(y1 - 3) + 15, "le haut ({}) est plus large que le bas ({})", row(y0 + 2), row(y1 - 3));
    let (px0, py0, px1, py1) = projected(&doc);
    let (x0, y0b, x1, y1b) = bbox(&img, 320).map(|(a, b, c, d)| (a, b, c, d)).expect("boîte");
    for (got, want) in [(x0, px0), (y0b, py0), (x1, px1), (y1b, py1)] {
        assert!((got as f32 - want).abs() <= 1.5, "boîte vue contre coins projetés ({px0}, {py0}, {px1}, {py1})");
    }
}

#[test]
fn a_card_farther_away_moves_less_when_the_camera_trucks_flat_or_tilted() {
    for tilt in [0.0f32, 30.0] {
        let mut doc = doc_with_cards(&[(RED, [140, 70, 180, 110], -0.5), (GREEN, [140, 70, 180, 110], 2.0)]);
        // Le vert est derrière le rouge : même place, deux profondeurs ; on les voit à part en les posant de côté.
        doc.panels[0].layers[0].transform = LayerTransform { x: -60.0, tilt_y: tilt, ..Default::default() };
        doc.panels[0].layers[1].transform = LayerTransform { x: 60.0, tilt_y: tilt, ..Default::default() };
        let start = Camera::centered(320, 180);
        let moved = Camera { x: start.x + 40.0, ..start };
        doc.panels[0].camera = vec![CameraKey::new(0, start), CameraKey::new(1, moved)];
        let at = |frame: usize, c: Color32| {
            let plan = render::panel_plan_at(&doc, 0, frame, None);
            let cam = doc.panel_camera(0, frame);
            let img = render::camera_image(&doc, &plan, &cam);
            centroid(&img, 320, c).expect("la carte est dans l'image")
        };
        let (r0, r1) = (at(0, RED), at(1, RED));
        let (g0, g1) = (at(0, GREEN), at(1, GREEN));
        let (dr, dg) = ((r1.0 - r0.0).abs(), (g1.0 - g0.0).abs());
        assert!(dr > dg && dg > 0.5, "inclinaison {tilt} : le premier plan bouge de {dr}, le fond de {dg}");
        // Le rapport est celui des distances : (1 + 2) / (1 − 0,5) = 6 (au pixel et à l'inclinaison près).
        let ratio = dr / dg;
        assert!((ratio - 6.0).abs() < 0.6, "inclinaison {tilt} : rapport {ratio} au lieu de 6");
    }
}

#[test]
fn a_tilted_card_keeps_its_effects_opacity_and_blend_on_the_flat_image() {
    let mut doc = doc_with_cards(&[(BLUE, [0, 0, 320, 180], 0.0), (RED, [60, 30, 260, 150], 0.0)]);
    doc.panels[0].layers[1].transform.tilt_y = 30.0;
    doc.panels[0].layers[1].opacity = 0.5;
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    assert_eq!(plan.items[1].opacity, 128, "l'opacité reste celle du calque");
    assert!(plan.items[1].pre.as_ref().is_some_and(|p| p.card && p.warp.is_some()), "le calque est posé par un maillage");
    assert!(plan.items[1].transform.is_none() && plan.items[0].pre.is_none());
    let img = render::composite_full(&doc, &plan);
    let c = img[90 * 320 + 160];
    assert!(c.r() > 60 && c.b() > 60, "le rouge à moitié par-dessus le bleu : {c:?}");
}

#[test]
fn a_card_behind_the_eye_is_not_drawn() {
    let mut doc = doc_with_cards(&[(RED, [0, 0, 320, 180], -0.5)]);
    doc.panels[0].layers[0].transform.tilt_y = 30.0;
    // La caméra avance de 95 % de la distance de référence : la carte, à moins d'elle, est derrière l'œil.
    let d0 = crate::camera::ref_distance(doc.frame);
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { dolly: d0 * 0.9, ..doc.default_camera() })];
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    assert!(plan.items.is_empty() || render::composite_full(&doc, &plan).iter().all(|c| c.a() == 0 || c.r() < 5), "rien ne reste derrière l'œil");
}

#[test]
fn thumbnails_and_the_screen_composition_agree_on_a_tilted_card() {
    let mut doc = doc_with_cards(&[(BLUE, [0, 0, 320, 180], 0.0), (RED, [40, 20, 280, 160], 0.6)]);
    doc.panels[0].layers[1].transform = LayerTransform { tilt_x: 20.0, tilt_y: -25.0, angle: 8.0, ..Default::default() };
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 175.0, y: 85.0, ..doc.default_camera() })];
    let plan = render::panel_plan_at(&doc, 0, 0, Some([255, 255, 255]));
    let full = render::composite_full(&doc, &plan);
    let mut checked = 0;
    for y in (3..180).step_by(7) {
        for x in (3..320).step_by(7) {
            let one = render::composite_pixel(&doc, &plan, x, y);
            let all = full[y * 320 + x];
            assert!(one.to_array().iter().zip(all.to_array()).all(|(a, b)| (*a as i32 - b as i32).abs() <= 1), "({x}, {y}) : {one:?} contre {all:?}");
            checked += 1;
        }
    }
    assert!(checked > 500);
    // La vignette (4 échantillons par pixel) retombe sur la même image.
    let thumb = render::thumbnail(&doc, &plan, &doc.default_camera(), 160, 90);
    let mid = thumb.pixels[45 * 160 + 80];
    assert!(mid.r() > 150 && mid.b() < 120, "le centre de la vignette est dans la carte rouge : {mid:?}");
}

// ----------------------------------------------------------------------------------------------- les clés

#[test]
fn position_keys_carry_the_tilt_and_the_depth_gap() {
    let a = LayerTransform { x: 10.0, tilt_x: 0.0, tilt_y: 10.0, dz: 0.0, ..Default::default() };
    let b = LayerTransform { x: 30.0, tilt_x: 40.0, tilt_y: -10.0, dz: 2.0, ..Default::default() };
    let keys = vec![PosKey::new(0, a), PosKey::new(10, b)];
    let mid = poskeys::pose_at(&keys, 5, LayerTransform::default());
    assert!(near(mid.x, 20.0, 1e-4) && near(mid.tilt_x, 20.0, 1e-4) && near(mid.tilt_y, 0.0, 1e-4) && near(mid.dz, 1.0, 1e-4), "{mid:?}");
    // Avant, après : les poses des clés.
    assert_eq!(poskeys::pose_at(&keys, 0, LayerTransform::default()), a);
    assert_eq!(poskeys::pose_at(&keys, 99, LayerTransform::default()), b);
    // Les adoucis valent aussi pour l'inclinaison : une clé lissée n'est pas à mi-chemin à mi-temps.
    let mut eased = keys.clone();
    eased[1].ease_in = 6.0;
    let slow = poskeys::pose_at(&eased, 5, LayerTransform::default());
    assert!(slow.tilt_x > 21.0, "ralentie à l'arrivée, la pose est plus avancée à mi-chemin : {slow:?}");
    // Le calque lit l'écart de profondeur de ses clés.
    let mut layer = Layer::new("L", LayerKind::Bitmap);
    layer.depth = 1.0;
    layer.pos_keys = keys;
    assert!(near(layer.depth_at(0), 1.0, 1e-5) && near(layer.depth_at(10), 3.0, 1e-5) && near(layer.depth_at(5), 2.0, 1e-5));
}

#[test]
fn a_pose_set_on_an_animated_layer_goes_to_its_key_and_on_a_fixed_one_to_its_fields() {
    let w = Monde::new([320, 180], [320, 180]);
    let mut fixed = Layer::new("F", LayerKind::Bitmap);
    fixed.depth = 0.5;
    let xf = w.card_xf(&fixed.pose(3), fixed.depth_at(3));
    let moved = Xf { pos: [xf.pos[0] + 0.3, xf.pos[1], xf.pos[2] - 1.0], rot: [10.0, -20.0, 0.0], ..xf };
    w.apply_card_xf(&mut fixed, 3, &moved);
    assert!(fixed.pos_keys.is_empty() && fixed.transform.dz == 0.0, "un calque fixe n'a pas d'écart de profondeur");
    assert!(fixed.depth > 0.5 && near(fixed.transform.tilt_x, 10.0, 1e-4) && near(fixed.transform.tilt_y, -20.0, 1e-4), "{:?} {}", fixed.transform, fixed.depth);
    // Animé : la clé de l'image est créée avec l'écart de profondeur.
    let mut anim = Layer::new("A", LayerKind::Bitmap);
    anim.depth = 0.5;
    anim.pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(10, LayerTransform::default())];
    w.apply_card_xf(&mut anim, 5, &moved);
    assert_eq!(anim.pos_keys.len(), 3, "une clé de plus, à l'image 5");
    assert!(near(anim.depth, 0.5, 1e-6), "la profondeur de base ne bouge pas");
    assert!(near(anim.depth_at(5), fixed.depth, 1e-3), "même profondeur finale : {} contre {}", anim.depth_at(5), fixed.depth);
    assert!(near(anim.depth_at(0), 0.5, 1e-6), "les autres clés gardent la leur");
}

// ------------------------------------------------------------------------- le format 35 et les anciens projets

fn dir_for(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-espace3d-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    dir
}

#[test]
fn tilt_depth_gap_and_keys_survive_a_save_and_a_reload() {
    let dir = dir_for("format");
    let manifest = dir.join("P.pelure");
    let mut doc = doc_with_cards(&[(RED, [10, 10, 80, 60], 0.5), (GREEN, [100, 40, 200, 120], 1.0)]);
    doc.panels[0].layers[0].transform = LayerTransform { tilt_x: 12.5, tilt_y: -33.0, ..pose(4.0, 5.0) };
    let moved = LayerTransform { tilt_x: 40.0, tilt_y: 5.0, dz: 1.25, ..pose(20.0, -5.0) };
    doc.panels[0].layers[1].pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey { smooth: true, ..PosKey::new(8, moved) }];
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(text.contains(&format!("\"version\": {}", project::VERSION)) && project::VERSION == 35);
    assert!(text.contains("\"inclinaison_x\": 12.5") && text.contains("\"ecart_profondeur\": 1.25"), "les nouveaux champs sont écrits");
    let back = project::load(&manifest).expect("relu").doc;
    assert_eq!(back.panels[0].layers[0].transform, doc.panels[0].layers[0].transform);
    assert_eq!(back.panels[0].layers[1].pos_keys, doc.panels[0].layers[1].pos_keys);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_project_without_tilt_writes_no_new_field_and_an_older_one_opens_with_the_same_pixels() {
    let dir = dir_for("ancien");
    let manifest = dir.join("P.pelure");
    let mut doc = doc_with_cards(&[(RED, [10, 10, 80, 60], -0.4), (GREEN, [100, 40, 200, 120], 1.5)]);
    doc.panels[0].layers[1].transform = pose(15.0, -8.0);
    doc.panels[0].layers[1].pos_keys = vec![PosKey::new(0, pose(0.0, 0.0)), PosKey::new(6, pose(40.0, 0.0))];
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 180.0, y: 85.0, dolly: 20.0, ..doc.default_camera() })];
    let before: Vec<Vec<Color32>> = (0..7).map(|f| sheet(&doc, f)).collect();
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(!text.contains("inclinaison") && !text.contains("ecart_profondeur"), "rien de nouveau n'est écrit pour une carte à plat");
    // Un projet de format 33 (celui d'avant) : même texte, ancien numéro.
    std::fs::write(&manifest, text.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 33")).expect("écrit");
    let old = project::load(&manifest).expect("un projet d'avant s'ouvre").doc;
    for (f, want) in before.iter().enumerate() {
        assert!(sheet(&old, f) == *want, "l'image {f} d'un projet d'avant est la même, pixel pour pixel");
    }
    assert_eq!(old.panels[0].layers[1].depth_at(3), 1.5);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_project_from_a_newer_format_is_refused_and_the_shared_number_follows_the_classic_one() {
    assert_eq!(project::VERSION, 35);
    assert_eq!(project::version_partage_ecrite(), 35, "un projet partagé écrit le plus grand des deux numéros");
    let dir = dir_for("recent");
    let manifest = dir.join("P.pelure");
    let doc = doc_with_cards(&[(RED, [10, 10, 80, 60], 0.0)]);
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    std::fs::write(&manifest, text.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 36")).expect("écrit");
    let err = project::load(&manifest).err().expect("refusé");
    assert!(err.contains("plus récente"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------- carte graphique contre processeur

#[test]
fn the_gpu_composes_a_tilted_card_like_the_cpu_does() {
    let Some(rs) = crate::gpu_tests::headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let mut doc = doc_with_cards(&[(BLUE, [0, 0, 320, 180], 0.0), (RED, [30, 20, 290, 160], 0.3)]);
    // Un motif, pas un aplat : on veut voir si les coordonnées de texture sont justes.
    for k in 0..12 {
        let cel = doc.panels[0].layers[1].cel_at(0).expect("dessin");
        fill_rect(&mut doc, cel, 40 + k * 20, 30, 50 + k * 20, 150, if k % 2 == 0 { Color32::WHITE } else { GREEN });
    }
    doc.panels[0].layers[1].transform = LayerTransform { tilt_x: 25.0, tilt_y: -35.0, angle: 6.0, ..Default::default() };
    doc.panels[0].layers[1].opacity = 0.8;
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 170.0, y: 95.0, dolly: 40.0, ..doc.default_camera() })];
    let plan = render::panel_plan_at(&doc, 0, 0, Some([255, 255, 255]));
    assert!(plan.items[1].pre.is_some());
    let mut gpu = crate::gpu::GpuCanvas::new(&rs);
    let on_gpu = crate::gpu_tests::render_with(&mut gpu, &rs, &doc, &plan, true);
    let on_cpu = render::composite_full(&doc, &plan);
    let far2 = on_gpu.iter().zip(&on_cpu).filter(|(a, b)| a.to_array().iter().zip(b.to_array()).any(|(p, q)| (*p as i32 - q as i32).abs() > 2)).count();
    let far6 = on_gpu.iter().zip(&on_cpu).filter(|(a, b)| a.to_array().iter().zip(b.to_array()).any(|(p, q)| (*p as i32 - q as i32).abs() > 6)).count();
    let painted = on_cpu.iter().filter(|c| c.g() < 200).count();
    println!("carte inclinée : {far2} pixels à plus de 2 niveaux, {far6} à plus de 6, sur {painted} peints");
    assert!(painted > 20_000, "{painted} pixels peints");
    // (Les bords du motif : le lissage de la carte n'est pas celui du processeur, à un demi-pixel près.)
    assert!(far6 * 100 < painted, "{far6} pixels diffèrent de plus de 6 niveaux");
    assert!(far2 * 25 < painted, "{far2} pixels diffèrent de plus de 2 niveaux");
}

// ---------------------------------------------------------------------------------------- les budgets

#[test]
fn building_the_plan_with_many_tilted_cards_stays_cheap() {
    // Le maillage d'une carte inclinée (65 × 65 points) se refait à chaque image : 10 cartes doivent tenir en
    // quelques millisecondes (la composition, elle, est faite par la carte graphique).
    let cards: Vec<_> = (0..10).map(|k| (RED, [k * 10, 10, k * 10 + 60, 120], k as f32 * 0.3)).collect();
    let mut doc = doc_with_cards(&cards);
    for (k, l) in doc.panels[0].layers.iter_mut().enumerate() {
        l.transform = LayerTransform { tilt_x: 10.0 + k as f32, tilt_y: -20.0 + 3.0 * k as f32, ..Default::default() };
    }
    let start = std::time::Instant::now();
    let n = 20;
    for _ in 0..n {
        let plan = render::panel_plan_at(&doc, 0, 3, None);
        assert_eq!(plan.items.len(), 10);
    }
    let per_plan = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("PERF plan de 10 cartes inclinées : {per_plan:.2} ms");
    assert!(per_plan < 12.0, "{per_plan:.2} ms pour construire le plan de 10 cartes inclinées");
}

#[test]
fn layer_selection_numbers_are_told_from_objects_and_the_camera() {
    let s = espace3d::card_selection(42);
    assert_eq!(espace3d::card_of(s), Some(42));
    assert_eq!(espace3d::card_of(42), None, "un petit numéro est un objet");
    assert_eq!(espace3d::card_of(crate::scene3dgizmo::CAMERA_ID), None, "la caméra n'est pas une carte");
    // Les calques qui ne sont pas des cartes.
    let mut folder = Layer::new_folder("D");
    assert!(!espace3d::is_card(&folder));
    folder.folder = false;
    assert!(espace3d::is_card(&folder));
}

// ======================================================================= l'application : l'Espace 3D

/// Une application sans fenêtre dont le projet (feuille = cadre, 320 × 180) a ces cartes, l'Espace 3D ouvert et la vue
/// de face, centrée, sans quadrillage ni attente d'un affichage de plus.
fn space(cards: &[(Color32, [usize; 4], f32)]) -> Banc {
    let mut b = banc_taille(doc_with_cards(cards), false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.app.scene3d.show_grid = false;
    b.app.scene3d.snap = false;
    b.chauffer(4);
    b.app.scene3d.cam = Some(Cam3d { target: [0.0, 0.0, 0.0], azimuth: 0.0, elevation: 0.0, distance: 12.0, focal: 35.0 });
    b.chauffer(2);
    b
}

fn orbit(b: &Banc) -> (crate::scene3dmath::View, [f32; 2]) {
    let rect = b.app.canvas_rect;
    (crate::scene3dview::orbit_view(&b.app.scene3d.cam.expect("vue"), rect.width()), [rect.width() / 2.0, rect.height() / 2.0])
}

/// Le geste d'un manipulateur : de l'écran `from` à l'écran `to` (relatifs à la zone de dessin), en une seule étape
/// d'annulation, avec ce nom.
fn gesture(b: &mut Banc, handle: Handle, from: [f32; 2], to: [f32; 2]) {
    let (view, center) = orbit(b);
    let gizmo = b.app.s3_gizmo(&view).expect("un manipulateur");
    let mode = b.app.scene3d.mode;
    b.app.s3_begin(handle, mode, gizmo, view.ray(from, center), from, "Geste d'essai");
    b.app.s3_drag(&view, center, to);
    b.app.s3_finish_drag();
}

#[test]
fn one_3d_button_one_command_and_the_old_names_still_work() {
    // Les anciens noms du plateau en 3D et de la scène 3D sont des doubles de la commande unique.
    for (alias, original) in [("vue.3d", "espace3d.basculer"), ("scene3d.basculer", "espace3d.basculer"), ("vue3d.recadrer", "scene3d.cadrer"), ("outil3d.deplacer", "scene3d.outil_deplacer"), ("outil3d.orbite", "scene3d.outil_choisir")] {
        assert!(crate::keymap::ALIASES.contains(&(alias, original)), "{alias} double {original}");
        assert!(crate::keymap::action_info(alias).is_some_and(|a| a.implemented));
    }
    let commands = crate::palette::palette_commands();
    assert_eq!(commands.iter().filter(|(_, label)| label.starts_with("Espace 3D (")).count(), 1, "une seule commande pour l'Espace 3D dans la palette : {commands:?}");
    assert!(!commands.iter().any(|(_, label)| label.contains("Plateau en 3D") || label.contains("Scène 3D")), "plus de plateau en 3D ni de scène 3D dans la palette");
    let mut b = banc_taille(doc_with_cards(&[(RED, [10, 10, 80, 60], 0.0)]), false, 1280, 720);
    b.chauffer(2);
    for name in ["vue.3d", "espace3d.basculer", "scene3d.basculer"] {
        let was = b.app.scene3d.on;
        b.app.run_action(name);
        assert_eq!(b.app.scene3d.on, !was, "{name} ouvre ou ferme l'Espace 3D");
    }
    // Les outils du plateau en 3D mènent aux outils de l'espace.
    b.app.run_action("outil3d.deplacer");
    assert!(b.app.scene3d.on && b.app.scene3d.mode == Mode::Move);
}

#[test]
fn the_ray_picks_the_nearest_drawn_card_and_a_click_chooses_it_and_its_layer() {
    use eframe::egui::{Event, Modifiers, PointerButton};
    let mut b = space(&[(BLUE, [0, 0, 320, 180], 1.5), (RED, [100, 50, 220, 130], 0.0)]);
    let (view, center) = orbit(&b);
    let (red, blue) = (b.app.layers()[1].id, b.app.layers()[0].id);
    let centre = view.ray(view.project([0.0, 0.0, 0.0], center).expect("visible"), center);
    assert_eq!(b.app.s3_pick(&centre), Some(espace3d::card_selection(red)), "le rouge, plus près, est devant le bleu");
    // Hors du rouge (sur le bleu seulement).
    let edge = view.ray(view.project([-2.5, 0.0, 0.0], center).expect("visible"), center);
    assert_eq!(b.app.s3_pick(&edge), Some(espace3d::card_selection(blue)));
    // Un clic de souris choisit la carte et fait de son calque le calque courant.
    b.app.scene3d.mode = Mode::Select;
    b.app.select_layer(0);
    b.chauffer(2);
    assert_eq!(b.app.scene3d.sel, vec![espace3d::card_selection(blue)], "le choix suit le panneau Calques");
    let rect = b.app.canvas_rect;
    let p = view.project([0.0, 0.0, 0.0], center).expect("visible");
    let at = pos2(rect.left() + p[0], rect.top() + p[1]);
    let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(at)], 0.02);
    b.image(vec![button(at, true)], 0.02);
    b.image(vec![button(at, false)], 0.02);
    b.chauffer(2);
    assert_eq!(b.app.scene3d.sel, vec![espace3d::card_selection(red)], "le clic choisit la carte rouge");
    assert_eq!(b.app.layer().id, red, "son calque devient le calque courant");
    assert!(b.app.history.names().0.is_empty(), "choisir n'est pas une étape d'annulation");
}

#[test]
fn a_card_is_moved_tilted_and_scaled_by_the_same_handles_as_an_object_each_gesture_one_undo_step() {
    let mut b = space(&[(RED, [100, 50, 220, 130], 0.0)]);
    let card = espace3d::card_selection(b.app.layers()[0].id);
    b.app.scene3d.sel = vec![card];
    let (view, center) = orbit(&b);
    let w = b.app.s3_world();
    let before = b.app.history.names().0.len();
    // Déplacer le long de X : une carte à 12 m, 0,5 m vers la droite.
    b.app.scene3d.mode = Mode::Move;
    let (from, to) = (view.project([0.0, 0.0, 0.0], center).expect("visible"), view.project([0.5, 0.0, 0.0], center).expect("visible"));
    gesture(&mut b, Handle::Axis(0), from, to);
    let t = b.app.layers()[0].pose(0);
    assert!(near(t.x, 0.5 / w.k, 0.8), "0,5 m = {} px de la feuille : {}", 0.5 / w.k, t.x);
    assert_eq!(b.app.history.names().0.len(), before + 1, "un geste, une étape");
    b.app.undo();
    assert_eq!(b.app.layers()[0].pose(0), LayerTransform::default(), "annuler rend la carte telle qu'elle était");
    b.app.redo();
    // En profondeur (axe Z) : la profondeur du calque suit, la taille dans le monde reste.
    let size_before = w.card_xf(&b.app.layers()[0].pose(0), b.app.layers()[0].depth_at(0)).size;
    let xfs = b.app.s3_xfs(&[card]);
    b.app.s3_set_xfs(&gz::move_xfs(&xfs, [0.0, 0.0, -2.0]));
    let l = &b.app.layers()[0];
    assert!(l.depth > 0.3 && near(l.depth, 2.0 / w.d0, 1e-3), "reculée de 2 m : profondeur {}", l.depth);
    let size_after = w.card_xf(&l.pose(0), l.depth_at(0)).size;
    assert!(near3(size_after, size_before, 1e-3), "la taille dans le monde est gardée : {size_before:?} → {size_after:?}");
    // (Remise à sa place : le reste de l'essai part d'une carte à profondeur 0.)
    let xfs = b.app.s3_xfs(&[card]);
    b.app.s3_set_xfs(&gz::move_xfs(&xfs, [0.0, 0.0, 2.0]));
    assert!(b.app.layers()[0].depth.abs() < 1e-3);
    // Tourner autour de Y : l'inclinaison de la carte. (Vu de face, à la hauteur de la carte, le cercle est de chant : on
    // regarde un peu de côté et d'au-dessus.)
    b.app.scene3d.cam = Some(Cam3d { target: [0.0, 0.0, 0.0], azimuth: 35.0, elevation: 25.0, distance: 12.0, focal: 35.0 });
    b.app.scene3d.mode = Mode::Rotate;
    let (view, center) = orbit(&b);
    let g = b.app.s3_gizmo(&view).expect("manipulateur");
    let ring = |deg: f32| {
        let t = deg.to_radians();
        let p = m::add(g.origin, m::scale(m::add(m::scale(g.axes[2], t.cos()), m::scale(g.axes[0], t.sin())), g.len));
        view.project(p, center).expect("visible")
    };
    gesture(&mut b, Handle::Ring(1), ring(0.0), ring(30.0));
    let t = b.app.layers()[0].pose(0);
    assert!(near(t.tilt_y, 30.0, 0.5) && t.tilt_x.abs() < 0.01, "inclinée de 30° autour de Y : {t:?}");
    assert!(near(t.angle, 0.0, 0.01), "l'angle dans le plan ne bouge pas");
    // Échelle uniforme (cube du centre) : plus grande.
    b.app.scene3d.mode = Mode::Scale;
    let (view, center) = orbit(&b);
    let o = view.project(w.card_xf(&b.app.layers()[0].pose(0), b.app.layers()[0].depth_at(0)).pos, center).expect("visible");
    let scale0 = b.app.layers()[0].pose(0).scale;
    gesture(&mut b, Handle::Center, o, [o[0], o[1] - 60.0]);
    assert!(b.app.layers()[0].pose(0).scale > scale0 * 1.5, "agrandie : {}", b.app.layers()[0].pose(0).scale);
    // Tout est annulable, un geste après l'autre.
    for _ in 0..3 {
        b.app.undo();
    }
    assert_eq!(b.app.layers()[0].pose(0), LayerTransform::default());
}

#[test]
fn a_locked_card_does_not_move_and_says_why() {
    let mut b = space(&[(RED, [100, 50, 220, 130], 0.0), (GREEN, [10, 10, 60, 60], 0.0)]);
    b.app.layers_mut()[0].locked = true;
    let card = espace3d::card_selection(b.app.layers()[0].id);
    let xfs = b.app.s3_xfs(&[card]);
    b.app.s3_set_xfs(&gz::move_xfs(&xfs, [1.0, 1.0, 0.0]));
    assert_eq!(b.app.layers()[0].pose(0), LayerTransform::default(), "un calque verrouillé ne bouge pas");
    b.app.layers_mut()[0].locked = false;
    b.app.layers_mut()[0].position_locked = true;
    b.app.s3_set_xfs(&gz::move_xfs(&xfs, [1.0, 1.0, 0.0]));
    assert_eq!(b.app.layers()[0].pose(0), LayerTransform::default(), "ni un calque de position verrouillée");
    b.app.layers_mut()[0].position_locked = false;
    b.app.s3_set_xfs(&gz::move_xfs(&xfs, [1.0, 0.0, 0.0]));
    assert!(b.app.layers()[0].pose(0).x != 0.0);
}

#[test]
fn an_animated_card_moves_its_key_and_the_camera_is_the_plan_camera() {
    let mut b = space(&[(RED, [100, 50, 220, 130], 0.0)]);
    let w = b.app.s3_world();
    // La carte animée : tirée à l'image 5, elle règle (crée) la clé de l'image 5.
    b.app.layers_mut()[0].pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(10, LayerTransform::default())];
    b.app.cur_frame = 5;
    let card = espace3d::card_selection(b.app.layers()[0].id);
    let xfs = b.app.s3_xfs(&[card]);
    b.app.s3_set_xfs(&gz::move_xfs(&xfs, [0.4, 0.0, -1.0]));
    let l = &b.app.layers()[0];
    assert_eq!(l.pos_keys.len(), 3, "une clé à l'image 5");
    assert!(l.pos_keys[1].frame == 5 && l.pos_keys[1].t.x > 0.0 && l.pos_keys[1].t.dz > 0.0, "{:?}", l.pos_keys[1]);
    assert_eq!(l.depth, 0.0, "la profondeur de base ne bouge pas");
    // La caméra : choisie, elle se déplace par les mêmes poignées et c'est la caméra du plan qui bouge.
    b.app.cur_frame = 0;
    assert!(b.app.doc.panels[0].camera.is_empty());
    let cam_xf = b.app.s3_xfs(&[CAMERA_ID]);
    assert!(near3(cam_xf[0].1.pos, w.eye(&b.app.current_camera()), 1e-4), "l'œil de la caméra du plan");
    b.app.s3_set_xfs(&gz::move_xfs(&cam_xf, [0.5, 0.2, -1.0]));
    let cam = b.app.current_camera();
    let base = Camera::centered(320, 180);
    assert!(near(cam.x, base.x + 0.5 / w.k, 0.1) && near(cam.y, base.y - 0.2 / w.k, 0.1) && near(cam.dolly, 1.0 / w.k, 0.1), "{cam:?}");
    // Animée (deux clés) : on règle la clé de l'image courante.
    b.app.doc.panels[0].camera = vec![CameraKey::new(0, base), CameraKey::new(10, base)];
    b.app.cur_frame = 4;
    let cam_xf = b.app.s3_xfs(&[CAMERA_ID]);
    b.app.s3_set_xfs(&gz::move_xfs(&cam_xf, [1.0, 0.0, 0.0]));
    let keys = &b.app.doc.panels[0].camera;
    assert_eq!(keys.len(), 3, "une clé de caméra de plus, à l'image 4 : {keys:?}");
    assert!(keys[1].frame == 4 && keys[1].cam.x > base.x + 10.0);
    assert_eq!(keys[0].cam, base);
}

#[test]
fn the_camera_gesture_is_one_named_undo_step_and_undo_restores_the_plan_camera() {
    let mut b = space(&[(RED, [100, 50, 220, 130], 0.0)]);
    b.app.scene3d.sel = vec![CAMERA_ID];
    b.app.scene3d.mode = Mode::Move;
    let (view, center) = orbit(&b);
    let before = b.app.doc.panels[0].camera.clone();
    let steps = b.app.history.names().0.len();
    let eye = b.app.s3_world().eye(&b.app.current_camera());
    let (from, to) = (view.project(eye, center).expect("visible"), view.project([eye[0] + 0.3, eye[1], eye[2]], center).expect("visible"));
    gesture(&mut b, Handle::Axis(0), from, to);
    assert_ne!(b.app.doc.panels[0].camera, before);
    assert_eq!(b.app.history.names().0.len(), steps + 1);
    b.app.undo();
    assert_eq!(b.app.doc.panels[0].camera, before);
}

#[test]
fn the_pile_is_ordered_by_depth_in_one_step_and_refused_when_it_would_break_links() {
    let mut b = space(&[(RED, [10, 10, 80, 60], 1.0), (GREEN, [100, 40, 200, 120], -0.5), (BLUE, [20, 100, 60, 170], 2.0)]);
    let names: Vec<String> = b.app.layers().iter().map(|l| l.name.clone()).collect();
    b.app.run_action("espace3d.ordonner_profondeur");
    let depths: Vec<f32> = b.app.layers().iter().map(|l| l.depth).collect();
    assert_eq!(depths, vec![2.0, 1.0, -0.5], "le plus loin en bas de la pile");
    assert_eq!(b.app.history.names().0.last().copied(), Some("Ordonner la pile selon la profondeur"));
    b.app.undo();
    assert_eq!(b.app.layers().iter().map(|l| l.name.clone()).collect::<Vec<_>>(), names);
    // Un dossier dans la pile : refusé, rien ne change.
    b.app.layers_mut().push(Layer::new_folder("Dossier"));
    let n = b.app.history.names().0.len();
    b.app.run_action("espace3d.ordonner_profondeur");
    assert_eq!(b.app.history.names().0.len(), n);
    assert!(b.app.status.contains("refusé"), "{}", b.app.status);
}

#[test]
fn drawing_on_a_tilted_card_follows_the_tilt_both_ways() {
    let mut doc = doc_with_cards(&[(RED, [0, 0, 320, 180], 0.4)]);
    doc.panels[0].layers[0].transform = LayerTransform { tilt_x: 20.0, tilt_y: -30.0, angle: 5.0, x: 6.0, ..Default::default() };
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 170.0, y: 95.0, dolly: 30.0, ..doc.default_camera() })];
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    let (world, xf, eye) = (b.app.s3_world(), b.app.s3_world().card_xf(&b.app.layer().pose(0), b.app.layer().depth_at(0)), b.app.s3_world().eye_seen(&b.app.doc.camera_seen(0, 0)));
    for p in [pos2(160.0, 90.0), pos2(40.0, 30.0), pos2(300.0, 150.0)] {
        let sheet = world.card_to_sheet(eye, &xf, p).expect("devant l'œil");
        let back = b.app.to_layer(sheet);
        assert!((back - p).length() < 0.05, "le pinceau peint en {back:?} au lieu de {p:?}");
        let screen = b.app.layer_to_screen(p);
        let again = b.app.view.to_canvas(b.app.canvas_rect, b.app.canvas_size(), screen);
        assert!((again - sheet).length() < 0.2, "{again:?} contre {sheet:?}");
    }
}

#[test]
fn the_view_with_ten_cards_and_twenty_objects_stays_within_the_frame_budget() {
    let cards: Vec<_> = (0..10).map(|k| (if k % 2 == 0 { RED } else { GREEN }, [10 + k * 20, 20, 90 + k * 20, 140], k as f32 * 0.4 - 1.0)).collect();
    let mut doc = doc_with_cards(&cards);
    let (panel, seq) = (doc.panels[0].id, doc.sequence_of(0));
    let shapes = Shape3d::basics();
    for k in 0..20 {
        doc.scenes3d.add(panel, seq, Target::Plan, shapes[k % shapes.len()].clone(), [(k % 5) as f32 * 1.4 - 3.0, 0.0, -((k / 5) as f32) * 1.2]);
    }
    let mut b = banc_taille(doc, true, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.chauffer(6);
    let ids: Vec<u64> = b.app.layers().iter().map(|l| espace3d::card_selection(l.id)).collect();
    b.app.scene3d.sel = ids;
    let frames: Vec<f64> = (0..60).map(|k| b.image(vec![eframe::egui::Event::PointerMoved(pos2(900.0 + k as f32, 500.0))], 1.0 / 60.0).total()).collect();
    let mean = frames.iter().sum::<f64>() / frames.len() as f64;
    println!("PERF Espace 3D : 10 cartes, 20 objets, carte graphique {} : {mean:.2} ms par image (max {:.2})", b.app.gpu.is_some(), frames.iter().cloned().fold(0.0, f64::max));
    assert!(mean < 16.0, "{mean:.2} ms par image : l'Espace 3D doit tenir 60 images par seconde (10 cartes, 20 objets)");
    // Choisir à la souris, aussi.
    let (view, center) = orbit(&b);
    let ray = view.ray(center, center);
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let _ = b.app.s3_pick(&ray);
    }
    let pick = t.elapsed().as_secs_f64() * 1000.0 / 20.0;
    println!("PERF Espace 3D : choisir à la souris (10 cartes, 20 objets) : {pick:.2} ms");
    assert!(pick < 5.0, "{pick:.2} ms pour choisir");
}