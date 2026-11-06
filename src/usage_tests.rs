//! Le compteur d'usage (`usage.rs`, `usageui.rs` ; Anthony, 2026-11-06) : le compte sans interface, puis dans
//! l'application entière (clavier, anneau, icône, fichier du poste, fenêtre « Mes habitudes »).

use eframe::egui::{epaint::Shape, vec2, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::app::{App, Tool};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::usage::{self, jour_en_clair, numero_du_jour, Source, Usage};

const DT: f64 = 1.0 / 60.0;
const JOUR: &str = "2026-11-06";

// ------------------------------------------------------------------------------------------------ le compte, sans interface

#[test]
fn each_command_is_counted_with_the_way_it_was_launched() {
    let mut u = Usage::default();
    assert_eq!(u.total(), 0);
    assert!(u.depuis.is_empty() && u.jours(JOUR).is_none());
    u.noter("outil.gomme", Source::Clavier, JOUR, 1.0, true);
    u.noter("outil.gomme", Source::Clavier, JOUR, 2.0, true);
    u.noter("outil.gomme", Source::Anneau, "2026-11-07", 3.0, true);
    u.noter("vue.miroir", Source::Menu, "2026-11-07", 4.0, true);
    let gomme = &u.commandes["outil.gomme"];
    assert_eq!((gomme.clavier, gomme.anneau, gomme.total()), (2, 1, 3));
    assert_eq!(gomme.dernier, "2026-11-07", "le dernier jour où elle a servi");
    assert_eq!(gomme.detail(), "clavier 2 · anneau 1");
    assert_eq!(gomme.chemin_principal(), Some(Source::Clavier));
    assert_eq!(u.depuis, JOUR, "le comptage commence au premier jour compté");
    assert_eq!(u.total(), 4);
    assert!(u.a_ecrire);
    // Les plus utilisées d'abord ; à égalité, par identifiant.
    u.noter("edition.annuler", Source::Clavier, JOUR, 5.0, true);
    let noms: Vec<&str> = u.plus_utilisees(10).into_iter().map(|(a, _)| a).collect();
    assert_eq!(noms, ["outil.gomme", "edition.annuler", "vue.miroir"]);
    assert_eq!(u.plus_utilisees(1).len(), 1);
    // Jamais utilisées, parmi une liste donnée, dans son ordre.
    assert_eq!(u.jamais(["outil.cutter", "outil.gomme", "outil.main"]), ["outil.cutter", "outil.main"]);
    // Une commande sans nom n'est pas comptée.
    u.noter("", Source::Clavier, JOUR, 6.0, true);
    assert_eq!(u.total(), 5);
}

#[test]
fn a_click_and_the_command_it_launches_count_once() {
    // La commande part (chemin inconnu), puis son clic d'icône arrive à l'image suivante : une fois, sous « icône ».
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Autre, JOUR, 10.0, true);
    u.noter("outil.gomme", Source::Icone, JOUR, 10.016, true);
    let c = &u.commandes["outil.gomme"];
    assert_eq!((c.icone, c.autre, c.total()), (1, 0, 1));
    assert_eq!(u.recentes.len(), 1);
    assert_eq!(u.recentes[0].source, Source::Icone);
    // Dans l'autre ordre : pareil.
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Menu, JOUR, 10.0, true);
    u.noter("outil.gomme", Source::Autre, JOUR, 10.01, true);
    assert_eq!((u.commandes["outil.gomme"].menu, u.commandes["outil.gomme"].total()), (1, 1));
    // Deux vraies frappes rapprochées de la même touche comptent toutes les deux.
    let mut u = Usage::default();
    u.noter("image.suivante", Source::Clavier, JOUR, 10.0, true);
    u.noter("image.suivante", Source::Clavier, JOUR, 10.1, true);
    assert_eq!(u.commandes["image.suivante"].clavier, 2);
    // Passé le délai, ce sont deux usages.
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Autre, JOUR, 10.0, true);
    u.noter("outil.gomme", Source::Icone, JOUR, 10.0 + usage::DOUBLON + 0.1, true);
    assert_eq!(u.commandes["outil.gomme"].total(), 2);
    // Une autre commande entre les deux : rien n'est fondu.
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Autre, JOUR, 10.0, true);
    u.noter("vue.miroir", Source::Autre, JOUR, 10.01, true);
    u.noter("outil.gomme", Source::Icone, JOUR, 10.02, true);
    assert_eq!(u.commandes["outil.gomme"].total(), 2);
}

