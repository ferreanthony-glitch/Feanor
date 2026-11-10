//! Partie 136 : les réglages d'un calque, ses effets et leurs clés agissent sur tous les calques choisis ; retourner un calque à
//! trajectoire avec d'autres retourne aussi son chemin (Anthony, 2026-11-10, règle « la sélection agit sur tout »). Sur le banc des
//! essais de la partie 132 (`multicalques_tests.rs`) : un plan de quatre images, le calque 0 courant, les autres choisis.

use eframe::egui::pos2;

use crate::app::Tool;
use crate::blend::BlendMode;
use crate::document::LayerKind;
use crate::fx::EffectKind;
use crate::multicalques_tests::{banc, etapes, y_de};
use crate::perf_tests::Banc;

const V: LayerKind = LayerKind::Vector;
const B: LayerKind = LayerKind::Bitmap;

fn calques(b: &Banc) -> &Vec<crate::document::Layer> {
    &b.app.doc.panels[0].layers
}

/// Les calques `garder` restent choisis avec le calque courant (0), les autres ne le sont plus.
fn choisir(b: &mut Banc, garder: &[usize]) {
    b.app.layer_sel = garder.iter().map(|&li| calques(b)[li].id).collect();
    assert_eq!(b.app.chosen_layers(), std::iter::once(0).chain(garder.iter().copied()).collect::<Vec<_>>());
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(1);
}

/// Enregistre le projet et le relit.
fn rouvrir(b: &Banc, nom: &str) -> crate::document::Document {
    let dir = std::env::temp_dir().join(format!("shot-selection-tout-2-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("P.pelure");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("ouverture").doc;
    std::fs::remove_dir_all(&dir).ok();
    relu
}

// ------------------------------------------------------------------------------------------------ 1. propriétés des calques

#[test]
fn the_eye_and_the_lock_of_a_chosen_layer_act_on_every_chosen_layer_with_the_inverse_of_the_clicked_one() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    let n0 = etapes(&b);
    // L'œil du calque 1 (choisi) : les trois se cachent, en une étape.
    b.app.toggle_layer_visible(1);
    assert!(calques(&b).iter().all(|l| !l.visible), "les trois calques choisis sont cachés");
    assert_eq!(etapes(&b), n0 + 1, "une étape");
    assert!(b.app.status.contains("3 calques cachés"), "{}", b.app.status);
    annuler(&mut b);
    assert!(calques(&b).iter().all(|l| l.visible), "une annulation les remontre tous");
    // Le cadenas : l'inverse de l'état du calque cliqué. Le calque 2 est déjà verrouillé : cliquer sur lui les déverrouille tous.
    b.app.doc.panels[0].layers[2].locked = true;
    b.app.doc.panels[0].layers[0].locked = true;
    b.app.toggle_layer_lock(2);
    assert!(calques(&b).iter().all(|l| !l.locked), "tous déverrouillés (l'inverse du calque 2)");
    b.app.toggle_layer_lock(1);
    assert!(calques(&b).iter().all(|l| l.locked), "tous verrouillés (l'inverse du calque 1)");
    assert!(b.app.status.contains("3 calques verrouillés"), "{}", b.app.status);
}

#[test]
fn a_click_on_a_layer_outside_the_choice_only_changes_that_layer() {
    let mut b = banc(&[V, V, V, V], Tool::Brush);
    choisir(&mut b, &[1]);
    b.app.toggle_layer_visible(3);
    assert_eq!(calques(&b).iter().map(|l| l.visible).collect::<Vec<_>>(), [true, true, true, false]);
    b.app.toggle_layer_lock(2);
    assert_eq!(calques(&b).iter().map(|l| l.locked).collect::<Vec<_>>(), [false, false, true, false]);
    b.app.basculer_calques(3, "Pochoir", |l| l.stencil, |l, v| l.stencil = v, false);
    assert_eq!(calques(&b).iter().map(|l| l.stencil).collect::<Vec<_>>(), [false, false, false, true]);
    b.app.regler_mode_calques(2, BlendMode::Multiply);
    assert_eq!(calques(&b).iter().filter(|l| l.blend == BlendMode::Multiply).count(), 1);
    // Un seul calque, le courant : comme avant.
    b.app.run_action("calque.choisir_aucun");
    b.app.toggle_layer_visible(0);
    assert_eq!(calques(&b).iter().map(|l| l.visible).collect::<Vec<_>>(), [false, true, true, false]);
    b.app.run_action("calque.pelure");
    assert_eq!(calques(&b).iter().filter(|l| l.onion).count(), 1);
    assert!(b.app.status.contains("Pelure d'oignon du calque"), "{}", b.app.status);
}

#[test]
fn blend_mode_onion_and_stencil_go_to_every_chosen_layer_and_are_saved() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    let n0 = etapes(&b);
    b.app.regler_mode_calques(1, BlendMode::Multiply);
    assert!(calques(&b).iter().all(|l| l.blend == BlendMode::Multiply), "le mode va aux trois");
    assert_eq!(etapes(&b), n0 + 1);
    annuler(&mut b);
    assert!(calques(&b).iter().all(|l| l.blend != BlendMode::Multiply), "une annulation les remet tous");
    b.app.regler_mode_calques(2, BlendMode::Screen);
    // La pelure (raccourci, calque courant choisi) et le pochoir et son sens (clic sur la ligne d'un calque choisi).
    b.app.doc.panels[0].layers[0].onion = false;
    b.app.run_action("calque.pelure");
    assert!(calques(&b).iter().all(|l| l.onion), "pelure : les trois");
    b.app.basculer_calques(2, "Pochoir", |l| l.stencil, |l, v| l.stencil = v, false);
    b.app.basculer_calques(1, "Pochoir négatif", |l| l.stencil_negative, |l, v| l.stencil_negative = v, false);
    assert!(calques(&b).iter().all(|l| l.stencil && l.stencil_negative), "pochoir et pochoir négatif : les trois");
    assert!(b.app.status.contains("sur 3 calques"), "{}", b.app.status);
    let relu = rouvrir(&b, "proprietes");
    assert!(relu.panels[0].layers.iter().all(|l| l.blend == BlendMode::Screen && l.onion && l.stencil && l.stencil_negative), "le fichier les a");
}

