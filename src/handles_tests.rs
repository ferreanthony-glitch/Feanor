//! Poignées de la timeline (passe interface) : ce qui se déplace, et jusqu'où.

use crate::document::{ImageMark, Layer, LayerKind};
use crate::fx::{Effect, EffectKind, FxKey};
use crate::handles::{
    between, content_bounds, drop_marks, duplicate_run, key_at, key_start_taken, move_bookmark, move_boundary, move_fx_key, move_layer, move_mark, overwrite_run,
    remove_run, set_layer_end, set_layer_start, set_run_length, shift_runs, sound_shift_note,
};

fn mark(frame: usize) -> ImageMark {
    ImageMark { frame, ..Default::default() }
}

#[test]
fn marks_stay_between_their_neighbours() {
    let mut marks = vec![mark(5), mark(10), mark(20)];
    assert_eq!(move_mark(&mut marks, 1, 15, 30), 15);
    assert_eq!(move_mark(&mut marks, 1, 25, 30), 19, "pas au-delà de la suivante");
    assert_eq!(move_mark(&mut marks, 0, 0, 30), 1, "jamais sur la première image du plan");
    assert_eq!(move_mark(&mut marks, 2, 40, 30), 29, "avant la fin du plan");
    assert_eq!(marks.iter().map(|m| m.frame).collect::<Vec<_>>(), vec![1, 19, 29]);
    assert_eq!(between(3, Some(3), Some(4)), 4, "pas de place : juste après la précédente");
}

#[test]
fn bookmarks_and_fx_keys_keep_their_order() {
    let mut b = vec![2, 8];
    assert_eq!(move_bookmark(&mut b, 0, 9, 20), 7);
    assert_eq!(move_bookmark(&mut b, 1, 30, 20), 19);
    assert_eq!(move_bookmark(&mut b, 0, 0, 20), 0);
    let mut keys = vec![FxKey { frame: 0, values: vec![1.0] }, FxKey { frame: 6, values: vec![2.0] }];
    assert_eq!(move_fx_key(&mut keys, 1, 0), 1);
    assert_eq!(move_fx_key(&mut keys, 0, 3), 0, "la première ne passe pas la seconde");
}

#[test]
fn exposure_stretches_and_what_follows_slides() {
    // A tenu 3 images, B 2 images ; un rôle et une clé d'effet sur B.
    let mut l = Layer::new("L", LayerKind::Bitmap);
    l.frames = vec![Some(1), Some(1), Some(1), Some(2), Some(2)];
    l.roles.insert(3, crate::roles::Role::Key);
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.keys.push(FxKey { frame: 4, values: vec![3.0] });
    l.fx.push(e);
    set_run_length(&mut l, 0, 3, 5);
    assert_eq!(l.frames, vec![Some(1), Some(1), Some(1), Some(1), Some(1), Some(2), Some(2)]);
    assert!(l.roles.contains_key(&5), "le rôle de B suit");
    assert_eq!(l.fx[0].keys[0].frame, 6, "la clé d'effet suit");
    set_run_length(&mut l, 0, 5, 1);
    assert_eq!(l.frames, vec![Some(1), Some(2), Some(2)]);
    assert!(l.roles.contains_key(&1));
    assert_eq!(l.fx[0].keys[0].frame, 2);
    // Le dernier dessin : on étire le calque.
    set_run_length(&mut l, 1, 3, 6);
    assert_eq!(l.frames.len(), 7);
    set_run_length(&mut l, 1, 7, 0);
    assert_eq!(l.frames, vec![Some(1), Some(2)], "au moins une image");
}

/// Un calque : vide 2 images, A 3 images, B 2 images ; un rôle sur B et une clé d'effet à
/// l'image 5.
fn layer_ab() -> Layer {
    let mut l = Layer::new("L", LayerKind::Vector);
    l.frames = vec![None, None, Some(1), Some(1), Some(1), Some(2), Some(2)];
    l.roles.insert(5, crate::roles::Role::Key);
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.keys.push(FxKey { frame: 5, values: vec![3.0] });
    l.fx.push(e);
    l
}

