//! Essais de l'onglet 3D (partie 198, `onglet3d.rs`, `onglet3dui.rs`, `docs/INTERFACE.md`) : l'onglet et sa commande, sa pile et sa
//! bande d'origine, le plan et le choix gardés en y passant, les points de vue et 1, 2 ou 4 vues, la boussole, le sommaire « Scène
//! 3D » face à la séquence, le choix dans le sommaire et dans la vue, l'onglet de Propriétés qui suit, la timeline de la séquence,
//! la banque 3D, le repos, la bande sur toute la largeur. Dans la vraie fenêtre sans écran (`Banc`).

use std::sync::Arc;

use eframe::egui::{epaint::Shape, pos2, Event, Modifiers, PointerButton, Rect};

use crate::bulles::vus;
use crate::decor3d::{Decor3d, Modele3d, Objet3d, Place3d};
use crate::dock::{DockNode, Pane};
use crate::document::{Document, Layer, LayerKind};
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dvues::{Disposition, PointDeVue};
use crate::onglet3d::{self, Groupe3d, Ligne3d};
use crate::onglet3dui::{LIGNES_S3D, TIMELINE_SEQ};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

/// Un film : une séquence de trois plans (deux calques chacun), puis une autre d'un plan ; la première a un décor de deux carrés.
fn film() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    doc.add_sequence_after(2);
    for (k, p) in doc.panels.iter_mut().enumerate() {
        p.layers = vec![Layer::new(&format!("Fond {k}"), LayerKind::Vector), Layer::new(&format!("Perso {k}"), LayerKind::Vector)];
        p.set_length(24);
    }
    let seq = doc.sequence_of(0);
    let octets = crate::decor3d_tests::carre(true, None, |_| {});
    let (scene, _) = crate::decor3dgltf::lire(&octets, None).unwrap();
    let modele = Arc::new(Modele3d::new(Arc::new(scene), Arc::new(octets)));
    let mut d = Decor3d::new(crate::decor3drendu::Aspect { ombres: false, ..Default::default() });
    for (nom, x) in [("Maison", -1.0), ("Arbre", 1.5)] {
        let id = doc.alloc();
        d.objets.push(Objet3d { id, nom: nom.into(), source: "carre.glb".into(), allege: Vec::new(), place: Place3d { position: [x, 0.0, -2.0], rotation_y: 0.0, echelle: 1.0 }, origine: [0.0; 3], aimante: false, modele: modele.clone(), couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() });
    }
    doc.decors3d.par_sequence.insert(seq, d);
    doc
}

