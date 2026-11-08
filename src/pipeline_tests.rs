//! Essais du pipeline, étape 1 : liste des plans (CSV, JSON), paquet de livraison, modèle d'épisode, lecture des anciens projets.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::document::Document;
use crate::editorial_tests::film;
use crate::exports_v1_tests::{dossier_neuf, ffmpeg_ou_saute};
use crate::listeplans::{self, Format, Profil, Reglages as ListeReglages, Separateur};
use crate::modeleepisode;
use crate::paquet::{self, PaquetJob, Reglages as PaquetReglages};
use crate::settings::Settings;

fn film_textes() -> Document {
    let mut doc = film(24);
    doc.properties.serie = "Brawlidays".into();
    doc.properties.episode = "2".into();
    doc.panels[0].action = "Léa dit « salut » ; puis rit.\nElle \"court\".".into();
    doc.panels[0].dialogue = "Léa : « Allons-y, vite ! »".into();
    doc.panels[2].action = "Un plan, sans virgule".into();
    doc
}

fn tous(doc: &Document) -> Vec<usize> {
    (0..doc.panels.len()).collect()
}

// ------------------------------------------------------------------------------------------ liste des plans

#[test]
fn the_kitsu_profile_has_the_import_headers_and_inclusive_frame_out() {
    let doc = film_textes();
    let l = listeplans::construire(&doc, &tous(&doc), 1001, None, false);
    assert_eq!(l.len(), 4);
    let csv = listeplans::csv(&l, Profil::Kitsu, Separateur::Virgule);
    let lignes: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(lignes[0], "Episode,Sequence,Name,Description,Nb Frames,Frame In,Frame Out,FPS");
    // Plans de 24, 12, 36 et 24 images, de suite à partir de 1001 ; la sortie est incluse.
    assert_eq!(lignes[2].split(',').skip(4).collect::<Vec<_>>(), ["12", "1025", "1036", "24"]);
    assert_eq!((l[0].entree, l[0].sortie, l[1].entree, l[1].sortie, l[3].entree, l[3].sortie), (1001, 1024, 1025, 1036, 1073, 1096));
    for x in &l {
        assert_eq!(x.sortie - x.entree + 1, x.images as i64, "Nb Frames = sortie - entrée + 1");
    }
    assert_eq!(l[0].episode, "2");
    assert_eq!(l[0].plan, "SQ01_PL001", "le nom suit le modèle de noms du projet");
    assert!(!csv.starts_with('\u{feff}'), "pas de marque d'ordre des octets pour Kitsu");
    assert!(csv.ends_with("\r\n"));
}

#[test]
fn start_frame_can_be_1_or_0() {
    let doc = film_textes();
    for depart in [1, 0, 1001] {
        let l = listeplans::construire(&doc, &tous(&doc), depart, None, false);
        assert_eq!(l[0].entree, depart);
        assert_eq!(l[1].entree, depart + 24);
    }
}

#[test]
fn csv_escapes_quotes_separators_and_newlines_and_keeps_accents() {
    let doc = film_textes();
    let l = listeplans::construire(&doc, &tous(&doc), 1001, None, false);
    for sep in [Separateur::Virgule, Separateur::PointVirgule] {
        let csv = listeplans::csv(&l, Profil::Generique, sep);
        // Lecture à la main du CSV (RFC 4180) : le tour complet rend les mêmes cellules.
        let lu = lire_csv(csv.trim_start_matches('\u{feff}'), sep.car());
        assert_eq!(lu[0], listeplans::Profil::Generique.en_tetes());
        assert_eq!(lu.len(), 5);
        let desc = lu[1].get(3).unwrap();
        assert_eq!(desc, "Léa dit « salut » ; puis rit. Elle \"court\".", "accents, point-virgule, guillemets : intacts");
        assert_eq!(lu[1].get(4).unwrap(), "Léa : « Allons-y, vite ! »");
        assert_eq!(lu[1].len(), 14);
        if sep == Separateur::PointVirgule {
            assert!(csv.starts_with('\u{feff}'), "l'Excel français lit mieux le point-virgule avec la marque");
        }
    }
    let kitsu = listeplans::csv(&l, Profil::Kitsu, Separateur::Virgule);
    assert!(kitsu.contains("\"Léa dit « salut » ; puis rit. Elle \"\"court\"\".\""), "{kitsu}");
    // Un plan dont la description contient une virgule est entre guillemets avec la virgule.
    assert!(listeplans::cellule_csv("a,b", ',') == "\"a,b\"" && listeplans::cellule_csv("a;b", ',') == "a;b" && listeplans::cellule_csv("a;b", ';') == "\"a;b\"");
}

