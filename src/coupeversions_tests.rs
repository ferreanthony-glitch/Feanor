//! Couper un plan qui a des versions rangées (`coupeversions.rs`, partie 102 de `docs/INTERFACE.md`) : la question « avec quelle
//! partie ? », rien n'est fait en attendant, les versions rangées restent entières avec la partie choisie sans copier un dessin,
//! une étape d'annulation, enregistrer et rouvrir, tous les chemins de coupe, la question qui tombe quand le plan ou le projet change.

use crate::coupeversions::Refaire;
use crate::deposer::Cible;
use crate::document::{Document, ImageMark};
use crate::fiabilite_fixture::dossier;
use crate::perf_tests::Banc;
use crate::project::{self, Loaded};
use crate::raster::Raster;
use crate::variantes::{avec_cote, Cote, Nouvelle};
use crate::variantes_tests::{banc_projet, contenu, enregistrer, film, ids_film, manifeste, retoucher};

/// Un projet de deux plans de 48 images ; le premier a une version rangée (V1) et la V2 est celle du plan (retouchée). La question
/// est posée pour de bon (pas de réponse d'avance).
fn banc_a_versions() -> (Banc, u64) {
    let mut b = banc_projet(2);
    b.app.coupe_versions_auto = None;
    let plan = b.app.doc.panels[0].id;
    assert_eq!(b.app.doc.nouvelle_version(0, 1), Some(2));
    retoucher(&mut b.app.doc, 0, 200);
    b.app.modified = false;
    (b, plan)
}

/// Les contenus des versions rangées du plan `plan`, et les numéros de leurs dessins (rien n'est recopié si ceux-ci ne changent pas).
fn rangees(doc: &Document, plan: u64) -> (Vec<u64>, Vec<Vec<u64>>) {
    let Some(v) = doc.versions_du_plan(plan) else { return (Vec::new(), Vec::new()) };
    let cels = v.autres.iter().map(|r| r.panel.layers.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect()).collect();
    (v.autres.iter().map(|r| contenu(doc, &r.panel)).collect(), cels)
}

fn couper(b: &mut Banc, frame: usize) {
    b.app.cur_frame = frame;
    b.app.split_current_panel();
}

#[test]
fn cutting_a_plan_with_stored_versions_asks_and_touches_nothing_until_the_answer() {
    let (mut b, plan) = banc_a_versions();
    let ids = ids_film(&b.app.doc);
    let versions = rangees(&b.app.doc, plan);
    let historique = b.app.history.names().0.len();
    couper(&mut b, 10);
    let q = b.app.coupe_versions.as_ref().expect("la question est posée");
    assert_eq!((q.plan, q.n), (plan, 1));
    assert!(b.app.modal_open(), "les raccourcis attendent la réponse");
    assert_eq!(ids_film(&b.app.doc), ids, "le document n'est pas touché");
    assert_eq!(rangees(&b.app.doc, plan), versions);
    assert_eq!(b.app.history.names().0.len(), historique, "aucune étape d'annulation");
    assert!(!b.app.modified);
    // Un plan sans version rangée se coupe sans question.
    let mut b2 = banc_projet(2);
    b2.app.coupe_versions_auto = None;
    couper(&mut b2, 10);
    assert!(b2.app.coupe_versions.is_none());
    assert_eq!(b2.app.doc.panels.len(), 3);
}

#[test]
fn the_stored_versions_follow_the_first_part_when_asked() {
    let (mut b, plan) = banc_a_versions();
    let ids = ids_film(&b.app.doc);
    let versions = rangees(&b.app.doc, plan);
    couper(&mut b, 10);
    assert!(b.app.coupe_versions_repondre(Cote::Premiere));
    assert!(b.app.coupe_versions.is_none());
    let apres = ids_film(&b.app.doc);
    assert_eq!(apres.len(), 3);
    let suite = apres[1];
    assert_eq!(apres[0], plan);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2, "la première partie garde ses versions");
    assert_eq!(b.app.doc.nombre_de_versions(suite), 1, "la seconde n'a que son contenu courant");
    assert_eq!(rangees(&b.app.doc, plan), versions, "la version rangée est entière, ses dessins sont les mêmes (rien de recopié)");
    assert_eq!((b.app.doc.panels[0].frame_count(), b.app.doc.panels[1].frame_count()), (10, 38), "la version courante est coupée en deux");
    // Une étape d'annulation : annuler remet un seul plan et ses versions, rétablir recoupe.
    b.app.run_action("edition.annuler");
    assert_eq!(ids_film(&b.app.doc), ids);
    assert_eq!(rangees(&b.app.doc, plan), versions);
    assert_eq!(b.app.doc.panels[0].frame_count(), 48);
    b.app.run_action("edition.retablir");
    assert_eq!(ids_film(&b.app.doc), apres);
    assert_eq!(rangees(&b.app.doc, plan), versions);
}

