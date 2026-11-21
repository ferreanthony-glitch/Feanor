//! Essais de la partie 206 (`bordscles.rs`) : la fin d'un plan et le bord entre deux clés d'un calque (Anthony, 2026-11-21 : « Feanor
//! échoue à modifier la durée d'un shot sans bouger les clés d'images, ou bouger des clés d'images sans bouger les suivantes »).
//!
//! Les gestes passent par les mêmes appels que l'écran : `begin_handle_drag` / `drag_handle` / `end_handle_drag` (la poignée de durée
//! de la timeline, le bord entre deux clés dans la timeline du Storyboard, le bord droit d'un dessin dans celle de l'Animation),
//! `begin_edge_drag` / `update_edge_drag` / `end_montage_drag` (le bord de fin d'un plan dans le montage). Le contexte d'essai n'a
//! aucune touche tenue : Ctrl et Maj passent par les deux boutons du stylet (`cles.decaler_suite`, `cles.etirer_proportion`), qui
//! en tiennent lieu ; un essai les tient vraiment au clavier, par la vraie fenêtre sans écran (`Banc`).

use eframe::egui;

use crate::app::App;
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::handles::Handle;
use crate::montagekeys::KeyRef;

fn app_avec(doc: Document, name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-bords-cles-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

/// Un calque nommé `name` aux expositions `spans` (début, fin exclue), chacune son dessin.
fn calque(doc: &mut Document, name: &str, spans: &[(usize, usize)]) -> (Layer, Vec<CelId>) {
    let mut l = Layer::new(name, LayerKind::Vector);
    let mut cels = Vec::new();
    for &(s, e) in spans {
        let c = doc.new_cel(LayerKind::Vector);
        for f in s..e {
            l.set(f, Some(c));
        }
        cels.push(c);
    }
    (l, cels)
}

/// Un plan de 24 images, deux calques : « A » à quatre clés 0-6, 6-12, 12-18, 18-24 ; « B » à deux clés 0-12, 12-24. Un second plan
/// de 10 images derrière.
fn deux_calques(name: &str) -> App {
    app_avec(doc_deux_calques(), name)
}

fn doc_deux_calques() -> Document {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let (a, _) = calque(&mut doc, "A", &[(0, 6), (6, 12), (12, 18), (18, 24)]);
    let (b, _) = calque(&mut doc, "B", &[(0, 12), (12, 24)]);
    doc.panels[0].layers.extend([a, b]);
    doc.panels[1].set_length(10);
    doc
}

/// Les (début, fin) des expositions dessinées du calque `li` du plan 0.
fn spans(app: &App, li: usize) -> Vec<(usize, usize)> {
    crate::handles::runs(&app.doc.panels[0].layers[li]).into_iter().filter(|r| r.2.is_some()).map(|r| (r.0, r.1)).collect()
}

/// Les durées des clés dessinées du calque `li` du plan 0.
fn durees(app: &App, li: usize) -> Vec<usize> {
    spans(app, li).iter().map(|(s, e)| e - s).collect()
}

fn tirer_la_fin(app: &mut App, to: usize) {
    assert!(app.begin_handle_drag(Handle::PlanEnd));
    app.drag_handle(to, to);
    app.end_handle_drag();
}

fn tirer_le_bord(app: &mut App, layer: usize, at: usize, to: usize) {
    assert!(app.begin_handle_drag(Handle::Boundary { layer, at }));
    app.drag_handle(to, to);
    app.end_handle_drag();
}

// ------------------------------------------------------------------ Règle 1 : la fin du plan

#[test]
fn shortening_a_plan_only_shortens_the_last_key_of_each_layer() {
    let mut app = deux_calques("raccourcir");
    let n = app.history.undo_len();
    tirer_la_fin(&mut app, 20);
    assert_eq!(app.doc.panels[0].frame_count(), 20);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 20)], "seule la dernière clé de A raccourcit");
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 20)], "seule la dernière clé de B raccourcit");
    assert_eq!(app.history.undo_len(), n + 1, "une étape");
}

#[test]
fn shortening_a_plan_stops_one_frame_after_the_start_of_the_last_key() {
    // Plus court que le début de la dernière clé de A (18) : la fin s'arrête à 19, aucune clé n'est coupée ni rangée.
    let mut app = deux_calques("butee");
    tirer_la_fin(&mut app, 10);
    assert_eq!(app.doc.panels[0].frame_count(), 19, "butée : début de la dernière clé + 1 image");
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 19)]);
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 19)]);
    assert!(app.status.contains("butée") || app.status.contains("Butée"), "{}", app.status);
}

#[test]
fn lengthening_a_plan_only_lengthens_the_last_key_of_each_layer() {
    let mut app = deux_calques("allonger");
    tirer_la_fin(&mut app, 30);
    assert_eq!(app.doc.panels[0].frame_count(), 30);
    assert_eq!(durees(&app, 0), vec![6, 6, 6, 12]);
    assert_eq!(durees(&app, 1), vec![12, 18]);
}

// ------------------------------------------------------------------ Règle 2 : le bord entre deux clés

#[test]
fn dragging_the_cut_between_two_keys_changes_only_those_two_keys() {
    let mut app = deux_calques("bord");
    let n = app.history.undo_len();
    // La coupure entre la clé 2 (6-12) et la clé 3 (12-18) de A tirée de 12 à 15.
    tirer_le_bord(&mut app, 0, 12, 15);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 15), (15, 18), (18, 24)], "la clé 4 et la fin ne bougent pas");
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 24)], "l'autre calque ne bouge pas");
    assert_eq!(app.doc.panels[0].frame_count(), 24, "le plan ne bouge pas");
    assert_eq!(app.history.undo_len(), n + 1);
    // Vers la gauche : la clé 3 prend du temps à la clé 2.
    tirer_le_bord(&mut app, 0, 15, 8);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 8), (8, 18), (18, 24)]);
}

#[test]
fn the_cut_stops_when_a_key_would_have_less_than_one_frame() {
    let mut app = deux_calques("bord-butee");
    tirer_le_bord(&mut app, 0, 12, 30);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 17), (17, 18), (18, 24)], "la clé d'après garde une image");
    tirer_le_bord(&mut app, 0, 17, 0);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 7), (7, 18), (18, 24)], "la clé d'avant garde une image");
}

#[test]
fn the_right_edge_of_a_drawing_in_the_animation_timeline_moves_only_the_cut() {
    let mut app = deux_calques("exposition");
    assert!(app.begin_handle_drag(Handle::Exposure { layer: 0, start: 6, end: 12 }));
    app.drag_handle(14, 14);
    app.end_handle_drag();
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 14), (14, 18), (18, 24)]);
}

#[test]
fn the_status_bar_shows_the_key_its_new_length_and_the_change_while_dragging() {
    let mut app = deux_calques("barre");
    assert!(app.begin_handle_drag(Handle::Boundary { layer: 0, at: 12 }));
    app.drag_handle(16, 16);
    assert!(app.status.starts_with("Clé 2 : 10 i (+4)"), "{}", app.status);
    app.end_handle_drag();
}

#[test]
fn several_chosen_layers_move_the_same_cut_by_the_same_amount_each_with_its_limits() {
    let mut app = deux_calques("plusieurs");
    let (panel, a, b) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id, app.doc.panels[0].layers[1].id);
    // Choisis : la clé 2 de A (6-12) et la clé 1 de B (0-12) : la même coupure, à 12.
    app.key_sel = vec![KeyRef { panel, layer: a, start: 6 }, KeyRef { panel, layer: b, start: 0 }];
    let n = app.history.undo_len();
    tirer_le_bord(&mut app, 0, 12, 16);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 16), (16, 18), (18, 24)]);
    assert_eq!(spans(&app, 1), vec![(0, 16), (16, 24)], "même écart sur B");
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    // Chaque calque a ses butées : +10 depuis 16, A s'arrête à 17, B à 23.
    tirer_le_bord(&mut app, 0, 16, 26);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 17), (17, 18), (18, 24)]);
    assert_eq!(spans(&app, 1), vec![(0, 23), (23, 24)]);
    app.run_action("edition.annuler");
    assert_eq!(spans(&app, 1), vec![(0, 16), (16, 24)]);
    app.run_action("edition.annuler");
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 24)]);
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 24)]);
}

#[test]
fn a_locked_chosen_layer_is_left_aside() {
    let mut app = deux_calques("verrou");
    let (panel, a, b) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id, app.doc.panels[0].layers[1].id);
    app.doc.panels[0].layers[1].locked = true;
    app.key_sel = vec![KeyRef { panel, layer: a, start: 6 }, KeyRef { panel, layer: b, start: 0 }];
    tirer_le_bord(&mut app, 0, 12, 14);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 14), (14, 18), (18, 24)]);
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 24)], "le calque verrouillé ne bouge pas");
    assert!(app.status.contains("verrouillé"), "{}", app.status);
}

