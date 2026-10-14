//! Tests de la planche (phases 7 et 8) : vignettes en pages, place visée quand on lâche des
//! panneaux, déplacement et suppression de plusieurs panneaux, notes (traits, textes, attache).

use eframe::egui::{pos2, vec2, Rect};

use crate::board::{self, Board, BoardSettings, NoteRef, MARGIN, MAX_COLUMNS, PAGE_PAD};
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

fn settings(columns: usize, rows: usize, scene_breaks: bool) -> BoardSettings {
    BoardSettings { card_w: 200.0, columns, rows, spacing: 18.0, scene_breaks }
}

#[test]
fn cards_follow_each_other_in_pages() {
    let doc = doc();
    // Quatre colonnes, une ligne par page : deux pages.
    let l = board::layout(&doc, &settings(4, 1, false));
    let c = &l.cards;
    assert_eq!(c.len(), 6);
    assert_eq!(l.pages.len(), 2);
    assert_eq!(c[0].card.min, pos2(MARGIN + PAGE_PAD, MARGIN + PAGE_PAD));
    assert_eq!(c[0].thumb.height(), 200.0 * 90.0 / 160.0);
    assert_eq!(c[1].card.left(), c[0].card.right() + 18.0);
    assert_eq!(c[3].card.top(), c[0].card.top(), "les plans se suivent, scène ou pas");
    assert_eq!(c[4].card.left(), c[0].card.left());
    assert!(l.pages[1].contains(c[4].card.center()), "deuxième page");
    assert_eq!(l.empty.len(), 2, "cases vides au bout de la dernière page");
    assert!(l.size.y >= l.pages[1].bottom());
    // Une scène par ligne : la scène 2 commence une ligne.
    let l = board::layout(&doc, &settings(6, 0, true));
    assert_eq!(l.pages.len(), 1, "« toutes » : une seule page");
    assert_eq!(l.cards[4].card.left(), l.cards[0].card.left());
    assert!(l.cards[4].card.top() > l.cards[3].card.top());
    // Jamais plus de six colonnes.
    let l = board::layout(&doc, &settings(10, 0, false));
    assert_eq!(l.cards[5].card.top(), l.cards[0].card.top());
    assert_eq!(MAX_COLUMNS, 6);
    // Ne dépend pas de la taille de la vue : rien à donner d'autre que les réglages.
    assert_eq!(board::layout(&doc, &settings(4, 2, false)), board::layout(&doc, &settings(4, 2, false)));
}

#[test]
fn drop_targets_between_cards() {
    let doc = doc();
    let l = board::layout(&doc, &settings(4, 0, false));
    let c = &l.cards;
    let (s1, s2) = (doc.panels[0].scene, doc.panels[4].scene);
    let at = |k: usize, right: bool| pos2(if right { c[k].card.right() - 5.0 } else { c[k].card.left() + 5.0 }, c[k].card.center().y);
    let t = board::drop_target(&l, at(1, false), 18.0).unwrap();
    assert_eq!((t.scene, t.before), (s1, Some(1)));
    // Après la dernière vignette de la scène 1 : à la fin de la scène 1 (pas devant la scène 2).
    let t = board::drop_target(&l, at(3, true), 18.0).unwrap();
    assert_eq!((t.scene, t.before), (s1, None));
    let t = board::drop_target(&l, at(4, false), 18.0).unwrap();
    assert_eq!((t.scene, t.before), (s2, Some(4)));
    let t = board::drop_target(&l, at(4, true), 18.0).unwrap();
    assert_eq!((t.scene, t.before), (s2, Some(5)));
    // Lasso : les vignettes touchées.
    let r = Rect::from_two_pos(c[0].card.center(), c[1].card.center());
    assert_eq!(board::cards_in(&l, r), vec![0, 1]);
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
    assert!(doc.move_panels(&[], s1, None).is_none());
    assert!(doc.move_panels(&[all[1]], s1, Some(all[3])).is_none(), "déjà à sa place");
    // Vers la fin de la scène 2 : ils changent de scène.
    doc.move_panels(&[all[0], all[2]], s2, None).unwrap();
    assert_eq!(ids(&doc), vec![all[1], all[3], all[4], all[5], all[0], all[2]]);
    assert!(doc.panels[4..].iter().all(|p| p.scene == s2));
    // Toute une scène déplacée dans une autre : la scène vide disparaît.
    doc.move_panels(&[all[1], all[3]], s2, None).unwrap();
    assert_eq!(doc.scenes.len(), 1);
}

#[test]
fn removing_several_panels() {
    let mut doc = doc();
    let all = ids(&doc);
    let i = doc.remove_panels(&[all[1], all[2]]).unwrap();
    assert_eq!(ids(&doc), vec![all[0], all[3], all[4], all[5]]);
    assert_eq!(i, 1);
    assert!(doc.remove_panels(&[999]).is_none());
    doc.remove_panels(&ids(&doc)).unwrap();
    assert_eq!(ids(&doc), vec![all[0]], "tous : le premier reste");
}

fn pts(points: &[(f32, f32)]) -> Vec<VPoint> {
    points.iter().map(|&(x, y)| VPoint { x, y, r: 1.0 }).collect()
}

fn close(a: VPoint, x: f32, y: f32) -> bool {
    (a.x - x).abs() < 1e-3 && (a.y - y).abs() < 1e-3
}

