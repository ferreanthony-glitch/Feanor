//! Feuille flottante déformée par un maillage (`warp.rs`) : enveloppe et perspective sans rejeu du
//! moteur ni pixel refait pendant le geste (Anthony, 2026-10-18 : « l'outil Transformer en mode
//! Enveloppe rame encore »). Ce que montre le maillage est ce que donnait la déformation exacte de
//! la feuille, et un trait qui dépasse de la feuille se montre en entier.

use eframe::egui::{pos2, vec2, Color32, Rect};

use crate::affichage_tests::{bord_haut_gauche, couverture_differente, doc_with, gros_traits, line_item};
use crate::envelope::{self, Envelope};
use crate::flottante::{Flottante, Pending, BASE_CEL, SHEET_CEL};
use crate::persp;
use crate::render::{self, Plan};
use crate::vector::{Affine, Element};
use crate::warp::Warp;

fn bounds(elements: &[Element]) -> Rect {
    elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()))
}

/// Une enveloppe qui courbe : un coin tiré, une poignée poussée.
fn enveloppe_courbe(src: Rect) -> Envelope {
    let mut env = Envelope::identity(src);
    env.move_corner(2, vec2(-90.0, 60.0));
    env.move_corner(0, vec2(40.0, 25.0));
    env.handles[1] += vec2(0.0, 70.0);
    env.handles[4] += vec2(-50.0, 30.0);
    env
}

/// Le plan de la feuille flottante au repos dans `doc` (le dessin sans la sélection + sa feuille).
fn flottant(f: &Flottante, id: crate::document::CelId) -> Plan {
    Plan { paper: None, items: f.items(&line_item(id)).to_vec() }
}

#[test]
fn the_mesh_covers_the_region_and_remembers_the_sheet_margin() {
    let src = Rect::from_min_max(pos2(100.0, 50.0), pos2(500.0, 250.0));
    let w = Warp::new(src, (30, 20), 8, |p| p + vec2(7.0, -3.0)).expect("maillage");
    assert_eq!(w.from.len(), (w.n + 1) * (w.n + 1));
    assert_eq!(w.to.len(), w.from.len());
    assert!((8..=48).contains(&w.n));
    // Le premier point : le coin de la région (un peu élargie), dans l'image de la feuille.
    assert_eq!(w.from[0], pos2(98.0 + 30.0, 48.0 + 20.0));
    assert_eq!(w.to[0], pos2(98.0 + 7.0, 48.0 - 3.0));
    // Une région vide ou infinie : pas de maillage.
    assert!(Warp::new(Rect::NOTHING, (0, 0), 8, |p| p).is_none());
    assert!(Warp::new(Rect::from_min_max(pos2(0.0, 0.0), pos2(f32::INFINITY, 4.0)), (0, 0), 8, |p| p).is_none());
    // Plus la région est grande, plus il y a de cases, dans une limite.
    let big = Warp::new(Rect::from_min_max(pos2(0.0, 0.0), pos2(9000.0, 100.0)), (0, 0), 8, |p| p).unwrap();
    assert_eq!(big.n, 48);
    let wide = Warp::new(src, (0, 0), 40, |p| p).unwrap();
    assert_eq!(wide.n, 40, "la perspective demande plus de cases");
}

#[test]
fn the_gpu_gets_three_vertices_per_triangle_with_normalised_positions_and_texture_coordinates() {
    let src = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 100.0));
    let w = Warp::new(src, (16, 8), 8, |p| p).unwrap();
    // La feuille fait 232 × 116 (marge 16 et 8 de chaque côté, 200 × 100 de dessin).
    let v = w.gpu_vertices((232, 116), None, |p| [p.x / 200.0 * 2.0 - 1.0, 1.0 - p.y / 100.0 * 2.0]);
    assert_eq!(v.len(), w.n * w.n * 6);
    // Le premier sommet : le point (−2, −2) du dessin, le pixel (14, 6) de la feuille.
    assert!((v[0][0] - (-2.0 / 200.0 * 2.0 - 1.0)).abs() < 1e-6 && (v[0][1] - (1.0 + 2.0 / 100.0 * 2.0)).abs() < 1e-6, "{:?}", v[0]);
    assert!((v[0][2] - 14.0 / 232.0).abs() < 1e-6 && (v[0][3] - 6.0 / 116.0).abs() < 1e-6, "{:?}", v[0]);
    // La transformation du calque s'applique aux points d'arrivée, pas à la texture.
    let layer = Affine::translate(vec2(20.0, 10.0));
    let moved = w.gpu_vertices((232, 116), Some(&layer), |p| [p.x, p.y]);
    assert_eq!((moved[0][0], moved[0][1]), (18.0, 8.0));
    assert_eq!((moved[0][2], moved[0][3]), (v[0][2], v[0][3]));
}

