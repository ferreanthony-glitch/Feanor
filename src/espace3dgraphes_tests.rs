//! Essais des graphiques du panneau Espace 3D (`espace3dgraphes.rs`, `docs/INTERFACE.md` partie 162) : chaque graphique change
//! la scène quand on le manipule (le plateau, la profondeur ; les pictogrammes, l'orientation ; la réglette, la distance ; le
//! curseur, la taille ; les vignettes et le cône, la focale ; la vue de côté, la hauteur et la bascule ; le cadran, le
//! panoramique ; l'horizon, la rotation), en une étape d'annulation par geste ; au repos, rien ne change ; rien ne se montre
//! hors de la vue de la scène ; chaque réglage d'avant reste joignable.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};

use crate::app::App;
use crate::bulles::vus;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d;
use crate::espace3ddecor::Sorte3d;
use crate::espace3dgizmo::CAMERA_ID;
use crate::perf_tests::{banc_taille, Banc};

// ------------------------------------------------------------------------------------ le banc

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

/// Le banc, la vue Espace 3D ouverte, le calque 0 courant, rien de choisi.
fn ouvert(n: usize) -> Banc {
    let mut b = banc_taille(doc_cartes(n), false, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.espace3d_vue_ouverte(), "la vue Espace 3D est ouverte");
    b.app.espace3d.sel.clear();
    b
}

/// Le panneau Espace 3D tel que le dock le dessine, dans un contexte à part de 420 × 1 600.
struct Panneau {
    ctx: egui::Context,
    t: f64,
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

impl Panneau {
    fn new() -> Panneau {
        Panneau { ctx: egui::Context::default(), t: 0.0 }
    }

    /// Une image du panneau, avec ces événements ; `contenu` : ce qu'on dessine (d'ordinaire le panneau Espace 3D).
    fn image_de(&mut self, app: &mut App, events: Vec<Event>, contenu: &dyn Fn(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        vus::vider();
        // (Comme l'application au début de chaque image : un geste que rien n'a prolongé est fini, `App::end_gesture_if_idle`.)
        app.gesture.end_frame(false);
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 1600.0))), time: Some(self.t), events, ..RawInput::default() };
        self.t += 1.0 / 60.0;
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| contenu(app, ui));
        });
        let mut textes = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.image_de(app, events, &|app, ui| app.pane_ui(ui, crate::dock::Pane::Espace3d))
    }

    /// Quelques images au repos : les textes peints.
    fn repos(&mut self, app: &mut App) -> Vec<(String, Rect)> {
        let mut textes = Vec::new();
        for _ in 0..4 {
            textes = self.image(app, Vec::new());
        }
        textes
    }

    /// Le rectangle de l'élément dont la bulle s'appelle `nom`.
    fn ou(&mut self, app: &mut App, nom: &str) -> Rect {
        self.repos(app);
        *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » est dans le panneau : {:?}", vus::noms()))
    }

    fn cliquer(&mut self, app: &mut App, p: Pos2) {
        let bouton = |pressed: bool| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        for ev in [vec![Event::PointerMoved(p)], vec![bouton(true)], vec![bouton(false)], Vec::new()] {
            self.image(app, ev);
        }
    }

    /// Un glissé le long de `chemin` (le bouton enfoncé au premier point, lâché au dernier).
    fn glisser(&mut self, app: &mut App, chemin: &[Pos2]) {
        let bouton = |p: Pos2, pressed: bool| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let (debut, fin) = (chemin[0], *chemin.last().expect("un chemin"));
        self.image(app, vec![Event::PointerMoved(debut)]);
        self.image(app, vec![bouton(debut, true)]);
        for &p in &chemin[1..] {
            self.image(app, vec![Event::PointerMoved(p)]);
        }
        self.image(app, vec![bouton(fin, false)]);
        self.image(app, Vec::new());
    }
}

/// Le chemin de `a` à `b` en `n` pas.
fn droit(a: Pos2, b: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n).map(|k| a + (b - a) * (k as f32 / n as f32)).collect()
}

