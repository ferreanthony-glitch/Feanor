//! Le démarrage : ce qui s'exécute avant la première image ne doit pas dépendre de ce qu'Anthony a
//! sur son disque (des centaines de pinceaux importés, un projet sur un disque réseau éteint).
//!
//! Le 2026-10-19 la fenêtre est restée noire plus de 14 s au lancement, sur une machine chargée par des
//! compilations. La fenêtre d'eframe n'apparaît qu'une fois la première image peinte : tout le
//! constructeur de l'application (`App::new`) et ce que le lancement demande se paient avant. Mesuré :
//! la lecture de la bibliothèque de pinceaux importés y tenait seule 90 % du temps (5 s de calcul pour
//! 300 pointes, que huit fils ramènent à 0,4 s sur une machine au repos — et à plusieurs secondes quand
//! les cœurs sont pris). Elle se fait maintenant sur un autre fil (`App::poll_brushes`), et la présence
//! du dernier projet se vérifie aussi sur un autre fil (`App::reopen_last_project`).

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Rect, RawInput, ViewportId};

use crate::app::App;
use crate::brushtex::{BrushLibrary, GrayImage};
use crate::fiabilite_fixture::{dossier, projet_enregistre};

/// Une bibliothèque de pinceaux importés lourde, rangée dans le dossier de réglages de l'essai : `tips`
/// pointes et `papers` papiers de 256 × 256.
fn bibliotheque_lourde(tips: usize, papers: usize) {
    let mut lib = BrushLibrary::builtin();
    let mut graine = 12345u64;
    let mut image = |n: usize| {
        GrayImage::new(
            n,
            n,
            (0..n * n)
                .map(|_| {
                    graine = graine.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((graine >> 40) & 0xff) as f32 / 255.0
                })
                .collect(),
        )
    };
    for k in 0..tips {
        lib.add_tip(&format!("Pointe importée {k}"), image(256));
    }
    for k in 0..papers {
        lib.add_paper(&format!("Papier importé {k}"), image(256));
    }
    let dir = BrushLibrary::user_dir().expect("dossier des pinceaux (bac à sable)");
    lib.save(&dir).expect("enregistrement de la bibliothèque d'essai");
}

/// Un dossier de réglages neuf pour l'essai (propre à ce fil).
fn bac(nom: &str) {
    crate::donnees::bac_a_sable(&dossier(nom));
}

fn entree() -> RawInput {
    let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0))), time: Some(0.0), ..RawInput::default() };
    input.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
    input
}

/// Images jusqu'à ce que `fini` soit vrai (au plus `limite`).
fn jusqu_a(ctx: &egui::Context, app: &mut App, limite: Duration, fini: impl Fn(&App) -> bool) -> bool {
    let t = Instant::now();
    let mut temps = 0.0;
    while t.elapsed() < limite {
        if fini(app) {
            return true;
        }
        temps += 1.0 / 60.0;
        let mut input = entree();
        input.time = Some(temps);
        let _ = ctx.run(input, |ctx| app.frame(ctx));
        std::thread::sleep(Duration::from_millis(5));
    }
    fini(app)
}

