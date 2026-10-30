//! Tests des pinceaux texturés et de la bibliothèque de pinceaux (phase 3d-2).

use std::sync::Arc;

use eframe::egui::pos2;

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::brushtex::{self, BrushLibrary, BrushTextures, GrayImage, Target, Tip};
use crate::raster::Raster;

/// Un papier importé est réparti par rang (audit du rendu) : les valeurs égales prennent le même
/// rang moyen, un papier uniforme reste uniforme, et `add_paper` s'en sert.
#[test]
fn imported_papers_are_rank_equalized() {
    let g = GrayImage::new(2, 2, vec![0.2, 0.9, 0.9, 0.5]).equalized();
    let want = [0.0, 2.5 / 3.0, 2.5 / 3.0, 1.0 / 3.0];
    for (v, w) in g.values.iter().zip(want) {
        assert!((v - w).abs() < 1e-6, "{:?}", g.values);
    }
    let flat = GrayImage::new(3, 1, vec![0.4; 3]).equalized();
    assert!(flat.values.iter().all(|v| (v - 0.5).abs() < 1e-6));
    // Un papier « photographié » : presque tout dans les gris moyens. Réparti, un seuil à 0,3
    // laisse 30 % de creux, comme pour un papier fabriqué.
    let n = 1000;
    let photo = GrayImage::new(n, 1, (0..n).map(|i| 0.45 + 0.1 * (i as f32 / n as f32)).collect());
    let mut lib = BrushLibrary::builtin();
    let id = lib.add_paper("photo", photo);
    let paper = lib.textures(&BrushSettings { paper: Some(id), grain: 1.0, ..BrushSettings::brush() }).paper.unwrap();
    let below = paper.values.iter().filter(|&&v| v < 0.3).count();
    assert!((below as f32 / n as f32 - 0.3).abs() < 0.01, "{below} creux sur {n}");
}

#[test]
fn grain_depends_on_paper_and_pressure() {
    let old = brushtex::GRAIN_SOFTNESS_OLD;
    assert_eq!(brushtex::grain_factor(0.0, 1.0, 0.0, old), 1.0, "papier lisse");
    // Sommets avant creux ; appuyer remplit.
    assert!(brushtex::grain_factor(0.9, 0.5, 1.0, old) > brushtex::grain_factor(0.2, 0.5, 1.0, old));
    assert!(brushtex::grain_factor(0.3, 1.0, 1.0, old) > brushtex::grain_factor(0.3, 0.2, 1.0, old));
    assert!(brushtex::grain_factor(0.0, 1.0, 1.0, old) < 0.2, "les creux profonds restent vides");
    // Douceur (rendu, étape 2) : à mi-appui, le seuil est à 0,45 ; net, un relief de 0,35 est
    // vide et 0,55 plein ; doux, les deux sont à mi-chemin.
    let (soft, hard) = (brushtex::GRAIN_SOFTNESS_SOFT, 0.05);
    assert!(brushtex::grain_factor(0.35, 0.5, 1.0, hard) < 0.05 && brushtex::grain_factor(0.55, 0.5, 1.0, hard) > 0.95);
    assert!(brushtex::grain_factor(0.35, 0.5, 1.0, soft) > 0.2 && brushtex::grain_factor(0.55, 0.5, 1.0, soft) < 0.8);
    let p = brushtex::procedural_paper(64, 8, 3);
    let (lo, hi) = p.values.iter().fold((1.0f32, 0.0f32), |(a, b), &v| (a.min(v), b.max(v)));
    assert!(lo == 0.0 && hi == 1.0);
}

/// Un trait horizontal au milieu d'un dessin de 128 × 64 ; renvoie le dessin.
fn stroke(settings: BrushSettings, textures: BrushTextures, pressure: f32) -> Raster {
    let mut raster = Raster::new(128, 64);
    let mut s = StrokeState::new(1, StrokeMode::Paint([0, 0, 0]), settings, None, true).with_textures(textures);
    let mut dirty = Vec::new();
    for i in 0..=40 {
        s.add_point(&mut raster, pos2(20.0 + i as f32 * 2.0, 32.0), pressure, &mut dirty);
    }
    s.finish(&mut raster, &mut dirty);
    raster
}

fn holes_in_middle(r: &Raster) -> usize {
    (40..90).flat_map(|x| (29..36).map(move |y| (x, y))).filter(|&(x, y)| r.get(x, y).a() < 128).count()
}

#[test]
fn paper_grain_leaves_holes_and_pressure_fills_them() {
    let lib = BrushLibrary::builtin();
    let s = BrushSettings {
        size: 14.0,
        hardness: 1.0,
        smoothing: 0.0,
        pressure_size: false,
        grain: 1.0,
        paper: Some(brushtex::PAPER_COARSE),
        ..BrushSettings::brush()
    };
    assert_eq!(holes_in_middle(&stroke(BrushSettings { grain: 0.0, ..s }, lib.textures(&s), 1.0)), 0, "sans grain : plein");
    let soft = holes_in_middle(&stroke(s, lib.textures(&s), 0.3));
    let hard = holes_in_middle(&stroke(s, lib.textures(&s), 1.0));
    assert!(soft > 0, "le grain laisse des creux");
    assert!(soft > hard, "appuyer remplit les creux ({soft} contre {hard})");
}

