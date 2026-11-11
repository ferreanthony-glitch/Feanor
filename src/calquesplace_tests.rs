//! Les calques de la timeline prennent la place du panneau (Anthony, 2026-11-11, partie 140) : « La taille la plus grande reste la même
//! mais quand il y a plus de place pour afficher les calques, les calques les plus proches du calque sélectionné prennent aussi la taille
//! la plus grande », « il faut que ce soit toujours le plus confortable possible ».
//!
//! `pistes::agrandir` : du plus proche au plus loin du calque actif, le dessus avant le dessous, tant qu'il reste de la place ; les
//! lignes de dossier et les fines bandes gardent leur hauteur. Le dernier essai passe par la vraie fenêtre : la bande ne se met pas à
//! défiler à cause des calques agrandis.

use crate::calques_timeline_tests::{banc_de_retakes, film_de_retakes, rang};
use crate::montagerows::{layer_in_row, Rows, LAYER_H, SOUS_LA_BANDE};
use crate::pistes::{agrandir, lanes_in, Lane, Pile, Replis, Sorte, DOSSIER_H, FOLDED_H, VOISIN_H};
use crate::pistes_tests::app_with;

// ------------------------------------------------------------------------------------ l'outillage

/// Les lignes du premier plan du film de retakes (« Dossier 2 » ouvert), le calque `courant` actif, dans ce mode.
fn lignes(courant: &str, mode: Pile) -> (crate::app::App, Vec<Lane>) {
    let mut app = app_with(&format!("place-{courant}"), film_de_retakes(false));
    app.settings.montage_pile = mode;
    app.cur_layer = rang(&app, courant);
    app.pistes_follow_current(false);
    let lanes = lanes_in(&app.doc.panels, 0, app.cur_layer, 11, app.pistes_auto_open(), &Replis { mode, ..Replis::default() });
    (app, lanes)
}

/// Les noms des calques à la grande hauteur, de haut en bas.
fn grands(app: &crate::app::App, lanes: &[Lane]) -> Vec<String> {
    let layers = &app.doc.panels[0].layers;
    lanes
        .iter()
        .filter(|l| l.sorte == Sorte::Calque && !l.folded)
        .map(|l| {
            assert_eq!(l.h, LAYER_H, "un calque déplié a la grande hauteur");
            layers[layer_in_row(layers, l.row).expect("le calque de la rangée")].name.clone()
        })
        .collect()
}

fn total(lanes: &[Lane]) -> f32 {
    lanes.iter().map(|l| l.h).sum()
}

/// Les hauts se suivent sans trou ni recouvrement.
fn sans_trou(lanes: &[Lane]) {
    let mut top = 0.0;
    for l in lanes {
        assert_eq!(l.top, top, "le haut de la rangée {} ({:?})", l.row, l.sorte);
        top += l.h;
    }
}

// ------------------------------------------------------------------------------------ la règle

#[test]
fn with_more_room_the_layers_nearest_to_the_active_one_take_the_big_height_one_by_one() {
    // Toute la pile, du haut vers le bas : Dossier 2 [V11, V10, V9, V8], V13, Dossier 1 [Dessin, V5 (actif), V4], Réf.
    let (app, base) = lignes("V5", Pile::Toute);
    let juste = total(&base);
    assert_eq!(juste, 2.0 * DOSSIER_H + 8.0 * FOLDED_H + LAYER_H);
    assert_eq!(grands(&app, &base), ["V5"]);
    let gain = LAYER_H - FOLDED_H;
    let avec = |place: f32| {
        let mut l = base.clone();
        agrandir(&mut l, place);
        sans_trou(&l);
        l
    };
    // Pas de place en plus (ou moins de place que les pistes : la bande défile) : rien ne change.
    assert_eq!(grands(&app, &avec(juste)), ["V5"]);
    assert_eq!(grands(&app, &avec(juste - 80.0)), ["V5"]);
    assert_eq!(grands(&app, &avec(juste + gain - 1.0)), ["V5"], "pas tout à fait la place d'un calque de plus");
    // La place d'un calque : celui du dessus. De deux : celui du dessous aussi. Puis les suivants, de proche en proche, par-dessus
    // la ligne d'un dossier.
    assert_eq!(grands(&app, &avec(juste + gain)), ["Dessin", "V5"]);
    assert_eq!(grands(&app, &avec(juste + 2.0 * gain)), ["Dessin", "V5", "V4"]);
    assert_eq!(grands(&app, &avec(juste + 3.0 * gain)), ["V13", "Dessin", "V5", "V4"]);
    assert_eq!(grands(&app, &avec(juste + 4.0 * gain)), ["V13", "Dessin", "V5", "V4", "Réf"]);
    assert_eq!(grands(&app, &avec(juste + 5.0 * gain)), ["V8", "V13", "Dessin", "V5", "V4", "Réf"]);
    // Toute la place qu'on veut : tous les calques sont grands, jamais plus grands ; les lignes de dossier ne changent pas.
    let tout = avec(5000.0);
    assert_eq!(grands(&app, &tout).len(), 9);
    assert_eq!(total(&tout), 2.0 * DOSSIER_H + 9.0 * LAYER_H);
    assert!(tout.iter().filter(|l| l.sorte == Sorte::Dossier).all(|l| l.h == DOSSIER_H));
    // La place prise ne dépasse jamais la place offerte.
    for k in 0..40 {
        let place = juste + 7.0 * k as f32;
        assert!(total(&avec(place)) <= place.max(juste), "{place}");
    }
}

