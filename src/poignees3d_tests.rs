//! Essais de la partie 184 : **les poignées dans la vue** (`poignees3d.rs`). Un objet du décor 3D se choisit au clic dans la vue
//! Espace 3D (le plus proche de l'œil, carte ou objet, l'emporte ; ses vrais triangles, pas seulement sa boîte), et dans la liste
//! du panneau du même coup ; le manipulateur le déplace, le tourne et le met à l'échelle autour de son origine, en une étape
//! d'annulation, les cartes aimantées posées sur lui suivant ; un objet aimanté garde la hauteur que lui donne l'aimant ;
//! « Modifier l'origine » déplace l'origine d'une carte ou d'un objet sans rien bouger d'autre, Maj l'accroche aux points de
//! la boîte, et l'aimant repose ensuite par la nouvelle origine.

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::decor3d::{Decor3d, Objet3d};
use crate::decor3d_tests::boite;
use crate::decor3dgltf::{Primitive3d, Scene3d};
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::{self, Monde};
use crate::espace3dgizmo::{Handle, Mode as GizmoMode, CAMERA_ID};
use crate::espace3dmath::{self as m, Orbite, View, V3};
use crate::espace3dui::Drag;
use crate::poignees3d::{self, objet_of, objet_selection, ORIGINE_ACTION};

const LARGEUR: f32 = 1280.0;
const CENTRE_VUE: [f32; 2] = [640.0, 360.0];
const ID_OBJET: u64 = 900;

/// Un projet de 160 × 90 (feuille 320 × 180), sans calque.
fn projet() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].length = 24;
    doc.panels[0].layers.clear();
    doc
}

/// Peint un rectangle plein (`x0..x1` × `y0..y1`, pixels de la feuille) sur le dessin `cel`.
fn peindre(doc: &mut Document, cel: crate::document::CelId, [x0, x1, y0, y1]: [usize; 4]) {
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            let (ox, oy) = r.tile_origin(idx);
            r.tile_mut(idx)[(y as i64 - oy) as usize * crate::raster::TILE + (x as i64 - ox) as usize] = Color32::from_rgb(0, 0, 255);
        }
    }
}

/// Un calque bitmap dont le dessin est ce rectangle plein, sa carte à `z` mètres. Son rang.
fn calque(doc: &mut Document, nom: &str, rect: [usize; 4], z: f32) -> usize {
    let c = doc.new_cel(LayerKind::Bitmap);
    peindre(doc, c, rect);
    let mut l = Layer::new(nom, LayerKind::Bitmap);
    l.frames = vec![Some(c); 24];
    l.depth = -z / Monde::of(doc).d0;
    doc.panels[0].layers.push(l);
    doc.panels[0].layers.len() - 1
}

/// Le rectangle du centre de la feuille (± 30 × ± 20 pixels).
const CENTRE: [usize; 4] = [130, 190, 70, 110];

/// Une boîte du décor de `a` à `b` (repère du modèle), son origine au centre du dessous, posée à `pos`, sans aimant.
fn objet_boite(id: u64, a: [f32; 3], b: [f32; 3], pos: V3) -> Objet3d {
    let mut o = crate::decor3d_tests::objet(id, boite(a, b), pos[0], pos[2]);
    o.place.position = pos;
    o.aimante = false;
    o
}

fn poser_decor(doc: &mut Document, objets: Vec<Objet3d>) {
    let seq = doc.sequence_of(0);
    let mut d = Decor3d::new(Default::default());
    d.objets = objets;
    doc.decors3d.par_sequence.insert(seq, d);
}

