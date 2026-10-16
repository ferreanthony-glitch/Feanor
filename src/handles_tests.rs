//! Poignées de la timeline (passe interface) : ce qui se déplace, et jusqu'où.

use crate::document::{ImageMark, Layer, LayerKind};
use crate::fx::{Effect, EffectKind, FxKey};
use crate::handles::{between, move_bookmark, move_fx_key, move_mark, set_run_length};

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
