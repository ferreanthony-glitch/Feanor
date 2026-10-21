//! Tests de la planche (phases 7, 8 et 13a) : une vignette par image de storyboard, en pages,
//! place visée quand on lâche des plans, déplacement et suppression de plusieurs plans, notes
//! (traits, textes, attache).

use eframe::egui::{pos2, vec2, Rect};

use crate::board::{self, gap_cut, gap_same, Board, BoardSettings, Hit, NoteRef, BAR_CUT_W, BAR_W, GAP_CUT, GAP_SAME, LABEL_H, MARGIN, MAX_COLUMNS, NOTE_GAP, NOTE_LINE_H, NOTE_MAX_LINES, PAGE_PAD, ROW_GAP};
use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::project;
use crate::vector::VPoint;

/// Quatre plans, six vignettes (cadre 16:9) : le plan 1 et ses deux marques d'image (un ancien
/// projet de trois panneaux, converti), les plans 2 et 3, puis le plan 4 dans une deuxième
/// séquence.
fn doc() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    let p2 = doc.add_scene_after(2);
    let p3 = doc.add_scene_after(p2);
    doc.add_sequence_after(p3);
    doc.into_plans();
    assert!(doc.is_plans());
    assert_eq!((doc.panels.len(), doc.panels[0].marks.len()), (4, 2));
    doc
}

fn ids(doc: &Document) -> Vec<u64> {
    doc.panels.iter().map(|p| p.id).collect()
}

fn settings(columns: usize, rows: usize) -> BoardSettings {
    BoardSettings { card_w: 200.0, columns, rows }
}

/// La hauteur d'une ligne dont les notes les plus hautes ont `lines` lignes (cadre 16:9, vignettes de 200 de large).
fn row_height(lines: usize) -> f32 {
    LABEL_H + 200.0 * 90.0 / 160.0 + NOTE_GAP + lines as f32 * NOTE_LINE_H + ROW_GAP
}

#[test]
fn cards_follow_each_other_in_pages() {
    let doc = doc();
    // Quatre colonnes, une ligne par page : deux pages.
    let l = board::layout(&doc, &settings(4, 1));
    let c = &l.cards;
    assert_eq!(c.len(), 6, "une vignette par image de storyboard");
    assert_eq!((c[0].id, c[1].id, c[2].id), (doc.panels[0].id, doc.panels[0].marks[0].id, doc.panels[0].marks[1].id));
    assert_eq!((c[1].frame, c[1].end), (doc.panels[0].marks[0].frame, doc.panels[0].marks[1].frame));
    assert_eq!(c[2].end, doc.panels[0].frame_count());
    assert!(c[0].starts_plan() && !c[1].starts_plan() && c[3].starts_plan());
    assert_eq!(l.pages.len(), 2);
    assert_eq!(c[0].card.min, pos2(MARGIN + PAGE_PAD, MARGIN + PAGE_PAD));
    assert_eq!(c[0].thumb.min, c[0].card.min + vec2(0.0, LABEL_H), "le numéro du plan est dans la ligne au-dessus de l'image");
    assert_eq!(c[0].thumb.height(), 200.0 * 90.0 / 160.0);
    assert_eq!(c[0].thumb.width(), 200.0);
    assert_eq!(c[3].card.top(), c[0].card.top(), "les plans se suivent sur la même ligne : une ligne ne se casse pas à une coupe");
    assert!(c[4].card.top() > c[0].card.top() && c[4].card.left() == c[0].card.left(), "quatre colonnes : la cinquième image est au début de la ligne suivante");
    assert!(l.pages[1].contains(c[5].card.center()), "deuxième page");
    assert_eq!(l.empty.len(), 1, "une seule case vide, juste après la dernière vignette");
    assert!(l.pages[1].contains(l.empty[0].center()), "elle est sur la dernière page, qui s'arrête après sa rangée");
    assert!(l.size.y >= l.pages[1].bottom());
    // « toutes » : une seule page, une ligne neuve quand la ligne est pleine.
    let l = board::layout(&doc, &settings(6, 0));
    assert_eq!(l.pages.len(), 1);
    assert_eq!(l.cards[5].card.top(), l.cards[0].card.top(), "six images tiennent sur six colonnes");
    assert_eq!(l.empty.len(), 1, "la ligne pleine appelle une rangée neuve, avec sa seule case vide : on peut toujours continuer");
    assert!(l.empty[0].top() > l.cards[0].thumb.top());
    assert_eq!(l.pages[0].bottom(), l.empty[0].bottom() + board::NOTE_GAP + NOTE_LINE_H + PAGE_PAD, "la planche s'arrête après la rangée de la case vide");
    // Jamais plus de six colonnes.
    let l = board::layout(&doc, &settings(10, 0));
    assert_eq!(l.cards[5].card.top(), l.cards[0].card.top());
    assert_eq!(MAX_COLUMNS, 6);
    // Ne dépend pas de la taille de la vue : rien à donner d'autre que les réglages.
    assert_eq!(board::layout(&doc, &settings(4, 2)), board::layout(&doc, &settings(4, 2)));
}

