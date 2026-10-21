//! Sélection sur plusieurs calques (`multisel.rs`) : le lasso prend sur chaque calque choisi, une
//! boîte entoure l'ensemble, un geste s'applique pareil à tous, en une seule étape d'annulation ;
//! les calques cachés ou verrouillés sont laissés de côté ; traits et pixels se mêlent.

use std::time::Instant;

use eframe::egui::{pos2, vec2, Color32, Pos2};

use crate::document::{CelId, Document, Layer, LayerKind, LayerTransform};
use crate::flottante::Flottante;
use crate::history::{Edit, History, Nav};
use crate::multisel::{self, MultiSel, PartKind};
use crate::raster::Raster;
use crate::selbox::{gesture, Handle, Mods};
use crate::vector::{Affine, Element, VPoint};

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

fn new_doc() -> Document {
    let mut d = Document::new(400, 300, 24);
    d.panels[0].layers.clear();
    d
}

/// Un calque vectoriel de plus au plan 0, avec ces traits à l'image 0 : son rang.
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

/// Un calque bitmap avec un rectangle plein rouge `(x, y, largeur, hauteur)` à l'image 0.
fn add_bitmap(doc: &mut Document, name: &str, rect: (usize, usize, usize, usize)) -> usize {
    let cel = doc.new_cel(LayerKind::Bitmap);
    let c = doc.cels.get_mut(&cel).unwrap();
    let (x0, y0, w, h) = rect;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let i = c.raster.tile_index_of(x, y);
            c.raster.tile_mut(i)[(y % 64) * 64 + x % 64] = Color32::from_rgb(200, 0, 0);
        }
    }
    let mut l = Layer::new(name, LayerKind::Bitmap);
    l.set(0, Some(cel));
    doc.panels[0].layers.push(l);
    doc.panels[0].layers.len() - 1
}

fn cel_of(doc: &Document, li: usize) -> CelId {
    doc.panels[0].layers[li].cel_at(0).expect("dessin")
}

fn points_of(doc: &Document, li: usize) -> Vec<Pos2> {
    let d = doc.cels[&cel_of(doc, li)].vector.as_ref().expect("vectoriel");
    d.elements
        .iter()
        .flat_map(|e| match e {
            Element::Stroke { points, .. } => points.iter().map(|p| pos2(p.x, p.y)).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn near(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 0.05
}

/// Un lasso carré de `(x0, y0)` à `(x1, y1)`.
fn lasso(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Pos2> {
    vec![pos2(x0, y0), pos2(x1, y0), pos2(x1, y1), pos2(x0, y1)]
}

fn select(doc: &Document, chosen: &[usize], poly: &[Pos2]) -> MultiSel {
    let t = multisel::targets(doc, 0, 0, chosen);
    MultiSel::new(0, 0, multisel::select_polygon(doc, &t, poly))
}

fn vector_items(sel: &MultiSel, k: usize) -> Vec<usize> {
    match &sel.parts[k].kind {
        PartKind::Vector { items } => items.clone(),
        PartKind::Bitmap { .. } => panic!("une zone, pas des traits"),
    }
}

/// Trois calques vectoriels, deux traits chacun : le premier tout en haut à gauche, le second
/// plus bas. Un lasso en haut prend le premier de chaque calque.
fn three_layers() -> Document {
    let mut doc = new_doc();
    add_vector(&mut doc, "Encre", vec![stroke(&[(20.0, 20.0), (80.0, 30.0)]), stroke(&[(20.0, 220.0), (80.0, 230.0)])]);
    add_vector(&mut doc, "Couleur", vec![stroke(&[(30.0, 40.0), (90.0, 50.0)]), stroke(&[(30.0, 240.0), (90.0, 250.0)])]);
    add_vector(&mut doc, "Décor", vec![stroke(&[(40.0, 60.0), (100.0, 70.0)]), stroke(&[(40.0, 260.0), (100.0, 270.0)])]);
    doc
}

#[test]
fn the_lasso_takes_the_strokes_inside_it_on_every_chosen_layer() {
    let doc = three_layers();
    let sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 200.0, 120.0));
    assert_eq!(sel.parts.len(), 3, "un morceau par calque choisi");
    for k in 0..3 {
        assert_eq!(vector_items(&sel, k), vec![0], "seul le trait du haut est dans le lasso");
    }
    let names: Vec<&str> = sel.parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Encre", "Couleur", "Décor"]);
    // Un lasso en bas prend l'autre trait de chaque calque ; un lasso vide ne prend rien.
    let low = select(&doc, &[0, 1, 2], &lasso(0.0, 200.0, 200.0, 290.0));
    assert!((0..3).all(|k| vector_items(&low, k) == vec![1]));
    assert!(select(&doc, &[0, 1, 2], &lasso(300.0, 0.0, 390.0, 40.0)).parts.is_empty());
    // Un calque qui n'est pas choisi n'est pas touché.
    let two = select(&doc, &[0, 2], &lasso(0.0, 0.0, 200.0, 120.0));
    assert_eq!(two.parts.len(), 2);
    assert!(!two.has_layer(doc.panels[0].layers[1].id));
}

#[test]
fn select_everything_takes_every_stroke_of_every_chosen_layer() {
    let doc = three_layers();
    let t = multisel::targets(&doc, 0, 0, &[0, 1, 2]);
    let sel = MultiSel::new(0, 0, multisel::select_everything(&doc, &t));
    assert_eq!(sel.parts.len(), 3);
    assert!((0..3).all(|k| vector_items(&sel, k) == vec![0, 1]));
    assert_eq!(sel.summary(), "3 calques, 6 éléments");
}

#[test]
fn the_box_is_the_union_of_every_part() {
    let doc = three_layers();
    let sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 200.0, 120.0));
    let b = sel.stage_bounds(&doc).expect("boîte");
    // Les bornes de chaque part, réunies.
    let each: Vec<_> = sel.parts.iter().map(|p| p.stage_bounds(&doc).expect("bornes")).collect();
    let union = each.iter().fold(each[0], |r, b| r.union(*b));
    assert_eq!(b, union);
    // La boîte couvre les trois traits (de x 20 à 100, de y 20 à 70, plus l'épaisseur).
    assert!(b.min.x <= 20.0 && b.max.x >= 100.0 && b.min.y <= 20.0 && b.max.y >= 70.0, "{b:?}");
    assert!(b.max.y < 120.0, "les traits du bas n'y sont pas : {b:?}");
    // La boîte droite entoure les bornes, avec le pivot au centre.
    let frame = sel.base_frame(&doc).expect("cadre");
    assert_eq!(frame.rect0, b);
    assert_eq!(frame.pivot_world(), b.center());
}

