//! Connexions des brosses (moteur de dessin, étape 2) : courbes, sources (pression, vitesse,
//! direction, fondu, hasard), et les anciens réglages qui donnent le même trait.

use eframe::egui::pos2;

use crate::brush::{stroke_preview, BrushSettings, Connection, Curve, Source, StrokeMode, StrokeState};
use crate::raster::Raster;

#[test]
fn curves_are_kept_in_order_and_read_between_points() {
    let c = Curve::from_points(&[[0.8, 0.9], [0.2, 0.1], [2.0, 1.5]]);
    assert_eq!(c.points(), &[[0.0, 0.1], [0.8, 0.9], [1.0, 1.0]], "triés, bornés, bouts aux bords");
    assert!((c.eval(0.4) - 0.5).abs() < 1e-5, "entre deux points : en ligne droite");
    assert!((c.eval(0.9) - 0.95).abs() < 1e-5);
    assert_eq!(Curve::from_points(&[]).points(), Curve::linear().points());
    let flat = Curve::from_points(&[[0.3, 0.6]]);
    assert_eq!(flat.eval(0.0), 0.6);
    assert_eq!(flat.eval(1.0), 0.6);
    let many: Vec<[f32; 2]> = (0..20).map(|i| [i as f32 / 19.0, 0.5]).collect();
    assert_eq!(Curve::from_points(&many).points().len(), Curve::MAX);
    // Réponse : de `min` à 1 ; inverser retourne l'entrée.
    let conn = Connection { source: Source::Pressure, invert: true, curve: Curve::linear(), min: 0.2 };
    assert!((conn.response(1.0) - 0.2).abs() < 1e-5);
    assert!((conn.response(0.0) - 1.0).abs() < 1e-5);
}

#[test]
fn the_old_pressure_settings_draw_the_same_stroke() {
    let mut old = BrushSettings::brush();
    old.pressure_size = true;
    old.min_size = 0.3;
    old.pressure_opacity = true;
    old.min_opacity = 0.4;
    let mut new = old;
    new.pressure_size = false;
    new.pressure_opacity = false;
    new.dynamics.size = Connection { source: Source::Pressure, min: 0.3, ..Default::default() };
    new.dynamics.opacity = Connection { source: Source::Pressure, min: 0.4, ..Default::default() };
    let tex = Default::default;
    assert_eq!(stroke_preview(&old, tex(), 160, 48), stroke_preview(&new, tex(), 160, 48));
    // Une connexion choisie remplace la pression simple.
    let mut both = old;
    both.dynamics.size = Connection { source: Source::Random, ..Default::default() };
    assert_eq!(both.size_connection().map(|c| c.source), Some(Source::Random));
}

/// Un trait de `a` à `b` en `n` points, `dt` secondes entre deux points, à l'appui `pressure` ;
/// renvoie la feuille.
fn draw(s: BrushSettings, a: (f32, f32), b: (f32, f32), n: usize, dt: f64, pressure: f32) -> Raster {
    let mut raster = Raster::new(320, 320);
    let mut stroke = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut dirty = Vec::new();
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let p = pos2(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        stroke.add_point_at(&mut raster, p, pressure, k as f64 * dt, &mut dirty);
    }
    stroke.finish(&mut raster, &mut dirty);
    raster
}

/// Pixels encrés (plus qu'à moitié) de la colonne `x`, ou de la ligne `y`.
fn column(r: &Raster, x: usize) -> usize {
    (0..r.height).filter(|&y| r.get(x, y).a() > 128).count()
}
fn row(r: &Raster, y: usize) -> usize {
    (0..r.width).filter(|&x| r.get(x, y).a() > 128).count()
}

fn plain(size: f32) -> BrushSettings {
    BrushSettings { size, smoothing: 0.0, pressure_size: false, pressure_opacity: false, hardness: 1.0, ..BrushSettings::brush() }
}

#[test]
fn a_fast_stroke_is_thinner_when_size_follows_speed_inverted() {
    let mut s = plain(20.0);
    s.dynamics.size = Connection { source: Source::Speed, invert: true, min: 0.2, ..Default::default() };
    s.dynamics.speed_max = 1000.0;
    // 200 pixels en 20 points : 10 px tous les 0,1 s (100 px/s) ou tous les 0,005 s (2000 px/s).
    let slow = draw(s, (60.0, 160.0), (260.0, 160.0), 20, 0.1, 1.0);
    let fast = draw(s, (60.0, 160.0), (260.0, 160.0), 20, 0.005, 1.0);
    let (ws, wf) = (column(&slow, 200), column(&fast, 200));
    assert!(ws >= 16 && wf <= 6, "lent {ws} px, rapide {wf} px");
    // Sans instants (rejeu), la vitesse reste nulle : trait plein.
    let mut raster = Raster::new(320, 320);
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut dirty = Vec::new();
    for k in 0..=20 {
        st.add_point(&mut raster, pos2(60.0 + 10.0 * k as f32, 160.0), 1.0, &mut dirty);
    }
    assert!(column(&raster, 200) >= 16);
}

