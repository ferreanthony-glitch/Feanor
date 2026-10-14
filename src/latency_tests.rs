//! Tests du rendu vectoriel par zone et de la pose d'un trait par-dessus (latence) : même image,
//! pixel pour pixel, qu'un rendu complet.

use eframe::egui::{pos2, Rect};

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::document::{Cel, LayerKind};
use crate::history::Edit;
use crate::vector::{Element, VPoint};

fn stroke(points: &[(f32, f32)], r: f32, color: [u8; 3]) -> Element {
    Element::Stroke {
        points: points.iter().map(|&(x, y)| VPoint { x, y, r }).collect(),
        color,
        opacity: 0.8,
        hardness: 0.7,
        brush: None,
    }
}

/// Un dessin vectoriel chargé : remplissages, traits qui se croisent, au bord de la feuille.
fn busy_cel() -> Cel {
    let mut c = Cel::new(300, 200, LayerKind::Vector);
    let d = c.vector.as_mut().unwrap();
    d.elements.push(Element::Fill {
        contours: vec![vec![[20.0, 20.0], [180.0, 30.0], [150.0, 170.0], [10.0, 140.0]]],
        color: [200, 210, 225],
        opacity: 1.0,
    });
    for k in 0..25 {
        let x = 5.0 + k as f32 * 12.0;
        d.elements.push(stroke(&[(x, 3.0), (x + 40.0, 100.0), (x - 10.0, 198.0)], 1.5 + (k % 4) as f32, [k as u8 * 9, 40, 90]));
    }
    d.elements.push(Element::Fill {
        contours: vec![vec![[120.0, 60.0], [290.0, 80.0], [260.0, 190.0]], vec![[180.0, 100.0], [240.0, 110.0], [220.0, 160.0]]],
        color: [230, 120, 60],
        opacity: 0.6,
    });
    c.rerender();
    c
}

fn same(a: &Cel, b: &Cel) -> usize {
    (0..a.raster.height)
        .flat_map(|y| (0..a.raster.width).map(move |x| (x, y)))
        .filter(|&(x, y)| a.raster.get(x, y) != b.raster.get(x, y))
        .count()
}

#[test]
fn region_rerender_matches_full_rerender() {
    let base = busy_cel();
    // On déplace un trait (comme une sélection qu'on tire) et on ne refait que sa zone.
    for (i, dx) in [(5usize, 37.0f32), (12, -60.0), (26, 15.0)] {
        let mut partial = base.clone();
        let d = partial.vector.as_mut().unwrap();
        let before = d.elements[i].bounds();
        d.elements[i] = d.elements[i].transformed(&crate::vector::Affine::translate(eframe::egui::vec2(dx, 3.0)));
        let after = d.elements[i].bounds();
        partial.rerender_region(before.union(after).expand(3.0));
        let mut full = partial.clone();
        full.rerender();
        assert_eq!(same(&partial, &full), 0, "élément {i}");
    }
    // Zone vide ou hors de la feuille : rien ne change.
    let mut c = base.clone();
    assert!(c.rerender_region(Rect::from_min_max(pos2(500.0, 500.0), pos2(600.0, 600.0))).is_empty());
    assert_eq!(same(&c, &base), 0);
}

#[test]
fn new_stroke_on_top_matches_full_rerender() {
    let mut c = busy_cel();
    // Aperçu pendant le geste : le moteur de pinceau peint sur l'image du dessin.
    let path: Vec<(f32, f32)> = (0..30).map(|k| (30.0 + k as f32 * 8.0, 50.0 + (k as f32 * 0.4).sin() * 40.0)).collect();
    let settings = BrushSettings { size: 6.0, smoothing: 0.0, pressure_size: false, ..BrushSettings::brush() };
    let mut s = StrokeState::new(1, StrokeMode::Paint([10, 10, 10]), settings, None, false);
    let mut dirty = Vec::new();
    for &(x, y) in &path {
        s.add_point(&mut c.raster, pos2(x, y), 1.0, &mut dirty);
    }
    let Some(Edit::Tiles { tiles, .. }) = s.finish(&mut c.raster, &mut dirty) else { panic!("tuiles d'avant l'aperçu") };
    // Le trait définitif, posé par-dessus.
    let element = stroke(&path, 3.0, [10, 10, 10]);
    c.vector.as_mut().unwrap().elements.push(element.clone());
    let v0 = c.version;
    let changed = c.render_on_top(tiles, &element);
    assert!(!changed.is_empty());
    assert_eq!(c.version, v0 + 1);
    let mut full = c.clone();
    full.rerender();
    assert_eq!(same(&c, &full), 0, "même image qu'un rendu complet");
}
