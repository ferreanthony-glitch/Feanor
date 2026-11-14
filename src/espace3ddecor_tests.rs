//! Essais de l'environnement de l'Espace 3D et des calques 3D (`espace3ddecor.rs`, `docs/INTERFACE.md` partie 157) : l'aspect
//! d'origine pour un projet qui n'en dit rien, l'aller-retour des nouveaux champs, les calques Sol, Mur et Ciel du menu
//! « Nouveau calque », la grille de repère (jamais dans le dessin ni les exports), et le dessin qui ne change pas d'un pixel.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use eframe::egui::{pos2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::document::{Document, Layer, LayerKind};
use crate::espace3ddecor::{lignes_de_grille, Calque3d, Environnement3d, Sorte3d, FOND_D_ORIGINE, TRAITS_MAX};
use crate::espace3dview::GridLine;
use crate::exportrender::{snapshot_of, Papier, Rendu};
use crate::perf_tests::{banc_taille, Banc};
use crate::project;

/// Un projet d'essai : cadre 320 × 180 sur une feuille 640 × 360, un calque « Carte 0 » de 12 images.
fn doc_carte(kind: LayerKind) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    let mut layer = Layer::new("Carte 0", kind);
    let cel = doc.new_cel(kind);
    layer.frames = vec![Some(cel); 12];
    doc.panels[0].layers.push(layer);
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

/// Ouvert, et le plan en Espace 3D.
fn en_espace(kind: LayerKind) -> Banc {
    let mut b = ouvert(doc_carte(kind));
    b.app.run_action("espace3d.plan_en_espace");
    b.chauffer(2);
    assert!(b.app.doc.plan_en_espace(0));
    b
}

/// Un geste de souris qui passe par ces points de l'écran : appui au premier, relâché au dernier.
fn geste(b: &mut Banc, pts: &[Pos2]) {
    let bouton = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(pts[0])], 1.0 / 60.0);
    b.image(vec![bouton(pts[0], true)], 1.0 / 60.0);
    for w in pts.windows(2) {
        for k in 0..6 {
            let ev = (0..3).map(|j| Event::PointerMoved(w[0] + (w[1] - w[0]) * ((k * 3 + j + 1) as f32 / 18.0))).collect();
            b.image(ev, 1.0 / 60.0);
        }
    }
    b.image(vec![bouton(*pts.last().expect("un point au moins"), false)], 1.0 / 60.0);
    b.chauffer(2);
}

/// Un trait du point `a` au point `c` du dessin de la carte du calque courant, au pinceau, dans la vue.
fn trait_sur_la_carte(b: &mut Banc, a: Pos2, c: Pos2) {
    b.app.run_action("outil.pinceau");
    b.chauffer(2);
    assert!(b.app.e3_dessine(), "le pinceau pris dans la vue fait dessiner le pointeur");
    let vue = b.app.e3_carte_vue().expect("on dessine sur une carte");
    let (ea, ec) = (vue.vers_ecran(a).expect("le départ se voit"), vue.vers_ecran(c).expect("l'arrivée se voit"));
    assert!(b.app.canvas_rect.contains(ea) && b.app.canvas_rect.contains(ec), "le trait tient dans la vue");
    geste(b, &[ea, ec]);
}

/// Tous les pixels de la feuille du dessin du calque `li` à l'image courante, et le nombre de tuiles hors de la feuille.
fn pixels(b: &Banc, li: usize) -> (Vec<Color32>, usize) {
    let cel = b.app.layers()[li].shown_at(b.app.cur_frame).expect("un dessin");
    let raster = &b.app.doc.cels[&cel].raster;
    let (w, h) = (b.app.doc.width as i64, b.app.doc.height as i64);
    ((0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| raster.get_i(x, y)).collect(), raster.outer.len())
}

fn images_exportees(doc: &Document) -> Vec<Color32> {
    let rendu = Rendu::new(snapshot_of(doc, &[0]), Papier::Fond, true, Vec::new(), Arc::new(AtomicBool::new(false)));
    (*rendu.image(0, 0).expect("image")).clone()
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-decor3d-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    dir
}

