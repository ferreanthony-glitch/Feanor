//! Retour de terrain d'Anthony (2026-10-24, branche `retours-terrain`) : coller un calque dans un autre plan (Ctrl+C / Ctrl+V depuis le
//! panneau Calques ; un calque de couleur sans son trait ; un dossier ; les sources d'un calque libre).

use eframe::egui::Color32;

use crate::app::App;
use crate::calqueffet::CalqueEffet;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::griffonnage_tests::{app, put, ring};

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-retours-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}


// ------------------------------------------------------------------------------------ 2. coller un calque dans un autre plan

/// Deux plans. Le premier (12 images) : « Trait couleur » (calque de couleur, courant) sous « Trait » (bitmap, deux clés), « Vect »
/// (vectoriel) et « Effet » (calque d'effet). Le second est vide.
fn deux_plans() -> App {
    let mut app = app("retours-collage");
    let mut doc = Document::new(400, 300, 24);
    let a = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 60.0, 0.0)));
    let b = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 40.0, 0.0)));
    let v = doc.new_cel(LayerKind::Vector);
    let mut trait_ = Layer::new("Trait", LayerKind::Bitmap);
    trait_.frames = [vec![Some(a); 6], vec![Some(b); 6]].concat();
    let mut vect = Layer::new("Vect", LayerKind::Vector);
    vect.frames = vec![Some(v); 12];
    let effet = Layer::new_effect("Effet", CalqueEffet::ajustement(""));
    doc.panels[0].layers = vec![trait_, vect, effet];
    doc.panels[0].length = 12;
    doc.add_scene_after(0);
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.run_action("calque.nouveau_couleur");
    // Un peu de couleur dans le dessin de couleur de la première clé.
    let cel = app.layers()[0].frames[0].expect("dessin de couleur");
    let c = app.doc.cels.get_mut(&cel).expect("dessin");
    put(&mut c.raster, 10, 10, Color32::from_rgb(255, 0, 0));
    c.version += 1;
    app
}

/// Les dessins cités par les images de tous les calques du plan `p`.
fn dessins_du_plan(doc: &Document, p: usize) -> std::collections::BTreeSet<CelId> {
    doc.panels[p].layers.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect()
}

/// Ce qui pend dans les calques du plan `p` : un calque de couleur dont le trait n'est plus là, une source qui n'existe pas.
fn liens_perdus(doc: &Document, p: usize) -> Vec<String> {
    let layers = &doc.panels[p].layers;
    let mut perdus = Vec::new();
    for l in layers {
        let Some(c) = &l.couleur else { continue };
        if c.trait_id != 0 && !layers.iter().any(|t| t.id == c.trait_id) {
            perdus.push(format!("« {} » colore le calque {}, qui n'est pas là", l.name, c.trait_id));
        }
        for s in &c.sources {
            if !layers.iter().any(|t| t.id == *s) {
                perdus.push(format!("« {} » lit le calque {s}, qui n'est pas là", l.name));
            }
        }
        for (trait_cel, couleur_cel) in &c.dessins {
            if !doc.cels.contains_key(trait_cel) || !doc.cels.contains_key(couleur_cel) {
                perdus.push(format!("« {} » cite un dessin qui n'existe pas", l.name));
            }
        }
    }
    perdus
}

fn copier_dans_l_autre_plan(app: &mut App, li: usize) -> (usize, u64) {
    app.select_panel(0);
    app.select_layer(li);
    app.copy_layer();
    app.select_panel(1);
    (app.layers().len(), app.history.serial())
}

#[test]
fn a_bitmap_a_vector_and_an_effect_layer_paste_into_another_plan_with_their_own_drawings_in_one_undo_step() {
    for (nom, li) in [("Trait", 1), ("Vect", 2), ("Effet", 3)] {
        let mut app = deux_plans();
        let source = dessins_du_plan(&app.doc, 0);
        let origine = app.layers()[li].clone();
        let (n, serial) = copier_dans_l_autre_plan(&mut app, li);
        app.paste_layer();
        assert_eq!(app.history.serial(), serial + 1, "{nom} : une étape d'annulation");
        let collee = app.layers().iter().find(|l| l.name == nom).unwrap_or_else(|| panic!("{nom} collé")).clone();
        assert!(app.layers().len() > n, "{nom} : un calque de plus");
        assert_ne!(collee.id, origine.id);
        assert_eq!(collee.kind, origine.kind);
        assert_eq!(collee.effet, origine.effet, "{nom} : son réglage d'effet");
        assert_eq!(collee.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len(), origine.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len(), "{nom} : les mêmes clés");
        for c in collee.frames.iter().flatten() {
            assert!(app.doc.cels.contains_key(c), "{nom} : son dessin existe");
            assert!(!source.contains(c), "{nom} : aucun dessin n'est partagé avec le plan d'où il vient");
        }
        assert_eq!(liens_perdus(&app.doc, 1), Vec::<String>::new());
        app.undo();
        assert_eq!(app.layers().len(), n, "{nom} : une annulation retire le calque collé");
    }
}

