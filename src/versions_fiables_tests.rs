//! Versionnage fiable (partie 102 de `docs/INTERFACE.md`) : rien ne se perd en silence. Un plan sorti de sa séquence reste dans ses
//! versions rangées, le dernier plan d'une séquence qui a d'autres versions ne s'en va pas, couper ou réunir un plan à plusieurs
//! versions est refusé avec la raison, un glisser en cours ne survit pas à un changement de version. Chaque geste : une étape
//! d'annulation, un aller-retour par le disque.

use std::collections::HashSet;

use crate::document::{Document, Panel};
use crate::fiabilite_fixture::dossier;
use crate::handles::Handle;
use crate::perf_tests::banc;
use crate::project::{self, Loaded};
use crate::variantes::{prendre_refus, Nouvelle};
use crate::variantes_tests::{banc_projet, contenu, contenus_film, enregistrer, film, film_fixe, ids_film, manifeste};

/// Quatre plans dans une séquence qui a deux versions (la B est celle du film, la A est rangée et montre les quatre plans), puis les
/// deux derniers plans quittent la séquence pour une séquence neuve.
fn deux_sequences() -> (Document, u64, u64, Vec<u64>, Vec<u64>) {
    let mut doc = film(4);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    let quatre = contenus_film(&doc, seq);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    assert!(doc.new_sequence_from_plans(&[ids[2], ids[3]]).is_some());
    let autre = doc.sequence_of(2);
    assert_ne!(autre, seq);
    (doc, seq, autre, ids, quatre)
}

fn scenes_uniques(doc: &Document) -> bool {
    let mut vues = HashSet::new();
    doc.scenes.iter().all(|s| vues.insert(s.id)) && doc.panels.iter().all(|p| doc.scene(p.scene).is_some())
}

// ------------------------------------------------------------------------------------ 1. un plan sorti de sa séquence

#[test]
fn a_plan_moved_to_another_sequence_stays_in_the_stored_versions_of_its_first_sequence() {
    let (mut doc, seq, autre, ids, quatre) = deux_sequences();
    assert_eq!(doc.plans_de_sequence(seq), vec![ids[0], ids[1]]);
    // La A (rangée) montrait les quatre plans : on y passe, rien n'y manque.
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(doc.plans_de_sequence(seq).len(), 4, "la version A garde ses quatre plans");
    assert_eq!(contenus_film(&doc, seq), quatre, "les mêmes dessins, dans le même ordre");
    // L'autre séquence n'a pas bougé : ses plans sont toujours les siens.
    assert_eq!(doc.plans_de_sequence(autre), vec![ids[2], ids[3]]);
    assert!(scenes_uniques(&doc), "aucune scène en double dans le film");
    // Aller et retour : la B a toujours ses deux plans, la A ses quatre, sans plus de copies.
    let plans_a = doc.plans_de_sequence(seq);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(doc.plans_de_sequence(seq), vec![ids[0], ids[1]]);
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(doc.plans_de_sequence(seq), plans_a, "les copies sont celles de la première fois");
    assert!(scenes_uniques(&doc));
}

#[test]
fn a_plan_that_came_back_to_its_sequence_is_not_copied() {
    let (mut doc, seq, _autre, ids, _) = deux_sequences();
    // Les deux plans reviennent dans la première séquence avant qu'on passe à la A.
    assert!(doc.move_plans(&[ids[2], ids[3]], Some(ids[1])).is_some());
    assert_eq!(doc.plans_de_sequence(seq).len(), 4);
    let n = doc.panels.len();
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(doc.panels.len(), n, "aucun plan de plus : ceux du film sont ceux de la version");
    assert_eq!(doc.plans_de_sequence(seq), ids);
    assert!(scenes_uniques(&doc));
}

#[test]
fn a_single_plan_moved_to_another_sequence_stays_in_every_stored_version() {
    let mut doc = film(4);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    let quatre = contenus_film(&doc, seq);
    doc.add_sequence_after(3);
    let autre = doc.sequence_of(4);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    // Le troisième plan passe dans l'autre séquence.
    assert!(doc.move_plans(&[ids[2]], None).is_some());
    assert_eq!(doc.sequence_of(doc.panels.iter().position(|p| p.id == ids[2]).unwrap()), autre);
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(doc.plans_de_sequence(seq).len(), 4);
    assert_eq!(contenus_film(&doc, seq), quatre);
    assert!(scenes_uniques(&doc));
}

