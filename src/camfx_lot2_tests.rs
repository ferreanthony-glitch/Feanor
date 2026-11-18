//! Le lot 2 des préréglages de caméra : la caméra 2D (`docs/INTERFACE.md` partie 171).
//!
//! 1. Les trente préréglages ont chacun une famille, un nom rangé dans le projet et des réglages de départ utilisables.
//! 2. Ceux qui posent peu de clés arrivent où il faut : le panoramique au bout de sa flèche, le zoom vers un point avec la
//!    cible au centre, le dézoom choc et le panoramique de découverte de retour sur le cadre du plan, le recadrage d'un coup,
//!    l'arrêt net à vitesse constante, les paliers qui tiennent, le dépassement qui revient.
//! 3. Une flèche tourne avec le cadre ; les secousses d'avant gardent leur décalage sur la feuille.
//! 4. Les secousses du lot 2 restent dans leurs bornes et finissent calmes.
//! 5. Le format 58 garde la flèche, l'angle et les paliers.
//! 6. Avec l'outil Caméra, la flèche se tire dans le cadre (banc sans écran) : un geste, une étape d'annulation, la caméra du
//!    plan ne bouge pas.

use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2};

use crate::camera::{camera_at, Camera, CameraKey};
use crate::camfx::{self, Famille, Generator, Params, Plan, Poignee, Preset, Vivant, MAX_KEYS};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};

const FRAME: [usize; 2] = [200, 100];
const FPS: u32 = 24;
const DT: f64 = 1.0 / 60.0;

fn centered() -> Camera {
    Camera::centered(200, 100)
}

/// Pose l'effet `p` à l'image `start` d'un plan de `len` images à la caméra `base` (sans clé) ; rend le plan et les clés.
fn run(p: &Params, start: usize, len: usize, base: Camera) -> (Plan, Vec<CameraKey>) {
    let g = Generator::new(p, FPS);
    let b = move |_f: usize| base;
    let plan = camfx::plan(&g, FRAME, start, len, &[], false, &b);
    let mut keys = Vec::new();
    camfx::write(&mut keys, &plan);
    (plan, keys)
}

fn lot2() -> Vec<Preset> {
    Preset::ALL.into_iter().filter(|p| (Preset::DezoomChoc as u8..=Preset::ArretNet as u8).contains(&(*p as u8))).collect()
}

fn proche(a: f32, b: f32, ctx: &str) {
    assert!((a - b).abs() < 1e-3, "{ctx} : {a} au lieu de {b}");
}

#[test]
fn chaque_prereglage_a_sa_famille_son_nom_et_des_reglages_utilisables() {
    assert_eq!(lot2().len(), 22, "les vingt-deux du lot 2 (C1 à C10, C12, C18 à C27 ; C5 en deux)");
    // Les familles se suivent dans `ALL` et le couvrent entier.
    let ranges: Vec<Preset> = Famille::ALL.into_iter().flat_map(|f| f.presets()).collect();
    assert_eq!(ranges, Preset::ALL.to_vec(), "ALL est rangé par famille");
    assert!(Famille::ALL.into_iter().all(|f| f.presets().count() >= 2), "aucune famille vide");
    let mut noms: Vec<&str> = Preset::ALL.iter().map(|p| p.nom()).collect();
    noms.sort_unstable();
    noms.dedup();
    assert_eq!(noms.len(), Preset::ALL.len(), "chaque nom rangé dans le projet est unique");
    for p in Preset::ALL {
        assert_eq!(Preset::du_nom(p.nom()), Some(p));
        let r = Params::new(p, FPS);
        assert_eq!(r.sanitized(FPS), r, "{} : ses réglages de départ sont dans les bornes", p.label());
        let frames = Generator::new(&r, FPS).frames();
        assert_eq!(frames.first(), Some(&0), "{}", p.label());
        assert_eq!(frames.last(), Some(&r.duration), "{}", p.label());
        assert!(frames.windows(2).all(|w| w[0] < w[1]), "{} : des images de clés croissantes {frames:?}", p.label());
        let long = Generator::new(&Params { duration: 20_000, ..r }, FPS).frames().len();
        assert!(long <= MAX_KEYS + 1, "{} : {long} clés", p.label());
        // Une flèche ou une cible : seulement ceux qui en ont une.
        if p.poignee() == Poignee::Aucune {
            assert_eq!(r.vers, [0.0, 0.0], "{}", p.label());
        }
    }
}

