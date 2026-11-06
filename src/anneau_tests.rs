//! L'anneau sous le stylet (`anneau.rs`, `anneauui.rs` ; Anthony, 2026-11-06) : le geste sans interface, puis dans
//! l'application entière. Le clic bref garde la fenêtre des réglages (`reglagesdroit_tests.rs` le vérifie toujours).

use eframe::egui::{epaint::Shape, pos2, vec2, Event, Modifiers, PointerButton, Pos2, Vec2};

use crate::anneau::{self, commandes_d_origine, direction, ecart, poser, Arc, Geste, Issue, Reglages, PARTS, PASTILLES};
use crate::app::Tool;
use crate::document::Document;
use crate::monpanneau::commande_connue;
use crate::perf_tests::{banc_taille, jusqu_au_repos, Banc};
use crate::settings::Settings;

const DT: f64 = 1.0 / 60.0;
/// Les parts, par leur nom.
const DROITE: usize = 0;
const BAS: usize = 2;
const GAUCHE: usize = 4;
const HAUT_GAUCHE: usize = 5;
const HAUT: usize = 6;

// ------------------------------------------------------------------------------------------------ le geste, sans interface

#[test]
fn the_eight_directions_turn_clockwise_from_the_right() {
    let attendues = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0)];
    for (part, (x, y)) in attendues.into_iter().enumerate() {
        assert_eq!(direction(vec2(x, y) * 50.0), part, "({x}, {y})");
        // La distance ne compte pas.
        assert_eq!(direction(vec2(x, y) * 900.0), part);
    }
    // Une part va de 22,5° avant son milieu à 22,5° après.
    assert_eq!(direction(vec2(100.0, 40.0)), DROITE);
    assert_eq!(direction(vec2(100.0, -40.0)), DROITE);
    assert_eq!(direction(vec2(100.0, 43.0)), 1);
    assert_eq!(anneau::NOMS_DIRECTIONS[HAUT], "Haut");
    assert_eq!(anneau::NOMS_DIRECTIONS[HAUT_GAUCHE], "Haut gauche");
    // La grille des Préférences passe une fois par chaque part.
    let mut grille = anneau::GRILLE;
    grille.sort_unstable();
    assert_eq!(grille, [0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn angle_gaps_wrap_around_the_circle() {
    assert_eq!(ecart(10.0, 350.0), 20.0);
    assert_eq!(ecart(350.0, 10.0), -20.0);
    assert_eq!(ecart(-135.0, 225.0), 0.0);
    assert_eq!(ecart(90.0, 270.0), -180.0);
}

fn geste() -> Geste {
    Geste::nouveau(pos2(500.0, 400.0), 10.0)
}

/// Cinq pastilles sur la part en haut à gauche, comme dans la maquette.
const CINQ: Option<(usize, usize)> = Some((HAUT_GAUCHE, 5));

fn vers(g: &mut Geste, d: Vec2, pastilles: Option<(usize, usize)>) {
    let p = g.centre + d;
    g.bouger(p, pastilles);
}

#[test]
fn a_brief_click_keeps_the_centre_and_a_flick_launches_its_direction_without_showing_anything() {
    let mut g = geste();
    assert_eq!(g.lacher(), Issue::Centre, "un clic sans bouger");
    // Un tremblement de la main reste au centre.
    let mut g = geste();
    vers(&mut g, vec2(12.0, -9.0), CINQ);
    assert_eq!(g.survol, None);
    assert_eq!(g.lacher(), Issue::Centre);
    // Un geste vif : la commande, sans que l'anneau se soit dessiné.
    let mut g = geste();
    vers(&mut g, vec2(30.0, 0.0), CINQ);
    vers(&mut g, vec2(60.0, 4.0), CINQ);
    assert!(!g.temps(10.05, 0.18), "avant le délai, rien ne se dessine");
    assert!(!g.affiche);
    assert_eq!(g.lacher(), Issue::Part(DROITE));
    // Revenir au centre avant de relâcher : le centre.
    let mut g = geste();
    vers(&mut g, vec2(0.0, 70.0), CINQ);
    assert_eq!(g.survol, Some(BAS));
    vers(&mut g, vec2(0.0, 6.0), CINQ);
    assert_eq!(g.lacher(), Issue::Centre);
}

#[test]
fn holding_shows_the_ring_after_the_delay_and_only_once() {
    let mut g = geste();
    assert!((g.attente(10.0, 0.18) - 0.18).abs() < 1e-9);
    assert!(!g.temps(10.1, 0.18));
    assert!((g.attente(10.1, 0.18) - 0.08).abs() < 1e-9);
    assert!(g.temps(10.2, 0.18), "le délai écoulé : il apparaît");
    assert!(g.affiche);
    assert!(!g.temps(10.5, 0.18), "une seule fois");
    assert_eq!(g.attente(10.5, 0.18), 0.0);
    // Un délai nul : tout de suite.
    let mut g = geste();
    assert!(g.temps(10.0, 0.0));
}

#[test]
fn only_the_direction_counts_for_the_first_ring_however_far_the_pen_goes() {
    // Tout droit vers le haut, très loin : on n'est jamais passé par la part des pastilles, le second anneau ne s'ouvre pas.
    let mut g = geste();
    vers(&mut g, vec2(0.0, -60.0), CINQ);
    vers(&mut g, vec2(0.0, -140.0), CINQ);
    assert!(g.arc.is_none());
    assert_eq!(g.lacher(), Issue::Part(HAUT));
    let mut g = geste();
    vers(&mut g, vec2(-300.0, 20.0), CINQ);
    assert!(g.arc.is_none());
    assert_eq!(g.lacher(), Issue::Part(GAUCHE));
}

#[test]
fn the_second_ring_unfolds_from_the_pastilles_part_and_the_angle_picks_one() {
    // Par la part des pastilles, puis au-delà : la pastille du milieu.
    let mut g = geste();
    vers(&mut g, vec2(-40.0, -40.0), CINQ);
    let arc = g.arc.expect("le second anneau est déployé");
    assert_eq!((arc.part, arc.nombre, arc.pas), (HAUT_GAUCHE, 5, anneau::PAS_ARC));
    assert!(g.affiche, "des couleurs se choisissent en les voyant : il n'attend pas le délai");
    assert_eq!(g.pastille, None);
    vers(&mut g, vec2(-90.0, -90.0), CINQ);
    assert_eq!(g.pastille, Some(2));
    assert_eq!(g.lacher(), Issue::Pastille(2));
    // Une fois au-delà du premier anneau, glisser le long de l'arc change de pastille, même au-dessus d'une autre part.
    let mut g = geste();
    vers(&mut g, vec2(-40.0, -40.0), CINQ);
    vers(&mut g, vec2(-95.0, -70.0), CINQ);
    vers(&mut g, vec2(-130.0, -5.0), CINQ);
    assert_eq!(g.pastille, Some(0), "la première, à gauche");
    assert_eq!(g.survol, Some(HAUT_GAUCHE), "la part des pastilles reste allumée");
    vers(&mut g, vec2(-7.0, -130.0), CINQ);
    assert_eq!(g.pastille, Some(4), "la dernière, en haut");
    assert_eq!(g.lacher(), Issue::Pastille(4));
    // Quitter l'arc par un bout : il se replie, la direction reprend la main.
    let mut g = geste();
    vers(&mut g, vec2(-40.0, -40.0), CINQ);
    vers(&mut g, vec2(-90.0, -90.0), CINQ);
    vers(&mut g, vec2(60.0, 0.0), CINQ);
    assert!(g.arc.is_none() && g.pastille.is_none());
    assert_eq!(g.lacher(), Issue::Part(DROITE));
}

#[test]
fn releasing_on_the_pastilles_part_keeps_the_second_ring_open_for_a_tap() {
    let mut g = geste();
    vers(&mut g, vec2(-42.0, -42.0), CINQ);
    assert_eq!(g.lacher(), Issue::ResteOuvert);
    assert!(g.colle && g.arc.is_some());
    // Le survol allume la pastille sous le pointeur, sans replier l'anneau quand il passe ailleurs.
    let arc = g.arc.unwrap();
    let sur_la_derniere = anneau::point(g.centre, anneau::R_PASTILLE, arc.angle(4));
    g.bouger(sur_la_derniere, None);
    assert_eq!(g.pastille, Some(4));
    g.bouger(g.centre + vec2(200.0, 100.0), None);
    assert!(g.arc.is_some() && g.pastille.is_none());
    // Un appui : la pastille dessous, ou rien (au centre, sur une autre part, trop loin).
    assert_eq!(g.appui(sur_la_derniere), Some(4));
    assert_eq!(g.appui(anneau::point(g.centre, anneau::R_PASTILLE, arc.angle(0))), Some(0));
    assert_eq!(g.appui(g.centre), None);
    assert_eq!(g.appui(g.centre + vec2(120.0, 0.0)), None);
    assert_eq!(g.appui(anneau::point(g.centre, 400.0, arc.angle(2))), None, "trop loin : l'anneau se referme");
    // Sans pastille, la part ne déploie rien : c'est une part comme une autre, que l'application traite.
    let mut g = geste();
    vers(&mut g, vec2(-42.0, -42.0), Some((HAUT_GAUCHE, 0)));
    assert!(g.arc.is_none());
    assert_eq!(g.lacher(), Issue::Part(HAUT_GAUCHE));
}

#[test]
fn many_pastilles_tighten_the_second_ring_instead_of_overlapping() {
    assert!(Arc::nouveau(3, 0).is_none());
    let peu = Arc::nouveau(HAUT_GAUCHE, 5).unwrap();
    assert_eq!(peu.pas, anneau::PAS_ARC);
    assert_eq!(peu.demi_ouverture(), 60.0);
    // Les noms des trois parts sous l'arc s'effacent, pas ceux de leurs voisines.
    assert!(peu.couvre(GAUCHE) && peu.couvre(HAUT_GAUCHE) && peu.couvre(HAUT));
    assert!(!peu.couvre(3) && !peu.couvre(7) && !peu.couvre(DROITE));
    let beaucoup = Arc::nouveau(HAUT_GAUCHE, 64).unwrap();
    assert!((beaucoup.pas * 64.0 - anneau::TOUR_ARC).abs() < 1e-3, "presque le tour, jamais plus");
    // Chaque pastille se vise à son angle, de la première à la dernière.
    for arc in [peu, beaucoup] {
        for i in 0..arc.nombre {
            let d = anneau::point(pos2(0.0, 0.0), anneau::R_PASTILLE, arc.angle(i)).to_vec2();
            assert_eq!(arc.vise(d, f32::INFINITY), Some(i));
            assert_eq!(arc.vise(d * 0.5, f32::INFINITY), None, "en deçà du second anneau");
        }
    }
}

// ------------------------------------------------------------------------------------------------------- les réglages

#[test]
fn the_ring_settings_are_kept_out_of_the_file_until_they_change_and_come_back_the_same() {
    let origine = Settings::default();
    assert!(origine.anneau.actif && origine.anneau.delai_ms == 180);
    // (« [anneau] » et non « anneau » : le mot est dans « panneau ».)
    let ecrit = toml::to_string(&origine).unwrap();
    assert!(!ecrit.contains("[anneau]") && !ecrit.contains("delai_ms"), "l'anneau d'origine n'écrit rien : {ecrit}");
    let mut s = Settings::default();
    s.anneau.delai_ms = 320;
    poser(&mut s.anneau.commandes, HAUT, "vue.miroir");
    let ecrit = toml::to_string(&s).unwrap();
    assert!(ecrit.contains("[anneau]") && ecrit.contains("delai_ms = 320") && ecrit.contains("vue.miroir"), "{ecrit}");
    let relu = Settings::apres_lecture(toml::from_str(&ecrit).unwrap());
    assert_eq!(relu.anneau, s.anneau);
    // Des réglages d'avant l'anneau : celui d'origine.
    let ancien: Settings = toml::from_str("").unwrap();
    assert_eq!(ancien.anneau, Reglages::default());
}

#[test]
fn a_damaged_ring_section_never_breaks_the_other_settings() {
    for abime in [
        "copie_secours_minutes = 7\n[anneau]\ncommandes = [\"outil.gomme\", 3, \"n.importe.quoi\"]\ndelai_ms = 99999\n",
        "copie_secours_minutes = 7\n[anneau]\ncommandes = \"pas une liste\"\n",
        "copie_secours_minutes = 7\n[anneau]\ncommandes = [\"outil.gomme\", \"outil.gomme\", \"outil.gomme\", \"outil.gomme\", \"outil.gomme\", \"outil.gomme\", \"outil.gomme\", \"outil.gomme\", \"outil.gomme\"]\n",
    ] {
        let lu: Settings = toml::from_str(abime).unwrap_or_else(|e| panic!("la lecture échoue : {e}\n{abime}"));
        let s = Settings::apres_lecture(lu);
        assert_eq!(s.autosave_minutes, 7, "les autres réglages sont lus");
        assert!(s.anneau.delai_ms <= anneau::DELAI_MAX);
        // Huit commandes qui existent, toutes différentes.
        for c in &s.anneau.commandes {
            assert!(c == PASTILLES || commande_connue(c), "« {c} » n'existe pas : {:?}", s.anneau.commandes);
        }
        let mut uniques = s.anneau.commandes.to_vec();
        uniques.sort();
        uniques.dedup();
        assert_eq!(uniques.len(), PARTS, "{:?}", s.anneau.commandes);
    }
    // Ce qui était bon reste à sa place ; le reste reprend la commande d'origine de sa part.
    let s = Settings::apres_lecture(toml::from_str("[anneau]\ncommandes = [\"outil.gomme\", 3, \"n.importe.quoi\"]\n").unwrap());
    let origine = commandes_d_origine();
    assert_eq!(s.anneau.commandes[0], "outil.gomme");
    assert_eq!(s.anneau.commandes[1], origine[1]);
    // (La gomme d'origine, en bas, est en double : sa part reçoit une commande d'origine encore libre.)
    assert_ne!(s.anneau.commandes[BAS], "outil.gomme");
}

#[test]
fn the_default_and_proposed_commands_all_exist() {
    for c in commandes_d_origine().iter().map(String::as_str).chain(anneau::PROPOSEES.iter().copied()) {
        assert!(c == PASTILLES || commande_connue(c), "« {c} » n'est pas au registre des commandes");
    }
    assert_eq!(Reglages::default().nettoyes(commande_connue), Reglages::default(), "l'anneau d'origine est déjà propre");
    let mut uniques = anneau::PROPOSEES.to_vec();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(uniques.len(), anneau::PROPOSEES.len());
}

#[test]
fn placing_a_command_swaps_it_with_the_one_it_replaces() {
    let mut c = commandes_d_origine();
    // Une commande qui n'y était pas : elle remplace.
    assert_eq!(poser(&mut c, HAUT, "vue.miroir"), None);
    assert_eq!(c[HAUT], "vue.miroir");
    assert!(!c.contains(&"outil.crayon".to_string()));
    // Une commande déjà dans l'anneau : les deux échangent leur place.
    assert_eq!(poser(&mut c, HAUT, "outil.gomme"), Some(BAS));
    assert_eq!((c[HAUT].as_str(), c[BAS].as_str()), ("outil.gomme", "vue.miroir"));
    // La même à la même place : rien.
    let avant = c.clone();
    assert_eq!(poser(&mut c, HAUT, "outil.gomme"), None);
    assert_eq!(c, avant);
}

// -------------------------------------------------------------------------------------------- dans l'application entière

fn film() -> Document {
    let mut doc = Document::new(400, 300, 24);
    for _ in 0..2 {
        let dernier = doc.panels.len() - 1;
        doc.add_scene_after(dernier);
    }
    doc
}

fn banc() -> Banc {
    let mut b = banc_taille(film(), false, 1600, 900);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert!(!b.app.board_shown(), "le dessin, pas la planche");
    assert!(b.app.settings.anneau.actif, "en service d'origine");
    b
}

fn bouton(p: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }
}

fn appuyer(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, PointerButton::Secondary, true)], DT);
}

