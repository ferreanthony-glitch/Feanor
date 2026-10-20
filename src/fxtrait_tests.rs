//! Effets du trait (`fxtrait.rs`) : écriture du trait (0, 50, 100 %), effacement, clés, déterminisme
//! (même image, mêmes pixels, sur un fil ou sur plusieurs), enregistrement, calque vectoriel
//! seulement, dessin dérivé en arrière-plan et export.

use eframe::egui::Color32;

use crate::document::{Document, LayerKind};
use crate::fx::{self, Effect, EffectKind, FxStep, Shown};
use crate::fxexact_tests::differing;
use crate::fxtrait::{self as t, Stack};
use crate::raster::Raster;
use crate::render::{self, PlanItem};
use crate::vector::{Element, Trace, VPoint, VectorDrawing};

const W: usize = 280;
const H: usize = 220;

/// Un trait lisse, noir, le long de `pts` (x, y, rayon), un point tous les 5 pixels environ.
pub(crate) fn smooth_stroke(pts: &[(f32, f32, f32)]) -> Element {
    let mut points: Vec<VPoint> = Vec::new();
    for w in pts.windows(2) {
        let n = ((w[1].0 - w[0].0).hypot(w[1].1 - w[0].1) / 5.0).ceil().max(1.0) as usize;
        for k in 0..n {
            let u = k as f32 / n as f32;
            points.push(VPoint { x: w[0].0 + (w[1].0 - w[0].0) * u, y: w[0].1 + (w[1].1 - w[0].1) * u, r: w[0].2 + (w[1].2 - w[0].2) * u });
        }
    }
    if let Some(&(x, y, r)) = pts.last() {
        points.push(VPoint { x, y, r });
    }
    Element::Stroke { points, color: [20, 20, 20], opacity: 1.0, hardness: 0.9, brush: None, trace: Trace::default() }
}

/// Six traits horizontaux de 200 pixels de long, un tous les 30 pixels.
pub(crate) fn six_lines() -> VectorDrawing {
    VectorDrawing { elements: (0..6).map(|i| smooth_stroke(&[(20.0, 25.0 + 30.0 * i as f32, 2.5), (220.0, 25.0 + 30.0 * i as f32, 2.5)])).collect() }
}

/// Les valeurs d'un effet : celles par défaut, puis `set`.
pub(crate) fn effect(kind: EffectKind, set: &[(usize, f32)]) -> Effect {
    let mut e = Effect::new(kind);
    for &(k, v) in set {
        e.set_param(k, v);
    }
    e
}

pub(crate) fn steps(kind: EffectKind, set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    fx::steps_at(&[effect(kind, set)], frame)
}

fn write_steps(progress: f32, set: &[(usize, f32)]) -> Vec<FxStep> {
    let mut all = vec![(t::WRITE_PROGRESS, progress)];
    all.extend_from_slice(set);
    steps(EffectKind::Write, &all, 0)
}

pub(crate) fn render_with(d: &VectorDrawing, s: &[FxStep]) -> Raster {
    t::render(d, W, H, s)
}

/// Le nombre de pixels encrés de la rangée de pixels `y`, entre les colonnes `x0` et `x1`.
fn ink(r: &Raster, y: usize, x0: usize, x1: usize) -> usize {
    (x0..x1).filter(|&x| r.get(x, y).a() > 40).count()
}

fn empty(r: &Raster) -> bool {
    r.tiles.iter().flatten().all(|t| t.iter().all(|c| c.a() == 0))
}

// ------------------------------------------------------------------ écriture du trait

#[test]
fn the_write_on_is_empty_at_0_percent_partial_at_50_and_the_whole_drawing_at_100() {
    let d = six_lines();
    let plain = d.render(W, H);
    // 100 % : le dessin tel quel, pixel pour pixel (les effets sans effet ne coûtent rien).
    assert!(steps(EffectKind::Write, &[(t::WRITE_PROGRESS, 100.0)], 0).is_empty());
    assert_eq!(differing(&render_with(&d, &write_steps(99.99, &[])), &plain), 0, "presque 100 % : le même dessin");
    // 0 % : rien.
    assert!(empty(&render_with(&d, &write_steps(0.0, &[]))), "à 0 % rien n'est tracé");
    // 50 % : les trois premiers traits (même longueur) sont tracés, les trois derniers ne le sont pas.
    let half = render_with(&d, &write_steps(50.0, &[]));
    for i in 0..6 {
        let y = 25 + 30 * i;
        let drawn = ink(&half, y, 30, 200);
        if i < 3 {
            assert!(drawn > 150, "le trait {i} est tracé : {drawn}");
            for x in 30..200 {
                assert_eq!(half.get(x, y), plain.get(x, y), "le trait {i} est tel quel en {x}");
            }
        } else {
            assert_eq!(drawn, 0, "le trait {i} n'est pas commencé");
        }
    }
    // Entre les deux : à 25 %, le trait du milieu de la première moitié est entamé à gauche seulement.
    let quarter = render_with(&d, &write_steps(25.0, &[]));
    let (left, right) = (ink(&quarter, 55, 20, 120), ink(&quarter, 55, 120, 221));
    assert!(left > 60 && right < 8, "le deuxième trait avance de gauche à droite : {left} / {right}");
    assert_eq!(ink(&quarter, 25, 30, 200) > 150, true, "le premier trait est fini");
    assert_eq!(ink(&quarter, 85, 20, 221), 0, "le troisième n'est pas commencé");
}

#[test]
fn the_progress_grows_monotonically_the_ink_never_comes_back() {
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(20.0, 40.0, 3.0), (140.0, 90.0, 3.0), (250.0, 30.0, 2.0)]), smooth_stroke(&[(30.0, 150.0, 4.0), (240.0, 170.0, 4.0)])] };
    let mut last = 0usize;
    for p in (0..=100).step_by(5) {
        let r = render_with(&d, &write_steps(p as f32, &[]));
        let total: usize = (0..H).map(|y| ink(&r, y, 0, W)).sum();
        assert!(total >= last, "{p} % : {total} < {last}");
        last = total;
    }
    let plain: usize = (0..H).map(|y| ink(&d.render(W, H), y, 0, W)).sum();
    assert_eq!(last, plain, "100 % : tout l'encre");
}

#[test]
fn all_strokes_together_each_one_advances_by_the_same_share() {
    let d = six_lines();
    let r = render_with(&d, &write_steps(50.0, &[(t::WRITE_TOGETHER, 1.0)]));
    for i in 0..6 {
        let y = 25 + 30 * i;
        assert!(ink(&r, y, 20, 115) > 85, "le trait {i} est tracé jusqu'à la moitié");
        assert_eq!(ink(&r, y, 126, 221), 0, "le trait {i} s'arrête à la moitié");
    }
}

#[test]
fn equal_durations_give_every_stroke_the_same_slot_whatever_its_length() {
    // Un trait court (50 px), un long (240 px) : à 40 %, en durées égales le court est aux quatre
    // cinquièmes et le long pas commencé ; selon la longueur, le court est fini et le long avancé.
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(20.0, 40.0, 2.5), (70.0, 40.0, 2.5)]), smooth_stroke(&[(20.0, 100.0, 2.5), (260.0, 100.0, 2.5)])] };
    let equal = render_with(&d, &write_steps(40.0, &[(t::WRITE_BY_LENGTH, 0.0)]));
    let by_length = render_with(&d, &write_steps(40.0, &[(t::WRITE_BY_LENGTH, 1.0)]));
    assert!((30..45).contains(&ink(&equal, 40, 20, 71)), "le court est aux quatre cinquièmes : {}", ink(&equal, 40, 20, 71));
    assert_eq!(ink(&equal, 100, 20, 261), 0, "le long n'est pas commencé");
    assert!(ink(&by_length, 40, 20, 71) > 45, "le court est fini");
    assert!(ink(&by_length, 100, 20, 261) > 40, "le long est avancé : {}", ink(&by_length, 100, 20, 261));
}

