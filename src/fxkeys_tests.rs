//! Clés d'effets dans le montage et la timeline : les losanges, le choix, le déplacement, le
//! copier-coller, la suppression, l'annulation.

use crate::document::Document;
use crate::fx::{Effect, EffectKind};
use crate::fxkeys::{
    choose, clamp_delta, clamp_ripple, copy_keys, delete_keys, keys_between, marker_frames, marker_near, move_keys, paste_insert_keys, paste_keys, rank_after_remove, rank_after_swap, refs_on,
    ripple_keys, shown, FxClip, FxKeyRef, FxSel, Inserted, Pasted,
};
use crate::history::{Edit, History, Nav};

/// Une pile : un flou aux clés 2 et 6, un flou de mouvement à la clé 6 (et une lueur sans clé).
fn stack() -> Vec<Effect> {
    let mut blur = Effect::new(EffectKind::GaussianBlur);
    blur.set_key(2, vec![4.0]);
    blur.set_key(6, vec![12.0]);
    let mut motion = Effect::new(EffectKind::MotionBlur);
    motion.set_key(6, vec![20.0, 90.0]);
    vec![blur, motion, Effect::new(EffectKind::Glow)]
}

fn frames(e: &Effect) -> Vec<usize> {
    e.keys.iter().map(|k| k.frame).collect()
}

fn key(panel: u64, layer: u64, effect: usize, frame: usize) -> FxKeyRef {
    FxKeyRef { panel, layer, effect, frame }
}

/// L'effet de rang `effect` du calque 7 du plan 1.
fn sel(effect: usize) -> FxSel {
    FxSel { panel: 1, layer: 7, effect }
}

#[test]
fn one_marker_per_key_of_the_chosen_effect_only() {
    let fx = stack();
    assert_eq!(marker_frames(&fx[0]), vec![2, 6]);
    assert_eq!(marker_frames(&fx[1]), vec![6], "le flou de mouvement n'a que sa clé : celles du flou ne s'y mêlent pas");
    assert!(marker_frames(&fx[2]).is_empty(), "un effet sans clé n'a pas de losange");
    assert!(marker_frames(&Effect::new(EffectKind::Glow)).is_empty());
    // Un losange = la clé de l'effet choisi, jamais celles des autres effets de la même image.
    assert_eq!(refs_on(sel(0), &fx[0], 6), vec![key(1, 7, 0, 6)]);
    assert_eq!(refs_on(sel(1), &fx[1], 6), vec![key(1, 7, 1, 6)]);
    assert!(refs_on(sel(0), &fx[0], 3).is_empty());
}

#[test]
fn keys_show_only_for_the_chosen_effect_on_its_own_layer() {
    let mut doc = Document::new(64, 64, 24);
    doc.panels[0].layers[0].fx = stack();
    let (panel, layer) = (doc.panels[0].id, doc.panels[0].layers[0].id);
    let on = FxSel { panel, layer, effect: 0 };
    let l = &doc.panels[0].layers[0];
    assert_eq!(shown(Some(on), true, panel, l), Some(on));
    assert_eq!(shown(None, true, panel, l), None, "aucun effet choisi : aucun losange");
    assert_eq!(shown(Some(on), false, panel, l), None, "la case du panneau Effets est l'interrupteur général");
    assert_eq!(shown(Some(FxSel { effect: 3, ..on }), true, panel, l), None, "un rang qui n'existe plus ne montre rien");
    assert_eq!(shown(Some(FxSel { layer: layer + 1, ..on }), true, panel, l), None, "l'effet d'un autre calque ne montre rien ici");
    assert_eq!(shown(Some(FxSel { panel: panel + 1, ..on }), true, panel, l), None, "ni celui d'un autre plan");
    // Un calque sans effet, un dossier : rien.
    let mut folder = l.clone();
    folder.folder = true;
    assert_eq!(shown(Some(on), true, panel, &folder), None);
}

