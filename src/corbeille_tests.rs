//! Essais de la corbeille du projet (`corbeille.rs`, `corbeilleui.rs`, partie 104 de `docs/INTERFACE.md`, format 47) : chaque geste qui
//! retire quelque chose du film l'y envoie, restaurer remet à la place ou ailleurs en une étape, annuler ne laisse pas de double,
//! enregistrer et rouvrir gardent tout sans relire un dessin, un projet d'avant s'ouvre corbeille vide, vider, numéros uniques.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use crate::app::App;
use crate::corbeille::Sorte;
use crate::document::Document;
use crate::montage_sans_casse_tests::{annuler, appli, film_en_empreintes, inventaire, NOM};
use crate::project::{self, Reuse};
use crate::rognage::Rogner;

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-corbeille-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn manifeste(d: &std::path::Path) -> PathBuf {
    d.join("Film").join("Film.shot")
}

fn enregistrer(doc: &Document, m: &std::path::Path) -> HashMap<u64, u64> {
    let mut written = HashMap::new();
    project::save(doc, m, &mut written).expect("enregistrement");
    written
}

/// Restaure et attend la fin de la lecture des dessins (elle se fait sur un autre fil quand ils sont à relire du dossier).
fn restaurer(app: &mut App, id: u64) {
    app.corbeille_restaurer(id);
    let debut = std::time::Instant::now();
    while app.corbeille_en_cours() {
        assert!(debut.elapsed().as_secs() < 30, "la restauration ne se termine pas");
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.corbeille_sonder();
    }
}

fn ids_plans(app: &App) -> Vec<u64> {
    app.doc.panels.iter().map(|p| p.id).collect()
}

#[test]
fn deleting_plans_sends_them_to_the_bin_with_one_undo_step_and_no_double() {
    let mut app = appli("suppr");
    let ids = ids_plans(&app);
    let n = app.history.undo_len();
    app.delete_panels(&[ids[1], ids[2]]);
    assert_eq!(app.doc.panels.len(), 2);
    assert_eq!(app.doc.corbeille.len(), 2, "un élément par plan");
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert!(app.status.contains("corbeille"), "{}", app.status);
    assert!(app.doc.corbeille.entrees.iter().all(|e| e.sorte == Sorte::Plan && !e.cels.is_empty()));
    annuler(&mut app);
    assert_eq!(app.doc.panels.len(), 4);
    assert_eq!(app.doc.corbeille.len(), 0, "annuler rend les plans au film sans en laisser une copie dans la corbeille");
    app.run_action("edition.retablir");
    assert_eq!(app.doc.corbeille.len(), 2, "rétablir : de nouveau dans la corbeille, une fois");
    // Rien de la corbeille n'est dans le film : ni plans, ni dessins comptés par le projet.
    let corb: BTreeSet<u64> = app.doc.corbeille.cels().collect();
    assert!(corb.iter().all(|c| !app.doc.used_cels().contains(c)));
    assert_eq!(app.doc.total_frames(), 24);
}

#[test]
fn restoring_puts_a_plan_back_in_its_place_in_one_undo_step() {
    let mut app = appli("restaurer");
    let avant = film_en_empreintes(&app.doc);
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    let e = app.doc.corbeille.entrees[0].id;
    let n = app.history.undo_len();
    restaurer(&mut app, e);
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert_eq!(app.doc.corbeille.len(), 0);
    assert_eq!(film_en_empreintes(&app.doc), avant, "le même film, le plan à sa place, les mêmes dessins");
    assert_eq!(app.doc.panels[1].id, ids[1], "même numéro de plan");
    assert!(app.status.contains("à sa place"), "{}", app.status);
    annuler(&mut app);
    assert_eq!(app.doc.panels.len(), 3);
    assert_eq!(app.doc.corbeille.len(), 1, "annuler la restauration rend l'élément à la corbeille");
    let mut uniques = ids_plans(&app);
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(uniques.len(), app.doc.panels.len());
}

