//! Essais du rangement des exports dans Fichier (partie 130) : quatre sous-menus qui reprennent tous les exports de la grande
//! fenêtre Exporter, chaque entrée ouvre sa boîte de réglages ; les trois entrées d'Aide et la Performance ont quitté leurs menus
//! sans quitter la palette ni F1.

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::exportmenu::SOUS_MENUS;
use crate::exports_v1_tests::{dossier_neuf, projet_uni};
use crate::export::Format;

fn appli(nom: &str) -> (App, std::path::PathBuf) {
    let dir = dossier_neuf(nom);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = projet_uni(3, 12, Color32::from_rgb(90, 120, 200));
    app.cur_panel = 0;
    (app, dir)
}

fn actions_du_menu() -> Vec<&'static str> {
    SOUS_MENUS.iter().flat_map(|(_, e)| e.iter()).filter(|(t, _)| !t.is_empty()).map(|(_, a)| *a).collect()
}

#[test]
fn the_four_submenus_hold_every_export_of_the_big_window_once() {
    let titres: Vec<&str> = SOUS_MENUS.iter().map(|m| m.0).collect();
    assert_eq!(titres, ["Exporter en vidéo", "Exporter en image", "Exporter le son", "Exporter pour la production"]);
    let mut menu = actions_du_menu();
    let mut hub: Vec<&str> = crate::prefsui::FAMILLES.iter().flat_map(|f| f.1.iter()).map(|e| e.3).collect();
    // « Journal des différences » et « Noms et production » sont dans les deux : le menu ne perd rien de la fenêtre.
    for a in &hub {
        assert!(menu.contains(a), "{a} : dans la fenêtre Exporter mais plus dans le menu");
    }
    let n = menu.len();
    menu.sort_unstable();
    menu.dedup();
    assert_eq!(menu.len(), n, "une entrée par action");
    hub.sort_unstable();
    assert_eq!(menu, hub, "le menu ne propose rien que la fenêtre n'avait pas");
    for (_, entrees) in SOUS_MENUS {
        assert!(!entrees.is_empty() && entrees[0].0 != "" && entrees.last().unwrap().0 != "", "pas de trait en tête ni en queue");
        for (texte, action) in entrees {
            if texte.is_empty() {
                continue;
            }
            assert!(texte.ends_with('…'), "{texte} : une boîte s'ouvre");
            assert!(crate::keymap::action_info(action).is_some_and(|a| a.implemented), "{action} existe");
            for banni in ["Storyboard Pro", "Toon Boom", "Premiere", "DaVinci", "Avid", "After Effects", "Final Cut"] {
                assert!(!texte.contains(banni), "{texte} cite {banni}");
            }
        }
    }
}

#[test]
fn the_doubtful_exports_are_where_the_part_130_decided() {
    let dans = |menu: usize, action: &str| SOUS_MENUS[menu].1.iter().any(|e| e.1 == action);
    // Les pages et planches à regarder : image.
    for a in ["fichier.exporter_pdf", "fichier.exporter_planche", "fichier.exporter_colorscript"] {
        assert!(dans(1, a), "{a} : image");
    }
    // Ce qui part vers le réalisateur, le monteur ou la production : production.
    for a in ["fichier.exporter_relecture", "envoi.journal", "fichier.exporter_renumerotation", "fichier.exporter_montage", "fichier.kitsu_publier", "fichier.exporter_paquet"] {
        assert!(dans(3, a), "{a} : production");
    }
    assert!(dans(0, "fichier.exporter_film") && dans(0, "fichier.exporter_gif"));
    assert!(dans(2, "fichier.exporter_son") && dans(2, "fichier.exporter_son_par_piste") && dans(2, "fichier.exporter_son_par_plan"));
}

