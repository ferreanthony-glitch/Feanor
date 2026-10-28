//! Tests du coloriage par zones (`coloriage.rs`, `coloriageui.rs`) : des dessins au trait
//! synthétiques (formes qui glissent, grossissent, tournent, une zone qui apparaît, deux zones qui
//! échangent leurs places, un trou dans un trait), colorés sur leurs clés, puis coloriés ; la
//! sortie en formes pleines vectorielles et en aplats bitmap ; le trait intact ; l'annulation ; les
//! zones incertaines ; la vitesse (essai ignoré).

use std::sync::atomic::AtomicBool;

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::coloriage::{self, colorier, fill_raster, fill_vector, Bilan, ConseillerCouleur, Conseil, Demande, Doute, Incertain, Parametres, Question, Rgba, SansConseil, Why};
use crate::coloriageui::Dest;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::raster::{unpremultiply, Raster, TILE};
use crate::roles::Role;
use crate::vector::{Element, VectorDrawing};
use crate::zones::{Reglages, ZoneCache};

pub(crate) const W: usize = 420;
pub(crate) const H: usize = 300;

pub(crate) const RED: [u8; 3] = [220, 40, 40];
pub(crate) const BLUE: [u8; 3] = [40, 80, 220];
pub(crate) const GREEN: [u8; 3] = [40, 180, 70];
pub(crate) const YELLOW: [u8; 3] = [240, 200, 30];

// ----------------------------------------------------------------------------- dessins synthétiques

/// Une forme : ses points, fermée ou non, la couleur de son aplat dans les clés, et les côtés qu'on
/// ne trace pas (un trou dans le trait).
#[derive(Clone)]
pub(crate) struct Shape {
    pub pts: Vec<(f32, f32)>,
    pub closed: bool,
    pub fill: Option<[u8; 3]>,
    pub skip: Vec<usize>,
}

pub(crate) fn closed(pts: Vec<(f32, f32)>, fill: [u8; 3]) -> Shape {
    Shape { pts, closed: true, fill: Some(fill), skip: Vec::new() }
}

pub(crate) fn rot(p: (f32, f32), deg: f32) -> (f32, f32) {
    let (s, c) = deg.to_radians().sin_cos();
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

pub(crate) fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, deg: f32) -> Vec<(f32, f32)> {
    (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * std::f32::consts::TAU;
            let (x, y) = rot((rx * a.cos(), ry * a.sin()), deg);
            (cx + x, cy + y)
        })
        .collect()
}

pub(crate) fn rect(cx: f32, cy: f32, w: f32, h: f32, deg: f32) -> Vec<(f32, f32)> {
    [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)]
        .iter()
        .map(|&(x, y)| {
            let (rx, ry) = rot((x * w, y * h), deg);
            (cx + rx, cy + ry)
        })
        .collect()
}

fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let i = r.tile_index_of(x, y);
    r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = c;
}

/// Le trait noir des formes, d'épaisseur `thick`, au bord adouci.
pub(crate) fn art(w: usize, h: usize, thick: f32, shapes: &[Shape]) -> Raster {
    let mut r = Raster::new(w, h);
    for s in shapes {
        let n = s.pts.len();
        let count = if s.closed { n } else { n - 1 };
        for k in 0..count {
            if s.skip.contains(&k) {
                continue;
            }
            let (a, b) = (s.pts[k], s.pts[(k + 1) % n]);
            let reach = thick / 2.0 + 1.5;
            let (x0, x1) = ((a.0.min(b.0) - reach).floor().max(0.0) as usize, ((a.0.max(b.0) + reach).ceil() as usize).min(w - 1));
            let (y0, y1) = ((a.1.min(b.1) - reach).floor().max(0.0) as usize, ((a.1.max(b.1) + reach).ceil() as usize).min(h - 1));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let d = dist_to_segment((x as f32 + 0.5, y as f32 + 0.5), a, b);
                    let alpha = ((thick / 2.0 + 0.5 - d).clamp(0.0, 1.0) * 255.0) as u8;
                    if alpha > r.get(x, y).a() {
                        put(&mut r, x, y, Color32::from_rgba_premultiplied(0, 0, 0, alpha));
                    }
                }
            }
        }
    }
    r
}

