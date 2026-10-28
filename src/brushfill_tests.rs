//! Peint derrière et remplir la forme : quand un trait est fermé, quel contour se remplit, et
//! comment les couleurs se posent.

use eframe::egui::{pos2, Color32, Pos2};

use crate::brushfill::{closed_outline, fill_under_stroke, over_premul, paint_under, solid};

/// Un cercle de rayon 100 autour de (200, 200), de `turn` tours (1 : fermé pile).
fn circle(turn: f32, steps: usize) -> Vec<Pos2> {
    (0..=steps)
        .map(|k| {
            let a = turn * std::f32::consts::TAU * k as f32 / steps as f32;
            pos2(200.0 + 100.0 * a.cos(), 200.0 + 100.0 * a.sin())
        })
        .collect()
}

#[test]
fn a_stroke_that_comes_back_to_its_start_is_closed() {
    // Le bout revient près du début : tout le trait se remplit.
    let mut c = circle(0.97, 60);
    assert!(closed_outline(&c, 25.0).is_some(), "à moins de 25 px du début");
    assert!(closed_outline(&c, 5.0).is_none(), "trop loin du début pour 5 px");
    // Un simple aller-retour court n'entoure rien.
    c = vec![pos2(0.0, 0.0), pos2(5.0, 0.0), pos2(1.0, 0.0)];
    assert!(closed_outline(&c, 25.0).is_none());
    // Un trait ouvert (un demi-cercle) ne se remplit pas.
    assert!(closed_outline(&circle(0.5, 30), 25.0).is_none());
}

#[test]
fn an_overshooting_loop_fills_its_loop() {
    // 1,2 tour : le trait se recoupe ; la boucle se remplit, pas le bout qui dépasse.
    let c = circle(1.2, 72);
    let outline = closed_outline(&c, 5.0).expect("il se recoupe");
    assert!(outline.len() >= 50 && outline.len() < c.len(), "la boucle, sans le dépassement");
    // Tous ses points sont sur le cercle (à peu près).
    assert!(outline.iter().all(|p| (p.distance(pos2(200.0, 200.0)) - 100.0).abs() < 2.0));
}

#[test]
fn paint_behind_goes_under_what_is_drawn() {
    let red = [255, 0, 0];
    // Sur du vide : la couleur entière.
    assert_eq!(paint_under(Color32::TRANSPARENT, red, 1.0), Color32::from_rgb(255, 0, 0));
    // Sous un pixel opaque : rien ne change.
    let blue = Color32::from_rgb(0, 0, 255);
    assert_eq!(paint_under(blue, red, 1.0), blue);
    // Sous un pixel à moitié couvert : on voit le rouge à travers.
    let half = Color32::from_rgba_premultiplied(0, 0, 128, 128);
    let p = paint_under(half, red, 1.0);
    assert_eq!(p.a(), 255);
    assert!(p.r() > 120 && p.b() > 120);
    assert_eq!(over_premul(Color32::TRANSPARENT, blue), blue);
}

#[test]
fn the_fill_slides_between_the_stroke_and_what_was_there() {
    let yellow = solid([255, 255, 0], 1.0);
    let white = Color32::WHITE;
    // Pas de trait ici (couverture 0) : le remplissage recouvre le dessin d'avant.
    assert_eq!(fill_under_stroke(white, white, 0.0, yellow), yellow);
    // Trait noir plein : il reste dessus.
    let black = Color32::BLACK;
    assert_eq!(fill_under_stroke(black, white, 1.0, yellow), black);
    // Bord doux du trait (à moitié couvert, sur du vide) : le remplissage passe dessous, sans
    // liseré transparent entre le trait et la zone.
    let edge = Color32::from_rgba_premultiplied(0, 0, 0, 128);
    let p = fill_under_stroke(edge, Color32::TRANSPARENT, 128.0 / 255.0, yellow);
    assert_eq!(p.a(), 255);
    assert!(p.r() > 100 && p.r() < 140, "moitié noir, moitié jaune : {p:?}");
}

#[test]
fn the_brush_options_are_remembered_between_sessions() {
    // Anthony : « retiens-les, et garde-les pour le crayon ».
    use crate::settings::Settings;
    let mut s = Settings::default();
    assert!(!s.paint_behind, "éteinte au premier lancement");
    s.paint_behind = true;
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("peint_derriere = true"), "« Peindre derrière » garde sa clé d'avant");
    assert!(text.contains("peint_sur_aplat = false"), "« Peint sur l'aplat » a sa clé à lui, éteinte d'office");
    assert!(!text.contains("remplir_la_forme"), "« Remplir la forme » n'est plus écrit (retiré le 2026-10-23)");
    let back: Settings = toml::from_str(&text).unwrap();
    assert!(back.paint_behind);
    // Un fichier de réglages d'avant, sans ces lignes : éteinte.
    let old: Settings = toml::from_str("copie_secours_minutes = 1").unwrap();
    assert!(!old.paint_behind);
    // Un fichier de réglages d'avant où « Remplir la forme » était allumé s'ouvre toujours, sans rien perdre d'autre ;
    // l'option n'a plus d'effet et disparaît au prochain enregistrement.
    let old: Settings = toml::from_str("peint_derriere = true\nremplir_la_forme = true\ncopie_secours_minutes = 3").unwrap();
    assert!(old.paint_behind && old.ancien_remplir_la_forme && old.autosave_minutes == 3);
    assert!(!toml::to_string(&old).unwrap().contains("remplir_la_forme"));
}

#[test]
fn the_old_fill_shape_option_no_longer_fills_and_its_shortcut_toggles_the_aplat() {
    use crate::document::{Layer, LayerKind};
    let dir = std::env::temp_dir().join(format!("shot-remplir-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
    app.cur_layer = 0;
    // L'action d'avant (un raccourci posé dessus) : l'aplat du calque courant, comme « pinceau.fond ».
    assert!(app.layers()[0].fond.is_none());
    app.run_action("pinceau.remplir");
    assert!(app.layers()[0].fond.is_some(), "« pinceau.remplir » donne un aplat au calque : {}", app.status);
    app.run_action("pinceau.remplir");
    assert!(app.layers()[0].fond.is_none(), "et le retire");
    assert!(crate::keymap::ALIASES.contains(&("pinceau.remplir", "pinceau.fond")));
}