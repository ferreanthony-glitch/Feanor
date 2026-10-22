//! Les vignettes sur des fils de fond (`thumbs.rs`, levier 4 de `docs/PERFORMANCE.md`) : mêmes pixels
//! que l'ancien chemin (composition sur le fil de l'interface), mise à jour après chaque sorte de
//! changement, demandes périmées abandonnées, et rien de composé sur le fil de l'interface.

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Event, Modifiers};

use super::*;
use crate::blend::BlendMode;
use crate::document::{Document, LayerTransform};
use crate::fiabilite_fixture::{dossier, projet_complet};
use crate::fx::{Effect, EffectKind};
use crate::fxcache::monde;
use crate::perf_tests::{banc, projet_long};
use crate::raster::premultiply;

const WIDTH: usize = STRIP_WIDTH;

fn far() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

/// La vignette telle que l'ancien chemin la faisait : composée tout de suite, sur le fil appelant, du
/// projet lui-même, en mode doux.
fn old_path(doc: &Document, panel: usize, frame: usize, width: usize) -> (ColorImage, bool) {
    let plan = render::panel_plan_at(doc, panel, frame, Some(doc.paper));
    let cam = doc.panel_camera(panel, frame);
    let (tw, th) = thumb_size(doc, width);
    crate::fx::differe(|| crate::fxcache::soft_scope(|| render::thumbnail(doc, &plan, &cam, tw, th)))
}

fn same_image(a: &ColorImage, b: &ColorImage) -> bool {
    a.size == b.size && a.pixels == b.pixels
}

