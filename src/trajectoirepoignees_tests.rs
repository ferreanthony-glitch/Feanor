//! Essais des poignées de la trajectoire (`trajectoirepoignees.rs`, `docs/INTERFACE.md` partie 154) : Ctrl+clic sur le
//! chemin pose une poignée, ailleurs choisit toujours, sur une poignée la retire ; tirer une poignée tord le chemin en
//! douceur, les bouts et les autres poignées fixes ; dans l'espace, la flèche y monte le chemin hors du sol, avec la carte
//! et la caméra qui le suivent ; enregistrer et relire ; lissage, prolongement et poignées ensemble ; une étape
//! d'annulation, Échap abandonne.

use eframe::egui::{pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d;
use crate::espace3dgizmo::{Gizmo, CAMERA_ID};
use crate::espace3dmath as m;
use crate::perf_tests::{banc_taille, Banc};
use crate::project;
use crate::trajectoire::{Cles, Support, Trajectoire};
use crate::trajectoire3d_tests::{a_l_ecran, appuyer, carte, derniere_etape, doc_cartes, geste, lacher, ouvert, regard, tracer_au_sol, trajectoire};
use crate::trajectoirepoignees as tp;

const DT: f64 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------------------ le modèle

/// Un chemin droit de 200 unités sur un sol à la hauteur 0, une unité valant un centième de mètre.
fn au_sol() -> Trajectoire {
    let sol = Support::nouveau([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.01).expect("un plan");
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[0.0, 0.0], [200.0, 0.0]], sol).expect("un chemin");
    (t.cles, t.fin) = (Cles::Interpolation, Some(10));
    t
}

#[test]
fn pulling_a_handle_bends_the_path_smoothly_and_the_ends_and_the_other_handles_stay() {
    let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [200.0, 0.0]]).expect("un chemin");
    t.ajouter_marque(0.6);
    assert_eq!(t.ajouter_poignee(0.3), Some(0));
    assert_eq!(t.ajouter_poignee(0.8), Some(1));
    assert_eq!(t.ajouter_poignee(0.805), None, "une poignée est déjà là");
    let rayon = t.rayon_de_tirage();
    t.tirer_poignee(0, [0.0, 40.0, 0.0], rayon);
    // La poignée a suivi, à sa nouvelle fraction ; les bouts et l'autre poignée n'ont pas bougé.
    let p = t.point_a(t.poignees[0]);
    assert!((p[0] - 60.0).abs() < 2.0 && (p[1] - 40.0).abs() < 1.0, "la poignée suit le pointeur : {p:?}");
    assert_eq!(t.point_a(0.0), [0.0, 0.0], "le départ reste");
    assert!((t.point_a(1.0)[0] - 200.0).abs() < 1e-3 && t.point_a(1.0)[1].abs() < 1e-3, "l'arrivée reste");
    let q = t.point_a(t.poignees[1]);
    assert!((q[0] - 160.0).abs() < 1.5 && q[1].abs() < 1e-3, "l'autre poignée reste : {q:?}");
    assert!(t.points.iter().filter(|p| p[0] >= 160.0).all(|p| p[1].abs() < 1e-3), "au-delà de l'autre poignée, rien ne bouge");
    // En douceur : la hauteur monte puis descend sans saut, de part et d'autre de la poignée.
    let ys: Vec<f32> = t.points.iter().map(|p| p[1]).collect();
    assert!(ys.windows(2).all(|w| (w[1] - w[0]).abs() < 3.0), "pas de cassure");
    let haut = ys.iter().cloned().fold(0.0, f32::max);
    assert!((haut - 40.0).abs() < 1.0, "le sommet est à la poignée : {haut}");
    assert_eq!(t.marques, [0.6], "les marques gardent leurs fractions");
    // Le tirage du chemin, hors des poignées, s'arrête aux poignées voisines.
    let avant = t.clone();
    t.tirer(0.95, 0.0, -30.0, t.rayon_de_tirage());
    let q2 = t.point_a(t.poignees[1]);
    assert!((q2[0] - q[0]).abs() < 1.5 && (q2[1] - q[1]).abs() < 0.05, "une poignée borne le tirage du chemin : {q2:?}");
    assert!(t.points.iter().filter(|p| p[0] < 150.0).all(|p| avant.plus_proche(*p).1 < 0.05), "avant l'autre poignée, le chemin n'a pas bougé");
    t.retirer_poignee(0);
    assert_eq!(t.poignees.len(), 1);
}