/// Un lecteur de CSV minimal (guillemets doublés, séparateur et sauts de ligne dans les guillemets).
fn lire_csv(texte: &str, sep: char) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut cell) = (Vec::new(), Vec::new(), String::new());
    let (mut quoted, mut it) = (false, texte.chars().peekable());
    while let Some(c) = it.next() {
        match (c, quoted) {
            ('"', true) if it.peek() == Some(&'"') => {
                it.next();
                cell.push('"');
            }
            ('"', _) => quoted = !quoted,
            (c, false) if c == sep => row.push(std::mem::take(&mut cell)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            (c, _) => cell.push(c),
        }
    }
    rows
}

#[test]
fn json_has_the_headers_as_keys_and_real_numbers() {
    let doc = film_textes();
    let l = listeplans::construire(&doc, &tous(&doc), 1001, None, true);
    for profil in [Profil::Kitsu, Profil::Generique] {
        let v: serde_json::Value = serde_json::from_str(&listeplans::json(&doc, &l, profil, 1001)).unwrap();
        assert_eq!(v["format"], "shot-liste-des-plans");
        assert_eq!(v["episode"], "2");
        assert_eq!(v["projet"], "Brawlidays");
        let shots = v["shots"].as_array().unwrap();
        assert_eq!(shots.len(), 4);
        let cles: Vec<&str> = shots[0].as_object().unwrap().keys().map(String::as_str).collect();
        let mut attendues: Vec<&str> = profil.en_tetes().to_vec();
        attendues.sort_unstable();
        let mut cles = cles;
        cles.sort_unstable();
        assert_eq!(cles, attendues);
        let (fin, deb) = if profil == Profil::Kitsu { ("Frame Out", "Frame In") } else { ("frame_out", "frame_in") };
        assert_eq!(shots[0][deb], 1001);
        assert_eq!(shots[0][fin], 1024);
    }
    let v: serde_json::Value = serde_json::from_str(&listeplans::json(&doc, &l, Profil::Generique, 1001)).unwrap();
    assert_eq!(v["shots"][0]["thumbnail"], "vignettes/SQ01_PL001.png");
    assert_eq!(v["shots"][0]["duration_seconds"], 1.0);
    assert_eq!(v["shots"][0]["description"], "Léa dit « salut » ; puis rit.\nElle \"court\".");
}

#[test]
fn modified_since_send_and_the_file_name() {
    let doc = film_textes();
    let mut etats = std::collections::HashMap::new();
    etats.insert(doc.panels[1].id, crate::envois::Etat::Modifie);
    let l = listeplans::construire(&doc, &tous(&doc), 1001, Some(&etats), false);
    assert_eq!(l.iter().map(|x| x.modifie.as_str()).collect::<Vec<_>>(), ["", "Modifié", "", ""]);
    assert_eq!(listeplans::nom_de_fichier(&doc, "Mon film", Format::Csv), "Mon_film_EP2_plans.csv");
    let r = ListeReglages::default();
    assert_eq!((r.profil, r.depart), (Profil::Kitsu, 1001));
}

#[test]
fn a_studio_model_names_the_rows() {
    let mut doc = film_textes();
    doc.properties.modele_noms = crate::nommage::MODELES_PRETS[1].1.into();
    let l = listeplans::construire(&doc, &tous(&doc), 1001, None, false);
    assert_eq!(l[0].plan, "Brawlidays_EP02_SQ001_SH0001_v001");
}

// ------------------------------------------------------------------------------------------------- paquet

fn courir(job: &mut PaquetJob, doc: &Document) {
    let t = Instant::now();
    while job.running() {
        job.step(doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(240), "paquet trop long : {:?}", job.status().0);
    }
}

