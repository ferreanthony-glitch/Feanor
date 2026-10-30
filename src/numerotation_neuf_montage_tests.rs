//! Numérotation verrouillée d'origine (projets neufs) et montage redimensionnable (`montagehauteur.rs`).

use eframe::egui::{self, pos2, vec2, Rect};
use egui_dock::{Node, NodeIndex};

use crate::app::App;
use crate::document::Document;
use crate::dock::Pane;
use crate::montagehauteur::{self, Taille};
use crate::workspace::Espace;

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-numneuf-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

fn codes(doc: &Document) -> Vec<String> {
    doc.scene_order().iter().map(|&s| doc.plan_code(s)).collect()
}

// ------------------------------------------------------------------------------------ numérotation

#[test]
fn the_preference_is_on_by_default_and_remembered() {
    let s = crate::settings::Settings::default();
    assert!(s.numbering_locked_new);
    // Un ancien fichier de réglages : coché.
    let old: crate::settings::Settings = toml::from_str("copie_secours_minutes = 3\n").unwrap();
    assert!(old.numbering_locked_new);
    // Décoché : retenu, et relu tel quel.
    let mut s = crate::settings::Settings::default();
    s.numbering_locked_new = false;
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("numerotation_neufs_verrouillee"), "{text}");
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert!(!back.numbering_locked_new);
}

#[test]
fn a_new_project_is_born_locked_and_an_inserted_plan_is_2a() {
    let (_ctx, mut app) = headless("nouveau");
    let d = crate::nouveauui::NewDocDialog::open(&app.settings.new_project);
    assert!(app.create_project(&d));
    assert!(app.doc.numbering_locked, "Nouveau projet naît verrouillé");
    // Trois plans, puis « Nouveau plan » après le 2 : 2A, les autres gardent leur numéro.
    app.doc.add_scene_after(0);
    app.doc.add_scene_after(1);
    assert_eq!(codes(&app.doc), ["1", "2", "3"]);
    app.select_panel(1);
    app.new_panel();
    assert_eq!(codes(&app.doc), ["1", "2", "2A", "3"]);
}

#[test]
fn the_preference_off_gives_a_followed_numbering() {
    let (_ctx, mut app) = headless("suivie");
    app.settings.numbering_locked_new = false;
    let d = crate::nouveauui::NewDocDialog::open(&app.settings.new_project);
    assert!(app.create_project(&d));
    assert!(!app.doc.numbering_locked);
}

#[test]
fn a_project_from_chosen_plans_is_born_locked() {
    let (_ctx, mut app) = headless("choisis");
    app.doc.numbering_locked = false;
    app.doc.add_scene_after(0);
    app.doc.add_scene_after(1);
    app.select_panel(1);
    app.new_project_from_chosen();
    assert!(app.doc.numbering_locked);
    assert_eq!(codes(&app.doc), ["1"]);
}