#[test]
fn an_identity_mesh_lays_the_sheet_where_it_was() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(12, w, h, 4);
    let src = bounds(&elements);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (0..n).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    assert!(f.set_warp(src, 8, |p| p));
    let floating = render::composite_full(&doc, &flottant(&f, id));
    let rest = render::composite_full(&doc, &Plan { paper: None, items: vec![line_item(id)] });
    let far = floating.iter().zip(&rest).filter(|(a, b)| a.to_array().iter().zip(b.to_array()).any(|(x, y)| (*x as i32 - y as i32).abs() > 2)).count();
    let painted = rest.iter().filter(|c| c.a() > 0).count();
    // (Les traits qui dépassent de la feuille sont coupés par le dessin au repos, pas par la feuille.)
    assert!(far * 200 < painted, "{far} pixels sur {painted} diffèrent de plus de 2 niveaux");
}

#[test]
fn an_envelope_mesh_shows_what_the_exact_warp_of_the_sheet_gives() {
    // Des traits entiers dans la feuille : la feuille a la taille du dessin.
    let (w, h) = (1920, 1080);
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    let s = lib.presets.iter().find(|p| p.name == "Gouache").unwrap().settings;
    let elements: Vec<Element> = (0..8)
        .map(|k| {
            let pts: Vec<_> = (0..=60).map(|i| (pos2(500.0 + i as f32 * 12.0, 300.0 + k as f32 * 70.0 + (i as f32 * 0.2).sin() * 30.0), 0.7, i as f64 / 200.0)).collect();
            crate::affichage_tests::trait_rouge(&s, &lib, w, h, &pts)
        })
        .collect();
    let src = bounds(&elements);
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (0..8).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    assert_eq!(f.margin, (0, 0));
    let env = enveloppe_courbe(src);
    assert!(f.set_warp(src, 8, |p| env.map(src, p)));
    let floating = render::composite_full(&doc, &flottant(&f, id));
    // La déformation exacte (bilinéaire) de la même feuille.
    let sheet = &doc.cels[&SHEET_CEL].raster;
    let exact = envelope::warp_raster(sheet, src, &env);
    let mut reference = vec![Color32::TRANSPARENT; w * h];
    for y in 0..h {
        for x in 0..w {
            reference[y * w + x] = exact.get(x, y);
        }
    }
    let n = couverture_differente(&floating, &reference);
    let painted = reference.iter().filter(|c| c.a() >= 128).count();
    println!("enveloppe : {n} pixels sur {painted} peints diffèrent de la déformation exacte");
    assert!(painted > 20_000);
    assert!(n * 100 < painted * 3, "{n} pixels sur {painted} diffèrent");
}

#[test]
fn a_perspective_mesh_shows_what_the_exact_warp_of_the_sheet_gives() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(10, w, h, 12);
    // (Dans la feuille : les traits qui dépassent ont leur marge, vérifiée plus bas.)
    let src = Rect::from_min_max(pos2(300.0, 200.0), pos2(1500.0, 900.0));
    let elements: Vec<Element> = elements
        .into_iter()
        .map(|e| {
            let b = e.bounds();
            let (cx, cy) = ((b.center().x).clamp(src.min.x + b.width() / 2.0, src.max.x - b.width() / 2.0), b.center().y.clamp(src.min.y + b.height() / 2.0, src.max.y - b.height() / 2.0));
            e.transformed(&Affine::translate(vec2(cx - b.center().x, cy - b.center().y)))
        })
        .collect();
    let src = bounds(&elements);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (0..n).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    let quad = [src.left_top() + vec2(60.0, 30.0), src.right_top() + vec2(-40.0, 0.0), src.right_bottom() + vec2(30.0, 40.0), src.left_bottom() + vec2(-20.0, -50.0)];
    let hom = persp::rect_to_quad(src, quad).expect("homographie");
    assert!(f.set_warp(src, 24, |p| persp::apply(&hom, p)));
    let floating = render::composite_full(&doc, &flottant(&f, id));
    let exact = persp::warp_raster(&doc.cels[&SHEET_CEL].raster, src, quad).expect("déformation");
    let mut reference = vec![Color32::TRANSPARENT; w * h];
    for y in 0..h {
        for x in 0..w {
            reference[y * w + x] = exact.get(x, y);
        }
    }
    let diff = couverture_differente(&floating, &reference);
    let painted = reference.iter().filter(|c| c.a() >= 128).count();
    println!("perspective : {diff} pixels sur {painted} peints diffèrent de la déformation exacte");
    assert!(painted > 20_000);
    assert!(diff * 100 < painted * 3, "{diff} pixels sur {painted} diffèrent");
}