fn etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn profondeur(b: &Banc, li: usize) -> f32 {
    let w = b.app.e3_world();
    w.profondeur(&b.app.layers()[li], b.app.cur_frame)
}

fn camera(b: &Banc) -> espace3d::SceneCam {
    b.app.e3_world().scene_cam(&b.app.current_camera())
}

// ------------------------------------------------------------------------------------ la scène : le plateau

#[test]
fn dragging_a_layer_in_the_top_view_moves_it_in_depth_in_one_undo_step_and_a_click_chooses_it() {
    let mut b = ouvert(3);
    let mut p = Panneau::new();
    let avant = etapes(&b);
    let textes = p.repos(&mut b.app);
    // Rien de choisi : la scène. Les trois calques sont à la profondeur de la feuille : un seul trait, au nom du calque courant.
    assert!(textes.iter().any(|(s, _)| s == "Ajouter à la scène"), "{textes:?}");
    let (_, nom) = textes.iter().find(|(s, _)| s.starts_with("Carte 0 +2")).unwrap_or_else(|| panic!("le trait des calques : {textes:?}")).clone();
    assert_eq!(etapes(&b), avant, "au repos, le plateau ne change rien");
    // Glisser le trait vers le haut : le calque courant recule ; les autres ne bougent pas.
    let depart = pos2(nom.left() - 20.0, nom.center().y);
    p.glisser(&mut b.app, &droit(depart, depart - vec2(0.0, 40.0), 8));
    let d0 = profondeur(&b, 0);
    assert!(d0 > 0.05, "le calque a reculé : {d0}");
    assert_eq!((profondeur(&b, 1), profondeur(&b, 2)), (0.0, 0.0), "les autres calques n'ont pas bougé");
    let apres = etapes(&b);
    assert_eq!(apres.len(), avant.len() + 1, "une seule étape pour tout le geste : {apres:?}");
    assert_eq!(apres.last().map(String::as_str), Some("Distance du calque"));
    // Ctrl+Z rend la profondeur d'avant.
    b.app.run_action("edition.annuler");
    assert_eq!(profondeur(&b, 0), 0.0);
    b.app.run_action("edition.retablir");
    assert!((profondeur(&b, 0) - d0).abs() < 1e-5);
    // Un clic sur un trait choisit son calque : le panneau passe au calque choisi.
    let textes = p.repos(&mut b.app);
    let (_, r) = textes.iter().find(|(s, _)| s.starts_with("Carte 0")).unwrap_or_else(|| panic!("{textes:?}")).clone();
    p.cliquer(&mut b.app, pos2(r.left() - 20.0, r.center().y));
    assert_eq!(b.app.espace3d.sel, vec![espace3d::card_selection(b.app.layers()[0].id)], "{}", b.app.status);
    let textes = p.repos(&mut b.app);
    assert!(textes.iter().any(|(s, _)| s == "Comment il est posé"), "le calque choisi : {textes:?}");
    // « Revenir à la scène » : plus rien de choisi.
    let r = p.ou(&mut b.app, "Revenir à la scène");
    p.cliquer(&mut b.app, r.center());
    assert!(b.app.espace3d.sel.is_empty());
    // Un nouveau clic sur le même trait prend le calque suivant du groupe (les trois calques sont au même endroit... sauf le
    // calque 0, reculé : son trait est seul ; on clique celui des deux autres).
    let textes = p.repos(&mut b.app);
    let (_, r) = textes.iter().find(|(s, _)| s.starts_with("Carte 2 +1")).unwrap_or_else(|| panic!("{textes:?}")).clone();
    let ici = pos2(r.left() - 20.0, r.center().y);
    p.cliquer(&mut b.app, ici);
    assert_eq!(b.app.cur_layer, 2, "le haut de la pile d'abord (le calque courant n'est pas dans ce groupe)");
    let r = p.ou(&mut b.app, "Revenir à la scène");
    p.cliquer(&mut b.app, r.center());
    p.repos(&mut b.app);
    p.cliquer(&mut b.app, ici);
    assert_eq!(b.app.cur_layer, 1, "le même trait, cliqué de nouveau : le calque suivant du groupe");
    let r = p.ou(&mut b.app, "Revenir à la scène");
    p.cliquer(&mut b.app, r.center());
    b.app.cur_layer = 0;
    // Un calque verrouillé ne se tire pas.
    b.app.doc.panels[0].layers[0].locked = true;
    let textes = p.repos(&mut b.app);
    let (_, r) = textes.iter().find(|(s, _)| s.contains("Carte 0")).unwrap_or_else(|| panic!("{textes:?}")).clone();
    let depart = pos2(r.left() - 20.0, r.center().y);
    p.glisser(&mut b.app, &droit(depart, depart - vec2(0.0, 40.0), 8));
    assert!((profondeur(&b, 0) - d0).abs() < 1e-5, "verrouillé : il ne bouge pas");
    b.chauffer(3);
}

