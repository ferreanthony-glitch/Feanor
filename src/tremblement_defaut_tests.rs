//! Le tremblement du trait par défaut (Anthony, 2026-10-24 ; Préférences > Dessin) : un réglage de l'application, appliqué au
//! rendu à tout calque vectoriel qui n'a pas son propre « Tremblement du trait », sans rien écrire dans les projets.

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::document::{Document, LayerKind};
use crate::fx::{self, Effect, EffectKind};
use crate::raster::TILE;
use crate::render;
use crate::tremblement::{self as t, Defaut};

/// Un document de `frames` images avec un calque vectoriel (un dessin de lignes) et la pile d'effets `fx`.
fn doc_with(frames: usize, kind: LayerKind, fx: Vec<Effect>) -> Document {
    let mut doc = Document::new(256, 128, 24);
    let id = doc.new_cel(kind);
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        for y in 30..100 {
            if y % 14 < 4 {
                for x in 20..236 {
                    let i = cel.raster.tile_index_of(x, y);
                    cel.raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(0, 0, 0, 255);
                }
            }
        }
        cel.version = 1;
    }
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = kind;
    layer.frames = vec![Some(id); frames];
    layer.fx = fx;
    doc
}

fn image(doc: &Document, frame: usize) -> Vec<Color32> {
    render::composite_full(doc, &render::panel_plan_at(doc, 0, frame, None))
}

fn allume(intensite: f32) {
    t::set_defaut(Some(Defaut { actif: true, intensite, frequence: 60.0 }));
}

fn own_boil(intensity: f32, enabled: bool) -> Effect {
    let mut e = Effect::new(EffectKind::Boil);
    e.values[t::INTENSITY] = intensity;
    e.enabled = enabled;
    e
}

#[test]
fn turning_it_on_changes_the_composed_vector_pixels_and_turning_it_off_gives_them_back() {
    crate::fxcache::monde::ecrire();
    let doc = doc_with(12, LayerKind::Vector, Vec::new());
    t::set_defaut(None);
    let eteint = image(&doc, 2);
    allume(6.0);
    let allume_ = image(&doc, 2);
    assert!(eteint != allume_, "le tremblement par défaut change l'image composée");
    // Il bouge d'une variante à l'autre, comme l'effet : deux images par variante, trois variantes.
    assert!(image(&doc, 2) == image(&doc, 3));
    assert!(image(&doc, 2) != image(&doc, 4));
    assert!(image(&doc, 2) == image(&doc, 8), "trois variantes : on revient au bout de six images");
    // Une intensité nulle ne change rien.
    t::set_defaut(Some(Defaut { actif: true, intensite: 0.0, frequence: 60.0 }));
    assert!(image(&doc, 2) == eteint, "intensité nulle");
    t::set_defaut(None);
    assert!(image(&doc, 2) == eteint, "éteint : l'image d'avant, au pixel près");
}

#[test]
fn a_layer_with_its_own_tremble_setting_keeps_its_own() {
    crate::fxcache::monde::ecrire();
    t::set_defaut(None);
    let propre = doc_with(12, LayerKind::Vector, vec![own_boil(6.0, true)]);
    let avant = image(&propre, 2);
    allume(1.0);
    assert!(image(&propre, 2) == avant, "son propre réglage, pas celui par défaut");
    // Un tremblement éteint à la main reste éteint : le calque a « son » réglage.
    t::set_defaut(None);
    let eteint_a_la_main = doc_with(12, LayerKind::Vector, vec![own_boil(6.0, false)]);
    let nu = image(&eteint_a_la_main, 2);
    allume(6.0);
    assert!(image(&eteint_a_la_main, 2) == nu, "un tremblement éteint à la main n'est pas remplacé par celui par défaut");
    // Les effets du calque restent à leur place : un autre effet n'empêche pas le tremblement par défaut.
    let mut flou = Effect::new(EffectKind::Invert);
    flou.enabled = true;
    let avec_autre = doc_with(12, LayerKind::Vector, vec![flou]);
    let steps = fx::layer_steps(&avec_autre.panels[0].layers[0], 2, None);
    assert_eq!(steps.first().map(|s| s.kind), Some(EffectKind::Boil), "en tête de la pile");
    assert!(steps.iter().any(|s| s.kind == EffectKind::Invert));
    // Le bouton « fx » éteint du calque l'éteint aussi.
    let mut sans_fx = doc_with(12, LayerKind::Vector, Vec::new());
    sans_fx.panels[0].layers[0].fx_enabled = false;
    assert!(fx::layer_steps(&sans_fx.panels[0].layers[0], 2, None).is_empty());
    t::set_defaut(None);
}

#[test]
fn it_only_touches_vector_layers_and_never_the_editing_view() {
    crate::fxcache::monde::ecrire();
    allume(4.0);
    let bitmap = doc_with(4, LayerKind::Bitmap, Vec::new());
    assert!(fx::layer_steps(&bitmap.panels[0].layers[0], 0, None).is_empty(), "un calque bitmap ne tremble pas");
    let vecto = doc_with(4, LayerKind::Vector, Vec::new());
    let steps = fx::layer_steps(&vecto.panels[0].layers[0], 0, None);
    assert_eq!(steps.len(), 1);
    // Le calque qu'on édite se montre sans tremblement (on dessine et on prend ce qu'on voit) : le même chemin que l'effet.
    assert!(fx::without_boil(steps).is_empty());
    t::set_defaut(None);
}

