//! Versions de séquence qui partagent leurs plans (`variantes.rs`, `variantesui.rs`, partie 59 de `docs/INTERFACE.md`) : une
//! nouvelle version d'un plan est proposée par toutes les versions de séquence, chacune garde son choix de plans, ses durées et
//! la version de chaque plan, on bascule une version de plan dans une seule version de séquence, on met à jour les versions de
//! séquence cochées seulement, tout s'annule, tout se relit à l'identique, un projet d'avant (format 38, copies complètes) s'ouvre
//! sans perte, un projet plus récent est refusé, le film partagé suit.

use std::path::Path;

use crate::document::Document;
use crate::project::{self, Loaded};
use crate::variantes::Nouvelle;
use crate::variantes_tests::{app_partagee, banc_projet, contenu, contenus_film, contenus_range, enregistrer, film, film_fixe, film_riche, ids_film, image, manifeste, plans_range, pomper_enregistrement, retoucher};
use crate::cooperation_tests::{confirmer, film as film_partage};
use crate::fiabilite_fixture::dossier;
use crate::verrous::Cible;

/// Le contenu du plan `plan` sous sa version `version`, où qu'il soit (film, garage ou version rangée).
fn montre(doc: &Document, plan: u64, version: u32) -> u64 {
    contenu(doc, doc.plan_montre(plan, version).expect("plan montré"))
}

/// Le dessin du premier calque du plan `plan` sous sa version `version`, sans la durée (qui est propre à chaque version de séquence).
fn dessin_de(doc: &Document, plan: u64, version: u32) -> u64 {
    let p = doc.plan_montre(plan, version).expect("plan montré");
    crate::empreinte::Donnees::de(&doc.cels[&p.layers[0].frames[0].unwrap()]).empreinte()
}

fn derniere_etape(b: &crate::perf_tests::Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

// ------------------------------------------------------------------------------------ partager

#[test]
fn a_new_plan_version_is_offered_by_every_sequence_version_without_copying_anything() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    let ids = ids_film(&doc);
    let cels = doc.cels.len();
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 2), Some(3));
    assert_eq!(ids_film(&doc), ids, "mêmes plans, mêmes numéros");
    assert_eq!(doc.cels.len(), cels, "aucun dessin recopié");
    assert!(doc.variantes_de(seq).unwrap().garage.is_empty(), "rien au garage : le film montre tous les plans");
    // Une version neuve du plan, dans la version C (celle du film).
    assert_eq!(doc.nouvelle_version(0, 5), Some(2));
    retoucher(&mut doc, 0, 200);
    let v2 = contenu(&doc, &doc.panels[0]);
    let v1 = montre(&doc, p0, 1);
    assert_ne!(v1, v2);
    // Une seule réserve de versions : toutes les versions de séquence la voient, aucune ne bascule d'elle-même.
    assert_eq!(doc.versions_connues(p0), vec![1, 2]);
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 1), (2, 1)]);
    assert_eq!(montre(&doc, p0, 2), v2);
    let apres = doc.cels.len();
    // La version A montre V1 ; la réserve (V2) est la même.
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!((doc.numero_de_version(p0), doc.nombre_de_versions(p0)), (1, 2));
    assert_eq!(contenu(&doc, &doc.panels[0]), v1);
    // Dans la A, on choisit V2 pour ce plan : la B et la C n'en savent rien.
    assert!(doc.changer_de_version(0, 2));
    assert_eq!(contenu(&doc, &doc.panels[0]), v2);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(doc.numero_de_version(p0), 1, "la B montre toujours V1");
    assert_eq!(contenu(&doc, &doc.panels[0]), v1);
    assert!(doc.changer_de_variante(seq, 3));
    assert_eq!(doc.numero_de_version(p0), 2, "la C montrait V2 quand on l'a quittée");
    assert_eq!(doc.cels.len(), apres, "passer d'une version de séquence à l'autre ne recopie aucun dessin");
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 2), (2, 1)]);
}