#[test]
fn notes_erase_and_points() {
    let doc = doc();
    let l = board::layout(&doc, &settings(4, 0, false));
    let mut b = Board::default();
    b.add_stroke(&l, pts(&[(0.0, 0.0), (100.0, 0.0)]), [0; 3], None);
    b.add_stroke(&l, pts(&[(0.0, 50.0), (100.0, 50.0)]), [0; 3], None);
    assert!(!b.erase_at(&l, pos2(50.0, 25.0), 5.0), "entre les deux traits : rien");
    assert!(b.erase_at(&l, pos2(50.0, 4.0), 5.0), "touche le premier (rayon du trait compris)");
    assert_eq!(b.strokes.len(), 1);
    assert_eq!(b.stroke_at(&l, pos2(50.0, 50.0), 2.0), Some(0));
    let mut points = Vec::new();
    board::push_note_point(&mut points, VPoint { x: 0.0, y: 0.0, r: 1.0 }, 2.0);
    board::push_note_point(&mut points, VPoint { x: 1.0, y: 0.0, r: 3.0 }, 2.0);
    board::push_note_point(&mut points, VPoint { x: 5.0, y: 0.0, r: 1.0 }, 2.0);
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].r, 3.0);
}

#[test]
fn anchored_notes_follow_their_card() {
    let mut doc = doc();
    let all = ids(&doc);
    let s = settings(4, 0, false);
    let l = board::layout(&doc, &s);
    let card1 = l.cards[1].card;
    let mut b = Board::default();
    // Un trait et un texte posés sur la vignette 1, attachés à elle ; un trait libre.
    let on1 = card1.min + vec2(20.0, 30.0);
    b.add_stroke(&l, pts(&[(on1.x, on1.y), (on1.x + 40.0, on1.y)]), [0; 3], Some(all[1]));
    b.add_text(&l, on1, "Plan large".into(), 16.0, [0; 3], Some(all[1]));
    b.add_stroke(&l, pts(&[(5.0, 5.0), (9.0, 5.0)]), [0; 3], None);
    let p = b.stroke_points(&l, 0).unwrap();
    assert!(close(p[0], on1.x, on1.y), "à sa place tant que rien ne bouge");
    // Le panneau 1 passe en tête : la note le suit, la note libre ne bouge pas.
    doc.move_panels(&[all[1]], doc.panels[0].scene, Some(all[0])).unwrap();
    let l2 = board::layout(&doc, &s);
    let moved = l2.card_of(all[1]).unwrap().card;
    let p = b.stroke_points(&l2, 0).unwrap();
    assert!(close(p[0], moved.min.x + 20.0, moved.min.y + 30.0));
    let (tp, size) = b.text_place(&l2, 0).unwrap();
    assert!((tp - (moved.min + vec2(20.0, 30.0))).length() < 1e-3);
    assert!((size - 16.0).abs() < 1e-3);
    assert!(close(b.stroke_points(&l2, 1).unwrap()[0], 5.0, 5.0));
    // Vignettes deux fois plus grandes : la note attachée grandit avec.
    let big = board::layout(&doc, &BoardSettings { card_w: 400.0, ..s });
    let c = big.card_of(all[1]).unwrap().card;
    let p = b.stroke_points(&big, 0).unwrap();
    assert!(close(p[0], c.min.x + 40.0, c.min.y + 60.0));
    assert!((p[0].r - 2.0).abs() < 1e-3);
    assert!((b.text_place(&big, 0).unwrap().1 - 32.0).abs() < 1e-3);
    // Détacher : la note reste à sa place, puis ne suit plus.
    let before = b.stroke_points(&l2, 0).unwrap();
    b.set_anchor(&l2, &[NoteRef::Stroke(0)], None);
    assert_eq!(b.strokes[0].anchor, None);
    assert!(close(b.stroke_points(&big, 0).unwrap()[0], before[0].x, before[0].y));
    // Déplacer des notes : libres comme attachées, de la même distance à l'écran.
    b.translate(&l2, &[NoteRef::Stroke(0), NoteRef::Text(0)], vec2(10.0, 0.0));
    assert!(close(b.stroke_points(&l2, 0).unwrap()[0], before[0].x + 10.0, before[0].y));
    assert!((b.text_place(&l2, 0).unwrap().0.x - (moved.min.x + 30.0)).abs() < 1e-3);
    // Le panneau d'une note attachée disparaît : la note reste où elle était, libre.
    let place = b.text_place(&l2, 0).unwrap().0;
    b.release(&l2, &[all[1]]);
    assert_eq!(b.texts[0].anchor, None);
    assert!((b.text_place(&l2, 0).unwrap().0 - place).length() < 1e-3);
    b.remove(&[NoteRef::Text(0), NoteRef::Stroke(1)]);
    assert_eq!((b.strokes.len(), b.texts.len()), (1, 0));
}

#[test]
fn notes_are_undone_and_saved() {
    let mut doc = doc();
    let all = ids(&doc);
    let l = board::layout(&doc, &settings(4, 0, false));
    let snap = Edit::Structure { structure: doc.structure(), nav: Nav { panel: 0, layer: 0 } };
    doc.board.add_stroke(&l, pts(&[(10.0, 10.0), (60.0, 30.0)]), [10, 20, 30], None);
    doc.board.add_text(&l, pos2(300.0, 40.0), "Contre-champ\nplus serré".into(), 18.0, [200, 0, 0], Some(all[2]));
    let mut history = History::default();
    history.push(snap);
    let mut nav = Nav { panel: 0, layer: 0 };
    history.undo(&mut doc, &mut nav);
    assert!(doc.board.strokes.is_empty() && doc.board.texts.is_empty(), "les notes sont annulées");
    history.redo(&mut doc, &mut nav);
    assert_eq!(doc.board.texts.len(), 1);

    let dir = std::env::temp_dir().join(format!("pelure-test-planche-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.board, doc.board, "traits, textes et attaches relus");
}
