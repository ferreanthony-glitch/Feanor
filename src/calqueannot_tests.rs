//! Essais du calque d'annotation d'un plan (`calqueannot.rs`) : créé tout au-dessus, dessiné dans, caché, peint par-dessus tout à l'écran,
//! jamais dans un export ni dans une vignette, enregistré, fusion refusée, pas de clé de case.

use eframe::egui::{vec2, Color32, Event, Modifiers};

use crate::app::Tool;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::vignettes_tests::{appui, ouvert, quatre_plans};

// ------------------------------------------------------------------------------------ dans un plan

fn banc_dessin() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert!(!b.app.board_shown(), "le dessin, pas la planche");
    b
}

/// Un trait de gauche à droite au milieu du dessin.
fn trait_au_milieu(b: &mut Banc, dy: f32) {
    let c = b.app.canvas_rect.center() + vec2(0.0, dy);
    let (a, z) = (c - vec2(60.0, 0.0), c + vec2(60.0, 10.0));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![appui(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 10.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(z, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

fn encre_du_calque(b: &Banc, li: usize) -> bool {
    let doc = &b.app.doc;
    doc.panels[b.app.cur_panel].layers[li].frames.iter().flatten().any(|c| doc.cels.get(c).is_some_and(|c| c.vector.as_ref().is_some_and(|v| !v.elements.is_empty())))
}

#[test]
fn an_annotation_layer_is_created_on_top_drawn_in_hidden_and_kept_out_of_every_export() {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    use crate::render;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let mut b = banc_dessin();
    // Un vrai dessin d'abord, dans le calque du plan.
    b.app.color = [10, 20, 30];
    trait_au_milieu(&mut b, -30.0);
    assert!(encre_du_calque(&b, b.app.cur_layer), "le dessin est dans le calque ordinaire");
    let images = |doc: &Document| -> Vec<Vec<Color32>> {
        let rendu = Rendu::new(snapshot_of(doc, &[0]), Papier::Fond, true, Vec::new(), Arc::new(AtomicBool::new(false)));
        vec![(*rendu.image(0, 0).expect("image")).clone()]
    };
    let sans = images(&b.app.doc);
    let miniature_sans = render::composite_full(&b.app.doc, &render::panel_plan_at(&b.app.doc, 0, 0, Some(b.app.doc.paper)));
    // Un calque d'annotation : tout au-dessus, vectoriel, le calque courant.
    let etapes = b.app.history.undo_len();
    let avant = b.app.layers().len();
    b.app.run_action("calque.nouveau_annotation");
    b.chauffer(2);
    assert_eq!(b.app.layers().len(), avant + 1);
    let li = b.app.cur_layer;
    assert_eq!(li, avant, "tout au-dessus");
    let l = b.app.layers()[li].clone();
    assert!(l.annotation && l.kind == crate::document::LayerKind::Vector && l.visible);
    assert_eq!(crate::calquemenu::Sorte::of(&l), crate::calquemenu::Sorte::Annotation);
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape d'annulation");
    // On y dessine, en rouge : le trait est dans ce calque, pas dans l'autre.
    b.app.color = [255, 0, 0];
    trait_au_milieu(&mut b, 30.0);
    assert!(encre_du_calque(&b, li), "le trait est dans le calque d'annotation");
    // À l'écran, le calque est dans le plan, au-dessus de tout.
    let ecran = b.app.build_plan(0, false, true);
    let cel_annot = b.app.layers()[li].cel_at(0).expect("son dessin");
    assert_eq!(ecran.items.last().map(|i| i.cel), Some(cel_annot), "peint en dernier : par-dessus tout");
    // Caché (l'œil), il n'est plus dans le plan de l'écran, et le dessin est intact.
    b.app.layers_mut()[li].visible = false;
    assert!(b.app.build_plan(0, false, true).items.iter().all(|i| i.cel != cel_annot), "caché : pas dans le plan");
    b.app.layers_mut()[li].visible = true;
    // Jamais dans les exports, les vignettes, la lecture : le plan du film saute le calque.
    let film = render::panel_plan_at(&b.app.doc, 0, 0, Some(b.app.doc.paper));
    assert!(film.items.iter().all(|i| i.cel != cel_annot), "le plan du film n'a pas le calque d'annotation");
    assert!(images(&b.app.doc) == sans, "les images exportées sont celles d'avant l'annotation");
    assert!(render::composite_full(&b.app.doc, &film) == miniature_sans, "la vignette est celle d'avant");
    // Demandé expressément, il vient : par-dessus tout.
    let avec = render::panel_plan_at_avec(&b.app.doc, 0, 0, Some(b.app.doc.paper), true);
    assert_eq!(avec.items.last().map(|i| i.cel), Some(cel_annot));
    assert_eq!(avec.items.len(), film.items.len() + 1);
    // Même au fond de la pile, à l'écran, il est peint en dernier : par-dessus tout.
    let dernier = b.app.layers().len() - 1;
    b.app.layers_mut().swap(0, dernier);
    let plan = b.app.build_plan(0, false, true);
    assert_eq!(plan.items.last().map(|i| i.cel), Some(cel_annot), "tout en bas de la pile : peint en dernier quand même");
    assert!(plan.items.len() >= 2, "le dessin du plan est aussi là");
}

#[test]
fn an_annotation_layer_saves_reopens_and_does_not_leave_the_plan_when_duplicated_or_merged() {
    let mut b = banc_dessin();
    b.app.run_action("calque.nouveau_annotation");
    b.app.run_action("calque.nouveau_annotation");
    b.chauffer(2);
    let noms: Vec<String> = b.app.layers().iter().map(|l| l.name.clone()).collect();
    assert_eq!(&noms[noms.len() - 2..], ["Annotations", "Annotations 2"], "noms libres : {noms:?}");
    b.app.color = [255, 0, 0];
    trait_au_milieu(&mut b, 0.0);
    let li = b.app.cur_layer;
    assert!(encre_du_calque(&b, li));
    // Enregistré, rouvert : le calque est d'annotation, son dessin est là.
    let dir = std::env::temp_dir().join(format!("shot-test-calque-annot-plan-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(texte.contains("\"annotation\": true"), "le manifeste dit quel calque est d'annotation");
    let relu = crate::project::load(&manifeste).expect("ouverture");
    let l = &relu.doc.panels[0].layers[li];
    assert!(l.annotation && l.name == "Annotations 2" && l.kind == crate::document::LayerKind::Vector);
    assert!(l.frames.iter().flatten().any(|c| relu.doc.cels.get(c).is_some_and(|c| c.vector.as_ref().is_some_and(|v| !v.elements.is_empty()))), "son dessin est revenu");
    assert_eq!(relu.doc.panels[0].layers.iter().filter(|l| l.annotation).count(), 2);
    // Un calque ordinaire n'a pas le drapeau, et le manifeste ne l'écrit pas pour lui.
    assert!(!relu.doc.panels[0].layers[0].annotation);
    // Fusionner un calque d'annotation avec un calque ordinaire est refusé (ses notes n'entrent pas dans le dessin) ; avec un autre
    // calque d'annotation, c'est permis.
    let mut doc = relu.doc;
    assert!(doc.merge_layer_down(0, li).is_ok(), "deux calques d'annotation se fusionnent");
    let n = doc.panels[0].layers.len();
    doc.panels[0].layers.swap(0, n - 1);
    assert!(doc.merge_layer_down(0, 1).is_err(), "annotation et dessin ne se fusionnent pas");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_planche_never_draws_into_an_annotation_layer_of_the_plan() {
    let mut b = ouvert(quatre_plans(), 3);
    let avant = b.app.layers().len();
    // Un calque d'annotation de plan, choisi dans la vue Dessin, puis on passe à la planche : le calque courant n'est plus lui.
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.layers_mut().push(crate::document::Layer::new_annotation("Notes"));
    b.app.cur_layer = avant;
    assert!(b.app.layer().annotation);
    b.chauffer(3);
    assert!(!b.app.layer().annotation, "le dessin d'une case ne part pas dans un calque d'annotation");
    // Dans les Vignettes, « Nouveau calque d'annotation » crée un calque de la planche, pas du plan.
    let plans = b.app.layers().len();
    b.app.run_action("calque.nouveau_annotation");
    assert_eq!(b.app.layers().len(), plans, "aucun calque ajouté au plan");
    assert_eq!(b.app.doc.annotations.calques.len(), 1, "un calque d'annotation de la planche");
}

#[test]
fn an_annotation_layer_has_its_own_colour_and_is_not_given_a_key_by_a_new_case() {
    use crate::document::{Layer, LayerKind};
    // Sa couleur : celle du calque d'annotation du thème, pas celle d'un vectoriel, en vif et en sourd.
    let l = Layer::new_annotation("Notes");
    assert_eq!(crate::theme::couleur_du_calque(&l), crate::theme::palette().dessin_annotation);
    assert_ne!(crate::theme::couleur_du_calque(&l), crate::theme::couleur_calque(LayerKind::Vector, false));
    assert_ne!(crate::theme::fond_du_calque(&l), crate::theme::fond_calque(LayerKind::Vector, false));
    assert!(l.annotation && l.kind == LayerKind::Vector && l.pre == crate::document::Behaviour::Hold && l.post == crate::document::Behaviour::Hold);
    // Une case de plus dans le plan (une marque d'image) donne une clé de dessin au calque ordinaire, pas au calque d'annotation.
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].length = 6;
    let (c0, c1) = (doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector));
    doc.panels[0].layers[0].frames = vec![Some(c0); 6];
    let mut notes = Layer::new_annotation("Notes");
    notes.frames = vec![Some(c1); 6];
    doc.panels[0].layers.push(notes);
    assert_eq!(doc.cles_de_case(0, 3), 1, "une clé posée : celle du calque ordinaire");
    assert_ne!(doc.panels[0].layers[0].frames[3], Some(c0), "le calque ordinaire a son dessin à la nouvelle case");
    assert_eq!(doc.panels[0].layers[1].frames, vec![Some(c1); 6], "le calque d'annotation reste une feuille tenue sur tout le plan");
}
