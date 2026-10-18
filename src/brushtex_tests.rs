//! Tests des pinceaux texturés et de l'import Toon Boom (phase 3d-2).

use std::sync::Arc;

use eframe::egui::pos2;

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::brushtex::{self, BrushLibrary, BrushTextures, GrayImage, Target, Tip};
use crate::raster::Raster;
use crate::tbbrush;

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

/// `bundledFile` d'une image grise `w`×`h` (lignes complétées à 4 octets).
fn bundled(w: usize, h: usize, pixel: impl Fn(usize, usize) -> u8) -> String {
    let stride = (w + 3) & !3;
    let mut bytes = Vec::new();
    for v in [(stride * h) as u32, w as u32, h as u32, 8u32] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0]);
    for y in 0..h {
        for x in 0..stride {
            bytes.push(if x < w { pixel(x, y) } else { 0 });
        }
    }
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[test]
fn bundled_textures_are_decoded() {
    let hex = bundled(3, 2, |x, y| (x * 100 + y * 10) as u8);
    let img = tbbrush::decode_bundled(&hex).unwrap();
    assert_eq!((img.width, img.height), (3, 2));
    assert_eq!(img.get(2, 1), 210.0 / 255.0, "le bourrage de fin de ligne est sauté");
    // 32 bits par pixel, ou longueur incohérente : refusé avec un message.
    let mut bad = hex.clone();
    bad.replace_range(24..26, "20");
    assert!(tbbrush::decode_bundled(&bad).unwrap_err().contains("32 bits"));
    assert!(tbbrush::decode_bundled(&hex[..hex.len() - 4]).is_err());
}

#[test]
fn small_xml_reader() {
    let doc = r#"<?xml version="1.0"?><!DOCTYPE penstyles>
        <!-- commentaire -->
        <penstyles a="1">
          <pen><name value="CRAYON &amp; co"/><size><max value='12.5'/></size></pen>
          <pen><name value="B"/></pen>
        </penstyles>"#;
    let root = tbbrush::parse_xml(doc).unwrap();
    assert_eq!(root.name, "penstyles");
    assert_eq!(root.attr("a"), Some("1"));
    let pens: Vec<_> = root.all("pen").collect();
    assert_eq!(pens.len(), 2);
    assert_eq!(pens[0].value("name"), Some("CRAYON & co"));
    assert_eq!(pens[0].num("size/max"), Some(12.5));
    assert!(tbbrush::parse_xml("<a><b></a>").is_err(), "balises mal fermées");
}

/// Un fichier de préréglages minimal : une pointe, un papier, un crayon et une gomme.
fn preset_file() -> String {
    let tip = bundled(8, 8, |x, y| if (3..5).contains(&x) && (3..5).contains(&y) { 0 } else { 255 });
    let paper = bundled(6, 6, |x, y| ((x + y) * 40) as u8);
    let pressure = r#"<stylusFunction class="compositeFunction"><unitaryFunction class="pressureFunction"><enabled value="true"/></unitaryFunction></stylusFunction>"#;
    format!(
        r#"<!DOCTYPE penstyles><penstyles>
        <pen><style value="texture"/><name value="CRAYON 01"/>
          <mainBrushShape>
            <sizeRange><valueRange><minPercentage value="0"/><maxValue value="10"/>{pressure}</valueRange>
              <minPercentage value="20"/><maxValue value="10"/><randomPercentage value="0"/></sizeRange>
            <hardness value="2.5"/><roundness value="100"/><angle value="0"/><randomAngle value="0"/>
            <spacing value="7"/><brushTextureId value="T1"/>
          </mainBrushShape>
          <smooth value="20"/><antialiasingEnabled value="true"/>
          <flowRange><valueRange>{pressure}</valueRange><minPercentage value="50"/><maxValue value="80"/></flowRange>
          <opacityRange><valueRange>{pressure}</valueRange><minPercentage value="0"/><maxValue value="100"/></opacityRange>
          <paperTextureEnabled value="true"/><paperTextureId value="P1"/>
        </pen>
        <pen><style value="brush"/><name value="Eraser 1"/>
          <mainBrushShape><sizeRange><minPercentage value="0"/><maxValue value="100"/></sizeRange>
            <hardness value="5"/><spacing value="7"/></mainBrushShape>
          <paperTextureEnabled value="false"/>
        </pen>
        <texture class="bitmapBrushImageClass"><textureId value="T1"/><textureCategory value="bitmapBrushImageCategory"/>
          <textureName value="Pointe"/><bundledFile value="{tip}"/></texture>
        <texture class="bitmapBrushImageClass"><textureId value="P1"/><textureCategory value="paperTextureCategory"/>
          <textureName value="Papier"/><bundledFile value="{paper}"/></texture>
        </penstyles>"#
    )
}

