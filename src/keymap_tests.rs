use std::collections::BTreeSet;

use eframe::egui::{Key, Modifiers};

use crate::app::Mode;
use crate::keymap::{parse_combo, KeyMatch, Keymap, ACTIONS, DEFAULT_KEYMAP};

/// Les 65 raccourcis par défaut avec touche de sb_shortcuts.xml (Storyboard Pro 27).
const SBP27_IDS: [&str; 65] = [
    "NEW_KEY", "SAVE_ALL_KEY", "OPEN_KEY", "QUIT_KEY", "CLOSE_KEY",
    "UNDO_KEY", "REDO_KEY", "CUT_KEY", "COPY_KEY", "PASTE_KEY", "PASTE_IN_PLACE_KEY", "DELETE_KEY",
    "DELETE_ALT_KEY", "SELECT_ALL_KEY", "SELECT_ALL_DRAWING_IN_SHOT", "DESELECT_ALL_KEY",
    "DRAWING_ZOOM_TOOL_KEY", "ZOOM_OUT_KEY", "ZOOM_IN_KEY", "TOGGLE_FULLSCREEN_KEY", "ROTATE_CW_KEY",
    "ROTATE_CCW_KEY", "DRAWING_RESET_ROTATION_KEY", "RESET_ZOOM_KEY", "FREE_PAN_AND_ZOOM_KEY",
    "OVERRIDE_ROTATE", "OVERRIDE_ZOOM_IN", "OVERRIDE_ZOOM_OUT", "DRAWING_SHOW_GRID_KEY",
    "DRAWING_OS_TOGGLE_KEY", "LOCK_LAYER_KEY", "MERGE_DOWN_LAYERS", "RESET_LAYER_TRANSFORM_KEY",
    "LINK_CLIP_AND_PANEL", "DRAWING_UNLOAD_MODEL_KEY", "FIND_TEXT_KEY",
    "DRAWING_DELETE_NON_SELECTION_KEY", "DRAWING_DELETE_NON_SELECTION_BACKSPACE_KEY",
    "DRAWING_PAINT_TOOL_KEY", "DRAWING_PAINT_UNPAINTED_TOOL_KEY", "DRAWING_COLOR_DROPPER_TOOL_KEY",
    "DRAWING_ERASER_TOOL_KEY", "DRAWING_CUTTER_TOOL_KEY", "DRAWING_BRUSH_TOOL_KEY",
    "DRAWING_STENCIL_BRUSH_TOOL_KEY", "DRAWING_SELECT_TOOL_KEY", "RESIZE_PEN_STYLE_TOOL_KEY",
    "DRAWING_FLATTEN", "DRAWING_FLIP_HORIZONTAL_KEY", "DRAWING_FLIP_VERTICAL_KEY",
    "DRAWING_SNAP_TO_CONTOUR", "DRAWING_AUTO_FILL_INSIDE_REGIONS", "DRAWING_INCREMENT_BRUSH_SIZE",
    "DRAWING_DECREMENT_BRUSH_SIZE", "GOTO_SHOT_KEY", "PREV_PANEL_KEY", "FIRST_FRAME_KEY",
    "PREV_FRAME_KEY", "NEXT_FRAME_KEY", "LAST_FRAME_KEY", "NEXT_PANEL_KEY", "NEW_PANEL_KEY",
    "ADD_FRAME_KEY", "REMOVE_FRAME_KEY", "SB_PROPERTIES_KEY",
];

