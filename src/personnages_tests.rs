//! Brosse par personnage (`personnages.rs`, `personnagesui.rs`, `persofond.rs`).

use eframe::egui::{pos2, Color32};

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::fiabilite_fixture::{arbre, dossier, racine};
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::personnages::{self, Brosse, Personnage};
use crate::personnagesui::Choix;
use crate::project;
use crate::render;
use crate::workspace::{Espace, Layout};

// ------------------------------------------------------------------------------------ outils d'essai

fn banc_dessin() -> Banc {
    // Le fond sous le trait se garde dans des caches communs à tous les essais, rangés par numéro de dessin et par version :
    // deux essais de personnages qui tournent ensemble ont les mêmes numéros et les mêmes versions, et se prêtaient
    // alors le fond l'un de l'autre (un fond manquant, un essai sur deux sous charge). L'état commun pour cet essai seul.
    crate::fxcache::monde::ecrire();
    let mut b = banc(Document::new(400, 300, 24), false);
    b.app.tool = Tool::Brush;
    b
}

/// L'état commun remis à zéro pour un essai (`monde::ecrire`) oublie aussi les formes de fond gardées : deux essais ont les mêmes
/// numéros de dessin et les mêmes versions, et l'un retrouvait la forme de fond du dessin de l'autre (« Max a le sien » sans fond,
/// une exécution sur quelques-unes sous charge).
#[test]
fn the_shared_state_reset_forgets_the_kept_fond_shapes_of_the_essais_before() {
    std::thread::spawn(|| {
        crate::fxcache::monde::ecrire();
        crate::fx::garder_une_forme_pour_essai(7, 3);
        assert_eq!(crate::fx::formes_de_fond_gardees(), 1);
    })
    .join()
    .expect("le premier essai");
    std::thread::spawn(|| {
        crate::fxcache::monde::ecrire();
        assert_eq!(crate::fx::formes_de_fond_gardees(), 0, "page blanche : plus de forme d'un essai d'avant");
    })
    .join()
    .expect("le second essai");
}

/// Un anneau carré de `x0, y0` à `x1, y1`, tracé au pinceau comme une main : un seul trait fermé.
fn anneau(app: &mut App, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut pts = Vec::new();
    let step = 4.0;
    let mut x = x0;
    while x < x1 {
        pts.push(pos2(x, y0));
        x += step;
    }
    let mut y = y0;
    while y < y1 {
        pts.push(pos2(x1, y));
        y += step;
    }
    let mut x = x1;
    while x > x0 {
        pts.push(pos2(x, y1));
        x -= step;
    }
    let mut y = y1;
    while y > y0 - 2.0 * step {
        pts.push(pos2(x0, y));
        y -= step;
    }
    app.begin_paint(pts[0], 1.0, false);
    for p in &pts[1..] {
        app.stroke_to(*p, 1.0);
    }
    app.end_stroke();
}