/// Une application sans fenêtre sur ce projet, l'Espace 3D ouvert, vue de face (l'œil à 12 m sur l'axe Z), sans aimantation à
/// la grille.
fn app_avec(nom: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("feanor-poignees3d-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    // (Un plan a toujours un calque : un calque vide, sans dessin, si l'essai n'en pose pas.)
    let mut doc = doc;
    if doc.panels[0].layers.is_empty() {
        doc.panels[0].layers.push(Layer::new("Vide", LayerKind::Bitmap));
    }
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.espace3d.on = true;
    app.espace3d.snap = false;
    app.espace3d.cam = Some(face());
    app.espace3d.last_layer = app.layer().id;
    app
}

fn face() -> Orbite {
    Orbite { target: [0.0, 0.0, 0.0], azimuth: 0.0, elevation: 0.0, distance: 12.0, focal: 35.0 }
}

fn vue(app: &App) -> View {
    app.espace3d.cam.expect("vue").view(LARGEUR)
}

/// Le pixel de la vue où est le point `p` du monde.
fn px(view: &View, p: V3) -> [f32; 2] {
    view.project(p, CENTRE_VUE).expect("devant l'œil")
}

/// Le geste d'une poignée, de `de` à `a` (points du monde, vus par la vue de l'application), en une étape.
fn geste(app: &mut App, handle: Handle, de: V3, a: V3) {
    let view = vue(app);
    let gizmo = app.e3_gizmo(&view).expect("un manipulateur");
    let mode = if app.espace3d.origine { GizmoMode::Move } else { app.espace3d.mode };
    let (p0, p1) = (px(&view, de), px(&view, a));
    app.e3_begin(handle, mode, gizmo, view.ray(p0, CENTRE_VUE), p0, "Geste d'essai");
    app.e3_drag(&view, CENTRE_VUE, p1);
    app.e3_finish_drag();
}

fn objet(app: &App) -> Objet3d {
    app.e3_objet(ID_OBJET).expect("l'objet").clone()
}

fn proche(a: V3, b: V3, tol: f32) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
}

/// L'origine (monde) de la carte du calque `li` à l'image `f`.
fn origine_monde(app: &App, li: usize, f: usize) -> V3 {
    let world = app.e3_world();
    let l = &app.layers()[li];
    world.origine_monde(&world.carte(l, f), world.ecart(l))
}

/// Le point du monde où est le pixel (`x`, `y`) du dessin du calque `li`, sur sa carte à l'image 0.
fn sur_la_carte(app: &App, li: usize, x: f32, y: f32) -> V3 {
    let world = app.e3_world();
    let xf = world.carte(&app.layers()[li], 0);
    m::point(&xf.matrix(), [x / world.stage[0] - 0.5, 0.5 - y / world.stage[1], 0.0])
}

fn coins(app: &App, li: usize, f: usize) -> [V3; 4] {
    let world = app.e3_world();
    world.card_corners(&world.carte(&app.layers()[li], f))
}

// ------------------------------------------------------------------------------------------------ les numéros, le rayon

#[test]
fn les_numeros_des_objets_ne_se_confondent_ni_avec_les_cartes_ni_avec_la_camera() {
    let s = objet_selection(42);
    assert_eq!(objet_of(s), Some(42));
    assert_eq!(espace3d::card_of(s), None, "un objet n'est pas une carte");
    assert_eq!(objet_of(espace3d::card_selection(42)), None, "une carte n'est pas un objet");
    assert_eq!(objet_of(CAMERA_ID), None, "la caméra non plus");
    assert_eq!(objet_of(42), None);
}

#[test]
fn le_rayon_prend_les_vrais_triangles_pas_seulement_la_boite() {
    // Un seul triangle, dans le plan z = 0 : (0, 0), (1, 0), (0, 1). Sa boîte va de (0, 0) à (1, 1).
    let p = Primitive3d { positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], normales: vec![[0.0, 0.0, 1.0]; 3], indices: vec![0, 1, 2], couleur: [1.0; 4], ..Default::default() };
    let scene = Scene3d { primitives: vec![p], textures: Vec::new() };
    let mut o = crate::decor3d_tests::objet(1, scene, 0.0, 0.0);
    (o.origine, o.place.position) = ([0.0; 3], [0.0, 0.0, -2.0]);
    let depuis = |x: f32, y: f32| m::Ray::new([x, y, 3.0], [0.0, 0.0, -1.0]);
    let t = poignees3d::toucher_objet(&o, &depuis(0.25, 0.25)).expect("dans le triangle");
    assert!((t - 5.0).abs() < 1e-4, "la vraie distance dans le monde : {t}");
    assert!(poignees3d::toucher_objet(&o, &depuis(0.8, 0.8)).is_none(), "dans la boîte, mais hors du triangle : rien");
    // Agrandi : la distance reste celle du monde.
    o.place.echelle = 2.0;
    let t = poignees3d::toucher_objet(&o, &depuis(0.5, 0.5)).expect("le triangle agrandi");
    assert!((t - 5.0).abs() < 1e-4, "{t}");
    assert!(poignees3d::toucher_objet(&o, &m::Ray::new([0.25, 0.25, 3.0], [0.0, 0.0, 1.0])).is_none(), "derrière l'œil : rien");
}