#[test]
fn in_the_three_layers_window_the_two_neighbours_grow_and_the_strips_stay() {
    let (app, base) = lignes("V5", Pile::Trois);
    let juste = total(&base);
    assert_eq!(grands(&app, &base), ["V5"]);
    let gain = LAYER_H - VOISIN_H;
    let avec = |place: f32| {
        let mut l = base.clone();
        agrandir(&mut l, place);
        sans_trou(&l);
        l
    };
    assert_eq!(grands(&app, &avec(juste + gain - 1.0)), ["V5"]);
    assert_eq!(grands(&app, &avec(juste + gain)), ["Dessin", "V5"]);
    let tout = avec(5000.0);
    assert_eq!(grands(&app, &tout), ["Dessin", "V5", "V4"], "trois calques, pas un de plus");
    assert_eq!(total(&tout), juste + 2.0 * gain);
    // Les deux fines bandes gardent leur hauteur et leurs comptes.
    let bandes = |l: &[Lane]| l.iter().filter(|x| x.is_more()).map(|x| (x.h, x.compte)).collect::<Vec<_>>();
    assert_eq!(bandes(&tout), bandes(&base));
}

#[test]
fn nothing_grows_around_an_active_layer_folded_by_hand() {
    let (_, mut lanes) = lignes("V5", Pile::Toute);
    for l in lanes.iter_mut().filter(|l| l.sorte == Sorte::Calque) {
        l.h = FOLDED_H;
        l.folded = true;
    }
    let avant: Vec<f32> = lanes.iter().map(|l| l.h).collect();
    agrandir(&mut lanes, 5000.0);
    assert_eq!(lanes.iter().map(|l| l.h).collect::<Vec<_>>(), avant);
}

#[test]
fn the_rows_of_the_strip_use_the_height_of_the_panel_when_the_setting_is_on() {
    let (mut app, base) = lignes("V5", Pile::Toute);
    let n_son = app.doc.audio_tracks.len();
    // Ce que la bande montre sans les pistes de calques, et la place gardée sous elle.
    let sans = Rows { block_h: app.settings.montage_plans.points(), lanes: Vec::new() }.height(n_son);
    let haut = sans + SOUS_LA_BANDE + total(&base) + 2.0 * (LAYER_H - FOLDED_H);
    // Dans les essais, le réglage est éteint d'office : les hauteurs fixes d'avant.
    assert!(!app.settings.montage_grands);
    assert_eq!(grands(&app, &app.montage_rows(haut).lanes), ["V5"]);
    // Allumé (d'office dans l'application) : deux calques de plus tiennent, pas trois.
    app.settings.montage_grands = true;
    let rows = app.montage_rows(haut);
    assert_eq!(grands(&app, &rows.lanes), ["Dessin", "V5", "V4"]);
    assert!(rows.height(n_son) + SOUS_LA_BANDE <= haut + 0.01);
    // Un panneau trop bas pour toutes les pistes : rien ne grandit, la bande défile comme avant.
    assert_eq!(grands(&app, &app.montage_rows(120.0).lanes), ["V5"]);
    // Un autre calque devient actif : les grands calques le suivent.
    app.cur_layer = rang(&app, "V10");
    app.pistes_follow_current(false);
    assert_eq!(grands(&app, &app.montage_rows(haut).lanes), ["V11", "V10", "V9"]);
}

// ------------------------------------------------------------------------------------ à l'écran

#[test]
fn on_screen_the_layers_fill_the_strip_without_making_it_scroll() {
    let mut b = banc_de_retakes(false, "V5");
    b.app.settings.montage_pile = Pile::Toute;
    b.chauffer(6);
    let (contenu_avant, vue) = crate::montagehauteur::DEFILEMENT.with(|c| c.get());
    assert!(vue > 0.0 && contenu_avant <= vue + 0.5, "le banc montre toute la bande sans défiler : {contenu_avant} dans {vue}");
    b.app.settings.montage_grands = true;
    b.chauffer(6);
    let (contenu, vue) = crate::montagehauteur::DEFILEMENT.with(|c| c.get());
    // Les rangées que la fenêtre vient de montrer : celles de cette hauteur de panneau.
    let rows = b.app.montage_rows(vue);
    let n = grands(&b.app, &rows.lanes).len();
    assert!(n >= 2, "le panneau du banc a la place d'au moins un calque de plus ({vue} points) : {n}");
    assert!(contenu > contenu_avant, "les calques ont grandi : {contenu} > {contenu_avant}");
    assert!(contenu <= vue + 0.5, "la bande ne défile pas à cause d'eux : {contenu} dans {vue}");
    // La place est prise : il n'en reste pas pour un calque de plus (sauf si tous sont déjà grands).
    if n < 9 {
        assert!(vue - contenu < (LAYER_H - FOLDED_H) + SOUS_LA_BANDE, "il reste {} points", vue - contenu);
    }
    // Le calque actif n'a pas changé.
    assert_eq!(b.app.layers()[b.app.cur_layer].name, "V5");
}
