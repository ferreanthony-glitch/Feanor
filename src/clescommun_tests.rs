//! Tests de ce que les trois sortes de clés ont en commun (`clescommun.rs`, passe d'interface du
//! 2026-10-19) : le même menu du clic droit, la même règle de choix, Ctrl+A pour chacune.

use eframe::egui;

use crate::app::App;
use crate::camera::{Camera, CameraKey};
use crate::clescommun::{choose_group, core_entries, KeyCmd, KeyKind, MenuState, CORE};
use crate::fx::{Effect, EffectKind};
use crate::keymap;

fn state(n: usize, has_target: bool, can_paste: bool) -> MenuState {
    MenuState { n, has_target, can_paste }
}

/// Le nom d'une entrée sans le nom de la sorte de clé : « Copier la clé de caméra » devient
/// « Copier la clé ».
fn bare(kind: KeyKind, label: &str) -> String {
    label.replace(kind.noun(true), "clés").replace(kind.noun(false), "clé")
}

#[test]
fn the_menus_of_the_three_kinds_of_keys_list_the_same_entries_in_the_same_order() {
    for st in [state(1, true, true), state(3, true, false), state(0, false, false), state(1, false, true)] {
        let menus: Vec<_> = KeyKind::ALL.iter().map(|k| (*k, core_entries(*k, st))).collect();
        let reference = &menus[0];
        for (kind, entries) in &menus {
            // Les mêmes commandes, dans le même ordre.
            assert_eq!(entries.iter().map(|e| e.cmd).collect::<Vec<_>>(), CORE.to_vec(), "{kind:?}");
            for (e, r) in entries.iter().zip(&reference.1) {
                // Allumées ou grisées pareil.
                assert_eq!(e.enabled, r.enabled, "{kind:?} : {}", e.label);
                // Le même nom, au nom de la sorte de clé près.
                if e.cmd == KeyCmd::ChooseAll {
                    assert!(e.label.starts_with("Choisir toutes les clés"), "{}", e.label);
                } else {
                    assert_eq!(bare(*kind, &e.label), bare(reference.0, &r.label), "{kind:?}");
                }
                // Le même raccourci, et c'est une commande qui existe.
                assert_eq!(e.cmd.action(), r.cmd.action());
                assert!(keymap::action_info(e.cmd.action()).is_some_and(|a| a.implemented), "{}", e.cmd.action());
            }
        }
    }
}

#[test]
fn the_names_say_the_kind_and_the_number() {
    let one = |k| core_entries(k, state(1, true, true)).into_iter().map(|e| e.label).collect::<Vec<_>>();
    assert_eq!(one(KeyKind::Drawing)[0], "Copier la clé");
    assert_eq!(one(KeyKind::Camera)[0], "Copier la clé de caméra");
    assert_eq!(one(KeyKind::Effect)[0], "Copier la clé d'effet");
    assert_eq!(one(KeyKind::Camera)[1], "Coller les clés de caméra");
    assert_eq!(one(KeyKind::Effect)[2], "Coller à la tête de lecture");
    assert_eq!(one(KeyKind::Drawing)[3], "Supprimer la clé");
    let three = core_entries(KeyKind::Effect, state(3, true, true));
    assert_eq!(three[0].label, "Copier les clés d'effet (3)");
    assert_eq!(three[3].label, "Supprimer les clés d'effet (3)");
}

#[test]
fn nothing_to_copy_or_delete_greys_them_and_nothing_to_paste_greys_both_pastes() {
    for kind in KeyKind::ALL {
        let none = core_entries(kind, state(0, false, false));
        let enabled: Vec<bool> = none.iter().map(|e| e.enabled).collect();
        assert_eq!(enabled, vec![false, false, false, false, true], "{kind:?} : rien à viser, rien à coller");
        let some = core_entries(kind, state(2, true, true));
        assert!(some.iter().all(|e| e.enabled), "{kind:?}");
    }
}