/// La couleur vue à l'écran en (x, y), le fond sous le trait compris.
fn vu(app: &App, x: usize, y: usize) -> Color32 {
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    render::composite_pixel(&app.doc, &plan, x, y)
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

const ROUGE: [u8; 3] = [200, 30, 30];
const ROSE: [u8; 3] = [250, 200, 200];
const BLEU: [u8; 3] = [30, 40, 200];
const CIEL: [u8; 3] = [190, 210, 250];

/// Deux personnages : Lou (trait rouge, fond rose) et Max (trait bleu, fond bleu ciel).
fn deux_personnages(app: &mut App) -> (u64, u64) {
    let a = app.perso_ajouter().expect("premier");
    app.perso_ecrire(Choix::Personnage(a), "Lou".into(), ROUGE, Some(ROSE));
    let b = app.perso_ajouter().expect("second");
    app.perso_ecrire(Choix::Personnage(b), "Max".into(), BLEU, Some(CIEL));
    (a, b)
}

fn etapes(app: &App) -> usize {
    app.history.names().0.len()
}

// ------------------------------------------------------------------------------------ le modèle

#[test]
fn characters_can_be_added_edited_deleted_and_reordered_with_undo() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let avant = etapes(app);
    let (a, c) = deux_personnages(app);
    assert_ne!(a, c, "des numéros différents");
    assert_eq!(app.doc.personnages.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Lou", "Max"]);
    assert_eq!(app.doc.personnages[0].fill, Some(ROSE));
    // Ajouter = une étape d'annulation chacun (la couleur et le nom, eux, sont posés sans étape ici : le champ
    // de l'interface enregistre la sienne, `App::edited`).
    assert_eq!(etapes(app), avant + 2);
    assert!(app.history.names().0.last().is_some_and(|n| n.contains("Nouveau personnage")), "{:?}", app.history.names().0);
    // Ranger : Max passe avant Lou, une étape, annulable.
    app.perso_deplacer(Choix::Personnage(c), -1);
    assert_eq!(app.doc.personnages[0].name, "Max");
    assert_eq!(etapes(app), avant + 3);
    app.undo();
    assert_eq!(app.doc.personnages[0].name, "Lou");
    app.redo();
    assert_eq!(app.doc.personnages[0].name, "Max");
    // Un personnage déjà au bout ne bouge pas (et n'enregistre rien).
    let n = etapes(app);
    app.perso_deplacer(Choix::Personnage(c), -1);
    assert_eq!(etapes(app), n);
    // Dupliquer : juste après, avec un nom de copie et un numéro neuf.
    app.perso_dupliquer(Choix::Personnage(a));
    assert_eq!(app.doc.personnages.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Max", "Lou", "Lou (copie)"]);
    let ids: std::collections::HashSet<u64> = app.doc.personnages.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(app.doc.personnages[2].line, ROUGE);
    // Supprimer, puis annuler.
    app.perso_supprimer(Choix::Personnage(a));
    assert_eq!(app.doc.personnages.len(), 2);
    app.undo();
    assert_eq!(app.doc.personnages.len(), 3);
    // Modifier le nom, le trait et le fond.
    app.perso_ecrire(Choix::Personnage(a), "Léa".into(), ROUGE, None);
    assert_eq!(app.doc.personnages[1].fill, None, "sans fond");
}

#[test]
fn a_character_name_is_clean_and_limited() {
    assert_eq!(personnages::nom_net("  Un nom  "), "Un nom");
    assert_eq!(personnages::nom_net(&"x".repeat(100)).chars().count(), personnages::NOM_MAX);
    let liste = vec![Personnage { id: 1, name: "Personnage 2".into(), line: [0; 3], fill: None }];
    assert_eq!(personnages::nom_neuf(&liste), "Personnage 3", "pas deux fois le même nom");
    assert_eq!(personnages::nom_de_copie(&liste, "Léa"), "Léa (copie)");
    let deux = vec![Personnage { id: 1, name: "Léa".into(), line: [0; 3], fill: None }, Personnage { id: 2, name: "Léa (copie)".into(), line: [0; 3], fill: None }];
    assert_eq!(personnages::nom_de_copie(&deux, "Léa (copie)"), "Léa (copie 2)");
    let mut v = vec![1, 2, 3];
    assert_eq!(personnages::deplacer(&mut v, 0, 1), Some(1));
    assert_eq!(v, [2, 1, 3]);
    assert_eq!(personnages::deplacer(&mut v, 2, 5), None, "déjà au bout");
    assert_eq!(personnages::deplacer(&mut v, 9, 1), None, "rang inconnu");
}

// ------------------------------------------------------------------------------ choisir, dessiner

#[test]
fn choosing_a_character_sets_the_line_colour_and_the_stroke_gets_the_fill_at_once() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, _) = deux_personnages(app);
    app.tool = Tool::Eraser;
    app.perso_choisir(Choix::Personnage(lou));
    assert_eq!(app.color, ROUGE, "la couleur du trait est celle du personnage");
    assert_eq!(app.tool, Tool::Brush, "un outil qui ne dessine pas cède la place au pinceau");
    assert!(app.layers()[0].fond.is_none(), "pas encore de fond");
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    assert!(app.layers()[0].fond.is_some(), "le calque a pris un fond sous le trait");
    app.perso_attendre();
    assert_eq!(vu(app, 110, 110), rgb(ROSE), "le dedans a le fond du personnage");
    assert_eq!(vu(app, 60, 110).a(), 255, "le trait est là");
    assert_eq!(vu(app, 60, 110), rgb(ROUGE), "de la couleur du personnage");
    assert_eq!(vu(app, 300, 250).a(), 0, "dehors : rien");
}