#[test]
fn each_entry_opens_its_own_box_without_the_big_window() {
    crate::fxcache::monde::lire();
    let (mut app, dir) = appli("exportmenu-boites");
    // (L'image courante et le rapport de numérotation n'ont pas de boîte : un dialogue d'enregistrement, qu'un essai ne peut pas ouvrir.)
    for (action, format) in [
        ("fichier.exporter_film", Format::Mp4),
        ("fichier.exporter_gif", Format::Gif),
        ("fichier.exporter_images", Format::Png),
        ("fichier.exporter_images_par_plan", Format::Stills),
        ("fichier.exporter_images_cles", Format::Stills),
        ("fichier.exporter_pdf", Format::Pdf),
        ("fichier.exporter_son", Format::Wav),
        ("fichier.exporter_son_par_piste", Format::Stems),
        ("fichier.exporter_son_par_plan", Format::SonPlans),
        ("fichier.exporter_calques", Format::Layers),
        ("fichier.exporter_camera", Format::Camera),
    ] {
        assert!(actions_du_menu().contains(&action));
        app.export_dialog = None;
        app.export_hub = false;
        app.run_action(action);
        assert_eq!(app.export_dialog.as_ref().map(|s| s.format), Some(format), "{action}");
        assert!(!app.export_hub, "{action} : la grande fenêtre ne s'ouvre pas");
    }
    app.run_action("fichier.exporter_planche");
    assert!(app.vignettes.planche.ouverte);
    app.run_action("fichier.exporter_colorscript");
    assert!(app.vignettes.colorscript.ouverte);
    app.run_action("fichier.exporter_montage");
    assert!(app.editorial.is_some());
    app.run_action("fichier.exporter_fiche");
    assert!(app.fiche.is_some());
    app.run_action("fichier.exporter_liste_plans");
    assert!(app.liste_plans.is_some());
    app.run_action("fichier.exporter_paquet");
    assert!(app.paquet.is_some());
    app.run_action("fichier.kitsu_publier");
    assert!(app.kitsu.ouvert);
    app.run_action("projet.noms_production");
    assert!(app.noms_fenetre.is_some());
    assert!(!app.export_hub);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_menus_have_lost_what_anthony_circled_and_the_commands_stay_reachable() {
    let panels = include_str!("panels.rs");
    let barre = &panels[panels.find("pub(crate) fn menu_bar").expect("menu_bar")..];
    let entre = |de: &str, a: &str| {
        let i = barre.find(de).unwrap_or_else(|| panic!("{de}"));
        let j = barre[i..].find(a).unwrap_or_else(|| panic!("{a}")) + i;
        &barre[i..j]
    };
    let fichier = entre("ui.menu_button(\"Fichier\"", "if menu_actif(ui, sans_projet, \"Édition\")");
    assert!(fichier.contains("self.export_menus(ui)") && !fichier.contains("\"fichier.exporter\")"), "Fichier : quatre sous-menus, plus d'« Exporter… »");
    let affichage = entre("ui.menu_button(\"Affichage\"", "if self.settings.experimental");
    assert!(!affichage.contains("vue.performance"), "Affichage n'a plus « Performance »");
    let labo = entre("ui.menu_button(\"Labo\"", "ui.menu_button(\"Aide\"");
    assert!(labo.contains("vue.performance"), "Labo garde l'outil");
    let aide = entre("ui.menu_button(\"Aide\"", "À propos de FEANOR");
    for parti in ["aide.raccourcis", "aide.regler_stylet", "styletui::ACTION"] {
        assert!(!aide.contains(parti), "Aide n'a plus {parti}");
    }
    // Rien n'est supprimé : la palette connaît toujours ces commandes, et F1 ouvre toujours les raccourcis.
    for a in ["aide.raccourcis", "aide.regler_stylet", crate::styletui::ACTION, "vue.performance", "fichier.exporter"] {
        assert!(crate::keymap::action_info(a).is_some_and(|i| i.implemented), "{a} reste dans la palette");
    }
    assert_eq!(crate::keymap_tests::default_map().label("aide.raccourcis"), "F1");
}

#[test]
fn the_preferences_hold_the_three_commands_that_left_the_help_menu() {
    use crate::prefsonglets::{Groupe, Onglet, REGLAGES};
    let dessin = |id: &str| REGLAGES.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("{id}"));
    assert_eq!(dessin("dessin.regler_stylet").onglet, Onglet::Dessin);
    assert_eq!(dessin("dessin.stylet_banc").onglet, Onglet::Dessin);
    assert_eq!(dessin("dessin.stylet_banc").groupe, Groupe::StyletMain);
    assert_eq!(dessin("raccourcis.liste").onglet, Onglet::Raccourcis);
}

// ------------------------------------------------------------------------------ à regarder