fn inside(pts: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut c = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

/// Les aplats des clés : l'intérieur de chaque forme qui a une couleur, dans l'ordre (la dernière
/// par-dessus), tels qu'un pot de peinture les poserait (un peu sous le trait).
pub(crate) fn paint(w: usize, h: usize, shapes: &[Shape]) -> Raster {
    let mut r = Raster::new(w, h);
    for s in shapes {
        let Some(c) = s.fill else { continue };
        let (x0, x1) = (s.pts.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (s.pts.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as usize).min(w - 1));
        let (y0, y1) = (s.pts.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (s.pts.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as usize).min(h - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                if inside(&s.pts, (x as f32 + 0.5, y as f32 + 0.5)) {
                    put(&mut r, x, y, Color32::from_rgb(c[0], c[1], c[2]));
                }
            }
        }
    }
    r
}

/// Les mêmes aplats en formes pleines vectorielles.
pub(crate) fn paint_vector(shapes: &[Shape]) -> VectorDrawing {
    VectorDrawing {
        elements: shapes
            .iter()
            .filter_map(|s| s.fill.map(|c| Element::Fill { contours: vec![s.pts.iter().map(|p| [p.0, p.1]).collect()], color: c, opacity: 1.0, gradient: None }))
            .collect(),
    }
}

/// La couleur (non prémultipliée) du pixel (`x`, `y`).
pub(crate) fn px(r: &Raster, x: f32, y: f32) -> Rgba {
    unpremultiply(r.get(x as usize, y as usize))
}

pub(crate) fn rgba(c: [u8; 3]) -> Rgba {
    [c[0], c[1], c[2], 255]
}

// ----------------------------------------------------------------------------- le personnage

/// Position, échelle et angle du personnage à une image.
#[derive(Clone, Copy)]
pub(crate) struct Pose {
    pub cx: f32,
    pub cy: f32,
    pub s: f32,
    pub a: f32,
}

impl Pose {
    fn at(&self, dx: f32, dy: f32) -> (f32, f32) {
        let (rx, ry) = rot((dx * self.s, dy * self.s), self.a);
        (self.cx + rx, self.cy + ry)
    }

    /// Le corps (rouge, percé de l'œil et de l'insigne), l'œil (bleu), l'insigne (vert).
    pub(crate) fn shapes(&self) -> Vec<Shape> {
        let (ex, ey) = self.at(15.0, -35.0);
        let (bx, by) = self.at(-20.0, 40.0);
        vec![
            closed(rect(self.cx, self.cy, 100.0 * self.s, 140.0 * self.s, self.a), RED),
            closed(ellipse(ex, ey, 14.0 * self.s, 9.0 * self.s, self.a), BLUE),
            closed(ellipse(bx, by, 10.0 * self.s, 10.0 * self.s, 0.0), GREEN),
        ]
    }

    /// Un point de chaque zone, avec sa couleur attendue : corps, œil, insigne.
    pub(crate) fn samples(&self) -> [((f32, f32), [u8; 3]); 3] {
        [(self.at(-30.0, -10.0), RED), (self.at(15.0, -35.0), BLUE), (self.at(-20.0, 40.0), GREEN)]
    }
}

pub(crate) const THICK: f32 = 3.0;

// ----------------------------------------------------------------------------- la demande

/// Une demande de coloriage : les formes de chaque image, les images clés.
pub(crate) fn demande(frames: &[Vec<Shape>], keys: &[usize], parametres: Parametres) -> Demande {
    Demande {
        cadres: frames
            .iter()
            .enumerate()
            .map(|(f, shapes)| coloriage::Cadre {
                cel: f as u64 + 1,
                version: 0,
                ligne: art(W, H, THICK, shapes),
                cle: keys.contains(&f).then(|| paint(W, H, shapes)),
                cible: !keys.contains(&f),
            })
            .collect(),
        parametres,
    }
}

/// Colorie et rend, pour chaque image coloriée, ses aplats en pixels, avec les doutes et le bilan.
pub(crate) fn run(d: &Demande, advisor: &dyn ConseillerCouleur) -> (Vec<(usize, Raster)>, Vec<Doute>, Bilan) {
    let cache = ZoneCache::new(256 << 20);
    let mut out = Vec::new();
    let mut doubts = Vec::new();
    let bilan = colorier(d, &cache, advisor, &AtomicBool::new(false), &|_, _| {}, &mut |image| {
        out.push((image.cadre, fill_raster(&image.seg, &image.couleurs)));
        doubts.extend(image.doutes);
    })
    .expect("coloriage");
    (out, doubts, bilan)
}

fn check_pose(out: &[(usize, Raster)], f: usize, pose: Pose) {
    let r = &out.iter().find(|(i, _)| *i == f).unwrap_or_else(|| panic!("image {f} pas coloriée")).1;
    for ((x, y), want) in pose.samples() {
        assert_eq!(px(r, x, y), rgba(want), "image {f}, point ({x:.0}, {y:.0})");
    }
}

// ----------------------------------------------------------------------------- les essais

#[test]
fn a_character_that_slides_keeps_its_colours() {
    let poses: Vec<Pose> = (0..8).map(|f| Pose { cx: 120.0 + 9.0 * f as f32, cy: 150.0, s: 1.0, a: 0.0 }).collect();
    let frames: Vec<Vec<Shape>> = poses.iter().map(Pose::shapes).collect();
    let (out, doubts, bilan) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    assert_eq!(out.len(), 7);
    for f in 1..8 {
        check_pose(&out, f, poses[f]);
    }
    assert!(doubts.is_empty(), "rien d'incertain : {doubts:?}");
    assert_eq!((bilan.images, bilan.zones_coloriees), (7, 21));
}

#[test]
fn a_character_that_scales_and_turns_keeps_its_colours() {
    let poses: Vec<Pose> = (0..10).map(|f| Pose { cx: 130.0 + 6.0 * f as f32, cy: 150.0, s: 1.0 + 0.05 * f as f32, a: 6.0 * f as f32 }).collect();
    let frames: Vec<Vec<Shape>> = poses.iter().map(Pose::shapes).collect();
    let (out, doubts, _) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    for f in 1..10 {
        check_pose(&out, f, poses[f]);
    }
    assert!(doubts.is_empty(), "rien d'incertain : {doubts:?}");
}

#[test]
fn two_keys_around_the_frames_both_agree() {
    let poses: Vec<Pose> = (0..9).map(|f| Pose { cx: 110.0 + 12.0 * f as f32, cy: 150.0, s: 1.0 + 0.02 * f as f32, a: 4.0 * f as f32 }).collect();
    let frames: Vec<Vec<Shape>> = poses.iter().map(Pose::shapes).collect();
    let (out, doubts, _) = run(&demande(&frames, &[0, 8], Parametres::default()), &SansConseil);
    assert_eq!(out.len(), 7);
    for f in 1..8 {
        check_pose(&out, f, poses[f]);
    }
    assert!(doubts.is_empty(), "{doubts:?}");
}

/// Un rond rouge et un rectangle bleu, qui échangent leurs places (l'un monte à droite, l'autre à
/// gauche, sur deux lignes).
#[test]
fn two_zones_that_swap_places_keep_their_own_colours() {
    let at = |f: usize| {
        let (ax, bx) = (80.0 + 20.0 * f as f32, 240.0 - 20.0 * f as f32);
        (vec![closed(ellipse(ax, 70.0, 24.0, 24.0, 0.0), RED), closed(rect(bx, 170.0, 70.0, 40.0, 0.0), BLUE)], (ax, bx))
    };
    let frames: Vec<Vec<Shape>> = (0..9).map(|f| at(f).0).collect();
    let (out, doubts, _) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    for f in 1..9 {
        let (_, (ax, bx)) = at(f);
        let r = &out.iter().find(|(i, _)| *i == f).unwrap().1;
        assert_eq!(px(r, ax, 70.0), rgba(RED), "image {f} : le rond reste rouge");
        assert_eq!(px(r, bx, 170.0), rgba(BLUE), "image {f} : le rectangle reste bleu");
    }
    assert!(doubts.is_empty(), "{doubts:?}");
}

/// Un troisième objet entre dans l'image à l'image 3.
fn with_newcomer(f: usize) -> Vec<Shape> {
    let mut shapes = Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 }.shapes();
    if f >= 3 {
        shapes.push(closed(rect(330.0, 90.0, 60.0, 50.0, 0.0), YELLOW));
    }
    shapes
}

