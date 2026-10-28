//! Pistes du montage (Anthony, 2026-10-20, puis 2026-10-28 : « seul le calque actif est déplié ») : deux hauteurs de piste (dépliée,
//! repliée), un seul calque actif déplié, tout replier / tout déplier à la main, les dossiers qui cachent les pistes de leurs calques,
//! le mode « cachés » avec sa ligne « N autres calques », les gestes qui marchent sur une bande fine.
//!
//! Adaptation du 2026-10-28 (branche `timeline-repliee`) : les essais d'avant lisaient `Layer::collapsed` comme état des pistes (replis
//! communs ou propres à chaque plan, enregistrés avec le projet). Le montage ne lit plus ce drapeau : l'état est de l'affichage, par poste
//! (calque actif + pistes dépliées à la main), et par défaut tout est replié sauf le calque actif. Les essais de replis par plan, de
//! l'état enregistré et de l'état partagé avec la timeline sont remplacés par leurs équivalents (rien n'est écrit dans le projet, la
//! timeline de l'animation garde ses lignes). Les losanges d'une bande fine sont devenus de petits traits qu'on ne prend pas : leurs
//! essais disent ce qui reste (les bords, les limites, le milieu d'un dessin).
//!
//! Les premiers essais sont purs (pistes, hauteurs, hits) ; les derniers passent par la vraie fenêtre sans écran (`Banc`) : on retrouve
//! où les losanges sont tombés à l'écran d'après les formes peintes, puis on clique et on glisse avec le pointeur.

use eframe::egui::{self, epaint::Shape, pos2, Color32, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::app::App;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::handles::Handle;
use crate::montagerows::{layer_in_row, LAYER_H};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes::{lanes_in, row_hidden, Inactifs, Replis, FOLDED_H, MORE_H};
use crate::poskeys::PosKey;
use crate::workspace::Espace;

pub(crate) const PPF: f32 = 12.0;

// ------------------------------------------------------------------------------------ l'outillage

/// Un film dont chaque plan a ces calques, donnés du haut vers le bas.
pub(crate) fn doc_with(plans: &[&[&str]]) -> Document {
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
pub(crate) fn app_with(name: &str, doc: Document) -> App {
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
fn by_default_only_the_active_layer_is_unfolded_and_the_others_are_thin_bands() {
    let mut app = app_with("defaut", doc_with(&[&["A", "B", "C"], &["X", "Y"]]));
    // Le calque courant est « C » (rang 0, rangée 2) : après le relâchement de la souris, il est seul déplié.
    app.pistes_follow_current(false);
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, FOLDED_H), (2, LAYER_H)]);
    assert!(app.settings.montage_inactifs == Inactifs::Replies, "d'office : repliés");
    let l = app.montage_rows(300.0).lanes;
    assert_eq!(l[2].top, 2.0 * FOLDED_H, "les bandes fines sont au-dessus, serrées");
    assert!(FOLDED_H <= 11.0 && FOLDED_H >= 8.0, "une bande de 8 à 9 points de haut une fois les filets retirés : {FOLDED_H}");
    assert!(FOLDED_H < LAYER_H / 3.0);
}

#[test]
fn a_closed_folder_hides_the_tracks_of_its_layers_and_is_itself_a_thin_track() {
    let mut app = app_with("dossier-ferme", doc_with(&[&["A"]]));
    app.doc.panels[0].layers = stack_with_folder(true);
    app.pistes_follow_current(false);
    // Le calque courant est « Libre » (rang 0, tout en bas, rangée 3) ; le dossier fermé est une bande fine, C1 et C2 n'ont plus de piste.
    assert_eq!(heights(&app), [(0, FOLDED_H), (3, LAYER_H)]);
    assert!(row_hidden(&app.doc.panels, 0, 0, 1) && row_hidden(&app.doc.panels, 0, 0, 2) && !row_hidden(&app.doc.panels, 0, 0, 0));
    // Ouvert : tout revient, en bandes fines sauf le calque actif.
    app.doc.panels[0].layers[3].collapsed = false;
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H, FOLDED_H, FOLDED_H, LAYER_H]);
    // Le calque courant d'un dossier fermé garde sa piste (comme dans le panneau Calques).
    app.doc.panels[0].layers[3].collapsed = true;
    app.cur_layer = 2;
    let rows: Vec<usize> = heights(&app).iter().map(|l| l.0).collect();
    assert_eq!(rows, [0, 1, 3], "C1 (rang 2, rangée 1) reste");
}

