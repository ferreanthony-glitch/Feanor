//! Essais de la question « calques de même nom » avant de réunir deux plans (`fusionnoms.rs`).
//!
//! Ce qu'ils gardent :
//! - la fenêtre ne s'ouvre que s'il y a un calque de même nom (qui se réunirait) ; sinon la jonction se fait tout de suite ;
//! - « Fusionner ces calques » : ce qui se faisait ; « Renommer avant de fusionner » : les noms retapés, calques séparés ;
//!   « Ne pas fusionner les calques » : le second renommé « Dessin_2 » (« _3 »…), tous les dessins gardés ; « Annuler » : rien ne bouge ;
//! - une seule étape d'annulation, qui rend le projet exactement ;
//! - les cinq façons de réunir des plans posent la question : « Réunir avec le suivant », « avec le précédent », « Fusionner les plans
//!   choisis », la fusion en glissant, la barre noire des Vignettes ;
//! - plusieurs plans à fusionner : un nom d'avance par plan (« Dessin_2 », « Dessin_3 »).

use std::collections::BTreeSet;

use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{instantane, meme_projet};
use crate::fusionnoms::Reponse;
use crate::perf_tests::{banc_taille, Banc};

const L: usize = 12;

/// Des plans de 12 images dans une séquence ; `noms[k]` : les calques (du bas au haut) du plan k, chacun avec un dessin à lui tenu sur tout le plan.
/// Rend aussi ces dessins : `cels[k][j]`.
fn film(noms: &[&[&str]]) -> (Document, Vec<Vec<CelId>>) {
    let mut doc = Document::with_frame(64, 48, 24);
    for i in 0..noms.len().saturating_sub(1) {
        doc.add_scene_after(i);
    }
    let mut cels = Vec::new();
    for (k, calques) in noms.iter().enumerate() {
        doc.panels[k].layers.clear();
        doc.panels[k].set_length(L);
        let mut ids = Vec::new();
        for nom in *calques {
            let c = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
            let mut l = Layer::new(*nom, LayerKind::Bitmap);
            l.frames = vec![Some(c); L];
            doc.panels[k].layers.push(l);
            ids.push(c);
        }
        cels.push(ids);
    }
    (doc, cels)
}

/// Un banc où la question se pose (les essais, d'ordinaire, y répondent d'avance).
fn banc(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 800, 600);
    b.app.fusion_noms_auto = None;
    b.garder_textes = true;
    b.chauffer(2);
    b
}

fn photo(b: &Banc) -> String {
    instantane(&b.app.doc, &BTreeSet::new())
}

fn nom_des_calques(b: &Banc, plan: usize) -> Vec<String> {
    b.app.doc.panels[plan].layers.iter().map(|l| l.name.clone()).collect()
}

fn calque<'a>(b: &'a Banc, plan: usize, nom: &str) -> &'a Layer {
    b.app.doc.panels[plan].layers.iter().find(|l| l.name == nom).unwrap_or_else(|| panic!("pas de calque « {nom} » : {:?}", nom_des_calques(b, plan)))
}

fn dessins_perdus(avant: &BTreeSet<CelId>, b: &Banc) -> Vec<CelId> {
    let apres = b.app.doc.used_cels();
    avant.iter().copied().filter(|c| !apres.contains(c)).collect()
}

fn nom_de_l_etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

#[test]
fn the_question_is_asked_only_when_two_layers_of_the_same_name_would_merge() {
    // Des noms différents : aucune question, la jonction se fait (le calque du second se réunit au premier par son rang).
    let (doc, _) = film(&[&["Dessin"], &["Fond"]]);
    let mut b = banc(doc);
    b.app.cur_panel = 0;
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms.is_none());
    assert_eq!(b.app.doc.panels.len(), 1, "{}", b.app.status);
    // Le même nom, réglés pareil : la question.
    let (doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    let avant = photo(&b);
    b.app.run_action("panneau.fusionner_suivant");
    let dlg = b.app.fusion_noms.as_ref().expect("la fenêtre s'ouvre");
    assert_eq!(dlg.lignes.len(), 1);
    assert_eq!((dlg.lignes[0].nom.as_str(), dlg.lignes[0].nouveau.as_str()), ("Dessin", "Dessin_2"), "« Dessin_2 » d'avance");
    // Rien n'a bougé, rien n'est enregistré, tant qu'on n'a pas répondu.
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(b.app.history.undo_len(), 0);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
    // Le même nom mais pas réglés pareil (le second est caché) : ils ne se réuniraient pas, donc pas de question.
    let (mut doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    doc.panels[1].layers[0].visible = false;
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms.is_none());
    assert_eq!(b.app.doc.panels.len(), 1);
    // Deux noms en commun : deux lignes ; un nom en commun et un nom à lui : une seule.
    let (doc, _) = film(&[&["Dessin", "Fond"], &["Fond", "Dessin"]]);
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    assert_eq!(b.app.fusion_noms.as_ref().expect("la fenêtre").lignes.len(), 2);
    let (doc, _) = film(&[&["Dessin", "Fond"], &["Ciel", "Dessin"]]);
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    let lignes = &b.app.fusion_noms.as_ref().expect("la fenêtre").lignes;
    assert_eq!(lignes.len(), 1, "seul « Dessin » est en commun : {:?}", lignes.iter().map(|l| &l.nom).collect::<Vec<_>>());
    // Deux plans de deux séquences : rien à réunir, pas de question.
    let (mut doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let (scene, seq) = (doc.panels[1].scene, doc.new_sequence());
    doc.scene_mut(scene).expect("scène").sequence = seq;
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms.is_none());
}