#[test]
fn the_erase_direction_removes_strokes_in_drawing_order_and_complements_the_write() {
    let d = six_lines();
    let erased = render_with(&d, &write_steps(50.0, &[(t::WRITE_ERASE, 1.0)]));
    for i in 0..6 {
        let y = 25 + 30 * i;
        if i < 3 {
            assert_eq!(ink(&erased, y, 30, 200), 0, "le trait {i} est effacé");
        } else {
            assert!(ink(&erased, y, 30, 200) > 150, "le trait {i} est encore là");
        }
    }
    assert!(empty(&render_with(&d, &write_steps(100.0, &[(t::WRITE_ERASE, 1.0)]))), "100 % : tout est effacé");
    assert!(steps(EffectKind::Write, &[(t::WRITE_PROGRESS, 0.0), (t::WRITE_ERASE, 1.0)], 0).is_empty(), "0 % d'effacement : rien à faire");
    // À l'avancement p, le trait écrit et le trait effacé se complètent (aux arrondis près).
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(20.0, 40.0, 3.0), (140.0, 90.0, 3.0), (250.0, 30.0, 2.0)])] };
    let (w, e, all) = (render_with(&d, &write_steps(37.0, &[(t::WRITE_SOFT, 12.0)])), render_with(&d, &write_steps(37.0, &[(t::WRITE_SOFT, 12.0), (t::WRITE_ERASE, 1.0)])), d.render(W, H));
    for y in 0..H {
        for x in 0..W {
            let sum = w.get(x, y).a() as i32 + e.get(x, y).a() as i32;
            assert!((sum - all.get(x, y).a() as i32).abs() <= 2, "({x}, {y}) : {sum} contre {}", all.get(x, y).a());
        }
    }
}

#[test]
fn the_soft_edge_fades_the_front_and_the_ease_slows_the_ends() {
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(20.0, 60.0, 3.0), (260.0, 60.0, 3.0)])] };
    let hard = render_with(&d, &write_steps(50.0, &[]));
    let soft = render_with(&d, &write_steps(50.0, &[(t::WRITE_SOFT, 40.0)]));
    let alpha = |r: &Raster, x: usize| r.get(x, 60).a();
    // Avec le bord doux, la pointe s'efface sur ~40 px au lieu d'une coupure nette.
    let partial = |r: &Raster| (20..260).filter(|&x| alpha(r, x) > 20 && alpha(r, x) < 235).count();
    assert!(partial(&soft) > 20 && partial(&hard) <= 3, "{} contre {}", partial(&soft), partial(&hard));
    // Rythme : à 25 % d'avancement, une écriture adoucie est moins avancée qu'une écriture linéaire.
    let linear = ink(&render_with(&d, &write_steps(25.0, &[])), 60, 0, W);
    let eased = ink(&render_with(&d, &write_steps(25.0, &[(t::WRITE_EASE, 100.0)])), 60, 0, W);
    assert!(eased < linear, "{eased} < {linear}");
    let (a, b) = (t::WriteS::of(&[50.0, 0.0, 0.0, 1.0, 100.0]), t::WriteS::of(&[50.0, 0.0, 0.0, 1.0, 0.0]));
    assert!((a.eased() - 0.5).abs() < 1e-6 && (b.eased() - 0.5).abs() < 1e-6, "le milieu reste le milieu");
}

#[test]
fn filled_zones_follow_the_write_on_when_asked() {
    let mut d = six_lines();
    d.elements.insert(0, Element::Fill { contours: vec![vec![[40.0, 10.0], [200.0, 10.0], [200.0, 200.0], [40.0, 200.0]]], color: [200, 30, 30], opacity: 1.0 });
    let stay = render_with(&d, &write_steps(30.0, &[]));
    assert!(stay.get(60, 195).a() == 255, "les zones remplies restent");
    let follow = render_with(&d, &write_steps(30.0, &[(t::WRITE_FILLS, 1.0)]));
    assert_eq!(follow.get(60, 195).a(), 0, "les zones remplies attendent la fin");
    let late = render_with(&d, &write_steps(90.0, &[(t::WRITE_FILLS, 1.0)]));
    assert!(late.get(60, 195).a() > 0 && late.get(60, 195).a() < 255, "elles apparaissent en fondu à la fin");
    assert_eq!(late.get(60, 195), render_with(&d, &write_steps(90.0, &[(t::WRITE_FILLS, 1.0)])).get(60, 195));
}

#[test]
fn the_keys_animate_the_progress_in_a_straight_line() {
    let mut e = Effect::new(EffectKind::Write);
    e.set_key(0, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 0.0)]).values);
    e.set_key(10, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 100.0)]).values);
    assert_eq!(e.values_at(5)[t::WRITE_PROGRESS], 50.0);
    assert!(fx::steps_at(std::slice::from_ref(&e), 10).is_empty(), "100 % : plus rien à faire");
    let s = fx::steps_at(std::slice::from_ref(&e), 4);
    assert!((s[0].values[t::WRITE_PROGRESS] - 40.0).abs() < 1e-4);
    // Un réglage qui ne s'anime pas se pose dans toutes les clés.
    e.set_param(t::WRITE_TOGETHER, 1.0);
    assert!(e.keys.iter().all(|k| k.values[t::WRITE_TOGETHER] == 1.0));
    // Chaque avancement a son dessin dérivé.
    let ids: Vec<u64> = (0..10).map(|f| fx::fx_id(9, &fx::steps_at(std::slice::from_ref(&e), f))).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 10);
}

// ------------------------------------------------------------------ déterminisme

#[test]
fn the_same_frame_always_gives_the_same_pixels_on_one_thread_or_many() {
    let mut elements = six_lines().elements;
    elements.extend((0..40).map(|i| smooth_stroke(&[(10.0 + i as f32 * 5.0, 10.0, 2.0), (30.0 + i as f32 * 5.0, 200.0, 2.0)])));
    let d = VectorDrawing { elements };
    for s in [write_steps(37.0, &[]), write_steps(63.0, &[(t::WRITE_TOGETHER, 1.0), (t::WRITE_SOFT, 9.0)])] {
        let a = render_with(&d, &s);
        let b = render_with(&d, &s);
        assert_eq!(differing(&a, &b), 0, "deux calculs, les mêmes pixels");
        crate::parallele::EN_SERIE.with(|x| x.set(true));
        let serial = render_with(&d, &s);
        crate::parallele::EN_SERIE.with(|x| x.set(false));
        assert_eq!(differing(&a, &serial), 0, "un fil ou plusieurs, les mêmes pixels");
    }
}

// ------------------------------------------------------------------ enregistrement et calques

