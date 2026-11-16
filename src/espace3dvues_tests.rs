//! Essais des vues multiples de l'Espace 3D (`espace3dvues.rs`, `docs/INTERFACE.md` partie 144).

use eframe::egui::{pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::document::{Document, Layer, LayerKind};
use crate::espace3d;
use crate::espace3dvues::{self as v, Disposition, PointDeVue, Retenue, Vues, POINTS_D_ORIGINE};
use crate::perf_tests::{banc_taille, Banc};
use crate::project;
use crate::raster::TILE;

const RED: Color32 = Color32::from_rgb(220, 20, 20);
const GREEN: Color32 = Color32::from_rgb(20, 200, 40);

/// Un projet 320 × 180 dont chaque calque a un rectangle plein, à la profondeur donnée.
fn doc_with_cards(cards: &[(Color32, [usize; 4], f32)]) -> Document {
    let mut doc = Document::new(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for (k, (color, r, depth)) in cards.iter().enumerate() {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        let raster = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
        for y in r[1]..r[3] {
            for x in r[0]..r[2] {
                let idx = raster.tile_index_of(x, y);
                raster.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = *color;
            }
        }
        layer.frames = vec![Some(cel); 12];
        layer.depth = *depth;
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// L'Espace 3D ouvert sur ces cartes, sans grille.
fn space_with(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.app.espace3d.show_grid = false;
    b.app.espace3d.snap = false;
    b.chauffer(4);
    b
}

fn space() -> Banc {
    space_with(doc_with_cards(&[(RED, [100, 50, 220, 130], 0.0)]))
}

/// Quatre vues, mises en place (deux images pour que les rectangles soient connus).
fn quatre() -> Banc {
    let mut b = space();
    b.app.run_action("espace3d.vues_quatre");
    b.chauffer(3);
    b
}

fn rect_of(b: &Banc, i: usize) -> Rect {
    b.app.espace3d.vues.rects.iter().find(|(k, _)| *k == i).map(|x| x.1).expect("vue montrée")
}

fn bouton(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE }
}

/// Un glisser du bouton `button` de `from` à `to`, en quelques images.
fn glisser(b: &mut Banc, button: PointerButton, from: Pos2, to: Pos2) {
    b.image(vec![Event::PointerMoved(from)], 0.02);
    b.image(vec![bouton(from, button, true)], 0.02);
    for k in 1..=6 {
        let p = from + (to - from) * (k as f32 / 6.0);
        b.image(vec![Event::PointerMoved(p)], 0.02);
    }
    b.image(vec![bouton(to, button, false)], 0.02);
    b.chauffer(2);
}

fn clic(b: &mut Banc, at: Pos2) {
    b.image(vec![Event::PointerMoved(at)], 0.02);
    b.image(vec![bouton(at, PointerButton::Primary, true)], 0.02);
    b.image(vec![bouton(at, PointerButton::Primary, false)], 0.02);
    b.chauffer(2);
}

/// Le point `p` de l'espace dans la vue `i`, à l'écran.
fn a_l_ecran(b: &Banc, i: usize, p: [f32; 3]) -> Pos2 {
    let r = rect_of(b, i);
    let view = b.app.espace3d.vues.cadres[i].vue(r.width()).expect("une vue qui n'est pas la caméra");
    let q = view.project(p, [r.width() / 2.0, r.height() / 2.0]).expect("visible");
    pos2(r.left() + q[0], r.top() + q[1])
}

// ================================================================================ maths et rangement

#[test]
fn les_rectangles_partagent_la_zone_sans_se_chevaucher() {
    let zone = Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0));
    let q = v::rects(Disposition::Quatre, [0.5, 0.5], zone, None);
    assert_eq!(q.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    let aire: f32 = q.iter().map(|x| x.1.area()).sum();
    assert!(aire <= zone.area() && aire > zone.area() * 0.98, "les quatre vues couvrent la zone : {aire}");
    for a in &q {
        for b in &q {
            if a.0 != b.0 {
                assert!(!a.1.intersects(b.1), "{a:?} et {b:?} ne se chevauchent pas");
            }
        }
    }
    let c = v::rects(Disposition::CoteACote, [0.3, 0.5], zone, None);
    assert!((c[0].1.width() - 240.0).abs() < 2.0 && c[0].1.height() == 600.0 && c[1].1.left() > c[0].1.right());
    let s = v::rects(Disposition::LUneSurLAutre, [0.5, 0.7], zone, None);
    assert!((s[0].1.height() - 420.0).abs() < 2.0 && s[1].1.top() > s[0].1.bottom() && s[0].1.width() == 800.0);
    // Agrandie : une seule, toute la zone. Une bordure ne sort pas de ses bornes.
    assert_eq!(v::rects(Disposition::Quatre, [0.5, 0.5], zone, Some(2)), vec![(2, zone)]);
    let x = v::rects(Disposition::CoteACote, [0.0, 0.5], zone, None);
    assert!(x[0].1.width() >= 800.0 * v::PARTAGE_MIN - 2.0);
}

#[test]
fn une_vue_orthographique_ne_montre_pas_la_profondeur() {
    let o = v::Ortho { target: [0.0, 0.0, 0.0], span: 10.0 };
    for p in [PointDeVue::Dessus, PointDeVue::Dessous, PointDeVue::Face, PointDeVue::Arriere, PointDeVue::Gauche, PointDeVue::Droite] {
        let view = v::vue_ortho(p, &o, 1000.0).expect("orthographique");
        assert!(v::est_ortho(&view) && (v::largeur_ortho(&view).expect("largeur") - 10.0).abs() < 1e-3);
        let (f, r, _) = p.axes().expect("axes");
        // Deux points à 6 m l'un de l'autre dans la profondeur de la vue : au même pixel (au centième près).
        let a = crate::espace3dmath::scale(r, 2.0);
        let b = crate::espace3dmath::add(a, crate::espace3dmath::scale(f, 6.0));
        let (pa, pb) = (view.project(a, [500.0, 300.0]).expect("visible"), view.project(b, [500.0, 300.0]).expect("visible"));
        assert!((pa[0] - pb[0]).abs() < 0.1 && (pa[1] - pb[1]).abs() < 0.1, "{p:?} : {pa:?} contre {pb:?}");
        // 2 m vers la droite de la vue = 200 pixels sur 1000 qui montrent 10 m.
        assert!((pa[0] - 700.0).abs() < 0.5, "{p:?} : {pa:?}");
    }
    // Le dessus : X vers la droite, le fond (-Z) vers le haut de l'écran ; la face : Y vers le haut.
    let dessus = v::vue_ortho(PointDeVue::Dessus, &o, 1000.0).expect("vue");
    assert!(dessus.project([0.0, 0.0, -1.0], [500.0, 300.0]).expect("visible")[1] < 300.0);
    let face = v::vue_ortho(PointDeVue::Face, &o, 1000.0).expect("vue");
    assert!(face.project([0.0, 1.0, 0.0], [500.0, 300.0]).expect("visible")[1] < 300.0);
    let droite = v::vue_ortho(PointDeVue::Droite, &o, 1000.0).expect("vue");
    assert!(droite.project([0.0, 0.0, 1.0], [500.0, 300.0]).expect("visible")[0] < 500.0, "vue de droite : la caméra du plan (+Z) est à gauche");
}

#[test]
fn ce_que_retient_le_projet_se_relit_meme_abime() {
    let mut vues = Vues::default();
    vues.disposition = Disposition::Quatre;
    vues.cadres[2].point = PointDeVue::Camera;
    vues.partage = [0.3, 0.6];
    let r = vues.retenue();
    let back = Vues::depuis(Some(&r));
    assert_eq!(back.disposition, Disposition::Quatre);
    assert_eq!(back.cadres.map(|c| c.point)[2], PointDeVue::Camera);
    assert_eq!(back.partage, [0.3, 0.6]);
    // Des noms inconnus (une version plus récente) et des bordures hors de leurs bornes : les valeurs d'origine.
    let abimee: Retenue = serde_json::from_str(r#"{ "disposition": "seize", "points": ["hyper", "dessous"], "partage": [7.0, -1.0] }"#).expect("se lit");
    let back = Vues::depuis(Some(&abimee));
    assert_eq!(back.disposition, Disposition::Une);
    assert_eq!(back.cadres[0].point, POINTS_D_ORIGINE[0]);
    assert_eq!(back.cadres[1].point, PointDeVue::Dessous);
    assert_eq!(back.partage, [v::PARTAGE_MAX, v::PARTAGE_MIN]);
    // Rien : une vue.
    assert_eq!(Vues::depuis(None).disposition, Disposition::Une);
}

// ================================================================================ dans l'application

#[test]
fn changer_de_disposition() {
    let mut b = space();
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Une, "une vue au départ, comme avant");
    assert!(b.app.doc.vues_3d.is_none());
    b.app.run_action("espace3d.vues_quatre");
    b.chauffer(3);
    let canvas = b.app.canvas_rect;
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Quatre);
    assert_eq!(b.app.espace3d.vues.rects.len(), 4, "quatre vues montrées");
    assert_eq!(b.app.espace3d.vues.cadres[..4].iter().map(|c| c.point).collect::<Vec<_>>(), vec![PointDeVue::Dessus, PointDeVue::Face, PointDeVue::Droite, PointDeVue::Perspective], "les quatre points de vue d'origine");
    assert!(b.app.espace3d.vues.rects.iter().all(|(_, r)| canvas.contains_rect(*r)));
    b.app.run_action("espace3d.vues_cote");
    b.chauffer(2);
    let r = &b.app.espace3d.vues.rects;
    assert!(r.len() == 2 && r[0].1.right() < r[1].1.left() && (r[0].1.height() - r[1].1.height()).abs() < 1.0, "côte à côte : {r:?}");
    b.app.run_action("espace3d.vues_superposees");
    b.chauffer(2);
    let r = &b.app.espace3d.vues.rects;
    assert!(r.len() == 2 && r[0].1.bottom() < r[1].1.top() && (r[0].1.width() - r[1].1.width()).abs() < 1.0, "l'une sur l'autre : {r:?}");
    // La commande du bouton passe à la suivante : après « l'une sur l'autre », les quatre vues, puis une seule.
    b.app.run_action("espace3d.vues");
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Quatre);
    b.app.run_action("espace3d.vues");
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Une);
    b.chauffer(2);
    assert!(b.app.doc.vues_3d.is_some(), "le projet retient la disposition");
    assert!(b.app.history.names().0.is_empty(), "changer de vues n'est pas une étape d'annulation");
    // Les commandes sont assignables (Préférences > Raccourcis) et connues.
    for d in Disposition::ALL {
        assert!(crate::keymap::action_info(d.action()).is_some_and(|a| a.implemented), "{}", d.action());
    }
    assert!(crate::keymap::action_info("espace3d.vues").is_some() && crate::keymap::action_info("espace3d.vue_agrandir").is_some());
}

