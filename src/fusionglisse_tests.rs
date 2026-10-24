//! Essais de « fusionner deux plans en glissant l'un sur l'autre » dans le montage (`fusionglisse.rs`).
//!
//! Les essais passent par la vraie fenêtre sans écran (`Banc`) : on retrouve où les plans sont tombés à l'écran par les blocs
//! que la bande du montage a peints (`storyboard::BLOCS_DU_MONTAGE`), puis on glisse avec le pointeur.
//!
//! Ce qu'ils gardent :
//! - lâcher au milieu d'un plan le fusionne, lâcher sur un bord (les deux quarts) déplace, comme avant ;
//! - le plan qui vient avant garde son identité, ses images avant celles de l'autre ; deux plans qui ne se touchent pas se
//!   fusionnent aussi (le plan glissé quitte sa place) ;
//! - aucun dessin n'est perdu ; une seule étape d'annulation rend les deux plans exactement ;
//! - Échap pendant le glissé n'a aucun effet ; le plan visé s'entoure et dit « Fusionner » ;
//! - ni de plan d'une autre séquence, ni de groupe de plans choisis, ni de storyboard verrouillé.

use std::collections::BTreeSet;

use eframe::egui::{pos2, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{instantane, meme_projet};
use crate::fusionglisse::{plan_vise, zone_du_milieu};
use crate::perf_tests::{banc_taille, Banc};
use crate::storyboard::{MontageDrag, BLOCS_DU_MONTAGE};
use crate::workspace::Espace;

const LONGUEUR: usize = 12;
const DT: f64 = 1.0 / 60.0;

/// Quatre plans de 12 images, chacun avec son dessin (le plan k montre le dessin `cels[k]`, sur un calque « Dessin »), dans une
/// seule séquence.
fn film() -> (Document, Vec<CelId>) {
    let mut doc = Document::with_frame(64, 48, 24);
    for i in 0..3 {
        doc.add_scene_after(i);
    }
    let mut cels = Vec::new();
    for i in 0..4 {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(LONGUEUR);
        let c = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
        let mut l = Layer::new("Dessin", LayerKind::Bitmap);
        l.frames = vec![Some(c); LONGUEUR];
        doc.panels[i].layers.push(l);
        doc.panels[i].action = format!("Action {i}");
        cels.push(c);
    }
    (doc, cels)
}

fn banc(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = 12.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_textes = true;
    b.chauffer(8);
    b
}

fn blocs() -> Vec<Rect> {
    BLOCS_DU_MONTAGE.with(|b| b.borrow().clone())
}

fn ids(b: &Banc) -> Vec<u64> {
    b.app.doc.panels.iter().map(|p| p.id).collect()
}

fn pointeur(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Le point du plan `k` à la fraction `t` de sa largeur, à mi-hauteur.
fn point(k: usize, t: f32) -> Pos2 {
    let r = blocs()[k];
    pos2(r.left() + r.width() * t, r.center().y)
}

/// Prend le plan `de` en son milieu et le tire jusqu'en `a`, sans lâcher.
fn tenir(b: &mut Banc, de: usize, a: Pos2) {
    let p = point(de, 0.5);
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![pointeur(p, true)], DT);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(p + (a - p) * (k as f32 / 6.0))], DT);
    }
    b.image(Vec::new(), DT);
}

fn lacher(b: &mut Banc, a: Pos2) {
    b.image(vec![pointeur(a, false)], DT);
    b.image(vec![Event::PointerGone], DT);
    b.chauffer(3);
}

/// Glisse le plan `de` jusqu'à la fraction `t` de la largeur du plan `vers`, et lâche.
fn glisser(b: &mut Banc, de: usize, vers: usize, t: f32) {
    let a = point(vers, t);
    tenir(b, de, a);
    lacher(b, a);
}

