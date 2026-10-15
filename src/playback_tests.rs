//! Tests de la lecture fluide (phase 3a) : dessins à envoyer d'avance, cadence mesurée.

use crate::document::{Document, Layer, LayerKind};
use crate::playback::{upcoming_cels, upcoming_frames, PlayMeter, Position};
use crate::render::Plan;

/// Trois panneaux de 4 images, chacun avec un calque et deux dessins (images 0-1 et 2-3).
fn doc() -> Document {
    let mut doc = Document::new(64, 48, 4);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    for p in 0..3 {
        let a = doc.new_cel(LayerKind::Bitmap);
        let b = doc.new_cel(LayerKind::Bitmap);
        let mut l = Layer::new("L", LayerKind::Bitmap);
        l.frames = vec![Some(a), Some(a), Some(b), Some(b)];
        doc.panels[p].layers = vec![l];
        doc.panels[p].length = 4;
    }
    doc
}

fn at(panel: usize, frame: usize, playing: bool, montage: bool, looping: bool) -> Position {
    // Tout le plan (pas de points d'entrée ni de sortie, 13d).
    Position { panel, frame, playing, montage, looping, range: (0, usize::MAX) }
}

#[test]
fn playing_the_montage_looks_one_second_ahead_across_panels() {
    let doc = doc();
    // 4 i/s : la seconde qui suit l'image 3 du panneau 0 = images 0 à 3 du panneau 1.
    let frames = upcoming_frames(&doc, at(0, 3, true, true, true));
    assert_eq!(frames, vec![(1, 0), (1, 1), (1, 2), (1, 3)]);
    // En fin de montage : on repart du début si la lecture boucle, sinon on s'arrête.
    assert_eq!(upcoming_frames(&doc, at(2, 2, true, true, true)), vec![(2, 3), (0, 0), (0, 1), (0, 2)]);
    assert_eq!(upcoming_frames(&doc, at(2, 2, true, true, false)), vec![(2, 3)]);
    // Lecture du panneau seul : on boucle dans le panneau.
    assert_eq!(upcoming_frames(&doc, at(1, 2, true, false, true)), vec![(1, 3), (1, 0), (1, 1), (1, 2)]);
}

#[test]
fn stopped_looks_at_neighbours() {
    let doc = doc();
    let frames = upcoming_frames(&doc, at(1, 0, false, true, true));
    assert_eq!(frames, vec![(1, 1), (2, 0), (0, 3), (0, 0)]);
}

#[test]
fn upcoming_cels_skip_shown_ones_and_respect_the_cap() {
    let doc = doc();
    let shown_cel = doc.panels[0].layers[0].cel_at(3).unwrap();
    let shown = Plan {
        paper: None,
        items: vec![crate::render::PlanItem { cel: shown_cel, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: Vec::new() }],
    };
    // Depuis l'image 1 du panneau 0 : images 2, 3 (dessin affiché, ignoré), puis panneau 1.
    let cels = upcoming_cels(&doc, at(0, 1, true, true, true), &shown, 10);
    let p1 = &doc.panels[1].layers[0];
    assert_eq!(cels, vec![p1.cel_at(0).unwrap()]);
    let cels = upcoming_cels(&doc, at(0, 0, true, true, true), &Plan { paper: None, items: vec![] }, 1);
    assert_eq!(cels.len(), 1, "pas plus que la place disponible");
    // Un calque masqué n'est pas préparé.
    let mut hidden = doc;
    hidden.panels[1].layers[0].visible = false;
    assert!(upcoming_cels(&hidden, at(0, 1, true, true, true), &shown, 10).is_empty());
}

#[test]
fn play_meter_counts_shown_and_skipped_frames() {
    let mut m = PlayMeter::default();
    // 24 images voulues en une seconde, mais une sur trois sautée.
    let mut g = 0;
    let mut t = 0.0;
    m.tick(t, g);
    for k in 0..16 {
        g += if k % 2 == 0 { 1 } else { 2 };
        t += 1.0 / 16.0;
        m.tick(t, g);
    }
    let (rate, skipped) = m.rate.expect("une mesure par seconde");
    assert!((rate - 16.0).abs() < 0.5, "{rate}");
    assert_eq!(skipped, 8);
    // Une boucle (retour au début) n'est pas comptée comme un saut.
    let mut m = PlayMeter::default();
    for (k, g) in [10, 11, 0, 1].into_iter().enumerate() {
        m.tick(k as f64 * 0.4, g);
    }
    assert_eq!(m.rate.map(|r| r.1), Some(0));
}