#[test]
fn heights_and_handles_survive_json_and_an_old_file_still_reads() {
    let mut t = au_sol();
    t.ajouter_poignee(0.5);
    t.tirer_poignee(0, [0.0, 0.0, 50.0], t.rayon_de_tirage());
    assert!(!t.hauteurs.is_empty() && t.hauteurs.len() == t.points.len(), "le chemin a quitté son plan");
    let texte = serde_json::to_string(&t).expect("écrit");
    assert!(texte.contains("\"poignees\"") && texte.contains("\"hauteurs\""), "{texte}");
    let relue: Trajectoire = serde_json::from_str(&texte).expect("relu");
    assert_eq!(relue.clone().assainie().as_ref(), Some(&t), "elle revient telle quelle");
    // Un fichier d'avant (sans ces champs) se relit : pas de poignée, à plat.
    let mut v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    for champ in ["poignees", "hauteurs", "hauteurs_brut"] {
        v.as_object_mut().unwrap().remove(champ);
    }
    let vieux: Trajectoire = serde_json::from_value(v).expect("un fichier d'avant se relit");
    let vieux = vieux.assainie().expect("tient debout");
    assert!(vieux.poignees.is_empty() && vieux.hauteurs.is_empty());
    assert!(vieux.deplacement_a(0.5).unwrap()[1].abs() < 1e-6, "à plat");
    // Des hauteurs abîmées (pas une par point) ne se lisent pas ; une trajectoire sur la feuille n'en a jamais.
    let mut abimee = t.clone();
    abimee.hauteurs.pop();
    assert!(abimee.assainie().unwrap().hauteurs.is_empty());
    let mut feuille = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    feuille.ajouter_poignee(0.5);
    feuille.hauteurs = vec![1.0; feuille.points.len()];
    let feuille = feuille.assainie().unwrap();
    assert!(feuille.hauteurs.is_empty() && feuille.poignees == [0.5]);
    // Le déplacement dans l'espace monte avec le chemin.
    // (La normale de ce sol, x × z, regarde vers le bas : une hauteur positive descend. La flèche y, elle, lit le monde.)
    let d = t.deplacement_a(t.poignees[0]).unwrap();
    assert!((d[1] + 0.5).abs() < 0.02, "50 unités d'un centième de mètre, le long de la normale : un demi-mètre : {d:?}");
    assert!(t.deplacement_a(1.0).unwrap()[1].abs() < 1e-4, "le bout reste au sol");
}