fn relacher(b: &mut Banc, p: Pos2) {
    b.image(vec![bouton(p, PointerButton::Secondary, false)], DT);
}

/// Le bouton du stylet appuyé en `p`, glissé de `d` en deux images, relâché : un geste vif (bien moins que le délai).
fn glisser(b: &mut Banc, p: Pos2, d: Vec2) {
    appuyer(b, p);
    b.image(vec![Event::PointerMoved(p + d * 0.5)], DT);
    b.image(vec![Event::PointerMoved(p + d)], DT);
    relacher(b, p + d);
}

fn milieu(b: &Banc) -> Pos2 {
    b.app.canvas_rect.center()
}

fn encre(b: &Banc) -> bool {
    let doc = &b.app.doc;
    doc.panels.iter().flat_map(|p| p.layers.iter()).flat_map(|l| l.frames.iter().flatten()).any(|c| doc.cels.get(c).is_some_and(|c| !c.raster.is_blank() || c.vector.as_ref().is_some_and(|v| !v.elements.is_empty())))
}

fn textes_peints(shape: &Shape, sortie: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => sortie.push(t.galley.job.text.clone()),
        Shape::Vec(v) => v.iter().for_each(|s| textes_peints(s, sortie)),
        _ => {}
    }
}

/// Les textes peints à la dernière image.
fn textes(b: &Banc) -> Vec<String> {
    let mut t = Vec::new();
    for s in &b.formes {
        textes_peints(&s.shape, &mut t);
    }
    t
}