#[test]
fn spacing_is_tight_inside_a_plan_and_wide_at_a_cut() {
    let doc = doc();
    let l = board::layout(&doc, &settings(6, 0));
    let c = &l.cards;
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    let (same, cut) = (gap_same(200.0), gap_cut(200.0));
    assert!(near(c[1].card.left(), c[0].card.right() + same), "deux images d'un même plan : 10 pour 136, soit {same} pour 200");
    assert!(near(c[2].card.left(), c[1].card.right() + same));
    assert!(near(c[3].card.left(), c[2].card.right() + cut), "à une coupe : 30 pour 136, soit {cut} pour 200");
    assert!(near(c[4].card.left(), c[3].card.right() + cut));
    // Les proportions de la maquette (136 de large, 10 et 30 d'écart), à la largeur de référence.
    assert_eq!((GAP_SAME, GAP_CUT), (18.0, 53.0));
    assert!((gap_same(136.0) - 10.0).abs() < 0.5 && (gap_cut(136.0) - 30.0).abs() < 1.0, "pour la vignette de 136 de la maquette : 10 et 30");
    assert!(gap_same(300.0) > gap_same(200.0) && gap_cut(300.0) > gap_cut(200.0), "les écarts suivent la largeur des vignettes");
    // Les lignes ont un nombre fixe d'images (les colonnes) : la ligne suivante commence au bord gauche.
    let l = board::layout(&doc, &settings(4, 0));
    assert_eq!(l.cards[4].card.left(), l.cards[0].card.left());
    assert!(l.cards[4].card.top() > l.cards[0].card.top());
    // Les pages sont assez larges pour la ligne la plus large (quatre coupes).
    assert!(l.pages[0].width() >= PAGE_PAD * 2.0 + 4.0 * 200.0 + 3.0 * gap_cut(200.0) - 1e-3);
}

#[test]
fn bars_are_grey_in_a_plan_black_at_a_cut_and_the_last_leads_to_the_empty_box() {
    let doc = doc();
    let l = board::layout(&doc, &settings(6, 0));
    assert_eq!(l.bars.len(), 6, "une barre après chaque image, la dernière avant la première case vide");
    assert_eq!(l.bars.iter().map(|b| b.cut).collect::<Vec<_>>(), vec![false, false, true, true, true, true]);
    for (k, b) in l.bars.iter().enumerate() {
        assert_eq!(b.after, k);
        let gap = if b.cut { gap_cut(200.0) } else { gap_same(200.0) };
        assert_eq!(b.bar.width(), if b.cut { BAR_CUT_W } else { BAR_W }, "la coupe est un peu plus épaisse que la barre grise");
        assert!((b.bar.height() - l.cards[k].thumb.height() / 3.0).abs() < 1e-3, "un tiers de la hauteur de la vignette");
        assert!((b.bar.center() - pos2(l.cards[k].card.right() + gap * 0.5, l.cards[k].thumb.center().y)).length() < 1e-3);
        assert_eq!(b.zone.left(), l.cards[k].card.right());
        assert!((b.zone.width() - gap).abs() < 1e-3);
    }
    // La dernière barre, grise : la première case vide est une image de plus du dernier plan (écart serré).
    let (joined, apart) = (board::layout_with(&doc, &settings(4, 0), &Default::default(), true), board::layout(&doc, &settings(4, 0)));
    assert!(!joined.bars[5].cut && apart.bars[5].cut);
    assert!((joined.empty[0].left() - (joined.cards[5].card.right() + gap_same(200.0))).abs() < 1e-3);
    assert!((apart.empty[0].left() - (apart.cards[5].card.right() + gap_cut(200.0))).abs() < 1e-3);
    // Au bout d'une ligne (trois colonnes), la barre est après la dernière image de la ligne.
    let three = board::layout(&doc, &settings(3, 0));
    assert_eq!(three.cards[3].row, 1);
    assert_eq!(three.bars[2].after, 2);
    assert!(three.bars[2].bar.center().y < three.cards[3].thumb.top(), "sur la ligne de l'image 3, pas sur la suivante");
    assert!(three.bars[2].bar.center().x > three.cards[2].card.right());
    assert!(three.bars[2].cut);
}

