//! Effets de la pile au-delà des flous : réglages de couleur, mosaïque, ombre, et ce qui ne
//! coûte rien quand ils ne changent rien. Le dessin d'origine ne change jamais.

use eframe::egui::Color32;

use crate::fx::{self, Effect, EffectKind, FxStep, FAMILIES};
use crate::fxfilters as f;
use crate::raster::{Raster, TILE};

/// Une feuille de 128 × 128 avec un pixel `c` en (x, y).
fn sheet(points: &[(usize, usize, Color32)]) -> Raster {
    let mut r = Raster::new(128, 128);
    for &(x, y, c) in points {
        let idx = (y / TILE) * r.tiles_x + x / TILE;
        r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
    }
    r
}

#[test]
fn colour_adjustments_change_only_what_is_drawn() {
    let red = Color32::from_rgb(255, 0, 0);
    let r = sheet(&[(5, 5, red)]);
    let bw = f::black_white(&r, 100.0).get(5, 5);
    assert!(bw.r() == bw.g() && bw.g() == bw.b() && (70..=82).contains(&bw.r()), "gris de luminance : {bw:?}");
    assert_eq!(f::invert(&r, 100.0).get(5, 5), Color32::from_rgb(0, 255, 255));
    let levels = f::levels(&sheet(&[(5, 5, Color32::from_gray(128))]), 0.0, 128.0, 1.0).get(5, 5);
    assert_eq!(levels, Color32::WHITE, "128 devient le blanc");
    assert_eq!(f::threshold(&sheet(&[(5, 5, Color32::from_gray(100))]), 128.0).get(5, 5), Color32::BLACK);
    let hue = f::hue_saturation(&r, 120.0, 0.0, 0.0).get(5, 5);
    assert!(hue.g() > 250 && hue.r() < 5, "le rouge tourné de 120° est vert : {hue:?}");
    // Le vide reste vide, la couverture ne change pas.
    let half = sheet(&[(5, 5, Color32::from_rgba_premultiplied(128, 0, 0, 128))]);
    assert_eq!(f::invert(&half, 100.0).get(5, 5).a(), 128);
    assert_eq!(f::brightness_contrast(&r, 50.0, 50.0).get(60, 60), Color32::TRANSPARENT);
}

#[test]
fn mosaic_and_shadow_spread_beyond_the_drawing() {
    // Mosaïque de 4 : le carré de (4, 4) à (8, 8) prend la moyenne, le pixel seul s'y étale.
    let r = sheet(&[(5, 5, Color32::WHITE)]);
    let m = f::mosaic(&r, 4.0);
    assert_eq!(m.get(4, 4), m.get(7, 7));
    assert!(m.get(4, 4).a() > 0 && m.get(4, 4).a() < 255, "un seizième du blanc");
    assert_eq!(m.get(8, 8), Color32::TRANSPARENT, "hors du carré");
    // Ombre portée de (10, 10), sans flou, à 50 % : sous le dessin, décalée.
    let s = f::drop_shadow(&sheet(&[(20, 20, Color32::WHITE)]), 10.0, 10.0, 0.0, 50.0);
    assert_eq!(s.get(20, 20), Color32::WHITE, "le dessin reste dessus");
    assert_eq!(s.get(30, 30).a(), 128, "l'ombre, à moitié");
    assert_eq!((s.get(30, 30).r(), s.get(30, 30).g()), (0, 0), "noire");
}

#[test]
fn effects_that_change_nothing_cost_nothing_and_the_drawing_never_changes() {
    // Réglés par défaut, les réglages de couleur ne changent rien : ils ne sont pas calculés.
    let fx = vec![Effect::new(EffectKind::Levels), Effect::new(EffectKind::BrightnessContrast), Effect::new(EffectKind::HueSaturation)];
    assert!(fx::steps_at(&fx, 0).is_empty());
    // Décoché : il reste sur le calque, avec ses réglages, sans s'appliquer.
    let mut neg = Effect::new(EffectKind::Invert);
    neg.enabled = false;
    assert!(fx::steps_at(&[neg], 0).is_empty());
    // Appliqué : une copie filtrée, le dessin d'origine est intact.
    let r = sheet(&[(5, 5, Color32::from_rgb(255, 0, 0))]);
    let out = fx::apply(&r, &[FxStep { kind: EffectKind::Invert, values: vec![100.0] }]);
    assert_eq!(r.get(5, 5), Color32::from_rgb(255, 0, 0));
    assert_ne!(out.get(5, 5), r.get(5, 5));
}

#[test]
fn every_effect_has_a_family_and_is_saved_under_its_own_name() {
    for k in EffectKind::ALL {
        assert!(FAMILIES.contains(&k.family()), "{k:?}");
        assert!(!k.params().is_empty());
        let text = serde_json::to_string(&k).unwrap();
        let back: EffectKind = serde_json::from_str(&text).unwrap();
        assert_eq!(back, k);
    }
    assert_eq!(serde_json::to_string(&EffectKind::DropShadow).unwrap(), "\"ombre_portee\"");
}
