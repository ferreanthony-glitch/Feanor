//! Les effets « Cinéma » (`fxcine.rs`) : flash, image d'impact, lueur diffuse, secousse, vignettage,
//! grain, aberration chromatique. Chacun est déterministe et fait ce que son nom dit.

use eframe::egui::Color32;

use crate::fx::{self, Effect, EffectKind, FxStep};
use crate::fxcine;
use crate::raster::Raster;

const W: usize = 192;
const H: usize = 128;

/// Une feuille avec des pixels posés (opaques).
fn sheet(pixels: &[(usize, usize, Color32)]) -> Raster {
    let mut r = Raster::new(W, H);
    for &(x, y, c) in pixels {
        let i = r.tile_index_of(x, y);
        r.tile_mut(i)[(y % 64) * 64 + x % 64] = c;
    }
    r
}

/// Toute la feuille d'une couleur (opaque).
fn flat(c: Color32) -> Raster {
    let mut r = Raster::new(W, H);
    for i in 0..r.tiles.len() {
        let (_, _, tw, th) = r.tile_bounds(i);
        let t = r.tile_mut(i);
        for y in 0..th {
            for x in 0..tw {
                t[y * 64 + x] = c;
            }
        }
    }
    r
}

fn step(kind: EffectKind, values: &[f32]) -> FxStep {
    FxStep { kind, values: values.to_vec() }
}

#[test]
fn flash_covers_the_whole_sheet_even_where_nothing_is_drawn() {
    let r = sheet(&[(10, 10, Color32::from_rgb(200, 0, 0))]);
    let out = fx::apply(&r, &[step(EffectKind::Flash, &[100.0, 255.0, 255.0, 255.0, 0.0])]);
    for (x, y) in [(0, 0), (10, 10), (191, 127), (100, 60)] {
        assert_eq!(out.get(x, y), Color32::WHITE, "({x}, {y})");
    }
    // À moitié : le milieu entre le dessin et le blanc ; le vide devient un blanc translucide.
    let half = fx::apply(&r, &[step(EffectKind::Flash, &[50.0, 255.0, 255.0, 255.0, 0.0])]);
    let p = half.get(10, 10);
    assert!((p.r() as i32 - 228).abs() <= 1 && (p.g() as i32 - 128).abs() <= 1 && p.a() == 255, "{p:?}");
    assert_eq!(half.get(0, 0).a(), 128);
}

#[test]
fn a_flash_at_zero_changes_nothing_and_is_not_computed() {
    assert!(!EffectKind::Flash.changes(&[0.0, 255.0, 255.0, 255.0, 0.0]));
    let mut e = Effect::new(EffectKind::Flash);
    e.values[0] = 0.0;
    assert!(fx::steps_at(&[e], 0).is_empty());
}

#[test]
fn the_screen_flash_only_lightens() {
    let r = flat(Color32::from_rgb(120, 60, 200));
    let out = fx::apply(&r, &[step(EffectKind::Flash, &[60.0, 255.0, 200.0, 0.0, 1.0])]);
    let p = out.get(5, 5);
    assert!(p.r() >= 120 && p.g() >= 60 && p.b() >= 200, "{p:?}");
    assert_ne!(p, Color32::from_rgb(120, 60, 200));
}

fn impact_values(mode: f32, burst: f32, lines: f32, seed: f32) -> Vec<f32> {
    vec![100.0, mode, 128.0, 230.0, 30.0, 30.0, 0.0, burst, lines, 50.0, 50.0, seed]
}

