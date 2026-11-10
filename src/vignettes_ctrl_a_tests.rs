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

// ------------------------------------------------------------------------------------------------ relecture de la partie 134

/// Pose `e` dans le dessin du premier calque du plan `p`, à l'image 0 (le dessin est créé s'il n'y en a pas).
fn poser(b: &mut Banc, p: usize, e: Element) {
    let cel = match b.app.doc.panels[p].layers[0].cel_at(0) {
        Some(c) => c,
        None => {
            let c = b.app.doc.new_cel(LayerKind::Vector);
            b.app.doc.panels[p].layers[0].set(0, Some(c));
            c
        }
    };
    let c = b.app.doc.cels.get_mut(&cel).unwrap();
    c.vector.as_mut().unwrap().elements.push(e);
    c.rerender();
}

fn trait_droit(x0: f32, y: f32) -> Element {
    Element::Stroke {
        points: (0..20).map(|k| VPoint { x: x0 + 30.0 * k as f32, y, r: 4.0 }).collect(),
        color: [0, 0, 160],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

/// Le premier x d'une forme et le placement d'un texte, où qu'ils soient dans le projet.
fn forme_et_texte(b: &Banc) -> (Vec<f32>, Vec<[f32; 6]>) {
    let (mut f, mut t) = (Vec::new(), Vec::new());
    for c in b.app.doc.cels.values() {
        for e in c.vector.iter().flat_map(|v| &v.elements) {
            match e {
                Element::Fill { contours, .. } => f.push(contours[0][0][0]),
                Element::Text { place, .. } => t.push(*place),
                _ => {}
            }
        }
    }
    (f, t)
}

/// Les éléments des calques de ce nom, dans tous les plans.
fn du_calque(b: &Banc, nom: &str) -> usize {
    b.app.doc.panels.iter().flat_map(|p| p.layers.iter().filter(|l| l.name == nom)).flat_map(|l| l.frames.iter().flatten()).collect::<std::collections::BTreeSet<_>>().into_iter().filter_map(|c| b.app.doc.cels.get(c).and_then(|c| c.vector.as_ref())).map(|v| v.elements.len()).sum()
}

/// Relecture, 1 : sur un calque bitmap, un trait parti du papier (un morceau de la couche bitmap de la planche) : Ctrl+A ne fige pas (le lasso
/// « de toute la planche » n'est plus l'infini) et prend ses pixels.
#[test]
fn ctrl_a_takes_a_bitmap_paper_stroke_without_freezing() {
    let Some((mut b, _, _)) = planche_trois_calques(true) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let t1 = b.app.vignettes.xf.rect(l.cards[1].thumb);
    tracer(&mut b, pos2(t1.left() + 10.0, t1.top() - 25.0), pos2(t1.right() - 10.0, t1.top() - 25.0));
    repos(&mut b);
    let morceaux: Vec<(u64, usize)> = b.app.doc.annotations.bitmap.morceaux.iter().map(|m| {
        let mut n = 0;
        crate::planchebitmap::chaque_pixel(&m.pixels, |_, _, c| n += (c.a() > 0) as usize);
        (m.id, n)
    }).collect();
    assert!(!morceaux.is_empty(), "le trait du papier est un morceau de la planche");
    let t = std::time::Instant::now();
    b.app.run_action("edition.tout_selectionner");
    let duree = t.elapsed();
    assert!(duree < std::time::Duration::from_secs(1), "Ctrl+A en {duree:?}");
    assert!(b.app.selb_active(), "{}", b.app.status);
    let s = b.app.vignettes.selb.as_ref().unwrap();
    for (id, n) in morceaux {
        let p = s.pieces.iter().find(|p| p.source == crate::vignetteselbmp::Source::Morceau { id }).unwrap_or_else(|| panic!("le morceau {id} est pris ({})", b.app.status));
        let mut pris = 0;
        crate::planchebitmap::chaque_pixel(&p.pixels, |_, _, c| pris += (c.a() > 0) as usize);
        assert_eq!(pris, n, "tous les pixels du morceau");
    }
}

/// Relecture, 2 : une forme et un texte, dans deux cases : Ctrl+A les prend (le lasso de toute la planche ne les laissait plus), un glissé les
/// déplace avec les traits, une annulation défait tout.
#[test]
fn ctrl_a_takes_shapes_and_texts_and_moves_them_with_the_strokes() {
    let Some((mut b, de, a)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let (p3, p4) = { let l = b.app.board_layout(); (l.cards[3].panel, l.cards[4].panel) };
    poser(&mut b, p3, Element::Fill { contours: vec![vec![[800.0, 400.0], [1000.0, 400.0], [1000.0, 600.0], [800.0, 600.0]]], color: [200, 0, 0], opacity: 1.0, gradient: None });
    let police = b.app.text_font.clone();
    poser(&mut b, p4, Element::Text { text: "Bonjour".into(), font: police, size: 120.0, color: [0, 0, 0], opacity: 1.0, place: [1.0, 0.0, 0.0, 1.0, 900.0, 500.0] });
    b.chauffer(3);
    repos(&mut b);
    let avant = forme_et_texte(&b);
    assert_eq!((avant.0.len(), avant.1.len()), (1, 1));
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selp_active(), "{}", b.app.status);
    let objets = b.app.vignettes.selp.as_ref().unwrap().traits.iter().filter(|t| t.objet).count();
    assert_eq!(objets, 2, "la forme et le texte sont pris : {}", b.app.status);
    let pas0 = b.app.history.undo_len();
    let q = boite(&b);
    let prise = de + (a - de) * 0.15;
    assert!(crate::vector::point_in_polygon(prise, &q));
    glisser_droit(&mut b, prise, vec2(30.0, 0.0));
    repos(&mut b);
    assert_eq!(b.app.history.undo_len(), pas0 + 1, "une seule étape");
    let apres = forme_et_texte(&b);
    assert_eq!((apres.0.len(), apres.1.len()), (1, 1), "ni perdus, ni doublés");
    assert!(apres.0[0] > avant.0[0] + 5.0, "la forme a suivi : {avant:?} → {apres:?}");
    assert!(apres.1[0][4] > avant.1[0][4] + 5.0, "le texte a suivi : {avant:?} → {apres:?}");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(forme_et_texte(&b), avant, "une annulation défait tout");
}

/// Relecture, 3 : un plan sans calque de ce nom (le plan de la case 5 n'a que « Fond ») : Ctrl+A n'y prend rien (le lasso, lui, se rabat sur
/// le calque qui peut recevoir ; Ctrl+A ne prend que le calque où l'on choisit, ou celui de même nom).
#[test]
fn ctrl_a_takes_nothing_in_a_plan_without_a_layer_of_that_name() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let p5 = b.app.board_layout().cards[5].panel;
    assert_ne!(p5, b.app.cur_panel);
    let cel = b.app.doc.new_cel(LayerKind::Vector);
    {
        let c = b.app.doc.cels.get_mut(&cel).unwrap();
        c.vector.as_mut().unwrap().elements.push(trait_droit(600.0, 500.0));
        c.rerender();
    }
    let mut fond = Layer::new("Fond", LayerKind::Vector);
    fond.frames = vec![None; b.app.doc.panels[p5].layers[0].frames.len().max(1)];
    fond.set(0, Some(cel));
    b.app.doc.panels[p5].layers = vec![fond];
    b.chauffer(3);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selp_active(), "{}", b.app.status);
    let s = b.app.vignettes.selp.as_ref().unwrap();
    assert!(!s.traits.iter().flat_map(|t| &t.parts).any(|p| matches!(p, crate::vignettesel::PartRef::Dessin { cel: c, .. } if *c == cel)), "rien de « Fond »");
}