/// Tes 8 réglages personnels (STB_Shortcuts.xml).
const PERSO: [(&str, &str, &[&str]); 8] = [
    ("TOGGLE_PLAYBACK_ALL_KEY", "lecture.basculer", &["Entrée"]),
    // F5 dans son Storyboard Pro ; F6 dans Pelure depuis le 2026-10-17 (F5 pose une clé).
    ("SPLIT_PANEL_KEY", "panneau.couper", &["F6"]),
    ("ADD_VECTOR_LAYER_KEY", "calque.nouveau_vectoriel", &["Maj+S"]),
    ("COPY_LAYERS_KEY", "calque.copier", &["W"]),
    ("PASTE_LAYERS_KEY", "calque.coller", &["X"]),
    ("DELETE_LAYER_KEY", "calque.supprimer", &["Q"]),
    ("BLUR_BITMAP_LAYER_KEY", "calque.flou", &["Maj+D"]),
    ("DRAWING_STENCIL_BRUSH_TOOL_KEY", "outil.pochoir", &[]),
];

fn default_map() -> Keymap {
    Keymap::from_texts(DEFAULT_KEYMAP, None)
}

/// Touche « logique » (le caractère tapé) et « physique » (la position), comme egui les donne.
fn press(map: &Keymap, key: Key, physical: Key, m: Modifiers, mode: Mode) -> Option<String> {
    map.find(key, Some(physical), m, mode).map(str::to_string)
}

const NONE: Modifiers = Modifiers::NONE;

#[test]
fn default_file_is_clean() {
    let map = default_map();
    assert!(map.warnings.is_empty(), "remarques : {:#?}", map.warnings);
    let ids: BTreeSet<&str> = ACTIONS.iter().map(|a| a.id).collect();
    assert_eq!(ids.len(), ACTIONS.len(), "identifiants d'actions en double");
}

#[test]
fn storyboard_pro_sources_are_exact() {
    // Chaque raccourci qui se dit « Storyboard Pro 27 » cite un identifiant du fichier d'origine,
    // et les 65 y sont tous.
    let map = default_map();
    let mut cited = BTreeSet::new();
    for b in &map.bindings {
        if let Some(rest) = b.source.strip_prefix("Storyboard Pro 27 : ") {
            for id in rest.split(|c: char| c == ',' || c == ' ' || c == '(').filter(|s| !s.is_empty()) {
                if id.chars().all(|c| c.is_ascii_uppercase() || c == '_') && id.len() > 3 {
                    assert!(SBP27_IDS.contains(&id), "{id} n'est pas un raccourci par défaut de Storyboard Pro 27");
                    cited.insert(id);
                }
            }
        }
    }
    // Le pinceau pochoir est cité via ton réglage perso (raccourci retiré).
    cited.insert("DRAWING_STENCIL_BRUSH_TOOL_KEY");
    let missing: Vec<_> = SBP27_IDS.iter().filter(|id| !cited.contains(*id)).collect();
    assert!(missing.is_empty(), "raccourcis Storyboard Pro 27 oubliés : {missing:?}");
}

#[test]
fn personal_shortcuts_are_kept_as_is() {
    let map = default_map();
    for (sbp_id, action, keys) in PERSO {
        let b = map
            .bindings
            .iter()
            .find(|b| b.action == action && b.source.starts_with("perso"))
            .unwrap_or_else(|| panic!("{action} absent"));
        assert!(b.source.contains(sbp_id), "{action} : source {}", b.source);
        assert_eq!(b.keys, keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), "{action}");
    }
}

#[test]
fn parse_combos() {
    let c = &parse_combo("Ctrl+Maj+Z").unwrap()[0];
    assert!(c.ctrl && c.shift && !c.alt && c.key == KeyMatch::Char(Key::Z));
    assert_eq!(parse_combo("Ctrl+Shift+Z").unwrap(), parse_combo("Ctrl+Maj+Z").unwrap());
    assert_eq!(parse_combo("Alt+l").unwrap()[0].key, KeyMatch::Char(Key::L));
    assert_eq!(parse_combo("Entrée").unwrap()[0].key, KeyMatch::Char(Key::Enter));
    assert_eq!(parse_combo("Return").unwrap(), parse_combo("Entrée").unwrap());
    assert_eq!(parse_combo("[").unwrap()[0].key, KeyMatch::Pos(Key::OpenBracket));
    assert_eq!(parse_combo("1").unwrap()[0].key, KeyMatch::Pos(Key::Num1));
    assert_eq!(parse_combo("car:,").unwrap()[0].key, KeyMatch::Char(Key::Comma));
    assert_eq!(parse_combo("pos:A").unwrap()[0].key, KeyMatch::Pos(Key::A));
    assert_eq!(parse_combo("F5").unwrap()[0].key, KeyMatch::Char(Key::F5));
    let plus = parse_combo("+").unwrap();
    assert_eq!(plus.len(), 2, "touche =/+ et + du pavé numérique");
    assert!(plus.iter().all(|c| c.any_shift && !c.ctrl));
    let ctrl_plus = parse_combo("Ctrl++").unwrap();
    assert!(ctrl_plus[0].ctrl && ctrl_plus[0].any_shift);
    assert!(parse_combo("Ctrl+Truc").is_err());
    assert!(parse_combo("Hyper+A").is_err());
}

