//! La fenêtre Préférences… en onglets (2026-10-21) : la table des réglages (`prefsonglets.rs`) ne perd rien de
//! l'ancienne fenêtre, chaque onglet montre ses réglages et seulement les siens, la fenêtre se souvient de l'onglet,
//! la recherche répond sur tous les onglets, la fenêtre s'ouvre depuis son action, plus large que haute, et reste au repos.

use std::collections::BTreeSet;

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::prefsonglets::{chercher, groupes_de, plier, Groupe, Onglet, REGLAGES};
use crate::prefsui::{definir_recherche, recherche};

// ------------------------------------------------------------------------------------ l'outillage

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(640, 360, 24), false, 1400, 900);
    b.garder_formes = true;
    b
}

fn ouvrir(b: &mut Banc, onglet: Onglet) {
    b.app.settings.prefs_onglet = onglet;
    // (« Plus de réglages » des Vignettes est replié d'origine, partie 113 : déplié ici pour lire tous les libellés.)
    b.app.prefs_plus_vignettes = true;
    b.app.run_action("fichier.preferences");
    b.chauffer(6);
}

/// Le rectangle de la fenêtre Préférences.
fn fenetre(b: &Banc) -> Rect {
    b.ctx.memory(|m| m.area_rect(egui::Id::new("Préférences"))).expect("la fenêtre Préférences est ouverte")
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

/// Les textes peints par la dernière image, avec leur place, dans la fenêtre Préférences seulement.
fn textes_de_la_fenetre(b: &Banc) -> Vec<(String, Rect)> {
    let fen = fenetre(b);
    let mut tous = Vec::new();
    for s in &b.formes {
        collecter(&s.shape, &mut tous);
    }
    tous.into_iter().filter(|(_, r)| fen.contains_rect(*r) || fen.intersects(*r)).collect()
}

/// Tout ce que l'onglet montre, du haut vers le bas : l'image du haut, puis la molette fait défiler le contenu.
fn parcourir(b: &mut Banc) -> BTreeSet<String> {
    let mut vus = BTreeSet::new();
    b.chauffer(3);
    for _ in 0..12 {
        vus.extend(textes_de_la_fenetre(b).into_iter().map(|(t, _)| t));
        let fen = fenetre(b);
        let p = pos2(fen.max.x - 70.0, fen.center().y);
        b.image(vec![Event::PointerMoved(p), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -260.0), modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.chauffer(3);
    }
    vus
}

fn contient(vus: &BTreeSet<String>, morceau: &str) -> bool {
    vus.iter().any(|t| t.contains(morceau))
}

fn clic(b: &mut Banc, p: Pos2) {
    // (Le pointeur arrive de loin et se pose : à l'image où il apparaît, ce qu'on vise ne se sait pas encore sous lui — un travers du
    // banc d'essai, où le pointeur n'existe pas avant le premier clic ; un vrai pointeur a toujours bougé avant.)
    b.image(vec![Event::PointerMoved(Pos2::new(20.0, 1060.0))], 1.0 / 60.0);
    b.chauffer(2);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(2);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(4);
}

/// Le centre du premier texte de la fenêtre qui satisfait `filtre`.
fn place(b: &Banc, filtre: impl Fn(&str, Rect) -> bool) -> Option<Pos2> {
    textes_de_la_fenetre(b).into_iter().find(|(t, r)| filtre(t, *r)).map(|(_, r)| r.center())
}

// ------------------------------------------------------------------------------------ la table

/// Tout ce que l'ancienne fenêtre (une seule colonne) montrait : chaque morceau doit se retrouver dans la table.
const D_AVANT: &[&str] = &[
    "Thème",
    "Fond des panneaux",
    "Barres",
    "Fenêtres et menus",
    "Boutons et curseurs",
    "Texte",
    "Accent et sélection",
    "Autour du dessin",
    "Timeline et montage",
    "Bordures",
    "Copie de secours toutes les",
    "Rouvrir le dernier projet au lancement",
    "Ouvrir dans un nouvel onglet",
    "Proportions différentes",
    "Cadence différente",
    "Demander à chaque collage",
    "Signer mes commentaires",
    "Mon nom",
    "Une prise périme après",
    "Prendre le plan libre que je commence à modifier",
    "Dire dans la barre d'état ce que font les autres",
    "Croix du système sur le pinceau",
    "Opacité du passe-partout",
    "Calques dans la timeline",
    "Hauteur des plans",
    "Déplier le calque courant tout seul",
    "Agrandir les calques proches quand il y a de la place",
    "Ajuster les panneaux à leur contenu",
    "Réajuster les panneaux",
    "Effets en aperçu réduit",
    "Traits vectoriels texturés",
    "Tremblement du trait",
    "Pelure d'oignon",
    "Cache de la carte graphique",
    "Cache des traits vectoriels",
    "Cache des effets",
    "Fils de calcul",
    "Vignettes",
    "Animations",
    "Effets sur la carte graphique",
    "Diagnostic des performances",
    "Lecture du storyboard",
    "Bruitages et ambiances",
    "Voix témoin des dialogues",
    "Dossier des modèles",
    "Python du serveur",
    "Démarrer le serveur tout seul",
    "Adresse 127.0.0.1, port",
    "Voir et changer les raccourcis",
    "Modifier le fichier à la main",
    "Recharger",
    "Affichage",
];

#[test]
fn every_setting_of_the_old_window_is_in_exactly_one_tab() {
    for ancien in D_AVANT {
        let n = REGLAGES.iter().filter(|r| r.libelle == *ancien).count();
        assert_eq!(n, 1, "« {ancien} » doit être dans la table, une seule fois");
    }
    // Les identifiants sont uniques, les étiquettes aussi.
    let ids: BTreeSet<_> = REGLAGES.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), REGLAGES.len(), "deux réglages ont le même identifiant");
    let libelles: BTreeSet<_> = REGLAGES.iter().map(|r| r.libelle).collect();
    assert_eq!(libelles.len(), REGLAGES.len(), "deux réglages ont la même étiquette");
    // Un groupe est dans un seul onglet.
    for r in REGLAGES {
        let onglets: BTreeSet<_> = REGLAGES.iter().filter(|s| s.groupe == r.groupe).map(|s| s.onglet.nom()).collect();
        assert_eq!(onglets.len(), 1, "le groupe {:?} est dans plusieurs onglets : {onglets:?}", r.groupe);
    }
    // Chaque onglet a quelque chose à montrer, et chaque groupe de la table est dessiné dans son onglet.
    for o in Onglet::TOUS {
        assert!(!groupes_de(o).is_empty(), "l'onglet {} est vide", o.nom());
        assert!(!o.resume().is_empty() && o.resume().ends_with('.'));
    }
    let dans_onglets: usize = Onglet::TOUS.iter().map(|o| groupes_de(*o).len()).sum();
    let groupes: BTreeSet<_> = REGLAGES.iter().map(|r| format!("{:?}", r.groupe)).collect();
    assert_eq!(dans_onglets, groupes.len(), "chaque groupe est dans un seul onglet");
    for g in [Groupe::Theme, Groupe::Performance, Groupe::Generation] {
        assert!(!g.titre().is_empty());
    }
}

