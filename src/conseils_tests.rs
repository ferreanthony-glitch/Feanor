//! Les conseils d'un seul passage (`conseils.rs`) : une bulle, une phrase, « Compris », la première fois seulement ; jamais
//! deux à la fois ; retenus dans les réglages ; « Revoir les conseils » les rend.

use eframe::egui::{epaint::Shape, Event, Modifiers, PointerButton, Pos2};

use crate::conseils::{a_montrer, Conseil, ReglageConseils};
use crate::document::Document;
use crate::keymap::{Keymap, PresetId};
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::workspace::Espace;

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.confort.conseils_actifs = true;
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

fn tous_les_textes(b: &Banc) -> Vec<(String, Pos2)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut out);
    }
    out
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|(t, _)| t.contains(texte))
}

/// Combien de boutons « Compris » à l'écran (une bulle = un bouton).
fn bulles(b: &Banc) -> usize {
    tous_les_textes(b).iter().filter(|(t, _)| t == "Compris").count()
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

#[test]
fn there_are_five_tips_at_most_each_one_sentence_with_a_stable_id() {
    assert!(Conseil::TOUS.len() <= 5);
    let b = banc();
    let mut ids = std::collections::BTreeSet::new();
    for c in Conseil::TOUS {
        assert!(ids.insert(c.id()), "identifiant en double : {}", c.id());
        let t = b.app.conseil_texte(c);
        assert!(t.ends_with('.') && !t.contains('\n'), "{c:?} : une seule phrase : {t}");
        assert_eq!(t.matches(". ").count(), 0, "{c:?} : une seule phrase : {t}");
        assert!(t.chars().count() < 170, "{c:?} : une phrase courte : {t}");
        assert!(!c.titre().is_empty());
    }
    // Le vocabulaire de SHOT.
    assert!(b.app.conseil_texte(Conseil::Montage).contains("Pistes") && b.app.conseil_texte(Conseil::Montage).contains("Plan"));
    assert!(b.app.conseil_texte(Conseil::Vignettes).contains("Cases"));
}

#[test]
fn the_shortcut_tip_reads_the_live_keymap() {
    let mut b = banc();
    let t = b.app.conseil_texte(Conseil::Raccourcis);
    assert!(t.contains("Z masque") && t.contains("maintiens R"), "{t}");
    // Le fichier personnel les change : la phrase suit.
    let user = "version = 1\n[[raccourci]]\naction = \"vue.concentration\"\ntouches = [\"K\"]\n[[raccourci]]\naction = \"aide.aide_memoire\"\ntouches = [\"J\"]\n";
    b.app.keymap = Keymap::build(PresetId::Shot, Some(user));
    let t = b.app.conseil_texte(Conseil::Raccourcis);
    assert!(t.contains("K masque") && t.contains("maintiens J"), "{t}");
    // Plus de touche du tout : la phrase le dit sans inventer.
    let sans = "version = 1\n[[raccourci]]\naction = \"vue.concentration\"\ntouches = []\n[[raccourci]]\naction = \"aide.aide_memoire\"\ntouches = []\n";
    b.app.keymap = Keymap::build(PresetId::Shot, Some(sans));
    assert!(b.app.conseil_texte(Conseil::Raccourcis).contains("menu Affichage"));
}

#[test]
fn the_tip_of_a_place_is_chosen_only_when_unseen_and_the_screen_is_free() {
    let mut vus = ReglageConseils::default();
    assert_eq!(a_montrer(Some(Conseil::Montage), &vus, true), Some(Conseil::Montage));
    assert_eq!(a_montrer(Some(Conseil::Montage), &vus, false), None, "écran occupé : on attend");
    assert_eq!(a_montrer(None, &vus, true), None);
    assert!(vus.marquer(Conseil::Montage) && !vus.marquer(Conseil::Montage));
    assert_eq!(a_montrer(Some(Conseil::Montage), &vus, true), None, "déjà vu");
    assert_eq!(a_montrer(Some(Conseil::Vignettes), &vus, true), Some(Conseil::Vignettes));
    assert_eq!(vus.nombre_vus(), 1);
    vus.tout_oublier();
    assert!(vus.est_vide());
}

#[test]
fn opening_montage_shows_its_tip_once_with_a_compris_button() {
    let mut b = banc();
    assert_eq!(bulles(&b), 0, "rien au départ, dans l'onglet Storyboard d'un projet intact");
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Montage));
    assert_eq!(bulles(&b), 1);
    assert!(est_la(&b, "tire le bord d'un Plan"), "la phrase est à l'écran");
    assert!(b.app.settings.conseils.a_vu(Conseil::Montage), "noté vu dès qu'il apparaît");
    // « Compris » le ferme.
    let compris = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Compris").unwrap().1;
    clic(&mut b, compris);
    assert_eq!(b.app.confort.conseil, None);
    assert_eq!(bulles(&b), 0);
    // Il ne revient pas : ni en restant, ni en repassant par là.
    b.chauffer(6);
    assert_eq!(bulles(&b), 0);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, None, "une seule fois");
    assert_eq!(bulles(&b), 0);
}

#[test]
fn a_tip_that_was_not_clicked_leaves_with_its_place_and_does_not_come_back() {
    let mut b = banc();
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Vignettes));
    // On part sans cliquer : la bulle s'en va avec l'onglet.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    assert_eq!(bulles(&b), 0);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    assert_eq!(bulles(&b), 0, "il était noté vu quand il est apparu");
}

