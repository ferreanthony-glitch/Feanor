//! Partie 179.10 (Anthony : « je veux que le repli des calques soit une option. D'origine, les calques ne changent pas de taille que
//! l'on soit sur le calque actif ou non. Remonte un peu l'icône de clé d'opacité, les flèches et les trois points pour les aligner
//! sur effet et opacité. »).
//!
//! 1. D'origine, le calque actif a la hauteur des autres (15 dans le Storyboard, 18 dans l'Animation), surligné, ses réglages en
//!    icônes à droite.
//! 2. « Déplier le calque actif » (le ⋯ de la ligne « Calques », les Préférences) le remet sur deux rangées, tout de suite ; un
//!    réglage du poste. L'espace Montage ne change pas.
//! 3. Sur la seconde rangée dépliée, ‹ ›, le losange de l'opacité et le ⋮ ont le centre vertical de la liste du mode et de la barre
//!    d'opacité.

use eframe::egui::{Event, Modifiers, Rect};
use egui_phosphor::regular as ph;

use crate::barrecalques::NOMS;
use crate::bulles::vus;
use crate::layerheads::Colonne;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::settings::Settings;
use crate::timelinesb_tests::{banc_sb, colonne, film, haut, rangees};
use crate::workspace::Espace;

/// Les hauteurs des pistes de calques du Storyboard (sans les lignes de dossier).
fn hauteurs(b: &Banc) -> Vec<f32> {
    rangees(b).lanes.iter().filter(|l| !l.est_dossier()).map(|l| l.h).collect()
}

/// La piste du calque actif à l'écran (haut, bas) : celle qui a le cadre du calque actif.
fn piste_du_calque_actif(b: &Banc) -> (f32, f32) {
    let rows = rangees(b);
    let h = haut(b) + rows.layers_top();
    let rang = crate::montagerows::row_of_layer(b.app.layers(), b.app.cur_layer).expect("la rangée du calque actif");
    let l = rows.lanes.iter().find(|l| l.row == rang).expect("la piste du calque actif");
    (h + l.top, h + l.top + l.h)
}

