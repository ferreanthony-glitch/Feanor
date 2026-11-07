//! Un fichier annexe ou un manifeste qui a déjà les octets à écrire n'est pas réécrit (`project::write_sidecar`,
//! `SaveJob::run_with`) : un enregistrement où rien n'a changé ne touche aucun fichier, et un dossier partagé par Dropbox
//! ou Drive n'envoie plus rien de nouveau.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::enregistrement_cliche_tests::projet_riche;
use crate::fiabilite_fixture::dossier;
use crate::perf_tests::{banc, Banc};
use crate::project;
use crate::recovery::{Recovery, Snapshot};

/// Chaque fichier sous `dir` : sa date de modification et sa taille.
fn dates(dir: &Path) -> BTreeMap<String, (SystemTime, u64)> {
    fn parcourir(base: &Path, dir: &Path, out: &mut BTreeMap<String, (SystemTime, u64)>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                parcourir(base, &p, out);
            } else {
                let m = std::fs::metadata(&p).expect("métadonnées");
                out.insert(p.strip_prefix(base).expect("sous le dossier").to_string_lossy().replace('\\', "/"), (m.modified().expect("date"), m.len()));
            }
        }
    }
    let mut out = BTreeMap::new();
    parcourir(dir, dir, &mut out);
    out
}

/// Les fichiers dont la date ou la taille a changé, ou qui sont apparus ou partis.
fn touches(avant: &BTreeMap<String, (SystemTime, u64)>, apres: &BTreeMap<String, (SystemTime, u64)>) -> Vec<String> {
    avant.keys().chain(apres.keys()).filter(|k| avant.get(*k) != apres.get(*k)).cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect()
}

#[test]
fn an_unchanged_save_writes_no_file_and_a_changed_sidecar_only_itself() {
    let travail = dossier("identique");
    let mut doc = projet_riche(&travail);
    let manifeste = travail.join("Film").join("Film.pelure");
    let dir = manifeste.parent().expect("dossier").to_path_buf();
    let mut written = HashMap::new();
    project::save(&doc, &manifeste, &mut written).expect("premier");
    let avant = dates(&dir);
    std::thread::sleep(Duration::from_millis(30));
    let rapport = project::save(&doc, &manifeste, &mut written).expect("rien de modifié");
    assert_eq!((rapport.written, rapport.removed), (0, 0));
    assert_eq!(touches(&avant, &dates(&dir)), Vec::<String>::new(), "rien n'a changé : aucun fichier réécrit");
    // Un commentaire de plus : seul son fichier change.
    let plan = doc.panels[1].id;
    doc.add_comment(plan, 4, "Anthony", 11);
    std::thread::sleep(Duration::from_millis(30));
    project::save(&doc, &manifeste, &mut written).expect("un commentaire");
    assert_eq!(touches(&avant, &dates(&dir)), vec![crate::commentaires::FILE.to_string()]);
    assert!(project::load(&manifeste).expect("relu").warnings.is_empty());
    // L'ancienne façon (tout réécrire) touchait chaque annexe et le manifeste.
    let avant = dates(&dir);
    std::thread::sleep(Duration::from_millis(30));
    project::ecrire_toujours(true);
    let r = project::save(&doc, &manifeste, &mut written);
    project::ecrire_toujours(false);
    r.expect("comme avant");
    let reecrits = touches(&avant, &dates(&dir));
    assert!(reecrits.len() > 5 && reecrits.iter().any(|f| f == "Film.pelure"), "{reecrits:?}");
    let _ = std::fs::remove_dir_all(&travail);
}

#[test]
fn an_identical_sidecar_is_a_success_and_an_unreadable_one_is_still_set_aside() {
    let dir = dossier("identique-annexe");
    let f = dir.join(crate::commentaires::FILE);
    let lisible = |t: &str| t.starts_with('{');
    project::write_sidecar(&f, Some(b"{\"a\": 1}"), lisible).expect("premier");
    let date = std::fs::metadata(&f).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(30));
    project::write_sidecar(&f, Some(b"{\"a\": 1}"), lisible).expect("identique : un succès");
    assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), date, "pas réécrit");
    // Un fichier illisible que le neuf remplace est toujours mis de côté.
    std::fs::write(&f, b"abime").unwrap();
    project::write_sidecar(&f, Some(b"{\"a\": 2}"), lisible).expect("remplacé");
    assert_eq!(std::fs::read(dir.join(format!("{}.illisible", crate::commentaires::FILE))).unwrap(), b"abime");
    assert_eq!(std::fs::read(&f).unwrap(), b"{\"a\": 2}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_recovery_copy_of_an_unchanged_project_writes_no_file() {
    let root = dossier("identique-secours");
    let travail = dossier("identique-secours-projet");
    let doc = projet_riche(&travail);
    let aucun = HashMap::new();
    let snap = |serial| Snapshot { doc: &doc, serial, modified: true, title: "Film".into(), project: None, project_written: &aucun };
    let mut rec = Recovery::start(&root).expect("séance");
    let copier = |rec: &mut Recovery, serial: u64| {
        rec.start_copy(snap(serial));
        let t = std::time::Instant::now();
        while !rec.tick(snap(serial), None, false) {
            assert!(rec.error.is_none(), "{:?}", rec.error);
            std::thread::sleep(Duration::from_millis(5));
            assert!(t.elapsed() < Duration::from_secs(30));
        }
    };
    copier(&mut rec, 1);
    let copie = rec.dir.join("projet");
    let avant = dates(&copie);
    std::thread::sleep(Duration::from_millis(30));
    copier(&mut rec, 2);
    assert_eq!(touches(&avant, &dates(&copie)), Vec::<String>::new(), "la copie de secours n'a rien réécrit");
    rec.close();
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&travail);
}

fn pomper(b: &mut Banc) {
    for _ in crate::perf_tests::attente() {
        if !b.app.save_en_cours() {
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("l'enregistrement ne se termine pas");
}

/// Ctrl+S sans rien de modifié : la date de modification du projet (dans le manifeste, à la seconde) peut changer, rien
/// d'autre n'est réécrit.
#[test]
fn ctrl_s_with_nothing_changed_rewrites_at_most_the_manifest() {
    let travail = dossier("identique-interface");
    let manifeste = travail.join("Film").join("Film.shot");
    let mut b = banc(projet_riche(&travail), false);
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    assert!(b.app.save());
    pomper(&mut b);
    let dir = manifeste.parent().expect("dossier").to_path_buf();
    let avant = dates(&dir);
    std::thread::sleep(Duration::from_millis(30));
    assert!(b.app.save());
    pomper(&mut b);
    assert!(b.app.save_error.is_none(), "{}", b.app.status);
    let reecrits = touches(&avant, &dates(&dir));
    assert!(reecrits.iter().all(|f| f == "Film.shot"), "{reecrits:?}");
    let _ = std::fs::remove_dir_all(&travail);
}
