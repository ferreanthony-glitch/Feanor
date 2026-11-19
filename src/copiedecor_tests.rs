//! Essais de la partie 190 : **copier les objets du décor** (`copiedecor.rs`). Copier, couper, coller les objets du décor 3D
//! (dans la même séquence : à côté ; ailleurs : à la même place), plusieurs à la fois, au clavier après un clic dans la vue
//! Espace 3D ; les copier ou les envoyer vers une autre séquence ; d'un projet ouvert à l'autre ; une séquence coupée qui emporte
//! une copie de son décor ; Ctrl+C / Ctrl+V qui gardent leur sens d'avant ailleurs ; l'enregistrement après un collage ; le
//! budget de la machine.

use std::sync::Arc;

use eframe::egui::{self, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::app::App;
use crate::decor3d::{Decor3d, Objet3d};
use crate::decor3d_tests::{boite, objet};
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::Monde;
use crate::espace3dmath::{Orbite, V3};
use crate::poignees3d::objet_selection;

const DALLE: u64 = 900;
const BANC: u64 = 901;

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

/// Une carte (calque bitmap, un rectangle au centre de la feuille) dans le plan `pi`, à `z` mètres, son origine au bas de son
/// dessin. Son rang.
fn carte(doc: &mut Document, pi: usize, z: f32) -> usize {
    let c = doc.new_cel(LayerKind::Bitmap);
    peindre(doc, c, [130, 190, 70, 110]);
    let mut l = Layer::new("Perso", LayerKind::Bitmap);
    l.frames = vec![Some(c); 24];
    l.depth = -z / Monde::of(doc).d0;
    l.origine = Some([160.0, 110.0]);
    doc.panels[pi].layers.push(l);
    doc.panels[pi].layers.len() - 1
}

/// Une dalle (2 × 0,4 × 2 m, non aimantée) posée sur le sol à z = −3 m et un banc aimanté posé dessus.
fn dalle_et_banc(sol: f32) -> Decor3d {
    let mut dalle = objet(DALLE, boite([-1.0, 0.0, -1.0], [1.0, 0.4, 1.0]), 0.0, -3.0);
    dalle.place.position = [0.0, sol, -3.0];
    dalle.aimante = false;
    dalle.nom = "Dalle".into();
    let mut banc = objet(BANC, boite([-0.3, 0.0, -0.2], [0.3, 0.5, 0.2]), 0.0, -3.0);
    banc.nom = "Banc".into();
    let mut d = Decor3d::new(Default::default());
    d.aspect.brume = false;
    d.objets = vec![dalle, banc];
    crate::decor3daimant::aimanter(&mut d, sol);
    d
}

/// Un projet de deux séquences d'un plan chacune (160 × 90, feuille 320 × 180) : la première a un calque vectoriel et le décor
/// « dalle et banc » ; la seconde, une carte à z = −3 m et pas de décor.
fn deux_sequences() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].length = 24;
    let v = doc.new_cel(LayerKind::Vector);
    let mut vect = Layer::new("Vect", LayerKind::Vector);
    vect.frames = vec![Some(v); 24];
    doc.panels[0].layers = vec![vect];
    let b = doc.add_sequence_after(0);
    doc.panels[b].length = 24;
    doc.panels[b].layers.clear();
    carte(&mut doc, b, -3.0);
    let sol = Monde::of(&doc).sol_y();
    let seq = doc.sequence_of(0);
    doc.decors3d.par_sequence.insert(seq, dalle_et_banc(sol));
    doc
}

fn app_avec(nom: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("feanor-copiedecor-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
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

fn seq(app: &App, pi: usize) -> u64 {
    app.doc.sequence_of(pi)
}

fn decor(app: &App, pi: usize) -> Option<&Decor3d> {
    app.doc.decors3d.par_sequence.get(&seq(app, pi)).filter(|d| !d.objets.is_empty())
}

fn nomme<'a>(d: &'a Decor3d, nom: &str) -> Vec<&'a Objet3d> {
    d.objets.iter().filter(|o| o.nom == nom).collect()
}

fn proche(a: V3, b: V3, eps: f32) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < eps)
}

