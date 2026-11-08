//! Le dessin libre sur toute la planche des Vignettes (parties 101, 111, 118) avec « Peindre derrière » allumé (Anthony, 2026-11-08 : « je ne peux plus
//! dessiner dans tout le canevas vignettes » ; ses traits s'arrêtaient net au bord du champ des cases d'où ils partaient). Le trait vectoriel peint derrière
//! est posé sous les autres éléments du dessin, pas en dernier : la répartition au lâcher le laissait entier dans le plan d'origine, qui ne le montre que
//! dans son champ. Rien n'allait sur la planche.

use eframe::egui::vec2;

use crate::vector::Element;
use crate::traits_continus_tests::{continu, des_plans, planche, repos, sauver, tracer};

/// Un trait parti du centre de la case `k`, qui sort loin de son champ (au-dessus de la première rangée, ou sous la dernière), peint derrière.
fn trait_derriere(bitmap: bool, k: usize, d: eframe::egui::Vec2, nom: &str) {
    let Some(mut b) = planche(des_plans(8, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.paint_behind = true;
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let de = xf.rect(l.cards[k].thumb).center();
    let a = de + d;
    tracer(&mut b, de, a);
    repos(&mut b);
    let rgba = b.capturer().unwrap();
    let nom = format!("derriere-{nom}-{}", if bitmap { "bitmap" } else { "vecteur" });
    sauver(&rgba, &nom);
    // (Partie 127 : en bitmap, ses pixels sur la couche bitmap de la planche.)
    let sur_la_planche = !b.app.doc.annotations.traces.is_empty() || !b.app.doc.annotations.bitmap.morceaux.is_empty();
    assert!(sur_la_planche, "{nom} : la part hors de tout champ va sur la planche ({})", b.app.status);
    continu(&rgba, de, a, &nom);
}

#[test]
fn a_stroke_painted_behind_leaves_its_case_and_draws_on_the_whole_board() {
    for bitmap in [false, true] {
        // Vers le haut, au-dessus de la première rangée (la capture d'Anthony : les traits s'arrêtaient à 12 % de la case au-dessus d'elle).
        trait_derriere(bitmap, 3, vec2(60.0, -150.0), "haut");
        // Vers le bas, de la deuxième rangée à travers les cases pâles jusque sous la dernière.
        trait_derriere(bitmap, 7, vec2(-60.0, 260.0), "bas");
    }
}

#[test]
fn the_part_kept_by_the_case_stays_behind_what_was_already_drawn() {
    let Some(mut b) = planche(des_plans(8, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let u = xf.rect(l.cards[2].thumb);
    // Un premier trait dans la case, au-dessus ; puis un trait peint derrière, qui en sort.
    tracer(&mut b, u.left_center() + vec2(10.0, 0.0), u.right_center() - vec2(10.0, 0.0));
    repos(&mut b);
    b.app.paint_behind = true;
    tracer(&mut b, u.center(), u.center() + vec2(40.0, -150.0));
    repos(&mut b);
    let cel = b.app.doc.panels[2].layers.iter().find_map(|l| l.cel_at(0)).expect("le dessin du plan 3");
    let d = b.app.drawing(cel).expect("un dessin vectoriel");
    assert_eq!(d.elements.len(), 2, "le premier trait, et la part du second gardée par sa case");
    // Le premier trait, horizontal, est resté dessus ; la part du trait peint derrière est dessous.
    let Element::Stroke { points, .. } = &d.elements[1] else { panic!("un trait") };
    let (h, v) = points.iter().fold((0.0_f32, 0.0_f32), |(h, v), p| (h.max((p.x - points[0].x).abs()), v.max((p.y - points[0].y).abs())));
    assert!(h > v, "le premier trait reste au-dessus du trait peint derrière");
    assert!(!b.app.doc.annotations.traces.is_empty());
    // Une seule étape d'annulation : le trait entier revient dans son plan, rien sur la planche.
    b.app.undo();
    repos(&mut b);
    assert!(b.app.doc.annotations.traces.is_empty());
    assert_eq!(b.app.drawing(cel).map_or(0, |d| d.elements.len()), 1);
}
