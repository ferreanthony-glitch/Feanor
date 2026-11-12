//! Essais du dessin dans la vue Espace 3D (`espace3ddessin.rs`, `docs/INTERFACE.md` partie 146) : de vrais traits de
//! souris dans la vue d'orbite et par la caméra, sur une carte de face, penchée et au sol ; les refus ; l'outil qui
//! décide ; la taille du pinceau ; la texture de la carte qui suit le trait ; le repos.

use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::espace3ddessin::{self as d3, CarteVue, Manque, Regard3d};
use crate::espace3dgizmo::Mode as GizmoMode;
use crate::perf_tests::{banc_taille, se_reposer, Banc};

/// Un projet d'essai : cadre 320 × 180 sur une feuille 640 × 360, `n` calques de 12 images, « Carte 0 » en bas.
fn doc_cartes(n: usize, kind: LayerKind) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for k in 0..n {
        let mut layer = Layer::new(format!("Carte {k}"), kind);
        let cel = doc.new_cel(kind);
        layer.frames = vec![Some(cel); 12];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Le banc, la vue Espace 3D ouverte, le calque 0 courant.
fn ouvert(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.espace3d.on && !b.app.camera_view, "la vue Espace 3D est ouverte");
    b
}

/// Prend le pinceau dans la vue : le pointeur dessine.
fn dessiner(b: &mut Banc) {
    b.app.run_action("outil.pinceau");
    b.chauffer(2);
    assert!(b.app.e3_dessine(), "le pinceau pris dans la vue fait dessiner le pointeur");
}

/// La carte du calque courant, vue par la vue où l'on dessine.
fn carte(b: &Banc) -> CarteVue {
    b.app.e3_carte_vue().expect("on dessine dans la vue, sur une carte")
}

/// Le dessin du calque `li` à l'image courante a-t-il de l'encre à moins de `r` pixels du point `p` ?
fn encre_pres(b: &Banc, li: usize, p: Pos2, r: i64) -> bool {
    let Some(cel) = b.app.layers()[li].shown_at(b.app.cur_frame) else { return false };
    let raster = &b.app.doc.cels[&cel].raster;
    let (cx, cy) = (p.x.round() as i64, p.y.round() as i64);
    (-r..=r).any(|dy| (-r..=r).any(|dx| raster.get_i(cx + dx, cy + dy).a() > 0))
}

/// Le nombre de pixels encrés du dessin du calque `li` à l'image courante (feuille seulement).
fn encre(b: &Banc, li: usize) -> usize {
    let Some(cel) = b.app.layers()[li].shown_at(b.app.cur_frame) else { return 0 };
    let raster = &b.app.doc.cels[&cel].raster;
    let (w, h) = (b.app.doc.width as i64, b.app.doc.height as i64);
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| raster.get_i(x, y).a() > 0).count()
}