#[test]
fn a_row_goes_down_just_enough_for_a_note_of_two_or_three_lines() {
    let doc = doc();
    let s = settings(4, 0);
    let plain = board::layout(&doc, &s);
    assert_eq!(plain.cards[4].card.top() - plain.cards[0].card.top(), row_height(1));
    assert_eq!(plain.cards[0].note.height(), NOTE_LINE_H, "une bande vide garde une ligne");
    assert_eq!(plain.cards[0].note.top(), plain.cards[0].thumb.bottom() + NOTE_GAP);
    for lines in [2usize, 3] {
        let mut notes = std::collections::HashMap::new();
        notes.insert(doc.panels[0].marks[0].id, lines);
        let l = board::layout_with(&doc, &s, &notes, false);
        assert_eq!(l.cards[1].note.height(), lines as f32 * NOTE_LINE_H);
        assert_eq!(l.cards[4].card.top() - l.cards[0].card.top(), row_height(lines), "la rangée suivante descend d'autant ({lines} lignes)");
        assert_eq!(l.cards[0].note.height(), NOTE_LINE_H, "les autres bandes gardent leur ligne");
        assert!(l.cards[4].card.top() >= l.cards[1].note.bottom() + ROW_GAP - 1e-3, "la marge sous la note est gardée");
    }
    // Quatre lignes ne sont jamais affichées : au plus trois.
    let mut notes = std::collections::HashMap::new();
    notes.insert(doc.panels[0].id, 9);
    let l = board::layout_with(&doc, &s, &notes, false);
    assert_eq!(l.cards[0].note.height(), NOTE_MAX_LINES as f32 * NOTE_LINE_H);
    // Une note sur la dernière ligne de la planche descend la page, pas ce qui est avant.
    let mut notes = std::collections::HashMap::new();
    notes.insert(doc.panels[3].id, 2);
    let l = board::layout_with(&doc, &s, &notes, false);
    assert_eq!(l.cards[4].card.top(), plain.cards[4].card.top());
    assert!(l.pages[0].height() > plain.pages[0].height());
}

#[test]
fn hit_testing_tells_images_notes_bars_and_empty_boxes_apart() {
    let doc = doc();
    let l = board::layout(&doc, &settings(4, 0));
    let c = &l.cards;
    assert_eq!(board::hit(&l, c[0].thumb.center(), 0.0), Hit::Thumb(0));
    assert_eq!(board::hit(&l, c[3].note.center(), 0.0), Hit::Note(3));
    assert_eq!(board::hit(&l, c[0].label().center(), 0.0), Hit::Nothing, "le numéro du plan ne prend aucun geste");
    assert_eq!(board::hit(&l, l.empty[0].center(), 0.0), Hit::Empty(0));
    // Une seule case vide : à côté d'elle, plus rien (pas de grille de cases vides).
    assert_eq!(l.empty.len(), 1);
    let apres = l.empty[0].translate(vec2(l.empty[0].width() + gap_cut(200.0), 0.0)).center();
    assert_eq!(board::hit(&l, apres, 0.0), Hit::Nothing, "pas de case vide au-delà de la première");
    let zone = l.bars[1].zone;
    assert_eq!(board::hit(&l, zone.center(), 0.0), Hit::Bar(1));
    // L'écart d'un même plan est étroit : à l'écran réduit, la zone est élargie pour qu'on la touche.
    let near = pos2(zone.left() - 2.0, zone.center().y);
    assert_eq!(board::hit(&l, near, 0.0), Hit::Thumb(1));
    assert_eq!(board::hit(&l, near, 24.0), Hit::Bar(1));
    // Hors de tout : le fond de la page.
    assert_eq!(board::hit(&l, pos2(2.0, 2.0), 0.0), Hit::Nothing);
    // Lasso : les plans touchés, une fois chacun.
    let r = Rect::from_two_pos(c[0].card.center(), c[1].card.center());
    assert_eq!(board::cards_in(&l, r), vec![0]);
    let r = Rect::from_two_pos(c[2].card.center(), c[3].card.center());
    assert_eq!(board::cards_in(&l, r), vec![0, 1]);
}
#[test]
fn moving_several_plans() {
    let mut doc = doc();
    let all = ids(&doc);
    // Les plans 3 et 2 devant le plan 1 : ils gardent l'ordre du film.
    let i = doc.move_plans(&[all[2], all[1]], Some(all[0])).unwrap();
    assert_eq!(i, 0);
    assert_eq!(ids(&doc), vec![all[1], all[2], all[0], all[3]]);
    assert!(doc.move_plans(&[], None).is_none());
    assert!(doc.move_plans(&[all[1]], Some(all[2])).is_none(), "déjà à sa place");
    assert!(doc.is_plans());
}