#[test]
fn les_points_ou_s_accroche_l_origine_d_un_objet() {
    let p = poignees3d::points_de_la_boite(([-1.0, 0.0, -2.0], [1.0, 3.0, 2.0]));
    assert_eq!(p.len(), 15, "le centre, six centres de faces, huit coins");
    assert_eq!(p[0], [0.0, 1.5, 0.0], "le centre");
    assert_eq!(p[1], [0.0, 0.0, 0.0], "puis le centre du dessous");
    for q in [[1.0, 1.5, 0.0], [0.0, 3.0, 0.0], [0.0, 1.5, -2.0], [-1.0, 0.0, -2.0], [1.0, 3.0, 2.0]] {
        assert!(p.contains(&q), "{q:?}");
    }
    assert_eq!(poignees3d::plus_proche(&p, [0.9, 0.1, 1.7]), [1.0, 0.0, 2.0], "le coin le plus proche");
}

// ------------------------------------------------------------------------------------------------------ le choix

/// Une carte au centre à `z_carte`, une boîte (1 × 2 × 0,4 m) dont le devant est à `z_boite` + 0,2, au milieu de la vue.
fn carte_et_boite(nom: &str, z_carte: f32, z_boite: f32) -> App {
    let mut doc = projet();
    calque(&mut doc, "Perso", CENTRE, z_carte);
    poser_decor(&mut doc, vec![objet_boite(ID_OBJET, [-0.5, 0.0, -0.2], [0.5, 2.0, 0.2], [0.0, -1.0, z_boite])]);
    app_avec(nom, doc)
}

#[test]
fn un_clic_prend_le_plus_proche_de_l_oeil_carte_ou_objet() {
    // La carte devant la boîte : la carte. (On vise 50 cm au-dessus du centre : le corps de la caméra du plan est au centre de
    // la vue, et un clic sur lui la prend.)
    let mut app = carte_et_boite("choix-devant", -1.0, -3.0);
    let view = vue(&app);
    let ici = px(&view, [0.0, 0.5, 0.0]);
    app.e3_click(&view, CENTRE_VUE, ici, egui::Modifiers::NONE);
    assert_eq!(app.espace3d.sel, vec![espace3d::card_selection(app.layers()[0].id)], "la carte, devant");
    // La carte derrière la boîte : la boîte, choisie aussi dans la liste du panneau.
    let mut app = carte_et_boite("choix-derriere", -5.0, -3.0);
    let carte = espace3d::card_selection(app.layers()[0].id);
    app.decor3d.choisi = None;
    // (L'outil Choisir : pas de manipulateur, dont une flèche passerait sous le pointeur.)
    app.espace3d.mode = GizmoMode::Select;
    app.e3_click(&view, CENTRE_VUE, ici, egui::Modifiers::NONE);
    assert_eq!(app.espace3d.sel, vec![objet_selection(ID_OBJET)], "l'objet, devant la carte");
    assert_eq!(app.decor3d.choisi, Some(ID_OBJET), "la liste du panneau suit la vue");
    // Maj : la carte s'ajoute (touchée à côté de la boîte : on élargit son dessin vers la droite) ; Maj sur la boîte : elle s'en va.
    let cel = app.layers()[0].frames[0].unwrap();
    peindre(&mut app.doc, cel, [130, 320, 70, 110]);
    let a_cote = px(&view, [2.0, 0.0, -5.0]);
    app.e3_click(&view, CENTRE_VUE, a_cote, egui::Modifiers::SHIFT);
    assert_eq!(app.espace3d.sel, vec![objet_selection(ID_OBJET), carte], "cartes et objets se mêlent");
    app.e3_click(&view, CENTRE_VUE, ici, egui::Modifiers::SHIFT);
    assert_eq!(app.espace3d.sel, vec![carte]);
    // « Toujours devant le décor 3D » : la carte, derrière la boîte, passe devant elle, comme à l'écran.
    app.layers_mut()[0].devant_decor = true;
    app.e3_click(&view, CENTRE_VUE, ici, egui::Modifiers::NONE);
    assert_eq!(app.espace3d.sel, vec![carte], "une carte toujours devant le décor l'emporte");
    app.layers_mut()[0].devant_decor = false;
    app.e3_click(&view, CENTRE_VUE, ici, egui::Modifiers::NONE);
    assert_eq!(app.espace3d.sel, vec![objet_selection(ID_OBJET)]);
}