#[test]
fn switched_off_nothing_is_counted_but_the_journal_goes_on() {
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Clavier, JOUR, 1.0, false);
    assert_eq!(u.total(), 0);
    assert!(u.depuis.is_empty() && !u.a_ecrire);
    assert_eq!(u.recentes.len(), 1, "le journal des dernières commandes sert à la note de friction");
    // Le journal ne garde que les dernières.
    for k in 0..(usage::RECENTES + 10) {
        u.noter(&format!("commande.{k}"), Source::Clavier, JOUR, 2.0 + k as f64, false);
    }
    assert_eq!(u.recentes.len(), usage::RECENTES);
    let dernieres: Vec<&str> = u.dernieres(2).map(|r| r.action.as_str()).collect();
    assert_eq!(dernieres, [format!("commande.{}", usage::RECENTES + 8).as_str(), format!("commande.{}", usage::RECENTES + 9).as_str()]);
}

#[test]
fn the_count_file_holds_only_command_names_and_numbers_and_survives_damage() {
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Anneau, JOUR, 1.0, true);
    u.noter("vue.miroir", Source::Clavier, JOUR, 2.0, true);
    let ecrit = u.ecrire();
    assert!(ecrit.contains("\"outil.gomme\"") && ecrit.contains("\"anneau\": 1") && ecrit.contains(JOUR), "{ecrit}");
    // Les chemins à zéro ne sont pas écrits, le journal non plus.
    assert!(!ecrit.contains("palette") && !ecrit.contains("recentes") && !ecrit.contains("instant"), "{ecrit}");
    let relu = Usage::lire(&ecrit);
    assert_eq!(relu.commandes, u.commandes);
    assert_eq!(relu.depuis, JOUR);
    assert!(relu.recentes.is_empty() && !relu.a_ecrire);
    // Un fichier abîmé, vide ou d'un autre monde : un compte neuf.
    for abime in ["", "{", "[1, 2, 3]", "{\"commandes\": 12}", "pas du json"] {
        assert_eq!(Usage::lire(abime), Usage::default(), "{abime:?}");
    }
    // Des champs inconnus ou manquants ne gênent pas.
    let ancien = Usage::lire("{\"depuis\": \"2026-10-01\", \"commandes\": {\"outil.main\": {\"clavier\": 3, \"plus_tard\": true}}, \"autre_chose\": 1}");
    assert_eq!(ancien.commandes["outil.main"].total(), 3);
    // Remettre à zéro.
    let mut u = relu;
    u.effacer();
    assert!(u.commandes.is_empty() && u.depuis.is_empty() && u.a_ecrire);
}

#[test]
fn days_are_counted_across_months_and_leap_years() {
    assert_eq!(numero_du_jour("1970-01-01"), Some(0));
    assert_eq!(numero_du_jour("1970-01-02"), Some(1));
    assert_eq!(numero_du_jour("2000-03-01").unwrap() - numero_du_jour("2000-02-28").unwrap(), 2, "2000 est bissextile");
    assert_eq!(numero_du_jour("2026-03-01").unwrap() - numero_du_jour("2026-02-28").unwrap(), 1);
    assert_eq!(numero_du_jour("2026-11-06").unwrap() - numero_du_jour("2026-10-23").unwrap(), 14);
    for faux in ["", "2026", "2026-13-01", "2026-11-00", "hier", "2026-11-xx"] {
        assert_eq!(numero_du_jour(faux), None, "{faux:?}");
    }
    let mut u = Usage::default();
    u.noter("outil.gomme", Source::Clavier, "2026-10-23", 1.0, true);
    assert_eq!(u.jours("2026-11-06"), Some(14));
    assert_eq!(u.jours("2026-10-23"), Some(0));
    assert_eq!(u.jours("2026-10-01"), Some(0), "une horloge reculée ne donne pas un nombre négatif");
    assert_eq!(jour_en_clair("2026-11-06"), "6 novembre 2026");
    assert_eq!(jour_en_clair("2026-03-01"), "1er mars 2026");
    assert_eq!(jour_en_clair("n'importe quoi"), "n'importe quoi");
}