#[test]
fn toon_boom_pens_become_pelure_presets() {
    let mut lib = BrushLibrary::builtin();
    let builtin = lib.presets.len();
    let report = tbbrush::import(&preset_file(), "essai.xml", &mut lib).unwrap();
    assert_eq!((report.presets, report.tips, report.papers), (2, 1, 1));
    let crayon = &lib.presets[builtin];
    assert_eq!((crayon.name.as_str(), crayon.target, crayon.source.as_str()), ("CRAYON 01", Target::Brush, "essai.xml"));
    let s = crayon.settings;
    assert_eq!(s.size, 10.0);
    assert!((s.min_size - 0.2).abs() < 1e-6, "taille en effleurant : 20 %");
    assert!((s.hardness - 0.5).abs() < 1e-6, "dureté 2,5 sur 5");
    assert!((s.spacing - 0.07).abs() < 1e-6 && (s.smoothing - 0.2).abs() < 1e-6);
    assert!((s.opacity - 0.8).abs() < 1e-6, "opacité 100 % × débit 80 %");
    assert!(s.pressure_opacity && s.min_opacity == 0.0, "opacité 0 % en effleurant");
    assert!(s.pressure_size && !s.aliased);
    assert_eq!(s.grain, 1.0);
    // Pointe : noir = couleur ; le centre de l'image (noir) dépose, le bord (blanc) non.
    let tex = lib.textures(&s);
    let tip = tex.tip.expect("pointe importée");
    assert_eq!(tip.full().get(3, 3), 1.0);
    assert_eq!(tip.full().get(0, 0), 0.0);
    assert!(tex.paper.is_some());
    let eraser = &lib.presets[builtin + 1];
    assert_eq!(eraser.target, Target::Eraser);
    assert!(eraser.settings.tip.is_none() && eraser.settings.paper.is_none());
    // Pas un fichier de pinceaux : message clair.
    assert!(tbbrush::import("<autre/>", "x.xml", &mut lib).unwrap_err().contains("penstyles"));
}

#[test]
fn grain_depends_on_paper_and_pressure() {
    assert_eq!(brushtex::grain_factor(0.0, 1.0, 0.0), 1.0, "papier lisse");
    // Sommets avant creux ; appuyer remplit.
    assert!(brushtex::grain_factor(0.9, 0.5, 1.0) > brushtex::grain_factor(0.2, 0.5, 1.0));
    assert!(brushtex::grain_factor(0.3, 1.0, 1.0) > brushtex::grain_factor(0.3, 0.2, 1.0));
    assert!(brushtex::grain_factor(0.0, 1.0, 1.0) < 0.2, "les creux profonds restent vides");
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
    tbbrush::import(&preset_file(), "essai.xml", &mut lib).unwrap();
    lib.save(&dir).unwrap();
    let back = BrushLibrary::load(Some(&dir));
    assert_eq!(back.presets.len(), lib.presets.len());
    let crayon = back.presets.iter().find(|p| p.name == "CRAYON 01").unwrap();
    assert_eq!(crayon.settings, lib.presets.iter().find(|p| p.name == "CRAYON 01").unwrap().settings);
    let tex = back.textures(&crayon.settings);
    assert!(tex.tip.is_some() && tex.paper.is_some(), "textures relues");
    assert_eq!(tex.tip.unwrap().full().get(3, 3), 1.0);
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

/// Import du vrai fichier d'Anthony (lancé à la main : `PELURE_PINCEAUX=<fichier>` et `--ignored`).
#[test]
#[ignore]
fn import_real_file() {
    let path = std::env::var("PELURE_PINCEAUX").expect("PELURE_PINCEAUX");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lib = BrushLibrary::builtin();
    let n = lib.presets.len();
    let r = tbbrush::import(&text, "STB_TextureBrushes_preset.xml", &mut lib).unwrap();
    eprintln!("{} pinceaux, {} pointes, {} papiers, ignorées : {:?}", r.presets, r.tips, r.papers, r.skipped);
    for p in &lib.presets[n..] {
        let s = p.settings;
        eprintln!(
            "{:<12} {:?} taille {:.1} (min {:.0} %) dureté {:.2} espace {:.2} lissage {:.2} opacité {:.2} (min {:.2}) grain {} pointe {:?} papier {:?} rondeur {:.2} angle {:.0} ±{:.0} taille± {:.2}",
            p.name, p.target, s.size, s.min_size * 100.0, s.hardness, s.spacing, s.smoothing, s.opacity, s.min_opacity,
            s.grain, s.tip.and_then(|id| lib.tip_name(id)), s.paper, s.roundness, s.angle, s.angle_jitter, s.size_jitter
        );
    }
}
