//! Mesure de la vitesse d'un projet Storyboard Pro importé (import 73, `docs/IMPORT-SBP.md`).
//!
//! À la main, sur une COPIE : `SBP_ESSAI=<copie> cargo test --release --offline --target-dir target\msvc
//! --lib sbperf_tests::projet_importe -- --ignored --nocapture --test-threads=1`.
//! `SBP_PLANS=a,b,c` : les numéros de plan (de 0) à mesurer (par défaut : les trois plans qui ont le plus de calques).

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{pos2, vec2, Rect};

use crate::app::Mode;
use crate::document::Document;
use crate::perf_tests::{banc, bouge, jusqu_au_repos, scenario, trait_souris};
use crate::vector::Element;

#[repr(C)]
#[derive(Default)]
struct MemoireProcessus {
    cb: u32,
    page_faults: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged: usize,
    quota_paged: usize,
    quota_peak_non_paged: usize,
    quota_non_paged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private_usage: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> isize;
}
#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: isize, counters: *mut MemoireProcessus, cb: u32) -> i32;
}

/// Mémoire vive du processus d'essai, en Mo : (ensemble de travail, mémoire privée).
fn memoire_vive() -> (f64, f64) {
    let mut m = MemoireProcessus::default();
    m.cb = std::mem::size_of::<MemoireProcessus>() as u32;
    unsafe {
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) == 0 {
            return (0.0, 0.0);
        }
    }
    (m.working_set as f64 / 1e6, m.private_usage as f64 / 1e6)
}

/// Ce que pèse le projet : dessins, tuiles de pixels, zones vectorielles, points.
fn poids(doc: &Document) {
    let (mut tuiles, mut hors, mut zones, mut points, mut vides, mut avec_vecteur) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for c in doc.cels.values() {
        tuiles += c.raster.tiles.iter().flatten().count();
        hors += c.raster.outer.len();
        if c.is_empty() {
            vides += 1;
        }
        if let Some(v) = &c.vector {
            avec_vecteur += 1;
            for e in &v.elements {
                if let Element::Fill { contours, .. } = e {
                    zones += 1;
                    points += contours.iter().map(Vec::len).sum::<usize>();
                }
            }
        }
    }
    {
        let (mut unies, mut opaques_unies, mut cmax) = (0usize, 0usize, 0usize);
        let mut gros: Vec<usize> = Vec::new();
        for c in doc.cels.values() {
            let n = c.raster.tiles.iter().flatten().count() + c.raster.outer.len();
            gros.push(n);
            cmax = cmax.max(n);
            for t in c.raster.tiles.iter().flatten().chain(c.raster.outer.values()) {
                if t.iter().all(|p| *p == t[0]) {
                    unies += 1;
                    if t[0].a() == 255 {
                        opaques_unies += 1;
                    }
                }
            }
        }
        gros.sort_unstable_by(|a, b| b.cmp(a));
        let top: usize = gros.iter().take(gros.len() / 10).sum();
        println!("TUILES uniformes {unies} (dont opaques {opaques_unies}) sur {} ; le plus gros dessin {cmax} tuiles ; les 10 % plus gros portent {top}", tuiles + hors);
    }
    let calques: usize = doc.panels.iter().map(|p| p.layers.len()).sum();
    let fx: usize = doc.panels.iter().flat_map(|p| p.layers.iter()).map(|l| l.fx.len()).sum();
    let multiplan = doc.panels.iter().flat_map(|p| p.layers.iter()).filter(|l| l.depth != 0.0).count();
    let cachees = doc.panels.iter().flat_map(|p| p.layers.iter()).filter(|l| !l.visible).count();
    println!(
        "POIDS {} plans, {calques} calques ({cachees} cachés, {multiplan} en profondeur, {fx} effets), {} dessins ({vides} vides, {avec_vecteur} vectoriels) ; tuiles de la feuille {tuiles} ({:.0} Mo), hors-feuille {hors} ({:.0} Mo) ; {zones} zones vectorielles, {points} points ; feuille {}x{}, cadre {:?}",
        doc.panels.len(),
        doc.cels.len(),
        tuiles as f64 * 16384.0 / 1e6,
        hors as f64 * 16384.0 / 1e6,
        doc.width,
        doc.height,
        doc.frame
    );
}