#[test]
fn two_characters_on_the_same_layer_keep_their_own_fills() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, max) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 40.0, 60.0, 140.0, 160.0);
    app.perso_attendre();
    app.perso_choisir(Choix::Personnage(max));
    anneau(app, 220.0, 60.0, 320.0, 160.0);
    app.perso_attendre();
    assert_eq!(app.layers().len(), 1, "un seul calque pour les deux");
    assert_eq!(vu(app, 90, 110), rgb(ROSE), "Lou garde son fond");
    assert_eq!(vu(app, 270, 110), rgb(CIEL), "Max a le sien");
    assert_eq!(vu(app, 40, 110), rgb(ROUGE));
    assert_eq!(vu(app, 220, 110), rgb(BLEU));
    // Lou dessine encore, à côté : le fond de Max ne bouge pas.
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 160.0, 200.0, 240.0, 270.0);
    app.perso_attendre();
    assert_eq!(vu(app, 200, 235), rgb(ROSE));
    assert_eq!(vu(app, 270, 110), rgb(CIEL));
}

#[test]
fn a_character_without_fill_cuts_the_fond_and_a_neutral_without_fond_adds_none() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    // Noir, sans fond (une couleur neutre réglée ainsi : celles d'origine ont toutes un aplat) sur un calque sans fond : aucun fond
    // n'apparaît.
    app.settings.brosses_classiques[0] = Brosse::classique("Noir", [0, 0, 0], None);
    app.perso_choisir(Choix::Classique(0));
    assert_eq!(app.color, [0, 0, 0]);
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    app.perso_attendre();
    assert!(app.layers()[0].fond.is_none(), "un neutre sans fond n'allume pas le fond");
    assert_eq!(vu(app, 110, 110).a(), 0, "on voit à travers");
    // Le gris a un fond, clair.
    app.perso_choisir(Choix::Classique(1));
    anneau(app, 220.0, 60.0, 320.0, 160.0);
    app.perso_attendre();
    let gris = app.settings.brosses_classiques[1].fill.expect("fond du gris");
    assert_eq!(vu(app, 270, 110), rgb(gris));
    assert_eq!(vu(app, 110, 110).a(), 0, "le noir n'a toujours pas de fond");
    // Un personnage sans fond, sur ce même calque à fond : le fond gagné est coupé.
    let p = app.perso_ajouter().expect("personnage");
    app.perso_ecrire(Choix::Personnage(p), "Fantôme".into(), BLEU, None);
    app.perso_choisir(Choix::Personnage(p));
    anneau(app, 60.0, 190.0, 160.0, 280.0);
    app.perso_attendre();
    assert_eq!(vu(app, 110, 235).a(), 0, "sans fond : on voit à travers");
    assert_eq!(vu(app, 270, 110), rgb(gris), "le fond du gris n'a pas bougé");
}

#[test]
fn strokes_faster_than_the_background_thread_all_get_their_fill_and_undo_stays_sane() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, max) = deux_personnages(app);
    // Trois traits de suite, sans attendre le fil de fond entre eux.
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 40.0, 60.0, 140.0, 160.0);
    app.perso_choisir(Choix::Personnage(max));
    anneau(app, 220.0, 60.0, 320.0, 160.0);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 160.0, 200.0, 240.0, 270.0);
    app.perso_attendre();
    assert_eq!(vu(app, 90, 110), rgb(ROSE));
    assert_eq!(vu(app, 270, 110), rgb(CIEL));
    assert_eq!(vu(app, 200, 235), rgb(ROSE));
    // Défaire le dernier trait emporte son fond (et lui seul).
    app.undo();
    assert_eq!(vu(app, 200, 235).a(), 0, "le dernier trait et son fond sont défaits");
    assert_eq!(vu(app, 90, 110), rgb(ROSE));
    assert_eq!(vu(app, 270, 110), rgb(CIEL));
    // Le rétablir le rend avec son fond.
    app.redo();
    assert_eq!(vu(app, 200, 235), rgb(ROSE));
}

#[test]
fn the_fill_of_a_character_stroke_is_saved_with_the_project() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, max) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 40.0, 60.0, 140.0, 160.0);
    app.perso_choisir(Choix::Personnage(max));
    anneau(app, 220.0, 60.0, 320.0, 160.0);
    app.perso_attendre();
    let manifeste = enregistre("personnages-fonds", &app.doc);
    let l = project::load(&manifeste).expect("relecture");
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.doc.personnages, app.doc.personnages);
    let plan = render::panel_plan_at(&l.doc, 0, 0, None);
    assert_eq!(render::composite_pixel(&l.doc, &plan, 90, 110), rgb(ROSE), "le fond de Lou revient");
    assert_eq!(render::composite_pixel(&l.doc, &plan, 270, 110), rgb(CIEL), "et celui de Max");
}