#[test]
fn image_tip_shapes_the_dab() {
    // Pointe en anneau : rien au centre.
    let n = 32;
    let ring = GrayImage::new(
        n,
        n,
        (0..n * n)
            .map(|i| {
                let (x, y) = ((i % n) as f32 + 0.5 - 16.0, (i / n) as f32 + 0.5 - 16.0);
                let d = (x * x + y * y).sqrt();
                if (10.0..15.0).contains(&d) { 1.0 } else { 0.0 }
            })
            .collect(),
    );
    let tip = Arc::new(Tip::new(ring));
    let s = BrushSettings { size: 32.0, smoothing: 0.0, pressure_size: false, ..BrushSettings::brush() };
    let mut raster = Raster::new(64, 64);
    let mut st = StrokeState::new(1, StrokeMode::Paint([0, 0, 0]), s, None, false)
        .with_textures(BrushTextures { tip: Some(tip.clone()), paper: None });
    let mut dirty = Vec::new();
    st.add_point(&mut raster, pos2(32.0, 32.0), 1.0, &mut dirty);
    assert_eq!(raster.get(32, 32).a(), 0, "centre vide");
    assert!(raster.get(32 + 12, 32).a() > 200, "anneau peint");
    // Petite pointe : une réduction de l'image, pas l'image entière.
    assert!(tip.level(6.0).width >= 6 && tip.level(6.0).width < 16);
}

#[test]
fn imported_brushes_are_kept() {
    let dir = std::env::temp_dir().join(format!("pelure-test-pinceaux-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut lib = BrushLibrary::builtin();
    let tip = lib.add_tip("Pointe", GrayImage::new(8, 8, (0..64).map(|i| if (3..5).contains(&(i % 8)) && (3..5).contains(&(i / 8)) { 1.0 } else { 0.0 }).collect()));
    let paper = lib.add_paper("Papier", GrayImage::new(6, 6, (0..36).map(|i| ((i % 6 + i / 6) * 40) as f32 / 255.0).collect()));
    let settings = BrushSettings { tip: Some(tip), paper: Some(paper), grain: 1.0, ..BrushSettings::brush() };
    lib.save_mine("CRAYON 01", Target::Brush, settings);
    lib.save(&dir).unwrap();
    let back = BrushLibrary::load(Some(&dir));
    assert_eq!(back.presets.len(), lib.presets.len());
    let crayon = back.presets.iter().find(|p| p.name == "CRAYON 01").unwrap();
    assert_eq!(crayon.settings, lib.presets.iter().find(|p| p.name == "CRAYON 01").unwrap().settings);
    let tex = back.textures(&crayon.settings);
    assert!(tex.tip.is_some() && tex.paper.is_some(), "textures relues");
    assert!(tex.tip.unwrap().full().get(3, 3) > 0.99);
    std::fs::remove_dir_all(&dir).ok();
}
#[test]
fn old_brush_settings_get_defaults() {
    let s: BrushSettings = serde_json::from_str(r#"{"taille": 12.0}"#).unwrap();
    assert_eq!(s.size, 12.0);
    assert_eq!(s.grain, 0.0);
    assert_eq!(s.roundness, 1.0);
    assert!(s.tip.is_none());
}

#[test]
fn preset_previews_fit_and_show_size() {
    let (w, h) = (60, 30);
    let ink = |px: &[u8]| px.iter().map(|&a| a as u32).sum::<u32>();
    let thin = crate::brush::stroke_preview(&BrushSettings { size: 2.0, ..BrushSettings::brush() }, BrushTextures::default(), w, h);
    let thick = crate::brush::stroke_preview(&BrushSettings { size: 10.0, ..BrushSettings::brush() }, BrushTextures::default(), w, h);
    let huge = crate::brush::stroke_preview(&BrushSettings { size: 300.0, ..BrushSettings::brush() }, BrushTextures::default(), w, h);
    assert_eq!(thin.len(), w * h);
    assert!(ink(&thin) > 0, "un trait fin se voit");
    assert!(ink(&thick) > 2 * ink(&thin), "plus gros, plus d'encre");
    // Un très gros pinceau est ramené à la hauteur : le trait ne remplit pas tout, ni ne touche
    // les bords gauche et droit.
    assert!(ink(&huge) < (w * h * 255) as u32 * 9 / 10);
    for y in 0..h {
        assert_eq!(huge[y * w], 0, "bord gauche, ligne {y}");
        assert_eq!(huge[y * w + w - 1], 0, "bord droit, ligne {y}");
    }
    // Une gomme est montrée comme le trait qu'elle enlève ; le lissage ne déforme pas l'aperçu.
    let eraser = crate::brush::stroke_preview(&BrushSettings::eraser(), BrushTextures::default(), w, h);
    assert!(ink(&eraser) > 0);
}