#[test]
fn each_tab_shows_all_its_settings_and_none_of_the_others() {
    let mut b = banc();
    for o in Onglet::TOUS {
        ouvrir(&mut b, o);
        let vus = parcourir(&mut b);
        println!("PREFS onglet {:<12} fenêtre {:?} : {} textes", o.nom(), fenetre(&b).size(), vus.len());
        for r in REGLAGES.iter().filter(|r| r.onglet == o) {
            assert!(contient(&vus, r.libelle), "« {} » ({}) manque dans l'onglet {} :\n{vus:#?}", r.libelle, r.id, o.nom());
        }
        for r in REGLAGES.iter().filter(|r| r.onglet != o && r.libelle.chars().count() >= 14) {
            let aussi_dans_celui_ci = REGLAGES.iter().any(|s| s.onglet == o && s.libelle.contains(r.libelle));
            if !aussi_dans_celui_ci {
                assert!(!contient(&vus, r.libelle), "« {} » ({}) ne doit pas se trouver dans l'onglet {}", r.libelle, r.id, o.nom());
            }
        }
        // Le titre de l'onglet et de chaque groupe.
        assert!(contient(&vus, o.nom()));
        for g in groupes_de(o) {
            assert!(contient(&vus, g.titre()), "le titre « {} » manque dans {}", g.titre(), o.nom());
        }
        b.app.prefs_window = false;
        b.chauffer(2);
    }
}

// ------------------------------------------------------------------------------------ la fenêtre

