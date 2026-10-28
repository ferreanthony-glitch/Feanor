//! Le mode concentration (`concentration.rs`) : la touche Z cache tous les panneaux et toutes les barres et ne laisse que le
//! dessin et une barre minimale ; la même touche remet tout, exactement comme avant, sans toucher à aucune disposition.
//! Et les touches choisies pour ce chantier (Z, R) sont libres : aucun conflit, dans SHOT comme dans le jeu « Façon
//! Storyboard Pro », et le fichier personnel les remplace comme les autres.

use eframe::egui::{epaint::Shape, Event, Key, Modifiers, PointerButton, Pos2};
use egui_phosphor::regular as ph;

use crate::app::Mode;
use crate::document::Document;
use crate::dock::Pane;
use crate::keymap::{Keymap, PresetId};
use crate::keymap_tests::{default_map, shared_keys};
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

const NONE: Modifiers = Modifiers::NONE;

fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for _ in 0..3 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

fn banc() -> Banc {
    let mut b = banc_taille(film(), false, 1600, 900);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn touche(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: NONE }], 1.0 / 60.0);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: NONE }], 1.0 / 60.0);
}

fn textes(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

fn tous_les_textes(b: &Banc) -> Vec<(String, Pos2)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut out);
    }
    out
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|(t, _)| t.contains(texte))
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------ les touches

#[test]
fn z_and_r_are_free_keys_with_no_conflict_in_either_key_set() {
    for map in [default_map(), Keymap::build(PresetId::Shot, None), Keymap::build(PresetId::StoryboardPro, None)] {
        assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
        assert!(shared_keys(&map).is_empty(), "aucune touche n'est donnée à deux commandes : {:#?}", shared_keys(&map));
        for mode in [Mode::Storyboard, Mode::Animation] {
            assert_eq!(map.find(Key::Z, Some(Key::Z), NONE, mode), Some("vue.concentration"), "Z : mode concentration ({mode:?})");
            assert_eq!(map.find(Key::R, Some(Key::R), NONE, mode), Some("aide.aide_memoire"), "R : aide-mémoire ({mode:?})");
            // Les touches voisines gardent leur sens.
            assert_eq!(map.find(Key::Z, Some(Key::Z), Modifiers::SHIFT, mode), Some("vue.reinit_zoom"));
            assert_eq!(map.find(Key::Z, Some(Key::Z), Modifiers::COMMAND, mode), Some("edition.annuler"));
            assert_eq!(map.find(Key::R, Some(Key::R), Modifiers::COMMAND, mode), map.find(Key::R, Some(Key::R), Modifiers::COMMAND, mode));
        }
        assert_eq!(map.label("vue.concentration"), "Z");
        assert_eq!(map.label("aide.aide_memoire"), "R");
    }
}

#[test]
fn a_personal_keymap_moves_the_two_keys_like_any_other() {
    let user = r#"
        version = 1
        [[raccourci]]
        action = "vue.concentration"
        touches = ["K"]
        [[raccourci]]
        action = "aide.aide_memoire"
        touches = ["J"]
    "#;
    let map = Keymap::build(PresetId::Shot, Some(user));
    assert!(map.warnings.is_empty(), "{:?}", map.warnings);
    assert_eq!(map.find(Key::K, Some(Key::K), NONE, Mode::Storyboard), Some("vue.concentration"));
    assert_eq!(map.find(Key::J, Some(Key::J), NONE, Mode::Storyboard), Some("aide.aide_memoire"));
    assert_eq!(map.find(Key::Z, Some(Key::Z), NONE, Mode::Storyboard), None, "Z est rendue");
    assert_eq!(map.label("vue.concentration"), "K");
}

#[test]
fn the_new_commands_are_real_commands_with_a_block_in_the_file() {
    for id in ["vue.concentration", "aide.aide_memoire", "aide.conseils", "aide.regler_stylet"] {
        let info = crate::keymap::action_info(id).unwrap_or_else(|| panic!("{id} manque dans ACTIONS"));
        assert!(info.implemented, "{id}");
        assert!(default_map().bindings.iter().any(|b| b.action == id), "{id} n'a pas de bloc dans keymap.toml");
    }
}

