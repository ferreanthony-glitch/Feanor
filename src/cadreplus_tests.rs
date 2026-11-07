//! Essais du « + » de la peau de Maj (partie 115) : un cadre de caméra de plus, sans clic droit, par de vrais gestes sur le banc.

use eframe::egui::{pos2, vec2, Event, Key, Modifiers, Pos2, Rect};

use crate::app::Tool;
use crate::boardui::Press;
use crate::cadreplus::RAYON_PRISE;
use crate::camera::CameraKey;
use crate::document::Document;
use crate::perf_tests::{se_reposer, Banc};
use crate::vignettes_camera_tests::{avec_b_a_droite, cadre_ecran, centre, clefs, ctrl_glisser, glisser_avec, largeur_cadre_ecran, pas, plan_court, K};
use crate::vignettes_tests::{appui, dernier_nom, ouvert, pas_d_annulation, quatre_plans, vignette};

const MAJ: Modifiers = Modifiers::SHIFT;

/// Maj tenue, le pointeur au centre de la case `k` : la place du « + » à l'écran (rien si l'on ne le voit pas).
fn survoler(b: &mut Banc, k: usize) -> Option<Pos2> {
    b.touches = MAJ;
    let c = vignette(b, k).center();
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.app.vignettes.plus.place.map(|p| p.centre)
}

fn plus(b: &Banc) -> Pos2 {
    b.app.vignettes.plus.place.expect("le + est là").centre
}

fn relacher_maj(b: &mut Banc) {
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un clic sur le « + » de la case `k` (Maj tenue).
fn cliquer_plus(b: &mut Banc, k: usize) {
    let p = survoler(b, k).expect("le + est visible");
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, MAJ)], 1.0 / 60.0);
    b.image(vec![appui(p, false, MAJ)], 1.0 / 60.0);
    relacher_maj(b);
}

fn sur_une_barre(b: &Banc, p: Pos2) -> bool {
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    matches!(crate::board::hit(&l, xf.to_board(p), 14.0 / xf.zoom), crate::board::Hit::Bar(_))
}

fn plans_voisins_epargnes(b: &Banc, p: Pos2, plan: u64) {
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    for c in l.cards.iter().filter(|c| c.plan != plan) {
        assert!(xf.rect(c.foot).distance_to_pos(p) >= 8.0, "le + mord sur la vignette d'un autre plan : {p:?} / {:?}", xf.rect(c.foot));
    }
}

// ------------------------------------------------------------------------------ il n'est là que sous Maj, T ou l'outil Caméra

#[test]
fn the_plus_is_only_there_while_shift_or_the_camera_is_held_and_never_on_plain_paper() {
    let mut b = ouvert(plan_court(), 6);
    let c = vignette(&b, K).center();
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.vignettes.plus.place.is_none(), "sans Maj ni caméra : du papier");
    let p = survoler(&mut b, K).expect("Maj tenue : le + est là");
    let case = vignette(&b, K);
    let d = case.distance_to_pos(p);
    assert!((5.0..22.0).contains(&d), "juste à côté de la case : {p:?} / {case:?} ({d})");
    assert!(!sur_une_barre(&b, p), "jamais sur une barre");
    assert!(b.app.doc.panels[1].camera.is_empty(), "il ne pose rien dans le projet");
    relacher_maj(&mut b);
    assert!(b.app.vignettes.plus.place.is_none(), "Maj relâchée : du papier");
    // L'outil Caméra en main : le + est là sans Maj.
    b.app.set_tool(Tool::Camera);
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.vignettes.plus.place.is_some(), "l'outil Caméra le montre");
    // Ctrl + Maj (le geste de caméra) ne le montre pas.
    b.app.set_tool(Tool::Brush);
    b.touches = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
    b.image(vec![Event::PointerMoved(c + vec2(2.0, 0.0))], 1.0 / 60.0);
    assert!(b.app.vignettes.plus.place.is_none());
    relacher_maj(&mut b);
}

