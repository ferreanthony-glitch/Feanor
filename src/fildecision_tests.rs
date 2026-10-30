//! Tests du fil de décision par plan : statuts (et leur annulation), refus sans raison, fil à travers les versions,
//! conflits, compteurs et filtre par envoi, note suivante à traiter, page de relecture suivante avec les statuts,
//! retakes (CSV et fiche technique), ancien projet lu à l'identique.

use std::collections::HashSet;

use eframe::egui;

use crate::app::App;
use crate::commentaires::{self, Comment, Scope, View};
use crate::document::Document;
use crate::envois::Envoi;
use crate::fiche::{self, Cellule, Colonne, Entree};
use crate::fildecision::{Filter, Source, Status};
use crate::relecture::{self, Meta, Options};

/// Trois plans de 10, 20 et 30 images.
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (p, n) in doc.panels.iter_mut().zip([10, 20, 30]) {
        p.set_length(n);
    }
    doc
}

fn envoi(doc: &mut Document, id: u64, nom: &str) {
    doc.envois.list.push(Envoi { id, nom: nom.into(), date: 1_760_000_000, note_le: 0, fichier: String::new(), plans: Vec::new(), variantes: Vec::new() });
}

fn app_with(name: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("shot-fil-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_frame = 0;
    app
}

fn roundtrip(doc: &Document) -> Vec<Comment> {
    let bytes = commentaires::to_bytes(doc).unwrap().expect("des commentaires");
    commentaires::parse(std::str::from_utf8(&bytes).unwrap()).unwrap()
}

// ------------------------------------------------------------------------------------- les statuts

#[test]
fn an_old_file_reads_identically_and_writes_back_without_the_new_object() {
    let doc = film();
    let a = doc.panels[0].id;
    let text = format!(
        r#"{{"format":"shot-commentaires","version":1,"commentaires":[
        {{"id":1,"plan":{a},"image":2,"texte":"fait déjà","auteur":"A","couleur":[1,2,3],"cree":9,"resolu":true}},
        {{"id":2,"plan":{a},"image":3,"texte":"pas fait","auteur":"B","couleur":[1,2,3],"cree":10}},
        {{"id":3,"plan":{a},"image":4,"texte":"d'une relecture","auteur":"Claire","couleur":[250,204,21],"cree":11,"note":"n1"}}]}}"#
    );
    let list = commentaires::parse(&text).unwrap();
    assert_eq!(list[0].status(), Status::Done, "résolu devient fait");
    assert_eq!(list[1].status(), Status::Todo, "les autres : à faire");
    assert_eq!(list[2].source(), Source::Relecture, "une note de relecture d'avant est reconnue");
    assert!(list.iter().all(|c| c.follow.is_empty()), "rien n'est ajouté à la lecture");
    // Réécrit, le fichier n'a ni « suivi » ni « statut » : un SHOT d'avant le lit tel quel.
    let mut d = doc;
    d.comments = list;
    let out = String::from_utf8(commentaires::to_bytes(&d).unwrap().unwrap()).unwrap();
    assert!(!out.contains("suivi") && !out.contains("statut"), "{out}");
    assert!(out.contains("\"resolu\": true"), "{out}");
}

