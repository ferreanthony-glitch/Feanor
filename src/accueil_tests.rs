//! L'écran d'accueil (`accueil.rs`, `accueilfenetre.rs`) après la demande d'Anthony du 2026-10-29 : une petite fenêtre qui ne
//! montre que l'accueil (bandeau, cadre « Nouveau projet », cadre « Projets récents »), sans menus ni logiciel vide derrière ;
//! le formulaire de création partagé avec la fenêtre de Fichier > Nouveau ; la fenêtre qui change de taille quand un projet
//! s'ouvre ou se ferme. Le tirage de l'image du bandeau est dans `accueilimages_tests.rs`.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2, ViewportCommand};

use crate::accueilfenetre::{demarre_sur_l_accueil, ModeFenetre, TAILLE_ACCUEIL, TAILLE_MIN_TRAVAIL};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

/// L'accueil dans une fenêtre de la taille de l'accueil compact (1000 × 620).
fn accueil() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1000, 620);
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
    tous_les_textes(b).into_iter().find(|(t, _)| t == texte).unwrap_or_else(|| panic!("« {texte} » n'est pas à l'écran : {:?}", tous_les_textes(b).iter().map(|t| t.0.clone()).collect::<Vec<_>>())).1
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|(t, _)| t.contains(texte))
}

