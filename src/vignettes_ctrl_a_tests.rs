//! Partie 134 : Ctrl+A dans les Vignettes passe par le choix de la planche (Anthony, 2026-11-10). Il prend les traits entiers du calque où l'on
//! choisit, ceux qui débordent des cases et ceux du papier compris, et rien des autres calques ; on peut ensuite les déplacer sans couper un
//! seul trait. Au vrai geste sur la planche (carte graphique), comme `vignettes_selection_tests.rs`.

use eframe::egui::{pos2, vec2, Pos2};

use crate::document::{Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, planche, repos, tracer};
use crate::vector::{Element, VPoint};
use crate::vignettes_selection_tests::{boite, choisies, glisser_droit, parts};

/// La planche de douze plans, six par rangée ; un trait tracé du milieu de la case 2 jusque dans la case 3, en passant par le papier entre elles ;
/// et dans chaque plan deux autres calques vectoriels, dont un avec un trait (les calques qu'on n'a pas choisis). Rend aussi le trait à l'écran.
fn planche_trois_calques(bitmap: bool) -> Option<(Banc, Pos2, Pos2)> {
    let mut doc = des_plans(12, bitmap);
    for p in &mut doc.panels {
        for nom in ["Autre 1", "Autre 2"] {
            let mut l = Layer::new(nom, LayerKind::Vector);
            l.frames = vec![None; p.layers.first().map_or(1, |l| l.frames.len().max(1))];
            p.layers.push(l);
        }
    }
    let mut b = planche(doc, 6, 1.0)?;
    // Le calque où l'on dessine : le premier (celui des plans).
    b.app.cur_layer = 0;
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (t2, t3) = (xf.rect(l.cards[2].thumb), xf.rect(l.cards[3].thumb));
    let (de, a) = (t2.center(), pos2(t3.center().x, t3.center().y + 10.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    // Un trait sur « Autre 1 » dans le plan de la case 2 : il ne doit jamais être pris.
    let p2 = l.cards[2].panel;
    let cel = b.app.doc.new_cel(LayerKind::Vector);
    let c = b.app.doc.cels.get_mut(&cel).unwrap();
    c.vector.as_mut().unwrap().elements.push(Element::Stroke {
        points: (0..20).map(|k| VPoint { x: 200.0 + 40.0 * k as f32, y: 500.0, r: 4.0 }).collect(),
        color: [0, 120, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    });
    c.rerender();
    let n = b.app.doc.panels[p2].layers.len();
    b.app.doc.panels[p2].layers[n - 2].set(0, Some(cel));
    b.chauffer(3);
    Some((b, de, a))
}

/// Les éléments du calque « Autre 1 », dans tous les plans.
fn autres(b: &Banc) -> Vec<Element> {
    let mut out = Vec::new();
    for p in &b.app.doc.panels {
        for l in p.layers.iter().filter(|l| l.name == "Autre 1") {
            for c in l.frames.iter().flatten() {
                if let Some(d) = b.app.doc.cels.get(c).and_then(|c| c.vector.as_ref()) {
                    out.extend(d.elements.iter().cloned());
                }
            }
        }
    }
    out
}

#[test]
fn ctrl_a_on_the_board_takes_the_whole_strokes_of_the_chosen_layer_and_nothing_else() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selp_active(), "le choix de la planche ({})", b.app.status);
    let (el, pl) = parts(&b);
    let pris = choisies(&b);
    // Tout ce qui est sur le calque où l'on choisit et sur le papier, rien de « Autre 1 ».
    let autres_n = autres(&b).len();
    assert_eq!(pris, el + pl - autres_n, "tous les traits du calque et du papier, entiers, et rien des autres calques");
    assert!(b.app.status.contains("Tout sélectionné") && b.app.status.contains("entier"), "{}", b.app.status);
}

#[test]
fn ctrl_a_then_a_move_cuts_no_stroke_and_leaves_the_other_layers_in_one_step() {
    let Some((mut b, de, a)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let avant_autres = autres(&b);
    let (el0, pl0) = parts(&b);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selp_active(), "{}", b.app.status);
    let pas0 = b.app.history.undo_len();
    // Ctrl + glisser dans la boîte, sur le trait : 30 pixels vers la droite.
    let q = boite(&b);
    let centre = pos2((q[0].x + q[2].x) * 0.5, (q[0].y + q[2].y) * 0.5);
    // (Sur le trait, dans l'image de la case 2 : entre deux cases, l'appui irait à la barre de continuité.)
    let prise = de + (a - de) * 0.15;
    let prise = if crate::vector::point_in_polygon(prise, &q) { prise } else { centre };
    glisser_droit(&mut b, prise, vec2(30.0, 0.0));
    repos(&mut b);
    assert_eq!(b.app.history.undo_len(), pas0 + 1, "une seule étape d'annulation");
    assert_eq!(parts(&b), (el0, pl0), "aucun trait coupé : autant d'éléments qu'avant");
    assert!(autres(&b) == avant_autres, "les calques qu'on n'a pas choisis n'ont pas bougé");
}

#[test]
fn ctrl_a_on_the_board_names_a_locked_layer() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    for p in &mut b.app.doc.panels {
        p.layers[0].locked = true;
    }
    b.chauffer(1);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.status.contains("verrouillé"), "la barre d'état nomme le calque laissé de côté : {}", b.app.status);
    assert!(b.app.vsel.is_none(), "rien n'est pris sur le calque verrouillé");
}

#[test]
fn ctrl_a_on_the_board_takes_the_pixels_of_a_bitmap_layer() {
    let Some((mut b, _, _)) = planche_trois_calques(true) else { return println!("pas de carte graphique") };
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selb_active(), "le choix des pixels de la planche ({})", b.app.status);
    assert!(b.app.status.contains("Tout sélectionné"), "{}", b.app.status);
}
