//! Essai d'endurance d'un épisode (2026-10-31, branche `endurance`) : un générateur d'épisode réaliste (150 à 250 plans, 11 à 22
//! minutes, plusieurs séquences, plusieurs calques par plan — bitmap, vectoriel, quelques plans animés —, clés de caméra, trois
//! pistes de son, notes, versions), puis des bancs de mesure : ouvrir, naviguer, lire, éditer au début de l'épisode, une longue
//! séance de 2 000 gestes, enregistrer (tout, puis ce qui change), la copie de secours, deux envois, les exports.
//!
//! Tous `#[ignore]`, lancés à la main (le générateur de dessins est celui du film lourd, `lourd_tests.rs`, dont les plans sont
//! seulement allongés : `DUREE_PCT`). Voir `docs/ENDURANCE.md` : méthode, chiffres, ce qui reste.
//!
//! ```text
//! cargo test --release --offline --target-dir target\msvc endurance_generer -- --ignored --nocapture
//! cargo test --release --offline --target-dir target\msvc endurance_naviguer -- --ignored --nocapture
//! cargo test --release --offline --target-dir target\msvc endurance_editer   -- --ignored --nocapture
//! cargo test --release --offline --target-dir target\msvc endurance_exports  -- --ignored --nocapture
//! ```
//!
//! Variables : `ENDU_DIR` (dossier de travail, `%TEMP%\shot-endurance`), `ENDU_PLANS` (200), `ENDU_DUREE` (pour cent de la durée des
//! plans du film lourd : 250), `ENDU_OPS` (nombre de gestes de la longue séance, 2000), `ENDU_GRAINE`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Event, Modifiers, Rect};

use crate::camera::Camera;
use crate::document::Document;
use crate::lourd_tests::{
    appui, bouge, cpu, film_lourd, incoherences, installer_crochet, jusqu_au_calme, memoire, memoire_pic, ouvrir_mesure, scene, taille_dossier, tri, wav, Hasard, DUREE_PCT,
};
use crate::perf_tests::{Banc, Mesure};
use crate::workspace::Espace;

/// Le tas vivant du processus d'essais (octets alloués et pas encore rendus) : distingue ce que SHOT tient de ce que Windows garde (tas
/// fragmenté, mémoire de la carte graphique comptée dans la mémoire privée). Un compteur autour de l'allocateur du système ; il ne change
/// ni le tas segmenté ni les décisions de SHOT.
struct TasCompte;