fn clic(b: &mut Banc, p: Pos2) {
    // (Le pointeur se pose d'abord : l'infobulle du bouton paraît et se range avant le clic.)
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(3);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

fn double_clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(3);
    for _ in 0..2 {
        b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.chauffer(3);
}

fn touche(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
}

/// Un dossier de projets pour l'essai, vidé d'avance.
fn dossier_d_essai(nom: &str) -> std::path::PathBuf {
    crate::fiabilite_fixture::dossier(nom)
}

/// Écrit `nom` dans le champ du nom du formulaire de l'accueil (le curseur y est dès la première image) et pointe le dossier
/// du projet sur `dossier`.
fn nommer(b: &mut Banc, nom: &str, dossier: &std::path::Path) {
    let form = b.app.accueil_etat.form.as_mut().expect("le formulaire est sur l'accueil");
    form.folder = dossier.to_path_buf();
    // (Un clic ailleurs a pu prendre le curseur : Ctrl+N le remet dans le nom.)
    form.focus_nom = true;
    b.chauffer(2);
    b.image(vec![Event::Text(nom.to_string())], 1.0 / 60.0);
    b.chauffer(2);
}

/// Crée un projet par le formulaire de l'accueil : un nom, « Créer ».
fn creer_un_projet(b: &mut Banc, nom: &str) {
    let dossier = dossier_d_essai(&format!("accueil-{nom}"));
    nommer(b, nom, &dossier);
    let cree = centre(b, "Créer");
    clic(b, cree);
    b.chauffer(4);
    assert!(!b.app.welcome_shown(), "l'accueil est parti : le projet est créé (état : {:?} ; nom {:?})", b.app.status, b.app.accueil_etat.form.as_ref().map(|f| (f.name.clone(), f.peut_creer(true), f.problem())));
}

// ---------------------------------------------------------------------------------------------- la page

#[test]
fn the_welcome_page_has_the_two_frames_the_form_fields_and_the_checkbox_and_no_menu() {
    let b = accueil();
    assert!(b.app.welcome_shown() && b.app.accueil_compact());
    for present in [
        "Nouveau projet", "Nom du projet", "Créer", "Dossier", "Parcourir…", "Titre", "Sous-titre", "Format", "Taille", "Largeur", "Hauteur", "Cadence",
        "Plus d'options", "Projets récents", "Ouvrir un projet…", "Rouvrir le dernier projet au lancement",
    ] {
        assert!(est_la(&b, present), "« {present} » est là : {:?}", tous_les_textes(&b).iter().map(|t| t.0.clone()).collect::<Vec<_>>());
    }
    // Le cadre « Nouveau projet » est à gauche, « Projets récents » à droite ; la case est en bas à gauche.
    let (gauche, droite) = (centre(&b, "Nom du projet"), centre(&b, "Ouvrir un projet…"));
    assert!(gauche.x < droite.x, "le formulaire est à gauche, les récents à droite");
    let case = centre(&b, "Rouvrir le dernier projet au lancement");
    assert!(case.y > gauche.y && case.x < 500.0, "la case est en bas à gauche : {case:?}");
    let ouvrir = centre(&b, "Ouvrir un projet…");
    assert!(ouvrir.y > centre(&b, "Projets récents").y + 100.0, "« Ouvrir un projet… » est en bas du cadre de droite");
    // Pas de menus, de barre d'état, de panneaux, ni des anciennes grandes entrées.
    for absent in ["Fichier", "Édition", "Aide", "Labo", "Commencer sans projet", "Importer un scénario", "Dessiner des vignettes", "Choisis le format, la cadence et le dossier, puis commence."] {
        assert!(!tous_les_textes(&b).iter().any(|(t, _)| t == absent), "« {absent} » ne paraît pas");
    }
    // Rien ne dépasse de la fenêtre compacte.
    for (t, p) in tous_les_textes(&b) {
        assert!(p.x > 0.0 && p.x < 1000.0 && p.y > 0.0 && p.y < 620.0, "« {t} » est dans la fenêtre : {p:?}");
    }
}

#[test]
fn the_form_fields_are_the_ones_of_the_new_project_window_and_nothing_invented() {
    let b = accueil();
    // Ni espace colorimétrique ni champ de vision : SHOT ne les gère pas.
    for absent in ["Colour Space", "Espace colorimétrique", "Champ de vision", "Field of View", "sRGB", "structure de fichier"] {
        assert!(!est_la(&b, absent), "« {absent} » n'existe pas dans SHOT");
    }
    // Les valeurs d'origine du format : HD 1080 à 24 i/s.
    let f = b.app.accueil_etat.form.as_ref().unwrap();
    assert_eq!((f.values.width, f.values.height, f.values.fps), (1920, 1080, 24));
    assert!(est_la(&b, "HD 1080   1920×1080"), "le préréglage reconnu est dit dans la liste");
    assert!(est_la(&b, "24 i/s"));
}

#[test]
fn create_is_enabled_only_once_a_name_is_typed_and_enter_in_the_name_field_creates() {
    let mut b = accueil();
    let f = b.app.accueil_etat.form.as_ref().unwrap();
    assert!(!f.peut_creer(true), "sans nom, « Créer » est grisé");
    assert!(f.peut_creer(false), "la fenêtre de Fichier > Nouveau, elle, accepte un projet sans nom");
    // Sans nom, un clic sur « Créer » ne crée rien.
    let cree = centre(&b, "Créer");
    clic(&mut b, cree);
    assert!(b.app.sans_projet());
    // Un nom : « Créer » s'allume.
    let dossier = dossier_d_essai("accueil-entree");
    nommer(&mut b, "Film", &dossier);
    assert!(b.app.accueil_etat.form.as_ref().unwrap().peut_creer(true));
    // Entrée dans le champ du nom : le projet est créé, un seul.
    touche(&mut b, Key::Enter);
    b.chauffer(4);
    assert!(!b.app.sans_projet() && !b.app.welcome_shown(), "Entrée a créé le projet");
    assert_eq!(b.app.onglets.len(), 1, "un seul projet : {:?}", b.app.infos_onglets());
    assert!(dossier.join("Film").join("Film.shot").exists(), "le projet est enregistré dans son dossier");
}

#[test]
fn a_new_project_takes_its_title_and_subtitle_from_the_form_and_keeps_the_name_as_title_otherwise() {
    let mut b = accueil();
    let dossier = dossier_d_essai("accueil-titres");
    {
        let f = b.app.accueil_etat.form.as_mut().unwrap();
        f.name = "Piste".into();
        f.titre = "Le grand film".into();
        f.sous_titre = "Première version".into();
        f.folder = dossier.clone();
        f.values.width = 1280;
        f.values.height = 720;
        f.values.fps = 25;
    }
    b.chauffer(2);
    let cree = centre(&b, "Créer");
    clic(&mut b, cree);
    b.chauffer(4);
    assert!(!b.app.welcome_shown());
    assert_eq!(b.app.doc.properties.title, "Le grand film");
    assert_eq!(b.app.doc.properties.info, "Première version");
    assert_eq!((b.app.doc.frame, b.app.doc.fps), ([1280, 720], 25), "le format et la cadence du formulaire");
    // Sans titre : le nom.
    b.app.run_action("fichier.fermer");
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    let dossier = dossier_d_essai("accueil-titres-2");
    {
        let f = b.app.accueil_etat.form.as_mut().unwrap();
        assert_eq!((f.values.width, f.values.height, f.values.fps), (1280, 720, 25), "les dernières valeurs sont retenues");
        f.name = "Autre".into();
        f.folder = dossier.clone();
    }
    b.chauffer(2);
    let cree = centre(&b, "Créer");
    clic(&mut b, cree);
    b.chauffer(4);
    assert_eq!(b.app.doc.properties.title, "Autre");
    assert!(b.app.doc.properties.info.is_empty());
}

#[test]
fn the_folder_list_proposes_the_chosen_folder_then_those_of_recent_projects_then_documents() {
    use crate::nouveau::{default_folder, dossiers_proposes, DOSSIERS_PROPOSES};
    use std::path::PathBuf;
    let choisi = PathBuf::from(r"D:\Films");
    let recents = vec![PathBuf::from(r"E:\Anim\Mon film\Mon film.shot"), PathBuf::from(r"d:\films\Autre\Autre.shot"), PathBuf::from(r"E:\Anim\Second\Second.shot")];
    let liste = dossiers_proposes(&choisi, &recents);
    assert_eq!(liste[0], choisi, "le dossier choisi d'abord");
    assert_eq!(liste[1], PathBuf::from(r"E:\Anim"), "puis le dossier d'un projet récent");
    assert!(!liste.iter().skip(1).any(|d| d.to_string_lossy().to_lowercase() == r"d:\films"), "sans doublon, la casse ignorée : {liste:?}");
    assert_eq!(liste.iter().filter(|d| d.starts_with(r"E:\Anim")).count(), 1, "un dossier qui sert à deux projets n'est proposé qu'une fois");
    assert_eq!(liste.last(), Some(&default_folder()), "Documents en dernier");
    // Au plus DOSSIERS_PROPOSES, même avec beaucoup de projets.
    let beaucoup: Vec<PathBuf> = (0..30).map(|i| PathBuf::from(format!(r"F:\d{i}\p\p.shot"))).collect();
    assert_eq!(dossiers_proposes(&choisi, &beaucoup).len(), DOSSIERS_PROPOSES);
}

// ---------------------------------------------------------------------------------------------- les projets récents

/// Deux « projets » récents : un qui existe (un dossier avec son manifeste) et un qui a disparu.
fn deux_recents(b: &mut Banc, nom: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let racine = dossier_d_essai(nom);
    let present = racine.join("Present").join("Present.shot");
    std::fs::create_dir_all(present.parent().unwrap()).unwrap();
    std::fs::write(&present, "").unwrap();
    let absent = racine.join("Perdu").join("Perdu.shot");
    b.app.settings.recent_projects = vec![present.clone(), absent.clone()];
    // (La présence se vérifie sur un fil, au plus toutes les trois secondes : on attend sa réponse.)
    let t = std::time::Instant::now();
    while b.app.recent_presence.exists(&absent) && t.elapsed() < std::time::Duration::from_secs(10) {
        b.image(Vec::new(), 1.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    b.chauffer(3);
    (racine, present, absent)
}

#[test]
fn recent_projects_show_their_name_only_and_a_missing_one_is_greyed_with_introuvable() {
    let mut b = accueil();
    let (racine, present, _) = deux_recents(&mut b, "accueil-recents");
    assert!(est_la(&b, "Present"), "le nom du projet");
    assert!(!tous_les_textes(&b).iter().any(|(t, _)| t.contains(&racine.display().to_string())), "le chemin n'est pas dans la liste (seulement en bulle)");
    assert!(est_la(&b, "Perdu   introuvable"), "le projet qui a disparu est dit introuvable");
    // Grisé : la couleur du texte doux, pas celle du texte fort.
    let couleur = |texte: &str| -> egui::Color32 {
        let mut c = None;
        fn cherche(s: &Shape, texte: &str, c: &mut Option<egui::Color32>) {
            match s {
                Shape::Text(t) if t.galley.job.text == texte => *c = t.galley.job.sections.first().map(|s| s.format.color),
                Shape::Vec(v) => v.iter().for_each(|s| cherche(s, texte, c)),
                _ => {}
            }
        }
        for s in &b.formes {
            cherche(&s.shape, texte, &mut c);
        }
        c.unwrap_or_else(|| panic!("« {texte} » n'est pas peint"))
    };
    assert_eq!(couleur("Perdu   introuvable"), crate::theme::texte_doux());
    assert_eq!(couleur("Present"), crate::theme::texte_fort());
    // Les deux sont dans l'ordre des récents, sous le titre du cadre.
    assert!(centre(&b, "Present").y < centre(&b, "Perdu   introuvable").y);
    assert!(centre(&b, "Present").y > centre(&b, "Projets récents").y);
    let _ = present;
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_double_click_on_a_recent_project_opens_it_and_on_a_missing_one_only_says_so() {
    let mut b = accueil();
    // Un vrai projet enregistré, pour que l'ouverture réussisse.
    let racine = dossier_d_essai("accueil-ouvrir-recent");
    let mut a = crate::perf_tests::banc(Document::with_frame(320, 180, 24), false);
    let manifeste = racine.join("Vrai").join("Vrai.shot");
    a.app.path = Some(manifeste.clone());
    a.app.modified = true;
    assert!(a.app.save());
    let t = std::time::Instant::now();
    while a.app.saving.is_some() && t.elapsed() < std::time::Duration::from_secs(30) {
        a.image(Vec::new(), 0.016);
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    let absent = racine.join("Perdu").join("Perdu.shot");
    b.app.settings.recent_projects = vec![manifeste.clone(), absent.clone()];
    let t = std::time::Instant::now();
    while b.app.recent_presence.exists(&absent) && t.elapsed() < std::time::Duration::from_secs(10) {
        b.image(Vec::new(), 1.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    b.chauffer(3);
    // Le projet disparu : le double-clic le dit, rien ne s'ouvre.
    let p = centre(&b, "Perdu   introuvable");
    double_clic(&mut b, p);
    assert!(b.app.sans_projet(), "rien ne s'ouvre");
    assert!(b.app.status.contains("introuvable"), "{:?}", b.app.status);
    // Le projet présent : il s'ouvre, seul. (Un peu plus tard : le double-clic d'avant est loin.)
    b.image(Vec::new(), 1.0);
    b.chauffer(3);
    let p = centre(&b, "Vrai");
    double_clic(&mut b, p);
    let t = std::time::Instant::now();
    while b.app.path.is_none() && t.elapsed() < std::time::Duration::from_secs(20) {
        b.image(Vec::new(), 0.016);
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    b.chauffer(3);
    assert_eq!(b.app.path.as_deref(), Some(manifeste.as_path()), "{}", b.app.status);
    assert_eq!(b.app.onglets.len(), 1, "un seul projet");
    assert!(!b.app.welcome_shown());
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn enter_on_the_chosen_recent_project_opens_it() {
    let mut b = accueil();
    let (racine, present, _) = deux_recents(&mut b, "accueil-entree-recent");
    let p = centre(&b, "Present");
    clic(&mut b, p);
    assert_eq!(b.app.accueil_etat.selection.as_deref(), Some(present.as_path()), "un clic choisit le projet");
    touche(&mut b, Key::Enter);
    // (Le manifeste d'essai est vide : l'ouverture échoue, mais elle a été demandée, ce que dit l'état.)
    assert!(b.app.open_job.is_some() || !b.app.status.is_empty() && b.app.status != crate::sansprojet::STATUT_DE_DEPART, "Entrée demande l'ouverture : {:?}", b.app.status);
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn a_right_click_offers_to_remove_a_recent_project_from_the_list() {
    let mut b = accueil();
    let (racine, present, absent) = deux_recents(&mut b, "accueil-retirer");
    let p = centre(&b, "Perdu   introuvable");
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(3);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    assert!(est_la(&b, "Retirer de la liste"), "le menu du clic droit : {:?}", tous_les_textes(&b).iter().map(|t| t.0.clone()).collect::<Vec<_>>());
    let r = centre(&b, "Retirer de la liste");
    clic(&mut b, r);
    assert_eq!(b.app.settings.recent_projects, vec![present], "seul le projet cliqué est retiré de la liste");
    assert!(!b.app.settings.recent_projects.contains(&absent));
    // (Le projet n'est pas supprimé du disque : il n'y était déjà plus, et ses voisins sont intacts.)
    assert!(racine.join("Present").join("Present.shot").exists());
    std::fs::remove_dir_all(&racine).ok();
}

#[test]
fn the_reopen_checkbox_is_kept_and_saved() {
    let mut b = accueil();
    let avant = b.app.settings.reopen_last;
    let case = centre(&b, "Rouvrir le dernier projet au lancement");
    clic(&mut b, pos2(case.x - 120.0, case.y));
    assert_ne!(b.app.settings.reopen_last, avant, "un clic sur la case la change");
    assert_eq!(crate::settings::Settings::load().reopen_last, b.app.settings.reopen_last, "elle est enregistrée");
}

// ---------------------------------------------------------------------------------------------- la fenêtre

#[test]
fn without_a_project_the_window_is_the_compact_welcome_and_no_working_size_is_asked() {
    let b = accueil();
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Accueil, "la fenêtre est compacte");
    assert!(b.app.accueil_compact());
    // La fenêtre est revenue à la taille de l'accueil : jamais maximisée.
    assert!(b.commandes.contains(&ViewportCommand::Maximized(false)));
    assert!(!b.commandes.contains(&ViewportCommand::Maximized(true)), "la taille de travail n'est pas demandée sans projet : {:?}", b.commandes);
    assert!(b.commandes.contains(&ViewportCommand::InnerSize(TAILLE_ACCUEIL.into())), "{:?}", b.commandes);
    assert!(b.commandes.contains(&ViewportCommand::Resizable(false)), "taille fixe");
}

#[test]
fn creating_a_project_grows_the_window_to_its_working_size_once() {
    let mut b = accueil();
    let avant = b.commandes.len();
    creer_un_projet(&mut b, "Grandit");
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Travail);
    let apres = &b.commandes[avant..];
    assert!(apres.contains(&ViewportCommand::Maximized(true)), "la fenêtre est maximisée : {apres:?}");
    assert!(apres.contains(&ViewportCommand::Resizable(true)));
    assert!(apres.contains(&ViewportCommand::MinInnerSize(TAILLE_MIN_TRAVAIL.into())));
    assert_eq!(apres.iter().filter(|c| **c == ViewportCommand::Maximized(true)).count(), 1, "une seule fois, pas à chaque image");
    assert_eq!(b.app.onglets.len(), 1, "un seul projet");
    // Quelques images de plus : plus rien n'est demandé à la fenêtre.
    let n = b.commandes.len();
    b.chauffer(10);
    assert_eq!(b.commandes.len(), n, "la fenêtre ne bouge plus : {:?}", &b.commandes[n..]);
}

#[test]
fn closing_the_last_project_shrinks_the_window_back_to_the_welcome_in_two_steps_and_keeps_the_working_place() {
    let mut b = accueil();
    creer_un_projet(&mut b, "Revient");
    b.chauffer(3);
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Travail);
    let avant = b.commandes.len();
    b.app.run_action("fichier.fermer");
    b.chauffer(5);
    assert!(b.app.welcome_shown() && b.app.accueil_compact(), "l'accueil revient");
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Accueil);
    let apres = b.commandes[avant..].to_vec();
    let pos = |c: &ViewportCommand| apres.iter().position(|x| x == c);
    let (maxi, taille, fixe) = (pos(&ViewportCommand::Maximized(false)), pos(&ViewportCommand::InnerSize(TAILLE_ACCUEIL.into())), pos(&ViewportCommand::Resizable(false)));
    assert!(maxi.is_some() && taille.is_some() && fixe.is_some(), "{apres:?}");
    assert!(maxi < taille && taille < fixe, "d'abord la fin de la maximisation, puis la taille, puis la taille fixe : {apres:?}");
    // Un autre projet : la fenêtre de travail revient.
    creer_un_projet(&mut b, "Revient2");
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Travail);
    assert!(b.commandes[avant..].iter().filter(|c| **c == ViewportCommand::Maximized(true)).count() >= 1);
}

#[test]
fn a_window_that_was_not_maximized_gets_its_size_and_place_back() {
    let mut b = accueil();
    creer_un_projet(&mut b, "Libre");
    // La fenêtre de travail, pas maximisée, à une certaine place (ce que le système dit à egui).
    let dehors = egui::Rect::from_min_size(pos2(120.0, 80.0), vec2(1316.0, 839.0));
    let dedans = egui::Rect::from_min_size(pos2(128.0, 119.0), vec2(1300.0, 800.0));
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 620.0))), ..Default::default() };
    let vp = input.viewports.entry(egui::ViewportId::ROOT).or_default();
    vp.maximized = Some(false);
    vp.outer_rect = Some(dehors);
    vp.inner_rect = Some(dedans);
    b.app.run_action("fichier.fermer");
    let _ = b.ctx.run(input, |ctx| b.app.frame(ctx));
    b.chauffer(4);
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Accueil);
    let avant = b.commandes.len();
    creer_un_projet(&mut b, "Libre2");
    let apres = &b.commandes[avant..];
    assert!(apres.contains(&ViewportCommand::InnerSize(vec2(1300.0, 800.0))), "la taille de travail d'avant : {apres:?}");
    assert!(apres.contains(&ViewportCommand::OuterPosition(pos2(120.0, 80.0))), "et sa place : {apres:?}");
    assert!(!apres.contains(&ViewportCommand::Maximized(true)));
}

