//! Essais du chemin continué avec Maj (`trajectoireprolonge.rs`, `Trajectoire::prolonger`, `docs/INTERFACE.md` partie
//! 151) : le modèle (devant, derrière, marques à leur place, point de référence, lissage), le vrai geste sur la feuille
//! (une étape, Échap, Maj ailleurs déplace toujours en bloc) et dans l'Espace 3D (sur le plan du chemin, pour ce qui est
//! choisi).

use eframe::egui::{pos2, Event, Key, Modifiers, PointerButton, Pos2};

use crate::document::{Document, Layer, LayerKind};
use crate::espace3dmath as m;
use crate::perf_tests::{banc_taille, Banc};
use crate::trajectoire::{distance, Cles, Support, Trajectoire};
use crate::trajectoireui_tests::{banc, marquer_le_milieu, tracer_le_chemin};

fn proche2(a: [f32; 2], b: [f32; 2], tol: f32) -> bool {
    distance(a, b) <= tol
}

#[test]
fn prolonger_par_la_fin_ajoute_derriere_et_les_marques_restent_a_leur_place() {
    let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    t.ajouter_marque(0.5);
    t.temps = Some(vec![0.8]);
    assert!(t.prolonger(&[[103.0, 0.0], [200.0, 0.0]], false));
    assert!((t.longueur() - 200.0).abs() < 1.0, "{}", t.longueur());
    assert!(proche2(t.points[0], [0.0, 0.0], 1e-4), "le début ne bouge pas");
    assert!(proche2(*t.points.last().unwrap(), [200.0, 0.0], 1e-3));
    assert_eq!(t.marques.len(), 1);
    assert!(proche2(t.point_a(t.marques[0]), [50.0, 0.0], 1.0), "la marque garde sa place : {:?}", t.point_a(t.marques[0]));
    assert_eq!((t.temps.clone(), t.reference), (None, None), "le temps se répartit de nouveau ; la référence reste le départ");
    // Un trait trop court ne prolonge rien.
    let avant = t.clone();
    assert!(!t.prolonger(&[[201.0, 0.0]], false));
    assert_eq!(t, avant);
}

#[test]
fn prolonger_par_le_debut_ajoute_devant_et_ce_qui_suit_passe_toujours_au_meme_endroit() {
    let sol = Support::nouveau([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.5).unwrap();
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[0.0, 0.0], [100.0, 0.0]], sol).unwrap();
    t.ajouter_marque(0.5);
    let avant_marque = t.deplacement_a(0.5).unwrap();
    let avant_bout = t.deplacement_a(1.0).unwrap();
    assert!(t.prolonger(&[[-3.0, 0.0], [-100.0, 0.0]], true));
    assert!(proche2(t.points[0], [-100.0, 0.0], 1e-3), "le chemin commence au bout du trait : {:?}", t.points[0]);
    assert!(proche2(*t.points.last().unwrap(), [100.0, 0.0], 1e-4), "la fin ne bouge pas");
    assert_eq!(t.reference, Some([0.0, 0.0]), "l'ancien départ");
    assert!(proche2(t.point_a(t.marques[0]), [50.0, 0.0], 1.0));
    // Ce qui suit le chemin : au nouveau départ, 50 m derrière ; à la marque et au bout, là où il passait.
    assert!(m::length(m::sub(t.deplacement_a(0.0).unwrap(), [-50.0, 0.0, 0.0])) < 1e-2, "{:?}", t.deplacement_a(0.0));
    assert!(m::length(m::sub(t.deplacement_a(t.marques[0]).unwrap(), avant_marque)) < 0.6);
    assert!(m::length(m::sub(t.deplacement_a(1.0).unwrap(), avant_bout)) < 1e-2);
    // Le point de référence part et revient dans le fichier ; un chemin retracé l'oublie.
    let relue: Trajectoire = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(relue.assainie().as_ref(), Some(&t));
    let mut retrace = t.clone();
    assert!(retrace.retracer(&[[0.0, 0.0], [0.0, 80.0]]));
    assert_eq!(retrace.reference, None);
}

#[test]
fn un_chemin_lisse_prolonge_lisse_aussi_le_morceau_ajoute() {
    let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    t.lisser_a(0.7);
    let zigzag: Vec<[f32; 2]> = (1..=100).map(|k| [100.0 + 2.0 * k as f32, if (k / 5) % 2 == 0 { -10.0 } else { 10.0 }]).collect();
    assert!(t.prolonger(&zigzag, false));
    assert_eq!(t.lissage, 0.7);
    let brut = t.brut.clone().expect("le trait brut, prolongé");
    assert!(crate::trajectoire::longueur_de(&brut) > 290.0);
    assert_eq!(t.points, crate::trajectoire::lissage_de(&brut, 0.7));
    let ecart = |p: &[[f32; 2]]| p.iter().filter(|q| q[0] > 180.0 && q[0] < 270.0).map(|q| q[1].abs()).fold(0.0, f32::max);
    assert!(ecart(&t.points) < ecart(&brut) * 0.5, "le morceau ajouté est lissé : {} contre {}", ecart(&t.points), ecart(&brut));
}