fn peint(b: &Banc, texte: &str) -> bool {
    textes(b).iter().any(|t| t == texte)
}

#[test]
fn a_flick_of_the_pen_button_launches_the_command_of_its_direction() {
    let mut b = banc();
    let avant = b.app.history.undo_len();
    let p = milieu(&b);
    // Vers le bas : la gomme. Rien ne s'est dessiné pendant le geste, sauf le nom de la commande une fois partie.
    glisser(&mut b, p, vec2(0.0, 60.0));
    assert_eq!(b.app.tool, Tool::Eraser);
    assert!(b.app.anneau.geste.is_none());
    assert!(b.app.reglages_droit.is_none(), "un glissé n'ouvre pas la fenêtre des réglages");
    assert!(!peint(&b, "Plan suivant"), "l'anneau ne s'est pas dessiné");
    assert_eq!(b.app.anneau.echo.as_ref().map(|e| e.texte.as_str()), Some("Gomme"));
    // Vers le haut : le crayon ; en haut à droite : le pinceau.
    glisser(&mut b, p, vec2(0.0, -60.0));
    assert_eq!(b.app.tool, Tool::Pencil);
    glisser(&mut b, p, vec2(50.0, -50.0));
    assert_eq!(b.app.tool, Tool::Brush);
    // À droite et à gauche : le plan suivant, le plan précédent.
    assert_eq!(b.app.cur_panel, 0);
    glisser(&mut b, p, vec2(60.0, 3.0));
    assert_eq!(b.app.cur_panel, 1);
    glisser(&mut b, p, vec2(-60.0, 0.0));
    assert_eq!(b.app.cur_panel, 0);
    // Changer d'outil ou de plan ne trace rien et n'est pas une étape d'annulation.
    assert!(b.app.stroke.is_none() && !encre(&b));
    assert_eq!(b.app.history.undo_len(), avant);
}

