//! Essais du panneau « Espace 3D » (`espace3dpanneau.rs`, `docs/INTERFACE.md` parties 150 et 152) : ce que montrent le
//! panneau Réglages et le panneau Espace 3D quand la vue est ouverte (le pinceau, la Trajectoire dans l'espace) et
//! fermée ; sa place, une colonne à lui à droite de celle des réglages de l'outil (partie 152 : « Place-le à côté de
//! réglages plutôt »), qui arrive quand la vue s'ouvre et repart quand elle se ferme ; rangé ou ouvert à la main, il
//! reste où on l'a mis ; les dispositions retenues qui l'avaient en onglet avec la couleur (partie 150) le perdent une
//! fois ; sa commande, son icône, et les messages qui disaient « dans Réglages ».

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};

use crate::app::{App, Tool};
use crate::barrepanneaux::est_devant;
use crate::dock::{self, DockLayout, DockNode, FloatingPanel, Group, Pane};
use crate::document::{Document, Layer, LayerKind};
use crate::espace3dpanneau::{OUVRIR, SUR_LA_PLANCHE, VUE_FERMEE};
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::trajectoire::{Cles, Support, Trajectoire};
use crate::workspace::{self, Espace, Layout, Workspace};

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

/// Le banc dans l'espace Storyboard, sa disposition d'origine, la vue Espace 3D fermée, le calque 0 courant.
fn banc_d_essai(n: usize) -> Banc {
    let mut b = banc_taille(doc_cartes(n), false, 1280, 720);
    b.chauffer(2);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    assert!(b.app.espace == Espace::Storyboard && !b.app.espace3d_vue_ouverte());
    b
}

fn ouvrir_la_vue(b: &mut Banc) {
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(b.app.espace3d_vue_ouverte(), "la vue Espace 3D est ouverte ({})", b.app.status);
}

fn fermer_la_vue(b: &mut Banc) {
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.espace3d.on, "la vue Espace 3D est fermée ({})", b.app.status);
}

/// `pane` est l'onglet de devant de son groupe, à l'écran.
fn devant(b: &Banc, pane: Pane) -> bool {
    est_devant(b.app.dock.as_ref().expect("la disposition à l'écran"), pane)
}

/// Les onglets du groupe de `pane` à l'écran, dans l'ordre.
fn groupe(b: &Banc, pane: Pane) -> Vec<Pane> {
    onglets_de(&b.app.dock_layout(), pane).map(|(onglets, _)| onglets).unwrap_or_default()
}

// ------------------------------------------------------------------------------------------------ les dispositions

fn tabs(p: &[Pane]) -> DockNode {
    DockNode::Tabs { panneaux: p.to_vec(), actif: 0 }
}

fn dans(n: &DockNode, pane: Pane) -> Option<(Vec<Pane>, Pane)> {
    match n {
        DockNode::Tabs { panneaux, actif } => panneaux.contains(&pane).then(|| (panneaux.clone(), panneaux[(*actif).min(panneaux.len() - 1)])),
        DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => dans(a, pane).or_else(|| dans(b, pane)),
    }
}

/// Le groupe d'onglets de `pane` dans une disposition (fenêtre principale ou panneau flottant) : ses onglets, et celui
/// qui est devant.
fn onglets_de(l: &DockLayout, pane: Pane) -> Option<(Vec<Pane>, Pane)> {
    dans(&l.principal, pane).or_else(|| l.fenetres.iter().find_map(|w| dans(&w.panneaux, pane)))
}

/// Met `pane` devant les onglets de son groupe, dans la fenêtre principale d'une disposition.
fn mettre_devant(n: &mut DockNode, pane: Pane) -> bool {
    match n {
        DockNode::Tabs { panneaux, actif } => match panneaux.iter().position(|p| *p == pane) {
            Some(rang) => {
                *actif = rang;
                true
            }
            None => false,
        },
        DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => mettre_devant(a, pane) || mettre_devant(b, pane),
    }
}

/// La disposition standard, sans le panneau Espace 3D : la couleur et les brosses dédiées sous les réglages de l'outil.
/// C'est celle d'avant lui, et de nouveau celle d'origine (partie 152 : sa colonne n'arrive qu'avec la vue).
fn disposition_d_avant() -> DockLayout {
    let d = DockLayout::from_legacy(&Layout::default());
    assert!(!d.contains(Pane::Espace3d));
    assert_eq!(onglets_de(&d, Pane::Color), Some((vec![Pane::Color, Pane::Personnages], Pane::Color)));
    d
}

/// La disposition standard telle que la partie 150 la faisait : le panneau Espace 3D en onglet avec la couleur et les
/// brosses dédiées, sous les réglages de l'outil.
fn disposition_150() -> DockLayout {
    let mut d = disposition_d_avant();
    fn avec_la_couleur(n: &mut DockNode) -> bool {
        match n {
            DockNode::Tabs { panneaux, .. } if panneaux.contains(&Pane::Color) => {
                panneaux.push(Pane::Espace3d);
                true
            }
            DockNode::Tabs { .. } => false,
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => avec_la_couleur(a) || avec_la_couleur(b),
        }
    }
    assert!(avec_la_couleur(&mut d.principal));
    assert_eq!(onglets_de(&d, Pane::Espace3d), Some((vec![Pane::Color, Pane::Personnages, Pane::Espace3d], Pane::Color)));
    d
}

// ------------------------------------------------------------------------------------------------ ce qu'un panneau montre

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

/// Une image du panneau `pane` tel que le dock le dessine (`App::pane_ui`), dans un contexte à part, haut pour que tout
/// s'y voie, avec ces événements : les textes peints et leur place.
fn image_du_panneau(ctx: &egui::Context, app: &mut App, pane: Pane, t: &mut f64, events: Vec<Event>) -> Vec<(String, Rect)> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 1600.0))), time: Some(*t), events, ..RawInput::default() };
    *t += 1.0 / 60.0;
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.pane_ui(ui, pane));
    });
    let mut textes = Vec::new();
    for s in &out.shapes {
        collecter(&s.shape, &mut textes);
    }
    textes
}

/// Les textes que montre le panneau `pane`, au repos (quelques images, le temps que sa mise en page se pose).
fn textes_du_panneau(app: &mut App, pane: Pane) -> Vec<String> {
    let (ctx, mut t) = (egui::Context::default(), 0.0);
    let mut textes = Vec::new();
    for _ in 0..4 {
        textes = image_du_panneau(&ctx, app, pane, &mut t, Vec::new());
    }
    textes.into_iter().map(|(s, _)| s).collect()
}

fn contient(textes: &[String], mot: &str) -> bool {
    textes.iter().any(|s| s.contains(mot))
}

