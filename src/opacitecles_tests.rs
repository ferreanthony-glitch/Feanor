//! Les clés d'opacité (`opacitecles.rs`, `opacitecleselui.rs`) : valeur entre les clés, adoucis, suivi des images, enregistrement,
//! composition (écran, vignettes, exports), annulation d'un geste en une étape.

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::opacitecles::{self as ok, OpKey};

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-opacite-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let mut app = App::sans_fenetre(&egui::Context::default(), None);
    app.welcome = false;
    app
}

/// Un plan de 12 images, un calque bitmap tenu sur tout le plan avec un pixel rouge plein en (10, 10).
fn avec_calque(name: &str) -> App {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    {
        let c = doc.cels.get_mut(&cel).unwrap();
        for y in 0..64 {
            for x in 0..64 {
                c.raster.tile_mut(0)[y * 64 + x] = Color32::from_rgb(255, 0, 0);
            }
        }
        c.version += 1;
    }
    let mut layer = Layer::new("Rouge", LayerKind::Bitmap);
    layer.frames = vec![Some(cel); 12];
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = 12;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

fn pixel(doc: &Document, frame: usize, x: usize, y: usize) -> Color32 {
    let plan = crate::render::panel_plan_at(doc, 0, frame, Some([255, 255, 255]));
    crate::render::composite_pixel(doc, &plan, x, y)
}

#[test]
fn the_value_goes_straight_between_two_keys_and_stays_before_and_after() {
    let keys = vec![OpKey::new(2, 0.0), OpKey::new(10, 1.0)];
    assert_eq!(ok::value_at(&[], 5, 0.7), 0.7, "sans clé : l'opacité fixe");
    assert_eq!(ok::value_at(&keys[..1], 5, 0.7), 0.0, "une clé seule vaut partout");
    assert_eq!(ok::value_at(&keys, 0, 0.7), 0.0, "avant la première");
    assert_eq!(ok::value_at(&keys, 2, 0.7), 0.0);
    assert!((ok::value_at(&keys, 6, 0.7) - 0.5).abs() < 1e-5, "à mi-chemin : la moitié");
    assert!((ok::value_at(&keys, 4, 0.7) - 0.25).abs() < 1e-5);
    assert_eq!(ok::value_at(&keys, 10, 0.7), 1.0);
    assert_eq!(ok::value_at(&keys, 11, 0.7), 1.0, "après la dernière");
}

#[test]
fn smoothing_slows_the_start_and_the_end_like_the_other_keys() {
    let mut keys = vec![OpKey::new(0, 0.0), OpKey::new(10, 1.0)];
    let droit = ok::value_at(&keys, 2, 0.0);
    assert!(ok::set_eases(&mut keys, 0, true, 0.0, 0.0));
    assert!(ok::set_eases(&mut keys, 10, true, 0.0, 0.0));
    let lisse = ok::value_at(&keys, 2, 0.0);
    assert!(lisse < droit, "lissée, elle part plus lentement : {lisse} < {droit}");
    assert!((ok::value_at(&keys, 5, 0.0) - 0.5).abs() < 1e-4, "et passe toujours par le milieu");
    assert!(!ok::set_eases(&mut keys, 3, true, 0.0, 0.0), "pas de clé à l'image 3");
}

#[test]
fn setting_a_key_keeps_the_eases_and_the_values_are_clamped() {
    let mut keys = Vec::new();
    ok::set_key(&mut keys, 4, 0.5);
    ok::set_key(&mut keys, 1, 2.0);
    ok::set_key(&mut keys, 8, -3.0);
    assert_eq!(keys.iter().map(|k| (k.frame, k.value)).collect::<Vec<_>>(), vec![(1, 1.0), (4, 0.5), (8, 0.0)], "triées et ramenées entre 0 et 1");
    ok::set_eases(&mut keys, 4, true, 0.0, 0.0);
    ok::set_key(&mut keys, 4, 0.9);
    assert!(keys[1].smooth && keys[1].value == 0.9, "la valeur change, l'adouci reste");
    assert_eq!(ok::clamp(f32::NAN), 1.0);
}

#[test]
fn the_keys_follow_inserted_and_removed_frames_and_survive_a_cut_and_a_join() {
    let mut keys = vec![OpKey::new(2, 0.2), OpKey::new(6, 0.6), OpKey::new(9, 0.9)];
    ok::shift_keys(&mut keys, 4, 3, false);
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 9, 12]);
    ok::shift_keys(&mut keys, 8, 2, true);
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 10], "la clé de l'image 9 était dans les images retirées");
    // Coupe : les deux parties gardent la valeur qu'elles avaient à la coupure.
    let k = vec![OpKey::new(0, 0.0), OpKey::new(10, 1.0)];
    let (a, b) = ok::split_keys(&k, 5, 0.0);
    assert!((ok::value_at(&a, 4, 0.0) - 0.4).abs() < 1e-5);
    assert!((ok::value_at(&b, 0, 0.0) - 0.5).abs() < 1e-5, "le second plan commence à 0,5");
    assert!((ok::value_at(&b, 5, 0.0) - 1.0).abs() < 1e-5);
    // Réunion : un plan fixe à 1 puis un plan animé.
    let joined = ok::join_keys(&[], 4, 1.0, &[OpKey::new(0, 0.0), OpKey::new(3, 1.0)], 4, 1.0);
    assert!((ok::value_at(&joined, 3, 1.0) - 1.0).abs() < 1e-5);
    assert!((ok::value_at(&joined, 4, 1.0) - 0.0).abs() < 1e-5, "le deuxième plan démarre à 0");
    assert!(ok::join_keys(&[], 4, 0.5, &[], 4, 0.5).is_empty(), "rien à garder si rien ne bouge");
}