#[test]
fn layer_start_is_trimmed_or_held_earlier() {
    let mut l = layer_ab();
    assert_eq!(content_bounds(&l), Some((2, 7)));
    // Plus tôt : le premier dessin est tenu dès l'image 0.
    set_layer_start(&mut l, 0);
    assert_eq!(&l.frames[..3], &[Some(1), Some(1), Some(1)]);
    // Plus tard : les images d'avant se vident, la suite ne bouge pas.
    let mut l = layer_ab();
    set_layer_start(&mut l, 5);
    assert_eq!(l.frames, vec![None, None, None, None, None, Some(2), Some(2)]);
    assert!(l.roles.contains_key(&5), "le rôle de B reste");
    // Jamais au-delà de la dernière image.
    let mut l = layer_ab();
    set_layer_start(&mut l, 50);
    assert_eq!(content_bounds(&l), Some((6, 7)));
    assert!(content_bounds(&Layer::new("vide", LayerKind::Bitmap)).is_none());
}

#[test]
fn layer_end_holds_the_last_drawing_and_stops_at_the_plan_end() {
    let mut l = layer_ab();
    set_layer_end(&mut l, 9, 10);
    assert_eq!(&l.frames[7..], &[Some(2), Some(2)], "le dernier dessin est tenu");
    let mut l = layer_ab();
    set_layer_end(&mut l, 40, 10);
    assert_eq!(l.frames.len(), 10, "la poignée s'arrête à la fin du plan");
    // Plus court : coupé, rôle compris.
    let mut l = layer_ab();
    set_layer_end(&mut l, 4, 10);
    assert_eq!(l.frames, vec![None, None, Some(1), Some(1)]);
    assert!(l.roles.is_empty());
    // Jamais avant le début.
    let mut l = layer_ab();
    set_layer_end(&mut l, 0, 10);
    assert_eq!(content_bounds(&l), Some((2, 3)));
    // Un calque qui dépassait déjà la fin du plan ne s'allonge plus, mais se raccourcit.
    let mut l = layer_ab();
    set_layer_end(&mut l, 9, 5);
    assert_eq!(l.frames.len(), 7);
}

#[test]
fn a_layer_moves_with_its_roles_and_effect_keys() {
    let mut l = layer_ab();
    assert_eq!(move_layer(&mut l, 2, 12), 2);
    assert_eq!(content_bounds(&l), Some((4, 9)));
    assert!(l.roles.contains_key(&7), "le rôle suit");
    assert_eq!(l.fx[0].keys[0].frame, 7, "la clé d'effet suit");
    // Vers la gauche : pas avant la première image du plan.
    let mut l = layer_ab();
    assert_eq!(move_layer(&mut l, -5, 12), -2);
    assert_eq!(content_bounds(&l), Some((0, 5)));
    assert!(l.roles.contains_key(&3));
    // Vers la droite : pas après la fin du plan.
    let mut l = layer_ab();
    assert_eq!(move_layer(&mut l, 9, 10), 3);
    assert_eq!(content_bounds(&l), Some((5, 10)));
}

#[test]
fn a_plan_length_change_warns_only_with_sound_after_it() {
    let mut doc = crate::export_tests::doc_with_drawing(2, 10);
    assert!(sound_shift_note(&doc, 0, 10, 14).is_none(), "pas de son");
    doc.add_audio_track();
    let pcm = crate::audio::Pcm::from_interleaved(&vec![0.1f32; 2 * 48_000], 2, 48_000);
    let s = doc.add_sound("a.wav".into(), "wav".into(), std::sync::Arc::new(Vec::new()), std::sync::Arc::new(pcm));
    doc.add_clip(0, s, 12, 0, 6).unwrap();
    let note = sound_shift_note(&doc, 0, 10, 14).expect("du son après la fin du plan 1");
    assert!(note.contains("+4 images"), "{note}");
    assert!(sound_shift_note(&doc, 0, 10, 10).is_none(), "même durée");
    assert!(sound_shift_note(&doc, 1, 10, 12).is_none(), "le son finit avant la fin du plan 2");
}

fn frames(l: &Layer) -> Vec<Option<u64>> {
    l.frames.clone()
}