#[test]
fn the_impact_frame_inverts_or_posterises_to_black_and_white() {
    // Fond blanc, un trait noir : le négatif les échange ; le noir et blanc ne garde que 0 et 255.
    let mut r = flat(Color32::WHITE);
    for x in 20..40 {
        let i = r.tile_index_of(x, 30);
        r.tile_mut(i)[30 % 64 * 64 + x % 64] = Color32::BLACK;
    }
    let neg = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(0.0, 0.0, 0.0, 1.0))]);
    assert_eq!((neg.get(30, 30), neg.get(100, 100)), (Color32::WHITE, Color32::BLACK));
    let bw = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(1.0, 0.0, 0.0, 1.0))]);
    assert_eq!((bw.get(30, 30), bw.get(100, 100)), (Color32::BLACK, Color32::WHITE));
    let inv = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(2.0, 0.0, 0.0, 1.0))]);
    assert_eq!((inv.get(30, 30), inv.get(100, 100)), (Color32::WHITE, Color32::BLACK));
    // Un fond vide est du papier : il devient noir au négatif.
    let empty = fx::apply(&Raster::new(W, H), &[step(EffectKind::Impact, &impact_values(0.0, 0.0, 0.0, 1.0))]);
    assert_eq!(empty.get(10, 10), Color32::BLACK);
}

#[test]
fn the_impact_tint_colours_the_whites() {
    let r = flat(Color32::BLACK);
    let mut v = impact_values(0.0, 0.0, 0.0, 1.0);
    v[6] = 100.0;
    let out = fx::apply(&r, &[step(EffectKind::Impact, &v)]);
    // Le négatif d'un noir est blanc ; teinté à fond, il prend la teinte (230, 30, 30).
    let p = out.get(7, 7);
    assert!(p.r() > p.g() && p.r() > p.b() && p.r() >= 225, "{p:?}");
}

#[test]
fn the_burst_and_the_speed_lines_are_drawn_and_depend_only_on_the_seed() {
    let r = flat(Color32::from_gray(128));
    let plain = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(0.0, 0.0, 0.0, 3.0))]);
    let burst = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(0.0, 100.0, 0.0, 3.0))]);
    let lines = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(0.0, 0.0, 100.0, 3.0))]);
    let differ = |a: &Raster, b: &Raster| (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).filter(|&(x, y)| a.get(x, y) != b.get(x, y)).count();
    assert!(differ(&plain, &burst) > 500, "des rayons sont tracés");
    assert!(differ(&plain, &lines) > 500, "des lignes de vitesse sont tracées");
    // Même graine, même image ; autre graine, autres rayons.
    let again = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(0.0, 100.0, 0.0, 3.0))]);
    let other = fx::apply(&r, &[step(EffectKind::Impact, &impact_values(0.0, 100.0, 0.0, 4.0))]);
    assert_eq!(differ(&burst, &again), 0);
    assert!(differ(&burst, &other) > 100);
    // Le centre reste net (les rayons commencent un peu plus loin).
    assert_eq!(burst.get(W / 2, H / 2), plain.get(W / 2, H / 2));
}

#[test]
fn the_shake_moves_the_picture_by_the_offset() {
    let r = sheet(&[(50, 40, Color32::from_rgb(255, 0, 0))]);
    let out = fx::apply(&r, &[step(EffectKind::Shake, &[10.0, -6.0, 0.0, 100.0, 0.0])]);
    assert_eq!(out.get(60, 34), Color32::from_rgb(255, 0, 0), "le pixel est parti de 10 à droite et 6 en haut");
    assert_eq!(out.get(50, 40).a(), 0);
    // Immobile : l'image est la même, pixel pour pixel.
    let still = fxcine::shake(&r, 0.0, 0.0, 0.0, 100.0, 0.0);
    assert_eq!(still.get(50, 40), r.get(50, 40));
    // La rotation tourne autour du centre de la feuille.
    // Un quart de tour dans le sens des aiguilles d'une montre : le point à droite du centre passe en dessous.
    let turned = fxcine::shake(&sheet(&[(W / 2 + 40, H / 2, Color32::WHITE)]), 0.0, 0.0, 90.0, 100.0, 0.0);
    let near = |x: usize, y: usize| (0..3).flat_map(|j| (0..3).map(move |i| (i, j))).map(|(i, j)| turned.get(x + i - 1, y + j - 1).a()).max().unwrap();
    assert!(near(W / 2, H / 2 + 40) > 200, "un quart de tour, vers le bas");
    assert_eq!(near(W / 2, H / 2 - 40), 0);
}