/// Un geste de souris qui passe par ces points de l'écran, touches `mods` tenues : appui au premier, relâché au dernier.
fn geste(b: &mut Banc, pts: &[Pos2], mods: Modifiers) {
    b.touches = mods;
    let bouton = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: mods };
    b.image(vec![Event::PointerMoved(pts[0])], 1.0 / 60.0);
    b.image(vec![bouton(pts[0], true)], 1.0 / 60.0);
    for w in pts.windows(2) {
        for k in 0..6 {
            let ev = (0..3).map(|j| Event::PointerMoved(w[0] + (w[1] - w[0]) * ((k * 3 + j + 1) as f32 / 18.0))).collect();
            b.image(ev, 1.0 / 60.0);
        }
    }
    b.image(vec![bouton(*pts.last().expect("un point au moins"), false)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
}

/// Un trait de souris du point `a` au point `c` du dessin de la carte du calque courant (points de l'écran calculés par
/// le regard de la vue).
fn trait_sur_la_carte(b: &mut Banc, a: Pos2, c: Pos2) {
    let vue = carte(b);
    let (ea, ec) = (vue.vers_ecran(a).expect("le départ se voit"), vue.vers_ecran(c).expect("l'arrivée se voit"));
    let zone = b.app.canvas_rect.shrink(4.0);
    assert!(zone.contains(ea) && zone.contains(ec), "le trait tient dans la vue : {ea:?} → {ec:?} dans {zone:?} (par la caméra : {}, regard {:?})", b.app.espace3d.through_camera, vue.regard);
    geste(b, &[ea, ec], Modifiers::NONE);
}

/// L'écran rend chaque point de la carte au pixel près du dessin, et le trait de `a` à `c` laisse de l'encre le long de
/// son chemin et nulle part ailleurs ; une annulation l'enlève.
fn verifier_un_trait(b: &mut Banc, quoi: &str, a: Pos2, c: Pos2) {
    let li = b.app.cur_layer;
    let vue = carte(b);
    for p in [a, c, a + (c - a) * 0.5, pos2(320.0, 180.0)] {
        let ecran = vue.vers_ecran(p).unwrap_or_else(|| panic!("{quoi} : le point {p:?} de la carte se voit"));
        let retour = vue.vers_dessin(ecran).unwrap_or_else(|m| panic!("{quoi} : le regard touche la carte en {p:?} ({m:?})"));
        assert!((retour - p).length() < 0.05, "{quoi} : écran → carte rend {retour:?} au lieu de {p:?}");
    }
    let (avant, etapes) = (encre(b, li), b.app.history.undo_len());
    trait_sur_la_carte(b, a, c);
    assert!(b.app.stroke.is_none(), "{quoi} : le trait est fini au relâché");
    for t in [0.2, 0.5, 0.8] {
        let p = a + (c - a) * t;
        assert!(encre_pres(b, li, p, 6), "{quoi} : de l'encre en {p:?}, sur le chemin du trait (barre d'état : {})", b.app.status);
    }
    // Rien loin du chemin : à 90 pixels de part et d'autre de son milieu.
    let n = (c - a).normalized().rot90() * 90.0;
    for p in [a + (c - a) * 0.5 + n, a + (c - a) * 0.5 - n] {
        assert!(!encre_pres(b, li, p, 6), "{quoi} : pas d'encre en {p:?}, loin du trait");
    }
    assert!(encre(b, li) > avant, "{quoi} : le dessin a gagné de l'encre");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "{quoi} : le trait est une étape d'annulation");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(encre(b, li), avant, "{quoi} : l'annulation enlève le trait");
}

#[test]
fn a_mouse_stroke_in_the_orbit_view_inks_the_card_from_the_front_tilted_and_flat_on_the_ground() {
    // Plan ordinaire, carte de face.
    let mut b = ouvert(doc_cartes(1, LayerKind::Bitmap));
    dessiner(&mut b);
    verifier_un_trait(&mut b, "carte de face", pos2(200.0, 130.0), pos2(440.0, 230.0));

    // Plan ordinaire, carte penchée et décalée.
    let mut doc = doc_cartes(1, LayerKind::Bitmap);
    doc.panels[0].layers[0].transform = LayerTransform { x: 30.0, y: 10.0, tilt_y: 35.0, tilt_x: -15.0, ..LayerTransform::default() };
    let mut b = ouvert(doc);
    dessiner(&mut b);
    verifier_un_trait(&mut b, "carte penchée", pos2(220.0, 120.0), pos2(420.0, 240.0));

    // Plan en Espace 3D, surface au sol (couchée à 90°) : elle est le calque courant, on dessine dessus vue de dessus.
    let mut b = ouvert(doc_cartes(1, LayerKind::Bitmap));
    b.app.run_action("espace3d.plan_en_espace");
    b.app.run_action("espace3d.surface_sol");
    assert_eq!(b.app.layer().name, "Surface au sol", "{}", b.app.status);
    assert!((b.app.layer().pose(0).tilt_x.abs() - 90.0).abs() < 0.5, "la surface est couchée : {:?}", b.app.layer().pose(0));
    b.app.run_action("espace3d.cadrer");
    b.chauffer(2);
    dessiner(&mut b);
    verifier_un_trait(&mut b, "surface au sol", pos2(220.0, 140.0), pos2(420.0, 220.0));
}

