//! Les préréglages d'effets dans l'application (`prereglagesui.rs`) : un calque d'effet posé par
//! préréglage en une étape d'annulation aux noms français, l'aperçu en direct de la fenêtre, les
//! actions de la palette, la pose sur le calque courant ou dans tous les plans.

use eframe::egui;

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::fx::EffectKind;
use crate::prereglages::{Genre, Reglages};

/// Une application sans fenêtre avec un plan de 24 images et un calque de dessin.
fn fixture(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-prereglages-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 6;
    app
}

fn last_step(app: &App) -> String {
    app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

#[test]
fn a_preset_makes_an_effect_layer_at_the_playhead_in_one_undoable_step() {
    let mut app = fixture("impact");
    let before = app.history.names().0.len();
    app.apply_effect_preset(&Reglages { duree: Some(3), ..Reglages::new(Genre::Impact, 24) });
    assert_eq!(app.layers().len(), 2);
    let l = &app.layers()[1];
    assert_eq!(l.name, "Impact frame");
    assert!(l.is_adjustment() && !l.drawable());
    assert_eq!(app.cur_layer, 1, "le nouveau calque est courant");
    // Les images 6, 7 et 8, et pas les autres.
    let on: Vec<usize> = (0..24).filter(|&f| l.effect_active(f)).collect();
    assert_eq!(on, vec![6, 7, 8]);
    assert_eq!(l.fx.len(), 1);
    assert_eq!(l.fx[0].kind, EffectKind::Impact);
    assert!(l.fx[0].key_at(6).is_some(), "les clés partent de la tête de lecture");
    // Une seule étape, avec son nom français.
    assert_eq!(app.history.names().0.len(), before + 1);
    assert_eq!(last_step(&app), "Préréglage d'effet : Impact frame");
    app.undo();
    assert_eq!(app.layers().len(), 1);
    assert_eq!(app.cur_layer, 0);
    app.redo();
    assert_eq!(app.layers().len(), 2);
    assert_eq!(app.layers()[1].fx[0].kind, EffectKind::Impact);
}

#[test]
fn a_second_preset_gets_its_own_name_and_a_stack_that_still_covers_everything() {
    let mut app = fixture("deux");
    app.apply_effect_preset(&Reglages::new(Genre::Impact, 24));
    app.apply_effect_preset(&Reglages::new(Genre::Impact, 24));
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, vec!["Dessin", "Impact frame", "Impact frame 2"]);
    // Le plus récent est tout en haut : il couvre le dessin et le premier effet.
    assert_eq!(app.cur_layer, 2);
}

#[test]
fn a_preset_can_go_on_the_current_layer_instead() {
    let mut app = fixture("sur-calque");
    let r = Reglages { sur_calque: true, duree: Some(4), ..Reglages::new(Genre::FlashBlanc, 24) };
    app.apply_effect_preset(&r);
    assert_eq!(app.layers().len(), 1, "pas de calque neuf");
    let fx = &app.layers()[0].fx;
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].kind, EffectKind::Flash);
    // Rien de visible avant ni après : le flash est éteint hors de ses clés.
    assert_eq!(fx[0].values_at(2)[0], 0.0);
    assert_eq!(fx[0].values_at(20)[0], 0.0);
    assert!((6..10).map(|f| fx[0].values_at(f)[0]).fold(0.0f32, f32::max) > 50.0);
    assert_eq!(last_step(&app), "Préréglage d'effet : Flash blanc");
    app.undo();
    assert!(app.layers()[0].fx.is_empty());
}

#[test]
fn a_folder_layer_refuses_effects_on_itself_and_a_preset_past_the_end_says_so() {
    let mut app = fixture("refus");
    let before = app.history.names().0.len();
    app.cur_frame = 30;
    app.apply_effect_preset(&Reglages::new(Genre::Impact, 24));
    assert_eq!(app.layers().len(), 1);
    assert!(app.status.contains("après la fin du plan"), "{}", app.status);
    assert_eq!(app.history.names().0.len(), before, "pas d'étape d'annulation pour rien");
    app.cur_frame = 3;
    let folder = Layer::new_folder("Dossier");
    app.doc.panels[0].layers.push(folder);
    app.cur_layer = 1;
    app.apply_effect_preset(&Reglages { sur_calque: true, ..Reglages::new(Genre::Impact, 24) });
    assert!(app.status.contains("dossier"), "{}", app.status);
    assert!(app.layers()[1].fx.is_empty());
}

#[test]
fn a_preset_from_a_layer_in_a_folder_joins_the_folder_and_covers_only_it() {
    let mut app = fixture("dossier");
    let mut folder = Layer::new_folder("Dossier");
    app.doc.panels[0].layers[0].parent = Some(folder.id);
    folder.collapsed = false;
    app.doc.panels[0].layers.push(folder);
    app.cur_layer = 0;
    app.apply_effect_preset(&Reglages::new(Genre::Lueur, 24));
    let layers = app.layers();
    assert_eq!(layers.len(), 3);
    let e = layers.iter().find(|l| l.is_adjustment()).unwrap();
    assert_eq!(e.parent, layers.iter().find(|l| l.folder).map(|f| f.id), "rangé dans le dossier");
    assert_eq!(crate::calqueffet::scope_low(layers, layers.iter().position(|l| l.is_adjustment()).unwrap()), 0);
}