#[test]
fn les_courbes_posent_peu_de_cles_et_arrivent_ou_il_faut() {
    let c = centered();
    let w = 200.0;
    let cles = |p: Preset| run(&Params::new(p, FPS), 0, 400, c).1;
    // Le panoramique à plat : deux clés, la dernière au bout de la flèche (0,6 largeur vers la droite), adoucie.
    let k = cles(Preset::PanoPlat);
    assert_eq!(k.len(), 2);
    proche(k[1].cam.x, c.x + 0.6 * w, "pano à plat, x");
    proche(k[1].cam.y, c.y, "pano à plat, y");
    assert!(k[0].ease_out > 0.0 && k[1].ease_in > 0.0, "doux au départ et à l'arrivée");
    // Le zoom vers un point : la cible arrive au centre du cadre, trois fois plus serré.
    let k = cles(Preset::ZoomPoint);
    assert_eq!(k.len(), 2);
    let v = Preset::ZoomPoint.default_vers();
    let cible = c.to_stage(FRAME, 100.0 + v[0] * 200.0, 50.0 + v[1] * 200.0);
    let fin = k[1].cam.to_stage(FRAME, 100.0, 50.0);
    assert!(cible.distance(fin) < 1e-2, "la cible {cible:?} est au centre {fin:?}");
    proche(k[1].cam.zoom, 3.0, "zoom vers un point");
    // Le dézoom choc part serré et revient au cadre du plan ; posé plus loin, une clé calme juste avant.
    let (plan, k) = run(&Params::new(Preset::DezoomChoc, FPS), 10, 400, c);
    proche(k.iter().find(|x| x.frame == 10).unwrap().cam.zoom, 2.6, "dézoom choc au départ");
    assert_eq!(k.last().unwrap().cam, c, "puis le cadre du plan");
    assert_eq!(plan.before.map(|b| b.0), Some(9), "une clé calme juste avant");
    // Le panoramique de découverte part du bout de sa flèche (le ciel) et se pose sur le cadre du plan.
    let k = cles(Preset::PanoDecouverte);
    proche(k[0].cam.y, c.y - 0.45 * w, "découverte, départ");
    assert_eq!(k[1].cam, c, "découverte, arrivée");
    // Le recadrage sec : d'un coup, sur une image.
    let (plan, k) = run(&Params::new(Preset::Recadrage, FPS), 10, 400, c);
    assert_eq!(plan.before, Some((9, c)));
    let a = k.iter().find(|x| x.frame == 10).unwrap();
    proche(a.cam.zoom, 2.0, "recadrage");
    proche(camera_at(&k, 9, c).zoom, 1.0, "juste avant, rien");
    // L'arrêt net : vitesse constante (aucun adouci), puis plus rien.
    let k = cles(Preset::ArretNet);
    assert_eq!((k[0].ease_out, k[1].ease_in), (0.0, 0.0));
    let d = k[1].frame;
    proche(camera_at(&k, d / 2, c).x, (k[0].cam.x + k[1].cam.x) / 2.0, "à mi-chemin, à mi-course");
    proche(camera_at(&k, d + 30, c).x, k[1].cam.x, "figée ensuite");
    // Le tonneau : un tour, qui accélère, et un peu de zoom.
    let k = cles(Preset::Tonneau);
    proche(k[1].cam.angle, 360.0, "tonneau");
    proche(k[1].cam.zoom, 1.25, "tonneau, zoom");
    assert_eq!(k[0].ease_out, k[1].frame as f32, "il accélère jusqu'au bout");
    // Le panoramique en rotation tourne la caméra (en perspective), sans la déplacer ; le cadre penché, le cadre.
    let k = cles(Preset::PanoRotation);
    proche(k[1].cam.pan, 35.0, "panoramique en rotation");
    assert_eq!((k[1].cam.x, k[1].cam.zoom), (c.x, c.zoom));
    proche(cles(Preset::Penche)[1].cam.angle, 14.0, "plan penché");
    // Le travelling avant : la focale ne change pas, la caméra avance, le plan de référence grandit de ×1,6.
    let k = cles(Preset::TravellingAvant);
    proche(k[1].cam.zoom, c.zoom, "travelling, la focale");
    proche(k[1].cam.magnification(FRAME) / c.magnification(FRAME), 1.6, "travelling avant");
    let k = cles(Preset::TravellingArriere);
    proche(k[1].cam.magnification(FRAME) / c.magnification(FRAME), 1.0 / 1.6, "travelling arrière");
    // Ken Burns et le zoom de tension : leurs facteurs.
    proche(cles(Preset::KenBurns)[1].cam.zoom, 1.25, "Ken Burns");
    proche(cles(Preset::ZoomTension)[1].cam.zoom, 1.3, "zoom de tension");
    // Le dépassement va plus loin que la flèche, puis revient.
    let k = cles(Preset::PanoDepassement);
    assert_eq!(k.len(), 3);
    assert!(k[1].cam.x > k[2].cam.x + 1.0, "il dépasse : {} puis {}", k[1].cam.x, k[2].cam.x);
    proche(k[2].cam.x, c.x + 0.5 * w, "dépassement, arrivée");
    // Les paliers : trois à-coups, tenus entre deux, jusqu'au facteur demandé.
    let k = cles(Preset::ZoomPaliers);
    assert_eq!(k.len(), 7, "{:?}", k.iter().map(|x| x.frame).collect::<Vec<_>>());
    let palier = |f: usize| camera_at(&k, f, c).zoom;
    proche(palier(8), palier(18), "le premier palier tient");
    assert!(palier(25) > palier(18) * 1.2, "le second à-coup");
    proche(palier(k.last().unwrap().frame), 2.5, "le dernier palier");
}

