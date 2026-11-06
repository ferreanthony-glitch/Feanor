//! Coller une image : objet vectoriel sur un calque vectoriel, pixels sur un calque bitmap (`imagevec.rs`, `pasteimage.rs`) ; copier-coller
//! de calques depuis le panneau Calques (`app.rs`, `layerops.rs`). Le presse-papiers de Windows n'est jamais lu : l'image collée est simulée.

use std::collections::HashSet;

use eframe::egui::{pos2, Color32};

use crate::app::App;
use crate::blend::BlendMode;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::griffonnage_tests::app;
use crate::vector::{Affine, Element};

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-imagevec-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Une image unie `w` × `h` de la couleur `rgb` (opaque), RVBA.
fn unie(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
    (0..w * h).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
}

/// Un projet de deux plans de 12 images : le premier a un calque vectoriel « Vect » (courant) sous un calque bitmap « Bitmap » ;
/// le second est vide. Les deux calques ont une image à la première clé.
fn deux_plans(nom: &str) -> App {
    let mut app = app(nom);
    let mut doc = Document::new(400, 300, 24);
    let b = doc.new_cel(LayerKind::Bitmap);
    let v = doc.new_cel(LayerKind::Vector);
    let mut vect = Layer::new("Vect", LayerKind::Vector);
    vect.frames = vec![Some(v); 12];
    let mut bitmap = Layer::new("Bitmap", LayerKind::Bitmap);
    bitmap.frames = vec![Some(b); 12];
    doc.panels[0].layers = vec![vect, bitmap];
    doc.panels[0].length = 12;
    doc.add_scene_after(0);
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

fn elements(app: &App, li: usize) -> Vec<Element> {
    let cel = app.layers()[li].frames[0].expect("dessin");
    app.doc.cels[&cel].vector.as_ref().map(|v| v.elements.clone()).unwrap_or_default()
}

fn pixel(app: &App, li: usize, x: usize, y: usize) -> Color32 {
    let cel = app.layers()[li].frames[0].expect("dessin");
    app.doc.cels[&cel].raster.get(x, y)
}

// ------------------------------------------------------------------------------------ coller une image

#[test]
fn an_image_pasted_on_a_vector_layer_becomes_a_selected_vector_object_without_a_new_layer() {
    let mut app = deux_plans("objet");
    let (n, serial) = (app.layers().len(), app.history.serial());
    app.paste_clipboard_image((40, 30, unie(40, 30, [255, 0, 0])));
    assert_eq!(app.layers().len(), n, "aucun calque n'est créé : {}", app.status);
    assert_eq!(app.history.serial(), serial + 1, "une seule étape d'annulation");
    let els = elements(&app, 0);
    assert_eq!(els.len(), 1);
    let Element::Image { size, .. } = &els[0] else { panic!("un objet image, pas {:?}", els[0]) };
    assert_eq!(*size, [40, 30]);
    // Sélectionné juste après le collage.
    let sel = app.vsel.as_ref().expect("sélectionné");
    assert_eq!(sel.items, vec![0]);
    // Rendu dans le dessin : rouge au centre de l'objet, rien à côté.
    let b = els[0].bounds();
    let (cx, cy) = (b.center().x as usize, b.center().y as usize);
    assert_eq!(pixel(&app, 0, cx, cy), Color32::from_rgb(255, 0, 0));
    assert_eq!(pixel(&app, 0, 1, 1), Color32::TRANSPARENT);
    // Une annulation retire l'objet.
    app.undo();
    assert!(elements(&app, 0).is_empty());
    assert_eq!(pixel(&app, 0, cx, cy), Color32::TRANSPARENT);
}

#[test]
fn the_image_object_moves_scales_rotates_hits_and_is_deleted_like_other_objects() {
    let mut app = deux_plans("transformer");
    app.paste_clipboard_image((40, 30, unie(40, 30, [0, 0, 255])));
    let e = elements(&app, 0).remove(0);
    let b = e.bounds();
    assert!(e.hit(b.center(), 1.0), "se prend dans son rectangle");
    assert!(!e.hit(pos2(b.min.x - 20.0, b.min.y - 20.0), 1.0));
    // Déplacé de (50, 20) : sa boîte suit.
    let moved = e.transformed(&Affine::translate(eframe::egui::vec2(50.0, 20.0)));
    assert!((moved.bounds().min.x - (b.min.x + 50.0)).abs() < 1e-3 && (moved.bounds().min.y - (b.min.y + 20.0)).abs() < 1e-3);
    // Agrandi deux fois autour de son centre, puis tourné d'un quart de tour : une boîte plus grande, un rectangle qui tourne.
    let big = e.transformed(&Affine::scale_about(b.center(), 2.0, 2.0));
    assert!((big.bounds().width() - 2.0 * b.width()).abs() < 1e-2);
    let tourne = e.transformed(&Affine::rotate_about(b.center(), std::f32::consts::FRAC_PI_2));
    assert!((tourne.bounds().width() - b.height()).abs() < 1e-2 && (tourne.bounds().height() - b.width()).abs() < 1e-2);
    // Les outils : transformer la sélection pose l'objet transformé ; supprimer l'enlève.
    let cel = app.layers()[0].frames[0].expect("dessin");
    app.doc.cels.get_mut(&cel).expect("dessin").vector.as_mut().expect("vectoriel").elements[0] = moved.clone();
    app.refresh_cel(cel);
    let c = moved.bounds().center();
    assert_eq!(pixel(&app, 0, c.x as usize, c.y as usize), Color32::from_rgb(0, 0, 255), "rendu à sa nouvelle place");
    assert_eq!(pixel(&app, 0, b.center().x as usize - 15, b.center().y as usize - 10), Color32::TRANSPARENT, "plus à l'ancienne");
    app.vsel = Some(crate::vtools::VSelection { cel, items: vec![0] });
    app.delete_selection();
    assert!(elements(&app, 0).is_empty(), "supprimé");
}

#[test]
fn an_image_pasted_on_a_bitmap_layer_stays_pixels() {
    let mut app = deux_plans("bitmap");
    app.select_layer(1);
    let (n, serial) = (app.layers().len(), app.history.serial());
    app.paste_clipboard_image((20, 20, unie(20, 20, [0, 255, 0])));
    assert_eq!(app.layers().len(), n, "dans le calque bitmap courant : pas de calque neuf : {}", app.status);
    assert!(app.bsel.is_some(), "une zone flottante à placer");
    assert!(elements(&app, 1).is_empty(), "aucun objet vectoriel");
    app.drop_bitmap_selection();
    assert!(app.history.serial() > serial);
    let b = app.layers()[1].frames[0].expect("dessin");
    let non_vides = app.doc.cels[&b].raster.tiles.iter().flatten().count();
    assert!(non_vides > 0, "des pixels dans le dessin bitmap");
    assert!(app.doc.cels[&b].vector.is_none());
}

#[test]
fn a_big_image_is_prepared_in_the_background_then_dropped_on_the_layer_where_it_was_pasted() {
    let mut app = deux_plans("grande");
    let (w, h) = (1200, 1000);
    assert!(w * h > crate::imagevec::GRANDE);
    app.paste_clipboard_image((w, h, unie(w, h, [200, 100, 0])));
    assert!(app.collage.images.is_some(), "préparation sur un fil de fond");
    assert!(app.collage_occupe(), "on ne change pas d'onglet pendant ce temps");
    assert!(elements(&app, 0).is_empty(), "rien n'est posé tout de suite");
    // L'utilisateur change de calque : l'image va là où le collage a été demandé.
    app.select_layer(1);
    let ctx = eframe::egui::Context::default();
    for _ in 0..400 {
        app.tick_collage(&ctx);
        if app.collage.images.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(app.collage.images.is_none(), "préparée : {}", app.status);
    assert_eq!(app.cur_layer, 0);
    assert_eq!(elements(&app, 0).len(), 1);
    assert_eq!(app.layers().len(), 2);
}

#[test]
fn the_same_image_pasted_twice_is_kept_once_in_the_file_and_survives_a_round_trip() {
    let mut app = deux_plans("fichier");
    let rgba = unie(30, 20, [10, 200, 30]);
    app.paste_clipboard_image((30, 20, rgba.clone()));
    app.paste_clipboard_image((30, 20, rgba));
    let els = elements(&app, 0);
    assert_eq!(els.len(), 2);
    let keys: HashSet<u64> = els.iter().filter_map(|e| if let Element::Image { key, .. } = e { Some(*key) } else { None }).collect();
    assert_eq!(keys.len(), 1, "le même contenu a la même empreinte");
    let key = *keys.iter().next().expect("empreinte");
    let dir = dossier("aller-retour");
    let manifest = dir.join("P.shot");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).expect("enregistrement");
    let png = crate::imagevec::fichier(&dir, key);
    assert!(png.exists(), "images/<empreinte>.png écrit");
    assert_eq!(std::fs::read_dir(dir.join("images")).expect("dossier").count(), 1, "une seule fois");
    // Rouvert, sans rien dans le magasin : les pixels viennent du fichier.
    crate::imagevec::oublier(key);
    assert!(crate::imagevec::obtenir(key).is_none());
    let relu = crate::project::load(&manifest).expect("réouverture");
    let relus: Vec<Element> = {
        let cel = relu.doc.panels[0].layers[0].frames[0].expect("dessin");
        relu.doc.cels[&cel].vector.as_ref().expect("vectoriel").elements.clone()
    };
    assert_eq!(relus, els, "les objets reviennent à l'identique");
    let px = crate::imagevec::obtenir(key).expect("pixels relus");
    assert_eq!((px.w, px.h), (30, 20));
    assert_eq!(&px.rgba[..4], &[10, 200, 30, 255]);
    // Et rendus : le dessin relu montre l'image.
    let c = els[0].bounds().center();
    let cel = relu.doc.panels[0].layers[0].frames[0].expect("dessin");
    assert_eq!(relu.doc.cels[&cel].raster.get(c.x as usize, c.y as usize), Color32::from_rgb(10, 200, 30));
    // Plus aucun objet ne la cite : le fichier part au prochain enregistrement.
    let cel: CelId = app.layers()[0].frames[0].expect("dessin");
    app.doc.cels.get_mut(&cel).expect("dessin").vector.as_mut().expect("vectoriel").elements.clear();
    app.doc.cels.get_mut(&cel).expect("dessin").version += 1;
    crate::project::save(&app.doc, &manifest, &mut Default::default()).expect("second enregistrement");
    assert!(!png.exists(), "l'image inutile est effacée du dossier");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_project_format_is_42_for_image_objects_and_an_old_file_still_reads() {
    assert!(crate::project::VERSION >= 42, "le format des objets image (42) est toujours lu ; le suivant (43) ajoute les clés d'opacité");
    // Un dessin vectoriel d'avant (traits, zones) se lit toujours : le nouvel élément ne change rien aux autres.
    let ancien = r#"{"elements":[{"type":"Fill","contours":[[[0.0,0.0],[10.0,0.0],[10.0,10.0]]],"color":[1,2,3],"opacity":1.0}]}"#;
    let d: crate::vector::VectorDrawing = serde_json::from_str(ancien).expect("ancien dessin");
    assert_eq!(d.elements.len(), 1);
    // Et l'objet image s'écrit avec son empreinte, sa taille et son placement.
    let e = Element::image(0xABCDEF, [4, 3], Affine::IDENTITY);
    let json = serde_json::to_string(&e).expect("json");
    assert!(json.contains("\"type\":\"Image\"") && json.contains("\"image\"") && json.contains("\"taille\""), "{json}");
    assert_eq!(serde_json::from_str::<Element>(&json).expect("relu"), e);
}

#[test]
fn an_image_object_is_not_picked_by_colour_and_keeps_its_opacity_when_recoloured() {
    let mut app = deux_plans("couleur");
    app.paste_clipboard_image((10, 10, unie(10, 10, [5, 5, 5])));
    let cel = app.layers()[0].frames[0].expect("dessin");
    let d = app.doc.cels[&cel].vector.clone().expect("vectoriel");
    assert_eq!(crate::colorsel::same_color(&d, 0), vec![0], "seule");
    let mut e = d.elements[0].clone();
    e.set_color([200, 0, 0]);
    assert_eq!(e, d.elements[0], "recolorier ne change pas une image");
}

// ------------------------------------------------------------------------------------ copier-coller des calques

#[test]
fn a_layer_copied_from_the_layers_panel_pastes_into_another_plan_in_one_undo_step_with_all_it_has() {
    let mut app = deux_plans("calque");
    // Un calque vectoriel réglé : mode de fusion, opacité, un trait.
    app.paste_clipboard_image((10, 10, unie(10, 10, [9, 9, 9])));
    app.layers_mut()[0].blend = BlendMode::Multiply;
    app.layers_mut()[0].opacity = 0.4;
    app.layers_mut()[0].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur));
    let source = app.layers()[0].clone();
    // Ctrl+C dans le panneau Calques.
    app.layers_pane_focus = true;
    app.run_action("edition.copier");
    assert!(app.layer_clipboard.is_some(), "{}", app.status);
    app.select_panel(1);
    app.layers_pane_focus = true;
    let (n, serial) = (app.layers().len(), app.history.serial());
    app.run_action("edition.coller");
    assert_eq!(app.history.serial(), serial + 1, "une seule étape d'annulation");
    assert_eq!(app.layers().len(), n + 1);
    let collee = app.layer().clone();
    assert_eq!(collee.name, "Vect", "aucun calque de ce nom dans ce plan");
    assert_eq!((collee.blend, collee.opacity, collee.kind), (BlendMode::Multiply, 0.4, LayerKind::Vector));
    assert_eq!(collee.fx.len(), 1, "ses effets");
    assert_ne!(collee.id, source.id);
    let cel = collee.frames[0].expect("dessin collé");
    assert_ne!(Some(cel), source.frames[0], "son propre dessin");
    assert_eq!(app.doc.cels[&cel].vector.as_ref().expect("vectoriel").elements.len(), 1, "son image est là");
    app.undo();
    assert_eq!(app.layers().len(), n);
}

