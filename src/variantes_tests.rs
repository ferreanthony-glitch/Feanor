//! Variantes de séquence (`variantes.rs`, `variantesui.rs`) : créer (en dupliquant, vide), passer de l'une à l'autre avec
//! annulation, la variante choisie est celle du film, les variantes rangées survivent à l'enregistrement et au partage,
//! supprimer, renommer, copier entre variantes, envois, comparaison, jeton du montage, repos.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::Color32;

use crate::camera::set_key;
use crate::comparer::Source;
use crate::cooperation::{Delais, Horloge, Session};
use crate::cooperation_tests::{confirmer, film as film_partage, partager};
use crate::depot::DossierPartage;
use crate::document::{Cel, CelId, Document, ImageMark, Layer, LayerKind, Panel};
use crate::empreinte::{self, Donnees};
use crate::envois::{self, Envoi, Etat, PlanEnvoye};
use crate::equipe::Identite;
use crate::fiabilite_fixture::dossier;
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::project::{self, Loaded};
use crate::transitions::Kind;
use crate::variantes::{etiquette, lettre, Nouvelle, MAX_VARIANTES};
use crate::verrous::Cible;

fn rouge(r: u8) -> Color32 {
    Color32::from_rgba_premultiplied(r, 10, 10, 255)
}

/// Un film de `n` plans de 48 images dans une seule séquence : un calque bitmap, un dessin par plan.
pub(crate) fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    for k in 1..n {
        doc.add_scene_after(k - 1);
    }
    for i in 0..n {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(0)[0] = rouge((20 + i * 9) as u8);
        let id = doc.add_cel(cel);
        layer.frames = vec![Some(id); 48];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 48;
    }
    doc
}

/// Ce qu'est un plan, pixels compris, sans les numéros : deux plans qui se ressemblent ont la même.
pub(crate) fn contenu(doc: &Document, p: &Panel) -> u64 {
    let cle = |id: CelId| doc.cels.get(&id).map_or(id, |c| Donnees::de(c).empreinte());
    empreinte::de_octets(&project::digest_bytes(p, &cle))
}

pub(crate) fn contenus_film(doc: &Document, seq: u64) -> Vec<u64> {
    doc.plans_de_sequence(seq).iter().map(|id| contenu(doc, doc.panels.iter().find(|p| p.id == *id).expect("plan"))).collect()
}

/// Le contenu des plans que montre la version de séquence rangée `n`, chacun sous la version qu'elle en montre.
pub(crate) fn contenus_range(doc: &Document, seq: u64, n: u32) -> Vec<u64> {
    let r = doc.variantes_de(seq).and_then(|v| v.rangee(n)).expect("version de séquence rangée");
    r.contenu.slots.iter().map(|e| contenu(doc, doc.plan_montre(e.plan, e.version).expect("plan montré"))).collect()
}

/// Les numéros de plans de la version de séquence rangée `n`.
pub(crate) fn plans_range(doc: &Document, seq: u64, n: u32) -> Vec<u64> {
    doc.variantes_de(seq).and_then(|v| v.rangee(n)).expect("version de séquence rangée").contenu.slots.iter().map(|e| e.plan).collect()
}

/// Un film de `n` plans fixes : un dessin tenu, une durée voulue de 48 images (la durée se règle sans toucher aux dessins).
pub(crate) fn film_fixe(n: usize) -> Document {
    let mut doc = film(n);
    for p in &mut doc.panels {
        for l in &mut p.layers {
            l.frames.truncate(1);
        }
        p.length = 48;
    }
    doc
}

pub(crate) fn ids_film(doc: &Document) -> Vec<u64> {
    doc.panels.iter().map(|p| p.id).collect()
}

pub(crate) fn retoucher(doc: &mut Document, i: usize, r: u8) {
    let id = doc.panels[i].layers[0].frames[0].unwrap();
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(7)[3] = rouge(r);
    cel.version += 1;
}

pub(crate) fn manifeste(dir: &Path) -> PathBuf {
    dir.join("Film").join("Film.pelure")
}

pub(crate) fn enregistrer(doc: &Document, manifeste: &Path) {
    project::prepare_save(doc, manifeste, &mut HashMap::new(), None).expect("préparation").run().expect("écriture");
}

pub(crate) fn banc_projet(n: usize) -> Banc {
    let mut b = banc(film(n), false);
    b.app.modified = false;
    b
}

/// Un film riche : marques d'image, clés de caméra, textes, une version de plan, un commentaire, une transition entre deux plans.
pub(crate) fn film_riche() -> Document {
    let mut doc = film(3);
    let id = doc.alloc();
    doc.panels[0].marks.push(ImageMark { id, frame: 5, color: 2, action: "Elle entre.".into(), ..ImageMark::default() });
    let cam = doc.default_camera();
    set_key(&mut doc.panels[0].camera, 6, cam);
    doc.panels[0].action = "Plan large.".into();
    doc.panels[1].notes = "Garder le ciel.".into();
    doc.nouvelle_version(1, 5);
    retoucher(&mut doc, 1, 150);
    let plan = doc.panels[2].id;
    doc.add_comment(plan, 3, "Anthony", 7);
    assert!(doc.set_transition(1, Kind::Dissolve).is_some());
    doc
}

// ------------------------------------------------------------------------------------ nomenclature

