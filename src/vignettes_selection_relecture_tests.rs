//! La relecture de la partie 125 (2026-11-08) : chaque défaut relevé a son essai, au vrai geste sur le `Banc`, en pixels ou en données. Rouges sur
//! 66c533d pour ceux qui étaient réels, verts après la correction.

use eframe::egui::{pos2, vec2, Event, Modifiers};

use crate::app::Tool;
use crate::document::{CelId, LayerKind};
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, planche, relever, repos, tracer};
use crate::vector::{Element, VPoint};
use crate::vignettes_selection_tests::{absent, choisies, glisser_avec, glisser_droit, lasso, parts, present, sur_le_trait, trait_a_travers, voir};
use crate::vignettes_tests::appui;

/// Le dessin vectoriel de la case `k` (calque 0, à son image).
fn dessin_de(b: &Banc, k: usize) -> Option<CelId> {
    let l = b.app.board_layout();
    let c = l.cards[k];
    b.app.doc.panels[c.panel].layers[0].cel_at(c.frame)
}

/// Le numéro des parts de l'élément `i` du dessin de la case `k`.
fn lien_de(b: &Banc, k: usize, i: usize) -> Option<u64> {
    match b.app.drawing(dessin_de(b, k)?)?.elements.get(i)? {
        Element::Stroke { trace, .. } => trace.lien,
        _ => None,
    }
}

/// Les rangs, dans le dessin de la case `k`, des parts du numéro `lien`.
fn rangs(b: &Banc, k: usize, lien: u64) -> Vec<usize> {
    let Some(d) = dessin_de(b, k).and_then(|c| b.app.drawing(c)) else { return Vec::new() };
    d.elements.iter().enumerate().filter(|(_, e)| matches!(e, Element::Stroke { trace, .. } if trace.lien == Some(lien))).map(|(i, _)| i).collect()
}

fn nombre(b: &Banc, k: usize) -> usize {
    dessin_de(b, k).and_then(|c| b.app.drawing(c)).map_or(0, |d| d.elements.len())
}

/// La case `k` à l'écran.
fn case(b: &Banc, k: usize) -> eframe::egui::Rect {
    let l = b.app.board_layout();
    b.app.vignettes.xf.rect(l.cards[k].thumb)
}

/// Prendre la boîte loin de son pivot et la glisser de `d`.
fn deplacer_le_choix(b: &mut Banc, d: eframe::egui::Vec2) {
    let q = crate::vignettes_selection_tests::boite(b);
    let prise = pos2(q[0].x * 0.7 + q[2].x * 0.3, q[0].y * 0.7 + q[2].y * 0.3);
    glisser_droit(b, prise, d);
}

// ------------------------------------------------------------------------------ 1. l'ordre de peinture

/// Le premier point de l'élément `i` du dessin de la case `k`.
fn depart_de(b: &Banc, k: usize, i: usize) -> Option<eframe::egui::Pos2> {
    match b.app.drawing(dessin_de(b, k)?)?.elements.get(i)? {
        Element::Stroke { points, .. } => points.first().map(|p| p.pos()),
        _ => None,
    }
}