#[test]
fn a_pasted_layer_with_a_name_already_taken_becomes_name_2_then_3() {
    let mut app = deux_plans("noms");
    app.layers_pane_focus = true;
    app.run_action("edition.copier");
    app.run_action("edition.coller");
    assert_eq!(app.layer().name, "Vect (2)");
    app.run_action("edition.coller");
    assert_eq!(app.layer().name, "Vect (3)");
    // La copie de « Vect (2) » n'est pas « Vect (2) (2) ».
    let mut pris: HashSet<String> = ["Vect".to_string(), "Vect (2)".to_string()].into();
    assert_eq!(crate::layerops::nom_libre(&pris, "Vect (2)"), "Vect (3)");
    pris.insert("Vect (3)".into());
    assert_eq!(crate::layerops::nom_libre(&pris, "Autre"), "Autre");
    assert_eq!(crate::layerops::nom_libre(&pris, "Vect"), "Vect (4)");
}

#[test]
fn several_chosen_layers_are_copied_and_pasted_together_in_order_in_one_step() {
    let mut app = deux_plans("plusieurs");
    // Deux calques choisis : « Vect » (courant) et « Bitmap » (Ctrl + clic).
    app.layers_pane_focus = true;
    app.click_layer_head(1, true, false);
    assert_eq!(app.chosen_layers(), vec![0, 1]);
    app.run_action("edition.copier");
    let clip = app.layer_clipboard.as_ref().expect("copie");
    assert_eq!(clip.extras.len(), 1, "les deux calques sont copiés");
    app.select_panel(1);
    app.layers_pane_focus = true;
    let (n, serial) = (app.layers().len(), app.history.serial());
    app.run_action("edition.coller");
    assert_eq!(app.history.serial(), serial + 1, "une seule étape d'annulation pour tous");
    assert_eq!(app.layers().len(), n + 2);
    let noms: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    let (v, b) = (noms.iter().position(|n| *n == "Vect").expect("Vect"), noms.iter().position(|n| *n == "Bitmap").expect("Bitmap"));
    assert!(v < b, "même ordre que dans le plan d'origine : {noms:?}");
    app.undo();
    assert_eq!(app.layers().len(), n);
}