#[test]
fn a_mouse_stroke_through_the_camera_inks_the_card_in_an_ordinary_plan_and_in_a_plan_in_3d_space() {
    // Plan ordinaire, par la caméra du plan.
    let mut b = ouvert(doc_cartes(1, LayerKind::Bitmap));
    b.app.run_action("espace3d.vue_camera");
    b.chauffer(2);
    assert!(b.app.espace3d.through_camera);
    dessiner(&mut b);
    verifier_un_trait(&mut b, "par la caméra, plan ordinaire", pos2(260.0, 150.0), pos2(380.0, 210.0));

    // Plan en Espace 3D, caméra tournée, surface posée face à elle (donc penchée dans le monde) : par la caméra.
    let mut b = ouvert(doc_cartes(1, LayerKind::Bitmap));
    b.app.run_action("espace3d.plan_en_espace");
    let tournee = crate::camera::Camera { pan: 12.0, tilt: -6.0, ..b.app.current_camera() };
    b.app.set_current_camera(tournee);
    b.app.run_action("espace3d.surface_face");
    assert_eq!(b.app.layer().name, "Surface face à la caméra", "{}", b.app.status);
    b.app.run_action("espace3d.vue_camera");
    b.chauffer(2);
    dessiner(&mut b);
    // Le cadre tombe au centre de la surface : on trace autour du point de la carte que vise le centre de la vue.
    let vue = carte(&b);
    let milieu = vue.vers_dessin(b.app.canvas_rect.center()).expect("le regard du centre de la vue touche la surface");
    assert!(vue.dans_la_carte(milieu), "le centre de la vue est sur la surface : {milieu:?}");
    verifier_un_trait(&mut b, "par la caméra, plan en Espace 3D", milieu + vec2(-50.0, -25.0), milieu + vec2(50.0, 25.0));
}

