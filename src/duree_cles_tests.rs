//! Durée d'un plan et clés (Anthony, 2026-11-07, partie 121 de `docs/INTERFACE.md`) : la poignée latérale d'un plan change sa
//! durée ; sans touche seule la durée change, avec Ctrl tenue pendant le geste toutes les clés suivent la poignée du même nombre
//! d'images. Et les clés de dessin d'une piste de calque alternent de couleur (`cles_alternees.rs`).
//!
//! Les essais passent par la vraie fenêtre sans écran (`Banc`) : le montage (bords des blocs de plans, `BLOCS_DU_MONTAGE`) et la
//! timeline (la prise au bout de la règle, `PRISE_FIN_DU_PLAN`) reçoivent de vrais gestes du pointeur, Ctrl comprise.

use std::collections::BTreeSet;

use eframe::egui::{pos2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::camera::{camera_at, Camera, CameraKey};
use crate::cles_alternees::{alternee, luminosite, rang_de, PAS};
use crate::document::{Cel, CelId, Document, Layer, LayerKind, LayerTransform};
use crate::fx::{Effect, EffectKind};
use crate::handles::runs;
use crate::opacitecles::OpKey;
use crate::perf_tests::{banc_taille, Banc};
use crate::poskeys::PosKey;
use crate::rognage::Rogner;
use crate::storyboard::BLOCS_DU_MONTAGE;
use crate::timeline::PRISE_FIN_DU_PLAN;
use crate::workspace::Espace;

const DT: f64 = 1.0 / 60.0;
const PPF: f32 = 12.0;

fn ctrl() -> Modifiers {
    Modifiers { ctrl: true, command: true, ..Modifiers::NONE }
}

fn cam(x: f32) -> Camera {
    Camera { x, y: 50.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 }
}

/// Trois plans de 48, 24 et 24 images (24 i/s : le premier dure 2 s). Le plan 0 a, sur « Dessin », quatre dessins de 12 images
/// (clés à 0, 12, 24, 36), sur « Fond » un seul ; quatre clés de caméra (0, 12, 24, 36 : la deuxième lissée, la troisième avec un
/// adouci de sortie), deux clés d'effet (12, 36), deux clés de position (6, 30), deux clés d'opacité (6, 42), deux signets. Les plans 1
/// et 2 ont leurs propres clés.
fn film() -> Document {
    film_de(64, 48)
}

/// Le même film sur une feuille de `w` × `h` (les captures ont besoin d'une feuille qu'une carte graphique accepte : un petit
/// dessin de 64 × 48 fait échouer `ahead_capacity`, un défaut d'avant qui ne touche que ces feuilles d'essai).
fn film_de(w: usize, h: usize) -> Document {
    let mut doc = Document::with_frame(w, h, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    let mut k = 0u8;
    for (i, (n, pas)) in [(48usize, 12usize), (24, 6), (24, 8)].into_iter().enumerate() {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(n);
        let mut dessin = Layer::new("Dessin", LayerKind::Bitmap);
        for d in 0..n / pas {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            cel.raster.tile_mut(0)[0] = eframe::egui::Color32::from_rgb(20 + 9 * k, 40, 200);
            k += 1;
            let c = doc.add_cel(cel);
            for f in d * pas..(d + 1) * pas {
                dessin.set(f, Some(c));
            }
        }
        let mut fond = Layer::new("Fond", LayerKind::Bitmap);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(1)[0] = eframe::egui::Color32::from_rgb(200, 30 + 9 * k, 60);
        k += 1;
        let c = doc.add_cel(cel);
        fond.frames = vec![Some(c); n];
        doc.panels[i].layers.push(dessin);
        doc.panels[i].layers.push(fond);
    }
    {
        let p = &mut doc.panels[0];
        p.camera = vec![CameraKey::new(0, cam(0.0)), CameraKey::new(12, cam(120.0)), CameraKey::new(24, cam(240.0)), CameraKey::new(36, cam(360.0))];
        p.camera[1].smooth = true;
        p.camera[2].ease_out = 4.0;
        let mut e = Effect::new(EffectKind::GaussianBlur);
        e.set_key(12, vec![2.0]);
        e.set_key(36, vec![8.0]);
        p.layers[0].fx.push(e);
        p.layers[0].pos_keys = vec![PosKey::new(6, LayerTransform { x: 10.0, ..LayerTransform::default() }), PosKey::new(30, LayerTransform { x: 40.0, ..LayerTransform::default() })];
        p.layers[0].opacity_keys = vec![OpKey::new(6, 0.2), OpKey::new(42, 0.9)];
        p.cues.bookmarks = vec![10, 40];
    }
    {
        let p = &mut doc.panels[1];
        p.camera = vec![CameraKey::new(0, cam(5.0)), CameraKey::new(12, cam(55.0))];
        p.layers[0].pos_keys = vec![PosKey::new(3, LayerTransform { x: 7.0, ..LayerTransform::default() })];
    }
    doc
}

/// Toutes les clés d'un plan, par sorte, en images du plan.
#[derive(Debug, PartialEq)]
struct Cles {
    dessin: Vec<usize>,
    fond: Vec<usize>,
    camera: Vec<usize>,
    lissage: Vec<(bool, i32, i32)>,
    effet: Vec<usize>,
    position: Vec<usize>,
    opacite: Vec<usize>,
    signets: Vec<usize>,
}

fn cles(doc: &Document, pi: usize) -> Cles {
    let p = &doc.panels[pi];
    let starts = |l: &Layer| runs(l).into_iter().filter(|r| r.2.is_some()).map(|r| r.0).collect::<Vec<_>>();
    Cles {
        dessin: starts(&p.layers[0]),
        fond: starts(&p.layers[1]),
        camera: p.camera.iter().map(|k| k.frame).collect(),
        lissage: p.camera.iter().map(|k| (k.smooth, (k.ease_in * 10.0) as i32, (k.ease_out * 10.0) as i32)).collect(),
        effet: p.layers[0].fx.first().map_or(Vec::new(), |e| e.keys.iter().map(|k| k.frame).collect()),
        position: p.layers[0].pos_keys.iter().map(|k| k.frame).collect(),
        opacite: p.layers[0].opacity_keys.iter().map(|k| k.frame).collect(),
        signets: p.cues.bookmarks.clone(),
    }
}

/// Tout ce que le film dit des plans et de leurs réserves, en texte (pour dire « exactement comme avant »).
fn etat(doc: &Document) -> String {
    let mut reserves: Vec<_> = doc.reserves.iter().collect();
    reserves.sort_by_key(|(id, _)| **id);
    let panneaux: Vec<String> = doc
        .panels
        .iter()
        .map(|p| {
            format!(
                "{:?}",
                (
                    p.id,
                    p.length,
                    p.layers.iter().map(|l| (l.id, &l.frames, &l.roles, &l.fx, &l.pos_keys, &l.opacity_keys)).collect::<Vec<_>>(),
                    &p.camera,
                    &p.marks,
                    &p.cues
                )
            )
        })
        .collect();
    format!("{:?}", (panneaux, reserves, &doc.audio_tracks.iter().map(|t| t.clips.iter().map(|c| (c.start, c.end())).collect::<Vec<_>>()).collect::<Vec<_>>(), &doc.comments.len()))
}

/// Ce que montre chaque image de chaque plan, au pixel : un résumé de l'image composée.
fn pixels(doc: &Document, pi: usize) -> Vec<u64> {
    (0..doc.panels[pi].frame_count())
        .map(|f| {
            let plan = crate::render::panel_plan_at(doc, pi, f, Some(doc.paper));
            let buf = crate::render::composite_full(doc, &plan);
            crate::fiabilite_fixture::empreinte(&buf.iter().flat_map(|c| c.to_array()).collect::<Vec<u8>>())
        })
        .collect()
}

/// Le contenu de chaque dessin du projet (empreinte des pixels), par numéro de dessin : il ne doit jamais changer.
fn contenu(doc: &Document) -> Vec<(CelId, u64)> {
    let mut v: Vec<(CelId, u64)> = doc.cels.iter().map(|(id, c)| (*id, crate::empreinte::Donnees::de(c).empreinte())).collect();
    v.sort_unstable();
    v
}

/// Les dessins (par contenu) qui sont dans le film ou dans une réserve : aucun ne doit disparaître.
fn dessins(doc: &Document) -> BTreeSet<CelId> {
    let mut ids = doc.used_cels();
    for r in doc.reserves.values() {
        ids.extend(r.cels());
    }
    ids
}

fn banc_montage(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = PPF;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(8);
    b
}

fn banc_timeline(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_espace(Espace::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(8);
    b
}

fn blocs() -> Vec<Rect> {
    BLOCS_DU_MONTAGE.with(|b| b.borrow().clone())
}

fn bouton(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Un bord tenu : le point de départ, et où est le pointeur.
struct Tenu {
    de: Pos2,
    ppf: f32,
    ici: Pos2,
}

/// Prend un bord de plan du montage (`droit` : sa fin ; sinon son début) et le tire de `frames` images, `touches` tenues au
/// départ. Ne lâche pas.
fn tenir_montage(b: &mut Banc, plan: usize, droit: bool, frames: f32, touches: Modifiers) -> Tenu {
    let r = blocs()[plan];
    let de = pos2(if droit { r.right() } else { r.left() }, r.center().y);
    b.touches = Modifiers::NONE;
    b.image(vec![Event::PointerMoved(de)], DT);
    b.touches = touches;
    b.image(vec![bouton(de, true)], DT);
    let mut t = Tenu { de, ppf: PPF, ici: de };
    aller_a(b, &mut t, frames);
    t
}

/// Le pointeur va à `frames` images du point de départ, en plusieurs pas.
fn aller_a(b: &mut Banc, t: &mut Tenu, frames: f32) {
    let vers = pos2(t.de.x + frames * t.ppf, t.de.y);
    for k in 1..=8 {
        let p = t.ici + (vers - t.ici) * (k as f32 / 8.0);
        b.image(vec![Event::PointerMoved(p)], DT);
    }
    b.image(Vec::new(), DT);
    t.ici = vers;
}

fn lacher(b: &mut Banc, t: &Tenu) {
    b.image(vec![bouton(t.ici, false)], DT);
    b.touches = Modifiers::NONE;
    b.image(vec![Event::PointerGone], DT);
    b.chauffer(3);
}

/// Un geste complet sur un bord du montage.
fn glisser_montage(b: &mut Banc, plan: usize, droit: bool, frames: f32, touches: Modifiers) {
    let t = tenir_montage(b, plan, droit, frames, touches);
    lacher(b, &t);
}

/// Prend la fin du plan courant dans la timeline et la tire de `frames` images.
fn tenir_timeline(b: &mut Banc, frames: f32, touches: Modifiers) -> Tenu {
    let tab = PRISE_FIN_DU_PLAN.with(|c| c.get()).expect("la timeline a peint la prise de la fin du plan");
    let de = pos2(tab.center().x, tab.center().y);
    b.touches = Modifiers::NONE;
    b.image(vec![Event::PointerMoved(de)], DT);
    b.touches = touches;
    b.image(vec![bouton(de, true)], DT);
    let mut t = Tenu { de, ppf: b.app.timeline_cell, ici: de };
    aller_a(b, &mut t, frames);
    t
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
}

fn retablir(b: &mut Banc) {
    b.app.run_action("edition.retablir");
}

fn etapes(b: &Banc) -> usize {
    b.app.history.undo_len()
}

// ------------------------------------------------------------------------------------ sans touche : seule la durée change

#[test]
fn without_a_key_pressed_the_right_edge_changes_only_the_duration_in_the_montage() {
    let mut b = banc_montage(film());
    let (avant, cles0, p1, p2, pix0, des0) = (etat(&b.app.doc), cles(&b.app.doc, 0), cles(&b.app.doc, 1), b.app.doc.panels[2].layers[0].frames.clone(), pixels(&b.app.doc, 0), dessins(&b.app.doc));
    let n = etapes(&b);
    glisser_montage(&mut b, 0, true, 24.0, Modifiers::NONE);
    let doc = &b.app.doc;
    assert_eq!(doc.panels[0].frame_count(), 72, "le plan dure 24 images de plus");
    assert_eq!(cles(doc, 0), cles0, "les clés sont restées où elles sont");
    assert_eq!(cles(doc, 1), p1, "les clés des plans suivants sont les leurs");
    assert_eq!(doc.panels[2].layers[0].frames, p2);
    assert_eq!(etapes(&b), n + 1, "une seule étape d'annulation");
    assert_eq!(&pixels(doc, 0)[..48], &pix0[..], "les 48 premières images se rendent comme avant");
    assert_eq!(dessins(doc), des0, "aucun dessin ne change");
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant, "annulé : exactement l'état d'avant");
    retablir(&mut b);
    assert_eq!(b.app.doc.panels[0].frame_count(), 72);
    assert_eq!(cles(&b.app.doc, 0), cles0);
}

#[test]
fn without_a_key_pressed_the_end_of_the_plan_handle_does_the_same_in_the_timeline() {
    let mut b = banc_timeline(film());
    let cles0 = cles(&b.app.doc, 0);
    let t = tenir_timeline(&mut b, 24.0, Modifiers::NONE);
    lacher(&mut b, &t);
    assert_eq!(b.app.doc.panels[0].frame_count(), 72);
    assert_eq!(cles(&b.app.doc, 0), cles0, "les clés restent où elles sont");
}

// ------------------------------------------------------------------------------------ Ctrl : les clés suivent la poignée

#[test]
fn ctrl_on_the_right_edge_moves_every_key_by_the_same_number_of_frames_in_the_montage() {
    let mut b = banc_montage(film());
    let (avant, cles0, p1, des0, pix0) = (etat(&b.app.doc), cles(&b.app.doc, 0), cles(&b.app.doc, 1), dessins(&b.app.doc), pixels(&b.app.doc, 0));
    let (cam0, def) = (b.app.doc.panels[0].camera.clone(), b.app.doc.default_camera());
    let n = etapes(&b);
    // Le cas validé par Anthony : plan de 2 s (48 i), clés à 0, 0,5, 1, 1,5 s ; le bord droit tiré de 2 s (48 i) : clés à 2, 2,5, 3, 3,5 s.
    glisser_montage(&mut b, 0, true, 48.0, ctrl());
    let doc = &b.app.doc;
    assert_eq!(doc.panels[0].frame_count(), 96);
    let c = cles(doc, 0);
    let plus = |v: &[usize]| v.iter().map(|f| f + 48).collect::<Vec<_>>();
    assert_eq!(c.dessin, plus(&cles0.dessin), "clés de dessin du calque Dessin");
    assert_eq!(c.fond, plus(&cles0.fond), "clé de dessin du calque Fond : elle aussi glisse (les 48 premières images sont vides)");
    assert_eq!(c.camera, plus(&cles0.camera), "clés de caméra");
    assert_eq!(c.lissage, cles0.lissage, "leurs poignées de lissage suivent chaque clé");
    assert_eq!(c.effet, plus(&cles0.effet), "clés d'effet");
    assert_eq!(c.position, plus(&cles0.position), "clés de position");
    assert_eq!(c.opacite, plus(&cles0.opacite), "clés d'opacité");
    assert_eq!(c.signets, plus(&cles0.signets), "signets");
    assert_eq!(cles(doc, 1), p1, "les clés du plan suivant ne bougent pas");
    // Le rythme ne change pas : ce que montrait l'image f montre l'image f + 48, au pixel et pour la caméra.
    assert_eq!(&pixels(doc, 0)[48..], &pix0[..], "le film rend les mêmes images, 48 images plus tard");
    for f in 0..48 {
        let (a, o) = (camera_at(&doc.panels[0].camera, f + 48, def), camera_at(&cam0, f, def));
        assert!((a.x - o.x).abs() < 1e-3, "caméra à l'image {f} : {} et {}", a.x, o.x);
    }
    assert_eq!(dessins(doc), des0, "aucun dessin créé ni perdu");
    assert_eq!(etapes(&b), n + 1, "une seule étape d'annulation");
    assert_eq!(b.app.history.names().0.last().map(|n| n.to_string()), Some(crate::rognage::noms::DUREE_CLES_LONGUE.to_string()));
    let apres = etat(&b.app.doc);
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant, "annulé : exactement l'état d'avant");
    retablir(&mut b);
    assert_eq!(etat(&b.app.doc), apres, "rétabli : exactement l'état d'après");
}

#[test]
fn ctrl_on_the_end_of_the_plan_handle_does_the_same_in_the_timeline() {
    let mut b = banc_timeline(film());
    let (avant, cles0) = (etat(&b.app.doc), cles(&b.app.doc, 0));
    let t = tenir_timeline(&mut b, 24.0, ctrl());
    assert!(b.app.status.contains("Ctrl"), "la ligne d'état parle de Ctrl pendant le geste : {}", b.app.status);
    lacher(&mut b, &t);
    let doc = &b.app.doc;
    assert_eq!(doc.panels[0].frame_count(), 72);
    let c = cles(doc, 0);
    let plus = |v: &[usize]| v.iter().map(|f| f + 24).collect::<Vec<_>>();
    assert_eq!((c.dessin, c.camera, c.effet, c.position, c.opacite), (plus(&cles0.dessin), plus(&cles0.camera), plus(&cles0.effet), plus(&cles0.position), plus(&cles0.opacite)));
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant);
}

#[test]
fn ctrl_pressed_or_released_during_the_gesture_takes_effect_at_once_and_going_back_to_the_origin_gives_the_start_back_exactly() {
    let mut b = banc_montage(film());
    let avant = etat(&b.app.doc);
    let n = etapes(&b);
    let mut t = tenir_montage(&mut b, 0, true, 12.0, Modifiers::NONE);
    let sans = cles(&b.app.doc, 0);
    assert_eq!(sans.dessin, [0, 12, 24, 36], "sans Ctrl : les clés restent");
    b.touches = ctrl();
    aller_a(&mut b, &mut t, 12.0);
    assert_eq!(cles(&b.app.doc, 0).dessin, [12, 24, 36, 48], "Ctrl pressée en cours de geste : les clés suivent");
    b.touches = Modifiers::NONE;
    aller_a(&mut b, &mut t, 12.0);
    assert_eq!(cles(&b.app.doc, 0).dessin, [0, 12, 24, 36], "Ctrl lâchée : elles restent");
    // De nouveau Ctrl, puis la poignée revient à son origine : l'état de départ, exactement.
    b.touches = ctrl();
    aller_a(&mut b, &mut t, 30.0);
    assert_ne!(etat(&b.app.doc), avant);
    aller_a(&mut b, &mut t, 0.0);
    assert_eq!(etat(&b.app.doc), avant, "la poignée est revenue à son origine : rien n'a changé");
    lacher(&mut b, &t);
    assert_eq!(etat(&b.app.doc), avant);
    assert_eq!(etapes(&b), n, "un geste qui ne change rien n'est pas une étape d'annulation");
}

#[test]
fn ctrl_shortening_pushes_the_first_keys_into_the_reserve_and_nothing_is_lost() {
    // (Sans courbes de position ni d'opacité pour comparer les images au pixel : la clé posée au bord du plan donne les mêmes
    // valeurs, à l'arrondi des flottants près ; elles ont leur essai plus bas.)
    let mut doc0 = film();
    doc0.panels[0].layers[0].pos_keys.clear();
    doc0.panels[0].layers[0].opacity_keys.clear();
    let mut b = banc_montage(doc0);
    let (avant, des0, contenu0) = (etat(&b.app.doc), dessins(&b.app.doc), contenu(&b.app.doc));
    let pix0 = pixels(&b.app.doc, 0);
    glisser_montage(&mut b, 0, true, -12.0, ctrl());
    let doc = &b.app.doc;
    assert_eq!(doc.panels[0].frame_count(), 36);
    let c = cles(doc, 0);
    // Le premier dessin (clé à 0) sort par le début, dans la réserve ; les trois autres reculent de 12 images.
    assert_eq!(c.dessin, [0, 12, 24]);
    assert_eq!(c.camera, [0, 12, 24], "la clé de caméra à 0 sort, celles de 12, 24, 36 reculent ; la courbe est gardée");
    assert_eq!(doc.hidden(0), (12, 0), "12 images cachées au début, en réserve");
    assert_eq!(dessins(doc), des0, "aucun dessin perdu : ils sont dans le film ou la réserve");
    // Le film : l'image f montre ce que montrait l'image f + 12.
    assert_eq!(&pixels(doc, 0)[..], &pix0[12..], "le rythme interne ne change pas");
    // Les clés de position et d'opacité sorties sont gardées elles aussi, et rendues par un nouveau rallongement.
    assert_eq!(contenu(doc), contenu0, "les dessins n'ont pas changé d'un pixel");
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant, "annulé : exactement l'état d'avant");
    retablir(&mut b);
    // Rallonger de nouveau (même geste, Ctrl) : les clés reviennent exactement où elles étaient.
    let mut sans_courbes = film();
    sans_courbes.panels[0].layers[0].pos_keys.clear();
    sans_courbes.panels[0].layers[0].opacity_keys.clear();
    assert_eq!(b.app.doc.slide_plan_end(0, 48), 48);
    assert_eq!(cles(&b.app.doc, 0), cles(&sans_courbes, 0), "rallonger rend les clés exactement où elles étaient");
    assert_eq!(etat(&b.app.doc), avant, "et le plan est exactement celui d'avant, réserve vide comprise");
    assert_eq!(b.app.doc.hidden(0), (0, 0));
}

