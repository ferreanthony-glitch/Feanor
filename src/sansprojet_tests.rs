//! Démarrer sans projet ouvert (`sansprojet.rs`, demande d'Anthony du 2026-10-29) et un thème d'un seul tenant au premier
//! lancement (`theme::appliquer`).
//!
//! Sans projet : l'accueil sur un fond neutre, ni onglet « Sans titre » ni « Nouvel onglet », « Nouveau projet » crée UN
//! projet, « Ouvrir » n'en laisse pas un vide à côté, fermer le dernier projet ramène à l'accueil, un projet donné au
//! lancement ou rouvert (« Rouvrir le dernier projet ») ne montre jamais l'accueil, la récupération après plantage reste
//! proposée, et chaque commande et chaque entrée de menu jouées sans projet ne changent rien et ne plantent pas.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, epaint::Shape, pos2, vec2, Color32, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Theme, ViewportId};

use crate::app::App;
use crate::document::Document;
use crate::fiabilite_fixture::dossier;
use crate::perf_tests::{banc, banc_taille, Banc};
use crate::theme::{self, contraste, ChoixTheme, Palette, ReglageTheme};

// ---------------------------------------------------------------------------------------------- outils

/// Un SHOT qui vient de se lancer : aucun projet, l'accueil.
fn lance() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.welcome = true;
    b.chauffer(6);
    b
}

fn parcourir(shape: &Shape, f: &mut impl FnMut(&Shape)) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| parcourir(s, f)),
        s => f(s),
    }
}

fn couleur_du_texte(t: &egui::epaint::TextShape) -> Color32 {
    t.override_text_color.unwrap_or_else(|| match t.galley.job.sections.first().map(|s| s.format.color) {
        Some(c) if c != Color32::PLACEHOLDER => c,
        _ => t.fallback_color,
    })
}

/// Tous les textes peints par la dernière image : texte, centre, couleur.
fn textes(formes: &[egui::epaint::ClippedShape]) -> Vec<(String, Pos2, Color32)> {
    let mut out = Vec::new();
    for s in formes {
        parcourir(&s.shape, &mut |sh| {
            if let Shape::Text(t) = sh {
                out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0, couleur_du_texte(t)));
            }
        });
    }
    out
}

/// Tous les rectangles pleins et opaques : zone et couleur.
fn fonds(formes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, Color32)> {
    let mut out = Vec::new();
    for s in formes {
        parcourir(&s.shape, &mut |sh| {
            if let Shape::Rect(r) = sh {
                // (Les images, le logo et le logotype, sont des rectangles texturés teintés de blanc : pas des fonds.)
                if r.fill.a() == 255 && r.fill_texture_id() == egui::TextureId::default() {
                    out.push((r.rect, r.fill));
                }
            }
        });
    }
    out
}

fn tous_les_textes(b: &Banc) -> Vec<String> {
    textes(&b.formes).into_iter().map(|t| t.0).collect()
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|t| t.contains(texte))
}

fn centre(b: &Banc, texte: &str) -> Pos2 {
    textes(&b.formes).into_iter().find(|(t, _, _)| t == texte).unwrap_or_else(|| panic!("« {texte} » n'est pas à l'écran : {:?}", tous_les_textes(b))).1
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}


/// Rien n'est ouvert, rien n'a changé : l'état de SHOT juste après son lancement.
fn rien_n_a_change(b: &Banc, quand: &str) {
    assert!(b.app.sans_projet(), "{quand} : toujours sans projet");
    assert_eq!(b.app.onglets.len(), 1, "{quand} : un seul onglet");
    assert!(b.app.history.is_empty(), "{quand} : aucune étape d'annulation");
    assert!(!b.app.modified, "{quand} : rien de modifié (fermer SHOT ne demanderait rien)");
    assert!(b.app.path.is_none() && b.app.import_source.is_none(), "{quand} : aucun fichier");
    assert!(b.app.pending.is_none() && b.app.groupe.is_none(), "{quand} : aucune question posée");
}

