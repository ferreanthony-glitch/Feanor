//! La colonne des pastilles dans le Storyboard (Anthony, 2026-10-23) :
//! - les brosses du décor y sont aussi, comme dans les Vignettes : leurs traits vont dans le calque Décor du plan ;
//! - la pastille « classique », en tête de colonne : le pinceau de base (« Pinceau rond »), sans personnage ni décor.

use eframe::egui::{vec2, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::brushtex::Target;
use crate::decor::Genre;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
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
fn the_storyboard_column_has_the_classic_pastille_first_and_the_decor_brushes() {
    let b = banc();
    let rangs: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    assert_eq!(rangs.first(), Some(&Case::Classique), "la pastille classique en tête : {rangs:?}");
    let decor: Vec<usize> = Genre::ALL.iter().map(|g| rangs.iter().position(|c| *c == Case::Decor(*g)).unwrap_or_else(|| panic!("{g:?} dans le Storyboard : {rangs:?}"))).collect();
    let derniere_neutre = rangs.iter().rposition(|c| matches!(c, Case::Choix(_))).unwrap();
    assert!(decor[0] > derniere_neutre && decor.windows(2).all(|w| w[1] == w[0] + 1), "après les couleurs neutres, ensemble : {rangs:?}");
    assert!(decor[2] < rangs.iter().position(|c| *c == Case::Plus).unwrap(), "avant le « + »");
}

#[test]
fn a_decor_brush_in_the_storyboard_draws_in_the_decor_layer_of_the_plan_in_one_undo_step() {
    let mut b = banc();
    let (couches, avant) = (b.app.layers().len(), b.app.history.undo_len());
    let calque0 = b.app.layers()[b.app.cur_layer].id;
    clic_case(&mut b, Case::Decor(Genre::Feutre));
    assert_eq!(b.app.perso.decor, Some(Genre::Feutre));
    assert_eq!(b.app.color, b.app.settings.vignettes.couleur_decor, "la couleur du décor (Préférences > Vignettes)");
    assert_eq!(b.app.brush.size, b.app.settings.vignettes.taille_decor(Genre::Feutre));
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
    clic_case(&mut b, Case::Decor(Genre::Feutre));
    assert_eq!(b.app.perso.decor, None);
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
    clic_case(&mut b, Case::Decor(Genre::Crayon));
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
fn the_classic_pastille_goes_back_to_the_round_brush_without_character_or_decor() {
    let mut b = banc();
    let rond = b.app.brushes.presets.iter().find(|p| p.target == Target::Brush && p.source == crate::pinceaux::PELURE && p.name == "Pinceau rond").expect("le Pinceau rond de SHOT").settings;
    // Un personnage choisi, un pinceau retouché, puis la pastille classique.
    clic_case(&mut b, Case::Choix(Choix::Neutre(1)));
    assert_eq!(b.app.perso.actif, Some(Choix::Neutre(1)));
    b.app.brush.size = 42.0;
    b.app.brush.hardness = 0.2;
    b.app.set_tool(Tool::Eraser);
    let avant = b.app.history.undo_len();
    clic_case(&mut b, Case::Classique);
    assert_eq!(b.app.perso.actif, None, "plus de personnage");
    assert_eq!(b.app.perso.decor, None);
    assert_eq!(b.app.tool, Tool::Brush, "le pinceau");
    assert_eq!(b.app.brush, rond, "le réglage du Pinceau rond");
    assert_eq!(b.app.history.undo_len(), avant, "rien dans l'annulation");
    // Depuis une brosse du décor : le décor est lâché, le Pinceau rond revient.
    clic_case(&mut b, Case::Decor(Genre::Aplat));
    assert_eq!(b.app.perso.decor, Some(Genre::Aplat));
    clic_case(&mut b, Case::Classique);
    assert_eq!(b.app.perso.decor, None);
    assert_eq!((b.app.tool, b.app.brush), (Tool::Brush, rond));
    // L'action du clavier fait la même chose.
    b.app.brush.size = 13.0;
    b.app.run_action("perso.classique");
    assert_eq!(b.app.brush, rond);
}

#[test]
fn the_decor_brushes_go_away_in_the_animation_and_come_back_in_the_storyboard() {
    let mut b = banc();
    clic_case(&mut b, Case::Decor(Genre::Feutre));
    assert_eq!(b.app.perso.decor, Some(Genre::Feutre));
    b.app.set_espace(Espace::Animation);
    b.chauffer(4);
    assert_eq!(b.app.perso.decor, None, "le décor est lâché en animation");
    assert!(b.app.perso.cases.iter().all(|(_, c)| !matches!(c, Case::Decor(_))), "pas de pastille de décor en animation");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    assert!(b.app.perso.cases.iter().any(|(_, c)| matches!(c, Case::Decor(_))));
}
