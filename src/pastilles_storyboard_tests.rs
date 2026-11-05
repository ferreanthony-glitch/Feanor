//! Les pastilles dans le Storyboard (Anthony, 2026-10-23 ; depuis le 2026-10-24 une rangée à l'extrémité droite de la barre du haut,
//! plus une colonne sur le dessin) :
//! - les brosses du décor y sont aussi, comme dans les Vignettes : leurs traits vont dans le calque Décor du plan ;
//! - la pastille « classique », en tête de rangée : le pinceau de base (« Pinceau rond »), sans personnage ni décor.

use eframe::egui::{vec2, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::decor::Genre;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::personnages::Type;
use crate::personnagesui::{Case, Choix};
use crate::workspace::Espace;

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert_eq!(b.app.espace, Espace::Storyboard);
    assert!(!b.app.board_shown(), "le dessin, pas la planche");
    b
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

fn case(b: &Banc, c: Case) -> Pos2 {
    b.app.perso.cases.iter().find(|(_, k)| *k == c).map(|(r, _)| r.center()).unwrap_or_else(|| panic!("la case {c:?} n'est pas dans la colonne : {:?}", b.app.perso.cases))
}

fn clic_case(b: &mut Banc, c: Case) {
    let p = case(b, c);
    clic(b, p);
}

/// Un trait au milieu du dessin, de gauche à droite.
fn trait_au_milieu(b: &mut Banc) {
    let c = b.app.canvas_rect.center();
    let (a, z) = (c - vec2(60.0, 0.0), c + vec2(60.0, 10.0));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 10.0))], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerButton { pos: z, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

fn encre(b: &Banc, li: usize) -> bool {
    let doc = &b.app.doc;
    doc.panels[b.app.cur_panel].layers[li].frames.iter().flatten().any(|c| doc.cels.get(c).is_some_and(|c| !c.raster.is_blank() || c.vector.as_ref().is_some_and(|v| !v.elements.is_empty())))
}

#[test]
fn clicking_a_pastille_in_the_top_bar_picks_it_and_a_right_click_opens_its_menu_without_drawing() {
    let mut b = banc();
    b.garder_textes = true;
    let avant = b.app.history.undo_len();
    // Les pastilles sont dans la barre du haut : un clic sur « classique » puis sur une couleur neutre les choisit, sans rien tracer.
    let dessin = b.app.canvas_rect;
    assert!(case(&b, Case::Choix(Choix::Classique(0))).y < dessin.top(), "au-dessus du dessin, dans la barre");
    clic_case(&mut b, Case::Choix(Choix::Classique(1)));
    assert_eq!(b.app.perso.actif, Some(Choix::Classique(1)));
    assert!(b.app.stroke.is_none());
    assert_eq!(b.app.history.undo_len(), avant, "choisir n'est pas une étape d'annulation");
    // Clic droit : le menu de la pastille (modifier, dupliquer…).
    let p = case(&b, Case::Choix(Choix::Classique(1)));
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t.contains("Modifier…")) && b.textes.iter().any(|t| t.contains("Dupliquer")), "{:?}", b.textes);
    assert!(b.app.stroke.is_none());
    // « + » : un personnage de plus (la fenêtre « Modifier » s'ouvre sur lui) ; l'icône du panneau l'ouvre.
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
    let n = b.app.doc.personnages.len();
    // Le « + » ouvre un petit menu à trois types ; « Brosse personnage » ajoute un personnage, sans fenêtre « Modifier ».
    clic_case(&mut b, Case::Plus);
    assert!(Type::ALL.iter().all(|t| b.app.perso.cases.iter().any(|(_, c)| *c == Case::Ajouter(*t))), "le menu a une ligne par type : {:?}", b.app.perso.cases);
    assert_eq!(b.app.doc.personnages.len(), n, "ouvrir le menu n'ajoute rien");
    clic_case(&mut b, Case::Ajouter(Type::Personnage));
    assert_eq!(b.app.doc.personnages.len(), n + 1, "« Brosse personnage » ajoute un personnage");
    assert!(case(&b, Case::Choix(Choix::Classique(0))).y < dessin.top(), "toujours dans la barre");
    // La pastille du personnage neuf est dans la rangée, avant les classiques.
    let rangs: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    assert!(matches!(rangs[0], Case::Choix(Choix::Personnage(_))), "{rangs:?}");
}