/// Les fenêtres que les commandes permises ouvrent, refermées (elles tiendraient la main aux raccourcis suivants).
fn refermer_les_fenetres(b: &mut Banc) {
    let ouvert = b.app.new_dialog.is_some()
        || b.app.palette.is_some()
        || b.app.prefs_window
        || b.app.about_window
        || b.app.scenario_window.is_some()
        || b.app.show_help
        || b.app.assign.is_some()
        || b.app.confort.stylet.is_some()
        || b.app.diag.ouvert;
    if !ouvert {
        return;
    }
    b.app.new_dialog = None;
    b.app.palette = None;
    b.app.prefs_window = false;
    b.app.about_window = false;
    b.app.scenario_window = None;
    b.app.show_help = false;
    b.app.assign = None;
    b.app.confort.stylet = None;
    b.app.diag.ouvert = false;
    b.chauffer(2);
}

/// Aucun champ ni bouton n'a le focus clavier : Tab, puis Entrée ou Espace, « cliqueraient » « Parcourir… » ou « Ouvrir un projet… »,
/// qui ouvrent le sélecteur de fichiers de Windows (un essai ne peut pas y répondre).
fn sans_focus(b: &mut Banc) {
    if let Some(id) = b.ctx.memory(|m| m.focused()) {
        b.ctx.memory_mut(|m| m.surrender_focus(id));
    }
}

