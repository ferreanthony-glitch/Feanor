//! Travailler à plusieurs, côté application (`cooperationui.rs`) : les barrières de lecture seule, l'enregistrement par la voie
//! ordinaire, les onglets, le repos, le partage d'un projet existant.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::cooperation::{Delais, Horloge, Session};
use crate::cooperation_tests::{confirmer, film, partager, Poste};
use crate::depot::DossierPartage;
use crate::equipe::Identite;
use crate::fiabilite_fixture as fx;
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::project::{self, Loaded};
use crate::verrous::Cible;

/// Une application qui a ouvert un film partagé comme Anthony, et un poste d'essai pour Léa.
fn app_partagee(nom: &str, n: usize) -> (Banc, Poste, Horloge, PathBuf) {
    let racine = fx::dossier(&format!("coop-ui-{nom}"));
    let manifeste = partager(&film(n), &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let Loaded { doc, partage, written, .. } = project::load(&manifeste).expect("projet partagé");
    let mut b = banc(doc, false);
    b.app.path = Some(manifeste.clone());
    b.app.written = written;
    let moi = Identite { id: "anthony".into(), nom: "Anthony".into(), couleur: [220, 60, 60], machine: "poste".into(), seance: "s1".into() };
    let depot = Arc::new(DossierPartage::nouveau(&manifeste));
    let s = Session::rejoindre(depot, moi, partage.expect("partagé"), &mut b.app.doc, Delais::default(), h.clone(), false, None).expect("session");
    b.app.partage = Some(s);
    let lea = Poste::ouvrir(&manifeste, "Lea", &h);
    (b, lea, h, racine)
}

/// Un tour du moteur d'Anthony puis une image d'écran (qui pose ce que le moteur a préparé).
fn image(b: &mut Banc) {
    b.app.partage.as_ref().expect("session").pomper();
    b.image(Vec::new(), 0.05);
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

#[test]
fn drawing_in_a_plan_one_does_not_hold_is_refused_with_the_reason_and_allowed_once_taken() {
    let (mut b, mut lea, h, _) = app_partagee("barriere-plan", 3);
    lea.prendre_plan(0).expect("libre");
    confirmer(&h);
    lea.tour();
    image(&mut b);
    // Le plan 0 est à Léa : on ne dessine pas dedans.
    b.app.cur_panel = 0;
    assert!(b.app.content_refuses());
    assert!(b.app.status.contains("Lea"), "{}", b.app.status);
    assert!(!b.app.layer_editable());
    // Le plan 1 est libre : refusé aussi, avec le chemin pour le prendre.
    b.app.cur_panel = 1;
    assert!(b.app.content_refuses());
    assert!(b.app.status.contains("libre"), "{}", b.app.status);
    // Anthony le prend : il peut dessiner.
    b.app.run_action("equipe.prendre_plan");
    assert!(!b.app.content_refuses(), "ma prise est vérifiée tout de suite");
    assert!(b.app.layer_editable());
    // Un projet qui n'est pas partagé n'a aucune de ces barrières.
    let mut solo = banc(film(2), false);
    assert!(!solo.app.content_refuses() && !solo.app.storyboard_refuses());
    solo.image(Vec::new(), 0.05);
}

#[test]
fn the_montage_needs_the_token_and_a_plan_one_creates_is_one_s_own() {
    let (mut b, mut lea, h, _) = app_partagee("barriere-montage", 3);
    let avant = b.app.doc.panels.len();
    // Sans jeton : refusé.
    b.app.new_panel();
    assert_eq!(b.app.doc.panels.len(), avant);
    assert!(b.app.status.contains("jeton"), "{}", b.app.status);
    // Marquer un envoi est une décision sur tout le film : le jeton aussi.
    b.app.status.clear();
    b.app.envois_ouvrir_marquage();
    assert!(b.app.envois_ui.nouveau.is_none() && b.app.status.contains("jeton"), "{}", b.app.status);
    // Léa tient le jeton : refusé, avec son nom.
    lea.s.prendre(&[Cible::Montage], false);
    confirmer(&h);
    lea.tour();
    image(&mut b);
    b.app.new_panel();
    assert_eq!(b.app.doc.panels.len(), avant);
    assert!(b.app.status.contains("Lea"), "{}", b.app.status);
    // Léa le rend, Anthony le prend et crée un plan : il est à lui dès sa naissance.
    lea.s.rendre(&[Cible::Montage]);
    lea.tour();
    image(&mut b);
    b.app.run_action("equipe.prendre_montage");
    h.avancer(9_000);
    image(&mut b);
    b.app.new_panel();
    assert_eq!(b.app.doc.panels.len(), avant + 1);
    b.image(Vec::new(), 0.05);
    let neuf = b.app.doc.panels[b.app.cur_panel].id;
    assert!(b.app.partage.as_ref().expect("session").tiens(Cible::Plan(neuf)), "le plan créé est à moi");
    assert!(!b.app.content_refuses());
}

#[test]
fn a_marked_image_is_plan_content_not_montage() {
    let (mut b, _, _, _) = app_partagee("marque", 3);
    // Sans le jeton mais en tenant le plan : poser une marque d'image est permis.
    b.app.run_action("equipe.prendre_plan");
    b.app.cur_frame = 1;
    b.app.toggle_mark();
    assert_eq!(b.app.doc.panels[b.app.cur_panel].marks.len(), 1, "{}", b.app.status);
    // Une durée change le montage : refusée sans le jeton.
    let avant = b.app.doc.panels[b.app.cur_panel].length;
    b.app.edit_panel_length(avant + 5);
    assert_eq!(b.app.doc.panels[b.app.cur_panel].length, avant);
}

#[test]
fn saving_a_shared_project_writes_what_one_holds_by_the_ordinary_path() {
    let (mut b, mut lea, h, racine) = app_partagee("enregistrer-ui", 3);
    b.app.cur_panel = 0;
    b.app.run_action("equipe.prendre_plan");
    h.avancer(9_000);
    image(&mut b);
    // Un trait : un dessin du plan change.
    let cel = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    {
        let c = b.app.doc.cels.get_mut(&cel).expect("dessin");
        c.raster.tile_mut(0)[0] = eframe::egui::Color32::from_rgba_premultiplied(77, 1, 2, 255);
        c.version += 1;
    }
    b.app.modified = true;
    assert!(b.app.save());
    pomper_enregistrement(&mut b);
    assert!(!b.app.modified, "enregistré");
    let plan = fx::lire_json(&racine.join("Film").join("plans").join(format!("{}.json", b.app.doc.panels[0].id)));
    assert_eq!(plan["revision"], 2);
    // Le fichier de structure n'a pas bougé (pas de jeton) et reste au format des projets partagés.
    assert_eq!(fx::lire_json(&racine.join("Film").join("Film.pelure"))["version"], project::version_partage_ecrite());
    // Léa voit le dessin.
    lea.tour();
    lea.tour();
    assert_eq!(lea.rouge(0), 77);
    // Le chemin du projet n'a pas changé et le projet reste partagé.
    assert!(b.app.partage.is_some());
}

/// Les projets classiques passent de `.pelure` à `.shot` à leur enregistrement (2026-10-23) ; pas un film partagé : sa
/// structure garde son nom, que des équipiers à une version d'avant cherchent.
#[test]
fn a_shared_film_keeps_its_pelure_structure_when_saved() {
    let (mut b, _, h, racine) = app_partagee("garde-pelure", 3);
    let structure = racine.join("Film").join("Film.pelure");
    b.app.run_action("equipe.prendre_montage");
    b.app.run_action("equipe.prendre_plan");
    h.avancer(9_000);
    image(&mut b);
    b.app.doc.properties.title = "Titre changé par le montage".into();
    b.app.modified = true;
    assert!(b.app.save());
    pomper_enregistrement(&mut b);
    assert!(!b.app.modified && b.app.save_error.is_none(), "{}", b.app.status);
    assert!(structure.is_file(), "la structure garde son nom");
    assert!(!structure.with_extension("shot").exists(), "aucun manifeste .shot à côté");
    assert_eq!(b.app.path.as_ref(), Some(&structure));
    assert!(b.app.partage.is_some());
    assert_eq!(fx::lire_json(&structure)["version"], project::version_partage_ecrite());
    // « Partager ce projet » : un nouveau film partagé s'écrit aussi sous `.pelure`, quel que soit le nom tapé.
    assert_eq!(project::target_for_partage(&racine.join("Autre.shot")).1, racine.join("Autre").join("Autre.pelure"));
}

#[test]
fn save_as_makes_a_flat_classic_copy_and_leaves_the_sharing() {
    let (mut b, _, h, racine) = app_partagee("enregistrer-sous", 3);
    b.app.run_action("equipe.prendre_plan");
    h.avancer(9_000);
    image(&mut b);
    let copie = racine.join("Copie").join("Copie.pelure");
    assert!(b.app.start_save(copie.clone(), None));
    pomper_enregistrement(&mut b);
    assert!(b.app.partage.is_none(), "la copie à plat n'est plus un film partagé");
    assert_eq!(b.app.path.as_ref(), Some(&copie));
    let v = fx::lire_json(&copie);
    assert_eq!(v["version"], project::VERSION);
    assert!(v.get("partage").is_none());
    assert_eq!(project::load(&copie).expect("copie").doc.panels.len(), 3);
}

#[test]
fn closing_the_project_gives_back_what_one_held() {
    let (mut b, mut lea, h, _) = app_partagee("fermer", 3);
    b.app.run_action("equipe.prendre_plan");
    h.avancer(9_000);
    image(&mut b);
    lea.tour();
    assert!(matches!(lea.etat(0), crate::verrous::EtatPrise::Autre(_)));
    b.app.lacher_la_session();
    // (La fermeture se fait sur un fil.)
    for _ in 0..200 {
        std::thread::sleep(Duration::from_millis(10));
        lea.tour();
        if matches!(lea.etat(0), crate::verrous::EtatPrise::Libre) {
            return;
        }
    }
    panic!("le plan rendu à la fermeture n'est pas libre : {:?}", lea.etat(0));
}

#[test]
fn a_hidden_tab_keeps_its_session_and_the_project_in_front_is_unaffected() {
    let (mut b, _, _, _) = app_partagee("onglets", 3);
    assert!(b.app.nouvel_onglet());
    assert!(b.app.partage.is_none(), "l'onglet neuf n'a pas de session");
    b.app.activer_onglet(0);
    assert!(b.app.partage.is_some(), "elle est revenue avec son onglet");
    assert_eq!(b.app.doc.panels.len(), 3);
}

#[test]
fn an_idle_application_with_a_shared_project_open_asks_for_no_repaint() {
    crate::fxcache::monde::ecrire();
    let (mut b, _, _, _) = app_partagee("repos", 4);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT finit par se reposer avec un film partagé ouvert");
    b.app.partage.as_ref().expect("session").pomper();
    let mesures: Vec<_> = (0..10).map(|_| b.image(Vec::new(), 0.5)).collect();
    assert!(mesures.iter().all(|m| m.reveil >= Duration::from_secs(1)), "{:?}", mesures.iter().map(|m| m.reveil).min());
    // Fenêtre Équipe ouverte : au repos aussi.
    b.app.equipe_ui.fenetre = true;
    b.chauffer(3);
    assert!(se_reposer(&mut b, 3000) < 3000);
}

#[test]
fn every_screen_of_the_team_draws_without_a_panic_and_names_the_others() {
    let (mut b, mut lea, h, _) = app_partagee("fumee", 5);
    b.garder_textes = true;
    lea.prendre_plan(1).expect("libre");
    lea.s.prendre(&[Cible::Plan(lea.plan_id(2))], false);
    lea.s.regarde(Some(lea.plan_id(3)), "Animation");
    confirmer(&h);
    lea.tour();
    b.app.equipe_ui.fenetre = true;
    for _ in 0..4 {
        image(&mut b);
    }
    assert!(b.textes.iter().any(|t| t.contains("Équipe")), "la fenêtre Équipe est là");
    assert!(b.textes.iter().any(|t| t.contains("Lea")), "Léa est nommée : {:?}", b.textes.iter().filter(|t| t.len() < 40).collect::<Vec<_>>());
    // Les plans de Léa sont refusés avec son nom, les menus d'équipe se construisent, l'animation aussi.
    for mode in [crate::app::Mode::Storyboard, crate::app::Mode::Animation, crate::app::Mode::Storyboard] {
        b.app.set_mode(mode);
        for _ in 0..3 {
            image(&mut b);
        }
    }
    // Un brouillon et une copie en conflit dans le dossier : la fenêtre les liste sans planter.
    let dir = b.app.path.as_ref().and_then(|p| p.parent()).expect("dossier").to_path_buf();
    std::fs::create_dir_all(dir.join("partage").join("brouillons")).expect("dossier");
    std::fs::write(dir.join("partage").join("brouillons").join(format!("plan-{}.lea.1.perdu.json", lea.plan_id(1))), b"{}").expect("brouillon");
    std::fs::write(dir.join("plans").join(format!("{} (Copie en conflit de Lea 2026-10-20).json", lea.plan_id(0))), b"{}").expect("copie");
    h.avancer(4_000);
    for _ in 0..3 {
        image(&mut b);
    }
    assert!(b.textes.iter().any(|t| t.contains("Brouillons")), "{:?}", b.textes.iter().filter(|t| t.len() < 40).collect::<Vec<_>>());
    assert!(b.textes.iter().any(|t| t.contains("Copies en conflit")));
}

#[test]
fn sharing_an_open_project_writes_it_on_a_thread_then_opens_it_in_a_new_tab() {
    let racine = fx::dossier("coop-ui-partager");
    let mut b = banc(film(3), false);
    let manifeste = racine.join("Film").join("Film.pelure");
    b.app.demarrer_le_partage(manifeste.clone());
    assert!(b.app.equipe_ui.partage.is_some());
    for _ in 0..800 {
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(5));
        if b.app.partage.is_some() && b.app.open_job.is_none() {
            break;
        }
    }
    assert!(b.app.partage.is_some(), "le film partagé est ouvert : {}", b.app.status);
    assert_eq!(b.app.onglets.len(), 2, "dans un nouvel onglet (le projet de départ reste)");
    assert_eq!(b.app.doc.panels.len(), 3);
    assert_eq!(fx::lire_json(&manifeste)["version"], project::version_partage_ecrite());
    // Le projet de départ n'a pas été touché : toujours sans chemin, sans partage.
    b.app.activer_onglet(0);
    assert!(b.app.partage.is_none() && b.app.path.is_none());
    // Un dossier existant est refusé.
    b.app.demarrer_le_partage(manifeste);
    assert!(b.app.status.contains("existe déjà") || b.app.status.contains("Partage annulé"), "{}", b.app.status);
}

#[test]
fn the_team_settings_round_trip_and_stay_in_bounds() {
    let r = crate::equipe::ReglageEquipe { id: "a b!c".into(), nom: "x".repeat(100), couleur: None, bail_minutes: 0, balayage_secondes: 9999, prise_automatique: true, notifications: false };
    let n = r.nettoye();
    assert_eq!(n.id, "abc");
    assert_eq!(n.nom.chars().count(), 40);
    assert_eq!(n.bail_minutes, 2);
    assert_eq!(n.balayage_secondes, 120);
    let s = crate::settings::Settings { equipe: n.clone(), ..Default::default() };
    let texte = toml::to_string(&s).expect("toml");
    let relu: crate::settings::Settings = toml::from_str(&texte).expect("relu");
    assert_eq!(relu.equipe, n);
    // Des réglages d'avant l'équipe se lisent avec les valeurs d'origine.
    let vieux: crate::settings::Settings = toml::from_str("copie_secours_minutes = 3").expect("anciens réglages");
    assert_eq!(vieux.equipe, crate::equipe::ReglageEquipe::default());
    assert_eq!(vieux.equipe.bail_minutes, 10);
}
