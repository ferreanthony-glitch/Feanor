//! Essais du panneau « Espace 3D » (`espace3dpanneau.rs`, `docs/INTERFACE.md` partie 150) : ce que montrent le panneau
//! Réglages et le panneau Espace 3D quand la vue est ouverte (le pinceau, la Trajectoire dans l'espace) et fermée ; sa
//! place d'origine, en onglet avec la couleur sous les réglages de l'outil ; il passe devant quand la vue s'ouvre et rend
//! sa place quand elle se ferme ; les dispositions enregistrées avant lui le reçoivent sans être refaites ; sa commande,
//! son icône, et les messages qui disaient « dans Réglages ».

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

/// La disposition standard telle qu'elle était avant le panneau Espace 3D : la couleur et les brosses dédiées sous les
/// réglages de l'outil, sans lui.
fn disposition_d_avant() -> DockLayout {
    let mut d = DockLayout::from_legacy(&Layout::default());
    d.close(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Color), Some((vec![Pane::Color, Pane::Personnages], Pane::Color)));
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
    // Sans chemin encore : qui est choisi et les deux sortes de clés sont dans Réglages (le mode d'emploi est dans l'aide animée et le
    // registre des explications, partie 153).
    let (images, interpolation) = (Cles::Images.label(), Cles::Interpolation.label());
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Trajectoire.label(), "Trajectoire dans l'espace", "Choisi : 1 carte.", images, interpolation], &DE_LA_VUE);
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
    let commandes = ["Choisi : 1 carte. 1 marque", "Partir de la tête de lecture", "Finir à la tête de lecture", "Temps égaux entre les marques", images, interpolation, "Retirer les marques", "Détacher", "Effacer"];
    verifier(&mut b.app, Pane::ToolOptions, &commandes, &DE_LA_VUE);
    verifier(&mut b.app, Pane::Espace3d, &DE_LA_VUE, &["Trajectoire dans l'espace", "Choisi :", "Partir de la tête de lecture", "Détacher", "Retirer les marques", "Temps égaux"]);
    // Sur la feuille, pour un calque sans chemin dans l'espace : les réglages de la trajectoire à plat, comme avant.
    fermer_la_vue(&mut b);
    b.app.cur_layer = 1;
    b.chauffer(1);
    verifier(&mut b.app, Pane::ToolOptions, &[Tool::Trajectoire.label(), "Lissage"],&["Trajectoire dans l'espace", "Choisi :"]);
}