fn banc_3d(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    b.app.set_espace(Espace::TroisD);
    // (Assez haut pour tout le sommaire du petit film ; la colonne défile au besoin, partie 196.5.)
    if let Some(k) = b.app.colonne.rang_de(Pane::Scene3d) {
        b.app.colonne.panneaux[k].hauteur = Some(330.0);
    }
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

/// La place à l'écran du panneau d'egui_dock qui montre `pane`.
fn place(b: &Banc, pane: Pane) -> Rect {
    let state = b.app.dock.as_ref().expect("la disposition à l'écran");
    let (s, n, _) = state.find_tab(&pane).unwrap_or_else(|| panic!("{pane:?} n'est pas dans egui_dock"));
    match &state[s][n] {
        egui_dock::Node::Leaf { rect, .. } => *rect,
        _ => unreachable!(),
    }
}

fn lignes() -> Vec<(Ligne3d, Rect)> {
    LIGNES_S3D.with(|l| l.borrow().clone())
}

fn ligne(l: Ligne3d) -> Rect {
    lignes().into_iter().find(|x| x.0 == l).map(|x| x.1).unwrap_or_else(|| panic!("{l:?} : {:?}", lignes()))
}

/// Un clic sur la ligne, sur son nom (à gauche de l'œil et du cadenas).
fn clic_ligne(b: &mut Banc, l: Ligne3d) {
    let r = ligne(l);
    clic(b, pos2(r.left() + 60.0, r.center().y), Modifiers::NONE);
}

/// Les textes peints, et où.
fn textes(b: &Banc) -> Vec<(String, Rect)> {
    fn chercher(s: &Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out
}

/// Laisse passer `s` secondes, image par image.
fn attendre(b: &mut Banc, s: f64) {
    let n = (s * 60.0).ceil() as usize;
    for _ in 0..n {
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

// ------------------------------------------------------------------------------------------------ l'onglet

#[test]
fn the_3d_tab_sits_between_storyboard_and_montage_with_its_action() {
    let v = Espace::VISIBLES;
    let k = v.iter().position(|e| *e == Espace::TroisD).expect("un onglet");
    assert_eq!((v[k - 1], v[k + 1]), (Espace::Storyboard, Espace::Montage));
    assert_eq!(Espace::TroisD.action(), "espace.3d");
    assert_eq!(Espace::de_action("espace.3d"), Some(Espace::TroisD));
    assert!(crate::keymap::action_info("espace.3d").is_some_and(|a| a.implemented), "dans le fichier de raccourcis et la palette");
    assert_eq!(serde_json::to_string(&Espace::TroisD).unwrap(), "\"3d\"");
    // À l'écran : l'onglet « 3D » en haut, entre « Storyboard » et « Montage » ; la commande y mène.
    let mut b = banc_taille(film(), false, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    let (sb, trois, mo) = (ou("Storyboard"), ou("3D"), ou("Montage"));
    assert!(sb.right() <= trois.left() + 0.5 && trois.right() <= mo.left() + 0.5, "{sb:?} {trois:?} {mo:?}");
    b.app.run_action("espace.3d");
    b.chauffer(3);
    assert_eq!(b.app.espace, Espace::TroisD);
    assert!(b.app.espace3d.on && b.app.espace3d_vue_ouverte(), "la vue Espace 3D, en grand");
    // Sa disposition se retient à part.
    let ctx = b.app.ctx.clone();
    b.app.remember_layout(&ctx);
    assert!(b.app.settings.layout_of(Espace::TroisD).is_some());
}

#[test]
fn its_default_stack_is_scene_3d_over_properties_and_the_band_spans_the_whole_width() {
    let d = Espace::TroisD.defaut().dock_layout();
    let pile: Vec<Vec<Pane>> = d.colonne.panneaux.iter().map(|p| p.panneaux.clone()).collect();
    assert_eq!(pile, vec![vec![Pane::Scene3d], vec![Pane::Objet3d, Pane::Camera3d, Pane::Lumieres3d, Pane::Monde3d, Pane::Decor3d]]);
    assert!(d.colonne.panneaux[1].est_proprietes());
    match &d.principal {
        DockNode::Column { bas, .. } => assert!(matches!(&**bas, DockNode::Tabs { panneaux, .. } if *panneaux == vec![Pane::TimelineSequence, Pane::Banque3d]), "{:?}", d.principal),
        n => panic!("la bande en bas : {n:?}"),
    }
    // « + Panneau » dans l'onglet 3D : ses panneaux (et l'Historique), pas ceux du dessin ; ailleurs, pas les siens.
    let p = crate::colonne::propositions(Espace::TroisD, false, false, true, &|_| false);
    assert!(p.iter().any(|x| x.pane == Some(Pane::Scene3d)) && p.iter().any(|x| x.pane == Some(Pane::History)));
    assert!(!p.iter().any(|x| x.pane == Some(Pane::Color) || x.pane == Some(Pane::Scene)));
    let p = crate::colonne::propositions(Espace::Storyboard, false, false, true, &|_| false);
    assert!(!p.iter().any(|x| x.pane.is_some_and(Pane::de_la_3d)));
    // À l'écran : la bande du bas sous la vue et la colonne, sur toute la largeur.
    let b = banc_3d(film());
    let bande = place(&b, Pane::TimelineSequence);
    let vue = place(&b, Pane::Canvas);
    let colonne = place(&b, Pane::Colonne);
    assert!(bande.top() >= vue.bottom() - 1.0 && bande.top() >= colonne.bottom() - 1.0, "dessous : {bande:?} / {vue:?} {colonne:?}");
    assert!(bande.left() <= vue.left() + 1.0 && bande.right() >= colonne.right() - 1.0, "toute la largeur : {bande:?} / {vue:?} {colonne:?}");
    assert!(b.app.colonne.contient(Pane::Scene3d) && b.app.colonne.contient(Pane::Decor3d));
}

#[test]
fn switching_to_the_tab_and_back_keeps_the_plan_the_layer_and_the_selection() {
    let mut b = banc_taille(film(), false, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    b.app.select_panel(1);
    b.app.select_layer(1);
    b.chauffer(3);
    let calque = b.app.layer().id;
    assert!(!b.app.espace3d.on);
    b.app.set_espace(Espace::TroisD);
    b.chauffer(4);
    assert_eq!((b.app.cur_panel, b.app.layer().id), (1, calque), "le même plan, le même calque");
    let carte = crate::espace3d::card_selection(calque);
    b.app.espace3d.sel = vec![carte];
    b.chauffer(2);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    assert_eq!((b.app.cur_panel, b.app.layer().id), (1, calque));
    assert!(!b.app.espace3d.on, "la vue Espace 3D, fermée avant, se referme en sortant");
    b.app.set_espace(Espace::TroisD);
    b.chauffer(3);
    assert_eq!(b.app.espace3d.sel, vec![carte], "le choix de la vue est gardé");
    // Les outils : ceux de la vue seulement, pas ceux du dessin.
    for nom in ["Choisir", "Déplacer", "Tourner", "Mettre à l'échelle", crate::poignees3d::ORIGINE_NOM, "Aimanter au sol"] {
        ou(nom);
    }
    assert!(vus::ou("Dessiner sur la carte").is_empty(), "{:?}", vus::noms());
    assert!(vus::ou(crate::app::Tool::Brush.label()).is_empty(), "pas le pinceau");
}

// ------------------------------------------------------------------------------------------------ les vues

#[test]
fn the_view_bar_sets_the_point_of_view_and_one_two_or_four_views() {
    let mut b = banc_3d(film());
    assert_eq!((b.app.o3_nombre_de_vues(), b.app.o3_point_actif()), (1, PointDeVue::Perspective));
    for (_, nom) in onglet3d::POINTS {
        ou(&format!("Vue {nom}"));
    }
    // Dessus, seule : la vue orthographique, agrandie des quatre.
    clic(&mut b, ou("Vue Dessus").center(), Modifiers::NONE);
    assert_eq!((b.app.o3_nombre_de_vues(), b.app.o3_point_actif()), (1, PointDeVue::Dessus));
    assert!(b.app.e3_vues_multiples() && b.app.espace3d.vues.agrandie.is_some());
    // La caméra du plan : la vue seule d'avant, par la caméra.
    clic(&mut b, ou("Vue Caméra du plan").center(), Modifiers::NONE);
    assert_eq!((b.app.o3_nombre_de_vues(), b.app.o3_point_actif()), (1, PointDeVue::Camera));
    assert!(!b.app.e3_vues_multiples() && b.app.espace3d.through_camera);
    clic(&mut b, ou("Vue Perspective").center(), Modifiers::NONE);
    assert_eq!(b.app.o3_point_actif(), PointDeVue::Perspective);
    assert!(!b.app.espace3d.through_camera);
    // Deux vues : la perspective et la caméra du plan.
    clic(&mut b, ou("2 vues").center(), Modifiers::NONE);
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::CoteACote);
    assert_eq!((b.app.espace3d.vues.cadres[4].point, b.app.espace3d.vues.cadres[5].point), (PointDeVue::Perspective, PointDeVue::Camera));
    // Quatre : dessus, perspective, face, la caméra du plan.
    clic(&mut b, ou("4 vues").center(), Modifiers::NONE);
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Quatre);
    assert_eq!(b.app.espace3d.vues.cadres[..4].iter().map(|c| c.point).collect::<Vec<_>>(), onglet3d::QUATRE.to_vec());
    assert_eq!(b.app.o3_nombre_de_vues(), 4);
    // Le point de vue s'applique à la vue active ; « 1 » revient à une vue.
    clic(&mut b, ou("Vue Face").center(), Modifiers::NONE);
    let active = b.app.espace3d.vues.active;
    assert_eq!(b.app.espace3d.vues.cadres[active].point, PointDeVue::Face);
    clic(&mut b, ou("1 vue").center(), Modifiers::NONE);
    assert_eq!(b.app.o3_nombre_de_vues(), 1);
    // Ce que la vue montre : la grille, les cartes, les caméras, le soleil, les ombres ; Recadrer.
    let grille = b.app.espace3d.show_grid;
    clic(&mut b, ou("Grille du sol").center(), Modifiers::NONE);
    assert_ne!(b.app.espace3d.show_grid, grille);
    clic(&mut b, ou("Caméras").center(), Modifiers::NONE);
    assert!(!b.app.o3.montrer_cameras);
    let ombres = b.app.doc.decor_du_plan(0).unwrap().aspect.ombres;
    clic(&mut b, ou("Ombres").center(), Modifiers::NONE);
    assert_ne!(b.app.doc.decor_du_plan(0).unwrap().aspect.ombres, ombres, "les ombres du décor");
    for nom in ["Recadrer", "Lumières", "Cartes"] {
        ou(nom);
    }
}

#[test]
fn a_click_on_a_compass_axis_turns_the_view_along_it_with_a_short_animation() {
    let mut b = banc_3d(film());
    b.app.o3.boussoles.clear();
    b.chauffer(2);
    let bouts = b.app.o3.boussoles.iter().rev().find(|x| x.0.is_none()).expect("la boussole de la vue seule").1.clone();
    assert_eq!(bouts.len(), 6);
    let vue = place(&b, Pane::Canvas);
    let x_plus = bouts.iter().find(|x| x.0 == 0 && x.1).unwrap().2;
    assert!(x_plus.x > vue.center().x && x_plus.y < vue.center().y, "en haut à droite : {x_plus:?} / {vue:?}");
    clic(&mut b, x_plus, Modifiers::NONE);
    assert!(b.app.o3.vise.is_some() || (b.app.espace3d.cam.unwrap().azimuth.rem_euclid(360.0) - 90.0).abs() < 0.5, "la vue tourne… {}", b.app.status);
    attendre(&mut b, 0.4);
    assert!(b.app.o3.vise.is_none(), "… et s'arrête");
    let o = b.app.espace3d.cam.unwrap();
    assert!((o.azimuth.rem_euclid(360.0) - 90.0).abs() < 0.5 && o.elevation.abs() < 0.5, "le long de X : {o:?}");
    // +Y : de dessus.
    b.app.o3.boussoles.clear();
    b.chauffer(2);
    let y = b.app.o3.boussoles.iter().rev().find(|x| x.0.is_none()).unwrap().1.iter().find(|x| x.0 == 1 && x.1).unwrap().2;
    clic(&mut b, y, Modifiers::NONE);
    attendre(&mut b, 0.4);
    assert!((b.app.espace3d.cam.unwrap().elevation - crate::espace3dmath::MAX_ELEVATION).abs() < 0.5);
    // Dans la perspective des quatre vues (seule vue en perspective : seule à avoir la boussole).
    b.app.o3_poser_vues(4);
    b.app.o3.boussoles.clear();
    b.chauffer(3);
    let (i, bouts) = b.app.o3.boussoles.iter().rev().find(|x| x.0.is_some()).map(|x| (x.0.unwrap(), x.1.clone())).expect("la boussole de la perspective");
    assert_eq!(b.app.espace3d.vues.cadres[i].point, PointDeVue::Perspective);
    assert!(b.app.o3.boussoles.iter().filter(|x| x.0.is_some()).all(|x| x.0 == Some(i)), "seulement dans la vue en perspective");
    // (Elle regarde la face, depuis +Z : −Z est caché derrière +Z, au centre ; −X, à gauche, se prend.)
    let x_moins = bouts.iter().find(|x| x.0 == 0 && !x.1).unwrap().2;
    clic(&mut b, x_moins, Modifiers::NONE);
    attendre(&mut b, 0.4);
    let o = b.app.espace3d.vues.cadres[i].orbite;
    assert!((o.azimuth.rem_euclid(360.0) - 270.0).abs() < 0.5 && o.elevation.abs() < 0.5, "depuis −X : {o:?}");
    // Les calculs : le regard d'un axe, le plus court chemin.
    assert_eq!(onglet3d::regard_d_axe(2, true, 10.0), (0.0, 0.0));
    assert_eq!(onglet3d::azimut_proche(350.0, 10.0), 370.0);
}

// ------------------------------------------------------------------------------------------------ Scène 3D

#[test]
fn the_outliner_lists_the_sequence_objects_cameras_lights_and_the_cards_of_the_plan() {
    let doc = film();
    let ids: Vec<u64> = doc.decor_du_plan(0).unwrap().objets.iter().map(|o| o.id).collect();
    let s = onglet3d::sommaire(&doc, 1, "", &[]);
    let attendu = vec![
        Ligne3d::Groupe(Groupe3d::Decor, 2),
        Ligne3d::Objet(ids[0]),
        Ligne3d::Objet(ids[1]),
        Ligne3d::Groupe(Groupe3d::Cameras, 3),
        Ligne3d::Camera(0),
        Ligne3d::Camera(1),
        Ligne3d::Camera(2),
        Ligne3d::Groupe(Groupe3d::Lumieres, 2),
        Ligne3d::Soleil,
        Ligne3d::Ciel,
        Ligne3d::Groupe(Groupe3d::Cartes, 2),
        Ligne3d::Carte(1, doc.panels[1].layers[1].id),
        Ligne3d::Carte(0, doc.panels[1].layers[0].id),
    ];
    assert_eq!(s, attendu, "les caméras des trois plans de la séquence, les cartes du plan 2, du haut de la pile");
    // L'autre séquence : son plan seul, pas de décor.
    let s = onglet3d::sommaire(&doc, 3, "", &[]);
    assert!(s.contains(&Ligne3d::Groupe(Groupe3d::Decor, 0)) && s.contains(&Ligne3d::Camera(3)) && !s.contains(&Ligne3d::Camera(0)));
    // La recherche ; un groupe replié.
    let s = onglet3d::sommaire(&doc, 1, "arbre", &[]);
    assert_eq!(s, vec![Ligne3d::Groupe(Groupe3d::Decor, 1), Ligne3d::Objet(ids[1])]);
    let s = onglet3d::sommaire(&doc, 1, "", &[Groupe3d::Cameras]);
    assert!(s.contains(&Ligne3d::Groupe(Groupe3d::Cameras, 3)) && !s.contains(&Ligne3d::Camera(0)));
    // À l'écran : les mêmes lignes ; le plan courant marqué ; replier un groupe d'un clic.
    let mut b = banc_3d(film());
    let vues: Vec<Ligne3d> = lignes().into_iter().map(|x| x.0).collect();
    assert_eq!(vues, onglet3d::sommaire(&b.app.doc, 0, "", &[]));
    assert!(textes(&b).iter().any(|(t, _)| t.ends_with("· plan courant")));
    clic(&mut b, ligne(Ligne3d::Groupe(Groupe3d::Cameras, 3)).center(), Modifiers::NONE);
    assert!(b.app.o3.replies.contains(&Groupe3d::Cameras) && !lignes().iter().any(|x| x.0 == Ligne3d::Camera(0)));
    // L'œil d'un objet le cache de la vue et de l'image (pour la séance) ; son cadenas l'empêche d'être choisi.
    let r = ligne(Ligne3d::Objet(ids[0]));
    let oeil = vus::ou("Afficher / cacher").into_iter().find(|x| r.contains(x.center())).expect("l'œil de l'objet");
    clic(&mut b, oeil.center(), Modifiers::NONE);
    assert!(b.app.doc.decor_du_plan(0).unwrap().objet(ids[0]).unwrap().cache);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().vus().count(), 1, "la vue et l'image ne le dessinent plus");
    let r = ligne(Ligne3d::Objet(ids[1]));
    let cadenas = vus::ou("Verrouiller / déverrouiller").into_iter().find(|x| r.contains(x.center())).expect("le cadenas de l'objet");
    clic(&mut b, cadenas.center(), Modifiers::NONE);
    clic_ligne(&mut b, Ligne3d::Objet(ids[1]));
    assert!(b.app.e3_objets_choisis().is_empty(), "verrouillé, il ne se choisit pas");
}

#[test]
fn choosing_in_the_outliner_chooses_in_the_view_and_the_properties_tab_follows() {
    let mut b = banc_3d(film());
    let ids: Vec<u64> = b.app.doc.decor_du_plan(0).unwrap().objets.iter().map(|o| o.id).collect();
    // Un objet : choisi dans la vue ; Propriétés passe à l'onglet Objet, qui montre ses champs.
    clic_ligne(&mut b, Ligne3d::Objet(ids[0]));
    assert_eq!(b.app.e3_objets_choisis(), vec![ids[0]]);
    b.chauffer(2);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Objet3d));
    assert!(textes(&b).iter().any(|(t, _)| t == "Position (m)"), "les champs de l'objet choisi");
    // La caméra d'un autre plan : on va à ce plan, sa caméra est choisie ; Propriétés passe à Caméra.
    clic_ligne(&mut b, Ligne3d::Camera(2));
    assert_eq!(b.app.cur_panel, 2);
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID]);
    b.chauffer(2);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Camera3d));
    // Une carte : son calque devient le calque courant.
    let carte = b.app.layers()[0].id;
    clic_ligne(&mut b, Ligne3d::Carte(0, carte));
    assert_eq!((b.app.cur_layer, b.app.espace3d.sel.clone()), (0, vec![crate::espace3d::card_selection(carte)]));
    b.chauffer(2);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Objet3d));
    // Et dans l'autre sens : ce qui est choisi dans la vue, Propriétés le suit quand il change ; un onglet mis devant reste devant.
    b.app.espace3d.sel = vec![crate::poignees3d::objet_selection(ids[1])];
    b.app.colonne.montrer(Pane::Monde3d);
    b.chauffer(3);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Monde3d), "le même sujet : l'onglet qu'on a mis devant reste");
    assert!(b.app.scene3d_choisie(Ligne3d::Objet(ids[1])) && !b.app.scene3d_choisie(Ligne3d::Objet(ids[0])), "choisi dans la vue : surligné dans le sommaire");
    b.app.espace3d.sel = vec![CAMERA_ID];
    b.chauffer(3);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Camera3d));
    // Les onglets Lumières, Monde, Décor montrent les parties du réglage du décor.
    for (pane, mot) in [(Pane::Lumieres3d, "Soleil"), (Pane::Monde3d, "Brume"), (Pane::Decor3d, "Ombres")] {
        b.app.colonne.montrer(pane);
        b.chauffer(3);
        assert!(textes(&b).iter().any(|(t, _)| t == mot), "{pane:?} : « {mot} »");
    }
}