static TAS_VIVANT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// SAFETY : chaque appel est transmis tel quel à l'allocateur du système ; le compteur n'est qu'une addition.
unsafe impl std::alloc::GlobalAlloc for TasCompte {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = std::alloc::System.alloc(l);
        if !p.is_null() {
            TAS_VIVANT.fetch_add(l.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        std::alloc::System.dealloc(p, l);
        TAS_VIVANT.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = std::alloc::System.alloc_zeroed(l);
        if !p.is_null() {
            TAS_VIVANT.fetch_add(l.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        let q = std::alloc::System.realloc(p, l, n);
        if !q.is_null() {
            TAS_VIVANT.fetch_add(n, Ordering::Relaxed);
            TAS_VIVANT.fetch_sub(l.size(), Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static TAS: TasCompte = TasCompte;

/// Octets du tas vivant, en Mo.
pub(crate) fn tas_vivant_mo() -> f64 {
    TAS_VIVANT.load(Ordering::Relaxed) as f64 / 1e6
}

fn env_usize(nom: &str, defaut: usize) -> usize {
    std::env::var(nom).ok().and_then(|v| v.parse().ok()).unwrap_or(defaut)
}

pub(crate) fn dossier_endurance() -> PathBuf {
    std::env::var_os("ENDU_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("shot-endurance"))
}

fn manifeste_de(dir: &Path) -> PathBuf {
    dir.join("Episode").join("Episode.shot")
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Une ligne de résultat, reconnaissable par `findstr ENDU`.
macro_rules! endu {
    ($($a:tt)*) => { println!("ENDU {}", format!($($a)*)) };
}

// ------------------------------------------------------------------ l'épisode

/// L'épisode : le film lourd aux plans allongés, avec en plus des pistes de dialogues et de bruitages (de courts sons témoins).
pub(crate) fn episode(plans: usize, pct: usize, seed: u64) -> (Document, crate::lourd_tests::Stats) {
    DUREE_PCT.store(pct, Ordering::Relaxed);
    let (mut doc, st) = film_lourd(plans, seed);
    DUREE_PCT.store(100, Ordering::Relaxed);
    let o = wav(2.0, seed + 40);
    let pcm = crate::audio::decode(&o, "wav").expect("le WAV fabriqué se décode");
    let voix = doc.add_sound("voix-temoin.wav".into(), "wav".into(), Arc::new(o), Arc::new(pcm));
    let o = wav(1.0, seed + 41);
    let pcm = crate::audio::decode(&o, "wav").expect("le WAV fabriqué se décode");
    let bruit = doc.add_sound("bruitage-temoin.wav".into(), "wav".into(), Arc::new(o), Arc::new(pcm));
    doc.add_audio_track();
    doc.add_audio_track();
    let (t_voix, t_bruit) = (doc.audio_tracks.len() - 2, doc.audio_tracks.len() - 1);
    doc.audio_tracks[t_voix].name = "Dialogues".into();
    doc.audio_tracks[t_bruit].name = "Bruitages".into();
    for i in 0..doc.panels.len() {
        let debut = doc.panel_start(i);
        let n = doc.panels[i].frame_count();
        if !doc.panels[i].dialogue.is_empty() && n > 12 {
            doc.add_clip(t_voix, voix, debut + 4, 0, 48.min(n - 6));
        }
        if i % 5 == 2 {
            doc.add_clip(t_bruit, bruit, debut + n / 2, 0, 24);
        }
    }
    doc.properties.title = format!("Épisode d'essai ({plans} plans)");
    doc.properties.serie = "Essai".into();
    doc.properties.episode = "1".into();
    (doc, st)
}

/// Fabrique l'épisode et l'enregistre dans `ENDU_DIR\Episode\Episode.shot` (le temps de fabrication, de l'enregistrement complet, la
/// taille sur le disque). À lancer une fois ; les bancs suivants lisent ce dossier.
#[test]
#[ignore]
fn endurance_generer() {
    let plans = env_usize("ENDU_PLANS", 200);
    let pct = env_usize("ENDU_DUREE", 250);
    let seed = env_usize("ENDU_GRAINE", 7) as u64;
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    let t = Instant::now();
    let (doc, st) = episode(plans, pct, seed);
    let fab = t.elapsed().as_secs_f64();
    let frames = doc.total_frames();
    endu!(
        "épisode : {} plans, {} séquences, {} cases, {} calques, {} dessins bitmap ({} tuiles = {:.0} Mo de pixels), {} dessins vectoriels, {} traits, {} images = {:.1} min à {} i/s, {} plans à caméra, {} plans animés, {} pistes de son, {} clips, {} dessins en tout, {} versions rangées, {} commentaires",
        st.plans,
        st.sequences,
        st.cases,
        st.calques,
        st.dessins_bitmap,
        st.tuiles,
        st.tuiles as f64 * 16384.0 / 1e6,
        st.dessins_vecteur,
        st.traits,
        frames,
        frames as f64 / doc.fps as f64 / 60.0,
        doc.fps,
        st.cameras,
        st.animes,
        doc.audio_tracks.len(),
        doc.audio_tracks.iter().map(|t| t.clips.len()).sum::<usize>(),
        doc.cels.len(),
        doc.versions.values().map(|v| v.autres.len()).sum::<usize>(),
        doc.comments.len()
    );
    endu!("fabrication : {fab:.1} s");
    let dir = dossier_endurance();
    let _ = std::fs::remove_dir_all(dir.join("Episode"));
    std::fs::create_dir_all(dir.join("Episode")).unwrap();
    let manifeste = manifeste_de(&dir);
    let t2 = Instant::now();
    let r = crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let (octets, fichiers) = taille_dossier(&dir.join("Episode"));
    endu!("enregistrement complet (project::save) : {:.1} s, {} dessins écrits, {:.0} Mo sur le disque, {} fichiers", t2.elapsed().as_secs_f64(), r.written, octets as f64 / 1e6, fichiers);
    mem_ligne("après la fabrication");
    // (Sans lâcher l'épisode : le tas de Windows mettrait des minutes, `lourd_tas`.)
    std::process::exit(0);
}

/// Les plages de mémoire engagée du processus, par sorte (privée, projetée, image) et les plus grosses (diagnostic : d'où vient ce que le
/// tas vivant ne compte pas).
fn plages_memoire(titre: &str) {
    #[repr(C)]
    struct Mbi {
        base: usize,
        alloc_base: usize,
        alloc_protect: u32,
        _p1: u32,
        taille: usize,
        etat: u32,
        protection: u32,
        sorte: u32,
        _p2: u32,
    }
    extern "system" {
        fn VirtualQuery(addr: usize, mbi: *mut Mbi, len: usize) -> usize;
    }
    let (mut privee, mut projetee, mut image) = (0usize, 0usize, 0usize);
    let mut grosses: Vec<(usize, u32, u32)> = Vec::new();
    let mut adresse = 0usize;
    loop {
        let mut m = Mbi { base: 0, alloc_base: 0, alloc_protect: 0, _p1: 0, taille: 0, etat: 0, protection: 0, sorte: 0, _p2: 0 };
        // SAFETY : VirtualQuery remplit la structure fournie, de la taille annoncée.
        let n = unsafe { VirtualQuery(adresse, &mut m, std::mem::size_of::<Mbi>()) };
        if n == 0 {
            break;
        }
        if m.etat == 0x1000 {
            match m.sorte {
                0x20000 => privee += m.taille,
                0x40000 => projetee += m.taille,
                0x1000000 => image += m.taille,
                _ => {}
            }
            if m.taille >= 32 << 20 {
                grosses.push((m.taille, m.sorte, m.protection));
            }
        }
        let suite = m.base.wrapping_add(m.taille);
        if suite <= adresse {
            break;
        }
        adresse = suite;
    }
    grosses.sort_unstable_by(|a, b| b.cmp(a));
    let g: Vec<String> = grosses.iter().take(8).map(|(t, s, p)| format!("{:.0} Mo (sorte {s:#x}, protection {p:#x})", *t as f64 / 1e6)).collect();
    endu!("plages {titre} : privée {:.0} Mo, projetée {:.0} Mo, image {:.0} Mo ; plus grosses : {}", privee as f64 / 1e6, projetee as f64 / 1e6, image as f64 / 1e6, g.join(" ; "));
}

fn mem_ligne(titre: &str) {
    plages_memoire(titre);
    let (ws, pv) = memoire();
    endu!("mémoire {titre} : ensemble de travail {ws:.0} Mo, privée {pv:.0} Mo, pic {:.0} Mo, tas vivant {:.0} Mo", memoire_pic(), tas_vivant_mo());
}

/// Copie le dossier de l'épisode (`ENDU_DIR\Episode`) dans `ENDU_DIR\<nom>` : les bancs qui enregistrent ne touchent pas à l'original.
fn copier_episode(nom: &str) -> PathBuf {
    fn rec(de: &Path, vers: &Path) {
        std::fs::create_dir_all(vers).unwrap();
        for e in std::fs::read_dir(de).unwrap().flatten() {
            let (p, q) = (e.path(), vers.join(e.file_name()));
            if p.is_dir() {
                rec(&p, &q);
            } else {
                std::fs::copy(&p, &q).unwrap();
            }
        }
    }
    let dir = dossier_endurance();
    assert!(manifeste_de(&dir).exists(), "pas d'épisode : lance d'abord endurance_generer");
    let cible = dir.join(nom);
    let _ = std::fs::remove_dir_all(&cible);
    let t = Instant::now();
    rec(&dir.join("Episode"), &cible.join("Episode"));
    endu!("copie de l'épisode pour {nom} : {:.1} s", t.elapsed().as_secs_f64());
    manifeste_de(&cible)
}

/// Ouvre `manifeste` dans un banc, dans l'espace `espace` (lecture sur un fil de fond, comme l'application).
fn ouvrir(manifeste: &Path, espace: Espace) -> Banc {
    std::env::set_var("LOURD_PROJET", manifeste);
    ouvrir_mesure(espace)
}

/// Images de `b` jusqu'à ce que `fini(b)` soit vrai ; rend (temps total en ms, images, la plus lente en ms, durées).
fn jusqu_a(b: &mut Banc, max_s: f64, mut fini: impl FnMut(&mut Banc) -> bool) -> (f64, Vec<Mesure>) {
    let t = Instant::now();
    let mut v = Vec::new();
    while !fini(b) && t.elapsed().as_secs_f64() < max_s {
        v.push(b.image(Vec::new(), 1.0 / 60.0));
        std::thread::sleep(Duration::from_millis(2));
    }
    (ms(t), v)
}

fn resume_images(titre: &str, v: &[Mesure]) {
    if v.is_empty() {
        endu!("{titre} : aucune image");
        return;
    }
    let (moy, p95, max) = tri(v.iter().map(Mesure::total).collect());
    let lent = |s: f64| v.iter().filter(|m| m.total() > s).count();
    endu!("{titre} : {} images, moy {moy:.1} ms, p95 {p95:.1}, pire {max:.1} ; > 33 ms : {}, > 50 ms : {}, > 100 ms : {}", v.len(), lent(33.3), lent(50.0), lent(100.0));
}

// ------------------------------------------------------------------ ouvrir, naviguer, lire

#[test]
#[ignore]
fn endurance_naviguer() {
    let dir = dossier_endurance();
    let manifeste = manifeste_de(&dir);
    assert!(manifeste.exists(), "pas d'épisode : lance d'abord endurance_generer");
    let cpu0 = cpu();
    let t = Instant::now();
    let mut b = ouvrir(&manifeste, Espace::Vignettes);
    endu!("ouvrir (Vignettes), de la demande au calme : {:.1} s", t.elapsed().as_secs_f64());
    mem_ligne("après l'ouverture et le calme");
    let n = b.app.doc.panels.len();
    let total = b.app.doc.total_frames();
    endu!("épisode ouvert : {n} plans, {total} images ({:.1} min)", total as f64 / b.app.doc.fps as f64 / 60.0);

    // Vignettes : toutes les cases (zoom arrière jusqu'à voir tout l'épisode), défilement.
    let vue = b.app.vignettes.view;
    let z0 = b.app.vignettes.xf.zoom;
    let v = scene("vignettes : Ctrl+molette, zoom arrière pour tout voir", &mut b, 90, 1.0 / 60.0, |b, _| {
        b.touches = Modifiers::CTRL;
        vec![Event::PointerMoved(vue.center()), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -80.0), modifiers: Modifiers::CTRL }]
    });
    b.touches = Modifiers::NONE;
    resume_images("vignettes, zoom arrière", &v);
    endu!("zoom de la planche : {z0:.3} -> {:.3}", b.app.vignettes.xf.zoom);
    let (i, msec, pire) = jusqu_au_calme(&mut b, 300.0);
    endu!("vignettes de toutes les cases : {i} images, {msec:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    scene("vignettes : défilement (molette)", &mut b, 150, 1.0 / 60.0, |_, k| {
        vec![Event::PointerMoved(vue.center()), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, if k < 100 { -120.0 } else { 300.0 }), modifiers: Modifiers::NONE }]
    });
    jusqu_au_calme(&mut b, 120.0);

    // Changer de plan (une image après chaque changement) : la durée du changement lui-même, et de l'image qui suit.
    for (espace, nom) in [(Espace::Vignettes, "Vignettes"), (Espace::Storyboard, "Storyboard"), (Espace::Montage, "Montage")] {
        b.app.set_espace(espace);
        b.chauffer(5);
        jusqu_au_calme(&mut b, 120.0);
        let mut choix = Vec::new();
        let mut images = Vec::new();
        for k in 0..n.min(200) {
            let t = Instant::now();
            b.app.select_panel((k * 7) % n);
            choix.push(ms(t));
            images.push(b.image(Vec::new(), 1.0 / 60.0));
        }
        let (moy, p95, max) = tri(choix);
        endu!("changer de plan ({nom}) : select_panel moy {moy:.2} ms, p95 {p95:.2}, pire {max:.2}");
        resume_images(&format!("images après chaque changement de plan ({nom})"), &images);
        jusqu_au_calme(&mut b, 120.0);
    }

    // Parcourir la timeline : tête de lecture qui saute, puis qui glisse sur tout l'épisode.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(5);
    let montage = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    let m = scene("montage : tête de lecture qui saute (120 sauts)", &mut b, 120, 1.0 / 60.0, |b, k| {
        b.app.goto_global((k * 977) % total);
        bouge(k, montage)
    });
    resume_images("tête qui saute", &m);
    let pas = (total / 600).max(1);
    let m = scene("montage : tête qui parcourt tout l'épisode (600 pas)", &mut b, 600, 1.0 / 60.0, |b, k| {
        b.app.goto_global((k * pas).min(total - 1));
        Vec::new()
    });
    resume_images("tête qui parcourt l'épisode", &m);
    jusqu_au_calme(&mut b, 120.0);
    mem_ligne("après la navigation");

    // Lecture de tout l'épisode, à 24 images par seconde.
    b.app.set_espace(Espace::Montage);
    b.chauffer(5);
    jusqu_au_calme(&mut b, 120.0);
    b.app.goto_global(0);
    b.app.toggle_play_scope(true);
    let (pas_mem, mut courbe) = (2400usize, Vec::new());
    crate::perf::activer(true);
    let mut toutes: Vec<Mesure> = Vec::with_capacity(total);
    let t = Instant::now();
    let mut k = 0;
    while b.app.playing && k < total + 200 {
        toutes.push(b.image(Vec::new(), 1.0 / 24.0));
        k += 1;
        if k % pas_mem == 0 {
            let (ws, pv) = memoire();
            courbe.push(format!("image {k} : RSS {ws:.0} Mo, privée {pv:.0} Mo"));
        }
    }
    let r = crate::perf::resume();
    crate::perf::activer(false);
    b.app.playing = false;
    endu!("lecture de l'épisode : {k} images d'écran en {:.0} s (tête à {} sur {total})", t.elapsed().as_secs_f64(), b.app.cur_frame);
    resume_images("lecture de tout l'épisode", &toutes);
    // Par tranche de 10 % de l'épisode : le pire de chaque tranche (une dérive dirait une fuite ou un cache qui gonfle).
    let tranche = (toutes.len() / 10).max(1);
    let par_tranche: Vec<String> = toutes.chunks(tranche).map(|c| format!("{:.1}/{:.0}", c.iter().map(Mesure::total).sum::<f64>() / c.len() as f64, c.iter().map(Mesure::total).fold(0.0, f64::max))).collect();
    endu!("lecture, moyenne/pire par dixième (ms) : {}", par_tranche.join("  "));
    for l in courbe {
        endu!("lecture, mémoire à {l}");
    }
    let tete: Vec<String> = r.sections.iter().take(8).map(|(nom, moy, max)| format!("{nom} {moy:.2}/{max:.1}")).collect();
    endu!("lecture, sections (moy/pire ms) : {}", tete.join(" | "));
    mem_ligne("après la lecture");
    jusqu_au_calme(&mut b, 120.0);
    mem_ligne("après le calme qui suit la lecture");
    endu!("processeur total du banc : {:.0} s", cpu() - cpu0);
    std::process::exit(0);
}

// ------------------------------------------------------------------ éditer : au début de l'épisode, puis une longue séance

/// Une opération de la séance, choisie par `op` ; rend ce qui a été fait. `i` est un rang de plan (souvent près du début),
/// `k` un petit nombre.
fn operation(b: &mut Banc, h: &mut Hasard, op: usize, i: usize, k: usize, espaces: &[Espace]) -> String {
    use crate::app::Tool;
    let n = b.app.doc.panels.len();
    match op {
        0 => {
            let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k.min(4)).map(|p| p.id).collect();
            b.app.structure_op(|d, _| d.remove_panels(&ids));
            format!("supprimer {} plans dès {i}", ids.len())
        }
        1 => {
            b.app.select_panel(i);
            b.app.structure_op(|d, cur| Some(d.add_scene_after(cur)));
            format!("nouveau plan après {i}")
        }
        2 => {
            b.app.structure_op_dessins(|d, _| Some(d.duplicate_scene_after(i)));
            format!("dupliquer le plan {i}")
        }
        3 => {
            let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k.min(5)).map(|p| p.id).collect();
            let avant = b.app.doc.panels.get(h.n(0, 3.min(n - 1))).map(|p| p.id);
            let ok = b.app.structure_op_dessins(|d, _| d.move_plans(&ids, avant));
            format!("déplacer {} plans dès {i} vers le début ({ok})", ids.len())
        }
        4 => {
            b.app.select_panel(i);
            b.app.record_step();
            let len = b.app.doc.panels[i].frame_count();
            let c = b.app.doc.panel_camera(i, 0);
            let f = h.n(0, len - 1);
            let cam = Camera { x: c.x + h.entre(-300.0, 300.0), zoom: h.entre(0.7, 2.0), angle: h.entre(-10.0, 10.0), tilt: h.entre(-5.0, 5.0), ..c };
            crate::camera::set_key(&mut b.app.doc.panels[i].camera, f, cam);
            b.app.modified = true;
            format!("clé de caméra, plan {i} image {f}")
        }
        5 => {
            let e = *h.choix(espaces);
            b.app.set_espace(e);
            format!("onglet {e:?}")
        }
        6 => {
            for _ in 0..k {
                b.app.undo();
            }
            format!("{k} annulations")
        }
        7 => {
            for _ in 0..k {
                b.app.redo();
            }
            format!("{k} rétablissements")
        }
        8 => {
            b.app.select_panel(i);
            b.app.tool = Tool::Brush;
            b.chauffer(2);
            let (rect, ou) = if b.app.espace == Espace::Vignettes {
                let layout = b.app.board_layout();
                match layout.cards.iter().position(|c| c.panel == b.app.cur_panel) {
                    Some(kk) => (b.app.vignettes.xf.rect(layout.cards[kk].thumb), "case"),
                    None => return format!("trait : case du plan {i} hors de la vue"),
                }
            } else {
                (b.app.canvas_rect.shrink2(vec2(b.app.canvas_rect.width() * 0.2, b.app.canvas_rect.height() * 0.3)), "dessin")
            };
            let (de, a) = (rect.left_center() + vec2(rect.width() * 0.1, 0.0), rect.right_center() - vec2(rect.width() * 0.1, 0.0));
            b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
            b.image(vec![appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
            for s in 1..=12 {
                b.image(vec![Event::PointerMoved(de + (a - de) * (s as f32 / 12.0))], 1.0 / 60.0);
            }
            b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
            format!("trait dans le {ou} du plan {i}")
        }
        9 => {
            let len = b.app.doc.panels[i].frame_count();
            let at = h.n(1, len.max(2) - 1);
            let ok = b.app.structure_op_dessins(|d, _| d.split_plan(i, at));
            format!("couper le plan {i} à l'image {at} ({ok})")
        }
        10 => {
            let ok = b.app.structure_op(|d, _| {
                d.nouvelle_version(i, 1)?;
                Some(i)
            });
            format!("nouvelle version du plan {i} ({ok})")
        }
        11 => {
            b.app.select_panel(i);
            let len = b.app.doc.panels[i].frame_count();
            let f = h.n(0, len - 1);
            let ok = b.app.structure_op_porte(false, |d, cur| d.toggle_mark(cur, f).map(|_| cur));
            format!("marque {f} du plan {i} ({ok})")
        }
        12 => {
            let nn = h.n(8, 300);
            b.app.structure_op(|d, _| {
                d.set_duration(i, nn);
                Some(i)
            });
            format!("durée du plan {i} : {nn} images")
        }
        13 => {
            let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k.min(4)).map(|p| p.id).collect();
            let clip = b.app.doc.copy_panels(&ids);
            let ok = clip.is_some_and(|c| b.app.structure_op_dessins(|d, cur| d.paste_plans(&c, cur)));
            format!("copier-coller {} plans ({ok})", ids.len())
        }
        14 => {
            b.app.goto_global(h.n(0, b.app.doc.total_frames().saturating_sub(1)));
            b.app.toggle_play_scope(true);
            for _ in 0..10 {
                b.image(Vec::new(), 1.0 / 24.0);
            }
            b.app.playing = false;
            "lecture du montage, 10 images".into()
        }
        _ => {
            for _ in 0..10 {
                let j = h.n(0, b.app.doc.panels.len() - 1);
                b.app.select_panel(j);
                b.image(Vec::new(), 1.0 / 60.0);
            }
            "10 changements de plan".into()
        }
    }
}

/// Un geste « structure » au début de l'épisode, dans chaque onglet : sa durée sur le fil de l'interface, celle des images qui
/// suivent, et la cohérence du projet (le motif connu des plantages : un rang de plan resté dans l'interface après un changement
/// de structure).
fn au_debut(b: &mut Banc, nom: &str, mut f: impl FnMut(&mut Banc)) -> (f64, f64) {
    let t = Instant::now();
    f(b);
    let appel = ms(t);
    let mut pire = 0.0f64;
    for _ in 0..6 {
        pire = pire.max(b.image(Vec::new(), 1.0 / 60.0).total());
    }
    let inc = incoherences(b);
    endu!("{nom} : geste {appel:.1} ms, pire image après {pire:.1} ms, {} plans{}", b.app.doc.panels.len(), if inc.is_empty() { String::new() } else { format!(", INCOHÉRENCES : {}", inc.join(" ; ")) });
    (appel, pire)
}

#[test]
#[ignore]
fn endurance_editer() {
    installer_crochet();
    let manifeste = copier_episode("travail-edition");
    let cpu0 = cpu();
    let mut b = ouvrir(&manifeste, Espace::Vignettes);
    let n0 = b.app.doc.panels.len();
    mem_ligne("épisode ouvert");

    // --- Au début de l'épisode, dans chaque onglet (la renumérotation de tout ce qui suit)
    println!("--- Gestes de structure au début de l'épisode");
    let espaces = [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage];
    let mut pires: Vec<(String, f64, f64)> = Vec::new();
    for e in espaces {
        b.app.set_espace(e);
        b.chauffer(4);
        jusqu_au_calme(&mut b, 60.0);
        let tag = format!("{e:?}");
        b.app.select_panel(0);
        let (a, p) = au_debut(&mut b, &format!("[{tag}] nouveau plan au début"), |b| {
            b.app.select_panel(0);
            b.app.structure_op(|d, cur| Some(d.add_scene_after(cur)));
        });
        pires.push((format!("nouveau plan au début ({tag})"), a, p));
        let (a, p) = au_debut(&mut b, &format!("[{tag}] supprimer les 3 premiers plans"), |b| {
            let ids: Vec<u64> = b.app.doc.panels.iter().take(3).map(|p| p.id).collect();
            b.app.structure_op(|d, _| d.remove_panels(&ids));
        });
        pires.push((format!("supprimer 3 plans au début ({tag})"), a, p));
        let (a, p) = au_debut(&mut b, &format!("[{tag}] annuler"), |b| b.app.undo());
        pires.push((format!("annuler la suppression ({tag})"), a, p));
        let (a, p) = au_debut(&mut b, &format!("[{tag}] déplacer 5 plans de la fin au début"), |b| {
            let n = b.app.doc.panels.len();
            let ids: Vec<u64> = b.app.doc.panels.iter().skip(n - 5).map(|p| p.id).collect();
            let avant = b.app.doc.panels.first().map(|p| p.id);
            b.app.structure_op_dessins(|d, _| d.move_plans(&ids, avant));
        });
        pires.push((format!("déplacer 5 plans au début ({tag})"), a, p));
        let (a, p) = au_debut(&mut b, &format!("[{tag}] dupliquer le premier plan"), |b| {
            b.app.select_panel(0);
            b.app.structure_op_dessins(|d, _| Some(d.duplicate_scene_after(0)));
        });
        pires.push((format!("dupliquer le premier plan ({tag})"), a, p));
        let (a, p) = au_debut(&mut b, &format!("[{tag}] allonger le premier plan à 300 images"), |b| {
            b.app.structure_op(|d, _| {
                d.set_duration(0, 300);
                Some(0)
            });
        });
        pires.push((format!("allonger le premier plan ({tag})"), a, p));
        for _ in 0..3 {
            b.app.undo();
        }
        b.chauffer(3);
    }
    let (a, p) = au_debut(&mut b, "supprimer 40 plans au début d'un coup", |b| {
        let ids: Vec<u64> = b.app.doc.panels.iter().take(40).map(|p| p.id).collect();
        b.app.structure_op(|d, _| d.remove_panels(&ids));
    });
    pires.push(("supprimer 40 plans au début".into(), a, p));
    let (a, p) = au_debut(&mut b, "annuler (40 plans reviennent)", |b| b.app.undo());
    pires.push(("annuler la suppression de 40 plans".into(), a, p));
    jusqu_au_calme(&mut b, 120.0);
    endu!("gestes de structure : le plus lent {:.1} ms, la pire image après {:.1} ms ({} plans au départ, {} maintenant)", pires.iter().map(|x| x.1).fold(0.0, f64::max), pires.iter().map(|x| x.2).fold(0.0, f64::max), n0, b.app.doc.panels.len());
    for (nom, a, p) in pires.iter().filter(|x| x.1 > 50.0 || x.2 > 50.0) {
        endu!("   lent : {nom} : geste {a:.0} ms, image après {p:.0} ms");
    }

    // --- La longue séance
    println!("--- Longue séance");
    let ops = env_usize("ENDU_OPS", 2000);
    let mut h = Hasard::new(env_usize("ENDU_GRAINE", 11) as u64);
    let mut journal: Vec<String> = Vec::new();
    let mut lents: Vec<(String, f64, f64)> = Vec::new();
    let mut courbe: Vec<String> = Vec::new();
    let mut par_op: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    let mut incoh = 0;
    let t0 = Instant::now();
    let (ws0, pv0) = memoire();
    for tour in 0..ops {
        let n = b.app.doc.panels.len();
        // Les trois quarts des gestes visent le début de l'épisode (les rangs bas : tout ce qui suit se renumérote).
        let i = if h.chance(0.7) { h.n(0, 5.min(n - 1)) } else { h.n(0, n - 1) };
        let op = h.n(0, 15);
        let k = h.n(1, 6);
        // Le nombre de plans reste dans une fourchette : trop peu, on ne supprime plus ; trop, on ne duplique plus.
        let op = if n < n0 / 2 && matches!(op, 0 | 6) {
            1
        } else if n > n0 * 3 / 2 && matches!(op, 1 | 2 | 13) {
            0
        } else {
            op
        };
        let t = Instant::now();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(&mut b, &mut h, op, i, k, &espaces)));
        let ms_op = ms(t);
        let nom = match r {
            Ok(s) => s,
            Err(_) => format!("op {op} PLANTÉE"),
        };
        let r2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (0..3).map(|_| b.image(Vec::new(), 1.0 / 60.0).total()).fold(0.0, f64::max)));
        let pire = r2.unwrap_or(-1.0);
        // (Les gestes qui contiennent eux-mêmes des images — traits, lecture — se comptent par leur pire image seulement.)
        let propre = !matches!(op, 5 | 8 | 14 | 15);
        par_op.entry(nom.split(|c: char| c.is_ascii_digit()).next().unwrap_or("").trim().to_string()).or_default().push(if propre { ms_op } else { pire.max(0.0) });
        if (propre && ms_op > 50.0) || pire > 50.0 {
            lents.push((format!("tour {tour} : {nom}"), ms_op, pire));
        }
        journal.push(format!("{tour} {nom}"));
        let inc = incoherences(&b);
        if !inc.is_empty() && incoh < 12 {
            incoh += 1;
            endu!("INCOHÉRENCE au tour {tour} « {nom} » : {}", inc.iter().take(3).cloned().collect::<Vec<_>>().join(" ; "));
        }
        if pire < 0.0 || nom.contains("PLANTÉE") {
            endu!("PLANTAGE au tour {tour} : {nom} ; avant : {}", journal.iter().rev().skip(1).take(4).cloned().collect::<Vec<_>>().join(" | "));
        }
        if tour % 100 == 0 {
            let (ws, pv) = memoire();
            courbe.push(format!("tour {tour:>4} : {} plans, {} dessins, {} étapes d'annulation, RSS {ws:.0} Mo, privée {pv:.0} Mo, {:.0} s", b.app.doc.panels.len(), b.app.doc.cels.len(), b.app.history.undo_len(), t0.elapsed().as_secs_f64()));
        }
        if crate::lourd_tests::nb_plantages() > 20 {
            endu!("trop de plantages, arrêt");
            break;
        }
    }
    for l in &courbe {
        endu!("séance {l}");
    }
    jusqu_au_calme(&mut b, 120.0);
    let (ws1, pv1) = memoire();
    endu!("séance de {ops} gestes en {:.0} s ; mémoire {ws0:.0} -> {ws1:.0} Mo (ensemble de travail), {pv0:.0} -> {pv1:.0} Mo (privée) ; {} dessins, {} plans", t0.elapsed().as_secs_f64(), b.app.doc.cels.len(), b.app.doc.panels.len());
    lents.sort_by(|a, c| c.1.max(c.2).total_cmp(&a.1.max(a.2)));
    endu!("{} gestes dépassent 50 ms (le geste ou la pire des 3 images d'après), les pires :", lents.len());
    for (nom, a, p) in lents.iter().take(25) {
        endu!("   geste {a:>7.0} ms, image après {p:>6.0} ms : {nom}");
    }
    for (nom, v) in &par_op {
        let (moy, p95, max) = tri(v.clone());
        endu!("   par sorte « {nom} » : {} fois, moy {moy:.1} ms, p95 {p95:.1}, pire {max:.1}", v.len());
    }
    endu!("plantages : {}", crate::lourd_tests::nb_plantages());

    // Annuler tout, rétablir tout (l'historique garde 200 étapes).
    let etapes = b.app.history.undo_len();
    let mut d_undo = Vec::new();
    let mut p_undo = 0.0f64;
    let t = Instant::now();
    while b.app.history.undo_len() > 0 {
        let t1 = Instant::now();
        b.app.undo();
        d_undo.push(ms(t1));
        if d_undo.len() % 20 == 0 {
            p_undo = p_undo.max(b.image(Vec::new(), 1.0 / 60.0).total());
        }
    }
    let (moy, p95, max) = tri(d_undo);
    endu!("annuler {etapes} étapes : {:.1} s en tout, par étape moy {moy:.1} ms, p95 {p95:.1}, pire {max:.1} ; pire image {p_undo:.1} ms ; {} plans ; incohérences : {}", t.elapsed().as_secs_f64(), b.app.doc.panels.len(), incoherences(&b).len());
    let mut d_redo = Vec::new();
    let t = Instant::now();
    for _ in 0..etapes {
        let t1 = Instant::now();
        b.app.redo();
        d_redo.push(ms(t1));
    }
    let (moy, p95, max) = tri(d_redo);
    endu!("rétablir {etapes} étapes : {:.1} s en tout, par étape moy {moy:.1} ms, p95 {p95:.1}, pire {max:.1} ; incohérences : {}", t.elapsed().as_secs_f64(), incoherences(&b).len());
    jusqu_au_calme(&mut b, 120.0);
    mem_ligne("après la séance");
    repartition_memoire(&b, "après la séance");

    // --- Enregistrer
    println!("--- Enregistrer");
    let enregistrer = |b: &mut Banc, titre: &str| {
        let t = Instant::now();
        b.app.save();
        let appel = ms(t);
        let mut pire = 0.0f64;
        let mut images = 0;
        while b.app.save_en_cours() {
            pire = pire.max(b.image(Vec::new(), 1.0 / 60.0).total());
            images += 1;
            std::thread::sleep(Duration::from_millis(2));
        }
        endu!("{titre} : appel (fil de l'interface) {appel:.1} ms, fin {:.0} ms, {images} images pendant, la plus lente {pire:.1} ms, état : {}", ms(t), b.app.status);
    };
    enregistrer(&mut b, "Ctrl+S après la séance (incrémental)");
    enregistrer(&mut b, "Ctrl+S, rien de modifié");
    // Un seul dessin change : l'enregistrement qui suit n'écrit que lui.
    b.app.select_panel(0);
    b.app.tool = crate::app::Tool::Brush;
    b.chauffer(2);
    let r = b.app.canvas_rect.shrink2(vec2(b.app.canvas_rect.width() * 0.2, b.app.canvas_rect.height() * 0.3));
    let _ = operation(&mut b, &mut Hasard::new(5), 8, 0, 1, &[Espace::Storyboard]);
    let _ = r;
    enregistrer(&mut b, "Ctrl+S après un trait (un dessin)");
    let ailleurs = dossier_endurance().join("copie-enregistrement");
    let _ = std::fs::remove_dir_all(&ailleurs);
    std::fs::create_dir_all(&ailleurs).unwrap();
    let chemin = b.app.path.clone();
    b.app.path = Some(ailleurs.join("Copie.shot"));
    b.app.written.clear();
    b.app.modified = true;
    enregistrer(&mut b, "premier enregistrement (tout écrire)");
    let (o, f) = taille_dossier(&ailleurs);
    endu!("taille du projet enregistré : {:.0} Mo, {f} fichiers", o as f64 / 1e6);
    b.app.path = chemin;
    let _ = std::fs::remove_dir_all(&ailleurs);

    // --- Copie de secours (enregistrement automatique pendant les gestes)
    println!("--- Copie de secours");
    let racine = dossier_endurance().join("secours");
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(&racine).unwrap();
    b.app.recovery = crate::recovery::Recovery::start(&racine).ok();
    assert!(b.app.recovery.is_some(), "dossier de secours");
    b.app.modified = true;
    for essai in 0..3 {
        // Un geste, puis une copie demandée tout de suite ; les images pendant qu'elle s'écrit.
        let _ = operation(&mut b, &mut Hasard::new(70 + essai), 4, 3, 1, &[Espace::Storyboard]);
        b.app.recovery.as_mut().unwrap().demander();
        let t = Instant::now();
        let mut v = Vec::new();
        loop {
            v.push(b.image(Vec::new(), 1.0 / 60.0));
            let rec = b.app.recovery.as_ref().unwrap();
            if !rec.busy() && !rec.en_attente() && v.len() > 3 {
                break;
            }
            if t.elapsed().as_secs() > 300 {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        resume_images(&format!("copie de secours n°{} ({:.0} ms, {} Mo écrits)", essai + 1, ms(t), taille_dossier(&racine).0 / 1_000_000), &v);
    }
    // Pendant un trait : la copie attend (hold), puis part.
    b.app.recovery.as_mut().unwrap().demander();
    let v: Vec<Mesure> = (0..60).map(|_| b.image(Vec::new(), 1.0 / 60.0)).collect();
    resume_images("images avec une copie de secours demandée (60)", &v);
    b.app.recovery = None;

    // --- Deux envois
    println!("--- Envois");
    let ctx = b.ctx.clone();
    for (num, nom) in [(1, "v1 au réal"), (2, "v2 au réal")] {
        if num == 2 {
            for k in 0..8 {
                let _ = operation(&mut b, &mut Hasard::new(90 + k), 8, (k as usize * 3) % 20, 1, &[Espace::Storyboard]);
            }
        }
        let t = Instant::now();
        b.app.envois_marquer(&ctx, nom, 1_761_900_000);
        let appel = ms(t);
        let cp = b.app.envois_ui.derniere_copie.map_or(0.0, |d| d.as_secs_f64() * 1e3);
        let mut v = Vec::new();
        while b.app.envois_ui.occupe() || b.app.save_en_cours() {
            v.push(b.image(Vec::new(), 1.0 / 60.0));
            std::thread::sleep(Duration::from_millis(2));
            if t.elapsed().as_secs() > 900 {
                break;
            }
        }
        endu!("envoi « {nom} » : appel {appel:.1} ms (copie des plans {cp:.1} ms), fini en {:.1} s, statut : {}", t.elapsed().as_secs_f64(), b.app.status);
        resume_images(&format!("images pendant l'envoi « {nom} »"), &v);
    }
    // Le suivi qui suit un envoi (relevé des plans modifiés).
    let t = Instant::now();
    let _ = operation(&mut b, &mut Hasard::new(99), 8, 1, 1, &[Espace::Storyboard]);
    let (msec, v) = jusqu_a(&mut b, 300.0, |b| {
        let u = &b.app.envois_ui;
        !u.occupe() && u.suivi_fait() && u.au_repos() && !b.app.vignettes_en_cours()
    });
    resume_images(&format!("images pendant le relevé de suivi des envois ({msec:.0} ms après le geste, {:.0} ms depuis)", ms(t)), &v);
    let (o, f) = taille_dossier(manifeste.parent().unwrap());
    endu!("projet avec deux envois : {:.0} Mo, {f} fichiers", o as f64 / 1e6);
    mem_ligne("fin de l'édition");
    endu!("processeur total du banc : {:.0} s", cpu() - cpu0);
    std::process::exit(0);
}

// ------------------------------------------------------------------ exports

/// Suit un export lancé dans `b` : images pendant l'export (mesure de l'interface), mémoire maximale, durée.
fn suivre_export(b: &mut Banc, titre: &str, fini: impl Fn(&Banc) -> bool, max_s: f64) -> bool {
    let (ws0, _) = memoire();
    let pic0 = memoire_pic();
    let t = Instant::now();
    let mut v = Vec::new();
    let mut ws_max = ws0;
    let mut k = 0usize;
    let r = b.app.canvas_rect;
    while !fini(b) && t.elapsed().as_secs_f64() < max_s {
        // Une souris qui bouge : l'interface travaille comme pendant que l'utilisateur continue.
        v.push(b.image(bouge(k, r), 1.0 / 60.0));
        k += 1;
        if k % 8 == 0 {
            ws_max = ws_max.max(memoire().0);
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    let ok = fini(b);
    endu!("{titre} : {:.1} s{} ; mémoire de SHOT {ws0:.0} -> max {ws_max:.0} Mo (pic du processus {:.0} Mo, avant {pic0:.0}) ; ffmpeg non compté", t.elapsed().as_secs_f64(), if ok { "" } else { " (ARRÊTÉ : trop long)" }, memoire_pic());
    resume_images(&format!("  interface pendant « {titre} »"), &v);
    ok
}

#[test]
#[ignore]
fn endurance_exports() {
    use crate::export::{self, ExportJob, Format, Range, SoundSpan};
    use crate::paquet::{PaquetJob, Reglages as PaquetReglages};
    let manifeste = manifeste_de(&dossier_endurance());
    assert!(manifeste.exists(), "pas d'épisode : lance d'abord endurance_generer");
    let sortie = dossier_endurance().join("exports");
    let _ = std::fs::remove_dir_all(&sortie);
    std::fs::create_dir_all(&sortie).unwrap();
    let cpu0 = cpu();
    let mut b = ouvrir(&manifeste, Espace::Storyboard);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(5);
    jusqu_au_calme(&mut b, 120.0);
    mem_ligne("épisode ouvert");
    let ffmpeg = export::find_ffmpeg(None);
    let enc = ffmpeg.as_ref().and_then(|f| export::probe(f).ok());
    endu!("ffmpeg : {:?} ; encodeurs : {:?}", ffmpeg, enc);
    let frames_all = export::frames_for(&b.app.doc, Range::Montage, 0);
    endu!("épisode à exporter : {} images, {} plans", frames_all.len(), b.app.doc.panels.len());

    // MP4 de tout l'épisode, avec le son.
    if let (Some(ff), Some(enc)) = (ffmpeg.clone(), enc.clone()) {
        let out = sortie.join("animatique.mp4");
        let (frame, fps) = (b.app.doc.frame, b.app.doc.fps as u32);
        let (start, len) = export::sample_span(b.app.doc.fps, 0, frames_all.len());
        let o2 = out.clone();
        let job = ExportJob::video_with_sound_alpha(&ff, move |wav| export::video_args(Format::Mp4, &enc, frame, fps, false, wav, &o2), &out, frames_all.clone(), Some(SoundSpan { start, len }), false);
        let t = Instant::now();
        b.app.start_job(job);
        endu!("MP4 : démarrage (fil de l'interface) {:.1} ms", ms(t));
        suivre_export(&mut b, "MP4 de tout l'épisode (1920x1080, avec son)", |b| b.app.export_job.as_ref().is_none_or(|j| j.outcome.is_some()), 3600.0);
        endu!("MP4 : {:?}, {:.0} Mo", b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).map(|r| r.map(|_| "ok")), std::fs::metadata(&out).map_or(0.0, |m| m.len() as f64 / 1e6));
        b.app.export_job = None;
        b.app.export_projet = None;
    } else {
        endu!("MP4 sauté : pas de ffmpeg");
    }

    // PDF du storyboard.
    {
        let out = sortie.join("storyboard.pdf");
        let t = Instant::now();
        let panels = export::panels_of(&frames_all);
        let (shots, boards) = export::storyboard_of_with(&b.app.doc, &panels, false);
        let preparation = ms(t);
        let job = ExportJob::pdf_layout(&out, "Episode", shots, boards, b.app.doc.frame, Default::default());
        let t = Instant::now();
        b.app.start_job(job);
        endu!("PDF : préparation des données {preparation:.1} ms + démarrage {:.1} ms (fil de l'interface)", ms(t));
        suivre_export(&mut b, "PDF du storyboard (tout l'épisode)", |b| b.app.export_job.as_ref().is_none_or(|j| j.outcome.is_some()), 3600.0);
        endu!("PDF : {:?}, {:.0} Mo", b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).map(|r| r.map(|_| "ok")), std::fs::metadata(&out).map_or(0.0, |m| m.len() as f64 / 1e6));
        b.app.export_job = None;
        b.app.export_projet = None;
    }

    // Liste des plans (CSV + JSON), sur le fil appelant : c'est un calcul de texte.
    {
        let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
        let t = Instant::now();
        let l = crate::listeplans::construire(&b.app.doc, &tous, 1001, None, false);
        endu!("liste des plans : {} lignes construites en {:.1} ms (sur le fil appelant)", l.len(), ms(t));
    }

    // Paquet de livraison : vidéos par plan, vignettes, PDF, animatique, liste.
    let paquet = |b: &mut Banc, titre: &str, r: PaquetReglages, ffmpeg: Option<(PathBuf, export::Encoders)>, dest: &Path| {
        let _ = std::fs::remove_dir_all(dest);
        std::fs::create_dir_all(dest).unwrap();
        let t = Instant::now();
        match PaquetJob::start(&b.app.doc, &r, 0, ffmpeg, dest, Vec::new()) {
            Ok(mut job) => {
                let mut v = Vec::new();
                let (ws0, _) = memoire();
                let mut ws_max = ws0;
                let mut k = 0usize;
                let rect = b.app.canvas_rect;
                while job.running() && t.elapsed().as_secs() < 3600 {
                    job.step(&b.app.doc, Duration::from_millis(8));
                    v.push(b.image(bouge(k, rect), 1.0 / 60.0));
                    k += 1;
                    if k % 8 == 0 {
                        ws_max = ws_max.max(memoire().0);
                    }
                    std::thread::sleep(Duration::from_millis(3));
                }
                let (o, f) = taille_dossier(dest);
                endu!("{titre} : {:.1} s, {:?}, {:.0} Mo, {f} fichiers ; mémoire de SHOT {ws0:.0} -> max {ws_max:.0} Mo (pic {:.0})", t.elapsed().as_secs_f64(), job.outcome.as_ref().map(|r| r.as_ref().map(|_| "ok").map_err(|e| e.clone())), o as f64 / 1e6, memoire_pic());
                resume_images(&format!("  interface pendant « {titre} »"), &v);
            }
            Err(e) => endu!("{titre} : refusé ({e})"),
        }
    };
    let ff = ffmpeg.clone().zip(enc.clone());
    paquet(&mut b, "liste des plans seule (CSV + JSON)", PaquetReglages { videos: false, vignettes: false, pdf: false, animatique: false, liste_csv: true, liste_json: true, ..Default::default() }, None, &sortie.join("paquet-liste"));
    paquet(&mut b, "vignettes par plan seules", PaquetReglages { videos: false, vignettes: true, pdf: false, animatique: false, liste_csv: false, liste_json: false, ..Default::default() }, None, &sortie.join("paquet-vignettes"));
    if ff.is_some() {
        paquet(&mut b, "vidéos par plan seules (H.264)", PaquetReglages { videos: true, vignettes: false, pdf: false, animatique: false, liste_csv: false, liste_json: false, ..Default::default() }, ff.clone(), &sortie.join("paquet-videos"));
        paquet(&mut b, "paquet de livraison complet", PaquetReglages::default(), ff.clone(), &sortie.join("paquet-complet"));
    }

    // Montage : EDL + FCPXML (+ les films par plan que le montage demande).
    if let Some((ff, enc)) = ff {
        use crate::editorial::{self, Fcpxml, Media, Options};
        use crate::editorialjob::EditorialJob;
        let media = if enc.h264.is_some() { Media::H264 } else { Media::ProRes };
        let o = Options { fcpxml: true, fcpxml_version: Fcpxml::V1_10, media, half: true, whole_film: false, ..Options::default() };
        let t = Instant::now();
        match editorial::build_plan(&b.app.doc, &o, 0, "Episode") {
            Ok(plan) => {
                let preparation = ms(t);
                let dir = sortie.join("montage");
                std::fs::create_dir_all(&dir).unwrap();
                match EditorialJob::start(plan, o, ff, enc, &dir) {
                    Ok(mut job) => {
                        let t = Instant::now();
                        let (ws0, _) = memoire();
                        let mut ws_max = ws0;
                        let mut v = Vec::new();
                        let mut k = 0usize;
                        let rect = b.app.canvas_rect;
                        while job.running() && t.elapsed().as_secs() < 3600 {
                            job.step(&b.app.doc, Duration::from_millis(8));
                            v.push(b.image(bouge(k, rect), 1.0 / 60.0));
                            k += 1;
                            if k % 8 == 0 {
                                ws_max = ws_max.max(memoire().0);
                            }
                            std::thread::sleep(Duration::from_millis(3));
                        }
                        let (o, f) = taille_dossier(&dir);
                        endu!("montage EDL/FCPXML : plan {preparation:.1} ms, export {:.1} s, {:.0} Mo, {f} fichiers ; mémoire de SHOT {ws0:.0} -> max {ws_max:.0} Mo (pic {:.0})", t.elapsed().as_secs_f64(), o as f64 / 1e6, memoire_pic());
                        resume_images("  interface pendant le montage", &v);
                    }
                    Err(e) => endu!("montage : démarrage refusé ({e})"),
                }
            }
            Err(e) => endu!("montage : {e}"),
        }
    }
    mem_ligne("fin des exports");
    endu!("processeur total du banc : {:.0} s", cpu() - cpu0);
    std::process::exit(0);
}

// ------------------------------------------------------------------ profil : d'où vient une image lente ?

/// Les sections les plus lourdes (le pire temps) des dernières images : d'où vient une pointe.
fn pires_sections(n: usize) -> String {
    let mut s = crate::perf::resume().sections;
    s.sort_by(|a, b| b.2.total_cmp(&a.2));
    s.iter().take(n).map(|(nom, moy, max)| format!("{nom} {moy:.1}/{max:.0}")).collect::<Vec<_>>().join(" | ")
}

/// `n` images, `avant` préparant chacune ; les images de plus de `seuil` ms disent où le temps est passé.
fn images_avec_pointes(b: &mut Banc, titre: &str, n: usize, seuil: f64, mut avant: impl FnMut(&mut Banc, usize) -> Vec<Event>) -> Vec<Mesure> {
    crate::perf::activer(true);
    let mut v = Vec::new();
    let mut dits = 0;
    for k in 0..n {
        let ev = avant(b, k);
        let m = b.image(ev, 1.0 / 60.0);
        if m.total() > seuil && dits < 12 {
            dits += 1;
            endu!("   pointe « {titre} » image {k} : {:.0} ms (interface {:.0}, découpage {:.0}, carte {:.0} dont textures {:.0}/préparation {:.0}/passe {:.0}/attente {:.0}) ; sections : {}", m.total(), m.construction, m.decoupage, m.carte, m.detail[0], m.detail[1], m.detail[2], m.detail[3], pires_sections(6));
        }
        v.push(m);
    }
    crate::perf::activer(false);
    resume_images(titre, &v);
    v
}

/// Où va la mémoire : les tuiles des dessins bitmap, celles des dessins vectoriels (leur rendu), ce que la carte graphique garde.
/// Où va la mémoire : les tuiles des dessins bitmap, celles des dessins vectoriels (leur rendu), ce que la carte graphique garde, les
/// caches. Ne lit aucun pixel (lire une tuile rangée la rouvrirait et fausserait la mesure).
fn repartition_memoire(b: &Banc, titre: &str) {
    let (mut bt, mut vt, mut bn, mut vn, mut elems, mut pts) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut ouvertes, mut en_tout, mut octets, mut octets_b, mut octets_v) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for c in b.app.doc.cels.values() {
        let (o, n) = c.raster.tuiles_ouvertes();
        ouvertes += o;
        en_tout += n;
        octets += c.octets_en_memoire();
        match &c.vector {
            Some(v) => {
                vt += n;
                vn += 1;
                octets_v += c.octets_en_memoire();
                elems += v.elements.len();
                for e in &v.elements {
                    if let crate::vector::Element::Stroke { points, .. } = e {
                        pts += points.len();
                    }
                }
            }
            None => {
                bt += n;
                bn += 1;
                octets_b += c.octets_en_memoire();
            }
        }
    }
    let g = b.app.gpu.as_ref().map(|g| g.stats()).unwrap_or_default();
    let (vignettes_gpu, _) = b.app.thumbs.octets();
    endu!(
        "répartition ({titre}) : {bn} dessins bitmap = {bt} tuiles, tiennent {:.0} Mo ; {vn} dessins vectoriels = {vt} tuiles de rendu, tiennent {:.0} Mo ({elems} éléments, {pts} points) ; au total {ouvertes} tuiles ouvertes sur {en_tout}, {:.0} Mo (nominal sans rangement : {:.0} Mo) ; tuiles rouvertes depuis le début : {} ; tenue : {} tuiles lâchées, {} dessins confiés au fil de fond",
        octets_b as f64 / 1e6,
        octets_v as f64 / 1e6,
        octets as f64 / 1e6,
        en_tout as f64 * 16.384 / 1e3,
        crate::raster::tuiles_rouvertes(),
        b.app.gel.lachees,
        b.app.gel.confies
    );
    endu!(
        "caches ({titre}) : traits {:.0} Mo, vignettes {:.0} Mo, carte : {} textures, {:.0} Mo",
        crate::vector::cache_total_bytes() as f64 / 1e6,
        vignettes_gpu as f64 / 1e6,
        g.textures,
        g.bytes as f64 / 1e6
    );
    let (ho, ht, hb) = b.app.history.tuiles();
    endu!("historique ({titre}) : {} étapes à annuler, {ho} tuiles ouvertes sur {ht}, tiennent {:.0} Mo", b.app.history.undo_len(), hb as f64 / 1e6);
}