fn photo(b: &Banc) -> String {
    instantane(&b.app.doc, &BTreeSet::new())
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

fn cels_du_plan(b: &Banc, id: u64) -> Vec<Option<CelId>> {
    b.app.doc.panels.iter().find(|p| p.id == id).expect("plan").layers[0].frames.clone()
}

// ------------------------------------------------------------------------------------ le choix de la zone

#[test]
fn the_middle_zone_is_the_central_half_of_a_block_and_only_another_plan_of_the_same_sequence_is_aimed_at() {
    let r = Rect::from_min_max(pos2(100.0, 10.0), pos2(300.0, 90.0));
    let z = zone_du_milieu(r);
    assert_eq!((z.left(), z.right(), z.top(), z.bottom()), (150.0, 250.0, 10.0, 90.0));
    let (mut doc, _) = film();
    let blocs: Vec<Rect> = (0..4).map(|k| Rect::from_min_max(pos2(100.0 * k as f32, 0.0), pos2(100.0 * k as f32 + 98.0, 60.0))).collect();
    let id = |k: usize| doc.panels[k].id;
    // Au milieu du plan 2 (x = 249) : le plan 2 est visé ; sur ses bords (210, 288), non.
    assert_eq!(plan_vise(&doc, &blocs, pos2(249.0, 30.0), id(0), true), Some(id(2)));
    assert_eq!(plan_vise(&doc, &blocs, pos2(210.0, 30.0), id(0), true), None);
    assert_eq!(plan_vise(&doc, &blocs, pos2(288.0, 30.0), id(0), true), None);
    // Sur soi-même : rien. Un groupe de plans : rien.
    assert_eq!(plan_vise(&doc, &blocs, pos2(49.0, 30.0), id(0), true), None);
    assert_eq!(plan_vise(&doc, &blocs, pos2(249.0, 30.0), id(0), false), None);
    // Une autre séquence : rien.
    let before = doc.panels.len();
    doc.add_sequence_after(3);
    assert_eq!(doc.panels.len(), before + 1);
    let blocs5: Vec<Rect> = (0..5).map(|k| Rect::from_min_max(pos2(100.0 * k as f32, 0.0), pos2(100.0 * k as f32 + 98.0, 60.0))).collect();
    assert_eq!(plan_vise(&doc, &blocs5, pos2(449.0, 30.0), doc.panels[0].id, true), None, "le plan de la séquence 2 n'est pas visé");
    assert_eq!(plan_vise(&doc, &blocs5, pos2(249.0, 30.0), doc.panels[0].id, true), Some(doc.panels[2].id));
    // Storyboard verrouillé : rien.
    doc.storyboard_locked = true;
    assert_eq!(plan_vise(&doc, &blocs5, pos2(249.0, 30.0), doc.panels[0].id, true), None);
}

// ------------------------------------------------------------------------------------ lâcher : fusionner ou déplacer

#[test]
fn dropping_in_the_middle_merges_and_dropping_on_an_edge_moves() {
    // Milieu : fusion, il reste trois plans.
    let (doc, _) = film();
    let mut b = banc(doc);
    let v = ids(&b);
    glisser(&mut b, 0, 2, 0.5);
    assert_eq!(b.app.doc.panels.len(), 3, "deux plans n'en font plus qu'un");
    assert_eq!(ids(&b), vec![v[1], v[0], v[3]], "le plan glissé est venu contre l'autre : [p1, p0 + p2, p3]");
    assert_eq!(b.app.doc.panels[1].frame_count(), 2 * LONGUEUR);

    // Bord gauche (le premier quart) : déplacé devant, rien n'est fusionné.
    let (doc, _) = film();
    let mut b = banc(doc);
    let v = ids(&b);
    glisser(&mut b, 0, 2, 0.1);
    assert_eq!(ids(&b), vec![v[1], v[0], v[2], v[3]]);
    assert!(b.app.doc.panels.iter().all(|p| p.frame_count() == LONGUEUR));

    // Bord droit (le dernier quart) : déplacé derrière.
    let (doc, _) = film();
    let mut b = banc(doc);
    let v = ids(&b);
    glisser(&mut b, 0, 2, 0.9);
    assert_eq!(ids(&b), vec![v[1], v[2], v[0], v[3]]);
    assert!(b.app.doc.panels.iter().all(|p| p.frame_count() == LONGUEUR));
}

#[test]
fn the_plan_that_comes_first_keeps_its_identity_and_its_frames_go_first() {
    let (doc, cels) = film();
    // Le plan glissé vient avant le plan visé (p1 sur p3, ils ne se touchent pas) : ses images d'abord.
    let mut b = banc(doc);
    let v = ids(&b);
    glisser(&mut b, 1, 3, 0.5);
    assert_eq!(ids(&b), vec![v[0], v[2], v[1]], "[p0, p2, p1 + p3] : p1 garde son numéro");
    let frames = cels_du_plan(&b, v[1]);
    assert_eq!(frames.len(), 2 * LONGUEUR);
    assert!(frames[..LONGUEUR].iter().all(|c| *c == Some(cels[1])) && frames[LONGUEUR..].iter().all(|c| *c == Some(cels[3])), "images de p1, puis de p3");
    // Le plan glissé vient après le plan visé (p3 sur p1) : ses images après.
    let mut b = banc(film().0);
    glisser(&mut b, 3, 1, 0.5);
    assert_eq!(ids(&b), vec![v[0], v[1], v[2]], "[p0, p1 + p3, p2] : p1 garde son numéro");
    let frames = cels_du_plan(&b, v[1]);
    assert!(frames[..LONGUEUR].iter().all(|c| *c == Some(cels[1])) && frames[LONGUEUR..].iter().all(|c| *c == Some(cels[3])), "images de p1, puis de p3");
    // Voisins : p1 sur p2.
    let mut b = banc(film().0);
    glisser(&mut b, 1, 2, 0.5);
    assert_eq!(ids(&b), vec![v[0], v[1], v[3]]);
    let frames = cels_du_plan(&b, v[1]);
    assert!(frames[..LONGUEUR].iter().all(|c| *c == Some(cels[1])) && frames[LONGUEUR..].iter().all(|c| *c == Some(cels[2])));
    // Voisins dans l'autre sens : p2 sur p1.
    let mut b = banc(film().0);
    glisser(&mut b, 2, 1, 0.5);
    assert_eq!(ids(&b), vec![v[0], v[1], v[3]]);
    let frames = cels_du_plan(&b, v[1]);
    assert!(frames[..LONGUEUR].iter().all(|c| *c == Some(cels[1])) && frames[LONGUEUR..].iter().all(|c| *c == Some(cels[2])));
}

#[test]
fn a_merge_keeps_every_drawing_and_the_texts_in_order() {
    let (doc, cels) = film();
    let avant = crate::hasard_tests::dessins_ordinaires(&doc);
    let mut b = banc(doc);
    let v = ids(&b);
    glisser(&mut b, 0, 3, 0.5);
    let doc = &b.app.doc;
    let apres = doc.used_cels();
    assert!(avant.iter().all(|c| apres.contains(c)), "aucun dessin perdu");
    assert!(cels.iter().all(|c| apres.contains(c)));
    // Les deux plans : les textes du second rejoignent ceux du premier (ou une marque d'image), dans l'ordre.
    let p = doc.panels.iter().find(|p| p.id == v[0]).expect("p0");
    let tout = format!("{} {}", p.action, p.marks.iter().map(|m| m.action.clone()).collect::<Vec<_>>().join(" "));
    assert!(tout.contains("Action 0") && tout.contains("Action 3"), "{tout}");
    assert!(tout.find("Action 0") < tout.find("Action 3"));
}

#[test]
fn one_undo_step_gives_both_plans_back_exactly_and_redo_merges_again() {
    let (doc, _) = film();
    let mut b = banc(doc);
    let (avant, h0) = (photo(&b), etapes(&b));
    glisser(&mut b, 0, 2, 0.5);
    let merge = photo(&b);
    assert_ne!(merge, avant);
    assert_eq!(etapes(&b), h0 + 1, "une seule étape d'annulation");
    assert_eq!(b.app.history.names().0.last().map(|s| s.to_string()), Some("Fusionner deux plans".to_string()));
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    meme_projet(&avant, &photo(&b)).expect("annuler rend les deux plans exactement");
    assert_eq!(b.app.doc.panels.len(), 4);
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    meme_projet(&merge, &photo(&b)).expect("rétablir fusionne de nouveau");
}

// ------------------------------------------------------------------------------------ pendant le glissé

#[test]
fn the_target_is_outlined_and_says_merge_only_over_the_middle() {
    let (doc, _) = film();
    let mut b = banc(doc);
    let v = ids(&b);
    // Au milieu : le plan visé est celui du glissé, et « Fusionner » est peint.
    tenir(&mut b, 0, point(2, 0.5));
    match &b.app.montage_drag {
        Some(MontageDrag::Move { panel, merge, .. }) => assert_eq!((*panel, *merge), (v[0], Some(v[2]))),
        _ => panic!("un plan est en train d'être déplacé"),
    }
    assert!(b.textes.iter().any(|t| t == "Fusionner"), "le mot est peint sur le plan visé : {:?}", b.textes);
    // Sur un bord : plus de fusion, le repère de place est celui d'avant, et le mot a disparu.
    let a = point(2, 0.1);
    b.image(vec![Event::PointerMoved(a)], DT);
    b.image(Vec::new(), DT);
    match &b.app.montage_drag {
        Some(MontageDrag::Move { merge, target, .. }) => assert_eq!((*merge, *target), (None, 2)),
        _ => panic!("un plan est en train d'être déplacé"),
    }
    assert!(!b.textes.iter().any(|t| t == "Fusionner"));
    // Retour au milieu du plan : de nouveau visé ; puis au milieu de lui-même : rien.
    b.image(vec![Event::PointerMoved(point(2, 0.5))], DT);
    assert!(matches!(&b.app.montage_drag, Some(MontageDrag::Move { merge: Some(m), .. }) if *m == v[2]));
    b.image(vec![Event::PointerMoved(point(0, 0.5))], DT);
    assert!(matches!(&b.app.montage_drag, Some(MontageDrag::Move { merge: None, .. })), "un plan ne se fusionne pas avec lui-même");
    lacher(&mut b, point(0, 0.5));
    assert_eq!(b.app.doc.panels.len(), 4);
}

#[test]
fn escape_during_the_drag_does_nothing() {
    let (doc, _) = film();
    let mut b = banc(doc);
    let (avant, h0) = (photo(&b), etapes(&b));
    let a = point(2, 0.5);
    tenir(&mut b, 0, a);
    assert!(matches!(&b.app.montage_drag, Some(MontageDrag::Move { merge: Some(_), .. })));
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: false, repeat: false, modifiers: Modifiers::NONE }], DT);
    assert!(b.app.montage_drag.is_none(), "le glissé est abandonné");
    // On lâche ensuite : rien ne se passe.
    lacher(&mut b, a);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
    assert_eq!(etapes(&b), h0, "aucune étape d'annulation");
    assert_eq!(b.app.doc.panels.len(), 4);
}

