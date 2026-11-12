//! Essais de la Trajectoire dans l'Espace 3D (`trajectoire3d.rs`, `docs/INTERFACE.md` partie 149) : le support (le plan de
//! l'espace où le chemin est posé), le déplacement dans l'espace, le temps des marques ; puis de vrais gestes de souris dans
//! la vue : un chemin sur le sol déplace la carte choisie, plusieurs cartes, la caméra ; marques, second trait, Échap ; le
//! refus d'un plan ordinaire ; l'enregistrement ; et les trois défauts corrigés avec elle (caméra retournée relue, message de
//! la case, silhouettes).

use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::camera::{Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::espace3d::{self, Monde, Xf};
use crate::espace3ddessin::Regard3d;
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath as m;
use crate::perf_tests::{banc_taille, Banc};
use crate::project;
use crate::trajectoire::{Cles, Support, Trajectoire};
use crate::trajectoire3d::{self as t3, cles_dans_l_espace, Sujet};

// ------------------------------------------------------------------------------------------------ le modèle

fn proche(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    m::length(m::sub(a, b)) <= tol
}

/// Un sol à la hauteur −1, une unité du chemin valant un demi-mètre : vers la droite, et vers l'avant (+Z).
fn sol_d_essai() -> Support {
    Support::nouveau([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.5).expect("un plan")
}

/// Un chemin droit de 100 unités sur ce sol, en clés d'interpolation, du départ à l'image 0 à la fin à l'image 10.
fn chemin_au_sol() -> Trajectoire {
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[10.0, 20.0], [110.0, 20.0]], sol_d_essai()).expect("un chemin");
    (t.cles, t.fin) = (Cles::Interpolation, Some(10));
    t
}