#[test]
fn smoothing_extending_and_handles_go_together() {
    let mut t = au_sol();
    t.ajouter_poignee(0.5);
    t.tirer_poignee(0, [0.0, 0.0, 60.0], t.rayon_de_tirage());
    let sommet = t.monde_a(t.poignees[0]).unwrap();
    // Lissé : les hauteurs sont lissées avec le reste, les bouts restent au sol ; à 0, le chemin revient tel quel.
    let avant = t.clone();
    t.lisser_a(0.6);
    assert!(!t.hauteurs.is_empty() && !t.hauteurs_brut.is_empty());
    let h_max = t.hauteurs.iter().cloned().fold(0.0, f32::max);
    assert!(h_max > 20.0 && h_max < 60.0, "lissées, pas perdues : {h_max}");
    assert!(t.hauteur_du_point(0).abs() < 1e-4 && t.hauteur_du_point(t.points.len() - 1).abs() < 1e-4, "les bouts restent au sol");
    t.lisser_a(0.0);
    assert_eq!((t.points.clone(), t.hauteurs.clone()), (avant.points.clone(), avant.hauteurs.clone()), "0 % : le chemin tel qu'il était");
    // Tirer une poignée d'un chemin lissé : le chemin brut est tiré, puis lissé de nouveau.
    t.lisser_a(0.4);
    t.tirer_poignee(0, [0.0, 0.0, 10.0], t.rayon_de_tirage());
    assert!(t.brut.is_some() && t.hauteurs_brut.iter().cloned().fold(0.0, f32::max) > 65.0);
    // Prolongé par la fin : la poignée garde sa place dans l'espace, le morceau ajouté part à la hauteur du bout.
    let mut u = avant.clone();
    assert!(u.prolonger(&[[260.0, 0.0], [300.0, 0.0]], false));
    let ici = u.monde_a(u.poignees[0]).unwrap();
    assert!(m::length(m::sub(ici, sommet)) < 0.03, "la poignée reste où elle était : {ici:?} contre {sommet:?}");
    assert!(u.hauteur_a(0.95).abs() < 1e-4, "le morceau ajouté est au sol, comme le bout");
    assert_eq!(u.hauteurs.len(), u.points.len());
    // Prolongé par le début : le déplacement se mesure toujours depuis l'ancien départ.
    let mut v = avant.clone();
    let d_avant = v.deplacement_a(v.poignees[0]).unwrap();
    assert!(v.prolonger(&[[-40.0, 0.0], [-100.0, 0.0]], true));
    let d_apres = v.deplacement_a(v.poignees[0]).unwrap();
    assert!(m::length(m::sub(d_avant, d_apres)) < 0.03, "{d_avant:?} contre {d_apres:?}");
    // Retracé : le trait neuf est à plat, les poignées restent à leurs fractions.
    let mut w = avant.clone();
    assert!(w.retracer(&[[0.0, 50.0], [200.0, 50.0]]));
    assert!(w.hauteurs.is_empty() && w.poignees == avant.poignees);
    // Dans un miroir, la normale du plan se retourne : les hauteurs aussi, le chemin reste au-dessus du sol.
    let mut x = avant.clone();
    let haut = x.monde_a(x.poignees[0]).unwrap()[1];
    x.miroir_x(0.0);
    assert!((x.monde_a(x.poignees[0]).unwrap()[1] - haut).abs() < 1e-4);
}

#[test]
fn on_a_strongly_smoothed_zigzag_the_handles_stay_where_they_are_seen() {
    // Un zigzag, lissé fort : le chemin brut est bien plus long que le chemin lissé qu'on voit.
    let zigzag: Vec<[f32; 2]> = (0..=80).map(|i| [i as f32 * 5.0, 40.0 * (i as f32 * std::f32::consts::TAU / 8.0).sin()]).collect();
    let mut t = Trajectoire::nouvelle(&zigzag).expect("un chemin");
    t.lisser_a(1.0);
    let (brut, lisse) = (crate::trajectoire::longueur_de(t.chemin_brut()), t.longueur());
    assert!(brut > lisse * 1.3, "l'essai a un sens : {brut} contre {lisse}");
    t.ajouter_poignee(0.3);
    t.ajouter_poignee(0.7);
    let (a, b) = (t.point_a(t.poignees[0]), t.point_a(t.poignees[1]));
    t.tirer_poignee(0, [0.0, 40.0, 0.0], t.rayon_de_tirage());
    let (a2, b2) = (t.point_a(t.poignees[0]), t.point_a(t.poignees[1]));
    assert!(crate::trajectoire::distance(b, b2) < 4.0, "l'autre poignée reste où on la voit : {b:?} → {b2:?}");
    assert!((a2[0] - a[0]).abs() < 8.0 && a2[1] > a[1] + 15.0, "la poignée tirée monte là où on l'a prise : {a:?} → {a2:?}");
}