/// Ce que montrent les deux panneaux : chaque mot de `la` est dans `pane` et aucun mot de `pas_la` n'y est.
fn verifier(app: &mut App, pane: Pane, la: &[&str], pas_la: &[&str]) {
    let textes = textes_du_panneau(app, pane);
    for mot in la {
        assert!(contient(&textes, mot), "« {mot} » devrait être dans le panneau {} : {textes:?}", pane.name());
    }
    for mot in pas_la {
        assert!(!contient(&textes, mot), "« {mot} » ne devrait pas être dans le panneau {} : {textes:?}", pane.name());
    }
}

/// Ce qui ne se voit que dans les réglages de la vue Espace 3D.
const DE_LA_VUE: [&str; 5] = ["Cartes (calques)", "Carte 0", "Caméra du plan", "Position (m)", "Focale"];

// ------------------------------------------------------------------------------------------------ Réglages montre l'outil

#[test]
fn in_the_3d_view_the_settings_pane_shows_the_brush_and_the_space_pane_the_cards_and_the_camera() {
    let mut b = banc_d_essai(2);
    ouvrir_la_vue(&mut b);
    b.app.set_tool(Tool::Brush);
    b.chauffer(2);
    assert!(b.app.e3_dessine(), "le pinceau dessine sur la carte ({})", b.app.status);
    // Réglages : le pinceau (son nom, son opacité, « Plus de réglages »), et rien de la vue.
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Brush.label(), "Opacité", "Plus de réglages"], &DE_LA_VUE);
    // Le panneau Espace 3D : les cartes et la caméra, et rien du pinceau.
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &["Opacité", "Plus de réglages", VUE_FERMEE]);
    verifier(&mut b.app, Pane::Espace3d, &["Carte 1", "Espace 3D"], &[]);
    // Le crayon et la gomme aussi.
    for outil in [Tool::Pencil, Tool::Eraser] {
        b.app.set_tool(outil);
        b.chauffer(1);
        verifier(&mut b.app, Pane::ToolOptions, &[outil.label(), "Opacité"], &DE_LA_VUE);
    }
    // Un outil de la vue (déplacer) rend le pointeur aux cartes : Réglages montre toujours l'outil en main, et les outils
    // de la vue gardent leurs réglages dans le panneau Espace 3D.
    b.app.run_action("espace3d.outil_deplacer");
    b.chauffer(1);
    assert!(!b.app.e3_dessine() && b.app.tool == Tool::Eraser);
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Eraser.label()], &DE_LA_VUE);
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &[]);
    // Vue fermée : Réglages n'a pas changé de sujet.
    fermer_la_vue(&mut b);
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Eraser.label(), "Opacité"], &DE_LA_VUE);
}

#[test]
fn with_the_trajectory_tool_in_a_plan_in_space_the_settings_pane_shows_the_space_path_and_the_space_pane_does_not() {
    let mut b = banc_d_essai(2);
    ouvrir_la_vue(&mut b);
    b.app.run_action("espace3d.plan_en_espace");
    assert!(b.app.doc.plan_en_espace(0), "{}", b.app.status);
    b.app.traj.cles_propose = Cles::Interpolation;
    b.app.run_action("outil.trajectoire");
    b.chauffer(2);
    assert!(b.app.e3_trace(), "l'outil Trajectoire a le pointeur dans la vue ({})", b.app.status);
    // Sans chemin encore : qui est choisi, le mode d'emploi et les deux sortes de clés sont dans Réglages.
    let (images, interpolation) = (Cles::Images.label(), Cles::Interpolation.label());
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Trajectoire.label(), "Trajectoire dans l'espace", "Choisi : 1 carte.", "Trace d'un trait", images, interpolation], &DE_LA_VUE);
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &["Trajectoire dans l'espace", "Choisi :", "Trace d'un trait"]);
    // La carte du calque courant suit un chemin au sol, avec une marque recalée à la main : toutes les commandes du chemin
    // sont dans Réglages.
    let world = b.app.e3_world();
    let sol = Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k).expect("un plan");
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[0.0, 0.0], [4.0 / world.k, 2.0 / world.k]], sol).expect("un chemin");
    (t.cles, t.fin) = (Cles::Interpolation, Some(23));
    assert_eq!(t.ajouter_marque(0.3), Some(0));
    let jalons = t.images_des_jalons(24);
    b.app.doc.panels[0].layers[0].trajectoire = Some(t);
    b.app.trajectoire_recalculer(0);
    assert!(b.app.trajectoire_recaler(0, jalons[1], 4), "{}", b.app.status);
    b.app.cur_frame = 9;
    b.chauffer(1);
    let commandes = ["Choisi : 1 carte. 1 marque", "Partir de la tête de lecture", "Finir à la tête de lecture", "Temps égaux entre les marques", images, interpolation, "Retirer les marques", "Détacher", "Effacer", "Tirer sur le chemin"];
    verifier(&mut b.app, Pane::ToolOptions, &commandes, &DE_LA_VUE);
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &["Trajectoire dans l'espace", "Choisi :", "Partir de la tête de lecture", "Détacher", "Retirer les marques", "Temps égaux"]);
    // Sur la feuille, pour un calque sans chemin dans l'espace : les réglages de la trajectoire à plat, comme avant.
    fermer_la_vue(&mut b);
    b.app.cur_layer = 1;
    b.chauffer(1);
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Trajectoire.label(), "Trace le chemin du calque d'un trait"], &["Trajectoire dans l'espace", "Choisi :"]);
}

#[test]
fn in_an_ordinary_plan_the_trajectory_settings_point_to_the_space_pane_for_the_checkbox() {
    let mut b = banc_d_essai(1);
    ouvrir_la_vue(&mut b);
    b.app.run_action("outil.trajectoire");
    b.chauffer(2);
    assert!(!b.app.doc.plan_en_espace(0));
    let textes = textes_du_panneau(&mut b.app, Pane::ToolOptions);
    assert!(contient(&textes, "Trajectoire dans l'espace") && contient(&textes, "dans le panneau Espace 3D"), "{textes:?}");
    assert!(!contient(&textes, "plus bas"), "la case n'est plus sous ces réglages : {textes:?}");
    // La case est bien dans le panneau Espace 3D (son bouton y est, avec les autres options de la vue).
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &["Trajectoire dans l'espace"]);
}

// ------------------------------------------------------------------------------------------------ vue fermée

/// Un clic dans le panneau `pane`, au point `p` de son contexte à part.
fn cliquer(ctx: &egui::Context, app: &mut App, pane: Pane, t: &mut f64, p: Pos2) {
    let bouton = |pressed: bool| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    for ev in [vec![Event::PointerMoved(p)], vec![bouton(true)], vec![bouton(false)], Vec::new()] {
        image_du_panneau(ctx, app, pane, t, ev);
    }
}

