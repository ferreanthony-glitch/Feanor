//! Retours d'Anthony du 8 novembre 2026 sur la caméra (`docs/INTERFACE.md` partie 129).
//!
//! 1. « Je ne peux pas pousser la clé de caméra vers la droite » (capture 114) : une clé suivie d'une clé posée sur la
//!    dernière image du plan. Les essais passent par la vraie fenêtre sans écran (`Banc`) : on retrouve les losanges
//!    peints, puis on les tire au pointeur, dans la timeline et dans le Montage.
//! 2. Les effets de caméra se voient tout de suite, sans clé ; « Convertir en clés » les cuit, à l'image près.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::camera::{Camera, CameraKey};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};

const DT: f64 = 1.0 / 60.0;

fn cam(x: f32) -> Camera {
    Camera { x, ..Camera::centered(64, 48) }
}

fn cles(frames: &[usize]) -> Vec<CameraKey> {
    frames.iter().map(|&f| CameraKey::new(f, cam(100.0 + f as f32 * 2.0))).collect()
}

fn bouton(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn chercher(shape: &Shape, out: &mut Vec<Pos2>, couleur: Color32) {
    match shape {
        Shape::Path(p) if p.fill == couleur && p.points.len() == 4 => out.push(egui::Rect::from_points(&p.points).center()),
        Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out, couleur)),
        _ => {}
    }
}

/// Les centres des losanges de clés de caméra peints par la dernière image, de gauche à droite.
fn losanges(b: &Banc) -> Vec<Pos2> {
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out, crate::theme::camera());
    }
    out.sort_by(|a, b| a.x.total_cmp(&b.x));
    out.dedup_by(|a, b| (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5);
    out
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

fn images(b: &Banc, pi: usize) -> Vec<usize> {
    b.app.doc.panels[pi].camera.iter().map(|k| k.frame).collect()
}

/// Comme la capture 114 : un plan de 40 images, des clés au début, une clé vers la fin (27) et une clé posée sur la
/// dernière image (39), comme en laisse un Ctrl sur la durée (partie 123) ou la Trajectoire (partie 119).
fn plan_de_la_capture() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].set_length(40);
    doc.panels[0].camera = cles(&[0, 3, 6, 27, 39]);
    doc
}

