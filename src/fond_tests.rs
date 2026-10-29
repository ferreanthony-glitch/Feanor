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
    let out = compose(&cover, w, h, [255, 255, 255], Some(&paint), None);
    assert_eq!(out.get(0, 0), Color32::from_rgb(255, 0, 0), "la peinture dans la forme");
    assert_eq!(out.get(1, 0).a(), 128, "le bord adouci");
    assert_eq!(out.get(2, 0).a(), 0, "hors de la forme : rien, même peint");
    assert_eq!(out.get(3, 0), Color32::WHITE, "le fond, de sa couleur");
}

#[test]
fn the_fond_shows_under_the_line_in_the_picture() {
    use crate::document::{Cel, Document, Strate};
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
    let mut s = Strate::new(200, 200);
    s.peinture.raster = paint;
    c.fond = Some(Box::new(s));
    c.version += 1;
    assert_eq!(render::composite_pixel(&doc, &plan, 100, 100), Color32::from_rgb(0, 0, 255));
    assert_eq!(render::composite_pixel(&doc, &plan, 5, 5).a(), 0, "hors de la forme : rien, même peint");
    // La gomme coupe le fond : là, on voit à travers (peinture comprise), le trait reste.
    let c = doc.cels.get_mut(&a).unwrap();
    let s = c.fond.as_mut().unwrap();
    put(&mut s.coupe, 100, 100, Color32::WHITE);
    put(&mut s.coupe, 150, 100, Color32::WHITE);
    s.peinture.version += 1;
    c.version += 1;
    assert_eq!(render::composite_pixel(&doc, &plan, 100, 100).a(), 0, "le fond coupé");
    assert_eq!(render::composite_pixel(&doc, &plan, 150, 100), Color32::BLACK, "le trait n'est pas touché");
    assert_eq!(render::composite_pixel(&doc, &plan, 101, 100), Color32::from_rgb(250, 240, 200), "à côté, le fond reste");
}

#[test]
fn a_cut_removes_the_fond_and_its_paint() {
    let (w, h) = (3, 1);
    let cover = vec![255, 255, 255];
    let mut paint = Raster::new(w, h);
    put(&mut paint, 0, 0, Color32::from_rgb(255, 0, 0));
    let mut cut = Raster::new(w, h);
    put(&mut cut, 0, 0, Color32::WHITE);
    put(&mut cut, 1, 0, Color32::from_rgba_premultiplied(128, 128, 128, 128));
    let out = compose(&cover, w, h, [255, 255, 255], Some(&paint), Some(&cut));
    assert_eq!(out.get(0, 0).a(), 0, "coupé : ni fond ni peinture");
    assert_eq!(out.get(1, 0).a(), 127, "à moitié coupé : à moitié là");
    assert_eq!(out.get(2, 0), Color32::WHITE);
}

#[test]
fn one_tile_recomposed_matches_the_whole() {
    use crate::fond::compose_tile;
    let line = ring(0.0);
    let r = FondReglages::default();
    let cover = silhouette(&line, &r);
    let mut paint = Raster::new(200, 200);
    put(&mut paint, 100, 100, Color32::from_rgb(0, 200, 0));
    let mut cut = Raster::new(200, 200);
    put(&mut cut, 90, 90, Color32::WHITE);
    let whole = compose(&cover, 200, 200, r.couleur, Some(&paint), Some(&cut));
    // Le fond d'avant (sans peinture ni coupe), refait tuile par tuile.
    let mut live = compose(&cover, 200, 200, r.couleur, None, None);
    for idx in 0..live.tiles.len() {
        compose_tile(&mut live, idx, &cover, r.couleur, Some(&paint), Some(&cut));
    }
    for (y, x) in [(100, 100), (90, 90), (50, 100), (5, 5)] {
        assert_eq!(live.get(x, y), whole.get(x, y), "({x}, {y})");
    }
}