#[test]
fn with_the_view_closed_the_pane_shows_one_sentence_and_a_button_that_opens_the_view() {
    let mut b = banc_d_essai(2);
    let (ctx, mut t) = (egui::Context::default(), 0.0);
    let mut textes = Vec::new();
    for _ in 0..4 {
        textes = image_du_panneau(&ctx, &mut b.app, Pane::Espace3d, &mut t, Vec::new());
    }
    // Une phrase et un bouton, rien d'autre.
    assert_eq!(textes.len(), 2, "{textes:?}");
    assert_eq!(textes[0].0, VUE_FERMEE);
    assert!(textes[1].0.ends_with(OUVRIR), "{textes:?}");
    // Le bouton ouvre la vue ; le panneau montre alors les cartes et la caméra.
    cliquer(&ctx, &mut b.app, Pane::Espace3d, &mut t, textes[1].1.center());
    assert!(b.app.espace3d_vue_ouverte(), "le bouton ouvre la vue ({})", b.app.status);
    b.chauffer(3);
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &[VUE_FERMEE, OUVRIR]);
    // La vue caméra cache la vue Espace 3D : le panneau le dit comme une vue fermée.
    b.app.set_camera_view(true);
    b.chauffer(2);
    verifier(&mut b.app, Pane::Espace3d, &[VUE_FERMEE, OUVRIR], &DE_LA_VUE);
    b.app.set_camera_view(false);
    // Sur la planche des Vignettes, la vue ne se montre pas : le panneau le dit, sans bouton, et Réglages y montre le pinceau.
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    assert!(b.app.board_shown() && b.app.espace3d.on && !b.app.espace3d_vue_ouverte());
    verifier(&mut b.app, Pane::Espace3d, &[SUR_LA_PLANCHE], &[OUVRIR, "Cartes (calques)"]);
    let outil = b.app.tool.label();
    verifier(&mut b.app, Pane::ToolOptions, &[outil], &DE_LA_VUE);
}

// ------------------------------------------------------------------------------------------------ sa colonne, à côté des réglages

/// La largeur de chaque groupe d'onglets d'une disposition, en part de la largeur de toute la disposition (`w` : celle de
/// ce nœud) : deux parties côte à côte se partagent la largeur, deux parties l'une sur l'autre ont chacune toute la largeur.
fn largeurs(n: &DockNode, w: f32, out: &mut Vec<(Vec<Pane>, f32)>) {
    match n {
        DockNode::Tabs { panneaux, .. } => out.push((panneaux.clone(), w)),
        DockNode::Row { part, gauche, droite } => {
            largeurs(gauche, part * w, out);
            largeurs(droite, (1.0 - part) * w, out);
        }
        DockNode::Column { haut, bas, .. } => {
            largeurs(haut, w, out);
            largeurs(bas, w, out);
        }
    }
}

/// La largeur du groupe d'onglets de `pane` dans la fenêtre principale d'une disposition, en part de sa largeur.
fn largeur(d: &DockLayout, pane: Pane) -> f32 {
    let mut tous = Vec::new();
    largeurs(&d.principal, 1.0, &mut tous);
    tous.into_iter().find(|(p, _)| p.contains(&pane)).map(|(_, w)| w).unwrap_or_else(|| panic!("{pane:?} est dans la fenêtre principale : {:?}", d.principal))
}

/// Deux dispositions ont les mêmes groupes d'onglets aux mêmes places (les parts et l'onglet de devant mis à part).
fn meme_forme(a: &DockNode, b: &DockNode) -> bool {
    match (a, b) {
        (DockNode::Tabs { panneaux: p, .. }, DockNode::Tabs { panneaux: q, .. }) => p == q,
        (DockNode::Row { gauche: a1, droite: b1, .. }, DockNode::Row { gauche: a2, droite: b2, .. }) | (DockNode::Column { haut: a1, bas: b1, .. }, DockNode::Column { haut: a2, bas: b2, .. }) => meme_forme(a1, a2) && meme_forme(b1, b2),
        _ => false,
    }
}

/// Ajoute `pane` en dernier onglet du groupe de `avec` (comme en le glissant sur ce groupe).
fn ajouter_onglet(n: &mut DockNode, avec: Pane, pane: Pane) -> bool {
    match n {
        DockNode::Tabs { panneaux, .. } if panneaux.contains(&avec) => {
            panneaux.push(pane);
            true
        }
        DockNode::Tabs { .. } => false,
        DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => ajouter_onglet(a, avec, pane) || ajouter_onglet(b, avec, pane),
    }
}

/// La largeur à l'écran, en points, du groupe d'onglets de `pane`.
fn largeur_a_l_ecran(b: &Banc, pane: Pane) -> f32 {
    let state = b.app.dock.as_ref().expect("la disposition à l'écran");
    let ici = state.find_tab(&pane).unwrap_or_else(|| panic!("{pane:?} est à l'écran"));
    match &state[ici.0][ici.1] {
        egui_dock::Node::Leaf { rect, .. } => rect.width(),
        _ => panic!("un onglet est dans une feuille"),
    }
}

/// La colonne que les parties 149 et 150 n'avaient pas : les réglages de l'outil au-dessus de la couleur, et le panneau
/// Espace 3D à leur droite, sur toute la hauteur.
const A_COTE_DES_REGLAGES: &str = "gauche: Column { part: 0.55, haut: Tabs { panneaux: [ToolOptions, Sheet], actif: 0 }, bas: Tabs { panneaux: [Color, Personnages], actif: 0 } }, droite: Tabs { panneaux: [Espace3d], actif: 0 }";