#[test]
fn point_de_vue_par_vue() {
    let mut b = quatre();
    let avant: Vec<PointDeVue> = b.app.espace3d.vues.cadres.iter().map(|c| c.point).collect();
    b.app.e3_vues_point(1, PointDeVue::Arriere);
    b.chauffer(2);
    let apres: Vec<PointDeVue> = b.app.espace3d.vues.cadres.iter().map(|c| c.point).collect();
    assert_eq!(apres[1], PointDeVue::Arriere);
    for k in [0, 2, 3, 4, 5] {
        assert_eq!(apres[k], avant[k], "les autres vues gardent leur point de vue");
    }
    // Chaque vue regarde d'où elle doit : le dessus vers le bas, l'arrière vers +Z.
    let w = rect_of(&b, 0).width();
    assert_eq!(b.app.espace3d.vues.cadres[0].vue(w).expect("vue").forward, [0.0, -1.0, 0.0]);
    assert_eq!(b.app.espace3d.vues.cadres[1].vue(w).expect("vue").forward, [0.0, 0.0, 1.0]);
    // La caméra du plan : la vue montre le cadre de la caméra.
    b.app.e3_vues_point(2, PointDeVue::Camera);
    b.chauffer(2);
    assert_eq!(b.app.espace3d.vues.cadres[2].point, PointDeVue::Camera);
    assert!(b.app.e3_par_la_camera(), "la vue active filme par la caméra : le bouton est enfoncé");
    assert!(!b.app.espace3d.through_camera, "la vue seule, elle, n'a pas changé");
}

