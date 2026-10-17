//! Cadence de la propagation : quelles images fabriquer, lesquelles tiennent l'image d'avant.

use crate::propager::{held_frames, made_frames, Cadence};

const EVERY: Cadence = Cadence { keys_only: false, step: 1, follow_keys: true };

#[test]
fn every_frame_by_default() {
    // Clé peinte à l'image 0 : on fabrique toutes les autres (comme avant).
    assert_eq!(made_frames(0..6, &[0], Some(&[0, 3]), Cadence::default()), vec![1, 2, 3, 4, 5]);
    assert_eq!(made_frames(0..6, &[0], None, EVERY), vec![1, 2, 3, 4, 5]);
}

#[test]
fn on_twos_from_the_painted_keys() {
    let twos = Cadence { keys_only: false, step: 2, follow_keys: false };
    // En 2 depuis la clé 0 ; la clé peinte 5 relance la cadence.
    assert_eq!(made_frames(0..9, &[0, 5], Some(&[0, 3]), twos), vec![2, 4, 7]);
    // Le début de la plage est une image même sans clé : les suivantes ont quoi tenir.
    assert_eq!(made_frames(1..6, &[3], None, twos), vec![1, 5]);
    assert_eq!(held_frames(0..9, &[0, 5], &[2, 4, 7]), vec![1, 3, 6, 8]);
}

#[test]
fn following_the_guide_keys_restarts_the_cadence() {
    // En 3, en suivant les clés du guide (0, 4) : une image à la clé 4, puis 3 plus loin.
    let threes = Cadence { keys_only: false, step: 3, follow_keys: true };
    assert_eq!(made_frames(0..10, &[0], Some(&[0, 4]), threes), vec![3, 4, 7]);
    // Sans les suivre : en 3, tout droit.
    let strict = Cadence { follow_keys: false, ..threes };
    assert_eq!(made_frames(0..10, &[0], Some(&[0, 4]), strict), vec![3, 6, 9]);
}

#[test]
fn only_on_the_guide_keys() {
    let keys_only = Cadence { keys_only: true, step: 2, follow_keys: false };
    // L'animation du guide change aux images 0, 2, 6 ; la clé peinte 0 n'est pas refaite.
    assert_eq!(made_frames(0..8, &[0], Some(&[0, 2, 6]), keys_only), vec![2, 6]);
    assert_eq!(held_frames(0..8, &[0], &[2, 6]), vec![1, 3, 4, 5, 7]);
    // Une vidéo n'a pas de clés : chaque image est fabriquée.
    assert_eq!(made_frames(0..4, &[0], None, keys_only), vec![1, 2, 3]);
}

#[test]
fn guide_keys_and_held_frames_in_a_document() {
    use crate::document::{Document, Layer, LayerKind};
    use crate::propager::{guide_keys, hold, Guide};
    // Une animation en 2 (dessins aux images 0, 2, 4) sous un calque peint.
    let mut doc = Document::with_frame(64, 36, 24);
    doc.panels[0].layers = vec![Layer::new("Animation", LayerKind::Bitmap), Layer::new("Peinture", LayerKind::Bitmap)];
    for f in [0, 2, 4] {
        let cel = doc.new_cel(LayerKind::Bitmap);
        doc.panels[0].layers[0].set(f, Some(cel));
        doc.panels[0].layers[0].set(f + 1, Some(cel));
    }
    assert_eq!(guide_keys(&doc, 0, 1, Guide::Below), Some(vec![0, 2, 4]));
    assert_eq!(guide_keys(&doc, 0, 1, Guide::Video(99)), None, "une vidéo n'a pas de clés");
    // La peinture : des dessins aux images 0 et 2 ; les images 1 et 3 tiennent l'image d'avant.
    let (a, b) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    doc.panels[0].layers[1].set(0, Some(a));
    doc.panels[0].layers[1].set(2, Some(b));
    hold(&mut doc, 0, 1, &[1, 3]);
    let paint = &doc.panels[0].layers[1];
    assert_eq!((paint.frames[1], paint.frames[3]), (Some(a), Some(b)));
}