#[test]
fn the_pane_gets_a_column_of_its_own_right_of_the_tool_settings_and_the_drawing_gives_the_width() {
    let origine = DockLayout::from_legacy(&Layout::default());
    let e = dock::part_de_la_colonne_espace3d(None);
    assert!((e - dock::ESPACE3D_WIDTH / 1454.0).abs() < 1e-6 && dock::ESPACE3D_WIDTH == 340.0, "340 points dans la fenêtre de référence : {e}");
    assert!((dock::part_de_la_colonne_espace3d(Some(1700.0)) - 0.2).abs() < 1e-6, "340 points d'une fenêtre de 1 700");
    assert_eq!((dock::part_de_la_colonne_espace3d(Some(400.0)), dock::part_de_la_colonne_espace3d(Some(1.0e5))), (0.4, 0.12), "bornée dans une fenêtre minuscule ou immense");
    let mut d = origine.clone();
    assert!(d.poser_en_colonne(Pane::Espace3d, e));
    // Les réglages de l'outil au-dessus de la couleur, et le panneau à leur droite : les trois se voient en même temps.
    let s = format!("{:?}", d.principal);
    assert!(s.contains(A_COTE_DES_REGLAGES), "{s}");
    assert!(d.en_colonne(Pane::Espace3d) && !origine.en_colonne(Pane::Espace3d));
    assert_eq!(d.panes().iter().filter(|p| **p == Pane::Espace3d).count(), 1);
    for p in [Pane::ToolOptions, Pane::Color, Pane::Layers, Pane::Effects] {
        assert_eq!(onglets_de(&d, p), onglets_de(&origine, p), "{p:?} garde ses onglets et celui de devant");
        assert!((largeur(&d, p) - largeur(&origine, p)).abs() < 1e-4, "{p:?} garde sa largeur : {} au lieu de {}", largeur(&d, p), largeur(&origine, p));
    }
    // Sa largeur est prise au dessin ; la bande du bas garde toute la largeur.
    assert!((largeur(&d, Pane::Espace3d) - e).abs() < 1e-4, "{}", largeur(&d, Pane::Espace3d));
    assert!((largeur(&d, Pane::Canvas) - (largeur(&origine, Pane::Canvas) - e)).abs() < 1e-4, "{} au lieu de {}", largeur(&d, Pane::Canvas), largeur(&origine, Pane::Canvas) - e);
    assert!((largeur(&d, Pane::Montage) - 1.0).abs() < 1e-6);
    // L'aller-retour par egui_dock ne la perd pas.
    assert!(dock::from_state(&dock::to_state(&d), &|_| None).close_to(&d));
    // Une seconde fois : rien (il y est déjà). « Ouvrir » le panneau, c'est le poser là, à la largeur d'origine.
    let avant = d.clone();
    assert!(!d.poser_en_colonne(Pane::Espace3d, e));
    assert_eq!(d, avant);
    let mut ouvert = origine.clone();
    ouvert.open(Pane::Espace3d);
    assert_eq!(ouvert, d, "« ouvrir » le panneau le pose dans cette colonne");
    // Retirée : la disposition d'origine revient, le dessin retrouve sa largeur.
    assert!(d.retirer_la_colonne(Pane::Espace3d));
    assert!(d.close_to(&origine), "{:#?}\n{:#?}", d.principal, origine.principal);
    assert!(!d.retirer_la_colonne(Pane::Espace3d), "elle n'y est plus");
    // Rangé ailleurs à la main (un onglet avec les calques) : ce n'est pas sa colonne, rien n'est retiré.
    let mut ailleurs = origine.clone();
    assert!(ajouter_onglet(&mut ailleurs.principal, Pane::Layers, Pane::Espace3d));
    let avant = ailleurs.clone();
    assert!(!ailleurs.en_colonne(Pane::Espace3d) && !ailleurs.retirer_la_colonne(Pane::Espace3d));
    assert_eq!(ailleurs, avant);
    // Un dessin déjà étroit (une colonne des calques très large) garde de quoi dessiner : un cinquième de la largeur.
    let mut etroit = DockLayout::from_legacy(&Layout { sheet_width: 800.0, ..Layout::default() });
    assert!(largeur(&etroit, Pane::Canvas) < 0.25);
    assert!(etroit.poser_en_colonne(Pane::Espace3d, e));
    assert!(largeur(&etroit, Pane::Canvas) >= 0.2 - 1e-4 && largeur(&etroit, Pane::Espace3d) > 0.12, "{} et {}", largeur(&etroit, Pane::Canvas), largeur(&etroit, Pane::Espace3d));
    // Les Vignettes (ouvert à la main) : à droite de toute la colonne des réglages (la planche, l'outil, la couleur).
    let mut d = DockLayout::pour_les_vignettes();
    let dessin = largeur(&d, Pane::Canvas);
    d.open(Pane::Espace3d);
    assert!(d.en_colonne(Pane::Espace3d));
    let s = format!("{:?}", d.principal);
    assert!(s.contains("bas: Tabs { panneaux: [Color, Scenario], actif: 0 } } }, droite: Tabs { panneaux: [Espace3d], actif: 0 }"), "{s}");
    assert!((largeur(&d, Pane::Canvas) - (dessin - e)).abs() < 1e-4 && (largeur(&d, Pane::ToolOptions) - largeur(&DockLayout::pour_les_vignettes(), Pane::ToolOptions)).abs() < 1e-4);
    assert!(d.retirer_la_colonne(Pane::Espace3d) && d.close_to(&DockLayout::pour_les_vignettes()));
}

#[test]
fn without_the_tool_settings_in_the_main_window_the_pane_opens_as_a_tab_of_its_group() {
    // Le Montage (ni réglages de l'outil ni couleur) : en onglet avec la fiche du plan.
    let mut d = DockLayout::pour_le_montage(440.0);
    assert!(!d.poser_en_colonne(Pane::Espace3d, 0.2));
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Sheet, Pane::Espace3d]));
    assert!(!d.en_colonne(Pane::Espace3d));
    // Les réglages de l'outil fermés : avec les autres panneaux de sa colonne (la fiche, restée seule à leur place).
    let mut d = DockLayout::from_legacy(&Layout::default());
    d.close(Pane::ToolOptions);
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Sheet, Pane::Espace3d]));
    // Les réglages de l'outil dans un panneau flottant : pas de colonne à côté d'eux, un onglet dans la fenêtre principale.
    let mut d = DockLayout::from_legacy(&Layout::default());
    d.close(Pane::ToolOptions);
    d.close(Pane::Sheet);
    d.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::ToolOptions]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 320.0 });
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Color, Pane::Personnages, Pane::Espace3d]));
    assert_eq!(d.fenetres[0].panneaux, tabs(&[Pane::ToolOptions]));
    // Le groupe « Colonne des réglages et fiche » le ferme avec les autres, et ne le rouvre pas : sa colonne n'arrive
    // qu'avec la vue, ou à la demande.
    let mut d = DockLayout::from_legacy(&Layout::default());
    d.open(Pane::Espace3d);
    d.toggle_group(Group::Left);
    assert!(!d.contains(Pane::Espace3d) && !d.contains(Pane::Color) && !d.contains(Pane::ToolOptions));
    d.toggle_group(Group::Left);
    assert!(!d.contains(Pane::Espace3d) && d.contains(Pane::Color) && d.contains(Pane::ToolOptions));
}

// ------------------------------------------------------------------------------------------------ il suit la vue

