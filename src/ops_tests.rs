//! Tests de la phase 1e : couper / réunir des panneaux et des scènes, fusionner des calques,
//! verrouillage enregistré.

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::project;
use crate::vector::{Element, VPoint};

const RED: Color32 = Color32::from_rgb(200, 0, 0);
const BLUE: Color32 = Color32::from_rgb(0, 0, 200);

/// Un panneau de 48 images : dessin A sur 0..30, dessin B sur 30..48.
fn two_drawings() -> (Document, u64, u64) {
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_cel(LayerKind::Vector);
    let b = doc.new_cel(LayerKind::Vector);
    let mut frames = vec![Some(a); 30];
    frames.extend(vec![Some(b); 18]);
    doc.panels[0].layers[0].frames = frames;
    doc.panels[0].action = "Il court.".into();
    (doc, a, b)
}

#[test]
fn split_panel_at_frame() {
    let (mut doc, a, b) = two_drawings();
    assert_eq!(doc.split_panel(0, 0), None, "pas de coupe sur la première image");
    assert_eq!(doc.split_panel(0, 48), None, "ni après la dernière");
    let i = doc.split_panel(0, 20).unwrap();
    assert_eq!(i, 1);
    assert_eq!(doc.panels[0].frame_count(), 20);
    assert_eq!(doc.panels[1].frame_count(), 28, "la durée totale ne change pas");
    assert_eq!(doc.panels[1].scene, doc.panels[0].scene);
    let second = &doc.panels[1].layers[0].frames;
    let (a2, b2) = (second[0].unwrap(), second[10].unwrap());
    assert!(a2 != a && b2 != b, "le nouveau panneau a ses propres dessins");
    assert_eq!(second.iter().filter(|f| **f == Some(a2)).count(), 10, "le dessin coupé reste tenu");
    assert_eq!(doc.panels[0].layers[0].frames, vec![Some(a); 20]);
    assert_eq!((doc.panels[0].action.as_str(), doc.panels[1].action.as_str()), ("Il court.", ""));
}

#[test]
fn merge_is_the_inverse_of_split() {
    let (mut doc, a, _) = two_drawings();
    doc.split_panel(0, 20).unwrap();
    doc.panels[1].action = "Il tombe.".into();
    // Un calque de plus dans le second panneau : il est ajouté, décalé.
    let mut extra = Layer::new("Ombre", LayerKind::Bitmap);
    let c = doc.new_cel(LayerKind::Bitmap);
    extra.frames = vec![Some(c); 5];
    doc.panels[1].layers.push(extra);
    assert_eq!(doc.merge_with_next(0), Some(0));
    assert_eq!(doc.panels.len(), 1);
    let p = &doc.panels[0];
    assert_eq!(p.frame_count(), 48);
    assert_eq!(p.layers[0].frames[..20], vec![Some(a); 20][..]);
    assert!(p.layers[0].frames[20].is_some() && p.layers[0].frames[20] != Some(a));
    assert_eq!(p.layers[1].frames.iter().position(|f| f.is_some()), Some(20));
    assert_eq!(p.action, "Il court.\nIl tombe.");
    assert_eq!(doc.merge_with_next(0), None, "plus de panneau suivant");
    // Pas de réunion par-dessus une limite de scène.
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    assert_eq!(doc.merge_with_next(0), None);
}

#[test]
fn split_and_join_scenes() {
    let mut doc = Document::new(32, 32, 24);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    assert_eq!(doc.split_scene(0), None, "déjà le premier panneau de sa scène");
    assert_eq!(doc.split_scene(1), Some(1));
    assert_eq!(doc.scenes.len(), 2);
    assert_eq!(doc.scene_run(1), (1, 3));
    assert_eq!(doc.scene_label(doc.panels[2].scene), "2");
    assert_eq!(doc.join_previous_scene(0), None, "pas de scène avant la première");
    assert_eq!(doc.join_previous_scene(2), Some(2));
    assert_eq!((doc.scenes.len(), doc.scene_run(0)), (1, (0, 3)));
    // Pas de jonction par-dessus une limite de séquence.
    doc.add_sequence_after(2);
    assert_eq!(doc.join_previous_scene(3), None);
}

fn paint(doc: &mut Document, cel: u64, x: usize, y: usize, c: Color32) {
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % 64) * 64 + x % 64] = c;
}

