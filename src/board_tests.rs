//! Tests de la planche (phase 7) : disposition des vignettes, place visée quand on lâche des
//! panneaux, déplacement et suppression de plusieurs panneaux, notes libres.

use eframe::egui::{pos2, Rect};

use crate::board::{self, Board, NoteStroke, GAP, HEADER_H, MARGIN, SCENE_H};
use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::project;
use crate::vector::VPoint;

/// Scène 1 de quatre panneaux, scène 2 de deux, cadre 16:9.
fn doc() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    for i in 0..3 {
        doc.add_panel_after(i);
    }
    let s = doc.add_scene_after(3);
    doc.add_panel_after(s);
    assert_eq!(doc.panels.len(), 6);
    doc
}

fn ids(doc: &Document) -> Vec<u64> {
    doc.panels.iter().map(|p| p.id).collect()
}

#[test]
fn cards_wrap_by_scene() {
    let doc = doc();
    // Largeur pour deux vignettes de 200 par ligne.
    let wrap = MARGIN * 2.0 + 200.0 * 2.0 + GAP + 10.0;
    let l = board::layout(&doc, 200.0, wrap);
    assert_eq!(l.cards.len(), 6);
    assert_eq!(l.scenes.len(), 2);
    let c = &l.cards;
    assert_eq!(c[0].card.min, pos2(MARGIN, MARGIN + SCENE_H));
    assert_eq!(c[0].thumb.height(), 200.0 * 90.0 / 160.0);
    assert_eq!(c[0].thumb.top(), c[0].card.top() + HEADER_H);
    assert_eq!(c[1].card.top(), c[0].card.top(), "deux par ligne");
    assert!(c[2].card.top() > c[0].card.bottom(), "la troisième revient à la ligne");
    assert_eq!(c[2].card.left(), MARGIN);
    // La scène 2 commence sous la scène 1, avec son titre.
    assert!(l.scenes[1].title.y > c[3].card.bottom());
    assert_eq!(c[4].card.top(), l.scenes[1].title.y + SCENE_H);
    // Au moins une vignette par ligne, même trop étroit.
    let narrow = board::layout(&doc, 200.0, 50.0);
    assert!(narrow.cards.windows(2).filter(|w| w[0].panel + 1 == w[1].panel).all(|w| w[1].card.top() > w[0].card.top()));
    assert!(l.size.x >= c[1].card.right() && l.size.y >= c[5].card.bottom());
}

#[test]
fn drop_targets_between_cards() {
    let doc = doc();
    let wrap = MARGIN * 2.0 + 200.0 * 2.0 + GAP + 10.0;
    let l = board::layout(&doc, 200.0, wrap);
    let c = &l.cards;
    let s1 = doc.panels[0].scene;
    let s2 = doc.panels[4].scene;
    // Moitié gauche de la vignette 1 : devant elle.
    let t = board::drop_target(&l, pos2(c[1].card.left() + 10.0, c[1].card.center().y)).unwrap();
    assert_eq!((t.scene, t.before), (s1, Some(1)));
    // Moitié droite de la vignette 1 (fin de ligne) : devant la première de la ligne suivante.
    let t = board::drop_target(&l, pos2(c[1].card.right() - 10.0, c[1].card.center().y)).unwrap();
    assert_eq!((t.scene, t.before), (s1, Some(2)));
    // Après la dernière vignette de la scène : à la fin de la scène.
    let t = board::drop_target(&l, pos2(c[3].card.right() + 30.0, c[3].card.center().y)).unwrap();
    assert_eq!((t.scene, t.before), (s1, None));
    // Plus bas : la scène 2.
    let t = board::drop_target(&l, pos2(MARGIN, c[4].card.center().y)).unwrap();
    assert_eq!((t.scene, t.before), (s2, Some(4)));
    // Lasso : les vignettes touchées.
    let r = Rect::from_two_pos(c[0].card.center(), c[1].card.center());
    assert_eq!(board::cards_in(&l, r), vec![0, 1]);
    let r = Rect::from_two_pos(c[0].card.center(), c[3].card.center());
    assert_eq!(board::cards_in(&l, r), vec![0, 1, 2, 3]);
}

