//! Essais de la partie 141 : un calque accroché à la caméra (`accroche.rs`). La règle : il garde sa place dans l'image filmée, quoi que
//! fasse la caméra ; l'accrocher ou le décrocher ne le déplace pas à l'image courante.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect};

use crate::app::{App, Tool};
use crate::camera::{Camera, CameraKey};
use crate::document::{CameraSeen, Document, Layer, LayerKind, LayerTransform};
use crate::render;

const ROUGE: Color32 = Color32::from_rgb(220, 20, 20);

fn rouge(c: Color32) -> bool {
    c.a() > 120 && c.r() > 140 && c.g() < 110
}

/// Feuille 200 × 120 (le cadre a la même taille), un plan de 6 images, un calque bitmap « Perso » avec un carré rouge de 8 px
/// (40..48 × 30..38) tenu sur tout le plan.
fn doc_carre() -> Document {
    let mut doc = Document::new(200, 120, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Bitmap)];
    let c = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&c).unwrap().raster;
    let idx = r.tile_index_of(40, 30);
    for y in 30..38 {
        for x in 40..48 {
            r.tile_mut(idx)[y * crate::raster::TILE + x] = ROUGE;
        }
    }
    doc.panels[0].layers[0].frames = vec![Some(c); 6];
    doc.panels[0].length = 6;
    doc
}

/// Une caméra décalée, zoomée, tournée et avancée.
fn camera_ailleurs(doc: &Document) -> Camera {
    let c = doc.default_camera();
    Camera { x: c.x + 30.0, y: c.y + 10.0, zoom: 1.6, angle: 20.0, dolly: 15.0, ..c }
}

/// Où tombe, dans l'image filmée à l'image `f` du plan, le point `q` du dessin du calque `li`.
fn dans_l_image(doc: &Document, f: usize, li: usize, q: Pos2) -> Pos2 {
    let sur_la_feuille = sur_la_feuille(doc, f, li, q);
    crate::persp::apply(&doc.panel_camera(0, f).homography_inverse(doc.frame), sur_la_feuille)
}

/// Où tombe, sur la feuille à l'image `f`, le point `q` du dessin du calque `li`.
fn sur_la_feuille(doc: &Document, f: usize, li: usize, q: Pos2) -> Pos2 {
    crate::multisel::layer_to_stage(doc, 0, f, li).expect("calque posé").apply(q)
}

fn proche(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 0.05
}

/// Le centre et le nombre de pixels rouges d'une image `w` de large.
fn tache(px: &[Color32], w: usize) -> Option<(Pos2, usize)> {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0usize);
    for (i, c) in px.iter().enumerate() {
        if rouge(*c) {
            sx += (i % w) as f32 + 0.5;
            sy += (i / w) as f32 + 0.5;
            n += 1;
        }
    }
    (n > 0).then(|| (pos2(sx / n as f32, sy / n as f32), n))
}