#[test]
fn a_second_tab_keeps_the_working_window_and_the_welcome_fills_it() {
    let mut b = accueil();
    creer_un_projet(&mut b, "Onglets");
    b.app.run_action("onglet.nouveau");
    b.chauffer(5);
    assert!(b.app.sans_projet(), "l'onglet neuf est sans projet");
    assert_eq!(b.app.onglets.len(), 2);
    assert!(b.app.welcome_shown() && !b.app.accueil_compact(), "l'accueil montre sa page, mais la fenêtre garde sa taille : la bande des onglets est là");
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Travail);
    assert!(est_la(&b, "Nouveau projet") && est_la(&b, "Projets récents"), "la même page dans l'onglet");
}

#[test]
fn the_launch_guess_opens_the_working_size_when_a_project_is_coming_and_the_compact_welcome_otherwise() {
    use crate::settings::Settings;
    let mut s = Settings::default();
    s.reopen_last = true;
    s.session_vide = false;
    s.recent_projects = vec![std::path::PathBuf::from(r"C:\x\a\a.shot")];
    assert!(!demarre_sur_l_accueil(false, &s, false), "un projet à rouvrir : la fenêtre de travail, l'accueil ne se voit pas");
    assert!(!demarre_sur_l_accueil(true, &Settings::default(), false), "un fichier donné : la fenêtre de travail");
    assert!(!demarre_sur_l_accueil(true, &s, true), "un fichier donné, même avec une copie de secours");
    assert!(demarre_sur_l_accueil(false, &s, true), "une copie de secours à proposer : l'accueil, où elle se propose");
    s.reopen_last = false;
    assert!(demarre_sur_l_accueil(false, &s, false), "case décochée : l'accueil");
    s.reopen_last = true;
    s.session_vide = true;
    assert!(demarre_sur_l_accueil(false, &s, false), "la séance d'avant s'est finie sans projet : l'accueil");
    s.session_vide = false;
    s.recent_projects.clear();
    s.session_tabs.clear();
    assert!(demarre_sur_l_accueil(false, &s, false), "rien à rouvrir : l'accueil");
}

