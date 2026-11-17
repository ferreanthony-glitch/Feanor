//! Le dessin libre sur toute la planche des Vignettes (parties 101, 111, 118) avec « Peindre derrière » allumé (Anthony, 2026-11-08 : « je ne peux plus
//! dessiner dans tout le canevas vignettes » ; ses traits s'arrêtaient net au bord du champ des cases d'où ils partaient). Le trait vectoriel peint derrière
//! est posé sous les autres éléments du dessin, pas en dernier.
//!
//! Partie 131 : un trait parti d'une case lui appartient tout entier, peint derrière ou non ; il reste dans le dessin de son plan, où qu'il aille, et la
//! planche le montre en entier. (Avant : sa part hors du champ de sa case allait sur la planche.) Ce que ces essais gardent : on dessine sur toute la planche
//! avec « Peindre derrière », le trait se voit en entier, et il reste derrière ce qui était déjà dessiné.

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
    let sur_la_planche = !b.app.doc.annotations.traces.is_empty() || !b.app.doc.annotations.bitmap.morceaux.is_empty();
    assert!(!sur_la_planche, "{nom} : le trait reste entier dans le dessin de son plan, rien ne va sur la planche ({})", b.app.status);
    assert!(crate::vignettes_tests::encre(&b.app.doc, l.cards[k].panel), "{nom} : il est dans le plan de la case où il commence");
    // Il ne s'arrête pas au bord du champ de sa case : la planche le montre jusqu'au bout.
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
fn a_stroke_painted_behind_stays_whole_behind_what_was_already_drawn() {
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
    assert_eq!(d.elements.len(), 2, "le premier trait, et le second, entier");
    // Le premier trait, horizontal, est resté dessus ; le trait peint derrière est dessous, entier (il monte de 150 points d'écran, hors du cadre).
    let etendue = |e: &Element| match e {
        Element::Stroke { points, .. } => points.iter().fold((0.0_f32, 0.0_f32), |(h, v), p| (h.max((p.x - points[0].x).abs()), v.max((p.y - points[0].y).abs()))),
        _ => (0.0, 0.0),
    };
    let ((h1, v1), (h0, v0)) = (etendue(&d.elements[1]), etendue(&d.elements[0]));
    assert!(h1 > v1, "le premier trait reste au-dessus du trait peint derrière");
    assert!(v0 > h0, "le trait peint derrière est le premier élément du dessin");
    // (Le cadre de la caméra, sur la feuille du plan : son haut.)
    let haut_du_cadre = b.app.doc.default_camera().to_stage(b.app.doc.frame, 0.0, 0.0).y;
    let haut = d.elements[0].bounds().top();
    assert!(haut < haut_du_cadre - 50.0, "il est entier dans le dessin : il sort du cadre par le haut ({haut} contre {haut_du_cadre})");
    assert!(b.app.doc.annotations.traces.is_empty(), "rien sur la planche");
    // Une seule étape d'annulation.
    b.app.undo();
    repos(&mut b);
    assert!(b.app.doc.annotations.traces.is_empty());
    assert_eq!(b.app.drawing(cel).map_or(0, |d| d.elements.len()), 1);
}
