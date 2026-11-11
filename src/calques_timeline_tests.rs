//! Les calques dans la timeline du Storyboard et du Montage (Anthony, 2026-11-10 et 11, branche `calques-timeline`, partie 137) :
//! - **trois calques à la fois** : le calque actif au milieu, toujours à la même place, entre ses deux voisins ; deux fines bandes
//!   comptent les autres ; « Toute la pile » rouvre tout ;
//! - **un dossier n'est jamais un calque** : une ligne fine, qui ne compte pas dans les trois, qu'un clic ferme ou ouvre ;
//! - **un dossier fermé se traverse** avec « calque du dessus / du dessous » et se referme quand on en sort ;
//! - **la bande dit le type, le filet à gauche dit le dossier** (`folders::couleur_du_rangement`) ;
//! - **les plans** de hauteur moyenne ou en bandeau ;
//! - **le zoom suit le pointeur** : les touches du zoom de l'image zooment la bande quand le pointeur est dessus.
//!
//! La pile des essais est celle du plan le plus chargé de son projet « retakes brawlydays » : deux dossiers, un calque seul entre eux,
//! une référence tout en bas. Les premiers essais sont purs ; les derniers passent par la vraie fenêtre sans écran (`Banc`). Le dernier,
//! ignoré, écrit des captures à regarder (`FEANOR_CAPTURE_DIR` ; `FEANOR_PROJET_ESSAI` : le manifeste d'une COPIE de projet).

use eframe::egui::{pos2, vec2, Event, Modifiers, Rect};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::montagerows::{layer_in_row, LAYER_H};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes::{dossier_de_passage, dossier_ferme, HauteurPlans, Pile, Sorte, DOSSIER_H, MARGE_H, MORE_ROW, VOISIN_H};
use crate::pistes_tests::{app_with, clic, doc_with, textes, PPF};
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------ l'outillage

/// La pile du plan de retakes, du bas vers le haut. Du haut vers le bas (les rangées) : 0 « Dossier 2 » [1 V11, 2 V10, 3 V9, 4 V8],
/// 5 V13, 6 « Dossier 1 » [7 Dessin, 8 V5, 9 V4], 10 Réf.
fn pile_de_retakes(dossier_2_ferme: bool) -> Vec<Layer> {
    let mut d2 = Layer::new_folder("Dossier 2");
    d2.collapsed = dossier_2_ferme;
    let d1 = Layer::new_folder("Dossier 1");
    let dans = |nom: &str, dossier: &Layer| {
        let mut l = Layer::new(nom, LayerKind::Vector);
        l.parent = Some(dossier.id);
        l
    };
    let du_haut = vec![
        d2.clone(),
        dans("V11", &d2),
        dans("V10", &d2),
        dans("V9", &d2),
        dans("V8", &d2),
        Layer::new("V13", LayerKind::Vector),
        d1.clone(),
        dans("Dessin", &d1),
        dans("V5", &d1),
        dans("V4", &d1),
        Layer::new("Réf", LayerKind::Bitmap),
    ];
    du_haut.into_iter().rev().collect()
}

/// Un film de deux plans : le premier a la pile de retakes (un dessin tenu sur chaque calque), le second un seul calque.
fn film_de_retakes(dossier_2_ferme: bool) -> Document {
    let mut doc = doc_with(&[&["x"], &["Autre"]]);
    doc.panels[0].layers = pile_de_retakes(dossier_2_ferme);
    doc.panels[0].set_length(24);
    let cels: Vec<_> = (0..12).map(|_| doc.add_cel(crate::document::Cel::new(doc.width, doc.height, LayerKind::Bitmap))).collect();
    for (k, l) in doc.panels[0].layers.iter_mut().enumerate().filter(|(_, l)| !l.folder) {
        l.frames = vec![Some(cels[k]); 24];
    }
    doc.panels[1].layers[0].frames = vec![Some(cels[11]); 24];
    doc
}

fn rang(app: &App, nom: &str) -> usize {
    app.layers().iter().position(|l| l.name == nom).unwrap_or_else(|| panic!("pas de calque « {nom} »"))
}

