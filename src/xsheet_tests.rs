//! Feuille d'exposition (13g-1) : ce que montre chaque case.

use crate::camera::CameraKey;
use crate::document::{Behaviour, Document, ImageMark, Layer, LayerKind};
use crate::xsheet::{camera_cells, drawing_labels, layer_cells, story_rows, CamCell, Cell, XSheetSettings};

#[test]
fn a_drawing_is_written_once_then_held() {
    let mut doc = Document::new(16, 16, 24);
    let [a, b] = [0; 2].map(|_| doc.new_cel(LayerKind::Bitmap));
    let mut l = Layer::new("L", LayerKind::Bitmap);
    // A A B — B : images 0 à 4, la dernière vide.
    l.frames = vec![Some(a), Some(a), Some(b), None, Some(b)];
    l.names.insert(b, "pose".into());
    l.color_marks.insert(a, 2);
    let cells = layer_cells(&l, 7, false);
    assert_eq!(cells[0], Cell::Start { cel: a, label: "1".into(), mark: Some(2) });
    assert_eq!(cells[1], Cell::Hold);
    assert_eq!(cells[2], Cell::Start { cel: b, label: "pose".into(), mark: None });
    assert_eq!(cells[3], Cell::EmptyStart, "une croix au début des images vides");
    assert_eq!(cells[4], Cell::Start { cel: b, label: "pose".into(), mark: None }, "repris après un vide : réécrit");
    assert_eq!(cells[5], Cell::EmptyStart, "fin de l'exposition");
    assert_eq!(cells[6], Cell::Blank);
    assert_eq!(drawing_labels(&l)[&a], "1");
}

#[test]
fn behaviours_show_what_is_seen_where_nothing_is_exposed() {
    let mut doc = Document::new(16, 16, 24);
    let a = doc.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new("L", LayerKind::Bitmap);
    l.frames = vec![None, Some(a), Some(a)];
    l.pre = Behaviour::Hold;
    l.post = Behaviour::Hold;
    let cells = layer_cells(&l, 5, true);
    assert_eq!(cells[0], Cell::Behaviour { label: "1".into() });
    assert_eq!(cells[3], Cell::Behaviour { label: "1".into() });
    assert_eq!(layer_cells(&l, 5, false)[0], Cell::EmptyStart);
}

#[test]
fn camera_keys_and_moves() {
    let mut doc = Document::new(16, 16, 24);
    let cam = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(2, cam), CameraKey::new(5, cam)];
    let cells = camera_cells(&doc.panels[0], 7);
    assert_eq!(cells, vec![CamCell::None, CamCell::None, CamCell::Key, CamCell::Between, CamCell::Between, CamCell::Key, CamCell::None]);
}

#[test]
fn story_texts_start_at_their_storyboard_image() {
    let mut doc = Document::new(16, 16, 24);
    let p = &mut doc.panels[0];
    p.action = "Il entre".into();
    p.marks.push(ImageMark { frame: 30, action: "Il s'assoit".into(), dialogue: "Bonjour".into(), ..Default::default() });
    p.marks.push(ImageMark { frame: 12, notes: "Regard caméra".into(), ..Default::default() });
    let rows = story_rows(p);
    assert_eq!(rows.iter().map(|r| r.frame).collect::<Vec<_>>(), vec![0, 12, 30]);
    assert_eq!(rows[0].action, "Il entre");
    assert_eq!((rows[2].action.as_str(), rows[2].dialogue.as_str()), ("Il s'assoit", "Bonjour"));
    assert_eq!(rows[1].notes, "Regard caméra");
}

#[test]
fn separator_defaults_to_one_second() {
    let s = XSheetSettings::default();
    assert_eq!(s.separator_every(24), 24);
    assert_eq!(XSheetSettings { separator: 8, ..s }.separator_every(24), 8);
}