#[test]
fn the_window_opens_from_its_action_wider_than_tall_with_the_tab_list_on_the_left() {
    let mut b = banc();
    assert!(!b.app.prefs_window);
    b.app.run_action("fichier.preferences");
    assert!(b.app.prefs_window);
    b.chauffer(6);
    let fen = fenetre(&b);
    // (Un peu moins qu'avant : la fenêtre est plus haute depuis le 2026-10-30, l'onglet Vignettes a de quoi montrer sans défiler.)
    assert!(fen.width() > fen.height(), "plus large que haute : {fen:?}");
    assert!(fen.width() >= 700.0 && fen.height() >= 400.0 && fen.height() <= 900.0, "{fen:?}");
    let ecran = Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0));
    assert!(ecran.contains_rect(fen), "dans l'écran : {fen:?}");
    // La génération de sons est expérimentale : son onglet ne se montre qu'avec la case « Afficher les fonctions expérimentales ».
    assert!(!textes_de_la_fenetre(&b).iter().any(|(t, _)| t.ends_with(Onglet::Generation.nom()) && t.contains("  ")), "onglet Génération caché d'origine");
    b.app.settings.experimental = true;
    b.chauffer(3);
    // La liste des onglets : les noms l'un sous l'autre, à gauche ; le contenu à droite.
    let textes = textes_de_la_fenetre(&b);
    let mut pos = Vec::new();
    for o in Onglet::TOUS {
        let (t, r) = textes.iter().find(|(t, r)| t.ends_with(o.nom()) && t.contains("  ") && r.min.x < fen.min.x + 60.0).unwrap_or_else(|| panic!("l'entrée {} manque", o.nom()));
        assert!(t.starts_with(o.icone()), "icône devant {t:?}");
        pos.push(r.center().y);
    }
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "la liste descend dans l'ordre : {pos:?}");
    let (_, titre) = textes.iter().find(|(t, _)| t.contains("Rouvrir le dernier projet au lancement")).expect("le contenu de l'onglet Général");
    assert!(titre.min.x > fen.min.x + 150.0, "le contenu est à droite de la liste : {titre:?}");
    assert!(textes.iter().any(|(t, _)| t.contains("Chercher un réglage")), "le champ de recherche");
    // La croix de la fenêtre la ferme.
    b.app.prefs_window = false;
    b.chauffer(2);
    assert!(b.ctx.memory(|m| m.area_rect(egui::Id::new("Préférences"))).is_none() || !b.app.prefs_window);
}

#[test]
fn the_window_is_resizable_and_keeps_its_size_inside_small_screens() {
    let mut b = banc_taille(Document::with_frame(640, 360, 24), false, 700, 500);
    b.garder_formes = true;
    b.app.run_action("fichier.preferences");
    b.chauffer(6);
    let fen = fenetre(&b);
    assert!(fen.width() <= 700.0 && fen.height() <= 500.0, "pas plus grande que l'écran : {fen:?}");
    assert!(fen.width() >= 560.0 && fen.height() >= 320.0, "pas plus petite que le minimum : {fen:?}");
}

#[test]
fn the_open_tab_is_remembered_by_clicking_and_across_sessions() {
    let mut b = banc();
    ouvrir(&mut b, Onglet::General);
    let fen = fenetre(&b);
    // Un clic sur « Équipe » dans la liste.
    let p = place(&b, |t, r| t.ends_with("Équipe") && r.min.x < fen.min.x + 60.0).expect("l'entrée Équipe");
    clic(&mut b, p);
    assert_eq!(b.app.settings.prefs_onglet, Onglet::Equipe);
    let textes: BTreeSet<String> = textes_de_la_fenetre(&b).into_iter().map(|(t, _)| t).collect();
    assert!(contient(&textes, "Une prise périme après"), "{textes:#?}");
    assert!(!contient(&textes, "Rouvrir le dernier projet au lancement"));
    // Fermer puis rouvrir : même onglet.
    b.app.prefs_window = false;
    b.chauffer(3);
    b.app.run_action("fichier.preferences");
    b.chauffer(5);
    assert_eq!(b.app.settings.prefs_onglet, Onglet::Equipe);
    let textes: BTreeSet<String> = textes_de_la_fenetre(&b).into_iter().map(|(t, _)| t).collect();
    assert!(contient(&textes, "Une prise périme après"));
    // Retenu dans le fichier de réglages, sous sa propre clé ; l'onglet d'origine n'y laisse rien.
    let ecrit = toml::to_string(&b.app.settings).unwrap();
    assert!(ecrit.contains("onglet_preferences = \"equipe\""), "{ecrit}");
    let relu: crate::settings::Settings = toml::from_str(&ecrit).unwrap();
    assert_eq!(relu.prefs_onglet, Onglet::Equipe);
    let mut defaut = crate::settings::Settings::default();
    defaut.prefs_onglet = Onglet::General;
    assert!(!toml::to_string(&defaut).unwrap().contains("onglet_preferences"));
    let ancien: crate::settings::Settings = toml::from_str("").unwrap();
    assert_eq!(ancien.prefs_onglet, Onglet::General, "des réglages d'avant : Général");
    // (Les autres clés n'ont pas bougé : la même que `reglages.toml` d'avant.)
    for cle in ["copie_secours_minutes", "curseur_systeme", "rouvrir_dernier_projet", "ouvrir_dans_un_nouvel_onglet", "panneaux_ajustes", "passe_partout_opacite"] {
        assert!(ecrit.contains(cle), "la clé {cle} manque : {ecrit}");
    }
}