#[test]
fn the_settings_are_saved_with_the_layer_and_old_projects_open_unchanged() {
    assert_eq!(crate::project::VERSION, 31);
    let mut doc = Document::new(64, 64, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = LayerKind::Vector;
    layer.frames = vec![Some(id)];
    let mut e = Effect::new(EffectKind::Write);
    e.set_key(0, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 0.0), (t::WRITE_TOGETHER, 1.0)]).values);
    e.set_key(12, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 100.0), (t::WRITE_TOGETHER, 1.0)]).values);
    layer.fx = vec![e, Effect::new(EffectKind::GaussianBlur)];
    let dir = std::env::temp_dir().join(format!("shot-test-fxtrait-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"ecriture_trait\""), "{text}");
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].fx, doc.panels[0].layers[0].fx, "mêmes réglages et mêmes clés");
    // Un projet d'avant (version 27, sans effet du trait) s'ouvre tel quel.
    let old = text.replace("\"version\": 31", "\"version\": 27").replace("\"ecriture_trait\"", "\"flou_gaussien\"");
    std::fs::write(&manifest, old).unwrap();
    assert!(crate::project::load(&manifest).is_ok());
    // Une version d'après est refusée, comme toujours.
    std::fs::write(&manifest, text.replace("\"version\": 31", "\"version\": 32")).unwrap();
    assert!(crate::project::load(&manifest).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_stroke_effects_only_apply_to_vector_layers() {
    let kinds = [EffectKind::Write, EffectKind::Sketch, EffectKind::Breath, EffectKind::StrokeWave, EffectKind::Dashes, EffectKind::Dust];
    let mut vector = crate::document::Layer::new("V", LayerKind::Vector);
    let mut bitmap = crate::document::Layer::new("B", LayerKind::Bitmap);
    for k in kinds {
        assert!(k.is_trait() && k.vector_only() && k.family() == "Trait");
        vector.fx.push(Effect::new(k));
        bitmap.fx.push(Effect::new(k));
    }
    bitmap.fx.push(Effect::new(EffectKind::GaussianBlur));
    assert_eq!(fx::layer_steps(&vector, 0, None).len(), 6);
    assert_eq!(fx::layer_steps(&bitmap, 0, None).iter().map(|s| s.kind).collect::<Vec<_>>(), vec![EffectKind::GaussianBlur]);
    for k in kinds {
        assert_eq!(crate::fxui::offer(k, LayerKind::Vector, false), crate::fxui::Offer::Enabled);
        assert!(matches!(crate::fxui::offer(k, LayerKind::Bitmap, false), crate::fxui::Offer::Disabled(why) if why.contains("vectoriel")), "{k:?}");
        assert_eq!(crate::fxui::offer(k, LayerKind::Vector, true), crate::fxui::Offer::Absent);
    }
    // Le calque qu'on édite se montre tel qu'il est.
    assert!(fx::without_boil(fx::layer_steps(&vector, 0, None)).is_empty());
    // Sans le dessin vectoriel (calque d'effet, aperçu réduit), les effets du trait sont laissés.
    let raster = six_lines().render(W, H);
    assert_eq!(differing(&fx::apply(&raster, &write_steps(10.0, &[])), &raster), 0);
}

#[test]
fn every_stroke_effect_has_sound_defaults_and_bad_settings_never_panic() {
    for k in EffectKind::ALL.into_iter().filter(|k| k.is_trait()) {
        assert!(!k.params().is_empty());
        assert_eq!(Effect::new(k).values_at(0).len(), k.params().len());
        // Des valeurs absentes, absurdes ou hors bornes sont ramenées, jamais ne plantent.
        for values in [vec![], vec![f32::NAN; 8], vec![f32::INFINITY, -1e9, 1e9, 3.0, -7.0, 0.0, 99.0, 5.0, 5.0], vec![0.0; 9]] {
            let s = FxStep { kind: k, values };
            let _ = (Stack::of(std::slice::from_ref(&s)), k.changes(&s.values));
            let _ = render_with(&six_lines(), std::slice::from_ref(&s));
        }
    }
}

// ------------------------------------------------------------------ dessin dérivé et export

/// Un document de `frames` images dont le calque vectoriel porte les traits de `d` et la pile `fx`.
pub(crate) fn drawing_doc(d: VectorDrawing, fx_stack: Vec<Effect>, frames: usize) -> Document {
    drawing_doc_sized(d, fx_stack, frames, W, H)
}

pub(crate) fn drawing_doc_sized(d: VectorDrawing, fx_stack: Vec<Effect>, frames: usize, w: usize, h: usize) -> Document {
    let mut doc = Document::new(w, h, 24);
    let id = doc.new_cel(LayerKind::Vector);
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        cel.vector = Some(d);
        cel.rerender();
    }
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = LayerKind::Vector;
    layer.frames = vec![Some(id); frames];
    layer.fx = fx_stack;
    doc
}

#[test]
fn the_export_the_thumbnails_and_the_screen_show_the_same_written_frame() {
    crate::fxcache::monde::ecrire();
    let mut e = Effect::new(EffectKind::Write);
    e.set_key(0, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 0.0)]).values);
    e.set_key(10, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 100.0)]).values);
    let doc = drawing_doc(six_lines(), vec![e], 11);
    let image = |frame: usize| render::composite_full(&doc, &render::panel_plan_at(&doc, 0, frame, None));
    let plain = doc.cels[&doc.panels[0].layers[0].frames[0].unwrap()].raster.to_flat();
    assert!(image(0).iter().all(|c| c.a() == 0), "image 0 : rien");
    assert_eq!(image(10), plain, "image 10 : le dessin entier");
    let (f3, f6) = (image(3), image(6));
    let count = |v: &[Color32]| v.iter().filter(|c| c.a() > 40).count();
    assert!(count(&f3) > 0 && count(&f3) < count(&f6) && count(&f6) < count(&plain), "{} {} {}", count(&f3), count(&f6), count(&plain));
    // Le dessin n'a pas bougé.
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    assert_eq!(doc.cels[&id].raster.to_flat(), plain);
    // Une vignette (calcul doux) montre la même image une fois le calcul fini.
    let plan = render::panel_plan_at(&doc, 0, 6, None);
    let want = render::composite_pixel(&doc, &plan, 100, 25 + 30 * 2);
    crate::fxcache::settle();
    let (got, _) = crate::fxcache::soft_scope(|| render::composite_pixel(&doc, &plan, 100, 25 + 30 * 2));
    assert_eq!(got, want);
}

#[test]
fn the_derived_drawing_is_computed_in_the_background_and_equals_the_direct_computation() {
    crate::fxcache::monde::ecrire();
    let doc = drawing_doc(six_lines(), vec![effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 45.0)])], 3);
    let plan = render::panel_plan_at(&doc, 0, 1, None);
    let item: &PlanItem = &plan.items[0];
    assert!(item.fx.iter().all(|s| s.kind == EffectKind::Write));
    let first = fx::display(&doc, item, 0).unwrap();
    assert!(!first.ready, "calculé en arrière-plan, jamais sur le fil de l'interface");
    crate::fxcache::settle();
    let d = fx::display(&doc, item, 0).unwrap();
    assert!(d.ready && matches!(d.cel, Shown::Fx(_)));
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let direct = fx::apply_with(&doc.cels[&id].raster, doc.cels[&id].vector.as_ref(), &item.fx, 0);
    assert_eq!(differing(&d.cel.raster, &direct), 0);
    // Un réglage de plus (sans le dessin vectoriel) : le même calcul n'en change pas la lecture.
    assert!(!fx::display(&doc, &PlanItem { fx: write_steps(46.0, &[]), ..item.clone() }, 0).unwrap().ready);
    crate::fxcache::settle();
}

// ------------------------------------------------------------------ crayonné multiple

/// Une ligne horizontale épaisse (rayon 3) à `y`.
fn one_line(y: f32) -> VectorDrawing {
    VectorDrawing { elements: vec![smooth_stroke(&[(30.0, y, 3.0), (250.0, y, 3.0)])] }
}

fn rows_with_ink(r: &Raster, x: usize) -> Vec<usize> {
    (0..H).filter(|&y| r.get(x, y).a() > 24).collect()
}

fn sketch_steps(set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    steps(EffectKind::Sketch, set, frame)
}

#[test]
fn the_sketch_doubles_each_stroke_with_offset_passes_under_the_intact_main_line() {
    let d = one_line(100.0);
    let plain = d.render(W, H);
    let s = sketch_steps(&[(t::SKETCH_OFFSET, 5.0), (t::SKETCH_PASSES, 2.0), (t::SKETCH_OPACITY, 70.0)], 0);
    let sketched = render_with(&d, &s);
    // Le trait principal est intact par-dessus : où il est opaque, il l'est encore, de la même couleur.
    for y in 0..H {
        for x in 0..W {
            if plain.get(x, y).a() == 255 {
                assert_eq!(sketched.get(x, y), plain.get(x, y), "({x}, {y})");
            }
        }
    }
    // Les passes ajoutent de l'encre autour, sans aller plus loin que le décalage et le rayon.
    let count = |r: &Raster| (0..H).map(|y| (0..W).filter(|&x| r.get(x, y).a() > 24).count()).sum::<usize>();
    assert!(count(&sketched) > count(&plain) * 12 / 10, "{} contre {}", count(&sketched), count(&plain));
    let mut wider = 0;
    for x in (60..240).step_by(10) {
        let (a, b) = (rows_with_ink(&plain, x), rows_with_ink(&sketched, x));
        let (lo, hi) = (*b.first().unwrap(), *b.last().unwrap());
        if b.len() >= a.len() + 4 {
            wider += 1;
        }
        // 3 px de rayon + 5 de décalage + 2 de lissage, de part et d'autre de l'axe.
        assert!(lo >= 90 && hi <= 110, "colonne {x} : {lo}..{hi}");
    }
    assert!(wider >= 14, "le trait est redoublé presque partout : {wider} colonnes sur 18");
    // Les passes se voient moins que le trait : leur opacité est celle du réglage.
    let faint = render_with(&d, &sketch_steps(&[(t::SKETCH_OFFSET, 5.0), (t::SKETCH_OPACITY, 30.0)], 0));
    let strong = render_with(&d, &sketch_steps(&[(t::SKETCH_OFFSET, 5.0), (t::SKETCH_OPACITY, 100.0)], 0));
    let mass = |r: &Raster| (0..H).map(|y| (0..W).map(|x| r.get(x, y).a() as u32).sum::<u32>()).sum::<u32>();
    assert!(mass(&plain) < mass(&faint) && mass(&faint) < mass(&strong));
    // Sans décalage ou sans opacité, rien à faire.
    assert!(sketch_steps(&[(t::SKETCH_OFFSET, 0.0)], 0).is_empty() && sketch_steps(&[(t::SKETCH_OPACITY, 0.0)], 0).is_empty());
}

