//! Colonne de gauche du montage : ce qui tient dans la rangée d'un calque selon la largeur
//! qu'on lui donne, et la largeur permise.

use eframe::egui::{pos2, vec2, Rect};

use crate::montagehead::{clamp_head_w, head_layout, HEAD_ALL, HEAD_MAX, HEAD_MIN};
use crate::montagerows::LAYER_H;

/// La rangée d'un calque dans une colonne large de `column` (comme `row_headers` : 8 points de
/// moins, la hauteur d'une piste moins 2).
fn row(column: f32) -> Rect {
    Rect::from_min_size(pos2(2.0, 101.0), vec2(column - 8.0, LAYER_H - 2.0))
}

#[test]
fn the_narrow_column_keeps_the_name_alone() {
    // La largeur d'avant : le nom (et le menu ⋮), rien d'autre, le nom centré dans la hauteur.
    let r = row(HEAD_MIN);
    let at = head_layout(r);
    assert!(at.icons.is_empty() && at.opacity.is_none() && at.blend.is_none() && at.after.is_none());
    assert!(at.menu.is_some(), "le menu ⋮ dit qu'il y a des réglages");
    assert!((at.name.center().y - r.center().y).abs() < 0.5);
    assert!(at.name.width() >= 56.0);
}

#[test]
fn widening_the_column_brings_the_options_in_order() {
    // Afficher et verrouiller avec l'opacité, puis la pelure, le mode de fusion, enfin la boucle.
    let mut seen = Vec::new();
    for column in (HEAD_MIN as i32..=HEAD_MAX as i32).step_by(2) {
        let at = head_layout(row(column as f32));
        let state = (at.icons.len(), at.opacity.is_some(), at.blend.is_some(), at.after.is_some());
        if seen.last() != Some(&state) {
            seen.push(state);
        }
    }
    let (first, last) = (seen[0], *seen.last().unwrap());
    assert_eq!(first, (0, false, false, false));
    assert_eq!(last, (3, true, true, true));
    // Rien ne disparaît en élargissant.
    for pair in seen.windows(2) {
        let ((i0, o0, b0, a0), (i1, o1, b1, a1)) = (pair[0], pair[1]);
        assert!(i1 >= i0 && o1 >= o0 && b1 >= b0 && a1 >= a0, "{pair:?}");
    }
    // Le double-clic sur la barre montre tout.
    let at = head_layout(row(HEAD_ALL));
    assert_eq!((at.icons.len(), at.blend.is_some(), at.after.is_some()), (3, true, true));
}

#[test]
fn the_options_stay_in_the_row_without_overlapping() {
    for column in [HEAD_MIN, 130.0, 180.0, 240.0, HEAD_ALL, HEAD_MAX] {
        let r = row(column);
        let at = head_layout(r);
        let mut all: Vec<Rect> = vec![at.name];
        all.extend(at.icons.iter().copied());
        all.extend([at.menu, at.opacity, at.blend, at.after].into_iter().flatten());
        for (i, a) in all.iter().enumerate() {
            assert!(r.contains_rect(*a), "colonne {column} : {a:?} sort de {r:?}");
            for b in &all[i + 1..] {
                assert!(!a.intersects(b.shrink(0.5)), "colonne {column} : {a:?} et {b:?} se chevauchent");
            }
        }
    }
}

#[test]
fn the_width_read_from_the_settings_is_kept_in_bounds() {
    assert_eq!(clamp_head_w(10.0), HEAD_MIN);
    assert_eq!(clamp_head_w(5000.0), HEAD_MAX);
    assert_eq!(clamp_head_w(f32::NAN), HEAD_MIN);
    assert_eq!(clamp_head_w(250.0), 250.0);
}