#[test]
fn a_key_drawing_is_laid_from_the_montage() {
    // Un dessin A tenu sur tout le plan (10 images), comme souvent en storyboard.
    let mut l = Layer::new("L", LayerKind::Vector);
    l.frames = vec![Some(1); 10];
    assert!(key_start_taken(&l, 0), "A commence à l'image 0");
    assert_eq!(key_at(&mut l, 0, 9, 10), None, "rien sur un début de dessin");
    // Une clé à l'image 4 : A jusqu'à 4, la clé B ensuite.
    assert_eq!(key_at(&mut l, 4, 2, 10), Some(10));
    assert_eq!(frames(&l), [vec![Some(1); 4], vec![Some(2); 6]].concat());
    // Une clé à l'image 7 coupe B seulement.
    assert_eq!(key_at(&mut l, 7, 3, 10), Some(10));
    assert_eq!(frames(&l)[4..], [Some(2), Some(2), Some(2), Some(3), Some(3), Some(3)]);
    // Sur des images vides : jusqu'au dessin suivant, ou jusqu'à la fin du plan.
    let mut v = Layer::new("V", LayerKind::Vector);
    v.frames = vec![None, None, None, Some(1), Some(1)];
    assert_eq!(key_at(&mut v, 1, 2, 8), Some(3));
    assert_eq!(frames(&v), vec![None, Some(2), Some(2), Some(1), Some(1)]);
    assert_eq!(key_at(&mut v, 6, 3, 8), Some(8), "après la fin du calque : jusqu'à la fin du plan");
    assert_eq!(frames(&v)[5..], [None, Some(3), Some(3)]);
}

#[test]
fn a_key_moves_before_or_after_the_others() {
    // A sur 0..3, B sur 3..5, C sur 5..9 ; un rôle au début de B et un sur C.
    let base = || {
        let mut l = Layer::new("L", LayerKind::Bitmap);
        l.frames = vec![Some(1), Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(3), Some(3)];
        l.roles.insert(3, crate::roles::Role::Key);
        l.roles.insert(6, crate::roles::Role::Key);
        l
    };
    // B (milieu 4) tiré vers la droite : il passe après C (milieu 7) quand son milieu le passe.
    let moved = |delta: isize| {
        let mut l = base();
        let s = shift_runs(&mut l, &[3], delta)[0];
        (s, frames(&l), l)
    };
    assert_eq!(moved(0).0, 3, "sans bouger : sa place");
    assert_eq!(moved(2).0, 3, "le milieu de B (6) avant celui de C (7)");
    assert_eq!(moved(3).0, 3, "milieux égaux (7) : il reste avant");
    let (s, f, l) = moved(4);
    assert_eq!(s, 7, "le milieu de B (8) passe celui de C (7) : après C");
    assert_eq!(f, vec![Some(1), Some(1), Some(1), Some(3), Some(3), Some(3), Some(3), Some(2), Some(2)], "A, C, B : chacun garde sa durée");
    assert_eq!(l.roles.keys().copied().collect::<Vec<_>>(), vec![4, 7], "les rôles suivent leurs images");
    // Devant tous les autres.
    let (s, f, _) = moved(-5);
    assert_eq!((s, &f[..5]), (0, &[Some(2), Some(2), Some(1), Some(1), Some(1)][..]));
    // Le repère, vu dans le calque tel qu'il est : là où il se posera.
    let l = base();
    assert_eq!(drop_marks(&l, &[3], 0), Vec::<usize>::new(), "il reste : pas de repère");
    assert_eq!(drop_marks(&l, &[3], 3), Vec::<usize>::new());
    assert_eq!(drop_marks(&l, &[3], 4), vec![9], "après C : la fin de C");
    assert_eq!(drop_marks(&l, &[3], -5), vec![0], "avant A : tout au début");
    // Rien à prendre sur une plage vide.
    let mut v = Layer::new("V", LayerKind::Bitmap);
    v.frames = vec![Some(1), None, Some(2)];
    assert_eq!(shift_runs(&mut v, &[1], 2), vec![1]);
    assert_eq!(frames(&v), vec![Some(1), None, Some(2)]);
}