#[test]
#[ignore]
fn projet_importe() {
    let Some(dir) = std::env::var_os("SBP_ESSAI").map(PathBuf::from) else { return };
    let (ram0, _) = memoire_vive();
    let sboard = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "sboard")).unwrap();
    let mut sb = crate::sboard::read(&sboard).unwrap();
    // Le format d'Anthony est carré (1920 x 1920, avec des cadres de contrôle 16:9 et 9:16) : SBP_TAILLE=1920,1920.
    if let Some(t) = std::env::var("SBP_TAILLE").ok().and_then(|s| s.split_once(',').and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?]))) {
        sb.size = t;
    }
    // SBP_SHOTS=2,7,34 : seulement ces plans de Storyboard Pro (un import plus court).
    if let Ok(s) = std::env::var("SBP_SHOTS") {
        let keep: Vec<usize> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let mut i = 0;
        sb.shots.retain(|_| {
            i += 1;
            keep.contains(&(i - 1))
        });
    }
    println!("SBP taille du projet lue : {:?}", sb.size);
    let t0 = Instant::now();
    let (doc, _warnings) = crate::sbnative::build(&sb, &dir, &|_| {});
    println!("IMPORT {:.1} s", t0.elapsed().as_secs_f32());
    poids(&doc);
    let (ram1, priv1) = memoire_vive();
    println!("MEMOIRE vive après l'import : {ram1:.0} Mo (ensemble de travail), {priv1:.0} Mo privés (avant : {ram0:.0})");
    if std::env::var_os("SBP_POIDS_SEUL").is_some() {
        return;
    }
    let noms: Vec<String> = sb.shots.iter().map(|s| s.name.clone()).collect();
    mesurer(doc, &noms);
}