#[test]
fn variants_are_named_by_letter_and_an_optional_short_name() {
    assert_eq!((lettre(1).as_str(), lettre(2).as_str(), lettre(26).as_str()), ("A", "B", "Z"));
    assert_eq!(etiquette(2, ""), "B");
    assert_eq!(etiquette(2, "  serré "), "B · serré");
    assert_eq!(crate::variantes::nom_propre("  une   idée \n de plus "), "une idée de plus");
    assert_eq!(crate::variantes::nom_propre(&"x".repeat(100)).chars().count(), crate::variantes::MAX_NOM);
}

// ------------------------------------------------------------------------------------ créer

#[test]
fn a_new_variant_by_duplication_keeps_the_film_and_stores_a_deep_copy() {
    let mut doc = film_riche();
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    let avant = contenus_film(&doc, seq);
    let cels_avant = doc.cels.len();
    assert_eq!((doc.numero_de_variante(seq), doc.nombre_de_variantes(seq)), (1, 1));
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Copier, 1_760_000_000), Some(2));
    assert_eq!((doc.numero_de_variante(seq), doc.nombre_de_variantes(seq)), (2, 2));
    // Le film ne bouge pas : mêmes plans, mêmes numéros, même contenu.
    assert_eq!(ids_film(&doc), ids);
    assert_eq!(contenus_film(&doc, seq), avant);
    // La version rangée est la A, une copie profonde : numéros neufs, dessins à elle, même contenu (ses plans sont au garage).
    let v = doc.variantes_de(seq).unwrap().clone();
    let r = v.rangee(1).unwrap().clone();
    assert_eq!(r.contenu.slots.len(), 3);
    let copies: Vec<u64> = r.contenu.slots.iter().map(|e| e.plan).collect();
    assert!(copies.iter().all(|c| !ids.contains(c)), "numéros de plans neufs");
    assert!(copies.iter().all(|c| v.garage.contains_key(c)), "ses plans sont au garage");
    assert_eq!(v.garage.len(), 3);
    assert!(r.contenu.scenes.iter().all(|s| doc.scene(s.id).is_none()), "scènes neuves");
    assert_eq!(contenus_range(&doc, seq, 1), avant);
    let film_cels: Vec<CelId> = doc.panels.iter().flat_map(|p| p.layers.iter().flat_map(|l| l.frames.iter().flatten().copied())).collect();
    assert!(v.garage.values().flat_map(|g| g.panel.layers.iter().flat_map(|l| l.frames.iter().flatten())).all(|c| !film_cels.contains(c)), "dessins à elle");
    assert_eq!(doc.cels.len(), cels_avant + 3 + 1, "un dessin par plan, plus celui de la version rangée");
    // Marques, caméra, textes, version du plan, commentaire, transition : copiés.
    let g0 = &v.garage[&copies[0]].panel;
    assert_eq!(g0.marks.len(), 1);
    assert_ne!(g0.marks[0].id, doc.panels[0].marks[0].id);
    assert_eq!(g0.camera.len(), 1);
    assert_eq!(g0.action, "Plan large.");
    assert_eq!(v.garage[&copies[1]].versions.as_ref().map(|v| v.nombre()), Some(2));
    assert_eq!(r.contenu.slots[1].version, 2, "elle montre la version active de la copie");
    assert_eq!(v.garage[&copies[2]].comments.len(), 1);
    assert_eq!(v.garage[&copies[2]].comments[0].panel, copies[2]);
    assert_eq!(r.contenu.transitions.len(), 1);
    assert_eq!((r.contenu.transitions[0].out, r.contenu.transitions[0].inn), (Some(copies[0]), Some(copies[1])));
    // Elle se souvient d'où viennent ses plans.
    assert_eq!(doc.origine_du_plan(copies[1]), Some(ids[1]));
    // Retoucher le film ne touche pas la copie.
    retoucher(&mut doc, 0, 220);
    assert_eq!(contenus_range(&doc, seq, 1), avant);
    // Aucun numéro en double, nulle part.
    let mut tous: Vec<u64> = doc.panels.iter().map(|p| p.id).chain(doc.plans_ranges().map(|p| p.id)).collect();
    let n = tous.len();
    tous.sort_unstable();
    tous.dedup();
    assert_eq!(tous.len(), n);
}

#[test]
fn a_new_empty_variant_stores_the_plans_as_they_are_and_restarts_from_one_empty_plan() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    let ids = ids_film(&doc);
    let avant = contenus_film(&doc, seq);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 5), Some(2));
    assert_eq!(doc.plans_de_sequence(seq).len(), 1);
    assert!(!ids.contains(&doc.panels[0].id), "un plan neuf");
    assert!(doc.panels[0].layers.iter().all(|l| l.frames.iter().flatten().count() == 0), "vide");
    assert!(doc.total_frames() >= 1);
    assert_eq!(plans_range(&doc, seq, 1), ids, "le jeu en cours est rangé tel quel");
    assert!(ids.iter().all(|id| doc.variantes_de(seq).unwrap().garage.contains_key(id)), "ses plans sont au garage");
    assert_eq!(contenus_range(&doc, seq, 1), avant);
}

