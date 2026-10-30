//! Essais de la planche par séquences (`planchesequences.rs`, `planchesequencesui.rs`, 2026-10-30).
//!
//! Ce qu'ils gardent :
//! - un projet d'une seule séquence a la planche d'avant, case pour case (ni titre ni décalage) ;
//! - chaque séquence commence sur une nouvelle rangée avec son titre ; le sens de lecture, les cases par ligne, les cases pâles suivent ;
//! - l'ordre d'affichage ne change que la planche ; masquer ne perd rien ; une séquence ajoutée se montre à sa place du film ;
//! - créer, couper, renommer une séquence : une étape d'annulation chacune, aucun plan perdu ;
//! - l'affichage est enregistré dans `annotations.json`, un projet d'avant s'ouvre comme avant, un fichier abîmé ne gêne pas ;
//! - dessiner dans une case d'une séquence déplacée va au bon plan ; une case pâle fait un plan dans sa séquence ;
//! - l'export de la planche suit l'affichage.

use std::collections::BTreeSet;

use eframe::egui::{vec2, Vec2};

use crate::board::{self, BoardSettings, Options};
use crate::boardsens::Sens;
use crate::document::Document;
use crate::planchesequences::{self, liste_de, Affichage};
use crate::vignettes_tests::{case_vide, encre, glisser, ouvert, vague, vignette};

/// Cinq plans en trois séquences : A (plans 0 et 1), B (plans 2 et 3), C (plan 4), nommées « Prologue », « Course » et « Fin ».
fn trois_sequences() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    let b0 = doc.add_sequence_after(1);
    doc.add_scene_after(b0);
    doc.add_sequence_after(3);
    let seqs = doc.sequence_order();
    assert_eq!(seqs.len(), 3);
    for (q, nom) in seqs.iter().zip(["Prologue", "Course", "Fin"]) {
        doc.sequence_mut(*q).expect("séquence").name = nom.into();
    }
    assert_eq!(doc.panels.len(), 5);
    doc
}

fn reglages(colonnes: usize) -> BoardSettings {
    BoardSettings { card_w: 170.0, columns: colonnes, rows: 0 }
}

fn vue_de(doc: &Document) -> board::VueSequences {
    planchesequences::vue(doc).expect("plusieurs séquences")
}

fn planche(doc: &Document, colonnes: usize, opt: &Options) -> board::BoardLayout {
    planchesequences::layout_sequences(doc, &reglages(colonnes), false, opt, &vue_de(doc))
}

fn plans_de(l: &board::BoardLayout) -> Vec<usize> {
    l.cards.iter().map(|c| c.panel).collect()
}

fn photo(doc: &Document) -> String {
    crate::fiabilite_fixture::instantane(doc, &BTreeSet::new())
}

// ------------------------------------------------------------------------------ un projet d'une seule séquence

#[test]
fn a_project_with_one_sequence_has_the_old_board_without_title() {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    assert_eq!(doc.sequences.len(), 1);
    assert!(planchesequences::vue(&doc).is_none(), "une séquence : pas de planche par séquences");
    let mut cache = board::CacheDisposition::default();
    let b = board::layout_cache_sequences(&doc, &reglages(6), &Default::default(), false, &Options::pale(3), None, &mut cache);
    assert!(b.groupes.is_empty(), "ni titre ni groupe");
    assert_eq!(b, board::layout_with(&doc, &reglages(6), &Default::default(), false, &Options::pale(3)));
    assert!(b.size.y > 0.0 && b.decalage == Vec2::ZERO);
}

// ------------------------------------------------------------------------------ la mise en page