/// Les mesures d'affichage d'un projet ouvert (le même banc pour un projet Storyboard Pro importé et pour un projet SHOT).
fn mesurer(doc: Document, noms: &[String]) {
    // Les plans à mesurer : les plus chargés en calques, ou ceux de SBP_PLANS.
    let mut plans: Vec<usize> = match std::env::var("SBP_PLANS") {
        Ok(s) => s.split(',').filter_map(|v| v.trim().parse().ok()).collect(),
        Err(_) => {
            let mut idx: Vec<usize> = (0..doc.panels.len()).collect();
            idx.sort_by_key(|&i| std::cmp::Reverse(doc.panels[i].layers.len()));
            idx.truncate(3);
            idx
        }
    };
    plans.retain(|&i| i < doc.panels.len());
    let mut b = banc(doc, true);
    b.app.set_mode(Mode::Storyboard);
    b.chauffer(10);
    let t = Instant::now();
    let (images, ms) = jusqu_au_repos(&mut b, 900);
    println!("OUVERTURE {images} images, {ms:.0} ms avant le repos ({:.0} ms mur)", t.elapsed().as_secs_f64() * 1e3);
    let repos = b.image(Vec::new(), 0.5);
    println!("REPOS prochaine image dans {:?}", repos.reveil);
    for &i in &plans {
        let n = b.app.doc.panels[i].layers.len();
        println!("=== plan {i} « {} » ({n} calques)", noms.get(i).map_or("", |s| s.as_str()));
        b.app.select_panel(i);
        b.chauffer(6);
        jusqu_au_repos(&mut b, 300);
        let zone = Rect::from_min_size(pos2(700.0, 500.0), vec2(500.0, 200.0));
        scenario("à l'arrêt (souris qui bouge)", &mut b, 60, 1.0 / 60.0, |_, k| bouge(k, zone));
        let frames = b.app.doc.panels[i].frame_count().max(2);
        scenario("tête de lecture à chaque image", &mut b, 90, 1.0 / 60.0, |b, k| {
            b.app.cur_frame = (k * 3) % frames;
            bouge(k, zone)
        });
        b.app.cur_frame = 0;
        b.app.toggle_play_scope(false);
        let m = scenario("lecture du plan (25 images/s)", &mut b, 100.min(frames * 2).max(30), 1.0 / 25.0, |_, _| Vec::new());
        let pires = m.iter().filter(|x| x.total() > 40.0).count();
        println!("LECTURE {pires} images sur {} dépassent 40 ms", m.len());
        b.app.playing = false;
        b.app.cur_frame = frames / 2;
        b.app.cur_layer = n / 2;
        b.chauffer(4);
        let r = b.app.canvas_rect;
        let z = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
        scenario("dessin au pinceau (8 points/image)", &mut b, 40, 1.0 / 60.0, |_, k| trait_souris(k, 40, 8, z));
        b.app.undo();
        b.chauffer(3);
        if let Some(g) = &b.app.gpu {
            let s = g.stats();
            println!("CARTE {} textures, {:.0} Mo, {} tuiles envoyées, {} compositions", s.textures, s.bytes as f64 / 1e6, s.copies, s.composes);
        }
    }
    // La vue de la caméra (capture d'Anthony : « Caméra (animée) »), la lecture du montage d'un plan à l'autre, les sauts de plan en plan.
    let n_plans = b.app.doc.panels.len();
    b.app.set_camera_view(true);
    b.app.select_panel(plans.first().copied().unwrap_or(0));
    b.chauffer(6);
    jusqu_au_repos(&mut b, 300);
    let zone = Rect::from_min_size(pos2(700.0, 500.0), vec2(500.0, 200.0));
    scenario("vue caméra, à l'arrêt", &mut b, 60, 1.0 / 60.0, |_, k| bouge(k, zone));
    scenario("sauts de plan en plan (tous les 4 images)", &mut b, 120, 1.0 / 60.0, |b, k| {
        if k % 4 == 0 {
            b.app.select_panel((k / 4 * 5) % n_plans);
        }
        bouge(k, zone)
    });
    b.app.select_panel(0);
    b.app.cur_frame = 0;
    b.chauffer(4);
    b.app.toggle_play_scope(true);
    let m = scenario("lecture du montage (25 images/s)", &mut b, 300, 1.0 / 25.0, |_, _| Vec::new());
    let pires = m.iter().filter(|x| x.total() > 40.0).count();
    println!("LECTURE MONTAGE {pires} images sur {} dépassent 40 ms", m.len());
    b.app.playing = false;
    if let Some(g) = &b.app.gpu {
        let s = g.stats();
        println!("CARTE {} textures, {:.0} Mo", s.textures, s.bytes as f64 / 1e6);
    }
    // La copie de secours (écrite toutes les N minutes sur un autre fil) pendant que l'on regarde le plan.
    if std::env::var_os("SBP_SANS_SECOURS").is_none() {
        let manifeste = std::env::temp_dir().join(format!("sbperf-secours-{}", std::process::id())).join("secours.shot");
        let mut ecrit = std::collections::HashMap::new();
        let t = Instant::now();
        match crate::project::prepare_save(&b.app.doc, &manifeste, &mut ecrit, None) {
            Ok(job) => {
                println!("SECOURS préparation (fil de l'interface) {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                let fin = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let (f2, t2) = (fin.clone(), Instant::now());
                let h = std::thread::spawn(move || {
                    let r = job.run();
                    f2.store(true, std::sync::atomic::Ordering::SeqCst);
                    (r.is_ok(), t2.elapsed().as_secs_f64())
                });
                let mut mesures = Vec::new();
                while !fin.load(std::sync::atomic::Ordering::SeqCst) && mesures.len() < 3000 {
                    let k = mesures.len();
                    mesures.push(b.image(bouge(k, zone), 1.0 / 60.0));
                    std::thread::sleep(std::time::Duration::from_millis(8));
                }
                let (ok, dur) = h.join().unwrap();
                let tot: Vec<f64> = mesures.iter().map(|m| m.total()).collect();
                let moy = tot.iter().sum::<f64>() / tot.len().max(1) as f64;
                let pire = tot.iter().cloned().fold(0.0, f64::max);
                println!("SECOURS écriture {dur:.1} s (ok {ok}) ; pendant ce temps {} images : {moy:.2} ms en moyenne, pire {pire:.1} ms", tot.len());
            }
            Err(e) => println!("SECOURS impossible : {e}"),
        }
        let _ = std::fs::remove_dir_all(manifeste.parent().unwrap());
    }
    let (ram2, priv2) = memoire_vive();
    println!("MEMOIRE vive à la fin : {ram2:.0} Mo (ensemble de travail), {priv2:.0} Mo privés");
}

/// Les tuiles d'une seule couleur d'un dessin rendu se partagent (import 73) sans changer un pixel, et écrire dans l'une
/// ne touche pas les autres.
#[test]
fn les_tuiles_pleines_se_partagent_sans_changer_un_pixel() {
    use crate::vector::{Element, VectorDrawing};
    let carre = |x0: f32, y0: f32, x1: f32, y1: f32| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    let dessin = VectorDrawing { elements: vec![Element::Fill { contours: vec![carre(10.0, 10.0, 600.0, 400.0)], color: [200, 30, 30], opacity: 1.0, gradient: None }] };
    // (Rendu à la main, sans partage, pour comparer.)
    let mut brut = crate::raster::Raster::new(768, 512);
    for e in &dessin.elements {
        crate::vector::render_element(&mut brut, e);
    }
    let mut r = dessin.render(768, 512);
    assert!(r.tiles.iter().flatten().count() > 40);
    for (a, b) in brut.tiles.iter().zip(&r.tiles) {
        match (a, b) {
            (Some(a), Some(b)) => assert!(a.iter().zip(b.iter()).all(|(p, q)| p == q), "aucun pixel ne change"),
            (None, None) => {}
            _ => panic!("les mêmes tuiles existent"),
        }
    }
    let pleines: Vec<usize> = r.tiles.iter().enumerate().filter(|(_, t)| t.as_ref().is_some_and(|t| t.iter().all(|p| *p == t[0]))).map(|(i, _)| i).collect();
    assert!(pleines.len() >= 2);
    let (a, b) = (r.tiles[pleines[0]].clone().unwrap(), r.tiles[pleines[1]].clone().unwrap());
    assert!(a.same_as(&b), "deux tuiles pleines de même couleur : une seule mémoire");
    assert_eq!(r.share_solid_tiles(), 0, "déjà partagées");
    // Écrire dans l'une : l'autre garde sa couleur.
    r.tile_mut(pleines[0])[0] = eframe::egui::Color32::BLACK;
    assert_ne!(r.tiles[pleines[0]].as_ref().unwrap()[0], r.tiles[pleines[1]].as_ref().unwrap()[0]);
    assert_eq!(r.tiles[pleines[1]].as_ref().unwrap()[0], b[0]);
}
/// Même mesure sur un projet SHOT (copie) : `SHOT_PROJET=<dossier>\<nom>.pelure` (ou `.shot`), pour ne rien ralentir ailleurs.
#[test]
#[ignore]
fn projet_shot() {
    let Some(manifeste) = std::env::var_os("SHOT_PROJET").map(PathBuf::from) else { return };
    let t0 = Instant::now();
    let doc = crate::project::load(&manifeste).unwrap().doc;
    println!("OUVERTURE DU PROJET {:.1} s", t0.elapsed().as_secs_f32());
    poids(&doc);
    let (ram, privee) = memoire_vive();
    println!("MEMOIRE vive après l'ouverture : {ram:.0} Mo, {privee:.0} Mo privés");
    mesurer(doc, &[]);
}