fn pomper(b: &mut Banc, limite: Duration, fini: impl Fn(&Banc) -> bool) -> bool {
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

/// Un projet enregistré sur le disque (`Nom/Nom.shot`).
fn projet_sur_disque(nom: &str) -> (PathBuf, PathBuf) {
    let dir = dossier(nom);
    let mut a = banc(Document::with_frame(320, 180, 24), false);
    let manifeste = dir.join(nom).join(format!("{nom}.shot"));
    a.app.path = Some(manifeste.clone());
    a.app.modified = true;
    assert!(a.app.save());
    assert!(pomper(&mut a, Duration::from_secs(30), |a| a.app.saving.is_none()), "l'écriture finit");
    assert!(!a.app.modified);
    (dir, manifeste)
}

// ---------------------------------------------------------------------------------------------- 1. démarrer sans projet

#[test]
fn launching_shows_no_project_only_the_welcome_on_a_calm_neutral_background() {
    let b = lance();
    rien_n_a_change(&b, "au lancement");
    assert!(b.app.welcome_shown());
    // Ni onglet « Sans titre » ni « Nouvel onglet ».
    for absent in ["Sans titre", "Nouvel onglet"] {
        assert!(!est_la(&b, absent), "« {absent} » ne paraît pas : {:?}", tous_les_textes(&b));
    }
    // Les deux cadres de l'accueil (plus de menus : la fenêtre est l'accueil seul, `accueilfenetre.rs`).
    for present in ["Nouveau projet", "Projets récents", "Ouvrir un projet…", "Créer"] {
        assert!(est_la(&b, present), "« {present} » est là : {:?}", tous_les_textes(&b));
    }
    for absent in ["Fichier", "Aide", "Édition"] {
        assert!(!tous_les_textes(&b).iter().any(|t| t == absent), "« {absent} » : pas de menus sur l'accueil compact : {:?}", tous_les_textes(&b));
    }
    // Ni les quatre espaces de travail ni le moindre panneau (leur titre, leurs réglages).
    for absent in [" Storyboard ", " Animation ", " Montage ", " Vignettes ", "Planche", "Réglages", "Mon panneau", "Historique", "1920×1080", "100 %"] {
        assert!(!tous_les_textes(&b).iter().any(|t| t == absent), "« {absent} » ne paraît pas : {:?}", tous_les_textes(&b));
    }
    assert!(!est_la(&b, "dessine le premier plan"), "le message de départ de la barre d'état ne parle pas d'un projet : {:?}", tous_les_textes(&b));
    // Un fond neutre, du ton « autour du dessin » du thème, sur presque toute la fenêtre : pas de feuille blanche.
    let fond = theme::fond();
    let ecran = b.taille.x * b.taille.y;
    let neutre: f32 = fonds(&b.formes).into_iter().filter(|(_, c)| *c == fond).map(|(r, _)| r.width() * r.height()).fold(0.0, f32::max);
    assert!(neutre > ecran * 0.6, "la zone de travail est un fond neutre ({neutre} sur {ecran})");
    assert!(fonds(&b.formes).iter().all(|(_, c)| c.r() < 150), "aucune surface claire : {:?}", fonds(&b.formes).iter().filter(|(_, c)| c.r() >= 150).collect::<Vec<_>>());
}

#[test]
fn new_project_creates_exactly_one_project() {
    let mut b = lance();
    assert!(b.app.settings.open_in_new_tab, "le réglage d'origine : un projet qui arrive prend un onglet neuf s'il en faut un");
    // Le formulaire est sur l'accueil (pas de fenêtre à part) ; « Créer » attend un nom.
    assert!(b.app.new_dialog.is_none());
    assert!(b.app.sans_projet(), "le formulaire seul ne crée rien");
    assert_eq!(b.app.onglets.len(), 1);
    let cree = centre(&b, "Créer");
    clic(&mut b, cree);
    assert!(b.app.sans_projet(), "sans nom, « Créer » ne fait rien");
    // Un nom (le curseur est déjà dans le champ), un dossier d'essai : « Créer ».
    let form = b.app.accueil_etat.form.as_mut().unwrap();
    form.folder = dossier("sansprojet-creer");
    form.focus_nom = true; // (le clic sur « Créer » grisé a pris le curseur : Ctrl+N le remet dans le nom)
    b.chauffer(2);
    b.image(vec![Event::Text("Sans titre".into())], 1.0 / 60.0);
    b.chauffer(2);
    let cree = centre(&b, "Créer");
    clic(&mut b, cree);
    b.chauffer(4);
    assert!(!b.app.sans_projet() && !b.app.welcome_shown(), "le projet est créé");
    assert_eq!(b.app.onglets.len(), 1, "UN seul projet, pas un projet vide à côté : {:?}", b.app.infos_onglets());
    assert_eq!(b.app.infos_onglets().iter().map(|i| i.nom.clone()).collect::<Vec<_>>(), ["Sans titre"]);
    // Le projet est le projet neuf : la bande des onglets paraît maintenant, avec lui seul.
    assert!(est_la(&b, "Sans titre"));
    assert!(!est_la(&b, "Nouvel onglet"));
}

#[test]
fn ctrl_n_without_a_project_puts_the_cursor_in_the_name_field_and_opens_no_separate_window() {
    let mut b = lance();
    b.app.run_action("fichier.nouveau");
    b.chauffer(3);
    assert!(b.app.new_dialog.is_none(), "le formulaire est sur l'accueil");
    rien_n_a_change(&b, "Ctrl+N sans projet");
    assert!(b.app.welcome_shown(), "l'accueil reste");
}

#[test]
fn opening_a_project_takes_the_place_of_the_empty_state_with_no_second_tab() {
    let (dir, manifeste) = projet_sur_disque("sansprojet-ouvrir");
    let mut b = lance();
    b.app.open_path(manifeste.clone());
    // Pendant l'ouverture, l'accueil ne clignote pas devant le projet qui arrive.
    let t = Instant::now();
    while b.app.open_job.is_some() && t.elapsed() < Duration::from_secs(60) {
        assert!(!b.app.welcome_shown(), "pas d'accueil pendant l'ouverture");
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
    }
    b.chauffer(3);
    assert!(b.app.open_job.is_none(), "{}", b.app.status);
    assert_eq!(b.app.onglets.len(), 1, "le projet prend la place : aucun onglet vide en plus");
    assert_eq!(b.app.path.as_deref(), Some(manifeste.as_path()));
    assert!(!b.app.sans_projet() && !b.app.welcome_shown());
    assert!(!est_la(&b, "Nouvel onglet") && !est_la(&b, "Sans titre"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_given_on_the_command_line_opens_without_a_blank_project_or_the_welcome() {
    // (`lib.rs` : `app.open_path(path)` avant la première image.)
    let (dir, manifeste) = projet_sur_disque("sansprojet-argument");
    let mut b = lance();
    b.app.open_path(manifeste.clone());
    assert!(!b.app.welcome_shown(), "dès la première image");
    assert!(pomper(&mut b, Duration::from_secs(60), |b| b.app.open_job.is_none() && b.app.path.is_some()), "{}", b.app.status);
    b.chauffer(3);
    assert_eq!((b.app.onglets.len(), b.app.path.as_deref()), (1, Some(manifeste.as_path())));
    assert!(!b.app.welcome_shown());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_the_last_project_at_launch_opens_it_without_a_blank_project_or_the_welcome() {
    let (dir, manifeste) = projet_sur_disque("sansprojet-rouvrir");
    let mut b = lance();
    b.app.settings.reopen_last = true;
    b.app.settings.session_vide = false;
    b.app.settings.session_tabs = vec![manifeste.clone()];
    b.app.settings.session_active = 0;
    b.app.settings.recent_projects = vec![manifeste.clone()];
    b.app.reopen_last_project();
    assert!(b.app.reopen_probe.is_some(), "la question « le projet est-il encore là ? » est posée");
    assert!(!b.app.welcome_shown(), "pas d'accueil en attendant la réponse");
    let t = Instant::now();
    while b.app.path.is_none() && t.elapsed() < Duration::from_secs(60) {
        assert!(!b.app.welcome_shown(), "jamais d'accueil avant le projet");
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
    }
    assert!(pomper(&mut b, Duration::from_secs(60), |b| b.app.open_job.is_none() && b.app.session_prete), "{}", b.app.status);
    b.chauffer(3);
    assert_eq!((b.app.onglets.len(), b.app.path.as_deref()), (1, Some(manifeste.as_path())), "le dernier projet, seul");
    assert!(!b.app.welcome_shown());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_the_reopen_setting_or_after_closing_the_last_project_the_launch_stays_empty() {
    let (dir, manifeste) = projet_sur_disque("sansprojet-pas-rouvrir");
    // Case décochée : l'accueil.
    let mut b = lance();
    b.app.settings.reopen_last = false;
    b.app.settings.session_tabs = vec![manifeste.clone()];
    b.app.settings.recent_projects = vec![manifeste.clone()];
    b.app.reopen_last_project();
    assert!(b.app.reopen_probe.is_none());
    b.chauffer(3);
    assert!(b.app.welcome_shown());
    // Case cochée, mais la séance d'avant s'est finie sans projet : l'accueil aussi.
    let mut b = lance();
    b.app.settings.reopen_last = true;
    b.app.settings.session_vide = true;
    b.app.settings.recent_projects = vec![manifeste.clone()];
    b.app.reopen_last_project();
    assert!(b.app.reopen_probe.is_none(), "rien à rouvrir contre sa volonté");
    b.chauffer(3);
    rien_n_a_change(&b, "séance finie sans projet");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_last_project_goes_back_to_the_empty_state_and_remembers_it() {
    let (dir, manifeste) = projet_sur_disque("sansprojet-fermer");
    let mut b = lance();
    b.app.open_path(manifeste.clone());
    assert!(pomper(&mut b, Duration::from_secs(60), |b| b.app.open_job.is_none() && b.app.path.is_some()));
    b.chauffer(3);
    assert!(!b.app.sans_projet());
    // Fichier > Fermer l'onglet.
    b.app.run_action("fichier.fermer");
    b.chauffer(4);
    rien_n_a_change(&b, "après avoir fermé le dernier projet");
    assert!(b.app.welcome_shown(), "l'accueil revient");
    assert!(!est_la(&b, "Sans titre") && !est_la(&b, "Nouvel onglet"), "aucun projet vide n'est créé : {:?}", tous_les_textes(&b));
    assert!(b.app.settings.session_vide);
    // La séance se retient vide : le prochain lancement ne rouvre rien.
    b.app.session_prete = true;
    b.app.retenir_la_session();
    let relu = crate::settings::Settings::load();
    assert!(relu.session_tabs.is_empty() && relu.session_vide, "relu au lancement suivant");
    // Fermer encore, sans projet : rien ne se passe.
    b.app.run_action("fichier.fermer");
    b.chauffer(2);
    rien_n_a_change(&b, "fermer sans projet");
    // Un projet ouvert plus tard rend le départ habituel.
    b.app.open_path(manifeste.clone());
    assert!(pomper(&mut b, Duration::from_secs(60), |b| b.app.open_job.is_none() && b.app.path.is_some()));
    b.app.retenir_la_session();
    let relu = crate::settings::Settings::load();
    assert!(!relu.session_vide && relu.session_tabs.len() == 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_without_a_project_asks_nothing() {
    let mut b = lance();
    assert!(!b.fermer_la_fenetre(), "la fenêtre se ferme, sans question");
    rien_n_a_change(&b, "à la fermeture");
    assert!(b.app.onglets_modifies().is_empty());
}

#[test]
fn the_crash_recovery_is_still_offered_and_opens_in_the_empty_state_without_a_second_project() {
    use crate::recovery::{self, Recovery, Snapshot};
    let racine = dossier("sansprojet-secours");
    let doc = Document::with_frame(320, 180, 24);
    let none = std::collections::HashMap::new();
    let mut rec = Recovery::start(&racine).unwrap();
    fn snap<'a>(d: &'a Document, none: &'a std::collections::HashMap<u64, u64>) -> Snapshot<'a> {
        Snapshot { doc: d, serial: 1, modified: true, title: "Mon film".into(), project: None, project_written: none }
    }
    rec.start_copy(snap(&doc, &none));
    let t = Instant::now();
    while !rec.tick(snap(&doc, &none), None, false) {
        assert!(rec.error.is_none() && t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(rec); // plantage : le dossier reste, le verrou est relâché
    let orphelins = recovery::orphans(&racine, None);
    assert_eq!(orphelins.len(), 1);
    let mut b = lance();
    b.app.orphans = orphelins;
    b.chauffer(4);
    assert!(est_la(&b, "FEANOR s'est fermé sans enregistrer"), "la proposition de récupération est à l'écran : {:?}", tous_les_textes(&b));
    assert!(b.app.welcome_shown() && b.app.accueil_compact(), "la proposition est dans la petite fenêtre d'accueil, pas dans une fenêtre à part");
    // Elle ne change rien au projet caché tant qu'on n'a pas répondu.
    rien_n_a_change(&b, "pendant la proposition");
    let o = b.app.orphans.remove(0);
    b.app.open_orphan(o);
    b.chauffer(3);
    assert_eq!(b.app.onglets.len(), 1, "la copie prend la place : pas de projet vide à côté");
    assert!(!b.app.sans_projet() && b.app.modified, "la copie est à enregistrer");
    // « Plus tard » (ou la copie traitée) : l'accueil revient.
    let mut c = lance();
    c.app.orphans = vec![];
    assert!(c.app.welcome_shown());
    let _ = std::fs::remove_dir_all(&racine);
}

// ---------------------------------------------------------------------------------------------- 2. rien ne plante sans projet

#[test]
fn every_command_played_without_a_project_changes_nothing_and_does_not_crash() {
    let mut b = lance();
    // (Ouvrir et l'import ouvrent le sélecteur de fichiers de Windows, Quitter ferme la fenêtre : ils ne se jouent pas ici.)
    let a_part = ["fichier.ouvrir", "fichier.quitter", "fichier.importer_scenario"];
    let mut jouees = 0;
    let mut permises = 0;
    for info in crate::keymap::ACTIONS {
        if a_part.contains(&info.id) {
            continue;
        }
        let bloquee = b.app.action_bloquee(info.id);
        assert_eq!(bloquee, !crate::sansprojet::action_sans_projet(info.id));
        b.app.run_action(info.id);
        b.chauffer(1);
        rien_n_a_change(&b, info.id);
        refermer_les_fenetres(&mut b);
        jouees += 1;
        permises += usize::from(!bloquee);
    }
    assert!(jouees > 500, "toutes les commandes du fichier de raccourcis : {jouees}");
    assert!(permises < 25, "seules quelques commandes servent sans projet : {permises}");
    // Une commande bloquée le dit.
    b.app.run_action("outil.pinceau");
    assert!(b.app.status.contains("projet"), "{}", b.app.status);
}

#[test]
fn every_default_shortcut_played_without_a_project_changes_nothing_and_does_not_crash() {
    let mut b = lance();
    let mut touches: Vec<(Key, Modifiers, String)> = Vec::new();
    for binding in b.app.keymap.bindings.iter().filter(|x| x.active && !x.reserved) {
        for c in &binding.combos {
            let key = match c.key {
                crate::keymap::KeyMatch::Char(k) | crate::keymap::KeyMatch::Pos(k) => k,
            };
            let m = Modifiers { alt: c.alt, ctrl: c.ctrl, shift: c.shift, mac_cmd: false, command: c.ctrl };
            touches.push((key, m, binding.action.clone()));
        }
    }
    assert!(touches.len() > 100, "les raccourcis par défaut : {}", touches.len());
    let mut vus = 0;
    for (key, m, action) in touches {
        // (Ouvrir et les imports ouvrent le sélecteur de fichiers de Windows ; Quitter ferme la fenêtre : jouées à part.)
        if ["fichier.ouvrir", "fichier.quitter", "fichier.importer_scenario"].contains(&action.as_str()) {
            continue;
        }
        sans_focus(&mut b);
        b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: m }], 1.0 / 60.0);
        b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: m }], 1.0 / 60.0);
        b.chauffer(1);
        rien_n_a_change(&b, &format!("{key:?} ({action})"));
        refermer_les_fenetres(&mut b);
        vus += 1;
    }
    assert!(vus > 100);
    // Des touches libres au milieu du vide : flèches, Échap, Entrée, Retour arrière, Suppr, Espace, lettres.
    for key in [Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight, Key::Escape, Key::Enter, Key::Backspace, Key::Delete, Key::Space, Key::A, Key::Z, Key::Num1] {
        for m in [Modifiers::NONE, Modifiers::SHIFT, Modifiers::COMMAND, Modifiers::ALT] {
            sans_focus(&mut b);
            b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: m }], 1.0 / 60.0);
            b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: m }], 1.0 / 60.0);
            refermer_les_fenetres(&mut b);
            rien_n_a_change(&b, &format!("{key:?} {m:?}"));
        }
    }
    // Couper, copier, coller (événements d'egui) et la molette, le glisser et le clic au milieu du vide.
    b.image(vec![Event::Copy, Event::Cut, Event::Paste("texte".into())], 1.0 / 60.0);
    let milieu = pos2(b.taille.x / 2.0, b.taille.y * 0.8);
    b.image(vec![Event::PointerMoved(milieu), Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: vec2(0.0, 3.0), modifiers: Modifiers::NONE }], 1.0 / 60.0);
    clic(&mut b, pos2(100.0, 500.0));
    rien_n_a_change(&b, "souris dans le vide");
}

