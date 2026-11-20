//! Les blocs de la piste Caméra : plusieurs effets de caméra vivants par plan (`docs/INTERFACE.md` partie 170).
//!
//! 1. Deux effets s'ajoutent, dans l'ordre de la liste : exactement ce que donneraient deux « Convertir en clés » à la suite.
//! 2. Convertir tous les effets en clés ne change aucune image.
//! 3. Le format 57 garde la liste ; un projet du format 52 à 56 (un seul `effet_camera`) s'ouvre avec la même image.
//! 4. Poser, choisir et retirer un bloc : une étape d'annulation chacun ; Suppr vise le bloc choisi.
//! 5. Dans la vraie timeline (banc sans écran) : le bloc est peint sous les clés, son bord droit tiré l'allonge, son corps
//!    tiré le décale.

use eframe::egui::{epaint::Shape, pos2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::blocscamera::{depart_qui_tient, rangs, BLOC_H};
use crate::camera::{Camera, CameraKey};
use crate::camfx::{self, Params, Preset, Vivant};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};

const DT: f64 = 1.0 / 60.0;

fn effet(preset: Preset, start: usize, duration: usize) -> Vivant {
    Vivant { params: Params { duration, intensity: 2.0, ..Params::new(preset, 24) }, start }.sanitized(24)
}

fn plan(len: usize) -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].set_length(len);
    doc
}

fn cameras(doc: &Document, pi: usize) -> Vec<Camera> {
    (0..doc.panels[pi].frame_count()).map(|f| doc.panel_camera(pi, f)).collect()
}

// ------------------------------------------------------------------------------------------------------- le modèle

#[test]
fn deux_effets_s_ajoutent_comme_deux_conversions_a_la_suite() {
    let mut doc = plan(48);
    let zoom = effet(Preset::ZoomIn, 0, 30);
    let tremble = effet(Preset::LightShake, 10, 30);
    doc.panels[0].effets_camera = vec![zoom, tremble];
    let vues = cameras(&doc, 0);
    // À la main : le zoom cuit dans les clés, puis le tremblement cuit dans le résultat.
    let defaut = doc.default_camera();
    let base0 = |f: usize| doc.panel_camera_douce(0, f);
    let (k1, _) = camfx::cuire(&zoom, 24, doc.frame, 48, &[], &base0).expect("le zoom pose des clés");
    let base1 = |f: usize| crate::camera::camera_at(&k1, f, defaut);
    let (k2, _) = camfx::cuire(&tremble, 24, doc.frame, 48, &k1, &base1).expect("le tremblement pose des clés");
    let attendues: Vec<Camera> = (0..48).map(|f| crate::camera::camera_at(&k2, f, defaut)).collect();
    assert_eq!(vues, attendues, "les deux effets se cuisent l'un après l'autre");
    // Le tremblement tremble autour du zoom : avec les deux, la caméra n'est ni celle du zoom seul ni celle du tremblement seul.
    let mut seul = plan(48);
    seul.panels[0].effets_camera = vec![zoom];
    let zoom_seul = cameras(&seul, 0);
    assert_eq!(vues[5], zoom_seul[5], "avant le tremblement, le zoom seul");
    assert!((12..30).any(|f| vues[f] != zoom_seul[f]), "le tremblement se voit pendant le zoom");
    assert!((12..30).all(|f| (vues[f].zoom - zoom_seul[f].zoom).abs() < 1e-3), "le tremblement ne change pas le zoom");
    assert!(doc.panels[0].camera.is_empty(), "aucune clé écrite dans le plan");
}

#[test]
fn convertir_tous_les_effets_en_cles_ne_change_aucune_image() {
    let mut doc = plan(48);
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 20.0, ..Camera::centered(64, 48) }), CameraKey::new(40, Camera { x: 44.0, ..Camera::centered(64, 48) })];
    doc.panels[0].effets_camera = vec![effet(Preset::CrashZoom, 6, 8), effet(Preset::Shoulder, 0, 47), effet(Preset::Jolt, 30, 9)];
    let avant = cameras(&doc, 0);
    assert!(doc.cuire_effet_vivant(0));
    assert!(doc.panels[0].effets_camera.is_empty(), "les effets s'en vont");
    assert_eq!(cameras(&doc, 0), avant, "les mêmes caméras, image par image");
}

#[test]
fn un_effet_apres_la_fin_du_plan_ne_gene_pas_les_autres() {
    let mut doc = plan(24);
    let dehors = Vivant { start: 200, ..effet(Preset::ZoomOut, 0, 10) };
    doc.panels[0].effets_camera = vec![dehors, effet(Preset::ZoomIn, 2, 12)];
    let mut seul = plan(24);
    seul.panels[0].effets_camera = vec![effet(Preset::ZoomIn, 2, 12)];
    assert_eq!(cameras(&doc, 0), cameras(&seul, 0));
}

