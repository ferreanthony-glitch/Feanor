//! Essais de la partie 106 : renommer une séquence (titre de la planche, clic droit, double-clic, fenêtre du montage et de la liste),
//! un titre trop long, et les lignes de texte qui se cochent quand un texte est saisi ou arrive avec un projet.

use eframe::egui::{Event, Modifiers, PointerButton};

use crate::boardjapon_tests::{banc_vignettes, clic};
use crate::boardsens::{Champ, Sens, Textes};
use crate::document::Document;

/// Cinq plans en trois séquences nommées « Prologue », « Course » et « Fin ».
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
    doc
}

fn titre_a_l_ecran(b: &crate::perf_tests::Banc, rang: usize) -> eframe::egui::Pos2 {
    let t = b.app.board_layout().groupes[rang].titre.expect("le titre est montré");
    b.app.vignettes.xf.rect(t).center()
}

#[test]
fn a_double_click_on_a_title_opens_it_and_one_undo_step_gives_the_old_name_back() {
    let mut b = banc_vignettes(trois_sequences(), Sens::Lignes, 4);
    let q = b.app.doc.sequence_order()[1];
    let p = titre_a_l_ecran(&b, 1);
    clic(&mut b, p);
    clic(&mut b, p);
    let e = b.app.vignettes.titre_edit.as_ref().expect("le double-clic ouvre le titre");
    assert!(e.sequence == q && !e.fenetre, "sur le titre de la planche");
    b.app.vignettes.titre_edit.as_mut().unwrap().buffer = "Poursuite à l'école".into();
    b.app.titre_valider();
    b.chauffer(3);
    assert_eq!(b.app.doc.sequence_label(q), "Poursuite à l'école");
    assert_eq!(b.app.history.undo_len(), 1);
    assert!(b.textes.iter().any(|t| t.contains("Poursuite à l'école")), "le titre montre le nouveau nom : {:?}", b.textes);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.sequence_label(q), "Course");
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.sequence_label(q), "Poursuite à l'école");
}

#[test]
fn the_right_click_on_a_title_offers_to_rename_the_sequence() {
    let mut b = banc_vignettes(trois_sequences(), Sens::Lignes, 4);
    let q = b.app.doc.sequence_order()[1];
    let p = titre_a_l_ecran(&b, 1);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    assert_eq!(b.app.vignettes.menu, Some(crate::boardui::MenuTarget::Titre(q)));
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t.contains("Renommer la séquence…")), "l'entrée du menu est là : {:?}", b.textes);
    // Un clic droit ailleurs sur le fond garde le menu d'avant.
    let ailleurs = b.app.vignettes.view.right_bottom() - eframe::egui::vec2(6.0, 40.0);
    b.image(vec![Event::PointerMoved(ailleurs)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: ailleurs, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    assert_ne!(b.app.vignettes.menu, Some(crate::boardui::MenuTarget::Titre(q)));
}

#[test]
fn a_sequence_the_board_does_not_title_is_renamed_in_a_small_window() {
    // Une séquence seule dans le film : pas de titre sur la planche, mais elle se renomme.
    let mut b = banc_vignettes(crate::vignettes_tests::quatre_plans(), Sens::Lignes, 4);
    let q = b.app.doc.sequence_order()[0];
    assert!(b.app.board_layout().groupes.iter().all(|g| g.titre.is_none()), "une seule séquence : aucun titre");
    b.app.run_action("sequence.renommer");
    let e = b.app.vignettes.titre_edit.as_ref().expect("la commande ouvre le nom");
    assert!(e.sequence == q && e.fenetre, "dans une fenêtre");
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t.contains("Renommer la séquence")), "la fenêtre est dessinée : {:?}", b.textes);
    b.app.vignettes.titre_edit.as_mut().unwrap().buffer = "12. INT. JOUR - CUISINE".into();
    b.app.titre_valider();
    b.chauffer(3);
    assert!(b.app.vignettes.titre_edit.is_none());
    assert_eq!(b.app.doc.sequence_label(q), "12. INT. JOUR - CUISINE");
    assert_eq!(b.app.history.undo_len(), 1);
    // Un nom vide rend le numéro automatique ; la même séquence, ouverte à vide, se renomme encore.
    b.app.titre_ouvrir_fenetre(q);
    b.app.vignettes.titre_edit.as_mut().unwrap().buffer = String::new();
    b.app.titre_valider();
    assert_eq!(b.app.doc.sequence_label(q), b.app.doc.sequence_code(q));
    b.app.titre_ouvrir_fenetre(q);
    assert_eq!(b.app.vignettes.titre_edit.as_ref().unwrap().buffer, "", "un nom vide s'ouvre vide, le numéro est l'indication");
    b.app.vignettes.titre_edit.as_mut().unwrap().buffer = "Épilogue — l'été".into();
    b.app.titre_valider();
    assert_eq!(b.app.doc.sequence_label(q), "Épilogue — l'été");
}

