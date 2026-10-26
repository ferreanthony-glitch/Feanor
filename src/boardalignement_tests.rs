//! Essais de l'alignement des mouvements de caméra sur la planche des Vignettes (Anthony, 2026-10-24) : le cadre le plus haut d'un mouvement
//! s'aligne sur le haut des images de la rangée (A est en retrait, plus bas), le plus à gauche sur le bord de la colonne à la japonaise.

use std::collections::HashMap;

use eframe::egui::Rect;

use crate::board::{self, BoardLayout, BoardSettings, Options, ROW_GAP};
use crate::boardsens::Sens;
use crate::document::Document;
use crate::vignettecam::{coins_de, se_recouvrent};
use crate::vignettes_camera_tests::{deux_cadres, plan_court, sans_chevauchement, K};

fn reglages(colonnes: usize) -> BoardSettings {
    BoardSettings { card_w: 170.0, columns: colonnes, rows: 0 }
}

fn en_lignes(doc: &Document, colonnes: usize) -> BoardLayout {
    board::layout_with(doc, &reglages(colonnes), &HashMap::new(), false, &Options::default())
}

fn a_la_japonaise(doc: &Document, colonnes: usize) -> BoardLayout {
    board::layout_with(doc, &reglages(colonnes), &HashMap::new(), false, &Options { sens: Sens::Japonais, ..Default::default() })
}

fn proche(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-2
}

fn film_de_deux_cadres(dx: f32, dy: f32) -> Document {
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, dx, dy);
    doc
}

/// Le rectangle qui entoure tous les cadres du mouvement de la case `K` (A comprise).
fn boite_des_cadres(l: &BoardLayout) -> Rect {
    l.move_of(K).unwrap().cadres.iter().fold(l.cards[K].thumb, |r, c| r.union(c.rect))
}

#[test]
fn an_upward_move_aligns_its_topmost_frame_on_the_row_line_and_a_hangs_lower() {
    let ordinaire = en_lignes(&plan_court(), 4);
    // B au-dessus de A : à cheval (-45), juste au-dessus (-90), en haut à gauche (-80, -60), en haut à droite (80, -60), très haut (0, -200).
    for (dx, dy) in [(0.0, -45.0), (0.0, -90.0), (-80.0, -60.0), (80.0, -60.0), (0.0, -200.0)] {
        for colonnes in [4, 6] {
            let doc = film_de_deux_cadres(dx, dy);
            let l = en_lignes(&doc, colonnes);
            let etiquette = format!("({dx}, {dy}), {colonnes} colonnes");
            let (c, m) = (l.cards[K], l.move_of(K).expect("une empreinte"));
            // La ligne de la rangée de A : le haut des images des autres cases de la rangée (une enveloppe en haut à gauche passe parfois à la
            // rangée d'après, quand elle ne tient plus à la suite de la case d'avant).
            let ligne = l.cards.iter().filter(|n| n.row == c.row && n.id != c.id).map(|n| n.thumb.top()).next().expect("une autre case dans la rangée");
            // Le cadre le plus haut est à la ligne des images de la rangée, comme les autres cases : rien ne descend pour lui.
            let haut = boite_des_cadres(&l).top();
            assert!(proche(haut, ligne), "{etiquette} : le cadre le plus haut est à la ligne {ligne} : {haut}");
            assert!(proche(c.ligne.top(), ligne) && proche(m.tuile.rect.top(), ligne), "{etiquette} : l'empreinte commence à la ligne");
            assert!(c.thumb.top() > ligne + 1.0, "{etiquette} : A est plus bas que la ligne : {} {ligne}", c.thumb.top());
            // A est à sa place dans l'enveloppe : plus bas que la ligne de ce dont B est plus haut qu'elle.
            let b = m.cadres.last().unwrap().rect;
            assert!(proche(c.thumb.top() - ligne, c.thumb.top() - b.top()), "{etiquette} : A est plus bas que la ligne de l'écart de B à A");
            // Rien ne glisse, les autres cases de la rangée restent à la ligne, celles d'avant ne bougent pas.
            assert_eq!(l.decalage.y, 0.0, "{etiquette} : rien ne glisse vers le bas");
            for (j, n) in l.cards.iter().enumerate().filter(|(j, _)| *j != K) {
                if n.row == c.row {
                    assert!(proche(n.thumb.top(), ligne), "{etiquette} : la case {j} de la rangée reste à la ligne");
                }
                if j < K {
                    assert_eq!(n.thumb, ordinaire.cards[j].thumb, "{etiquette} : la case {j}, d'avant, ne bouge pas");
                }
            }
            // La rangée est aussi haute qu'il faut pour A et sa note : la rangée suivante est plus bas que tout.
            assert!(!se_recouvrent(&coins_de(c.note).to_vec(), &m.enveloppe, 0.5), "{etiquette} : la note ne recouvre pas l'enveloppe");
            let bas = c.note.bottom().max(c.foot.bottom()).max(c.thumb.bottom());
            for n in l.cards.iter().filter(|n| n.row > c.row) {
                assert!(n.card.top() >= bas + ROW_GAP - 1e-2, "{etiquette} : la case {} de la rangée suivante remonte à {} (A et sa note vont jusqu'à {bas})", n.id, n.card.top());
            }
            // Les barres de coupe de la rangée sont toutes à la ligne, celle qui suit le mouvement aussi.
            for b in l.bars.iter().filter(|b| l.cards[b.after].row == c.row) {
                assert!(proche(b.zone.top(), ligne), "{etiquette} : la barre après {} est à la ligne : {:?}", b.after, b.zone);
            }
            sans_chevauchement(&l);
            assert_eq!(l.cards.len(), ordinaire.cards.len(), "{etiquette} : aucune vignette perdue");
        }
    }
}