fn sans_ffmpeg() -> PaquetReglages {
    PaquetReglages { videos: false, animatique: false, ..PaquetReglages::default() }
}

fn fichiers_de(dir: &Path) -> Vec<PathBuf> {
    fn rec(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rec(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    rec(dir, &mut v);
    v.sort();
    v
}

#[test]
fn the_path_model_makes_folders_and_leaves_out_empty_tokens() {
    let mut doc = film_textes();
    assert_eq!(paquet::dossier_de(&doc, paquet::CHEMIN_DEFAUT, Some(0)).unwrap(), Path::new("Brawlidays").join("EP02").join("storyboard").join("SQ01"));
    assert_eq!(paquet::dossier_de(&doc, paquet::CHEMIN_DEFAUT, Some(3)).unwrap(), Path::new("Brawlidays").join("EP02").join("storyboard").join("SQ02"));
    // Sans plan : le morceau de la séquence est laissé de côté ; sans épisode, celui de l'épisode aussi.
    assert_eq!(paquet::dossier_de(&doc, paquet::CHEMIN_DEFAUT, None).unwrap(), Path::new("Brawlidays").join("EP02").join("storyboard"));
    doc.properties.episode.clear();
    assert_eq!(paquet::dossier_de(&doc, paquet::CHEMIN_DEFAUT, None).unwrap(), Path::new("Brawlidays").join("storyboard"));
    assert_eq!(paquet::dossier_de(&doc, ".", Some(0)).unwrap(), PathBuf::new());
    // Pas de remontée, pas de caractère interdit, pas de jeton de plan.
    assert_eq!(paquet::dossier_de(&doc, "../x/a:b", None).unwrap(), Path::new("x").join("a_b"));
    assert!(paquet::dossier_de(&doc, "{plan}", None).is_err());
    assert!(paquet::dossier_de(&doc, "{inconnu}", None).is_err());
}

#[test]
fn a_package_has_the_tree_a_manifest_with_right_hashes_and_leaves_nothing_else() {
    let doc = film_textes();
    let dest = dossier_neuf("paquet-arbre");
    let r = PaquetReglages { liste_json: true, ..sans_ffmpeg() };
    let mut job = PaquetJob::start(&doc, &r, 0, None, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    let rapport = job.outcome.take().unwrap().expect("paquet");
    let racine = dest.join("Brawlidays").join("EP02").join("storyboard");
    assert_eq!(rapport.racine, racine);
    assert!(rapport.manifeste.is_file());
    let rel: Vec<String> = fichiers_de(&dest).iter().map(|p| p.strip_prefix(&dest).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")).collect();
    assert_eq!(
        rel,
        [
            "Brawlidays/EP02/storyboard/Brawlidays_EP2_plans.csv",
            "Brawlidays/EP02/storyboard/Brawlidays_EP2_plans.json",
            "Brawlidays/EP02/storyboard/Brawlidays_EP2_storyboard.pdf",
            "Brawlidays/EP02/storyboard/SQ01/SQ01_PL001.png",
            "Brawlidays/EP02/storyboard/SQ01/SQ01_PL002.png",
            "Brawlidays/EP02/storyboard/SQ01/SQ01_PL003.png",
            "Brawlidays/EP02/storyboard/SQ02/SQ01_PL004.png".replace("SQ01_PL004", "SQ02_PL004").as_str(),
            "Brawlidays/EP02/storyboard/manifest.json",
        ]
    );
    // Le dossier de travail est parti.
    assert!(std::fs::read_dir(&dest).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with(".shot-paquet")));
    // Le manifeste : chaque fichier, avec son empreinte juste.
    let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(racine.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["format"], "shot-paquet");
    assert_eq!(m["projet"], "Brawlidays");
    assert_eq!(m["episode"], "2");
    assert_eq!(m["cadence"], 24);
    assert_eq!(m["shot"], env!("CARGO_PKG_VERSION"));
    assert!(m["date"].as_str().unwrap().ends_with('Z') && m["date"].as_str().unwrap().len() == 20);
    let fichiers = m["fichiers"].as_array().unwrap();
    assert_eq!(fichiers.len(), 7, "tout sauf le manifeste lui-même");
    let mut sortes = std::collections::BTreeSet::new();
    for f in fichiers {
        let chemin = racine.join(f["chemin"].as_str().unwrap());
        let octets = std::fs::read(&chemin).unwrap();
        assert_eq!(f["sha256"].as_str().unwrap(), crate::sha256::de_octets(&octets), "{}", chemin.display());
        assert_eq!(f["octets"].as_u64().unwrap(), octets.len() as u64);
        sortes.insert(f["sorte"].as_str().unwrap().to_string());
    }
    assert_eq!(sortes.into_iter().collect::<Vec<_>>(), ["liste", "pdf", "vignette"]);
    // Un projet sans envoi : pas de champ « envoi ».
    assert!(m.get("envoi").is_none());
    // La liste des plans du paquet est celle de la fenêtre, profil Kitsu.
    let csv = std::fs::read_to_string(racine.join("Brawlidays_EP2_plans.csv")).unwrap();
    assert!(csv.starts_with("Episode,Sequence,Name,"));
    // Une vignette est un vrai PNG.
    let png = std::fs::read(racine.join("SQ01").join("SQ01_PL001.png")).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn stopping_a_package_leaves_nothing() {
    let doc = film_textes();
    let dest = dossier_neuf("paquet-arret");
    let mut job = PaquetJob::start(&doc, &sans_ffmpeg(), 0, None, &dest, Vec::new()).unwrap();
    for _ in 0..3 {
        job.step(&doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(std::fs::read_dir(&dest).unwrap().count() > 0, "le dossier de travail existe pendant l'export");
    job.cancel();
    assert!(job.outcome.as_ref().unwrap().is_err());
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0, "rien ne reste");
    // Annuler une deuxième fois, ou lâcher la tâche, ne fait rien de plus.
    job.cancel();
    drop(job);
    // Fermer SHOT (lâcher la tâche) en cours de route : rien ne reste non plus.
    let mut job = PaquetJob::start(&doc, &sans_ffmpeg(), 0, None, &dest, Vec::new()).unwrap();
    job.step(&doc, Duration::from_millis(8));
    drop(job);
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0);
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn existing_files_are_kept_unless_replacing_is_asked_and_then_restored_on_failure() {
    let doc = film_textes();
    let dest = dossier_neuf("paquet-existant");
    let r = PaquetReglages { vignettes: false, pdf: false, ..sans_ffmpeg() };
    let racine = dest.join("Brawlidays").join("EP02").join("storyboard");
    std::fs::create_dir_all(&racine).unwrap();
    std::fs::write(racine.join("Brawlidays_EP2_plans.csv"), "vieux").unwrap();
    // Sans « remplacer » : refus, rien ne bouge, rien ne reste.
    let mut job = PaquetJob::start(&doc, &r, 0, None, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    let e = job.outcome.take().unwrap().unwrap_err();
    assert!(e.contains("existent déjà"), "{e}");
    assert_eq!(std::fs::read_to_string(racine.join("Brawlidays_EP2_plans.csv")).unwrap(), "vieux");
    assert_eq!(fichiers_de(&dest).len(), 1);
    assert!(std::fs::read_dir(&dest).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with(".shot-paquet")));
    // Avec « remplacer » : le fichier est remplacé.
    let r2 = PaquetReglages { remplacer: true, ..r };
    let mut job = PaquetJob::start(&doc, &r2, 0, None, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    job.outcome.take().unwrap().expect("paquet");
    assert!(std::fs::read_to_string(racine.join("Brawlidays_EP2_plans.csv")).unwrap().starts_with("Episode,"));
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn a_failing_move_puts_everything_back() {
    // Le dossier de la séquence 2 est un fichier : le rangement échoue à mi-chemin ; ce qui était rangé est retiré, l'ancien fichier remis.
    let doc = film_textes();
    let dest = dossier_neuf("paquet-defaire");
    let racine = dest.join("Brawlidays").join("EP02").join("storyboard");
    std::fs::create_dir_all(&racine).unwrap();
    std::fs::write(racine.join("SQ02"), "je suis un fichier").unwrap();
    std::fs::write(racine.join("Brawlidays_EP2_plans.csv"), "vieux").unwrap();
    let r = PaquetReglages { pdf: false, remplacer: true, ..sans_ffmpeg() };
    let mut job = PaquetJob::start(&doc, &r, 0, None, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    assert!(job.outcome.take().unwrap().is_err());
    assert_eq!(std::fs::read_to_string(racine.join("Brawlidays_EP2_plans.csv")).unwrap(), "vieux", "l'ancien fichier est remis");
    assert_eq!(std::fs::read_to_string(racine.join("SQ02")).unwrap(), "je suis un fichier");
    let tout: Vec<String> = fichiers_de(&dest).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(tout.len(), 2, "rien d'autre : {tout:?}");
    assert!(!racine.join("SQ01").exists(), "les dossiers créés sont retirés");
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn two_files_with_the_same_path_are_refused_before_anything_is_written() {
    let mut doc = film_textes();
    // Un modèle de noms sans numéro de plan : tous les plans d'une séquence portent le même nom.
    doc.properties.modele_noms = "SQ{sequence:02}".into();
    let dest = dossier_neuf("paquet-doublon");
    let r = PaquetReglages { vignettes: false, pdf: false, liste_csv: false, liste_json: false, ..sans_ffmpeg() };
    // (Sans rien à écrire : refusé tout de suite.)
    assert!(PaquetJob::start(&doc, &r, 0, None, &dest, Vec::new()).is_err());
    let prevus = vec![
        paquet::Prevu { sorte: paquet::Sorte::Vignette, panel: Some(0), nom: "a.png".into(), source: dest.join("x") },
        paquet::Prevu { sorte: paquet::Sorte::Vignette, panel: Some(1), nom: "A.png".into(), source: dest.join("y") },
    ];
    assert!(paquet::rangement(&doc, paquet::CHEMIN_DEFAUT, &prevus).unwrap_err().contains("même chemin"));
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn a_sequence_package_has_only_that_sequence_and_the_manifest_is_in_the_common_folder() {
    let doc = film_textes();
    let dest = dossier_neuf("paquet-sequence");
    let r = PaquetReglages { film_entier: false, pdf: false, liste_csv: false, ..sans_ffmpeg() };
    // Le plan courant : le dernier (séquence 2).
    let mut job = PaquetJob::start(&doc, &r, 3, None, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    let rapport = job.outcome.take().unwrap().expect("paquet");
    assert_eq!(rapport.racine, dest.join("Brawlidays").join("EP02").join("storyboard").join("SQ02"));
    assert_eq!(fichiers_de(&dest).len(), 2, "une vignette et le manifeste");
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn a_package_with_videos_and_the_animatic_uses_the_existing_export_tasks() {
    let Some((ffmpeg, enc)) = ffmpeg_ou_saute("paquet avec vidéos") else { return };
    let doc = film_textes();
    let dest = dossier_neuf("paquet-videos");
    let r = PaquetReglages { vignettes: false, pdf: false, liste_csv: false, ..PaquetReglages::default() };
    let mut job = PaquetJob::start(&doc, &r, 0, Some((ffmpeg, enc)), &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    let rapport = job.outcome.take().unwrap().expect("paquet");
    let racine = dest.join("Brawlidays").join("EP02").join("storyboard");
    assert_eq!(rapport.racine, racine);
    for (dossier, nom) in [("SQ01", "SQ01_PL001"), ("SQ01", "SQ01_PL002"), ("SQ01", "SQ01_PL003"), ("SQ02", "SQ02_PL004")] {
        let f = racine.join(dossier).join(format!("{nom}.mp4"));
        assert!(f.is_file() && std::fs::metadata(&f).unwrap().len() > 100, "{}", f.display());
    }
    assert!(racine.join("Brawlidays_EP2_animatique.mp4").is_file());
    let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(racine.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["fichiers"].as_array().unwrap().len(), 5);
    assert!(std::fs::read_dir(&dest).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with(".shot-paquet")));
    std::fs::remove_dir_all(&dest).ok();
}

#[test]
fn a_package_needs_ffmpeg_for_videos_and_a_content() {
    let doc = film_textes();
    let dest = dossier_neuf("paquet-ffmpeg");
    assert!(PaquetJob::start(&doc, &PaquetReglages::default(), 0, None, &dest, Vec::new()).unwrap_err().contains("ffmpeg"));
    assert!(PaquetJob::start(&doc, &PaquetReglages { videos: false, vignettes: false, pdf: false, animatique: false, liste_csv: false, liste_json: false, ..Default::default() }, 0, None, &dest, Vec::new()).is_err());
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0, "un refus n'écrit rien");
    std::fs::remove_dir_all(&dest).ok();
}

// ------------------------------------------------------------------------------------------ modèle d'épisode

#[test]
fn an_episode_template_round_trips_and_a_new_episode_has_the_same_settings_without_drawings() {
    let mut doc = film_textes();
    doc.properties.modele_noms = crate::nommage::MODELES_PRETS[1].1.into();
    doc.properties.author = "Anthony".into();
    doc.properties.copyright = "© 2026".into();
    doc.panels[0].layers[0].name = "Crayonné".into();
    doc.panels[0].layers[0].opacity = 0.5;
    let mut s = Settings::default();
    s.liste_plans.depart = 1;
    s.paquet.modele_chemin = "{projet}/SQ{sequence:02}".into();
    s.paquet.dossier = Some(PathBuf::from("C:\\quelque\\part"));
    let m = modeleepisode::depuis(&doc, &s, "Brawlidays, épisode type");
    // Aller-retour par le fichier des modèles.
    let chemin = modeleepisode::enregistrer(&m).unwrap();
    assert!(chemin.ends_with("Brawlidays, épisode type.episode.json"), "{}", chemin.display());
    assert_eq!(modeleepisode::lister(), ["Brawlidays, épisode type"]);
    let relu = modeleepisode::charger("Brawlidays, épisode type").unwrap();
    assert_eq!(relu, m);
    assert!(relu.paquet.dossier.is_none(), "le dossier du poste n'est pas dans le modèle");
    // Un modèle ne porte aucun dessin : pas de pixels, pas de chemin d'image.
    let texte = std::fs::read_to_string(&chemin).unwrap();
    assert!(!texte.contains("png") && !texte.contains("dessins"), "{texte}");
    assert!(texte.len() < 20_000);
    // Nouvel épisode.
    assert_eq!(relu.episode_suivant(), "3");
    let valeurs = relu.valeurs(&s.new_project.values);
    assert_eq!((valeurs.width, valeurs.height, valeurs.fps), (doc.frame[0], doc.frame[1], doc.fps));
    let mut neuf = crate::nouveau::build_document(&valeurs, "Épisode 3");
    relu.appliquer_au_projet(&mut neuf, &relu.episode_suivant());
    assert_eq!(neuf.properties.episode, "3");
    assert_eq!(neuf.properties.serie, "Brawlidays");
    assert_eq!(neuf.properties.modele_noms, doc.properties.modele_noms);
    assert_eq!(neuf.properties.author, "Anthony");
    assert_eq!(neuf.numbering_locked, doc.numbering_locked);
    assert_eq!(neuf.frame, doc.frame);
    assert_eq!(neuf.panels[0].layers.len(), doc.panels[0].layers.len());
    assert_eq!(neuf.panels[0].layers[0].name, "Crayonné");
    assert!((neuf.panels[0].layers[0].opacity - 0.5).abs() < 1e-6);
    assert!(neuf.panels[0].layers.iter().all(|l| l.frames.iter().all(Option::is_none)), "aucun dessin");
    assert_eq!(neuf.personnages.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), doc.personnages.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
    // Les réglages du poste suivent, sauf le dossier du paquet.
    let mut s2 = Settings::default();
    s2.paquet.dossier = Some(PathBuf::from("C:\\ailleurs"));
    relu.appliquer_aux_reglages(&mut s2);
    assert_eq!(s2.liste_plans.depart, 1);
    assert_eq!(s2.paquet.modele_chemin, "{projet}/SQ{sequence:02}");
    assert_eq!(s2.paquet.dossier, Some(PathBuf::from("C:\\ailleurs")));
    // Le nom du premier plan du nouvel épisode suit le modèle de noms.
    assert_eq!(neuf.nom_de_plan(0), "Brawlidays_EP03_SQ001_SH0001_v001");
    // Supprimer.
    modeleepisode::supprimer("Brawlidays, épisode type").unwrap();
    assert!(modeleepisode::lister().is_empty());
}

#[test]
fn episode_numbers_are_followed_and_unreadable_templates_are_refused() {
    let mut m = modeleepisode::depuis(&film_textes(), &Settings::default(), "x");
    for (e, suivant) in [("1", "2"), ("09", "10"), ("007", "008"), ("pilote", "pilote"), ("", "")] {
        m.episode = e.into();
        assert_eq!(m.episode_suivant(), suivant, "{e}");
    }
    assert!(modeleepisode::ModeleEpisode::lire("{ pas du json").is_err());
    assert!(modeleepisode::ModeleEpisode::lire("{\"format\":\"autre\"}").is_err());
    let mut futur = m.clone();
    futur.version = 99;
    assert!(modeleepisode::ModeleEpisode::lire(&futur.en_texte()).unwrap_err().contains("plus récente"));
}

// ------------------------------------------------------------------------------------------ anciens projets

#[test]
fn an_old_project_is_read_identically_and_a_new_one_keeps_the_production_fields() {
    let dir = dossier_neuf("projet-45");
    let manifeste = dir.join("Film.shot");
    // Un projet sans champ de production s'écrit sans ces champs (rien ne change pour lui) …
    let doc = film(24);
    let mut written = std::collections::HashMap::new();
    crate::project::save(&doc, &manifeste, &mut written).unwrap();
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(!texte.contains("modele_noms") && !texte.contains("\"serie\"") && !texte.contains("\"episode\""), "{texte}");
    assert!(texte.contains(&format!("\"version\": {}", crate::project::VERSION)) || texte.contains(&format!("\"version\":{}", crate::project::VERSION)));
    assert_eq!(crate::project::VERSION, 50);
    // … et un manifeste de version 44 s'ouvre à l'identique, avec les noms d'avant.
    let ancien = texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 44");
    assert_ne!(ancien, texte, "le numéro de version a été trouvé dans le manifeste");
    std::fs::write(&manifeste, ancien).unwrap();
    let l = crate::project::load(&manifeste).unwrap();
    assert!(l.doc.properties.modele_noms.is_empty() && l.doc.properties.serie.is_empty() && l.doc.properties.episode.is_empty());
    for p in 0..doc.panels.len() {
        assert_eq!(l.doc.nom_de_plan(p), doc.plan_full_ref(p));
        assert_eq!(l.doc.nom_de_plan(p), doc.nom_de_plan(p));
    }
    // Un projet qui porte la production la retrouve.
    let mut d2 = film_textes();
    d2.properties.modele_noms = "{projet}_{plan:03}".into();
    crate::project::save(&d2, &manifeste, &mut written).unwrap();
    let l = crate::project::load(&manifeste).unwrap();
    assert_eq!((l.doc.properties.serie.as_str(), l.doc.properties.episode.as_str(), l.doc.properties.modele_noms.as_str()), ("Brawlidays", "2", "{projet}_{plan:03}"));
    assert_eq!(l.doc.nom_de_plan(0), "Brawlidays_001");
    std::fs::remove_dir_all(&dir).ok();
}

// ----------------------------------------------------------------------------------------------- fenêtres

fn app_de(doc: Document, nom: &str) -> (eframe::egui::Context, crate::app::App, PathBuf) {
    let dir = dossier_neuf(nom);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    (ctx, app, dir)
}

fn image(ctx: &eframe::egui::Context, app: &mut crate::app::App) {
    let input = eframe::egui::RawInput { screen_rect: Some(eframe::egui::Rect::from_min_size(eframe::egui::pos2(0.0, 0.0), eframe::egui::vec2(1280.0, 900.0))), time: Some(0.0), ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        app.noms_window(ctx);
        app.liste_plans_window(ctx);
        app.paquet_window(ctx);
        app.modele_episode_window(ctx);
        app.export_hub_window(ctx);
    });
}

#[test]
fn the_four_new_windows_open_and_draw_from_their_commands() {
    let (ctx, mut app, dir) = app_de(film_textes(), "fenetres");
    for action in ["projet.noms_production", "fichier.exporter_liste_plans", "fichier.exporter_paquet", "projet.enregistrer_modele_episode"] {
        app.run_action(action);
    }
    assert!(app.noms_fenetre.is_some() && app.liste_plans.is_some() && app.paquet.is_some() && app.modele_episode.is_some());
    for _ in 0..3 {
        image(&ctx, &mut app);
    }
    // Le modèle de noms se change aussi par la fenêtre : une étape d'annulation, et les exports suivent.
    app.doc.properties.modele_noms = "{plan:03}".into();
    image(&ctx, &mut app);
    assert_eq!(crate::editorial::clip_name(&app.doc, 0), "001");
    // Le hub montre les nouvelles entrées.
    app.export_hub = true;
    image(&ctx, &mut app);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_new_episode_from_a_template_is_created_from_the_new_project_form() {
    let (ctx, mut app, dir) = app_de(film_textes(), "nouvel-episode");
    let _ = &ctx;
    let m = modeleepisode::depuis(&app.doc, &app.settings, "Type");
    modeleepisode::enregistrer(&m).unwrap();
    let mut d = crate::nouveauui::NewDocDialog::open(&app.settings.new_project);
    d.name = "EP3".into();
    d.folder = dir.clone();
    d.modele = Some(modeleepisode::charger("Type").unwrap());
    d.episode = "3".into();
    d.values = d.modele.as_ref().unwrap().valeurs(&d.values);
    assert!(app.create_project(&d));
    assert_eq!(app.doc.properties.episode, "3");
    assert_eq!(app.doc.properties.serie, "Brawlidays");
    assert_eq!(app.doc.frame, [64, 36]);
    assert_eq!(app.doc.panels.len(), 1, "un seul plan vide : rien n'est repris des dessins");
    assert!(app.doc.numbering_locked);
    assert!(dir.join("EP3").is_dir() || dir.join("EP3").with_extension("shot").exists() || fichiers_de(&dir).iter().any(|p| p.extension().is_some_and(|e| e == "shot")));
    std::fs::remove_dir_all(&dir).ok();
}
/// Produit les exemples du rapport (`SHOT_EXEMPLES` : dossier de sortie, `SHOT_PROJET_ESSAI` : le manifeste d'une copie du projet d'essai).
#[test]
#[ignore]
fn exemples_pour_le_rapport() {
    let (Some(out), Some(src)) = (std::env::var_os("SHOT_EXEMPLES"), std::env::var_os("SHOT_PROJET_ESSAI")) else { return };
    let out = PathBuf::from(out);
    let mut doc = crate::project::load(Path::new(&src)).expect("projet d'essai").doc;
    doc.properties.serie = "Essai effets".into();
    doc.properties.episode = "1".into();
    doc.properties.modele_noms = crate::nommage::MODELES_PRETS[1].1.into();
    let plans: Vec<usize> = (0..doc.panels.len()).collect();
    for (profil, nom) in [(Profil::Kitsu, "kitsu"), (Profil::Generique, "generique")] {
        let l = listeplans::construire(&doc, &plans, 1001, None, true);
        std::fs::write(out.join(format!("liste_plans_{nom}.csv")), listeplans::csv(&l, profil, Separateur::Virgule)).unwrap();
        std::fs::write(out.join(format!("liste_plans_{nom}.json")), listeplans::json(&doc, &l, profil, 1001)).unwrap();
    }
    let ff = ffmpeg_ou_saute("exemple de paquet");
    let r = PaquetReglages { liste_json: true, videos: ff.is_some(), animatique: ff.is_some(), ..PaquetReglages::default() };
    let dest = out.join("paquet");
    std::fs::create_dir_all(&dest).unwrap();
    let mut job = PaquetJob::start(&doc, &r, 0, ff, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    let rap = job.outcome.take().unwrap().expect("paquet");
    println!("PAQUET {} ({} fichiers, {} octets)", rap.racine.display(), rap.fichiers, rap.octets);
}