// ------------------------------------------------------------------------------------------------ la bande du bas

#[test]
fn the_sequence_timeline_changes_plan_and_image() {
    let mut b = banc_3d(film());
    let (piste, blocs) = TIMELINE_SEQ.with(|t| t.borrow().clone()).expect("la timeline de la séquence");
    assert_eq!(blocs.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2], "les plans de la séquence seulement");
    // Un clic sur le troisième plan y mène.
    clic(&mut b, blocs[2].1.center(), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 2);
    // Glisser sur la règle : la tête de lecture suit, de plan en plan.
    let y = piste.top() + 6.0;
    let depart = pos2(piste.left() + 2.0, y);
    b.image(vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: depart, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(1);
    assert_eq!(b.app.cur_panel, 0);
    let arrivee = pos2(piste.left() + piste.width() * 0.5 + 1.0, y);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(depart + (arrivee - depart) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerButton { pos: arrivee, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!((b.app.cur_panel, b.app.cur_frame), (1, 12), "le milieu de la séquence : l'image 12 du deuxième plan");
    // La rangée des clés de caméra, et « Objet choisi » (les clés d'objet : étape 5).
    let t = textes(&b);
    assert!(t.iter().any(|(x, _)| x == "Caméra") && t.iter().any(|(x, _)| x == "Objet choisi"));
}

#[test]
fn a_click_in_the_3d_bank_adds_the_model_through_the_budget_path() {
    let mut b = crate::decor3d_tests::banc_avec_budget(crate::decor3d_tests::budget_large());
    b.app.set_espace(Espace::TroisD);
    b.garder_formes = true;
    b.app.show_pane(Pane::Banque3d);
    b.chauffer(6);
    let bande = place(&b, Pane::Banque3d);
    let noms: Vec<String> = crate::banque3d::modeles().iter().map(|x| x.nom_lisible()).collect();
    let (texte, nom) = textes(&b).into_iter().find(|(t, r)| noms.contains(t) && bande.contains(r.center())).expect("un modèle dans la banque, en bas");
    let m = &crate::banque3d::modeles()[noms.iter().position(|n| *n == texte).unwrap()];
    let etapes = b.app.history.undo_len();
    clic(&mut b, nom.center() - eframe::egui::vec2(0.0, 30.0), Modifiers::NONE);
    crate::decor3d_tests::attendre(&mut b);
    let d = b.app.doc.decor_du_plan(b.app.cur_panel).expect("posé dans le décor de la séquence");
    assert_eq!(d.objets[0].nom, m.nom_lisible());
    assert!(d.objets[0].source.starts_with("Banque 3D"));
    assert_eq!(b.app.history.undo_len(), etapes + 1, "le chemin d'un import : une étape");
    // Au-delà du budget : la fenêtre « Ce décor est lourd », comme un fichier importé.
    let mut b = crate::decor3d_tests::banc_avec_budget(crate::decor3dbudget::BudgetMachine { triangles: 10, ..crate::decor3d_tests::budget_large() });
    b.app.set_espace(Espace::TroisD);
    b.chauffer(3);
    b.app.decor3d_ajouter_banque(0, None);
    crate::decor3d_tests::attendre(&mut b);
    assert!(b.app.decor3d.fenetre_ouverte());
}

// ------------------------------------------------------------------------------------------------ le repos

#[test]
fn at_rest_the_3d_tab_asks_for_no_redraw() {
    crate::fxcache::monde::ecrire();
    let mut b = banc_3d(film());
    assert!(crate::perf_tests::se_reposer(&mut b, 3000) < 3000, "l'onglet 3D se repose");
    let pire = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).min().unwrap();
    assert!(pire >= std::time::Duration::from_secs(1), "au repos, l'onglet 3D demande à être repeint dans {pire:?}");
    // L'animation de la boussole demande des images, puis plus rien.
    b.app.o3_viser(None, 0, true);
    let m = b.image(Vec::new(), 1.0 / 60.0);
    assert!(m.reveil < std::time::Duration::from_millis(100), "{:?}", m.reveil);
    attendre(&mut b, 0.5);
    assert!(crate::perf_tests::se_reposer(&mut b, 3000) < 3000);
    let pire = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).min().unwrap();
    assert!(pire >= std::time::Duration::from_secs(1), "l'animation finie : {pire:?}");
}