// ------------------------------------------------------------------ Ctrl et Maj (par les boutons du stylet), annulation

#[test]
fn shift_stretches_every_key_of_every_layer_in_proportion_and_the_sum_is_exact() {
    let mut app = deux_calques("proportion");
    app.run_action("cles.etirer_proportion");
    assert!(app.settings.cles_etirer_proportion);
    tirer_la_fin(&mut app, 36);
    assert_eq!(app.doc.panels[0].frame_count(), 36);
    assert_eq!(durees(&app, 0), vec![9, 9, 9, 9], "×1,5");
    assert_eq!(durees(&app, 1), vec![18, 18]);
    // Raccourcir à 10 : arrondi à l'image, chaque clé au moins une image, la somme juste.
    let mut app = deux_calques("proportion-court");
    app.run_action("cles.etirer_proportion");
    tirer_la_fin(&mut app, 10);
    assert_eq!(app.doc.panels[0].frame_count(), 10);
    assert_eq!(durees(&app, 0).iter().sum::<usize>(), 10);
    assert_eq!(durees(&app, 1), vec![5, 5]);
    assert!(durees(&app, 0).iter().all(|&d| d >= 2), "{:?}", durees(&app, 0));
    // Plus court que le nombre de clés : chacune garde une image (butée à 4).
    tirer_la_fin(&mut app, 1);
    assert_eq!(app.doc.panels[0].frame_count(), 4);
    assert_eq!(durees(&app, 0), vec![1, 1, 1, 1]);
    assert!(app.status.contains("Maj"), "{}", app.status);
}

#[test]
fn ctrl_on_the_cut_shifts_the_rest_of_the_layer() {
    let mut app = deux_calques("ctrl-bord");
    app.run_action("cles.decaler_suite");
    assert!(app.settings.cles_decaler_suite);
    // La clé 2 passe de 6 à 3 images : toute la suite recule de 3 ; A était tenu jusqu'au bout, sa dernière clé absorbe.
    tirer_le_bord(&mut app, 0, 12, 9);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 9), (9, 15), (15, 24)]);
    assert_eq!(app.doc.panels[0].frame_count(), 24);
    assert!(app.status.contains("la suite du calque se décale"), "{}", app.status);
}

#[test]
fn ctrl_on_the_plan_end_still_slides_the_keys() {
    let mut app = deux_calques("ctrl-fin");
    app.run_action("cles.decaler_suite");
    tirer_la_fin(&mut app, 30);
    assert_eq!(spans(&app, 1), vec![(6, 18), (18, 30)], "les clés suivent la poignée (partie 123)");
}

#[test]
fn the_two_pen_buttons_are_exclusive_and_written_only_when_lit() {
    let mut app = deux_calques("boutons");
    let defaut = toml::to_string(&app.settings).unwrap();
    assert!(!defaut.contains("cles_decaler_suite") && !defaut.contains("cles_etirer_proportion"), "rien d'écrit d'origine");
    app.run_action("cles.decaler_suite");
    app.run_action("cles.etirer_proportion");
    assert!(app.settings.cles_etirer_proportion && !app.settings.cles_decaler_suite, "un seul à la fois");
    let ecrit = toml::to_string(&app.settings).unwrap();
    assert!(ecrit.contains("cles_etirer_proportion = true"), "{ecrit}");
    let relu: crate::settings::Settings = toml::from_str(&ecrit).unwrap();
    assert!(relu.cles_etirer_proportion);
    app.run_action("cles.etirer_proportion");
    assert!(!app.settings.cles_etirer_proportion);
    // Les deux boutons sont dans la barre de la timeline de l'Animation et dans celle du montage, à bascule.
    for b in [crate::barres::Barre::Timeline, crate::barres::Barre::Montage] {
        for id in ["cles.decaler_suite", "cles.etirer_proportion"] {
            let c = b.cherche(id).unwrap_or_else(|| panic!("{id} dans {}", b.nom()));
            assert_eq!(c.genre, crate::barres::Genre::Etat);
        }
    }
}

#[test]
fn real_ctrl_on_the_keyboard_shifts_the_rest_too() {
    let mut b = crate::perf_tests::banc_taille(doc_deux_calques(), false, 1280, 800);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.touches = egui::Modifiers { ctrl: true, command: true, ..egui::Modifiers::NONE };
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.begin_handle_drag(Handle::Boundary { layer: 0, at: 12 }));
    b.app.drag_handle(9, 9);
    b.app.end_handle_drag();
    b.touches = egui::Modifiers::NONE;
    let s: Vec<(usize, usize)> = crate::handles::runs(&b.app.doc.panels[0].layers[0]).into_iter().filter(|r| r.2.is_some()).map(|r| (r.0, r.1)).collect();
    assert_eq!(s, vec![(0, 6), (6, 9), (9, 15), (15, 24)]);
}

#[test]
fn a_plan_end_gesture_is_one_undo_step_and_escape_restores_everything() {
    let mut app = deux_calques("annuler-fin");
    let n = app.history.undo_len();
    tirer_la_fin(&mut app, 21);
    assert_eq!(app.history.undo_len(), n + 1);
    app.run_action("edition.annuler");
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 24)]);
    assert_eq!(app.doc.panels[0].frame_count(), 24);
    // Échap pendant un bord : rien n'est enregistré.
    assert!(app.begin_handle_drag(Handle::Boundary { layer: 0, at: 12 }));
    app.drag_handle(15, 15);
    app.cancel_handle_drag();
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 24)]);
    assert_eq!(app.history.undo_len(), n);
}

#[test]
fn the_plan_end_in_the_montage_follows_the_same_rule() {
    let mut app = deux_calques("montage-fin");
    app.doc.storyboard_locked = false;
    let fait = app.doc.poser_fin_du_plan(0, 5, false);
    assert_eq!(fait.duree, 19, "butée");
    assert_eq!(fait.butee.map(|b| b.0), Some(19));
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 19)]);
}

// ------------------------------------------------------------------ + et −

#[test]
fn plus_and_minus_add_or_remove_a_frame_to_the_chosen_keys_and_shift_the_rest() {
    let mut app = deux_calques("plus");
    let (panel, a, b) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id, app.doc.panels[0].layers[1].id);
    app.key_sel = vec![KeyRef { panel, layer: a, start: 0 }, KeyRef { panel, layer: a, start: 6 }, KeyRef { panel, layer: b, start: 12 }];
    let n = app.history.undo_len();
    app.run_action("cles.image_plus");
    assert_eq!(app.history.undo_len(), n + 1, "une étape");
    assert!(app.status.starts_with("3 clés allongées d'une image sur 2 calques"), "{}", app.status);
    assert_eq!(&spans(&app, 0)[..3], &[(0, 7), (7, 14), (14, 20)], "la suite se décale");
    assert_eq!(app.doc.panels[0].frame_count(), 25, "le plan s'allonge : la dernière clé de B dépasse");
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 25)]);
    // Le choix suit les clés : − les ramène.
    app.run_action("cles.image_moins");
    assert!(app.status.starts_with("3 clés raccourcies d'une image sur 2 calques"), "{}", app.status);
    assert_eq!(&spans(&app, 0)[..2], &[(0, 6), (6, 12)]);
    app.run_action("edition.annuler");
    app.run_action("edition.annuler");
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 24)]);
    assert_eq!(app.doc.panels[0].frame_count(), 24);
}

#[test]
fn minus_keeps_at_least_one_frame_and_says_so() {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(10);
    let (l, _) = calque(&mut doc, "A", &[(0, 1), (1, 10)]);
    doc.panels[0].layers.push(l);
    let mut app = app_avec(doc, "moins");
    let (panel, a) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id);
    app.key_sel = vec![KeyRef { panel, layer: a, start: 0 }];
    let n = app.history.undo_len();
    app.run_action("cles.image_moins");
    assert_eq!(spans(&app, 0), vec![(0, 1), (1, 10)]);
    assert_eq!(app.history.undo_len(), n, "rien n'a changé, pas d'étape");
    assert!(app.status.contains("qu'une image"), "{}", app.status);
}