/// Rend courant le calque `nom`, souris relâchée : il devient le calque actif.
fn aller(app: &mut App, nom: &str) {
    app.cur_layer = rang(app, nom);
    app.pistes_follow_current(false);
}

/// Une application sans fenêtre sur le film de retakes, en « trois calques », le calque `courant` actif.
fn app_de_retakes(nom: &str, dossier_2_ferme: bool, courant: &str) -> App {
    let mut app = app_with(nom, film_de_retakes(dossier_2_ferme));
    app.settings.montage_pile = Pile::Trois;
    aller(&mut app, courant);
    app
}

/// Les lignes de la timeline, de haut en bas : (sorte, rangée, hauteur).
fn lignes(app: &App) -> Vec<(Sorte, usize, f32)> {
    app.montage_rows(300.0).lanes.iter().map(|l| (l.sorte, l.row, l.h)).collect()
}

fn courant(app: &App) -> String {
    app.layers()[app.cur_layer].name.clone()
}

/// « Calque du dessus » ou « du dessous », puis la souris relâchée.
fn pas(app: &mut App, action: &str) -> String {
    app.run_action(action);
    app.pistes_follow_current(false);
    courant(app)
}

// ------------------------------------------------------------------------------------ trois calques, et les dossiers

#[test]
fn a_folder_is_never_one_of_the_three_layers_and_its_thin_line_takes_its_room_from_the_strip() {
    // « Dessin » est le premier calque de « Dossier 1 » : la ligne du dossier tombe entre lui et son voisin du dessus, « V13 ».
    let mut app = app_de_retakes("fenetre-dossier", false, "Dessin");
    assert_eq!(
        lignes(&app),
        [
            (Sorte::Dessus, MORE_ROW, MARGE_H - VOISIN_H - DOSSIER_H),
            (Sorte::Calque, 5, VOISIN_H),
            (Sorte::Dossier, 6, DOSSIER_H),
            (Sorte::Calque, 7, LAYER_H),
            (Sorte::Calque, 8, VOISIN_H),
            (Sorte::Dessous, MORE_ROW, MARGE_H - VOISIN_H),
        ]
    );
    let lanes = app.montage_rows(300.0).lanes;
    assert_eq!((lanes[0].compte, lanes[0].rangees), (4, (0, 5)), "V11, V10, V9 et V8 au-dessus : le dossier ne compte pas");
    assert_eq!((lanes[5].compte, lanes[5].rangees), (2, (9, 11)), "V4 et Réf au-dessous");
    // Un cran plus bas, plus de dossier entre les trois : la fine bande reprend sa place.
    aller(&mut app, "V5");
    assert_eq!(lignes(&app).iter().map(|l| (l.0, l.1)).collect::<Vec<_>>(), [(Sorte::Dessus, MORE_ROW), (Sorte::Calque, 7), (Sorte::Calque, 8), (Sorte::Calque, 9), (Sorte::Dessous, MORE_ROW)]);
    assert_eq!(app.montage_rows(300.0).lanes[0].compte, 5, "V11, V10, V9, V8 et V13");
    // D'un bout à l'autre de la pile : la fenêtre garde sa hauteur, le calque actif sa place, et aucune des trois pistes n'est un dossier.
    let noms: Vec<String> = app.layers().iter().filter(|l| !l.folder).map(|l| l.name.clone()).collect();
    for nom in &noms {
        aller(&mut app, nom);
        let lanes = app.montage_rows(300.0).lanes;
        assert_eq!(lanes.last().map(|l| l.top + l.h), Some(2.0 * MARGE_H + LAYER_H), "{nom}");
        let actif = lanes.iter().find(|l| !l.folded).unwrap_or_else(|| panic!("{nom} : pas de calque actif"));
        assert_eq!((actif.top, actif.h), (MARGE_H, LAYER_H), "{nom}");
        assert_eq!(layer_in_row(app.layers(), actif.row), Some(rang(&app, nom)), "{nom} est au milieu");
        let pistes: Vec<_> = lanes.iter().filter(|l| l.sorte == Sorte::Calque).collect();
        assert!(pistes.len() <= 3, "{nom} : trois calques au plus");
        assert!(pistes.iter().all(|l| layer_in_row(app.layers(), l.row).is_some_and(|k| !app.layers()[k].folder)), "{nom} : un dossier parmi les trois");
        assert!(lanes.iter().filter(|l| l.est_dossier()).all(|l| l.h == DOSSIER_H && l.h < VOISIN_H), "{nom} : un dossier est toujours plus petit");
    }
    // « Toute la pile » : tous les calques en bandes, les dossiers en lignes fines.
    app.settings.montage_pile = Pile::Toute;
    aller(&mut app, "V5");
    let toute = lignes(&app);
    assert_eq!(toute.len(), 11);
    assert_eq!(toute.iter().filter(|l| l.0 == Sorte::Dossier).map(|l| (l.1, l.2)).collect::<Vec<_>>(), [(0, DOSSIER_H), (6, DOSSIER_H)]);
    assert_eq!(toute.iter().filter(|l| l.2 == LAYER_H).map(|l| l.1).collect::<Vec<_>>(), [8], "seul le calque actif est déplié");
}