#[test]
fn a_launch_that_guessed_a_reopen_but_found_nothing_shrinks_the_window_to_the_welcome() {
    // La fenêtre est née à sa taille de travail (on attendait un projet à rouvrir) ; le projet a disparu.
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1000, 620);
    b.app.welcome = true;
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Travail, "le départ d'une fenêtre de travail");
    b.app.maximized_once = 1;
    b.chauffer(6);
    assert_eq!(b.app.fenetre.mode, ModeFenetre::Accueil, "rien à rouvrir : la fenêtre se met à la taille de l'accueil");
    assert!(b.commandes.contains(&ViewportCommand::InnerSize(TAILLE_ACCUEIL.into())));
    assert!(!b.commandes.contains(&ViewportCommand::Maximized(true)), "la maximisation du lancement n'a pas repris la fenêtre : {:?}", b.commandes);
}

#[test]
fn no_working_window_size_is_remembered_by_eframe_so_the_compact_one_cannot_become_it() {
    // eframe ne retient la taille de la fenêtre que si sa fonction « persistence » est activée : elle ne l'est pas.
    let cargo = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let ligne = cargo.lines().find(|l| l.trim_start().starts_with("eframe")).expect("la ligne d'eframe");
    assert!(!ligne.contains("persistence"), "{ligne}");
    // Et `lib.rs` coupe `persist_window` de toute façon.
    let lib = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("lib.rs")).unwrap();
    assert!(lib.contains("persist_window: false"));
}

