//! Essais des boutons de clés du panneau Réglages de l'Espace 3D (`espace3dcles.rs`, `docs/INTERFACE.md` partie 145) :
//! la clé de caméra et la clé de position des cartes choisies, dans un plan ordinaire et dans un plan en Espace 3D.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::camera::Camera;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d;
use crate::espace3dcles::{CARTES_VERROUILLEES, CLE_DE_CAMERA_DEJA_LA};
use crate::espace3dgizmo::CAMERA_ID;
use crate::perf_tests::{banc_taille, Banc};

/// Un projet d'essai : cadre 320 × 180 sur une feuille 640 × 360, `n` calques bitmap de 12 images, « Carte 0 » en bas.
fn doc_cartes(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for k in 0..n {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        layer.frames = vec![Some(cel); 12];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Le banc, la vue Espace 3D ouverte, le calque 0 courant (sa carte est choisie, comme à l'écran).
fn ouvert(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.espace3d.on && !b.app.camera_view, "la vue Espace 3D est ouverte");
    b
}

/// Choisit ces cartes dans la vue (par le rang de leur calque), sans changer le calque courant.
fn choisir(b: &mut Banc, lis: &[usize]) {
    b.app.espace3d.sel = lis.iter().map(|&li| espace3d::card_selection(b.app.layers()[li].id)).collect();
}

/// Les images des clés de position du calque `li`.
fn cles(b: &Banc, li: usize) -> Vec<usize> {
    b.app.layers()[li].pos_keys.iter().map(|k| k.frame).collect()
}

/// Les images des clés de caméra du plan.
fn cles_camera(b: &Banc) -> Vec<usize> {
    b.app.doc.panels[0].camera.iter().map(|k| k.frame).collect()
}

// ------------------------------------------------------------------------------------ le panneau Réglages, à la souris

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

/// Une image du panneau Réglages, dans un contexte à part, avec ces événements : les textes peints et leur place.
fn image_du_panneau(ctx: &egui::Context, app: &mut App, t: &mut f64, events: Vec<Event>) -> Vec<(String, Rect)> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 900.0))), time: Some(*t), events, ..RawInput::default() };
    *t += 1.0 / 60.0;
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
    });
    let mut textes = Vec::new();
    for s in &out.shapes {
        collecter(&s.shape, &mut textes);
    }
    textes
}

/// Le panneau au repos (quelques images, le temps que sa mise en page se pose).
fn textes_du_panneau(ctx: &egui::Context, app: &mut App, t: &mut f64) -> Vec<(String, Rect)> {
    let mut textes = Vec::new();
    for _ in 0..4 {
        textes = image_du_panneau(ctx, app, t, Vec::new());
    }
    textes
}

/// La place du bouton `icone` sur la ligne du texte `ligne` (le texte entier, ou sa fin).
fn bouton_de_la_ligne(textes: &[(String, Rect)], ligne: &str, icone: &str) -> Pos2 {
    let y = textes.iter().find(|(s, _)| s.trim().ends_with(ligne)).map(|(_, r)| r.center().y).unwrap_or_else(|| panic!("la ligne « {ligne} » est dans le panneau : {textes:?}"));
    textes
        .iter()
        .find(|(s, r)| s.trim() == icone && (r.center().y - y).abs() < 12.0)
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| panic!("le bouton {icone:?} est sur la ligne « {ligne} » : {textes:?}"))
}

/// Un clic dans le panneau.
fn cliquer(ctx: &egui::Context, app: &mut App, t: &mut f64, p: Pos2) {
    let bouton = |pressed: bool| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    for ev in [vec![Event::PointerMoved(p)], vec![bouton(true)], vec![bouton(false)], Vec::new()] {
        image_du_panneau(ctx, app, t, ev);
    }
}