#[test]
fn the_active_layer_of_another_plan_unfolds_its_row_in_every_plan() {
    let doc = doc_with(&[&["A", "B"], &["X", "Y"]]);
    let (plan, layer) = (doc.panels[1].id, doc.panels[1].layers[0].id); // « Y », rangée 1 du plan 2
    let l = lanes_in(&doc.panels, 0, 0, 2, Some((plan, layer)), &Replis::default());
    assert_eq!(l.iter().map(|x| x.h).collect::<Vec<_>>(), [FOLDED_H, LAYER_H], "la rangée de « Y » est dépliée dans tous les plans");
}

// ------------------------------------------------------------------------------------ replier, déplier à la main

#[test]
fn unfolding_or_folding_by_hand_is_display_only_and_writes_nothing_in_the_project() {
    let mut app = app_with("main-1", doc_with(&[&["A", "B", "C"], &["X", "Y", "Z"], &["P"]]));
    app.settings.montage_auto_open = false;
    app.modified = false;
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, FOLDED_H), (2, FOLDED_H)], "rien n'est actif : tout est en bandes fines");
    assert!(app.set_track_folded(1, false));
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, LAYER_H), (2, FOLDED_H)]);
    assert!(!app.modified, "de l'affichage : le projet n'est pas modifié");
    assert!(app.doc.panels.iter().flat_map(|p| &p.layers).all(|l| !l.collapsed), "le drapeau enregistré (`replie`) n'est pas touché");
    assert!(app.set_track_folded(1, true));
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H; 3]);
    assert!(!app.set_track_folded(7, true), "une rangée qui n'existe pas ne fait rien");
}

#[test]
fn fold_all_and_unfold_all() {
    let mut app = app_with("tout", doc_with(&[&["A", "B", "C"]]));
    app.pistes_follow_current(false);
    let steps = app.history.names().0.len();
    assert_eq!(app.set_all_tracks_folded(false), 2, "les deux bandes fines se déplient, le calque actif l'est déjà");
    assert!(heights(&app).iter().all(|h| h.1 == LAYER_H));
    assert_eq!(app.set_all_tracks_folded(true), 3, "tout se replie, même le calque actif");
    assert!(heights(&app).iter().all(|h| h.1 == FOLDED_H));
    assert_eq!(app.history.names().0.len(), steps, "replier est de l'affichage : pas d'étape d'annulation");
}

#[test]
fn the_action_ids_do_the_same() {
    let mut app = app_with("actions", doc_with(&[&["A", "B"]]));
    app.cur_layer = 1; // « A », rangée 0
    app.pistes_follow_current(false);
    assert_eq!(heights(&app), [(0, LAYER_H), (1, FOLDED_H)]);
    app.run_action("piste.basculer");
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, FOLDED_H)]);
    app.run_action("piste.basculer");
    assert_eq!(heights(&app), [(0, LAYER_H), (1, FOLDED_H)]);
    app.run_action("piste.deplier_tout");
    assert_eq!(heights(&app), [(0, LAYER_H), (1, LAYER_H)]);
    app.run_action("piste.replier_tout");
    assert_eq!(heights(&app), [(0, FOLDED_H), (1, FOLDED_H)]);
    app.run_action("piste.auto_deplier");
    assert!(!app.settings.montage_auto_open);
    app.run_action("piste.auto_deplier");
    assert!(app.settings.montage_auto_open);
    assert!(crate::keymap::ACTIONS.iter().all(|a| a.id != "piste.replis_par_plan"), "le choix commun / par plan n'existe plus");
}

// ------------------------------------------------------------------------------------ le calque actif

