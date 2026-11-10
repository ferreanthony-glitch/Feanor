//! Les jalons sur le disque (`jalons.rs`) : un jalon est un projet complet qui ne prend presque pas de place, ce que FEANOR écrit
//! ensuite ne le change pas, y revenir remet les fichiers, un retour coupé au milieu se finit, seuls les dix derniers jalons
//! automatiques restent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::fiabilite_fixture::{arbre, dossier, instantane, meme_projet, projet_complet};
use crate::jalons::{self, DOSSIER};
use crate::project;

/// Le projet d'essai complet (cinq plans, bitmap et vectoriel, aplats, hors-feuille, son, vidéo, annexes) enregistré en `.shot`.
fn film(nom: &str) -> (crate::fiabilite_fixture::Fixture, PathBuf) {
    let travail = dossier(nom);
    let f = projet_complet(&travail);
    let manifeste = travail.join("Film").join("Film.shot");
    project::save(&f.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    (f, manifeste)
}

/// Les fichiers du projet, sans ses jalons.
fn arbre_du_projet(projet: &Path) -> BTreeMap<String, u64> {
    arbre(projet).into_iter().filter(|(k, _)| !k.starts_with(&format!("{DOSSIER}/"))).collect()
}

/// Les fichiers d'un jalon, sans sa fiche.
fn arbre_du_jalon(jalon: &Path) -> BTreeMap<String, u64> {
    arbre(jalon).into_iter().filter(|(k, _)| k != "jalon.json").collect()
}

/// Change le projet comme une séance de travail le fait : un pixel dans chaque dessin bitmap, une note, un plan de plus, et tout
/// réécrit (les dessins, les annexes et le manifeste, même ceux qui n'ont pas changé).
fn travailler_et_enregistrer(doc: &mut crate::document::Document, manifeste: &Path) {
    let ids: Vec<_> = doc.cels.keys().copied().collect();
    for id in ids {
        let cel = doc.cels.get_mut(&id).expect("dessin");
        if cel.vector.is_none() {
            cel.raster.tile_mut(0)[7] = eframe::egui::Color32::from_rgba_premultiplied(9, 99, 199, 255);
        }
        cel.version += 1;
    }
    doc.panels[0].notes = "Retouché après le jalon.".into();
    doc.add_scene_after(doc.panels.len() - 1);
    project::ecrire_toujours(true);
    let ecrit = project::save(doc, manifeste, &mut Default::default());
    project::ecrire_toujours(false);
    ecrit.expect("enregistrement");
}

#[test]
fn a_milestone_is_a_whole_project_made_of_links() {
    let (f, manifeste) = film("jalon-complet");
    let projet = manifeste.parent().expect("dossier");
    let (jalon, compte) = jalons::poser(&manifeste, "  Avant   les retakes ", false, 1_794_300_000).expect("jalon");
    assert_eq!(jalon.fiche.nom, "Avant les retakes", "le nom tient sur une ligne, sans blancs autour");
    assert!(!jalon.fiche.auto && jalon.fiche.fini.is_some());
    assert_eq!(jalon.dossier.parent(), Some(projet.join(DOSSIER).as_path()));
    // Les mêmes fichiers que le projet, aux mêmes endroits.
    assert_eq!(arbre_du_jalon(&jalon.dossier), arbre_du_projet(projet));
    // Les dessins, les sons, les images de la vidéo : des liens (aucune place prise) ; le manifeste et les annexes : des copies.
    let dessins = std::fs::read_dir(projet.join("dessins")).expect("dessins").count();
    assert!(compte.liens >= dessins, "{compte:?} pour {dessins} fichiers de dessins");
    assert!(compte.copies >= 1 && compte.copies < compte.liens, "{compte:?}");
    // Il s'ouvre comme un projet, et c'est le même.
    let ouvert = project::load(&jalon.manifeste().expect("manifeste du jalon")).expect("le jalon s'ouvre");
    meme_projet(&instantane(&f.doc, &f.sans_pixels), &instantane(&ouvert.doc, &f.sans_pixels)).expect("le jalon est le projet");
    assert!(jalons::est_dans_un_jalon(&jalon.manifeste().expect("manifeste")));
    assert!(!jalons::est_dans_un_jalon(&manifeste));
    assert_eq!(jalons::lister(projet), vec![jalon]);
}

#[test]
fn what_is_written_after_a_milestone_does_not_change_the_milestone() {
    let (mut f, manifeste) = film("jalon-intact");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    let avant_travail = instantane(&f.doc, &f.sans_pixels);
    let (jalon, _) = jalons::poser(&manifeste, "", true, 1_794_300_000).expect("jalon");
    let garde = arbre(&jalon.dossier);
    travailler_et_enregistrer(&mut f.doc, &manifeste);
    // Le projet a changé ; le jalon, pas d'un octet, dans aucun de ses fichiers.
    assert_ne!(arbre_du_projet(&projet), arbre_du_jalon(&jalon.dossier), "le projet a bien été réécrit");
    assert_eq!(arbre(&jalon.dossier), garde, "un fichier du jalon a changé quand le projet s'est écrit");
    let ouvert = project::load(&jalon.manifeste().expect("manifeste")).expect("le jalon s'ouvre");
    meme_projet(&avant_travail, &instantane(&ouvert.doc, &f.sans_pixels)).expect("le jalon garde le projet d'avant");
    // Aucun reste d'écriture dans le jalon.
    assert!(!garde.keys().any(|k| k.ends_with(".tmp")));
}

#[test]
fn returning_to_a_milestone_puts_every_file_back_and_removes_what_came_after() {
    let (mut f, manifeste) = film("jalon-retour");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    let avant_travail = instantane(&f.doc, &f.sans_pixels);
    let (jalon, _) = jalons::poser(&manifeste, "Avant", false, 1_794_300_000).expect("jalon");
    let garde = arbre_du_jalon(&jalon.dossier);
    travailler_et_enregistrer(&mut f.doc, &manifeste);
    let (apres, _) = jalons::poser(&manifeste, "Après", false, 1_794_300_060).expect("second jalon");
    let garde_apres = arbre(&apres.dossier);
    jalons::restaurer(&projet, &jalon.dossier).expect("retour");
    assert_eq!(arbre_du_projet(&projet), garde, "le projet a les fichiers du jalon, et rien de plus");
    let ouvert = project::load(&manifeste).expect("le projet s'ouvre");
    meme_projet(&avant_travail, &instantane(&ouvert.doc, &f.sans_pixels)).expect("le projet est revenu au jalon");
    // Les jalons, eux, n'ont pas bougé : ni celui d'où l'on vient, ni celui où l'on revient.
    assert_eq!(arbre(&apres.dossier), garde_apres);
    assert_eq!(arbre_du_jalon(&jalon.dossier), garde);
    assert!(!projet.join(DOSSIER).join(".retour").exists(), "la marque du retour est retirée");
    // Et le retour se défait : revenir au second jalon rend le travail.
    jalons::restaurer(&projet, &apres.dossier).expect("retour dans l'autre sens");
    let rouvert = project::load(&manifeste).expect("le projet s'ouvre");
    meme_projet(&instantane(&f.doc, &f.sans_pixels), &instantane(&rouvert.doc, &f.sans_pixels)).expect("le travail est revenu");
}

#[test]
fn a_return_cut_in_the_middle_finishes_at_the_next_opening() {
    let (mut f, manifeste) = film("jalon-coupe");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    let (jalon, _) = jalons::poser(&manifeste, "Avant", false, 1_794_300_000).expect("jalon");
    let garde = arbre_du_jalon(&jalon.dossier);
    travailler_et_enregistrer(&mut f.doc, &manifeste);
    // Le retour a commencé (la marque est écrite, un dessin est déjà remis), puis le courant a sauté.
    let nom = jalon.dossier.file_name().expect("nom").to_string_lossy().into_owned();
    std::fs::write(projet.join(DOSSIER).join(".retour"), &nom).expect("marque");
    let un_dessin = std::fs::read_dir(jalon.dossier.join("dessins")).expect("dessins").flatten().next().expect("un dessin").path();
    std::fs::copy(&un_dessin, projet.join("dessins").join(un_dessin.file_name().expect("nom"))).expect("dessin remis");
    assert_ne!(arbre_du_projet(&projet), garde, "le projet est entre deux états");
    assert_eq!(jalons::finir_un_retour(&projet).expect("fin du retour"), Some("Avant".to_string()));
    assert_eq!(arbre_du_projet(&projet), garde, "le retour est fini");
    assert_eq!(jalons::finir_un_retour(&projet).expect("rien à finir"), None);
    // Une marque vers un jalon qui n'existe plus : rien n'est touché, la marque part.
    std::fs::write(projet.join(DOSSIER).join(".retour"), "2000-01-01_00-00-00").expect("marque");
    assert_eq!(jalons::finir_un_retour(&projet).expect("jalon absent"), None);
    assert_eq!(arbre_du_projet(&projet), garde);
    assert!(!projet.join(DOSSIER).join(".retour").exists());
}

#[test]
fn only_the_ten_latest_automatic_milestones_stay_and_named_ones_are_never_removed() {
    let (_f, manifeste) = film("jalon-elagage");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    for k in 0..12u64 {
        jalons::poser(&manifeste, "À l'ouverture", true, 1_794_300_000 + k * 3600).expect("jalon automatique");
    }
    let nomme = jalons::poser(&manifeste, "Version envoyée", false, 1_794_200_000).expect("jalon nommé").0;
    // Un jalon coupé au milieu de sa pose : il n'a pas de fiche à son vrai nom.
    std::fs::create_dir_all(projet.join(DOSSIER).join(".en-cours-2026-01-01_00-00-00").join("dessins")).expect("reste");
    assert_eq!(jalons::lister(&projet).len(), 13, "le reste n'est pas un jalon");
    assert_eq!(jalons::elaguer(&projet), 2);
    let reste = jalons::lister(&projet);
    assert_eq!(reste.iter().filter(|j| j.fiche.auto).count(), jalons::GARDES_AUTO);
    assert!(reste.iter().any(|j| j.dossier == nomme.dossier), "un jalon nommé, même le plus ancien, reste");
    // Les deux retirés sont les deux plus anciens automatiques.
    let plus_ancien = reste.iter().filter(|j| j.fiche.auto).map(|j| j.fiche.quand).min().expect("jalons");
    assert_eq!(plus_ancien, 1_794_300_000 + 2 * 3600);
    assert!(!projet.join(DOSSIER).join(".en-cours-2026-01-01_00-00-00").exists(), "le reste d'une pose coupée part");
    // Du plus récent au plus ancien.
    assert!(reste.windows(2).all(|w| w[0].fiche.quand >= w[1].fiche.quand));
    // Les jalons posés à la main et laissés sans nom (Ctrl+S) : les trente derniers restent ; ceux qu'on a nommés, tous.
    for k in 0..(jalons::GARDES_SANS_NOM as u64 + 3) {
        jalons::poser(&manifeste, "", false, 1_794_400_000 + k * 60).expect("jalon sans nom");
    }
    assert_eq!(jalons::elaguer(&projet), 3);
    let reste = jalons::lister(&projet);
    let sans_nom: Vec<u64> = reste.iter().filter(|j| !j.fiche.auto && j.fiche.nom.is_empty()).map(|j| j.fiche.quand).collect();
    assert_eq!(sans_nom.len(), jalons::GARDES_SANS_NOM);
    assert_eq!(sans_nom.iter().min(), Some(&(1_794_400_000 + 3 * 60)), "les trois plus anciens sans nom sont partis");
    assert!(reste.iter().any(|j| j.dossier == nomme.dossier) && reste.iter().filter(|j| j.fiche.auto).count() == jalons::GARDES_AUTO);
}

#[test]
fn naming_an_automatic_milestone_keeps_it_and_only_milestones_are_ever_deleted() {
    let (_f, manifeste) = film("jalon-nom");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    let (auto, _) = jalons::poser(&manifeste, "", true, 1_794_300_000).expect("jalon");
    assert_eq!(auto.titre(), {
        let (_, _, _, h, mi) = crate::remplacer::heure_locale(1_794_300_000);
        format!("Jalon de {h:02}:{mi:02}")
    });
    let fiche = jalons::renommer(&auto.dossier, "Le bon découpage").expect("renommer");
    assert!(!fiche.auto && fiche.nom == "Le bon découpage" && fiche.fini == auto.fiche.fini);
    assert_eq!(jalons::lister(&projet)[0].titre(), "Le bon découpage");
    // Un nom vide ne rend pas un jalon nommé automatique, et ne le fait pas automatique non plus.
    assert!(!jalons::renommer(&auto.dossier, "   ").expect("nom vide").auto);
    // Rien d'autre qu'un jalon n'est renommé, supprimé ni remis : pas le dossier des dessins, pas le projet.
    assert!(jalons::supprimer(&projet.join("dessins")).is_err());
    assert!(jalons::supprimer(&projet).is_err());
    assert!(jalons::renommer(&projet.join("dessins"), "x").is_err());
    assert!(jalons::restaurer(&projet, &projet.join("dessins")).is_err());
    assert!(projet.join("dessins").is_dir() && manifeste.is_file());
    jalons::supprimer(&auto.dossier).expect("supprimer");
    assert!(jalons::lister(&projet).is_empty() && manifeste.is_file());
}

#[test]
fn a_project_untouched_since_its_last_milestone_needs_no_new_one() {
    let travail = dossier("jalon-change");
    let mut f = projet_complet(&travail);
    let manifeste = travail.join("Film").join("Film.shot");
    let mut written = Default::default();
    project::save(&f.doc, &manifeste, &mut written).expect("enregistrement");
    assert!(jalons::a_change_depuis_le_dernier(&manifeste), "sans jalon : oui");
    jalons::poser(&manifeste, "", true, 1_794_300_000).expect("jalon");
    assert!(!jalons::a_change_depuis_le_dernier(&manifeste), "rien n'a été écrit depuis le jalon");
    // Un seul dessin réécrit, le manifeste identique : cela compte (l'heure du jalon est celle des fichiers, pas la seconde).
    std::thread::sleep(std::time::Duration::from_millis(30));
    let manifeste_avant = std::fs::read(&manifeste).expect("manifeste");
    let id = *f.doc.cels.iter().find(|(_, c)| c.vector.is_none()).expect("un dessin bitmap").0;
    let cel = f.doc.cels.get_mut(&id).expect("dessin");
    cel.raster.tile_mut(0)[3] = eframe::egui::Color32::from_rgba_premultiplied(1, 2, 3, 255);
    cel.version += 1;
    project::save(&f.doc, &manifeste, &mut written).expect("enregistrement");
    assert_eq!(std::fs::read(&manifeste).expect("manifeste"), manifeste_avant, "le manifeste n'a pas changé");
    assert!(jalons::a_change_depuis_le_dernier(&manifeste), "un dessin a été réécrit depuis le jalon");
}
