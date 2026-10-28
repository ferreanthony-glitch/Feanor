//! « Peindre derrière » et « Peint sur l'aplat » (deux interrupteurs, exclusifs), puis les outils de l'aplat : sélection et
//! transformation (une clé ou plusieurs), peinture et pot de peinture sur plusieurs clés, portée du changement de couleur.
//! (Anthony, 2026-10-28.)

use eframe::egui::{self, pos2, Color32};

use crate::app::{App, Tool};
use crate::document::Document;
use crate::perf_tests::{banc, Banc};

// ------------------------------------------------------------------------------------ l'outillage

/// Un plan de 400 × 300, un calque bitmap et un anneau carré tracé au pinceau (60..160).
fn banc_anneau() -> Banc {
    let mut b = banc(Document::new(400, 300, 24), false);
    b.app.tool = Tool::Brush;
    trait_carre(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    b
}

/// Un anneau carré tracé au pinceau, comme une main : un seul trait fermé.
fn trait_carre(app: &mut App, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut pts = Vec::new();
    let mut x = x0;
    while x < x1 {
        pts.push(pos2(x, y0));
        x += 4.0;
    }
    let mut y = y0;
    while y < y1 {
        pts.push(pos2(x1, y));
        y += 4.0;
    }
    let mut x = x1;
    while x > x0 {
        pts.push(pos2(x, y1));
        x -= 4.0;
    }
    let mut y = y1;
    while y > y0 - 8.0 {
        pts.push(pos2(x0, y));
        y -= 4.0;
    }
    app.begin_paint(pts[0], 1.0, false);
    for p in &pts[1..] {
        app.stroke_to(*p, 1.0);
    }
    app.end_stroke();
}

fn gliss(app: &mut App, a: egui::Pos2, b: egui::Pos2) {
    app.begin_paint(a, 1.0, false);
    for i in 1..=12 {
        let t = i as f32 / 12.0;
        app.stroke_to(a + (b - a) * t, 1.0);
    }
    app.end_stroke();
}

fn noms(app: &App) -> Vec<String> {
    app.history.names().0.iter().map(|n| n.to_string()).collect()
}

fn peint(r: &[Color32]) -> usize {
    r.iter().filter(|c| c.a() > 0).count()
}

/// Le trait du calque `li` à l'image `f`.
fn trait_de(app: &App, li: usize, f: usize) -> Vec<Color32> {
    let cel = app.layers()[li].frames[f].expect("dessin");
    app.doc.cels[&cel].raster.to_flat()
}

/// La peinture de la strate d'aplat du dessin du calque `li` à l'image `f`.
fn strate_de(app: &App, li: usize, f: usize) -> Vec<Color32> {
    let cel = app.layers()[li].frames[f].expect("dessin");
    app.doc.cels[&cel].fond.as_ref().map(|s| s.peinture.raster.to_flat()).unwrap_or_default()
}

// ------------------------------------------------------------------------------------ les deux interrupteurs

#[test]
fn the_two_toggles_are_separate_and_exclusive() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    assert!(!app.paint_behind && !app.paint_on_aplat);
    app.run_action("pinceau.derriere");
    assert!(app.paint_behind && !app.paint_on_aplat);
    // L'autre l'éteint (et inversement).
    app.run_action("pinceau.sur_aplat");
    assert!(!app.paint_behind && app.paint_on_aplat, "{}", app.status);
    app.run_action("pinceau.derriere");
    assert!(app.paint_behind && !app.paint_on_aplat);
    app.run_action("pinceau.derriere");
    assert!(!app.paint_behind && !app.paint_on_aplat);
    // Chacun a son action, son nom et sa phrase dans la liste des raccourcis ; « Peint sur l'aplat » n'a pas de touche d'office.
    let derriere = crate::keymap::ACTIONS.iter().find(|a| a.id == "pinceau.derriere").expect("pinceau.derriere");
    let sur = crate::keymap::ACTIONS.iter().find(|a| a.id == "pinceau.sur_aplat").expect("pinceau.sur_aplat");
    assert!(derriere.label.to_lowercase().contains("derrière"), "{}", derriere.label);
    assert!(sur.label.to_lowercase().contains("sur l'aplat"), "{}", sur.label);
    assert_eq!(app.raccourci("pinceau.sur_aplat"), "", "pas de touche par défaut");
}