#[test]
fn nothing_is_painted_off_the_card_from_behind_edge_on_on_hidden_cards_or_on_a_locked_layer() {
    let mut b = ouvert(doc_cartes(2, LayerKind::Bitmap));
    dessiner(&mut b);
    let etapes = b.app.history.undo_len();
    let sans_encre = |b: &Banc, quoi: &str| {
        assert_eq!(encre(b, 0) + encre(b, 1), 0, "{quoi} : aucune encre");
        assert!(b.app.stroke.is_none(), "{quoi} : aucun trait en cours");
    };

    // Hors des bords de la carte : le trait ne commence pas, la barre d'état dit pourquoi.
    let vue = carte(&b);
    let dehors = vue.vers_ecran(pos2(-60.0, 180.0)).expect("un point à gauche de la carte se voit");
    assert!(b.app.canvas_rect.contains(dehors));
    assert_eq!(vue.vers_dessin(dehors).map(|p| vue.dans_la_carte(p)), Ok(false));
    let dedans = vue.vers_ecran(pos2(200.0, 180.0)).expect("dans la carte");
    geste(&mut b, &[dehors, dedans], Modifiers::NONE);
    sans_encre(&b, "appui hors de la carte");
    assert_eq!(b.app.status, d3::HORS_CARTE);

    // Un trait commencé sur la carte et qui en sort : les points d'au-delà d'une feuille sont laissés, le reste est tracé.
    geste(&mut b, &[dedans, dehors], Modifiers::NONE);
    assert!(encre_pres(&b, 0, pos2(150.0, 180.0), 6), "le trait commencé sur la carte est tracé");
    b.app.run_action("edition.annuler");
    b.chauffer(2);

    // Vue de dos : la vue a fait un demi-tour.
    let devant = b.app.espace3d.cam.expect("la vue");
    b.app.espace3d.cam.as_mut().expect("la vue").azimuth += 180.0;
    b.chauffer(2);
    let centre = b.app.canvas_rect.center();
    assert_eq!(carte(&b).vers_dessin(centre), Err(Manque::Dos));
    geste(&mut b, &[centre, centre + vec2(60.0, 20.0)], Modifiers::NONE);
    sans_encre(&b, "carte vue de dos");
    assert_eq!(b.app.status, d3::CARTE_DE_DOS);

    // Vue par la tranche.
    let c = b.app.espace3d.cam.as_mut().expect("la vue");
    (c.azimuth, c.elevation) = (90.0, 0.0);
    b.chauffer(2);
    assert_eq!(carte(&b).vers_dessin(centre), Err(Manque::Tranche));
    geste(&mut b, &[centre, centre + vec2(60.0, 20.0)], Modifiers::NONE);
    sans_encre(&b, "carte vue par la tranche");
    assert_eq!(b.app.status, d3::CARTE_PAR_LA_TRANCHE);
    b.app.espace3d.cam = Some(devant);
    b.chauffer(2);

    // Les cartes cachées dans la vue.
    b.app.espace3d.show_cards = false;
    geste(&mut b, &[dedans, dedans + vec2(40.0, 10.0)], Modifiers::NONE);
    sans_encre(&b, "cartes cachées");
    assert_eq!(b.app.status, d3::CARTES_CACHEES);
    b.app.espace3d.show_cards = true;

    // Un calque verrouillé : le refus habituel.
    b.app.layers_mut()[0].locked = true;
    geste(&mut b, &[dedans, dedans + vec2(40.0, 10.0)], Modifiers::NONE);
    sans_encre(&b, "calque verrouillé");
    assert!(b.app.status.contains("verrouillé"), "{}", b.app.status);
    b.app.layers_mut()[0].locked = false;

    // Un calque caché : le refus habituel.
    b.app.layers_mut()[0].visible = false;
    geste(&mut b, &[dedans, dedans + vec2(40.0, 10.0)], Modifiers::NONE);
    sans_encre(&b, "calque caché");
    assert!(b.app.status.contains("masqué"), "{}", b.app.status);
    b.app.layers_mut()[0].visible = true;

    assert_eq!(b.app.history.undo_len(), etapes, "aucun de ces refus ne laisse d'étape d'annulation");
}

