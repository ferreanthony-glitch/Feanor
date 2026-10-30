//! La fusion en tâche de fond : l'interface n'est jamais bloquée plus d'une image, « Arrêter » ne change rien, une modification
//! entre-temps abandonne la fusion, une seule étape d'annulation, et les fusions rapides restent immédiates.

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};
use crate::render;

const RED: Color32 = Color32::from_rgb(200, 0, 0);
const BLUE: Color32 = Color32::from_rgb(0, 0, 200);

/// Une image d'écran.
const IMAGE: Duration = Duration::from_millis(16);

fn paint(doc: &mut Document, cel: u64, x: usize, y: usize, c: Color32) {
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    for dy in 0..10 {
        for dx in 0..10 {
            let idx = r.tile_index_of(x + dx, y + dy);
            r.tile_mut(idx)[((y + dy) % 64) * 64 + (x + dx) % 64] = c;
        }
    }
}

/// Une animation de `n` images : un dessin différent à chaque image sur chacun des deux calques, un flou sur celui du dessus.
fn animation(name: &str, n: usize, flou: bool) -> App {
    let mut doc = Document::new(320, 180, 24);
    doc.panels[0].set_length(n);
    let (mut bas, mut haut) = (Layer::new("Bas", LayerKind::Bitmap), Layer::new("Haut", LayerKind::Bitmap));
    for f in 0..n {
        let (b, h) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
        paint(&mut doc, b, (10 + f * 5) % 300, 20, RED);
        paint(&mut doc, h, (20 + f * 4) % 300, 60, BLUE);
        bas.frames.push(Some(b));
        haut.frames.push(Some(h));
    }
    if flou {
        haut.fx.push(Effect::new(EffectKind::GaussianBlur));
    }
    doc.panels[0].layers = vec![bas, haut];
    let dir = std::env::temp_dir().join(format!("shot-fusiontache-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 1;
    app.cur_frame = 0;
    app
}

fn images(app: &App) -> Vec<Vec<Color32>> {
    (0..app.doc.panels[0].frame_count()).map(|f| render::composite_full(&app.doc, &render::panel_plan_at(&app.doc, 0, f, None))).collect()
}

/// Regarde la tâche comme le fait l'interface (une fois par image) jusqu'à sa fin ; rend le plus long regard pendant le calcul.
fn regarder(app: &mut App) -> Duration {
    let ctx = egui::Context::default();
    let mut pire = Duration::ZERO;
    let debut = Instant::now();
    while app.fusion.is_some() {
        let t = Instant::now();
        app.fusion_tick(&ctx);
        if app.fusion.is_some() {
            pire = pire.max(t.elapsed());
        }
        assert!(debut.elapsed() < Duration::from_secs(300), "la fusion ne finit pas");
        std::thread::sleep(Duration::from_millis(4));
    }
    pire
}

#[test]
fn a_long_animation_with_an_effect_merges_in_the_background_without_blocking_the_interface() {
    let mut app = animation("longue", 14, true);
    let avant = images(&app);
    let layers = app.layers().to_vec();
    let t = Instant::now();
    app.run_action("calque.fusionner_dessous");
    let lancement = t.elapsed();
    assert!(app.fusion.is_some(), "une longue animation avec un flou part en tâche de fond : {}", app.status);
    assert_eq!(app.layers().len(), 2, "rien n'est posé tant que ça calcule");
    assert!(lancement < IMAGE * 2, "le départ rend la main en {lancement:?}");
    // La barre d'état se peint pendant le calcul (progression, « Arrêter »).
    let ctx = egui::Context::default();
    let mut peint = Duration::ZERO;
    let mut images_peintes = 0;
    while app.fusion.as_ref().is_some_and(|t| !t.est_finie()) && images_peintes < 5 {
        let t = Instant::now();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.fusion_status_ui(ui));
        });
        peint = peint.max(t.elapsed());
        images_peintes += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(peint < IMAGE * 2, "la barre d'état se peint en {peint:?}");
    let pire = regarder(&mut app);
    assert!(pire < IMAGE, "le fil de l'interface n'est jamais bloqué au-delà d'une image : {pire:?}");
    assert_eq!(app.layers().len(), 1, "{}", app.status);
    assert!(app.status.contains("fusionnés"), "{}", app.status);
    assert_eq!(images(&app), avant, "même rendu que les deux calques, flou compris");
    // Une seule étape d'annulation.
    app.undo();
    assert_eq!(app.layers(), &layers[..]);
    assert_eq!(images(&app), avant);
}

#[test]
fn stopping_changes_nothing() {
    let mut app = animation("arret", 14, true);
    let layers = app.layers().to_vec();
    let cels = app.doc.cels.len();
    app.run_action("calque.fusionner_dessous");
    assert!(app.fusion.is_some());
    app.arreter_fusion();
    regarder(&mut app);
    assert_eq!(app.layers(), &layers[..], "rien n'a changé");
    assert_eq!(app.doc.cels.len(), cels, "aucun dessin de plus");
    assert!(app.status.contains("arrêtée") && app.status.contains("rien n'a changé"), "{}", app.status);
    assert_eq!(app.cur_layer, 1);
    // Ctrl+Z n'a rien à défaire de la fusion.
    let avant = app.layers().to_vec();
    app.undo();
    assert_eq!(app.layers().len(), avant.len());
}