/// Où va la mémoire après une séance de changements de plan, et ce qui la rend : le tas de Windows (compactage), la carte graphique (cache
/// ramené à 64 Mo), les caches de traits (budget à zéro). Court : ouvrir, parcourir 120 plans, mesurer, expérimenter.
#[test]
#[ignore]
fn endurance_memoire() {
    installer_crochet();
    let manifeste = manifeste_de(&dossier_endurance());
    assert!(manifeste.exists(), "pas d'épisode : lance d'abord endurance_generer");
    let mut b = ouvrir(&manifeste, Espace::Storyboard);
    let n = b.app.doc.panels.len();
    mem_ligne("épisode ouvert (Storyboard)");
    repartition_memoire(&b, "après l'ouverture");
    jusqu_au_calme(&mut b, 120.0);
    mem_ligne("après le calme");
    b.chauffer(5);
    let v = images_avec_pointes(&mut b, "storyboard : un changement de plan par image", 120, 1e9, |b, k| {
        b.app.select_panel((k * 7) % n);
        Vec::new()
    });
    let _ = v;
    jusqu_au_calme(&mut b, 60.0);
    mem_ligne("après les changements de plan");
    repartition_memoire(&b, "après les changements de plan");
    // Expérience : le tas de Windows garde-t-il de la mémoire rendue (fragmentation) ? On lui demande de se compacter, on remesure.
    {
        extern "system" {
            fn GetProcessHeap() -> isize;
            fn HeapCompact(heap: isize, flags: u32) -> usize;
        }
        std::thread::sleep(Duration::from_secs(3));
        mem_ligne("avant le compactage du tas");
        // SAFETY : deux appels Win32 sur le tas du processus ; HeapCompact ne touche aucun bloc vivant.
        let plus_grand = unsafe { HeapCompact(GetProcessHeap(), 0) };
        mem_ligne(&format!("après le compactage du tas (plus grand bloc libre {plus_grand})"));
    }
    // Expérience : la mémoire privée compte-t-elle la carte graphique ? On réduit son cache, on laisse passer des images, on remesure.
    if let Some(avant) = b.app.gpu.as_ref().map(|g| g.cache_limit()) {
        b.app.gpu.as_ref().map(|g| g.set_cache_bytes(64 << 20));
        b.chauffer(30);
        std::thread::sleep(Duration::from_secs(2));
        b.chauffer(10);
        mem_ligne("cache de la carte ramené à 64 Mo");
        repartition_memoire(&b, "cache de la carte ramené à 64 Mo");
        b.app.gpu.as_ref().map(|g| g.set_cache_bytes(avant));
    }


    // Expérience : les caches de traits à zéro.
    crate::vector::set_cache_budget(0);
    b.chauffer(5);
    mem_ligne("budget des caches de traits à zéro");
    repartition_memoire(&b, "budget des caches de traits à zéro");
    std::process::exit(0);
}
#[test]
#[ignore]
fn endurance_profil() {
    installer_crochet();
    let manifeste = manifeste_de(&dossier_endurance());
    assert!(manifeste.exists(), "pas d'épisode : lance d'abord endurance_generer");
    let mut b = ouvrir(&manifeste, Espace::Storyboard);
    let n = b.app.doc.panels.len();
    mem_ligne("épisode ouvert (Storyboard)");
    repartition_memoire(&b, "après l'ouverture");
    jusqu_au_calme(&mut b, 120.0);

    println!("--- Changer de plan dans le Storyboard");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(5);
    images_avec_pointes(&mut b, "storyboard : un changement de plan par image", 120, 60.0, |b, k| {
        b.app.select_panel((k * 7) % n);
        Vec::new()
    });
    jusqu_au_calme(&mut b, 60.0);
    mem_ligne("après les changements de plan");
    repartition_memoire(&b, "après les changements de plan");
    println!("--- Changer la durée d'un plan");
    b.app.select_panel(4);
    b.chauffer(5);
    let mut h = Hasard::new(3);
    images_avec_pointes(&mut b, "storyboard : durée d'un plan changée à chaque image", 40, 60.0, |b, k| {
        let i = if k % 2 == 0 { 4 } else { h.n(0, n - 1) };
        let nn = h.n(20, 200);
        b.app.select_panel(i);
        b.app.structure_op(|d, _| {
            d.set_duration(i, nn);
            Some(i)
        });
        Vec::new()
    });
    jusqu_au_calme(&mut b, 60.0);

    println!("--- Montage et animation");
    for e in [Espace::Montage, Espace::Animation] {
        b.app.set_espace(e);
        b.chauffer(5);
        images_avec_pointes(&mut b, &format!("{e:?} : un changement de plan par image"), 80, 60.0, |b, k| {
            b.app.select_panel((k * 11) % n);
            Vec::new()
        });
        jusqu_au_calme(&mut b, 60.0);
    }

    println!("--- Vignettes après un geste de structure");
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(5);
    jusqu_au_calme(&mut b, 60.0);
    images_avec_pointes(&mut b, "vignettes : un plan neuf au début toutes les 10 images", 60, 40.0, |b, k| {
        if k % 10 == 0 {
            b.app.select_panel(0);
            b.app.structure_op(|d, cur| Some(d.add_scene_after(cur)));
        }
        Vec::new()
    });
    mem_ligne("fin du profil");
    std::process::exit(0);
}