#[test]
fn a_character_layer_keeps_its_link_inside_the_same_project_and_becomes_ordinary_in_another() {
    let mut app = deux_plans("personnage");
    app.doc.personnages.push(crate::personnages::Personnage { id: 77, name: "Perso".into(), line: [0, 0, 0], fill: None, brosse: None });
    app.layers_mut()[0].personnage = Some(77);
    app.layers_pane_focus = true;
    app.run_action("edition.copier");
    // Dans le même projet (un autre plan) : le lien est gardé.
    app.select_panel(1);
    app.layers_pane_focus = true;
    app.run_action("edition.coller");
    assert_eq!(app.layer().personnage, Some(77), "le lien au personnage est gardé");
    // Venue d'un autre projet : un calque ordinaire.
    app.layer_clipboard.as_mut().expect("copie").projet.0 += 1;
    app.layers_pane_focus = true;
    app.run_action("edition.coller");
    assert_eq!(app.layer().personnage, None, "dans un autre projet, il devient ordinaire");
    // Le personnage supprimé entre-temps : pas de lien qui pend.
    app.layer_clipboard.as_mut().expect("copie").projet.0 -= 1;
    app.doc.personnages.clear();
    app.layers_pane_focus = true;
    app.run_action("edition.coller");
    assert_eq!(app.layer().personnage, None);
}

#[test]
fn ctrl_c_and_ctrl_v_outside_the_layers_panel_keep_their_meaning() {
    let mut app = deux_plans("hors-panneau");
    app.paste_clipboard_image((10, 10, unie(10, 10, [1, 2, 3])));
    assert!(app.vsel.is_some());
    // Hors du panneau Calques : Ctrl+C copie la sélection du dessin, pas le calque.
    app.layers_pane_focus = false;
    app.run_action("edition.copier");
    assert!(app.layer_clipboard.is_none(), "aucun calque copié");
    assert_eq!(app.clipboard.len(), 1, "la sélection est copiée");
    // Ctrl+V colle l'élément (l'image) dans le dessin, pas un calque.
    let n = app.layers().len();
    app.run_action("edition.coller");
    assert_eq!(app.layers().len(), n);
    assert_eq!(elements(&app, 0).len(), 2, "l'objet image collé comme les autres éléments");
}

#[test]
fn the_layers_panel_menu_offers_copy_and_paste_and_stays_short() {
    // (La longueur du menu est tenue par `menus_tests` ; ici, les deux commandes existent bien.)
    for id in ["calque.copier", "calque.coller"] {
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == id), "{id}");
    }
    let _ = Cel::bitmap;
}