#[test]
fn restoring_elsewhere_when_the_place_is_gone() {
    let mut app = appli("ailleurs");
    let ids = ids_plans(&app);
    // Le plan 2 part, puis le plan qui le précédait : il n'a plus de « derrière qui revenir ».
    // (Le plan d'après aussi : sans plan devant lequel ou derrière lequel revenir, il va à la fin de sa séquence.)
    app.delete_panels(&[ids[1]]);
    app.delete_panels(&[ids[0], ids[2]]);
    let e = app.doc.corbeille.entrees.iter().find(|e| e.plan == ids[1]).unwrap().id;
    restaurer(&mut app, e);
    assert_eq!(app.doc.panels.last().unwrap().id, ids[1], "à la fin de sa séquence");
    assert!(app.status.contains("fin de sa séquence"), "{}", app.status);
    // Les identifiants restent uniques même si le numéro du plan était pris.
    let e2 = app.doc.corbeille.entrees[0].id;
    restaurer(&mut app, e2);
    let mut v = ids_plans(&app);
    v.sort_unstable();
    v.dedup();
    assert_eq!(v.len(), app.doc.panels.len(), "aucun numéro en double");
}

#[test]
fn a_deleted_layer_goes_to_the_bin_and_comes_back_in_its_plan_or_says_why_not() {
    let mut app = appli("calque");
    let avant = film_en_empreintes(&app.doc);
    app.cur_layer = 0;
    app.delete_chosen_layers();
    assert_eq!(app.doc.corbeille.len(), 1);
    assert_eq!(app.doc.corbeille.entrees[0].sorte, Sorte::Calques);
    assert!(app.status.contains("corbeille"), "{}", app.status);
    let id = app.doc.corbeille.entrees[0].id;
    restaurer(&mut app, id);
    assert_eq!(film_en_empreintes(&app.doc), avant, "le calque est revenu à son rang, avec ses dessins");
    // Son plan n'existe plus : la phrase le dit, le calque reste dans la corbeille.
    app.cur_layer = 0;
    app.delete_chosen_layers();
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[0]]);
    let calque = app.doc.corbeille.entrees.iter().find(|e| e.sorte == Sorte::Calques).unwrap().id;
    let n = app.doc.corbeille.len();
    restaurer(&mut app, calque);
    assert_eq!(app.doc.corbeille.len(), n, "rien n'a bougé");
    assert!(app.status.contains("n'existe plus"), "{}", app.status);
}

#[test]
fn drawings_covered_by_an_alt_drag_go_to_the_bin_and_come_back_as_a_layer() {
    use crate::handles::KeyMove;
    use crate::montagekeys::KeyRef;
    let mut app = appli("alt");
    let (panel, layer) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id);
    let inv0 = inventaire(&app.doc);
    app.key_sel = vec![KeyRef { panel, layer, start: 0 }];
    assert!(app.begin_keys_drag(0));
    app.drag_keys_mode(3, KeyMove::Copy);
    app.end_keys_drag();
    assert_eq!(app.doc.corbeille.len(), 1);
    assert!(app.status.contains("corbeille"), "{}", app.status);
    assert!(inventaire(&app.doc).len() < inv0.len() + 1);
    let id = app.doc.corbeille.entrees[0].id;
    restaurer(&mut app, id);
    assert_eq!(app.doc.panels[0].layers.len(), 3, "le calque « Dessins remplacés » revient dans le plan");
    assert!(inv0.is_subset(&inventaire(&app.doc)), "tous les dessins d'avant sont de nouveau là");
}

#[test]
fn what_a_join_forgets_goes_to_the_bin() {
    use crate::fusionnoms::{JoinOpts, Jonction};
    let mut app = appli("jonction");
    app.rogner_plan(0, Rogner::Tail(-3), NOM);
    app.rogner_plan(1, Rogner::Head(2), NOM);
    let inv0 = inventaire(&app.doc);
    let a = app.doc.panels[0].id;
    assert!(app.jonction_faire(Jonction::Suivant { a, precedent: false }, &JoinOpts::default()));
    assert!(app.doc.corbeille.len() >= 1, "les images rognées du joint sont à la corbeille");
    assert!(app.doc.corbeille.entrees.iter().all(|e| e.sorte == Sorte::Plan && e.titre.starts_with("Images rognées")));
    assert!(app.status.contains("rognée"), "{}", app.status);
    let id = app.doc.corbeille.entrees[0].id;
    let plans = app.doc.panels.len();
    restaurer(&mut app, id);
    assert_eq!(app.doc.panels.len(), plans + 1, "un petit plan à part, derrière le plan réuni");
    assert!(inv0.is_subset(&inventaire(&app.doc)), "aucun dessin perdu");
}