#[test]
fn sans_decor_le_choix_est_celui_d_avant() {
    let mut doc = projet();
    calque(&mut doc, "Perso", CENTRE, -1.0);
    let mut app = app_avec("sans-decor", doc);
    let ray = vue(&app).ray(px(&vue(&app), [0.0, 0.5, 0.0]), CENTRE_VUE);
    assert!(app.e3_pick_tout(&ray).is_some());
    assert_eq!(app.e3_pick_tout(&ray), app.e3_pick(&ray));
}

#[test]
fn choisir_dans_le_panneau_choisit_dans_la_vue_et_enlever_l_oublie() {
    let mut app = carte_et_boite("panneau", -1.0, -3.0);
    // (Le clic sur une ligne de la liste du panneau appelle ceci, `decor3d_reglages`.)
    app.e3_choisir_objet(ID_OBJET, false);
    assert_eq!(app.espace3d.sel, vec![objet_selection(ID_OBJET)]);
    assert_eq!(app.decor3d.choisi, Some(ID_OBJET));
    assert!(app.e3_gizmo(&vue(&app)).is_some(), "les poignées vont à l'objet");
    app.decor3d_enlever_objet(ID_OBJET);
    assert!(app.espace3d.sel.is_empty(), "un objet enlevé n'est plus choisi");
    assert!(app.e3_gizmo(&vue(&app)).is_none());
}

// ------------------------------------------------------------------------------------------------------ le geste

/// Une dalle (2 × 0,4 × 2 m) posée sur le sol à z = −3 m, non aimantée, et une carte aimantée au sol dont l'origine (le bas de
/// son dessin) est au-dessus d'elle : la carte repose sur la dalle.
fn dalle_et_carte(nom: &str) -> App {
    let mut doc = projet();
    let li = calque(&mut doc, "Perso", CENTRE, -3.0);
    doc.panels[0].layers[li].origine = Some([160.0, 110.0]);
    let sol = Monde::of(&doc).sol_y();
    poser_decor(&mut doc, vec![objet_boite(ID_OBJET, [-1.0, 0.0, -1.0], [1.0, 0.4, 1.0], [0.0, sol, -3.0])]);
    let mut app = app_avec(nom, doc);
    app.basculer_aimant(li);
    let o = origine_monde(&app, li, 0);
    assert!((o[1] - (sol + 0.4)).abs() < 1e-3, "la carte repose sur la dalle : {o:?}");
    app
}

#[test]
fn deplacer_un_objet_est_une_etape_et_les_cartes_posees_dessus_suivent() {
    let mut app = dalle_et_carte("deplacer");
    let sol = app.e3_world().sol_y();
    app.e3_choisir_objet(ID_OBJET, false);
    app.espace3d.mode = GizmoMode::Move;
    let avant = app.history.undo_len();
    let p0 = objet(&app).place.position;
    // La dalle monte de 30 cm par la flèche Y (elle n'est pas aimantée).
    assert!(app.e3_poignee_refusee(GizmoMode::Move, Handle::Axis(1)).is_none());
    geste(&mut app, Handle::Axis(1), m::add(p0, [0.0, 0.2, 0.0]), m::add(p0, [0.0, 0.5, 0.0]));
    let p1 = objet(&app).place.position;
    assert!(proche(p1, m::add(p0, [0.0, 0.3, 0.0]), 2e-3), "{p0:?} → {p1:?}");
    assert!((origine_monde(&app, 0, 0)[1] - (sol + 0.7)).abs() < 3e-3, "la carte aimantée posée dessus suit");
    assert_eq!(app.history.undo_len(), avant + 1, "une seule étape d'annulation pour le geste");
    app.undo();
    assert!(proche(objet(&app).place.position, p0, 1e-6), "l'annulation ramène la dalle");
    assert!((origine_monde(&app, 0, 0)[1] - (sol + 0.4)).abs() < 1e-3, "et la carte avec elle");
    // Le long de X, dans le repère de l'espace.
    geste(&mut app, Handle::Axis(0), m::add(p0, [0.3, 0.0, 0.0]), m::add(p0, [0.8, 0.0, 0.0]));
    assert!(proche(objet(&app).place.position, m::add(p0, [0.5, 0.0, 0.0]), 2e-3), "{:?}", objet(&app).place.position);
}