/// L'origine de la carte `li` du plan courant, dans le monde.
fn origine_carte(app: &App, li: usize) -> V3 {
    let world = app.e3_world();
    let l = &app.layers()[li];
    world.origine_monde(&world.carte(l, 0), world.ecart(l))
}

/// Les objets `ids` choisis dans la vue (comme des clics, le premier seul, les autres avec Maj), et le décor a la main (un appui
/// dans la vue Espace 3D ou dans le décor du panneau).
fn choisir(app: &mut App, ids: &[u64]) {
    for (k, id) in ids.iter().enumerate() {
        app.e3_choisir_objet(*id, k > 0);
    }
    app.decor3d_prendre_la_main();
}

// ------------------------------------------------------------------------------------------------- copier, coller

#[test]
fn copier_coller_dans_la_meme_sequence_pose_a_cote_en_une_etape_avec_le_meme_modele() {
    let mut app = app_avec("meme", deux_sequences());
    let sol = Monde::of(&app.doc).sol_y();
    let dalle = decor(&app, 0).unwrap().objet(DALLE).unwrap().clone();
    choisir(&mut app, &[DALLE]);
    app.run_action("edition.copier");
    assert_eq!(app.copie3d.presse.as_ref().map(|p| p.objets.len()), Some(1), "Ctrl+C copie l'objet choisi : {}", app.status);
    assert!(app.status.contains("« Dalle » copié"), "{}", app.status);
    let (avant, n) = (app.history.undo_len(), decor(&app, 0).unwrap().objets.len());
    app.run_action("edition.coller");
    assert_eq!(app.history.undo_len(), avant + 1, "une seule étape d'annulation : {}", app.status);
    let d = decor(&app, 0).unwrap();
    assert_eq!(d.objets.len(), n + 1);
    let copies: Vec<&Objet3d> = nomme(d, "Dalle").into_iter().filter(|o| o.id != DALLE).collect();
    assert_eq!(copies.len(), 1, "le nom est gardé");
    let c = copies[0];
    assert!(Arc::ptr_eq(&c.modele, &dalle.modele), "le même modèle, partagé : rien de relu ni de renvoyé à la carte");
    assert_eq!((c.origine, c.aimante, c.place.rotation_y, c.place.echelle, &c.source), (dalle.origine, dalle.aimante, dalle.place.rotation_y, dalle.place.echelle, &dalle.source));
    assert!(proche(c.place.position, [2.2, sol, -3.0], 1e-4), "à côté, comme « Dupliquer » (sa largeur et un dixième) : {:?}", c.place.position);
    assert_eq!(app.espace3d.sel, vec![objet_selection(c.id)], "la copie est choisie dans la vue");
    assert_eq!(app.decor3d.choisi, Some(c.id), "et dans la liste");
    assert!(app.status.contains("à côté"), "{}", app.status);
    // Collé encore : plus loin, pas sur la première copie.
    app.run_action("edition.coller");
    let d = decor(&app, 0).unwrap();
    let xs: Vec<f32> = nomme(d, "Dalle").iter().map(|o| o.place.position[0]).collect();
    assert_eq!(xs.len(), 3);
    assert!((xs[2] - 4.4).abs() < 1e-4, "{xs:?}");
    let ids: std::collections::BTreeSet<u64> = d.objets.iter().map(|o| o.id).collect();
    assert_eq!(ids.len(), d.objets.len(), "des numéros neufs");
    // Ctrl+Z, une étape par collage.
    app.undo();
    app.undo();
    assert_eq!(decor(&app, 0).unwrap().objets.len(), n);
    assert!(decor(&app, 0).unwrap().objet(DALLE).is_some());
}