fn banc_timeline(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 20.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn banc_montage(doc: Document) -> Banc {
    use crate::workspace::Espace;
    let mut doc = doc;
    doc.add_scene_after(0);
    doc.panels[1].set_length(20);
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = 14.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

// ------------------------------------------------------------------------------ 1. la clé qu'on ne pousse pas

#[test]
fn timeline_la_cle_avant_la_cle_de_fin_se_pousse_vers_la_droite() {
    let mut b = banc_timeline(plan_de_la_capture());
    let l = losanges(&b);
    assert_eq!(l.len(), 5, "cinq losanges : {l:?}");
    let cell = (l[1].x - l[0].x) / 3.0;
    let etapes = b.app.history.undo_len();
    // Sans touche (« les clés d'après suivent ») : la clé 27 tirée de 5 images vers la droite.
    glisser(&mut b, l[3], l[3] + vec2(5.0 * cell, 0.0));
    assert_eq!(images(&b, 0), vec![0, 3, 6, 32, 39], "la clé 27 doit aller en 32 ; la clé de la dernière image reste au bout : {}", b.app.status);
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape d'annulation");
    // Tirée bien au-delà de la fin : elle s'arrête juste avant la clé de la dernière image, et la ligne d'état le dit.
    let l = losanges(&b);
    glisser(&mut b, l[3], l[3] + vec2(30.0 * cell, 0.0));
    assert_eq!(images(&b, 0), vec![0, 3, 6, 38, 39]);
    assert!(b.app.status.contains("fin du plan"), "la ligne d'état dit pourquoi elle s'arrête : {}", b.app.status);
    // Annuler rend les clés d'origine, caméra comprise.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].camera, cles(&[0, 3, 6, 27, 39]));
}

#[test]
fn timeline_les_cles_d_apres_se_tassent_contre_la_fin_sans_disparaitre() {
    // Sans clé sur la dernière image, mais deux clés après celle qu'on tire.
    let mut doc = plan_de_la_capture();
    doc.panels[0].camera = cles(&[0, 10, 30, 35]);
    let mut b = banc_timeline(doc);
    let l = losanges(&b);
    let cell = (l[1].x - l[0].x) / 10.0;
    glisser(&mut b, l[1], l[1] + vec2(25.0 * cell, 0.0));
    // 35 + 25 passerait la fin : elle s'arrête à 39, la 30 juste avant (38), la 10 va où on la tire (35).
    assert_eq!(images(&b, 0), vec![0, 35, 38, 39]);
    let k = &b.app.doc.panels[0].camera;
    assert_eq!((k[1].cam.x, k[2].cam.x, k[3].cam.x), (120.0, 160.0, 170.0), "chaque clé garde sa caméra");
}

#[test]
fn timeline_maj_une_cle_passe_par_dessus_une_autre() {
    let mut b = banc_timeline(plan_de_la_capture());
    let l = losanges(&b);
    let cell = (l[1].x - l[0].x) / 3.0;
    b.touches = Modifiers::SHIFT;
    // Maj : seule la clé 6 bouge ; tirée jusqu'à 30, elle passe la clé 27.
    glisser(&mut b, l[2], l[2] + vec2(24.0 * cell, 0.0));
    b.touches = Modifiers::NONE;
    assert_eq!(images(&b, 0), vec![0, 3, 27, 30, 39], "{}", b.app.status);
    let k = &b.app.doc.panels[0].camera;
    assert_eq!(k[3].cam.x, 112.0, "la clé passée devant garde sa caméra");
    // Sur la clé 27 exactement : elle s'arrête à côté, sans l'écraser.
    let l = losanges(&b);
    b.touches = Modifiers::SHIFT;
    glisser(&mut b, l[3], l[2]);
    b.touches = Modifiers::NONE;
    assert_eq!(b.app.doc.panels[0].camera.len(), 5, "aucune clé écrasée : {:?}", images(&b, 0));
}

#[test]
fn montage_la_cle_avant_la_cle_de_fin_se_pousse_vers_la_droite() {
    let mut b = banc_montage(plan_de_la_capture());
    let l = losanges(&b);
    assert!(l.len() >= 5, "{l:?}");
    let ppf = (l[1].x - l[0].x) / 3.0;
    glisser(&mut b, l[3], l[3] + vec2(5.0 * ppf, 0.0));
    assert_eq!(images(&b, 0), vec![0, 3, 6, 32, 39], "{}", b.app.status);
    assert!(b.app.doc.panels[1].camera.is_empty(), "le plan d'après ne reçoit rien");
}

// ------------------------------------------------------------------------------ 2. les effets de caméra vivants

use std::sync::Arc;

use eframe::egui::{RawInput, Rect};

use crate::app::{App, Tool};
use crate::camfx::{Params, Preset, Vivant};
use crate::exportrender::{self, Papier, Rendu};
use crate::render;

/// Une application sans fenêtre, un plan de 48 images avec un dessin (12 i/s), sans clé de caméra.
fn app_dessin(nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-camera-retours-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = crate::export_tests::doc_with_drawing(1, 48);
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.set_mode(crate::app::Mode::Animation);
    app.tool = Tool::Camera;
    app
}

/// Le panneau de l'outil Caméra peint seul, sans fenêtre, pour y cliquer et y glisser.
struct Panneau {
    ctx: egui::Context,
    t: f64,
}

impl Panneau {
    fn image(&mut self, app: &mut App, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(520.0, 1600.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| app.camera_panel(ui));
            });
        });
        fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
                Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        let mut textes = Vec::new();
        out.shapes.iter().for_each(|s| collecter(&s.shape, &mut textes));
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize) -> Vec<(String, Rect)> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.image(app, Vec::new());
        }
        t
    }

    fn cliquer(&mut self, app: &mut App, p: Pos2) {
        self.image(app, vec![Event::PointerMoved(p)]);
        self.image(app, vec![bouton(p, true)]);
        self.image(app, vec![bouton(p, false)]);
        self.chauffer(app, 3);
    }

    fn glisser(&mut self, app: &mut App, de: Pos2, vers: Pos2) {
        self.image(app, vec![Event::PointerMoved(de)]);
        self.image(app, vec![bouton(de, true)]);
        for k in 1..=8 {
            self.image(app, vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 8.0))]);
        }
        self.image(app, vec![bouton(vers, false)]);
        self.chauffer(app, 3);
    }

    /// Le texte `quoi` peint par la dernière image (le premier, de haut en bas).
    fn ou(&mut self, app: &mut App, quoi: &str) -> Rect {
        let textes = self.chauffer(app, 3);
        let mut trouves: Vec<Rect> = textes.iter().filter(|(t, _)| t == quoi).map(|(_, r)| *r).collect();
        trouves.sort_by(|a, b| a.top().total_cmp(&b.top()));
        *trouves.first().unwrap_or_else(|| panic!("« {quoi} » dans le panneau : {:?}", textes.iter().map(|t| &t.0).collect::<Vec<_>>()))
    }
}