#[test]
fn the_bin_survives_saving_and_reopening_without_reading_a_drawing() {
    let d = dossier("rouvrir");
    let m = manifeste(&d);
    let mut app = appli("rouvrir");
    let avant = film_en_empreintes(&app.doc);
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    app.cur_layer = 0;
    app.delete_chosen_layers();
    assert_eq!(app.doc.corbeille.len(), 2);
    let cels: BTreeSet<u64> = app.doc.corbeille.cels().collect();
    enregistrer(&app.doc, &m);
    assert!(m.parent().unwrap().join("corbeille.json").is_file());
    for id in &cels {
        assert!(m.parent().unwrap().join("dessins").join(format!("{id}.png")).is_file() || m.parent().unwrap().join("dessins").join(format!("{id}.json")).is_file(), "dessin {id} gardé");
    }
    let loaded = project::load(&m).unwrap();
    assert_eq!(loaded.doc.corbeille.len(), 2);
    assert!(cels.iter().all(|c| !loaded.doc.cels.contains_key(c)), "aucun dessin de la corbeille relu à l'ouverture");
    // Les numéros neufs restent au-dessus de tout ce que la corbeille garde.
    let mut doc = loaded.doc;
    let haut = doc.corbeille.entrees.iter().map(|e| e.id_max).max().unwrap();
    assert!(doc.alloc() > haut);
    // Un nouvel enregistrement, sans que la corbeille soit en mémoire, garde ses fichiers.
    let w = enregistrer(&doc, &m);
    drop(w);
    for id in &cels {
        assert!(m.parent().unwrap().join("dessins").join(format!("{id}.png")).is_file() || m.parent().unwrap().join("dessins").join(format!("{id}.json")).is_file(), "dessin {id} toujours gardé");
    }
    // Restaurer après la réouverture : les dessins se relisent du dossier.
    let mut app2 = appli("rouvert");
    app2.doc = doc;
    app2.path = Some(m.clone());
    let plan = app2.doc.corbeille.entrees.iter().find(|e| e.sorte == Sorte::Plan).unwrap().id;
    restaurer(&mut app2, plan);
    assert_eq!(app2.doc.corbeille.len(), 1);
    assert_eq!(app2.doc.panels.len(), 4, "le plan est revenu : trois plans + un");
    let fond: Vec<Vec<Option<u64>>> = film_en_empreintes(&app2.doc)[1].clone();
    assert_eq!(fond, avant[1], "le plan restauré montre les mêmes dessins qu'avant sa suppression");
}

#[test]
fn an_older_project_opens_with_an_empty_bin_and_a_newer_one_is_refused() {
    assert_eq!(project::VERSION, 53);
    let d = dossier("version");
    let m = manifeste(&d);
    let app = appli("version");
    enregistrer(&app.doc, &m);
    assert!(!m.parent().unwrap().join("corbeille.json").exists(), "corbeille vide : pas de fichier");
    let texte = std::fs::read_to_string(&m).unwrap();
    assert!(texte.contains("\"version\": 53"));
    let ancien = texte.replace("\"version\": 53", "\"version\": 46");
    std::fs::write(&m, ancien).unwrap();
    let l = project::load(&m).expect("un projet 46 s'ouvre");
    assert!(l.doc.corbeille.is_empty());
    assert_eq!(l.doc.panels.len(), 4, "rien de perdu");
    let futur = texte.replace("\"version\": 53", "\"version\": 54");
    std::fs::write(&m, futur).unwrap();
    assert!(project::load(&m).is_err(), "un projet plus récent est refusé");
}