#[test]
fn coller_dans_une_autre_sequence_pose_a_la_meme_place_et_les_cartes_aimantees_se_reposent() {
    let mut app = app_avec("autre", deux_sequences());
    let sol = Monde::of(&app.doc).sol_y();
    // La carte de la seconde séquence, aimantée au sol (pas de décor) : sur le sol.
    app.select_panel(1);
    app.basculer_aimant(0);
    assert!((origine_carte(&app, 0)[1] - sol).abs() < 1e-3, "sur le sol : {:?}", origine_carte(&app, 0));
    app.select_panel(0);
    let dalle = decor(&app, 0).unwrap().objet(DALLE).unwrap().clone();
    let (uid_a, aspect_a) = (decor(&app, 0).unwrap().uid, decor(&app, 0).unwrap().aspect);
    choisir(&mut app, &[DALLE]);
    app.run_action("edition.copier");
    app.select_panel(1);
    app.decor3d_prendre_la_main();
    let avant = app.history.undo_len();
    app.run_action("edition.coller");
    assert_eq!(app.history.undo_len(), avant + 1, "{}", app.status);
    let d = decor(&app, 1).expect("un décor neuf pour la seconde séquence");
    assert_ne!(d.uid, uid_a, "un décor à lui (sa texture)");
    assert_eq!(d.aspect, aspect_a, "l'aspect de celui d'où vient la copie");
    assert_eq!(d.objets.len(), 1);
    let c = &d.objets[0];
    assert_ne!(c.id, DALLE);
    assert_eq!(c.place, dalle.place, "à la même place dans le monde");
    assert!(Arc::ptr_eq(&c.modele, &dalle.modele));
    assert!(app.status.contains("à la même place"), "{}", app.status);
    assert!((origine_carte(&app, 0)[1] - (sol + 0.4)).abs() < 3e-3, "la carte aimantée monte sur la dalle collée : {:?}", origine_carte(&app, 0));
    // La première séquence n'a pas changé.
    assert_eq!(decor(&app, 0).unwrap().objets.len(), 2);
    app.undo();
    assert!(decor(&app, 1).is_none(), "Ctrl+Z retire le collage");
    assert!((origine_carte(&app, 0)[1] - sol).abs() < 1e-3, "et la carte redescend");
}

#[test]
fn couper_puis_coller_deplace_l_objet_et_ce_qui_reposait_dessus_redescend() {
    let mut app = app_avec("couper", deux_sequences());
    let sol = Monde::of(&app.doc).sol_y();
    let banc_y = |app: &App, pi: usize| decor(app, pi).unwrap().objet(BANC).unwrap().place.position[1];
    assert!((banc_y(&app, 0) - (sol + 0.4)).abs() < 2e-3, "le banc est sur la dalle");
    let dalle = decor(&app, 0).unwrap().objet(DALLE).unwrap().clone();
    choisir(&mut app, &[DALLE]);
    let avant = app.history.undo_len();
    app.run_action("edition.couper");
    assert_eq!(app.history.undo_len(), avant + 1, "couper : une étape ({})", app.status);
    assert!(decor(&app, 0).unwrap().objet(DALLE).is_none(), "la dalle est partie");
    assert!((banc_y(&app, 0) - sol).abs() < 2e-3, "le banc redescend sur le sol : {}", banc_y(&app, 0));
    assert!(app.espace3d.sel.is_empty() && app.decor3d.choisi != Some(DALLE), "l'objet coupé n'est plus choisi");
    // Collée dans la même séquence : à la même place (elle n'y est plus), le banc remonte dessus.
    app.decor3d_prendre_la_main();
    app.run_action("edition.coller");
    let d = decor(&app, 0).unwrap();
    let c = nomme(d, "Dalle")[0];
    assert_eq!(c.place, dalle.place, "coupée puis collée : à la même place");
    assert!((banc_y(&app, 0) - (sol + 0.4)).abs() < 2e-3, "le banc remonte sur la dalle");
    // Coupée de nouveau puis collée dans l'autre séquence : elle y est déplacée.
    let id = c.id;
    choisir(&mut app, &[id]);
    app.run_action("edition.couper");
    app.select_panel(1);
    app.decor3d_prendre_la_main();
    app.run_action("edition.coller");
    assert!(nomme(decor(&app, 0).unwrap(), "Dalle").is_empty(), "plus dans la première");
    let b = decor(&app, 1).unwrap();
    assert_eq!(b.objets.len(), 1);
    assert_eq!(b.objets[0].place, dalle.place);
}