#[test]
fn strokes_crossing_the_sheet_edge_show_their_outside_part_when_the_envelope_pulls_them_in() {
    let (mut doc, id) = doc_with(1920, 1080, bord_haut_gauche());
    let items: Vec<usize> = (0..4).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    assert!(f.dims.is_some() && f.margin.0 > 0 && f.margin.1 > 0, "la feuille garde ce qui dépasse en haut et à gauche");
    let src = bounds(&doc.cels[&id].vector.as_ref().unwrap().elements);
    assert!(src.min.x < 0.0 && src.min.y < 0.0);
    // Une enveloppe qui ne fait que déplacer : le dessin refait à sa nouvelle place est la référence.
    let d = vec2(160.0, 140.0);
    let mut env = Envelope::identity(src);
    env.translate(d);
    assert!(f.set_warp(src, 8, |p| env.map(src, p)));
    let floating = render::composite_full(&doc, &flottant(&f, id));
    Flottante::release(&mut doc);
    let cel = doc.cels.get_mut(&id).unwrap();
    for e in cel.vector.as_mut().unwrap().elements.iter_mut() {
        *e = e.transformed(&Affine::translate(d));
    }
    cel.rerender();
    let exact = render::composite_full(&doc, &Plan { paper: None, items: vec![line_item(id)] });
    let n = couverture_differente(&floating, &exact);
    let painted = exact.iter().filter(|c| c.a() >= 128).count();
    println!("débord : {n} pixels sur {painted} peints diffèrent");
    assert!(n * 100 < painted * 6, "{n} pixels sur {painted} diffèrent (morceaux manquants)");
}

#[test]
fn the_layer_transform_applies_after_the_mesh() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(6, w, h, 31);
    let src = bounds(&elements);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (0..n).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    assert!(f.set_warp(src, 8, |p| p + vec2(40.0, 20.0)));
    let mut plan = flottant(&f, id);
    let t = crate::document::LayerTransform { x: 60.0, y: -30.0, ..Default::default() };
    for it in &mut plan.items {
        it.transform = Some(t);
    }
    let with_layer = render::composite_full(&doc, &plan);
    // Le même décalage du calque, avec la feuille posée à son endroit sans maillage.
    let mut f2 = Flottante::lift(&mut doc, id, &items).expect("feuille");
    f2.aff = Affine::translate(vec2(40.0, 20.0));
    let mut plan2 = flottant(&f2, id);
    for it in &mut plan2.items {
        it.transform = Some(t);
    }
    let reference = render::composite_full(&doc, &plan2);
    let n = couverture_differente(&with_layer, &reference);
    let painted = reference.iter().filter(|c| c.a() >= 128).count();
    assert!(painted > 5_000 && n * 100 < painted * 3, "{n} pixels sur {painted} diffèrent");
}

#[test]
fn finishing_a_released_warp_early_gives_the_image_the_screen_was_showing() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(10, w, h, 8);
    let src = bounds(&elements);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (0..n).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    let env = enveloppe_courbe(src);
    assert!(f.set_warp(src, 8, |p| env.map(src, p)));
    f.pending = Some(Pending { items: Vec::new(), originals: Vec::new() });
    let shown = render::composite_full(&doc, &flottant(&f, id));
    let warp = f.warp.clone().expect("maillage");
    let tiles = Flottante::flatten_warp(&mut doc, id, &warp);
    Flottante::release(&mut doc);
    assert!(!tiles.is_empty());
    assert!(!doc.cels.contains_key(&BASE_CEL) && !doc.cels.contains_key(&SHEET_CEL));
    let flat = render::composite_full(&doc, &Plan { paper: None, items: vec![line_item(id)] });
    let far = shown.iter().zip(&flat).filter(|(a, b)| a.to_array().iter().zip(b.to_array()).any(|(x, y)| (*x as i32 - y as i32).abs() > 2)).count();
    assert_eq!(far, 0, "{far} pixels diffèrent de ce que montrait l'écran");
}

#[test]
fn a_session_lifts_the_original_drawing_not_the_geometry_already_posed_in_the_document() {
    let (w, h) = (960, 540);
    let elements = gros_traits(10, w, h, 5);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let original = doc.cels[&id].vector.clone().unwrap();
    let items: Vec<usize> = (0..n).collect();
    let _ = Flottante::lift(&mut doc, id, &items).expect("feuille");
    let first = doc.cels[&SHEET_CEL].raster.to_flat();
    Flottante::release(&mut doc);
    // Un premier geste a posé sa géométrie dans le dessin du document (sans pixels en cache).
    let cel = doc.cels.get_mut(&id).unwrap();
    for e in cel.vector.as_mut().unwrap().elements.iter_mut() {
        *e = e.transformed(&Affine::translate(vec2(120.0, 60.0)));
    }
    cel.rerender();
    let f = Flottante::lift_from(&mut doc, id, &original, &items).expect("feuille");
    assert!(f.for_warp && f.warp.is_none());
    assert!(doc.cels[&SHEET_CEL].raster.to_flat() == first, "la feuille est celle du dessin d'origine");
    Flottante::release(&mut doc);
}

#[test]
fn a_plan_with_the_same_mesh_is_the_same_plan() {
    let src = Rect::from_min_max(pos2(10.0, 10.0), pos2(300.0, 200.0));
    let a = Warp::new(src, (0, 0), 8, |p| p + vec2(3.0, 0.0)).unwrap();
    let b = Warp::new(src, (0, 0), 8, |p| p + vec2(3.0, 0.0)).unwrap();
    let c = Warp::new(src, (0, 0), 8, |p| p + vec2(4.0, 0.0)).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}