#[test]
fn the_current_layer_becomes_active_after_the_release_and_the_old_one_folds() {
    let mut app = app_with("courant", doc_with(&[&["A", "B", "C"]]));
    // Souris tenue : rien n'est actif encore, rien ne bouge (les pistes d'en dessous glisseraient sous le pointeur).
    app.pistes_follow_current(true);
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H; 3]);
    // Relâchée : le calque courant (rang 0 = rangée 2) se déplie.
    app.pistes_follow_current(false);
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H, FOLDED_H, LAYER_H]);
    // Un autre calque devient courant : il s'ouvre quand la souris est relâchée, l'ancien se referme.
    app.select_layer(1);
    app.pistes_follow_current(true);
    assert_eq!(heights(&app)[2].1, LAYER_H, "pendant le geste, la disposition reste");
    app.pistes_follow_current(false);
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H, LAYER_H, FOLDED_H]);
    // Le réglage éteint : tout est en bandes fines.
    app.settings.montage_auto_open = false;
    assert_eq!(heights(&app).iter().map(|h| h.1).collect::<Vec<_>>(), [FOLDED_H; 3]);
}

#[test]
fn folding_the_active_track_by_hand_keeps_it_folded_until_the_current_layer_changes() {
    let mut app = app_with("main-actif", doc_with(&[&["A", "B", "C"]]));
    app.pistes_follow_current(false);
    assert!(!app.track_is_folded(2), "le calque courant (rang 0, rangée 2) est déplié");
    app.toggle_track_fold(2, false);
    assert!(app.track_is_folded(2), "replié à la main");
    app.pistes_follow_current(false);
    assert!(app.track_is_folded(2), "le calque courant n'a pas changé : il reste replié");
    app.select_layer(1);
    app.pistes_follow_current(false);
    assert!(!app.track_is_folded(1), "le nouveau calque courant s'ouvre");
    assert!(app.track_is_folded(2), "et l'ancien se replie");
    // Une piste dépliée à la main le reste quand le calque actif change ; le nouvel actif se déplie, même s'il avait été replié à la main.
    app.set_track_folded(0, false);
    app.select_layer(0);
    app.pistes_follow_current(false);
    assert!(!app.track_is_folded(2), "« C » redevient actif : il se déplie");
    assert!(app.track_is_folded(1), "« B » se replie");
    assert!(!app.track_is_folded(0), "« A », dépliée à la main, le reste");
    // Allumer le réglage remet le calque courant dans l'état « ouvert ».
    app.settings.montage_auto_open = false;
    app.toggle_auto_open();
    app.pistes_follow_current(false);
    assert!(app.settings.montage_auto_open && !app.track_is_folded(2));
}

#[test]
fn the_timeline_of_the_animation_keeps_its_own_reduced_rows() {
    use crate::layerheads::{rows_of, ROW_FULL, ROW_SMALL};
    let mut app = app_with("timeline", doc_with(&[&["A", "B", "C"]]));
    app.pistes_follow_current(false);
    // Les pistes du montage se replient : la timeline de l'animation (tous les calques dépliés) n'en sait rien.
    app.set_all_tracks_folded(true);
    let rows = rows_of(app.layers(), &app.doc.groups);
    assert!(rows.iter().all(|r| r.h == ROW_FULL), "la timeline garde tous ses calques dépliés");
    // Une ligne réduite de la timeline (`collapsed`) ne change rien aux pistes du montage.
    let before = heights(&app);
    app.layers_mut()[1].collapsed = true;
    assert_eq!(heights(&app), before);
    let rows = rows_of(app.layers(), &app.doc.groups);
    assert_eq!(rows.iter().filter(|r| r.h == ROW_SMALL).count(), 1);
}

// ------------------------------------------------------------------------------------ les calques cachés