// ------------------------------------------------------------------------------------ l'environnement

#[test]
fn l_environnement_d_origine_est_exactement_l_aspect_d_avant() {
    let env = Environnement3d::default();
    assert!(env.est_d_origine());
    assert_eq!(env.fond, FOND_D_ORIGINE);
    assert_eq!(env.couleur_du_fond(), Color32::from_gray(26), "le gris de toujours");
    assert!(env.maillage && env.pas_du_maillage().is_none(), "maillage montré, au pas automatique");
    // Les traits du maillage ne changent pas d'un bit.
    let brut = crate::espace3dview::grid_lines([1.0, 0.0, -2.0], 8.0);
    let teints = env.teinter_maillage(brut.clone());
    assert_eq!(brut.len(), teints.len());
    for (a, b) in brut.iter().zip(&teints) {
        assert_eq!((a.a, a.b, a.color), (b.a, b.b, b.color));
    }
    // Un projet neuf : maillage montré, fond d'origine, rien de retenu.
    let b = ouvert(doc_carte(LayerKind::Bitmap));
    assert!(b.app.espace3d.show_grid);
    assert!(b.app.doc.environnement_3d.is_none());
    assert_eq!(b.app.e3_environnement(), Environnement3d::default());
}

#[test]
fn le_maillage_prend_la_couleur_l_opacite_et_le_pas_de_l_environnement() {
    let mut b = ouvert(doc_carte(LayerKind::Bitmap));
    let avant = b.app.e3_quadrillage([0.0; 3], 8.0, None);
    let etapes = b.app.history.undo_len();
    b.app.modified = false;
    b.app.e3_poser_environnement(Environnement3d { fond: [200, 210, 230], maillage_couleur: Some([10, 20, 30]), maillage_opacite: 0.5, maillage_pas: 0.5, ..Environnement3d::default() });
    assert!(b.app.modified, "le projet est à enregistrer");
    assert_eq!(b.app.history.undo_len(), etapes, "une façon de regarder : pas d'étape d'annulation");
    assert_eq!(b.app.e3_environnement().couleur_du_fond(), Color32::from_rgb(200, 210, 230));
    let apres = b.app.e3_quadrillage([0.0; 3], 8.0, None);
    // Le pas : deux traits voisins parallèles à X sont à 0,5 m.
    let mut z: Vec<f32> = apres.iter().filter(|l| (l.a[2] - l.b[2]).abs() < 1e-6).map(|l| l.a[2]).collect();
    z.sort_by(f32::total_cmp);
    assert!(z.windows(2).all(|w| ((w[1] - w[0]) - 0.5).abs() < 1e-4), "un trait tous les 0,5 m");
    assert!(apres.len() > avant.len(), "les traits couvrent la même étendue, plus serrés");
    // La couleur des traits ordinaires, l'opacité de tous ; les axes gardent leur couleur.
    let gris = |l: &GridLine| l.color[..3] == [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0];
    assert!(apres.iter().filter(|l| gris(l)).count() > 10, "les traits ordinaires sont à la couleur choisie");
    let axe = apres.iter().find(|l| l.color[0] > 0.85).expect("l'axe rouge reste");
    assert!((axe.color[3] - 0.75 * 0.5).abs() < 1e-5, "son opacité est multipliée : {:?}", axe.color);
    // Le bouton « Grille du sol » : le projet retient le maillage caché.
    b.app.run_action("espace3d.grille");
    assert!(!b.app.espace3d.show_grid);
    assert!(!b.app.e3_environnement().maillage);
    // L'aspect d'origine (maillage montré) : le projet ne retient plus rien.
    b.app.e3_poser_environnement(Environnement3d::default());
    assert!(b.app.doc.environnement_3d.is_none());
    assert!(b.app.espace3d.show_grid);
}