#[test]
fn ctrl_plus_and_ctrl_minus_are_the_default_shortcuts_and_steal_nothing() {
    use crate::app::Mode;
    use egui::{Key, Modifiers};
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    let f = |k: Key, m: Modifiers, mode: Mode| map.find(k, Some(k), m, mode).map(str::to_string);
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(f(Key::Equals, Modifiers::COMMAND, mode).as_deref(), Some("cles.image_plus"));
        assert_eq!(f(Key::Plus, Modifiers::COMMAND, mode).as_deref(), Some("cles.image_plus"), "le + du pavé numérique");
        assert_eq!(f(Key::Minus, Modifiers::COMMAND, mode).as_deref(), Some("cles.image_moins"));
        assert_eq!(f(Key::Equals, Modifiers::NONE, mode).as_deref(), Some("panneau.ajouter_image"), "+ garde la durée du plan");
        assert_eq!(f(Key::Minus, Modifiers::NONE, mode).as_deref(), Some("panneau.retirer_image"));
    }
    assert_eq!(f(Key::Equals, Modifiers::ALT, Mode::Animation).as_deref(), Some("exposition.allonger"), "Alt + garde l'exposition");
}

// ------------------------------------------------------------------ L'affichage des clés (A à G)

/// Les formes peintes par `habiller` sur une clé de `seg` (images `start` à `end`), dans une passe d'egui à part.
fn formes_de(seg: egui::Rect, start: usize, end: usize, vide: bool, tete: Option<usize>, durees: bool, vignettes: bool) -> (Vec<egui::Shape>, usize) {
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput::default());
    let painter = ctx.layer_painter(egui::LayerId::background());
    let cases = std::cell::RefCell::new(Vec::new());
    let est_vide = move |_: CelId| vide;
    let habit = crate::clesvue::Habit { vide: &est_vide, tete, durees, vignettes: vignettes.then_some(&cases), fine: false };
    let mut doc = Document::with_frame(32, 24, 24);
    let cel = doc.new_cel(LayerKind::Vector);
    crate::clesvue::habiller(&painter, seg, start, end, cel, true, egui::Color32::WHITE, &habit);
    let out = ctx.end_pass();
    (out.shapes.into_iter().map(|c| c.shape).collect(), cases.into_inner().len())
}

fn segments(shapes: &[egui::Shape]) -> Vec<[egui::Pos2; 2]> {
    shapes.iter().filter_map(|s| if let egui::Shape::LineSegment { points, .. } = s { Some(*points) } else { None }).collect()
}

fn textes(shapes: &[egui::Shape]) -> Vec<String> {
    shapes.iter().filter_map(|s| if let egui::Shape::Text(t) = s { Some(t.galley.text().to_string()) } else { None }).collect()
}

#[test]
fn each_key_starts_with_a_dark_cut_and_the_key_under_the_playhead_is_underlined() {
    let seg = egui::Rect::from_min_size(egui::pos2(100.0, 10.0), egui::vec2(120.0, 20.0));
    // B : la coupure, un trait vertical au bord gauche.
    let (s, _) = formes_de(seg, 0, 12, false, None, false, false);
    let lignes = segments(&s);
    assert!(lignes.iter().any(|p| (p[0].x - p[1].x).abs() < 0.01 && (p[0].x - seg.left()).abs() < 1.5), "{lignes:?}");
    assert_eq!(lignes.len(), 1, "un seul trait : la coupure");
    // (F, la clé vide : partie 208, elle n'affiche rien, la timeline ne l'habille pas ; plus de hachures.)
    // G : la tête de lecture dans la clé : un soulignement en bas ; ailleurs, rien.
    let souligne = |s: &[egui::Shape]| s.iter().any(|f| matches!(f, egui::Shape::Rect(r) if (r.rect.bottom() - seg.bottom()).abs() < 0.01 && r.rect.height() <= crate::clesvue::SOULIGNE_H + 0.01));
    assert!(souligne(&formes_de(seg, 0, 12, false, Some(5), false, false).0));
    assert!(!souligne(&formes_de(seg, 0, 12, false, Some(12), false, false).0), "la tête est sur la clé suivante");
}

#[test]
fn the_duration_and_the_thumbnail_are_options_shown_only_in_wide_keys() {
    let large = egui::Rect::from_min_size(egui::pos2(100.0, 10.0), egui::vec2(120.0, 20.0));
    let etroite = egui::Rect::from_min_size(egui::pos2(100.0, 10.0), egui::vec2(10.0, 20.0));
    assert!(textes(&formes_de(large, 0, 12, false, None, false, false).0).is_empty(), "C éteinte d'origine");
    assert_eq!(textes(&formes_de(large, 0, 12, false, None, true, false).0), vec!["12 i".to_string()]);
    assert!(textes(&formes_de(etroite, 0, 12, false, None, true, false).0).is_empty(), "trop étroite");
    assert_eq!(formes_de(large, 0, 12, false, None, false, false).1, 0, "A éteinte d'origine");
    assert_eq!(formes_de(large, 0, 12, false, None, false, true).1, 1);
    assert_eq!(formes_de(etroite, 0, 12, false, None, false, true).1, 0, "trop étroite");
}

#[test]
fn the_two_display_options_are_off_by_default_and_written_only_when_ticked() {
    let s = crate::settings::Settings::default();
    assert!(!s.cles_vignettes && !s.cles_durees);
    let texte = toml::to_string(&s).unwrap();
    assert!(!texte.contains("cles_vignettes") && !texte.contains("cles_durees"));
    // D'anciens réglages (sans ces clés) se relisent éteints.
    let ancien: crate::settings::Settings = toml::from_str("").unwrap();
    assert!(!ancien.cles_vignettes && !ancien.cles_durees);
    let mut s = s;
    s.cles_durees = true;
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("cles_durees = true") && !texte.contains("cles_vignettes"), "{texte}");
}

#[test]
fn while_dragging_a_cut_the_old_place_is_a_dotted_ghost_and_a_bubble_says_the_new_length() {
    let mut app = deux_calques("fantome");
    assert!(app.begin_handle_drag(Handle::Boundary { layer: 0, at: 12 }));
    app.drag_handle(16, 16);
    assert_eq!(app.bulle_geste.as_deref(), Some("Clé 2 : 10 i (+4)"));
    // Les clés qui ont changé de place : la 2 (6-12) et la 3 (12-18) ; la 1 et la 4 n'ont pas de fantôme.
    let d = app.handle_drag.as_ref().unwrap();
    assert_eq!(crate::clesvue::fantomes(&d.before_panel().layers[0], &app.doc.panels[0].layers[0]), vec![(6, 12), (12, 18)]);
    assert!(crate::clesvue::fantomes(&d.before_panel().layers[1], &app.doc.panels[0].layers[1]).is_empty());
    app.end_handle_drag();
    // La fin du plan aussi : « Durée du plan : … (−4 i) ».
    assert!(app.begin_handle_drag(Handle::PlanEnd));
    app.drag_handle(20, 20);
    assert!(app.bulle_geste.as_deref().is_some_and(|b| b.starts_with("Durée du plan") && b.ends_with("(-4 i)")), "{:?}", app.bulle_geste);
    app.end_handle_drag();
}

#[test]
fn the_storyboard_timeline_paints_cuts_and_the_bubble_through_the_real_window() {
    // (Des dessins avec un trait : depuis la partie 208, une clé vide n'affiche rien, ni sa durée.)
    let mut doc = doc_deux_calques();
    let cels: Vec<CelId> = doc.panels[0].layers.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect();
    for c in cels {
        un_trait(&mut doc, c);
    }
    let mut b = crate::perf_tests::banc_taille(doc, false, 1600, 900);
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.settings.cles_durees = true;
    b.garder_textes = true;
    b.chauffer(4);
    // C allumée : des durées s'écrivent dans les clés assez larges (selon le zoom de la bande).
    let avec = b.textes.iter().filter(|t| t.ends_with(" i") && t.trim_end_matches(" i").parse::<usize>().is_ok()).count();
    b.app.settings.cles_durees = false;
    b.chauffer(2);
    let sans = b.textes.iter().filter(|t| t.ends_with(" i") && t.trim_end_matches(" i").parse::<usize>().is_ok()).count();
    assert!(avec > sans, "{avec} durées allumée, {sans} éteinte");
    // E : la bulle du geste se peint près du pointeur (dans une passe à part : le banc n'a pas de bouton tenu, et la timeline lâche
    // le geste dès que le bouton n'est plus enfoncé).
    assert!(b.app.begin_handle_drag(Handle::Boundary { layer: 0, at: 12 }));
    b.app.drag_handle(16, 16);
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput { events: vec![egui::Event::PointerMoved(egui::pos2(600.0, 500.0))], ..Default::default() });
    b.app.bulle_du_geste(&ctx);
    let _ = ctx.end_pass();
    // (Une zone d'egui se mesure à sa première passe et se montre à la suivante.)
    ctx.begin_pass(egui::RawInput { events: vec![egui::Event::PointerMoved(egui::pos2(601.0, 500.0))], ..Default::default() });
    b.app.bulle_du_geste(&ctx);
    let out = ctx.end_pass();
    let mut vus = Vec::new();
    for c in out.shapes {
        if let egui::Shape::Text(t) = c.shape {
            vus.push(t.galley.text().to_string());
        }
    }
    assert!(vus.iter().any(|t| t == "Clé 2 : 10 i (+4)"), "{vus:?}");
    b.app.end_handle_drag();
}