#[test]
fn les_rangs_des_blocs_ne_se_chevauchent_pas() {
    let e = [effet(Preset::ZoomIn, 0, 10), effet(Preset::LightShake, 5, 10), effet(Preset::Jolt, 12, 5), effet(Preset::ZoomOut, 30, 4), effet(Preset::Explosion, 3, 30)];
    let (r, n) = rangs(&e);
    assert_eq!(r, vec![0, 1, 0, 0, 2]);
    assert_eq!(n, 3);
    assert_eq!(rangs(&[]).1, 0, "sans bloc, aucun rang");
}

#[test]
fn un_nouvel_effet_recule_pour_tenir_dans_le_plan() {
    assert_eq!(depart_qui_tient(10, 20, 48), 10);
    assert_eq!(depart_qui_tient(40, 20, 48), 28, "recule juste assez");
    assert_eq!(depart_qui_tient(40, 60, 48), 40, "trop long pour le plan : il part où l'on vise, coupé par la fin");
    assert_eq!(depart_qui_tient(90, 5, 48), 43, "au-delà de la fin : ramené dans le plan");
}

// ------------------------------------------------------------------------------------------------------- le fichier

#[test]
fn le_format_57_garde_la_liste_et_un_ancien_effet_seul_se_relit() {
    let dir = std::env::temp_dir().join(format!("shot-blocs-camera-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("essai.shot");
    let mut doc = plan(40);
    doc.panels[0].effets_camera = vec![effet(Preset::ZoomIn, 0, 20), Vivant { params: Params { seed: 77, ..effet(Preset::Explosion, 12, 10).params }, start: 12 }];
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"version\": 61"), "format 61 (partie 182 : l'origine des calques)");
    assert!(texte.contains("\"effets_camera\"") && !texte.contains("\"effet_camera\""), "la liste, plus l'ancien champ");
    let lu = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(lu.panels[0].effets_camera, doc.panels[0].effets_camera, "les deux effets reviennent, dans l'ordre");
    assert_eq!(cameras(&lu, 0), cameras(&doc, 0));
    // Un projet du format 56 : un seul effet, sous l'ancien nom. Il s'ouvre avec ce seul effet, et la même image.
    let mut un = plan(40);
    un.panels[0].effets_camera = vec![effet(Preset::Shoulder, 3, 30)];
    crate::project::save(&un, &manifest, &mut Default::default()).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&manifest).unwrap().trim_start_matches('\u{feff}')).unwrap();
    fn vers_l_ancien(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(serde_json::Value::Array(mut liste)) = m.remove("effets_camera") {
                    m.insert("effet_camera".into(), liste.remove(0));
                }
                if m.get("version").and_then(|x| x.as_u64()) == Some(61) {
                    m.insert("version".into(), 56.into());
                }
                m.values_mut().for_each(vers_l_ancien);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(vers_l_ancien),
            _ => {}
        }
    }
    vers_l_ancien(&mut v);
    let ancien = serde_json::to_string_pretty(&v).unwrap();
    assert!(ancien.contains("\"effet_camera\"") && ancien.contains("\"version\": 56"));
    std::fs::write(&manifest, ancien).unwrap();
    let vieux = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(vieux.panels[0].effets_camera, un.panels[0].effets_camera, "l'ancien effet devient le premier de la liste");
    assert_eq!(cameras(&vieux, 0), cameras(&un, 0), "la même image");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------------- l'application

