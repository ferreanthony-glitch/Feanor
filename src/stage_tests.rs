//! Tests de la phase 2b-1 : feuille de dessin plus grande que le cadre de la caméra.

use eframe::egui::{pos2, Color32};

use crate::document::{Document, Layer, LayerKind};
use crate::project;
use crate::render;
use crate::vector::{Element, VPoint};

const RED: Color32 = Color32::from_rgb(220, 20, 20);

fn paint(doc: &mut Document, cel: u64, x: usize, y: usize) {
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % 64) * 64 + x % 64] = RED;
}

/// Un panneau avec un calque bitmap (un pixel rouge en (1, 1)) et un trait vectoriel.
fn small_doc() -> (Document, u64, u64) {
    let mut doc = Document::new(40, 30, 24);
    doc.panels[0].layers = vec![Layer::new("B", LayerKind::Bitmap), Layer::new("V", LayerKind::Vector)];
    let b = doc.new_cel(LayerKind::Bitmap);
    paint(&mut doc, b, 1, 1);
    let v = doc.new_cel(LayerKind::Vector);
    let c = doc.cels.get_mut(&v).unwrap();
    c.vector.as_mut().unwrap().elements.push(Element::Stroke {
        points: vec![VPoint { x: 5.0, y: 5.0, r: 1.0 }, VPoint { x: 15.0, y: 5.0, r: 1.0 }],
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
    });
    c.rerender();
    doc.panels[0].layers[0].frames = vec![Some(b)];
    doc.panels[0].layers[1].frames = vec![Some(v)];
    (doc, b, v)
}

#[test]
fn new_projects_have_a_stage_twice_the_frame() {
    let doc = Document::with_frame(100, 60, 24);
    assert_eq!((doc.width, doc.height, doc.frame), (200, 120, [100, 60]));
    let r = doc.frame_rect();
    assert_eq!((r.min, r.max), (pos2(50.0, 30.0), pos2(150.0, 90.0)));
    // Sans marge (anciens usages, tests) : le cadre est la feuille.
    let same = Document::new(40, 30, 24);
    assert_eq!(same.frame_rect().min, pos2(0.0, 0.0));
}

#[test]
fn enlarging_the_stage_keeps_the_framing() {
    let (mut doc, b, v) = small_doc();
    doc.guides.push(crate::guides::Guide {
        name: "g".into(),
        points: vec![[10.0, 10.0]],
    });
    doc.enlarge_stage(80, 60);
    doc.frame = [40, 30];
    assert_eq!((doc.width, doc.height), (80, 60));
    let raster = &doc.cels[&b].raster;
    assert_eq!((raster.width, raster.get(21, 16)), (80, RED), "dessin bitmap décalé de la marge");
    assert_eq!(raster.get(1, 1), Color32::TRANSPARENT);
    let d = doc.cels[&v].vector.as_ref().unwrap();
    assert!(matches!(&d.elements[0], Element::Stroke { points, .. } if points[0].x == 25.0 && points[0].y == 20.0));
    assert!(doc.cels[&v].raster.get(30, 20).a() > 200, "rendu vectoriel refait à la nouvelle taille");
    assert_eq!(doc.guides[0].points[0], [30.0, 25.0]);
    // L'image de la caméra est la même qu'avant l'agrandissement.
    let plan = render::panel_plan(&doc.panels[0], 0, None);
    let img = render::camera_image(&doc, &plan, &doc.default_camera());
    assert_eq!(img.len(), 40 * 30);
    assert_eq!(img[40 + 1], RED, "pixel (1, 1) du cadre");
}

#[test]
fn old_projects_are_converted_on_opening() {
    let dir = std::env::temp_dir().join(format!("pelure-test-feuille-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("Ancien.pelure");
    let (doc, b, _) = small_doc();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    // Le même projet, tel qu'un Pelure d'avant la phase 2b l'aurait écrit (format 3, sans cadre).
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    json["version"] = 3.into();
    json.as_object_mut().unwrap().remove("cadre");
    std::fs::write(&manifest, serde_json::to_vec(&json).unwrap()).unwrap();

    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.converted);
    assert!(loaded.written.is_empty(), "tous les dessins seront réécrits");
    let back = loaded.doc;
    assert_eq!((back.width, back.height, back.frame), (80, 60, [40, 30]));
    assert_eq!(back.cels[&b].raster.get(21, 16), RED);

    // Réenregistré : format 4, dessins à la taille de la feuille, plus de conversion.
    let mut written = loaded.written;
    project::save(&back, &manifest, &mut written).unwrap();
    let png = manifest.parent().unwrap().join("dessins").join(format!("{b}.png"));
    assert_eq!(image::image_dimensions(&png).unwrap(), (80, 60));
    let again = project::load(&manifest).unwrap();
    assert!(!again.converted);
    assert_eq!((again.doc.width, again.doc.frame), (80, [40, 30]));
    std::fs::remove_dir_all(&dir).ok();
}
