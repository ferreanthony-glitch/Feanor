//! L'outil Trajectoire dans l'application (`trajectoireui.rs`, Anthony, 2026-11-07) : tracer, marquer, glisser, arracher,
//! déplacer, annuler, recaler sur la timeline, clés d'images, détacher, enregistrer.

use eframe::egui::{pos2, Event, Key, Modifiers, PointerButton, Pos2};

use crate::app::{Mode, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::trajectoire::{Ancre, Cles};

const DT: f64 = 1.0 / 60.0;

/// Un plan 400 × 300 de 24 images, un calque vectoriel, l'outil Trajectoire en main, à l'écran (1280 × 720). Les essais des clés
/// de position demandent les clés d'interpolation (d'origine, ce sont les clés d'images).
pub(crate) fn banc() -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Vector)];
    doc.panels[0].length = 24;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.set_tool(Tool::Trajectoire);
    assert_eq!(b.app.traj.cles_propose, Cles::Images, "d'origine, les clés d'images");
    b.app.traj.cles_propose = Cles::Interpolation;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

/// Un trait de pinceau en L, de (60, 130) à (60, 170) puis (100, 170), à l'image 0 : le dessin du départ, dont la boîte fait 40 × 40
/// autour de (80, 150). Rend son numéro.
pub(crate) fn dessiner_le_depart(b: &mut Banc) -> u64 {
    b.app.set_tool(Tool::Brush);
    b.app.color = [0, 0, 0];
    b.app.brush.size = 6.0;
    glisser(b, &[(60.0, 130.0), (60.0, 150.0), (60.0, 170.0), (80.0, 170.0), (100.0, 170.0)]);
    b.app.set_tool(Tool::Trajectoire);
    b.app.layers()[0].cel_at(0).expect("un dessin à l'image 0")
}

fn centre_du_dessin(b: &Banc, cel: u64) -> Pos2 {
    b.app.doc.cels[&cel].vector.as_ref().unwrap().elements[0].bounds().center()
}

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn bouton(pos: Pos2, pressed: bool) -> Event {
    bouton_avec(pos, pressed, Modifiers::NONE)
}

fn bouton_avec(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers }
}

/// Un glisser sur les points `pts` de la feuille, une image d'écran par point.
fn glisser(b: &mut Banc, pts: &[(f32, f32)]) {
    glisser_avec(b, pts, Modifiers::NONE);
}

/// Comme `glisser`, ces touches tenues pendant tout le geste.
fn glisser_avec(b: &mut Banc, pts: &[(f32, f32)], touches: Modifiers) {
    b.touches = touches;
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], DT);
    b.image(vec![bouton_avec(p0, true, touches)], DT);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], DT);
    }
    let (x, y) = pts[pts.len() - 1];
    let fin = ecran(b, x, y);
    b.image(vec![bouton_avec(fin, false, touches)], DT);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), DT);
}

/// Un chemin droit de (50, 150) à (350, 150), en six points.
pub(crate) fn tracer_le_chemin(b: &mut Banc) {
    glisser(b, &[(50.0, 150.0), (110.0, 150.0), (170.0, 150.0), (230.0, 150.0), (290.0, 150.0), (350.0, 150.0)]);
}

/// Une marque au milieu du chemin.
pub(crate) fn marquer_le_milieu(b: &mut Banc) {
    glisser(b, &[(200.0, 135.0), (200.0, 150.0), (200.0, 165.0)]);
}

fn derniere_etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

fn cles(b: &Banc) -> Vec<(usize, f32, f32)> {
    b.app.layers()[0].pos_keys.iter().map(|k| (k.frame, k.t.x, k.t.y)).collect()
}

fn jalons(b: &Banc) -> Vec<usize> {
    b.app.layers()[0].trajectoire.as_ref().map(|t| t.images_des_jalons(b.app.frame_count())).unwrap_or_default()
}

#[test]
fn a_stroke_draws_the_path_and_lays_one_key_per_image_relative_to_the_start() {
    let mut b = banc();
    assert!(!b.app.modified, "prendre l'outil ne change rien au projet");
    tracer_le_chemin(&mut b);
    let t = b.app.layers()[0].trajectoire.clone().expect("un chemin");
    assert!((t.longueur() - 300.0).abs() < 2.0, "{}", t.longueur());
    assert!(t.marques.is_empty());
    let c = cles(&b);
    assert_eq!(c.len(), 24, "une clé par image du plan");
    assert_eq!(c[0], (0, 0.0, 0.0), "le calque part de là où il est");
    assert!((c[23].1 - 300.0).abs() < 2.0 && c[23].2.abs() < 1.0, "à la dernière image, au bout du chemin : {:?}", c[23]);
    assert!((c[12].1 - 300.0 * 12.0 / 23.0).abs() < 2.0, "à vitesse constante : {:?}", c[12]);
    assert_eq!(derniere_etape(&b), "Trajectoire");
    assert!(b.app.modified);
    // Une seule étape : Ctrl+Z retire le chemin et ses clés ; rétablir les rend.
    b.app.undo();
    assert!(b.app.layers()[0].trajectoire.is_none());
    assert!(b.app.layers()[0].pos_keys.is_empty());
    b.app.redo();
    assert!(b.app.layers()[0].trajectoire.is_some());
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
    // Un simple clic ne fait rien.
    let avant = b.app.history.names().0.len();
    glisser(&mut b, &[(200.0, 250.0)]);
    assert_eq!(b.app.history.names().0.len(), avant);
}