#[test]
fn azerty_letters_follow_the_printed_letter() {
    let map = default_map();
    // Sur un AZERTY, la touche marquée Q est à la place du A d'un QWERTY.
    assert_eq!(press(&map, Key::Q, Key::A, NONE, Mode::Animation).as_deref(), Some("calque.supprimer"));
    // La touche marquée A (place du Q QWERTY) : panneau précédent.
    assert_eq!(press(&map, Key::A, Key::Q, NONE, Mode::Animation).as_deref(), Some("panneau.precedent"));
    // W et X, comme dans tes réglages (W et X sont aux mêmes places en AZERTY et QWERTY, ou presque).
    assert_eq!(press(&map, Key::Z, Key::W, NONE, Mode::Animation), None, "Z AZERTY n'est pas W");
}

#[test]
fn azerty_punctuation_follows_the_position() {
    let map = default_map();
    // « , » de Storyboard Pro = la touche ; en AZERTY (même place que la virgule QWERTY).
    assert_eq!(press(&map, Key::Semicolon, Key::Comma, NONE, Mode::Animation).as_deref(), Some("image.precedente"));
    assert_eq!(press(&map, Key::Colon, Key::Period, NONE, Mode::Animation).as_deref(), Some("image.suivante"));
    // La vraie virgule AZERTY (touche à côté du N) ne déclenche rien.
    assert_eq!(press(&map, Key::Comma, Key::M, NONE, Mode::Animation), None);
    // Chiffres : la touche &/1 sans Maj.
    assert_eq!(press(&map, Key::Num1, Key::Num1, NONE, Mode::Animation).as_deref(), Some("vue.zoom_arriere"));
    // « + » : touche =/+ avec ou sans Maj, et + du pavé numérique.
    assert_eq!(press(&map, Key::Equals, Key::Equals, NONE, Mode::Storyboard).as_deref(), Some("panneau.ajouter_image"));
    assert_eq!(press(&map, Key::Plus, Key::Equals, Modifiers::SHIFT, Mode::Storyboard).as_deref(), Some("panneau.ajouter_image"));
    assert_eq!(press(&map, Key::Plus, Key::Plus, NONE, Mode::Storyboard).as_deref(), Some("panneau.ajouter_image"));
    assert_eq!(press(&map, Key::Equals, Key::Equals, Modifiers::ALT, Mode::Animation).as_deref(), Some("exposition.allonger"));
    // [ ] = touches ^ et $ en AZERTY.
    assert_eq!(press(&map, Key::OpenBracket, Key::OpenBracket, NONE, Mode::Animation).as_deref(), Some("outil.taille_moins"));
}

