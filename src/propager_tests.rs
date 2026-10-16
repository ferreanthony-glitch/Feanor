//! Propagation d'une peinture sur un calque vidéo (`propager.rs`) : ce qui est relevé (plage,
//! clés, rectangle de la vidéo), les refus, puis une vraie propagation sur une petite vidéo dont
//! le décor glisse.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use eframe::egui::Color32;
use synthese::propagation::PropagateParams;
use synthese::{Cpu, Image};

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::propager;
use crate::raster::TILE;
use crate::roles::Role;
use crate::video::{self, VideoClip};

/// Taille des images de la vidéo, celle du cadre (la feuille fait le double).
const W: usize = 48;
const H: usize = 32;
/// Où le cadre (donc la vidéo) commence sur la feuille.
const ORIGIN: (usize, usize) = (W / 2, H / 2);

/// Décor coloré (ondes de plusieurs tailles) ; l'image k le montre décalé de 2k pixels à droite.
fn scene(x: f32, y: f32) -> [u8; 3] {
    let wave = |a: f32, b: f32, p: f32| (a * x + b * y + p).sin();
    let r = 128.0 + 60.0 * wave(0.09, 0.04, 0.2) + 40.0 * wave(0.45, -0.31, 1.0);
    let g = 128.0 + 60.0 * wave(-0.05, 0.11, 1.3) + 40.0 * wave(0.27, 0.52, 2.1);
    let b = 128.0 + 60.0 * wave(0.13, -0.08, 2.4) + 40.0 * wave(-0.38, 0.41, 0.4);
    [r as u8, g as u8, b as u8]
}