#[test]
fn the_first_frame_does_not_wait_for_the_imported_brushes() {
    bac("demarrage-pinceaux");
    bibliotheque_lourde(60, 20);
    // Ce que coûte leur lecture sur le fil de l'interface (c'était le prix du lancement).
    let t = Instant::now();
    let lue = BrushLibrary::load(BrushLibrary::user_dir().as_deref());
    let lecture_ms = t.elapsed().as_secs_f64() * 1e3;
    let importes = lue.tips().len();
    assert!(importes >= 60 + 5, "la bibliothèque d'essai a ses pointes ({importes})");
    drop(lue);

    let lectures_avant = crate::brushtex::loads_on_this_thread();
    let ctx = egui::Context::default();
    let t0 = Instant::now();
    let mut app = App::sans_fenetre(&ctx, None);
    let constructeur_ms = t0.elapsed().as_secs_f64() * 1e3;
    let t1 = Instant::now();
    let out = ctx.run(entree(), |ctx| app.frame(ctx));
    let premiere_ms = t1.elapsed().as_secs_f64() * 1e3;
    println!(
        "FIABILITE démarrage : lecture des pinceaux importés {lecture_ms:.0} ms ; constructeur {constructeur_ms:.0} ms + première image {premiere_ms:.0} ms = {:.0} ms jusqu'à l'interface (avant : {:.0} ms)\n{}",
        constructeur_ms + premiere_ms,
        constructeur_ms + premiere_ms + lecture_ms,
        crate::demarrage::rapport()
    );
    // La première image montre de l'interface (l'écran d'accueil : aucun projet n'est ouvert).
    assert!(!out.shapes.is_empty(), "la première image doit produire de l'interface");
    assert!(app.welcome_shown(), "sans projet, la première image est l'écran d'accueil");
    // Structurel : ni le constructeur ni la première image n'ont lu la bibliothèque sur ce fil.
    assert_eq!(crate::brushtex::loads_on_this_thread(), lectures_avant, "la bibliothèque de pinceaux importés ne se lit pas sur le fil de l'interface");
    // Les pinceaux de SHOT sont là tout de suite ; les importés arrivent ensuite.
    assert!(app.brushes_job.is_some() || app.brushes.tips().len() >= importes, "les importés sont en route ou déjà là");
    assert!(app.brushes.presets.iter().any(|p| p.name == "Pinceau rond"), "les pinceaux de SHOT sont disponibles dès la première image");
    // Indicatif : le lancement ne doit plus coûter la lecture (signalé, pas échoué : machine chargée).
    if constructeur_ms + premiere_ms > lecture_ms * 0.8 && lecture_ms > 150.0 {
        println!("ATTENTION le lancement ({:.0} ms) coûte encore presque autant que la lecture des pinceaux ({lecture_ms:.0} ms)", constructeur_ms + premiere_ms);
    }
    assert!(jusqu_a(&ctx, &mut app, Duration::from_secs(60), |a| a.brushes_job.is_none()), "les pinceaux importés finissent par arriver");
    assert!(app.brushes.tips().len() >= importes, "tous les pointes importées sont là ({} sur {importes})", app.brushes.tips().len());
}

/// Tant que les importés ne sont pas arrivés, rien ne modifie ni n'enregistre la bibliothèque : la
/// lecture écraserait le changement, ou l'enregistrement effacerait les pinceaux pas encore arrivés.
#[test]
fn nothing_changes_the_brush_library_before_the_imported_brushes_arrive() {
    bac("demarrage-garde");
    bibliotheque_lourde(40, 10);
    let fichier = BrushLibrary::user_dir().expect("dossier").join("pinceaux.json");
    let avant = std::fs::read(&fichier).expect("bibliothèque d'essai");
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    // Le fil de lecture est parti ; on retient son résultat en occupant le test juste ce qu'il faut :
    // tant que `brushes_job` existe, les commandes de la bibliothèque refusent.
    if app.brushes_job.is_some() {
        app.start_save_preset(crate::brushtex::Target::Brush);
        assert!(app.preset_edit.is_none(), "pas d'enregistrement de réglage avant l'arrivée des importés");
        assert!(app.status.contains("se chargent encore"), "{}", app.status);
        app.apply_preset_menu(0, crate::brushui::PresetMenu::Delete, crate::brushtex::Target::Brush, app.brush);
        assert!(app.brushes.presets.iter().any(|p| p.name == "Pinceau rond"), "rien n'est supprimé");
    }
    assert!(jusqu_a(&ctx, &mut app, Duration::from_secs(60), |a| a.brushes_job.is_none()));
    assert_eq!(std::fs::read(&fichier).expect("bibliothèque"), avant, "la bibliothèque enregistrée n'a pas été touchée");
    // Une fois arrivés, la commande marche de nouveau.
    app.start_save_preset(crate::brushtex::Target::Brush);
    assert!(app.preset_edit.is_some());
}

