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

fn near3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

#[test]
fn hsl_matches_the_usual_values() {
    use crate::couleur::{hsl_to_hsv, hsl_to_rgb, hsv_to_hsl, rgb_to_hsl};
    // Valeurs de référence (CSS) : teinte en degrés, saturation et luminosité de 0 à 1.
    assert!(near3(rgb_to_hsl([255, 0, 0]), [0.0, 1.0, 0.5], 1e-4));
    assert!(near3(rgb_to_hsl([255, 255, 255]), [0.0, 0.0, 1.0], 1e-4));
    assert!(near3(rgb_to_hsl([0, 0, 0]), [0.0, 0.0, 0.0], 1e-4));
    assert!(near3(rgb_to_hsl([128, 128, 128]), [0.0, 0.0, 128.0 / 255.0], 1e-4));
    assert!(near3(rgb_to_hsl([255, 128, 128]), [0.0, 1.0, 0.751], 2e-3), "rose : {:?}", rgb_to_hsl([255, 128, 128]));
    assert!(near3(rgb_to_hsl([0, 0, 128]), [240.0, 1.0, 0.251], 2e-3));
    assert_eq!(hsl_to_rgb([120.0, 1.0, 0.5]), [0, 255, 0]);
    assert_eq!(hsl_to_rgb([60.0, 1.0, 0.25]), [128, 128, 0]);
    assert_eq!(hsl_to_rgb([0.0, 0.0, 0.75]), [191, 191, 191]);
    // Aller et retour, en nombres (la teinte reste telle quelle).
    for hsv in [[0.0, 0.0, 0.0], [30.0, 0.5, 0.5], [200.0, 1.0, 1.0], [310.0, 0.2, 0.9], [90.0, 0.0, 1.0], [45.0, 1.0, 0.3]] {
        let back = hsl_to_hsv(hsv_to_hsl(hsv));
        assert!((back[2] - hsv[2]).abs() < 1e-5 && back[0] == hsv[0], "{hsv:?} → {back:?}");
        if hsv[2] > 0.0 {
            assert!((back[1] - hsv[1]).abs() < 1e-5, "{hsv:?} → {back:?}");
        }
    }
    // Toutes les couleurs d'un cube grossier reviennent à l'identique.
    for r in (0..=255).step_by(51) {
        for g in (0..=255).step_by(51) {
            for b in (0..=255).step_by(51) {
                let c = [r as u8, g as u8, b as u8];
                assert_eq!(hsl_to_rgb(rgb_to_hsl(c)), c);
            }
        }
    }
}

#[test]
fn greys_and_black_keep_the_hue_being_set() {
    use crate::couleur::hsv_following;
    // Un gris : la teinte d'avant reste.
    assert_eq!(hsv_following([128, 128, 128], [200.0, 0.7, 0.4]), [200.0, 0.0, 128.0 / 255.0]);
    // Le noir : teinte et saturation d'avant restent.
    assert_eq!(hsv_following([0, 0, 0], [200.0, 0.7, 0.4]), [200.0, 0.7, 0.0]);
    // Une vraie couleur : la sienne.
    assert_eq!(hsv_following([255, 0, 0], [200.0, 0.7, 0.4]), [0.0, 1.0, 1.0]);
    for c in [[0, 0, 0], [128, 128, 128], [255, 255, 255], [10, 200, 30]] {
        assert_eq!(couleur::hsv_to_rgb(hsv_following(c, [123.0, 0.5, 0.5])), c, "la couleur ne change pas");
    }
}

#[test]
fn recent_colors_come_first_without_duplicates() {
    use crate::couleur::{remember, RECENT_MAX};
    let mut recent = Vec::new();
    remember(&mut recent, [1, 1, 1]);
    remember(&mut recent, [2, 2, 2]);
    remember(&mut recent, [1, 1, 1]);
    assert_eq!(recent, vec![[1, 1, 1], [2, 2, 2]], "la couleur réutilisée remonte, sans doublon");
    for k in 0..100u8 {
        remember(&mut recent, [k, 0, 0]);
    }
    assert_eq!(recent.len(), RECENT_MAX);
    assert_eq!(recent[0], [99, 0, 0]);
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
    s.colors.sliders = couleur::SliderMode::Hsl;
    s.colors.wheel = crate::roue::WheelShape::Triangle;
    s.colors.recent = vec![[1, 2, 3], [4, 5, 6]];
    let text = toml::to_string(&s).unwrap();
    for key in ["[couleurs]", "avant_plan", "arriere_plan", "curseurs = \"tsl\"", "roue = \"triangle\"", "recentes"] {
        assert!(text.contains(key), "{key} manque :\n{text}");
    }
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.colors, s.colors);
}

/// Les textes entre guillemets d'une source qui ressemblent à une commande des couleurs
/// (« couleur.… », « nuancier.… », « pastille.… »).
fn color_actions_in(file: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    for (i, part) in text.split('"').enumerate() {
        let family = part.split('.').next().unwrap_or("");
        let looks = part.split('.').count() == 2 && part.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c == '.');
        if i % 2 == 1 && looks && ["couleur", "nuancier", "pastille"].contains(&family) {
            out.push(part.to_string());
        }
    }
    out
}

#[test]
fn every_color_command_is_known_mapped_and_handled() {
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    let mut cited: Vec<String> = color_actions_in("colorui.rs");
    cited.extend(color_actions_in("nuancierui.rs"));
    assert!(cited.len() > 20, "{cited:?}");
    for id in &cited {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id} n'est pas dans keymap::ACTIONS");
        assert!(map.bindings.iter().any(|b| &b.action == id), "{id} n'est pas dans keymap.toml");
    }
    // Et chaque commande des couleurs déclarée est traitée (citée dans un des deux fichiers).
    for a in crate::keymap::ACTIONS.iter().filter(|a| ["couleur.", "nuancier.", "pastille."].iter().any(|f| a.id.starts_with(f))) {
        assert!(cited.iter().any(|c| c == a.id), "{} n'est traitée nulle part", a.id);
    }
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
