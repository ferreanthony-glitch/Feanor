//! Régler le stylet à sa main (`reglagestylet.rs`) : trois traits (léger, moyen, appuyé) donnent la courbe de pression du stylet ;
//! aperçu avant de poser, rien n'est changé sans « Appliquer », « Rétablir » rend la courbe d'avant, la souris (100 % d'appui)
//! n'est pas mesurée.

use eframe::egui::{epaint::Shape, pos2, Event, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};

use crate::brush::Curve;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::reglagestylet::{appui_typique, courbe_pour, Refus, Reglage, CIBLES, ECART_MIN, ECHANTILLONS_MIN};

// ------------------------------------------------------------------------------------ la mesure

#[test]
fn the_typical_pressure_of_a_stroke_is_the_median_of_its_middle() {
    // Le début et la fin, où l'appui monte et retombe, ne comptent pas.
    let trait_: Vec<f32> = [0.02, 0.05, 0.3, 0.31, 0.3, 0.32, 0.29, 0.3, 0.06, 0.02].to_vec();
    let a = appui_typique(&trait_).unwrap();
    assert!((a - 0.3).abs() < 0.02, "{a}");
    // Un trait trop court ne dit rien (un clic, un point).
    assert_eq!(appui_typique(&[0.5; ECHANTILLONS_MIN - 1]), None);
    assert_eq!(appui_typique(&[]), None);
    assert!(appui_typique(&[0.5; ECHANTILLONS_MIN]).is_some());
    // Des valeurs absurdes sont ramenées dans 0..1.
    assert_eq!(appui_typique(&[f32::NAN, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]), Some(1.0));
}

#[test]
fn the_fitted_curve_turns_every_hand_into_the_same_light_medium_and_firm_responses() {
    let mains: [[f32; 3]; 5] = [[0.05, 0.15, 0.35], [0.12, 0.4, 0.7], [0.2, 0.55, 1.0], [0.3, 0.6, 0.95], [0.5, 0.75, 0.9]];
    for m in mains {
        let c = courbe_pour(m).unwrap_or_else(|e| panic!("{m:?} : {e:?}"));
        for (k, appui) in m.iter().enumerate() {
            assert!((c.eval(*appui) - CIBLES[k]).abs() < 0.01, "main {m:?} : l'appui {appui} doit donner {} ({})", CIBLES[k], c.eval(*appui));
        }
        // Une courbe qui ne redescend jamais : appuyer plus ne donne jamais moins.
        let ys: Vec<f32> = (0..=100).map(|k| c.eval(k as f32 / 100.0)).collect();
        assert!(ys.windows(2).all(|w| w[1] >= w[0] - 1e-6), "{m:?} : monotone");
        assert!((c.eval(0.0)).abs() < 1e-6 && (c.eval(1.0) - 1.0).abs() < 1e-6, "de 0 à 1 : {m:?}");
        assert!(c.points().len() <= Curve::MAX);
    }
    // Une main déjà « droite » (0,2, 0,55, 1) donne la droite à peu près : on ne déforme pas ce qui va bien.
    let droite = courbe_pour([0.2, 0.55, 1.0]).unwrap();
    for x in [0.0_f32, 0.1, 0.3, 0.5, 0.8, 1.0] {
        assert!((droite.eval(x) - Curve::linear().eval(x)).abs() < 0.2, "{x} : {}", droite.eval(x));
    }
    // Une main légère donne une courbe qui monte vite : les appuis faibles donnent déjà du corps.
    let legere = courbe_pour([0.05, 0.15, 0.35]).unwrap();
    assert!(legere.eval(0.15) > 0.5 && legere.eval(0.05) >= 0.19);
}

#[test]
fn three_strokes_that_look_alike_give_no_proposal() {
    assert_eq!(courbe_pour([0.3, 0.31, 0.32]).err(), Some(Refus::TropProches));
    assert_eq!(courbe_pour([0.2, 0.2 + ECART_MIN * 0.5, 0.9]).err(), Some(Refus::TropProches));
    assert_eq!(courbe_pour([0.5, 0.3, 0.9]).err(), Some(Refus::TropProches), "dans le désordre : léger plus fort que moyen");
    assert!(courbe_pour([0.2, 0.2 + ECART_MIN * 1.5, 0.9]).is_ok());
    assert!(!Refus::TropProches.texte().is_empty() && !Refus::Incomplet.texte().is_empty());
}

// ------------------------------------------------------------------------------------ la lecture des événements