#[test]
fn the_four_add_thumbnails_lay_a_surface_a_ground_a_wall_and_a_sky() {
    let mut b = ouvert(1);
    let mut p = Panneau::new();
    for (nom, sorte) in [("Surface", None), ("Sol", None), ("Mur", None), ("Ciel", Some(Sorte3d::Ciel))] {
        let n = b.app.layers().len();
        let r = p.ou(&mut b.app, nom);
        p.cliquer(&mut b.app, r.center());
        assert_eq!(b.app.layers().len(), n + 1, "« {nom} » ajoute un calque ({})", b.app.status);
        let neuf = &b.app.layers()[b.app.cur_layer];
        assert_eq!(neuf.calque_3d.map(|c| c.sorte), sorte, "« {nom} »");
        // (Le calque neuf est choisi : on revient à la scène pour le suivant.)
        b.app.espace3d.sel.clear();
    }
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------ un calque choisi

#[test]
fn the_pictograms_lay_the_card_facing_the_camera_on_the_ground_or_as_a_wall() {
    let mut b = ouvert(2);
    b.app.espace3d.sel = vec![espace3d::card_selection(b.app.layers()[1].id)];
    let mut p = Panneau::new();
    p.repos(&mut b.app);
    assert_eq!(b.app.e3_pose_du_calque(1), Some(0), "d'origine, face à la caméra : « Face » est allumé");
    for (nom, k) in [("Au sol", 1), ("Mur à gauche", 2), ("Mur à droite", 3), ("Face à la caméra", 0)] {
        let n = etapes(&b).len();
        let r = p.ou(&mut b.app, nom);
        p.cliquer(&mut b.app, r.center());
        assert_eq!(b.app.e3_pose_du_calque(1), Some(k), "« {nom} » ({})", b.app.status);
        assert_eq!(etapes(&b).len(), n + 1, "« {nom} » : une étape");
        assert_eq!(b.app.e3_pose_du_calque(0), Some(0), "l'autre calque n'a pas bougé");
    }
    // Au sol, le calque est couché : sa normale est la verticale. Il garde sa place.
    let r = p.ou(&mut b.app, "Au sol");
    let avant = b.app.e3_world().carte(&b.app.layers()[1], 0).pos;
    p.cliquer(&mut b.app, r.center());
    let xf = b.app.e3_world().carte(&b.app.layers()[1], 0);
    let n = crate::espace3dmath::axes_of(&crate::espace3dmath::rotation(xf.rot))[2];
    assert!(n[1].abs() > 0.999, "couché : {n:?}");
    for k in 0..3 {
        assert!((xf.pos[k] - avant[k]).abs() < 1e-3, "à sa place : {:?} / {avant:?}", xf.pos);
    }
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.e3_pose_du_calque(1), Some(0));
    b.chauffer(3);
}

