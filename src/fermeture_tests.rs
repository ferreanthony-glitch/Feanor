//! Fermer un projet (un onglet, ou la fenêtre) :
//! - rien n'a changé depuis le dernier enregistrement (ou depuis l'ouverture) : aucune question « Modifications non
//!   enregistrées » ; ouvrir un projet, regarder (zoom, vue, outils, panneaux, plans, lecture…) puis fermer ne demande rien ;
//! - « Enregistrer » dans la question : l'écriture se fait sur un autre fil, puis l'onglet (ou la fenêtre) se ferme tout seul ;
//! - une écriture qui échoue garde le projet ouvert, avec l'erreur, et il reste marqué comme modifié.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::app::Pending;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fiabilite_fixture::dossier;
use crate::perf_tests::{banc, Banc};

fn projet(n: usize) -> Document {
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

/// Des images jusqu'à ce que `fini` soit vrai (ou `limite`).
fn jusqu_a(b: &mut Banc, limite: Duration, fini: impl Fn(&Banc) -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < limite {
        if fini(b) {
            return true;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
    }
    fini(b)
}

/// Un petit projet enregistré dans un dossier d'essai, que `Ouvrir` rouvre : `(dossier, manifeste)`.
fn projet_enregistre(nom: &str) -> (PathBuf, PathBuf) {
    let dir = dossier(nom);
    let mut b = banc(projet(3), false);
    let manifeste = dir.join("P").join("P.shot");
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    assert!(b.app.save());
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| !b.app.save_en_cours()));
    assert!(!b.app.modified, "enregistré");
    (dir, manifeste)
}

/// Ouvre `manifeste` dans un banc neuf, comme Fichier > Ouvrir le fait (sur un autre fil), et attend qu'il soit là (le projet
/// s'ouvre dans un onglet à lui, qui devient l'onglet affiché).
fn banc_ouvert(manifeste: &Path) -> Banc {
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    b.app.open_path(manifeste.to_path_buf());
    assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.open_job.is_none()), "l'ouverture finit : {}", b.app.status);
    b.chauffer(3);
    assert!(b.app.path.as_deref().is_some_and(|p| crate::onglets::meme_chemin(p, manifeste)), "le projet est ouvert : {:?}", b.app.path);
    b
}

