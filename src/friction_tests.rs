//! Noter une friction sans quitter le dessin (`friction.rs`, `frictionui.rs` ; Anthony, 2026-11-06) : la note sans
//! interface, puis le geste entier dans l'application (la touche, la capture, le champ, le fichier).

use std::sync::Arc;

use eframe::egui::{epaint::Shape, Color32, ColorImage, Event, Key, Modifiers, UserData, ViewportId};

use crate::app::Tool;
use crate::document::Document;
use crate::friction::{self, ajouter, duree, Avant, Ecriture, Note};
use crate::frictionui::{self, ACTION};
use crate::perf_tests::{banc_taille, Banc};

const DT: f64 = 1.0 / 60.0;

fn note() -> Note {
    Note {
        quand: (2026, 11, 6, 10, 15, 23),
        texte: "le lasso part tout seul\nquand je lève le stylet".into(),
        projet: "MonFilm.shot".into(),
        espace: "Storyboard".into(),
        outil: "Pinceau".into(),
        plan: "3 sur 12".into(),
        avant: vec![Avant { nom: "Gomme".into(), chemin: "anneau", il_y_a: 2.4 }, Avant { nom: "Plan suivant".into(), chemin: "clavier", il_y_a: 75.0 }],
        capture: true,
        version: "FEANOR 0.1.0".into(),
    }
}

// ------------------------------------------------------------------------------------------------ la note, sans interface

#[test]
fn a_note_says_when_where_what_came_before_and_shows_the_window() {
    let n = note();
    assert_eq!(n.horodatage(), "2026-11-06 10:15:23");
    assert_eq!(n.nom_de_capture(), "friction-2026-11-06-101523.png");
    let e = n.entree();
    // Le titre tient sur une ligne, même si le texte en avait deux.
    assert!(e.contains("\n## 2026-11-06 10:15:23 — le lasso part tout seul quand je lève le stylet\n"), "{e}");
    assert!(e.contains("- Projet : MonFilm.shot · Espace : Storyboard · Outil : Pinceau · Plan : 3 sur 12\n"), "{e}");
    assert!(e.contains("Gomme (anneau, il y a 2 s) ; Plan suivant (clavier, il y a 1 min)"), "{e}");
    assert!(e.contains("- Capture : ![friction-2026-11-06-101523.png](friction-2026-11-06-101523.png)\n"), "{e}");
    assert!(e.trim_end().ends_with("- FEANOR 0.1.0"), "{e}");
    // Sans texte, sans capture, sans commande d'avant, sans projet : la note le dit, sans ligne vide de sens.
    let vide = Note { quand: (2026, 1, 2, 3, 4, 5), ..Default::default() };
    let e = vide.entree();
    assert!(e.contains("## 2026-01-02 03:04:05 — (sans texte)"), "{e}");
    assert!(e.contains("aucune commande depuis le lancement") && e.contains("Capture : aucune"), "{e}");
    assert!(!e.contains("Projet :") && !e.contains("Outil :"), "{e}");
    assert_eq!(duree(0.2), "0 s");
    assert_eq!(duree(59.4), "59 s");
    assert_eq!(duree(60.0), "1 min");
    assert_eq!(duree(3725.0), "1 h 02");
    assert_eq!(duree(-5.0), "0 s");
}