#[test]
fn geste_route_vers_la_bonne_vue() {
    let mut b = quatre();
    let avant = b.app.espace3d.vues.cadres;
    // La molette sur la vue de dessus (0) : elle seule zoome.
    let c0 = rect_of(&b, 0).center();
    b.image(vec![Event::PointerMoved(c0)], 0.02);
    for _ in 0..6 {
        let mut e = b.molette(vec2(0.0, 60.0), Modifiers::NONE);
        e.insert(0, Event::PointerMoved(c0));
        b.image(e, 0.02);
    }
    b.chauffer(10);
    let apres = b.app.espace3d.vues.cadres;
    assert!(apres[0].ortho.span < avant[0].ortho.span * 0.95, "la vue de dessus a zoomé : {} → {}", avant[0].ortho.span, apres[0].ortho.span);
    for k in 1..4 {
        assert_eq!(apres[k], avant[k], "la vue {k} n'a pas bougé");
    }
    assert_eq!(b.app.espace3d.vues.active, 0, "la vue touchée devient la vue active");
    // Le bouton du milieu dans la vue en perspective (3) : elle seule glisse.
    let r3 = rect_of(&b, 3);
    let avant = b.app.espace3d.vues.cadres;
    glisser(&mut b, PointerButton::Middle, r3.center(), r3.center() + vec2(80.0, 30.0));
    let apres = b.app.espace3d.vues.cadres;
    assert_ne!(apres[3].orbite.target, avant[3].orbite.target, "la perspective a glissé");
    for k in 0..3 {
        assert_eq!(apres[k], avant[k], "la vue {k} n'a pas bougé");
    }
    assert_eq!(b.app.espace3d.vues.active, 3);
    assert_eq!(b.app.espace3d.vues.sous_pointeur, Some(3));
}