/// Les menus rangés, le panneau Transformer et les Préférences, en PNG (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --locked --target-dir target\msvc menus_exports_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn menus_exports_captures() {
    use crate::app::Tool;
    use crate::perf_tests::Banc;
    use eframe::egui::{epaint::ClippedShape, pos2, Event, Modifiers, PointerButton, Pos2, Rect, Shape};
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("CAPTURE {nom}");
    };
    fn a_plat(formes: &[ClippedShape]) -> Vec<&Shape> {
        fn va<'a>(s: &'a Shape, out: &mut Vec<&'a Shape>) {
            match s {
                Shape::Vec(v) => v.iter().for_each(|s| va(s, out)),
                autre => out.push(autre),
            }
        }
        let mut out = Vec::new();
        formes.iter().for_each(|c| va(&c.shape, &mut out));
        out
    }
    // Le centre du dernier texte qui commence par `debut` (le menu ouvert est peint par-dessus).
    fn ou(b: &Banc, debut: &str) -> Option<Pos2> {
        a_plat(&b.formes).iter().rev().find_map(|s| match s {
            Shape::Text(t) if t.galley.job.text.trim_start().starts_with(debut) => Some(Rect::from_min_size(t.pos, t.galley.size()).center()),
            _ => None,
        })
    }
    fn clic(b: &mut Banc, p: Pos2) {
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
        b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.chauffer(4);
    }
    fn survoler(b: &mut Banc, p: Pos2) {
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
        b.chauffer(5);
    }
    let mut b = crate::perf_tests::banc(crate::vignettes_tests::quatre_plans(), true);
    b.garder_formes = true;
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.chauffer(10);
    // Fichier, puis chacun des quatre sous-menus.
    let p = ou(&b, "Fichier").expect("Fichier");
    clic(&mut b, p);
    sauver(&b, "menus-01-fichier.png");
    for (k, (titre, _)) in SOUS_MENUS.iter().enumerate() {
        let p = ou(&b, titre).unwrap_or_else(|| panic!("{titre}"));
        survoler(&mut b, p);
        sauver(&b, &format!("menus-0{}-{}.png", k + 2, titre.to_lowercase().replace(' ', "-").replace('é', "e")));
    }
    // Aide et Affichage.
    b.image(vec![Event::PointerMoved(pos2(900.0, 600.0))], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: pos2(900.0, 600.0), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }, Event::PointerButton { pos: pos2(900.0, 600.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(4);
    for (menu, marque, nom) in [("Aide", "À propos", "menus-06-aide.png"), ("Affichage", "Ajuster à la fenêtre", "menus-07-affichage.png")] {
        let p = ou(&b, menu).unwrap_or_else(|| panic!("{menu}"));
        for _ in 0..3 {
            if ou(&b, marque).is_some() {
                break;
            }
            clic(&mut b, p);
        }
        assert!(ou(&b, marque).is_some(), "le menu {menu} est ouvert");
        sauver(&b, nom);
        clic(&mut b, p);
    }
    // Le panneau Transformer, replié puis déplié, sur un calque vectoriel et un calque bitmap.
    b.app.settings.experimental = false;
    b.image(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    b.app.set_tool(Tool::Transform);
    b.chauffer(6);
    sauver(&b, "transformer-01-vectoriel-plie.png");
    b.app.pinceaux.regler_plus(Tool::Transform.action(), true);
    b.chauffer(6);
    sauver(&b, "transformer-02-vectoriel-deplie.png");
    b.app.pinceaux.regler_plus(Tool::Transform.action(), false);
    { let (p, l) = (b.app.cur_panel, b.app.cur_layer); b.app.doc.panels[p].layers[l].kind = crate::document::LayerKind::Bitmap; }
    b.chauffer(6);
    sauver(&b, "transformer-03-bitmap-plie.png");
    // Les Préférences : l'onglet Dessin (le stylet) et l'onglet Raccourcis.
    for (onglet, nom) in [(crate::prefsonglets::Onglet::Dessin, "preferences-01-dessin.png"), (crate::prefsonglets::Onglet::Raccourcis, "preferences-02-raccourcis.png")] {
        b.app.settings.prefs_onglet = onglet;
        b.app.prefs_window = true;
        b.chauffer(8);
        sauver(&b, nom);
    }
}