/// Partie 131 : le trait n'est plus coupé, il est entier dans le dessin de sa case. Déplacé sans quitter sa case, il y garde son rang ; peint
/// derrière et déplacé dans une autre case, il y passe sous ce qui y était. (Avant : la même chose, part par part.)
#[test]
fn a_moved_stroke_keeps_its_rank_in_its_case_and_one_painted_behind_stays_behind() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let t2 = case(&b, 2);
    // Deux traits dans la case 2 : le second, posé après, est dessus.
    let (de, a) = (t2.center() + vec2(-40.0, -20.0), t2.center() + vec2(40.0, -6.0));
    tracer(&mut b, de, a);
    tracer(&mut b, t2.center() + vec2(-40.0, 20.0), t2.center() + vec2(40.0, 20.0));
    repos(&mut b);
    assert_eq!(nombre(&b, 2), 2);
    let (premier, second) = (depart_de(&b, 2, 0).unwrap(), depart_de(&b, 2, 1).unwrap());
    lasso(&mut b, sur_le_trait(de, a, t2.center().x), 8.0);
    assert_eq!(choisies(&b), 1, "le premier trait seul ({})", b.app.status);
    deplacer_le_choix(&mut b, vec2(0.0, 6.0));
    repos(&mut b);
    assert_eq!(nombre(&b, 2), 2, "le trait déplacé reste dans le dessin de sa case");
    let (p, s) = (depart_de(&b, 2, 0).unwrap(), depart_de(&b, 2, 1).unwrap());
    assert!(p.y > premier.y + 1.0 && (p.x - premier.x).abs() < 1.0, "le trait déplacé garde son rang : toujours sous le second ({premier:?} -> {p:?})");
    assert!((s - second).length() < 0.01, "le second trait n'a pas bougé");

    // Peint derrière, déplacé dans une case d'arrivée qui a déjà un trait : il y passe dessous.
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return };
    let (t1, t2) = (case(&b, 1), case(&b, 2));
    for t in [t1, t2] {
        tracer(&mut b, t.center() + vec2(-40.0, 25.0), t.center() + vec2(40.0, 25.0));
        repos(&mut b);
    }
    b.app.paint_behind = true;
    let (de, a) = (t1.center() + vec2(-30.0, -20.0), t1.center() + vec2(30.0, -8.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    b.app.paint_behind = false;
    assert_eq!(nombre(&b, 1), 2);
    let derriere = depart_de(&b, 1, 0).unwrap();
    assert!(derriere.y < depart_de(&b, 1, 1).unwrap().y, "peint derrière : il est le premier élément du dessin de la case 1");
    lasso(&mut b, sur_le_trait(de, a, t1.center().x), 8.0);
    assert_eq!(choisies(&b), 1, "le trait peint derrière ({})", b.app.status);
    deplacer_le_choix(&mut b, vec2(t2.center().x - t1.center().x, 0.0));
    repos(&mut b);
    assert_eq!((nombre(&b, 1), nombre(&b, 2)), (1, 2), "le trait a quitté la case 1 pour la case 2, entier");
    let arrive = depart_de(&b, 2, 0).unwrap();
    assert!((arrive - derriere).length() < 2.0, "dans la case d'arrivée, un trait peint derrière passe sous ce qui y était : {derriere:?} -> {arrive:?}");
    let _ = (lien_de, rangs, trait_a_travers);
}

// ------------------------------------------------------------------------------ 2. aucune perte

/// Verrouille le calque des plans des cases `ks`.
fn verrouiller(b: &mut Banc, ks: &[usize]) {
    let l = b.app.board_layout();
    for &k in ks {
        b.app.doc.panels[l.cards[k].panel].layers[0].locked = true;
    }
}

/// Partie 131 : un trait parti d'une case qui reçoit reste entier dans son plan, même s'il traverse des cases verrouillées (elles ne reçoivent
/// rien, rien ne va par-dessus elles). Déplacé pour **commencer** dans une case verrouillée, il reste entier sur la planche, par-dessus son
/// image, visible. (Avant : ses parts sur les cases verrouillées étaient par-dessus, une par case.)
#[test]
fn a_stroke_moved_onto_a_case_that_refuses_stays_whole_above_its_image() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    verrouiller(&mut b, &[1, 2, 3]);
    let (t0, t1, t2) = (case(&b, 0), case(&b, 1), case(&b, 2));
    // (En biais : la boîte du choix a de la hauteur, la prendre ne tombe pas sur un bord.)
    let (de, a) = (t0.center() + vec2(0.0, -18.0), t2.center() + vec2(0.0, 18.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(parts(&b), (1, 0), "le trait est entier dans le plan de la case 0 ; rien sur la planche, rien dans les cases verrouillées");
    present(&voir(&mut b, "relecture-dessus-avant", de, a), "il se voit en entier, par-dessus les cases verrouillées qu'il traverse");
    lasso(&mut b, sur_le_trait(de, a, t0.center().x + 20.0), 10.0);
    assert_eq!(choisies(&b), 1, "le trait entier ({})", b.app.status);
    let d = vec2(t1.center().x - t0.center().x, 0.0);
    deplacer_le_choix(&mut b, d);
    println!("dessus, après : {} ; {:?}", b.app.status, parts(&b));
    assert_eq!(parts(&b), (0, 1), "il commence dans une case verrouillée : entier sur la planche");
    assert!(b.app.doc.annotations.traces.iter().all(|t| t.dessus), "par-dessus l'image de la case qui ne pouvait pas le recevoir");
    let r = voir(&mut b, "relecture-dessus", de + d, a + d);
    present(&r, "le trait reste visible, entier, par-dessus les images");
}

/// Un projet dont la planche est pleine : des traits d'un calque d'annotation caché.
fn remplir_la_planche(b: &mut Banc) {
    let l = b.app.board_layout();
    let id = b.app.doc.annotations.ajouter_calque().expect("un calque");
    let c = l.cards[11].thumb.center();
    while b.app.doc.annotations.traces.len() < crate::annotations::MAX_TRACES {
        assert!(b.app.doc.annotations.ajouter_dans(&l, Some(id), &[VPoint { x: c.x, y: c.y, r: 1.0 }, VPoint { x: c.x + 1.0, y: c.y, r: 1.0 }], [0, 0, 0]));
    }
    b.app.doc.annotations.regler_visible(id, false);
}

#[test]
fn a_full_board_refuses_the_move_and_loses_nothing() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let t2 = case(&b, 2);
    let (de, a) = (t2.center() + vec2(-30.0, -15.0), t2.center() + vec2(30.0, 15.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    crate::vignettes_tests::clic_avec(&mut b, t2.center(), Modifiers::CTRL);
    b.touches = Modifiers::NONE;
    assert_eq!(choisies(&b), 1);
    remplir_la_planche(&mut b);
    let (avant, pas) = (parts(&b), b.app.history.undo_len());
    // Loin sous la planche, hors de toute case : il faudrait une place sur la planche, il n'y en a plus.
    deplacer_le_choix(&mut b, vec2(0.0, 380.0));
    println!("planche pleine : {} ; parts {:?}", b.app.status, parts(&b));
    assert_eq!(parts(&b), avant, "rien n'est retiré : le geste est refusé tel quel");
    assert_eq!(b.app.history.undo_len(), pas, "aucune étape");
    present(&voir(&mut b, "relecture-pleine", de, a), "le trait est toujours à sa place");
    assert!(b.app.status.contains("planche"), "la ligne d'état dit pourquoi : {}", b.app.status);
}

#[test]
fn a_camera_that_cannot_see_part_of_the_stroke_loses_nothing() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let p3 = l.cards[3].panel;
    let base = b.app.doc.default_camera();
    crate::camera::set_key(&mut b.app.doc.panels[p3].camera, 0, crate::camera::Camera { tilt: 85.0, ..base });
    b.app.renderer.invalidate_all();
    b.chauffer(4);
    let (t2, t3) = (case(&b, 2), case(&b, 3));
    // Un trait de la case 2 vers la droite, déplacé pour commencer dans la case 3 (caméra très basculée) et filer loin à droite.
    let (de, a) = (t2.center() + vec2(0.0, -20.0), t2.center() + vec2(60.0, 20.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    crate::vignettes_tests::clic_avec(&mut b, t2.center() + vec2(30.0, 0.0), Modifiers::CTRL);
    assert_eq!(choisies(&b), 1, "le trait est pris");
    b.touches = Modifiers::NONE;
    let d = vec2(t3.center().x - t2.center().x, 0.0);
    let echelle = vec2(9.0, 1.0);
    // (D'abord l'étirer loin vers la droite, sur la planche, puis le glisser : la caméra de la case 3 ne voit pas sa fin.)
    let q = crate::vignettes_selection_tests::boite(&b);
    let cote = pos2(q[1].x, (q[1].y + q[2].y) * 0.5);
    glisser_droit(&mut b, cote, vec2(60.0 * (echelle.x - 1.0), 0.0));
    let l = b.app.board_layout();
    let pose = crate::horschamp::pose_de_la_case(&b.app.doc, &l, 3).expect("une pose");
    let xf = b.app.vignettes.xf;
    let loin = xf.to_board(a + vec2(60.0 * (echelle.x - 1.0), 0.0) + d);
    println!("la caméra de la case 3 voit-elle le bout ? {:?}", pose.dessin_sur(loin));
    let avant = parts(&b);
    deplacer_le_choix(&mut b, d);
    println!("caméra : {} ; parts {:?} -> {:?}", b.app.status, avant, parts(&b));
    assert!(parts(&b).0 + parts(&b).1 >= 1, "le trait n'est jamais perdu");
    repos(&mut b);
    let img = b.capturer().unwrap();
    let r = relever(&img, de + d, a + d + vec2(60.0 * (echelle.x - 1.0), 0.0));
    println!("caméra : {r:?}");
    assert!(r.largeur.1 > 0, "le trait se voit encore");
}

// ------------------------------------------------------------------------------ 3. recolorer

#[test]
fn recoloring_also_recolors_the_parts_on_the_paper() {
    let Some((mut b, de, a, t1, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    lasso(&mut b, sur_le_trait(de, a, t2.center().x), 14.0);
    b.app.color = [20, 40, 220];
    b.app.recolor_selection();
    repos(&mut b);
    let choix = b.app.vignettes.selp.take();
    b.chauffer(2);
    let img = b.capturer().unwrap();
    b.app.vignettes.selp = choix;
    // Sur le papier, au-dessus de la case 1 : la part de la planche.
    let fin_papier = sur_le_trait(de, a, de.x + (a.x - de.x) * ((t1.top() - 22.0 - de.y) / (a.y - de.y)));
    let (mut bleu, mut rouge) = (0, 0);
    for k in 2..20 {
        let p = de + (fin_papier - de) * (k as f32 / 20.0);
        for w in -3..=3 {
            let q = p + vec2(0.0, w as f32);
            let v = crate::traits_continus_tests::px(&img, q.x.round() as i32, q.y.round() as i32);
            if v[2] - v[0] > 60 {
                bleu += 1;
            }
            if v[0] - v[1] > 60 {
                rouge += 1;
            }
        }
    }
    println!("papier : {bleu} pixels bleus, {rouge} rouges");
    assert!(bleu > 0 && rouge == 0, "la part de la planche prend la nouvelle couleur ({bleu} bleus, {rouge} rouges)");
}

// ------------------------------------------------------------------------------ 4. le focus

#[test]
fn delete_and_copy_go_to_the_board_selection_only_when_the_drawing_has_the_hand() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    lasso(&mut b, sur_le_trait(de, a, t2.center().x), 14.0);
    let avant = parts(&b);
    // Un clic dans le montage a donné la main aux plans : Suppr est pour eux, pas pour les traits.
    b.app.panel_focus = true;
    b.app.run_action("edition.supprimer");
    println!("focus aux plans : {} ; parts {:?}", b.app.status, parts(&b));
    assert_eq!(parts(&b).0 + parts(&b).1, avant.0 + avant.1, "Suppr ne touche pas aux traits quand le montage a la main");
    b.app.panel_focus = false;
    // Le calque courant devient bitmap : le choix de la planche ne vaut plus.
    let pi = b.app.cur_panel;
    let mut l = crate::document::Layer::new("Bitmap", LayerKind::Bitmap);
    l.frames = vec![None; b.app.doc.panels[pi].frame_count()];
    b.app.doc.panels[pi].layers.push(l);
    b.app.cur_layer = b.app.doc.panels[pi].layers.len() - 1;
    b.chauffer(2);
    b.app.run_action("edition.supprimer");
    assert_eq!(parts(&b).0 + parts(&b).1, avant.0 + avant.1, "calque bitmap : Suppr ne touche pas aux traits de la planche");
    assert!(!b.app.selp_active(), "le choix est levé");
}

// ------------------------------------------------------------------------------ 5. les gestes d'avant

#[test]
fn the_camera_tool_keeps_ctrl_and_the_lasso_still_takes_fills_with_the_strokes() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    // a) L'outil Caméra : Ctrl + glisser reste à la caméra.
    b.app.set_tool(Tool::Camera);
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 14.0);
    assert!(b.app.vignettes.selp.as_ref().is_none_or(|s| s.traits.is_empty() && s.lasso.is_none()), "l'outil Caméra n'ouvre pas le lasso de la planche");
    b.app.set_tool(Tool::Brush);
    b.chauffer(2);
    // b) Une forme remplie dans la case 2, prise avec le trait par le même lasso, déplacée avec lui en une étape.
    let cel = dessin_de(&b, 2).expect("le dessin de la case 2");
    let l = b.app.board_layout();
    let pose = crate::horschamp::pose_de_la_case(&b.app.doc, &l, 2).unwrap();
    let xf = b.app.vignettes.xf;
    let c = pose.dessin(xf.to_board(m + vec2(0.0, 18.0)));
    let carre = vec![vec![[c.x - 60.0, c.y - 60.0], [c.x + 60.0, c.y - 60.0], [c.x + 60.0, c.y + 60.0], [c.x - 60.0, c.y + 60.0]]];
    b.app.drawing_mut(cel).unwrap().elements.insert(0, Element::Fill { contours: carre, color: [30, 160, 60], opacity: 1.0, gradient: None });
    b.app.refresh_cel(cel);
    b.chauffer(2);
    crate::vignettes_selection_tests::lasso_par(&mut b, &[m + vec2(-16.0, -14.0), m + vec2(16.0, -14.0), m + vec2(16.0, 26.0), m + vec2(-16.0, 26.0), m + vec2(-16.0, -14.0)]);
    println!("choix : {} parts ; {}", choisies(&b), b.app.status);
    let pas = b.app.history.undo_len();
    let d = vec2(30.0, 0.0);
    deplacer_le_choix(&mut b, d);
    let x = b.app.drawing(cel).unwrap().elements.iter().find_map(|e| match e {
        Element::Fill { contours, .. } => Some(contours[0][0][0]),
        _ => None,
    });
    let attendu = c.x - 60.0 + d.x / (xf.zoom * pose.echelle_en(c));
    println!("forme : x {x:?}, attendu {attendu}");
    assert!(x.is_some_and(|x| (x - attendu).abs() < 4.0), "la forme remplie a suivi le geste, dans sa case ({x:?} contre {attendu})");
    assert_eq!(b.app.history.undo_len(), pas + 1, "un seul geste, une étape");
    // (Le trait déplacé passe sur le coin de la case courante : son filet et son liseré d'accent, peints par-dessus, le croisent de biais sur quelques pixels, comme en partie 122.)
    let r = voir(&mut b, "relecture-forme-et-trait", de + d, a + d);
    assert!(r.trou <= 8, "le trait entier a suivi : {r:?}");
}

// ------------------------------------------------------------------------------ 6. le plan dupliqué

#[test]
fn a_duplicated_plan_next_to_the_rest_of_the_stroke_is_not_taken_with_it() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let (t1, t3) = (case(&b, 1), case(&b, 3));
    let (de, a) = (t1.center() + vec2(-40.0, 12.0), t3.center() + vec2(40.0, 12.0));
    // (Un trait d'un projet d'avant la partie 131 : coupé en parts de même numéro, une par case.)
    crate::vignettes_selection_tests::tracer_comme_avant(&mut b, de, a);
    let p = de + vec2(15.0, 0.0);
    assert!(t1.contains(p));
    let l = b.app.board_layout();
    let plan = l.cards[1].panel;
    b.app.select_board(plan, 0);
    b.app.duplicate_panel();
    b.chauffer(4);
    repos(&mut b);
    let t2 = case(&b, 2);
    // La copie est dans la case 2, juste avant la case qui porte la suite du trait (maintenant la case 3).
    lasso(&mut b, p + vec2(t2.left() - t1.left(), 0.0), 8.0);
    println!("plan dupliqué : {} parts choisies ; {}", choisies(&b), b.app.status);
    assert_eq!(choisies(&b), 1, "seule la part de la copie : elle n'est pas une part du trait d'origine");
}

// ------------------------------------------------------------------------------ 7. l'historique plein

fn remplir_l_historique(b: &mut Banc) {
    for _ in 0..crate::history::LIMIT + 5 {
        let s = b.app.snapshot();
        b.app.history.push(s);
    }
}

#[test]
fn a_full_history_still_makes_one_step() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    let avant = parts(&b);
    remplir_l_historique(&mut b);
    lasso(&mut b, sur_le_trait(de, a, t2.center().x), 14.0);
    let d = vec2(40.0, 0.0);
    deplacer_le_choix(&mut b, d);
    present(&voir(&mut b, "relecture-histoire-deplace", de + d, a + d), "déplacé");
    b.app.undo();
    present(&voir(&mut b, "relecture-histoire-annule", de, a), "un seul Ctrl+Z rend tout le trait à sa place");
    assert_eq!(parts(&b), avant, "un seul Ctrl+Z rend les parts d'avant");
    // Le trait tracé à travers les cases et coupé comme avant la partie 131 (plusieurs étapes regroupées), historique plein : un Ctrl+Z l'enlève entier.
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return };
    remplir_l_historique(&mut b);
    let (t1, t3) = (case(&b, 1), case(&b, 3));
    crate::vignettes_selection_tests::tracer_comme_avant(&mut b, t1.center(), t3.center());
    assert!(parts(&b).0 >= 3);
    b.app.undo();
    repos(&mut b);
    assert_eq!(parts(&b), (0, 0), "historique plein : un Ctrl+Z enlève tout le trait réparti");
    // Depuis la partie 131 : un trait parti du papier passe entier sur la planche au lâcher (le trait, puis sa pose : deux étapes regroupées) ;
    // historique plein, un Ctrl+Z l'enlève.
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return };
    remplir_l_historique(&mut b);
    let t1 = case(&b, 1);
    let de = pos2(t1.center().x, t1.top() - t1.width() * 0.2);
    tracer(&mut b, de, t1.center());
    repos(&mut b);
    assert_eq!(parts(&b), (0, 1), "le trait du papier : {}", b.app.status);
    b.app.undo();
    repos(&mut b);
    assert_eq!(parts(&b), (0, 0), "historique plein : un Ctrl+Z enlève le trait du papier");
}

