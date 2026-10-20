//! Pistes repliables du montage (Anthony, 2026-10-20) : deux hauteurs de piste (dépliée, repliée), un triangle
//! par piste, tout replier / tout déplier, le calque courant qui se déplie tout seul, les dossiers qui cachent
//! les pistes de leurs calques, l'état enregistré avec le projet et partagé avec les lignes réduites de la
//! timeline de l'animation, les gestes qui marchent sur une piste repliée.
//!
//! Les premiers essais sont purs (pistes, hauteurs, hits) ; les derniers passent par la vraie fenêtre sans
//! écran (`Banc`) : on retrouve où les losanges sont tombés à l'écran d'après les formes peintes, puis on clique
//! et on glisse avec le pointeur.

use eframe::egui::{self, epaint::Shape, pos2, Color32, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::app::App;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::handles::Handle;
use crate::montagerows::LAYER_H;
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes::{lanes, row_folded, row_hidden, FOLDED_H};
use crate::poskeys::PosKey;
use crate::workspace::Espace;

const PPF: f32 = 12.0;
const ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };

// ------------------------------------------------------------------------------------ l'outillage

/// Un film dont chaque plan a ces calques, donnés du haut vers le bas.
fn doc_with(plans: &[&[&str]]) -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    while doc.panels.len() < plans.len() {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    for (p, names) in doc.panels.iter_mut().zip(plans) {
        p.layers = names.iter().rev().map(|n| Layer::new(*n, LayerKind::Vector)).collect();
        p.set_length(24);
    }
    doc
}

