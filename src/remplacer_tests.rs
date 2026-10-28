//! Enregistrer par-dessus un projet existant (`remplacer.rs`) et ne plus jamais perdre en silence (Anthony, 2026-10-28 :
//! un projet « Sans titre » enregistré sur « Mon film », refusé dans la barre d'état seulement, puis perdu).
//!
//! La fenêtre de fichiers de Windows (`rfd`) ne tourne pas dans les essais : on appelle la décision avec le chemin choisi
//! (`enregistrer_sous_vers`), et la croix de la fenêtre est une image qui porte `close_requested` (`Banc::fermer_la_fenetre`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::Color32;

use crate::app::Pending;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{arbre, dossier};
use crate::perf_tests::{banc, Banc};
use crate::project;
use crate::recovery::{self, Recovery, Snapshot};
use crate::remplacer::{ChoixRemplacement, Decision};

fn projet(n: usize, rouge: u8) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(k)[0] = Color32::from_rgba_premultiplied(rouge, 10, 10, 255);
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

/// Un projet « Mon film » déjà enregistré dans `dir` (3 dessins rouges) : son manifeste.
fn mon_film(dir: &Path) -> PathBuf {
    let manifeste = dir.join("Mon film").join("Mon film.shot");
    std::fs::create_dir_all(manifeste.parent().unwrap()).unwrap();
    project::save(&projet(3, 200), &manifeste, &mut Default::default()).expect("projet existant");
    manifeste
}

/// Un projet « Sans titre » : jamais enregistré, modifié, avec 5 dessins bleus.
fn sans_titre() -> Banc {
    let mut b = banc(projet(5, 20), false);
    b.app.path = None;
    b.app.modified = true;
    b
}