#[test]
fn the_band_head_buttons_switch_between_the_sequence_timeline_and_the_3d_bank() {
    let mut b = banc_3d(film());
    assert_eq!(b.app.o3_bande_devant(), Some(Pane::TimelineSequence), "d'origine, la timeline de la séquence");
    let (tl, bq) = (ou("Onglet Timeline de la séquence"), ou("Onglet Banque 3D"));
    let bande = place(&b, Pane::TimelineSequence);
    assert!(bande.contains(tl.center()) && bande.contains(bq.center()) && tl.right() <= bq.left() + 0.5, "en tête de la bande, à gauche : {tl:?} {bq:?} / {bande:?}");
    let lire = ou("Lire / pause");
    assert!((lire.center().y - tl.center().y).abs() < 4.0 && lire.left() > bq.right(), "sur la ligne de la lecture et du compteur, avant eux");
    // « Banque 3D » : la banque passe devant, ses modèles sont dessinés.
    clic(&mut b, bq.center(), Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.o3_bande_devant(), Some(Pane::Banque3d));
    let noms: Vec<String> = crate::banque3d::modeles().iter().map(|m| m.nom_lisible()).collect();
    let bande = place(&b, Pane::Banque3d);
    assert!(textes(&b).iter().any(|(t, r)| noms.contains(t) && bande.contains(r.center())), "la grille des modèles");
    // « Timeline de la séquence » : elle revient.
    TIMELINE_SEQ.with(|t| *t.borrow_mut() = None);
    clic(&mut b, ou("Onglet Timeline de la séquence").center(), Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.o3_bande_devant(), Some(Pane::TimelineSequence));
    assert!(TIMELINE_SEQ.with(|t| t.borrow().is_some()), "la timeline de la séquence est peinte");
    assert!(!textes(&b).iter().any(|(t, r)| noms.contains(t) && bande.contains(r.center())), "la banque n'est plus devant");
}

