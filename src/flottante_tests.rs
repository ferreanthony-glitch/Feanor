//! Feuille flottante d'une sélection vectorielle (`flottante.rs`) : le résultat à l'écran est celui
//! d'avant, et un geste ne coûte plus qu'un changement de transformation (essai `#[ignore]` des
//! durées : le cas d'Anthony, 40 traits de « Crayon net » sur toute la feuille).

use std::time::Instant;

use eframe::egui::{vec2, Color32, Rect};

use crate::brush::BrushSettings;
use crate::brushtex::BrushLibrary;
use crate::document::{Cel, CelId, Document, Layer, LayerKind, LayerTransform};
use crate::flottante::{Flottante, BASE_CEL, SHEET_CEL};
use crate::fondfil_tests::{geste, ms, trait_de};
use crate::fx;
use crate::raster::TILE_PIXELS;
use crate::render::{self, Plan, PlanItem};
use crate::vector::{Affine, Element};

fn line_item(cel: CelId) -> PlanItem {
    PlanItem { cel, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: Vec::new(), ctg: None, fond: None, pre: None }
}

fn doc_with(elements: Vec<Element>) -> (Document, CelId) {
    let mut doc = Document::new(1920, 1080, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.vector.as_mut().unwrap().elements = elements;
    cel.rerender();
    (doc, id)
}

/// 8 colonnes × 5 lignes de traits de « Crayon net » de 2,5 px sur toute la feuille.
fn crayon_net(n: usize) -> Vec<Element> {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut s = BrushSettings::pencil();
    s.size = 2.5;
    (0..n).map(|k| trait_de(&s, &lib, &geste(160, 20.0 + (k % 8) as f32 * 225.0, 70.0 + (k / 8) as f32 * 200.0))).collect()
}

fn flat(doc: &Document, plan: &Plan) -> Vec<Color32> {
    render::composite_full(doc, plan)
}

#[test]
fn a_whole_selection_floats_and_shows_the_same_image_as_the_rendered_drawing() {
    let (mut doc, id) = doc_with(crayon_net(6));
    let items: Vec<usize> = (0..6).collect();
    let f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    // Sans transformation : l'image du dessin.
    let plan = Plan { paper: None, items: f.items(&line_item(id)).to_vec() };
    let reference = Plan { paper: None, items: vec![line_item(id)] };
    assert_eq!(flat(&doc, &plan), flat(&doc, &reference));
    Flottante::release(&mut doc);
    assert!(!doc.cels.contains_key(&BASE_CEL) && !doc.cels.contains_key(&SHEET_CEL), "les dessins temporaires s'en vont");
}

#[test]
fn a_floating_translation_matches_the_moved_strokes() {
    let (mut doc, id) = doc_with(crayon_net(6));
    let items: Vec<usize> = (0..6).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).unwrap();
    f.aff = Affine::translate(vec2(37.0, -12.0));
    let plan = Plan { paper: None, items: f.items(&line_item(id)).to_vec() };
    let floating = flat(&doc, &plan);
    Flottante::release(&mut doc);
    // Ce que fait le lâcher : la géométrie, et l'image refaite.
    let cel = doc.cels.get_mut(&id).unwrap();
    let d = cel.vector.as_mut().unwrap();
    for e in d.elements.iter_mut() {
        *e = e.transformed_preview(&f.aff);
    }
    cel.rerender();
    let committed = flat(&doc, &Plan { paper: None, items: vec![line_item(id)] });
    let diff = floating.iter().zip(&committed).filter(|(a, b)| a != b).count();
    if diff > 0 {
        let (i, (a, b)) = floating.iter().zip(&committed).enumerate().find(|(_, (a, b))| a != b).unwrap();
        let maxd = floating.iter().zip(&committed).map(|(a, b)| (a.a() as i32 - b.a() as i32).abs()).max().unwrap();
        println!("premier écart à ({}, {}) : {a:?} contre {b:?}, écart d'opacité max {maxd}", i % 1920, i / 1920);
    }
    assert_eq!(diff, 0, "{diff} pixels diffèrent");
}