fn app_avec(nom: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("feanor-accroche-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

#[test]
fn la_pose_accrochee_est_celle_que_filme_la_camera() {
    let doc = doc_carre();
    let cam = camera_ailleurs(&doc);
    let seen = CameraSeen::de(&cam, doc.stage_center(), doc.frame);
    // Sous la caméra par défaut, rien ne change.
    let repos = CameraSeen::de(&doc.default_camera(), doc.stage_center(), doc.frame);
    let t = LayerTransform { x: 12.0, y: -7.0, scale: 1.5, angle: 33.0, stretch_x: 0.8, ..Default::default() };
    let r = repos.accrochee(t);
    assert!((r.x - t.x).abs() < 1e-4 && (r.y - t.y).abs() < 1e-4 && (r.scale - t.scale).abs() < 1e-5 && (r.angle - t.angle).abs() < 1e-4, "caméra par défaut : la pose telle quelle, {r:?}");
    // Sous une autre caméra : un point du dessin tombe là où cette caméra filme le point de l'image qu'il occupait.
    for q in [pos2(44.0, 34.0), pos2(0.0, 0.0), pos2(180.0, 100.0)] {
        for pose in [LayerTransform::default(), t] {
            let dans_le_cadre = pose.affine(doc.stage_center()).apply(q);
            // (Le cadre de la caméra par défaut couvre la feuille : un point de la feuille est le même point de l'image.)
            let attendu = cam.to_stage(doc.frame, dans_le_cadre.x, dans_le_cadre.y);
            let vu = seen.accrochee(pose).affine(doc.stage_center()).apply(q);
            assert!(vu.distance(attendu) < 0.02, "{q:?} : vu {vu:?}, attendu {attendu:?}");
        }
    }
    // Et l'inverse rend la pose.
    let retour = seen.decrochee(seen.accrochee(t));
    assert!((retour.x - t.x).abs() < 1e-3 && (retour.y - t.y).abs() < 1e-3 && (retour.scale - t.scale).abs() < 1e-5 && (retour.angle - t.angle).abs() < 1e-4, "{retour:?}");
    assert_eq!((retour.stretch_x, retour.stretch_y), (t.stretch_x, t.stretch_y), "l'étirement n'est pas touché");
}

#[test]
fn un_calque_accroche_garde_sa_place_dans_l_image_et_suit_le_cadre_sur_la_feuille() {
    let mut doc = doc_carre();
    let c = doc.default_camera();
    // Caméra décalée de (30, 10) et deux fois plus serrée.
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x + 30.0, y: c.y + 10.0, zoom: 2.0, ..c })];
    let cam = doc.panel_camera(0, 0);
    let (w, h) = (doc.width, doc.height);
    // Pas accroché : le carré est à sa place sur la feuille, et la caméra le filme ailleurs dans l'image.
    let feuille = render::composite_full(&doc, &render::panel_plan_at(&doc, 0, 0, None));
    let (centre, n) = tache(&feuille, w).expect("le carré");
    assert!(proche(centre, pos2(44.0, 34.0)) && n == 64, "sur la feuille : {centre:?}, {n} px");
    // Accroché : sur la feuille, il est dans le cadre de la caméra, deux fois plus petit…
    doc.panels[0].layers[0].accroche = true;
    let feuille = render::composite_full(&doc, &render::panel_plan_at(&doc, 0, 0, None));
    let (centre, n) = tache(&feuille, w).expect("le carré accroché");
    let attendu = pos2(c.x + 30.0 + (44.0 - c.x) / 2.0, c.y + 10.0 + (34.0 - c.y) / 2.0);
    assert!(centre.distance(attendu) < 0.6, "sur la feuille : {centre:?}, attendu {attendu:?}");
    assert!((12..=20).contains(&n), "deux fois plus petit sur la feuille : {n} px");
    // …et dans l'image filmée, il est où il a été dessiné, à sa taille.
    let image = render::thumbnail(&doc, &render::panel_plan_at(&doc, 0, 0, None), &cam, w, h);
    let (centre, n) = tache(&image.pixels, w).expect("le carré dans l'image");
    assert!(centre.distance(pos2(44.0, 34.0)) < 0.6, "dans l'image : {centre:?}");
    assert!((56..=72).contains(&n), "à sa taille dans l'image : {n} px");
    // La caméra tourne aussi : même place dans l'image.
    doc.panels[0].camera = vec![CameraKey::new(0, camera_ailleurs(&doc))];
    let cam = doc.panel_camera(0, 0);
    let image = render::thumbnail(&doc, &render::panel_plan_at(&doc, 0, 0, None), &cam, w, h);
    let (centre, n) = tache(&image.pixels, w).expect("le carré dans l'image de la caméra tournée");
    assert!(centre.distance(pos2(44.0, 34.0)) < 0.8, "caméra tournée, zoomée, avancée : {centre:?}");
    assert!((50..=80).contains(&n), "{n} px");
}