#[test]
fn the_three_menus_are_drawn_by_the_shared_code_not_by_hand() {
    // Chaque menu appelle `key_menu_core` avec sa sorte ; aucun ne refait à la main une des
    // entrées communes (c'est comme ça que les trois menus s'étaient écartés).
    let sources = [
        ("camkeys.rs", include_str!("camkeys.rs"), "KeyKind::Camera"),
        ("fxkeys.rs", include_str!("fxkeys.rs"), "KeyKind::Effect"),
        ("storyboard.rs", include_str!("storyboard.rs"), "KeyKind::Drawing"),
    ];
    for (file, text, kind) in sources {
        assert!(text.contains(&format!("key_menu_core(ui, crate::clescommun::{kind})")), "{file} ne passe pas par key_menu_core ({kind})");
        for by_hand in ["\"Coller à la tête de lecture\"", "\"Choisir toutes les clés", "\"Copier la clé", "\"Copier les clés"] {
            assert!(!text.contains(by_hand), "{file} refait à la main {by_hand}");
        }
    }
}

#[test]
fn a_click_chooses_one_ctrl_toggles_shift_takes_the_range() {
    // Pour les trois sortes de clés (ici des entiers).
    assert_eq!(choose_group(&[1, 2], Some(1), &[5], false, None), (vec![5], Some(5)));
    assert_eq!(choose_group(&[1], Some(1), &[5], true, None), (vec![1, 5], Some(5)));
    assert_eq!(choose_group(&[1, 5], Some(5), &[5], true, None), (vec![1], Some(5)));
    assert_eq!(choose_group(&[1], Some(1), &[5], false, Some(vec![1, 2, 3, 4, 5])), (vec![1, 2, 3, 4, 5], Some(1)));
    // Maj sans ancre : un simple clic.
    assert_eq!(choose_group(&[], None, &[5], false, Some(vec![4, 5])), (vec![5], Some(5)));
    // Un losange qui porte deux clés se choisit et se retire d'un bloc.
    assert_eq!(choose_group(&[1], Some(1), &[7, 8], true, None), (vec![1, 7, 8], Some(7)));
    assert_eq!(choose_group(&[1, 7, 8], Some(7), &[7, 8], true, None), (vec![1], Some(7)));
    // Rien cliqué : le choix reste.
    assert_eq!(choose_group::<u8>(&[1], Some(1), &[], false, None), (vec![1], Some(1)));
}

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-cles-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let app = App::sans_fenetre(&ctx, None);
    (ctx, app)
}

#[test]
fn ctrl_a_chooses_all_the_camera_keys_then_all_the_effect_keys_of_the_kind_that_has_the_hand() {
    let (_ctx, mut app) = headless("ctrl-a");
    app.welcome = false;
    let pi = app.cur_panel;
    let cam = Camera::centered(app.doc.width, app.doc.height);
    app.doc.panels[pi].length = app.doc.panels[pi].length.max(20);
    app.doc.panels[pi].camera = [2, 6, 11].iter().map(|f| CameraKey::new(*f, cam)).collect();
    let li = app.cur_layer;
    let mut blur = Effect::new(EffectKind::GaussianBlur);
    let values = blur.values.clone();
    for f in [1, 4, 9, 12] {
        blur.set_key(f, values.clone());
    }
    app.doc.panels[pi].layers[li].fx.push(blur);
    app.select_fx(0);

    // Un clic sur une clé de caméra : Ctrl+A choisit les trois, pas les clés de dessins.
    app.click_cam_key(pi, 6, false, false);
    assert_eq!(app.cam_key_sel.len(), 1);
    app.run_action("edition.tout_selectionner");
    assert_eq!(app.cam_key_sel.len(), 3, "toutes les clés de caméra du plan");
    assert!(app.key_sel.is_empty(), "les clés de dessins ne sont pas touchées");

    // Un clic sur un losange d'effet : Ctrl+A choisit les quatre clés de l'effet.
    app.click_fx_marker(pi, li, 4, false, false);
    assert_eq!(app.fx_key_sel.len(), 1);
    assert!(app.cam_key_sel.is_empty(), "le choix des clés de caméra est lâché");
    app.run_action("edition.tout_selectionner");
    assert_eq!(app.fx_key_sel.len(), 4, "toutes les clés de l'effet choisi");
    assert!(app.cam_key_sel.is_empty());
}

#[test]
fn choosing_all_the_effect_keys_without_a_chosen_effect_says_why() {
    let (_ctx, mut app) = headless("sans-effet");
    app.choose_all_fx_keys();
    assert!(app.fx_key_sel.is_empty());
    assert!(app.status.contains("effet"), "{}", app.status);
}