#[test]
fn hidden_mode_shows_only_the_active_layer_and_a_line_that_shows_the_others() {
    let mut app = app_with("caches", doc_with(&[&["A", "B", "C", "D"], &["X"]]));
    app.settings.montage_inactifs = Inactifs::Caches;
    app.cur_layer = 2; // « B », rangée 1
    app.pistes_follow_current(false);
    let lanes = app.montage_rows(300.0).lanes;
    assert_eq!(lanes.iter().map(|l| (l.row, l.h)).collect::<Vec<_>>(), [(1, LAYER_H), (crate::pistes::MORE_ROW, MORE_H)], "seul le calque actif, et la ligne « N autres »");
    assert!(lanes[1].is_more());
    assert_eq!(crate::pistes::others_count(&app.doc.panels, 0, 2, 4, app.pistes_auto_open(), &app.pistes_replis()), 3);
    // Un clic sur la ligne les montre en bandes fines, un autre les cache.
    app.pistes.autres_montres = true;
    assert_eq!(app.montage_rows(300.0).lanes.iter().map(|l| (l.row, l.h)).collect::<Vec<_>>(), [(0, FOLDED_H), (1, LAYER_H), (2, FOLDED_H), (3, FOLDED_H), (crate::pistes::MORE_ROW, MORE_H)]);
    // Aucun autre calque : pas de ligne.
    let mut seul = app_with("caches-seul", doc_with(&[&["A"]]));
    seul.settings.montage_inactifs = Inactifs::Caches;
    seul.pistes_follow_current(false);
    assert_eq!(heights(&seul), [(0, LAYER_H)]);
}

