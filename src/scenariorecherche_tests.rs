//! Essais de la recherche du panneau Scénario (`scenariorecherche.rs`) : normalisation (casse, accents), occurrences,
//! compteur, suivant / précédent qui boucle, terme vide, scénario vide, et les touches dans le panneau (Ctrl+F, Échap).

use std::sync::Arc;

use eframe::egui::{self, Event, Key, Modifiers};

use crate::app::App;
use crate::scenario::{parse_fountain, Script};
use crate::scenariogarde::Gardee;
use crate::scenariorecherche::{normaliser, Recherche};

const TEXTE: &str = "Title: L'Été\n\nINT. CUISINE - NUIT\n\nMarie cherche sa clé. Elle cherche encore.\n\nMARIE\n(bas)\nOù est-elle ? Elle est passée en été.\n\nPAUL\nCHERCHE encore, Marie.\n";

fn gardee(texte: &str) -> Gardee {
    Gardee::new("essai.fountain", parse_fountain(texte, &|_| {}), Vec::new())
}

fn ouverte(g: &Gardee, terme: &str) -> Recherche {
    let mut r = Recherche::default();
    r.ouvrir();
    r.terme = terme.to_string();
    r.mettre_a_jour(g);
    r
}

#[test]
fn normalising_ignores_case_and_accents_letter_for_letter() {
    assert_eq!(normaliser("Été"), "ete");
    assert_eq!(normaliser("ÉTÉ"), "ete");
    assert_eq!(normaliser("Où ÇA Ñ œ"), "ou ca n œ");
    assert_eq!(normaliser("a\nb"), "a b");
    for t in ["Été", "Où est-elle ?", "ÀÉÎÕÜ ß œ æ", "l’été"] {
        assert_eq!(normaliser(t).chars().count(), t.chars().count(), "{t} : une lettre pour une lettre");
    }
}

#[test]
fn ete_finds_the_accented_word_whatever_the_case() {
    let g = gardee(TEXTE);
    // « Été » dans la réplique de Marie ; le titre n'est pas dans le texte du panneau.
    for terme in ["ete", "ETE", "Été", "été"] {
        let r = ope(&g, terme);
        assert_eq!(r, 1, "{terme}");
    }
    assert_eq!(ope(&g, "cherche"), 3, "deux dans l'action, une dans la réplique de Paul (en majuscules)");
}

fn ope(g: &Gardee, terme: &str) -> usize {
    ouverte(g, terme).nombre()
}

#[test]
fn the_counter_says_the_rank_or_no_result() {
    let g = gardee(TEXTE);
    let mut r = ouverte(&g, "marie");
    let n = r.nombre();
    assert!(n >= 3);
    assert_eq!(r.compteur(), format!("1 / {n}"));
    r.suivant();
    assert_eq!(r.compteur(), format!("2 / {n}"));
    r.terme = "zzzz".into();
    r.mettre_a_jour(&g);
    assert_eq!(r.compteur(), "Aucun résultat");
    assert!(r.courante().is_none());
    // Sans résultat, suivant et précédent ne font rien (et ne plantent pas).
    r.suivant();
    r.precedent();
    assert_eq!(r.compteur(), "Aucun résultat");
}

#[test]
fn next_and_previous_wrap_around() {
    let g = gardee(TEXTE);
    let mut r = ouverte(&g, "marie");
    let n = r.nombre();
    for _ in 0..n {
        r.suivant();
    }
    assert_eq!(r.courant, 0, "après la dernière, la première");
    r.precedent();
    assert_eq!(r.courant, n - 1, "avant la première, la dernière");
    r.precedent();
    assert_eq!(r.courant, n - 2);
    // Chaque déplacement demande de mener la vue à l'occurrence.
    r.a_defiler = false;
    r.suivant();
    assert!(r.a_defiler);
}

#[test]
fn a_new_term_restarts_at_the_first_occurrence_and_matches_do_not_overlap() {
    let g = gardee(TEXTE);
    let mut r = ouverte(&g, "marie");
    r.suivant();
    r.terme = "elle".into();
    r.mettre_a_jour(&g);
    assert_eq!(r.courant, 0);
    let g2 = gardee("aaaa\n");
    assert_eq!(ope(&g2, "aa"), 2);
}

