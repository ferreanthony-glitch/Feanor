//! Propagation d'une peinture (`propager.rs`) : ce qui est relevé (plage, clés, rectangle de
//! travail), les guides proposés, les refus, la composition des guides faits de calques, puis de
//! vraies propagations : sur une petite vidéo dont le décor glisse, et sur une animation faite de
//! dessins, sans vidéo.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use eframe::egui::Color32;
use synthese::propagation::PropagateParams;
use synthese::{Cpu, Image};

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::propager::{self, Guide};
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

/// Le guide vidéo du projet de test.
fn video_guide(doc: &Document) -> Guide {
    Guide::Video(doc.panels[0].layers[0].id)
}

/// Un nouveau dessin à l'image `frame` du calque `layer` : un carré de côté `side` en (x, y) du
/// cadre, de la couleur prémultipliée `c`.
fn square(doc: &mut Document, layer: usize, frame: usize, (x, y, side): (usize, usize, usize), c: Color32) -> CelId {
    let cel = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    for py in y..y + side {
        for px in x..x + side {
            let (sx, sy) = (ORIGIN.0 + px, ORIGIN.1 + py);
            let i = r.tile_index_of(sx, sy);
            r.tile_mut(i)[(sy % TILE) * TILE + sx % TILE] = c;
        }
    }
    doc.panels[0].layers[layer].set(frame, Some(cel));
    cel
}

/// La peinture des essais : un carré rouge à moitié transparent sur le calque peint (1).
fn paint_square(doc: &mut Document, frame: usize, x: usize, y: usize, side: usize) -> CelId {
    square(doc, 1, frame, (x, y, side), Color32::from_rgba_premultiplied(128, 0, 0, 128))
}