// ------------------------------------------------------------------------------------ le mode

#[test]
fn the_key_hides_every_panel_and_bar_at_once_and_the_same_key_brings_them_back() {
    let mut b = banc();
    assert!(est_la(&b, " Montage ") && est_la(&b, "Calques"), "au départ : les onglets des espaces et les panneaux sont là");
    let (rect0, dock0) = (b.app.canvas_rect, b.app.dock_layout());
    // Sans latence : la touche agit dans l'image d'écran où elle arrive (les raccourcis sont lus avant les panneaux).
    b.image(vec![Event::Key { key: Key::Z, physical_key: Some(Key::Z), pressed: true, repeat: false, modifiers: NONE }], 1.0 / 60.0);
    assert!(b.app.confort.concentration && b.app.concentration_active());
    assert!(!est_la(&b, " Montage ") && !est_la(&b, " Vignettes "), "plus d'onglets d'espaces : {:?}", tous_les_textes(&b).iter().map(|t| &t.0).collect::<Vec<_>>());
    assert!(b.app.canvas_rect.width() > rect0.width() + 300.0 && b.app.canvas_rect.height() > rect0.height(), "le dessin prend la place : {:?} contre {:?}", b.app.canvas_rect, rect0);
    assert!(b.app.canvas_rect.width() > 1500.0, "presque toute la fenêtre : {:?}", b.app.canvas_rect);
    b.image(vec![Event::Key { key: Key::Z, physical_key: Some(Key::Z), pressed: false, repeat: false, modifiers: NONE }], 1.0 / 60.0);
    b.chauffer(4);
    // Aucune barre ni aucun panneau, mais la barre minimale : l'outil, le plan, annuler, rétablir, la sortie.
    for absent in ["Fichier", "Édition", " Storyboard ", "Calques"] {
        assert!(!est_la(&b, absent), "« {absent} » ne doit plus être peint");
    }
    for present in [b.app.tool.label(), ph::ARROWS_IN, ph::ARROW_COUNTER_CLOCKWISE, ph::ARROW_CLOCKWISE] {
        assert!(est_la(&b, present), "la barre minimale montre « {present} »");
    }
    // La même touche remet tout.
    touche(&mut b, Key::Z);
    b.chauffer(6);
    assert!(!b.app.confort.concentration);
    assert!(est_la(&b, " Montage ") && est_la(&b, "Calques"));
    assert!(b.app.dock_layout().close_to(&dock0), "les panneaux sont revenus à leur place");
    assert!((b.app.canvas_rect.width() - rect0.width()).abs() < 2.0 && (b.app.canvas_rect.height() - rect0.height()).abs() < 2.0, "le dessin aussi : {:?} contre {rect0:?}", b.app.canvas_rect);
}

#[test]
fn focus_restores_the_exact_layout_even_after_the_user_arranged_panels() {
    let mut b = banc();
    // L'utilisateur a rangé ses panneaux : un panneau de plus, un onglet choisi.
    b.app.show_pane(Pane::History);
    b.app.show_pane(Pane::Comments);
    b.chauffer(6);
    let espace = b.app.espace;
    let (dock, courant, retenu) = (b.app.dock_layout(), b.app.current_layout(), b.app.settings.layout_of(espace).cloned());
    for _ in 0..3 {
        touche(&mut b, Key::Z);
        b.chauffer(10);
        assert!(b.app.concentration_active());
        // Pendant le mode, rien de la disposition ne bouge ni ne s'écrit.
        assert!(b.app.dock_layout().close_to(&dock), "le dock n'est pas touché");
        assert_eq!(b.app.settings.layout_of(espace).cloned(), retenu, "la disposition retenue n'est pas réécrite");
        touche(&mut b, Key::Z);
        b.chauffer(6);
        assert!(!b.app.concentration_active());
        assert!(b.app.dock_layout().close_to(&dock), "revenu exactement");
        assert_eq!(b.app.current_layout().dock_layout().close_to(&courant.dock_layout()), true);
        assert_eq!(b.app.settings.layout_of(espace).cloned(), retenu);
    }
    assert!(b.app.pane_shown(Pane::History) && b.app.pane_shown(Pane::Comments));
}