#[test]
fn merge_bitmap_layers() {
    let mut doc = Document::new(32, 32, 24);
    doc.panels[0].layers = vec![Layer::new("Fond", LayerKind::Bitmap), Layer::new("Perso", LayerKind::Bitmap)];
    let (below, above) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    paint(&mut doc, below, 1, 1, RED);
    paint(&mut doc, above, 2, 2, BLUE);
    doc.panels[0].layers[0].frames = vec![Some(below); 4];
    doc.panels[0].layers[1].frames = vec![Some(above); 2];
    doc.panels[0].layers[1].opacity = 0.5;
    assert!(doc.merge_layer_down(0, 0).is_err(), "rien sous le premier calque");
    doc.merge_layer_down(0, 1).unwrap();
    let p = &doc.panels[0];
    assert_eq!(p.layers.len(), 1);
    assert_eq!((p.layers[0].name.as_str(), p.layers[0].opacity), ("Fond", 1.0));
    let (m1, m2) = (p.layers[0].frames[0].unwrap(), p.layers[0].frames[3].unwrap());
    assert_ne!(m1, m2, "images 0-1 : les deux dessins ; images 2-3 : le fond seul");
    assert_eq!(p.layers[0].frames[1], Some(m1), "l'exposition est gardée");
    let merged = &doc.cels[&m1].raster;
    assert_eq!(merged.get(1, 1), RED);
    assert!((merged.get(2, 2).a() as i32 - 128).abs() <= 1, "l'opacité du calque est appliquée");
    assert_eq!(doc.cels[&above].raster.get(2, 2), BLUE, "les anciens dessins restent intacts");
    assert_ne!(m1, below);
}

#[test]
fn merge_vector_and_mixed_layers() {
    let mut doc = Document::new(32, 32, 24);
    doc.panels[0].layers.push(Layer::new("Haut", LayerKind::Vector));
    let (below, above) = (doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector));
    let stroke = Element::Stroke {
        points: vec![VPoint { x: 2.0, y: 2.0, r: 1.0 }, VPoint { x: 20.0, y: 2.0, r: 1.0 }],
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    };
    for c in [below, above] {
        doc.cels.get_mut(&c).unwrap().vector.as_mut().unwrap().elements.push(stroke.clone());
    }
    doc.panels[0].layers[0].frames = vec![Some(below)];
    doc.panels[0].layers[1].frames = vec![Some(above)];
    doc.panels[0].layers[1].opacity = 0.5;
    doc.merge_layer_down(0, 1).unwrap();
    let layer = &doc.panels[0].layers[0];
    assert_eq!(layer.kind, LayerKind::Vector, "deux calques vectoriels restent vectoriels");
    let d = doc.cels[&layer.frames[0].unwrap()].vector.as_ref().unwrap();
    assert_eq!(d.elements.len(), 2);
    assert!(matches!(d.elements[1], Element::Stroke { opacity, .. } if (opacity - 0.5).abs() < 1e-6));

    // Bitmap par-dessus vectoriel : le résultat est bitmap.
    doc.panels[0].layers.push(Layer::new("Peinture", LayerKind::Bitmap));
    let p = doc.new_cel(LayerKind::Bitmap);
    paint(&mut doc, p, 5, 5, RED);
    doc.panels[0].layers[1].frames = vec![Some(p)];
    doc.merge_layer_down(0, 1).unwrap();
    let layer = &doc.panels[0].layers[0];
    assert_eq!(layer.kind, LayerKind::Bitmap);
    let cel = &doc.cels[&layer.frames[0].unwrap()];
    assert!(cel.vector.is_none());
    assert_eq!(cel.raster.get(5, 5), RED);
    assert!(cel.raster.get(10, 2).a() > 200, "le trait vectoriel est passé en pixels");
}

#[test]
fn merge_refuses_hidden_or_locked_layers() {
    let mut doc = Document::new(32, 32, 24);
    doc.panels[0].layers.push(Layer::new("Haut", LayerKind::Vector));
    doc.panels[0].layers[1].visible = false;
    assert!(doc.merge_layer_down(0, 1).is_err());
    doc.panels[0].layers[1].visible = true;
    doc.panels[0].layers[0].locked = true;
    assert!(doc.merge_layer_down(0, 1).is_err());
    assert_eq!(doc.panels[0].layers.len(), 2, "rien n'a changé");
}

#[test]
fn locked_layers_are_saved() {
    let dir = std::env::temp_dir().join(format!("pelure-test-verrou-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("V.pelure");
    let mut doc = Document::new(32, 32, 24);
    doc.panels[0].layers.push(Layer::new("Décor", LayerKind::Bitmap));
    doc.panels[0].layers[1].locked = true;
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(
        back.panels[0].layers.iter().map(|l| l.locked).collect::<Vec<_>>(),
        vec![false, true]
    );
    std::fs::remove_dir_all(&dir).ok();
}