#[test]
fn the_box_follows_a_layer_with_its_own_transform() {
    let mut doc = three_layers();
    doc.panels[0].layers[1].transform = LayerTransform { x: 150.0, y: 10.0, ..Default::default() };
    let sel = select(&doc, &[0, 1], &lasso(0.0, 0.0, 400.0, 120.0));
    // Le lasso est sur la feuille : le trait décalé du calque 1 (de 150 vers la droite) est pris
    // à sa place vue, 180..240.
    assert_eq!(sel.parts.len(), 2);
    let b = sel.parts[1].stage_bounds(&doc).expect("bornes");
    assert!(b.min.x >= 175.0 && b.max.x <= 245.0, "{b:?}");
    // Un lasso qui ne couvre que l'ancienne place du trait ne le prend plus.
    let miss = select(&doc, &[1], &lasso(0.0, 0.0, 120.0, 120.0));
    assert!(miss.parts.is_empty());
}

#[test]
fn one_transform_moves_every_layer_the_same_way() {
    let mut doc = three_layers();
    // Un calque décalé et tourné, un autre agrandi : la boîte et le geste sont sur la feuille.
    doc.panels[0].layers[1].transform = LayerTransform { x: 40.0, y: -20.0, angle: 10.0, ..Default::default() };
    doc.panels[0].layers[2].transform = LayerTransform { scale: 1.5, ..Default::default() };
    let mut sel = select(&doc, &[0, 1, 2], &lasso(-100.0, -100.0, 500.0, 160.0));
    assert_eq!(sel.parts.len(), 3);
    let before: Vec<Vec<Pos2>> = (0..3).map(|li| points_of(&doc, li)).collect();
    let frame = sel.base_frame(&doc).expect("cadre");
    // Un geste de rotation de la boîte, tiré par un coin.
    let a = gesture(Handle::Rotate(2), &frame, pos2(frame.rect0.max.x + 5.0, frame.rect0.max.y + 5.0), pos2(frame.rect0.max.x - 10.0, frame.rect0.max.y + 30.0), Mods::default());
    let mut floats = Vec::new();
    let (edits, _) = multisel::run_gesture(&mut doc, &mut sel, &mut floats, &a, false);
    assert_eq!(edits.len(), 3, "un retour en arrière par dessin");
    for li in 0..3 {
        let to_stage = multisel::layer_to_stage(&doc, 0, 0, li).expect("passage");
        let after = points_of(&doc, li);
        assert_eq!(after.len(), before[li].len());
        // Chaque point sélectionné (les trois premiers du trait du haut) a suivi la même
        // transformation de la feuille ; les traits du bas n'ont pas bougé.
        let n = 2;
        for k in 0..n {
            let expected = a.apply(to_stage.apply(before[li][k]));
            assert!(near(to_stage.apply(after[k]), expected), "calque {li}, point {k} : {:?} contre {expected:?}", to_stage.apply(after[k]));
        }
        for k in n..after.len() {
            assert!(near(after[k], before[li][k]), "calque {li} : le trait non choisi ne bouge pas");
        }
    }
}