#[test]
fn the_other_workspaces_keep_their_own_layouts_while_focus_is_on() {
    let mut b = banc();
    let avant: Vec<_> = Espace::TOUS.iter().map(|e| b.app.settings.layout_of(*e).cloned()).collect();
    touche(&mut b, Key::Z);
    b.chauffer(8);
    // Les onglets d'espaces sont cachés mais les actions (Alt+M…) marchent : on change d'espace en restant concentré.
    b.app.set_espace(Espace::Montage);
    b.chauffer(8);
    assert!(b.app.concentration_active() && b.app.espace == Espace::Montage);
    b.app.set_espace(Espace::Storyboard);
    touche(&mut b, Key::Z);
    b.chauffer(8);
    assert!(!b.app.concentration_active() && !est_la(&b, ph::ARROWS_IN), "la barre minimale est partie");
    // Aucune disposition n'est devenue « vide » : celles qui existaient sont encore là, et le dock est complet.
    for (e, l) in Espace::TOUS.iter().zip(avant) {
        if l.is_some() {
            assert!(b.app.settings.layout_of(*e).is_some(), "{e:?}");
        }
    }
    assert!(b.app.pane_shown(Pane::Layers), "le panneau Calques est revenu");
}

#[test]
fn the_exit_button_of_the_minimal_bar_works_and_so_does_the_menu_command() {
    let mut b = banc();
    b.app.run_action("vue.concentration");
    b.chauffer(6);
    assert!(b.app.concentration_active());
    let sortie = tous_les_textes(&b).into_iter().find(|(t, _)| t == ph::ARROWS_IN).expect("le bouton de sortie").1;
    clic(&mut b, sortie);
    assert!(!b.app.concentration_active(), "le bouton remet tout");
    assert!(est_la(&b, " Montage "));
}

#[test]
fn focus_waits_for_a_project_and_never_hides_the_welcome_card() {
    let mut b = banc();
    b.app.welcome = true;
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    touche(&mut b, Key::Z);
    b.chauffer(2);
    assert!(!b.app.confort.concentration, "rien à garder sans projet");
    assert!(b.app.status.contains("concentration"), "{}", b.app.status);
    assert!(est_la(&b, "Nouveau projet"));
}

#[test]
fn z_is_text_while_typing_in_a_field() {
    let mut b = banc();
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    // Un clic dans le champ de recherche de la fenêtre Préférences : il a le focus clavier.
    let champ = tous_les_textes(&b).into_iter().find(|(t, _)| t.contains("Chercher un réglage…")).expect("le champ de recherche").1;
    clic(&mut b, champ);
    assert!(b.ctx.wants_keyboard_input(), "le champ a le focus");
    b.image(vec![Event::Key { key: Key::Z, physical_key: Some(Key::Z), pressed: true, repeat: false, modifiers: NONE }, Event::Text("z".into())], 1.0 / 60.0);
    b.image(vec![Event::Key { key: Key::Z, physical_key: Some(Key::Z), pressed: false, repeat: false, modifiers: NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert!(!b.app.confort.concentration, "la touche est du texte dans un champ");
    assert_eq!(crate::prefsui::recherche(&b.ctx), "z");
}

#[test]
fn the_idle_focus_screen_asks_for_no_repaint() {
    let mut b = banc();
    touche(&mut b, Key::Z);
    b.chauffer(8);
    crate::perf_tests::se_reposer(&mut b, 200);
    let repos: Vec<std::time::Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "au repos, le mode concentration ne fait pas repeindre : {repos:?}");
}