#[test]
fn each_sequence_starts_on_a_new_row_with_its_title_above() {
    let doc = trois_sequences();
    let l = planche(&doc, 6, &Options::pale(3));
    assert_eq!(l.groupes.len(), 3);
    assert_eq!(plans_de(&l), vec![0, 1, 2, 3, 4], "l'ordre du film");
    let mut bas_precedent = f32::MIN;
    for g in &l.groupes {
        let t = g.titre.expect("un titre par séquence");
        let haut = l.cards[g.cartes.start].card.top();
        assert!(t.bottom() <= haut + 0.1, "le titre est au-dessus de la première rangée");
        assert!(t.top() > bas_precedent, "la séquence suit celle d'avant, sans la recouvrir");
        // Toutes les cases de la séquence sont sous son titre ; les cases pâles aussi.
        assert!(l.cards[g.cartes.clone()].iter().all(|c| c.card.top() >= t.bottom() - 0.1));
        assert!(l.empty[g.vides.clone()].iter().all(|e| e.top() >= t.bottom()));
        bas_precedent = l.cards[g.cartes.clone()].iter().map(|c| c.reach_y).chain(l.empty[g.vides.clone()].iter().map(|e| e.bottom())).fold(f32::MIN, f32::max);
    }
    // Les cases pâles sont à la fin de chaque séquence : une par rangée au moins.
    assert!(l.groupes.iter().all(|g| !g.vides.is_empty()), "des cases pâles à la fin de chaque séquence");
    // Les rangées se comptent d'un bout à l'autre de la planche : jamais deux séquences sur la même rangée.
    let rangs: Vec<usize> = l.groupes.iter().map(|g| l.cards[g.cartes.start].row).collect();
    assert!(rangs.windows(2).all(|w| w[0] < w[1]), "{rangs:?}");
}

#[test]
fn the_reading_direction_and_the_columns_apply_inside_each_sequence() {
    let doc = trois_sequences();
    // Deux cases par ligne : la séquence B (deux plans) tient sur une rangée, la A aussi ; avec une case par ligne, deux rangées chacune.
    let une = planche(&doc, 1, &Options::pale(3));
    let (a, b) = (&une.groupes[0], &une.groupes[1]);
    let tops: Vec<f32> = une.cards[a.cartes.clone()].iter().map(|c| c.thumb.top()).collect();
    assert!(tops[1] > tops[0] + 10.0, "une case par ligne : la deuxième case de A est sur une autre rangée");
    assert!(une.cards[b.cartes.start].thumb.top() > une.cards[a.cartes.end - 1].thumb.top(), "B commence sous A");
    let deux = planche(&doc, 6, &Options::pale(3));
    let tops: Vec<f32> = deux.cards[deux.groupes[0].cartes.clone()].iter().map(|c| c.thumb.top()).collect();
    assert!((tops[1] - tops[0]).abs() < 0.1, "six cases par ligne : les deux plans de A sont côte à côte");
    // À la japonaise : les colonnes, dans chaque séquence ; la séquence suivante est dessous.
    let jap = planche(&doc, 6, &Options { sens: Sens::Japonais, ..Options::pale(3) });
    let a = &jap.groupes[0];
    let (c0, c1) = (jap.cards[a.cartes.start], jap.cards[a.cartes.start + 1]);
    assert!((c0.thumb.left() - c1.thumb.left()).abs() < 0.1 && c1.thumb.top() > c0.thumb.top(), "à la japonaise, de haut en bas");
    let suivante = jap.cards[jap.groupes[1].cartes.start];
    assert!(suivante.thumb.top() > c1.thumb.bottom(), "la séquence suivante est sous la première");
    assert_eq!(plans_de(&jap), vec![0, 1, 2, 3, 4]);
}

#[test]
fn the_pale_cases_are_at_the_end_of_each_sequence_and_the_bars_do_not_cross_a_title() {
    let doc = trois_sequences();
    let l = planche(&doc, 6, &Options::pale(3));
    for g in &l.groupes {
        let derniere = l.cards[g.cartes.end - 1];
        let premiere_vide = l.empty[g.vides.start];
        assert!(premiere_vide.top() >= derniere.thumb.top() - 0.1, "la case pâle suit la dernière case de sa séquence");
        // La barre de la dernière case de la séquence mène à sa case pâle.
        assert!(l.vers_pale(g.cartes.end - 1));
    }
    // Il n'y a pas d'autre barre « vers le vide » : les cases d'une séquence qui se suivent ont une vraie barre.
    assert!(!l.vers_pale(0) && !l.vers_pale(2));
    // La case pâle `e` désigne le dernier plan de sa séquence et son rang dans celle-ci.
    let (dernier_a, e0) = l.vide_de(l.groupes[0].vides.start, doc.panels.len());
    assert_eq!((dernier_a, e0), (1, 0));
    // Une seule case pâle à la fin de chaque séquence sauf la dernière ; la dernière garde les rangées pâles du réglage.
    assert_eq!(l.groupes[1].vides.len(), 1);
    let (dernier_b, e1) = l.vide_de(l.groupes[1].vides.start, doc.panels.len());
    assert_eq!((dernier_b, e1), (3, 0));
    assert!(l.groupes[2].vides.len() > 1);
    let (dernier_c, e2) = l.vide_de(l.groupes[2].vides.start + 1, doc.panels.len());
    assert_eq!((dernier_c, e2), (4, 1));
    // Sans case pâle (les exports) : rien.
    let sans = planche(&doc, 6, &Options::default());
    assert!(sans.empty.is_empty());
}