#[test]
fn the_tool_decides_a_view_tool_moves_the_card_a_drawing_tool_draws_on_it() {
    let mut b = ouvert(doc_cartes(2, LayerKind::Bitmap));
    // À l'entrée dans la vue, le pointeur manipule (le pinceau est pourtant l'outil courant) : rien ne change pour qui
    // ne dessine pas.
    assert_eq!(b.app.tool, Tool::Brush);
    assert!(!b.app.e3_dessine());

    // Le pinceau, pris dans la vue : on dessine, la carte du calque courant est choisie.
    dessiner(&mut b);
    assert_eq!(b.app.espace3d.sel, vec![crate::espace3d::card_selection(b.app.layers()[0].id)]);
    assert!(b.app.e3_dessin_legende().contains("Pinceau sur la carte « Carte 0 »"), "{}", b.app.e3_dessin_legende());
    let vue = carte(&b);
    let (a, c) = (vue.vers_ecran(pos2(250.0, 160.0)).expect("a"), vue.vers_ecran(pos2(390.0, 200.0)).expect("c"));

    // Un outil de la vue : le même geste déplace la carte et ne laisse aucune encre.
    b.app.run_action("espace3d.outil_deplacer");
    assert!(!b.app.e3_dessine() && b.app.espace3d.mode == GizmoMode::Move);
    let pose = b.app.layers()[0].pose(0);
    geste(&mut b, &[a, c], Modifiers::NONE);
    assert_eq!(encre(&b, 0), 0, "un outil de la vue ne trace pas");
    assert_ne!(b.app.layers()[0].pose(0), pose, "il déplace la carte");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.layers()[0].pose(0), pose);

    // Le crayon, par sa commande (sa touche) : on dessine de nouveau, et le même geste trace.
    b.app.run_action("outil.crayon");
    b.chauffer(2);
    assert!(b.app.e3_dessine() && b.app.tool == Tool::Pencil);
    geste(&mut b, &[a, c], Modifiers::NONE);
    assert!(encre(&b, 0) > 0, "le crayon trace sur la carte : {}", b.app.status);
    assert_eq!(b.app.layers()[0].pose(0), pose, "il ne déplace pas la carte");

    // La gomme : toujours le dessin ; elle enlève de l'encre.
    let pleine = encre(&b, 0);
    b.app.run_action("outil.gomme");
    b.chauffer(2);
    assert!(b.app.e3_dessine() && b.app.tool == Tool::Eraser);
    geste(&mut b, &[a, c], Modifiers::NONE);
    assert!(encre(&b, 0) < pleine, "la gomme enlève de l'encre : {} → {}", pleine, encre(&b, 0));

    // Un outil qui ne trace pas (la sélection) : le pointeur manipule, la barre d'état le dit.
    b.app.run_action("outil.selection");
    assert!(!b.app.e3_dessine());
    assert_eq!(b.app.status, d3::MANIPULE);

    // Le bouton « Dessiner sur la carte » : il reprend le dernier outil de dessin, puis rend la main aux cartes.
    b.app.run_action(d3::ACTION);
    assert!(b.app.e3_dessine() && b.app.tool == Tool::Eraser, "le dernier outil de dessin revient : {:?}", b.app.tool);
    b.app.run_action(d3::ACTION);
    assert!(!b.app.e3_dessine());
    assert_eq!(b.app.tool, Tool::Eraser, "l'outil reste celui-là ; c'est le pointeur qui manipule");

    // Sortir de la vue et y revenir garde le choix ; hors de la vue, rien ne change pour le dessin à plat.
    b.app.run_action(d3::ACTION);
    b.app.run_action("espace3d.basculer");
    b.chauffer(2);
    assert!(!b.app.espace3d.on && !b.app.e3_dessine());
    assert!(b.app.e3_carte_vue().is_none() && b.app.e3_zoom_du_dessin().is_none());
    b.app.run_action("espace3d.basculer");
    b.chauffer(2);
    assert!(b.app.e3_dessine(), "revenu dans la vue avec un outil de dessin : on dessine encore");
}

#[test]
fn alt_drag_turns_the_view_and_ctrl_click_takes_another_card_without_a_stroke() {
    let mut doc = doc_cartes(2, LayerKind::Bitmap);
    // La carte 1 est plus loin et décalée à droite : sa moitié droite dépasse de la carte 0.
    doc.panels[0].layers[1].transform = LayerTransform { x: 380.0, ..LayerTransform::default() };
    doc.panels[0].layers[1].depth = 0.5;
    let mut b = ouvert(doc);
    b.app.run_action("espace3d.cadrer");
    b.chauffer(2);
    dessiner(&mut b);
    let vue = carte(&b);
    let (a, c) = (vue.vers_ecran(pos2(250.0, 160.0)).expect("a"), vue.vers_ecran(pos2(390.0, 200.0)).expect("c"));

    // Alt+glisser : la vue tourne, rien n'est tracé.
    let avant = b.app.espace3d.cam.expect("la vue");
    geste(&mut b, &[a, c], Modifiers::ALT);
    let apres = b.app.espace3d.cam.expect("la vue");
    assert!((apres.azimuth - avant.azimuth).abs() > 1.0, "Alt+glisser tourne la vue : {avant:?} → {apres:?}");
    assert_eq!(encre(&b, 0) + encre(&b, 1), 0, "Alt+glisser ne trace pas");
    assert!(b.app.espace3d.drag.is_none() && b.app.e3_dessine());
    b.app.espace3d.cam = Some(avant);
    b.chauffer(2);

    // Ctrl+clic sur la carte 1 : elle devient le calque courant, on dessine toujours, rien n'est tracé.
    let regard: Regard3d = b.app.e3d.regard.expect("le regard de la vue");
    let world = b.app.e3_world();
    let l = &b.app.layers()[1];
    let autre = CarteVue::new(regard, world.card_xf(&l.pose(0), l.depth_at(0)).matrix(), world.stage);
    let ici = autre.vers_ecran(pos2(600.0, 180.0)).expect("la droite de la carte 1 se voit");
    assert!(b.app.canvas_rect.contains(ici));
    geste(&mut b, &[ici, ici], Modifiers::CTRL);
    assert_eq!(b.app.cur_layer, 1, "Ctrl+clic prend la carte sous le pointeur : {}", b.app.status);
    assert!(b.app.e3_dessine());
    assert_eq!(encre(&b, 0) + encre(&b, 1), 0, "Ctrl+clic ne trace pas");
    assert!(b.app.e3_dessin_legende().contains("« Carte 1 »"));

    // Le trait qui suit va sur elle.
    let vue = carte(&b);
    let (a, c) = (vue.vers_ecran(pos2(520.0, 150.0)).expect("a"), vue.vers_ecran(pos2(610.0, 210.0)).expect("c"));
    geste(&mut b, &[a, c], Modifiers::NONE);
    assert!(encre(&b, 1) > 0 && encre(&b, 0) == 0, "le trait va sur la carte prise : {}", b.app.status);
}