#[test]
fn a_sequence_has_at_most_twenty_six_variants_and_a_missing_sequence_has_none() {
    let mut doc = film(1);
    let seq = doc.sequence_of(0);
    for k in 2..=MAX_VARIANTES as u32 {
        assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(k));
    }
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), None);
    assert_eq!(doc.nombre_de_variantes(seq), MAX_VARIANTES);
    assert_eq!(doc.nouvelle_variante(9_999, Nouvelle::Copier, 1), None);
}

// ------------------------------------------------------------------------------------ passer de l'une à l'autre

#[test]
fn switching_exchanges_the_plan_sets_and_the_chosen_variant_drives_the_film() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    // Une deuxième séquence, avant et après : leur place ne change pas.
    doc.add_sequence_after(2);
    let apres = doc.panels[3].id;
    let a = contenus_film(&doc, seq);
    let duree_a = doc.total_frames();
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 1), Some(2));
    // Variante B : un seul plan vide.
    let b = contenus_film(&doc, seq);
    assert_eq!(doc.panels.len(), 2);
    assert!(doc.total_frames() < duree_a, "le montage suit la variante choisie");
    assert_eq!(doc.panels[1].id, apres);
    assert_eq!(doc.sequence_of(0), seq);
    // Retour à la A, puis à la B : le contenu est exactement celui de chacune.
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!((doc.numero_de_variante(seq), doc.panels.len(), doc.total_frames()), (1, 4, duree_a));
    assert_eq!(contenus_film(&doc, seq), a);
    assert_eq!(doc.panels[3].id, apres);
    assert_eq!(contenus_range(&doc, seq, 2), b);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(contenus_film(&doc, seq), b);
    assert_eq!(contenus_range(&doc, seq, 1), a);
    // Un numéro qu'on n'a pas, ou la variante choisie : rien ne se passe.
    assert!(!doc.changer_de_variante(seq, 9) && !doc.changer_de_variante(seq, 2));
    // Les numéros de plan et de séquence du film suivent la variante choisie.
    assert_eq!(doc.sequence_ids().iter().filter(|s| **s == seq).count(), 1);
    assert_eq!(doc.plan_code(doc.panels[1].scene), "2");
}

#[test]
fn plan_data_travels_with_its_variant_and_the_edge_transitions_follow_the_cut() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    doc.add_sequence_after(1);
    let apres = doc.panels[2].id;
    let (p0, p1) = (doc.panels[0].id, doc.panels[1].id);
    doc.add_comment(p0, 2, "Léa", 3);
    assert!(doc.set_transition(1, Kind::Dissolve).is_some(), "à l'intérieur de la séquence");
    assert!(doc.set_transition(2, Kind::Black).is_some(), "à la coupe avec la séquence suivante");
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 1), Some(2));
    let neuf = doc.panels[0].id;
    // La transition interne est restée avec la variante A ; celle de la coupe suit le nouveau dernier plan.
    assert!(doc.comments.is_empty() && doc.transitions.len() == 1, "{} {}", doc.comments.len(), doc.transitions.len());
    assert_eq!((doc.transitions[0].out, doc.transitions[0].inn), (Some(neuf), Some(apres)));
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(doc.comments.len(), 1);
    assert_eq!(doc.comments[0].panel, p0);
    assert_eq!(doc.transitions.len(), 2);
    assert!(doc.transitions.iter().any(|t| t.out == Some(p0) && t.inn == Some(p1)));
    assert!(doc.transitions.iter().any(|t| t.out == Some(p1) && t.inn == Some(apres)));
}

#[test]
fn the_plan_versions_of_a_stored_variant_come_back_with_it() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    let plan = doc.panels[0].id;
    doc.nouvelle_version(0, 3);
    retoucher(&mut doc, 0, 99);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 1), Some(2));
    assert_eq!(doc.nombre_de_versions(plan), 1, "les versions du plan sont rangées avec lui");
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!((doc.numero_de_version(plan), doc.nombre_de_versions(plan)), (2, 2));
}

#[test]
fn locked_numbering_stays_unique_when_a_variant_gets_a_plan_and_comes_back() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    doc.lock_numbering();
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Partager, 1), Some(2));
    let numeros = |d: &Document| d.scene_order().iter().map(|s| d.plan_code(*s)).collect::<Vec<_>>();
    assert_eq!(numeros(&doc), ["1", "2", "3"], "la copie n'a rien changé au film");
    // La variante B gagne un plan, inséré entre le 1 et le 2.
    doc.add_scene_after(0);
    assert_eq!(numeros(&doc), ["1", "1A", "2", "3"]);
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!(numeros(&doc), ["1", "2", "3"]);
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(numeros(&doc), ["1", "1A", "2", "3"]);
}

// ------------------------------------------------------------------------------------ supprimer, renommer

#[test]
fn deleting_a_variant_keeps_the_last_one_and_deleting_the_chosen_one_chooses_another() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    let a = contenus_film(&doc, seq);
    assert_eq!(doc.supprimer_variante(seq, 1), None, "une seule variante");
    doc.nouvelle_variante(seq, Nouvelle::Vide, 1);
    doc.nouvelle_variante(seq, Nouvelle::Vide, 2);
    assert_eq!(doc.variantes_de(seq).unwrap().numeros(), [1, 2, 3]);
    // Supprimer une variante rangée.
    assert_eq!(doc.supprimer_variante(seq, 2), Some(3));
    assert_eq!(doc.variantes_de(seq).unwrap().numeros(), [1, 3]);
    // Supprimer la variante choisie : la séquence passe à la plus petite des autres.
    assert_eq!(doc.supprimer_variante(seq, 3), Some(1));
    assert_eq!(contenus_film(&doc, seq), a);
    assert!(doc.variantes_de(seq).is_none(), "plus de variantes : retour à l'état d'un projet d'avant");
    // La lettre libérée est reprise.
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 3), Some(2));
    assert_eq!(doc.supprimer_variante(seq, 7), None);
}