#[test]
fn the_follow_round_trips_and_an_old_reader_still_sees_resolved_or_not() {
    let mut doc = film();
    let a = doc.panels[0].id;
    envoi(&mut doc, 1, "v1 au réal");
    let r = doc.add_comment(a, 1, "Léa", 100);
    let d = doc.add_comment(a, 2, "Léa", 101);
    let f = doc.add_comment(a, 3, "Léa", 102);
    doc.set_note_status(r, Status::Refused, "  Hors budget.  ", 500).unwrap();
    doc.set_note_status(d, Status::Discussion, "", 501).unwrap();
    doc.set_note_status(f, Status::Done, "", 502).unwrap();
    doc.add_reply(r, "Anthony", "On en reparle jeudi.", 600);
    doc.link_conflict(r, d);
    doc.set_retake(f, Some("Mauvais raccord"));
    {
        let c = doc.comment_mut(d).unwrap();
        c.follow.source = Source::Kitsu;
        c.follow.external = Some("kitsu-4521".into());
    }
    let back = roundtrip(&doc);
    let get = |id: u64| back.iter().find(|c| c.id == id).unwrap().clone();
    assert_eq!(get(r), *doc.comment(r).unwrap());
    assert_eq!(get(d), *doc.comment(d).unwrap());
    assert_eq!(get(f), *doc.comment(f).unwrap());
    assert_eq!((get(r).status(), get(r).follow.reason.as_str()), (Status::Refused, "Hors budget."));
    assert_eq!(get(d).source(), Source::Kitsu);
    assert_eq!(get(d).follow.external.as_deref(), Some("kitsu-4521"));
    assert_eq!(get(r).follow.send, Some(1), "l'envoi concerné est celui du dernier envoi à la pose");
    assert_eq!(get(r).follow.plan_version, 1);
    assert_eq!(get(f).follow.retake.as_deref(), Some("Mauvais raccord"));
    assert_eq!(get(r).follow.conflicts, vec![d]);
    // « resolu » reste la vérité pour un SHOT d'avant : vrai seulement pour « fait ».
    let text = String::from_utf8(commentaires::to_bytes(&doc).unwrap().unwrap()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let resolu = |id: u64| value["commentaires"].as_array().unwrap().iter().find(|c| c["id"] == id).map_or(false, |c| c["resolu"] == true);
    assert!(resolu(f) && !resolu(r) && !resolu(d));
}

#[test]
fn a_damaged_follow_is_set_straight_on_reading() {
    let doc = film();
    let a = doc.panels[0].id;
    let text = format!(
        r#"{{"format":"shot-commentaires","version":1,"commentaires":[
        {{"id":1,"plan":{a},"image":0,"texte":"x","resolu":true,"suivi":{{"statut":"refuse","raison":"ne doit pas rester"}}}},
        {{"id":2,"plan":{a},"image":0,"texte":"y","suivi":{{"statut":"discussion","raison":"pas ici","conflits":[2,1,1,777,0]}}}},
        {{"id":3,"plan":{a},"image":0,"texte":"z","suivi":{{"statut":"fait"}}}}]}}"#
    );
    let list = commentaires::parse(&text).unwrap();
    assert_eq!((list[0].status(), list[0].follow.reason.as_str()), (Status::Done, ""), "fait gagne, sans raison");
    assert_eq!((list[1].status(), list[1].follow.reason.as_str()), (Status::Discussion, ""), "la raison ne se garde que pour un refus");
    assert_eq!(list[1].follow.conflicts, vec![1], "ni soi-même, ni un doublon, ni une note inconnue");
    assert_eq!(list[2].status(), Status::Done);
}

#[test]
fn a_status_change_is_one_named_undo_step() {
    let mut doc = film();
    let id = doc.add_comment(doc.panels[0].id, 0, "A", 1);
    let mut app = app_with("statuts", doc);
    let serial = app.history.serial();
    app.set_note_status(id, Status::Done, "");
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Done);
    assert!(app.history.serial() > serial);
    app.set_note_status(id, Status::Discussion, "");
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Discussion);
    app.set_note_status(id, Status::Refused, "Pas dans le budget");
    assert_eq!(app.doc.comment(id).unwrap().follow.reason, "Pas dans le budget");
    app.undo();
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Discussion, "Ctrl+Z défait le refus d'un coup");
    assert_eq!(app.doc.comment(id).unwrap().follow.reason, "");
    app.undo();
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Done);
    app.undo();
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Todo);
    app.redo();
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Done);
    // Le même statut deux fois : pas d'étape pour rien.
    let serial = app.history.serial();
    app.set_note_status(id, Status::Done, "");
    assert_eq!(app.history.serial(), serial);
}

#[test]
fn a_refusal_needs_a_reason_and_changes_nothing_without_one() {
    let mut doc = film();
    let id = doc.add_comment(doc.panels[0].id, 0, "A", 1);
    assert!(doc.set_note_status(id, Status::Refused, "   ", 5).is_err());
    assert_eq!(doc.comment(id).unwrap().status(), Status::Todo);
    assert!(doc.set_note_status(999, Status::Done, "", 5).is_err(), "une note qui n'existe plus");
    let mut app = app_with("refus", doc);
    let serial = app.history.serial();
    app.set_note_status(id, Status::Refused, "");
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Todo);
    assert_eq!(app.history.serial(), serial, "aucune étape d'annulation");
    assert!(app.status.contains("raison"), "{}", app.status);
    // Depuis un menu : la raison s'écrit d'abord, le statut ne change pas encore.
    app.ask_note_status(id, Status::Refused);
    assert_eq!(app.comment_ui.refusing.as_ref().map(|r| r.0), Some(id));
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Todo);
    app.set_note_status(id, Status::Refused, "  Le plan reste large.  ");
    let c = app.doc.comment(id).unwrap();
    assert_eq!((c.status(), c.follow.reason.as_str()), (Status::Refused, "Le plan reste large."));
    // Un autre statut efface la raison.
    app.set_note_status(id, Status::Done, "");
    assert_eq!(app.doc.comment(id).unwrap().follow.reason, "");
}

// ------------------------------------------------------------------------------------- le fil d'un plan