#[test]
fn the_performance_offer_does_not_appear_on_the_compact_welcome_but_waits_for_a_project() {
    let mut b = accueil();
    b.app.diag.offre = true;
    b.chauffer(3);
    assert!(!est_la(&b, "Régler SHOT pour cette machine ?"), "pas sur l'accueil compact");
    creer_un_projet(&mut b, "Offre");
    b.chauffer(3);
    assert!(est_la(&b, "Régler SHOT pour cette machine ?"), "elle vient une fois un projet ouvert");
}

#[test]
fn closing_the_compact_welcome_quits_without_asking() {
    let mut b = accueil();
    assert!(!b.fermer_la_fenetre(), "la fenêtre se ferme, sans question");
    assert!(b.app.pending.is_none() && !b.app.modified);
}

// ---------------------------------------------------------------------------------------------- le bandeau

#[test]
fn the_banner_crops_the_image_to_fill_its_area_without_stretching_it() {
    use crate::accueil::recadrage;
    let zone = vec2(1000.0, 168.0);
    for [w, h] in [[1920usize, 1080], [1600, 1200], [1000, 1000], [3000, 1000], [600, 1800], [1000, 168], [16, 9]] {
        let uv = recadrage([w, h], zone);
        assert!(uv.min.x >= 0.0 && uv.min.y >= 0.0 && uv.max.x <= 1.0 + 1e-5 && uv.max.y <= 1.0 + 1e-5, "{w}x{h} : {uv:?} reste dans l'image");
        // Les proportions de la part montrée sont celles de la zone : rien n'est déformé.
        let montre = (uv.width() * w as f32) / (uv.height() * h as f32);
        assert!((montre - zone.x / zone.y).abs() < 0.01, "{w}x{h} : {montre} contre {}", zone.x / zone.y);
        // Elle remplit une dimension entière.
        assert!((uv.width() - 1.0).abs() < 1e-5 || (uv.height() - 1.0).abs() < 1e-5, "{w}x{h} : {uv:?}");
    }
    // Une image plus haute que la zone : le haut est favorisé (les visages), pas le centre pile.
    let uv = recadrage([1000, 1000], zone);
    assert!(uv.min.y < (1.0 - uv.height()) / 2.0);
    // Une image plus large : le centre.
    let uv = recadrage([3000, 1000], zone);
    assert!((uv.min.x - (1.0 - uv.width()) / 2.0).abs() < 1e-5);
}

