//! Le calque « Couleur unie » (`couleurunie.rs`) : un calque de dessin ordinaire avec un fond d'une couleur sur toute la feuille.

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-unie-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let mut app = App::sans_fenetre(&egui::Context::default(), None);
    app.welcome = false;
    app
}

/// Un plan de 12 images, un calque bitmap « Fond » avec un petit dessin (rouge, 20 × 20) tenu sur tout le plan.
fn banc(name: &str) -> App {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut layer = Layer::new("Fond", LayerKind::Bitmap);
    layer.frames = vec![Some(cel); 12];
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = 12;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

const ROUGE: Color32 = Color32::from_rgb(255, 0, 0);
const BLANC: [u8; 3] = [255, 255, 255];

fn pixel(doc: &Document, frame: usize, x: usize, y: usize) -> Color32 {
    let plan = crate::render::panel_plan_at(doc, 0, frame, Some(BLANC));
    crate::render::composite_pixel(doc, &plan, x, y)
}

/// Dessine (ou efface, `c` transparent) un carré 20 × 20 dans le dessin du calque `li` à l'image `frame`.
fn carre(app: &mut App, li: usize, frame: usize, c: Color32) {
    let cel = app.doc.panels[0].layers[li].cel_at(frame).expect("un dessin");
    let cel = app.doc.cels.get_mut(&cel).unwrap();
    for y in 10..30 {
        for x in 10..30 {
            let (tx, ty) = (x / 64, y / 64);
            let idx = ty * cel.raster.tiles_x + tx;
            cel.raster.tile_mut(idx)[(y % 64) * 64 + x % 64] = c;
        }
    }
    cel.version += 1;
}

#[test]
fn the_new_layer_covers_the_whole_sheet_in_mid_grey_in_one_undo_step() {
    let mut app = banc("creer");
    let n = app.history.names().0.len();
    app.run_action("calque.nouveau_couleur_unie");
    assert_eq!(app.layers().len(), 2);
    let l = &app.layers()[1];
    assert_eq!((l.unie, l.kind, l.name.as_str()), (Some(crate::couleurunie::DEFAUT), LayerKind::Bitmap, "Couleur unie"));
    assert_eq!(app.cur_layer, 1, "le nouveau calque devient courant");
    assert_eq!(app.history.names().0.len(), n + 1, "une seule étape d'annulation");
    // Toute la feuille, coins compris, et même sur une image sans dessin.
    for (x, y) in [(0, 0), (399, 0), (0, 299), (399, 299), (200, 150)] {
        for f in [0, 11] {
            assert_eq!(pixel(&app.doc, f, x, y), Color32::from_rgb(128, 128, 128), "({x}, {y}) à l'image {f}");
        }
    }
    app.run_action("calque.nouveau_couleur_unie");
    assert_eq!(app.layers()[2].name, "Couleur unie 2", "un nom libre");
    app.undo();
    assert_eq!(app.layers().len(), 2);
}

#[test]
fn the_card_follows_opacity_blend_visibility_and_the_stack_order() {
    let mut app = banc("reglages");
    carre(&mut app, 0, 0, ROUGE);
    // Le calque couleur unie est sous le calque « Fond » (rang 0) : le dessin rouge reste visible dessus.
    let id = app.doc.ajouter_couleur_unie(0, [0, 0, 255], None, Some(0)).unwrap();
    assert_eq!(app.layers()[0].id, id);
    assert_eq!(pixel(&app.doc, 0, 20, 20), ROUGE, "le dessin du calque d'au-dessus passe devant");
    assert_eq!(pixel(&app.doc, 0, 100, 100), Color32::from_rgb(0, 0, 255));
    // Opacité : 50 % de bleu sur le papier blanc.
    app.layers_mut()[0].opacity = 0.5;
    let c = pixel(&app.doc, 0, 100, 100);
    assert!((c.r() as i32 - 127).abs() <= 2 && (c.b() as i32 - 255).abs() <= 1, "{c:?}");
    // Caché : plus rien.
    app.layers_mut()[0].visible = false;
    assert_eq!(pixel(&app.doc, 0, 100, 100), Color32::from_rgb(255, 255, 255));
    app.layers_mut()[0].visible = true;
    // Mode de fusion : Multiplier le bleu sur le papier blanc donne le bleu (et pas plus clair).
    app.layers_mut()[0].opacity = 1.0;
    app.layers_mut()[0].blend = crate::blend::BlendMode::Multiply;
    assert_eq!(pixel(&app.doc, 0, 100, 100), Color32::from_rgb(0, 0, 255));
    // Dans un dossier caché, il disparaît avec lui.
    let mut dossier = Layer::new("Dossier", LayerKind::Vector);
    dossier.folder = true;
    app.layers_mut().insert(1, dossier);
    let did = app.layers()[1].id;
    app.layers_mut()[0].parent = Some(did);
    crate::folders::normalize(app.layers_mut());
    let bleu = app.layers().iter().position(|l| l.unie.is_some()).unwrap();
    let dossier_i = app.layers().iter().position(|l| l.folder).unwrap();
    app.layers_mut()[dossier_i].visible = false;
    assert_eq!(app.layers()[bleu].parent, Some(did));
    assert_eq!(pixel(&app.doc, 0, 100, 100), Color32::from_rgb(255, 255, 255), "le dossier caché cache le fond");
}

#[test]
fn the_color_animates_with_the_opacity_keys() {
    let mut app = banc("cles");
    app.doc.ajouter_couleur_unie(0, [0, 0, 0], None, None).unwrap();
    app.layers_mut()[1].set_opacity_keys(&[(0, 1.0), (10, 0.0)]);
    assert_eq!(pixel(&app.doc, 0, 5, 5), Color32::from_rgb(0, 0, 0));
    let m = pixel(&app.doc, 5, 5, 5);
    assert!((m.r() as i32 - 127).abs() <= 2, "à mi-chemin : gris moyen sur blanc : {m:?}");
    assert_eq!(pixel(&app.doc, 10, 5, 5), Color32::from_rgb(255, 255, 255));
}

#[test]
fn drawing_keeps_working_and_the_eraser_removes_the_drawing_not_the_color() {
    let mut app = banc("gomme");
    app.run_action("calque.nouveau_couleur_unie");
    app.layers_mut()[1].frames = vec![Some(app.doc.new_cel(LayerKind::Bitmap)); 12];
    assert!(app.layers()[1].drawable(), "on peut dessiner sur le calque");
    carre(&mut app, 1, 0, ROUGE);
    assert_eq!(pixel(&app.doc, 0, 20, 20), ROUGE, "le dessin est par-dessus le fond");
    assert_eq!(pixel(&app.doc, 0, 100, 100), Color32::from_rgb(128, 128, 128));
    // Gomme : le dessin part, le fond reste.
    carre(&mut app, 1, 0, Color32::TRANSPARENT);
    assert_eq!(pixel(&app.doc, 0, 20, 20), Color32::from_rgb(128, 128, 128), "gommé jusqu'au fond uni, pas jusqu'au papier");
}

#[test]
fn changing_the_color_repaints_the_background_and_keeps_the_drawing() {
    let mut app = banc("couleur");
    app.run_action("calque.nouveau_couleur_unie");
    app.layers_mut()[1].frames = vec![Some(app.doc.new_cel(LayerKind::Bitmap)); 12];
    carre(&mut app, 1, 3, ROUGE);
    let avant = app.history.names().0.len();
    app.record_step();
    app.layers_mut()[1].unie = Some([20, 160, 90]);
    assert_eq!(pixel(&app.doc, 3, 100, 100), Color32::from_rgb(20, 160, 90), "le fond a changé de couleur");
    assert_eq!(pixel(&app.doc, 3, 20, 20), ROUGE, "le dessin n'a pas bougé");
    assert_eq!(app.history.names().0.len(), avant + 1);
    app.undo();
    assert_eq!(pixel(&app.doc, 3, 100, 100), Color32::from_rgb(128, 128, 128));
    assert_eq!(pixel(&app.doc, 3, 20, 20), ROUGE);
}

#[test]
fn the_card_is_light_no_bitmap_of_the_sheet_and_one_pixel_per_color() {
    let mut app = banc("leger");
    app.doc.ajouter_couleur_unie(0, [10, 20, 30], None, None).unwrap();
    let plan = crate::render::panel_plan_at(&app.doc, 0, 0, None);
    let item = plan.items.iter().find(|i| matches!(i.fond, Some(crate::render::FondPart::Unie(_)))).expect("l'élément de la couleur unie");
    assert!(!app.doc.cels.contains_key(&item.cel), "aucun dessin du projet derrière");
    let cel = crate::fx::shown(&app.doc, item).expect("son dessin");
    assert_eq!((cel.raster.width, cel.raster.height), (1, 1), "un pixel, étiré sur la feuille");
    assert!(std::sync::Arc::ptr_eq(&crate::fx::unie_cel([10, 20, 30]), &crate::fx::unie_cel([10, 20, 30])), "gardé par couleur");
    // Le projet n'a que la couleur : pas de dessin de plus.
    assert_eq!(app.doc.cels.len(), 1);
}

#[test]
fn thumbnails_the_full_image_and_the_world_tiles_show_the_card() {
    let mut app = banc("vues");
    app.doc.ajouter_couleur_unie(0, [30, 60, 90], None, Some(0)).unwrap();
    let plan = crate::render::panel_plan_at(&app.doc, 0, 0, Some(BLANC));
    let full = crate::render::composite_full(&app.doc, &plan);
    assert_eq!(full[299 * 400 + 399], Color32::from_rgb(30, 60, 90));
    assert_eq!(full[0], Color32::from_rgb(30, 60, 90));
    let cam = app.doc.default_camera();
    let v = crate::render::thumbnail(&app.doc, &plan, &cam, 100, 75);
    assert_eq!(v.pixels[40 * 100 + 50], Color32::from_rgb(30, 60, 90), "la vignette");
    // Une tuile du monde sur la feuille (le chemin de l'écran) : même couleur.
    let mut buf = vec![Color32::TRANSPARENT; 64 * 64];
    crate::render::composite_world_tile(&app.doc, &plan, 1, 1, &mut buf, 64, (64, 64));
    assert_eq!(buf[10 * 64 + 10], Color32::from_rgb(30, 60, 90));
}

#[test]
fn the_project_keeps_only_the_color_and_the_drawing_and_old_projects_open() {
    let mut app = banc("fichier");
    app.run_action("calque.nouveau_couleur_unie");
    app.layers_mut()[1].frames = vec![Some(app.doc.new_cel(LayerKind::Bitmap)); 12];
    app.layers_mut()[1].unie = Some([200, 100, 50]);
    app.layers_mut()[1].set_opacity_keys(&[(0, 0.5), (11, 1.0)]);
    carre(&mut app, 1, 0, ROUGE);
    let dir = std::env::temp_dir().join(format!("shot-unie-fichier-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("U.shot");
    crate::project::save(&app.doc, &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    let (a, b) = (&app.doc.panels[0].layers[1], &relu.doc.panels[0].layers[1]);
    assert_eq!((b.unie, b.name.as_str(), b.kind), (Some([200, 100, 50]), "Couleur unie", LayerKind::Bitmap));
    assert_eq!(b.opacity_keys, a.opacity_keys);
    for f in [0, 5, 11] {
        for (x, y) in [(5, 5), (20, 20), (399, 299)] {
            assert_eq!(pixel(&relu.doc, f, x, y), pixel(&app.doc, f, x, y), "image {f}, ({x}, {y})");
        }
    }
    assert_eq!(pixel(&relu.doc, 11, 20, 20), ROUGE, "le dessin est revenu (image 11 : calque opaque)");
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert_eq!(texte.matches("couleur_unie").count(), 1, "la couleur est écrite une fois, pour ce calque seul");
    // Enregistrer par-dessus ne change rien.
    crate::project::save(&relu.doc, &manifeste, &mut relu.written.clone()).unwrap();
    let encore = crate::project::load(&manifeste).unwrap();
    assert_eq!(encore.doc.panels[0].layers[1].unie, Some([200, 100, 50]));
    std::fs::remove_dir_all(&dir).ok();
    // (43 : le format des couleurs unies ; 44 : dégradés à pastilles, partie 80.)
    assert!(crate::project::VERSION >= 43);
}

#[test]
fn copies_keep_the_card_and_it_never_merges_into_another_layer() {
    let mut app = banc("copies");
    app.doc.ajouter_couleur_unie(0, [1, 2, 3], None, Some(0)).unwrap();
    let copie = crate::layerops::duplicate_layer(&mut app.doc, 0, 0);
    assert_eq!(app.doc.panels[0].layers[copie].unie, Some([1, 2, 3]), "la copie a le même fond");
    app.cur_layer = 1;
    assert!(app.doc.merge_layer_down(0, 1).is_err(), "un fond uni ne se fusionne pas");
    assert!(crate::document::layers_same(&app.doc.panels[0].layers[0], &app.doc.panels[0].layers[0].clone()));
    let mut autre = app.doc.panels[0].layers[0].clone();
    autre.unie = Some([9, 9, 9]);
    assert!(!crate::document::layers_same(&app.doc.panels[0].layers[0], &autre), "deux fonds différents ne se réunissent pas");
}

#[test]
fn the_kind_icon_the_menu_and_the_theme_color_know_the_card() {
    use crate::calquemenu::Sorte;
    let l = Layer::new_unie("x", [0, 0, 0]);
    assert_eq!(Sorte::of(&l), Sorte::Unie);
    assert_eq!(Sorte::Unie.nom(), "Couleur unie");
    assert_eq!(Sorte::Unie.action(), Some("calque.nouveau_couleur_unie"));
    assert!(Sorte::actions_du_menu().contains(&"calque.nouveau_couleur_unie"));
    assert_eq!(crate::theme::couleur_du_calque(&l), crate::theme::palette().dessin_unie);
    assert_ne!(crate::theme::palette().dessin_unie, crate::theme::palette().dessin_couleur);
}