// ------------------------------------------------------------------------------------------------ sur la feuille

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

/// Un geste de l'outil par ces points de la feuille, touches `mods` tenues ; `lacher` faux : le bouton reste appuyé.
fn geste(b: &mut Banc, pts: &[(f32, f32)], mods: Modifiers, lacher: bool) {
    b.touches = mods;
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p0, button: PointerButton::Primary, pressed: true, modifiers: mods }], 1.0 / 60.0);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    }
    if lacher {
        let (x, y) = pts[pts.len() - 1];
        let fin = ecran(b, x, y);
        b.image(vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: mods }], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

fn derniere_etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

#[test]
fn sur_la_feuille_maj_depuis_la_fin_prolonge_le_chemin_en_une_etape() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    marquer_le_milieu(&mut b);
    let avant = b.app.layers()[0].trajectoire.clone().unwrap();
    let cles_avant = b.app.layers()[0].pos_keys.clone();
    let etapes = b.app.history.names().0.len();
    geste(&mut b, &[(350.0, 150.0), (370.0, 150.0), (390.0, 150.0), (390.0, 120.0), (390.0, 100.0)], Modifiers::SHIFT, true);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    // (90 pixels de trait, dont un coin que le petit lissage du tracé arrondit.)
    assert!(t.longueur() > avant.longueur() + 70.0 && t.longueur() < avant.longueur() + 92.0, "{} : {}", t.longueur(), b.app.status);
    assert!(proche2(t.points[0], avant.points[0], 1e-3), "le début ne bouge pas");
    assert!(proche2(*t.points.last().unwrap(), [390.0, 100.0], 3.0), "{:?}", t.points.last());
    assert!(proche2(t.point_a(t.marques[0]), avant.point_a(avant.marques[0]), 2.0), "la marque garde sa place");
    assert_eq!(b.app.history.names().0.len(), etapes + 1);
    assert_eq!(derniere_etape(&b), "Prolonger la trajectoire");
    // Le calque suit le chemin prolongé jusqu'au bout : sa dernière clé est au bout du trait.
    let cles = &b.app.layers()[0].pos_keys;
    let (k0, kn) = (cles[0].t, cles.last().unwrap().t);
    assert!((k0.x - cles_avant[0].t.x).abs() < 1e-3, "au départ, à la même place");
    assert!((kn.x - k0.x - (390.0 - 50.0)).abs() < 3.0 && (kn.y - k0.y - (100.0 - 150.0)).abs() < 3.0, "{k0:?} → {kn:?}");
    b.app.undo();
    assert_eq!(b.app.layers()[0].trajectoire.as_ref(), Some(&avant));
    assert_eq!(b.app.layers()[0].pos_keys, cles_avant);
}

#[test]
fn sur_la_feuille_maj_depuis_le_debut_ajoute_devant_et_echap_abandonne() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    let avant = b.app.layers()[0].trajectoire.clone().unwrap();
    let cles_avant = b.app.layers()[0].pos_keys.clone();
    // Échap pendant le trait : rien ne change.
    geste(&mut b, &[(50.0, 150.0), (30.0, 150.0), (10.0, 150.0)], Modifiers::SHIFT, false);
    assert!(b.app.trajectoire_touche(Key::Escape));
    let fin = ecran(&b, 10.0, 150.0);
    b.image(vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::SHIFT }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.layers()[0].trajectoire.as_ref(), Some(&avant));
    // Depuis le début : devant.
    geste(&mut b, &[(50.0, 150.0), (30.0, 150.0), (10.0, 150.0)], Modifiers::SHIFT, true);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!(proche2(t.points[0], [10.0, 150.0], 3.0), "{:?} : {}", t.points[0], b.app.status);
    assert_eq!(t.reference, Some(avant.points[0]));
    // Au départ (même image), le calque est 40 pixels plus à gauche ; à la fin, au même endroit qu'avant.
    let cles = &b.app.layers()[0].pos_keys;
    assert!((cles[0].t.x - (cles_avant[0].t.x - 40.0)).abs() < 3.0, "{:?} contre {:?}", cles[0].t, cles_avant[0].t);
    let (a, z) = (cles.last().unwrap().t, cles_avant.last().unwrap().t);
    assert!((a.x - z.x).abs() < 1e-2 && (a.y - z.y).abs() < 1e-2);
    // Maj ailleurs sur le chemin : il se déplace toujours en bloc.
    let avant = b.app.layers()[0].trajectoire.clone().unwrap();
    geste(&mut b, &[(200.0, 150.0), (200.0, 175.0), (200.0, 200.0)], Modifiers::SHIFT, true);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert_eq!(derniere_etape(&b), "Déplacer la trajectoire");
    assert!((t.longueur() - avant.longueur()).abs() < 1.0 && (t.points[0][1] - avant.points[0][1] - 50.0).abs() < 1.0);
}