#[test]
fn un_ancien_projet_sans_ces_champs_s_ouvre_avec_l_aspect_d_origine() {
    let dir = dossier("ancien");
    let manifest = dir.join("P.shot");
    let doc = doc_carte(LayerKind::Bitmap);
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(!text.contains("environnement_3d") && !text.contains("calque_3d"), "un projet qui n'y touche pas n'écrit rien de nouveau");
    let mut b = ouvert(doc_carte(LayerKind::Bitmap));
    b.app.e3_poser_environnement(Environnement3d { maillage: false, ..Environnement3d::default() });
    b.app.load_doc(project::load(&manifest).expect("relu").doc);
    b.chauffer(2);
    assert!(b.app.doc.environnement_3d.is_none());
    assert!(b.app.espace3d.show_grid, "un projet d'avant montre le maillage");
    assert!(b.app.layers().iter().all(|l| l.calque_3d.is_none()));
    // Des champs écrits par une version plus récente, ou abîmés : le projet s'ouvre, à l'aspect d'origine, le calque ordinaire.
    for (env, calque) in [("5", "\"lune\""), ("{ \"fond\": \"noir\" }", "{ \"sorte\": \"lune\", \"pas\": 1.0 }"), ("null", "{ \"pas\": 2.0 }")] {
        let abime = text
            .replacen("\"largeur\"", &format!("\"environnement_3d\": {env},\n  \"largeur\""), 1)
            .replacen("\"nom\": \"Carte 0\"", &format!("\"calque_3d\": {calque},\n          \"nom\": \"Carte 0\""), 1);
        assert!(abime.contains("calque_3d") && abime.contains("environnement_3d"));
        std::fs::write(&manifest, abime).expect("écrit");
        let relu = project::load(&manifest).unwrap_or_else(|e| panic!("{env} / {calque} : le projet doit s'ouvrir ({e})"));
        assert!(relu.doc.environnement_3d.is_none(), "{env} : l'aspect d'origine");
        assert!(relu.doc.panels[0].layers[0].calque_3d.is_none(), "{calque} : un calque ordinaire");
        assert_eq!(relu.doc.panels[0].layers[0].name, "Carte 0");
    }
    // Des nombres abîmés reviennent dans leurs bornes.
    let e = Environnement3d::lire(serde_json::json!({ "fond": [1, 2, 3], "maillage_opacite": 7.0, "maillage_pas": -3.0 })).expect("lu");
    assert_eq!((e.fond, e.maillage, e.maillage_opacite, e.maillage_pas), ([1, 2, 3], true, 1.0, 0.0));
    let c = Calque3d::lire(serde_json::json!({ "sorte": "ciel", "pas": 0.0 })).expect("lu");
    assert_eq!((c.sorte, c.grille, c.pas), (Sorte3d::Ciel, false, 10.0));
    let c = Calque3d::lire(serde_json::json!({ "sorte": "mur", "grille": true })).expect("un pas absent ne perd pas le calque 3D");
    assert_eq!((c.sorte, c.grille, c.pas), (Sorte3d::Mur, true, 1.0));
    // Une carte démesurée : pas de grille plutôt qu'une boucle sans fin.
    assert!(lignes_de_grille([640.0, 360.0], [1e30, 1e30], 1e-30).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn les_nouveaux_champs_font_l_aller_retour_sans_changer_le_format() {
    // En JSON seul.
    let env = Environnement3d { fond: [12, 34, 56], maillage: false, maillage_couleur: Some([200, 100, 50]), maillage_opacite: 0.25, maillage_pas: 2.5 };
    assert_eq!(Environnement3d::lire(serde_json::to_value(env).expect("écrit")), Some(env));
    let c = Calque3d { sorte: Sorte3d::Mur, grille: true, pas: 2.5 };
    let v = serde_json::to_value(c).expect("écrit");
    assert_eq!(v, serde_json::json!({ "sorte": "mur", "grille": true, "pas": 2.5 }));
    assert_eq!(Calque3d::lire(v), Some(c));
    // Dans un projet enregistré puis rouvert.
    let dir = dossier("aller-retour");
    let manifest = dir.join("P.shot");
    let mut doc = doc_carte(LayerKind::Vector);
    doc.environnement_3d = Some(env);
    doc.panels[0].layers[0].calque_3d = Some(c);
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(text.contains("\"environnement_3d\"") && text.contains("\"calque_3d\""), "le manifeste les retient");
    assert!(text.contains(&format!("\"version\": {}", project::VERSION)), "le numéro de format ne change pas");
    assert_eq!(project::VERSION, 55, "on reste au format 55");
    let relu = project::load(&manifest).expect("relu");
    assert_eq!(relu.doc.environnement_3d, Some(env));
    assert_eq!(relu.doc.panels[0].layers[0].calque_3d, Some(c));
    // Rouvert dans l'application : le maillage suit le projet.
    let mut b = ouvert(doc_carte(LayerKind::Bitmap));
    b.app.load_doc(relu.doc);
    b.chauffer(2);
    assert!(!b.app.espace3d.show_grid, "le projet a caché le maillage");
    assert_eq!(b.app.e3_environnement().couleur_du_fond(), Color32::from_rgb(12, 34, 56));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ les calques 3D

#[test]
fn le_menu_calques_3d_pose_un_sol_un_mur_et_un_ciel_en_une_etape_chacun() {
    // Les trois commandes sont dans le menu « Nouveau calque ».
    let du_menu = crate::calquemenu::Sorte::actions_du_menu();
    for s in Sorte3d::TOUTES {
        assert!(du_menu.contains(&s.action()), "{} est dans le menu", s.nom());
    }
    // Hors d'un plan en Espace 3D : refusé, rien ne bouge, la barre d'état le dit d'abord.
    let mut b = ouvert(doc_carte(LayerKind::Bitmap));
    let (n, etapes) = (b.app.layers().len(), b.app.history.undo_len());
    b.app.run_action("calque.nouveau_sol");
    assert_eq!((b.app.layers().len(), b.app.history.undo_len()), (n, etapes));
    assert!(b.app.status.starts_with("Sol refusé : ce plan n'est pas en Espace 3D"), "{}", b.app.status);
    assert!(crate::messages::resume(&b.app.status).contains("pas en Espace 3D"), "la phrase utile tient dans la barre : {}", crate::messages::resume(&b.app.status));

    let mut b = en_espace(LayerKind::Bitmap);
    let world = b.app.e3_world();
    let oeil = world.eye(&b.app.current_camera());
    for s in Sorte3d::TOUTES {
        let (n, etapes) = (b.app.layers().len(), b.app.history.undo_len());
        b.app.run_action(s.action());
        b.chauffer(1);
        assert_eq!(b.app.layers().len(), n + 1, "{} : un calque de plus ({})", s.nom(), b.app.status);
        assert_eq!(b.app.history.undo_len(), etapes + 1, "{} : une étape d'annulation", s.nom());
        let l = b.app.layer().clone();
        assert_eq!(l.name, s.nom());
        assert_eq!(l.calque_3d, Some(Calque3d::neuf(s)), "marqué, grille éteinte");
        assert_eq!(l.kind, LayerKind::Bitmap, "du genre du calque courant");
        assert!(l.drawable() && l.visible, "un calque de dessin ordinaire");
        assert!(b.app.status.starts_with(&format!("« {} » posé à", s.nom())), "{}", b.app.status);
        assert!(b.app.espace3d.sel.contains(&crate::espace3d::card_selection(l.id)), "sa carte est choisie");
        let xf = world.carte(&l, 0);
        let t = l.pose(0);
        match s {
            Sorte3d::Sol => assert!((t.tilt_x.abs() - 90.0).abs() < 0.5, "couché : {t:?}"),
            Sorte3d::Mur => {
                assert!(t.tilt_x.abs() < 0.5 && t.tilt_y.abs() < 0.5, "debout, face à la caméra : {t:?}");
                assert!((xf.pos[1] - xf.size[1] / 2.0 - world.sol_y()).abs() < 1e-2, "posé sur le sol");
            }
            Sorte3d::Ciel => {
                assert!(t.tilt_x.abs() < 0.5 && t.tilt_y.abs() < 0.5, "debout, face à la caméra : {t:?}");
                let loin = oeil[2] - xf.pos[2];
                assert!(loin > 15.0 * world.d0, "tout au fond : {loin} m (distance de référence {} m)", world.d0);
                assert!((xf.pos[1] - oeil[1]).abs() < 1e-2, "centré sur l'horizon");
                assert!(xf.size[1] > 0.0, "à l'endroit");
                // Il remplit le cadre de la caméra.
                let cadre = crate::espace3d::SceneCam::from_plan_camera(&b.app.current_camera(), [b.app.doc.width, b.app.doc.height], b.app.doc.frame).corners_at(loin, b.app.doc.frame);
                let largeur_cadre = (cadre[1][0] - cadre[0][0]).abs();
                assert!(xf.size[0].abs() >= largeur_cadre * 0.99, "aussi large que le cadre : {} / {largeur_cadre}", xf.size[0]);
            }
        }
    }
    // Un deuxième Sol : « Sol 2 » ; l'annulation l'enlève.
    let n = b.app.layers().len();
    b.app.run_action("calque.nouveau_sol");
    assert_eq!(b.app.layer().name, "Sol 2");
    b.app.run_action("edition.annuler");
    b.chauffer(1);
    assert_eq!(b.app.layers().len(), n, "l'annulation enlève le calque");
}

#[test]
fn un_trait_sur_un_sol_du_menu_est_le_meme_que_sur_un_calque_ordinaire() {
    // Deux projets pareils : l'un a un Sol créé par le menu, grille allumée ; l'autre la surface au sol de toujours (un
    // calque ordinaire, au même endroit). Le même geste au pinceau dans la vue donne le même dessin, pixel pour pixel.
    let mut dessins = Vec::new();
    for par_le_menu in [true, false] {
        let mut b = en_espace(LayerKind::Bitmap);
        if par_le_menu {
            b.app.run_action("calque.nouveau_sol");
            b.app.run_action("calque.grille_3d");
            assert!(b.app.layer().calque_3d.is_some_and(|c| c.grille), "{}", b.app.status);
        } else {
            b.app.run_action("espace3d.surface_sol");
            assert!(b.app.layer().calque_3d.is_none(), "un calque ordinaire");
        }
        let li = b.app.cur_layer;
        b.app.run_action("espace3d.cadrer");
        b.chauffer(2);
        trait_sur_la_carte(&mut b, pos2(220.0, 140.0), pos2(420.0, 220.0));
        let (px, hors) = pixels(&b, li);
        assert!(px.iter().any(|c| c.a() > 0), "le trait a laissé de l'encre ({})", b.app.status);
        dessins.push((b.app.layers()[li].pose(0), px, hors, b.app.history.undo_len()));
    }
    let (menu, ordinaire) = (&dessins[0], &dessins[1]);
    assert_eq!(menu.0, ordinaire.0, "les deux cartes sont posées pareil");
    assert_eq!(menu.2, ordinaire.2, "rien de plus hors de la feuille");
    let differents = menu.1.iter().zip(&ordinaire.1).filter(|(a, b)| a != b).count();
    assert_eq!(differents, 0, "le même dessin, pixel pour pixel");
    // (Une étape de plus pour le Sol : la grille allumée.)
    assert_eq!(menu.3, ordinaire.3 + 1);
}

// ------------------------------------------------------------------------------------ la grille de repère

#[test]
fn les_traits_de_la_grille_suivent_le_pas_en_metres_et_restent_bornes() {
    // Une feuille de 640 × 360 pixels qui mesure 12 × 6,75 m : un trait tous les 53,3 pixels, depuis le milieu.
    let l = lignes_de_grille([640.0, 360.0], [12.0, 6.75], 1.0);
    let verticales: Vec<_> = l.iter().filter(|l| l.a.x == l.b.x).collect();
    let horizontales: Vec<_> = l.iter().filter(|l| l.a.y == l.b.y).collect();
    assert_eq!(verticales.len(), 13, "six de chaque côté du milieu");
    assert_eq!(horizontales.len(), 7);
    assert!(verticales.iter().any(|l| l.a.x == 320.0 && l.fort), "le trait du milieu est marqué");
    assert!(verticales.iter().all(|l| l.a.x >= 0.0 && l.a.x <= 640.0 && l.a.y == 0.0 && l.b.y == 360.0), "sur la feuille seulement");
    // Un pas minuscule : le nombre de traits reste borné.
    let l = lignes_de_grille([640.0, 360.0], [12.0, 6.75], 1e-4);
    assert!(l.len() as f32 <= 2.0 * (2.0 * TRAITS_MAX + 1.0), "{} traits", l.len());
    assert!(!l.is_empty());
    // Des mesures abîmées : rien.
    assert!(lignes_de_grille([640.0, 360.0], [0.0, 6.75], 1.0).is_empty());
    assert!(lignes_de_grille([640.0, 360.0], [12.0, 6.75], f32::NAN).is_empty());
}

#[test]
fn la_grille_n_est_jamais_dans_le_dessin_ni_sous_la_gomme_ni_dans_les_exports() {
    let mut b = en_espace(LayerKind::Bitmap);
    b.app.run_action("calque.nouveau_sol");
    let li = b.app.cur_layer;
    b.app.run_action("espace3d.cadrer");
    b.chauffer(2);
    trait_sur_la_carte(&mut b, pos2(220.0, 140.0), pos2(420.0, 220.0));
    let cel = b.app.layers()[li].shown_at(0).expect("un dessin");
    let (dessin, version) = (pixels(&b, li), b.app.doc.cels[&cel].version);
    let sans = images_exportees(&b.app.doc);

    // La grille allumée : une étape d'annulation, la barre d'état le dit ; le dessin ne bouge pas.
    let etapes = b.app.history.undo_len();
    b.app.run_action("calque.grille_3d");
    b.chauffer(3);
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert!(b.app.status.starts_with("Grille allumée"), "{}", b.app.status);
    assert_eq!(pixels(&b, li), dessin, "la grille n'est pas dans le dessin");
    assert_eq!(b.app.doc.cels[&cel].version, version, "le dessin n'a même pas été touché");
    assert!(images_exportees(&b.app.doc) == sans, "les images exportées sont celles d'avant la grille");
    // Dans le dessin à plat aussi (la vue fermée), la grille se peint sans toucher au dessin.
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.espace3d.on);
    assert_eq!(pixels(&b, li), dessin);

    // La gomme passe partout sur la carte : elle enlève le trait, la grille reste.
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    b.app.run_action("outil.gomme");
    b.chauffer(2);
    let vue = b.app.e3_carte_vue().expect("on gomme sur la carte");
    let pts: Vec<Pos2> = [pos2(200.0, 120.0), pos2(440.0, 240.0), pos2(200.0, 240.0), pos2(440.0, 120.0)].iter().filter_map(|p| vue.vers_ecran(*p)).collect();
    assert_eq!(pts.len(), 4);
    geste(&mut b, &pts);
    assert!(b.app.layers()[li].calque_3d.is_some_and(|c| c.grille), "la grille est toujours là");
    let encre = |p: &(Vec<Color32>, usize)| p.0.iter().filter(|c| c.a() > 0).count();
    assert!(encre(&pixels(&b, li)) < encre(&dessin), "la gomme a bien gommé le trait ({})", b.app.status);

    // Éteinte : une étape ; Ctrl+Z la rallume.
    b.app.run_action("calque.grille_3d");
    assert!(b.app.layers()[li].calque_3d.is_some_and(|c| !c.grille));
    b.app.run_action("edition.annuler");
    b.chauffer(1);
    assert!(b.app.layers()[li].calque_3d.is_some_and(|c| c.grille), "l'annulation la rallume");
    // Sur un calque ordinaire : rien, et la barre d'état le dit.
    b.app.cur_layer = 0;
    let etapes = b.app.history.undo_len();
    b.app.run_action("calque.grille_3d");
    assert_eq!(b.app.history.undo_len(), etapes);
    assert!(b.app.status.starts_with("Grille : seulement sur un calque Sol, Mur ou Ciel"), "{}", b.app.status);
}