#[test]
fn tourner_et_mettre_a_l_echelle_un_objet_autour_de_son_origine() {
    let mut app = dalle_et_carte("tourner");
    app.e3_choisir_objet(ID_OBJET, false);
    // Vue d'au-dessus, de biais : l'anneau Y se voit.
    app.espace3d.cam = Some(Orbite { target: [0.0, -1.0, -3.0], azimuth: 20.0, elevation: 55.0, distance: 10.0, focal: 35.0 });
    app.espace3d.mode = GizmoMode::Rotate;
    let view = vue(&app);
    let o = objet(&app).place.position;
    assert_eq!(app.e3_poignee_refusee(GizmoMode::Rotate, Handle::Ring(0)).as_deref(), Some(poignees3d::VERTICALE_SEULE), "hors de la verticale : refusé");
    assert!(app.e3_poignee_refusee(GizmoMode::Rotate, Handle::Ring(1)).is_none());
    let (de, a) = (m::add(o, [1.0, 0.0, 0.0]), m::add(o, [0.0, 0.0, -1.0]));
    let attendu = {
        let (r0, r1) = (view.ray(px(&view, de), CENTRE_VUE), view.ray(px(&view, a), CENTRE_VUE));
        m::drag_angle(&r0, &r1, o, [0.0, 1.0, 0.0])
    };
    assert!(attendu.abs() > 45.0, "{attendu}");
    let n = app.history.undo_len();
    geste(&mut app, Handle::Ring(1), de, a);
    let apres = objet(&app);
    assert!((apres.place.rotation_y - attendu).abs() < 0.05, "{} contre {attendu}", apres.place.rotation_y);
    assert!(proche(apres.place.position, o, 1e-4), "il tourne autour de son origine");
    assert_eq!(app.history.undo_len(), n + 1);
    // L'échelle : d'un bloc (un cube d'axe pris donne l'échelle uniforme), autour de l'origine.
    app.espace3d.mode = GizmoMode::Scale;
    let view = vue(&app);
    let g = app.e3_gizmo(&view).unwrap();
    let p0 = px(&view, g.axis_tip(0));
    app.e3_press(&view, CENTRE_VUE, p0, egui::Modifiers::NONE);
    assert!(matches!(app.espace3d.drag, Some(Drag::Handle { handle: Handle::Center, .. })), "un objet grandit d'un bloc");
    app.e3_drag(&view, CENTRE_VUE, [p0[0], p0[1] - 50.0]);
    app.e3_finish_drag();
    let f = m::scale_from_screen(-50.0);
    let apres = objet(&app);
    assert!((apres.place.echelle - f).abs() < 1e-4, "{} contre {f}", apres.place.echelle);
    assert!(proche(apres.place.position, o, 1e-4), "autour de son origine");
    app.undo();
    assert!((objet(&app).place.echelle - 1.0).abs() < 1e-6);
}

