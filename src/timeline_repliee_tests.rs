//! Montage plus lisible (Anthony, 2026-10-28, branche `timeline-repliee`) : seul le calque actif est déplié dans le Storyboard et le
//! Montage (tous les calques restent dépliés dans l'Animation) ; les clés de tous les calques d'un plan sont de petits traits de la
//! couleur de leur sorte sur son bloc ; un calque de couleur lié n'a plus de piste, un liseré rose sous le bloc de son trait le montre ; un
//! bloc n'écrit son nom que s'il diffère de celui de sa rangée ; au repos, rien ne repeint et rien ne se recalcule.
//!
//! (Les gestes, les hauteurs, le mode « cachés » en pur et les menus sont dans `pistes_tests.rs`.)

use eframe::egui::{epaint::Shape, Color32, Modifiers, Rect};

use crate::clesplan::{cles_du_plan, ClesBlocs};
use crate::document::{Cel, LayerKind};
use crate::montagerows::{layer_in_row, row_count, row_of_layer, LAYER_H};
use crate::perf_tests::Banc;
use crate::pistes::{Pile, FOLDED_H};
use crate::pistes_tests::{app_with, banc_montage, clic, doc_with, lieu, losanges, textes};
use crate::poskeys::PosKey;
use crate::workspace::Espace;

fn pose(x: f32) -> crate::document::LayerTransform {
    crate::document::LayerTransform { x, ..Default::default() }
}

fn rects_de(shape: &Shape, fill: Color32, out: &mut Vec<Rect>) {
    match shape {
        Shape::Rect(r) if r.fill == fill => out.push(r.rect),
        Shape::Vec(v) => v.iter().for_each(|s| rects_de(s, fill, out)),
        _ => {}
    }
}

/// Les rectangles pleins de cette couleur peints par la dernière image.
fn rects(b: &Banc, fill: Color32) -> Vec<Rect> {
    let mut out = Vec::new();
    for s in &b.formes {
        rects_de(&s.shape, fill, &mut out);
    }
    out
}

/// Combien de fois ce texte est peint par la dernière image.
fn combien(b: &Banc, texte: &str) -> usize {
    textes(b).iter().filter(|(s, _)| s == texte).count()
}

/// Un plan de 24 images avec un calque vectoriel « Perso » (dessins aux images 0 et 12, clés de position 4 et 10), un calque bitmap
/// « Fond » (un dessin, clé de position 6) et un calque de couleur lié à « Perso » (qui suit ses dessins).
fn plan_a_couleur() -> crate::document::Document {
    let mut doc = doc_with(&[&["Perso", "Fond"]]);
    let cels: Vec<_> = (0..4).map(|_| doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap))).collect();
    let mut couleur = {
        let layers = &mut doc.panels[0].layers; // du bas vers le haut : Fond, Perso
        layers[1].frames = (0..24).map(|f| Some(if f < 12 { cels[0] } else { cels[1] })).collect();
        layers[1].pos_keys = vec![PosKey::new(4, pose(0.0)), PosKey::new(10, pose(5.0))];
        layers[0].kind = LayerKind::Bitmap;
        layers[0].frames = vec![Some(cels[2]); 24];
        layers[0].pos_keys = vec![PosKey::new(6, pose(0.0))];
        crate::calquecouleur::nouveau(&layers[1], "Perso couleur".into())
    };
    // Ses images sont celles du trait.
    couleur.frames = (0..24).map(|f| Some(if f < 12 { cels[2] } else { cels[3] })).collect();
    doc.panels[0].layers.insert(1, couleur);
    doc
}

// ------------------------------------------------------------------------------------ les clés sur le bloc du plan