#[test]
fn modifiers_and_contexts() {
    let map = default_map();
    let ctrl = Modifiers::COMMAND;
    let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    assert_eq!(press(&map, Key::Z, Key::Z, ctrl, Mode::Animation).as_deref(), Some("edition.annuler"));
    assert_eq!(press(&map, Key::Z, Key::Z, ctrl_shift, Mode::Animation).as_deref(), Some("edition.retablir"));
    assert_eq!(press(&map, Key::Y, Key::Y, ctrl, Mode::Animation).as_deref(), Some("edition.retablir"));
    assert_eq!(press(&map, Key::Z, Key::Z, Modifiers::SHIFT, Mode::Animation).as_deref(), Some("vue.reinit_zoom"));
    assert_eq!(press(&map, Key::B, Key::B, Modifiers::ALT, Mode::Animation).as_deref(), Some("outil.pinceau"));
    assert_eq!(press(&map, Key::B, Key::B, NONE, Mode::Animation), None);
    assert_eq!(press(&map, Key::Insert, Key::Insert, NONE, Mode::Storyboard).as_deref(), Some("panneau.nouveau"));
    assert_eq!(press(&map, Key::Insert, Key::Insert, NONE, Mode::Animation).as_deref(), Some("dessin.nouveau"));
    assert_eq!(press(&map, Key::Delete, Key::Delete, ctrl, Mode::Storyboard).as_deref(), Some("panneau.supprimer"));
    assert_eq!(press(&map, Key::Delete, Key::Delete, ctrl, Mode::Animation).as_deref(), Some("dessin.supprimer"));
    assert_eq!(press(&map, Key::Tab, Key::Tab, NONE, Mode::Animation).as_deref(), Some("mode.basculer"));
    assert_eq!(press(&map, Key::Tab, Key::Tab, Modifiers::SHIFT, Mode::Storyboard).as_deref(), Some("vue.planches"));
    // Raccourcis en réserve : ils ne déclenchent rien.
    assert_eq!(press(&map, Key::Enter, Key::Enter, Modifiers::ALT, Mode::Storyboard), None);
    // Ctrl+L, en réserve pour « lier les clips au panneau », va à la pelure du calque (13b).
    assert_eq!(press(&map, Key::L, Key::L, ctrl, Mode::Storyboard).as_deref(), Some("calque.pelure"));
    // Réglages perso devenus actifs en 1e.
    assert_eq!(press(&map, Key::F6, Key::F6, NONE, Mode::Storyboard).as_deref(), Some("panneau.couper"));
    assert_eq!(press(&map, Key::W, Key::W, NONE, Mode::Animation).as_deref(), Some("calque.copier"));
    assert_eq!(press(&map, Key::E, Key::E, ctrl, Mode::Animation).as_deref(), Some("calque.fusionner_dessous"));
}

#[test]
fn menu_labels() {
    let map = default_map();
    assert_eq!(map.label("edition.retablir"), "Ctrl+Maj+Z");
    assert_eq!(map.label("lecture.basculer"), "Entrée");
    assert_eq!(map.label("image.precedente"), "; (QWERTY ,)");
    assert_eq!(map.label("outil.taille_plus"), "$ (QWERTY ])");
    assert_eq!(map.label("panneau.ajouter_image"), "+");
    assert_eq!(map.label("panneau.proprietes"), "", "pas d'étiquette pour un raccourci en réserve");
    assert_eq!(map.label("calque.verrouiller"), "Alt+L");
}

#[test]
fn user_file_overrides_per_action() {
    let user = r#"
        version = 1
        [[raccourci]]
        action = "calque.supprimer"
        touches = ["Maj+Suppr"]
        source = "essai"
        [[raccourci]]
        action = "action.inexistante"
        touches = ["K"]
    "#;
    let map = Keymap::from_texts(DEFAULT_KEYMAP, Some(user));
    assert_eq!(press(&map, Key::Q, Key::A, NONE, Mode::Animation), None, "Q n'est plus utilisé");
    assert_eq!(
        press(&map, Key::Delete, Key::Delete, Modifiers::SHIFT, Mode::Animation).as_deref(),
        Some("calque.supprimer")
    );
    // Le reste du fichier par défaut est toujours là.
    assert_eq!(press(&map, Key::Z, Key::Z, Modifiers::COMMAND, Mode::Animation).as_deref(), Some("edition.annuler"));
    assert!(map.warnings.iter().any(|w| w.contains("action.inexistante")));
}