#[test]
fn copying_a_colour_layer_brings_its_line_and_the_pair_pastes_linked_into_another_plan() {
    let mut app = deux_plans();
    let source = dessins_du_plan(&app.doc, 0);
    let cel_couleur = app.layers()[0].frames[0].expect("couleur");
    let (n, serial) = copier_dans_l_autre_plan(&mut app, 0);
    app.paste_layer();
    assert_eq!(app.history.serial(), serial + 1, "une seule étape d'annulation");
    assert_eq!(app.layers().len(), n + 2, "le trait vient avec sa couleur : {:?}", app.layers().iter().map(|l| l.name.clone()).collect::<Vec<_>>());
    let layers = app.layers().to_vec();
    let ci = layers.iter().position(|l| l.couleur.is_some()).expect("le calque de couleur est resté un calque de couleur");
    let ti = crate::calquecouleur::trait_de(&layers, ci).expect("il colore un trait de ce plan");
    assert_eq!(layers[ti].name, "Trait");
    assert_ne!(layers[ti].id, app.doc.panels[0].layers.iter().find(|l| l.name == "Trait").unwrap().id, "le trait collé est une copie");
    assert_eq!(liens_perdus(&app.doc, 1), Vec::<String>::new());
    // Des dessins à eux, avec la couleur.
    for c in layers[ci].frames.iter().chain(layers[ti].frames.iter()).flatten() {
        assert!(!source.contains(c), "dessin partagé avec le plan d'origine");
    }
    let colle = layers[ci].frames[0].expect("couleur collée");
    assert_ne!(colle, cel_couleur);
    assert_eq!(app.doc.cels[&colle].raster.get(10, 10), Color32::from_rgb(255, 0, 0), "la couleur est venue");
    // Enregistré, rouvert : rien ne pend.
    let dir = dossier("collage-couleur");
    let manifest = dir.join("P.shot");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifest).expect("réouverture");
    assert_eq!(liens_perdus(&relu.doc, 1), Vec::<String>::new());
    let layers = &relu.doc.panels[1].layers;
    let ci = layers.iter().position(|l| l.couleur.is_some()).expect("calque de couleur rouvert");
    assert!(crate::calquecouleur::trait_de(layers, ci).is_some());
    assert_eq!(relu.doc.cels[&layers[ci].frames[0].expect("couleur")].raster.get(10, 10), Color32::from_rgb(255, 0, 0));
    std::fs::remove_dir_all(&dir).ok();
    // Une annulation retire les deux.
    app.undo();
    assert_eq!(app.layers().len(), n);
}

#[test]
fn ctrl_c_and_ctrl_v_on_the_layers_panel_copy_and_paste_a_layer_into_another_plan() {
    let mut app = deux_plans();
    app.select_panel(0);
    app.select_layer(2);
    // Un clic dans le panneau Calques lui donne la main (`layers_panel`).
    app.layers_pane_focus = true;
    app.run_action("edition.copier");
    assert!(app.layer_clipboard.is_some(), "Ctrl+C copie le calque choisi : {}", app.status);
    // Un autre plan, puis un clic dans le panneau Calques.
    app.select_panel(1);
    app.layers_pane_focus = true;
    let (n, serial) = (app.layers().len(), app.history.serial());
    app.run_action("edition.coller");
    assert_eq!(app.layers().len(), n + 1, "Ctrl+V colle le calque : {}", app.status);
    assert_eq!(app.history.serial(), serial + 1);
    assert_eq!(app.layer().name, "Vect");
    // Ailleurs que dans le panneau Calques, Ctrl+V garde son sens d'avant : rien à coller ici.
    app.layers_pane_focus = false;
    let n = app.layers().len();
    app.run_action("edition.coller");
    assert_eq!(app.layers().len(), n, "hors du panneau Calques, Ctrl+V ne colle pas de calque");
}

#[test]
fn a_folder_pastes_into_another_plan_with_its_line_and_the_colour_still_linked() {
    let mut app = app("retours-dossier");
    let mut doc = Document::new(400, 300, 24);
    let a = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 60.0, 0.0)));
    let mut line = Layer::new("Trait", LayerKind::Bitmap);
    line.frames = vec![Some(a); 12];
    doc.panels[0].layers = vec![line];
    doc.panels[0].length = 12;
    doc.add_scene_after(0);
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    let dossier_layer = Layer::new_folder("Dossier");
    let fid = dossier_layer.id;
    app.layers_mut().push(dossier_layer);
    app.layers_mut()[0].parent = Some(fid);
    crate::folders::normalize(app.layers_mut());
    app.cur_layer = app.layers().iter().position(|l| l.name == "Trait").unwrap();
    app.run_action("calque.nouveau_couleur");
    let fi = app.layers().iter().position(|l| l.folder).expect("le dossier");
    let (n, _) = copier_dans_l_autre_plan(&mut app, fi);
    app.paste_layer();
    assert_eq!(app.layers().len(), n + 3, "le dossier, son trait et sa couleur : {:?}", app.layers().iter().map(|l| l.name.clone()).collect::<Vec<_>>());
    let layers = app.layers().to_vec();
    let ci = layers.iter().position(|l| l.couleur.is_some()).expect("la couleur est restée un calque de couleur");
    let ti = crate::calquecouleur::trait_de(&layers, ci).expect("elle colore le trait collé, pas celui de l'autre plan");
    assert_eq!(layers[ti].name, "Trait");
    assert_eq!(liens_perdus(&app.doc, 1), Vec::<String>::new());
}

#[test]
fn a_free_colour_layer_pasted_into_another_plan_forgets_the_sources_it_does_not_find_there() {
    let mut app = deux_plans();
    let ligne = app.layers()[1].id;
    if let Some(c) = app.layers_mut()[0].couleur.as_mut() {
        c.trait_id = 0;
        c.sources = vec![ligne];
    }
    let (_, _) = copier_dans_l_autre_plan(&mut app, 0);
    app.paste_layer();
    assert!(app.layers().iter().any(|l| l.couleur.is_some()), "le calque libre est resté un calque de couleur");
    assert_eq!(liens_perdus(&app.doc, 1), Vec::<String>::new());
}

