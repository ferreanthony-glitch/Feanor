//! Aller-retour d'un projet complet (`fiabilite_fixture.rs`) : enregistrer, rouvrir (lecture directe
//! et ouverture sur un autre fil, celle de l'application), enregistrer de nouveau, rouvrir. Chaque
//! fois, le projet relu est le même que celui qu'on a enregistré, pour tout ce qui est persistant
//! (voir `instantane` : les numéros de calques, les versions de dessins et les pixels en cache des
//! traits repartent de zéro, c'est leur définition).
//!
//! Trois chemins d'enregistrement : en une seule boucle (machine à un cœur), sur plusieurs fils
//! (le chemin courant) et par la copie de secours (écrite sur un autre fil, `recovery.rs`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::document::Document;
use crate::fiabilite_fixture::{arbre, dossier, instantane, meme_projet, projet_complet, racine};
use crate::project::{self, Loaded, Progress};
use crate::recovery::{Recovery, Snapshot};

/// Les chemins d'enregistrement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chemin {
    /// Tout le travail dans une seule boucle (`parallele::EN_SERIE`).
    Serie,
    /// Plusieurs fils.
    Parallele,
    /// La copie de secours : préparée sur le fil de l'interface, écrite sur un autre.
    Secours,
}

/// Pendant sa vie, le travail de ce fil se fait en une seule boucle (si `actif`).
struct EnSerie(bool);

impl EnSerie {
    fn pour(chemin: Chemin) -> EnSerie {
        let actif = chemin == Chemin::Serie;
        crate::parallele::EN_SERIE.with(|s| s.set(actif));
        EnSerie(actif)
    }
}

impl Drop for EnSerie {
    fn drop(&mut self) {
        if self.0 {
            crate::parallele::EN_SERIE.with(|s| s.set(false));
        }
    }
}

/// Enregistre `doc` dans le dossier de `manifeste` par ce chemin ; renvoie le manifeste à relire
/// (celui de la copie de secours n'est pas là où on le demande).
fn enregistrer(doc: &Document, manifeste: &Path, chemin: Chemin, written: &mut HashMap<u64, u64>) -> PathBuf {
    match chemin {
        Chemin::Serie | Chemin::Parallele => {
            project::save(doc, manifeste, written).expect("enregistrement");
            manifeste.to_path_buf()
        }
        Chemin::Secours => {
            let racine = manifeste.parent().expect("dossier").with_extension("secours");
            let mut rec = Recovery::start(&racine).expect("séance de secours");
            let rien = HashMap::new();
            let snap = |serial: u64| Snapshot { doc, serial, modified: true, title: "Essai".into(), project: None, project_written: &rien };
            rec.start_copy(snap(1));
            let t = Instant::now();
            while !rec.tick(snap(1), None, false) {
                assert!(rec.error.is_none(), "{:?}", rec.error);
                std::thread::sleep(Duration::from_millis(5));
                assert!(t.elapsed() < Duration::from_secs(60), "la copie de secours ne finit pas");
            }
            let manifeste = rec.dir.join("projet").join("secours.pelure");
            // (La séance est abandonnée telle quelle, comme après un plantage : le dossier reste.)
            drop(rec);
            manifeste
        }
    }
}

fn lire(manifeste: &Path) -> Loaded {
    let l = project::load(manifeste).expect("lecture");
    assert!(l.warnings.is_empty(), "avertissements à la lecture : {:?}", l.warnings);
    assert!(!l.converted, "un projet du format courant n'est pas converti");
    l
}

fn assert_meme(a: &str, b: &str, quoi: &str) {
    if let Err(e) = meme_projet(a, b) {
        panic!("{quoi} : le projet relu n'est pas celui qu'on a enregistré.\n{e}");
    }
}