#[test]
fn the_plans_a_stored_version_gets_back_survive_undo_redo_and_a_save() {
    let mut b = banc_projet(4);
    let seq = b.app.doc.sequence_of(0);
    let ids = ids_film(&b.app.doc);
    let quatre = contenus_film(&b.app.doc, seq);
    assert_eq!(b.app.doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    assert!(b.app.doc.new_sequence_from_plans(&[ids[2], ids[3]]).is_some());
    b.app.cur_panel = 0;
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(b.app.doc.plans_de_sequence(seq).len(), 4, "{}", b.app.status);
    assert_eq!(contenus_film(&b.app.doc, seq), quatre);
    // Une étape d'annulation : annuler revient à la B (deux plans), rétablir à la A (quatre).
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.numero_de_variante(seq), b.app.doc.plans_de_sequence(seq)), (2, vec![ids[0], ids[1]]));
    b.app.run_action("edition.retablir");
    assert_eq!((b.app.doc.numero_de_variante(seq), b.app.doc.plans_de_sequence(seq).len()), (1, 4));
    assert_eq!(contenus_film(&b.app.doc, seq), quatre);
    // Enregistré puis relu : la A a ses quatre plans, la B ses deux, aucun dessin perdu.
    let racine = dossier("versions-fiables-plan-sorti");
    let m = manifeste(&racine);
    enregistrer(&b.app.doc, &m);
    let Loaded { doc: mut relu, warnings, .. } = project::load(&m).expect("réouverture");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(contenus_film(&relu, seq), quatre);
    assert!(relu.changer_de_variante(seq, 2));
    assert_eq!(relu.plans_de_sequence(seq).len(), 2);
    assert!(relu.changer_de_variante(seq, 1));
    assert_eq!(contenus_film(&relu, seq), quatre);
    assert!(scenes_uniques(&relu));
}

#[test]
fn a_stored_version_saved_while_its_plan_lives_in_another_sequence_reopens_whole() {
    let (doc, seq, autre, ids, quatre) = deux_sequences();
    let racine = dossier("versions-fiables-plan-ailleurs");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    let Loaded { doc: mut relu, warnings, .. } = project::load(&m).expect("réouverture");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(relu.changer_de_variante(seq, 1));
    assert_eq!(contenus_film(&relu, seq), quatre, "la A relue montre ses quatre plans");
    assert_eq!(relu.plans_de_sequence(autre), vec![ids[2], ids[3]], "l'autre séquence garde ses plans : la A ne les lui prend pas");
    assert!(scenes_uniques(&relu));
}

// ------------------------------------------------------------------------------------ 2. le dernier plan d'une séquence

/// Trois plans : les deux premiers dans la séquence 1, le dernier seul dans la séquence 2, qui a une version rangée (la A, copiée).
fn sequence_seule_avec_version() -> (Document, u64, Vec<u64>) {
    let mut doc = film(2);
    doc.add_sequence_after(1);
    let ids = ids_film(&doc);
    let seq = doc.sequence_of(2);
    assert_ne!(seq, doc.sequence_of(0));
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Copier, 1), Some(2));
    (doc, seq, ids)
}

#[test]
fn the_last_plan_of_a_sequence_that_has_other_versions_is_never_removed_or_moved_away() {
    let (mut doc, seq, ids) = sequence_seule_avec_version();
    let garage = doc.variantes_de(seq).unwrap().garage.len();
    assert!(garage > 0);
    let id = ids[2];
    let (scene_premiere, avant) = (doc.panels[0].scene, ids_film(&doc));
    let intact = |doc: &Document| {
        assert_eq!(ids_film(doc), avant, "rien n'a bougé");
        let v = doc.variantes_de(seq).expect("les versions sont toujours là");
        assert_eq!((v.nombre(), v.garage.len()), (2, garage));
    };
    prendre_refus();
    assert_eq!(doc.remove_panel(2), None);
    assert!(prendre_refus().is_some_and(|m| m.contains("dernier plan") && m.contains("versions de séquence")));
    intact(&doc);
    assert_eq!(doc.remove_panels(&[id]), None);
    assert!(prendre_refus().is_some());
    intact(&doc);
    assert_eq!(doc.remove_scene(2), None);
    assert!(prendre_refus().is_some());
    intact(&doc);
    assert_eq!(doc.move_panel(2, 0), None, "glissé dans la première séquence");
    assert!(prendre_refus().is_some());
    intact(&doc);
    assert_eq!(doc.move_panels(&[id], scene_premiere, None), None);
    assert!(prendre_refus().is_some());
    intact(&doc);
    assert_eq!(doc.move_plans(&[id], Some(ids[0])), None);
    assert!(prendre_refus().is_some());
    intact(&doc);
    assert_eq!(doc.new_sequence_from_plans(&[ids[1], id]), None, "choisi avec un plan d'une autre séquence");
    assert!(prendre_refus().is_some());
    intact(&doc);
    // Ce qui laisse un plan à la séquence passe comme avant, sans rien dire.
    prendre_refus();
    assert!(doc.move_panel(0, 2).is_some(), "un plan d'une séquence sans version bouge");
    assert!(prendre_refus().is_none());
}

