//! Essais de la partie 142 : un mouvement de caméra de plus sur une case de continuité (B vers C, puis C vers D), par de vrais gestes sur
//! le banc. Avant, un plan ne portait qu'un mouvement, sur sa première case, et une case de continuité refusait la caméra.

use eframe::egui::{vec2, Event, Modifiers, Pos2};

use crate::camera::{Camera, CameraKey};
use crate::perf_tests::Banc;
use crate::vignettecam::{self, groupes, lettre_du_cadre, recolle, Groupe};
use crate::vignettes_camera_tests::{ctrl_glisser, largeur_cadre_ecran, pas, plan_court, K};
use crate::vignettes_tests::{appui, dernier_nom, ouvert, pas_d_annulation, vignette};

const MAJ: Modifiers = Modifiers::SHIFT;

fn cam(x: f32) -> Camera {
    Camera { x, ..Camera::centered(400, 300) }
}

fn cle(image: usize, x: f32) -> CameraKey {
    CameraKey::new(image, cam(x))
}

// ------------------------------------------------------------------------------ la règle, sans écran

#[test]
fn les_mouvements_se_lisent_dans_les_cles_case_par_case() {
    let cases = [0, 12, 24, 36];
    // Pas de clé, ou un seul mouvement : comme avant, tout est à la première case.
    assert_eq!(groupes(&[], &cases), vec![Groupe { debut: 0, rangs: 0..0, numero: 0 }]);
    let ab = [cle(0, 100.0), cle(11, 200.0)];
    assert_eq!(groupes(&ab, &cases), vec![Groupe { debut: 0, rangs: 0..2, numero: 0 }]);
    // Une case de continuité qui tient B à sa première image, puis va à C : elle porte son mouvement.
    let abc = [cle(0, 100.0), cle(11, 200.0), cle(24, 200.0), cle(35, 300.0)];
    let g = groupes(&abc, &cases);
    assert_eq!(g, vec![Groupe { debut: 0, rangs: 0..2, numero: 0 }, Groupe { debut: 24, rangs: 2..4, numero: 1 }]);
    assert_eq!((0..4).map(|r| lettre_du_cadre(&g, r)).collect::<String>(), "ABBC", "la case de continuité repart de la lettre qu'elle tient");
    assert_eq!(g[1].lettre0(), 1);
    // Puis une autre : C vers D.
    let abcd = [cle(0, 100.0), cle(11, 200.0), cle(24, 200.0), cle(35, 300.0), cle(36, 300.0), cle(47, 400.0)];
    let g = groupes(&abcd, &cases);
    assert_eq!(g.len(), 3);
    assert_eq!((g[2].debut, g[2].rangs.clone()), (36, 4..6));
    assert_eq!((0..6).map(|r| lettre_du_cadre(&g, r)).collect::<String>(), "ABBCCD");
    // Des clés posées à la main au début des cases, sans tenue : un seul mouvement, à la première case, comme avant.
    assert_eq!(groupes(&[cle(0, 100.0), cle(12, 200.0), cle(24, 300.0)], &cases).len(), 1);
    // Une tenue au début d'une case, sans rien après : pas de mouvement à elle.
    assert_eq!(groupes(&[cle(0, 100.0), cle(11, 200.0), cle(24, 200.0)], &cases).len(), 1);
    // Une tenue ailleurs qu'au début d'une case : pas de mouvement à elle non plus.
    assert_eq!(groupes(&[cle(0, 100.0), cle(11, 200.0), cle(20, 200.0), cle(30, 300.0)], &cases).len(), 1);
    // Deux tenues de suite : c'est la seconde qui commence le mouvement.
    let g = groupes(&[cle(0, 100.0), cle(11, 200.0), cle(12, 200.0), cle(24, 200.0), cle(35, 300.0)], &cases);
    assert_eq!(g.iter().map(|g| g.rangs.clone()).collect::<Vec<_>>(), vec![0..3, 3..5]);
    // Sa règle du 11 novembre au soir : un mouvement vit sur la case où il commence, même si ce n'est pas la première du plan. Un plan
    // dont le seul mouvement est sur sa deuxième case (A à sa première image, B à sa dernière) :
    let seconde = [cle(12, 100.0), cle(23, 200.0)];
    assert_eq!(groupes(&seconde, &cases), vec![Groupe { debut: 12, rangs: 0..2, numero: 0 }]);
    assert_eq!((0..2).map(|r| lettre_du_cadre(&groupes(&seconde, &cases), r)).collect::<String>(), "AB");
    // Une caméra fixe posée au début du plan, puis un mouvement sur la troisième case : elle part du cadre que tient le plan (A).
    let g = groupes(&[cle(0, 100.0), cle(24, 100.0), cle(35, 300.0)], &cases);
    assert_eq!(g, vec![Groupe { debut: 0, rangs: 0..1, numero: 0 }, Groupe { debut: 24, rangs: 1..3, numero: 1 }]);
    assert_eq!((0..3).map(|r| lettre_du_cadre(&g, r)).collect::<String>(), "AAB");
    // Des clés posées à la main au milieu d'une case : le mouvement vit sur la case où il commence.
    assert_eq!(groupes(&[cle(15, 100.0), cle(30, 200.0)], &cases)[0].debut, 12);
}