#[test]
fn the_distance_ruler_moves_the_card_away_and_the_size_slider_scales_it() {
    let mut b = ouvert(2);
    b.app.espace3d.sel = vec![espace3d::card_selection(b.app.layers()[1].id)];
    let mut p = Panneau::new();
    let r = p.ou(&mut b.app, "Distance à la caméra");
    // La carte est à la feuille, la caméra d'origine à d0 : le trait est au milieu de la réglette (qui va jusqu'à 2 · d0).
    let (x0, x1) = (r.left() + 18.0, r.right() - 12.0);
    let trait_ = pos2((x0 + x1) / 2.0, r.center().y);
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &droit(trait_, trait_ + vec2(40.0, 0.0), 8));
    assert!(profondeur(&b, 1) > 0.05, "plus loin : {}", profondeur(&b, 1));
    assert_eq!(profondeur(&b, 0), 0.0);
    assert_eq!(etapes(&b).len(), n + 1);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Distance du calque"));
    // Le curseur de la taille : vers le grand carré, le calque grandit.
    let r = p.ou(&mut b.app, "Taille dans le cadre");
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &droit(r.center(), r.center() + vec2(40.0, 0.0), 8));
    assert!(b.app.layers()[1].pose(0).scale > 1.2, "plus grand : {}", b.app.layers()[1].pose(0).scale);
    assert_eq!(b.app.layers()[0].pose(0).scale, 1.0);
    assert_eq!(etapes(&b).len(), n + 1);
    // « Précis » déplie les champs d'avant.
    let r = p.ou(&mut b.app, "Précis");
    p.cliquer(&mut b.app, r.center());
    assert!(b.app.espace3d.precis_carte);
    let textes = p.repos(&mut b.app);
    for mot in ["Position (m)", "Rotation (°)", "Échelle", "Étirement", "Profondeur"] {
        assert!(textes.iter().any(|(s, _)| s == mot), "« {mot} » : {textes:?}");
    }
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------ la caméra choisie

#[test]
fn the_lens_thumbnails_and_the_field_cone_set_the_focal_length() {
    let mut b = ouvert(1);
    b.app.espace3d.sel = vec![CAMERA_ID];
    let mut p = Panneau::new();
    let n = etapes(&b).len();
    let r = p.ou(&mut b.app, "Objectif 85 mm");
    p.cliquer(&mut b.app, r.center());
    assert!((camera(&b).focal - 85.0).abs() < 0.01, "85 mm : {}", camera(&b).focal);
    assert_eq!(etapes(&b).len(), n + 1);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Objectif de la caméra"));
    let textes = p.repos(&mut b.app);
    assert!(textes.iter().any(|(s, _)| s.starts_with("85 mm · champ ")), "{textes:?}");
    // Le cône : glisser à droite resserre (focale plus longue), en une étape.
    let r = p.ou(&mut b.app, "Champ de la caméra");
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &droit(r.center(), r.center() + vec2(40.0, 0.0), 8));
    assert!(camera(&b).focal > 95.0, "plus longue : {}", camera(&b).focal);
    assert_eq!(etapes(&b).len(), n + 1);
    b.app.run_action("edition.annuler");
    assert!((camera(&b).focal - 85.0).abs() < 0.01);
    b.chauffer(3);
}

/// Où la vue de côté montre la caméra (les mêmes mesures que `App::e3_hauteur_et_plongee`).
fn camera_de_cote(b: &Banc, r: Rect) -> Pos2 {
    let h = camera(b).pos[1] - b.app.e3_world().sol_y();
    let sol = r.bottom() - 9.0;
    let ppm = (sol - r.top() - 16.0) / (h + 0.6).max(3.0);
    pos2(r.left() + 28.0, sol - h * ppm)
}