#[test]
fn ctrl_shortening_a_lot_leaves_one_frame_and_no_key_is_lost() {
    let mut b = banc_montage(film());
    let (avant, des0) = (etat(&b.app.doc), dessins(&b.app.doc));
    glisser_montage(&mut b, 0, true, -200.0, ctrl());
    assert_eq!(b.app.doc.panels[0].frame_count(), 1, "un plan garde au moins une image");
    assert_eq!(dessins(&b.app.doc), des0);
    assert_eq!(b.app.doc.hidden(0), (47, 0));
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant);
}

#[test]
fn the_keys_pushed_out_of_the_plan_come_back_after_saving_and_reopening_the_project() {
    let mut b = banc_montage(film());
    let origine = film();
    glisser_montage(&mut b, 0, true, -12.0, ctrl());
    let dir = std::env::temp_dir().join(format!("shot-duree-cles-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("D.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.iter().all(|w| !w.contains("rognages")), "{:?}", relu.warnings);
    let mut doc = relu.doc;
    assert_eq!(doc.hidden(0), (12, 0));
    assert_eq!(cles(&doc, 0), cles(&b.app.doc, 0), "les clés du plan sont les mêmes après la réouverture");
    assert_eq!(doc.slide_plan_end(0, 48), 48);
    assert_eq!(cles(&doc, 0), cles(&origine, 0), "et celles de la réserve reviennent exactement où elles étaient (dessins, caméra, effets, position, opacité)");
    let (p, o) = (&doc.panels[0], &origine.panels[0]);
    assert_eq!(p.layers[0].pos_keys.iter().map(|k| (k.frame, k.t.x)).collect::<Vec<_>>(), o.layers[0].pos_keys.iter().map(|k| (k.frame, k.t.x)).collect::<Vec<_>>());
    assert_eq!(p.layers[0].opacity_keys.iter().map(|k| (k.frame, k.value)).collect::<Vec<_>>(), o.layers[0].opacity_keys.iter().map(|k| (k.frame, k.value)).collect::<Vec<_>>());
}

// ------------------------------------------------------------------------------------ le bord gauche

#[test]
fn on_the_left_edge_ctrl_keeps_the_keys_in_place_from_the_start_and_the_end_changes() {
    // Sans touche (comme avant) : le début se raccourcit, les clés gardent leur distance à la fin.
    let mut b = banc_montage(film());
    let avant = etat(&b.app.doc);
    glisser_montage(&mut b, 0, false, 12.0, Modifiers::NONE);
    assert_eq!(b.app.doc.panels[0].frame_count(), 36);
    assert_eq!(cles(&b.app.doc, 0).dessin, [0, 12, 24], "le premier dessin sort par le début, les autres reculent");
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant);
    // Ctrl : les clés gardent leur place depuis le début du plan ; c'est la fin qui perd 12 images (réserve de la fin).
    glisser_montage(&mut b, 0, false, 12.0, ctrl());
    assert_eq!(b.app.doc.panels[0].frame_count(), 36);
    assert_eq!(cles(&b.app.doc, 0).dessin, [0, 12, 24], "les trois premiers restent");
    assert_eq!(cles(&b.app.doc, 0).camera, [0, 12, 24, 35], "la caméra garde sa courbe : une clé est posée au nouveau bord");
    assert_eq!(b.app.doc.hidden(0), (0, 12), "le dernier dessin est caché à la fin, en réserve");
    assert!(b.app.history.names().0.last().is_some_and(|n| n.to_string().contains("clés décalées")));
    annuler(&mut b);
    assert_eq!(etat(&b.app.doc), avant, "annulé : exactement l'état d'avant");
}

