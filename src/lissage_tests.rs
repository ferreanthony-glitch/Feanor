//! Lissage (sensation des brosses, étude du 2026-10-18, §2.3) : « Suivi » selon le temps, corde
//! et distance en pixels d'écran, rattrapage de la pointe au levé, lissage après le trait.

use eframe::egui::{pos2, Pos2};

use crate::brush::{follow_alpha, BrushSettings, CatchUp, SmoothKind, StrokeMode, StrokeState, FOLLOW_REF_HZ};
use crate::raster::Raster;

/// Un pinceau net de 4 px, sans pression ni lissage.
fn base() -> BrushSettings {
    BrushSettings { size: 4.0, hardness: 1.0, pressure_size: false, pressure_opacity: false, smoothing: 0.0, ..BrushSettings::brush() }
}

/// Un trait horizontal de `n` pas de 4 px qui tremble de ± 4 px.
fn wobble(n: usize) -> Vec<Pos2> {
    (0..=n).map(|k| pos2(40.0 + 4.0 * k as f32, 160.0 + if k % 2 == 0 { 4.0 } else { -4.0 })).collect()
}

/// Les points `pts` reçus par un trait au zoom `zoom`, `dt` secondes entre deux (aucun : sans
/// instants) ; le trait, pas encore fini, et sa feuille.
fn run(s: BrushSettings, zoom: f32, pts: &[Pos2], dt: Option<f64>) -> (StrokeState, Raster) {
    let mut raster = Raster::new(320, 320);
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true).with_zoom(zoom);
    let mut dirty = Vec::new();
    for (k, p) in pts.iter().enumerate() {
        match dt {
            Some(dt) => st.add_point_at(&mut raster, *p, 1.0, k as f64 * dt, &mut dirty),
            None => st.add_point(&mut raster, *p, 1.0, &mut dirty),
        }
    }
    (st, raster)
}

/// Amplitude du tremblement du trait posé (hors les premiers points).
fn amplitude(st: &StrokeState) -> f32 {
    let ys: Vec<f32> = st.path.iter().skip(6).map(|p| p.y).collect();
    ys.iter().fold(f32::MIN, |a, &y| a.max(y)) - ys.iter().fold(f32::MAX, |a, &y| a.min(y))
}

#[test]
fn follow_alpha_depends_on_the_time_not_on_the_number_of_samples() {
    assert!((follow_alpha(0.3, None) - 0.7).abs() < 1e-6, "sans instant : par échantillon");
    assert!((follow_alpha(0.3, Some(1.0 / FOLLOW_REF_HZ)) - 0.7).abs() < 1e-5, "à la cadence de référence : comme avant");
    assert!((follow_alpha(0.3, Some(2.0 / FOLLOW_REF_HZ)) - 0.91).abs() < 1e-5, "deux fois plus de temps : deux rattrapages");
    let fast = follow_alpha(0.3, Some(0.5 / FOLLOW_REF_HZ));
    assert!(fast > 0.4 && fast < 0.7, "deux fois plus d'échantillons : moins par échantillon ({fast})");
    assert_eq!(follow_alpha(0.3, Some(0.0)), 0.7);
    assert_eq!(follow_alpha(0.0, Some(0.01)), 1.0);
    assert!(follow_alpha(0.95, Some(1.0)) > 0.999, "après une pause, on rattrape tout");
}

#[test]
fn the_time_based_follow_filters_by_time_whatever_the_cadence() {
    let mut s = base();
    s.smoothing = 0.5;
    let pts = wobble(60);
    // Même géométrie, échantillonnée vite (2 ms) ou lentement (20 ms) : le même lissage dans le
    // temps, donc un tremblement rapide plus gommé qu'un tremblement lent.
    let (fast, _) = run(s, 1.0, &pts, Some(0.002));
    let (slow, _) = run(s, 1.0, &pts, Some(0.020));
    assert!(amplitude(&fast) < amplitude(&slow) - 1.0, "vite : {} ; lentement : {}", amplitude(&fast), amplitude(&slow));
    // Par échantillon (comme avant) : la cadence ne change rien.
    s.smooth_time = false;
    let (a, _) = run(s, 1.0, &pts, Some(0.002));
    let (b, _) = run(s, 1.0, &pts, Some(0.020));
    assert!((amplitude(&a) - amplitude(&b)).abs() < 1e-3);
    // Sans instants : par échantillon aussi.
    s.smooth_time = true;
    let (c, _) = run(s, 1.0, &pts, None);
    assert!((amplitude(&c) - amplitude(&a)).abs() < 1e-3);
}