#[test]
fn the_sketch_is_redrawn_every_n_frames_and_each_variant_is_always_the_same() {
    let d = one_line(100.0);
    let at = |frame: usize, speed: f32| render_with(&d, &sketch_steps(&[(t::SKETCH_SPEED, speed), (t::SKETCH_OFFSET, 6.0)], frame));
    // Vitesse 3 : trois images pour la même variante, puis la suivante.
    let (f0, f1, f2, f3) = (at(0, 3.0), at(1, 3.0), at(2, 3.0), at(3, 3.0));
    assert_eq!(differing(&f0, &f1), 0);
    assert_eq!(differing(&f0, &f2), 0);
    assert!(differing(&f0, &f3) > 100, "la variante suivante est un autre crayonné");
    // Trois variantes : on revient au bout de neuf images.
    assert_eq!(differing(&f0, &at(9, 3.0)), 0);
    // Deux calculs : les mêmes pixels ; une autre graine : un autre crayonné.
    assert_eq!(differing(&at(4, 3.0), &at(4, 3.0)), 0);
    let seeded = render_with(&d, &sketch_steps(&[(t::SKETCH_SPEED, 3.0), (t::SKETCH_OFFSET, 6.0), (t::SKETCH_SEED, 2.0)], 0));
    assert!(differing(&f0, &seeded) > 100);
    // Le dessin dérivé de chaque variante a son numéro.
    let id = |frame: usize| fx::fx_id(5, &sketch_steps(&[(t::SKETCH_SPEED, 3.0)], frame));
    assert_eq!(id(0), id(2));
    assert_ne!(id(2), id(3));
    assert_eq!(id(0), id(9));
}

// ------------------------------------------------------------------ respiration

fn breath_steps(set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    steps(EffectKind::Breath, set, frame)
}

#[test]
fn the_breath_scales_the_stroke_width_and_pulses_with_the_period() {
    let d = one_line(100.0);
    let plain = d.render(W, H);
    let thick = |r: &Raster| rows_with_ink(r, 140).len();
    // L'épaisseur de base : 200 % double le trait, 50 % le réduit de moitié.
    let wide = render_with(&d, &breath_steps(&[(t::BREATH_THICKNESS, 200.0), (t::BREATH_AMPLITUDE, 0.0)], 0));
    let thin = render_with(&d, &breath_steps(&[(t::BREATH_THICKNESS, 50.0), (t::BREATH_AMPLITUDE, 0.0)], 0));
    let (p, w2, t2) = (thick(&plain), thick(&wide), thick(&thin));
    assert!(w2 >= p * 18 / 10 && w2 <= p * 22 / 10 + 1, "{p} -> {w2}");
    assert!(t2 * 2 <= p + 2 && t2 * 2 + 2 >= p, "{p} -> {t2}");
    // Pulsation : le quart de la période épaissit, les trois quarts amincissent, la période revient.
    let at = |frame: usize| render_with(&d, &breath_steps(&[(t::BREATH_AMPLITUDE, 60.0), (t::BREATH_PERIOD, 24.0)], frame));
    let (f0, f6, f18, f24) = (at(0), at(6), at(18), at(24));
    assert_eq!(differing(&f0, &plain), 0, "à l'image 0, sin(0) : le trait tel quel");
    assert!(thick(&f6) > thick(&f0) && thick(&f18) < thick(&f0), "{} {} {}", thick(&f6), thick(&f0), thick(&f18));
    assert_eq!(differing(&f0, &f24), 0, "la période revient");
    // Une vague le long du trait : l'épaisseur n'est pas la même partout au même instant.
    let along = render_with(&d, &breath_steps(&[(t::BREATH_AMPLITUDE, 70.0), (t::BREATH_WAVE, 100.0)], 6));
    let widths: Vec<usize> = (40..240).step_by(10).map(|x| rows_with_ink(&along, x).len()).collect();
    assert!(widths.iter().max().unwrap() >= &(widths.iter().min().unwrap() + 3), "{widths:?}");
    // Rien à faire : épaisseur 100 % sans amplitude.
    assert!(breath_steps(&[(t::BREATH_AMPLITUDE, 0.0)], 0).is_empty());
}

// ------------------------------------------------------------------ ondulation du trait

fn wave_steps(set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    steps(EffectKind::StrokeWave, set, frame)
}

/// L'ordonnée moyenne (centre de l'encre) de la colonne `x`.
fn centre(r: &Raster, x: usize) -> f32 {
    let (mut s, mut n) = (0.0f32, 0.0f32);
    for y in 0..H {
        let a = r.get(x, y).a() as f32;
        s += a * (y as f32 + 0.5);
        n += a;
    }
    if n > 0.0 { s / n } else { f32::NAN }
}

#[test]
fn the_wave_travels_along_the_stroke_with_the_given_amplitude_and_speed() {
    let d = one_line(100.0);
    let set = [(t::WAVE_AMPLITUDE, 8.0), (t::WAVE_LENGTH, 80.0), (t::WAVE_SPEED, 8.0)];
    let at = |frame: usize| render_with(&d, &wave_steps(&set, frame));
    let f0 = at(0);
    let swing: Vec<f32> = (60..230).map(|x| centre(&f0, x) - 100.5).collect();
    let (hi, lo) = (swing.iter().cloned().fold(f32::MIN, f32::max), swing.iter().cloned().fold(f32::MAX, f32::min));
    assert!(hi > 6.0 && lo < -6.0 && hi < 9.0 && lo > -9.0, "l'amplitude de 8 px : {lo} .. {hi}");
    // Les bouts sont fixes : la courbe y revient.
    assert!((centre(&f0, 32) - 100.5).abs() < 1.5 && (centre(&f0, 247) - 100.5).abs() < 1.5);
    // La vague avance : 8 px par image, 10 images = une longueur d'onde (80 px) : le même dessin.
    assert!(differing(&f0, &at(1)) > 100);
    assert_eq!(differing(&f0, &at(10)), 0, "après une longueur d'onde");
    // À l'image 5 (40 px = une demi-longueur), la vague est inversée.
    let half = at(5);
    for x in (80..220).step_by(7) {
        assert!((centre(&half, x) - 100.5 + (centre(&f0, x) - 100.5)).abs() < 1.2, "colonne {x}");
    }
    // Sans bouts fixes, l'extrémité bouge aussi.
    let free = render_with(&d, &wave_steps(&[(t::WAVE_AMPLITUDE, 8.0), (t::WAVE_LENGTH, 80.0), (t::WAVE_FIXED, 0.0), (t::WAVE_SPEED, 8.0)], 2));
    assert!((centre(&free, 33) - 100.5).abs() > 2.0);
    assert!(wave_steps(&[(t::WAVE_AMPLITUDE, 0.0)], 0).is_empty());
}

// ------------------------------------------------------------------ pointillés

fn dash_steps(set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    steps(EffectKind::Dashes, set, frame)
}

fn ink_columns(r: &Raster, y: usize) -> Vec<bool> {
    (0..W).map(|x| r.get(x, y).a() > 128).collect()
}

