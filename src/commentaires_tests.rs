//! Tests des commentaires sur la timeline : fichier à côté du manifeste, positions qui suivent les
//! plans, annulation, prise des fanions à la souris.

use crate::commentaires::{self, marker_hits, step_comment, Comment, Scope, DEFAULT_COLOR, FILE};
use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::project;

fn nav() -> Nav {
    Nav { panel: 0, layer: 0 }
}

fn snap(doc: &Document) -> Edit {
    Edit::Structure { structure: doc.structure(), nav: nav() }
}

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

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-commentaires-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_comment_is_attached_to_a_frame_of_a_plan() {
    let mut doc = film();
    let third = doc.panels[2].id;
    let id = doc.add_comment(third, 4, "Anthony", 1_000);
    let c = doc.comment(id).unwrap().clone();
    assert_eq!((c.panel, c.frame, c.color, c.done), (third, 4, DEFAULT_COLOR, false));
    assert_eq!(doc.comment_global(&c), Some(10 + 20 + 4), "dans le montage : début du plan + image");
    assert_eq!(doc.comment_spot(&c), Some((2, 4)));
    // Plus loin que la fin du plan : sur sa dernière image, sans perdre l'image voulue.
    doc.comment_mut(id).unwrap().frame = 500;
    let c = doc.comment(id).unwrap().clone();
    assert_eq!(doc.comment_global(&c), Some(59));
    assert_eq!(c.frame, 500);
}