#[test]
fn plan_choices_and_durations_differ_from_one_sequence_version_to_another() {
    let mut doc = film_fixe(3);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    let avant = contenus_film(&doc, seq);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    // Dans la B : le premier plan dure 24 images, le deuxième n'y est plus.
    doc.panels[0].length = 24;
    assert!(doc.remove_panel(1).is_some());
    assert_eq!(ids_film(&doc), vec![ids[0], ids[2]]);
    assert!(doc.variantes_de(seq).unwrap().garage.contains_key(&ids[1]), "le plan retiré reste à la A : il est au garage");
    // La A : trois plans, le premier à 48 images.
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(ids_film(&doc), ids);
    assert_eq!(doc.panels[0].frame_count(), 48);
    assert_eq!(contenus_film(&doc, seq), avant, "rien n'est perdu : même plans, même contenu");
    assert!(doc.variantes_de(seq).unwrap().garage.is_empty());
    // De nouveau la B.
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(ids_film(&doc), vec![ids[0], ids[2]]);
    assert_eq!(doc.panels[0].frame_count(), 24);
    assert_eq!(doc.panels[1].frame_count(), 48);
    // L'ordre aussi : la B remet le dernier plan en premier ; la A garde le sien.
    assert!(doc.move_panel(1, 0).is_some());
    assert_eq!(ids_film(&doc), vec![ids[2], ids[0]]);
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(ids_film(&doc), ids);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(ids_film(&doc), vec![ids[2], ids[0]]);
    assert_eq!(plans_range(&doc, seq, 1), ids);
}

#[test]
fn a_plan_removed_from_one_sequence_version_stays_in_the_others() {
    let mut doc = film(4);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    let avant = contenus_film(&doc, seq);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    // Supprimer des plans, et toute une scène, dans la B.
    assert!(doc.remove_panels(&[ids[0], ids[1]]).is_some());
    assert!(doc.remove_scene(0).is_some());
    assert_eq!(doc.panels.len(), 1);
    let garage: Vec<u64> = doc.variantes_de(seq).unwrap().garage.keys().copied().collect();
    assert_eq!(garage.len(), 3, "les trois plans sortis du film sont gardés pour la A");
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(ids_film(&doc), ids);
    assert_eq!(contenus_film(&doc, seq), avant);
    // Supprimer la B (la version de séquence) supprime le seul plan qu'elle seule montre, après le passage : aucun.
    assert!(doc.supprimer_variante(seq, 2).is_some());
    assert!(doc.variantes_de(seq).is_none());
    assert_eq!(ids_film(&doc), ids);
}

#[test]
fn deleting_a_sequence_version_deletes_only_the_plans_no_other_version_shows() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    assert!(doc.plans_exclusifs_de_variante(seq, 1).is_empty(), "la A montre les mêmes plans que la B");
    assert!(doc.plans_exclusifs_de_variante(seq, 2).is_empty());
    // Une version vide : son plan à elle est exclusif.
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 2), Some(3));
    let neuf = doc.panels[0].id;
    assert_eq!(doc.plans_exclusifs_de_variante(seq, 3).iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![neuf]);
    assert!(doc.plans_exclusifs_de_variante(seq, 1).is_empty(), "A et B montrent les mêmes trois plans");
    assert!(doc.supprimer_variante(seq, 3).is_some());
    assert!(!doc.variantes_de(seq).unwrap().garage.contains_key(&neuf), "le plan que plus personne ne montre est supprimé");
    assert_eq!(doc.plans_de_sequence(seq).len(), 3);
}