/// Un document avec un calque vidéo de `n` images et, au-dessus, un calque peint vide.
fn project(tag: &str, n: usize) -> (Document, PathBuf) {
    let dir = std::env::temp_dir().join(format!("pelure-test-propager-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for k in 0..n {
        let img = image::RgbImage::from_fn(W as u32, H as u32, |x, y| image::Rgb(scene(x as f32 - 2.0 * k as f32, y as f32)));
        img.save(video::frame_file(&dir, k, "png")).unwrap();
    }
    let clip = VideoClip { id: video::new_id(), name: "décor".into(), frames: n, fps: 24.0, start: 0, ext: "png".into() };
    video::register_dir(clip.id, dir.clone());
    let mut doc = Document::with_frame(W, H, 24);
    let mut v = Layer::new("Vidéo", LayerKind::Bitmap);
    v.video = Some(clip);
    doc.panels[0].layers = vec![v, Layer::new("Peinture", LayerKind::Bitmap)];
    (doc, dir)
}

/// Un nouveau dessin à l'image `frame` du calque peint : un carré rouge à moitié transparent de
/// côté `side`, en (x, y) dans l'image de la vidéo.
fn paint_square(doc: &mut Document, frame: usize, x: usize, y: usize, side: usize) -> CelId {
    let cel = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    for py in y..y + side {
        for px in x..x + side {
            let (sx, sy) = (ORIGIN.0 + px, ORIGIN.1 + py);
            let i = r.tile_index_of(sx, sy);
            r.tile_mut(i)[(sy % TILE) * TILE + sx % TILE] = Color32::from_rgba_premultiplied(128, 0, 0, 128);
        }
    }
    doc.panels[0].layers[1].set(frame, Some(cel));
    cel
}

#[test]
fn outline_finds_the_video_the_range_and_the_keys() {
    let (mut doc, dir) = project("outline", 6);
    // Un dessin aux images 0 à 2 (exposé trois fois), un autre à l'image 4.
    let first = paint_square(&mut doc, 0, 10, 10, 8);
    doc.panels[0].layers[1].set(1, Some(first));
    doc.panels[0].layers[1].set(2, Some(first));
    paint_square(&mut doc, 4, 20, 10, 8);
    let o = propager::outline(&doc, 0, 1).unwrap();
    assert_eq!((o.video, o.range.clone(), o.keys.clone(), o.outputs()), (0, 0..6, vec![0, 4], 4));
    // Des images marquées « Clé » : ce sont elles, les clés (pour repropager).
    doc.panels[0].layers[1].roles.insert(4, Role::Key);
    assert_eq!(propager::outline(&doc, 0, 1).unwrap().keys, [4]);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn what_cannot_be_propagated() {
    let (mut doc, dir) = project("refus", 3);
    assert!(propager::outline(&doc, 0, 1).unwrap_err().contains("clé"), "pas de dessin");
    assert!(propager::outline(&doc, 0, 0).unwrap_err().contains("calque vidéo"), "le calque vidéo lui-même");
    for f in 0..3 {
        paint_square(&mut doc, f, 4, 4, 4);
    }
    assert!(propager::outline(&doc, 0, 1).unwrap_err().contains("rien à propager"), "que des clés");
    doc.panels[0].layers.remove(0);
    assert!(propager::outline(&doc, 0, 0).unwrap_err().contains("Pas de calque vidéo"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_key_is_taken_in_the_video_rectangle() {
    let (mut doc, dir) = project("cle", 3);
    paint_square(&mut doc, 0, 10, 12, 6);
    let plan = propager::plan(&doc, 0, 1).unwrap();
    assert_eq!((plan.origin, plan.size, plan.files.len()), (ORIGIN, (W, H), 3));
    let s = &plan.styles[0];
    assert_eq!((s.width, s.height, s.channels), (W, H, 4));
    assert_eq!(s.data[(14 * W + 12) * 4..][..4], [128, 0, 0, 128]);
    assert_eq!(s.data[(2 * W + 2) * 4..][..4], [0, 0, 0, 0]);
    // Et une image fabriquée revient au même endroit de la feuille, sans tuile là où rien n'est peint.
    let r = propager::to_raster(doc.width, doc.height, s, plan.origin);
    assert_eq!(propager::crop(&r, plan.origin, plan.size), *s);
    assert!(r.tiles.iter().filter(|t| t.is_some()).count() < r.tiles.len());
    std::fs::remove_dir_all(dir).ok();
}

/// Centre (pondéré par l'alpha) de la peinture d'une image, en x.
fn paint_center_x(im: &Image) -> f32 {
    let (mut s, mut n) = (0f32, 0f32);
    for (i, p) in im.data.chunks_exact(4).enumerate() {
        let a = p[3] as f32;
        s += a * (i % im.width) as f32;
        n += a;
    }
    s / n.max(1.0)
}

#[test]
fn the_paint_follows_the_video_and_fills_the_layer() {
    let (mut doc, dir) = project("suit", 5);
    paint_square(&mut doc, 0, 10, 10, 10);
    let plan = propager::plan(&doc, 0, 1).unwrap();
    let mut made = Vec::new();
    propager::run(&plan, &Cpu, &PropagateParams::default(), &AtomicBool::new(false), |f, im| made.push((f, im))).unwrap();
    let mut frames: Vec<usize> = made.iter().map(|(f, _)| *f).collect();
    frames.sort();
    assert_eq!(frames, [1, 2, 3, 4]);
    // Le décor glisse de 2 pixels par image : la peinture (centre 15 à l'image 0) aussi.
    for (f, im) in &made {
        let cx = paint_center_x(im);
        let want = 15.0 + 2.0 * *f as f32;
        assert!((cx - want).abs() < 2.0, "image {f} : centre {cx}, attendu {want}");
    }
    let before = doc.cels.len();
    propager::apply(&mut doc, 0, 1, plan.origin, &plan.outline.keys, made);
    let layer = &doc.panels[0].layers[1];
    assert_eq!(doc.cels.len(), before + 4);
    assert!((1..5).all(|f| layer.cel_at(f).is_some() && layer.cel_at(f) != layer.cel_at(0)));
    assert_eq!(layer.roles.get(&0), Some(&Role::Key));
    // Repropager garde la clé marquée, et refait les quatre autres.
    assert_eq!(propager::outline(&doc, 0, 1).unwrap().keys, [0]);
    std::fs::remove_dir_all(dir).ok();
}

/// Fabrique un projet de démonstration pour essayer « Propager » dans l'appli :
/// - un calque vidéo fait des images d'un dossier (`PELURE_DEMO_IMAGES`, 000001.jpg…) ;
/// - un calque peint dont la clé, à l'image `PELURE_DEMO_IMAGE` (à partir de 1), est la peinture
///   `PELURE_DEMO_CLE` (même taille que la vidéo), gardée seulement dans le rectangle
///   `PELURE_DEMO_RECT` (« x,y,l,h », tout si absent) ;
/// - enregistré dans `PELURE_DEMO_PROJET` (le manifeste .pelure).
#[test]
#[ignore]
fn demo_project() {
    let var = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("{n} manque"));
    let images = PathBuf::from(var("PELURE_DEMO_IMAGES"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(&images).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jpg")).collect();
    files.sort();
    let (w, h) = image::image_dimensions(&files[0]).unwrap();
    let clip = VideoClip { id: video::new_id(), name: "démo".into(), frames: files.len(), fps: 25.0, start: 0, ext: "jpg".into() };
    video::register_dir(clip.id, images.clone());
    let mut doc = Document::with_frame(w as usize, h as usize, 25);
    let mut v = Layer::new("Vidéo", LayerKind::Bitmap);
    v.video = Some(clip);
    doc.panels[0].layers = vec![v, Layer::new("Peinture", LayerKind::Bitmap)];
    let key: usize = var("PELURE_DEMO_IMAGE").parse::<usize>().unwrap() - 1;
    let paint = image::open(var("PELURE_DEMO_CLE")).unwrap().to_rgba8();
    let rect: Vec<u32> = std::env::var("PELURE_DEMO_RECT").map_or(vec![0, 0, w, h], |r| r.split(',').map(|v| v.trim().parse().unwrap()).collect());
    let origin = propager::plan_origin(&doc, (w as usize, h as usize));
    let cel = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    for y in rect[1]..(rect[1] + rect[3]).min(h) {
        for x in rect[0]..(rect[0] + rect[2]).min(w) {
            let p = paint.get_pixel(x, y).0;
            let (sx, sy) = (origin.0 + x as usize, origin.1 + y as usize);
            let i = r.tile_index_of(sx, sy);
            r.tile_mut(i)[(sy % TILE) * TILE + sx % TILE] = crate::raster::premultiply(p[0], p[1], p[2], p[3]);
        }
    }
    doc.panels[0].layers[1].set(key, Some(cel));
    doc.panels[0].set_length(files.len());
    let manifest = PathBuf::from(var("PELURE_DEMO_PROJET"));
    crate::project::save(&doc, &manifest, &mut std::collections::HashMap::new()).unwrap();
}

#[test]
fn stopping_gives_an_error_and_nothing_else() {
    let (mut doc, dir) = project("arret", 4);
    paint_square(&mut doc, 0, 10, 10, 10);
    let plan = propager::plan(&doc, 0, 1).unwrap();
    let mut made = 0;
    let r = propager::run(&plan, &Cpu, &PropagateParams::default(), &AtomicBool::new(true), |_, _| made += 1);
    assert!(r.is_err());
    assert!(made <= 1, "{made}");
    std::fs::remove_dir_all(dir).ok();
}