#[test]
fn recoller_un_mouvement_garde_la_continuite_du_suivant() {
    let orig = [cle(0, 100.0), cle(11, 200.0), cle(24, 200.0), cle(35, 300.0)];
    // B déplacé sur la première case : la case de continuité part du nouveau B.
    let apres = recolle(&orig, &(0..2), 0, &[cle(0, 100.0), cle(11, 250.0)], cam(200.0));
    assert_eq!(apres.iter().map(|k| (k.frame, k.cam.x)).collect::<Vec<_>>(), vec![(0, 100.0), (11, 250.0), (24, 250.0), (35, 300.0)]);
    // Un mouvement de plus sur une case à l'arrêt, comptée depuis son début (24) : rien d'autre ne bouge.
    let ab = [cle(0, 100.0), cle(11, 200.0)];
    let apres = recolle(&ab, &(2..2), 24, &[cle(0, 200.0), cle(11, 300.0)], cam(200.0));
    assert_eq!(apres.iter().map(|k| (k.frame, k.cam.x)).collect::<Vec<_>>(), vec![(0, 100.0), (11, 200.0), (24, 200.0), (35, 300.0)]);
    // Un mouvement suivant qui ne partait pas de ce cadre (un saut posé à la main) n'est pas touché.
    let saut = [cle(0, 100.0), cle(11, 200.0), cle(24, 500.0), cle(35, 300.0)];
    let apres = recolle(&saut, &(0..2), 0, &[cle(0, 100.0), cle(11, 250.0)], cam(200.0));
    assert_eq!(apres[2].cam.x, 500.0);
}

// ------------------------------------------------------------------------------ à l'écran

/// Le plan 2 : un mouvement A vers B sur sa première case (12 images), puis deux cases de continuité de 12 images qui montrent B.
fn ab_puis_deux_cases() -> Banc {
    let mut doc = plan_court();
    doc.set_duration(1, 12);
    let mut b = ouvert(doc, 6);
    let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "{}", b.app.status);
    b.app.doc.set_duration(1, 36);
    b.app.doc.toggle_mark(1, 12);
    b.app.doc.toggle_mark(1, 24);
    b.chauffer(3);
    let l = b.app.board_layout();
    assert_eq!((l.cards[K].frame, l.cards[K + 1].frame, l.cards[K + 2].frame), (0, 12, 24));
    assert_eq!(l.cards[K + 2].plan, l.cards[K].plan);
    b
}

fn cles(b: &Banc, plan: usize) -> Vec<(usize, f32, f32)> {
    b.app.doc.panels[plan].camera.iter().map(|k| (k.frame, k.cam.x, k.cam.y)).collect()
}