#[test]
fn the_brush_circle_has_the_width_of_the_stroke_on_the_card_seen_by_the_view() {
    let mut b = ouvert(doc_cartes(1, LayerKind::Bitmap));
    dessiner(&mut b);
    let vue = carte(&b);
    let p = pos2(320.0, 180.0);
    let ecran = vue.vers_ecran(p).expect("le centre de la carte se voit");
    b.image(vec![Event::PointerMoved(ecran)], 1.0 / 60.0);
    b.chauffer(1);
    // Ce que dix pixels du dessin mesurent à l'écran, le long des deux axes de la carte.
    let vue = carte(&b);
    let (o, dx, dy) = (vue.vers_ecran(p).expect("o"), vue.vers_ecran(p + vec2(10.0, 0.0)).expect("x"), vue.vers_ecran(p + vec2(0.0, 10.0)).expect("y"));
    let attendu = (((dx - o).x * (dy - o).y - (dx - o).y * (dy - o).x).abs() / 100.0).sqrt();
    let zoom = b.app.layer_zoom();
    assert!((zoom / attendu - 1.0).abs() < 0.02, "pixels d'écran par pixel de la carte : {zoom} au lieu de {attendu}");
    // La vue recule de moitié : la carte est deux fois plus petite à l'écran, le pinceau aussi.
    b.app.espace3d.cam.as_mut().expect("la vue").dolly(2.0);
    b.chauffer(1);
    let ecran = carte(&b).vers_ecran(p).expect("le centre de la carte se voit");
    b.image(vec![Event::PointerMoved(ecran)], 1.0 / 60.0);
    b.chauffer(1);
    let loin = b.app.layer_zoom();
    assert!(loin < zoom * 0.75 && loin > zoom * 0.25, "vue reculée : {zoom} → {loin}");
    // Hors de la vue Espace 3D, la taille du pinceau redevient celle du dessin à plat.
    b.app.run_action("espace3d.basculer");
    b.chauffer(2);
    assert!((b.app.layer_zoom() - b.app.view.zoom).abs() < 1e-4);
}

