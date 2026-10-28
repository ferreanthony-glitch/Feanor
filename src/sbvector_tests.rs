//! Dessins vectoriels de Storyboard Pro (14a-2) : traits du modèle, zones colorées, calage.

use std::collections::HashMap;

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
    let raw = raw_elements(&tutorial(), &HashMap::new());
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
            Element::Fill { contours, color, opacity, gradient } => Element::Fill {
                contours: contours.iter().map(|c| c.iter().map(|p| tf(p[0], p[1])).collect()).collect(),
                color: *color,
                opacity: *opacity,
                gradient: *gradient,
            },
            Element::Stroke { points, color, opacity, hardness, brush, trace } => Element::Stroke {
                points: points.iter().map(|p| { let q = tf(p.x, p.y); VPoint { x: q[0], y: q[1], r: p.r * s } }).collect(),
                color: *color,
                opacity: *opacity,
                hardness: *hardness,
                brush: brush.clone(),
                trace: trace.without_cache(),
            },
            other => other.clone(),
        })
        .collect();
    VectorDrawing { elements }.render(1600, 1200)
}

#[test]
fn drawing_is_fitted_on_the_exported_image() {
    let raw = raw_elements(&tutorial(), &HashMap::new());
    let target = exported(&raw, 0.3, 800.0, 600.0);
    let fit = fit_to_image(&raw, &target).expect("calé");
    let (placed, score) = (&fit.elements, fit.score);
    assert!(score > 0.85, "recouvrement {score}");
    // La mesure : 0,3 pixel par unité, axe vers le haut, origine posée en (800, 600).
    assert!((fit.scale - 0.3).abs() < 0.01 && fit.y_up, "échelle {} {}", fit.scale, fit.y_up);
    assert!((fit.origin[0] - 800.0).abs() < 3.0 && (fit.origin[1] - 600.0).abs() < 3.0, "origine {:?}", fit.origin);
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

/// Le texte base64 d'octets (pour fabriquer une texture comme Storyboard Pro l'écrit).
fn base64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { T[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// Un dessin comme `Drawing.query.getData` le donne : un calque avec un carré troué (deux
/// boucles de la couleur « grain », une texture) et un petit carré noir, puis un trait de crayon.
fn data_drawing() -> Value {
    let sq = |x0: f64, y0: f64, x1: f64, y1: f64| {
        json!([{"x": x0, "y": y0, "onCurve": true}, {"x": x1, "y": y0, "onCurve": true}, {"x": x1, "y": y1, "onCurve": true}, {"x": x0, "y": y1, "onCurve": true}, {"x": x0, "y": y0, "onCurve": true}])
    };
    json!({
        "format": "getData",
        "box": {"x0": 0, "x1": 100, "y0": 0, "y1": 100},
        "arts": [{"art": 2, "layers": [
            {"contours": [
                {"colorId": "grain", "path": sq(10.0, 10.0, 90.0, 90.0)},
                {"colorId": "grain", "path": sq(40.0, 40.0, 60.0, 60.0)},
                {"colorId": "noir", "path": sq(0.0, 0.0, 8.0, 8.0)}
            ]},
            {"contours": [], "thicknessPaths": [{"minThickness": 4, "maxThickness": 4, "keys": [{"t": 0, "leftThickness": 4, "rightThickness": 4}, {"t": 1, "leftThickness": 4, "rightThickness": 4}]}],
             "strokes": [{"colorId": "noir", "thickness": {"minThickness": 4, "maxThickness": 4, "thicknessPath": 0}, "path": [{"x": 0, "y": 95, "onCurve": true}, {"x": 100, "y": 95, "onCurve": true}], "closed": false}]}
        ]}]
    })
}

#[test]
fn getdata_drawings_become_fills_with_holes_and_texture_colours() {
    // La texture « grain » : deux pixels rouges opaques, deux transparents.
    let img = image::RgbaImage::from_fn(2, 2, |x, _| if x == 0 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 0]) });
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let colours = json!({
        "grain": {"name": "Image", "isTexture": true, "image": format!("data:image/png;base64,{}", base64(&png))},
        "noir": {"name": "Black", "r": 0, "g": 0, "b": 0, "a": 255}
    });
    let palette = crate::sbvector::palette(&colours);
    let g = palette["grain"];
    assert!((g[0] - 255.0).abs() < 0.5 && g[1] < 0.5 && g[2] < 0.5, "teinte des pixels visibles : {g:?}");
    assert!((g[3] - 127.5).abs() < 0.5, "couverture moyenne : la moitié ({})", g[3]);

    let raw = raw_elements(&data_drawing(), &palette);
    let fills: Vec<&Element> = raw.iter().filter(|e| matches!(e, Element::Fill { .. })).collect();
    assert_eq!((fills.len(), raw.len()), (2, 3), "deux zones (grain, noir), un trait de crayon");
    let Element::Fill { contours, color, opacity, .. } = fills[0] else { unreachable!() };
    assert_eq!((contours.len(), *color), (2, [255, 0, 0]), "les deux boucles du grain ensemble");
    assert!((opacity - 0.5).abs() < 0.01);
    let Element::Stroke { points, color, .. } = &raw[2] else { panic!("le trait de crayon en dernier") };
    assert_eq!(*color, [0, 0, 0]);
    assert!((points[0].r - 2.0).abs() < 0.01, "rayon : le quart de gauche + droite");
    // Le trou reste vide (pair-impair), l'anneau est rempli.
    let r = VectorDrawing { elements: raw }.render(100, 100);
    assert_eq!(r.get(50, 50).a(), 0, "le trou");
    assert!(r.get(20, 50).a() > 0, "l'anneau");
}

