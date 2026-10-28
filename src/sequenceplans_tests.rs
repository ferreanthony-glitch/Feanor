//! Essais de « Nouvelle séquence avec les plans choisis » (`sequenceplans.rs`).
//!
//! Ce qu'ils gardent :
//! - des plans qui se suivent forment une séquence neuve à leur place ; des plans qui ne se suivent pas se rejoignent à la place du premier ;
//! - les autres plans restent dans leur séquence (une séquence coupée en deux par la neuve devient deux séquences, la première gardant son nom) ;
//! - une seule étape d'annulation, qui rend le projet exactement ; aucun dessin perdu ;
//! - en numérotation verrouillée, les plans gardent leur numéro et la séquence neuve reçoit le sien ;
//! - des plans qui forment déjà une séquence entière : rien ne change.

use std::collections::BTreeSet;

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{instantane, meme_projet};
use crate::perf_tests::{banc_taille, Banc};

const LONGUEUR: usize = 12;

/// `n` plans de 12 images, chacun avec son dessin sur un calque « Dessin », dans une seule séquence nommée « Intro ».
fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for i in 0..n.saturating_sub(1) {
        doc.add_scene_after(i);
    }
    for i in 0..n {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(LONGUEUR);
        let c = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
        let mut l = Layer::new("Dessin", LayerKind::Bitmap);
        l.frames = vec![Some(c); LONGUEUR];
        doc.panels[i].layers.push(l);
    }
    doc.sequences[0].name = "Intro".into();
    doc
}

fn banc(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 800, 600);
    b.chauffer(2);
    b
}

fn ids(b: &Banc) -> Vec<u64> {
    b.app.doc.panels.iter().map(|p| p.id).collect()
}

fn seqs(b: &Banc) -> Vec<u64> {
    b.app.doc.sequence_ids()
}

fn photo(b: &Banc) -> String {
    instantane(&b.app.doc, &BTreeSet::new())
}

/// Choisit les plans de rangs `rangs` (dans le montage) et lance la commande.
fn choisir_et_lancer(b: &mut Banc, rangs: &[usize]) {
    let tous = ids(b);
    b.app.board_sel = rangs.iter().map(|&r| tous[r]).collect();
    b.app.cur_panel = rangs[0];
    b.app.run_action("sequence.nouvelle_avec_plans");
    b.chauffer(1);
}

#[test]
fn contiguous_plans_make_a_new_sequence_in_place() {
    let mut b = banc(film(5));
    let avant = ids(&b);
    choisir_et_lancer(&mut b, &[1, 2]);
    assert_eq!(ids(&b), avant, "l'ordre du film ne change pas : {}", b.app.status);
    let s = seqs(&b);
    assert_eq!(s[1], s[2], "les deux plans choisis sont dans la même séquence");
    assert_ne!(s[1], s[0]);
    assert_ne!(s[1], s[3]);
    // La séquence d'origine, coupée en deux : la première partie garde son identité et son nom, la suite a une séquence à elle, de même nom.
    assert_ne!(s[0], s[3], "{s:?}");
    assert_eq!(s[3], s[4]);
    assert_eq!(b.app.doc.sequences.len(), 3);
    assert_eq!(b.app.doc.sequence_label(s[0]), "Intro");
    assert_eq!(b.app.doc.sequence_label(s[3]), "Intro");
    // Une seule étape d'annulation nommée ; le plan courant est le premier des plans choisis.
    assert_eq!(b.app.history.undo_len(), 1);
    assert_eq!(b.app.history.names().0.last().map(|n| n.to_string()).as_deref(), Some("Nouvelle séquence avec les plans choisis"));
    assert_eq!(b.app.cur_panel, 1);
}

#[test]
fn plans_that_do_not_follow_each_other_gather_at_the_first_one() {
    let mut b = banc(film(5));
    let [p0, p1, p2, p3, p4]: [u64; 5] = ids(&b).try_into().unwrap();
    choisir_et_lancer(&mut b, &[1, 3]);
    assert_eq!(ids(&b), vec![p0, p1, p3, p2, p4], "ils se rejoignent à la place du premier choisi");
    let s = seqs(&b);
    assert_eq!(s[1], s[2]);
    assert_ne!(s[0], s[1]);
    // Les autres plans restent dans leur séquence d'origine : « Intro » avant, « Intro » (suite) après.
    assert_ne!(s[3], s[1]);
    assert_eq!(s[3], s[4]);
    assert_eq!(b.app.doc.sequence_label(s[0]), "Intro");
    assert_eq!(b.app.doc.sequence_label(s[3]), "Intro");

    // Le premier choisi tout au début : rien ne reste avant lui, la séquence d'origine n'est pas coupée.
    let mut b = banc(film(5));
    let [p0, p1, p2, p3, p4]: [u64; 5] = ids(&b).try_into().unwrap();
    choisir_et_lancer(&mut b, &[0, 3]);
    assert_eq!(ids(&b), vec![p0, p3, p1, p2, p4]);
    let s = seqs(&b);
    assert_eq!(s[0], s[1]);
    assert!(s[2] == s[3] && s[3] == s[4]);
    assert_ne!(s[1], s[2]);
    assert_eq!(b.app.doc.sequences.len(), 2, "la séquence d'origine reste une seule séquence");
}

