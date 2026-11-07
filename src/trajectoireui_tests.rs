//! L'outil Trajectoire dans l'application (`trajectoireui.rs`, Anthony, 2026-11-07) : tracer, marquer, glisser, arracher,
//! déplacer, annuler, détacher, enregistrer.

use eframe::egui::{pos2, Event, Key, Modifiers, PointerButton, Pos2};

use crate::app::{Mode, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};

const DT: f64 = 1.0 / 60.0;

/// Un plan 400 × 300 de 24 images, un calque vectoriel, l'outil Trajectoire en main, à l'écran (1280 × 720).
fn banc() -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Vector)];
    doc.panels[0].length = 24;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.set_tool(Tool::Trajectoire);
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn bouton(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Un glisser sur les points `pts` de la feuille, une image d'écran par point.
fn glisser(b: &mut Banc, pts: &[(f32, f32)]) {
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], DT);
    b.image(vec![bouton(p0, true)], DT);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], DT);
    }
    let (x, y) = pts[pts.len() - 1];
    let fin = ecran(b, x, y);
    b.image(vec![bouton(fin, false)], DT);
    b.image(Vec::new(), DT);
}

/// Un chemin droit de (50, 150) à (350, 150), en six points.
fn tracer_le_chemin(b: &mut Banc) {
    glisser(b, &[(50.0, 150.0), (110.0, 150.0), (170.0, 150.0), (230.0, 150.0), (290.0, 150.0), (350.0, 150.0)]);
}

fn derniere_etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

fn cles(b: &Banc) -> Vec<(usize, f32, f32)> {
    b.app.layers()[0].pos_keys.iter().map(|k| (k.frame, k.t.x, k.t.y)).collect()
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
    glisser(&mut b, &[(200.0, 135.0), (200.0, 150.0), (200.0, 165.0)]);
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
fn the_path_moves_as_a_whole_and_escape_gives_the_gesture_up() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    // Prendre le chemin (loin de toute marque) et le descendre de 50.
    glisser(&mut b, &[(120.0, 150.0), (120.0, 175.0), (120.0, 200.0)]);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!((t.point_a(0.0)[1] - 200.0).abs() < 1.0, "descendu de 50 : {:?}", t.point_a(0.0));
    assert!((t.point_a(0.0)[0] - 50.0).abs() < 1.0);
    assert_eq!(derniere_etape(&b), "Déplacer la trajectoire");
    // Le déplacement est relatif : les clés ne changent pas.
    let c = cles(&b);
    assert_eq!(c[0], (0, 0.0, 0.0));
    assert!((c[23].1 - 300.0).abs() < 2.0 && c[23].2.abs() < 1.0);
    // Échap au milieu d'un glisser : tout revient, aucune étape.
    let etapes = b.app.history.names().0.len();
    let p0 = ecran(&b, 120.0, 200.0);
    b.image(vec![Event::PointerMoved(p0)], DT);
    b.image(vec![bouton(p0, true)], DT);
    let p1 = ecran(&b, 120.0, 260.0);
    b.image(vec![Event::PointerMoved(p1)], DT);
    assert!((b.app.layers()[0].trajectoire.as_ref().unwrap().point_a(0.0)[1] - 260.0).abs() < 1.0, "il suit pendant le geste");
    assert!(b.app.trajectoire_touche(Key::Escape));
    assert!((b.app.layers()[0].trajectoire.as_ref().unwrap().point_a(0.0)[1] - 200.0).abs() < 1.0, "Échap : comme avant");
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
fn editing_a_key_by_hand_detaches_the_path_and_keeps_the_keys() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    b.app.cur_frame = 5;
    b.app.pos_key_here();
    assert!(b.app.layers()[0].trajectoire.is_none(), "détachée");
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24, "les clés restent");
    assert!(b.app.status.contains("détachée"), "{}", b.app.status);
    b.app.undo();
    assert!(b.app.layers()[0].trajectoire.is_some(), "une seule étape, qui rend le chemin");
    // Les commandes du panneau : détacher, effacer.
    b.app.undo();
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

#[test]
fn the_path_starts_at_the_playhead_and_image_keys_hold_between_marks() {
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
    // Une marque, puis « Clés d'images » : une clé tenue à chaque jalon, et la pose ne bouge pas entre deux.
    glisser(&mut b, &[(200.0, 135.0), (200.0, 165.0)]);
    let snap = b.app.snapshot();
    b.app.layers_mut()[0].trajectoire.as_mut().unwrap().cles = crate::trajectoire::Cles::Images;
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, "Sorte des clés de la trajectoire");
    let keys = b.app.layers()[0].pos_keys.clone();
    // (Le milieu du temps, de 6 à 23, tombe à 14,5 : arrondi à l'image 15.)
    assert_eq!(keys.iter().map(|k| (k.frame, k.tenue)).collect::<Vec<_>>(), [(6, true), (15, true), (23, true)], "{keys:?}");
    let l = &b.app.layers()[0];
    assert_eq!(l.pose(10).x, l.pose(6).x, "tenue jusqu'à la marque");
    assert!((l.pose(15).x - 150.0).abs() < 3.0 && l.pose(20).x == l.pose(15).x, "tenue à la marque jusqu'à l'arrivée");
    // Le départ passe à la tête de lecture, sur demande.
    b.app.cur_frame = 2;
    b.app.trajectoire_depart_ici();
    assert_eq!(b.app.layers()[0].trajectoire.as_ref().unwrap().depart, 2);
    assert_eq!(b.app.layers()[0].pos_keys.first().map(|k| k.frame), Some(2));
    assert_eq!(derniere_etape(&b), "Départ de la trajectoire");
    b.app.undo();
    assert_eq!(b.app.layers()[0].trajectoire.as_ref().unwrap().depart, 6);
    // Enregistré, relu : le départ et la sorte des clés reviennent.
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire-depart-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.shot");
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).unwrap();
    let back = crate::project::load(&manifest).unwrap().doc;
    let t = back.panels[0].layers[0].trajectoire.as_ref().unwrap();
    assert_eq!((t.depart, t.cles), (6, crate::trajectoire::Cles::Images));
    assert!(back.panels[0].layers[0].pos_keys.iter().all(|k| k.tenue), "les clés tenues le restent");
    std::fs::remove_dir_all(&dir).ok();
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