#[test]
fn a_brief_click_still_opens_the_tool_settings_and_a_release_at_the_centre_too() {
    let mut b = banc();
    let p = milieu(&b);
    appuyer(&mut b, p);
    relacher(&mut b, p);
    b.chauffer(4);
    let fenetre = b.app.reglages_droit.expect("le clic bref ouvre les réglages de l'outil");
    assert!((fenetre.pos - p).length() < 1.0, "sous le curseur");
    assert!(b.app.anneau.geste.is_none());
    assert_eq!(b.app.tool, Tool::Brush);
    // Un clic droit ailleurs (hors de la fenêtre, qui s'étend à droite et en bas du clic) la ferme, sans rouvrir ni l'anneau ni la
    // fenêtre : la règle de la fenêtre, inchangée.
    let q = p - vec2(300.0, 120.0);
    assert!(!fenetre.rect.is_some_and(|r| r.contains(q)), "{:?} / {q:?}", fenetre.rect);
    appuyer(&mut b, q);
    assert!(b.app.anneau.geste.is_none(), "la fenêtre a pris l'appui : pas d'anneau");
    relacher(&mut b, q);
    b.chauffer(3);
    assert!(b.app.reglages_droit.is_none());
    // Parti vers une part puis revenu au centre : la fenêtre.
    appuyer(&mut b, p);
    b.image(vec![Event::PointerMoved(p + vec2(0.0, 60.0))], DT);
    b.image(vec![Event::PointerMoved(p + vec2(3.0, 4.0))], DT);
    relacher(&mut b, p + vec2(3.0, 4.0));
    b.chauffer(3);
    assert!(b.app.reglages_droit.is_some());
    assert_eq!(b.app.tool, Tool::Brush, "aucune commande n'est partie");
}