#[test]
fn a_hidden_or_titleless_sequence_opens_in_the_window_and_the_name_survives_save_and_reopen() {
    let mut b = banc_vignettes(trois_sequences(), Sens::Lignes, 4);
    let seqs = b.app.doc.sequence_order();
    // La séquence 2 est cachée de la planche : pas de titre à écrire, la fenêtre s'ouvre.
    b.app.affichage_basculer(seqs[1]);
    b.app.select_panel(2);
    b.app.run_action("sequence.renommer");
    assert!(b.app.vignettes.titre_edit.as_ref().is_some_and(|e| e.fenetre && e.sequence == seqs[1]));
    b.app.vignettes.titre_edit.as_mut().unwrap().buffer = "Café — été".into();
    b.app.titre_valider();
    // Enregistrer puis rouvrir : le nom est relu.
    let dossier = std::env::temp_dir().join(format!("feanor-seq-noms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("Essai.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("le projet se rouvre").doc;
    let noms: Vec<String> = relu.sequence_order().iter().map(|q| relu.sequence_label(*q)).collect();
    assert_eq!(noms, vec!["Prologue".to_string(), "Café — été".to_string(), "Fin".to_string()]);
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
fn a_very_long_title_is_cut_by_an_ellipsis_on_the_board_and_kept_whole_in_the_project() {
    let mut doc = trois_sequences();
    let long = format!("12. INT. {} - JOUR", "SALLE DES MACHINES DU GRAND PALAIS ".repeat(5).trim());
    let q = doc.sequence_order()[1];
    doc.sequence_mut(q).unwrap().name = long.clone();
    let mut b = banc_vignettes(doc, Sens::Lignes, 4);
    b.chauffer(3);
    let galley = b.formes.iter().filter_map(|s| if let eframe::egui::epaint::Shape::Text(t) = &s.shape { Some(t.galley.clone()) } else { None }).find(|g| g.job.text == long).expect("le titre est peint");
    assert_eq!(galley.rows.len(), 1, "une seule ligne");
    assert!(galley.rows[0].text().ends_with('…'), "coupé net : {:?}", galley.rows[0].text());
    let largeur = b.app.vignettes.xf.rect(b.app.board_layout().groupes[1].titre.unwrap()).width();
    assert!(galley.size().x < largeur, "il tient dans la largeur de la séquence : {} contre {largeur}", galley.size().x);
    assert_eq!(b.app.doc.sequence_label(q), long, "le nom entier reste dans le projet");
    // Rouvrir le nom puis valider sans rien changer ne le raccourcit pas.
    b.app.titre_ouvrir_fenetre(q);
    b.app.titre_valider();
    assert_eq!(b.app.doc.sequence_label(q), long);
    assert_eq!(b.app.history.undo_len(), 0);
}

#[test]
fn the_board_export_cuts_a_long_title_too() {
    assert_eq!(crate::planche::coupe_au_titre("Prologue", 22.0, 400.0), "Prologue");
    let long = "12. INT. SALLE DES MACHINES DU GRAND PALAIS - JOUR ".repeat(3);
    let coupe = crate::planche::coupe_au_titre(&long, 22.0, 300.0);
    assert!(coupe.ends_with('…') && coupe.chars().count() < long.chars().count(), "{coupe}");
    assert!(crate::text::layout(&coupe, crate::text::FALLBACK, 22.0).bounds.width() <= 300.0);
}

// ------------------------------------------------------------------------------ les lignes de texte qui se cochent

fn plan_textes(doc: &mut Document, plan: usize) {
    doc.panels[plan].dialogue = "Où vas-tu ?".into();
    doc.panels[plan].notes = "La clé".into();
}

#[test]
fn opening_a_project_with_texts_checks_their_lines_and_a_project_without_texts_changes_nothing() {
    let mut b = banc_vignettes(crate::vignettes_tests::quatre_plans(), Sens::Lignes, 4);
    b.app.layout.board_textes = Textes::aucun();
    let vide = crate::vignettes_tests::quatre_plans();
    assert!(b.app.charger_projet(vide));
    assert_eq!(b.app.layout.board_textes, Textes::aucun(), "aucun texte : rien ne se coche");
    let mut doc = crate::vignettes_tests::quatre_plans();
    plan_textes(&mut doc, 1);
    assert!(b.app.charger_projet(doc));
    assert_eq!(b.app.layout.board_textes, Textes { action: false, dialogue: true, notes: true }, "dialogue et notes ont un texte : leurs lignes sont cochées");
    assert!(b.app.status.contains("cochée"), "{}", b.app.status);
    b.chauffer(4);
    for morceau in ["Dialogue  Où vas-tu ?", "Note  La clé"] {
        assert!(b.textes.iter().any(|t| t.contains(morceau)), "« {morceau} » se voit sous la case : {:?}", b.textes);
    }
    // Décocher une ligne la cache ; rien ne la recoche tant qu'on ne saisit pas de nouveau texte.
    b.app.run_action("vignettes.texte_notes");
    b.chauffer(3);
    assert!(!b.app.layout.board_textes.notes);
    assert!(b.textes.iter().all(|t| !t.contains("La clé")));
}

#[test]
fn a_first_text_typed_in_a_field_checks_that_line() {
    let mut b = banc_vignettes(crate::vignettes_tests::quatre_plans(), Sens::Lignes, 4);
    b.app.layout.board_textes = Textes::aucun();
    b.app.vignettes_montrer_champ(Champ::Action);
    assert_eq!(b.app.layout.board_textes, Textes { action: true, dialogue: false, notes: false });
    assert!(b.app.settings_a_ecrire, "le choix est retenu");
    // Déjà cochée : rien de plus.
    b.app.status.clear();
    b.app.vignettes_montrer_champ(Champ::Action);
    assert!(b.app.status.is_empty());
}