#[test]
fn the_stored_versions_follow_the_second_part_when_asked_and_survive_undo_and_a_save() {
    let (mut b, plan) = banc_a_versions();
    let ids = ids_film(&b.app.doc);
    let versions = rangees(&b.app.doc, plan);
    couper(&mut b, 10);
    assert!(b.app.coupe_versions_repondre(Cote::Seconde));
    let apres = ids_film(&b.app.doc);
    assert_eq!(apres.len(), 3);
    let suite = apres[1];
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1, "la première partie n'a plus que son contenu courant");
    assert_eq!(b.app.doc.nombre_de_versions(suite), 2, "la seconde a les versions rangées");
    assert_eq!(rangees(&b.app.doc, suite), versions, "entières, mêmes dessins : rien de recopié");
    assert!(b.app.doc.versions.keys().all(|p| b.app.doc.panels.iter().any(|q| q.id == *p)), "aucune version sous un numéro de plan mort");
    // Une version rangée du plan suivant se rouvre : c'est la version entière d'avant la coupe.
    let numero = b.app.doc.versions_du_plan(suite).unwrap().autres[0].numero;
    assert!(b.app.doc.changer_de_version(1, numero));
    assert_eq!(b.app.doc.panels[1].frame_count(), 48);
    assert!(b.app.doc.changer_de_version(1, 2));
    // Annuler / rétablir : une seule étape pour la coupe (le passage de version fait par la ligne du dessus n'est pas dans l'historique).
    let (mut b, plan) = banc_a_versions();
    couper(&mut b, 10);
    assert!(b.app.coupe_versions_repondre(Cote::Seconde));
    let apres = ids_film(&b.app.doc);
    b.app.run_action("edition.annuler");
    assert_eq!(ids_film(&b.app.doc), ids);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2);
    assert_eq!(rangees(&b.app.doc, plan), versions);
    b.app.run_action("edition.retablir");
    assert_eq!(ids_film(&b.app.doc), apres);
    assert_eq!(rangees(&b.app.doc, apres[1]), versions);
    // Enregistré puis rouvert : les versions sont à la seconde partie, dessins compris.
    let racine = dossier("coupe-versions-seconde");
    let m = manifeste(&racine);
    enregistrer(&b.app.doc, &m);
    let Loaded { doc: relu, warnings, .. } = project::load(&m).expect("réouverture");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(ids_film(&relu), apres);
    assert_eq!((relu.nombre_de_versions(apres[0]), relu.nombre_de_versions(apres[1])), (1, 2));
    assert_eq!(rangees(&relu, apres[1]).0, versions.0);
}

#[test]
fn a_sequence_version_showing_a_stored_plan_version_follows_it_to_the_second_part() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    assert_eq!(doc.nouvelle_version(1, 1), Some(2));
    retoucher(&mut doc, 1, 150);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 2), Some(2));
    // La version de séquence A montre la V1 du plan 2 (version rangée), la B (celle du film) la V2.
    assert!(doc.choisir_version_dans_variante(seq, 1, ids[1], 1));
    assert!(avec_cote(Cote::Seconde, || doc.split_plan(1, 10)).is_some());
    let suite = doc.panels[2].id;
    let a = doc.variantes_de(seq).unwrap().rangee(1).unwrap();
    assert!(a.contenu.slots.iter().any(|e| e.plan == suite && e.version == 1), "la A montre maintenant la V1 de la seconde partie");
    assert!(doc.plan_montre(suite, 1).is_some());
    for (seq_plan, version) in a.contenu.slots.iter().map(|e| (e.plan, e.version)) {
        assert!(doc.versions_connues(seq_plan).contains(&version), "{seq_plan} {version}");
    }
    // Passer à la A : la version entière est montrée, rien ne manque.
    assert!(doc.changer_de_variante(seq, 1));
    assert!(doc.plans_de_sequence(seq).contains(&suite));
    assert!(doc.versions.keys().all(|p| doc.panels.iter().any(|q| q.id == *p) || doc.variantes_de(seq).is_some_and(|v| v.garage.contains_key(p))));
}

#[test]
fn a_cut_that_cannot_ask_keeps_the_versions_with_the_first_part() {
    // Le mouvement de caméra posé sur la première case sépare les cases suivantes : pas de fenêtre, les versions restent au premier plan.
    let mut doc = film(2);
    let plan = doc.panels[0].id;
    assert_eq!(doc.nouvelle_version(0, 1), Some(2));
    let id = doc.alloc();
    doc.panels[0].marks.push(ImageMark { id, frame: 20, ..Default::default() });
    let versions = rangees(&doc, plan);
    let suite = doc.scinder_apres_mouvement(0, &[], Vec::new());
    assert!(suite.is_some(), "les cases suivantes sont séparées, versions ou non");
    assert_eq!(doc.panels.len(), 3);
    assert_eq!(doc.nombre_de_versions(plan), 2);
    assert_eq!(rangees(&doc, plan), versions);
    assert_eq!(doc.nombre_de_versions(doc.panels[1].id), 1);
}