/// Combien le rangement des tuiles gagne sur les vrais dessins de l'épisode (`tuilepack.rs`) : taille et temps, une tuile sur `ENDU_PAS`.
#[test]
#[ignore]
fn endurance_tuiles() {
    let manifeste = manifeste_de(&dossier_endurance());
    assert!(manifeste.exists(), "pas d'épisode : lance d'abord endurance_generer");
    let l = crate::project::load(&manifeste).expect("ouverture");
    let pas = env_usize("ENDU_PAS", 4);
    for (nom, vecteur) in [("bitmap", false), ("vectoriel", true)] {
        let (mut n, mut ok, mut brut, mut rle, mut tp, mut td, mut vides) = (0usize, 0usize, 0usize, 0usize, 0f64, 0f64, 0usize);
        let mut k = 0usize;
        for c in l.doc.cels.values().filter(|c| c.vector.is_some() == vecteur) {
            for t in c.raster.tiles.iter().flatten() {
                k += 1;
                if k % pas != 0 {
                    continue;
                }
                n += 1;
                brut += 16384;
                if t.iter().all(|p| *p == egui::Color32::TRANSPARENT) {
                    vides += 1;
                }
                let t0 = Instant::now();
                let p = crate::tuilepack::pack(t);
                tp += t0.elapsed().as_secs_f64();
                if let Some(o) = p {
                    ok += 1;
                    rle += o.len();
                    let t1 = Instant::now();
                    let r = crate::tuilepack::unpack(&o);
                    td += t1.elapsed().as_secs_f64();
                    assert!(r[..] == t[..]);
                } else {
                    rle += 16384;
                }
            }
        }
        endu!("tuiles {nom} (1 sur {pas}) : {n} tuiles, {vides} transparentes, {ok} rangées ; {:.0} Mo -> {:.0} Mo (×{:.1}) ; rangement {:.1} µs/tuile, retour {:.1} µs/tuile", brut as f64 / 1e6, rle as f64 / 1e6, brut as f64 / rle.max(1) as f64, tp / n.max(1) as f64 * 1e6, td / ok.max(1) as f64 * 1e6);
    }
    std::process::exit(0);
}

