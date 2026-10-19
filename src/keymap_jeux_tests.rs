//! Les jeux de raccourcis (2026-10-19) : « SHOT (par défaut) » et « Façon Storyboard Pro ». Le jeu
//! Storyboard Pro garde pour l'instant toutes les touches de SHOT (Ctrl+Maj+V, A / F et F5 comme
//! Anthony les a réglées) et n'ajoute que des « à vérifier » ; il n'a aucun conflit en lui, ne vise
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

/// Les touches du jeu Storyboard Pro sont celles de SHOT, sans exception : depuis les choix d'Anthony
/// du 2026-10-19, A / F (clé précédente / suivante), F5 (nouvelle clé) et Ctrl+Maj+V (coller les clés à
/// la tête de lecture) gardent leurs touches de SHOT. Le jeu n'a aucune redéfinition ; il ne montre
/// que des « à vérifier ». Si une redéfinition revient, la mettre ici (action, touches données).
const SET: [(&str, &[&str]); 0] = [];

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
    // Aujourd'hui le jeu n'a aucune redéfinition (la boucle ne tourne pas) ; si une revient, elle est
    // vérifiée ici.
    let in_preset: Vec<_> = map.bindings.iter().filter(|b| b.preset).collect();
    for b in &in_preset {
        let info = action_info(&b.action).unwrap_or_else(|| panic!("{} n'existe pas", b.action));
        assert!(info.implemented, "{} n'est pas faite : le jeu ne peut pas lui donner de touche", b.action);
        assert!(b.active && !b.reserved, "{} doit être active", b.action);
        assert_eq!(b.combos.len() > 0, !b.keys.is_empty(), "{} : touches illisibles", b.action);
    }
    // Les « à vérifier » aussi visent des commandes faites, et le jeu les expose bien.
    assert!(!map.notes.is_empty(), "le jeu doit toujours exposer ses « à vérifier »");
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
        } else {
            let id = b.source.strip_prefix("Storyboard Pro 27 : ").unwrap_or_else(|| panic!("{action} : source {}", b.source));
            assert!(SBP27_IDS.contains(&id), "{id} n'est pas un raccourci de Storyboard Pro 27");
        }
    }
    // Les choix d'Anthony (2026-10-19) sont écrits dans le jeu, et leurs touches n'y sont pas :
    // A / F, F5 (le commentaire du jeu) et Ctrl+Maj+V (coller à la tête, noté dans keymap.toml).
    assert!(STORYBOARD_PRO_KEYMAP.contains("VOLONTAIREMENT PAS pris à Storyboard Pro"));
    assert!(STORYBOARD_PRO_KEYMAP.contains("Ctrl+Maj+V non plus"));
    assert!(DEFAULT_KEYMAP.contains("PASTE_IN_PLACE_KEY (sa touche Ctrl+Maj+V va à edition.coller_a_la_tete)"));
    for kept in [
        "cle.precedente",
        "cle.suivante",
        "dessin.nouvelle_cle",
        "panneau.precedent",
        "panneau.suivant",
        "panneau.couper",
        "edition.coller_a_la_tete",
        "edition.coller_sur_place",
    ] {
        assert!(!redefined.contains(kept), "{kept} doit garder la touche de SHOT");
        assert_eq!(map.origin_of(kept), Origin::Default, "{kept}");
    }
    // Le jeu ne change AUCUNE touche : toutes les actions ont les mêmes blocs que sous SHOT, au
    // caractère près (touches, contexte, état, réserve, source, origine).
    let (a, b) = (shot(), sbp());
    assert!(redefined.is_empty(), "le jeu ne redéfinit rien : {redefined:?}");
    assert_eq!(a.bindings.len(), b.bindings.len());
    for info in ACTIONS.iter() {
        let ba: Vec<_> = a.bindings.iter().filter(|x| x.action == info.id).map(|x| (&x.keys, x.context, x.active, x.reserved, &x.source)).collect();
        let bb: Vec<_> = b.bindings.iter().filter(|x| x.action == info.id).map(|x| (&x.keys, x.context, x.active, x.reserved, &x.source)).collect();
        assert_eq!(ba, bb, "{} ne devait pas changer", info.id);
        assert_eq!(a.keys_text(info.id), b.keys_text(info.id), "{}", info.id);
        assert_eq!(b.origin_of(info.id), Origin::Default, "{}", info.id);
    }
    // Seuls l'identité du jeu et ses « à vérifier » distinguent les deux cartes.
    let (sa, sb) = (snapshot(&a), snapshot(&b));
    assert_eq!(sa[..sa.len() - 1], sb[..sb.len() - 1]);
    assert_eq!((a.preset, b.preset), (PresetId::Shot, PresetId::StoryboardPro));
    assert!(a.notes.is_empty() && !b.notes.is_empty());
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
        // Storyboard Pro : rien ne change. Ctrl+Maj+V colle les clés à la tête de lecture, A / F (clé)
        // et F5 (nouvelle clé) restent à Anthony (choix du 2026-10-19), PgPréc / PgSuiv et F6 aussi.
        assert_eq!(press(&sbp, Key::V, ctrl_shift, mode).as_deref(), Some("edition.coller_a_la_tete"));
        for (key, action) in [
            (Key::A, "cle.precedente"),
            (Key::F, "cle.suivante"),
            (Key::PageUp, "panneau.precedent"),
            (Key::PageDown, "panneau.suivant"),
            (Key::F5, "dessin.nouvelle_cle"),
            (Key::F6, "panneau.couper"),
        ] {
            assert_eq!(press(&sbp, key, NONE, mode).as_deref(), Some(action), "{action} garde la touche de SHOT");
            assert_eq!(press(&sbp, key, NONE, mode), press(&shot, key, NONE, mode), "{action}");
        }
        // Ctrl+V, F4, Ctrl+L, Suppr ne bougent pas.
        assert_eq!(press(&sbp, Key::V, Modifiers::COMMAND, mode).as_deref(), Some("edition.coller"));
        assert_eq!(press(&sbp, Key::F4, NONE, mode).as_deref(), Some("dessin.nouvelle_cle_vide"));
        assert_eq!(press(&sbp, Key::L, Modifiers::COMMAND, mode).as_deref(), Some("calque.pelure"));
        assert_eq!(press(&sbp, Key::Delete, NONE, mode).as_deref(), Some("edition.supprimer"));
        assert_eq!(press(&sbp, Key::Delete, Modifiers::SHIFT, mode).as_deref(), Some("dessin.supprimer_hors_selection"));
    }
    // Le plan précédent / suivant par Ctrl+PgPréc / Ctrl+PgSuiv (second raccourci) ne change pas.
    assert_eq!(press(&sbp, Key::PageUp, Modifiers::COMMAND, Mode::Storyboard).as_deref(), Some("scene.precedente"));
    // Les noms montrés.
    assert_eq!(sbp.keys_text("panneau.suivant"), "Pg suiv.");
    assert_eq!(sbp.keys_text("cle.suivante"), "F");
    assert_eq!(shot.keys_text("panneau.suivant"), "Pg suiv.");
    assert_eq!(sbp.keys_text("panneau.couper"), "F6");
    assert_eq!(sbp.keys_text("dessin.nouvelle_cle"), "F5");
    assert_eq!(sbp.keys_text("edition.coller_a_la_tete"), "Ctrl+Maj+V");
    assert_eq!(shot.keys_text("edition.coller_a_la_tete"), "Ctrl+Maj+V");
    // « Coller sur place » reste une commande sans touche, dans les deux jeux.
    assert_eq!(sbp.keys_text("edition.coller_sur_place"), "");
    assert_eq!(shot.keys_text("edition.coller_sur_place"), "");
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
    // 2. Une commande sans touche dans les deux jeux (« Coller sur place ») : la touche de l'utilisateur
    //    joue dans les deux, Ctrl+Maj+V reste à « Coller à la tête de lecture ».
    let user = user_text(&sbp(), "edition.coller_sur_place", "F9");
    let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    for id in PresetId::ALL {
        let map = Keymap::build(id, Some(&user));
        assert_eq!(press(&map, Key::F9, NONE, Mode::Storyboard).as_deref(), Some("edition.coller_sur_place"), "{id:?}");
        assert_eq!(press(&map, Key::V, ctrl_shift, Mode::Storyboard).as_deref(), Some("edition.coller_a_la_tete"), "{id:?}");
        assert_eq!(map.origin_of("edition.coller_sur_place"), Origin::User, "{id:?}");
        assert_eq!(map.origin_of("edition.coller_a_la_tete"), Origin::Default, "{id:?}");
        assert_eq!(map.user_overrides(), 1, "{id:?}");
        assert!(map.warnings.is_empty(), "{id:?} : {:#?}", map.warnings);
    }
    // Un « à vérifier » du jeu disparaît quand l'utilisateur a choisi la touche de la commande.
    let user_tab = user_text(&sbp(), "mode.basculer", "F10");
    let map = Keymap::build(PresetId::StoryboardPro, Some(&user_tab));
    assert!(sbp().verify_note("mode.basculer").is_some());
    assert_eq!(map.verify_note("mode.basculer"), None);
    assert_eq!(map.origin_of("mode.basculer"), Origin::User);
    assert_eq!(press(&map, Key::F10, NONE, Mode::Animation).as_deref(), Some("mode.basculer"));
    // 3. Une touche de SHOT que l'utilisateur prend à une autre commande, avec « Assigner » : la
    //    commande qui l'avait en perd la touche (aucun conflit), dans les deux jeux.
    let user = user_text(&sbp(), "outil.main", "Ctrl+Maj+V");
    for id in PresetId::ALL {
        let map = Keymap::build(id, Some(&user));
        assert_eq!(press(&map, Key::V, ctrl_shift, Mode::Animation).as_deref(), Some("outil.main"), "{id:?}");
        assert!(map.labels_of("edition.coller_a_la_tete").is_empty(), "{id:?}");
        assert_eq!(map.origin_of("edition.coller_a_la_tete"), Origin::User, "{id:?}");
        assert!(map.warnings.is_empty(), "{id:?} : {:#?}", map.warnings);
    }
    // Limite connue : le retrait écrit par « Assigner » est un choix du fichier personnel ; « Raccourci
    // par défaut » (liste des raccourcis) rend sa touche à la commande, dans les deux jeux.
    let map = Keymap::build(PresetId::Shot, Some(&user));
    let undo = map
        .edited_user_file(Some(&user), &[("edition.coller_a_la_tete".into(), None), ("outil.main".into(), None)])
        .unwrap();
    for id in PresetId::ALL {
        let map = Keymap::build(id, Some(&undo));
        assert_eq!(map.labels_of("edition.coller_a_la_tete"), vec!["Ctrl+Maj+V".to_string()], "{id:?}");
        assert_eq!(map.origin_of("edition.coller_a_la_tete"), Origin::Default, "{id:?}");
        assert_eq!(map.user_overrides(), 0, "{id:?}");
    }
    // 3 ter. Le même vol de touche écrit à la main, sans retirer la touche à l'autre commande : celle
    //    de l'utilisateur l'emporte dans les deux jeux, et une remarque signale le conflit.
    let by_hand = r#"
        version = 1
        [[raccourci]]
        action = "outil.main"
        touches = ["Ctrl+Maj+V"]
        source = "choisi dans SHOT"
    "#;
    for id in PresetId::ALL {
        let map = Keymap::build(id, Some(by_hand));
        assert_eq!(press(&map, Key::V, ctrl_shift, Mode::Animation).as_deref(), Some("outil.main"), "{id:?} : la touche de l'utilisateur l'emporte");
        assert_eq!(map.origin_of("outil.main"), Origin::User, "{id:?}");
        assert!(map.warnings.iter().any(|w| w.contains("conflit") && w.contains("edition.coller_a_la_tete")), "{id:?} : {:?}", map.warnings);
    }
    // 3 bis. Une commande dont le jeu ne touche pas la touche (A, clé précédente) : la touche de
    //    l'utilisateur joue aussi, et le plan précédent n'est pas concerné.
    let user = user_text(&sbp(), "outil.main", "A");
    let map = Keymap::build(PresetId::StoryboardPro, Some(&user));
    assert_eq!(press(&map, Key::A, NONE, Mode::Animation).as_deref(), Some("outil.main"));
    assert!(map.labels_of("cle.precedente").is_empty(), "« Assigner » retire A à la clé précédente");
    assert_eq!(map.labels_of("panneau.precedent"), vec!["Pg préc.".to_string()]);
    // 4. Le retour au raccourci par défaut d'une commande redonne celui du jeu choisi (ici celui de
    //    SHOT, que le jeu ne change pas) : « Coller sur place » n'a plus de touche, « Coller à la tête »
    //    garde la sienne.
    let user = user_text(&sbp(), "edition.coller_sur_place", "F9");
    let map = Keymap::build(PresetId::StoryboardPro, Some(&user));
    assert_eq!(map.labels_of("edition.coller_sur_place"), vec!["F9".to_string()]);
    let reset = map.edited_user_file(Some(&user), &[("edition.coller_sur_place".into(), None)]).unwrap();
    let map = Keymap::build(PresetId::StoryboardPro, Some(&reset));
    assert!(map.labels_of("edition.coller_sur_place").is_empty());
    assert_eq!(map.origin_of("edition.coller_sur_place"), Origin::Default);
    assert_eq!(map.labels_of("edition.coller_a_la_tete"), vec!["Ctrl+Maj+V".to_string()]);
    assert_eq!(map.user_overrides(), 0);
    // Et le fichier personnel remis à zéro redonne exactement la carte du jeu.
    assert_eq!(snapshot(&map), snapshot(&sbp()));
}