/// Une application sans fenêtre, dans un bac à sable, avec ce document.
fn app_with(name: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("shot-pistes-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

/// Les hauteurs des pistes visibles d'une application, de haut en bas : (rangée, hauteur).
fn heights(app: &App) -> Vec<(usize, f32)> {
    app.montage_rows(300.0).lanes.iter().map(|l| (l.row, l.h)).collect()
}

/// Dossier fermé ou ouvert avec deux calques, et un calque libre dessous (du bas vers le haut).
fn stack_with_folder(closed: bool) -> Vec<Layer> {
    let mut folder = Layer::new_folder("Dossier");
    folder.collapsed = closed;
    let mut c1 = Layer::new("C1", LayerKind::Vector);
    c1.parent = Some(folder.id);
    let mut c2 = Layer::new("C2", LayerKind::Vector);
    c2.parent = Some(folder.id);
    vec![Layer::new("Libre", LayerKind::Vector), c2, c1, folder]
}

// ------------------------------------------------------------------------------------ les hauteurs

#[test]
fn an_untouched_montage_keeps_every_track_at_the_old_height() {
    let doc = doc_with(&[&["A", "B", "C"], &["X"]]);
    let l = lanes(&doc.panels, 0, 0, 3, None);
    assert_eq!(l.len(), 3);
    for (i, lane) in l.iter().enumerate() {
        assert_eq!((lane.row, lane.h, lane.top, lane.folded), (i, LAYER_H, i as f32 * LAYER_H, false));
    }
}

#[test]
fn a_folded_layer_makes_its_track_thin_in_every_plan_and_the_tracks_below_move_up() {
    let mut doc = doc_with(&[&["A", "B", "C"], &["X", "Y"]]);
    // Replié dans le plan 2 seulement : la piste 1 (rangée 1) l'est pour tous (« replié l'emporte »).
    doc.panels[1].layers[0].collapsed = true;
    let l = lanes(&doc.panels, 0, 0, 3, None);
    assert_eq!(l.iter().map(|x| x.h).collect::<Vec<_>>(), [LAYER_H, FOLDED_H, LAYER_H]);
    assert_eq!(l[2].top, LAYER_H + FOLDED_H, "la piste du dessous remonte");
    assert!(row_folded(&doc.panels, 1, None) && !row_folded(&doc.panels, 0, None));
    assert!(FOLDED_H < LAYER_H / 2.0, "une piste repliée prend moins de la moitié de la place");
}

#[test]
fn a_closed_folder_hides_the_tracks_of_its_layers_and_is_itself_a_thin_track() {
    let mut doc = doc_with(&[&["A"]]);
    doc.panels[0].layers = stack_with_folder(true);
    // Le calque courant est « Libre » (rang 0, tout en bas, rangée 3).
    let closed = lanes(&doc.panels, 0, 0, 4, None);
    assert_eq!(closed.iter().map(|l| (l.row, l.h)).collect::<Vec<_>>(), [(0, FOLDED_H), (3, LAYER_H)], "le dossier fermé, puis le calque libre ; C1 et C2 n'ont plus de piste");
    assert!(row_hidden(&doc.panels, 0, 0, 1) && row_hidden(&doc.panels, 0, 0, 2) && !row_hidden(&doc.panels, 0, 0, 0));
    // Ouvert : tout revient, le dossier à pleine hauteur (son opacité se règle dans sa colonne).
    doc.panels[0].layers[3].collapsed = false;
    let open = lanes(&doc.panels, 0, 0, 4, None);
    assert_eq!(open.iter().map(|l| l.h).collect::<Vec<_>>(), [LAYER_H; 4]);
    // Le calque courant d'un dossier fermé garde sa piste (comme dans le panneau Calques).
    doc.panels[0].layers[3].collapsed = true;
    let rows: Vec<usize> = lanes(&doc.panels, 0, 2, 4, None).iter().map(|l| l.row).collect();
    assert_eq!(rows, [0, 1, 3], "C1 (rang 2, rangée 1) reste");
}

#[test]
fn a_layer_that_opened_itself_is_open_whatever_the_other_plans_say() {
    let mut doc = doc_with(&[&["A", "B"], &["X", "Y"]]);
    for p in &mut doc.panels {
        for l in &mut p.layers {
            l.collapsed = true;
        }
    }
    let (plan, layer) = (doc.panels[1].id, doc.panels[1].layers[0].id); // « Y », rangée 1 du plan 2
    let l = lanes(&doc.panels, 0, 0, 2, Some((plan, layer)));
    assert_eq!(l.iter().map(|x| x.h).collect::<Vec<_>>(), [FOLDED_H, LAYER_H], "la rangée de « Y » est dépliée dans tous les plans");
    // Un dossier ne se déplie pas ainsi : son état dit s'il est fermé.
    let mut with_folder = doc_with(&[&["A"]]);
    with_folder.panels[0].layers = stack_with_folder(true);
    let (plan, folder) = (with_folder.panels[0].id, with_folder.panels[0].layers[3].id);
    assert!(row_folded(&with_folder.panels, 0, Some((plan, folder))));
}

// ------------------------------------------------------------------------------------ replier, déplier

#[test]
fn folding_a_track_folds_that_row_in_every_plan_and_unfolding_undoes_it() {
    let mut app = app_with("une", doc_with(&[&["A", "B", "C"], &["X", "Y", "Z"], &["P"]]));
    app.settings.montage_auto_open = false;
    assert!(app.set_track_folded(1, true));
    assert!(app.modified, "l'état s'enregistre avec le projet");
    let flags = |app: &App| app.doc.panels.iter().map(|p| p.layers.iter().rev().map(|l| l.collapsed).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert_eq!(flags(&app), vec![vec![false, true, false], vec![false, true, false], vec![false]], "la rangée 1 de chaque plan qui en a une");
    assert_eq!(heights(&app), [(0, LAYER_H), (1, FOLDED_H), (2, LAYER_H)]);
    assert!(app.set_track_folded(1, false));
    assert!(flags(&app).iter().flatten().all(|f| !f));
    assert!(!app.set_track_folded(7, true), "une rangée qui n'existe pas ne fait rien");
}

#[test]
fn folding_a_folder_closes_it_in_the_current_plan_only() {
    let mut doc = doc_with(&[&["A"], &["B", "C", "D", "E"]]);
    doc.panels[0].layers = stack_with_folder(false);
    let mut app = app_with("dossier", doc);
    app.settings.montage_auto_open = false;
    // Rangée 0 : le dossier du plan 1 ; le plan 2 a un calque ordinaire à cette rangée, qui n'est pas touché.
    assert!(app.set_track_folded(0, true));
    assert!(app.doc.panels[0].layers[3].collapsed);
    assert!(!app.doc.panels[1].layers[3].collapsed, "le calque ordinaire du plan 2 reste déplié");
    // Ses calques n'ont plus de piste dans le plan courant ; « Libre » (rang 0, courant) garde la sienne.
    assert_eq!(heights(&app).iter().map(|h| h.0).collect::<Vec<_>>(), [0, 3], "C1 et C2 sont cachés, la rangée 3 reste");
    app.set_track_folded(0, false);
    assert_eq!(heights(&app).len(), 4);
}

#[test]
fn fold_all_and_unfold_all_touch_every_plan_and_leave_folders_alone_until_unfolded() {
    let mut doc = doc_with(&[&["A"], &["B", "C"]]);
    doc.panels[0].layers = stack_with_folder(false);
    let mut app = app_with("tout", doc);
    app.settings.montage_auto_open = false;
    let steps = app.history.names().0.len();
    let changed = app.set_all_tracks_folded(true);
    assert_eq!(changed, 3 + 2, "tous les calques ordinaires des deux plans");
    assert!(app.doc.panels[0].layers.iter().filter(|l| !l.folder).all(|l| l.collapsed));
    assert!(!app.doc.panels[0].layers[3].collapsed, "le dossier reste ouvert : toutes les pistes restent visibles, en bandes fines");
    let h = heights(&app);
    assert_eq!(h.len(), 4);
    assert!(h.iter().all(|x| x.1 == FOLDED_H), "même la rangée du dossier, ouvert, est fine : le calque ordinaire du plan 2 à cette rangée est replié");
    // Déplier tout ouvre aussi les dossiers fermés : plus rien de caché.
    app.doc.panels[0].layers[3].collapsed = true;
    let changed = app.set_all_tracks_folded(false);
    assert_eq!(changed, 3 + 2 + 1);
    assert!(app.doc.panels.iter().flat_map(|p| &p.layers).all(|l| !l.collapsed));
    assert_eq!(app.history.names().0.len(), steps, "replier est de l'affichage : pas d'étape d'annulation");
    assert!(app.modified);
}

#[test]
fn the_action_ids_do_the_same() {
    let mut app = app_with("actions", doc_with(&[&["A", "B"]]));
    app.settings.montage_auto_open = false;
    app.cur_layer = 1; // « A », rangée 0
    app.run_action("piste.basculer");
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, LAYER_H)]);
    app.run_action("piste.basculer");
    assert_eq!(heights(&app), [(0, LAYER_H), (1, LAYER_H)]);
    app.run_action("piste.replier_tout");
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, FOLDED_H)]);
    app.run_action("piste.deplier_tout");
    assert_eq!(heights(&app), [(0, LAYER_H), (1, LAYER_H)]);
    app.run_action("piste.auto_deplier");
    assert!(app.settings.montage_auto_open);
    app.run_action("piste.auto_deplier");
    assert!(!app.settings.montage_auto_open);
}