fn aller_retour(chemin: Chemin) {
    let travail = dossier(&format!("ar-{chemin:?}"));
    let f = projet_complet(&travail);
    let sans = f.sans_pixels.clone();
    let voulu = instantane(&f.doc, &sans);
    let _serie = EnSerie::pour(chemin);

    // 1. Enregistrer, rouvrir (lecture directe).
    let a = travail.join("A").join("Film.pelure");
    let mut written = HashMap::new();
    let a = enregistrer(&f.doc, &a, chemin, &mut written);
    let mut l1 = lire(&a);
    assert_meme(&voulu, &instantane(&l1.doc, &sans), "première lecture");

    // 2. Enregistrer le projet relu ailleurs, rouvrir : le même projet, et les mêmes fichiers.
    let b = travail.join("B").join("Film.pelure");
    let b = enregistrer(&l1.doc, &b, chemin, &mut HashMap::new());
    let l2 = lire(&b);
    assert_meme(&voulu, &instantane(&l2.doc, &sans), "deuxième lecture (enregistré depuis un projet relu)");
    let (ta, tb) = (arbre(racine(&a)), arbre(racine(&b)));
    assert_eq!(ta.keys().collect::<Vec<_>>(), tb.keys().collect::<Vec<_>>(), "les mêmes fichiers dans les deux dossiers");
    for (nom, h) in &ta {
        // (Les fichiers des calques vidéo sont recopiés tels quels ; tout le reste est réécrit, et pareil.)
        assert_eq!(Some(h), tb.get(nom), "le fichier {nom} diffère après un aller-retour");
    }

    // 3. Enregistrer par-dessus, sans rien changer (chemin du projet ouvert) : rien ne bouge.
    if chemin != Chemin::Secours {
        let report = project::save(&l1.doc, &a, &mut l1.written).expect("réenregistrement");
        assert_eq!(report.removed, 0, "rien à effacer dans un projet inchangé");
        assert_eq!(arbre(racine(&a)), ta, "un enregistrement sans changement ne touche à rien");
        let l3 = lire(&a);
        assert_meme(&voulu, &instantane(&l3.doc, &sans), "troisième lecture (réenregistré sur place)");
    }

    // 4. La lecture rapide (celle de l'ouverture sur un autre fil) : même projet, sauf l'image des
    //    dessins vectoriels texturés, d'abord approchée (leur rendu exact arrive ensuite).
    let progress = Progress::default();
    let q = project::load_quick_progress(&a, &progress).expect("lecture rapide");
    assert!(q.warnings.is_empty(), "{:?}", q.warnings);
    assert!(!q.approx.is_empty(), "le projet d'essai a des traits texturés, lus en rendu rapide");
    assert_eq!(progress.done.load(std::sync::atomic::Ordering::Relaxed), progress.total.load(std::sync::atomic::Ordering::Relaxed));
    let mut sans_rapide = sans.clone();
    sans_rapide.extend(q.approx.iter().copied());
    assert_meme(&instantane(&f.doc, &sans_rapide), &instantane(&q.doc, &sans_rapide), "lecture rapide");
}

#[test]
fn a_full_project_survives_save_and_reopen_in_one_loop() {
    aller_retour(Chemin::Serie);
}

#[test]
fn a_full_project_survives_save_and_reopen_on_several_threads() {
    aller_retour(Chemin::Parallele);
}

#[test]
fn a_full_project_survives_the_recovery_copy() {
    aller_retour(Chemin::Secours);
}

/// L'application ouvre le projet sur un autre fil (`ouverture.rs`), attend les rendus exacts des
/// traits texturés, puis enregistre : le projet qu'elle tient est le même qu'au départ, et ce
/// qu'elle écrit se relit à l'identique.
#[test]
fn the_application_opens_and_saves_a_full_project_unchanged() {
    let travail = dossier("application");
    let f = projet_complet(&travail);
    let sans = f.sans_pixels.clone();
    let voulu = instantane(&f.doc, &sans);
    let a = travail.join("A").join("Film.pelure");
    project::save(&f.doc, &a, &mut HashMap::new()).expect("enregistrement");

    let mut b = crate::perf_tests::banc(Document::with_frame(160, 90, 24), false);
    b.app.open_path(a.clone());
    let t = Instant::now();
    while b.app.open_job.is_some() {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60), "l'ouverture ne finit pas");
    }
    assert!(b.app.status.starts_with("Ouvert"), "{}", b.app.status);
    assert_eq!(b.app.path.as_deref(), Some(a.as_path()));
    // Les rendus exacts des traits texturés arrivent en arrière-plan ; on les attend.
    let mut calme = 0;
    let t = Instant::now();
    while calme < 4 {
        b.image(Vec::new(), 1.0 / 60.0);
        if b.app.rejeu.has_pending() || !b.app.rejeu_deferred.is_empty() || !b.app.rejeu_again.is_empty() {
            calme = 0;
            std::thread::sleep(Duration::from_millis(5));
        } else {
            calme += 1;
        }
        assert!(t.elapsed() < Duration::from_secs(60), "les rendus exacts ne finissent pas");
    }
    assert_meme(&voulu, &instantane(&b.app.doc, &sans), "projet ouvert par l'application");
    assert!(!b.app.modified, "ouvrir un projet à jour ne le marque pas modifié");

    // « Enregistrer sous » un autre dossier, par l'application, puis relire.
    let c = travail.join("C").join("Film.pelure");
    b.app.written.clear();
    // (Par l'enregistrement en arrière-plan, dont on attend la fin.)
    assert!(b.app.save_to_blocking(c.clone()));
    let l = lire(&c);
    // (L'application date l'enregistrement dans les propriétés du projet : c'est le seul écart.)
    assert!(b.app.doc.properties.modified >= f.doc.properties.modified);
    assert_meme(&instantane(&b.app.doc, &sans), &instantane(&l.doc, &sans), "projet enregistré par l'application");
}
