//! Rendu des brosses, étapes 1 à 3 (`docs/AUDIT-RENDU-BROSSES.md` §5) : les pointes et papiers
//! fabriqués par Pelure, les brosses traditionnelles, la douceur du grain, le grain du crayon, le
//! papier qui suit le trait, et la relecture des anciens réglages.

use eframe::egui::pos2;

use crate::brush::{BrushSettings, PencilGrain, StrokeMode, StrokeState, Toucher};
use crate::brushtex::{self, BrushLibrary, GrayImage, Target, TIP_BRISTLES_FLAT, TIP_BRISTLES_ROUND, TIP_CHALK, TIP_CHARCOAL, TIP_GRAPHITE};
use crate::raster::Raster;

/// Un trait horizontal appuyé à `pressure`, de (20, 32) à (236, 32), sur une feuille de 256×64.
fn trait_(s: BrushSettings, lib: &BrushLibrary, pressure: f32, from: (f32, f32)) -> Raster {
    let mut raster = Raster::new(256, 64);
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true).with_textures(lib.textures(&s)).with_feel(Toucher::default());
    let mut dirty = Vec::new();
    for k in 0..=108 {
        st.add_point_at(&mut raster, pos2(from.0 + 2.0 * k as f32, from.1), pressure, k as f64 / 200.0, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    raster
}

/// Alphas des pixels d'une bande horizontale du trait.
fn alphas(r: &Raster, x0: usize, x1: usize, y0: usize, y1: usize) -> Vec<f32> {
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| r.get(x, y).a() as f32).collect()
}

fn mean(v: &[f32]) -> f32 {
    v.iter().sum::<f32>() / v.len().max(1) as f32
}

fn sd(v: &[f32]) -> f32 {
    let m = mean(v);
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

#[test]
fn pelure_makes_its_own_tips_and_papers() {
    let lib = BrushLibrary::builtin();
    for (id, cells) in [(TIP_GRAPHITE, 8), (TIP_CHARCOAL, 6), (TIP_CHALK, 6), (TIP_BRISTLES_FLAT, 4), (TIP_BRISTLES_ROUND, 4)] {
        let t = lib.tip(id).unwrap_or_else(|| panic!("pointe {id}"));
        let g = t.full();
        assert_eq!(g.width, g.height * cells, "pointe {id} : {cells} cases carrées");
        assert_eq!(brushtex::builtin_tip_cells(id), Some(cells as u32));
        let (lo, hi) = g.values.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(lo >= 0.0 && hi <= 1.0 && g.values.iter().all(|v| v.is_finite()), "pointe {id} : valeurs de {lo} à {hi}");
        // Chaque case dépose quelque chose, et aucune n'est un disque plein : il y a du creux.
        for c in 0..cells {
            let cell: Vec<f32> = (0..g.height).flat_map(|y| (0..g.height).map(move |x| (x, y))).map(|(x, y)| g.get(c * g.height + x, y)).collect();
            let m = mean(&cell);
            assert!(m > 0.05 && m < 0.9, "pointe {id}, case {c} : moyenne {m}");
        }
    }
    // Les papiers sont répartis par rang, et le papier à dessin a du relief à plusieurs échelles :
    // il est plus lisse de pixel à pixel que le bruit fin d'avant.
    let paper = |id| lib.textures(&BrushSettings { paper: Some(id), ..BrushSettings::brush() }).paper.unwrap();
    let roughness = |g: &GrayImage| {
        let mut d = 0.0;
        for y in 0..g.height {
            for x in 0..g.width - 1 {
                d += (g.get(x + 1, y) - g.get(x, y)).abs();
            }
        }
        d / (g.width * g.height) as f32
    };
    let fine = paper(brushtex::PAPER_FINE);
    let drawing = paper(brushtex::PAPER_DRAWING);
    assert!(roughness(&drawing) < 0.9 * roughness(&fine), "{} contre {}", roughness(&drawing), roughness(&fine));
    for id in [brushtex::PAPER_DRAWING, brushtex::PAPER_CANVAS, brushtex::PAPER_LAID] {
        let p = paper(id);
        let below = p.values.iter().filter(|&&v| v < 0.3).count() as f32 / p.values.len() as f32;
        assert!((below - 0.3).abs() < 0.02, "papier {id} réparti par rang : {below}");
    }
    // Déterministe : deux bibliothèques donnent les mêmes images (même clé dans les projets).
    let again = BrushLibrary::builtin();
    assert_eq!(lib.tip(TIP_GRAPHITE).unwrap().full(), again.tip(TIP_GRAPHITE).unwrap().full());
    assert_eq!(paper(brushtex::PAPER_DRAWING).as_ref(), lib.textures(&BrushSettings { paper: Some(brushtex::PAPER_DRAWING), ..BrushSettings::brush() }).paper.unwrap().as_ref());
    let _ = again;
}

#[test]
fn the_traditional_brushes_are_listed_and_the_round_brush_stays_first() {
    let lib = BrushLibrary::builtin();
    let names: Vec<&str> = lib.presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names[0], "Pinceau rond");
    for n in ["Crayon graphite", "Crayon de couleur", "Encre", "Feutre", "Fusain", "Craie", "Gouache", "Crayon net", "Gomme", "Gomme à grain"] {
        assert!(names.contains(&n), "{n} manque : {names:?}");
    }
    // Le Pinceau rond n'a pas changé (choix d'Anthony du 2026-10-16).
    assert_eq!(lib.presets[0].settings, BrushSettings::brush());
    let b = BrushSettings::brush();
    assert!(b.tip.is_none() && b.paper.is_none() && !b.build_up && b.power == 1.0 && b.pencil_grain == PencilGrain::None);
    // Les outils secs : un débit qui s'accumule sous un plafond, l'appui sur le débit, une pointe.
    for s in [BrushSettings::graphite(), BrushSettings::colour_pencil(), BrushSettings::charcoal(), BrushSettings::chalk()] {
        assert!(s.build_up && s.power < 1.0 && s.opacity <= 1.0 && s.tip.is_some() && s.paper.is_some(), "{s:?}");
        assert!(s.opacity_connection().is_some(), "l'appui fait le débit");
        assert!(s.paper_softness >= 0.25);
    }
    assert!(BrushSettings::graphite().opacity < 1.0, "le graphite ne fait jamais du noir pur");
    assert_eq!(BrushSettings::graphite().pencil_grain, PencilGrain::Soft);
    assert!(lib.presets.iter().all(|p| p.target != Target::Brush || p.settings.size <= 20.0));
}