#[test]
fn in_an_ordinary_plan_the_trajectory_settings_point_to_the_space_pane_for_the_checkbox() {
    let mut b = banc_d_essai(1);
    ouvrir_la_vue(&mut b);
    b.app.run_action("outil.trajectoire");
    b.chauffer(2);
    assert!(!b.app.doc.plan_en_espace(0));
    let textes = textes_du_panneau(&mut b.app, Pane::ToolOptions);
    assert!(contient(&textes, "Trajectoire dans l'espace") && contient(&textes, "du panneau Espace 3D"), "{textes:?}");
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

// ------------------------------------------------------------------------------------------------ il suit la vue

#[test]
fn opening_the_view_brings_the_pane_in_front_of_the_colour_and_closing_it_gives_the_tab_back() {
    let mut b = banc_d_essai(2);
    // D'origine : la couleur devant, le panneau derrière elle dans le même groupe, sous les réglages de l'outil.
    assert_eq!(groupe(&b, Pane::Espace3d), vec![Pane::Color, Pane::Personnages, Pane::Espace3d]);
    assert!(devant(&b, Pane::Color) && !devant(&b, Pane::Espace3d) && devant(&b, Pane::ToolOptions) && devant(&b, Pane::Layers));
    // La vue s'ouvre : il prend la place de la couleur ; Réglages et Calques se voient toujours.
    ouvrir_la_vue(&mut b);
    assert!(devant(&b, Pane::Espace3d) && !devant(&b, Pane::Color), "le panneau est devant la couleur");
    assert!(devant(&b, Pane::ToolOptions) && devant(&b, Pane::Layers), "Réglages et Calques se voient en même temps que lui");
    assert_eq!(b.app.e3_panneau.avant, Some(Pane::Color));
    // La vue se ferme : la couleur revient ; le panneau reste, derrière elle.
    fermer_la_vue(&mut b);
    assert!(devant(&b, Pane::Color) && !devant(&b, Pane::Espace3d) && b.app.pane_shown(Pane::Espace3d));
    assert_eq!(b.app.e3_panneau.avant, None);
    // C'est l'onglet d'avant qui revient, quel qu'il soit : ici les brosses dédiées.
    b.app.show_pane(Pane::Personnages);
    b.chauffer(1);
    ouvrir_la_vue(&mut b);
    assert!(devant(&b, Pane::Espace3d));
    // La vue caméra cache la vue Espace 3D (elle reste allumée derrière) : l'onglet d'avant revient ; de retour, le panneau aussi.
    b.app.set_camera_view(true);
    b.chauffer(2);
    assert!(devant(&b, Pane::Personnages) && !devant(&b, Pane::Espace3d));
    b.app.set_camera_view(false);
    b.chauffer(2);
    assert!(b.app.espace3d_vue_ouverte() && devant(&b, Pane::Espace3d));
    // Pendant que la vue est ouverte, la couleur reste à un clic ; mise devant à la main, elle y reste à la fermeture
    // (rien n'est défait derrière l'utilisateur).
    b.app.show_pane(Pane::Color);
    b.chauffer(2);
    assert!(devant(&b, Pane::Color) && b.app.espace3d_vue_ouverte(), "le panneau ne se bat pas pour repasser devant");
    fermer_la_vue(&mut b);
    assert!(devant(&b, Pane::Color) && !devant(&b, Pane::Personnages));
}

#[test]
fn the_view_puts_the_pane_back_at_its_home_when_the_layout_no_longer_has_it() {
    let mut b = banc_d_essai(2);
    // Fermé à la main, la vue fermée : il reste fermé.
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    // La vue s'ouvre : il revient avec la couleur, devant elle.
    ouvrir_la_vue(&mut b);
    assert_eq!(groupe(&b, Pane::Espace3d), vec![Pane::Color, Pane::Personnages, Pane::Espace3d]);
    assert!(devant(&b, Pane::Espace3d) && devant(&b, Pane::ToolOptions));
    assert_eq!(b.app.e3_panneau.avant, Some(Pane::Color));
    fermer_la_vue(&mut b);
    assert!(devant(&b, Pane::Color) && b.app.pane_shown(Pane::Espace3d), "la couleur revient, le panneau reste dans la disposition");
    // Fermé pendant que la vue est ouverte : il reste fermé tant qu'elle l'est.
    ouvrir_la_vue(&mut b);
    b.app.toggle_pane(Pane::Espace3d);
    b.chauffer(3);
    assert!(!b.app.pane_shown(Pane::Espace3d) && b.app.espace3d_vue_ouverte());
    fermer_la_vue(&mut b);
    // Ni couleur ni brosses dédiées dans la disposition : il se pose juste sous les réglages de l'outil, qui restent devant.
    for p in [Pane::Color, Pane::Personnages] {
        b.app.toggle_pane(p);
    }
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::Color) && !b.app.pane_shown(Pane::Personnages) && !b.app.pane_shown(Pane::Espace3d));
    ouvrir_la_vue(&mut b);
    let s = format!("{:?}", b.app.dock_layout().principal);
    assert!(s.contains("haut: Tabs { panneaux: [ToolOptions, Sheet], actif: 0 }, bas: Tabs { panneaux: [Espace3d], actif: 0 }"), "{s}");
    assert!(devant(&b, Pane::ToolOptions) && devant(&b, Pane::Espace3d));
    assert_eq!(b.app.e3_panneau.avant, None, "seul dans son groupe : aucun onglet à rendre");
    fermer_la_vue(&mut b);
    assert!(b.app.pane_shown(Pane::Espace3d));
}