#[test]
fn a_verbatim_copy_of_the_default_file_does_not_hide_the_preset() {
    // « Modifier à la main » crée une copie du fichier par défaut : ce ne sont pas des choix de
    // l'utilisateur, le jeu doit continuer à jouer.
    let with_copy = Keymap::build(PresetId::StoryboardPro, Some(DEFAULT_KEYMAP));
    assert_eq!(with_copy.user_overrides(), 0);
    assert_eq!(press(&with_copy, Key::V, Modifiers::COMMAND | Modifiers::SHIFT, Mode::Storyboard).as_deref(), Some("edition.coller_a_la_tete"));
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
    // Les quatre commandes qui gardent la touche de SHOT : Tab, Maj+Tab, Ctrl+K, F4. Depuis le choix
    // d'Anthony (2026-10-19), la nouvelle clé garde F5 et n'est plus « à vérifier ».
    let notes: BTreeSet<&str> = map.notes.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(notes, BTreeSet::from(["mode.basculer", "vue.planches", "aide.palette", "dessin.nouvelle_cle_vide"]));
    for action in ["mode.basculer", "vue.planches", "aide.palette", "dessin.nouvelle_cle_vide"] {
        assert_eq!(map.origin_of(action), Origin::Default, "{action} garde la touche de SHOT");
        assert!(!map.keys_text(action).is_empty(), "{action}");
        assert!(map.verify_note(action).is_some(), "{action}");
    }
    assert!(map.verify_note("dessin.nouvelle_cle").is_none());
    assert!(map.verify_note("edition.copier").is_none());
    assert_eq!(map.notes.len(), 4);
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