// ------------------------------------------------------------------ Les écrans en PNG (pour Anthony)

/// Un dessin vectoriel de traits simples (chaque trait : ses points, en pixels de l'image du plan, 1280 × 720).
fn dessin(doc: &mut Document, traits: &[&[(f32, f32)]], couleur: [u8; 3]) -> CelId {
    use crate::vector::{Element, VPoint};
    let mut cel = crate::document::Cel::new(doc.width, doc.height, LayerKind::Vector);
    // (Les points sont donnés dans le cadre de la caméra, centré sur la feuille.)
    let o = doc.frame_rect().min;
    for t in traits {
        // (Des points serrés : le trait suit la ligne brisée.)
        let mut points = Vec::new();
        for w in t.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            let n = ((x1 - x0).hypot(y1 - y0) / 6.0).ceil().max(1.0) as usize;
            for k in 0..n {
                let u = k as f32 / n as f32;
                points.push(VPoint { x: o.x + x0 + (x1 - x0) * u, y: o.y + y0 + (y1 - y0) * u, r: 9.0 });
            }
        }
        let &(xe, ye) = t.last().unwrap();
        points.push(VPoint { x: o.x + xe, y: o.y + ye, r: 9.0 });
        cel.vector.as_mut().expect("vectoriel").elements.push(Element::Stroke { points, color: couleur, opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() });
    }
    cel.rerender();
    doc.add_cel(cel)
}

/// Un trait dans le dessin `cel` (qui n'est plus une clé vide).
fn un_trait(doc: &mut Document, cel: CelId) {
    use crate::vector::{Element, VPoint};
    let Some(c) = doc.cels.get_mut(&cel) else { return };
    let Some(v) = c.vector.as_mut() else { return };
    if !v.elements.is_empty() {
        return;
    }
    let points = vec![VPoint { x: 4.0, y: 4.0, r: 2.0 }, VPoint { x: 20.0, y: 12.0, r: 2.0 }];
    v.elements.push(Element::Stroke { points, color: [0, 0, 0], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() });
    c.rerender();
}

/// Un cercle en ligne brisée.
fn rond(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    (0..=24).map(|k| k as f32 / 24.0 * std::f32::consts::TAU).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect()
}

/// Un bonhomme en bâtons, la tête en `x`, les bras levés de `bras` pixels.
fn bonhomme(doc: &mut Document, x: f32, bras: f32) -> CelId {
    let tete = rond(x, 230.0, 45.0);
    dessin(
        doc,
        &[&tete, &[(x, 275.0), (x, 470.0)], &[(x - 110.0, 330.0 - bras), (x, 320.0), (x + 110.0, 330.0 - bras)], &[(x - 80.0, 640.0), (x, 470.0), (x + 80.0, 640.0)]],
        [30, 30, 40],
    )
}

/// Un décor : l'horizon, une maison, un arbre.
fn paysage(doc: &mut Document, dx: f32) -> CelId {
    let arbre = rond(1000.0 + dx, 300.0, 90.0);
    dessin(
        doc,
        &[&[(0.0, 520.0), (1280.0, 520.0)], &[(150.0 + dx, 520.0), (150.0 + dx, 330.0), (300.0 + dx, 220.0), (450.0 + dx, 330.0), (450.0 + dx, 520.0)], &[(1000.0 + dx, 520.0), (1000.0 + dx, 390.0)], &arbre],
        [40, 70, 120],
    )
}

/// Un effet : des éclairs en zigzag.
fn eclair(doc: &mut Document, x: f32) -> CelId {
    dessin(doc, &[&[(x, 60.0), (x + 60.0, 160.0), (x - 20.0, 200.0), (x + 70.0, 330.0)], &[(x + 160.0, 90.0), (x + 210.0, 170.0), (x + 150.0, 210.0), (x + 220.0, 300.0)]], [200, 60, 30])
}

/// Un calque nommé `nom` : chaque clé (début, fin exclue) et son dessin.
fn calque_de(nom: &str, cles: &[(usize, usize, CelId)]) -> Layer {
    let mut l = Layer::new(nom, LayerKind::Vector);
    for &(s, e, c) in cles {
        for f in s..e {
            l.set(f, Some(c));
        }
    }
    l
}

/// Le projet des captures : trois plans (48, 36, 30 images), trois calques (Décor, Perso, Effet) aux clés de durées différentes, une
/// clé vide sur Perso (26-32 du plan 1).
fn doc_des_captures() -> Document {
    let mut doc = Document::with_frame(1280, 720, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    // Plan 1.
    let (d1, d2) = (paysage(&mut doc, 0.0), paysage(&mut doc, -120.0));
    let (p1, p2, p3, p5) = (bonhomme(&mut doc, 500.0, 0.0), bonhomme(&mut doc, 560.0, 80.0), bonhomme(&mut doc, 640.0, 160.0), bonhomme(&mut doc, 760.0, -60.0));
    let pv = doc.new_cel(LayerKind::Vector);
    let (e1, e2) = (eclair(&mut doc, 300.0), eclair(&mut doc, 700.0));
    // Plan 2.
    let d3 = paysage(&mut doc, 80.0);
    let (p6, p7, p8) = (bonhomme(&mut doc, 300.0, 40.0), bonhomme(&mut doc, 420.0, 120.0), bonhomme(&mut doc, 600.0, 0.0));
    let e3 = eclair(&mut doc, 900.0);
    // Plan 3.
    let d4 = paysage(&mut doc, -200.0);
    let (p9, p10) = (bonhomme(&mut doc, 900.0, 0.0), bonhomme(&mut doc, 700.0, 100.0));
    let (e4, e5) = (eclair(&mut doc, 200.0), eclair(&mut doc, 500.0));
    let plans: [(usize, Vec<Layer>); 3] = [
        (
            48,
            vec![
                calque_de("Décor", &[(0, 30, d1), (30, 48, d2)]),
                calque_de("Perso", &[(0, 8, p1), (8, 14, p2), (14, 26, p3), (26, 32, pv), (32, 48, p5)]),
                calque_de("Effet", &[(4, 12, e1), (12, 20, e2)]),
            ],
        ),
        (36, vec![calque_de("Décor", &[(0, 36, d3)]), calque_de("Perso", &[(0, 12, p6), (12, 18, p7), (18, 36, p8)]), calque_de("Effet", &[(6, 30, e3)])]),
        (30, vec![calque_de("Décor", &[(0, 30, d4)]), calque_de("Perso", &[(0, 10, p9), (10, 30, p10)]), calque_de("Effet", &[(0, 15, e4), (15, 30, e5)])]),
    ];
    for (pi, (n, layers)) in plans.into_iter().enumerate() {
        doc.panels[pi].layers = layers;
        doc.panels[pi].set_length(n);
    }
    doc
}

fn png(dir: &str, nom: &str, rgba: &[u8], w: u32, h: u32) {
    image::save_buffer(format!("{dir}/{nom}"), rgba, w, h, image::ColorType::Rgba8).unwrap();
}

/// La part `r` de l'écran, à l'échelle 1.
fn decouper(rgba: &[u8], w: u32, h: u32, r: egui::Rect) -> (Vec<u8>, u32, u32) {
    let x0 = r.left().max(0.0).floor() as u32;
    let y0 = r.top().max(0.0).floor() as u32;
    let x1 = (r.right().ceil() as u32).min(w);
    let y1 = (r.bottom().ceil() as u32).min(h);
    let mut out = Vec::new();
    for y in y0..y1 {
        let o = ((y * w + x0) * 4) as usize;
        out.extend_from_slice(&rgba[o..o + ((x1 - x0) * 4) as usize]);
    }
    (out, x1 - x0, y1 - y0)
}

type Banc = crate::perf_tests::Banc;

const DT: f64 = 1.0 / 60.0;

fn blocs_sb() -> Vec<egui::Rect> {
    crate::storyboard::BLOCS_DU_MONTAGE.with(|x| x.borrow().clone())
}

/// La bande de la timeline du Storyboard : de la barre de lecture (ses deux boutons) au bas des pistes de calques.
fn bande_sb(b: &Banc) -> egui::Rect {
    let rows = b.app.montage_rows(300.0);
    let haut = blocs_sb()[0].top() - 2.0 - rows.blocks_top();
    let mut r = egui::Rect::from_min_max(egui::pos2(0.0, haut - 4.0), egui::pos2(b.taille.x, haut + rows.background_top() + 16.0));
    for nom in [crate::barres::BOUTON_DECALER_NOM, crate::barres::BOUTON_PROPORTION_NOM] {
        for q in crate::bulles::vus::ou(nom) {
            r = r.union(q.expand(6.0));
        }
    }
    r.intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, b.taille))
}