fn pomper(b: &mut Banc) {
    for _ in 0..1200 {
        if !b.app.save_en_cours() {
            b.image(Vec::new(), 0.016);
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("l'enregistrement ne se termine pas");
}

fn dossiers_remplaces(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Mon film (remplacé le ")))
        .collect()
}

/// Ce qui doit rester vrai tant que rien n'est enregistré : projet ouvert, modifié, fenêtre pas fermée.
fn rien_n_est_perdu(b: &mut Banc) {
    assert!(b.app.modified, "le projet reste modifié");
    assert!(!b.app.allow_close, "on ne quitte pas");
    assert!(b.fermer_la_fenetre(), "la croix de la fenêtre est retenue : la fenêtre ne se ferme pas");
    assert!(!b.app.allow_close);
}

// ------------------------------------------------------------------------------------------ la décision

#[test]
fn the_decision_asks_before_replacing_an_existing_project_and_writes_nothing() {
    let dir = dossier("remplacer-decision");
    let existant = mon_film(&dir);
    let avant = arbre(&dir);
    let b = sans_titre();
    // Le projet qui existe : le manifeste, ou le dossier, ou le jumeau `.pelure` : toujours la question.
    for choisi in [existant.clone(), dir.join("Mon film.shot"), dir.join("Mon film").join("Mon film.pelure")] {
        match b.app.decider_enregistrer_sous(&choisi) {
            Decision::Remplacer(r) => {
                assert_eq!(r.nom, "Mon film");
                assert_eq!(r.manifest, existant);
                assert!(r.date.is_some(), "le dernier enregistrement est lu sur le disque");
                assert!(r.nouveau_nom.starts_with("Mon film (remplacé le "), "{}", r.nouveau_nom);
            }
            Decision::Ecrire => panic!("{} : écrire sur un projet existant sans demander", choisi.display()),
            Decision::Refuser(m) => panic!("{} : refusé : {m}", choisi.display()),
        }
    }
    // Un nom libre : on écrit.
    assert!(matches!(b.app.decider_enregistrer_sous(&dir.join("Autre.shot")), Decision::Ecrire));
    // Un dossier qui a déjà `dessins/` sans manifeste est aussi un projet à remplacer.
    let orphelin = dir.join("Orphelin");
    std::fs::create_dir_all(orphelin.join("dessins")).unwrap();
    assert!(matches!(b.app.decider_enregistrer_sous(&orphelin.join("Orphelin.shot")), Decision::Remplacer(_)));
    assert_eq!(arbre(&dir).keys().filter(|k| k.starts_with("Mon film/")).count(), avant.len(), "la décision n'écrit rien");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_open_in_another_tab_is_never_replaced() {
    let dir = dossier("remplacer-autre-onglet");
    let existant = mon_film(&dir);
    let avant = arbre(&dir);
    // « Mon film » est ouvert dans un onglet ; un projet sans titre dans un autre.
    let mut b = banc(projet(3, 200), false);
    b.app.path = Some(existant.clone());
    b.app.modified = false;
    assert!(b.app.nouvel_onglet(), "{}", b.app.status);
    assert!(b.app.charger_projet(projet(5, 20)));
    b.app.welcome = false;
    b.app.modified = true;
    b.chauffer(2);
    match b.app.decider_enregistrer_sous(&existant) {
        Decision::Refuser(m) => assert!(m.contains("ouvert dans un autre onglet"), "{m}"),
        _ => panic!("un projet ouvert ailleurs ne se remplace pas"),
    }
    assert!(!b.app.enregistrer_sous_vers(&existant, None));
    assert!(b.app.save_error.as_deref().is_some_and(|e| e.contains("autre onglet")), "une fenêtre dit pourquoi");
    assert!(b.app.remplacement.is_none());
    assert_eq!(arbre(&dir), avant, "rien n'a bougé");
    rien_n_est_perdu(&mut b);
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------------------------------ la croix de la fenêtre

/// La suite d'Anthony : projet sans titre modifié, croix, « Enregistrer », le projet existant choisi.
#[test]
fn the_window_cross_then_save_onto_an_existing_project_keeps_everything() {
    let dir = dossier("remplacer-croix");
    let existant = mon_film(&dir);
    let avant = arbre(&dir);
    let mut b = sans_titre();
    // 1. la croix : la fenêtre est retenue, la boîte « Modifications non enregistrées » s'ouvre.
    assert!(b.fermer_la_fenetre(), "la croix est retenue");
    assert!(matches!(b.app.pending, Some(Pending::Quit)), "la boîte s'ouvre");
    // 2. « Enregistrer » : le chemin choisi existe déjà.
    let action = b.app.pending.take().expect("action");
    assert!(!b.app.enregistrer_sous_vers(&existant, Some(action)));
    // 3. Une vraie fenêtre demande ; rien n'est écrit, rien n'est fermé.
    assert!(b.app.remplacement.is_some(), "la fenêtre « Un projet existe déjà ici » est ouverte");
    assert!(b.app.save_error.is_none());
    assert_eq!(arbre(&dir), avant, "rien n'est écrit");
    rien_n_est_perdu(&mut b);
    // Une seconde croix, la fenêtre de remplacement toujours là : retenue de même.
    b.chauffer(2);
    assert!(b.fermer_la_fenetre());
    assert!(b.app.remplacement.is_some(), "la question reste posée");
    assert!(!b.app.allow_close);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replacing_renames_the_old_folder_intact_writes_the_new_project_and_the_action_follows() {
    let dir = dossier("remplacer-remplacer");
    let existant = mon_film(&dir);
    let avant = arbre(&dir.join("Mon film"));
    let mut b = sans_titre();
    b.app.request(Pending::Quit);
    let action = b.app.pending.take().expect("action");
    assert!(!b.app.enregistrer_sous_vers(&existant, Some(action)));
    b.app.remplacement_choisir(ChoixRemplacement::Remplacer);
    assert!(b.app.save_error.is_none(), "{:?}", b.app.save_error);
    assert!(b.app.remplacement.is_none());
    pomper(&mut b);
    // L'ancien dossier est à côté, avec tout son contenu, intact.
    let gardes = dossiers_remplaces(&dir);
    assert_eq!(gardes.len(), 1, "{gardes:?}");
    assert_eq!(arbre(&gardes[0]), avant, "l'ancien projet est gardé tel quel");
    // Le nouveau projet est écrit à la place demandée.
    assert!(existant.is_file());
    let lu = project::load(&existant).expect("le nouveau projet se lit");
    assert_eq!(lu.doc.panels[0].layers[0].frames.len(), 5, "c'est le projet « Sans titre »");
    assert_eq!(b.app.path, Some(existant.clone()));
    assert!(!b.app.modified, "enregistré");
    // L'action en attente a suivi.
    assert!(b.app.allow_close, "Quitter a eu lieu après l'enregistrement");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replacing_twice_in_the_same_minute_keeps_both_old_folders() {
    let dir = dossier("remplacer-deux-fois");
    let existant = mon_film(&dir);
    for fois in 0..2 {
        let mut b = sans_titre();
        assert!(!b.app.enregistrer_sous_vers(&existant, None));
        b.app.remplacement_choisir(ChoixRemplacement::Remplacer);
        pomper(&mut b);
        assert!(b.app.save_error.is_none() && !b.app.modified, "passage {fois} : {:?} {}", b.app.save_error, b.app.status);
    }
    assert_eq!(dossiers_remplaces(&dir).len(), 2, "rien n'est écrasé ni effacé");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelling_the_replacement_changes_nothing_and_the_question_comes_back() {
    let dir = dossier("remplacer-annuler");
    let existant = mon_film(&dir);
    let avant = arbre(&dir);
    let mut b = sans_titre();
    b.app.request(Pending::Close);
    let action = b.app.pending.take().expect("action");
    assert!(!b.app.enregistrer_sous_vers(&existant, Some(action)));
    b.app.remplacement_choisir(ChoixRemplacement::Annuler);
    assert!(b.app.remplacement.is_none());
    assert_eq!(arbre(&dir), avant, "rien n'a changé");
    assert!(dossiers_remplaces(&dir).is_empty());
    assert!(b.app.path.is_none(), "le projet reste sans titre");
    assert!(matches!(b.app.pending, Some(Pending::Close)), "la boîte « Modifications non enregistrées » revient");
    let onglets = b.app.onglets.len();
    rien_n_est_perdu(&mut b);
    assert_eq!(b.app.onglets.len(), onglets, "l'onglet n'est pas fermé");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replacing_then_closing_the_tab_closes_it_after_the_save() {
    let dir = dossier("remplacer-fermer-onglet");
    let existant = mon_film(&dir);
    let mut b = sans_titre();
    b.app.request(Pending::Close);
    let action = b.app.pending.take().expect("action");
    assert!(!b.app.enregistrer_sous_vers(&existant, Some(action)));
    b.app.remplacement_choisir(ChoixRemplacement::Remplacer);
    pomper(&mut b);
    assert!(project::load(&existant).is_ok());
    assert!(b.app.path.is_none() && b.app.welcome, "le dernier onglet est fermé : un projet vide le remplace");
    assert_eq!(dossiers_remplaces(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn a_folder_that_cannot_be_renamed_says_why_and_writes_nothing() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = dossier("remplacer-verrou");
    let existant = mon_film(&dir);
    let mut b = sans_titre();
    b.app.request(Pending::Quit);
    let action = b.app.pending.take().expect("action");
    assert!(!b.app.enregistrer_sous_vers(&existant, Some(action)));
    // Un fichier du projet tenu sans partage par un autre programme : Windows refuse de renommer le dossier.
    let tenu = std::fs::OpenOptions::new().read(true).share_mode(0).open(&existant).expect("fichier tenu");
    b.app.remplacement_choisir(ChoixRemplacement::Remplacer);
    drop(tenu);
    let erreur = b.app.save_error.clone().expect("une fenêtre d'erreur dit pourquoi");
    assert!(erreur.contains("Mon film") && erreur.contains("Rien n'a été écrit"), "{erreur}");
    assert!(dossiers_remplaces(&dir).is_empty(), "rien n'est renommé");
    assert!(!b.app.save_en_cours(), "rien n'est écrit");
    assert!(project::load(&existant).is_ok(), "l'ancien projet est intact");
    assert!(b.app.path.is_none());
    rien_n_est_perdu(&mut b);
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------------------------------ un échec pendant une fermeture

#[test]
fn a_failed_write_during_a_close_keeps_the_project_open_modified_and_shows_the_error() {
    let dir = dossier("remplacer-echec-fermeture");
    let manifeste = dir.join("Essai").join("Essai.shot");
    std::fs::create_dir_all(manifeste.parent().unwrap()).unwrap();
    let mut b = banc(projet(4, 20), false);
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    assert!(b.app.save());
    pomper(&mut b);
    assert!(!b.app.modified);
    // Le disque refuse le manifeste (le fichier provisoire est occupé : comme un disque plein) ; le projet a changé.
    std::fs::create_dir_all(manifeste.with_extension("shot.tmp")).expect("obstacle");
    b.app.doc.panels[0].notes = "Plan retouché.".into();
    b.app.modified = true;
    let avant = arbre(manifeste.parent().unwrap());
    for action in [Pending::Quit, Pending::Close] {
        b.app.request(action);
        let a = b.app.pending.take().expect("la boîte demande");
        b.app.enregistrer_puis(a);
        pomper(&mut b);
        assert!(b.app.save_error.is_some(), "la fenêtre d'erreur est affichée");
        assert!(b.app.modified, "le projet reste modifié");
        assert!(!b.app.allow_close && b.app.after_save.is_none());
        assert!(b.app.path.is_some() && b.app.onglets.len() == 1, "le projet reste ouvert");
        // La croix, avec l'erreur affichée : toujours retenue, et la question revient.
        b.app.save_error = None;
        rien_n_est_perdu(&mut b);
        assert!(matches!(b.app.pending, Some(Pending::Quit)), "la question revient avec la croix");
        b.app.pending = None;
    }
    assert!(project::load(&manifeste).is_ok(), "l'ancien projet reste lisible");
    assert_eq!(arbre(manifeste.parent().unwrap()).get("Essai.shot"), avant.get("Essai.shot"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_window_closed_without_a_choice_brings_the_question_back() {
    let mut b = sans_titre();
    b.app.request(Pending::Quit);
    let action = b.app.pending.take().expect("action");
    // (La fenêtre de fichiers est fermée sans choisir : `save_as_then` rend faux sans rien dire.)
    b.app.rendre_la_question(Some(action));
    assert!(matches!(b.app.pending, Some(Pending::Quit)), "la boîte ne disparaît pas comme si de rien n'était");
    rien_n_est_perdu(&mut b);
}

#[test]
fn two_modified_tabs_and_the_cross_ask_once_and_a_cancelled_replacement_closes_nothing() {
    let dir = dossier("remplacer-tout-enregistrer");
    let existant = mon_film(&dir);
    // Deux onglets modifiés : le premier a un nom, le second est sans titre (son enregistrement a besoin d'un choix).
    let mut b = banc(projet(3, 200), false);
    b.app.path = Some(dir.join("Premier").join("Premier.shot"));
    std::fs::create_dir_all(dir.join("Premier")).unwrap();
    b.app.modified = true;
    assert!(b.app.nouvel_onglet());
    assert!(b.app.charger_projet(projet(5, 20)));
    b.app.welcome = false;
    b.app.modified = true;
    b.chauffer(2);
    assert!(b.fermer_la_fenetre());
    assert!(b.app.groupe.is_some(), "une seule boîte liste les projets modifiés");
    // « Enregistrer tout » : l'onglet sans titre vise le projet existant (la fenêtre de fichiers est remplacée par la décision).
    let r = b.app.enregistrer_sous_vers(&existant, None);
    assert!(!r && b.app.remplacement.is_some());
    b.app.remplacement_choisir(ChoixRemplacement::Annuler);
    b.app.groupe_choisir(2);
    assert!(!b.app.allow_close);
    assert!(b.app.infos_onglets().iter().any(|i| i.modifie), "rien n'est enregistré à tort");
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------------------------------ la copie de secours

fn secours_de(b: &mut Banc, racine: &Path) -> PathBuf {
    let r = Recovery::start(racine).expect("séance de secours");
    let d = r.dir.clone();
    b.app.recovery = Some(r);
    d
}

#[test]
fn the_recovery_copy_stays_when_the_window_closes_on_an_unconfirmed_modified_project() {
    let racine = dossier("remplacer-secours-reste");
    let mut b = sans_titre();
    let seance = secours_de(&mut b, &racine);
    // La suite d'Anthony, jusqu'au refus : rien n'est enregistré.
    assert!(b.fermer_la_fenetre());
    let action = b.app.pending.take().expect("action");
    let existant = mon_film(&dossier("remplacer-secours-reste-film"));
    assert!(!b.app.enregistrer_sous_vers(&existant, Some(action)));
    assert!(seance.is_dir(), "la copie de secours est là tant que rien n'est enregistré");
    // La fenêtre se ferme malgré tout (le système, un plantage de l'affichage) : la copie RESTE, à jour.
    b.app.fermeture();
    assert!(seance.is_dir(), "la copie de secours n'est pas effacée");
    assert!(seance.join("projet").join("secours.shot").is_file(), "elle contient le projet modifié");
    let orphelines = recovery::orphans(&racine, None);
    assert_eq!(orphelines.len(), 1, "SHOT la proposera au prochain démarrage");
    let lu = project::load(&orphelines[0].manifest).expect("la copie se lit");
    assert_eq!(lu.doc.panels[0].layers[0].frames.len(), 5, "c'est bien le travail du projet sans titre");
    let _ = std::fs::remove_dir_all(&racine);
}

#[test]
fn the_recovery_copy_is_erased_after_a_chosen_abandon_or_a_successful_save() {
    // « Ne pas enregistrer » : Quitter est confirmé, la copie est effacée.
    let racine = dossier("remplacer-secours-efface");
    let mut b = sans_titre();
    let seance = secours_de(&mut b, &racine);
    b.app.request(Pending::Quit);
    assert!(b.app.pending.is_some());
    let action = b.app.pending.take().expect("action");
    b.app.perform(action);
    assert!(b.app.allow_close);
    b.app.fermeture();
    assert!(!seance.exists(), "abandon choisi : la copie est effacée");
    // Un enregistrement réussi, puis la fin : la copie est effacée aussi.
    let racine2 = dossier("remplacer-secours-efface-2");
    let dir = dossier("remplacer-secours-efface-film");
    let mut c = banc(projet(2, 20), false);
    c.app.path = Some(dir.join("Film").join("Film.shot"));
    c.app.modified = true;
    let seance2 = secours_de(&mut c, &racine2);
    assert!(c.app.save());
    pomper(&mut c);
    assert!(!c.app.modified);
    c.app.fermeture();
    assert!(!seance2.exists(), "enregistré : la copie est effacée");
    let _ = std::fs::remove_dir_all(&racine);
    let _ = std::fs::remove_dir_all(&racine2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_final_copy_waits_for_the_running_copy_and_keeps_the_folder() {
    let racine = dossier("remplacer-copie-finale");
    let b = sans_titre();
    let r = Recovery::start(&racine).expect("séance");
    let seance = r.dir.clone();
    r.copie_finale(Snapshot { doc: &b.app.doc, serial: 1, modified: true, title: "Sans titre".into(), project: None, project_written: &Default::default() });
    assert!(seance.join("projet").join("secours.shot").is_file());
    assert!(seance.join("seance.json").is_file());
    assert_eq!(recovery::orphans(&racine, None).len(), 1);
    let _ = std::fs::remove_dir_all(&racine);
}

// ------------------------------------------------------------------------------------------ petites choses

#[test]
fn the_replaced_folder_name_carries_the_date_and_never_collides() {
    let dir = dossier("remplacer-nom");
    let nom = crate::remplacer::nom_de_remplacement("Mon film", 1_793_224_027);
    assert!(nom.starts_with("Mon film (remplacé le 2026-10-28 ") && nom.ends_with(')'), "{nom}");
    std::fs::create_dir_all(dir.join("A")).unwrap();
    std::fs::create_dir_all(dir.join("B")).unwrap();
    std::fs::create_dir_all(dir.join("C")).unwrap();
    std::fs::write(dir.join("A").join("x.txt"), b"a").unwrap();
    let un = crate::remplacer::mettre_de_cote(&dir.join("A"), "Pris").expect("renommé");
    let deux = crate::remplacer::mettre_de_cote(&dir.join("B"), "Pris").expect("renommé, nom pris");
    let trois = crate::remplacer::mettre_de_cote(&dir.join("C"), "Pris").expect("renommé, noms pris");
    assert_eq!([un.file_name().unwrap(), deux.file_name().unwrap(), trois.file_name().unwrap()], ["Pris", "Pris 2", "Pris 3"]);
    assert_eq!(std::fs::read(un.join("x.txt")).unwrap(), b"a", "le contenu est intact");
    assert!(crate::remplacer::date_locale(Some(1_793_224_027)).contains("/2026 à "));
    assert_eq!(crate::remplacer::date_locale(None), "date inconnue");
    let _ = std::fs::remove_dir_all(&dir);
}
