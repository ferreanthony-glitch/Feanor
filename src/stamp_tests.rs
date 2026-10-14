//! Tests des tampons (5e) : suite des tampons le long du trait, pose (couleur, miroir, angle),
//! tampon de duplication (copie exacte, sans accumulation).

use std::collections::HashMap;

use eframe::egui::{pos2, vec2, Color32};

use crate::raster::{premultiply, Raster};
use crate::stamp::{self, CloneSource, CloneStroke, Cycle, Dab, StampImage, StampSettings, StampTrail};

fn plain() -> StampSettings {
    StampSettings { random_size: 0.0, random_angle: 0.0, random_spacing: 0.0, min_size: 1.0, ..StampSettings::default() }
}

#[test]
fn stamps_follow_the_stroke_at_regular_intervals() {
    let s = StampSettings { size: 10.0, spacing: 1.0, ..plain() };
    let mut t = StampTrail::new(s, 1, 7, 0);
    let mut dabs = t.add_point(pos2(0.0, 0.0), 1.0);
    // Le trait arrive par petits bouts : les tampons tombent quand même tous les 10 pixels.
    for k in 1..=20 {
        dabs.extend(t.add_point(pos2(k as f32 * 5.0, 0.0), 1.0));
    }
    let xs: Vec<f32> = dabs.iter().map(|d| d.center.x.round()).collect();
    assert_eq!(xs, (0..=10).map(|k| k as f32 * 10.0).collect::<Vec<_>>());
    assert!(dabs.iter().all(|d| d.size == 10.0 && d.angle == 0.0));
    // Pression faible : tampons plus petits (taille min 50 %).
    let mut t = StampTrail::new(StampSettings { size: 10.0, min_size: 0.5, ..plain() }, 1, 7, 0);
    assert_eq!(t.add_point(pos2(0.0, 0.0), 0.0)[0].size, 5.0);
}

#[test]
fn several_images_cycle() {
    let mut t = StampTrail::new(StampSettings { size: 4.0, ..plain() }, 3, 1, 0);
    let mut images = vec![t.add_point(pos2(0.0, 0.0), 1.0)[0].image];
    images.extend(t.add_point(pos2(20.0, 0.0), 1.0).iter().map(|d| d.image));
    assert_eq!(&images[..6], &[0, 1, 2, 0, 1, 2]);
    assert_eq!(t.next_image(), images.len() % 3, "le cycle continue au trait suivant");
    let mut r = StampTrail::new(StampSettings { size: 4.0, cycle: Cycle::Random, ..plain() }, 3, 99, 0);
    r.add_point(pos2(0.0, 0.0), 1.0);
    let random: Vec<usize> = r.add_point(pos2(400.0, 0.0), 1.0).iter().map(|d| d.image).collect();
    assert!(random.iter().all(|&i| i < 3));
    assert!((0..3).all(|i| random.contains(&i)), "toutes les images sortent : {random:?}");
    // Variations au hasard : des tailles différentes, dans les bornes.
    let mut v = StampTrail::new(StampSettings { size: 20.0, random_size: 0.5, ..plain() }, 1, 5, 0);
    v.add_point(pos2(0.0, 0.0), 1.0);
    let sizes: Vec<f32> = v.add_point(pos2(500.0, 0.0), 1.0).iter().map(|d| d.size).collect();
    assert!(sizes.iter().all(|s| (10.0..=30.0).contains(s)));
    assert!(sizes.iter().any(|s| (s - 20.0).abs() > 1.0));
}

/// Image 4×2 : moitié gauche opaque, moitié droite transparente.
fn half() -> StampImage {
    let mut alpha = vec![0.0; 8];
    for y in 0..2 {
        alpha[y * 4] = 1.0;
        alpha[y * 4 + 1] = 1.0;
    }
    StampImage::from_alpha("moitié", 4, 2, &alpha)
}

