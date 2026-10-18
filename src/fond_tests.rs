//! Fond sous le trait : la forme déduite du trait, et la peinture qui y reste.

use eframe::egui::Color32;

use crate::fond::{compose, edt, silhouette, FondReglages};
use crate::raster::{Raster, TILE};

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

/// Un trait noir de 3 px : les pixels où `inside(x, y)` est vrai.
fn line(w: usize, h: usize, inside: impl Fn(f32, f32) -> bool) -> Raster {
    let mut r = Raster::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if inside(x as f32 + 0.5, y as f32 + 0.5) {
                put(&mut r, x, y, Color32::BLACK);
            }
        }
    }
    r
}

/// Un anneau de centre (100, 100), rayon 50, épaisseur 3 ; `gap` : un trou de cette largeur à
/// droite.
fn ring(gap: f32) -> Raster {
    line(200, 200, |x, y| {
        let d = ((x - 100.0).powi(2) + (y - 100.0).powi(2)).sqrt();
        (48.5..=51.5).contains(&d) && !(x > 140.0 && (y - 100.0).abs() < gap / 2.0)
    })
}

fn at(cover: &[u8], x: usize, y: usize) -> u8 {
    cover[y * 200 + x]
}

#[test]
fn a_closed_shape_is_filled_with_a_margin_around_the_line() {
    let r = FondReglages::default();
    let cover = silhouette(&ring(0.0), &r);
    assert_eq!(at(&cover, 100, 100), 255, "le dedans");
    assert_eq!(at(&cover, 150, 100), 255, "sous le trait");
    // Le bord extérieur du trait est à 151,5 ; le débord de 3 px va jusqu'à 154,5.
    assert_eq!(at(&cover, 153, 100), 255, "dans le débord");
    assert_eq!(at(&cover, 158, 100), 0, "au-delà du débord");
    assert_eq!(at(&cover, 5, 5), 0, "loin dehors");
}

#[test]
fn small_gaps_are_closed_big_ones_are_not() {
    let r = FondReglages::default();
    // Un trou de 8 px (moins de 2 × 6) : fermé.
    assert_eq!(at(&silhouette(&ring(8.0), &r), 100, 100), 255);
    // Un trou de 40 px : le fond ne couvre que le trait et son débord.
    let open = silhouette(&ring(40.0), &r);
    assert_eq!(at(&open, 100, 100), 0, "le dedans fuit par le trou");
    assert_eq!(at(&open, 100, 50), 255, "le trait garde son fond");
    // Plus de fermeture : 2 × 25 > 40.
    let wide = silhouette(&ring(40.0), &FondReglages { trous: 25, ..r });
    assert_eq!(at(&wide, 100, 100), 255);
}

#[test]
fn a_character_cut_by_the_frame_is_filled_to_the_edge() {
    // Un U ouvert en bas qui touche le bord bas de l'image (plan poitrine).
    let u = line(200, 200, |x, y| {
        let side = ((x - 60.0).abs() < 1.5 || (x - 140.0).abs() < 1.5) && y > 80.0;
        let top = (y - 80.0).abs() < 1.5 && (60.0..=140.0).contains(&x);
        side || top
    });
    let r = FondReglages::default();
    assert_eq!(at(&silhouette(&u, &r), 100, 190), 255, "le bord de l'image ferme la forme");
    assert_eq!(at(&silhouette(&u, &FondReglages { bord: false, ..r }), 100, 190), 0, "sinon le dedans touche le dehors");
}

#[test]
fn no_line_no_fond() {
    assert!(silhouette(&Raster::new(64, 64), &FondReglages::default()).iter().all(|&c| c == 0));
}

#[test]
fn paint_stays_inside_the_shape_under_its_color() {
    let (w, h) = (4, 1);
    let cover = vec![255, 128, 0, 255];
    let mut paint = Raster::new(w, h);
    put(&mut paint, 0, 0, Color32::from_rgb(255, 0, 0));
    put(&mut paint, 2, 0, Color32::from_rgb(0, 0, 255));
    let out = compose(&cover, w, h, [255, 255, 255], Some(&paint));
    assert_eq!(out.get(0, 0), Color32::from_rgb(255, 0, 0), "la peinture dans la forme");
    assert_eq!(out.get(1, 0).a(), 128, "le bord adouci");
    assert_eq!(out.get(2, 0).a(), 0, "hors de la forme : rien, même peint");
    assert_eq!(out.get(3, 0), Color32::WHITE, "le fond, de sa couleur");
}