// -------------------------------------------------------------------------------------------- dans l'application entière

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert!(b.app.settings.usage_actif, "allumé d'origine");
    assert_eq!(b.app.usage.compte.total(), 0, "un poste neuf : rien de compté");
    b
}

fn touche(b: &mut Banc, key: Key, repeat: bool) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat, modifiers: Modifiers::NONE }], DT);
}

fn lacher_la_touche(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: Modifiers::NONE }], DT);
}

fn bouton(p: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }
}

fn places_peintes(shape: &Shape, sortie: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => sortie.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| places_peintes(s, sortie)),
        _ => {}
    }
}

fn places(b: &Banc) -> Vec<(String, Rect)> {
    let mut t = Vec::new();
    for s in &b.formes {
        places_peintes(&s.shape, &mut t);
    }
    t
}

fn textes(b: &Banc) -> Vec<String> {
    places(b).into_iter().map(|(t, _)| t).collect()
}

fn compte(b: &Banc, action: &str) -> usage::Compte {
    b.app.usage.compte.commandes.get(action).cloned().unwrap_or_default()
}

#[test]
fn a_key_counts_as_keyboard_and_a_held_key_counts_once() {
    let mut b = banc();
    // M : le miroir de la vue.
    assert_eq!(b.app.keymap.label("vue.miroir"), "M");
    touche(&mut b, Key::M, false);
    for _ in 0..5 {
        touche(&mut b, Key::M, true);
    }
    lacher_la_touche(&mut b, Key::M);
    let c = compte(&b, "vue.miroir");
    assert_eq!((c.clavier, c.total()), (1, 1), "une touche tenue qui se répète ne compte qu'une fois : {c:?}");
    touche(&mut b, Key::M, false);
    lacher_la_touche(&mut b, Key::M);
    assert_eq!(compte(&b, "vue.miroir").clavier, 2);
    // Une commande lancée par un autre chemin : « autre ».
    b.app.run_action("vue.ajuster");
    assert_eq!(compte(&b, "vue.ajuster").autre, 1);
    // Le journal garde l'ordre.
    let journal: Vec<&str> = b.app.usage.compte.dernieres(3).map(|r| r.action.as_str()).collect();
    assert_eq!(journal, ["vue.miroir", "vue.miroir", "vue.ajuster"]);
    assert!(!b.app.usage.compte.depuis.is_empty());
}