#[test]
fn a_closed_folder_is_crossed_with_layer_above_and_below_and_closes_again_when_one_leaves() {
    let mut app = app_de_retakes("traverser", true, "V13");
    let d2 = rang(&app, "Dossier 2");
    app.modified = false;
    // Fermé : sa ligne est juste au-dessus du calque actif, ses quatre calques n'ont pas de piste mais la fine bande les compte.
    assert_eq!(
        lignes(&app),
        [
            (Sorte::Dessus, MORE_ROW, MARGE_H - DOSSIER_H),
            (Sorte::Dossier, 0, DOSSIER_H),
            (Sorte::Calque, 5, LAYER_H),
            (Sorte::Dossier, 6, DOSSIER_H),
            (Sorte::Calque, 7, VOISIN_H),
            (Sorte::Dessous, MORE_ROW, MARGE_H - VOISIN_H - DOSSIER_H),
        ]
    );
    assert_eq!(app.montage_rows(300.0).lanes[0].compte, 4, "« 4 au-dessus » reste vrai, dossier ouvert ou fermé");
    assert!(dossier_ferme(app.layers(), d2, app.cur_layer));
    // « Calque du dessus » : on entre dans le dossier fermé, qui se montre ouvert le temps du passage.
    assert_eq!(pas(&mut app, "calque.dessus"), "V8");
    assert!(dossier_de_passage(app.layers(), d2, app.cur_layer) && !dossier_ferme(app.layers(), d2, app.cur_layer));
    assert_eq!(lignes(&app).iter().map(|l| (l.0, l.1)).collect::<Vec<_>>(), [(Sorte::Dessus, MORE_ROW), (Sorte::Calque, 3), (Sorte::Calque, 4), (Sorte::Calque, 5), (Sorte::Dessous, MORE_ROW)]);
    assert_eq!([pas(&mut app, "calque.dessus"), pas(&mut app, "calque.dessus"), pas(&mut app, "calque.dessus")], ["V9", "V10", "V11"]);
    // En haut de la pile : la ligne du dossier, au-dessus du premier de ses calques ; on ne s'arrête jamais sur elle.
    assert_eq!(pas(&mut app, "calque.dessus"), "V11", "au bout : on reste sur le dernier calque, jamais sur le dossier");
    assert_eq!(lignes(&app).iter().map(|l| (l.0, l.1)).collect::<Vec<_>>(), [(Sorte::Dessus, MORE_ROW), (Sorte::Dossier, 0), (Sorte::Calque, 1), (Sorte::Calque, 2), (Sorte::Dessous, MORE_ROW)]);
    // On redescend et l'on sort : il se referme tout seul.
    for attendu in ["V10", "V9", "V8", "V13"] {
        assert_eq!(pas(&mut app, "calque.dessous"), attendu);
    }
    assert!(dossier_ferme(app.layers(), d2, app.cur_layer), "sorti du dossier : il est refermé");
    assert_eq!(lignes(&app).iter().filter(|l| l.0 == Sorte::Calque).map(|l| l.1).collect::<Vec<_>>(), [5, 7]);
    // Vers le bas, la ligne de « Dossier 1 » se passe d'un pas : de « V13 » à « Dessin ».
    assert_eq!(pas(&mut app, "calque.dessous"), "Dessin");
    // Rien n'a été écrit pour tout cela : le drapeau du dossier n'a pas bougé, le projet n'est pas modifié.
    assert!(app.layers()[d2].collapsed && !app.modified);
    // Un clic sur une fine bande saute de trois calques, toujours sans s'arrêter sur un dossier.
    app.aller_au_calque(1, crate::pistes::SAUT);
    assert_eq!(courant(&app), "V9", "Dessin, puis V13, V8, V9");
}

