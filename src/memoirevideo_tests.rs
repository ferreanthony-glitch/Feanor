//! Le cache des textures de dessin suit le budget de mémoire vidéo du système (`memoirevideo.rs`, audit du 2026-10-23,
//! point 5) : la règle en fonction pure, puis le cache d'une vraie carte qui reçoit des lectures posées par l'essai.

use eframe::egui::Color32;

use crate::affichage_tests::line_item;
use crate::document::{Cel, Document, LayerKind};
use crate::gpu::GpuCanvas;
use crate::gpu_tests::{headless, render_with};
use crate::memoirevideo::{cible_cache, effectif, poser_lecture_essai, vaut_d_etre_note, Lecture, PLANCHER};
use crate::render::{self, Plan};

const MO: u64 = 1 << 20;
const GO: u64 = 1 << 30;

#[test]
fn the_cache_target_keeps_shot_under_the_system_budget_with_a_margin() {
    // De la place : la cible dépasse le réglage, le réglage reste.
    let large = cible_cache(7 * GO, GO + 200 * MO, 800 * MO);
    assert!(large > 2 * GO, "{} Mo", large / MO);
    assert_eq!(effectif(GO, Some(large)), GO);
    // La situation de l'audit : 8 Go, Storyboard Pro en prend 3,5 ; le système n'accorde plus que 2,5 Go à SHOT, qui en
    // utilise 1,5 dont 1 Go de dessins. SHOT hors cache (0,5 Go) + cache ≤ 2,5 Go − 256 Mo.
    let serre = cible_cache(2560 * MO, 1536 * MO, GO);
    assert_eq!(serre, 2560 * MO - 512 * MO - 256 * MO);
    assert_eq!(effectif(2 * GO, Some(serre)), serre, "le cache réglé à 2 Go est ramené");
    // Au-delà du budget : le cache descend d'autant.
    let depasse = cible_cache(2 * GO, 2 * GO + 300 * MO, GO);
    assert_eq!(depasse, 2 * GO - (GO + 300 * MO) - 256 * MO);
    // La marge est d'un dixième du budget quand il est grand.
    assert_eq!(cible_cache(10 * GO, 2 * GO, GO), 10 * GO - GO - GO);
    // Plus rien pour le cache : le plancher (les dessins affichés restent de toute façon).
    assert_eq!(cible_cache(GO, 3 * GO, 100 * MO), PLANCHER);
    assert_eq!(cible_cache(0, 0, 0), PLANCHER);
    // Une lecture où le cache semble plus gros que tout SHOT (lecture d'avant un envoi) : rien d'autre ne compte.
    assert_eq!(cible_cache(4 * GO, 100 * MO, 500 * MO), 4 * GO - 4 * GO / 10);
    // Regrandir : les autres logiciels lâchent leur mémoire, le budget remonte, la cible aussi.
    assert!(cible_cache(6 * GO, 1536 * MO, GO) > serre);
    // Ce qui mérite d'être noté : un changement de plus d'un vingtième du poids visé.
    assert!(!vaut_d_etre_note(None, Some(3 * GO), GO), "de la place, le poids réglé : rien de neuf");
    assert!(vaut_d_etre_note(None, Some(500 * MO), GO));
    assert!(!vaut_d_etre_note(Some(500 * MO), Some(510 * MO), GO));
    assert!(vaut_d_etre_note(Some(500 * MO), Some(3 * GO), GO), "retour au poids réglé");
    // Pas de lecture : le poids réglé (comme avant).
    assert_eq!(effectif(GO, None), GO);
}

/// Un projet 2048 × 2048 de `n` dessins pleins (une texture de 16 Mo chacun, à la taille de son contenu).
fn gros_dessins(n: usize) -> (Document, Vec<crate::document::CelId>) {
    let mut doc = Document::new(2048, 2048, 24);
    let ids = (0..n)
        .map(|k| {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            for t in 0..cel.raster.tiles.len() {
                cel.raster.tile_mut(t).fill(Color32::from_rgba_premultiplied(10 + k as u8 * 20, 40, 90, 255));
            }
            doc.add_cel(cel)
        })
        .collect();
    (doc, ids)
}