#[test]
fn the_thread_of_a_plan_runs_through_its_versions_in_order() {
    let mut doc = film();
    let a = doc.panels[0].id;
    let b = doc.panels[1].id;
    // Écrit dans le désordre, avec des numéros qui ne suivent pas les dates.
    let second = doc.add_comment(a, 5, "Claire", 2_000);
    assert_eq!(doc.comment(second).unwrap().follow.plan_version, 1);
    doc.nouvelle_version(0, 3_000).expect("V2");
    let third = doc.add_comment(a, 1, "Claire", 3_500);
    assert_eq!(doc.comment(third).unwrap().follow.plan_version, 2, "la version affichée à la pose");
    let first = doc.add_comment(a, 8, "Léa", 1_000);
    let other = doc.add_comment(b, 0, "Léa", 1_500);
    assert_eq!(doc.thread_of(a), vec![first, second, third], "par date, toutes versions confondues");
    assert_eq!(doc.thread_of(b), vec![other]);
    doc.comment_mut(first).unwrap().follow.plan_version = 1;
    // Le fil survit à un changement de version : les notes restent sur le plan.
    assert_eq!(doc.nombre_de_versions(a), 2);
}

#[test]
fn replies_join_the_thread_and_empty_ones_are_refused() {
    let mut doc = film();
    let id = doc.add_comment(doc.panels[0].id, 0, "Claire", 1);
    assert!(!doc.add_reply(id, "Anthony", "   ", 2));
    assert!(doc.add_reply(id, " Anthony ", "  Compris.  ", 2));
    assert!(doc.add_reply(id, "Claire", "Merci", 3));
    let r = &doc.comment(id).unwrap().follow.replies;
    assert_eq!((r.len(), r[0].author.as_str(), r[0].text.as_str(), r[1].created), (2, "Anthony", "Compris.", 3));
}

#[test]
fn two_notes_in_conflict_are_marked_on_both_sides_and_cleaned_up() {
    let mut doc = film();
    let a = doc.panels[0].id;
    let (x, y, z) = (doc.add_comment(a, 0, "Claire", 1), doc.add_comment(a, 1, "Paul", 2), doc.add_comment(a, 2, "Paul", 3));
    assert!(!doc.link_conflict(x, x), "pas avec soi-même");
    assert!(!doc.link_conflict(x, 999), "ni avec une note inconnue");
    assert!(doc.link_conflict(x, y));
    assert!(!doc.link_conflict(y, x), "déjà marqué");
    assert_eq!((doc.conflicts_of(x), doc.conflicts_of(y)), (vec![y], vec![x]));
    assert!(doc.plan_mark(a).unwrap().conflict, "les deux sont à traiter : le plan le dit");
    // Une des deux est traitée : plus de conflit à régler.
    doc.set_note_status(x, Status::Done, "", 9).unwrap();
    assert!(!doc.plan_mark(a).unwrap().conflict);
    doc.set_note_status(x, Status::Todo, "", 9).unwrap();
    assert!(doc.plan_mark(a).unwrap().conflict);
    // Effacer une note défait le conflit chez l'autre.
    doc.link_conflict(y, z);
    doc.remove_comment(y);
    assert!(doc.conflicts_of(x).is_empty() && doc.conflicts_of(z).is_empty());
    assert!(doc.comment(x).unwrap().follow.conflicts.is_empty(), "la liste elle-même est nettoyée");
    assert!(doc.link_conflict(x, z) && doc.unlink_conflict(z, x));
    assert!(doc.conflicts_of(x).is_empty());
    // Un plan supprimé emporte ses notes et les conflits qui y tenaient.
    doc.link_conflict(x, z);
    doc.comment_mut(z).unwrap().panel = 987_654;
    doc.prune_comments();
    assert!(doc.comment(x).unwrap().follow.conflicts.is_empty());
}

// ------------------------------------------------------------------------------------- vue d'ensemble