#[test]
fn another_sequence_a_group_and_a_locked_storyboard_never_merge() {
    // Une autre séquence : le plan 3 (seul dans la sienne) n'est pas visé, et le lâcher ne fusionne rien.
    let (mut doc, _) = film();
    let nouveau = doc.add_sequence_after(2);
    assert_eq!(doc.panels.len(), 5);
    doc.panels[nouveau].set_length(LONGUEUR);
    let mut b = banc(doc);
    let n = b.app.doc.panels.len();
    tenir(&mut b, 0, point(nouveau, 0.5));
    assert!(matches!(&b.app.montage_drag, Some(MontageDrag::Move { merge: None, .. })), "une autre séquence : pas de fusion");
    lacher(&mut b, point(nouveau, 0.5));
    assert_eq!(b.app.doc.panels.len(), n, "aucun plan n'a disparu");

    // Plusieurs plans choisis, glissés ensemble : ils se déplacent, sans fusionner.
    let (doc, _) = film();
    let mut b = banc(doc);
    let v = ids(&b);
    b.app.board_sel = [v[0], v[1]].into_iter().collect();
    tenir(&mut b, 0, point(3, 0.5));
    assert!(matches!(&b.app.montage_drag, Some(MontageDrag::Move { merge: None, .. })), "un groupe ne fusionne pas");
    lacher(&mut b, point(3, 0.5));
    assert_eq!(b.app.doc.panels.len(), 4);

    // Storyboard verrouillé : le plan ne part même pas.
    let (mut doc, _) = film();
    doc.storyboard_locked = true;
    let mut b = banc(doc);
    let (avant, h0) = (photo(&b), etapes(&b));
    glisser(&mut b, 0, 2, 0.5);
    meme_projet(&avant, &photo(&b)).expect("verrouillé : rien ne bouge");
    assert_eq!(etapes(&b), h0);
}

