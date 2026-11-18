//! Le lot 3 des préréglages de caméra : la caméra dans l'espace (`docs/INTERFACE.md` partie 172).
//!
//! 1. La visée (`camespace`) : le point vu et l'image d'un point se répondent, viser garde un point à sa place dans l'image,
//!    l'orbite aussi.
//! 2. Les quatre préréglages (travelling latéral, orbite, grue, survol) sont rangés dans leur famille et vont dans l'espace.
//! 3. Chacun fait ce qu'il dit : le travelling suit les axes de la caméra, l'orbite et la grue gardent leur sujet à sa place
//!    dans l'image, le survol s'élève, bascule et avance.
//! 4. Un effet du lot 3 libère la caméra du plan : l'image passe par le changement de repère hors des anciennes limites.
//! 5. Le format 59 garde la hauteur, l'avancée et le sujet visé d'une trajectoire de caméra.
//! 6. Une trajectoire toute faite (C17) : posée depuis la tête de lecture, cinq marques, la caméra garde son sujet au centre du
//!    cadre à chaque image, le chemin inverse rend ses clés ; une étape d'annulation ; le miroir retourne le sujet.

use eframe::egui::{pos2, Event};

use crate::camera::{camera_at, ref_distance, Camera, CameraKey};
use crate::camespace;
use crate::camfx::{self, Famille, Generator, Params, Preset, Vivant};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::trajectoire::Cles;
use crate::trajformes::{self, Forme};

const FRAME: [usize; 2] = [200, 100];
const FPS: u32 = 24;

fn centered() -> Camera {
    Camera::centered(200, 100)
}

const LOT3: [Preset; 4] = [Preset::TravellingLateral, Preset::Orbite, Preset::Grue, Preset::Survol];

/// Les clés de l'effet `p` posé à l'image 0 d'un plan assez long, à la caméra `base` (sans clé).
fn cles(p: &Params, base: Camera) -> Vec<CameraKey> {
    let g = Generator::new(p, FPS);
    let b = move |_f: usize| base;
    let plan = camfx::plan(&g, FRAME, 0, p.duration + 10, &[], false, &b);
    let mut keys = Vec::new();
    camfx::write(&mut keys, &plan);
    keys
}

fn pres(a: f32, b: f32, tol: f32, ctx: &str) {
    assert!((a - b).abs() <= tol, "{ctx} : {a} au lieu de {b}");
}