#[test]
fn once_the_other_versions_are_gone_the_last_plan_of_a_sequence_goes_normally() {
    let (mut doc, seq, ids) = sequence_seule_avec_version();
    assert!(doc.supprimer_variante(seq, 1).is_some());
    assert!(doc.variantes_de(seq).is_none());
    assert!(doc.remove_panel(2).is_some());
    assert_eq!(ids_film(&doc), vec![ids[0], ids[1]]);
}

#[test]
fn a_sequence_with_other_versions_still_loses_its_other_plans_and_keeps_them_for_those_versions() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    assert!(doc.remove_panels(&[ids[0], ids[1]]).is_some(), "il reste un plan à la séquence");
    assert_eq!(doc.panels.len(), 1);
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(ids_film(&doc), ids);
}

#[test]
fn deleting_the_last_plan_from_the_application_says_why_and_changes_nothing() {
    let (doc, seq, _) = sequence_seule_avec_version();
    let mut b = banc(doc, false);
    b.app.modified = false;
    let ids = ids_film(&b.app.doc);
    let historique = b.app.history.names().0.len();
    b.app.cur_panel = 2;
    b.app.delete_panels(&[ids[2]]);
    b.app.dire_le_refus_des_versions();
    assert!(b.app.status.contains("versions de séquence"), "{}", b.app.status);
    assert_eq!(ids_film(&b.app.doc), ids);
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2);
    assert_eq!(b.app.history.names().0.len(), historique, "aucune étape d'annulation de plus");
    assert!(!b.app.modified);
}

// ------------------------------------------------------------------------------------ 3. couper et réunir un plan à plusieurs versions

#[test]
fn a_plan_with_several_versions_is_neither_cut_nor_joined_and_the_reason_is_given() {
    let mut doc = film(3);
    let ids = ids_film(&doc);
    assert_eq!(doc.nouvelle_version(1, 5), Some(2));
    let contenus: Vec<u64> = doc.panels.iter().map(|p| contenu(&doc, p)).collect();
    prendre_refus();
    assert_eq!(doc.split_plan(1, 10), None);
    assert!(prendre_refus().is_some_and(|m| m.contains("couper") && m.contains("V1") && m.contains("V2")));
    assert_eq!(doc.split_panel(1, 10), None);
    assert!(prendre_refus().is_some());
    // Le premier plan est seul, mais son suivant a des versions : réunir les perdrait.
    assert_eq!(doc.join_next_plan(0), None);
    assert!(prendre_refus().is_some_and(|m| m.contains("réunir")));
    assert_eq!(doc.join_next_plan_without_mark(0), None);
    assert!(prendre_refus().is_some());
    assert_eq!(ids_film(&doc), ids);
    assert_eq!(doc.panels.iter().map(|p| contenu(&doc, p)).collect::<Vec<_>>(), contenus, "aucun plan n'a changé, marques comprises");
    assert_eq!(doc.nombre_de_versions(ids[1]), 2);
    // Une fois la version en trop supprimée, le plan se coupe comme avant.
    assert!(doc.supprimer_version(ids[1], 1));
    assert!(doc.split_plan(1, 10).is_some());
    assert_eq!(doc.panels.len(), 4);
}

#[test]
fn cutting_a_plan_from_the_application_keeps_its_marks_when_the_versions_refuse() {
    let mut b = banc_projet(2);
    b.app.doc.nouvelle_version(0, 1);
    let plan = b.app.doc.panels[0].id;
    let id = b.app.doc.alloc();
    b.app.doc.panels[0].marks.push(crate::document::ImageMark { id, frame: 20, ..Default::default() });
    let ok = b.app.structure_op_dessins(|d, i| d.split_plan(i, 10));
    assert!(!ok);
    assert_eq!(b.app.doc.panels[0].marks.len(), 1, "la marque est toujours là");
    assert_eq!(b.app.doc.panels[0].id, plan);
    b.app.dire_le_refus_des_versions();
    assert!(b.app.status.contains("plusieurs versions"), "{}", b.app.status);
}

