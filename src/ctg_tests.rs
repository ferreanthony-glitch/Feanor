//! Calque CTG (13j) : remplissage par les graffitis, sources, structure, enregistrement.

use eframe::egui::Color32;

use crate::ctg::{self, Ctg};
use crate::document::{Document, Layer, LayerKind};
use crate::raster::Raster;

/// Une feuille 64×64 : un carré de traits noirs de (10, 10) à (40, 40), épais de 2 pixels.
fn square() -> Raster {
    let mut r = Raster::new(64, 64);
    for i in 10..=40 {
        for t in 0..2 {
            for (x, y) in [(i, 10 + t), (i, 40 - t), (10 + t, i), (40 - t, i)] {
                r.tile_mut(0)[y * 64 + x] = Color32::BLACK;
            }
        }
    }
    r
}

fn dot(r: &mut Raster, x: usize, y: usize, c: Color32) {
    r.tile_mut(0)[y * 64 + x] = c;
}

#[test]
fn a_graffiti_fills_the_closed_area_it_touches() {
    let lines = square();
    let mut g = Raster::new(64, 64);
    dot(&mut g, 25, 25, Color32::RED);
    let out = ctg::fill(&[&lines], &g, 0);
    assert_eq!(out.get(20, 30), Color32::RED, "toute la zone");
    assert_eq!(out.get(12, 25), Color32::RED, "jusqu'au trait");
    assert_eq!(out.get(11, 25), Color32::RED, "et sous le trait");
    assert_eq!(out.get(5, 5), Color32::TRANSPARENT, "pas dehors");
    assert_eq!(out.get(50, 25), Color32::TRANSPARENT);
}

#[test]
fn the_most_present_colour_wins_and_lines_do_not_count() {
    let lines = square();
    let mut g = Raster::new(64, 64);
    for x in 20..25 {
        dot(&mut g, x, 20, Color32::BLUE);
    }
    dot(&mut g, 30, 30, Color32::RED);
    // Un graffiti posé sur le trait ne remplit rien.
    dot(&mut g, 10, 50, Color32::GREEN);
    dot(&mut g, 10, 25, Color32::GREEN);
    let out = ctg::fill(&[&lines], &g, 0);
    assert_eq!(out.get(30, 30), Color32::BLUE, "le bleu est le plus présent dans la zone");
    assert_eq!(out.get(5, 5), Color32::GREEN, "le vert hors du carré remplit le dehors");
}

#[test]
fn small_holes_in_the_lines_are_closed() {
    // Le carré, avec un trou de 4 pixels dans son côté gauche (y de 23 à 26).
    let mut lines = square();
    for y in 23..27 {
        for x in 10..12 {
            lines.tile_mut(0)[y * 64 + x] = Color32::TRANSPARENT;
        }
    }
    let mut g = Raster::new(64, 64);
    dot(&mut g, 25, 25, Color32::RED);
    let leaking = ctg::fill(&[&lines], &g, 0);
    assert_eq!(leaking.get(5, 5), Color32::RED, "sans fermeture : la couleur fuit dehors");
    let closed = ctg::fill(&[&lines], &g, 2);
    assert_eq!(closed.get(5, 5), Color32::TRANSPARENT, "trou de 4 pixels fermé (2 de chaque côté)");
    assert_eq!(closed.get(25, 25), Color32::RED);
    assert_eq!(closed.get(13, 30), Color32::RED, "la couleur revient jusqu'au trait");
    assert_eq!(closed.get(11, 30), Color32::RED, "et dessous");
    assert_eq!(closed.get(4, 25), Color32::TRANSPARENT, "rien ne sort par le trou");
    // Un trou plus grand que la fermeture laisse passer.
    assert_eq!(ctg::fill(&[&lines], &g, 1).get(5, 5), Color32::RED);
}

#[test]
fn without_lines_a_graffiti_fills_everything() {
    let mut g = Raster::new(64, 64);
    dot(&mut g, 3, 3, Color32::RED);
    let out = ctg::fill(&[], &g, 0);
    assert_eq!(out.get(63, 63), Color32::RED);
}