#[test]
fn it_writes_nothing_into_the_document() {
    crate::fxcache::monde::ecrire();
    let doc = doc_with(12, LayerKind::Vector, Vec::new());
    let ecrit = |doc: &Document| serde_json::to_string(&crate::project::snapshot_value(&doc.panels[0], &|c| c).unwrap()).unwrap();
    let avant = ecrit(&doc);
    allume(6.0);
    let _ = image(&doc, 2);
    let _ = fx::layer_steps(&doc.panels[0].layers[0], 3, None);
    assert!(doc.panels[0].layers[0].fx.is_empty(), "aucun effet ajouté au calque");
    assert_eq!(ecrit(&doc), avant, "ce qui s'écrit dans le projet est le même, tremblement par défaut allumé ou non");
    t::set_defaut(None);
    assert_eq!(ecrit(&doc), avant);
}

#[test]
fn the_preference_is_saved_and_reloaded_and_stays_out_of_the_file_when_untouched() {
    let defaut = crate::settings::Settings::default();
    assert!(!defaut.tremblement_defaut.actif, "éteint d'origine");
    assert!(!toml::to_string(&defaut).unwrap().contains("tremblement_par_defaut"), "rien d'écrit tant qu'on n'y touche pas");
    let ancien: crate::settings::Settings = toml::from_str("").unwrap();
    assert_eq!(ancien.tremblement_defaut, Defaut::default(), "des réglages d'avant : éteint");
    let mut s = crate::settings::Settings::default();
    s.tremblement_defaut = Defaut { actif: true, intensite: 4.5, frequence: 80.0 };
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("tremblement_par_defaut"), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.tremblement_defaut, Defaut { actif: true, intensite: 4.5, frequence: 80.0 });
    // Un fichier modifié à la main : valeurs bornées.
    let fou = Defaut { actif: true, intensite: 900.0, frequence: f32::NAN }.sanitized();
    assert_eq!((fou.intensite, fou.frequence), (t::MAX_INTENSITY, 55.0));
}

// ------------------------------------------------------------------------------------ dans l'application

fn app_avec(doc: Document, nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-tremble-defaut-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 2;
    app
}

#[test]
fn the_app_applies_the_preference_to_the_screen_plan_and_to_the_export_images() {
    crate::fxcache::monde::ecrire();
    let mut app = app_avec(doc_with(12, LayerKind::Vector, Vec::new()), "app");
    let ctx = egui::Context::default();
    t::set_defaut(None);
    app.appliquer_tremblement_defaut(&ctx);
    let export = |app: &App| render::composite_full(&app.doc, &app.build_plan(2, false, true));
    let eteint = export(&app);
    assert!(app.build_plan(2, false, true).items.iter().all(|i| i.fx.is_empty()));
    app.settings.tremblement_defaut = Defaut { actif: true, intensite: 6.0, frequence: 60.0 };
    app.appliquer_tremblement_defaut(&ctx);
    assert_eq!(t::defaut(), Some(app.settings.tremblement_defaut), "posé là où le rendu le lit");
    let plan = app.build_plan(2, false, true);
    assert!(plan.items.iter().any(|i| i.fx.iter().any(|s| s.kind == EffectKind::Boil)), "le plan de l'écran porte le tremblement");
    assert!(export(&app) != eteint, "l'image exportée le montre");
    // Même chemin que l'effet : une étape du rendu, pas un rejeu sur le fil de l'interface (le dessin lui-même ne change pas).
    assert!(app.doc.panels[0].layers[0].fx.is_empty());
    // Éteint dans les réglages : retiré.
    app.settings.tremblement_defaut.actif = false;
    app.appliquer_tremblement_defaut(&ctx);
    assert_eq!(t::defaut(), None);
    assert!(export(&app) == eteint, "éteint : l'image d'avant");
}

#[test]
fn the_preferences_window_has_the_default_tremble_settings() {
    use crate::prefsonglets::Onglet;
    let mut app = app_avec(doc_with(2, LayerKind::Vector, Vec::new()), "prefs");
    app.settings.prefs_onglet = Onglet::Dessin;
    app.prefs_window = true;
    let ctx = egui::Context::default();
    let mut textes: Vec<String> = Vec::new();
    for k in 0..4 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))), time: Some(k as f64 / 60.0), ..Default::default() };
        let out = ctx.run(input, |ctx| app.preferences_window(ctx));
        textes.clear();
        fn collecter(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        out.shapes.iter().for_each(|s| collecter(&s.shape, &mut textes));
    }
    for libelle in ["Tremblement du trait par défaut", "Intensité du tremblement par défaut", "Fréquence du tremblement par défaut"] {
        assert!(textes.iter().any(|t| t.contains(libelle)), "« {libelle} » : {textes:?}");
    }
}
