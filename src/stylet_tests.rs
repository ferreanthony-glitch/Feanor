//! Le stylet lu auprès de Windows (`stylet.rs`, `styletui.rs` ; Anthony, 2026-11-06) : la lecture et les jalons sans
//! tablette, puis le banc dans l'application entière.

use eframe::egui::{epaint::Shape, pos2, Event, Modifiers, PointerButton, TouchDeviceId, TouchId, TouchPhase};

use crate::app::Tool;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::stylet::{self, clic_du_stylet, jalons_de, Jalon, Lecture, Recu, Releve, A_PORTEE, BOUTON, EN_CONTACT, GOMME, MESURE_APPUI, MESURE_PENCHE_X, MESURE_PENCHE_Y, RETOURNE};

const DT: f64 = 1.0 / 60.0;

fn vus(l: &Lecture) -> Vec<Jalon> {
    let j = jalons_de(l);
    Jalon::TOUS.into_iter().filter(|x| j & x.bit() != 0).collect()
}

// ------------------------------------------------------------------------------------------------ la lecture, sans tablette

#[test]
fn a_reading_decodes_contact_pressure_tilt_and_the_pen_flags() {
    let l = Lecture::decoder(A_PORTEE | EN_CONTACT, BOUTON, MESURE_APPUI | MESURE_PENCHE_X | MESURE_PENCHE_Y, 512, 0, [30, -12]);
    assert!(l.portee && l.contact && l.bouton && !l.retourne && !l.gomme);
    assert!((l.appui - 0.5).abs() < 1e-6);
    assert_eq!((l.penche, l.angle(), l.rotation), ([30, -12], 30, 0));
    // Des valeurs hors des bornes de Windows ne débordent pas.
    let l = Lecture::decoder(A_PORTEE, 0, 0, 5000, 725, [200, -200]);
    assert_eq!((l.appui, l.penche, l.rotation), (1.0, [90, -90], 5));
    // Rien : ni à portée ni en contact.
    assert_eq!(Lecture::decoder(0, 0, 0, 0, 0, [0, 0]), Lecture::default());
}

#[test]
fn each_gesture_of_the_list_is_recognised() {
    let lire = |pointeur, stylet, appui, penche| Lecture::decoder(pointeur, stylet, 0, appui, 0, penche);
    assert_eq!(vus(&lire(A_PORTEE, 0, 0, [0, 0])), [Jalon::Survol]);
    assert_eq!(vus(&lire(A_PORTEE | EN_CONTACT, 0, 300, [0, 0])), [Jalon::Appui]);
    // Tenu presque droit : le bruit de la mesure ne compte pas pour « penché ».
    assert_eq!(vus(&lire(A_PORTEE | EN_CONTACT, 0, 300, [3, -5])), [Jalon::Appui]);
    assert_eq!(vus(&lire(A_PORTEE | EN_CONTACT, 0, 300, [3, -40])), [Jalon::Appui, Jalon::Penche]);
    assert_eq!(vus(&lire(A_PORTEE, BOUTON, 0, [0, 0])), [Jalon::Survol, Jalon::BoutonEnLAir]);
    // Le cas qui décide des gestes « bouton tenu » : le bouton pendant que la pointe touche.
    assert_eq!(vus(&lire(A_PORTEE | EN_CONTACT, BOUTON, 200, [0, 0])), [Jalon::Appui, Jalon::BoutonPose]);
    assert_eq!(vus(&lire(A_PORTEE, RETOURNE, 0, [0, 0])), [Jalon::Survol, Jalon::Retourne]);
    assert_eq!(vus(&lire(A_PORTEE | EN_CONTACT, GOMME, 200, [0, 0])), [Jalon::Appui, Jalon::Retourne, Jalon::GommePosee]);
    assert_eq!(vus(&Lecture::decoder(A_PORTEE, 0, 0, 0, 90, [0, 0])), [Jalon::Survol, Jalon::Rotation]);
    // Chaque jalon a son bit, sa consigne et son nom.
    let mut bits = 0;
    for j in Jalon::TOUS {
        assert_eq!(bits & j.bit(), 0);
        bits |= j.bit();
        assert!(!j.consigne().is_empty() && !j.nom().is_empty());
    }
}