fn banc_animation() -> Banc {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    // (Le banc d'essai déplie le calque actif pour les essais d'avant : ici, le réglage d'origine.)
    b.app.settings.deplier_calque_actif = false;
    let perso = b.app.layers().iter().position(|l| l.name == "Perso").unwrap();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

#[test]
fn d_origine_le_calque_actif_a_la_hauteur_des_autres_et_garde_ses_reglages_en_icones() {
    assert!(!Settings::default().deplier_calque_actif, "éteint d'origine");
    let mut b = crate::perf_tests::banc_taille(film(), false, 1920, 1080);
    b.app.set_espace(Espace::Storyboard);
    b.app.settings.storyboard_calques_replies = false;
    b.app.settings.deplier_calque_actif = false;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    let h = hauteurs(&b);
    assert!(h.iter().all(|h| (*h - crate::pistes::FOLDED_H).abs() < 0.5), "toutes les pistes à 15 points, le calque actif compris : {h:?}");
    assert!(vus::ou("Mode de fusion").is_empty(), "pas de seconde rangée");
    let (p0, p1) = piste_du_calque_actif(&b);
    let grip = colonne();
    for col in &Colonne::TOUTES[1..] {
        let Some(_) = col.etat(&b.app.layers()[b.app.cur_layer]) else { continue };
        let r = vus::ou(col.nom()).into_iter().find(|r| r.center().y > p0 && r.center().y < p1).unwrap_or_else(|| panic!("« {} » sur la piste du calque actif", col.nom()));
        assert!(r.right() <= grip.left() + 0.5, "« {} » dans la colonne : {r:?}", col.nom());
        assert!(textes(&b).iter().any(|(t, q)| t == col.icone() && (q.center().y - r.center().y).abs() < 4.0), "« {} » en icône", col.nom());
    }
    // Un clic dans une colonne marche toujours (une étape, Ctrl+Z).
    let avant = b.app.layers()[b.app.cur_layer].onion;
    let pelure = vus::ou(Colonne::Pelure.nom()).into_iter().find(|r| r.center().y > p0 && r.center().y < p1).unwrap();
    clic(&mut b, pelure.center(), Modifiers::NONE);
    assert_eq!(b.app.layers()[b.app.cur_layer].onion, !avant, "{}", b.app.status);
}

#[test]
fn d_origine_dans_l_animation_le_calque_courant_est_sur_une_ligne() {
    let _b = banc_animation();
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    assert!(lignes.iter().all(|l| (l.2 - l.1 - crate::layerheads::ROW_FINE).abs() < 0.5), "toutes les lignes à 18 points : {lignes:?}");
}

#[test]
fn deplier_le_calque_actif_dans_le_menu_de_la_ligne_calques_le_remet_sur_deux_rangees() {
    let mut b = crate::perf_tests::banc_taille(film(), false, 1920, 1080);
    b.app.set_espace(Espace::Storyboard);
    b.app.settings.storyboard_calques_replies = false;
    b.app.settings.deplier_calque_actif = false;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    let rows = rangees(&b);
    let h = haut(&b);
    let (y0, y1) = (h + rows.calques_top(), h + rows.calques_top() + crate::montagerows::SB_CALQUES_H);
    let plus = vus::ou(NOMS[5]).into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("le ⋯ de la ligne Calques");
    clic(&mut b, plus.center(), Modifiers::NONE);
    b.chauffer(2);
    let case = textes(&b).into_iter().find(|(t, _)| t == "Déplier le calque actif").expect("« Déplier le calque actif » dans le ⋯").1;
    clic(&mut b, case.center(), Modifiers::NONE);
    assert!(b.app.settings.deplier_calque_actif, "allumé");
    assert!(Settings::load().deplier_calque_actif, "gardé dans les réglages du poste");
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(4);
    let h = hauteurs(&b);
    assert_eq!(h.iter().filter(|h| (**h - crate::montagerows::LAYER_H).abs() < 0.5).count(), 1, "le calque actif sur deux rangées : {h:?}");
    assert_eq!(vus::ou("Mode de fusion").len(), 1);
    // Éteint de nouveau : tout de suite.
    b.app.regler_deplier_calque_actif(false);
    b.chauffer(4);
    assert!(hauteurs(&b).iter().all(|h| (*h - crate::pistes::FOLDED_H).abs() < 0.5));
}

#[test]
fn le_reglage_est_ecrit_seulement_allume_et_est_dans_les_preferences() {
    let mut s = Settings::default();
    assert!(!toml::to_string(&s).unwrap().contains("deplier_calque_actif"));
    s.deplier_calque_actif = true;
    let t = toml::to_string(&s).unwrap();
    assert!(t.contains("deplier_calque_actif = true"), "{t}");
    assert!(toml::from_str::<Settings>(&t).unwrap().deplier_calque_actif);
    assert!(crate::prefsonglets::REGLAGES.iter().any(|r| r.id == "montage.deplier_actif" && r.libelle == "Déplier le calque actif"));
    // L'Animation suit le même réglage.
    let mut b = banc_animation();
    b.app.regler_deplier_calque_actif(true);
    b.chauffer(4);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    let cur = b.app.cur_layer;
    assert!(lignes.iter().any(|l| l.0 == cur && (l.2 - l.1 - crate::layerheads::ROW_FULL).abs() < 0.5), "le calque courant sur deux rangées : {lignes:?}");
}

#[test]
fn l_espace_montage_garde_son_calque_deplie() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    b.app.settings.deplier_calque_actif = false;
    b.app.set_espace(Espace::Montage);
    b.chauffer(8);
    let rows = b.app.montage_rows(300.0);
    assert!(rows.lanes.iter().any(|l| l.h == 66.0), "le Montage : sa piste de 66 : {:?}", rows.lanes);
}