#[test]
fn graphite_builds_up_and_keeps_grain() {
    let lib = BrushLibrary::builtin();
    let s = BrushSettings { smoothing: 0.0, ..BrushSettings::graphite() };
    // Un passage appuyé à fond : gris foncé mais pas noir (plafond 0,9 et débit qui s'accumule).
    let one = trait_(s, &lib, 1.0, (20.0, 32.0));
    let a1 = alphas(&one, 60, 200, 31, 33);
    assert!(mean(&a1) > 110.0 && a1.iter().all(|&a| a <= 230.0), "un passage : moyenne {}, max {}", mean(&a1), a1.iter().cloned().fold(0.0, f32::max));
    // Deux passages au même endroit (deux traits) : ça fonce.
    let mut two = one.clone();
    let mut st = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true).with_textures(lib.textures(&s));
    let mut dirty = Vec::new();
    for k in 0..=108 {
        st.add_point_at(&mut two, pos2(20.0 + 2.0 * k as f32, 32.0), 1.0, k as f64 / 200.0, &mut dirty);
    }
    st.finish(&mut two, &mut dirty);
    let a2 = alphas(&two, 60, 200, 31, 33);
    assert!(mean(&a2) > mean(&a1) + 20.0, "deux passages {} contre un {}", mean(&a2), mean(&a1));
    // Effleuré : clair et granuleux (l'alpha varie le long du trait, ce n'est pas un aplat).
    let light = trait_(s, &lib, 0.2, (20.0, 32.0));
    let al = alphas(&light, 60, 200, 31, 33);
    assert!(mean(&al) < mean(&a1) * 0.6, "effleuré {} contre appuyé {}", mean(&al), mean(&a1));
    assert!(sd(&al) > 12.0, "du grain : écart-type {}", sd(&al));
    // Le Crayon net d'avant, lui, est un aplat uniforme au même appui.
    let net = trait_(BrushSettings { smoothing: 0.0, ..BrushSettings::pencil() }, &lib, 0.2, (20.0, 32.0));
    let an = alphas(&net, 60, 200, 32, 33);
    assert!(sd(&an) < 6.0, "crayon net uniforme : {}", sd(&an));
}