#[test]
fn clicking_the_line_of_a_folder_closes_or_opens_it() {
    let mut app = app_de_retakes("basculer", false, "V13");
    let d1 = rang(&app, "Dossier 1");
    app.modified = false;
    app.basculer_dossier(d1);
    assert!(app.layers()[d1].collapsed && app.modified, "le même drapeau que dans le panneau Calques : il s'enregistre");
    assert_eq!(lignes(&app).iter().filter(|l| l.0 == Sorte::Calque).map(|l| l.1).collect::<Vec<_>>(), [4, 5, 10], "V8, V13, puis Réf : les calques de « Dossier 1 » sont rangés");
    app.basculer_dossier(d1);
    assert!(!app.layers()[d1].collapsed);
    // Fermé pendant qu'on est dedans : il reste ouvert le temps du passage, et le dit.
    aller(&mut app, "V5");
    app.basculer_dossier(d1);
    assert!(app.layers()[d1].collapsed && dossier_de_passage(app.layers(), d1, app.cur_layer));
    assert!(app.status.contains("refermera"), "{}", app.status);
    assert_eq!(lignes(&app).iter().filter(|l| l.0 == Sorte::Calque).map(|l| l.1).collect::<Vec<_>>(), [7, 8, 9]);
    // Un calque n'est pas un dossier : rien.
    let avant = app.layers().to_vec();
    app.basculer_dossier(rang(&app, "V13"));
    assert!(app.layers() == avant.as_slice());
}

// ------------------------------------------------------------------------------------ la couleur du rangement

#[test]
fn the_colour_of_a_folder_runs_along_its_layers_and_never_replaces_the_colour_of_their_kind() {
    use crate::folders::couleur_du_rangement;
    let mut app = app_de_retakes("couleurs", false, "V13");
    let groupes = app.doc.groups.clone();
    let de = |app: &App, nom: &str| couleur_du_rangement(app.layers(), rang(app, nom), &groupes);
    let couleur = |g: usize| Some(crate::layerheads::group_color(groupes[g].color));
    // Sans rien choisir : le premier dossier de la pile est bleu, le second magenta ; leurs calques prennent leur couleur.
    assert_eq!(de(&app, "Dossier 2"), couleur(7));
    assert_eq!(de(&app, "Dossier 1"), couleur(11));
    for nom in ["V11", "V10", "V9", "V8"] {
        assert_eq!(de(&app, nom), couleur(7), "{nom}");
    }
    assert_eq!(de(&app, "V5"), couleur(11));
    // Un calque hors dossier, du groupe « Défaut » : pas de filet.
    assert_eq!(de(&app, "V13"), None);
    assert_eq!(de(&app, "Réf"), None);
    // Un groupe de couleur choisi à la main passe avant : celui du dossier pour ses calques, le sien pour un calque seul.
    let (d1, v13, v5) = (rang(&app, "Dossier 1"), rang(&app, "V13"), rang(&app, "V5"));
    app.layers_mut()[d1].group = 8;
    app.layers_mut()[v13].group = 12;
    app.layers_mut()[v5].group = 3;
    assert_eq!(de(&app, "Dossier 1"), couleur(8));
    assert_eq!(de(&app, "V5"), couleur(8), "rangé : la couleur de son dossier, pas la sienne");
    assert_eq!(de(&app, "V13"), couleur(12));
    // La couleur de la bande reste celle de la sorte du calque : vectoriel et bitmap ne se confondent pas avec un dossier.
    let (v, b) = (crate::theme::couleur_du_calque(&app.layers()[v5]), crate::theme::couleur_du_calque(&app.layers()[rang(&app, "Réf")]));
    assert_ne!(v, b);
}