#[test]
fn two_tips_never_stack_and_the_first_launch_offer_goes_first() {
    let mut b = banc();
    // La proposition « Régler SHOT pour cette machine ? » est à l'écran : le conseil attend.
    b.app.diag.offre = true;
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, None, "le conseil attend la réponse à la proposition");
    assert_eq!(bulles(&b), 0);
    assert!(!b.app.settings.conseils.a_vu(Conseil::Montage), "il n'est pas noté vu tant qu'il n'est pas apparu");
    b.app.repondre_offre(false);
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Montage));
    // Un autre endroit pendant qu'il est là : l'un remplace l'autre, jamais deux boutons.
    for espace in [Espace::Vignettes, Espace::Animation, Espace::Montage, Espace::Storyboard, Espace::Vignettes] {
        b.app.set_espace(espace);
        for _ in 0..4 {
            b.chauffer(1);
            assert!(bulles(&b) <= 1, "{espace:?} : {} bulles", bulles(&b));
        }
    }
}

#[test]
fn a_tip_waits_for_the_welcome_card_and_for_open_dialogs() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    b.app.welcome = true;
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    assert_eq!(b.app.confort.conseil, None);
    assert!(!b.app.settings.conseils.a_vu(Conseil::Montage));
    b.app.welcome = false;
    b.app.pending = Some(crate::app::Pending::Close);
    b.chauffer(3);
    assert_eq!(b.app.confort.conseil, None, "une boîte de dialogue tient la main");
    b.app.pending = None;
    b.chauffer(3);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Montage));
}

#[test]
fn the_five_places_each_have_their_tip() {
    let mut b = banc();
    b.app.set_espace(Espace::Vignettes);
    assert_eq!(b.app.conseil_de_l_endroit(), Some(Conseil::Vignettes));
    b.app.set_espace(Espace::Montage);
    assert_eq!(b.app.conseil_de_l_endroit(), Some(Conseil::Montage));
    b.app.set_espace(Espace::Animation);
    assert_eq!(b.app.conseil_de_l_endroit(), Some(Conseil::Animation));
    b.app.set_espace(Espace::Storyboard);
    assert_eq!(b.app.conseil_de_l_endroit(), None, "un projet intact, dans l'onglet Storyboard : pas de conseil");
    b.app.modified = true;
    assert_eq!(b.app.conseil_de_l_endroit(), Some(Conseil::Raccourcis), "la première modification : Z et R");
    b.app.set_espace3d(true);
    assert_eq!(b.app.conseil_de_l_endroit(), Some(Conseil::Espace3d));
}

#[test]
fn seen_tips_are_remembered_in_the_settings_file_and_revoir_les_conseils_forgets_them() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    let ecrit = toml::to_string(&b.app.settings).unwrap();
    assert!(ecrit.contains("[conseils]") && ecrit.contains("\"montage\"") && ecrit.contains("\"vignettes\""), "{ecrit}");
    let relu: Settings = toml::from_str(&ecrit).unwrap();
    assert!(relu.conseils.a_vu(Conseil::Montage) && relu.conseils.a_vu(Conseil::Vignettes) && !relu.conseils.a_vu(Conseil::Animation));
    // Des réglages d'avant (sans la clé) : rien de vu, et la clé n'est pas écrite quand rien n'est vu.
    let ancien: Settings = toml::from_str("").unwrap();
    assert!(ancien.conseils.est_vide());
    assert!(!toml::to_string(&Settings::default()).unwrap().contains("conseils"));
    // Le fichier réellement écrit sur le disque (sandbox des essais) les a aussi.
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(disque.contains("montage"), "{disque}");
    // « Revoir les conseils » : tout est à revoir, et celui de l'endroit où l'on est revient tout de suite.
    b.app.run_action("aide.conseils");
    assert!(b.app.settings.conseils.est_vide());
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Vignettes), "l'endroit où l'on est");
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(!disque.contains("\"montage\""), "{disque}");
}

#[test]
fn the_preferences_button_revoir_les_conseils_is_in_the_general_tab() {
    use crate::prefsonglets::{groupes_de, Groupe, Onglet, REGLAGES};
    assert!(groupes_de(Onglet::General).contains(&Groupe::Conseils));
    assert!(REGLAGES.iter().any(|r| r.libelle == "Revoir les conseils" && r.onglet == Onglet::General));
    let mut b = banc();
    b.app.settings.prefs_onglet = Onglet::General;
    b.app.settings.conseils.marquer(Conseil::Montage);
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    let bouton = tous_les_textes(&b).into_iter().find(|(t, _)| t == "Revoir les conseils").expect("le bouton").1;
    assert!(est_la(&b, "1 conseil(s) vu(s) sur 5"));
    clic(&mut b, bouton);
    assert!(b.app.settings.conseils.est_vide());
}

#[test]
fn tips_are_off_unless_the_app_was_launched_by_the_user() {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1200, 800);
    assert!(!b.app.confort.conseils_actifs, "les essais n'ont aucune bulle par-dessus l'écran sauf s'ils la demandent");
    b.app.set_espace(Espace::Montage);
    b.chauffer(6);
    assert_eq!(b.app.confort.conseil, None);
    assert!(b.app.settings.conseils.est_vide());
}