#[test]
fn the_dashes_cut_the_stroke_into_runs_and_march_along_it() {
    let d = one_line(100.0);
    let set = [(t::DASH_LENGTH, 18.0), (t::DASH_GAP, 12.0), (t::DASH_SPEED, 3.0)];
    let f0 = render_with(&d, &dash_steps(&set, 0));
    let cols = ink_columns(&f0, 100);
    let runs = cols.windows(2).filter(|w| !w[0] && w[1]).count();
    // 220 pixels de trait, un tiret tous les 30 : sept ou huit.
    assert!((7..=8).contains(&runs), "{runs} tirets");
    let covered = cols.iter().filter(|&&c| c).count() as f32;
    assert!((covered / 220.0 - 0.6).abs() < 0.12, "tiret 18 sur 30 : {}", covered / 220.0);
    // Défilement : 3 px par image, le motif de l'image k est celui de l'image 0 décalé de 3k.
    let f4 = render_with(&d, &dash_steps(&set, 4));
    let (a, b) = (ink_columns(&f0, 100), ink_columns(&f4, 100));
    let moved = (45..235).filter(|&x| b[x] != a[x - 12]).count();
    assert!(moved <= 2, "le motif a avancé de 12 px ({moved} colonnes diffèrent)");
    // Vitesse nulle : l'image ne change pas d'une image à l'autre ; vitesse négative : sens inverse.
    let still = [(t::DASH_SPEED, 0.0)];
    assert_eq!(differing(&render_with(&d, &dash_steps(&still, 0)), &render_with(&d, &dash_steps(&still, 9))), 0);
    let back = ink_columns(&render_with(&d, &dash_steps(&[(t::DASH_LENGTH, 18.0), (t::DASH_GAP, 12.0), (t::DASH_SPEED, -3.0)], 4)), 100);
    assert!((45..235).filter(|&x| back[x] != a[x + 12]).count() <= 2);
    // Les pointillés ne touchent pas aux pixels qu'ils laissent : au cœur d'un tiret, le trait est tel quel.
    let plain = d.render(W, H);
    let middle = (40..230).filter(|&x| cols[x] && cols[x - 4] && cols[x + 4] && (x as f32 - 30.0).rem_euclid(30.0) > 6.0 && (x as f32 - 30.0).rem_euclid(30.0) < 12.0);
    assert!(middle.clone().count() > 5);
    for x in middle {
        assert_eq!(f0.get(x, 100), plain.get(x, 100));
    }
}

// ------------------------------------------------------------------ désagrégation

fn dust_steps(set: &[(usize, f32)]) -> Vec<FxStep> {
    steps(EffectKind::Dust, set, 0)
}

fn total_ink(r: &Raster) -> u32 {
    (0..H).map(|y| (0..W).map(|x| r.get(x, y).a() as u32).sum::<u32>()).sum()
}

#[test]
fn the_dust_breaks_the_stroke_up_and_empties_it_at_100_percent() {
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(30.0, 100.0, 4.0), (250.0, 100.0, 4.0)])] };
    let plain = d.render(W, H);
    assert!(dust_steps(&[(t::DUST_PROGRESS, 0.0)]).is_empty(), "0 % : rien à faire");
    assert!(empty(&render_with(&d, &dust_steps(&[(t::DUST_PROGRESS, 100.0)]))), "100 % : plus rien");
    let some = render_with(&d, &dust_steps(&[(t::DUST_PROGRESS, 50.0), (t::DUST_SPREAD, 60.0)]));
    assert!(total_ink(&some) > 0 && total_ink(&some) < total_ink(&plain), "{} / {}", total_ink(&some), total_ink(&plain));
    // De la poussière hors de l'axe : de l'encre là où le trait n'en avait pas.
    let off_axis = (0..H).filter(|y| !(90..110).contains(y)).map(|y| (0..W).filter(|&x| some.get(x, y).a() > 0).count()).sum::<usize>();
    assert!(off_axis > 30, "{off_axis} grains hors du trait");
    // Les mêmes réglages, la même graine : la même poussière ; une autre graine : une autre.
    let again = render_with(&d, &dust_steps(&[(t::DUST_PROGRESS, 50.0), (t::DUST_SPREAD, 60.0)]));
    assert_eq!(differing(&some, &again), 0);
    let other = render_with(&d, &dust_steps(&[(t::DUST_PROGRESS, 50.0), (t::DUST_SPREAD, 60.0), (t::DUST_SEED, 2.0)]));
    assert!(differing(&some, &other) > 100);
    // L'avancement ne ramène jamais plus d'encre : sur l'ensemble, la masse baisse.
    let masses: Vec<u32> = (0..=10).map(|k| total_ink(&render_with(&d, &dust_steps(&[(t::DUST_PROGRESS, 10.0 * k as f32 + 1.0), (t::DUST_SPREAD, 60.0)])))).collect();
    assert!(masses.windows(2).all(|w| w[1] <= w[0] + w[0] / 20), "{masses:?}");
}

#[test]
fn the_dust_follows_the_drawing_direction_when_asked() {
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(30.0, 100.0, 4.0), (250.0, 100.0, 4.0)])] };
    let along = render_with(&d, &dust_steps(&[(t::DUST_PROGRESS, 45.0), (t::DUST_SENSE, 100.0), (t::DUST_SPREAD, 20.0), (t::DUST_DRIFT, 0.0)]));
    let (left, right) = (ink(&along, 100, 30, 100), ink(&along, 100, 180, 250));
    assert!(left < right, "le début du trait part le premier : {left} / {right}");
}

#[test]
fn the_stroke_effects_stack_and_stay_deterministic() {
    let mut elements = six_lines().elements;
    elements.push(smooth_stroke(&[(20.0, 200.0, 3.0), (120.0, 180.0, 5.0), (250.0, 205.0, 2.0)]));
    let d = VectorDrawing { elements };
    let stack = [
        effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 80.0), (t::WRITE_SOFT, 6.0)]),
        effect(EffectKind::Breath, &[(t::BREATH_AMPLITUDE, 50.0)]),
        effect(EffectKind::StrokeWave, &[(t::WAVE_AMPLITUDE, 3.0)]),
        effect(EffectKind::Dashes, &[(t::DASH_LENGTH, 40.0)]),
        effect(EffectKind::Dust, &[(t::DUST_PROGRESS, 20.0)]),
        effect(EffectKind::Sketch, &[(t::SKETCH_PASSES, 2.0)]),
    ];
    let s = fx::steps_at(&stack, 7);
    assert_eq!(s.len(), 6);
    let a = render_with(&d, &s);
    let b = render_with(&d, &s);
    assert_eq!(differing(&a, &b), 0);
    crate::parallele::EN_SERIE.with(|x| x.set(true));
    let serial = render_with(&d, &s);
    crate::parallele::EN_SERIE.with(|x| x.set(false));
    assert_eq!(differing(&a, &serial), 0, "un fil ou plusieurs");
    assert!(total_ink(&a) > 0);
}

// ------------------------------------------------------------------ traits texturés

#[test]
fn the_stroke_effects_work_on_textured_brush_strokes_without_replaying_them_per_frame() {
    crate::fxcache::monde::ecrire();
    let strokes = crate::affichage_tests::gros_traits(6, W, H, 11);
    let d = VectorDrawing { elements: strokes };
    let plain = d.render(W, H);
    let replays = crate::vector::replay_count();
    for frame in 0..4 {
        let _ = render_with(&d, &steps(EffectKind::Breath, &[(t::BREATH_AMPLITUDE, 50.0)], frame));
        let _ = render_with(&d, &steps(EffectKind::Dashes, &[], frame));
        let _ = render_with(&d, &steps(EffectKind::StrokeWave, &[], frame));
    }
    assert_eq!(crate::vector::replay_count(), replays, "les pixels en cache des traits servent à chaque image : pas de rejeu");
    // L'écriture n'ajoute jamais d'encre, et à 100 % rend le dessin.
    let half = render_with(&d, &write_steps(50.0, &[]));
    for y in 0..H {
        for x in 0..W {
            assert!(half.get(x, y).a() as u16 <= plain.get(x, y).a() as u16 + 1, "({x}, {y}) : {:?} contre {:?}", half.get(x, y), plain.get(x, y));
        }
    }
    assert!(total_ink(&half) > 0 && total_ink(&half) < total_ink(&plain));
    assert_eq!(differing(&render_with(&d, &write_steps(99.99, &[])), &plain), 0);
}