/// Les deux images sont les mêmes, au lissage de la carte près (un niveau).
fn pareilles(a: &[Color32], b: &[Color32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_array().iter().zip(y.to_array()).all(|(p, q)| (*p as i32 - q as i32).abs() <= 1))
}

/// La lecture qui donne au cache, qui tient `own` octets, la cible `cible` (budget de 4 Go).
fn lecture_pour(seq: u64, own: u64, cible: u64) -> Lecture {
    let budget = 4 * GO;
    let hors_cache = budget - budget / 10 - cible;
    Lecture { seq, budget, utilise: hors_cache + own }
}

#[test]
fn eviction_follows_the_vram_target_and_never_drops_what_is_on_screen() {
    let Some(rs) = headless() else {
        eprintln!("aucune carte graphique : essai passé");
        return;
    };
    let (doc, cels) = gros_dessins(10);
    let mut gpu = GpuCanvas::new(&rs);
    poser_lecture_essai(None);
    // Sans lecture : les dix dessins vus tiennent dans le cache réglé (1 Go).
    for &c in &cels {
        render_with(&mut gpu, &rs, &doc, &Plan { paper: None, items: vec![line_item(c)] }, false);
    }
    let avant = gpu.stats();
    assert_eq!(avant.textures, 10);
    assert_eq!(avant.bytes, 10 * 16 * MO);
    assert_eq!(gpu.cache_limit(), 1 << 30);
    // La mémoire vidéo manque : la cible tombe à 64 Mo (quatre dessins). Deux dessins sont à l'écran. Lecture après
    // lecture (la mémoire chassée est bien rendue), le plafond descend d'un quart au plus à chaque fois.
    let deux = Plan { paper: None, items: vec![line_item(cels[3]), line_item(cels[7])] };
    let mut limites = vec![gpu.cache_limit()];
    let mut on_gpu = Vec::new();
    for seq in 1..=16 {
        poser_lecture_essai(Some(lecture_pour(seq, gpu.stats().bytes, 64 * MO)));
        on_gpu = render_with(&mut gpu, &rs, &doc, &deux, false);
        limites.push(gpu.cache_limit());
    }
    assert_eq!(limites[1], 768 * MO, "un quart de moins à la première lecture : {limites:?}");
    assert!(limites.windows(2).all(|w| w[1] <= w[0] && w[1] >= w[0] - w[0] / 4), "{limites:?}");
    let s = gpu.stats();
    assert!(gpu.cache_limit() <= 80 * MO && s.bytes <= 80 * MO, "{} Mo gardés pour une cible de 64 : {limites:?}", s.bytes / MO);
    assert!(s.textures >= 2);
    assert!(pareilles(&on_gpu, &render::composite_full(&doc, &deux)), "l'image est la même");
    let limite = gpu.cache_limit();
    // Les deux dessins affichés sont restés : les montrer de nouveau n'envoie rien.
    let copies = gpu.stats().copies;
    render_with(&mut gpu, &rs, &doc, &deux, false);
    assert_eq!(gpu.stats().copies, copies, "les dessins à l'écran n'ont pas été chassés");
    // Six dessins à l'écran, pour une cible de quatre : aucun des six ne part (la cible est dépassée le temps de l'image).
    let six = Plan { paper: None, items: cels[..6].iter().map(|&c| line_item(c)).collect() };
    let on_gpu = render_with(&mut gpu, &rs, &doc, &six, false);
    assert_eq!(gpu.stats().textures, 6, "les six dessins affichés restent");
    assert!(pareilles(&on_gpu, &render::composite_full(&doc, &six)));
    let copies = gpu.stats().copies;
    render_with(&mut gpu, &rs, &doc, &six, false);
    assert_eq!(gpu.stats().copies, copies);
    // La même lecture vue de nouveau ne refait pas la cible (elle n'est refaite qu'à chaque lecture).
    assert_eq!(gpu.cache_limit(), limite);
    // La place revient : à la lecture suivante, le cache retrouve son poids réglé, et regarde de nouveau grandir.
    poser_lecture_essai(Some(Lecture { seq: 100, budget: 7 * GO, utilise: GO }));
    for &c in &cels {
        render_with(&mut gpu, &rs, &doc, &Plan { paper: None, items: vec![line_item(c)] }, false);
    }
    assert_eq!(gpu.cache_limit(), 1 << 30);
    assert_eq!(gpu.stats().textures, 10, "le cache regrandit");
    // Une machine qui ne sait pas lire son budget : le comportement d'avant.
    poser_lecture_essai(None);
    render_with(&mut gpu, &rs, &doc, &six, false);
    assert_eq!(gpu.cache_limit(), 1 << 30);
}