#[test]
fn holding_the_pen_button_shows_the_ring_with_its_eight_commands() {
    let mut b = banc();
    let p = milieu(&b);
    appuyer(&mut b, p);
    for _ in 0..4 {
        b.image(Vec::new(), DT);
    }
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| !g.affiche), "avant le délai (180 ms), rien");
    assert!(!peint(&b, "Plan suivant"));
    for _ in 0..12 {
        b.image(Vec::new(), DT);
    }
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| g.affiche));
    for nom in ["Plan suivant", "Nouveau plan", "Gomme", "Annuler", "Plan précédent", "Pastilles", "Crayon", "Pinceau"] {
        assert!(peint(&b, nom), "« {nom} » autour de l'anneau : {:?}", textes(&b));
    }
    // Tant qu'il est tenu sans bouger, il ne demande aucune repeinture.
    let (images, _) = jusqu_au_repos(&mut b, 80);
    assert!(images < 80, "l'anneau affiché ne demande aucune repeinture continue ({images} images)");
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| g.affiche), "toujours là");
    let m = b.image(Vec::new(), DT);
    assert!(m.reveil >= std::time::Duration::from_millis(400), "réveil : {:?}{}", m.reveil, b.raison_du_reveil());
    // Glisser vers une part puis relâcher : sa commande.
    b.image(vec![Event::PointerMoved(p + vec2(-45.0, 45.0))], DT);
    relacher(&mut b, p + vec2(-45.0, 45.0));
    b.chauffer(2);
    assert!(b.app.anneau.geste.is_none());
    assert!(b.app.reglages_droit.is_none());
    assert_eq!(b.app.anneau.echo.as_ref().map(|e| e.texte.as_str()), Some("Annuler"));
    // Le délai se règle : nul, l'anneau est là dès l'appui.
    b.app.settings.anneau.delai_ms = 0;
    appuyer(&mut b, p);
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| g.affiche));
    assert!(peint(&b, "Gomme"));
    relacher(&mut b, p);
}

#[test]
fn the_second_ring_picks_a_pastille_like_a_click_in_the_pastille_bar() {
    let mut b = banc();
    let p = milieu(&b);
    let tous = b.app.perso_tous();
    assert!(tous.len() >= 3, "les brosses classiques d'origine : {tous:?}");
    assert_eq!(b.app.perso.actif, None);
    // Par la part des pastilles (en haut à gauche), puis au-delà du premier anneau.
    let d = vec2(-90.0, -90.0);
    let attendue = Arc::nouveau(HAUT_GAUCHE, tous.len()).unwrap().vise(d, f32::INFINITY).expect("une pastille est visée");
    appuyer(&mut b, p);
    b.image(vec![Event::PointerMoved(p + vec2(-40.0, -40.0))], DT);
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| g.arc.is_some() && g.affiche), "le second anneau se montre tout de suite");
    assert_eq!(b.app.anneau.pastilles.len(), tous.len());
    b.image(vec![Event::PointerMoved(p + d)], DT);
    relacher(&mut b, p + d);
    assert_eq!(b.app.perso.actif, Some(tous[attendue]));
    assert!(b.app.anneau.geste.is_none());
    assert!(!encre(&b));
}

#[test]
fn released_on_the_pastilles_part_the_second_ring_waits_for_a_tap_that_does_nothing_else() {
    let mut b = banc();
    let p = milieu(&b);
    let tous = b.app.perso_tous();
    glisser(&mut b, p, vec2(-42.0, -42.0));
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| g.colle), "resté ouvert");
    b.chauffer(2);
    // Un appui sur une pastille la choisit.
    let arc = b.app.anneau.geste.as_ref().unwrap().arc.unwrap();
    let sur = anneau::point(p, anneau::R_PASTILLE, arc.angle(0));
    b.image(vec![Event::PointerMoved(sur)], DT);
    b.image(vec![bouton(sur, PointerButton::Primary, true)], DT);
    b.image(vec![bouton(sur, PointerButton::Primary, false)], DT);
    b.chauffer(2);
    assert_eq!(b.app.perso.actif, Some(tous[0]));
    assert!(b.app.anneau.geste.is_none());
    assert!(!encre(&b), "choisir une pastille ne trace rien");
    // Resté ouvert de nouveau : un appui ailleurs le referme et ne fait rien d'autre.
    glisser(&mut b, p, vec2(-42.0, -42.0));
    assert!(b.app.anneau.geste.as_ref().is_some_and(|g| g.colle));
    b.chauffer(2);
    let ailleurs = p + vec2(260.0, 90.0);
    b.image(vec![Event::PointerMoved(ailleurs)], DT);
    b.image(vec![bouton(ailleurs, PointerButton::Primary, true)], DT);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(ailleurs + vec2(8.0 * k as f32, 0.0))], DT);
    }
    b.image(vec![bouton(ailleurs + vec2(48.0, 0.0), PointerButton::Primary, false)], DT);
    b.chauffer(2);
    assert!(b.app.anneau.geste.is_none());
    assert_eq!(b.app.perso.actif, Some(tous[0]), "la pastille n'a pas changé");
    assert!(b.app.stroke.is_none() && !encre(&b), "l'appui qui referme ne trace rien");
}