#[test]
fn retouching_a_shared_plan_retouches_it_in_the_other_sequence_version_but_a_copy_stays_apart() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    let avant = contenu(&doc, &doc.panels[0]);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    retoucher(&mut doc, 0, 230);
    let apres = contenu(&doc, &doc.panels[0]);
    assert_ne!(avant, apres);
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(contenu(&doc, &doc.panels[0]), apres, "un plan partagé est le même dans les deux");
    // Pour continuer à part : une nouvelle version du plan.
    assert_eq!(doc.nouvelle_version(0, 3), Some(2));
    retoucher(&mut doc, 0, 40);
    let v2 = contenu(&doc, &doc.panels[0]);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(contenu(&doc, &doc.panels[0]), apres, "la B montre toujours la version d'avant");
    assert_ne!(contenu(&doc, &doc.panels[0]), v2);
    // En copiant les plans : plus rien ne les relie (la copie est rangée sous la B, le film continue en C).
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Copier, 4), Some(3));
    let copie = contenus_range(&doc, seq, 2)[1];
    retoucher(&mut doc, 1, 10);
    assert_ne!(contenu(&doc, &doc.panels[1]), copie);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(contenu(&doc, &doc.panels[1]), copie, "la copie est restée comme avant la retouche");
}

// ------------------------------------------------------------------------------------ choisir, mettre à jour

#[test]
fn a_plan_version_is_switched_in_one_sequence_version_only() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    assert_eq!(doc.nouvelle_version(0, 1), Some(2));
    retoucher(&mut doc, 0, 99);
    let (c1, c2) = (montre(&doc, p0, 1), montre(&doc, p0, 2));
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 2), Some(2));
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 2)]);
    // La B passe le plan à V1.
    assert!(doc.changer_de_version(0, 1));
    assert_eq!(contenu(&doc, &doc.panels[0]), c1);
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 2)], "la A n'a pas bougé");
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!((doc.numero_de_version(p0), contenu(&doc, &doc.panels[0])), (2, c2));
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!((doc.numero_de_version(p0), contenu(&doc, &doc.panels[0])), (1, c1));
    // Une version qu'on n'a pas, ou une version de séquence qu'on n'a pas : rien.
    assert!(!doc.choisir_version_dans_variante(seq, 1, p0, 7));
    assert!(!doc.choisir_version_dans_variante(seq, 9, p0, 1));
    assert!(!doc.choisir_version_dans_variante(seq, 2, p0, 2), "la version choisie n'est pas rangée");
    assert!(doc.choisir_version_dans_variante(seq, 1, p0, 1));
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 1)]);
}

#[test]
fn updating_a_plan_changes_only_the_checked_sequence_versions_and_keeps_their_durations() {
    let mut doc = film_fixe(2);
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    doc.panels[0].length = 30;
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 2), Some(3));
    doc.panels[0].length = 12;
    // Une version neuve du plan, dans la C.
    assert_eq!(doc.nouvelle_version(0, 3), Some(2));
    retoucher(&mut doc, 0, 77);
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 1), (2, 1)]);
    let duree = |d: &Document, n: u32| d.variantes_de(seq).unwrap().rangee(n).unwrap().contenu.slots[0].duree;
    let (da, db) = (duree(&doc, 1), duree(&doc, 2));
    // Mettre à jour la B seulement.
    assert_eq!(doc.mettre_a_jour_variantes(seq, p0, &[2]), 1);
    assert_eq!(doc.variantes_montrant(seq, p0), vec![(1, 1), (2, 2)]);
    assert_eq!((duree(&doc, 1), duree(&doc, 2)), (da, db), "les durées ne bougent pas");
    // Déjà à jour : rien. La version choisie (C) n'est pas une cible.
    assert_eq!(doc.mettre_a_jour_variantes(seq, p0, &[2, 3]), 0);
    // Ce que montre la B : V2 ; la A : V1 ; chacune à sa durée (le dessin, lui, est celui de la version).
    let dessin = |d: &Document| crate::empreinte::Donnees::de(&d.cels[&d.panels[0].layers[0].frames[0].unwrap()]).empreinte();
    let (v1, v2) = (dessin_de(&doc, p0, 1), dessin_de(&doc, p0, 2));
    assert_ne!(v1, v2);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!((doc.numero_de_version(p0), dessin(&doc), doc.panels[0].frame_count()), (2, v2, 30));
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!((doc.numero_de_version(p0), dessin(&doc)), (1, v1));
    assert_eq!(doc.panels[0].frame_count(), 48);
    // La C : sa durée à elle (12), sa version (V2).
    assert!(doc.changer_de_variante(seq, 3));
    assert_eq!((doc.numero_de_version(p0), doc.panels[0].frame_count()), (2, 12));
    // Un plan qui n'est pas dans la séquence ou un plan inconnu : rien.
    assert_eq!(doc.mettre_a_jour_variantes(seq, 999_999, &[1]), 0);
}