#[test]
fn the_retained_layout_keeps_the_tab_the_user_had_in_front_and_each_work_space_gets_the_pane_in_front() {
    let mut b = banc_d_essai(2);
    ouvrir_la_vue(&mut b);
    // (Le temps que le bord entre Réglages et le panneau finisse de glisser : la disposition n'est retenue qu'ensuite.)
    b.chauffer(30);
    assert!(devant(&b, Pane::Espace3d) && !b.app.fit_busy);
    // La disposition retenue (celle qu'on enregistre, et que l'espace retrouve) garde la couleur devant.
    let retenue = b.app.current_layout().dock_layout();
    assert_eq!(onglets_de(&retenue, Pane::Espace3d), Some((vec![Pane::Color, Pane::Personnages, Pane::Espace3d], Pane::Color)));
    let reglages = b.app.settings.layout_of(Espace::Storyboard).expect("la disposition du Storyboard est retenue").dock_layout();
    assert_eq!(onglets_de(&reglages, Pane::Espace3d).map(|g| g.1), Some(Pane::Color), "dans les réglages aussi");
    // On entre en animation, la vue toujours ouverte : dans la disposition de l'Animation aussi, le panneau passe devant.
    b.app.enter_animation(0);
    b.chauffer(3);
    assert!(b.app.espace == Espace::Animation && b.app.espace3d_vue_ouverte());
    assert!(devant(&b, Pane::Espace3d) && devant(&b, Pane::ToolOptions), "{:?}", b.app.dock_layout().principal);
    let storyboard = b.app.settings.layout_of(Espace::Storyboard).expect("retenue en quittant le Storyboard").dock_layout();
    assert_eq!(onglets_de(&storyboard, Pane::Espace3d).map(|g| g.1), Some(Pane::Color));
    // Retour au Storyboard : le panneau y est de nouveau devant ; la vue fermée, la couleur revient.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    assert!(devant(&b, Pane::Espace3d));
    fermer_la_vue(&mut b);
    assert!(devant(&b, Pane::Color));
    // « Rétablir la disposition », la vue ouverte : la disposition d'origine, le panneau devant.
    ouvrir_la_vue(&mut b);
    b.app.run_action("espace.retablir");
    b.chauffer(3);
    assert!(devant(&b, Pane::Espace3d) && b.app.e3_panneau.avant == Some(Pane::Color));
}

// ------------------------------------------------------------------------------------------------ sa place d'origine

#[test]
fn the_default_layouts_of_the_drawing_work_spaces_have_the_pane_as_a_tab_with_the_colour_under_the_settings() {
    let sous_les_reglages = "haut: Tabs { panneaux: [ToolOptions, Sheet], actif: 0 }, bas: Tabs { panneaux: [Color, Personnages, Espace3d], actif: 0 }";
    // Le Storyboard et l'Animation : les espaces où l'on dessine, avec les réglages de l'outil, la couleur et les calques.
    for e in [Espace::Storyboard, Espace::Animation] {
        let d = e.defaut().dock_layout();
        assert_eq!(d.panes().iter().filter(|p| **p == Pane::Espace3d).count(), 1, "{e:?}");
        let s = format!("{:?}", d.principal);
        assert!(s.contains(sous_les_reglages), "{e:?} : {s}");
        // Réglages, Calques et la couleur y sont devant : on voit les réglages de l'outil et les calques avec lui.
        assert_eq!(onglets_de(&d, Pane::ToolOptions).map(|g| g.1), Some(Pane::ToolOptions));
        assert_eq!(onglets_de(&d, Pane::Layers).map(|g| g.1), Some(Pane::Layers));
        // L'aller-retour par egui_dock ne le perd pas.
        assert!(dock::from_state(&dock::to_state(&d), &|_| None).close_to(&d), "{e:?}");
    }
    // Les dispositions « Standard », « Dessin » et « Timeline » aussi : elles ont les réglages de l'outil et la couleur.
    for (action, _, layout) in workspace::builtin() {
        assert_eq!(onglets_de(&layout.dock_layout(), Pane::Espace3d), Some((vec![Pane::Color, Pane::Personnages, Pane::Espace3d], Pane::Color)), "{action}");
    }
    // Les Vignettes (la planche ne montre pas la vue) et le Montage (ni réglages de l'outil ni couleur) : pas de panneau
    // d'office. (Si la vue est ouverte dans le lecteur du Montage, le panneau y arrive avec la fiche du plan : plus bas.)
    for e in [Espace::Vignettes, Espace::Montage] {
        assert!(!e.defaut().dock_layout().contains(Pane::Espace3d), "{e:?}");
    }
}