/// L'image de l'écran (vue caméra) à l'image `f` : la caméra que montre l'écran, composée par le processeur.
fn a_l_ecran(app: &mut App, f: usize) -> Vec<Color32> {
    app.cur_frame = f;
    let plan = render::panel_plan_at(&app.doc, 0, f, app.doc.render_paper());
    render::camera_image(&app.doc, &plan, &app.camera_montree())
}

fn exportee(app: &App, f: usize) -> Arc<Vec<Color32>> {
    let rendu = Rendu::new(exportrender::snapshot_of(&app.doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default());
    crate::fxcache::avec_privee(256 << 20, || rendu.image(0, f)).expect("image")
}

#[test]
fn choisir_un_effet_dans_le_panneau_le_montre_tout_de_suite_sans_cle() {
    let mut app = app_dessin("choisir");
    let sans: Vec<Vec<Color32>> = (0..48).map(|f| a_l_ecran(&mut app, f)).collect();
    app.cur_frame = 0;
    let mut ecran = Panneau { ctx: egui::Context::default(), t: 0.0 };
    // La section « Durées » est partie (Anthony l'a barrée sur la capture 115).
    let textes = ecran.chauffer(&mut app, 4);
    assert!(!textes.iter().any(|(t, _)| t == "Durées" || t.starts_with("Séquence : ") || t.starts_with("Plan : ")), "plus de section Durées");
    assert!(!textes.iter().any(|(t, _)| t.contains("Appliquer")), "plus de bouton Appliquer");
    // Ouvrir la liste et choisir « Tremblement léger », à la souris.
    let liste = ecran.ou(&mut app, "Aucun effet");
    let etapes = app.history.undo_len();
    ecran.cliquer(&mut app, liste.center());
    let choix = ecran.ou(&mut app, "Tremblement léger");
    ecran.cliquer(&mut app, choix.center());
    let v = app.doc.panels[0].effet_camera.expect("un effet vivant sur le plan");
    assert_eq!((v.params.preset, v.start), (Preset::LightShake, 0));
    assert!(app.doc.panels[0].camera.is_empty(), "aucune clé posée");
    assert_eq!(app.history.undo_len(), etapes + 1, "une étape d'annulation");
    // Il se voit tout de suite : à l'écran (outil Caméra arrêté), en lecture, à l'export.
    let avec: Vec<Vec<Color32>> = (0..48).map(|f| a_l_ecran(&mut app, f)).collect();
    let bougees = (0..48).filter(|&f| avec[f] != sans[f]).count();
    assert!(bougees > 20, "le tremblement se voit sur la plupart des images : {bougees}");
    assert_eq!(avec[0], sans[0], "il part calme");
    app.playing = true;
    assert_eq!(a_l_ecran(&mut app, 10), avec[10], "en lecture");
    app.playing = false;
    app.tool = Tool::Pan;
    for f in [5, 10, 20] {
        assert!(*exportee(&app, f) == a_l_ecran(&mut app, f), "image {f} : l'export montre l'effet de l'écran");
    }
    // Les Vignettes et le Montage lisent la caméra du document : c'est celle de l'écran.
    app.cur_frame = 10;
    assert_eq!(app.doc.panel_camera(0, 10), app.camera_montree());
    assert_ne!(app.doc.panel_camera(0, 10), app.doc.panel_camera_douce(0, 10), "l'effet est dans la caméra montrée, pas dans les clés");
    app.tool = Tool::Camera;
    // Changer l'intensité au glisser : l'image change aussitôt, en une étape d'annulation.
    let avant = a_l_ecran(&mut app, 10);
    let champ = ecran.ou(&mut app, "100 %");
    let etapes = app.history.undo_len();
    ecran.glisser(&mut app, champ.center(), champ.center() + vec2(60.0, 0.0));
    let v2 = app.doc.panels[0].effet_camera.unwrap();
    assert!(v2.params.intensity > 1.2, "intensité montée : {}", v2.params.intensity);
    assert_eq!(app.history.undo_len(), etapes + 1, "un seul pas pour tout le glisser");
    assert_ne!(a_l_ecran(&mut app, 10), avant, "l'image a changé tout de suite");
    assert!(app.doc.panels[0].camera.is_empty(), "toujours aucune clé");
    // La fréquence aussi (à 12 i/s, le tremblement léger part de 6 Hz : la moitié de la cadence au plus).
    let freq0 = app.doc.panels[0].effet_camera.unwrap().params.freq;
    let f = ecran.ou(&mut app, &format!("{freq0:.1} Hz"));
    let apres_intensite = a_l_ecran(&mut app, 10);
    ecran.glisser(&mut app, f.center(), f.center() + vec2(-40.0, 0.0));
    assert!(app.doc.panels[0].effet_camera.unwrap().params.freq < freq0);
    assert_ne!(a_l_ecran(&mut app, 10), apres_intensite);
    // Annuler : la fréquence, puis l'intensité, puis l'effet.
    app.undo();
    assert_eq!(app.doc.panels[0].effet_camera.unwrap().params.freq, freq0);
    app.undo();
    assert_eq!(app.doc.panels[0].effet_camera.unwrap().params.intensity, 1.0);
    app.undo();
    assert!(app.doc.panels[0].effet_camera.is_none());
    assert_eq!(a_l_ecran(&mut app, 10), sans[10]);
    // « Aucun effet » retire l'effet.
    app.redo();
    assert!(app.doc.panels[0].effet_camera.is_some());
    let liste = ecran.ou(&mut app, "Tremblement léger");
    ecran.cliquer(&mut app, liste.center());
    let aucun = ecran.ou(&mut app, "Aucun effet");
    ecran.cliquer(&mut app, aucun.center());
    assert!(app.doc.panels[0].effet_camera.is_none());
}

/// Toutes les caméras montrées du plan 0, image par image.
fn cameras(app: &App) -> Vec<Camera> {
    (0..app.doc.panels[0].frame_count()).map(|f| app.doc.panel_camera(0, f)).collect()
}

#[test]
fn convertir_en_cles_donne_exactement_la_meme_image() {
    for preset in Preset::ALL {
        for (cles_avant, paliers) in [(false, false), (true, false), (true, true)] {
            let mut app = app_dessin("convertir");
            let c = app.doc.default_camera();
            if cles_avant {
                app.doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x - 15.0, ..c }), CameraKey { ease_in: 4.0, ..CameraKey::new(40, Camera { x: c.x + 25.0, zoom: 1.3, ..c }) }];
            }
            app.doc.panels[0].saccade = crate::camera::Saccade { actif: paliers, cadence: 3 };
            app.cur_frame = 5;
            app.pick_camera_effect(Some(preset));
            let vivant = app.doc.panels[0].effet_camera.expect("effet");
            assert_eq!(vivant.start, 5);
            let ctx = format!("{} clés={cles_avant} paliers={paliers}", preset.label());
            let avant = cameras(&app);
            let images_avant: Vec<_> = [6, 12, 30].iter().map(|&f| exportee(&app, f)).collect();
            let etapes = app.history.undo_len();
            app.run_action("camera.effet_appliquer");
            assert!(app.doc.panels[0].effet_camera.is_none(), "{ctx} : l'effet vivant s'en va");
            assert!(app.doc.panels[0].camera.len() >= 2, "{ctx} : des clés");
            assert_eq!(app.history.undo_len(), etapes + 1, "{ctx} : une étape");
            assert_eq!(cameras(&app), avant, "{ctx} : la même caméra à chaque image, au bit près");
            for (i, &f) in [6, 12, 30].iter().enumerate() {
                assert!(exportee(&app, f) == images_avant[i], "{ctx} : image {f} identique");
            }
            // Annuler rend l'effet vivant, sans clé de plus.
            app.undo();
            assert_eq!(app.doc.panels[0].effet_camera, Some(vivant), "{ctx}");
            assert_eq!(app.doc.panels[0].camera.len(), if cles_avant { 2 } else { 0 }, "{ctx}");
        }
    }
}

