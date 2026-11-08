//! Les bords des cases ne coupent jamais le dessin (partie 125, Anthony, 2026-11-08 : « quand je sélectionne des traits dans un cadre et que je les
//! déplace ou les transforme, seule la partie dans le champ du cadre est prise, le reste est coupé »). Au vrai geste sur la planche (Ctrl + lasso dans
//! une case, sur le papier ou à cheval, puis Ctrl + glisser dans la boîte), vu en pixels comme les parties 111 et 122 : le trait entier bouge, sans
//! trou ni doublon, une seule étape d'annulation.

use eframe::egui::{pos2, vec2, Event, Modifiers, Pos2, Rect, Vec2};

use crate::perf_tests::Banc;
use crate::selbox::{Handle, Mods};
use crate::traits_continus_tests::{des_plans, planche, relever, repos, sauver, tracer, Releve};
use crate::vector::Affine;
use crate::vignettes_tests::appui;

/// Un glissé à la souris, touches `m` tenues tout du long (un point par image d'écran).
pub(crate) fn glisser_avec(b: &mut Banc, path: &[Pos2], m: Modifiers) {
    b.touches = m;
    b.image(vec![Event::PointerMoved(path[0])], 1.0 / 60.0);
    b.image(vec![appui(path[0], true, m)], 1.0 / 60.0);
    for q in &path[1..] {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
    }
    b.image(vec![appui(*path.last().expect("point"), false, m)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
}

/// Un lasso fermé par les points `coins` (écran), Ctrl tenu.
pub(crate) fn lasso_par(b: &mut Banc, coins: &[Pos2]) {
    let mut chemin = Vec::new();
    for w in coins.windows(2) {
        for k in 0..8 {
            chemin.push(w[0] + (w[1] - w[0]) * (k as f32 / 8.0));
        }
    }
    chemin.push(coins[coins.len() - 1]);
    glisser_avec(b, &chemin, Modifiers::CTRL);
}

/// Un lasso carré de côté `2 c`, autour de `p`, Ctrl tenu.
pub(crate) fn lasso(b: &mut Banc, p: Pos2, c: f32) {
    lasso_par(b, &[p + vec2(-c, -c), p + vec2(c, -c), p + vec2(c, c), p + vec2(-c, c), p + vec2(-c, -c)]);
}

/// Un glissé droit de `de` sur `d`, Ctrl tenu (le geste de la boîte).
pub(crate) fn glisser_droit(b: &mut Banc, de: Pos2, d: Vec2) {
    let n = (d.length() / 4.0).ceil().max(2.0) as usize;
    let chemin: Vec<Pos2> = (0..=n).map(|k| de + d * (k as f32 / n as f32)).collect();
    glisser_avec(b, &chemin, Modifiers::CTRL);
}

/// Le point du segment `de`–`a` à l'abscisse `x`.
pub(crate) fn sur_le_trait(de: Pos2, a: Pos2, x: f32) -> Pos2 {
    let t = (x - de.x) / (a.x - de.x);
    de + (a - de) * t
}

/// La planche de la capture : six cases par rangée, un trait du papier au-dessus de la case 1 au papier sous la case 3, par le centre de la case 2.
/// Rend le banc, les deux bouts du trait à l'écran, et les images des cases 1 et 2.
pub(crate) fn trait_a_travers(bitmap: bool) -> Option<(Banc, Pos2, Pos2, Rect, Rect)> {
    let mut b = planche(des_plans(12, bitmap), 6, 1.0)?;
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (t1, t2) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[2].thumb));
    let de = pos2(t1.center().x, t1.top() - 30.0);
    let a = pos2(2.0 * t2.center().x - de.x, 2.0 * t2.center().y - de.y);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert!(t2.shrink(22.0).contains(sur_le_trait(de, a, t2.center().x)), "le trait passe par le milieu de la case 2");
    Some((b, de, a, t1, t2))
}

/// Les parts du trait : (éléments vectoriels des dessins, traits de la planche).
pub(crate) fn parts(b: &Banc) -> (usize, usize) {
    let doc = &b.app.doc;
    let el = doc.cels.values().filter_map(|c| c.vector.as_ref()).map(|v| v.elements.len()).sum();
    (el, doc.annotations.traces.len())
}