/// Le centre vertical de l'encre du texte `t` peint sur `y0..y1` à gauche de `droite` : l'union des rectangles des glyphes eux-mêmes
/// (pas la boîte de la ligne de texte, qui est la même pour une icône et pour un mot alors que leur dessin n'est pas au même niveau).
fn centre_de_l_encre(b: &Banc, t: &str, y0: f32, y1: f32, droite: f32) -> Option<f32> {
    use eframe::egui::epaint::Shape;
    fn chercher(s: &Shape, t: &str, out: &mut Vec<Rect>) {
        match s {
            Shape::Text(x) if x.galley.job.text == t => {
                let mut encre = Rect::NOTHING;
                for row in &x.galley.rows {
                    for g in &row.glyphs {
                        if g.uv_rect.size.x > 0.0 && g.uv_rect.size.y > 0.0 {
                            let min = x.pos + g.pos.to_vec2() + g.uv_rect.offset;
                            encre = encre.union(Rect::from_min_size(min, g.uv_rect.size));
                        }
                    }
                }
                if encre.is_positive() {
                    out.push(encre);
                }
            }
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, t, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, t, &mut v);
    }
    v.into_iter().find(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= droite + 1.0).map(|r| r.center().y)
}

/// Les centres de l'encre de ‹, ›, du losange et du ⋮, moins le milieu de la case de la liste du mode (celui de la barre
/// d'opacité), sur `y0..y1`, à gauche de `droite`.
fn ecarts(b: &Banc, y0: f32, y1: f32, droite: f32) -> Vec<(&'static str, f32)> {
    let mode = vus::ou("Mode de fusion").into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("la liste du mode");
    let barre = vus::ou("Opacité du calque").into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("la barre d'opacité");
    assert!((barre.center().y - mode.center().y).abs() < 0.6, "la liste et la barre au même niveau : {mode:?} / {barre:?}");
    let reference = mode.center().y;
    [("flèche ‹", ph::CARET_LEFT), ("flèche ›", ph::CARET_RIGHT), ("losange de l'opacité", ph::DIAMOND), ("⋮ du calque", ph::DOTS_THREE_VERTICAL)]
        .into_iter()
        .filter_map(|(q, g)| centre_de_l_encre(b, g, y0, y1, droite).map(|c| (q, c - reference)))
        .collect()
}

#[test]
fn sur_la_seconde_rangee_les_fleches_le_losange_et_le_menu_sont_au_niveau_du_mode_et_de_l_opacite() {
    let mut tout: Vec<(String, Vec<(&'static str, f32)>)> = Vec::new();
    for echelle in [100u16, 125, 150] {
        // Storyboard, le calque actif déplié, la colonne large (les flèches ‹ › n'y sont que quand la place y est).
        let mut b = banc_sb(film(), 1920, 1080, false);
        b.app.settings.echelle = echelle;
        b.app.regler_deplier_calque_actif(true);
        b.app.montage_head_w = 520.0;
        b.chauffer(8);
        let (p0, p1) = piste_du_calque_actif(&b);
        let e = ecarts(&b, p0 + crate::pistes::RANGEE_ACTIVE, p1, colonne().left());
        eprintln!("179.10 Storyboard {echelle} % : {e:?}");
        assert_eq!(e.len(), 4, "{echelle} % : les quatre sont là : {e:?}");
        tout.push((format!("Storyboard {echelle} %"), e));
        // Animation : la ligne du calque courant dépliée.
        let mut a = banc_animation();
        a.app.settings.echelle = echelle;
        a.app.regler_deplier_calque_actif(true);
        a.chauffer(8);
        let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
        let cur = a.app.cur_layer;
        let (h0, h1) = lignes.iter().find(|l| l.0 == cur).map(|l| (l.1, l.2)).unwrap();
        let e = ecarts(&a, h0 + 20.0, h1, f32::MAX);
        eprintln!("179.10 Animation {echelle} % : {e:?}");
        assert!(e.len() >= 3, "{echelle} % : le losange, le ⋮ (et les flèches s'il y a la place) : {e:?}");
        tout.push((format!("Animation {echelle} %"), e));
    }
    // Le centre de l'encre de chacune au milieu de la case du mode et de la barre d'opacité, à moins de 0,75 point, et jamais plus bas
    // que lui (au point près, l'arrondi au pixel ; mesuré avant : ‹ › et le losange 0,5 point plus haut, le ⋮ de l'Animation 1,5 à 2
    // points plus haut).
    assert!(tout.iter().all(|(_, e)| e.iter().all(|(_, d)| d.abs() <= 0.75 && *d <= 0.3)), "écart au milieu du mode et de l'opacité : {tout:?}");
}