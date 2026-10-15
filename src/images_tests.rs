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
fn deleting_images_leaves_the_chosen_hole() {
    use crate::images::{delete_frames, Hole};
    let base = vec![Some(1), Some(1), Some(2), Some(3), Some(3), Some(4)];
    let mut f = base.clone();
    delete_frames(&mut f, 2, 4, Hole::Empty);
    assert_eq!(f, [Some(1), Some(1), None, None, Some(3), Some(4)]);
    let mut f = base.clone();
    delete_frames(&mut f, 2, 4, Hole::FromLeft);
    assert_eq!(f, [Some(1), Some(1), Some(1), Some(1), Some(3), Some(4)], "le dessin d'avant est tenu");
    let mut f = base.clone();
    delete_frames(&mut f, 2, 4, Hole::FromRight);
    assert_eq!(f, [Some(1), Some(1), Some(3), Some(4)], "la suite recule");
    let mut f = base.clone();
    delete_frames(&mut f, 4, 99, Hole::Empty);
    assert_eq!(f, [Some(1), Some(1), Some(2), Some(3)], "les vides de la fin sont retirés");
}

#[test]
fn inserting_and_reversing_images() {
    use crate::images::{frames_in, insert_frames, reverse_frames};
    let mut f = vec![Some(1), Some(2), Some(3)];
    insert_frames(&mut f, 1, &[None, Some(9)]);
    assert_eq!(f, [Some(1), None, Some(9), Some(2), Some(3)]);
    insert_frames(&mut f, 10, &[None]);
    assert_eq!(f.len(), 5, "des images vides après la fin ne changent rien");
    insert_frames(&mut f, 7, &[Some(5)]);
    assert_eq!(f, [Some(1), None, Some(9), Some(2), Some(3), None, None, Some(5)]);
    reverse_frames(&mut f, 0, 3);
    assert_eq!(&f[..3], [Some(9), None, Some(1)]);
    assert_eq!(frames_in(&f, 6, 10), [None, Some(5), None, None]);
}

#[test]
fn plan_wide_changes_move_camera_keys_and_marks() {
    use crate::camera::{Camera, CameraKey};
    use crate::document::ImageMark;
    use crate::images::shift_panel;
    let mut doc = Document::new(16, 16, 24);
    let p = &mut doc.panels[0];
    let key = |frame| CameraKey::new(frame, Camera::centered(16, 16));
    p.camera = vec![key(0), key(5), key(10)];
    p.marks = vec![ImageMark { id: 1, frame: 4, ..Default::default() }, ImageMark { id: 2, frame: 8, ..Default::default() }];
    shift_panel(p, 3, 2, false);
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), [0, 7, 12]);
    assert_eq!(p.marks.iter().map(|m| m.frame).collect::<Vec<_>>(), [6, 10]);
    shift_panel(p, 6, 4, true);
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), [0, 8], "la clé de l'image 7 part avec elle");
    assert_eq!(p.marks.iter().map(|m| m.frame).collect::<Vec<_>>(), [6], "la marque de l'image 6 aussi");
}

#[test]
fn tvpaint_image_keys_are_in_place() {
    use crate::app::Mode;
    use crate::keymap::{Keymap, DEFAULT_KEYMAP};
    use eframe::egui::{Key, Modifiers};
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    let press = |key, m, mode| map.find(key, Some(key), m, mode).map(str::to_string);
    assert_eq!(press(Key::I, Modifiers::NONE, Mode::Animation).as_deref(), Some("images.ajouter"));
    assert_eq!(press(Key::I, Modifiers::SHIFT, Mode::Animation).as_deref(), Some("images.inserer"));
    assert_eq!(press(Key::F5, Modifiers::SHIFT, Mode::Animation).as_deref(), Some("images.supprimer_plan"));
    assert_eq!(press(Key::I, Modifiers::NONE, Mode::Storyboard), None, "en storyboard, rien");
}

#[test]
fn instances_are_found_broken_and_exposed() {
    use crate::images::{break_instance, change_exposures, next_instance, prev_instance};
    // 1 1 . . 2 2 2 3
    let mut f = vec![Some(1), Some(1), None, None, Some(2), Some(2), Some(2), Some(3)];
    assert_eq!(next_instance(&f, 0), Some(4), "le vide est sauté");
    assert_eq!(next_instance(&f, 5), Some(7));
    assert_eq!(next_instance(&f, 7), None);
    assert_eq!(prev_instance(&f, 6), Some(4), "au milieu : le début de l'instance");
    assert_eq!(prev_instance(&f, 4), Some(0), "au début : l'instance d'avant");
    assert_eq!(prev_instance(&f, 0), None);

    assert!(!break_instance(&mut f, 4, 9), "au début de l'instance, rien à casser");
    assert!(break_instance(&mut f, 5, 9));
    assert_eq!(&f[4..8], [Some(2), Some(9), Some(9), Some(3)]);

    // Chaque instance qui commence entre les images 4 et 7 prend une image de plus.
    let n = change_exposures(&mut f, 4, 8, true);
    assert_eq!(n, 3);
    assert_eq!(f, [Some(1), Some(1), None, None, Some(2), Some(2), Some(9), Some(9), Some(9), Some(3), Some(3)]);
    let n = change_exposures(&mut f, 0, 2, false);
    assert_eq!(n, 1);
    assert_eq!(&f[..3], [Some(1), None, None]);
}

#[test]
fn auto_settings_are_saved_and_keys_go_to_instances() {
    use crate::app::Mode;
    use crate::keymap::{Keymap, DEFAULT_KEYMAP};
    use eframe::egui::{Key, Modifiers};
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    let press = |key| map.find(key, Some(key), Modifiers::COMMAND, Mode::Animation).map(str::to_string);
    assert_eq!(press(Key::ArrowLeft).as_deref(), Some("instance.precedente"));
    assert_eq!(press(Key::ArrowRight).as_deref(), Some("instance.suivante"));

    let mut doc = Document::new(16, 16, 24);
    let l = &mut doc.panels[0].layers[0];
    l.auto_break = true;
    l.auto_create = false;
    let dir = std::env::temp_dir().join(format!("pelure-test-auto-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("A.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    let l = &back.panels[0].layers[0];
    assert!(l.auto_break && !l.auto_create);
    assert!(Layer::new("x", LayerKind::Bitmap).auto_create, "création automatique par défaut");
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