#[test]
fn plusieurs_objets_se_copient_et_se_collent_ensemble() {
    let mut app = app_avec("plusieurs", deux_sequences());
    let sol = Monde::of(&app.doc).sol_y();
    choisir(&mut app, &[DALLE, BANC]);
    assert_eq!(app.e3_objets_choisis(), vec![DALLE, BANC]);
    app.run_action("edition.copier");
    assert_eq!(app.copie3d.presse.as_ref().unwrap().objets.len(), 2);
    assert!(app.status.contains("2 objets 3D copiés"), "{}", app.status);
    // Dans l'autre séquence : tous deux à leur place, le banc sur la dalle.
    app.select_panel(1);
    app.decor3d_prendre_la_main();
    let avant = app.history.undo_len();
    app.run_action("edition.coller");
    assert_eq!(app.history.undo_len(), avant + 1);
    let b = decor(&app, 1).unwrap();
    assert_eq!(b.objets.len(), 2);
    let (d2, b2) = (nomme(b, "Dalle")[0], nomme(b, "Banc")[0]);
    assert!(proche(d2.place.position, [0.0, sol, -3.0], 1e-5));
    assert!((b2.place.position[1] - (sol + 0.4)).abs() < 2e-3, "le banc collé repose sur la dalle collée");
    assert_ne!(d2.id, b2.id);
    assert_eq!(app.espace3d.sel.len(), 2, "les deux sont choisis");
    // Dans la même séquence : tous deux décalés du même pas (la largeur du groupe et un dixième).
    app.select_panel(0);
    choisir(&mut app, &[DALLE, BANC]);
    app.run_action("edition.copier");
    app.run_action("edition.coller");
    let a = decor(&app, 0).unwrap();
    assert_eq!(a.objets.len(), 4);
    let (d3, b3) = (nomme(a, "Dalle")[1], nomme(a, "Banc")[1]);
    assert!(proche(d3.place.position, [2.2, sol, -3.0], 1e-4), "{:?}", d3.place.position);
    assert!((b3.place.position[0] - 2.2).abs() < 1e-4 && (b3.place.position[1] - (sol + 0.4)).abs() < 2e-3, "{:?}", b3.place.position);
}

// ---------------------------------------------------------------------------------------- vers une autre séquence

#[test]
fn copier_et_envoyer_vers_une_autre_sequence_sont_une_etape_chacun() {
    let mut app = app_avec("envoyer", deux_sequences());
    let sol = Monde::of(&app.doc).sol_y();
    let vers = seq(&app, 1);
    let avant = app.history.undo_len();
    app.decor3d_vers_sequence(&[DALLE], vers, false);
    assert_eq!(app.history.undo_len(), avant + 1, "{}", app.status);
    assert_eq!(decor(&app, 0).unwrap().objets.len(), 2, "copiée : elle reste ici");
    let b = decor(&app, 1).unwrap();
    assert_eq!(nomme(b, "Dalle").len(), 1);
    assert!(app.status.contains("copié vers la séquence"), "{}", app.status);
    // Envoyer le banc : il quitte ce décor, et se pose dans l'autre sur la dalle copiée.
    app.decor3d_vers_sequence(&[BANC], vers, true);
    assert_eq!(app.history.undo_len(), avant + 2);
    assert!(decor(&app, 0).unwrap().objet(BANC).is_none(), "envoyé : il n'est plus ici");
    let b = decor(&app, 1).unwrap();
    let banc = nomme(b, "Banc")[0];
    assert!((banc.place.position[1] - (sol + 0.4)).abs() < 2e-3);
    assert!(app.status.contains("envoyé vers la séquence"), "{}", app.status);
    // Ctrl+Z le ramène, en une fois, des deux côtés.
    app.undo();
    assert!(decor(&app, 0).unwrap().objet(BANC).is_some());
    assert!(nomme(decor(&app, 1).unwrap(), "Banc").is_empty());
    // Vers sa propre séquence : rien.
    let ici = seq(&app, 0);
    let n = app.history.undo_len();
    app.decor3d_vers_sequence(&[DALLE], ici, true);
    assert_eq!(app.history.undo_len(), n);
}

// --------------------------------------------------------------------------------------- au clavier, dans la vue

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