/// Un geste de caméra (Ctrl + Maj + glisser) parti du centre de la case `k`, de `d` (écran).
fn geste_camera(b: &mut Banc, k: usize, d: eframe::egui::Vec2) {
    let c = vignette(b, k).center();
    ctrl_glisser(b, c, &pas(c, d, 6), c + d, true);
    b.chauffer(2);
}

#[test]
fn une_case_de_continuite_prend_un_mouvement_de_plus_qui_part_du_cadre_qu_elle_montre() {
    let mut b = ab_puis_deux_cases();
    let w = largeur_cadre_ecran(&b);
    let avant = cles(&b, 1);
    let etapes = pas_d_annulation(&b);
    let (bx, by) = (avant[1].1, avant[1].2);
    // Avant : la troisième case montre B, à l'arrêt ; elle ne porte pas de mouvement.
    assert!(!b.app.board_layout().cards[K + 2].mouvement);
    geste_camera(&mut b, K + 2, vec2(0.5 * w, 0.3 * w));
    let k = cles(&b, 1);
    assert_eq!(k.len(), 4, "une tenue de B au début de la case, puis C à sa fin : {k:?} ({})", b.app.status);
    assert_eq!(&k[..2], &avant[..], "le mouvement A vers B n'a pas bougé");
    assert_eq!(k[2], (24, bx, by), "la case part de B, à sa première image");
    assert_eq!(k[3].0, 35, "C à la dernière image de la case");
    assert!(k[3].1 > bx + 1.0 && k[3].2 > by + 1.0, "C est en bas à droite de B : {k:?}");
    assert!(b.app.status.contains("Cadre C posé") && b.app.status.contains("de B à C"), "{}", b.app.status);
    assert_eq!(pas_d_annulation(&b), etapes + 1, "une seule étape d'annulation");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    // La planche : la troisième case porte son empreinte (deux cadres, le premier est la case), la première garde la sienne, la deuxième
    // reste une case ordinaire.
    let l = b.app.board_layout();
    assert!(l.cards[K].mouvement && !l.cards[K + 1].mouvement && l.cards[K + 2].mouvement);
    let m = l.move_of(K + 2).expect("l'empreinte de la troisième case");
    assert_eq!(m.cadres.len(), 2);
    assert_eq!((m.cadres[0].rang, m.cadres[1].rang), (2, 3), "les cadres sont les clés 2 et 3 du plan");
    assert!(m.cadres[0].rect.min.distance(l.cards[K + 2].thumb.min) < 0.5 && m.cadres[0].rect.max.distance(l.cards[K + 2].thumb.max) < 0.5, "le premier cadre est la case");
    assert_eq!(l.move_of(K).expect("celle de la première").cadres.len(), 2);
    let g = vignettecam::groupes_du_plan(&b.app.doc, 1);
    assert_eq!((2..4).map(|r| lettre_du_cadre(&g, r)).collect::<String>(), "BC");
    // La caméra : à l'arrêt sur B pendant la deuxième case, puis de B à C pendant la troisième.
    for f in [12, 18, 23, 24] {
        let c = b.app.doc.panel_camera(1, f);
        assert_eq!((c.x, c.y), (bx, by), "image {f} : la caméra tient B");
    }
    let milieu = b.app.doc.panel_camera(1, 30);
    assert!(milieu.x > bx && milieu.x < k[3].1, "image 30 : en route vers C");
    assert_eq!(b.app.doc.panel_camera(1, 35).x, k[3].1);
    // Ctrl+Z : tout revient, d'un coup.
    b.app.undo();
    assert_eq!(cles(&b, 1), avant);
}