#[test]
fn the_chosen_rank_follows_swaps_and_removals() {
    assert_eq!(rank_after_swap(0, 0, 1), 1);
    assert_eq!(rank_after_swap(1, 0, 1), 0);
    assert_eq!(rank_after_swap(2, 0, 1), 2);
    assert_eq!(rank_after_remove(1, 1), None, "l'effet retiré n'est plus choisi");
    assert_eq!(rank_after_remove(2, 0), Some(1), "il monte d'un rang");
    assert_eq!(rank_after_remove(0, 2), Some(0));
}

#[test]
fn the_nearest_marker_under_the_pointer_wins() {
    let x = |f: usize| 10.0 + f as f32 * 8.0;
    let frames = [2, 3, 9];
    assert_eq!(marker_near(&frames, x(2) + 1.0, x, 5.0), Some(2));
    assert_eq!(marker_near(&frames, x(3) - 1.0, x, 5.0), Some(3), "entre deux losanges : le plus proche");
    assert_eq!(marker_near(&frames, x(5), x, 5.0), None, "trop loin de tous");
    assert_eq!(marker_near(&[], 0.0, x, 5.0), None);
}

#[test]
fn a_click_chooses_one_marker_ctrl_toggles_shift_takes_the_range() {
    let fx = stack();
    let (a, b, c) = (refs_on(sel(0), &fx[0], 2), refs_on(sel(0), &fx[0], 6), keys_between(sel(0), &fx[0], 2, 6));
    assert_eq!(a, vec![key(1, 7, 0, 2)]);
    assert_eq!(b, vec![key(1, 7, 0, 6)], "un losange = la clé de l'effet choisi");
    assert_eq!(c.len(), 2, "Maj : les clés de cet effet seulement");
    // Un clic : lui seul.
    let (chosen, anchor) = choose(&[], None, &a, false, None);
    assert_eq!((chosen.clone(), anchor), (a.clone(), Some(a[0])));
    let (chosen, _) = choose(&chosen, anchor, &b, false, None);
    assert_eq!(chosen, b, "un autre clic remplace le choix");
    // Ctrl : s'ajoute, puis se retire.
    let (chosen, anchor) = choose(&chosen, anchor, &a, true, None);
    assert_eq!(chosen.len(), 2);
    let (sel, _) = choose(&chosen, anchor, &b, true, None);
    assert_eq!(sel, a, "Ctrl + clic sur un losange entièrement choisi le retire");
    // Maj : tout ce qui est entre l'ancre et lui.
    let (sel, anchor) = choose(&sel, Some(a[0]), &b, false, Some(c.clone()));
    assert_eq!((sel, anchor), (c, Some(a[0])));
    // Maj sans ancre : un simple clic.
    let (sel, _) = choose(&[], None, &b, false, Some(vec![]));
    assert_eq!(sel, b);
    // Pas de losange sous le clic : le choix ne bouge pas (c'est l'appelant qui le lâche).
    assert_eq!(choose(&b, None, &[], false, None).0, b);
}

#[test]
fn the_shift_stays_inside_the_plan_and_before_frame_zero() {
    // Clés aux images 2 et 6 d'un plan de 10 images.
    let g = [(vec![2usize, 6], 10usize)];
    assert_eq!(clamp_delta(&g, -5), -2, "pas avant l'image 0");
    assert_eq!(clamp_delta(&g, 9), 3, "pas après la dernière image du plan (9)");
    assert_eq!(clamp_delta(&g, 2), 2);
    // Plusieurs calques : le plus serré décide.
    let g = [(vec![1usize], 10usize), (vec![4usize, 5], 8)];
    assert_eq!(clamp_delta(&g, 50), 2, "le plan de 8 images s'arrête à l'image 7");
    assert_eq!(clamp_delta(&g, -50), -1);
    // Une clé déjà après la fin du plan peut rester où elle est, pas plus loin.
    let g = [(vec![12usize], 10usize)];
    assert_eq!(clamp_delta(&g, 3), 0);
    assert_eq!(clamp_delta(&g, -4), -4);
    assert_eq!(clamp_delta(&[], 3), 0);
}