// ------------------------------------------------------------------------------------ refus et son

#[test]
fn a_locked_storyboard_refuses_the_gesture_with_or_without_ctrl() {
    let mut doc = film();
    doc.storyboard_locked = true;
    let mut b = banc_montage(doc);
    let avant = etat(&b.app.doc);
    for touches in [Modifiers::NONE, ctrl()] {
        glisser_montage(&mut b, 0, true, 24.0, touches);
        assert_eq!(etat(&b.app.doc), avant, "storyboard verrouillé : rien ne change");
    }
    let mut b = banc_timeline({
        let mut d = film();
        d.storyboard_locked = true;
        d
    });
    let avant = etat(&b.app.doc);
    let t = tenir_timeline(&mut b, 24.0, ctrl());
    lacher(&mut b, &t);
    assert_eq!(etat(&b.app.doc), avant, "la timeline refuse comme le montage");
}

#[test]
fn the_sound_stays_where_it_is_and_the_status_line_says_the_next_plans_move_against_it() {
    let mut doc = film();
    doc.add_audio_track();
    let pcm = crate::audio::Pcm::from_interleaved(&vec![0.1f32; 2 * 48_000], 2, 48_000);
    let s = doc.add_sound("a.wav".into(), "wav".into(), std::sync::Arc::new(Vec::new()), std::sync::Arc::new(pcm));
    doc.add_clip(0, s, 60, 0, 6).unwrap();
    let clips0: Vec<(usize, usize)> = doc.audio_tracks[0].clips.iter().map(|c| (c.start, c.end())).collect();
    let mut b = banc_montage(doc);
    let t = tenir_montage(&mut b, 0, true, 24.0, ctrl());
    assert!(b.app.status.contains("par rapport au son"), "{}", b.app.status);
    lacher(&mut b, &t);
    let clips: Vec<(usize, usize)> = b.app.doc.audio_tracks[0].clips.iter().map(|c| (c.start, c.end())).collect();
    assert_eq!(clips, clips0, "le son ne bouge pas : il n'est pas lié au plan");
}