#[test]
fn an_empty_term_or_an_empty_script_finds_nothing() {
    let g = gardee(TEXTE);
    for terme in ["", "   "] {
        let r = ouverte(&g, terme);
        assert_eq!(r.nombre(), 0);
        assert_eq!(r.compteur(), "");
        assert!(r.de_l_element(0).is_none());
    }
    let vide = Gardee::new("vide", Script::default(), Vec::new());
    let r = ouverte(&vide, "marie");
    assert_eq!(r.nombre(), 0);
    assert_eq!(r.compteur(), "Aucun résultat");
    assert!(r.de_l_element(0).is_none());
}

#[test]
fn closing_clears_the_highlights_but_keeps_the_term_for_the_session() {
    let g = gardee(TEXTE);
    let mut r = ouverte(&g, "marie");
    assert!(r.nombre() > 0);
    r.fermer();
    r.mettre_a_jour(&g);
    assert_eq!(r.nombre(), 0);
    assert!(r.de_l_element(0).is_none());
    assert_eq!(r.compteur(), "");
    assert_eq!(r.terme, "marie");
    r.ouvrir();
    r.mettre_a_jour(&g);
    assert!(r.nombre() > 0, "le terme d'avant est repris");
    assert_eq!(r.courant, 0);
}

#[test]
fn occurrences_are_grouped_by_element_in_script_order() {
    let g = gardee(TEXTE);
    let r = ouverte(&g, "marie");
    let mut vu = 0;
    let mut dernier = 0;
    for i in 0..g.script.elements.len() {
        if let Some((plage, occs)) = r.de_l_element(i) {
            assert_eq!(plage.len(), occs.len());
            for o in occs {
                assert_eq!(o.element, i);
                assert!(o.element >= dernier);
                dernier = o.element;
                vu += 1;
            }
        }
    }
    assert_eq!(vu, r.nombre());
}

fn image(ctx: &egui::Context, app: &mut App, events: Vec<Event>) {
    let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(360.0, 700.0))), events, ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.scenario_panneau(ui));
    });
}

fn touche(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }
}

#[test]
fn ctrl_f_in_the_panel_opens_the_search_and_escape_closes_it() {
    let dir = std::env::temp_dir().join(format!("shot-scenario-recherche-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc.scenario = Some(Arc::new(gardee(TEXTE)));
    let sur_le_panneau = Event::PointerMoved(egui::pos2(100.0, 300.0));
    image(&ctx, &mut app, vec![sur_le_panneau.clone()]);
    assert!(!app.scenario_ui.recherche.ouverte);
    // Pointeur ailleurs : Ctrl+F n'ouvre rien (il reste « Aller à un plan »).
    image(&ctx, &mut app, vec![Event::PointerMoved(egui::pos2(-50.0, -50.0)), touche(Key::F, Modifiers::COMMAND)]);
    assert!(!app.scenario_ui.recherche.ouverte);
    // Pointeur sur le panneau : la recherche s'ouvre, le terme tapé surligne, Échap ferme.
    image(&ctx, &mut app, vec![sur_le_panneau.clone()]);
    image(&ctx, &mut app, vec![touche(Key::F, Modifiers::COMMAND)]);
    assert!(app.scenario_ui.recherche.ouverte);
    assert!(app.scenario_ui.recherche_prise(&ctx), "Ctrl+F est pris par le panneau tant que le pointeur y est");
    app.scenario_ui.recherche.terme = "ete".into();
    image(&ctx, &mut app, vec![]);
    image(&ctx, &mut app, vec![]);
    assert_eq!(app.scenario_ui.recherche.compteur(), "1 / 1");
    image(&ctx, &mut app, vec![touche(Key::Escape, Modifiers::NONE)]);
    assert!(!app.scenario_ui.recherche.ouverte);
    image(&ctx, &mut app, vec![]);
    assert_eq!(app.scenario_ui.recherche.nombre(), 0);
}

#[test]
fn a_long_script_is_searched_without_recomputing_every_frame() {
    let mut texte = String::from("Title: Long\n\n");
    for i in 0..4000 {
        texte.push_str(&format!("INT. LIEU {i} - JOUR\n\nÉlodie marche dans la rue numéro {i}.\n\nÉLODIE\nIl fait beau en été.\n\n"));
    }
    let g = gardee(&texte);
    let t = std::time::Instant::now();
    let mut r = ouverte(&g, "ete");
    assert_eq!(r.nombre(), 4000);
    for _ in 0..1000 {
        r.mettre_a_jour(&g);
    }
    assert!(t.elapsed().as_millis() < 3000, "{} ms", t.elapsed().as_millis());
}