#[test]
fn moved_keys_keep_their_values_and_their_order() {
    let mut fx = stack();
    // Les deux clés du flou, de 3 images.
    let at = move_keys(&mut fx, &[(0, 2), (0, 6)], 3);
    assert_eq!(at, vec![(0, 5), (0, 9)]);
    assert_eq!(frames(&fx[0]), vec![5, 9]);
    assert_eq!(fx[0].keys[0].values, vec![4.0], "les paramètres suivent leur clé");
    assert_eq!(frames(&fx[1]), vec![6], "les autres effets ne bougent pas");
    // Vers l'arrière, en passant la clé d'un autre : l'ordre reste celui des images.
    let mut fx = stack();
    move_keys(&mut fx, &[(0, 6)], -3);
    assert_eq!(frames(&fx[0]), vec![2, 3]);
    move_keys(&mut fx, &[(0, 3)], -1);
    assert_eq!(frames(&fx[0]), vec![2], "tombée sur une autre clé du même effet, elle la remplace");
    assert_eq!(fx[0].keys[0].values, vec![12.0], "c'est la clé déplacée qui reste");
    // Rien choisi : rien ne bouge.
    let before = stack();
    let mut fx = stack();
    assert!(move_keys(&mut fx, &[], 4).is_empty());
    assert_eq!(fx, before);
}

#[test]
fn moving_an_unselected_neighbour_is_left_alone_when_a_key_lands_beside_it() {
    let mut fx = stack();
    // Le flou : 2 et 6. On pousse la clé 2 jusqu'à 5, à côté de la 6 qui reste.
    move_keys(&mut fx, &[(0, 2)], 3);
    assert_eq!(frames(&fx[0]), vec![5, 6]);
}

#[test]
fn copied_keys_land_on_the_chosen_effect_at_the_playhead() {
    let fx = stack();
    // Copie : les deux clés du flou (2 et 6), dans le désordre.
    let clip = copy_keys(&fx[0], &[6, 2]);
    assert_eq!(clip.kind, EffectKind::GaussianBlur);
    assert_eq!(clip.keys.iter().map(|k| k.offset).collect::<Vec<_>>(), vec![0, 4]);
    // Un autre flou, sur un autre calque : les clés y vont à l'image courante, avec leurs paramètres.
    let mut other = Effect::new(EffectKind::GaussianBlur);
    let done = paste_keys(&mut other, &clip, 10, 40);
    assert_eq!(done, Pasted { placed: vec![10, 14], skipped: 0 });
    assert_eq!(frames(&other), vec![10, 14]);
    assert_eq!(other.keys[0].values, vec![4.0]);
    assert_eq!(other.keys[1].values, vec![12.0]);
    // Une image absente de l'effet n'est pas copiée.
    assert!(copy_keys(&fx[0], &[3]).keys.is_empty());
}

#[test]
fn a_paste_replaces_a_key_on_the_same_frame_and_skips_what_does_not_fit() {
    let fx = stack();
    let clip = copy_keys(&fx[0], &[2, 6]);
    // Un plan de 12 images, collage à l'image 8 : la clé de l'écart 4 tomberait en 12, hors du plan.
    let mut target = Effect::new(EffectKind::GaussianBlur);
    target.set_key(8, vec![99.0]);
    let done = paste_keys(&mut target, &clip, 8, 12);
    assert_eq!(done, Pasted { placed: vec![8], skipped: 1 });
    assert_eq!(target.keys.len(), 1);
    assert_eq!(target.keys[0].values, vec![4.0], "la clé collée remplace celle qui y était");
    // Un effet d'une autre sorte que celui copié : rien n'est collé, ses paramètres ne lui vont pas.
    let mut other_kind = Effect::new(EffectKind::Glow);
    let done = paste_keys(&mut other_kind, &clip, 0, 12);
    assert_eq!(done, Pasted { placed: vec![], skipped: 2 });
    assert!(other_kind.keys.is_empty());
    // Un presse-papiers vide ne fait rien.
    let mut same = stack().remove(0);
    assert_eq!(paste_keys(&mut same, &FxClip { kind: EffectKind::GaussianBlur, keys: vec![] }, 3, 10), Pasted { placed: vec![], skipped: 0 });
    assert_eq!(same, stack().remove(0));
}

