//! L'écran d'accueil (`accueil.rs`) après la demande d'Anthony du 2026-10-28 : deux grandes entrées (Nouveau projet,
//! Ouvrir un projet), les projets récents dessous, plus d'« Importer un scénario » (il reste dans le
//! menu Fichier), et le dépôt d'images qui marche depuis l'onglet Vignettes où mène un nouveau projet (par défaut). Le tirage de l'image
//! du haut est dans `accueilimages_tests.rs`.

use eframe::egui::{epaint::Shape, pos2, Event, Modifiers, PointerButton, Pos2};

use crate::accueil::ENTREES;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

fn accueil() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.welcome = true;
    b.chauffer(6);
    b
}

fn textes(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

/// Tous les textes peints par la dernière image, avec leur centre.
fn tous_les_textes(b: &Banc) -> Vec<(String, Pos2)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut out);
    }
    out
}

fn centre(b: &Banc, texte: &str) -> Pos2 {
    tous_les_textes(b).into_iter().find(|(t, _)| t == texte).unwrap_or_else(|| panic!("« {texte} » n'est pas à l'écran")).1
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|(t, _)| t.contains(texte))
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

#[test]
fn the_welcome_card_shows_two_big_entries_in_order_and_nothing_else_big() {
    let b = accueil();
    assert!(b.app.welcome_shown());
    let y: Vec<f32> = ENTREES.iter().map(|(_, titre, _, _)| centre(&b, titre).y).collect();
    assert!(y.windows(2).all(|w| w[0] < w[1]), "les deux entrées se suivent de haut en bas : {y:?}");
    assert_eq!(ENTREES.map(|e| e.1), ["Nouveau projet", "Ouvrir un projet"]);
    // Chaque entrée dit en une ligne ce qu'elle fait, sous son titre.
    for (_, titre, aide, _) in ENTREES {
        let ligne = centre(&b, aide);
        assert!(ligne.y > centre(&b, titre).y, "{titre} : l'explication est dessous");
    }
    // Les anciennes entrées qui encombraient la carte n'y sont plus.
    let tout = || tous_les_textes(&b).iter().map(|t| t.0.clone()).collect::<Vec<_>>();
    for ancien in ["Commencer sans projet", ".sboard", "Importer un scénario", "Dessiner des vignettes", "Dessiner un storyboard", "Nouveau fichier", "Nouveau projet…"] {
        assert!(!est_la(&b, ancien), "« {ancien} » n'est plus sur l'accueil : {:?}", tout());
    }
    // La case « Rouvrir le dernier projet » reste.
    assert!(est_la(&b, "Rouvrir le dernier projet au lancement"));
}