#[test]
fn one_gesture_is_one_named_undo_step_that_restores_every_layer() {
    let mut doc = three_layers();
    let mut sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 400.0, 160.0));
    let originals: Vec<Vec<Element>> = (0..3).map(|li| doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.clone()).collect();
    let frame = sel.base_frame(&doc).expect("cadre");
    let a = gesture(Handle::Move, &frame, pos2(50.0, 50.0), pos2(80.0, 65.0), Mods::default());
    let mut floats = Vec::new();
    let (edits, _) = multisel::run_gesture(&mut doc, &mut sel, &mut floats, &a, true);
    let moved: Vec<Vec<Element>> = (0..3).map(|li| doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.clone()).collect();
    assert!((0..3).all(|li| moved[li][0] != originals[li][0]), "chaque calque a bougé");

    let mut history = History::default();
    // (Comme `App::named_step` : le nom du geste est le contexte de l'étape.)
    history.context = Some(Handle::Move.step_name().to_string());
    history.push(Edit::Group(edits));
    history.context = None;
    assert_eq!(history.names().0, vec!["Déplacer"], "une seule étape, au nom du geste");
    let mut nav = Nav { panel: 0, layer: 0 };
    assert!(history.undo(&mut doc, &mut nav));
    for li in 0..3 {
        let now = doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.clone();
        assert!(now.iter().zip(&originals[li]).all(|(a, b)| a.same_as(b)), "annuler rend le calque {li} tel qu'il était");
    }
    assert!(history.undo(&mut doc, &mut nav) == false, "il n'y avait qu'une étape");
    assert!(history.redo(&mut doc, &mut nav));
    for li in 0..3 {
        let now = doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.clone();
        assert!(now.iter().zip(&moved[li]).all(|(a, b)| a.same_as(b)), "rétablir rend le calque {li} déplacé");
    }
    for f in floats {
        Flottante::release_slot(&mut doc, f.slot);
    }
}

#[test]
fn every_gesture_has_its_french_step_name_across_layers() {
    for (handle, name) in [(Handle::Move, "Déplacer"), (Handle::Corner(2), "Mettre à l'échelle"), (Handle::Rotate(0), "Tourner"), (Handle::Skew(1), "Incliner")] {
        assert_eq!(handle.step_name(), name);
    }
}

#[test]
fn arrow_nudges_join_one_undo_step() {
    let mut doc = three_layers();
    let mut sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 400.0, 160.0));
    let originals: Vec<Pos2> = (0..3).map(|li| points_of(&doc, li)[0]).collect();
    let mut history = History::default();
    let mut floats = Vec::new();
    for k in 0..4 {
        let (edits, _) = multisel::run_gesture(&mut doc, &mut sel, &mut floats, &Affine::translate(vec2(1.0, 0.0)), true);
        if k == 0 {
            history.push(Edit::Group(edits));
        } else {
            history.extend_last(Edit::Group(edits));
        }
    }
    assert_eq!(history.names().0.len(), 1, "quatre flèches, une étape");
    assert!(near(points_of(&doc, 0)[0], originals[0] + vec2(4.0, 0.0)));
    let mut nav = Nav { panel: 0, layer: 0 };
    assert!(history.undo(&mut doc, &mut nav));
    for li in 0..3 {
        assert!(near(points_of(&doc, li)[0], originals[li]), "annuler rend la place d'avant la première flèche");
    }
}