#[test]
fn renaming_a_variant_keeps_it_and_cleans_the_name() {
    let mut doc = film(1);
    let seq = doc.sequence_of(0);
    assert!(!doc.renommer_variante(seq, 1, "x"), "pas de variantes : rien à nommer");
    doc.nouvelle_variante(seq, Nouvelle::Vide, 1);
    assert!(doc.renommer_variante(seq, 2, "  avec  la poursuite "));
    assert!(doc.renommer_variante(seq, 1, "serré"));
    assert_eq!((doc.etiquette_de_variante(seq, 2).as_str(), doc.etiquette_de_variante(seq, 1).as_str()), ("B · avec la poursuite", "A · serré"));
    assert!(!doc.renommer_variante(seq, 1, "serré"), "rien ne change");
    // Le nom suit la variante quand on passe de l'une à l'autre.
    assert!(doc.changer_de_variante(seq, 1));
    assert_eq!((doc.etiquette_de_variante(seq, 1).as_str(), doc.etiquette_de_variante(seq, 2).as_str()), ("A · serré", "B · avec la poursuite"));
    assert!(!doc.renommer_variante(seq, 9, "x"));
}

// ------------------------------------------------------------------------------------ copier entre variantes

#[test]
fn plans_copied_in_one_variant_can_be_pasted_in_another() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    let id = doc.panels[1].id;
    let clip = doc.copy_panels(&[id]).expect("copie");
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Vide, 1), Some(2));
    assert_eq!(doc.plans_de_sequence(seq).len(), 1);
    assert_eq!(doc.paste_plans(&clip, 0), Some(1));
    assert_eq!(doc.plans_de_sequence(seq).len(), 2);
    assert_eq!(contenu(&doc, &doc.panels[1]), contenus_range(&doc, seq, 1)[1]);
}

#[test]
fn the_clipboard_commands_work_across_variants() {
    let mut b = banc_projet(3);
    let seq = b.app.doc.sequence_of(0);
    // (Le plan courant fait toujours partie des plans choisis.)
    b.app.cur_panel = 1;
    b.app.board_sel = vec![b.app.doc.panels[1].id];
    b.app.run_action("panneau.copier");
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Vide);
    assert_eq!(b.app.doc.plans_de_sequence(seq).len(), 1, "{}", b.app.status);
    b.app.run_action("panneau.coller");
    assert_eq!(b.app.doc.plans_de_sequence(seq).len(), 2, "{}", b.app.status);
}

// ------------------------------------------------------------------------------------ annuler

#[test]
fn every_variant_gesture_is_one_undo_step_and_the_plan_data_comes_back() {
    let mut b = banc_projet(3);
    let seq = b.app.doc.sequence_of(0);
    let ids = ids_film(&b.app.doc);
    let a = contenus_film(&b.app.doc, seq);
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Copier);
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2, "{}", b.app.status);
    assert!(b.app.modified);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 1, "annuler défait la création");
    assert_eq!(ids_film(&b.app.doc), ids);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2);
    // Passer à la A, annuler, rétablir.
    b.app.passer_a_la_variante(seq, 1);
    let copies = ids_film(&b.app.doc);
    assert_ne!(copies, ids);
    assert_eq!(contenus_film(&b.app.doc, seq), a);
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.numero_de_variante(seq), ids_film(&b.app.doc)), (2, ids.clone()));
    b.app.run_action("edition.retablir");
    assert_eq!((b.app.doc.numero_de_variante(seq), ids_film(&b.app.doc)), (1, copies));
    // Renommer, supprimer : une étape chacun.
    b.app.renommer_la_variante(seq, 1, "serré");
    assert_eq!(b.app.doc.etiquette_de_variante(seq, 1), "A · serré");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.etiquette_de_variante(seq, 1), "A");
    b.app.supprimer_la_variante(seq, 2);
    assert!(b.app.doc.variantes_de(seq).is_none());
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2);
    assert_eq!(contenus_range(&b.app.doc, seq, 2), a);
}

#[test]
fn a_locked_storyboard_refuses_every_variant_gesture() {
    let mut b = banc_projet(2);
    let seq = b.app.doc.sequence_of(0);
    b.app.doc.nouvelle_variante(seq, Nouvelle::Vide, 1);
    b.app.doc.storyboard_locked = true;
    b.app.passer_a_la_variante(seq, 1);
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Copier);
    b.app.supprimer_variante_demander(seq, 1);
    b.app.renommer_la_variante(seq, 1, "x");
    assert_eq!((b.app.doc.numero_de_variante(seq), b.app.doc.nombre_de_variantes(seq)), (2, 2));
    assert!(b.app.variantes_ui.supprimer.is_none());
    assert!(b.app.status.contains("verrouillé"), "{}", b.app.status);
}