// ------------------------------------------------------------------------------------------------ 198.5 : vu à l'écran

/// Les mesures d'un modèle de `triangles` triangles, sans texture.
fn mesures(triangles: usize) -> crate::decor3dgltf::Mesures {
    crate::decor3dgltf::Mesures { octets_fichier: 14 << 10, triangles, sommets: 3 * triangles, matieres: 2, ..Default::default() }
}

#[test]
fn a_tiny_model_is_never_heavy_because_of_one_timing_and_the_limit_is_strict() {
    use crate::decor3d::{verdict, Verdict};
    let budget = crate::decor3d_tests::budget_large();
    let limite = crate::decor3dbudget::BudgetMachine::ms_par_passe();
    // « Tree oak » : 196 triangles, aucune texture ; une première image à 1,6 ms (la limite), ou même un pic à 5 ms : accepté.
    for ms in [limite, limite + 0.01, 5.0] {
        assert_eq!(verdict(&mesures(196), &budget, Some(ms)), Verdict::Direct, "{ms} ms");
    }
    // Pas loin sous le budget (10 % des triangles) : le temps compte, strictement au-delà de la limite.
    let moyen = mesures((budget.triangles / 10) as usize);
    assert!(!crate::decor3d::loin_sous_le_budget(&moyen, &budget));
    assert_eq!(verdict(&moyen, &budget, Some(limite)), Verdict::Direct, "égal à la limite : accepté");
    match verdict(&moyen, &budget, Some(limite + 0.5)) {
        Verdict::Lourd { raisons, simplifier, .. } => assert!(simplifier && raisons.iter().any(|r| r.contains("ms à la carte graphique")), "{raisons:?}"),
        v => panic!("au-delà de la limite : lourd ({v:?})"),
    }
    // Vraiment lourd, en triangles : toujours vu, quel que soit le temps mesuré.
    assert!(matches!(verdict(&mesures(budget.triangles as usize + 1), &budget, None), Verdict::Lourd { .. }));
    // Des textures lourdes sur peu de triangles : le temps compte de nouveau.
    let texture = crate::decor3dgltf::Mesures { octets_textures: (budget.textures_mo << 20) / 10, textures: 1, ..mesures(196) };
    assert!(matches!(verdict(&texture, &budget, Some(limite + 0.5)), Verdict::Lourd { .. }));
}

