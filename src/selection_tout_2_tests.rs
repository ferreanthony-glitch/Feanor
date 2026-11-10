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

// ------------------------------------------------------------------------------------------------ relecture de 7725741

/// Relecture, 1 : décocher un effet n'en ajoute jamais aux calques choisis qui ne l'avaient pas.
#[test]
fn unchecking_an_effect_never_adds_it_to_the_chosen_layers_that_did_not_have_it() {
    let mut b = banc(&[V, V, V], Tool::Brush);
    b.app.doc.panels[0].layers[0].fx.push(crate::fx::Effect::new(EffectKind::Boil));
    b.app.run_action("fx.tremblement");
    assert_eq!(effets(&b, EffectKind::Boil), [false, false, false]);
    assert!(calques(&b)[1].fx.is_empty() && calques(&b)[2].fx.is_empty(), "rien n'est ajouté aux autres");
    // Le flou : même règle.
    b.app.doc.panels[0].layers[0].fx.push(crate::fx::Effect::new(EffectKind::GaussianBlur));
    b.app.run_action("calque.flou");
    assert!(calques(&b)[1].fx.is_empty() && calques(&b)[2].fx.is_empty());
}

/// Relecture, 2 : une trajectoire en clés d'images retournée avec un autre calque : après un nouveau calcul, chaque dessin de jalon
/// est le miroir de ce qu'il était.
#[test]
fn flipping_a_trajectory_in_drawing_keys_keeps_each_milestone_drawing_mirrored_after_a_new_computation() {
    let mut b = banc(&[V, V], Tool::Brush);
    let y1 = y_de(1);
    let mut t = crate::trajectoire::Trajectoire::nouvelle(&[[150.0, y1], [220.0, y1 + 20.0], [280.0, y1 + 40.0]]).expect("un chemin");
    t.cles = crate::trajectoire::Cles::Images;
    t.marques = vec![0.5];
    b.app.doc.panels[0].layers[1].trajectoire = Some(t);
    b.app.trajectoire_recalculer_dans(0, 1);
    let jalons = calques(&b)[1].trajectoire.as_ref().unwrap().images_des_jalons(4);
    assert!(jalons.len() >= 3, "{jalons:?}");
    let centre = |b: &Banc, li: usize, f: usize| {
        let bx = b.app.layer_content_box(li, f).expect("un dessin");
        crate::multisel::layer_to_stage(&b.app.doc, 0, f, li).expect("sur la feuille").apply_rect(bx).center()
    };
    let f = b.app.cur_frame;
    let avant0 = centre(&b, 0, f);
    let avant: Vec<_> = jalons.iter().map(|&j| centre(&b, 1, j)).collect();
    b.app.run_action("calque.miroir_h");
    let axe = (avant0.x + centre(&b, 0, f).x) * 0.5;
    b.app.trajectoire_recalculer_dans(0, 1);
    for (k, &j) in jalons.iter().enumerate() {
        let p = centre(&b, 1, j);
        assert!((p.x - (2.0 * axe - avant[k].x)).abs() < 1.0 && (p.y - avant[k].y).abs() < 1.0, "jalon {k} (image {j}) : le miroir de {:?} autour de x = {axe}, pas {p:?}", avant[k]);
    }
}

/// Relecture, 3 : le cadenas posé lâche la sélection sur plusieurs calques : un glissé ensuite ne bouge rien.
#[test]
fn locking_a_layer_drops_the_multi_layer_selection_and_a_drag_moves_nothing() {
    use crate::multicalques_tests::{banc_deformer, cel_de, empreinte, glisser, ligne, repos};
    let mut b = banc_deformer(crate::envelopetools::DeformMode::Perspective, 0);
    b.app.set_tool(Tool::Select);
    b.chauffer(2);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.multi.sel.is_some(), "une sélection sur les deux calques : {}", b.app.status);
    let (a, z) = (cel_de(&b, 0, 0), cel_de(&b, 1, 0));
    let (ea, ez) = (empreinte(&b, a), empreinte(&b, z));
    b.app.run_action("calque.verrouiller");
    assert!(calques(&b).iter().all(|l| l.locked));
    assert!(b.app.multi.sel.is_none(), "la sélection est lâchée");
    glisser(&mut b, &ligne(pos2(200.0, 150.0), pos2(240.0, 170.0), 8));
    repos(&mut b);
    assert!(empreinte(&b, a) == ea && empreinte(&b, z) == ez, "rien n'a bougé");
    let t = calques(&b).iter().map(|l| l.transform).collect::<Vec<_>>();
    assert!(t.iter().all(|t| t.x == 0.0 && t.y == 0.0), "aucun calque déplacé : {t:?}");
}