#[test]
fn removing_several_plans() {
    let mut doc = doc();
    let all = ids(&doc);
    let i = doc.remove_panels(&[all[1], all[2]]).unwrap();
    assert_eq!(ids(&doc), vec![all[0], all[3]]);
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
    let l = board::layout(&doc, &settings(4, 0));
    let mut b = Board::default();
    b.add_stroke(&l, pts(&[(0.0, 0.0), (100.0, 0.0)]), [0; 3], None);
    b.add_stroke(&l, pts(&[(0.0, 50.0), (100.0, 50.0)]), [0; 3], None);
    assert!(!b.erase_at(&l, pos2(50.0, 25.0), 5.0), "entre les deux traits : rien");
    assert!(b.erase_at(&l, pos2(50.0, 4.0), 5.0), "touche le premier (rayon du trait compris)");
    assert_eq!(b.strokes.len(), 1);
    assert_eq!(b.stroke_at(&l, pos2(50.0, 50.0), 2.0), Some(0));
}

#[test]
fn anchored_notes_follow_their_card() {
    let mut doc = doc();
    let all = ids(&doc);
    let s = settings(4, 0);
    let l = board::layout(&doc, &s);
    let card1 = l.card_of(all[1]).unwrap().card;
    let mut b = Board::default();
    // Un trait et un texte posés sur la vignette du plan 2, attachés à elle ; un trait libre.
    let on1 = card1.min + vec2(20.0, 30.0);
    b.add_stroke(&l, pts(&[(on1.x, on1.y), (on1.x + 40.0, on1.y)]), [0; 3], Some(all[1]));
    b.add_text(&l, on1, "Plan large".into(), 16.0, [0; 3], Some(all[1]));
    b.add_stroke(&l, pts(&[(5.0, 5.0), (9.0, 5.0)]), [0; 3], None);
    let p = b.stroke_points(&l, 0).unwrap();
    assert!(close(p[0], on1.x, on1.y), "à sa place tant que rien ne bouge");
    // Le plan 2 passe en tête : la note le suit, la note libre ne bouge pas.
    doc.move_plans(&[all[1]], Some(all[0])).unwrap();
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
    // L'image d'une note attachée disparaît : la note reste où elle était, libre.
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
    let l = board::layout(&doc, &settings(4, 0));
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
    assert_eq!(back.panels[0].marks, doc.panels[0].marks, "marques d'image relues");
}

#[test]
fn plan_color_is_saved_copied_and_undone() {
    let mut doc = doc();
    let all = ids(&doc);
    // Une couleur hors palette est refusée ; rien ne change si c'est la même.
    assert!(!doc.set_panels_color(&all, Some(99)));
    assert!(doc.set_panels_color(&all[1..3], Some(3)));
    assert!(!doc.set_panels_color(&all[1..3], Some(3)));
    assert_eq!(doc.panels.iter().map(|p| p.color).collect::<Vec<_>>(), vec![None, Some(3), Some(3), None]);

    // Annulation : une étape d'avant le changement.
    let snap = Edit::Structure { structure: doc.structure(), nav: Nav { panel: 0, layer: 0 } };
    let mut history = History::default();
    history.push(snap);
    doc.set_panels_color(&all[1..2], None);
    assert_eq!(doc.panels[1].color, None);
    let mut nav = Nav { panel: 0, layer: 0 };
    history.undo(&mut doc, &mut nav);
    assert_eq!(doc.panels[1].color, Some(3), "Ctrl+Z rend la couleur");
    history.redo(&mut doc, &mut nav);
    assert_eq!(doc.panels[1].color, None);
    history.undo(&mut doc, &mut nav);

    // Copiée avec le plan : dupliquer, copier-coller, nouveau projet des plans choisis.
    let copy = doc.duplicate_panel(1);
    assert_eq!(copy.color, Some(3));
    let clip = doc.copy_panels(&all[2..3]).unwrap();
    let at = doc.paste_panels(&clip, 3).unwrap();
    assert_eq!(doc.panels[at].color, Some(3));
    let mut sub = doc.from_plans(&[all[1], all[3]]).unwrap();
    assert_eq!(sub.panels.iter().map(|p| p.color).collect::<Vec<_>>(), vec![Some(3), None]);

    // Enregistrée et relue.
    let dir = std::env::temp_dir().join(format!("pelure-test-couleur-plan-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    project::save(&sub, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels.iter().map(|p| p.color).collect::<Vec<_>>(), sub.panels.iter().map(|p| p.color).collect::<Vec<_>>());

    // Un projet sans couleur n'écrit rien, et se relit sans couleur (comme un projet d'avant).
    let all = ids(&sub);
    sub.set_panels_color(&all, None);
    project::save(&sub, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(!text.contains("\"couleur\""), "pas de champ quand il n'y a pas de couleur");
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert!(back.panels.iter().all(|p| p.color.is_none()));
}