#[test]
fn the_storyboard_bar_has_the_personnages_then_the_classiques_then_the_decors() {
    // Adapté (2026-10-29) : plus de pastille « classique » en tête ; trois groupes : Personnages, Classiques, Décors.
    let b = banc();
    let rangs: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    assert_eq!(rangs.first(), Some(&Case::Choix(Choix::Classique(0))), "un projet sans personnage : les classiques en tête : {rangs:?}");
    let decor: Vec<usize> = Genre::ALL.iter().map(|g| rangs.iter().position(|c| *c == crate::decor_tests::case_du_decor(*g)).unwrap_or_else(|| panic!("{g:?} dans le Storyboard : {rangs:?}"))).collect();
    let derniere_classique = rangs.iter().rposition(|c| matches!(c, Case::Choix(Choix::Personnage(_) | Choix::Classique(_)))).unwrap();
    assert!(decor[0] > derniere_classique && decor.windows(2).all(|w| w[1] == w[0] + 1), "après les classiques, ensemble : {rangs:?}");
    assert!(decor[2] < rangs.iter().position(|c| *c == Case::Plus).unwrap(), "avant le « + »");
}

#[test]
fn a_decor_brush_in_the_storyboard_draws_in_the_decor_layer_of_the_plan_in_one_undo_step() {
    let mut b = banc();
    let (couches, avant) = (b.app.layers().len(), b.app.history.undo_len());
    let calque0 = b.app.layers()[b.app.cur_layer].id;
    let brosse_avant = b.app.brush;
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Feutre));
    assert_eq!(b.app.decor_actif(), Some(crate::decor_tests::rang(Genre::Feutre)));
    assert_eq!(b.app.color, crate::personnages::decors_par_defaut()[1].line, "la couleur de trait de la brosse Décor 2");
    assert_eq!(b.app.brush.size, brosse_avant.size, "la brosse Décor suit le pinceau : sa taille ne change pas");
    assert_eq!(b.app.doc.decor_de(b.app.cur_panel), None, "choisir ne crée rien");
    assert_eq!(b.app.history.undo_len(), avant, "choisir n'est pas une étape d'annulation");
    trait_au_milieu(&mut b);
    let at = b.app.doc.decor_de(b.app.cur_panel).expect("le calque Décor du plan est né au premier trait");
    assert_eq!(at, 0, "tout en bas du plan");
    assert_eq!(b.app.layers().len(), couches + 1);
    assert_eq!(b.app.cur_layer, at, "le calque Décor est le calque courant tant que le décor est choisi");
    assert!(encre(&b, at), "le trait est dans le décor");
    assert!((1..b.app.layers().len()).all(|li| !encre(&b, li)), "rien ailleurs");
    assert_eq!(b.app.history.undo_len(), avant + 1, "le calque et le premier trait ne font qu'une étape");
    assert!(b.app.history.names().0.last().is_some_and(|n| n.contains("décor")), "{:?}", b.app.history.names().0.last());
    // Un deuxième trait : une étape de plus, le même calque.
    trait_au_milieu(&mut b);
    assert_eq!(b.app.history.undo_len(), avant + 2);
    assert_eq!(b.app.layers().len(), couches + 1);
    // Lâcher le décor (sa pastille) : le calque d'avant revient.
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Feutre));
    assert_eq!(b.app.decor_actif(), None);
    assert_eq!(b.app.layers()[b.app.cur_layer].id, calque0, "le calque d'avant redevient le calque courant");
    // Annuler les deux traits : plus de calque Décor.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.decor_de(b.app.cur_panel), None, "le calque part avec son premier trait");
}