#[test]
fn stretching_a_key_changes_its_neighbour() {
    let base = || {
        let mut l = Layer::new("L", LayerKind::Bitmap);
        l.frames = vec![Some(1), Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(3)];
        l.roles.insert(3, crate::roles::Role::Key);
        l
    };
    // La fin de A (limite A|B à 3) amenée à 4 : A s'allonge, B raccourcit ; C ne bouge pas.
    let mut l = base();
    assert_eq!(move_boundary(&mut l, 3, 4), 4);
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(1), Some(2), Some(3), Some(3), Some(3)]);
    assert_eq!(l.roles.keys().copied().collect::<Vec<_>>(), vec![4], "le rôle du début de B le suit");
    // B garde au moins une image.
    assert_eq!(move_boundary(&mut l, 4, 9), 4);
    // La fin de B (limite B|C à 5) amenée à 7 : B s'allonge sur C, qui garde une image.
    let mut l = base();
    assert_eq!(move_boundary(&mut l, 5, 7), 7);
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(2), Some(2), Some(2), Some(2), Some(3)]);
    // Vers la gauche : B commence plus tôt, A raccourcit (au moins une image).
    let mut l = base();
    assert_eq!(move_boundary(&mut l, 3, 0), 1);
    assert_eq!(frames(&l)[..5], [Some(1), Some(2), Some(2), Some(2), Some(2)]);
    // Une plage vide peut disparaître.
    let mut v = Layer::new("V", LayerKind::Bitmap);
    v.frames = vec![Some(1), None, None, Some(2)];
    assert_eq!(move_boundary(&mut v, 1, 3), 3);
    assert_eq!(frames(&v), vec![Some(1), Some(1), Some(1), Some(2)]);
}

#[test]
fn a_pasted_key_replaces_what_was_there() {
    // A tenu sur 0..8 ; la clé collée (2 images) à l'image 3 : A, la clé, puis A reprend.
    let mut l = Layer::new("L", LayerKind::Vector);
    l.frames = vec![Some(1); 8];
    l.roles.insert(4, crate::roles::Role::Key);
    overwrite_run(&mut l, 3, 2, 7);
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(7), Some(7), Some(1), Some(1), Some(1)]);
    assert!(l.roles.is_empty(), "le rôle des images remplacées part");
    // Après la fin du calque : il s'allonge.
    overwrite_run(&mut l, 9, 2, 8);
    assert_eq!(frames(&l)[8..], [None, Some(8), Some(8)]);
}

#[test]
fn a_deleted_key_is_replaced_by_its_neighbour_or_leaves_a_gap() {
    // A sur 0..3, B sur 3..5, C sur 5..8 ; un rôle au début de C.
    let base = || {
        let mut l = Layer::new("L", LayerKind::Bitmap);
        l.frames = vec![Some(1), Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(3)];
        l.roles.insert(5, crate::roles::Role::Key);
        l
    };
    // B supprimée : A tient jusqu'à C, qui ne bouge pas.
    let mut l = base();
    assert_eq!(remove_run(&mut l, 4, false), Some((3, 5)));
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(1), Some(1), Some(3), Some(3), Some(3)]);
    // A supprimée (rien avant) : B commence plus tôt, avec son rôle s'il en a.
    let mut l = base();
    l.roles.insert(3, crate::roles::Role::Key);
    remove_run(&mut l, 0, false);
    assert_eq!(frames(&l)[..5], [Some(2); 5]);
    assert_eq!(l.roles.keys().copied().collect::<Vec<_>>(), vec![0, 5]);
    // B supprimée en laissant un vide : A et C ne bougent pas.
    let mut l = base();
    remove_run(&mut l, 3, true);
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), None, None, Some(3), Some(3), Some(3)]);
    // C (la dernière) en laissant un vide : la fin vide est retirée.
    let mut l = base();
    remove_run(&mut l, 6, true);
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(2), Some(2)]);
    // Rien sur une image vide.
    assert_eq!(remove_run(&mut l, 7, false), None);
}