#[test]
fn the_keys_of_every_layer_of_the_plan_are_summed_up_with_the_colour_of_their_kind() {
    let doc = plan_a_couleur();
    let cles = cles_du_plan(&doc.panels[0]);
    let frames: Vec<usize> = cles.iter().map(|c| c.0).collect();
    // Dessins : 0 et 12 (Perso, sa couleur), 0 (Fond) ; positions : 4 et 10 (Perso), 6 (Fond). Par image croissante.
    assert_eq!(frames, [0, 4, 6, 10, 12]);
    let vecto = crate::theme::couleur_calque(LayerKind::Vector, false);
    let bitmap = crate::theme::couleur_calque(LayerKind::Bitmap, false);
    let rose = crate::theme::couleur_calque(LayerKind::Bitmap, true);
    assert!(vecto != bitmap && rose != bitmap && rose != vecto, "trois couleurs de sorte différentes");
    let at = |f: usize| cles.iter().find(|c| c.0 == f).map(|c| c.1.clone()).unwrap();
    // (Le calque de couleur lié copie les clés de position de son trait : il en a aux mêmes images, en rose.)
    assert!(at(4).contains(&vecto) && at(4).contains(&rose) && at(4).len() == 2, "une clé de position d'un calque vectoriel : la couleur du vectoriel : {:?}", at(4));
    assert_eq!(at(6), [bitmap], "une clé de position d'un calque bitmap : la couleur du bitmap");
    assert!(at(12).contains(&rose) && at(12).contains(&vecto) && at(12).len() == 2, "le calque de couleur lié (l'aplat) compte, en rose, avec son trait : {:?}", at(12));
    let zero = at(0);
    assert!(zero.contains(&vecto) && zero.contains(&bitmap) && zero.contains(&rose), "à l'image 0, les trois : {zero:?}");
    // Les clés d'effets comptent aussi.
    let mut doc2 = doc_with(&[&["Seul"]]);
    let mut blur = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
    blur.set_key(7, vec![3.0]);
    doc2.panels[0].layers[0].fx = vec![blur];
    assert_eq!(cles_du_plan(&doc2.panels[0]).iter().map(|c| c.0).collect::<Vec<_>>(), [7]);
    // Les images au-delà du plan ne comptent pas.
    doc2.panels[0].layers[0].pos_keys = vec![PosKey::new(40, pose(0.0))];
    assert_eq!(cles_du_plan(&doc2.panels[0]).len(), 1);
}

#[test]
fn the_summary_is_computed_once_per_change_of_the_document() {
    let doc = plan_a_couleur();
    let mut cache = ClesBlocs::default();
    let a = cache.pour(1, &doc.panels[0], false);
    let b = cache.pour(1, &doc.panels[0], false);
    assert_eq!(cache.calculs, 1, "les mêmes clés, gardées");
    assert!(std::sync::Arc::ptr_eq(&a, &b));
    cache.pour(2, &doc.panels[0], false);
    assert_eq!(cache.calculs, 2, "le numéro de l'historique a changé : recalculées");
    cache.pour(2, &doc.panels[0], true);
    assert_eq!(cache.calculs, 3, "souris tenue : un geste est peut-être en cours");
    // Le plan a changé sans étape d'historique (un équipier) : ses calques ne sont plus les mêmes.
    let mut doc2 = plan_a_couleur();
    doc2.panels[0].layers.pop();
    cache.pour(2, &doc2.panels[0], false);
    assert_eq!(cache.calculs, 4);
}

#[test]
fn the_keys_are_drawn_on_the_plan_block_in_the_colour_of_their_kind_and_at_rest_nothing_recomputes_or_repaints() {
    let mut b = banc_montage();
    // « Décor » (rang 2) est bitmap, « Perso » (rang 3) et « Fond » (rang 1) vectoriels.
    b.app.doc.panels[0].layers[2].kind = LayerKind::Bitmap;
    b.app.record_step();
    b.chauffer(6);
    let l = lieu(&b);
    let vecto = crate::theme::couleur_calque(LayerKind::Vector, false);
    let bitmap = crate::theme::couleur_calque(LayerKind::Bitmap, false);
    let trait_a = |b: &Banc, fill: Color32, g: usize| {
        rects(b, fill).into_iter().find(|r| (r.center().x - l.x(g)).abs() < 0.6 && r.width() <= 3.1 && r.height() <= 5.1 && r.height() > 0.5)
    };
    // Image 4 : la seule clé de Perso (vectoriel) ; image 6 : la seule de Décor (bitmap) ; le trait est au bas du bloc, sous les pistes.
    let v = trait_a(&b, vecto, 4).expect("le trait vectoriel de l'image 4");
    let m = trait_a(&b, bitmap, 6).expect("le trait bitmap de l'image 6");
    assert!(trait_a(&b, bitmap, 4).is_none() && trait_a(&b, vecto, 6).is_none(), "chaque clé a la couleur de son type");
    assert!((v.bottom() - m.bottom()).abs() < 0.1, "tous les traits sont alignés le long du bas du bloc");
    assert!(v.bottom() < l.layers_top, "sur le bloc du plan, au-dessus des pistes de calques");
    // Un fin contour sombre derrière chaque trait.
    assert!(rects(&b, Color32::from_black_alpha(150)).iter().any(|r| (r.center().x - l.x(4)).abs() < 0.6), "le contour sombre");
    // Au repos : rien ne se recalcule ni ne repeint.
    b.chauffer(4);
    let calculs = b.app.cles_blocs.calculs;
    let repos: Vec<std::time::Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "pas de repeinture continue : {repos:?}");
    assert_eq!(b.app.cles_blocs.calculs, calculs, "aucun recalcul au repos");
    // Une étape d'historique (ici une clé de plus à l'image 20) : les clés sont recalculées, le nouveau trait apparaît.
    b.app.record_step();
    b.app.doc.panels[0].layers[3].pos_keys.push(PosKey::new(20, pose(1.0)));
    b.chauffer(4);
    assert!(b.app.cles_blocs.calculs > calculs, "recalculées après le changement du document");
    assert!(trait_a(&b, vecto, 20).is_some(), "le trait de la nouvelle clé");
}