#[test]
fn playing_a_written_plan_never_replays_the_engine_on_the_interface_thread() {
    crate::fxcache::monde::ecrire();
    let d = VectorDrawing { elements: crate::affichage_tests::gros_traits(4, W, H, 3) };
    let mut e = Effect::new(EffectKind::Write);
    e.set_key(0, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 0.0)]).values);
    e.set_key(11, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 100.0)]).values);
    let doc = drawing_doc(d, vec![e], 12);
    let replays = crate::vector::replay_count();
    let (mut ready, mut waiting) = (0, 0);
    for frame in 0..12 {
        let plan = render::panel_plan_at(&doc, 0, frame, None);
        for item in &plan.items {
            let shown = fx::display(&doc, item, 0).expect("dessin");
            if shown.ready { ready += 1 } else { waiting += 1 }
        }
        let at = crate::playback::Position { panel: 0, frame, playing: true, montage: false, looping: true, range: (0, 12) };
        let _ = crate::playback::upcoming_derived(&doc, at, 0, 8);
        crate::fxcache::settle();
    }
    assert_eq!(crate::vector::replay_count(), replays, "le moteur n'est jamais rejoué sur le fil de l'interface");
    assert!(ready > 0 && waiting > 0, "{ready} prêts, {waiting} attendus");
}

// ------------------------------------------------------------------ mesures

/// Un hasard reproductible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}

/// Un dessin dense : `smooth` traits lisses et `textured` traits de brosses texturées (feutre, fusain,
/// gouache) de 120 à 400 pixels, sur une feuille `w` × `h`.
pub(crate) fn dense_drawing(smooth: usize, textured: usize, w: usize, h: usize) -> VectorDrawing {
    let mut rng = Lcg(7);
    let mut elements = Vec::with_capacity(smooth + textured);
    for _ in 0..smooth {
        let (x0, y0) = (40.0 + rng.next() * (w as f32 - 480.0), 40.0 + rng.next() * (h as f32 - 280.0));
        let (len, bend) = (120.0 + rng.next() * 280.0, (rng.next() - 0.5) * 160.0);
        let r = 1.2 + rng.next() * 2.8;
        let pts: Vec<(f32, f32, f32)> = (0..=8).map(|i| {
            let u = i as f32 / 8.0;
            (x0 + len * u, y0 + bend * (u * std::f32::consts::PI).sin() + (rng.next() - 0.5) * 6.0, r * (0.4 + 0.6 * (u * std::f32::consts::PI).sin()))
        }).collect();
        elements.push(smooth_stroke(&pts));
    }
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    for k in 0..textured {
        let name = ["Feutre", "Fusain", "Gouache"][k % 3];
        let mut s = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
        s.size = 5.0 + (k % 5) as f32 * 2.0;
        let (x0, y0) = (80.0 + (k * 97 % (w - 500)) as f32, 60.0 + (k * 53 % (h - 250)) as f32);
        let pts: Vec<_> = (0..=60)
            .map(|i| {
                let u = i as f32 / 60.0;
                (eframe::egui::pos2(x0 + 300.0 * u, y0 + 90.0 * (u * 5.0 + k as f32).sin()), 0.3 + 0.7 * (u * std::f32::consts::PI).sin(), i as f64 / 200.0)
            })
            .collect();
        elements.push(crate::affichage_tests::trait_rouge(&s, &lib, w, h, &pts));
    }
    VectorDrawing { elements }
}

fn ms_of(f: impl FnOnce()) -> f64 {
    let t0 = std::time::Instant::now();
    f();
    t0.elapsed().as_secs_f64() * 1e3
}

/// Les mesures du chapitre « Effets du trait » de `docs/PERFORMANCE.md` :
/// `cargo test --release --offline --target-dir target\msvc --lib fxtrait_tests::measure -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore]
fn measure_stroke_effects_on_a_dense_vector_layer() {
    crate::fxcache::monde::ecrire();
    let (w, h) = (1920usize, 1080usize);
    for (smooth, textured) in [(1800usize, 200usize), (180, 20)] {
        measure_dense(smooth, textured, w, h);
    }
}

fn measure_dense(smooth: usize, textured: usize, w: usize, h: usize) {
    let d = dense_drawing(smooth, textured, w, h);
    println!("{} traits ({} texturés), feuille {w} x {h}, {} fils", d.elements.len(), textured, crate::parallele::coeurs());
    let cold = ms_of(|| {
        let _ = d.render(w, h);
    });
    let warm = ms_of(|| {
        let _ = d.render(w, h);
    });
    println!("rendu du dessin sans effet : {cold:.0} ms la première fois (rejeu des traits texturés), {warm:.0} ms ensuite");
    let run = |name: &str, kind: EffectKind, set: &[(usize, f32)]| {
        let mut times = Vec::new();
        for frame in 0..5 {
            let s = steps(kind, set, frame * 3 + 1);
            times.push(ms_of(|| {
                let _ = t::render(&d, w, h, &s);
            }));
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        println!("{name:<46} {mean:>7.0} ms par image (min {:.0}, max {:.0})", times.iter().cloned().fold(f64::MAX, f64::min), times.iter().cloned().fold(0.0, f64::max));
    };
    run("écriture 50 %, trait par trait", EffectKind::Write, &[(t::WRITE_PROGRESS, 50.0)]);
    run("écriture 50 %, tous ensemble", EffectKind::Write, &[(t::WRITE_PROGRESS, 50.0), (t::WRITE_TOGETHER, 1.0)]);
    run("écriture 100 % (rien à faire)", EffectKind::Write, &[(t::WRITE_PROGRESS, 99.99)]);
    run("crayonné, 2 passes", EffectKind::Sketch, &[]);
    run("crayonné, 4 passes, décalage 8", EffectKind::Sketch, &[(t::SKETCH_PASSES, 4.0), (t::SKETCH_OFFSET, 8.0)]);
    run("respiration (35 %)", EffectKind::Breath, &[]);
    run("ondulation du trait (4 px)", EffectKind::StrokeWave, &[]);
    run("pointillés", EffectKind::Dashes, &[]);
    run("désagrégation 40 %", EffectKind::Dust, &[]);
    run("désagrégation 40 %, le long du trait, 300 px", EffectKind::Dust, &[(t::DUST_SENSE, 100.0), (t::DUST_SPREAD, 300.0)]);
    // La pile entière.
    let stack = [
        effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 80.0)]),
        effect(EffectKind::Breath, &[]),
        effect(EffectKind::StrokeWave, &[]),
        effect(EffectKind::Sketch, &[]),
    ];
    let s = fx::steps_at(&stack, 4);
    println!("écriture + respiration + ondulation + crayonné : {:.0} ms", ms_of(|| {
        let _ = t::render(&d, w, h, &s);
    }));
    // Le fil de l'interface : le plan, la recherche du dessin dérivé prêt, la copie du dessin vectoriel.
    let doc = drawing_doc_sized(d.clone(), vec![effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 50.0)])], 2, w, h);
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let copy = ms_of(|| {
        let _ = crate::fxcache::vector_source(id, &doc.cels[&id]);
    });
    let copy_again = ms_of(|| {
        let _ = crate::fxcache::vector_source(id, &doc.cels[&id]);
    });
    println!("fil de l'interface : copie du dessin vectoriel (une fois par version) {copy:.2} ms, ensuite {copy_again:.3} ms");
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    let first = ms_of(|| {
        let _ = fx::display(&doc, &plan.items[0], 0);
    });
    crate::fxcache::settle();
    let ready: Vec<f64> = (0..50).map(|_| ms_of(|| {
        let _ = fx::display(&doc, &plan.items[0], 0);
    })).collect();
    println!("fil de l'interface : demande du dessin dérivé {first:.2} ms ; image prête : {:.3} ms (pire {:.3})", ready.iter().sum::<f64>() / 50.0, ready.iter().cloned().fold(0.0, f64::max));
}

// ------------------------------------------------------------------ dans l'application