#[test]
fn old_settings_with_peint_derriere_give_paint_behind() {
    use crate::settings::Settings;
    // Un ancien fichier : la clé d'avant est « Peindre derrière », « Peint sur l'aplat » est éteint.
    let old: Settings = toml::from_str("peint_derriere = true").unwrap();
    assert!(old.paint_behind && !old.paint_on_aplat);
    // La clé de « Peint sur l'aplat » est à elle, et fait l'aller-retour.
    let mut s = Settings::default();
    s.paint_on_aplat = true;
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("peint_sur_aplat = true") && text.contains("peint_derriere = false"), "{text}");
    let back: Settings = toml::from_str(&text).unwrap();
    assert!(back.paint_on_aplat && !back.paint_behind);
    // L'application les lit au lancement ; un fichier qui aurait les deux garde « Peindre derrière ».
    let dir = std::env::temp_dir().join(format!("shot-aplatsur-reglages-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let fichier = crate::donnees::roaming().expect("dossier des réglages").join("reglages.toml");
    let _ = std::fs::create_dir_all(fichier.parent().unwrap());
    let ctx = egui::Context::default();
    for (texte, derriere, sur) in [("peint_derriere = true", true, false), ("peint_sur_aplat = true", false, true), ("peint_derriere = true\npeint_sur_aplat = true", true, false)] {
        std::fs::write(&fichier, texte).unwrap();
        let app = App::sans_fenetre(&ctx, None);
        assert_eq!((app.paint_behind, app.paint_on_aplat), (derriere, sur), "{texte}");
    }
}

#[test]
fn both_toggles_are_buttons_above_more_settings_with_their_own_tooltips() {
    use egui_phosphor::regular as ph;
    let mut b = banc_anneau();
    b.app.set_tool(Tool::Brush);
    let ctx = egui::Context::default();
    let mut textes: Vec<String> = Vec::new();
    for _ in 0..3 {
        let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(400.0, 900.0))), ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| b.app.tool_options_panel(ui));
        });
        textes.clear();
        fn collecter(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
    }
    assert!(textes.iter().any(|t| t.contains(ph::SELECTION_BACKGROUND)), "« Peindre derrière » : {textes:?}");
    assert!(textes.iter().any(|t| t.contains(ph::SQUARE_HALF_BOTTOM)), "« Peint sur l'aplat » : {textes:?}");
    assert_ne!(ph::SELECTION_BACKGROUND, ph::SQUARE_HALF_BOTTOM, "chacun son icône");
    // Les deux textes de leur bulle d'une seconde sont dans `panels.rs`, reliés à `bulles::bulle` par `interrupteur_a`.
    let source = include_str!("panels.rs");
    assert!(source.contains("\"Peindre derrière\"") && source.contains("\"Peint sur l'aplat\""));
}

// ------------------------------------------------------------------------------------ peint sur l'aplat