#[test]
fn merge_these_layers_does_what_joining_always_did() {
    let (doc, cels) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    let (avant, dessins) = (photo(&b), b.app.doc.used_cels());
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms_repondre(Reponse::Fusionner), "{}", b.app.status);
    assert!(b.app.fusion_noms.is_none());
    assert_eq!(b.app.doc.panels.len(), 1);
    assert_eq!(nom_des_calques(&b, 0), vec!["Dessin"]);
    let l = calque(&b, 0, "Dessin");
    assert_eq!((l.cel_at(0), l.cel_at(L - 1), l.cel_at(L), l.cel_at(2 * L - 1)), (Some(cels[0][0]), Some(cels[0][0]), Some(cels[1][0]), Some(cels[1][0])));
    assert_eq!(b.app.history.undo_len(), 1, "une seule étape");
    assert_eq!(nom_de_l_etape(&b), "Réunir le plan et le suivant");
    assert!(dessins_perdus(&dessins, &b).is_empty());
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend les deux plans");
}

#[test]
fn do_not_merge_the_layers_keeps_them_apart_and_renames_the_second() {
    let (doc, cels) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    let (avant, dessins) = (photo(&b), b.app.doc.used_cels());
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms_repondre(Reponse::Separer), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 1);
    let mut noms = nom_des_calques(&b, 0);
    noms.sort();
    assert_eq!(noms, vec!["Dessin", "Dessin_2"]);
    let (premier, second) = (calque(&b, 0, "Dessin"), calque(&b, 0, "Dessin_2"));
    // Chaque calque porte ses images, à leur place dans le temps, et rien d'autre.
    assert_eq!((premier.cel_at(0), premier.cel_at(L - 1), premier.cel_at(L)), (Some(cels[0][0]), Some(cels[0][0]), None));
    assert_eq!((second.cel_at(0), second.cel_at(L - 1), second.cel_at(L), second.cel_at(2 * L - 1)), (None, None, Some(cels[1][0]), Some(cels[1][0])));
    assert!(dessins_perdus(&dessins, &b).is_empty(), "aucun dessin perdu");
    assert_eq!(b.app.history.undo_len(), 1);
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("une seule annulation rend les deux plans");
    // Le nom d'avance saute les noms pris : « Dessin_2 » existe déjà dans le premier plan, le second devient « Dessin_3 ».
    let (doc, _) = film(&[&["Dessin", "Dessin_2"], &["Dessin"]]);
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    assert_eq!(b.app.fusion_noms.as_ref().expect("la fenêtre").lignes[0].nouveau, "Dessin_3");
    assert!(b.app.fusion_noms_repondre(Reponse::Separer));
    let mut noms = nom_des_calques(&b, 0);
    noms.sort();
    assert_eq!(noms, vec!["Dessin", "Dessin_2", "Dessin_3"]);
}