#[test]
fn a_project_from_a_screenplay_is_born_locked() {
    let (ctx, mut app) = headless("scenario");
    let dir = std::env::temp_dir().join(format!("shot-numneuf-scen-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("Essai.fountain");
    std::fs::write(&file, "Title: Essai\n\nINT. SALON - JOUR\n\nMarie entre.\n").unwrap();
    app.read_scenario(file);
    let t = std::time::Instant::now();
    loop {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
        let w = app.scenario_window.as_ref().expect("fenêtre");
        if w.ready() || w.failure().is_some() {
            break;
        }
        assert!(t.elapsed().as_secs() < 10);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.doc.numbering_locked = false;
    app.scenario_new_project();
    assert!(app.doc.numbering_locked);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_headless_core_creates_locked_projects() {
    let p = crate::noyau::Projet::nouveau(640, 360, 24);
    assert!(p.doc.numbering_locked);
}

#[test]
fn an_existing_project_keeps_its_setting_when_opened() {
    let dir = std::env::temp_dir().join(format!("shot-numneuf-ancien-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join(format!("Vieux.{}", crate::project::EXTENSION));
    let (manifest, _) = (crate::project::target_for(&manifest).1, ());
    for locked in [false, true] {
        let mut doc = Document::new(64, 36, 24);
        doc.add_scene_after(0);
        if locked {
            doc.lock_numbering();
        }
        let mut written = std::collections::HashMap::new();
        crate::project::save(&doc, &manifest, &mut written).expect("enregistré");
        // Rouvert : le réglage du projet ne bouge pas (l'ouverture ne touche jamais à la numérotation).
        let loaded = crate::project::load(&manifest).expect("relu");
        assert_eq!(loaded.doc.numbering_locked, locked, "un projet qui existe garde son réglage");
        assert_eq!(codes(&loaded.doc), ["1", "2"]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renumbering_stays_available_in_a_locked_project() {
    let (_ctx, mut app) = headless("renum");
    let d = crate::nouveauui::NewDocDialog::open(&app.settings.new_project);
    assert!(app.create_project(&d));
    app.doc.add_scene_after(0);
    app.doc.add_scene_after(0);
    assert_eq!(codes(&app.doc), ["1", "1A", "2"]);
    app.doc.renumber();
    assert_eq!(codes(&app.doc), ["1", "2", "3"]);
}

// ------------------------------------------------------------------------------------ montage

#[test]
fn the_edge_keeps_a_minimum_band_and_a_minimum_drawing() {
    use montagehauteur::{fraction_bornee, BANDE_MIN, DESSIN_MIN};
    let total = 800.0;
    let f = fraction_bornee(total, 0.99, DESSIN_MIN, BANDE_MIN);
    assert!((total * (1.0 - f) - BANDE_MIN).abs() < 0.5, "bande au moins {BANDE_MIN} : {f}");
    let f = fraction_bornee(total, 0.01, DESSIN_MIN, BANDE_MIN);
    assert!((total * f - DESSIN_MIN).abs() < 0.5, "dessin au moins {DESSIN_MIN} : {f}");
    assert_eq!(fraction_bornee(total, 0.6, DESSIN_MIN, BANDE_MIN), 0.6, "un bord raisonnable ne bouge pas");
}

#[test]
fn the_dock_edge_above_the_montage_is_clamped_after_display() {
    let (ctx, mut app) = headless("bord");
    let mut state = crate::dock::to_state(&app.dock_layout());
    let tree = state.main_surface_mut();
    let mut found = false;
    for i in 0..tree.len() {
        if let Node::Vertical { rect, fraction, .. } = &mut tree[NodeIndex(i)] {
            *rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0));
            *fraction = 0.97;
            found = true;
            break;
        }
    }
    assert!(found);
    app.poignee_du_montage(&ctx, &mut state);
    let tree = state.main_surface_mut();
    let f = (0..tree.len()).find_map(|i| if let Node::Vertical { fraction, .. } = &tree[NodeIndex(i)] { Some(*fraction) } else { None }).unwrap();
    assert!(800.0 * (1.0 - f) >= montagehauteur::BANDE_MIN - 0.5, "{f}");
}

#[test]
fn the_chosen_height_is_kept_per_workspace() {
    let (_ctx, mut app) = headless("espaces");
    let part = |app: &App| match &app.dock_layout().principal {
        crate::dock::DockNode::Column { part, .. } => *part,
        other => panic!("{other:?}"),
    };
    app.aller_a_l_espace(Espace::Storyboard);
    let mut l = app.dock_layout();
    if let crate::dock::DockNode::Column { part, .. } = &mut l.principal {
        *part = 0.5;
    }
    app.set_dock_layout(l);
    app.aller_a_l_espace(Espace::Montage);
    app.aller_a_l_espace(Espace::Storyboard);
    assert!((part(&app) - 0.5).abs() < 1e-3, "la hauteur du Storyboard est retenue : {}", part(&app));
}

#[test]
fn the_montage_scrolls_when_small_and_nothing_is_cut() {
    let (ctx, mut app) = headless("defile");
    app.new_audio_track();
    app.new_audio_track();
    assert!(app.doc.audio_tracks.len() >= 2);
    let run = |app: &mut App, h: f32| {
        for _ in 0..3 {
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0)));
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.allocate_ui(vec2(1000.0, h), |ui| {
                        ui.set_max_height(h);
                        app.montage_panel(ui);
                    });
                });
            });
        }
        montagehauteur::DEFILEMENT.with(|c| c.get())
    };
    let (content, vue) = run(&mut app, 170.0);
    let rows = app.montage_rows(vue);
    assert!(content >= rows.height(app.doc.audio_tracks.len()) - 1.0, "tout le contenu défile : {content} contre {}", rows.height(app.doc.audio_tracks.len()));
    assert!(content > vue + 20.0, "bande petite : il faut défiler ({content} > {vue})");
    // Grande : tout tient, plus rien à défiler.
    let (content, vue) = run(&mut app, 690.0);
    assert!(content <= vue + 1.0, "bande haute : tout tient ({content} <= {vue})");
}

#[test]
fn track_height_setting_changes_the_rows_and_is_remembered() {
    let (_ctx, mut app) = headless("hauteur");
    app.new_audio_track();
    let n = app.doc.audio_tracks.len();
    let mut h = Vec::new();
    for t in Taille::TOUTES {
        app.regler_hauteur_pistes(t);
        h.push(app.montage_rows(400.0).height(n));
        assert_eq!(montagehauteur::piste_son(), t.piste_son());
    }
    assert!(h[0] < h[1] && h[1] < h[2], "petite < moyenne < grande : {h:?}");
    // Retenue dans les réglages (écrits et relus).
    let text = toml::to_string(&app.settings).unwrap();
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.track_height.son, Taille::Grande);
    assert_eq!(Taille::Grande.suivante(), Taille::Petite);
    // D'origine : rien d'écrit.
    assert!(!toml::to_string(&crate::settings::Settings::default()).unwrap().contains("pistes_hauteur"));
}

#[test]
fn the_track_buttons_stay_legible_at_every_height() {
    for t in Taille::TOUTES {
        let h = t.piste_son();
        let lane = Rect::from_min_size(pos2(2.0, 100.0), vec2(96.0, h - 2.0));
        let [m, s, l] = montagehauteur::boutons_piste(lane);
        for b in [m, s, l] {
            assert!(b.height() >= 16.0 && b.width() >= 22.0, "{t:?} : {b:?}");
            assert!(lane.contains_rect(b), "{t:?} : le bouton sort de la piste ({b:?} hors de {lane:?})");
        }
        assert!(m.right() < s.left() && s.right() < l.left(), "ils ne se touchent pas");
        // Sous le nom (une ligne de texte de 12 points en haut).
        assert!(m.top() >= lane.top() + 13.0, "{t:?} : sous le nom");
    }
}
