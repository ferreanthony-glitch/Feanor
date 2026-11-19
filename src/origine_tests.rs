//! Essais de la partie 182 : **l'origine d'un calque** (`origine.rs`), son vrai pivot. Sans origine, tout est comme avant au bit
//! près ; déplacer l'origine ne déplace jamais le dessin (sur la feuille comme dans l'Espace 3D, carte inclinée comprise) ;
//! entre deux clés de rotation, l'origine reste fixe ; les origines toutes faites se lisent sur la boîte du dessin ; une
//! trajectoire pose l'origine sur son chemin ; le manipulateur de l'Espace 3D tourne autour d'elle ; l'aimant pose l'origine sur
//! le sol, et décoché, laisse la carte où il l'a mise ; tout s'enregistre (format 61) et s'annule.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Vec2};

use crate::app::App;
use crate::camera::{Camera, CameraKey};
use crate::document::{CameraSeen, Document, Layer, LayerKind, LayerTransform};
use crate::espace3d::{Monde, Xf};
use crate::espace3dmath as m;
use crate::origine::{self, Preset};
use crate::poskeys::PosKey;
use crate::trajectoire::{Ancre, Cles, Support, Trajectoire};

const ROUGE: Color32 = Color32::from_rgb(220, 20, 20);

/// Cadre 320 × 240 sur une feuille 640 × 480, un plan de 24 images, un calque bitmap avec un rectangle rouge (100..140 ×
/// 60..140) tenu sur tout le plan.
fn doc_perso() -> Document {
    let mut doc = Document::with_frame(320, 240, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Bitmap)];
    let c = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&c).unwrap().raster;
    for y in 60..140usize {
        for x in 100..140usize {
            let idx = r.tile_index_of(x, y);
            let (ox, oy) = r.tile_origin(idx);
            r.tile_mut(idx)[(y as i64 - oy) as usize * crate::raster::TILE + (x as i64 - ox) as usize] = ROUGE;
        }
    }
    doc.panels[0].layers[0].frames = vec![Some(c); 24];
    doc.panels[0].length = 24;
    doc
}