#[test]
fn the_fond_survives_save_and_reload() {
    use crate::document::{Document, LayerKind, Strate};
    use crate::project;
    let dir = std::env::temp_dir().join(format!("pelure-test-fond-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let drawings = manifest.parent().unwrap().join("dessins");
    let mut doc = Document::new(64, 48, 24);
    let (a, b) = (doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector));
    let reglages = FondReglages { couleur: [250, 240, 200], debord: 5, trous: 9, bord: false, intelligent: false };
    doc.panels[0].layers[0].frames = vec![Some(a), Some(b)];
    doc.panels[0].layers[0].fond = Some(reglages);
    doc.properties.fond = Some([220, 220, 220]);
    let mut paint = Raster::new(64, 48);
    put(&mut paint, 10, 10, Color32::RED);
    let mut s = Strate::new(64, 48);
    s.peinture.raster = paint;
    let mut cut = Raster::new(64, 48);
    put(&mut cut, 20, 20, Color32::WHITE);
    s.coupe = cut;
    doc.cels.get_mut(&a).unwrap().fond = Some(Box::new(s));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(drawings.join(format!("{a}_fond.png")).exists(), "la strate Fond à côté du dessin");
    assert!(drawings.join(format!("{a}_coupe.png")).exists(), "et sa coupe");
    assert!(!drawings.join(format!("{b}_fond.png")).exists(), "rien de peint : pas de fichier");

    let mut back = project::load(&manifest).unwrap().doc;
    let layer = &back.panels[0].layers[0];
    assert_eq!(layer.fond, Some(reglages), "les réglages du fond, avec le calque");
    assert_eq!(back.properties.fond, Some([220, 220, 220]), "la couleur des fonds du projet");
    let ca = layer.frames[0].unwrap();
    assert_eq!(back.cels[&ca].fond.as_ref().unwrap().peinture.raster.get(10, 10), Color32::RED, "la peinture du fond");
    assert_eq!(back.cels[&ca].fond.as_ref().unwrap().coupe.get(20, 20).a(), 255, "la coupe du fond");
    // Fond effacé : son fichier part à l'enregistrement suivant.
    let c = back.cels.get_mut(&ca).unwrap();
    c.fond = None;
    c.version += 1;
    project::save(&back, &manifest, &mut Default::default()).unwrap();
    assert!(!drawings.join(format!("{ca}_fond.png")).exists());
    assert!(!drawings.join(format!("{ca}_coupe.png")).exists());
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
    let _ = compose(&cover, 1920, 1080, [255, 255, 255], None, None);
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

#[test]
fn a_click_finds_the_zone_closed_by_the_line() {
    use crate::fond::zone;
    let ring = ring(0.0);
    let z = zone(&ring, 100, 100, 6).expect("le dedans de l'anneau");
    let at = |x: usize, y: usize| z[y * 200 + x];
    assert!(at(100, 100) && at(100, 60) && at(60, 100), "tout le dedans");
    assert!(at(147, 100), "jusqu'au trait (la bande de fermeture regagnée)");
    assert!(at(149, 100), "et un peu sous lui : pas de liseré");
    assert!(!at(151, 100) && !at(160, 100) && !at(5, 5), "pas le dehors");
    assert!(zone(&ring, 150, 100, 6).is_none(), "un clic sur le trait ne vise rien");
    // Le dehors : un clic loin du trait.
    let out = zone(&ring, 5, 5, 6).unwrap();
    assert!(out[5 * 200 + 5] && out[100 * 200 + 160] && !out[100 * 200 + 100], "le dehors, pas le dedans");
}

#[test]
fn a_line_across_splits_the_zone_and_gaps_follow_the_setting() {
    use crate::fond::zone;
    // L'anneau coupé en deux par un diamètre vertical.
    let split = line(200, 200, |x, y| {
        let d = ((x - 100.0).powi(2) + (y - 100.0).powi(2)).sqrt();
        (48.5..=51.5).contains(&d) || ((x - 100.0).abs() < 1.5 && d < 50.0)
    });
    let z = zone(&split, 70, 100, 6).unwrap();
    assert!(z[100 * 200 + 70] && !z[100 * 200 + 130], "la moitié gauche seulement");
    // Un trou de 8 px est fermé ; un trou de 40 px laisse la zone rejoindre le dehors.
    assert!(!zone(&ring(8.0), 100, 100, 6).unwrap()[5 * 200 + 5]);
    assert!(zone(&ring(40.0), 100, 100, 6).unwrap()[5 * 200 + 5]);
}

#[test]
fn painting_a_mask_keeps_the_old_tiles() {
    use crate::fond::paint_mask;
    let mut r = Raster::new(100, 70);
    let mask: Vec<bool> = (0..100 * 70).map(|p| p % 100 < 10).collect();
    let old = paint_mask(&mut r, &mask, Some(Color32::RED));
    assert_eq!(old.len(), 2, "les deux tuiles touchées (à gauche, en haut et en bas)");
    assert!(old.iter().all(|(_, t)| t.is_none()), "elles étaient vides");
    assert_eq!((r.get(5, 50), r.get(50, 50).a()), (Color32::RED, 0));
    let old = paint_mask(&mut r, &mask, None);
    assert_eq!(old.len(), 2);
    assert_eq!(r.get(5, 50).a(), 0, "effacé");
}

/// Pose le pixel (`x`, `y`) du monde (hors de la feuille aussi).
fn put_i(r: &mut Raster, x: i64, y: i64, c: Color32) {
    let id = r.id_of(x.div_euclid(TILE as i64) as i32, y.div_euclid(TILE as i64) as i32);
    r.tile_mut(id)[(y.rem_euclid(TILE as i64) as usize) * TILE + x.rem_euclid(TILE as i64) as usize] = c;
}

#[test]
fn the_world_shape_of_a_line_inside_the_sheet_is_the_sheet_shape() {
    use crate::fond::silhouette_monde;
    let r = FondReglages::default();
    let u = line(200, 200, |x, y| {
        let side = ((x - 60.0).abs() < 1.5 || (x - 140.0).abs() < 1.5) && y > 80.0;
        let top = (y - 80.0).abs() < 1.5 && (60.0..=140.0).contains(&x);
        side || top
    });
    let cas = [
        ("anneau", ring(0.0), r),
        ("trou de 8", ring(8.0), r),
        ("trou de 40", ring(40.0), r),
        ("U touchant le bord", u.clone(), r),
        ("U sans le bord", u, FondReglages { bord: false, ..r }),
        ("débord large", ring(0.0), FondReglages { debord: 12, ..r }),
        ("rien", Raster::new(64, 64), r),
    ];
    for (nom, l, r) in cas {
        assert_eq!(silhouette_monde(&l, &r).feuille(l.width, l.height), silhouette(&l, &r), "{nom} : un trait qui reste dans la feuille a la même forme qu'avant");
    }
}

#[test]
fn a_line_leaving_the_sheet_gets_its_shape_and_its_fond_outside_too() {
    use crate::fond::{compose_monde, compose_tuile_monde, silhouette_monde};
    // Un anneau de centre (0, 100), rayon 50 : la moitié est hors de la feuille.
    let mut l = Raster::new(200, 200);
    for y in 30i64..170 {
        for x in -60i64..=60 {
            let d = ((x as f32 + 0.5).powi(2) + (y as f32 + 0.5 - 100.0).powi(2)).sqrt();
            if (48.5..=51.5).contains(&d) {
                put_i(&mut l, x, y, Color32::BLACK);
            }
        }
    }
    let r = FondReglages::default();
    let cover = silhouette_monde(&l, &r);
    for (x, y) in [(0, 100), (-20, 100), (20, 100), (-45, 100), (45, 100)] {
        assert_eq!(cover.get(x, y), 255, "le dedans ({x}, {y})");
    }
    assert_eq!(cover.get(-60, 100), 0, "au-delà du débord");
    assert_eq!(cover.get(-20, 20), 0, "loin dehors");
    // Pas de bord vertical à la limite de la feuille : la forme est symétrique de part et d'autre.
    for y in 60..140 {
        assert_eq!(cover.get(-3, y), cover.get(2, y), "y = {y}");
    }
    let couleur = [250, 240, 200];
    let fond = compose_monde(&cover, 200, 200, couleur, None, None);
    assert_eq!(fond.get_i(-20, 100), Color32::from_rgb(250, 240, 200), "le fond est aussi hors de la feuille");
    assert_eq!(fond.get_i(20, 100), fond.get_i(-20, 100));
    assert!(!fond.outer.is_empty(), "ses tuiles hors de la feuille sont rangées à part");
    assert_eq!(fond.get_i(-60, 100).a(), 0);
    // Ce qu'on peint dans le fond reste dans la forme, loin de la feuille aussi ; refait tuile par tuile = en entier.
    let mut paint = Raster::new(200, 200);
    put_i(&mut paint, -20, 100, Color32::from_rgb(0, 200, 0));
    put_i(&mut paint, 20, 100, Color32::from_rgb(0, 200, 0));
    put_i(&mut paint, -300, 100, Color32::from_rgb(0, 200, 0));
    let whole = compose_monde(&cover, 200, 200, couleur, Some(&paint), None);
    assert_eq!(whole.get_i(-20, 100), Color32::from_rgb(0, 200, 0));
    assert_eq!(whole.get_i(-300, 100).a(), 0, "hors de la forme : rien, même peint");
    let mut live = compose_monde(&cover, 200, 200, couleur, None, None);
    for id in paint.drawn_ids() {
        compose_tuile_monde(&mut live, id, &cover, couleur, Some(&paint), None);
    }
    for (x, y) in [(-20, 100), (20, 100), (-30, 100), (30, 100), (-300, 100)] {
        assert_eq!(live.get_i(x, y), whole.get_i(x, y), "({x}, {y})");
    }
}