#[test]
fn a_support_maps_path_points_to_the_space_and_back_and_refuses_what_is_not_a_plane() {
    // Les axes sont redressés (le second rendu perpendiculaire au premier) et mis à la longueur d'une unité du chemin.
    let s = Support::nouveau([1.0, 2.0, 3.0], [2.0, 0.0, 0.0], [0.5, 0.0, 1.0], 0.5).expect("un plan");
    assert!(proche(s.u, [0.5, 0.0, 0.0], 1e-6) && proche(s.v, [0.0, 0.0, 0.5], 1e-6), "{s:?}");
    assert!(proche(s.monde([4.0, 6.0]), [3.0, 2.0, 6.0], 1e-5));
    assert!(proche(s.deplacement([4.0, 6.0]), [2.0, 0.0, 3.0], 1e-5));
    let p = s.dans_le_plan(s.monde([4.0, 6.0]));
    assert!((p[0] - 4.0).abs() < 1e-4 && (p[1] - 6.0).abs() < 1e-4, "{p:?}");
    assert!(m::dot(s.normale(), [0.0, 1.0, 0.0]).abs() > 0.999, "la normale d'un sol est la verticale : {:?}", s.normale());
    // Un regard qui descend sur le plan le traverse là où il faut ; parallèle, derrière ou à l'horizon : rien.
    let p = s.couper([3.0, 10.0, 6.0], [0.0, -1.0, 0.0]).expect("le regard touche le plan");
    assert!((p[0] - 4.0).abs() < 1e-4 && (p[1] - 6.0).abs() < 1e-4, "{p:?}");
    assert!(s.couper([3.0, 10.0, 6.0], [1.0, 0.0, 0.0]).is_none(), "parallèle au plan");
    assert!(s.couper([3.0, 10.0, 6.0], [0.0, 1.0, 0.0]).is_none(), "le plan est derrière");
    assert!(s.couper([0.0, 2.001, 0.0], [1.0, -1e-9, 0.0]).is_none(), "à l'horizon, le point part à l'infini");
    // Déplacé, il emporte le chemin ; dans un miroir, la gauche et la droite s'échangent.
    assert!(proche(s.deplace([1.0, 1.0, 1.0]).monde([4.0, 6.0]), [4.0, 3.0, 7.0], 1e-5));
    let miroir = s.miroir_x(0.0);
    assert!(proche(miroir.monde([4.0, 6.0]), [-3.0, 2.0, 6.0], 1e-5), "{:?}", miroir.monde([4.0, 6.0]));
    // Ce qui ne fait pas un plan.
    assert!(Support::nouveau([0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], 1.0).is_none(), "deux axes alignés");
    assert!(Support::nouveau([0.0; 3], [0.0; 3], [0.0, 1.0, 0.0], 1.0).is_none(), "un axe nul");
    assert!(Support::nouveau([f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1.0).is_none(), "une origine abîmée");
    assert!(Support::nouveau([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0).is_none(), "une unité nulle");
}

#[test]
fn what_follows_a_path_in_space_moves_by_its_displacement_since_the_start_at_the_pace_of_the_marks() {
    let mut t = chemin_au_sol();
    assert!(!t.sur_la_feuille());
    assert!(Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).expect("un chemin").sur_la_feuille());
    // 100 unités d'un demi-mètre vers la droite : 50 m au bout, la moitié à mi-chemin, rien au départ.
    assert!(proche(t.deplacement_a(0.0).unwrap(), [0.0; 3], 1e-5));
    assert!(proche(t.deplacement_a(0.5).unwrap(), [25.0, 0.0, 0.0], 1e-3));
    assert!(proche(t.deplacement_a(1.0).unwrap(), [50.0, 0.0, 0.0], 1e-3));
    assert!(Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap().deplacement_a(0.5).is_none(), "sur la feuille : aucun déplacement dans l'espace");
    // Dans un plan de 20 images, départ à l'image 0, fin à l'image 10 : à vitesse constante, puis au bout.
    assert!(proche(t.deplacement_a_l_image(5, 20).unwrap(), [25.0, 0.0, 0.0], 1e-3));
    assert!(proche(t.deplacement_a_l_image(10, 20).unwrap(), [50.0, 0.0, 0.0], 1e-3));
    assert!(proche(t.deplacement_a_l_image(19, 20).unwrap(), [50.0, 0.0, 0.0], 1e-3), "après la fin, au bout du chemin");
    t.depart = 4;
    assert!(proche(t.deplacement_a_l_image(2, 20).unwrap(), [0.0; 3], 1e-5), "avant le départ, rien ne bouge");
    assert!(t.deplacement_a_l_image(2, 5).is_none(), "un plan qui s'arrête avant le chemin : rien");
    t.depart = 0;
    // Une marque à 20 % du chemin : deux intervalles de temps égaux, donc 20 % du chemin à la moitié du temps (lent, puis vite).
    assert_eq!(t.ajouter_marque(0.2), Some(0));
    assert!(proche(t.deplacement_a_l_image(5, 20).unwrap(), [10.0, 0.0, 0.0], 1e-2), "{:?}", t.deplacement_a_l_image(5, 20));
    assert_eq!(t.images_et_fractions(20).len(), 11, "clés d'interpolation : une par image du départ à la fin");
    // Clés d'images : ce qui suit saute de jalon en jalon et s'y tient.
    t.cles = Cles::Images;
    assert_eq!(t.images_et_fractions(20), vec![(0, 0.0), (5, 0.2), (10, 1.0)]);
    assert_eq!(t.fraction_suivie(4, 20), Some(0.0));
    assert_eq!(t.fraction_suivie(5, 20), Some(0.2));
    assert_eq!(t.fraction_suivie(9, 20), Some(0.2));
    assert_eq!(t.fraction_suivie(10, 20), Some(1.0));
    // Le temps décalé (le plan a gagné ou perdu des images à son début).
    t.decaler_de(3);
    assert_eq!((t.depart, t.fin), (3, Some(13)));
    t.decaler_de(-5);
    assert_eq!((t.depart, t.fin), (0, Some(8)), "un départ ne passe pas avant la première image");
}

#[test]
fn a_path_in_space_survives_json_and_a_broken_support_is_not_taken_for_a_sheet_path() {
    let mut t = chemin_au_sol();
    t.ajouter_marque(0.4);
    let texte = serde_json::to_string(&t).expect("écrit");
    assert!(texte.contains("\"support\""), "{texte}");
    let relue: Trajectoire = serde_json::from_str(&texte).expect("relu");
    assert_eq!(relue.clone().assainie().as_ref(), Some(&t), "elle revient telle quelle");
    // Une trajectoire sur la feuille n'écrit rien de plus qu'avant.
    let feuille = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    assert!(!serde_json::to_string(&feuille).unwrap().contains("support"));
    // Un support abîmé : la trajectoire ne se lit pas (ses points ne sont pas des pixels de la feuille).
    let mut abimee = t.clone();
    abimee.support = Some(Support { v: [0.0; 3], ..sol_d_essai() });
    assert!(abimee.assainie().is_none());
    let mut abimee = t.clone();
    abimee.support = Some(Support { origine: [f32::INFINITY, 0.0, 0.0], ..sol_d_essai() });
    assert!(abimee.assainie().is_none());
}

#[test]
fn the_keys_of_a_card_move_it_in_the_space_and_keep_its_size_and_its_orientation_there() {
    // Cadre 320 × 180 sur une feuille 640 × 360 ; une carte penchée, décalée, à mi-profondeur.
    let world = Monde { libre: true, ..Monde::new([640, 360], [320, 180]) };
    let mut l = Layer::new("Carte", LayerKind::Bitmap);
    l.transform = LayerTransform { x: 30.0, y: -10.0, scale: 1.5, tilt_y: 25.0, ..LayerTransform::default() };
    l.depth = 0.5;
    let propre = world.carte(&l, 0);
    // Un chemin au sol qui s'éloigne de la caméra d'origine du plan (vers −Z), de 200 unités, puis revient vers la droite.
    let sol = Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k).unwrap();
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[0.0, 0.0], [0.0, -200.0], [150.0, -200.0]], sol).expect("un chemin");
    (t.cles, t.fin) = (Cles::Interpolation, Some(11));
    let cles = cles_dans_l_espace(&world, &l, &t, 12);
    assert_eq!(cles.len(), 12, "une clé par image");
    assert_eq!(cles[0].t, LayerTransform { dz: 0.0, ..l.transform }, "au départ, la carte est à sa place");
    l.pos_keys = cles;
    for f in 0..12 {
        let xf = world.carte(&l, f);
        let d = t.deplacement_a_l_image(f, 12).unwrap();
        assert!(proche(xf.pos, m::add(propre.pos, d), 2e-3), "image {f} : {:?} au lieu de {:?}", xf.pos, m::add(propre.pos, d));
        assert!(proche(xf.size, propre.size, 2e-3), "image {f} : sa taille dans l'espace ne change pas ({:?}, {:?})", xf.size, propre.size);
        assert!(proche(xf.rot, propre.rot, 1e-3), "image {f} : son orientation non plus");
    }
    let bout = t.deplacement_a(1.0).unwrap();
    assert!(proche(bout, [150.0 * world.k, 0.0, -200.0 * world.k], 0.05), "le bout du chemin : {bout:?}");
    assert!(l.depth_libre_at(11) > l.depth_libre_at(0) + 0.5, "elle a reculé : sa profondeur a grandi");
    assert!(l.pose(11).scale < l.pose(0).scale, "et son dessin rapetisse dans l'image pour garder sa taille dans l'espace");
    assert!(l.pos_keys.iter().all(|k| !k.tenue));
    // Clés d'images : une clé tenue par jalon, la carte saute de l'une à l'autre.
    let mut images = t.clone();
    images.cles = Cles::Images;
    images.ajouter_marque(0.5);
    l.pos_keys = cles_dans_l_espace(&world, &l, &images, 12);
    assert_eq!(l.pos_keys.iter().map(|k| (k.frame, k.tenue)).collect::<Vec<_>>(), vec![(0, true), (6, true), (11, true)]);
    assert!(proche(world.carte(&l, 5).pos, propre.pos, 1e-3), "tenue jusqu'à la marque");
    assert!(proche(world.carte(&l, 6).pos, m::add(propre.pos, images.deplacement_a(0.5).unwrap()), 2e-3));
}

// ------------------------------------------------------------------------------------------------ le banc

/// Un projet d'essai : cadre 320 × 180 sur une feuille 640 × 360, un plan de 24 images, `n` calques (« Carte 0 » en bas).
fn doc_cartes(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    doc.panels[0].length = 24;
    for k in 0..n {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        layer.frames = vec![Some(cel); 24];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Le banc, la vue Espace 3D ouverte et cadrée, le calque 0 courant, l'outil Trajectoire en main (clés d'interpolation).
/// `espace` : le plan est un plan en Espace 3D.
fn ouvert(doc: Document, espace: bool) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.run_action("espace3d.basculer");
    if espace {
        b.app.run_action("espace3d.plan_en_espace");
        assert!(b.app.doc.plan_en_espace(0), "{}", b.app.status);
    }
    b.chauffer(2);
    b.app.run_action("espace3d.cadrer");
    b.app.traj.cles_propose = Cles::Interpolation;
    b.app.run_action("outil.trajectoire");
    b.chauffer(3);
    assert!(b.app.espace3d.on && !b.app.camera_view && b.app.tool == Tool::Trajectoire && b.app.e3_trace(), "l'outil a le pointeur dans la vue");
    b
}