#[test]
fn cards_keep_their_identity_whatever_the_order() {
    let doc = trois_sequences();
    let film = planche(&doc, 6, &Options::pale(3));
    let mut o = trois_sequences();
    let seqs = o.sequence_order();
    o.annotations.affichage.ordre = vec![(seqs[2], true), (seqs[0], true), (seqs[1], true)];
    let autre = planche(&o, 6, &Options::pale(3));
    assert_eq!(plans_de(&autre), vec![4, 0, 1, 2, 3], "C d'abord, puis A, puis B");
    let mut a: Vec<u64> = film.cards.iter().map(|c| c.id).collect();
    let mut b: Vec<u64> = autre.cards.iter().map(|c| c.id).collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b, "les mêmes cases, rangées autrement");
    // Les plans gardent leur rang dans le film : le film n'a pas bougé.
    assert_eq!(o.panels.iter().map(|p| p.id).collect::<Vec<_>>(), doc.panels.iter().map(|p| p.id).collect::<Vec<_>>());
    assert_eq!(o.sequence_order(), seqs);
}

// ------------------------------------------------------------------------------ ce qu'on affiche

#[test]
fn the_list_is_all_in_film_order_by_default_and_a_new_sequence_shows_at_its_place() {
    assert_eq!(liste_de(&[], &[7, 8, 9]), vec![(7, true), (8, true), (9, true)]);
    // Un ordre choisi, une séquence cachée, une séquence oubliée (qui n'existe plus) et une séquence neuve.
    let choix = [(9, true), (99, true), (7, false)];
    let l = liste_de(&choix, &[7, 8, 9]);
    // 8 est neuve : elle vient juste après 7 (sa précédente dans le film), 99 n'existe plus.
    assert_eq!(l, vec![(9, true), (7, false), (8, true)]);
    // Une séquence neuve en tête du film se met en tête ; une à la fin, après la dernière du film.
    assert_eq!(liste_de(&[(8, true), (9, false)], &[7, 8, 9])[0], (7, true));
    assert_eq!(liste_de(&[(7, true), (8, true)], &[7, 8, 9]).last(), Some(&(9, true)));
}

#[test]
fn the_board_always_shows_at_least_one_sequence() {
    let doc = trois_sequences();
    let s = doc.sequence_order();
    let mut d = trois_sequences();
    d.annotations.affichage = Affichage { ordre: s.iter().map(|&q| (q, false)).collect() };
    assert_eq!(d.annotations.affichage.montrees(&d), s, "tout caché : tout se montre");
}

#[test]
fn hiding_a_sequence_hides_its_cards_and_loses_nothing() {
    let mut b = ouvert(trois_sequences(), 6);
    let seqs = b.app.doc.sequence_order();
    let avant = photo(&b.app.doc);
    b.app.affichage_basculer(seqs[1]);
    b.chauffer(2);
    let l = b.app.board_layout();
    assert_eq!(plans_de(&l), vec![0, 1, 4], "B est cachée");
    assert_eq!(l.groupes.len(), 2);
    assert!(l.masquees.len() >= 2, "les vignettes de B sont notées masquées : {:?}", l.masquees);
    assert_eq!(b.app.doc.panels.len(), 5);
    let _ = avant;
    // Annuler ne rend rien et ne défait pas l'affichage : c'est un affichage, pas une étape.
    assert_eq!(b.app.history.undo_len(), 0);
    // La dernière séquence montrée ne se cache pas.
    b.app.affichage_basculer(seqs[0]);
    b.app.affichage_basculer(seqs[2]);
    assert_eq!(b.app.board_layout().groupes.len(), 1, "la dernière séquence montrée ne se cache pas");
}