#[test]
fn the_pulled_string_is_measured_on_screen_when_asked() {
    let mut s = base();
    s.smooth_kind = SmoothKind::PulledString;
    s.smooth_px = 12.0;
    let pts = wobble(60);
    // À 100 % : une corde de 12 px gomme un tremblement de ± 4 px.
    let (at_100, _) = run(s, 1.0, &pts, None);
    assert!(amplitude(&at_100) < 3.0, "{}", amplitude(&at_100));
    // Zoomé à 400 % : 12 px d'écran font 3 px du dessin, le tremblement passe (en bonne partie :
    // la corde reste 3 px derrière la pointe).
    let (at_400, _) = run(s, 4.0, &pts, None);
    assert!(amplitude(&at_400) > amplitude(&at_100) + 1.0 && amplitude(&at_400) > 3.5, "{} contre {}", amplitude(&at_400), amplitude(&at_100));
    // En pixels du dessin (comme avant) : le zoom ne change rien.
    s.smooth_screen = false;
    let (fixed, _) = run(s, 4.0, &pts, None);
    assert!((amplitude(&fixed) - amplitude(&at_100)).abs() < 1e-3);
    // La moyenne sur une distance suit la même règle (20 px : trois points à 100 %, un seul à
    // 400 % ; un nombre pair de points en dents de scie ferait une moyenne nulle).
    s.smooth_kind = SmoothKind::AverageDistance;
    s.smooth_px = 20.0;
    s.smooth_screen = true;
    let (avg_100, _) = run(s, 1.0, &pts, None);
    let (avg_400, _) = run(s, 4.0, &pts, None);
    assert!(amplitude(&avg_100) < amplitude(&avg_400), "{} < {}", amplitude(&avg_100), amplitude(&avg_400));
}

#[test]
fn at_the_end_the_stroke_catches_the_pen_up_as_asked() {
    let mut s = base();
    s.smoothing = 0.8;
    s.smooth_time = false;
    // Une ligne qui tourne à angle droit juste avant le levé : le lissage est en retard.
    let mut pts: Vec<Pos2> = (0..=20).map(|k| pos2(40.0 + 8.0 * k as f32, 160.0)).collect();
    pts.extend((1..=5).map(|k| pos2(200.0, 160.0 - 8.0 * k as f32)));
    let tip = *pts.last().unwrap();
    let mut rasters = Vec::new();
    for (catch, reaches) in [(CatchUp::None, false), (CatchUp::Straight, true), (CatchUp::Curve, true)] {
        s.catch_up = catch;
        let (mut st, mut raster) = run(s, 1.0, &pts, None);
        let before = st.path.len();
        let mut dirty = Vec::new();
        st.finish(&mut raster, &mut dirty);
        let last = st.path.last().unwrap().pos();
        assert_eq!(last.distance(tip) < 0.01, reaches, "{catch:?} : fin à {last:?}");
        match catch {
            CatchUp::None => assert_eq!(st.path.len(), before),
            CatchUp::Straight => assert_eq!(st.path.len(), before + 1, "un segment droit"),
            CatchUp::Curve => {
                assert!(st.path.len() >= before + 3, "{} points de rattrapage", st.path.len() - before);
                // La courbe reste entre le trait et la pointe (à une demi-distance près).
                let s0 = st.path[before - 1].pos();
                let reach = s0.distance(tip) * 0.5 + 0.01;
                for q in &st.path[before..] {
                    let q = q.pos();
                    assert!(q.x >= s0.x.min(tip.x) - reach && q.x <= s0.x.max(tip.x) + reach, "{q:?}");
                    assert!(q.y >= s0.y.min(tip.y) - reach && q.y <= s0.y.max(tip.y) + reach, "{q:?}");
                }
            }
        }
        rasters.push(raster);
    }
    let differ = |a: &Raster, b: &Raster| (0..320).flat_map(|y| (0..320).map(move |x| (x, y))).any(|(x, y)| a.get(x, y) != b.get(x, y));
    assert!(differ(&rasters[0], &rasters[1]), "sans rattrapage, le trait s'arrête avant");
    // Avec le Suivi, chaque pas vise déjà la pointe : la courbe se confond avec le segment droit.
    assert!(!differ(&rasters[1], &rasters[2]));
    // Avec une moyenne, la direction du trait lissé n'est pas celle de la pointe : la courbe
    // prolonge le trait, le segment droit coupe.
    s.smooth_kind = SmoothKind::AveragePoints;
    s.smooth_points = 8;
    let ends: Vec<Raster> = [CatchUp::Straight, CatchUp::Curve]
        .into_iter()
        .map(|c| {
            s.catch_up = c;
            let (mut st, mut raster) = run(s, 1.0, &pts, None);
            st.finish(&mut raster, &mut Vec::new());
            assert!(st.path.last().unwrap().pos().distance(tip) < 0.01);
            raster
        })
        .collect();
    assert!(differ(&ends[0], &ends[1]));
}

