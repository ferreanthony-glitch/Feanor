//! Images et expositions (13c) : pré / post-comportement, puis opérations sur les images.

use crate::document::{Behaviour, Document, Layer, LayerKind};
use crate::project;

/// Calque dont les images 2 à 4 montrent les dessins 10, 11, 12 (rien avant ni après).
fn layer() -> Layer {
    let mut l = Layer::new("C", LayerKind::Bitmap);
    l.frames = vec![None, None, Some(10), Some(11), Some(12)];
    l
}

fn shown(l: &Layer, range: std::ops::Range<usize>) -> Vec<Option<u64>> {
    range.map(|f| l.shown_at(f)).collect()
}

#[test]
fn nothing_before_and_after_by_default() {
    let l = layer();
    assert_eq!(shown(&l, 0..7), [None, None, Some(10), Some(11), Some(12), None, None]);
}

#[test]
fn hold_loop_and_ping_pong() {
    let mut l = layer();
    l.pre = Behaviour::Hold;
    l.post = Behaviour::Hold;
    assert_eq!(shown(&l, 0..7), [Some(10), Some(10), Some(10), Some(11), Some(12), Some(12), Some(12)]);

    l.pre = Behaviour::Loop;
    l.post = Behaviour::Loop;
    // Avant : la boucle continue à reculons (…, 11, 12 | 10, 11, 12 | 10, 11, …).
    assert_eq!(shown(&l, 0..9), [Some(11), Some(12), Some(10), Some(11), Some(12), Some(10), Some(11), Some(12), Some(10)]);

    l.pre = Behaviour::PingPong;
    l.post = Behaviour::PingPong;
    // 10 11 12 11 10 11 12… et à reculons 11 12 avant le début.
    assert_eq!(shown(&l, 0..10), [Some(12), Some(11), Some(10), Some(11), Some(12), Some(11), Some(10), Some(11), Some(12), Some(11)]);

    // Une seule image : le va-et-vient la maintient.
    let mut one = Layer::new("1", LayerKind::Bitmap);
    one.frames = vec![Some(5)];
    one.post = Behaviour::PingPong;
    assert_eq!(shown(&one, 0..3), [Some(5), Some(5), Some(5)]);
}

#[test]
fn empty_frames_inside_stay_empty_and_drawing_tools_see_the_timeline() {
    let mut l = layer();
    l.frames[3] = None;
    l.post = Behaviour::Loop;
    assert_eq!(l.shown_at(3), None, "un trou dans les images reste un trou");
    assert_eq!(l.shown_at(6), None, "même rejoué en boucle");
    assert_eq!(l.cel_at(6), None, "les outils ne dessinent pas sur une image rejouée");
}

#[test]
fn behaviours_are_saved() {
    let mut doc = Document::new(16, 16, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let l = &mut doc.panels[0].layers[0];
    l.frames = vec![Some(cel)];
    l.pre = Behaviour::PingPong;
    l.post = Behaviour::Hold;
    let dir = std::env::temp_dir().join(format!("pelure-test-comportements-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert!(text.contains("\"avant\": \"va_et_vient\"") && text.contains("\"apres\": \"maintenir\""));
    let l = &back.panels[0].layers[0];
    assert_eq!((l.pre, l.post), (Behaviour::PingPong, Behaviour::Hold));
}
