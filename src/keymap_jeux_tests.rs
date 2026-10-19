//! Les jeux de raccourcis (2026-10-19) : « SHOT (par défaut) » et « Façon Storyboard Pro ». Le jeu
//! Storyboard Pro ne redéfinit que ce que `keymap.toml` documente, n'a aucun conflit en lui, ne vise
//! que des commandes faites ; on passe de l'un à l'autre sans perdre une touche, et le fichier
//! personnel reste par-dessus.

use std::collections::BTreeSet;

use eframe::egui::{Key, Modifiers};

use crate::app::Mode;
use crate::keymap::{action_info, Keymap, Origin, PresetId, ACTIONS, DEFAULT_KEYMAP, STORYBOARD_PRO_KEYMAP};
use crate::keymap_tests::{default_map, shared_keys, SBP27_IDS};
use crate::settings::Settings;

const NONE: Modifiers = Modifiers::NONE;

fn shot() -> Keymap {
    Keymap::build(PresetId::Shot, None)
}

fn sbp() -> Keymap {
    Keymap::build(PresetId::StoryboardPro, None)
}

fn press(map: &Keymap, key: Key, m: Modifiers, mode: Mode) -> Option<String> {
    map.find(key, Some(key), m, mode).map(str::to_string)
}

/// Tout ce qui compte d'un jeu de raccourcis, pour comparer deux états au caractère près.
fn snapshot(map: &Keymap) -> Vec<String> {
    let mut v: Vec<String> = map
        .bindings
        .iter()
        .map(|b| {
            format!(
                "{}|{:?}|{:?}|{}|{}|{}|{}|{}|{}",
                b.action,
                b.keys,
                b.context,
                b.active,
                b.reserved,
                b.source,
                b.user,
                b.preset,
                map.origin_of(&b.action).label()
            )
        })
        .collect();
    v.push(format!("{:?}|{:?}|{:?}", map.preset, map.warnings, map.notes));
    v
}

/// Ce que le jeu Storyboard Pro redéfinit, et les touches qu'il donne (aucune : il les retire).
const SET: [(&str, &[&str]); 8] = [
    ("panneau.precedent", &["A"]),
    ("panneau.suivant", &["F"]),
    ("cle.precedente", &[]),
    ("cle.suivante", &[]),
    ("edition.coller_sur_place", &["Ctrl+Maj+V"]),
    ("edition.coller_a_la_tete", &[]),
    ("panneau.couper", &["F5"]),
    ("dessin.nouvelle_cle", &[]),
];

/// Un fichier personnel qui donne `key` à `action` dans l'état `map` (comme la fenêtre « Assigner »).
fn user_text(map: &Keymap, action: &str, key: &str) -> String {
    map.edited_user_file(None, &map.assign_changes(action, key, false)).unwrap()
}

#[test]
fn the_storyboard_pro_set_has_no_internal_conflict() {
    // Le jeu seul (lu comme un fichier de raccourcis) : aucune remarque, aucune touche partagée.
    let alone = Keymap::from_texts(STORYBOARD_PRO_KEYMAP, None);
    assert!(alone.warnings.is_empty(), "{:#?}", alone.warnings);
    assert!(shared_keys(&alone).is_empty(), "{:#?}", shared_keys(&alone));
    // Posé sur SHOT : la même vérification que pour les raccourcis par défaut (réserve comprise).
    let map = sbp();
    assert_eq!(map.preset, PresetId::StoryboardPro);
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    assert!(shared_keys(&map).is_empty(), "touches partagées : {:#?}", shared_keys(&map));
    // Et SHOT n'a pas bougé.
    assert!(shared_keys(&shot()).is_empty());
}

#[test]
fn every_preset_binding_targets_an_implemented_action() {
    let map = sbp();
    let in_preset: Vec<_> = map.bindings.iter().filter(|b| b.preset).collect();
    assert!(!in_preset.is_empty());
    for b in &in_preset {
        let info = action_info(&b.action).unwrap_or_else(|| panic!("{} n'existe pas", b.action));
        assert!(info.implemented, "{} n'est pas faite : le jeu ne peut pas lui donner de touche", b.action);
        assert!(b.active && !b.reserved, "{} doit être active", b.action);
        assert_eq!(b.combos.len() > 0, !b.keys.is_empty(), "{} : touches illisibles", b.action);
    }
    // Les « à vérifier » aussi visent des commandes faites.
    for (action, note) in &map.notes {
        assert!(action_info(action).is_some_and(|i| i.implemented), "{action} : à vérifier mais pas faite");
        assert!(!note.trim().is_empty(), "{action} : note vide");
    }
}