// ------------------------------------------------------------------------------------ le calque courant

#[test]
fn the_current_layer_unfolds_its_track_after_the_release_and_the_others_keep_their_state() {
    let mut app = app_with("courant", doc_with(&[&["A", "B", "C"]]));
    app.set_all_tracks_folded(true);
    // Souris tenue : rien ne bouge (les pistes d'en dessous glisseraient sous le pointeur).
    app.pistes_follow_current(true);
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H; 3]);
    // Relâchée : la piste du calque courant (rang 0 = rangée 2) se déplie.
    app.pistes_follow_current(false);
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H, FOLDED_H, LAYER_H]);
    assert!(app.doc.panels[0].layers.iter().all(|l| l.collapsed), "l'état enregistré n'a pas changé : le calque courant se montre ouvert, c'est tout");
    // Un autre calque devient courant : il s'ouvre quand la souris est relâchée, l'ancien se referme.
    app.select_layer(1);
    app.pistes_follow_current(true);
    assert_eq!(heights(&app)[2].1, LAYER_H, "pendant le geste, la disposition reste");
    app.pistes_follow_current(false);
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H, LAYER_H, FOLDED_H]);
    // Le réglage éteint : tout reste comme enregistré.
    app.settings.montage_auto_open = false;
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H; 3]);
}

#[test]
fn folding_the_open_current_track_by_hand_keeps_it_folded_until_the_current_layer_changes() {
    let mut app = app_with("main", doc_with(&[&["A", "B"]]));
    app.pistes_follow_current(false);
    assert!(!app.track_is_folded(1), "le calque courant (rang 0, rangée 1) est déplié");
    app.toggle_track_fold(1, false);
    assert!(app.track_is_folded(1), "replié à la main");
    app.pistes_follow_current(false);
    assert!(app.track_is_folded(1), "le calque courant n'a pas changé : il reste replié");
    app.select_layer(1);
    app.pistes_follow_current(false);
    assert!(!app.track_is_folded(0), "le nouveau calque courant s'ouvre");
    assert!(app.track_is_folded(1), "et l'ancien reste comme on l'a laissé");
    // Allumer le réglage remet le calque courant dans l'état « ouvert ».
    app.settings.montage_auto_open = false;
    app.toggle_auto_open();
    app.pistes_follow_current(false);
    assert!(app.settings.montage_auto_open && !app.track_is_folded(0));
}

// ------------------------------------------------------------------------------------ l'état enregistré

#[test]
fn the_fold_state_is_saved_with_the_project_without_changing_the_format() {
    let mut app = app_with("sauvegarde", doc_with(&[&["A", "B", "C"], &["X", "Y"]]));
    app.settings.montage_auto_open = false;
    app.set_track_folded(1, true);
    let dir = std::env::temp_dir().join(format!("shot-pistes-projet-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let back = crate::project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert!(text.contains(&format!("\"version\": {}", crate::project::VERSION)), "la version du format n'a pas changé");
    assert_eq!(text.matches("\"replie\": true").count(), 2, "un drapeau par calque replié, rien d'autre d'écrit");
    let flags: Vec<Vec<bool>> = back.panels.iter().map(|p| p.layers.iter().rev().map(|l| l.collapsed).collect()).collect();
    assert_eq!(flags, vec![vec![false, true, false], vec![false, true]]);
    // Rouvert, le montage a les mêmes hauteurs.
    let mut again = app_with("sauvegarde2", back);
    again.settings.montage_auto_open = false;
    assert_eq!(heights(&again), [(0, LAYER_H), (1, FOLDED_H), (2, LAYER_H)]);
}