// ------------------------------------------------------------------------------------------------ 2. effets de trait, de matière, flou

fn effets(b: &Banc, kind: EffectKind) -> Vec<bool> {
    calques(b).iter().map(|l| l.fx.iter().any(|e| e.kind == kind && e.enabled)).collect()
}

#[test]
fn trait_and_matter_effects_and_the_blur_go_on_every_chosen_layer_that_takes_them() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    let n0 = etapes(&b);
    // Le tremblement du trait ne va que sur un calque vectoriel : le bitmap est laissé de côté, et la barre d'état le dit.
    b.app.run_action("fx.tremblement");
    assert_eq!(effets(&b, EffectKind::Boil), [true, false, true]);
    assert_eq!(etapes(&b), n0 + 1, "une étape");
    assert!(b.app.status.contains("sur 2 calques") && b.app.status.contains("Calque 1"), "{}", b.app.status);
    // Décocher : sur les deux.
    b.app.run_action("fx.tremblement");
    assert_eq!(effets(&b, EffectKind::Boil), [false, false, false]);
    // Le flou sur le calque : sur les trois.
    b.app.run_action("calque.flou");
    assert_eq!(effets(&b, EffectKind::GaussianBlur), [true, true, true]);
    annuler(&mut b);
    assert_eq!(effets(&b, EffectKind::GaussianBlur), [false, false, false], "une annulation l'ôte des trois");
    // Un préréglage de matière : le contour blanc va partout, l'arc-en-ciel (vectoriel) pas sur le bitmap.
    b.app.apply_matter_preset(crate::fxmatiere::Preset::WhiteOutline);
    assert_eq!(effets(&b, EffectKind::Outline), [true, true, true]);
    b.app.apply_matter_preset(crate::fxmatiere::Preset::Rainbow);
    assert_eq!(effets(&b, EffectKind::StrokeGradient), [true, false, true]);
    assert!(b.app.status.contains("Laissé de côté"), "{}", b.app.status);
    // Un seul calque : comme avant.
    b.app.run_action("calque.choisir_aucun");
    b.app.run_action("calque.flou");
    assert_eq!(effets(&b, EffectKind::GaussianBlur), [true, false, false]);
}

#[test]
fn a_setting_of_an_effect_goes_to_the_same_effect_of_every_chosen_layer_in_one_step() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    b.app.run_action("calque.flou");
    b.app.doc.panels[0].layers[2].fx.insert(0, crate::fx::Effect::new(EffectKind::Outline));
    let rang = |b: &Banc, li: usize| calques(b)[li].fx.iter().position(|e| e.kind == EffectKind::GaussianBlur).unwrap();
    let v0 = calques(&b)[1].fx[rang(&b, 1)].values_at(0)[0];
    let n0 = etapes(&b);
    // (Le geste du panneau : son étape, puis le réglage.)
    b.app.record_step();
    let i = rang(&b, 0);
    assert_eq!(b.app.fx_regler(0, i, 0, &[v0 + 7.0], false), 3, "les trois flous");
    for li in 0..3 {
        assert_eq!(calques(&b)[li].fx[rang(&b, li)].values_at(0)[0], v0 + 7.0, "calque {li}");
    }
    assert_eq!(etapes(&b), n0 + 1);
    annuler(&mut b);
    for li in 0..3 {
        assert_eq!(calques(&b)[li].fx[rang(&b, li)].values_at(0)[0], v0, "calque {li} : une annulation");
    }
}