#[test]
fn a_character_stroke_and_its_fill_are_one_undo_step() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, _) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    // (Le fond du calque est une étape à part, posée avant le trait ; le trait et son fond sont une seule étape.)
    app.perso_attendre();
    let n = etapes(app);
    assert_eq!(vu(app, 110, 110), rgb(ROSE));
    app.undo();
    assert_eq!(etapes(app), n - 1, "une étape défaite");
    assert_eq!(vu(app, 60, 110).a(), 0, "le trait n'est plus là");
    assert_eq!(vu(app, 110, 110).a(), 0, "ni son fond");
    app.redo();
    assert_eq!(vu(app, 60, 110), rgb(ROUGE));
    assert_eq!(vu(app, 110, 110), rgb(ROSE), "le fond revient avec le trait");
}

#[test]
fn the_eraser_and_the_fond_stratum_are_not_touched_by_a_character() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, _) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    app.perso_attendre();
    let avant = vu(app, 110, 110);
    // La gomme n'enregistre aucun fond de personnage.
    app.tool = Tool::Eraser;
    app.begin_paint(pos2(50.0, 110.0), 1.0, false);
    app.stroke_to(pos2(70.0, 110.0), 1.0);
    app.end_stroke();
    assert_eq!(app.perso.fil.as_ref().map(|f| f.en_cours()), Some(0), "rien à faire pour la gomme");
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)), "elle ne lâche pas le personnage");
    let cel = app.layers()[0].frames[0].expect("dessin");
    assert_eq!(app.doc.cels[&cel].raster.get(60, 110).a(), 0, "la gomme a effacé le trait");
    app.perso_attendre();
    assert_eq!(vu(app, 110, 110), avant, "le fond du dedans n'est pas repeint");
}

#[test]
fn a_vector_layer_gets_the_fill_too() {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers[0] = Layer::new("Encre", LayerKind::Vector);
    crate::fxcache::monde::ecrire();
    let mut b = banc(doc, false);
    b.app.tool = Tool::Brush;
    let app = &mut b.app;
    let (lou, _) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    app.perso_attendre();
    assert!(app.layers()[0].fond.is_some());
    assert_eq!(vu(app, 110, 110), rgb(ROSE), "le dedans d'un trait vectoriel a le fond du personnage");
}

#[test]
fn the_fill_tool_in_the_fond_stratum_uses_the_fill_colour_of_the_character() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, max) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    app.perso_attendre();
    app.perso_choisir(Choix::Personnage(max));
    assert_eq!(app.perso_fond_ou([1, 2, 3]), CIEL);
    app.aplat_cible = Some(app.layer().id);
    app.tool = Tool::Fill;
    app.fond_zone_at(pos2(110.0, 110.0), false);
    assert_eq!(vu(app, 110, 110), rgb(CIEL), "le pot de peinture du fond prend le fond de Max");
    app.perso_lacher();
    assert_eq!(app.perso_fond_ou([1, 2, 3]), [1, 2, 3], "sans personnage, la couleur courante");
}

// ------------------------------------------------------------------------------ couleur à la main

#[test]
fn picking_a_colour_by_hand_lets_the_character_go() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, max) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    app.tick_personnages();
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)), "rien n'a changé : il reste choisi");
    // Le panneau Couleur, un nuancier, un échange des couleurs.
    app.set_front([10, 20, 30]);
    app.tick_personnages();
    assert_eq!(app.perso.actif, None);
    assert_eq!(app.color, [10, 20, 30], "la couleur choisie est gardée");
    // La pipette écrit la couleur directement.
    app.perso_choisir(Choix::Personnage(max));
    app.color = [1, 2, 3];
    app.tick_personnages();
    assert_eq!(app.perso.actif, None, "la pipette lâche le personnage");
    // Un trait commencé avant l'image suivante est un trait libre, sans fond.
    app.perso_choisir(Choix::Personnage(lou));
    app.color = [9, 9, 9];
    anneau(app, 60.0, 60.0, 160.0, 160.0);
    app.perso_attendre();
    assert!(app.layers()[0].fond.is_none(), "une couleur libre n'allume pas de fond");
    assert_eq!(app.perso.actif, None);
}