#[test]
fn un_clic_dans_la_vue_donne_la_main_au_decor_et_un_clic_sur_la_feuille_la_reprend() {
    let mut doc = deux_sequences();
    // Un grand mur (6 × 6 m) devant la dalle, pour viser juste.
    let sol = Monde::of(&doc).sol_y();
    let mut mur = objet(950, boite([-3.0, 0.0, -0.5], [3.0, 6.0, 0.5]), 0.0, -1.0);
    mur.place.position = [0.0, sol, -1.0];
    mur.aimante = false;
    mur.nom = "Mur".into();
    let s = doc.sequence_of(0);
    doc.decors3d.par_sequence.get_mut(&s).unwrap().objets.push(mur);
    let mut b = crate::perf_tests::banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.a_cote");
    b.app.espace3d.show_grid = false;
    b.app.espace3d.mode = crate::espace3dgizmo::Mode::Select;
    b.app.espace3d.cam = Some(face());
    b.chauffer(4);
    let rect = b.app.espace3d.rect;
    let view = b.app.espace3d.cam.unwrap().view(rect.width());
    let q = view.project([2.0, sol + 0.6, -0.5], [rect.width() / 2.0, rect.height() / 2.0]).expect("devant l'œil");
    let at = rect.min + egui::vec2(q[0], q[1]);
    b.image(vec![Event::PointerMoved(at)], 0.02);
    b.image(vec![press(at, true)], 0.02);
    b.image(vec![press(at, false)], 0.02);
    b.chauffer(2);
    assert_eq!(b.app.e3_objets_choisis(), vec![950], "le clic choisit le mur : {:?}", b.app.espace3d.sel);
    assert!(b.app.copie3d.main, "un appui dans la vue donne la main au décor");
    b.image(vec![Event::Copy], 0.02);
    b.chauffer(1);
    let presse = b.app.copie3d.presse.as_ref().map(|p| p.objets.iter().map(|o| o.id).collect::<Vec<_>>());
    assert_eq!(presse, Some(vec![950]), "Ctrl+C copie l'objet choisi dans la vue : {}", b.app.status);
    // Un appui sur la feuille reprend la main : Ctrl+C y garde son sens d'avant (le mur reste choisi dans la vue).
    let feuille = b.app.canvas_rect.center();
    b.image(vec![Event::PointerMoved(feuille)], 0.02);
    b.image(vec![press(feuille, true)], 0.02);
    b.image(vec![press(feuille, false)], 0.02);
    b.chauffer(2);
    assert!(!b.app.copie3d.main, "la feuille reprend la main");
    b.image(vec![Event::Copy], 0.02);
    b.chauffer(1);
    assert!(!b.app.copie3d.recente, "une autre copie : Ctrl+V ne vise plus les objets 3D");
}

// --------------------------------------------------------------------------------- Ctrl+C / Ctrl+V d'avant, ailleurs

#[test]
fn hors_du_decor_ctrl_c_et_ctrl_v_gardent_leur_sens_sur_les_dessins_les_calques_et_les_plans() {
    let mut app = app_avec("avant", deux_sequences());
    // Un objet du décor reste choisi dans la vue, mais le dernier appui était ailleurs.
    app.e3_choisir_objet(DALLE, false);
    assert!(!app.copie3d.main);
    // Le dessin : une image collée dans le calque vectoriel, choisie ; Ctrl+C la copie, Ctrl+V la colle.
    app.paste_clipboard_image((10, 10, vec![200; 10 * 10 * 4]));
    assert!(app.vsel.is_some());
    app.run_action("edition.copier");
    assert!(app.copie3d.presse.is_none(), "aucun objet 3D copié");
    assert_eq!(app.clipboard.len(), 1, "la sélection du dessin est copiée");
    let cel = app.layers()[0].frames[0].unwrap();
    let elements = |app: &App| app.doc.cels[&cel].vector.as_ref().unwrap().elements.len();
    let n = elements(&app);
    app.run_action("edition.coller");
    assert_eq!(elements(&app), n + 1, "Ctrl+V colle l'élément dans le dessin");
    // Le panneau Calques : Ctrl+C copie le calque, Ctrl+V le colle.
    app.layers_pane_focus = true;
    app.run_action("edition.copier");
    assert!(app.layer_clipboard.is_some() && app.copie3d.presse.is_none(), "{}", app.status);
    let couches = app.layers().len();
    app.run_action("edition.coller");
    assert_eq!(app.layers().len(), couches + 1);
    app.layers_pane_focus = false;
    app.select_layer(0);
    // Le montage : Ctrl+C copie le plan.
    app.panel_focus = true;
    app.run_action("edition.copier");
    assert!(app.panel_clipboard.is_some() && app.copie3d.presse.is_none());
    app.panel_focus = false;
    // Le décor a la main mais rien n'y est choisi (un clic sur une carte de la vue) : Ctrl+C copie le dessin, comme avant.
    app.espace3d.sel.clear();
    app.decor3d_prendre_la_main();
    app.clipboard.clear();
    app.paste_clipboard_image((10, 10, vec![100; 10 * 10 * 4]));
    app.run_action("edition.copier");
    assert!(app.copie3d.presse.is_none() && app.clipboard.len() == 1, "{}", app.status);
    // Des objets 3D copiés, puis une copie du dessin : Ctrl+V colle le dessin, même avec la main au décor.
    choisir(&mut app, &[DALLE]);
    app.run_action("edition.copier");
    assert!(app.copie3d.recente && app.copie3d.presse.is_some());
    app.copie3d.main = false;
    app.paste_clipboard_image((10, 10, vec![50; 10 * 10 * 4]));
    app.run_action("edition.copier");
    assert!(!app.copie3d.recente, "la dernière copie est celle du dessin");
    app.decor3d_prendre_la_main();
    let (n, objets) = (elements(&app), decor(&app, 0).unwrap().objets.len());
    app.run_action("edition.coller");
    assert_eq!(elements(&app), n + 1, "Ctrl+V colle le dessin copié en dernier");
    assert_eq!(decor(&app, 0).unwrap().objets.len(), objets, "pas d'objet 3D");
}