#[test]
fn the_ring_and_a_tool_icon_are_counted_under_their_own_way() {
    use egui_phosphor::regular as ph;
    let mut b = banc();
    // L'anneau sous le stylet : un glissé vers le bas, la Gomme.
    let p = b.app.canvas_rect.center();
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, PointerButton::Secondary, true)], DT);
    b.image(vec![Event::PointerMoved(p + vec2(0.0, 60.0))], DT);
    b.image(vec![bouton(p + vec2(0.0, 60.0), PointerButton::Secondary, false)], DT);
    b.chauffer(2);
    assert_eq!(b.app.tool, Tool::Eraser);
    let c = compte(&b, "outil.gomme");
    assert_eq!((c.anneau, c.total()), (1, 1), "{c:?}");
    // Un clic sur l'icône du Crayon, dans la barre d'outils : « icône », une seule fois.
    let (_, icone) = places(&b).into_iter().filter(|(t, r)| t == ph::PENCIL_SIMPLE && r.min.x < 60.0).min_by(|a, c| a.1.min.x.total_cmp(&c.1.min.x)).expect("l'icône du Crayon dans la barre d'outils");
    let q = icone.center();
    b.image(vec![Event::PointerMoved(q)], DT);
    b.image(vec![bouton(q, PointerButton::Primary, true)], DT);
    b.image(vec![bouton(q, PointerButton::Primary, false)], DT);
    b.chauffer(3);
    assert_eq!(b.app.tool, Tool::Pencil);
    let c = compte(&b, "outil.crayon");
    assert_eq!((c.icone, c.total()), (1, 1), "le clic et la commande qu'il lance ne font qu'un : {c:?}");
}

#[test]
fn switched_off_in_the_window_nothing_more_is_counted() {
    let mut b = banc();
    b.app.run_action("vue.miroir");
    assert_eq!(compte(&b, "vue.miroir").total(), 1);
    b.app.settings.usage_actif = false;
    b.app.run_action("vue.miroir");
    b.app.run_action("outil.gomme");
    assert_eq!(compte(&b, "vue.miroir").total(), 1, "le compte déjà fait reste");
    assert_eq!(compte(&b, "outil.gomme").total(), 0);
    assert_eq!(b.app.usage.compte.recentes.back().map(|r| r.action.as_str()), Some("outil.gomme"), "le journal continue");
    // Le réglage est retenu sous sa propre clé ; allumé, il n'écrit rien.
    let ecrit = toml::to_string(&b.app.settings).unwrap();
    assert!(ecrit.contains("compter_les_commandes = false"), "{ecrit}");
    assert!(!toml::to_string(&Settings::default()).unwrap().contains("compter_les_commandes"));
    let ancien: Settings = toml::from_str("").unwrap();
    assert!(ancien.usage_actif, "des réglages d'avant : allumé");
}

#[test]
fn the_count_is_written_at_rest_and_at_closing_then_read_back() {
    let mut b = banc();
    let fichier = crate::donnees::roaming().unwrap().join(usage::FICHIER);
    let _ = std::fs::remove_file(&fichier);
    b.app.run_action("vue.miroir");
    b.chauffer(3);
    assert!(!fichier.exists(), "pas d'écriture à chaque commande");
    // Une minute plus tard, au repos : écrit.
    b.image(Vec::new(), 61.0);
    b.chauffer(2);
    let texte = std::fs::read_to_string(&fichier).expect("le compte est écrit au repos");
    assert!(texte.contains("vue.miroir"), "{texte}");
    assert!(!b.app.usage.compte.a_ecrire);
    // Pas pendant qu'un bouton est appuyé.
    b.app.run_action("outil.gomme");
    let p = b.app.canvas_rect.center();
    b.image(vec![Event::PointerMoved(p), bouton(p, PointerButton::Secondary, true)], DT);
    b.image(Vec::new(), 61.0);
    assert!(b.app.usage.compte.a_ecrire, "bouton appuyé : l'écriture attend");
    b.image(vec![bouton(p, PointerButton::Secondary, false)], DT);
    // À la fermeture, ce qui reste est écrit ; le lancement suivant le relit.
    b.app.run_action("outil.crayon");
    b.app.usage_ecrire();
    let relu = App::usage_lire();
    assert_eq!(relu.commandes, b.app.usage.compte.commandes);
    assert!(relu.commandes.contains_key("outil.crayon") && relu.commandes.contains_key("vue.miroir"));
}