#[test]
fn a_decision_is_written_for_the_diagnostic_window() {
    let Some(rs) = headless() else {
        return;
    };
    let (doc, cels) = gros_dessins(2);
    let mut gpu = GpuCanvas::new(&rs);
    // (Au-delà du budget : un quart de moins, 1 024 → 768 Mo.)
    poser_lecture_essai(Some(Lecture { seq: 41, budget: 4 * GO, utilise: 4 * GO }));
    render_with(&mut gpu, &rs, &doc, &Plan { paper: None, items: vec![line_item(cels[0])] }, false);
    poser_lecture_essai(None);
    assert!(
        crate::memoirevideo::journal().iter().any(|l| l.contains("ramené à 768 Mo") && l.contains("réglé : 1024 Mo")),
        "{:?}",
        crate::memoirevideo::journal()
    );
}

/// Relecture de Fable (2026-10-24) : un pilote qui garde la mémoire des textures chassées (sous-allocations de Vulkan)
/// laisse `utilise` au même niveau pendant que le cache baisse. Sans retenue, chaque lecture voyait moins de place et le
/// plafond descendait jusqu'au plancher ; maintenant il baisse une fois, voit que rien n'a été rendu, et tient.
#[test]
fn a_driver_that_keeps_evicted_memory_does_not_ratchet_the_cache_down() {
    use crate::memoirevideo::{prochain_plafond, Suivi};
    let (regle, budget) = (GO, 4 * GO);
    // SHOT au-delà de son budget, et rien de ce que le cache lâche ne revient.
    let mut s = Suivi::default();
    let mut propre = GO;
    let mut plafonds = Vec::new();
    for seq in 1..=20 {
        s = prochain_plafond(s, Lecture { seq, budget, utilise: 4 * GO }, propre, regle, 32 * MO);
        plafonds.push(effectif(regle, s.plafond));
        propre = propre.min(effectif(regle, s.plafond));
    }
    assert_eq!(plafonds[0], 768 * MO, "un quart de moins");
    assert!(plafonds.iter().all(|&p| p == 768 * MO), "le plafond tient : {plafonds:?}");
    // La mémoire est rendue : la baisse reprend, vers la cible, d'un quart au plus par lecture, jamais sous l'image.
    let mut s = Suivi::default();
    let (mut propre, hors_cache) = (GO, 3 * GO);
    let mut plafonds = Vec::new();
    for seq in 1..=30 {
        s = prochain_plafond(s, Lecture { seq, budget, utilise: hors_cache + propre }, propre, regle, 200 * MO);
        plafonds.push(effectif(regle, s.plafond));
        propre = propre.min(effectif(regle, s.plafond));
    }
    assert!(plafonds.windows(2).all(|w| w[1] <= w[0] && w[1] >= w[0] - w[0] / 4), "{plafonds:?}");
    assert_eq!(*plafonds.last().unwrap(), 4 * GO - 3 * GO - 4 * GO / 10, "la cible est atteinte : {plafonds:?}");
    // Une image affichée plus grosse que la cible : le plafond ne descend pas sous elle.
    let s = prochain_plafond(Suivi { plafond: Some(300 * MO), baisse: None }, Lecture { seq: 1, budget, utilise: 4 * GO }, 300 * MO, regle, 280 * MO);
    assert_eq!(s.plafond, Some(280 * MO));
    // Sous le seuil (budget moins la marge) : jamais de baisse, même si la cible est plus basse que le plafond.
    let s = prochain_plafond(Suivi { plafond: Some(900 * MO), baisse: None }, Lecture { seq: 1, budget, utilise: 3 * GO }, 900 * MO, regle, 0);
    assert_eq!(s.plafond, Some(900 * MO).max(Some(cible_cache(budget, 3 * GO, 900 * MO))));
    assert!(effectif(regle, s.plafond) >= 900 * MO);
}