#[test]
fn a_bank_click_right_after_the_calibration_with_a_first_frame_spike_needs_no_window() {
    // Pas encore de budget : le clic lance l'étalonnage ; la lecture l'attend, puis « mesure » une première image lente (5 ms).
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1400, 900);
    b.app.settings.decor3d = None;
    b.app.set_espace(Espace::TroisD);
    b.chauffer(3);
    let chene = crate::banque3d::modeles().iter().position(|m| m.nom == "tree_oak").expect("Tree oak dans la banque");
    crate::decor3dui::COUT_ESSAI.with(|c| c.set(Some(5.0)));
    b.app.decor3d_ajouter_banque(chene, None);
    crate::decor3dui::COUT_ESSAI.with(|c| c.set(None));
    crate::decor3d_tests::attendre(&mut b);
    assert!(b.app.decor3d_budget().is_some(), "étalonné");
    assert!(!b.app.decor3d.fenetre_ouverte(), "pas de « Ce décor est lourd » pour 196 triangles : {}", b.app.status);
    assert_eq!(b.app.doc.decor_du_plan(0).map(|d| d.objets.len()), Some(1), "posé dans le décor");
    // Au-delà du budget : la fenêtre, toujours.
    let mut b = crate::decor3d_tests::banc_avec_budget(crate::decor3dbudget::BudgetMachine { triangles: 10, ..crate::decor3d_tests::budget_large() });
    b.app.decor3d_ajouter_banque(chene, None);
    crate::decor3d_tests::attendre(&mut b);
    assert!(b.app.decor3d.fenetre_ouverte());
}