#[test]
fn hidden_locked_and_empty_layers_are_left_aside_with_a_reason() {
    let mut doc = three_layers();
    let hidden = add_vector(&mut doc, "Caché", vec![stroke(&[(50.0, 50.0), (60.0, 60.0)])]);
    let locked = add_vector(&mut doc, "Verrouillé", vec![stroke(&[(50.0, 50.0), (60.0, 60.0)])]);
    let pos_locked = add_vector(&mut doc, "Position", vec![stroke(&[(50.0, 50.0), (60.0, 60.0)])]);
    let empty = {
        let mut l = Layer::new("Vide", LayerKind::Vector);
        l.frames = vec![None];
        doc.panels[0].layers.push(l);
        doc.panels[0].layers.len() - 1
    };
    doc.panels[0].layers[hidden].visible = false;
    doc.panels[0].layers[locked].locked = true;
    doc.panels[0].layers[pos_locked].position_locked = true;
    let all: Vec<usize> = (0..doc.panels[0].layers.len()).collect();
    let t = multisel::targets(&doc, 0, 0, &all);
    let kept: Vec<&str> = t.list.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(kept, ["Encre", "Couleur", "Décor"]);
    let reasons: Vec<(&str, &str)> = t.skipped.iter().map(|(n, w)| (n.as_str(), *w)).collect();
    assert!(reasons.contains(&("Caché", "masqué")));
    assert!(reasons.contains(&("Verrouillé", "verrouillé")));
    assert!(reasons.contains(&("Position", "verrouillé")));
    assert!(reasons.contains(&("Vide", "sans dessin à cette image")));
    let text = t.skipped_text().expect("message");
    assert!(text.starts_with("Laissés de côté : ") && text.contains("Caché (masqué)"), "{text}");
    // Le lasso ne prend rien sur ces calques.
    let sel = select(&doc, &all, &lasso(0.0, 0.0, 400.0, 160.0));
    assert_eq!(sel.parts.len(), 3);
    let _ = empty;
}

#[test]
fn a_locked_folder_locks_its_layers_and_a_hidden_one_hides_them() {
    let mut doc = three_layers();
    // Les calques 0 et 1 dans un dossier (juste au-dessus d'eux dans la pile).
    let mut folder = Layer::new_folder("Dossier");
    folder.locked = true;
    let id = folder.id;
    doc.panels[0].layers.insert(2, folder);
    doc.panels[0].layers[0].parent = Some(id);
    doc.panels[0].layers[1].parent = Some(id);
    // Choisi lui-même, un dossier n'apporte pas ses calques.
    let t = multisel::targets(&doc, 0, 0, &[2]);
    assert!(t.list.is_empty() && t.skipped.is_empty(), "un dossier n'a pas de dessin, ses calques ne comptent pas s'ils ne sont pas choisis");
    // Ses calques choisis un à un sont verrouillés avec lui.
    let t = multisel::targets(&doc, 0, 0, &[0, 1, 3]);
    assert_eq!(t.list.len(), 1);
    assert_eq!(t.skipped.len(), 2);
    assert!(t.skipped.iter().all(|(_, why)| *why == "verrouillé"));
    // Déverrouillé et caché : masqués.
    doc.panels[0].layers[2].locked = false;
    doc.panels[0].layers[2].visible = false;
    let t = multisel::targets(&doc, 0, 0, &[0, 1, 3]);
    assert_eq!(t.list.len(), 1);
    assert!(t.skipped.iter().all(|(_, why)| *why == "masqué"));
    // Déverrouillé et visible : ses calques, choisis, comptent.
    doc.panels[0].layers[2].visible = true;
    let t = multisel::targets(&doc, 0, 0, &[0, 1, 3]);
    assert_eq!(t.list.len(), 3);
}

#[test]
fn only_the_drawing_at_the_current_frame_counts() {
    let mut doc = three_layers();
    // Au calque 0, un second dessin à l'image 1.
    let second = doc.new_cel(LayerKind::Vector);
    doc.cels.get_mut(&second).unwrap().vector.as_mut().unwrap().elements = vec![stroke(&[(200.0, 100.0), (260.0, 110.0)])];
    doc.panels[0].layers[0].set(1, Some(second));
    let t0 = multisel::targets(&doc, 0, 0, &[0, 1]);
    let t1 = multisel::targets(&doc, 0, 1, &[0, 1]);
    assert_eq!(t0.list.len(), 2);
    assert_eq!(t1.list.len(), 1, "le calque 1 n'a pas de dessin à l'image 1");
    assert_eq!(t1.list[0].cel, second);
    assert!(t1.skipped.iter().any(|(n, why)| n == "Couleur" && *why == "sans dessin à cette image"));
    let poly = lasso(150.0, 50.0, 300.0, 150.0);
    let sel = MultiSel::new(0, 1, multisel::select_polygon(&doc, &t1, &poly));
    assert_eq!(sel.parts.len(), 1);
    assert_eq!(sel.parts[0].cel, second);
}