#[test]
fn the_status_line_and_the_handle_bubble_mention_ctrl() {
    let mut b = banc_montage(film());
    let t = tenir_montage(&mut b, 0, true, 6.0, Modifiers::NONE);
    assert!(b.app.status.contains("Ctrl"), "pendant le geste : {}", b.app.status);
    lacher(&mut b, &t);
    assert!(crate::handles::BULLE_DUREE_AIDE.contains("Ctrl") && crate::handles::BULLE_DEBUT_AIDE.contains("Ctrl"));
    // Le raccourci n'était pas pris sur ces poignées : le bord n'avait que Alt (roll, glisser le contenu).
    assert!(!crate::handles::BULLE_DUREE_AIDE.contains("Alt"));
}

#[test]
fn a_plan_with_a_head_reserve_gets_it_back_before_empty_frames_when_lengthened_with_ctrl() {
    let mut doc = film();
    assert_eq!(doc.rogner(0, Rogner::Head(6)), Ok(6));
    let cles0 = cles(&doc, 0);
    assert_eq!(doc.slide_plan_end(0, 48), 48);
    assert_eq!(doc.hidden(0), (0, 0), "ce qui était rogné au début revient d'abord");
    assert_eq!(doc.panels[0].frame_count(), 48);
    assert_ne!(cles(&doc, 0), cles0);
    assert_eq!(doc.slide_plan_end(0, 60), 60);
    assert_eq!(cles(&doc, 0).dessin, [12, 24, 36, 48], "puis les nouvelles images sont vides : la première clé glisse");
}