#[test]
fn counts_and_the_filter_by_send() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    envoi(&mut doc, 1, "v1 au réal");
    let n1 = doc.add_comment(a, 0, "Claire", 10);
    let n2 = doc.add_comment(a, 3, "Claire", 11);
    envoi(&mut doc, 2, "v2 au réal");
    let n3 = doc.add_comment(b, 0, "Claire", 12);
    let n4 = doc.add_comment(b, 4, "Claire", 13);
    let n5 = doc.add_comment(b, 6, "Claire", 14);
    assert_eq!([n1, n2, n3, n4, n5].map(|id| doc.comment(id).unwrap().follow.send), [Some(1), Some(1), Some(2), Some(2), Some(2)]);
    doc.set_note_status(n1, Status::Done, "", 1).unwrap();
    doc.set_note_status(n2, Status::Refused, "Non.", 1).unwrap();
    doc.set_note_status(n3, Status::Discussion, "", 1).unwrap();
    let all = doc.note_counts(None);
    assert_eq!((all.get(Status::Todo), all.get(Status::Done), all.get(Status::Refused), all.get(Status::Discussion)), (2, 1, 1, 1));
    assert_eq!((all.total(), all.open()), (5, 3));
    let one = doc.note_counts(Some(1));
    assert_eq!((one.get(Status::Done), one.get(Status::Refused), one.open()), (1, 1, 0));
    let two = doc.note_counts(Some(2));
    assert_eq!((two.get(Status::Todo), two.get(Status::Discussion), two.total()), (2, 1, 3));
    // Le filtre : par statut et par envoi, dans la liste comme dans les repères des règles.
    assert_eq!(doc.comments_in_order(&Filter::all()).len(), 5);
    assert_eq!(doc.comments_in_order(&Filter::open_only()).len(), 3);
    let only_v2_todo = Filter { shown: { let mut s = [false; 4]; s[Status::Todo.index()] = true; s }, send: Some(2) };
    assert_eq!(doc.comments_in_order(&only_v2_todo).iter().map(|x| x.1).collect::<Vec<_>>(), vec![n4, n5]);
    assert_eq!(doc.comment_positions(Scope::Film, &only_v2_todo, None).len(), 2);
    assert_eq!(doc.comment_positions(Scope::Plan(a), &Filter::all(), None).iter().map(|p| p.status).collect::<Vec<_>>(), vec![Status::Done, Status::Refused]);
    // Les pastilles : seulement ce qui reste à traiter compte.
    let marks = doc.plan_marks();
    assert_eq!((marks[&a].counts.open(), marks[&b].counts.open()), (0, 3));
    assert_eq!(doc.plan_mark(b), Some(marks[&b]));
    assert_eq!(doc.plan_mark(doc.panels[2].id), None);
}

#[test]
fn the_next_note_to_treat_goes_round_the_film_and_skips_what_is_treated() {
    let mut doc = film();
    let (a, b, c) = (doc.panels[0].id, doc.panels[1].id, doc.panels[2].id);
    envoi(&mut doc, 1, "v1");
    let n1 = doc.add_comment(a, 2, "X", 1); // image 2
    let n2 = doc.add_comment(b, 3, "X", 2); // image 13
    envoi(&mut doc, 2, "v2");
    let n3 = doc.add_comment(c, 0, "X", 3); // image 30
    let n4 = doc.add_comment(c, 5, "X", 4); // image 35
    doc.set_note_status(n2, Status::Done, "", 1).unwrap();
    doc.set_note_status(n4, Status::Discussion, "", 1).unwrap();
    // Depuis le début : la première à traiter ; une note en discussion reste à traiter.
    assert_eq!(doc.next_to_treat(0, None, None), Some(n1));
    assert_eq!(doc.next_to_treat(2, None, None), Some(n3), "n2 est faite : on la saute");
    assert_eq!(doc.next_to_treat(30, None, None), Some(n4));
    assert_eq!(doc.next_to_treat(35, None, None), Some(n1), "en fin de film on revient au début");
    // Avec une note choisie : celle d'après, même sur la même image.
    assert_eq!(doc.next_to_treat(0, Some(n1), None), Some(n3));
    assert_eq!(doc.next_to_treat(0, Some(n4), None), Some(n1));
    // Un envoi seulement.
    assert_eq!(doc.next_to_treat(0, None, Some(2)), Some(n3));
    assert_eq!(doc.next_to_treat(0, None, Some(1)), Some(n1));
    // Plus rien à traiter.
    for id in [n1, n3, n4] {
        doc.set_note_status(id, Status::Done, "", 1).unwrap();
    }
    assert_eq!(doc.next_to_treat(0, None, None), None);
    // La commande va au plan de la note.
    let mut doc = film();
    let (a, c) = (doc.panels[0].id, doc.panels[2].id);
    let _ = a;
    let late = doc.add_comment(c, 5, "X", 1);
    let mut app = app_with("suivante", doc);
    app.run_action("notes.suivante_a_traiter");
    assert_eq!((app.cur_panel, app.cur_frame, app.comment_ui.sel), (2, 5, Some(late)));
    app.set_note_status(late, Status::Done, "");
    app.run_action("notes.suivante_a_traiter");
    assert!(app.status.contains("Plus aucune"), "{}", app.status);
}

// ------------------------------------------------------------------------------------- la relecture suivante

fn fake_images(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| vec![0xFF, 0xD8, i as u8, 7, 8, 9, 0xFF, 0xD9]).collect()
}