#[test]
fn the_banner_shows_the_chosen_image_or_else_the_logo_alone() {
    let mut b = accueil();
    // Sans image : le logo seul, aucune erreur.
    let vide = dossier_d_essai("accueil-bandeau-vide");
    b.app.definir_dossier_accueil(Some(vide.clone()));
    let t = std::time::Instant::now();
    while matches!(b.app.confort.accueil.tirage, crate::accueilimages::Tirage::Attente(_) | crate::accueilimages::Tirage::AFaire) && t.elapsed() < std::time::Duration::from_secs(20) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(matches!(b.app.confort.accueil.tirage, crate::accueilimages::Tirage::Logo));
    assert!(b.app.status == crate::sansprojet::STATUT_DE_DEPART, "aucun message : {:?}", b.app.status);
    // Une image : elle est tirée, gardée en texture, et le bandeau la peint (un rectangle texturé de toute la largeur).
    let dir = dossier_d_essai("accueil-bandeau-image");
    image::RgbaImage::from_pixel(640, 360, image::Rgba([180, 60, 120, 255])).save(dir.join("plan.png")).unwrap();
    b.app.definir_dossier_accueil(Some(dir.clone()));
    let t = std::time::Instant::now();
    while !matches!(b.app.confort.accueil.tirage, crate::accueilimages::Tirage::Image(..)) && t.elapsed() < std::time::Duration::from_secs(20) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    b.chauffer(3);
    fn rects_textures(s: &Shape, out: &mut Vec<egui::Rect>) {
        match s {
            Shape::Mesh(m) if m.texture_id != egui::TextureId::default() && !m.vertices.is_empty() => {
                out.push(egui::Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()))
            }
            Shape::Vec(v) => v.iter().for_each(|s| rects_textures(s, out)),
            _ => {}
        }
    }
    let mut images = Vec::new();
    for s in &b.formes {
        rects_textures(&s.shape, &mut images);
    }
    let large = images.iter().find(|r| r.width() >= 999.0).expect("l'image du bandeau couvre toute la largeur");
    assert!((large.height() - crate::accueil::BANDEAU_H).abs() < 0.5 && large.min.y.abs() < 0.5, "en haut de la fenêtre : {large:?}");
    // Le logo est posé dessus, à gauche.
    let logo = images.iter().find(|r| r.width() < 400.0 && r.width() > 100.0).expect("le logo");
    assert!(logo.min.x < 60.0 && logo.min.y >= 0.0 && logo.max.y <= crate::accueil::BANDEAU_H, "{logo:?}");
    std::fs::remove_dir_all(&vide).ok();
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------------------------- les réglages du nouveau projet