#[test]
fn paint_on_aplat_creates_the_stratum_and_never_touches_the_line() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    assert!(app.layers()[0].fond.is_none());
    let trait0 = trait_de(app, 0, 0);
    app.paint_on_aplat = true;
    app.color = [10, 200, 10];
    let n0 = noms(app).len();
    gliss(app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    // L'aplat a été créé (comme le bouton du calque), la peinture est dedans, le trait n'a pas bougé.
    assert!(app.layers()[0].fond.is_some(), "{}", app.status);
    assert_eq!(trait_de(app, 0, 0), trait0, "le trait n'est pas touché");
    assert!(peint(&strate_de(app, 0, 0)) > 0, "la peinture est dans l'aplat");
    assert!(app.status.to_lowercase().contains("aplat"), "la barre d'état le dit : {}", app.status);
    let apres = noms(app);
    assert!(apres.len() >= n0 + 2 && apres.contains(&"Aplat : ajouter".to_string()) && apres.last().map(String::as_str) == Some("Aplat : peinture"), "{apres:?}");
    // Un deuxième trait : l'aplat existe, plus de création.
    let n1 = noms(app).len();
    gliss(app, pos2(90.0, 130.0), pos2(130.0, 130.0));
    assert_eq!(noms(app).len(), n1 + 1, "un geste, une étape : {:?}", noms(app));
    assert_eq!(trait_de(app, 0, 0), trait0);
    // Tout s'annule, jusqu'au trait seul.
    app.undo();
    app.undo();
    app.undo();
    assert!(app.layers()[0].fond.is_none(), "l'aplat créé s'annule aussi");
    assert_eq!(trait_de(app, 0, 0), trait0);
    // Éteint, le pinceau dessine de nouveau le trait.
    app.paint_on_aplat = false;
    gliss(app, pos2(200.0, 200.0), pos2(260.0, 200.0));
    assert_ne!(trait_de(app, 0, 0), trait0);
}

#[test]
fn paint_on_aplat_goes_into_the_colour_layer_of_a_line_layer() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    app.cur_layer = 0;
    app.new_colour_layer();
    assert_eq!(app.layers().len(), 2);
    let (couleur, trait_) = (0usize, 1usize);
    assert!(app.layers()[couleur].couleur.is_some());
    app.cur_layer = trait_;
    app.paint_on_aplat = true;
    app.color = [200, 30, 30];
    let trait0 = trait_de(app, trait_, 0);
    gliss(app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    assert_eq!(app.cur_layer, trait_, "le calque courant n'a pas changé");
    assert_eq!(trait_de(app, trait_, 0), trait0, "le trait n'est pas touché");
    assert!(app.layers()[trait_].fond.is_none(), "pas d'aplat sous le trait : le calque de couleur a servi");
    let c = app.layers()[couleur].frames[0].expect("le dessin de couleur de cette image");
    assert!(peint(&app.doc.cels[&c].raster.to_flat()) > 0, "la peinture est dans le calque de couleur");
    // Un geste, une étape, qui s'annule.
    let avant = noms(app).len();
    app.undo();
    assert_eq!(noms(app).len() + 1, avant, "{:?}", noms(app));
    // (Annuler le trait de couleur, pas autre chose.)
    let c = app.layers()[couleur].frames[0].expect("dessin");
    assert_eq!(peint(&app.doc.cels[&c].raster.to_flat()), 0, "la couleur est revenue à vide");
    assert_eq!(trait_de(app, trait_, 0), trait0);
}

#[test]
fn paint_on_aplat_does_not_divert_the_eraser_or_a_colour_layer() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    app.paint_on_aplat = true;
    // La gomme n'est pas détournée : elle efface du trait, et ne crée pas d'aplat.
    let trait0 = trait_de(app, 0, 0);
    app.set_tool(Tool::Eraser);
    gliss(app, pos2(55.0, 110.0), pos2(65.0, 110.0));
    assert_ne!(trait_de(app, 0, 0), trait0, "la gomme efface du trait");
    assert!(app.layers()[0].fond.is_none());
    // Sur un calque de couleur, rien n'est détourné : on peint la couleur, dans la zone du trait.
    app.set_tool(Tool::Brush);
    app.cur_layer = 0;
    app.new_colour_layer();
    let colour = app.cur_layer;
    assert!(app.layers()[colour].couleur.is_some());
    assert_eq!(app.sur_aplat_cible(), None, "un calque de couleur est déjà l'aplat");
    // La sous-ligne de l'aplat déjà choisie : elle reste la cible, rien à détourner.
    app.cur_layer = 1;
    app.toggle_fond(1);
    app.select_aplat(1);
    assert_eq!(app.sur_aplat_cible(), None);
    // Le pot de peinture ne l'est pas non plus.
    app.paint_on_aplat = true;
    app.set_tool(Tool::Fill);
    assert_eq!(app.sur_aplat_cible(), None);
}
