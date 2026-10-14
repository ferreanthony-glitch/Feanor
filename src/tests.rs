use eframe::egui::{pos2, vec2, Color32};

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::document::{Document, Layer, LayerKind};
use crate::fill;
use crate::history::{Edit, History, Nav};
use crate::io;
use crate::raster::{premultiply, unpremultiply, Raster};
use crate::render::{self, Plan, PlanItem};
use crate::vector::{self, Affine, Element, VPoint, VectorDrawing};

fn draw_line(doc: &mut Document, cel: u64, from: (f32, f32), to: (f32, f32), settings: BrushSettings) -> Option<Edit> {
    let mut stroke = StrokeState::new(cel, StrokeMode::Paint([0, 0, 0]), settings, None, false);
    let raster = &mut doc.cels.get_mut(&cel).unwrap().raster;
    let mut dirty = Vec::new();
    for i in 0..=20 {
        let t = i as f32 / 20.0;
        let p = pos2(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        stroke.add_point(raster, p, 1.0, &mut dirty);
    }
    stroke.finish(raster, &mut dirty)
}

fn line(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Element {
    Element::Stroke {
        points: vec![VPoint { x: x0, y: y0, r }, VPoint { x: x1, y: y1, r }],
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
    }
}

fn nav() -> Nav {
    Nav { shot: 0, layer: 0 }
}

#[test]
fn exposures() {
    let mut l = Layer::new("t", LayerKind::Bitmap);
    l.frames = vec![Some(1), Some(1), Some(2), None, Some(3), Some(3), Some(3)];
    assert_eq!(l.run_bounds(0), (0, 2));
    assert_eq!(l.run_bounds(1), (0, 2));
    assert_eq!(l.run_bounds(2), (2, 3));
    assert_eq!(l.run_bounds(5), (4, 7));
    assert_eq!(l.run_bounds(9), (9, 10));
    assert_eq!(l.empty_run_end(3, 10), 4);
    assert_eq!(l.empty_run_end(7, 10), 10);
    l.insert(2, Some(9), 2);
    assert_eq!(l.frames[..5], [Some(1), Some(1), Some(9), Some(9), Some(2)]);
    l.insert(12, Some(4), 1);
    assert_eq!(l.frames.len(), 13);
    l.frames.push(None);
    l.trim();
    assert_eq!(l.frames.last(), Some(&Some(4)));
}

#[test]
fn stroke_paints_and_undoes() {
    let mut doc = Document::new(200, 150, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let edit = draw_line(&mut doc, cel, (10.0, 10.0), (190.0, 140.0), BrushSettings::brush()).unwrap();
    assert_eq!(doc.cels[&cel].raster.get(100, 75).a(), 255, "le centre du trait doit être opaque");
    assert_eq!(doc.cels[&cel].raster.get(190, 10).a(), 0, "loin du trait, rien");

    let mut history = History::default();
    history.push(edit);
    let mut n = nav();
    assert!(history.undo(&mut doc, &mut n));
    assert!(doc.cels[&cel].is_empty(), "annuler efface le trait");
    assert!(history.redo(&mut doc, &mut n));
    assert_eq!(doc.cels[&cel].raster.get(100, 75).a(), 255, "rétablir le remet");
}

#[test]
fn half_opacity_stroke_is_uniform() {
    let mut doc = Document::new(120, 60, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut s = BrushSettings::brush();
    s.opacity = 0.5;
    s.size = 12.0;
    s.hardness = 1.0;
    s.spacing = 0.05;
    draw_line(&mut doc, cel, (10.0, 30.0), (110.0, 30.0), s);
    // Malgré des dizaines d'empreintes superposées, le trait reste à 50 %.
    let a = doc.cels[&cel].raster.get(60, 30).a();
    assert!((126..=129).contains(&a), "alpha {a}");
}

#[test]
fn fill_stops_at_lines() {
    let (w, h) = (50, 50);
    let mut src = vec![Color32::TRANSPARENT; w * h];
    for i in 0..w {
        src[25 * w + i] = Color32::BLACK; // ligne horizontale
    }
    let region = fill::flood(&src, w, h, 5, 5, 10);
    assert!(region[5 * w + 40]);
    assert!(!region[25 * w + 5]);
    assert!(!region[40 * w + 5]);
    assert_eq!(region.iter().filter(|&&b| b).count(), 25 * w);
    let grown = fill::dilate(&region, w, h, 1);
    assert!(grown[25 * w + 5] && !grown[26 * w + 5]);

    // Le débord passe sous le trait au lieu de le recouvrir.
    let mut raster = Raster::new(w, h);
    raster.tile_mut(0)[25 * 64 + 3] = Color32::BLACK;
    fill::apply(&mut raster, &region, &grown, [255, 0, 0]);
    assert_eq!(raster.get(3, 25), Color32::BLACK);
    assert_eq!(raster.get(3, 3), Color32::from_rgb(255, 0, 0));
}

#[test]
fn onion_tint_and_opacity() {
    let mut doc = Document::new(64, 64, 24);
    let a = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&a).unwrap().raster.tile_mut(0)[0] = Color32::BLACK;
    let plan = Plan {
        paper: Some([255, 255, 255]),
        items: vec![PlanItem {
            cel: a,
            opacity: 128,
            tint: Some([255, 0, 0]),
        }],
    };
    let c = render::composite_pixel(&doc, &plan, 0, 0);
    assert_eq!(c.r(), 255);
    assert!((125..=130).contains(&c.g()), "{c:?}");
}

#[test]
fn premultiply_roundtrip() {
    for &(r, g, b, a) in &[(255u8, 128u8, 0u8, 255u8), (200, 100, 50, 128), (10, 20, 30, 0)] {
        let back = unpremultiply(premultiply(r, g, b, a));
        if a == 0 {
            assert_eq!(back, [0, 0, 0, 0]);
        } else {
            for (x, y) in [r, g, b].iter().zip(&back[..3]) {
                assert!((*x as i32 - *y as i32).abs() <= 2);
            }
        }
    }
}

// ------------------------------------------------------------------------------ vectoriel

#[test]
fn vector_stroke_renders_and_hits() {
    let d = VectorDrawing {
        elements: vec![line(10.0, 20.0, 90.0, 20.0, 3.0)],
    };
    let r = d.render(100, 40);
    assert_eq!(r.get(50, 20).a(), 255);
    assert_eq!(r.get(50, 30).a(), 0);
    assert_eq!(d.hit_test(pos2(50.0, 22.0), 0.5), Some(0));
    assert_eq!(d.hit_test(pos2(50.0, 30.0), 0.5), None);
}

#[test]
fn simplify_keeps_shape() {
    let pts: Vec<VPoint> = (0..=100)
        .map(|i| VPoint { x: i as f32, y: 5.0, r: 2.0 })
        .chain((1..=100).map(|i| VPoint { x: 100.0, y: 5.0 + i as f32, r: 2.0 }))
        .collect();
    let s = vector::simplify(&pts, 0.25);
    assert_eq!(s.len(), 3, "deux segments droits : trois points suffisent");
    assert_eq!((s[1].x, s[1].y), (100.0, 5.0));
}

#[test]
fn eraser_cuts_stroke_in_two() {
    let mut d = VectorDrawing {
        elements: vec![line(0.0, 10.0, 100.0, 10.0, 2.0)],
    };
    assert!(vector::erase_circle(&mut d, pos2(50.0, 10.0), 5.0, false));
    assert_eq!(d.elements.len(), 2);
    // Rien au milieu, les deux bouts sont toujours là.
    assert!(d.hit_test(pos2(50.0, 10.0), 0.0).is_none());
    assert!(d.hit_test(pos2(10.0, 10.0), 0.0).is_some());
    assert!(d.hit_test(pos2(90.0, 10.0), 0.0).is_some());
    // Mode « trait entier ».
    assert!(vector::erase_circle(&mut d, pos2(10.0, 10.0), 2.0, true));
    assert_eq!(d.elements.len(), 1);
    assert!(!vector::erase_circle(&mut d, pos2(50.0, 80.0), 5.0, false));
}

#[test]
fn vector_fill_follows_region_with_hole() {
    // Un anneau : carré de 40 px troué d'un carré de 10 px.
    let (w, h) = (60, 60);
    let mut mask = vec![false; w * h];
    for y in 10..50 {
        for x in 10..50 {
            mask[y * w + x] = !(25..35).contains(&x) || !(25..35).contains(&y);
        }
    }
    let contours = vector::trace_contours(&mask, w, h);
    assert_eq!(contours.len(), 2, "un contour extérieur et un trou");
    let fill = Element::Fill {
        contours,
        color: [255, 0, 0],
        opacity: 1.0,
    };
    let r = VectorDrawing { elements: vec![fill.clone()] }.render(w, h);
    assert_eq!(r.get(15, 15).a(), 255);
    assert_eq!(r.get(30, 30).a(), 0, "le trou reste vide");
    assert_eq!(r.get(5, 5).a(), 0);
    assert!(fill.hit(pos2(15.0, 15.0), 0.0));
    assert!(!fill.hit(pos2(30.0, 30.0), 0.0));
}

#[test]
fn affine_transforms() {
    let e = line(0.0, 0.0, 10.0, 0.0, 1.0);
    let moved = e.transformed(&Affine::translate(vec2(5.0, 5.0)));
    assert!(moved.hit(pos2(10.0, 5.0), 0.1));
    let scaled = e.transformed(&Affine::scale_about(pos2(0.0, 0.0), 2.0, 2.0));
    let Element::Stroke { points, .. } = &scaled else { unreachable!() };
    assert_eq!((points[1].x, points[1].r), (20.0, 2.0));
    let turned = e.transformed(&Affine::rotate_about(pos2(0.0, 0.0), std::f32::consts::FRAC_PI_2));
    let Element::Stroke { points, .. } = &turned else { unreachable!() };
    assert!(points[1].x.abs() < 1e-4 && (points[1].y - 10.0).abs() < 1e-4);
}

#[test]
fn vector_undo_restores_drawing() {
    let mut doc = Document::new(64, 64, 24);
    let cel = doc.new_cel(LayerKind::Vector);
    let before = doc.cels[&cel].vector.clone().unwrap();
    let c = doc.cels.get_mut(&cel).unwrap();
    c.vector.as_mut().unwrap().elements.push(line(5.0, 5.0, 60.0, 5.0, 2.0));
    c.rerender();
    assert!(!doc.cels[&cel].is_empty());
    let mut history = History::default();
    history.push(Edit::Vector { cel, drawing: before });
    let mut n = nav();
    history.undo(&mut doc, &mut n);
    assert!(doc.cels[&cel].is_empty());
    assert_eq!(doc.cels[&cel].raster.get(30, 5).a(), 0, "l'image suit le vectoriel");
    history.redo(&mut doc, &mut n);
    assert_eq!(doc.cels[&cel].raster.get(30, 5).a(), 255);
}

// ------------------------------------------------------------------------------ plans

#[test]
fn shot_length_holds_last_drawing() {
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_cel(LayerKind::Vector);
    let shot = &mut doc.shots[0];
    assert_eq!(shot.frame_count(), 48, "un plan neuf dure deux secondes");
    shot.layers[0].frames = vec![Some(a); 48];
    shot.set_length(60);
    assert_eq!(shot.frame_count(), 60);
    assert_eq!(shot.layers[0].cel_at(59), Some(a), "le dessin tenu suit la nouvelle fin");
    shot.set_length(12);
    assert_eq!(shot.frame_count(), 12);
    assert_eq!(shot.layers[0].frames.len(), 12);
}

#[test]
fn montage_positions() {
    let mut doc = Document::new(32, 32, 10);
    let s = doc.make_shot();
    doc.shots.push(s);
    doc.shots[0].set_length(5);
    doc.shots[1].set_length(8);
    assert_eq!(doc.total_frames(), 13);
    assert_eq!(doc.shot_start(1), 5);
    assert_eq!(doc.locate(0), (0, 0));
    assert_eq!(doc.locate(4), (0, 4));
    assert_eq!(doc.locate(5), (1, 0));
    assert_eq!(doc.locate(99), (1, 7));
}

#[test]
fn duplicated_shot_owns_its_drawings() {
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_cel(LayerKind::Vector);
    doc.shots[0].layers[0].frames = vec![Some(a), Some(a)];
    let copy = doc.duplicate_shot(0);
    let b = copy.layers[0].frames[0].unwrap();
    assert_ne!(a, b);
    assert_eq!(copy.layers[0].frames, vec![Some(b), Some(b)], "l'exposition est conservée");
    assert_ne!(copy.id, doc.shots[0].id);
}

#[test]
fn structure_undo_restores_shots() {
    let mut doc = Document::new(32, 32, 24);
    let snap = Edit::Structure {
        shots: doc.shots.clone(),
        nav: nav(),
    };
    let s = doc.make_shot();
    doc.shots.push(s);
    let mut history = History::default();
    history.push(snap);
    let mut n = Nav { shot: 1, layer: 0 };
    history.undo(&mut doc, &mut n);
    assert_eq!(doc.shots.len(), 1);
    assert_eq!(n.shot, 0);
    history.redo(&mut doc, &mut n);
    assert_eq!(doc.shots.len(), 2);
    assert_eq!(n.shot, 1);
}

#[test]
fn structure_undo_keeps_texts_and_layer_settings() {
    let mut doc = Document::new(32, 32, 24);
    let snap = Edit::Structure {
        shots: doc.shots.clone(),
        nav: nav(),
    };
    // Après l'instantané : on écrit le dialogue, on règle le calque, et on rallonge le plan.
    doc.shots[0].dialogue = "— Toi ici ?".into();
    doc.shots[0].layers[0].opacity = 0.3;
    doc.shots[0].layers[0].name = "Crayonné".into();
    doc.shots[0].set_length(100);
    let mut history = History::default();
    history.push(snap);
    let mut n = nav();
    history.undo(&mut doc, &mut n);
    assert_eq!(doc.shots[0].frame_count(), 48, "la durée est bien annulée");
    assert_eq!(doc.shots[0].dialogue, "— Toi ici ?", "le texte saisi est conservé");
    assert_eq!(doc.shots[0].layers[0].opacity, 0.3);
    assert_eq!(doc.shots[0].layers[0].name, "Crayonné");
    history.redo(&mut doc, &mut n);
    assert_eq!(doc.shots[0].frame_count(), 100);
    assert_eq!(doc.shots[0].dialogue, "— Toi ici ?");
}

/// Projet de démonstration (trois plans) : `PELURE_DEMO=chemin.pelure cargo test -- --ignored demo`.
#[test]
#[ignore]
fn demo_project() {
    let Ok(out) = std::env::var("PELURE_DEMO") else { return };
    let mut doc = Document::new(1920, 1080, 24);
    let circle = |cx: f32, cy: f32, r: f32, w: f32| Element::Stroke {
        points: (0..=48)
            .map(|k| {
                let t = k as f32 / 48.0 * std::f32::consts::TAU;
                VPoint { x: cx + r * t.cos(), y: cy + r * t.sin(), r: w * (0.6 + 0.4 * (t * 0.5).sin().abs()) }
            })
            .collect(),
        color: [24, 24, 28],
        opacity: 1.0,
        hardness: 0.9,
    };
    let titles = [("Arrivée", "Le bus s'arrête. Brume.", ""), ("Gros plan", "Elle lève les yeux.", "— C'est ici ?"), ("Large", "La station, vide.", "")];
    for (i, (name, action, dialogue)) in titles.iter().enumerate() {
        if i > 0 {
            let s = doc.make_shot();
            doc.shots.push(s);
        }
        let cel = doc.new_cel(LayerKind::Vector);
        let c = doc.cels.get_mut(&cel).unwrap();
        let d = c.vector.as_mut().unwrap();
        d.elements.push(Element::Fill {
            contours: vec![vec![[0.0, 760.0], [1920.0, 700.0], [1920.0, 1080.0], [0.0, 1080.0]]],
            color: [200, 210, 225],
            opacity: 1.0,
        });
        d.elements.push(line(0.0, 760.0, 1920.0, 700.0, 4.0));
        let r = 120.0 + 90.0 * i as f32;
        d.elements.push(circle(960.0 + 200.0 * i as f32 - 200.0, 480.0, r, 6.0));
        d.elements.push(line(760.0, 900.0, 1160.0, 880.0, 3.0));
        c.rerender();
        let shot = &mut doc.shots[i];
        shot.name = name.to_string();
        shot.action = action.to_string();
        shot.dialogue = dialogue.to_string();
        shot.set_length(24 * (i + 1));
        let n = shot.frame_count();
        shot.layers[0].frames = vec![Some(cel); n];
    }
    io::save_project(&doc, std::path::Path::new(&out)).unwrap();
}

#[test]
fn project_roundtrip() {
    let mut doc = Document::new(130, 70, 12);
    let a = doc.new_cel(LayerKind::Bitmap);
    let b = doc.new_cel(LayerKind::Bitmap);
    draw_line(&mut doc, a, (5.0, 5.0), (120.0, 60.0), BrushSettings::brush());
    draw_line(&mut doc, b, (5.0, 60.0), (120.0, 5.0), BrushSettings::pencil());
    let v = doc.new_cel(LayerKind::Vector);
    {
        let c = doc.cels.get_mut(&v).unwrap();
        c.vector.as_mut().unwrap().elements.push(line(10.0, 35.0, 120.0, 35.0, 4.0));
        c.rerender();
    }
    let mut bitmap = Layer::new("Couleurs é", LayerKind::Bitmap);
    bitmap.frames = vec![Some(a), Some(a), None, Some(b)];
    bitmap.opacity = 0.5;
    bitmap.visible = false;
    doc.shots[0].layers[0].frames = vec![Some(v)];
    doc.shots[0].layers.push(bitmap);
    doc.shots[0].action = "Il entre, <hésite>.".into();
    let mut second = doc.make_shot();
    second.name = "Contrechamp".into();
    second.dialogue = "— Toi ici ?".into();
    second.set_length(30);
    doc.shots.push(second);
    doc.paper = [250, 240, 230];

    let dir = std::env::temp_dir().join(format!("pelure-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("essai.pelure");
    io::save_project(&doc, &path).unwrap();
    let loaded = io::load_project(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!((loaded.width, loaded.height, loaded.fps, loaded.paper), (130, 70, 12, [250, 240, 230]));
    assert_eq!(loaded.shots.len(), 2);
    let s0 = &loaded.shots[0];
    assert_eq!(s0.action, "Il entre, <hésite>.");
    assert_eq!(s0.layers.len(), 2);
    assert_eq!(s0.layers[0].kind, LayerKind::Vector);
    assert_eq!(s0.layers[1].frames, vec![Some(a), Some(a), None, Some(b)]);
    assert_eq!(s0.layers[1].name, "Couleurs é");
    assert!(!s0.layers[1].visible);
    let s1 = &loaded.shots[1];
    assert_eq!((s1.name.as_str(), s1.dialogue.as_str(), s1.frame_count()), ("Contrechamp", "— Toi ici ?", 30));
    for id in [a, b] {
        assert_eq!(loaded.cels[&id].raster.to_flat(), doc.cels[&id].raster.to_flat(), "dessin {id}");
    }
    assert_eq!(loaded.cels[&v].vector, doc.cels[&v].vector);
    assert_eq!(loaded.cels[&v].raster.get(60, 35).a(), 255, "le vectoriel est re-rendu au chargement");
    // Un nouvel élément ne doit pas réutiliser un identifiant existant.
    let mut loaded = loaded;
    let fresh = loaded.new_cel(LayerKind::Bitmap);
    assert!(fresh > v && loaded.shots.iter().all(|s| s.id != fresh));
}