#[test]
fn le_plus_de_maj_sur_une_case_de_continuite_ajoute_b_vers_c_puis_c_vers_d() {
    let mut b = ab_puis_deux_cases();
    // Maj tenue, le pointeur sur la troisième case : le « + » est pour elle.
    let survoler = |b: &mut Banc, k: usize| {
        b.touches = MAJ;
        let c = vignette(b, k).center();
        b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        b.app.vignettes.plus.place
    };
    let cliquer = |b: &mut Banc, p: Pos2| {
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
        b.image(vec![appui(p, true, MAJ)], 1.0 / 60.0);
        b.image(vec![appui(p, false, MAJ)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
        b.chauffer(2);
    };
    let place = survoler(&mut b, K + 2).expect("le + est là");
    assert_eq!(place.ancre, K + 2, "le + sert la case survolée, pas la première du plan");
    cliquer(&mut b, place.centre);
    let k = cles(&b, 1);
    assert_eq!(k.iter().map(|k| k.0).collect::<Vec<_>>(), vec![0, 11, 24, 35], "B vers C sur la troisième case : {} ", b.app.status);
    assert!(b.app.status.contains("Cadre C ajouté"), "{}", b.app.status);
    // Encore un « + » sur la même case : un cadre de plus dans son mouvement (B, C, D), toujours sur sa durée.
    let place = survoler(&mut b, K + 2).expect("le + est encore là");
    assert_eq!(place.ancre, K + 2);
    cliquer(&mut b, place.centre);
    let k = cles(&b, 1);
    assert_eq!(k.iter().map(|k| k.0).collect::<Vec<_>>(), vec![0, 11, 24, 30, 35], "{}", b.app.status);
    assert!(b.app.status.contains("Cadre D ajouté"), "{}", b.app.status);
    // Sur la première case, le « + » allonge toujours le premier mouvement, sans toucher à celui de la troisième case.
    let place = survoler(&mut b, K).expect("le + de la première case");
    assert_eq!(place.ancre, K);
    cliquer(&mut b, place.centre);
    let k2 = cles(&b, 1);
    assert_eq!(k2.iter().map(|k| k.0).collect::<Vec<_>>(), vec![0, 6, 11, 24, 30, 35], "{}", b.app.status);
    assert_eq!((k2[3].1, k2[3].2), (k2[2].1, k2[2].2), "la troisième case part du nouveau dernier cadre de la première : la continuité tient");
}

#[test]
fn deplacer_b_emmene_le_depart_du_mouvement_suivant_et_le_premier_cadre_d_une_continuite_ne_se_deplace_pas() {
    let mut b = ab_puis_deux_cases();
    let w = largeur_cadre_ecran(&b);
    geste_camera(&mut b, K + 2, vec2(0.6 * w, 0.0));
    assert_eq!(cles(&b, 1).len(), 4, "{}", b.app.status);
    // On glisse B (sur la première case) vers le bas : la tenue de la troisième case suit.
    let l = b.app.board_layout();
    let q = l.move_of(K).unwrap().cadres[1].rect;
    let c = b.app.vignettes.xf.rect(q).center();
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, 0.25 * w), 6), c + vec2(0.0, 0.25 * w), true);
    b.chauffer(2);
    let k = cles(&b, 1);
    assert_eq!(k.len(), 4);
    assert_eq!((k[2].1, k[2].2), (k[1].1, k[1].2), "la case de continuité part toujours de B : {k:?}");
    assert!(k[1].2 > b.app.doc.default_camera().y + 1.0, "B est descendu");
    // Le premier cadre de la troisième case est la case : un glissé de caméra parti de là où C ne la couvre pas ne déplace rien.
    let avant = cles(&b, 1);
    let etapes = pas_d_annulation(&b);
    let case = vignette(&b, K + 2);
    let p = case.left_center() + vec2(0.08 * w, 0.0);
    ctrl_glisser(&mut b, p, &pas(p, vec2(0.0, 0.3 * w), 6), p + vec2(0.0, 0.3 * w), true);
    assert_eq!(cles(&b, 1), avant, "{}", b.app.status);
    assert_eq!(pas_d_annulation(&b), etapes);
    assert!(b.app.status.contains("tient la caméra de la case d'avant"), "{}", b.app.status);
}