#[test]
fn vector_and_bitmap_layers_move_together() {
    let mut doc = new_doc();
    let v = add_vector(&mut doc, "Traits", vec![stroke(&[(60.0, 60.0), (90.0, 60.0)])]);
    let b = add_bitmap(&mut doc, "Aplat", (50, 50, 40, 40));
    let t = multisel::targets(&doc, 0, 0, &[v, b]);
    assert_eq!(t.list.len(), 2);
    assert!(t.list[0].vector && !t.list[1].vector);
    let mut sel = MultiSel::new(0, 0, multisel::select_polygon(&doc, &t, &lasso(40.0, 40.0, 100.0, 100.0)));
    assert_eq!(sel.parts.len(), 2, "des traits et une zone");
    assert!(sel.has_bitmap());
    assert_eq!(sel.summary(), "2 calques, 1 élément et 1 zone");
    let bitmap_cel = cel_of(&doc, b);
    let before_raster: Raster = doc.cels[&bitmap_cel].raster.clone();
    let before_points = points_of(&doc, v);
    let before_bounds = sel.stage_bounds(&doc).expect("boîte");

    let mut floats = Vec::new();
    let (edits, _) = multisel::run_gesture(&mut doc, &mut sel, &mut floats, &Affine::translate(vec2(20.0, 10.0)), true);
    assert_eq!(edits.len(), 2, "le dessin vectoriel et les tuiles du bitmap");
    assert!(near(points_of(&doc, v)[0], before_points[0] + vec2(20.0, 10.0)));
    let r = &doc.cels[&bitmap_cel].raster;
    assert_eq!(r.get(70, 60), Color32::from_rgb(200, 0, 0), "les pixels sont arrivés");
    assert_eq!(r.get(50, 50), Color32::TRANSPARENT, "et ont quitté leur place");
    // La boîte suit le déplacement.
    let bounds = sel.stage_bounds(&doc).expect("boîte");
    assert!((bounds.center().x - before_bounds.center().x - 20.0).abs() < 0.01 && (bounds.center().y - before_bounds.center().y - 10.0).abs() < 0.01, "{before_bounds:?} puis {bounds:?}");

    let mut history = History::default();
    history.context = Some("Déplacer".into());
    history.push(Edit::Group(edits));
    let mut nav = Nav { panel: 0, layer: 0 };
    assert!(history.undo(&mut doc, &mut nav));
    let r = &doc.cels[&bitmap_cel].raster;
    assert!(multisel::changed_tiles(&before_raster, r).is_empty(), "le bitmap est revenu tel qu'il était");
    assert!(near(points_of(&doc, v)[0], before_points[0]));
    for f in floats {
        Flottante::release_slot(&mut doc, f.slot);
    }
}

#[test]
fn a_second_gesture_on_a_bitmap_part_starts_from_the_lifted_pixels() {
    let mut doc = new_doc();
    let v = add_vector(&mut doc, "Traits", vec![stroke(&[(60.0, 60.0), (90.0, 60.0)])]);
    let b = add_bitmap(&mut doc, "Aplat", (50, 50, 40, 40));
    let t = multisel::targets(&doc, 0, 0, &[v, b]);
    let mut sel = MultiSel::new(0, 0, multisel::select_polygon(&doc, &t, &lasso(40.0, 40.0, 100.0, 100.0)));
    let cel = cel_of(&doc, b);
    let mut floats = Vec::new();
    multisel::run_gesture(&mut doc, &mut sel, &mut floats, &Affine::translate(vec2(10.0, 0.0)), true);
    multisel::run_gesture(&mut doc, &mut sel, &mut floats, &Affine::translate(vec2(0.0, 25.0)), true);
    let r = &doc.cels[&cel].raster;
    assert_eq!(r.get(65, 80), Color32::from_rgb(200, 0, 0), "deux déplacements s'ajoutent");
    assert_eq!(r.get(55, 55), Color32::TRANSPARENT);
    assert_eq!(r.get(65, 60), Color32::TRANSPARENT, "rien n'est resté à mi-chemin");
    for f in floats {
        Flottante::release_slot(&mut doc, f.slot);
    }
}

