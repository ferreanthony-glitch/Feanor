//! Tests des modes de fusion (phase 8) : formules, composition d'un pixel, rendu, fusion de
//! calques, enregistrement.

use eframe::egui::Color32;

use crate::blend::{blend_over, BlendMode};
use crate::document::{Document, LayerKind};
use crate::project;
use crate::render;

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn formulas() {
    let c = [0.2, 0.5, 0.9];
    // Neutres : le blanc en Produit, le noir en Superposition (écran), Addition, Éclaircir.
    assert!(near(BlendMode::Multiply.mix(c, [1.0; 3]), c));
    assert!(near(BlendMode::Screen.mix(c, [0.0; 3]), c));
    assert!(near(BlendMode::Add.mix(c, [0.0; 3]), c));
    assert!(near(BlendMode::Lighten.mix(c, [0.0; 3]), c));
    assert!(near(BlendMode::Darken.mix(c, [1.0; 3]), c));
    assert!(near(BlendMode::Difference.mix(c, c), [0.0; 3]));
    assert!(near(BlendMode::Normal.mix(c, [0.3; 3]), [0.3; 3]));
    // Produit et écran.
    assert!(near(BlendMode::Multiply.mix([0.5; 3], [0.5; 3]), [0.25; 3]));
    assert!(near(BlendMode::Screen.mix([0.5; 3], [0.5; 3]), [0.75; 3]));
    // Incrustation : le gris moyen dessus ne change rien ; lumière crue = incrustation inversée.
    assert!(near(BlendMode::Overlay.mix(c, [0.5; 3]), c));
    assert!(near(BlendMode::HardLight.mix([0.3; 3], [0.8; 3]), BlendMode::Overlay.mix([0.8; 3], [0.3; 3])));
    assert!(near(BlendMode::SoftLight.mix(c, [0.5; 3]), c));
    // Addition plafonnée, densités.
    assert!(near(BlendMode::Add.mix([0.7; 3], [0.6; 3]), [1.0; 3]));
    assert!(near(BlendMode::ColorDodge.mix([0.5; 3], [0.5; 3]), [1.0; 3]));
    assert!(near(BlendMode::ColorBurn.mix([0.5; 3], [0.5; 3]), [0.0; 3]));
    // Luminosité : garde celle du calque ; couleur : garde celle du dessous.
    let red = [1.0, 0.0, 0.0];
    let grey = [0.5; 3];
    let l = |c: [f32; 3]| 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    assert!((l(BlendMode::Luminosity.mix(red, grey)) - 0.5).abs() < 1e-4);
    assert!((l(BlendMode::Color.mix(grey, red)) - 0.5).abs() < 1e-4);
    // Saturation d'un gris : reste gris.
    let s = BlendMode::Saturation.mix(grey, red);
    assert!((s[0] - s[1]).abs() < 1e-4 && (s[1] - s[2]).abs() < 1e-4);
    // Chaque mode a un numéro distinct, Normal = 0.
    assert_eq!(BlendMode::Normal.index(), 0);
    let mut idx: Vec<u32> = BlendMode::ALL.iter().map(|m| m.index()).collect();
    idx.dedup();
    assert_eq!(idx.len(), BlendMode::ALL.len());
}

#[test]
fn pixels() {
    let white = Color32::WHITE;
    let red = Color32::from_rgb(255, 0, 0);
    // Produit : rouge sur blanc = rouge ; sur gris = rouge sombre.
    assert_eq!(blend_over(white, red, 255, None, BlendMode::Multiply), red);
    let grey = Color32::from_rgb(128, 128, 128);
    assert_eq!(blend_over(grey, red, 255, None, BlendMode::Multiply), Color32::from_rgb(128, 0, 0));
    // Sur du transparent : comme en mode normal (la norme le veut).
    let over = render::over(Color32::TRANSPARENT, red, 200, None);
    let got = blend_over(Color32::TRANSPARENT, red, 200, None, BlendMode::Multiply);
    assert!(got.to_array().iter().zip(over.to_array()).all(|(a, b)| a.abs_diff(b) <= 1), "{got:?} / {over:?}");
    // Opacité nulle : rien ; demi-opacité : à mi-chemin.
    assert_eq!(blend_over(grey, red, 0, None, BlendMode::Multiply), grey);
    let half = blend_over(white, Color32::BLACK, 128, None, BlendMode::Multiply);
    assert!(half.r().abs_diff(127) <= 1 && half.a() == 255);
    // Pelure d'oignon (teinte) : toujours normale.
    assert_eq!(
        blend_over(grey, red, 255, Some([0, 0, 255]), BlendMode::Multiply),
        render::over(grey, red, 255, Some([0, 0, 255]))
    );
}

/// Un projet : calque du dessous gris plein, calque du dessus rouge plein en Produit.
fn doc() -> Document {
    let mut doc = Document::new(64, 64, 24);
    let (a, b) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    for (cel, color) in [(a, Color32::from_rgb(128, 128, 128)), (b, Color32::from_rgb(255, 0, 0))] {
        let c = doc.cels.get_mut(&cel).unwrap();
        for idx in 0..c.raster.tiles.len() {
            c.raster.tile_mut(idx).fill(color);
        }
    }
    let p = &mut doc.panels[0];
    p.layers[0] = crate::document::Layer::new("Fond", LayerKind::Bitmap);
    p.layers[0].set(0, Some(a));
    let mut top = crate::document::Layer::new("Ombre", LayerKind::Bitmap);
    top.set(0, Some(b));
    top.blend = BlendMode::Multiply;
    p.layers.push(top);
    doc
}

#[test]
fn render_and_merge() {
    let doc = doc();
    let plan = render::panel_plan_at(&doc, 0, 0, Some([255, 255, 255]));
    assert_eq!(plan.items[1].blend, BlendMode::Multiply);
    assert_eq!(render::composite_pixel(&doc, &plan, 10, 10), Color32::from_rgb(128, 0, 0));
    assert_eq!(render::composite_full(&doc, &plan)[64 * 20 + 20], Color32::from_rgb(128, 0, 0));
    // Fusionner : le mode s'applique aux pixels ; le calque obtenu est normal.
    let mut doc = doc;
    doc.merge_layer_down(0, 1).unwrap();
    let layer = &doc.panels[0].layers[0];
    assert_eq!(layer.blend, BlendMode::Normal);
    let cel = &doc.cels[&layer.cel_at(0).unwrap()];
    assert_eq!(cel.raster.get(5, 5), Color32::from_rgb(128, 0, 0));
}

#[test]
fn modes_are_saved() {
    let doc = doc();
    let dir = std::env::temp_dir().join(format!("pelure-test-fusion-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"fusion\": \"produit\""));
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.panels[0].layers[1].blend, BlendMode::Multiply);
    assert_eq!(back.panels[0].layers[0].blend, BlendMode::Normal);
}
