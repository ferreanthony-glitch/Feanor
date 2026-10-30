//! Tests de la pelure d'oignon (phase 3d-1) et de son panneau complet (13f).

use crate::document::{Document, Layer, LayerKind};
use crate::onion::{neighbours, OnionMode, OnionSettings};
use crate::settings::Settings;

#[test]
fn defaults_are_three_red_before_and_three_green_after() {
    // 3 dessins avant / 3 après, rouge avant, vert après, teinté.
    let s = OnionSettings::default();
    assert_eq!((s.before, s.after), (3, 3));
    assert_eq!((s.before_color, s.after_color), ([255, 0, 0], [0, 255, 0]));
    assert!(s.enabled && s.tint_before && s.tint_after);
    assert_eq!(s.mode, OnionMode::Drawings);
    assert!(s.linked && !s.front);
}

#[test]
fn drawings_or_frames() {
    let mut doc = Document::new(64, 64, 24);
    let [a, b, c, d] = [0; 4].map(|_| doc.new_cel(LayerKind::Bitmap));
    let mut l = Layer::new("L", LayerKind::Bitmap);
    // Images : A A B C C C D (le dessin courant, à l'image 4, est C).
    l.frames = vec![Some(a), Some(a), Some(b), Some(c), Some(c), Some(c), Some(d)];
    let mut s = OnionSettings { before: 2, after: 2, ..Default::default() };
    // Par dessin : les 2 dessins différents d'avant (B puis A), le seul d'après (D).
    assert_eq!(neighbours(&l, 4, &s, &[]), (vec![(b, 0), (a, 1)], vec![(d, 0)]));
    // Par image : images 3 et 2 avant (C est le dessin courant : seul B reste, au rang 1),
    // images 5 et 6 après (D au rang 1).
    s.mode = OnionMode::Frames;
    assert_eq!(neighbours(&l, 4, &s, &[]), (vec![(b, 1)], vec![(d, 1)]));
    // Un dessin vu à plusieurs images n'est montré qu'une fois, au plus près.
    s.before = 4;
    assert_eq!(neighbours(&l, 4, &s, &[]).0, vec![(b, 1), (a, 2)]);
    // Bords de la timeline et images vides.
    l.frames[2] = None;
    assert_eq!(neighbours(&l, 0, &s, &[]).0, vec![]);
    assert_eq!(neighbours(&l, 4, &s, &[]).0, vec![(a, 2)]);
}

#[test]
fn bookmark_and_mark_modes_only_show_those_images() {
    let mut doc = Document::new(64, 64, 24);
    let [a, b, c, d, e] = [0; 5].map(|_| doc.new_cel(LayerKind::Bitmap));
    let mut l = Layer::new("L", LayerKind::Bitmap);
    // Images 0 à 6 : A B C D D E E ; l'image courante est 3 (D).
    l.frames = vec![Some(a), Some(b), Some(c), Some(d), Some(d), Some(e), Some(e)];
    let mut s = OnionSettings { before: 2, after: 2, mode: OnionMode::Bookmarks, ..Default::default() };
    // Signets aux images 0, 2, 4 et 6 : avant C puis A ; après, 4 est encore D (courant), puis E.
    assert_eq!(neighbours(&l, 3, &s, &[0, 2, 4, 6]), (vec![(c, 0), (a, 1)], vec![(e, 0)]));
    assert_eq!(neighbours(&l, 3, &s, &[]), (vec![], vec![]));
    // Marques de couleur sur B et E.
    s.mode = OnionMode::Marks;
    l.color_marks.insert(b, 1);
    l.color_marks.insert(e, 4);
    assert_eq!(neighbours(&l, 3, &s, &[]), (vec![(b, 0)], vec![(e, 0)]));
}