#[test]
fn the_file_round_trips_next_to_the_manifest() {
    let mut doc = film();
    let (a, c) = (doc.panels[0].id, doc.panels[2].id);
    let first = doc.add_comment(c, 7, "Anthony", 1_760_000_000);
    let second = doc.add_comment(a, 2, "Léa", 1_760_000_100);
    {
        let m = doc.comment_mut(first).unwrap();
        m.text = "Le plan est trop long.\nCouper « dix images ».".into();
        m.color = [250, 204, 21];
        m.done = true;
    }
    let dir = temp("aller-retour");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(FILE).exists(), "commentaires.json est écrit à côté du manifeste");
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(text.contains("\"commentaires\"") && text.contains("Léa"), "{text}");
    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    let mut back = loaded.doc.comments.clone();
    back.sort_by_key(|c| c.id);
    let mut want = doc.comments.clone();
    want.sort_by_key(|c| c.id);
    assert_eq!(back, want);
    assert_eq!(loaded.doc.comment(second).unwrap().author, "Léa");
    // Les identifiants suivants ne retombent pas sur ceux des commentaires relus.
    let mut doc2 = loaded.doc;
    let n = doc2.add_comment(a, 0, "x", 0);
    assert!(doc2.comments.iter().filter(|c| c.id == n).count() == 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn without_the_file_there_are_no_comments() {
    let doc = film();
    let dir = temp("sans-fichier");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists(), "rien à écrire sans commentaire");
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.doc.comments.is_empty() && !loaded.warnings.iter().any(|w| w.contains(FILE)));

    // Des commentaires, puis plus aucun : le fichier part à l'enregistrement.
    let mut doc = doc;
    let a = doc.panels[0].id;
    let id = doc.add_comment(a, 1, "A", 1);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(FILE).exists());
    doc.remove_comment(id);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists());
    // Un fichier effacé à la main : le projet s'ouvre, sans commentaires.
    doc.add_comment(a, 1, "A", 1);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    std::fs::remove_file(dir.join(FILE)).unwrap();
    assert!(project::load(&manifest).unwrap().doc.comments.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_damaged_file_does_not_stop_the_project_from_opening() {
    let doc = film();
    let dir = temp("abime");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    std::fs::write(dir.join(FILE), "{ pas du json").unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.doc.comments.is_empty());
    assert!(loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    // Un format d'une version plus récente est refusé de la même façon, sans écraser le projet.
    std::fs::write(dir.join(FILE), r#"{"format":"shot-commentaires","version":99,"commentaires":[]}"#).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.iter().any(|w| w.contains("plus récente")), "{:?}", loaded.warnings);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn saving_leaves_the_other_files_of_the_folder_alone() {
    let mut doc = film();
    let a = doc.panels[0].id;
    doc.add_comment(a, 0, "A", 1);
    let dir = temp("inconnus");
    let manifest = dir.join("F.pelure");
    std::fs::write(dir.join("notes-de-prod.txt"), "à garder").unwrap();
    std::fs::create_dir_all(dir.join("reference")).unwrap();
    std::fs::write(dir.join("reference").join("image.png"), "x").unwrap();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join("notes-de-prod.txt").exists());
    assert!(dir.join("reference").join("image.png").exists());
    assert!(dir.join(FILE).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_format_version_of_the_project_is_untouched() {
    // Les commentaires n'ont pas de version de projet à eux : le manifeste reste celui d'avant.
    // (25 : la version de main au moment de la fusion ; les commentaires n'y ont pas touché.)
    assert_eq!(project::VERSION, 25);
}

#[test]
fn parse_rejects_foreign_files_and_repairs_ids() {
    assert!(commentaires::parse(r#"{"format":"autre","version":1}"#).is_err());
    assert!(commentaires::parse("n'importe quoi").is_err());
    let text = r#"{"format":"shot-commentaires","version":1,"commentaires":[
        {"id":5,"plan":1,"image":2},
        {"id":5,"plan":1,"image":3,"texte":"deux"},
        {"id":0,"plan":2,"image":0}]}"#;
    let list = commentaires::parse(text).unwrap();
    assert_eq!(list.len(), 3);
    let mut ids: Vec<u64> = list.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3, "identifiants uniques et non nuls : {ids:?}");
    assert!(ids.iter().all(|&i| i != 0));
    assert_eq!(list[0].color, DEFAULT_COLOR, "couleur absente : celle par défaut");
    assert!(!list[0].done && list[0].text.is_empty() && list[1].text == "deux");
}

#[test]
fn a_comment_of_a_removed_plan_is_not_written() {
    let mut doc = film();
    let c = doc.panels[2].id;
    let id = doc.add_comment(c, 3, "A", 1);
    doc.comments.push(Comment { id: 999, panel: 123_456, frame: 0, text: "orphelin".into(), author: String::new(), color: DEFAULT_COLOR, created: 0, done: false });
    let bytes = commentaires::to_bytes(&doc).unwrap().unwrap();
    let list = commentaires::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert_eq!(list.iter().map(|c| c.id).collect::<Vec<_>>(), vec![id]);
}

#[test]
fn move_edit_and_delete_go_through_undo_and_redo() {
    let mut doc = film();
    let (a, c) = (doc.panels[0].id, doc.panels[2].id);
    let id = doc.add_comment(a, 2, "Anthony", 1);
    let mut history = History::default();
    let mut n = nav();

    // Déplacer : de l'image 2 du plan 1 à l'image 35 du film (image 5 du plan 3).
    history.context = Some("Déplacer le commentaire".into());
    history.push(snap(&doc));
    assert!(doc.move_comment_global(id, 35));
    let m = doc.comment(id).unwrap();
    assert_eq!((m.panel, m.frame), (c, 5), "il change de plan avec l'image");
    assert_eq!(history.names().0, ["Déplacer le commentaire"]);
    history.undo(&mut doc, &mut n);
    let m = doc.comment(id).unwrap();
    assert_eq!((m.panel, m.frame), (a, 2));
    history.redo(&mut doc, &mut n);
    assert_eq!(doc.comment(id).unwrap().frame, 5);

    // Modifier le texte, puis supprimer.
    history.push(snap(&doc));
    doc.comment_mut(id).unwrap().text = "À revoir".into();
    history.push(snap(&doc));
    assert!(doc.remove_comment(id));
    assert!(doc.comment(id).is_none());
    history.undo(&mut doc, &mut n);
    assert_eq!(doc.comment(id).unwrap().text, "À revoir", "la suppression s'annule");
    history.undo(&mut doc, &mut n);
    assert_eq!(doc.comment(id).unwrap().text, "", "le texte aussi");
    history.redo(&mut doc, &mut n);
    history.redo(&mut doc, &mut n);
    assert!(doc.comment(id).is_none());
}

#[test]
fn adding_a_comment_is_one_undo_step() {
    let mut doc = film();
    let a = doc.panels[0].id;
    let mut history = History::default();
    let mut n = nav();
    history.push(snap(&doc));
    doc.add_comment(a, 3, "A", 1);
    assert_eq!(doc.comments.len(), 1);
    history.undo(&mut doc, &mut n);
    assert!(doc.comments.is_empty());
    history.redo(&mut doc, &mut n);
    assert_eq!(doc.comments.len(), 1);
}

#[test]
fn moving_inside_a_plan_stays_in_the_plan() {
    let mut doc = film();
    let b = doc.panels[1].id;
    let id = doc.add_comment(b, 5, "A", 1);
    assert!(doc.move_comment_local(id, 12));
    assert_eq!(doc.comment(id).unwrap().frame, 12);
    assert!(doc.move_comment_local(id, 5000));
    assert_eq!(doc.comment(id).unwrap().frame, 19, "au plus la dernière image du plan");
    assert!(!doc.move_comment_local(id, 19), "rien n'a bougé");
    assert_eq!(doc.comment(id).unwrap().panel, b);
}

#[test]
fn comments_follow_their_plan_when_plans_are_reordered() {
    let mut doc = film();
    let (first, third) = (doc.panels[0].id, doc.panels[2].id);
    let id = doc.add_comment(third, 4, "A", 1);
    let global = |d: &Document| d.comment_global(d.comment(id).unwrap()).unwrap();
    assert_eq!(global(&doc), 34);
    // Le troisième plan passe en tête : son commentaire aussi.
    doc.move_plans(&[third], Some(first)).unwrap();
    assert_eq!(doc.panels[0].id, third);
    assert_eq!(global(&doc), 4);
    // Allonger un plan d'avant le décale ; le commentaire reste à la même image de son plan.
    doc.panels[0].set_length(50);
    assert_eq!(global(&doc), 4);
    doc.move_plans(&[third], None).unwrap();
    assert_eq!(global(&doc), 10 + 20 + 4);
}

#[test]
fn a_removed_plan_takes_its_comments_and_undo_brings_them_back() {
    let mut doc = film();
    let b = doc.panels[1].id;
    let id = doc.add_comment(b, 2, "A", 1);
    let kept = doc.add_comment(doc.panels[0].id, 0, "A", 1);
    let mut history = History::default();
    let mut n = nav();
    history.push(snap(&doc));
    doc.remove_panel(1).unwrap();
    assert!(doc.comment(id).is_none(), "parti avec son plan");
    assert!(doc.comment(kept).is_some());
    history.undo(&mut doc, &mut n);
    assert!(doc.comment(id).is_some() && doc.panels.len() == 3);
}

#[test]
fn cutting_and_joining_plans_carry_the_comments() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    let early = doc.add_comment(b, 3, "A", 1);
    let late = doc.add_comment(b, 15, "A", 1);
    // Couper le plan 2 (20 images) à l'image 10 : le commentaire d'après la coupe suit la suite.
    let second = doc.split_plan(1, 10).unwrap();
    let new_id = doc.panels[second].id;
    assert_eq!((doc.comment(early).unwrap().panel, doc.comment(early).unwrap().frame), (b, 3));
    assert_eq!((doc.comment(late).unwrap().panel, doc.comment(late).unwrap().frame), (new_id, 5));
    let global_late = doc.comment_global(doc.comment(late).unwrap()).unwrap();
    assert_eq!(global_late, 10 + 10 + 5, "même image du film qu'avant la coupe");
    // Les réunir : tout revient sur le premier plan, à la même image du film.
    doc.join_next_plan(1).unwrap();
    assert_eq!(doc.comment(late).unwrap().panel, b);
    assert_eq!(doc.comment_global(doc.comment(late).unwrap()), Some(10 + 15));
    assert_eq!(doc.comment_global(doc.comment(early).unwrap()), Some(10 + 3));
    let _ = a;
}

#[test]
fn positions_per_ruler_scope_filter_and_selection() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    let x = doc.add_comment(a, 8, "A", 1);
    let y = doc.add_comment(b, 2, "A", 1);
    let z = doc.add_comment(b, 9, "A", 1);
    doc.comment_mut(z).unwrap().done = true;
    // Le montage : tout le film, dans l'ordre.
    let film_view = doc.comment_positions(Scope::Film, true, None);
    assert_eq!(film_view.iter().map(|p| (p.id, p.pos)).collect::<Vec<_>>(), vec![(x, 8), (y, 12), (z, 19)]);
    // La timeline du plan 2 : ses commentaires seuls, à l'image du plan.
    let plan_view = doc.comment_positions(Scope::Plan(b), true, None);
    assert_eq!(plan_view.iter().map(|p| (p.id, p.pos)).collect::<Vec<_>>(), vec![(y, 2), (z, 9)]);
    assert!(plan_view[1].done);
    // Les résolus peuvent se cacher.
    assert_eq!(doc.comment_positions(Scope::Film, false, None).len(), 2);
    assert_eq!(doc.comments_in_order(false).len(), 2);
    // Le choisi se dessine en dernier (dessus).
    assert_eq!(doc.comment_positions(Scope::Film, true, Some(x)).last().map(|p| p.id), Some(x));
}