fn regard(b: &Banc) -> Regard3d {
    b.app.e3d.regard.expect("la vue a donné son regard")
}

/// Le point de l'écran où l'on voit ce point du monde ; il doit être dans la vue.
fn a_l_ecran(b: &Banc, p: [f32; 3]) -> Pos2 {
    let q = regard(b).ecran(p).unwrap_or_else(|| panic!("le point {p:?} se voit"));
    assert!(b.app.canvas_rect.shrink(4.0).contains(q), "le point {p:?} tombe en {q:?}, hors de la vue {:?}", b.app.canvas_rect);
    q
}

/// Le point de l'écran où l'on voit le point (x, z) du sol.
fn sol(b: &Banc, x: f32, z: f32) -> Pos2 {
    let y = b.app.e3_world().sol_y();
    a_l_ecran(b, [x, y, z])
}

/// Un geste de souris qui passe par ces points de l'écran, touches `mods` tenues : appui au premier, relâché au dernier.
fn geste(b: &mut Banc, pts: &[Pos2], mods: Modifiers) {
    appuyer(b, pts, mods);
    lacher(b, *pts.last().expect("un point au moins"), mods);
}

/// Le début d'un geste : appui au premier point, puis le pointeur passe par les suivants, bouton tenu.
fn appuyer(b: &mut Banc, pts: &[Pos2], mods: Modifiers) {
    b.touches = mods;
    b.image(vec![Event::PointerMoved(pts[0])], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: pts[0], button: PointerButton::Primary, pressed: true, modifiers: mods }], 1.0 / 60.0);
    for w in pts.windows(2) {
        for k in 0..6 {
            let ev = (0..3).map(|j| Event::PointerMoved(w[0] + (w[1] - w[0]) * ((k * 3 + j + 1) as f32 / 18.0))).collect();
            b.image(ev, 1.0 / 60.0);
        }
    }
}

fn lacher(b: &mut Banc, ou: Pos2, mods: Modifiers) {
    b.image(vec![Event::PointerButton { pos: ou, button: PointerButton::Primary, pressed: false, modifiers: mods }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
}

/// Le placement dans l'espace de la carte du calque `li` à l'image `f`.
fn carte(b: &Banc, li: usize, f: usize) -> Xf {
    b.app.e3_world().carte(&b.app.layers()[li], f)
}

fn derniere_etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

fn trajectoire(b: &Banc, li: usize) -> Trajectoire {
    b.app.layers()[li].trajectoire.clone().unwrap_or_else(|| panic!("le calque {li} suit un chemin ({})", b.app.status))
}

/// Le chemin d'essai sur le sol, devant les cartes : de (−2, 1) à (2, 3), soit 4 m vers la droite et 2 m vers l'avant.
fn tracer_au_sol(b: &mut Banc) {
    let (a, c) = (sol(b, -2.0, 1.0), sol(b, 2.0, 3.0));
    geste(b, &[a, c], Modifiers::NONE);
}

const AU_BOUT: [f32; 3] = [4.0, 0.0, 2.0];

#[test]
fn a_path_drawn_on_the_ground_moves_the_chosen_card_in_x_and_z_and_keeps_its_size_in_space() {
    let mut b = ouvert(doc_cartes(1), true);
    assert_eq!(b.app.status, t3::PRISE);
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Carte(b.app.layers()[0].id)], "rien de choisi : la carte du calque courant");
    let (depart, etapes) = (carte(&b, 0, 0), b.app.history.undo_len());
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    let support = t.support.expect("un chemin dans l'espace");
    assert!(m::dot(support.normale(), [0.0, 1.0, 0.0]).abs() > 0.999, "il est posé sur le sol : {support:?}");
    assert!(proche(support.monde(t.point_a(0.0)), [-2.0, b.app.e3_world().sol_y(), 1.0], 0.03), "il part d'où le trait est parti : {:?}", support.monde(t.point_a(0.0)));
    assert!(proche(t.deplacement_a(1.0).unwrap(), AU_BOUT, 0.05), "{:?}", t.deplacement_a(1.0));
    assert_eq!((t.depart, t.fin, t.cles), (0, Some(23), Cles::Interpolation));
    assert_eq!(derniere_etape(&b), "Trajectoire");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape d'annulation");
    // Une clé de position par image ; la carte part d'où elle était et arrive 4 m à droite et 2 m en avant.
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
    assert!(proche(carte(&b, 0, 0).pos, depart.pos, 1e-3), "au départ, elle n'a pas bougé");
    let arrivee = carte(&b, 0, 23);
    assert!(proche(arrivee.pos, m::add(depart.pos, AU_BOUT), 0.05), "à la dernière image : {:?}", arrivee.pos);
    let milieu = carte(&b, 0, 12);
    assert!(proche(milieu.pos, m::add(depart.pos, m::scale(AU_BOUT, 12.0 / 23.0)), 0.05), "à vitesse constante : {:?}", milieu.pos);
    assert!(proche(arrivee.size, depart.size, 2e-3) && proche(arrivee.rot, depart.rot, 1e-3), "sa taille et son orientation dans l'espace restent : {arrivee:?}");
    // Sa profondeur a changé (elle s'est approchée de la caméra d'origine du plan), et son dessin grossit dans l'image.
    let l = &b.app.layers()[0];
    assert!(l.depth_libre_at(23) < l.depth_libre_at(0) - 0.2, "profondeur {} → {}", l.depth_libre_at(0), l.depth_libre_at(23));
    assert!(l.pose(23).scale > l.pose(0).scale);
    // Un simple clic ne fait rien.
    let ailleurs = sol(&b, 0.0, 4.0);
    geste(&mut b, &[ailleurs], Modifiers::NONE);
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    // Ctrl+Z retire le chemin et ses clés ; rétablir les rend.
    b.app.undo();
    assert!(b.app.layers()[0].trajectoire.is_none() && b.app.layers()[0].pos_keys.is_empty());
    assert!(proche(carte(&b, 0, 23).pos, depart.pos, 1e-4));
    b.app.redo();
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
}