#[test]
fn retirer_le_cadre_c_rend_la_case_a_l_arret_et_la_tenue_ne_se_retire_pas() {
    let mut b = ab_puis_deux_cases();
    let w = largeur_cadre_ecran(&b);
    let avant = cles(&b, 1);
    geste_camera(&mut b, K + 2, vec2(0.5 * w, 0.0));
    assert_eq!(cles(&b, 1).len(), 4);
    // Le premier cadre (la case) ne se retire pas.
    b.app.cam_retirer_cadre_case(1, 24, 0);
    assert_eq!(cles(&b, 1).len(), 4);
    assert!(b.app.status.contains("tient la caméra"), "{}", b.app.status);
    // C retiré : il ne resterait que la tenue ; le mouvement de la case s'en va tout entier.
    b.app.cam_retirer_cadre_case(1, 24, 1);
    assert_eq!(cles(&b, 1), avant, "{}", b.app.status);
    assert!(b.app.status.contains("Mouvement de caméra retiré"), "{}", b.app.status);
    b.chauffer(2);
    assert!(!b.app.board_layout().cards[K + 2].mouvement);
}

#[test]
fn un_mouvement_cree_sur_une_case_du_milieu_met_les_cases_de_derriere_dans_un_plan_a_part() {
    let mut b = ab_puis_deux_cases();
    let w = largeur_cadre_ecran(&b);
    // D'abord B vers C sur la troisième case (la dernière).
    geste_camera(&mut b, K + 2, vec2(0.5 * w, 0.0));
    let k = cles(&b, 1);
    let (bx, cx) = (k[1].1, k[3].1);
    let plans = b.app.doc.panels.len();
    let etapes = pas_d_annulation(&b);
    // Puis un mouvement sur la deuxième case, à l'arrêt entre les deux : B vers X. La troisième case, derrière elle, devient un plan à part
    // (la règle du 22 octobre), avec la caméra qu'elle montrait : B vers C.
    geste_camera(&mut b, K + 1, vec2(0.0, 0.3 * w));
    assert_eq!(b.app.doc.panels.len(), plans + 1, "{}", b.app.status);
    assert!(b.app.status.contains("plan à part"), "{}", b.app.status);
    assert_eq!(pas_d_annulation(&b), etapes + 1, "le mouvement et la coupe : une seule étape");
    let k1 = cles(&b, 1);
    assert_eq!(k1.iter().map(|k| k.0).collect::<Vec<_>>(), vec![0, 11, 12, 23], "{k1:?}");
    assert_eq!(k1[2].1, bx, "la deuxième case part de B");
    assert_eq!(b.app.doc.panels[1].frame_count(), 24);
    // Le plan neuf : la troisième case, avec son mouvement de B à C.
    let k2 = cles(&b, 2);
    assert!(k2.len() >= 2, "{k2:?}");
    assert_eq!((k2.first().unwrap().1, k2.last().unwrap().1), (bx, cx), "le plan neuf montre ce que la case montrait : {k2:?}");
    b.app.undo();
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(cles(&b, 1), k);
}

// ------------------------------------------------------------------------------ sa règle du 11 novembre au soir : la case où on le demande

/// Le plan 2 : deux cases de 12 images, aucune clé de caméra (son plan 5 : « je ne peux pas créer de mouvement de caméra depuis la deuxième
/// vignette, il s'applique automatiquement à la première »).
fn deux_cases_sans_mouvement() -> Banc {
    let mut doc = plan_court();
    doc.set_duration(1, 24);
    doc.toggle_mark(1, 12);
    let b = ouvert(doc, 6);
    let l = b.app.board_layout();
    assert_eq!((l.cards[K].frame, l.cards[K + 1].frame, l.cards[K + 1].plan), (0, 12, l.cards[K].plan));
    assert!(b.app.doc.panels[1].camera.is_empty());
    b
}