#[test]
fn the_timecode_goes_into_every_plan_when_asked() {
    let mut app = fixture("tous");
    let n = app.doc.add_panel_after(0);
    app.doc.panels[n].layers.clear();
    app.doc.panels[n].set_length(10);
    let cel = app.doc.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    l.frames = vec![Some(cel); 10];
    app.doc.panels[n].layers.push(l);
    app.apply_effect_preset(&Reglages { tous_les_plans: true, ..Reglages::new(Genre::Timecode, 24) });
    for p in 0..2 {
        let overlay = app.doc.panels[p].layers.iter().find(|l| l.overlay().is_some()).unwrap_or_else(|| panic!("plan {p} sans timecode"));
        let len = app.doc.panels[p].frame_count();
        assert!((0..len).all(|f| overlay.effect_active(f)), "plan {p} : sur toute sa durée");
    }
    assert_eq!(app.history.names().0.iter().filter(|s| s.contains("Timecode") || s.contains("timecode")).count(), 1, "une seule étape");
    assert_eq!(last_step(&app), "Préréglage d'effet : Timecode");
    app.undo();
    assert!(app.doc.panels.iter().all(|p| p.layers.iter().all(|l| l.overlay().is_none())));
}

#[test]
fn the_dialog_previews_the_layer_it_would_create_without_creating_it() {
    let mut app = fixture("apercu");
    assert!(app.effect_preview_layers().is_none(), "fenêtre fermée : pas d'aperçu");
    let plain = app.build_plan(6, true, true);
    assert!(!crate::calqueffet::has_markers(&plain));
    app.run_action("effet.impact");
    assert!(app.effets_ui.dialog.is_some());
    // La pile d'aperçu a le calque d'effet en plus ; le document, lui, n'a pas bougé.
    let (layers, cur) = app.effect_preview_layers().expect("un aperçu");
    assert_eq!((layers.len(), cur), (2, 0));
    assert!(layers[1].is_adjustment());
    assert_eq!(app.layers().len(), 1);
    // Le plan affiché porte le repère du calque d'aperçu (l'image 6 est dans sa durée).
    let preview = app.build_plan(6, true, true);
    assert!(crate::calqueffet::has_markers(&preview));
    // L'export, lui, ne voit jamais l'aperçu.
    assert!(!crate::calqueffet::has_markers(&app.build_plan(6, false, true)));
    // Changer un réglage de la fenêtre change l'aperçu.
    app.effets_ui.dialog.as_mut().unwrap().r.mode_impact = 3;
    let (layers, _) = app.effect_preview_layers().unwrap();
    assert_eq!(layers[1].fx[0].values_at(6)[1], 3.0);
    // Annuler ferme la fenêtre sans rien poser ; plus d'aperçu.
    app.run_action("effet.fermer");
    assert!(app.effets_ui.dialog.is_none() && app.effect_preview_layers().is_none());
    assert_eq!(app.layers().len(), 1);
    assert!(!crate::calqueffet::has_markers(&app.build_plan(6, true, true)));
}

#[test]
fn the_actions_open_each_preset_apply_it_and_make_an_empty_effect_layer() {
    let mut app = fixture("actions");
    for genre in Genre::ALL {
        app.run_action(genre.action());
        let d = app.effets_ui.dialog.as_ref().unwrap_or_else(|| panic!("{} n'ouvre rien", genre.action()));
        assert_eq!(d.r.genre, genre);
    }
    // Appliquer le dernier ouvert (timecode) : la fenêtre se ferme, le calque est là.
    app.run_action("effet.appliquer");
    assert!(app.effets_ui.dialog.is_none());
    assert!(app.layers().iter().any(|l| l.overlay().is_some()));
    assert_eq!(last_step(&app), "Préréglage d'effet : Timecode");
    // Un calque d'effet vide : il couvre tout le plan, sans effet, courant.
    app.run_action("effet.vide");
    let l = app.layer();
    assert!(l.is_adjustment() && l.fx.is_empty());
    assert!((0..app.frame_count()).all(|f| l.effect_active(f)));
    assert_eq!(app.layers().len(), 3);
    // Les actions sont dans la palette (Ctrl+K), sous leur nom.
    let palette = crate::palette::palette_commands();
    for genre in Genre::ALL {
        assert!(palette.iter().any(|(id, _)| *id == genre.action()), "{} absent de la palette", genre.action());
    }
    assert!(palette.iter().any(|(id, _)| *id == "effet.vide"));
}

#[test]
fn the_dialog_reopens_with_the_last_intensity_and_seed() {
    let mut app = fixture("derniers");
    app.apply_effect_preset(&Reglages { intensite: 1.7, graine: 42, ..Reglages::new(Genre::Impact, 24) });
    app.run_action("effet.tremblement");
    let d = app.effets_ui.dialog.as_ref().unwrap();
    assert_eq!((d.r.genre, d.r.intensite, d.r.graine), (Genre::Tremblement, 1.7, 42));
    // La durée, elle, repart de celle du préréglage.
    assert_eq!(d.r.duree, Genre::Tremblement.default_frames(24));
}

#[test]
fn drawing_tools_leave_an_effect_layer_alone() {
    let mut app = fixture("dessin");
    app.apply_effect_preset(&Reglages::new(Genre::Lueur, 24));
    assert!(!app.layer().drawable());
    assert!(app.layer().effet.is_some());
}