#[test]
fn two_chosen_cards_both_follow_the_path_from_where_each_one_is_and_one_undo_removes_both() {
    let mut doc = doc_cartes(3);
    doc.panels[0].layers[1].transform.x = 120.0;
    doc.panels[0].layers[1].depth = 0.4;
    let mut b = ouvert(doc, true);
    let ids: Vec<u64> = b.app.layers().iter().map(|l| l.id).collect();
    b.app.espace3d.sel = vec![espace3d::card_selection(ids[0]), espace3d::card_selection(ids[1])];
    let departs = [carte(&b, 0, 0), carte(&b, 1, 0)];
    let etapes = b.app.history.undo_len();
    tracer_au_sol(&mut b);
    for li in 0..2 {
        let t = trajectoire(&b, li);
        assert!(t.support.is_some() && proche(t.deplacement_a(1.0).unwrap(), AU_BOUT, 0.05));
        let arrivee = carte(&b, li, 23);
        assert!(proche(arrivee.pos, m::add(departs[li].pos, AU_BOUT), 0.05), "la carte {li} : {:?}", arrivee.pos);
        assert!(proche(arrivee.size, departs[li].size, 3e-3), "la carte {li} garde sa taille dans l'espace");
    }
    assert_eq!(trajectoire(&b, 0), trajectoire(&b, 1), "la même trajectoire, une copie pour chacune");
    assert!(b.app.layers()[2].trajectoire.is_none() && b.app.layers()[2].pos_keys.is_empty(), "la carte qui n'est pas choisie ne bouge pas");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule étape pour les deux");
    assert!(b.app.status.contains("2 cartes"), "{}", b.app.status);
    b.app.undo();
    assert!(b.app.layers().iter().all(|l| l.trajectoire.is_none() && l.pos_keys.is_empty()), "une annulation retire les deux");
}

#[test]
fn a_path_drawn_on_a_card_that_is_not_chosen_lies_on_that_card() {
    // La carte 0 est un mur au fond ; la carte 1, choisie, est devant.
    let mut doc = doc_cartes(2);
    doc.panels[0].layers[1].depth = -0.3;
    let mut b = ouvert(doc, true);
    let id = b.app.layers()[1].id;
    b.app.espace3d.sel = vec![espace3d::card_selection(id)];
    let depart = carte(&b, 1, 0);
    // Deux points du mur, au-dessus du sol : le regard touche le mur avant le sol.
    let (a, c) = (a_l_ecran(&b, [3.5, 2.5, 0.0]), a_l_ecran(&b, [-3.0, 1.0, 0.0]));
    geste(&mut b, &[a, c], Modifiers::NONE);
    let t = trajectoire(&b, 1);
    let support = t.support.expect("dans l'espace");
    assert!(m::dot(support.normale(), [0.0, 0.0, 1.0]).abs() > 0.999, "le chemin est posé sur le mur : {support:?}");
    assert!(proche(t.deplacement_a(1.0).unwrap(), [-6.5, -1.5, 0.0], 0.06), "{:?}", t.deplacement_a(1.0));
    let arrivee = carte(&b, 1, 23);
    assert!(proche(arrivee.pos, m::add(depart.pos, [-6.5, -1.5, 0.0]), 0.06), "{:?}", arrivee.pos);
    assert!((arrivee.pos[2] - depart.pos[2]).abs() < 1e-3, "elle reste à sa distance du mur");
    assert!(b.app.layers()[0].trajectoire.is_none(), "le mur, lui, ne bouge pas");
}

#[test]
fn the_chosen_camera_follows_the_path_without_any_camera_key_and_can_still_be_set_at_any_image() {
    let mut b = ouvert(doc_cartes(1), true);
    b.app.espace3d.sel = vec![CAMERA_ID];
    let world = b.app.e3_world();
    let avant: Vec<Camera> = (0..24).map(|f| b.app.doc.panel_camera_reelle(0, f)).collect();
    let etapes = b.app.history.undo_len();
    tracer_au_sol(&mut b);
    assert!(b.app.layers()[0].trajectoire.is_none(), "la carte n'est pas choisie : elle ne reçoit rien");
    let t = b.app.doc.panels[0].trajectoire_camera.clone().expect("la caméra suit un chemin");
    assert!(t.support.is_some() && proche(t.deplacement_a(1.0).unwrap(), AU_BOUT, 0.05));
    assert!(b.app.doc.panels[0].camera.is_empty(), "aucune clé de caméra n'est posée");
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Trajectoire".to_string(), etapes + 1));
    // La caméra telle qu'elle est dans l'espace suit le chemin ; panoramique, bascule, rotation et focale ne changent pas.
    for f in [0usize, 7, 12, 23] {
        let c = b.app.doc.panel_camera_reelle(0, f);
        let d = m::scale(AU_BOUT, f as f32 / 23.0);
        assert!(proche(world.eye(&c), m::add(world.eye(&avant[f]), d), 0.05), "image {f} : {:?}", world.eye(&c));
        assert_eq!((c.pan, c.tilt, c.angle, c.zoom), (avant[f].pan, avant[f].tilt, avant[f].angle, avant[f].zoom));
    }
    // L'image la voit : le plan a son changement de repère, celui de la caméra déplacée.
    assert!(b.app.doc.repere_du_plan(0, 0).is_none(), "au départ, la caméra est encore celle d'origine");
    assert_eq!(b.app.doc.repere_du_plan(0, 23), Some(world.repere_camera(&b.app.doc.panel_camera_reelle(0, 23))));
    // La vue la montre là où le chemin l'a portée, et la régler à n'importe quelle image marche : la clé écrite, portée par le
    // chemin, donne la caméra voulue, ici et aux autres images.
    b.app.cur_frame = 12;
    let ici = b.app.current_camera();
    assert!(proche(world.eye(&ici), world.eye(&b.app.doc.panel_camera_reelle(0, 12)), 1e-3));
    let voulue = Camera { x: ici.x + 40.0, pan: 15.0, ..ici };
    b.app.set_current_camera(voulue);
    let lue = b.app.current_camera();
    assert!((lue.x - voulue.x).abs() < 1e-2 && (lue.y - voulue.y).abs() < 1e-2 && (lue.dolly - voulue.dolly).abs() < 1e-2 && lue.pan == 15.0, "{lue:?} au lieu de {voulue:?}");
    assert_eq!(b.app.doc.panels[0].camera.len(), 1, "une caméra fixe : sa seule clé");
    let fin = b.app.doc.panel_camera_reelle(0, 23);
    assert!(proche(world.eye(&fin), m::add(world.eye(&voulue), m::scale(AU_BOUT, 11.0 / 23.0)), 0.05), "elle continue son chemin depuis là : {:?}", world.eye(&fin));
    // Clés d'images : la caméra saute de jalon en jalon.
    b.app.doc.panels[0].trajectoire_camera.as_mut().unwrap().cles = Cles::Images;
    let d = |b: &Banc, f: usize| m::sub(world.eye(&b.app.doc.panel_camera_reelle(0, f)), world.eye(&b.app.doc.panel_camera_reelle(0, 0)));
    assert!(proche(d(&b, 22), [0.0; 3], 1e-3) && proche(d(&b, 23), AU_BOUT, 0.05), "tenue au départ, puis au bout : {:?} {:?}", d(&b, 22), d(&b, 23));
    // Une annulation de tout : plus de chemin, la caméra d'avant.
    while b.app.history.undo_len() > etapes {
        b.app.undo();
    }
    assert!(b.app.doc.panels[0].trajectoire_camera.is_none());
    assert_eq!(b.app.doc.panel_camera_reelle(0, 23), avant[23]);
}