#[test]
fn a_chosen_character_follows_its_colour_when_it_is_edited_and_goes_when_deleted() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, _) = deux_personnages(app);
    app.perso_choisir(Choix::Personnage(lou));
    app.perso_ecrire(Choix::Personnage(lou), "Lou".into(), [0, 120, 0], Some(ROSE));
    app.tick_personnages();
    assert_eq!(app.color, [0, 120, 0], "le pinceau suit la couleur du personnage");
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)), "et il reste choisi");
    app.undo();
    app.tick_personnages();
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)));
    app.perso_supprimer(Choix::Personnage(lou));
    app.tick_personnages();
    assert_eq!(app.perso.actif, None);
}

#[test]
fn next_and_previous_walk_through_the_characters_then_the_neutrals() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let (lou, max) = deux_personnages(app);
    app.perso_lacher();
    app.run_action("perso.suivant");
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)));
    app.run_action("perso.suivant");
    assert_eq!(app.perso.actif, Some(Choix::Personnage(max)));
    app.run_action("perso.suivant");
    assert_eq!(app.perso.actif, Some(Choix::Classique(0)), "après les personnages, les neutres");
    app.run_action("perso.precedent");
    assert_eq!(app.perso.actif, Some(Choix::Personnage(max)));
    app.run_action("perso.aucun");
    assert_eq!(app.perso.actif, None);
}

// ------------------------------------------------------------------------------ les couleurs neutres

#[test]
fn the_neutral_colours_are_app_settings_with_sensible_defaults() {
    let d = personnages::classiques_par_defaut();
    assert_eq!(d[0], Brosse::classique("Gris 1", [0x1E; 3], Some([0x8C; 3])));
    assert!(d.iter().all(|n| n.fill.is_some()), "les trois gris ont un aplat plus clair que leur trait");
    assert!(d.len() <= personnages::MAX_CLASSIQUES);
    let mut s = crate::settings::Settings::default();
    assert_eq!(s.brosses_classiques, d);
    s.brosses_classiques.push(Brosse::classique("Vert", [0, 128, 0], Some([200, 255, 200])));
    s.brosses_classiques.remove(0);
    let text = toml::to_string(&s).expect("TOML");
    let back: crate::settings::Settings = toml::from_str(&text).expect("relecture");
    assert_eq!(back.brosses_classiques, s.brosses_classiques, "les couleurs neutres reviennent telles quelles");
    // Un ancien fichier de réglages, sans la clé : les couleurs d'origine.
    let ancien: crate::settings::Settings = toml::from_str("").expect("réglages vides");
    assert_eq!(ancien.brosses_classiques, d);
    // Une liste vide reste vide ; trop longue, elle est coupée ; les noms sont nets.
    assert!(personnages::brosses_nettoyees(Vec::new(), personnages::MAX_CLASSIQUES).is_empty());
    let trop: Vec<Brosse> = (0..20).map(|k| Brosse::classique(&format!("  n{k}  "), [0; 3], None)).collect();
    let net = personnages::brosses_nettoyees(trop, personnages::MAX_CLASSIQUES);
    assert_eq!((net.len(), net[0].name.as_str()), (personnages::MAX_CLASSIQUES, "n0"));
}

#[test]
fn neutral_colours_can_be_added_edited_moved_and_deleted_and_the_choice_follows() {
    let mut b = banc_dessin();
    let app = &mut b.app;
    let n0 = app.settings.brosses_classiques.len();
    app.perso_choisir(Choix::Classique(1));
    app.perso_deplacer(Choix::Classique(1), -1);
    assert_eq!(app.perso.actif, Some(Choix::Classique(0)), "le choix suit sa couleur");
    assert_eq!(app.settings.brosses_classiques[0].name, "Gris 2");
    app.perso_supprimer(Choix::Classique(0));
    assert_eq!((app.perso.actif, app.settings.brosses_classiques.len()), (None, n0 - 1));
    let i = app.perso_ajouter_classique().expect("ajout");
    app.perso_ecrire(Choix::Classique(i), "Sépia".into(), [112, 66, 20], Some([240, 220, 190]));
    assert_eq!(app.settings.brosses_classiques[i].name, "Sépia");
    app.perso_dupliquer(Choix::Classique(i));
    assert_eq!(app.settings.brosses_classiques[i + 1].name, "Sépia (copie)");
    assert!(app.settings_a_ecrire, "à écrire dans les réglages");
    // Les neutres ne touchent pas à l'historique du projet.
    assert!(!app.history.names().0.iter().any(|n| n.contains("neutre")));
    while app.perso_ajouter_classique().is_some() {}
    assert_eq!(app.settings.brosses_classiques.len(), personnages::MAX_CLASSIQUES);
}