fn app_avec(nom: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("feanor-origine-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("feanor-origine-fichier-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Des poses variées : décalées, agrandies, étirées (une retournée), tournées.
fn poses() -> Vec<LayerTransform> {
    vec![
        LayerTransform::default(),
        LayerTransform { x: 12.5, y: -7.25, ..Default::default() },
        LayerTransform { x: -30.0, y: 18.0, scale: 1.7, angle: 33.0, ..Default::default() },
        LayerTransform { x: 5.0, y: 41.0, scale: 0.6, angle: -127.5, stretch_x: -1.3, stretch_y: 0.8, ..Default::default() },
        LayerTransform { x: 0.1, y: 0.2, scale: 2.5, angle: 270.0, stretch_x: 1.0, stretch_y: 2.0, dz: 0.3, ..Default::default() },
    ]
}

fn proche_p(a: Pos2, b: Pos2, tol: f32) -> bool {
    a.distance(b) <= tol
}

fn proche3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    m::length(m::sub(a, b)) <= tol
}

/// Deux placements de carte sont le même : leurs quatre coins et leur centre au même endroit du monde.
fn meme_carte(world: &Monde, a: &Xf, b: &Xf, tol: f32) -> bool {
    let (ca, cb) = (world.card_corners(a), world.card_corners(b));
    (0..4).all(|k| proche3(ca[k], cb[k], tol)) && proche3(a.pos, b.pos, tol)
}

/// Deux affinités sont la même, à `tol` près sur quelques points de la feuille.
fn meme_affinite(a: &crate::vector::Affine, b: &crate::vector::Affine, tol: f32) -> bool {
    [pos2(0.0, 0.0), pos2(640.0, 0.0), pos2(0.0, 480.0), pos2(640.0, 480.0), pos2(123.0, 77.0)].iter().all(|&p| proche_p(a.apply(p), b.apply(p), tol))
}

/// `Layer::pose_seen` tel qu'il était avant la partie 182 (sans l'accroche) : la référence de « rien ne change sans origine ».
fn pose_vue_d_avant(l: &Layer, t: LayerTransform, seen: &CameraSeen) -> Option<LayerTransform> {
    let depth = l.depth + t.dz;
    if depth == 0.0 || (seen.offset == Vec2::ZERO && seen.dolly == 0.0) {
        return Some(t);
    }
    let d = depth.max(Layer::MIN_DEPTH);
    let dist = seen.d0 * (1.0 + d) - seen.dolly;
    if dist <= seen.d0 * 0.02 {
        return None;
    }
    let s = (seen.d0 - seen.dolly) / dist;
    let k = s * (1.0 + d);
    Some(LayerTransform { x: t.x * k + seen.offset.x * (1.0 - s), y: t.y * k + seen.offset.y * (1.0 - s), scale: t.scale * k, ..t })
}

fn bits(t: &LayerTransform) -> [u32; 9] {
    [t.x, t.y, t.scale, t.angle, t.stretch_x, t.stretch_y, t.tilt_x, t.tilt_y, t.dz].map(f32::to_bits)
}

fn bits_a(a: &crate::vector::Affine) -> [u32; 6] {
    [a.a, a.b, a.c, a.d, a.tx, a.ty].map(f32::to_bits)
}

// ---------------------------------------------------------------------------------------------- sans origine : comme avant

#[test]
fn sans_origine_l_image_est_celle_d_avant_au_bit_pres() {
    let mut doc = doc_perso();
    let c = doc.stage_center();
    // Une caméra décalée et avancée : le multiplan travaille.
    let cam = Camera { x: c.x + 37.0, y: c.y - 11.0, dolly: 25.0, ..doc.default_camera() };
    doc.panels[0].camera = vec![CameraKey::new(0, cam)];
    let cel = doc.panels[0].layers[0].cel_at(0);
    for (i, t) in poses().into_iter().enumerate() {
        for depth in [0.0, 0.4, -0.3] {
            let l = &mut doc.panels[0].layers[0];
            (l.transform, l.depth, l.origine) = (LayerTransform { dz: 0.0, ..t }, depth, None);
            let seen = doc.camera_seen(0, 0);
            let l = &doc.panels[0].layers[0];
            let vu = l.transform_seen_at(&seen, 0);
            let avant = pose_vue_d_avant(l, l.pose(0), &seen);
            assert_eq!(vu.map(|t| bits(&t)), avant.map(|t| bits(&t)), "pose {i}, profondeur {depth} : la même pose vue, au bit près");
            // L'élément du plan : la même transformation, la même matrice qu'avant.
            let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
            let item = plan.items.iter().find(|it| Some(it.cel) == cel).expect("le dessin est dans le plan");
            let attendue = avant.filter(|t| !t.is_identity());
            assert_eq!(item.transform.map(|t| bits(&t)), attendue.map(|t| bits(&t)), "pose {i}, profondeur {depth}");
            if let (Some(a), Some(t)) = (item.affine(c), attendue) {
                assert_eq!(bits_a(&a), bits_a(&t.affine(c)));
            }
            // Une origine posée au centre de la feuille même : rien ne change non plus.
            let mut l2 = doc.panels[0].layers[0].clone();
            l2.origine = Some([c.x, c.y]);
            assert_eq!(l2.transform_seen_at(&seen, 0).map(|t| bits(&t)), avant.map(|t| bits(&t)));
            assert_eq!(bits(&l2.pose_au_centre(0, c)), bits(&l2.pose(0)));
        }
    }
    // Les cartes de l'Espace 3D : sans origine, exactement `card_xf` et son inverse.
    let world = Monde { libre: true, ..Monde::of(&doc) };
    for t in poses() {
        let t = LayerTransform { tilt_x: 20.0, tilt_y: -35.0, ..t };
        let xf = world.card_xf(&t, 0.4);
        let xo = world.card_xf_o(&t, 0.4, Vec2::ZERO);
        assert_eq!([xf.pos, xf.rot, xf.size], [xo.pos, xo.rot, xo.size]);
        let (a, da) = world.card_from_xf_libre(&t, 0.4, &xf);
        let (b, db) = world.card_from_xf_libre_o(&t, 0.4, &xf, Vec2::ZERO);
        assert_eq!((bits(&a), da.to_bits()), (bits(&b), db.to_bits()));
        let l = Layer::new("Carte", LayerKind::Bitmap);
        assert_eq!(world.card_xf_de(&l, &t, 0.4), xf);
    }
}

// ---------------------------------------------------------------------------------------------- déplacer l'origine

#[test]
fn deplacer_l_origine_garde_l_affinite_de_la_pose_fixe_et_de_chaque_cle() {
    let doc = doc_perso();
    let c = doc.stage_center();
    let world = Monde { libre: true, ..Monde::of(&doc) };
    let mut l = Layer::new("Perso", LayerKind::Bitmap);
    let p = poses();
    l.pos_keys = p.iter().enumerate().map(|(i, t)| PosKey::new(i * 4, *t)).collect();
    l.transform = p[2];
    let avant: Vec<_> = l.pos_keys.iter().map(|k| (k.frame, l.affine_at(k.frame, c))).collect();
    let fixe = l.transform.affine(l.pivot(c));
    for o in [Some([120.0, 140.0]), Some([-50.5, 333.0]), None, Some([200.0, 150.0])] {
        origine::deplacer_origine(&mut l, o, &world);
        assert_eq!(l.origine, o);
        for (f, a) in &avant {
            assert!(meme_affinite(a, &l.affine_at(*f, c), 1e-2), "origine {o:?}, clé de l'image {f} : le dessin n'a pas bougé");
        }
        assert!(meme_affinite(&fixe, &l.transform.affine(l.pivot(c)), 1e-2), "origine {o:?} : la pose fixe non plus");
    }
    // Et la formule : xy2 = xy1 + A·Δ − Δ.
    let t = p[3];
    let d = vec2(30.0, -12.0);
    let t2 = t.origine_deplacee(d);
    let ad = t.lineaire(d);
    assert!((t2.x - (t.x + ad.x - d.x)).abs() < 1e-5 && (t2.y - (t.y + ad.y - d.y)).abs() < 1e-5);
    // Et la pose rapportée au centre de la feuille en est l'inverse exacte.
    let e = vec2(-70.0, 25.0);
    let tc = t.au_centre(e);
    assert!(meme_affinite(&tc.affine(c), &t.affine(c + e), 1e-3));
    let r = tc.a_l_origine(e);
    assert!((r.x - t.x).abs() < 1e-4 && (r.y - t.y).abs() < 1e-4);
}

#[test]
fn deplacer_l_origine_d_une_carte_inclinee_garde_sa_place_dans_l_espace_et_sur_la_feuille() {
    let doc = doc_perso();
    let world = Monde { libre: true, ..Monde::of(&doc) };
    let mut l = Layer::new("Carte", LayerKind::Bitmap);
    l.depth = 0.35;
    l.pos_keys = vec![
        PosKey::new(0, LayerTransform { x: 20.0, y: -15.0, scale: 1.2, angle: 10.0, tilt_x: 35.0, tilt_y: -20.0, ..Default::default() }),
        PosKey::new(8, LayerTransform { x: -40.0, y: 25.0, scale: 0.9, angle: -40.0, stretch_x: 1.4, tilt_x: -15.0, tilt_y: 50.0, dz: 0.5, ..Default::default() }),
        PosKey::new(16, LayerTransform { x: 5.0, y: 5.0, angle: 90.0, tilt_y: 70.0, dz: -0.2, ..Default::default() }),
    ];
    let avant: Vec<Xf> = l.pos_keys.iter().map(|k| world.carte(&l, k.frame)).collect();
    let seen = doc.camera_seen(0, 8);
    let eye = world.eye_seen(&seen);
    let q = pos2(150.0, 90.0);
    let sur_la_feuille = world.card_to_sheet_m(eye, &avant[1].matrix(), q).unwrap();
    for o in [Some([600.0, 40.0]), Some([60.0, 450.0]), None] {
        origine::deplacer_origine(&mut l, o, &world);
        for (k, xf) in l.pos_keys.iter().zip(&avant) {
            let ici = world.carte(&l, k.frame);
            assert!(meme_carte(&world, xf, &ici, 5e-4), "origine {o:?}, clé {} : la carte n'a pas bougé dans l'espace ({xf:?} / {ici:?})", k.frame);
        }
        // Sur la feuille aussi (la carte inclinée y est posée par son maillage, vu de la caméra du plan) : le même point.
        let b = world.card_to_sheet_m(eye, &world.carte(&l, 8).matrix(), q).unwrap();
        assert!(proche_p(sur_la_feuille, b, 2e-2), "origine {o:?} : {sur_la_feuille:?} / {b:?}");
    }
    // L'aller-retour exact du placement, avec une origine.
    let e = vec2(-80.0, 60.0);
    for k in &l.pos_keys {
        let d = Layer::clamp_depth_libre(l.depth + k.t.dz);
        let xf = world.card_xf_o(&k.t, d, e);
        let (t2, d2) = world.card_from_xf_libre_o(&k.t, d, &xf, e);
        assert!((d2 - d).abs() < 1e-5 && (t2.x - k.t.x).abs() < 1e-2 && (t2.y - k.t.y).abs() < 1e-2 && (t2.scale - k.t.scale).abs() < 1e-4, "{:?} / {t2:?}", k.t);
        assert!(meme_carte(&world, &xf, &world.card_xf_o(&t2, d2, e), 1e-4));
    }
}

#[test]
fn entre_deux_cles_de_rotation_l_origine_reste_fixe() {
    let doc = doc_perso();
    let c = doc.stage_center();
    let mut l = Layer::new("Tête", LayerKind::Bitmap);
    l.origine = Some([120.0, 140.0]);
    let t = LayerTransform { x: 15.0, y: -8.0, ..Default::default() };
    l.pos_keys = vec![PosKey::new(0, t), PosKey::new(12, LayerTransform { angle: 90.0, scale: 1.5, ..t })];
    let o = pos2(120.0, 140.0);
    let ou = l.affine_at(0, c).apply(o);
    for f in 0..=12 {
        let a = l.affine_at(f, c).apply(o);
        assert!(proche_p(a, ou, 1e-3), "image {f} : l'origine (le cou) reste à {ou:?}, pas à {a:?}");
    }
    // Sans origine, c'est le centre de la feuille qui reste fixe, et le cou se déplace.
    l.origine = None;
    assert!(l.affine_at(6, c).apply(o).distance(l.affine_at(0, c).apply(o)) > 5.0);
}

// ---------------------------------------------------------------------------------------------- l'outil, les menus

#[test]
fn les_origines_toutes_faites_se_lisent_sur_la_boite_du_dessin_et_ne_bougent_pas_le_dessin() {
    let mut doc = doc_perso();
    doc.panels[0].layers[0].transform = LayerTransform { x: 20.0, y: -10.0, angle: 30.0, scale: 1.3, ..Default::default() };
    let mut app = app_avec("presets", doc);
    let c = app.doc.stage_center();
    let b = app.layer_content_box(0, 0).expect("un dessin");
    let avant = app.layers()[0].affine_at(0, c);
    for p in [Preset::CentreDessin, Preset::BasCentre, Preset::HautGauche, Preset::HautDroite, Preset::BasGauche, Preset::BasDroite] {
        app.origine_preset(p);
        assert_eq!(app.layers()[0].origine, p.dans_la_boite(b), "{}", p.label());
        assert!(meme_affinite(&avant, &app.layers()[0].affine_at(0, c), 1e-2), "{} : le dessin ne bouge pas", p.label());
    }
    assert_eq!(app.layers()[0].origine, Some([b.right_bottom().x, b.right_bottom().y]));
    assert_eq!(Preset::BasCentre.dans_la_boite(b), Some([b.center().x, b.max.y]));
    // « Ici » : le point de la feuille sous le dernier clic, ramené dans le dessin.
    let ici = app.layers()[0].affine_at(0, c).apply(pos2(110.0, 70.0));
    app.origine_ici = Some(ici);
    app.origine_preset(Preset::Ici);
    let o = app.layers()[0].origine.unwrap();
    assert!(proche_p(pos2(o[0], o[1]), pos2(110.0, 70.0), 1e-2), "{o:?}");
    // « Au centre de la feuille » : plus d'origine.
    app.origine_preset(Preset::Feuille);
    assert_eq!(app.layers()[0].origine, None);
    assert!(meme_affinite(&avant, &app.layers()[0].affine_at(0, c), 1e-2));
    // L'aimantation de l'origine tirée avec Maj : le point de la boîte le plus proche.
    assert_eq!(origine::aimanter_a_la_boite(b, b.center_top() + vec2(3.0, 2.0)), b.center_top());
    // Les commandes existent, sans touche.
    for p in Preset::ALL {
        assert!(crate::keymap::action_info(p.action()).is_some_and(|a| a.implemented), "{}", p.action());
        assert_eq!(Preset::de_l_action(p.action()), Some(p));
    }
    assert!(crate::keymap::action_info(origine::AIMANT_ACTION).is_some());
}

#[test]
fn deplacer_l_origine_est_une_seule_etape_qui_s_annule() {
    let mut doc = doc_perso();
    doc.panels[0].layers[0].pos_keys = vec![PosKey::new(0, LayerTransform { x: 3.0, angle: 20.0, ..Default::default() }), PosKey::new(10, LayerTransform { x: 40.0, angle: -60.0, scale: 2.0, ..Default::default() })];
    let mut app = app_avec("annuler", doc);
    let avant = app.layers()[0].clone();
    let etapes = app.history.undo_len();
    assert!(app.poser_origine(0, Some([130.0, 100.0]), "Déplacer l'origine"));
    assert_eq!(app.history.undo_len(), etapes + 1, "une étape");
    assert_eq!(app.layers()[0].origine, Some([130.0, 100.0]));
    assert_ne!(app.layers()[0].pos_keys, avant.pos_keys, "les clés sont recalculées");
    app.undo();
    assert_eq!(app.layers()[0].origine, None);
    assert_eq!(app.layers()[0].pos_keys, avant.pos_keys);
    assert_eq!(app.layers()[0].transform, avant.transform);
}

#[test]
fn la_poignee_ronde_tourne_autour_de_l_origine_et_l_echelle_au_centre_la_garde() {
    use crate::layertools::{stretched_autour, BoxHandle};
    let o = pos2(110.0, 90.0);
    let t0 = LayerTransform { x: 10.0, y: 5.0, angle: 15.0, ..Default::default() };
    // Tourner une pose autour de son origine : seul l'angle change, l'origine reste où elle est sur la feuille.
    let t1 = LayerTransform { angle: t0.angle + 40.0, ..t0 };
    assert!(proche_p(t0.affine(o).apply(o), t1.affine(o).apply(o), 1e-4));
    // Agrandir par un coin, le point fixe au centre : c'est l'origine qui reste en place.
    let b = egui::Rect::from_min_max(pos2(100.0, 60.0), pos2(140.0, 140.0));
    let t2 = stretched_autour(t0, o, b, BoxHandle::Corner(2), pos2(160.0, 180.0), false, Some(o));
    assert!(proche_p(t0.affine(o).apply(o), t2.affine(o).apply(o), 1e-3));
    assert!(t2.stretch_x > 1.0 && t2.stretch_y > 1.0);
}

// ---------------------------------------------------------------------------------------------- la trajectoire

fn chemin_feuille(t: &mut Trajectoire, duree: usize) {
    (t.cles, t.fin, t.ancre) = (Cles::Interpolation, Some(duree - 1), Ancre::Origine);
}

#[test]
fn l_ancrage_a_l_origine_met_l_origine_sur_le_chemin_et_le_dessin_tourne_autour_d_elle() {
    let mut doc = doc_perso();
    doc.panels[0].layers[0].origine = Some([120.0, 140.0]);
    doc.panels[0].layers[0].transform = LayerTransform { x: 7.0, y: 3.0, angle: 10.0, ..Default::default() };
    let mut app = app_avec("ancre", doc);
    let c = app.doc.stage_center();
    let mut t = Trajectoire::nouvelle(&[[50.0, 200.0], [150.0, 120.0], [300.0, 220.0], [350.0, 100.0]]).expect("un chemin");
    chemin_feuille(&mut t, 24);
    for tourne in [false, true] {
        t.tourne = tourne;
        app.doc.panels[0].layers[0].trajectoire = Some(t.clone());
        app.trajectoire_recalculer(0);
        let l = &app.layers()[0];
        assert_eq!(l.pos_keys.len(), 24);
        let mut angles = Vec::new();
        for f in 0..24 {
            let fr = t.fraction_a_l_image(f, 24).unwrap();
            let p = t.point_a(fr);
            let o = l.affine_at(f, c).apply(pos2(120.0, 140.0));
            assert!(proche_p(o, pos2(p[0], p[1]), 1e-2), "tourne {tourne}, image {f} : l'origine {o:?} est sur le chemin {p:?}");
            angles.push(l.pose(f).angle);
        }
        if tourne {
            assert!(angles.iter().any(|a| (a - 10.0).abs() > 5.0), "le dessin tourne avec le chemin, autour de l'origine : {angles:?}");
        } else {
            assert!(angles.iter().all(|a| *a == 10.0));
        }
    }
    // Un ancrage d'avant (le bas du dessin : (120, 140), le rectangle allant de 60 à 140) reste juste avec une origine ailleurs :
    // c'est ce point-là qui suit le chemin, que le dessin tourne ou non.
    app.doc.panels[0].layers[0].origine = Some([105.0, 70.0]);
    let bas = pos2(120.0, 140.0);
    for tourne in [false, true] {
        t.ancre = Ancre::Bas;
        t.tourne = tourne;
        app.doc.panels[0].layers[0].trajectoire = Some(t.clone());
        app.trajectoire_recalculer(0);
        let l = &app.layers()[0];
        for f in [0, 9, 23] {
            let fr = t.fraction_a_l_image(f, 24).unwrap();
            let p = t.point_a(fr);
            let ici = l.affine_at(f, c).apply(bas);
            assert!(proche_p(ici, pos2(p[0], p[1]), 2e-2), "tourne {tourne}, image {f} : le bas du dessin {ici:?} est sur le chemin {p:?}");
        }
    }
}

#[test]
fn deplacer_l_origine_d_un_calque_ancre_a_l_origine_ne_bouge_pas_le_dessin_et_garde_la_trajectoire() {
    let mut doc = doc_perso();
    doc.panels[0].layers[0].origine = Some([120.0, 140.0]);
    let mut app = app_avec("ancre-deplacee", doc);
    let c = app.doc.stage_center();
    let mut t = Trajectoire::nouvelle(&[[50.0, 200.0], [150.0, 120.0], [300.0, 220.0]]).expect("un chemin");
    chemin_feuille(&mut t, 24);
    app.doc.panels[0].layers[0].trajectoire = Some(t);
    app.trajectoire_recalculer(0);
    let avant: Vec<_> = (0..24).map(|f| app.layers()[0].affine_at(f, c)).collect();
    assert!(app.poser_origine(0, Some([100.0, 60.0]), "Déplacer l'origine"));
    for (f, a) in avant.iter().enumerate() {
        assert!(meme_affinite(a, &app.layers()[0].affine_at(f, c), 1e-2), "image {f} : le dessin n'a pas bougé");
    }
    // Le chemin a suivi l'origine : le calcul suivant redonne les mêmes clés.
    app.trajectoire_recalculer(0);
    for (f, a) in avant.iter().enumerate() {
        assert!(meme_affinite(a, &app.layers()[0].affine_at(f, c), 2e-2), "image {f}, après un nouveau calcul : le même dessin");
    }
}

fn sol(world: &Monde) -> Support {
    Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k).unwrap()
}