// ------------------------------------------------------------------------------------ le zoom suit le pointeur

#[test]
fn the_zoom_keys_of_the_image_zoom_the_strip_when_the_pointer_is_over_it() {
    let mut app = app_de_retakes("zoom", false, "V13");
    app.set_espace(Espace::Montage);
    let bande = app.montage_zoom;
    // Le pointeur sur la bande : les touches du zoom de l'image zooment le montage.
    app.bande_survolee = true;
    app.run_action("vue.zoom_avant");
    assert!(app.montage_zoom > bande, "{} > {bande}", app.montage_zoom);
    app.run_action("vue.zoom_arriere");
    assert!((app.montage_zoom - bande).abs() < 1e-4);
    // Le pointeur ailleurs : la bande ne bouge pas.
    app.bande_survolee = false;
    app.run_action("vue.zoom_avant");
    assert_eq!(app.montage_zoom, bande);
    // Les touches à part de la bande marchent toujours, où que soit le pointeur.
    app.run_action("bande.zoom_avant");
    assert!(app.montage_zoom > bande);
}

// ------------------------------------------------------------------------------------ à l'écran

fn banc_de_retakes(dossier_2_ferme: bool, courant: &str) -> Banc {
    let mut b = banc_taille(film_de_retakes(dossier_2_ferme), false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = PPF;
    b.app.settings.montage_pile = Pile::Trois;
    b.app.montage_head_w = 180.0;
    (b.app.cur_panel, b.app.cur_frame) = (0, 0);
    b.app.cur_layer = rang(&b.app, courant);
    b.garder_formes = true;
    b.garder_textes = true;
    b.chauffer(8);
    b
}

fn trouver(t: &[(String, Rect)], texte: &str) -> Vec<Rect> {
    t.iter().filter(|(s, _)| s.contains(texte)).map(|(_, r)| *r).collect()
}

/// Les textes de la colonne de gauche de la bande du montage (les noms des pistes, les lignes de dossier, les fines bandes) : en bas
/// de l'écran, dans les 180 points de la colonne. Le panneau Calques, plus haut et à droite, écrit les mêmes noms : il n'en est pas.
fn colonne(b: &Banc) -> Vec<(String, Rect)> {
    textes(b).into_iter().filter(|(_, r)| r.left() < 190.0 && r.center().y > 600.0).collect()
}

#[test]
fn on_screen_the_window_shows_three_layers_the_folder_line_and_the_two_strips() {
    let mut b = banc_de_retakes(true, "Dessin");
    let t = colonne(&b);
    // Le calque actif, ses deux voisins, et rien d'autre de la pile.
    for nom in ["V13", "Dessin", "V5"] {
        assert!(!trouver(&t, nom).is_empty(), "« {nom} » manque : {t:?}");
    }
    for nom in ["V11", "V10", "V9", "V8", "V4", "Réf"] {
        assert!(trouver(&t, nom).is_empty(), "« {nom} » ne devrait pas avoir de piste : {t:?}");
    }
    // Les deux fines bandes comptent les autres, ceux du dossier fermé compris.
    assert_eq!(trouver(&t, "4 au-dessus").len(), 1, "{t:?}");
    assert_eq!(trouver(&t, "2 au-dessous").len(), 1, "{t:?}");
    // « Dossier 1 » : sa ligne fine entre « V13 » et « Dessin », et son nom au-dessus de celui du calque actif.
    let dossier = trouver(&t, "Dossier 1");
    assert_eq!(dossier.len(), 2, "{t:?}");
    let (v13, dessin) = (trouver(&t, "V13")[0], trouver(&t, "Dessin")[0]);
    let ligne = dossier.iter().copied().min_by(|a, c| a.center().y.total_cmp(&c.center().y)).unwrap();
    assert!(ligne.center().y > v13.center().y && ligne.center().y < dessin.center().y, "la ligne du dossier est entre les deux calques");
    // Un clic sur la ligne le ferme : on est dedans, il reste ouvert le temps du passage.
    let d1 = rang(&b.app, "Dossier 1");
    // (Le pointeur arrive d'ailleurs : à sa toute première apparition, egui montre les bulles sans attendre la seconde, et celle d'une
    // ligne voisine recouvre alors la ligne visée — un travers du banc d'essai, pas de la fenêtre d'Anthony.)
    b.image(vec![Event::PointerMoved(pos2(900.0, 300.0))], 1.0 / 60.0);
    b.chauffer(2);
    clic(&mut b, ligne.center(), Modifiers::NONE);
    assert!(b.app.layers()[d1].collapsed && b.app.modified, "le clic a fermé le dossier");
    assert_eq!(b.app.layers()[b.app.cur_layer].name, "Dessin", "le dossier n'est pas devenu le calque courant");
    assert!(!trouver(&colonne(&b), "de passage").is_empty());
    // Le pointeur resté sur la ligne plus d'une seconde (le temps d'une bulle) : un clic rouvre le dossier, un autre le referme.
    b.image(Vec::new(), 1.6);
    b.chauffer(3);
    clic(&mut b, ligne.center(), Modifiers::NONE);
    assert!(!b.app.layers()[d1].collapsed, "un second clic rouvre le dossier");
    b.image(vec![Event::PointerMoved(ligne.center() + vec2(3.0, 0.0))], 1.0 / 60.0);
    clic(&mut b, ligne.center() + vec2(3.0, 0.0), Modifiers::NONE);
    assert!(b.app.layers()[d1].collapsed);
    // Un clic sur le voisin du dessus le rend actif : il vient au milieu, et « Dossier 1 » se referme derrière lui.
    clic(&mut b, v13.center(), Modifiers::NONE);
    b.chauffer(4);
    assert_eq!(b.app.layers()[b.app.cur_layer].name, "V13");
    let t = colonne(&b);
    assert!(trouver(&t, "Dessin").is_empty() && trouver(&t, "de passage").is_empty(), "{t:?}");
    assert!(!trouver(&t, "Réf").is_empty(), "le voisin du dessous est « Réf », après les deux dossiers fermés : {t:?}");
    // La fine bande du haut : un clic entre dans « Dossier 2 » (trois calques plus haut).
    let bande = trouver(&t, "4 au-dessus")[0];
    clic(&mut b, bande.center(), Modifiers::NONE);
    b.chauffer(4);
    assert_eq!(b.app.layers()[b.app.cur_layer].name, "V10");
}

#[test]
fn on_screen_the_bottom_bar_sets_the_stack_and_the_height_of_the_plans() {
    use egui_phosphor::regular as ph;
    let mut b = banc_de_retakes(false, "V5");
    let hauteur = || crate::storyboard::BLOCS_DU_MONTAGE.with(|v| v.borrow().first().map(Rect::height));
    assert_eq!(hauteur(), Some(HauteurPlans::Moyenne.points() - 2.0), "les plans à la hauteur moyenne");
    // Les quatre icônes de la barre du bas : trois calques / toute la pile, plans moyens / en bandeau.
    let t = textes(&b);
    let icone = |t: &[(String, Rect)], i: &str| t.iter().filter(|(s, _)| s == i).map(|(_, r)| *r).max_by(|a, c| a.center().y.total_cmp(&c.center().y));
    let (toute, bandeau) = (icone(&t, ph::STACK).expect("l'icône « Toute la pile »"), icone(&t, ph::MINUS).expect("l'icône « Plans en bandeau »"));
    clic(&mut b, toute.center(), Modifiers::NONE);
    assert_eq!(b.app.settings.montage_pile, Pile::Toute);
    b.chauffer(4);
    let t = colonne(&b);
    for nom in ["V11", "V10", "V9", "V8", "V13", "Dessin", "V5", "V4", "Réf", "Dossier 2"] {
        assert!(!trouver(&t, nom).is_empty(), "« {nom} » manque dans toute la pile : {t:?}");
    }
    assert!(trouver(&t, "au-dessus").is_empty());
    clic(&mut b, bandeau.center(), Modifiers::NONE);
    assert_eq!(b.app.settings.montage_plans, HauteurPlans::Bandeau);
    b.chauffer(4);
    assert_eq!(hauteur(), Some(HauteurPlans::Bandeau.points() - 2.0), "les plans en bandeau");
    // Les touches (sans touche d'office : l'action) font la même chose dans l'autre sens.
    b.app.run_action("piste.toute_la_pile");
    b.app.run_action("bande.plans_en_bandeau");
    assert_eq!((b.app.settings.montage_pile, b.app.settings.montage_plans), (Pile::Trois, HauteurPlans::Moyenne));
}

// ------------------------------------------------------------------------------------ à regarder

/// Des images de la bande du montage dans le dossier `FEANOR_CAPTURE_DIR` : trois calques ou toute la pile, plans moyens ou en bandeau,
/// sur le film de retakes des essais ou, si `FEANOR_PROJET_ESSAI` donne le manifeste d'une COPIE de projet, sur son plan le plus chargé.
/// Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn calques_timeline_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let (doc, origine) = match std::env::var_os("FEANOR_PROJET_ESSAI") {
        Some(p) => (crate::project::load(std::path::Path::new(&p)).expect("le projet s'ouvre").doc, "projet"),
        None => (film_de_retakes(true), "essai"),
    };
    let plan = (0..doc.panels.len()).max_by_key(|&i| doc.panels[i].layers.len()).unwrap_or(0);
    // Le calque du milieu de la pile, s'il n'est pas un dossier (sinon le premier calque au-dessus).
    let pile = &doc.panels[plan].layers;
    let depart = (pile.len() / 2..pile.len()).find(|&k| !pile[k].folder).unwrap_or(0);
    println!("CALQUES plan {plan} : {} calques, calque de départ « {} »", pile.len(), pile[depart].name);
    let mut b = banc_taille(doc, true, 1920, 1080);
    // (Le film des essais, tout petit, se regarde dans l'espace Montage ; un vrai projet, dans le Storyboard où Anthony travaille.)
    b.app.set_espace(if origine == "projet" { Espace::Storyboard } else { Espace::Montage });
    b.app.montage_head_w = 180.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (plan, depart, 0);
    b.app.montage_fit = true;
    b.chauffer(10);
    let mut sauver = |b: &mut Banc, nom: &str| {
        b.chauffer(8);
        let Some(rgba) = b.capturer() else { return println!("CALQUES pas de carte graphique : {nom} ignorée") };
        let img = image::RgbaImage::from_raw(1920, 1080, rgba).expect("image");
        let bas = image::imageops::crop_imm(&img, 0, 560, 1920, 520).to_image();
        bas.save(dossier.join(format!("calques-{origine}-{nom}.png"))).expect("png");
        let rows = b.app.montage_rows(300.0);
        println!("CALQUES {nom} : plans {} points, pistes {} points, {} lignes", rows.block_h, rows.layers_h(), rows.lanes.len());
    };
    b.app.settings.montage_pile = Pile::Trois;
    b.app.settings.montage_plans = HauteurPlans::Moyenne;
    sauver(&mut b, "1-trois-moyens");
    b.app.settings.montage_plans = HauteurPlans::Bandeau;
    sauver(&mut b, "2-trois-bandeau");
    b.app.run_action("calque.dessus");
    sauver(&mut b, "3-trois-bandeau-un-cran-plus-haut");
    b.app.settings.montage_pile = Pile::Toute;
    b.app.settings.montage_plans = HauteurPlans::Moyenne;
    sauver(&mut b, "4-toute-la-pile");
}