#[test]
fn a_zone_that_appears_is_left_uncoloured_and_reported() {
    let frames: Vec<Vec<Shape>> = (0..6).map(with_newcomer).collect();
    let (out, doubts, bilan) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    for f in 1..6 {
        check_pose(&out, f, Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 });
    }
    // Les images 3, 4 et 5 ont une zone sans pareille avant.
    let r3 = &out.iter().find(|(i, _)| *i == 3).unwrap().1;
    assert_eq!(px(r3, 330.0, 90.0)[3], 0, "la nouvelle zone reste vide");
    assert_eq!(doubts.len(), 3, "{doubts:?}");
    assert!(doubts.iter().all(|d| d.raison == Why::New && d.aire > 2000), "{doubts:?}");
    assert_eq!(doubts.iter().map(|d| d.cadre).collect::<Vec<_>>(), [3, 4, 5]);
    assert_eq!(bilan.zones_incertaines, 3);
    // Marquée « à vérifier » si on le demande.
    let marked = Parametres { incertain: Incertain::Marquer(coloriage::REPERE), ..Parametres::default() };
    let (out, doubts, _) = run(&demande(&frames, &[0], marked), &SansConseil);
    let r4 = &out.iter().find(|(i, _)| *i == 4).unwrap().1;
    assert_eq!(px(r4, 330.0, 90.0), [255, 0, 255, 255]);
    assert_eq!(doubts.len(), 3);
}

#[test]
fn a_zone_that_appears_is_coloured_by_the_key_that_has_it() {
    // Une seconde clé à la fin, où la nouvelle zone est jaune : l'appariement en arrière la retrouve.
    let frames: Vec<Vec<Shape>> = (0..6).map(with_newcomer).collect();
    let (out, doubts, _) = run(&demande(&frames, &[0, 5], Parametres::default()), &SansConseil);
    for f in 3..5 {
        let r = &out.iter().find(|(i, _)| *i == f).unwrap().1;
        assert_eq!(px(r, 330.0, 90.0), rgba(YELLOW), "image {f}");
        check_pose(&out, f, Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 });
    }
    assert!(doubts.is_empty(), "{doubts:?}");
}

#[test]
fn a_zone_split_in_two_keeps_the_colour_of_the_original() {
    // Un rectangle jaune qu'un trait coupe en deux à l'image 2.
    let whole = |_: usize| vec![closed(rect(200.0, 150.0, 160.0, 100.0, 0.0), YELLOW)];
    let cut = |f: usize| {
        let mut s = whole(f);
        s.push(Shape { pts: vec![(200.0, 90.0), (200.0, 210.0)], closed: false, fill: None, skip: Vec::new() });
        s
    };
    let frames: Vec<Vec<Shape>> = (0..4).map(|f| if f >= 2 { cut(f) } else { whole(f) }).collect();
    let (out, _, _) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    let r = &out.iter().find(|(i, _)| *i == 3).unwrap().1;
    assert_eq!(px(r, 150.0, 150.0), rgba(YELLOW));
    assert_eq!(px(r, 250.0, 150.0), rgba(YELLOW));
}

#[test]
fn a_gap_in_the_line_is_closed_by_the_gap_closing() {
    let ring = |cx: f32| {
        let mut s = closed(ellipse(cx, 150.0, 40.0, 40.0, 0.0), RED);
        s.skip = vec![10, 11];
        vec![s, closed(rect(330.0, 150.0, 40.0, 40.0, 0.0), BLUE)]
    };
    let frames: Vec<Vec<Shape>> = (0..4).map(|f| ring(100.0 + 15.0 * f as f32)).collect();
    // Fermé : le rond est une zone, rouge dans toutes les images.
    let (out, _, _) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    for f in 1..4 {
        let r = &out.iter().find(|(i, _)| *i == f).unwrap().1;
        assert_eq!(px(r, 100.0 + 15.0 * f as f32, 150.0), rgba(RED), "image {f}");
        assert_eq!(px(r, 330.0, 150.0), rgba(BLUE), "image {f}");
    }
    // Sans fermeture des trous, le rond fuit vers le dehors : plus de zone, donc plus de rouge.
    let open = Parametres { zones: Reglages { trous: 0, debord: 3 }, ..Parametres::default() };
    let (out, _, _) = run(&demande(&frames, &[0], open), &SansConseil);
    let r = &out.iter().find(|(i, _)| *i == 2).unwrap().1;
    assert_eq!(px(r, 130.0, 150.0)[3], 0, "sans fermeture, le rond n'est pas une zone");
}

