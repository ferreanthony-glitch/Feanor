//! Essais de J, K, L du montage (`jkl.rs`) : les vitesses, les touches (`keymap.toml`, contexte « montage »), la lecture en arrière,
//! le son coupé hors vitesse 1, les touches inertes hors du Montage, et le film qui ne bouge pas.

use eframe::egui::{self, Key, Modifiers, RawInput};

use crate::app::{App, Mode};
use crate::document::{Document, Layer, LayerKind};
use crate::jkl::{apres, texte, Touche, PALIERS};
use crate::keymap::{Keymap, ACTIONS, DEFAULT_KEYMAP};
use crate::workspace::Espace;

const NONE: Modifiers = Modifiers::NONE;

fn carte(perso: Option<&str>) -> Keymap {
    Keymap::from_texts(DEFAULT_KEYMAP, perso)
}

/// Un film de trois plans de 10 images, un dessin par image.
fn fixture(nom: &str) -> (App, egui::Context) {
    let dir = std::env::temp_dir().join(format!("shot-jkl-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for i in 0..3 {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(10);
        let mut l = Layer::new("A", LayerKind::Vector);
        for f in 0..10 {
            let c = doc.new_cel(LayerKind::Vector);
            l.set(f, Some(c));
        }
        doc.panels[i].layers.push(l);
    }
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.set_espace(Espace::Montage);
    (app, ctx)
}

fn avancer(app: &mut App, ctx: &egui::Context, secondes: f64) {
    // Une image d'écran toutes les 1/60 s.
    let pas = 1.0 / 60.0;
    // (L'horloge continue d'un appel à l'autre, comme à l'écran.)
    let mut t = app.last_time.unwrap_or(100.0) + pas;
    let n = (secondes / pas).round() as usize;
    for _ in 0..=n {
        ctx.run(RawInput { time: Some(t), ..Default::default() }, |c| app.tick_playback(c));
        t += pas;
    }
}

fn dessins(app: &App) -> Vec<Vec<Option<u64>>> {
    app.doc.panels.iter().map(|p| p.layers[0].frames.clone()).collect()
}

#[test]
fn each_press_picks_the_next_speed_and_k_always_stops() {
    assert_eq!(PALIERS, [1.0, 2.0, 4.0, 8.0]);
    let mut v = 0.0;
    let mut vus = Vec::new();
    for _ in 0..5 {
        v = apres(v, Touche::L);
        vus.push(v);
    }
    assert_eq!(vus, [1.0, 2.0, 4.0, 8.0, 8.0], "L accélère, puis tient la plus grande vitesse");
    assert_eq!(apres(8.0, Touche::K), 0.0);
    assert_eq!(apres(0.0, Touche::J), -1.0);
    assert_eq!(apres(-1.0, Touche::J), -2.0);
    assert_eq!(apres(-4.0, Touche::J), -8.0);
    // L'autre sens : un cran plus lent, puis l'arrêt.
    assert_eq!(apres(-4.0, Touche::L), -2.0);
    assert_eq!(apres(-2.0, Touche::L), -1.0);
    assert_eq!(apres(-1.0, Touche::L), 0.0);
    assert_eq!(apres(2.0, Touche::J), 1.0);
    assert_eq!(apres(1.0, Touche::J), 0.0);
    assert_eq!(texte(-2.0), "Lecture en arrière, x2.");
    assert_eq!(texte(0.0), "Lecture arrêtée.");
}

#[test]
fn the_keys_are_in_the_file_only_in_the_montage_and_take_nothing_from_anyone() {
    let map = carte(None);
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    for (action, touche) in [("montage.lecture_arriere", "J"), ("montage.lecture_arret", "K"), ("montage.lecture_avant", "L")] {
        assert!(ACTIONS.iter().any(|a| a.id == action && a.implemented), "{action}");
        assert_eq!(map.keys_of(action), vec![touche.to_string()], "{action}");
        assert!(map.montage_only(action), "{action} ne joue que dans le Montage");
    }
    let (j, k, l) = (Key::J, Key::K, Key::L);
    for mode in [Mode::Storyboard, Mode::Animation] {
        // Dans le Montage : la lecture.
        assert_eq!(map.find_in(j, Some(j), NONE, mode, true), Some("montage.lecture_arriere"));
        assert_eq!(map.find_in(k, Some(k), NONE, mode, true), Some("montage.lecture_arret"));
        assert_eq!(map.find_in(l, Some(l), NONE, mode, true), Some("montage.lecture_avant"));
        // Ailleurs : rien, les touches restent libres pour ce qu'on voudra y mettre.
        for key in [j, k, l] {
            assert_eq!(map.find_in(key, Some(key), NONE, mode, false), None);
            assert_eq!(map.find(key, Some(key), NONE, mode), None);
        }
    }
    // Avec un modificateur, rien ne change : Ctrl+L, Alt+L, Maj+L gardent leur sens, dans le Montage aussi.
    let ctrl = Modifiers { ctrl: true, command: true, ..NONE };
    assert_eq!(map.find_in(l, Some(l), ctrl, Mode::Storyboard, true), Some("calque.pelure"));
}

#[test]
fn a_personal_l_keeps_its_meaning_outside_the_montage_and_the_montage_says_so() {
    // Le fichier d'Anthony : L est la sélection. Dans le Montage la lecture passe devant ; partout ailleurs, sa touche.
    let perso = "version = 1\n[[raccourci]]\naction = \"outil.selection\"\ntouches = [\"L\"]\n";
    let map = carte(Some(perso));
    let l = Key::L;
    assert_eq!(map.find_in(l, Some(l), NONE, Mode::Storyboard, false), Some("outil.selection"));
    assert_eq!(map.find_in(l, Some(l), NONE, Mode::Animation, false), Some("outil.selection"));
    assert_eq!(map.find_in(l, Some(l), NONE, Mode::Storyboard, true), Some("montage.lecture_avant"));
    assert!(map.warnings.iter().all(|w| !w.contains("conflit")), "pas de conflit annoncé à tort : {:?}", map.warnings);
}

#[test]
fn a_shortcut_assigned_to_a_montage_action_stays_a_montage_shortcut() {
    let map = carte(None);
    let changes = map.assign_changes("montage.lecture_avant", "Maj+P", false);
    let text = map.edited_user_file(None, &changes).unwrap();
    assert!(text.contains("contexte = \"montage\""), "{text}");
    let reloaded = carte(Some(&text));
    assert!(reloaded.montage_only("montage.lecture_avant"));
    assert_eq!(reloaded.keys_of("montage.lecture_avant"), vec!["Maj+P".to_string()]);
}

#[test]
fn l_plays_forward_faster_at_each_press_and_k_stops_without_touching_the_film() {
    let (mut app, ctx) = fixture("avant");
    let film = dessins(&app);
    let n = app.history.undo_len();
    app.run_action("montage.lecture_avant");
    assert!(app.playing && app.play_montage && app.play_speed == 1.0);
    avancer(&mut app, &ctx, 0.25);
    let g1 = app.global_frame();
    assert!((4..=8).contains(&g1), "à x1, 24 i/s, un quart de seconde : {g1}");
    app.run_action("montage.lecture_avant");
    assert_eq!(app.play_speed, 2.0);
    let avant = app.global_frame();
    avancer(&mut app, &ctx, 0.25);
    let pas2 = app.global_frame() - avant;
    assert!((9..=14).contains(&pas2), "à x2 : {pas2} images en un quart de seconde");
    app.run_action("montage.lecture_arret");
    assert!(!app.playing);
    let fixe = app.global_frame();
    avancer(&mut app, &ctx, 0.25);
    assert_eq!(app.global_frame(), fixe, "K arrête");
    assert_eq!(dessins(&app), film, "les dessins n'ont pas bougé");
    assert_eq!(app.history.undo_len(), n, "lire n'est pas un changement : aucune étape d'annulation");
}

#[test]
fn j_plays_backward_through_the_plans_and_stops_at_the_start() {
    let (mut app, ctx) = fixture("arriere");
    app.looping = false;
    app.goto_global(25);
    app.run_action("montage.lecture_arriere");
    assert!(app.playing && app.play_speed == -1.0);
    avancer(&mut app, &ctx, 0.2);
    let g = app.global_frame();
    assert!(g < 25 && g >= 18, "recule : {g}");
    assert!(app.cur_panel >= 1, "pas encore au premier plan");
    app.run_action("montage.lecture_arriere");
    assert_eq!(app.play_speed, -2.0);
    avancer(&mut app, &ctx, 2.0);
    assert!(!app.playing, "arrêtée au début du film");
    assert_eq!((app.cur_panel, app.cur_frame), (0, 0));
    assert_eq!(app.play_speed, 1.0);
}

#[test]
fn l_slows_a_backward_play_then_stops_it_and_j_does_the_same_going_forward() {
    let (mut app, _ctx) = fixture("ralentir");
    app.goto_global(20);
    app.run_action("montage.lecture_arriere");
    app.run_action("montage.lecture_arriere");
    assert_eq!(app.play_speed, -2.0);
    app.run_action("montage.lecture_avant");
    assert_eq!(app.play_speed, -1.0);
    app.run_action("montage.lecture_avant");
    assert!(!app.playing);
    app.run_action("montage.lecture_avant");
    app.run_action("montage.lecture_avant");
    assert_eq!(app.play_speed, 2.0);
    app.run_action("montage.lecture_arriere");
    assert_eq!(app.play_speed, 1.0);
    app.run_action("montage.lecture_arriere");
    assert!(!app.playing);
}

#[test]
fn k_held_with_l_or_j_steps_one_frame_at_a_time() {
    let (mut app, ctx) = fixture("image");
    app.goto_global(12);
    // K tenue : la touche est enfoncée pendant que L est pressée.
    let k_enfoncee = RawInput {
        events: vec![egui::Event::Key { key: Key::K, physical_key: Some(Key::K), pressed: true, repeat: false, modifiers: NONE }],
        ..Default::default()
    };
    ctx.run(k_enfoncee, |_| {});
    app.run_action("montage.lecture_avant");
    assert!(!app.playing, "K tenue : pas de lecture");
    assert_eq!(app.global_frame(), 13);
    app.run_action("montage.lecture_avant");
    assert_eq!(app.global_frame(), 14);
    app.run_action("montage.lecture_arriere");
    assert_eq!(app.global_frame(), 13);
    // K relâchée : L lit de nouveau.
    let k_relachee = RawInput {
        events: vec![egui::Event::Key { key: Key::K, physical_key: Some(Key::K), pressed: false, repeat: false, modifiers: NONE }],
        ..Default::default()
    };
    ctx.run(k_relachee, |_| {});
    app.run_action("montage.lecture_avant");
    assert!(app.playing);
}

#[test]
fn the_keys_do_nothing_outside_the_montage() {
    let (mut app, _ctx) = fixture("dehors");
    for espace in [Espace::Storyboard, Espace::Animation, Espace::Vignettes] {
        app.set_espace(espace);
        for action in ["montage.lecture_avant", "montage.lecture_arriere", "montage.lecture_arret"] {
            app.run_action(action);
            assert!(!app.playing, "{action} dans {espace:?}");
        }
    }
}

#[test]
fn the_sound_is_cut_at_any_speed_but_one_and_comes_back_at_one() {
    let (mut app, _ctx) = fixture("son");
    // (Sans sortie audio, l'horloge reste celle de l'ordinateur : on vérifie l'état que le moteur lirait.)
    app.run_action("montage.lecture_avant");
    assert!(!app.audio_clock || app.play_speed == 1.0);
    app.run_action("montage.lecture_avant");
    assert_eq!(app.play_speed, 2.0);
    assert!(!app.audio_clock, "à x2 le son est coupé, l'image suit l'horloge");
    app.run_action("montage.lecture_arriere");
    assert_eq!(app.play_speed, 1.0);
    app.run_action("montage.lecture_arriere");
    assert!(!app.playing && !app.audio_clock);
}

#[test]
fn typing_in_a_text_field_never_plays() {
    // `handle_shortcuts` rend la main avant toute recherche de raccourci quand un champ de texte a le clavier : la lecture ne part pas.
    let src = include_str!("app.rs");
    let i = src.find("let saisie = ctx.wants_keyboard_input()").expect("la garde de saisie");
    let j = src.find("self.keymap.find_in(key, physical, m, self.key_mode()").expect("la recherche de raccourci");
    assert!(i < j, "la garde de saisie précède la recherche des raccourcis");
    assert!(src[i..j].contains("if saisie || self.modal_open()"));
}