#[test]
fn the_commands_reach_the_variants_of_the_current_sequence() {
    let mut b = banc_projet(2);
    for id in ["variante.nouvelle", "variante.copie", "variante.vide", "variante.precedente", "variante.suivante", "variante.mettre_a_jour", "variante.renommer", "variante.supprimer", "variante.comparer"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    let seq = b.app.doc.sequence_of(0);
    b.app.run_action("variante.precedente");
    assert!(b.app.status.contains("qu'une version"), "{}", b.app.status);
    b.app.run_action("variante.vide");
    assert_eq!(b.app.doc.numero_de_variante(seq), 2);
    b.app.run_action("variante.precedente");
    assert_eq!(b.app.doc.numero_de_variante(seq), 1);
    b.app.run_action("variante.suivante");
    assert_eq!(b.app.doc.numero_de_variante(seq), 2);
    b.app.run_action("variante.renommer");
    assert!(matches!(b.app.variantes_ui.renommer, Some((s, 2, _, _)) if s == seq));
    b.app.variantes_ui.renommer = None;
    b.app.run_action("variante.supprimer");
    assert_eq!(b.app.variantes_ui.supprimer, Some((seq, 2)));
    b.app.run_action("variante.comparer");
    assert!(b.app.comparaison.is_some());
}

// ------------------------------------------------------------------------------------ enregistrer et rouvrir

#[test]
fn stored_variants_survive_a_save_and_a_reopening_down_to_the_byte() {
    let mut doc = film_riche();
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1_760_000_000);
    doc.renommer_variante(seq, 1, "serré");
    doc.nouvelle_variante(seq, Nouvelle::Vide, 1_760_000_100);
    let racine = dossier("variantes-fichier");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    assert!(m.parent().unwrap().join("variantes.json").exists());
    let Loaded { doc: relu, warnings, .. } = project::load(&m).expect("réouverture");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(relu.variantes_de(seq).unwrap().numeros(), [1, 2, 3]);
    assert_eq!((relu.numero_de_variante(seq), relu.etiquette_de_variante(seq, 1).as_str()), (3, "A · serré"));
    assert_eq!(contenus_film(&relu, seq), contenus_film(&doc, seq));
    for n in [1, 2] {
        assert_eq!(contenus_range(&relu, seq, n), contenus_range(&doc, seq, n), "variante {n}");
    }
    assert_eq!(plans_range(&relu, seq, 2), plans_range(&doc, seq, 2));
    // Versions de plan, commentaires, transitions, origines : revenus avec leur version de séquence.
    let (v0, v1) = (doc.variantes_de(seq).unwrap(), relu.variantes_de(seq).unwrap());
    assert_eq!(v1.garage.keys().collect::<Vec<_>>(), v0.garage.keys().collect::<Vec<_>>());
    assert_eq!(v1.garage.values().map(|g| g.versions.as_ref().map_or(0, |v| v.nombre())).sum::<usize>(), 4, "deux plans à deux versions : la copie et l'original");
    assert_eq!((v1.garage.values().map(|g| g.comments.len()).sum::<usize>(), v1.rangee(2).unwrap().contenu.transitions.len()), (2, 1), "le commentaire de chaque copie et de chaque original rangé");
    assert_eq!(relu.variantes_de(seq).unwrap().origines, doc.variantes_de(seq).unwrap().origines);
    // Les dessins des variantes rangées sont des dessins du projet.
    assert!(relu.used_cels().len() >= doc.used_cels().len());
    // Réécrit, le fichier est le même, octet pour octet.
    assert_eq!(project::variantes_octets(&relu).unwrap(), project::variantes_octets(&doc).unwrap());
    // Les numéros neufs ne retombent pas sur ceux d'une variante rangée.
    let mut relu = relu;
    let neufs: Vec<u64> = (0..3).map(|_| relu.alloc()).collect();
    let pris: Vec<u64> = relu.ids_des_variantes();
    assert!(neufs.iter().all(|n| !pris.contains(n)));
}

#[test]
fn deleting_a_variant_frees_its_drawings_at_the_next_save_and_no_variant_means_no_file() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    let racine = dossier("variantes-menage");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    let dessins = |m: &Path| std::fs::read_dir(m.parent().unwrap().join("dessins")).unwrap().count();
    let avec = dessins(&m);
    doc.supprimer_variante(seq, 1);
    enregistrer(&doc, &m);
    assert!(dessins(&m) < avec);
    assert!(!m.parent().unwrap().join("variantes.json").exists());
}

#[test]
fn an_unreadable_or_newer_variants_file_is_never_erased_and_keeps_the_drawings() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    let racine = dossier("variantes-illisible");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    let fichier = m.parent().unwrap().join("variantes.json");
    let texte = std::fs::read_to_string(&fichier).unwrap().replace("\"version\": 2", "\"version\": 99");
    std::fs::write(&fichier, &texte).unwrap();
    let Loaded { doc: relu, warnings, .. } = project::load(&m).expect("s'ouvre quand même");
    assert!(relu.variantes_de(seq).is_none());
    assert!(warnings.iter().any(|w| w.contains("variantes.json")), "{warnings:?}");
    let avant = std::fs::read_dir(m.parent().unwrap().join("dessins")).unwrap().count();
    enregistrer(&relu, &m);
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), texte, "le fichier reste là, intact");
    assert_eq!(std::fs::read_dir(m.parent().unwrap().join("dessins")).unwrap().count(), avant, "aucun dessin effacé");
}