#[test]
fn un_objet_aimante_garde_la_hauteur_de_l_aimant() {
    let mut doc = projet();
    let sol = Monde::of(&doc).sol_y();
    let mut o = objet_boite(ID_OBJET, [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], [0.0, sol, -3.0]);
    o.aimante = true;
    poser_decor(&mut doc, vec![o]);
    let mut app = app_avec("aimant", doc);
    app.e3_choisir_objet(ID_OBJET, false);
    app.espace3d.mode = GizmoMode::Move;
    // La flèche Y est refusée (grise), comme le champ de la hauteur du panneau ; l'appui sur elle ne commence aucun geste.
    assert!(app.e3_poignee_refusee(GizmoMode::Move, Handle::Axis(1)).is_some_and(|r| r.contains("aimant")));
    let view = vue(&app);
    let g = app.e3_gizmo(&view).unwrap();
    let pointe = px(&view, g.axis_tip(1));
    assert_eq!(g.hit(&view, CENTRE_VUE, pointe, 9.0), Some(Handle::Axis(1)));
    app.e3_press(&view, CENTRE_VUE, pointe, egui::Modifiers::NONE);
    assert!(app.espace3d.drag.is_none(), "rien ne commence");
    assert!(app.status.contains("aimant"), "la barre d'état dit pourquoi : {}", app.status);
    // Dans le plan de l'écran (X, Y), en haut à droite : il va à droite, et reste posé sur le sol.
    let p0 = objet(&app).place.position;
    geste(&mut app, Handle::Plane(2), m::add(p0, [0.2, 0.2, 0.0]), m::add(p0, [0.7, 1.2, 0.0]));
    let p1 = objet(&app).place.position;
    assert!((p1[0] - (p0[0] + 0.5)).abs() < 2e-3, "{p1:?}");
    assert!((p1[1] - sol).abs() < 1e-4, "l'aimant le repose sur le sol : {p1:?}");
    // Tiré directement (sans poignée), Maj tenue : il ne monte pas non plus, il glisse sur le sol.
    let view = vue(&app);
    let corps = px(&view, m::add(p1, [0.25, 0.5, 0.5]));
    app.e3_press(&view, CENTRE_VUE, corps, egui::Modifiers::SHIFT);
    assert!(matches!(app.espace3d.drag, Some(Drag::Handle { handle: Handle::Plane(1), .. })), "sur le sol, pas en hauteur");
    app.e3_finish_drag();
}

#[test]
fn cartes_et_objets_bougent_ensemble_en_une_etape() {
    let mut app = carte_et_boite("ensemble", -1.0, -3.0);
    let carte = espace3d::card_selection(app.layers()[0].id);
    app.espace3d.sel = vec![carte, objet_selection(ID_OBJET)];
    app.espace3d.mode = GizmoMode::Move;
    let c0 = origine_monde(&app, 0, 0);
    let o0 = objet(&app).place.position;
    let n = app.history.undo_len();
    let g = app.e3_gizmo(&vue(&app)).unwrap();
    geste(&mut app, Handle::Axis(0), m::add(g.origin, [0.2, 0.0, 0.0]), m::add(g.origin, [1.2, 0.0, 0.0]));
    assert!(proche(origine_monde(&app, 0, 0), m::add(c0, [1.0, 0.0, 0.0]), 3e-3), "{:?}", origine_monde(&app, 0, 0));
    assert!(proche(objet(&app).place.position, m::add(o0, [1.0, 0.0, 0.0]), 3e-3), "{:?}", objet(&app).place.position);
    assert_eq!(app.history.undo_len(), n + 1);
    app.undo();
    assert!(proche(origine_monde(&app, 0, 0), c0, 1e-4) && proche(objet(&app).place.position, o0, 1e-6));
}

// ------------------------------------------------------------------------------------------------------ l'origine

#[test]
fn la_commande_modifier_l_origine() {
    let mut app = carte_et_boite("commande", -1.0, -3.0);
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == ORIGINE_ACTION));
    let toml = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("keymap.toml")).unwrap();
    assert!(toml.contains(&format!("action = \"{ORIGINE_ACTION}\"")), "son bloc dans le fichier des raccourcis");
    app.run_action(ORIGINE_ACTION);
    assert!(app.espace3d.origine);
    // Rien de choisi : pas de manipulateur ; une carte seule : il est sur son origine ; deux choses : rien.
    app.espace3d.sel.clear();
    assert!(app.e3_gizmo(&vue(&app)).is_none());
    app.espace3d.sel = vec![espace3d::card_selection(app.layers()[0].id)];
    let g = app.e3_gizmo(&vue(&app)).expect("le manipulateur de l'origine");
    assert!(proche(g.origin, origine_monde(&app, 0, 0), 1e-5));
    assert_eq!(g.mode, GizmoMode::Move);
    app.espace3d.sel.push(objet_selection(ID_OBJET));
    assert!(app.e3_gizmo(&vue(&app)).is_none());
    // Un outil de la vue l'éteint ; la commande aussi.
    app.run_action("espace3d.outil_tourner");
    assert!(!app.espace3d.origine);
    app.run_action(ORIGINE_ACTION);
    app.run_action(ORIGINE_ACTION);
    assert!(!app.espace3d.origine);
}

