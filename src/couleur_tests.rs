//! Tests de la passe couleur : code hexadécimal, réglages retenus, raccourcis des deux couleurs.

use eframe::egui::{Key, Modifiers};

use crate::app::Mode;
use crate::couleur::{self, hex, parse_hex, ColorSettings};
use crate::keymap::{Keymap, DEFAULT_KEYMAP};

#[test]
fn hex_codes_are_written_and_read() {
    assert_eq!(hex([255, 0, 128]), "#FF0080");
    assert_eq!(hex([0, 0, 0]), "#000000");
    assert_eq!(parse_hex("#FF0080"), Some([255, 0, 128]));
    assert_eq!(parse_hex("  ff0080 "), Some([255, 0, 128]), "sans #, minuscules, espaces autour");
    assert_eq!(parse_hex("#F08"), Some([255, 0, 136]), "forme courte : chaque chiffre doublé");
    assert_eq!(parse_hex("#abc"), Some([0xAA, 0xBB, 0xCC]));
    for bad in ["", "#", "#12", "#12345", "#1234567", "#GG0000", "#+12345", "12 345"] {
        assert_eq!(parse_hex(bad), None, "« {bad} »");
    }
    for c in [[0, 0, 0], [255, 255, 255], [24, 24, 28], [1, 2, 3]] {
        assert_eq!(parse_hex(&hex(c)), Some(c));
    }
}

#[test]
fn colors_are_kept_in_the_settings_file() {
    // Un ancien fichier de réglages, sans couleurs : celles d'un premier lancement.
    let old: crate::settings::Settings = toml::from_str("copie_secours_minutes = 3\n").unwrap();
    assert_eq!(old.colors, ColorSettings::default());
    assert_eq!(old.colors.front, couleur::ENCRE);
    assert_eq!(old.colors.back, couleur::BLANC);
    // Écrites puis relues, en français, dans la table [couleurs].
    let mut s = crate::settings::Settings::default();
    s.colors.front = [10, 20, 30];
    s.colors.back = [200, 100, 0];
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("[couleurs]") && text.contains("avant_plan") && text.contains("arriere_plan"), "{text}");
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.colors, s.colors);
}

#[test]
fn photoshop_keys_only_where_free() {
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    for mode in [Mode::Storyboard, Mode::Animation] {
        // D (couleurs par défaut) était libre dans les deux modes.
        assert_eq!(map.find(Key::D, Some(Key::D), Modifiers::NONE, mode), Some("couleur.defaut"));
        // X reste « coller le calque » (ton réglage perso) : intervertir n'a pas de touche.
        assert_eq!(map.find(Key::X, Some(Key::X), Modifiers::NONE, mode), Some("calque.coller"));
    }
    assert!(map.labels_of("couleur.intervertir").is_empty());
    assert!(map.source_of("couleur.defaut").starts_with("proposition"));
    assert!(map.source_of("couleur.intervertir").starts_with("proposition"));
}