#[test]
fn opening_the_view_brings_the_pane_beside_the_tool_settings_and_closing_it_takes_the_column_away() {
    let mut b = banc_d_essai(2);
    // (Le temps que le bord entre les réglages de l'outil et la couleur finisse de glisser.)
    b.chauffer(30);
    // Au repos : pas de panneau Espace 3D, il ne prend aucune place.
    let origine = b.app.dock_layout();
    assert!(!b.app.pane_shown(Pane::Espace3d) && !origine.contains(Pane::Espace3d));
    assert!(devant(&b, Pane::Color) && devant(&b, Pane::ToolOptions) && devant(&b, Pane::Layers));
    let (dessin, reglages, calques) = (largeur_a_l_ecran(&b, Pane::Canvas), largeur_a_l_ecran(&b, Pane::ToolOptions), largeur_a_l_ecran(&b, Pane::Layers));
    // La vue s'ouvre : sa colonne arrive à droite de celle des réglages de l'outil. Réglages, la couleur, les calques et
    // lui se voient en même temps.
    ouvrir_la_vue(&mut b);
    b.chauffer(30);
    let d = b.app.dock_layout();
    assert!(d.en_colonne(Pane::Espace3d), "{:?}", d.principal);
    assert!(format!("{:?}", d.principal).contains("droite: Tabs { panneaux: [Espace3d], actif: 0 }"));
    assert!(devant(&b, Pane::Espace3d) && devant(&b, Pane::ToolOptions) && devant(&b, Pane::Color) && devant(&b, Pane::Layers));
    assert!(b.app.e3_panneau.colonne && b.app.e3_panneau.avant.is_none());
    // 340 points pour lui, pris au dessin ; les réglages de l'outil et les calques gardent leur largeur (aux bords près).
    let panneau = largeur_a_l_ecran(&b, Pane::Espace3d);
    assert!((panneau - dock::ESPACE3D_WIDTH).abs() < 8.0, "le panneau fait {panneau} points de large");
    assert!((largeur_a_l_ecran(&b, Pane::ToolOptions) - reglages).abs() < 8.0, "les réglages de l'outil : {} au lieu de {reglages}", largeur_a_l_ecran(&b, Pane::ToolOptions));
    assert!((largeur_a_l_ecran(&b, Pane::Layers) - calques).abs() < 8.0);
    assert!((largeur_a_l_ecran(&b, Pane::Canvas) - (dessin - dock::ESPACE3D_WIDTH)).abs() < 10.0, "le dessin : {} au lieu de {}", largeur_a_l_ecran(&b, Pane::Canvas), dessin - dock::ESPACE3D_WIDTH);
    assert!(largeur_a_l_ecran(&b, Pane::Canvas) > 300.0, "il reste de quoi voir l'espace : {}", largeur_a_l_ecran(&b, Pane::Canvas));
    // La disposition retenue (celle qu'on enregistre, et que l'espace retrouve) est celle sans la colonne.
    assert!(!b.app.fit_busy);
    let retenue = b.app.current_layout().dock_layout();
    assert!(!retenue.contains(Pane::Espace3d) && meme_forme(&retenue.principal, &origine.principal), "{:?}", retenue.principal);
    assert!((largeur(&retenue, Pane::Canvas) - largeur(&origine, Pane::Canvas)).abs() < 0.01 && (largeur(&retenue, Pane::ToolOptions) - largeur(&origine, Pane::ToolOptions)).abs() < 0.01);
    let reglages_ecrits = b.app.settings.layout_of(Espace::Storyboard).expect("la disposition du Storyboard est retenue").dock_layout();
    assert!(!reglages_ecrits.contains(Pane::Espace3d), "dans les réglages non plus");
    // La vue se ferme : la colonne repart, le dessin retrouve sa largeur.
    fermer_la_vue(&mut b);
    b.chauffer(30);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    assert!(meme_forme(&b.app.dock_layout().principal, &origine.principal));
    assert!((largeur_a_l_ecran(&b, Pane::Canvas) - dessin).abs() < 8.0 && (largeur_a_l_ecran(&b, Pane::ToolOptions) - reglages).abs() < 8.0);
    assert_eq!(b.app.e3_panneau, crate::espace3dpanneau::Suivi::default());
    assert!(devant(&b, Pane::Color) && devant(&b, Pane::ToolOptions));
    // La vue caméra cache la vue Espace 3D (elle reste allumée derrière) : la colonne repart ; de retour, elle revient.
    ouvrir_la_vue(&mut b);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d));
    b.app.set_camera_view(true);
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    b.app.set_camera_view(false);
    b.chauffer(2);
    assert!(b.app.espace3d_vue_ouverte() && b.app.dock_layout().en_colonne(Pane::Espace3d));
}

#[test]
fn a_pane_placed_or_opened_by_hand_is_respected_the_view_never_moves_it() {
    let mut b = banc_d_essai(2);
    // Rangé à la main en onglet avec les calques : la vue le met devant là où il est, et rend l'onglet d'avant en se fermant.
    let mut d = b.app.dock_layout();
    assert!(ajouter_onglet(&mut d.principal, Pane::Layers, Pane::Espace3d));
    b.app.set_dock_layout(d);
    b.chauffer(2);
    assert!(devant(&b, Pane::Layers) && groupe(&b, Pane::Espace3d).contains(&Pane::Layers));
    ouvrir_la_vue(&mut b);
    assert!(devant(&b, Pane::Espace3d) && !b.app.dock_layout().en_colonne(Pane::Espace3d), "il passe devant, sans changer de place");
    assert_eq!((b.app.e3_panneau.avant, b.app.e3_panneau.colonne), (Some(Pane::Layers), false));
    assert_eq!(onglets_de(&b.app.current_layout().dock_layout(), Pane::Espace3d).map(|g| g.1), Some(Pane::Layers), "la disposition retenue garde les calques devant");
    fermer_la_vue(&mut b);
    assert!(devant(&b, Pane::Layers) && groupe(&b, Pane::Espace3d).contains(&Pane::Layers), "les calques reviennent devant, le panneau reste où on l'a mis");
    // Ouvert à la main, la vue fermée (son icône, sa commande) : dans sa colonne, et il y reste quand la vue se ferme.
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d) && !b.app.e3_panneau.colonne);
    ouvrir_la_vue(&mut b);
    fermer_la_vue(&mut b);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d), "ouvert à la main : il reste");
    assert!(b.app.current_layout().dock_layout().en_colonne(Pane::Espace3d), "et la disposition retenue l'a");
    // Fermé à la main pendant que la vue est ouverte : il reste fermé jusqu'à la prochaine ouverture de la vue.
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    ouvrir_la_vue(&mut b);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d) && b.app.e3_panneau.colonne, "la vue le ramène");
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(3);
    assert!(!b.app.pane_shown(Pane::Espace3d) && b.app.espace3d_vue_ouverte() && !b.app.e3_panneau.colonne);
    fermer_la_vue(&mut b);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    // Fermé puis rouvert à la main pendant que la vue est ouverte : il est à l'utilisateur, la vue ne le retire plus.
    ouvrir_la_vue(&mut b);
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    fermer_la_vue(&mut b);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d));
    // La colonne posée par la vue, glissée ailleurs pendant qu'elle est ouverte : le panneau reste où on l'a glissé.
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    ouvrir_la_vue(&mut b);
    assert!(b.app.e3_panneau.colonne);
    let mut d = b.app.dock_layout();
    d.close(Pane::Espace3d);
    assert!(ajouter_onglet(&mut d.principal, Pane::Layers, Pane::Espace3d));
    b.app.set_dock_layout(d);
    b.chauffer(2);
    assert!(b.app.current_layout().dock_layout().contains(Pane::Espace3d), "rangé à la main : la disposition retenue le garde");
    fermer_la_vue(&mut b);
    assert!(b.app.pane_shown(Pane::Espace3d) && groupe(&b, Pane::Espace3d).contains(&Pane::Layers));
}