#[test]
fn a_card_and_the_camera_chosen_together_share_one_path_and_one_undo_step() {
    let mut b = ouvert(doc_cartes(2), true);
    let id = b.app.layers()[1].id;
    b.app.espace3d.sel = vec![espace3d::card_selection(id), CAMERA_ID];
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Carte(id), Sujet::Camera]);
    let etapes = b.app.history.undo_len();
    // (La carte 0 n'est pas choisie : le trait part sur le sol, devant elle.)
    tracer_au_sol(&mut b);
    assert!(trajectoire(&b, 1).support.is_some());
    assert_eq!(b.app.doc.panels[0].trajectoire_camera.as_ref(), Some(&trajectoire(&b, 1)));
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert!(b.app.status.contains("1 carte et la caméra"), "{}", b.app.status);
    // « Effacer » des réglages : pour tout ce qui est choisi, caméra comprise, en une étape.
    b.app.e3_traj_retirer(&[Sujet::Carte(id), Sujet::Camera], true);
    assert!(b.app.layers()[1].trajectoire.is_none() && b.app.layers()[1].pos_keys.is_empty() && b.app.doc.panels[0].trajectoire_camera.is_none());
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Effacer la trajectoire".to_string(), etapes + 2));
    b.app.undo();
    assert!(b.app.layers()[1].trajectoire.is_some() && b.app.doc.panels[0].trajectoire_camera.is_some());
    // « Détacher » : le mouvement reste, en clés ; pour la caméra, une clé par image, et la même caméra à chaque image.
    let cameras: Vec<Camera> = (0..24).map(|f| b.app.doc.panel_camera_reelle(0, f)).collect();
    let cles = b.app.layers()[1].pos_keys.clone();
    b.app.e3_traj_retirer(&[Sujet::Carte(id), Sujet::Camera], false);
    assert!(b.app.layers()[1].trajectoire.is_none() && b.app.doc.panels[0].trajectoire_camera.is_none());
    assert_eq!(b.app.layers()[1].pos_keys, cles);
    assert_eq!(b.app.doc.panels[0].camera.len(), 24);
    let world = b.app.e3_world();
    for (f, c) in cameras.iter().enumerate() {
        assert!(proche(world.eye(&b.app.doc.panel_camera_reelle(0, f)), world.eye(c), 1e-3), "image {f} : la caméra n'a pas bougé en devenant des clés");
    }
}

#[test]
fn marks_change_the_timing_a_second_stroke_redraws_and_escape_cancels_a_gesture() {
    let mut b = ouvert(doc_cartes(1), true);
    let y = b.app.e3_world().sol_y();
    // (La vue s'approche du sol : les gestes fins, une marque, une prise, se font de près.)
    let vue = b.app.espace3d.cam.as_mut().expect("la vue");
    (vue.target, vue.distance) = ([0.0, y, 2.5], vue.distance * 0.4);
    b.chauffer(2);
    tracer_au_sol(&mut b);
    let depart = carte(&b, 0, 0);
    // Le chemin va de A (−2, 1) à B (2, 3) ; un petit trait en travers (50 pixels d'écran), à 15 % du chemin : une marque.
    let sur = |f: f32| [-2.0 + 4.0 * f, y, 1.0 + 2.0 * f];
    let marque = |b: &Banc, f: f32| {
        let (c, sens) = (a_l_ecran(b, sur(f)), (a_l_ecran(b, sur(f + 0.02)) - a_l_ecran(b, sur(f - 0.02))).normalized());
        [c - sens.rot90() * 25.0, c + sens.rot90() * 25.0]
    };
    let long = a_l_ecran(&b, sur(0.0)).distance(a_l_ecran(&b, sur(1.0)));
    assert!(long > 120.0, "le chemin est assez long à l'écran pour qu'un trait de 50 pixels soit une marque : {long} pixels");
    let etapes = b.app.history.undo_len();
    let coup = marque(&b, 0.15);
    geste(&mut b, &coup, Modifiers::NONE);
    let t = trajectoire(&b, 0);
    assert_eq!(t.marques.len(), 1, "{}", b.app.status);
    let ou = t.marques[0];
    assert!((ou - 0.15).abs() < 0.02, "à 15 % du chemin : {ou}");
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Marque de trajectoire".to_string(), etapes + 1));
    // Deux intervalles de temps égaux, la marque à la moitié du temps (image 12,5) : à l'image 11, la carte n'a pas encore
    // fait les 15 % du chemin (lent au départ) ; à vitesse constante elle en aurait fait près de la moitié.
    let vers_11 = m::sub(carte(&b, 0, 11).pos, depart.pos);
    assert!(proche(vers_11, m::scale(AU_BOUT, ou * (11.0 / 23.0) / 0.5), 0.05), "lent au départ : {vers_11:?}");
    assert!(proche(m::sub(carte(&b, 0, 23).pos, depart.pos), AU_BOUT, 0.05));
    // Sur la timeline, glisser la clé de la marque la recale dans le temps : les clés de la carte sont reposées.
    let jalons = t.images_des_jalons(24);
    assert!(b.app.trajectoire_recaler(0, jalons[1], 4));
    assert_eq!(trajectoire(&b, 0).images_des_jalons(24), vec![0, 4, 23]);
    assert!(proche(m::sub(carte(&b, 0, 4).pos, depart.pos), m::scale(AU_BOUT, ou), 0.05), "la marque est atteinte à l'image 5");
    // Glisser la marque le long du chemin la déplace ; la tirer loin du chemin la retire.
    let prise = a_l_ecran(&b, sur(trajectoire(&b, 0).marques[0]));
    let plus_loin = a_l_ecran(&b, sur(0.6));
    geste(&mut b, &[prise, plus_loin], Modifiers::NONE);
    let glissee = trajectoire(&b, 0);
    assert!((glissee.marques[0] - 0.6).abs() < 0.03, "{}", glissee.marques[0]);
    assert_eq!(derniere_etape(&b), "Glisser une marque");
    // Échap pendant qu'on tire le chemin : tout revient, sans étape d'annulation.
    let (avant, cles_avant, etapes) = (trajectoire(&b, 0), b.app.layers()[0].pos_keys.clone(), b.app.history.undo_len());
    let pris = a_l_ecran(&b, sur(0.3));
    appuyer(&mut b, &[pris, pris + eframe::egui::vec2(10.0, 60.0)], Modifiers::NONE);
    assert_ne!(trajectoire(&b, 0), avant, "le chemin suit pendant le geste");
    assert_ne!(b.app.layers()[0].pos_keys, cles_avant, "et la carte avec lui");
    assert!(b.app.trajectoire_touche(Key::Escape));
    assert_eq!(b.app.status, t3::ABANDON);
    lacher(&mut b, pris + eframe::egui::vec2(10.0, 60.0), Modifiers::NONE);
    assert_eq!((trajectoire(&b, 0), b.app.layers()[0].pos_keys.clone(), b.app.history.undo_len()), (avant.clone(), cles_avant, etapes), "Échap : comme avant");
    // Tirer le chemin pour de bon : il se tord, ses bouts restent, une étape.
    appuyer(&mut b, &[pris, pris + eframe::egui::vec2(10.0, 60.0)], Modifiers::NONE);
    lacher(&mut b, pris + eframe::egui::vec2(10.0, 60.0), Modifiers::NONE);
    let tire = trajectoire(&b, 0);
    assert_eq!(derniere_etape(&b), "Tirer la trajectoire");
    assert!(tire.longueur() > avant.longueur() + 2.0, "il a une bosse : {} > {}", tire.longueur(), avant.longueur());
    assert!(crate::trajectoire::distance(tire.point_a(1.0), avant.point_a(1.0)) < 1.0, "son bout n'a pas bougé");
    // Un second trait long, ailleurs sur le sol, retrace le chemin : la marque et le temps restent.
    let (a, c) = (sol(&b, -2.5, 4.0), sol(&b, 2.5, 2.0));
    geste(&mut b, &[a, c], Modifiers::NONE);
    let retrace = trajectoire(&b, 0);
    assert_eq!(derniere_etape(&b), "Retracer la trajectoire");
    assert_eq!((retrace.marques.clone(), retrace.temps.clone(), retrace.depart, retrace.fin), (tire.marques, tire.temps, tire.depart, tire.fin));
    assert!(proche(retrace.deplacement_a(1.0).unwrap(), [5.0, 0.0, -2.0], 0.06), "{:?}", retrace.deplacement_a(1.0));
    assert!(proche(carte(&b, 0, 0).pos, depart.pos, 1e-3), "la carte part toujours d'où elle était");
    assert!(proche(m::sub(carte(&b, 0, 23).pos, depart.pos), [5.0, 0.0, -2.0], 0.06));
    // Tirée loin du chemin, la marque est arrachée.
    let prise = a_l_ecran(&b, retrace.support.unwrap().monde(retrace.point_a(retrace.marques[0])));
    geste(&mut b, &[prise, prise + eframe::egui::vec2(0.0, -90.0)], Modifiers::NONE);
    assert!(trajectoire(&b, 0).marques.is_empty(), "{}", b.app.status);
    assert_eq!(derniere_etape(&b), "Retirer une marque");
}

