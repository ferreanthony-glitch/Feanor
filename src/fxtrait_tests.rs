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
    let mut doc = Document::new(W, H, 24);
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