#[test]
fn the_three_shortcuts() {
    let mut b = ouvert(trois_sequences(), 6);
    let seqs = b.app.doc.sequence_order();
    b.app.cur_panel = 2; // B
    b.app.run_action("sequence.afficher_aucune_autre");
    assert_eq!(plans_de(&b.app.board_layout()), vec![2, 3]);
    b.app.run_action("sequence.afficher_toutes");
    assert_eq!(plans_de(&b.app.board_layout()), vec![0, 1, 2, 3, 4]);
    b.app.cur_panel = 4;
    b.app.run_action("sequence.afficher_courante");
    assert_eq!(plans_de(&b.app.board_layout()), vec![4]);
    // Tout remettre : l'affichage retombe à « rien d'enregistré ».
    b.app.run_action("sequence.afficher_toutes");
    b.app.run_action("sequence.afficher_ordre_film");
    assert!(b.app.doc.annotations.affichage.est_par_defaut(), "toutes, dans l'ordre du film : rien à enregistrer ({:?})", b.app.doc.annotations.affichage);
    let _ = seqs;
}

#[test]
fn reordering_the_display_changes_only_the_board() {
    let mut b = ouvert(trois_sequences(), 6);
    let seqs = b.app.doc.sequence_order();
    let film: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.affichage_deplacer(seqs[2], 0);
    b.chauffer(2);
    assert_eq!(plans_de(&b.app.board_layout()), vec![4, 0, 1, 2, 3]);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), film, "le montage garde l'ordre du film");
    assert_eq!(b.app.doc.sequence_order(), seqs);
    // Une séquence ajoutée plus tard se montre, à sa place du film.
    b.app.sequence_nouvelle_fin();
    b.chauffer(2);
    let nouvelle = *b.app.doc.sequence_order().last().unwrap();
    let montrees = b.app.doc.annotations.affichage.montrees(&b.app.doc);
    assert_eq!(montrees, vec![seqs[2], nouvelle, seqs[0], seqs[1]], "juste après C, sa précédente dans le film : {montrees:?}");
}

// ------------------------------------------------------------------------------ créer, couper, renommer

#[test]
fn a_new_sequence_is_added_at_the_end_with_one_empty_plan_and_one_undo_step() {
    let mut b = ouvert(trois_sequences(), 6);
    let avant = photo(&b.app.doc);
    b.app.run_action("sequence.nouvelle_fin");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 6);
    assert_eq!(b.app.doc.sequences.len(), 4);
    assert_eq!(b.app.doc.sequence_of(5), *b.app.doc.sequence_order().last().unwrap());
    assert!(!encre(&b.app.doc, 5), "un plan vide");
    assert_eq!(b.app.history.undo_len(), 1);
    assert_eq!(b.app.board_layout().groupes.len(), 4, "la séquence se montre");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(photo(&b.app.doc), avant, "annuler rend le projet exactement");
}

#[test]
fn cutting_a_sequence_here_makes_a_new_one_and_the_film_does_not_move() {
    let mut b = ouvert(trois_sequences(), 6);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let avant = photo(&b.app.doc);
    // Couper B au plan 3 : B = plan 2, nouvelle = plans 3.
    b.app.cur_panel = 3;
    b.app.run_action("sequence.couper_ici");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids, "même ordre de plans");
    let s = b.app.doc.sequence_ids();
    assert_ne!(s[2], s[3]);
    assert_eq!(b.app.doc.sequences.len(), 4);
    assert_eq!(b.app.doc.sequence_label(s[2]), "Course", "la première partie garde son nom");
    assert_eq!(b.app.history.undo_len(), 1);
    assert_eq!(b.app.board_layout().groupes.len(), 4);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(photo(&b.app.doc), avant);
    // Au début d'une séquence : rien à couper, rien d'enregistré.
    b.app.cur_panel = 2;
    b.app.run_action("sequence.couper_ici");
    assert_eq!(b.app.history.undo_len(), 0, "{}", b.app.status);
    assert_eq!(b.app.doc.sequences.len(), 3);
}