#[test]
fn the_timeline_reduced_rows_and_the_montage_folded_tracks_are_the_same_state() {
    use crate::layerheads::{rows_of, ROW_FULL, ROW_SMALL};
    let mut app = app_with("timeline", doc_with(&[&["A", "B", "C"]]));
    app.settings.montage_auto_open = false;
    // Replié dans le montage (rangée 1 = « B », rang 1) : la ligne de la timeline de l'animation est réduite.
    app.set_track_folded(1, true);
    let rows = rows_of(app.layers(), &app.doc.groups);
    assert_eq!(rows.iter().map(|r| (r.li, r.h)).collect::<Vec<_>>(), [(2, ROW_FULL), (1, ROW_SMALL), (0, ROW_FULL)]);
    // Réduit par le triangle de la timeline (`collapsed`) : la piste du montage est repliée.
    app.layers_mut()[1].collapsed = false;
    app.layers_mut()[0].collapsed = true;
    assert_eq!(heights(&app), [(0, LAYER_H), (1, LAYER_H), (2, FOLDED_H)]);
}

// ------------------------------------------------------------------------------------ les hits

/// Un plan de 24 images : « Perso » (dessin de 0 à 12 puis de 12 à 24, clés de position 4, 10, 16) et
/// « Fond » (un dessin, clés de position 3 et 9), et un effet à clés 2, 10 et 14 sur « Perso ».
fn hit_app(name: &str) -> App {
    let mut doc = doc_with(&[&["Perso", "Fond"]]);
    let (a, b, c) = (
        doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap)),
        doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap)),
        doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap)),
    );
    let pose = |x: f32| crate::document::LayerTransform { x, ..Default::default() };
    {
        let perso = &mut doc.panels[0].layers[1];
        perso.frames = (0..24).map(|f| Some(if f < 12 { a } else { b })).collect();
        perso.pos_keys = vec![PosKey::new(4, pose(0.0)), PosKey::new(10, pose(50.0)), PosKey::new(16, pose(90.0))];
        let mut blur = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
        for (f, v) in [(2, 3.0), (10, 6.0), (14, 9.0)] {
            blur.set_key(f, vec![v]);
        }
        perso.fx = vec![blur];
    }
    {
        let fond = &mut doc.panels[0].layers[0];
        fond.frames = vec![Some(c); 24];
        fond.pos_keys = vec![PosKey::new(3, pose(0.0)), PosKey::new(9, pose(30.0))];
    }
    let mut app = app_with(name, doc);
    app.cur_layer = 1;
    app.settings.montage_auto_open = false;
    app.select_fx(0);
    app
}

#[test]
fn the_diamonds_and_the_handles_are_found_in_a_folded_track() {
    let mut app = hit_app("hits");
    app.set_track_folded(0, true);
    let rows = app.montage_rows(300.0);
    let lane = rows.lanes[0];
    assert!(lane.folded && lane.h == FOLDED_H);
    let top = 10.0;
    let x0 = 100.0;
    let blocks = vec![Rect::from_min_max(pos2(x0 + 1.0, 50.0), pos2(x0 + 24.0 * PPF - 1.0, 100.0))];
    let x = |f: usize| x0 + f as f32 * PPF + PPF * 0.5;
    let y = |d: f32| top + rows.layers_top() + d;
    // Losanges violets (effet) en haut, verts (position) plus bas, dans une piste de 16 de haut.
    assert_eq!(app.montage_fx_key_at(pos2(x(10), y(lane.fx_y())), top, &blocks, PPF, &rows), Some((0, 1, 10)));
    assert_eq!(app.montage_fx_key_at(pos2(x(10), y(8.0)), top, &blocks, PPF, &rows), None, "sous la bande des effets");
    assert_eq!(app.montage_pos_key_at(pos2(x(10), y(lane.pos_y())), top, &blocks, PPF, &rows), Some((0, 1, 10)));
    assert_eq!(app.montage_pos_key_at(pos2(x(10), y(lane.fx_y())), top, &blocks, PPF, &rows), None, "en haut : l'effet");
    assert_eq!(app.montage_pos_key_at(pos2(x(10), y(lane.pos_y() + 5.0)), top, &blocks, PPF, &rows), None, "trop bas");
    assert_eq!(app.montage_pos_key_at(pos2(x(7), y(lane.pos_y())), top, &blocks, PPF, &rows), None, "entre deux losanges");
    // Les poignées du dessin, comme dans la timeline : le milieu d'un dessin, la limite entre deux, la barre.
    let handle = |px: f32, d: f32| app.montage_layer_handle_at(pos2(px, y(d)), top, &blocks, PPF, &rows).map(|h| h.2);
    assert!(matches!(handle(x(6), 3.0), Some(Handle::Key { start: 0, end: 12, .. })), "le milieu du premier dessin");
    assert!(matches!(handle(x0 + 12.0 * PPF, 3.0), Some(Handle::Boundary { at: 12, .. })), "la limite entre les deux dessins");
    assert!(matches!(handle(x(6), lane.h - 3.0), Some(Handle::LayerMove { .. })), "la barre du calque, tout en bas de la piste");
    assert!(matches!(handle(x(6), lane.bar_from() - 1.0), Some(Handle::Key { .. })), "juste au-dessus de la barre : le dessin");
    // La rangée sous le point : la piste repliée, puis la suivante (dépliée) juste dessous.
    assert_eq!(app.row_at(pos2(x(6), y(5.0)), top, &rows), Some(0));
    assert_eq!(app.row_at(pos2(x(6), y(FOLDED_H + 2.0)), top, &rows), Some(1));
    assert_eq!(app.row_at(pos2(x(6), y(FOLDED_H + LAYER_H + 2.0)), top, &rows), None, "après la dernière piste");
    assert_eq!(app.layer_row_hit(pos2(x(6), y(FOLDED_H + 2.0)), top, &blocks, &rows), Some((0, 0)));
    // Le choix au rectangle : une hauteur avant la première piste ou après la dernière tombe sur la plus proche.
    assert_eq!(rows.row_near(-5.0), 0);
    assert_eq!(rows.row_near(FOLDED_H + 5.0), 1);
    assert_eq!(rows.row_near(1000.0), 1);
}