#[test]
fn outline_finds_the_range_and_the_keys() {
    let (mut doc, dir) = project("outline", 6);
    // Un dessin aux images 0 à 2 (exposé trois fois), un autre à l'image 4.
    let first = paint_square(&mut doc, 0, 10, 10, 8);
    doc.panels[0].layers[1].set(1, Some(first));
    doc.panels[0].layers[1].set(2, Some(first));
    paint_square(&mut doc, 4, 20, 10, 8);
    let guide = video_guide(&doc);
    assert_eq!(propager::default_guide(&doc, 0, 1), guide);
    let o = propager::outline(&doc, 0, 1, guide, None).unwrap();
    assert_eq!((o.guide, o.range.clone(), o.keys.clone(), o.outputs()), (guide, 0..6, vec![0, 4], 4));
    // Des images marquées « Clé » : ce sont elles, les clés (pour repropager).
    doc.panels[0].layers[1].roles.insert(4, Role::Key);
    assert_eq!(propager::outline(&doc, 0, 1, guide, None).unwrap().keys, [4]);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_guides_offered() {
    let (mut doc, dir) = project("guides", 3);
    let mut line = Layer::new("Ligne", LayerKind::Vector);
    line.id = 4242;
    doc.panels[0].layers.push(line);
    let names: Vec<(Guide, String)> = propager::guides(&doc, 0, 1);
    let kinds: Vec<Guide> = names.iter().map(|(g, _)| *g).collect();
    assert_eq!(kinds, [video_guide(&doc), Guide::Below, Guide::Layer(4242)]);
    assert!(names[2].1.contains("Ligne"));
    // Le calque du bas n'a rien dessous.
    assert!(!propager::guides(&doc, 0, 0).iter().any(|(g, _)| *g == Guide::Below));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn what_cannot_be_propagated() {
    let (mut doc, dir) = project("refus", 3);
    let guide = video_guide(&doc);
    assert!(propager::outline(&doc, 0, 1, guide, None).unwrap_err().contains("clé"), "pas de dessin");
    assert!(propager::outline(&doc, 0, 0, Guide::Below, None).unwrap_err().contains("calque vidéo"), "le calque vidéo lui-même");
    for f in 0..3 {
        paint_square(&mut doc, f, 4, 4, 4);
    }
    assert!(propager::outline(&doc, 0, 1, guide, None).unwrap_err().contains("rien à propager"), "que des clés");
    let paint_id = doc.panels[0].layers[1].id;
    assert!(propager::outline(&doc, 0, 1, Guide::Layer(paint_id), None).unwrap_err().contains("lui-même"));
    doc.panels[0].layers.remove(0);
    assert!(propager::outline(&doc, 0, 0, guide, None).unwrap_err().contains("plus dans le plan"));
    assert!(propager::outline(&doc, 0, 0, Guide::Below, None).unwrap_err().contains("Rien sous ce calque"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_key_is_taken_in_the_working_rectangle() {
    let (mut doc, dir) = project("cle", 3);
    paint_square(&mut doc, 0, 10, 12, 6);
    let plan = propager::plan(&doc, 0, 1, video_guide(&doc), None).unwrap();
    assert_eq!((plan.origin, plan.size), (ORIGIN, (W, H)));
    assert!(matches!(&plan.frames, propager::Frames::Files(f) if f.len() == 3));
    let s = &plan.styles[0];
    assert_eq!((s.width, s.height, s.channels), (W, H, 4));
    assert_eq!(s.data[(14 * W + 12) * 4..][..4], [128, 0, 0, 128]);
    assert_eq!(s.data[(2 * W + 2) * 4..][..4], [0, 0, 0, 0]);
    // Et une image fabriquée revient au même endroit de la feuille, sans tuile là où rien n'est peint.
    let r = propager::to_raster(doc.width, doc.height, s, plan.origin);
    assert_eq!(propager::crop(&r, plan.origin, plan.size), *s);
    assert!(r.tiles.iter().filter(|t| t.is_some()).count() < r.tiles.len());
    // Un guide fait de calques : le cadre.
    let plan = propager::plan(&doc, 0, 1, Guide::Below, None).unwrap();
    assert!(matches!(plan.frames, propager::Frames::Rendered));
    assert_eq!((plan.origin, plan.size), (ORIGIN, (W, H)));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn layer_guides_are_composed_on_the_paper() {
    // Décor rouge (0), ligne bleue (1), peinture (2) : dessous = les deux ; « Ligne » seule = bleu
    // sur le papier.
    let mut doc = Document::with_frame(W, H, 24);
    doc.paper = [255, 255, 255];
    doc.panels[0].layers = vec![
        Layer::new("Décor", LayerKind::Bitmap),
        Layer::new("Ligne", LayerKind::Bitmap),
        Layer::new("Peinture", LayerKind::Bitmap),
    ];
    square(&mut doc, 0, 0, (0, 0, 10), Color32::from_rgb(255, 0, 0));
    square(&mut doc, 1, 0, (20, 10, 10), Color32::from_rgb(0, 0, 255));
    let px = |im: &Image, x: usize, y: usize| im.data[(y * im.width + x) * 3..][..3].to_vec();
    let below = propager::render_guide(&doc, 0, 2, Guide::Below, 0);
    assert_eq!((below.width, below.height, below.channels), (W, H, 3));
    assert_eq!((px(&below, 5, 5), px(&below, 25, 15), px(&below, 40, 28)), (vec![255, 0, 0], vec![0, 0, 255], vec![255, 255, 255]));
    let line = propager::render_guide(&doc, 0, 2, Guide::Layer(doc.panels[0].layers[1].id), 0);
    assert_eq!((px(&line, 5, 5), px(&line, 25, 15)), (vec![255, 255, 255], vec![0, 0, 255]));
    // Un calque caché compte pour « calque choisi », pas pour « dessous ».
    doc.panels[0].layers[1].visible = false;
    assert_eq!(px(&propager::render_guide(&doc, 0, 2, Guide::Below, 0), 25, 15), vec![255, 255, 255]);
    assert_eq!(px(&propager::render_guide(&doc, 0, 2, Guide::Layer(doc.panels[0].layers[1].id), 0), 25, 15), vec![0, 0, 255]);
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

/// Les images fabriquées doivent suivre un décor qui glisse de 2 pixels par image (la peinture,
/// centrée en 15 à l'image 0, aussi).
fn check_follows(made: &[(usize, Image)], frames: usize) {
    let mut got: Vec<usize> = made.iter().map(|(f, _)| *f).collect();
    got.sort();
    assert_eq!(got, (1..frames).collect::<Vec<_>>());
    for (f, im) in made {
        let cx = paint_center_x(im);
        let want = 15.0 + 2.0 * *f as f32;
        assert!((cx - want).abs() < 2.0, "image {f} : centre {cx}, attendu {want}");
    }
}

#[test]
fn the_paint_follows_the_video_and_fills_the_layer() {
    let (mut doc, dir) = project("suit", 5);
    paint_square(&mut doc, 0, 10, 10, 10);
    let plan = propager::plan(&doc, 0, 1, video_guide(&doc), None).unwrap();
    let mut made = Vec::new();
    let cancel = AtomicBool::new(false);
    propager::run(&plan, &Cpu, &PropagateParams::default(), &cancel, |f| plan.load_file(f), |f, im| made.push((f, im))).unwrap();
    check_follows(&made, 5);
    let before = doc.cels.len();
    propager::apply(&mut doc, 0, 1, plan.origin, &plan.outline.keys, made);
    let layer = &doc.panels[0].layers[1];
    assert_eq!(doc.cels.len(), before + 4);
    assert!((1..5).all(|f| layer.cel_at(f).is_some() && layer.cel_at(f) != layer.cel_at(0)));
    assert_eq!(layer.roles.get(&0), Some(&Role::Key));
    // Repropager garde la clé marquée, et refait les quatre autres.
    assert_eq!(propager::outline(&doc, 0, 1, video_guide(&doc), None).unwrap().keys, [0]);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_paint_follows_an_animation_without_video() {
    // Une animation faite de dessins (un par image, le décor décalé de 2 pixels à chaque fois),
    // un décor fixe dessous, et la peinture au-dessus : guidée par le calque de l'animation seul.
    let mut doc = Document::with_frame(W, H, 24);
    doc.panels[0].layers =
        vec![Layer::new("Fond", LayerKind::Bitmap), Layer::new("Peinture", LayerKind::Bitmap), Layer::new("Animation", LayerKind::Bitmap)];
    square(&mut doc, 0, 0, (0, 0, W.max(H)), Color32::from_rgb(200, 200, 200));
    for k in 0..5 {
        let cel = doc.new_cel(LayerKind::Bitmap);
        let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
        for y in 0..H {
            for x in 0..W {
                let [cr, cg, cb] = scene(x as f32 - 2.0 * k as f32, y as f32);
                let (sx, sy) = (ORIGIN.0 + x, ORIGIN.1 + y);
                let i = r.tile_index_of(sx, sy);
                r.tile_mut(i)[(sy % TILE) * TILE + sx % TILE] = Color32::from_rgb(cr, cg, cb);
            }
        }
        doc.panels[0].layers[2].set(k, Some(cel));
    }
    paint_square(&mut doc, 0, 10, 10, 10);
    let guide = Guide::Layer(doc.panels[0].layers[2].id);
    let plan = propager::plan(&doc, 0, 1, guide, None).unwrap();
    assert_eq!(plan.outline.range, 0..5);
    let mut made = Vec::new();
    let load = |f: usize| Ok(propager::render_guide(&doc, 0, 1, guide, f));
    propager::run(&plan, &Cpu, &PropagateParams::default(), &AtomicBool::new(false), load, |f, im| made.push((f, im))).unwrap();
    check_follows(&made, 5);
}

/// Les images du calque peint (`Some` : un dessin), de 0 à `n`.
fn exposures(doc: &Document, n: usize) -> Vec<Option<CelId>> {
    (0..n).map(|f| doc.panels[0].layers[1].cel_at(f)).collect()
}

#[test]
fn a_chosen_range_only_remakes_its_frames() {
    // Un ancien dessin exposé partout, la clé (marquée) à l'image 2 ; on refait les images 1 à 4.
    let (mut doc, dir) = project("plage", 8);
    let old = square(&mut doc, 1, 0, (0, 0, 4), Color32::from_rgba_premultiplied(0, 100, 0, 100));
    for f in 1..8 {
        doc.panels[0].layers[1].set(f, Some(old));
    }
    let key = paint_square(&mut doc, 2, 14, 10, 10);
    doc.panels[0].layers[1].roles.insert(2, Role::Key);
    let plan = propager::plan(&doc, 0, 1, video_guide(&doc), Some(1..5)).unwrap();
    let o = &plan.outline;
    assert_eq!((o.range.clone(), o.keys.clone(), o.ignored.clone(), o.outputs()), (1..5, vec![2], vec![], 3));
    // Les images de la vidéo sont prises à partir du début de la plage.
    assert!(matches!(&plan.frames, propager::Frames::Files(f) if f.len() == 4));
    let fourth = image::open(video::frame_file(&dir, 3, "png")).unwrap().to_rgb8().into_raw();
    assert_eq!(plan.load_file(3).unwrap().data, fourth);
    let mut made = Vec::new();
    propager::run(&plan, &Cpu, &PropagateParams::default(), &AtomicBool::new(false), |f| plan.load_file(f), |f, im| made.push((f, im))).unwrap();
    let mut got: Vec<usize> = made.iter().map(|(f, _)| *f).collect();
    got.sort();
    assert_eq!(got, [1, 3, 4]);
    for (f, im) in &made {
        let (cx, want) = (paint_center_x(im), 18.5 + 2.0 * (*f as f32 - 2.0));
        assert!((cx - want).abs() < 2.0, "image {f} : centre {cx}, attendu {want}");
    }
    let before = exposures(&doc, 8);
    propager::apply(&mut doc, 0, 1, plan.origin, &plan.outline.keys, made);
    let after = exposures(&doc, 8);
    for f in [0, 5, 6, 7] {
        assert_eq!(after[f], Some(old), "image {f} hors plage : intacte");
    }
    assert_eq!(after[2], Some(key));
    for f in [1, 3, 4] {
        assert!(after[f].is_some() && after[f] != before[f] && after[f] != Some(key), "image {f} refaite");
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn keys_outside_the_range_are_ignored() {
    // Deux dessins sans rôle : bleu à l'image 1 (exposé jusqu'à la 5), rouge à la 6 (jusqu'à la 7).
    let (mut doc, dir) = project("hors-plage", 8);
    let blue = square(&mut doc, 1, 1, (10, 10, 10), Color32::from_rgba_premultiplied(0, 0, 128, 128));
    for f in 2..6 {
        doc.panels[0].layers[1].set(f, Some(blue));
    }
    let red = paint_square(&mut doc, 6, 20, 10, 10);
    doc.panels[0].layers[1].set(7, Some(red));
    // La plage déborde de celle du guide : on garde ce qui est dedans. L'exposition du bleu à
    // l'image 3 n'est pas une clé.
    let plan = propager::plan(&doc, 0, 1, video_guide(&doc), Some(3..20)).unwrap();
    let o = &plan.outline;
    assert_eq!((o.range.clone(), o.keys.clone(), o.ignored.clone()), (3..8, vec![6], vec![1]));
    assert_eq!(plan.styles.len(), 1);
    let mut made = Vec::new();
    propager::run(&plan, &Cpu, &PropagateParams::default(), &AtomicBool::new(false), |f| plan.load_file(f), |f, im| made.push((f, im))).unwrap();
    let mut got: Vec<usize> = made.iter().map(|(f, _)| *f).collect();
    got.sort();
    assert_eq!(got, [3, 4, 5, 7]);
    for (f, im) in &made {
        assert!(im.data.chunks_exact(4).all(|p| p[2] == 0), "image {f} : rien du bleu, clé hors plage");
        let (cx, want) = (paint_center_x(im), 24.5 + 2.0 * (*f as f32 - 6.0));
        assert!((cx - want).abs() < 2.0, "image {f} : centre {cx}, attendu {want}");
    }
    // Toutes les clés sont marquées, la bleue aussi : une propagation sur les premières images la
    // retrouve.
    let keys = [&plan.outline.keys[..], &plan.outline.ignored[..]].concat();
    propager::apply(&mut doc, 0, 1, plan.origin, &keys, made);
    let after = exposures(&doc, 8);
    assert_eq!(after[..3], [None, Some(blue), Some(blue)]);
    assert_eq!(after[6], Some(red));
    assert_eq!(doc.panels[0].layers[1].roles.get(&1), Some(&Role::Key));
    let o = propager::outline(&doc, 0, 1, video_guide(&doc), Some(0..3)).unwrap();
    assert_eq!((o.keys, o.ignored), (vec![1], vec![6]));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn no_key_in_the_chosen_range_is_refused() {
    let (mut doc, dir) = project("sans-cle", 8);
    paint_square(&mut doc, 0, 10, 10, 10);
    let guide = video_guide(&doc);
    let e = propager::outline(&doc, 0, 1, guide, Some(3..6)).unwrap_err();
    assert!(e.contains("Aucune clé dans la plage"), "{e}");
    assert!(propager::plan(&doc, 0, 1, guide, Some(3..6)).is_err());
    assert!(propager::outline(&doc, 0, 1, guide, Some(10..20)).unwrap_err().contains("hors des images"));
    assert!(propager::outline(&doc, 0, 1, guide, Some(0..1)).unwrap_err().contains("rien à propager"), "que la clé");
    // Sans plage choisie, toute celle du guide.
    assert_eq!(propager::outline(&doc, 0, 1, guide, None).unwrap().range, propager::span(&doc, 0, 1, guide).unwrap());
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
    let plan = propager::plan(&doc, 0, 1, video_guide(&doc), None).unwrap();
    let mut made = 0;
    let r = propager::run(&plan, &Cpu, &PropagateParams::default(), &AtomicBool::new(true), |f| plan.load_file(f), |_, _| made += 1);
    assert!(r.is_err());
    assert!(made <= 1, "{made}");
    std::fs::remove_dir_all(dir).ok();
}