/// Relecture, 3 : une sélection prise, puis le calque verrouillé autrement (dossier, fichier) : elle ne vaut plus.
#[test]
fn a_multi_layer_selection_on_a_layer_locked_since_is_no_longer_valid() {
    use crate::multicalques_tests::banc_deformer;
    let mut b = banc_deformer(crate::envelopetools::DeformMode::Perspective, 0);
    b.app.set_tool(Tool::Select);
    b.chauffer(2);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.multi.sel.as_ref().expect("une sélection").still_valid(&b.app.doc, 0, 0));
    b.app.doc.panels[0].layers[1].locked = true;
    assert!(!b.app.multi.sel.as_ref().unwrap().still_valid(&b.app.doc, 0, 0), "un calque verrouillé : la sélection ne vaut plus");
}

/// Relecture, 3 : l'Enveloppe ouverte, puis Alt+L : la séance se ferme sans rien écrire, un geste ensuite ne déforme rien.
#[test]
fn locking_during_an_envelope_session_closes_it_and_a_gesture_deforms_nothing() {
    use crate::multicalques_tests::{banc_deformer, cel_de, clic, empreinte, glisser, ligne, repos};
    let mut b = banc_deformer(crate::envelopetools::DeformMode::Envelope, 0);
    let (a, z) = (cel_de(&b, 0, 0), cel_de(&b, 1, 0));
    let (ea, ez) = (empreinte(&b, a), empreinte(&b, z));
    clic(&mut b, pos2(200.0, 150.0));
    assert!(b.app.deform_multi.is_some(), "la séance est ouverte : {}", b.app.status);
    b.app.run_action("calque.verrouiller");
    assert!(b.app.deform_multi.is_none(), "la séance est fermée");
    glisser(&mut b, &ligne(pos2(100.0, 100.0), pos2(140.0, 130.0), 8));
    repos(&mut b);
    assert!(empreinte(&b, a) == ea && empreinte(&b, z) == ez, "rien n'est déformé");
}

/// Relecture, 4 : supprimer une clé d'effet part du calque courant ; sans clé ici, rien n'est supprimé ailleurs. Les calques sans
/// effet de même sorte sont nommés.
#[test]
fn deleting_an_effect_key_starts_from_the_current_layer_and_names_the_layers_left_aside() {
    let mut b = banc(&[V, V, V], Tool::Brush);
    b.app.run_action("calque.flou");
    // Le calque 2 perd son flou : il est laissé de côté, nommé.
    b.app.doc.panels[0].layers[2].fx.clear();
    let i = calques(&b)[0].fx.iter().position(|e| e.kind == EffectKind::GaussianBlur).unwrap();
    // Une clé à l'image 1 sur le calque 1 seulement : depuis le calque courant (0), sans clé ici, rien n'est supprimé.
    let v = calques(&b)[1].fx[0].values_at(1);
    b.app.doc.panels[0].layers[1].fx[0].set_key(1, v);
    b.app.cur_frame = 1;
    b.chauffer(1);
    b.app.select_fx(i);
    b.app.run_action("fx.cle_supprimer");
    assert!(calques(&b)[1].fx[0].key_at(1).is_some(), "la clé du calque 1 reste : {}", b.app.status);
    // Créer : sur 0 et 1, le calque 2 nommé.
    b.app.run_action("fx.cle_creer");
    assert!(b.app.status.contains("sur 2 calques") && b.app.status.contains("Calque 2"), "{}", b.app.status);
    b.app.select_fx(i);
    b.app.run_action("fx.cle_supprimer");
    assert!(calques(&b)[0].fx[i].key_at(1).is_none() && calques(&b)[1].fx[0].key_at(1).is_none());
    assert!(b.app.status.contains("sur 2 calques") && b.app.status.contains("Calque 2"), "{}", b.app.status);
    // Un réglage : le compte et le calque laissé de côté.
    b.app.record_step();
    b.app.fx_regler(0, i, 0, &[3.0], false);
    assert!(b.app.status.contains("sur 2 calques") && b.app.status.contains("Calque 2"), "{}", b.app.status);
}