// ------------------------------------------------------------------------------------------------ sur la feuille

fn banc_feuille() -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Vector)];
    doc.panels[0].length = 24;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(crate::app::Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.set_tool(Tool::Trajectoire);
    b.app.traj.cles_propose = Cles::Interpolation;
    b.chauffer(10);
    b
}

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

/// Un geste sur la feuille par ces points, ces touches tenues ; `lacher` : le bouton est relâché au bout.
fn sur_la_feuille(b: &mut Banc, pts: &[(f32, f32)], touches: Modifiers, lacher: bool) {
    b.touches = touches;
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], DT);
    b.image(vec![Event::PointerButton { pos: p0, button: PointerButton::Primary, pressed: true, modifiers: touches }], DT);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], DT);
    }
    if lacher {
        let (x, y) = pts[pts.len() - 1];
        let fin = ecran(b, x, y);
        b.image(vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: touches }], DT);
        b.touches = Modifiers::NONE;
        b.chauffer(3);
    }
}

fn chemin(b: &Banc) -> Trajectoire {
    b.app.layers()[0].trajectoire.clone().expect("un chemin")
}

fn tracer_droit(b: &mut Banc) {
    sur_la_feuille(b, &[(50.0, 150.0), (110.0, 150.0), (170.0, 150.0), (230.0, 150.0), (290.0, 150.0), (350.0, 150.0)], Modifiers::NONE, true);
}

#[test]
fn on_the_sheet_ctrl_click_lays_a_handle_on_the_path_removes_it_on_a_handle_and_keeps_the_lasso_elsewhere() {
    let mut b = banc_feuille();
    tracer_droit(&mut b);
    let etapes = b.app.history.undo_len();
    // Ctrl+clic sur le chemin : une poignée, choisie, une étape.
    sur_la_feuille(&mut b, &[(200.0, 152.0)], Modifiers::CTRL, true);
    let t = chemin(&b);
    assert_eq!(t.poignees.len(), 1, "{}", b.app.status);
    assert!((t.poignees[0] - 0.5).abs() < 0.02, "{:?}", t.poignees);
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), (tp::POSER.to_string(), etapes + 1));
    assert_eq!(b.app.poignee_de(b.app.layers()[0].id), Some(0));
    assert!(crate::messages::resume(&b.app.status).starts_with("Poignée posée"), "ce que montre la barre : {}", crate::messages::resume(&b.app.status));
    assert_eq!(b.app.tool, Tool::Trajectoire, "Ctrl sur le chemin ne prend pas le lasso");
    // Ctrl+clic loin du chemin : le lasso, comme avant ; aucune poignée.
    sur_la_feuille(&mut b, &[(200.0, 40.0), (240.0, 40.0), (240.0, 80.0)], Modifiers::CTRL, false);
    assert_eq!(b.app.tool, Tool::Select, "ailleurs, Ctrl+appui prend le lasso");
    let fin = ecran(&b, 240.0, 80.0);
    b.image(vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::CTRL }], DT);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    assert_eq!(b.app.tool, Tool::Trajectoire, "Ctrl relâché : l'outil revient");
    assert_eq!(chemin(&b).poignees.len(), 1);
    // Ctrl+clic sur la poignée : retirée, une étape.
    let n = b.app.history.undo_len();
    sur_la_feuille(&mut b, &[(200.0, 150.0)], Modifiers::CTRL, true);
    assert!(chemin(&b).poignees.is_empty(), "{}", b.app.status);
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), (tp::RETIRER.to_string(), n + 1));
    b.app.undo();
    assert_eq!(chemin(&b).poignees.len(), 1, "Ctrl+Z la rend");
}

