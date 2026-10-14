//! Tests du panneau Couleur (6b) : conversions teinte / saturation / valeur.

use crate::colorui::{hsv_to_rgb, rgb_to_hsv};

#[test]
fn hsv_round_trip() {
    for c in [[255, 0, 0], [0, 255, 0], [0, 0, 255], [24, 24, 28], [240, 150, 40], [255, 255, 255], [0, 0, 0], [130, 70, 170]] {
        assert_eq!(hsv_to_rgb(rgb_to_hsv(c)), c, "{c:?}");
    }
    assert_eq!(rgb_to_hsv([255, 0, 0]), [0.0, 1.0, 1.0]);
    let [h, s, v] = rgb_to_hsv([0, 0, 255]);
    assert!((h - 240.0).abs() < 1e-3 && s == 1.0 && v == 1.0);
    // Gris : pas de saturation ; noir : pas de valeur.
    assert_eq!(rgb_to_hsv([128, 128, 128])[1], 0.0);
    assert_eq!(rgb_to_hsv([0, 0, 0]), [0.0, 0.0, 0.0]);
    // Teinte hors bornes : ramenée sur le cercle.
    assert_eq!(hsv_to_rgb([360.0, 1.0, 1.0]), [255, 0, 0]);
    assert_eq!(hsv_to_rgb([-120.0, 1.0, 1.0]), [0, 0, 255]);
}