#[test]
fn the_settings_of_the_3d_view_carry_the_key_buttons_and_a_click_poses_and_removes_the_key() {
    let mut b = ouvert(doc_cartes(3));
    b.app.cur_frame = 5;
    // La carte 2 est choisie ; le calque courant reste le calque 0.
    choisir(&mut b, &[2]);
    let (ctx, mut t) = (egui::Context::default(), 0.0);
    let ligne_carte = format!("{} Carte 2", ph::SQUARE);
    let textes = textes_du_panneau(&ctx, &mut b.app, &mut t);
    // Les deux boutons sont sur la ligne de la caméra et sur celle de la carte choisie.
    for ligne in ["Caméra du plan", ligne_carte.as_str()] {
        let (losange, corbeille) = (bouton_de_la_ligne(&textes, ligne, ph::DIAMOND), bouton_de_la_ligne(&textes, ligne, ph::TRASH));
        assert!(losange.x < corbeille.x, "« {ligne} » : le losange, puis la corbeille");
    }
    // Le losange de la caméra : une clé à l'image courante (la caméra fixe devient la clé du début).
    cliquer(&ctx, &mut b.app, &mut t, bouton_de_la_ligne(&textes, "Caméra du plan", ph::DIAMOND));
    assert_eq!(cles_camera(&b), vec![0, 5], "{}", b.app.status);
    // Le losange de la carte : une clé de position pour la carte choisie, pas pour le calque courant.
    let textes = textes_du_panneau(&ctx, &mut b.app, &mut t);
    cliquer(&ctx, &mut b.app, &mut t, bouton_de_la_ligne(&textes, &ligne_carte, ph::DIAMOND));
    assert_eq!(cles(&b, 2), vec![5], "{}", b.app.status);
    assert!(cles(&b, 0).is_empty() && cles(&b, 1).is_empty());
    // Les corbeilles les retirent.
    let textes = textes_du_panneau(&ctx, &mut b.app, &mut t);
    cliquer(&ctx, &mut b.app, &mut t, bouton_de_la_ligne(&textes, "Caméra du plan", ph::TRASH));
    assert_eq!(cles_camera(&b), vec![0], "{}", b.app.status);
    let textes = textes_du_panneau(&ctx, &mut b.app, &mut t);
    cliquer(&ctx, &mut b.app, &mut t, bouton_de_la_ligne(&textes, &ligne_carte, ph::TRASH));
    assert!(cles(&b, 2).is_empty(), "{}", b.app.status);
    // Plusieurs cartes choisies : les boutons sont sur la ligne « N cartes choisies », et servent toutes les cartes.
    choisir(&mut b, &[1, 2]);
    let textes = textes_du_panneau(&ctx, &mut b.app, &mut t);
    cliquer(&ctx, &mut b.app, &mut t, bouton_de_la_ligne(&textes, "2 cartes choisies", ph::DIAMOND));
    assert_eq!((cles(&b, 1), cles(&b, 2)), (vec![5], vec![5]), "{}", b.app.status);
    // La vue tracée par le banc, les boutons allumés : rien ne casse.
    b.chauffer(4);
}

// ------------------------------------------------------------------------------------ la clé de caméra

#[test]
fn the_camera_key_button_poses_a_key_here_and_moving_the_camera_at_another_image_poses_its_own() {
    let mut b = ouvert(doc_cartes(1));
    assert!(b.app.doc.panels[0].camera.is_empty(), "caméra fixe, sans clé");
    b.app.cur_frame = 4;
    assert!(b.app.camera_key_here().is_none(), "le losange est éteint");
    b.app.espace3d_cle_camera_ici();
    // La caméra fixe devient la clé du début ; la clé d'ici a la même caméra : l'image ne change pas.
    assert_eq!(cles_camera(&b), vec![0, 4]);
    let keys = b.app.doc.panels[0].camera.clone();
    assert_eq!(keys[0].cam, keys[1].cam);
    assert!(b.app.camera_key_here().is_some(), "le losange est allumé");
    // Déjà allumé : rien ne change, la barre d'état le dit.
    b.app.espace3d_cle_camera_ici();
    assert_eq!(b.app.doc.panels[0].camera, keys);
    assert_eq!(b.app.status, CLE_DE_CAMERA_DEJA_LA);
    // Une seule étape d'annulation pour la clé posée.
    b.app.run_action("edition.annuler");
    assert!(b.app.doc.panels[0].camera.is_empty(), "annulé : {:?}", cles_camera(&b));
    b.app.run_action("edition.retablir");
    assert_eq!(cles_camera(&b), vec![0, 4]);
    // À une autre image, déplacer la caméra pose la clé de cette image.
    b.app.cur_frame = 8;
    assert!(b.app.camera_key_here().is_none());
    let cam = Camera { x: b.app.current_camera().x + 40.0, ..b.app.current_camera() };
    b.app.set_current_camera(cam);
    assert_eq!(cles_camera(&b), vec![0, 4, 8]);
    assert!(b.app.camera_key_here().is_some());
    // La corbeille (`camera.cle_supprimer`) retire la clé d'ici, en une étape.
    b.app.run_action("camera.cle_supprimer");
    assert_eq!(cles_camera(&b), vec![0, 4]);
    b.app.run_action("edition.annuler");
    assert_eq!(cles_camera(&b), vec![0, 4, 8]);
}