// ------------------------------------------------------------------------------ la barre et le panneau

#[test]
fn the_bar_is_shown_per_workspace_with_the_chosen_defaults() {
    assert!(Espace::Storyboard.barre_personnages_par_defaut());
    assert!(!Espace::Animation.barre_personnages_par_defaut());
    assert!(!Espace::Montage.barre_personnages_par_defaut());
    let mut b = banc_dessin();
    let app = &mut b.app;
    assert_eq!(app.espace, Espace::Storyboard);
    assert!(app.barre_perso_visible(), "visible d'office en storyboard");
    app.run_action("perso.barre");
    assert!(!app.barre_perso_visible(), "cachée");
    assert_eq!(app.layout.character_bar, Some(false));
    app.set_espace(Espace::Animation);
    assert!(!app.barre_perso_visible(), "cachée d'office en animation");
    app.run_action("perso.barre");
    assert!(app.barre_perso_visible(), "montrée à la demande en animation");
    app.set_espace(Espace::Montage);
    assert!(!app.barre_perso_visible());
    // Chaque espace garde son choix : le storyboard est resté caché, l'animation montrée.
    app.set_espace(Espace::Storyboard);
    assert!(!app.barre_perso_visible());
    app.set_espace(Espace::Animation);
    assert!(app.barre_perso_visible());
    // Le choix est dans la disposition retenue (réglages), et un ancien fichier sans la clé suit l'espace.
    let l = app.current_layout();
    assert_eq!(l.character_bar, Some(true));
    let text = serde_json::to_string(&l).expect("JSON");
    assert!(text.contains("barre_personnages"));
    let vieux: Layout = serde_json::from_str("{\"outils\": true}").expect("ancienne disposition");
    assert_eq!(vieux.character_bar, None);
    assert!(!serde_json::to_string(&Layout::default()).expect("JSON").contains("barre_personnages"), "rien d'écrit tant qu'on n'a pas choisi");
}

#[test]
fn the_bar_and_the_panel_are_drawn_and_the_application_rests() {
    crate::fxcache::monde::ecrire();
    let mut b = banc_dessin();
    deux_personnages(&mut b.app);
    b.app.perso_choisir(Choix::Classique(1));
    b.garder_textes = true;
    b.chauffer(4);
    assert!(b.app.barre_perso_visible());
    // Le panneau, en onglet avec la couleur dans la disposition par défaut du storyboard.
    assert!(b.app.dock_layout().contains(crate::dock::Pane::Personnages), "le panneau est dans la disposition du storyboard");
    b.app.show_pane(crate::dock::Pane::Personnages);
    b.chauffer(4);
    assert!(b.textes.iter().any(|t| t.contains("Classiques")) && b.textes.iter().any(|t| t.contains("Décors")), "le panneau est dessiné : {:?}", b.textes.iter().take(40).collect::<Vec<_>>());
    // La fenêtre « Modifier ».
    let lou = b.app.doc.personnages[0].id;
    b.app.perso.edition = Some(Choix::Personnage(lou));
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t.contains("Modifier la brosse personnage")));
    b.app.perso.edition = Some(Choix::Classique(0));
    b.chauffer(3);
    assert!(b.textes.iter().any(|t| t.contains("Modifier la brosse classique")));
    b.app.perso.edition = None;
    // Au repos, rien ne demande de repeindre.
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose avec la barre et le panneau");
    let mesures: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = mesures.iter().map(|m| m.reveil).min().unwrap_or(std::time::Duration::MAX);
    assert!(pire >= std::time::Duration::from_secs(1), "au repos, SHOT demande à être repeint dans {pire:?}");
    // Après un trait de personnage, il se repose aussi (le fil de fond réveille l'écran une fois).
    let lou_choix = Choix::Personnage(lou);
    b.app.perso_choisir(lou_choix);
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose après un trait de personnage");
    b.app.perso_attendre();
    assert_eq!(vu(&b.app, 110, 110), rgb(ROSE));
}