#[test]
fn a_flat_tip_follows_the_direction_of_the_stroke() {
    let mut s = plain(24.0);
    s.roundness = 0.2;
    s.angle = 90.0;
    let horizontal = |s| draw(s, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0);
    let vertical = |s| draw(s, (160.0, 40.0), (160.0, 280.0), 24, 0.02, 1.0);
    // Angle fixe : la pointe plate est en travers d'un trait, dans le sens de l'autre.
    let (h, v) = (column(&horizontal(s), 160), row(&vertical(s), 160));
    assert!(h >= 20 && v <= 8, "sans connexion : {h} / {v}");
    // L'angle suit la direction : les deux traits ont la même épaisseur.
    s.dynamics.angle = Connection { source: Source::Direction, ..Default::default() };
    let (h, v) = (column(&horizontal(s), 160), row(&vertical(s), 160));
    assert!(h >= 20 && v >= 20 && h.abs_diff(v) <= 2, "avec connexion : {h} / {v}");
}

#[test]
fn a_fade_thins_the_stroke_along_its_length() {
    let mut s = plain(24.0);
    s.dynamics.size = Connection { source: Source::Fade, invert: true, min: 0.1, ..Default::default() };
    s.dynamics.fade_length = 200.0;
    let r = draw(s, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0);
    let (start, end) = (column(&r, 50), column(&r, 250));
    assert!(start >= 20 && end <= 4, "début {start} px, fin {end} px");
}

#[test]
fn random_sizes_vary_but_the_same_stroke_comes_out_the_same() {
    let mut s = plain(20.0);
    s.spacing = 0.6;
    s.dynamics.size = Connection { source: Source::Random, min: 0.1, ..Default::default() };
    let a = draw(s, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0);
    let b = draw(s, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0);
    let widths: Vec<usize> = (50..270).step_by(6).map(|x| column(&a, x)).collect();
    let (lo, hi) = (*widths.iter().min().unwrap(), *widths.iter().max().unwrap());
    assert!(hi - lo >= 6, "des tailles différentes : {widths:?}");
    assert!((0..320).all(|x| column(&a, x) == column(&b, x)), "même hasard pour le même trait");
}

#[test]
fn old_brush_settings_read_without_dynamics_and_curves_round_trip() {
    let old = r#"{"taille":12.0,"opacite":0.8,"pression_taille":true,"taille_min":0.25}"#;
    let s: BrushSettings = serde_json::from_str(old).unwrap();
    assert_eq!(s.size, 12.0);
    assert!(!s.dynamics.size.active() && s.dynamics.speed_max == 2000.0);
    let mut s2 = s;
    s2.dynamics.angle = Connection { source: Source::Direction, invert: true, curve: Curve::from_points(&[[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]]), min: 0.1 };
    let text = serde_json::to_string(&s2).unwrap();
    assert!(text.contains("\"dynamique\"") && text.contains("\"direction\"") && text.contains("[0.5,0.8]"), "{text}");
    let back: BrushSettings = serde_json::from_str(&text).unwrap();
    assert_eq!(back, s2);
}

/// Un aller-retour (`passes` passages) sur la même ligne, en un seul trait.
fn scribble(s: BrushSettings, passes: usize) -> Raster {
    let mut raster = Raster::new(320, 320);
    let mut stroke = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut dirty = Vec::new();
    let mut t = 0.0;
    for k in 0..passes {
        let (a, b) = if k % 2 == 0 { (60.0, 260.0) } else { (260.0, 60.0) };
        for i in 0..=20 {
            let x = a + (b - a) * i as f32 / 20.0;
            stroke.add_point_at(&mut raster, pos2(x, 160.0), 1.0, t, &mut dirty);
            t += 0.01;
        }
    }
    stroke.finish(&mut raster, &mut dirty);
    raster
}