fn pres2(a: [f32; 2], b: [f32; 2], tol: f32, ctx: &str) {
    assert!((a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol, "{ctx} : {a:?} au lieu de {b:?}");
}

#[test]
fn la_visee_et_le_point_vu_se_repondent() {
    let c0 = centered();
    let cams = [
        c0,
        Camera { pan: 30.0, ..c0 },
        Camera { tilt: -20.0, angle: 10.0, zoom: 1.5, dolly: 40.0, ..c0 },
        Camera { pan: -55.0, tilt: 12.0, angle: -25.0, x: c0.x + 30.0, ..c0 },
    ];
    for (i, c) in cams.iter().enumerate() {
        for (u, v) in [(100.0, 50.0), (20.0, 80.0), (170.0, 10.0)] {
            let ctx = format!("caméra {i}, point ({u}, {v})");
            let p = camespace::point_vu(c, FRAME, u, v);
            pres2(camespace::image_de(c, FRAME, p).expect("devant"), [u, v], 0.05, &ctx);
            // L'œil déplacé, viser remet le point à sa place dans l'image.
            let ailleurs = camespace::deplacer(c, FRAME, [30.0, 50.0, -20.0]);
            let vise = camespace::viser(&ailleurs, FRAME, p, u, v).expect("visable");
            pres2(camespace::image_de(&vise, FRAME, p).expect("devant"), [u, v], 0.05, &format!("{ctx} : visé"));
            assert_eq!((vise.x, vise.y, vise.dolly, vise.zoom, vise.angle), (ailleurs.x, ailleurs.y, ailleurs.dolly, ailleurs.zoom, ailleurs.angle), "{ctx} : viser ne fait que tourner la caméra");
            // L'orbite garde le point à sa place.
            let autour = camespace::tourner_autour(c, FRAME, p, 35.0);
            pres2(camespace::image_de(&autour, FRAME, p).expect("devant"), [u, v], 0.05, &format!("{ctx} : orbite"));
        }
    }
    // Le point vu au centre d'une caméra droite est sur la feuille, sous le centre du cadre.
    let p = camespace::point_vu(&c0, FRAME, 100.0, 50.0);
    pres2([p[0], -p[1]], [c0.x, c0.y], 1e-3, "au centre de la feuille");
    pres(p[2], 0.0, 1e-3, "au plan de référence");
}

#[test]
fn les_quatre_du_lot_3_sont_ranges_et_vont_dans_l_espace() {
    assert_eq!(Famille::Travellings.presets().collect::<Vec<_>>(), LOT3.to_vec(), "la famille Travellings");
    for p in Preset::ALL {
        assert_eq!(p.dans_l_espace(), LOT3.contains(&p), "{} : dans l'espace ou non", p.label());
    }
    for p in LOT3 {
        assert_eq!(Preset::du_nom(p.nom()), Some(p));
        let r = Params::new(p, FPS);
        assert_eq!(r.sanitized(FPS), r, "{} : réglages de départ dans les bornes", p.label());
    }
    assert_eq!(Preset::Orbite.poignee(), camfx::Poignee::Sujet);
    assert_eq!(Preset::Grue.poignee(), camfx::Poignee::Sujet);
    assert_eq!(Preset::TravellingLateral.poignee(), camfx::Poignee::Fleche);
    assert!(Preset::Grue.a_hauteur() && Preset::Survol.a_hauteur() && Preset::Survol.a_avancee() && Preset::Orbite.a_degres());
    // Le travelling latéral et le survol posent deux clés ; l'orbite et la grue une par image.
    assert_eq!(Generator::new(&Params::new(Preset::TravellingLateral, FPS), FPS).frames().len(), 2);
    assert_eq!(Generator::new(&Params::new(Preset::Survol, FPS), FPS).frames().len(), 2);
    let d = Params::new(Preset::Orbite, FPS).duration;
    assert_eq!(Generator::new(&Params::new(Preset::Orbite, FPS), FPS).frames().len(), d + 1);
}

#[test]
fn le_travelling_lateral_suit_les_axes_de_la_camera() {
    let d0 = ref_distance(FRAME);
    let p = Params::new(Preset::TravellingLateral, FPS);
    // Caméra droite : le cadre glisse d'une demi-largeur filmée vers la droite.
    let base = centered();
    let k = cles(&p, base);
    let fin = camera_at(&k, p.duration, base);
    pres(fin.x - base.x, 0.5 * 200.0, 1e-2, "une demi-largeur");
    pres(fin.y, base.y, 1e-3, "même hauteur");
    pres(fin.dolly, base.dolly, 1e-3, "même distance");
    // Caméra tournée de 40° : l'œil suit sa droite à elle, la distance au plan change.
    let tournee = Camera { pan: 40.0, ..centered() };
    let k = cles(&p, tournee);
    let fin = camera_at(&k, p.duration, tournee);
    let (e0, e1) = (camespace::oeil(&tournee, d0), camespace::oeil(&fin, d0));
    let largeur = 200.0 / tournee.magnification(FRAME);
    let (droite, _, _) = camespace::axes(&tournee);
    for i in 0..3 {
        pres(e1[i] - e0[i], droite[i] * 0.5 * largeur, 1e-2, &format!("axe {i}"));
    }
    assert_eq!(fin.pan, 40.0, "elle ne tourne pas");
    // À mi-chemin, adoucie : moins de la moitié du chemin au quart du temps.
    let quart = camera_at(&cles(&p, base), p.duration / 4, base);
    assert!(quart.x - base.x < 0.25 * 100.0, "elle part doucement");
}

#[test]
fn l_orbite_et_la_grue_gardent_leur_sujet_a_sa_place() {
    let d0 = ref_distance(FRAME);
    for preset in [Preset::Orbite, Preset::Grue] {
        for base in [centered(), Camera { pan: 15.0, tilt: -8.0, angle: 6.0, zoom: 1.4, ..centered() }] {
            let p = Params { vers: [0.12, -0.06], ..Params::new(preset, FPS) };
            let (u0, v0) = (100.0 + 0.12 * 200.0, 50.0 - 0.06 * 200.0);
            let sujet = camespace::point_vu(&base, FRAME, u0, v0);
            let k = cles(&p, base);
            for f in 0..=p.duration {
                let c = camera_at(&k, f, base);
                let ctx = format!("{} image {f}", preset.label());
                pres2(camespace::image_de(&c, FRAME, sujet).expect("le sujet reste devant"), [u0, v0], 0.1, &ctx);
            }
            let fin = camera_at(&k, p.duration, base);
            let (e0, e1) = (camespace::oeil(&base, d0), camespace::oeil(&fin, d0));
            if preset == Preset::Orbite {
                pres(fin.pan, base.pan - 40.0, 1e-3, "elle a tourné de 40° autour du sujet");
                let r = |e: [f32; 3]| (e[0] - sujet[0]).hypot(e[2] - sujet[2]);
                pres(r(e1), r(e0), 1e-2, "à la même distance du sujet");
                pres(e1[1], e0[1], 1e-3, "à la même hauteur");
            } else {
                let hauteur = 100.0 / base.magnification(FRAME);
                pres(e1[1] - e0[1], 1.5 * hauteur, 1e-2, "montée d'une hauteur et demie de cadre");
                assert!(fin.tilt < base.tilt - 5.0, "elle bascule vers le bas : {}", fin.tilt);
            }
        }
    }
}

#[test]
fn le_survol_s_eleve_bascule_et_avance() {
    let d0 = ref_distance(FRAME);
    let base = centered();
    let p = Params::new(Preset::Survol, FPS);
    let k = cles(&p, base);
    let fin = camera_at(&k, p.duration, base);
    let (e0, e1) = (camespace::oeil(&base, d0), camespace::oeil(&fin, d0));
    pres(e1[1] - e0[1], 0.8 * 100.0, 1e-2, "élevée");
    pres(e0[2] - e1[2], 1.6 * d0, 1e-2, "avancée de 1,6 fois sa distance : elle passe au-dessus des dessins");
    pres(fin.tilt, -20.0, 1e-3, "basculée vers le sol");
    assert!(fin.dolly > d0, "elle a passé le plan de référence (aucune borne)");
}

#[test]
fn un_effet_du_lot_3_libere_la_camera_du_plan() {
    let mut doc = Document::with_frame(200, 100, FPS);
    doc.panels[0].set_length(120);
    let p = Params { degres: 80.0, ..Params::new(Preset::Orbite, FPS) };
    doc.panels[0].effets_camera = vec![Vivant { params: p, start: 0 }.sanitized(FPS)];
    assert!(!doc.panels[0].camera_libre, "rien n'a encore libéré la caméra");
    assert!(doc.panels[0].a_un_effet_dans_l_espace());
    assert!(doc.repere_du_plan(0, 0).is_none(), "au départ, le chemin de toujours");
    let fin = doc.panel_camera_reelle(0, p.duration);
    assert!(!doc.camera_dans_les_anciennes_limites(&fin), "l'orbite de 80° sort des anciennes limites");
    pres(fin.pan, -80.0, 1e-2, "non bridée");
    assert!(doc.repere_du_plan(0, p.duration).is_some(), "là, l'image passe par le changement de repère");
    // Posé dans l'application, l'effet libère la caméra du plan, comme la vue de la scène.
    let mut b = banc_taille(crate::export_tests::doc_with_drawing(1, 48), false, 1280, 720);
    assert!(!b.app.doc.panels[0].camera_libre);
    b.app.ajouter_effet_camera(Preset::Grue, 0);
    assert!(b.app.doc.panels[0].camera_libre, "la grue libère la caméra du plan");
    b.app.run_action("edition.annuler");
    assert!(!b.app.doc.panels[0].camera_libre && b.app.doc.panels[0].effets_camera.is_empty(), "Ctrl+Z rend le plan d'avant");
}

/// Un plan de 120 images avec une trajectoire toute faite posée à l'image 10.
fn banc_avec_forme(forme: Forme) -> Banc {
    let mut doc = crate::export_tests::doc_with_drawing(1, 120);
    doc.panels[0].set_length(120);
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.poser_trajectoire_toute_faite(forme, 10);
    b
}

/// Le sujet visé de la trajectoire de caméra du plan 0, en pixels de la feuille dans l'espace.
fn sujet_de(doc: &Document) -> [f32; 3] {
    let v = doc.panels[0].trajectoire_camera.as_ref().and_then(|t| t.vise).expect("un sujet");
    let m = crate::espace3d::Monde::of(doc);
    [v[0] / m.k + m.stage[0] / 2.0, v[1] / m.k - m.stage[1] / 2.0, v[2] / m.k]
}

#[test]
fn une_trajectoire_toute_faite_garde_son_sujet_au_centre_du_cadre() {
    for forme in Forme::ALL {
        let b = banc_avec_forme(forme);
        let duree = (trajformes::SECONDES * b.app.doc.fps as f32).round() as usize;
        let doc = &b.app.doc;
        let t = doc.panels[0].trajectoire_camera.as_ref().unwrap_or_else(|| panic!("{} : un chemin ({})", forme.label(), b.app.status));
        assert_eq!((t.depart, t.fin), (10, Some(10 + duree)), "{} : de la tête de lecture, {} s", forme.label(), trajformes::SECONDES);
        assert_eq!(t.marques.len(), 5, "cinq marques");
        assert_eq!(t.cles, Cles::Interpolation, "la caméra glisse le long du chemin");
        assert!(t.support.is_some(), "un chemin dans l'espace");
        assert!(doc.panels[0].camera_libre, "la caméra du plan est libre");
        let sujet = sujet_de(doc);
        let [fw, fh] = doc.frame;
        let centre = [fw as f32 / 2.0, fh as f32 / 2.0];
        for f in (0..120).step_by(3) {
            let c = doc.panel_camera_reelle(0, f);
            pres2(camespace::image_de(&c, doc.frame, sujet).expect("devant"), centre, 0.5, &format!("{} image {f}", forme.label()));
            // Le chemin inverse rend la caméra des clés.
            let cles = doc.camera_hors_de_sa_trajectoire(0, c, f);
            let attendue = doc.camera_sans_trajectoire(0, f);
            for (a, e) in [(cles.x, attendue.x), (cles.y, attendue.y), (cles.dolly, attendue.dolly), (cles.pan, attendue.pan), (cles.tilt, attendue.tilt)] {
                pres(a, e, 0.05, &format!("{} image {f} : le chemin inverse", forme.label()));
            }
        }
        // Avant le départ, la caméra des clés ; au bout, ailleurs.
        assert_eq!(doc.panel_camera_reelle(0, 5), doc.camera_sans_trajectoire(0, 5));
        let (c0, c1) = (doc.panel_camera_reelle(0, 10), doc.panel_camera_reelle(0, 10 + duree));
        assert!((c1.x - c0.x).abs() + (c1.dolly - c0.dolly).abs() > 10.0, "{} : la caméra a bougé", forme.label());
        if forme == Forme::Spirale {
            assert!(c1.y < c0.y - 1.0, "la spirale monte");
        }
    }
}

#[test]
fn une_trajectoire_toute_faite_est_une_etape_et_se_retourne() {
    let mut b = banc_avec_forme(Forme::Arc);
    // Une autre forme remplace la première, en une étape.
    let etapes = b.app.history.undo_len();
    b.app.poser_trajectoire_toute_faite(Forme::S, 30);
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert_eq!(b.app.doc.panels[0].trajectoire_camera.as_ref().map(|t| t.depart), Some(30));
    assert!(b.app.status.contains("remplacé"), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels[0].trajectoire_camera.as_ref().map(|t| t.depart), Some(10), "Ctrl+Z rend l'arc");
    // La caméra garde son sujet, ou son orientation.
    b.app.chemin_camera_garde_le_sujet(false);
    assert!(b.app.doc.panels[0].trajectoire_camera.as_ref().unwrap().vise.is_none());
    b.app.chemin_camera_garde_le_sujet(true);
    assert!(b.app.doc.panels[0].trajectoire_camera.as_ref().unwrap().vise.is_some());
    // Le miroir passe le sujet de l'autre côté.
    let mut t = b.app.doc.panels[0].trajectoire_camera.clone().unwrap();
    let v0 = t.vise.unwrap();
    t.miroir_x(1.0);
    let v1 = t.vise.unwrap();
    pres(v1[0], 2.0 - v0[0], 1e-5, "le sujet retourné");
    assert_eq!((v1[1], v1[2]), (v0[1], v0[2]));
    // Un plan d'une image : rien ne bouge, la barre d'état le dit.
    let mut c = banc_taille(crate::export_tests::doc_with_drawing(1, 1), false, 1280, 720);
    c.app.poser_trajectoire_toute_faite(Forme::Arc, 0);
    assert!(c.app.doc.panels[0].trajectoire_camera.is_none(), "{}", c.app.status);
}

#[test]
fn le_format_59_garde_la_hauteur_l_avancee_et_le_sujet_vise() {
    let dir = std::env::temp_dir().join(format!("shot-camfx-lot3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("essai.shot");
    let mut b = banc_avec_forme(Forme::Spirale);
    let v = |p: Params, start: usize| Vivant { params: p, start }.sanitized(24);
    b.app.doc.panels[0].effets_camera = vec![
        v(Params { hauteur: 2.25, vers: [0.1, 0.0], ..Params::new(Preset::Grue, 24) }, 0),
        v(Params { avancee: 3.0, hauteur: -0.5, degres: 12.0, ..Params::new(Preset::Survol, 24) }, 20),
        v(Params::new(Preset::Orbite, 24), 40),
        v(Params::new(Preset::TravellingLateral, 24), 60),
    ];
    let doc = &b.app.doc;
    crate::project::save(doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"version\": 59"));
    for bout in ["\"grue\"", "\"survol\"", "\"orbite\"", "\"travelling_lateral\"", "\"hauteur\": 2.25", "\"avancee\": 3.0", "\"vise\""] {
        assert!(texte.contains(bout), "{bout} dans le projet");
    }
    let lu = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(lu.panels[0].effets_camera, doc.panels[0].effets_camera, "les quatre effets reviennent tels quels");
    assert_eq!(lu.panels[0].trajectoire_camera.as_ref().and_then(|t| t.vise), doc.panels[0].trajectoire_camera.as_ref().and_then(|t| t.vise), "le sujet visé revient");
    assert!(lu.panels[0].camera_libre, "la caméra libre aussi");
    let cams = |d: &Document| (0..120).map(|f| d.panel_camera_reelle(0, f)).collect::<Vec<_>>();
    assert_eq!(cams(&lu), cams(doc), "la même caméra à chaque image");
    // Un effet d'avant n'écrit ni hauteur ni avancée.
    let mut vieux = Document::with_frame(64, 48, 24);
    vieux.panels[0].set_length(40);
    vieux.panels[0].effets_camera = vec![v(Params::new(Preset::ZoomIn, 24), 0)];
    crate::project::save(&vieux, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    // (Le seul effet du projet, de son nom à la fin de son entrée : la feuille a aussi une « hauteur », ailleurs.)
    let i = texte.find("\"zoom_avant\"").expect("l'effet");
    let entree = &texte[i..i + texte[i..].find('}').expect("fin de l'entrée")];
    assert!(!entree.contains("\"hauteur\"") && !entree.contains("\"avancee\"") && !texte.contains("\"vise\""), "rien de neuf pour un zoom avant : {entree}");
    std::fs::remove_dir_all(&dir).ok();
}

/// Images de contrôle (partie 172), dans `FEANOR_CAPTURE_DIR` : le panneau Préréglages avec les travellings et les
/// trajectoires, une vignette d'orbite survolée ; la poignée du sujet d'une grue dans le cadre ; un survol à mi-chemin. Sans
/// carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn camera_espace_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("LOT3 pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("LOT3 {nom}");
    };
    let mut doc = crate::export_tests::doc_with_drawing(1, 120);
    doc.panels[0].effets_camera = vec![Vivant { params: Params::new(Preset::Grue, 24), start: 0 }.sanitized(24)];
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 10.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    b.app.tool = crate::app::Tool::Camera;
    b.app.poser_trajectoire_toute_faite(Forme::Arc, 0);
    b.garder_formes = true;
    b.chauffer(8);
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
    if let Some((_, r)) = textes.iter().find(|(t, _)| t == Preset::Orbite.label()) {
        b.image(vec![Event::PointerMoved(pos2(r.left() + 60.0, r.center().y - 46.0))], 1.0 / 60.0);
        for _ in 0..50 {
            b.image(Vec::new(), 1.0 / 60.0);
        }
    }
    sauver(&b, "lot3-1-panneau-et-sujet.png");
    b.app.cur_frame = 40;
    b.app.doc.panels[0].effets_camera = vec![Vivant { params: Params::new(Preset::Survol, 24), start: 0 }.sanitized(24)];
    b.app.doc.panels[0].trajectoire_camera = None;
    b.chauffer(6);
    sauver(&b, "lot3-2-survol.png");
}