#[test]
fn moving_keys_never_leaves_the_plan_and_a_landing_key_replaces_the_one_there() {
    let keys = vec![OpKey::new(1, 0.1), OpKey::new(3, 0.3), OpKey::new(5, 0.5)];
    assert_eq!(ok::clamp_delta(&[1, 3], -4, 11), -1);
    assert_eq!(ok::clamp_delta(&[3, 5], 20, 11), 6);
    let m = ok::moved(&keys, &[3], 2, 11);
    assert_eq!(m.iter().map(|k| (k.frame, k.value)).collect::<Vec<_>>(), vec![(1, 0.1), (5, 0.3)], "la clé qui arrive remplace celle de l'image 5");
}

#[test]
fn the_project_entries_round_trip_and_damaged_ones_are_dropped() {
    let mut keys = vec![OpKey::new(0, 0.25), OpKey::new(7, 1.0)];
    ok::set_eases(&mut keys, 7, false, 2.0, 3.0);
    let entries: Vec<ok::Entree> = keys.iter().map(ok::Entree::from).collect();
    let json = serde_json::to_string(&entries).unwrap();
    assert!(!json.contains("lisse"), "ce qui est par défaut ne s'écrit pas : {json}");
    let back = ok::from_entries(serde_json::from_str(&json).unwrap());
    assert_eq!(back, keys);
    let abimee = ok::Entree { image: 3, valeur: f32::NAN, lisse: false, adoucir_entree: 0.0, adoucir_sortie: 0.0 };
    assert!(ok::from_entries(vec![abimee]).is_empty());
}

#[test]
fn the_layer_opacity_is_animated_in_the_plan_the_screen_and_the_thumbnails_use() {
    let mut app = avec_calque("rendu");
    app.layers_mut()[0].set_opacity_keys(&[(0, 1.0), (10, 0.0)]);
    let doc = &app.doc;
    // L'image 5 est à mi-chemin : le plan porte 50 % (128 sur 255), l'image composée mêle le rouge au papier blanc.
    let plan = crate::render::panel_plan_at(doc, 0, 5, Some([255, 255, 255]));
    assert!((plan.items[0].opacity as i32 - 128).abs() <= 1, "opacité du plan à l'image 5 : {}", plan.items[0].opacity);
    let c = pixel(doc, 5, 10, 10);
    assert!(c.r() == 255 && (c.g() as i32 - 127).abs() <= 3 && (c.b() as i32 - 127).abs() <= 3, "rouge à moitié sur blanc : {c:?}");
    assert_eq!(pixel(doc, 0, 10, 10), Color32::from_rgb(255, 0, 0), "image 0 : opaque");
    assert_eq!(pixel(doc, 10, 10, 10), Color32::from_rgb(255, 255, 255), "image 10 : disparu");
    // L'image entière (exports) et la vignette suivent.
    let plan5 = crate::render::panel_plan_at(doc, 0, 5, Some([255, 255, 255]));
    let full = crate::render::composite_full(doc, &plan5);
    assert_eq!(full[10 * doc.width + 10], c, "l'image entière donne le même pixel que le calcul du pixel");
    let cam = doc.default_camera();
    let vignette = crate::render::thumbnail(doc, &plan5, &cam, 100, 75);
    let v = vignette.pixels[5 * 100 + 5];
    assert!(v.r() == 255 && (v.g() as i32 - 127).abs() <= 6, "la vignette montre l'opacité de l'image 5 : {v:?}");
    // Un dossier multiplie l'opacité du calque, clés comprises.
    assert_eq!(crate::folders::effect_at(app.layers(), 0, 5).map(|o| (o * 100.0).round()), Some(50.0));
}