// ------------------------------------------------------------------------------------------------ dans l'Espace 3D

#[test]
fn dans_l_espace_maj_depuis_la_fin_prolonge_le_chemin_sur_son_plan() {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Carte 0", LayerKind::Bitmap);
    let cel = doc.new_cel(LayerKind::Bitmap);
    layer.frames = vec![Some(cel); 24];
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = 24;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.run_action("espace3d.basculer");
    b.app.run_action("espace3d.plan_en_espace");
    b.chauffer(2);
    b.app.run_action("espace3d.cadrer");
    b.app.run_action("outil.trajectoire");
    b.chauffer(3);
    assert!(b.app.e3_trace());
    // Un chemin au sol, devant la carte, vers la droite.
    let world = b.app.e3_world();
    let sol = Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k).unwrap();
    let pied = world.carte(&b.app.layers()[0], 0).pos;
    let p0 = sol.dans_le_plan([pied[0] - 0.5, world.sol_y(), pied[2] + 0.3]);
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[p0, [p0[0] + 1.0 / world.k, p0[1]]], sol).unwrap();
    (t.cles, t.fin) = (Cles::Interpolation, Some(23));
    b.app.layers_mut()[0].trajectoire = Some(t.clone());
    b.app.trajectoire_recalculer(0);
    b.app.espace3d.sel = vec![crate::espace3d::card_selection(b.app.layers()[0].id)];
    b.chauffer(2);
    let regard = b.app.e3d.regard.expect("le regard de la vue");
    let a_l_ecran = |q: [f32; 2]| regard.ecran(sol.monde(q)).expect("se voit");
    let bout = *t.points.last().unwrap();
    let suite = [bout[0] + 0.8 / world.k, bout[1] + 0.6 / world.k];
    let (de, vers) = (a_l_ecran(bout), a_l_ecran(suite));
    let etapes = b.app.history.names().0.len();
    let mods = Modifiers::SHIFT;
    b.touches = mods;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: de, button: PointerButton::Primary, pressed: true, modifiers: mods }], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 12.0))], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerButton { pos: vers, button: PointerButton::Primary, pressed: false, modifiers: mods }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
    let n = b.app.layers()[0].trajectoire.clone().unwrap();
    assert_eq!(n.support, t.support, "le prolongement reste sur le plan du chemin");
    assert!(n.longueur() > t.longueur() * 1.6, "{} contre {} : {} (le trait fait {} pixels à l'écran)", n.longueur(), t.longueur(), b.app.status, de.distance(vers));
    assert!(proche2(*n.points.last().unwrap(), suite, 0.05 / world.k), "{:?} contre {suite:?}", n.points.last());
    assert_eq!(b.app.history.names().0.len(), etapes + 1);
    assert_eq!(b.app.history.names().0.last().map(|s| s.to_string()).as_deref(), Some("Prolonger la trajectoire"));
    // La carte va jusqu'au nouveau bout.
    let w = b.app.e3_world();
    let depart = w.carte(&b.app.layers()[0], 0).pos;
    let arrivee = w.carte(&b.app.layers()[0], 23).pos;
    let attendu = m::add(depart, n.deplacement_a(1.0).unwrap());
    assert!(m::length(m::sub(arrivee, attendu)) < 1e-2, "{arrivee:?} contre {attendu:?}");
    b.app.undo();
    assert_eq!(b.app.layers()[0].trajectoire.as_ref(), Some(&t));
}

// ------------------------------------------------------------------------------------------------ avec les clés de dessin