#[test]
fn a_project_without_variants_has_no_file_and_the_format_does_not_change() {
    let doc = film(2);
    let racine = dossier("variantes-aucune");
    let m = manifeste(&racine);
    enregistrer(&doc, &m);
    assert!(!m.parent().unwrap().join("variantes.json").exists());
    assert!(project::variantes_octets(&doc).unwrap().is_none());
    // (Les variantes n'ont pas de numéro de format à elles : la 36 est celle d'avant leur chantier, la 37 celle de la caméra par paliers.)
    assert!(project::VERSION >= 36, "le format du projet est celui d'avant : la variante est une annexe");
}

// ------------------------------------------------------------------------------------ envois

fn envoi_de(doc: &Document, seq: u64) -> Envoi {
    let plans = doc
        .plans_de_sequence(seq)
        .iter()
        .map(|id| {
            let p = doc.panels.iter().find(|p| p.id == *id).unwrap();
            PlanEnvoye { plan: *id, titre: "Plan".into(), empreinte: contenu(doc, p), version: 1, images: p.frame_count(), nom: String::new(), sequence: String::new() }
        })
        .collect();
    Envoi { id: 1, nom: "v1 au réal".into(), date: 1, note_le: 1, fichier: String::new(), plans, variantes: doc.variantes_pour_envoi() }
}

#[test]
fn a_copied_plan_is_compared_with_the_plan_it_comes_from_for_the_envoi() {
    let mut doc = film(3);
    let seq = doc.sequence_of(0);
    let envoi = envoi_de(&doc, seq);
    assert!(envoi.variantes.is_empty(), "une séquence à une variante ne note rien");
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    // La variante A (la copie) devient celle du film : ses plans ont des numéros neufs, mais sont comme à l'envoi.
    assert!(doc.changer_de_variante(seq, 1));
    let d = |doc: &Document, i: usize| contenu(doc, &doc.panels[i]);
    for i in 0..3 {
        let id = doc.panels[i].id;
        assert_eq!(envoi.plan(id), None, "l'envoi ne connaît pas ce numéro");
        assert_eq!(envois::etat_du_plan_dans(&doc, &envoi, id, d(&doc, i)), Etat::Inchange, "plan {i}");
    }
    // Retouché : modifié depuis l'envoi. Sans origine : nouveau.
    retoucher(&mut doc, 1, 200);
    let id = doc.panels[1].id;
    assert_eq!(envois::etat_du_plan_dans(&doc, &envoi, id, d(&doc, 1)), Etat::Modifie);
    assert_eq!(envois::etat_du_plan_dans(&doc, &envoi, 424_242, 0), Etat::Nouveau);
    // Et repassé à la B (les plans d'origine, retouchés ou non) : ceux de l'envoi.
    assert!(doc.changer_de_variante(seq, 2));
    assert_eq!(envois::etat_du_plan_dans(&doc, &envoi, doc.panels[0].id, d(&doc, 0)), Etat::Inchange);
}

#[test]
fn an_envoi_records_the_chosen_variants_and_the_file_round_trips() {
    let mut doc = film(2);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Vide, 1);
    doc.renommer_variante(seq, 2, "serré");
    let envoi = envoi_de(&doc, seq);
    assert_eq!(envoi.variantes, vec![envois::VarianteEnvoyee { sequence: seq, numero: 2, nom: "serré".into() }]);
    let texte = serde_json::to_string(&envoi).unwrap();
    assert!(texte.contains("variantes"));
    let relu: Envoi = serde_json::from_str(&texte).unwrap();
    assert_eq!(relu, envoi);
    // Un envoi d'avant n'a pas le champ, et le sien ne l'écrit pas non plus.
    let ancien: Envoi = serde_json::from_str(r#"{"id":1,"nom":"v1","date":1,"plans":[]}"#).unwrap();
    assert!(ancien.variantes.is_empty());
    assert!(!serde_json::to_string(&ancien).unwrap().contains("variantes"));
}

#[test]
fn marking_an_envoi_in_the_application_notes_the_variants_and_the_changed_plans_follow() {
    let mut b = banc_projet(2);
    let seq = b.app.doc.sequence_of(0);
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Copier);
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1 au réal", 1_760_000_000);
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.05);
        if !b.app.doc.envois.list.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let e = b.app.doc.envois.dernier().expect("envoi marqué").clone();
    assert_eq!(e.variantes.len(), 1);
    assert_eq!(e.variantes[0].numero, 2);
    // Passer à la variante A (une copie) : ses plans sont comme à l'envoi, rien n'est signalé.
    b.app.passer_a_la_variante(seq, 1);
    for _ in 0..200 {
        b.image(Vec::new(), 0.05);
    }
    assert!(b.app.plans_modifies().is_empty(), "{:?}", b.app.plans_modifies());
    // Retouche d'un plan : seul celui-là est signalé.
    retoucher(&mut b.app.doc, 1, 180);
    b.app.modified = true;
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.05);
        if !b.app.plans_modifies().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(b.app.plans_modifies(), vec![1]);
}