#[test]
fn each_work_space_gets_the_column_while_the_view_is_open_and_a_layout_without_tool_settings_a_tab() {
    let mut b = banc_d_essai(2);
    ouvrir_la_vue(&mut b);
    b.chauffer(30);
    // On entre en animation, la vue toujours ouverte : la colonne arrive dans la disposition de l'Animation, et celle que
    // le Storyboard retient ne l'a pas.
    b.app.enter_animation(0);
    b.chauffer(3);
    assert!(b.app.espace == Espace::Animation && b.app.espace3d_vue_ouverte());
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d) && devant(&b, Pane::ToolOptions), "{:?}", b.app.dock_layout().principal);
    let storyboard = b.app.settings.layout_of(Espace::Storyboard).expect("retenue en quittant le Storyboard").dock_layout();
    assert!(!storyboard.contains(Pane::Espace3d));
    // Retour au Storyboard : la colonne y revient ; la vue fermée, elle repart. (Le temps que les bords de l'Animation
    // finissent de glisser : sa disposition n'est retenue qu'ensuite.)
    b.chauffer(30);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d));
    let animation = b.app.settings.layout_of(Espace::Animation).expect("la disposition de l'Animation est retenue").dock_layout();
    assert!(!animation.contains(Pane::Espace3d));
    fermer_la_vue(&mut b);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    // « Rétablir la disposition », la vue ouverte : la disposition d'origine, et la colonne.
    ouvrir_la_vue(&mut b);
    b.app.run_action("espace.retablir");
    b.chauffer(3);
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d) && b.app.e3_panneau.colonne);
    fermer_la_vue(&mut b);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    // Sans les réglages de l'outil dans la disposition : pas de colonne à côté d'eux. Le panneau arrive en onglet, devant,
    // et l'onglet d'avant revient à la fermeture (comme à la partie 150 ; le panneau reste alors dans la disposition).
    b.app.toggle_pane(Pane::ToolOptions);
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::ToolOptions));
    ouvrir_la_vue(&mut b);
    assert!(devant(&b, Pane::Espace3d) && !b.app.dock_layout().en_colonne(Pane::Espace3d) && !b.app.e3_panneau.colonne);
    let avant = b.app.e3_panneau.avant.expect("un onglet était devant");
    fermer_la_vue(&mut b);
    assert!(devant(&b, avant) && b.app.pane_shown(Pane::Espace3d));
}

// ------------------------------------------------------------------------------------------------ les dispositions d'origine

#[test]
fn the_default_layouts_no_longer_hold_the_pane_it_only_comes_with_the_view() {
    let colonne_des_reglages = "haut: Tabs { panneaux: [ToolOptions, Sheet], actif: 0 }, bas: Tabs { panneaux: [Color, Personnages], actif: 0 }";
    for e in Espace::TOUS {
        assert!(!e.defaut().dock_layout().contains(Pane::Espace3d), "{e:?}");
    }
    // Le Storyboard et l'Animation : les réglages de l'outil au-dessus de la couleur, comme avant la partie 150.
    for e in [Espace::Storyboard, Espace::Animation] {
        let s = format!("{:?}", e.defaut().dock_layout().principal);
        assert!(s.contains(colonne_des_reglages), "{e:?} : {s}");
    }
    for (action, _, layout) in workspace::builtin() {
        let d = layout.dock_layout();
        assert!(!d.contains(Pane::Espace3d), "{action}");
        assert_eq!(onglets_de(&d, Pane::Color), Some((vec![Pane::Color, Pane::Personnages], Pane::Color)), "{action}");
    }
}

#[test]
fn the_tab_that_was_in_front_comes_back_only_if_the_pane_is_still_in_front_of_it() {
    let origine = disposition_150();
    // Le panneau devant la couleur : elle repasse devant.
    let mut d = origine.clone();
    assert!(mettre_devant(&mut d.principal, Pane::Espace3d));
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.1), Some(Pane::Espace3d));
    assert!(d.remettre_devant(Pane::Espace3d, Pane::Color));
    assert_eq!(d, origine);
    // Les brosses dédiées étaient devant : ce sont elles qui reviennent.
    let mut d = origine.clone();
    mettre_devant(&mut d.principal, Pane::Espace3d);
    assert!(d.remettre_devant(Pane::Espace3d, Pane::Personnages));
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.1), Some(Pane::Personnages));
    // Le panneau n'est pas devant (l'utilisateur a mis un autre onglet devant) : rien ne change.
    let mut d = origine.clone();
    mettre_devant(&mut d.principal, Pane::Personnages);
    let avant = d.clone();
    assert!(!d.remettre_devant(Pane::Espace3d, Pane::Color));
    assert_eq!(d, avant);
    // L'onglet d'avant n'est plus dans son groupe (les calques, ailleurs ; la couleur, fermée) : rien ne change.
    let mut d = origine.clone();
    mettre_devant(&mut d.principal, Pane::Espace3d);
    let avant = d.clone();
    assert!(!d.remettre_devant(Pane::Espace3d, Pane::Layers));
    assert_eq!(d, avant);
    d.close(Pane::Color);
    assert!(!d.remettre_devant(Pane::Espace3d, Pane::Color));
    // Le panneau absent de la disposition : rien.
    let mut d = disposition_d_avant();
    assert!(!d.remettre_devant(Pane::Espace3d, Pane::Color));
    // Dans un panneau flottant, avec la couleur : pareil que dans la fenêtre principale.
    let mut d = disposition_d_avant();
    d.close(Pane::Color);
    d.fenetres.push(FloatingPanel { panneaux: DockNode::Tabs { panneaux: vec![Pane::Color, Pane::Espace3d], actif: 1 }, x: 40.0, y: 50.0, largeur: 300.0, hauteur: 320.0 });
    assert!(d.remettre_devant(Pane::Espace3d, Pane::Color));
    assert_eq!(d.fenetres[0].panneaux, tabs(&[Pane::Color, Pane::Espace3d]));
}