/// Une grille de petites cases de 14 pixels (plus petites que la fermeture des trous), de cinq
/// couleurs en damier, qui glisse de 2 pixels par image.
fn cells(f: usize, cols: usize, rows: usize) -> Vec<Shape> {
    let palette = [RED, BLUE, GREEN, YELLOW, [150, 60, 200]];
    let mut out = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            out.push(closed(rect(12.0 + 17.0 * i as f32 + 2.0 * f as f32, 12.0 + 17.0 * j as f32 + f as f32, 15.0, 15.0, 0.0), palette[(i + 2 * j) % 5]));
        }
    }
    out
}

#[test]
fn hundreds_of_small_zones_keep_their_colours() {
    // 24 × 16 = 384 zones : plus que le hongrois n'en prend, le choix glouton les apparie.
    let frames: Vec<Vec<Shape>> = (0..4).map(|f| cells(f, 24, 16)).collect();
    let (out, doubts, bilan) = run(&demande(&frames, &[0], Parametres::default()), &SansConseil);
    assert_eq!(bilan.images, 3);
    let palette = [RED, BLUE, GREEN, YELLOW, [150, 60, 200]];
    let mut wrong = 0;
    for (f, r) in &out {
        for j in 0..16 {
            for i in 0..24 {
                let (x, y) = (12.0 + 17.0 * i as f32 + 2.0 * *f as f32, 12.0 + 17.0 * j as f32 + *f as f32);
                wrong += usize::from(px(r, x, y) != rgba(palette[(i + 2 * j) % 5]));
            }
        }
    }
    assert!(wrong <= 3 * 384 / 50, "{wrong} cases mal coloriées sur {} ({} doutes)", 3 * 384, doubts.len());
}

#[test]
fn a_cancel_stops_the_work() {
    let frames: Vec<Vec<Shape>> = (0..6).map(|f| Pose { cx: 120.0 + 5.0 * f as f32, cy: 150.0, s: 1.0, a: 0.0 }.shapes()).collect();
    let d = demande(&frames, &[0], Parametres::default());
    let cache = ZoneCache::new(64 << 20);
    let stop = AtomicBool::new(true);
    let r = colorier(&d, &cache, &SansConseil, &stop, &|_, _| {}, &mut |_| {});
    assert!(r.is_err());
}

#[test]
fn without_a_key_there_is_nothing_to_do() {
    let frames: Vec<Vec<Shape>> = (0..3).map(|_| Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 }.shapes()).collect();
    let cache = ZoneCache::new(64 << 20);
    let d = demande(&frames, &[], Parametres::default());
    assert!(colorier(&d, &cache, &SansConseil, &AtomicBool::new(false), &|_, _| {}, &mut |_| {}).is_err());
}

/// Un conseiller qui colore en vert toute zone nouvelle.
struct Greenish;
impl ConseillerCouleur for Greenish {
    fn conseille(&self, q: &Question) -> Option<Conseil> {
        (q.proposition.is_none() && q.zone.area > 100).then_some(Conseil { couleur: rgba(GREEN), confiance: 0.9 })
    }
}

#[test]
fn the_colour_advisor_can_settle_a_new_zone() {
    let frames: Vec<Vec<Shape>> = (0..6).map(with_newcomer).collect();
    let d = demande(&frames, &[0], Parametres::default());
    let (out, doubts, _) = run(&d, &Greenish);
    let r = &out.iter().find(|(i, _)| *i == 4).unwrap().1;
    assert_eq!(px(r, 330.0, 90.0), rgba(GREEN));
    assert!(doubts.is_empty(), "le conseiller a tranché : {doubts:?}");
    // Le conseiller par défaut ne dit rien : la zone reste signalée.
    let (_, doubts, _) = run(&d, &SansConseil);
    assert_eq!(doubts.len(), 3);
}

#[test]
fn the_colours_of_a_key_are_read_from_pixels_and_from_vector_fills() {
    let shapes = Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 }.shapes();
    let line = art(W, H, THICK, &shapes);
    let seg = crate::zones::segment(&line, &Reglages::default());
    let pose = Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 };
    let from_pixels = coloriage::read_colours(&seg, &paint(W, H, &shapes));
    let vector = paint_vector(&shapes);
    let from_vector = coloriage::read_colours(&seg, &vector.render(W, H));
    for colours in [&from_pixels, &from_vector] {
        for ((x, y), want) in pose.samples() {
            let id = seg.label_at(x as i64, y as i64);
            assert_eq!(colours[id as usize - 1], Some(rgba(want)));
        }
        // Le dehors n'a jamais de couleur.
        assert!(seg.zones.iter().zip(colours.iter()).all(|(z, c)| !z.outside || c.is_none()));
    }
    // Un dessin de couleurs vide : rien de colorié.
    assert!(coloriage::read_colours(&seg, &Raster::new(W, H)).iter().all(Option::is_none));
}

#[test]
fn the_fills_are_flat_and_cover_the_line_without_a_gap() {
    let shapes = Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 }.shapes();
    let line = art(W, H, THICK, &shapes);
    let seg = crate::zones::segment(&line, &Reglages::default());
    let colours: Vec<Option<Rgba>> = seg
        .zones
        .iter()
        .map(|z| {
            let (cx, cy) = z.centroid;
            (!z.outside).then(|| if z.area > 8000 { rgba(RED) } else { rgba(BLUE) }).filter(|_| cx > 0.0 && cy > 0.0)
        })
        .collect();
    let flat = fill_raster(&seg, &colours);
    // Aucun pixel de demi-teinte : un aplat net, sous le trait.
    let mut tints = std::collections::BTreeSet::new();
    for idx in 0..flat.tiles.len() {
        if let Some(t) = &flat.tiles[idx] {
            tints.extend(t.iter().filter(|c| c.a() != 0).map(|c| c.to_array()));
        }
    }
    assert!(tints.len() <= 2, "{tints:?}");
    assert!(tints.iter().all(|c| c[3] == 255));
    // Chaque pixel du trait (hors dehors) est sous un aplat : pas de jour sous le trait.
    let (mut open, mut total) = (0, 0);
    for y in 0..H {
        for x in 0..W {
            if line.get(x, y).a() >= 200 && seg.label_at(x as i64, y as i64) != 0 {
                total += 1;
                open += usize::from(flat.get(x, y).a() == 0);
            }
        }
    }
    assert!(total > 500);
    assert!(open * 20 < total, "{open} pixels de trait sans aplat sur {total}");
}