#[test]
fn the_preset_redefines_only_what_keymap_toml_documents() {
    let map = sbp();
    let redefined: BTreeSet<&str> = map.bindings.iter().filter(|b| b.preset).map(|b| b.action.as_str()).collect();
    assert_eq!(redefined, SET.iter().map(|(a, _)| *a).collect::<BTreeSet<_>>());
    for (action, keys) in SET {
        let blocks: Vec<_> = map.bindings.iter().filter(|b| b.action == action).collect();
        assert_eq!(blocks.len(), 1, "{action}");
        let b = blocks[0];
        assert_eq!(b.keys, keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), "{action}");
        assert_eq!(map.origin_of(action), Origin::Preset);
        // Le bloc de SHOT que le jeu remplace est seul et sans contexte : rien d'autre n'est perdu.
        let in_default: Vec<_> = default_map().bindings.iter().filter(|d| d.action == action).map(|d| d.context).collect();
        assert_eq!(in_default, vec![b.context], "{action} : le jeu doit garder le contexte de SHOT");
        // Une touche donnée cite Storyboard Pro ; un retrait cite SHOT.
        if keys.is_empty() {
            assert!(b.source.starts_with("SHOT : "), "{action} : {}", b.source);
        } else if let Some(id) = b.source.strip_prefix("Storyboard Pro 27 : ") {
            assert!(SBP27_IDS.contains(&id), "{id} n'est pas un raccourci de Storyboard Pro 27");
        } else {
            assert!(b.source.starts_with("perso : SPLIT_PANEL_KEY"), "{action} : source {}", b.source);
        }
    }
    // Chaque touche donnée se retrouve, notée par le fichier de SHOT, comme celle de Storyboard Pro.
    for (note, pourquoi) in [
        ("PREV_PANEL_KEY est A", "A"),
        ("NEXT_PANEL_KEY est F", "F"),
        ("PASTE_IN_PLACE_KEY (sa touche Ctrl+Maj+V", "Ctrl+Maj+V"),
        ("F5 dans son Storyboard Pro", "F5"),
    ] {
        assert!(DEFAULT_KEYMAP.contains(note), "keymap.toml ne documente plus {pourquoi} ({note})");
    }
    // Tout le reste garde la touche de SHOT, à l'identique.
    let (a, b) = (shot(), sbp());
    for info in ACTIONS.iter().filter(|i| !redefined.contains(i.id)) {
        let ba: Vec<_> = a.bindings.iter().filter(|x| x.action == info.id).map(|x| (&x.keys, x.context, x.active, &x.source)).collect();
        let bb: Vec<_> = b.bindings.iter().filter(|x| x.action == info.id).map(|x| (&x.keys, x.context, x.active, &x.source)).collect();
        assert_eq!(ba, bb, "{} ne devait pas changer", info.id);
    }
}