#[test]
fn moving_several_panels() {
    let mut doc = doc();
    let all = ids(&doc);
    let s1 = doc.panels[0].scene;
    let s2 = doc.panels[4].scene;
    // Les panneaux 1 et 3 devant le panneau 0 : ils gardent leur ordre.
    let i = doc.move_panels(&[all[3], all[1]], s1, Some(all[0])).unwrap();
    assert_eq!(i, 0);
    assert_eq!(ids(&doc), vec![all[1], all[3], all[0], all[2], all[4], all[5]]);
    // Rien ne change : aucun panneau à déplacer ou déjà à sa place.
    assert!(doc.move_panels(&[], s1, None).is_none());
    assert!(doc.move_panels(&[all[1]], s1, Some(all[3])).is_none());
    // Vers la fin de la scène 2 : ils changent de scène.
    doc.move_panels(&[all[0], all[2]], s2, None).unwrap();
    assert_eq!(ids(&doc), vec![all[1], all[3], all[4], all[5], all[0], all[2]]);
    assert!(doc.panels[4..].iter().all(|p| p.scene == s2));
    // Devant un panneau qui part lui aussi : devant le suivant qui reste.
    doc.move_panels(&[all[4], all[5]], s2, Some(all[4])).map(|_| ());
    assert_eq!(ids(&doc).len(), 6);
    // Toute une scène déplacée dans une autre : la scène vide disparaît.
    doc.move_panels(&[all[1], all[3]], s2, None).unwrap();
    assert_eq!(doc.scenes.len(), 1);
    assert!(doc.panels.iter().all(|p| p.scene == s2));
}

#[test]
fn removing_several_panels() {
    let mut doc = doc();
    let all = ids(&doc);
    let i = doc.remove_panels(&[all[1], all[2]]).unwrap();
    assert_eq!(ids(&doc), vec![all[0], all[3], all[4], all[5]]);
    assert_eq!(i, 1);
    assert!(doc.remove_panels(&[999]).is_none());
    // Tous : le premier reste.
    doc.remove_panels(&ids(&doc)).unwrap();
    assert_eq!(ids(&doc), vec![all[0]]);
}

fn stroke(points: &[(f32, f32)]) -> NoteStroke {
    NoteStroke { points: points.iter().map(|&(x, y)| VPoint { x, y, r: 1.0 }).collect(), color: [20, 20, 20] }
}

#[test]
fn notes_erase_and_points() {
    let mut b = Board { strokes: vec![stroke(&[(0.0, 0.0), (100.0, 0.0)]), stroke(&[(0.0, 50.0), (100.0, 50.0)])] };
    assert!(!b.erase_at(pos2(50.0, 25.0), 5.0), "entre les deux traits : rien");
    assert!(b.erase_at(pos2(50.0, 4.0), 5.0), "touche le premier (rayon du trait compris)");
    assert_eq!(b.strokes.len(), 1);
    assert_eq!(b.strokes[0].points[0].y, 50.0);
    assert!(b.strokes[0].bounds().contains(pos2(50.0, 50.0)));
    // Points trop proches : fondus, le plus gros rayon gardé.
    let mut pts = Vec::new();
    board::push_note_point(&mut pts, VPoint { x: 0.0, y: 0.0, r: 1.0 }, 2.0);
    board::push_note_point(&mut pts, VPoint { x: 1.0, y: 0.0, r: 3.0 }, 2.0);
    board::push_note_point(&mut pts, VPoint { x: 5.0, y: 0.0, r: 1.0 }, 2.0);
    assert_eq!(pts.len(), 2);
    assert_eq!(pts[0].r, 3.0);
}

#[test]
fn notes_are_undone_and_saved() {
    let mut doc = doc();
    let snap = Edit::Structure { structure: doc.structure(), nav: Nav { panel: 0, layer: 0 } };
    doc.board.strokes.push(stroke(&[(10.0, 10.0), (60.0, 30.0)]));
    let mut history = History::default();
    history.push(snap);
    let mut nav = Nav { panel: 0, layer: 0 };
    history.undo(&mut doc, &mut nav);
    assert!(doc.board.strokes.is_empty(), "la note est annulée");
    history.redo(&mut doc, &mut nav);
    assert_eq!(doc.board.strokes.len(), 1);

    let dir = std::env::temp_dir().join(format!("pelure-test-planche-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.board, doc.board);
}