#[test]
fn moving_a_card_that_follows_a_path_moves_its_own_pose_at_every_image_and_the_path_with_it() {
    let mut b = ouvert(doc_cartes(1), true);
    tracer_au_sol(&mut b);
    let avant: Vec<Xf> = (0..24).map(|f| carte(&b, 0, f)).collect();
    let chemin = trajectoire(&b, 0);
    // À l'image 12, le manipulateur pose la carte 1 m plus haut et 0,5 m plus loin.
    b.app.cur_frame = 12;
    let id = b.app.layers()[0].id;
    let glisse = [0.0, 1.0, -0.5];
    let voulu = Xf { pos: m::add(avant[12].pos, glisse), ..avant[12] };
    b.app.e3_set_xfs(&[(espace3d::card_selection(id), voulu)]);
    for f in [0usize, 5, 12, 23] {
        let xf = carte(&b, 0, f);
        assert!(proche(xf.pos, m::add(avant[f].pos, glisse), 5e-3), "image {f} : {:?}", xf.pos);
        assert!(proche(xf.size, avant[f].size, 3e-3), "image {f} : même taille dans l'espace");
    }
    let apres = trajectoire(&b, 0);
    assert_eq!(apres.points, chemin.points, "le chemin garde sa forme");
    assert!(proche(apres.support.unwrap().origine, m::add(chemin.support.unwrap().origine, glisse), 5e-3), "et suit la carte");
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
}

#[test]
fn an_ordinary_plan_refuses_with_its_message_and_the_sheet_only_shows_a_path_in_space() {
    // Plan ordinaire : la vue répond qu'il faut un plan en Espace 3D, rien n'est posé.
    let mut b = ouvert(doc_cartes(1), false);
    assert_eq!(b.app.status, t3::PLAN_ORDINAIRE);
    let etapes = b.app.history.undo_len();
    b.app.status.clear();
    let (a, c) = (sol(&b, -2.0, 1.0), sol(&b, 2.0, 3.0));
    geste(&mut b, &[a, c], Modifiers::NONE);
    assert_eq!(b.app.status, t3::PLAN_ORDINAIRE);
    assert!(b.app.layers()[0].trajectoire.is_none() && b.app.doc.panels[0].trajectoire_camera.is_none());
    assert_eq!(b.app.history.undo_len(), etapes);
    // Alt+glisser tourne toujours la vue avec l'outil en main.
    let vue = b.app.espace3d.cam.expect("la vue");
    geste(&mut b, &[a, c], Modifiers::ALT);
    assert_ne!(b.app.espace3d.cam.expect("la vue").azimuth, vue.azimuth, "Alt+glisser tourne la vue");
    // Un outil de la vue rend le pointeur aux cartes ; reprendre l'outil Trajectoire le lui redonne.
    b.app.run_action("espace3d.outil_deplacer");
    assert!(!b.app.e3_trace());
    b.app.run_action("outil.trajectoire");
    assert!(b.app.e3_trace());

    // Plan en Espace 3D : un chemin dans l'espace ne se prend pas sur la feuille, la barre d'état dit où le retoucher.
    let mut b = ouvert(doc_cartes(1), true);
    tracer_au_sol(&mut b);
    let (avant, etapes) = (trajectoire(&b, 0), b.app.history.undo_len());
    b.app.run_action("espace3d.basculer");
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.set_tool(Tool::Trajectoire);
    b.chauffer(6);
    assert!(!b.app.espace3d.on && b.app.tool == Tool::Trajectoire, "retour sur la feuille, l'outil en main");
    b.app.status.clear();
    let centre = b.app.canvas_rect.center();
    geste(&mut b, &[centre, centre + eframe::egui::vec2(120.0, 30.0)], Modifiers::NONE);
    assert_eq!(b.app.status, t3::SUR_LA_FEUILLE);
    assert_eq!((trajectoire(&b, 0), b.app.history.undo_len()), (avant, etapes), "rien n'a changé");
    // Décocher « Plan en Espace 3D » : la trajectoire dans l'espace est détachée, ses clés restent ; une annulation rend tout.
    b.app.run_action("espace3d.plan_en_espace");
    assert!(!b.app.doc.plan_en_espace(0));
    assert!(b.app.layers()[0].trajectoire.is_none() && b.app.layers()[0].pos_keys.len() == 24);
    b.app.undo();
    assert!(b.app.doc.plan_en_espace(0) && b.app.layers()[0].trajectoire.is_some());
}

