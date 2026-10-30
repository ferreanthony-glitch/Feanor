//! Tests du geste Ctrl+Maj avec l'outil Sélection (« sélectionner
//! plusieurs images clés de plusieurs calques en même temps, en sélectionnant avec Ctrl+Maj + outil
//! de sélection sur le canevas ») : le lasso prend sur TOUS les calques du plan à l'image courante,
//! sauf les cachés, verrouillés, vides et vidéo ; les calques touchés deviennent les calques choisis
//! et leurs clés à l'image courante, des clés choisies du montage et de la timeline.

use eframe::egui::{self, pos2, Modifiers, Pos2};

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::multitools::is_all_layers_gesture;
use crate::vector::{Element, VPoint};

fn stroke(points: &[(f32, f32)]) -> Element {
    Element::Stroke {
        points: points.iter().map(|&(x, y)| VPoint { x, y, r: 2.0 }).collect(),
        color: [20, 20, 20],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

fn add_vector(doc: &mut Document, name: &str, strokes: Vec<Element>) -> usize {
    let cel = doc.new_cel(LayerKind::Vector);
    let c = doc.cels.get_mut(&cel).unwrap();
    c.vector.as_mut().unwrap().elements = strokes;
    c.rerender();
    let mut l = Layer::new(name, LayerKind::Vector);
    l.set(0, Some(cel));
    doc.panels[0].layers.push(l);
    doc.panels[0].layers.len() - 1
}

fn lasso(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Pos2> {
    vec![pos2(x0, y0), pos2(x1, y0), pos2(x1, y1), pos2(x0, y1)]
}

/// Un plan à six calques : trois utilisables (Encre, Couleur, Décor), un caché, un verrouillé, un vide.
fn app_with_layers(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-multiall-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers.clear();
    add_vector(&mut doc, "Encre", vec![stroke(&[(20.0, 20.0), (80.0, 30.0)])]);
    add_vector(&mut doc, "Couleur", vec![stroke(&[(30.0, 40.0), (90.0, 50.0)])]);
    add_vector(&mut doc, "Décor", vec![stroke(&[(40.0, 60.0), (100.0, 70.0)])]);
    let hidden = add_vector(&mut doc, "Caché", vec![stroke(&[(50.0, 50.0), (60.0, 60.0)])]);
    let locked = add_vector(&mut doc, "Verrouillé", vec![stroke(&[(50.0, 55.0), (60.0, 65.0)])]);
    let mut empty = Layer::new("Vide", LayerKind::Vector);
    empty.frames = vec![None];
    doc.panels[0].layers.push(empty);
    doc.panels[0].layers[hidden].visible = false;
    doc.panels[0].layers[locked].locked = true;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_frame = 0;
    app.cur_layer = 0;
    app.tool = Tool::Select;
    app
}

#[test]
fn only_ctrl_and_shift_together_mean_all_the_layers() {
    let none = Modifiers::NONE;
    assert!(!is_all_layers_gesture(&none));
    assert!(!is_all_layers_gesture(&Modifiers::SHIFT), "Maj seul : ajouter / retirer");
    assert!(!is_all_layers_gesture(&Modifiers::COMMAND), "Ctrl seul : le lasso temporaire, ou la boîte autour du pivot");
    assert!(!is_all_layers_gesture(&Modifiers::ALT), "Alt : la pipette");
    assert!(is_all_layers_gesture(&(Modifiers::COMMAND | Modifiers::SHIFT)));
    assert!(!is_all_layers_gesture(&(Modifiers::COMMAND | Modifiers::SHIFT | Modifiers::ALT)));
}

#[test]
fn the_all_layers_gesture_reaches_every_usable_layer_and_skips_the_others_with_a_reason() {
    let mut app = app_with_layers("cibles");
    // Sans Ctrl+Maj : seulement les calques choisis (ici le courant).
    assert_eq!(app.multi_targets().list.len(), 1);
    app.multi.all_layers = true;
    let t = app.multi_targets();
    let kept: Vec<&str> = t.list.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(kept, ["Encre", "Couleur", "Décor"], "visibles, déverrouillés, avec un dessin à cette image");
    let reasons: Vec<(&str, &str)> = t.skipped.iter().map(|(n, w)| (n.as_str(), *w)).collect();
    assert!(reasons.contains(&("Caché", "masqué")), "{reasons:?}");
    assert!(reasons.contains(&("Verrouillé", "verrouillé")), "{reasons:?}");
    assert!(reasons.contains(&("Vide", "sans dessin à cette image")), "{reasons:?}");
}

#[test]
fn a_lasso_with_ctrl_shift_picks_on_all_layers_and_chooses_them_and_their_keys() {
    let mut app = app_with_layers("lasso");
    let ids: Vec<u64> = app.doc.panels[0].layers.iter().map(|l| l.id).collect();
    app.multi.all_layers = true;
    app.multi_select_polygon(&lasso(0.0, 0.0, 200.0, 120.0), false);
    let sel = app.multi.sel.as_ref().expect("une sélection");
    let names: Vec<&str> = sel.parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Encre", "Couleur", "Décor"], "un morceau par calque utilisable, aucun sur le caché ni le verrouillé");
    assert!(!app.multi.all_layers, "le geste est fini");
    // Les calques touchés sont les calques choisis (le courant, Encre, reste le courant).
    assert_eq!(app.chosen_layers(), vec![0, 1, 2]);
    assert!(app.layer_sel.contains(&ids[1]) && app.layer_sel.contains(&ids[2]));
    assert!(!app.layer_sel.contains(&ids[3]) && !app.layer_sel.contains(&ids[4]), "ni le caché ni le verrouillé");
    // Leurs clés à l'image courante sont choisies dans le montage et la timeline.
    let (n, m) = crate::montagekeys::key_counts(&app.key_sel);
    assert_eq!((n, m), (3, 3));
    assert_eq!(crate::montagekeys::key_counts_text(&app.key_sel), "3 clés sur 3 calques");
    assert!(app.status.contains("3 clés sur 3 calques"), "{}", app.status);
    // La sélection reste valable : les calques choisis la contiennent, l'outil agit sur tous.
    assert!(app.multi_wanted());
    app.multi_sync();
    assert!(app.multi.sel.is_some(), "multi_sync ne l'efface pas");
}

#[test]
fn a_lasso_that_hits_only_some_layers_chooses_only_those() {
    let mut app = app_with_layers("partiel");
    let ids: Vec<u64> = app.doc.panels[0].layers.iter().map(|l| l.id).collect();
    app.multi.all_layers = true;
    // Un lasso étroit : seul le trait de « Décor » (y 60 à 70) est dedans.
    app.multi_select_polygon(&lasso(35.0, 55.0, 110.0, 75.0), false);
    let sel = app.multi.sel.as_ref().expect("une sélection");
    assert_eq!(sel.parts.len(), 1);
    assert_eq!(sel.parts[0].layer, ids[2]);
    assert!(app.layer_sel.contains(&ids[2]) && !app.layer_sel.contains(&ids[1]));
    assert_eq!(crate::montagekeys::key_counts(&app.key_sel), (1, 1));
}

#[test]
fn without_ctrl_shift_the_lasso_stays_on_the_chosen_layers() {
    let mut app = app_with_layers("choisis");
    assert!(!app.multi.all_layers);
    app.multi_select_polygon(&lasso(0.0, 0.0, 200.0, 120.0), false);
    let sel = app.multi.sel.as_ref().expect("une sélection");
    assert_eq!(sel.parts.len(), 1, "seulement le calque courant, Encre");
    assert!(app.key_sel.is_empty(), "les clés ne changent pas");
}

#[test]
fn an_empty_all_layers_lasso_leaves_the_layer_choice_alone() {
    let mut app = app_with_layers("vide");
    app.multi.all_layers = true;
    app.multi_select_polygon(&lasso(300.0, 200.0, 390.0, 290.0), false);
    assert!(app.multi.sel.is_none());
    assert!(app.layer_sel.is_empty() && app.key_sel.is_empty());
    assert!(app.status.contains("Rien"), "{}", app.status);
}