// ------------------------------------------------------------------------------------ une séquence faite d'une autre

/// Trois plans dans une séquence au décor « dalle et banc ».
fn trois_plans() -> App {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    let sol = Monde::of(&doc).sol_y();
    let s = doc.sequence_of(0);
    assert!((0..3).all(|i| doc.sequence_of(i) == s));
    doc.decors3d.par_sequence.insert(s, dalle_et_banc(sol));
    app_avec("trois", doc)
}

/// Le décor `b` est une copie indépendante de `a` : un autre décor, les mêmes modèles, d'autres numéros, les mêmes places.
fn copie_de(a: &Decor3d, b: &Decor3d) {
    assert_ne!(a.uid, b.uid);
    assert_eq!(a.aspect, b.aspect);
    assert_eq!(a.objets.len(), b.objets.len());
    for (x, y) in a.objets.iter().zip(&b.objets) {
        assert_ne!(x.id, y.id, "des numéros neufs");
        assert!(Arc::ptr_eq(&x.modele, &y.modele), "le même modèle");
        assert_eq!((&x.nom, x.place, x.origine, x.aimante), (&y.nom, y.place, y.origine, y.aimante));
    }
}

#[test]
fn couper_la_sequence_emporte_une_copie_du_decor_independante() {
    let mut app = trois_plans();
    app.select_panel(2);
    let avant = app.history.undo_len();
    app.sequence_couper_ici();
    assert_eq!(app.history.undo_len(), avant + 1, "{}", app.status);
    assert_ne!(seq(&app, 2), seq(&app, 0), "une séquence neuve");
    let (a, b) = (decor(&app, 0).unwrap().clone(), decor(&app, 2).expect("elle a le décor qu'elle montrait").clone());
    copie_de(&a, &b);
    // Indépendante : déplacer un objet de la copie laisse l'original.
    let s2 = seq(&app, 2);
    app.doc.decors3d.par_sequence.get_mut(&s2).unwrap().objets[0].place.position[0] += 1.0;
    assert_eq!(decor(&app, 0).unwrap().objets[0].place, a.objets[0].place);
    // Ctrl+Z : une seule séquence, un seul décor.
    app.undo();
    assert_eq!(seq(&app, 2), seq(&app, 0));
    assert_eq!(app.doc.decors3d.par_sequence.len(), 1);
}

#[test]
fn une_sequence_faite_de_plans_choisis_et_la_suite_emportent_chacune_une_copie_du_decor() {
    let mut app = trois_plans();
    app.select_panel(1);
    app.board_sel = vec![app.doc.panels[1].id];
    app.new_sequence_from_cases();
    let ids: Vec<u64> = (0..3).map(|i| seq(&app, i)).collect();
    assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2], "trois séquences : {ids:?} ({})", app.status);
    let a = decor(&app, 0).unwrap().clone();
    copie_de(&a, decor(&app, 1).unwrap());
    copie_de(&a, decor(&app, 2).unwrap());
    let tous: std::collections::BTreeSet<u64> = app.doc.decors3d.par_sequence.values().flat_map(|d| d.objets.iter().map(|o| o.id)).collect();
    assert_eq!(tous.len(), 6, "aucun numéro en double");
    app.undo();
    assert_eq!(app.doc.decors3d.par_sequence.len(), 1);
}