#[test]
fn a_new_project_opens_on_the_vignettes_page_by_default() {
    let mut b = accueil();
    assert_eq!(b.app.settings.new_project.ouvre_sur, crate::nouveau::OuvreSur::Vignettes, "d'origine : les Vignettes");
    assert_eq!(b.app.espace, Espace::Storyboard);
    creer_un_projet(&mut b, "Planche");
    assert_eq!(b.app.espace, Espace::Vignettes, "la page Vignettes");
    assert!(b.app.board_shown(), "la planche est à l'écran");
    assert!(b.app.path.is_some(), "un projet nommé est enregistré tout de suite dans son dossier");
}

#[test]
fn with_the_setting_on_storyboard_a_new_project_opens_on_the_storyboard_space() {
    let mut b = accueil();
    b.app.settings.new_project.ouvre_sur = crate::nouveau::OuvreSur::Storyboard;
    // On part de l'espace Vignettes pour voir le changement.
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    creer_un_projet(&mut b, "Montage");
    assert_eq!(b.app.espace, Espace::Storyboard, "l'espace Storyboard, avec sa timeline");
    assert!(!b.app.board_shown(), "pas la planche : le plan et sa timeline");
}

// ---------------------------------------------------------------------------------------------- Fichier > Nouveau avec un projet ouvert