fn app_plan(len: usize) -> crate::app::App {
    let dir = std::env::temp_dir().join(format!("shot-blocs-app-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = plan(len);
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.set_mode(crate::app::Mode::Animation);
    app
}

#[test]
fn poser_choisir_et_retirer_un_bloc() {
    let mut app = app_plan(48);
    let etapes = app.history.undo_len();
    app.cur_frame = 40;
    app.ajouter_effet_camera_a_la_tete(Preset::ZoomIn);
    let p = &app.doc.panels[0];
    assert_eq!(p.effets_camera.len(), 1);
    let v = p.effets_camera[0];
    assert_eq!(v.params.preset, Preset::ZoomIn);
    assert_eq!(v.start + v.params.duration, 48, "posé à la tête de lecture, reculé pour tenir dans le plan");
    assert_eq!(app.history.undo_len(), etapes + 1, "une étape d'annulation");
    assert_eq!(app.bloc_camera_choisi(), Some(0), "le bloc posé est choisi");
    // Un deuxième, qui s'ajoute au premier ; la section « Effets de caméra » règle le bloc choisi.
    app.ajouter_effet_camera(Preset::LightShake, 0);
    assert_eq!(app.doc.panels[0].effets_camera.len(), 2);
    assert_eq!(app.effet_vivant().map(|v| v.params.preset), Some(Preset::LightShake));
    app.choisir_bloc_camera(Some(0));
    assert_eq!(app.effet_vivant().map(|v| v.params.preset), Some(Preset::ZoomIn));
    app.pick_camera_effect(Some(Preset::CrashZoom));
    assert_eq!(app.doc.panels[0].effets_camera.iter().map(|v| v.params.preset).collect::<Vec<_>>(), vec![Preset::CrashZoom, Preset::LightShake], "seul le bloc choisi change");
    // Suppr vise le bloc choisi (le dernier clic était dans la timeline), et rien d'autre.
    app.key_focus = true;
    let n = app.history.undo_len();
    app.run_action("edition.supprimer");
    assert_eq!(app.doc.panels[0].effets_camera.iter().map(|v| v.params.preset).collect::<Vec<_>>(), vec![Preset::LightShake]);
    assert_eq!(app.history.undo_len(), n + 1);
    assert!(app.doc.panels[0].camera.is_empty(), "aucune clé de caméra touchée");
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels[0].effets_camera.len(), 2, "Ctrl+Z le remet");
    // Un clic dans le dessin rend la main au dessin : Suppr ne vise plus le bloc.
    app.choisir_bloc_camera(Some(1));
    app.key_focus = false;
    assert!(!app.bloc_camera_a_le_focus());
}

#[test]
fn convertir_en_cles_cuit_tous_les_blocs_d_un_coup() {
    let mut app = app_plan(48);
    app.ajouter_effet_camera(Preset::ZoomIn, 0);
    app.ajouter_effet_camera(Preset::LightShake, 8);
    let avant: Vec<Camera> = cameras(&app.doc, 0);
    let n = app.history.undo_len();
    app.run_action("camera.effet_appliquer");
    assert!(app.doc.panels[0].effets_camera.is_empty());
    assert!(!app.doc.panels[0].camera.is_empty());
    assert_eq!(cameras(&app.doc, 0), avant, "la même image");
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels[0].effets_camera.len(), 2, "les deux blocs reviennent");
    assert!(app.doc.panels[0].camera.is_empty());
}

// ------------------------------------------------------------------------------------------------------- la timeline

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

/// Les blocs peints par la dernière image : les rectangles pleins à la couleur de la caméra, de la hauteur d'un rang.
fn blocs_peints(b: &Banc) -> Vec<Rect> {
    fn chercher(s: &Shape, out: &mut Vec<Rect>) {
        match s {
            Shape::Rect(r) if r.fill == crate::theme::camera() && (r.rect.height() - (BLOC_H - 2.0)).abs() < 0.6 => out.push(r.rect),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out.sort_by(|a, b| a.left().total_cmp(&b.left()));
    out
}

#[test]
fn dans_la_timeline_le_bloc_s_allonge_par_son_bord_et_glisse_par_son_corps() {
    let mut doc = plan(60);
    doc.panels[0].effets_camera = vec![effet(Preset::ZoomIn, 10, 20)];
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 20.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    let blocs = blocs_peints(&b);
    assert_eq!(blocs.len(), 1, "un bloc peint sous les clés : {blocs:?}");
    let r = blocs[0];
    let cell = r.width() / 20.0;
    assert!((cell - 20.0).abs() < 0.6, "un bloc de 20 images : {r:?}");
    // Le bord droit tiré de 6 images vers la droite : la durée passe de 20 à 26, le départ reste.
    let etapes = b.app.history.undo_len();
    glisser(&mut b, pos2(r.right() - 2.0, r.center().y), pos2(r.right() - 2.0 + 6.0 * cell, r.center().y));
    let v = b.app.doc.panels[0].effets_camera[0];
    assert_eq!((v.start, v.params.duration), (10, 26), "{}", b.app.status);
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape d'annulation");
    assert_eq!(b.app.bloc_camera_choisi(), Some(0), "le bloc pris est choisi");
    // Le corps tiré de 4 images vers la gauche : le bloc glisse, sa durée reste.
    let r = blocs_peints(&b)[0];
    glisser(&mut b, pos2(r.center().x, r.center().y), pos2(r.center().x - 4.0 * cell, r.center().y));
    let v = b.app.doc.panels[0].effets_camera[0];
    assert_eq!((v.start, v.params.duration), (6, 26));
    // Annuler deux fois rend le bloc d'origine.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    let v = b.app.doc.panels[0].effets_camera[0];
    assert_eq!((v.start, v.params.duration), (10, 20));
}

/// Les textes peints par la dernière image, avec leur place.
pub(crate) fn textes_peints(b: &Banc) -> Vec<(String, Rect)> {
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

#[test]
fn une_vignette_du_panneau_glissee_sur_la_timeline_pose_un_bloc_la_ou_on_la_lache() {
    let mut doc = plan(60);
    doc.panels[0].effets_camera = vec![effet(Preset::ZoomIn, 30, 20)];
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 20.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    // (Partie 194 : sorti en panneau à lui, toute la largeur et toute la hauteur de la colonne, comme l'onglet d'avant.)
    crate::colonne_tests::toute_la_place(&mut b.app, crate::dock::Pane::Prereglages);
    b.garder_formes = true;
    b.chauffer(8);
    let bloc = blocs_peints(&b)[0];
    let cell = bloc.width() / 20.0;
    // (Les lignes du haut du panneau, partie 171 : la famille des zooms vient en premier, les secousses sont plus bas ; partie 195 :
    // une liste de noms sous un seul aperçu, on prend la ligne par son nom.)
    let etiquette = textes_peints(&b).into_iter().find(|(t, _)| t == Preset::ZoomOut.label()).map(|(_, r)| r).expect("la vignette « Zoom arrière » dans le panneau");
    let vignette = etiquette.center();
    // Lâché sur la rangée des clés de caméra, à l'image 5.
    let cible = pos2(bloc.left() - 25.0 * cell + 0.5 * cell, bloc.top() - 10.0);
    let etapes = b.app.history.undo_len();
    glisser(&mut b, vignette, cible);
    let effets = &b.app.doc.panels[0].effets_camera;
    assert_eq!(effets.len(), 2, "un bloc de plus : {}", b.app.status);
    assert_eq!((effets[1].params.preset, effets[1].start), (Preset::ZoomOut, 5));
    assert_eq!(effets[0], effet(Preset::ZoomIn, 30, 20), "l'autre bloc ne bouge pas");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape d'annulation");
    assert_eq!(b.app.bloc_camera_choisi(), Some(1), "le bloc posé est choisi");
    assert!(b.app.doc.panels[0].camera.is_empty(), "aucune clé de caméra");
    // Un clic sur une vignette pose le préréglage à la tête de lecture.
    b.app.cur_frame = 12;
    b.chauffer(2);
    let etiquette = textes_peints(&b).into_iter().find(|(t, _)| t == Preset::CrashZoom.label()).map(|(_, r)| r).expect("la vignette « Zoom choc »");
    let p = etiquette.center();
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, true)], DT);
    b.image(vec![bouton(p, false)], DT);
    b.chauffer(2);
    let effets = &b.app.doc.panels[0].effets_camera;
    assert_eq!(effets.len(), 3);
    assert_eq!((effets[2].params.preset, effets[2].start), (Preset::CrashZoom, 12));
}