#[test]
fn the_recent_projects_stay_under_the_two_entries() {
    let mut b = accueil();
    let dossier = std::env::temp_dir().join(format!("shot-accueil-recent-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    b.app.settings.recent_projects = vec![dossier.clone()];
    b.chauffer(6);
    assert!(est_la(&b, "Projets récents"));
    let (recents, derniere) = (centre(&b, "Projets récents").y, centre(&b, ENTREES[1].1).y);
    assert!(recents > derniere, "les récents sont sous les entrées : {recents} contre {derniere}");
    std::fs::remove_dir_all(&dossier).ok();
}

#[test]
fn the_entries_run_real_commands() {
    for (_, titre, _, action) in ENTREES {
        assert!(crate::keymap::action_info(action).is_some_and(|a| a.implemented), "{titre} : {action} existe");
    }
    assert_eq!(ENTREES[0].3, "fichier.nouveau", "la fenêtre de nouveau projet, avec le choix du format");
    assert_eq!(ENTREES[1].3, "fichier.ouvrir");
    // L'import de scénario existe toujours : dans le menu Fichier > Importer.
    assert!(crate::keymap::action_info("fichier.importer_scenario").is_some_and(|a| a.implemented));
}

#[test]
fn new_project_opens_the_new_project_window_and_the_welcome_steps_aside_and_comes_back_with_the_same_image() {
    let mut b = accueil();
    assert!(b.app.new_dialog.is_none());
    let p = centre(&b, "Nouveau projet");
    clic(&mut b, p);
    assert!(b.app.new_dialog.is_some(), "la fenêtre de nouveau projet (avec le choix du format) est ouverte");
    assert!(!b.app.welcome_shown(), "la fenêtre est ce qu'on regarde");
    // Fermée sans rien créer : l'accueil revient, avec la même image (le tirage n'est pas refait).
    let tirages = b.app.confort.accueil.tirages;
    b.app.new_dialog = None;
    b.app.welcome = true; // (Annuler la fenêtre rend l'accueil : `nouveauui.rs`, `from_welcome`.)
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    assert_eq!(b.app.confort.accueil.tirages, tirages, "pas de nouveau tirage pour une fenêtre passagère");
}

/// Clique « Nouveau projet » sur l'accueil, puis « OK » dans la fenêtre : un projet vide, sans nom.
fn creer_un_projet(b: &mut Banc) {
    let p = centre(b, "Nouveau projet");
    clic(b, p);
    assert!(b.app.new_dialog.is_some());
    b.chauffer(3);
    let ok = centre(b, "OK");
    clic(b, ok);
    b.chauffer(4);
    assert!(b.app.new_dialog.is_none(), "la fenêtre s'est fermée : le projet est créé");
    assert!(!b.app.welcome_shown(), "l'accueil est parti");
}

#[test]
fn a_new_project_opens_on_the_vignettes_page_by_default() {
    let mut b = accueil();
    assert_eq!(b.app.settings.new_project.ouvre_sur, crate::nouveau::OuvreSur::Vignettes, "d'origine : les Vignettes");
    assert_eq!(b.app.espace, Espace::Storyboard);
    creer_un_projet(&mut b);
    assert_eq!(b.app.espace, Espace::Vignettes, "la page Vignettes");
    assert!(b.app.board_shown(), "la planche est à l'écran");
    assert!(b.app.path.is_none(), "un projet sans fichier : « Enregistrer » demandera où le ranger");
}

#[test]
fn with_the_setting_on_storyboard_a_new_project_opens_on_the_storyboard_space() {
    let mut b = accueil();
    b.app.settings.new_project.ouvre_sur = crate::nouveau::OuvreSur::Storyboard;
    // On part de l'espace Vignettes pour voir le changement.
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    creer_un_projet(&mut b);
    assert_eq!(b.app.espace, Espace::Storyboard, "l'espace Storyboard, avec sa timeline");
    assert!(!b.app.board_shown(), "pas la planche : le plan et sa timeline");
}

#[test]
fn the_same_setting_applies_to_file_new_project_from_a_project_tab() {
    let mut b = accueil();
    b.app.settings.new_project.ouvre_sur = crate::nouveau::OuvreSur::Storyboard;
    b.app.welcome = false;
    b.app.set_espace(Espace::Animation);
    b.chauffer(3);
    // Fichier > Nouveau projet… (la même fenêtre) : « OK ».
    b.app.run_action("fichier.nouveau");
    b.chauffer(4);
    assert!(b.app.new_dialog.is_some());
    let ok = centre(&b, "OK");
    clic(&mut b, ok);
    b.chauffer(4);
    assert!(b.app.new_dialog.is_none());
    assert_eq!(b.app.espace, Espace::Storyboard);
}

#[test]
fn the_new_project_page_setting_is_saved_and_read_back_and_is_in_the_preferences_table() {
    use crate::nouveau::OuvreSur;
    let mut b = accueil();
    b.app.settings.new_project.ouvre_sur = OuvreSur::Storyboard;
    b.app.settings.save().unwrap();
    assert_eq!(crate::settings::Settings::load().new_project.ouvre_sur, OuvreSur::Storyboard, "relu au lancement suivant");
    b.app.settings.new_project.ouvre_sur = OuvreSur::Vignettes;
    b.app.settings.save().unwrap();
    assert_eq!(crate::settings::Settings::load().new_project.ouvre_sur, OuvreSur::Vignettes);
    // Un ancien fichier de réglages : les Vignettes.
    let ancien: crate::settings::Settings = toml::from_str("").unwrap();
    assert_eq!(ancien.new_project.ouvre_sur, OuvreSur::Vignettes);
    // Dans la table de recherche des Préférences, onglet Général.
    for requete in ["nouveau projet s'ouvre", "storyboard", "vignettes"] {
        let r = crate::prefsonglets::chercher(requete);
        assert!(r.iter().any(|t| t.onglet == crate::prefsonglets::Onglet::General && t.libelles.contains(&"Un nouveau projet s'ouvre sur")), "« {requete} »");
    }
}
#[test]
fn images_dropped_on_the_board_work_right_after_the_welcome_entry() {
    use crate::vignettes_tests::vignette;
    let mut b = accueil();
    b.app.welcome = false;
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(8);
    assert!(b.app.board_shown());
    let dir = std::env::temp_dir().join(format!("shot-accueil-depot-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("rouge.png");
    image::RgbaImage::from_pixel(160, 90, image::Rgba([240, 10, 10, 255])).save(&png).unwrap();
    let au_centre = vignette(&b, 0).center();
    assert!(b.app.board_take_drop(std::slice::from_ref(&png), Some(au_centre)), "le dépôt est pris par la planche");
    let debut = std::time::Instant::now();
    while !b.app.vignettes.depots.is_empty() && debut.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.chauffer(3);
    assert!(b.app.doc.panels[0].layers.iter().any(|l| l.name == crate::deposer::NOM_DU_CALQUE), "le calque « Image » est là");
    assert!(b.app.modified, "le projet vide est maintenant modifié");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_welcome_steps_aside_while_the_scenario_window_is_open_and_comes_back() {
    let mut b = accueil();
    let dir = std::env::temp_dir().join(format!("shot-accueil-scenario-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fichier = dir.join("court.fountain");
    std::fs::write(&fichier, "Title: Essai\n\nINT. CUISINE - JOUR\n\nMarie ouvre la fenêtre.\n").unwrap();
    b.app.read_scenario(fichier);
    b.chauffer(4);
    assert!(b.app.scenario_window.is_some());
    assert!(!b.app.welcome_shown(), "la fenêtre d'import est ce qu'on regarde");
    // Fermée sans rien importer : l'accueil revient.
    b.app.scenario_window = None;
    b.chauffer(3);
    assert!(b.app.welcome_shown());
    assert!(est_la(&b, "Ouvrir un projet") && !est_la(&b, "Importer un scénario"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_logo_shrinks_with_the_window_so_the_card_fits_at_150_percent() {
    use crate::accueil::logo_largeur;
    assert_eq!(logo_largeur(1200.0), 400.0, "un grand écran : la taille d'origine");
    assert_eq!(logo_largeur(900.0), 400.0);
    assert_eq!(logo_largeur(600.0), 300.0, "une fenêtre de 900 pixels à 150 % : 600 points de haut");
    assert_eq!(logo_largeur(300.0), 240.0, "jamais plus petit que 240");
    assert!(logo_largeur(647.0) % 10.0 == 0.0, "par dizaines de points : la texture ne se refait pas à chaque pixel");
    // La carte entière tient dans la fenêtre à toutes les tailles : le logo, deux entrées, le reste.
    for e in [90.0_f32, 100.0, 125.0, 150.0] {
        let hauteur = 900.0 / (e / 100.0);
        let carte = logo_largeur(hauteur) * 0.54 + 2.0 * (crate::accueil::ENTREE_H + 4.0) + 140.0;
        assert!(carte < hauteur - 60.0, "{e} % : la carte ({carte}) tient sous les menus dans {hauteur} points");
    }
}

#[test]
fn an_image_of_any_proportions_fits_in_the_place_of_the_logo_with_its_proportions() {
    use crate::accueil::place_de_l_image;
    use crate::logo::{VUE_H, VUE_L};
    let place = eframe::egui::vec2(400.0, 400.0 * VUE_H / VUE_L);
    for [w, h] in [[1920, 1080], [1600, 1200], [1000, 1000], [3000, 1000], [600, 1800], [16, 9], [1, 1]] {
        let t = place_de_l_image([w, h], place);
        assert!(t.x <= place.x + 0.01 && t.y <= place.y + 0.01, "{w}x{h} : {t:?} sort de la place {place:?}");
        assert!((t.x / t.y - w as f32 / h as f32).abs() < 0.01, "{w}x{h} : les proportions sont gardées ({t:?})");
        assert!((t.x - place.x).abs() < 0.01 || (t.y - place.y).abs() < 0.01, "{w}x{h} : elle prend toute la place possible");
    }
}

#[test]
fn the_cursor_of_a_dropped_file_is_in_window_points_at_every_interface_size() {
    use crate::deposer::curseur_en_points;
    // Le coin du dessin de la fenêtre est à 100 px de l'écran : en points, 100 / ppp.
    for ppp in [0.9_f32, 1.0, 1.25, 1.5, 2.0] {
        let coin = eframe::egui::vec2(100.0 / ppp, 40.0 / ppp);
        let p = curseur_en_points((900, 500), coin, ppp);
        let attendu = pos2(800.0 / ppp, 460.0 / ppp);
        assert!((p - attendu).length() < 1e-3, "ppp {ppp} : {p:?} au lieu de {attendu:?}");
    }
}