#[test]
fn the_shake_blur_smears_along_the_offset() {
    let r = sheet(&[(80, 60, Color32::WHITE)]);
    let sharp = fxcine::shake(&r, 12.0, 0.0, 0.0, 100.0, 0.0);
    let smeared = fxcine::shake(&r, 12.0, 0.0, 0.0, 100.0, 100.0);
    let lit = |x: &Raster| (0..W).filter(|&i| x.get(i, 60).a() > 0).count();
    assert_eq!(lit(&sharp), 1);
    assert!(lit(&smeared) > 3, "le flou étale le pixel sur plusieurs colonnes");
}

#[test]
fn the_vignette_darkens_the_edges_only() {
    let r = flat(Color32::WHITE);
    let out = fx::apply(&r, &[step(EffectKind::Vignette, &[100.0, 40.0, 40.0, 50.0, 50.0, 100.0, 100.0])]);
    assert_eq!(out.get(W / 2, H / 2), Color32::WHITE, "le centre ne bouge pas");
    let corner = out.get(1, 1);
    assert!(corner.r() < 30 && corner.a() == 255, "le coin est presque noir : {corner:?}");
    // Un cadre plus étroit assombrit plus tôt.
    let narrow = fx::apply(&r, &[step(EffectKind::Vignette, &[100.0, 40.0, 40.0, 50.0, 50.0, 50.0, 100.0])]);
    assert!(narrow.get(W / 2 + 60, H / 2).r() < out.get(W / 2 + 60, H / 2).r());
    assert!(!EffectKind::Vignette.changes(&[0.0, 60.0, 60.0, 50.0, 50.0, 100.0, 100.0]));
}

#[test]
fn the_grain_changes_with_the_frame_and_the_seed_but_not_between_two_draws() {
    let r = flat(Color32::from_gray(128));
    let at = |frame: f32, seed: f32| fx::apply(&r, &[step(EffectKind::Grain, &[50.0, 1.0, seed, frame])]);
    let a = at(3.0, 1.0);
    let again = at(3.0, 1.0);
    let next = at(4.0, 1.0);
    let seeded = at(3.0, 2.0);
    let same = |x: &Raster, y: &Raster| (0..H).all(|j| (0..W).all(|i| x.get(i, j) == y.get(i, j)));
    assert!(same(&a, &again));
    assert!(!same(&a, &next), "une autre image, un autre grain");
    assert!(!same(&a, &seeded), "une autre graine, un autre grain");
    // Le grain varie autour du gris sans changer la couverture.
    let values: std::collections::BTreeSet<u8> = (0..W).map(|i| a.get(i, 5).r()).collect();
    assert!(values.len() > 8 && a.get(0, 0).a() == 255);
    // L'image est portée par les valeurs de l'effet (comme la variante du tremblement).
    let mut e = Effect::new(EffectKind::Grain);
    e.values[0] = 40.0;
    let s0 = fx::steps_at(std::slice::from_ref(&e), 0);
    let s7 = fx::steps_at(std::slice::from_ref(&e), 7);
    assert_eq!(s7[0].values.last(), Some(&7.0));
    assert_ne!(fx::fx_id(1, &s0), fx::fx_id(1, &s7), "un dessin dérivé par image");
}

#[test]
fn chromatic_aberration_separates_red_and_blue_away_from_the_centre() {
    // Un point blanc loin du centre : le rouge et le bleu en sortent, de côtés opposés.
    let r = sheet(&[(150, 64, Color32::WHITE)]);
    let out = fxcine::aberration(&r, 12.0, 50.0, 50.0);
    let row: Vec<Color32> = (100..W).map(|x| out.get(x, 64)).collect();
    let red_at = row.iter().position(|c| c.r() > c.b() + 20);
    let blue_at = row.iter().position(|c| c.b() > c.r() + 20);
    assert!(red_at.is_some() && blue_at.is_some(), "le rouge et le bleu se séparent");
    assert!(blue_at.unwrap() < red_at.unwrap(), "le rouge est plus loin du centre que le bleu");
    // Au centre, rien ne bouge.
    let centre = fxcine::aberration(&sheet(&[(W / 2, H / 2, Color32::WHITE)]), 12.0, 50.0, 50.0);
    let c = centre.get(W / 2, H / 2);
    assert!(c.r() >= 220 && c.g() == 255 && c.b() >= 220, "presque rien ne bouge au centre : {c:?}");
}

