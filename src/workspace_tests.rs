//! Tests des espaces de travail (phase 4b).

use eframe::egui;

use crate::app::{App, Mode};
use crate::dock::Pane;
use crate::settings::Settings;
use crate::workspace::{self, Espace, Layout, Workspace};

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-espaces-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    // (Un projet est ouvert : sans projet, les commandes d'un projet ne partent pas, `sansprojet.rs`.)
    app.welcome = false;
    (ctx, app)
}

fn has(app: &App, pane: Pane) -> bool {
    app.dock_layout().contains(pane)
}

#[test]
fn builtin_layouts() {
    let all = workspace::builtin();
    let names: Vec<&str> = all.iter().map(|w| w.1).collect();
    assert_eq!(names, vec!["Standard", "Dessin", "Timeline", "Vue d'ensemble"]);
    assert_eq!(all[0].2, Layout::default());
    let dessin = &all[1].2;
    assert!(dessin.tools && !dessin.sheet, "Dessin : plus de place pour dessiner");
    assert!(all[2].2.montage_height > Layout::default().montage_height, "Timeline : bande plus haute");
    assert!(all[3].2.vignettes && !all[3].2.tools, "Vue d'ensemble : vignettes");
    // Chaque disposition a son action dans le fichier de raccourcis.
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for (action, _, _) in &all {
        assert!(map.warnings.iter().all(|w| !w.contains(action)), "{action}");
    }
}

#[test]
fn saving_a_workspace_replaces_one_with_the_same_name() {
    let mut list: Vec<Workspace> = Vec::new();
    let small = Layout { sheet_width: 200.0, ..Layout::default() };
    workspace::save_workspace(&mut list, " Mon dessin ", small.clone());
    workspace::save_workspace(&mut list, "", Layout::default());
    assert_eq!(list.len(), 1, "nom vide : rien");
    assert_eq!(list[0].name, "Mon dessin");
    let wide = Layout { sheet_width: 400.0, ..Layout::default() };
    workspace::save_workspace(&mut list, "MON DESSIN", wide.clone());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].layout, wide);
}

#[test]
fn layouts_are_kept_in_the_settings() {
    let mut s = Settings::default();
    s.layout = Some(Layout { strip: false, timeline_height: 333.0, ..Layout::default() });
    workspace::save_workspace(&mut s.workspaces, "Montage", Layout { tools: false, ..Layout::default() });
    let text = toml::to_string(&s).unwrap();
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.layout, s.layout);
    assert_eq!(back.workspaces, s.workspaces);
    // Disposition incomplète ou absurde dans le fichier : valeurs par défaut, tailles bornées.
    let partial: Layout = toml::from_str("bande = false\nlargeur_fiche = 5.0").unwrap();
    assert!(!partial.strip && partial.tools);
    assert_eq!(partial.sanitized().sheet_width, 160.0);
}

// ---------------------------------------------------------------- espaces liés aux modes (6.1)

#[test]
fn the_three_spaces_have_their_own_starting_layouts() {
    let (storyboard, animation, montage) = (Espace::Storyboard.defaut(), Espace::Animation.defaut(), Espace::Montage.defaut());
    assert_eq!(storyboard, Layout::default(), "le storyboard : la disposition standard");
    assert!(animation.montage_height > storyboard.montage_height, "l'animation : la bande (la timeline) plus haute");
    assert!(animation.sheet_width > storyboard.sheet_width, "et les calques plus larges");
    // Montage : la bande en grand, les panneaux de dessin repliés, la fiche du plan gardée.
    let dock = montage.dock_layout();
    assert!(dock.contains(Pane::Montage) && dock.contains(Pane::Sheet) && dock.contains(Pane::Canvas));
    for gone in [Pane::ToolOptions, Pane::Brushes, Pane::Color, Pane::Layers, Pane::Effects] {
        assert!(!dock.contains(gone), "{gone:?} est replié dans l'espace Montage");
    }
    // Les panneaux de son (arrivés depuis) s'ouvrent dans le montage, pas dans les deux autres espaces.
    assert!(dock.contains(Pane::Bruitages) && dock.contains(Pane::SuggestionsSonores));
    for autre in [&storyboard, &animation] {
        for p in [Pane::Bruitages, Pane::SuggestionsSonores, Pane::References, Pane::XSheet] {
            assert!(!autre.dock_layout().contains(p), "{p:?} reste à la demande");
        }
    }
    assert!(montage.montage_height > animation.montage_height);
    // Chacun est une disposition valable telle quelle.
    for e in Espace::TOUS {
        assert_eq!(e.defaut().dock_layout().clone().sanitized().panes().len(), e.defaut().dock_layout().panes().len());
    }
    // Le montage est une disposition du mode storyboard ; l'animation a son mode.
    assert_eq!(Espace::Storyboard.mode(), Mode::Storyboard);
    assert_eq!(Espace::Montage.mode(), Mode::Storyboard);
    assert_eq!(Espace::Animation.mode(), Mode::Animation);
}