#[test]
fn the_plus_stays_beside_the_last_frame_and_never_bites_another_plan() {
    let mut b = ouvert(plan_court(), 6);
    let plan = b.app.board_layout().cards[K].plan;
    let p = survoler(&mut b, K).unwrap();
    plans_voisins_epargnes(&b, p, plan);
    // Après un B posé à droite, le + est à droite de B.
    cliquer_plus(&mut b, K);
    let p = survoler(&mut b, K).unwrap();
    let qb = cadre_ecran(&b, 1);
    let boite_b = Rect::from_points(&qb);
    assert!(boite_b.distance_to_pos(p) < 22.0, "à côté de B : {p:?} {boite_b:?}");
    assert!(!vignette(&b, K).contains(p), "pas dans la case A : {p:?}");
    assert!(!sur_une_barre(&b, p));
    plans_voisins_epargnes(&b, p, plan);
}

#[test]
fn on_a_crowded_board_the_plus_stays_in_view_and_off_the_other_plans() {
    use crate::vignettes_camera_tests::K as _K;
    let mut b = ouvert(plan_court(), 12);
    b.app.board_zoom = 0.45;
    b.chauffer(4);
    let vue = b.app.vignettes.view;
    let n = b.app.board_layout().cards.len();
    let mut vus = 0;
    for k in 0..n {
        let c = vignette(&b, k);
        if !vue.contains_rect(c) {
            continue;
        }
        let plan = b.app.board_layout().cards[k].plan;
        if let Some(p) = survoler(&mut b, k) {
            vus += 1;
            assert!(vue.contains(p), "le + est dans la vue : {p:?} {vue:?}");
            assert!(!sur_une_barre(&b, p), "le + n'est pas sur une barre : case {k}");
            plans_voisins_epargnes(&b, p, plan);
        }
    }
    assert!(vus >= 4, "il se montre sur les cases visibles : {vus}");
    let _ = _K;
}

// ------------------------------------------------------------------------------ le clic

#[test]
fn a_click_on_the_plus_adds_b_then_c_then_d_one_undo_step_each() {
    let mut b = ouvert(plan_court(), 6);
    let nb = pas_d_annulation(&b);
    cliquer_plus(&mut b, K);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert!(clefs(&b)[1].1 > clefs(&b)[0].1, "B à droite de A");
    assert_eq!(pas_d_annulation(&b), nb + 1, "une seule étape");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    cliquer_plus(&mut b, K);
    assert_eq!(b.app.doc.panels[1].camera.len(), 3);
    assert_eq!(pas_d_annulation(&b), nb + 2);
    assert_eq!(dernier_nom(&b), "Cadre C");
    cliquer_plus(&mut b, K);
    assert_eq!(b.app.doc.panels[1].camera.len(), 4);
    assert_eq!(dernier_nom(&b), "Cadre D");
    // Ctrl+Z les rend un par un.
    for n in [3, 2, 0] {
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert_eq!(b.app.doc.panels[1].camera.len(), n);
    }
    assert_eq!(pas_d_annulation(&b), nb);
}

#[test]
fn the_click_gives_the_same_result_as_the_right_click_menu() {
    let mut a = ouvert(plan_court(), 6);
    let mut b = ouvert(plan_court(), 6);
    cliquer_plus(&mut a, K);
    b.app.cam_ajouter_cadre(1);
    assert_eq!(a.app.doc.panels[1].camera, b.app.doc.panels[1].camera);
}

#[test]
fn a_locked_storyboard_refuses_the_plus_and_says_so() {
    let mut b = ouvert(plan_court(), 6);
    b.app.doc.storyboard_locked = true;
    let nb = pas_d_annulation(&b);
    cliquer_plus(&mut b, K);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert_eq!(pas_d_annulation(&b), nb);
    assert!(b.app.status.to_lowercase().contains("verrouill"), "la ligne d'état le dit : {}", b.app.status);
}

#[test]
fn a_plan_too_short_for_a_frame_says_so() {
    let mut doc = quatre_plans();
    doc.set_duration(1, 1);
    let mut b = ouvert(doc, 6);
    cliquer_plus(&mut b, K);
    assert!(b.app.doc.panels[1].camera.len() < 2);
    assert!(b.app.status.contains("trop court"), "{}", b.app.status);
}

// ------------------------------------------------------------------------------ le glissé