#[test]
fn emptying_the_bin_frees_the_files_at_the_next_save() {
    let d = dossier("vider");
    let m = manifeste(&d);
    let mut app = appli("vider");
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    let cels: Vec<u64> = app.doc.corbeille.cels().collect();
    enregistrer(&app.doc, &m);
    let fichier = |id: &u64| m.parent().unwrap().join("dessins").join(format!("{id}.png"));
    assert!(cels.iter().any(|c| fichier(c).is_file()));
    app.corbeille_supprimer(None);
    assert!(app.doc.corbeille.is_empty());
    assert!(app.status.contains("Corbeille vidée") || app.status.contains("définitivement"), "{}", app.status);
    enregistrer(&app.doc, &m);
    assert!(!m.parent().unwrap().join("corbeille.json").exists());
    assert!(cels.iter().all(|c| !fichier(c).is_file()), "les dessins de la corbeille vidée partent");
    // Vider est définitif : Ctrl+Z ne rend rien à la corbeille (sans dessins, il la rendrait vide de sens).
    annuler(&mut app);
    assert!(app.doc.corbeille.is_empty(), "vider ne s'annule pas");
}

#[test]
fn saving_elsewhere_carries_the_bin_files_that_are_not_in_memory() {
    let a = dossier("sous-a");
    let b = dossier("sous-b");
    let (ma, mb) = (manifeste(&a), manifeste(&b));
    let mut app = appli("sous");
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    let cels: Vec<u64> = app.doc.corbeille.cels().collect();
    enregistrer(&app.doc, &ma);
    let loaded = project::load(&ma).unwrap();
    assert!(cels.iter().all(|c| !loaded.doc.cels.contains_key(c)));
    let doc = loaded.doc;
    let mut written = loaded.written;
    let reuse = Reuse { manifest_path: &ma, written: &written.clone() };
    let job = project::prepare_save(&doc, &mb, &mut written, Some(reuse)).unwrap();
    job.run().unwrap();
    for id in &cels {
        let dessins = mb.parent().unwrap().join("dessins");
        assert!(dessins.join(format!("{id}.png")).is_file() || dessins.join(format!("{id}.json")).is_file(), "dessin {id} copié");
    }
    assert!(mb.parent().unwrap().join("corbeille.json").is_file());
}

#[test]
fn a_bin_file_this_version_cannot_read_is_kept_and_the_project_still_opens() {
    let d = dossier("illisible");
    let m = manifeste(&d);
    let mut app = appli("illisible");
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    enregistrer(&app.doc, &m);
    let cels: Vec<u64> = app.doc.corbeille.cels().collect();
    let f = m.parent().unwrap().join("corbeille.json");
    let texte = std::fs::read_to_string(&f).unwrap().replace("\"version\": 1", "\"version\": 99");
    std::fs::write(&f, &texte).unwrap();
    let l = project::load(&m).unwrap();
    assert!(l.doc.corbeille.is_empty());
    assert!(l.warnings.iter().any(|w| w.contains("corbeille")), "{:?}", l.warnings);
    // L'enregistrement suivant n'efface aucun dessin de ce que le fichier décrit, et le fichier reste intact.
    enregistrer(&l.doc, &m);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), texte, "le fichier reste là, intact");
    assert!(dessins_presents(m.parent().unwrap(), &cels).is_empty(), "les dessins de la corbeille illisible sont toujours dans dessins/");
}

fn empreintes_du_plan(doc: &Document, p: &crate::document::Panel) -> Vec<Vec<Option<u64>>> {
    p.layers.iter().map(|l| l.frames.iter().map(|f| f.as_ref().and_then(|id| doc.cels.get(id)).map(|c| crate::empreinte::Donnees::de(c).empreinte())).collect()).collect()
}

