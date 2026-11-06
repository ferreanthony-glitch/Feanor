//! Essais de la place des numéros de plan (`numerosmarge.rs`) : dans la marge au-dessus de A, à une place fixe, jamais sur une image, un
//! cadre, la zone balayée d'un autre plan ni un autre numéro, même aux petits zooms.

use eframe::egui::{pos2, vec2, Rect};

use super::*;

fn case(x: f32, y: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(120.0, 68.0))
}

/// Un numéro de 40 × 20 au zoom de départ, qui s'amincit avec l'échelle.
fn mesure(sc: f32) -> Vec2 {
    vec2(40.0, 20.0) * sc
}

fn marges(obstacles: Vec<Obstacle>) -> Marges {
    Marges { obstacles, places: Vec::new() }
}

#[test]
fn the_number_sits_at_a_fixed_place_above_the_top_left_corner_of_a_when_the_margin_is_free() {
    let a = case(100.0, 100.0);
    let mut m = marges(vec![Obstacle::rect(a)]);
    let (r, sc) = m.choisir(a, a.top(), mesure).expect("une place");
    assert_eq!(sc, 1.0);
    assert_eq!(r.left_bottom(), pos2(a.left(), a.top() - 2.0), "la place d'origine : le coin haut-gauche de la case, dans la marge");
    // Même place à chaque appel (rien ne dépend du hasard ni de l'état d'avant).
    let mut m2 = marges(vec![Obstacle::rect(a)]);
    assert_eq!(m2.choisir(a, a.top(), mesure), Some((r, sc)));
}

#[test]
fn a_neighbouring_movement_that_spreads_over_the_margin_pushes_the_number_to_a_free_place_without_covering_it() {
    let a = case(100.0, 100.0);
    // L'enveloppe d'un autre plan : un grand losange qui couvre la marge au-dessus de A, à droite de son coin.
    let voisin = vec![pos2(130.0, 60.0), pos2(260.0, 60.0), pos2(260.0, 100.0), pos2(130.0, 100.0)];
    let obstacles = vec![Obstacle::rect(a), Obstacle::polygone(voisin)];
    let mut m = marges(obstacles.clone());
    let (r, _) = m.choisir(a, a.top(), mesure).expect("une place");
    for o in &obstacles {
        assert!(!o.touche(r), "le numéro recouvre un obstacle : {r:?}");
    }
    // Il reste tout près de A (dans l'écart à gauche du losange, au bord gauche de A) : il ne saute pas loin.
    assert!((r.left() - a.left()).abs() <= 40.0 && r.right() <= 130.3, "{r:?}");
    // Un obstacle sur tout le haut de A : il glisse à gauche, dans l'écart, ou monte, mais ne recouvre rien.
    let tout = vec![Obstacle::rect(a), Obstacle::rect(Rect::from_min_max(pos2(95.0, 40.0), pos2(260.0, 99.0)))];
    let mut m = marges(tout.clone());
    // (La zone du plan monte jusqu'à 40 : `haut`.)
    let (r, _) = m.choisir(a, 40.0, mesure).expect("une place");
    assert!(tout.iter().all(|o| !o.touche(r)), "{r:?}");
    assert!(r.right() <= 95.5 || r.bottom() <= 40.5, "à gauche de l'obstacle ou au-dessus de toute la zone : {r:?}");
    // Sans cette indication, rien ne tient : pas de numéro plutôt qu'un numéro sur la zone.
    let mut m = marges(tout.clone());
    assert_eq!(m.choisir(a, a.top(), mesure), None);
}

#[test]
fn a_small_zoom_makes_the_number_thinner_instead_of_putting_it_on_an_image() {
    // Une marge de 14 points de haut entre la rangée du dessus et A : un numéro de 20 n'y tient pas, un plus mince oui.
    let a = case(100.0, 100.0);
    let dessus = Rect::from_min_max(pos2(0.0, 0.0), pos2(600.0, 86.0));
    let mut m = marges(vec![Obstacle::rect(a), Obstacle::rect(dessus)]);
    let (r, sc) = m.choisir(a, a.top(), mesure).expect("une place plus petite");
    assert!(sc < 1.0, "amincie : {sc}");
    assert!(r.top() >= 86.0 - 0.3 && r.bottom() <= a.top(), "dans la marge : {r:?}");
    // Rien ne tient, même au plus petit : pas de numéro (jamais sur une image).
    let mut m = marges(vec![Obstacle::rect(a), Obstacle::rect(Rect::from_min_max(pos2(0.0, 0.0), pos2(600.0, 99.0)))]);
    assert_eq!(m.choisir(a, a.top(), mesure), None);
    // Le plus petit corps lisible : en dessous, la mesure rend rien et le numéro n'est pas essayé.
    let mut m = marges(vec![Obstacle::rect(a)]);
    let (_, sc) = m.choisir(a, a.top(), |sc| if 15.0 * sc < CORPS_MIN { Vec2::ZERO } else { mesure(sc) }).expect("place");
    assert_eq!(sc, 1.0);
}

#[test]
fn two_numbers_never_overlap_each_other() {
    let a = case(100.0, 100.0);
    let mut m = marges(vec![Obstacle::rect(a)]);
    let (r1, _) = m.choisir(a, a.top(), mesure).expect("première");
    let (r2, _) = m.choisir(a, a.top(), mesure).expect("deuxième");
    assert!(!r1.shrink(0.3).intersects(r2.shrink(0.3)), "{r1:?} {r2:?}");
}

#[test]
fn an_obstacle_polygon_only_counts_where_it_really_is() {
    // Un triangle : sa boîte couvre un coin que le triangle laisse libre.
    let tri = Obstacle::polygone(vec![pos2(0.0, 0.0), pos2(100.0, 0.0), pos2(0.0, 100.0)]);
    assert!(tri.touche(Rect::from_min_size(pos2(5.0, 5.0), vec2(10.0, 10.0))));
    assert!(!tri.touche(Rect::from_min_size(pos2(80.0, 80.0), vec2(10.0, 10.0))), "le coin de la boîte hors du triangle est libre");
    // Une simple touche du bord n'est pas un recouvrement.
    assert!(!Obstacle::rect(case(0.0, 0.0)).touche(Rect::from_min_size(pos2(0.0, -20.0), vec2(40.0, 20.0))));
}