// ------------------------------------------------------------------------------------ 4. échanger, glisser, calque

#[test]
fn switching_a_plan_version_swaps_everything_that_belongs_to_a_version_and_keeps_the_rest_with_the_plan() {
    let mut doc = film(2);
    let plan = doc.panels[0].id;
    doc.add_comment(plan, 3, "Anthony", 7);
    doc.panels[0].action = "version un".into();
    doc.panels[0].cues.bookmarks = vec![4];
    doc.panels[0].color = Some(2);
    assert_eq!(doc.nouvelle_version(0, 5), Some(2));
    doc.panels[0].action = "version deux".into();
    doc.panels[0].cues.bookmarks = vec![9];
    doc.panels[0].color = Some(3);
    assert!(doc.changer_de_version(0, 1));
    let p: &Panel = &doc.panels[0];
    assert_eq!((p.action.as_str(), p.cues.bookmarks.clone(), p.color), ("version un", vec![4], Some(2)));
    assert_eq!(doc.comments.iter().filter(|c| c.panel == plan).count(), 1, "le commentaire reste au plan, à toute version");
    assert!(doc.changer_de_version(0, 2));
    assert_eq!(doc.panels[0].action, "version deux");
    assert_eq!(doc.comments.iter().filter(|c| c.panel == plan).count(), 1);
}

#[test]
fn a_montage_drag_in_progress_is_abandoned_before_a_plan_version_changes() {
    let mut b = banc(film_fixe(2), false);
    b.app.modified = false;
    let plan = b.app.doc.panels[0].id;
    assert_eq!(b.app.doc.nouvelle_version(0, 1), Some(2));
    // La V1 rangée dure 30 images, la V2 (celle du plan) 48.
    b.app.doc.versions.get_mut(&plan).unwrap().autres[0].panel.length = 30;
    assert!(b.app.begin_handle_drag(Handle::PlanEnd));
    b.app.drag_handle(20, 20);
    assert_eq!(b.app.doc.panels[0].frame_count(), 20, "le glisser a raccourci le plan");
    // On passe à la V1 sans lâcher : le glisser s'abandonne, la V2 rangée garde sa durée d'avant le geste.
    b.app.passer_a_la_version(0, 1);
    assert!(!b.app.dragging(), "plus de glisser en cours");
    assert_eq!(b.app.doc.numero_de_version(plan), 1);
    assert_eq!(b.app.doc.panels[0].frame_count(), 30);
    assert_eq!(b.app.doc.versions_du_plan(plan).unwrap().rangee(2).unwrap().panel.frame_count(), 48, "la V2 n'a pas gardé la durée à moitié glissée");
    // Une étape d'annulation, et elle ramène la V2 telle qu'elle était.
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.numero_de_version(plan), b.app.doc.panels[0].frame_count()), (2, 48));
}

#[test]
fn a_montage_drag_in_progress_is_abandoned_before_a_sequence_version_changes() {
    let mut b = banc(film_fixe(3), false);
    b.app.modified = false;
    let seq = b.app.doc.sequence_of(0);
    let ids = ids_film(&b.app.doc);
    assert_eq!(b.app.doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    assert!(b.app.doc.remove_panel(2).is_some());
    let avant = b.app.doc.panels[0].frame_count();
    assert!(b.app.begin_handle_drag(Handle::PlanEnd));
    b.app.drag_handle(10, 10);
    assert_ne!(b.app.doc.panels[0].frame_count(), avant);
    b.app.passer_a_la_variante(seq, 1);
    assert!(!b.app.dragging());
    assert_eq!(ids_film(&b.app.doc), ids, "la A a ses trois plans");
    assert_eq!(b.app.doc.panels[0].frame_count(), avant, "la durée du premier plan n'est pas celle du glisser");
}

#[test]
fn a_sequence_version_whose_plans_have_no_layer_does_not_break_the_switch() {
    let mut b = banc_projet(2);
    let seq = b.app.doc.sequence_of(0);
    assert_eq!(b.app.doc.nouvelle_variante(seq, Nouvelle::Copier, 1), Some(2));
    for v in std::sync::Arc::make_mut(&mut b.app.doc.variantes).values_mut() {
        for g in v.garage.values_mut() {
            g.panel.layers.clear();
        }
    }
    b.app.cur_layer = 3;
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(b.app.doc.numero_de_variante(seq), 1, "{}", b.app.status);
    assert!(b.app.cur_layer <= b.app.layers().len().max(1), "le calque choisi reste dans les bornes");
}