/// Une application sans fenêtre avec un plan de 24 images et un calque de la sorte `kind`.
fn app_with(name: &str, kind: LayerKind) -> crate::app::App {
    let dir = std::env::temp_dir().join(format!("shot-fxtrait-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(kind);
    let mut l = crate::document::Layer::new("Dessin", kind);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 6;
    app
}

#[test]
fn the_actions_toggle_each_stroke_effect_on_a_vector_layer_in_one_undoable_step() {
    let mut app = app_with("actions", LayerKind::Vector);
    for kind in t::KINDS {
        let steps_before = app.history.names().0.len();
        app.run_action(t::action(kind));
        assert!(app.fx_on(0, kind), "{kind:?} est coché");
        assert_eq!(app.history.names().0.len(), steps_before + 1, "une étape d'annulation");
        assert_eq!(app.layers()[0].fx.iter().filter(|e| e.kind == kind).count(), 1);
        app.run_action(t::action(kind));
        assert!(!app.fx_on(0, kind), "{kind:?} est décoché, mais reste sur le calque");
        assert_eq!(app.layers()[0].fx.iter().filter(|e| e.kind == kind).count(), 1);
    }
    app.undo();
    assert!(app.fx_on(0, EffectKind::Dust), "défaire remet l'effet coché");
    // Ajouter deux fois par le menu « Ajouter un effet » ne double pas l'effet.
    let mut app = app_with("une-fois", LayerKind::Vector);
    app.add_effect_once(0, EffectKind::Sketch);
    app.add_effect_once(0, EffectKind::Sketch);
    assert_eq!(app.layers()[0].fx.len(), 1);
}

#[test]
fn a_bitmap_layer_refuses_the_stroke_effects_and_says_why() {
    let mut app = app_with("bitmap", LayerKind::Bitmap);
    for kind in t::KINDS {
        assert!(matches!(app.fx_offer(0, kind), crate::fxui::Offer::Disabled(why) if why.contains("calque vectoriel")));
        app.run_action(t::action(kind));
        assert!(app.layers()[0].fx.is_empty(), "{kind:?} n'est pas posé sur un bitmap");
        assert!(app.status.contains("vectoriel"), "{}", app.status);
    }
    app.apply_trait_preset(t::Preset::Appearance);
    assert!(app.layers()[0].fx.is_empty());
}

#[test]
fn the_presets_write_keyed_effects_from_the_playhead_and_replace_their_own_kind() {
    let mut app = app_with("presets", LayerKind::Vector);
    app.apply_trait_preset(t::Preset::Appearance);
    let fx = &app.layers()[0].fx;
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].kind, EffectKind::Write);
    assert_eq!(fx[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![6, 23], "de la tête de lecture à la fin du plan (une seconde = 24 images, coupée à 23)");
    assert_eq!((fx[0].values_at(6)[t::WRITE_PROGRESS], fx[0].values_at(23)[t::WRITE_PROGRESS]), (0.0, 100.0));
    assert_eq!(fx[0].values_at(0)[t::WRITE_PROGRESS], 0.0, "avant la première clé : sa valeur");
    assert!(fx::steps_at(fx, 23).is_empty(), "fini : rien à faire");
    // Une seconde fois : remplacé, pas doublé.
    app.cur_frame = 2;
    app.apply_trait_preset(t::Preset::Appearance);
    assert_eq!(app.layers()[0].fx.len(), 1);
    assert_eq!(app.layers()[0].fx[0].keys[0].frame, 2);
    app.apply_trait_preset(t::Preset::Dust);
    app.apply_trait_preset(t::Preset::NervousSketch);
    let kinds: Vec<EffectKind> = app.layers()[0].fx.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EffectKind::Write, EffectKind::Dust, EffectKind::Sketch, EffectKind::Boil]);
    // Chaque préréglage a son action, dans la palette, et un nom français.
    let palette = crate::palette::palette_commands();
    for p in t::Preset::ALL {
        assert_eq!(t::Preset::from_action(p.action()), Some(p));
        assert!(palette.iter().any(|(id, _)| *id == p.action()), "{} absent de la palette", p.action());
        assert!(!p.label().is_empty() && !p.note().is_empty());
    }
    for kind in t::KINDS {
        assert!(palette.iter().any(|(id, _)| *id == t::action(kind)), "{} absent de la palette", t::action(kind));
        assert_eq!(t::kind_of_action(t::action(kind)), Some(kind));
    }
    // Ils se défont d'un coup.
    let n = app.layers()[0].fx.len();
    app.undo();
    assert!(app.layers()[0].fx.len() < n);
}

#[test]
fn the_presets_build_the_same_effects_every_time() {
    for p in t::Preset::ALL {
        assert_eq!(p.build(24, 5, 100), p.build(24, 5, 100));
        for e in p.build(24, 5, 100) {
            assert_eq!(e.values_at(5).len(), e.kind.params().len());
        }
    }
    // Au bout du plan : les clés ne sortent pas du plan.
    for e in t::Preset::Appearance.build(24, 99, 100) {
        assert!(e.keys.iter().all(|k| k.frame <= 100));
    }
}

#[test]
fn copy_and_paste_keep_one_stroke_effect_of_each_kind_per_layer() {
    use crate::fxcopie::paste_onto;
    let mut stack = vec![Effect::new(EffectKind::Write)];
    let landed = paste_onto(&mut stack, &[Effect::new(EffectKind::Write), Effect::new(EffectKind::Dust), Effect::new(EffectKind::GaussianBlur)], true, 24, false);
    assert_eq!(stack.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![EffectKind::Write, EffectKind::Dust, EffectKind::GaussianBlur]);
    assert_eq!(landed.duplicates_skipped, vec![EffectKind::Write]);
    // Sur un calque bitmap : laissés de côté, le flou passe.
    let mut bitmap = Vec::new();
    let landed = paste_onto(&mut bitmap, &[Effect::new(EffectKind::Sketch), Effect::new(EffectKind::GaussianBlur)], false, 24, false);
    assert_eq!(landed.vector_only_skipped, vec![EffectKind::Sketch]);
    assert_eq!(bitmap.len(), 1);
    // Les clés et les réglages se copient tels quels.
    let mut keyed = Effect::new(EffectKind::Write);
    keyed.set_key(3, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 0.0), (t::WRITE_SOFT, 12.0)]).values);
    keyed.set_key(9, effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 100.0), (t::WRITE_SOFT, 12.0)]).values);
    let mut other = Vec::new();
    paste_onto(&mut other, std::slice::from_ref(&keyed), true, 24, false);
    assert_eq!(other, vec![keyed]);
}

#[test]
fn the_stroke_effects_stay_on_the_background_thread_and_the_boil_goes_to_the_graphics_card() {
    // Le tremblement a sa passe sur la carte graphique : une pile avec flou et tremblement y passe
    // entière ; un effet du trait, lui, garde le calcul de fond (il lit les traits du dessin).
    let doc = drawing_doc(six_lines(), Vec::new(), 1);
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let on_card = |steps: Vec<FxStep>| {
        let item = PlanItem { cel: id, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: steps, ctg: None, fond: None, pre: None };
        crate::gpu::gpu_fx_item(&doc, &item)
    };
    let boil = FxStep { kind: EffectKind::Boil, values: vec![3.0, 55.0, 3.0, 2.0, 0.0, 1.0, 0.0, 1.0] };
    let blur = FxStep { kind: EffectKind::GaussianBlur, values: vec![6.0] };
    assert!(on_card(vec![boil.clone()]));
    assert!(on_card(vec![blur.clone(), boil.clone()]));
    assert!(!on_card(write_steps(50.0, &[])), "un effet du trait seul : le fil de fond");
    // Un effet du trait mêlé à des effets de la carte : le fil de fond calcule le dessin dérivé par l'effet du
    // trait, la carte passe le flou et le tremblement dessus (plus de pile entière sur le processeur).
    let mixed = [write_steps(50.0, &[]), vec![blur.clone(), boil.clone()]].concat();
    assert!(on_card(mixed.clone()));
    let item = PlanItem { cel: id, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: mixed, ctg: None, fond: None, pre: None };
    let (line, rest) = fx::hybrid_split(&item).expect("pile mêlée");
    assert_eq!(line.fx.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![EffectKind::Write]);
    assert_eq!(rest.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![EffectKind::GaussianBlur, EffectKind::Boil]);
    // Pas mêlée : rien à séparer.
    assert!(fx::hybrid_split(&PlanItem { fx: vec![blur, boil], ..item.clone() }).is_none());
    assert!(fx::hybrid_split(&PlanItem { fx: write_steps(50.0, &[]), ..item.clone() }).is_none());
    // Un remplissage CTG ou une feuille flottante : tout sur le processeur.
    assert!(fx::hybrid_split(&PlanItem { ctg: Some(crate::ctg::CtgInput { sources: vec![id], gap: 4 }), ..item }).is_none());
}