#[test]
fn a_mouse_click_made_by_the_pen_is_told_from_a_real_mouse_and_from_a_finger() {
    assert!(clic_du_stylet(0xFF51_5700));
    assert!(clic_du_stylet(0xFF51_5712));
    assert!(!clic_du_stylet(0xFF51_5780), "le bit 0x80 : un doigt");
    assert!(!clic_du_stylet(0), "une vraie souris");
}

#[test]
fn the_report_says_what_was_seen_and_what_was_not() {
    let r = Releve {
        lecture: Lecture::decoder(A_PORTEE | EN_CONTACT, BOUTON, MESURE_APPUI | MESURE_PENCHE_X, 800, 0, [20, 0]),
        jalons: Jalon::Appui.bit() | Jalon::BoutonPose.bit() | Jalon::Penche.bit(),
        appui_max: 0.78,
        penche_max: 41,
        messages: 312,
        doigts: 0,
        clics: stylet::Clics { gauche: 0, gauche_du_stylet: 0, droit: 2, droit_du_stylet: 2 },
    };
    let recu = Recu { touchers: 300, appui_max: 0.78, principal: 4, secondaire: 2, milieu: 0 };
    let t = r.texte(&recu, Some(true), "06/11/2026 à 15:00");
    for attendu in [
        "Relevé du stylet, FEANOR",
        "06/11/2026 à 15:00",
        "Écoute posée sur la fenêtre : oui",
        "Messages de stylet lus : 312",
        "- bouton, pointe posée : vu",
        "- gomme posée : pas vu",
        "- stylet retourné : pas vu",
        "- appui le plus fort : 78 %",
        "- inclinaison la plus forte : 41°",
        "appui oui, inclinaison oui, rotation non",
        "- droit : 2 (dont 2 venus du stylet)",
        "- événements de stylet : 300 (appui le plus fort : 78 %)",
        "clic droit : 2",
    ] {
        assert!(t.contains(attendu), "« {attendu} » dans le relevé :\n{t}");
    }
    assert!(r.texte(&recu, Some(false), "").contains("NON (Windows a refusé)"));
    assert!(r.texte(&recu, None, "").contains("pas encore demandée"));
}

// ------------------------------------------------------------------------------------------------ le banc, dans l'application

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

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    b
}