#[test]
fn the_preset_keys_work_and_their_old_owners_let_go() {
    let (shot, sbp) = (shot(), sbp());
    let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    for mode in [Mode::Storyboard, Mode::Animation] {
        // SHOT : A / F de clé en clé, PgPréc / PgSuiv de plan en plan, F5 une clé, F6 coupe.
        assert_eq!(press(&shot, Key::A, NONE, mode).as_deref(), Some("cle.precedente"));
        assert_eq!(press(&shot, Key::F, NONE, mode).as_deref(), Some("cle.suivante"));
        assert_eq!(press(&shot, Key::PageUp, NONE, mode).as_deref(), Some("panneau.precedent"));
        assert_eq!(press(&shot, Key::F5, NONE, mode).as_deref(), Some("dessin.nouvelle_cle"));
        assert_eq!(press(&shot, Key::F6, NONE, mode).as_deref(), Some("panneau.couper"));
        assert_eq!(press(&shot, Key::V, ctrl_shift, mode).as_deref(), Some("edition.coller_a_la_tete"));
        // Storyboard Pro : A / F changent de plan, Ctrl+Maj+V colle sur place, F5 coupe.
        assert_eq!(press(&sbp, Key::A, NONE, mode).as_deref(), Some("panneau.precedent"));
        assert_eq!(press(&sbp, Key::F, NONE, mode).as_deref(), Some("panneau.suivant"));
        assert_eq!(press(&sbp, Key::V, ctrl_shift, mode).as_deref(), Some("edition.coller_sur_place"));
        assert_eq!(press(&sbp, Key::F5, NONE, mode).as_deref(), Some("panneau.couper"));
        // Les anciennes touches du plan et de la coupe sont libres ; Ctrl+V, F4, Ctrl+L, Suppr ne bougent pas.
        assert_eq!(press(&sbp, Key::PageUp, NONE, mode), None);
        assert_eq!(press(&sbp, Key::PageDown, NONE, mode), None);
        assert_eq!(press(&sbp, Key::F6, NONE, mode), None);
        assert_eq!(press(&sbp, Key::V, Modifiers::COMMAND, mode).as_deref(), Some("edition.coller"));
        assert_eq!(press(&sbp, Key::F4, NONE, mode).as_deref(), Some("dessin.nouvelle_cle_vide"));
        assert_eq!(press(&sbp, Key::L, Modifiers::COMMAND, mode).as_deref(), Some("calque.pelure"));
        assert_eq!(press(&sbp, Key::Delete, NONE, mode).as_deref(), Some("edition.supprimer"));
        assert_eq!(press(&sbp, Key::Delete, Modifiers::SHIFT, mode).as_deref(), Some("dessin.supprimer_hors_selection"));
    }
    // Le plan précédent / suivant par Ctrl+PgPréc / Ctrl+PgSuiv (second raccourci) ne change pas.
    assert_eq!(press(&sbp, Key::PageUp, Modifiers::COMMAND, Mode::Storyboard).as_deref(), Some("scene.precedente"));
    // Les noms montrés.
    assert_eq!(sbp.keys_text("panneau.suivant"), "F");
    assert_eq!(sbp.keys_text("cle.suivante"), "");
    assert_eq!(shot.keys_text("panneau.suivant"), "Pg suiv.");
    assert_eq!(sbp.keys_text("panneau.couper"), "F5");
}

#[test]
fn switching_back_and_forth_restores_the_exact_bindings() {
    let first = snapshot(&shot());
    let preset = snapshot(&sbp());
    assert_ne!(first, preset, "les deux jeux se distinguent");
    // SHOT -> Storyboard Pro -> SHOT -> Storyboard Pro -> SHOT : les mêmes touches, au caractère près.
    for _ in 0..3 {
        assert_eq!(snapshot(&sbp()), preset);
        assert_eq!(snapshot(&shot()), first);
    }
    // Le jeu SHOT est exactement les raccourcis par défaut d'avant les jeux.
    assert_eq!(snapshot(&Keymap::from_texts(DEFAULT_KEYMAP, None)), first);
    // Avec un fichier personnel, pareil.
    let user = user_text(&shot(), "outil.main", "Alt+F12");
    let (u_shot, u_sbp) = (snapshot(&Keymap::build(PresetId::Shot, Some(&user))), snapshot(&Keymap::build(PresetId::StoryboardPro, Some(&user))));
    for _ in 0..3 {
        assert_eq!(snapshot(&Keymap::build(PresetId::StoryboardPro, Some(&user))), u_sbp);
        assert_eq!(snapshot(&Keymap::build(PresetId::Shot, Some(&user))), u_shot);
    }
    // Sans le fichier personnel, SHOT revient sans trace du jeu : aucune touche « jeu ».
    assert!(shot().bindings.iter().all(|b| !b.preset) && shot().notes.is_empty());
}