#[test]
fn dans_l_espace_l_origine_de_la_carte_est_posee_sur_le_chemin() {
    let doc = doc_perso();
    let world = Monde { libre: true, ..Monde::of(&doc) };
    let mut l = Layer::new("Carte", LayerKind::Bitmap);
    l.origine = Some([120.0, 290.0]);
    l.transform = LayerTransform { x: 30.0, y: -10.0, scale: 1.5, tilt_y: 25.0, ..Default::default() };
    l.depth = 0.5;
    let mut t = Trajectoire::nouvelle_dans_l_espace(&[[0.0, 0.0], [0.0, -200.0], [150.0, -200.0]], sol(&world)).unwrap();
    (t.cles, t.fin, t.ancre) = (Cles::Interpolation, Some(11), Ancre::Origine);
    // `assainie` garde l'ancrage à l'origine dans l'espace (et y refuse « le dessin tourne »).
    let mut tourne = t.clone();
    tourne.tourne = true;
    let relue = tourne.assainie().unwrap();
    assert_eq!((relue.ancre, relue.tourne), (Ancre::Origine, false));
    l.pos_keys = crate::trajectoire3d::cles_dans_l_espace(&world, &l, &t, 12);
    assert_eq!(l.pos_keys.len(), 12);
    let e = world.ecart(&l);
    let rot = world.carte(&l, 0).rot;
    for f in 0..12 {
        let xf = world.carte(&l, f);
        let fr = t.fraction_suivie(f, 12).unwrap();
        let sur = t.monde_a(fr).unwrap();
        assert!(proche3(world.origine_monde(&xf, e), sur, 2e-3), "image {f} : l'origine {:?} est sur le chemin {sur:?}", world.origine_monde(&xf, e));
        assert!(proche3(xf.rot, rot, 1e-3), "la carte ne tourne pas avec le chemin dans l'espace");
    }
    // Le déplacement relatif d'avant (`Depart`) : au départ, la carte est à sa place.
    t.ancre = Ancre::Depart;
    l.pos_keys = crate::trajectoire3d::cles_dans_l_espace(&world, &l, &t, 12);
    assert!(proche3(world.carte(&l, 0).pos, world.card_xf_o(&l.transform, 0.5, e).pos, 1e-3));
}