#[test]
fn delete_removes_the_chosen_items_of_every_layer_in_one_step() {
    let mut doc = three_layers();
    let b = add_bitmap(&mut doc, "Aplat", (50, 50, 40, 40));
    let sel = select(&doc, &[0, 1, 2, b], &lasso(0.0, 0.0, 200.0, 120.0));
    assert_eq!(sel.parts.len(), 4);
    let counts: Vec<usize> = (0..3).map(|li| doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.len()).collect();
    assert_eq!(counts, [2, 2, 2]);
    let edits = multisel::delete_selection(&mut doc, &sel);
    for li in 0..3 {
        assert_eq!(doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.len(), 1, "le trait du haut est parti");
    }
    assert_eq!(doc.cels[&cel_of(&doc, b)].raster.get(60, 60), Color32::TRANSPARENT);
    let mut history = History::default();
    history.context = Some("Supprimer".into());
    history.push(Edit::Group(edits));
    assert_eq!(history.names().0, vec!["Supprimer"]);
    assert!(history.undo(&mut doc, &mut Nav { panel: 0, layer: 0 }));
    for li in 0..3 {
        assert_eq!(doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap().elements.len(), 2);
    }
    assert_eq!(doc.cels[&cel_of(&doc, b)].raster.get(60, 60), Color32::from_rgb(200, 0, 0));
}

#[test]
fn copy_then_paste_puts_each_layer_back_on_itself() {
    let mut doc = three_layers();
    let b = add_bitmap(&mut doc, "Aplat", (50, 50, 40, 40));
    let all = [0usize, 1, 2, b];
    let sel = select(&doc, &all, &lasso(0.0, 0.0, 200.0, 120.0));
    let clip = multisel::copy_selection(&doc, &sel);
    assert_eq!(clip.items.len(), 4, "un morceau par calque");
    let t = multisel::targets(&doc, 0, 0, &all);
    let (parts, edits) = multisel::paste_clip(&mut doc, &t, &clip);
    assert_eq!(parts.len(), 4);
    // (Les pixels collés tombent sur les mêmes : le bitmap ne change pas, rien à annuler.)
    assert_eq!(edits.len(), 3);
    for li in 0..3 {
        let d = doc.cels[&cel_of(&doc, li)].vector.as_ref().unwrap();
        assert_eq!(d.elements.len(), 3, "le trait copié s'ajoute au calque {li}");
        assert!(d.elements[2].same_as(&d.elements[0]), "à la même place");
    }
    // Ce qui est collé est la sélection : un geste ne déplace que lui.
    let mut sel = MultiSel::new(0, 0, parts);
    assert_eq!(sel.summary(), "4 calques, 3 éléments et 1 zone");
    let mut floats = Vec::new();
    multisel::run_gesture(&mut doc, &mut sel, &mut floats, &Affine::translate(vec2(100.0, 0.0)), true);
    let d = doc.cels[&cel_of(&doc, 0)].vector.as_ref().unwrap();
    assert!(near(points_of(&doc, 0)[0], pos2(20.0, 20.0)), "l'original ne bouge pas");
    assert!(d.elements.len() == 3);
    for f in floats {
        Flottante::release_slot(&mut doc, f.slot);
    }
}

#[test]
fn shift_adds_to_the_selection_and_toggles_single_strokes() {
    let doc = three_layers();
    let mut sel = select(&doc, &[0, 1], &lasso(0.0, 0.0, 200.0, 120.0));
    let more = select(&doc, &[0, 1, 2], &lasso(0.0, 200.0, 200.0, 290.0)).parts;
    sel.merge(more);
    assert_eq!(sel.parts.len(), 3, "le calque 2 s'ajoute");
    assert_eq!(vector_items(&sel, 0), vec![0, 1], "les traits d'un même calque se réunissent");
    let t = multisel::targets(&doc, 0, 0, &[0, 1, 2]);
    sel.toggle_item(&t.list[0], 1);
    assert_eq!(vector_items(&sel, 0), vec![0], "Maj + clic retire un trait choisi");
    sel.toggle_item(&t.list[0], 0);
    assert!(!sel.has_layer(t.list[0].id), "plus aucun trait : le calque quitte la sélection");
}

#[test]
fn a_click_finds_the_topmost_stroke_across_layers() {
    let mut doc = new_doc();
    add_vector(&mut doc, "Dessous", vec![stroke(&[(10.0, 50.0), (200.0, 50.0)])]);
    add_vector(&mut doc, "Dessus", vec![stroke(&[(10.0, 50.0), (200.0, 50.0)])]);
    let t = multisel::targets(&doc, 0, 0, &[0, 1]);
    let (hit, item) = multisel::hit_item(&doc, &t, pos2(100.0, 51.0), 4.0).expect("un trait");
    assert_eq!((hit.name.as_str(), item), ("Dessus", 0));
    assert!(multisel::hit_item(&doc, &t, pos2(100.0, 150.0), 4.0).is_none());
}