#[test]
fn deleting_removes_only_the_chosen_keys() {
    let mut fx = stack();
    assert_eq!(delete_keys(&mut fx, &[(0, 2), (1, 6)]), 2);
    assert_eq!(frames(&fx[0]), vec![6]);
    assert!(fx[1].keys.is_empty());
    assert_eq!(delete_keys(&mut fx, &[(0, 2), (5, 0)]), 0, "une clé déjà partie, un effet absent : rien");
    // Rien choisi : rien ne part.
    let before = fx.clone();
    assert_eq!(delete_keys(&mut fx, &[]), 0);
    assert_eq!(fx, before);
}

// ---------------------------------------------------------------------- décaler, insérer (2026-10-19)

/// Un flou aux clés 2, 6, 12 et 18 (plan de 24 images).
fn blur_2_6_12_18() -> Effect {
    let mut e = Effect::new(EffectKind::GaussianBlur);
    for f in [2usize, 6, 12, 18] {
        e.set_key(f, vec![f as f32]);
    }
    e
}

/// Comme l'interface : décalage ramené dans les bornes, puis les clés bougent. `ripple` : les clés
/// d'après suivent ; sinon Maj tenue, les clés choisies seules.
fn drag_fx(fx: &mut [Effect], chosen: &[usize], want: isize, ripple: bool) -> isize {
    let keys: Vec<(usize, usize)> = chosen.iter().map(|&f| (0, f)).collect();
    let (delta, moving) = if ripple {
        let all = marker_frames(&fx[0]);
        (clamp_ripple(&[(all, *chosen.iter().min().unwrap(), 24)], want), ripple_keys(fx, &keys))
    } else {
        (clamp_delta(&[(chosen.to_vec(), 24)], want), keys)
    };
    move_keys(fx, &moving, delta);
    delta
}

#[test]
fn dragging_an_effect_key_carries_the_later_keys_of_its_effect_only() {
    let mut fx = stack();
    fx.insert(0, blur_2_6_12_18());
    // La clé 6 du premier flou glisse de 3 : elle et les clés d'après (12, 18) ; la 2 reste.
    assert_eq!(drag_fx(&mut fx, &[6], 3, true), 3);
    assert_eq!(frames(&fx[0]), vec![2, 9, 15, 21]);
    assert_eq!(fx[0].keys[1].values, vec![6.0], "les paramètres suivent leur clé");
    assert_eq!(frames(&fx[1]), vec![2, 6], "un autre effet ne bouge pas");
    // Le décalage s'arrête après la clé d'avant, et la dernière clé reste dans le plan.
    let mut fx = vec![blur_2_6_12_18()];
    assert_eq!(drag_fx(&mut fx, &[6], -50, true), -3);
    assert_eq!(frames(&fx[0]), vec![2, 3, 9, 15]);
    assert_eq!(drag_fx(&mut fx, &[3], 50, true), 8, "la dernière clé (15) s'arrête à l'image 23");
    assert_eq!(frames(&fx[0]), vec![2, 11, 17, 23]);
    assert_eq!(clamp_ripple(&[(vec![2, 6, 12, 18], 2, 24)], -9), -2, "la première ne passe pas l'image 0");
    assert_eq!(clamp_ripple(&[], 4), 0);
    // Plusieurs choisies : la première décide.
    let mut fx = vec![blur_2_6_12_18()];
    drag_fx(&mut fx, &[6, 18], 2, true);
    assert_eq!(frames(&fx[0]), vec![2, 8, 14, 20], "la 12, non choisie, suit aussi");
}