#[test]
fn un_plan_sans_mouvement_prend_son_mouvement_sur_la_case_ou_on_le_demande() {
    let mut b = deux_cases_sans_mouvement();
    let w = largeur_cadre_ecran(&b);
    let (plans, etapes) = (b.app.doc.panels.len(), pas_d_annulation(&b));
    let a = b.app.doc.panel_camera(1, 12);
    // Le geste de caméra sur la deuxième case : le mouvement naît sur elle, de sa première à sa dernière image.
    geste_camera(&mut b, K + 1, vec2(0.5 * w, 0.2 * w));
    let k = cles(&b, 1);
    assert_eq!(k.iter().map(|k| k.0).collect::<Vec<_>>(), vec![12, 23], "du début à la fin de la clé d'image : {} ", b.app.status);
    assert_eq!((k[0].1, k[0].2), (a.x, a.y), "il part du cadre que la case montrait");
    assert!(k[1].1 > a.x + 1.0, "{k:?}");
    assert!(b.app.status.contains("Cadre B posé") && b.app.status.contains("de A à B"), "{}", b.app.status);
    assert_eq!(pas_d_annulation(&b), etapes + 1);
    assert_eq!(b.app.doc.panels.len(), plans, "rien derrière elle : pas de coupe");
    // La planche : c'est la deuxième case qui porte l'empreinte ; la première reste une case ordinaire, à l'arrêt sur le même cadre.
    let l = b.app.board_layout();
    assert!(!l.cards[K].mouvement && l.cards[K + 1].mouvement, "le mouvement est sur la case où on l'a demandé");
    assert_eq!(l.move_of(K + 1).unwrap().cadres.len(), 2);
    for f in [0, 6, 11, 12] {
        let c = b.app.doc.panel_camera(1, f);
        assert_eq!((c.x, c.y), (a.x, a.y), "image {f} : la première case n'a pas bougé");
    }
    assert_eq!(b.app.doc.panel_camera(1, 23).x, k[1].1);
    // Ctrl+Z : plus aucune clé.
    b.app.undo();
    assert!(b.app.doc.panels[1].camera.is_empty());
}