// ------------------------------------------------------------------------------------ le calque de couleur lié

#[test]
fn a_linked_colour_layer_has_no_row_of_its_own() {
    let doc = plan_a_couleur();
    let layers = &doc.panels[0].layers; // du bas vers le haut : Fond, Perso couleur, Perso
    assert_eq!((layers.len(), row_count(layers)), (3, 2));
    assert_eq!(layer_in_row(layers, 0), Some(2), "rangée 0 : Perso");
    assert_eq!(layer_in_row(layers, 1), Some(0), "rangée 1 : Fond ; le calque de couleur est sauté");
    assert_eq!(layer_in_row(layers, 2), None);
    assert_eq!((row_of_layer(layers, 2), row_of_layer(layers, 1), row_of_layer(layers, 0)), (Some(0), Some(0), Some(1)), "le calque de couleur est à la rangée de son trait");
    // Les pistes : deux, pas trois ; le calque de couleur courant rend son trait actif.
    let mut app = app_with("lie", doc);
    app.cur_layer = 1;
    app.pistes_follow_current(false);
    let lanes = app.montage_rows(300.0).lanes;
    assert_eq!(lanes.iter().map(|l| (l.row, l.h)).collect::<Vec<_>>(), [(0, LAYER_H), (1, FOLDED_H)], "le trait du calque de couleur courant est déplié");
    // Un calque de couleur libre (sans trait) garde sa rangée.
    app.doc.panels[0].layers[1].couleur.as_mut().unwrap().trait_id = 0;
    assert_eq!(row_count(&app.doc.panels[0].layers), 3);
}

#[test]
fn a_pink_edge_under_the_block_of_the_line_layer_says_the_colour_layer_exists_only_when_it_is_unfolded() {
    let mut b = banc_montage();
    // Un calque de couleur sous « Perso » (rang 3) ; la structure change comme à la main.
    b.app.cur_layer = 3;
    b.app.new_colour_layer();
    b.chauffer(8);
    let layers = b.app.doc.panels[0].layers.clone();
    assert_eq!(layers.len(), 5);
    assert!(layers[b.app.cur_layer].keys_follow_line());
    let rose = crate::theme::couleur_calque(LayerKind::Bitmap, true);
    let liseres = |b: &Banc| rects(b, rose).into_iter().filter(|r| (r.height() - 3.0).abs() < 0.01 && r.width() > 20.0).collect::<Vec<_>>();
    // Le calque de couleur est courant : son trait (rangée 0) est actif, déplié, et porte le liseré, sous son bloc.
    let rows = b.app.montage_rows(300.0);
    assert_eq!(rows.lanes.len(), 4, "quatre pistes pour cinq calques : pas de piste pour le calque de couleur");
    assert!(!rows.lanes[0].folded);
    assert!(!liseres(&b).is_empty(), "un liseré rose sous le bloc du trait");
    // Replié (un autre calque devient actif) : plus de liseré.
    b.app.select_layer(1);
    b.chauffer(8);
    assert!(b.app.track_is_folded(0), "le trait est replié");
    assert!(liseres(&b).is_empty(), "rien sous une bande fine");
}

// ------------------------------------------------------------------------------------ les noms des blocs

