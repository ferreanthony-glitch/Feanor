//! Essais des exports V1 : les calques séparés, le menu Exporter regroupé par usage, les fenêtres et les réglages retenus.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::blend::BlendMode;
use crate::document::{Document, Layer, LayerKind};
use crate::export::{self, Format};
use crate::exports_v1_tests::{court, dossier_neuf, projet_uni};
use crate::exportlayers::{self, LayersSettings};
use crate::exportstills::StillFormat;
use crate::exportvideo::{BurnSettings, VideoRate, VideoSize};
use crate::fxcache::monde;
use crate::pdfmise::Paper;
use crate::raster::unpremultiply;
use crate::render;

// ------------------------------------------------------------------------------------ calques séparés

#[test]
fn only_visible_plain_layers_are_separated() {
    let mut doc = crate::export_tests::doc_with_drawing(1, 3);
    let mut dossier = Layer::new("Dossier", LayerKind::Bitmap);
    dossier.folder = true;
    let mut cache = Layer::new("Caché", LayerKind::Bitmap);
    cache.visible = false;
    cache.frames = doc.panels[0].layers[0].frames.clone();
    let mut note = Layer::new("Note", LayerKind::Bitmap);
    note.annotation = true;
    note.frames = doc.panels[0].layers[0].frames.clone();
    doc.panels[0].layers.extend([dossier, cache, note]);
    let p = &doc.panels[0];
    assert_eq!(render::separable_layers(p, false), [0, 1], "ni dossier, ni calque caché, ni annotation");
    assert_eq!(render::separable_layers(p, true), [0, 1, 4], "l'annotation vient avec « Avec les annotations »");
    let info = exportlayers::plans_info(&doc, &[0], false);
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].folder, "SQ01_PL001");
    assert_eq!(info[0].layers.iter().map(|l| l.1.as_str()).collect::<Vec<_>>(), ["01_C0", "02_C1"]);
    assert_eq!(exportlayers::file_count(&info, &[(0, 0), (0, 1), (0, 2)]), 6);
    assert_eq!(exportlayers::effect_layers(&doc, &[0]), 0);
}

#[test]
fn separate_layers_recompose_the_mixed_image_pixel_for_pixel() {
    monde::lire();
    let doc = crate::export_tests::doc_with_drawing(2, 3);
    for (p, f) in [(0, 0), (1, 2)] {
        // Avec la caméra : les images des calques, posées l'une sur l'autre, redonnent l'image du film.
        let cam = doc.panel_camera(p, f);
        let plein = render::camera_image(&doc, &render::panel_plan_at(&doc, p, f, None), &cam);
        let calques = render::panel_layer_plans(&doc, p, f, false);
        assert_eq!(calques.len(), 2, "deux calques");
        let [w, _] = doc.frame;
        let mut somme = vec![Color32::TRANSPARENT; plein.len()];
        for (_, plan) in &calques {
            assert!(plan.paper.is_none(), "fond transparent");
            for (i, (d, s)) in somme.iter_mut().zip(render::camera_image(&doc, plan, &cam)).enumerate() {
                if s.a() != 0 {
                    *d = crate::blend::blend_over(*d, s, 255, None, BlendMode::Normal, (i % w, i / w));
                }
            }
        }
        assert!(somme == plein, "image {p}/{f} : les calques ne redonnent pas l'image");
        assert!(plein.iter().any(|c| c.a() != 0), "l'image n'est pas vide");
        // Sans caméra : la feuille entière, de même.
        let feuille = render::composite_full(&doc, &render::panel_plan_at(&doc, p, f, None));
        let mut somme = vec![Color32::TRANSPARENT; feuille.len()];
        let sw = doc.width;
        for (_, plan) in &calques {
            for (i, (d, s)) in somme.iter_mut().zip(render::composite_full(&doc, plan)).enumerate() {
                if s.a() != 0 {
                    *d = crate::blend::blend_over(*d, s, 255, None, BlendMode::Normal, (i % sw, i / sw));
                }
            }
        }
        assert!(somme == feuille, "feuille entière {p}/{f}");
    }
}