// ------------------------------------------------------------------------------------ clés de dessin mieux identifiables

#[test]
fn drawing_keys_alternate_between_the_layer_colour_and_a_lighter_one_in_every_theme() {
    use crate::theme::{self, ChoixTheme, ReglageTheme};
    for choix in ChoixTheme::TOUS {
        let mut r = ReglageTheme::default();
        r.choisir(choix);
        theme::installer(&r);
        let p = theme::palette();
        let couleurs = [p.dessin_vectoriel, p.dessin_bitmap, p.dessin_couleur, p.dessin_vide, p.calque_vectoriel, p.calque_bitmap, p.calque_couleur, p.calque_decor, p.dessin_cfx, p.dessin_annotation, p.dessin_unie];
        for c in couleurs.into_iter().chain([eframe::egui::Color32::from_rgb(10, 40, 30), eframe::egui::Color32::from_rgb(250, 250, 245), eframe::egui::Color32::BLACK, eframe::egui::Color32::WHITE]) {
            assert_eq!(alternee(c, 0), c, "{choix:?} : le rang pair garde la couleur du calque");
            assert_eq!(alternee(c, 2), c);
            let clair = alternee(c, 1);
            let ecart = (luminosite(clair) - luminosite(c)).abs();
            assert!(ecart > PAS * 0.6, "{choix:?} : {c:?} -> {clair:?} : écart de luminosité {ecart}");
            assert!(ecart < PAS * 1.6, "{choix:?} : l'écart reste discret ({ecart})");
            if luminosite(c) < 0.8 {
                assert!(luminosite(clair) > luminosite(c), "{choix:?} : le rang impair est plus clair");
            }
        }
    }
    theme::installer(&ReglageTheme::default());
}