#[test]
fn notes_pile_up_in_one_file_with_their_captures_beside() {
    let dossier = std::env::temp_dir().join(format!("shot-friction-{}-{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')'], ""));
    let _ = std::fs::remove_dir_all(&dossier);
    let pixels = vec![200u8; 4 * 3 * 4];
    let fichier = Ecriture { dossier: dossier.clone(), note: note(), image: Some((pixels, 4, 3)) }.faire().expect("la note s'écrit");
    assert_eq!(fichier, dossier.join(friction::FICHIER));
    let image = image::open(dossier.join("friction-2026-11-06-101523.png")).expect("la capture est un PNG lisible");
    assert_eq!((image.width(), image.height()), (4, 3));
    // Une seconde note, sans capture : elle s'ajoute à la fin, l'en-tête n'est écrit qu'une fois.
    let mut seconde = note();
    seconde.quand.5 = 59;
    seconde.texte = "la seconde".into();
    Ecriture { dossier: dossier.clone(), note: seconde, image: None }.faire().unwrap();
    let texte = std::fs::read_to_string(&fichier).unwrap();
    assert_eq!(texte.matches("# Frictions notées dans FEANOR").count(), 1);
    assert_eq!(texte.matches("\n## ").count(), 2);
    assert!(texte.find("le lasso part tout seul").unwrap() < texte.find("la seconde").unwrap(), "la plus récente en bas");
    assert!(texte.contains("Capture : aucune"), "{texte}");
    // Une capture impossible (taille fausse) n'empêche pas la note.
    let mut troisieme = note();
    troisieme.quand.5 = 7;
    Ecriture { dossier: dossier.clone(), note: troisieme, image: Some((vec![0u8; 5], 4, 3)) }.faire().expect("la note part quand même");
    assert_eq!(std::fs::read_to_string(&fichier).unwrap().matches("\n## ").count(), 3);
    assert!(ajouter(&dossier.join("pas").join("la").join("x.md"), "x").is_err(), "un dossier absent : une erreur dite, pas un plantage");
    let _ = std::fs::remove_dir_all(&dossier);
}

// -------------------------------------------------------------------------------------------- dans l'application entière

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    let _ = std::fs::remove_dir_all(frictionui::dossier());
    b
}

fn touche(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: Modifiers::NONE }], DT);
}

fn capture(largeur: usize, hauteur: usize) -> Event {
    Event::Screenshot { viewport_id: ViewportId::ROOT, user_data: UserData::default(), image: Arc::new(ColorImage::new([largeur, hauteur], Color32::from_rgb(40, 80, 120))) }
}

fn textes_peints(shape: &Shape, sortie: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => sortie.push(t.galley.job.text.clone()),
        Shape::Vec(v) => v.iter().for_each(|s| textes_peints(s, sortie)),
        _ => {}
    }
}

fn textes(b: &Banc) -> Vec<String> {
    let mut t = Vec::new();
    for s in &b.formes {
        textes_peints(&s.shape, &mut t);
    }
    t
}

/// Laisse le fil d'écriture finir et l'application le dire dans la barre d'état.
fn attendre_la_note(b: &mut Banc) -> String {
    for _ in 0..400 {
        b.image(Vec::new(), DT);
        if b.app.status.starts_with("Friction notée dans") || b.app.status.starts_with("La friction n'a pas pu") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(b.app.status.starts_with("Friction notée dans"), "{}", b.app.status);
    std::fs::read_to_string(frictionui::dossier().join(friction::FICHIER)).expect("le fichier des notes")
}

#[test]
fn one_key_one_line_and_the_friction_is_filed_with_the_window_and_the_last_commands() {
    let mut b = banc();
    assert_eq!(b.app.keymap.label(ACTION), "F9");
    // Ce qu'on faisait juste avant.
    b.app.usage.source = Some(crate::usage::Source::Clavier);
    b.app.run_action("vue.miroir");
    b.app.usage.source = Some(crate::usage::Source::Anneau);
    b.app.run_action("outil.gomme");
    b.chauffer(2);
    // La touche : rien ne se montre encore, la capture est attendue.
    touche(&mut b, Key::F9);
    assert!(b.app.friction.en_cours() && !b.app.friction.en_saisie(), "la capture d'abord, sans rien dessiner de plus");
    assert!(!textes(&b).iter().any(|t| t.contains("Qu'est-ce qui t'a freiné")));
    // La capture arrive : le champ paraît, avec la main.
    b.image(vec![capture(16, 9)], DT);
    b.chauffer(3);
    assert!(b.app.friction.en_saisie());
    assert!(textes(&b).iter().any(|t| t.contains("Qu'est-ce qui t'a freiné")), "{:?}", textes(&b));
    assert!(b.ctx.wants_keyboard_input(), "le champ a la main : on tape tout de suite");
    // On tape : les lettres vont dans le champ, pas aux raccourcis (M est le miroir de la vue).
    let miroir = b.app.usage.compte.commandes["vue.miroir"].total();
    b.image(vec![Event::Text("la gomme reste prise".into())], DT);
    b.image(vec![Event::Key { key: Key::M, physical_key: Some(Key::M), pressed: true, repeat: false, modifiers: Modifiers::NONE }], DT);
    assert_eq!(b.app.friction.texte, "la gomme reste prise");
    assert_eq!(b.app.usage.compte.commandes["vue.miroir"].total(), miroir, "aucun raccourci pendant la saisie");
    // Entrée : la note part, on est de retour dans le dessin.
    touche(&mut b, Key::Enter);
    assert!(!b.app.friction.en_cours());
    assert_eq!(b.app.friction.notees, 1);
    let texte = attendre_la_note(&mut b);
    assert!(texte.contains("— la gomme reste prise\n"), "{texte}");
    assert!(texte.contains("Outil : Gomme") && texte.contains("Espace : "), "où on était : {texte}");
    assert!(texte.contains("Gomme (anneau, il y a ") && texte.contains("Miroir de la vue (clavier, il y a "), "ce qu'on venait de faire : {texte}");
    assert!(texte.find("Gomme (anneau").unwrap() < texte.find("Miroir de la vue (clavier").unwrap(), "de la plus récente à la plus ancienne");
    assert!(!texte.contains("Noter une friction ("), "la commande qui prend la note n'est pas dans la note : {texte}");
    assert!(texte.contains("FEANOR "));
    // La capture est à côté, à la taille de la fenêtre capturée.
    let image = std::fs::read_dir(frictionui::dossier()).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "png")).expect("la capture");
    let image = image::open(image).unwrap();
    assert_eq!((image.width(), image.height()), (16, 9));
    // Rien n'a été dessiné ni changé dans le projet.
    assert!(b.app.stroke.is_none());
    assert_eq!(b.app.history.undo_len(), 0);
}