/// Tous les fichiers sous `dir`.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.path().is_dir() {
            out.extend(walk(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

#[test]
fn the_layer_export_writes_a_folder_per_plan_and_a_transparent_png_sequence_per_layer() {
    monde::lire();
    let mut doc = crate::export_tests::doc_with_drawing(2, 3);
    // Le calque C1 du premier plan n'a rien à la deuxième image : une image transparente garde la suite continue.
    doc.panels[0].layers[1].frames[1] = None;
    let tous = export::frames_for(&doc, export::Range::Montage, 0);
    assert_eq!(tous.len(), 6);
    let info = exportlayers::plans_info(&doc, &[0, 1], false);
    for avec_camera in [true, false] {
        let dir = dossier_neuf(&format!("calques-{avec_camera}"));
        let s = LayersSettings { with_camera: avec_camera };
        let mut job = exportlayers::job(&dir, info.clone(), tous.clone(), &s, false);
        court(&mut job, &doc);
        assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
        let mut dossiers: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        dossiers.sort();
        // (Les deux cases de ce projet d'essai sont dans la même scène : le deuxième dossier prend « _2 ».)
        assert_eq!(dossiers, ["SQ01_PL001", "SQ01_PL001_2"], "un dossier par plan");
        let (w, h) = if avec_camera { (doc.frame[0], doc.frame[1]) } else { (doc.width, doc.height) };
        for d in &dossiers {
            let mut noms: Vec<String> = std::fs::read_dir(dir.join(d)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            noms.sort();
            let attendus: Vec<String> = ["01_C0", "02_C1"].iter().flat_map(|c| (1..=3).map(move |f| format!("{c}_{f:04}.png"))).collect();
            assert_eq!(noms, attendus, "une suite par calque, continue");
        }
        // Chaque image est exactement celle que l'export calcule (en couleurs droites), à la bonne taille.
        for (p, f) in [(0usize, 0usize), (0, 2), (1, 1)] {
            let calques = render::panel_layer_plans(&doc, p, f, false);
            for (rang, (li, plan)) in calques.iter().enumerate() {
                let pixels = if avec_camera { render::camera_image(&doc, plan, &doc.panel_camera(p, f)) } else { render::composite_full(&doc, plan) };
                let lu = image::open(dir.join(&dossiers[p]).join(format!("{:02}_C{}_{:04}.png", rang + 1, li, f + 1))).expect("PNG").to_rgba8();
                assert_eq!(lu.dimensions(), (w as u32, h as u32));
                let attendu: Vec<u8> = pixels.iter().flat_map(|c| unpremultiply(*c)).collect();
                assert!(lu.as_raw() == &attendu, "plan {p} image {f} calque {li}");
            }
        }
        // L'image sans rien est transparente.
        let vide = image::open(dir.join(&dossiers[0]).join("02_C1_0002.png")).unwrap().to_rgba8();
        assert!(vide.pixels().all(|p| p.0[3] == 0));
        assert!(walk(&dir).iter().all(|e| !e.to_string_lossy().ends_with(".part")));
        std::fs::remove_dir_all(&dir).ok();
    }
    // Annuler : rien ne reste.
    let dir = dossier_neuf("calques-annule");
    let longue = crate::export_tests::doc_with_drawing(1, 30);
    let frames = export::frames_for(&longue, export::Range::Montage, 0);
    let info = exportlayers::plans_info(&longue, &[0], false);
    let mut job = exportlayers::job(&dir, info, frames, &LayersSettings::default(), false);
    let t = Instant::now();
    while job.outcome.is_none() && !matches!(job.progress(), export::Progress::Running(n, _) if n >= 2) {
        job.step(&longue, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(1));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    job.cancel();
    assert!(matches!(job.outcome, Some(Err(_))));
    let restes = walk(&dir);
    assert!(restes.is_empty(), "rien ne reste après l'annulation : {restes:?}");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------- le menu et les fenêtres

fn app_de(doc: Document, nom: &str) -> (egui::Context, App, PathBuf) {
    let dir = dossier_neuf(nom);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    (ctx, app, dir)
}

fn image(ctx: &egui::Context, app: &mut App) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))), time: Some(0.0), ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        app.export_hub_window(ctx);
        app.export_windows(ctx);
    });
}

#[test]
fn the_export_hub_is_grouped_by_use_with_an_action_for_every_entry() {
    let familles = crate::prefsui::FAMILLES;
    let titres: Vec<&str> = familles.iter().map(|f| f.0).collect();
    assert_eq!(titres, ["Vidéo", "Images & PDF", "Son", "Montage & échange"]);
    let noms = |k: usize| familles[k].1.iter().map(|e| e.1).collect::<Vec<_>>();
    assert_eq!(noms(0), ["Film", "GIF animé"]);
    assert_eq!(noms(1), ["Images", "Image courante", "Images par plan", "Storyboard PDF", "Planche des vignettes", "Color script", "Relecture", "Journal des différences"]);
    assert_eq!(noms(2), ["Son mixé", "Son par piste"]);
    assert_eq!(noms(3), ["Pour le montage", "Liste des plans", "Kitsu", "Paquet de livraison", "Rapport de numérotation", "Noms et production", "Fiche technique", "Calques séparés", "Caméra"]);
    let mut actions = Vec::new();
    for (_, exports) in familles {
        for (icone, nom, phrase, action) in exports {
            assert!(!icone.is_empty() && !phrase.is_empty() && phrase.ends_with('.'), "{nom} : une icône et une phrase");
            assert!(crate::keymap::action_info(action).is_some_and(|a| a.implemented), "{nom} : l'action {action} existe");
            assert!(!nom.to_lowercase().contains("html"), "l'ancienne page HTML est retirée");
            for banni in ["Storyboard Pro", "Toon Boom", "Premiere", "DaVinci", "Avid", "After Effects"] {
                assert!(!nom.contains(banni) && !phrase.contains(banni), "{nom} cite {banni}");
            }
            actions.push(*action);
        }
    }
    assert_eq!(actions.len(), 21);
    let n = actions.len();
    actions.sort_unstable();
    actions.dedup();
    assert_eq!(actions.len(), n, "une action par entrée");
    // Tout est dans le fichier de raccourcis, sans touche (la palette Ctrl+K liste les actions connues).
    let map = crate::keymap_tests::default_map();
    for a in &actions {
        assert!(map.bindings.iter().any(|b| b.action == *a), "{a} : un bloc dans keymap.toml");
    }
    for a in ["fichier.exporter_images_par_plan", "fichier.exporter_son_par_piste", "fichier.exporter_calques", "fichier.exporter_camera"] {
        assert!(map.bindings.iter().filter(|b| b.action == a).all(|b| b.keys.is_empty()), "{a} : sans touche");
    }
}

