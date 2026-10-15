//! Pile d'effets (13i) : clés, interpolation, flous, dessins dérivés, enregistrement.

use eframe::egui::Color32;

use crate::document::{Document, LayerKind};
use crate::fx::{self, Effect, EffectKind, FxStep};

#[test]
fn values_follow_the_keys_in_straight_lines() {
    let mut e = Effect::new(EffectKind::GaussianBlur);
    assert_eq!(e.values_at(5), vec![8.0], "sans clé : la valeur de l'effet");
    e.set_key(10, vec![0.0]);
    e.set_key(20, vec![20.0]);
    assert_eq!(e.values_at(0), vec![0.0], "avant la première clé : sa valeur");
    assert_eq!(e.values_at(15), vec![10.0]);
    assert_eq!(e.values_at(30), vec![20.0], "après la dernière : sa valeur");
    e.set_key(20, vec![40.0]);
    assert_eq!(e.keys.len(), 2, "une clé à la même image est remplacée");
    e.set_key(5, vec![500.0]);
    assert_eq!(e.keys[0].frame, 5, "les clés restent dans l'ordre");
    assert_eq!(e.values_at(5), vec![200.0], "bornée au maximum du paramètre");
    assert!(e.remove_key(5) && !e.remove_key(5));
    assert_eq!(e.key_at(20), Some(1));
}

#[test]
fn keys_follow_inserted_and_removed_images_and_cut_plans() {
    let mut fx = vec![Effect::new(EffectKind::MotionBlur)];
    fx[0].set_key(2, vec![10.0, 0.0]);
    fx[0].set_key(8, vec![30.0, 90.0]);
    fx::shift_keys(&mut fx, 5, 3, false);
    assert_eq!(fx[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 11]);
    fx::shift_keys(&mut fx, 1, 2, true);
    assert_eq!(fx[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![9], "la clé de l'image 2 part");
    // Plan coupé à l'image 4 : le second garde la clé 9 → 5.
    let second = fx::keys_from(&fx, 4);
    assert_eq!(second[0].keys[0].frame, 5);
    // Coupé après la dernière clé : le second tient la valeur de la coupe.
    let late = fx::keys_from(&fx, 12);
    assert_eq!((late[0].keys[0].frame, late[0].keys[0].values.clone()), (0, vec![30.0, 90.0]));
}

#[test]
fn only_active_effects_that_change_something_apply() {
    let mut fx = vec![Effect::new(EffectKind::GaussianBlur), Effect::new(EffectKind::MotionBlur)];
    assert_eq!(fx::steps_at(&fx, 0).len(), 2);
    fx[0].enabled = false;
    fx[1].values = vec![0.0, 45.0];
    assert!(fx::steps_at(&fx, 0).is_empty(), "inactif, ou longueur nulle");
}

/// Un dessin 64×64 avec un seul point opaque au milieu.
fn dot() -> (Document, u64) {
    let mut doc = Document::new(64, 64, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&id).unwrap().raster.tile_mut(0)[32 * 64 + 32] = Color32::BLACK;
    (doc, id)
}

#[test]
fn gaussian_blur_spreads_the_drawing_around() {
    let (doc, id) = dot();
    let out = fx::apply(&doc.cels[&id].raster, &[FxStep { kind: EffectKind::GaussianBlur, values: vec![9.0] }]);
    let a = |x, y| out.get(x, y).a();
    assert!(a(32, 32) > 0 && a(32, 32) < 255, "le point s'étale : {}", a(32, 32));
    assert!(a(35, 32) > 0 && a(32, 35) > 0, "dans les deux directions");
    assert_eq!(a(32, 32 - 20), 0, "rien au-delà de la portée");
    assert!(a(32, 32) >= a(35, 32), "plus dense au centre");
}

#[test]
fn motion_blur_spreads_along_its_direction_only() {
    let (doc, id) = dot();
    let out = fx::apply(&doc.cels[&id].raster, &[FxStep { kind: EffectKind::MotionBlur, values: vec![12.0, 0.0] }]);
    let a = |x, y| out.get(x, y).a();
    assert!(a(37, 32) > 0 && a(27, 32) > 0, "à l'horizontale, des deux côtés");
    assert_eq!(a(32, 37), 0, "rien à la verticale");
    let vertical = fx::apply(&doc.cels[&id].raster, &[FxStep { kind: EffectKind::MotionBlur, values: vec![12.0, 90.0] }]);
    assert!(vertical.get(32, 37).a() > 0 && vertical.get(37, 32).a() == 0);
}

#[test]
fn derived_drawings_are_kept_until_the_drawing_changes() {
    let (mut doc, id) = dot();
    let steps = vec![FxStep { kind: EffectKind::GaussianBlur, values: vec![4.0] }];
    let a = match fx::shown_with(&doc, id, &steps, None).unwrap() {
        fx::Shown::Fx(c) => c,
        fx::Shown::Plain(_) => panic!("un dessin dérivé"),
    };
    let b = match fx::shown_with(&doc, id, &steps, None).unwrap() {
        fx::Shown::Fx(c) => c,
        fx::Shown::Plain(_) => panic!(),
    };
    assert!(std::sync::Arc::ptr_eq(&a, &b), "calculé une fois");
    assert_ne!(fx::fx_id(id, &steps), id);
    doc.cels.get_mut(&id).unwrap().version += 1;
    let c = match fx::shown_with(&doc, id, &steps, None).unwrap() {
        fx::Shown::Fx(c) => c,
        fx::Shown::Plain(_) => panic!(),
    };
    assert!(!std::sync::Arc::ptr_eq(&a, &c), "recalculé quand le dessin change");
    assert!(matches!(fx::shown_with(&doc, id, &[], None).unwrap(), fx::Shown::Plain(_)), "sans effet : le dessin lui-même");
}

#[test]
fn the_export_composites_the_blurred_drawing() {
    let (mut doc, id) = dot();
    let layer = &mut doc.panels[0].layers[0];
    layer.frames = vec![Some(id)];
    layer.fx.push(Effect::new(EffectKind::GaussianBlur));
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert_eq!(plan.items[0].fx.len(), 1);
    let at = |x, y| crate::render::composite_pixel(&doc, &plan, x, y).a();
    assert!(at(35, 32) > 0, "le flou se voit à l'export");
}

#[test]
fn the_effect_stack_is_saved() {
    let (mut doc, id) = dot();
    let layer = &mut doc.panels[0].layers[0];
    layer.frames = vec![Some(id)];
    let mut e = Effect::new(EffectKind::MotionBlur);
    e.set_key(0, vec![20.0, 45.0]);
    e.enabled = false;
    layer.fx = vec![Effect::new(EffectKind::GaussianBlur), e];
    let dir = std::env::temp_dir().join(format!("pelure-test-effets-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("E.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"flou_mouvement\""), "{text}");
    let back = crate::project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.panels[0].layers[0].fx, doc.panels[0].layers[0].fx);
}