// ------------------------------------------------------------------------------------ le modèle

#[test]
fn merging_in_the_document_works_in_both_directions_and_refuses_what_it_cannot_do() {
    let (mut doc, cels) = film();
    let v: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    // Soi-même, un plan qui n'existe pas : rien, et rien n'a bougé.
    assert_eq!(doc.merge_plan_into(v[0], v[0]), None);
    assert_eq!(doc.merge_plan_into(v[0], 999_999), None);
    assert_eq!(doc.panels.len(), 4);
    // Après vers avant : p3 dans p0 → [p0 + p3, p1, p2].
    assert_eq!(doc.merge_plan_into(v[3], v[0]), Some(0));
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![v[0], v[1], v[2]]);
    let f = &doc.panels[0].layers[0].frames;
    assert!(f[..LONGUEUR].iter().all(|c| *c == Some(cels[0])) && f[LONGUEUR..].iter().all(|c| *c == Some(cels[3])));
    // Avant vers après : p1 dans p2 → [p0 + p3, p1 + p2].
    assert_eq!(doc.merge_plan_into(v[1], v[2]), Some(1));
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![v[0], v[1]]);
    // Deux séquences : refusé, rien ne bouge.
    let (mut doc, _) = film();
    doc.add_sequence_after(3);
    let (a, b) = (doc.panels[0].id, doc.panels[4].id);
    let avant = instantane(&doc, &BTreeSet::new());
    assert_eq!(doc.merge_plan_into(a, b), None);
    assert_eq!(instantane(&doc, &BTreeSet::new()), avant);
}