#[test]
fn the_key_button_the_eases_and_the_deletions_are_single_named_undo_steps() {
    let mut app = avec_calque("annuler");
    app.layers_mut()[0].opacity = 0.8;
    let avant = app.history.names().0.len();
    app.run_action("calque.cle_opacite");
    assert_eq!(app.layers()[0].opacity_keys.len(), 1);
    assert!((app.layers()[0].opacity_keys[0].value - 0.8).abs() < 1e-6, "la clé prend l'opacité actuelle");
    assert_eq!(app.history.names().0.len(), avant + 1, "une seule étape");
    assert!(app.history.names().0.last().is_some_and(|n| n.contains("clé d'opacité")), "{:?}", app.history.names().0.last());
    // Image 8, on règle l'opacité (le champ du panneau) : une clé naît là.
    app.cur_frame = 8;
    let n = app.history.names().0.len();
    app.record_step();
    app.layers_mut()[0].set_opacity_at(8, 0.0);
    assert_eq!(app.layers()[0].opacity_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 8]);
    assert!((app.layers()[0].opacity_at(4) - 0.4).abs() < 1e-5);
    assert_eq!(app.layers()[0].opacity, 0.8, "l'opacité fixe n'a pas bougé");
    assert_eq!(app.history.names().0.len(), n + 1);
    // Lisser, supprimer la clé d'ici, tout supprimer.
    let n = app.history.names().0.len();
    app.run_action("calque.cle_opacite_lisser");
    assert!(app.layers()[0].opacity_keys[1].smooth);
    assert_eq!(app.history.names().0.len(), n + 1);
    app.run_action("calque.cle_opacite_supprimer");
    assert_eq!(app.layers()[0].opacity_keys.len(), 1);
    app.undo();
    assert_eq!(app.layers()[0].opacity_keys.len(), 2, "annulé d'un coup");
    app.cur_frame = 4;
    let a_4 = app.layers()[0].opacity_at(4);
    app.run_action("calque.cles_opacite_supprimer");
    assert!(app.layers()[0].opacity_keys.is_empty());
    assert!((app.layers()[0].opacity - a_4).abs() < 1e-6, "le calque garde l'opacité qu'il avait à l'image courante");
    app.undo();
    assert_eq!(app.layers()[0].opacity_keys.len(), 2);
}

#[test]
fn dragging_keys_in_the_timeline_is_one_undo_step() {
    let mut app = avec_calque("glisser");
    app.layers_mut()[0].set_opacity_keys(&[(2, 1.0), (4, 0.5), (8, 0.0)]);
    let n = app.history.names().0.len();
    assert!(app.begin_op_drag(0, 4, 4));
    app.drag_op_keys(5);
    app.drag_op_keys(6);
    assert_eq!(app.layers()[0].opacity_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 6, 8]);
    app.end_op_drag();
    assert_eq!(app.history.names().0.len(), n + 1, "tout le geste est une étape");
    assert!(app.history.names().0.last().is_some_and(|nom| nom.contains("clés d'opacité")));
    app.undo();
    assert_eq!(app.layers()[0].opacity_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 4, 8]);
    // Échap pendant le geste : tout revient, rien n'est enregistré.
    let n = app.history.names().0.len();
    assert!(app.begin_op_drag(0, 8, 8));
    app.drag_op_keys(10);
    app.cancel_op_drag();
    assert_eq!(app.layers()[0].opacity_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 4, 8]);
    assert_eq!(app.history.names().0.len(), n);
}