#[test]
fn the_fond_shows_under_the_line_in_the_picture() {
    use crate::document::{Cel, Document};
    use crate::render;
    let mut doc = Document::new(200, 200, 24);
    let a = doc.add_cel(Cel::bitmap(ring(0.0)));
    doc.panels[0].layers[0].frames = vec![Some(a)];
    let plain = render::panel_plan_at(&doc, 0, 0, None);
    assert_eq!(render::composite_pixel(&doc, &plain, 100, 100).a(), 0, "sans fond : on voit à travers");
    doc.panels[0].layers[0].fond = Some(FondReglages { couleur: [250, 240, 200], ..FondReglages::default() });
    let plan = render::panel_plan_at(&doc, 0, 0, None);
    assert_eq!(plan.items.len(), 2, "le fond, puis le trait");
    assert_eq!(render::composite_pixel(&doc, &plan, 100, 100), Color32::from_rgb(250, 240, 200), "le dedans, de la couleur du fond");
    assert_eq!(render::composite_pixel(&doc, &plan, 150, 100), Color32::BLACK, "le trait par-dessus");
    assert_eq!(render::composite_pixel(&doc, &plan, 5, 5).a(), 0, "dehors : rien");
    // Ce qu'on peint dans le fond y reste, sous le trait.
    let mut paint = Raster::new(200, 200);
    put(&mut paint, 100, 100, Color32::from_rgb(0, 0, 255));
    put(&mut paint, 5, 5, Color32::from_rgb(0, 0, 255));
    let c = doc.cels.get_mut(&a).unwrap();
    c.fond = Some(Box::new(Cel::bitmap(paint)));
    c.version += 1;
    assert_eq!(render::composite_pixel(&doc, &plan, 100, 100), Color32::from_rgb(0, 0, 255));
    assert_eq!(render::composite_pixel(&doc, &plan, 5, 5).a(), 0, "hors de la forme : rien, même peint");
}

#[test]
fn the_fond_survives_save_and_reload() {
    use crate::document::{Cel, Document, LayerKind};
    use crate::project;
    let dir = std::env::temp_dir().join(format!("pelure-test-fond-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let drawings = manifest.parent().unwrap().join("dessins");
    let mut doc = Document::new(64, 48, 24);
    let (a, b) = (doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector));
    let reglages = FondReglages { couleur: [250, 240, 200], debord: 5, trous: 9, bord: false };
    doc.panels[0].layers[0].frames = vec![Some(a), Some(b)];
    doc.panels[0].layers[0].fond = Some(reglages);
    let mut paint = Raster::new(64, 48);
    put(&mut paint, 10, 10, Color32::RED);
    doc.cels.get_mut(&a).unwrap().fond = Some(Box::new(Cel::bitmap(paint)));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(drawings.join(format!("{a}_fond.png")).exists(), "la strate Fond à côté du dessin");
    assert!(!drawings.join(format!("{b}_fond.png")).exists(), "rien de peint : pas de fichier");

    let mut back = project::load(&manifest).unwrap().doc;
    let layer = &back.panels[0].layers[0];
    assert_eq!(layer.fond, Some(reglages), "les réglages du fond, avec le calque");
    let ca = layer.frames[0].unwrap();
    assert_eq!(back.cels[&ca].fond.as_ref().unwrap().raster.get(10, 10), Color32::RED, "la peinture du fond");
    // Fond effacé : son fichier part à l'enregistrement suivant.
    let c = back.cels.get_mut(&ca).unwrap();
    c.fond = None;
    c.version += 1;
    project::save(&back, &manifest, &mut Default::default()).unwrap();
    assert!(!drawings.join(format!("{ca}_fond.png")).exists());
    std::fs::remove_dir_all(&dir).ok();
}

/// Mesure (ignorée) : un grand personnage sur une image 1920 × 1080.
#[test]
#[ignore]
fn silhouette_speed_full_hd() {
    let big = line(1920, 1080, |x, y| {
        let d = ((x - 960.0).powi(2) + ((y - 540.0) * 1.6).powi(2)).sqrt();
        (480.0..=484.0).contains(&d)
    });
    let t = std::time::Instant::now();
    let cover = silhouette(&big, &FondReglages::default());
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = std::time::Instant::now();
    let _ = compose(&cover, 1920, 1080, [255, 255, 255], None);
    eprintln!("silhouette {ms:.1} ms, composition {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(cover[540 * 1920 + 960], 255);
}

#[test]
fn distances_match_brute_force() {
    let (w, h) = (23, 17);
    let feature: Vec<bool> = (0..w * h).map(|p| p % 37 == 0 || p == 200).collect();
    let d = edt(&feature, w, h);
    for p in 0..w * h {
        let (x, y) = ((p % w) as f64, (p / w) as f64);
        let best = (0..w * h)
            .filter(|&q| feature[q])
            .map(|q| ((q % w) as f64 - x).powi(2) + ((q / w) as f64 - y).powi(2))
            .fold(f64::INFINITY, f64::min);
        assert!((d[p] - best).abs() < 1e-6, "pixel {p} : {} au lieu de {best}", d[p]);
    }
}