#[test]
fn a_new_sequence_from_the_chosen_cases_works_with_one_case() {
    let mut b = ouvert(trois_sequences(), 6);
    b.app.cur_panel = 1;
    b.app.board_sel = vec![b.app.doc.panels[1].id];
    b.app.run_action("sequence.nouvelle_avec_cases");
    b.chauffer(2);
    assert_eq!(b.app.doc.sequences.len(), 4, "{}", b.app.status);
    let s = b.app.doc.sequence_ids();
    assert_ne!(s[0], s[1]);
    assert_eq!(b.app.history.undo_len(), 1);
    // Deux cases : la commande des plans choisis sert aussi.
    b.app.cur_panel = 2;
    b.app.board_sel = vec![b.app.doc.panels[2].id, b.app.doc.panels[3].id];
    b.app.run_action("sequence.nouvelle_avec_cases");
    assert!(b.app.doc.panels.len() == 5);
}

#[test]
fn renaming_a_sequence_is_one_undo_step_and_the_title_shows_the_name() {
    let mut b = ouvert(trois_sequences(), 6);
    let q = b.app.doc.sequence_order()[1];
    b.app.sequence_renommer(q, "  Poursuite  ");
    assert_eq!(b.app.doc.sequence_label(q), "Poursuite");
    assert_eq!(b.app.history.undo_len(), 1);
    b.app.sequence_renommer(q, "Poursuite");
    assert_eq!(b.app.history.undo_len(), 1, "le même nom : rien de plus");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.sequence_label(q), "Course");
    // Un nom vide revient au numéro automatique ; un nom trop long est coupé.
    b.app.sequence_renommer(q, "");
    assert_ne!(b.app.doc.sequence_label(q), "Course");
    b.app.sequence_renommer(q, &"x".repeat(200));
    assert_eq!(b.app.doc.sequence_label(q).chars().count(), 60);
}

// ------------------------------------------------------------------------------ ce qui suit la mise en page