// ------------------------------------------------------------------------------------ comparer

#[test]
fn compare_offers_the_plan_of_the_same_rank_in_each_other_variant() {
    let mut b = banc_projet(3);
    let seq = b.app.doc.sequence_of(0);
    b.app.doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    b.app.doc.nouvelle_variante(seq, Nouvelle::Vide, 2);
    // Variante C (vide, un plan) : A et B ont des plans de rang 0, 1 et 2.
    b.app.cur_panel = 0;
    let s0 = b.app.comparer_sources(0);
    assert!(s0.contains(&Source::Variante(1)) && s0.contains(&Source::Variante(2)), "{s0:?}");
    assert_eq!(b.app.comparer_libelle(b.app.doc.panels[0].id, &Source::Variante(2)), "Version de séquence B");
    // Le film n'a qu'un plan : il n'y en a pas de rang 1 à comparer ailleurs que dans A et B.
    b.app.comparer_ouvrir(0, Some(Source::Variante(1)), Some(Source::Actuelle));
    assert!(b.app.comparaison.is_some());
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.05);
        if b.app.comparaison.as_ref().is_some_and(|c| c.difference().is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let c = b.app.comparaison.as_ref().expect("fenêtre ouverte");
    assert!(c.erreurs.iter().all(Option::is_none), "{:?}", c.erreurs);
    assert!(c.difference().is_some(), "les deux côtés ont été composés");
}

// ------------------------------------------------------------------------------------ interface

#[test]
fn the_board_selector_and_the_menus_draw_and_the_application_rests() {
    let mut b = banc_projet(3);
    let seq = b.app.doc.sequence_of(0);
    b.app.mode = crate::app::Mode::Storyboard;
    b.app.board_view = crate::app::BoardView::Panels;
    b.chauffer(3);
    assert!(se_reposer(&mut b, 3000) < 3000, "sans variantes");
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Copier);
    b.chauffer(3);
    assert!(se_reposer(&mut b, 3000) < 3000, "avec des variantes");
    b.app.variantes_ui.supprimer = Some((seq, 1));
    b.app.variantes_ui.renommer = Some((seq, 2, "x".into(), true));
    b.chauffer(3);
    b.app.variantes_ui.supprimer = None;
    b.app.variantes_ui.renommer = None;
    assert!(se_reposer(&mut b, 3000) < 3000, "fenêtres fermées");
    assert_eq!(crate::variantesui::suffixe_barre(&b.app.doc, seq), " · B");
    assert_eq!(crate::variantesui::suffixe_barre(&film(1), 1), "");
}

#[test]
fn switching_a_variant_does_no_heavy_work_on_the_interface_thread() {
    // 60 plans de 48 images avec leurs dessins : un échange de listes, pas un seul dessin recopié ni relu.
    let mut doc = film(60);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    let cels = doc.cels.len();
    let t = std::time::Instant::now();
    for _ in 0..20 {
        assert!(doc.changer_de_variante(seq, 1));
        assert!(doc.changer_de_variante(seq, 2));
    }
    assert_eq!(doc.cels.len(), cels, "aucun dessin créé");
    assert!(t.elapsed() < std::time::Duration::from_millis(500), "{:?}", t.elapsed());
    // L'étape d'annulation ne recopie pas les variantes : l'Arc est partagé.
    let s = doc.structure();
    assert!(Arc::ptr_eq(&s.variantes, &doc.variantes));
}

// ------------------------------------------------------------------------------------ travailler à plusieurs