#[test]
fn the_side_view_raises_the_camera_and_tilts_its_aim() {
    let mut b = ouvert(1);
    b.app.espace3d.sel = vec![CAMERA_ID];
    let mut p = Panneau::new();
    let r = p.ou(&mut b.app, "Hauteur et plongée");
    let y0 = camera(&b).pos[1];
    let textes = p.repos(&mut b.app);
    assert!(textes.iter().any(|(s, _)| s == "Hauteur 1,7 m · à plat"), "{textes:?}");
    // Glisser la caméra vers le haut : elle monte.
    let cam = camera_de_cote(&b, r);
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &droit(cam, cam - vec2(0.0, 30.0), 8));
    assert!(camera(&b).pos[1] > y0 + 0.5, "plus haut : {} / {y0}", camera(&b).pos[1]);
    assert_eq!(camera(&b).tilt, 0.0, "la bascule n'a pas changé");
    assert_eq!(etapes(&b).len(), n + 1);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Hauteur de la caméra"));
    // Glisser loin de la caméra, vers le haut : la visée monte (contre-plongée) ; la hauteur ne bouge plus.
    let cam = camera_de_cote(&b, r);
    let y1 = camera(&b).pos[1];
    let loin = pos2(r.left() + 200.0, cam.y);
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &droit(loin, loin - vec2(0.0, 40.0), 8));
    let t = camera(&b).tilt;
    assert!(t > 5.0, "contre-plongée : {t}");
    assert_eq!(crate::espace3dgraphes::plongee(t), "légère contre-plongée");
    assert!((camera(&b).pos[1] - y1).abs() < 1e-4);
    assert_eq!(etapes(&b).len(), n + 1, "{:?}", etapes(&b));
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Bascule de la caméra"));
    b.chauffer(3);
}

#[test]
fn the_dial_pans_and_the_horizon_rolls_the_camera() {
    let mut b = ouvert(1);
    b.app.espace3d.sel = vec![CAMERA_ID];
    let mut p = Panneau::new();
    // Le cadran : un quart de tour, du haut vers la droite.
    let r = p.ou(&mut b.app, "Panoramique");
    let c = pos2(r.center().x, r.top() + crate::espace3dgraphes::CENTRE_TOUR);
    let quart: Vec<Pos2> = (0..=9).map(|k| (k as f32 * 10.0_f32).to_radians()).map(|a| c + vec2(a.sin(), -a.cos()) * 25.0).collect();
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &quart);
    assert!((camera(&b).pan - 90.0).abs() < 0.5, "un quart de tour à droite : {}", camera(&b).pan);
    assert_eq!(etapes(&b).len(), n + 1);
    // L'horizon : penché de 45° dans le sens horaire, la caméra tourne de 45° dans l'autre sens.
    let r = p.ou(&mut b.app, "Horizon");
    let c = pos2(r.center().x, r.top() + crate::espace3dgraphes::CENTRE_TOUR);
    let huitieme: Vec<Pos2> = (0..=9).map(|k| (k as f32 * 5.0_f32).to_radians()).map(|a| c + vec2(a.cos(), a.sin()) * 30.0).collect();
    let n = etapes(&b).len();
    p.glisser(&mut b.app, &huitieme);
    assert!((camera(&b).roll + 45.0).abs() < 0.5, "rotation : {}", camera(&b).roll);
    assert!((camera(&b).pan - 90.0).abs() < 0.5, "le panoramique n'a pas changé");
    assert_eq!(etapes(&b).len(), n + 1);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Rotation de la caméra"));
    // « Viser la sélection », la caméra choisie seule : elle se tourne vers le calque courant.
    let r = p.ou(&mut b.app, "Viser la sélection");
    p.cliquer(&mut b.app, r.center());
    assert!(camera(&b).pan.abs() < 1.0, "elle regarde de nouveau la feuille : {}", camera(&b).pan);
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------ au repos, hors de la vue, rien de perdu

#[test]
fn showing_the_graphics_in_each_state_changes_nothing() {
    let mut b = ouvert(2);
    // (Une carte derrière la caméra d'origine et une caméra retournée : les graphiques ne les ramènent pas.)
    b.app.doc.panels[0].layers[1].depth = -2.5;
    let cam = crate::camera::Camera { pan: 150.0, tilt: -60.0, ..b.app.current_camera() };
    b.app.set_current_camera_libre(cam);
    let avant = etapes(&b);
    let camera_avant = b.app.current_camera();
    let mut p = Panneau::new();
    for sel in [vec![], vec![espace3d::card_selection(b.app.layers()[1].id)], vec![CAMERA_ID]] {
        b.app.espace3d.sel = sel;
        b.app.espace3d.precis_carte = true;
        b.app.espace3d.precis_camera = true;
        p.repos(&mut b.app);
    }
    assert_eq!(etapes(&b), avant, "aucune étape");
    assert_eq!(b.app.doc.panels[0].layers[1].depth, -2.5);
    assert_eq!(b.app.current_camera(), camera_avant);
}