/// Où sont les parts (pour lire un échec).
pub(crate) fn decrire(b: &Banc) {
    for (i, p) in b.app.doc.panels.iter().enumerate() {
        for l in &p.layers {
            for c in l.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>() {
                if let Some(v) = b.app.doc.cels.get(c).and_then(|c| c.vector.as_ref()) {
                    for e in &v.elements {
                        if let crate::vector::Element::Stroke { points, trace, .. } = e {
                            println!("  plan {i} : {} points de {:?} à {:?}, lien {:?}", points.len(), points.first().map(|p| p.pos()), points.last().map(|p| p.pos()), trace.lien);
                        }
                    }
                }
            }
        }
    }
    for t in &b.app.doc.annotations.traces {
        println!("  planche : {} points, lien {:?}, dessus {}", t.points.len(), t.lien, t.dessus);
    }
    println!("  état : {} ; cur_panel {}", b.app.status, b.app.cur_panel);
}

/// Combien de parts sont choisies sur la planche.
pub(crate) fn choisies(b: &Banc) -> usize {
    if !b.app.selp_active() {
        return 0;
    }
    b.app.vignettes.selp.as_ref().map_or(0, |s| s.traits.iter().map(|t| t.parts.len()).sum())
}

pub(crate) fn voir(b: &mut Banc, nom: &str, de: Pos2, a: Pos2) -> Releve {
    repos(b);
    // (Le choix souligne le trait d'un filet orange : on le lève le temps de la capture, pour ne voir que le dessin.)
    let choix = b.app.vignettes.selp.take();
    b.chauffer(2);
    let rgba = b.capturer().unwrap();
    b.app.vignettes.selp = choix;
    b.chauffer(1);
    sauver(&rgba, nom);
    let r = relever(&rgba, de, a);
    println!("{nom} : {r:?}");
    r
}

/// Le trait est là, continu.
pub(crate) fn present(r: &Releve, quoi: &str) {
    assert!(r.trou <= 2, "{quoi} : le trait est continu, sans trou : {r:?}");
}

/// Plus rien du trait ici (pas de doublon) : le relevé ne voit presque que du papier.
pub(crate) fn absent(r: &Releve, quoi: &str) {
    // (Aucun pixel d'encre le long du trait : `largeur` vide. Les filets et une bulle qui passent dessus ne comptent ni pour l'encre ni pour le trou.)
    assert!(r.largeur == (0, 0) || r.trou * 10 >= r.longueur * 8, "{quoi} : plus rien du trait ici : {r:?}");
}

/// La boîte du choix à l'écran : ses quatre coins.
pub(crate) fn boite(b: &Banc) -> [Pos2; 4] {
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    b.app.selp_cadre(&l, xf.zoom).expect("une boîte").corners().map(|q| xf.to_screen(q))
}

/// L'affinité (écran) que donne le geste de `handle` de `de` à `vers` sur la boîte d'avant : ce que l'application doit avoir fait.
fn attendu(b: &Banc, handle: Handle, de: Pos2, vers: Pos2) -> impl Fn(Pos2) -> Pos2 {
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let frame = b.app.selp_cadre(&l, xf.zoom).expect("une boîte");
    let m = Mods::with(false, false, b.app.keep_proportions, b.app.settings.point_fixe, b.app.settings.rotation_autour);
    let aff: Affine = crate::selbox::gesture(handle, &frame, xf.to_board(de), xf.to_board(vers), m);
    move |p: Pos2| xf.to_screen(aff.apply(xf.to_board(p)))
}