#[test]
fn shift_held_moves_only_the_chosen_effect_keys() {
    let mut fx = vec![blur_2_6_12_18()];
    assert_eq!(drag_fx(&mut fx, &[6], 3, false), 3);
    assert_eq!(frames(&fx[0]), vec![2, 9, 12, 18], "les clés d'après ne bougent pas");
    // Comme avant, une clé choisie qui tombe sur une autre du même effet la remplace.
    drag_fx(&mut fx, &[9], 3, false);
    assert_eq!(frames(&fx[0]), vec![2, 12, 18]);
    assert_eq!(fx[0].keys[1].values, vec![6.0], "c'est la clé déplacée qui reste");
}

#[test]
fn ctrl_v_inserts_effect_keys_after_the_key_under_the_playhead() {
    let clip = copy_keys(&blur_2_6_12_18(), &[6, 12]);
    assert_eq!(clip.keys.iter().map(|k| k.offset).collect::<Vec<_>>(), vec![0, 6]);
    // Tête sur la clé 6 : les clés vont en 7 et 13, la 12 et la 18 sont repoussées de 7 (6 + 1).
    let mut e = blur_2_6_12_18();
    let done = paste_insert_keys(&mut e, &clip, 6, 40).unwrap();
    assert_eq!(done, Inserted { pasted: Pasted { placed: vec![7, 13], skipped: 0 }, pushed: 2 });
    assert_eq!(frames(&e), vec![2, 6, 7, 13, 19, 25]);
    assert_eq!(e.keys[2].values, vec![6.0], "les paramètres copiés");
    assert_eq!(e.keys[4].values, vec![12.0], "la clé repoussée garde les siens");
    // Tête entre deux clés : la dernière clé d'avant compte (6).
    let mut e = blur_2_6_12_18();
    paste_insert_keys(&mut e, &clip, 9, 40).unwrap();
    assert_eq!(frames(&e), vec![2, 6, 7, 13, 19, 25]);
    // Aucune clé avant la tête : sur la tête, celles d'après sont repoussées.
    let mut e = blur_2_6_12_18();
    paste_insert_keys(&mut e, &clip, 0, 40).unwrap();
    assert_eq!(frames(&e), vec![0, 6, 9, 13, 19, 25]);
    // Plan trop court pour la clé repoussée : rien ne change.
    let mut e = blur_2_6_12_18();
    assert_eq!(paste_insert_keys(&mut e, &clip, 6, 24), Err(crate::camkeys::NoRoom { short: 2 }), "18 + 7 = 25 > 23");
    assert_eq!(e, blur_2_6_12_18());
    // Un effet d'une autre sorte : rien n'est inséré.
    let mut glow = Effect::new(EffectKind::Glow);
    let done = paste_insert_keys(&mut glow, &clip, 3, 40).unwrap();
    assert_eq!(done.pasted, Pasted { placed: vec![], skipped: 2 });
    assert!(glow.keys.is_empty());
}

#[test]
fn ctrl_shift_v_pastes_effect_keys_on_the_playhead_and_pushes_nothing() {
    let clip = copy_keys(&blur_2_6_12_18(), &[2, 6]);
    let mut e = blur_2_6_12_18();
    let done = paste_keys(&mut e, &clip, 12, 40);
    assert_eq!(done, Pasted { placed: vec![12, 16], skipped: 0 });
    assert_eq!(frames(&e), vec![2, 6, 12, 16, 18]);
    assert_eq!(e.keys[2].values, vec![2.0], "la clé qui était sur la tête est remplacée");
}