#[test]
fn a_version_shown_by_another_sequence_version_is_neither_deleted_nor_purged() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    assert_eq!(doc.nouvelle_version(0, 1), Some(2));
    retoucher(&mut doc, 0, 99);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 2), Some(2));
    assert!(doc.changer_de_version(0, 1));
    // V2 est rangée, et la A la montre.
    assert_eq!(doc.version_montree_ailleurs(p0, 2), vec![1]);
    assert!(!doc.supprimer_version(p0, 2));
    assert_eq!(doc.versions_non_affichees(None), (0, 0));
    assert_eq!(doc.supprimer_versions_non_affichees(None), (0, 0));
    assert_eq!(doc.nombre_de_versions(p0), 2);
    // Dès que la A montre V1, V2 se supprime.
    assert!(doc.choisir_version_dans_variante(seq, 1, p0, 1));
    assert_eq!(doc.versions_non_affichees(None), (1, 1));
    assert!(doc.supprimer_version(p0, 2));
    assert_eq!((doc.numero_de_version(p0), doc.nombre_de_versions(p0)), (1, 1));
}

#[test]
fn purging_keeps_the_numbering_of_a_version_another_sequence_version_shows() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    assert_eq!(doc.nouvelle_version(0, 1), Some(2));
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 2), Some(2));
    // La A montre la version V2 (celle du film) : supprimer les versions non affichées garde le plan à V2, pas à V1.
    assert_eq!(doc.supprimer_versions_non_affichees(None), (1, 1), "V1 n'est montrée par personne");
    assert_eq!((doc.numero_de_version(p0), doc.nombre_de_versions(p0)), (2, 1));
    assert!(doc.versions_du_plan(p0).is_some(), "le numéro V2 reste celui du plan : la A le montre");
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(doc.numero_de_version(p0), 2);
}

// ------------------------------------------------------------------------------------ annuler