#[test]
fn supprimer_une_sequence_et_l_annuler_rend_son_decor() {
    let mut app = app_avec("supprimer", deux_sequences());
    app.decor3d_vers_sequence(&[DALLE], seq(&app, 1), false);
    let s = seq(&app, 1);
    let objets = decor(&app, 1).unwrap().objets.iter().map(|o| o.id).collect::<Vec<_>>();
    let id = app.doc.panels[1].id;
    app.delete_panels(&[id]);
    assert_eq!(app.doc.panels.len(), 1, "{}", app.status);
    app.undo();
    assert_eq!(app.doc.panels.len(), 2);
    assert_eq!(seq(&app, 1), s);
    assert_eq!(decor(&app, 1).unwrap().objets.iter().map(|o| o.id).collect::<Vec<_>>(), objets);
}

// --------------------------------------------------------------------------------------- l'enregistrement, les projets

#[test]
fn apres_un_collage_le_projet_s_enregistre_avec_un_fichier_par_modele_et_se_relit() {
    let mut app = app_avec("enregistrer", deux_sequences());
    choisir(&mut app, &[DALLE, BANC]);
    app.run_action("edition.copier");
    app.run_action("edition.coller");
    app.select_panel(1);
    app.decor3d_prendre_la_main();
    app.run_action("edition.coller");
    let dir = crate::decor3d_tests::dossier("copiedecor-enregistrer");
    let manifeste = dir.join("Film.shot");
    crate::project::save(&app.doc, &manifeste, &mut Default::default()).unwrap();
    let fichiers: Vec<_> = std::fs::read_dir(dir.join(crate::decor3d::DIR)).unwrap().flatten().collect();
    assert_eq!(fichiers.len(), 2, "deux modèles (la dalle, le banc), six objets : deux fichiers");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    for pi in 0..2 {
        let (x, y) = (app.doc.decor_du_plan(pi).unwrap(), relu.doc.decor_du_plan(pi).unwrap());
        assert_eq!(x.objets.len(), y.objets.len());
        for (a, b) in x.objets.iter().zip(&y.objets) {
            assert_eq!((a.id, &a.nom, a.place, a.origine, a.aimante), (b.id, &b.nom, b.place, b.origine, b.aimante));
        }
    }
    let dalles: Vec<&Objet3d> = (0..2).flat_map(|pi| nomme(relu.doc.decor_du_plan(pi).unwrap(), "Dalle")).collect();
    assert_eq!(dalles.len(), 3);
    assert!(dalles.iter().all(|o| Arc::ptr_eq(&o.modele, &dalles[0].modele)), "relues : un seul modèle pour les trois dalles");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn coller_dans_un_autre_projet_ouvert_emporte_le_modele_et_l_enregistre_avec_lui() {
    let mut b = crate::perf_tests::banc(deux_sequences(), false);
    b.chauffer(2);
    // Le second projet : un plan, sans décor.
    let mut autre = Document::with_frame(160, 90, 24);
    autre.panels[0].length = 24;
    assert!(b.app.nouvel_onglet(), "{}", b.app.status);
    assert!(b.app.charger_projet(autre));
    b.app.welcome = false;
    b.chauffer(2);
    assert!(b.app.activer_onglet(0));
    b.app.select_panel(0);
    let (dalle, banc) = {
        let d = decor(&b.app, 0).unwrap();
        (d.objet(DALLE).unwrap().clone(), d.objet(BANC).unwrap().clone())
    };
    choisir(&mut b.app, &[DALLE, BANC]);
    b.app.run_action("edition.copier");
    assert!(b.app.activer_onglet(1));
    b.app.select_panel(0);
    b.app.decor3d_prendre_la_main();
    let avant = b.app.history.undo_len();
    b.app.run_action("edition.coller");
    assert_eq!(b.app.history.undo_len(), avant + 1, "{}", b.app.status);
    let d = decor(&b.app, 0).expect("le décor collé dans l'autre projet").clone();
    assert_eq!(d.objets.len(), 2);
    let (d2, b2) = (nomme(&d, "Dalle")[0], nomme(&d, "Banc")[0]);
    assert!(Arc::ptr_eq(&d2.modele, &dalle.modele) && Arc::ptr_eq(&b2.modele, &banc.modele), "le modèle partagé, pas relu");
    assert_eq!(d2.place, dalle.place, "à la même place (un autre projet)");
    // Enregistré, relu : le modèle est dans ce projet, sous son empreinte.
    let dir = crate::decor3d_tests::dossier("copiedecor-autre-projet");
    let manifeste = dir.join("Autre.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).unwrap();
    for o in [&dalle, &banc] {
        let f = dir.join(crate::decor3d::DIR).join(format!("{}.glb", o.modele.empreinte));
        assert_eq!(std::fs::read(&f).expect("le modèle est enregistré avec ce projet"), *o.modele.octets);
    }
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let r = relu.doc.decor_du_plan(0).unwrap();
    assert_eq!(r.objets.len(), 2);
    assert_eq!(nomme(r, "Dalle")[0].place, dalle.place);
    std::fs::remove_dir_all(&dir).ok();
    // Le premier projet n'a pas changé.
    assert!(b.app.activer_onglet(0));
    assert_eq!(decor(&b.app, 0).unwrap().objets.len(), 2);
    // Un projet qui a déjà ce contenu (lu à part, un autre `Arc`) : c'est son modèle qui sert, un seul par contenu.
    let mut troisieme = Document::with_frame(160, 90, 24);
    let sol = Monde::of(&troisieme).sol_y();
    let mut sienne = objet(5, boite([-1.0, 0.0, -1.0], [1.0, 0.4, 1.0]), 4.0, -3.0);
    sienne.place.position = [4.0, sol, -3.0];
    assert_eq!(sienne.modele.empreinte, dalle.modele.empreinte);
    assert!(!Arc::ptr_eq(&sienne.modele, &dalle.modele));
    let s = troisieme.sequence_of(0);
    let mut d3 = Decor3d::new(Default::default());
    d3.objets.push(sienne);
    troisieme.decors3d.par_sequence.insert(s, d3);
    assert!(b.app.nouvel_onglet());
    assert!(b.app.charger_projet(troisieme));
    b.app.welcome = false;
    b.chauffer(2);
    b.app.select_panel(0);
    b.app.decor3d_prendre_la_main();
    b.app.run_action("edition.coller");
    let d = decor(&b.app, 0).unwrap();
    assert_eq!(d.objets.len(), 3, "{}", b.app.status);
    let dalles = d.objets.iter().filter(|o| o.modele.empreinte == dalle.modele.empreinte).collect::<Vec<_>>();
    assert_eq!(dalles.len(), 2);
    assert!(Arc::ptr_eq(&dalles[0].modele, &dalles[1].modele), "le modèle déjà dans le projet est repris");
}

// -------------------------------------------------------------------------------------------------------- le budget

#[test]
fn un_collage_qui_ferait_depasser_le_budget_de_la_machine_est_refuse() {
    let mut app = app_avec("budget", deux_sequences());
    // Un budget de 30 triangles : la dalle et le banc en ont 24.
    app.settings.decor3d = Some(crate::decor3dbudget::BudgetMachine { triangles: 30, ..crate::decor3d_tests::budget_large() });
    choisir(&mut app, &[DALLE]);
    app.run_action("edition.copier");
    let (avant, n) = (app.history.undo_len(), decor(&app, 0).unwrap().objets.len());
    app.run_action("edition.coller");
    assert_eq!(decor(&app, 0).unwrap().objets.len(), n, "refusé");
    assert_eq!(app.history.undo_len(), avant);
    assert!(app.status.starts_with("Refusé"), "{}", app.status);
    // Dans l'autre séquence, vide : il tient.
    app.select_panel(1);
    app.decor3d_prendre_la_main();
    app.run_action("edition.coller");
    assert_eq!(decor(&app, 1).unwrap().objets.len(), 1, "{}", app.status);
}