#[test]
fn rename_before_merging_uses_the_retyped_names_and_keeps_the_layers_apart() {
    let (doc, cels) = film(&[&["Dessin", "Fond"], &["Dessin", "Fond"]]);
    let mut b = banc(doc);
    let dessins = b.app.doc.used_cels();
    b.app.run_action("panneau.fusionner_suivant");
    {
        let dlg = b.app.fusion_noms.as_mut().expect("la fenêtre");
        assert_eq!(dlg.lignes.len(), 2);
        let prefill: Vec<(&str, &str)> = dlg.lignes.iter().map(|l| (l.nom.as_str(), l.nouveau.as_str())).collect();
        assert_eq!(prefill, vec![("Dessin", "Dessin_2"), ("Fond", "Fond_2")]);
        dlg.lignes[0].nouveau = "Action".into();
        dlg.lignes[1].nouveau = "  Décor lointain  ".into();
    }
    let noms = b.app.fusion_noms.as_ref().unwrap().noms_retapes().expect("tous les noms sont là");
    assert!(b.app.fusion_noms_repondre(Reponse::Renommer(noms)), "{}", b.app.status);
    let mut calques = nom_des_calques(&b, 0);
    calques.sort();
    assert_eq!(calques, vec!["Action", "Dessin", "Décor lointain", "Fond"]);
    // Les calques renommés sont ceux du second plan, avec ses dessins ; ils ne se sont pas réunis par rang.
    assert_eq!(calque(&b, 0, "Action").cel_at(L), Some(cels[1][0]));
    assert_eq!(calque(&b, 0, "Action").cel_at(0), None);
    assert_eq!(calque(&b, 0, "Décor lointain").cel_at(L), Some(cels[1][1]));
    assert_eq!(calque(&b, 0, "Dessin").cel_at(L), None);
    assert!(dessins_perdus(&dessins, &b).is_empty());
    assert_eq!(b.app.history.undo_len(), 1);
    // Un nom vide n'est pas accepté.
    let (doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    b.app.fusion_noms.as_mut().unwrap().lignes[0].nouveau = "   ".into();
    assert!(b.app.fusion_noms.as_ref().unwrap().noms_retapes().is_none());
    // Retaper le nom que le premier plan a déjà : les deux calques se réunissent (c'est ce qu'on a demandé).
    let (doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    b.app.fusion_noms.as_mut().unwrap().lignes[0].nouveau = "Dessin".into();
    let noms = b.app.fusion_noms.as_ref().unwrap().noms_retapes().unwrap();
    assert!(b.app.fusion_noms_repondre(Reponse::Renommer(noms)));
    assert_eq!(nom_des_calques(&b, 0), vec!["Dessin"]);
}

#[test]
fn cancelling_leaves_the_plans_untouched() {
    let (doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    let avant = photo(&b);
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms.is_some());
    b.app.fusion_noms_annuler();
    assert!(b.app.fusion_noms.is_none());
    assert_eq!(b.app.history.undo_len(), 0);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
    assert!(b.app.status.contains("non réunis"), "{}", b.app.status);
    // Échap dans la fenêtre ouverte : la même chose.
    b.app.run_action("panneau.fusionner_suivant");
    b.chauffer(2);
    b.image(vec![eframe::egui::Event::Key { key: eframe::egui::Key::Escape, physical_key: Some(eframe::egui::Key::Escape), pressed: true, repeat: false, modifiers: eframe::egui::Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert!(b.app.fusion_noms.is_none(), "Échap ferme la question");
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
}

#[test]
fn the_window_shows_the_three_answers_and_cancel() {
    let (doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    b.app.run_action("panneau.fusionner_suivant");
    b.chauffer(3);
    let vus = b.textes.join("\n");
    for t in ["Calques de même nom", "Fusionner ces calques", "Renommer avant de fusionner", "Ne pas fusionner les calques", "Annuler", "« Dessin »"] {
        assert!(vus.contains(t), "« {t} » manque :\n{vus}");
    }
    // L'étape « Renommer » montre la liste avec le nom d'avance.
    b.app.fusion_noms.as_mut().unwrap().etape = crate::fusionnoms::Etape::Renommer;
    b.chauffer(3);
    let vus = b.textes.join("\n");
    assert!(vus.contains("Renommer et fusionner") && vus.contains("Retour"), "{vus}");
}

#[test]
fn merging_several_chosen_plans_gives_each_plan_its_own_name() {
    let (doc, cels) = film(&[&["Dessin"], &["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    let (avant, dessins) = (photo(&b), b.app.doc.used_cels());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.board_sel = ids.clone();
    b.app.cur_panel = 0;
    b.app.run_action("panneau.fusionner_choisis");
    let prefill: Vec<String> = b.app.fusion_noms.as_ref().expect("la fenêtre").lignes.iter().map(|l| l.nouveau.clone()).collect();
    assert_eq!(prefill, vec!["Dessin_2", "Dessin_3"], "un nom d'avance par plan");
    assert!(b.app.fusion_noms_repondre(Reponse::Separer), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 1);
    let mut noms = nom_des_calques(&b, 0);
    noms.sort();
    assert_eq!(noms, vec!["Dessin", "Dessin_2", "Dessin_3"]);
    assert_eq!(calque(&b, 0, "Dessin_2").cel_at(L), Some(cels[1][0]));
    assert_eq!(calque(&b, 0, "Dessin_3").cel_at(2 * L), Some(cels[2][0]));
    assert!(dessins_perdus(&dessins, &b).is_empty());
    assert_eq!(b.app.history.undo_len(), 1);
    assert_eq!(nom_de_l_etape(&b), "Fusionner les plans choisis");
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("une seule annulation");
    // Fusionner : un seul calque « Dessin », les trois dessins à la suite.
    b.app.board_sel = ids;
    b.app.cur_panel = 0;
    b.app.run_action("panneau.fusionner_choisis");
    assert!(b.app.fusion_noms_repondre(Reponse::Fusionner));
    assert_eq!(nom_des_calques(&b, 0), vec!["Dessin"]);
    assert_eq!(calque(&b, 0, "Dessin").cel_at(2 * L), Some(cels[2][0]));
}

#[test]
fn the_other_ways_to_join_plans_ask_the_question_too() {
    // « Réunir avec le plan précédent ».
    let (doc, cels) = film(&[&["Dessin"], &["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    b.app.cur_panel = 2;
    b.app.run_action("scene.joindre_precedente");
    assert!(b.app.fusion_noms.is_some(), "la question se pose");
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.fusion_noms_repondre(Reponse::Separer), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(nom_de_l_etape(&b), "Réunir avec le plan précédent");
    assert_eq!(calque(&b, 1, "Dessin_2").cel_at(L), Some(cels[2][0]));
    // La fusion en glissant un plan sur un autre (le plan glissé n'est pas voisin : il vient se coller au visé).
    let (doc, cels) = film(&[&["Dessin"], &["Fond"], &["Dessin"]]);
    let mut b = banc(doc);
    let (p0, p2) = (b.app.doc.panels[0].id, b.app.doc.panels[2].id);
    b.app.fusionner_en_glissant(p2, p0);
    assert!(b.app.fusion_noms.is_some(), "la question se pose aussi en glissant");
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.fusion_noms_repondre(Reponse::Separer), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(nom_de_l_etape(&b), "Fusionner deux plans");
    // (Le plan glissé venait après le plan visé : ses images viennent à la suite.)
    assert_eq!(calque(&b, 0, "Dessin_2").cel_at(L), Some(cels[2][0]));
    // La barre noire des Vignettes.
    let mut doc = film(&[&["Dessin"], &["Dessin"], &["Dessin"]]).0;
    doc.into_plans();
    let mut b = crate::vignettes_tests::ouvert(doc, 6);
    b.app.fusion_noms_auto = None;
    let layout = b.app.board_layout();
    assert!(!b.app.board_toggle_cut(&layout, 0), "tant que la question attend, rien n'est fait");
    assert!(b.app.fusion_noms.is_some());
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.fusion_noms_repondre(Reponse::Separer), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(nom_de_l_etape(&b), "Réunir deux vignettes en un plan");
    let mut noms = nom_des_calques(&b, 0);
    noms.sort();
    assert_eq!(noms, vec!["Dessin", "Dessin_2"]);
}

#[test]
fn an_answer_given_in_advance_skips_the_window() {
    let (doc, _) = film(&[&["Dessin"], &["Dessin"]]);
    let mut b = banc(doc);
    b.app.fusion_noms_auto = Some(Reponse::Separer);
    b.app.run_action("panneau.fusionner_suivant");
    assert!(b.app.fusion_noms.is_none());
    assert_eq!(b.app.doc.panels.len(), 1);
    assert_eq!(nom_des_calques(&b, 0).len(), 2);
}

#[test]
fn the_conflicts_are_found_without_touching_the_document() {
    let (doc, _) = film(&[&["Dessin", "Fond"], &["Dessin", "Ciel"], &["Ciel", "Dessin"]]);
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let avant = instantane(&doc, &BTreeSet::new());
    let c = doc.join_name_conflicts(&ids);
    // Le deuxième et le troisième plan ont « Dessin » en commun avec le premier. (Les « Ciel » se réunissent au « Fond » du premier par
    // leur rang, et n'ont de nom en commun avec rien : pas de question pour eux.)
    let noms: Vec<(u64, &str)> = c.iter().map(|x| (x.panel, x.name.as_str())).collect();
    assert_eq!(noms, vec![(ids[1], "Dessin"), (ids[2], "Dessin")]);
    assert_eq!(instantane(&doc, &BTreeSet::new()), avant);
    assert!(doc.join_name_conflicts(&ids[..1]).is_empty());
}

#[test]
fn free_names_count_up_from_two() {
    use crate::fusionnoms::nom_libre;
    let pris = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<std::collections::HashSet<String>>();
    assert_eq!(nom_libre("Dessin", &pris(&["Dessin"])), "Dessin_2");
    assert_eq!(nom_libre("Dessin", &pris(&["Dessin", "Dessin_2", "Dessin_3"])), "Dessin_4");
    assert_eq!(nom_libre("Dessin_2", &pris(&["Dessin", "Dessin_2"])), "Dessin_3");
    assert_eq!(nom_libre("Fond_", &pris(&["Fond_"])), "Fond__2");
}