#[test]
fn the_choice_of_inactive_layers_is_a_setting_of_the_workstation_with_a_preferences_section() {
    let mut app = app_with("prefs-inactifs", doc_with(&[&["A", "B"]]));
    app.modified = false;
    app.set_inactifs(Inactifs::Caches);
    assert_eq!(app.settings.montage_inactifs, Inactifs::Caches);
    assert!(!app.modified, "un réglage du poste : rien dans le projet");
    // Enregistré avec les réglages, relu ; un vieux réglage (sans la clé, ou avec l'ancien « repliage_pistes ») donne « repliés ».
    let mut s = crate::settings::Settings::default();
    s.montage_inactifs = Inactifs::Caches;
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("calques_non_actifs"), "{text}");
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.montage_inactifs, Inactifs::Caches);
    let vieux: crate::settings::Settings = toml::from_str("repliage_pistes = \"par_plan\"\n").unwrap();
    assert_eq!(vieux.montage_inactifs, Inactifs::Replies);
    let defaut = toml::to_string(&crate::settings::Settings::default()).unwrap();
    assert!(!defaut.contains("calques_non_actifs"), "la valeur d'office ne s'écrit pas");
    assert!(Inactifs::Replies.est_replie() && crate::pistes::INACTIFS_REPLIES.ends_with('.') && crate::pistes::INACTIFS_CACHES.ends_with('.'));
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
fn the_handles_are_found_in_a_thin_band_but_the_diamonds_are_not() {
    let mut app = hit_app("hits");
    app.set_track_folded(1, false); // « Fond » (rangée 1) déplié à la main ; « Perso » (rangée 0) est une bande fine
    let rows = app.montage_rows(300.0);
    let lane = rows.lanes[0];
    assert!(lane.folded && lane.h == FOLDED_H);
    let top = 10.0;
    let x0 = 100.0;
    let blocks = vec![Rect::from_min_max(pos2(x0 + 1.0, 50.0), pos2(x0 + 24.0 * PPF - 1.0, 100.0))];
    let x = |f: usize| x0 + f as f32 * PPF + PPF * 0.5;
    let y = |d: f32| top + rows.layers_top() + d;
    // Les clés d'effets et de position d'une bande fine sont de petits traits : on ne les prend pas (un clic choisit le calque).
    assert_eq!(app.montage_fx_key_at(pos2(x(10), y(2.0)), top, &blocks, PPF, &rows), None);
    for d in [1.0, 3.0, 5.0, 8.0] {
        assert_eq!(app.montage_pos_key_at(pos2(x(10), y(d)), top, &blocks, PPF, &rows), None, "d = {d}");
    }
    // Les poignées du dessin, comme dans la timeline : le milieu d'un dessin, la limite entre deux ; pas de barre qui décale le calque.
    let handle = |px: f32, d: f32| app.montage_layer_handle_at(pos2(px, y(d)), top, &blocks, PPF, &rows).map(|h| h.2);
    assert!(matches!(handle(x(6), 3.0), Some(Handle::Key { start: 0, end: 12, .. })), "le milieu du premier dessin");
    assert!(matches!(handle(x0 + 12.0 * PPF, 3.0), Some(Handle::Boundary { at: 12, .. })), "la limite entre les deux dessins");
    assert!(matches!(handle(x(6), lane.h - 2.0), Some(Handle::Key { .. })), "tout en bas de la bande : encore le dessin, jamais la barre");
    // La rangée sous le point : la bande fine, puis la suivante (dépliée) juste dessous.
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
fn the_geometry_of_an_unfolded_track_keeps_its_diamonds_and_its_bar() {
    let mut app = hit_app("hits2");
    app.set_track_folded(0, false);
    let rows = app.montage_rows(300.0);
    let lane = rows.lanes[0];
    assert!(!lane.folded);
    assert_eq!((lane.fx_hit_max(), lane.pos_y(), lane.pos_hit()), (crate::fxkeys::STRIP, LAYER_H * 0.5, crate::poskeysui::HIT_H));
    // La barre du calque (5 points) est dessinée à `BAR_OFF` du bas de la piste (le liseré du calque de couleur est dessous) et se prend
    // à partir de son bord haut, à un point près.
    assert_eq!(lane.bar_from(), LAYER_H - crate::montagerows::BAR_OFF - crate::montagerows::BAR_H - 2.0);
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
    app.doc.panels[0].layers[3].collapsed = true;
    assert_eq!(app.keys_in_rect(0..=3, 0, 5).len(), 1, "le dossier fermé cache C1 et C2 : seule « Libre » (courante) reste");
}

// ------------------------------------------------------------------------------------ la vraie fenêtre

/// Quatre calques dans le plan 1 (« Perso », « Décor », « Fond », « Vide », de haut en bas), un seul dans le
/// plan 2. Des clés de position : Perso 4, 10, 16 ; Décor 6, 12 ; Fond 5, 15 ; « Vide » n'a aucun dessin.
pub(crate) fn film() -> Document {
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

pub(crate) fn banc_montage() -> Banc {
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
pub(crate) fn losanges(b: &Banc, fill: Color32) -> Vec<Rect> {
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
pub(crate) fn textes(b: &Banc) -> Vec<(String, Rect)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes_de(&s.shape, &mut out);
    }
    out
}

/// Où sont les repères de l'écran : l'abscisse de l'image 4 du film et le haut des pistes de calques.
pub(crate) struct Lieu {
    pub x4: f32,
    /// Ordonnée écran du haut des pistes de calques.
    pub layers_top: f32,
}

impl Lieu {
    pub fn x(&self, g: usize) -> f32 {
        self.x4 + (g as f32 - 4.0) * PPF
    }
}

/// Les repères de l'écran, d'après les losanges verts du calque actif (le seul déplié, donc le seul à en avoir).
pub(crate) fn lieu(b: &Banc) -> Lieu {
    let rows = b.app.montage_rows(300.0);
    let lane = *rows.lanes.iter().find(|l| !l.folded).expect("une piste dépliée");
    let layers = &b.app.doc.panels[b.app.cur_panel].layers;
    let k = layer_in_row(layers, lane.row).expect("son calque");
    let f0 = layers[k].pos_keys.first().expect("une clé de position").frame;
    let mut l = losanges(b, crate::theme::POIGNEE_CALQUE);
    l.sort_by(|a, c| a.center().x.total_cmp(&c.center().x));
    let first = l[0].center();
    Lieu { x4: first.x - (f0 as f32 - 4.0) * PPF, layers_top: first.y - lane.pos_y() - lane.top }
}

/// L'ordonnée écran de `d` points sous le haut de la piste de la rangée `row`.
pub(crate) fn y_in(b: &Banc, l: &Lieu, row: usize, d: f32) -> f32 {
    let rows = b.app.montage_rows(300.0);
    l.layers_top + rows.lanes.iter().find(|x| x.row == row).expect("la piste est visible").top + d
}

pub(crate) fn pointer_button(p: Pos2, pressed: bool, mods: Modifiers) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: mods }
}