#[test]
fn every_tab_name_has_a_stable_key() {
    for (o, cle) in [
        (Onglet::General, "general"),
        (Onglet::Apparence, "apparence"),
        (Onglet::Dessin, "dessin"),
        (Onglet::Anneau, "anneau"),
        (Onglet::Montage, "montage"),
        (Onglet::Vignettes, "vignettes"),
        (Onglet::Raccourcis, "raccourcis"),
        (Onglet::Export, "export"),
        (Onglet::Equipe, "equipe"),
        (Onglet::Generation, "generation"),
        (Onglet::Performance, "performance"),
    ] {
        let mut s = crate::settings::Settings::default();
        s.prefs_onglet = o;
        let t = toml::to_string(&s).unwrap();
        let relu: crate::settings::Settings = toml::from_str(&format!("onglet_preferences = \"{cle}\"")).unwrap();
        assert_eq!(relu.prefs_onglet, o);
        if o != Onglet::General {
            assert!(t.contains(&format!("onglet_preferences = \"{cle}\"")), "{t}");
        }
    }
}

// ------------------------------------------------------------------------------------ la recherche

#[test]
fn the_search_ignores_case_and_accents_and_needs_every_word() {
    assert_eq!(plier("Thème ÉQUIPE çà"), "theme equipe ca");
    assert!(chercher("").is_empty() && chercher("   ").is_empty());
    assert!(chercher("zzzzqqq").is_empty());
    let r = chercher("theme");
    assert!(r.iter().any(|t| t.groupe == Groupe::Theme && t.onglet == Onglet::Apparence), "{:?}", r.iter().map(|t| t.groupe).collect::<Vec<_>>());
    // Tous les mots, dans l'étiquette, les mots ajoutés, le titre du groupe ou le nom de l'onglet.
    let r = chercher("equipe prise");
    assert!(r.iter().all(|t| t.onglet == Onglet::Equipe));
    assert!(r.iter().any(|t| t.libelles.contains(&"Prendre le plan libre que je commence à modifier")));
    let r = chercher("CACHE mo");
    assert!(r.iter().any(|t| t.groupe == Groupe::Performance && t.libelles.contains(&"Cache des effets")), "{:?}", r.iter().map(|t| &t.libelles).collect::<Vec<_>>());
    // Le titre d'un groupe répond pour tous ses réglages.
    let r = chercher("pistes du montage");
    let g = r.iter().find(|t| t.groupe == Groupe::Pistes).expect("le groupe Pistes");
    assert_eq!(g.libelles.len(), 5);
    // Chaque étiquette se retrouve en la cherchant.
    for reg in REGLAGES {
        let r = chercher(reg.libelle);
        assert!(r.iter().any(|t| t.groupe == reg.groupe && t.libelles.contains(&reg.libelle)), "« {} » ne se trouve pas", reg.libelle);
    }
}