#[test]
fn accrocher_ne_deplace_rien_a_l_image_courante_puis_le_calque_suit_la_camera() {
    let mut doc = doc_carre();
    let ailleurs = camera_ailleurs(&doc);
    doc.panels[0].camera = vec![CameraKey::new(0, doc.default_camera()), CameraKey::new(5, ailleurs)];
    let mut app = app_avec("suivre", doc);
    let q = pos2(44.0, 34.0);
    // Avant : le calque est fixe sur la feuille, la caméra le filme à des endroits différents de l'image.
    assert!(proche(sur_la_feuille(&app.doc, 0, 0, q), sur_la_feuille(&app.doc, 5, 0, q)));
    assert!(!proche(dans_l_image(&app.doc, 0, 0, q), dans_l_image(&app.doc, 5, 0, q)));
    // On l'accroche à l'image 3 (la caméra est en route) : il ne bouge pas à cette image.
    app.cur_frame = 3;
    let avant = sur_la_feuille(&app.doc, 3, 0, q);
    app.run_action("calque.accrocher_camera");
    assert!(app.layers()[0].accroche, "{}", app.status);
    assert!(app.status.contains("accroché à la caméra"), "{}", app.status);
    assert!(proche(sur_la_feuille(&app.doc, 3, 0, q), avant), "rien ne bouge à l'image où on l'accroche");
    // Il garde cette place dans l'image à toutes les images, et suit donc le cadre sur la feuille.
    let place = dans_l_image(&app.doc, 3, 0, q);
    for f in 0..6 {
        assert!(proche(dans_l_image(&app.doc, f, 0, q), place), "image {f} : {:?} au lieu de {place:?}", dans_l_image(&app.doc, f, 0, q));
    }
    assert!(!proche(sur_la_feuille(&app.doc, 0, 0, q), sur_la_feuille(&app.doc, 5, 0, q)), "sur la feuille il suit le cadre");
    // Une seule étape d'annulation.
    app.undo();
    assert!(!app.layers()[0].accroche && app.layers()[0].transform.is_identity(), "annulé : comme avant");
    app.redo();
    assert!(app.layers()[0].accroche);
    // On le décroche à l'image 5 : il reste là où il est à cette image, et ne suit plus la caméra.
    app.cur_frame = 5;
    let avant = sur_la_feuille(&app.doc, 5, 0, q);
    app.run_action("calque.accrocher_camera");
    assert!(!app.layers()[0].accroche, "{}", app.status);
    assert!(app.status.contains("décroché"), "{}", app.status);
    assert!(proche(sur_la_feuille(&app.doc, 5, 0, q), avant), "rien ne bouge à l'image où on le décroche");
    assert!(proche(sur_la_feuille(&app.doc, 0, 0, q), avant), "décroché : il est fixe sur la feuille");
}

#[test]
fn un_calque_anime_garde_son_mouvement_quand_on_l_accroche() {
    let mut doc = doc_carre();
    doc.panels[0].camera = vec![CameraKey::new(0, doc.default_camera()), CameraKey::new(5, camera_ailleurs(&doc))];
    // Le calque glisse de 0 à 50 px vers la droite sur le plan.
    let l = &mut doc.panels[0].layers[0];
    l.set_pose(0, LayerTransform::default());
    crate::poskeys::set_key(&mut l.pos_keys, 0, LayerTransform::default());
    crate::poskeys::set_key(&mut l.pos_keys, 5, LayerTransform { x: 50.0, ..Default::default() });
    let mut app = app_avec("anime", doc);
    let q = pos2(44.0, 34.0);
    let avant = sur_la_feuille(&app.doc, 0, 0, q);
    app.run_action("calque.accrocher_camera");
    assert!(app.layers()[0].accroche && app.layers()[0].pos_keys.len() == 2, "{}", app.status);
    assert!(proche(sur_la_feuille(&app.doc, 0, 0, q), avant), "rien ne bouge à l'image courante");
    // Dans l'image, il glisse toujours de 50 px vers la droite (la caméra de l'image 0 est celle par défaut).
    let (a, b) = (dans_l_image(&app.doc, 0, 0, q), dans_l_image(&app.doc, 5, 0, q));
    assert!(proche(b, a + vec2(50.0, 0.0)), "son mouvement reste le sien, dans le cadre : {a:?} → {b:?}");
}

#[test]
fn le_trait_tombe_sous_la_pointe_sur_un_calque_accroche() {
    let mut doc = Document::new(200, 120, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Bitmap)];
    doc.panels[0].length = 4;
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x + 30.0, y: c.y + 10.0, zoom: 2.0, ..c })];
    doc.panels[0].layers[0].accroche = true;
    let mut app = app_avec("pointe", doc);
    app.tool = Tool::Brush;
    app.color = [255, 0, 0];
    // Un trait horizontal sur la feuille, de (90, 50) à (110, 50) : chaque point passe par `to_layer`, comme dans la vue.
    let (a, b) = (pos2(90.0, 50.0), pos2(110.0, 50.0));
    let p = app.to_layer(a);
    assert!(proche(p, pos2(20.0, 20.0)), "le point de la feuille ramené au dessin du calque : {p:?}");
    app.begin_paint(p, 1.0, false);
    for k in 1..=20 {
        let s = a + (b - a) * (k as f32 / 20.0);
        let p = app.to_layer(s);
        app.stroke_to(p, 1.0);
    }
    app.end_stroke();
    // Ce qu'on voit sur la feuille : de l'encre sous la pointe, au milieu du trait, et rien là où le dessin est rangé.
    let feuille = render::composite_full(&app.doc, &render::panel_plan_at(&app.doc, 0, 0, None));
    let (centre, n) = tache(&feuille, app.doc.width).expect("le trait se voit");
    assert!(n > 10 && (centre.x - 100.0).abs() < 3.0 && (centre.y - 50.0).abs() < 2.0, "le trait est sous la pointe : {centre:?} ({n} px)");
}