// ------------------------------------------------------------------------------------------------ le fichier

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire3d-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

#[test]
fn both_kinds_of_paths_in_space_survive_a_save_and_a_reload_and_a_format_54_project_still_loads() {
    let mut b = ouvert(doc_cartes(2), true);
    let id = b.app.layers()[1].id;
    b.app.espace3d.sel = vec![espace3d::card_selection(id), CAMERA_ID];
    tracer_au_sol(&mut b);
    let (carte_avant, camera_avant) = (trajectoire(&b, 1), b.app.doc.panels[0].trajectoire_camera.clone().expect("la caméra suit un chemin"));
    let cameras: Vec<Camera> = (0..24).map(|f| b.app.doc.panel_camera_reelle(0, f)).collect();
    let dir = dossier("fichier");
    let manifest = dir.join("T.shot");
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    let texte = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(texte.contains("\"support\"") && texte.contains("\"trajectoire_camera\""), "les deux champs sont écrits");
    assert!(texte.contains(&format!("\"version\": {}", project::VERSION)) && project::VERSION == 55, "sous le format 55, sans changer de numéro");
    let relu = project::load(&manifest).expect("relu").doc;
    assert_eq!(relu.panels[0].layers[1].trajectoire.as_ref(), Some(&carte_avant), "la trajectoire de la carte revient telle quelle");
    assert_eq!(relu.panels[0].trajectoire_camera.as_ref(), Some(&camera_avant), "celle de la caméra aussi");
    assert_eq!(relu.panels[0].layers[1].pos_keys.len(), 24);
    for (f, c) in cameras.iter().enumerate() {
        assert_eq!(relu.panel_camera_reelle(0, f), *c, "image {f} : la même caméra après la relecture");
    }
    // Sans trajectoire dans l'espace, rien de nouveau n'est écrit ; ce même projet, numéroté 54, s'ouvre comme avant.
    b.app.e3_traj_retirer(&[Sujet::Carte(id), Sujet::Camera], true);
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    let texte = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(!texte.contains("\"support\"") && !texte.contains("\"trajectoire_camera\"") && !texte.contains("\"trajectoire\""));
    std::fs::write(&manifest, texte.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 54")).expect("écrit");
    let ancien = project::load(&manifest).expect("un projet du format 54 s'ouvre").doc;
    assert!(ancien.panels[0].espace && ancien.panels[0].trajectoire_camera.is_none());
    assert!(ancien.panels[0].layers.iter().all(|l| l.trajectoire.is_none()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_turned_round_camera_of_a_plan_in_space_comes_back_as_it_was_and_an_ordinary_plan_keeps_its_bound() {
    let dir = dossier("camera");
    let manifest = dir.join("C.shot");
    let mut doc = doc_cartes(1);
    doc.panels[0].espace = true;
    let retournee = Camera { pan: 172.0, tilt: -88.0, ..doc.default_camera() };
    let loin = Camera { pan: -400.0, tilt: 90.0, ..doc.default_camera() };
    doc.panels[0].camera = vec![CameraKey::new(0, retournee), CameraKey::new(12, loin)];
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let relu = project::load(&manifest).expect("relu").doc;
    assert_eq!(relu.panels[0].camera, doc.panels[0].camera, "plan en Espace 3D : la caméra retournée revient comme elle était");
    assert_eq!(relu.panel_camera_reelle(0, 0), retournee);
    // Le même fichier, la case en moins : un plan ordinaire, dont la caméra doit voir la feuille. Sa borne de 85° reste.
    let texte = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(texte.contains("\"espace\": true"));
    std::fs::write(&manifest, texte.replace("\"espace\": true", "\"espace\": false")).expect("écrit");
    let ordinaire = project::load(&manifest).expect("relu").doc;
    assert!(!ordinaire.panels[0].espace);
    let c = ordinaire.panels[0].camera[0].cam;
    assert_eq!((c.pan, c.tilt), (85.0, -85.0), "plan ordinaire : bascule et panoramique s'arrêtent à 85°");
    assert_eq!(project::bornes_de_camera(true, 95.0, 4000.0), (90.0, 3600.0), "plan en Espace 3D : la verticale au plus, et des tours sans fin raisonnable");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ la caméra et le montage

/// Un plan en Espace 3D de 24 images dont la caméra suit, en clés d'interpolation, un chemin au sol de 4 m vers la droite et
/// 2 m vers l'avant, de l'image `depart` à l'image `fin`.
fn doc_camera_sur_un_chemin(depart: usize, fin: usize) -> Document {
    let mut doc = doc_cartes(1);
    doc.panels[0].espace = true;
    let world = Monde::du_plan(&doc, 0);
    let support = Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k).unwrap();
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[0.0, 0.0], [4.0 / world.k, 2.0 / world.k]], support).expect("un chemin");
    (t.cles, t.depart, t.fin) = (Cles::Interpolation, depart, Some(fin));
    doc.panels[0].trajectoire_camera = Some(t);
    doc
}

fn yeux(doc: &Document, pi: usize) -> Vec<[f32; 3]> {
    let world = Monde::du_plan(doc, pi);
    (0..doc.panels[pi].frame_count()).map(|f| world.eye(&doc.panel_camera_reelle(pi, f))).collect()
}

#[test]
fn cutting_trimming_and_inserting_images_keep_the_camera_where_its_path_put_it() {
    // Écrite dans les clés : la même caméra à chaque image, des clés seulement là où le chemin la porte.
    let mut doc = doc_camera_sur_un_chemin(4, 16);
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { pan: 10.0, ..doc.default_camera() }), CameraKey::new(20, Camera { pan: 30.0, x: 400.0, ..doc.default_camera() })];
    let avant = yeux(&doc, 0);
    assert!(proche(m::sub(avant[23], avant[0]), m::add([4.0, 0.0, 2.0], [80.0 * Monde::of(&doc).k, 0.0, 0.0]), 0.02), "le chemin s'ajoute au mouvement des clés : {:?}", m::sub(avant[23], avant[0]));
    let pans: Vec<f32> = (0..24).map(|f| doc.panel_camera_reelle(0, f).pan).collect();
    assert!(doc.cuire_trajectoires_camera(&[0]));
    assert!(doc.panels[0].trajectoire_camera.is_none());
    assert_eq!(doc.panels[0].camera.len(), 21, "une clé par image de la clé d'avant le départ à la clé d'après la fin");
    for (f, (oeil, pan)) in avant.iter().zip(&pans).enumerate() {
        let c = doc.panel_camera_reelle(0, f);
        assert!(proche(Monde::of(&doc).eye(&c), *oeil, 2e-3) && (c.pan - pan).abs() < 1e-3, "image {f} : la caméra n'a pas bougé en devenant des clés");
    }
    assert!(!doc.cuire_trajectoires_camera(&[0]), "plus rien à écrire");

    // Couper le plan : chaque moitié garde sa caméra, image pour image.
    let mut doc = doc_camera_sur_un_chemin(0, 23);
    let avant = yeux(&doc, 0);
    let second = doc.split_panel(0, 10).expect("coupé");
    assert!(doc.panels[0].trajectoire_camera.is_none() && doc.panels[second].trajectoire_camera.is_none());
    let (a, b) = (yeux(&doc, 0), yeux(&doc, second));
    for f in 0..24 {
        let ici = if f < 10 { a[f] } else { b[f - 10] };
        assert!(proche(ici, avant[f], 2e-3), "image {f} : {ici:?} au lieu de {:?}", avant[f]);
    }

    // Raccourcir le plan par la fin ne resserre pas le chemin : la caméra garde son mouvement, et le rallonger rend la suite.
    let mut doc = doc_camera_sur_un_chemin(0, 23);
    let avant = yeux(&doc, 0);
    assert_eq!(doc.trim_tail(0, -6), -6);
    assert!(doc.panels[0].trajectoire_camera.is_some(), "elle reste un chemin");
    let court = yeux(&doc, 0);
    assert_eq!(court.len(), 18);
    for f in 0..18 {
        assert!(proche(court[f], avant[f], 1e-4), "image {f} : le mouvement n'a pas changé");
    }
    assert_eq!(doc.trim_tail(0, 6), 6);
    assert!(yeux(&doc, 0).iter().zip(&avant).all(|(a, b)| proche(*a, *b, 1e-4)));

    // Rogner le début : un chemin qui commence plus tard recule avec les images ; rogné au-delà de son départ, il devient des clés.
    let mut doc = doc_camera_sur_un_chemin(6, 20);
    let avant = yeux(&doc, 0);
    assert_eq!(doc.trim_head(0, 4), 4);
    let t = doc.panels[0].trajectoire_camera.as_ref().expect("elle reste un chemin");
    assert_eq!((t.depart, t.fin), (2, Some(16)));
    assert!(yeux(&doc, 0).iter().enumerate().all(|(f, o)| proche(*o, avant[f + 4], 1e-4)));
    assert_eq!(doc.trim_head(0, -4), -4);
    assert!(yeux(&doc, 0).iter().zip(&avant).all(|(a, b)| proche(*a, *b, 1e-4)), "et revient avec elles");
    assert_eq!(doc.trim_head(0, 9), 9);
    assert!(doc.panels[0].trajectoire_camera.is_none(), "rogné au-delà de son départ : des clés");
    assert!(yeux(&doc, 0).iter().enumerate().all(|(f, o)| proche(*o, avant[f + 9], 2e-3)), "sans changer l'image");

    // Insérer des images dans le plan : le départ et la fin du chemin restent sur leurs images.
    let mut doc = doc_camera_sur_un_chemin(6, 20);
    crate::images::shift_panel(&mut doc.panels[0], 2, 3, false);
    let t = doc.panels[0].trajectoire_camera.as_ref().unwrap();
    assert_eq!((t.depart, t.fin), (9, Some(23)));
}