#[test]
fn the_pane_reopens_with_the_colour_wherever_it_is_and_else_right_under_the_tool_settings() {
    // Fermé puis rouvert : à son rang, après la couleur et les brosses dédiées.
    let mut d = disposition_d_avant();
    d.open(Pane::Espace3d);
    assert_eq!(d, DockLayout::from_legacy(&Layout::default()), "il revient exactement à sa place d'origine");
    // Les brosses dédiées fermées : avec la couleur ; rouvertes, elles reprennent leur rang entre les deux.
    let mut d = disposition_d_avant();
    d.close(Pane::Personnages);
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d), Some((vec![Pane::Color, Pane::Espace3d], Pane::Color)));
    d.open(Pane::Personnages);
    assert_eq!(onglets_de(&d, Pane::Espace3d), Some((vec![Pane::Color, Pane::Personnages, Pane::Espace3d], Pane::Color)));
    // La couleur dans un panneau flottant : il la rejoint là où elle est.
    let mut d = disposition_d_avant();
    d.close(Pane::Color);
    d.close(Pane::Personnages);
    d.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Color]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 320.0 });
    d.open(Pane::Espace3d);
    assert_eq!(d.fenetres[0].panneaux, tabs(&[Pane::Color, Pane::Espace3d]));
    // La couleur dans un panneau flottant, les brosses dédiées restées sous les réglages : c'est la couleur qu'il rejoint.
    let mut d = disposition_d_avant();
    d.close(Pane::Color);
    d.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Color]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 320.0 });
    d.open(Pane::Espace3d);
    assert_eq!(d.fenetres[0].panneaux, tabs(&[Pane::Color, Pane::Espace3d]));
    assert_eq!(onglets_de(&d, Pane::Personnages).map(|g| g.0), Some(vec![Pane::Personnages]));
    // Sans la couleur, avec les brosses dédiées : il les rejoint.
    let mut d = disposition_d_avant();
    d.close(Pane::Color);
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d), Some((vec![Pane::Personnages, Pane::Espace3d], Pane::Personnages)));
    // Ni couleur ni brosses dédiées : juste sous les réglages de l'outil (là où la couleur est d'ordinaire).
    let mut d = disposition_d_avant();
    d.close(Pane::Color);
    d.close(Pane::Personnages);
    d.open(Pane::Espace3d);
    let s = format!("{:?}", d.principal);
    assert!(s.contains("haut: Tabs { panneaux: [ToolOptions, Sheet], actif: 0 }, bas: Tabs { panneaux: [Espace3d], actif: 0 }"), "{s}");
    // La couleur rouverte ensuite le rejoint, devant lui dans l'ordre des onglets.
    d.open(Pane::Color);
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Color, Pane::Espace3d]));
    // Les Vignettes (ouvert à la main) : avec la couleur, avant le scénario.
    let mut d = DockLayout::pour_les_vignettes();
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d), Some((vec![Pane::Color, Pane::Espace3d, Pane::Scenario], Pane::Color)));
    // Le Montage (ni réglages ni couleur) : avec la fiche du plan.
    let mut d = DockLayout::pour_le_montage(440.0);
    d.open(Pane::Espace3d);
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Sheet, Pane::Espace3d]));
    // Le groupe « Colonne des réglages et fiche » le ferme et le rouvre avec les autres.
    let mut d = DockLayout::from_legacy(&Layout::default());
    d.toggle_group(Group::Left);
    assert!(!d.contains(Pane::Espace3d) && !d.contains(Pane::Color));
    d.toggle_group(Group::Left);
    assert_eq!(onglets_de(&d, Pane::Espace3d).map(|g| g.0), Some(vec![Pane::Color, Pane::Personnages, Pane::Espace3d]));
}