#[test]
fn the_selection_is_only_valid_where_it_was_taken() {
    let mut doc = three_layers();
    let sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 200.0, 120.0));
    assert!(sel.still_valid(&doc, 0, 0));
    assert!(!sel.still_valid(&doc, 0, 1), "une autre image");
    // Un dessin qui perd des éléments : les rangs choisis n'existent plus.
    let cel = cel_of(&doc, 1);
    doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements.clear();
    assert!(!sel.still_valid(&doc, 0, 0));
}

#[test]
fn the_summary_counts_layers_and_items() {
    let doc = three_layers();
    let sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 400.0, 300.0));
    assert_eq!(sel.summary(), "3 calques, 6 éléments");
    let one = select(&doc, &[0, 1], &lasso(0.0, 0.0, 200.0, 120.0));
    assert_eq!(one.summary(), "2 calques, 2 éléments");
    let single = select(&doc, &[0], &lasso(0.0, 0.0, 200.0, 120.0));
    assert_eq!(single.summary(), "1 calque, 1 élément");
}

#[test]
fn the_layer_delta_applies_the_stage_gesture_in_the_layer_space() {
    let t = LayerTransform { x: 30.0, y: 10.0, angle: 20.0, scale: 0.8, ..Default::default() }.affine(pos2(200.0, 150.0));
    let a = Affine::rotate_about(pos2(120.0, 90.0), 0.4).then(&Affine::translate(vec2(7.0, -3.0)));
    let d = multisel::layer_delta(&t, &a).expect("inversible");
    for p in [pos2(5.0, 8.0), pos2(300.0, 20.0), pos2(-40.0, 200.0)] {
        let by_layer = t.apply(d.apply(p));
        let by_stage = a.apply(t.apply(p));
        assert!(near(by_layer, by_stage), "{by_layer:?} contre {by_stage:?}");
    }
    // Un calque sans transformation : le geste est le même.
    let same = multisel::layer_delta(&Affine::IDENTITY, &a).expect("identité");
    assert!(near(same.apply(pos2(10.0, 10.0)), a.apply(pos2(10.0, 10.0))));
}

/// L'outil Transformer sur plusieurs calques (`layertools.rs`) fait tourner et agrandir chaque
/// calque par `LayerTransform::around`, autour d'un point de la feuille : ce que voit l'écran est la
/// même similitude pour tous (rotation `rotate_about`, échelle `scale_about`).
#[test]
fn the_layer_tool_moves_every_layer_by_the_same_similarity_of_the_stage() {
    let pivot = pos2(200.0, 150.0);
    let c = pos2(130.0, 90.0);
    for t0 in [
        LayerTransform::default(),
        LayerTransform { x: 30.0, y: -10.0, angle: 15.0, scale: 0.7, ..Default::default() },
        LayerTransform { x: -12.0, y: 40.0, angle: -50.0, scale: 1.8, stretch_x: 1.3, ..Default::default() },
    ] {
        let before = t0.affine(pivot);
        // Tourner de 35° autour de c.
        let turned = t0.around(pivot, c, 1.0, 35.0).affine(pivot);
        let want = before.then(&Affine::rotate_about(c, 35f32.to_radians()));
        // Agrandir de 1,6 autour de c.
        let grown = t0.around(pivot, c, 1.6, 0.0).affine(pivot);
        let want_grown = before.then(&Affine::scale_about(c, 1.6, 1.6));
        for p in [pos2(0.0, 0.0), pos2(120.0, 80.0), pos2(310.0, 260.0)] {
            assert!(near(turned.apply(p), want.apply(p)), "rotation : {:?} contre {:?}", turned.apply(p), want.apply(p));
            assert!(near(grown.apply(p), want_grown.apply(p)), "échelle : {:?} contre {:?}", grown.apply(p), want_grown.apply(p));
        }
    }
}

#[test]
fn floating_sheets_have_their_own_places() {
    let mut doc = three_layers();
    let mut sel = select(&doc, &[0, 1, 2], &lasso(0.0, 0.0, 400.0, 160.0));
    let mut floats = Vec::new();
    let snaps = multisel::begin_gesture(&mut doc, &mut sel, &mut floats);
    assert_eq!(snaps.len(), 3);
    assert_eq!(floats.len(), 3, "une feuille par calque");
    let slots: Vec<usize> = floats.iter().map(|f| f.slot).collect();
    assert!(slots.iter().all(|&s| s >= 1), "la place 0 reste à la sélection d'un seul calque");
    for f in &floats {
        let (base, sheet) = crate::flottante::slot_ids(f.slot);
        assert!(doc.cels.contains_key(&base) && doc.cels.contains_key(&sheet));
    }
    for f in &floats {
        Flottante::release_slot(&mut doc, f.slot);
    }
    assert!(!doc.cels.contains_key(&crate::flottante::BASE_CEL) && !doc.cels.contains_key(&crate::flottante::SHEET_CEL));
}