#[test]
fn moving_a_stroke_selected_in_one_case_moves_the_whole_stroke() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    present(&voir(&mut b, "selection-0-avant", de, a), "avant le geste");
    let (el0, pl0) = parts(&b);
    println!("parts avant : {el0} éléments, {pl0} sur la planche");
    assert!(el0 >= 3 && pl0 >= 1, "le trait est réparti : trois cases et la planche ({el0}, {pl0})");
    let pas0 = b.app.history.undo_len();
    // Ctrl + lasso autour du trait, dans la case du milieu, puis Ctrl + glisser dans la boîte : 40 pixels à droite.
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 20.0);
    println!("choisies : {} ; {}", choisies(&b), b.app.status);
    assert_eq!(choisies(&b), el0 + pl0, "le lasso dans la case prend le trait entier, toutes ses parts");
    assert_eq!(b.app.history.undo_len(), pas0, "choisir ne fait aucune étape");
    let d = vec2(40.0, 0.0);
    let rejeux = crate::vector::replay_count();
    // (Dans la boîte, loin de son pivot, sur le trait.)
    let prise = m + (a - m) * 0.3;
    glisser_droit(&mut b, prise, d);
    let rejeux = crate::vector::replay_count() - rejeux;
    decrire(&b);
    present(&voir(&mut b, "selection-1-deplace", de + d, a + d), "le trait déplacé");
    absent(&voir(&mut b, "selection-1b-ancienne", de, a), "l'ancienne place");
    assert_eq!(b.app.history.undo_len(), pas0 + 1, "une seule étape d'annulation pour le geste");
    assert_eq!(rejeux, 0, "aucun rejeu du moteur sur le fil de l'interface, du geste au lâcher");
    assert!(choisies(&b) >= 3, "le choix suit le trait à sa nouvelle place ({})", choisies(&b));
    // Annuler, rétablir.
    b.app.undo();
    present(&voir(&mut b, "selection-2-annule", de, a), "annulé : le trait revient");
    absent(&relever(&b.capturer().unwrap(), de + d, a + d), "annulé : plus rien à la place d'après");
    assert_eq!(parts(&b), (el0, pl0), "annuler rend les parts d'avant");
    b.app.redo();
    present(&voir(&mut b, "selection-3-retabli", de + d, a + d), "rétabli");
    // Enregistrer, rouvrir : le trait est là, et ses parts sont toujours liées (le lasso le reprend entier).
    let dossier = std::env::temp_dir().join(format!("shot-test-vignettes-selection-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dossier).ok();
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    std::fs::remove_dir_all(&dossier).ok();
    let (zoom, pan) = (b.app.board_zoom, b.app.board_pan);
    let mut c = planche(relu.doc, 6, 1.0).expect("la planche");
    c.app.board_zoom = zoom;
    c.app.board_pan = pan;
    c.chauffer(4);
    c.attendre_vignettes();
    present(&voir(&mut c, "selection-4-rouvert", de + d, a + d), "rouvert");
    let n = parts(&c).0 + parts(&c).1;
    lasso(&mut c, m + d, 20.0);
    assert_eq!(choisies(&c), n, "rouvert : le lasso reprend le trait entier");
}

#[test]
fn scaling_and_rotating_take_the_whole_stroke() {
    for (nom, geste) in [("echelle", 0), ("rotation", 1)] {
        let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
        let m = sur_le_trait(de, a, t2.center().x);
        lasso(&mut b, m, 20.0);
        let q = boite(&b);
        let pas0 = b.app.history.undo_len();
        let (handle, depart, vers) = if geste == 0 {
            // Le coin haut gauche, tiré vers l'extérieur : la boîte grandit, le coin opposé reste.
            (Handle::Corner(0), q[0], q[0] + vec2(-30.0, -20.0))
        } else {
            // Juste hors du coin haut droit : tourner.
            let c = q[1] + (q[1] - q[3]).normalized() * 10.0;
            (Handle::Rotate(1), c, c + vec2(0.0, 45.0))
        };
        let f = attendu(&b, handle, depart, vers);
        glisser_droit(&mut b, depart, vers - depart);
        let (de2, a2) = (f(de), f(a));
        println!("{nom} : {de:?}->{a:?} devient {de2:?}->{a2:?}");
        present(&voir(&mut b, &format!("selection-{nom}"), de2, a2), nom);
        assert_eq!(b.app.history.undo_len(), pas0 + 1, "{nom} : une seule étape");
    }
}

#[test]
fn copy_cut_paste_and_delete_take_the_whole_stroke() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    let m = sur_le_trait(de, a, t2.center().x);
    let avant = parts(&b);
    lasso(&mut b, m, 20.0);
    let pas0 = b.app.history.undo_len();
    b.app.run_action("edition.copier");
    assert_eq!(b.app.clipboard.len(), 1, "une copie : le trait entier, d'un seul tenant ({})", b.app.status);
    b.app.run_action("edition.supprimer");
    absent(&voir(&mut b, "selection-supprime", de, a), "supprimé : toutes les parts");
    assert_eq!(parts(&b), (0, 0), "plus aucune part, ni dans les cases, ni sur la planche");
    assert_eq!(b.app.history.undo_len(), pas0 + 1, "supprimer : une étape");
    // Coller : le trait revient à sa place, réparti comme un trait qu'on vient de tracer.
    b.app.run_action("edition.coller");
    present(&voir(&mut b, "selection-colle", de, a), "collé");
    assert_eq!(parts(&b), avant, "collé : réparti comme le trait d'origine");
    assert_eq!(b.app.history.undo_len(), pas0 + 2, "coller : une étape");
    // Couper : copié, puis effacé ; annuler le rend.
    lasso(&mut b, m, 20.0);
    b.app.run_action("edition.couper");
    absent(&voir(&mut b, "selection-coupe", de, a), "coupé");
    b.app.undo();
    present(&voir(&mut b, "selection-coupe-annule", de, a), "coupé puis annulé");
    assert_eq!(parts(&b), avant);
}