#[test]
fn l_origine_d_une_carte_se_deplace_sans_que_son_image_bouge() {
    let mut app = carte_et_boite("origine-carte", -1.0, -3.0);
    // Deux clés de position (la seconde tournée) : rien ne bouge à aucune clé. (Entre deux clés, le calque tourne désormais
    // autour de sa nouvelle origine : c'est la règle de la partie 182.)
    let t0 = app.layers()[0].transform;
    let t1 = crate::document::LayerTransform { x: 12.0, angle: 20.0, ..t0 };
    crate::poskeys::set_key(&mut app.layers_mut()[0].pos_keys, 0, t0);
    crate::poskeys::set_key(&mut app.layers_mut()[0].pos_keys, 10, t1);
    let avant: Vec<[V3; 4]> = [0, 10].map(|f| coins(&app, 0, f)).to_vec();
    app.espace3d.sel = vec![espace3d::card_selection(app.layers()[0].id)];
    app.run_action(ORIGINE_ACTION);
    let g = app.e3_gizmo(&vue(&app)).unwrap();
    // Hors du dessin : refusé.
    assert_eq!(app.e3_poignee_refusee(GizmoMode::Move, Handle::Axis(2)).as_deref(), Some(poignees3d::HORS_DU_DESSIN));
    let n = app.history.undo_len();
    // Vers le bas de la carte, le long de son axe Y.
    let bas = m::scale(g.axes[1], -0.3);
    geste(&mut app, Handle::Axis(1), g.origin, m::add(g.origin, bas));
    let o = app.layers()[0].origine.expect("une origine à soi");
    assert!((o[0] - 160.0).abs() < 0.5 && o[1] > 95.0, "plus bas sur le dessin : {o:?}");
    for (k, f) in [0, 10].into_iter().enumerate() {
        let apres = coins(&app, 0, f);
        for c in 0..4 {
            assert!(proche(apres[c], avant[k][c], 2e-4), "image {f}, coin {c} : {:?} contre {:?}", apres[c], avant[k][c]);
        }
    }
    assert!(proche(origine_monde(&app, 0, 0), m::add(g.origin, bas), 2e-3), "l'origine est là où la poignée l'a menée");
    assert_eq!(app.history.undo_len(), n + 1, "une étape d'annulation");
    app.undo();
    assert_eq!(app.layers()[0].origine, None);
    // Maj : elle s'accroche au point de la boîte du dessin le plus proche (le milieu du bas ; la boîte d'un dessin bitmap est à
    // la tuile près, comme pour l'outil Transformer).
    app.espace3d.maj = true;
    let g = app.e3_gizmo(&vue(&app)).unwrap();
    let b = app.layer_content_box(0, 0).expect("la boîte du dessin");
    let vise = sur_la_carte(&app, 0, b.center_bottom().x + 2.0, b.center_bottom().y - 3.0);
    geste(&mut app, Handle::Plane(2), g.origin, vise);
    assert_eq!(app.layers()[0].origine, Some([b.center_bottom().x, b.center_bottom().y]), "accrochée au milieu du bas de la boîte");
}

#[test]
fn l_origine_d_un_objet_se_deplace_sans_que_l_objet_bouge() {
    let mut app = carte_et_boite("origine-objet", -1.0, -3.0);
    app.e3_choisir_objet(ID_OBJET, false);
    app.run_action(ORIGINE_ACTION);
    let avant = objet(&app);
    let (blo, bhi) = avant.bornes_monde();
    let g = app.e3_gizmo(&vue(&app)).unwrap();
    assert!(proche(g.origin, avant.place.position, 1e-6));
    assert!(app.e3_poignee_refusee(GizmoMode::Move, Handle::Axis(2)).is_none(), "un objet : ses trois axes");
    let n = app.history.undo_len();
    geste(&mut app, Handle::Axis(1), m::add(g.origin, [0.0, 0.1, 0.0]), m::add(g.origin, [0.0, 0.8, 0.0]));
    let apres = objet(&app);
    assert!((apres.origine[1] - 0.7).abs() < 2e-3, "l'origine monte de 70 cm dans le modèle : {:?}", apres.origine);
    let (alo, ahi) = apres.bornes_monde();
    assert!(proche(alo, blo, 1e-4) && proche(ahi, bhi, 1e-4), "l'objet ne bouge pas");
    assert!(proche(apres.place.position, m::add(avant.place.position, [0.0, 0.7, 0.0]), 2e-3), "sa position est celle de la nouvelle origine");
    assert_eq!(app.history.undo_len(), n + 1);
    app.undo();
    assert!(proche(objet(&app).origine, avant.origine, 1e-6));
    // Maj : au point de sa boîte le plus proche (le centre, à 1 m de haut).
    app.espace3d.maj = true;
    let g = app.e3_gizmo(&vue(&app)).unwrap();
    geste(&mut app, Handle::Axis(1), g.origin, m::add(g.origin, [0.0, 0.9, 0.0]));
    assert_eq!(objet(&app).origine, [0.0, 1.0, 0.0], "accrochée au centre de la boîte");
    let (alo, ahi) = objet(&app).bornes_monde();
    assert!(proche(alo, blo, 1e-4) && proche(ahi, bhi, 1e-4));
    // Un appui sur l'objet lui-même, hors des poignées : il est choisi, rien ne bouge, aucun geste ne commence.
    // (Les cartes cachées : celle de l'essai est devant la boîte.)
    app.espace3d.show_cards = false;
    let view = vue(&app);
    let avant = objet(&app);
    app.e3_press(&view, CENTRE_VUE, px(&view, [0.3, 0.6, -2.8]), egui::Modifiers::NONE);
    assert!(app.espace3d.drag.is_none(), "aucun geste");
    assert_eq!(app.espace3d.sel, vec![objet_selection(ID_OBJET)]);
    assert!(proche(objet(&app).place.position, avant.place.position, 0.0));
}