/// La part du côté de l'image que ce qui est dessiné occupe (le plus grand des deux côtés de sa boîte).
fn part_dessinee(img: &image::RgbaImage) -> f32 {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let fond = *img.get_pixel(0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 24 && (fond[3] <= 24 || (0..3).any(|k| (p[k] as i32 - fond[k] as i32).abs() > 24)) {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (x1.saturating_sub(x0) + 1).max(y1.saturating_sub(y0) + 1) as f32 / img.width().max(img.height()) as f32
}

#[test]
fn bank_thumbnails_are_framed_on_their_model() {
    // Un petit modèle au milieu d'une grande image : cadré, il la remplit, sa marge autour.
    let mut img = image::RgbaImage::from_pixel(512, 512, image::Rgba([0, 0, 0, 0]));
    for y in 240..270 {
        for x in 250..270 {
            img.put_pixel(x, y, image::Rgba([200, 120, 40, 255]));
        }
    }
    assert!(part_dessinee(&img) < 0.1);
    let c = crate::banque3dui::cadrer(&img);
    let p = part_dessinee(&c);
    assert!((p - 1.0 / (1.0 + 2.0 * crate::banque3dui::MARGE_APERCU)).abs() < 0.03, "le modèle remplit sa case, avec sa marge : {p}");
    // Les vrais aperçus de la banque : chacun cadré sur son modèle.
    for m in crate::banque3d::modeles().iter().take(8) {
        let img = image::load_from_memory_with_format(m.png().unwrap(), image::ImageFormat::Png).unwrap().to_rgba8();
        let c = crate::banque3dui::cadrer(&img);
        assert!(part_dessinee(&c) > 0.8, "« {} » : {} de sa case (avant : {})", m.nom, part_dessinee(&c), part_dessinee(&img));
    }
    // Rien de dessiné : l'image telle quelle.
    let vide = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]));
    assert_eq!(crate::banque3dui::cadrer(&vide).dimensions(), (64, 64));
}

#[test]
fn in_the_world_tab_the_colour_names_are_whole_beside_their_swatch() {
    let mut b = banc_3d(film());
    b.app.colonne.montrer(Pane::Monde3d);
    b.chauffer(4);
    let pastilles = crate::espace3ddecor::PASTILLES_ENV.with(|p| p.borrow().clone());
    assert_eq!(pastilles.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), vec!["Couleur du fond", "Couleur du maillage"]);
    let colonne = place(&b, Pane::Colonne);
    assert!((colonne.width() - crate::dock::COLONNE).abs() < 3.0, "la colonne de 290 points : {colonne:?}");
    for (nom, _, case) in pastilles {
        let texte = textes(&b).into_iter().find(|(t, _)| *t == nom).unwrap_or_else(|| panic!("« {nom} » peint")).1;
        assert!(case.left() >= texte.right() + 2.0, "« {nom} » entier, la pastille après lui : {texte:?} / {case:?}");
        assert!(case.right() <= colonne.right() + 0.5 && texte.left() >= colonne.left(), "dans la colonne : {case:?} / {colonne:?}");
    }
}