fn bouton_g(p: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

/// Prend en `de`, tire de `dx` points en huit pas, et reste enfoncé.
fn prendre_et_tirer(b: &mut Banc, de: egui::Pos2, dx: f32) {
    b.image(vec![egui::Event::PointerMoved(de)], DT);
    b.image(vec![bouton_g(de, true)], DT);
    for k in 1..=8 {
        b.image(vec![egui::Event::PointerMoved(de + egui::vec2(dx * k as f32 / 8.0, 0.0))], DT);
    }
    // (La bulle d'egui se mesure à sa première passe et se montre à la suivante.)
    for _ in 0..4 {
        b.image(vec![egui::Event::PointerMoved(de + egui::vec2(dx, 0.0))], DT);
    }
}

/// Échap pendant le geste, puis le bouton lâché, le pointeur loin.
fn echap_et_lacher(b: &mut Banc, p: egui::Pos2) {
    b.image(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }], DT);
    b.image(vec![bouton_g(p, false)], DT);
    b.image(vec![egui::Event::PointerMoved(egui::pos2(5.0, 5.0))], DT);
    b.chauffer(4);
}

/// Le point du bord à l'abscisse `x` : cherché au curseur ↔, de haut en bas de la bande.
fn trouver_le_bord(b: &mut Banc, x: f32) -> egui::Pos2 {
    let r = bande_sb(b);
    let mut y = r.top();
    while y < r.bottom() {
        b.image(vec![egui::Event::PointerMoved(egui::pos2(x, y))], DT);
        b.image(Vec::new(), DT);
        if b.curseur == egui::CursorIcon::ResizeHorizontal {
            return egui::pos2(x, y + 4.0);
        }
        y += 3.0;
    }
    panic!("aucun bord à x = {x}");
}

/// (Pour regarder : `FEANOR_BORDSCLES_PNG=<dossier>` écrit les écrans de la partie 206 : la timeline du Storyboard au repos, le
/// survol d'un bord de clé, le geste sur ce bord, le geste sur la fin d'un plan sans touche et en proportion, les options A et C, la
/// timeline de l'Animation ; partie 208 : la colonne de gauche à sa largeur d'origine, et dans l'Animation le survol puis le geste sur
/// le bord droit d'un dessin. Chaque écran entier, et la timeline recadrée, à taille réelle.)
#[test]
fn ecrire_les_ecrans_des_bords_des_cles_en_png() {
    use crate::workspace::Espace;
    use egui::{pos2, vec2, Event, Pos2, Rect};
    let Ok(dir) = std::env::var("FEANOR_BORDSCLES_PNG") else { return };
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    std::fs::create_dir_all(&dir).unwrap();
    let mut b = crate::perf_tests::banc_taille(doc_des_captures(), true, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.app.settings.storyboard_calques_replies = false;
    // (Partie 208 : la colonne de gauche à sa largeur d'origine ; la barre de lecture y montre « Décaler la suite » et « Étirer en
    // proportion ».)
    b.app.montage_zoom = 8.0;
    // La tête de lecture au milieu de la clé 14-26 de Perso, plan 1.
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 1, 18);
    b.garder_formes = true;
    b.chauffer(8);
    b.attendre_vignettes();
    b.chauffer(4);
    let (w, h) = (b.taille.x as u32, b.taille.y as u32);
    let ecran_plein = Rect::from_min_size(Pos2::ZERO, b.taille);
    let ppf = b.app.montage_zoom;
    let ecrire = |b: &Banc, nom: &str| {
        let ecran = b.capturer().unwrap();
        png(&dir, &format!("{nom}-ecran.png"), &ecran, w, h);
        let (z, zw, zh) = decouper(&ecran, w, h, bande_sb(b));
        png(&dir, &format!("{nom}-timeline.png"), &z, zw, zh);
    };
    let x_bord = |f: usize| blocs_sb()[0].left() - 1.0 + f as f32 * ppf;

    // a) Au repos (le pointeur loin).
    b.image(vec![Event::PointerMoved(pos2(5.0, 5.0))], DT);
    b.chauffer(4);
    ecrire(&b, "a-repos");
    // b) Le survol du bord entre la clé 2 (8-14) et la clé 3 (14-26) de Perso.
    let p = trouver_le_bord(&mut b, x_bord(14));
    for _ in 0..4 {
        b.image(vec![Event::PointerMoved(p)], DT);
    }
    ecrire(&b, "b-survol-du-bord");
    // c) Le geste sur ce bord, 4 images à droite, pas lâché.
    prendre_et_tirer(&mut b, p, 4.0 * ppf);
    assert!(b.app.handle_drag.is_some(), "le geste du bord est pris");
    ecrire(&b, "c-geste-du-bord");
    echap_et_lacher(&mut b, p + vec2(4.0 * ppf, 0.0));
    // d) La fin du plan 1 (le bord droit de son bloc), 8 images à gauche, sans touche.
    let bl = blocs_sb()[0];
    let q = pos2(bl.right(), bl.center().y);
    prendre_et_tirer(&mut b, q, -8.0 * ppf);
    ecrire(&b, "d-geste-fin-du-plan");
    echap_et_lacher(&mut b, q + vec2(-8.0 * ppf, 0.0));
    // e) La même chose, « Étirer en proportion » allumé.
    b.app.run_action("cles.etirer_proportion");
    b.chauffer(3);
    prendre_et_tirer(&mut b, q, -8.0 * ppf);
    ecrire(&b, "e-geste-fin-du-plan-en-proportion");
    echap_et_lacher(&mut b, q + vec2(-8.0 * ppf, 0.0));
    b.app.run_action("cles.etirer_proportion");
    // f) Les options A (vignette du dessin) et C (« 12 i »), la timeline zoomée.
    b.app.settings.cles_vignettes = true;
    b.app.settings.cles_durees = true;
    b.app.montage_zoom = 22.0;
    b.chauffer(6);
    b.attendre_vignettes();
    b.chauffer(30);
    b.attendre_vignettes();
    b.chauffer(4);
    ecrire(&b, "f-options-vignette-et-duree");
    b.app.settings.cles_vignettes = false;
    b.app.settings.cles_durees = false;
    // g) La timeline de l'Animation (plan 1), au repos ; puis la poignée de durée tirée vers la gauche.
    b.app.set_espace(Espace::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 1, 18);
    b.chauffer(10);
    b.attendre_vignettes();
    b.chauffer(4);
    let ecran = b.capturer().unwrap();
    png(&dir, "g-animation-ecran.png", &ecran, w, h);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().clone());
    let prise = crate::timeline::PRISE_FIN_DU_PLAN.with(|c| c.get());
    let bas = lignes.1.iter().map(|l| l.2).fold(0.0_f32, f32::max);
    let haut = prise.map_or(bas - 200.0, |r| r.top()) - 60.0;
    let zone = Rect::from_min_max(pos2(0.0, haut), pos2(b.taille.x, bas + 30.0)).intersect(ecran_plein);
    let (z, zw, zh) = decouper(&ecran, w, h, zone);
    png(&dir, "g-animation-timeline.png", &z, zw, zh);
    if let Some(r) = prise {
        let q = r.center();
        prendre_et_tirer(&mut b, q, -60.0);
        let ecran = b.capturer().unwrap();
        png(&dir, "g-animation-geste-duree-ecran.png", &ecran, w, h);
        let (z, zw, zh) = decouper(&ecran, w, h, zone);
        png(&dir, "g-animation-geste-duree-timeline.png", &z, zw, zh);
        echap_et_lacher(&mut b, q + vec2(-60.0, 0.0));
    }
    // h) Partie 208 : le bord droit de la clé 8-14 de Perso dans l'Animation, survolé, puis tiré de 3 images (pas lâché).
    b.chauffer(3);
    if let (Some((gauche, cell)), Some((_, haut, bas))) =
        (crate::timeline::GRILLE_PEINTE.with(|g| g.get()), crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().1.iter().copied().find(|l| l.0 == 1)))
    {
        let p = pos2(gauche + 14.0 * cell - 1.0, (haut + bas) * 0.5);
        for _ in 0..4 {
            b.image(vec![Event::PointerMoved(p)], DT);
        }
        let ecran = b.capturer().unwrap();
        png(&dir, "h-animation-survol-du-bord-ecran.png", &ecran, w, h);
        let (z, zw, zh) = decouper(&ecran, w, h, zone);
        png(&dir, "h-animation-survol-du-bord-timeline.png", &z, zw, zh);
        prendre_et_tirer(&mut b, p, 3.0 * cell);
        let ecran = b.capturer().unwrap();
        png(&dir, "h-animation-geste-du-bord-ecran.png", &ecran, w, h);
        let (z, zw, zh) = decouper(&ecran, w, h, zone);
        png(&dir, "h-animation-geste-du-bord-timeline.png", &z, zw, zh);
        echap_et_lacher(&mut b, p + vec2(3.0 * cell, 0.0));
    }
    // i) Partie 208, point 7 : la clé vide (26-32 de Perso) choisie dans la timeline du Storyboard : son cadre de choix, ses deux
    // coupures.
    b.app.set_espace(Espace::Storyboard);
    b.app.montage_zoom = 8.0;
    let (panel, perso) = (b.app.doc.panels[0].id, b.app.doc.panels[0].layers[1].id);
    b.app.key_sel = vec![crate::montagekeys::KeyRef { panel, layer: perso, start: 26 }];
    b.image(vec![Event::PointerMoved(pos2(5.0, 5.0))], DT);
    b.chauffer(8);
    ecrire(&b, "i-cle-vide-choisie");
}