#[test]
fn user_overrides_survive_a_switch() {
    // 1. Une commande que le jeu ne touche pas : la touche de l'utilisateur joue dans les deux jeux.
    let user = user_text(&shot(), "outil.main", "Alt+F12");
    for id in PresetId::ALL {
        let map = Keymap::build(id, Some(&user));
        assert_eq!(press(&map, Key::F12, Modifiers::ALT, Mode::Animation).as_deref(), Some("outil.main"), "{id:?}");
        assert_eq!(map.origin_of("outil.main"), Origin::User);
        assert_eq!(map.user_overrides(), 1, "{id:?}");
        assert!(map.warnings.is_empty(), "{id:?} : {:#?}", map.warnings);
    }
    // 2. Une commande que le jeu redéfinit : le choix de l'utilisateur l'emporte sur le jeu, et il
    //    reste quand on revient à SHOT.
    let user = user_text(&sbp(), "panneau.couper", "F9");
    let map = Keymap::build(PresetId::StoryboardPro, Some(&user));
    assert_eq!(press(&map, Key::F9, NONE, Mode::Storyboard).as_deref(), Some("panneau.couper"));
    assert_eq!(press(&map, Key::F5, NONE, Mode::Storyboard), None, "F5 ne coupe plus : la commande a la touche de l'utilisateur");
    assert_eq!(map.origin_of("panneau.couper"), Origin::User);
    assert_eq!(map.verify_note("panneau.couper"), None);
    let back = Keymap::build(PresetId::Shot, Some(&user));
    assert_eq!(press(&back, Key::F9, NONE, Mode::Storyboard).as_deref(), Some("panneau.couper"));
    assert_eq!(press(&back, Key::F5, NONE, Mode::Storyboard).as_deref(), Some("dessin.nouvelle_cle"));
    // 3. Une touche du jeu que l'utilisateur prend à une autre commande : la sienne joue, dans les
    //    deux jeux. Sous Storyboard Pro, « Assigner » a retiré A au plan précédent (aucun conflit) ;
    //    sous SHOT la touche est aussi à la clé précédente : la sienne l'emporte et une remarque le dit.
    let user = user_text(&sbp(), "outil.main", "A");
    let map = Keymap::build(PresetId::StoryboardPro, Some(&user));
    assert_eq!(press(&map, Key::A, NONE, Mode::Animation).as_deref(), Some("outil.main"));
    assert!(map.labels_of("panneau.precedent").is_empty());
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    let back = Keymap::build(PresetId::Shot, Some(&user));
    assert_eq!(press(&back, Key::A, NONE, Mode::Animation).as_deref(), Some("outil.main"), "la touche de l'utilisateur l'emporte");
    assert!(back.warnings.iter().any(|w| w.contains("conflit") && w.contains("cle.precedente")), "{:?}", back.warnings);
    // Limite connue : le retrait écrit sous Storyboard Pro (« A n'est plus au plan précédent ») est
    // un choix du fichier personnel, il reste après le changement de jeu ; « Raccourci par défaut »
    // (liste des raccourcis) rend sa touche à la commande.
    assert!(back.labels_of("panneau.precedent").is_empty());
    let undo = back.edited_user_file(Some(&user), &[("panneau.precedent".into(), None)]).unwrap();
    assert_eq!(Keymap::build(PresetId::Shot, Some(&undo)).labels_of("panneau.precedent"), vec!["Pg préc.".to_string()]);
    // 4. Le retour au raccourci par défaut d'une commande redonne celui du jeu choisi.
    let user = user_text(&sbp(), "panneau.couper", "F9");
    let map = Keymap::build(PresetId::StoryboardPro, Some(&user));
    let reset = map.edited_user_file(Some(&user), &[("panneau.couper".into(), None)]).unwrap();
    let map = Keymap::build(PresetId::StoryboardPro, Some(&reset));
    assert_eq!(map.labels_of("panneau.couper"), vec!["F5".to_string()]);
    assert_eq!(map.origin_of("panneau.couper"), Origin::Preset);
    assert_eq!(map.user_overrides(), 0);
}

#[test]
fn a_verbatim_copy_of_the_default_file_does_not_hide_the_preset() {
    // « Modifier à la main » crée une copie du fichier par défaut : ce ne sont pas des choix de
    // l'utilisateur, le jeu doit continuer à jouer.
    let with_copy = Keymap::build(PresetId::StoryboardPro, Some(DEFAULT_KEYMAP));
    assert_eq!(with_copy.user_overrides(), 0);
    assert_eq!(press(&with_copy, Key::A, NONE, Mode::Storyboard).as_deref(), Some("panneau.precedent"));
    assert_eq!(snapshot(&with_copy), snapshot(&sbp()));
    // Une action de la copie que l'utilisateur a changée, elle, reste la sienne ; son autre bloc
    // (inchangé) ne se perd pas.
    let user = r#"
        version = 1
        [[raccourci]]
        action = "panneau.nouveau"
        touches = ["P"]
        source = "Storyboard Pro 27 : NEW_PANEL_KEY"
        [[raccourci]]
        action = "panneau.nouveau"
        touches = ["Maj+Inser"]
        contexte = "storyboard"
        source = "choisi dans SHOT"
        [[raccourci]]
        action = "fichier.nouveau"
        touches = ["Ctrl+N"]
        source = "Storyboard Pro 27 : NEW_KEY"
    "#;
    let map = Keymap::build(PresetId::StoryboardPro, Some(user));
    assert_eq!(map.user_overrides(), 1, "seule panneau.nouveau est un choix");
    assert_eq!(press(&map, Key::P, NONE, Mode::Storyboard).as_deref(), Some("panneau.nouveau"));
    assert_eq!(press(&map, Key::Insert, Modifiers::SHIFT, Mode::Storyboard).as_deref(), Some("panneau.nouveau"));
    assert_eq!(map.origin_of("fichier.nouveau"), Origin::Default);
}