#[test]
fn a_deleted_plan_takes_its_stored_versions_to_the_bin_and_they_come_back_with_it() {
    let d = dossier("versions");
    let m = manifeste(&d);
    let mut app = appli("versions");
    let id = app.doc.panels[1].id;
    app.doc.nouvelle_version(1, 1_000).expect("une nouvelle version");
    assert_eq!(app.doc.nombre_de_versions(id), 2);
    let avant: Vec<_> = app.doc.versions[&id].autres.iter().map(|r| (r.numero, empreintes_du_plan(&app.doc, &r.panel))).collect();
    let actuelle = app.doc.versions[&id].actuelle;
    let film = film_en_empreintes(&app.doc);
    app.delete_panels(&[id]);
    assert!(!app.doc.versions.contains_key(&id), "les versions partent avec le plan");
    assert!(app.doc.corbeille.entrees[0].cels.len() > 5, "les dessins des versions sont gardés avec le plan");
    // Restaurer dans la séance.
    let e = app.doc.corbeille.entrees[0].id;
    restaurer(&mut app, e);
    assert_eq!(app.doc.nombre_de_versions(id), 2, "les versions reviennent avec lui");
    assert_eq!(film_en_empreintes(&app.doc), film);
    // Puis enregistrer, rouvrir, restaurer : les versions se relisent du dossier.
    annuler(&mut app);
    enregistrer(&app.doc, &m);
    let loaded = project::load(&m).unwrap();
    let mut app2 = appli("versions-rouvert");
    app2.doc = loaded.doc;
    app2.path = Some(m.clone());
    let e = app2.doc.corbeille.entrees[0].id;
    restaurer(&mut app2, e);
    let v = &app2.doc.versions[&id];
    assert_eq!(v.actuelle, actuelle);
    let apres: Vec<_> = v.autres.iter().map(|r| (r.numero, empreintes_du_plan(&app2.doc, &r.panel))).collect();
    assert_eq!(apres, avant, "la version rangée est la même, dessin par dessin");
    assert_eq!(film_en_empreintes(&app2.doc), film);
}

#[test]
fn a_key_paste_that_covers_drawings_sends_them_to_the_bin() {
    use crate::montagekeys::KeyRef;
    let mut app = appli("coller");
    let (panel, layer) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id);
    let avant = film_en_empreintes(&app.doc);
    app.key_sel = vec![KeyRef { panel, layer, start: 0 }];
    app.copy_keys();
    app.cur_frame = 3;
    app.cur_layer = 0;
    let n = app.history.undo_len();
    app.paste_keys_at_head();
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert_eq!(app.doc.corbeille.len(), 1, "le dessin recouvert est à la corbeille");
    assert!(app.status.contains("corbeille"), "{}", app.status);
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant);
    assert_eq!(app.doc.corbeille.len(), 0, "annuler : pas de double");
}

#[test]
fn the_rescue_copy_and_the_recovery_after_a_crash_keep_the_bin() {
    // Un projet enregistré dont la corbeille n'est pas en mémoire (plan supprimé avant), puis un second plan supprimé dans la séance :
    // la copie de secours porte les deux, dessins compris ; après un plantage (dossier de séance resté), la copie s'ouvre avec les deux.
    let d = dossier("secours");
    let racine = dossier("secours-racine");
    let m = manifeste(&d);
    let mut app = appli("secours");
    let avant = film_en_empreintes(&app.doc);
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    enregistrer(&app.doc, &m);
    let loaded = project::load(&m).unwrap();
    let mut app2 = appli("secours-ouvert");
    app2.doc = loaded.doc;
    app2.path = Some(m.clone());
    let ecrit = loaded.written;
    app2.delete_panels(&[ids[2]]);
    assert_eq!(app2.doc.corbeille.len(), 2);
    let rec = crate::recovery::Recovery::start(&racine).expect("séance de secours");
    let snap = crate::recovery::Snapshot { doc: &app2.doc, serial: 1, modified: true, title: "Essai".into(), project: Some(m.clone()), project_written: &ecrit };
    rec.copie_finale(snap);
    // (Plantage : le dossier de la séance reste, son verrou est rendu.)
    let orphelins = crate::recovery::orphans(&racine, None);
    assert_eq!(orphelins.len(), 1, "{orphelins:?}");
    let copie = &orphelins[0].manifest;
    let l = project::load(copie).expect("la copie de secours s'ouvre");
    assert_eq!(l.doc.corbeille.len(), 2, "la corbeille est dans la copie");
    assert_eq!(l.doc.panels.len(), 2);
    let dessins = copie.parent().unwrap().join("dessins");
    for id in l.doc.corbeille.cels() {
        assert!(dessins.join(format!("{id}.png")).is_file() || dessins.join(format!("{id}.json")).is_file(), "dessin {id} dans la copie");
    }
    let mut app3 = appli("secours-recupere");
    app3.doc = l.doc;
    app3.path = Some(copie.clone());
    while let Some(e) = app3.doc.corbeille.entrees.first().map(|e| e.id) {
        restaurer(&mut app3, e);
    }
    assert_eq!(app3.doc.panels.len(), 4);
    let mut a: Vec<_> = film_en_empreintes(&app3.doc);
    let mut b: Vec<_> = avant;
    a.sort();
    b.sort();
    assert_eq!(a, b, "les quatre plans, avec leurs dessins, après la récupération");
}
fn dessins_presents(dir: &std::path::Path, ids: &[u64]) -> Vec<u64> {
    let d = dir.join("dessins");
    ids.iter().copied().filter(|id| !(d.join(format!("{id}.png")).is_file() || d.join(format!("{id}.json")).is_file())).collect()
}

