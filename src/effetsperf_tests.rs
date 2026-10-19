//! Mesures de vitesse des calques d'effets (Anthony, 2026-10-19 : la lecture d'un plan avec une
//! image d'impact, une lueur, un tremblement et un timecode doit tenir 24 images par seconde par le
//! chemin du fil de fond et de l'aperçu). Les mesures sont des tests `#[ignore]` (à lancer à la
//! main : `cargo test --release --offline --target-dir target\msvc --lib effetsperf -- --ignored
//! --nocapture --test-threads=1`).
//!
//! La scène est celle de `fxperf_tests` (feuille 3840 × 2160, cadre 1920 × 1080, 24 i/s, deux calques de
//! dessin, 47 images) sans les effets des calques de dessin, avec par-dessus : une image d'impact
//! (images 12 et 13, négatif teinté, rayons et lignes de vitesse), une lueur qui pulse (tout le
//! plan), un tremblement de l'image avec flou de bougé (images 20 à 43) et un timecode.
//!
//! Ce qui est mesuré :
//! - le débit du fil de fond : toutes les images du plan, demandées d'un coup (comme la lecture le fait
//!   à la seconde qui vient), à l'aperçu de l'écran (moitié et quart de la résolution) ; il doit tenir
//!   24 images par seconde ;
//! - le coût sur le fil de l'interface d'une image dont le dessin dérivé est prêt (plan, résolution,
//!   recherche) : il doit rester sous 2 ms ;
//! - pour mémoire, la pleine résolution (repos) et l'export exact d'une image.

use std::time::Instant;

use crate::calqueffet::{self, Mode};
use crate::document::{Document, Layer, LayerKind};
use crate::prereglages::{self, Contexte, Genre, Reglages};
use crate::prereglagesui::layer_of;
use crate::render;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Le plan de l'utilisateur, ses effets de calques retirés, avec les quatre calques d'effets.
fn scene_with_effects() -> Document {
    let (mut doc, _) = crate::fxperf_tests::scene();
    for l in &mut doc.panels[0].layers {
        l.fx.clear();
    }
    let images = doc.panels[0].frame_count();
    let frame = doc.frame;
    let ctx = |debut: usize| Contexte { fps: 24, cadre: frame, debut, images, centre: [50.0, 50.0], taille: [50.0, 50.0] };
    let presets = [
        (12, Reglages { duree: Some(2), mode_impact: 0, teinte: true, eclat: true, lignes: true, ..Reglages::new(Genre::Impact, 24) }),
        (0, Reglages { pulsation: true, periode: 1.0, ..Reglages::new(Genre::Lueur, 24) }),
        (20, Reglages { duree: Some(24), flou_bouge: true, ..Reglages::new(Genre::Tremblement, 24) }),
        (0, Reglages::new(Genre::Timecode, 24)),
    ];
    let mut effect_layers: Vec<Layer> = Vec::new();
    for (debut, r) in presets {
        let built = prereglages::build(&r, &ctx(debut)).expect("un préréglage");
        let cel = doc.new_cel(LayerKind::Bitmap);
        effect_layers.push(layer_of(&built, cel));
    }
    doc.panels[0].layers.extend(effect_layers);
    doc
}

/// Le débit du fil de fond à `level`, en images par seconde : toutes les images du plan sont
/// demandées, on attend la fin.
fn throughput(doc: &Document, level: u8, frames: std::ops::Range<usize>) -> f64 {
    crate::fx::forget_all();
    let n = frames.len();
    // (Un geste ou la lecture : l'image est en aperçu, même à un zoom élevé.)
    crate::fx::note_interaction(&eframe::egui::Context::default());
    let served = calqueffet::proxy_level(level, doc.width, doc.height);
    let t = Instant::now();
    for (k, f) in frames.enumerate() {
        let plan = render::panel_plan_at(doc, 0, f, doc.render_paper());
        let _ = calqueffet::resolve(doc, &plan, Mode::Ahead { level, prio: k as u32 + 1 });
    }
    let asked = ms(t);
    let t = Instant::now();
    crate::fxcache::settle();
    let total = asked + ms(t);
    println!("  niveau demandé {level}, servi {served} : {n} images demandées en {asked:.1} ms (fil de l'interface), calculées en {total:.0} ms : {:.1} images/s ({:.1} ms par image)", n as f64 / (total / 1000.0), total / n as f64);
    n as f64 / (total / 1000.0)
}