#[test]
fn nothing_of_the_new_panel_or_of_its_menus_shows_outside_the_3d_view() {
    let mut b = ouvert(2);
    b.chauffer(2);
    for nom in ["Le plateau, vu de dessus", "Affichage", "Aimant", "Vues"] {
        assert!(!vus::ou(nom).is_empty(), "vue ouverte : « {nom} » est à l'écran ({:?})", vus::noms());
    }
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.espace3d.on);
    for nom in ["Le plateau, vu de dessus", "Ajouter à la scène", "Affichage", "Aimant", "Objectif 85 mm", "Hauteur et plongée", "Distance à la caméra"] {
        assert!(vus::ou(nom).is_empty(), "à plat : « {nom} » ne se montre pas ({:?})", vus::noms());
    }
}

#[test]
fn every_former_setting_of_the_3d_panel_is_still_reachable() {
    let mut b = ouvert(2);
    let mut p = Panneau::new();
    // La scène.
    p.repos(&mut b.app);
    let mut vus_ici: Vec<String> = vus::noms();
    // Un calque Sol choisi, « Précis » déplié (sa grille de repère y est).
    b.app.nouveau_calque_3d(Sorte3d::Sol);
    b.app.espace3d.precis_carte = true;
    p.repos(&mut b.app);
    vus_ici.extend(vus::noms());
    // La caméra choisie, « Précis » déplié.
    b.app.espace3d.sel = vec![CAMERA_ID];
    b.app.espace3d.precis_camera = true;
    p.repos(&mut b.app);
    vus_ici.extend(vus::noms());
    // Les deux petits panneaux de la barre de la vue.
    for _ in 0..3 {
        p.image_de(&mut b.app, Vec::new(), &|app, ui| app.e3_affichage_ui(ui));
    }
    vus_ici.extend(vus::noms());
    for _ in 0..3 {
        p.image_de(&mut b.app, Vec::new(), &|app, ui| app.e3_aimant_ui(ui));
    }
    vus_ici.extend(vus::noms());
    // La barre de la vue elle-même.
    b.chauffer(2);
    vus_ici.extend(vus::noms());
    let d_avant = [
        // La ligne du haut d'avant : repère des poignées, aimantation, grille, vues, devant cache derrière.
        "Manipulateur dans le repère de la carte",
        "Aimanter à la grille",
        "Pas de la grille",
        "Pas de rotation",
        "Pas de taille",
        "Grille du sol",
        "Vues",
        "Le plus proche passe devant",
        // L'environnement.
        "Couleur du fond",
        "Couleur du maillage",
        "Opacité du maillage",
        "Pas automatique",
        "Taille des cases",
        // Les cartes : montrer, ordonner, nouvelle surface (face, sol, mur) et sa distance.
        "Montrer les cartes",
        "Ordonner la pile selon la profondeur",
        "Surface",
        "Sol",
        "Mur",
        "Distance de la surface",
        // La carte choisie : ses clés, ses champs, la remettre à plat, sa grille.
        "Clé de position",
        "Supprimer la clé d'ici",
        "Profondeur (multiplan)",
        "Remettre à plat",
        "Grille",
        // La caméra : ses clés, ses champs, regarder par elle, viser.
        "Clé de caméra ici",
        "Supprimer la clé de caméra",
        "Panoramique (degrés)",
        "Bascule",
        "Rotation",
        "Focale de la caméra",
        "Regarder par la caméra",
        "Viser la sélection",
        "Recadrer",
    ];
    for nom in d_avant {
        assert!(vus_ici.iter().any(|n| n == nom), "« {nom} » n'est plus joignable : {vus_ici:?}");
    }
}