#[test]
fn le_plus_de_maj_sert_la_deuxieme_case_d_un_plan_sans_mouvement_puis_la_premiere_garde_la_continuite() {
    let mut b = deux_cases_sans_mouvement();
    let survoler = |b: &mut Banc, k: usize| {
        b.touches = MAJ;
        let c = vignette(b, k).center();
        b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        b.app.vignettes.plus.place
    };
    let cliquer = |b: &mut Banc, p: Pos2| {
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
        b.image(vec![appui(p, true, MAJ)], 1.0 / 60.0);
        b.image(vec![appui(p, false, MAJ)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
        b.chauffer(2);
    };
    let place = survoler(&mut b, K + 1).expect("le + est là");
    assert_eq!(place.ancre, K + 1, "le + sert la case survolée");
    cliquer(&mut b, place.centre);
    let k = cles(&b, 1);
    assert_eq!(k.iter().map(|k| k.0).collect::<Vec<_>>(), vec![12, 23], "{}", b.app.status);
    assert!(b.app.status.contains("Cadre B ajouté"), "{}", b.app.status);
    assert!(b.app.board_layout().cards[K + 1].mouvement && !b.app.board_layout().cards[K].mouvement);
    // Puis un mouvement sur la première case : la deuxième, derrière elle, devient un plan à part (la règle du 22 octobre) et garde son
    // mouvement.
    let plans = b.app.doc.panels.len();
    let place = survoler(&mut b, K).expect("le + de la première case");
    assert_eq!(place.ancre, K);
    cliquer(&mut b, place.centre);
    assert_eq!(b.app.doc.panels.len(), plans + 1, "{}", b.app.status);
    assert_eq!(cles(&b, 1).iter().map(|k| k.0).collect::<Vec<_>>(), vec![0, 11], "{}", b.app.status);
    let k2 = cles(&b, 2);
    assert_eq!((k2.first().unwrap().1, k2.last().unwrap().1), (k[0].1, k[1].1), "le plan neuf garde le mouvement de la case : {k2:?}");
}

#[test]
fn une_camera_fixe_posee_au_debut_du_plan_laisse_la_troisieme_case_commencer_un_mouvement() {
    let mut doc = plan_court();
    doc.set_duration(1, 36);
    doc.toggle_mark(1, 12);
    doc.toggle_mark(1, 24);
    let d = doc.default_camera();
    let fixe = Camera { x: d.x + 40.0, ..d };
    doc.panels[1].camera = vec![CameraKey::new(0, fixe)];
    let mut b = ouvert(doc, 6);
    let w = largeur_cadre_ecran(&b);
    geste_camera(&mut b, K + 2, vec2(0.5 * w, 0.0));
    let k = cles(&b, 1);
    assert_eq!(k.iter().map(|k| k.0).collect::<Vec<_>>(), vec![0, 24, 35], "{}", b.app.status);
    assert_eq!((k[0].1, k[1].1), (fixe.x, fixe.x), "la clé du plan reste, la case part de son cadre");
    let l = b.app.board_layout();
    assert!(!l.cards[K].mouvement && !l.cards[K + 1].mouvement && l.cards[K + 2].mouvement);
    let g = vignettecam::groupes_du_plan(&b.app.doc, 1);
    assert_eq!((0..3).map(|r| lettre_du_cadre(&g, r)).collect::<String>(), "AAB");
}

#[test]
fn une_case_ou_la_camera_est_en_route_refuse_le_geste_et_le_dit() {
    let mut doc = plan_court();
    doc.set_duration(1, 36);
    doc.toggle_mark(1, 12);
    doc.toggle_mark(1, 24);
    let d = doc.default_camera();
    // Des clés posées à la main au montage : la caméra va de A à B sur les trois cases.
    doc.panels[1].camera = vec![CameraKey::new(0, d), CameraKey::new(30, Camera { x: d.x + 80.0, ..d })];
    let mut b = ouvert(doc, 6);
    let avant = cles(&b, 1);
    assert!(b.app.mouvement_de_case(1, 12).is_none() && b.app.mouvement_de_case(1, 24).is_none());
    assert!(!b.app.cam_possible_ici(1, 12));
    b.app.cam_ajouter_cadre_case(1, 12);
    assert_eq!(cles(&b, 1), avant);
    assert!(b.app.status.contains("déjà en mouvement"), "{}", b.app.status);
    // La première case, elle, porte toujours le mouvement du plan (les deux cadres), comme avant.
    assert_eq!(b.app.board_layout().move_of(K).map(|m| m.cadres.len()), Some(2));
}

// ------------------------------------------------------------------------------ sur une copie de son projet

/// Ses deux cas du 11 novembre au soir, sur une COPIE de son projet (hors de la suite ; `FEANOR_PROJET_ESSAI` : le manifeste de la copie).
/// Un plan de deux cases sans caméra (son plan 5) : le mouvement demandé sur la deuxième case naît sur elle. Un plan dont la clé B est
/// au-delà de sa première case (son plan 3) : la dernière case dit pourquoi elle ne peut pas en commencer un. Rien n'est enregistré.
#[test]
#[ignore = "lit une COPIE de projet : FEANOR_PROJET_ESSAI"]
fn ses_deux_cas_sur_une_copie_de_son_projet() {
    let Some(chemin) = std::env::var_os("FEANOR_PROJET_ESSAI") else { return };
    let doc = crate::project::load(std::path::Path::new(&chemin)).expect("le projet s'ouvre").doc;
    let mut b = ouvert(doc, 6);
    let pi = b.app.doc.panels.iter().position(|p| p.camera.is_empty() && p.marks.len() == 1).expect("un plan de deux cases sans caméra");
    let (debut, fin) = (b.app.doc.panels[pi].marks[0].frame, b.app.doc.panels[pi].frame_count());
    b.app.cam_ajouter_cadre_case(pi, debut);
    let k: Vec<usize> = b.app.doc.panels[pi].camera.iter().map(|k| k.frame).collect();
    println!("CAS plan {} : clés {k:?} ({})", pi + 1, b.app.status);
    assert_eq!(k, vec![debut, fin - 1], "du début à la fin de la clé d'image");
    assert_eq!(vignettecam::groupes_du_plan(&b.app.doc, pi)[0].debut, debut);
    b.chauffer(3);
    let cartes: Vec<(usize, bool)> = b.app.board_layout().cards.iter().filter(|c| c.panel == pi).map(|c| (c.frame, c.mouvement)).collect();
    println!("CAS cases du plan {} (image de début, porte un mouvement) : {cartes:?}", pi + 1);
    assert_eq!(cartes, vec![(0, false), (debut, true)]);
    // Un plan dont le mouvement, parti de la première case, traverse les suivantes.
    match b.app.doc.panels.iter().position(|p| p.camera.len() == 2 && p.marks.len() == 2 && p.camera[1].frame >= p.marks[0].frame) {
        Some(pj) => {
            let derniere = b.app.doc.panels[pj].marks[1].frame;
            let avant = b.app.doc.panels[pj].camera.clone();
            b.app.cam_ajouter_cadre_case(pj, derniere);
            println!("CAS plan {} : {}", pj + 1, b.app.status);
            assert_eq!(b.app.doc.panels[pj].camera, avant, "rien n'est changé");
            assert!(b.app.status.contains("déjà en mouvement"), "{}", b.app.status);
        }
        None => println!("CAS pas de plan dont le mouvement traverse les cases dans cette copie"),
    }
}

// ------------------------------------------------------------------------------ à regarder

/// À regarder (hors de la suite) : la planche avec un mouvement A vers B, deux cases de continuité, puis B vers C sur la troisième case et
/// C vers D sur une quatrième. Quatre images dans `FEANOR_CAPTURE_DIR`. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn camera_suite_captures() {
    use crate::workspace::Espace;
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("SUITE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("SUITE {nom}");
    };
    let mut doc = plan_court();
    doc.set_duration(1, 12);
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.layout.board_textes = crate::boardsens::Textes::aucun();
    b.app.set_espace(Espace::Vignettes);
    b.app.board_zoom = 0.0;
    b.app.vignettes.ouverture_a_faire = true;
    b.chauffer(16);
    b.attendre_vignettes();
    // A vers B sur la première case du plan 2, puis deux cases de continuité.
    let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(0.7 * w, 0.0), 6), case.center() + vec2(0.7 * w, 0.0), true);
    b.app.doc.set_duration(1, 36);
    b.app.doc.toggle_mark(1, 12);
    b.app.doc.toggle_mark(1, 24);
    b.chauffer(6);
    b.attendre_vignettes();
    // 1. Maj tenue sur la troisième case : le « + » est à côté d'elle.
    b.touches = MAJ;
    let c = vignette(&b, K + 2).center();
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.chauffer(3);
    sauver(&b, "camera-suite-01-maj-sur-la-case-de-continuite.png");
    b.touches = Modifiers::NONE;
    b.chauffer(2);
    // 2. B vers C sur la troisième case.
    geste_camera(&mut b, K + 2, vec2(0.6 * w, 0.35 * w));
    println!("SUITE {}", b.app.status);
    b.chauffer(6);
    b.attendre_vignettes();
    sauver(&b, "camera-suite-02-b-vers-c.png");
    // 3. Une case de continuité de plus, qui montre C ; puis C vers D.
    b.app.doc.set_duration(1, 48);
    b.app.doc.toggle_mark(1, 36);
    b.chauffer(6);
    b.attendre_vignettes();
    sauver(&b, "camera-suite-03-la-case-suivante-montre-c.png");
    geste_camera(&mut b, K + 3, vec2(-0.5 * w, 0.3 * w));
    println!("SUITE {}", b.app.status);
    b.chauffer(6);
    b.attendre_vignettes();
    sauver(&b, "camera-suite-04-c-vers-d.png");
    println!("SUITE clés : {:?}", cles(&b, 1).iter().map(|k| k.0).collect::<Vec<_>>());
}