#[test]
fn la_fleche_tourne_avec_le_cadre_et_les_secousses_d_avant_gardent_la_feuille() {
    let tourne = Camera { angle: 90.0, ..centered() };
    let k = run(&Params::new(Preset::PanoPlat, FPS), 0, 400, tourne).1;
    // Vers la droite de l'image, c'est vers le bas de la feuille quand le cadre a tourné d'un quart de tour.
    proche(k[1].cam.x, tourne.x, "x");
    proche(k[1].cam.y, tourne.y + 0.6 * 200.0, "y");
    // Le tremblement léger d'avant : les mêmes décalages sur la feuille, cadre tourné ou non.
    let p = Params::new(Preset::LightShake, FPS);
    let droit = run(&p, 0, 400, centered()).1;
    let penche = run(&p, 0, 400, tourne).1;
    for (a, b) in droit.iter().zip(&penche) {
        proche(a.cam.x - centered().x, b.cam.x - tourne.x, "le tremblement ne tourne pas");
    }
}

#[test]
fn les_secousses_du_lot_2_restent_dans_leurs_bornes_et_finissent_calmes() {
    for p in [Preset::Seisme, Preset::PasLourds, Preset::Marche, Preset::Vehicule, Preset::Houle, Preset::Respiration] {
        for intensity in [1.0, 2.5] {
            let r = Params { intensity, ..Params::new(p, FPS) };
            let g = Generator::new(&r, FPS);
            let b = g.bounds();
            for f in 0..=r.duration {
                let s = g.at(f);
                assert!(s.dx.abs() <= b.pos + 1e-5 && s.dy.abs() <= b.pos + 1e-5, "{} image {f} : {s:?} hors de {b:?}", p.label());
                assert!(s.angle.abs() <= b.angle + 1e-4, "{} image {f}", p.label());
                assert!((s.zoom - 1.0).abs() <= b.punch + 1e-5, "{} image {f}", p.label());
            }
            let fin = g.at(r.duration);
            match p {
                Preset::Marche => proche(fin.avance, 1.25, "la marche a avancé"),
                Preset::Vehicule => assert_eq!((fin.dx, fin.dy), (r.vers[0], r.vers[1]), "le véhicule a glissé au bout de sa flèche"),
                _ => assert_eq!((fin.dx, fin.dy, fin.angle, fin.zoom), (0.0, 0.0, 0.0, 1.0), "{} finit calme", p.label()),
            }
            assert!((0..r.duration).any(|f| g.at(f).dx.abs() + g.at(f).dy.abs() + g.at(f).angle.abs() > 1e-4), "{} bouge", p.label());
        }
        if p.a_graine() {
            let un = Generator::new(&Params::new(p, FPS), FPS);
            let deux = Generator::new(&Params { seed: 9, ..Params::new(p, FPS) }, FPS);
            assert!((0..20).any(|f| un.at(f) != deux.at(f)), "{} : une autre graine, une autre secousse", p.label());
        }
    }
    // Les continues partent de zéro ; les pas lourds frappent d'emblée.
    for p in [Preset::Seisme, Preset::Houle, Preset::Respiration, Preset::Marche] {
        let s = Generator::new(&Params::new(p, FPS), FPS).at(0);
        assert!(s.dx.abs() + s.dy.abs() + s.angle.abs() < 1e-6, "{} part calme", p.label());
    }
    assert!(Generator::new(&Params::new(Preset::PasLourds, FPS), FPS).at(0).dy > 0.01 && Preset::PasLourds.is_abrupt());
}

