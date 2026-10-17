//! Effets « Déformation » et « Rendu » : où va le dessin, comment il s'éclaire, et rien ne
//! bouge quand ils ne font rien.

use eframe::egui::Color32;

use crate::fx::{self, Effect, EffectKind};
use crate::fxwarp as w;
use crate::raster::{Raster, TILE};

/// Une feuille de 200 × 100 avec des pixels `c` aux points donnés.
fn sheet(points: &[(usize, usize, Color32)]) -> Raster {
    let mut r = Raster::new(200, 100);
    for &(x, y, c) in points {
        let idx = (y / TILE) * r.tiles_x + x / TILE;
        r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
    }
    r
}

/// Le centre de masse (couverture) du dessin.
fn centroid(r: &Raster) -> (f32, f32) {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for y in 0..r.height {
        for x in 0..r.width {
            let a = r.get(x, y).a() as f32;
            sx += x as f32 * a;
            sy += y as f32 * a;
            n += a;
        }
    }
    (sx / n, sy / n)
}

#[test]
fn a_wave_moves_the_drawing_up_and_down() {
    // Une ligne horizontale ; vagues verticales de 40 px, amplitude 5 : à x = 10 (un quart de
    // vague), la ligne descend ou monte de 5 px.
    let line: Vec<(usize, usize, Color32)> = (0..200).map(|x| (x, 50, Color32::WHITE)).collect();
    let r = w::wave(&sheet(&line), 5.0, 40.0, 0.0);
    let col = |x: usize| (0..100).max_by_key(|&y| r.get(x, y).a()).unwrap();
    assert!((col(10) as i32 - 50).abs() >= 4, "à un quart de vague : {}", col(10));
    assert!((col(20) as i32 - 50).abs() <= 1, "à une demi-vague, elle revient : {}", col(20));
}

#[test]
fn twirl_and_pinch_leave_the_centre_in_place() {
    // Un carré autour du centre : tourner le garde centré ; pincer le resserre.
    let mut pts = Vec::new();
    for y in 40..60 {
        for x in 90..110 {
            pts.push((x, y, Color32::WHITE));
        }
    }
    let r = sheet(&pts);
    let t = w::twirl(&r, 90.0, 80.0, 50.0, 50.0);
    let (cx, cy) = centroid(&t);
    assert!((cx - 99.5).abs() < 1.5 && (cy - 49.5).abs() < 1.5, "toujours centré : {cx} {cy}");
    let p = w::pinch(&r, 80.0, 80.0, 50.0, 50.0);
    let area = |r: &Raster| (0..r.height).flat_map(|y| (0..r.width).map(move |x| (x, y))).filter(|&(x, y)| r.get(x, y).a() > 128).count();
    assert!(area(&p) < area(&r), "pincé, il rétrécit : {} < {}", area(&p), area(&r));
    let b = w::pinch(&r, -80.0, 80.0, 50.0, 50.0);
    assert!(area(&b) > area(&r), "bombé, il grossit");
}

#[test]
fn render_effects_light_and_shade_without_changing_coverage() {
    let grey = Color32::from_gray(128);
    let r = sheet(&[(20, 50, grey), (180, 50, grey)]);
    // Dégradé vers la droite : la droite fonce, la gauche à peine.
    let g = w::gradient(&r, 0.0, 100.0);
    assert!(g.get(180, 50).r() < 20 && g.get(20, 50).r() > 110);
    assert_eq!(g.get(180, 50).a(), 255, "la couverture reste");
    // Éclairage centré à gauche : la gauche s'éclaire, la droite s'assombrit.
    let l = w::lighting(&r, 10.0, 50.0, 60.0, 50.0);
    assert!(l.get(20, 50).r() > 128 && l.get(180, 50).r() < 128);
    // Nuages : des tons différents d'un point à l'autre, le vide reste vide.
    let c = w::clouds(&sheet(&(0..200).map(|x| (x, 50, grey)).collect::<Vec<_>>()), 20.0, 100.0, 1.0);
    let tones: std::collections::BTreeSet<u8> = (0..200).map(|x| c.get(x, 50).r()).collect();
    assert!(tones.len() > 10);
    assert_eq!(c.get(5, 5), Color32::TRANSPARENT);
}

#[test]
fn new_effects_that_do_nothing_are_skipped() {
    let mut fx = vec![Effect::new(EffectKind::Twirl), Effect::new(EffectKind::Wave)];
    fx[0].values[0] = 0.0;
    fx[1].values[0] = 0.0;
    assert!(fx::steps_at(&fx, 0).is_empty());
    for k in [EffectKind::Wave, EffectKind::Ripple, EffectKind::Twirl, EffectKind::Pinch, EffectKind::Clouds, EffectKind::Lighting, EffectKind::Gradient] {
        assert!(matches!(k.family(), "Déformation" | "Rendu"));
        assert!(k.changes(&Effect::new(k).values), "{k:?} change quelque chose avec ses réglages par défaut");
    }
}