#[test]
fn the_geometry_of_an_unfolded_track_is_the_old_one() {
    let app = hit_app("hits2");
    let rows = app.montage_rows(300.0);
    let lane = rows.lanes[0];
    assert!(!lane.folded);
    assert_eq!((lane.fx_hit_max(), lane.pos_y(), lane.pos_hit()), (crate::fxkeys::STRIP, LAYER_H * 0.5, crate::poskeysui::HIT_H));
    assert_eq!(lane.bar_from(), LAYER_H - 3.0 - crate::montagerows::BAR_H - 1.0);
}

#[test]
fn the_rectangle_choice_skips_the_tracks_of_a_closed_folder() {
    let mut doc = doc_with(&[&["A"]]);
    let cel = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
    doc.panels[0].layers = stack_with_folder(false);
    for l in &mut doc.panels[0].layers {
        if !l.folder {
            l.frames = vec![Some(cel); 24];
        }
    }
    let mut app = app_with("rect", doc);
    app.cur_layer = 0;
    // Rangées 0 (dossier), 1 (C1), 2 (C2), 3 (Libre) : le dossier ouvert montre tout.
    assert_eq!(app.keys_in_rect(0..=3, 0, 5).len(), 3, "C1, C2 et Libre");
    app.set_track_folded(0, true);
    assert_eq!(app.keys_in_rect(0..=3, 0, 5).len(), 1, "le dossier fermé cache C1 et C2 : seule « Libre » (courante) reste");
}

// ------------------------------------------------------------------------------------ la vraie fenêtre

/// Quatre calques dans le plan 1 (« Perso », « Décor », « Fond », « Vide », de haut en bas), un seul dans le
/// plan 2. Des clés de position : Perso 4, 10, 16 ; Décor 6, 12 ; Fond 5, 15 ; « Vide » n'a aucun dessin.
fn film() -> Document {
    let mut doc = doc_with(&[&["Perso", "Décor", "Fond", "Vide"], &["Autre"]]);
    let cels: Vec<_> = (0..5).map(|_| doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap))).collect();
    let pose = |x: f32| crate::document::LayerTransform { x, ..Default::default() };
    let keys = |frames: &[usize]| frames.iter().map(|&f| PosKey::new(f, pose(f as f32))).collect::<Vec<_>>();
    {
        let layers = &mut doc.panels[0].layers; // du bas vers le haut : Vide, Fond, Décor, Perso
        layers[3].frames = (0..24).map(|f| Some(if f < 12 { cels[0] } else { cels[1] })).collect();
        layers[3].pos_keys = keys(&[4, 10, 16]);
        layers[2].frames = vec![Some(cels[2]); 24];
        layers[2].pos_keys = keys(&[6, 12]);
        layers[1].frames = vec![Some(cels[3]); 24];
        layers[1].pos_keys = keys(&[5, 15]);
    }
    doc.panels[1].layers[0].frames = vec![Some(cels[4]); 24];
    doc
}

fn banc_montage() -> Banc {
    let mut b = banc_taille(film(), false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = PPF;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 3, 0);
    b.garder_formes = true;
    b.garder_textes = true;
    b.chauffer(8);
    b
}

fn chercher(shape: &Shape, fill: Color32, out: &mut Vec<Rect>) {
    match shape {
        Shape::Path(p) if p.fill == fill && p.points.len() == 4 => out.push(Rect::from_points(&p.points)),
        Shape::Vec(v) => v.iter().for_each(|s| chercher(s, fill, out)),
        _ => {}
    }
}

/// Les losanges de cette couleur peints par la dernière image.
fn losanges(b: &Banc, fill: Color32) -> Vec<Rect> {
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, fill, &mut out);
    }
    out
}

fn textes_de(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| textes_de(s, out)),
        _ => {}
    }
}

/// Les textes peints par la dernière image, avec leur place.
fn textes(b: &Banc) -> Vec<(String, Rect)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes_de(&s.shape, &mut out);
    }
    out
}

/// Où sont les repères de l'écran : l'abscisse de l'image 4 du film et le haut des pistes de calques.
struct Lieu {
    x4: f32,
    /// Ordonnée écran du haut des pistes de calques.
    layers_top: f32,
}

impl Lieu {
    fn x(&self, g: usize) -> f32 {
        self.x4 + (g as f32 - 4.0) * PPF
    }
}