#[test]
fn power_builds_up_when_passing_again_and_stops_at_the_opacity() {
    let mut s = plain(16.0);
    s.power = 0.3;
    s.opacity = 0.8;
    // Sans accumuler : repasser ne change rien, le trait reste au débit fois l'opacité.
    let once = scribble(s, 1).get(160, 160).a();
    let thrice = scribble(s, 3).get(160, 160).a();
    assert_eq!(once, thrice);
    assert!((once as f32 - 0.3 * 0.8 * 255.0).abs() <= 2.0, "{once}");
    // En accumulant : chaque passage fonce, jamais au-delà de l'opacité.
    s.build_up = true;
    let one = scribble(s, 1).get(160, 160).a();
    let three = scribble(s, 3).get(160, 160).a();
    let many = scribble(s, 12).get(160, 160).a();
    assert!((one as f32 - 0.3 * 0.8 * 255.0).abs() <= 12.0, "un passage dépose à peu près le débit : {one}");
    assert!(one < three && three < many, "{one} < {three} < {many}");
    assert!(many as f32 <= 0.8 * 255.0 + 1.0, "plafond de l'opacité : {many}");
    assert!(many as f32 >= 0.75 * 255.0, "presque au plafond après 12 passages : {many}");
}

#[test]
fn the_default_power_draws_like_before() {
    let s = BrushSettings::brush();
    assert_eq!((s.power, s.build_up), (1.0, false));
    // Lu d'un ancien fichier : débit plein, pas d'accumulation.
    let old: BrushSettings = serde_json::from_str(r#"{"taille":8.0}"#).unwrap();
    assert_eq!((old.power, old.build_up), (1.0, false));
}

/// Une seule empreinte de diamètre 40 au centre d'une feuille.
fn one_dab(s: BrushSettings) -> Raster {
    let mut raster = Raster::new(80, 80);
    let mut stroke = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut dirty = Vec::new();
    stroke.add_point(&mut raster, pos2(40.0, 40.0), 1.0, &mut dirty);
    stroke.finish(&mut raster, &mut dirty);
    raster
}

#[test]
fn the_tip_profile_shapes_the_dab_and_can_mimic_the_hardness() {
    let mut s = plain(40.0);
    s.hardness = 0.5;
    // Le profil tiré de la dureté donne presque la même empreinte.
    let hard = one_dab(s);
    s.profile = Some(BrushSettings::profile_from_hardness(0.5));
    let prof = one_dab(s);
    let worst = (0..80).flat_map(|y| (0..80).map(move |x| (x, y))).map(|(x, y)| hard.get(x, y).a().abs_diff(prof.get(x, y).a())).max().unwrap();
    assert!(worst <= 24, "écart au pire {worst}");
    // Un profil diffus : plein au centre, bien plus léger à mi-rayon qu'un profil dur.
    s.profile = Some(Curve::from_points(&[[0.0, 1.0], [0.15, 0.6], [0.4, 0.25], [0.7, 0.07], [1.0, 0.0]]));
    let diffuse = one_dab(s);
    s.profile = Some(Curve::from_points(&[[0.0, 1.0], [0.9, 1.0], [1.0, 0.0]]));
    let solid = one_dab(s);
    // (Le pixel du centre est à 0,7 pixel du centre exact : le profil diffus y descend déjà un peu.)
    assert!(diffuse.get(40, 40).a() >= 220 && solid.get(40, 40).a() >= 250);
    let (d, h) = (diffuse.get(50, 40).a(), solid.get(50, 40).a());
    assert!(h >= 250 && d <= 120, "à mi-rayon : diffus {d}, dur {h}");
    assert_eq!(solid.get(65, 40).a(), 0, "rien au-delà du bord");
}

#[test]
fn a_profile_is_saved_only_when_set() {
    let s = BrushSettings::brush();
    assert!(!serde_json::to_string(&s).unwrap().contains("profil"));
    let mut s2 = s;
    s2.profile = Some(BrushSettings::profile_from_hardness(0.3));
    let back: BrushSettings = serde_json::from_str(&serde_json::to_string(&s2).unwrap()).unwrap();
    assert_eq!(back.profile, s2.profile);
}

#[test]
fn a_pulled_string_ignores_small_wobbles() {
    let mut s = plain(4.0);
    s.smooth_kind = crate::brush::SmoothKind::PulledString;
    s.smooth_px = 12.0;
    let mut raster = Raster::new(320, 320);
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut dirty = Vec::new();
    // Un trait horizontal qui tremble de ± 4 pixels.
    for k in 0..=60 {
        let y = 160.0 + if k % 2 == 0 { 4.0 } else { -4.0 };
        st.add_point(&mut raster, pos2(40.0 + 4.0 * k as f32, y), 1.0, &mut dirty);
    }
    let ys: Vec<f32> = st.path.iter().skip(5).map(|p| p.y).collect();
    let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
    assert!(hi - lo < 3.0, "le tremblement est gommé : {lo}..{hi}");
}

/// Lignes touchées par l'encre, de haut en bas.
fn ink_rows(r: &Raster) -> (usize, usize) {
    let rows: Vec<usize> = (0..r.height).filter(|&y| row(r, y) > 0).collect();
    (*rows.first().unwrap_or(&0), *rows.last().unwrap_or(&0))
}

#[test]
fn scatter_spreads_the_dabs_across_the_stroke() {
    let s = plain(8.0);
    let (a, b) = ink_rows(&draw(s, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0));
    let mut sc = s;
    sc.scatter = 2.0;
    sc.count = 3;
    let r = draw(sc, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0);
    let (c, d) = ink_rows(&r);
    assert!(b - a <= 10 && d - c >= 20, "sans dispersion {a}..{b}, avec {c}..{d}");
    // La dispersion reste dans ±2 diamètres (plus le rayon).
    assert!(c >= 160 - 16 - 5 && d <= 160 + 16 + 5, "{c}..{d}");
    // Même trait, même hasard.
    let r2 = draw(sc, (40.0, 160.0), (280.0, 160.0), 24, 0.02, 1.0);
    assert!((0..320).all(|x| column(&r, x) == column(&r2, x)));
}

/// Un papier à rayures verticales : 4 texels de sommet (1), 4 de creux (0).
fn striped_paper() -> crate::brushtex::GrayImage {
    let values = (0..8 * 8).map(|i| if (i % 8) < 4 { 1.0 } else { 0.0 }).collect();
    crate::brushtex::GrayImage::new(8, 8, values)
}

/// Un trait horizontal large, appuyé à moitié, sur le papier rayé ; renvoie l'encre de la ligne
/// du milieu, colonne par colonne (de 100 à 164).
fn on_paper(s: BrushSettings) -> Vec<u8> {
    let tex = crate::brushtex::BrushTextures { tip: None, paper: Some(std::sync::Arc::new(striped_paper())) };
    let mut raster = Raster::new(320, 320);
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true).with_textures(tex);
    let mut dirty = Vec::new();
    for k in 0..=20 {
        st.add_point(&mut raster, pos2(40.0 + 12.0 * k as f32, 160.0), 0.3, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    (100..164).map(|x| raster.get(x, 160).a()).collect()
}

/// Longueur moyenne des plages encrées (plus qu'à moitié) d'une ligne.
fn run_length(v: &[u8]) -> f32 {
    let mut runs = Vec::new();
    let mut n = 0;
    for &a in v {
        if a > 128 {
            n += 1;
        } else if n > 0 {
            runs.push(n);
            n = 0;
        }
    }
    if runs.is_empty() {
        return 0.0;
    }
    runs.iter().sum::<usize>() as f32 / runs.len() as f32
}

#[test]
fn the_paper_grain_scales_turns_and_inverts() {
    let mut s = plain(30.0);
    s.paper = Some(0);
    s.grain = 1.0;
    let base = on_paper(s);
    assert!((run_length(&base) - 4.0).abs() <= 1.0, "rayures de 4 : {base:?}");
    // Deux fois plus grand : des plages deux fois plus longues.
    s.paper_scale = 2.0;
    let big = on_paper(s);
    assert!((run_length(&big) - 8.0).abs() <= 1.5, "rayures de 8 : {}", run_length(&big));
    // Inversé : l'encre passe dans les anciens creux.
    s.paper_scale = 1.0;
    s.paper_invert = true;
    let inv = on_paper(s);
    let same = base.iter().zip(&inv).filter(|(a, b)| (**a > 128) == (**b > 128)).count();
    assert!(same <= 12, "inversé : {same} colonnes pareilles sur 64");
    // Tourné d'un quart de tour : les rayures deviennent horizontales, la ligne est d'un seul ton.
    s.paper_invert = false;
    s.paper_angle = 90.0;
    let turned = on_paper(s);
    let inked = turned.iter().filter(|a| **a > 128).count();
    assert!(inked == 0 || inked == 64, "une ligne d'un seul ton : {inked}");
}

#[test]
fn an_image_becomes_a_wrapped_gray_paper() {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(4, 2, |x, _| if x < 2 { image::Rgb([255, 255, 255]) } else { image::Rgb([0, 0, 0]) }));
    let g = crate::brushtex::GrayImage::from_image(&img, 1024);
    assert_eq!((g.width, g.height), (4, 2));
    assert_eq!((g.get(0, 0), g.get(3, 1)), (1.0, 0.0));
    // Répétée sans fin, dans les deux sens.
    assert!((g.sample_wrapped(4.5, 0.5) - 1.0).abs() < 1e-5 && (g.sample_wrapped(-0.5, 0.5) - 0.0).abs() < 1e-5);
    let big = image::DynamicImage::ImageLuma8(image::GrayImage::new(3000, 1500));
    let g = crate::brushtex::GrayImage::from_image(&big, 1024);
    assert!(g.width <= 1024 && g.height <= 1024, "réduite : {}×{}", g.width, g.height);
}