/// Images de contrôle (partie 170), dans `FEANOR_CAPTURE_DIR` : le panneau Préréglages ouvert, une vignette survolée qui joue son
/// mouvement, deux blocs sous les clés de caméra (le second choisi) ; puis une vignette glissée au-dessus de la timeline, avec son
/// repère. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn blocs_camera_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("BLOCS pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("BLOCS {nom}");
    };
    let mut doc = crate::export_tests::doc_with_drawing(1, 72);
    doc.panels[0].effets_camera = vec![effet(Preset::ZoomIn, 0, 40), effet(Preset::LightShake, 18, 40)];
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 14.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 30);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    b.garder_formes = true;
    b.chauffer(8);
    b.app.choisir_bloc_camera(Some(1));
    let etiquette = textes_peints(&b).into_iter().find(|(t, _)| t == Preset::CrashZoom.label()).map(|(_, r)| r).expect("la vignette « Zoom choc »");
    let survol = etiquette.center();
    b.image(vec![Event::PointerMoved(survol)], DT);
    for _ in 0..14 {
        b.image(Vec::new(), DT);
    }
    sauver(&b, "blocs-1-panneau-et-blocs.png");
    // Une vignette prise et glissée au-dessus de la rangée des clés, sans la lâcher.
    let bloc = blocs_peints(&b)[0];
    let cell = bloc.width() / 40.0;
    let etiquette = textes_peints(&b).into_iter().find(|(t, _)| t == Preset::ZoomOut.label()).map(|(_, r)| r).expect("la vignette « Zoom arrière »");
    let de = etiquette.center();
    let vers = pos2(bloc.left() + 44.0 * cell, bloc.top() - 10.0);
    b.image(vec![Event::PointerMoved(de)], DT);
    b.image(vec![bouton(de, true)], DT);
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 10.0))], DT);
    }
    b.image(Vec::new(), DT);
    sauver(&b, "blocs-2-glisser-sur-la-timeline.png");
    b.image(vec![bouton(vers, false)], DT);
    b.chauffer(3);
    sauver(&b, "blocs-3-pose.png");
    println!("BLOCS effets : {:?}", b.app.doc.panels[0].effets_camera.iter().map(|v| (v.params.preset.label(), v.start)).collect::<Vec<_>>());
}