/// Relecture, 4 : après Ctrl+A, choisir un autre calque lève le choix (déplacé, il serait reposé dans le nouveau calque).
#[test]
fn changing_layer_after_ctrl_a_drops_the_board_choice() {
    let Some((mut b, de, a)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selp_active(), "{}", b.app.status);
    let q = boite(&b);
    b.app.cur_layer = b.app.layers().len() - 1;
    b.chauffer(2);
    assert!(!b.app.selp_active(), "sur « Autre 2 », le choix fait sur le premier calque ne vaut plus");
    let prise = de + (a - de) * 0.15;
    assert!(crate::vector::point_in_polygon(prise, &q));
    glisser_droit(&mut b, prise, vec2(30.0, 0.0));
    repos(&mut b);
    assert_eq!(du_calque(&b, "Autre 2"), 0, "rien n'est reposé dans « Autre 2 »");
    // Bitmap : de même.
    let Some((mut b, _, _)) = planche_trois_calques(true) else { return };
    for p in &mut b.app.doc.panels {
        if let Some(l) = p.layers.last_mut() {
            l.kind = LayerKind::Bitmap;
        }
    }
    b.chauffer(2);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.selb_active(), "{}", b.app.status);
    b.app.cur_layer = b.app.layers().len() - 1;
    b.chauffer(2);
    assert!(!b.app.selb_active(), "bitmap : sur un autre calque, le choix des pixels ne vaut plus");
}