#[test]
fn orthographique_sans_rotation() {
    let mut b = quatre();
    // Glisser (milieu) et glisser dans le vide (bouton principal) : la vue de face reste de face.
    let r = rect_of(&b, 1);
    let avant = b.app.espace3d.vues.cadres[1];
    glisser(&mut b, PointerButton::Middle, r.center(), r.center() + vec2(40.0, 0.0));
    let vide = pos2(r.right() - 30.0, r.bottom() - 40.0);
    glisser(&mut b, PointerButton::Primary, vide, vide + vec2(-60.0, -20.0));
    let c = b.app.espace3d.vues.cadres[1];
    assert_eq!(c.point, PointDeVue::Face, "elle ne tourne pas");
    assert_ne!(c.ortho.target, avant.ortho.target, "elle a glissé");
    // Un geste de rotation (clic droit) : elle passe en perspective, et la barre d'état le dit.
    glisser(&mut b, PointerButton::Secondary, r.center(), r.center() + vec2(60.0, 0.0));
    let c = b.app.espace3d.vues.cadres[1];
    assert_eq!(c.point, PointDeVue::Perspective);
    assert!(b.app.status.contains("perspective"), "{}", b.app.status);
    // Elle part de là où regardait la face (à peu près de face, un peu tournée).
    assert!(c.orbite.elevation.abs() < 5.0 && c.orbite.azimuth.rem_euclid(360.0) > 300.0, "{:?}", c.orbite);
    assert_eq!(b.app.doc.vues_3d.as_ref().map(|r| r.points[1].clone()), Some("perspective".to_string()), "le projet le retient");
}

#[test]
fn agrandir_puis_revenir() {
    let mut b = quatre();
    let canvas = b.app.canvas_rect;
    // Le raccourci : la vue sous le pointeur.
    let c2 = rect_of(&b, 2).center();
    b.image(vec![Event::PointerMoved(c2)], 0.02);
    b.chauffer(1);
    b.app.run_action("espace3d.vue_agrandir");
    b.chauffer(2);
    assert_eq!(b.app.espace3d.vues.agrandie, Some(2));
    assert_eq!(b.app.espace3d.vues.rects, vec![(2, canvas)], "en plein cadre");
    b.app.run_action("espace3d.vue_agrandir");
    b.chauffer(2);
    assert_eq!(b.app.espace3d.vues.agrandie, None);
    assert_eq!(b.app.espace3d.vues.rects.len(), 4, "retour aux quatre vues");
    // Un double-clic sur le menu de la vue 1.
    let menu = rect_of(&b, 1).min + vec2(24.0, 15.0);
    b.image(vec![Event::PointerMoved(menu)], 0.02);
    for _ in 0..2 {
        b.image(vec![bouton(menu, PointerButton::Primary, true)], 0.02);
        b.image(vec![bouton(menu, PointerButton::Primary, false)], 0.02);
    }
    b.chauffer(2);
    assert_eq!(b.app.espace3d.vues.agrandie, Some(1), "le double-clic sur son menu l'agrandit");
    let menu = canvas.min + vec2(24.0, 15.0);
    // (Une seconde plus tard : sinon egui compterait un triple clic avec le double clic d'avant.)
    b.chauffer(60);
    b.image(vec![Event::PointerMoved(menu)], 0.02);
    for _ in 0..2 {
        b.image(vec![bouton(menu, PointerButton::Primary, true)], 0.02);
        b.image(vec![bouton(menu, PointerButton::Primary, false)], 0.02);
    }
    b.chauffer(2);
    assert_eq!(b.app.espace3d.vues.agrandie, None, "un autre double-clic ramène la disposition");
}

