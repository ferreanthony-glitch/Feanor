//! Colonne de gauche du montage : la largeur permise, et ce que porte la rangée d'un calque.
//!
//! Adaptation du 2026-10-28 (branche `timeline-repliee`) : la rangée d'un calque ne montre plus selon sa largeur l'opacité, le verrou, la
//! pelure, le menu ⋮, le mode de fusion et la boucle (anciens essais `head_layout`) ; elle ne montre que l'œil, la pastille de la sorte du
//! calque et son nom, le reste étant au clic droit. Les essais d'avant sont remplacés par ceux de la nouvelle mise en page.

use eframe::egui::{pos2, vec2, Rect};

use crate::montagehead::{clamp_head_w, HEAD_ALL, HEAD_MAX, HEAD_MIN};
use crate::montagerows::LAYER_H;
use crate::pistes::{head_layout, name_size, FOLDED_H};

/// La rangée d'un calque dans une colonne large de `column` (comme `row_headers` : 8 points de moins, la hauteur d'une piste moins 2).
fn row(column: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(2.0, 101.0), vec2(column - 8.0, h - 2.0))
}

#[test]
fn the_row_of_a_layer_carries_the_eye_the_dot_and_the_name_without_overlapping() {
    for h in [LAYER_H, FOLDED_H] {
        for column in [HEAD_MIN, 130.0, 180.0, HEAD_ALL, HEAD_MAX] {
            let r = row(column, h);
            let at = head_layout(r, h == FOLDED_H);
            for part in [at.eye, at.name] {
                assert!(r.contains_rect(part), "colonne {column}, hauteur {h} : {part:?} sort de {r:?}");
            }
            assert!(r.contains(at.dot), "la pastille est dans la rangée");
            assert!(at.eye.right() <= at.dot.x - at.dot_r + 0.5, "l'œil est avant la pastille");
            assert!(at.dot.x + at.dot_r <= at.name.left(), "la pastille est avant le nom");
            assert!(at.name.width() >= 30.0, "le nom garde de la place : {}", at.name.width());
            assert!((at.name.center().y - r.center().y).abs() < 0.5, "le nom est centré dans la hauteur");
        }
    }
    assert!(name_size(true) < name_size(false), "le nom d'une bande fine est plus petit");
}

#[test]
fn the_width_read_from_the_settings_is_kept_in_bounds() {
    assert_eq!(clamp_head_w(10.0), HEAD_MIN);
    assert_eq!(clamp_head_w(5000.0), HEAD_MAX);
    assert_eq!(clamp_head_w(f32::NAN), HEAD_MIN);
    assert_eq!(clamp_head_w(250.0), 250.0);
}