// ------------------------------------------------------------------------------------------------ 3. clés d'effets

#[test]
fn creating_or_deleting_an_effect_key_acts_on_the_same_effect_of_every_chosen_layer_and_is_saved() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    b.app.run_action("calque.flou");
    b.app.cur_frame = 2;
    b.chauffer(1);
    let i = calques(&b)[0].fx.iter().position(|e| e.kind == EffectKind::GaussianBlur).unwrap();
    b.app.select_fx(i);
    let n0 = etapes(&b);
    b.app.run_action("fx.cle_creer");
    let cles = |b: &Banc| calques(b).iter().map(|l| l.fx.iter().find(|e| e.kind == EffectKind::GaussianBlur).is_some_and(|e| e.key_at(2).is_some())).collect::<Vec<_>>();
    assert_eq!(cles(&b), [true, true, true], "{}", b.app.status);
    assert_eq!(etapes(&b), n0 + 1);
    assert!(b.app.status.contains("sur 3 calques"), "{}", b.app.status);
    let relu = rouvrir(&b, "cles");
    assert!(relu.panels[0].layers.iter().all(|l| l.fx.iter().any(|e| e.kind == EffectKind::GaussianBlur && e.key_at(2).is_some())), "le fichier a les trois clés");
    b.app.select_fx(i);
    b.app.run_action("fx.cle_supprimer");
    assert_eq!(cles(&b), [false, false, false]);
    assert!(b.app.status.contains("sur 3 calques"), "{}", b.app.status);
    annuler(&mut b);
    assert_eq!(cles(&b), [true, true, true], "une annulation les remet toutes");
}

// ------------------------------------------------------------------------------------------------ 6. retourner un calque à trajectoire

#[test]
fn flipping_a_trajectory_layer_with_others_flips_its_path_around_the_common_middle() {
    let mut b = banc(&[V, V], Tool::Brush);
    let y1 = y_de(1);
    let mut t = crate::trajectoire::Trajectoire::nouvelle(&[[150.0, y1], [220.0, y1 + 20.0], [280.0, y1 + 40.0]]).expect("un chemin");
    t.cles = crate::trajectoire::Cles::Interpolation;
    b.app.doc.panels[0].layers[1].trajectoire = Some(t);
    b.app.trajectoire_recalculer_dans(0, 1);
    assert!(!calques(&b)[1].pos_keys.is_empty(), "des clés de position");
    let pivot = b.app.doc.stage_center();
    let sur_la_feuille = |b: &Banc, li: usize, f: usize, p: eframe::egui::Pos2| {
        let l = &calques(b)[li];
        crate::poskeys::pose_at(&l.pos_keys, f, l.transform).affine(pivot).apply(p)
    };
    let f = b.app.cur_frame;
    let avant0 = sur_la_feuille(&b, 0, f, pos2(120.0, y_de(0)));
    let avant1: Vec<_> = (0..4).map(|f| sur_la_feuille(&b, 1, f, pos2(200.0, y1))).collect();
    let chemin0 = calques(&b)[1].trajectoire.as_ref().unwrap().points.clone();
    b.app.run_action("calque.miroir_h");
    assert!(!b.app.status.contains("trajectoire"), "le calque à trajectoire n'est plus laissé de côté : {}", b.app.status);
    // Le milieu commun : celui où le calque 0 a été retourné.
    let apres0 = sur_la_feuille(&b, 0, f, pos2(120.0, y_de(0)));
    let axe = (avant0.x + apres0.x) * 0.5;
    let verifier = |b: &Banc, quoi: &str| {
        for (f, a) in avant1.iter().enumerate() {
            let p = sur_la_feuille(b, 1, f, pos2(200.0, y1));
            assert!((p.x - (2.0 * axe - a.x)).abs() < 1.0 && (p.y - a.y).abs() < 1.0, "{quoi}, image {f} : le mouvement est retourné autour de x = {axe} ({a:?} → {p:?})");
        }
    };
    verifier(&b, "retourné");
    let chemin = &calques(&b)[1].trajectoire.as_ref().unwrap().points;
    for (p, q) in chemin0.iter().zip(chemin) {
        assert!((q[0] - (2.0 * axe - p[0])).abs() < 0.5 && (q[1] - p[1]).abs() < 1e-3, "le chemin aussi : {p:?} → {q:?}");
    }
    // Les clés refaites d'après le chemin retourné retrouvent la même pose.
    b.app.trajectoire_recalculer_dans(0, 1);
    verifier(&b, "après un nouveau calcul des clés");
    let relu = rouvrir(&b, "trajectoire");
    assert_eq!(relu.panels[0].layers[1].trajectoire.as_ref().map(|t| t.points.clone()), calques(&b)[1].trajectoire.as_ref().map(|t| t.points.clone()), "le fichier a le chemin retourné");
}