#[test]
fn the_rank_of_a_key_counts_from_the_start_of_the_plan_not_from_what_is_scrolled_into_view() {
    let doc = film();
    let l = &doc.panels[0].layers[0];
    assert_eq!([0, 12, 24, 36].map(|s| rang_de(l, s)), [0, 1, 2, 3]);
    // Une plage vide n'a pas de rang : le dessin d'après garde la parité.
    let mut l = l.clone();
    for f in 12..24 {
        l.set(f, None);
    }
    assert_eq!([0, 24, 36].map(|s| rang_de(&l, s)), [0, 1, 2]);
}

// ------------------------------------------------------------------------------------ à regarder

/// Les pistes de calques, avant et après l'alternance, en PNG (hors de la suite), dans la timeline et dans le montage, trois thèmes :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --locked --target-dir target\msvc cles_alternees_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn cles_alternees_captures() {
    use crate::cles_alternees::SANS_ALTERNANCE;
    use crate::theme::{self, ChoixTheme, ReglageTheme};
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut doc = film_de(512, 288);
    // Deux calques de trait (jade dans Atelier et Papier).
    for (nom, kind, pas) in [("Trait", LayerKind::Vector, 6usize), ("Encre", LayerKind::Vector, 4)] {
        let mut l = Layer::new(nom, kind);
        for d in 0..48 / pas {
            let c = doc.add_cel(Cel::new(doc.width, doc.height, kind));
            for f in d * pas..(d + 1) * pas {
                l.set(f, Some(c));
            }
        }
        doc.panels[0].layers.push(l);
    }
    let mut b = banc_taille(doc, true, 1920, 1080);
    b.app.montage_zoom = 14.0;
    let sauver = |b: &Banc, nom: &str, zone: Rect| {
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom} ignorée") };
        let img = image::RgbaImage::from_raw(1920, 1080, rgba).expect("image");
        let (x, y) = (zone.left().max(0.0) as u32, zone.top().max(0.0) as u32);
        let (w, h) = ((zone.width() as u32).min(1920 - x), (zone.height() as u32).min(1080 - y));
        let rogne = image::imageops::crop_imm(&img, x, y, w, h).to_image();
        let grand = image::imageops::resize(&rogne, w * 2, h * 2, image::imageops::FilterType::Nearest);
        grand.save(dir.join(nom)).expect("png");
        println!("CAPTURE {nom}");
    };
    for (choix, court) in [(ChoixTheme::Shot, "atelier"), (ChoixTheme::Papier, "papier"), (ChoixTheme::GrisSombre, "gris-sombre")] {
        let mut r = ReglageTheme::default();
        r.choisir(choix);
        theme::installer(&r);
        theme::appliquer(&b.ctx);
        for (avant, etiquette) in [(true, "avant"), (false, "apres")] {
            SANS_ALTERNANCE.with(|c| c.set(avant));
            b.app.set_espace(Espace::Montage);
            (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
            b.chauffer(10);
            let r = blocs().first().copied().unwrap_or(Rect::NOTHING);
            sauver(&b, &format!("cles-montage-{court}-{etiquette}.png"), Rect::from_min_max(pos2(r.left() - 10.0, r.top() - 10.0), pos2(r.right() + 340.0, r.top() + 330.0)));
            b.app.set_espace(Espace::Animation);
            b.chauffer(10);
            let tab = PRISE_FIN_DU_PLAN.with(|c| c.get()).unwrap_or(Rect::NOTHING);
            sauver(&b, &format!("cles-timeline-{court}-{etiquette}.png"), Rect::from_min_max(pos2(tab.left() - 780.0, tab.top() - 4.0), pos2(tab.left() + 60.0, tab.top() + 330.0)));
        }
    }
    SANS_ALTERNANCE.with(|c| c.set(false));
    theme::installer(&ReglageTheme::default());
}