/// Relecture, 5 : un trait du papier ne garde pas le calque où il a été tracé (il est sur le calque « Hors cadres » de la planche, quel que soit le
/// calque courant au tracé) ; déplacé jusque dans une case, il arrive dans **le calque où l'on choisit** (`case_recoit`), pas dans celui où on
/// l'avait tracé. Ce qu'on fixe ici ; Anthony décidera s'il le faut autrement.
#[test]
fn a_paper_stroke_moved_into_a_case_lands_on_the_current_layer() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let t1 = b.app.vignettes.xf.rect(l.cards[1].thumb);
    let p1 = l.cards[1].panel;
    // Tracé sur « Autre 1 » (en biais : une boîte assez haute pour qu'on y appuie loin de ses bords)…
    b.app.cur_layer = b.app.layers().len() - 2;
    let (de, a) = (pos2(t1.center().x - 30.0, t1.top() - 45.0), pos2(t1.center().x + 30.0, t1.top() - 15.0));
    let traces0 = b.app.doc.annotations.traces.len();
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.annotations.traces.len(), traces0 + 1, "le trait est sur le papier");
    let t = b.app.doc.annotations.traces.last().unwrap();
    assert!(b.app.doc.annotations.calque(t.calque).is_some_and(|c| c.planche), "il est sur le calque de la planche, pas sur « Autre 1 »");
    // … puis, choisi sur « Autre 2 », glissé jusqu'au milieu de la case 1.
    b.app.cur_layer = b.app.layers().len() - 1;
    b.chauffer(1);
    let milieu = pos2(t1.center().x, t1.top() - 30.0);
    crate::vignettes_selection_tests::lasso(&mut b, milieu, 9.0);
    assert!(b.app.selp_active(), "{}", b.app.status);
    let (autre1, autre2) = (du_calque(&b, "Autre 1"), du_calque(&b, "Autre 2"));
    // (Sur le trait, loin du point fixe au centre de la boîte : l'appui le déplacerait, lui.)
    glisser_droit(&mut b, de + (a - de) * 0.25, vec2(0.0, t1.center().y - milieu.y));
    repos(&mut b);
    assert_eq!(b.app.doc.annotations.traces.len(), traces0, "il a quitté le papier");
    assert_eq!(du_calque(&b, "Autre 1"), autre1, "pas dans « Autre 1 », où on l'avait tracé");
    assert_eq!(du_calque(&b, "Autre 2"), autre2 + 1, "dans « Autre 2 », le calque où l'on choisit");
    let li = b.app.doc.panels[p1].layers.iter().position(|l| l.name == "Autre 2").unwrap();
    let cel = b.app.doc.panels[p1].layers[li].cel_at(0).expect("dans le plan de la case 1");
    assert_eq!(b.app.doc.cels[&cel].vector.as_ref().unwrap().elements.len(), 1);
}
/// Relecture, 6 : bitmap, calque verrouillé partout et rien sur le papier : la barre d'état le nomme quand même.
#[test]
fn ctrl_a_on_a_locked_bitmap_layer_names_it() {
    let Some((mut b, _, _)) = planche_trois_calques(true) else { return println!("pas de carte graphique") };
    for p in &mut b.app.doc.panels {
        p.layers[0].locked = true;
    }
    b.chauffer(1);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.vignettes.selb.is_none());
    assert!(b.app.status.contains("verrouillé"), "{}", b.app.status);
}

/// Relecture, 7 : l'aplat visé par l'outil Sélection garde son chemin dans les Vignettes (`multi_select_all`), pas le choix de la planche.
#[test]
fn ctrl_a_on_the_aplat_line_in_the_board_keeps_its_own_path() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let li = b.app.cur_layer;
    b.app.layers_mut()[li].fond = Some(crate::fond::FondReglages::default());
    b.app.aplat_cible = Some(b.app.layers()[li].id);
    b.app.set_tool(crate::app::Tool::Select);
    b.chauffer(1);
    assert!(b.app.aplat_strate_selection());
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.vignettes.selp.is_none(), "pas le choix de la planche : {}", b.app.status);
    assert!(!b.app.status.contains("trait entier") && !b.app.status.contains("traits entiers"), "{}", b.app.status);
}

/// Relecture, 8 : le calque verrouillé dans un autre plan que le plan courant est nommé.
#[test]
fn ctrl_a_names_a_layer_locked_in_another_plan() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let p5 = b.app.board_layout().cards[5].panel;
    assert_ne!(p5, b.app.cur_panel);
    b.app.doc.panels[p5].layers[0].locked = true;
    b.chauffer(1);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.status.contains("verrouillé dans 1 plan"), "{}", b.app.status);
}