#[test]
fn l_outil_transformer_deplace_le_calque_accroche_de_ce_que_fait_la_main() {
    let mut doc = doc_carre();
    let c = doc.default_camera();
    // Caméra décalée, deux fois plus serrée et tournée d'un quart de tour : un geste sur la feuille ne s'ajoute pas tel quel à la pose.
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x + 30.0, y: c.y + 10.0, zoom: 2.0, angle: 90.0, ..c })];
    doc.panels[0].layers[0].accroche = true;
    let mut app = app_avec("transformer", doc);
    app.tool = Tool::Transform;
    app.canvas_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let q = pos2(44.0, 34.0);
    let avant = sur_la_feuille(&app.doc, 0, 0, q);
    // On prend le calque par le milieu de sa boîte et on le glisse de 20 px vers la droite de la feuille.
    let boite = app.content_box();
    let prise = sur_la_feuille(&app.doc, 0, 0, boite.center());
    let ecran = app.stage_to_screen(prise);
    app.transform_press(prise, ecran);
    app.transform_drag(prise + vec2(20.0, 0.0), false, false);
    app.finish_layer_drag();
    let apres = sur_la_feuille(&app.doc, 0, 0, q);
    assert!(proche(apres, avant + vec2(20.0, 0.0)), "le calque suit la main sur la feuille : {avant:?} → {apres:?} ({})", app.status);
    // Sa pose, elle, est écrite dans le cadre de la caméra : 20 px de la feuille font 40 px du cadre, tournés d'un quart de tour.
    let t = app.layers()[0].transform;
    assert!((t.x - 0.0).abs() < 0.05 && (t.y + 40.0).abs() < 0.05, "la pose dans le cadre : {t:?}");
    // Une étape d'annulation rend la pose d'avant.
    app.undo();
    assert!(app.layers()[0].transform.is_identity());
}