#[test]
fn the_vector_fills_are_editable_shapes_that_match_the_pixel_fills() {
    let shapes = Pose { cx: 120.0, cy: 150.0, s: 1.0, a: 0.0 }.shapes();
    let line = art(W, H, THICK, &shapes);
    let seg = crate::zones::segment(&line, &Reglages::default());
    let colours: Vec<Option<Rgba>> = seg.zones.iter().map(|z| (!z.outside).then(|| if z.area > 8000 { rgba(RED) } else { rgba(GREEN) })).collect();
    let drawing = fill_vector(&seg, &colours);
    assert_eq!(drawing.elements.len(), 3, "une forme pleine par zone");
    assert!(drawing.elements.iter().all(|e| matches!(e, Element::Fill { contours, .. } if !contours.is_empty() && contours.iter().all(|c| c.len() >= 3))));
    // Le corps est percé de deux trous (l'œil et l'insigne).
    let body = drawing.elements.iter().find_map(|e| match e {
        Element::Fill { contours, color, .. } if *color == RED => Some(contours.len()),
        _ => None,
    });
    assert_eq!(body, Some(3), "le contour du corps et ses deux trous");
    // Rendue, la forme donne les mêmes couleurs que l'aplat en pixels, hors des bords.
    let rendered = drawing.render(W, H);
    let flat = fill_raster(&seg, &colours);
    let mut differing = 0;
    for y in 0..H {
        for x in 0..W {
            let (a, b) = (rendered.get(x, y), flat.get(x, y));
            if (a.a() > 250) != (b.a() > 250) || (a.a() > 250 && a != b) {
                differing += 1;
            }
        }
    }
    assert!(differing < 800, "{differing} pixels diffèrent (les bords seulement)");
}

#[test]
fn hungarian_finds_the_cheapest_assignment() {
    // Les paires (0,1), (1,0), (2,2) coûtent 1 + 2 + 3 ; la diagonale 4 + 5 + 3.
    let a = [vec![4.0, 1.0, 9.0], vec![2.0, 5.0, 9.0], vec![9.0, 9.0, 3.0]];
    assert_eq!(coloriage::hungarian(&a), vec![1, 0, 2]);
}

// ----------------------------------------------------------------------------- dans l'application

struct Fixture {
    app: App,
    ctx: egui::Context,
    line: u64,
    colour: u64,
}

impl Fixture {
    fn line_index(&self) -> usize {
        self.app.doc.panels[0].layers.iter().position(|l| l.id == self.line).unwrap()
    }
    fn colour_index(&self) -> usize {
        self.app.doc.panels[0].layers.iter().position(|l| l.id == self.colour).unwrap()
    }
    /// Colorie avec les réglages posés et attend la fin.
    fn colorier(&mut self) {
        self.app.start_coloriage(&self.ctx);
        assert!(self.app.coloriage.running(), "{}", self.app.status);
        self.app.finish_coloriage_blocking();
    }
}