#[test]
fn the_pen_tip_draws_nothing_while_the_pen_button_is_held() {
    let mut b = banc();
    let p = milieu(&b);
    appuyer(&mut b, p);
    // La pointe se pose et glisse pendant que le bouton est tenu.
    b.image(vec![bouton(p, PointerButton::Primary, true)], DT);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(p + vec2(10.0 * k as f32, 0.0))], DT);
    }
    b.image(vec![bouton(p + vec2(60.0, 0.0), PointerButton::Primary, false)], DT);
    assert!(b.app.stroke.is_none() && !encre(&b));
    relacher(&mut b, p + vec2(60.0, 0.0));
    assert_eq!(b.app.cur_panel, 1, "le geste, lui, a lancé sa commande");
    // Pendant un trait, le bouton du stylet n'ouvre pas l'anneau.
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, PointerButton::Primary, true)], DT);
    b.image(vec![Event::PointerMoved(p + vec2(20.0, 5.0))], DT);
    b.image(vec![bouton(p + vec2(20.0, 5.0), PointerButton::Secondary, true)], DT);
    assert!(b.app.anneau.geste.is_none());
    b.image(vec![bouton(p + vec2(20.0, 5.0), PointerButton::Secondary, false)], DT);
    b.image(vec![bouton(p + vec2(20.0, 5.0), PointerButton::Primary, false)], DT);
}

#[test]
fn escape_and_a_lost_release_abandon_the_gesture_without_launching_anything() {
    use eframe::egui::Key;
    let mut b = banc();
    let p = milieu(&b);
    appuyer(&mut b, p);
    b.image(vec![Event::PointerMoved(p + vec2(0.0, 60.0))], DT);
    let touche = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![touche(true)], DT);
    b.image(vec![touche(false)], DT);
    assert!(b.app.anneau.geste.is_none());
    relacher(&mut b, p + vec2(0.0, 60.0));
    assert_eq!(b.app.tool, Tool::Brush, "Échap : aucune commande");
    // Le relâché n'arrive jamais (le pointeur est parti avec le bouton) : le geste s'efface.
    appuyer(&mut b, p);
    b.image(vec![Event::PointerMoved(p + vec2(0.0, 60.0))], DT);
    b.image(vec![Event::PointerGone], DT);
    b.chauffer(2);
    assert!(b.app.anneau.geste.is_none());
    assert_eq!(b.app.tool, Tool::Brush);
}

#[test]
fn the_ring_asks_for_no_continuous_repaint_and_recomputes_nothing() {
    let mut b = banc();
    let p = milieu(&b);
    let (cels, rejeu) = (b.app.doc.cels.len(), b.app.rejeu.has_pending());
    glisser(&mut b, p, vec2(0.0, 60.0));
    assert_eq!(b.app.doc.cels.len(), cels, "aucun dessin de plus");
    assert_eq!(b.app.rejeu.has_pending(), rejeu, "aucun rejeu demandé");
    // Le nom de la commande s'efface de lui-même, puis plus rien ne réveille l'écran.
    let (images, _) = jusqu_au_repos(&mut b, 120);
    assert!(images < 120, "au repos après un geste ({images} images)");
    b.image(Vec::new(), 1.0);
    assert!(b.app.anneau.echo.is_none(), "le nom de la commande est effacé");
    let m = b.image(Vec::new(), 0.5);
    assert!(m.reveil >= std::time::Duration::from_millis(400), "réveil : {:?}{}", m.reveil, b.raison_du_reveil());
}

#[test]
fn switched_off_the_right_click_is_the_one_from_before() {
    let mut b = banc();
    b.app.settings.anneau.actif = false;
    b.chauffer(2);
    let p = milieu(&b);
    glisser(&mut b, p, vec2(0.0, 60.0));
    assert_eq!(b.app.tool, Tool::Brush, "éteint : un glissé ne lance rien");
    assert!(b.app.anneau.geste.is_none());
    appuyer(&mut b, p);
    relacher(&mut b, p);
    b.chauffer(4);
    assert!(b.app.reglages_droit.is_some(), "le clic droit ouvre toujours les réglages");
}

#[test]
fn selection_keeps_its_menu_on_a_click_and_gets_the_ring_on_a_flick() {
    let mut b = banc();
    b.app.set_tool(Tool::Select);
    b.chauffer(3);
    let p = milieu(&b);
    appuyer(&mut b, p);
    relacher(&mut b, p);
    b.chauffer(4);
    assert!(b.app.reglages_droit.is_none());
    assert!(textes(&b).iter().any(|t| t.contains("Supprimer la sélection")), "le menu du dessin : {:?}", textes(&b));
    // (Échap ferme le menu.)
    use eframe::egui::Key;
    let touche = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![touche(true)], DT);
    b.image(vec![touche(false)], DT);
    b.chauffer(3);
    glisser(&mut b, p, vec2(0.0, 60.0));
    assert_eq!(b.app.tool, Tool::Eraser, "le glissé lance la commande, quel que soit l'outil");
}