/// Relecture, 4 : Suppr sur une clé d'effet choisie dans la timeline agit sur l'effet de même sorte de tous les calques choisis.
#[test]
fn delete_on_a_chosen_effect_key_in_the_timeline_acts_on_every_chosen_layer() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    b.app.run_action("calque.flou");
    b.app.cur_frame = 2;
    b.chauffer(1);
    let i = calques(&b)[0].fx.iter().position(|e| e.kind == EffectKind::GaussianBlur).unwrap();
    b.app.select_fx(i);
    b.app.run_action("fx.cle_creer");
    let cles = |b: &Banc| calques(b).iter().map(|l| l.fx.iter().find(|e| e.kind == EffectKind::GaussianBlur).is_some_and(|e| e.key_at(2).is_some())).collect::<Vec<_>>();
    assert_eq!(cles(&b), [true, true, true]);
    // La clé créée est la clé d'effet choisie (celle du calque courant) ; Suppr la supprime, et celle des autres calques choisis.
    let n0 = etapes(&b);
    b.app.delete_fx_keys();
    assert_eq!(cles(&b), [false, false, false], "{}", b.app.status);
    assert_eq!(etapes(&b), n0 + 1);
    assert!(b.app.status.contains("sur 3 calques"), "{}", b.app.status);
}

/// Relecture, 5 : l'effet répond par son rang parmi ceux de même sorte ; sans équivalent, le calque est laissé de côté.
#[test]
fn an_effect_answers_by_its_rank_among_effects_of_the_same_kind_or_the_layer_is_left_aside() {
    let mut b = banc(&[V, V], Tool::Brush);
    for li in 0..2 {
        b.app.doc.panels[0].layers[li].fx.push(crate::fx::Effect::new(EffectKind::GaussianBlur));
    }
    b.app.doc.panels[0].layers[0].fx.push(crate::fx::Effect::new(EffectKind::GaussianBlur));
    assert_eq!(b.app.effet_pareil(0, 0, 1), Some(0));
    assert_eq!(b.app.effet_pareil(0, 1, 1), None, "le 2e flou n'a pas d'équivalent : pas le 1er à sa place");
    let v0 = calques(&b)[1].fx[0].values_at(0)[0];
    b.app.record_step();
    b.app.fx_regler(0, 1, 0, &[v0 + 5.0], false);
    assert_eq!(calques(&b)[1].fx[0].values_at(0)[0], v0, "le flou du calque 1 n'est pas touché");
    assert!(b.app.status.contains("Calque 1"), "il est nommé : {}", b.app.status);
}

/// Relecture, 6 et 7 : un préréglage dit combien d'effets il remplace ; la raison d'un refus est la vraie.
#[test]
fn a_preset_on_several_layers_counts_the_replaced_effects_and_a_refusal_gives_the_real_reason() {
    let mut b = banc(&[V, B, V], Tool::Brush);
    b.app.apply_matter_preset(crate::fxmatiere::Preset::WhiteOutline);
    b.app.apply_matter_preset(crate::fxmatiere::Preset::WhiteOutline);
    assert!(b.app.status.contains("3 effets de même sorte remplacés"), "{}", b.app.status);
    b.app.run_action("fx.tremblement");
    assert!(b.app.status.contains("Calque 1 (calque bitmap"), "la vraie raison : {}", b.app.status);
}