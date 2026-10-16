//! Dessins vectoriels de Storyboard Pro (14a-2) : traits du modèle, zones colorées, calage.

use serde_json::{json, Value};

use crate::raster::Raster;
use crate::sbvector::{bezier_points, fit_to_image, raw_elements};
use crate::vector::{Element, VPoint, VectorDrawing};

/// Le dessin de l'exemple du modèle vectoriel (documentation de Storyboard Pro) : un rectangle
/// coupé en deux zones colorées par un trait, et deux traits qui dépassent.
pub(crate) fn tutorial() -> Value {
    let p = |pts: &[(f64, f64)]| -> Value { Value::Array(pts.iter().map(|&(x, y)| json!({"x": x, "y": y, "onCurve": true})).collect()) };
    let th = json!({"minThickness": 7.1, "maxThickness": 7.1, "thicknessPath": 0});
    let layer = json!({
        "index": 0,
        "shaders": [{"colorId": "zone-a"}, {"colorId": "zone-b"}],
        "thicknessPaths": [{"minThickness": 7.1, "maxThickness": 7.1, "keys": [
            {"t": 0.0, "leftThickness": 7.1, "rightThickness": 7.1}, {"t": 1.0, "leftThickness": 7.1, "rightThickness": 7.1}]}],
        "strokes": [
            {"fromJoint": 0, "toJoint": 2, "path": p(&[(-1333.5, 619.4), (-1333.5, -337.9), (-640.7, -337.9)]), "shaderRight": 0, "pencilColorId": "crayon", "thickness": th},
            {"fromJoint": 2, "toJoint": 3, "path": p(&[(-640.7, -337.9), (223.5, -337.9), (223.5, 619.4), (-514.4, 619.4)]), "shaderRight": 1, "pencilColorId": "crayon", "thickness": th},
            {"fromJoint": 3, "toJoint": 0, "path": p(&[(-514.4, 619.4), (-1333.5, 619.4)]), "shaderRight": 0, "pencilColorId": "crayon", "thickness": th},
            {"fromJoint": 4, "toJoint": 3, "path": p(&[(-460.3, 1029.7), (-514.4, 619.4)]), "pencilColorId": "crayon", "thickness": th},
            {"fromJoint": 3, "toJoint": 2, "path": p(&[(-514.4, 619.4), (-640.7, -337.9)]), "shaderLeft": 0, "shaderRight": 1, "pencilColorId": "crayon", "thickness": th},
            {"fromJoint": 2, "toJoint": 1, "path": p(&[(-640.7, -337.9), (-712.8, -885.0)]), "pencilColorId": "crayon", "thickness": th}
        ]
    });
    json!({
        "arts": [{"art": 2, "strokes": {"layers": [layer]}}],
        "colors": {"zone-a": [200, 40, 40, 255], "zone-b": [40, 40, 200, 255], "crayon": [10, 10, 10, 255]}
    })
}

#[test]
fn colour_zones_are_rebuilt_from_their_borders() {
    let raw = raw_elements(&tutorial());
    let fills: Vec<&Element> = raw.iter().filter(|e| matches!(e, Element::Fill { .. })).collect();
    let strokes = raw.iter().filter(|e| matches!(e, Element::Stroke { .. })).count();
    assert_eq!((fills.len(), strokes), (2, 6), "deux zones, six traits de crayon");
    // Les zones d'abord, les traits par-dessus.
    assert!(matches!(raw[0], Element::Fill { .. }) && matches!(raw[7], Element::Stroke { .. }));
    let Element::Fill { contours, color, .. } = fills[0] else { unreachable!() };
    assert_eq!(*color, [200, 40, 40]);
    assert_eq!(contours.len(), 1, "la zone A est bordée par trois traits mis bout à bout");
    let ring = &contours[0];
    let first = ring[0];
    let last = ring[ring.len() - 1];
    assert!((first[0] - last[0]).abs() < 0.01 && (first[1] - last[1]).abs() < 0.01, "contour fermé");
    let Element::Stroke { points, color, .. } = raw.iter().find(|e| matches!(e, Element::Stroke { .. })).unwrap() else { unreachable!() };
    assert_eq!(*color, [10, 10, 10]);
    assert!((points[0].r - 3.55).abs() < 0.01, "rayon : le quart de gauche + droite ({})", points[0].r);
}

#[test]
fn curves_go_through_their_control_points() {
    // Un segment quadratique : départ, contrôle, arrivée.
    let path = json!([{"x": 0.0, "y": 0.0, "onCurve": true}, {"x": 50.0, "y": 100.0, "onCurve": false}, {"x": 100.0, "y": 0.0, "onCurve": true}]);
    let pts = bezier_points(Some(&path));
    assert_eq!(pts.first(), Some(&[0.0, 0.0]));
    assert_eq!(pts.last(), Some(&[100.0, 0.0]));
    let top = pts.iter().fold(0f32, |m, p| m.max(p[1]));
    assert!((top - 50.0).abs() < 1.0, "le sommet d'une quadratique est à mi-hauteur du contrôle ({top})");
}

/// Ce que Storyboard Pro exporterait : le dessin à l'échelle `s`, axe vertical vers le bas,
/// posé à (`ox`, `oy`) sur une feuille 1600×1200.
fn exported(raw: &[Element], s: f32, ox: f32, oy: f32) -> Raster {
    let tf = |x: f32, y: f32| [ox + x * s, oy - y * s];
    let elements = raw
        .iter()
        .map(|e| match e {
            Element::Fill { contours, color, opacity } => Element::Fill {
                contours: contours.iter().map(|c| c.iter().map(|p| tf(p[0], p[1])).collect()).collect(),
                color: *color,
                opacity: *opacity,
            },
            Element::Stroke { points, color, opacity, hardness, brush } => Element::Stroke {
                points: points.iter().map(|p| { let q = tf(p.x, p.y); VPoint { x: q[0], y: q[1], r: p.r * s } }).collect(),
                color: *color,
                opacity: *opacity,
                hardness: *hardness,
                brush: brush.clone(),
            },
            other => other.clone(),
        })
        .collect();
    VectorDrawing { elements }.render(1600, 1200)
}

#[test]
fn drawing_is_fitted_on_the_exported_image() {
    let raw = raw_elements(&tutorial());
    let target = exported(&raw, 0.3, 800.0, 600.0);
    let (placed, score) = fit_to_image(&raw, &target).expect("calé");
    assert!(score > 0.85, "recouvrement {score}");
    // Le coin (-1333.5, 619.4) du modèle tombe en (800 - 400, 600 - 186).
    let Element::Fill { contours, .. } = &placed[0] else { panic!() };
    let p = contours[0][0];
    assert!((p[0] - 399.9).abs() < 3.0 && (p[1] - 414.2).abs() < 3.0, "point {p:?}");
    // Une image qui n'a rien à voir : le calque garde l'image.
    let mut other = Raster::new(1600, 1200);
    for y in 100..140 {
        for x in 100..900 {
            let i = other.tile_index_of(x, y);
            other.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = eframe::egui::Color32::BLACK;
        }
    }
    assert!(fit_to_image(&raw, &other).is_none());
}