#[test]
fn a_mixed_stack_gives_the_cpu_pixels_with_the_card_drawing_the_card_effects() {
    use crate::gpu::GpuCanvas;
    let Some(rs) = crate::gpu_tests::headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    crate::fxcache::monde::ecrire();
    let boil = FxStep { kind: EffectKind::Boil, values: vec![3.0, 55.0, 3.0, 2.0, 0.0, 1.0, 0.0, 1.0] };
    let steps = [write_steps(45.0, &[(t::WRITE_SOFT, 4.0)]), vec![FxStep { kind: EffectKind::GaussianBlur, values: vec![3.0] }, boil]].concat();
    let doc = drawing_doc(six_lines(), Vec::new(), 1);
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let plan = render::Plan { paper: None, items: vec![PlanItem { cel: id, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: steps, ctg: None, fond: None, pre: None }] };
    let want = render::composite_full(&doc, &plan);
    let mut gpu = GpuCanvas::new(&rs);
    let mut worst = (u8::MAX, 0usize);
    for _ in 0..300 {
        let _ = crate::gpu_tests::render_with(&mut gpu, &rs, &doc, &plan, true);
        crate::fxcache::settle();
        let _ = crate::gpu_tests::render_with(&mut gpu, &rs, &doc, &plan, true);
        let got = gpu.read_frame();
        let far = |a: Color32, b: Color32| a.to_array().iter().zip(b.to_array()).map(|(x, y)| (*x as i32 - y as i32).unsigned_abs()).max().unwrap_or(0);
        worst = (0, got.iter().zip(&want).filter(|(a, b)| far(**a, **b) > 2).count());
        if worst.1 == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(worst.1, 0, "{} pixels à plus de 2 niveaux de la référence", worst.1);
    assert!(gpu.effects_passes() > 0, "le flou et le tremblement sont dessinés par la carte");
}

/// Un petit dessin à regarder : un visage, des cheveux, un soleil, un sol, de quelques épaisseurs.
fn sample_drawing() -> VectorDrawing {
    let arc = |cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, r: f32| {
        let pts: Vec<(f32, f32, f32)> = (0..=40)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / 40.0;
                (cx + rx * a.cos(), cy + ry * a.sin(), r * (0.55 + 0.45 * (i as f32 / 40.0 * std::f32::consts::PI).sin()))
            })
            .collect();
        smooth_stroke(&pts)
    };
    let tau = std::f32::consts::TAU;
    let mut elements = vec![
        arc(240.0, 160.0, 90.0, 105.0, 0.0, tau, 3.2),
        arc(205.0, 135.0, 14.0, 10.0, 0.0, tau, 2.4),
        arc(275.0, 135.0, 14.0, 10.0, 0.0, tau, 2.4),
        arc(240.0, 180.0, 40.0, 26.0, 0.25, 2.9, 2.8),
        smooth_stroke(&[(240.0, 140.0, 2.0), (234.0, 165.0, 2.4), (246.0, 168.0, 2.0)]),
        arc(240.0, 150.0, 118.0, 130.0, 3.5, 5.9, 4.5),
        arc(520.0, 90.0, 38.0, 38.0, 0.0, tau, 3.0),
        smooth_stroke(&[(20.0, 300.0, 3.0), (200.0, 312.0, 4.0), (380.0, 296.0, 3.0), (620.0, 306.0, 4.5)]),
    ];
    for k in 0..10 {
        let a = tau * k as f32 / 10.0;
        elements.push(smooth_stroke(&[(520.0 + 48.0 * a.cos(), 90.0 + 48.0 * a.sin(), 2.2), (520.0 + 72.0 * a.cos(), 90.0 + 72.0 * a.sin(), 1.4)]));
    }
    VectorDrawing { elements }
}

/// Regarder les effets : une planche de contact écrite dans le dossier donné par `FXTRAIT_PNG`.
/// `$env:FXTRAIT_PNG = 'C:\\...'; cargo test --release --offline --target-dir target\msvc --lib fxtrait_tests::dump -- --ignored --nocapture`
#[test]
#[ignore]
fn dump_stroke_effects_to_png() {
    let Ok(dir) = std::env::var("FXTRAIT_PNG") else { return };
    for (file, d) in [("effets_du_trait.png", sample_drawing()), ("effets_du_trait_brosses.png", VectorDrawing { elements: crate::affichage_tests::gros_traits(5, 640, 340, 5) })] {
        dump_sheet(&dir, file, &d);
    }
}

fn dump_sheet(dir: &str, file: &str, d: &VectorDrawing) {
    let (w, h) = (640usize, 340usize);
    let panels: Vec<(&str, Vec<FxStep>)> = vec![
        ("dessin", Vec::new()),
        ("ecriture 40", write_steps(40.0, &[(t::WRITE_SOFT, 20.0)])),
        ("ecriture ensemble 50", write_steps(50.0, &[(t::WRITE_TOGETHER, 1.0), (t::WRITE_SOFT, 10.0)])),
        ("effacement 60", write_steps(60.0, &[(t::WRITE_ERASE, 1.0)])),
        ("crayonne", steps(EffectKind::Sketch, &[(t::SKETCH_OFFSET, 5.0), (t::SKETCH_PASSES, 3.0)], 0)),
        ("respiration 6", steps(EffectKind::Breath, &[(t::BREATH_AMPLITUDE, 70.0)], 6)),
        ("respiration vague", steps(EffectKind::Breath, &[(t::BREATH_AMPLITUDE, 80.0), (t::BREATH_WAVE, 90.0)], 6)),
        ("ondulation", steps(EffectKind::StrokeWave, &[(t::WAVE_AMPLITUDE, 6.0), (t::WAVE_LENGTH, 70.0)], 3)),
        ("pointilles", steps(EffectKind::Dashes, &[(t::DASH_LENGTH, 14.0), (t::DASH_GAP, 9.0)], 2)),
        ("poussiere 40", steps(EffectKind::Dust, &[(t::DUST_PROGRESS, 40.0)], 0)),
        ("poussiere 70 sens", steps(EffectKind::Dust, &[(t::DUST_PROGRESS, 70.0), (t::DUST_SENSE, 100.0), (t::DUST_GRAIN, 3.0)], 0)),
        ("ecriture + crayonne", [write_steps(70.0, &[]), steps(EffectKind::Sketch, &[(t::SKETCH_OFFSET, 4.0)], 0)].concat()),
    ];
    let cols = 3usize;
    let rows = panels.len().div_ceil(cols);
    let mut sheet = vec![eframe::egui::Color32::WHITE; cols * w * rows * h];
    for (n, (name, s)) in panels.iter().enumerate() {
        let r = t::render(d, w, h, s);
        let (ox, oy) = ((n % cols) * w, (n / cols) * h);
        for y in 0..h {
            for x in 0..w {
                let c = crate::render::over(eframe::egui::Color32::WHITE, r.get(x, y), 255, None);
                sheet[(oy + y) * cols * w + ox + x] = c;
            }
        }
        // Un liseré pour séparer les vignettes.
        for x in 0..w {
            sheet[oy * cols * w + ox + x] = eframe::egui::Color32::from_gray(200);
        }
        for y in 0..h {
            sheet[(oy + y) * cols * w + ox] = eframe::egui::Color32::from_gray(200);
        }
        println!("{n} : {name}");
    }
    std::fs::create_dir_all(dir).ok();
    crate::io::write_png(&std::path::Path::new(dir).join(file), &sheet, cols * w, rows * h).unwrap();
}