#[test]
fn what_the_preset_could_not_verify_is_marked() {
    let map = sbp();
    // Le bloc du jeu lui-même (nouvelle clé sans touche) et les quatre commandes qui gardent la
    // touche de SHOT : Tab, Maj+Tab, Ctrl+K, F4.
    let notes: BTreeSet<&str> = map.notes.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(
        notes,
        BTreeSet::from(["dessin.nouvelle_cle", "mode.basculer", "vue.planches", "aide.palette", "dessin.nouvelle_cle_vide"])
    );
    for action in ["mode.basculer", "vue.planches", "aide.palette", "dessin.nouvelle_cle_vide"] {
        assert_eq!(map.origin_of(action), Origin::Default, "{action} garde la touche de SHOT");
        assert!(!map.keys_text(action).is_empty(), "{action}");
        assert!(map.verify_note(action).is_some(), "{action}");
    }
    assert!(map.verify_note("dessin.nouvelle_cle").is_some());
    assert!(map.verify_note("edition.copier").is_none());
    assert_eq!(map.notes.len(), 5);
    // SHOT n'a rien à vérifier.
    assert!(shot().notes.is_empty() && shot().verify_note("mode.basculer").is_none());
}

#[test]
fn the_choice_is_saved_in_the_settings_and_old_settings_load() {
    // Un réglage d'avant les jeux : SHOT, le reste intact.
    let old: Settings = toml::from_str("copie_secours_minutes = 7\ncurseur_systeme = false\nrouvrir_dernier_projet = false\n").unwrap();
    assert_eq!(old.keymap_preset, PresetId::Shot);
    assert_eq!((old.autosave_minutes, old.system_cursor, old.reopen_last), (7, false, false));
    let empty: Settings = toml::from_str("").unwrap();
    assert_eq!(empty.keymap_preset, PresetId::Shot);
    // Le jeu par défaut n'ajoute rien au fichier : un ancien fichier reste tel quel.
    let text = toml::to_string(&Settings::default()).unwrap();
    assert!(!text.contains("jeu_raccourcis"), "{text}");
    // Storyboard Pro s'écrit et se relit.
    let mut s = Settings::default();
    s.keymap_preset = PresetId::StoryboardPro;
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("jeu_raccourcis = \"storyboard_pro\""), "{text}");
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.keymap_preset, PresetId::StoryboardPro);
    // Un nom inconnu (réglage d'une autre version) ne bloque rien : SHOT, et les autres réglages restent.
    let odd: Settings = toml::from_str("copie_secours_minutes = 9\njeu_raccourcis = \"autre_chose\"\n").unwrap();
    assert_eq!((odd.keymap_preset, odd.autosave_minutes), (PresetId::Shot, 9));
    for p in PresetId::ALL {
        assert_eq!(PresetId::from_name(p.name()), p);
        assert!(!p.label().is_empty() && !p.description().is_empty());
    }
    assert!(PresetId::Shot.is_default() && !PresetId::StoryboardPro.is_default());
}

#[test]
fn a_broken_preset_text_leaves_shot_in_place() {
    let map = Keymap::from_layers(DEFAULT_KEYMAP, Some((PresetId::StoryboardPro, "ceci n'est pas du toml [")), None);
    assert!(map.warnings.iter().any(|w| w.contains("illisible")), "{:?}", map.warnings);
    assert_eq!(map.preset, PresetId::Shot, "le jeu n'a pas pris : on reste sur SHOT");
    assert_eq!(snapshot(&map)[..map.bindings.len()], snapshot(&shot())[..map.bindings.len()]);
}