/// Les dessins lus par l'essai d'Anthony (`analyse-sbpro\essai-traits`) : ils doivent donner des
/// zones colorées. `PELURE_SB_PNG` : dossier où écrire chaque dessin rendu, pour le voir.
#[test]
#[ignore]
fn real_getdata_drawings_from_the_test_script() {
    let path = std::env::var("PELURE_SB_ESSAI").unwrap_or_else(|_| r"C:\Users\ferre\Documents\Work\analyse-sbpro\essai-traits\essai-traits-lecture.json".into());
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let report: Value = serde_json::from_str(&text).unwrap();
    let mut drawings: Vec<(String, &Value)> = Vec::new();
    if let Some(d) = report.pointer("/dessin_courant/donnees/valeur") {
        drawings.push(("courant".into(), d));
    }
    for p in report["panneaux"].as_array().into_iter().flatten() {
        for c in p["calques"].as_array().into_iter().flatten() {
            if let Some(d) = c.get("donnees") {
                drawings.push((format!("{}-{}", p["nom"].as_str().unwrap_or("?"), c["nom"].as_str().unwrap_or("?")), d));
            }
        }
    }
    assert!(!drawings.is_empty());
    for (name, d) in drawings {
        let mut json = d.clone();
        json["format"] = json!("getData");
        let t = std::time::Instant::now();
        let raw = raw_elements(&json, &HashMap::new());
        let fills = raw.iter().filter(|e| matches!(e, Element::Fill { .. })).count();
        let black = raw.iter().filter(|e| matches!(e, Element::Fill { color: [0, 0, 0], .. })).count();
        println!("{name} : {} éléments, {fills} zones ({black} noires), en {:?}", raw.len(), t.elapsed());
        assert!(fills > 0, "{name} : aucune zone");
        if let Ok(dir) = std::env::var("PELURE_SB_PNG") {
            // Le dessin à 1600 pixels de large, axe vertical retourné (vers le haut dans le modèle).
            let b = raw.iter().fold(eframe::egui::Rect::NOTHING, |r, e| r.union(e.bounds()));
            let s = 1600.0 / b.width().max(1.0);
            let h = (b.height() * s).ceil().max(1.0) as usize;
            let tf = |p: [f32; 2]| [(p[0] - b.min.x) * s, (b.max.y - p[1]) * s];
            let placed: Vec<Element> = raw
                .iter()
                .map(|e| match e {
                    Element::Fill { contours, color, opacity, gradient } => Element::Fill { contours: contours.iter().map(|c| c.iter().map(|p| tf(*p)).collect()).collect(), color: *color, opacity: *opacity, gradient: *gradient },
                    other => other.clone(),
                })
                .collect();
            let r = VectorDrawing { elements: placed }.render(1600, h);
            let mut img = image::RgbaImage::from_pixel(1600, h as u32, image::Rgba([255, 255, 255, 255]));
            for y in 0..h {
                for x in 0..1600 {
                    let c = r.get(x, y);
                    if c.a() > 0 {
                        let a = c.a() as f32 / 255.0;
                        let px = img.get_pixel_mut(x as u32, y as u32);
                        for (k, v) in [c.r(), c.g(), c.b()].into_iter().enumerate() {
                            // Couleur prémultipliée posée sur le blanc.
                            px[k] = (v as f32 + 255.0 * (1.0 - a)).min(255.0) as u8;
                        }
                    }
                }
            }
            img.save(std::path::Path::new(&dir).join(format!("{name}.png"))).unwrap();
        }
    }
}