#[test]
fn history_extend_last_makes_a_group_of_both_steps() {
    let mut doc = three_layers();
    let cel = cel_of(&doc, 0);
    let first = doc.cels[&cel].vector.clone().unwrap();
    let mut history = History::default();
    history.push(Edit::Vector { cel, drawing: first.clone() });
    doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements.truncate(1);
    let second = doc.cels[&cel].vector.clone().unwrap();
    history.extend_last(Edit::Vector { cel, drawing: second });
    doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements.clear();
    assert_eq!(history.names().0.len(), 1);
    assert!(history.undo(&mut doc, &mut Nav { panel: 0, layer: 0 }));
    assert_eq!(doc.cels[&cel].vector.as_ref().unwrap().elements.len(), 2, "l'étape unique rend tout");
    assert!(history.redo(&mut doc, &mut Nav { panel: 0, layer: 0 }));
    assert_eq!(doc.cels[&cel].vector.as_ref().unwrap().elements.len(), 0);
}

/// Le cas d'Anthony, sur plusieurs calques : trois calques choisis de trente traits texturés, un
/// geste de déplacement. Par image : les trois feuilles flottantes reçoivent la transformation et le
/// plan les montre ; jamais de rejeu du moteur (une milliseconde par image environ).
/// `cargo test --release --offline --target-dir target\msvc three_layers_thirty_strokes -- --ignored --nocapture`
#[test]
#[ignore]
fn three_layers_thirty_strokes_move_at_one_ms_per_frame() {
    use crate::affichage_tests::{gros_traits, line_item, ms};
    use crate::render::Plan;
    let (w, h) = (1920, 1080);
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    for (k, name) in ["Encre", "Couleur", "Décor"].iter().enumerate() {
        let li = add_vector(&mut doc, name, gros_traits(30, w, h, 11 + k as u64));
        // (Le premier est décalé, le second tourné : chacun garde sa transformation.)
        match k {
            1 => doc.panels[0].layers[li].transform = LayerTransform { x: 25.0, y: -12.0, ..Default::default() },
            2 => doc.panels[0].layers[li].transform = LayerTransform { angle: 4.0, ..Default::default() },
            _ => {}
        }
    }
    let t = multisel::targets(&doc, 0, 0, &[0, 1, 2]);
    let mut sel = MultiSel::new(0, 0, multisel::select_everything(&doc, &t));
    assert_eq!(sel.summary(), "3 calques, 90 éléments");
    let mut floats = Vec::new();
    let t0 = Instant::now();
    let snaps = multisel::begin_gesture(&mut doc, &mut sel, &mut floats);
    let lift = ms(t0);
    let replays = crate::vector::replay_count();
    let frames = 300;
    let t0 = Instant::now();
    for k in 0..frames {
        let a = Affine::translate(vec2(k as f32 * 0.7, k as f32 * 0.3));
        multisel::set_gesture(&mut doc, &mut sel, &mut floats, &snaps, &a, false);
        // Ce que le plan montre : par calque, le dessin sans la sélection et sa feuille transformée.
        let items: Vec<_> = floats.iter().flat_map(|f| f.items(&line_item(f.src)).into_iter()).collect();
        assert_eq!(items.len(), 6);
        let plan = Plan { paper: None, items };
        assert!(plan.items.iter().filter(|i| i.pre.is_some()).count() == 3);
    }
    let per_frame = ms(t0) / frames as f64;
    assert_eq!(crate::vector::replay_count(), replays, "aucun rejeu du moteur pendant le geste");
    // Le lâcher : la géométrie des 90 traits, sans rien refaire.
    let t0 = Instant::now();
    let a = Affine::translate(vec2(100.0, 40.0));
    let edits = multisel::commit_gesture(&mut doc, &mut sel, &mut floats, snaps, &a);
    let commit = ms(t0);
    assert_eq!(edits.len(), 3);
    assert_eq!(crate::vector::replay_count(), replays, "ni au lâcher");
    println!("3 calques × 30 traits texturés : saisie {lift:.1} ms (une fois) | par image {per_frame:.3} ms | lâcher {commit:.2} ms | 0 rejeu");
    assert!(per_frame < 2.0, "{per_frame:.3} ms par image");
    for f in floats {
        Flottante::release_slot(&mut doc, f.slot);
    }
}