/// Le seul essai qui touche au relevé commun de l'écoute (des atomiques du processus) : les autres essais tournent en
/// même temps, et aucun ne doit le lire ni l'écrire.
#[test]
fn the_bench_opens_from_help_shows_what_windows_says_and_writes_its_report() {
    let mut b = banc();
    assert!(!b.app.stylet.fenetre);
    assert_eq!(b.app.stylet.ecoute, None, "rien n'est posé sur la fenêtre tant que le banc n'a pas été ouvert");
    assert!(crate::keymap::action_info(crate::styletui::ACTION).is_some_and(|a| a.implemented));
    b.app.run_action(crate::styletui::ACTION);
    assert!(b.app.stylet.fenetre);
    // Dans les essais, il n'y a pas de fenêtre de Windows : l'écoute est refusée, et le banc le dit.
    assert_eq!(b.app.stylet.ecoute, Some(false));
    stylet::effacer();
    b.chauffer(4);
    let t = textes(&b);
    for attendu in ["Ce que le stylet transmet", "sans rien changer au dessin", "Windows n'a pas laissé FEANOR écouter", "À essayer", "Ce que FEANOR reçoit aujourd'hui", "Enregistrer le relevé", "Recommencer", "Stylet : hors de portée", "Bouton du stylet : relâché"] {
        assert!(t.iter().any(|x| x.contains(attendu)), "« {attendu} » dans le banc : {t:?}");
    }
    for j in Jalon::TOUS {
        assert!(t.iter().any(|x| x == j.consigne()), "la consigne « {} » : {t:?}", j.consigne());
    }
    // Ce que l'écoute aurait lu : la pointe posée à mi-appui, couchée, le bouton enfoncé.
    stylet::noter(A_PORTEE | EN_CONTACT, BOUTON, MESURE_APPUI | MESURE_PENCHE_X | MESURE_PENCHE_Y, 512, 0, [35, -10]);
    // Et ce que FEANOR reçoit aujourd'hui du même geste : un toucher avec son appui, puis un clic droit.
    let p = pos2(1200.0, 700.0);
    b.image(vec![Event::Touch { device_id: TouchDeviceId(0), id: TouchId(0), phase: TouchPhase::Start, pos: p, force: Some(0.5) }, Event::PointerMoved(p), Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(2);
    assert_eq!(b.app.stylet.recu, Recu { touchers: 1, appui_max: 0.5, principal: 0, secondaire: 1, milieu: 0 });
    let t = textes(&b);
    for attendu in ["Stylet : pointe posée", "Appui : 50 %", "Bouton du stylet : enfoncé", "Inclinaison gauche-droite : 35°", "Inclinaison avant-arrière : -10°", "Stylet : 1 événements, appui jusqu'à 50 %", "clic droit : 1"] {
        assert!(t.iter().any(|x| x.contains(attendu)), "« {attendu} » dans le banc : {t:?}");
    }
    assert!(t.iter().any(|x| x.starts_with(Jalon::Appui.consigne()) && x.ends_with("(jusqu'à 50 %)")), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with(Jalon::Penche.consigne()) && x.ends_with("(jusqu'à 35°)")), "{t:?}");
    let r = stylet::lire();
    assert!(r.vu(Jalon::BoutonPose) && r.vu(Jalon::Appui) && r.vu(Jalon::Penche) && !r.vu(Jalon::Retourne) && !r.vu(Jalon::Survol));
    assert_eq!(r.messages, 1);
    // Le stylet s'en va : ni à portée ni en contact, mais ce qui a été vu reste coché.
    stylet::noter_le_depart();
    let r = stylet::lire();
    assert!(!r.lecture.portee && !r.lecture.contact && r.lecture.appui == 0.0 && r.vu(Jalon::BoutonPose));
    // Les clics de souris arrivés à la fenêtre.
    stylet::noter_un_clic(true, true);
    stylet::noter_un_clic(false, false);
    stylet::noter_un_doigt();
    let r = stylet::lire();
    assert_eq!((r.clics, r.doigts), (stylet::Clics { gauche: 1, gauche_du_stylet: 0, droit: 1, droit_du_stylet: 1 }, 1));
    // Le relevé : dans le fichier du poste (ici, le bac à sable de l'essai) et dans le presse-papiers.
    b.app.stylet_enregistrer();
    let fichier = crate::styletui::fichier().expect("un dossier de données");
    let ecrit = std::fs::read_to_string(&fichier).expect("le relevé écrit");
    for attendu in ["- bouton, pointe posée : vu", "- stylet retourné : pas vu", "- droit : 1 (dont 1 venus du stylet)", "NON (Windows a refusé)"] {
        assert!(ecrit.contains(attendu), "« {attendu} » dans le fichier :\n{ecrit}");
    }
    b.chauffer(2);
    assert!(textes(&b).iter().any(|x| x.contains("Relevé enregistré")));
    // Recommencer : tout est décoché.
    stylet::effacer();
    assert_eq!(stylet::lire(), Releve::default());
    // La même commande referme le banc ; l'écoute n'est pas redemandée.
    b.app.run_action(crate::styletui::ACTION);
    assert!(!b.app.stylet.fenetre);
    b.chauffer(2);
    assert!(!textes(&b).iter().any(|x| x.contains("À essayer")));
}

#[test]
fn the_bench_is_in_the_help_menu_and_works_without_a_project() {
    assert!(crate::sansprojet::action_sans_projet(crate::styletui::ACTION));
}

/// Capture pour voir le banc (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc stylet_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn stylet_captures() {
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut b = crate::perf_tests::banc(Document::new(400, 300, 24), true);
    b.app.tool = Tool::Brush;
    b.chauffer(8);
    b.app.run_action(crate::styletui::ACTION);
    // Comme si l'écoute était posée et avait vu la pointe posée, couchée, le bouton enfoncé.
    b.app.stylet.ecoute = Some(true);
    stylet::effacer();
    stylet::noter(A_PORTEE, 0, MESURE_APPUI | MESURE_PENCHE_X | MESURE_PENCHE_Y, 0, 0, [0, 0]);
    stylet::noter(A_PORTEE | EN_CONTACT, BOUTON, MESURE_APPUI | MESURE_PENCHE_X | MESURE_PENCHE_Y, 640, 0, [38, -22]);
    b.app.stylet.recu = Recu { touchers: 214, appui_max: 0.62, principal: 3, secondaire: 0, milieu: 0 };
    b.chauffer(8);
    let Some(rgba) = b.capturer() else { return println!("STYLET pas de carte graphique : capture ignorée") };
    image::save_buffer(dir.join("stylet-1-banc.png"), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    println!("STYLET stylet-1-banc.png");
    stylet::effacer();
}