#[test]
fn le_format_58_garde_la_fleche_l_angle_et_les_paliers() {
    let dir = std::env::temp_dir().join(format!("shot-camfx-lot2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("essai.shot");
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].set_length(80);
    let v = |p: Params, start: usize| Vivant { params: p, start }.sanitized(24);
    doc.panels[0].effets_camera = vec![
        v(Params { vers: [-0.3, 0.2], ..Params::new(Preset::PanoPlat, 24) }, 0),
        v(Params { degres: -720.0, ..Params::new(Preset::Tonneau, 24) }, 10),
        v(Params { paliers: 5, ..Params::new(Preset::ZoomPaliers, 24) }, 20),
        v(Params::new(Preset::Seisme, 24), 30),
        v(Params::new(Preset::ZoomIn, 24), 40),
    ];
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"version\": 59"));
    for bout in ["\"pano_plat\"", "\"tonneau\"", "\"zoom_paliers\"", "\"seisme\"", "\"vers\"", "\"degres\": -720", "\"paliers\": 5"] {
        assert!(texte.contains(bout), "{bout} dans le projet");
    }
    let lu = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(lu.panels[0].effets_camera, doc.panels[0].effets_camera, "les cinq effets reviennent tels quels");
    let cams = |d: &Document| (0..80).map(|f| d.panel_camera(0, f)).collect::<Vec<_>>();
    assert_eq!(cams(&lu), cams(&doc), "la même image");
    // Un effet d'avant n'écrit aucun des trois nouveaux réglages : un projet sans eux se relit comme avant.
    let mut vieux = Document::with_frame(64, 48, 24);
    vieux.panels[0].set_length(40);
    vieux.panels[0].effets_camera = vec![v(Params::new(Preset::ZoomIn, 24), 0)];
    crate::project::save(&vieux, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(!texte.contains("\"vers\"") && !texte.contains("\"degres\"") && !texte.contains("\"paliers\""), "rien de neuf pour un zoom avant");
    std::fs::remove_dir_all(&dir).ok();
}

fn bouton(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn glisser(b: &mut Banc, de: Pos2, vers: Pos2) {
    b.image(vec![Event::PointerMoved(de)], DT);
    b.image(vec![bouton(de, true)], DT);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 8.0))], DT);
    }
    b.image(vec![bouton(vers, false)], DT);
    b.chauffer(3);
}

/// Le bout de la flèche (ou la cible) de l'effet 0 du plan 0, à l'écran.
fn bout_a_l_ecran(b: &Banc) -> Pos2 {
    let v = b.app.doc.panels[0].effets_camera[0];
    let cam = b.app.doc.camera_avant_l_effet(0, 0, v.start);
    let [fw, fh] = b.app.doc.frame;
    let bout = cam.to_stage(b.app.doc.frame, fw as f32 / 2.0 + v.params.vers[0] * fw as f32, fh as f32 / 2.0 + v.params.vers[1] * fw as f32);
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), bout)
}