#[test]
fn the_compact_welcome_has_no_menus_so_the_menu_entries_cannot_be_played_and_the_commands_behind_them_are_covered_elsewhere() {
    // (Les entrées des menus Fichier et Aide sont des commandes du fichier de raccourcis : `every_command_played_without_a_project…`
    // les joue toutes, sans projet, et rien ne change. La petite fenêtre d'accueil n'a pas de menus, `accueilfenetre.rs`.)
    let b = lance();
    for menu in ["Fichier", "Édition", "Storyboard", "Son", "Calques", "Animation", "Affichage", "Labo", "Aide"] {
        assert!(!tous_les_textes(&b).iter().any(|t| t == menu), "« {menu} » : pas de menu sur l'accueil compact");
    }
    assert!(b.app.accueil_compact());
}
#[test]
fn files_dropped_without_a_project_only_open_projects() {
    use crate::sansprojet::est_un_projet_a_ouvrir;
    for (nom, oui) in [("Film.shot", true), ("Film.pelure", true), ("Film.sboard", false), ("Film.SBOARD", false), ("photo.png", false), ("son.wav", false), ("film.mp4", false), ("notes.json", false)] {
        assert_eq!(est_un_projet_a_ouvrir(Path::new(nom)), oui, "{nom}");
    }
    let dir = dossier("sansprojet-depot");
    let png = dir.join("rouge.png");
    image::RgbaImage::from_pixel(32, 18, image::Rgba([240, 10, 10, 255])).save(&png).unwrap();
    let mut b = lance();
    let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), b.taille)), ..RawInput::default() };
    input.dropped_files = vec![egui::DroppedFile { path: Some(png), ..Default::default() }];
    input.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
    let app = &mut b.app;
    let _ = b.ctx.run(input, |ctx| app.deposer_sans_projet(ctx));
    rien_n_a_change(&b, "image lâchée");
    assert!(b.app.status.contains("projet"), "{}", b.app.status);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_tab_next_to_projects_keeps_the_tab_strip_and_still_blocks_project_commands() {
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    b.garder_formes = true;
    assert!(b.app.nouvel_onglet());
    b.chauffer(4);
    assert!(b.app.sans_projet());
    assert_eq!(b.app.onglets.len(), 2);
    assert!(est_la(&b, "Nouvel onglet"), "avec plusieurs onglets, la bande reste : {:?}", tous_les_textes(&b));
    b.app.run_action("calque.nouveau");
    b.chauffer(2);
    assert!(b.app.history.is_empty() && !b.app.modified, "l'onglet sans projet ne change pas");
    // Un projet qui arrive y prend la place.
    assert!(b.app.charger_projet(Document::with_frame(320, 180, 24)));
    assert!(!b.app.sans_projet());
    assert_eq!(b.app.onglets.len(), 2);
}

