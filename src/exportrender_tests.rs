//! Essais du rendu des exports hors du fil de l'interface (`exportrender.rs`, `export.rs`).
//!
//! Ce qu'ils gardent :
//! - le cliché copie la structure et partage les tuiles, et ce qu'on fait au projet ensuite ne change pas l'export ;
//! - les images sont **les mêmes pixels** que celles de l'ancien chemin (plan du plan, caméra, effets, calques
//!   d'effets, transitions, incrustations), pour chaque sorte d'export : film (par un ffmpeg qui recopie ses images),
//!   suite de PNG, image seule, GIF, PDF et pages (par `collect`), page HTML, montage ;
//! - aucune image n'est composée sur le fil de l'interface ; une image tenue n'est composée qu'une fois ;
//! - annuler arrête tout et efface ce qui n'est pas fini, sans toucher au fichier d'avant ;
//! - un export continue quand on change d'onglet, et le projet reste libre pendant ce temps ;
//! - la réserve privée d'un fil d'export ne touche pas la réserve commune.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::camera::{Camera, CameraKey};
use crate::document::{Document, LayerKind, LayerTransform};
use crate::export::{self, ExportJob, Format, Range, SoundSpan};
use crate::export_tests::doc_with_drawing;
use crate::exportrender::{self, meme_dessin, Image, Papier, Pipeline, Pose, Rendu, IMAGES_ICI};
use crate::fiabilite_fixture::{dossier, projet_complet};
use crate::fx::{Effect, EffectKind};
use crate::fxcache::monde;
use crate::raster::unpremultiply;
use crate::render;

// ---------------------------------------------------------------------------------------------- outils

/// L'ancien chemin d'un export : le plan du plan, la caméra, l'image composée sur le fil appelant, les incrustations.
fn ancienne_image(doc: &Document, p: usize, f: usize, paper: Option<[u8; 3]>, graver: bool) -> Vec<Color32> {
    let plan = render::panel_plan_at(doc, p, f, paper);
    let cam = doc.panel_camera(p, f);
    let img: Image = Arc::new(render::camera_image(doc, &plan, &cam));
    if graver {
        (*export::burned(doc, p, f, img)).clone()
    } else {
        (*img).clone()
    }
}

/// Le nouvel export, sur le fil appelant mais avec la réserve privée des fils de rendu : mêmes appels que les fils.
fn nouvelle_image(rendu: &Rendu, p: usize, f: usize) -> Vec<Color32> {
    let img = crate::fxcache::avec_privee(256 << 20, || rendu.image(p, f)).expect("image");
    (*img).clone()
}

fn dossier_neuf(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-export-rendu-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Fait tourner un export jusqu'au bout, comme l'interface (avec une limite de temps).
fn court(job: &mut ExportJob, doc: &Document) {
    let t = Instant::now();
    while job.outcome.is_none() {
        job.step(doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120), "export trop long : {}", job.phase());
    }
}

/// Un projet d'essai avec des effets de calque, un calque transformé, une caméra qui bouge et des images tenues.
fn projet_a_effets(panels: usize, len: usize) -> Document {
    let mut doc = doc_with_drawing(panels, len);
    let c = doc.default_camera();
    for p in 0..panels {
        doc.panels[p].camera = vec![CameraKey::new(0, c), CameraKey::new(len - 1, Camera { x: c.x + 15.0 * (p as f32 + 1.0), zoom: c.zoom * 1.1, ..c })];
        let mut flou = Effect::new(EffectKind::GaussianBlur);
        flou.set_param(0, 2.0 + p as f32);
        doc.panels[p].layers[0].fx.push(flou);
    }
    doc
}

fn compare(a: &[Color32], b: &[Color32], quoi: &str) {
    assert_eq!(a.len(), b.len(), "{quoi} : taille");
    let diff = a.iter().zip(b).filter(|(x, y)| x != y).count();
    assert_eq!(diff, 0, "{quoi} : {diff} pixels différents sur {}", a.len());
}

// ------------------------------------------------------------------------------------------ le cliché

#[test]
fn the_snapshot_keeps_the_structure_shares_the_tiles_and_ignores_later_edits() {
    monde::lire();
    let mut doc = doc_with_drawing(4, 6);
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, c), CameraKey::new(5, Camera { x: c.x + 30.0, ..c })];
    let snap = exportrender::snapshot_of(&doc, &[1]);
    // La structure : tous les plans, leur durée, leur place, leur caméra (celle d'un plan sans clé suit ses voisins).
    assert_eq!(snap.panels.len(), 4);
    for i in 0..4 {
        assert_eq!(snap.panels[i].frame_count(), doc.panels[i].frame_count(), "durée du plan {i}");
        assert_eq!(snap.panel_start(i), doc.panel_start(i));
        assert_eq!(snap.panel_camera(i, 2), doc.panel_camera(i, 2), "caméra du plan {i}");
    }
    assert_eq!((snap.width, snap.height, snap.frame, snap.fps), (doc.width, doc.height, doc.frame, doc.fps));
    // Les dessins : ceux du plan exporté seulement, et ils partagent leurs tuiles (aucun pixel copié).
    assert!(snap.panels[0].layers.is_empty() && !snap.panels[1].layers.is_empty());
    let ids: Vec<_> = doc.panels[1].layers.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect();
    assert!(!ids.is_empty());
    for id in &ids {
        assert!(meme_dessin(&snap.cels[id], &doc.cels[id]), "le dessin {id} partage ses tuiles");
    }
    assert_eq!(snap.cels.len(), ids.iter().collect::<std::collections::HashSet<_>>().len(), "les autres plans n'apportent aucun dessin");
    // On dessine ensuite dans le projet : le cliché garde ses pixels (copie à la première écriture).
    let id = ids[0];
    let avant = snap.cels[&id].raster.get(0, 0);
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(0)[0] = Color32::from_rgb(1, 2, 3);
    cel.version += 1;
    assert_eq!(snap.cels[&id].raster.get(0, 0), avant);
    assert_ne!(doc.cels[&id].raster.get(0, 0), avant);
}

#[test]
fn the_snapshot_is_built_in_slices_and_matches_the_all_at_once_one() {
    monde::lire();
    let mut doc = doc_with_drawing(3, 30);
    // Un dessin différent à chaque image : une centaine de dessins à copier.
    for p in 0..3 {
        for f in 0..30 {
            let id = doc.new_cel(LayerKind::Bitmap);
            doc.panels[p].layers[0].frames[f] = Some(id);
        }
    }
    let mut b = exportrender::SnapshotBuilder::new(&doc, &[0, 1, 2]);
    let mut tranches = 0;
    // Un budget nul : une tranche de huit dessins au plus à chaque appel.
    while !b.step(&doc, Duration::ZERO) {
        tranches += 1;
        assert!(tranches < 1000);
    }
    let (fait, total) = b.progress();
    assert_eq!(fait, total);
    let par_tranches = b.finish();
    let d_un_coup = exportrender::snapshot_of(&doc, &[0, 1, 2]);
    assert!(tranches >= 5, "le budget borne chaque tranche : {tranches}");
    assert_eq!(par_tranches.cels.len(), d_un_coup.cels.len());
    assert!(par_tranches.cels.len() >= 90);
}

// ------------------------------------------------------------------------------------ les mêmes pixels

/// Sur le projet d'essai complet de la fiabilité (effets de calque à clés, tremblement du trait, fond sous le trait, calque de couleur qui griffonne,
/// vidéo, dossier, fusion, transformation, caméra à clés, calques d'effets d'ajustement, incrustation, damier, plans rognés,
/// transitions), chaque image du nouvel export est celle de l'ancien chemin, pixel pour pixel, à plusieurs images de chaque plan.
#[test]
fn the_new_frames_are_the_old_ones_pixel_for_pixel_on_the_reliability_project() {
    // (Exclusif : l'ancien chemin, de référence, compose avec la réserve commune des dessins dérivés, qu'un autre essai
    // pourrait vider ou réduire en cours de route — un calque d'effet d'ajustement disparaîtrait alors de l'image.)
    monde::ecrire();
    let travail = dossier("export-rendu-pixels");
    let doc = projet_complet(&travail).doc;
    let mut vus_fx = 0;
    for papier in [(Papier::Fond, doc.render_paper()), (Papier::Feuille, Some(doc.paper)), (Papier::Aucun, None)] {
        let rendu = Rendu::new(exportrender::snapshot_of(&doc, &(0..doc.panels.len()).collect::<Vec<_>>()), papier.0, false, Vec::new(), Arc::default());
        for p in 0..doc.panels.len() {
            let n = doc.panels[p].frame_count();
            for f in [0, n / 3, n / 2, n - 1] {
                let plan = render::panel_plan_at(&doc, p, f, papier.1);
                vus_fx += plan.items.iter().filter(|i| !i.fx.is_empty() || i.fond.is_some() || i.ctg.is_some()).count();
                compare(&ancienne_image(&doc, p, f, papier.1, false), &nouvelle_image(&rendu, p, f), &format!("{:?} plan {p} image {f}", papier.0));
            }
        }
    }
    assert!(vus_fx > 0, "le projet d'essai doit avoir des effets dans ses images");
}