#[test]
fn pencil_grain_textures_a_plain_brush_and_the_old_settings_do_not_change() {
    let lib = BrushLibrary::builtin();
    let plain = BrushSettings { size: 8.0, pressure_size: false, smoothing: 0.0, opacity: 0.8, ..BrushSettings::brush() };
    let flat = trait_(plain, &lib, 0.5, (20.0, 32.0));
    let grainy = trait_(BrushSettings { pencil_grain: PencilGrain::Medium, ..plain }, &lib, 0.5, (20.0, 32.0));
    let (af, ag) = (alphas(&flat, 40, 220, 30, 34), alphas(&grainy, 40, 220, 30, 34));
    assert!(sd(&af) < 3.0, "sans grain : uniforme ({})", sd(&af));
    assert!(sd(&ag) > 15.0 && mean(&ag) < mean(&af), "avec grain : du relief ({}, {})", sd(&ag), mean(&ag));
    // Plus le grain est dur, plus il retient de couleur ; la soie en retient peu.
    let silk = alphas(&trait_(BrushSettings { pencil_grain: PencilGrain::Silk, ..plain }, &lib, 0.5, (20.0, 32.0)), 40, 220, 30, 34);
    let hard = alphas(&trait_(BrushSettings { pencil_grain: PencilGrain::Hard, ..plain }, &lib, 0.5, (20.0, 32.0)), 40, 220, 30, 34);
    assert!(mean(&silk) > mean(&ag) && mean(&ag) > mean(&hard), "{} > {} > {}", mean(&silk), mean(&ag), mean(&hard));
    // Collé à la feuille : le même trait deux fois donne les mêmes creux ; décalé de 7 px, d'autres.
    let again = trait_(BrushSettings { pencil_grain: PencilGrain::Medium, ..plain }, &lib, 0.5, (20.0, 32.0));
    assert_eq!(alphas(&again, 40, 220, 30, 34), ag);
    // Un vieux réglage (sans les champs nouveaux) se relit tel quel : douceur d'avant, pas de grain
    // du crayon, papier collé à la feuille — et un trait avec papier rend la même image qu'avant.
    let old_json = r#"{"taille":12.0,"grain":1.0,"papier":2,"pression_taille":false,"lissage":0.0}"#;
    let old: BrushSettings = serde_json::from_str(old_json).unwrap();
    assert_eq!((old.paper_softness, old.pencil_grain, old.paper_follow), (brushtex::GRAIN_SOFTNESS_OLD, PencilGrain::None, false));
    let reference = BrushSettings { size: 12.0, grain: 1.0, paper: Some(brushtex::PAPER_COARSE), pressure_size: false, smoothing: 0.0, ..BrushSettings::brush() };
    assert_eq!(old, reference);
    let json = serde_json::to_string(&BrushSettings::graphite()).unwrap();
    let back: BrushSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back, BrushSettings::graphite());
}

#[test]
fn the_pressure_of_a_vector_point_is_found_back_from_its_radius() {
    use crate::brush::{Connection, Curve, Source};
    use crate::vector::pressure_for_radius;
    // Pression simple : la droite de la taille mini à la taille.
    let simple = BrushSettings { size: 10.0, pressure_size: true, min_size: 0.2, ..BrushSettings::brush() };
    for p in [0.0f32, 0.3, 0.75, 1.0] {
        let r = 5.0 * (0.2 + 0.8 * p);
        assert!((pressure_for_radius(&simple, r) - p).abs() < 1e-3, "{p}");
    }
    // Une connexion à courbe (l'encre, les brosses importées) : retrouvée par dichotomie.
    let mut curved = BrushSettings { size: 10.0, ..BrushSettings::brush() };
    curved.dynamics.size = Connection { source: Source::Pressure, curve: Curve::from_points(&[[0.0, 0.0], [0.4, 0.15], [0.8, 0.7], [1.0, 1.0]]), min: 0.04, ..Default::default() };
    for p in [0.1f32, 0.4, 0.6, 0.95] {
        let r = 5.0 * curved.size_connection().unwrap().response(p);
        assert!((pressure_for_radius(&curved, r) - p).abs() < 1e-3, "{p}");
    }
    // Inversée : la taille décroît avec l'appui.
    let mut inv = curved;
    inv.dynamics.size.invert = true;
    let r = 5.0 * inv.size_connection().unwrap().response(0.3);
    assert!((pressure_for_radius(&inv, r) - 0.3).abs() < 1e-3);
    // Sans connexion : appui plein.
    assert_eq!(pressure_for_radius(&BrushSettings { pressure_size: false, ..BrushSettings::brush() }, 1.0), 1.0);
}

#[test]
fn the_paper_can_follow_the_stroke() {
    let lib = BrushLibrary::builtin();
    let glued = BrushSettings { size: 10.0, pressure_size: false, smoothing: 0.0, grain: 1.0, paper: Some(brushtex::PAPER_DRAWING), paper_softness: 0.3, ..BrushSettings::brush() };
    let follow = BrushSettings { paper_follow: true, ..glued };
    // Collé à la feuille : décaler le trait de 6 px change les creux sous lui.
    let g0 = alphas(&trait_(glued, &lib, 0.5, (20.0, 32.0)), 40, 200, 30, 34);
    let g6 = alphas(&trait_(glued, &lib, 0.5, (26.0, 32.0)), 46, 206, 30, 34);
    assert!(g0 != g6);
    // Qui suit le trait : les mêmes creux, où que le trait commence.
    let f0 = alphas(&trait_(follow, &lib, 0.5, (20.0, 32.0)), 40, 200, 30, 34);
    let f6 = alphas(&trait_(follow, &lib, 0.5, (26.0, 32.0)), 46, 206, 30, 34);
    assert_eq!(f0, f6);
    assert!(sd(&f0) > 10.0, "il y a bien du grain : {}", sd(&f0));
}