/// La même chose sur une vraie carte : le cache réglé à 160 Mo, SHOT au-delà de son budget et la mémoire jamais rendue.
#[test]
fn the_gpu_cache_holds_when_evictions_free_nothing() {
    let Some(rs) = headless() else {
        return;
    };
    let (doc, cels) = gros_dessins(10);
    let mut gpu = GpuCanvas::new(&rs);
    gpu.set_cache_bytes(160 * MO);
    poser_lecture_essai(None);
    for &c in &cels {
        render_with(&mut gpu, &rs, &doc, &Plan { paper: None, items: vec![line_item(c)] }, false);
    }
    assert_eq!(gpu.stats().bytes, 160 * MO);
    let deux = Plan { paper: None, items: vec![line_item(cels[3]), line_item(cels[7])] };
    for seq in 1..=12 {
        poser_lecture_essai(Some(Lecture { seq, budget: 4 * GO, utilise: 4 * GO }));
        render_with(&mut gpu, &rs, &doc, &deux, false);
    }
    poser_lecture_essai(None);
    assert_eq!(gpu.cache_limit(), 120 * MO, "une baisse d'un quart, puis le plafond tient");
    assert_eq!(gpu.stats().bytes, 112 * MO, "sept dessins gardés, pas le plancher");
}

// ---------------------------------------------------------------------------------------------- mesure sur cette machine