/// Où va le temps d'une image : chaque étape de `Snapshot::render` à `level`, pour l'image `frame`.
fn stages(doc: &Document, frame: usize, level: u8) {
    use crate::calqueffet::{Low, Snapshot};
    let plan = render::panel_plan_at(doc, 0, frame, doc.render_paper());
    let lows: Vec<Low> = plan
        .items
        .iter()
        .filter(|i| i.fond.is_none())
        .map(|i| Low { raster: doc.cels[&i.cel].raster.clone(), opacity: i.opacity, tint: i.tint, transform: i.transform, blend: i.blend })
        .collect();
    let steps: Vec<crate::fx::FxStep> = plan
        .items
        .iter()
        .filter_map(|i| match &i.fond {
            Some(render::FondPart::Reglage(r)) => Some(r.steps.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    let snap = Snapshot { width: doc.width, height: doc.height, paper: doc.render_paper(), lows, steps: steps.clone(), opacity: 255, blend: crate::blend::BlendMode::Normal };
    println!("  image {frame}, niveau {level} ({} cœurs) :", crate::parallele::coeurs());
    let t = Instant::now();
    let base = snap.compose(level);
    println!("    composition de ce qui est dessous : {:.1} ms", ms(t));
    for s in &steps {
        let t = Instant::now();
        let _ = crate::fx::apply_scaled(&base, std::slice::from_ref(s), level);
        println!("    {:<24} {:.1} ms", s.kind.label(), ms(t));
    }
    let t = Instant::now();
    let _ = snap.render(level);
    println!("    tout (composition, effets, fusion) : {:.1} ms", ms(t));
}

#[test]
#[ignore]
fn mesure_etapes_d_une_image() {
    crate::fxcache::monde::ecrire();
    let doc = scene_with_effects();
    println!("\nLes étapes d'une image :");
    for (frame, level) in [(12, 2), (22, 2), (22, 1), (22, 0)] {
        stages(&doc, frame, level);
    }
}

#[test]
#[ignore]
fn mesure_calques_effets_en_lecture() {
    crate::fxcache::monde::ecrire();
    let doc = scene_with_effects();
    let images = doc.panels[0].frame_count();
    println!("\nScène : feuille {}×{}, cadre {:?}, {} images, {} calques ({} d'effets)", doc.width, doc.height, doc.frame, images, doc.panels[0].layers.len(), 4);

    println!("Débit du fil de fond (ce que la lecture demande ; le niveau servi dépend de la taille de la feuille) :");
    // Zoom moyen (niveau demandé 1), zoom faible (2), zoom élevé (0 : la lecture est quand même en aperçu).
    let medium = throughput(&doc, 1, 0..images);
    let low = throughput(&doc, 2, 0..images);
    let high = throughput(&doc, 0, 0..images);
    // Seule la partie de la lecture avec les quatre effets à la fois (images 20 à 43 : tremblement, lueur, timecode).
    let busy = throughput(&doc, 1, 20..44);
    println!("Pour mémoire, la pleine résolution (au repos, pas en lecture) :");
    crate::fx::forget_all();
    let t = Instant::now();
    for f in 20..26 {
        let plan = render::panel_plan_at(&doc, 0, f, doc.render_paper());
        let _ = calqueffet::resolve(&doc, &plan, Mode::Ahead { level: 0, prio: 1 });
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    crate::fxcache::settle();
    println!("  6 images : {:.0} ms", ms(t) - 300.0);

    println!("Sur le fil de l'interface, une image dont les dessins dérivés sont prêts :");
    crate::fx::forget_all();
    crate::fx::note_interaction(&eframe::egui::Context::default());
    for f in 0..images {
        let plan = render::panel_plan_at(&doc, 0, f, doc.render_paper());
        let _ = calqueffet::resolve(&doc, &plan, Mode::Display { level: 1, prio: 0 });
    }
    crate::fxcache::settle();
    let mut costs = Vec::new();
    for f in 0..images {
        let t = Instant::now();
        let plan = render::panel_plan_at(&doc, 0, f, doc.render_paper());
        let shown = calqueffet::resolve(&doc, &plan, Mode::Display { level: 1, prio: 0 });
        costs.push(ms(t));
        if f >= 20 && f < 44 {
            assert_eq!(shown.items.len(), 1, "image {f} : les dessins dérivés sont prêts, un seul élément reste");
        }
    }
    costs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = costs.iter().sum::<f64>() / costs.len() as f64;
    println!("  plan + résolution : moy {mean:.2} ms, médiane {:.2}, pire {:.2} ({} images)", costs[costs.len() / 2], costs[costs.len() - 1], costs.len());

    println!("Export exact d'une image (calcul bloquant, pleine résolution) :");
    crate::fx::forget_all();
    let t = Instant::now();
    let plan = render::panel_plan_at(&doc, 0, 22, doc.render_paper());
    let _ = render::camera_image(&doc, &plan, &doc.panel_camera(0, 22));
    println!("  image 22 : {:.0} ms", ms(t));

    assert!(medium >= 24.0, "zoom moyen : {medium:.1} images/s, il en faut 24");
    assert!(low >= 24.0, "zoom faible : {low:.1} images/s, il en faut 24");
    assert!(high >= 24.0, "zoom élevé : {high:.1} images/s, il en faut 24");
    assert!(busy >= 24.0, "les images chargées (quatre effets à la fois) : {busy:.1} images/s");
    assert!(mean < 2.0, "le fil de l'interface paie {mean:.2} ms par image, il en faut moins de 2");
}