#[test]
fn the_tab_that_was_in_front_comes_back_only_if_the_pane_is_still_in_front_of_it() {
    let origine = DockLayout::from_legacy(&Layout::default());
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

// ------------------------------------------------------------------------------------------------ les dispositions d'avant

#[test]
fn a_layout_saved_before_the_pane_gets_it_once_without_being_reset() {
    // Une disposition d'avant, que l'utilisateur a faite sienne : la bande plus haute, les brosses dédiées devant, la fiche
    // dans un panneau flottant.
    let mut avant = disposition_d_avant();
    avant.close(Pane::Sheet);
    avant.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Sheet]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 200.0 });
    let DockNode::Column { part, .. } = &mut avant.principal else { panic!("la bande est en bas de la disposition standard") };
    *part = 0.52;
    assert!(mettre_devant(&mut avant.principal, Pane::Personnages));
    let de_la_planche = Layout { vignettes: true, panneaux: Some(DockLayout::pour_les_vignettes()), ..Layout::default() };
    let mut s = Settings::default();
    s.layout = Some(Layout { panneaux: Some(avant.clone()), ..Layout::default() });
    s.animation_layout = Some(Layout { panneaux: Some(avant.clone()), ..Espace::Animation.defaut() });
    s.vignettes_layout = Some(de_la_planche.clone());
    s.montage_layout = Some(Layout { montage_height: 500.0, panneaux: Some(DockLayout::pour_le_montage(500.0)), ..Espace::Montage.defaut() });
    s.workspaces = vec![Workspace { name: "Le mien".into(), layout: Layout { panneaux: Some(avant.clone()), ..Layout::default() } }, Workspace { name: "Planche".into(), layout: de_la_planche }];
    // Le fichier des réglages, tel qu'une version d'avant l'a écrit : il ne connaît pas le panneau.
    let texte = toml::to_string(&s).expect("écrit");
    assert!(!texte.contains("\"espace3d\"") && !texte.contains("panneau_espace3d_pose"), "{texte}");
    let lu = Settings::apres_lecture(toml::from_str(&texte).expect("lu"));
    assert!(lu.panneau_espace3d_pose);
    // Le Storyboard, l'Animation et l'espace enregistré : le panneau est un onglet de plus avec la couleur, les brosses
    // dédiées sont restées devant, et rien d'autre n'a bougé (le panneau ôté, on retrouve la disposition d'avant).
    let recues = [lu.layout.as_ref().expect("storyboard"), lu.animation_layout.as_ref().expect("animation"), &lu.workspaces[0].layout];
    for (k, l) in recues.into_iter().enumerate() {
        let d = l.dock_layout();
        assert_eq!(onglets_de(&d, Pane::Espace3d), Some((vec![Pane::Color, Pane::Personnages, Pane::Espace3d], Pane::Personnages)), "disposition {k}");
        let mut sans = d.clone();
        sans.close(Pane::Espace3d);
        assert!(sans.close_to(&avant), "disposition {k} : le reste n'a pas bougé\n{sans:#?}\n{avant:#?}");
        assert_eq!(d.fenetres.len(), 1, "le panneau flottant de la fiche est resté");
    }
    assert_eq!(lu.animation_layout.as_ref().unwrap().montage_height, Espace::Animation.defaut().montage_height, "les autres réglages de la disposition restent");
    // Les Vignettes, le Montage et l'espace enregistré depuis la planche ne le reçoivent pas.
    for (nom, l) in [("vignettes", lu.vignettes_layout.as_ref().unwrap()), ("montage", lu.montage_layout.as_ref().unwrap()), ("planche", &lu.workspaces[1].layout)] {
        assert!(!l.dock_layout().contains(Pane::Espace3d), "{nom}");
    }
    assert!(lu.montage_layout.as_ref().unwrap().dock_layout().close_to(&DockLayout::pour_le_montage(500.0)));
    // Réécrit puis relu : le panneau est dans le fichier, et la pose ne se refait pas. Fermé à la main ensuite, il reste fermé.
    let mut ferme = lu;
    let mut d = ferme.layout.as_ref().unwrap().dock_layout();
    d.close(Pane::Espace3d);
    ferme.layout.as_mut().unwrap().panneaux = Some(d);
    let texte = toml::to_string(&ferme).expect("écrit");
    assert!(texte.contains("\"espace3d\"") && texte.contains("panneau_espace3d_pose = true"), "{texte}");
    let relu = Settings::apres_lecture(toml::from_str(&texte).expect("relu"));
    assert!(!relu.layout.as_ref().unwrap().dock_layout().contains(Pane::Espace3d), "fermé à la main : il reste fermé");
    assert!(relu.animation_layout.as_ref().unwrap().dock_layout().contains(Pane::Espace3d));
    // Une disposition sans couleur ni brosses dédiées : il se pose sous les réglages de l'outil ; sans réglages ni couleur : rien.
    let mut sans_couleur = disposition_d_avant();
    sans_couleur.close(Pane::Color);
    sans_couleur.close(Pane::Personnages);
    let mut nue = sans_couleur.clone();
    nue.close(Pane::ToolOptions);
    let mut s = Settings::default();
    s.layout = Some(Layout { panneaux: Some(sans_couleur), ..Layout::default() });
    s.animation_layout = Some(Layout { panneaux: Some(nue.clone()), ..Layout::default() });
    let lu = Settings::apres_lecture(toml::from_str(&toml::to_string(&s).expect("écrit")).expect("lu"));
    let d = format!("{:?}", lu.layout.as_ref().unwrap().dock_layout().principal);
    assert!(d.contains("haut: Tabs { panneaux: [ToolOptions, Sheet], actif: 0 }, bas: Tabs { panneaux: [Espace3d], actif: 0 }"), "{d}");
    assert!(lu.animation_layout.as_ref().unwrap().dock_layout().close_to(&nue), "ni réglages ni couleur : la disposition reste comme elle est");
    // Des réglages sans disposition retenue (un poste neuf) : rien à poser, l'origine du jour a le panneau.
    let neuf = Settings::apres_lecture(Settings::default());
    assert!(neuf.panneau_espace3d_pose && neuf.layout.is_none() && neuf.animation_layout.is_none());
}