#[test]
fn conflicts_and_broken_files_are_reported() {
    let user = r#"
        version = 1
        [[raccourci]]
        action = "outil.main"
        touches = ["Ctrl+Z"]
    "#;
    let map = Keymap::from_texts(DEFAULT_KEYMAP, Some(user));
    assert!(map.warnings.iter().any(|w| w.contains("conflit")), "{:?}", map.warnings);

    let broken = Keymap::from_texts(DEFAULT_KEYMAP, Some("ceci n'est pas du toml ["));
    assert!(broken.warnings.iter().any(|w| w.contains("illisible")));
    assert_eq!(
        press(&broken, Key::Z, Key::Z, Modifiers::COMMAND, Mode::Animation).as_deref(),
        Some("edition.annuler"),
        "un fichier cassé ne casse pas les raccourcis par défaut"
    );
}

// ------------------------------------------------------- raccourcis choisis dans Pelure

#[test]
fn typed_shortcut_is_written_like_the_file() {
    use crate::keymap::combo_text;
    let ctrl_shift = Modifiers { ctrl: true, command: true, shift: true, ..NONE };
    assert_eq!(combo_text(Key::K, Some(Key::K), ctrl_shift).as_deref(), Some("Ctrl+Maj+K"));
    assert_eq!(combo_text(Key::F5, Some(Key::F5), NONE).as_deref(), Some("F5"));
    assert_eq!(combo_text(Key::Enter, Some(Key::Enter), NONE).as_deref(), Some("Entrée"));
    assert_eq!(combo_text(Key::ArrowLeft, Some(Key::ArrowLeft), Modifiers::ALT).as_deref(), Some("Alt+Gauche"));
    assert_eq!(combo_text(Key::Escape, Some(Key::Escape), NONE), None, "Échap sert à annuler");
    // Chiffres et signes : par leur position. En AZERTY, la touche « ; » est à la place du « , »
    // du QWERTY : egui la lit Point-virgule, à la position Virgule.
    assert_eq!(combo_text(Key::Num1, Some(Key::Num1), Modifiers::SHIFT).as_deref(), Some("Maj+1"));
    let t = combo_text(Key::Semicolon, Some(Key::Comma), NONE).unwrap();
    assert_eq!(t, ",");
    let combos = parse_combo(&t).unwrap();
    assert!(combos[0].matches(Key::Semicolon, Some(Key::Comma), NONE), "relu, il reconnaît la même touche");
}

#[test]
fn assigned_shortcut_replaces_frees_and_returns_to_default() {
    let map = default_map();
    assert!(map.conflicts("F6", "fichier.importer_sboard").contains(&"panneau.couper".to_string()));
    let changes = map.assign_changes("fichier.importer_sboard", "F6", false);
    let text = map.edited_user_file(None, &changes).unwrap();
    let map2 = Keymap::from_texts(DEFAULT_KEYMAP, Some(&text));
    assert!(map2.warnings.is_empty(), "{:#?}", map2.warnings);
    assert_eq!(press(&map2, Key::F6, Key::F6, NONE, Mode::Storyboard).as_deref(), Some("fichier.importer_sboard"));
    assert!(map2.labels_of("panneau.couper").is_empty(), "F6 est retiré de l'action qui l'avait");
    assert!(map2.is_user("fichier.importer_sboard") && map2.is_user("panneau.couper"));
    // Ajouter une deuxième touche garde la première.
    let text2 = map2.edited_user_file(Some(&text), &map2.assign_changes("fichier.importer_sboard", "Alt+F12", true)).unwrap();
    let map3 = Keymap::from_texts(DEFAULT_KEYMAP, Some(&text2));
    assert_eq!(map3.labels_of("fichier.importer_sboard"), vec!["F6".to_string(), "Alt+F12".to_string()]);
    // Retour au défaut : la découpe retrouve F6 (et le conflit est signalé).
    let text3 = map3.edited_user_file(Some(&text2), &[("panneau.couper".into(), None)]).unwrap();
    let map4 = Keymap::from_texts(DEFAULT_KEYMAP, Some(&text3));
    assert_eq!(map4.labels_of("panneau.couper"), vec!["F6".to_string()]);
    assert!(!map4.is_user("panneau.couper"));
    assert!(map4.warnings.iter().any(|w| w.contains("conflit")));
    // Plus de raccourci du tout.
    let text4 = map4.edited_user_file(Some(&text3), &[("fichier.importer_sboard".into(), Some(Vec::new()))]).unwrap();
    assert!(Keymap::from_texts(DEFAULT_KEYMAP, Some(&text4)).labels_of("fichier.importer_sboard").is_empty());
    // Un fichier personnel illisible n'est pas écrasé.
    assert!(map.edited_user_file(Some("ceci n'est pas du toml ["), &changes).is_err());
}

