//! Sélection par couleur : pixels de même couleur, éléments vectoriels de même couleur, zone
//! tirée d'un masque.

use eframe::egui::{pos2, Color32};

use crate::colorsel::{same_color, similar};
use crate::pixsel::PixelMask;
use crate::vector::{Element, VPoint, VectorDrawing};

fn stroke(color: [u8; 3], y: f32) -> Element {
    Element::Stroke {
        points: vec![VPoint { x: 0.0, y, r: 2.0 }, VPoint { x: 50.0, y, r: 2.0 }],
        color,
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

#[test]
fn every_pixel_of_the_clicked_colour_even_apart() {
    // Rouge à gauche et à droite, séparés par du bleu : la sélection globale prend les deux.
    let (r, b) = (Color32::from_rgb(200, 0, 0), Color32::from_rgb(0, 0, 200));
    let src = vec![r, b, r, r, b, b];
    let m = similar(&src, 3, 2, 0, 0, 10);
    assert_eq!(m, vec![true, false, true, true, false, false]);
    // La tolérance laisse passer une teinte voisine.
    let near = Color32::from_rgb(205, 4, 0);
    assert_eq!(similar(&[r, near], 2, 1, 0, 0, 8), vec![true, true]);
    assert_eq!(similar(&[r, near], 2, 1, 0, 0, 2), vec![true, false]);
    // Contiguë : le pot de peinture, qui s'arrête au bleu (le rouge du dessous touche le clic,
    // celui de droite non).
    let flood = crate::fill::flood(&src, 3, 2, 0, 0, 10);
    assert_eq!(flood, vec![true, false, false, true, false, false]);
}

#[test]
fn a_mask_from_pixels_is_cropped_to_them() {
    let mut bools = vec![false; 6 * 4];
    bools[6 + 2] = true;
    bools[2 * 6 + 4] = true;
    let m = PixelMask::from_bools(&bools, 6, 4).expect("zone");
    assert_eq!((m.x0, m.y0, m.w, m.h), (2, 1, 3, 2));
    assert_eq!(m.at(2, 1), 1.0);
    assert_eq!(m.at(4, 2), 1.0);
    assert_eq!(m.at(3, 1), 0.0);
    assert_eq!(m.at(0, 0), 0.0);
    assert!(PixelMask::from_bools(&[false; 4], 2, 2).is_none(), "rien de choisi : pas de zone");
}

#[test]
fn every_element_of_the_clicked_colour() {
    let (red, blue) = ([200, 0, 0], [0, 0, 200]);
    let d = VectorDrawing {
        elements: vec![
            stroke(red, 0.0),
            stroke(blue, 10.0),
            Element::Fill { contours: vec![vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]]], color: red, opacity: 1.0 },
            stroke(red, 20.0),
        ],
    };
    assert_eq!(same_color(&d, 0), vec![0, 2, 3], "traits et zones de la même couleur");
    assert_eq!(same_color(&d, 1), vec![1]);
    assert!(same_color(&d, 9).is_empty());
    assert_eq!(d.hit_test(pos2(25.0, 10.0), 1.0), Some(1));
}