#[test]
fn drag_insert_and_paste_at_head_of_effect_keys_undo_in_one_step_each() {
    let mut doc = Document::new(64, 64, 24);
    doc.panels[0].layers[0].fx = vec![blur_2_6_12_18()];
    let original = doc.panels[0].layers[0].fx.clone();
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut history = History::default();
    let mut step = |doc: &mut Document, name: &str, edit: &dyn Fn(&mut Vec<Effect>)| {
        let snap = Edit::Structure { structure: doc.structure(), nav };
        edit(&mut doc.panels[0].layers[0].fx);
        let before = history.context.replace(name.to_string());
        history.push(snap);
        history.context = before;
    };
    let clip = copy_keys(&blur_2_6_12_18(), &[2]);
    step(&mut doc, "Décaler les clés d'effet", &|fx| {
        drag_fx(fx, &[6], 2, true);
    });
    step(&mut doc, "Insérer des clés d'effet", &|fx| {
        paste_insert_keys(&mut fx[0], &clip, 2, 24).unwrap();
    });
    step(&mut doc, "Coller les clés d'effet à la tête de lecture", &|fx| {
        paste_keys(&mut fx[0], &clip, 0, 24);
    });
    assert_eq!(frames(&doc.panels[0].layers[0].fx[0]), vec![0, 2, 3, 9, 15, 21]);
    assert_eq!(history.names().0, vec!["Décaler les clés d'effet", "Insérer des clés d'effet", "Coller les clés d'effet à la tête de lecture"]);
    for _ in 0..3 {
        assert!(history.undo(&mut doc, &mut nav));
    }
    assert_eq!(doc.panels[0].layers[0].fx, original);
    for _ in 0..3 {
        assert!(history.redo(&mut doc, &mut nav));
    }
    assert_eq!(frames(&doc.panels[0].layers[0].fx[0]), vec![0, 2, 3, 9, 15, 21]);
}

#[test]
fn every_edit_is_one_named_undo_step_that_restores_the_keys() {
    let mut doc = Document::new(64, 64, 24);
    doc.panels[0].layers[0].fx = stack();
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut history = History::default();
    let step = |doc: &mut Document, history: &mut History, nav: Nav, name: &str, edit: &dyn Fn(&mut Vec<Effect>)| {
        let snap = Edit::Structure { structure: doc.structure(), nav };
        edit(&mut doc.panels[0].layers[0].fx);
        let before = history.context.replace(name.to_string());
        history.push(snap);
        history.context = before;
    };
    step(&mut doc, &mut history, nav, "Déplacer des clés d'effet", &|fx| {
        move_keys(fx, &[(0, 2), (0, 6)], 2);
    });
    step(&mut doc, &mut history, nav, "Supprimer des clés d'effet", &|fx| {
        delete_keys(fx, &[(1, 6)]);
    });
    let clip = copy_keys(&stack()[0], &[2]);
    step(&mut doc, &mut history, nav, "Coller des clés d'effet", &|fx| {
        paste_keys(&mut fx[0], &clip, 20, 40);
    });
    assert_eq!(history.names().0, vec!["Déplacer des clés d'effet", "Supprimer des clés d'effet", "Coller des clés d'effet"]);
    assert_eq!(frames(&doc.panels[0].layers[0].fx[0]), vec![4, 8, 20]);
    assert!(doc.panels[0].layers[0].fx[1].keys.is_empty());
    // Annuler trois fois : la pile d'origine, clé pour clé.
    for _ in 0..3 {
        assert!(history.undo(&mut doc, &mut nav));
    }
    assert_eq!(doc.panels[0].layers[0].fx, stack());
    // Rétablir : le collage revient.
    for _ in 0..3 {
        assert!(history.redo(&mut doc, &mut nav));
    }
    assert_eq!(frames(&doc.panels[0].layers[0].fx[0]), vec![4, 8, 20]);
}