#[test]
fn dragging_from_the_plus_makes_the_new_frame_follow_the_pointer_and_drop_where_it_is_released() {
    let mut b = ouvert(plan_court(), 6);
    let nb = pas_d_annulation(&b);
    let w = largeur_cadre_ecran(&b);
    let p = survoler(&mut b, K).unwrap();
    let vers = p + vec2(0.9 * w, 0.6 * w);
    glisser_avec(&mut b, MAJ, p, &pas(p, vers - p, 8), vers, false);
    // En plein geste : le cadre est là et suit le pointeur, l'étape n'est pas encore prise.
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert_eq!(pas_d_annulation(&b), nb);
    assert!((centre(cadre_ecran(&b, 1)) - vers).length() < 14.0, "le cadre est centré sur le pointeur : {:?} / {vers:?}", centre(cadre_ecran(&b, 1)));
    b.image(vec![appui(vers, false, MAJ)], 1.0 / 60.0);
    relacher_maj(&mut b);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert_eq!(pas_d_annulation(&b), nb + 1, "une seule étape");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    assert!((centre(cadre_ecran(&b, 1)) - vers).length() < 14.0, "posé où l'on a lâché");
    assert!(b.app.vignettes.cam.is_none());
    // Un second glissé : C, une étape, « Cadre C ».
    let p = survoler(&mut b, K).unwrap();
    let vers = p + vec2(-0.4 * w, 0.9 * w);
    glisser_avec(&mut b, MAJ, p, &pas(p, vers - p, 8), vers, true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 3);
    assert_eq!(pas_d_annulation(&b), nb + 2);
    assert_eq!(dernier_nom(&b), "Cadre C");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
}

#[test]
fn a_drag_that_comes_back_to_the_start_is_a_click() {
    let mut a = ouvert(plan_court(), 6);
    cliquer_plus(&mut a, K);
    let mut b = ouvert(plan_court(), 6);
    let nb = pas_d_annulation(&b);
    let w = largeur_cadre_ecran(&b);
    let p = survoler(&mut b, K).unwrap();
    let loin = p + vec2(0.8 * w, 0.5 * w);
    let mut chemin = pas(p, loin - p, 6);
    chemin.push(loin);
    chemin.extend(pas(loin, p - loin, 6));
    glisser_avec(&mut b, MAJ, p, &chemin, p + vec2(1.0, 0.0), true);
    assert_eq!(pas_d_annulation(&b), nb + 1, "une seule étape");
    assert_eq!(b.app.doc.panels[1].camera, a.app.doc.panels[1].camera, "le cadre est celui d'un clic");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    assert!(b.app.vignettes.cam.is_none());
}

#[test]
fn escape_during_a_drag_from_the_plus_adds_nothing() {
    let mut b = ouvert(plan_court(), 6);
    let nb = pas_d_annulation(&b);
    let w = largeur_cadre_ecran(&b);
    let p = survoler(&mut b, K).unwrap();
    let vers = p + vec2(0.9 * w, 0.4 * w);
    glisser_avec(&mut b, MAJ, p, &pas(p, vers - p, 6), vers, false);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: MAJ }], 1.0 / 60.0);
    b.image(vec![appui(vers, false, MAJ)], 1.0 / 60.0);
    relacher_maj(&mut b);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert_eq!(pas_d_annulation(&b), nb);
}

// ------------------------------------------------------------------------------ rien d'autre ne change

#[test]
fn dragging_inside_b_still_moves_b_and_shift_dragging_elsewhere_still_moves_the_plan_with_the_plus_showing() {
    let mut b = avec_b_a_droite();
    let w = largeur_cadre_ecran(&b);
    let avant = clefs(&b);
    survoler(&mut b, K).expect("le + se montre");
    relacher_maj(&mut b);
    // Glisser dans B (Ctrl + Maj) : B se déplace, il ne naît rien.
    let c = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, 0.1 * w), 6), c + vec2(0.0, 0.1 * w), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert_eq!(dernier_nom(&b), "Cadre B");
    assert!(clefs(&b)[1].2 > avant[1].2 + 5.0);
    // Maj + le bord de B : B se déplace.
    let qb = cadre_ecran(&b, 1);
    let bord = pos2((qb[0].x + qb[1].x) * 0.5, qb[0].y);
    let plus_ici = survoler(&mut b, K).unwrap();
    assert!((plus_ici - bord).length() > RAYON_PRISE + 2.0, "le + n'est pas sur le bord pris : {plus_ici:?} {bord:?}");
    glisser_avec(&mut b, MAJ, bord, &pas(bord, vec2(0.0, 0.1 * w), 5), bord + vec2(0.0, 0.1 * w), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "B déplacé, rien de plus");
    // Maj + glisser au milieu de la case A : le plan se déplace.
    let a = cadre_ecran(&b, 0);
    let milieu = pos2(a[0].x + (a[2].x - a[0].x) * 0.3, centre(a).y);
    let plus_ici = survoler(&mut b, K).unwrap();
    assert!((plus_ici - milieu).length() > RAYON_PRISE + 2.0);
    b.image(vec![Event::PointerMoved(milieu)], 1.0 / 60.0);
    b.image(vec![appui(milieu, true, MAJ)], 1.0 / 60.0);
    assert_eq!(b.app.vignettes.press, Press::Move);
    assert!(b.app.vignettes.moving.is_some() && b.app.vignettes.cam.is_none());
    b.image(vec![Event::PointerMoved(milieu + vec2(30.0, 30.0))], 1.0 / 60.0);
    assert!(b.app.vignettes.plus.place.is_none(), "pas de + pendant un geste");
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: MAJ }], 1.0 / 60.0);
    b.image(vec![appui(milieu, false, MAJ)], 1.0 / 60.0);
    relacher_maj(&mut b);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
}