/// À prendre quand la piste de Perso (rangée 0) est la première et que son losange de l'image 4 est le plus à
/// gauche des losanges verts.
fn lieu(b: &Banc) -> Lieu {
    let rows = b.app.montage_rows(300.0);
    let mut l = losanges(b, crate::theme::POIGNEE_CALQUE);
    l.sort_by(|a, c| a.center().x.total_cmp(&c.center().x));
    let first = l[0].center();
    let lane = rows.lanes[0];
    Lieu { x4: first.x, layers_top: first.y - lane.pos_y() - lane.top }
}

/// L'ordonnée écran de `d` points sous le haut de la piste de la rangée `row`.
fn y_in(b: &Banc, l: &Lieu, row: usize, d: f32) -> f32 {
    let rows = b.app.montage_rows(300.0);
    l.layers_top + rows.lanes.iter().find(|x| x.row == row).expect("la piste est visible").top + d
}

fn pointer_button(p: Pos2, pressed: bool, mods: Modifiers) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: mods }
}

fn clic(b: &mut Banc, p: Pos2, mods: Modifiers) {
    b.touches = mods;
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![pointer_button(p, true, mods)], 1.0 / 60.0);
    b.image(vec![pointer_button(p, false, mods)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
}

fn double_clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    for _ in 0..2 {
        b.image(vec![pointer_button(p, true, Modifiers::NONE)], 1.0 / 60.0);
        b.image(vec![pointer_button(p, false, Modifiers::NONE)], 1.0 / 60.0);
    }
    b.chauffer(3);
}