// ------------------------------------------------------------------ Partie 208 : les retouches vues sur les captures

/// Toutes les formes d'une liste, celles des groupes (`Shape::Vec`) comprises.
fn a_plat(formes: &[egui::epaint::ClippedShape]) -> Vec<egui::Shape> {
    fn ajouter(s: &egui::Shape, out: &mut Vec<egui::Shape>) {
        if let egui::Shape::Vec(v) = s {
            for t in v {
                ajouter(t, out);
            }
        } else {
            out.push(s.clone());
        }
    }
    let mut out = Vec::new();
    for c in formes {
        ajouter(&c.shape, &mut out);
    }
    out
}

/// Les formes peintes entièrement dans `zone` (à un demi-point près).
fn formes_dans(formes: &[egui::epaint::ClippedShape], zone: egui::Rect) -> Vec<egui::Shape> {
    a_plat(formes)
        .into_iter()
        .filter(|s| {
            let r = s.visual_bounding_rect();
            r.is_positive() && zone.expand(0.5).contains_rect(r)
        })
        .collect()
}

/// Le bord allumé : un rectangle clair (presque blanc, presque opaque) d'au moins 3 points de large, posé sur l'abscisse `x`, sur
/// au moins la moitié de la hauteur `haut`..`bas`.
fn bord_allume_a(formes: &[egui::epaint::ClippedShape], x: f32, haut: f32, bas: f32) -> bool {
    a_plat(formes).iter().any(|s| {
        let egui::Shape::Rect(r) = s else { return false };
        let c = r.fill;
        r.rect.left() <= x + 0.5
            && r.rect.right() >= x - 0.5
            && r.rect.width() >= 3.0
            && r.rect.width() <= 12.0
            && r.rect.height() >= (bas - haut) * 0.5
            && c.a() >= 200
            && c.r() >= 180
            && c.g() >= 180
            && c.b() >= 180
    })
}

/// `formes` n'est que la coupure verticale à l'abscisse `x`.
fn seule_la_coupure(formes: &[egui::Shape], x: f32) -> bool {
    formes.len() == 1 && matches!(&formes[0], egui::Shape::LineSegment { points, .. } if (points[0].x - x).abs() < 0.3 && (points[1].x - x).abs() < 0.3)
}

/// Le banc des captures, sans carte graphique, timeline du Storyboard, la colonne de gauche à sa largeur d'origine.
fn banc_sb() -> Banc {
    let mut b = crate::perf_tests::banc_taille(doc_des_captures(), false, 1600, 1000);
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.app.settings.storyboard_calques_replies = false;
    b.app.montage_zoom = 8.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 1, 18);
    b.garder_formes = true;
    b.chauffer(6);
    b.image(vec![egui::Event::PointerMoved(egui::pos2(5.0, 5.0))], DT);
    b.chauffer(3);
    b
}

/// Le rectangle peint de la clé `s`..`e` du calque Perso (plan 1) dans la timeline du Storyboard.
fn cle_sb(b: &Banc, s: usize, e: usize) -> Option<egui::Rect> {
    let (x0, ppf) = (blocs_sb()[0].left() - 1.0, b.app.montage_zoom);
    let (gauche, droite) = (x0 + s as f32 * ppf, x0 + e as f32 * ppf - 1.0);
    a_plat(&b.formes).into_iter().find_map(|f| match f {
        egui::Shape::Rect(r) if (r.rect.left() - gauche).abs() < 0.6 && (r.rect.right() - droite).abs() < 0.6 && r.fill.a() > 0 => Some(r.rect),
        _ => None,
    })
}

#[test]
fn an_empty_key_shows_nothing_but_the_grey_of_the_storyboard_timeline() {
    // Anthony : « Plutôt que des rayures, une clé vide n'affiche rien, si ce n'est le gris du fond de la timeline. »
    let b = banc_sb();
    let pleine = cle_sb(&b, 14, 26).expect("la clé 14-26 de Perso est peinte");
    assert!(cle_sb(&b, 26, 32).is_none(), "la clé vide 26-32 n'a pas de remplissage");
    let (x0, ppf) = (blocs_sb()[0].left() - 1.0, b.app.montage_zoom);
    let zone = egui::Rect::from_min_max(egui::pos2(x0 + 26.0 * ppf, pleine.top()), egui::pos2(x0 + 32.0 * ppf - 1.0, pleine.bottom()));
    // (Le trait de coupure déborde d'un demi-trait en haut et en bas.)
    let dedans = formes_dans(&b.formes, zone.expand2(egui::vec2(0.0, crate::clesvue::COUPURE_W)));
    // Rien que la coupure à son début (partie 208, point 7 : ses deux limites restent lisibles).
    assert!(seule_la_coupure(&dedans, x0 + 26.0 * ppf + crate::clesvue::COUPURE_W * 0.5), "rien dans la clé vide que sa coupure : {dedans:?}");
    // Le trait de coupure au début de la clé suivante reste.
    let x = x0 + 32.0 * ppf + crate::clesvue::COUPURE_W * 0.5;
    assert!(
        a_plat(&b.formes).iter().any(|s| matches!(s, egui::Shape::LineSegment { points, .. } if (points[0].x - x).abs() < 0.3 && (points[1].x - x).abs() < 0.3)),
        "la coupure de la clé 32"
    );
}

#[test]
fn an_empty_key_shows_nothing_in_the_animation_timeline_not_even_its_thumbnail() {
    let mut b = banc_sb();
    b.app.timeline_thumbs = true;
    b.app.set_espace(crate::workspace::Espace::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 1, 28);
    b.chauffer(8);
    let (gauche, cell) = crate::timeline::GRILLE_PEINTE.with(|g| g.get()).expect("grille peinte");
    let (_, haut, bas) = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().1.iter().copied().find(|l| l.0 == 1)).expect("la ligne de Perso");
    let zone = egui::Rect::from_min_max(egui::pos2(gauche + 26.0 * cell + 0.5, haut + 2.0), egui::pos2(gauche + 32.0 * cell - 0.5, bas - 2.0));
    let dedans: Vec<egui::Shape> = formes_dans(&b.formes, zone);
    assert!(
        seule_la_coupure(&dedans, gauche + 26.0 * cell + 1.0 + crate::clesvue::COUPURE_W * 0.5),
        "rien dans la clé vide (ni couleur, ni pastille, ni vignette, ni soulignement) que sa coupure : {dedans:?}"
    );
    // Une clé pleine, elle, est peinte.
    let pleine = egui::Rect::from_min_max(egui::pos2(gauche + 14.0 * cell + 0.5, haut + 2.0), egui::pos2(gauche + 26.0 * cell - 0.5, bas - 2.0));
    assert!(!formes_dans(&b.formes, pleine).is_empty());
}

#[test]
fn an_effect_layer_keeps_its_keys_even_though_its_drawings_are_empty() {
    let mut doc = Document::with_frame(32, 24, 24);
    let cel = doc.new_cel(LayerKind::Vector);
    let mut l = Layer::new("Effet", LayerKind::Vector);
    l.set(0, Some(cel));
    assert!(crate::clesvue::cle_vide(&doc, &l, cel), "un dessin sans trait sur un calque de dessin : clé vide");
    l.effet = Some(crate::calqueffet::CalqueEffet::ajustement("Flou"));
    assert!(!crate::clesvue::cle_vide(&doc, &l, cel), "un calque d'effet n'a que des dessins vides : ses clés se montrent");
}