#[test]
fn every_new_gesture_is_one_named_undo_step() {
    let mut b = banc_projet(3);
    let seq = b.app.doc.sequence_of(0);
    let p0 = b.app.doc.panels[0].id;
    let ids = ids_film(&b.app.doc);
    // Nouvelle version de séquence, en gardant les plans.
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Partager);
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2, "{}", b.app.status);
    assert_eq!(derniere_etape(&b), "Nouvelle version de la séquence");
    assert_eq!(ids_film(&b.app.doc), ids);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 1);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2);
    // Une nouvelle version du plan (partie 15), puis la mise à jour de la A.
    b.app.cur_panel = 0;
    b.app.nouvelle_version_du_plan(0);
    assert_eq!(b.app.doc.numero_de_version(p0), 2);
    b.app.mettre_a_jour_demander(0);
    let m = b.app.variantes_ui.mettre_a_jour.take().expect("la fenêtre s'ouvre");
    assert_eq!((m.sequence, m.plan, m.coches.clone()), (seq, p0, vec![(1, true)]));
    b.app.mettre_a_jour_dans_les_variantes(seq, p0, &[1]);
    assert_eq!(b.app.doc.variantes_montrant(seq, p0), vec![(1, 2)], "{}", b.app.status);
    assert_eq!(derniere_etape(&b), "Mettre à jour le plan dans 1 version de séquence");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.variantes_montrant(seq, p0), vec![(1, 1)]);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.variantes_montrant(seq, p0), vec![(1, 2)]);
    // Passer à la A : une étape, annulable ; le plan montre V2 (la mise à jour).
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(derniere_etape(&b), "Passer à la version de séquence A");
    assert_eq!(b.app.doc.numero_de_version(p0), 2);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.numero_de_variante(seq), 2);
    // Supprimer la B ne supprime aucun plan : les deux montrent les mêmes.
    assert_eq!(b.app.resume_variante(seq, 2), (0, 0));
    b.app.supprimer_la_variante(seq, 2);
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.doc.variantes_de(seq).is_none());
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2);
    assert_eq!(b.app.doc.variantes_montrant(seq, p0), vec![(1, 2)]);
    // Supprimer la version d'un plan qu'une autre version de séquence montre : refusé, avec la raison.
    b.app.cur_panel = 0;
    assert!(b.app.doc.choisir_version_dans_variante(seq, 1, p0, 1));
    b.app.status.clear();
    b.app.supprimer_la_version(0, 1);
    assert_eq!(b.app.doc.nombre_de_versions(p0), 2, "{}", b.app.status);
    assert!(b.app.status.contains("montrée par la version de séquence A"), "{}", b.app.status);
    // La A montre V2 : V1 n'est plus montrée, elle se supprime.
    assert!(b.app.doc.choisir_version_dans_variante(seq, 1, p0, 2));
    b.app.supprimer_la_version(0, 1);
    assert_eq!(b.app.doc.nombre_de_versions(p0), 1, "{}", b.app.status);
}

#[test]
fn the_update_window_lists_the_other_sequence_versions_and_refuses_when_there_is_nothing_to_do() {
    let mut b = banc_projet(2);
    let seq = b.app.doc.sequence_of(0);
    b.app.mettre_a_jour_demander(0);
    assert!(b.app.variantes_ui.mettre_a_jour.is_none());
    assert!(b.app.status.contains("qu'une version"), "{}", b.app.status);
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Vide);
    // La version choisie est la B (un plan vide) ; ce plan n'est pas dans la A.
    b.app.mettre_a_jour_demander(0);
    assert!(b.app.variantes_ui.mettre_a_jour.is_none());
    assert!(b.app.status.contains("aucune autre version"), "{}", b.app.status);
    // Avec des versions de séquence qui partagent le plan : la fenêtre s'ouvre, se dessine, et n'a rien changé.
    let mut b = banc_projet(2);
    let seq = b.app.doc.sequence_of(0);
    let p0 = b.app.doc.panels[0].id;
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Partager);
    b.app.nouvelle_version_du_plan(0);
    b.app.mettre_a_jour_demander(0);
    assert!(b.app.variantes_ui.mettre_a_jour.is_some(), "{}", b.app.status);
    b.chauffer(3);
    assert!(b.app.variantes_ui.mettre_a_jour.is_some(), "la fenêtre reste ouverte");
    assert_eq!(b.app.doc.variantes_montrant(seq, p0), vec![(1, 1)], "dessiner la fenêtre ne change rien");
    b.app.variantes_ui.mettre_a_jour = None;
    b.chauffer(2);
}

// ------------------------------------------------------------------------------------ enregistrer et rouvrir