// ------------------------------------------------------------------------------------------------ sa commande, son icône, les messages

#[test]
fn the_pane_has_its_name_its_cube_its_command_its_icon_in_the_bar_and_its_line_in_the_display_menu() {
    assert_eq!((Pane::Espace3d.name(), Pane::Espace3d.tab_name()), ("Espace 3D", "Espace 3D"));
    assert_eq!(Pane::Espace3d.icon(), egui_phosphor::regular::CUBE, "le cube du bouton de la vue");
    assert_eq!(Pane::Espace3d.group(), Group::Left);
    assert!(!Pane::Espace3d.on_demand() && !Pane::Espace3d.animation_only());
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
    // La commande ouvre, met devant ou ferme le panneau, comme le clic sur son icône.
    let mut b = banc_d_essai(1);
    assert!(b.app.pane_shown(Pane::Espace3d) && !devant(&b, Pane::Espace3d));
    b.app.run_action(action);
    b.chauffer(1);
    assert!(devant(&b, Pane::Espace3d), "derrière la couleur : la commande le met devant");
    b.app.run_action(action);
    b.chauffer(1);
    assert!(!b.app.pane_shown(Pane::Espace3d), "devant : la commande le ferme");
    b.app.run_action(action);
    b.chauffer(1);
    assert!(b.app.pane_shown(Pane::Espace3d) && devant(&b, Pane::Espace3d), "fermé : elle le rouvre, avec la couleur, devant");
    assert_eq!(groupe(&b, Pane::Espace3d), vec![Pane::Color, Pane::Personnages, Pane::Espace3d]);
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