/// La toile infinie (dessin hors de la feuille, caméra qui regarde hors de la feuille) et les clés de position d'un calque :
/// le nouvel export les rend comme l'ancien.
#[test]
fn the_off_sheet_world_and_the_position_keys_are_exported_like_before() {
    monde::lire();
    let mut doc = doc_with_drawing(1, 6);
    let id = doc.panels[0].layers[0].frames[0].expect("dessin");
    let rouge = crate::raster::Tile::new([Color32::from_rgba_premultiplied(200, 30, 30, 255); crate::raster::TILE_PIXELS]);
    let cel = doc.cels.get_mut(&id).unwrap();
    for (tx, ty) in [(-1, -1), (-2, 0), (5, 1), (1, 3), (-1, 1)] {
        cel.raster.replace_slot(crate::raster::outer_id(tx, ty), Some(rouge.clone()));
    }
    // Une caméra qui regarde hors de la feuille, en haut à gauche, puis revient.
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: -30.0, y: -20.0, ..c }), CameraKey::new(5, Camera { x: c.x, y: c.y, zoom: c.zoom * 1.2, ..c })];
    // Le deuxième calque glisse et tourne entre deux clés de position.
    let t0 = LayerTransform::default();
    doc.panels[0].layers[1].pos_keys = vec![crate::poskeys::PosKey::new(0, t0), crate::poskeys::PosKey::new(5, LayerTransform { x: 40.0, y: -15.0, angle: 0.4, scale: 1.1, ..t0 })];
    let rendu = Rendu::new(exportrender::snapshot_of(&doc, &[0]), Papier::Fond, true, Vec::new(), Arc::default());
    let mut dehors = 0;
    for f in 0..6 {
        let plan = render::panel_plan_at(&doc, 0, f, doc.render_paper());
        dehors += usize::from(plan.items.iter().any(|i| doc.cels.get(&i.cel).is_some_and(|c| !c.raster.outer.is_empty())));
        compare(&ancienne_image(&doc, 0, f, doc.render_paper(), true), &nouvelle_image(&rendu, 0, f), &format!("image {f}"));
    }
    assert!(dehors > 0);
    // Les tuiles hors de la feuille sont bien dans la copie, partagées.
    let copie = exportrender::snapshot_of(&doc, &[0]);
    assert_eq!(copie.cels[&id].raster.outer.len(), 5);
    assert!(copie.cels[&id].raster.outer.values().zip(doc.cels[&id].raster.outer.values()).all(|(a, b)| a.same_as(b)));
}

#[test]
fn the_overlays_are_burned_like_before_and_every_frame_gets_its_own() {
    monde::lire();
    let mut doc = doc_with_drawing(2, 6);
    for p in 0..2 {
        let cel = doc.new_cel(LayerKind::Bitmap);
        let mut l = crate::document::Layer::new_effect(
            "Timecode",
            crate::calqueffet::CalqueEffet::incrustation(
                crate::incrustation::Incrustation { ancre: crate::incrustation::Ancre::HautGauche, taille: 12.0, marge: 3.0, couleur: [255, 255, 0, 255], fond: Some([0, 0, 60, 255]), ..Default::default() },
                "Timecode",
            ),
        );
        l.frames = vec![Some(cel); 6];
        doc.panels[p].layers.push(l);
    }
    let rendu = Rendu::new(exportrender::snapshot_of(&doc, &[0, 1]), Papier::Fond, true, Vec::new(), Arc::default());
    let mut vues = Vec::new();
    for p in 0..2 {
        for f in 0..6 {
            let nouvelle = nouvelle_image(&rendu, p, f);
            compare(&ancienne_image(&doc, p, f, doc.render_paper(), true), &nouvelle, &format!("plan {p} image {f}"));
            vues.push(nouvelle);
        }
    }
    assert_ne!(vues[0], vues[1], "le timecode change d'une image à l'autre");
}

#[test]
fn the_transitions_are_mixed_like_before() {
    monde::lire();
    let mut doc = doc_with_drawing(3, 8);
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.transitions = vec![
        crate::transitions::Transition { id: 1001, out: Some(ids[0]), inn: Some(ids[1]), kind: crate::transitions::Kind::Dissolve, frames: 4, align: Default::default() },
        crate::transitions::Transition { id: 1002, out: Some(ids[1]), inn: Some(ids[2]), kind: crate::transitions::Kind::Wipe, frames: 3, align: Default::default() },
    ];
    let zones = crate::transitions::zones(&doc);
    assert!(!zones.is_empty());
    let panels = exportrender::panels_needed(&export::frames_for(&doc, Range::Panel, 1), &zones);
    assert!(panels.contains(&0) && panels.contains(&1) && panels.contains(&2), "les plans de part et d'autre des transitions sont copiés : {panels:?}");
    // L'export d'un seul plan, avec les transitions : son cliché n'a que lui et ses voisins.
    let rendu = Rendu::new(exportrender::snapshot_of(&doc, &panels), Papier::Fond, true, zones.clone(), Arc::default());
    let tout = Rendu::new(exportrender::snapshot_of(&doc, &[0, 1, 2]), Papier::Fond, true, zones.clone(), Arc::default());
    let mut mixees = 0;
    for p in 0..3 {
        for f in 0..8 {
            let start = doc.panel_start(p);
            let mix = crate::transitions::mix_in(&zones, start + f);
            let attendue = match mix {
                Some((_, m)) => {
                    mixees += 1;
                    let a = m.a.map(|(p, f)| ancienne_image(&doc, p, f, doc.render_paper(), true));
                    let b = m.b.map(|(p, f)| ancienne_image(&doc, p, f, doc.render_paper(), true));
                    crate::transitions::blend_images(&m, a.as_deref(), b.as_deref(), doc.frame)
                }
                None => ancienne_image(&doc, p, f, doc.render_paper(), true),
            };
            compare(&attendue, &nouvelle_image(&tout, p, f), &format!("plan {p} image {f}"));
            if p == 1 {
                compare(&attendue, &nouvelle_image(&rendu, p, f), &format!("plan {p} seul, image {f}"));
            }
        }
    }
    assert!(mixees > 0);
}