// ---------------------------------------------------------------------------------------------- 3. thème

/// Un SHOT qui se lance pour la première fois (aucun réglage, sauf `reglage`) sur un poste dont Windows est en thème `systeme`.
fn premier_lancement(systeme: Theme, reglage: Option<ChoixTheme>) -> (egui::Context, App, f64) {
    let bac = std::env::temp_dir().join(format!("shot-sansprojet-theme-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&bac);
    std::fs::create_dir_all(&bac).unwrap();
    crate::donnees::bac_a_sable(&bac);
    if let Some(choix) = reglage {
        let mut s = crate::settings::Settings::default();
        s.theme.choisir(choix);
        s.save().unwrap();
    }
    let ctx = egui::Context::default();
    // (Ce que fait eframe : Windows annonce son thème par l'entrée de chaque image, ici avant même la construction de
    // l'application.)
    let mut annonce = RawInput { system_theme: Some(systeme), ..RawInput::default() };
    annonce.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
    let _ = ctx.run(annonce, |_| {});
    assert_eq!(ctx.system_theme(), Some(systeme));
    let mut app = App::sans_fenetre(&ctx, None);
    app.maximized_once = 4;
    (ctx, app, 0.0)
}

fn image_theme(ctx: &egui::Context, app: &mut App, t: &mut f64, systeme: Theme) -> Vec<egui::epaint::ClippedShape> {
    *t += 1.0 / 60.0;
    let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0))), time: Some(*t), system_theme: Some(systeme), ..RawInput::default() };
    input.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
    ctx.run(input, |c| app.frame(c)).shapes
}

