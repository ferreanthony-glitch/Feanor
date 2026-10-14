//! Tests des traits vectoriels texturés (7b) : clés de contenu des textures, réglage d'un trait,
//! rendu avec le grain, transformations, enregistrement des textures avec le projet.

use crate::brush::BrushSettings;
use crate::brushtex::{self, BrushLibrary, GrayImage, TextureKind, PAPER_COARSE};
use crate::document::{Document, LayerKind};
use crate::project;
use crate::raster::Raster;
use crate::vector::{Affine, Element, VPoint, VectorDrawing};

#[test]
fn content_keys() {
    let a = GrayImage::new(2, 2, vec![0.0, 0.5, 1.0, 0.25]);
    let b = GrayImage::new(2, 2, vec![0.0, 0.5, 1.0, 0.27]);
    let key = brushtex::content_key(TextureKind::Tip, &a);
    assert_eq!(key, brushtex::content_key(TextureKind::Tip, &a.clone()));
    assert_ne!(key, brushtex::content_key(TextureKind::Tip, &b));
    assert_ne!(key, brushtex::content_key(TextureKind::Paper, &a));
    assert_ne!(key & 0x8000_0000, 0, "jamais un numéro de la bibliothèque");
}

#[test]
fn library_settings_for_vector() {
    let lib = BrushLibrary::builtin();
    lib.share();
    assert!(lib.for_vector(&BrushSettings::brush()).is_none(), "sans texture : trait lisse");
    let chalk = BrushSettings { paper: Some(PAPER_COARSE), grain: 1.0, ..BrushSettings::brush() };
    let v = lib.for_vector(&chalk).expect("texturé");
    let key = v.paper.expect("papier");
    assert_ne!(key & 0x8000_0000, 0);
    assert!(brushtex::shared_textures(&v).paper.is_some());
    assert!(brushtex::shared_image(TextureKind::Paper, key).is_some());
    // Un grain nul ne compte pas.
    assert!(lib.for_vector(&BrushSettings { grain: 0.0, ..chalk }).is_none());
}

fn chalk_stroke(lib: &BrushLibrary) -> Element {
    let s = BrushSettings { size: 16.0, paper: Some(PAPER_COARSE), grain: 1.0, spacing: 0.05, ..BrushSettings::brush() };
    Element::Stroke {
        points: (0..=20).map(|k| VPoint { x: 20.0 + k as f32 * 6.0, y: 40.0, r: 8.0 }).collect(),
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 0.8,
        brush: lib.for_vector(&s).map(Box::new),
    }
}

fn render(e: &Element) -> Raster {
    VectorDrawing { elements: vec![e.clone()] }.render(200, 80)
}

/// Pixels pleins sur la ligne médiane du trait.
fn full(r: &Raster) -> usize {
    (24..136).filter(|&x| r.get(x, 40).a() >= 250).count()
}

#[test]
fn textured_strokes_render_with_grain() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let textured = chalk_stroke(&lib);
    let mut smooth = textured.clone();
    if let Element::Stroke { brush, .. } = &mut smooth {
        *brush = None;
    }
    let (a, b) = (render(&textured), render(&smooth));
    assert_eq!(full(&b), 112, "trait lisse : plein");
    assert!(full(&a) < 100, "le grain laisse des creux ({})", full(&a));
    let inked = (24..136).filter(|&x| a.get(x, 40).a() > 0).count();
    assert!(inked > 60, "mais le trait est là ({inked})");
    assert_eq!(render(&textured).to_rgba(), a.to_rgba(), "le même à chaque rendu");
    // Rendu d'une zone seulement (gomme, sélection) : comme le rendu complet dans la zone.
    let mut part = Raster::new(200, 80);
    let zone = eframe::egui::Rect::from_min_max(eframe::egui::pos2(40.0, 0.0), eframe::egui::pos2(90.0, 80.0));
    crate::vector::render_element_clipped(&mut part, &textured, zone);
    for x in 40..90 {
        assert_eq!(part.get(x, 40), a.get(x, 40), "x = {x}");
    }
    assert_eq!(part.get(30, 40).a(), 0, "rien hors de la zone");
}

#[test]
fn transforms_keep_the_brush_in_scale() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let e = chalk_stroke(&lib);
    let big = e.transformed(&Affine::scale_about(eframe::egui::pos2(0.0, 0.0), 2.0, 2.0));
    let Element::Stroke { points, brush: Some(b), .. } = &big else { panic!() };
    assert_eq!(points[0].r, 16.0);
    assert_eq!(b.size, 32.0, "la taille du pinceau suit, la pression se retrouve");
    let mut thin = e.clone();
    thin.scale_width(0.5);
    let Element::Stroke { brush: Some(b), .. } = &thin else { panic!() };
    assert_eq!(b.size, 8.0);
    // La gomme coupe le trait : les morceaux gardent leur pinceau.
    let mut d = VectorDrawing { elements: vec![e] };
    assert!(crate::vector::erase_circle(&mut d, eframe::egui::pos2(80.0, 40.0), 6.0, false));
    assert_eq!(d.elements.len(), 2);
    assert!(d.elements.iter().all(|p| matches!(p, Element::Stroke { brush: Some(_), .. })));
}

#[test]
fn textures_travel_with_the_project() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let stroke = chalk_stroke(&lib);
    let Element::Stroke { brush: Some(b), .. } = &stroke else { panic!() };
    let key = b.paper.unwrap();
    let mut doc = Document::new(200, 80, 24);
    let cel = doc.new_cel(LayerKind::Vector);
    let c = doc.cels.get_mut(&cel).unwrap();
    c.vector.as_mut().unwrap().elements.push(stroke.clone());
    c.rerender();
    let before = c.raster.to_rgba();
    doc.panels[0].layers[0].set(0, Some(cel));

    let dir = std::env::temp_dir().join(format!("pelure-test-textures-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let file = dir.join("textures").join(format!("papier_{key}.png"));
    assert!(file.exists(), "la texture est dans le projet");
    let back = project::load(&manifest).unwrap().doc;
    let id = back.panels[0].layers[0].cel_at(0).unwrap();
    let c = &back.cels[&id];
    assert!(matches!(&c.vector.as_ref().unwrap().elements[0], Element::Stroke { brush: Some(b), .. } if b.paper == Some(key)));
    assert_eq!(c.raster.to_rgba(), before, "même rendu à la réouverture");
    // Plus utilisée : effacée au prochain enregistrement.
    let mut doc = back;
    doc.panels[0].layers[0].set(0, None);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!file.exists());
    std::fs::remove_dir_all(&dir).ok();
}