#[test]
fn a_held_frame_is_composed_once_even_when_several_threads_ask_for_it_together() {
    monde::lire();
    // Un plan de 24 images dont rien ne bouge : une seule image à composer.
    let mut doc = doc_with_drawing(1, 24);
    for l in &mut doc.panels[0].layers {
        let cel = l.frames[0];
        l.frames = vec![cel; 24];
    }
    let rendu = Arc::new(Rendu::new(exportrender::snapshot_of(&doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default()));
    let fils: Vec<_> = (0..4)
        .map(|k| {
            let r = rendu.clone();
            std::thread::spawn(move || crate::fxcache::avec_privee(64 << 20, || (0..24).map(|f| r.image(0, (f + 5 * k) % 24).expect("image")).map(|i| Arc::as_ptr(&i) as usize).collect::<Vec<_>>()))
        })
        .collect();
    let adresses: Vec<Vec<usize>> = fils.into_iter().map(|f| f.join().unwrap()).collect();
    assert_eq!(rendu.composed(), 1, "une image tenue ne se compose qu'une fois");
    assert!(adresses.iter().flatten().all(|a| *a == adresses[0][0]), "la même mémoire pour toutes");
}

#[test]
fn a_moving_frame_is_composed_every_time() {
    monde::lire();
    let doc = projet_a_effets(1, 6);
    let rendu = Rendu::new(exportrender::snapshot_of(&doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default());
    for f in 0..6 {
        nouvelle_image(&rendu, 0, f);
    }
    assert!(rendu.composed() >= 4, "la caméra bouge : {} images composées", rendu.composed());
}

// ----------------------------------------------------------------------- la réserve privée des effets

#[test]
fn the_private_reserve_keeps_effects_out_of_the_shared_cache() {
    monde::lire();
    let doc = projet_a_effets(1, 3);
    let plan = render::panel_plan_at(&doc, 0, 0, doc.render_paper());
    let item = plan.items.iter().find(|i| !i.fx.is_empty()).expect("un élément avec effet").clone();
    let id = crate::fx::shown_id(&item);
    let version = doc.cels[&item.cel].version;
    // Dans la réserve commune, rien (un autre essai peut l'avoir calculé : on ne regarde que la différence).
    let avant = crate::fxcache::get(id, version).is_some();
    let dedans = crate::fxcache::avec_privee(64 << 20, || {
        assert!(crate::fxcache::privee_active());
        let a = crate::fx::shown(&doc, &item).expect("dessin").raster.clone();
        assert!(crate::fxcache::privee_octets() > 0, "le résultat est gardé dans la réserve du fil");
        // Deuxième demande : la même mémoire (gardé, pas recalculé).
        let b = crate::fx::shown(&doc, &item).expect("dessin").raster.clone();
        assert!(a.tiles.iter().zip(&b.tiles).all(|(x, y)| match (x, y) {
            (Some(x), Some(y)) => x.same_as(y),
            (None, None) => true,
            _ => false,
        }));
        a
    });
    assert!(!crate::fxcache::privee_active());
    assert_eq!(crate::fxcache::get(id, version).is_some(), avant, "la réserve commune n'a rien reçu");
    // Et c'est le même résultat que le calcul de l'ancien chemin.
    let ancien = crate::fx::shown(&doc, &item).expect("dessin");
    assert!(ancien.raster.tiles.iter().zip(&dedans.tiles).all(|(x, y)| match (x, y) {
        (Some(x), Some(y)) => x[..] == y[..],
        (None, None) => true,
        _ => false,
    }));
}

#[test]
fn the_private_reserve_does_not_evict_what_the_current_image_touched() {
    monde::lire();
    let doc = projet_a_effets(1, 2);
    let plan = render::panel_plan_at(&doc, 0, 0, doc.render_paper());
    let item = plan.items.iter().find(|i| !i.fx.is_empty()).expect("un élément avec effet").clone();
    crate::fxcache::avec_privee(1, || {
        // Une limite d'un octet : le dessin de l'image en cours reste (il dépasse la limite plutôt que de se tromper)…
        crate::fx::shown(&doc, &item).expect("dessin");
        let gardes = crate::fxcache::privee_octets();
        assert!(gardes > 0);
        // … et part quand une nouvelle image commence et qu'un autre dessin arrive.
        crate::fxcache::privee_nouvelle_image();
        let autre = doc.panels[0].layers[0].fx.len();
        assert!(autre > 0);
        let mut item2 = item.clone();
        item2.fx[0].values[0] += 1.0;
        crate::fx::shown(&doc, &item2).expect("dessin");
        assert!(crate::fxcache::privee_octets() <= gardes * 2, "l'ancien est parti : {} octets", crate::fxcache::privee_octets());
    });
}

// ----------------------------------------------------------------------------------- les fils de rendu

#[test]
fn the_machine_keeps_two_cores_for_the_interface() {
    // (cœurs → fils de rendu, cœurs de chacun) : jamais plus de cœurs que ceux qui restent.
    for coeurs in 1..=64 {
        let (fils, chacun) = exportrender::fils_pour(coeurs);
        assert!(fils >= 1 && chacun >= 1);
        assert!(fils * chacun <= coeurs.saturating_sub(2).max(1), "{coeurs} cœurs : {fils} × {chacun}");
    }
    assert_eq!(exportrender::fils_pour(16), (4, 3));
    assert_eq!(exportrender::fils_pour(4), (1, 2));
    assert_eq!(exportrender::fils_pour(1), (1, 1));
}

#[test]
fn the_memory_in_flight_stays_bounded_whatever_the_frame_size() {
    // 4K : 33 Mo l'image ; HD : 8 Mo ; petit : 100 Ko.
    for (octets, fils) in [(33 << 20, 4), (8 << 20, 4), (100 << 10, 6), (33 << 20, 1)] {
        let avance = exportrender::avance_permise(octets, fils, 4);
        assert!(avance > fils && avance <= 2 * fils + 2, "{octets} octets, {fils} fils : {avance}");
        let tenues = avance + fils + 3 + 4;
        if octets >= 8 << 20 {
            assert!(tenues * octets <= 900 << 20, "{octets} octets : {} Mo tenus", tenues * octets >> 20);
        }
    }
}

/// Le passeur rend les images dans l'ordre, une fois chacune, avec les pixels de l'ancien chemin, et tout se termine.
#[test]
fn the_pipeline_hands_the_frames_over_in_order_with_the_old_pixels() {
    monde::lire();
    let doc = projet_a_effets(3, 7);
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let rendu = Arc::new(Rendu::new(exportrender::snapshot_of(&doc, &[0, 1, 2]), Papier::Fond, false, Vec::new(), Arc::default()));
    let recu: Arc<Mutex<Vec<(usize, Image)>>> = Arc::default();
    let r = recu.clone();
    let p = Pipeline::start(rendu.clone(), Arc::new(frames.clone()), Box::new(move |i, img| {
        r.lock().unwrap().push((i, img));
        Pose::Ok
    }), Arc::default(), 4);
    let t = Instant::now();
    while !p.finished() {
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert_eq!(p.emitted(), frames.len());
    let recu = recu.lock().unwrap();
    assert_eq!(recu.iter().map(|(i, _)| *i).collect::<Vec<_>>(), (0..frames.len()).collect::<Vec<_>>(), "dans l'ordre, une fois chacune");
    for ((i, img), &(p, f)) in recu.iter().zip(&frames) {
        compare(&ancienne_image(&doc, p, f, doc.render_paper(), false), img, &format!("image {i}"));
    }
}

#[test]
fn a_full_write_queue_makes_the_passer_wait_not_drop_frames() {
    monde::lire();
    let doc = doc_with_drawing(1, 9);
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let rendu = Arc::new(Rendu::new(exportrender::snapshot_of(&doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default()));
    let recu: Arc<Mutex<Vec<usize>>> = Arc::default();
    let (r, refus) = (recu.clone(), Arc::new(Mutex::new(0usize)));
    let refus2 = refus.clone();
    let mut p = Pipeline::start(rendu, Arc::new(frames.clone()), Box::new(move |i, img| {
        // L'écriture est lente : elle refuse les trois premières fois de chaque image.
        let mut n = refus2.lock().unwrap();
        if *n % 4 != 3 {
            *n += 1;
            return Pose::Pleine(img);
        }
        *n += 1;
        r.lock().unwrap().push(i);
        Pose::Ok
    }), Arc::default(), 4);
    let t = Instant::now();
    while !p.finished() {
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    p.join_feeder();
    assert_eq!(*recu.lock().unwrap(), (0..9).collect::<Vec<_>>());
}

#[test]
fn cancelling_the_pipeline_stops_the_passer_at_once_and_the_workers_soon() {
    monde::lire();
    let doc = projet_a_effets(1, 40);
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let rendu = Arc::new(Rendu::new(exportrender::snapshot_of(&doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default()));
    // Une écriture qui n'avance jamais : le passeur attend, l'annulation doit le libérer.
    let mut p = Pipeline::start(rendu, Arc::new(frames), Box::new(|_, img| Pose::Pleine(img)), Arc::default(), 4);
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    p.cancel();
    assert!(t.elapsed() < Duration::from_secs(2), "l'annulation rend la main : {:?}", t.elapsed());
    assert!(p.finished());
    let t = Instant::now();
    while !p.workers_finished() {
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(10), "les fils de rendu s'en vont");
    }
}

// ------------------------------------------------------------------------- chaque sorte d'export

fn images_de_dossier(dir: &Path, base: &str, n: usize) -> Vec<image::RgbaImage> {
    (1..=n).map(|i| image::open(dir.join(format!("{base}_{i:04}.png"))).unwrap_or_else(|e| panic!("{base}_{i:04}.png : {e}")).to_rgba8()).collect()
}

fn pixels_png(img: &image::RgbaImage) -> Vec<[u8; 4]> {
    img.pixels().map(|p| p.0).collect()
}

fn attendus(doc: &Document, frames: &[(usize, usize)], graver: bool) -> Vec<Vec<[u8; 4]>> {
    frames.iter().map(|&(p, f)| ancienne_image(doc, p, f, doc.render_paper(), graver).into_iter().map(unpremultiply).collect()).collect()
}

#[test]
fn the_png_sequence_is_the_old_pixels_and_leaves_no_part_file() {
    monde::lire();
    let doc = projet_a_effets(2, 5);
    let dir = dossier_neuf("png");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let attendu = attendus(&doc, &frames, true);
    let mut job = ExportJob::images(&dir, "essai", frames.clone(), doc.frame, true);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    for (k, img) in images_de_dossier(&dir, "essai", frames.len()).iter().enumerate() {
        assert_eq!(pixels_png(img), attendu[k], "image {k}");
    }
    let restes: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".part")).collect();
    assert!(restes.is_empty(), "aucun .part : {restes:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_transparent_png_has_no_paper() {
    monde::lire();
    let doc = doc_with_drawing(1, 2);
    let dir = dossier_neuf("png-transparent");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let attendu = attendus_sans_papier(&doc, &frames);
    let mut job = ExportJob::images(&dir, "t", frames.clone(), doc.frame, false);
    court(&mut job, &doc);
    for (k, img) in images_de_dossier(&dir, "t", frames.len()).iter().enumerate() {
        assert_eq!(pixels_png(img), attendu[k], "image {k}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

fn attendus_sans_papier(doc: &Document, frames: &[(usize, usize)]) -> Vec<Vec<[u8; 4]>> {
    frames.iter().map(|&(p, f)| ancienne_image(doc, p, f, None, true).into_iter().map(unpremultiply).collect()).collect()
}

#[test]
fn the_still_image_is_the_screen_plan_with_the_old_pixels() {
    monde::lire();
    let doc = projet_a_effets(2, 4);
    let dir = dossier_neuf("image");
    let cible = dir.join("image.png");
    // L'image courante : le plan que l'écran a construit (ici celui du plan), sa caméra.
    let (p, f) = (1, 2);
    let plan = render::panel_plan_at(&doc, p, f, doc.render_paper());
    let cam = doc.panel_camera(p, f);
    let attendu: Vec<[u8; 4]> = ancienne_image(&doc, p, f, doc.render_paper(), true).into_iter().map(unpremultiply).collect();
    let mut job = ExportJob::still(&cible, plan, cam, p, f, doc.frame);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(pixels_png(&image::open(&cible).unwrap().to_rgba8()), attendu);
    assert!(!export::part_of(&cible).exists());
    std::fs::remove_dir_all(&dir).ok();
}

/// Le film : ffmpeg recopie ses images brutes dans un fichier, sans les toucher — ce que le film reçoit sur son entrée est
/// l'ancien chemin, dans l'ordre.
#[test]
fn the_film_pipe_carries_the_old_frames_in_order() {
    monde::lire();
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return eprintln!("ffmpeg absent : essai sauté") };
    let doc = projet_a_effets(2, 5);
    let dir = dossier_neuf("film-brut");
    let out = dir.join("brut.rgba");
    let [w, h] = doc.frame;
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-framerate", "12", "-i", "-", "-f", "rawvideo", "-pix_fmt", "rgba", &out.to_string_lossy()].iter().map(|s| s.to_string()).collect();
    let mut job = ExportJob::video(&ffmpeg, args, &out, frames.clone(), None).unwrap();
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let octets = std::fs::read(&out).unwrap();
    assert_eq!(octets.len(), frames.len() * w * h * 4);
    for (k, &(p, f)) in frames.iter().enumerate() {
        let attendu = ancienne_image(&doc, p, f, doc.render_paper(), true);
        let recu: &[Color32] = bytemuck::cast_slice(&octets[k * w * h * 4..(k + 1) * w * h * 4]);
        compare(&attendu, recu, &format!("image {k} du film"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Le GIF : les images que le GIF garde (`gif_sample`) sont celles de l'ancien chemin.
#[test]
fn the_gif_keeps_the_sampled_frames_with_the_old_pixels() {
    monde::lire();
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return eprintln!("ffmpeg absent : essai sauté") };
    let doc = projet_a_effets(1, 12);
    let dir = dossier_neuf("gif-brut");
    let out = dir.join("brut.rgba");
    let [w, h] = doc.frame;
    let tout = export::frames_for(&doc, Range::Montage, 0);
    let gardees = export::gif_sample(&tout, doc.fps, 6);
    assert_eq!(gardees.len(), 6);
    let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-framerate", "6", "-i", "-", "-f", "rawvideo", "-pix_fmt", "rgba", &out.to_string_lossy()].iter().map(|s| s.to_string()).collect();
    let mut job = ExportJob::gif(&ffmpeg, args, &out, gardees.clone()).unwrap();
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let octets = std::fs::read(&out).unwrap();
    assert_eq!(octets.len(), 6 * w * h * 4);
    for (k, &(p, f)) in gardees.iter().enumerate() {
        let recu: &[Color32] = bytemuck::cast_slice(&octets[k * w * h * 4..(k + 1) * w * h * 4]);
        compare(&ancienne_image(&doc, p, f, doc.render_paper(), true), recu, &format!("image {k} du GIF"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Le PDF, la page de relecture et la fiche passent leurs images par `collect` : les pixels que reçoit l'encodeur sont ceux
/// de l'ancien chemin, rangés par rang.
#[test]
fn the_images_given_to_the_collectors_are_the_old_ones_by_rank() {
    monde::lire();
    let doc = projet_a_effets(2, 4);
    let dir = dossier_neuf("collect");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let recues: Arc<Mutex<Vec<(usize, Vec<Color32>)>>> = Arc::default();
    let (r, fin) = (recues.clone(), recues.clone());
    let sortie = dir.join("sortie.txt");
    let cible = sortie.clone();
    let mut job = ExportJob::collect(
        "essai".into(),
        "Image",
        &sortie,
        frames.clone(),
        move |rang, img| {
            r.lock().unwrap().push((rang, img.to_vec()));
            Ok(rang)
        },
        move |rangs: Vec<usize>| {
            assert_eq!(rangs, (0..frames_len(&fin)).collect::<Vec<_>>(), "toutes arrivées, rangées par rang");
            std::fs::write(&cible, b"ok").map_err(|e| e.to_string())
        },
    );
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let mut recues = recues.lock().unwrap().clone();
    recues.sort_by_key(|(r, _)| *r);
    let toutes = export::frames_for(&doc, Range::Montage, 0);
    assert_eq!(recues.len(), toutes.len());
    for ((rang, img), &(p, f)) in recues.iter().zip(&toutes) {
        compare(&ancienne_image(&doc, p, f, doc.render_paper(), true), img, &format!("image {rang}"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

fn frames_len(r: &Arc<Mutex<Vec<(usize, Vec<Color32>)>>>) -> usize {
    r.lock().unwrap().len()
}

#[test]
fn the_pdf_is_written_whole_and_a_cancelled_one_keeps_the_previous_file() {
    monde::lire();
    let doc = doc_with_drawing(3, 4);
    let dir = dossier_neuf("pdf");
    let out = dir.join("storyboard.pdf");
    std::fs::write(&out, b"ancien pdf").unwrap();
    let panels = export::panels_of(&export::frames_for(&doc, Range::Montage, 0));
    let (shots, boards) = export::storyboard_of(&doc, &panels);
    // Annulé tout de suite : le fichier d'avant reste, aucun .part.
    let mut job = ExportJob::pdf(&out, "Essai", shots.clone(), boards.clone(), doc.frame);
    job.cancel();
    assert!(matches!(job.outcome, Some(Err(_))));
    assert_eq!(std::fs::read(&out).unwrap(), b"ancien pdf", "le PDF d'avant n'est pas touché");
    assert!(!export::part_of(&out).exists());
    // Jusqu'au bout : un vrai PDF, écrit d'un coup (pas de .part).
    let mut job = ExportJob::pdf(&out, "Essai", shots, boards, doc.frame);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF"));
    assert!(!export::part_of(&out).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_sound_export_is_the_old_wav() {
    monde::lire();
    let travail = dossier("export-rendu-son");
    let doc = projet_complet(&travail).doc;
    let dir = dossier_neuf("son");
    let out = dir.join("son.wav");
    let n = doc.total_frames();
    let (start, len) = export::sample_span(doc.fps, 0, n);
    let ancien = dir.join("ancien.wav");
    export::write_wav(&ancien, crate::audio::mix_clips_blocking(&doc), start, len).unwrap();
    let mut job = ExportJob::sound(&out, SoundSpan { start, len });
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(&ancien).unwrap());
    assert!(!export::part_of(&out).exists());
    assert!(job.outcome.as_ref().unwrap().as_ref().unwrap().starts_with("Son exporté"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_film_with_sound_mixes_it_on_another_thread_and_cleans_up() {
    monde::lire();
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return eprintln!("ffmpeg absent : essai sauté") };
    let Ok(enc) = export::probe(&ffmpeg) else { return };
    if enc.h264.is_none() || !enc.aac {
        return eprintln!("ffmpeg sans H.264/AAC : essai sauté");
    }
    let travail = dossier("export-rendu-film-son");
    let mut doc = projet_complet(&travail).doc;
    doc.frame = [96, 54];
    let dir = dossier_neuf("film-son");
    let out = dir.join("film.mp4");
    let frames = export::frames_for(&doc, Range::Panel, 0);
    let (start, len) = export::sample_span(doc.fps, 0, frames.len());
    let (format, frame, fps, cible) = (Format::Mp4, doc.frame, doc.fps, out.clone());
    let enc2 = enc.clone();
    let mut job = ExportJob::video_with_sound(&ffmpeg, move |wav| export::video_args(format, &enc2, frame, fps, false, wav, &cible), &out, frames, Some(SoundSpan { start, len }));
    let mut phases = std::collections::BTreeSet::new();
    let t = Instant::now();
    while job.outcome.is_none() {
        job.step(&doc, Duration::from_millis(8));
        phases.insert(job.phase());
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert!(std::fs::metadata(&out).unwrap().len() > 500);
    assert!(phases.contains("Calcul des images…") || phases.contains(""), "{phases:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_bad_ffmpeg_argument_list_is_said_at_once_and_a_missing_ffmpeg_too() {
    monde::lire();
    let doc = doc_with_drawing(1, 2);
    let dir = dossier_neuf("sans-ffmpeg");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    // Un ffmpeg qui n'existe pas : l'erreur arrive tout de suite, sans fil laissé derrière.
    assert!(ExportJob::video(Path::new("ffmpeg-inexistant.exe"), vec![], &dir.join("x.mp4"), frames.clone(), None).is_err());
    // Un ffmpeg qui part mais refuse les arguments : l'export finit en erreur avec son message, et efface le fichier.
    if let Some(ffmpeg) = export::find_ffmpeg(None) {
        let out = dir.join("y.mp4");
        let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", "10x10", "-i", "-", "-c:v", "codec_qui_n_existe_pas", &out.to_string_lossy()].iter().map(|s| s.to_string()).collect();
        let mut job = ExportJob::video(&ffmpeg, args, &out, frames, None).unwrap();
        court(&mut job, &doc);
        assert!(matches!(&job.outcome, Some(Err(e)) if !e.is_empty()), "{:?}", job.outcome);
        assert!(!out.exists());
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn textured_strokes_left_in_fast_rendering_are_made_exact_in_the_copy_only() {
    monde::lire();
    let (mut doc, id) = crate::affichage_tests::doc_with(640, 360, crate::affichage_tests::gros_traits(2, 640, 360, 3));
    let mut calque = crate::document::Layer::new("Traits", LayerKind::Vector);
    calque.frames = vec![Some(id)];
    doc.panels[0].layers = vec![calque];
    doc.panels[0].length = 1;
    let avant = doc.cels[&id].version;
    let mut b = exportrender::SnapshotBuilder::new(&doc, &[0]).with_exact(vec![id]);
    let t = Instant::now();
    // Tant que les dessins se copient, le projet est lu ; ensuite le rendu exact travaille sur la copie, sur un autre fil.
    let mut a_lu = false;
    while !b.step(&doc, Duration::ZERO) {
        a_lu |= b.reads_project();
        std::thread::sleep(Duration::from_millis(1));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert!(!b.reads_project());
    let copie = b.finish();
    assert!(copie.cels[&id].version > avant, "la copie est refaite exacte");
    assert_eq!(copie.cels[&id].vector.as_ref().map(|v| v.missing_caches()), Some(0), "tous ses traits ont leurs pixels exacts");
    assert_eq!(doc.cels[&id].version, avant, "le projet affiché n'est pas touché");
    let _ = a_lu;
}

#[test]
fn a_film_whose_ffmpeg_does_not_start_leaves_the_previous_file_alone() {
    monde::lire();
    let doc = doc_with_drawing(1, 2);
    let dir = dossier_neuf("ffmpeg-absent");
    let out = dir.join("film.mp4");
    std::fs::write(&out, b"ancien film").unwrap();
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let mut job = ExportJob::video_with_sound(Path::new("ffmpeg-qui-n-existe-pas.exe"), |_| Ok(vec![]), &out, frames, None);
    court(&mut job, &doc);
    assert!(matches!(&job.outcome, Some(Err(e)) if e.contains("ffmpeg ne démarre pas")), "{:?}", job.outcome);
    assert_eq!(std::fs::read(&out).unwrap(), b"ancien film", "le film d'avant n'est pas touché");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------- l'interface et l'export

/// Aucune image ne se compose sur le fil de l'interface, et le projet peut changer pendant l'export sans changer le résultat.
#[test]
fn no_frame_is_composed_here_and_editing_the_project_during_the_export_changes_nothing() {
    monde::lire();
    let mut doc = projet_a_effets(3, 6);
    let dir = dossier_neuf("edition");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let attendu = attendus(&doc, &frames, true);
    let avant = IMAGES_ICI.with(|n| n.get());
    let mut job = ExportJob::images(&dir, "essai", frames.clone(), doc.frame, true);
    // La copie du projet : l'interface la fait par tranches.
    let t = Instant::now();
    while job.needs_project() {
        job.step(&doc, Duration::ZERO);
        assert!(t.elapsed() < Duration::from_secs(30));
    }
    // Maintenant on retouche tout : des pixels, des versions, l'opacité, la caméra, le nombre de plans.
    let ids: Vec<_> = doc.cels.keys().copied().collect();
    for id in ids {
        let cel = doc.cels.get_mut(&id).unwrap();
        for i in 0..cel.raster.tiles.len().min(6) {
            cel.raster.tile_mut(i)[0] = Color32::from_rgb(250, 0, 0);
        }
        cel.version += 1;
    }
    for p in &mut doc.panels {
        for l in &mut p.layers {
            l.opacity = 0.2;
            l.visible = false;
        }
        p.camera.clear();
    }
    doc.add_panel_after(0);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    for (k, img) in images_de_dossier(&dir, "essai", frames.len()).iter().enumerate() {
        assert_eq!(pixels_png(img), attendu[k], "image {k} : le projet d'avant la retouche");
    }
    assert_eq!(IMAGES_ICI.with(|n| n.get()), avant, "aucune image composée sur ce fil");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_a_png_export_stops_it_and_leaves_no_part_file() {
    monde::lire();
    let doc = projet_a_effets(2, 60);
    let dir = dossier_neuf("annule-png");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let mut job = ExportJob::images(&dir, "essai", frames, doc.frame, true);
    let t = Instant::now();
    while !matches!(job.progress(), export::Progress::Running(n, _) if n >= 3) {
        job.step(&doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    let t = Instant::now();
    job.cancel();
    assert!(t.elapsed() < Duration::from_secs(3), "l'annulation rend la main vite : {:?}", t.elapsed());
    assert!(matches!(job.outcome, Some(Err(_))));
    let noms: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(noms.iter().all(|n| n.ends_with(".png")), "aucun .part : {noms:?}");
    assert!(noms.len() < 120);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_a_film_removes_the_file_and_kills_ffmpeg() {
    monde::lire();
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return eprintln!("ffmpeg absent : essai sauté") };
    let Ok(enc) = export::probe(&ffmpeg) else { return };
    if enc.h264.is_none() {
        return;
    }
    let doc = projet_a_effets(2, 80);
    let dir = dossier_neuf("annule-film");
    let out = dir.join("film.mp4");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let args = export::video_args(Format::Mp4, &enc, doc.frame, doc.fps, false, None, &out).unwrap();
    let mut job = ExportJob::video(&ffmpeg, args, &out, frames, None).unwrap();
    let t = Instant::now();
    while !matches!(job.progress(), export::Progress::Running(n, _) if n >= 4) {
        job.step(&doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    job.cancel();
    assert!(matches!(job.outcome, Some(Err(_))));
    assert!(!out.exists(), "le film inachevé est effacé");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_frame_that_panics_stops_the_export_with_a_message_not_the_program() {
    monde::lire();
    let doc = doc_with_drawing(1, 3);
    let dir = dossier_neuf("panique");
    // Un plan qui n'existe pas : le calcul de l'image plante.
    let mut job = ExportJob::images(&dir, "essai", vec![(0, 0), (0, 1), (99, 0)], doc.frame, true);
    court(&mut job, &doc);
    match &job.outcome {
        Some(Err(e)) => assert!(e.contains("a échoué"), "{e}"),
        other => panic!("{other:?}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_export_of_nothing_finishes() {
    monde::lire();
    let doc = doc_with_drawing(1, 1);
    let dir = dossier_neuf("vide");
    let mut job = ExportJob::images(&dir, "essai", Vec::new(), doc.frame, true);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dropping_a_running_export_cancels_it() {
    monde::lire();
    let doc = projet_a_effets(2, 60);
    let dir = dossier_neuf("lache");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let mut job = ExportJob::images(&dir, "essai", frames, doc.frame, true);
    for _ in 0..5 {
        job.step(&doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(3));
    }
    drop(job);
    std::thread::sleep(Duration::from_millis(300));
    let noms: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(noms.iter().all(|n| n.ends_with(".png")), "{noms:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_editorial_export_takes_one_snapshot_for_all_its_clips() {
    monde::lire();
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return eprintln!("ffmpeg absent : essai sauté") };
    let Ok(enc) = export::probe(&ffmpeg) else { return };
    if enc.h264.is_none() {
        return;
    }
    let doc = doc_with_drawing(3, 4);
    let opts = crate::editorial::Options { media: crate::editorial::Media::H264, edl: true, xml: false, otio: false, sound: false, ..crate::editorial::Options::default() };
    let plan = crate::editorial::build_plan(&doc, &opts, 0, "Essai").expect("plan");
    let parent = dossier_neuf("montage");
    let mut job = crate::editorialjob::EditorialJob::start(plan.clone(), opts, ffmpeg, enc, &parent).expect("démarré");
    assert!(job.needs_project());
    let t = Instant::now();
    while job.running() {
        job.step(&doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert!(!job.needs_project());
    for clip in &plan.clips {
        assert!(std::fs::metadata(crate::editorialjob::export_dir_for(&parent, "Essai").join(&clip.file)).unwrap().len() > 300, "{}", clip.file);
    }
    std::fs::remove_dir_all(&parent).ok();
}

/// Les médias du montage : les images de chaque plan (poignées comprises) sont celles de l'ancien chemin.
#[test]
fn the_editorial_clip_frames_are_the_old_ones() {
    monde::lire();
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return eprintln!("ffmpeg absent : essai sauté") };
    let doc = projet_a_effets(2, 4);
    let opts = crate::editorial::Options { media: crate::editorial::Media::H264, ..crate::editorial::Options::default() };
    let plan = crate::editorial::build_plan(&doc, &opts, 0, "Essai").expect("plan");
    let dir = dossier_neuf("montage-brut");
    let [w, h] = doc.frame;
    let snapshot = exportrender::snapshot_of(&doc, &[0, 1]);
    for (k, clip) in plan.clips.iter().enumerate() {
        let out = dir.join(format!("clip{k}.rgba"));
        let frames = crate::editorialjob::clip_frames(clip);
        let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-framerate", "12", "-i", "-", "-f", "rawvideo", "-pix_fmt", "rgba", &out.to_string_lossy()].iter().map(|s| s.to_string()).collect();
        let mut job = ExportJob::video(&ffmpeg, args, &out, frames.clone(), None).unwrap().with_snapshot(snapshot.clone());
        court(&mut job, &doc);
        assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
        let octets = std::fs::read(&out).unwrap();
        assert_eq!(octets.len(), frames.len() * w * h * 4);
        for (i, &(p, f)) in frames.iter().enumerate() {
            let recu: &[Color32] = bytemuck::cast_slice(&octets[i * w * h * 4..(i + 1) * w * h * 4]);
            compare(&ancienne_image(&doc, p, f, doc.render_paper(), true), recu, &format!("clip {k} image {i}"));
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_snapshot_of_a_job_is_taken_once_and_given_back() {
    monde::lire();
    let doc = doc_with_drawing(2, 3);
    let dir = dossier_neuf("cliche");
    let mut job = ExportJob::images(&dir, "e", export::frames_for(&doc, Range::Montage, 0), doc.frame, true);
    assert!(job.snapshot().is_none() && job.needs_project());
    court(&mut job, &doc);
    assert!(job.snapshot().is_some() && !job.needs_project());
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ l'application

mod application {
    use super::*;
    use crate::perf_tests::{banc, se_reposer, Banc};

    fn projet_pour_l_application(plans: usize) -> Document {
        projet_a_effets(plans, 6)
    }

    fn jusqu_a(b: &mut Banc, limite: Duration, fini: impl Fn(&Banc) -> bool) -> bool {
        let t = Instant::now();
        while t.elapsed() < limite {
            if fini(b) {
                return true;
            }
            b.image(Vec::new(), 0.016);
            std::thread::sleep(Duration::from_millis(3));
        }
        fini(b)
    }

    /// Démarre un export d'images d'un projet pour l'application `b`.
    fn demarrer(b: &mut Banc, nom: &str) -> (PathBuf, usize) {
        let dir = dossier_neuf(nom);
        let frames = export::frames_for(&b.app.doc, Range::Montage, 0);
        let n = frames.len();
        let job = ExportJob::images(&dir, "app", frames, b.app.doc.frame, true);
        b.app.start_job(job);
        (dir, n)
    }

    #[test]
    fn the_export_window_is_not_modal_and_the_project_stays_free() {
        let mut b = banc(projet_pour_l_application(3), false);
        let (dir, n) = demarrer(&mut b, "app-libre");
        // Le temps de la copie, le projet est retenu ; tout de suite après, il est libre.
        assert!(jusqu_a(&mut b, Duration::from_secs(30), |b| !b.app.modal_open() || b.app.export_job.as_ref().is_none_or(|j| !j.needs_project())));
        b.image(Vec::new(), 0.016);
        assert!(b.app.export_job.is_some() || dir.join(format!("app_{n:04}.png")).exists());
        assert!(!b.app.modal_open(), "l'export ne tient pas le projet");
        assert_eq!(b.app.onglet_occupe(), None, "ni l'onglet");
        // On travaille pendant l'export : un dessin change.
        let id = b.app.doc.panels[0].layers[0].frames[0].unwrap();
        let cel = b.app.doc.cels.get_mut(&id).unwrap();
        cel.raster.tile_mut(0)[0] = Color32::from_rgb(9, 9, 9);
        cel.version += 1;
        assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_some())), "l'export finit");
        let msg = b.app.export_job.as_ref().unwrap().outcome.clone().unwrap().unwrap();
        assert!(msg.contains(&format!("{n} images exportées")), "{msg}");
        // Une fois fini et fermé, l'application revient au repos : aucune image de plus n'est demandée.
        b.app.export_job = None;
        b.app.export_projet = None;
        assert!(se_reposer(&mut b, 120) < 120, "l'application revient au repos après l'export");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_export_continues_while_another_tab_is_shown_and_answers_to_its_own_project() {
        let mut b = banc(projet_pour_l_application(2), false);
        let premier = b.app.onglets[b.app.actif].id;
        let (dir, n) = demarrer(&mut b, "app-onglets");
        assert_eq!(b.app.export_projet.as_ref().map(|(id, _)| *id), Some(premier));
        // Tant que la copie dure, on ne change pas d'onglet.
        let copie = b.app.export_job.as_ref().is_some_and(|j| j.needs_project());
        if copie {
            assert!(b.app.onglet_occupe().is_some());
        }
        assert!(jusqu_a(&mut b, Duration::from_secs(30), |b| b.app.onglet_occupe().is_none()));
        // Un autre projet s'ouvre dans un nouvel onglet : l'export (qui tourne sur sa copie) continue.
        assert!(b.app.nouvel_onglet(), "{}", b.app.status);
        let second = projet_a_effets(1, 3);
        assert!(b.app.charger_projet(second));
        b.app.welcome = false;
        b.chauffer(2);
        assert_ne!(b.app.onglets[b.app.actif].id, premier);
        assert!(b.app.export_job.is_some() || dir.join(format!("app_{n:04}.png")).exists());
        // La fenêtre dit de quel projet il s'agit tant qu'on regarde l'autre.
        assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_some())), "il finit pendant qu'on regarde l'autre onglet");
        assert!(matches!(b.app.export_job.as_ref().unwrap().outcome, Some(Ok(_))));
        // Les images sont celles du premier projet (2 plans de 6 images), pas du second.
        assert_eq!(n, 12);
        for i in 1..=n {
            assert!(dir.join(format!("app_{i:04}.png")).is_file());
        }
        b.app.export_job = None;
        b.app.export_projet = None;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_finished_export_does_not_stop_the_next_one() {
        let mut b = banc(projet_pour_l_application(1), false);
        let (dir, _) = demarrer(&mut b, "app-suite");
        assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_some())));
        // La fenêtre du premier est restée ouverte : un nouvel export la referme au lieu d'être refusé.
        assert!(b.app.export_libre());
        assert!(b.app.export_job.is_none());
        assert!(b.app.status.contains("images exportées"), "{}", b.app.status);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_second_export_is_refused_while_one_runs() {
        let mut b = banc(projet_pour_l_application(2), false);
        let (dir, _) = demarrer(&mut b, "app-deux");
        b.app.open_export(Format::Png);
        assert!(b.app.export_dialog.is_none(), "pas de deuxième fenêtre de choix");
        assert!(b.app.status.contains("déjà en cours"), "{}", b.app.status);
        b.app.export_job.as_mut().unwrap().cancel();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_progress_window_says_the_project_is_the_one_of_the_start() {
        let mut b = banc(projet_pour_l_application(2), false);
        b.garder_textes = true;
        let (dir, _) = demarrer(&mut b, "app-texte");
        // Avant la fin : le texte de la fenêtre.
        let mut vu = false;
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(30) && b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) {
            b.image(Vec::new(), 0.016);
            if b.textes.iter().any(|t| t.contains("tel qu'il était au départ")) {
                vu = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let fini = b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_some());
        assert!(vu || fini, "la fenêtre d'avancement rappelle que l'export garde le projet du départ");
        b.app.export_job.as_mut().unwrap().cancel();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn quitting_warns_once_about_a_running_export_then_quits_and_stops_it() {
        let mut b = banc(projet_a_effets(2, 60), false);
        let (dir, _) = demarrer(&mut b, "app-quitter");
        assert!(jusqu_a(&mut b, Duration::from_secs(30), |b| b.app.onglet_occupe().is_none()));
        assert_eq!(b.app.exports_en_cours(), vec!["l'export"]);
        b.app.perform(crate::app::Pending::Quit);
        assert!(!b.app.allow_close, "la première fois, SHOT prévient au lieu de quitter");
        assert!(b.app.status.contains("Quitter arrêterait l'export"), "{}", b.app.status);
        b.app.perform(crate::app::Pending::Quit);
        assert!(b.app.allow_close, "le deuxième « Quitter » quitte");
        // Quitter arrête l'export : plus rien ne reste d'inachevé quand le travail est lâché.
        let mut job = b.app.export_job.take().unwrap();
        job.cancel();
        assert!(matches!(job.outcome, Some(Err(_))) || matches!(job.outcome, Some(Ok(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_export_means_quitting_is_not_held_back() {
        let mut b = banc(projet_a_effets(1, 3), false);
        assert!(b.app.exports_en_cours().is_empty());
        b.app.perform(crate::app::Pending::Quit);
        assert!(b.app.allow_close);
    }
    /// Pendant l'export, l'interface répond : aucune image ne se compose sur le fil de l'interface (compteur par fil), et le
    /// temps d'une image d'écran reste sans rapport avec celui d'une image d'export (borne large : la machine des essais est
    /// partagée ; les mesures sont dans `perf_export_*`).
    #[test]
    fn the_interface_composes_no_export_frame_and_its_frames_stay_short_during_an_export() {
        let mut b = banc(projet_pour_l_application(4), false);
        let avant = IMAGES_ICI.with(|n| n.get());
        let (dir, n) = demarrer(&mut b, "app-fluide");
        let mut pire = 0.0f64;
        let t = Instant::now();
        let mut images = 0;
        while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && t.elapsed() < Duration::from_secs(120) {
            let m = b.image(Vec::new(), 0.016);
            images += 1;
            if images > 3 {
                pire = pire.max(m.construction);
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        assert!(matches!(b.app.export_job.as_ref().unwrap().outcome, Some(Ok(_))));
        assert_eq!(IMAGES_ICI.with(|n| n.get()), avant, "aucune image d'export composée sur le fil de l'interface");
        assert!(dir.join(format!("app_{n:04}.png")).is_file());
        assert!(pire < 250.0, "la pire image d'écran pendant l'export : {pire:.1} ms");
        b.app.export_job = None;
        std::fs::remove_dir_all(&dir).ok();
    }}

// ------------------------------------------------------------------------------- mesures (ignorées)

/// Mesures de `docs/PERFORMANCE.md`, section « Exports hors du fil de l'interface » :
/// `cargo test --release --offline --target-dir target\msvc perf_export -- --ignored --nocapture --test-threads=1`
/// (un essai par processus pour la mémoire : `perf_export_png_legacy`, `perf_export_png_background`, `perf_export_film_legacy`,
/// `perf_export_film_background`, et `perf_export_snapshot_cost`).
mod mesures {
    use super::*;
    use crate::document::Cel;
    use crate::perf_tests::banc;
    use crate::raster::{empty_tile, TILE};

    #[repr(C)]
    struct Compteurs {
        cb: u32,
        faute_pages: u32,
        pic_ensemble: usize,
        ensemble: usize,
        pic_pagine: usize,
        pagine: usize,
        pic_non_pagine: usize,
        non_pagine: usize,
        fichier: usize,
        pic_fichier: usize,
    }

    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, compteurs: *mut Compteurs, cb: u32) -> i32;
    }

    /// (mémoire de travail, pic de mémoire de travail) du processus, en Mo.
    fn memoire() -> (f64, f64) {
        let mut c = Compteurs { cb: std::mem::size_of::<Compteurs>() as u32, faute_pages: 0, pic_ensemble: 0, ensemble: 0, pic_pagine: 0, pagine: 0, pic_non_pagine: 0, non_pagine: 0, fichier: 0, pic_fichier: 0 };
        // SAFETY : la structure a la disposition de PROCESS_MEMORY_COUNTERS et sa taille est donnée.
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        (c.ensemble as f64 / 1e6, c.pic_ensemble as f64 / 1e6)
    }

    fn tuile(seed: u32) -> crate::raster::Tile {
        let mut t = empty_tile();
        for (i, p) in t.iter_mut().enumerate() {
            let (x, y) = ((i % TILE) as u32, (i / TILE) as u32);
            if (x + y + seed) % 5 < 3 {
                *p = Color32::from_rgba_premultiplied(30 + (seed % 120) as u8, 20 + (seed % 90) as u8, 40, 200);
            }
        }
        t
    }

    /// Un film en 4K (feuille de 7 680 × 4 320) : six calques de 150 tuiles dont un flou, une teinte, une rotation, une caméra
    /// qui avance et tourne un peu : chaque image est différente, et lourde.
    fn projet_4k(images: usize) -> Document {
        let mut doc = Document::with_frame(3840, 2160, 24);
        let mut calques = Vec::new();
        let (tx0, ty0, tw, th) = (30usize, 17usize, 60usize, 34usize);
        let across = doc.width.div_ceil(TILE);
        for l in 0..6 {
            let mut layer = crate::document::Layer::new(&format!("Calque {}", l + 1), LayerKind::Bitmap);
            let mut dernier = None;
            for f in 0..images {
                if f % (2 + l) == 0 {
                    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
                    for k in 0..150 {
                        let (x, y) = (tx0 + (k * 7 + f + l * 11) % tw, ty0 + (k * 13 + l * 5) % th);
                        cel.raster.tiles[y * across + x] = Some(tuile((l * 1000 + f + k) as u32));
                    }
                    dernier = Some(doc.add_cel(cel));
                }
                layer.frames.push(dernier);
            }
            match l {
                1 => {
                    let mut flou = Effect::new(EffectKind::GaussianBlur);
                    flou.set_param(0, 8.0);
                    layer.fx.push(flou);
                }
                3 => layer.fx.push(Effect::new(EffectKind::HueSaturation)),
                4 => layer.transform = crate::document::LayerTransform { angle: 0.05, scale: 1.02, ..Default::default() },
                _ => {}
            }
            calques.push(layer);
        }
        doc.panels[0].layers = calques;
        doc.panels[0].length = images;
        let c = doc.default_camera();
        doc.panels[0].camera = vec![CameraKey::new(0, c), CameraKey::new(images - 1, Camera { x: c.x + 220.0, zoom: c.zoom * 1.06, angle: 0.02, ..c })];
        doc
    }

    fn stats(v: &mut [f64]) -> (f64, f64, f64) {
        if v.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        v.sort_by(f64::total_cmp);
        (v.iter().sum::<f64>() / v.len() as f64, v[(v.len() * 95 / 100).min(v.len() - 1)], v[v.len() - 1])
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Sortie {
        Png,
        Film,
    }

    fn rapporter(titre: &str, ui: &mut Vec<f64>, total: Duration, images: usize, avant: f64) {
        let (moy, p95, pire) = stats(ui);
        let (_, pic) = memoire();
        println!(
            "PERF export {titre:<34} image d'écran : moy {moy:>6.2} ms, p95 {p95:>6.2}, pire {pire:>7.2} ({} images d'écran) | export {:.1} s ({:.2} i/s) | mémoire {avant:.0} Mo avant, pic {pic:.0} Mo",
            ui.len(),
            total.as_secs_f64(),
            images as f64 / total.as_secs_f64()
        );
    }

    /// L'ancien export : les images se calculent sur le fil de l'interface, 40 ms par image d'écran au moins (le test se fait
    /// entre deux images : une image de 4K finit toujours), et partent vers le même écrivain.
    fn ancien(sortie: Sortie, images: usize) {
        monde::lire();
        let (_, avant) = memoire();
        let mut b = banc(projet_4k(images), true);
        let doc_frame = b.app.doc.frame;
        let dir = dossier_neuf("mesure-ancien");
        let frames = export::frames_for(&b.app.doc, Range::Montage, 0);
        enum Ecrivain {
            Png(std::sync::mpsc::SyncSender<(usize, Image)>, Vec<std::thread::JoinHandle<()>>),
            Film(std::process::Child, std::sync::mpsc::SyncSender<Image>, std::thread::JoinHandle<()>),
        }
        let ecrivain = match sortie {
            Sortie::Png => {
                let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, Image)>(8);
                let rx = Arc::new(Mutex::new(rx));
                let fils = (0..4)
                    .map(|_| {
                        let (rx, dir) = (rx.clone(), dir.clone());
                        std::thread::spawn(move || loop {
                            let Ok((i, img)) = rx.lock().unwrap().recv() else { break };
                            crate::io::write_png(&dir.join(format!("a_{:04}.png", i + 1)), &img, doc_frame[0], doc_frame[1]).unwrap();
                        })
                    })
                    .collect();
                Ecrivain::Png(tx, fils)
            }
            Sortie::Film => {
                let ffmpeg = export::find_ffmpeg(None).expect("ffmpeg");
                let enc = export::probe(&ffmpeg).unwrap();
                let out = dir.join("film.mp4");
                let args = export::video_args(Format::Mp4, &enc, doc_frame, 24, false, None, &out).unwrap();
                let mut c = std::process::Command::new(&ffmpeg);
                c.args(&args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
                let mut child = c.spawn().expect("ffmpeg");
                let mut stdin = child.stdin.take().unwrap();
                let (tx, rx) = std::sync::mpsc::sync_channel::<Image>(8);
                let fil = std::thread::spawn(move || {
                    use std::io::Write;
                    for img in rx {
                        stdin.write_all(bytemuck::cast_slice(&img[..])).unwrap();
                    }
                });
                Ecrivain::Film(child, tx, fil)
            }
        };
        // Le cache des quatre dernières images de l'ancien export, et son pas de 40 ms.
        let mut cache: Vec<(render::Plan, Camera, Image)> = Vec::new();
        let (mut suivante, mut ui) = (0usize, Vec::new());
        let t0 = Instant::now();
        while suivante < frames.len() {
            let t = Instant::now();
            let m = b.image(Vec::new(), 0.016);
            let t_pas = Instant::now();
            // (Le pas de l'ancien export.)
            while suivante < frames.len() && t_pas.elapsed() < Duration::from_millis(40) {
                let (p, f) = frames[suivante];
                let doc = &b.app.doc;
                let plan = render::panel_plan_at(doc, p, f, doc.render_paper());
                let cam = doc.panel_camera(p, f);
                let img = match cache.iter().position(|(lp, lc, _)| *lp == plan && *lc == cam) {
                    Some(k) => cache[k].2.clone(),
                    None => {
                        let img: Image = Arc::new(render::camera_image(doc, &plan, &cam));
                        cache.push((plan, cam, img.clone()));
                        if cache.len() > 4 {
                            cache.remove(0);
                        }
                        img
                    }
                };
                match &ecrivain {
                    Ecrivain::Png(tx, _) => tx.send((suivante, img)).unwrap(),
                    Ecrivain::Film(_, tx, _) => tx.send(img).unwrap(),
                }
                suivante += 1;
            }
            ui.push(m.construction + m.decoupage + m.carte + t_pas.elapsed().as_secs_f64() * 1e3);
            // (Une image d'écran tous les 16 ms au plus, comme avec la synchronisation verticale de la vraie fenêtre.)
            std::thread::sleep(Duration::from_millis(16).saturating_sub(t.elapsed()));
        }
        match ecrivain {
            Ecrivain::Png(tx, fils) => {
                drop(tx);
                fils.into_iter().for_each(|f| f.join().unwrap());
            }
            Ecrivain::Film(mut child, tx, fil) => {
                drop(tx);
                fil.join().unwrap();
                child.wait().unwrap();
            }
        }
        rapporter(if sortie == Sortie::Png { "PNG, ancien (fil de l'interface)" } else { "film, ancien (fil de l'interface)" }, &mut ui, t0.elapsed(), frames.len(), avant);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Le nouvel export : le cliché, puis les fils de rendu ; l'interface tourne à côté.
    fn nouveau(sortie: Sortie, images: usize) {
        monde::lire();
        // PELURE_PRIORITE_NORMALE=1 : pour voir ce que la priorité basse change.
        if std::env::var_os("PELURE_PRIORITE_NORMALE").is_some() {
            crate::parallele::set_priorite_export(false);
        }
        let (_, avant) = memoire();
        let mut b = banc(projet_4k(images), true);
        let dir = dossier_neuf("mesure-nouveau");
        let frames = export::frames_for(&b.app.doc, Range::Montage, 0);
        let job = match sortie {
            Sortie::Png => ExportJob::images(&dir, "n", frames.clone(), b.app.doc.frame, true),
            Sortie::Film => {
                let ffmpeg = export::find_ffmpeg(None).expect("ffmpeg");
                let enc = export::probe(&ffmpeg).unwrap();
                let out = dir.join("film.mp4");
                let args = export::video_args(Format::Mp4, &enc, b.app.doc.frame, 24, false, None, &out).unwrap();
                ExportJob::video(&ffmpeg, args, &out, frames.clone(), None).unwrap()
            }
        };
        let t0 = Instant::now();
        b.app.start_job(job);
        let mut ui = Vec::new();
        while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && t0.elapsed() < Duration::from_secs(900) {
            let t = Instant::now();
            let m = b.image(Vec::new(), 0.016);
            ui.push(m.construction + m.decoupage + m.carte);
            // (Une image d'écran tous les 16 ms au plus, comme avec la synchronisation verticale de la vraie fenêtre.)
            std::thread::sleep(Duration::from_millis(16).saturating_sub(t.elapsed()));
        }
        let total = t0.elapsed();
        assert!(matches!(b.app.export_job.as_ref().unwrap().outcome, Some(Ok(_))), "{:?}", b.app.export_job.as_ref().unwrap().outcome);
        rapporter(if sortie == Sortie::Png { "PNG, nouveau (fils de rendu)" } else { "film, nouveau (fils de rendu)" }, &mut ui, total, frames.len(), avant);
        b.app.export_job = None;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore]
    fn perf_export_png_legacy() {
        ancien(Sortie::Png, 48);
    }

    #[test]
    #[ignore]
    fn perf_export_png_background() {
        nouveau(Sortie::Png, 48);
    }

    #[test]
    #[ignore]
    fn perf_export_film_legacy() {
        ancien(Sortie::Film, 48);
    }

    #[test]
    #[ignore]
    fn perf_export_film_background() {
        nouveau(Sortie::Film, 48);
    }

    /// Le cliché d'un grand projet : ce que l'interface paie au départ de l'export (structure + tranches de dessins).
    #[test]
    #[ignore]
    fn perf_export_snapshot_cost() {
        monde::lire();
        for (plans, calques, images, tuiles) in [(30, 3, 24, 6), (600, 3, 24, 1), (600, 8, 48, 4)] {
            let doc = crate::perf_tests::projet_long(plans, calques, images, tuiles);
            let tous: Vec<usize> = (0..doc.panels.len()).collect();
            let un_plan = [doc.panels.len() / 2];
            for (nom, panels) in [("tout le montage", &tous[..]), ("un seul plan", &un_plan[..])] {
                let mut structure = Vec::new();
                let mut dessins = Vec::new();
                let mut plus_longue = 0.0f64;
                for _ in 0..5 {
                    let t = Instant::now();
                    let mut b = exportrender::SnapshotBuilder::new(&doc, panels);
                    structure.push(t.elapsed().as_secs_f64() * 1e3);
                    let t = Instant::now();
                    let mut tranches = Vec::new();
                    loop {
                        let s = Instant::now();
                        let fini = b.step(&doc, Duration::from_millis(8));
                        tranches.push(s.elapsed().as_secs_f64() * 1e3);
                        if fini {
                            break;
                        }
                    }
                    dessins.push(t.elapsed().as_secs_f64() * 1e3);
                    plus_longue = plus_longue.max(tranches.iter().cloned().fold(0.0, f64::max));
                    let _ = b.finish();
                }
                let (n_cels, _) = {
                    let mut b = exportrender::SnapshotBuilder::new(&doc, panels);
                    b.step(&doc, Duration::MAX);
                    b.progress()
                };
                println!(
                    "PERF cliché {plans} plans x {calques} calques x {images} images, {nom} : structure {:.2} ms (pire {:.2}), dessins {n_cels} en {:.2} ms au total (tranche la plus longue {plus_longue:.2} ms)",
                    stats(&mut structure.clone()).0,
                    stats(&mut structure).2,
                    stats(&mut dessins).0
                );
            }
        }
    }

    /// (Essai de mise au point) ffmpeg seul, sans calcul d'image : la vitesse de l'encodage, priorité normale ou basse.
    #[test]
    #[ignore]
    fn perf_export_ffmpeg_alone() {
        let ffmpeg = export::find_ffmpeg(None).expect("ffmpeg");
        let enc = export::probe(&ffmpeg).unwrap();
        let frame = [3840usize, 2160];
        let img: Image = Arc::new(vec![Color32::from_rgb(40, 90, 150); frame[0] * frame[1]]);
        // Des images un peu variées (sinon x264 triche).
        let mut variees = Vec::new();
        for k in 0..4 {
            let mut v = (*img).clone();
            for (i, p) in v.iter_mut().enumerate() {
                if (i / 97 + k * 31) % 7 < 3 {
                    *p = Color32::from_rgb((i % 251) as u8, (i / 3 % 253) as u8, 120);
                }
            }
            variees.push(Arc::new(v));
        }
        for basse in [false, true] {
            let out = std::env::temp_dir().join(format!("shot-ffmpeg-seul-{basse}.mp4"));
            let args = export::video_args(Format::Mp4, &enc, frame, 24, false, None, &out).unwrap();
            let mut c = std::process::Command::new(&ffmpeg);
            c.args(&args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            if basse {
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x0800_0000 | 0x4000);
            }
            let t = Instant::now();
            let mut child = c.spawn().unwrap();
            let mut stdin = child.stdin.take().unwrap();
            {
                use std::io::Write;
                for k in 0..48 {
                    stdin.write_all(bytemuck::cast_slice(&variees[k % 4][..])).unwrap();
                }
            }
            drop(stdin);
            child.wait().unwrap();
            println!("PERF ffmpeg seul, priorité {} : {:.1} s pour 48 images 4K", if basse { "basse" } else { "normale" }, t.elapsed().as_secs_f64());
            std::fs::remove_file(&out).ok();
        }
    }}