#[test]
fn l_accroche_s_enregistre_avec_le_projet() {
    let dir = std::env::temp_dir().join(format!("feanor-accroche-fichier-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.shot");
    let mut doc = doc_carre();
    doc.panels[0].layers.push(Layer::new("Décor", LayerKind::Bitmap));
    doc.panels[0].layers[0].accroche = true;
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert_eq!(texte.matches("accroche_camera").count(), 1, "écrit pour le seul calque accroché");
    let relu = crate::project::load(&manifest).unwrap().doc;
    assert!(relu.panels[0].layers[0].accroche && !relu.panels[0].layers[1].accroche);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn les_calques_choisis_s_accrochent_ensemble_et_un_dossier_compte_pour_ses_calques() {
    let mut doc = doc_carre();
    let c = doc.panels[0].layers[0].frames[0];
    let mut dossier = Layer::new_folder("Avant-plan");
    dossier.id = crate::document::fresh_layer_id();
    let mut dedans = Layer::new("Dedans", LayerKind::Bitmap);
    dedans.frames = vec![c; 6];
    dedans.parent = Some(dossier.id);
    let mut autre = Layer::new("Autre", LayerKind::Bitmap);
    autre.frames = vec![c; 6];
    // De bas en haut : Perso, Autre, Dedans (dans le dossier), Avant-plan (le dossier).
    doc.panels[0].layers.extend([autre, dedans, dossier]);
    let mut app = app_avec("ensemble", doc);
    // Deux calques choisis : les deux s'accrochent, en une étape.
    app.cur_layer = 0;
    app.layer_sel = vec![app.layers()[0].id, app.layers()[1].id];
    app.run_action("calque.accrocher_camera");
    assert!(app.layers()[0].accroche && app.layers()[1].accroche && !app.layers()[2].accroche, "{}", app.status);
    assert!(app.status.contains("2 calques"), "{}", app.status);
    app.undo();
    assert!(!app.layers()[0].accroche && !app.layers()[1].accroche, "une seule étape pour les deux");
    // Le dossier : ses calques.
    app.layer_sel.clear();
    app.cur_layer = 3;
    assert!(!app.est_accroche(3));
    app.run_action("calque.accrocher_camera");
    assert!(app.layers()[2].accroche && !app.layers()[3].accroche && !app.layers()[0].accroche, "{}", app.status);
    assert!(app.est_accroche(3), "le dossier se lit accroché quand tous ses calques le sont");
    app.run_action("calque.accrocher_camera");
    assert!(!app.layers()[2].accroche, "et un second appel les décroche : {}", app.status);
}

#[test]
fn ce_qui_ne_s_accroche_pas_le_dit() {
    let mut l = Layer::new("L", LayerKind::Vector);
    assert_eq!(l.raison_sans_accroche(), None);
    l.decor = true;
    assert!(l.raison_sans_accroche().is_some_and(|r| r.contains("Décor")));
    l.decor = false;
    l.transform.tilt_x = 20.0;
    assert!(l.raison_sans_accroche().is_some_and(|r| r.contains("inclinée")));
    assert!(Layer::new_folder("D").raison_sans_accroche().is_some());
    // Dans l'application : le calque Décor est laissé de côté, avec sa raison.
    let mut doc = doc_carre();
    doc.panels[0].layers[0].decor = true;
    let mut app = app_avec("refus", doc);
    let avant = app.layers().to_vec();
    app.run_action("calque.accrocher_camera");
    assert!(!app.layers()[0].accroche);
    assert!(app.status.contains("Décor"), "{}", app.status);
    assert_eq!(app.layers(), &avant[..], "rien n'a changé");
}

#[test]
fn un_calque_accroche_ne_se_fond_pas_dans_un_calque_qui_ne_l_est_pas() {
    let mut doc = doc_carre();
    let c = doc.panels[0].layers[0].frames[0];
    let mut dessus = Layer::new("Dessus", LayerKind::Bitmap);
    dessus.frames = vec![c; 6];
    doc.panels[0].layers.push(dessus);
    assert!(crate::fusioncalques::blocker_down(&doc, 0, 1).is_none());
    doc.panels[0].layers[1].accroche = true;
    let raison = crate::fusioncalques::blocker_down(&doc, 0, 1).expect("refusé");
    assert!(raison.contains("accroché à la caméra") && raison.contains("Dessus") && raison.contains("Perso"), "{raison}");
    // Les deux accrochés : ils se fondent, et le calque obtenu reste accroché.
    doc.panels[0].layers[0].accroche = true;
    assert!(crate::fusioncalques::blocker_down(&doc, 0, 1).is_none());
    doc.merge_layer_down(0, 1).unwrap();
    assert!(doc.panels[0].layers.len() == 1 && doc.panels[0].layers[0].accroche);
}

#[test]
fn le_calque_de_couleur_suit_l_accroche_de_son_trait() {
    let mut doc = doc_carre();
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x + 30.0, zoom: 2.0, ..c })];
    let trait_ = doc.panels[0].layers[0].clone();
    doc.panels[0].layers.insert(0, crate::calquecouleur::nouveau(&trait_, "Couleur".into()));
    crate::calquecouleur::refleter(&mut doc, 0);
    let mut app = app_avec("couleur", doc);
    app.cur_layer = 1;
    app.run_action("calque.accrocher_camera");
    assert!(app.layers()[1].accroche, "{}", app.status);
    assert!(app.layers()[0].accroche, "la couleur est accrochée avec son trait");
    assert_eq!(app.layers()[0].transform, app.layers()[1].transform, "et elle a la même pose");
    // Demandée sur le calque de couleur, la commande agit sur son trait : les deux se décrochent ensemble.
    app.cur_layer = 0;
    app.run_action("calque.accrocher_camera");
    assert!(!app.layers()[0].accroche && !app.layers()[1].accroche, "{}", app.status);
    assert_eq!(app.layers()[0].transform, app.layers()[1].transform);
}

// ------------------------------------------------------------------------------ à regarder