/// Un autre processus qui tient de la mémoire vidéo (`SHOT_OCCUPER_MO` mégaoctets de tampons, effacés par la carte pour
/// qu'ils soient vraiment posés), jusqu'à ce qu'on l'arrête (au plus deux minutes). Lancé par `mesure_memoire_video`.
#[test]
#[ignore]
fn occupe_la_carte() {
    let Ok(mo) = std::env::var("SHOT_OCCUPER_MO").map(|v| v.parse::<u64>().unwrap_or(0)) else { return };
    let Some(rs) = headless() else { return };
    let mut tampons = Vec::new();
    let morceau = 256 * MO;
    let mut enc = rs.device.create_command_encoder(&Default::default());
    for _ in 0..mo * MO / morceau {
        let b = rs.device.create_buffer(&eframe::egui_wgpu::wgpu::BufferDescriptor { label: Some("occupe"), size: morceau, usage: eframe::egui_wgpu::wgpu::BufferUsages::COPY_DST | eframe::egui_wgpu::wgpu::BufferUsages::STORAGE, mapped_at_creation: false });
        enc.clear_buffer(&b, 0, None);
        tampons.push(b);
    }
    rs.queue.submit([enc.finish()]);
    rs.device.poll(eframe::egui_wgpu::wgpu::Maintain::Wait);
    println!("OCCUPE {mo} Mo");
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_secs(120) {
        // (Toutes les secondes, la carte relit les tampons : ils restent en mémoire vidéo.)
        let mut enc = rs.device.create_command_encoder(&Default::default());
        for b in &tampons {
            enc.clear_buffer(b, 0, Some(4096));
        }
        rs.queue.submit([enc.finish()]);
        rs.device.poll(eframe::egui_wgpu::wgpu::Maintain::Wait);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Le temps d'une image recomposée (envoi, composition, affichage dans un petit écran, attente de la carte), en ms :
/// médiane et pire, sur `n` images où l'opacité d'un calque change (rien n'est renvoyé, tout se recompose).
fn images_recomposees(gpu: &mut GpuCanvas, rs: &eframe::egui_wgpu::RenderState, doc: &Document, cels: &[crate::document::CelId], n: usize) -> (f64, f64) {
    use eframe::egui::{pos2, vec2, Rect};
    let mut temps = Vec::new();
    for f in 0..n {
        let mut items: Vec<_> = cels.iter().map(|&c| line_item(c)).collect();
        items[0].opacity = if f % 2 == 0 { 255 } else { 200 };
        let plan = Plan { paper: Some([255, 255, 255]), items };
        let t = std::time::Instant::now();
        let resolved = gpu.sync(doc, &plan, false, &[], &[], 0);
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(256.0, 144.0));
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        let cb = gpu.paint(&plan, &resolved, corners, None, rect, [doc.width, doc.height]);
        crate::gpu_tests::read_callback(rs, 256, 144, rect, cb);
        temps.push(t.elapsed().as_secs_f64() * 1e3);
    }
    temps.sort_by(f64::total_cmp);
    (temps[temps.len() / 2], temps[temps.len() - 1])
}

/// Mesure sur cette machine (`--ignored --nocapture`) : un projet 4K de 60 dessins pleins (2 Go de textures) parcouru
/// une fois (le cache se remplit jusqu'à son poids réglé, 2 Go comme le diagnostic le règle pour 8 Go de mémoire vidéo),
/// puis des images recomposées sur quatre calques. Un autre processus tient `SHOT_OCCUPER_MO` (3 000 Mo par défaut) de
/// mémoire vidéo. Avant (aucune lecture du budget) puis après (les vraies lectures), dans le même processus.
#[test]
#[ignore]
fn mesure_memoire_video() {
    let Some(rs) = headless() else { return };
    let info = rs.adapter.get_info();
    println!("carte : {} ({:?})", info.name, info.backend);
    crate::memoirevideo::lectures_reelles_pour_essai(&info, true);
    let attendre_lecture = |apres: u64| {
        let t = std::time::Instant::now();
        while crate::memoirevideo::lecture().is_none_or(|l| l.seq <= apres) && t.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        crate::memoirevideo::lecture()
    };
    let Some(l0) = attendre_lecture(0) else {
        println!("budget illisible sur cette carte : rien à mesurer");
        return;
    };
    println!("au départ : budget {} Mo, ce processus utilise {} Mo", l0.budget / MO, l0.utilise / MO);
    let occuper: u64 = std::env::var("SHOT_OCCUPER_MO").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    // Le projet : 3840 × 2160, 60 dessins pleins de couleurs différentes.
    let cache_mo: u64 = std::env::var("SHOT_CACHE_MO").ok().and_then(|v| v.parse().ok()).unwrap_or(2048);
    let n = (cache_mo / 32) as usize / 4 * 4 + 4;
    let mut doc = Document::new(3840, 2160, 24);
    // (Les dessins partagent leurs tuiles en mémoire centrale — elles sont comptées par référence — mais chacun a sa
    // texture sur la carte : la mesure ne prend pas 4 Go de mémoire à la machine.)
    let mut modele = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    for t in 0..modele.raster.tiles.len() {
        modele.raster.tile_mut(t).fill(Color32::from_rgba_premultiplied(30, 40, 90, 255));
    }
    let cels: Vec<_> = (0..n).map(|_| doc.add_cel(modele.clone())).collect();
    let mut hog: Option<std::process::Child> = None;
    let lancer_hog = || {
        let exe = std::env::current_exe().expect("exécutable des essais");
        let mut c = std::process::Command::new(exe)
            .args(["--ignored", "--exact", "memoirevideo_tests::occupe_la_carte", "--nocapture", "--test-threads=1"])
            .env("SHOT_OCCUPER_MO", occuper.to_string())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("processus qui occupe la carte");
        // (Il dit quand ses tampons sont posés.)
        let mut sortie = std::io::BufReader::new(c.stdout.take().expect("sortie"));
        let mut ligne = String::new();
        while std::io::BufRead::read_line(&mut sortie, &mut ligne).unwrap_or(0) > 0 && !ligne.contains("OCCUPE") {
            ligne.clear();
        }
        c
    };
    for (nom, avec_hog) in [("sans autre processus", false), ("un autre processus tient la carte", true)] {
        if avec_hog && hog.is_none() {
            hog = Some(lancer_hog());
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        // (`SHOT_ORDRE=apres` : l'après d'abord, pour voir ce que l'ordre des passes change.)
        let mut phases = [("avant (poids réglé seul)", false), ("après (budget du système)", true)];
        if std::env::var("SHOT_ORDRE").is_ok_and(|v| v == "apres") {
            phases.reverse();
        }
        for (phase, reelles) in phases {
            crate::memoirevideo::lectures_reelles_pour_essai(&info, reelles);
            let mut gpu = GpuCanvas::new(&rs);
            gpu.set_cache_bytes(cache_mo * MO);
            // Le projet parcouru une fois, quatre calques par image (le cache se remplit).
            for f in 0..cels.len() / 4 {
                let items = cels[f * 4..f * 4 + 4].iter().map(|&c| line_item(c)).collect();
                let plan = Plan { paper: Some([255, 255, 255]), items };
                gpu.sync(&doc, &plan, false, &[], &[], 0);
                // (Une lecture du budget toutes les 1,5 s : le parcours dure assez pour en voir.)
                rs.device.poll(eframe::egui_wgpu::wgpu::Maintain::Wait);
            }
            // Six secondes d'images recomposées (quatre lectures du budget) ; la mesure est celle des trois dernières.
            let debut = std::time::Instant::now();
            let mut vu = 0;
            let (mut med, mut pire) = (0.0, 0.0);
            while debut.elapsed() < std::time::Duration::from_secs(6) {
                let r = images_recomposees(&mut gpu, &rs, &doc, &cels[cels.len() - 4..], 24);
                if debut.elapsed() > std::time::Duration::from_secs(3) {
                    (med, pire) = r;
                }
                if let Some(l) = crate::memoirevideo::lecture().filter(|l| reelles && l.seq != vu) {
                    vu = l.seq;
                    println!(
                        "  {:.1} s : budget {} Mo, ce processus {} Mo, cache {} Mo (visé {} Mo), image {:.2} ms",
                        debut.elapsed().as_secs_f64(),
                        l.budget / MO,
                        l.utilise / MO,
                        gpu.stats().bytes / MO,
                        gpu.cache_limit() / MO,
                        r.0
                    );
                }
            }
            let l = crate::memoirevideo::lecture();
            println!(
                "MEMOIRE {nom}, {phase} : image recomposée médiane {med:.2} ms, pire {pire:.2} ms ; cache {} Mo (visé {} Mo) ; budget {} Mo, ce processus {} Mo",
                gpu.stats().bytes / MO,
                gpu.cache_limit() / MO,
                l.map_or(0, |l| l.budget / MO),
                l.map_or(0, |l| l.utilise / MO)
            );
            drop(gpu);
            rs.device.poll(eframe::egui_wgpu::wgpu::Maintain::Wait);
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    if let Some(mut h) = hog {
        let _ = h.kill();
        let _ = h.wait();
    }
    crate::memoirevideo::lectures_reelles_pour_essai(&info, false);
    for l in crate::memoirevideo::journal() {
        println!("journal : {l}");
    }
}
