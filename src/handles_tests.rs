//! Poignées de la timeline (passe interface) : ce qui se déplace, et jusqu'où.

use crate::document::{ImageMark, Layer, LayerKind};
use crate::fx::{Effect, EffectKind, FxKey};
use crate::handles::{
    between, content_bounds, key_at, key_start_taken, move_bookmark, move_boundary, move_fx_key, move_layer, move_mark, reorder_run, reorder_slot, set_layer_end,
    set_layer_start, set_run_length, sound_shift_note,
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
    // B tiré vers la droite : il passe après C quand son milieu passe celui de C.
    let l = base();
    assert_eq!(reorder_slot(&l, 3, 3), 1, "sans bouger : sa place");
    assert_eq!(reorder_slot(&l, 3, 5), 1, "le milieu de B (6) avant celui de C (7)");
    assert_eq!(reorder_slot(&l, 3, 6), 1, "milieux égaux (7) : il reste avant");
    assert_eq!(reorder_slot(&l, 3, 7), 2, "le milieu de B (8) passe celui de C (7) : après C");
    assert_eq!(reorder_slot(&l, 3, -2), 0, "tout au début : avant A");
    let mut l = base();
    assert_eq!(reorder_run(&mut l, 3, 2), Some(7));
    assert_eq!(frames(&l), vec![Some(1), Some(1), Some(1), Some(3), Some(3), Some(3), Some(3), Some(2), Some(2)], "A, C, B : chacun garde sa durée");
    assert_eq!(l.roles.keys().copied().collect::<Vec<_>>(), vec![4, 7], "les rôles suivent leurs images");
    // Devant tous les autres.
    let mut l = base();
    assert_eq!(reorder_run(&mut l, 3, 0), Some(0));
    assert_eq!(frames(&l)[..5], [Some(2), Some(2), Some(1), Some(1), Some(1)]);
    // Rien à prendre sur une plage vide.
    let mut v = Layer::new("V", LayerKind::Bitmap);
    v.frames = vec![Some(1), None, Some(2)];
    assert_eq!(reorder_run(&mut v, 1, 0), None);
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