/// Un plan de `frames` images : le calque de couleur (en bas, `kind`, peint aux clés `keys`) et le
/// calque de trait par-dessus (bitmap), les formes de chaque image données par `shapes`.
fn fixture(name: &str, frames: usize, kind: LayerKind, keys: &[usize], shapes: &dyn Fn(usize) -> Vec<Shape>) -> Fixture {
    let dir = std::env::temp_dir().join(format!("shot-coloriage-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(frames);
    let mut colour = Layer::new("Couleur", kind);
    let mut line = Layer::new("Trait", LayerKind::Bitmap);
    for f in 0..frames {
        let s = shapes(f);
        let cel = doc.add_cel(Cel::bitmap(art(W, H, THICK, &s)));
        line.set(f, Some(cel));
        if keys.contains(&f) {
            let cel = match kind {
                LayerKind::Bitmap => doc.add_cel(Cel::bitmap(paint(W, H, &s))),
                LayerKind::Vector => {
                    let drawing = paint_vector(&s);
                    doc.add_cel(Cel { raster: drawing.render(W, H), vector: Some(drawing), version: 0, fond: None })
                }
            };
            colour.set(f, Some(cel));
        }
    }
    let ids = (line.id, colour.id);
    doc.panels[0].layers.extend([colour, line]);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 1;
    app.cur_frame = 0;
    Fixture { app, ctx, line: ids.0, colour: ids.1 }
}

fn slide(f: usize) -> Vec<Shape> {
    Pose { cx: 120.0 + 9.0 * f as f32, cy: 150.0, s: 1.0, a: 0.0 }.shapes()
}

fn slide_pose(f: usize) -> Pose {
    Pose { cx: 120.0 + 9.0 * f as f32, cy: 150.0, s: 1.0, a: 0.0 }
}

fn line_snapshot(fx: &Fixture) -> (Vec<Option<u64>>, Vec<(u64, u64, Vec<Option<crate::raster::Tile>>)>) {
    let layer = &fx.app.doc.panels[0].layers[fx.line_index()];
    let mut cels: Vec<(u64, u64, Vec<Option<crate::raster::Tile>>)> =
        layer.frames.iter().flatten().map(|id| (*id, fx.app.doc.cels[id].version, fx.app.doc.cels[id].raster.tiles.clone())).collect();
    cels.sort_by_key(|c| c.0);
    cels.dedup_by_key(|c| c.0);
    (layer.frames.clone(), cels)
}

#[test]
fn vector_colour_layer_gets_editable_fills_and_the_line_is_untouched() {
    let mut fx = fixture("vecteur", 6, LayerKind::Vector, &[0], &slide);
    let before = line_snapshot(&fx);
    fx.colorier();
    let layer = &fx.app.doc.panels[0].layers[fx.colour_index()];
    assert!((0..6).all(|f| layer.cel_at(f).is_some()), "chaque image a ses aplats");
    for f in 1..6 {
        let cel = &fx.app.doc.cels[&layer.cel_at(f).unwrap()];
        let drawing = cel.vector.as_ref().expect("un dessin vectoriel");
        assert_eq!(drawing.elements.len(), 3, "image {f} : une forme pleine par zone");
        assert!(drawing.elements.iter().all(|e| matches!(e, Element::Fill { .. })));
        for ((x, y), want) in slide_pose(f).samples() {
            assert_eq!(px(&cel.raster, x, y), rgba(want), "image {f}");
        }
    }
    // La clé n'a pas bougé, et est marquée « Clé » ; le trait est intact.
    assert_eq!(layer.roles.get(&0), Some(&Role::Key));
    assert_eq!(line_snapshot(&fx), before, "le calque de trait n'est jamais modifié");
    assert_eq!(fx.app.history.names().0.last().copied(), Some("Colorier par zones"));
    assert!(fx.app.status.contains("Coloriage par zones"), "{}", fx.app.status);
}

#[test]
fn bitmap_colour_layer_gets_flat_fills_under_the_line() {
    let mut fx = fixture("bitmap", 5, LayerKind::Bitmap, &[0], &slide);
    let before = line_snapshot(&fx);
    fx.colorier();
    let layer = &fx.app.doc.panels[0].layers[fx.colour_index()];
    for f in 1..5 {
        let cel = &fx.app.doc.cels[&layer.cel_at(f).unwrap()];
        assert!(cel.vector.is_none(), "un calque bitmap reçoit des pixels");
        for ((x, y), want) in slide_pose(f).samples() {
            assert_eq!(px(&cel.raster, x, y), rgba(want), "image {f}");
        }
    }
    assert_eq!(line_snapshot(&fx), before);
}

#[test]
fn colouring_is_one_undo_step() {
    let mut fx = fixture("annuler", 5, LayerKind::Vector, &[0], &slide);
    let frames_before = fx.app.doc.panels[0].layers[fx.colour_index()].frames.clone();
    let steps_before = fx.app.history.names().0.len();
    fx.colorier();
    assert_eq!(fx.app.history.names().0.len(), steps_before + 1, "une seule étape");
    assert_ne!(fx.app.doc.panels[0].layers[fx.colour_index()].frames, frames_before);
    fx.app.undo();
    assert_eq!(fx.app.doc.panels[0].layers[fx.colour_index()].frames, frames_before);
    assert!(fx.app.doc.panels[0].layers[fx.colour_index()].roles.is_empty() || fx.app.doc.panels[0].layers[fx.colour_index()].roles.get(&0).is_none());
    fx.app.redo();
    assert!(fx.app.doc.panels[0].layers[fx.colour_index()].cel_at(3).is_some());
}

#[test]
fn a_new_colour_layer_goes_just_under_the_line_and_undo_removes_it() {
    let mut fx = fixture("nouveau", 5, LayerKind::Bitmap, &[0], &slide);
    fx.app.coloriage.dest = Dest::New(LayerKind::Vector);
    let n = fx.app.doc.panels[0].layers.len();
    fx.colorier();
    let layers = &fx.app.doc.panels[0].layers;
    assert_eq!(layers.len(), n + 1);
    let line = fx.line_index();
    let made = &layers[line - 1];
    assert_eq!((made.name.as_str(), made.kind), ("Couleur 2", LayerKind::Vector), "sous le trait, vectoriel, et le nom « Couleur » était pris");
    // Les clés y sont aussi (réécrites en formes pleines), comme les autres images.
    assert!((0..5).all(|f| made.cel_at(f).is_some()));
    let key = &fx.app.doc.cels[&made.cel_at(0).unwrap()];
    assert!(key.vector.as_ref().is_some_and(|d| d.elements.len() == 3));
    assert_eq!(fx.app.cur_layer, line, "le calque de trait reste le calque courant");
    fx.app.undo();
    assert_eq!(fx.app.doc.panels[0].layers.len(), n);
}

#[test]
fn another_layer_can_receive_the_fills_and_a_locked_layer_refuses() {
    let mut fx = fixture("autre", 5, LayerKind::Bitmap, &[0], &slide);
    let other = Layer::new("Aplats", LayerKind::Vector);
    let other_id = other.id;
    fx.app.doc.panels[0].layers.insert(0, other);
    fx.app.cur_layer += 1;
    // Les clés viennent du calque « Couleur », les aplats vont dans « Aplats » (vectoriel).
    fx.app.coloriage.dest = Dest::Layer(other_id);
    let source_frames = fx.app.doc.panels[0].layers[fx.colour_index()].frames.clone();
    fx.colorier();
    let layers = &fx.app.doc.panels[0].layers;
    let other = layers.iter().find(|l| l.id == other_id).unwrap();
    assert!((0..5).all(|f| other.cel_at(f).is_some()), "les clés aussi y sont écrites");
    assert!(fx.app.doc.cels[&other.cel_at(2).unwrap()].vector.is_some());
    assert_eq!(layers[fx.colour_index()].frames, source_frames, "le calque des clés ne change pas");
    // Verrouillé : refusé.
    let mut fx = fixture("verrou", 5, LayerKind::Bitmap, &[0], &slide);
    let colour = fx.colour_index();
    fx.app.doc.panels[0].layers[colour].locked = true;
    assert!(fx.app.coloriage_plan().is_err());
    fx.app.start_coloriage(&fx.ctx);
    assert!(!fx.app.coloriage.running());
}

/// Le trait d'un calque vectoriel est lu par son rendu.
#[test]
fn a_vector_line_layer_is_read_through_its_render() {
    let stroke = |pts: &[(f32, f32)]| Element::Stroke {
        points: pts.iter().map(|p| crate::vector::VPoint { x: p.0, y: p.1, r: 1.6 }).collect(),
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    };
    let square = |cx: f32| {
        let pts = rect(cx, 150.0, 120.0, 90.0, 0.0);
        let mut closed_pts = pts.clone();
        closed_pts.push(pts[0]);
        stroke(&closed_pts)
    };
    let dir = std::env::temp_dir().join(format!("shot-coloriage-vecteur-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(4);
    let mut line = Layer::new("Trait", LayerKind::Vector);
    let mut colour = Layer::new("Couleur", LayerKind::Bitmap);
    for f in 0..4 {
        let drawing = VectorDrawing { elements: vec![square(150.0 + 20.0 * f as f32)] };
        let cel = doc.add_cel(Cel { raster: drawing.render(W, H), vector: Some(drawing), version: 0, fond: None });
        line.set(f, Some(cel));
    }
    let key = doc.add_cel(Cel::bitmap(paint(W, H, &[closed(rect(150.0, 150.0, 120.0, 90.0, 0.0), YELLOW)])));
    colour.set(0, Some(key));
    doc.panels[0].layers.extend([colour, line]);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 1;
    app.start_coloriage(&ctx);
    assert!(app.coloriage.running(), "{}", app.status);
    app.finish_coloriage_blocking();
    let layer = &app.doc.panels[0].layers[0];
    for f in 1..4 {
        let cel = &app.doc.cels[&layer.cel_at(f).unwrap()];
        assert_eq!(px(&cel.raster, 150.0 + 20.0 * f as f32, 150.0), rgba(YELLOW), "image {f}");
        assert_eq!(px(&cel.raster, 20.0, 20.0)[3], 0);
    }
}

#[test]
fn a_chosen_range_and_chosen_keys() {
    let mut fx = fixture("plage", 10, LayerKind::Vector, &[0, 9], &slide);
    // Seules les images 3 à 5.
    fx.app.coloriage.whole_plan = false;
    fx.app.coloriage.range = (3, 5);
    fx.colorier();
    let layer = &fx.app.doc.panels[0].layers[fx.colour_index()];
    let got: Vec<usize> = (0..10).filter(|&f| layer.cel_at(f).is_some()).collect();
    assert_eq!(got, [0, 3, 4, 5, 9]);
    // Des clés tapées : l'image 9 seule, qui a de la couleur.
    let mut fx = fixture("cles", 10, LayerKind::Vector, &[0, 9], &slide);
    fx.app.coloriage.keys_typed = true;
    fx.app.coloriage.keys_text = "10".into();
    fx.colorier();
    let layer = &fx.app.doc.panels[0].layers[fx.colour_index()];
    for f in 1..9 {
        let cel = &fx.app.doc.cels[&layer.cel_at(f).unwrap()];
        for ((x, y), want) in slide_pose(f).samples() {
            assert_eq!(px(&cel.raster, x, y), rgba(want), "image {f}");
        }
    }
    // Une image tapée sans couleur : rien à faire, et rien ne change.
    let mut fx = fixture("cles2", 6, LayerKind::Vector, &[0], &slide);
    fx.app.coloriage.keys_typed = true;
    fx.app.coloriage.keys_text = "3".into();
    let frames = fx.app.doc.panels[0].layers[fx.colour_index()].frames.clone();
    assert!(fx.app.coloriage_plan().is_err());
    fx.app.start_coloriage(&fx.ctx);
    assert!(!fx.app.coloriage.running());
    assert_eq!(fx.app.doc.panels[0].layers[fx.colour_index()].frames, frames);
}

#[test]
fn a_held_drawing_shares_its_colours_and_uncertain_zones_are_listed() {
    // L'image 1 tient le dessin de l'image 0 (la clé) ; la zone nouvelle arrive à l'image 3.
    let shapes = |f: usize| with_newcomer(f);
    let mut fx = fixture("tenu", 6, LayerKind::Vector, &[0], &shapes);
    // L'image 1 montre le même dessin de trait que l'image 0.
    let line_idx = fx.line_index();
    let first = fx.app.doc.panels[0].layers[line_idx].cel_at(0);
    fx.app.doc.panels[0].layers[line_idx].set(1, first);
    fx.colorier();
    let layer = &fx.app.doc.panels[0].layers[fx.colour_index()];
    assert_eq!(layer.cel_at(1), layer.cel_at(0), "le dessin tenu garde la couleur de sa clé");
    // La nouvelle zone des images 3 à 5 est dans la liste, une ligne par image.
    assert_eq!(fx.app.coloriage.report_len(), Some(3));
    let entries = fx.app.coloriage.report_entries();
    assert_eq!(entries.iter().map(|e| e.0).collect::<Vec<_>>(), [3, 4, 5]);
    assert!(entries.iter().all(|e| e.2 == Why::New));
    // Cliquer une ligne mène à l'image.
    assert_eq!(fx.app.cur_frame, 0);
}

#[test]
fn the_command_exists_and_opens_the_window() {
    let mut fx = fixture("commande", 4, LayerKind::Vector, &[0], &slide);
    assert!(crate::keymap::action_info("calque.colorier_zones").is_some_and(|a| a.implemented));
    assert!(!fx.app.coloriage.window_open());
    fx.app.run_action("calque.colorier_zones");
    assert!(fx.app.coloriage.window_open());
    // La fenêtre se dessine sans erreur, avec le résumé des clés.
    let ctx = fx.ctx.clone();
    let _ = ctx.run(egui::RawInput::default(), |ctx| fx.app.coloriage_ui(ctx));
    let plan = fx.app.coloriage_plan().expect("un plan");
    assert_eq!((plan.keys.clone(), plan.targets), (vec![0], 3));
}

#[test]
fn the_second_run_finds_the_segmentations_in_the_cache() {
    let mut fx = fixture("cache", 6, LayerKind::Vector, &[0], &slide);
    fx.colorier();
    let first = fx.app.coloriage.cache().computed();
    assert!(first >= 6, "{first}");
    fx.colorier();
    assert_eq!(fx.app.coloriage.cache().computed(), first, "rien n'est redécoupé");
    // Un dessin retouché (sa version change) est redécoupé, lui seul.
    let line = fx.line_index();
    let cel = fx.app.doc.panels[0].layers[line].cel_at(2).unwrap();
    fx.app.doc.cels.get_mut(&cel).unwrap().version += 1;
    fx.colorier();
    assert_eq!(fx.app.coloriage.cache().computed(), first + 1);
}

#[test]
fn frames_typed_in_the_window_are_read_like_the_timeline() {
    let fx = fixture("saisie", 20, LayerKind::Vector, &[0], &slide);
    // La numérotation commence à 1.
    assert_eq!(crate::coloriageui::parse_frames("1, 5-7 ; 12", &fx.app), Ok(vec![0, 4, 5, 6, 11]));
    assert!(crate::coloriageui::parse_frames("a", &fx.app).is_err());
}

// ----------------------------------------------------------------------------- vitesse

/// Une grille de 10 × 5 rectangles (50 zones) qui glisse, sur une feuille de 1920 × 1080.
fn grid(f: usize) -> Vec<Shape> {
    let mut out = Vec::new();
    for j in 0..5 {
        for i in 0..10 {
            let (cx, cy) = (110.0 + 180.0 * i as f32 + 2.0 * f as f32, 120.0 + 200.0 * j as f32 + 1.0 * f as f32);
            let colour = [[220, 40, 40], [40, 80, 220], [40, 180, 70], [240, 200, 30], [150, 60, 200]][(i + j) % 5];
            out.push(closed(rect(cx, cy, 150.0 + (i * 3) as f32, 160.0 - (j * 4) as f32, 0.0), colour));
        }
    }
    out
}

#[test]
#[ignore]
fn perf_coloriage_1080p_50_zones() {
    use std::time::Instant;
    let (w, h) = (1920, 1080);
    let frames: Vec<Vec<Shape>> = (0..13).map(grid).collect();
    let t = Instant::now();
    let lines: Vec<Raster> = frames.iter().map(|s| art(w, h, 3.0, s)).collect();
    println!("dessin synthétique : {:?}", t.elapsed());
    let r = Reglages::default();
    let t = Instant::now();
    let seg0 = crate::zones::segment(&lines[0], &r);
    let one = t.elapsed();
    println!("découpe d'une image 1080p ({} zones) : {one:?}", seg0.zones.len());
    let seg1 = crate::zones::segment(&lines[1], &r);
    let t = Instant::now();
    let links = coloriage::match_zones(&seg0, &seg1);
    println!("appariement de deux images : {:?} ({} liens)", t.elapsed(), links.iter().flatten().count());
    let t = Instant::now();
    let _ = fill_raster(&seg0, &seg0.zones.iter().map(|z| (!z.outside).then_some([1, 2, 3, 255])).collect::<Vec<_>>());
    println!("aplats en pixels : {:?}", t.elapsed());
    let t = Instant::now();
    let d = fill_vector(&seg0, &seg0.zones.iter().map(|z| (!z.outside).then_some([1, 2, 3, 255])).collect::<Vec<_>>());
    println!("aplats vectoriels (contours) : {:?}", t.elapsed());
    let t = Instant::now();
    let _ = d.render(w, h);
    println!("rendu des formes : {:?}", t.elapsed());
    // Tout le coloriage : 1 clé, 12 images.
    let d = Demande {
        cadres: frames
            .iter()
            .zip(&lines)
            .enumerate()
            .map(|(f, (s, l))| coloriage::Cadre { cel: f as u64 + 1, version: 0, ligne: l.clone(), cle: (f == 0).then(|| paint(w, h, s)), cible: f != 0 })
            .collect(),
        parametres: Parametres::default(),
    };
    let cache = ZoneCache::new(1 << 30);
    let t = Instant::now();
    let mut doubts = 0;
    let bilan = colorier(&d, &cache, &SansConseil, &AtomicBool::new(false), &|_, _| {}, &mut |img| {
        let _ = fill_raster(&img.seg, &img.couleurs);
        doubts += img.doutes.len();
    })
    .unwrap();
    let all = t.elapsed();
    println!("coloriage de 12 images ({} zones coloriées, {doubts} doutes) : {all:?}, soit {:?} par image", bilan.zones_coloriees, all / 12);
    // Relevé du plan sur le fil de l'interface : le démarrage ne bloque rien.
    let ctx = egui::Context::default();
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(100);
    let mut line = Layer::new("Trait", LayerKind::Bitmap);
    let mut colour = Layer::new("Couleur", LayerKind::Bitmap);
    for f in 0..100 {
        let c = doc.add_cel(Cel::bitmap(lines[f % 13].clone()));
        line.set(f, Some(c));
    }
    let key = doc.add_cel(Cel::bitmap(paint(w, h, &frames[0])));
    colour.set(0, Some(key));
    doc.panels[0].layers.extend([colour, line]);
    let mut app = App::sans_fenetre(&ctx, None);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 1;
    let t = Instant::now();
    let plan = app.coloriage_plan().unwrap();
    println!("relevé du plan (100 images, sur le fil de l'interface) : {:?} ({} images à colorier)", t.elapsed(), plan.targets);
    let t = Instant::now();
    app.start_coloriage(&ctx);
    println!("démarrage de la tâche (fil de l'interface) : {:?}", t.elapsed());
    app.finish_coloriage_blocking();
    println!("{}", app.status);
}