#[test]
fn in_a_plan_in_space_the_camera_key_keeps_the_free_camera() {
    let mut doc = doc_cartes(1);
    doc.panels[0].espace = true;
    let mut b = ouvert(doc);
    b.app.cur_frame = 3;
    b.app.espace3d_cle_camera_ici();
    assert_eq!(cles_camera(&b), vec![0, 3]);
    // À l'image 9, la caméra passe derrière les cartes : sa clé garde le demi-tour (la caméra n'est pas bridée).
    b.app.cur_frame = 9;
    let libre = Camera { pan: 150.0, tilt: -60.0, ..b.app.current_camera() };
    b.app.set_current_camera(libre);
    assert_eq!(cles_camera(&b), vec![0, 3, 9]);
    let cle = b.app.doc.panels[0].camera[2];
    assert_eq!((cle.cam.pan, cle.cam.tilt), (150.0, -60.0));
    // Les clés d'avant n'ont pas bougé.
    assert_eq!(b.app.doc.panels[0].camera[1].cam.pan, 0.0);
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------ la clé de position

#[test]
fn the_position_key_goes_to_the_chosen_cards_all_together_in_one_undo_step() {
    let mut b = ouvert(doc_cartes(3));
    // Une carte choisie qui n'est pas le calque courant : la clé est pour elle.
    choisir(&mut b, &[2]);
    b.app.cur_frame = 3;
    b.app.run_action("calque.cle_position");
    assert_eq!((cles(&b, 0), cles(&b, 1), cles(&b, 2)), (vec![], vec![], vec![3]), "{}", b.app.status);
    assert!(b.app.status.starts_with("Clé de position posée"), "{}", b.app.status);
    // Deux cartes choisies : chacune reçoit sa clé, en une seule étape d'annulation.
    choisir(&mut b, &[1, 2]);
    b.app.cur_frame = 7;
    b.app.run_action("calque.cle_position");
    assert_eq!((cles(&b, 1), cles(&b, 2)), (vec![7], vec![3, 7]), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!((cles(&b, 1), cles(&b, 2)), (vec![], vec![3]), "une étape pour les deux cartes");
    b.app.run_action("edition.retablir");
    assert_eq!((cles(&b, 1), cles(&b, 2)), (vec![7], vec![3, 7]));
    // La corbeille (`calque.cle_position_supprimer`) retire la clé d'ici des cartes choisies, en une étape.
    b.app.pos_keys_delete_here();
    assert_eq!((cles(&b, 1), cles(&b, 2)), (vec![], vec![3]), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!((cles(&b, 1), cles(&b, 2)), (vec![7], vec![3, 7]));
    // La caméra seule choisie : la commande vise le calque courant, comme hors de la vue.
    b.app.espace3d.sel = vec![CAMERA_ID];
    assert!(b.app.e3_cartes_choisies().is_none());
    b.app.run_action("calque.cle_position");
    assert_eq!(cles(&b, 0), vec![7], "{}", b.app.status);
    // Vue fermée : le choix des cartes ne compte plus, c'est le calque courant.
    choisir(&mut b, &[1]);
    b.app.run_action("espace3d.basculer");
    assert!(!b.app.espace3d.on && b.app.e3_cartes_choisies().is_none());
    b.app.cur_frame = 10;
    b.app.run_action("calque.cle_position");
    assert_eq!((cles(&b, 0), cles(&b, 1)), (vec![7, 10], vec![7]), "{}", b.app.status);
}

#[test]
fn a_locked_card_takes_no_position_key_and_the_status_bar_says_why() {
    let mut b = ouvert(doc_cartes(3));
    b.app.doc.panels[0].layers[1].locked = true;
    b.app.doc.panels[0].layers[2].position_locked = true;
    b.app.cur_frame = 2;
    for li in [1, 2] {
        choisir(&mut b, &[li]);
        assert_eq!(b.app.e3_cartes_des_cles(), Some(Vec::new()));
        b.app.run_action("calque.cle_position");
        assert!(cles(&b, li).is_empty(), "carte {li}");
        assert_eq!(b.app.status, CARTES_VERROUILLEES);
    }
    // Choisies avec une carte libre : seule la carte libre reçoit la clé.
    choisir(&mut b, &[0, 1, 2]);
    b.app.run_action("calque.cle_position");
    assert_eq!((cles(&b, 0), cles(&b, 1), cles(&b, 2)), (vec![2], vec![], vec![]), "{}", b.app.status);
    // Une clé posée avant le verrou reste : la corbeille ne la retire pas tant que la carte est verrouillée.
    b.app.doc.panels[0].layers[0].locked = true;
    choisir(&mut b, &[0]);
    b.app.pos_keys_delete_here();
    assert_eq!(cles(&b, 0), vec![2], "{}", b.app.status);
    b.chauffer(3);
}

#[test]
fn in_a_plan_in_space_the_key_of_a_card_on_the_ground_keeps_its_tilt_and_its_depth() {
    let mut doc = doc_cartes(1);
    doc.panels[0].espace = true;
    let mut b = ouvert(doc);
    b.app.run_action("espace3d.surface_sol");
    assert_eq!(b.app.layers().len(), 2, "{}", b.app.status);
    // La surface neuve est le calque courant et la carte choisie.
    let li = b.app.cur_layer;
    assert_eq!(b.app.e3_cartes_choisies(), Some(vec![li]));
    let couchee = |b: &Banc, k: usize| {
        let t = b.app.layers()[li].pos_keys[k].t;
        t.tilt_x.abs().max(t.tilt_y.abs())
    };
    let pose0 = b.app.layers()[li].pose(0);
    let profondeur0 = b.app.layers()[li].depth_at(0);
    assert!((pose0.tilt_x.abs().max(pose0.tilt_y.abs()) - 90.0).abs() < 0.01, "au sol : {pose0:?}");
    b.app.cur_frame = 0;
    b.app.run_action("calque.cle_position");
    assert_eq!(cles(&b, li), vec![0], "{}", b.app.status);
    // La clé garde la pose entière : la carte n'a pas bougé, elle est toujours couchée, à la même profondeur.
    assert_eq!(b.app.layers()[li].pose(0), pose0);
    assert!((couchee(&b, 0) - 90.0).abs() < 0.01);
    assert!((b.app.layers()[li].depth_at(0) - profondeur0).abs() < 1e-4);
    // À l'image 6, la carte est déplacée dans l'espace : elle est animée, sa clé de cette image se pose toute seule.
    b.app.cur_frame = 6;
    let w = b.app.e3_world();
    let l = &b.app.layers()[li];
    let mut xf = w.card_xf(&l.pose(6), l.depth_at(6));
    xf.pos[0] += 1.0;
    let id = espace3d::card_selection(l.id);
    b.app.e3_set_xfs(&[(id, xf)]);
    assert_eq!(cles(&b, li), vec![0, 6]);
    assert!((couchee(&b, 1) - 90.0).abs() < 0.01, "toujours au sol à l'image 6");
    assert_ne!(b.app.layers()[li].pose(6).x, pose0.x, "elle a glissé");
    assert_eq!(b.app.layers()[li].pose(0), pose0, "la clé du début n'a pas bougé");
    // Entre les deux clés, elle reste couchée.
    let milieu = b.app.layers()[li].pose(3);
    assert!((milieu.tilt_x.abs().max(milieu.tilt_y.abs()) - 90.0).abs() < 0.01, "{milieu:?}");
    b.chauffer(3);
}