pub(crate) fn app_partagee(nom: &str, doc: &Document) -> (Banc, Horloge, PathBuf) {
    let racine = dossier(&format!("variantes-coop-{nom}"));
    let m = partager(doc, &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let Loaded { doc, partage, written, .. } = project::load(&m).expect("projet partagé");
    let mut b = banc(doc, false);
    b.app.path = Some(m.clone());
    b.app.written = written;
    let moi = Identite { id: "anthony".into(), nom: "Anthony".into(), couleur: [220, 60, 60], machine: "poste".into(), seance: "s1".into() };
    let depot = Arc::new(DossierPartage::nouveau(&m));
    let s = Session::rejoindre(depot, moi, partage.expect("partagé"), &mut b.app.doc, Delais::default(), h.clone(), false, None).expect("session");
    b.app.partage = Some(s);
    (b, h, m)
}

pub(crate) fn image(b: &mut Banc) {
    b.app.partage.as_ref().expect("session").pomper();
    b.image(Vec::new(), 0.05);
}

pub(crate) fn pomper_enregistrement(b: &mut Banc) {
    for _ in crate::perf_tests::attente() {
        if !b.app.save_en_cours() {
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("l'enregistrement ne se termine pas");
}

#[test]
fn a_shared_project_keeps_the_stored_variants_in_its_folder_and_opens_them_again() {
    let mut doc = film_partage(3);
    let seq = doc.sequence_of(0);
    assert_eq!(doc.nouvelle_variante(seq, Nouvelle::Copier, 1), Some(2));
    let (b, _, m) = app_partagee("ouvrir", &doc);
    assert!(m.parent().unwrap().join("variantes.json").exists());
    assert!(m.parent().unwrap().join("plans").exists());
    assert_eq!(b.app.doc.variantes_de(seq).map(|v| v.numeros()), Some(vec![1, 2]));
    assert_eq!(contenus_range(&b.app.doc, seq, 1), contenus_range(&doc, seq, 1));
    assert!(!b.app.doc.variantes_partielles);
}

#[test]
fn in_a_shared_project_switching_and_creating_variants_need_the_montage_token() {
    let mut doc = film_partage(3);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Vide, 1);
    let (mut b, h, m) = app_partagee("jeton", &doc);
    let film_avant = ids_film(&b.app.doc);
    // Sans jeton : rien ne change, avec la raison.
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(ids_film(&b.app.doc), film_avant);
    assert!(b.app.status.contains("jeton"), "{}", b.app.status);
    b.app.status.clear();
    b.app.nouvelle_variante_de_sequence(seq, Nouvelle::Copier);
    assert_eq!(b.app.doc.nombre_de_variantes(seq), 2);
    assert!(b.app.status.contains("jeton"), "{}", b.app.status);
    b.app.status.clear();
    b.app.renommer_la_variante(seq, 2, "x");
    assert_eq!(b.app.doc.etiquette_de_variante(seq, 2), "B");
    // Avec le jeton : permis, et les plans qui entrent dans le film sont à moi dès leur arrivée.
    b.app.run_action("equipe.prendre_montage");
    h.avancer(9_000);
    image(&mut b);
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(b.app.doc.numero_de_variante(seq), 1, "{}", b.app.status);
    image(&mut b);
    b.image(Vec::new(), 0.05);
    let entres = b.app.doc.plans_de_sequence(seq);
    assert!(!entres.is_empty());
    for p in &entres {
        assert!(b.app.partage.as_ref().expect("session").tiens(Cible::Plan(*p)), "le plan {p} est à moi");
    }
    // L'enregistrement écrit les variantes rangées avec le montage.
    b.app.save();
    pomper_enregistrement(&mut b);
    let Loaded { doc: relu, .. } = project::load(&m).expect("réouverture");
    assert_eq!(relu.numero_de_variante(seq), 1);
    assert_eq!(relu.variantes_de(seq).map(|v| v.numeros()), Some(vec![1, 2]));
    assert_eq!(ids_film(&relu), ids_film(&b.app.doc));
    assert_eq!(contenus_range(&relu, seq, 2), contenus_range(&b.app.doc, seq, 2));
    confirmer(&h);
}

#[test]
fn a_plan_held_by_someone_else_blocks_the_switch() {
    let mut doc = film_partage(3);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Vide, 1);
    let racine = dossier("variantes-coop-pris");
    let m = partager(&doc, &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let Loaded { doc: d, partage, written, .. } = project::load(&m).expect("projet partagé");
    let mut b = banc(d, false);
    b.app.path = Some(m.clone());
    b.app.written = written;
    let moi = Identite { id: "anthony".into(), nom: "Anthony".into(), couleur: [220, 60, 60], machine: "poste".into(), seance: "s1".into() };
    let s = Session::rejoindre(Arc::new(DossierPartage::nouveau(&m)), moi, partage.expect("partagé"), &mut b.app.doc, Delais::default(), h.clone(), false, None).expect("session");
    b.app.partage = Some(s);
    let mut lea = crate::cooperation_tests::Poste::ouvrir(&m, "Lea", &h);
    let plan = b.app.doc.plans_de_sequence(seq)[0];
    let rang = b.app.doc.panels.iter().position(|p| p.id == plan).unwrap();
    lea.prendre_plan(rang).expect("libre");
    confirmer(&h);
    lea.tour();
    image(&mut b);
    b.app.run_action("equipe.prendre_montage");
    h.avancer(9_000);
    image(&mut b);
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(b.app.doc.numero_de_variante(seq), 2, "un plan de Léa ne sort pas du film");
    assert!(b.app.status.contains("Lea"), "{}", b.app.status);
}

#[test]
fn a_shared_project_whose_variant_drawings_have_not_arrived_never_rewrites_the_variants_file() {
    let mut doc = film_partage(3);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 1);
    let racine = dossier("variantes-coop-partiel");
    let m = partager(&doc, &racine);
    // Les dessins d'une variante rangée ne sont pas (encore) là.
    let g = doc.variantes_de(seq).unwrap().garage.values().next().expect("un plan au garage");
    let id: CelId = g.panel.layers[0].frames.iter().flatten().copied().next().expect("un dessin");
    for ext in ["png", "json"] {
        let _ = std::fs::remove_file(m.parent().unwrap().join("dessins").join(format!("{id}.{ext}")));
    }
    let avant = std::fs::read(m.parent().unwrap().join("variantes.json")).unwrap();
    let Loaded { doc: d, warnings, .. } = project::load(&m).expect("s'ouvre");
    assert!(d.variantes_partielles, "{warnings:?}");
    assert_eq!(d.variantes_de(seq).map(|v| v.nombre()), Some(2));
    // Les opérations sur les variantes sont refusées tant qu'il manque des dessins.
    let mut b = banc(d, false);
    b.app.passer_a_la_variante(seq, 1);
    assert_eq!(b.app.doc.numero_de_variante(seq), 2);
    assert!(b.app.status.contains("manquent"), "{}", b.app.status);
    assert_eq!(std::fs::read(m.parent().unwrap().join("variantes.json")).unwrap(), avant);
}
