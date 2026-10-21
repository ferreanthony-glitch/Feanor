//! Légende et filtre des clés : toutes les sortes sont montrées d'office, le filtre est un affichage (le projet ne
//! change pas), il est retenu dans les réglages, et la légende nomme chaque sorte.

use eframe::egui;

use crate::app::App;
use crate::legendecles::{FiltreCles, Sorte};
use crate::settings::Settings;
use crate::transitions::Kind;

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-legende-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let app = App::sans_fenetre(&ctx, None);
    (ctx, app)
}

#[test]
fn every_kind_is_shown_by_default_and_the_drawing_keys_cannot_be_hidden() {
    let mut f = FiltreCles::default();
    for s in Sorte::TOUTES {
        assert!(f.montre(s), "{s:?}");
    }
    assert!(f.est_complet());
    f.regle(Sorte::Dessin, false);
    assert!(f.montre(Sorte::Dessin) && f.est_complet(), "les clés de dessin sont le contenu des pistes : elles ne se cachent pas");
    assert!(!Sorte::Dessin.masquable());
    for s in [Sorte::Camera, Sorte::Effet, Sorte::Position, Sorte::Transition] {
        assert!(s.masquable());
        f.regle(s, false);
        assert!(!f.montre(s) && !f.est_complet(), "{s:?}");
        // Les autres sortes ne bougent pas.
        for autre in Sorte::TOUTES.into_iter().filter(|a| *a != s) {
            assert!(f.montre(autre), "{autre:?} reste montrée quand {s:?} est cachée");
        }
        f.regle(s, true);
        assert!(f.est_complet());
    }
}

#[test]
fn the_legend_names_each_kind_with_its_own_colour_and_a_description() {
    let mut couleurs = Vec::new();
    for s in Sorte::TOUTES {
        assert!(!s.nom().is_empty() && s.description().ends_with('.'), "{s:?}");
        couleurs.push(s.couleur());
    }
    assert_eq!(Sorte::Position.couleur(), crate::theme::POIGNEE_CALQUE);
    assert_eq!(Sorte::Camera.couleur(), crate::theme::camera());
    assert_eq!(Sorte::Effet.couleur(), crate::fxkeys::color());
    for (i, a) in couleurs.iter().enumerate() {
        for b in &couleurs[i + 1..] {
            assert_ne!(a, b, "deux sortes de clés ont la même couleur");
        }
    }
}

#[test]
fn the_filter_is_kept_in_the_settings_and_writes_nothing_when_complete() {
    let s = Settings::default();
    assert!(s.key_filter.est_complet());
    let text = toml::to_string(&s).unwrap();
    assert!(!text.contains("cles_vues"), "{text}");
    // Des réglages d'avant le filtre : tout est montré.
    let old: Settings = toml::from_str("copie_secours_minutes = 1").unwrap();
    assert!(old.key_filter.est_complet());
    let mut s = Settings::default();
    s.key_filter.regle(Sorte::Camera, false);
    s.key_filter.regle(Sorte::Transition, false);
    let text = toml::to_string(&s).unwrap();
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.key_filter, s.key_filter);
    assert!(!back.key_filter.camera && !back.key_filter.transitions && back.key_filter.effets && back.key_filter.positions);
    // Un champ qui manque dans le fichier : montré.
    let partiel: Settings = toml::from_str("[cles_vues]\ncamera = false").unwrap();
    assert!(!partiel.key_filter.camera && partiel.key_filter.effets && partiel.key_filter.positions && partiel.key_filter.transitions);
}

#[test]
fn hiding_a_kind_changes_what_is_seen_and_grabbed_but_never_the_project() {
    let (_ctx, mut app) = headless("affichage");
    let second = app.doc.add_scene_after(0);
    assert!(app.doc.panels.len() >= 2 && second > 0);
    app.doc.set_transition(1, Kind::Dissolve).expect("une transition sur la coupe");
    let etat = |app: &App| (app.doc.transitions.clone(), app.doc.panels.iter().map(|p| (p.camera.len(), p.layers.iter().map(|l| l.pos_keys.len()).sum::<usize>())).collect::<Vec<_>>());
    let avant = etat(&app);
    // Montrée : une forme à saisir.
    assert_eq!(app.trans_badges(0.0, 10.0, 0.0).len(), 1);
    assert!(app.cle_vue(Sorte::Transition));
    // Cachée : plus de forme (donc ni dessinée ni saisie : tous les gestes passent par ces formes).
    app.settings.key_filter.regle(Sorte::Transition, false);
    assert!(!app.cle_vue(Sorte::Transition));
    assert!(app.trans_badges(0.0, 10.0, 0.0).is_empty());
    // Le projet est exactement le même : la transition est toujours là.
    assert!(avant == etat(&app));
    assert_eq!(app.doc.transitions.len(), 1);
    // Rallumée : elle revient.
    app.settings.key_filter.regle(Sorte::Transition, true);
    assert_eq!(app.trans_badges(0.0, 10.0, 0.0).len(), 1);
    // Les autres sortes ne dépendent pas de celle-là.
    app.settings.key_filter.regle(Sorte::Camera, false);
    assert_eq!(app.trans_badges(0.0, 10.0, 0.0).len(), 1);
    assert!(app.cle_vue(Sorte::Position) && app.cle_vue(Sorte::Effet));
}

#[test]
fn the_legend_icon_is_drawn_at_the_end_of_the_headers_whatever_the_filter() {
    // L'icône « ? » est dans la ligne d'en-tête des deux bandes : une image entière est peinte sans plantage, filtre
    // allumé ou non.
    let (ctx, mut app) = headless("icone");
    for filtre_actif in [false, true] {
        app.settings.key_filter = FiltreCles::default();
        if filtre_actif {
            app.settings.key_filter.regle(Sorte::Effet, false);
        }
        for barre in [crate::barres::Barre::Montage, crate::barres::Barre::Timeline] {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 300.0))), time: Some(0.0), ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        app.barre_ui(ui, barre);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| app.legende_cles_ui(ui));
                    });
                });
            });
        }
    }
}