#[test]
fn drawing_in_a_case_of_a_reordered_sequence_goes_to_the_right_plan() {
    let mut b = ouvert(trois_sequences(), 6);
    let seqs = b.app.doc.sequence_order();
    b.app.affichage_deplacer(seqs[2], 0);
    b.app.board_zoom = 0.45;
    b.app.board_pan = Vec2::ZERO;
    b.chauffer(3);
    // La case du plan 4 (C) est maintenant la première de la planche.
    let l = b.app.board_layout();
    assert_eq!(l.cards[0].panel, 4);
    let v = vignette(&b, 0);
    glisser(&mut b, &vague(v.center() - vec2(20.0, 0.0), v.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.cur_panel, 4, "le plan visé est courant");
    assert!(encre(&b.app.doc, 4), "le trait est dans le plan 4");
    assert!((0..4).all(|i| !encre(&b.app.doc, i)), "aucun autre plan n'a de trait");
    // Dessiner dans la case du plan 2 (B, devenue la quatrième).
    let k = b.app.board_layout().cards.iter().position(|c| c.panel == 2).unwrap();
    // (La vue est petite : on fait défiler la planche jusqu'à cette case.)
    let haut = b.app.board_layout().cards[k].thumb.top() * b.app.board_zoom;
    b.app.board_pan = Vec2::new(0.0, 40.0 - haut);
    b.chauffer(3);
    let v = vignette(&b, k);
    glisser(&mut b, &vague(v.center() - vec2(20.0, 0.0), v.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.cur_panel, 2);
    assert!(encre(&b.app.doc, 2));
    assert!(!encre(&b.app.doc, 3));
}

#[test]
fn drawing_in_a_pale_case_makes_a_plan_in_its_own_sequence() {
    let mut b = ouvert(trois_sequences(), 6);
    b.app.board_zoom = 0.45;
    b.app.board_pan = Vec2::ZERO;
    b.chauffer(3);
    let l = b.app.board_layout();
    // La première case pâle de la séquence A (la première séquence, qui n'est pas la dernière du film).
    let e = l.groupes[0].vides.start;
    let r = case_vide(&b, e);
    glisser(&mut b, &vague(r.center() - vec2(20.0, 0.0), r.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.doc.panels.len(), 6, "un plan de plus");
    let s = b.app.doc.sequence_ids();
    assert_eq!(s[..3], [s[0]; 3], "le plan neuf est le troisième de la séquence A, juste après son dernier plan");
    assert_eq!(b.app.cur_panel, 2);
    assert!(encre(&b.app.doc, 2));
    assert_eq!(b.app.doc.sequences.len(), 3, "aucune séquence de plus");
}

#[test]
fn the_continuity_bar_of_a_sequence_continues_the_last_plan_of_that_sequence() {
    let mut b = ouvert(trois_sequences(), 6);
    b.app.board_zoom = 0.45;
    b.app.board_pan = Vec2::ZERO;
    b.chauffer(3);
    let l = b.app.board_layout();
    let apres = l.groupes[0].cartes.end - 1;
    let layout = l.clone();
    assert!(b.app.board_toggle_cut(&layout, apres), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 5, "une image de plus, pas un plan de plus");
    assert!(b.app.doc.panels[1].marks.len() == 1, "le dernier plan de A a une image de plus");
    assert_eq!(b.app.doc.panels[4].marks.len(), 0);
    assert_eq!(b.app.history.undo_len(), 1);
}

#[test]
fn an_annotation_follows_its_case_when_the_sequences_are_reordered_and_waits_when_hidden() {
    let mut b = ouvert(trois_sequences(), 6);
    let seqs = b.app.doc.sequence_order();
    let l = b.app.board_layout();
    let k = l.cards.iter().position(|c| c.panel == 2).unwrap();
    let c = l.cards[k].thumb;
    let trait_ = vec![crate::vector::VPoint { x: c.center().x - 20.0, y: c.center().y, r: 2.0 }, crate::vector::VPoint { x: c.center().x + 20.0, y: c.center().y, r: 2.0 }];
    assert!(b.app.doc.annotations.ajouter(&l, &trait_, [220, 38, 38]));
    let centre_avant = |b: &crate::perf_tests::Banc| -> (f32, f32) {
        let l = b.app.board_layout();
        let p = b.app.doc.annotations.points_planche(&l, 0).expect("le trait est sur la planche");
        let k = l.cards.iter().position(|c| c.panel == 2).unwrap();
        (p[0].x - l.cards[k].thumb.left(), p[0].y - l.cards[k].thumb.top())
    };
    let relatif = centre_avant(&b);
    b.app.affichage_deplacer(seqs[2], 0);
    b.chauffer(2);
    let apres = centre_avant(&b);
    assert!((relatif.0 - apres.0).abs() < 0.01 && (relatif.1 - apres.1).abs() < 0.01, "le trait reste à sa place dans sa case : {relatif:?} {apres:?}");
    // Masquer sa séquence : le trait n'est pas peint, mais il n'est ni perdu ni passé à une voisine.
    b.app.affichage_basculer(seqs[1]);
    b.chauffer(3);
    assert_eq!(b.app.doc.annotations.traces.len(), 1);
    let ancre = b.app.doc.annotations.traces[0].anchor;
    assert_eq!(ancre.card, b.app.doc.panels[2].id, "toujours attaché à sa case");
    assert!(b.app.doc.annotations.points_planche(&b.app.board_layout(), 0).is_none(), "pas peint tant que sa séquence est cachée");
    b.app.affichage_basculer(seqs[1]);
    b.chauffer(3);
    let revenu = centre_avant(&b);
    assert!((relatif.0 - revenu.0).abs() < 0.01 && (relatif.1 - revenu.1).abs() < 0.01, "il revient où il était");
}

#[test]
fn the_opening_view_frames_the_sequence_of_the_current_plan() {
    let mut b = ouvert(trois_sequences(), 6);
    b.app.cur_panel = 4;
    let l = b.app.board_layout();
    let view = b.app.vignettes.view;
    let k = l.cards.iter().position(|c| c.panel == 4).unwrap();
    let (zoom, decalage) = crate::vignettesprefs::ouverture_cadre(&l, view, 6, 2, k, 0.2, 6.0, 1);
    // Cadrée sur la séquence C : son titre et sa première rangée sont dans la vue, les autres séquences non.
    let g = l.groupe_de(k).expect("séquence").clone();
    let titre = g.titre.expect("titre");
    let ecran = |r: eframe::egui::Rect| eframe::egui::Rect::from_min_max(view.min + decalage + r.min.to_vec2() * zoom, view.min + decalage + r.max.to_vec2() * zoom);
    assert!(view.contains_rect(ecran(titre)), "le titre de la séquence est dans la vue");
    assert!(view.contains_rect(ecran(l.cards[k].thumb)), "sa première case aussi");
    let a = l.cards[0].thumb;
    assert!(!view.intersects(ecran(a)), "la séquence A, dessus, n'est pas dans la vue : {:?} {:?}", ecran(a), view);
}

// ------------------------------------------------------------------------------ le fichier

#[test]
fn the_display_is_saved_in_the_project_and_comes_back() {
    let mut doc = trois_sequences();
    let s = doc.sequence_order();
    doc.annotations.affichage.ordre = vec![(s[2], true), (s[0], false), (s[1], true)];
    let dir = std::env::temp_dir().join(format!("shot-test-affichage-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).expect("enregistrement");
    let texte = std::fs::read_to_string(dir.join("annotations.json")).expect("annotations.json");
    assert!(texte.contains("\"sequences\""), "{texte}");
    let rouvert = crate::project::load(&manifest).expect("réouverture");
    assert_eq!(rouvert.doc.annotations.affichage, doc.annotations.affichage);
    assert_eq!(planchesequences::vue(&rouvert.doc).unwrap().sequences, vec![s[2], s[1]], "A cachée, C d'abord");
    assert!(rouvert.warnings.iter().all(|w| !w.contains("annotations")), "{:?}", rouvert.warnings);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_old_project_without_the_key_opens_with_all_the_sequences_in_film_order() {
    let doc = trois_sequences();
    let dir = std::env::temp_dir().join(format!("shot-test-affichage-ancien-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).expect("enregistrement");
    assert!(!dir.join("annotations.json").exists(), "un affichage par défaut n'écrit aucun fichier");
    // Un annotations.json d'avant (version 2, sans la clé) se lit tel quel.
    std::fs::write(dir.join("annotations.json"), r#"{"format":"shot-annotations","version":2,"calques":[{"id":1,"nom":"Annotations","visible":true,"traces":[]}]}"#).unwrap();
    let rouvert = crate::project::load(&manifest).expect("réouverture");
    assert!(rouvert.doc.annotations.affichage.est_par_defaut());
    assert_eq!(planchesequences::vue(&rouvert.doc).unwrap().sequences, rouvert.doc.sequence_order());
    // Une clé abîmée (identifiant à zéro, doublon, séquence qui n'existe pas) est écartée sans rien casser.
    std::fs::write(dir.join("annotations.json"), r#"{"format":"shot-annotations","version":2,"sequences":[{"id":0},{"id":123456,"montree":false},{"id":123456},{"id":1,"montree":false}]}"#).unwrap();
    let abime = crate::project::load(&manifest).expect("réouverture");
    assert!(abime.warnings.iter().all(|w| !w.contains("annotations")), "{:?}", abime.warnings);
    let montrees = planchesequences::vue(&abime.doc).unwrap().sequences;
    assert!(!montrees.is_empty() && montrees.len() <= 3);
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------ robustesse

#[test]
fn deleting_merging_reordering_and_undoing_sequences_keep_the_board_coherent() {
    let mut b = ouvert(trois_sequences(), 6);
    let s = b.app.doc.sequence_order();
    b.app.affichage_deplacer(s[2], 0);
    b.app.affichage_basculer(s[1]);
    for tour in 0..4 {
        // Supprimer tous les plans de la séquence B : elle disparaît, l'affichage l'oublie.
        match tour {
            0 => {
                b.app.cur_panel = 2;
                b.app.board_sel = vec![b.app.doc.panels[2].id, b.app.doc.panels[3].id];
                b.app.delete_chosen_panels();
            }
            1 => b.app.run_action("edition.annuler"),
            2 => {
                // Réunir deux séquences : tous les plans dans la même.
                b.app.cur_panel = 0;
                b.app.board_sel = b.app.doc.panels.iter().map(|p| p.id).collect();
                b.app.run_action("sequence.nouvelle_avec_plans");
            }
            _ => {
                b.app.run_action("edition.annuler");
                b.app.run_action("edition.retablir");
            }
        }
        b.chauffer(3);
        let l = b.app.board_layout();
        let montres: usize = l.cards.iter().map(|c| c.panel).collect::<BTreeSet<_>>().len();
        assert!(montres >= 1 && l.cards.iter().all(|c| c.panel < b.app.doc.panels.len()), "tour {tour} : {:?}", plans_de(&l));
        assert!(l.groupes.iter().all(|g| g.cartes.end <= l.cards.len() && g.vides.end <= l.empty.len()));
        assert_eq!(l.groupes.is_empty(), b.app.doc.sequence_order().len() <= 1, "tour {tour}");
    }
}

#[test]
fn a_sequence_variant_and_a_version_change_do_not_disturb_the_display() {
    let mut b = ouvert(trois_sequences(), 6);
    let s = b.app.doc.sequence_order();
    b.app.affichage_deplacer(s[1], 0);
    b.chauffer(2);
    let avant = b.app.doc.annotations.affichage.clone();
    // Une version du plan : la planche garde ses séquences et leur ordre.
    b.app.cur_panel = 0;
    b.app.run_action("version.nouvelle");
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.affichage, avant);
    assert_eq!(b.app.board_layout().groupes.iter().map(|g| g.sequence).collect::<Vec<_>>(), vec![s[1], s[0], s[2]]);
}

// ------------------------------------------------------------------------------ l'export

#[test]
fn the_export_follows_the_displayed_sequences_in_their_order_with_titles() {
    let mut doc = trois_sequences();
    let s = doc.sequence_order();
    doc.annotations.affichage.ordre = vec![(s[2], true), (s[0], false), (s[1], true)];
    let plans = crate::planchesequences::plans_affiches(&doc);
    assert_eq!(plans, vec![4, 2, 3], "C, puis B ; A est cachée");
    let opt = crate::planche::Options::default();
    let p = crate::planche::construire(&doc, &plans, &reglages(6), &opt, &doc.annotations);
    assert_eq!(p.cartes.iter().map(|c| c.plan_reel).collect::<Vec<_>>(), vec![4, 2, 3]);
    assert_eq!(p.layout.groupes.len(), 2, "un titre par séquence exportée");
    assert!(p.layout.groupes.iter().all(|g| g.titre.is_some()));
    assert_eq!(p.layout.groupes.iter().map(|g| g.sequence).collect::<Vec<_>>(), vec![s[2], s[1]]);
    assert!(p.cadre.height() > 0.0);
    // Sans plusieurs séquences : la planche d'avant (aucun titre).
    let seul = Document::with_frame(160, 90, 24);
    let p = crate::planche::construire(&seul, &[0], &reglages(6), &opt, &seul.annotations);
    assert!(p.layout.groupes.is_empty());
    let _ = doc.panels.len();
}

/// Prépare une copie d'un vrai projet pour une capture à l'écran (`SHOT_SEQ_COPIE` : le fichier du projet copié) : trois séquences par deux
/// coupures, des noms, la deuxième cachée, la troisième montrée en premier. Lancé à la main : `cargo test ... prepare_une_copie -- --ignored`.
#[test]
#[ignore]
fn prepare_une_copie_pour_une_capture() {
    let Ok(chemin) = std::env::var("SHOT_SEQ_COPIE") else { return };
    let manifest = std::path::PathBuf::from(chemin);
    let mut charge = crate::project::load(&manifest).expect("le projet s'ouvre");
    let doc = &mut charge.doc;
    // (Le projet d'essai n'a que quatre plans : des plans vides de plus, pour avoir de quoi couper.)
    while doc.panels.len() < 11 {
        let dernier = doc.panels.len() - 1;
        doc.add_scene_after(dernier);
    }
    let n = doc.panels.len();
    println!("plans : {n}, séquences : {}", doc.sequences.len());
    doc.couper_sequence_avant(n / 3).expect("première coupure");
    let i = (2 * n) / 3;
    doc.couper_sequence_avant(i).expect("deuxième coupure");
    let seqs = doc.sequence_order();
    assert_eq!(seqs.len(), 3);
    for (q, nom) in seqs.iter().zip(["Ouverture", "Poursuite", "Final"]) {
        doc.sequence_mut(*q).expect("séquence").name = nom.into();
    }
    doc.annotations.affichage.ordre = vec![(seqs[2], true), (seqs[0], true), (seqs[1], false)];
    crate::project::save(doc, &manifest, &mut charge.written).expect("enregistrement");
    println!("séquences : {:?}", seqs.iter().map(|q| doc.sequence_label(*q)).collect::<Vec<_>>());
}