#[test]
fn a_plain_press_beside_the_plus_behaves_as_before_and_the_plus_never_shows_during_a_stroke() {
    let mut b = ouvert(plan_court(), 6);
    b.app.set_tool(Tool::Brush);
    let nb = pas_d_annulation(&b);
    let case = vignette(&b, K);
    // Un trait (sans Maj) dans la case, Maj pressée en plein trait : ni + ni cadre.
    let a = case.center() + vec2(-20.0, 0.0);
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![appui(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for j in 1..6 {
        b.image(vec![Event::PointerMoved(a + vec2(j as f32 * 6.0, j as f32 * 2.0))], 1.0 / 60.0);
    }
    b.touches = MAJ;
    b.image(vec![Event::PointerMoved(a + vec2(40.0, 14.0))], 1.0 / 60.0);
    assert!(b.app.vignettes.plus.place.is_none(), "pas de + pendant un trait");
    b.image(vec![appui(a + vec2(40.0, 14.0), false, MAJ)], 1.0 / 60.0);
    relacher_maj(&mut b);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert!(pas_d_annulation(&b) > nb, "le trait est une étape");
}

#[test]
fn without_shift_a_press_where_the_plus_would_be_does_nothing_new() {
    let mut b = ouvert(plan_court(), 6);
    let p = survoler(&mut b, K).unwrap();
    relacher_maj(&mut b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    assert!(b.app.vignettes.plus.place.is_none() && b.app.vignettes.plus.geste.is_none());
    b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    assert!(b.app.vignettes.plus.geste.is_none(), "l'appui n'a pas pris le +");
    b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    // (Un appui simple dessine, comme avant : aucun cadre ne naît.)
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert_ne!(dernier_nom(&b), "Mouvement de caméra");
}

#[test]
fn the_camera_tool_shows_the_plus_and_a_click_on_it_adds_a_frame() {
    let mut b = ouvert(plan_court(), 6);
    b.app.set_tool(Tool::Camera);
    let c = vignette(&b, K).center();
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let p = plus(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
}

// ------------------------------------------------------------------------------ plan à plusieurs cases

#[test]
fn a_plan_with_several_cases_gets_its_plus_from_any_case() {
    // Le plan 0 a trois cases (deux marques d'image) et ne bouge pas : le + est à côté de la case survolée, et crée B.
    let mut b = ouvert(quatre_plans(), 6);
    let plan0 = b.app.board_layout().cards[0].plan;
    assert!(b.app.board_layout().cards[..3].iter().all(|c| c.plan == plan0));
    let p = survoler(&mut b, 2).expect("le + est là sur la troisième case");
    let case2 = vignette(&b, 2);
    assert!((p - case2.center()).length() < case2.width(), "à côté de la case survolée : {p:?} / {case2:?}");
    assert_eq!(b.app.vignettes.plus.place.unwrap().ancre, 2);
    cliquer_plus(&mut b, 2);
    assert_eq!(b.app.doc.panels[0].camera.len(), 2, "le plan a son mouvement");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
}

#[test]
fn a_moving_plan_keeps_its_plus_beside_its_last_frame_whatever_case_is_hovered() {
    let mut doc = quatre_plans();
    let n = doc.panels[0].frame_count();
    let a = doc.panel_camera(0, 0);
    let b_cam = a.shifted(60.0, 0.0);
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(n - 1, b_cam)];
    let mut b = ouvert(doc, 6);
    let l = b.app.board_layout();
    assert!(l.cards[0].mouvement && l.cards[1].plan == l.cards[0].plan && l.cards[1].frame > 0);
    // Survol de la case 0 puis d'une autre case du plan : le même + (à côté du dernier cadre de la case 0).
    let sur_0 = survoler(&mut b, 0).expect("sur la première case");
    let place = b.app.vignettes.plus.place.unwrap();
    assert_eq!(place.ancre, 0);
    let (cadres, _) = b.app.cadres_sur_la_planche(&b.app.board_layout(), 0);
    let dernier = Rect::from_points(&cadres.last().unwrap().map(|p| b.app.vignettes.xf.to_screen(p)));
    assert!(dernier.expand(40.0).contains(sur_0), "à côté du dernier cadre : {sur_0:?} {dernier:?}");
    let sur_1 = survoler(&mut b, 1);
    if let Some(p) = sur_1 {
        assert_eq!(b.app.vignettes.plus.place.unwrap().ancre, 0, "les cadres vivent sur la première case");
        assert!(p.distance(sur_0) < 1.0 || p.distance(dernier.center()) < dernier.width() + 60.0, "{p:?}");
    }
    // Un clic dessus : C.
    cliquer_plus(&mut b, 1);
    assert_eq!(b.app.doc.panels[0].camera.len(), 3);
    assert_eq!(dernier_nom(&b), "Cadre C");
    let _: Document = quatre_plans();
}

// ------------------------------------------------------------------------------ le repos

#[test]
fn the_vignettes_rest_after_a_plus_click_and_while_shift_is_held() {
    let mut b = ouvert(plan_court(), 6);
    cliquer_plus(&mut b, K);
    assert!(se_reposer(&mut b, 3000) < 3000, "FEANOR se repose après un clic sur le +");
    survoler(&mut b, K).unwrap();
    relacher_maj(&mut b);
    assert!(se_reposer(&mut b, 3000) < 3000);
}

// ------------------------------------------------------------------------------ à regarder

/// Le « + » à l'écran, en PNG, pour juger sa lisibilité (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --locked --target-dir target\msvc cadre_plus_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn cadre_plus_captures() {
    use crate::workspace::Espace;
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("CAPTURE {nom}");
    };
    let mut b = crate::perf_tests::banc(quatre_plans(), true);
    b.app.layout.board_textes = crate::boardsens::Textes::aucun();
    b.app.set_espace(Espace::Vignettes);
    b.app.board_zoom = 0.0;
    b.app.vignettes.ouverture_a_faire = true;
    b.chauffer(16);
    b.attendre_vignettes();
    // Les cases de la planche, un plan dessiné pour qu'on voie quelque chose.
    let c = vignette(&b, 3).center();
    // 1. Sans Maj : du papier.
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.chauffer(2);
    sauver(&b, "cadre-plus-01-sans-maj.png");
    // 2. Maj tenue sur une case : la peau et le +.
    b.touches = MAJ;
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.chauffer(2);
    sauver(&b, "cadre-plus-02-maj-tenue.png");
    // 3. Le pointeur sur le + : il se remplit.
    let p = plus(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(2);
    sauver(&b, "cadre-plus-03-plus-sous-le-pointeur.png");
    // 4. Après un clic : B posé, le + est à côté de B.
    b.image(vec![appui(p, true, MAJ)], 1.0 / 60.0);
    b.image(vec![appui(p, false, MAJ)], 1.0 / 60.0);
    b.chauffer(3);
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.chauffer(3);
    b.attendre_vignettes();
    sauver(&b, "cadre-plus-04-apres-un-clic.png");
    // 5. Un glissé en cours : le nouveau cadre suit le pointeur.
    let p = plus(&b);
    let vers = p + vec2(110.0, 90.0);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, MAJ)], 1.0 / 60.0);
    for q in pas(p, vers - p, 8) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(vers)], 1.0 / 60.0);
    b.chauffer(2);
    sauver(&b, "cadre-plus-05-glisse-en-cours.png");
    b.image(vec![appui(vers, false, MAJ)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    // 6. Planche serrée, Maj tenue sur une case du bord.
    b.app.board_zoom = 0.4;
    b.touches = MAJ;
    b.chauffer(3);
    let c = vignette(&b, 1).center();
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.chauffer(2);
    sauver(&b, "cadre-plus-06-planche-serree.png");
}