#[test]
fn the_bloom_lights_the_neighbourhood_of_bright_parts_only() {
    let dark = flat(Color32::from_gray(20));
    let mut r = dark.clone();
    for y in 60..68 {
        for x in 90..98 {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[y % 64 * 64 + x % 64] = Color32::WHITE;
        }
    }
    let out = fx::apply(&r, &[step(EffectKind::Bloom, &[12.0, 150.0, 140.0])]);
    // Près du point brillant, ça s'éclaire ; loin, rien ne bouge.
    assert!(out.get(88, 64).r() > 40, "halo près du point : {:?}", out.get(88, 64));
    assert_eq!(out.get(5, 5), Color32::from_gray(20));
    // Sans rien de brillant, rien ne change.
    let none = fx::apply(&dark, &[step(EffectKind::Bloom, &[12.0, 150.0, 140.0])]);
    assert_eq!(none.get(50, 50), Color32::from_gray(20));
    assert!(!EffectKind::Bloom.changes(&[12.0, 0.0, 140.0]));
}

#[test]
fn a_wide_bloom_uses_the_reduced_blur_and_keeps_the_sheet_size() {
    // Rayon 40 : flou sur une image réduite du quart ; le résultat garde sa taille.
    let mut r = flat(Color32::from_gray(10));
    for y in 50..70 {
        for x in 80..100 {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[y % 64 * 64 + x % 64] = Color32::WHITE;
        }
    }
    let out = fxcine::bloom(&r, 40.0, 100.0, 140.0);
    assert_eq!((out.width, out.height), (W, H));
    assert!(out.get(90, 60).r() == 255 && out.get(110, 60).r() > 10);
}

#[test]
fn the_cinema_effects_scale_with_the_preview_and_are_saved_by_name() {
    for (kind, name) in [
        (EffectKind::Flash, "flash"),
        (EffectKind::Impact, "image_impact"),
        (EffectKind::Bloom, "lueur_diffuse"),
        (EffectKind::Shake, "secousse"),
        (EffectKind::Vignette, "vignettage"),
        (EffectKind::Grain, "grain"),
        (EffectKind::Aberration, "aberration"),
    ] {
        assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
        assert_eq!(kind.family(), "Cinéma");
        assert!(crate::fxproxy::proxyable(&[step(kind, &[])]), "{kind:?}");
    }
    // Aperçu à moitié : les pixels sont divisés par deux.
    assert_eq!(crate::fxproxy::scaled_values(EffectKind::Shake, &[10.0, -6.0, 3.0, 100.0, 0.0], 1), vec![5.0, -3.0, 3.0, 100.0, 0.0]);
    assert_eq!(crate::fxproxy::scaled_values(EffectKind::Bloom, &[40.0, 100.0, 140.0], 2)[0], 10.0);
    assert_eq!(crate::fxproxy::scaled_values(EffectKind::Aberration, &[8.0, 50.0, 50.0], 1)[0], 4.0);
}

#[test]
fn an_effect_applied_to_a_reduced_sheet_matches_the_full_one_downscaled() {
    // Le flash (sans position) : l'aperçu réduit est la réduction du résultat entier.
    let values = [30.0, 255.0, 128.0, 0.0, 0.0];
    let full = fx::apply(&flat(Color32::from_rgb(40, 80, 160)), &[step(EffectKind::Flash, &values)]);
    let small = fx::apply_scaled(&crate::fxproxy::downscale(&flat(Color32::from_rgb(40, 80, 160)), 1), &[step(EffectKind::Flash, &values)], 1);
    let reduced = crate::fxproxy::downscale(&full, 1);
    assert_eq!((small.width, small.height), (reduced.width, reduced.height));
    for (x, y) in [(3, 3), (40, 20), (90, 60)] {
        assert_eq!(small.get(x, y), reduced.get(x, y));
    }
}