#[test]
fn on_the_vignettes_board_a_flick_launches_the_command_and_a_click_keeps_the_case_menu() {
    use crate::vignettes_tests::{ouvert, quatre_plans, vignette};
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_formes = true;
    b.app.set_tool(Tool::Brush);
    b.chauffer(3);
    let v = vignette(&b, 4).center();
    // Un geste vif depuis une case : la commande part, sans menu ni fenêtre, sans rien tracer dans la case.
    glisser(&mut b, v, vec2(0.0, 60.0));
    b.chauffer(2);
    assert_eq!(b.app.tool, Tool::Eraser, "le glissé lance la commande sur la planche aussi");
    assert!(b.app.reglages_droit.is_none());
    assert!(!textes(&b).iter().any(|t| t.contains("Animer cette image")), "pas de menu de case après un glissé : {:?}", textes(&b));
    assert!(b.app.stroke.is_none() && !encre(&b));
    // La pointe posée pendant que le bouton est tenu ne dessine pas dans la case.
    b.app.set_tool(Tool::Brush);
    b.chauffer(2);
    appuyer(&mut b, v);
    b.image(vec![bouton(v, PointerButton::Primary, true)], DT);
    for k in 1..=5 {
        b.image(vec![Event::PointerMoved(v + vec2(6.0 * k as f32, 0.0))], DT);
    }
    b.image(vec![bouton(v + vec2(30.0, 0.0), PointerButton::Primary, false)], DT);
    relacher(&mut b, v + vec2(30.0, 0.0));
    b.chauffer(2);
    assert!(b.app.stroke.is_none() && !encre(&b), "aucun trait dans la case pendant le geste");
    // Le clic bref : le menu de la case, comme avant, et pas la fenêtre des réglages.
    let v = vignette(&b, 4).center();
    appuyer(&mut b, v);
    relacher(&mut b, v);
    b.chauffer(4);
    assert!(b.app.reglages_droit.is_none(), "sur une case, le clic droit garde son menu");
    assert!(textes(&b).iter().any(|t| t.contains("Animer cette image")), "le menu de la case : {:?}", textes(&b));
}

#[test]
fn a_command_sent_from_a_right_click_menu_takes_its_place_in_the_ring() {
    let mut b = banc();
    // Ce que fait l'entrée « Envoyer dans l'anneau » du clic droit d'une icône.
    crate::anneauui::demander(&b.ctx, "vue.miroir", HAUT);
    b.chauffer(2);
    assert_eq!(b.app.settings.anneau.commandes[HAUT], "vue.miroir");
    assert!(b.app.status.contains("Miroir de la vue") && b.app.status.contains("haut"), "{}", b.app.status);
    // Un outil déjà dans l'anneau : il échange sa place.
    crate::anneauui::demander(&b.ctx, "outil.gomme", HAUT);
    b.chauffer(2);
    assert_eq!(b.app.settings.anneau.commandes[HAUT], "outil.gomme");
    assert_eq!(b.app.settings.anneau.commandes[BAS], "vue.miroir");
    // Le geste suit aussitôt.
    let p = milieu(&b);
    glisser(&mut b, p, vec2(0.0, -60.0));
    assert_eq!(b.app.tool, Tool::Eraser);
    // Une commande inconnue est refusée, l'anneau ne change pas.
    let avant = b.app.settings.anneau.commandes.clone();
    crate::anneauui::demander(&b.ctx, "n.importe.quoi", DROITE);
    b.chauffer(2);
    assert_eq!(b.app.settings.anneau.commandes, avant);
}

fn places_peintes(shape: &Shape, sortie: &mut Vec<(String, eframe::egui::Rect)>) {
    match shape {
        Shape::Text(t) => sortie.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| places_peintes(s, sortie)),
        _ => {}
    }
}

/// Les textes peints à la dernière image, avec leur place.
fn places(b: &Banc) -> Vec<(String, eframe::egui::Rect)> {
    let mut t = Vec::new();
    for s in &b.formes {
        places_peintes(&s.shape, &mut t);
    }
    t
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, PointerButton::Primary, true)], DT);
    b.image(vec![bouton(p, PointerButton::Primary, false)], DT);
    b.chauffer(3);
}

/// Le vrai chemin : clic droit sur l'icône de la Gomme dans la barre d'outils, « Envoyer dans l'anneau », une direction.
fn envoyer_la_gomme_en_haut(b: &mut Banc) {
    use egui_phosphor::regular as ph;
    // L'icône de la Gomme, dans la barre d'outils de gauche.
    let (_, icone) = places(b).into_iter().filter(|(t, r)| t == ph::ERASER && r.min.x < 60.0).min_by(|a, c| a.1.min.x.total_cmp(&c.1.min.x)).expect("l'icône de la Gomme dans la barre d'outils");
    appuyer(b, icone.center());
    relacher(b, icone.center());
    b.chauffer(4);
    assert!(b.app.anneau.geste.is_none(), "hors du dessin, le bouton du stylet n'ouvre pas l'anneau");
    let (_, entree) = places(b).into_iter().find(|(t, _)| t.contains("Envoyer dans l'anneau")).unwrap_or_else(|| panic!("l'entrée du menu : {:?}", textes(b)));
    clic(b, entree.center());
    b.chauffer(4);
}

#[test]
fn right_clicking_a_tool_offers_to_send_it_to_one_of_the_eight_directions() {
    let mut b = banc();
    envoyer_la_gomme_en_haut(&mut b);
    // Les huit directions, chacune avec la commande qu'elle porte ; la Gomme est déjà en bas (une coche).
    let t = textes(&b);
    for attendu in ["Haut : Crayon", "Bas : Gomme", "Gauche : Plan précédent", "Haut gauche : Pastilles", "Droite : Plan suivant"] {
        assert!(t.iter().any(|x| x.contains(attendu)), "« {attendu} » dans le sous-menu : {t:?}");
    }
    let (_, haut) = places(&b).into_iter().find(|(x, _)| x.contains("Haut : Crayon")).unwrap();
    clic(&mut b, haut.center());
    b.chauffer(3);
    // La Gomme monte, le Crayon prend sa place en bas : un échange.
    assert_eq!(b.app.settings.anneau.commandes[HAUT], "outil.gomme");
    assert_eq!(b.app.settings.anneau.commandes[BAS], "outil.crayon");
    assert!(b.app.status.contains("Gomme") && b.app.status.contains("Crayon"), "{}", b.app.status);
    // Éteint, le menu du clic droit n'a plus l'entrée.
    b.app.settings.anneau.actif = false;
    b.chauffer(3);
    use egui_phosphor::regular as ph;
    let (_, icone) = places(&b).into_iter().filter(|(t, r)| t == ph::ERASER && r.min.x < 60.0).min_by(|a, c| a.1.min.x.total_cmp(&c.1.min.x)).unwrap();
    appuyer(&mut b, icone.center());
    relacher(&mut b, icone.center());
    b.chauffer(4);
    let t = textes(&b);
    assert!(t.iter().any(|x| x.contains("Assigner un raccourci")), "le menu du clic droit de l'outil : {t:?}");
    assert!(!t.iter().any(|x| x.contains("Envoyer dans l'anneau")));
}