#[test]
fn stepping_goes_to_the_next_and_previous_comment() {
    let list = [(5, 1), (12, 2), (12, 3), (40, 4)];
    assert_eq!(step_comment(&list, 0, true), Some(1));
    assert_eq!(step_comment(&list, 5, true), Some(2));
    assert_eq!(step_comment(&list, 12, true), Some(4));
    assert_eq!(step_comment(&list, 40, true), None);
    assert_eq!(step_comment(&list, 40, false), Some(3));
    assert_eq!(step_comment(&list, 12, false), Some(1));
    assert_eq!(step_comment(&list, 5, false), None);
}

#[test]
fn hit_test_of_a_marker() {
    // Deux fanions : mâts en x = 100 et x = 140 (le dernier est dessiné dessus).
    let markers = [(1, 100.0), (2, 140.0)];
    assert!(marker_hits(&markers, 80.0).is_empty(), "à gauche du fanion");
    assert_eq!(marker_hits(&markers, 98.0), vec![1], "juste à gauche du mât");
    assert_eq!(marker_hits(&markers, 106.0), vec![1], "sur le fanion");
    assert_eq!(marker_hits(&markers, 110.0), vec![1], "au bout du fanion");
    assert!(marker_hits(&markers, 111.0).is_empty(), "après le fanion");
    assert!(marker_hits(&markers, 120.0).is_empty(), "entre les deux");
    assert_eq!(marker_hits(&markers, 143.0), vec![2]);
    // Deux commentaires à la même image : celui qui est dessus d'abord, l'autre ensuite.
    let stacked = [(7, 50.0), (8, 50.0)];
    assert_eq!(marker_hits(&stacked, 52.0), vec![8, 7]);
    // Deux fanions voisins : le plus proche d'abord.
    let near = [(1, 100.0), (2, 106.0)];
    assert_eq!(marker_hits(&near, 100.0), vec![1], "hors de la zone du second");
    assert_eq!(marker_hits(&near, 104.0), vec![1, 2]);
    assert_eq!(marker_hits(&near, 108.0), vec![2, 1]);
}