/// Des images d'écran jusqu'à ce que toutes les vignettes de `wants` (plan, image, numéro de vignette)
/// soient là, à jour et complètes, et que les fils de fond n'aient plus rien à faire.
fn settle(thumbs: &mut Thumbs, ctx: &egui::Context, doc: &Document, wants: &[(usize, usize, u64)]) {
    let t = Instant::now();
    loop {
        crate::fx::begin_frame(ctx);
        thumbs.begin_frame(ctx);
        for &(p, f, id) in wants {
            thumbs.texture_at(ctx, doc, p, f, id, WIDTH, far());
        }
        if !thumbs.busy() && wants.iter().all(|&(_, _, id)| thumbs.is_complete(id, WIDTH)) {
            return;
        }
        assert!(t.elapsed() < Duration::from_secs(90), "les vignettes ne finissent pas : {} en file", thumbs.outstanding());
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Une vignette par plan, image 0.
fn firsts(doc: &Document) -> Vec<(usize, usize, u64)> {
    doc.panels.iter().enumerate().map(|(i, p)| (i, 0, p.id)).collect()
}

fn pause(thumbs: &Thumbs, on: bool) {
    let pool = thumbs.pool.as_ref().expect("les fils sont lancés");
    pool.shared.lock().paused = on;
    pool.shared.wake.notify_all();
}

// ------------------------------------------------------------------ mêmes pixels que l'ancien chemin

/// Le projet d'essai de la fiabilité (effets de calque à clés, tremblement, fond sous le trait, calque
/// de couleur qui griffonne, calque vidéo, dossier, mode de fusion, transformation, caméra à clés, calque d'effet
/// d'ajustement, incrustation) : chaque vignette faite par les fils de fond est celle de l'ancien chemin,
/// pixel pour pixel. Les incrustations (texte du timecode…) ne sont pas dans les vignettes, avant comme
/// après.
#[test]
fn background_thumbnails_are_the_old_ones_pixel_for_pixel() {
    monde::ecrire();
    let travail = dossier("vignettes-fil");
    let mut doc = projet_complet(&travail).doc;
    // Le calque de couleur qui griffonne est caché dans le projet d'essai : on le montre, pour qu'il soit dans les vignettes.
    for l in doc.panels[0].layers.iter_mut() {
        l.visible = true;
    }
    let ctx = egui::Context::default();
    let mut wants = Vec::new();
    let (mut fx, mut markers, mut fonds, mut ctgs, mut videos, mut moved, mut blended) = (0, 0, 0, 0, 0, 0, 0);
    for p in 0..doc.panels.len() {
        let n = doc.panels[p].frame_count();
        for frame in [0, n / 3, n / 2, n - 1] {
            let plan = render::panel_plan_at(&doc, p, frame, Some(doc.paper));
            for i in &plan.items {
                fx += usize::from(!i.fx.is_empty());
                markers += usize::from(matches!(i.fond, Some(render::FondPart::Reglage(_))));
                fonds += usize::from(matches!(i.fond, Some(render::FondPart::Forme(_))));
                ctgs += usize::from(i.ctg.is_some());
                videos += usize::from(i.cel & (1 << 62) != 0 && !doc.cels.contains_key(&i.cel));
                moved += usize::from(i.transform.is_some());
                blended += usize::from(i.blend != BlendMode::Normal);
            }
            // L'ancien chemin et le nouveau partent d'un état où les effets sont calculés (les deux
            // montrent alors le même résultat, complet).
            let cam = doc.panel_camera(p, frame);
            let _ = render::camera_image(&doc, &plan, &cam);
            wants.push((p, frame, ((p as u64) << 20) | frame as u64));
        }
    }
    for (what, n) in [("effets de calque", fx), ("calques d'effet", markers), ("aplats sous le trait", fonds), ("remplissages au griffonnage", ctgs), ("vidéo", videos), ("transformations", moved), ("modes de fusion", blended)] {
        assert!(n > 0, "le projet d'essai doit avoir des {what} dans ses vignettes");
    }
    let mut thumbs = Thumbs::default();
    settle(&mut thumbs, &ctx, &doc, &wants);
    let mut painted = 0;
    for &(p, frame, id) in &wants {
        let (old, complete) = old_path(&doc, p, frame, WIDTH);
        assert!(complete, "plan {p} image {frame} : l'ancien chemin est complet une fois les effets calculés");
        let new = thumbs.image_of(id, WIDTH).expect("la vignette est là");
        assert!(same_image(new, &old), "plan {p} image {frame} : la vignette des fils de fond diffère de l'ancienne");
        painted += old.pixels.iter().filter(|c| **c != Color32::TRANSPARENT).count();
    }
    assert!(painted > 5_000, "les vignettes montrent des dessins ({painted} pixels)");
    drop(thumbs);
    let _ = std::fs::remove_dir_all(&travail);
}

/// Le cliché ne garde que les dessins du plan, et la composition qu'il porte est celle du projet : une
/// vignette composée sur le cliché, sur ce fil, donne les mêmes pixels que sur le projet.
#[test]
fn a_snapshot_composes_like_the_project() {
    monde::lire();
    let doc = projet_long(2, 4, 6, 8);
    let plan = render::panel_plan_at(&doc, 0, 2, Some(doc.paper));
    let cam = doc.panel_camera(0, 2);
    let mini = snapshot(&doc, &plan);
    assert!(mini.cels.len() <= plan.items.len(), "seulement les dessins du plan ({} sur {})", mini.cels.len(), doc.cels.len());
    assert!(mini.cels.len() >= 3);
    let (tw, th) = thumb_size(&doc, WIDTH);
    let a = render::thumbnail(&doc, &plan, &cam, tw, th);
    let b = render::thumbnail(&mini, &plan, &cam, tw, th);
    assert!(same_image(&a, &b));
    // Les tuiles sont partagées, pas copiées.
    let (id, cel) = mini.cels.iter().next().expect("un dessin");
    let t = cel.raster.tiles.iter().position(Option::is_some).expect("une tuile peinte");
    assert!(cel.raster.tiles[t].as_ref().unwrap().same_as(doc.cels[id].raster.tiles[t].as_ref().unwrap()), "le cliché partage les tuiles");
}

/// Une composition interrompue s'arrête (une vignette que plus personne n'attend).
#[test]
fn a_cancelled_composition_stops_early() {
    monde::lire();
    let doc = projet_long(1, 3, 6, 8);
    let plan = render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    let cam = doc.panel_camera(0, 0);
    assert!(render::thumbnail_until(&doc, &plan, &cam, 96, 54, &|| true).is_none());
    let rows = std::cell::Cell::new(0);
    let r = render::thumbnail_until(&doc, &plan, &cam, 96, 54, &|| {
        rows.set(rows.get() + 1);
        rows.get() > 10
    });
    assert!(r.is_none() && rows.get() == 11, "arrêtée à la onzième ligne ({})", rows.get());
    assert!(render::thumbnail_until(&doc, &plan, &cam, 96, 54, &|| false).is_some());
}

// ------------------------------------------------------------------ mise à jour après chaque changement

/// Un bloc de couleur dans le cadre de chaque dessin, ailleurs d'un dessin à l'autre : les calques se
/// recouvrent en partie, un changement se voit.
fn decorate(doc: &mut Document) {
    let ids: Vec<u64> = doc.cels.keys().copied().collect();
    for id in ids {
        let (x0, y0) = (1000 + (id as usize * 97) % 1000, 560 + (id as usize * 61) % 400);
        let color = premultiply(40 + (id * 53 % 200) as u8, 40 + (id * 29 % 200) as u8, 40 + (id * 17 % 200) as u8, 220);
        let c = doc.cels.get_mut(&id).expect("dessin");
        for y in y0..y0 + 300 {
            for x in x0..x0 + 500 {
                let i = c.raster.tile_index_of(x, y);
                c.raster.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = color;
            }
        }
        c.version += 1;
    }
}

fn paint_block(doc: &mut Document, panel: usize) {
    let cel = doc.panels[panel].layers[0].frames[0].expect("un dessin à l'image 0");
    let c = doc.cels.get_mut(&cel).expect("dessin");
    for y in 700..900 {
        for x in 1400..1600 {
            let i = c.raster.tile_index_of(x, y);
            c.raster.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = premultiply(220, 30, 30, 255);
        }
    }
    c.version += 1;
}

/// Chaque sorte de changement (un trait, l'opacité, la visibilité, le mode de fusion, la transformation
/// ou un effet d'un calque, la caméra, la durée d'un plan voisin, le papier, le dessin d'une image,
/// un calque de moins) refait la vignette, et la vignette montrée est celle de l'ancien chemin. Jusque
/// là, l'ancienne reste à l'écran.
#[test]
fn every_kind_of_change_refreshes_the_thumbnail() {
    monde::ecrire();
    let mut doc = projet_long(3, 3, 12, 6);
    decorate(&mut doc);
    // Deux plans de la même scène : le second reprend la caméra où le premier la laisse.
    let second = doc.add_panel_after(0);
    doc.panels[second].layers = doc.panels[0].layers.clone();
    doc.panels[second].length = 12;
    let cam = |zoom: f32| crate::camera::Camera { x: 1920.0, y: 1080.0, zoom, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
    crate::camera::set_key(&mut doc.panels[0].camera, 0, cam(1.0));
    crate::camera::set_key(&mut doc.panels[0].camera, 11, cam(2.0));
    let ctx = egui::Context::default();
    let mut thumbs = Thumbs::default();
    let wants = firsts(&doc);
    settle(&mut thumbs, &ctx, &doc, &wants);

    type Change = Box<dyn Fn(&mut Document)>;
    let changes: Vec<(&str, Vec<usize>, Change)> = vec![
        // (Le plan 2 montre le même dessin que le plan 1.)
        ("un trait", vec![0, 1], Box::new(|d| paint_block(d, 0))),
        ("l'opacité d'un calque", vec![0], Box::new(|d| d.panels[0].layers[1].opacity = 0.3)),
        ("un calque caché", vec![0], Box::new(|d| d.panels[0].layers[0].visible = false)),
        ("le mode de fusion", vec![0], Box::new(|d| d.panels[0].layers[2].blend = BlendMode::Multiply)),
        ("la transformation d'un calque", vec![0], Box::new(|d| d.panels[0].layers[1].transform = LayerTransform { x: 300.0, y: -120.0, scale: 0.7, angle: 12.0, ..Default::default() })),
        (
            "un effet de calque",
            vec![0],
            Box::new(|d| {
                let mut e = Effect::new(EffectKind::GaussianBlur);
                e.set_param(0, 14.0);
                d.panels[0].layers[2].fx.push(e);
            }),
        ),
        // (Le plan 2 reprend la caméra du dernier instant du plan 1 : la clé de la fin ne change pas.)
        ("une clé de caméra", vec![0], Box::new(move |d| crate::camera::set_key(&mut d.panels[0].camera, 0, cam(1.6)))),
        (
            "la durée du plan d'avant (caméra du plan d'après)",
            vec![1],
            Box::new(|d| {
                for l in &mut d.panels[0].layers {
                    l.frames.truncate(4);
                }
                d.panels[0].length = 4;
            }),
        ),
        ("le papier", vec![0, 1, 2, 3], Box::new(|d| d.paper = [30, 190, 80])),
        (
            "le dessin d'une image",
            vec![2],
            Box::new(|d| {
                let other = d.panels[2].layers[0].frames[5];
                d.panels[2].layers[0].frames[0] = other;
            }),
        ),
        ("un calque de moins", vec![2], Box::new(|d| drop(d.panels[2].layers.remove(1)))),
    ];
    for (what, changed, change) in changes {
        let before: Vec<ColorImage> = wants.iter().map(|w| thumbs.image_of(w.2, WIDTH).expect("avant").clone()).collect();
        change(&mut doc);
        // Avant que les fils aient fini : l'ancienne vignette reste à l'écran.
        pause(&thumbs, true);
        crate::fx::begin_frame(&ctx);
        thumbs.begin_frame(&ctx);
        for &(p, f, id) in &wants {
            assert!(thumbs.texture_at(&ctx, &doc, p, f, id, WIDTH, far()).is_some(), "{what} : une vignette en attendant");
        }
        for (k, w) in wants.iter().enumerate() {
            assert!(same_image(thumbs.image_of(w.2, WIDTH).unwrap(), &before[k]), "{what} : l'ancienne vignette reste en attendant");
        }
        pause(&thumbs, false);
        settle(&mut thumbs, &ctx, &doc, &wants);
        for (k, &(p, f, id)) in wants.iter().enumerate() {
            let now = thumbs.image_of(id, WIDTH).unwrap();
            let (want, _) = old_path(&doc, p, f, WIDTH);
            assert!(same_image(now, &want), "{what} : la vignette du plan {p} n'est pas celle de l'ancien chemin");
            assert_eq!(!same_image(now, &before[k]), changed.contains(&p), "{what} : la vignette du plan {p} {}", if changed.contains(&p) { "devait changer" } else { "ne devait pas changer" });
        }
    }
}

/// Un trait posé alors que la vignette se compose : la vignette montrée est celle du dessin d'après, et
/// la tuile partagée avec le cliché n'a pas bougé sous la composition (copie à la première écriture).
#[test]
fn drawing_while_a_thumbnail_is_composed_leaves_the_snapshot_alone() {
    monde::lire();
    let mut doc = projet_long(1, 3, 6, 8);
    let plan = render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    let mini = snapshot(&doc, &plan);
    let cam = doc.panel_camera(0, 0);
    let (tw, th) = thumb_size(&doc, WIDTH);
    let before = render::thumbnail(&mini, &plan, &cam, tw, th);
    paint_block(&mut doc, 0);
    let after = render::thumbnail(&mini, &plan, &cam, tw, th);
    assert!(same_image(&before, &after), "le cliché ne change pas quand le projet est dessiné");
    let now = render::thumbnail(&doc, &plan, &cam, tw, th);
    assert!(!same_image(&before, &now), "le projet, lui, a changé");
}

// ------------------------------------------------------------------ file d'attente, priorité, abandon

fn fake_job(doc: &Document, slot: Slot, tint: u8) -> Job {
    let mut plan = render::panel_plan_at(doc, 0, 0, Some(doc.paper));
    plan.paper = Some([tint, 0, 0]);
    Job {
        slot,
        work: Work::Plan { doc: snapshot(doc, &plan), cam: doc.panel_camera(0, 0), size: (96, 54), marge: 0, masque: None, generation: 0 },
        key: Arc::new(Key::Plan(plan, vec![u64::from(tint)], [0; 7])),
        seq: 0,
        era: 0,
        wanted: 0,
        order: 0,
        cancel: Arc::new(AtomicBool::new(false)),
        world: None,
    }
}

fn shared() -> Arc<Shared> {
    Arc::new(Shared {
        queue: Mutex::new(Queue::default()),
        wake: Condvar::new(),
        stop: AtomicBool::new(false),
        ctx: egui::Context::default(),
        rendered: std::sync::atomic::AtomicUsize::new(0),
    })
}

/// Une demande plus récente pour la même vignette remplace l'ancienne : celle-ci ne se compose jamais.
#[test]
fn a_newer_request_replaces_the_one_waiting() {
    monde::lire();
    let doc = projet_long(1, 2, 4, 2);
    let s = shared();
    s.push(fake_job(&doc, (1, 192), 10));
    s.push(fake_job(&doc, (2, 192), 20));
    s.push(fake_job(&doc, (1, 192), 30));
    {
        let q = s.lock();
        assert_eq!(q.waiting.len(), 2, "une demande par vignette");
        let one = q.waiting.iter().find(|j| j.slot == (1, 192)).unwrap();
        assert!(matches!(&*one.key, Key::Plan(_, v, _) if *v == vec![30]), "la plus récente");
    }
    // Le même contenu redemandé : seulement renouvelé.
    let key = s.lock().waiting.iter().find(|j| j.slot == (1, 192)).unwrap().key.clone();
    assert!(s.touch((1, 192), &key));
    assert_eq!(s.lock().waiting.len(), 2);
    assert!(!s.touch((1, 192), &fake_job(&doc, (1, 192), 99).key), "un autre contenu n'est pas déjà demandé");
}

/// Un calcul en cours dont la vignette a encore changé est arrêté ; et s'il avait été arrêté puis que
/// l'écran revient à son contenu (annuler), la demande est refaite.
#[test]
fn a_running_job_is_cancelled_when_its_thumbnail_changes_again() {
    monde::lire();
    let doc = projet_long(1, 2, 4, 2);
    let s = shared();
    let first = fake_job(&doc, (1, 192), 10);
    let (key, cancel) = (first.key.clone(), first.cancel.clone());
    s.push(first);
    let job = s.lock().pick().expect("la demande");
    assert_eq!(s.lock().running.len(), 1);
    assert!(s.touch((1, 192), &key), "en calcul : déjà demandée");
    s.push(fake_job(&doc, (1, 192), 20));
    assert!(cancel.load(Ordering::Relaxed), "son calcul est arrêté");
    assert!(job.cancel.load(Ordering::Relaxed));
    assert!(!s.touch((1, 192), &key), "arrêté : il faut la redemander si l'écran revient à ce contenu");
    // Autre projet : tout est abandonné, jusqu'aux images finies.
    s.lock().done.push_back(Done { slot: (9, 192), key: key.clone(), seq: 1, image: ColorImage::new([1, 1], Color32::RED), waiting: None, marge: 0 });
    s.reset();
    let q = s.lock();
    assert!(q.waiting.is_empty() && q.done.is_empty());
    assert!(q.running.iter().all(|r| r.cancel.load(Ordering::Relaxed)));
}

/// Une vignette finie, pas encore envoyée à la carte graphique, est déjà la réponse à son contenu ; mais
/// plus si une demande plus récente pour la même vignette l'a suivie (l'écran revient au contenu d'avant,
/// annuler : il faut le redemander, sinon la plus récente s'afficherait).
#[test]
fn a_finished_thumbnail_answers_its_content_until_a_newer_request() {
    monde::lire();
    let doc = projet_long(1, 2, 4, 2);
    let s = shared();
    let first = fake_job(&doc, (1, 192), 10);
    let key = first.key.clone();
    s.push(first);
    let job = s.lock().pick().expect("la demande");
    {
        let mut q = s.lock();
        q.running.retain(|r| r.seq != job.seq);
        q.done.push_back(Done { slot: job.slot, key: job.key.clone(), seq: job.seq, image: ColorImage::new([1, 1], Color32::RED), waiting: None, marge: 0 });
    }
    assert!(s.touch((1, 192), &key), "finie : déjà la réponse");
    assert!(!s.touch((2, 192), &key), "une autre vignette n'est pas finie");
    s.push(fake_job(&doc, (1, 192), 20));
    assert!(!s.touch((1, 192), &key), "une demande plus récente la remplacerait : à redemander");
}

/// Les vignettes qu'on ne voit plus (demande non renouvelée depuis quelques images) ne sont jamais
/// composées ; les plus récemment demandées passent d'abord, dans l'ordre où l'écran les a demandées.
#[test]
fn only_the_visible_thumbnails_are_composed_and_the_latest_first() {
    monde::lire();
    let doc = projet_long(1, 2, 4, 2);
    let s = shared();
    s.push(fake_job(&doc, (1, 192), 1));
    s.lock().frame += 2;
    s.push(fake_job(&doc, (2, 192), 2));
    s.push(fake_job(&doc, (3, 192), 3));
    // (2 et 3 sont renouvelées à l'image suivante, dans cet ordre : 3 d'abord, 2 ensuite.)
    s.lock().frame += 2;
    let k3 = s.lock().waiting.iter().find(|j| j.slot == (3, 192)).unwrap().key.clone();
    let k2 = s.lock().waiting.iter().find(|j| j.slot == (2, 192)).unwrap().key.clone();
    assert!(s.touch((3, 192), &k3));
    assert!(s.touch((2, 192), &k2));
    let order: Vec<u64> = std::iter::from_fn(|| s.lock().pick()).map(|j| j.slot.0).collect();
    // Le (1) a plus de `STALE_FRAMES` images : abandonné, jamais pris.
    assert_eq!(order, vec![3, 2], "{order:?}");
    assert!(s.lock().waiting.is_empty());
}

/// Lance les fils des vignettes en pause : une demande reste dans la file, aucun fil ne la prend avant
/// qu'on le décide.
fn start_paused(thumbs: &mut Thumbs, ctx: &egui::Context) {
    let pool = Pool::start(ctx, 4);
    pool.shared.lock().paused = true;
    thumbs.pool = Some(pool);
}

/// Bout en bout : une demande que l'écran cesse de renouveler est abandonnée et n'est jamais composée.
#[test]
fn a_thumbnail_that_leaves_the_screen_is_never_composed() {
    monde::lire();
    let doc = projet_long(3, 2, 4, 2);
    let ctx = egui::Context::default();
    let mut thumbs = Thumbs::default();
    start_paused(&mut thumbs, &ctx);
    thumbs.begin_frame(&ctx);
    thumbs.texture_at(&ctx, &doc, 0, 0, doc.panels[0].id, WIDTH, far());
    // La demande part sans que personne ne la prenne ; l'écran ne la renouvelle plus.
    for _ in 0..STALE_FRAMES + 2 {
        thumbs.begin_frame(&ctx);
    }
    assert_eq!(thumbs.outstanding(), 0, "abandonnée");
    pause(&thumbs, false);
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(thumbs.rendered(), 0, "jamais composée");
    // Et si l'écran la redemande, elle se fait.
    settle(&mut thumbs, &ctx, &doc, &[(0, 0, doc.panels[0].id)]);
    assert_eq!(thumbs.rendered(), 1);
}

/// Plusieurs changements d'une vignette avant qu'un fil la prenne : une seule composition, celle du
/// dernier état.
#[test]
fn many_changes_before_a_thread_is_free_cost_one_composition() {
    monde::ecrire();
    let mut doc = projet_long(1, 3, 6, 6);
    let ctx = egui::Context::default();
    let mut thumbs = Thumbs::default();
    let wants = firsts(&doc);
    start_paused(&mut thumbs, &ctx);
    thumbs.begin_frame(&ctx);
    for k in 0..8 {
        doc.panels[0].layers[1].opacity = 0.9 - k as f32 * 0.1;
        thumbs.begin_frame(&ctx);
        thumbs.texture_at(&ctx, &doc, 0, 0, wants[0].2, WIDTH, far());
    }
    pause(&thumbs, false);
    settle(&mut thumbs, &ctx, &doc, &wants);
    assert_eq!(thumbs.rendered(), 1, "huit états, une composition");
    let (want, _) = old_path(&doc, 0, 0, WIDTH);
    assert!(same_image(thumbs.image_of(wants[0].2, WIDTH).unwrap(), &want));
}

/// Attend qu'un fil ait fini une vignette (elle attend d'être envoyée à la carte graphique), sans
/// commencer d'image d'écran.
fn wait_done(thumbs: &Thumbs) {
    let shared = &thumbs.pool.as_ref().expect("les fils sont lancés").shared;
    let t = Instant::now();
    while !shared.has_done() {
        assert!(t.elapsed() < Duration::from_secs(60), "la vignette ne finit pas : {} en file", shared.outstanding());
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Une vignette finie entre le début d'une image d'écran et la demande de l'écran (elle n'est pas encore
/// envoyée à la carte graphique) est la réponse : l'écran ne la redemande pas, elle n'est composée qu'une
/// fois. (Avant, elle était redemandée et composée deux fois : les essais qui comptent les compositions
/// échouaient de temps en temps sur une machine chargée, quand le fil finissait dans cet intervalle.)
#[test]
fn a_thumbnail_finished_during_the_frame_is_not_asked_again() {
    monde::lire();
    let doc = projet_long(1, 2, 4, 2);
    let ctx = egui::Context::default();
    let mut thumbs = Thumbs::default();
    let wants = firsts(&doc);
    start_paused(&mut thumbs, &ctx);
    thumbs.begin_frame(&ctx);
    thumbs.texture_at(&ctx, &doc, 0, 0, wants[0].2, WIDTH, far());
    pause(&thumbs, false);
    wait_done(&thumbs);
    // Même image d'écran : la vignette finie n'est pas encore là, l'écran la demande encore.
    assert!(thumbs.image_of(wants[0].2, WIDTH).is_none());
    thumbs.texture_at(&ctx, &doc, 0, 0, wants[0].2, WIDTH, far());
    {
        let q = thumbs.pool.as_ref().unwrap().shared.lock();
        assert!(q.waiting.is_empty() && q.running.is_empty(), "pas redemandée ({} en file, {} en calcul)", q.waiting.len(), q.running.len());
    }
    settle(&mut thumbs, &ctx, &doc, &wants);
    assert_eq!(thumbs.rendered(), 1, "une seule composition");
    let (want, _) = old_path(&doc, 0, 0, WIDTH);
    assert!(same_image(thumbs.image_of(wants[0].2, WIDTH).unwrap(), &want));
}

/// Un fond sous le trait adopté ailleurs (un autre dessin, un autre onglet, un autre essai) ne refait
/// que les vignettes des plans qui montrent un fond (elles montraient le fond d'avant) ; celle d'un plan
/// sans fond reste. (Avant, il refaisait toutes les vignettes, y compris celles que comptent les essais
/// d'au-dessus quand un autre essai adoptait un fond en même temps.)
#[test]
fn a_background_adopted_elsewhere_only_refreshes_the_thumbnails_that_show_one() {
    monde::ecrire();
    let mut doc = projet_long(2, 2, 4, 2);
    doc.panels[1].layers[0].fond = Some(crate::fond::FondReglages { couleur: [201, 187, 142], ..Default::default() });
    let ctx = egui::Context::default();
    let mut thumbs = Thumbs::default();
    let wants = firsts(&doc);
    settle(&mut thumbs, &ctx, &doc, &wants);
    assert_eq!(thumbs.rendered(), 2);
    crate::fx::fond_adopte_ailleurs();
    settle(&mut thumbs, &ctx, &doc, &wants);
    assert_eq!(thumbs.rendered(), 3, "seule la vignette du plan qui a un fond se refait");
    for &(p, f, id) in &wants {
        let (want, _) = old_path(&doc, p, f, WIDTH);
        assert!(same_image(thumbs.image_of(id, WIDTH).unwrap(), &want), "plan {p}");
    }
}

// ------------------------------------------------------------------ rien sur le fil de l'interface

/// Défilement du montage, sauts de la tête de lecture, trait de pinceau, avec le montage sous les yeux :
/// le fil de l'interface ne compose aucune vignette (`render::THUMBNAILS_HERE`), les fils de fond en
/// composent, et toutes les vignettes visibles finissent par être là.
#[test]
fn no_thumbnail_is_composed_on_the_interface_thread() {
    monde::ecrire();
    let mut b = banc(projet_long(60, 3, 24, 4), false);
    b.app.thumbs.clear();
    let here = || render::THUMBNAILS_HERE.with(std::cell::Cell::get);
    let before = here();
    b.app.montage_zoom = 3.0;
    b.app.follow_frame = false;
    for k in 0..30 {
        let mut ev = b.souris(900.0, 940.0);
        ev.extend(b.molette(vec2(-120.0 - (k % 5) as f32 * 20.0, 0.0), Modifiers::NONE));
        b.image(ev, 1.0 / 60.0);
    }
    let total = b.app.doc.total_frames();
    b.app.follow_frame = true;
    for k in 0..30 {
        b.app.goto_global((k * 977) % total);
        b.image(Vec::new(), 1.0 / 60.0);
    }
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    for k in 0..20 {
        let p = |i: usize| pos2(zone.left() + zone.width() * (0.15 + 0.7 * i as f32 / 160.0), zone.center().y);
        let mut ev = Vec::new();
        if k == 0 {
            ev.push(Event::PointerMoved(p(0)));
            ev.push(Event::PointerButton { pos: p(0), button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        }
        for j in 0..8 {
            ev.push(Event::PointerMoved(p(k * 8 + j + 1)));
        }
        if k == 19 {
            ev.push(Event::PointerButton { pos: p(160), button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        }
        b.image(ev, 1.0 / 60.0);
    }
    b.attendre_vignettes();
    assert_eq!(here(), before, "aucune vignette n'a été composée sur le fil de l'interface");
    assert!(b.app.thumbs.rendered() > 10, "les fils de fond ont composé les vignettes ({})", b.app.thumbs.rendered());
    let cur = b.app.doc.panels[b.app.cur_panel].id;
    assert!(b.app.thumbs.image_of(cur, WIDTH).is_some(), "la vignette du plan courant est là");
    // (Et rien ne reste à faire : au repos.)
    assert!(!b.app.thumbs.busy());
}

// ------------------------------------------------------------------ miniatures de dessins (timeline, Calques)

/// L'ancien calcul de la miniature d'un dessin, sur le fil de l'interface (`App::layer_thumb` d'avant),
/// recopié.
fn old_cel_thumbnail(raster: &crate::raster::Raster, fr: egui::Rect, w: usize, h: usize) -> ColorImage {
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let sx = fr.min.x + (x as f32 + dx) / w as f32 * fr.width();
                let sy = fr.min.y + (y as f32 + dy) / h as f32 * fr.height();
                let (sx, sy) = (sx as usize, sy as usize);
                let p = if sx < raster.width && sy < raster.height { raster.get(sx, sy) } else { Color32::TRANSPARENT };
                let q = crate::render::over(Color32::WHITE, p, 255, None);
                r += q.r() as u32;
                g += q.g() as u32;
                b += q.b() as u32;
            }
            pixels.push(Color32::from_rgb((r / 4) as u8, (g / 4) as u8, (b / 4) as u8));
        }
    }
    ColorImage { size: [w, h], pixels }
}

/// Des images d'écran jusqu'à ce que les miniatures de `cels` soient là et que les fils n'aient plus rien à faire.
fn settle_cels(lt: &mut crate::layersui::LayerThumbs, ctx: &egui::Context, doc: &Document, cels: &[u64], size: (usize, usize)) {
    let fr = doc.frame_rect();
    let t = Instant::now();
    let mut pass = 0;
    loop {
        pass += 1;
        lt.begin_frame(ctx);
        for &c in cels {
            lt.texture(ctx, c, &doc.cels[&c], fr, size, pass);
        }
        if !lt.busy() && cels.iter().all(|&c| lt.image_of(c).is_some()) {
            return;
        }
        assert!(t.elapsed() < Duration::from_secs(60), "les miniatures ne finissent pas");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Les miniatures des fils de fond sont celles de l'ancien calcul ; un dessin modifié refait la sienne
/// (l'ancienne reste en attendant).
#[test]
fn drawing_thumbnails_on_background_threads_are_the_old_ones_and_follow_changes() {
    monde::lire();
    let mut doc = projet_long(2, 3, 6, 10);
    decorate(&mut doc);
    let ctx = egui::Context::default();
    let mut lt = crate::layersui::LayerThumbs::default();
    let cels: Vec<u64> = doc.cels.keys().copied().collect();
    let size = (64, 36);
    settle_cels(&mut lt, &ctx, &doc, &cels, size);
    let fr = doc.frame_rect();
    let mut painted = 0;
    for &c in &cels {
        let old = old_cel_thumbnail(&doc.cels[&c].raster, fr, size.0, size.1);
        assert!(same_image(lt.image_of(c).unwrap(), &old), "la miniature du dessin {c} diffère de l'ancienne");
        painted += old.pixels.iter().filter(|p| **p != Color32::WHITE).count();
    }
    assert!(painted > 100, "les miniatures montrent des dessins ({painted})");
    // Un trait sur un dessin : sa version change.
    let target = doc.panels[0].layers[0].frames[0].expect("un dessin");
    let before = lt.image_of(target).unwrap().clone();
    paint_block(&mut doc, 0);
    let version = doc.cels[&target].version;
    assert!(lt.texture(&ctx, target, &doc.cels[&target], fr, size, 99).is_some(), "l'ancienne miniature reste en attendant");
    assert!(same_image(lt.image_of(target).unwrap(), &before));
    settle_cels(&mut lt, &ctx, &doc, &cels, size);
    let now = lt.image_of(target).unwrap();
    assert!(same_image(now, &old_cel_thumbnail(&doc.cels[&target].raster, fr, size.0, size.1)), "version {version}");
    assert!(!same_image(now, &before), "la miniature a changé");
    // Les autres n'ont pas bougé.
    for &c in cels.iter().filter(|&&c| c != target) {
        assert!(same_image(lt.image_of(c).unwrap(), &old_cel_thumbnail(&doc.cels[&c].raster, fr, size.0, size.1)));
    }
}

/// Timeline zoomée (une miniature par bloc), sous les yeux : le fil de l'interface n'en compose aucune.
#[test]
fn no_drawing_thumbnail_is_composed_on_the_interface_thread() {
    monde::ecrire();
    let mut b = banc(projet_long(1, 6, 80, 3), false);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 48.0;
    let here = || CEL_THUMBNAILS_HERE.with(std::cell::Cell::get);
    let before = here();
    for k in 0..40 {
        b.app.cur_frame = (k * 7) % 80;
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.attendre_vignettes();
    assert!(b.app.layer_thumbs.len() > 5, "la timeline zoomée montre des miniatures ({})", b.app.layer_thumbs.len());
    assert_eq!(here(), before, "aucune miniature n'a été composée sur le fil de l'interface");
}

/// Ce que coûte une vignette au fil de l'interface (`cargo test --release --offline --target-dir target\msvc
/// perf_thumbnail_request_cost -- --ignored --nocapture`) : la demande à froid (clé, cliché, mise en file ; les
/// fils sont en pause, rien ne leur fait concurrence), la même vignette à jour, et, pour comparer, ce
/// que coûtait sa composition sur ce fil.
#[test]
#[ignore]
fn perf_thumbnail_request_cost() {
    monde::lire();
    for layers in [3usize, 15] {
        let doc = projet_long(600, layers, 24, 6);
        let ctx = egui::Context::default();
        let mut thumbs = Thumbs::default();
        start_paused(&mut thumbs, &ctx);
        thumbs.begin_frame(&ctx);
        let n = doc.panels.len();
        let t = Instant::now();
        for (i, p) in doc.panels.iter().enumerate() {
            thumbs.texture_at(&ctx, &doc, i, 0, p.id, WIDTH, far());
        }
        let cold = t.elapsed().as_secs_f64() * 1e6 / n as f64;
        let t = Instant::now();
        let plan = render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
        for _ in 0..n {
            std::hint::black_box(snapshot(&doc, &plan));
        }
        let snap = t.elapsed().as_secs_f64() * 1e6 / n as f64;
        pause(&thumbs, false);
        settle(&mut thumbs, &ctx, &doc, &doc.panels.iter().enumerate().map(|(i, p)| (i, 0, p.id)).collect::<Vec<_>>());
        let t = Instant::now();
        for (i, p) in doc.panels.iter().enumerate() {
            thumbs.texture_at(&ctx, &doc, i, 0, p.id, WIDTH, far());
        }
        let fresh = t.elapsed().as_secs_f64() * 1e6 / n as f64;
        let t = Instant::now();
        for i in 0..20 {
            std::hint::black_box(old_path(&doc, i, 0, WIDTH));
        }
        let old = t.elapsed().as_secs_f64() * 1e3 / 20.0;
        println!(
            "PERF vignette, {layers} calques : demande à froid {cold:.0} µs (dont cliché {snap:.0} µs), à jour {fresh:.0} µs sur le fil de l'interface ; l'ancien chemin la composait ici en {old:.2} ms"
        );
    }
}

/// Demander les vignettes de deux cents plans à froid coûte peu de temps au fil de l'interface
/// (l'ancien chemin en composait une par milliseconde et demie).
#[test]
fn asking_for_two_hundred_thumbnails_is_cheap_on_the_interface_thread() {
    monde::lire();
    let doc = projet_long(200, 3, 6, 6);
    let ctx = egui::Context::default();
    let mut thumbs = Thumbs::default();
    thumbs.begin_frame(&ctx);
    let start = Instant::now();
    for (i, p) in doc.panels.iter().enumerate() {
        thumbs.texture_at(&ctx, &doc, i, 0, p.id, WIDTH, far());
    }
    let asked = start.elapsed();
    assert!(asked < Duration::from_millis(250), "200 demandes en {asked:?} sur le fil de l'interface");
}