#[test]
fn a_key_is_duplicated_in_place() {
    // A sur 0..5, B sur 5..7 : A dupliquée, la copie (9) prend la seconde moitié de A ; B ne bouge pas.
    let mut l = Layer::new("L", LayerKind::Vector);
    l.frames = vec![Some(1), Some(1), Some(1), Some(1), Some(1), Some(2), Some(2)];
    assert_eq!(duplicate_run(&mut l, 2, 9), Some(3));
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(9), Some(9), Some(2), Some(2)]);
    // Une exposition d'une image ne se partage pas ; une image vide non plus.
    let mut one = Layer::new("U", LayerKind::Vector);
    one.frames = vec![Some(1), None];
    assert_eq!(duplicate_run(&mut one, 0, 9), None);
    assert_eq!(duplicate_run(&mut one, 1, 9), None);
}

#[test]
fn several_keys_slide_together() {
    // A 0..2, B 2..4, C 4..6, D 6..8 (2 images chacune) ; un rôle au début de B.
    let base = || {
        let mut l = Layer::new("L", LayerKind::Bitmap);
        l.frames = vec![Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(4), Some(4)];
        l.roles.insert(2, crate::roles::Role::Key);
        l
    };
    // Sans bouger : rien ne change.
    let mut l = base();
    assert_eq!(shift_runs(&mut l, &[2, 4], 0), vec![2, 4]);
    assert_eq!(frames(&l), frames(&base()));
    // B et C (à la suite) forment un bloc (milieu 4) : glissés de 3 (milieu 7, celui de D),
    // ils restent avant D ; de 4, ils le passent ensemble, dans leur ordre.
    let mut l = base();
    assert_eq!(shift_runs(&mut l, &[2, 4], 3), vec![2, 4]);
    let mut l = base();
    assert_eq!(shift_runs(&mut l, &[2, 4], 4), vec![4, 6]);
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(4), Some(4), Some(2), Some(2), Some(3), Some(3)]);
    assert_eq!(l.roles.keys().copied().collect::<Vec<_>>(), vec![4], "le rôle suit B");
    // A et C (séparées) glissées de 3 : chacune passe sa voisine (B, D).
    let mut l = base();
    assert_eq!(shift_runs(&mut l, &[0, 4], 3), vec![2, 6]);
    assert_eq!(frames(&l), vec![Some(2), Some(2), Some(1), Some(1), Some(4), Some(4), Some(3), Some(3)]);
    // Les repères : un par bloc qui change de place, vu dans le calque tel qu'il est.
    assert_eq!(drop_marks(&base(), &[2, 4], 4), vec![8], "le bloc B-C après D : la fin de D");
    assert_eq!(drop_marks(&base(), &[0, 4], 3), vec![4, 8], "A après B, C après D");
}
// ---------------------------------------------------------------------- décaler, déplacer, insérer (2026-10-19)

/// A (dessin 1) sur 0..3, B (2) sur 3..5, C (3) sur 5..9 ; un rôle au début de B et un sur C, une
/// clé d'effet à l'image 6.
fn abc() -> Layer {
    let mut l = Layer::new("L", LayerKind::Bitmap);
    l.frames = vec![Some(1), Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(3), Some(3)];
    l.roles.insert(3, crate::roles::Role::Key);
    l.roles.insert(6, crate::roles::Role::Key);
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.keys.push(FxKey { frame: 6, values: vec![1.0] });
    l.fx.push(e);
    l
}

fn roles(l: &Layer) -> Vec<usize> {
    l.roles.keys().copied().collect()
}