#[test]
fn shared_sequence_versions_survive_a_save_and_a_reopening_down_to_the_byte() {
    let mut doc = film_riche();
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1_760_000_000), Some(2));
    assert_eq!(doc.nouvelle_version(0, 1_760_000_050), Some(2));
    retoucher(&mut doc, 0, 90);
    doc.renommer_variante(seq, 1, "serré");
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 1_760_000_100), Some(3));
    let racine = dossier("versions-sequence-fichier");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    assert!(m.parent().unwrap().join("variantes.json").exists());
    let Loaded { doc: mut relu, warnings, .. } = project::load(&m).expect("réouverture");
    assert!(warnings.is_empty(), "{warnings:?}");
    let (v0, v1) = (doc.variantes_de(seq).unwrap(), relu.variantes_de(seq).unwrap());
    assert_eq!(v1.numeros(), [1, 2, 3]);
    assert_eq!((relu.numero_de_variante(seq), relu.etiquette_de_variante(seq, 1).as_str()), (3, "A · serré"));
    assert_eq!(v1.garage.keys().collect::<Vec<_>>(), v0.garage.keys().collect::<Vec<_>>());
    for n in [1, 2] {
        assert_eq!(v1.rangee(n).unwrap().contenu.slots, v0.rangee(n).unwrap().contenu.slots, "emplacements de la version {n}");
        assert_eq!(contenus_range(&relu, seq, n), contenus_range(&doc, seq, n), "contenu de la version {n}");
    }
    assert_eq!(relu.variantes_montrant(seq, p0), doc.variantes_montrant(seq, p0));
    assert_eq!(contenus_film(&relu, seq), contenus_film(&doc, seq));
    // Réécrit, le fichier est le même, octet pour octet ; et passer à la B donne le même film des deux côtés.
    assert_eq!(project::variantes_octets(&relu).unwrap(), project::variantes_octets(&doc).unwrap());
    assert!(doc.changer_de_variante(seq, 2) && relu.changer_de_variante(seq, 2));
    assert_eq!(ids_film(&relu), ids_film(&doc));
    assert_eq!(contenus_film(&relu, seq), contenus_film(&doc, seq));
    assert_eq!((relu.numero_de_version(p0), relu.nombre_de_versions(p0)), (doc.numero_de_version(p0), doc.nombre_de_versions(p0)));
    // Aucun numéro en double, nulle part ; les numéros neufs ne retombent sur aucun.
    let mut tous: Vec<u64> = relu.panels.iter().map(|p| p.id).chain(relu.plans_ranges().map(|p| p.id)).collect();
    let n = tous.len();
    tous.sort_unstable();
    tous.dedup();
    assert_eq!(tous.len(), n);
    let neufs: Vec<u64> = (0..3).map(|_| relu.alloc()).collect();
    assert!(neufs.iter().all(|n| !tous.contains(n) && !relu.ids_des_variantes().contains(n)));
}