#[test]
fn delete_only_hits_a_comment_that_has_the_focus() {
    let mut doc = film();
    let a = doc.panels[0].id;
    let id = doc.add_comment(a, 1, "A", 1);
    assert_eq!(commentaires::delete_target(true, Some(id), &doc), Some(id));
    assert_eq!(commentaires::delete_target(false, Some(id), &doc), None, "pas de focus : Suppr ne fait rien");
    assert_eq!(commentaires::delete_target(true, None, &doc), None, "pas de sélection : rien");
    doc.remove_comment(id);
    assert_eq!(commentaires::delete_target(true, Some(id), &doc), None, "un commentaire qui n'est plus là");
}

#[test]
fn author_preview_and_dates() {
    assert_eq!(commentaires::author_name("  Léa "), "Léa");
    assert!(!commentaires::author_name("").is_empty(), "l'utilisateur Windows, sinon « Anonyme »");
    assert_eq!(commentaires::preview("une\n\n  deux  \ntrois\nquatre", 2, 80), "une\ndeux\n…");
    assert_eq!(commentaires::preview("", 3, 10), "");
    assert_eq!(commentaires::preview("abcdefghij", 1, 4), "abcd…");
    assert_eq!(commentaires::first_line("\n  Trop long.  \nCouper.", 40), "Trop long. …");
    assert_eq!(commentaires::first_line("Une seule ligne", 40), "Une seule ligne");
    assert_eq!(commentaires::first_line("abcdefghij\nsuite", 4), "abcd…");
    assert_eq!(commentaires::first_line("  \n ", 4), "");
}

#[test]
fn the_comments_pane_is_closed_by_default_and_opens_on_the_right() {
    use crate::dock::{DockLayout, Group, Pane};
    use crate::workspace::Layout;
    let mut l = DockLayout::from_legacy(&Layout::default());
    assert!(!l.contains(Pane::Comments), "fermé par défaut");
    assert_eq!(Pane::Comments.group(), Group::Right);
    l.open(Pane::Comments);
    assert!(l.contains(Pane::Comments));
    assert!(format!("{:?}", l.principal).contains("Layers, Comments"), "{:?}", l.principal);
    // Une disposition enregistrée garde son nom de panneau.
    assert_eq!(serde_json::to_string(&Pane::Comments).unwrap(), "\"commentaires\"");
}

#[test]
fn the_actions_exist() {
    for id in ["vue.commentaires", "commentaire.ajouter", "commentaire.resoudre", "commentaire.supprimer", "commentaire.precedent", "commentaire.suivant"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.commentaires"), Some(crate::dock::Pane::Comments));
}