#[test]
fn shortcuts_follow_the_pane_that_has_the_hand() {
    use crate::app::key_mode_of;
    let map = default_map();
    let insert = |mode| press(&map, Key::Insert, Key::Insert, NONE, mode);
    let left = |mode| press(&map, Key::Semicolon, Key::Comma, NONE, mode);
    // Dernier clic dans le dessin (aucune bande n'a la main) : le mode du dessin, comme avant.
    assert_eq!(key_mode_of(Mode::Storyboard, false, false, false), Mode::Storyboard);
    assert_eq!(key_mode_of(Mode::Animation, false, false, false), Mode::Animation);
    // La piste d'un calque dans le montage : on travaille les clés, les raccourcis de l'animation.
    assert_eq!(key_mode_of(Mode::Storyboard, true, false, true), Mode::Animation);
    // La timeline a la main en storyboard : Insert fait un nouveau dessin, pas un nouveau plan.
    let timeline = key_mode_of(Mode::Storyboard, false, true, false);
    assert_eq!(insert(timeline).as_deref(), Some("dessin.nouveau"));
    assert_eq!(left(timeline).as_deref(), Some("image.precedente"));
    // Le montage a la main en animation : Insert fait un nouveau plan.
    assert_eq!(insert(key_mode_of(Mode::Animation, true, false, false)).as_deref(), Some("panneau.nouveau"));
    // Les deux à la fois ne devraient pas arriver (chaque clic retire l'autre) : le montage l'emporte.
    assert_eq!(key_mode_of(Mode::Animation, true, true, false), Mode::Storyboard);
}

#[test]
fn the_strip_zooms_with_alt_and_the_zoom_keys() {
    // Comme 1 et 2 pour le dessin (positions de touche) : Alt+1 arrière, Alt+2 avant, Alt+3 tout voir.
    let map = default_map();
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(press(&map, Key::Num1, Key::Num1, Modifiers::ALT, mode).as_deref(), Some("bande.zoom_arriere"));
        assert_eq!(press(&map, Key::Num2, Key::Num2, Modifiers::ALT, mode).as_deref(), Some("bande.zoom_avant"));
        assert_eq!(press(&map, Key::Num3, Key::Num3, Modifiers::ALT, mode).as_deref(), Some("bande.tout_voir"));
    }
    // Sans Alt, 1 et 2 zooment toujours le dessin.
    assert_eq!(press(&map, Key::Num1, Key::Num1, NONE, Mode::Storyboard).as_deref(), Some("vue.zoom_arriere"));
}

#[test]
fn f5_lays_a_key_everywhere_and_f6_cuts_the_plan() {
    // Anthony, 2026-10-17 : « F5 pose une clé partout. F6 coupe le plan ».
    let map = default_map();
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(press(&map, Key::F5, Key::F5, NONE, mode).as_deref(), Some("dessin.nouvelle_cle"));
        assert_eq!(press(&map, Key::F6, Key::F6, NONE, mode).as_deref(), Some("panneau.couper"));
    }
}