/// Réécrit `variantes.json` dans le format 1 (celui d'avant le format 39 du projet) : chaque version de séquence rangée porte ses plans
/// complets, leurs versions, leur contenu rogné et leurs commentaires. Ne vaut que pour un projet fait de copies (`Nouvelle::Copier` :
/// chaque plan du garage est montré par une seule version de séquence).
fn reecrire_en_format_1(fichier: &Path) {
    use serde_json::Value;
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(fichier).unwrap()).unwrap();
    v["version"] = Value::from(1);
    for s in v["sequences"].as_array_mut().unwrap() {
        let garage: Vec<Value> = s.as_object_mut().unwrap().remove("garage").and_then(|g| g.as_array().cloned()).unwrap_or_default();
        for var in s["autres"].as_array_mut().unwrap() {
            let slots = var.as_object_mut().unwrap().remove("emplacements").and_then(|e| e.as_array().cloned()).unwrap_or_default();
            let (mut plans, mut commentaires) = (Vec::new(), Vec::new());
            for e in slots {
                let id = e["plan"].as_u64().unwrap();
                let mut g = garage.iter().find(|g| g["entree"]["id"].as_u64() == Some(id)).expect("plan du garage").clone();
                if let Some(c) = g.as_object_mut().unwrap().remove("commentaires") {
                    commentaires.extend(c.as_array().cloned().unwrap_or_default());
                }
                plans.push(g);
            }
            var["plans"] = Value::from(plans);
            if !commentaires.is_empty() {
                var["commentaires"] = Value::from(commentaires);
            }
        }
    }
    std::fs::write(fichier, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

#[test]
fn a_project_of_format_38_with_variants_opens_without_losing_anything_and_a_newer_one_is_refused() {
    let mut doc = film_riche();
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1_760_000_000);
    doc.renommer_variante(seq, 1, "serré");
    let racine = dossier("versions-sequence-format38");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    let fichier = m.parent().unwrap().join("variantes.json");
    // Le projet tel que la version 38 l'écrivait : manifeste 38, variantes complètes (format 1 du fichier).
    reecrire_en_format_1(&fichier);
    let texte = std::fs::read_to_string(&m).unwrap();
    assert!(texte.contains(&format!("\"version\": {}", project::VERSION)) && project::VERSION >= 39);
    std::fs::write(&m, texte.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 38")).unwrap();
    let Loaded { doc: vieux, warnings, .. } = project::load(&m).expect("un projet 38 s'ouvre");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(vieux.variantes_de(seq).unwrap().numeros(), [1, 2]);
    assert_eq!(vieux.etiquette_de_variante(seq, 1), "A · serré");
    assert_eq!(contenus_film(&vieux, seq), contenus_film(&doc, seq));
    assert_eq!(contenus_range(&vieux, seq, 1), contenus_range(&doc, seq, 1), "la copie rangée est celle d'avant, plans et dessins compris");
    assert_eq!(plans_range(&vieux, seq, 1), plans_range(&doc, seq, 1));
    // Ses versions de plan, son commentaire, sa transition : revenus avec elle.
    let (g0, g1) = (&doc.variantes_de(seq).unwrap().garage, &vieux.variantes_de(seq).unwrap().garage);
    assert_eq!(g1.len(), g0.len());
    assert_eq!(g1.values().map(|g| g.versions.as_ref().map_or(0, |v| v.nombre())).sum::<usize>(), g0.values().map(|g| g.versions.as_ref().map_or(0, |v| v.nombre())).sum::<usize>());
    assert_eq!(g1.values().map(|g| g.comments.len()).sum::<usize>(), 1);
    assert_eq!(vieux.variantes_de(seq).unwrap().rangee(1).unwrap().contenu.transitions.len(), 1);
    // Rien n'est fusionné : les plans copiés ne sont pas ceux du film.
    assert!(plans_range(&vieux, seq, 1).iter().all(|p| !ids_film(&vieux).contains(p)));
    // Enregistré, il devient un projet du format actuel, et se relit à l'identique.
    let racine2 = dossier("versions-sequence-format38-bis");
    let m2 = manifeste(&racine2);
    enregistrer(&vieux, &m2);
    assert!(std::fs::read_to_string(&m2).unwrap().contains(&format!("\"version\": {}", project::VERSION)));
    assert!(std::fs::read_to_string(m2.parent().unwrap().join("variantes.json")).unwrap().contains("\"emplacements\""));
    let Loaded { doc: relu, warnings, .. } = project::load(&m2).expect("réouverture");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(contenus_range(&relu, seq, 1), contenus_range(&doc, seq, 1));
    assert_eq!(project::variantes_octets(&relu).unwrap(), project::variantes_octets(&vieux).unwrap());
    // Un projet d'une version plus récente que celle-ci est refusé (comme toujours) ; celui-ci le serait par la 38.
    std::fs::write(&m2, std::fs::read_to_string(&m2).unwrap().replace(&format!("\"version\": {}", project::VERSION), &format!("\"version\": {}", project::VERSION + 1))).unwrap();
    assert!(project::load(&m2).is_err());
    assert!(project::VERSION > 38, "une version 38 refuse ce format : numéro de projet plus grand");
}

#[test]
fn a_variants_file_of_format_1_is_still_read_when_the_variant_has_no_plan_of_its_own_in_the_film() {
    // Une version de séquence vide (un seul plan neuf) rangée au format 1 : son plan unique est au garage, sans rien d'autre.
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    doc.nouvelle_variante(seq, Nouvelle::Vide, 2);
    let racine = dossier("versions-sequence-format1-vide");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    let fichier = m.parent().unwrap().join("variantes.json");
    reecrire_en_format_1(&fichier);
    let Loaded { doc: relu, warnings, .. } = project::load(&m).expect("s'ouvre");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(relu.variantes_de(seq).unwrap().numeros(), [1, 2, 3]);
    for n in [1, 2] {
        assert_eq!(contenus_range(&relu, seq, n), contenus_range(&doc, seq, n), "version de séquence {n}");
    }
}

// ------------------------------------------------------------------------------------ film partagé

#[test]
fn a_shared_project_keeps_shared_plans_and_the_garage_in_its_folder() {
    let mut doc = film_partage(3);
    let seq = doc.sequence_of(0);
    let p0 = doc.panels[0].id;
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    let (mut b, h, m) = app_partagee("versions-sequence", &doc);
    assert!(m.parent().unwrap().join("variantes.json").exists());
    assert_eq!(b.app.doc.variantes_de(seq).map(|v| v.numeros()), Some(vec![1, 2]));
    assert_eq!(plans_range(&b.app.doc, seq, 1), b.app.doc.plans_de_sequence(seq), "la A montre les mêmes plans que le film");
    // Sans jeton du montage : rien ne change (ni créer, ni passer, ni mettre à jour).
    let film_avant = ids_film(&b.app.doc);
    let plans_seq = b.app.doc.plans_de_sequence(seq);
    for geste in 0..3 {
        b.app.status.clear();
        match geste {
            0 => b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Vide),
            1 => b.app.passer_a_la_variante(seq, 1),
            _ => b.app.mettre_a_jour_dans_les_variantes(seq, p0, &[1]),
        }
        assert!(b.app.status.contains("jeton"), "geste {geste} : {}", b.app.status);
    }
    assert_eq!((ids_film(&b.app.doc), b.app.doc.nombre_de_variantes(seq)), (film_avant.clone(), 2));
    // Avec le jeton : un plan neuf (les trois plans partent au garage), puis le retour à la A.
    b.app.run_action("equipe.prendre_montage");
    h.avancer(9_000);
    image(&mut b);
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Vide);
    assert_eq!(b.app.doc.numero_de_variante(seq), 3, "{}", b.app.status);
    assert_eq!(b.app.doc.variantes_de(seq).unwrap().garage.len(), plans_seq.len(), "les plans de la séquence sont au garage");
    image(&mut b);
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(b.app.doc.numero_de_variante(seq), 1, "{}", b.app.status);
    assert_eq!(ids_film(&b.app.doc), film_avant);
    image(&mut b);
    b.image(Vec::new(), 0.05);
    for p in b.app.doc.plans_de_sequence(seq) {
        assert!(b.app.partage.as_ref().expect("session").tiens(Cible::Plan(p)), "le plan {p} est à moi");
    }
    // L'enregistrement écrit le garage et les emplacements avec le montage ; la relecture est la même.
    b.app.save();
    pomper_enregistrement(&mut b);
    let Loaded { doc: relu, .. } = project::load(&m).expect("réouverture");
    assert_eq!(relu.numero_de_variante(seq), 1);
    assert_eq!(relu.variantes_de(seq).map(|v| v.numeros()), Some(vec![1, 2, 3]));
    assert_eq!(ids_film(&relu), ids_film(&b.app.doc));
    for n in [2, 3] {
        assert_eq!(contenus_range(&relu, seq, n), contenus_range(&b.app.doc, seq, n), "version de séquence {n}");
    }
    confirmer(&h);
}