#[test]
fn avec_l_outil_camera_la_fleche_se_tire_dans_le_cadre() {
    let mut doc = crate::export_tests::doc_with_drawing(1, 48);
    doc.panels[0].effets_camera = vec![Vivant { params: Params { vers: [0.3, 0.0], ..Params::new(Preset::PanoPlat, 12) }, start: 0 }.sanitized(12)];
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.tool = crate::app::Tool::Camera;
    b.app.cur_frame = 0;
    b.chauffer(6);
    let bout = bout_a_l_ecran(&b);
    assert_eq!(b.app.camera_handle_at(bout), Some(crate::camtools::CamHandle::Effet), "le bout de la flèche s'attrape");
    assert_eq!(b.app.camera_handle_at(bout + vec2(0.0, 60.0)), Some(crate::camtools::CamHandle::Move), "à côté, le cadre se déplace comme avant");
    let etapes = b.app.history.undo_len();
    glisser(&mut b, bout, bout + vec2(150.0, 0.0));
    let v = b.app.doc.panels[0].effets_camera[0];
    assert!(v.params.vers[0] > 0.35 && v.params.vers[1].abs() < 1e-3, "la flèche s'allonge vers la droite : {:?} ({})", v.params.vers, b.app.status);
    assert!(b.app.doc.panels[0].camera.is_empty(), "la caméra du plan ne bouge pas : aucune clé");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape d'annulation");
    // Le bout a suivi le pointeur.
    assert!(bout_a_l_ecran(&b).distance(bout + vec2(150.0, 0.0)) < 3.0, "le bout est sous le pointeur");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].effets_camera[0].params.vers, [0.3, 0.0], "Ctrl+Z rend la flèche d'avant");
    // Un effet sans flèche (un tremblement) : rien à attraper, le cadre comme avant.
    b.app.doc.panels[0].effets_camera = vec![Vivant { params: Params::new(Preset::LightShake, 12), start: 0 }.sanitized(12)];
    b.chauffer(2);
    assert_ne!(b.app.camera_handle_at(bout), Some(crate::camtools::CamHandle::Effet));
}

/// Images de contrôle (partie 171), dans `FEANOR_CAPTURE_DIR` : le panneau Préréglages par famille, une vignette de travelling
/// survolée ; la flèche d'un panoramique et la cible d'un zoom dans le cadre, avec l'outil Caméra. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn camera_2d_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("LOT2 pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("LOT2 {nom}");
    };
    let mut doc = crate::export_tests::doc_with_drawing(1, 72);
    doc.panels[0].effets_camera = vec![Vivant { params: Params::new(Preset::PanoPlat, 24), start: 0 }.sanitized(24)];
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 14.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    b.app.tool = crate::app::Tool::Camera;
    b.garder_formes = true;
    b.chauffer(8);
    // Survol d'une vignette de travelling avant.
    let mut textes = Vec::new();
    fn chercher(s: &eframe::egui::epaint::Shape, out: &mut Vec<(String, eframe::egui::Rect)>) {
        match s {
            eframe::egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            eframe::egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    for s in &b.formes {
        chercher(&s.shape, &mut textes);
    }
    if let Some((_, r)) = textes.iter().find(|(t, _)| t == Preset::TravellingAvant.label()) {
        b.image(vec![Event::PointerMoved(pos2(r.left() + 60.0, r.center().y - 46.0))], DT);
        for _ in 0..30 {
            b.image(Vec::new(), DT);
        }
    }
    sauver(&b, "lot2-1-panneau-et-fleche.png");
    // La cible d'un zoom vers un point.
    b.app.doc.panels[0].effets_camera = vec![Vivant { params: Params::new(Preset::ZoomPoint, 24), start: 0 }.sanitized(24)];
    b.app.cur_frame = 30;
    b.chauffer(4);
    sauver(&b, "lot2-2-cible.png");
}