#[test]
fn dragging_a_drawing_key_carries_every_later_key() {
    use crate::handles::{clamp_ripple_delta, ripple_at, ripple_bounds};
    let plan = 12;
    assert_eq!(ripple_bounds(&abc(), 3, plan), (-2, 3), "A garde une image ; le calque finit au plus à la fin du plan");
    assert_eq!(clamp_ripple_delta(&abc(), 3, -9, plan), -2);
    assert_eq!(clamp_ripple_delta(&abc(), 3, 9, plan), 3);
    // B tirée de 2 images vers la droite : A s'allonge, B et C glissent avec leur durée, leurs rôles
    // et la clé d'effet de C.
    let mut l = abc();
    ripple_at(&mut l, 3, 2, plan);
    assert_eq!(frames(&l), [vec![Some(1); 5], vec![Some(2); 2], vec![Some(3); 4]].concat());
    assert_eq!(roles(&l), vec![5, 8]);
    assert_eq!(l.fx[0].keys[0].frame, 8);
    // Vers la gauche : A raccourcit, la suite recule.
    let mut l = abc();
    ripple_at(&mut l, 3, -2, plan);
    assert_eq!(frames(&l), [vec![Some(1)], vec![Some(2); 2], vec![Some(3); 4]].concat());
    assert_eq!(roles(&l), vec![1, 4]);
    assert_eq!(l.fx[0].keys[0].frame, 4);
    // Les clés d'avant ne bougent pas : C tirée de 1 image, A et B restent telles quelles.
    let mut l = abc();
    ripple_at(&mut l, 5, 1, plan);
    assert_eq!(frames(&l), [vec![Some(1); 3], vec![Some(2); 3], vec![Some(3); 4]].concat());
    assert_eq!(roles(&l), vec![3, 7], "le rôle de B reste, celui de C suit");
}

#[test]
fn the_plan_never_grows_and_the_last_exposure_absorbs_when_it_was_held_to_the_end() {
    use crate::handles::{ripple_at, ripple_bounds};
    // Le calque tient jusqu'à la fin du plan (9 images) : C, tenue jusque-là, absorbe le décalage.
    assert_eq!(ripple_bounds(&abc(), 3, 9), (-2, 3), "C garde au moins une image");
    let mut l = abc();
    ripple_at(&mut l, 3, 3, 9);
    assert_eq!(frames(&l), [vec![Some(1); 6], vec![Some(2); 2], vec![Some(3)]].concat(), "la queue de C est coupée, le plan reste à 9 images");
    // Vers la gauche, C reste tenue jusqu'à la fin du plan.
    let mut l = abc();
    ripple_at(&mut l, 3, -2, 9);
    assert_eq!(frames(&l), [vec![Some(1)], vec![Some(2); 2], vec![Some(3); 6]].concat());
    // Un calque qui finit avant la fin du plan garde sa fin décalée, sans la dépasser.
    assert_eq!(ripple_bounds(&abc(), 3, 11), (-2, 2));
    // La fin de la dernière exposition n'a rien après elle : elle ne peut que raccourcir (C garde une image).
    assert_eq!(ripple_bounds(&abc(), 9, 9), (-3, 0));
}

#[test]
fn shift_held_moves_only_the_boundary_of_the_chosen_keys() {
    use crate::handles::{clamp_key_delta, move_keys_only, single_bounds, KeyMove};
    assert_eq!(single_bounds(&abc(), &[3]), (-2, 1), "A garde une image, B aussi");
    // B avance d'une image : A s'allonge, B raccourcit, C ne bouge pas ; le rôle suit B.
    let mut l = abc();
    assert_eq!(move_keys_only(&mut l, &[3], 1), vec![4]);
    assert_eq!(frames(&l), [vec![Some(1); 4], vec![Some(2)], vec![Some(3); 4]].concat());
    assert_eq!(roles(&l), vec![4, 6]);
    // B recule de 2 : A raccourcit, B s'allonge.
    let mut l = abc();
    move_keys_only(&mut l, &[3], -2);
    assert_eq!(frames(&l), [vec![Some(1)], vec![Some(2); 4], vec![Some(3); 4]].concat());
    // Elle ne passe pas par-dessus ses voisines.
    assert_eq!(clamp_key_delta(&abc(), &[3], 5, KeyMove::Single, 12), 1);
    assert_eq!(clamp_key_delta(&abc(), &[3], -5, KeyMove::Single, 12), -2);
    // B et C ensemble : elles glissent du même nombre d'images, la suite (la fin du calque) reste.
    let mut l = abc();
    assert_eq!(move_keys_only(&mut l, &[3, 5], 1), vec![4, 6]);
    assert_eq!(frames(&l), [vec![Some(1); 4], vec![Some(2); 2], vec![Some(3); 3]].concat());
    assert_eq!(clamp_key_delta(&abc(), &[3, 5], 10, KeyMove::Single, 12), 3, "C garde une image");
    assert_eq!(clamp_key_delta(&abc(), &[3, 5], -10, KeyMove::Single, 12), -2, "A garde une image");
    // Une clé à l'image 0 qui avance laisse des images vides devant elle.
    let mut d = Layer::new("D", LayerKind::Bitmap);
    d.frames = vec![Some(1), Some(1), Some(1), Some(2), Some(2)];
    assert_eq!(move_keys_only(&mut d, &[0], 2), vec![2]);
    assert_eq!(frames(&d), vec![None, None, Some(1), Some(2), Some(2)]);
    assert_eq!(clamp_key_delta(&d, &[0], -4, KeyMove::Single, 8), 0, "pas avant l'image 0");
}

