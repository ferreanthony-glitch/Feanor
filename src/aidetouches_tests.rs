//! L'aide-mémoire des raccourcis (`aidetouches.rs`) : tenue, la touche R pose sur le dessin les raccourcis de l'outil courant, de
//! l'onglet courant et ceux qui valent partout, lus dans le fichier de raccourcis en vigueur ; relâchée, la carte s'en va.

use eframe::egui::{epaint::Shape, Event, Key, Modifiers};

use crate::aidetouches::{commandes_de_l_onglet, commandes_de_l_outil, sections, CHOISIR_L_OUTIL, PARTOUT};
use crate::app::Tool;
use crate::document::Document;
use crate::keymap::{action_info, Keymap, PresetId};
use crate::keymap_tests::default_map;
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

const NONE: Modifiers = Modifiers::NONE;

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1600, 900);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn textes(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.job.text.clone()),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

fn tous_les_textes(b: &Banc) -> Vec<String> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut out);
    }
    out
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|t| t.contains(texte))
}

fn presser(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: NONE }], 1.0 / 60.0);
}

fn relacher(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: NONE }], 1.0 / 60.0);
}

#[test]
fn every_command_the_card_cites_exists_so_a_typo_cannot_hide_a_line() {
    let mut cites: Vec<&str> = PARTOUT.iter().map(|(a, _)| *a).collect();
    for e in Espace::TOUS {
        cites.extend(commandes_de_l_onglet(e).iter().map(|(a, _)| *a));
    }
    for t in Tool::ALL {
        cites.extend(commandes_de_l_outil(t).iter().map(|(a, _)| *a));
    }
    for a in cites {
        assert!(action_info(a).is_some_and(|i| i.implemented), "{a} n'est pas une commande de SHOT");
    }
}

#[test]
fn the_card_lists_the_tool_the_tab_and_what_works_everywhere_with_their_keys() {
    let map = default_map();
    let s = sections(&map, Tool::Brush, Espace::Storyboard);
    let titres: Vec<&str> = s.iter().map(|s| s.titre.as_str()).collect();
    assert_eq!(titres, ["Pinceau", "Storyboard", "Partout"]);
    let ligne = |section: usize, nom: &str| s[section].lignes.iter().find(|l| l.nom == nom).unwrap_or_else(|| panic!("« {nom} » manque : {:?}", s[section].lignes)).touches.clone();
    assert_eq!(ligne(0, CHOISIR_L_OUTIL), "Alt+B");
    assert_eq!(ligne(0, "Plus gros"), map.keys_text("outil.taille_plus"));
    assert_eq!(ligne(0, "Pipette"), "Alt + clic", "un geste de souris, écrit comme tel");
    assert_eq!(ligne(1, "Nouveau Plan"), map.keys_text("panneau.nouveau"));
    assert_eq!(ligne(2, "Annuler"), "Ctrl+Z");
    assert_eq!(ligne(2, "Mode concentration"), "Z");
    assert_eq!(ligne(2, "Chercher une commande"), "Ctrl+K");
    // Chaque onglet a sa colonne.
    for e in Espace::TOUS {
        let s = sections(&map, Tool::Select, e);
        assert!(s.iter().any(|x| x.titre == e.nom() && !x.lignes.is_empty()), "{e:?} : sa colonne");
    }
    // Chaque outil a la sienne, avec son nom en titre ; sans touche, la colonne le dit au lieu de disparaître.
    for t in Tool::ALL {
        let s = sections(&map, t, Espace::Animation);
        assert_eq!(s[0].titre, t.label(), "{t:?}");
        assert_eq!(s[0].lignes[0].nom, CHOISIR_L_OUTIL, "{t:?}");
        assert!(!s[0].lignes[0].touches.is_empty(), "{t:?}");
    }
    let camera = sections(&map, Tool::Camera, Espace::Storyboard);
    assert_eq!(camera[0].lignes[0].touches, "pas de touche", "la caméra n'a pas de touche par défaut");
}