fn notes_file(doc: &Document, envoi: &str, extra: &str, notes: &[(&str, u64, &str)]) -> String {
    let list: Vec<String> = notes.iter().map(|(id, plan, text)| format!(r#"{{"id":"{id}","plan":"{plan}","image":"{plan}","numero":"1","texte":"{text}","cree":1760000400}}"#)).collect();
    let _ = doc;
    format!(r#"{{"format":"shot-relecture","version":1,"paquet":"p1","projet":"Mon film","relecteur":"Claire","cree":1760000500{envoi}{extra},"notes":[{}]}}"#, list.join(","))
}

#[test]
fn the_next_review_page_shows_the_previous_notes_with_what_was_done() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    envoi(&mut doc, 1, "v1 au réal");
    envoi(&mut doc, 2, "v2 au réal");
    // La relecture de l'envoi 1 revient : deux notes sur le plan A, une sur B.
    let json = notes_file(&doc, r#","envoi":1"#, "", &[("n1", a, "Plus large ?"), ("n2", a, "Changer la couleur du ciel"), ("n3", b, "Plus lent")]);
    let notes = relecture::parse_notes(json.as_bytes()).unwrap();
    assert_eq!(notes.send, Some(1));
    let places = relecture::place(&doc, &notes);
    relecture::apply(&mut doc, &notes, &places, None, 1_760_001_000);
    let get = |doc: &Document, key: &str| doc.comments.iter().find(|c| c.note.as_deref() == Some(key)).unwrap().id;
    let (n1, n2, n3) = (get(&doc, "n1"), get(&doc, "n2"), get(&doc, "n3"));
    for id in [n1, n2, n3] {
        let c = doc.comment(id).unwrap();
        assert_eq!((c.source(), c.follow.send, c.author.as_str()), (Source::Relecture, Some(1), "Claire"));
    }
    // Une note interne de l'équipe, déjà réglée : elle ne doit jamais partir chez le client.
    let internal = doc.add_comment(a, 1, "Anthony", 5);
    doc.comment_mut(internal).unwrap().text = "NOTE INTERNE SECRETE".into();
    doc.set_note_status(internal, Status::Done, "", 1).unwrap();
    doc.set_note_status(n1, Status::Done, "", 1).unwrap();
    doc.set_note_status(n2, Status::Refused, "Le ciel est voulu ainsi par la charte.", 1).unwrap();
    // n3 reste à faire.
    let meta = Meta { title: "Mon film".into(), project: "Mon film".into(), paquet: "p2".into(), envoi: Some("v2 au réal".into()), now: 1_760_100_000, nonce: "nonce0123456789".into(), file_stem: "notes".into(), send_id: Some(2), prior_send: Some(1), changed: HashSet::from([a]) };
    let (pkg, frames) = relecture::build(&doc, &[0, 1, 2], &Options::default(), meta);
    assert_eq!(pkg.plans[0].prior.len(), 2, "les deux notes du relecteur sur le plan A, pas la note interne");
    assert!(pkg.plans[0].changed && !pkg.plans[1].changed);
    let page = relecture::page(&pkg, &fake_images(frames.len()));
    assert!(page.contains("data-envoi=\"2\""), "la page dit sur quel envoi elle est");
    assert!(page.contains("Vos notes de l\u{2019}envoi précédent"));
    assert!(page.contains("class=\"prec fait\"") && page.contains("class=\"prec refuse\"") && page.contains("class=\"prec afaire\""));
    assert!(page.contains("Plus large ?") && page.contains("Le ciel est voulu ainsi par la charte.") && page.contains("Raison du refus"));
    assert!(page.contains(&format!("data-note=\"{n2}\"")), "la page garde le numéro de la note pour la réponse");
    assert!(page.contains("Modifié depuis l\u{2019}envoi précédent"), "ce qui a changé est mis en avant");
    assert!(!page.contains("NOTE INTERNE SECRETE"), "les notes internes ne sortent pas");
    // Une note de l'envoi d'avant n'est pas redite dans « Déjà noté dans SHOT ».
    assert!(!page.contains("Déjà noté dans SHOT"), "tout ce qui est ouvert est dans le rappel : {}", page.matches("Plus lent").count());
    assert_eq!(page.matches("Plus lent").count(), 1);
    // Sans envoi d'avant : la page est celle d'avant (aucun rappel).
    let (pkg0, frames0) = relecture::build(&doc, &[0, 1, 2], &Options::default(), Meta { prior_send: None, ..Default::default() });
    assert!(pkg0.plans.iter().all(|p| p.prior.is_empty()));
    assert!(!relecture::page(&pkg0, &fake_images(frames0.len())).contains("Vos notes de l\u{2019}envoi précédent"));
}

#[test]
fn the_reviewer_answers_and_the_answers_find_their_notes_once() {
    let mut doc = film();
    let a = doc.panels[0].id;
    envoi(&mut doc, 1, "v1 au réal");
    let id = doc.add_comment(a, 0, "Claire", 10);
    doc.comment_mut(id).unwrap().follow.source = Source::Relecture;
    doc.set_note_status(id, Status::Refused, "Voulu ainsi.", 11).unwrap();
    let json = format!(r#"{{"format":"shot-relecture","version":1,"paquet":"p2","projet":"Mon film","relecteur":"Claire","cree":1760000500,"envoi":2,"suites":[{{"note":{id},"texte":"Je ne suis pas convaincue."}},{{"note":424242,"texte":"orpheline"}},{{"note":{id},"texte":"   "}}],"notes":[]}}"#);
    let notes = relecture::parse_notes(json.as_bytes()).unwrap();
    assert_eq!(notes.send, Some(2), "l'envoi est lu");
    assert_eq!(notes.suites.len(), 2, "la réponse vide est ignorée");
    let places = relecture::place(&doc, &notes);
    let sum = relecture::preview(&doc, &notes, &places, None);
    assert_eq!((sum.imported, sum.replies, sum.new_things()), (0, 1, 1), "la réponse à une note inconnue n'est pas comptée");
    let sum = relecture::apply(&mut doc, &notes, &places, None, 1_760_001_000);
    assert!(sum.message().contains("1 réponse"), "{}", sum.message());
    let r = &doc.comment(id).unwrap().follow.replies;
    assert_eq!((r.len(), r[0].author.as_str(), r[0].text.as_str(), r[0].source), (1, "Claire", "Je ne suis pas convaincue.", Source::Relecture));
    // Réimporter le même fichier : rien de plus.
    let again = relecture::preview(&doc, &notes, &places, None);
    assert_eq!(again.new_things(), 0);
    relecture::apply(&mut doc, &notes, &places, None, 1_760_002_000);
    assert_eq!(doc.comment(id).unwrap().follow.replies.len(), 1);
    // La réponse ne change pas le statut : c'est à l'équipe de décider.
    assert_eq!(doc.comment(id).unwrap().status(), Status::Refused);
    // Les notes d'un fichier sans « envoi » tombent sur le dernier envoi du projet.
    let json = notes_file(&doc, "", "", &[("z1", a, "Autre")]);
    let notes = relecture::parse_notes(json.as_bytes()).unwrap();
    assert_eq!(notes.send, None);
    let places = relecture::place(&doc, &notes);
    relecture::apply(&mut doc, &notes, &places, None, 1_760_003_000);
    assert_eq!(doc.comments.iter().find(|c| c.note.as_deref() == Some("z1")).unwrap().follow.send, Some(1));
}

#[test]
fn the_export_window_picks_the_envoi_before_the_chosen_one() {
    let mut doc = film();
    envoi(&mut doc, 1, "v1");
    envoi(&mut doc, 4, "v2");
    envoi(&mut doc, 7, "v3");
    let app = app_with("avant", doc);
    assert_eq!(app.envoi_precedent(Some(7)), Some(4));
    assert_eq!(app.envoi_precedent(Some(4)), Some(1));
    assert_eq!(app.envoi_precedent(Some(1)), None, "le premier envoi n'a rien avant lui");
    assert_eq!(app.envoi_precedent(None), Some(7), "sans envoi choisi : le dernier");
}

// ------------------------------------------------------------------------------------- les retakes

#[test]
fn retakes_are_listed_in_film_order_and_written_as_csv() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    envoi(&mut doc, 1, "v1 au réal");
    let late = doc.add_comment(b, 4, "Léa", 1_760_000_100);
    let early = doc.add_comment(a, 2, "Claire", 1_760_000_200);
    let plain = doc.add_comment(a, 0, "X", 3);
    doc.comment_mut(late).unwrap().text = "Refaire le raccord ; vite".into();
    doc.set_retake(late, Some("Raccord \"cassé\"\nvoir plan 2"));
    doc.comment_mut(early).unwrap().text = "=SOMME(A1)".into();
    doc.set_retake(early, Some(""));
    doc.set_note_status(early, Status::Refused, "Pas de budget", 1).unwrap();
    let rows = doc.retake_rows();
    assert_eq!(rows.iter().map(|r| r.note).collect::<Vec<_>>(), vec![early, late], "dans l'ordre du film, sans la note qui n'est pas un retake");
    assert!(!rows.iter().any(|r| r.note == plain));
    assert_eq!((rows[0].image, rows[0].status, rows[0].reason.as_str()), (3, Status::Refused, "Pas de budget"));
    assert_eq!(rows[1].send, "v1 au réal, 14/10/2025".replace("14/10/2025", &crate::envois::date_courte(1_760_000_000)));
    let csv = String::from_utf8(doc.retakes_csv()).unwrap();
    assert!(csv.starts_with('\u{feff}'), "BOM : Excel lit l'UTF-8");
    assert!(csv.lines().next().unwrap().contains("Motif du retake") && csv.contains("Statut"), "{csv}");
    assert!(csv.contains("\r\n") && !csv.contains(",Claire"), "séparateur point-virgule, fins de ligne Windows");
    assert!(csv.contains("\"Raccord \"\"cassé\"\"\nvoir plan 2\""), "guillemets et retours à la ligne protégés : {csv}");
    assert!(csv.contains("\"Refaire le raccord ; vite\""), "{csv}");
    assert!(csv.contains("'=SOMME(A1)"), "une formule n'est pas exécutée : {csv}");
    assert!(csv.contains("Refusé") && csv.contains("Pas de budget") && csv.contains("À faire"), "{csv}");
    // Aucun retake : l'en-tête seul.
    let mut vide = film();
    vide.add_comment(vide.panels[0].id, 0, "A", 1);
    assert_eq!(String::from_utf8(vide.retakes_csv()).unwrap().lines().count(), 1);
}

#[test]
fn the_technical_sheet_has_a_retake_column() {
    let mut doc = film();
    let a = doc.panels[0].id;
    let n = doc.add_comment(a, 2, "Claire", 1);
    doc.set_retake(n, Some("Raccord"));
    let m = doc.add_comment(a, 3, "", 2);
    doc.set_retake(m, Some(""));
    let done = doc.add_comment(a, 4, "Léa", 3);
    doc.set_retake(done, Some("Déjà refait"));
    doc.set_note_status(done, Status::Done, "", 1).unwrap();
    assert!(Colonne::depuis_id("retake") == Some(Colonne::Retake) && Colonne::TOUTES.contains(&Colonne::Retake));
    assert!(!Colonne::Retake.par_defaut(), "les fiches d'avant restent identiques");
    let f = fiche::build(&doc, &Entree { plans: &[0, 1], colonnes: &[Colonne::Retake], par_image: false, start_tc: 86_400, etats: None, envoi: None }).expect("fiche");
    let k = f.colonnes.iter().position(|c| *c == Colonne::Retake).unwrap();
    let cell = |l: usize| match &f.lignes[l].cellules[k] {
        Cellule::Texte(t) => t.clone(),
        _ => String::new(),
    };
    assert_eq!(cell(0), "Raccord (Claire)\nRetake", "les retakes encore à traiter, un par ligne ; celui qui est fait n'y est plus");
    assert_eq!(cell(1), "");
}

// ------------------------------------------------------------------------------------- l'écran

#[test]
fn the_panel_and_the_badges_draw_without_trouble_in_both_views() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    envoi(&mut doc, 1, "v1 au réal");
    let n1 = doc.add_comment(a, 1, "Claire", 10);
    let n2 = doc.add_comment(a, 4, "Paul", 11);
    let n3 = doc.add_comment(b, 0, "Claire", 12);
    doc.comment_mut(n1).unwrap().text = "Plus large ?".into();
    doc.comment_mut(n2).unwrap().text = "Plus serré !".into();
    doc.link_conflict(n1, n2);
    doc.set_note_status(n3, Status::Refused, "Voulu.", 1).unwrap();
    doc.add_reply(n1, "Anthony", "Je regarde.", 20);
    doc.set_retake(n2, Some("Cadre"));
    let mut app = app_with("ecran", doc);
    app.comment_ui.sel = Some(n1);
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(420.0, 900.0))), ..Default::default() };
    for (view, sel, pick, refusing) in [(View::Liste, n1, false, false), (View::Fil, n2, true, false), (View::Fil, n3, false, true), (View::Liste, n3, false, false)] {
        app.comment_ui.view = view;
        app.comment_ui.sel = Some(sel);
        app.comment_ui.conflict_pick = pick;
        app.comment_ui.refusing = refusing.then(|| (sel, "essai".to_string()));
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.comments_panel(ui));
            });
        }
    }
    // Le filtre ne laisse rien : un message, pas un plantage.
    app.comment_ui.filter.shown = [false; 4];
    let _ = ctx.run(input.clone(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.comments_panel(ui));
    });
    // La pastille : sur un bloc large, rien sur un bloc petit.
    let _ = ctx.run(input, |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let mark = app.doc.plan_mark(a).unwrap();
        assert!(mark.conflict && mark.retake && mark.counts.open() == 2);
        let large = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(120.0, 90.0));
        let zone = crate::fildecisionui::peindre_marque_du_bloc(&painter, large, &mark).expect("pastille");
        assert!(large.contains_rect(zone) && zone.right() > large.right() - 30.0 && zone.top() > large.top() + 20.0, "sous la pastille d'envoi : {zone:?}");
        assert!(crate::fildecisionui::peindre_marque_du_bloc(&painter, egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(20.0, 90.0)), &mark).is_none());
        assert!(crate::fildecisionui::peindre_marque_du_bloc(&painter, egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(120.0, 40.0)), &mark).is_none());
    });
    let phrase = crate::fildecisionui::marque_phrase(&app.doc.plan_mark(a).unwrap());
    assert!(phrase.contains("2 notes à traiter") && phrase.contains("conflit") && phrase.contains("retake"), "{phrase}");
}