/// Le dernier projet : sa présence est vérifiée sur un autre fil ; absent, on ne dit rien et SHOT
/// reste sur l'écran d'accueil ; présent, il s'ouvre.
#[test]
fn the_last_project_is_probed_off_the_interface_thread() {
    bac("demarrage-dernier");
    let (_f, manifeste) = projet_enregistre("demarrage-projet");
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.settings.reopen_last = true;
    // Absent : rien ne s'ouvre, aucun message d'erreur.
    app.settings.recent_projects = vec![manifeste.with_file_name("Disparu.pelure")];
    let message = app.status.clone();
    app.reopen_last_project();
    assert!(app.reopen_probe.is_some() && app.open_job.is_none(), "la question part sur un autre fil, rien n'est lu sur celui-ci");
    assert!(jusqu_a(&ctx, &mut app, Duration::from_secs(30), |a| a.reopen_probe.is_none()));
    assert!(app.open_job.is_none() && app.path.is_none() && app.welcome_shown());
    assert_eq!(app.status, message, "un dernier projet disparu ne fait pas de bruit");
    // Présent : il s'ouvre.
    app.settings.recent_projects = vec![manifeste.clone()];
    app.reopen_last_project();
    assert!(jusqu_a(&ctx, &mut app, Duration::from_secs(60), |a| a.path.as_deref() == Some(manifeste.as_path())), "le dernier projet s'ouvre : {}", app.status);
    // Déjà occupé à autre chose (un projet modifié) : on ne lui prend pas la main.
    let mut autre = App::sans_fenetre(&ctx, None);
    autre.settings.reopen_last = true;
    autre.settings.recent_projects = vec![manifeste.clone()];
    autre.reopen_last_project();
    autre.modified = true;
    assert!(jusqu_a(&ctx, &mut autre, Duration::from_secs(30), |a| a.reopen_probe.is_none()));
    assert!(autre.path.is_none() && autre.open_job.is_none(), "un projet modifié entre-temps n'est pas remplacé");
}

/// Le lancement pendant que la machine est chargée : un calcul par cœur en parallèle, comme des
/// compilations. `cargo test --release --offline --target-dir target\msvc perf_startup -- --ignored --nocapture`.
#[test]
#[ignore]
fn perf_startup_under_load() {
    bac("demarrage-charge");
    bibliotheque_lourde(160, 60);
    let coeurs = std::thread::available_parallelism().map_or(8, |n| n.get());
    for charge in [0usize, coeurs, coeurs * 2] {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let occupes: Vec<_> = (0..charge)
            .map(|_| {
                let stop = stop.clone();
                std::thread::spawn(move || {
                    let mut x = 1u64;
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        for _ in 0..10_000 {
                            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                        }
                        std::hint::black_box(x);
                    }
                })
            })
            .collect();
        std::thread::sleep(Duration::from_millis(200));
        let t = Instant::now();
        let lue = BrushLibrary::load(BrushLibrary::user_dir().as_deref());
        let lecture = t.elapsed().as_secs_f64() * 1e3;
        drop(lue);
        let ctx = egui::Context::default();
        let t = Instant::now();
        let mut app = App::sans_fenetre(&ctx, None);
        let constructeur = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let _ = ctx.run(entree(), |ctx| app.frame(ctx));
        let premiere = t.elapsed().as_secs_f64() * 1e3;
        println!("PERF démarrage, {charge} calculs en parallèle : constructeur {constructeur:.0} ms + première image {premiere:.0} ms = {:.0} ms ; avant, avec la lecture des pinceaux sur le fil de l'interface : {:.0} ms", constructeur + premiere, constructeur + premiere + lecture);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        for t in occupes {
            let _ = t.join();
        }
    }
}