#[test]
fn a_lasso_on_the_paper_or_across_a_case_edge_takes_the_whole_stroke() {
    for (nom, ou) in [("papier", 0), ("a-cheval", 1)] {
        let Some((mut b, de, a, t1, _)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
        let n = parts(&b).0 + parts(&b).1;
        let p = if ou == 0 {
            // Sur le papier, au-dessus de la case 1, hors de son champ : la part de la planche.
            de + (a - de).normalized() * 6.0
        } else {
            // Sur le bord haut de la case 1 : à cheval sur son image et le papier.
            sur_le_trait(de, a, de.x + (a.x - de.x) * ((t1.top() - de.y) / (a.y - de.y)))
        };
        lasso(&mut b, p, 9.0);
        assert_eq!(choisies(&b), n, "{nom} : le lasso prend le trait entier ({})", b.app.status);
        let d = vec2(0.0, 30.0);
        // (La boîte : on la prend en son centre.)
        let q = boite(&b);
        let centre = pos2(q[0].x * 0.7 + q[2].x * 0.3, q[0].y * 0.7 + q[2].y * 0.3);
        let pas0 = b.app.history.undo_len();
        glisser_droit(&mut b, centre, d);
        present(&voir(&mut b, &format!("selection-{nom}"), de + d, a + d), nom);
        absent(&voir(&mut b, "ancienne-place", de, a), &format!("{nom} : l'ancienne place"));
        assert_eq!(b.app.history.undo_len(), pas0 + 1);
    }
}

#[test]
fn a_stroke_leaving_its_case_is_shared_again_and_a_click_takes_it() {
    // Un trait tout entier dans la case 2, déplacé à moitié dans la case 3 : il est réparti comme un trait neuf (sa part dans la 3 va à son plan).
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (t2, t3) = (xf.rect(l.cards[2].thumb), xf.rect(l.cards[3].thumb));
    let (de, a) = (t2.center() + vec2(-30.0, -15.0), t2.center() + vec2(30.0, 15.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(parts(&b), (1, 0), "un seul trait, dans la case 2");
    // Ctrl + clic sur le trait : il est pris.
    crate::vignettes_tests::clic_avec(&mut b, t2.center(), Modifiers::CTRL);
    b.touches = Modifiers::NONE;
    assert_eq!(choisies(&b), 1, "un clic prend le trait ({})", b.app.status);
    let d = vec2(t3.center().x - t2.center().x, 0.0);
    let q = boite(&b);
    glisser_droit(&mut b, pos2(q[0].x * 0.7 + q[2].x * 0.3, q[0].y * 0.7 + q[2].y * 0.3), d);
    decrire(&b);
    present(&voir(&mut b, "selection-vers-la-case-3", de + d, a + d), "déplacé dans la case 3");
    let plan3 = b.app.doc.panels[l.cards[3].panel].id;
    assert!(crate::vignettes_tests::encre(&b.app.doc, l.cards[3].panel), "la case 3 a reçu le trait (plan {plan3})");
    assert!(!crate::vignettes_tests::encre(&b.app.doc, l.cards[2].panel), "la case 2 n'a plus rien");
}

#[test]
fn strokes_cut_before_the_link_stay_as_they_are() {
    // Un projet d'avant la partie 125 : les parts n'ont pas de numéro. Le lasso ne prend que ce qu'il touche, comme avant ; rien d'autre ne bouge.
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    for c in b.app.doc.cels.values_mut() {
        if let Some(v) = c.vector.as_mut() {
            for e in &mut v.elements {
                if let crate::vector::Element::Stroke { trace, .. } = e {
                    trace.lien = None;
                }
            }
        }
    }
    for t in &mut b.app.doc.annotations.traces {
        t.lien = None;
    }
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 20.0);
    assert_eq!(choisies(&b), 1, "sans numéro : seule la part touchée");
    let avant = parts(&b);
    b.app.run_action("edition.supprimer");
    assert_eq!(parts(&b), (avant.0 - 1, avant.1), "seule cette part part ; les autres restent telles quelles");
}

#[test]
fn escape_or_a_still_click_changes_nothing() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(false) else { return println!("pas de carte graphique") };
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 20.0);
    let pas0 = b.app.history.undo_len();
    let avant = parts(&b);
    // Un appui dans la boîte sans bouger : rien.
    glisser_avec(&mut b, &[m, m], Modifiers::CTRL);
    assert_eq!((b.app.history.undo_len(), parts(&b)), (pas0, avant));
    // Un trait au pinceau, sans Ctrl : le choix est levé, le trait se dessine.
    tracer(&mut b, t2.center() + vec2(-20.0, 15.0), t2.center() + vec2(20.0, 15.0));
    assert!(!b.app.selp_active(), "dessiner lève le choix");
    present(&voir(&mut b, "selection-rien", de, a), "le trait d'avant n'a pas bougé");
}