#[test]
fn the_right_click_menu_actions_reach_the_same_commands() {
    let ids = ["vue.personnages", "perso.barre", "perso.ajouter", "perso.suivant", "perso.precedent", "perso.aucun"];
    for id in ids {
        assert!(crate::keymap::action_info(id).is_some(), "{id} manque au fichier des actions");
    }
    let commandes = crate::palette::palette_commands();
    for id in ["perso.barre", "perso.ajouter", "vue.personnages"] {
        assert!(commandes.iter().any(|(c, _)| *c == id), "{id} manque à la palette");
    }
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.personnages"), Some(crate::dock::Pane::Personnages));
    assert_eq!(serde_json::to_string(&crate::dock::Pane::Personnages).expect("JSON"), "\"personnages\"");
    let mut b = banc_dessin();
    b.app.run_action("perso.ajouter");
    assert_eq!(b.app.doc.personnages.len(), 1);
    assert!(b.app.perso.edition.is_none(), "un nouveau personnage n'ouvre plus la fenêtre « Modifier » (Anthony, 2026-10-28)");
    assert!(b.app.perso.actif.is_some(), "et il est choisi");
}

// ------------------------------------------------------------------------------------ le fichier

fn doc_avec_personnages() -> Document {
    let mut doc = Document::new(64, 48, 24);
    for (k, (nom, fond)) in [("Lou", Some(ROSE)), ("Max", None)].into_iter().enumerate() {
        let id = doc.alloc();
        doc.personnages.push(Personnage { id, name: nom.into(), line: if k == 0 { ROUGE } else { BLEU }, fill: fond });
    }
    doc
}

fn enregistre(nom: &str, doc: &Document) -> std::path::PathBuf {
    let travail = dossier(nom);
    let manifeste = travail.join("Film").join("Film.pelure");
    project::save(doc, &manifeste, &mut Default::default()).expect("enregistrement");
    manifeste
}

#[test]
fn the_characters_round_trip_through_the_project_file() {
    let doc = doc_avec_personnages();
    let manifeste = enregistre("personnages-aller-retour", &doc);
    let dir = racine(&manifeste).to_path_buf();
    let tree = arbre(&dir);
    assert!(tree.contains_key(personnages::FILE), "{:?}", tree.keys().collect::<Vec<_>>());
    let manifest_text = std::fs::read_to_string(&manifeste).expect("manifeste");
    assert!(!manifest_text.contains("Lou"), "les personnages sont dans leur fichier, pas dans le manifeste");
    let mut l = project::load(&manifeste).expect("relecture");
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.doc.personnages, doc.personnages, "mêmes personnages, même ordre, mêmes couleurs");
    // Réenregistrer ne change rien, octet pour octet.
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    assert_eq!(arbre(&dir), tree);
    // Un numéro neuf ne reprend pas ceux des personnages.
    let neuf = l.doc.alloc();
    assert!(l.doc.personnages.iter().all(|p| p.id != neuf));
}

#[test]
fn a_project_without_characters_writes_no_file_and_deleting_them_all_removes_it() {
    let manifeste = enregistre("personnages-aucun", &Document::new(64, 48, 24));
    assert!(!arbre(racine(&manifeste)).contains_key(personnages::FILE), "pas de personnage, pas de fichier");
    let manifeste = enregistre("personnages-tous-retires", &doc_avec_personnages());
    let mut l = project::load(&manifeste).expect("lecture");
    l.doc.personnages.clear();
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let tree = arbre(racine(&manifeste));
    assert!(!tree.contains_key(personnages::FILE) && !tree.keys().any(|k| k.ends_with(".illisible")), "{:?}", tree.keys().collect::<Vec<_>>());
}

#[test]
fn an_unreadable_or_newer_characters_file_is_reported_and_never_destroyed() {
    for newer in [true, false] {
        let manifeste = enregistre(if newer { "personnages-recent" } else { "personnages-abime" }, &doc_avec_personnages());
        let dir = racine(&manifeste).to_path_buf();
        let path = dir.join(personnages::FILE);
        if newer {
            let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).expect("lecture")).expect("JSON");
            v["version"] = serde_json::Value::from(2);
            v["champ_d_une_version_future"] = serde_json::Value::from("à garder");
            std::fs::write(&path, serde_json::to_vec_pretty(&v).expect("JSON")).expect("écriture");
        } else {
            let bytes = std::fs::read(&path).expect("lecture");
            std::fs::write(&path, &bytes[..bytes.len() / 2]).expect("écriture");
        }
        let avant = arbre(&dir);
        let mut l = project::load(&manifeste).expect("le projet s'ouvre sans ses personnages");
        assert_eq!(l.warnings.len(), 1, "{:#?}", l.warnings);
        assert!(l.warnings[0].contains(personnages::FILE) && (!newer || l.warnings[0].contains("plus récente")), "{:?}", l.warnings);
        assert!(l.doc.personnages.is_empty(), "rien n'est lu d'un fichier que cette version ne sait pas lire");
        assert_eq!(arbre(&dir), avant, "ouvrir ne touche à aucun fichier");
        // Enregistrer par-dessus (sans personnage) : le fichier reste tel quel.
        project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
        assert_eq!(arbre(&dir).get(personnages::FILE), avant.get(personnages::FILE), "le fichier illisible a été effacé ou réécrit");
        // Un personnage neuf : son fichier est écrit, l'ancien est mis de côté.
        let id = l.doc.alloc();
        l.doc.personnages.push(Personnage { id, name: "Neuf".into(), line: ROUGE, fill: None });
        project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
        let relu = project::load(&manifeste).expect("relecture");
        assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
        assert_eq!(relu.doc.personnages.len(), 1);
        assert!(dir.join(format!("{}.illisible", personnages::FILE)).exists(), "l'ancien fichier est gardé à côté");
    }
}