pub(crate) fn clic(b: &mut Banc, p: Pos2, mods: Modifiers) {
    b.touches = mods;
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![pointer_button(p, true, mods)], 1.0 / 60.0);
    b.image(vec![pointer_button(p, false, mods)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
}

pub(crate) fn double_clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    for _ in 0..2 {
        b.image(vec![pointer_button(p, true, Modifiers::NONE)], 1.0 / 60.0);
        b.image(vec![pointer_button(p, false, Modifiers::NONE)], 1.0 / 60.0);
    }
    b.chauffer(3);
}

/// Prend en `a`, tire jusqu'en `z` (six pas), lâche.
pub(crate) fn glisser(b: &mut Banc, a: Pos2, z: Pos2) {
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![pointer_button(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![pointer_button(z, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
}

pub(crate) fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

#[test]
fn only_the_active_layer_is_drawn_tall_with_its_diamonds_and_the_others_are_thin() {
    let mut b = banc_montage();
    let rows = b.app.montage_rows(300.0);
    assert_eq!(rows.lanes.iter().map(|l| l.h).collect::<Vec<_>>(), [LAYER_H, FOLDED_H, FOLDED_H, FOLDED_H], "Perso, le calque courant, est seul déplié");
    let diamonds = losanges(&b, crate::theme::POIGNEE_CALQUE);
    assert_eq!(diamonds.len(), 3, "trois clés de position de Perso ; celles de Décor et de Fond sont de petits traits : {diamonds:?}");
    assert!(diamonds.iter().all(|d| d.height() > 8.0), "des losanges de piste dépliée");
    // Les noms des calques repliés restent lisibles dans la colonne (en petit).
    let t = textes(&b);
    for nom in ["Perso", "Décor", "Fond", "Vide"] {
        assert!(t.iter().any(|(s, _)| s == nom), "« {nom} » manque : {t:?}");
    }
    let (decor, perso) = (t.iter().find(|(s, _)| s == "Décor").unwrap().1, t.iter().find(|(s, _)| s == "Perso").unwrap().1);
    assert!(decor.height() < perso.height(), "le nom d'une bande fine est plus petit : {} contre {}", decor.height(), perso.height());
    // Replier l'un après l'autre : les pistes d'en dessous remontent.
    let l = lieu(&b);
    let avant = y_in(&b, &l, 3, 0.0);
    b.app.set_track_folded(1, false);
    b.chauffer(4);
    assert!((y_in(&b, &lieu(&b), 3, 0.0) - avant - (LAYER_H - FOLDED_H)).abs() < 0.6, "déplier « Décor » à la main pousse « Vide » vers le bas");
}

#[test]
fn clicking_the_name_of_a_folded_layer_makes_it_active_and_unfolds_it() {
    let mut b = banc_montage();
    assert_eq!(b.app.cur_layer, 3);
    let nom = textes(&b).iter().filter(|(t, _)| t == "Décor").map(|(_, r)| *r).next().expect("le nom de Décor");
    clic(&mut b, nom.center(), Modifiers::NONE);
    assert_eq!(b.app.cur_layer, 2, "« Décor » est le rang 2 (de bas en haut : Vide, Fond, Décor, Perso)");
    assert!(!b.app.track_is_folded(1), "Décor se déplie");
    assert!(b.app.track_is_folded(0), "Perso, l'ancien actif, se replie");
    assert_eq!(b.app.montage_rows(300.0).lanes.iter().map(|l| l.h).collect::<Vec<_>>(), [FOLDED_H, LAYER_H, FOLDED_H, FOLDED_H]);
    // Le panneau Calques suit : c'est le même calque courant ; et choisir dans ce panneau déplie la piste.
    b.app.select_layer(1);
    b.chauffer(4);
    assert!(!b.app.track_is_folded(2) && b.app.track_is_folded(1), "choisir dans le panneau Calques déplie la piste, et replie l'autre");
    assert!(b.app.doc.panels[0].layers.iter().all(|l| !l.collapsed), "rien n'est écrit dans les calques");
}

#[test]
fn the_gestures_of_the_timeline_work_on_a_thin_band() {
    let mut b = banc_montage();
    let l = lieu(&b);
    assert!(b.app.montage_rows(300.0).lanes[1..].iter().all(|x| x.folded));
    // 1. Un clic sur le dessin de « Décor » (bande fine, rangée 1) le choisit, et son calque devient courant.
    let start = etapes(&b);
    let p = pos2(l.x(2), y_in(&b, &l, 1, 5.0));
    clic(&mut b, p, Modifiers::NONE);
    assert_eq!(b.app.cur_layer, 2, "« Décor » est le rang 2 (de bas en haut : Vide, Fond, Décor, Perso)");
    assert_eq!(b.app.key_sel.len(), 1, "{:?}", b.app.key_sel);
    assert_eq!(etapes(&b), start, "choisir n'est pas une étape d'annulation");
    // 2. La limite entre les deux dessins de « Perso » (maintenant une bande fine, rangée 0 ; entre les images 11 et 12) se tire :
    // l'un s'allonge, l'autre raccourcit.
    let l = lieu(&b);
    let before = etapes(&b);
    let boundary = pos2(l.x(12) - PPF * 0.5, y_in(&b, &l, 0, 5.0));
    glisser(&mut b, boundary, pos2(boundary.x + 3.0 * PPF, boundary.y));
    let perso = &b.app.doc.panels[0].layers[3];
    assert!(perso.frames[12..15].iter().all(|f| *f == perso.frames[0]), "le premier dessin tient 3 images de plus : {:?}", perso.frames);
    assert_ne!(perso.frames[15], perso.frames[0]);
    assert_eq!(etapes(&b), before + 1, "un geste, une étape d'annulation");
    b.app.undo();
    b.chauffer(3);
    let perso = &b.app.doc.panels[0].layers[3];
    assert!(perso.frames[12] != perso.frames[0] && perso.frames[11] == perso.frames[0]);
    // 3. Un losange de position du calque actif (« Perso », devenu courant au geste d'avant, image 10) se glisse à l'image 11, dans sa
    // piste dépliée (ceux des bandes fines ne se prennent pas : voir `the_handles_are_found_in_a_thin_band_but_the_diamonds_are_not`).
    b.app.select_layer(3);
    b.chauffer(4);
    let l = lieu(&b);
    let before = etapes(&b);
    let cy = y_in(&b, &l, 0, LAYER_H * 0.5);
    glisser(&mut b, pos2(l.x(10), cy), pos2(l.x(11), cy));
    let keys: Vec<usize> = b.app.doc.panels[0].layers[3].pos_keys.iter().map(|k| k.frame).collect();
    assert_eq!(keys, vec![4, 11, 16]);
    assert!(etapes(&b) > before);
    // 4. Le choix au rectangle : parti de « Vide » (rangée 3, vide, bande fine), jusqu'à « Fond » (rangée 2, bande fine).
    b.app.key_sel.clear();
    let l = lieu(&b);
    let (from, to) = (pos2(l.x(20), y_in(&b, &l, 3, 5.0)), pos2(l.x(1), y_in(&b, &l, 2, 5.0)));
    glisser(&mut b, from, to);
    let mut rows: Vec<usize> = b.app.key_sel.iter().map(|k| b.app.doc.panels[0].layers.iter().position(|x| x.id == k.layer).unwrap()).collect();
    rows.sort();
    rows.dedup();
    assert_eq!(rows, vec![1], "seule « Fond » (rang 1) est dans le rectangle ; « Vide » n'a pas de dessin");
}

#[test]
fn a_double_click_next_to_the_name_folds_the_track_and_on_the_name_renames_it() {
    let mut b = banc_montage();
    let nom = textes(&b).iter().filter(|(t, _)| t == "Perso").map(|(_, r)| *r).min_by(|a, c| a.left().total_cmp(&c.left())).expect("le nom");
    // Sur le nom : renommer, comme avant.
    double_clic(&mut b, pos2(nom.left() + 4.0, nom.center().y));
    assert!(b.app.layer_rename.is_some(), "le double-clic sur le nom le renomme");
    b.app.layer_rename = None;
    b.chauffer(3);
    // À côté du nom (dans la colonne, à droite du texte) : replier la piste du calque actif.
    assert!(!b.app.track_is_folded(0));
    b.image(Vec::new(), 0.6); // pas de triple clic avec le précédent
    double_clic(&mut b, pos2(nom.right() + 14.0, nom.center().y));
    assert!(b.app.layer_rename.is_none());
    assert!(b.app.track_is_folded(0), "le double-clic à côté du nom replie la piste");
}

#[test]
fn the_layout_does_not_move_under_the_pointer_during_a_gesture() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let total = b.app.montage_rows(300.0).layers_h();
    // Presse sur le dessin de « Décor » (bande fine) : la disposition ne bouge pas tant que la souris est tenue.
    let p = pos2(l.x(2), y_in(&b, &l, 1, 5.0));
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![pointer_button(p, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=4 {
        b.image(vec![Event::PointerMoved(p + egui::vec2(k as f32, 0.0))], 1.0 / 60.0);
    }
    assert_eq!(b.app.montage_rows(300.0).layers_h(), total, "pendant le geste, rien ne glisse sous la souris");
    assert!(!b.app.track_is_folded(0), "Perso est encore déplié");
    b.image(vec![pointer_button(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(4);
    assert_eq!(b.app.cur_layer, 2, "le calque pressé est devenu courant");
    assert!(!b.app.track_is_folded(1), "souris relâchée, la piste du nouveau calque courant se déplie");
    assert!(b.app.track_is_folded(0) && b.app.track_is_folded(2), "les autres sont repliées");
}

#[test]
fn idle_costs_nothing_more_with_thin_bands_and_the_lanes_are_cheap_to_compute() {
    let mut b = banc_montage();
    b.chauffer(6);
    let repos: Vec<std::time::Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "les bandes fines ne font pas repeindre : {repos:?}");
    // Les hauteurs se calculent en quelques microsecondes, même pour un gros film : 30 calques dans 20 plans.
    let names: Vec<String> = (0..30).map(|i| format!("C{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let plans: Vec<&[&str]> = (0..20).map(|_| refs.as_slice()).collect();
    let doc = doc_with(&plans);
    let replis = Replis::default();
    let start = std::time::Instant::now();
    let mut rows = 0;
    for _ in 0..2_000 {
        rows += lanes_in(&doc.panels, 0, 0, 30, None, &replis).len();
    }
    let per_call = start.elapsed().as_secs_f64() * 1000.0 / 2_000.0;
    assert_eq!(rows, 2_000 * 30);
    assert!(per_call < 1.0, "{per_call:.3} ms par calcul des pistes");
}

/// Des images des pistes (calque actif déplié, bandes fines, calques cachés) dans le dossier `SHOT_CAPTURE_DIR`, pour en vérifier la
/// mise en page à l'œil. Sans carte graphique : rien.
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
    let blur = {
        let mut e = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
        for (f, v) in [(2, 3.0), (10, 6.0), (14, 9.0)] {
            e.set_key(f, vec![v]);
        }
        e
    };
    b.app.doc.panels[0].layers[3].fx = vec![blur];
    b.app.select_fx(0);
    let sauver = |b: &mut Banc, nom: &str| {
        b.chauffer(6);
        let l = lieu(b);
        let Some(rgba) = b.capturer() else { return println!("PISTES pas de carte graphique : {nom} ignorée") };
        let img = image::RgbaImage::from_raw(1920, 1080, rgba).expect("image");
        let (x, y, w, h) = (0u32, (l.layers_top - 210.0).max(0.0) as u32, 760u32, 450u32);
        let crop = image::imageops::crop_imm(&img, x, y, w, h).to_image();
        let big = image::imageops::resize(&crop, w * 2, h * 2, image::imageops::FilterType::Nearest);
        big.save(dossier.join(nom)).expect("png");
        println!("PISTES {nom}");
    };
    sauver(&mut b, "pistes-1-actif.png");
    b.app.set_all_tracks_folded(false);
    sauver(&mut b, "pistes-2-tout-deplie.png");
    b.app.set_all_tracks_folded(true);
    b.app.set_track_folded(0, false);
    b.app.settings.montage_inactifs = Inactifs::Caches;
    sauver(&mut b, "pistes-3-caches.png");
}
