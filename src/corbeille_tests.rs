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
use crate::bulles::vus;

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
    assert_eq!(project::VERSION, 61);
    let d = dossier("version");
    let m = manifeste(&d);
    let app = appli("version");
    enregistrer(&app.doc, &m);
    assert!(!m.parent().unwrap().join("corbeille.json").exists(), "corbeille vide : pas de fichier");
    let texte = std::fs::read_to_string(&m).unwrap();
    assert!(texte.contains("\"version\": 61"));
    let ancien = texte.replace("\"version\": 61", "\"version\": 46");
    std::fs::write(&m, ancien).unwrap();
    let l = project::load(&m).expect("un projet 46 s'ouvre");
    assert!(l.doc.corbeille.is_empty());
    assert_eq!(l.doc.panels.len(), 4, "rien de perdu");
    let futur = texte.replace("\"version\": 61", "\"version\": 62");
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
// ------------------------------------------------------------------ partie 207 : le numéro et la séquence d'un plan restauré

fn noms_des_plans(app: &App) -> Vec<String> {
    (0..app.doc.panels.len()).map(|i| crate::storyboard::panel_title(&app.doc, i)).collect()
}

fn entree_du_plan(app: &App, plan: u64) -> u64 {
    app.doc.corbeille.entrees.iter().find(|e| e.plan == plan).expect("le plan est dans la corbeille").id
}

#[test]
fn a_restored_plan_gets_its_locked_number_back_in_the_session_and_after_reopening() {
    for rouvrir in [false, true] {
        let nom = if rouvrir { "numero-rouvert" } else { "numero" };
        let mut app = appli(nom);
        app.doc.lock_numbering();
        let ids = ids_plans(&app);
        app.delete_panels(&[ids[1]]);
        assert_eq!(noms_des_plans(&app), ["Plan 1", "Plan 3", "Plan 4"], "un trou : les autres gardent leur numéro");
        if rouvrir {
            let d = dossier(nom);
            let m = manifeste(&d);
            enregistrer(&app.doc, &m);
            app = rouvert(&m, nom);
            assert!(app.doc.numbering_locked);
        }
        let e = entree_du_plan(&app, ids[1]);
        restaurer(&mut app, e);
        assert_eq!(noms_des_plans(&app), ["Plan 1", "Plan 2", "Plan 3", "Plan 4"], "il revient sous son numéro (rouvert : {rouvrir})");
        assert_eq!(app.status, "« Plan 2 » restauré à sa place.", "la phrase dit le vrai nom");
        assert_eq!(crate::storyboard::panel_long_title(&app.doc, 1), "Séquence 1 — plan 2", "ce qu'imprime le PDF");
        // Un plan neuf juste après lui : la règle d'insertion, depuis son numéro.
        let neuf = app.doc.add_scene_after(1);
        assert_eq!(crate::storyboard::panel_title(&app.doc, neuf), "Plan 2A");
    }
}

#[test]
fn a_restored_plan_whose_number_was_given_away_gets_a_new_one_and_the_message_says_so() {
    let mut app = appli("numero-pris");
    app.doc.lock_numbering();
    let ids = ids_plans(&app);
    app.delete_panels(&[ids[1]]);
    // « Renuméroter » (une remise à zéro voulue) donne le 2 au plan qui était le 3.
    app.doc.renumber();
    assert_eq!(noms_des_plans(&app), ["Plan 1", "Plan 2", "Plan 3"]);
    let e = entree_du_plan(&app, ids[1]);
        restaurer(&mut app, e);
    let noms = noms_des_plans(&app);
    assert_eq!(noms[1], "Plan 1A", "un numéro neuf, jamais celui d'un autre plan : {noms:?}");
    assert_eq!(&noms[2..], ["Plan 2", "Plan 3"], "les autres gardent le leur");
    assert_eq!(app.status, "« Plan 2 » restauré à sa place : son numéro est pris, il devient « Plan 1A ».");
}