#[test]
fn file_new_with_a_project_open_keeps_the_window_and_shares_the_same_form() {
    let mut b = accueil();
    creer_un_projet(&mut b, "Premier");
    b.app.settings.new_project.ouvre_sur = crate::nouveau::OuvreSur::Storyboard;
    b.app.set_espace(Espace::Animation);
    b.chauffer(3);
    b.app.run_action("fichier.nouveau");
    b.chauffer(4);
    assert!(b.app.new_dialog.is_some(), "avec un projet ouvert, la fenêtre « Nouveau projet » est là");
    assert!(b.app.accueil_etat.form.is_none(), "le formulaire de l'accueil n'existe plus : un projet est ouvert");
    // Les mêmes champs que sur l'accueil, avec le même code.
    for champ in ["Nom du projet", "Créer", "Dossier", "Parcourir…", "Titre", "Sous-titre", "Format", "Taille", "Cadence", "Plus d'options", "Annuler"] {
        assert!(est_la(&b, champ), "« {champ} » est dans la fenêtre : {:?}", tous_les_textes(&b).iter().map(|t| t.0.clone()).collect::<Vec<_>>());
    }
    // Les options sont ouvertes d'office dans la fenêtre (elle a la place) : le papier y est.
    assert!(est_la(&b, "Papier") && est_la(&b, "Premier plan"));
    // Sans nom, « Créer » crée un projet sans nom (comme l'ancien « OK »).
    let dossier = dossier_d_essai("accueil-fichier-nouveau");
    b.app.new_dialog.as_mut().unwrap().folder = dossier;
    let cree = centre(&b, "Créer");
    clic(&mut b, cree);
    b.chauffer(4);
    assert!(b.app.new_dialog.is_none());
    assert_eq!(b.app.espace, Espace::Storyboard);
    assert_eq!(b.app.onglets.len(), 2, "le projet neuf s'ajoute dans un onglet : {:?}", b.app.infos_onglets());
}

#[test]
fn file_new_cancelled_with_escape_leaves_the_project_alone() {
    let mut b = accueil();
    creer_un_projet(&mut b, "Garde");
    b.app.run_action("fichier.nouveau");
    b.chauffer(4);
    assert!(b.app.new_dialog.is_some());
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    assert!(b.app.new_dialog.is_none());
    assert_eq!(b.app.onglets.len(), 1);
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