#[test]
fn without_a_capture_the_field_still_comes_and_an_empty_note_is_enough() {
    let mut b = banc();
    b.app.run_action(ACTION);
    b.chauffer(2);
    assert!(!b.app.friction.en_saisie());
    // La capture ne vient pas (pas de carte graphique dans cet essai) : 0,6 s plus tard, le champ est là.
    b.image(Vec::new(), 0.7);
    b.chauffer(2);
    assert!(b.app.friction.en_saisie());
    assert!(textes(&b).iter().any(|t| t.contains("Sans capture de la fenêtre")), "{:?}", textes(&b));
    touche(&mut b, Key::Enter);
    let texte = attendre_la_note(&mut b);
    assert!(texte.contains("— (sans texte)") && texte.contains("Capture : aucune"), "{texte}");
    assert!(texte.contains("aucune commande depuis le lancement"), "{texte}");
    // Une seconde note s'ajoute à la première.
    b.app.run_action(ACTION);
    b.image(vec![capture(8, 8)], DT);
    b.chauffer(2);
    b.image(vec![Event::Text("encore".into())], DT);
    touche(&mut b, Key::Enter);
    let texte = attendre_la_note(&mut b);
    assert_eq!(texte.matches("\n## ").count(), 2, "{texte}");
    assert_eq!(b.app.friction.notees, 2);
}

#[test]
fn escape_drops_the_note_and_writes_nothing() {
    let mut b = banc();
    b.app.run_action(ACTION);
    b.image(vec![capture(8, 8)], DT);
    b.chauffer(2);
    b.image(vec![Event::Text("finalement non".into())], DT);
    touche(&mut b, Key::Escape);
    b.chauffer(3);
    assert!(!b.app.friction.en_cours());
    assert_eq!(b.app.friction.notees, 0);
    assert!(b.app.status.contains("abandonnée"), "{}", b.app.status);
    assert!(!frictionui::dossier().join(friction::FICHIER).exists(), "rien n'est écrit");
    assert!(!textes(&b).iter().any(|t| t.contains("Qu'est-ce qui t'a freiné")));
    // Échap pendant l'attente de la capture aussi.
    b.app.run_action(ACTION);
    touche(&mut b, Key::Escape);
    assert!(!b.app.friction.en_cours());
    // « Mes frictions notées » sans note : la barre d'état dit où elles iront, rien ne s'ouvre.
    b.app.run_action(frictionui::ACTION_DOSSIER);
    assert!(b.app.status.contains("Aucune friction notée"), "{}", b.app.status);
}