#[test]
fn a_stamp_is_tinted_flipped_and_turned() {
    let img = half();
    let dab = Dab { center: pos2(50.0, 50.0), size: 40.0, angle: 0.0, image: 0 };
    let s = StampSettings { roundness: 1.0, ..plain() };
    let mut r = Raster::new(100, 100);
    let mut before = HashMap::new();
    let dirty = stamp::place(&mut r, &img, &dab, &s, [200, 0, 0], &mut before);
    assert!(!dirty.is_empty() && before.len() == dirty.len());
    // Forme blanche : la couleur courante ; la moitié gauche seulement.
    assert_eq!(r.get(40, 50), Color32::from_rgb(200, 0, 0));
    assert_eq!(r.get(60, 50).a(), 0);
    // Miroir horizontal : la moitié droite.
    let mut m = Raster::new(100, 100);
    stamp::place(&mut m, &img, &dab, &StampSettings { flip_x: true, ..s }, [200, 0, 0], &mut HashMap::new());
    assert_eq!(m.get(40, 50).a(), 0);
    assert!(m.get(60, 50).a() > 200);
    // Tourné d'un quart de tour : la moitié opaque passe en haut.
    let mut q = Raster::new(100, 100);
    let turned = Dab { angle: std::f32::consts::FRAC_PI_2, ..dab };
    stamp::place(&mut q, &img, &turned, &s, [200, 0, 0], &mut HashMap::new());
    assert!(q.get(50, 42).a() > 200 && q.get(50, 58).a() == 0, "{:?} {:?}", q.get(50, 42), q.get(50, 58));
    // Image en couleurs : ses couleurs, sauf « teinter ».
    let colored = StampImage::from_rgba("vert".into(), 2, 2, &[0, 255, 0, 255].repeat(4));
    let mut c = Raster::new(100, 100);
    stamp::place(&mut c, &colored, &dab, &s, [200, 0, 0], &mut HashMap::new());
    assert_eq!(c.get(50, 50), Color32::from_rgb(0, 255, 0));
    stamp::place(&mut c, &colored, &dab, &StampSettings { tint: true, ..s }, [200, 0, 0], &mut HashMap::new());
    assert_eq!(c.get(50, 50), Color32::from_rgb(200, 0, 0));
}

#[test]
fn the_clone_stamp_copies_without_building_up() {
    // Source : un carré rouge en (10..30, 10..30).
    let mut src = Raster::new(128, 128);
    for y in 10..30 {
        for x in 10..30 {
            let idx = src.tile_index_of(x, y);
            src.tile_mut(idx)[(y % 64) * 64 + x % 64] = premultiply(220, 0, 0, 255);
        }
    }
    let mut dst = src.clone();
    // Pinceau en (80, 20), source à (20, 20) : décalage (-60, 0).
    let mut stroke = CloneStroke::new(CloneSource::Layer(src.clone()), vec2(-60.0, 0.0), 12.0, 1.0, 1.0);
    stroke.add_point(&mut dst, pos2(80.0, 20.0));
    assert_eq!(dst.get(80, 20), Color32::from_rgb(220, 0, 0), "copié");
    assert_eq!(dst.get(20, 20), Color32::from_rgb(220, 0, 0), "la source ne bouge pas");
    assert_eq!(dst.get(100, 20).a(), 0, "rien hors du pinceau");
    // Demi-opacité : repasser au même endroit ne fonce pas (pas d'accumulation).
    let mut once = src.clone();
    let mut a = CloneStroke::new(CloneSource::Layer(src.clone()), vec2(-60.0, 0.0), 12.0, 1.0, 0.5);
    a.add_point(&mut once, pos2(80.0, 20.0));
    let mut twice = src.clone();
    let mut b = CloneStroke::new(CloneSource::Layer(src.clone()), vec2(-60.0, 0.0), 12.0, 1.0, 0.5);
    b.add_point(&mut twice, pos2(80.0, 20.0));
    b.add_point(&mut twice, pos2(80.5, 20.0));
    b.add_point(&mut twice, pos2(80.0, 20.0));
    assert_eq!(once.get(80, 20), twice.get(80, 20));
    assert!(once.get(80, 20).a() > 100 && once.get(80, 20).a() < 160);
    assert!(!b.before.is_empty(), "tuiles d'avant gardées pour l'annulation");
}

#[test]
fn built_in_stamps() {
    let all = stamp::builtin();
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Point doux", "Feuille", "Brin d'herbe", "Étoile", "Caillou", "Nuage"]);
    for s in &all {
        assert!(s.mask);
        let covered = s.pixels.iter().filter(|p| p.a() > 128).count();
        assert!(covered > 300 && covered < s.w * s.h, "{} : {covered}", s.name);
        // Bords transparents (la forme tient dans l'image).
        assert_eq!(s.pixels[0].a(), 0, "{}", s.name);
    }
}