#[test]
fn a_click_with_a_decor_brush_that_draws_nothing_leaves_no_decor_layer() {
    let mut b = banc();
    let avant = b.app.history.undo_len();
    // (§107 : la brosse Décor suit le pinceau de l'utilisateur, ici le crayon, au lieu d'imposer le sien.)
    b.app.set_tool(Tool::Pencil);
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Crayon));
    assert_eq!(b.app.tool, Tool::Pencil);
    // Un simple clic sur le dessin (le crayon pose un point) : soit un trait, soit rien, jamais un calque vide.
    let c = b.app.canvas_rect.center();
    clic(&mut b, c);
    match b.app.doc.decor_de(b.app.cur_panel) {
        Some(at) => {
            assert!(encre(&b, at), "un calque Décor n'existe pas sans trait");
            assert_eq!(b.app.history.undo_len(), avant + 1);
        }
        None => assert_eq!(b.app.history.undo_len(), avant),
    }
}

#[test]
fn clicking_the_chosen_brush_lets_it_go_and_there_is_no_classic_pastille_any_more() {
    // Adapté (2026-10-29) : la pastille « classique » ⊘ n'existe plus ; recliquer la brosse choisie la lâche, et la couleur est libre, dans le
    // calque courant. Une brosse Décor lâchée rend l'outil et les pinceaux d'avant.
    let mut b = banc();
    clic_case(&mut b, Case::Choix(Choix::Classique(1)));
    assert_eq!(b.app.perso.actif, Some(Choix::Classique(1)));
    let (calque, avant) = (b.app.cur_layer, b.app.history.undo_len());
    clic_case(&mut b, Case::Choix(Choix::Classique(1)));
    assert_eq!(b.app.perso.actif, None, "un clic sur la brosse choisie la lâche");
    assert_eq!(b.app.decor_actif(), None);
    assert_eq!(b.app.cur_layer, calque, "dans le calque courant");
    assert_eq!(b.app.history.undo_len(), avant, "rien dans l'annulation");
    // Une brosse Décor : lâchée, l'outil et le pinceau d'avant reviennent.
    b.app.brush.size = 42.0;
    b.app.brush.hardness = 0.2;
    b.app.set_tool(Tool::Eraser);
    let (outil0, pinceau0) = (b.app.tool, b.app.brush);
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Aplat));
    assert_eq!(b.app.decor_actif(), Some(crate::decor_tests::rang(Genre::Aplat)));
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Aplat));
    assert_eq!(b.app.decor_actif(), None);
    assert_eq!((b.app.tool, b.app.brush), (outil0, pinceau0));
    // La commande « couleur libre » fait la même chose, d'une brosse Décor comme d'une Classique.
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Feutre));
    b.app.run_action("perso.aucun");
    assert_eq!((b.app.perso.actif, b.app.tool, b.app.brush), (None, outil0, pinceau0));
}

#[test]
fn the_decor_brushes_go_away_in_the_animation_and_come_back_in_the_storyboard() {
    let mut b = banc();
    clic_case(&mut b, crate::decor_tests::case_du_decor(Genre::Feutre));
    assert_eq!(b.app.decor_actif(), Some(crate::decor_tests::rang(Genre::Feutre)));
    b.app.set_espace(Espace::Animation);
    b.chauffer(4);
    assert_eq!(b.app.decor_actif(), None, "le décor est lâché en animation");
    assert!(b.app.perso.cases.iter().all(|(_, c)| !matches!(c, Case::Choix(Choix::Decor(_)))), "pas de pastille de décor en animation");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    assert!(b.app.perso.cases.iter().any(|(_, c)| matches!(c, Case::Choix(Choix::Decor(_)))));
}