#[test]
fn a_command_without_a_key_has_no_line_and_a_changed_key_shows_changed() {
    let user = "version = 1\n[[raccourci]]\naction = \"outil.taille_plus\"\ntouches = [\"K\"]\n[[raccourci]]\naction = \"outil.taille_moins\"\ntouches = []\n";
    let map = Keymap::build(PresetId::Shot, Some(user));
    let s = sections(&map, Tool::Brush, Espace::Vignettes);
    let outil = &s[0];
    assert_eq!(outil.lignes.iter().find(|l| l.nom == "Plus gros").map(|l| l.touches.as_str()), Some("K"), "la touche de l'utilisateur");
    assert!(outil.lignes.iter().all(|l| l.nom != "Plus fin"), "plus de touche, plus de ligne");
}

#[test]
fn holding_r_shows_the_card_over_the_drawing_and_releasing_it_takes_it_away() {
    let mut b = banc();
    assert!(!est_la(&b, "Raccourcis"));
    presser(&mut b, Key::R);
    b.chauffer(1);
    assert!(b.app.confort.aide_touches, "la touche est tenue");
    for attendu in ["Raccourcis", "Pinceau", "Storyboard", "Partout", "Annuler", "Mode concentration"] {
        assert!(est_la(&b, attendu), "« {attendu} » est sur la carte : {:?}", tous_les_textes(&b));
    }
    // Tant que la touche est tenue, la carte reste (images sans événement).
    b.chauffer(5);
    assert!(b.app.confort.aide_touches && est_la(&b, "Partout"));
    // Relâchée : la carte s'en va dès l'image suivante.
    relacher(&mut b, Key::R);
    assert!(!b.app.confort.aide_touches);
    assert!(!est_la(&b, "Partout") && !est_la(&b, "Mode concentration"), "la carte est partie");
    // Rien n'a changé dans le projet ni dans l'outil.
    assert_eq!(b.app.tool, Tool::Brush);
    assert!(!b.app.modified);
}

#[test]
fn the_card_follows_the_tool_and_the_tab() {
    let mut b = banc();
    b.app.set_tool(Tool::Select);
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    presser(&mut b, Key::R);
    b.chauffer(1);
    assert!(est_la(&b, "Sélection") && est_la(&b, "Tout sélectionner"), "{:?}", tous_les_textes(&b));
    assert!(est_la(&b, "Montage") && est_la(&b, "Début du film"));
    assert!(!est_la(&b, "Plus gros"), "ce n'est pas l'outil courant");
    relacher(&mut b, Key::R);
}

#[test]
fn the_card_reads_a_key_changed_after_the_app_started() {
    let mut b = banc();
    let user = "version = 1\n[[raccourci]]\naction = \"outil.taille_plus\"\ntouches = [\"K\"]\n";
    b.app.keymap = Keymap::build(PresetId::Shot, Some(user));
    presser(&mut b, Key::R);
    b.chauffer(1);
    assert!(est_la(&b, "K"), "{:?}", tous_les_textes(&b));
    relacher(&mut b, Key::R);
}

#[test]
fn the_card_is_not_shown_while_typing_or_on_the_welcome_card() {
    let mut b = banc();
    b.app.welcome = true;
    b.chauffer(3);
    presser(&mut b, Key::R);
    b.chauffer(2);
    assert!(!b.app.confort.aide_touches, "pas sur l'accueil");
    relacher(&mut b, Key::R);
    b.app.welcome = false;
    b.chauffer(3);
    // Une boîte de dialogue tient la main.
    b.app.pending = Some(crate::app::Pending::Close);
    presser(&mut b, Key::R);
    b.chauffer(2);
    assert!(!b.app.confort.aide_touches);
    relacher(&mut b, Key::R);
    b.app.pending = None;
    b.chauffer(2);
}

#[test]
fn ctrl_r_is_not_the_card() {
    let mut b = banc();
    // (Le banc passe les modificateurs tenus par `touches`, comme la vraie fenêtre.)
    b.touches = Modifiers::COMMAND;
    b.image(vec![Event::Key { key: Key::R, physical_key: Some(Key::R), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }], 1.0 / 60.0);
    b.chauffer(1);
    assert!(!b.app.confort.aide_touches, "Ctrl+R garde son sens");
    b.image(vec![Event::Key { key: Key::R, physical_key: Some(Key::R), pressed: false, repeat: false, modifiers: Modifiers::COMMAND }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
}

#[test]
fn the_idle_card_asks_for_no_repaint_beyond_the_ordinary() {
    let mut b = banc();
    crate::perf_tests::se_reposer(&mut b, 200);
    let repos: Vec<std::time::Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "{repos:?}");
}