fn rouvert(m: &std::path::Path, nom: &str) -> App {
    let loaded = project::load(m).expect("ouverture");
    let mut app = appli(nom);
    app.doc = loaded.doc;
    app.path = Some(m.to_path_buf());
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

#[test]
fn emptying_cold_entries_saving_then_undoing_brings_back_no_entry_without_drawings() {
    let d = dossier("vider-froid");
    let m = manifeste(&d);
    let mut app = appli("vider-froid");
    let avant = film_en_empreintes(&app.doc);
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    enregistrer(&app.doc, &m);
    // Rouvert : l'entrée est « froide » (relue du fichier, dessins non lus) ; un second plan est supprimé dans la séance.
    let mut app2 = rouvert(&m, "vider-froid-2");
    app2.delete_panels(&[ids[2]]);
    let cels: Vec<u64> = app2.doc.corbeille.cels().collect();
    assert_eq!(app2.doc.corbeille.len(), 2);
    app2.corbeille_supprimer(None);
    enregistrer(&app2.doc, &m);
    assert!(!dessins_presents(m.parent().unwrap(), &cels).is_empty(), "vidée : leurs dessins partent à l'enregistrement");
    for _ in 0..3 {
        annuler(&mut app2);
        assert!(app2.doc.corbeille.is_empty(), "aucun Ctrl+Z ne rend une entrée de la corbeille vidée");
    }
    // Le plan supprimé dans la séance, lui, revient par Ctrl+Z avec ses dessins (en mémoire) : un nouvel enregistrement les écrit.
    enregistrer(&app2.doc, &m);
    let l = project::load(&m).unwrap();
    let apres = film_en_empreintes(&l.doc);
    assert_eq!(apres, vec![avant[0].clone(), avant[2].clone(), avant[3].clone()], "les trois plans du film, intacts");
    assert!(l.doc.corbeille.is_empty());
}

#[test]
fn delete_save_undo_restore_save_undo_loses_no_drawing_on_cold_entries() {
    let d = dossier("va-et-vient");
    let m = manifeste(&d);
    let mut app = appli("va-et-vient");
    let avant = film_en_empreintes(&app.doc);
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    enregistrer(&app.doc, &m);
    let mut app2 = rouvert(&m, "va-et-vient-2");
    let e = app2.doc.corbeille.entrees[0].id;
    let cels: Vec<u64> = app2.doc.corbeille.cels().collect();
    let sauver = |a: &App| enregistrer(&a.doc, &m);
    // Restaurer, enregistrer, Ctrl+Z (l'entrée revient, ses dessins sont en mémoire), enregistrer.
    restaurer(&mut app2, e);
    sauver(&app2);
    annuler(&mut app2);
    assert_eq!(app2.doc.corbeille.len(), 1);
    sauver(&app2);
    assert!(dessins_presents(m.parent().unwrap(), &cels).is_empty(), "aucun dessin de l'entrée n'a disparu du dossier");
    // Jeter un plan, enregistrer, Ctrl+Z, enregistrer.
    let plan3 = app2.doc.panels.last().unwrap().id;
    app2.delete_panels(&[plan3]);
    sauver(&app2);
    annuler(&mut app2);
    sauver(&app2);
    // Au bout : tout restaurer, enregistrer, rouvrir : les quatre plans, dessin par dessin.
    while let Some(e) = app2.doc.corbeille.entrees.first().map(|e| e.id) {
        restaurer(&mut app2, e);
    }
    sauver(&app2);
    let l = project::load(&m).unwrap();
    let mut a = film_en_empreintes(&l.doc);
    let mut b = avant;
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

#[test]
fn plans_deleted_together_come_back_in_film_order_whatever_the_restore_order() {
    for ordre in [[0usize, 1], [1, 0]] {
        let mut app = appli("ordre");
        let ids = ids_plans(&app);
        app.delete_panels(&[ids[1], ids[2]]);
        assert_eq!(app.doc.corbeille.len(), 2);
        let entrees: Vec<u64> = ids[1..3].iter().map(|p| app.doc.corbeille.entrees.iter().find(|e| e.plan == *p).unwrap().id).collect();
        for k in ordre {
            restaurer(&mut app, entrees[k]);
        }
        assert_eq!(ids_plans(&app), ids, "l'ordre du film, restauré dans l'ordre {ordre:?}");
    }
}

#[test]
fn saving_elsewhere_also_carries_the_textures_and_pasted_images_of_cold_drawings() {
    let a = dossier("tex-a");
    let b = dossier("tex-b");
    let (ma, mb) = (manifeste(&a), manifeste(&b));
    let mut app = appli("tex");
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    enregistrer(&app.doc, &ma);
    let src = ma.parent().unwrap();
    for (dossier, nom) in [("textures", "papier_1.png"), (crate::imagevec::DOSSIER, "abcdef.png")] {
        std::fs::create_dir_all(src.join(dossier)).unwrap();
        std::fs::write(src.join(dossier).join(nom), b"x").unwrap();
    }
    let loaded = project::load(&ma).unwrap();
    let doc = loaded.doc;
    let mut written = loaded.written;
    let reuse = Reuse { manifest_path: &ma, written: &written.clone() };
    project::prepare_save(&doc, &mb, &mut written, Some(reuse)).unwrap().run().unwrap();
    let dst = mb.parent().unwrap();
    assert!(dst.join("textures").join("papier_1.png").is_file(), "texture recopiée");
    assert!(dst.join(crate::imagevec::DOSSIER).join("abcdef.png").is_file(), "image collée recopiée");
}

#[test]
fn the_window_and_a_restore_never_freeze_the_interface_thread() {
    use std::time::{Duration, Instant};
    let d = dossier("fil");
    let m = manifeste(&d);
    let mut app = appli("fil");
    for _ in 0..100 {
        app.doc.jeter_plan(0, &[], "Plan supprimé");
    }
    assert_eq!(app.doc.corbeille.len(), 100);
    enregistrer(&app.doc, &m);
    let mut app2 = rouvert(&m, "fil-2");
    let ctx = eframe::egui::Context::default();
    app2.ouvrir_la_corbeille();
    let (mut pire, mut total) = (Duration::ZERO, Duration::ZERO);
    let debut = Instant::now();
    let mut images = 0;
    while debut.elapsed() < Duration::from_secs(20) {
        let t = Instant::now();
        let _ = ctx.run(Default::default(), |c| app2.corbeille_fenetre_ui(c));
        let dt = t.elapsed();
        pire = pire.max(dt);
        total += dt;
        images += 1;
        let fen = app2.corbeille_ui.as_ref().unwrap();
        if fen.vignettes.len() == 100 && fen.taille.is_some_and(|(k, _)| k == 100) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let fen = app2.corbeille_ui.as_ref().unwrap();
    assert!(fen.vignettes.len() == 100 && fen.taille.is_some(), "vignettes et taille arrivent");
    eprintln!("MESURE corbeille : 100 entrées, {images} images d'écran, la plus longue {pire:?}, total sur le fil de l'interface {total:?}");
    assert!(pire < Duration::from_millis(150), "une image d'écran ne dépasse pas 150 ms : {pire:?}");
    // Restaurer : l'appel rend la main tout de suite, les dessins se relisent sur un autre fil.
    let e = app2.doc.corbeille.entrees[0].id;
    let t = Instant::now();
    app2.corbeille_restaurer(e);
    let appel = t.elapsed();
    eprintln!("MESURE restaurer : l'appel sur le fil de l'interface {appel:?}");
    assert!(app2.corbeille_en_cours(), "la lecture se fait à part");
    assert!(appel < Duration::from_millis(50), "{appel:?}");
    while app2.corbeille_en_cours() {
        std::thread::sleep(Duration::from_millis(5));
        app2.corbeille_sonder();
    }
    assert_eq!(app2.doc.corbeille.len(), 99);
}