#[test]
fn the_number_of_a_plan_in_the_bin_is_not_given_to_a_new_plan() {
    let mut app = appli("numero-reserve");
    app.doc.lock_numbering();
    let ids = ids_plans(&app);
    // Le dernier plan part : un plan neuf au bout du film aurait reçu « 4 », le numéro de celui de la corbeille.
    app.delete_panels(&[ids[3]]);
    let neuf = app.doc.add_scene_after(2);
    assert_eq!(crate::storyboard::panel_title(&app.doc, neuf), "Plan 4A");
    let e = entree_du_plan(&app, ids[3]);
        restaurer(&mut app, e);
    let i = app.doc.panels.iter().position(|p| p.id == ids[3]).unwrap();
    assert_eq!(crate::storyboard::panel_title(&app.doc, i), "Plan 4", "il retrouve le sien");
    // Le reste d'un plan de plusieurs cases : une case jetée, puis rendue, rejoint son plan (le même numéro, pas un plan à part).
    let mut app = appli("numero-case");
    app.doc.lock_numbering();
    app.doc.add_panel_after(1);
    let case = app.doc.panels[2].id;
    assert_eq!(noms_des_plans(&app), ["Plan 1", "Plan 2", "Plan 2", "Plan 3", "Plan 4"]);
    app.delete_panels(&[case]);
    let e = entree_du_plan(&app, case);
        restaurer(&mut app, e);
    assert_eq!(noms_des_plans(&app), ["Plan 1", "Plan 2", "Plan 2", "Plan 3", "Plan 4"]);
    assert_eq!(app.doc.panels[2].scene, app.doc.panels[1].scene, "la case est revenue dans son plan");
}

#[test]
fn the_last_plan_of_a_deleted_sequence_brings_its_sequence_back_at_its_place() {
    for rouvrir in [false, true] {
        let nom = if rouvrir { "sequence-rouverte" } else { "sequence" };
        let mut app = appli(nom);
        // Trois séquences : les quatre plans, puis « Poursuite » (un plan), puis une troisième (un plan).
        let i = app.doc.add_sequence_after(3);
        let j = app.doc.add_sequence_after(i);
        let (seq2, seq3) = (app.doc.sequence_of(i), app.doc.sequence_of(j));
        app.doc.sequence_mut(seq2).unwrap().name = "Poursuite".into();
        app.doc.lock_numbering();
        let plan = app.doc.panels[i].id;
        let avant_plans = noms_des_plans(&app);
        let avant_seqs: Vec<String> = app.doc.sequence_order().iter().map(|&q| app.doc.sequence_code(q)).collect();
        assert_eq!(avant_seqs, ["1", "2", "3"]);
        app.delete_panels(&[plan]);
        assert!(!app.doc.sequences.iter().any(|s| s.id == seq2), "la séquence est partie avec son dernier plan");
        assert_eq!(app.doc.sequence_order(), [app.doc.sequence_of(0), seq3]);
        // (Une séquence neuve au bout du film ne prend pas le numéro de celle de la corbeille.)
        if rouvrir {
            let d = dossier(nom);
            let m = manifeste(&d);
            enregistrer(&app.doc, &m);
            app = rouvert(&m, nom);
        }
        app.cur_panel = 0;
        let e = entree_du_plan(&app, plan);
        restaurer(&mut app, e);
        assert_eq!(app.doc.sequence_order(), [app.doc.sequence_of(0), seq2, seq3], "elle revient entre ses voisines (rouvert : {rouvrir})");
        let s = app.doc.sequences.iter().find(|s| s.id == seq2).unwrap();
        assert_eq!(s.name, "Poursuite", "avec son nom");
        assert_eq!(app.doc.sequence_code(seq2), "2", "et son numéro");
        assert_eq!(noms_des_plans(&app), avant_plans, "le plan sous son numéro");
        assert_eq!(app.status, "« Plan 5 » restauré avec sa séquence (« Poursuite »), à sa place dans le film.");
        annuler(&mut app);
        assert!(!app.doc.sequences.iter().any(|s| s.id == seq2), "une seule étape d'annulation, séquence comprise");
        assert_eq!(app.doc.corbeille.len(), 1);
    }
}

/// Partie 207 : la vignette d'une entrée se fait comme celle d'un bloc de la timeline (la partie filmée, chaque pixel la moyenne de ce
/// qu'il couvre) : un zigzag noir de 6 pixels sur une feuille de 3840 × 2160 s'y lit, comme dans la timeline (avant : presque blanc).
#[test]
fn the_bin_thumbnail_is_made_like_the_timeline_thumbnail_and_a_thin_line_shows() {
    use crate::document::{Cel, Layer, LayerKind};
    use eframe::egui::Color32;
    let mut doc = Document::new(3840, 2160, 24);
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    for x in 200..3640usize {
        let creneau = 300 - ((x % 600) as i64 - 300).abs();
        let y0 = (900 + creneau) as usize;
        for y in y0..y0 + 6 {
            let i = cel.raster.tile_index_of(x, y);
            cel.raster.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = Color32::BLACK;
        }
    }
    let id = doc.add_cel(cel.clone());
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    l.frames = vec![Some(id); doc.panels[0].length];
    doc.panels[0].layers = vec![l];
    let (w, h) = (192, 108);
    let timeline = crate::render::thumbnail(&doc, &crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper)), &doc.panel_camera(0, 0), w, h);
    let corbeille = crate::corbeilleui::miniature(&cel, &crate::project::Tete::de(&doc), doc.paper, w, h);
    assert_eq!(corbeille.size, timeline.size);
    assert!(corbeille.pixels == timeline.pixels, "la même image que la vignette de la timeline");
    // Le trait couvre 0,3 pixel de vignette en épaisseur : un gris franc, d'un bout à l'autre (172 colonnes de vignette).
    let fonces = corbeille.pixels.iter().filter(|c| c.r() < 230).count();
    let le_plus_fonce = corbeille.pixels.iter().map(|c| c.r()).min().unwrap();
    // Avant : un échantillon de la feuille entière par point de 96 × 54.
    let avant = (0..54usize).flat_map(|y| (0..96usize).map(move |x| (x, y))).filter(|&(x, y)| cel.raster.get(x * 3840 / 96, y * 2160 / 54).a() > 0).count();
    eprintln!("MESURE vignette de la corbeille : {fonces} pixels plus foncés que 230, le plus foncé {le_plus_fonce} ; avant : {avant} points touchés");
    assert!(fonces >= 160, "le trait se lit sur toute sa longueur : {fonces} pixels");
    assert!(le_plus_fonce < 200, "{le_plus_fonce}");
}