// ---------------------------------------------------------------------------------------------- Espace 3D

#[test]
fn le_pivot_du_manipulateur_est_l_origine_de_la_carte_ou_la_moyenne_des_origines() {
    let mut doc = doc_perso();
    let mut b = Layer::new("B", LayerKind::Bitmap);
    b.transform = LayerTransform { x: -60.0, y: 20.0, tilt_x: 30.0, ..Default::default() };
    b.depth = 0.3;
    b.origine = Some([50.0, 250.0]);
    doc.panels[0].layers[0].origine = Some([120.0, 140.0]);
    doc.panels[0].layers[0].transform = LayerTransform { x: 40.0, angle: 25.0, tilt_y: -20.0, ..Default::default() };
    doc.panels[0].layers.push(b);
    let app = app_avec("pivot", doc);
    let world = app.e3_world();
    let ids: Vec<u64> = app.layers().iter().map(|l| crate::espace3d::card_selection(l.id)).collect();
    let o = |li: usize| world.origine_monde(&world.carte(&app.layers()[li], 0), world.ecart(&app.layers()[li]));
    let un = app.e3_xfs(&ids[..1]);
    assert!(proche3(app.e3_pivot(&un), o(0), 1e-5), "une carte : son origine");
    let deux = app.e3_xfs(&ids);
    assert!(proche3(app.e3_pivot(&deux), m::scale(m::add(o(0), o(1)), 0.5), 1e-5), "deux : la moyenne de leurs origines");
    // Tourner autour du pivot : l'origine d'une carte seule ne bouge pas.
    let tournee = crate::espace3dgizmo::rotate_xfs(&un, app.e3_pivot(&un), [0.0, 1.0, 0.0], 40.0);
    assert!(proche3(world.origine_monde(&tournee[0].1, world.ecart(&app.layers()[0])), o(0), 1e-4));
    // Sans origine, le centre de la boîte des positions, comme avant.
    let mut sans = app_avec("pivot-sans", doc_perso());
    sans.doc.panels[0].layers[0].transform.x = 33.0;
    let x = sans.e3_xfs(&[crate::espace3d::card_selection(sans.layers()[0].id)]);
    assert_eq!(sans.e3_pivot(&x), crate::espace3dgizmo::pivot_of(&x));
}