// ------------------------------------------------------------------------------------ le coût du geste

#[test]
fn one_step_of_the_gesture_is_list_work_even_on_a_very_big_plan() {
    // Un plan de 3 000 images, 12 calques de 300 dessins, 200 clés de caméra, des clés de position et d'opacité sur chaque calque :
    // chaque image d'écran du geste restaure le plan d'avant puis refait le décalage (aucun pixel, aucun envoi à la carte graphique).
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(3000);
    for k in 0..12 {
        let mut l = Layer::new(&format!("Calque {k}"), LayerKind::Bitmap);
        for d in 0..300 {
            let c = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
            for f in d * 10..(d + 1) * 10 {
                l.set(f, Some(c));
            }
        }
        l.pos_keys = (0..100).map(|i| PosKey::new(i * 30, LayerTransform { x: i as f32, ..LayerTransform::default() })).collect();
        l.opacity_keys = (0..100).map(|i| OpKey::new(i * 30, 0.5)).collect();
        doc.panels[0].layers.push(l);
    }
    doc.panels[0].camera = (0..200).map(|i| CameraKey::new(i * 15, cam(i as f32))).collect();
    let sauve = crate::rognageui::PlanBackup::capture(&doc, 0);
    let mut pire = std::time::Duration::ZERO;
    for (i, n) in [3600usize, 2400, 3300, 1200, 3600, 3000].into_iter().enumerate() {
        let t = std::time::Instant::now();
        sauve.restore(&mut doc);
        doc.slide_plan_end(0, n);
        pire = pire.max(t.elapsed());
        assert_eq!(doc.panels[0].frame_count(), n, "pas {i}");
    }
    sauve.restore(&mut doc);
    assert_eq!(doc.panels[0].frame_count(), 3000);
    println!("DUREE-CLES un pas du geste sur 3 000 images, 12 calques : {pire:?} (le pire de six)");
    assert!(pire < std::time::Duration::from_millis(60), "un pas du geste ne doit pas peser sur le fil de l'interface : {pire:?}");
}