/// Prend en `a`, tire jusqu'en `z` (six pas), lâche.
fn glisser(b: &mut Banc, a: Pos2, z: Pos2) {
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![pointer_button(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![pointer_button(z, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

#[test]
fn a_folded_track_is_drawn_thin_with_small_diamonds_and_the_tracks_below_move_up() {
    let mut b = banc_montage();
    let before = losanges(&b, crate::theme::POIGNEE_CALQUE);
    assert_eq!(before.len(), 7, "trois clés de Perso, deux de Décor, deux de Fond : {before:?}");
    let l = lieu(&b);
    let x4 = l.x4;
    let l_x = move |g: usize| x4 + (g as f32 - 4.0) * PPF;
    let at = |ds: &[Rect], g: usize| ds.iter().find(|r| (r.center().x - l_x(g)).abs() < 1.0).copied().unwrap_or_else(|| panic!("pas de losange à l'image {g} : {ds:?}"));
    let (d0, f0, p0) = (at(&before, 6), at(&before, 5), at(&before, 4));
    assert!((d0.height() - 11.0).abs() < 1.5, "un losange de piste dépliée fait 11 de haut : {}", d0.height());
    assert!((d0.center().y - y_in(&b, &l, 1, 20.0)).abs() < 0.6, "au milieu de sa piste");
    // Replie « Décor » (rangée 1) ; Perso, le calque courant, reste déplié. Les pistes prennent moins de place :
    // les plans, au-dessus, en gagnent, et le haut des pistes descend d'autant (on le relit).
    b.app.set_track_folded(1, true);
    b.chauffer(4);
    let (l_avant, l) = (l, lieu(&b));
    assert!(l.layers_top >= l_avant.layers_top, "les plans reprennent la place gagnée");
    let after = losanges(&b, crate::theme::POIGNEE_CALQUE);
    assert_eq!(after.len(), 7);
    let (d1, f1, p1) = (at(&after, 6), at(&after, 5), at(&after, 4));
    assert!(d1.height() < 8.0, "en piste repliée, le losange est petit : {}", d1.height());
    assert!((d1.center().y - y_in(&b, &l, 1, 9.0)).abs() < 0.6, "le losange de Décor est dans sa bande fine");
    assert!((f1.center().y - y_in(&b, &l, 2, 20.0)).abs() < 0.6 && (f0.center().y - f1.center().y - (l_avant.layers_top - l.layers_top) - (LAYER_H - FOLDED_H)).abs() < 0.6, "Fond remonte de la différence des deux hauteurs");
    assert!((p1.center().y - y_in(&b, &l, 0, 20.0)).abs() < 0.6 && (p0.height() - p1.height()).abs() < 0.1, "Perso, déplié, garde sa piste");
    // Le nom du calque replié reste lisible à l'écran.
    assert!(textes(&b).iter().any(|(t, _)| t == "Décor"));
}

#[test]
fn the_gestures_of_the_timeline_work_on_a_folded_track() {
    let mut b = banc_montage();
    b.app.settings.montage_auto_open = false;
    b.app.set_all_tracks_folded(true);
    b.chauffer(4);
    let l = lieu(&b);
    assert!(b.app.montage_rows(300.0).lanes.iter().all(|x| x.folded));
    // 1. Un clic sur le dessin de « Décor » le choisit, et son calque devient courant.
    let start = etapes(&b);
    let p = pos2(l.x(2), y_in(&b, &l, 1, 4.0));
    clic(&mut b, p, Modifiers::NONE);
    assert_eq!(b.app.cur_layer, 2, "« Décor » est le rang 2 (de bas en haut : Vide, Fond, Décor, Perso)");
    assert_eq!(b.app.key_sel.len(), 1, "{:?}", b.app.key_sel);
    assert_eq!(etapes(&b), start, "choisir n'est pas une étape d'annulation");
    // 2. La limite entre les deux dessins de « Perso » (entre les images 11 et 12) se tire : l'un s'allonge,
    // l'autre raccourcit.
    let before = etapes(&b);
    let boundary = pos2(l.x(12) - PPF * 0.5, y_in(&b, &l, 0, 3.0));
    glisser(&mut b, boundary, pos2(boundary.x + 3.0 * PPF, boundary.y));
    let perso = &b.app.doc.panels[0].layers[3];
    assert!(perso.frames[12..15].iter().all(|f| *f == perso.frames[0]), "le premier dessin tient 3 images de plus : {:?}", perso.frames);
    assert_ne!(perso.frames[15], perso.frames[0]);
    assert_eq!(etapes(&b), before + 1, "un geste, une étape d'annulation");
    b.app.undo();
    b.chauffer(3);
    let perso = &b.app.doc.panels[0].layers[3];
    assert!(perso.frames[12] != perso.frames[0] && perso.frames[11] == perso.frames[0]);
    // 3. Un losange de position de « Perso » (image 10) se glisse à l'image 11, comme dans une piste dépliée.
    let before = etapes(&b);
    let cy = y_in(&b, &l, 0, 9.0);
    glisser(&mut b, pos2(l.x(10), cy), pos2(l.x(11), cy));
    let keys: Vec<usize> = b.app.doc.panels[0].layers[3].pos_keys.iter().map(|k| k.frame).collect();
    assert_eq!(keys, vec![4, 11, 16]);
    assert_eq!(etapes(&b), before + 1);
    // 4. Le choix au rectangle : parti de « Vide » (rangée 3, vide, repliée), jusqu'à « Décor » (rangée 1).
    b.app.key_sel.clear();
    let (from, to) = (pos2(l.x(20), y_in(&b, &l, 3, 8.0)), pos2(l.x(1), y_in(&b, &l, 1, 8.0)));
    glisser(&mut b, from, to);
    let mut rows: Vec<usize> = b.app.key_sel.iter().map(|k| b.app.doc.panels[0].layers.iter().position(|x| x.id == k.layer).unwrap()).collect();
    rows.sort();
    rows.dedup();
    assert_eq!(rows, vec![1, 2], "Fond (rang 1) et Décor (rang 2), pas Perso");
}

#[test]
fn the_triangle_folds_a_track_and_alt_click_folds_all_of_them() {
    let mut b = banc_montage();
    // « Décor » : son triangle est à gauche de son nom, dans la colonne.
    let triangle = |b: &Banc, glyph: &str| {
        let all = textes(b);
        let name = all.iter().filter(|(t, _)| t == "Décor").map(|(_, r)| *r).min_by(|a, c| a.left().total_cmp(&c.left())).expect("le nom de Décor");
        all.iter().filter(|(t, r)| t == glyph && (r.center().y - name.center().y).abs() < 5.0 && r.right() <= name.left() + 1.0).map(|(_, r)| r.center()).next()
    };
    let down = triangle(&b, egui_phosphor::regular::CARET_DOWN).expect("le triangle de Décor");
    assert!(!b.app.track_is_folded(1));
    clic(&mut b, down, Modifiers::NONE);
    assert!(b.app.track_is_folded(1) && !b.app.track_is_folded(0) && !b.app.track_is_folded(2), "seule cette piste est repliée");
    assert!(b.app.doc.panels[0].layers[2].collapsed);
    // Replié, le triangle regarde à droite, et rouvre.
    let right = triangle(&b, egui_phosphor::regular::CARET_RIGHT).expect("le triangle replié");
    clic(&mut b, right, Modifiers::NONE);
    assert!(!b.app.track_is_folded(1));
    // Alt + clic : toutes les pistes se replient (la piste courante comprise : le réglage est éteint).
    b.app.settings.montage_auto_open = false;
    b.chauffer(3);
    let down = triangle(&b, egui_phosphor::regular::CARET_DOWN).expect("le triangle");
    clic(&mut b, down, ALT);
    assert!((0..4).all(|r| b.app.track_is_folded(r)), "toutes les pistes sont repliées");
    // Alt + clic encore : toutes se déplient.
    let right = triangle(&b, egui_phosphor::regular::CARET_RIGHT).expect("le triangle replié");
    clic(&mut b, right, ALT);
    assert!((0..4).all(|r| !b.app.track_is_folded(r)));
}

#[test]
fn a_double_click_next_to_the_name_folds_the_track_and_on_the_name_renames_it() {
    let mut b = banc_montage();
    let name = textes(&b).iter().filter(|(t, _)| t == "Décor").map(|(_, r)| *r).min_by(|a, c| a.left().total_cmp(&c.left())).expect("le nom");
    // Sur le nom : renommer, comme avant.
    double_clic(&mut b, pos2(name.left() + 4.0, name.center().y));
    assert!(b.app.layer_rename.is_some(), "le double-clic sur le nom le renomme");
    b.app.layer_rename = None;
    b.chauffer(3);
    // À côté du nom (dans la colonne, à droite du texte) : replier.
    assert!(!b.app.track_is_folded(1));
    b.image(Vec::new(), 0.6); // pas de triple clic avec le précédent
    double_clic(&mut b, pos2(name.right() + 14.0, name.center().y));
    assert!(b.app.layer_rename.is_none());
    assert!(b.app.track_is_folded(1), "le double-clic à côté du nom replie la piste");
}

#[test]
fn the_layout_does_not_move_under_the_pointer_during_a_gesture() {
    let mut b = banc_montage();
    b.app.set_all_tracks_folded(true);
    b.chauffer(4);
    let l = lieu(&b);
    // Tout est replié (« Replier toutes les pistes » replie aussi la piste du calque courant).
    assert!((0..4).all(|r| b.app.track_is_folded(r)));
    let total = b.app.montage_rows(300.0).layers_h();
    // Presse sur le dessin de « Décor » (replié) : la disposition ne bouge pas tant que la souris est tenue.
    let p = pos2(l.x(2), y_in(&b, &l, 1, 4.0));
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![pointer_button(p, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=4 {
        b.image(vec![Event::PointerMoved(p + egui::vec2(k as f32, 0.0))], 1.0 / 60.0);
    }
    assert_eq!(b.app.montage_rows(300.0).layers_h(), total, "pendant le geste, rien ne glisse sous la souris");
    b.image(vec![pointer_button(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(4);
    assert_eq!(b.app.cur_layer, 2, "le calque pressé est devenu courant");
    assert!(!b.app.track_is_folded(1), "souris relâchée, la piste du nouveau calque courant se déplie");
    assert!(b.app.track_is_folded(0) && b.app.track_is_folded(2), "les autres restent repliées");
}

#[test]
fn idle_costs_nothing_more_with_folded_tracks() {
    let mut b = banc_montage();
    b.app.set_all_tracks_folded(true);
    b.chauffer(6);
    let repos: Vec<std::time::Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "les pistes repliées ne font pas repeindre : {repos:?}");
    // Les hauteurs se calculent en quelques microsecondes, même pour un gros film : 30 calques dans 20 plans.
    let names: Vec<String> = (0..30).map(|i| format!("C{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let plans: Vec<&[&str]> = (0..20).map(|_| refs.as_slice()).collect();
    let mut doc = doc_with(&plans);
    doc.panels[3].layers[5].collapsed = true;
    let start = std::time::Instant::now();
    let mut rows = 0;
    for _ in 0..2_000 {
        rows += lanes(&doc.panels, 0, 0, 30, None).len();
    }
    let per_call = start.elapsed().as_secs_f64() * 1000.0 / 2_000.0;
    assert_eq!(rows, 2_000 * 30);
    assert!(per_call < 1.0, "{per_call:.3} ms par calcul des pistes");
}

/// Des images des pistes (toutes dépliées, mélange, toutes repliées) dans le dossier `SHOT_CAPTURE_DIR`, pour en
/// vérifier la mise en page à l'œil. Sans carte graphique : rien.
#[test]
#[ignore]
fn pistes_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut b = banc_taille(film(), true, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = PPF;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 3, 6);
    b.garder_formes = true;
    b.chauffer(8);
    let l = lieu(&b);
    let blur = {
        let mut e = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
        for (f, v) in [(2, 3.0), (10, 6.0), (14, 9.0)] {
            e.set_key(f, vec![v]);
        }
        e
    };
    b.app.doc.panels[0].layers[3].fx = vec![blur];
    b.app.select_fx(0);
    b.app.doc.panels[0].layers[2].opacity = 0.6;
    let sauver = |b: &mut Banc, nom: &str, top: f32| {
        b.chauffer(6);
        let Some(rgba) = b.capturer() else { return println!("PISTES pas de carte graphique : {nom} ignorée") };
        let img = image::RgbaImage::from_raw(1920, 1080, rgba).expect("image");
        let (x, y, w, h) = (0u32, (top - 70.0).max(0.0) as u32, 760u32, 330u32);
        let crop = image::imageops::crop_imm(&img, x, y, w, h).to_image();
        let big = image::imageops::resize(&crop, w * 2, h * 2, image::imageops::FilterType::Nearest);
        big.save(dossier.join(nom)).expect("png");
        println!("PISTES {nom}");
    };
    sauver(&mut b, "pistes-1-depliees.png", l.layers_top);
    b.app.set_track_folded(1, true);
    b.app.set_track_folded(3, true);
    b.chauffer(6);
    let l2 = lieu(&b);
    sauver(&mut b, "pistes-2-melange.png", l2.layers_top);
    b.app.set_all_tracks_folded(true);
    b.chauffer(6);
    let l3 = lieu(&b);
    sauver(&mut b, "pistes-3-repliees.png", l3.layers_top);
}