#[test]
fn deferred_smoothing_draws_raw_then_replays_smoothed_at_the_end() {
    let mut s = base();
    s.smoothing = 0.6;
    let pts = wobble(40);
    // En direct : le trait lissé de référence.
    let (mut live, mut live_raster) = run(s, 1.0, &pts, Some(0.004));
    let mut dirty = Vec::new();
    live.finish(&mut live_raster, &mut dirty);
    let smoothed_amplitude = amplitude(&live);
    assert!(smoothed_amplitude < 6.0, "{smoothed_amplitude}");
    // Après le trait : pendant le geste, le trait brut passe par les points reçus, sans retard…
    s.smooth_deferred = true;
    let (mut st, mut raster) = run(s, 1.0, &pts, Some(0.004));
    assert!(st.path.iter().zip(&pts).all(|(a, b)| a.pos().distance(*b) < 1e-3), "brut pendant le geste");
    assert!(amplitude(&st) > 7.0);
    let (_, raw_tip, _) = st.tip().unwrap();
    assert_eq!(raw_tip, *pts.last().unwrap());
    // … et au levé, il est rejoué lissé : la même image, pixel pour pixel, que le lissage en
    // direct ; le trait brut a disparu ; le journal du banc est intact.
    let mut dirty = Vec::new();
    let edit = st.finish(&mut raster, &mut dirty);
    assert!((amplitude(&st) - smoothed_amplitude).abs() < 1e-3, "{} contre {smoothed_amplitude}", amplitude(&st));
    let same = (0..320).flat_map(|y| (0..320).map(move |x| (x, y))).all(|(x, y)| raster.get(x, y) == live_raster.get(x, y));
    assert!(same, "même image que le lissage en direct");
    assert!(edit.is_some());
    assert_eq!(st.raw_log.len(), pts.len());
    assert!(!dirty.is_empty());
    // Sans lissage réglé, rien n'est rejoué : le trait brut reste.
    let mut plain = base();
    plain.smooth_deferred = true;
    let (mut st2, mut raster2) = run(plain, 1.0, &pts, Some(0.004));
    st2.finish(&mut raster2, &mut dirty);
    assert!(amplitude(&st2) > 7.0);
}

#[test]
fn old_brush_settings_read_with_the_new_smoothing_options() {
    let s: BrushSettings = serde_json::from_str(r#"{"taille":8.0,"lissage":0.3}"#).unwrap();
    assert!(s.smooth_time && s.smooth_screen && !s.smooth_deferred && s.catch_up == CatchUp::Curve);
    let text = serde_json::to_string(&s).unwrap();
    assert!(text.contains("\"rattrapage\":\"courbe\"") && text.contains("\"lissage_differe\":false") && text.contains("\"suivi_temps\":true"), "{text}");
    let back: BrushSettings = serde_json::from_str(&text).unwrap();
    assert_eq!(back, s);
}