#[test]
fn the_commands_are_in_the_keymap_and_work_on_the_chosen_note() {
    for id in ["notes.suivante_a_traiter", "notes.fil", "notes.statut_a_faire", "notes.statut_fait", "notes.statut_refuse", "notes.statut_discussion", "notes.retake", "notes.conflit", "notes.retakes_exporter"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
        assert!(crate::keymap::DEFAULT_KEYMAP.contains(&format!("action = \"{id}\"")), "{id} a son bloc dans keymap.toml");
    }
    let mut doc = film();
    let id = doc.add_comment(doc.panels[0].id, 0, "A", 1);
    let mut app = app_with("commandes", doc);
    // Sans note choisie : un message, rien ne change.
    assert!(app.notes_action("notes.statut_fait"));
    assert!(app.status.contains("Aucune note choisie"), "{}", app.status);
    app.comment_ui.sel = Some(id);
    app.notes_action("notes.statut_fait");
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Done);
    app.notes_action("notes.statut_discussion");
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Discussion);
    app.notes_action("notes.statut_refuse");
    assert!(app.comment_ui.refusing.is_some() && app.doc.comment(id).unwrap().status() == Status::Discussion, "la raison d'abord");
    app.notes_action("notes.retake");
    assert!(app.doc.comment(id).unwrap().follow.retake.is_some());
    app.notes_action("notes.retake");
    assert!(app.doc.comment(id).unwrap().follow.retake.is_none());
    app.notes_action("notes.statut_a_faire");
    assert_eq!(app.doc.comment(id).unwrap().status(), Status::Todo);
    app.notes_action("notes.fil");
    assert_eq!(app.comment_ui.view, View::Fil);
    assert!(!app.notes_action("commentaire.ajouter"), "ce n'est pas une commande du fil");
}

