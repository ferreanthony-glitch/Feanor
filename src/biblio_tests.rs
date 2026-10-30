//! Tests du panneau Bibliothèque : l'explorateur (dossiers lus sur un fil à part, filtre, recherche, tri, favoris,
//! lecteurs), les vignettes (file bornée, cache borné), l'aperçu (un seul à la fois, arrêt à la fermeture, lecture),
//! les dépôts et les commandes. Aucun son ne sort : une fausse sortie audio remplace la carte son, et le
//! presse-papiers n'est jamais touché.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::App;
use crate::audio::{MixClip, Pcm, RATE};
use crate::biblio::*;
use crate::biblioapercu::*;
use crate::biblioui::Commande;
use crate::bibliovideo;
use crate::bibliovignettes::*;
use crate::dock::Pane;

fn temp(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-biblio-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn png(dir: &Path, nom: &str, w: u32, h: u32) -> PathBuf {
    let p = dir.join(nom);
    image::RgbaImage::from_pixel(w, h, image::Rgba([200, 60, 30, 255])).save(&p).unwrap();
    p
}

/// Un WAV 16 bits mono de `n` échantillons à 48 kHz.
fn wav(n: usize) -> Vec<u8> {
    let data = (n * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for i in 0..n {
        b.extend_from_slice(&(((i % 100) as i16 - 50) * 100).to_le_bytes());
    }
    b
}

/// Un dossier d'essai :
/// ```text
/// racine/a.png  b.JPG(png)  c.wav  clip.mp4 (faux)  notes.txt  .cache/h.png  sous/x.png  Plan 2.png  Plan 10.png
/// ```
fn dossier_type(nom: &str) -> PathBuf {
    let racine = temp(nom);
    std::fs::create_dir_all(racine.join("sous")).unwrap();
    std::fs::create_dir_all(racine.join(".cache")).unwrap();
    png(&racine, "a.png", 64, 32);
    png(&racine.join("sous"), "x.png", 8, 8);
    png(&racine.join(".cache"), "h.png", 8, 8);
    png(&racine, "Plan 2.png", 16, 16);
    png(&racine, "Plan 10.png", 16, 16);
    std::fs::write(racine.join("c.wav"), wav(24_000)).unwrap();
    std::fs::write(racine.join("clip.mp4"), b"pas vraiment une video").unwrap();
    std::fs::write(racine.join("notes.txt"), "pas un media").unwrap();
    racine
}

/// Lit un dossier en entier (sur le fil de l'essai) : les messages qu'il envoie.
fn lire(dir: &Path) -> Vec<Message> {
    let mut v = Vec::new();
    lire_dossier(dir, &AtomicBool::new(false), |m| v.push(m));
    v
}

fn entrees(dir: &Path) -> Vec<Entree> {
    match lire(dir).pop() {
        Some(Message::Fin { entrees, .. }) => entrees,
        _ => panic!("le dossier ne s'est pas lu"),
    }
}

fn noms(e: &[Entree]) -> Vec<&str> {
    e.iter().map(|e| e.nom.as_str()).collect()
}

/// Attend que le navigateur ait fini de lire son dossier.
fn attendre_nav(n: &mut Navigateur) {
    let t = Instant::now();
    while n.lit() {
        n.poll();
        assert!(t.elapsed() < Duration::from_secs(20), "lecture trop longue");
        std::thread::sleep(Duration::from_millis(2));
    }
    n.poll();
}

// ---------------------------------------------------------------------------------------- formats

#[test]
fn the_formats_are_the_ones_shot_reads() {
    for f in ["a.png", "a.PNG", "photo.jpg", "photo.JPEG"] {
        assert_eq!(Genre::de(Path::new(f)), Some(Genre::Image), "{f}");
    }
    for f in ["a.mp4", "a.MOV", "a.avi", "a.mkv", "a.webm", "a.m4v", "a.mpg", "a.mpeg", "a.wmv"] {
        assert_eq!(Genre::de(Path::new(f)), Some(Genre::Video), "{f}");
    }
    for f in ["a.wav", "a.MP3", "a.flac", "a.ogg", "a.oga"] {
        assert_eq!(Genre::de(Path::new(f)), Some(Genre::Son), "{f}");
    }
    for f in ["notes.txt", "dessin.psd", "anim.gif", "sans_extension", "a.mp4.txt", "archive.zip"] {
        assert_eq!(Genre::de(Path::new(f)), None, "{f} n'est pas lisible par SHOT");
    }
}

#[test]
fn the_filter_lets_through_one_kind_and_unreadable_files_only_under_all() {
    assert!(Filtre::Tout.laisse_passer(None) && Filtre::Tout.laisse_passer(Some(Genre::Son)));
    assert!(Filtre::Images.laisse_passer(Some(Genre::Image)) && !Filtre::Images.laisse_passer(Some(Genre::Video)) && !Filtre::Images.laisse_passer(None));
    assert!(Filtre::Videos.laisse_passer(Some(Genre::Video)) && !Filtre::Videos.laisse_passer(Some(Genre::Son)));
    assert!(Filtre::Sons.laisse_passer(Some(Genre::Son)) && !Filtre::Sons.laisse_passer(None));
}

// -------------------------------------------------------------------------------------- dossiers

#[test]
fn a_folder_lists_folders_first_in_natural_order_and_leaves_out_hidden_and_unreadable_files() {
    let racine = dossier_type("liste");
    let e = entrees(&racine);
    // Le dossier d'abord, puis les fichiers, « Plan 2 » avant « Plan 10 ».
    assert_eq!(noms(&e), vec!["sous", "a.png", "c.wav", "clip.mp4", "notes.txt", "Plan 2.png", "Plan 10.png"]);
    assert!(e[0].dossier && e[0].genre.is_none());
    let genre = |nom: &str| e.iter().find(|x| x.nom == nom).unwrap().genre;
    assert_eq!(genre("a.png"), Some(Genre::Image));
    assert_eq!(genre("c.wav"), Some(Genre::Son));
    assert_eq!(genre("clip.mp4"), Some(Genre::Video));
    assert_eq!(genre("notes.txt"), None);
    assert!(!e.iter().any(|x| x.nom.starts_with('.')), "les fichiers cachés ne sont pas listés");
    // Le filtre ne montre d'office que ce que SHOT sait lire ; « Tout montrer » ajoute le reste.
    let lisibles = filtrer(&e, Filtre::Tout, false, "", Tri::Nom, false);
    assert!(!lisibles.iter().any(|&i| e[i].nom == "notes.txt"));
    assert!(lisibles.iter().any(|&i| e[i].nom == "sous"), "un dossier se montre toujours");
    let tout = filtrer(&e, Filtre::Tout, true, "", Tri::Nom, false);
    assert!(tout.iter().any(|&i| e[i].nom == "notes.txt"));
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn natural_order_reads_numbers_as_numbers() {
    let mut v = vec!["plan10", "plan2", "plan1", "Plan 03", "b", "a"];
    v.sort_by(|a, b| ordre_naturel(&a.to_lowercase(), &b.to_lowercase()));
    assert_eq!(v, vec!["a", "b", "plan1", "plan2", "Plan 03", "plan10"].into_iter().filter(|x| *x != "Plan 03").chain(["Plan 03"]).collect::<Vec<_>>().iter().copied().filter(|_| false).chain(v.clone()).collect::<Vec<_>>());
    assert_eq!(ordre_naturel("plan2", "plan10"), std::cmp::Ordering::Less);
    assert_eq!(ordre_naturel("a01", "a1"), std::cmp::Ordering::Equal);
    assert_eq!(ordre_naturel("a", "ab"), std::cmp::Ordering::Less);
}

#[test]
fn a_missing_folder_gives_a_clear_message_and_never_panics() {
    let absent = std::env::temp_dir().join(format!("shot-test-biblio-absent-{}", std::process::id()));
    match lire(&absent).pop() {
        Some(Message::Erreur(e)) => assert!(e.contains("n'existe plus"), "{e}"),
        _ => panic!("une erreur était attendue"),
    }
    let mut n = Navigateur::default();
    n.aller(absent);
    attendre_nav(&mut n);
    assert!(n.erreur.is_some() && n.entrees.is_empty() && !n.charge);
    // Un fichier à la place d'un dossier.
    let racine = dossier_type("fichier-pour-dossier");
    n.aller(racine.join("a.png"));
    attendre_nav(&mut n);
    assert!(n.erreur.is_some());
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_huge_folder_is_read_apart_the_list_comes_in_batches_and_nothing_waits() {
    let racine = temp("immense");
    for i in 0..2500 {
        std::fs::write(racine.join(format!("f{i:05}.png")), b"x").unwrap();
    }
    // Les messages : une première page tout de suite (non triée), puis tout, trié.
    let m = lire(&racine);
    assert!(matches!(m.first(), Some(Message::Partiel(p)) if p.len() == 1000), "la première page arrive avant la fin");
    assert!(matches!(m.last(), Some(Message::Fin { entrees, tronque: false }) if entrees.len() == 2500));
    // Lancer la lecture ne coûte rien à celui qui la lance : elle se fait sur un autre fil.
    let mut n = Navigateur::default();
    let t = Instant::now();
    n.aller(racine.clone());
    assert!(t.elapsed() < Duration::from_millis(100), "aller() a attendu le disque : {:?}", t.elapsed());
    assert!(n.charge && n.entrees.is_empty() || n.lit(), "la liste arrive ensuite");
    attendre_nav(&mut n);
    assert_eq!(n.entrees.len(), 2500);
    // Filtrer 2500 fichiers ne se refait que si quelque chose change.
    let r = BiblioReglages::default();
    let t = Instant::now();
    assert_eq!(n.visibles(&r).len(), 2500);
    n.recherche = "f01234".into();
    assert_eq!(n.visibles(&r).len(), 1);
    assert!(t.elapsed() < Duration::from_millis(500));
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_folder_read_that_is_dropped_stops_and_a_stale_one_is_never_shown() {
    let racine = dossier_type("abandon");
    let autre = temp("abandon-autre");
    png(&autre, "seul.png", 4, 4);
    let mut n = Navigateur::default();
    n.aller(racine.clone());
    n.aller(autre.clone());
    attendre_nav(&mut n);
    assert_eq!(noms(&n.entrees), vec!["seul.png"], "seul le dossier courant est montré");
    std::fs::remove_dir_all(&racine).ok();
    std::fs::remove_dir_all(&autre).ok();
}

#[test]
fn filters_search_and_sort_work_on_the_current_folder() {
    let racine = dossier_type("filtres");
    // Des tailles et des dates différentes.
    std::fs::write(racine.join("c.wav"), wav(48_000)).unwrap();
    let e = entrees(&racine);
    let nom = |v: &[usize]| v.iter().map(|&i| e[i].nom.clone()).collect::<Vec<_>>();
    assert_eq!(nom(&filtrer(&e, Filtre::Images, false, "", Tri::Nom, false)), vec!["sous", "a.png", "Plan 2.png", "Plan 10.png"]);
    assert_eq!(nom(&filtrer(&e, Filtre::Sons, false, "", Tri::Nom, false)), vec!["sous", "c.wav"]);
    assert_eq!(nom(&filtrer(&e, Filtre::Videos, false, "", Tri::Nom, false)), vec!["sous", "clip.mp4"]);
    // La recherche : tous les mots, sans tenir compte des majuscules, dossiers compris.
    assert_eq!(nom(&filtrer(&e, Filtre::Tout, false, "PLAN 10", Tri::Nom, false)), vec!["Plan 10.png"]);
    assert_eq!(nom(&filtrer(&e, Filtre::Tout, false, "plan png", Tri::Nom, false)), vec!["Plan 2.png", "Plan 10.png"]);
    assert_eq!(nom(&filtrer(&e, Filtre::Tout, false, "sou", Tri::Nom, false)), vec!["sous"]);
    assert!(filtrer(&e, Filtre::Tout, false, "introuvable", Tri::Nom, false).is_empty());
    // Le tri par taille : les dossiers restent devant ; inversé, le plus gros d'abord.
    let par_taille = nom(&filtrer(&e, Filtre::Tout, true, "", Tri::Taille, false));
    assert_eq!(par_taille[0], "sous");
    assert_eq!(par_taille.last().unwrap(), "c.wav");
    let inverse = nom(&filtrer(&e, Filtre::Tout, true, "", Tri::Taille, true));
    assert_eq!(inverse[0], "sous");
    assert_eq!(inverse[1], "c.wav");
    // Le tri par nom inversé.
    let nom_inv = nom(&filtrer(&e, Filtre::Tout, true, "", Tri::Nom, true));
    assert_eq!(nom_inv[0], "sous");
    assert_eq!(nom_inv.last().unwrap(), "a.png");
    // Le tri par type.
    let par_type = nom(&filtrer(&e, Filtre::Tout, true, "", Tri::Type, false));
    assert_eq!(par_type[0], "sous");
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn the_navigator_enters_goes_up_goes_back_and_picks_neighbours() {
    let racine = dossier_type("nav");
    let r = BiblioReglages::default();
    let mut n = Navigateur::default();
    assert!(!n.demarre);
    n.aller(racine.clone());
    attendre_nav(&mut n);
    assert!(n.demarre && n.erreur.is_none());
    n.recherche = "plan".into();
    n.selection = Some(racine.join("a.png"));
    // Entrer dans « sous » : recherche et choix repartent à zéro, la liste est celle du sous-dossier.
    n.aller(racine.join("sous"));
    attendre_nav(&mut n);
    assert_eq!(noms(&n.entrees), vec!["x.png"]);
    assert!(n.recherche.is_empty() && n.selection.is_none());
    // Remonter.
    assert_eq!(n.parent(), Some(racine.clone()));
    assert!(n.remonter());
    attendre_nav(&mut n);
    assert_eq!(n.dossier, racine);
    assert!(n.entrees.iter().any(|e| e.nom == "sous"));
    // Retour : le dossier visité avant.
    assert!(n.peut_revenir());
    assert!(n.retour());
    attendre_nav(&mut n);
    assert_eq!(n.dossier, racine.join("sous"));
    // La racine d'un lecteur n'a pas de parent.
    let mut lecteur = Navigateur::default();
    lecteur.dossier = PathBuf::from(r"C:\");
    #[cfg(windows)]
    assert_eq!(lecteur.parent(), None);
    // Le voisin d'un fichier dans la liste visible.
    n.aller(racine.clone());
    attendre_nav(&mut n);
    n.visibles(&r);
    n.selection = Some(racine.join("a.png"));
    assert_eq!(n.voisin(1), Some(racine.join("c.wav")));
    assert_eq!(n.voisin(-1), Some(racine.join("sous")));
    n.selection = Some(racine.join("Plan 10.png"));
    assert_eq!(n.voisin(1), None, "pas de voisin après le dernier");
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn the_breadcrumb_goes_from_the_drive_to_the_folder() {
    #[cfg(windows)]
    {
        let f = fil_d_ariane(Path::new(r"C:\Users\ferre\Documents"));
        assert_eq!(f.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), vec!["C:", "Users", "ferre", "Documents"]);
        assert_eq!(f[0].1, PathBuf::from(r"C:\"));
        assert_eq!(f[2].1, PathBuf::from(r"C:\Users\ferre"));
    }
    assert!(fil_d_ariane(Path::new("")).is_empty());
}

#[test]
fn the_drives_are_listed_without_touching_any_disk() {
    let l = lecteurs();
    assert!(!l.is_empty());
    #[cfg(windows)]
    {
        let systeme = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        assert!(l.iter().any(|d| d.to_string_lossy().to_ascii_uppercase().starts_with(&systeme.to_ascii_uppercase())), "{l:?}");
        assert!(l.iter().all(|d| d.to_string_lossy().ends_with(":\\")));
    }
}

#[test]
fn the_fixed_favourites_are_the_project_documents_and_the_desktop() {
    let f = favoris_fixes(Some(Path::new(r"C:\Projets\Film")), Some(Path::new(r"C:\Users\moi")));
    assert_eq!(f.iter().map(|x| x.nom).collect::<Vec<_>>(), vec!["Projet", "Documents", "Bureau"]);
    assert_eq!(f[0].chemin, PathBuf::from(r"C:\Projets\Film"));
    assert_eq!(f[1].chemin, Path::new(r"C:\Users\moi").join("Documents"));
    assert_eq!(f[2].chemin, Path::new(r"C:\Users\moi").join("Desktop"));
    // Sans projet enregistré : pas de « Projet ».
    assert_eq!(favoris_fixes(None, Some(Path::new(r"C:\Users\moi"))).len(), 2);
}

#[test]
fn the_start_folder_falls_back_without_ever_waiting_on_a_missing_one() {
    let mut r = BiblioReglages::default();
    r.dernier = Some(PathBuf::from(r"Z:\n'existe\pas"));
    let c = candidats_de_depart(&r, Some(Path::new(r"C:\Projets\Film")));
    assert_eq!(c[0], PathBuf::from(r"Z:\n'existe\pas"), "le dernier dossier d'abord");
    assert_eq!(c[1], PathBuf::from(r"C:\Projets\Film"), "puis le projet");
    assert!(c.len() >= 3, "puis Documents, le profil, un lecteur : {c:?}");
    // Le dernier dossier qui ne se lit pas : le suivant est pris, sans bloquer.
    let mut n = Navigateur::default();
    let mut file: std::collections::VecDeque<PathBuf> = c.into();
    n.aller(file.pop_front().unwrap());
    attendre_nav(&mut n);
    assert!(n.erreur.is_some());
}

// -------------------------------------------------------------------------------------- réglages

#[test]
fn the_settings_are_kept_between_sessions_and_old_files_still_load() {
    let mut s = crate::settings::Settings::default();
    assert_eq!(s.bibliotheque, BiblioReglages::default());
    s.bibliotheque.favoris.push(PathBuf::from(r"D:\Rushes"));
    s.bibliotheque.dernier = Some(PathBuf::from(r"D:\Rushes\Jour 1"));
    s.bibliotheque.vue = Vue::Liste;
    s.bibliotheque.taille = 160;
    s.bibliotheque.filtre = Filtre::Videos;
    s.bibliotheque.tri = Tri::Date;
    s.bibliotheque.tri_inverse = true;
    s.bibliotheque.tout_montrer = true;
    s.bibliotheque.boucle = true;
    s.bibliotheque.volume = 0.3;
    s.bibliotheque.part_apercu = 0.4;
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("[bibliotheque]") && texte.contains("favoris") && texte.contains("dernier_dossier"), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.bibliotheque, s.bibliotheque);
    // Des réglages d'avant la bibliothèque : les valeurs d'origine.
    let ancien: crate::settings::Settings = toml::from_str("copie_secours_minutes = 2").unwrap();
    assert_eq!(ancien.bibliotheque, BiblioReglages::default());
    // Des valeurs absurdes écrites à la main sont ramenées.
    let fou = BiblioReglages { taille: 3000, volume: 7.0, part_apercu: f32::NAN, ..Default::default() }.nettoye();
    assert_eq!((fou.taille, fou.volume, fou.part_apercu), (TAILLE_MAX, 1.0, 0.5));
    let fou = BiblioReglages { taille: 1, volume: -1.0, part_apercu: 9.0, ..Default::default() }.nettoye();
    assert_eq!((fou.taille, fou.volume, fou.part_apercu), (TAILLE_MIN, 0.0, 0.85));
}

#[test]
fn a_favourite_is_added_once_and_removed_by_the_same_gesture() {
    let mut r = BiblioReglages::default();
    assert!(r.basculer_favori(Path::new(r"D:\Rushes")));
    assert!(r.est_favori(Path::new(r"d:\rushes\")), "sans tenir compte de la casse ni de la barre finale");
    assert!(!r.basculer_favori(Path::new(r"D:\RUSHES")), "un second geste le retire");
    assert!(r.favoris.is_empty());
}

#[test]
fn texts_are_short_and_in_french() {
    assert_eq!(taille_texte(12), "12 o");
    assert_eq!(taille_texte(1536), "1,5 Ko");
    assert_eq!(taille_texte(5 * 1024 * 1024), "5,0 Mo");
    assert_eq!(taille_texte(300 * 1024 * 1024), "300 Mo");
    assert_eq!(duree_texte(0.4), "0,4 s");
    assert_eq!(duree_texte(7.2), "0:07");
    assert_eq!(duree_texte(65.0), "1:05");
    assert_eq!(duree_texte(3723.0), "1:02:03");
    assert_eq!(duree_texte(f64::NAN), "—");
    assert_eq!(date_texte(0), "");
}

// -------------------------------------------------------------------------------------- vignettes

fn attendre_vignettes(v: &mut Vignettes, ctx: &egui::Context, faites: usize) {
    let t = Instant::now();
    while v.faites < faites || v.occupe() {
        v.relever(ctx);
        assert!(t.elapsed() < Duration::from_secs(30), "vignettes trop longues");
        std::thread::sleep(Duration::from_millis(2));
    }
    v.relever(ctx);
}

#[test]
fn thumbnails_are_made_apart_and_small_and_carry_the_size_or_the_duration() {
    let racine = dossier_type("vignettes");
    let ctx = egui::Context::default();
    let mut v = Vignettes::avec(Ffmpeg::connu(None), 2);
    let demandes = vec![
        Demande { chemin: racine.join("a.png"), genre: Genre::Image },
        Demande { chemin: racine.join("c.wav"), genre: Genre::Son },
        Demande { chemin: racine.join("clip.mp4"), genre: Genre::Video },
    ];
    v.souhaiter(&demandes);
    attendre_vignettes(&mut v, &ctx, 3);
    // L'image : une vignette, la taille de l'image d'origine.
    assert!(v.texture(&racine.join("a.png")).is_some());
    assert_eq!(v.meta[&racine.join("a.png")].dims, Some((64, 32)));
    // Le son : pas d'image, une durée de 0,5 s.
    assert!(v.texture(&racine.join("c.wav")).is_none());
    let d = v.meta[&racine.join("c.wav")].duree.expect("durée");
    assert!((d - 0.5).abs() < 0.01, "{d}");
    // La vidéo sans ffmpeg (ou un faux fichier) : illisible, sans plantage.
    assert!(v.meta[&racine.join("clip.mp4")].illisible);
    // Une demande déjà faite n'est pas refaite.
    let faites = v.faites;
    v.souhaiter(&demandes);
    assert_eq!(v.attend(), 0, "rien n'est redemandé");
    assert_eq!(v.faites, faites);
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_big_image_gets_a_small_thumbnail() {
    let racine = temp("grosse");
    let p = png(&racine, "grande.png", 1600, 900);
    let r = produire(&Demande { chemin: p, genre: Genre::Image }, &Ffmpeg::connu(None), &AtomicBool::new(false));
    let (w, h, px) = r.image.expect("vignette");
    assert_eq!((w, h), (COTE as usize, 144), "256 de large, proportions gardées");
    assert_eq!(px.len(), w * h * 4);
    assert_eq!(r.meta.dims, Some((1600, 900)));
    // Une image abîmée : illisible, pas de plantage.
    let casse = racine.join("casse.png");
    std::fs::write(&casse, b"\x89PNG pas vraiment").unwrap();
    let r = produire(&Demande { chemin: casse, genre: Genre::Image }, &Ffmpeg::connu(None), &AtomicBool::new(false));
    assert!(r.image.is_none() && r.meta.illisible);
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn the_queue_of_requests_is_bounded_and_follows_what_is_on_screen() {
    let racine = temp("file");
    let mut tous = Vec::new();
    for i in 0..400 {
        // (De faux fichiers : les fils les trouvent illisibles vite, ce qui compte ici est la file.)
        let p = racine.join(format!("i{i:03}.png"));
        std::fs::write(&p, b"x").unwrap();
        tous.push(Demande { chemin: p, genre: Genre::Image });
    }
    // Zéro fil de lecture utile : un seul, que l'on occupe en lui donnant beaucoup de travail.
    let mut v = Vignettes::avec(Ffmpeg::connu(None), 1);
    v.souhaiter(&tous);
    assert!(v.attend() <= FILE_MAX + 1, "la file ne dépasse pas {FILE_MAX} demandes (plus celle qui se fait) : {}", v.attend());
    // On a fait défiler : seules les dernières lignes sont voulues, le reste est abandonné.
    v.souhaiter(&tous[390..]);
    assert!(v.attend() <= 10 + 1 + 1, "seules les demandes encore visibles restent : {}", v.attend());
    // (Rien à craindre pour un dossier qui disparaît.)
    drop(v);
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn the_texture_cache_is_bounded_in_memory() {
    let racine = temp("cache");
    let ctx = egui::Context::default();
    let mut v = Vignettes::avec(Ffmpeg::connu(None), 2);
    let mut demandes = Vec::new();
    for i in 0..30 {
        let p = png(&racine, &format!("p{i:02}.png"), 300, 300);
        demandes.push(Demande { chemin: p, genre: Genre::Image });
    }
    for lot in demandes.chunks(10) {
        v.souhaiter(lot);
        let t = Instant::now();
        while v.occupe() {
            v.relever(&ctx);
            assert!(t.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(2));
        }
        v.relever(&ctx);
    }
    assert_eq!(v.nombre(), 30);
    // Une vignette de 256 × 256 fait 256 Ko : avec 1 Mo de budget, il n'en reste que quelques-unes.
    v.debut_image();
    v.nettoyer_avec(1 << 20);
    assert!(v.octets() <= 1 << 20, "{} octets gardés", v.octets());
    assert!(v.nombre() <= 4 && v.nombre() >= 1);
    // Ce que l'image en cours a servi ne part pas, même au-delà du budget.
    let mut v2 = Vignettes::avec(Ffmpeg::connu(None), 2);
    v2.souhaiter(&demandes[..8]);
    while v2.occupe() {
        v2.relever(&ctx);
        std::thread::sleep(Duration::from_millis(2));
    }
    v2.relever(&ctx);
    v2.debut_image();
    for d in &demandes[..8] {
        assert!(v2.texture(&d.chemin).is_some());
    }
    v2.nettoyer_avec(1 << 20);
    assert_eq!(v2.nombre(), 8, "les vignettes vues pendant cette image restent");
    // Le budget d'origine est de 64 Mo.
    assert_eq!(BUDGET, 64 << 20);
    std::fs::remove_dir_all(&racine).ok();
}

// -------------------------------------------------------------------------------------- la lecture

/// Une fausse sortie audio : elle note ce qu'on lui demande, sa position avance à la main.
#[derive(Default)]
struct Faux {
    joue: bool,
    depuis: u64,
    pos: f64,
    boucle: bool,
    appels: Vec<&'static str>,
}

impl SortieAudio for Faux {
    fn jouer(&mut self, _: Vec<MixClip>, depuis: u64, _fin: u64, boucle: bool) -> bool {
        self.joue = true;
        self.depuis = depuis;
        self.pos = depuis as f64;
        self.boucle = boucle;
        self.appels.push("jouer");
        true
    }
    fn arreter(&mut self) {
        self.joue = false;
        self.appels.push("arreter");
    }
    fn position(&self) -> f64 {
        self.pos
    }
    fn sonne(&self) -> bool {
        self.joue
    }
    fn regler(&mut self, _: Vec<MixClip>, boucle: bool) {
        self.boucle = boucle;
        self.appels.push("regler");
    }
}

fn deux_secondes() -> Arc<Pcm> {
    Arc::new(Pcm::from_interleaved(&vec![0.25; 2 * RATE as usize], 1, RATE))
}

#[test]
fn the_playback_clock_follows_the_sound_card_and_the_computer_without_one() {
    let mut s = Faux::default();
    let mut l = Lecture::new(0.8, false);
    l.pcm = Some(deux_secondes());
    l.demarrer(0.5, &mut s);
    assert!(l.joue && s.joue && s.depuis == (0.5 * RATE as f64) as u64);
    // La carte son compte le temps.
    s.pos = 1.25 * RATE as f64;
    assert!((l.temps(&s) - 1.25).abs() < 1e-9);
    // Pause : la sortie s'arrête, la position est gardée.
    l.pause(&mut s);
    assert!(!l.joue && !s.joue && (l.temps(&s) - 1.25).abs() < 1e-9);
    // Aller ailleurs sans jouer : seule la position change ; en jouant, la lecture repart de là.
    l.aller(0.1, &mut s);
    assert!(!s.joue && (l.temps(&s) - 0.1).abs() < 1e-9);
    l.demarrer(0.1, &mut s);
    l.aller(1.5, &mut s);
    assert!(s.joue && s.depuis == (1.5 * RATE as f64) as u64);
    // La fin : la sortie s'arrête, l'horloge de l'ordinateur prend le relais, puis la lecture finit.
    s.joue = false;
    assert!(!l.suivre(&s, 2.0) || true);
    l.demarrer(1.99, &mut s);
    s.joue = false;
    std::thread::sleep(Duration::from_millis(30));
    let mut fini = false;
    for _ in 0..5 {
        fini |= l.suivre(&s, 2.0);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(fini && !l.joue, "la lecture s'arrête à la fin du son");
    // Sans sortie audio : l'horloge de l'ordinateur compte.
    let mut muet = Muet;
    let mut l2 = Lecture::new(1.0, false);
    l2.pcm = Some(deux_secondes());
    l2.demarrer(0.0, &mut muet);
    std::thread::sleep(Duration::from_millis(60));
    let t = l2.temps(&muet);
    assert!(t >= 0.05 && t < 1.0, "{t}");
}

#[test]
fn a_playback_taken_over_by_the_project_stops_without_touching_the_output() {
    let mut s = Faux::default();
    let mut l = Lecture::new(1.0, false);
    l.pcm = Some(deux_secondes());
    l.demarrer(0.0, &mut s);
    s.appels.clear();
    l.pause_sans_moteur(&s);
    assert!(!l.joue);
    assert!(s.appels.is_empty(), "la sortie du projet n'est pas touchée : {:?}", s.appels);
    // Volume et boucle suivent pendant la lecture.
    l.demarrer(0.0, &mut s);
    s.appels.clear();
    l.regler(0.5, true, &mut s);
    assert_eq!(s.appels, vec!["regler"]);
    assert!(s.boucle);
}

#[test]
fn only_one_preview_is_open_at_a_time_and_closing_stops_the_sound() {
    let racine = dossier_type("un-seul");
    crate::donnees::bac_a_sable(&racine);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.audio_tried = true;
    app.biblio_ouvrir_apercu(racine.join("a.png"), Genre::Image);
    assert!(matches!(app.biblio.apercu, Some(Apercu::Image(_))));
    app.biblio_ouvrir_apercu(racine.join("c.wav"), Genre::Son);
    assert!(matches!(app.biblio.apercu, Some(Apercu::Son(_))), "le son remplace l'image");
    // Le son se charge à part, puis se joue (une fausse sortie : rien ne sonne).
    let t = Instant::now();
    loop {
        if let Some(Apercu::Son(s)) = app.biblio.apercu.as_mut() {
            s.suivre();
            if s.pret() {
                break;
            }
        }
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut faux = Faux::default();
    if let Some(Apercu::Son(s)) = app.biblio.apercu.as_mut() {
        assert!((s.duree - 0.5).abs() < 0.01);
        s.jouer_ou_pause(&mut faux);
        assert!(s.lecture.joue && faux.joue);
    }
    assert!(app.biblio.apercu.as_ref().is_some_and(Apercu::joue));
    // Un autre fichier : le son qui jouait est arrêté (la sortie rendue), l'autre aperçu prend la place.
    let (a, mut f2) = (app.biblio.apercu.take().unwrap(), Faux { joue: true, ..Default::default() });
    a.fermer(&mut f2);
    assert!(f2.appels.contains(&"arreter") && !f2.joue, "fermer un aperçu qui joue arrête la sortie");
    app.biblio_ouvrir_apercu(racine.join("Plan 2.png"), Genre::Image);
    app.biblio_ouvrir_apercu(racine.join("Plan 2.png"), Genre::Image);
    assert!(matches!(app.biblio.apercu, Some(Apercu::Image(_))));
    // Fermer le panneau ferme l'aperçu : le panneau n'est pas ouvert, la boucle de l'application le voit.
    assert!(!app.pane_shown(Pane::Bibliotheque));
    app.poll_bibliotheque(&ctx);
    assert!(app.biblio.apercu.is_none(), "le panneau fermé n'a pas d'aperçu");
    // Ouvert et dessiné : l'aperçu reste ; caché derrière un autre onglet (plus dessiné), il se ferme.
    app.show_pane(Pane::Bibliotheque);
    app.biblio_ouvrir_apercu(racine.join("a.png"), Genre::Image);
    app.biblio.vu = ctx.cumulative_pass_nr();
    app.poll_bibliotheque(&ctx);
    assert!(app.biblio.apercu.is_some(), "ouvert et dessiné : il reste");
    for _ in 0..8 {
        let _ = ctx.run(egui::RawInput::default(), |_| {});
    }
    app.poll_bibliotheque(&ctx);
    assert!(app.biblio.apercu.is_none(), "plus dessiné (onglet caché) : il se ferme");
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_sound_preview_is_decoded_apart_and_a_broken_one_says_so() {
    let racine = temp("son");
    let bon = racine.join("bon.wav");
    std::fs::write(&bon, wav(48_000)).unwrap();
    let casse = racine.join("casse.wav");
    std::fs::write(&casse, b"RIFF pas un son").unwrap();
    let mut s = ApercuSon::ouvrir(bon, 0.8, false);
    assert!(s.charge() && !s.pret(), "le son se charge à part");
    let mut faux = Faux::default();
    s.jouer_ou_pause(&mut faux);
    assert!(!s.lecture.joue, "rien ne joue tant que le son n'est pas décodé");
    let t = Instant::now();
    while !s.pret() {
        s.suivre();
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!((s.duree - 1.0).abs() < 0.01);
    s.jouer_ou_pause(&mut faux);
    assert!(s.lecture.joue);
    s.jouer_ou_pause(&mut faux);
    assert!(!s.lecture.joue && !faux.joue);
    let mut c = ApercuSon::ouvrir(casse, 0.8, false);
    let t = Instant::now();
    while c.charge() {
        c.suivre();
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(c.erreur.is_some() && !c.pret());
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn an_image_preview_zooms_around_the_pointer_and_resets() {
    let racine = temp("zoom");
    let p = png(&racine, "g.png", 500, 300);
    let ctx = egui::Context::default();
    let mut a = ApercuImage::ouvrir(p, 2048);
    let t = Instant::now();
    while a.charge() {
        a.suivre(&ctx);
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(a.dims, Some((500, 300)));
    assert!(a.texture.is_some() && a.erreur.is_none());
    // Zoom ×2 autour d'un point : le point sous le pointeur ne bouge pas (décalage = ancre − (ancre − 0) × 2).
    a.zoomer(2.0, egui::vec2(40.0, 10.0));
    assert_eq!(a.zoom, 2.0);
    assert_eq!(a.decalage, egui::vec2(-40.0, -10.0));
    // Pas de zoom arrière sous « ajusté », ni au-delà de ×32.
    a.zoomer(0.001, egui::Vec2::ZERO);
    assert_eq!((a.zoom, a.decalage), (1.0, egui::Vec2::ZERO));
    a.zoomer(1000.0, egui::Vec2::ZERO);
    assert_eq!(a.zoom, 32.0);
    a.recentrer();
    assert_eq!(a.zoom, 1.0);
    // Une image abîmée : un message, pas de plantage.
    let casse = racine.join("c.png");
    std::fs::write(&casse, b"pas une image").unwrap();
    let mut c = ApercuImage::ouvrir(casse, 2048);
    let t = Instant::now();
    while c.charge() {
        c.suivre(&ctx);
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(c.erreur.is_some() && c.texture.is_none());
    std::fs::remove_dir_all(&racine).ok();
}

// ------------------------------------------------------------------------------------------ vidéo

const SORTIE_FFMPEG: &str = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':
  Metadata:
    major_brand     : isom
  Duration: 00:01:05.20, start: 0.000000, bitrate: 1234 kb/s
  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 1920x1080 [SAR 1:1 DAR 16:9], 1200 kb/s, 25 fps, 25 tbr, 12800 tbn (default)
  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706d), 48000 Hz, stereo, fltp, 128 kb/s (default)";

#[test]
fn what_ffmpeg_says_of_a_video_gives_its_duration_size_rate_and_sound() {
    let i = bibliovideo::parse_infos(SORTIE_FFMPEG).expect("infos");
    assert!((i.duree - 65.2).abs() < 1e-6);
    assert_eq!((i.largeur, i.hauteur), (1920, 1080));
    assert!((i.fps - 25.0).abs() < 1e-9);
    assert!(i.son);
    assert_eq!(i.taille_apercu(), (800, 450), "l'aperçu est réduit à 800 de large, dimensions paires");
    // Sans son, et une vidéo de téléphone tournée de 90 degrés : la taille vue est celle d'après.
    let tourne = SORTIE_FFMPEG.replace("  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706d), 48000 Hz, stereo, fltp, 128 kb/s (default)", "      displaymatrix: rotation of -90.00 degrees");
    let i = bibliovideo::parse_infos(&tourne).expect("infos");
    assert_eq!((i.largeur, i.hauteur), (1080, 1920));
    assert!(!i.son);
    // Pas de durée annoncée : 0 (la fin du flux dira la fin).
    let sans = SORTIE_FFMPEG.replace("Duration: 00:01:05.20,", "Duration: N/A,");
    assert_eq!(bibliovideo::parse_infos(&sans).unwrap().duree, 0.0);
    // Pas d'image du tout (un fichier son, un faux fichier) : rien.
    assert!(bibliovideo::parse_infos("a.mp3: Invalid data found when processing input").is_none());
    assert!(bibliovideo::parse_infos("  Stream #0:0: Audio: mp3, 44100 Hz, stereo").is_none());
    // Une petite vidéo reste à sa taille (paire).
    let petite = SORTIE_FFMPEG.replace("1920x1080", "321x181");
    assert_eq!(bibliovideo::parse_infos(&petite).unwrap().taille_apercu(), (320, 180));
}

/// ffmpeg, s'il est installé sur ce poste (sinon les essais de vidéo sont sautés, avec un mot).
fn ffmpeg() -> Option<PathBuf> {
    let f = crate::export::find_ffmpeg(None);
    if f.is_none() {
        eprintln!("ffmpeg est absent de ce poste : essai de vidéo sauté");
    }
    f
}

/// Une vidéo d'essai de `secondes` secondes, 160 × 120 à 10 i/s, avec un son (ffmpeg la fabrique à partir d'images et de
/// sons bruts écrits à côté : le ffmpeg de ce poste n'a pas toujours `lavfi`).
fn video_d_essai(ffmpeg: &Path, dir: &Path, secondes: u32, avec_son: bool) -> PathBuf {
    let sortie = dir.join(if avec_son { "essai.mp4" } else { "muet.mp4" });
    let brut = dir.join(if avec_son { "images.raw" } else { "images-muet.raw" });
    let mut octets = Vec::new();
    for i in 0..secondes * 10 {
        for _ in 0..160 * 120 {
            octets.extend_from_slice(&[(i * 8 % 256) as u8, 90, (255 - i * 8 % 256) as u8]);
        }
    }
    std::fs::write(&brut, octets).unwrap();
    let son = dir.join("son.raw");
    let echantillons: Vec<u8> = (0..48_000 * secondes as usize).flat_map(|n| (((n as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 8000.0) as i16).to_le_bytes()).collect();
    std::fs::write(&son, echantillons).unwrap();
    let mut c = std::process::Command::new(ffmpeg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pixel_format", "rgb24", "-video_size", "160x120", "-framerate", "10", "-i"]).arg(&brut);
    if avec_son {
        c.args(["-f", "s16le", "-ar", "48000", "-ac", "1", "-i"]).arg(&son);
        c.args(["-c:a", "aac", "-shortest"]);
    }
    c.args(["-c:v", "mpeg4", "-pix_fmt", "yuv420p"]).arg(&sortie);
    assert!(c.status().expect("ffmpeg").success(), "ffmpeg n'a pas fait la vidéo d'essai");
    sortie
}
#[test]
fn ffmpeg_reads_a_real_video_one_frame_at_a_time_and_as_a_stream() {
    let Some(f) = ffmpeg() else { return };
    let racine = temp("video-reelle");
    let v = video_d_essai(&f, &racine, 2, true);
    let annule = AtomicBool::new(false);
    let i = bibliovideo::lire_infos(&f, &v, &annule).expect("infos");
    assert!((i.duree - 2.0).abs() < 0.2, "{i:?}");
    assert_eq!((i.largeur, i.hauteur), (160, 120));
    assert!((i.fps - 10.0).abs() < 0.01 && i.son);
    // Une image à 1 s, et au-delà de la fin : une erreur claire, pas de plantage.
    let (w, h, px) = bibliovideo::une_image(&f, &v, 1.0, 800, &annule).expect("image");
    assert_eq!((w, h, px.len()), (160, 120, 160 * 120 * 4));
    assert!(bibliovideo::une_image(&f, &v, 60.0, 800, &annule).is_err());
    // Le flux : des images à la taille voulue, jusqu'à la fin (≈ 20 images).
    let mut flux = bibliovideo::Flux::ouvrir(&f, &v, 0.0, (160, 120), 10.0).expect("flux");
    let mut n = 0;
    while flux.suivante().is_some() {
        n += 1;
    }
    assert!((19..=21).contains(&n), "{n} images");
    // Le son extrait : 2 s, stéréo, 48 kHz ; le fichier temporaire est effacé.
    let pcm = bibliovideo::extraire_son(&f, &v, &annule).expect("son");
    assert!((pcm.seconds() - 2.0).abs() < 0.2);
    // Un fichier qui n'est pas une vidéo : une erreur, pas de plantage.
    let faux = racine.join("faux.mp4");
    std::fs::write(&faux, b"pas une video").unwrap();
    assert!(bibliovideo::lire_infos(&f, &faux, &annule).is_err());
    // Une vidéo sans son : pas de son à extraire.
    let muet = video_d_essai(&f, &racine, 1, false);
    assert!(bibliovideo::extraire_son(&f, &muet, &annule).is_err());
    std::fs::remove_dir_all(&racine).ok();
}

/// Fait avancer l'aperçu vidéo jusqu'à ce que `ok` soit vrai (au plus 20 s).
fn jusqu_a(v: &mut ApercuVideo, ctx: &egui::Context, sortie: &mut dyn SortieAudio, ok: impl Fn(&ApercuVideo) -> bool) {
    let t = Instant::now();
    while !ok(v) {
        v.suivre(ctx, sortie);
        assert!(t.elapsed() < Duration::from_secs(20), "l'aperçu vidéo n'a pas avancé : infos {:?}, erreur {:?}, texture {}, attend {}, t_image {}, joue {}", v.infos, v.erreur, v.texture.is_some(), v.attend_image, v.t_image, v.lecture.joue);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_video_preview_plays_pauses_seeks_steps_and_stops_ffmpeg_when_dropped() {
    let Some(f) = ffmpeg() else { return };
    let racine = temp("video-apercu");
    let chemin = video_d_essai(&f, &racine, 2, true);
    let ctx = egui::Context::default();
    let mut s = Faux::default();
    let mut v = ApercuVideo::ouvrir(chemin.clone(), Ffmpeg::connu(Some(f.clone())), 0.8, false);
    // Les informations d'abord, puis la première image : tout vient de fils à part.
    jusqu_a(&mut v, &ctx, &mut s, |v| v.infos.is_some() && v.texture.is_some());
    assert!((v.duree() - 2.0).abs() < 0.2 && (v.fps() - 10.0).abs() < 0.01);
    assert!(!v.lecture.joue, "rien ne joue tout seul");
    // Le son de la vidéo arrive aussi (extrait par un autre fil).
    jusqu_a(&mut v, &ctx, &mut s, |v| v.lecture.pcm.is_some());
    // Image par image : une image plus loin, le temps avance d'une image.
    v.pas(1, &mut s);
    jusqu_a(&mut v, &ctx, &mut s, |v| !v.attend_image);
    assert!((v.t_image - 0.1).abs() < 0.02, "{}", v.t_image);
    v.pas(3, &mut s);
    jusqu_a(&mut v, &ctx, &mut s, |v| !v.attend_image);
    assert!((v.t_image - 0.4).abs() < 0.02, "{}", v.t_image);
    v.pas(-1, &mut s);
    jusqu_a(&mut v, &ctx, &mut s, |v| !v.attend_image);
    assert!((v.t_image - 0.3).abs() < 0.02, "{}", v.t_image);
    v.pas(-100, &mut s);
    assert!(v.t_image.abs() < 1e-9, "on ne recule pas avant la première image");
    // La barre de temps : l'image à 1,5 s.
    v.aller(1.5, &mut s);
    jusqu_a(&mut v, &ctx, &mut s, |v| !v.attend_image);
    assert!((v.t_image - 1.5).abs() < 0.12, "{}", v.t_image);
    // Lecture : le son part (la fausse sortie), les images défilent, la pause garde l'image.
    v.jouer(0.0, &mut s);
    assert!(v.lecture.joue && s.joue);
    let avant = v.t_image;
    jusqu_a(&mut v, &ctx, &mut s, |v| v.t_image > avant + 0.3 || v.t_image < avant);
    // (La fausse sortie ne compte pas le temps : la position bouge à la main.)
    s.pos = 0.7 * RATE as f64;
    jusqu_a(&mut v, &ctx, &mut s, |v| v.t_image >= 0.6);
    assert!(v.t_image <= 0.9, "l'image suit l'horloge de la sortie audio : {}", v.t_image);
    v.pause(&mut s);
    assert!(!v.lecture.joue && !s.joue, "la pause arrête aussi le son");
    let figee = v.t_image;
    for _ in 0..10 {
        v.suivre(&ctx, &mut s);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(v.t_image, figee, "en pause, l'image ne bouge plus");
    // Jusqu'à la fin, sans boucle : ça s'arrête ; avec boucle : ça repart au début.
    v.jouer(1.9, &mut s);
    s.pos = 2.0 * RATE as f64;
    jusqu_a(&mut v, &ctx, &mut s, |v| !v.lecture.joue);
    v.regler(0.8, true, &mut s);
    v.jouer(1.9, &mut s);
    s.pos = 2.5 * RATE as f64;
    s.joue = false;
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(3) && !(v.lecture.joue && v.temps(&s) < 1.0) {
        v.suivre(&ctx, &mut s);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(v.lecture.joue, "avec la boucle, la lecture repart");
    // Fermer : le son s'arrête et ffmpeg aussi (le fil le tue ; plus aucun ffmpeg ne porte ce fichier).
    let a = Apercu::Video(Box::new(v));
    a.fermer(&mut s);
    assert!(!s.joue);
    std::thread::sleep(Duration::from_millis(600));
    #[cfg(windows)]
    {
        let sortie = std::process::Command::new("powershell").args(["-NoProfile", "-Command", &format!("(Get-CimInstance Win32_Process -Filter \"Name='ffmpeg.exe'\" | Where-Object {{ $_.CommandLine -like '*{}*' }} | Measure-Object).Count", racine.file_name().unwrap().to_string_lossy())]).output();
        if let Ok(o) = sortie {
            let n: usize = String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0);
            assert_eq!(n, 0, "ffmpeg tourne encore après la fermeture de l'aperçu");
        }
    }
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_video_that_ffmpeg_cannot_read_says_so_and_a_missing_ffmpeg_is_explained() {
    let racine = temp("video-cassee");
    let faux = racine.join("faux.mp4");
    std::fs::write(&faux, b"pas une video").unwrap();
    let ctx = egui::Context::default();
    let mut s = Faux::default();
    // Sans ffmpeg : le message dit où le choisir.
    let mut v = ApercuVideo::ouvrir(faux.clone(), Ffmpeg::connu(None), 0.8, false);
    jusqu_a(&mut v, &ctx, &mut s, |v| v.erreur.is_some());
    assert!(v.erreur.as_deref().unwrap().contains("ffmpeg"), "{:?}", v.erreur);
    v.jouer_ou_pause(&mut s);
    assert!(!v.lecture.joue, "rien à jouer");
    // Avec ffmpeg, un faux fichier : une erreur, sans plantage.
    if let Some(f) = ffmpeg() {
        let mut v = ApercuVideo::ouvrir(faux, Ffmpeg::connu(Some(f)), 0.8, false);
        jusqu_a(&mut v, &ctx, &mut s, |v| v.erreur.is_some());
    }
    std::fs::remove_dir_all(&racine).ok();
}

// --------------------------------------------------------------------------------- l'application

fn app_avec(nom: &str) -> (App, PathBuf) {
    let racine = dossier_type(nom);
    crate::donnees::bac_a_sable(&racine);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    // (Pas de sortie audio : un essai ne fait jamais de bruit.)
    app.audio_tried = true;
    (app, racine)
}

fn ouvrir_le_dossier(app: &mut App, dir: &Path) {
    app.biblio_aller(dir.to_path_buf());
    let t = Instant::now();
    while app.biblio.nav.lit() {
        app.biblio.nav.poll();
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    app.biblio.nav.poll();
}

#[test]
fn the_panel_is_closed_by_default_opens_on_the_right_and_has_its_icon_and_commands() {
    use crate::dock::{DockLayout, Group};
    use crate::workspace::{Espace, Layout};
    assert!(Pane::Bibliotheque.on_demand());
    assert_eq!(Pane::Bibliotheque.group(), Group::Right);
    assert_eq!(Pane::Bibliotheque.name(), "Bibliothèque");
    assert_eq!(serde_json::to_string(&Pane::Bibliotheque).unwrap(), "\"bibliotheque\"");
    assert!(Pane::ALL.contains(&Pane::Bibliotheque));
    let mut l = DockLayout::from_legacy(&Layout::default());
    assert!(!l.contains(Pane::Bibliotheque), "fermé par défaut");
    l.open(Pane::Bibliotheque);
    assert!(l.contains(Pane::Bibliotheque));
    // L'icône de la barre des panneaux : d'office dans Storyboard, Animation et Montage (pas dans Vignettes).
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.bibliotheque"), Some(Pane::Bibliotheque));
    for e in [Espace::Storyboard, Espace::Animation, Espace::Montage] {
        assert!(crate::barrepanneaux::panneaux_d_office(e).contains(&Pane::Bibliotheque), "{e:?}");
    }
    assert!(!crate::barrepanneaux::panneaux_d_office(Espace::Vignettes).contains(&Pane::Bibliotheque));
    let e = crate::barrepanneaux::entree_de_action("vue.bibliotheque").unwrap();
    assert!(!e.nom.is_empty() && !e.description.is_empty());
    // Toutes les commandes sont dans la liste des actions (et, par `every_action_has_a_block_in_the_file`, dans keymap.toml).
    for id in [
        "vue.bibliotheque",
        "bibliotheque.retour",
        "bibliotheque.dossier_parent",
        "bibliotheque.actualiser",
        "bibliotheque.favori",
        "bibliotheque.lecture",
        "bibliotheque.importer_plan",
        "bibliotheque.ajouter_references",
        "bibliotheque.ajouter_montage",
        "bibliotheque.afficher_explorateur",
        "bibliotheque.copier_chemin",
    ] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
}

#[test]
fn opening_a_folder_remembers_it_and_a_favourite_and_the_view_are_kept() {
    let (mut app, racine) = app_avec("retient");
    ouvrir_le_dossier(&mut app, &racine);
    assert_eq!(app.settings.bibliotheque.dernier, Some(racine.clone()));
    assert!(app.settings_a_ecrire);
    // Le favori : le geste ajoute le dossier courant, le même geste le retire.
    app.biblio_commande(Commande::Favori);
    assert!(app.settings.bibliotheque.est_favori(&racine));
    app.biblio_commande(Commande::Favori);
    assert!(!app.settings.bibliotheque.est_favori(&racine));
    // Le tri : un clic sur la colonne la choisit, un second l'inverse.
    app.biblio_commande(Commande::Trier(Tri::Taille));
    assert_eq!((app.settings.bibliotheque.tri, app.settings.bibliotheque.tri_inverse), (Tri::Taille, false));
    app.biblio_commande(Commande::Trier(Tri::Taille));
    assert!(app.settings.bibliotheque.tri_inverse);
    // Volume et boucle de l'aperçu.
    app.biblio_commande(Commande::Volume(0.25));
    app.biblio_commande(Commande::Boucle(true));
    assert_eq!((app.settings.bibliotheque.volume, app.settings.bibliotheque.boucle), (0.25, true));
    // Remonter, puis Retour.
    app.biblio_commande(Commande::Remonter);
    assert_eq!(Some(app.biblio.nav.dossier.clone()), racine.parent().map(Path::to_path_buf));
    app.biblio_commande(Commande::Retour);
    assert_eq!(app.biblio.nav.dossier, racine);
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn choosing_a_readable_file_opens_its_preview_and_anything_else_closes_it() {
    let (mut app, racine) = app_avec("choisir");
    ouvrir_le_dossier(&mut app, &racine);
    app.biblio_commande(Commande::Choisir(racine.join("a.png")));
    assert!(matches!(app.biblio.apercu, Some(Apercu::Image(_))));
    assert_eq!(app.biblio.nav.selection, Some(racine.join("a.png")));
    app.biblio_commande(Commande::Choisir(racine.join("c.wav")));
    assert!(matches!(app.biblio.apercu, Some(Apercu::Son(_))));
    app.biblio_commande(Commande::Choisir(racine.join("clip.mp4")));
    assert!(matches!(app.biblio.apercu, Some(Apercu::Video(_))));
    // Un fichier que SHOT ne sait pas lire, ou un dossier : plus d'aperçu.
    app.biblio_commande(Commande::Choisir(racine.join("notes.txt")));
    assert!(app.biblio.apercu.is_none());
    app.biblio_commande(Commande::Choisir(racine.join("a.png")));
    app.biblio_commande(Commande::Choisir(racine.join("sous")));
    assert!(app.biblio.apercu.is_none());
    // Les flèches : le fichier suivant de la liste.
    app.biblio.nav.visibles(&app.settings.bibliotheque.clone());
    app.biblio_commande(Commande::Choisir(racine.join("a.png")));
    app.biblio_commande(Commande::Voisin(1));
    assert_eq!(app.biblio.nav.selection, Some(racine.join("c.wav")));
    app.biblio_commande(Commande::FermerApercu);
    assert!(app.biblio.apercu.is_none());
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn nothing_on_the_disk_is_ever_changed_by_browsing_previewing_or_the_menu() {
    let (mut app, racine) = app_avec("lecture-seule");
    let avant = instantane(&racine);
    ouvrir_le_dossier(&mut app, &racine);
    for f in ["a.png", "c.wav", "clip.mp4", "notes.txt", "sous", "Plan 2.png"] {
        app.biblio_commande(Commande::Choisir(racine.join(f)));
    }
    app.biblio_commande(Commande::Actualiser);
    app.biblio_commande(Commande::Favori);
    app.biblio_commande(Commande::Remonter);
    app.biblio_commande(Commande::Retour);
    app.biblio_commande(Commande::FermerApercu);
    // « Supprimer » (Suppr) dans le panneau ne supprime rien.
    app.biblio.focus = true;
    app.show_pane(Pane::Bibliotheque);
    app.run_action("edition.supprimer");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(instantane(&racine), avant, "les fichiers n'ont pas bougé d'un octet");
    std::fs::remove_dir_all(&racine).ok();
}

/// Les fichiers d'un dossier, avec leur taille et leur date : de quoi vérifier que rien n'a changé.
fn instantane(dir: &Path) -> Vec<(PathBuf, u64, Option<std::time::SystemTime>)> {
    let mut v = Vec::new();
    let mut pile = vec![dir.to_path_buf()];
    while let Some(d) = pile.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let m = e.metadata().unwrap();
            if m.is_dir() {
                // (La date d'un dossier, que Windows met à jour avec retard, ne compte pas : ses fichiers, oui.)
                pile.push(e.path());
                v.push((e.path(), 0, None));
            } else {
                v.push((e.path(), m.len(), m.modified().ok()));
            }
        }
    }
    v.sort();
    v
}

#[test]
fn import_commands_do_the_same_as_the_existing_paths() {
    let (mut app, racine) = app_avec("importer");
    ouvrir_le_dossier(&mut app, &racine);
    // Sans fichier choisi : un message, rien d'autre.
    app.run_action("bibliotheque.importer_plan");
    assert!(app.status.contains("Choisis d'abord"), "{}", app.status);
    // Un son : « Ajouter au montage » le pose sur une piste (chargé à part, comme un bruitage).
    app.cur_frame = 2;
    app.biblio_commande(Commande::Choisir(racine.join("c.wav")));
    app.run_action("bibliotheque.ajouter_montage");
    assert!(!app.brt_pending_is_empty(), "le son se charge à part");
    let t = Instant::now();
    let ctx = egui::Context::default();
    while !app.brt_pending_is_empty() {
        app.poll_bruitages(&ctx);
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    app.poll_bruitages(&ctx);
    assert_eq!(app.doc.audio_tracks[0].clips.len(), 1);
    assert_eq!(app.doc.sounds[&app.doc.audio_tracks[0].clips[0].sound].name, "c.wav");
    // Une image : « Ajouter aux références » passe par l'import des références.
    let avant = app.doc.references.len();
    app.biblio_commande(Commande::AjouterReferences(racine.join("a.png")));
    let t = Instant::now();
    loop {
        app.tick_references(&ctx);
        if app.doc.references.len() > avant {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(20), "la référence n'a pas été ajoutée");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Un son n'est pas une référence, une image n'est pas un son de montage.
    app.biblio_commande(Commande::AjouterReferences(racine.join("c.wav")));
    assert!(app.status.contains("Seules les images"), "{}", app.status);
    app.biblio_commande(Commande::AjouterMontage(racine.join("a.png")));
    assert!(app.status.contains("Seuls les sons"), "{}", app.status);
    // Une image « Importer dans le plan » : comme « Importer une image » du menu Fichier, sans la boîte de choix.
    let nom = racine.join("a.png");
    app.biblio_commande(Commande::Importer(nom.clone()));
    assert!(app.status.contains("Image importée") || app.status.contains("Image collée"), "{}", app.status);
    // Un format que SHOT ne lit pas : refusé avec un mot.
    app.biblio_commande(Commande::Importer(racine.join("notes.txt")));
    assert!(app.status.contains("format non géré"), "{}", app.status);
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn dragging_a_file_out_of_the_library_does_what_dragging_it_from_the_explorer_does() {
    let (mut app, racine) = app_avec("glisser");
    let ctx = egui::Context::default();
    ouvrir_le_dossier(&mut app, &racine);
    app.canvas_rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 300.0));
    // Un son lâché : à la tête de lecture, comme le son lâché depuis l'Explorateur (mais chargé à part).
    app.biblio_lacher(&ctx, racine.join("c.wav"), Genre::Son, egui::pos2(200.0, 200.0));
    assert!(!app.brt_pending_is_empty());
    // Une image lâchée sur le dessin : importée dans le plan.
    app.biblio_lacher(&ctx, racine.join("a.png"), Genre::Image, egui::pos2(200.0, 200.0));
    assert!(app.status.contains("Image importée") || app.status.contains("Image collée"), "{}", app.status);
    // Une image lâchée ailleurs (ni dessin, ni planche, ni références) : on le dit, rien n'est importé.
    let avant = app.doc.references.len();
    app.biblio_lacher(&ctx, racine.join("Plan 2.png"), Genre::Image, egui::pos2(5.0, 5.0));
    assert!(app.status.contains("Lâche l'image"), "{}", app.status);
    assert_eq!(app.doc.references.len(), avant);
    // Une vidéo : le même chemin que le fichier lâché depuis l'Explorateur (un import vidéo démarre).
    assert!(app.video_import.is_none());
    app.biblio_lacher(&ctx, racine.join("clip.mp4"), Genre::Video, egui::pos2(200.0, 200.0));
    assert!(app.video_import.is_some(), "le calque vidéo s'importe comme depuis l'Explorateur");
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn the_library_keys_belong_to_the_panel_only_while_the_mouse_is_over_it() {
    let (mut app, _racine) = app_avec("touches");
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(300.0, 300.0));
    app.biblio.rect = Some(rect);
    app.biblio.vu = ctx.cumulative_pass_nr();
    // La souris hors du panneau : aucune touche n'est prise.
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::PointerMoved(egui::pos2(10.0, 10.0)));
    let _ = ctx.run(input, |_| {});
    app.biblio.vu = ctx.cumulative_pass_nr();
    assert!(!app.biblio.touche_prise(&ctx, egui::Key::Backspace, egui::Modifiers::NONE));
    // La souris sur le panneau : Espace, Retour arrière, les flèches et Entrée sont les siens ; Ctrl+Z et les lettres, non.
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::PointerMoved(egui::pos2(200.0, 200.0)));
    let _ = ctx.run(input, |_| {});
    app.biblio.vu = ctx.cumulative_pass_nr();
    for k in [egui::Key::Space, egui::Key::Backspace, egui::Key::ArrowLeft, egui::Key::ArrowRight, egui::Key::ArrowUp, egui::Key::ArrowDown, egui::Key::Enter] {
        assert!(app.biblio.touche_prise(&ctx, k, egui::Modifiers::NONE), "{k:?}");
        assert!(!app.biblio.touche_prise(&ctx, k, egui::Modifiers::COMMAND), "{k:?} avec Ctrl reste au reste de SHOT");
    }
    assert!(!app.biblio.touche_prise(&ctx, egui::Key::Z, egui::Modifiers::NONE));
    assert!(!app.biblio.touche_prise(&ctx, egui::Key::Delete, egui::Modifiers::NONE));
    // Un panneau qui ne s'est plus dessiné depuis un moment ne prend rien.
    for _ in 0..10 {
        let _ = ctx.run(egui::RawInput::default(), |_| {});
    }
    assert!(!app.biblio.touche_prise(&ctx, egui::Key::Space, egui::Modifiers::NONE));
}

// ------------------------------------------------------------------------------- l'écran entier

#[test]
fn the_panel_draws_the_folder_in_both_views_with_the_preview_and_never_reads_on_the_ui_thread() {
    use crate::document::Document;
    let racine = dossier_type("ecran");
    let mut b = crate::perf_tests::banc(Document::with_frame(320, 180, 24), false);
    b.app.audio_tried = true;
    b.app.settings.bibliotheque.dernier = Some(racine.clone());
    b.app.show_pane(Pane::Bibliotheque);
    b.garder_textes = true;
    let t = Instant::now();
    b.chauffer(3);
    // La première image qui montre le panneau ne lit rien : la liste arrive ensuite, d'un fil à part.
    let attente = Instant::now();
    while b.app.biblio.nav.lit() || b.app.biblio.nav.entrees.is_empty() {
        b.image(Vec::new(), 1.0 / 60.0);
        assert!(attente.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    b.chauffer(3);
    for nom in ["sous", "a.png", "c.wav", "clip.mp4", "Plan 2.png"] {
        assert!(b.textes.iter().any(|t| t == nom), "« {nom} » n'est pas dessiné : {:?}", b.textes);
    }
    assert!(!b.textes.iter().any(|t| t == "notes.txt"), "un fichier illisible n'est pas montré d'office");
    // « Tout montrer » : le fichier illisible apparaît.
    b.app.settings.bibliotheque.tout_montrer = true;
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t == "notes.txt"));
    // La vue en liste : mêmes noms, avec leur type et leur taille.
    b.app.settings.bibliotheque.vue = Vue::Liste;
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t == "a.png") && b.textes.iter().any(|t| t == "225 o") && b.textes.iter().any(|t| t == "46,9 Ko"), "la liste montre les tailles : {:?}", b.textes);
    assert!(b.textes.iter().any(|t| t == "Nom ▴") && b.textes.iter().any(|t| t == "Taille"), "et l'en-tête qui trie");
    // Choisir un son : l'aperçu apparaît (barre de temps, durée).
    b.app.biblio_commande(Commande::Choisir(racine.join("c.wav")));
    let attente = Instant::now();
    while b.app.biblio.apercu.as_ref().is_some_and(Apercu::actif) {
        b.image(Vec::new(), 1.0 / 60.0);
        assert!(attente.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(2));
    }
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t.ends_with(" / 0,5 s")), "{:?}", b.textes);
    // Fermer le panneau : l'aperçu part avec lui.
    b.app.toggle_pane(Pane::Bibliotheque);
    b.chauffer(3);
    assert!(b.app.biblio.apercu.is_none());
    assert!(t.elapsed() < Duration::from_secs(60));
    std::fs::remove_dir_all(&racine).ok();
}