/// Une carte inclinée, en profondeur, avec une origine et des clés : de quoi éprouver l'aimant.
fn doc_aimant() -> Document {
    let mut doc = doc_perso();
    let l = &mut doc.panels[0].layers[0];
    l.origine = Some([120.0, 140.0]);
    l.depth = 0.2;
    l.pos_keys = vec![
        PosKey::new(0, LayerTransform { x: 10.0, y: -30.0, tilt_y: 30.0, ..Default::default() }),
        PosKey::new(10, LayerTransform { x: 80.0, y: 20.0, angle: 15.0, scale: 1.3, dz: 0.6, ..Default::default() }),
        PosKey::new(20, LayerTransform { x: -50.0, y: 0.0, tilt_x: -20.0, dz: -0.4, ..Default::default() }),
    ];
    l.transform = LayerTransform { x: 5.0, y: 12.0, ..Default::default() };
    doc
}

#[test]
fn aimante_au_sol_l_origine_repose_sur_le_sol_puis_decoche_la_carte_reste_ou_l_aimant_l_a_mise() {
    let mut app = app_avec("aimant", doc_aimant());
    let world = app.e3_world();
    let c = app.doc.stage_center();
    let cles: Vec<usize> = app.layers()[0].pos_keys.iter().map(|k| k.frame).collect();
    let hauteur = |app: &App, f: usize| {
        let l = &app.layers()[0];
        world.origine_monde(&world.carte(l, f), world.ecart(l))[1]
    };
    assert!(cles.iter().all(|&f| (hauteur(&app, f) - world.sol_y()).abs() > 0.05), "au départ, l'origine n'est pas au sol");
    let etapes = app.history.undo_len();
    app.basculer_aimant(0);
    assert!(app.layers()[0].aimante_au_sol);
    assert_eq!(app.history.undo_len(), etapes + 1, "cocher : une étape");
    for &f in &cles {
        assert!((hauteur(&app, f) - world.sol_y()).abs() < 1e-4, "clé {f} : l'origine est au sol ({} / {})", hauteur(&app, f), world.sol_y());
    }
    // La pose fixe aussi (elle sert quand les clés s'en vont).
    let l = &app.layers()[0];
    let d = Layer::clamp_depth_libre(l.depth + l.transform.dz);
    let xf = world.card_xf_o(&l.transform, d, world.ecart(l));
    assert!((world.origine_monde(&xf, world.ecart(l))[1] - world.sol_y()).abs() < 1e-4, "la pose fixe : l'origine au sol");
    // Ce que l'aimant a donné, à chaque clé : l'affinité sur la feuille et le placement dans l'espace.
    let aimantees: Vec<_> = cles.iter().map(|&f| (app.layers()[0].affine_at(f, c), world.carte(&app.layers()[0], f))).collect();
    let fixe = app.layers()[0].transform;
    let pareil = |app: &App, quand: &str| {
        for (&f, (a, xf)) in cles.iter().zip(&aimantees) {
            assert_eq!(app.layers()[0].affine_at(f, c), *a, "{quand}, clé {f} : la même affinité sur la feuille");
            assert_eq!(world.carte(&app.layers()[0], f), *xf, "{quand}, clé {f} : la même carte dans l'espace");
        }
        assert_eq!(app.layers()[0].transform, fixe, "{quand} : la même pose fixe");
    };
    // Décoché : rien ne bouge, maintenant…
    app.basculer_aimant(0);
    assert!(!app.layers()[0].aimante_au_sol);
    assert_eq!(app.history.undo_len(), etapes + 2, "décocher : une étape");
    pareil(&app, "décoché");
    // … ni après ce qui refait l'aimant (il n'est plus coché)…
    app.aimanter_si(0);
    pareil(&app, "après un geste");
    // … ni après l'enregistrement et la relecture.
    let dir = dossier("aimant");
    let manifest = dir.join("A.shot");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    app.doc = crate::project::load(&manifest).unwrap().doc;
    pareil(&app, "relu");
    assert!(!app.layers()[0].aimante_au_sol);
    // Cocher, enregistrer, relire : toujours coché, toujours au sol (rien n'est refait à la lecture).
    app.basculer_aimant(0);
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifest).unwrap().doc;
    assert!(relu.panels[0].layers[0].aimante_au_sol);
    assert_eq!(relu.panels[0].layers[0].pos_keys, app.layers()[0].pos_keys);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn l_aimant_suit_les_gestes_et_le_sol_passe_par_une_seule_fonction() {
    let mut app = app_avec("aimant-geste", doc_aimant());
    let world = app.e3_world();
    app.basculer_aimant(0);
    // Une clé déplacée vers le haut dans l'espace : l'aimant la ramène au sol après le geste.
    app.cur_frame = 10;
    let id = crate::espace3d::card_selection(app.layers()[0].id);
    let mut xf = app.e3_xfs(&[id]);
    xf[0].1.pos[1] += 1.5;
    xf[0].1.pos[0] += 0.7;
    app.e3_set_xfs(&xf);
    let l = &app.layers()[0];
    let o = world.origine_monde(&world.carte(l, 10), world.ecart(l));
    assert!((o[1] - world.sol_y()).abs() < 1e-4, "après le geste, au sol : {o:?}");
    assert!((origine::hauteur_du_sol_sous(&world, o[0], o[2]) - world.sol_y()).abs() < 1e-6);
    // Déplacer l'origine d'une carte aimantée : c'est la nouvelle origine qui repose au sol.
    assert!(app.poser_origine(0, Some([130.0, 60.0]), "Déplacer l'origine"));
    let l = &app.layers()[0];
    let o = world.origine_monde(&world.carte(l, 10), world.ecart(l));
    assert!((o[1] - world.sol_y()).abs() < 1e-4);
}