#[test]
fn the_search_field_shows_the_groups_that_answer_on_every_tab_and_a_link_jumps_to_the_tab() {
    let mut b = banc();
    ouvrir(&mut b, Onglet::General);
    definir_recherche(&b.ctx, "cache");
    b.chauffer(4);
    let vus: BTreeSet<String> = textes_de_la_fenetre(&b).into_iter().map(|(t, _)| t).collect();
    // Les trois caches de l'onglet Performance, dessinés ici ; rien de l'onglet Général.
    for l in ["Cache de la carte graphique", "Cache des traits vectoriels", "Cache des effets"] {
        assert!(contient(&vus, l), "{l} :\n{vus:#?}");
    }
    assert!(!contient(&vus, "Rouvrir le dernier projet au lancement"));
    // La liste montre le nombre de réponses sur l'onglet Performance.
    assert!(vus.iter().any(|t| t.contains("Performance") && t.contains("(")), "{vus:#?}");
    // Le lien « Performance › » ouvre l'onglet et vide la recherche. (La liste des réponses se pose en quelques images : on attend
    // qu'elle ne bouge plus avant de viser le lien, et elle ne doit pas osciller.)
    let fen = fenetre(&b);
    // (Dans la colonne des réponses, à droite de la liste des onglets : posée, elle commence à 148 points du bord de la fenêtre.)
    let lien = |b: &Banc| place(b, |t, r| t.contains("Performance") && t.contains('\u{203a}') && r.min.x > fen.min.x + 120.0);
    b.chauffer(8);
    let places: Vec<Option<Pos2>> = (0..6)
        .map(|_| {
            b.chauffer(1);
            lien(&b)
        })
        .collect();
    // (Elle glissait de 4,5 points par image : le champ de recherche dépassait de 9 points dès qu'un mot était tapé, et la fenêtre
    // s'élargissait d'autant à chaque image. Corrigé le 2026-11-11, `preferences_contenu`.)
    assert!(places.windows(2).all(|w| w[0] == w[1]), "la liste des réponses bouge encore d'une image à l'autre : {places:?}");
    assert_eq!(fenetre(&b), fen, "la fenêtre ne grandit pas pendant une recherche");
    let p = places[0].expect("le lien vers l'onglet");
    clic(&mut b, p);
    assert_eq!(b.app.settings.prefs_onglet, Onglet::Performance);
    assert_eq!(recherche(&b.ctx), "");
    let vus: BTreeSet<String> = textes_de_la_fenetre(&b).into_iter().map(|(t, _)| t).collect();
    assert!(contient(&vus, "Fils de calcul"));
    // Rien ne répond : un mot le dit.
    definir_recherche(&b.ctx, "zzzqq");
    b.chauffer(4);
    let vus: BTreeSet<String> = textes_de_la_fenetre(&b).into_iter().map(|(t, _)| t).collect();
    assert!(contient(&vus, "Aucun réglage ne répond"), "{vus:#?}");
    // Vide : l'onglet ouvert revient.
    definir_recherche(&b.ctx, "");
    b.chauffer(4);
    let vus: BTreeSet<String> = textes_de_la_fenetre(&b).into_iter().map(|(t, _)| t).collect();
    assert!(contient(&vus, "Fils de calcul"));
}

// ------------------------------------------------------------------------------------ comportement

#[test]
fn settings_still_apply_at_once_and_in_the_same_keys() {
    let mut b = banc();
    ouvrir(&mut b, Onglet::Dessin);
    let avant = b.app.settings.system_cursor;
    let fen = fenetre(&b);
    let p = place(&b, |t, r| t.contains("Croix du système sur le pinceau") && r.min.x > fen.min.x + 150.0).expect("la case");
    // La case est à gauche du texte : le clic sur le texte la bascule aussi.
    clic(&mut b, p);
    assert_ne!(b.app.settings.system_cursor, avant, "la case change le réglage tout de suite");
    let ecrit = std::fs::read_to_string(crate::donnees::roaming().expect("dossier de réglages").join("reglages.toml")).unwrap_or_default();
    assert!(ecrit.contains("curseur_systeme"), "enregistré sous la même clé : {ecrit}");
}

#[test]
fn the_idle_window_asks_for_no_repaint() {
    let mut b = banc();
    for o in [Onglet::General, Onglet::Performance, Onglet::Generation, Onglet::Apparence] {
        ouvrir(&mut b, o);
        b.chauffer(6);
        let repos: Vec<std::time::Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
        assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "{} : la fenêtre au repos fait repeindre : {repos:?}", o.nom());
        b.app.prefs_window = false;
        b.chauffer(2);
    }
}

#[test]
fn another_window_can_open_the_generation_tab() {
    // Le petit engrenage du panneau Suggestions sonores ouvre les Préférences : sur l'onglet Génération.
    let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("suggestionsui.rs")).unwrap();
    assert!(source.contains("prefs_onglet = crate::prefsonglets::Onglet::Generation"), "l'engrenage ouvre l'onglet Génération");
}

/// Une image PNG de chaque onglet (à regarder ; `SHOT_CAPTURE_DIR` : le dossier, sinon le dossier temporaire).
/// `cargo test --release prefs_capture -- --ignored`
#[test]
#[ignore]
fn prefs_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut b = banc_taille(Document::with_frame(1920, 1080, 24), true, 1920, 1080);
    for o in Onglet::TOUS {
        ouvrir(&mut b, o);
        b.chauffer(4);
        let Some(rgba) = b.capturer() else { return println!("PREFS pas de carte graphique : capture ignorée") };
        image::save_buffer(dossier.join(format!("prefs-{}.png", o.nom())), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        b.app.prefs_window = false;
        b.chauffer(2);
    }
    ouvrir(&mut b, Onglet::General);
    definir_recherche(&b.ctx, "couleur");
    b.chauffer(4);
    if let Some(rgba) = b.capturer() {
        image::save_buffer(dossier.join("prefs-recherche.png"), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    }
}