#[test]
fn a_block_writes_its_name_only_when_it_differs_from_the_name_of_its_row() {
    let mut b = banc_montage();
    // Les noms de la colonne de gauche (et d'un éventuel panneau Calques) comptent : « Fond » est replié, il n'est écrit que là.
    let base = combien(&b, "Fond");
    assert!(base >= 1);
    // Rangée 0 : « Perso » dans le plan 1 (le même nom que la colonne : le bloc n'écrit rien) et « Autre » dans le plan 2 (qui diffère :
    // il s'écrit dans son bloc). Les bandes fines n'écrivent aucun nom dans leurs blocs.
    assert_eq!(combien(&b, "Perso"), base, "le bloc de Perso n'écrit rien de plus que la colonne");
    assert_eq!(combien(&b, "Autre"), 1, "le bloc « Autre » écrit son nom, qui diffère de celui de sa rangée");
    for nom in ["Décor", "Vide"] {
        assert_eq!(combien(&b, nom), base, "{nom} : une bande fine, seulement dans la colonne");
    }
    // Même nom dans le plan 2 : le bloc n'écrit rien.
    b.app.doc.panels[1].layers[0].name = "Perso".to_string();
    b.app.record_step();
    b.chauffer(6);
    assert_eq!(combien(&b, "Perso"), base, "plus de nom répété dans les blocs");
    assert_eq!(combien(&b, "Autre"), 0);
}

// ------------------------------------------------------------------------------------ les onglets

#[test]
fn only_the_active_layer_is_unfolded_in_the_storyboard_and_the_montage_but_every_layer_in_the_animation() {
    for espace in [Espace::Storyboard, Espace::Montage] {
        let mut b = banc_montage();
        b.app.set_espace(espace);
        b.chauffer(8);
        let hs: Vec<f32> = b.app.montage_rows(300.0).lanes.iter().map(|l| l.h).collect();
        assert_eq!(hs, [LAYER_H, FOLDED_H, FOLDED_H, FOLDED_H], "{espace:?}");
    }
    // L'Animation : sa timeline (tous les calques dépliés, comme avant) ne lit pas l'état des pistes du montage.
    let mut b = banc_montage();
    b.app.set_espace(Espace::Animation);
    b.chauffer(8);
    let rows = crate::layerheads::rows_of(b.app.layers(), &b.app.doc.groups);
    assert!(rows.iter().all(|r| r.h == crate::layerheads::ROW_FULL), "tous les calques de la timeline sont dépliés");
    assert_eq!(rows.len(), 4);
    for nom in ["Perso", "Décor", "Fond", "Vide"] {
        assert!(combien(&b, nom) >= 1, "« {nom} » manque dans la timeline de l'animation");
    }
    assert!(b.app.doc.panels[0].layers.iter().all(|l| !l.collapsed), "rien n'a été écrit dans les calques");
}

/// (2026-11-11, branche `calques-timeline` : le mode « cachés » n'existe plus ; « trois calques » le remplace. Le calque actif — « Perso »,
/// en haut de la pile — a son voisin du dessous, et une fine bande compte les deux autres ; un clic dessus y saute.)
#[test]
fn the_three_layers_mode_shows_a_strip_that_a_click_jumps_into() {
    let mut b = banc_montage();
    let base = combien(&b, "Fond");
    b.app.settings.montage_pile = Pile::Trois;
    b.chauffer(6);
    let t = textes(&b);
    let ligne = |t: &[(String, Rect)], fin: &str| t.iter().find(|(s, _)| s.contains(fin)).map(|(_, r)| *r);
    assert!(ligne(&t, "2 au-dessous").is_some(), "{t:?}");
    assert!(ligne(&t, "Haut de la pile").is_some(), "{t:?}");
    assert!(combien(&b, "Fond") < base, "« Fond » n'est plus dans la fenêtre");
    assert_eq!(b.app.montage_rows(300.0).lanes.len(), 4, "la bande du haut, le calque actif, son voisin, la bande du bas");
    // Les losanges du calque actif restent ceux d'une piste dépliée.
    assert_eq!(losanges(&b, crate::theme::POIGNEE_CALQUE).len(), 3);
    // Un clic sur la fine bande descend de trois calques : au bout de la pile.
    let avant = b.app.cur_layer;
    clic(&mut b, ligne(&t, "2 au-dessous").unwrap().center(), Modifiers::NONE);
    b.chauffer(4);
    assert_eq!(b.app.cur_layer, 0, "de {avant} au dernier calque");
    let t = textes(&b);
    assert!(ligne(&t, "Bas de la pile").is_some() && ligne(&t, "2 au-dessus").is_some(), "{t:?}");
}