#[test]
fn every_space_has_a_registered_action() {
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for e in Espace::TOUS {
        let info = crate::keymap::action_info(e.action()).unwrap_or_else(|| panic!("{} n'est pas dans keymap::ACTIONS", e.action()));
        assert!(info.implemented);
        assert_eq!(Espace::de_action(e.action()), Some(e));
        assert!(map.warnings.iter().all(|w| !w.contains(e.action())));
    }
    assert!(crate::keymap::action_info("espace.retablir").is_some_and(|a| a.implemented));
    assert_eq!(Espace::de_action("espace.retablir"), None);
}

#[test]
fn switching_mode_restores_the_layout_of_that_mode() {
    let (_ctx, mut app) = headless("modes");
    assert_eq!((app.espace, app.mode), (Espace::Storyboard, Mode::Storyboard));
    // Storyboard : on ferme la fiche.
    assert!(has(&app, Pane::Sheet));
    app.toggle_pane(Pane::Sheet);
    assert!(!has(&app, Pane::Sheet));
    // Animation : sa disposition à elle (la fiche y est), et on y ferme la couleur.
    app.set_mode(Mode::Animation);
    assert_eq!((app.espace, app.mode), (Espace::Animation, Mode::Animation));
    assert!(has(&app, Pane::Sheet) && has(&app, Pane::Color));
    app.toggle_pane(Pane::Color);
    assert!(!has(&app, Pane::Color));
    // Retour en storyboard : la fiche est encore fermée, la couleur est là.
    app.set_mode(Mode::Storyboard);
    assert!(!has(&app, Pane::Sheet) && has(&app, Pane::Color));
    // Et l'animation retrouve ce qu'on y avait fait.
    app.set_mode(Mode::Animation);
    assert!(has(&app, Pane::Sheet) && !has(&app, Pane::Color));
}

#[test]
fn the_layouts_are_written_to_the_settings_per_space() {
    let (ctx, mut app) = headless("ecriture");
    app.toggle_pane(Pane::Sheet);
    app.set_mode(Mode::Animation);
    app.toggle_pane(Pane::Color);
    // L'image d'écran suivante retient la disposition de l'animation et écrit les réglages.
    app.remember_layout(&ctx);
    let file = Settings::load();
    let story = file.layout_of(Espace::Storyboard).expect("la disposition du storyboard");
    let anim = file.layout_of(Espace::Animation).expect("la disposition de l'animation");
    assert!(!story.dock_layout().contains(Pane::Sheet) && story.dock_layout().contains(Pane::Color));
    assert!(anim.dock_layout().contains(Pane::Sheet) && !anim.dock_layout().contains(Pane::Color));
    assert!(file.layout_of(Espace::Montage).is_none(), "l'espace Montage n'a pas encore été visité");
    // Une autre séance démarre en storyboard avec sa disposition, et l'animation garde la sienne.
    let ctx2 = egui::Context::default();
    let mut app2 = App::sans_fenetre(&ctx2, None);
    assert_eq!(app2.espace, Espace::Storyboard);
    assert!(!has(&app2, Pane::Sheet) && has(&app2, Pane::Color));
    app2.set_mode(Mode::Animation);
    assert!(has(&app2, Pane::Sheet) && !has(&app2, Pane::Color));
}