#[test]
fn the_preferences_tab_changes_a_command_and_the_delay() {
    let mut b = banc();
    b.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Anneau;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    let t = textes(&b);
    for attendu in ["Anneau sous le stylet", "Délai avant d'afficher l'anneau", "Commande de chaque direction"] {
        assert!(t.iter().any(|x| x.contains(attendu)), "« {attendu} » dans l'onglet : {t:?}");
    }
    // L'aperçu : les huit noms autour de l'anneau.
    for nom in ["Plan suivant", "Gomme", "Pastilles", "Crayon"] {
        assert!(peint(&b, nom), "« {nom} » dans l'aperçu");
    }
    // La liste du haut : y choisir le Cutter.
    let liste = places(&b).into_iter().filter(|(x, _)| x.ends_with("Crayon") && x != "Crayon").map(|(_, r)| r).next().expect("la liste de la direction du haut");
    clic(&mut b, liste.center());
    let (_, cutter) = places(&b).into_iter().find(|(x, _)| x.ends_with("Cutter")).unwrap_or_else(|| panic!("la liste ouverte : {:?}", textes(&b)));
    clic(&mut b, cutter.center());
    b.chauffer(3);
    assert_eq!(b.app.settings.anneau.commandes[HAUT], "outil.cutter");
    assert!(peint(&b, "Cutter"), "l'aperçu suit");
    assert!(!b.app.settings.anneau.commandes.contains(&"outil.crayon".to_string()), "le Crayon a cédé sa place");
    // Choisir dans une autre liste une commande déjà dans l'anneau : les deux échangent.
    b.app.anneau_poser(BAS, "outil.cutter");
    assert_eq!(b.app.settings.anneau.commandes[BAS], "outil.cutter");
    assert_eq!(b.app.settings.anneau.commandes[HAUT], "outil.gomme");
    assert_eq!(b.app.settings.anneau.clone().nettoyes(commande_connue), b.app.settings.anneau, "toujours huit commandes différentes");
}

#[test]
fn long_registry_names_are_shortened_for_the_ring_labels() {
    use crate::anneauui::nom_court;
    assert_eq!(nom_court("Gomme"), "Gomme");
    assert_eq!(nom_court("Passer du storyboard à l'animation, et retour"), "Passer du storyboard à l'anim…");
    assert_eq!(nom_court("Lecture en boucle (oui / non)"), "Lecture en boucle");
    assert_eq!(nom_court("Mode concentration : ne garder que le dessin (la même touche remet tout)"), "Mode concentration");
    let b = banc();
    assert_eq!(b.app.anneau_part(PASTILLES).nom, "Pastilles");
    assert_eq!(b.app.anneau_part("panneau.suivant").nom, "Plan suivant");
    assert!(!b.app.anneau_part("outil.gomme").icone.is_empty());
}

/// Captures pour voir l'anneau à l'écran (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc anneau_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn anneau_captures() {
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("ANNEAU pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("ANNEAU {nom}");
    };
    let mut b = crate::perf_tests::banc(film(), true);
    b.app.tool = Tool::Brush;
    b.chauffer(8);
    // Trois personnages, pour des pastilles en couleur.
    for _ in 0..3 {
        b.app.perso_ajouter();
    }
    b.chauffer(3);
    let p = milieu(&b);
    // L'anneau tenu, une part visée.
    b.app.settings.anneau.delai_ms = 0;
    appuyer(&mut b, p);
    b.image(vec![Event::PointerMoved(p + vec2(0.0, 55.0))], DT);
    sauver(&b, "anneau-1-tenu.png");
    // Le second anneau, une pastille visée.
    b.image(vec![Event::PointerMoved(p + vec2(-45.0, -45.0))], DT);
    b.image(vec![Event::PointerMoved(p + vec2(-70.0, -105.0))], DT);
    sauver(&b, "anneau-2-pastilles.png");
    relacher(&mut b, p + vec2(-70.0, -105.0));
    b.chauffer(2);
    sauver(&b, "anneau-3-nom-de-la-commande.png");
    // Le clic droit sur un outil : « Envoyer dans l'anneau » et ses huit directions.
    b.garder_formes = true;
    b.chauffer(60);
    envoyer_la_gomme_en_haut(&mut b);
    sauver(&b, "anneau-4-envoyer.png");
    use eframe::egui::Key;
    let touche = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    for _ in 0..2 {
        b.image(vec![touche(true)], DT);
        b.image(vec![touche(false)], DT);
    }
    b.chauffer(3);
    // Les Préférences.
    b.app.settings.anneau.delai_ms = 180;
    b.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Anneau;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    sauver(&b, "anneau-5-preferences.png");
}