// ------------------------------------------------------------------------------------------------ les défauts corrigés avec elle

#[test]
fn the_plan_in_space_toggle_no_longer_says_it_is_under_construction() {
    let mut b = banc_taille(doc_cartes(1), false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.app.run_action("espace3d.plan_en_espace");
    assert!(b.app.doc.plan_en_espace(0));
    assert!(!b.app.status.contains("En construction") && !b.app.status.contains("ne change pas encore"), "{}", b.app.status);
    assert!(b.app.status.contains("caméra est libre"), "{}", b.app.status);
}

#[test]
fn the_ghosts_of_a_sheet_path_are_placed_through_the_camera_of_a_plan_in_space() {
    // Un plan en Espace 3D dont la caméra est tournée ; le calque suit une trajectoire tracée sur la feuille, en clés
    // d'interpolation : avec l'outil, ses silhouettes aux jalons sont posées comme le calque lui-même, par le repère de la caméra.
    let mut doc = doc_cartes(1);
    doc.panels[0].espace = true;
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { pan: 20.0, ..doc.default_camera() })];
    let mut t = Trajectoire::nouvelle(&[[100.0, 180.0], [400.0, 180.0]]).expect("un chemin");
    (t.cles, t.fin) = (Cles::Interpolation, Some(23));
    doc.panels[0].layers[0].trajectoire = Some(t);
    let mut b = banc_taille(doc, false, 1280, 720);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.trajectoire_recalculer(0);
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
    b.app.set_tool(Tool::Trajectoire);
    b.chauffer(2);
    let plan = b.app.build_plan(0, true, true);
    let fantomes: Vec<_> = plan.items.iter().filter(|i| i.tint == Some(crate::trajectoireui::FANTOME)).collect();
    assert_eq!(fantomes.len(), 1, "la silhouette de l'arrivée (celle du départ est l'image courante)");
    let world = Monde::of(&b.app.doc);
    let repere = b.app.doc.repere_du_plan(0, 0).expect("la caméra tournée a son repère");
    let voulue = espace3d::place_dans(&world, &b.app.layers()[0], 23, &b.app.doc.camera_seen(0, 0), Some(&repere)).expect("la carte se voit");
    assert!(voulue.pre.is_some(), "par le repère, une carte se pose par un maillage");
    assert_eq!((fantomes[0].transform, &fantomes[0].pre), (voulue.transform, &voulue.pre), "la silhouette est posée par le repère de la caméra du plan");
    let sans_repere = espace3d::place(&world, &b.app.layers()[0], 23, &b.app.doc.camera_seen(0, 0)).expect("posée");
    assert_ne!(sans_repere.pre, fantomes[0].pre, "et non plus comme si la caméra n'avait pas tourné");
}