#[test]
fn on_the_sheet_pulling_a_handle_bends_the_path_smoothly_in_one_undo_step_and_escape_gives_it_up() {
    let mut b = banc_feuille();
    tracer_droit(&mut b);
    sur_la_feuille(&mut b, &[(200.0, 150.0)], Modifiers::CTRL, true);
    sur_la_feuille(&mut b, &[(290.0, 150.0)], Modifiers::CTRL, true);
    assert_eq!(chemin(&b).poignees.len(), 2, "{}", b.app.status);
    let (avant, cles_avant, etapes) = (chemin(&b), b.app.layers()[0].pos_keys.clone(), b.app.history.undo_len());
    // Échap pendant qu'on la tire : tout revient.
    sur_la_feuille(&mut b, &[(200.0, 150.0), (200.0, 130.0), (200.0, 110.0)], Modifiers::NONE, false);
    assert!(chemin(&b).points.iter().any(|p| p[1] < 120.0), "le chemin suit pendant le geste");
    assert!(b.app.trajectoire_touche(Key::Escape));
    assert_eq!(chemin(&b), avant, "Échap : comme avant");
    assert_eq!(b.app.layers()[0].pos_keys, cles_avant);
    let p = ecran(&b, 200.0, 110.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(2);
    assert_eq!(b.app.history.undo_len(), etapes, "rien d'annulable");
    // Tirée pour de bon : la poignée suit, le chemin se tord en douceur, les bouts et l'autre poignée restent.
    sur_la_feuille(&mut b, &[(200.0, 150.0), (200.0, 130.0), (200.0, 100.0)], Modifiers::NONE, true);
    let t = chemin(&b);
    let ici = t.point_a(t.poignees[0]);
    assert!((ici[0] - 200.0).abs() < 2.0 && (ici[1] - 100.0).abs() < 2.0, "la poignée est sous le pointeur : {ici:?}");
    assert!((t.point_a(0.0)[1] - 150.0).abs() < 0.5 && (t.point_a(1.0)[1] - 150.0).abs() < 0.5, "les bouts restent");
    let autre = t.point_a(t.poignees[1]);
    assert!((autre[0] - 290.0).abs() < 2.0 && (autre[1] - 150.0).abs() < 0.5, "l'autre poignée reste : {autre:?}");
    assert!(t.points.iter().filter(|p| p[0] > 291.0).all(|p| (p[1] - 150.0).abs() < 1e-3), "au-delà, rien ne bouge");
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), (tp::TIRER.to_string(), etapes + 1), "une étape");
    // Les clés suivent : le calque passe au-dessus de la ligne de départ à mi-chemin.
    assert!(b.app.layers()[0].pos_keys.iter().any(|k| k.t.y < -30.0), "{:?}", b.app.layers()[0].pos_keys.iter().map(|k| k.t.y).collect::<Vec<_>>());
    b.app.undo();
    assert_eq!(chemin(&b), avant, "une annulation rend le chemin");
}

// ------------------------------------------------------------------------------------------------ dans l'Espace 3D

/// Le point de l'écran, aux deux tiers de la flèche `axe` de la poignée `i` de `t` (choisie), dans la vue seule.
fn sur_la_fleche(b: &Banc, t: &Trajectoire, i: usize, axe: usize) -> Pos2 {
    let r = regard(b);
    let o = t.monde_a(t.poignees[i]).unwrap();
    let mut dir = [0.0; 3];
    dir[axe] = 1.0;
    let bout = m::add(o, m::scale(dir, Gizmo::size_for(&r.view, o) * 0.75));
    let (po, pb) = (r.ecran(o).unwrap(), r.ecran(bout).unwrap());
    po + (pb - po) * 0.66
}