#[test]
fn sources_and_structure() {
    let mut doc = Document::new(64, 64, 24);
    let [a, b, c, g] = [0; 4].map(|_| doc.new_cel(LayerKind::Bitmap));
    let mut lines = Layer::new("Traits", LayerKind::Bitmap);
    // A A B B — C
    lines.frames = vec![Some(a), Some(a), Some(b), Some(b), None, Some(c)];
    let mut other = Layer::new("Autre", LayerKind::Bitmap);
    other.frames = vec![Some(a)];
    let mut ctg_layer = Layer::new("CTG", LayerKind::Bitmap);
    ctg_layer.ctg = Some(Ctg { sources: vec![lines.id], ..Ctg::default() });
    ctg_layer.frames = vec![Some(g)];
    let layers = vec![ctg_layer.clone(), lines.clone(), other.clone()];
    assert_eq!(ctg::source_layers(&layers, 0), vec![1], "la source choisie");
    assert_eq!(ctg::source_cels(&layers, 0, 2), vec![b]);
    let mut all = layers.clone();
    all[0].ctg = Some(Ctg::default());
    assert_eq!(ctg::source_layers(&all, 0), vec![1, 2], "aucune choisie : toutes les autres");

    // Structure : un dessin de graffitis par exposition des traits ; le premier garde le sien.
    let mut next = 100;
    let frames = ctg::structure(6, &[&lines], &ctg_layer.frames, false, || {
        next += 1;
        next
    });
    assert_eq!(frames, vec![Some(g), Some(g), Some(101), Some(101), None, Some(102)]);
    let mut next = 200;
    let cleared = ctg::structure(6, &[&lines], &ctg_layer.frames, true, || {
        next += 1;
        next
    });
    assert_eq!(cleared[0], Some(201), "effacer : des graffitis neufs");
}

#[test]
fn the_plan_fills_the_ctg_layer_at_export() {
    let mut doc = Document::new(64, 64, 24);
    let (lines_cel, g) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    doc.cels.get_mut(&lines_cel).unwrap().raster = square();
    dot(&mut doc.cels.get_mut(&g).unwrap().raster, 25, 25, Color32::RED);
    let mut lines = Layer::new("Traits", LayerKind::Bitmap);
    lines.frames = vec![Some(lines_cel)];
    let mut ctg_layer = Layer::new("CTG", LayerKind::Bitmap);
    ctg_layer.ctg = Some(Ctg { sources: vec![lines.id], ..Ctg::default() });
    ctg_layer.frames = vec![Some(g)];
    doc.panels[0].layers = vec![ctg_layer, lines];
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert_eq!(plan.items[0].ctg, Some(ctg::CtgInput { sources: vec![lines_cel], gap: ctg::DEFAULT_GAP }));
    assert_eq!(crate::render::composite_pixel(&doc, &plan, 30, 30), Color32::RED);
    assert_eq!(crate::render::composite_pixel(&doc, &plan, 5, 5), Color32::TRANSPARENT);
}

#[test]
fn ctg_layers_are_saved_with_their_sources() {
    let mut doc = Document::new(64, 64, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut lines = Layer::new("Traits", LayerKind::Bitmap);
    lines.frames = vec![Some(cel)];
    let mut ctg_layer = Layer::new("CTG", LayerKind::Bitmap);
    ctg_layer.ctg = Some(Ctg { sources: vec![lines.id], ..Ctg::default() });
    let mut plain = Layer::new("CTG tous", LayerKind::Bitmap);
    plain.ctg = Some(Ctg::default());
    doc.panels[0].layers = vec![ctg_layer, lines, plain];
    let dir = std::env::temp_dir().join(format!("pelure-test-ctg-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("C.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = crate::project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    let l = &back.panels[0].layers;
    assert_eq!(l[0].ctg, Some(Ctg { sources: vec![l[1].id], ..Ctg::default() }), "la source retrouvée par son rang");
    assert_eq!(l[2].ctg, Some(Ctg::default()));
    assert_eq!(l[1].ctg, None);
}

#[test]
fn a_copied_plan_keeps_its_ctg_sources_on_the_copies() {
    let mut doc = Document::new(64, 64, 24);
    let lines = Layer::new("Traits", LayerKind::Bitmap);
    let mut ctg_layer = Layer::new("CTG", LayerKind::Bitmap);
    ctg_layer.ctg = Some(Ctg { sources: vec![lines.id], ..Ctg::default() });
    doc.panels[0].layers = vec![ctg_layer, lines];
    let at = doc.duplicate_panel_after(0);
    let copy = &doc.panels[at].layers;
    assert_eq!(copy[0].ctg, Some(Ctg { sources: vec![copy[1].id], ..Ctg::default() }));
}