/// Relecture, 9 : Ctrl+A pendant un trait : il est fini et posé d'abord, et il entre dans le choix ; le lâcher n'en fait pas un second.
#[test]
fn ctrl_a_during_a_stroke_finishes_it_and_takes_it() {
    let Some((mut b, _, _)) = planche_trois_calques(false) else { return println!("pas de carte graphique") };
    let (el0, pl0) = parts(&b);
    let l = b.app.board_layout();
    let t1 = b.app.vignettes.xf.rect(l.cards[1].thumb);
    let (de, a) = (t1.center(), pos2(t1.center().x, t1.top() - 30.0));
    let m = eframe::egui::Modifiers::NONE;
    b.image(vec![eframe::egui::Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(de, true, m)], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![eframe::egui::Event::PointerMoved(de + (a - de) * (k as f32 / 12.0))], 1.0 / 120.0);
    }
    b.app.run_action("edition.tout_selectionner");
    let (el, pl) = parts(&b);
    assert_eq!(el + pl, el0 + pl0 + 1, "le trait en cours est posé");
    assert!(b.app.selp_active(), "{}", b.app.status);
    assert_eq!(choisies(&b), el + pl - autres(&b).len(), "et il est pris, avec tout le reste");
    b.image(vec![crate::vignettes_tests::appui(a, false, m)], 1.0 / 60.0);
    repos(&mut b);
    assert_eq!(parts(&b), (el, pl), "le lâcher n'ajoute rien");
}

/// Relecture, 9 (bitmap) : une couche bitmap de la planche illisible à l'ouverture : Ctrl+A n'y mène rien, comme le lasso, et le dit.
#[test]
fn ctrl_a_does_nothing_on_an_unreadable_bitmap_board_layer() {
    let Some((mut b, _, _)) = planche_trois_calques(true) else { return println!("pas de carte graphique") };
    b.app.doc.annotations.bitmap.illisible = true;
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.vignettes.selb.is_none(), "{}", b.app.status);
    assert!(b.app.status.contains("n'a pas pu être lue"), "{}", b.app.status);
}

/// Relecture, 10 : la mesure. 100 cases, 50 traits chacune, coupés comme avant la partie 131 : chaque trait va du milieu d'une case au milieu de
/// sa voisine, en deux parts du même numéro qui se touchent sur le papier entre elles (une dans chaque plan). Le temps de Ctrl+A.
#[test]
fn ctrl_a_on_a_hundred_cases_of_fifty_split_strokes_is_measured() {
    let mut doc = crate::traits_continus_tests::des_plans(100, false);
    for p in 0..100 {
        let cel = doc.new_cel(LayerKind::Vector);
        doc.panels[p].layers[0].set(0, Some(cel));
    }
    let Some(mut b) = planche(doc, 10, 1.0) else { return println!("pas de carte graphique") };
    b.app.cur_layer = 0;
    let l = b.app.board_layout();
    let ligne = |pose: &crate::horschamp::Pose, de: Pos2, a: Pos2| -> Vec<VPoint> {
        (0..=10).map(|k| { let q = pose.dessin(de + (a - de) * (k as f32 / 10.0)); VPoint { x: q.x, y: q.y, r: 3.0 } }).collect()
    };
    let mut lien = 1u64;
    for k in (0..100).step_by(2) {
        let (c0, c1) = (&l.cards[k], &l.cards[k + 1]);
        let (p0, p1) = (crate::horschamp::pose_de_la_case(&b.app.doc, &l, k).unwrap(), crate::horschamp::pose_de_la_case(&b.app.doc, &l, k + 1).unwrap());
        let (cel0, cel1) = (b.app.doc.panels[c0.panel].layers[0].cel_at(0).unwrap(), b.app.doc.panels[c1.panel].layers[0].cel_at(0).unwrap());
        for j in 0..50 {
            let y = c0.thumb.top() + c0.thumb.height() * (j as f32 + 1.0) / 51.0;
            let (a, m, z) = (pos2(c0.thumb.center().x, y), pos2((c0.thumb.right() + c1.thumb.left()) * 0.5, y), pos2(c1.thumb.center().x, y));
            for (cel, pts) in [(cel0, ligne(&p0, a, m)), (cel1, ligne(&p1, m, z))] {
                let trace = crate::vector::Trace { lien: Some(lien), ..Default::default() };
                b.app.doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements.push(Element::Stroke { points: pts, color: [0, 0, 160], opacity: 1.0, hardness: 1.0, brush: None, trace });
            }
            lien += 1;
        }
    }
    for c in b.app.doc.cels.values_mut() {
        c.rerender();
    }
    b.chauffer(3);
    repos(&mut b);
    let mut mesures = Vec::new();
    for _ in 0..3 {
        b.app.vignettes.selp = None;
        let t = std::time::Instant::now();
        b.app.run_action("edition.tout_selectionner");
        mesures.push(t.elapsed());
        assert!(b.app.selp_active(), "{}", b.app.status);
    }
    let n: usize = b.app.vignettes.selp.as_ref().unwrap().traits.len();
    println!("MESURE Ctrl+A, 100 cases × 50 traits coupés : {mesures:?} ; {n} traits entiers ({})", b.app.status);
    assert_eq!(n, 2500, "les parts recollées : un trait entier pour deux parts");
}