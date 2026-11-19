//! Partie 179.15 (Anthony, sur les trois indicateurs de la rangée 1 d'un calque, le dossier du groupe de couleur, le + / − du sens
//! du pochoir et la punaise de la position verrouillée : « Non supprime les carrément. Ça sera accessible en clic droit ça suffit »).
//!
//! 1. Les trois indicateurs ne sont plus sur les lignes de calque, ni dans le Storyboard ni dans l'Animation (les effets et le maillon
//!    de la planche-mère restent).
//! 2. Le clic droit d'un calque, Apparence : « Groupe · Couleur d'étiquette », « Pochoir négatif » (actif seulement quand le pochoir
//!    est allumé), « Verrouiller la position » ; ils agissent, Ctrl+Z les défait.

use eframe::egui::{Event, Modifiers, PointerButton, Pos2};

use crate::bulles::vus;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::timelinesb_tests::{banc_sb, colonne, film, haut, rangees};

/// Les noms de bulle des trois indicateurs retirés, vus sur `y0..y1` à gauche de `droite`.
fn indicateurs(y0: f32, y1: f32, droite: f32) -> Vec<String> {
    vus::noms()
        .into_iter()
        .filter(|n| n == "Pochoir positif" || n == "Pochoir négatif" || n == "Verrouiller la position" || n.starts_with("Groupe : "))
        .filter(|n| vus::ou(n).iter().any(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= droite + 1.0))
        .collect()
}

fn clic_droit(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

fn survoler(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p - eframe::egui::vec2(0.0, 1.0))], 1.0 / 60.0);
    b.chauffer(3);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(4);
}

#[test]
fn les_trois_indicateurs_ne_sont_plus_sur_les_lignes_de_calque_du_storyboard() {
    let mut doc = film();
    doc.panels[0].layers[0].stencil = true;
    doc.panels[0].layers[0].position_locked = true;
    doc.panels[0].layers[0].group = 2;
    let b = banc_sb(doc, 1920, 1080, false);
    let rows = rangees(&b);
    let h = haut(&b);
    let v = indicateurs(h + rows.layers_top(), h + rows.background_top(), colonne().left());
    assert!(v.is_empty(), "plus d'indicateurs : {v:?}");
    // Les réglages restent (la pelure, le pochoir…), et le nom a la place libérée.
    assert!(!vus::ou(crate::layerheads::Colonne::Pochoir.nom()).is_empty());
}

#[test]
fn les_trois_indicateurs_ne_sont_plus_sur_les_lignes_de_l_animation() {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    let perso = b.app.layers().iter().position(|l| l.name == "Perso").unwrap();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.app.layers_mut()[perso].stencil = true;
    b.garder_formes = true;
    b.chauffer(8);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    let (y0, y1) = (lignes.iter().map(|l| l.1).fold(f32::MAX, f32::min), lignes.iter().map(|l| l.2).fold(0.0, f32::max));
    let v = indicateurs(y0, y1, f32::MAX);
    assert!(v.is_empty(), "plus d'indicateurs : {v:?}");
}

#[test]
fn le_clic_droit_d_un_calque_donne_le_groupe_le_sens_du_pochoir_et_la_position_et_ils_agissent() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let k = b.app.cur_layer;
    let rows = rangees(&b);
    let h = haut(&b) + rows.layers_top();
    let rang = crate::montagerows::row_of_layer(b.app.layers(), k).unwrap();
    let lane = rows.lanes.iter().find(|l| l.row == rang).copied().unwrap();
    // Sur le nom du calque actif.
    let p = eframe::egui::pos2(colonne().left() - 250.0, h + lane.top + 9.0);
    let ouvrir = |b: &mut Banc| {
        clic_droit(b, p);
        let app = textes(b).into_iter().find(|(t, _)| t.contains("Apparence")).expect("« Apparence » au clic droit").1;
        survoler(b, app.center());
    };
    ouvrir(&mut b);
    let t: Vec<String> = textes(&b).into_iter().map(|(t, _)| t).collect();
    for mot in ["Groupe · Couleur d'étiquette", "Pochoir négatif", "Verrouiller la position"] {
        assert!(t.iter().any(|x| x.contains(mot)), "« {mot} » au clic droit, Apparence : {t:?}");
    }
    // Verrouiller la position : une coche, une étape, Ctrl+Z.
    let n = b.app.history.names().0.len();
    let r = textes(&b).into_iter().find(|(t, _)| t.contains("Verrouiller la position")).unwrap().1;
    clic(&mut b, r.center(), Modifiers::NONE);
    assert!(b.app.layers()[k].position_locked, "la position verrouillée : {}", b.app.status);
    assert_eq!(b.app.history.names().0.len(), n + 1);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(!b.app.layers()[k].position_locked, "Ctrl+Z");
    // Le sens du pochoir n'agit que sur un pochoir allumé.
    b.app.layers_mut()[k].stencil = true;
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    ouvrir(&mut b);
    let r = textes(&b).into_iter().find(|(t, _)| t.contains("Pochoir négatif")).unwrap().1;
    clic(&mut b, r.center(), Modifiers::NONE);
    assert!(b.app.layers()[k].stencil_negative, "pochoir négatif : {}", b.app.status);
}