#[test]
fn the_old_single_layout_becomes_the_storyboard_one() {
    // Un fichier de réglages d'avant les espaces : une seule `disposition`.
    let (_ctx0, app0) = headless("migration");
    drop(app0);
    let mut old = Settings::default();
    old.layout = Some(Layout { tools: false, sheet: false, ..Layout::default() });
    let text: String = toml::to_string(&old).unwrap().lines().filter(|l| !l.starts_with("espace_storyboard")).collect::<Vec<_>>().join("\n");
    assert!(!text.contains("disposition_animation") && !text.contains("espace_storyboard"));
    let parsed: Settings = toml::from_str(&text).unwrap();
    assert_eq!(parsed.storyboard_space, Espace::Storyboard);
    assert_eq!(parsed.layout_of(Espace::Storyboard), old.layout.as_ref());
    assert!(parsed.layout_of(Espace::Animation).is_none() && parsed.layout_of(Espace::Montage).is_none());
    // L'application la prend pour le storyboard ; l'animation commence par sa disposition d'origine.
    parsed.save().unwrap();
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    assert_eq!(app.espace, Espace::Storyboard);
    assert!(!has(&app, Pane::Layers) && !has(&app, Pane::ToolOptions), "la disposition d'avant, telle quelle");
    app.set_mode(Mode::Animation);
    assert!(has(&app, Pane::Layers) && has(&app, Pane::ToolOptions), "l'animation : sa disposition d'origine");
    app.set_mode(Mode::Storyboard);
    assert!(!has(&app, Pane::Layers), "et le storyboard garde l'ancienne");
}

#[test]
fn the_three_layouts_and_the_storyboard_side_survive_a_round_trip() {
    let mut s = Settings::default();
    s.layout = Some(Layout { strip: false, ..Layout::default() });
    s.animation_layout = Some(Layout { timeline_height: 420.0, ..Layout::default() });
    s.montage_layout = Some(Espace::Montage.defaut());
    s.storyboard_space = Espace::Montage;
    let back: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.layout, s.layout);
    assert_eq!(back.animation_layout, s.animation_layout);
    assert_eq!(back.montage_layout, s.montage_layout);
    assert_eq!(back.storyboard_space, Espace::Montage);
}

#[test]
fn the_montage_space_is_a_storyboard_layout_and_the_mode_button_returns_to_it() {
    let (_ctx, mut app) = headless("montage");
    app.run_action("espace.montage");
    assert_eq!((app.espace, app.mode), (Espace::Montage, Mode::Storyboard));
    assert!(has(&app, Pane::Montage) && has(&app, Pane::Sheet) && !has(&app, Pane::Layers) && !has(&app, Pane::Color));
    assert_eq!(app.settings.storyboard_space, Espace::Montage);
    // Animation, puis retour : on retrouve le montage (le côté storyboard où l'on était).
    app.set_mode(Mode::Animation);
    assert!(has(&app, Pane::Layers));
    app.set_mode(Mode::Storyboard);
    assert_eq!(app.espace, Espace::Montage);
    assert!(!has(&app, Pane::Layers));
    // Le menu et la palette ramènent à la disposition du storyboard.
    app.run_action("espace.storyboard");
    assert_eq!((app.espace, app.mode), (Espace::Storyboard, Mode::Storyboard));
    assert!(has(&app, Pane::Layers) && has(&app, Pane::Color));
    assert_eq!(app.settings.storyboard_space, Espace::Storyboard);
}