#[test]
fn selection_visible_partout() {
    let mut b = quatre();
    let card = espace3d::card_selection(b.app.layers()[0].id);
    b.app.espace3d.sel.clear();
    b.chauffer(2);
    // Un clic sur la carte dans la vue de face la choisit.
    let centre = b.app.e3_world().card_xf(&b.app.layers()[0].pose(0), b.app.layers()[0].depth_at(0)).pos;
    // (Un peu à côté du centre : dans la vue de face, l'œil de la caméra, devant la carte, est sur son centre, et le corps de
    // la caméra se choisit avant la carte qu'il couvre — partie 152 ; ses traits fins, eux, laissent la carte se prendre.)
    let at = a_l_ecran(&b, 1, centre) + vec2(18.0, -14.0);
    clic(&mut b, at);
    assert_eq!(b.app.espace3d.sel, vec![card], "choisie dans la vue de face");
    // Toutes les vues la montrent choisie : chacune a son manipulateur.
    for (i, r) in b.app.espace3d.vues.rects.clone() {
        let c = b.app.espace3d.vues.cadres[i];
        let view = c.vue(r.width()).expect("vue");
        assert!(b.app.e3_gizmo(&view).is_some(), "la vue {i} montre le manipulateur de la carte choisie");
    }
    // Déplacée de 1 m vers la droite : la vue de dessus la voit bouger d'autant.
    let avant = a_l_ecran(&b, 0, centre);
    let xfs = b.app.e3_xfs(&[card]);
    b.app.e3_set_xfs(&crate::espace3dgizmo::move_xfs(&xfs, [1.0, 0.0, 0.0]));
    b.chauffer(2);
    let nouveau = b.app.e3_world().card_xf(&b.app.layers()[0].pose(0), b.app.layers()[0].depth_at(0)).pos;
    let apres = a_l_ecran(&b, 0, nouveau);
    let px_par_m = rect_of(&b, 0).width() / b.app.espace3d.vues.cadres[0].ortho.span;
    assert!(((apres.x - avant.x) - px_par_m).abs() < 1.0, "{avant:?} → {apres:?} ({px_par_m} px par mètre)");
    // Dans la vue de dessus, la carte est vue par la tranche : un clic sur son trait la choisit aussi.
    b.app.espace3d.sel.clear();
    b.chauffer(2);
    clic(&mut b, apres);
    assert_eq!(b.app.espace3d.sel, vec![card], "choisie par la tranche dans la vue de dessus");
}