// ------------------------------------------------------------------------------------------------ les dispositions retenues

#[test]
fn retained_layouts_that_had_the_pane_with_the_colour_lose_it_once_and_a_pane_placed_by_hand_stays() {
    // Le Storyboard : la disposition de la partie 150 (le panneau en onglet avec la couleur), que l'utilisateur a faite
    // sienne : la bande plus haute, les brosses dédiées devant, la fiche dans un panneau flottant.
    let mut de_150 = disposition_150();
    de_150.close(Pane::Sheet);
    de_150.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Sheet]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 200.0 });
    let DockNode::Column { part, .. } = &mut de_150.principal else { panic!("la bande est en bas de la disposition standard") };
    *part = 0.52;
    assert!(mettre_devant(&mut de_150.principal, Pane::Personnages));
    // L'Animation : le panneau rangé à la main, en onglet avec les calques.
    let mut a_la_main = disposition_d_avant();
    assert!(ajouter_onglet(&mut a_la_main.principal, Pane::Layers, Pane::Espace3d));
    // Trois espaces enregistrés : le panneau flottant, seul (rangé à la main) ; seul juste sous les réglages de l'outil (la
    // place de la partie 150 sans la couleur) ; avec la couleur dans un panneau flottant (la partie 150 l'y rejoignait).
    let mut flottant = disposition_d_avant();
    flottant.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Espace3d]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 320.0 });
    let mut sous_les_reglages = disposition_d_avant();
    sous_les_reglages.close(Pane::Color);
    sous_les_reglages.close(Pane::Personnages);
    let sans_couleur = sous_les_reglages.clone();
    fn dessous(n: &mut DockNode) -> bool {
        match n {
            DockNode::Tabs { panneaux, .. } if panneaux.contains(&Pane::ToolOptions) => {
                let haut = std::mem::replace(n, DockNode::Tabs { panneaux: Vec::new(), actif: 0 });
                *n = DockNode::Column { part: 0.55, haut: Box::new(haut), bas: Box::new(DockNode::Tabs { panneaux: vec![Pane::Espace3d], actif: 0 }) };
                true
            }
            DockNode::Tabs { .. } => false,
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => dessous(a) || dessous(b),
        }
    }
    assert!(dessous(&mut sous_les_reglages.principal));
    let mut couleur_flottante = disposition_d_avant();
    couleur_flottante.close(Pane::Color);
    couleur_flottante.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Color, Pane::Espace3d]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 320.0 });
    let espace = |nom: &str, d: &DockLayout| Workspace { name: nom.into(), layout: Layout { panneaux: Some(d.clone()), ..Layout::default() } };
    let mut s = Settings::default();
    s.panneau_espace3d_pose = true;
    s.layout = Some(Layout { panneaux: Some(de_150.clone()), ..Layout::default() });
    s.animation_layout = Some(Layout { panneaux: Some(a_la_main.clone()), ..Espace::Animation.defaut() });
    s.montage_layout = Some(Layout { montage_height: 500.0, panneaux: Some(DockLayout::pour_le_montage(500.0)), ..Espace::Montage.defaut() });
    s.workspaces = vec![espace("Flottant", &flottant), espace("Sous les réglages", &sous_les_reglages), espace("Couleur flottante", &couleur_flottante)];
    // Le fichier des réglages, tel que la version de la partie 150 l'a écrit.
    let texte = toml::to_string(&s).expect("écrit");
    assert!(texte.contains("\"espace3d\"") && texte.contains("panneau_espace3d_pose = true") && !texte.contains("panneau_espace3d_a_cote"), "{texte}");
    let lu = Settings::apres_lecture(toml::from_str(&texte).expect("lu"));
    assert!(lu.panneau_espace3d_pose && lu.panneau_espace3d_a_cote);
    // Le Storyboard : le panneau a quitté le groupe de la couleur ; les brosses dédiées sont restées devant, la bande, la
    // fiche flottante et tout le reste n'ont pas bougé.
    let d = lu.layout.as_ref().expect("storyboard").dock_layout();
    let mut sans = de_150.clone();
    sans.close(Pane::Espace3d);
    assert!(!d.contains(Pane::Espace3d) && d.close_to(&sans), "{d:#?}\n{sans:#?}");
    assert_eq!(onglets_de(&d, Pane::Color), Some((vec![Pane::Color, Pane::Personnages], Pane::Personnages)));
    assert_eq!(d.fenetres.len(), 1, "le panneau flottant de la fiche est resté");
    // L'Animation : rangé à la main avec les calques, il y reste, et la disposition n'a pas changé.
    let d = lu.animation_layout.as_ref().expect("animation").dock_layout();
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Layers, Pane::Espace3d]));
    assert!(d.close_to(&a_la_main));
    assert_eq!(lu.animation_layout.as_ref().unwrap().montage_height, Espace::Animation.defaut().montage_height, "les autres réglages de la disposition restent");
    assert!(lu.montage_layout.as_ref().unwrap().dock_layout().close_to(&DockLayout::pour_le_montage(500.0)));
    // Les espaces enregistrés : flottant seul, il reste ; sous les réglages ou avec la couleur flottante, il part.
    let (f, r, c) = (lu.workspaces[0].layout.dock_layout(), lu.workspaces[1].layout.dock_layout(), lu.workspaces[2].layout.dock_layout());
    assert!(f.close_to(&flottant) && f.fenetres[0].panneaux == tabs(&[Pane::Espace3d]));
    assert!(!r.contains(Pane::Espace3d) && r.close_to(&sans_couleur), "{r:#?}");
    assert!(!c.contains(Pane::Espace3d) && c.fenetres[0].panneaux == tabs(&[Pane::Color]));
    // Réécrit puis relu : ce n'est pas refait. Remis ensuite à la main avec la couleur, le panneau y reste.
    let mut remis = lu;
    remis.layout.as_mut().unwrap().panneaux = Some(de_150.clone());
    let texte = toml::to_string(&remis).expect("écrit");
    assert!(texte.contains("panneau_espace3d_a_cote = true"), "{texte}");
    let relu = Settings::apres_lecture(toml::from_str(&texte).expect("relu"));
    assert!(relu.layout.as_ref().unwrap().dock_layout().close_to(&de_150), "remis à la main ensuite : il y reste");
    // Des réglages d'avant la partie 150 (une disposition sans le panneau, aucune marque) : rien n'y est ajouté.
    let mut s = Settings::default();
    s.layout = Some(Layout { panneaux: Some(disposition_d_avant()), ..Layout::default() });
    let texte = toml::to_string(&s).expect("écrit");
    assert!(!texte.contains("\"espace3d\"") && !texte.contains("panneau_espace3d"), "{texte}");
    let lu = Settings::apres_lecture(toml::from_str(&texte).expect("lu"));
    assert!(lu.panneau_espace3d_pose && lu.panneau_espace3d_a_cote);
    assert!(lu.layout.as_ref().unwrap().dock_layout().close_to(&disposition_d_avant()), "le panneau n'arrive qu'avec la vue");
    // Des réglages sans disposition retenue (un poste neuf) : rien à ranger.
    let neuf = Settings::apres_lecture(Settings::default());
    assert!(neuf.panneau_espace3d_pose && neuf.panneau_espace3d_a_cote && neuf.layout.is_none() && neuf.animation_layout.is_none());
}

