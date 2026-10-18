//! Sensation des brosses (étude du 2026-10-18, `docs/SENSATION-BROSSES.md`) : le toucher du
//! stylet (courbe de pression globale, plancher de taille, traits fins, bord doux, seuil d'appui),
//! l'ancien toucher à portée de main, et les traits enregistrés qui gardent leur toucher.

use eframe::egui::pos2;

use crate::brush::{coverage, coverage_soft, BrushSettings, Curve, Inputs, StrokeMode, StrokeState, Toucher};
use crate::raster::Raster;

/// Un pinceau rond net, sans lissage, dont la taille suit l'appui (taille mini 0).
fn pen_brush(size: f32) -> BrushSettings {
    BrushSettings { size, smoothing: 0.0, hardness: 1.0, pressure_size: true, min_size: 0.0, pressure_opacity: false, ..BrushSettings::brush() }
}

/// Un trait au stylet (ou à la souris) par les points (x, y, appui), avec ce toucher ; la feuille.
fn draw(s: BrushSettings, feel: Toucher, pen: bool, pts: &[(f32, f32, f32)]) -> Raster {
    let mut raster = Raster::new(320, 320);
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, pen).with_feel(feel);
    let mut dirty = Vec::new();
    for &(x, y, p) in pts {
        st.add_point(&mut raster, pos2(x, y), p, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    raster
}

/// Un trait horizontal de x = 40 à 280 sur la ligne 160, à l'appui `pressure`.
fn line(pressure: f32) -> Vec<(f32, f32, f32)> {
    (0..=24).map(|k| (40.0 + 10.0 * k as f32, 160.0, pressure)).collect()
}

/// Pixels encrés (plus qu'à moitié) de la colonne `x`.
fn column(r: &Raster, x: usize) -> usize {
    (0..r.height).filter(|&y| r.get(x, y).a() > 128).count()
}

/// Pixels touchés (si peu que ce soit) de la colonne `x`.
fn touched(r: &Raster, x: usize) -> usize {
    (0..r.height).filter(|&y| r.get(x, y).a() > 0).count()
}

#[test]
fn the_global_pressure_curve_shapes_the_pen_pressure_only() {
    let s = pen_brush(20.0);
    // Une courbe qui monte vite : un appui de 0,5 vaut 0,9.
    let steep = Toucher { pressure_curve: Curve::from_points(&[[0.0, 0.0], [0.5, 0.9], [1.0, 1.0]]), ..Toucher::default() };
    let curved = draw(s, steep, true, &line(0.5));
    let plain_half = draw(s, Toucher::default(), true, &line(0.5));
    let plain_heavy = draw(s, Toucher::default(), true, &line(0.9));
    assert_eq!(column(&curved, 160), column(&plain_heavy, 160), "0,5 par la courbe = 0,9 tel quel");
    assert!(column(&curved, 160) > column(&plain_half, 160) + 4, "{} > {}", column(&curved, 160), column(&plain_half, 160));
    // La souris n'y passe pas.
    let mouse = draw(s, steep, false, &line(0.5));
    assert_eq!(column(&mouse, 160), column(&plain_half, 160));
}

#[test]
fn the_size_floor_keeps_a_light_touch_visible() {
    let s = pen_brush(40.0);
    assert!((s.size_factor(&Inputs::pressure(0.0), 0.1) - 0.1).abs() < 1e-6);
    assert!((s.size_factor(&Inputs::pressure(0.0), 0.03) - 0.03).abs() < 1e-6);
    assert_eq!(s.size_factor(&Inputs::pressure(1.0), 0.1), 1.0);
    // Appui nul : 4 px de trait avec le plancher de 10 %, un fil d'avant avec 3 %.
    let new = draw(s, Toucher { threshold: 0.0, ..Toucher::default() }, true, &line(0.0));
    let old = draw(s, Toucher::ancien(), true, &line(0.0));
    assert!(column(&new, 160) >= 3, "nouveau : {} px", column(&new, 160));
    assert!(column(&old, 160) <= 1, "ancien : {} px", column(&old, 160));
    // Le pinceau rond garde du corps quand on effleure (taille mini 18 %).
    assert!((BrushSettings::brush().min_size - 0.18).abs() < 1e-6);
}

#[test]
fn a_hairline_keeps_a_full_pixel_instead_of_a_grey_one() {
    let s = BrushSettings { size: 1.0, pressure_size: false, ..pen_brush(1.0) };
    // Ligne sur les centres des pixels de la rangée 160.
    let pts: Vec<(f32, f32, f32)> = (0..=24).map(|k| (40.0 + 10.0 * k as f32, 160.5, 1.0)).collect();
    let full = draw(s, Toucher::default(), true, &pts);
    let grey = draw(s, Toucher::ancien(), true, &pts);
    // (Les empreintes sont tous les 0,3 px : un centre de pixel est à 0,15 px au plus de l'une
    // d'elles, soit 94 % de couverture au moins ; avant, 67 %.)
    assert!(full.get(160, 160).a() >= 235, "plein : {}", full.get(160, 160).a());
    assert!(grey.get(160, 160).a() <= 200, "ancien, éclairci : {}", grey.get(160, 160).a());
    // Un seul pixel de haut dans les deux cas (pas de bavure).
    assert_eq!(column(&full, 160), 1);
    assert_eq!(full.get(160, 159).a(), 0);
    assert_eq!(full.get(160, 161).a(), 0);
}

#[test]
fn below_the_threshold_the_pen_does_not_touch() {
    let s = pen_brush(20.0);
    // Posé en effleurant (0,01), vrai trait de 80 à 200, puis la pression s'effondre au levé.
    let mut pts: Vec<(f32, f32, f32)> = (0..4).map(|k| (40.0 + 10.0 * k as f32, 160.0, 0.01)).collect();
    pts.extend((0..=12).map(|k| (80.0 + 10.0 * k as f32, 160.0, 0.5)));
    pts.push((230.0, 160.0, 0.01));
    pts.push((260.0, 160.0, 0.01));
    let new = draw(s, Toucher::default(), true, &pts);
    assert_eq!(touched(&new, 50), 0, "rien avant que le stylet touche");
    assert!(column(&new, 150) >= 8, "le trait : {}", column(&new, 150));
    assert_eq!(touched(&new, 245), 0, "rien après que la pression s'est effondrée");
    // L'ancien toucher posait tout, en fil.
    let old = draw(s, Toucher::ancien(), true, &pts);
    assert!(touched(&old, 50) >= 1 && touched(&old, 245) >= 1);
    // La souris appuie toujours à fond : le seuil ne la concerne pas.
    let mouse = draw(s, Toucher::default(), false, &[(40.0, 160.0, 1.0), (280.0, 160.0, 1.0)]);
    assert!(column(&mouse, 50) >= 18);
    // Les points sont tous dans le journal du banc, tels que reçus.
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut raster = Raster::new(64, 64);
    let mut dirty = Vec::new();
    st.add_point(&mut raster, pos2(10.0, 10.0), 0.01, &mut dirty);
    st.add_point(&mut raster, pos2(20.0, 10.0), 0.5, &mut dirty);
    assert_eq!(st.raw_log.len(), 2);
    assert_eq!(st.raw_log[0][2], 0.01f32 as f64);
}

#[test]
fn the_soft_edge_has_the_optical_radius_and_no_notch() {
    // Pointe dure : à moitié au rayon, pleine un demi-pixel avant, rien un demi-pixel après.
    assert!((coverage_soft(10.0, 10.0, 10.0) - 0.5).abs() < 1e-5);
    assert_eq!(coverage_soft(9.5, 10.0, 10.0), 1.0);
    assert_eq!(coverage_soft(10.5, 10.0, 10.0), 0.0);
    // (L'ancienne rampe coupée par la dureté s'éteignait déjà au rayon : une pointe dure était
    // un demi-pixel plus fine que sa taille.)
    assert_eq!(coverage(10.0, 10.0, 9.5, false), 0.0);
    // Demi-dure : une seule chute, qui ne remonte jamais.
    let mut prev = 1.0;
    for k in 0..=420 {
        let d = k as f32 * 0.05;
        let c = coverage_soft(d, 20.0, 10.0);
        assert!(c <= prev + 1e-6, "cran à d = {d} : {c} > {prev}");
        prev = c;
    }
    assert_eq!(coverage_soft(10.0, 20.0, 10.0), 1.0);
    assert_eq!(coverage_soft(20.5, 20.0, 10.0), 0.0);
    // Un demi-pixel de rayon remplit le pixel de son centre, à moitié les voisins touchés.
    assert_eq!(coverage_soft(0.0, 0.5, 0.5), 1.0);
    assert!((coverage_soft(0.5, 0.5, 0.5) - 0.5).abs() < 1e-5);
    assert_eq!(coverage_soft(1.0, 0.5, 0.5), 0.0);
}

#[test]
fn the_pencil_is_antialiased_and_bites_with_the_pressure() {
    let p = BrushSettings::pencil();
    assert!(!p.aliased && p.hardness == 1.0 && p.pressure_opacity && (p.min_opacity - 0.3).abs() < 1e-6, "{p:?}");
    assert!(p.size >= 2.0 && p.smoothing > 0.0 && p.smoothing < 0.3);
    // Une diagonale : des demi-teintes sur les bords (anticrénelage) ; en pixels nets, rien que
    // du plein ou du vide, en escalier.
    let diagonal: Vec<(f32, f32, f32)> = (0..=24).map(|k| (40.0 + 10.0 * k as f32, 40.0 + 7.0 * k as f32, 1.0)).collect();
    let shades = |r: &Raster| (0..320).flat_map(|y| (0..320).map(move |x| (x, y))).filter(|&(x, y)| matches!(r.get(x, y).a(), 1..=254)).count();
    let smooth = draw(BrushSettings { smoothing: 0.0, ..p }, Toucher::default(), false, &diagonal);
    let crisp = draw(BrushSettings { smoothing: 0.0, aliased: true, ..p }, Toucher::default(), false, &diagonal);
    assert!(shades(&smooth) > 100, "demi-teintes : {}", shades(&smooth));
    assert_eq!(shades(&crisp), 0, "pixels nets : aucune demi-teinte");
    // Effleuré au stylet, le crayon mord moins (30 % au moins) ; appuyé, il est noir. (Ligne sur
    // les centres des pixels : un trait effleuré fait moins d'un pixel.)
    let centered = |pressure: f32| -> Vec<(f32, f32, f32)> { (0..=24).map(|k| (40.0 + 10.0 * k as f32, 160.5, pressure)).collect() };
    let light = draw(BrushSettings { smoothing: 0.0, ..p }, Toucher::default(), true, &centered(0.05));
    let heavy = draw(BrushSettings { smoothing: 0.0, ..p }, Toucher::default(), true, &centered(1.0));
    let (l, h) = (light.get(160, 160).a(), heavy.get(160, 160).a());
    assert!(h >= 250 && l < h && l >= 60, "effleuré {l}, appuyé {h}");
}

#[test]
fn the_old_feel_is_within_reach_and_strokes_keep_their_feel() {
    let old = Toucher::ancien();
    assert_eq!((old.size_floor, old.thin_full, old.soft_edge, old.threshold), (0.03, false, false, 0.0));
    let new = Toucher::default();
    assert!(new.size_floor > old.size_floor && new.thin_full && new.soft_edge && new.threshold > 0.0);
    // Un trait enregistré avant l'étude (sans toucher) relit le toucher par défaut ; un trait
    // d'aujourd'hui garde le sien.
    let text = r#"{"reglage":{"taille":8.0},"couleur":[0,0,0],"gomme":false,"points":[[1.0,1.0,0.5,-1.0]]}"#;
    let r: crate::banc::Recorded = serde_json::from_str(text).unwrap();
    assert_eq!(r.feel, Toucher::default());
    let mut r2 = r.clone();
    r2.feel = Toucher { size_floor: 0.2, ..Toucher::ancien() };
    let back: crate::banc::Recorded = serde_json::from_str(&serde_json::to_string(&r2).unwrap()).unwrap();
    assert_eq!(back, r2);
    // Le toucher se garde en TOML (réglages de l'application), champ par champ.
    let toml_text = toml::to_string(&new).unwrap();
    assert!(toml_text.contains("plancher_taille") && toml_text.contains("courbe_pression"), "{toml_text}");
    let partial: Toucher = toml::from_str("fin_plein = false\n").unwrap();
    assert!(!partial.thin_full && partial.soft_edge && (partial.size_floor - 0.1).abs() < 1e-6);
}