/// Après la partie 152 : une carte en « Clés d'images » dans l'espace a une clé de dessin à chaque jalon. Le lissage et le
/// prolongement reposent ses clés par `trajectoire_recalculer` : les copies suivies restent les mêmes dessins, à leurs
/// jalons, et la carte est au bout du chemin à la fin.
#[test]
fn en_cles_d_images_le_lissage_et_le_prolongement_gardent_une_cle_de_dessin_par_jalon() {
    use crate::trajectoire3d_tests::{carte, doc_cartes, ouvert, tracer_au_sol, trajectoire};
    let mut b = ouvert(doc_cartes(1), true);
    b.app.traj.cles_propose = Cles::Images;
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    assert_eq!((t.cles, t.dessins.len(), t.images_des_jalons(24)), (Cles::Images, 2, vec![0, 23]), "{}", b.app.status);
    let (d0, d1) = (t.dessins[0].dessin, t.dessins[1].dessin);
    let depart = carte(&b, 0, 0).pos;
    let sujets = b.app.traj_sujets_des_reglages();
    // Le lissage : pendant le geste, rien n'est copié ; à la fin, les mêmes dessins, à leurs jalons.
    let dessins_avant = b.app.doc.cels.len();
    b.app.trajectoire_lisser(&[], &sujets, 0.8);
    b.app.trajectoire_lissage_fini(&[], &sujets);
    let t = trajectoire(&b, 0);
    assert_eq!(t.lissage, 0.8);
    assert_eq!(t.dessins.iter().map(|d| d.dessin).collect::<Vec<_>>(), vec![d0, d1], "les copies suivies restent");
    assert_eq!((b.app.layers()[0].cel_at(22), b.app.layers()[0].cel_at(23)), (Some(d0), Some(d1)));
    assert_eq!(b.app.doc.cels.len(), dessins_avant, "aucune copie de plus");
    // Le prolongement par la fin : toujours une clé de dessin par jalon, la carte au nouveau bout.
    let bout = *t.points.last().unwrap();
    let plan = [[bout[0] + 20.0, bout[1]], [bout[0] + 60.0, bout[1] + 10.0]];
    b.app.e3_traj_prolonger(&[Pos2::new(0.0, 0.0), Pos2::new(60.0, 0.0)], &plan, false);
    let n = trajectoire(&b, 0);
    assert!(n.longueur() > t.longueur() + 40.0, "{}", b.app.status);
    assert_eq!((n.dessins.len(), n.images_des_jalons(24)), (2, vec![0, 23]));
    assert_eq!(b.app.layers()[0].cel_at(23), Some(n.dessins[1].dessin));
    let attendu = m::add(depart, n.deplacement_a(1.0).unwrap());
    assert!(m::length(m::sub(carte(&b, 0, 23).pos, attendu)) < 0.05, "{:?} contre {attendu:?}", carte(&b, 0, 23).pos);
    assert_eq!(b.app.history.names().0.last().map(|s| s.to_string()).as_deref(), Some("Prolonger la trajectoire"));
}

// ------------------------------------------------------------------------------------------------ après la relecture

#[test]
fn une_marque_tout_pres_du_bout_fixe_n_est_pas_perdue_et_le_dessin_qui_tourne_garde_son_orientation() {
    let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    t.ajouter_marque(0.3);
    t.ajouter_marque(0.98);
    t.tourne = true;
    // Prolongé par devant, trois fois plus long : la marque à 2 % de la fin passerait sous l'écart minimal.
    assert!(t.prolonger(&[[0.0, -3.0], [0.0, -100.0], [0.0, -200.0]], true));
    assert_eq!(t.marques.len(), 2, "aucune marque perdue : {:?}", t.marques);
    assert!(t.marques[0] < t.marques[1] && t.marques[1] <= 1.0 - crate::trajectoire::ECART_MIN);
    assert!(proche2(t.point_a(t.marques[0]), [30.0, 0.0], 1.5), "{:?}", t.point_a(t.marques[0]));
    // Le nouveau morceau monte à la verticale : le dessin qui tourne avec le chemin n'est pas tourné sur l'ancienne partie.
    let f = t.plus_proche([50.0, 0.0]).0;
    assert!(t.angle_a(f).abs() < 2.0, "sur l'ancienne partie, l'orientation d'avant : {}", t.angle_a(f));
    assert!((t.angle_a(0.1).abs() - 90.0).abs() < 5.0, "sur le morceau ajouté, le quart de tour : {}", t.angle_a(0.1));
}

#[test]
fn retracer_le_chemin_garde_son_lissage() {
    let mut b = banc();
    tracer_le_chemin(&mut b);
    b.app.trajectoire_lisser(&[0], &[], 0.6);
    b.app.traj.lissage = 0.0;
    geste(&mut b, &[(60.0, 200.0), (160.0, 220.0), (260.0, 200.0), (340.0, 230.0)], Modifiers::NONE, true);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert_eq!(derniere_etape(&b), "Retracer la trajectoire");
    assert!((t.lissage - 0.6).abs() < 1e-6 && t.brut.is_some(), "{}", t.lissage);
}