#[test]
fn the_import_api_lays_keys_on_a_layer_and_a_plan_split_keeps_the_animation() {
    let mut app = avec_calque("api");
    app.layers_mut()[0].set_opacity_keys(&[(9, 0.0), (0, 1.0), (0, 0.9)]);
    assert_eq!(app.layers()[0].opacity_keys.iter().map(|k| (k.frame, k.value)).collect::<Vec<_>>(), vec![(0, 0.9), (9, 0.0)], "triées, une par image");
    // Un calque fixe : set_opacity_at change l'opacité fixe, pas de clé.
    let mut l = Layer::new("x", LayerKind::Bitmap);
    l.set_opacity_at(3, 0.4);
    assert_eq!((l.opacity, l.opacity_keys.len()), (0.4, 0));
    // Couper le plan en deux : les deux parties continuent la même courbe.
    let index = 0;
    app.doc.split_panel(index, 5);
    let (a, b) = (app.doc.panels[0].layers[0].opacity_at(4), app.doc.panels[1].layers[0].opacity_at(0));
    let attendu = |f: usize| 0.9 - 0.9 * (f as f32 / 9.0);
    assert!((a - attendu(4)).abs() < 1e-4, "{a}");
    assert!((b - attendu(5)).abs() < 1e-4, "{b}");
}

#[test]
fn a_project_with_opacity_keys_is_saved_and_reopened_unchanged_and_an_old_one_opens() {
    let mut app = avec_calque("fichier");
    app.layers_mut()[0].set_opacity_keys(&[(0, 1.0), (6, 0.25), (11, 0.0)]);
    ok::set_eases(&mut app.layers_mut()[0].opacity_keys, 6, true, 0.0, 0.0);
    app.layers_mut()[0].opacity = 0.7;
    let dir = std::env::temp_dir().join(format!("shot-opacite-fichier-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("O.shot");
    crate::project::save(&app.doc, &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    let (a, b) = (&app.doc.panels[0].layers[0], &relu.doc.panels[0].layers[0]);
    assert_eq!(b.opacity_keys, a.opacity_keys, "les clés reviennent identiques");
    assert_eq!(b.opacity, 0.7);
    for f in 0..12 {
        assert!((a.opacity_at(f) - b.opacity_at(f)).abs() < 1e-6, "image {f}");
    }
    // Un ancien projet (format 42, sans ces champs) s'ouvre sans perte.
    let mut sans = avec_calque("fichier-ancien");
    sans.layers_mut()[0].opacity = 0.6;
    let manifeste42 = dir.join("Vieux.shot");
    crate::project::save(&sans.doc, &manifeste42, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifeste42).unwrap();
    assert!(!texte.contains("cles_opacite") && !texte.contains("couleur_unie"), "rien de nouveau n'est écrit tant qu'on ne s'en sert pas");
    let mut v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    v["version"] = serde_json::json!(42);
    std::fs::write(&manifeste42, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let ancien = crate::project::load(&manifeste42).expect("un projet de format 42 s'ouvre");
    let l = &ancien.doc.panels[0].layers[0];
    assert_eq!((l.opacity, l.opacity_keys.len(), l.unie), (0.6, 0, None));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_duplicated_layer_and_the_keys_of_the_plan_strip_keep_the_opacity_keys() {
    let mut app = avec_calque("dupliquer");
    app.layers_mut()[0].set_opacity_keys(&[(2, 1.0), (7, 0.0)]);
    crate::layerops::duplicate_layer(&mut app.doc, 0, 0);
    assert_eq!(app.doc.panels[0].layers[1].opacity_keys, app.doc.panels[0].layers[0].opacity_keys);
    let traits = crate::clesplan::cles_du_plan(&app.doc.panels[0]);
    assert_eq!(traits.iter().map(|(f, _)| *f).filter(|f| [2, 7].contains(f)).count(), 2, "les clés d'opacité ont leur trait sur le bloc du plan");
}