// ------------------------------------------------------------------ fuites : une séance qui ne change rien au bout du compte

/// Des cycles qui laissent le projet comme il était (un plan neuf, un trait dedans, une clé de caméra, une copie, puis tout supprimé), pour
/// voir si la mémoire plafonne (l'historique garde 200 étapes) ou monte sans fin.
#[test]
#[ignore]
fn endurance_fuite() {
    installer_crochet();
    let manifeste = copier_episode("travail-fuite");
    let cycles = env_usize("ENDU_CYCLES", 400);
    let mut b = ouvrir(&manifeste, Espace::Storyboard);
    let n0 = b.app.doc.panels.len();
    let c0 = b.app.doc.cels.len();
    mem_ligne("épisode ouvert");
    let mut h = Hasard::new(21);
    let mut courbe = Vec::new();
    let (ws0, pv0) = memoire();
    let t0 = Instant::now();
    for cycle in 0..cycles {
        let tour = |b: &mut Banc, h: &mut Hasard| {
            b.app.select_panel(0);
            b.app.structure_op(|d, cur| Some(d.add_scene_after(cur)));
            let _ = operation(b, h, 8, 1, 1, &[Espace::Storyboard]);
            let _ = operation(b, h, 4, 1, 1, &[Espace::Storyboard]);
            b.app.structure_op_dessins(|d, _| Some(d.duplicate_scene_after(1)));
            b.chauffer(2);
            let ids: Vec<u64> = b.app.doc.panels.iter().skip(1).take(2).map(|p| p.id).collect();
            b.app.structure_op(|d, _| d.remove_panels(&ids));
            b.chauffer(2);
        };
        tour(&mut b, &mut h);
        // (Une fois l'historique plein, les étapes les plus vieilles — et leurs pixels — sont lâchées.)
        if cycle % 25 == 0 {
            let (ws, pv) = memoire();
            courbe.push(format!("cycle {cycle:>4} : {} plans, {} dessins, {} étapes, RSS {ws:.0} Mo, privée {pv:.0} Mo, {:.0} s", b.app.doc.panels.len(), b.app.doc.cels.len(), b.app.history.undo_len(), t0.elapsed().as_secs_f64()));
        }
    }
    for l in courbe {
        endu!("fuite {l}");
    }
    jusqu_au_calme(&mut b, 60.0);
    let (ws1, pv1) = memoire();
    endu!("fuite : {cycles} cycles, plans {n0} -> {}, dessins {c0} -> {} (les dessins des plans supprimés restent dans l'historique), mémoire {ws0:.0} -> {ws1:.0} Mo (travail), {pv0:.0} -> {pv1:.0} Mo (privée)", b.app.doc.panels.len(), b.app.doc.cels.len());
    // Vider l'historique : la mémoire des étapes oubliées revient-elle ?
    b.app.history.clear();
    b.chauffer(3);
    std::thread::sleep(Duration::from_secs(5));
    let (ws2, pv2) = memoire();
    endu!("fuite : historique vidé -> mémoire {ws2:.0} Mo (travail), {pv2:.0} Mo (privée) ; dessins {}", b.app.doc.cels.len());
    std::process::exit(0);
}
// ------------------------------------------------------------------ essais de la suite (rapides)