#[test]
fn a_change_meanwhile_abandons_the_merge_with_a_message() {
    // Un dessin retouché sur un des calques.
    let mut app = animation("modif", 14, true);
    app.run_action("calque.fusionner_dessous");
    assert!(app.fusion.is_some());
    let cel = app.layers()[0].frames[0].unwrap();
    app.doc.cels.get_mut(&cel).unwrap().version += 1;
    app.fusion_attendre();
    assert_eq!(app.layers().len(), 2, "rien n'est écrasé");
    assert!(app.status.contains("abandonnée") && app.status.contains("Rien n'a été écrasé"), "{}", app.status);
    // Un calque supprimé.
    let mut app = animation("suppression", 14, true);
    app.run_action("calque.fusionner_dessous");
    assert!(app.fusion.is_some());
    app.layers_mut().remove(1);
    app.fusion_attendre();
    assert_eq!(app.layers().len(), 1);
    assert!(app.status.contains("abandonnée"), "{}", app.status);
    // Un réglage du calque (ici l'opacité) changé.
    let mut app = animation("reglage", 14, true);
    app.run_action("calque.fusionner_dessous");
    assert!(app.fusion.is_some());
    app.layers_mut()[1].opacity = 0.5;
    app.fusion_attendre();
    assert_eq!(app.layers().len(), 2);
    assert!(app.status.contains("abandonnée"), "{}", app.status);
    // Une annulation entre-temps : elle rend au calque son réglage d'avant, la fusion calculée n'est plus la bonne.
    let mut app = animation("annulation", 14, true);
    app.set_layer_flag(1, |l| l.opacity = 0.5);
    app.run_action("calque.fusionner_dessous");
    assert!(app.fusion.is_some());
    app.undo();
    app.fusion_attendre();
    assert_eq!(app.layers().len(), 2, "{}", app.status);
    assert!(app.status.contains("abandonnée"), "{}", app.status);
}
#[test]
fn a_second_merge_waits_for_the_first() {
    let mut app = animation("deux", 14, true);
    app.run_action("calque.fusionner_dessous");
    assert!(app.fusion.is_some());
    app.run_action("calque.fusionner_dessous");
    assert!(app.status.contains("déjà en cours"), "{}", app.status);
    app.arreter_fusion();
    regarder(&mut app);
}

#[test]
fn a_quick_merge_stays_immediate_and_takes_less_than_a_frame() {
    // Deux calques, une image chacun, sans effet : tout de suite.
    let mut app = animation("rapide", 1, false);
    let avant = images(&app);
    let t = Instant::now();
    app.run_action("calque.fusionner_dessous");
    let duree = t.elapsed();
    eprintln!("FUSION RAPIDE (feuille 320x180, 1 composition) : {duree:?}");
    assert!(app.fusion.is_none(), "immédiate");
    assert_eq!(app.layers().len(), 1, "{}", app.status);
    assert_eq!(images(&app), avant);
    // Mesure sur une feuille de 1920 × 1080 : trois images de dessins légers, ce que la fusion immédiate accepte.
    let mut doc = Document::new(1920, 1080, 24);
    doc.panels[0].set_length(3);
    let (mut bas, mut haut) = (Layer::new("Bas", LayerKind::Bitmap), Layer::new("Haut", LayerKind::Bitmap));
    for f in 0..3 {
        let (b, h) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
        paint(&mut doc, b, 100 + f * 50, 200, RED);
        paint(&mut doc, h, 300 + f * 40, 400, BLUE);
        bas.frames.push(Some(b));
        haut.frames.push(Some(h));
    }
    doc.panels[0].layers = vec![bas, haut];
    assert!(crate::fusioncalques::est_rapide(&doc, 0, &[0, 1]), "trois images légères : immédiat");
    // (Le meilleur de cinq passages : la suite complète tourne sur tous les cœurs, un seul passage peut être retardé.)
    let meilleur = (0..5).map(|_| hd_max()).min().unwrap();
    assert!(meilleur < IMAGE, "100 tuiles pleines en {meilleur:?}");
    let t = Instant::now();
    doc.merge_layer_down(0, 1).unwrap();
    let hd = t.elapsed();
    eprintln!("FUSION IMMEDIATE (feuille 1920x1080, 3 images) : {hd:?} ; seuil : {} tuiles", crate::fusioncalques::RAPIDE_MAX);
    // Avec un effet, ou plus de compositions : en tâche de fond.
    let app = animation("lourde", 300, false);
    assert!(!crate::fusioncalques::est_rapide(&app.doc, 0, &[0, 1]), "trois cents dessins : en tâche de fond");
    let app = animation("dix", 10, false);
    assert!(crate::fusioncalques::est_rapide(&app.doc, 0, &[0, 1]), "dix dessins légers : tout de suite");
    let app = animation("flou", 1, true);
    assert!(!crate::fusioncalques::est_rapide(&app.doc, 0, &[0, 1]), "un effet : en tâche de fond");
}

/// Le coût d'une fusion immédiate au seuil : 100 tuiles pleines à composer, mesuré (doit tenir dans une image d'écran).
fn hd_max() -> Duration {
    let mut doc = Document::new(1920, 1080, 24);
    doc.panels[0].set_length(1);
    let (mut bas, mut haut) = (Layer::new("Bas", LayerKind::Bitmap), Layer::new("Haut", LayerKind::Bitmap));
    let (b, h) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    // 50 tuiles pleines chacun : 100 au total, le seuil.
    for (cel, couleur) in [(b, RED), (h, BLUE)] {
        let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
        for idx in 0..50 {
            r.tile_mut(idx).fill(couleur);
        }
    }
    bas.frames.push(Some(b));
    haut.frames.push(Some(h));
    doc.panels[0].layers = vec![bas, haut];
    assert!(crate::fusioncalques::est_rapide(&doc, 0, &[0, 1]));
    let t = Instant::now();
    doc.merge_layer_down(0, 1).unwrap();
    let d = t.elapsed();
    eprintln!("FUSION IMMEDIATE AU SEUIL (100 tuiles pleines) : {d:?}");
    d
}