#[test]
fn the_card_texture_follows_the_stroke_piece_by_piece_and_the_application_rests_afterwards() {
    // Une feuille de production (3 840 × 2 160) : la texture de la carte ne se refait qu'une fois, puis par morceaux.
    let mut doc = Document::with_frame(1920, 1080, 24);
    doc.panels[0].layers = Vec::new();
    let mut layer = Layer::new("Carte", LayerKind::Bitmap);
    let cel = doc.new_cel(LayerKind::Bitmap);
    layer.frames = vec![Some(cel); 12];
    doc.panels[0].layers.push(layer);
    let mut b = ouvert(doc);
    dessiner(&mut b);
    assert_eq!((b.app.e3d.constructions, b.app.e3d.morceaux), (1, 0), "la texture de la carte est faite une fois, à l'entrée");
    let vue = carte(&b);
    let (pa, pc) = (pos2(1500.0, 900.0), pos2(2300.0, 1250.0));
    let (a, c) = (vue.vers_ecran(pa).expect("a"), vue.vers_ecran(pc).expect("c"));
    // Le trait, image par image : pendant le geste, la texture suit par morceaux.
    let bouton = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true)], 1.0 / 60.0);
    let mut temps = Vec::new();
    for k in 0..12 {
        let ev = (0..3).map(|j| Event::PointerMoved(a + (c - a) * ((k * 3 + j + 1) as f32 / 36.0))).collect();
        let t = std::time::Instant::now();
        b.image(ev, 1.0 / 60.0);
        temps.push(t.elapsed().as_secs_f64() * 1e3);
        assert!(b.app.stroke.is_some(), "le trait est en cours");
    }
    assert_eq!(b.app.e3d.constructions, 1, "pendant le trait, la texture n'est jamais refaite en entier");
    assert!(b.app.e3d.morceaux >= 6, "elle suit le trait par morceaux : {} morceaux", b.app.e3d.morceaux);
    // Le dernier morceau refait tient dans la texture et couvre moins du quart de son aire.
    let m = b.app.e3d.dernier_morceau.expect("un morceau refait");
    assert!(m[2] <= 2048 && m[3] <= 1152 && (m[2] - m[0]) * (m[3] - m[1]) < 2048 * 1152 / 4, "morceau {m:?}");
    b.image(vec![bouton(c, false)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(encre_pres(&b, 0, pa + (pc - pa) * 0.5, 8), "le trait est sur la carte");
    assert_eq!(b.app.e3d.constructions, 1);
    temps.sort_by(|x, y| x.total_cmp(y));
    println!("PERF dessin dans la vue Espace 3D : une image pendant le trait (feuille 3840 × 2160, écran 1280 × 720, sans carte graphique) : médiane {:.2} ms, max {:.2} ms", temps[temps.len() / 2], temps[temps.len() - 1]);
    // Au repos après le trait : plus rien ne réveille l'écran.
    let n = se_reposer(&mut b, 400);
    assert!(n < 400, "l'application ne revient pas au repos après un trait dans la vue Espace 3D");
    let morceaux = b.app.e3d.morceaux;
    b.chauffer(5);
    assert_eq!(b.app.e3d.morceaux, morceaux, "au repos, la texture n'est pas retouchée");
}

#[test]
fn a_stroke_on_a_vector_card_keeps_its_element_and_shift_draws_a_straight_segment() {
    let mut b = ouvert(doc_cartes(1, LayerKind::Vector));
    dessiner(&mut b);
    let vue = carte(&b);
    let (pa, pc) = (pos2(220.0, 140.0), pos2(420.0, 220.0));
    let (a, c) = (vue.vers_ecran(pa).expect("a"), vue.vers_ecran(pc).expect("c"));
    geste(&mut b, &[a, c], Modifiers::NONE);
    let cel = b.app.layers()[0].shown_at(0).expect("dessin");
    let elements = |b: &Banc| b.app.doc.cels[&cel].vector.as_ref().map_or(0, |d| d.elements.len());
    assert_eq!(elements(&b), 1, "le trait est un élément du dessin vectoriel : {}", b.app.status);
    assert!(encre_pres(&b, 0, pa + (pc - pa) * 0.5, 6));

    // Maj tenue : un segment droit, du départ à l'arrivée, même si la main fait un détour.
    let detour = a + (c - a) * 0.5 + vec2(0.0, 70.0);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(elements(&b), 0);
    geste(&mut b, &[a, detour, c], Modifiers::SHIFT);
    assert_eq!(elements(&b), 1, "le segment droit est posé : {}", b.app.status);
    assert!(encre_pres(&b, 0, pa + (pc - pa) * 0.5, 6), "le segment passe par le milieu de la corde");
    let hors = vue.vers_dessin(detour).expect("le détour est sur la carte");
    assert!(!encre_pres(&b, 0, hors, 6), "le détour de la main n'est pas tracé");
}