#[test]
fn a_short_stroke_across_the_path_is_a_mark_and_the_keys_follow_its_spacing() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    let t = b.app.layers()[0].trajectoire.clone().expect("un chemin");
    assert_eq!(t.marques.len(), 1, "une marque");
    assert!((t.marques[0] - 0.5).abs() < 0.02, "au milieu : {}", t.marques[0]);
    assert_eq!(derniere_etape(&b), "Marque de trajectoire");
    // Une seconde marque près du départ : deux intervalles courts d'abord, un long ensuite ⇒ lent puis vite.
    glisser(&mut b, &[(80.0, 138.0), (80.0, 162.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert_eq!(t.marques.len(), 2);
    assert!(t.marques[0] < t.marques[1]);
    let c = cles(&b);
    // Trois intervalles de temps égaux : à un tiers du temps, un dixième du chemin ; à deux tiers, la moitié.
    let x_a = |f: usize| c.iter().find(|k| k.0 == f).map(|k| k.1).unwrap();
    assert!((x_a(23) - 300.0).abs() < 2.0);
    assert!(x_a(8) < 45.0, "lent au départ : {}", x_a(8));
    assert!((x_a(16) - 150.0).abs() < 15.0, "la moitié aux deux tiers du temps : {}", x_a(16));
    // Les silhouettes aux jalons : quatre (départ, deux marques, arrivée), moins celle de l'image courante.
    b.app.layers_mut()[0].frames = vec![Some(b.app.doc.cels.keys().next().copied().unwrap_or(1)); 24];
    let plan = b.app.build_plan_avec(0, true, true, true);
    let fantomes = plan.items.iter().filter(|i| i.tint == Some(crate::trajectoireui::FANTOME)).count();
    assert_eq!(fantomes, 3, "une silhouette par jalon, sauf l'image courante");
    b.app.set_tool(Tool::Brush);
    let plan = b.app.build_plan_avec(0, true, true, true);
    assert_eq!(plan.items.iter().filter(|i| i.tint == Some(crate::trajectoireui::FANTOME)).count(), 0, "sans l'outil, pas de silhouette");
}

#[test]
fn a_mark_slides_along_the_path_and_is_torn_off_when_pulled_away() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    glisser(&mut b, &[(200.0, 135.0), (200.0, 165.0)]);
    // Prendre la marque (à 200, 150) et la glisser jusqu'à 260.
    glisser(&mut b, &[(200.0, 150.0), (230.0, 151.0), (260.0, 150.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert_eq!(t.marques.len(), 1);
    assert!((t.marques[0] - 0.7).abs() < 0.02, "glissée à 70 % : {}", t.marques[0]);
    assert_eq!(derniere_etape(&b), "Glisser une marque");
    // Les clés ont suivi pendant le geste : à l'image du milieu du temps, le calque est bien avant la marque.
    let c = cles(&b);
    assert!(c.iter().find(|k| k.0 == 23).is_some_and(|k| (k.1 - 300.0).abs() < 2.0));
    // La tirer loin du chemin : elle est arrachée.
    glisser(&mut b, &[(260.0, 150.0), (260.0, 200.0), (260.0, 270.0)]);
    assert!(b.app.layers()[0].trajectoire.as_ref().unwrap().marques.is_empty(), "marque retirée");
    assert_eq!(derniere_etape(&b), "Retirer une marque");
    b.app.undo();
    assert_eq!(b.app.layers()[0].trajectoire.as_ref().unwrap().marques.len(), 1, "Ctrl+Z la rend");
}

#[test]
fn pulling_the_path_bends_it_shift_moves_it_as_a_whole_and_escape_gives_the_gesture_up() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    // Tirer le chemin (loin de toute marque) vers le bas, de 50 : il se tord là, les bouts restent.
    glisser(&mut b, &[(150.0, 150.0), (150.0, 175.0), (150.0, 200.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    let pris = t.plus_proche([150.0, 200.0]);
    assert!(pris.1 < 3.0, "le point pris a suivi le pointeur : {pris:?}");
    assert!((t.point_a(1.0)[0] - 350.0).abs() < 1.0 && (t.point_a(1.0)[1] - 150.0).abs() < 1.0, "l'arrivée n'a pas bougé : {:?}", t.point_a(1.0));
    assert!((t.point_a(0.0)[1] - 150.0).abs() < 12.0, "le départ, à un tiers du chemin, à peine : {:?}", t.point_a(0.0));
    assert_eq!(derniere_etape(&b), "Tirer la trajectoire");
    // Les clés ont suivi la nouvelle forme : au milieu du temps, le calque est descendu.
    let c = cles(&b);
    assert!(c.iter().find(|k| k.0 == 8).is_some_and(|k| k.2 > 20.0), "{:?}", c.iter().find(|k| k.0 == 8));
    // Maj tenue : le chemin se déplace en bloc.
    glisser_avec(&mut b, &[(300.0, 150.0), (300.0, 175.0), (300.0, 200.0)], Modifiers::SHIFT);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!((t.point_a(1.0)[1] - 200.0).abs() < 1.0 && (t.point_a(1.0)[0] - 350.0).abs() < 1.0, "descendu de 50 en bloc : {:?}", t.point_a(1.0));
    assert_eq!(derniere_etape(&b), "Déplacer la trajectoire");
    // Le déplacement en bloc est relatif : la clé du départ ne change pas.
    assert_eq!(cles(&b)[0], (0, 0.0, 0.0));
    // Échap au milieu d'un glisser : tout revient, aucune étape.
    let etapes = b.app.history.names().0.len();
    let avant = t.clone();
    let p0 = ecran(&b, 350.0, 200.0);
    b.image(vec![Event::PointerMoved(p0)], DT);
    b.image(vec![bouton(p0, true)], DT);
    let p1 = ecran(&b, 350.0, 260.0);
    b.image(vec![Event::PointerMoved(p1)], DT);
    assert!((b.app.layers()[0].trajectoire.as_ref().unwrap().point_a(1.0)[1] - 260.0).abs() < 1.0, "il suit pendant le geste");
    assert!(b.app.trajectoire_touche(Key::Escape));
    assert_eq!(b.app.layers()[0].trajectoire.as_ref(), Some(&avant), "Échap : comme avant");
    b.image(vec![bouton(p1, false)], DT);
    b.image(Vec::new(), DT);
    assert_eq!(b.app.history.names().0.len(), etapes, "aucune étape");
}

#[test]
fn a_long_stroke_retraces_the_path_and_keeps_the_marks() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    glisser(&mut b, &[(200.0, 135.0), (200.0, 165.0)]);
    glisser(&mut b, &[(50.0, 50.0), (50.0, 100.0), (50.0, 150.0), (50.0, 200.0), (50.0, 250.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!((t.longueur() - 200.0).abs() < 2.0, "le nouveau chemin : {}", t.longueur());
    assert_eq!(t.marques.len(), 1, "la marque reste, à sa fraction");
    assert_eq!(derniere_etape(&b), "Retracer la trajectoire");
    let c = cles(&b);
    assert!((c[23].2 - 200.0).abs() < 2.0 && c[23].1.abs() < 1.0, "vers le bas maintenant : {:?}", c[23]);
}

#[test]
fn dragging_a_key_on_the_timeline_retimes_the_trajectory_instead_of_detaching_it() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    assert_eq!(jalons(&b), [0, 12, 23]);
    // Seuls les jalons se prennent : la clé de l'image 5 (une clé par image) n'est pas visible.
    assert_eq!(crate::poskeysui::pos_frames_visibles(&b.app.layers()[0], 24), [0, 12, 23]);
    // La dernière clé, glissée de 23 à 17 : la fin recule, la marque suit au milieu du temps, la trajectoire reste.
    let etapes = b.app.history.names().0.len();
    assert!(b.app.begin_pos_drag(0, 23, 23));
    b.app.drag_pos_keys(17);
    assert_eq!(jalons(&b), [0, 9, 17], "pendant le geste déjà");
    b.app.end_pos_drag();
    let t = b.app.layers()[0].trajectoire.clone().expect("la trajectoire reste");
    assert_eq!((t.fin, jalons(&b)), (Some(17), vec![0, 9, 17]));
    let c = cles(&b);
    assert_eq!(c.last().map(|k| k.0), Some(17));
    assert!((c.last().unwrap().1 - 300.0).abs() < 2.0, "au bout du chemin à la fin");
    assert_eq!(derniere_etape(&b), "Recaler la trajectoire");
    assert_eq!(b.app.history.names().0.len(), etapes + 1);
    // La marque, glissée de 9 à 5 : son temps devient explicite ; le départ et la fin ne bougent pas.
    assert!(b.app.begin_pos_drag(0, 9, 9));
    b.app.drag_pos_keys(5);
    b.app.end_pos_drag();
    assert_eq!(jalons(&b), [0, 5, 17]);
    assert!(b.app.layers()[0].trajectoire.as_ref().unwrap().temps.is_some());
    // La première clé, glissée de 0 à 3 : le départ avance.
    assert!(b.app.begin_pos_drag(0, 0, 0));
    b.app.drag_pos_keys(3);
    b.app.end_pos_drag();
    assert_eq!(jalons(&b)[0], 3);
    assert_eq!(cles(&b).first().map(|k| k.0), Some(3));
    // Échap au milieu d'un glisser : tout revient.
    assert!(b.app.begin_pos_drag(0, 17, 17));
    b.app.drag_pos_keys(12);
    assert_eq!(jalons(&b).last(), Some(&12));
    b.app.cancel_pos_drag();
    assert_eq!(jalons(&b).last(), Some(&17));
    // Trois étapes : Ctrl+Z trois fois rend les jalons d'origine, la trajectoire toujours là.
    b.app.undo();
    b.app.undo();
    b.app.undo();
    assert_eq!(jalons(&b), [0, 12, 23]);
    assert!(b.app.layers()[0].trajectoire.is_some());
    // Les boutons des réglages : finir et partir à la tête de lecture.
    b.app.cur_frame = 20;
    b.app.trajectoire_fin_ici();
    assert_eq!((jalons(&b), derniere_etape(&b)), (vec![0, 10, 20], "Fin de la trajectoire".to_string()));
    b.app.cur_frame = 4;
    b.app.trajectoire_depart_ici();
    assert_eq!((jalons(&b), derniere_etape(&b)), (vec![4, 12, 20], "Départ de la trajectoire".to_string()));
}

#[test]
fn image_keys_put_a_drawing_at_each_mark_that_splits_the_timeline_and_moves_with_the_path() {
    let mut b = banc();
    // D'origine, les clés d'images : avec un dessin au départ, elles se posent dès le tracé.
    b.app.traj.cles_propose = Cles::Images;
    let source = dessiner_le_depart(&mut b);
    let c0 = centre_du_dessin(&b, source);
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    let l = &b.app.layers()[0];
    assert!(l.pos_keys.is_empty(), "pas de clé de position : les dessins font le mouvement");
    let t = l.trajectoire.clone().unwrap();
    assert_eq!(t.cles, Cles::Images);
    assert_eq!(t.dessins.len(), 3, "un dessin par jalon : le départ (le dessin d'origine), la marque, l'arrivée");
    assert_eq!(t.source, Some(source));
    assert_eq!(jalons(&b), [0, 12, 23]);
    assert_eq!(t.dessins[0].dessin, source, "au départ, le dessin d'origine lui-même");
    assert_eq!(l.cel_at(0), Some(source));
    assert_eq!(l.cel_at(11), Some(source), "le départ tenu jusqu'à la marque");
    let (milieu, bout) = (t.dessins[1].dessin, t.dessins[2].dessin);
    assert_eq!((l.cel_at(12), l.cel_at(22), l.cel_at(23)), (Some(milieu), Some(milieu), Some(bout)), "la timeline se scinde aux jalons");
    assert_ne!(milieu, source);
    assert_ne!(bout, milieu);
    // Les copies sont déplacées sur le chemin : à la marque, 150 px plus loin ; au bout, 300.
    let (cm, cb) = (centre_du_dessin(&b, milieu), centre_du_dessin(&b, bout));
    assert!((cm.x - c0.x - 150.0).abs() < 4.0 && (cm.y - c0.y).abs() < 1.0, "{cm:?} contre {c0:?}");
    assert!((cb.x - c0.x - 300.0).abs() < 4.0, "{cb:?} contre {c0:?}");
    // Glisser la marque le long du chemin : le dessin de la marque est déplacé (pas refait), ce qu'on y a dessiné suit.
    glisser(&mut b, &[(200.0, 150.0), (230.0, 151.0), (260.0, 150.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!((t.marques[0] - 0.7).abs() < 0.02);
    let milieu2 = t.dessins[1].dessin;
    let cm2 = centre_du_dessin(&b, milieu2);
    assert!((cm2.x - c0.x - 210.0).abs() < 4.0, "à 70 % du chemin : {cm2:?}");
    assert_eq!(b.app.layers()[0].cel_at(12), Some(milieu2));
    // La clé de dessin de l'arrivée, glissée sur la timeline (montage des clés) de 23 à 18 : la fin recule.
    let (plan, calque) = (b.app.doc.panels[0].id, b.app.layers()[0].id);
    b.app.key_sel = vec![crate::montagekeys::KeyRef { panel: plan, layer: calque, start: 23 }];
    assert!(b.app.begin_keys_drag(23));
    b.app.drag_keys_mode(18, crate::handles::KeyMove::Single);
    b.app.end_keys_drag();
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    // (La marque est à 70 % du chemin, mais au milieu du temps : à l'image 9 sur 18.)
    assert_eq!((t.fin, jalons(&b)), (Some(18), vec![0, 9, 18]));
    assert_eq!(b.app.layers()[0].cel_at(18), Some(t.dessins[2].dessin));
    assert_eq!(b.app.layers()[0].cel_at(9), Some(t.dessins[1].dessin));
    assert_eq!(b.app.layers()[0].cel_at(8), Some(source));
    // Retour aux clés d'interpolation : les dessins posés s'en vont, le dessin du départ tient, les clés reviennent.
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().cles = Cles::Interpolation;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Sorte des clés de la trajectoire");
    let l = &b.app.layers()[0];
    assert!(l.trajectoire.as_ref().unwrap().dessins.is_empty());
    assert!((0..19).all(|f| l.cel_at(f) == Some(source)), "le départ tenu partout");
    assert_eq!(l.pos_keys.len(), 19);
    // Sans dessin au départ, les clés d'images ne peuvent rien poser : la ligne d'état le dit.
    b.app.layers_mut()[0].frames = vec![None; 24];
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().source = None;
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().cles = Cles::Images;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Sorte des clés de la trajectoire");
    assert!(b.app.status.contains("dessine d'abord"), "{}", b.app.status);
}

#[test]
fn the_anchor_puts_the_bottom_the_centre_or_a_chosen_point_of_the_drawing_on_the_path() {
    let mut b = banc();
    let source = dessiner_le_depart(&mut b);
    let boite = b.app.doc.cels[&source].vector.as_ref().unwrap().elements[0].bounds();
    tracer_le_chemin(&mut b);
    // D'origine, là où part le trait : le calque ne bouge pas au départ.
    assert_eq!(cles(&b)[0], (0, 0.0, 0.0));
    // Le bas du dessin : au départ, le milieu du bord bas du trait est sur le départ du chemin (50, 150).
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().ancre = Ancre::Bas;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Ancrage de la trajectoire");
    let c = cles(&b);
    let (bas_x, bas_y) = (boite.center().x, boite.max.y);
    assert!((c[0].1 - (50.0 - bas_x)).abs() < 1.5 && (c[0].2 - (150.0 - bas_y)).abs() < 1.5, "le bas sur le départ : {:?} pour un bas à ({bas_x}, {bas_y})", c[0]);
    assert!((c[23].1 - (350.0 - bas_x)).abs() < 1.5, "et au bout : {:?}", c[23]);
    assert_eq!(b.app.layers()[0].trajectoire.as_ref().unwrap().decalage_depart, [c[0].1, c[0].2]);
    // Recalculer encore (glisser une marque) ne fait pas dériver la pose propre du calque.
    marquer_le_milieu(&mut b);
    let c2 = cles(&b);
    assert!((c2[0].1 - c[0].1).abs() < 1e-3 && (c2[0].2 - c[0].2).abs() < 1e-3, "même départ après un second calcul : {:?}", c2[0]);
    // Le centre.
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().ancre = Ancre::Centre;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Ancrage de la trajectoire");
    let c = cles(&b);
    assert!((c[0].1 - (50.0 - boite.center().x)).abs() < 1.5 && (c[0].2 - (150.0 - boite.center().y)).abs() < 1.5, "{:?}", c[0]);
    // Un point cliqué, après le bouton des réglages : le dessin est affiché déplacé (son centre, à 80, est sur le départ du chemin, à
    // 50 : 30 px plus à gauche) ; cliquer la feuille en (60, 150), c'est cliquer le point (90, 150) du dessin, son bout droit.
    b.app.traj.choisir_ancre = true;
    glisser(&mut b, &[(60.0, 150.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!(matches!(t.ancre, Ancre::Point { x, y } if (x - 90.0).abs() < 2.0 && (y - 150.0).abs() < 2.0), "{:?}", t.ancre);
    assert!(!b.app.traj.choisir_ancre);
    assert_eq!(derniere_etape(&b), "Point d'ancrage de la trajectoire");
    let c = cles(&b);
    assert!((c[0].1 - (50.0 - 90.0)).abs() < 2.5 && c[0].2.abs() < 2.5, "le point cliqué sur le départ : {:?}", c[0]);
    // Retour à « là où part le trait » : plus de déplacement au départ.
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().ancre = Ancre::Depart;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Ancrage de la trajectoire");
    assert_eq!(cles(&b)[0], (0, 0.0, 0.0));
    // En clés d'images avec le bas du dessin : le départ aussi reçoit une copie déplacée, et l'arrivée.
    let snap = b.app.snapshot();
    {
        let t = b.app.layers_mut()[0].trajectoire.as_mut().unwrap();
        t.ancre = Ancre::Bas;
        t.cles = Cles::Images;
    }
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Sorte des clés de la trajectoire");
    let l = &b.app.layers()[0];
    let t = l.trajectoire.clone().unwrap();
    assert_eq!(t.dessins.len(), 3);
    assert_ne!(t.dessins[0].dessin, source, "le départ est une copie déplacée");
    assert_eq!(l.cel_at(0), Some(t.dessins[0].dessin));
    let c0 = centre_du_dessin(&b, t.dessins[0].dessin);
    assert!((c0.x - (50.0 - (bas_x - boite.center().x))).abs() < 4.0 && (c0.y - (150.0 - (bas_y - boite.center().y))).abs() < 4.0, "{c0:?}");
    // Ctrl+Z : le dessin d'origine revient au départ.
    b.app.undo();
    assert_eq!(b.app.layers()[0].cel_at(0), Some(source));
}

#[test]
fn without_a_drawing_image_keys_say_to_draw_first_and_nothing_moves() {
    let mut b = banc();
    b.app.traj.cles_propose = Cles::Images;
    tracer_le_chemin(&mut b);
    let l = &b.app.layers()[0];
    assert!(l.trajectoire.is_some(), "le chemin est là");
    assert!(l.pos_keys.is_empty() && l.trajectoire.as_ref().unwrap().dessins.is_empty());
    assert!(b.app.status.contains("dessine d'abord"), "{}", b.app.status);
}

#[test]
fn the_path_starts_at_the_playhead() {
    let mut b = banc();
    b.app.cur_frame = 6;
    b.chauffer(2);
    tracer_le_chemin(&mut b);
    let t = b.app.layers()[0].trajectoire.clone().expect("un chemin");
    assert_eq!(t.depart, 6, "la clé de départ démarre sur la tête de lecture");
    let c = cles(&b);
    assert_eq!(c.first().map(|k| k.0), Some(6), "aucune clé avant la tête de lecture");
    assert_eq!(c.last().map(|k| k.0), Some(23));
    assert_eq!(c.len(), 18);
    assert!((c.last().unwrap().1 - 300.0).abs() < 2.0);
    // Enregistré, relu : le départ revient.
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire-depart-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.shot");
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).unwrap();
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].trajectoire.as_ref().map(|t| t.depart), Some(6));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_trajectory_never_goes_away_by_itself_keys_by_hand_are_refused_a_mark_is_deleted_and_transform_moves_both() {
    let mut b = banc();
    let source = dessiner_le_depart(&mut b);
    let c0 = centre_du_dessin(&b, source);
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    let etapes = b.app.history.names().0.len();
    // Poser une clé à la main : refusé, la trajectoire reste, la ligne d'état dit pourquoi.
    b.app.cur_frame = 5;
    b.app.pos_key_here();
    assert!(b.app.layers()[0].trajectoire.is_some());
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
    assert!(b.app.status.contains("trajectoire"), "{}", b.app.status);
    assert_eq!(b.app.history.names().0.len(), etapes, "rien d'enregistré");
    // Lisser une clé : refusé aussi.
    b.app.pos_key_smooth();
    assert!(b.app.layers()[0].pos_keys.iter().all(|k| !k.is_eased()));
    // Suppr sur la marque (image 12) : la marque s'en va, le chemin reste.
    assert_eq!(jalons(&b), [0, 12, 23]);
    b.app.cur_frame = 12;
    b.app.pos_keys_delete();
    assert!(b.app.layers()[0].trajectoire.is_some());
    assert_eq!(jalons(&b), [0, 23]);
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24, "les clés sont reposées, une par image");
    // Suppr sur le départ : refusé, la ligne d'état dit comment enlever la trajectoire.
    b.app.cur_frame = 0;
    b.app.pos_keys_delete();
    assert_eq!(jalons(&b), [0, 23]);
    assert!(b.app.status.contains("Effacer"), "{}", b.app.status);
    // Transformer : le calque et son chemin bougent ensemble (de 40 px vers la droite, pris au milieu du trait, à l'image 0).
    b.app.cur_frame = 0;
    b.app.set_tool(Tool::Transform);
    b.chauffer(3);
    glisser(&mut b, &[(80.0, 150.0), (100.0, 150.0), (120.0, 150.0)]);
    let t = b.app.layers()[0].trajectoire.clone().expect("la trajectoire reste");
    assert!((t.point_a(0.0)[0] - 90.0).abs() < 1.5, "le chemin a suivi : {:?}", t.point_a(0.0));
    let c = cles(&b);
    assert!((c[0].1 - 40.0).abs() < 1.5 && c[0].2.abs() < 1.0, "le calque a bougé de 40 : {:?}", c[0]);
    assert!((c[23].1 - 340.0).abs() < 2.0, "et au bout : {:?}", c[23]);
    assert!(b.app.status.contains("ensemble"), "{}", b.app.status);
    // Une seule étape : Ctrl+Z rend le calque et le chemin.
    b.app.undo();
    assert!((b.app.layers()[0].trajectoire.as_ref().unwrap().point_a(0.0)[0] - 50.0).abs() < 1.5);
    assert_eq!(cles(&b)[0], (0, 0.0, 0.0));
    // Transformer à une autre image que le départ, en clés d'interpolation : même résultat (le déplacement s'applique à la pose propre).
    b.app.cur_frame = 10;
    b.chauffer(2);
    let x10 = b.app.layers()[0].pose(10).x;
    glisser(&mut b, &[(80.0 + x10, 150.0), (80.0 + x10, 180.0), (80.0 + x10, 210.0)]);
    let c = cles(&b);
    assert!((c[0].2 - 60.0).abs() < 1.5 && c[0].1.abs() < 1.0, "descendu de 60 : {:?}", c[0]);
    assert!((b.app.layers()[0].trajectoire.as_ref().unwrap().point_a(0.0)[1] - 210.0).abs() < 1.5);
    // Le calque d'origine reste bien à sa place : le centre du dessin n'a pas changé (ce sont les clés qui bougent).
    assert_eq!(centre_du_dessin(&b, source), c0);
    // « Supprimer toutes les clés » : explicite, la trajectoire s'en va avec.
    b.app.pos_keys_clear();
    assert!(b.app.layers()[0].trajectoire.is_none() && b.app.layers()[0].pos_keys.is_empty());
}

#[test]
fn the_path_is_saved_with_the_layer_and_comes_back_and_cutting_the_plan_detaches_it() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    glisser(&mut b, &[(200.0, 135.0), (200.0, 165.0)]);
    let avant = b.app.layers()[0].trajectoire.clone().unwrap();
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.shot");
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"trajectoire\"") && texte.contains("\"marques\""), "le chemin est écrit");
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].trajectoire.as_ref(), Some(&avant), "il revient tel quel");
    assert_eq!(back.panels[0].layers[0].pos_keys.len(), 24);
    // Sans chemin, rien n'est écrit.
    b.app.layers_mut()[0].trajectoire = None;
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).unwrap();
    assert!(!std::fs::read_to_string(&manifest).unwrap().contains("\"trajectoire\""));
    std::fs::remove_dir_all(&dir).ok();
    // Couper le plan : chaque moitié garde ses clés, le chemin ne commande plus ni l'une ni l'autre.
    b.app.layers_mut()[0].trajectoire = Some(avant);
    let doc = &mut b.app.doc;
    doc.split_panel(0, 12).expect("coupé");
    assert!(doc.panels[0].layers[0].trajectoire.is_none() && doc.panels[1].layers[0].trajectoire.is_none());
    assert!(!doc.panels[0].layers[0].pos_keys.is_empty() && !doc.panels[1].layers[0].pos_keys.is_empty());
}

/// Un trait de pinceau horizontal, de (60, 150) à (120, 150), à l'image 0 : un dessin plus large que haut, dont on voit la rotation.
fn dessiner_un_trait_couche(b: &mut Banc) -> u64 {
    b.app.set_tool(Tool::Brush);
    b.app.color = [0, 0, 0];
    b.app.brush.size = 6.0;
    glisser(b, &[(60.0, 150.0), (80.0, 150.0), (100.0, 150.0), (120.0, 150.0)]);
    b.app.set_tool(Tool::Trajectoire);
    b.app.layers()[0].cel_at(0).expect("un dessin à l'image 0")
}

/// Un chemin en équerre : vers la droite de (50, 150) à (150, 150), puis vers le bas jusqu'à (150, 220).
fn tracer_une_equerre(b: &mut Banc) {
    glisser(b, &[(50.0, 150.0), (75.0, 150.0), (100.0, 150.0), (125.0, 150.0), (150.0, 150.0), (150.0, 175.0), (150.0, 200.0), (150.0, 220.0)]);
}

#[test]
fn inserting_or_removing_images_moves_the_trajectory_with_the_keys_and_the_drawings() {
    // Clés d'interpolation : la fin est posée dès le tracé (dernière image du plan), chaque jalon reste sur son image.
    let mut b = banc();
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    assert_eq!(b.app.layers()[0].trajectoire.as_ref().unwrap().fin, Some(23), "la fin est posée sur la dernière image du plan");
    assert_eq!(jalons(&b), [0, 12, 23]);
    // Six images insérées avant l'image 15, sur tout le plan : le plan dure 30, la marque reste à 12, la fin passe à 29.
    b.app.cur_frame = 15;
    b.app.frame_sel = None;
    b.app.insert_images(true, 6, true);
    assert_eq!(b.app.frame_count(), 30);
    assert_eq!(jalons(&b), [0, 12, 29], "la marque reste à son image, la fin suit");
    let c = cles(&b);
    assert_eq!(c.last().map(|k| k.0), Some(29));
    assert!((c.last().unwrap().1 - 300.0).abs() < 2.0, "au bout du chemin à la fin");
    assert!(b.app.layers()[0].trajectoire.is_some(), "la trajectoire reste");
    // Recalculer d'après la trajectoire ne bouge rien.
    b.app.trajectoire_recalculer(0);
    assert_eq!(jalons(&b), [0, 12, 29]);
    assert_eq!(cles(&b).len(), 30);
    // Quatre images retirées avant le départ ne se peut pas (il est à 0) ; quatre retirées entre la marque et la fin (20 à 24), la
    // suite reculant : la fin passe à 25, la marque reste.
    b.app.hole = crate::images::Hole::FromRight;
    b.app.frame_sel = Some((20, 24));
    b.app.delete_images(true);
    assert_eq!(b.app.frame_count(), 26);
    assert_eq!(jalons(&b), [0, 12, 25]);
    // Ctrl+Z deux fois : tout revient.
    b.app.undo();
    b.app.undo();
    assert_eq!((b.app.frame_count(), jalons(&b)), (24, vec![0, 12, 23]));
    // Clés d'images : les dessins posés suivent avec leurs images, le modèle dit la même chose qu'eux.
    let mut b = banc();
    b.app.traj.cles_propose = Cles::Images;
    let source = dessiner_le_depart(&mut b);
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    let (milieu, bout) = (t.dessins[1].dessin, t.dessins[2].dessin);
    assert_eq!(jalons(&b), [0, 12, 23]);
    b.app.cur_frame = 6;
    b.app.frame_sel = None;
    b.app.insert_images(true, 6, true);
    assert_eq!(jalons(&b), [0, 18, 29], "la marque et la fin ont suivi les images");
    let l = &b.app.layers()[0];
    assert_eq!((l.cel_at(0), l.cel_at(18), l.cel_at(29)), (Some(source), Some(milieu), Some(bout)), "les dessins sont là où le modèle les dit");
    assert_eq!(l.cel_at(8), None, "les images insérées sont vides, comme demandé");
    assert_eq!(l.trajectoire.as_ref().unwrap().dessins.len(), 3);
    // Recalculer repose les mêmes dessins aux mêmes images (et comble le vide inséré par le dessin du départ).
    b.app.trajectoire_recalculer(0);
    let l = &b.app.layers()[0];
    assert_eq!((l.cel_at(0), l.cel_at(8), l.cel_at(18), l.cel_at(29)), (Some(source), Some(source), Some(milieu), Some(bout)));
    assert_eq!(l.trajectoire.as_ref().unwrap().dessins.iter().map(|d| d.dessin).collect::<Vec<_>>(), vec![source, milieu, bout]);
}

#[test]
fn with_the_option_the_drawing_turns_with_the_path_copies_are_rotated_and_keys_carry_the_angle() {
    let mut b = banc();
    b.app.traj.cles_propose = Cles::Images;
    b.app.traj.tourne_propose = true;
    let source = dessiner_un_trait_couche(&mut b);
    let boite = b.app.doc.cels[&source].vector.as_ref().unwrap().elements[0].bounds();
    assert!(boite.width() > 55.0 && boite.height() < 12.0, "un trait couché : {boite:?}");
    tracer_une_equerre(&mut b);
    let t = b.app.layers()[0].trajectoire.clone().expect("un chemin");
    assert!(t.tourne, "le réglage proposé est pris");
    assert_eq!(t.dessins.len(), 2, "le départ et l'arrivée");
    assert_eq!(t.dessins[0].dessin, source, "au départ, le dessin d'origine, droit");
    assert!((t.dessins[1].angle - 90.0).abs() < 8.0, "à l'arrivée, le chemin descend : un quart de tour, {}", t.dessins[1].angle);
    // La copie de l'arrivée est debout : tournée autour du départ du trait (50, 150) puis portée au bout du chemin (150, 220), ses
    // points vont de (150, 230) à (150, 290) à peu près.
    let copie = t.dessins[1].dessin;
    assert_ne!(copie, source);
    let bc = b.app.doc.cels[&copie].vector.as_ref().unwrap().elements[0].bounds();
    assert!(bc.height() > 55.0 && bc.width() < 20.0, "debout maintenant : {bc:?}");
    assert!((bc.center().x - 150.0).abs() < 8.0 && (bc.center().y - 260.0).abs() < 8.0, "{bc:?}");
    assert_eq!(b.app.layers()[0].cel_at(23), Some(copie));
    assert_eq!(derniere_etape(&b), "Trajectoire");
    // Enregistré, relu : la rotation revient (format 50).
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire-tourne-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.shot");
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"tourne\": true") || texte.contains("\"tourne\":true"), "{}", texte.lines().filter(|l| l.contains("tourne")).collect::<Vec<_>>().join(" / "));
    let back = crate::project::load(&manifest).unwrap().doc;
    let tr = back.panels[0].layers[0].trajectoire.as_ref().unwrap();
    assert!(tr.tourne && (tr.dessins[1].angle - t.dessins[1].angle).abs() < 1e-3);
    std::fs::remove_dir_all(&dir).ok();
    // Clés d'interpolation : chaque clé porte l'angle du chemin, le point d'ancrage reste sur le chemin.
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().cles = Cles::Interpolation;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Sorte des clés de la trajectoire");
    let l = &b.app.layers()[0];
    assert_eq!(l.pos_keys.len(), 24);
    assert_eq!(l.pos_keys[0].t.angle, 0.0, "droit au départ");
    assert!((l.pos_keys[23].t.angle - 90.0).abs() < 8.0, "{}", l.pos_keys[23].t.angle);
    let centre = b.app.doc.stage_center();
    let au_bout = l.pose(23).affine(centre).apply(pos2(50.0, 150.0));
    assert!((au_bout.x - 150.0).abs() < 0.5 && (au_bout.y - 220.0).abs() < 0.5, "le départ du trait, tourné, est au bout du chemin : {au_bout:?}");
    // Le chemin tourne d'un coup dans le coin : l'angle y change vite (c'est voulu), mais toujours dans le même sens, jamais d'un tour.
    assert!(l.pos_keys.windows(2).all(|w| { let d = w[1].t.angle - w[0].t.angle; (-2.0..95.0).contains(&d) }), "l'angle tourne dans un seul sens, sans faire le tour : {:?}", l.pos_keys.iter().map(|k| k.t.angle.round()).collect::<Vec<_>>());
    assert!((0..24).all(|f| l.cel_at(f) == Some(source)), "les copies sont parties");
    // Sans l'option : les clés ne tournent plus.
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().tourne = false;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Le dessin ne tourne plus avec le chemin");
    assert!(b.app.layers()[0].pos_keys.iter().all(|k| k.t.angle == 0.0));
    assert_eq!(derniere_etape(&b), "Le dessin ne tourne plus avec le chemin");
    assert!(crate::trajectoireui::TOURNE.contains("tourne"));
}

#[test]
fn a_bitmap_is_resampled_where_the_affinity_sends_it_and_a_plain_shift_is_a_shift() {
    use crate::raster::{Raster, TILE};
    use crate::vector::Affine;
    use eframe::egui::{vec2, Color32};
    // Une barre couchée de 60 × 6 pixels, de (50, 100) à (110, 106), noire.
    let mut r = Raster::new(400, 300);
    for y in 100..106 {
        for x in 50..110 {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = Color32::BLACK;
        }
    }
    // Un quart de tour (sens des aiguilles, y vers le bas) autour de (50, 100) : la barre est debout, de (44, 100) à (50, 160).
    let a = Affine::rotate_about(pos2(50.0, 100.0), std::f32::consts::FRAC_PI_2);
    let debout = r.transformee(&a);
    assert!(debout.get_i(47, 130).a() > 200, "au milieu de la barre debout : {:?}", debout.get_i(47, 130));
    assert!(debout.get_i(47, 158).a() > 100, "près du bout : {:?}", debout.get_i(47, 158));
    assert_eq!(debout.get_i(80, 103).a(), 0, "plus rien à l'ancienne place");
    assert_eq!(debout.get_i(47, 170).a(), 0, "rien au-delà du bout");
    assert_eq!(debout.get_i(60, 130).a(), 0, "rien à côté");
    // Un demi-pixel de décalage se voit dans les bords adoucis, jamais au-delà d'un pixel.
    assert!(debout.get_i(42, 130).a() == 0 && debout.get_i(52, 130).a() == 0);
    // Une translation entière : exactement `decalee`.
    let d = r.transformee(&Affine::translate(vec2(30.0, -20.0)));
    let e = r.decalee(30, -20);
    for (x, y) in [(80, 80), (139, 85), (79, 80), (140, 80), (100, 79), (100, 86)] {
        assert_eq!(d.get_i(x, y), e.get_i(x, y), "({x}, {y})");
    }
    // Hors de la feuille aussi (la toile infinie) : poussée à gauche de la feuille, la barre est là.
    let dehors = r.transformee(&Affine::translate(vec2(-100.0, 0.0)));
    assert!(dehors.get_i(-30, 103).a() > 200);
    // Une affinité écrasée : rien.
    assert!(r.transformee(&Affine { a: 0.0, b: 0.0, c: 0.0, d: 0.0, tx: 0.0, ty: 0.0 }).is_blank());
}

#[test]
fn the_tool_is_refused_in_the_vignettes_and_has_its_action_its_slot_and_its_hint() {
    assert!(!Tool::Trajectoire.permis_dans_les_vignettes());
    assert_eq!(Tool::Trajectoire.action(), "outil.trajectoire");
    assert!(crate::toolbars::TOOL_GROUPS.iter().flat_map(|g| g.iter()).any(|s| s.contains(&Tool::Trajectoire)));
    assert!(crate::viewbar::tool_hint(Tool::Trajectoire).contains("marque"));
    let mut b = banc();
    b.app.set_tool(Tool::Brush);
    b.app.modified = false;
    b.app.run_action("outil.trajectoire");
    assert_eq!(b.app.tool, Tool::Trajectoire);
    assert!(!b.app.modified);
}
