//! Le manifeste des projets s'appelle `.shot` depuis le 2026-10-23 (le logiciel s'appelle SHOT) : un projet neuf
//! s'enregistre en `.shot` ; un projet d'avant (`.pelure`) s'ouvre toujours et passe à `.shot` à son enregistrement
//! (l'ancien manifeste part après l'écriture du nouveau) ; un plantage entre les deux laisse un projet qui s'ouvre ;
//! les récents, la séance et les copies de secours suivent ; un film partagé garde son `.pelure`
//! (`cooperation_ui_tests.rs`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::perf_tests::{banc, Banc};
use crate::project;

fn petit_projet(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(k)[0] = Color32::from_rgba_premultiplied(200, 10, 10, 255);
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

fn dossier(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-extension-{}-{nom}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Un projet d'avant : `<dir>/Bus/Bus.pelure`, ses dessins et un fichier à côté que SHOT ne connaît pas.
fn ancien_projet(dir: &Path) -> PathBuf {
    let manifeste = dir.join("Bus").join("Bus.pelure");
    let mut doc = petit_projet(3);
    doc.panels[0].notes = "Plan d'avant.".into();
    project::save(&doc, &manifeste, &mut Default::default()).expect("projet d'avant");
    std::fs::write(manifeste.with_file_name("notes d'Anthony.txt"), "à garder").expect("fichier à côté");
    manifeste
}

/// Les fichiers du dossier du projet (chemin relatif → octets), sans les manifestes.
fn contenu_sans_manifeste(racine: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn parcourir(racine: &Path, d: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in std::fs::read_dir(d).expect("dossier").flatten() {
            let p = e.path();
            if p.is_dir() {
                parcourir(racine, &p, out);
            } else if !project::est_un_projet(&p) {
                out.insert(p.strip_prefix(racine).expect("relatif").to_path_buf(), std::fs::read(&p).expect("fichier"));
            }
        }
    }
    let mut out = BTreeMap::new();
    parcourir(racine, racine, &mut out);
    out
}

fn pomper_enregistrement(b: &mut Banc) {
    for _ in 0..600 {
        if !b.app.save_en_cours() {
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("l'enregistrement ne se termine pas");
}

fn ouvrir(b: &mut Banc, chemin: &Path) {
    b.app.open_path(chemin.to_path_buf());
    let t = Instant::now();
    while b.app.open_job.is_some() {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60), "l'ouverture ne finit pas");
    }
}

#[test]
fn the_project_extensions_and_names() {
    assert!(project::est_un_projet(Path::new(r"C:\F\Bus\Bus.shot")) && project::est_un_projet(Path::new(r"C:\F\Bus\Bus.PELURE")));
    assert!(!project::est_un_projet(Path::new(r"C:\F\Bus.sboard")) && !project::est_un_projet(Path::new(r"C:\F\Bus.json")));
    assert_eq!(project::manifeste_shot(Path::new(r"C:\F\Mon.film\Mon.film.pelure")), PathBuf::from(r"C:\F\Mon.film\Mon.film.shot"));
    assert_eq!(project::manifeste_shot(Path::new(r"C:\F\Bus\Bus.shot")), PathBuf::from(r"C:\F\Bus\Bus.shot"));
    assert_eq!(project::manifeste_jumeau(Path::new(r"C:\F\Bus\Bus.shot")), Some(PathBuf::from(r"C:\F\Bus\Bus.pelure")));
    assert_eq!(project::manifeste_jumeau(Path::new(r"C:\F\Bus.sboard")), None);
    // Les deux noms d'un projet sont le même projet (onglets, récents), la casse ignorée.
    assert_eq!(project::cle_de_projet(Path::new(r"C:\F\Bus\Bus.shot")), project::cle_de_projet(Path::new(r"c:\f\bus\BUS.pelure")));
    assert!(crate::onglets::meme_chemin(Path::new(r"C:\F\Bus\Bus.shot"), Path::new(r"C:\F\Bus\Bus.pelure")));
    assert!(!crate::onglets::meme_chemin(Path::new(r"C:\F\Bus\Bus.shot"), Path::new(r"C:\F\Car\Car.shot")));
    // « Enregistrer sous » et « Nouveau » : toujours un `.shot`, quelle que soit l'extension tapée.
    let base = PathBuf::from(r"C:\Films");
    for choisi in ["Bus.shot", "Bus.pelure", "Bus"] {
        assert_eq!(project::target_for(&base.join(choisi)), (base.join("Bus"), base.join("Bus").join("Bus.shot")), "{choisi}");
    }
    // Un film partagé garde le nom `.pelure` de sa structure (des équipiers ont peut-être une version d'avant).
    assert_eq!(project::target_for_partage(&base.join("Film.shot")), (base.join("Film"), base.join("Film").join("Film.pelure")));
}

#[test]
fn a_new_project_is_saved_as_shot() {
    let dir = dossier("neuf");
    let mut b = banc(petit_projet(2), false);
    let (_, manifeste) = project::target_for(&dir.join("Neuf.shot"));
    assert!(project::check_target(&manifeste, None).is_ok());
    assert!(b.app.save_to_blocking(manifeste.clone()));
    assert_eq!(b.app.path.as_deref(), Some(dir.join("Neuf").join("Neuf.shot").as_path()));
    assert!(manifeste.is_file() && !manifeste.with_extension("pelure").exists());
    assert_eq!(project::load(&manifeste).expect("lisible").doc.panels[0].length, 2);
    assert_eq!(b.app.settings.recent_projects.first(), Some(&manifeste), "en tête des récents");
    // Un second projet au même nom est refusé, que le premier s'appelle `.shot` ou `.pelure`.
    assert!(project::check_target(&manifeste, None).is_err());
    let ancien = dossier("neuf-ancien");
    let pelure = ancien_projet(&ancien);
    assert!(project::check_target(&pelure.with_extension("shot"), None).is_err(), "le même projet sous l'autre nom");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&ancien);
}

#[test]
fn an_old_pelure_project_opens_and_its_save_converts_only_the_manifest() {
    let dir = dossier("conversion");
    let pelure = ancien_projet(&dir);
    let shot = pelure.with_extension("shot");
    let racine = pelure.parent().expect("dossier").to_path_buf();
    let avant = contenu_sans_manifeste(&racine);
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    ouvrir(&mut b, &pelure);
    assert_eq!(b.app.path.as_ref(), Some(&pelure), "ouvert sous son nom d'avant : {}", b.app.status);
    assert_eq!(b.app.doc.panels[0].notes, "Plan d'avant.");
    // (Une séance d'avant et un récent à l'ancien nom.)
    assert_eq!(b.app.settings.recent_projects.first(), Some(&pelure));
    // Rien n'a changé : « Enregistrer » convertit quand même (c'est lui qui écrit le projet).
    assert!(b.app.save());
    pomper_enregistrement(&mut b);
    assert!(!b.app.modified && b.app.save_error.is_none(), "{}", b.app.status);
    assert!(shot.is_file(), "le nouveau manifeste");
    assert!(!pelure.exists(), "l'ancien manifeste est parti");
    assert_eq!(b.app.path.as_ref(), Some(&shot), "le chemin courant suit");
    assert!(b.app.status.contains("Bus.shot"), "{}", b.app.status);
    // Seul le manifeste a changé de nom : dessins, annexes et dossier n'ont pas bougé d'un octet.
    assert_eq!(contenu_sans_manifeste(&racine), avant);
    // Les récents (et le « dernier projet ») ne gardent que le nouveau nom.
    assert_eq!(b.app.settings.recent_projects.first(), Some(&shot));
    assert!(!b.app.settings.recent_projects.contains(&pelure));
    // Le projet se relit tel quel.
    let relu = project::load(&shot).expect("lisible");
    assert_eq!((relu.doc.panels[0].notes.as_str(), relu.doc.panels[0].length), ("Plan d'avant.", 3));
    // Un ancien chemin (raccourci, séance d'avant) mène au nouveau, et ne rouvre pas un second onglet.
    assert_eq!(project::manifeste_a_ouvrir(&pelure), shot);
    assert!(b.app.montrer_si_deja_ouvert(&pelure), "le même projet");
    // L'enregistrement suivant écrit au même endroit.
    b.app.doc.panels[0].notes = "Plan retouché.".into();
    b.app.modified = true;
    assert!(b.app.save());
    pomper_enregistrement(&mut b);
    assert_eq!(b.app.path.as_ref(), Some(&shot));
    assert_eq!(project::load(&shot).expect("lisible").doc.panels[0].notes, "Plan retouché.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_crash_between_writing_shot_and_removing_pelure_still_opens_the_newest() {
    let dir = dossier("plantage");
    let pelure = ancien_projet(&dir);
    let shot = pelure.with_extension("shot");
    // La conversion s'arrête juste après l'écriture (synchronisée) du `.shot` : les deux fichiers restent.
    let mut doc = project::load(&pelure).expect("projet d'avant").doc;
    doc.panels[0].notes = "Écrit dans le .shot.".into();
    let mut written = Default::default();
    project::PLANTER_AVANT_RETRAIT.with(|p| p.set(true));
    let r = project::prepare_save(&doc, &shot, &mut written, None).expect("cliché").retirer_apres(pelure.clone()).run();
    project::PLANTER_AVANT_RETRAIT.with(|p| p.set(false));
    assert!(r.is_err(), "le plantage simulé");
    assert!(shot.is_file() && pelure.is_file(), "les deux noms sont là");
    // Le plus récent s'ouvre, qu'on demande l'un ou l'autre nom.
    assert_eq!(project::manifeste_a_ouvrir(&pelure), shot);
    assert_eq!(project::manifeste_a_ouvrir(&shot), shot);
    assert_eq!(project::load(&project::manifeste_a_ouvrir(&pelure)).expect("lisible").doc.panels[0].notes, "Écrit dans le .shot.");
    // L'application aussi (un récent à l'ancien nom) ; son enregistrement efface le reste.
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    ouvrir(&mut b, &pelure);
    assert_eq!(b.app.path.as_ref(), Some(&shot), "{}", b.app.status);
    assert_eq!(b.app.doc.panels[0].notes, "Écrit dans le .shot.");
    b.app.modified = true;
    assert!(b.app.save());
    pomper_enregistrement(&mut b);
    assert!(shot.is_file() && !pelure.exists(), "le reste du plantage est effacé");
    // Si c'est le `.pelure` le plus récent (une version d'avant l'a enregistré entre-temps), c'est lui qui s'ouvre.
    std::fs::copy(&shot, &pelure).expect("copie");
    let plus_tard = std::time::SystemTime::now() + Duration::from_secs(60);
    std::fs::File::options().write(true).open(&pelure).expect("ouvert").set_modified(plus_tard).expect("date");
    assert_eq!(project::manifeste_a_ouvrir(&shot), pelure);
    // Ni l'un ni l'autre : le nom demandé (l'ouverture dira qu'il manque).
    let absent = dir.join("Absent").join("Absent.pelure");
    assert_eq!(project::manifeste_a_ouvrir(&absent), absent);
    assert!(!project::manifeste_present(&absent) && project::manifeste_present(&shot));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_recent_projects_keep_one_entry_per_project_under_its_new_name() {
    let mut list: Vec<PathBuf> = Vec::new();
    crate::recents::remember(&mut list, Path::new(r"C:\Films\Bus\Bus.pelure"));
    crate::recents::remember(&mut list, Path::new(r"C:\Films\Car\Car.pelure"));
    crate::recents::remember(&mut list, Path::new(r"C:\Films\Bus\Bus.shot"));
    assert_eq!(list, vec![PathBuf::from(r"C:\Films\Bus\Bus.shot"), PathBuf::from(r"C:\Films\Car\Car.pelure")]);
    // La présence d'un récent converti depuis : il est là (sous son nouveau nom).
    let dir = dossier("recents");
    let pelure = ancien_projet(&dir);
    let shot = pelure.with_extension("shot");
    std::fs::rename(&pelure, &shot).expect("converti");
    let ctx = eframe::egui::Context::default();
    let mut presence = crate::recents::Presence::default();
    let absent = dir.join("Parti").join("Parti.pelure");
    presence.refresh(&ctx, &[pelure.clone(), absent.clone()]);
    let t = Instant::now();
    while presence.exists(&absent) && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!presence.exists(&absent));
    assert!(presence.exists(&pelure), "converti : toujours là");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_session_reopens_a_project_converted_since_under_its_new_name() {
    let dir = dossier("seance");
    let pelure = ancien_projet(&dir);
    let shot = pelure.with_extension("shot");
    std::fs::rename(&pelure, &shot).expect("converti");
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    // (Au lancement : l'accueil, rien d'ouvert ni de modifié.)
    b.app.welcome = true;
    b.app.modified = false;
    b.app.settings.reopen_last = true;
    b.app.settings.session_tabs = vec![pelure.clone()];
    b.app.settings.session_active = 0;
    b.app.reopen_last_project();
    let t = Instant::now();
    while b.app.path.is_none() && t.elapsed() < Duration::from_secs(30) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(b.app.path.as_ref(), Some(&shot), "{}", b.app.status);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_recovery_copies_are_found_under_both_names_and_follow_a_converted_project() {
    let root = dossier("secours");
    let projets = dossier("secours-projets");
    let pelure = ancien_projet(&projets);
    let doc = project::load(&pelure).expect("projet").doc;
    // Une séance d'aujourd'hui (`secours.shot`) et une d'une version d'avant (`secours.pelure`).
    let copie = |seance: &str, nom: &str, copie_le: u64| {
        let d = root.join(seance);
        project::save(&doc, &d.join("projet").join(nom), &mut Default::default()).expect("copie");
        let info = crate::recovery::SessionInfo { title: "Bus".into(), project: Some(pelure.clone()), copied: Some(copie_le) };
        std::fs::write(d.join("seance.json"), serde_json::to_string(&info).expect("fiche")).expect("fiche");
    };
    let maintenant = crate::recovery::now_secs();
    copie("1-neuve", "secours.shot", maintenant + 3600);
    copie("2-ancienne", "secours.pelure", maintenant - 3600);
    let orphelines = crate::recovery::orphans(&root, None);
    assert_eq!(orphelines.len(), 2, "les deux séances sont proposées");
    assert!(orphelines.iter().any(|o| o.manifest.ends_with("secours.shot")) && orphelines.iter().any(|o| o.manifest.ends_with("secours.pelure")));
    for o in &orphelines {
        assert!(project::load(&o.manifest).is_ok());
    }
    // Le projet d'origine est passé à `.shot` depuis : sa date se lit sous le nouveau nom.
    let ancienne = orphelines.iter().find(|o| o.manifest.ends_with("secours.pelure")).expect("ancienne").clone();
    let neuve = orphelines.iter().find(|o| o.manifest.ends_with("secours.shot")).expect("neuve").clone();
    std::fs::rename(&pelure, pelure.with_extension("shot")).expect("converti");
    assert!(ancienne.project_is_newer(), "enregistré (sous `.shot`) après la copie");
    assert!(!neuve.project_is_newer(), "copie plus récente que le projet");
    // Ouvrir la copie : « Enregistrer » écrira dans le projet sous son nom d'aujourd'hui.
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    b.app.open_orphan(ancienne);
    assert_eq!(b.app.path.as_ref(), Some(&pelure.with_extension("shot")));
    // Une copie que fait la séance d'aujourd'hui s'appelle `secours.shot`.
    let mut r = crate::recovery::Recovery::start(&root).expect("séance");
    let snap = crate::recovery::Snapshot { doc: &b.app.doc, serial: 1, modified: true, title: "Bus".into(), project: b.app.path.clone(), project_written: &Default::default() };
    r.start_copy(snap);
    let t = Instant::now();
    while r.busy() && t.elapsed() < Duration::from_secs(30) {
        let snap = crate::recovery::Snapshot { doc: &b.app.doc, serial: 1, modified: true, title: "Bus".into(), project: b.app.path.clone(), project_written: &Default::default() };
        r.tick(snap, None, true);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(r.dir.join("projet").join("secours.shot").is_file());
    r.close();
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&projets);
}

#[test]
fn a_dropped_shot_or_pelure_file_is_a_project() {
    let dir = dossier("depose");
    let pelure = ancien_projet(&dir);
    let shot = pelure.with_extension("shot");
    std::fs::rename(&pelure, &shot).expect("converti");
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    assert!(b.app.import_dropped_project(&shot), "un .shot lâché s'ouvre");
    let t = Instant::now();
    while b.app.open_job.is_some() && t.elapsed() < Duration::from_secs(30) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(b.app.path.as_ref(), Some(&shot), "{}", b.app.status);
    // L'ancien nom lâché : c'est le même projet, son onglet est montré.
    assert!(b.app.import_dropped_project(&pelure));
    assert!(b.app.open_job.is_none() && b.app.status.contains("déjà ouvert"), "{}", b.app.status);
    assert!(!b.app.import_dropped_project(&dir.join("son.wav")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_old_manifest_is_never_removed_when_it_is_a_shared_structure_or_another_project() {
    let dir = dossier("garde");
    let pelure = ancien_projet(&dir);
    let shot = pelure.with_extension("shot");
    // Le nouveau manque : rien.
    assert!(!project::retirer_ancien_manifeste(&shot, &pelure) && pelure.is_file());
    std::fs::copy(&pelure, &shot).expect("copie");
    // Un autre projet, ou un autre dossier : rien.
    assert!(!project::retirer_ancien_manifeste(&shot, &dir.join("Bus").join("Car.pelure")));
    assert!(!project::retirer_ancien_manifeste(&shot, &dir.join("Bus.pelure")));
    // La structure d'un film partagé (des équipiers la lisent sous ce nom) : rien.
    let structure = crate::cooperation_tests::partager(&petit_projet(2), &dir.join("Partage"));
    assert!(project::est_une_structure_partagee(&structure));
    std::fs::copy(&structure, structure.with_extension("shot")).expect("copie");
    assert!(!project::retirer_ancien_manifeste(&structure.with_extension("shot"), &structure) && structure.is_file());
    // Le cas ordinaire : l'ancien part.
    assert!(!project::est_une_structure_partagee(&pelure));
    assert!(project::retirer_ancien_manifeste(&shot, &pelure) && !pelure.exists() && shot.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