#[test]
fn a_downward_move_keeps_a_on_the_line_as_before() {
    let ordinaire = en_lignes(&plan_court(), 4);
    for (dx, dy) in [(0.0, 90.0), (160.0, 55.0), (80.0, 120.0)] {
        let l = en_lignes(&film_de_deux_cadres(dx, dy), 4);
        assert_eq!(l.cards[K].thumb, ordinaire.cards[K].thumb, "({dx}, {dy}) : A reste sur la ligne");
        assert_eq!(l.cards[K].ligne.top(), l.cards[K].thumb.top());
    }
}

#[test]
fn a_move_held_during_a_gesture_keeps_a_where_it_was_and_the_release_lowers_it() {
    let ordinaire = en_lignes(&plan_court(), 4);
    let reserve = board::Reserve::depuis(&ordinaire, K).unwrap();
    assert_eq!(reserve.pose_dec, Some(0.0), "A était sur la ligne à l'appui");
    for (dx, dy) in [(0.0, -90.0), (-80.0, -60.0), (0.0, -200.0)] {
        let doc = film_de_deux_cadres(dx, dy);
        let retenu = board::layout_with(&doc, &reglages(4), &HashMap::new(), false, &Options { hold: Some(reserve), ..Default::default() });
        // (Si l'enveloppe monte au-dessus de la page, tout glisse d'autant et la vue compense : A ne bouge pas à l'écran.)
        assert!((retenu.cards[K].thumb.translate(-retenu.decalage).min - ordinaire.cards[K].thumb.min).length() < 1e-2, "({dx}, {dy}) : A ne bouge pas pendant le geste");
        // Au lâcher : tout se range d'après les cadres, A est plus bas que la ligne.
        let libre = en_lignes(&doc, 4);
        assert!(libre.cards[K].thumb.top() > libre.cards[K].ligne.top() + 1.0, "({dx}, {dy}) : au lâcher, A est en retrait");
        let apres = board::Reserve::depuis(&libre, K).unwrap();
        assert!(apres.pose_dec.unwrap() > 1.0, "({dx}, {dy}) : le retrait est celui de la case");
    }
}

#[test]
fn the_same_moves_mirrored_in_the_japanese_reading_align_the_leftmost_frame_on_the_column() {
    let ordinaire = a_la_japonaise(&plan_court(), 4);
    // B à gauche de A : à cheval (-80), juste à gauche (-160), en haut à gauche (-80, -60), en bas à gauche (-80, 60), très à gauche (-400).
    for (dx, dy) in [(-80.0, 0.0), (-160.0, 0.0), (-80.0, -60.0), (-80.0, 60.0), (-400.0, 0.0)] {
        for colonnes in [4, 6] {
            let doc = film_de_deux_cadres(dx, dy);
            let l = a_la_japonaise(&doc, colonnes);
            let etiquette = format!("({dx}, {dy}), {colonnes} par colonne");
            let (c, m) = (l.cards[K], l.move_of(K).expect("une empreinte"));
            // Le bord de sa colonne : celui des autres cases de la colonne (une enveloppe qui ne tient plus passe en haut de la suivante).
            let colonne = l.cards.iter().filter(|n| n.row == c.row && n.id != c.id).map(|n| n.thumb.left()).next().expect("une autre case dans la colonne");
            let gauche = boite_des_cadres(&l).left();
            assert!(proche(gauche, colonne), "{etiquette} : le cadre le plus à gauche est au bord de la colonne {colonne} : {gauche}");
            assert!(proche(c.ligne.left(), colonne) && proche(m.tuile.rect.left(), colonne), "{etiquette} : l'empreinte commence au bord de la colonne");
            assert!(c.thumb.left() > colonne + 1.0, "{etiquette} : A est plus à droite que le bord : {} {colonne}", c.thumb.left());
            assert_eq!(l.decalage.x, 0.0, "{etiquette} : rien ne glisse vers la droite");
            // Les autres cases de la colonne restent au bord, celles d'avant ne bougent pas.
            for (j, n) in l.cards.iter().enumerate().filter(|(j, _)| *j != K) {
                if n.row == c.row {
                    assert!(proche(n.thumb.left(), colonne), "{etiquette} : la case {j} de la colonne reste au bord");
                }
                if j < K {
                    assert_eq!(n.thumb, ordinaire.cards[j].thumb, "{etiquette} : la case {j}, d'avant, ne bouge pas");
                }
            }
            // La colonne suivante est entièrement à droite de tout, A et son texte compris.
            let droite = c.note.right().max(c.foot.right());
            for n in l.cards.iter().filter(|n| n.row > c.row) {
                assert!(n.card.left() >= droite + board::gap_cut(170.0) - 1e-2, "{etiquette} : la case {} de la colonne suivante commence à {} (le mouvement va jusqu'à {droite})", n.id, n.card.left());
            }
            // Les barres de la colonne sont à son bord, comme celles des cases ordinaires.
            for b in l.bars.iter().filter(|b| l.cards[b.after].row == c.row) {
                assert!(proche(b.zone.left(), colonne), "{etiquette} : la barre après {} est au bord de la colonne : {:?}", b.after, b.zone);
            }
            sans_chevauchement(&l);
            assert_eq!(l.cards.len(), ordinaire.cards.len());
        }
    }
}