#[test]
fn the_drag_modes_and_their_preview() {
    use crate::handles::{apply_key_move, clamp_key_delta, key_preview, KeyMove};
    // Par défaut, les clés d'après suivent ; le décalage tient compte de la fin du plan.
    assert_eq!(clamp_key_delta(&abc(), &[3, 5], 4, KeyMove::Ripple, 12), 3);
    assert_eq!(clamp_key_delta(&abc(), &[5], -4, KeyMove::Ripple, 12), -1, "B garde une image");
    assert_eq!(clamp_key_delta(&abc(), &[3], 7, KeyMove::Reorder, 12), 7, "réordonner : l'ancien geste, sans borne");
    let mut l = abc();
    assert_eq!(apply_key_move(&mut l, &[3], 2, KeyMove::Ripple, 12), vec![5]);
    assert_eq!(frames(&l).len(), 11);
    let mut l = abc();
    assert_eq!(apply_key_move(&mut l, &[3], 4, KeyMove::Reorder, 12), vec![7], "réordonner : B passe après C");
    // L'aperçu (rien ne bouge avant le lâcher) : les expositions déplacées et où se pose la clé.
    let (ghosts, marks) = key_preview(&abc(), &[3], 2, KeyMove::Ripple, 12);
    assert_eq!((ghosts, marks), (vec![(5, 7), (7, 11)], vec![5]), "B et C, glissées de 2");
    let (ghosts, marks) = key_preview(&abc(), &[3], 1, KeyMove::Single, 12);
    assert_eq!((ghosts, marks), (vec![(4, 5)], vec![4]), "B seule : sa nouvelle exposition");
    assert_eq!(key_preview(&abc(), &[3], 0, KeyMove::Ripple, 12), (vec![], vec![]), "sans bouger : rien");
}

#[test]
fn pasted_drawing_keys_are_inserted_after_the_key_under_the_playhead() {
    use crate::handles::runs;
    use crate::montagekeys::{gap_short, insert_point, open_gap};
    // Après la clé sous la tête de lecture.
    assert_eq!(insert_point(&abc(), 3), 5, "la tête sur B : la fin de B");
    assert_eq!(insert_point(&abc(), 4), 5);
    assert_eq!(insert_point(&abc(), 0), 3);
    assert_eq!(insert_point(&abc(), 8), 9);
    let mut v = Layer::new("V", LayerKind::Bitmap);
    v.frames = vec![Some(1), Some(1), None, None, Some(2)];
    assert_eq!(insert_point(&v, 3), 2, "sur un vide : la fin du dernier dessin d'avant");
    assert_eq!(insert_point(&v, 20), 5, "après la fin du calque");
    let mut w = Layer::new("W", LayerKind::Bitmap);
    w.frames = vec![None, None, Some(1)];
    assert_eq!(insert_point(&w, 1), 1, "aucun dessin avant : la tête elle-même");
    // La place : le plan ne s'allonge pas, la queue de la dernière exposition peut être coupée.
    assert_eq!(gap_short(&abc(), 5, 3, 12), 0);
    assert_eq!(gap_short(&abc(), 5, 7, 12), 1, "le début de C sortirait du plan de 12 images");
    assert_eq!(gap_short(&abc(), 5, 3, 10), 0, "C perd sa queue, pas son début");
    assert_eq!(gap_short(&abc(), 5, 5, 10), 1);
    assert_eq!(gap_short(&abc(), 9, 50, 12), 0, "rien après : rien à repousser");
    // Insérer une clé de 2 images après B : C est repoussée de 2, ses rôles et sa clé d'effet aussi.
    let mut l = abc();
    open_gap(&mut l, 5, 2, 12);
    overwrite_run(&mut l, 5, 2, 9);
    assert_eq!(frames(&l), [vec![Some(1); 3], vec![Some(2); 2], vec![Some(9); 2], vec![Some(3); 4]].concat());
    assert_eq!(roles(&l), vec![3, 8]);
    assert_eq!(l.fx[0].keys[0].frame, 8);
    assert_eq!(runs(&l).len(), 4);
    // Plan de 10 images : la queue de C est coupée, le plan reste à 10.
    let mut l = abc();
    open_gap(&mut l, 5, 3, 10);
    assert_eq!(l.frames.len(), 10);
    assert_eq!(&l.frames[5..], &[None, None, None, Some(3), Some(3)]);
    // Maj+Ctrl+V : sur la tête de lecture, par-dessus ce qui y était, sans rien repousser.
    let mut l = abc();
    overwrite_run(&mut l, 3, 2, 9);
    assert_eq!(frames(&l), [vec![Some(1); 3], vec![Some(9); 2], vec![Some(3); 4]].concat());
}