/// Capture du fil d'un plan avec plusieurs statuts (à regarder) :
/// `SHOT_CAPTURE_DIR=… SHOT_CAPTURE_PROJET=<fichier .pelure d'une copie> cargo test --release --offline --target-dir target\msvc fil_capture -- --ignored --nocapture`
#[test]
#[ignore]
fn fil_capture() {
    let dir = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut doc = match std::env::var_os("SHOT_CAPTURE_PROJET") {
        Some(p) => crate::project::load(std::path::Path::new(&p)).expect("projet").doc,
        None => film(),
    };
    doc.comments.clear();
    let a = doc.panels[0].id;
    let jour = 86_400u64;
    let t0 = 1_761_000_000u64;
    envoi(&mut doc, 1, "v1 au réal");
    let n1 = doc.add_comment(a, 0, "Claire", t0);
    let n2 = doc.add_comment(a, 3, "Claire", t0 + 60);
    let n3 = doc.add_comment(a, 5, "Paul", t0 + jour);
    doc.nouvelle_version(0, t0 + jour + 5).expect("V2");
    envoi(&mut doc, 2, "v2 au réal");
    let n4 = doc.add_comment(a, 1, "Claire", t0 + 3 * jour);
    let n5 = doc.add_comment(a, 6, "Léa", t0 + 3 * jour + 90);
    for (id, text, source) in [
        (n1, "Le plan démarre trop tard : couper les quatre premières images.", Source::Relecture),
        (n2, "Remplacer le ciel par un coucher de soleil.", Source::Relecture),
        (n3, "Au contraire, garder le ciel gris : c'est la charte du film.", Source::Relecture),
        (n4, "Le raccord avec le plan précédent saute encore.", Source::Relecture),
        (n5, "Ajouter un gros plan sur les mains ?", Source::Shot),
    ] {
        let c = doc.comment_mut(id).unwrap();
        c.text = text.into();
        c.follow.source = source;
    }
    doc.set_note_status(n1, Status::Done, "", t0 + jour).unwrap();
    doc.set_note_status(n2, Status::Refused, "La charte graphique impose le ciel gris jusqu'à la scène 12.", t0 + 2 * jour).unwrap();
    doc.add_reply(n2, "Anthony", "Voir la note de Paul : on garde le gris, je te confirme jeudi.", t0 + 2 * jour + 30);
    doc.link_conflict(n2, n3);
    doc.set_note_status(n5, Status::Discussion, "", t0 + 3 * jour + 120).unwrap();
    doc.set_retake(n4, Some("Raccord"));
    let mut b = crate::perf_tests::banc_taille(doc, true, 1920, 1080);
    b.chauffer(8);
    b.app.comment_ui.view = View::Fil;
    b.app.comment_ui.sel = Some(n4);
    b.app.show_pane(crate::dock::Pane::Comments);
    b.chauffer(12);
    let Some(rgba) = b.capturer() else { return println!("pas de carte graphique") };
    let f = dir.join("fil-du-plan.png");
    image::save_buffer(&f, &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    println!("CAPTURE {}", f.display());
}