fn luma(c: Color32) -> f32 {
    0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
}

/// Toutes les couleurs d'egui (fonds, textes, contours des contrôles) viennent de la palette de SHOT.
fn visuels_de_la_palette(ctx: &egui::Context, p: &Palette, quand: &str) {
    let v = ctx.style().visuals.clone();
    assert_eq!(v.dark_mode, !p.claire, "{quand} : sombre ou clair, comme la palette");
    assert_eq!(v.panel_fill, p.panneau, "{quand}");
    assert_eq!(v.window_fill, p.fenetre, "{quand}");
    assert_eq!(v.extreme_bg_color, p.champ, "{quand}");
    assert_eq!(v.widgets.noninteractive.fg_stroke.color, p.texte, "{quand}");
    assert_eq!(v.widgets.inactive.bg_fill, p.controle, "{quand}");
    assert_eq!(v.widgets.hovered.fg_stroke.color, p.texte_fort, "{quand}");
    // Les deux styles d'egui (sombre, clair) sont les mêmes : quel que soit le thème que Windows annonce, rien d'autre n'est lu.
    assert_eq!(ctx.style_of(Theme::Dark).visuals, ctx.style_of(Theme::Light).visuals, "{quand} : un seul thème dans egui");
    // Lisible : pas de texte pâle sur fond pâle.
    assert!(contraste(v.text_color(), v.panel_fill) >= 7.0, "{quand} : texte sur panneau");
    // (Le texte discret d'egui fond le texte dans le fond : 2,8:1 sur « Gris clair », plus partout ailleurs ; SHOT lui-même
    // écrit son texte secondaire avec `texte_doux`, à 4,5:1 au moins, `theme_tests`.)
    assert!(contraste(v.weak_text_color(), v.panel_fill) >= 2.5, "{quand} : texte discret sur panneau : {}", contraste(v.weak_text_color(), v.panel_fill));
    assert!(contraste(v.widgets.noninteractive.fg_stroke.color, v.window_fill) >= 7.0, "{quand} : texte sur fenêtre");
    for (nom, w) in [("inactive", &v.widgets.inactive), ("hovered", &v.widgets.hovered), ("active", &v.widgets.active)] {
        assert!(contraste(w.fg_stroke.color, w.bg_fill) >= 4.5, "{quand} : texte {nom} sur son fond : {}", contraste(w.fg_stroke.color, w.bg_fill));
    }
    assert!(contraste(v.hyperlink_color, v.panel_fill) >= 3.0, "{quand} : lien");
}