// ---------------------------------------------------------------------------------------------- le fichier

#[test]
fn l_origine_l_aimant_et_l_ancrage_s_enregistrent_et_un_projet_60_s_ouvre_sans_eux() {
    assert_eq!(crate::project::VERSION, 61);
    let dir = dossier("fichier");
    let manifest = dir.join("O.shot");
    let mut doc = doc_perso();
    doc.panels[0].layers.push(Layer::new("Autre", LayerKind::Bitmap));
    {
        let l = &mut doc.panels[0].layers[0];
        l.origine = Some([123.5, 98.25]);
        l.aimante_au_sol = true;
        let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 40.0]]).unwrap();
        t.ancre = Ancre::Origine;
        l.trajectoire = Some(t);
    }
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert_eq!(texte.matches("aimante_au_sol").count(), 1, "écrit pour le seul calque aimanté");
    assert!(texte.contains("\"Origine\""), "l'ancrage à l'origine est écrit");
    let relu = crate::project::load(&manifest).unwrap().doc;
    let l = &relu.panels[0].layers[0];
    assert_eq!((l.origine, l.aimante_au_sol), (Some([123.5, 98.25]), true));
    assert_eq!(l.trajectoire.as_ref().map(|t| t.ancre), Some(Ancre::Origine));
    let autre = &relu.panels[0].layers[1];
    assert_eq!((autre.origine, autre.aimante_au_sol), (None, false));
    // Un projet du format 60, sans ces champs : il s'ouvre, sans origine ni aimant, l'ancrage d'avant gardé.
    let mut vieux = doc_perso();
    let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 40.0]]).unwrap();
    t.ancre = Ancre::Bas;
    vieux.panels[0].layers[0].trajectoire = Some(t);
    let manifest60 = dir.join("V.shot");
    crate::project::save(&vieux, &manifest60, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest60).unwrap();
    assert!(!texte.contains("aimante_au_sol") && !texte.contains("\"Origine\""));
    assert!(texte.contains(&format!("\"version\": {}", crate::project::VERSION)));
    std::fs::write(&manifest60, texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 60")).unwrap();
    let relu = crate::project::load(&manifest60).expect("un projet du format 60 s'ouvre").doc;
    let l = &relu.panels[0].layers[0];
    assert_eq!((l.origine, l.aimante_au_sol), (None, false));
    assert_eq!(l.trajectoire.as_ref().map(|t| t.ancre), Some(Ancre::Bas));
    // Une trajectoire sans ancrage écrit (le cas d'avant) se lit « là où part le trait ».
    let t: Trajectoire = serde_json::from_str("{\"points\": [[0.0, 0.0], [50.0, 0.0]]}").unwrap();
    assert_eq!(t.ancre, Ancre::Depart);
    // Un projet plus récent est refusé.
    let texte = std::fs::read_to_string(&manifest60).unwrap();
    std::fs::write(&manifest60, texte.replace("\"version\": 60", "\"version\": 62")).unwrap();
    assert!(crate::project::load(&manifest60).is_err());
    // Une origine abîmée redonne le centre de la feuille.
    assert_eq!(origine::assainie(Some([f32::NAN, 3.0])), None);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn un_calque_duplique_ou_partage_garde_son_origine_et_la_couleur_suit_celle_du_trait() {
    let mut doc = doc_perso();
    doc.panels[0].layers[0].origine = Some([110.0, 90.0]);
    let couleur = crate::calquecouleur::nouveau(&doc.panels[0].layers[0], "Couleur".into());
    assert_eq!(couleur.origine, Some([110.0, 90.0]));
    let copie = doc.panels[0].layers[0].clone();
    assert_eq!(copie.origine, Some([110.0, 90.0]));
    // L'exemplaire d'une planche-mère garde la sienne.
    let mut autre = Layer::new("Ailleurs", LayerKind::Bitmap);
    autre.origine = Some([1.0, 2.0]);
    autre.aimante_au_sol = true;
    let accorde = autre.accorde_au_partage(&doc.panels[0].layers[0], 24);
    assert_eq!((accorde.origine, accorde.aimante_au_sol), (Some([1.0, 2.0]), true));
    // L'adaptation à une autre feuille : l'origine suit le dessin.
    let _ = doc;
}

#[test]
fn une_trajectoire_neuve_fait_suivre_le_chemin_a_l_origine_du_calque() {
    let mut b = crate::trajectoireui_tests::banc();
    // (Le banc des essais d'avant propose le déplacement relatif : ici, ce que propose l'application d'origine.)
    b.app.traj.ancre_propose = None;
    b.app.doc.panels[0].layers[0].origine = Some([60.0, 170.0]);
    crate::trajectoireui_tests::tracer_le_chemin(&mut b);
    let l = &b.app.layers()[0];
    let t = l.trajectoire.as_ref().expect("une trajectoire");
    assert_eq!(t.ancre, Ancre::Origine, "« L'origine du calque » d'office");
    assert_eq!(Ancre::CHOIX[0], Ancre::Origine, "et en tête de la liste");
    let c = b.app.doc.stage_center();
    let fr = t.fraction_a_l_image(5, b.app.frame_count()).unwrap();
    let p = t.point_a(fr);
    assert!(proche_p(l.affine_at(5, c).apply(pos2(60.0, 170.0)), pos2(p[0], p[1]), 1e-2), "l'origine est sur le chemin");
}

#[test]
fn une_trajectoire_neuve_sur_un_calque_sans_origine_ne_fait_pas_sauter_le_dessin() {
    let mut b = crate::trajectoireui_tests::banc();
    b.app.traj.ancre_propose = None;
    assert!(b.app.doc.panels[0].layers[0].origine.is_none());
    crate::trajectoireui_tests::tracer_le_chemin(&mut b);
    let t = b.app.layers()[0].trajectoire.as_ref().expect("une trajectoire");
    assert_eq!(t.ancre, Ancre::Depart, "sans origine posée : le chemin part du début du trait, comme avant");
}