#[test]
fn plans_from_two_sequences_leave_the_others_where_they_are() {
    let mut doc = film(4);
    let i = doc.add_sequence_after(3);
    doc.add_scene_after(i);
    let mut b = banc(doc);
    // Plans : A0 A1 A2 A3 | B0 B1. On choisit A3 et B0 (de part et d'autre de la limite).
    let avant = ids(&b);
    let (sa, sb) = (seqs(&b)[0], seqs(&b)[4]);
    choisir_et_lancer(&mut b, &[3, 4]);
    assert_eq!(ids(&b), avant);
    let s = seqs(&b);
    assert_eq!(s[3], s[4]);
    assert_eq!((s[0], s[1], s[2]), (sa, sa, sa), "les trois premiers restent dans leur séquence");
    assert_eq!(s[5], sb, "B1 reste dans la sienne");
    assert_ne!(s[3], sa);
    assert_ne!(s[3], sb);
}

#[test]
fn one_undo_step_restores_the_project_and_no_drawing_is_lost() {
    let mut b = banc(film(5));
    let (avant, dessins) = (photo(&b), b.app.doc.used_cels());
    choisir_et_lancer(&mut b, &[1, 3]);
    assert_ne!(photo(&b), avant);
    assert_eq!(b.app.history.undo_len(), 1);
    assert!(dessins.iter().all(|c| b.app.doc.used_cels().contains(c)), "aucun dessin perdu");
    b.app.run_action("edition.annuler");
    b.chauffer(1);
    meme_projet(&avant, &photo(&b)).expect("annuler rend le projet d'avant");
    b.app.run_action("edition.retablir");
    b.chauffer(1);
    assert_eq!(seqs(&b)[1], seqs(&b)[2], "rétablir refait la séquence");
}

#[test]
fn locked_numbering_keeps_the_plan_numbers_and_numbers_the_new_sequence() {
    let mut doc = film(5);
    doc.lock_numbering();
    let mut b = banc(doc);
    let codes_avant: Vec<String> = b.app.doc.panels.iter().map(|p| b.app.doc.plan_code(p.scene)).collect();
    let tous = ids(&b);
    choisir_et_lancer(&mut b, &[1, 2]);
    // Chaque plan garde son numéro (l'ordre n'a pas changé).
    let codes: Vec<String> = b.app.doc.panels.iter().map(|p| b.app.doc.plan_code(p.scene)).collect();
    assert_eq!(codes, codes_avant);
    assert_eq!(ids(&b), tous);
    // Toute séquence a un numéro propre, y compris la neuve et la suite de l'ancienne.
    assert!(b.app.doc.sequences.iter().all(|q| !q.number.is_empty()), "{:?}", b.app.doc.sequences);
    // Non contigus : les plans gardent leur numéro, ils se rejoignent (le numéro du plan ne dépend pas de sa place).
    let mut doc = film(5);
    doc.lock_numbering();
    let mut b = banc(doc);
    let numeros: Vec<(u64, String)> = b.app.doc.panels.iter().map(|p| (p.id, b.app.doc.plan_code(p.scene))).collect();
    choisir_et_lancer(&mut b, &[1, 3]);
    for (id, code) in numeros {
        let p = b.app.doc.panels.iter().find(|p| p.id == id).unwrap();
        assert_eq!(b.app.doc.plan_code(p.scene), code, "le plan garde son numéro");
    }
}

#[test]
fn an_entire_sequence_is_left_as_it_is_and_one_plan_is_refused_by_the_command() {
    let mut b = banc(film(3));
    let avant = photo(&b);
    choisir_et_lancer(&mut b, &[0, 1, 2]);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
    assert_eq!(b.app.history.undo_len(), 0);
    assert!(b.app.status.contains("séquence entière"), "{}", b.app.status);
    // Un seul plan choisi : la commande le dit.
    b.app.board_sel.clear();
    b.app.cur_panel = 1;
    b.app.run_action("sequence.nouvelle_avec_plans");
    assert!(b.app.status.contains("au moins deux plans"), "{}", b.app.status);
    assert_eq!(b.app.history.undo_len(), 0);
}

#[test]
fn the_storyboard_lock_refuses_it() {
    let mut doc = film(4);
    doc.storyboard_locked = true;
    let mut b = banc(doc);
    let avant = photo(&b);
    choisir_et_lancer(&mut b, &[1, 2]);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
    assert_eq!(b.app.history.undo_len(), 0);
}