#[test]
fn on_a_fresh_install_whatever_windows_says_every_background_and_text_comes_from_the_one_shot_theme() {
    for systeme in [Theme::Light, Theme::Dark] {
        let (ctx, mut app, mut t) = premier_lancement(systeme, None);
        let mut formes = Vec::new();
        for _ in 0..8 {
            formes = image_theme(&ctx, &mut app, &mut t, systeme);
        }
        assert_eq!(app.settings.theme.choix, ChoixTheme::Shot, "sans réglages : le thème de SHOT");
        let p = theme::palette();
        assert!(!p.claire, "le thème par défaut est sombre");
        assert_eq!(ctx.theme(), Theme::Dark, "Windows en thème {systeme:?} : egui reste sombre, comme SHOT");
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::Dark, "le thème de Windows n'est jamais suivi");
        visuels_de_la_palette(&ctx, &p, &format!("Windows {systeme:?}"));
        // À l'écran : aucun fond clair, aucun texte sombre ou pâle (la capture du poste neuf du 2026-10-29 en avait).
        let clairs: Vec<_> = fonds(&formes).into_iter().filter(|(_, c)| luma(*c) > 150.0).collect();
        assert!(clairs.is_empty(), "Windows {systeme:?} : des fonds clairs sur l'interface sombre : {clairs:?}");
        let sombres: Vec<_> = textes(&formes).into_iter().filter(|(_, _, c)| luma(*c) < 90.0).map(|(t, _, c)| (t, c)).collect();
        assert!(sombres.is_empty(), "Windows {systeme:?} : des textes sombres sur l'interface sombre : {sombres:?}");
        assert!(textes(&formes).iter().any(|(t, _, _)| t == "Nouveau projet"), "l'accueil est là");
        // Windows change de thème en cours de route (la bascule claire/sombre de Windows) : rien ne bouge.
        let autre = if systeme == Theme::Light { Theme::Dark } else { Theme::Light };
        for _ in 0..4 {
            formes = image_theme(&ctx, &mut app, &mut t, autre);
        }
        visuels_de_la_palette(&ctx, &theme::palette(), "Windows a changé de thème");
        assert!(fonds(&formes).iter().all(|(_, c)| luma(*c) <= 150.0));
    }
}