#[test]
fn opening_a_saved_project_then_closing_does_not_ask() {
    let (dir, manifeste) = projet_enregistre("fermeture-ouvrir-fermer");
    let mut b = banc_ouvert(&manifeste);
    let onglets = b.app.onglets.len();
    assert!(!b.app.modified, "un projet qu'on vient d'ouvrir n'est pas modifié");
    assert!(!b.app.infos_onglets().iter().any(|i| i.a_perdre()));
    b.app.request(Pending::Close);
    assert!(b.app.pending.is_none(), "aucune question");
    assert_eq!(b.app.onglets.len(), onglets - 1, "l'onglet est fermé");
    assert!(b.app.path.as_deref().is_none_or(|p| !crate::onglets::meme_chemin(p, &manifeste)), "ce n'est plus lui qui est affiché");
    // Quitter : pas de question non plus, la fenêtre peut se fermer.
    let mut c = banc_ouvert(&manifeste);
    c.app.request(Pending::Quit);
    assert!(c.app.pending.is_none() && c.app.allow_close, "quitter ne demande rien");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Les commandes qui ne changent que ce qu'on voit ou l'outil en main, jamais le projet.
const SANS_EFFET_SUR_LE_PROJET: &[&str] = &[
    "vue.zoom_avant",
    "vue.zoom_arriere",
    "vue.rotation_horaire",
    "vue.rotation_antihoraire",
    "vue.reinit_rotation",
    "vue.reinit_zoom",
    "vue.ajuster",
    "vue.miroir",
    "vue.grille",
    "vue.pelure",
    "vue.table_lumineuse",
    "vue.dessus",
    "vue.cadres_securite",
    "vue.planches",
    "vue.concentration",
    "vue.concentration",
    "vue.feuille_exposition",
    "vue.historique",
    "vue.commentaires",
    "vue.references",
    "vue.personnages",
    "vue.vignettes_panneau",
    "vue.reglages",
    "vue.pinceaux",
    "vue.couleur",
    "vue.calques",
    "vue.fiche",
    "vue.barre_panneaux",
    "outil.pinceau",
    "outil.crayon",
    "outil.gomme",
    "outil.remplissage",
    "outil.pipette",
    "outil.selection",
    "outil.selection_couleur",
    "outil.cutter",
    "outil.formes",
    "outil.rectangle",
    "outil.ellipse",
    "outil.polyligne",
    "outil.texte",
    "outil.perspective",
    "outil.grab",
    "outil.pull",
    "outil.main",
    "outil.camera",
    "outil.transformer",
    "outil.taille_plus",
    "outil.taille_moins",
    "camera.vue",
    "camera.masque",
    "cadrage.tiers",
    "cadrage.grille_or",
    "cadrage.spirale",
    "cadrage.diagonales",
    "cadrage.croix",
    "cadrage.aucune",
    "cadrage.format_suivant",
    "espace.storyboard",
    "espace.montage",
    "espace.vignettes",
    "espace.standard",
    "espace.dessin",
    "espace.timeline",
    "mode.basculer",
    "lecture.basculer",
    "lecture.boucle",
    "panneau.suivant",
    "panneau.precedent",
    "montage.debut",
    "montage.fin",
    "timeline.tout_voir",
    "pelure.panneau",
    "aide.aide_memoire",
    "aide.conseils",
];

/// Ouvre le grand projet d'essai (calques de toutes sortes, caméras, effets, son…) enregistré : `(dossier, manifeste)`.
fn grand_projet_enregistre(nom: &str) -> (PathBuf, PathBuf) {
    let (f, manifeste) = crate::fiabilite_fixture::projet_enregistre(nom);
    let dir = crate::fiabilite_fixture::racine(&manifeste).parent().map(Path::to_path_buf).expect("dossier de travail");
    drop(f);
    (dir, manifeste)
}

#[test]
fn a_rich_project_opened_and_left_idle_is_not_modified_and_view_commands_keep_it_so() {
    let (dir, manifeste) = grand_projet_enregistre("fermeture-grand-projet");
    let mut b = banc_ouvert(&manifeste);
    // Des images au repos (vignettes, rendus de fond, calques de couleur accordés…) : rien ne marque le projet.
    for _ in 0..120 {
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!b.app.modified, "un projet ouvert qu'on laisse au repos n'est pas modifié ({})", b.app.status);
    let mut coupables = Vec::new();
    for id in SANS_EFFET_SUR_LE_PROJET {
        // (Un identifiant qui n'existe plus ne se fait pas passer pour une commande sans effet.)
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == *id), "« {id} » n'est plus une commande");
        b.app.run_action(id);
        b.chauffer(2);
        b.app.playing = false;
        if b.app.modified {
            coupables.push(*id);
            b.app.modified = false;
        }
    }
    assert!(coupables.is_empty(), "ces commandes de vue ou d'outil marquent le projet comme modifié : {coupables:?}");
    // Se promener dans les plans et les images ne le modifie pas non plus.
    for p in [0usize, 1, 2, 1, 0] {
        b.app.set_panel(p.min(b.app.doc.panels.len() - 1));
        b.app.goto_frame(p);
        b.chauffer(2);
    }
    assert!(!b.app.modified, "changer de plan et d'image");
    // Fermer : aucune question.
    let onglets = b.app.onglets.len();
    b.app.request(Pending::Close);
    assert!(b.app.pending.is_none(), "aucune question après avoir seulement regardé");
    assert_eq!(b.app.onglets.len(), onglets - 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn view_only_commands_never_mark_a_small_project_as_modified() {
    let (dir, manifeste) = projet_enregistre("fermeture-vue-seule");
    let mut b = banc_ouvert(&manifeste);
    let mut coupables = Vec::new();
    for id in SANS_EFFET_SUR_LE_PROJET {
        b.app.run_action(id);
        b.chauffer(2);
        b.app.playing = false;
        if b.app.modified {
            coupables.push(*id);
            b.app.modified = false;
        }
    }
    assert!(coupables.is_empty(), "ces commandes de vue ou d'outil marquent le projet comme modifié : {coupables:?}");
    for f in [0usize, 1, 2, 1, 0] {
        b.app.goto_frame(f);
        b.chauffer(1);
    }
    assert!(!b.app.modified, "changer d'image");
    b.app.request(Pending::Close);
    assert!(b.app.pending.is_none(), "aucune question après avoir seulement regardé");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_modified_project_asks_and_save_then_close_writes_it_then_closes_the_tab() {
    let (dir, manifeste) = projet_enregistre("fermeture-enregistrer-fermer");
    let mut b = banc_ouvert(&manifeste);
    let onglets = b.app.onglets.len();
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    {
        let cel = b.app.doc.cels.get_mut(&id).unwrap();
        cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(66, 10, 10, 255);
        cel.version += 1;
    }
    b.app.modified = true;
    b.app.request(Pending::Close);
    let action = b.app.pending.take().expect("la question « Modifications non enregistrées »");
    // « Enregistrer » : l'écriture, puis la fermeture, toutes seules.
    assert!(b.app.save_then(Some(action)));
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.onglets.len() == onglets - 1), "l'onglet se ferme quand l'écriture est finie : {}", b.app.status);
    assert!(b.app.pending.is_none(), "pas de seconde question");
    let relu = crate::project::load(&manifeste).expect("projet lisible");
    let id = relu.doc.panels[0].layers[0].frames[0].expect("dessin");
    assert_eq!(relu.doc.cels[&id].raster.get(0, 0).r(), 66, "le fichier a le changement");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_then_quit_writes_the_project_then_lets_the_window_close() {
    let (dir, manifeste) = projet_enregistre("fermeture-enregistrer-quitter");
    let mut b = banc_ouvert(&manifeste);
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    {
        let cel = b.app.doc.cels.get_mut(&id).unwrap();
        cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(77, 10, 10, 255);
        cel.version += 1;
    }
    b.app.modified = true;
    b.app.request(Pending::Quit);
    let action = b.app.pending.take().expect("la question");
    assert!(b.app.save_then(Some(action)));
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.allow_close), "la fenêtre peut se fermer à la fin de l'écriture : {}", b.app.status);
    let relu = crate::project::load(&manifeste).expect("projet lisible");
    let id = relu.doc.panels[0].layers[0].frames[0].expect("dessin");
    assert_eq!(relu.doc.cels[&id].raster.get(0, 0).r(), 77);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_save_keeps_the_project_open_and_modified_with_the_error() {
    let dir = dossier("fermeture-echec");
    let mut b = banc(projet(3), false);
    b.app.modified = true;
    // Le « dossier » du projet est un fichier : rien ne peut s'y écrire.
    let bloque = dir.join("Bloque");
    std::fs::write(&bloque, b"pas un dossier").unwrap();
    b.app.path = Some(bloque.join("Bloque.shot"));
    assert!(b.app.nouvel_onglet());
    b.app.welcome = false;
    assert!(b.app.activer_onglet(0));
    b.app.request(Pending::Close);
    let action = b.app.pending.take().expect("la question");
    let lance = b.app.save_then(Some(action));
    // (L'échec peut venir tout de suite ou à la fin de l'écriture : dans les deux cas, rien ne se ferme.)
    jusqu_a(&mut b, Duration::from_secs(60), |b| !b.app.save_en_cours());
    b.chauffer(3);
    assert_eq!(b.app.onglets.len(), 2, "l'onglet reste ouvert (écriture lancée : {lance})");
    assert!(b.app.modified, "le projet reste marqué comme modifié");
    assert!(b.app.save_error.is_some(), "l'erreur est dite : {}", b.app.status);
    assert!(b.app.pending.is_none() && b.app.after_save.is_none(), "aucune suite ne reste en attente");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rich_project_just_saved_and_left_idle_stays_clean_and_closes_without_asking() {
    let (dir, manifeste) = grand_projet_enregistre("fermeture-grand-enregistre");
    let mut b = banc_ouvert(&manifeste);
    // Un changement, puis « Enregistrer » (Ctrl+S) : l'écriture se fait sur un autre fil pendant que l'écran tourne.
    b.app.modified = true;
    assert!(b.app.save());
    assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| !b.app.save_en_cours()), "l'écriture finit : {}", b.app.status);
    for _ in 0..120 {
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!b.app.modified, "enregistré, au repos : rien n'est modifié ({})", b.app.status);
    let onglets = b.app.onglets.len();
    b.app.request(Pending::Close);
    assert!(b.app.pending.is_none(), "aucune question juste après un enregistrement");
    assert_eq!(b.app.onglets.len(), onglets - 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rich_project_modified_asks_then_save_and_close_does_not_ask_again() {
    let (dir, manifeste) = grand_projet_enregistre("fermeture-grand-enregistrer-fermer");
    let mut b = banc_ouvert(&manifeste);
    let onglets = b.app.onglets.len();
    b.app.modified = true;
    b.app.request(Pending::Close);
    let action = b.app.pending.take().expect("la question « Modifications non enregistrées »");
    assert!(b.app.save_then(Some(action)));
    assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.onglets.len() == onglets - 1), "l'onglet se ferme quand l'écriture est finie : {} (question : {:?})", b.app.status, b.app.pending.is_some());
    assert!(b.app.pending.is_none(), "pas de seconde question");
    let _ = std::fs::remove_dir_all(&dir);
}
