//! Essais de la partie 205 (`bordscles.rs`) : la fin d'un plan et le bord entre deux clés d'un calque (Anthony, 2026-11-21 : « Feanor
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
fn each_key_starts_with_a_dark_cut_an_empty_key_is_hatched_and_the_key_under_the_playhead_is_underlined() {
    let seg = egui::Rect::from_min_size(egui::pos2(100.0, 10.0), egui::vec2(120.0, 20.0));
    // B : la coupure, un trait vertical au bord gauche.
    let (s, _) = formes_de(seg, 0, 12, false, None, false, false);
    let lignes = segments(&s);
    assert!(lignes.iter().any(|p| (p[0].x - p[1].x).abs() < 0.01 && (p[0].x - seg.left()).abs() < 1.5), "{lignes:?}");
    assert_eq!(lignes.len(), 1, "un dessin tenu : pas de hachures");
    // F : une clé vide, hachurée (des traits en biais).
    let (s, _) = formes_de(seg, 0, 12, true, None, false, false);
    assert!(segments(&s).iter().filter(|p| (p[0].x - p[1].x).abs() > 1.0).count() >= 5, "hachures");
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
    let mut b = crate::perf_tests::banc_taille(doc_deux_calques(), false, 1600, 900);
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