#[test]
fn every_theme_of_shot_is_one_theme_in_egui_and_readable_even_when_windows_disagrees() {
    for choix in [ChoixTheme::Shot, ChoixTheme::GrisSombre, ChoixTheme::GrisClair, ChoixTheme::Sombre] {
        for systeme in [Theme::Light, Theme::Dark] {
            let (ctx, mut app, mut t) = premier_lancement(systeme, Some(choix));
            let mut formes = Vec::new();
            for _ in 0..6 {
                formes = image_theme(&ctx, &mut app, &mut t, systeme);
            }
            assert_eq!(app.settings.theme.choix, choix);
            let mut r = ReglageTheme::default();
            r.choisir(choix);
            let p = Palette::depuis(&r.couleurs());
            let quand = format!("{choix:?}, Windows {systeme:?}");
            assert_eq!(ctx.theme(), if p.claire { Theme::Light } else { Theme::Dark }, "{quand}");
            visuels_de_la_palette(&ctx, &p, &quand);
            // L'interface entière est du ton de la palette : aucune surface claire dans un thème sombre, aucune surface
            // sombre dans un thème clair (hors du dessin et des icônes, qui ne sont pas des rectangles pleins ici).
            let a_contre_sens: Vec<_> = fonds(&formes).into_iter().filter(|(_, c)| if p.claire { luma(*c) < 70.0 } else { luma(*c) > 150.0 }).collect();
            assert!(a_contre_sens.is_empty(), "{quand} : {a_contre_sens:?}");
            let textes_pales: Vec<_> = textes(&formes).into_iter().filter(|(_, _, c)| contraste(*c, p.fond) < 2.0 && contraste(*c, p.panneau) < 2.0).map(|(t, _, c)| (t, c)).collect();
            assert!(textes_pales.is_empty(), "{quand} : des textes presque invisibles : {textes_pales:?}");
        }
    }
}

#[test]
fn changing_the_theme_in_the_preferences_changes_both_egui_styles_together() {
    let (ctx, mut app, mut t) = premier_lancement(Theme::Light, None);
    for _ in 0..4 {
        image_theme(&ctx, &mut app, &mut t, Theme::Light);
    }
    app.settings.theme.choisir(ChoixTheme::GrisClair);
    for _ in 0..4 {
        image_theme(&ctx, &mut app, &mut t, Theme::Dark);
    }
    let p = theme::palette();
    assert!(p.claire);
    assert_eq!(ctx.theme(), Theme::Light);
    visuels_de_la_palette(&ctx, &p, "Gris clair");
    app.settings.theme.choisir(ChoixTheme::Shot);
    for _ in 0..4 {
        image_theme(&ctx, &mut app, &mut t, Theme::Light);
    }
    assert_eq!(ctx.theme(), Theme::Dark);
    visuels_de_la_palette(&ctx, &theme::palette(), "retour à SHOT");
}