// ------------------------------------------------------------------------------ 8. le coût

#[test]
fn the_cost_with_sixty_cases_and_two_thousand_strokes() {
    let Some(mut b) = planche(des_plans(60, false), 6, 0.6) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    // 2 000 traits courts, chacun avec son numéro (le pire cas pour la recherche des parts).
    let mut n = 0;
    for k in 0..l.cards.len() {
        let c = l.cards[k];
        let pi = c.panel;
        let cel = match b.app.doc.panels[pi].layers[0].cel_at(c.frame) {
            Some(id) => id,
            None => {
                let id = b.app.doc.new_cel(LayerKind::Vector);
                let fc = b.app.doc.panels[pi].frame_count();
                for f in 0..fc {
                    b.app.doc.panels[pi].layers[0].set(f, Some(id));
                }
                id
            }
        };
        let d = b.app.drawing_mut(cel).unwrap();
        for j in 0..34 {
            let (x, y) = (100.0 + (j % 6) as f32 * 290.0, 100.0 + (j / 6) as f32 * 160.0);
            let points: Vec<VPoint> = (0..20).map(|t| VPoint { x: x + t as f32 * 8.0, y: y + (t as f32 * 0.7).sin() * 30.0, r: 4.0 }).collect();
            d.elements.push(Element::Stroke { points, color: [200, 30, 40], opacity: 1.0, hardness: 1.0, brush: None, trace: crate::vector::Trace { lien: Some(crate::horschamp::nouveau_lien()), ..Default::default() } });
            n += 1;
        }
        b.app.refresh_cel(cel);
    }
    println!("{n} traits sur {} cases", l.cards.len());
    b.chauffer(4);
    repos(&mut b);
    // Un grand lasso sur la moitié de la planche.
    let v = b.app.vignettes.view;
    let (g, h) = (v.left_top() + vec2(20.0, 20.0), v.center() + vec2(0.0, 120.0));
    let coins = [g, pos2(h.x, g.y), h, pos2(g.x, h.y), g];
    let mut chemin = Vec::new();
    for w in coins.windows(2) {
        for k in 0..8 {
            chemin.push(w[0] + (w[1] - w[0]) * (k as f32 / 8.0));
        }
    }
    b.touches = Modifiers::CTRL;
    b.image(vec![Event::PointerMoved(chemin[0])], 1.0 / 60.0);
    b.image(vec![appui(chemin[0], true, Modifiers::CTRL)], 1.0 / 60.0);
    for q in &chemin[1..] {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
    }
    let lacher = b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::CTRL)], 1.0 / 60.0).construction;
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    let pris = b.app.vignettes.selp.as_ref().map_or(0, |s| s.traits.len());
    let mesurer = |b: &mut Banc| {
        let mut v2: Vec<f64> = (0..30).map(|k| b.image(vec![Event::PointerMoved(v.center() + vec2(k as f32, 0.0))], 1.0 / 60.0).construction).collect();
        v2.sort_by(f64::total_cmp);
        v2[15]
    };
    let avec = mesurer(&mut b);
    let choix = b.app.vignettes.selp.take();
    let sans = mesurer(&mut b);
    b.app.vignettes.selp = choix;
    println!("COÛT : {pris} traits pris ; lâcher du lasso {lacher:.1} ms ; image (médiane) : {sans:.2} ms sans le choix, {avec:.2} ms avec");
    assert!(pris > 300, "le lasso prend beaucoup de traits ({pris})");
    assert!(lacher < 100.0, "le lâcher du lasso reste sous 100 ms ({lacher:.1})");
    assert!(avec - sans < 4.0, "le choix coûte moins de 4 ms par image ({sans:.2} -> {avec:.2})");
    let _ = (absent, glisser_avec);
}