/// Partie 207 : Échap ferme la fenêtre de la corbeille, comme les autres fenêtres (dans l'application entière, pas seulement dans sa
/// fonction) ; pendant une confirmation, il l'abandonne d'abord.
#[test]
fn escape_closes_the_bin_window_and_first_cancels_a_confirmation() {
    use eframe::egui::{Event, Key, Modifiers};
    let echap = || vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    let mut b = crate::perf_tests::banc(crate::montage_sans_casse_tests::film(), false);
    b.app.welcome = false;
    b.chauffer(2);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.delete_panels(&[ids[1]]);
    b.app.run_action("fichier.corbeille");
    b.chauffer(2);
    assert!(b.app.corbeille_ui.is_some(), "la fenêtre est ouverte");
    b.app.corbeille_ui.as_mut().unwrap().confirmation = Some(crate::corbeilleui::Confirmation::Vider);
    b.image(echap(), 1.0 / 60.0);
    assert!(b.app.corbeille_ui.as_ref().is_some_and(|f| f.confirmation.is_none()), "Échap abandonne la confirmation");
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(b.app.corbeille_ui.is_none(), "Échap ferme la fenêtre");
    assert_eq!(b.app.doc.corbeille.len(), 1, "rien n'est vidé");
    assert_eq!(b.app.doc.panels.len(), 3, "et rien d'autre ne bouge");
}

/// Partie 207 (vu à l'écran après la partie 207) : le bloc du dernier plan de la Séquence 2 choisi dans la timeline, supprimé, rendu par
/// la Corbeille (alors vide) : Échap, trois fois, ne fermait plus la fenêtre. La cause : une liste déroulante qu'egui croyait encore
/// ouverte alors que plus rien ne la peignait (un champ de couleur dont la fenêtre ou le panneau a disparu) ; `echap_ferme` prenait la
/// touche et ne fermait rien, à chaque appui. Maintenant la Corbeille se ferme au premier Échap, avant tout autre usage de la touche ; et
/// l'Export (la fenêtre « Exporter » et celle d'un export) de même.
#[test]
fn escape_closes_the_bin_window_after_restoring_a_chosen_plan_block_even_with_a_forgotten_popup() {
    use eframe::egui::{Event, Id, Key, Modifiers};
    let echap = || vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    let oubliee = |b: &crate::perf_tests::Banc| b.ctx.memory_mut(|m| m.open_popup(Id::new("liste dont le champ a disparu")));
    let mut doc = crate::timelinesb_tests::film();
    doc.add_sequence_after(2);
    let mut b = crate::timelinesb_tests::banc_sb(doc, 1600, 1000, false);
    let blocs = crate::timelinesb_tests::blocs();
    assert_eq!(blocs.len(), 4, "{blocs:?}");
    crate::pistes_tests::clic(&mut b, blocs[3].center(), Modifiers::NONE);
    let plan = b.app.doc.panels[3].id;
    b.app.run_action("edition.supprimer");
    b.chauffer(2);
    assert_eq!((b.app.doc.panels.len(), b.app.doc.sequence_order().len()), (3, 1), "le plan 4 est parti avec sa séquence : {}", b.app.status);
    b.app.run_action("fichier.corbeille");
    b.chauffer(2);
    // (Le vrai bouton « Restaurer » de la fenêtre.)
    let r = vus::ou("Restaurer");
    assert_eq!(r.len(), 1, "{r:?}");
    crate::pistes_tests::clic(&mut b, r[0].center(), Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.doc.sequence_order().len(), 2, "la séquence est revenue : {}", b.app.status);
    assert!(b.app.corbeille_ui.is_some() && b.app.doc.corbeille.is_empty(), "la fenêtre ouverte, vide");
    let i = b.app.doc.panels.iter().position(|p| p.id == plan).unwrap();
    crate::pistes_tests::clic(&mut b, crate::timelinesb_tests::blocs()[i].center(), Modifiers::NONE);
    assert!(b.app.corbeille_ui.is_some(), "le bloc rendu choisi, la fenêtre toujours ouverte");
    let choix = (b.app.board_sel.clone(), b.app.key_sel.clone(), b.app.cur_panel);
    oubliee(&b);
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(b.app.corbeille_ui.is_none(), "le premier Échap ferme la Corbeille");
    assert_eq!((b.app.board_sel.clone(), b.app.key_sel.clone(), b.app.cur_panel), choix, "et ne fait rien d'autre");
    assert!(!b.ctx.memory(|m| m.any_popup_open()), "la liste oubliée est fermée");
    // Même chose pour l'Export : la fenêtre « Exporter », puis celle d'un export.
    b.app.run_action("fichier.exporter");
    b.chauffer(2);
    assert!(b.app.export_hub);
    oubliee(&b);
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(!b.app.export_hub, "Échap ferme « Exporter »");
    b.app.run_action("fichier.exporter_pdf");
    b.chauffer(2);
    assert!(b.app.export_dialog.is_some());
    oubliee(&b);
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(b.app.export_dialog.is_none(), "Échap ferme la fenêtre de l'export");
    // Les Préférences, qui ne se fermaient pas du tout à Échap.
    b.app.run_action("fichier.preferences");
    b.chauffer(2);
    assert!(b.app.prefs_window);
    oubliee(&b);
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(!b.app.prefs_window, "Échap ferme les Préférences");
}