#[test]
fn the_two_pen_buttons_stay_in_the_storyboard_bar_at_the_original_column_width() {
    let b = banc_sb();
    assert!((b.app.montage_head_w - crate::montagehead::LARGEUR_STORYBOARD).abs() < 0.5, "largeur d'origine : {}", b.app.montage_head_w);
    let caches = crate::barres::DEBORDENT.with(|d| d.borrow().clone());
    // « Nouveau plan » leur laisse sa place : il est aussi sur la ligne « Plans », juste dessous ; le menu Plans reste.
    assert!(caches.contains(&"panneau.nouveau"), "{caches:?}");
    assert!(!caches.contains(&"plans"), "le menu Plans reste dans la barre : {caches:?}");
    for id in ["cles.decaler_suite", "cles.etirer_proportion"] {
        assert!(!caches.contains(&id), "{id} sous « Plus… » : {caches:?}");
    }
    for nom in [crate::barres::BOUTON_DECALER_NOM, crate::barres::BOUTON_PROPORTION_NOM] {
        assert!(!crate::bulles::vus::ou(nom).is_empty(), "« {nom} » visible");
    }
}

/// La bulle du geste peinte dans `ctx` (une passe) : ses textes et leur nombre de lignes.
fn bulle_peinte(ctx: &egui::Context, app: &App, x: f32) -> Vec<(String, usize)> {
    ctx.begin_pass(egui::RawInput {
        events: vec![egui::Event::PointerMoved(egui::pos2(x, 500.0))],
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))),
        ..Default::default()
    });
    app.bulle_du_geste(ctx);
    let out = ctx.end_pass();
    let mut vus = Vec::new();
    for c in out.shapes {
        if let egui::Shape::Text(t) = c.shape {
            vus.push((t.galley.text().to_string(), t.galley.rows.len()));
        }
    }
    vus
}

#[test]
fn the_gesture_bubble_is_one_line_even_when_its_text_grows() {
    // Sur les captures : « Clé 2 : 10 i / (+4) » sur deux lignes. La bulle gardait la largeur de ses premières images (« Clé 2 :
    // 7 i (+1) ») : le texte qui s'allonge passait à la ligne.
    let mut app = deux_calques("bulle-ligne");
    let ctx = egui::Context::default();
    assert!(app.begin_handle_drag(Handle::Boundary { layer: 0, at: 12 }));
    app.drag_handle(13, 13);
    for k in 0..3 {
        bulle_peinte(&ctx, &app, 700.0 + k as f32);
    }
    app.drag_handle(16, 16);
    let mut vus = Vec::new();
    for k in 0..3 {
        vus = bulle_peinte(&ctx, &app, 704.0 + k as f32);
    }
    assert!(vus.iter().any(|(t, n)| t == "Clé 2 : 10 i (+4)" && *n == 1), "{vus:?}");
    app.end_handle_drag();
}

#[test]
fn the_plan_end_in_the_storyboard_timeline_has_one_bubble_with_the_plan_duration() {
    use crate::rognageui::EdgeKind;
    let mut app = deux_calques("fin-sb");
    app.doc.storyboard_locked = false;
    app.montage_zoom = 10.0;
    let d = app.begin_edge_drag(EdgeKind::Tail, 0, 240.0).expect("le bord de fin");
    app.montage_drag = Some(d);
    app.update_edge_drag(200.0);
    let attendu = format!("Durée du plan : {} (-4 i)", crate::storyboard::duration_label(20, app.doc.fps));
    assert_eq!(app.bulle_geste.as_deref(), Some(attendu.as_str()));
    assert!(app.status.contains("fin raccourcie"), "la barre d'état garde sa phrase : {}", app.status);
    // Pas de seconde bulle (la bande noire en haut) pour la fin d'un plan.
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput::default());
    let painter = ctx.layer_painter(egui::LayerId::background());
    let blocs = [egui::Rect::from_min_max(egui::pos2(0.0, 50.0), egui::pos2(200.0, 80.0))];
    app.paint_edge_drag(&painter, &blocs, &|f| f as f32 * 10.0, 50.0, 80.0, egui::Color32::RED);
    let out = ctx.end_pass();
    let textes: Vec<String> = out.shapes.iter().filter_map(|c| if let egui::Shape::Text(t) = &c.shape { Some(t.galley.text().to_string()) } else { None }).collect();
    assert!(textes.is_empty(), "{textes:?}");
    app.cancel_montage_drag();
}

#[test]
fn with_stretch_in_proportion_the_bubble_says_so() {
    use crate::rognageui::EdgeKind;
    let mut app = deux_calques("bulle-proportion");
    app.run_action("cles.etirer_proportion");
    assert!(app.begin_handle_drag(Handle::PlanEnd));
    app.drag_handle(20, 20);
    let b = app.bulle_geste.clone().unwrap_or_default();
    assert!(b.starts_with("Durée du plan : ") && b.ends_with("(-4 i) · toutes les clés en proportion"), "{b}");
    app.cancel_handle_drag();
    // Le bord de fin d'un plan dans la timeline du Storyboard (et le montage).
    app.doc.storyboard_locked = false;
    app.montage_zoom = 10.0;
    let d = app.begin_edge_drag(EdgeKind::Tail, 0, 240.0).expect("le bord de fin");
    app.montage_drag = Some(d);
    app.update_edge_drag(200.0);
    let b = app.bulle_geste.clone().unwrap_or_default();
    assert!(b.ends_with("(-4 i) · toutes les clés en proportion"), "{b}");
    app.cancel_montage_drag();
    // Éteint : la bulle ne le dit pas.
    app.run_action("cles.etirer_proportion");
    assert!(app.begin_handle_drag(Handle::PlanEnd));
    app.drag_handle(20, 20);
    assert!(!app.bulle_geste.clone().unwrap_or_default().contains("proportion"));
    app.cancel_handle_drag();
}

#[test]
fn the_lit_edge_is_a_light_band_on_hover_and_during_the_whole_gesture_in_the_storyboard() {
    let mut b = banc_sb();
    let pleine = cle_sb(&b, 14, 26).expect("clé 14-26");
    let (x0, ppf) = (blocs_sb()[0].left() - 1.0, b.app.montage_zoom);
    let x = x0 + 14.0 * ppf;
    let p = trouver_le_bord(&mut b, x);
    for _ in 0..3 {
        b.image(vec![egui::Event::PointerMoved(p)], DT);
    }
    assert!(bord_allume_a(&b.formes, x, pleine.top(), pleine.bottom()), "au survol");
    prendre_et_tirer(&mut b, p, 4.0 * ppf);
    assert!(b.app.handle_drag.is_some());
    assert!(bord_allume_a(&b.formes, x + 4.0 * ppf, pleine.top(), pleine.bottom()), "pendant le geste, à sa nouvelle place");
    echap_et_lacher(&mut b, p + egui::vec2(4.0 * ppf, 0.0));
}

#[test]
fn the_lit_edge_stays_lit_during_the_whole_gesture_in_the_animation_timeline() {
    let mut b = banc_sb();
    b.app.set_espace(crate::workspace::Espace::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 1, 2);
    b.chauffer(8);
    let (gauche, cell) = crate::timeline::GRILLE_PEINTE.with(|g| g.get()).expect("grille peinte");
    let (_, haut, bas) = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().1.iter().copied().find(|l| l.0 == 1)).expect("la ligne de Perso");
    // Le bord droit de la clé 8-14 de Perso.
    let x = gauche + 14.0 * cell;
    let p = egui::pos2(x - 1.0, (haut + bas) * 0.5);
    for _ in 0..3 {
        b.image(vec![egui::Event::PointerMoved(p)], DT);
    }
    assert_eq!(b.curseur, egui::CursorIcon::ResizeHorizontal, "le bord est sous le pointeur");
    assert!(bord_allume_a(&b.formes, x, haut, bas), "au survol");
    prendre_et_tirer(&mut b, p, 3.0 * cell);
    assert!(b.app.handle_drag.is_some(), "le geste est pris");
    assert!(bord_allume_a(&b.formes, x + 3.0 * cell, haut, bas), "pendant le geste, à sa nouvelle place");
    echap_et_lacher(&mut b, p + egui::vec2(3.0 * cell, 0.0));
}

// ------------------------------------------------------------------ Partie 208, point 7 : une clé vide n'est vide qu'à l'affichage