// ------------------------------------------------------------------------------------------------ sa commande, son icône, les messages

#[test]
fn the_pane_has_its_name_its_cube_its_command_its_icon_in_the_bar_and_its_line_in_the_display_menu() {
    assert_eq!((Pane::Espace3d.name(), Pane::Espace3d.tab_name()), ("Espace 3D", "Espace 3D"));
    assert_eq!(Pane::Espace3d.icon(), egui_phosphor::regular::CUBE, "le cube du bouton de la vue");
    assert_eq!(Pane::Espace3d.group(), Group::Left);
    assert!(Pane::Espace3d.on_demand() && !Pane::Espace3d.animation_only(), "fermé d'origine : il s'ouvre avec la vue, ou à la demande");
    assert_eq!(serde_json::to_string(&Pane::Espace3d).unwrap(), "\"espace3d\"");
    // Menu Affichage > Panneaux : une case par panneau de `Pane::ALL`.
    assert!(Pane::ALL.contains(&Pane::Espace3d));
    // La commande (palette, raccourci à choisir) et l'icône de la barre des panneaux.
    let action = "vue.espace3d_panneau";
    let info = crate::keymap::action_info(action).expect("la commande existe");
    assert!(info.implemented && info.label == "Panneau Espace 3D (ouvrir / fermer)");
    assert!(crate::palette::palette_commands().iter().any(|(id, _)| *id == action), "elle est dans la palette");
    let map = crate::keymap_tests::default_map();
    assert!(map.bindings.iter().any(|b| b.action == action) && map.keys_of(action).is_empty(), "nommée dans keymap.toml, sans touche d'origine");
    assert_eq!(crate::barrepanneaux::pane_de_action(action), Some(Pane::Espace3d));
    let e = crate::barrepanneaux::entree_de_action(action).expect("son icône");
    assert!(e.nom == "Panneau Espace 3D" && e.description.contains("vue Espace 3D"));
    assert!(e.description.contains("à côté des réglages de l'outil") && !e.description.contains("devant la couleur"), "{}", e.description);
    // La commande ouvre le panneau (dans sa colonne, à côté des réglages de l'outil) ou le ferme, comme le clic sur son icône.
    let mut b = banc_d_essai(1);
    assert!(!b.app.pane_shown(Pane::Espace3d), "d'origine, il n'est pas dans la disposition");
    b.app.run_action(action);
    b.chauffer(1);
    assert!(b.app.pane_shown(Pane::Espace3d) && devant(&b, Pane::Espace3d) && devant(&b, Pane::ToolOptions) && devant(&b, Pane::Color), "fermé : la commande l'ouvre, à côté des réglages");
    assert!(b.app.dock_layout().en_colonne(Pane::Espace3d));
    b.app.run_action(action);
    b.chauffer(1);
    assert!(!b.app.pane_shown(Pane::Espace3d), "devant : la commande le ferme");
    // Rangé en onglet derrière un autre : elle le met devant.
    let mut d = b.app.dock_layout();
    assert!(ajouter_onglet(&mut d.principal, Pane::Layers, Pane::Espace3d));
    b.app.set_dock_layout(d);
    b.chauffer(1);
    assert!(b.app.pane_shown(Pane::Espace3d) && !devant(&b, Pane::Espace3d));
    b.app.run_action(action);
    b.chauffer(1);
    assert!(devant(&b, Pane::Espace3d), "derrière les calques : la commande le met devant");
}

#[test]
fn the_messages_send_to_the_space_pane_and_no_longer_to_the_settings_pane() {
    use crate::trajectoire3d as t3;
    for message in [crate::espace3ddessin::CARTES_CACHEES, t3::PLAN_ORDINAIRE, t3::RIEN_A_DEPLACER] {
        assert!(message.contains("panneau Espace 3D") && !message.contains("Réglages"), "{message}");
    }
    let mut b = banc_d_essai(1);
    // La vue s'ouvre sur un plan ordinaire, puis sur un plan en Espace 3D.
    b.app.run_action("espace3d.basculer");
    assert!(b.app.status.contains("Réglages de la vue : le panneau Espace 3D.") && !b.app.status.contains("panneau Réglages"), "{}", b.app.status);
    // « Nouvelle surface » hors d'un plan en Espace 3D : la case est dans le panneau Espace 3D.
    b.app.run_action("espace3d.surface_sol");
    assert!(b.app.status.contains("« Plan en Espace 3D » dans le panneau Espace 3D") && !b.app.status.contains("dans Réglages"), "{}", b.app.status);
    // La légende de l'outil Trajectoire dans un plan ordinaire.
    b.app.run_action("outil.trajectoire");
    b.chauffer(2);
    assert_eq!(b.app.status, t3::PLAN_ORDINAIRE);
    let legende = b.app.e3_trajectoire_legende();
    assert!(legende.contains("(panneau Espace 3D)") && !legende.contains("Réglages"), "{legende}");
    b.app.run_action("espace3d.plan_en_espace");
    b.app.run_action("espace3d.basculer");
    b.app.run_action("espace3d.basculer");
    assert!(b.app.status.contains("« Nouvelle surface » dans le panneau Espace 3D") && !b.app.status.contains("dans Réglages"), "{}", b.app.status);
    // L'outil Caméra sur la feuille d'un plan en Espace 3D : les champs de la caméra sont dans le panneau Espace 3D.
    b.app.run_action("espace3d.basculer");
    b.app.camera_press(pos2(10.0, 10.0), pos2(10.0, 10.0));
    assert!(b.app.status.contains("ses champs dans le panneau Espace 3D") && !b.app.status.contains("dans Réglages"), "{}", b.app.status);
}