#[test]
fn the_cheap_release_gives_the_geometry_the_background_replay_will_confirm() {
    // Au lâcher la géométrie seule est posée ; le rejeu exact copie ce dessin, et ne sera adopté
    // que si la géométrie est celle de l'ancienne voie (`same_as`).
    let (mut doc, id) = doc_with(crayon_net(5));
    let originals = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    let items: Vec<usize> = vec![1, 3];
    let picked: Vec<Element> = items.iter().map(|&i| originals[i].clone()).collect();
    let aff = Affine::rotate_about(eframe::egui::pos2(900.0, 500.0), 0.4);
    Flottante::apply_geometry(&mut doc, id, &items, &picked, &aff);
    let cheap = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    // Les autres traits n'ont pas bougé.
    for i in [0, 2, 4] {
        assert!(cheap[i].same_as(&originals[i]));
    }
    // Retour à l'état d'avant, puis l'ancienne voie.
    doc.cels.get_mut(&id).unwrap().vector.as_mut().unwrap().elements = originals.clone();
    let tiles = Flottante::apply_preview(&mut doc, id, &items, &picked, &aff);
    assert!(!tiles.is_empty());
    let old = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    for i in 0..5 {
        assert!(cheap[i].same_as(&old[i]), "trait {i}");
    }
}

#[test]
fn only_the_chosen_strokes_float_the_others_stay_in_the_base() {
    let (mut doc, id) = doc_with(crayon_net(6));
    let f = Flottante::lift(&mut doc, id, &[0, 1]).unwrap();
    let ink = |c: CelId| doc.cels[&c].raster.tiles.iter().flatten().count();
    assert!(ink(BASE_CEL) > 0 && ink(SHEET_CEL) > 0);
    // Les deux feuilles ensemble sont l'image du dessin.
    let both = flat(&doc, &Plan { paper: None, items: f.items(&line_item(id)).to_vec() });
    let whole = flat(&doc, &Plan { paper: None, items: vec![line_item(id)] });
    assert_eq!(both, whole);
}

#[test]
fn the_gesture_transform_comes_before_the_layer_transform() {
    let mut item = line_item(1);
    let pivot = vec2(960.0, 540.0).to_pos2();
    let t = LayerTransform { x: 30.0, y: 10.0, scale: 1.5, angle: 20.0, ..Default::default() };
    item.transform = Some(t);
    item.pre = Some(crate::render::Pre { aff: Affine::translate(vec2(100.0, 0.0)), dims: None, warp: None });
    let p = vec2(400.0, 300.0).to_pos2();
    let expected = t.affine(pivot).apply(Affine::translate(vec2(100.0, 0.0)).apply(p));
    let got = item.affine(pivot).unwrap().apply(p);
    assert!((expected - got).length() < 1e-3);
    item.pre = None;
    assert_eq!(item.affine(pivot).unwrap().apply(p), t.affine(pivot).apply(p));
    item.transform = None;
    assert!(item.affine(pivot).is_none());
    let _ = Layer::new("L", LayerKind::Vector);
    let _ = Cel::new(1, 1, LayerKind::Vector);
}

// ------------------------------------------------------------------ durées (scénario d'Anthony)

/// Le cas d'Anthony : un projet 1920×1080, un dessin de 40 traits de « Crayon net » de 2,5 px qui
/// couvre presque toute la feuille, tout sélectionné, puis déplacement, rotation, échelle.
/// Avant : à chaque image les 40 traits refaits dans l'image du dessin (`transformed_preview` puis
/// `rerender_region`), et les tuiles invalidées renvoyées à la carte graphique. Après : la feuille
/// flottante, une transformation par image.
/// `cargo test --release --offline --target-dir target\audit floating_selection -- --ignored --nocapture`
#[test]
#[ignore]
fn floating_selection_gesture_is_cheap_per_frame() {
    mesure("Crayon net 2,5 px", crayon_net(40));
}

/// Les brosses texturées : 50 traits de Crayon graphite, Fusain, Craie et Gouache.
/// `cargo test --release --offline --target-dir target\audit floating_selection_textured -- --ignored --nocapture`
#[test]
#[ignore]
fn floating_selection_textured_brushes_are_cheap_per_frame() {
    let lib = BrushLibrary::builtin();
    lib.share();
    for name in ["Crayon graphite", "Fusain", "Craie", "Gouache"] {
        let s = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
        let elements = (0..50).map(|k| trait_de(&s, &lib, &geste(160, 20.0 + (k % 8) as f32 * 225.0, 60.0 + (k / 8) as f32 * 150.0))).collect();
        mesure(name, elements);
    }
}