fn stylet(zone: eframe::egui::Rect, y: f32, force: f32, n: usize) -> Vec<Vec<Event>> {
    let p = |k: usize| pos2(zone.left() + 30.0 + k as f32 * 10.0, zone.top() + y);
    let touch = |phase, k: usize| Event::Touch { device_id: TouchDeviceId(0), id: TouchId(0), phase, pos: p(k), force: Some(force) };
    let mut images = vec![vec![touch(TouchPhase::Start, 0), Event::PointerMoved(p(0)), Event::PointerButton { pos: p(0), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]];
    for k in 1..n {
        images.push(vec![touch(TouchPhase::Move, k), Event::PointerMoved(p(k))]);
    }
    images.push(vec![touch(TouchPhase::End, n), Event::PointerButton { pos: p(n), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    images
}

fn souris(zone: eframe::egui::Rect, y: f32, n: usize) -> Vec<Vec<Event>> {
    let p = |k: usize| pos2(zone.left() + 30.0 + k as f32 * 10.0, zone.top() + y);
    let mut images = vec![vec![Event::PointerMoved(p(0)), Event::PointerButton { pos: p(0), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]];
    for k in 1..n {
        images.push(vec![Event::PointerMoved(p(k))]);
    }
    images.push(vec![Event::PointerButton { pos: p(n), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    images
}

#[test]
fn the_reader_keeps_pen_strokes_and_ignores_the_mouse() {
    let zone = eframe::egui::Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(400.0, 110.0));
    let mut r = Reglage::nouveau();
    assert_eq!(r.etape(), Some(0));
    for images in [stylet(zone, 30.0, 0.1, 12), souris(zone, 50.0, 12), stylet(zone, 60.0, 0.4, 12)] {
        for e in images {
            r.lire(zone, &e);
        }
    }
    assert_eq!(r.traits.len(), 2, "le trait à la souris n'est pas retenu");
    assert!(r.souris_vue && r.stylet_vu);
    assert_eq!(r.etape(), Some(2));
    assert!(r.appuis().is_none() && r.proposition().err() == Some(Refus::Incomplet));
    // Un trait hors du cadre n'est pas pris, un trait trop court non plus.
    for e in stylet(zone, 500.0, 0.9, 12) {
        r.lire(zone, &e);
    }
    for e in stylet(zone, 80.0, 0.9, 2) {
        r.lire(zone, &e);
    }
    assert_eq!(r.traits.len(), 2);
    for e in stylet(zone, 80.0, 0.9, 12) {
        r.lire(zone, &e);
    }
    assert_eq!(r.traits.len(), 3);
    assert_eq!(r.etape(), None);
    let a = r.appuis().unwrap();
    assert!((a[0] - 0.1).abs() < 1e-3 && (a[1] - 0.4).abs() < 1e-3 && (a[2] - 0.9).abs() < 1e-3, "{a:?}");
    let c = r.proposition().unwrap();
    assert!((c.eval(0.1) - 0.2).abs() < 0.01 && (c.eval(0.4) - 0.55).abs() < 0.01 && (c.eval(0.9) - 1.0).abs() < 0.01);
    r.recommencer();
    assert!(r.traits.is_empty() && r.etape() == Some(0));
}

// ------------------------------------------------------------------------------------ la fenêtre

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

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1600, 900);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

fn zone_de_la_fenetre(b: &Banc) -> eframe::egui::Rect {
    let c = tous_les_textes(b).into_iter().find(|(t, _)| t.contains("Trace ici avec le stylet")).expect("la zone").1;
    eframe::egui::Rect::from_center_size(c, eframe::egui::vec2(400.0, 110.0))
}

fn tracer(b: &mut Banc, images: Vec<Vec<Event>>) {
    for e in images {
        b.image(e, 1.0 / 120.0);
    }
    b.chauffer(2);
}

#[test]
fn the_window_proposes_with_a_preview_and_changes_nothing_until_appliquer() {
    let mut b = banc();
    let avant = b.app.toucher.pressure_curve;
    assert_eq!(avant, Curve::linear());
    b.app.run_action("aide.regler_stylet");
    b.chauffer(6);
    assert!(est_la(&b, "Régler le stylet à ma main") && est_la(&b, "1/3  ·  un trait léger"));
    let zone = zone_de_la_fenetre(&b);
    tracer(&mut b, stylet(zone, 25.0, 0.08, 14));
    assert!(est_la(&b, "2/3  ·  un trait moyen"));
    tracer(&mut b, stylet(zone, 55.0, 0.3, 14));
    assert!(est_la(&b, "3/3  ·  un trait appuyé"));
    tracer(&mut b, stylet(zone, 85.0, 0.65, 14));
    // L'aperçu : « Aujourd'hui » et « Proposé pour ta main », les appuis mesurés, les deux boutons.
    for attendu in ["Les trois traits sont tracés.", "Aujourd'hui", "Proposé pour ta main", "léger : 8 %", "moyen : 30 %", "appuyé : 65 %", "Appliquer", "Rétablir", "Recommencer"] {
        assert!(est_la(&b, attendu), "« {attendu} » manque : {:?}", tous_les_textes(&b).iter().map(|t| &t.0).collect::<Vec<_>>());
    }
    assert_eq!(b.app.toucher.pressure_curve, avant, "rien n'est changé avant « Appliquer »");
    // « Rétablir » est grisé tant que rien n'est appliqué (cliquer ne fait rien).
    let retablir = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Rétablir").unwrap().1;
    clic(&mut b, retablir);
    assert_eq!(b.app.toucher.pressure_curve, avant);
    // « Appliquer » pose la courbe proposée.
    let appliquer = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Appliquer").unwrap().1;
    clic(&mut b, appliquer);
    let neuve = b.app.toucher.pressure_curve;
    assert_ne!(neuve, avant);
    assert!((neuve.eval(0.08) - 0.2).abs() < 0.01 && (neuve.eval(0.3) - 0.55).abs() < 0.01 && (neuve.eval(0.65) - 1.0).abs() < 0.01);
    // Comme le panneau Réglages de l'outil : le réglage de l'application suit (écrit dans les réglages).
    b.chauffer(3);
    assert_eq!(b.app.settings.toucher.pressure_curve, neuve);
    // « Appliquer » ferme la fenêtre (il n'y a plus rien à y faire).
    assert!(b.app.confort.stylet.is_none(), "la fenêtre est fermée");
    assert!(!est_la(&b, "Trace trois traits dans le cadre") && !est_la(&b, "Appliquer"));
    // Rouverte : « Rétablir » est là, et rend la courbe d'avant.
    b.app.run_action("aide.regler_stylet");
    b.chauffer(6);
    let retablir = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Rétablir").unwrap().1;
    clic(&mut b, retablir);
    assert_eq!(b.app.toucher.pressure_curve, avant, "la courbe d'avant");
    b.chauffer(3);
    assert_eq!(b.app.settings.toucher.pressure_curve, avant);
}

#[test]
fn the_mouse_is_told_it_has_nothing_to_set() {
    let mut b = banc();
    b.app.run_action("aide.regler_stylet");
    b.chauffer(6);
    let zone = zone_de_la_fenetre(&b);
    tracer(&mut b, souris(zone, 40.0, 14));
    assert!(est_la(&b, "toujours de 100 %"), "{:?}", tous_les_textes(&b).iter().map(|t| &t.0).collect::<Vec<_>>());
    assert!(est_la(&b, "1/3  ·  un trait léger"), "le trait à la souris n'avance pas");
    assert_eq!(b.app.toucher.pressure_curve, Curve::linear());
    // L'avertissement est aussi écrit en bas de la fenêtre, à demeure.
    assert!(est_la(&b, "Avec la souris, l'appui vaut toujours 100 %"));
}

#[test]
fn strokes_that_look_alike_show_a_hint_and_cannot_be_applied() {
    let mut b = banc();
    b.app.run_action("aide.regler_stylet");
    b.chauffer(6);
    let zone = zone_de_la_fenetre(&b);
    for (y, f) in [(25.0, 0.3), (55.0, 0.31), (85.0, 0.32)] {
        tracer(&mut b, stylet(zone, y, f, 14));
    }
    assert!(est_la(&b, "se ressemblent trop"), "{:?}", tous_les_textes(&b).iter().map(|t| &t.0).collect::<Vec<_>>());
    let appliquer = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Appliquer").unwrap().1;
    clic(&mut b, appliquer);
    assert_eq!(b.app.toucher.pressure_curve, Curve::linear(), "rien à appliquer");
    // « Recommencer » efface les traits.
    let recommencer = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Recommencer").unwrap().1;
    clic(&mut b, recommencer);
    assert!(est_la(&b, "1/3  ·  un trait léger"));
}

#[test]
fn the_window_opens_from_the_help_menu_the_palette_and_the_preferences() {
    use crate::prefsonglets::Onglet;
    assert!(crate::keymap::action_info("aide.regler_stylet").is_some_and(|a| a.implemented));
    let mut b = banc();
    b.app.settings.prefs_onglet = Onglet::Dessin;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    let bouton = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Régler le stylet à ma main…").expect("le bouton de Préférences > Dessin").1;
    clic(&mut b, bouton);
    assert!(b.app.confort.stylet.is_some(), "la fenêtre est ouverte");
    b.chauffer(3);
    assert!(est_la(&b, "Trace trois traits dans le cadre"));
    // La croix de la fenêtre la ferme.
    b.app.confort.stylet = None;
    b.chauffer(3);
    assert!(!est_la(&b, "Trace trois traits dans le cadre"));
}