#[test]
fn every_export_action_opens_its_window_and_the_windows_draw() {
    monde::lire();
    let (ctx, mut app, dir) = app_de(projet_uni(3, 12, Color32::from_rgb(90, 120, 200)), "fenetres");
    let attendus = [
        ("fichier.exporter_film", Format::Mp4),
        ("fichier.exporter_gif", Format::Gif),
        ("fichier.exporter_images", Format::Png),
        ("fichier.exporter_images_par_plan", Format::Stills),
        ("fichier.exporter_pdf", Format::Pdf),
        ("fichier.exporter_son", Format::Wav),
        ("fichier.exporter_son_par_piste", Format::Stems),
        ("fichier.exporter_calques", Format::Layers),
        ("fichier.exporter_camera", Format::Camera),
    ];
    for (action, format) in attendus {
        app.export_dialog = None;
        app.run_action(action);
        assert_eq!(app.export_dialog.as_ref().map(|s| s.format), Some(format), "{action}");
        for _ in 0..2 {
            image(&ctx, &mut app);
        }
        assert!(app.export_dialog.is_some(), "{action} : la fenêtre reste ouverte");
    }
    // Les formats du film, ses réglages et ceux des autres fenêtres dessinent sans plantage.
    for f in [Format::ProRes, Format::ProRes4444, Format::DnxHr] {
        let mut s = app.export_last.clone();
        s.format = f;
        s.burn.enabled = true;
        s.size = VideoSize::Width(640);
        s.rate = VideoRate::F2997;
        app.export_dialog = Some(s);
        image(&ctx, &mut app);
    }
    // Le hub dessine aussi, et l'action « Exporter » l'ouvre.
    app.run_action("fichier.exporter");
    image(&ctx, &mut app);
    assert!(app.export_hub);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_choices_are_kept_in_the_application_settings() {
    monde::lire();
    let (ctx, mut app, dir) = app_de(projet_uni(2, 6, Color32::WHITE), "prefs");
    app.settings.exports.size = VideoSize::Hd720;
    app.settings.exports.rate = VideoRate::F25;
    app.settings.exports.burn = BurnSettings { enabled: true, plan: false, timecode: true, sequence: false, texts: true };
    app.settings.exports.pdf.per_page = 3;
    app.settings.exports.pdf.paper = Paper::A3;
    app.settings.exports.stills.format = StillFormat::Jpeg;
    app.run_action("fichier.exporter_film");
    image(&ctx, &mut app);
    let s = app.export_dialog.clone().unwrap();
    assert_eq!((s.size, s.rate, s.burn.texts, s.pdf.per_page, s.pdf.paper, s.stills.format), (VideoSize::Hd720, VideoRate::F25, true, 3, Paper::A3, StillFormat::Jpeg));
    // Écrits dans reglages.toml puis relus (un nombre libre de pixels compris).
    let mut prefs = export::ExportPrefs::of(&s);
    prefs.size = VideoSize::Width(1234);
    let texte = toml::to_string(&crate::settings::Settings { exports: prefs.clone(), ..Default::default() }).expect("écriture des réglages");
    let relus: crate::settings::Settings = toml::from_str(&texte).expect("relecture des réglages");
    assert_eq!(relus.exports, prefs);
    // Un fichier de réglages d'avant : les choix d'export reprennent leurs valeurs d'origine.
    let ancien: crate::settings::Settings = toml::from_str("copie_secours_minutes = 3\n").unwrap();
    assert_eq!(ancien.exports, export::ExportPrefs::default());
    std::fs::remove_dir_all(&dir).ok();
}