/// Un trait de pinceau qui passe par ces points de la feuille (ramenés au dessin du calque courant, comme dans la vue).
fn tracer(app: &mut App, points: &[(f32, f32)]) {
    let p0 = app.to_layer(pos2(points[0].0, points[0].1));
    app.begin_paint(p0, 1.0, false);
    for w in points.windows(2) {
        for k in 1..=12 {
            let t = k as f32 / 12.0;
            let s = pos2(w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
            let p = app.to_layer(s);
            app.stroke_to(p, 1.0);
        }
    }
    app.end_stroke();
}

/// À regarder (hors de la suite) : un décor fixe, un personnage, une caméra qui glisse vers la droite en zoomant. Quatre images dans
/// `FEANOR_CAPTURE_DIR` : l'image 1, l'image 24 sans accroche (le personnage est resté sur la feuille, hors du cadre), l'image 24 avec le
/// personnage accroché (il a suivi le cadre), et la même vue par la caméra. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn accroche_captures() {
    use crate::perf_tests::Banc;
    use crate::workspace::Espace;
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("ACCROCHE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("ACCROCHE {nom}");
    };
    let mut doc = Document::new(3840, 2160, 24);
    doc.frame = [1920, 1080];
    doc.panels[0].layers = vec![Layer::new("Décor", LayerKind::Bitmap), Layer::new("Perso", LayerKind::Bitmap)];
    doc.panels[0].set_length(24);
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x - 620.0, ..c }), CameraKey::new(23, Camera { x: c.x + 620.0, y: c.y - 60.0, zoom: 1.5, ..c })];
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.set_espace(Espace::Storyboard);
    b.app.show_pane(crate::dock::Pane::Layers);
    b.chauffer(8);
    b.app.tool = Tool::Brush;
    b.app.brush.size = 16.0;
    // Le décor : un sol, des immeubles, un soleil, sur toute la largeur de la feuille.
    (b.app.cur_layer, b.app.cur_frame) = (0, 0);
    b.app.color = [40, 70, 110];
    tracer(&mut b.app, &[(250.0, 1500.0), (3600.0, 1500.0)]);
    for (k, x) in [420.0_f32, 900.0, 1380.0, 1900.0, 2400.0, 2900.0, 3300.0].into_iter().enumerate() {
        let (l, h) = (260.0, 380.0 + 110.0 * (k % 3) as f32);
        tracer(&mut b.app, &[(x, 1500.0), (x, 1500.0 - h), (x + l, 1500.0 - h), (x + l, 1500.0)]);
        tracer(&mut b.app, &[(x + 70.0, 1500.0 - h + 90.0), (x + 190.0, 1500.0 - h + 90.0)]);
    }
    let soleil: Vec<(f32, f32)> = (0..=24).map(|k| (k as f32 / 24.0 * std::f32::consts::TAU).sin_cos()).map(|(s, co)| (3150.0 + 120.0 * co, 640.0 + 120.0 * s)).collect();
    tracer(&mut b.app, &soleil);
    // Le personnage, dessiné à l'image 1 au milieu du cadre de la caméra.
    b.app.cur_layer = 1;
    b.app.color = [200, 40, 40];
    let (px, py) = (c.x - 620.0, 1180.0);
    let tete: Vec<(f32, f32)> = (0..=20).map(|k| (k as f32 / 20.0 * std::f32::consts::TAU).sin_cos()).map(|(s, co)| (px + 60.0 * co, py - 60.0 + 60.0 * s)).collect();
    tracer(&mut b.app, &tete);
    tracer(&mut b.app, &[(px, py), (px, py + 190.0)]);
    tracer(&mut b.app, &[(px - 90.0, py + 60.0), (px, py + 30.0), (px + 90.0, py + 60.0)]);
    tracer(&mut b.app, &[(px - 70.0, py + 320.0), (px, py + 190.0), (px + 70.0, py + 320.0)]);
    // (Chaque dessin est tenu sur tout le plan.)
    for li in 0..2 {
        let cel = b.app.layers()[li].frames.iter().copied().flatten().next().expect("un dessin");
        b.app.layers_mut()[li].frames = vec![Some(cel); 24];
    }
    b.app.renderer.invalidate_all();
    b.chauffer(8);
    sauver(&b, "accroche-01-image-1.png");
    b.app.cur_frame = 23;
    b.chauffer(8);
    sauver(&b, "accroche-02-image-24-sans-accroche.png");
    // On accroche le personnage à l'image 1, puis on va à l'image 24.
    b.app.cur_frame = 0;
    b.chauffer(2);
    b.app.run_action("calque.accrocher_camera");
    println!("ACCROCHE {}", b.app.status);
    assert!(b.app.layers()[1].accroche);
    b.app.cur_frame = 23;
    b.chauffer(8);
    sauver(&b, "accroche-03-image-24-accroche.png");
    b.app.run_action("camera.vue");
    b.chauffer(8);
    sauver(&b, "accroche-04-image-24-vue-par-la-camera.png");
}