#[test]
fn the_habits_window_shows_the_most_used_commands_and_says_when_it_is_too_early() {
    let mut b = banc();
    assert!(!b.app.usage.fenetre);
    for _ in 0..3 {
        b.app.usage.source = Some(Source::Clavier);
        b.app.run_action("outil.gomme");
    }
    b.app.usage.source = Some(Source::Anneau);
    b.app.run_action("outil.gomme");
    b.app.run_action("vue.miroir");
    b.app.run_action("aide.habitudes");
    assert!(b.app.usage.fenetre);
    b.chauffer(6);
    let t = textes(&b);
    for attendu in ["Mes habitudes", "Les plus utilisées", "Jamais utilisées", "Compter mes commandes", "Le compte reste sur cet ordinateur"] {
        assert!(t.iter().any(|x| x.contains(attendu)), "« {attendu} » dans la fenêtre : {t:?}");
    }
    assert!(t.iter().any(|x| x.ends_with("Gomme")), "la Gomme, la plus utilisée : {t:?}");
    assert!(t.iter().any(|x| x.contains("clavier 3 · anneau 1")), "le détail par chemin : {t:?}");
    assert!(t.iter().any(|x| x.contains("dans l'anneau")), "la Gomme est déjà dans l'anneau : {t:?}");
    assert!(t.iter().any(|x| x.contains("commandes comptées depuis aujourd'hui")), "{t:?}");
    assert!(t.iter().any(|x| x.contains("Trop tôt pour conclure")), "le premier jour, « jamais » ne veut rien dire : {t:?}");
    // Deux semaines plus tard : les outils et les boutons jamais touchés sont listés.
    let debut = usage::numero_du_jour(&b.app.usage.compte.depuis).unwrap();
    b.app.usage.compte.depuis = "2020-01-01".into();
    assert!(usage::numero_du_jour("2020-01-01").unwrap() < debut);
    b.chauffer(3);
    let t = textes(&b);
    assert!(t.iter().any(|x| x.contains("Bons candidats à cacher")), "{t:?}");
    assert!(t.iter().any(|x| x.ends_with("Cutter")), "le Cutter n'a jamais servi : {t:?}");
    // La même commande la referme.
    b.app.run_action("aide.habitudes");
    assert!(!b.app.usage.fenetre);
    // Remise à zéro.
    b.app.usage.compte.effacer();
    assert_eq!(b.app.usage.compte.total(), 0);
}

/// Captures pour voir « Mes habitudes » et le champ de la note de friction (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc usage_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn usage_captures() {
    use eframe::egui::{Color32, ColorImage, UserData, ViewportId};
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("USAGE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("USAGE {nom}");
    };
    let mut b = crate::perf_tests::banc(Document::new(400, 300, 24), true);
    b.app.tool = Tool::Brush;
    b.chauffer(8);
    // Deux semaines d'un usage plausible.
    let mut t = 1.0;
    for (action, source, n) in [
        ("outil.gomme", Source::Clavier, 42),
        ("outil.gomme", Source::Anneau, 18),
        ("outil.pinceau", Source::Clavier, 37),
        ("edition.annuler", Source::Clavier, 31),
        ("panneau.suivant", Source::Anneau, 22),
        ("vue.miroir", Source::Menu, 9),
        ("panneau.dupliquer", Source::Icone, 7),
        ("outil.selection", Source::Palette, 3),
    ] {
        for _ in 0..n {
            b.app.usage.compte.noter(action, source, "2026-10-23", t, true);
            t += 1.0;
        }
    }
    b.app.run_action("aide.habitudes");
    b.chauffer(8);
    sauver(&b, "usage-1-habitudes.png");
    b.app.run_action("aide.habitudes");
    b.chauffer(2);
    // Le champ de la note de friction.
    b.app.run_action("aide.noter_friction");
    b.image(vec![Event::Screenshot { viewport_id: ViewportId::ROOT, user_data: UserData::default(), image: std::sync::Arc::new(ColorImage::new([16, 9], Color32::GRAY)) }], DT);
    b.chauffer(3);
    b.image(vec![Event::Text("la gomme reste prise après l'anneau".into())], DT);
    b.chauffer(2);
    sauver(&b, "usage-2-friction.png");
}
