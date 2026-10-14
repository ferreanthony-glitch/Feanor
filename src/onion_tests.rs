//! Tests de la pelure d'oignon (phase 3d-1).

use crate::document::{Document, Layer, LayerKind};
use crate::onion::{fade, neighbours, OnionMode, OnionSettings};
use crate::settings::Settings;

#[test]
fn defaults_follow_the_storyboard_pro_preferences() {
    // STB_Preferences.xml : 3 dessins avant / 3 après, rouge avant, vert après, teinté.
    let s = OnionSettings::default();
    assert_eq!((s.before, s.after), (3, 3));
    assert_eq!((s.before_color, s.after_color), ([255, 0, 0], [0, 255, 0]));
    assert!(s.enabled && s.tinted);
    assert_eq!(s.mode, OnionMode::Drawings);
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
    assert_eq!(neighbours(&l, 4, &s), (vec![(b, 0), (a, 1)], vec![(d, 0)]));
    // Par image : images 3 et 2 avant (C est le dessin courant : seul B reste, au rang 1),
    // images 5 et 6 après (D au rang 1).
    s.mode = OnionMode::Frames;
    assert_eq!(neighbours(&l, 4, &s), (vec![(b, 1)], vec![(d, 1)]));
    // Un dessin vu à plusieurs images n'est montré qu'une fois, au plus près.
    s.before = 4;
    assert_eq!(neighbours(&l, 4, &s).0, vec![(b, 1), (a, 2)]);
    // Bords de la timeline et images vides.
    l.frames[2] = None;
    assert_eq!(neighbours(&l, 0, &s).0, vec![]);
    assert_eq!(neighbours(&l, 4, &s).0, vec![(a, 2)]);
}

#[test]
fn farther_neighbours_are_paler() {
    assert_eq!(fade(0.5, 0, 3), 128);
    assert_eq!(fade(0.5, 1, 3), 85);
    assert_eq!(fade(0.5, 2, 3), 43);
    assert_eq!(fade(0.5, 5, 3), 43, "au-delà : comme le plus lointain");
    assert_eq!(fade(2.0, 0, 1), 255);
}

#[test]
fn onion_settings_are_saved_with_the_app_settings() {
    let mut set = Settings::default();
    set.onion.before = 5;
    set.onion.mode = OnionMode::Frames;
    set.onion.after_color = [10, 20, 30];
    let text = toml::to_string(&set).unwrap();
    assert!(text.contains("[pelure]") && text.contains("mode = \"images\""), "{text}");
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.onion, set.onion);
    // Fichier d'avant la 3d (sans pelure) : valeurs par défaut.
    let old: Settings = toml::from_str("ffmpeg = \"C:/ffmpeg.exe\"\n").unwrap();
    assert_eq!(old.onion, OnionSettings::default());
}