#[test]
fn bordure_glissee_et_vues_enregistrees_puis_rouvertes() {
    let dir = std::env::temp_dir().join(format!("shot-test-vues3d-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    let manifest = dir.join("P.shot");
    let mut b = quatre();
    b.app.e3_vues_point(0, PointDeVue::Dessous);
    // La bordure verticale, glissée au tiers.
    let canvas = b.app.canvas_rect;
    let x = canvas.left() + canvas.width() * 0.5;
    glisser(&mut b, PointerButton::Primary, pos2(x, canvas.top() + 200.0), pos2(canvas.left() + canvas.width() * 0.3, canvas.top() + 200.0));
    let p = b.app.espace3d.vues.partage[0];
    assert!((p - 0.3).abs() < 0.02, "la bordure a suivi : {p}");
    assert!(rect_of(&b, 0).width() < rect_of(&b, 1).width(), "la vue de gauche est plus étroite");
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(text.contains("\"vues_3d\"") && text.contains("\"quatre\"") && text.contains("\"dessous\""), "le manifeste retient les vues");
    assert!(text.contains(&format!("\"version\": {}", project::VERSION)), "le numéro de format ne change pas");
    let loaded = project::load(&manifest).expect("relu");
    let mut c = space_with(doc_with_cards(&[(GREEN, [0, 0, 50, 50], 0.0)]));
    assert_eq!(c.app.espace3d.vues.disposition, Disposition::Une);
    c.app.load_doc(loaded.doc);
    c.chauffer(3);
    let vues = &c.app.espace3d.vues;
    assert_eq!(vues.disposition, Disposition::Quatre, "rouvert en quatre vues");
    assert_eq!(vues.cadres[0].point, PointDeVue::Dessous);
    assert!((vues.partage[0] - p).abs() < 1e-4);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn ancien_projet_en_une_vue() {
    let dir = std::env::temp_dir().join(format!("shot-test-vues3d-ancien-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    let manifest = dir.join("P.shot");
    let doc = doc_with_cards(&[(RED, [10, 10, 80, 60], 0.0)]);
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    assert!(!text.contains("vues_3d"), "un projet qui n'a pas touché aux vues n'écrit rien de nouveau");
    let mut b = quatre();
    b.app.load_doc(project::load(&manifest).expect("relu").doc);
    b.chauffer(3);
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Une, "un projet d'avant s'ouvre en une vue");
    // Un champ écrit par une version plus récente, aux valeurs inconnues : le projet s'ouvre, en une vue.
    let abime = text.replacen("\"largeur\"", "\"vues_3d\": { \"disposition\": \"seize\", \"points\": [\"hyper\"], \"partage\": [9.0, 0.5], \"autre\": 1 },\n  \"largeur\"", 1);
    std::fs::write(&manifest, abime).expect("écrit");
    let relu = project::load(&manifest).expect("un champ inconnu n'empêche pas d'ouvrir");
    b.app.load_doc(relu.doc);
    b.chauffer(3);
    assert_eq!(b.app.espace3d.vues.disposition, Disposition::Une);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn quatre_vues_restent_sans_latence() {
    let cards: Vec<_> = (0..10).map(|k| (if k % 2 == 0 { RED } else { GREEN }, [10 + k * 20, 20, 90 + k * 20, 140], k as f32 * 0.4 - 1.0)).collect();
    let mut b = banc_taille(doc_with_cards(&cards), true, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.chauffer(6);
    let ids: Vec<u64> = b.app.layers().iter().map(|l| espace3d::card_selection(l.id)).collect();
    b.app.espace3d.sel = ids;
    let mesurer = |b: &mut Banc| {
        let frames: Vec<f64> = (0..60).map(|k| b.image(vec![Event::PointerMoved(pos2(700.0 + k as f32, 400.0))], 1.0 / 60.0).total()).collect();
        (frames.iter().sum::<f64>() / frames.len() as f64, frames.iter().cloned().fold(0.0, f64::max))
    };
    let (une, une_max) = mesurer(&mut b);
    b.app.run_action("espace3d.vues_quatre");
    b.chauffer(6);
    let (quatre, quatre_max) = mesurer(&mut b);
    println!("PERF Espace 3D, 10 cartes, carte graphique {} : 1 vue {une:.2} ms par image (max {une_max:.2}), 4 vues {quatre:.2} ms (max {quatre_max:.2})", b.app.gpu.is_some());
    assert!(quatre < 16.0, "{quatre:.2} ms par image en 4 vues : l'Espace 3D doit tenir 60 images par seconde");
}

// ================================================================================ relecture de 16f24d9

/// Un geste de manipulateur dans la vue `i` (coordonnées dans la vue), en une étape.
fn geste_vue(b: &mut Banc, i: usize, handle: crate::espace3dgizmo::Handle, from: [f32; 2], to: &[[f32; 2]]) {
    let r = rect_of(b, i);
    let view = b.app.espace3d.vues.cadres[i].vue(r.width()).expect("vue");
    let center = [r.width() / 2.0, r.height() / 2.0];
    let gizmo = b.app.e3_gizmo(&view).expect("un manipulateur");
    let mode = b.app.espace3d.mode;
    b.app.e3_begin(handle, mode, gizmo, view.ray(from, center), from, "Geste d'essai");
    for p in to {
        b.app.e3_drag(&view, center, *p);
    }
    b.app.e3_finish_drag();
}

/// Le point `p` de l'espace dans la vue `i`, en coordonnées de la vue.
fn dans_la_vue(b: &Banc, i: usize, p: [f32; 3]) -> [f32; 2] {
    let r = rect_of(b, i);
    let q = a_l_ecran(b, i, p);
    [q.x - r.left(), q.y - r.top()]
}

fn px_par_metre(b: &Banc, i: usize) -> f32 {
    rect_of(b, i).width() / b.app.espace3d.vues.cadres[i].ortho.span
}

#[test]
fn vue_de_dessus_une_carte_dressee_glisse_dans_le_plan_de_l_ecran() {
    let mut b = quatre();
    b.app.espace3d.snap = false;
    let card = espace3d::card_selection(b.app.layers()[0].id);
    b.app.espace3d.sel = vec![card];
    b.app.espace3d.mode = crate::espace3dgizmo::Mode::Move;
    let avant = b.app.e3_xfs(&[card])[0].1.pos;
    let p = dans_la_vue(&b, 0, avant);
    // La carte est vue par la tranche : son plan (XY) est de profil. 50 px à droite et 30 px vers le haut de l'écran.
    geste_vue(&mut b, 0, crate::espace3dgizmo::Handle::Plane(2), p, &[[p[0] + 50.0, p[1] - 30.0]]);
    let apres = b.app.e3_xfs(&[card])[0].1.pos;
    let k = px_par_metre(&b, 0);
    let (dx, dz) = (apres[0] - avant[0], apres[2] - avant[2]);
    assert!((dx - 50.0 / k).abs() < 0.02 && (dz + 30.0 / k).abs() < 0.02 && (apres[1] - avant[1]).abs() < 1e-3, "glissée de {dx} m en X et {dz} m en Z, attendu {} et {}", 50.0 / k, -30.0 / k);
}

#[test]
fn vue_de_face_la_camera_glisse_dans_le_plan_de_l_ecran() {
    let mut b = quatre();
    b.app.espace3d.snap = false;
    b.app.espace3d.sel = vec![crate::espace3dgizmo::CAMERA_ID];
    b.app.espace3d.mode = crate::espace3dgizmo::Mode::Move;
    let avant = b.app.e3_xfs(&[crate::espace3dgizmo::CAMERA_ID])[0].1.pos;
    let p = dans_la_vue(&b, 1, avant);
    // Le plan horizontal de la caméra est de profil dans la vue de face.
    geste_vue(&mut b, 1, crate::espace3dgizmo::Handle::Plane(1), p, &[[p[0] + 40.0, p[1] - 20.0]]);
    let apres = b.app.e3_xfs(&[crate::espace3dgizmo::CAMERA_ID])[0].1.pos;
    let k = px_par_metre(&b, 1);
    assert!((apres[0] - avant[0] - 40.0 / k).abs() < 0.02 && (apres[1] - avant[1] - 20.0 / k).abs() < 0.02 && (apres[2] - avant[2]).abs() < 1e-3, "{avant:?} → {apres:?} ({k} px/m)");
}

#[test]
fn un_anneau_vu_de_profil_tourne_d_un_angle_fini_et_monotone() {
    let mut b = quatre();
    b.app.espace3d.snap = false;
    let card = espace3d::card_selection(b.app.layers()[0].id);
    b.app.espace3d.sel = vec![card];
    b.app.espace3d.mode = crate::espace3dgizmo::Mode::Rotate;
    let o = dans_la_vue(&b, 0, b.app.e3_xfs(&[card])[0].1.pos);
    let r = rect_of(&b, 0);
    let view = b.app.espace3d.vues.cadres[0].vue(r.width()).expect("vue");
    let center = [r.width() / 2.0, r.height() / 2.0];
    let gizmo = b.app.e3_gizmo(&view).expect("manipulateur");
    let at = |deg: f32| [o[0] + 60.0 * deg.to_radians().cos(), o[1] + 60.0 * deg.to_radians().sin()];
    // L'anneau autour de X : vu du dessus, un trait.
    b.app.e3_begin(crate::espace3dgizmo::Handle::Ring(0), crate::espace3dgizmo::Mode::Rotate, gizmo, view.ray(at(0.0), center), at(0.0), "Tourner");
    let mut angles = Vec::new();
    for deg in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0] {
        b.app.e3_drag(&view, center, at(deg));
        angles.push(b.app.layers()[0].pose(0).tilt_x);
    }
    b.app.e3_finish_drag();
    assert!(angles.iter().all(|a| a.is_finite()), "{angles:?}");
    let croissant = angles.windows(2).all(|w| w[1] > w[0]);
    let decroissant = angles.windows(2).all(|w| w[1] < w[0]);
    assert!(croissant || decroissant, "l'angle suit la souris sans à-coups : {angles:?}");
    assert!((angles[5] - angles[0]).abs() > 30.0 && (angles[5] - angles[0]).abs() < 70.0, "50° de souris autour du centre : {angles:?}");
}

#[test]
fn un_champ_vues_3d_mal_forme_n_empeche_pas_d_ouvrir_le_projet() {
    let dir = std::env::temp_dir().join(format!("shot-test-vues3d-abime-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    let manifest = dir.join("P.shot");
    project::save(&doc_with_cards(&[(RED, [10, 10, 80, 60], 0.0)]), &manifest, &mut Default::default()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).expect("manifeste");
    let mut b = quatre();
    for champ in [r#"{ "disposition": "quatre", "partage": [0.5] }"#, r#"{ "disposition": "quatre", "partage": null }"#, r#"{ "disposition": "quatre", "points": [1] }"#, "3"] {
        std::fs::write(&manifest, text.replacen("\"largeur\"", &format!("\"vues_3d\": {champ},\n  \"largeur\""), 1)).expect("écrit");
        let relu = project::load(&manifest).unwrap_or_else(|e| panic!("vues_3d = {champ} : le projet doit s'ouvrir ({e})"));
        b.app.load_doc(relu.doc);
        b.chauffer(2);
        assert_eq!(b.app.espace3d.vues.disposition, Disposition::Une, "vues_3d = {champ} : une vue");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn une_vue_orthographique_le_reste_au_plus_grand_zoom_arriere() {
    for span in [0.02, 10.0, 2000.0, 1e4] {
        let view = v::vue_ortho(PointDeVue::Face, &v::Ortho { target: [0.0; 3], span }, 800.0).expect("vue");
        assert!(v::est_ortho(&view), "largeur {span} m : toujours orthographique");
        assert!((v::largeur_ortho(&view).expect("largeur") - span).abs() <= span * 1e-4, "largeur {span}");
    }
}

#[test]
fn glisser_en_vue_de_face_ne_fait_pas_deriver_la_profondeur() {
    let mut b = quatre();
    b.app.espace3d.snap = false;
    let card = espace3d::card_selection(b.app.layers()[0].id);
    b.app.espace3d.sel = vec![card];
    b.app.espace3d.mode = crate::espace3dgizmo::Mode::Move;
    let z0 = b.app.e3_xfs(&[card])[0].1.pos[2];
    let d0 = b.app.layers()[0].depth_at(0);
    for _ in 0..20 {
        let p = dans_la_vue(&b, 1, b.app.e3_xfs(&[card])[0].1.pos);
        geste_vue(&mut b, 1, crate::espace3dgizmo::Handle::Plane(2), p, &[[p[0] + 80.0, p[1] + 35.0]]);
        let p = dans_la_vue(&b, 1, b.app.e3_xfs(&[card])[0].1.pos);
        geste_vue(&mut b, 1, crate::espace3dgizmo::Handle::Plane(2), p, &[[p[0] - 80.0, p[1] - 35.0]]);
    }
    let z = b.app.e3_xfs(&[card])[0].1.pos[2];
    assert!((z - z0).abs() < 1e-4 && (b.app.layers()[0].depth_at(0) - d0).abs() < 1e-4, "20 allers-retours : Z {z0} → {z}");
    // Maj + glisser le long de l'axe de visée (Z, vu de bout) : rien ne bouge, et la ligne d'état le dit.
    let avant = b.app.e3_xfs(&[card])[0].1.pos;
    let p = dans_la_vue(&b, 1, avant);
    geste_vue(&mut b, 1, crate::espace3dgizmo::Handle::Axis(2), p, &[[p[0] + 30.0, p[1] + 30.0]]);
    assert_eq!(b.app.e3_xfs(&[card])[0].1.pos, avant);
    assert!(b.app.status.contains("vu de bout"), "{}", b.app.status);
}

#[test]
fn une_vue_de_taille_nulle_attend_pour_se_cadrer_et_un_rayon_abime_ne_touche_rien() {
    let mut b = space();
    b.app.canvas_rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(0.0, 0.0));
    b.app.espace3d.vues.rects.clear();
    b.app.e3_vues_poser(Disposition::Quatre);
    assert!(!b.app.espace3d.vues.prets, "des vues de 0 × 0 ne sont pas cadrées");
    b.chauffer(3);
    assert!(b.app.espace3d.vues.prets, "cadrées dès qu'elles ont leur taille");
    assert!(b.app.espace3d.vues.cadres.iter().all(|c| c.ortho.span.is_finite() && c.orbite.distance.is_finite()));
    let w = b.app.e3_world();
    let xf = w.card_xf(&b.app.layers()[0].pose(0), b.app.layers()[0].depth_at(0));
    let abime = crate::espace3dmath::Ray { o: [f32::NAN; 3], d: [0.0, 0.0, -1.0] };
    assert!(w.pick_card(&xf, &abime).is_none(), "un rayon abîmé ne touche rien");
    let abime = crate::espace3dmath::Ray { o: [0.0, 0.0, 10.0], d: [f32::NAN, 0.0, -1.0] };
    assert!(w.pick_card(&xf, &abime).is_none());
}