fn mesure(label: &str, elements: Vec<Element>) {
    println!("--- {label}");
    let n = elements.len();
    let items: Vec<usize> = (0..n).collect();
    let (mut doc, id) = doc_with(elements);
    let originals = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    let bbox = originals.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
    println!("dessin : {n} traits, boîte {:.0}×{:.0} px", bbox.width(), bbox.height());
    let gestures: [(&str, Box<dyn Fn(f32) -> Affine>); 3] = [
        ("translation", Box::new(|t| Affine::translate(vec2(400.0 * t, 250.0 * t)))),
        ("rotation", Box::new(move |t| Affine::rotate_about(bbox.center(), 0.9 * t))),
        ("échelle", Box::new(move |t| Affine::scale_about(bbox.left_top(), 1.0 + 0.5 * t, 1.0 + 0.5 * t))),
    ];
    println!("  geste        | avant : ms/image, tuiles, Mo envoyés | après : ms/image, tuiles, octets | CPU seul (sans GPU) ms/image");
    let frames = 20;
    for (name, make) in &gestures {
        // Avant : l'ancien select_drag, image par image.
        let (mut total, mut tiles_total) = (0.0, 0usize);
        for f in 1..=frames {
            let aff = make(f as f32 / frames as f32);
            let t = Instant::now();
            let cel = doc.cels.get_mut(&id).unwrap();
            let d = cel.vector.as_mut().unwrap();
            let mut region = Rect::NOTHING;
            for (i, src) in originals.iter().enumerate() {
                region = region.union(d.elements[i].bounds().expand(1.5));
                d.elements[i] = src.transformed_preview(&aff);
                region = region.union(d.elements[i].bounds().expand(1.5));
            }
            let tiles = cel.rerender_region(region.expand(2.0));
            total += ms(t);
            tiles_total += tiles.len();
        }
        let before_ms = total / frames as f64;
        let before_tiles = tiles_total / frames as usize;
        let before_mb = before_tiles as f64 * TILE_PIXELS as f64 * 4.0 / 1e6;
        // Retour au dessin d'origine.
        let cel = doc.cels.get_mut(&id).unwrap();
        cel.vector.as_mut().unwrap().elements = originals.clone();
        cel.rerender();

        // Après : saisie, puis une transformation par image.
        let t = Instant::now();
        let mut fl = Flottante::lift(&mut doc, id, &items).unwrap();
        let lift_ms = ms(t);
        let versions = |doc: &Document| (doc.cels[&BASE_CEL].version, doc.cels[&SHEET_CEL].version);
        let v0 = versions(&doc);
        let (mut total, mut cpu) = (0.0, 0.0);
        let mut changed_textures = 0;
        let item = line_item(id);
        for f in 1..=frames {
            fl.aff = make(f as f32 / frames as f32);
            // Ce que l'application fait à chaque image : le plan, et la recherche des dessins.
            let t = Instant::now();
            let plan = Plan { paper: Some([255, 255, 255]), items: fl.items(&item).to_vec() };
            for it in &plan.items {
                let _ = fx::shown(&doc, it);
            }
            total += ms(t);
            if versions(&doc) != v0 {
                changed_textures += 1;
            }
            // Sans carte graphique : la composition par le processeur (tuiles qui touchent la feuille).
            if f == frames {
                let t = Instant::now();
                let _ = render::composite_full(&doc, &plan);
                cpu = ms(t);
            }
        }
        let after_ms = total / frames as f64;
        // Le lâcher : la géométrie et l'image du dessin refaites une fois.
        // (La feuille reste affichée jusqu'à l'arrivée du rendu exact.)
        let t = Instant::now();
        let aff = fl.aff;
        Flottante::apply_geometry(&mut doc, id, &items, &originals, &aff);
        let release_ms = ms(t);
        // Si l'utilisateur agit avant l'arrivée du rendu : l'ancienne voie, une fois.
        let t = Instant::now();
        let rel_tiles = Flottante::apply_preview(&mut doc, id, &items, &originals, &aff).len();
        let flush_ms = ms(t);
        Flottante::release(&mut doc);
        println!(
            "  {name:<12} | {before_ms:>8.1} ms, {before_tiles:>4} tuiles, {before_mb:>5.1} Mo | {after_ms:>8.3} ms, {changed_textures} tuiles, 0 octet | {cpu:>6.1}   (saisie {lift_ms:.1} ms, lâcher {release_ms:.2} ms ; si action avant le rendu exact : {flush_ms:.1} ms, {rel_tiles} tuiles)"
        );
        assert!(release_ms < 8.0 && lift_ms < 16.0, "{name} : saisie {lift_ms:.1} ms, lâcher {release_ms:.1} ms");
        assert!(after_ms < 8.0, "{name} : {after_ms:.2} ms par image");
        assert_eq!(changed_textures, 0, "aucune tuile renvoyée pendant le geste");
        let cel = doc.cels.get_mut(&id).unwrap();
        cel.vector.as_mut().unwrap().elements = originals.clone();
        cel.rerender();
    }
}