#[test]
fn un_plan_sans_cle_suivi_d_un_plan_sans_cle_garde_la_continuite_de_l_effet() {
    // Le plan d'après, sans clé, prend la caméra où le plan à effet la laisse : après la cuisson, la même.
    let mut app = app_dessin("continuite");
    app.doc.add_panel_after(0);
    app.doc.panels[1].set_length(12);
    app.cur_frame = 30;
    app.pick_camera_effect(Some(Preset::ZoomIn));
    let suite: Vec<Camera> = (0..12).map(|f| app.doc.panel_camera(1, f)).collect();
    assert!((suite[0].zoom - app.doc.default_camera().zoom).abs() > 0.1, "le plan d'après hérite du zoom de l'effet");
    app.run_action("camera.effet_appliquer");
    assert_eq!((0..12).map(|f| app.doc.panel_camera(1, f)).collect::<Vec<_>>(), suite);
}

#[test]
fn l_effet_vivant_s_enregistre_et_les_anciens_projets_gardent_leurs_cles() {
    let dir = std::env::temp_dir().join(format!("shot-camera-retours-save-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("E.pelure");
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].set_length(30);
    doc.add_scene_after(0);
    doc.panels[1].set_length(30);
    assert_eq!(doc.panels.len(), 2);
    let p = Params { intensity: 1.7, seed: 42, ..Params::new(Preset::Explosion, 24) };
    doc.panels[0].effet_camera = Some(Vivant { params: Params { duration: 14, ..p }, start: 3 }.sanitized(24));
    // Un effet déjà converti en clés (comme avant) sur l'autre plan.
    doc.panels[1].camera = cles(&[0, 1, 2, 3, 4]);
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"version\": 52"), "format 52");
    let lu = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(lu.panels[0].effet_camera, doc.panels[0].effet_camera, "l'effet revient tel quel");
    assert_eq!(lu.panels[1].effet_camera, None);
    assert_eq!((0..30).map(|f| lu.panel_camera(0, f)).collect::<Vec<_>>(), (0..30).map(|f| doc.panel_camera(0, f)).collect::<Vec<_>>());
    // Un projet du format 51 (sans le champ) : ses clés d'effet restent des clés, aucun effet vivant n'apparaît.
    let ancien = texte.replace("\"version\": 52", "\"version\": 51");
    let sans_effet: serde_json::Value = {
        let mut v: serde_json::Value = serde_json::from_str(&ancien).unwrap();
        fn retirer(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::Object(m) => {
                    m.remove("effet_camera");
                    m.values_mut().for_each(retirer);
                }
                serde_json::Value::Array(a) => a.iter_mut().for_each(retirer),
                _ => {}
            }
        }
        retirer(&mut v);
        v
    };
    std::fs::write(&manifest, serde_json::to_string_pretty(&sans_effet).unwrap()).unwrap();
    let vieux = crate::project::load(&manifest).unwrap().doc;
    assert!(vieux.panels.iter().all(|p| p.effet_camera.is_none()));
    assert_eq!(vieux.panels[1].camera, doc.panels[1].camera, "les clés de l'ancien effet sont là, intactes");
    std::fs::remove_dir_all(&dir).ok();
}