#[test]
fn opacity_per_image_global_and_linked_sliders() {
    let mut s = OnionSettings { before: 3, opacity: 0.5, ..Default::default() };
    // Liés (par défaut) : en ligne droite, comme avant la 13f.
    assert_eq!((s.alpha(false, 0), s.alpha(false, 1), s.alpha(false, 2)), (128, 85, 43));
    assert_eq!(s.alpha(false, 5), 43, "au-delà : comme le plus lointain");
    // Tirer le curseur de la 2e image à 0,5 : la ligne passe par 0,5 au rang 1 (1 × 2/3 → 0,75).
    s.set_rank_opacity(false, 1, 0.5);
    assert!((s.rank_opacity(false, 0) - 0.75).abs() < 1e-5);
    assert!((s.rank_opacity(false, 1) - 0.5).abs() < 1e-5);
    // Délier garde ce qu'on voyait ; ensuite chaque curseur va seul.
    s.set_linked(false);
    assert!((s.rank_opacity(false, 2) - 0.25).abs() < 1e-5);
    s.set_rank_opacity(false, 2, 0.9);
    assert!((s.rank_opacity(false, 0) - 0.75).abs() < 1e-5);
    assert!((s.rank_opacity(false, 2) - 0.9).abs() < 1e-5);
    // Opacité globale × la sienne.
    s.opacity = 1.0;
    assert_eq!(s.alpha(false, 2), 230);
}

#[test]
fn gradient_goes_from_the_nearest_colour_to_the_farthest() {
    let mut s = OnionSettings { before: 3, before_color: [255, 0, 0], before_color2: [255, 200, 0], ..Default::default() };
    assert_eq!(s.tint_of(false, 2), Some([255, 0, 0]), "sans dégradé : une seule couleur");
    s.gradient_before = true;
    assert_eq!(s.tint_of(false, 0), Some([255, 0, 0]));
    assert_eq!(s.tint_of(false, 1), Some([255, 100, 0]));
    assert_eq!(s.tint_of(false, 2), Some([255, 200, 0]));
    s.before = 1;
    assert_eq!(s.tint_of(false, 0), Some([255, 0, 0]), "une seule image : la première couleur");
    s.tint_before = false;
    assert_eq!(s.tint_of(false, 0), None, "vraies couleurs");
    assert_eq!(s.tint_of(true, 0), Some([0, 255, 0]));
}

#[test]
fn each_image_can_be_hidden_and_all_shown_again() {
    let mut s = OnionSettings::default();
    s.show_after[1] = false;
    assert!(s.shown(true, 0) && !s.shown(true, 1) && s.shown(false, 1));
    s.show_all(false);
    assert!(!s.shown(false, 0) && !s.shown(true, 2));
    s.show_all(true);
    assert!(s.shown(true, 1));
}

#[test]
fn settings_are_stored_by_name_and_recalled() {
    let mut s = OnionSettings { before: 5, front: true, ..Default::default() };
    s.store("Ma pelure");
    s.store("Ma pelure");
    assert_eq!(s.bin.len(), 1, "un nom déjà rangé est remplacé");
    assert!(s.bin[0].settings.bin.is_empty());
    s.before = 1;
    s.front = false;
    s.enabled = false;
    assert!(s.recall("Ma pelure"));
    assert_eq!((s.before, s.front, s.enabled), (5, true, false), "la pelure reste éteinte");
    assert_eq!(s.bin.len(), 1, "le rangement reste");
    assert!(!s.recall("Inconnue"));
}

#[test]
fn onion_settings_are_saved_with_the_app_settings() {
    let mut set = Settings::default();
    set.onion.before = 5;
    set.onion.mode = OnionMode::Frames;
    set.onion.after_color = [10, 20, 30];
    set.onion.show_before[2] = false;
    set.onion.store("Rangée");
    let text = toml::to_string(&set).unwrap();
    assert!(text.contains("[pelure]") && text.contains("mode = \"images\""), "{text}");
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.onion, set.onion);
    // Fichier d'avant la 3d (sans pelure) : valeurs par défaut.
    let old: Settings = toml::from_str("ffmpeg = \"C:/ffmpeg.exe\"\n").unwrap();
    assert_eq!(old.onion, OnionSettings::default());
    // Fichier d'avant la 13f : une seule colorisation, reprise des deux côtés.
    let mut old: Settings = toml::from_str("[pelure]\ntinted = false\nbefore = 2\n").unwrap();
    old.onion.migrate();
    assert!(!old.onion.tint_before && !old.onion.tint_after);
    assert_eq!(old.onion.before, 2);
    assert_eq!(old.onion.show_after.len(), crate::onion::MAX_NEIGHBOURS);
}
