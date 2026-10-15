//! Repères du plan (13d) : lecture entre l'entrée et la sortie, numéros (image, time code,
//! trame), signets, et leur enregistrement.

use crate::cues::{frame_label, parse_frame, play_range, toggle_bookmark, TimeDisplay};
use crate::document::{Cues, Document};
use crate::project;

#[test]
fn playback_stays_between_in_and_out() {
    let mut c = Cues::default();
    assert_eq!(play_range(&c, 48), (0, 48));
    c.in_point = Some(10);
    assert_eq!(play_range(&c, 48), (10, 48));
    c.out_point = Some(20);
    assert_eq!(play_range(&c, 48), (10, 21), "la sortie est lue");
    c.out_point = Some(5);
    assert_eq!(play_range(&c, 48), (0, 48), "points croisés : tout le plan");
    c.in_point = Some(100);
    c.out_point = None;
    assert_eq!(play_range(&c, 48), (47, 48), "entrée au-delà de la fin : la dernière image");
}

#[test]
fn frame_numbers_time_code_and_fields() {
    assert_eq!(frame_label(0, 1, 24, TimeDisplay::Image), "1");
    assert_eq!(frame_label(0, 101, 24, TimeDisplay::Image), "101", "image de départ");
    assert_eq!(frame_label(0, 1, 24, TimeDisplay::TimeCode), "00:00:00:00");
    assert_eq!(frame_label(25, 1, 24, TimeDisplay::TimeCode), "00:00:01:01");
    assert_eq!(frame_label(24 * 3661, 1, 24, TimeDisplay::TimeCode), "01:01:01:00");
    assert_eq!(frame_label(0, 1, 24, TimeDisplay::Field), "1");
    assert_eq!(frame_label(2, 1, 24, TimeDisplay::Field), "5", "deux trames par image");
    for how in TimeDisplay::ALL {
        for f in [0, 7, 30, 500] {
            let label = frame_label(f, 1, 24, how);
            assert_eq!(parse_frame(&label, 1, 24, how), Some(f), "{label} ({how:?})");
        }
    }
    assert_eq!(parse_frame("1:00", 1, 24, TimeDisplay::Image), Some(24), "un time code tapé marche partout");
    assert_eq!(parse_frame("0", 1, 24, TimeDisplay::Image), None, "avant l'image de départ");
}

#[test]
fn bookmarks_toggle_and_follow_their_images() {
    let mut c = Cues::default();
    assert!(toggle_bookmark(&mut c, 12));
    assert!(toggle_bookmark(&mut c, 3));
    assert_eq!(c.bookmarks, [3, 12], "triés");
    assert!(!toggle_bookmark(&mut c, 12));
    assert_eq!(c.bookmarks, [3]);

    let mut doc = Document::new(16, 16, 24);
    let p = &mut doc.panels[0];
    p.cues = Cues { bookmarks: vec![2, 8, 15], in_point: Some(5), out_point: Some(20) };
    crate::images::shift_panel(p, 6, 3, false);
    assert_eq!(p.cues.bookmarks, [2, 11, 18]);
    assert_eq!((p.cues.in_point, p.cues.out_point), (Some(5), Some(23)));
    crate::images::shift_panel(p, 10, 4, true);
    assert_eq!(p.cues.bookmarks, [2, 14], "le signet de l'image 11 part avec elle");
    assert_eq!(p.cues.out_point, Some(19));
}

#[test]
fn cues_and_start_frame_are_saved() {
    let mut doc = Document::new(16, 16, 24);
    doc.start_frame = 101;
    doc.panels[0].cues = Cues { bookmarks: vec![4, 9], in_point: Some(2), out_point: Some(30) };
    let dir = std::env::temp_dir().join(format!("pelure-test-reperes-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("R.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.start_frame, 101);
    assert_eq!(back.panels[0].cues, doc.panels[0].cues);
}

#[test]
fn tvpaint_bookmark_keys_are_in_place() {
    use crate::app::Mode;
    use crate::keymap::{Keymap, DEFAULT_KEYMAP};
    use eframe::egui::{Key, Modifiers};
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    let m = Modifiers::COMMAND | Modifiers::SHIFT;
    let press = |key| map.find(key, Some(key), m, Mode::Animation).map(str::to_string);
    assert_eq!(press(Key::ArrowUp).as_deref(), Some("signet.basculer"));
    assert_eq!(press(Key::ArrowLeft).as_deref(), Some("signet.precedent"));
    assert_eq!(press(Key::ArrowRight).as_deref(), Some("signet.suivant"));
}