#[test]
fn a_question_left_open_while_the_plan_or_the_project_changes_cancels_cleanly() {
    // Annuler : rien ne change.
    let (mut b, plan) = banc_a_versions();
    let ids = ids_film(&b.app.doc);
    couper(&mut b, 10);
    b.app.coupe_versions_annuler();
    assert!(b.app.coupe_versions.is_none() && !b.app.modal_open());
    assert_eq!(ids_film(&b.app.doc), ids);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2);
    // Un autre plan devient courant pendant la question : la réponse ne coupe rien.
    couper(&mut b, 10);
    b.app.cur_panel = 1;
    assert!(!b.app.coupe_versions_repondre(Cote::Seconde));
    assert_eq!(ids_film(&b.app.doc), ids);
    assert!(b.app.coupe_versions.is_none(), "la question est close");
    // Même chose quand l'image courante change.
    b.app.cur_panel = 0;
    couper(&mut b, 10);
    b.app.cur_frame = 20;
    assert!(!b.app.coupe_versions_repondre(Cote::Premiere));
    assert_eq!(ids_film(&b.app.doc), ids);
    // Un autre projet (époque) : la question tombe sans rien faire.
    b.app.cur_frame = 0;
    couper(&mut b, 10);
    b.app.doc_epoch += 1;
    assert!(!b.app.coupe_versions_repondre(Cote::Seconde));
    assert_eq!(ids_film(&b.app.doc), ids);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2);
    // La fenêtre, à l'écran : le plan courant change, la fenêtre se ferme d'elle-même.
    b.app.doc_epoch -= 1;
    b.app.cur_panel = 0;
    couper(&mut b, 10);
    assert!(b.app.coupe_versions.is_some());
    b.app.cur_panel = 1;
    b.chauffer(3);
    assert!(b.app.coupe_versions.is_none(), "{}", b.app.status);
    assert_eq!(ids_film(&b.app.doc), ids);
}

#[test]
fn the_bar_between_thumbnails_and_a_dropped_image_ask_the_same_question() {
    // La barre entre deux vignettes d'un même plan (une marque d'image à l'image 20).
    let (mut b, plan) = banc_a_versions();
    let id = b.app.doc.alloc();
    b.app.doc.panels[0].marks.push(ImageMark { id, frame: 20, ..Default::default() });
    let versions = rangees(&b.app.doc, plan);
    let layout = b.app.board_layout();
    assert!(layout.cards.len() >= 2 && layout.cards[0].plan == plan && layout.cards[1].plan == plan);
    assert!(!b.app.board_toggle_cut(&layout, 0), "la question attend : rien n'a changé");
    assert!(b.app.coupe_versions.is_some());
    assert_eq!(b.app.doc.panels.len(), 2);
    assert!(b.app.coupe_versions_repondre(Cote::Seconde));
    assert_eq!(b.app.doc.panels.len(), 3);
    assert_eq!((b.app.doc.nombre_de_versions(plan), b.app.doc.nombre_de_versions(b.app.doc.panels[1].id)), (1, 2));
    assert_eq!(rangees(&b.app.doc, b.app.doc.panels[1].id), versions);
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.panels.len(), b.app.doc.nombre_de_versions(plan)), (2, 2));

    // Une image déposée entre deux images de ce plan : elle coupe, donc la question vient d'abord ; les images n'ont rien fait.
    let (mut b, plan) = banc_a_versions();
    let id = b.app.doc.alloc();
    b.app.doc.panels[0].marks.push(ImageMark { id, frame: 20, ..Default::default() });
    let versions = rangees(&b.app.doc, plan);
    let layout = b.app.board_layout();
    let carte = layout.cards[0];
    assert_eq!(carte.plan, plan);
    assert!(carte.end < b.app.doc.panels[0].frame_count());
    let (w, h) = (b.app.doc.width as usize, b.app.doc.height as usize);
    b.app.depot_appliquer(Cible::Entre(carte.id), vec![Raster::new(w, h)]);
    assert!(matches!(b.app.coupe_versions.as_ref().map(|q| &q.refaire), Some(Refaire::Depot { .. })), "la question est posée");
    assert_eq!(b.app.doc.panels.len(), 2, "aucune image posée, aucun plan coupé en attendant");
    assert!(b.app.coupe_versions_repondre(Cote::Premiere));
    assert!(b.app.doc.panels.len() >= 4, "le plan est coupé et l'image a son plan : {}", b.app.doc.panels.len());
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2);
    assert_eq!(rangees(&b.app.doc, plan), versions);
    assert!(b.app.doc.versions.keys().all(|p| b.app.doc.panels.iter().any(|q| q.id == *p)));
}
