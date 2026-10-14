use eframe::egui::{pos2, vec2, Color32};

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::document::{Document, Layer, LayerKind};
use crate::fill;
use crate::history::{Edit, History, Nav};
use crate::io;
use crate::project;
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
    Nav { panel: 0, layer: 0 }
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

// ------------------------------------------------------------------------------ séquences, scènes, panneaux

/// Projet type : séquence 1 = scène A (panneaux a1, a2) + scène B (b1) ; séquence 2 = scène C (c1).
fn sample() -> Document {
    let mut doc = Document::new(32, 32, 10);
    let a2 = doc.add_panel_after(0);
    let b1 = doc.add_scene_after(a2);
    doc.add_sequence_after(b1);
    doc
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn panel_length_holds_last_drawing() {
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_cel(LayerKind::Vector);
    let panel = &mut doc.panels[0];
    assert_eq!(panel.frame_count(), 48, "un panneau neuf dure deux secondes");
    panel.layers[0].frames = vec![Some(a); 48];
    panel.set_length(60);
    assert_eq!(panel.frame_count(), 60);
    assert_eq!(panel.layers[0].cel_at(59), Some(a), "le dessin tenu suit la nouvelle fin");
    panel.set_length(12);
    assert_eq!(panel.frame_count(), 12);
    assert_eq!(panel.layers[0].frames.len(), 12);
}

#[test]
fn montage_positions() {
    let mut doc = Document::new(32, 32, 10);
    doc.add_panel_after(0);
    doc.panels[0].set_length(5);
    doc.panels[1].set_length(8);
    assert_eq!(doc.total_frames(), 13);
    assert_eq!(doc.panel_start(1), 5);
    assert_eq!(doc.locate(0), (0, 0));
    assert_eq!(doc.locate(4), (0, 4));
    assert_eq!(doc.locate(5), (1, 0));
    assert_eq!(doc.locate(99), (1, 7));
}

#[test]
fn hierarchy_and_numbering() {
    let doc = sample();
    assert_eq!((doc.sequences.len(), doc.scenes.len(), doc.panels.len()), (2, 3, 4));
    let (a, b, c) = (doc.panels[0].scene, doc.panels[2].scene, doc.panels[3].scene);
    assert_eq!(doc.panels[1].scene, a);
    assert_eq!((doc.scene_number(a), doc.scene_number(b), doc.scene_number(c)), (1, 2, 3));
    assert_eq!(doc.panel_number(0), (1, 2));
    assert_eq!(doc.panel_number(1), (2, 2));
    assert_eq!(doc.panel_number(2), (1, 1));
    assert_eq!(doc.scene_run(1), (0, 2));
    assert_eq!(doc.sequence_run(2), (0, 3));
    assert_eq!(doc.sequence_run(3), (3, 4));
    assert_eq!(doc.sequence_number(doc.sequence_of(3)), 2);
    assert_eq!(doc.scene_label(b), "2");
    let mut doc = doc;
    doc.scene_mut(b).unwrap().name = "120".into();
    assert_eq!(doc.scene_label(b), "120");
    assert_eq!(doc.scene_number(c), 3, "un nom n'empêche pas la numérotation des suivantes");
}

#[test]
fn new_scene_and_sequence_go_after_the_current_one() {
    let mut doc = sample();
    // Depuis le 1er panneau de la scène A : la nouvelle scène vient après A, avant B.
    let i = doc.add_scene_after(0);
    assert_eq!(i, 2);
    assert_eq!(doc.scene_number(doc.panels[2].scene), 2);
    assert_eq!(doc.sequence_of(2), doc.sequence_of(0), "même séquence");
    // Nouvelle séquence depuis la séquence 1 : elle passe avant l'ancienne séquence 2.
    let j = doc.add_sequence_after(0);
    assert_eq!(j, 4);
    assert_eq!(doc.sequence_number(doc.sequence_of(4)), 2);
    assert_eq!(doc.sequence_number(doc.sequence_of(5)), 3);
}

#[test]
fn moving_a_panel_keeps_or_changes_its_scene() {
    let mut doc = sample();
    let (a, b) = (doc.panels[0].scene, doc.panels[2].scene);
    let a1 = doc.panels[0].id;
    // a1 passe après a2 : il reste dans la scène A.
    let i = doc.move_panel(0, 2).unwrap();
    assert_eq!((i, doc.panels[i].id, doc.panels[i].scene), (1, a1, a));
    assert_eq!(doc.move_panel(1, 2), None, "même place : rien ne bouge");
    // c1 posé devant b1 (hors de sa scène) : il rejoint B, juste avant b1 ; sa scène C et la
    // séquence 2, vides, disparaissent.
    let c1 = doc.panels[3].id;
    let i = doc.move_panel(3, 2).unwrap();
    assert_eq!((doc.panels[i].id, doc.panels[i].scene), (c1, b));
    assert_eq!(doc.panel_number(i), (1, 2));
    assert_eq!((doc.scenes.len(), doc.sequences.len()), (2, 1));
    // À la frontière entre deux scènes, un panneau venu d'ailleurs rejoint la scène qui suit.
    let a1_now = doc.panels.iter().position(|p| p.id == a1).unwrap();
    let i = doc.move_panel(a1_now, 4).unwrap();
    assert_eq!(doc.panels[i].scene, b, "posé tout à la fin : il rejoint la dernière scène");
}

#[test]
fn normalize_regroups_and_drops_empty_groups() {
    let mut doc = sample();
    // On mélange à la main : un panneau de A se retrouve après B.
    let a1 = doc.panels.remove(0);
    doc.panels.insert(2, a1);
    doc.normalize();
    let scenes: Vec<u64> = doc.panels.iter().map(|p| p.scene).collect();
    assert_eq!(scenes[0], scenes[1], "les panneaux de A sont regroupés");
    // Une scène qui n'a plus de panneau disparaît, sa séquence aussi.
    doc.panels.pop();
    doc.normalize();
    assert_eq!((doc.sequences.len(), doc.scenes.len()), (1, 2));
}

#[test]
fn removing_panels_and_scenes() {
    let mut doc = sample();
    assert!(doc.remove_scene(0).is_some());
    assert_eq!(doc.panels.len(), 2);
    assert!(doc.remove_panel(0).is_some());
    assert_eq!(doc.panels.len(), 1);
    assert_eq!(doc.remove_panel(0), None, "on garde toujours un panneau");
    assert_eq!(doc.remove_scene(0), None, "et une scène");
}

#[test]
fn duplicated_panel_and_scene_own_their_drawings() {
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames = vec![Some(a), Some(a)];
    let i = doc.duplicate_panel_after(0);
    let b = doc.panels[i].layers[0].frames[0].unwrap();
    assert_ne!(a, b);
    assert_eq!(doc.panels[i].layers[0].frames, vec![Some(b), Some(b)], "l'exposition est conservée");
    assert_eq!(doc.panels[i].scene, doc.panels[0].scene);
    doc.scene_mut(doc.panels[0].scene).unwrap().name = "Bus".into();
    let j = doc.duplicate_scene_after(0);
    assert_eq!(j, 2);
    assert_ne!(doc.panels[2].scene, doc.panels[0].scene);
    assert_eq!(doc.scene_label(doc.panels[2].scene), "Bus copie");
    assert_eq!(doc.scene_run(2), (2, 4), "les deux panneaux sont copiés");
    assert!(doc.panels[2].layers[0].frames[0] != Some(a));
}

#[test]
fn structure_undo_restores_scenes_and_panels() {
    let mut doc = Document::new(32, 32, 24);
    let snap = Edit::Structure {
        structure: doc.structure(),
        nav: nav(),
    };
    doc.add_scene_after(0);
    let mut history = History::default();
    history.push(snap);
    let mut n = Nav { panel: 1, layer: 0 };
    history.undo(&mut doc, &mut n);
    assert_eq!((doc.scenes.len(), doc.panels.len(), n.panel), (1, 1, 0));
    history.redo(&mut doc, &mut n);
    assert_eq!((doc.scenes.len(), doc.panels.len(), n.panel), (2, 2, 1));
}

#[test]
fn structure_undo_keeps_texts_and_layer_settings() {
    let mut doc = Document::new(32, 32, 24);
    let snap = Edit::Structure {
        structure: doc.structure(),
        nav: nav(),
    };
    // Après l'instantané : on écrit, on renomme, on règle le calque, et on rallonge le panneau.
    doc.panels[0].dialogue = "— Toi ici ?".into();
    doc.panels[0].layers[0].opacity = 0.3;
    doc.panels[0].layers[0].name = "Crayonné".into();
    let scene = doc.panels[0].scene;
    doc.scene_mut(scene).unwrap().name = "12A".into();
    doc.panels[0].set_length(100);
    let mut history = History::default();
    history.push(snap);
    let mut n = nav();
    history.undo(&mut doc, &mut n);
    assert_eq!(doc.panels[0].frame_count(), 48, "la durée est bien annulée");
    assert_eq!(doc.panels[0].dialogue, "— Toi ici ?", "le texte saisi est conservé");
    assert_eq!(doc.panels[0].layers[0].opacity, 0.3);
    assert_eq!(doc.panels[0].layers[0].name, "Crayonné");
    assert_eq!(doc.scene_label(scene), "12A");
    history.redo(&mut doc, &mut n);
    assert_eq!(doc.panels[0].frame_count(), 100);
}

// ------------------------------------------------------------------------------ fichiers

/// Projet de démonstration : `PELURE_DEMO=chemin\demo.pelure cargo test -- --ignored demo`.
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
    // Séquence 1 : « Arrivée » (2 panneaux), « Gros plan » ; séquence 2 : « Large ».
    let i1 = doc.add_panel_after(0);
    let i2 = doc.add_scene_after(i1);
    let i3 = doc.add_sequence_after(i2);
    doc.scene_mut(doc.panels[0].scene).unwrap().name = "Arrivée".into();
    doc.scene_mut(doc.panels[i2].scene).unwrap().name = "Gros plan".into();
    doc.scene_mut(doc.panels[i3].scene).unwrap().name = "Large".into();
    let texts = [
        ("Le bus s'arrête. Brume.", ""),
        ("Les portes s'ouvrent.", ""),
        ("Elle lève les yeux.", "— C'est ici ?"),
        ("La station, vide.", ""),
    ];
    for (i, (action, dialogue)) in texts.iter().enumerate() {
        let cel = doc.new_cel(LayerKind::Vector);
        let c = doc.cels.get_mut(&cel).unwrap();
        let d = c.vector.as_mut().unwrap();
        d.elements.push(Element::Fill {
            contours: vec![vec![[0.0, 760.0], [1920.0, 700.0], [1920.0, 1080.0], [0.0, 1080.0]]],
            color: [200, 210, 225],
            opacity: 1.0,
        });
        d.elements.push(line(0.0, 760.0, 1920.0, 700.0, 4.0));
        let r = 120.0 + 70.0 * i as f32;
        d.elements.push(circle(760.0 + 150.0 * i as f32, 480.0, r, 6.0));
        d.elements.push(line(760.0, 900.0, 1160.0, 880.0, 3.0));
        c.rerender();
        let panel = &mut doc.panels[i];
        panel.action = action.to_string();
        panel.dialogue = dialogue.to_string();
        panel.set_length(24 * (i % 3 + 1));
        let n = panel.frame_count();
        panel.layers[0].frames = vec![Some(cel); n];
    }
    // Sons de synthèse : une ambiance sur tout le montage (et un peu après), et une réplique
    // (des « syllabes ») au début du panneau « Gros plan ».
    let synth = |seconds: f32, f: &dyn Fn(f32) -> f32| {
        let n = (seconds * 48_000.0) as usize;
        let mut b = Vec::new();
        for (tag, value) in [(b"RIFF", 36 + 2 * n as u32), (b"WAVE", 0), (b"fmt ", 16)] {
            b.extend_from_slice(tag);
            if tag != b"WAVE" {
                b.extend_from_slice(&value.to_le_bytes());
            }
        }
        for v in [1u16, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&48_000u32.to_le_bytes());
        b.extend_from_slice(&96_000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(2 * n as u32).to_le_bytes());
        for i in 0..n {
            let s = (f(i as f32 / 48_000.0).clamp(-1.0, 1.0) * 32_000.0) as i16;
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    };
    let ambiance = synth(8.0, &|t| {
        let swell = 0.25 + 0.15 * (t * 0.8).sin();
        swell * ((t * 110.0 * std::f32::consts::TAU).sin() * 0.6 + (t * 173.0 * std::f32::consts::TAU).sin() * 0.4)
    });
    let voice = synth(1.6, &|t| {
        let syllable = (t * 5.0 * std::f32::consts::PI).sin().abs().powf(1.5);
        syllable * 0.8 * (t * 220.0 * std::f32::consts::TAU).sin()
    });
    for (name, bytes, at) in [("ambiance.wav", ambiance, 0), ("replique.wav", voice, doc.panel_start(i2) + 4)] {
        let pcm = crate::audio::decode(&bytes, "wav").unwrap();
        let sound = doc.add_sound(name.into(), "wav".into(), std::sync::Arc::new(bytes), std::sync::Arc::new(pcm));
        doc.place_sound(sound, at, None).unwrap();
    }
    doc.audio_tracks[0].name = "Ambiance".into();
    doc.audio_tracks[0].volume = 0.6;
    doc.audio_tracks[1].name = "Dialogue".into();
    let path = std::path::PathBuf::from(out);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    project::save(&doc, &path, &mut Default::default()).unwrap();
}

fn rich_doc() -> (Document, [u64; 3]) {
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
    doc.panels[0].layers[0].frames = vec![Some(v)];
    doc.panels[0].layers.push(bitmap);
    doc.panels[0].action = "Il entre, <hésite>.".into();
    let i = doc.add_scene_after(0);
    doc.scene_mut(doc.panels[i].scene).unwrap().name = "Contrechamp".into();
    doc.panels[i].dialogue = "— Toi ici ?".into();
    doc.panels[i].set_length(30);
    let s = doc.sequence_of(0);
    doc.sequence_mut(s).unwrap().name = "Ouverture".into();
    doc.paper = [250, 240, 230];
    (doc, [a, b, v])
}

fn check_rich(loaded: &Document, original: &Document, [a, b, v]: [u64; 3]) {
    assert_eq!((loaded.width, loaded.height, loaded.fps, loaded.paper), (130, 70, 12, [250, 240, 230]));
    assert_eq!(loaded.panels.len(), 2);
    let p0 = &loaded.panels[0];
    assert_eq!(p0.action, "Il entre, <hésite>.");
    assert_eq!(p0.layers.len(), 2);
    assert_eq!(p0.layers[0].kind, LayerKind::Vector);
    assert_eq!(p0.layers[1].frames, vec![Some(a), Some(a), None, Some(b)]);
    assert_eq!(p0.layers[1].name, "Couleurs é");
    assert!(!p0.layers[1].visible);
    let p1 = &loaded.panels[1];
    assert_eq!((loaded.scene_label(p1.scene).as_str(), p1.dialogue.as_str(), p1.frame_count()), ("Contrechamp", "— Toi ici ?", 30));
    for id in [a, b] {
        assert_eq!(loaded.cels[&id].raster.to_flat(), original.cels[&id].raster.to_flat(), "dessin {id}");
    }
    assert_eq!(loaded.cels[&v].vector, original.cels[&v].vector);
    assert_eq!(loaded.cels[&v].raster.get(60, 35).a(), 255, "le vectoriel est re-rendu au chargement");
}

#[test]
fn folder_project_roundtrip() {
    let (doc, ids) = rich_doc();
    let dir = temp_dir("dossier");
    let (proj_dir, manifest) = project::target_for(&dir.join("Essai.pelure"));
    assert_eq!(proj_dir, dir.join("Essai"));
    let mut written = std::collections::HashMap::new();
    let report = project::save(&doc, &manifest, &mut written).unwrap();
    assert_eq!(report.written, 3);
    assert!(proj_dir.join("dessins").join(format!("{}.json", ids[2])).exists());

    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    check_rich(&loaded.doc, &doc, ids);
    assert_eq!(loaded.doc.sequence_label(loaded.doc.sequence_of(0)), "Ouverture");
    let mut loaded_doc = loaded.doc;
    let fresh = loaded_doc.new_cel(LayerKind::Bitmap);
    assert!(fresh > ids[2] && loaded_doc.panels.iter().all(|p| p.id != fresh), "pas de réutilisation d'identifiant");

    // Réenregistrer sans rien changer n'écrit aucun dessin.
    let mut written = loaded.written;
    assert_eq!(project::save(&loaded_doc, &manifest, &mut written).unwrap().written, 0);
    // Un dessin modifié est réécrit, seul.
    loaded_doc.cels.get_mut(&ids[0]).unwrap().version += 1;
    assert_eq!(project::save(&loaded_doc, &manifest, &mut written).unwrap().written, 1);
    // Un dessin qui ne sert plus est effacé du dossier.
    loaded_doc.panels[0].layers[1].frames = vec![Some(ids[0])];
    let report = project::save(&loaded_doc, &manifest, &mut written).unwrap();
    assert_eq!(report.removed, 1);
    assert!(!proj_dir.join("dessins").join(format!("{}.png", ids[1])).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_drawing_is_reported_not_fatal() {
    let (doc, ids) = rich_doc();
    let dir = temp_dir("manquant");
    let (_, manifest) = project::target_for(&dir.join("P.pelure"));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    std::fs::remove_file(manifest.parent().unwrap().join("dessins").join(format!("{}.png", ids[1]))).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.doc.panels[0].layers[1].frames, vec![Some(ids[0]), Some(ids[0]), None, None]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_target_rules() {
    let base = std::path::Path::new("C:\\Films");
    let (d, m) = project::target_for(&base.join("Bus.pelure"));
    assert_eq!((d.clone(), m.clone()), (base.join("Bus"), base.join("Bus").join("Bus.pelure")));
    // Choisir le manifeste déjà dans son dossier ne crée pas « Bus\Bus\ ».
    assert_eq!(project::target_for(&m), (d, m));

    let dir = temp_dir("cible");
    let (_, manifest) = project::target_for(&dir.join("Film.pelure"));
    let (doc, _) = rich_doc();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(project::check_target(&manifest, None).is_err(), "pas d'écrasement d'un autre projet");
    assert!(project::check_target(&manifest, Some(&manifest)).is_ok(), "réenregistrer au même endroit");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn legacy_file_is_imported() {
    let (doc, ids) = rich_doc();
    let dir = temp_dir("ancien");
    let path = dir.join("ancien.pelure");
    io::save_legacy_v2(&doc, &path).unwrap();
    assert!(io::is_legacy_file(&path));
    let loaded = io::load_legacy(&path).unwrap();
    check_rich(&loaded, &doc, ids);
    // Chaque ancien « plan » devient une scène d'un panneau, dans une seule séquence.
    assert_eq!((loaded.sequences.len(), loaded.scenes.len()), (1, 2));
    std::fs::remove_dir_all(&dir).ok();
}