#[test]
fn drawing_key_edits_are_named_undo_steps() {
    use crate::handles::{ripple_at, move_keys_only};
    use crate::history::{Edit, History, Nav};
    use crate::montagekeys::{key_move_name, open_gap};
    let mut doc = crate::document::Document::new(64, 64, 24);
    doc.panels[0].set_length(12);
    doc.panels[0].layers[0].frames = abc().frames;
    let original = doc.panels[0].layers[0].frames.clone();
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut history = History::default();
    let mut step = |doc: &mut crate::document::Document, name: &str, edit: &dyn Fn(&mut Layer)| {
        let snap = Edit::Structure { structure: doc.structure(), nav };
        edit(&mut doc.panels[0].layers[0]);
        let before = history.context.replace(name.to_string());
        history.push(snap);
        history.context = before;
    };
    use crate::handles::KeyMove;
    step(&mut doc, key_move_name(KeyMove::Ripple, false), &|l| ripple_at(l, 3, 2, 12));
    step(&mut doc, key_move_name(KeyMove::Single, false), &|l| {
        move_keys_only(l, &[5], 1);
    });
    step(&mut doc, "Insérer des clés", &|l| {
        open_gap(l, 8, 1, 12);
        overwrite_run(l, 8, 1, 7);
    });
    assert_eq!(history.names().0, vec!["Décaler les clés", "Déplacer une clé", "Insérer des clés"]);
    assert_eq!(doc.panels[0].layers[0].frames.len(), 12);
    for _ in 0..3 {
        assert!(history.undo(&mut doc, &mut nav));
    }
    assert_eq!(doc.panels[0].layers[0].frames, original);
    for _ in 0..3 {
        assert!(history.redo(&mut doc, &mut nav));
    }
    assert_eq!(doc.panels[0].layers[0].frames.len(), 12);
    assert_eq!(key_move_name(KeyMove::Single, true), "Déplacer des clés");
    assert_eq!(key_move_name(KeyMove::Reorder, false), "Réordonner les clés");
}

#[test]
fn a_new_key_starts_from_a_copy_of_the_drawing() {
    use crate::handles::key_source;
    // Anthony : « quand une clé est créée avec F5 sur un calque déjà dessiné, la clé ne doit pas
    // être vide » : le dessin montré à l'image, sinon le dernier d'avant, sinon le premier d'après.
    let mut l = Layer::new("Perso", LayerKind::Vector);
    l.frames = vec![None, None, Some(1), Some(1), Some(1), None, None, Some(2), Some(2)];
    assert_eq!(key_source(&l, 3), Some(1), "au milieu d'une exposition : son dessin");
    assert_eq!(key_source(&l, 5), Some(1), "dans un trou : le dessin d'avant");
    assert_eq!(key_source(&l, 12), Some(2), "après la fin du calque : le dernier dessin");
    assert_eq!(key_source(&l, 0), Some(1), "avant le premier dessin : celui-ci");
    assert_eq!(key_source(&Layer::new("Vide", LayerKind::Vector), 4), None, "calque vide : clé vide");
}