#[test]
fn in_space_ctrl_click_lays_a_handle_and_its_y_arrow_lifts_the_path_the_card_and_the_camera_off_the_ground() {
    let mut doc = doc_cartes(3);
    doc.panels[0].layers[1].transform.x = 120.0;
    let mut b = ouvert(doc, true);
    let ids: Vec<u64> = b.app.layers().iter().map(|l| l.id).collect();
    b.app.espace3d.sel = vec![espace3d::card_selection(ids[0]), espace3d::card_selection(ids[1]), CAMERA_ID];
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    let (depart, camera_avant) = (carte(&b, 0, 0), b.app.doc.panel_camera_reelle(0, 12));
    // La carte 1 se verrouille : elle est laissée de côté, la barre d'état le dit d'abord.
    b.app.layers_mut()[1].locked = true;
    let etapes = b.app.history.undo_len();
    let milieu = a_l_ecran(&b, t.monde_a(0.5).unwrap());
    geste(&mut b, &[milieu], Modifiers::CTRL);
    assert_eq!(trajectoire(&b, 0).poignees.len(), 1, "{}", b.app.status);
    assert_eq!(b.app.doc.panels[0].trajectoire_camera.as_ref().map(|t| t.poignees.len()), Some(1), "la caméra a la sienne");
    assert!(trajectoire(&b, 1).poignees.is_empty(), "la carte verrouillée est laissée");
    assert!(crate::messages::resume(&b.app.status).contains("laissé de côté"), "ce que montre la barre : {}", crate::messages::resume(&b.app.status));
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), (tp::POSER.to_string(), etapes + 1), "une étape pour tous");
    assert_eq!(b.app.espace3d.sel.len(), 3, "Ctrl+clic sur le chemin ne change pas ce qui est choisi");
    // La flèche y de la poignée choisie : tirée vers le haut de l'écran, la poignée monte, hors du sol.
    let t = trajectoire(&b, 0);
    let f = t.poignees[0];
    let prise = sur_la_fleche(&b, &t, 0, 1);
    appuyer(&mut b, &[prise, prise + vec2(0.0, -40.0), prise + vec2(0.0, -80.0)], Modifiers::NONE);
    assert!(matches!(b.app.traj.espace, Some(crate::trajectoire3d::GesteEspace::Fleche { axe: 1, .. })), "la flèche y est prise : {}", b.app.status);
    lacher(&mut b, prise + vec2(0.0, -80.0), Modifiers::NONE);
    let t = trajectoire(&b, 0);
    let sol_y = b.app.e3_world().sol_y();
    let haut = t.monde_a(t.poignees[0]).unwrap();
    assert!(haut[1] > sol_y + 0.3, "la poignée a monté : {haut:?} (sol {sol_y})");
    assert!((t.monde_a(0.0).unwrap()[1] - sol_y).abs() < 1e-3 && (t.monde_a(1.0).unwrap()[1] - sol_y).abs() < 1e-3, "les bouts restent au sol");
    assert!((t.poignees[0] - f).abs() < 0.03);
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), (tp::TIRER.to_string(), etapes + 2));
    // La carte qui suit monte avec le chemin, et la caméra aussi.
    let plus_haute = (0..24).map(|i| carte(&b, 0, i).pos[1] - depart.pos[1]).fold(f32::MIN, f32::max);
    assert!(plus_haute > 0.3, "la carte monte : {plus_haute}");
    assert!((carte(&b, 0, 23).pos[1] - depart.pos[1]).abs() < 1e-2, "et redescend au bout");
    let world = b.app.e3_world();
    let monte = world.eye(&b.app.doc.panel_camera_reelle(0, 12))[1] - world.eye(&camera_avant)[1];
    assert!(monte > 0.3, "la caméra monte : {monte}");
    assert!(b.app.doc.panels[0].camera.is_empty(), "toujours sans clé de caméra");
    // Enregistrer puis relire garde les poignées et les hauteurs, au format 55.
    let dir = std::env::temp_dir().join(format!("feanor-poignees-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    let manifest = dir.join("P.shot");
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    assert!(project::VERSION == 55);
    let relu = project::load(&manifest).expect("relu").doc;
    assert_eq!(relu.panels[0].layers[0].trajectoire.as_ref(), Some(&t));
    assert_eq!(relu.panels[0].trajectoire_camera, b.app.doc.panels[0].trajectoire_camera);
    assert_eq!(relu.panel_camera_reelle(0, 12), b.app.doc.panel_camera_reelle(0, 12));
    std::fs::remove_dir_all(&dir).ok();
    // Échap pendant qu'on tire la flèche : tout revient.
    let avant = trajectoire(&b, 0);
    let prise = sur_la_fleche(&b, &avant, 0, 1);
    appuyer(&mut b, &[prise, prise + vec2(0.0, -50.0)], Modifiers::NONE);
    assert_ne!(trajectoire(&b, 0), avant, "le chemin suit pendant le geste");
    assert!(b.app.trajectoire_touche(Key::Escape));
    assert_eq!(trajectoire(&b, 0), avant);
    lacher(&mut b, prise + vec2(0.0, -50.0), Modifiers::NONE);
    assert_eq!(b.app.history.undo_len(), etapes + 2, "Échap : aucune étape");
    // Ctrl+Z : à plat de nouveau, la carte aussi.
    b.app.undo();
    assert!(trajectoire(&b, 0).hauteurs.is_empty());
    assert!((carte(&b, 0, 12).pos[1] - depart.pos[1]).abs() < 1e-3);
    // Ctrl+clic sur la poignée : retirée, pour tous.
    let t = trajectoire(&b, 0);
    let ici = a_l_ecran(&b, t.monde_a(t.poignees[0]).unwrap());
    geste(&mut b, &[ici], Modifiers::CTRL);
    assert!(trajectoire(&b, 0).poignees.is_empty() && b.app.doc.panels[0].trajectoire_camera.as_ref().unwrap().poignees.is_empty(), "{}", b.app.status);
    assert_eq!(derniere_etape(&b), tp::RETIRER);
}

#[test]
fn in_space_ctrl_click_away_from_the_path_still_chooses_a_card_and_dragging_a_handle_bends_the_path_on_its_plane() {
    let mut doc = doc_cartes(2);
    doc.panels[0].layers[1].transform.x = 150.0;
    let mut b = ouvert(doc, true);
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    let ici = a_l_ecran(&b, t.monde_a(0.4).unwrap());
    geste(&mut b, &[ici], Modifiers::CTRL);
    assert_eq!(trajectoire(&b, 0).poignees.len(), 1, "{}", b.app.status);
    // Ctrl+clic sur l'autre carte, loin du chemin : elle est choisie, seule, comme avant.
    let id = b.app.layers()[1].id;
    let t = trajectoire(&b, 0);
    let s = t.support.unwrap();
    let r = regard(&b);
    let dans_la_vue = |p: Pos2| [p.x - r.rect.left(), p.y - r.rect.top()];
    let centre = [r.rect.width() / 2.0, r.rect.height() / 2.0];
    let vu = crate::trajectoire3d::CheminVu::de(&t, &|p| r.ecran(s.monde3(p)));
    // (Un point de la carte 1 loin du chemin et du dessin de la caméra, qui passe devant tout.)
    let matrice = carte(&b, 1, 0).matrix();
    let candidats: Vec<Pos2> = [[0.42, 0.1], [-0.42, 0.1], [0.42, -0.35], [-0.42, -0.35], [0.0, 0.4], [0.0, -0.4]].iter().filter_map(|q| r.ecran(m::point(&matrice, [q[0], q[1], 0.0]))).collect();
    let mut sur_la_carte = None;
    for p in candidats {
        if b.app.canvas_rect.shrink(4.0).contains(p) && !b.app.e3_pick_camera(&r.view, dans_la_vue(p), centre) && b.app.e3_pick(&r.rayon(p)) == Some(espace3d::card_selection(id)) && vu.plus_proche(p).is_none_or(|(_, d)| d > 20.0) {
            sur_la_carte = Some(p);
            break;
        }
    }
    let sur_la_carte = sur_la_carte.expect("un point de la carte 1, loin du chemin et de la caméra");
    geste(&mut b, &[sur_la_carte], Modifiers::CTRL);
    assert_eq!(b.app.espace3d.sel, vec![espace3d::card_selection(id)], "{}", b.app.status);
    assert_eq!(trajectoire(&b, 0).poignees.len(), 1, "aucune poignée de plus");
    // De nouveau la carte 0 ; tirer sa poignée (sans flèche) la déplace sur le sol : le chemin reste dans son plan.
    b.app.espace3d.sel = vec![espace3d::card_selection(b.app.layers()[0].id)];
    b.chauffer(2);
    let t = trajectoire(&b, 0);
    let p0 = t.monde_a(t.poignees[0]).unwrap();
    let a = a_l_ecran(&b, p0);
    let z = a_l_ecran(&b, m::add(p0, [0.0, 0.0, 1.0]));
    geste(&mut b, &[a, z], Modifiers::NONE);
    let u = trajectoire(&b, 0);
    let ici = u.monde_a(u.poignees[0]).unwrap();
    assert!(m::length(m::sub(ici, m::add(p0, [0.0, 0.0, 1.0]))) < 0.1, "la poignée suit le pointeur sur le sol : {ici:?} ({})", b.app.status);
    assert!(u.hauteurs.is_empty(), "à plat");
    assert!(m::length(m::sub(u.monde_a(0.0).unwrap(), t.monde_a(0.0).unwrap())) < 1e-3, "le départ reste");
    assert_eq!(derniere_etape(&b), tp::TIRER);
}

#[test]
fn in_a_front_orthographic_view_the_y_arrow_lifts_the_path() {
    use crate::espace3dvues::{Disposition, PointDeVue};
    let mut b = ouvert(doc_cartes(1), true);
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    let ici = a_l_ecran(&b, t.monde_a(0.5).unwrap());
    geste(&mut b, &[ici], Modifiers::CTRL);
    assert_eq!(trajectoire(&b, 0).poignees.len(), 1, "{}", b.app.status);
    // Quatre vues, toutes de face, recadrées.
    b.app.run_action(Disposition::Quatre.action());
    b.chauffer(3);
    for i in Disposition::Quatre.cadres() {
        b.app.espace3d.vues.cadres[*i].point = PointDeVue::Face;
    }
    b.app.run_action("espace3d.cadrer");
    b.chauffer(4);
    let (i, r) = *b.app.espace3d.vues.rects.first().expect("des vues");
    let view = b.app.espace3d.vues.cadres[i].vue(r.width()).expect("une vue de face");
    assert!(view.is_ortho(), "la vue de face est orthographique");
    let t = trajectoire(&b, 0);
    let o = t.monde_a(t.poignees[0]).unwrap();
    let a_l_ecran_vue = |p: [f32; 3]| -> Pos2 {
        let q = view.project(p, [r.width() / 2.0, r.height() / 2.0]).expect("se voit");
        pos2(r.left() + q[0], r.top() + q[1])
    };
    let (po, pb) = (a_l_ecran_vue(o), a_l_ecran_vue(m::add(o, [0.0, Gizmo::size_for(&view, o) * 0.75, 0.0])));
    assert!(pb.y < po.y - 10.0, "de face, la flèche y monte à l'écran : {po:?} {pb:?}");
    let prise = po + (pb - po) * 0.66;
    assert!(r.shrink(4.0).contains(prise), "{prise:?} hors de la vue {r:?}");
    appuyer(&mut b, &[prise, prise + vec2(0.0, -30.0), prise + vec2(0.0, -60.0)], Modifiers::NONE);
    lacher(&mut b, prise + vec2(0.0, -60.0), Modifiers::NONE);
    let u = trajectoire(&b, 0);
    assert!(u.monde_a(u.poignees[0]).unwrap()[1] > o[1] + 0.2, "la vue de face monte le chemin : {}", b.app.status);
    assert_eq!(derniere_etape(&b), tp::TIRER);
}