#[test]
fn an_empty_chosen_key_shows_the_same_selection_frame_as_the_others() {
    let mut b = banc_sb();
    let (panel, perso) = (b.app.doc.panels[0].id, b.app.doc.panels[0].layers[1].id);
    b.app.key_sel = vec![KeyRef { panel, layer: perso, start: 26 }];
    b.chauffer(3);
    let pleine = cle_sb(&b, 14, 26).expect("clé 14-26");
    let (x0, ppf) = (blocs_sb()[0].left() - 1.0, b.app.montage_zoom);
    let (gauche, droite) = (x0 + 26.0 * ppf, x0 + 32.0 * ppf - 1.0);
    let cadre = a_plat(&b.formes).into_iter().any(|f| {
        matches!(f, egui::Shape::Rect(r) if (r.rect.left() - gauche).abs() < 0.6 && (r.rect.right() - droite).abs() < 0.6
            && (r.rect.top() - pleine.top()).abs() < 0.6 && r.stroke.width >= 2.0 && r.fill.a() == 0)
    });
    assert!(cadre, "la clé vide choisie a son cadre de choix");
}

#[test]
fn an_empty_key_behaves_like_a_full_key_in_every_gesture() {
    // Anthony : « Une clé vide n'est pas véritablement vide. On peut agir dessus comme une clé pleine. Étirer le temps ou pousser /
    // être poussée par les clés précédentes et suivantes. » Calque A : 0-6 plein, 6-12 VIDE, 12-18 plein, 18-24 VIDE (la dernière).
    let pleins = |app: &mut App| {
        let cels: Vec<CelId> = [0usize, 12].iter().filter_map(|&f| app.doc.panels[0].layers[0].frames[f]).collect();
        for c in cels {
            un_trait(&mut app.doc, c);
        }
    };
    let neuf = |nom: &str| {
        let mut app = deux_calques(nom);
        pleins(&mut app);
        let l = &app.doc.panels[0].layers[0];
        assert!(crate::clesvue::cle_vide(&app.doc, l, l.frames[6].unwrap()) && !crate::clesvue::cle_vide(&app.doc, l, l.frames[0].unwrap()));
        app
    };
    // Ses deux bords se tirent (roll avec la voisine).
    let mut app = neuf("vide-bords");
    tirer_le_bord(&mut app, 0, 6, 4);
    tirer_le_bord(&mut app, 0, 12, 14);
    assert_eq!(spans(&app, 0), vec![(0, 4), (4, 14), (14, 18), (18, 24)]);
    // « Décaler la suite » : elle s'allonge et pousse la suite ; et elle est poussée par la clé d'avant.
    let mut app = neuf("vide-pousse");
    app.run_action("cles.decaler_suite");
    tirer_le_bord(&mut app, 0, 12, 15);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 15), (15, 21), (21, 24)], "la vide pousse la suite");
    tirer_le_bord(&mut app, 0, 6, 8);
    assert_eq!(spans(&app, 0), vec![(0, 8), (8, 17), (17, 23), (23, 24)], "la vide poussée par la clé d'avant");
    // La fin du plan raccourcit la dernière clé, vide, jusqu'à sa butée (une image).
    let mut app = neuf("vide-fin");
    tirer_la_fin(&mut app, 2);
    assert_eq!(app.doc.panels[0].frame_count(), 19);
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 19)]);
    // « Étirer en proportion » l'étire avec les autres.
    let mut app = neuf("vide-proportion");
    app.run_action("cles.etirer_proportion");
    tirer_la_fin(&mut app, 36);
    assert_eq!(durees(&app, 0), vec![9, 9, 9, 9]);
    // Ctrl + : elle gagne une image et la suite se décale.
    let mut app = neuf("vide-plus");
    let (panel, a) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id);
    app.key_sel = vec![KeyRef { panel, layer: a, start: 6 }];
    app.run_action("cles.image_plus");
    assert_eq!(&spans(&app, 0)[..3], &[(0, 6), (6, 13), (13, 19)]);
    // Prise par son milieu, elle se glisse comme les autres (après la clé 12-18).
    let mut app = neuf("vide-glisse");
    let vide = app.doc.panels[0].layers[0].frames[6];
    assert!(app.begin_handle_drag(Handle::Key { layer: 0, start: 6, end: 12, grab: 9 }));
    app.drag_handle(16, 16);
    app.end_handle_drag();
    assert_eq!(app.doc.panels[0].layers[0].frames[12], vide, "la clé vide est passée après sa voisine : {}", app.status);
    assert_ne!(app.doc.panels[0].layers[0].frames[6], vide);
}

#[test]
fn the_edges_of_an_empty_key_are_taken_by_the_pointer_in_the_storyboard_timeline() {
    // Ses deux bords (images 26 et 32 de Perso) mettent le curseur en ↔ et s'allument : on peut les tirer.
    let mut b = banc_sb();
    let pleine = cle_sb(&b, 14, 26).expect("clé 14-26");
    let (x0, ppf) = (blocs_sb()[0].left() - 1.0, b.app.montage_zoom);
    for f in [26.0, 32.0] {
        let x = x0 + f * ppf;
        let p = trouver_le_bord(&mut b, x);
        for _ in 0..3 {
            b.image(vec![egui::Event::PointerMoved(p)], DT);
        }
        assert!(bord_allume_a(&b.formes, x, pleine.top(), pleine.bottom()), "le bord {f} s'allume");
    }
}

// ------------------------------------------------------------------ Partie 208, point 8 : Retour arrière supprime la clé

#[test]
fn backspace_and_delete_remove_the_chosen_keys_and_their_neighbours_fill_the_place() {
    // Anthony : « Retour arrière supprime la clé car pas de Suppr sur Mac. On trouve un autre raccourci pour vider le dessin. »
    let mut app = deux_calques("suppr-supprime");
    let (panel, a, b) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id, app.doc.panels[0].layers[1].id);
    app.doc.panels[0].layers[1].locked = true;
    app.key_sel = vec![KeyRef { panel, layer: a, start: 6 }, KeyRef { panel, layer: a, start: 18 }, KeyRef { panel, layer: b, start: 12 }];
    app.key_focus = true;
    let n = app.history.undo_len();
    app.run_action("edition.supprimer");
    assert_eq!(spans(&app, 0), vec![(0, 12), (12, 24)], "les clés 6-12 et 18-24 supprimées, celles d'avant prennent leur place");
    assert_eq!(spans(&app, 1), vec![(0, 12), (12, 24)], "le calque verrouillé n'est pas touché");
    assert_eq!(app.doc.panels[0].frame_count(), 24, "le plan garde sa durée");
    assert_eq!(app.history.undo_len(), n + 1, "une étape");
    assert!(app.status.starts_with("2 clés supprimées"), "{}", app.status);
    assert!(app.status.contains("B") && app.status.contains("verrouillé"), "le calque laissé de côté est nommé : {}", app.status);
    app.run_action("edition.annuler");
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 24)]);
}

#[test]
fn backspace_is_delete_and_alt_backspace_or_alt_delete_empty_the_drawing_of_the_chosen_keys() {
    use crate::app::Mode;
    use egui::{Key, Modifiers};
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    let f = |k: Key, m: Modifiers, mode: Mode| map.find(k, Some(k), m, mode).map(str::to_string);
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(f(Key::Backspace, Modifiers::NONE, mode).as_deref(), Some("edition.supprimer"));
        assert_eq!(f(Key::Delete, Modifiers::NONE, mode).as_deref(), Some("edition.supprimer"));
        // (Maj+Retour arrière et Maj+Suppr sont pris : « Effacer ce qui est hors de la sélection », « Remplir la sélection ».)
        assert_eq!(f(Key::Backspace, Modifiers::ALT, mode).as_deref(), Some("cles.vider_dessin"));
        assert_eq!(f(Key::Delete, Modifiers::ALT, mode).as_deref(), Some("cles.vider_dessin"));
    }
    // « Vider le dessin des clés choisies » : les clés restent, leurs dessins se vident, une étape.
    let mut app = deux_calques("vider-dessin");
    let (panel, a) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id);
    let cel = app.doc.panels[0].layers[0].frames[6].unwrap();
    un_trait(&mut app.doc, cel);
    app.key_sel = vec![KeyRef { panel, layer: a, start: 6 }];
    app.key_focus = true;
    let n = app.history.undo_len();
    app.run_action("cles.vider_dessin");
    assert_eq!(spans(&app, 0), vec![(0, 6), (6, 12), (12, 18), (18, 24)], "les clés restent");
    assert!(app.doc.cels[&cel].is_empty(), "le dessin est vidé : {}", app.status);
    assert_eq!(app.history.undo_len(), n + 1);
}