#[test]
fn apres_l_origine_deplacee_l_aimant_repose_la_carte_par_elle() {
    let mut doc = projet();
    let li = calque(&mut doc, "Perso", CENTRE, -1.0);
    let mut app = app_avec("origine-aimant", doc);
    app.basculer_aimant(li);
    let sol = app.e3_world().sol_y();
    assert!((origine_monde(&app, li, 0)[1] - sol).abs() < 1e-4, "l'origine (le centre de la feuille) au sol");
    let haut_avant = coins(&app, li, 0)[0][1];
    app.espace3d.sel = vec![espace3d::card_selection(app.layers()[li].id)];
    app.run_action(ORIGINE_ACTION);
    let g = app.e3_gizmo(&vue(&app)).unwrap();
    // L'origine aux pieds du dessin (Maj : le milieu du bas de sa boîte) : au lâcher, l'aimant pose les pieds sur le sol, le
    // dessin monte.
    app.espace3d.maj = true;
    let b = app.layer_content_box(li, 0).expect("la boîte du dessin");
    let vise = sur_la_carte(&app, li, 160.0, b.bottom() - 2.0);
    geste(&mut app, Handle::Axis(1), g.origin, vise);
    assert_eq!(app.layers()[li].origine, Some([b.center_bottom().x, b.center_bottom().y]));
    assert!((origine_monde(&app, li, 0)[1] - sol).abs() < 1e-4, "la nouvelle origine repose sur le sol");
    assert!(coins(&app, li, 0)[0][1] > haut_avant + 0.05, "la carte est remontée");
    app.undo();
    assert_eq!(app.layers()[li].origine, None);
    assert!((coins(&app, li, 0)[0][1] - haut_avant).abs() < 1e-4, "l'annulation rend tout");
}

#[test]
fn le_survol_d_une_poignee_refusee_le_dit() {
    let mut doc = projet();
    let sol = Monde::of(&doc).sol_y();
    let mut o = objet_boite(ID_OBJET, [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], [0.0, sol, -3.0]);
    o.aimante = true;
    poser_decor(&mut doc, vec![o]);
    let mut app = app_avec("survol", doc);
    app.e3_choisir_objet(ID_OBJET, false);
    app.espace3d.mode = GizmoMode::Move;
    app.status.clear();
    app.espace3d.hover = Some(Handle::Axis(1));
    app.e3_survol_poignee();
    assert!(app.status.contains(poignees3d::HAUTEUR_TENUE), "{}", app.status);
    app.status.clear();
    app.espace3d.hover = Some(Handle::Axis(0));
    app.e3_survol_poignee();
    assert!(app.status.is_empty(), "une poignée qui peut : rien à dire");
    // Pendant « Modifier l'origine », le survol dit ce que fait la poignée.
    app.run_action(ORIGINE_ACTION);
    app.espace3d.hover = Some(Handle::Axis(1));
    app.e3_survol_poignee();
    assert_eq!(app.status, poignees3d::SURVOL_ORIGINE);
}