#[test]
fn the_characters_file_is_read_defensively() {
    let ok = personnages::parse("{\"format\":\"shot-personnages\",\"version\":1,\"personnages\":[{\"id\":4,\"nom\":\"A\",\"trait\":[1,2,3]},{\"id\":4,\"nom\":\"  B  \",\"trait\":[4,5,6],\"fond\":[7,8,9]},{\"id\":0,\"nom\":\"C\",\"trait\":[0,0,0]}]}").expect("lisible");
    assert_eq!(ok.len(), 3);
    assert_eq!(ok[0].fill, None, "pas de fond");
    assert_eq!(ok[1].name, "B");
    let ids: std::collections::HashSet<u64> = ok.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), 3, "les numéros doubles ou nuls sont refaits : {ids:?}");
    assert!(ids.iter().all(|&i| i != 0));
    assert!(personnages::parse("{\"format\":\"autre\",\"version\":1,\"personnages\":[]}").is_err());
    assert!(personnages::parse("pas du JSON").is_err());
    let trop: Vec<String> = (1..=100).map(|k| format!("{{\"id\":{k},\"nom\":\"p{k}\",\"trait\":[0,0,0]}}")).collect();
    let coupe = personnages::parse(&format!("{{\"format\":\"shot-personnages\",\"version\":1,\"personnages\":[{}]}}", trop.join(","))).expect("lisible");
    assert_eq!(coupe.len(), personnages::MAX_PERSONNAGES, "un fichier abîmé est coupé");
}

#[test]
fn the_background_save_job_writes_the_characters() {
    let doc = doc_avec_personnages();
    let travail = dossier("personnages-fil");
    let manifeste = travail.join("Film").join("Film.pelure");
    let mut written = Default::default();
    let job = project::prepare_save(&doc, &manifeste, &mut written, None).expect("cliché");
    // Les personnages changent pendant l'écriture : le fichier est celui du cliché.
    let mut doc = doc;
    doc.personnages.clear();
    std::thread::spawn(move || job.run_with(None, 2)).join().expect("fil").expect("écriture");
    let l = project::load(&manifeste).expect("relecture");
    assert_eq!(l.doc.personnages.len(), 2);
    assert!(!arbre(racine(&manifeste)).keys().any(|k| k.ends_with(".tmp")));
}

#[test]
fn the_characters_are_part_of_the_structure_so_undo_restores_them() {
    let doc = doc_avec_personnages();
    let mut d2 = Document::new(64, 48, 24);
    let s = doc.structure();
    assert_eq!(s.personnages.len(), 2);
    let ancien = d2.replace_structure(s);
    assert!(ancien.personnages.is_empty());
    assert_eq!(d2.personnages.len(), 2);
    // Les numéros des personnages comptent dans ceux du document : un numéro neuf ne les reprend pas.
    let max = doc.personnages.iter().map(|p| p.id).max().unwrap_or(0);
    let mut relu = Document::from_parts(64, 48, doc.structure(), Default::default(), Default::default());
    assert!(relu.alloc() > max);
}

// ------------------------------------------------------------------------------ le fond d'un trait

#[test]
fn the_pixels_a_stroke_gains_are_only_the_new_ones() {
    let avant = [0u8, 0, 255, 128, 0];
    let apres = [0u8, 255, 255, 255, 0];
    assert_eq!(crate::persofond::pixels_gagnes(&avant, &apres), [false, true, false, true, false]);
}