/// Partie 209 (vu à l'écran) : dans la fenêtre d'export du film, la liste de la cadence ouverte, un seul Échap fermait la liste **et** la
/// fenêtre (egui ferme la liste en la peignant, avant que la fenêtre ne demande si une liste est à l'écran). Maintenant le premier Échap ne
/// ferme que la liste, le second la fenêtre.
#[test]
fn escape_with_an_open_list_in_the_export_window_closes_only_the_list() {
    use eframe::egui::{Event, Key, Modifiers};
    let echap = || vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    let mut b = crate::perf_tests::banc(crate::montage_sans_casse_tests::film(), false);
    b.app.welcome = false;
    b.chauffer(2);
    b.app.run_action("fichier.exporter_film");
    b.chauffer(2);
    assert!(b.app.export_dialog.is_some(), "la fenêtre de l'export du film est ouverte");
    if vus::ou("Liste : cadence du film").is_empty() {
        let p = vus::ou("Précis : Exporter");
        assert_eq!(p.len(), 1, "{:?}", vus::noms());
        crate::pistes_tests::clic(&mut b, p[0].center(), Modifiers::NONE);
        b.chauffer(2);
    }
    let l = vus::ou("Liste : cadence du film");
    assert_eq!(l.len(), 1, "{:?}", vus::noms());
    crate::pistes_tests::clic(&mut b, l[0].center(), Modifiers::NONE);
    b.chauffer(2);
    assert!(crate::exportui::liste_a_l_ecran(&b.ctx), "la liste de la cadence est ouverte, à l'écran");
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(!b.ctx.memory(|m| m.any_popup_open()), "le premier Échap ferme la liste");
    assert!(b.app.export_dialog.is_some(), "et laisse la fenêtre ouverte");
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(b.app.export_dialog.is_none(), "le second Échap ferme la fenêtre");
}

/// Partie 209 (vu à l'écran) : la fenêtre « Raccourcis clavier », ouverte depuis les Préférences et posée par-dessus, ne se fermait pas à
/// Échap ; c'étaient les Préférences, dessous, qui se fermaient. Maintenant Échap ferme d'abord la liste des raccourcis, puis les
/// Préférences.
#[test]
fn escape_closes_the_shortcuts_list_before_the_preferences_under_it() {
    use eframe::egui::{Event, Key, Modifiers};
    let echap = || vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    let mut b = crate::perf_tests::banc(crate::montage_sans_casse_tests::film(), false);
    b.app.welcome = false;
    b.chauffer(2);
    b.app.run_action("fichier.preferences");
    b.chauffer(2);
    b.app.show_help = true;
    b.chauffer(2);
    assert!(b.app.prefs_window && b.app.show_help, "les Préférences et, par-dessus, la liste des raccourcis");
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(!b.app.show_help, "le premier Échap ferme la liste des raccourcis");
    assert!(b.app.prefs_window, "et laisse les Préférences ouvertes");
    b.image(echap(), 1.0 / 60.0);
    b.chauffer(1);
    assert!(!b.app.prefs_window, "le second Échap ferme les Préférences");
}
