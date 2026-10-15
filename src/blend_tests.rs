//! Tests des modes de fusion (phases 8 et 9) : formules, composition d'un pixel, Fondu, rendu,
//! fusion de calques, enregistrement.

use eframe::egui::Color32;

use crate::blend::{blend_over, dissolve_noise, BlendMode};
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
    assert!(near(BlendMode::LinearBurn.mix(c, [1.0; 3]), c));
    assert!(near(BlendMode::Subtract.mix(c, [0.0; 3]), c));
    assert!(near(BlendMode::Divide.mix(c, [1.0; 3]), c));
    assert!(near(BlendMode::Difference.mix(c, c), [0.0; 3]));
    assert!(near(BlendMode::Normal.mix(c, [0.3; 3]), [0.3; 3]));
    // Produit et écran.
    assert!(near(BlendMode::Multiply.mix([0.5; 3], [0.5; 3]), [0.25; 3]));
    assert!(near(BlendMode::Screen.mix([0.5; 3], [0.5; 3]), [0.75; 3]));
    // Le gris moyen dessus ne change rien : incrustation, lumières tamisée, linéaire, ponctuelle.
    assert!(near(BlendMode::Overlay.mix(c, [0.5; 3]), c));
    assert!(near(BlendMode::SoftLight.mix(c, [0.5; 3]), c));
    assert!(near(BlendMode::LinearLight.mix(c, [0.5; 3]), c));
    assert!(near(BlendMode::PinLight.mix(c, [0.5; 3]), c));
    assert!(near(BlendMode::HardLight.mix([0.3; 3], [0.8; 3]), BlendMode::Overlay.mix([0.8; 3], [0.3; 3])));
    // Densités, lumière vive, mélange maximal, soustraction, division.
    assert!(near(BlendMode::Add.mix([0.7; 3], [0.6; 3]), [1.0; 3]));
    assert!(near(BlendMode::ColorDodge.mix([0.5; 3], [0.5; 3]), [1.0; 3]));
    assert!(near(BlendMode::ColorBurn.mix([0.5; 3], [0.5; 3]), [0.0; 3]));
    assert!(near(BlendMode::LinearBurn.mix([0.5; 3], [0.25; 3]), [0.0; 3]));
    assert!(near(BlendMode::VividLight.mix([0.5; 3], [0.25; 3]), BlendMode::ColorBurn.mix([0.5; 3], [0.5; 3])));
    assert!(near(BlendMode::HardMix.mix([0.4, 0.6, 0.2], [0.7; 3]), [1.0, 1.0, 0.0]));
    assert!(near(BlendMode::Subtract.mix([0.5; 3], [0.7; 3]), [0.0; 3]));
    assert!(near(BlendMode::Divide.mix([0.25; 3], [0.5; 3]), [0.5; 3]));
    // Couleur plus foncée / plus claire : la couleur entière, pas composante par composante.
    let (red, green) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert!(near(BlendMode::DarkerColor.mix(green, red), red), "le rouge est plus sombre");
    assert!(near(BlendMode::LighterColor.mix(green, red), green));
    // Luminosité : garde celle du calque ; couleur : garde celle du dessous.
    let grey = [0.5; 3];
    let l = |c: [f32; 3]| 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    assert!((l(BlendMode::Luminosity.mix(red, grey)) - 0.5).abs() < 1e-4);
    assert!((l(BlendMode::Color.mix(grey, red)) - 0.5).abs() < 1e-4);
    let s = BlendMode::Saturation.mix(grey, red);
    assert!((s[0] - s[1]).abs() < 1e-4 && (s[1] - s[2]).abs() < 1e-4, "un gris reste gris");
    // Numéros distincts, Normal = 0, Fondu = 1 (le shader en dépend) ; tous ont un nom.
    assert_eq!((BlendMode::Normal.index(), BlendMode::Dissolve.index()), (0, 1));
    let mut idx: Vec<u32> = BlendMode::ALL.iter().map(|m| m.index()).collect();
    idx.dedup();
    assert_eq!(idx.len(), 27);
    assert!(BlendMode::ALL.iter().all(|m| !m.label().is_empty() && !m.hint().is_empty()));
}

#[test]
fn pixels() {
    let white = Color32::WHITE;
    let red = Color32::from_rgb(255, 0, 0);
    let at = (3, 4);
    // Produit : rouge sur blanc = rouge ; sur gris = rouge sombre.
    assert_eq!(blend_over(white, red, 255, None, BlendMode::Multiply, at), red);
    let grey = Color32::from_rgb(128, 128, 128);
    assert_eq!(blend_over(grey, red, 255, None, BlendMode::Multiply, at), Color32::from_rgb(128, 0, 0));
    // Sur du transparent : comme en mode normal (la norme le veut).
    let over = render::over(Color32::TRANSPARENT, red, 200, None);
    let got = blend_over(Color32::TRANSPARENT, red, 200, None, BlendMode::Multiply, at);
    assert!(got.to_array().iter().zip(over.to_array()).all(|(a, b)| a.abs_diff(b) <= 1), "{got:?} / {over:?}");
    // Opacité nulle : rien ; demi-opacité : à mi-chemin.
    assert_eq!(blend_over(grey, red, 0, None, BlendMode::Multiply, at), grey);
    let half = blend_over(white, Color32::BLACK, 128, None, BlendMode::Multiply, at);
    assert!(half.r().abs_diff(127) <= 1 && half.a() == 255);
    // Pelure d'oignon (teinte) : toujours normale.
    assert_eq!(
        blend_over(grey, red, 255, Some([0, 0, 255]), BlendMode::Multiply, at),
        render::over(grey, red, 255, Some([0, 0, 255]))
    );
}

#[test]
fn dissolve() {
    // Le bruit : entre 0 et 1, le même à chaque fois, réparti à peu près également.
    let n: Vec<f32> = (0..64).flat_map(|y| (0..64).map(move |x| dissolve_noise(x, y))).collect();
    assert!(n.iter().all(|v| (0.0..1.0).contains(v)));
    assert_eq!(dissolve_noise(10, 20), dissolve_noise(10, 20));
    let below_half = n.iter().filter(|v| **v < 0.5).count();
    assert!((1800..2300).contains(&below_half), "{below_half}");
    // Un calque à 30 % : à peu près 30 % de points pleins, le reste laisse le fond.
    let src = Color32::from_rgb(200, 0, 0);
    let (mut full, mut kept) = (0, 0);
    for y in 0..64 {
        for x in 0..64 {
            match blend_over(Color32::WHITE, src, 77, None, BlendMode::Dissolve, (x, y)) {
                c if c == src => full += 1,
                c if c == Color32::WHITE => kept += 1,
                c => panic!("ni l'un ni l'autre : {c:?}"),
            }
        }
    }
    assert_eq!(full + kept, 4096);
    assert!((1000..1450).contains(&full), "{full}");
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
    let mut doc = doc();
    doc.panels[0].layers[0].blend = BlendMode::PinLight;
    let dir = std::env::temp_dir().join(format!("pelure-test-fusion-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"fusion\": \"produit\"") && text.contains("\"fusion\": \"lumiere_ponctuelle\""));
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.panels[0].layers[1].blend, BlendMode::Multiply);
    assert_eq!(back.panels[0].layers[0].blend, BlendMode::PinLight);
}