#[test]
fn resetting_the_layout_resets_only_the_current_space() {
    let (_ctx, mut app) = headless("retablir");
    app.toggle_pane(Pane::Sheet);
    app.set_mode(Mode::Animation);
    app.toggle_pane(Pane::Color);
    app.toggle_pane(Pane::ToolOptions);
    assert!(!has(&app, Pane::Color) && !has(&app, Pane::ToolOptions));
    app.run_action("espace.retablir");
    assert!(has(&app, Pane::Color) && has(&app, Pane::ToolOptions), "l'animation retrouve sa disposition d'origine");
    assert_eq!(app.layout.montage_height, Espace::Animation.defaut().montage_height);
    assert_eq!(app.mode, Mode::Animation, "le mode ne change pas");
    // Le storyboard, lui, n'a pas bougé : sa fiche est toujours fermée.
    app.set_mode(Mode::Storyboard);
    assert!(!has(&app, Pane::Sheet));
    app.run_action("espace.retablir");
    assert!(has(&app, Pane::Sheet));
}

#[test]
fn the_exposure_sheet_only_exists_in_animation_and_comes_back_with_its_space() {
    let (_ctx, mut app) = headless("exposition");
    app.set_mode(Mode::Animation);
    app.toggle_pane(Pane::XSheet);
    assert!(has(&app, Pane::XSheet));
    app.set_mode(Mode::Storyboard);
    assert!(!has(&app, Pane::XSheet), "pas de feuille d'exposition en storyboard");
    app.set_mode(Mode::Animation);
    assert!(has(&app, Pane::XSheet), "elle revient avec la disposition de l'animation");
    // Le montage ne la montre pas non plus.
    app.run_action("espace.montage");
    assert!(!has(&app, Pane::XSheet));
}

#[test]
fn a_panel_in_its_own_window_stays_where_it_is_when_the_space_changes() {
    let (_ctx, mut app) = headless("fenetres");
    let mut layout = app.dock_layout();
    layout.detach(Pane::Color);
    app.set_dock_layout(layout);
    assert!(app.detached.iter().any(|d| d.panneau == Pane::Color));
    app.set_mode(Mode::Animation);
    assert!(app.detached.iter().any(|d| d.panneau == Pane::Color), "la fenêtre de la couleur est toujours là en animation");
    assert_eq!(app.dock_layout().panes().iter().filter(|p| **p == Pane::Color).count(), 1, "et la couleur n'est pas en double");
    app.run_action("espace.montage");
    assert!(app.detached.iter().any(|d| d.panneau == Pane::Color));
    assert_eq!(app.dock_layout().panes().iter().filter(|p| **p == Pane::Color).count(), 1);
}

#[test]
fn every_panel_of_the_panel_bar_opens_in_every_space() {
    // Même dans l'espace Montage, qui n'a ni calques ni effets : l'historique, les commentaires, les
    // bruitages, les suggestions sonores et les références forment leur colonne à droite.
    let (_ctx, mut app) = headless("barre");
    for espace in Espace::TOUS {
        app.set_espace(espace);
        for e in crate::barrepanneaux::GROUPES.iter().flat_map(|g| g.iter()).filter(|e| e.visible(espace == Espace::Animation)) {
            app.show_pane(e.pane);
            assert!(has(&app, e.pane), "{:?} ne s'ouvre pas dans l'espace {}", e.pane, espace.nom());
        }
    }
}

#[test]
fn switching_is_instant_and_does_not_touch_the_adaptive_panels_state() {
    let (ctx, mut app) = headless("rapide");
    let t = std::time::Instant::now();
    for _ in 0..20 {
        app.set_mode(Mode::Animation);
        app.set_mode(Mode::Storyboard);
        app.run_action("espace.montage");
        app.run_action("espace.storyboard");
    }
    // 80 changements d'espace : un instant (aucun rejeu, aucun calcul d'image).
    assert!(t.elapsed().as_millis() < 2000, "{:?}", t.elapsed());
    // Les panneaux qui s'ajustent à leur contenu repartent de la disposition posée.
    assert!(app.fit_set.is_empty());
    app.remember_layout(&ctx);
}