/// L'écriture et la copie de secours passent après l'interface : sans cela, huit fils d'encodage prennent les cœurs de l'interface
/// (mesuré sur un épisode de 200 plans : 40 ms par image au lieu de 8 à 18 pendant la copie de secours, pointes de 200 ms).
#[test]
fn the_save_and_the_recovery_copy_run_below_the_interface_priority() {
    use crate::enregistrement_cliche_tests::projet_riche;
    use crate::fiabilite_fixture::dossier;
    use crate::parallele::{PRIORITE_ECRITURE, PRIORITE_SECOURS};
    use crate::recovery::{Recovery, Snapshot};
    let travail = dossier("endurance-priorite");
    let doc = projet_riche(&travail);
    // L'enregistrement de l'application.
    let manifeste = travail.join("Film").join("Film.shot");
    std::fs::create_dir_all(manifeste.parent().unwrap()).unwrap();
    let mut b = crate::perf_tests::banc(doc, false);
    b.app.modified = true;
    assert!(b.app.save_to_blocking(manifeste.clone()), "{}", b.app.status);
    assert_eq!(PRIORITE_ECRITURE.load(Ordering::Relaxed), -1, "le fil d'écriture est sous la priorité normale (THREAD_PRIORITY_BELOW_NORMAL)");
    // La copie de secours.
    let racine = dossier("endurance-priorite-secours");
    let aucun = std::collections::HashMap::new();
    let snap = |serial| Snapshot { doc: &b.app.doc, serial, modified: true, title: "Film".into(), project: None, project_written: &aucun };
    let mut rec = Recovery::start(&racine).expect("séance");
    rec.start_copy(snap(1));
    let t = Instant::now();
    while !rec.tick(snap(1), None, false) {
        assert!(rec.error.is_none(), "{:?}", rec.error);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert_eq!(PRIORITE_SECOURS.load(Ordering::Relaxed), -1, "le fil de la copie de secours est sous la priorité normale");
    rec.close();
    let _ = std::fs::remove_dir_all(&racine);
    let _ = std::fs::remove_dir_all(&travail);
}

/// Le générateur d'épisode : des plans plus longs que ceux du film lourd, trois pistes de son, un projet qui s'enregistre et se relit.
#[test]
fn the_episode_generator_makes_a_project_that_saves_and_reloads() {
    use crate::fiabilite_fixture::dossier;
    crate::brushtex::BrushLibrary::builtin().share();
    let (court, _) = episode(9, 100, 5);
    let (doc, st) = episode(9, 250, 5);
    assert_eq!(doc.panels.len(), 9);
    assert!(doc.total_frames() > court.total_frames() * 2, "les plans sont allongés : {} contre {}", doc.total_frames(), court.total_frames());
    assert_eq!(doc.audio_tracks.len(), 3, "musique, dialogues, bruitages");
    assert!(doc.audio_tracks.iter().all(|t| !t.clips.is_empty()), "chaque piste a des extraits");
    assert!(st.dessins_bitmap > 0 && st.dessins_vecteur > 0, "des dessins bitmap et vectoriels");
    let travail = dossier("endurance-episode");
    let manifeste = travail.join("Episode").join("Episode.shot");
    std::fs::create_dir_all(manifeste.parent().unwrap()).unwrap();
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("relu");
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert_eq!(relu.doc.panels.len(), doc.panels.len());
    assert_eq!(relu.doc.total_frames(), doc.total_frames());
    assert_eq!(relu.doc.audio_tracks.iter().map(|t| t.clips.len()).collect::<Vec<_>>(), doc.audio_tracks.iter().map(|t| t.clips.len()).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&travail);
}
