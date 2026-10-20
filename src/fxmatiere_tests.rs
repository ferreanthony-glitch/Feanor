//! Effets de matière (`fxmatiere.rs`) : liseré, trait sec, encre qui bave, dégradé le long du trait,
//! changer de plume. Exactitude de la distance au bord, largeur du liseré, déterminisme (un fil ou plusieurs,
//! mêmes réglages, mêmes pixels), réglages extrêmes, clés, enregistrement (version 32), calque vectoriel ou
//! bitmap, dessin dérivé en arrière-plan, écran = export, « Appliquer pour de bon » et son annulation.

use eframe::egui::Color32;

use crate::document::{Document, LayerKind};
use crate::fx::{self, Effect, EffectKind, FxStep};
use crate::fxexact_tests::differing;
use crate::fxmatiere::{self as m, distance, FAR};
use crate::fxtrait::{self as t};
use crate::fxtrait_tests::{drawing_doc, effect, render_with, six_lines, smooth_stroke, steps};
use crate::raster::{Raster, TILE};
use crate::render::{self, PlanItem};
use crate::vector::{Element, Trace, VPoint, VectorDrawing};

const W: usize = 280;
const H: usize = 220;

// ------------------------------------------------------------------ outils

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

fn black(a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(0, 0, 0, a)
}

/// Une feuille `w` × `h` avec un rectangle plein [x0, x1) × [y0, y1) de la couleur `c`.
fn rect(w: usize, h: usize, x0: usize, y0: usize, x1: usize, y1: usize, c: Color32) -> Raster {
    let mut r = Raster::new(w, h);
    for y in y0..y1 {
        for x in x0..x1 {
            put(&mut r, x, y, c);
        }
    }
    r
}

fn total_alpha(r: &Raster) -> u64 {
    (0..r.height).map(|y| (0..r.width).map(|x| r.get(x, y).a() as u64).sum::<u64>()).sum()
}

/// Le plus grand écart de canal entre deux feuilles de même taille.
fn max_diff(a: &Raster, b: &Raster) -> u8 {
    let mut worst = 0u8;
    for y in 0..a.height {
        for x in 0..a.width {
            let (p, q) = (a.get(x, y).to_array(), b.get(x, y).to_array());
            for k in 0..4 {
                worst = worst.max(p[k].abs_diff(q[k]));
            }
        }
    }
    worst
}

/// Un hasard reproductible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}

fn run_serial<R>(f: impl FnOnce() -> R) -> R {
    crate::parallele::EN_SERIE.with(|x| x.set(true));
    let out = f();
    crate::parallele::EN_SERIE.with(|x| x.set(false));
    out
}

/// Une ligne horizontale épaisse (rayon 3) à `y`, de 30 à 250.
fn one_line(y: f32) -> VectorDrawing {
    VectorDrawing { elements: vec![smooth_stroke(&[(30.0, y, 3.0), (250.0, y, 3.0)])] }
}

/// Les largeurs (pixels d'encre au-dessus du seuil) de la colonne `x`.
fn thickness(r: &Raster, x: usize) -> usize {
    (0..r.height).filter(|&y| r.get(x, y).a() > 24).count()
}

// ------------------------------------------------------------------ la distance exacte

#[test]
fn the_distance_transform_is_exact_whatever_the_sites() {
    let mut rng = Lcg(5);
    for (w, h) in [(47usize, 31usize), (1, 17), (23, 1), (64, 64), (5, 5)] {
        for density in [0.0f32, 0.004, 0.03, 0.25, 1.0] {
            let site: Vec<bool> = (0..w * h).map(|_| rng.next() < density).collect();
            let d = distance(w, h, &site);
            let sites: Vec<(i64, i64)> = (0..w * h).filter(|&i| site[i]).map(|i| ((i % w) as i64, (i / w) as i64)).collect();
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let best = sites.iter().map(|&(sx, sy)| ((x as i64 - sx).pow(2) + (y as i64 - sy).pow(2)) as f32).fold(f32::MAX, f32::min);
                    if sites.is_empty() {
                        assert!(d.v[i].0 > FAR * 0.5 && d.v[i].1 == u32::MAX, "aucun site : très loin");
                        continue;
                    }
                    assert_eq!(d.v[i].0, best, "({x}, {y}) dans {w} x {h}, densité {density}");
                    let s = d.v[i].1 as usize;
                    assert!(site[s], "le site trouvé est bien un site");
                    let (sx, sy) = ((s % w) as i64, (s / w) as i64);
                    assert_eq!(((x as i64 - sx).pow(2) + (y as i64 - sy).pow(2)) as f32, best, "et c'est un des plus proches");
                }
            }
        }
    }
}

// ------------------------------------------------------------------ liseré

fn outline_steps(set: &[(usize, f32)]) -> Vec<FxStep> {
    steps(EffectKind::Outline, set, 0)
}

#[test]
fn the_outline_is_as_wide_as_asked_around_the_ink_and_leaves_the_ink_alone() {
    // Un seul pixel d'encre : le liseré est un disque exact (distance euclidienne, pas un carré ni un flou).
    let src = rect(100, 100, 50, 50, 51, 51, black(255));
    let out = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 5.0), (m::OUTLINE_SOFT, 1.0)]));
    let mut partial = 0;
    for y in 30..70usize {
        for x in 30..70usize {
            let r = ((x as f32 - 50.0).powi(2) + (y as f32 - 50.0).powi(2)).sqrt();
            let c = out.get(x, y);
            if (x, y) == (50, 50) {
                assert_eq!(c, black(255), "l'encre n'a pas bougé");
            } else if r <= 5.0 + 1e-3 {
                assert_eq!(c, Color32::WHITE, "({x}, {y}) à {r:.2} px : dans le liseré");
            } else if r >= 6.0 {
                assert_eq!(c.a(), 0, "({x}, {y}) à {r:.2} px : hors du liseré");
            } else {
                partial += 1;
                assert!(c.a() > 0 && c.a() < 255, "({x}, {y}) à {r:.2} px : le bord est adouci");
            }
        }
    }
    assert!(partial > 8, "{partial} pixels de bord");
    // Plus large, plus d'encre ; la couleur et l'opacité sont celles du réglage.
    let mass = |w: f32| total_alpha(&fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, w)])));
    assert!(mass(2.0) < mass(4.0) && mass(4.0) < mass(9.0));
    let red = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 5.0), (m::OUTLINE_RGB, 200.0), (m::OUTLINE_RGB + 1, 30.0), (m::OUTLINE_RGB + 2, 20.0)]));
    assert_eq!(red.get(53, 50), Color32::from_rgb(200, 30, 20));
    let half = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 5.0), (m::OUTLINE_OPACITY, 50.0)]));
    assert!((127..=128).contains(&half.get(53, 50).a()), "{}", half.get(53, 50).a());
    // La douceur étale le bord : plus de pixels à moitié couverts.
    let soft = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 8.0), (m::OUTLINE_SOFT, 12.0)]));
    let hard = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 8.0), (m::OUTLINE_SOFT, 0.0)]));
    let halves = |r: &Raster| (0..100).flat_map(|y| (0..100).map(move |x| (x, y))).filter(|&(x, y)| (20..235).contains(&r.get(x, y).a())).count();
    assert!(halves(&soft) > halves(&hard) * 2, "{} contre {}", halves(&soft), halves(&hard));
    // Rien à faire : largeur nulle, opacité nulle, feuille vide.
    assert!(outline_steps(&[(m::OUTLINE_WIDTH, 0.0)]).is_empty() && outline_steps(&[(m::OUTLINE_OPACITY, 0.0)]).is_empty());
    let empty = Raster::new(40, 40);
    assert_eq!(differing(&fx::apply(&empty, &outline_steps(&[])), &empty), 0);
}

#[test]
fn the_outline_goes_behind_the_stroke_or_makes_a_ring_around_it() {
    // Un trait à moitié transparent : « derrière » laisse voir le liseré à travers, « en anneau » non.
    let src = rect(80, 80, 30, 30, 50, 50, black(128));
    let behind = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 4.0), (m::OUTLINE_BEHIND, 1.0)]));
    let ring = fx::apply(&src, &outline_steps(&[(m::OUTLINE_WIDTH, 4.0), (m::OUTLINE_BEHIND, 0.0)]));
    assert!(behind.get(40, 40).g() > 100 && behind.get(40, 40).a() == 255, "derrière : {:?}", behind.get(40, 40));
    assert_eq!(ring.get(40, 40), black(128), "en anneau : l'encre est telle quelle");
    // Dehors, les deux se ressemblent.
    assert_eq!(behind.get(53, 40), ring.get(53, 40));
    assert_eq!(ring.get(53, 40), Color32::WHITE);
    // Derrière : le liseré couvre aussi les 400 pixels de l'encre (255 − 128 de plus chacun) ; en anneau, non.
    assert_eq!(total_alpha(&outline_only(&behind, &src)), total_alpha(&outline_only(&ring, &src)) + 127 * 400);
}

/// Ce que `out` ajoute à `src` (pour compter l'encre du liseré seul).
fn outline_only(out: &Raster, src: &Raster) -> Raster {
    let mut r = Raster::new(out.width, out.height);
    for y in 0..out.height {
        for x in 0..out.width {
            let (a, b) = (out.get(x, y).a(), src.get(x, y).a());
            if a > b {
                put(&mut r, x, y, black(a - b));
            }
        }
    }
    r
}

#[test]
fn the_outline_works_the_same_on_a_vector_layer_and_on_a_bitmap() {
    let d = VectorDrawing { elements: six_lines().elements.into_iter().take(3).collect() };
    let raster = d.render(W, H);
    let s = outline_steps(&[(m::OUTLINE_WIDTH, 6.0), (m::OUTLINE_RGB, 255.0), (m::OUTLINE_RGB + 2, 0.0)]);
    let on_bitmap = fx::apply_with(&raster, None, &s, 0);
    let on_vector = fx::apply_with(&raster, Some(&d), &s, 0);
    assert_eq!(differing(&on_bitmap, &on_vector), 0);
    assert!(total_alpha(&on_bitmap) > total_alpha(&raster) * 2);
}

// ------------------------------------------------------------------ encre qui bave

fn bleed_steps(set: &[(usize, f32)]) -> Vec<FxStep> {
    steps(EffectKind::Bleed, set, 0)
}

/// Combien de pixels au-dessus de la barre (rangée `top`) ont de l'encre dans la colonne `x`.
fn reach_above(r: &Raster, x: usize, top: usize) -> usize {
    (0..top).rev().take_while(|&y| r.get(x, y).a() > 0).count()
}

#[test]
fn the_ink_bleeds_a_little_around_the_stroke_and_the_stroke_stays_readable() {
    let bar = rect(200, 100, 20, 40, 180, 52, black(255));
    let s = bleed_steps(&[(m::BLEED_SPREAD, 4.0), (m::BLEED_IRREG, 0.0), (m::BLEED_DARK, 0.0), (m::BLEED_HALO, 55.0)]);
    let out = fx::apply(&bar, &s);
    // Le cœur du trait n'a pas bougé.
    for y in 41..51 {
        for x in 30..170 {
            assert_eq!(out.get(x, y), black(255), "({x}, {y})");
        }
    }
    // Sans irrégularité, la bavure a la même portée partout : 4 pixels, de plus en plus pâles.
    for x in (40..160).step_by(7) {
        assert_eq!(reach_above(&out, x, 40), 4, "colonne {x}");
        assert!(out.get(x, 39).a() > out.get(x, 38).a() && out.get(x, 38).a() > out.get(x, 36).a());
    }
    assert!((90..140).contains(&out.get(100, 39).a()), "près du trait : {}", out.get(100, 39).a());
    assert_eq!(out.get(100, 35).a(), 0, "au-delà de l'étalement : rien");
    // La couleur de la bavure est celle de l'encre.
    let red = rect(200, 100, 20, 40, 180, 52, Color32::from_rgb(200, 40, 40));
    let r = fx::apply(&red, &s);
    let c = r.get(100, 39);
    assert!(c.r() as u32 > c.g() as u32 * 3 && c.a() > 0, "{c:?}");
    // Avec de l'irrégularité, la portée change d'une colonne à l'autre ; avec une autre graine, ailleurs.
    let irregular = bleed_steps(&[(m::BLEED_SPREAD, 6.0), (m::BLEED_IRREG, 100.0), (m::BLEED_DARK, 0.0)]);
    let a = fx::apply(&bar, &irregular);
    let reaches: Vec<usize> = (30..170).map(|x| reach_above(&a, x, 40)).collect();
    let distinct = reaches.iter().collect::<std::collections::BTreeSet<_>>().len();
    assert!(distinct >= 3 && *reaches.iter().max().unwrap() <= 12 && *reaches.iter().max().unwrap() >= 4, "{reaches:?}");
    let other = fx::apply(&bar, &bleed_steps(&[(m::BLEED_SPREAD, 6.0), (m::BLEED_IRREG, 100.0), (m::BLEED_DARK, 0.0), (m::BLEED_SEED, 2.0)]));
    assert!(differing(&a, &other) > 100, "une autre graine : une autre bavure");
    assert_eq!(differing(&a, &fx::apply(&bar, &irregular)), 0, "mêmes réglages, mêmes pixels");
    // Rien à faire.
    assert!(bleed_steps(&[(m::BLEED_SPREAD, 0.0), (m::BLEED_DARK, 0.0)]).is_empty());
}

#[test]
fn the_ink_edge_darkens_and_frays_without_eating_the_core() {
    let red = rect(200, 100, 20, 40, 180, 52, Color32::from_rgb(200, 40, 40));
    let out = fx::apply(&red, &bleed_steps(&[(m::BLEED_SPREAD, 0.0), (m::BLEED_DARK, 100.0), (m::BLEED_IRREG, 0.0)]));
    // Le bord (première rangée de l'encre) est plus foncé que le milieu, qui est intact.
    assert!(out.get(100, 40).r() < 150 && out.get(100, 40).a() == 255, "{:?}", out.get(100, 40));
    assert_eq!(out.get(100, 46), Color32::from_rgb(200, 40, 40));
    assert!(out.get(100, 41).r() > out.get(100, 40).r() && out.get(100, 41).r() < 200, "le fond du bord est progressif");
    // L'irrégularité effrite un peu le bord, jamais le cœur.
    let frayed = fx::apply(&red, &bleed_steps(&[(m::BLEED_SPREAD, 0.0), (m::BLEED_DARK, 1.0), (m::BLEED_IRREG, 100.0)]));
    assert!(frayed.get(100, 46) == red.get(100, 46) && total_alpha(&frayed) < total_alpha(&red));
    assert!(bleed_steps(&[(m::BLEED_SPREAD, 0.0), (m::BLEED_DARK, 100.0)]).len() == 1, "foncer le bord suffit à faire quelque chose");
}

// ------------------------------------------------------------------ trait sec

fn dry_steps(set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    steps(EffectKind::Dry, set, frame)
}

/// Les trous du trait horizontal à `y` : suites de colonnes (entre `x0` et `x1`) dont la couverture est
/// sous la moitié du trait d'origine.
fn gaps(r: &Raster, y: usize, x0: usize, x1: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for x in x0..x1 {
        if r.get(x, y).a() < 200 {
            match out.last_mut() {
                Some(last) if last.1 + 1 == x => last.1 = x,
                _ => out.push((x, x)),
            }
        }
    }
    out
}

#[test]
fn the_dry_stroke_breaks_each_vector_stroke_along_its_length() {
    let d = one_line(100.0);
    let plain = d.render(W, H);
    let set = [(m::DRY_FREQ, 4.0), (m::DRY_LENGTH, 10.0), (m::DRY_DEPTH, 100.0), (m::DRY_ROUGH, 0.0)];
    let dry = render_with(&d, &dry_steps(&set, 0));
    let holes = gaps(&dry, 100, 35, 245);
    // 4 trous pour 100 pixels sur 210 pixels de trait : une dizaine, longs de quelques pixels à une quinzaine.
    assert!((3..=18).contains(&holes.len()), "{} trous : {holes:?}", holes.len());
    assert!(holes.iter().all(|&(a, b)| b - a < 40), "{holes:?}");
    let (m0, m1) = (total_alpha(&plain), total_alpha(&dry));
    assert!(m1 < m0 * 92 / 100 && m1 > m0 * 30 / 100, "{m1} contre {m0}");
    // Le hasard est fixé par la graine, et chaque trait a le sien (rang dans le dessin).
    assert_eq!(differing(&dry, &render_with(&d, &dry_steps(&set, 0))), 0);
    let mut seeded = set.to_vec();
    seeded.push((m::DRY_SEED, 2.0));
    assert!(differing(&dry, &render_with(&d, &dry_steps(&seeded, 0))) > 50);
    let two = VectorDrawing { elements: vec![smooth_stroke(&[(30.0, 60.0, 3.0), (250.0, 60.0, 3.0)]), smooth_stroke(&[(30.0, 140.0, 3.0), (250.0, 140.0, 3.0)])] };
    let r = render_with(&two, &dry_steps(&set, 0));
    assert_ne!(gaps(&r, 60, 35, 245), gaps(&r, 140, 35, 245), "deux traits identiques n'ont pas les mêmes trous");
    // Plus de trous pour 100 px, plus de matière en moins ; profondeur nulle : aucun trou.
    let mass = |freq: f32| total_alpha(&render_with(&d, &dry_steps(&[(m::DRY_FREQ, freq), (m::DRY_LENGTH, 10.0), (m::DRY_ROUGH, 0.0)], 0)));
    assert!(mass(2.0) > mass(8.0) && mass(8.0) > mass(20.0), "{} {} {}", mass(2.0), mass(8.0), mass(20.0));
    assert!(dry_steps(&[(m::DRY_DEPTH, 0.0), (m::DRY_ROUGH, 0.0)], 0).is_empty() && dry_steps(&[(m::DRY_FREQ, 0.0), (m::DRY_ROUGH, 0.0)], 0).is_empty());
}

#[test]
fn the_dry_edge_frays_in_streaks_along_the_stroke() {
    let d = VectorDrawing { elements: vec![smooth_stroke(&[(30.0, 100.0, 7.0), (250.0, 100.0, 7.0)])] };
    let plain = d.render(W, H);
    let rough = render_with(&d, &dry_steps(&[(m::DRY_FREQ, 0.0), (m::DRY_ROUGH, 100.0)], 0));
    // Le bord perd de la matière, le centre reste.
    let edge = |r: &Raster| (50..230).map(|x| r.get(x, 94).a() as u32 + r.get(x, 106).a() as u32).sum::<u32>();
    let centre = |r: &Raster| (50..230).map(|x| r.get(x, 100).a() as u32).sum::<u32>();
    assert!(edge(&rough) < edge(&plain) * 80 / 100, "{} contre {}", edge(&rough), edge(&plain));
    assert!(centre(&rough) > centre(&plain) * 95 / 100, "{} contre {}", centre(&rough), centre(&plain));
    // Des stries : le long d'une rangée du bord, l'encre va et vient (des plages, pas du bruit à chaque pixel).
    let runs = (50..230).filter(|&x| (rough.get(x, 94).a() > 100) != (rough.get(x + 1, 94).a() > 100)).count();
    assert!((2..=100).contains(&runs), "{runs} changements");
    // Un tirage par rang : une autre graine, un autre bord.
    assert!(differing(&rough, &render_with(&d, &dry_steps(&[(m::DRY_FREQ, 0.0), (m::DRY_ROUGH, 100.0), (m::DRY_SEED, 7.0)], 0))) > 30);
}

#[test]
fn the_dry_stroke_can_be_redrawn_every_n_frames_like_the_boil() {
    let d = one_line(100.0);
    let at = |frame: usize, every: f32| render_with(&d, &dry_steps(&[(m::DRY_FREQ, 6.0), (m::DRY_DEPTH, 100.0), (m::DRY_EVERY, every), (m::DRY_VARIANTS, 3.0)], frame));
    // Toutes les 3 images : trois images pour le même tirage, puis le suivant, et on revient au bout de neuf.
    let (f0, f1, f2, f3) = (at(0, 3.0), at(1, 3.0), at(2, 3.0), at(3, 3.0));
    assert_eq!(differing(&f0, &f1), 0);
    assert_eq!(differing(&f0, &f2), 0);
    assert!(differing(&f0, &f3) > 50, "le tirage suivant");
    assert_eq!(differing(&f0, &at(9, 3.0)), 0);
    // Jamais (0) : toujours le même, quelle que soit l'image ; et un seul dessin dérivé.
    assert_eq!(differing(&at(0, 0.0), &at(7, 0.0)), 0);
    let id = |frame: usize, every: f32| fx::fx_id(5, &dry_steps(&[(m::DRY_EVERY, every)], frame));
    assert_eq!(id(0, 0.0), id(40, 0.0));
    assert_eq!(id(0, 3.0), id(2, 3.0));
    assert_ne!(id(2, 3.0), id(3, 3.0));
}

#[test]
fn the_dry_stroke_on_a_bitmap_is_a_simpler_version_with_edge_noise_and_hollow_patches() {
    let bar = rect(200, 100, 20, 40, 180, 52, black(255));
    let edge_only = fx::apply(&bar, &dry_steps(&[(m::DRY_FREQ, 0.0), (m::DRY_ROUGH, 100.0)], 0));
    let row = |r: &Raster, y: usize| (30..170).filter(|&x| r.get(x, y).a() > 128).count();
    assert!(row(&edge_only, 40) < row(&bar, 40) * 85 / 100, "le bord s'effrite : {} sur {}", row(&edge_only, 40), row(&bar, 40));
    assert_eq!(row(&edge_only, 46), row(&bar, 46), "le milieu reste");
    assert_eq!(edge_only.get(100, 46), black(255));
    let set = [(m::DRY_FREQ, 3.0), (m::DRY_LENGTH, 20.0), (m::DRY_DEPTH, 100.0), (m::DRY_ROUGH, 0.0)];
    let holes = fx::apply(&bar, &dry_steps(&set, 0));
    assert!(total_alpha(&holes) < total_alpha(&bar) * 90 / 100 && total_alpha(&holes) > total_alpha(&bar) / 10, "{} contre {}", total_alpha(&holes), total_alpha(&bar));
    let again = fx::apply(&bar, &dry_steps(&set, 0));
    assert_eq!(differing(&holes, &again), 0);
    let seeded = fx::apply(&bar, &dry_steps(&[set[0], set[1], set[2], set[3], (m::DRY_SEED, 3.0)], 0));
    assert!(differing(&holes, &seeded) > 100);
}

// ------------------------------------------------------------------ dégradé le long du trait

fn grad_steps(set: &[(usize, f32)], frame: usize) -> Vec<FxStep> {
    steps(EffectKind::StrokeGradient, set, frame)
}

fn rgb(c: Color32) -> [i32; 3] {
    [c.r() as i32, c.g() as i32, c.b() as i32]
}

#[test]
fn the_gradient_changes_the_colour_from_the_start_to_the_end_of_each_stroke_and_keeps_the_shape() {
    let d = one_line(100.0);
    let plain = d.render(W, H);
    let g = render_with(&d, &grad_steps(&[], 0));
    // Rouge (224, 64, 48) au début, bleu (48, 104, 224) à la fin, le mélange entre les deux.
    let (start, mid, end) = (rgb(g.get(40, 100)), rgb(g.get(140, 100)), rgb(g.get(240, 100)));
    assert!(start[0] > 190 && start[2] < 90, "{start:?}");
    assert!(end[0] < 90 && end[2] > 190, "{end:?}");
    assert!(mid[0] > 90 && mid[0] < 190 && mid[2] > 90 && mid[2] < 190, "{mid:?}");
    // La forme est intacte : la couverture est celle du trait d'origine, pixel pour pixel.
    for y in 0..H {
        for x in 0..W {
            assert_eq!(g.get(x, y).a(), plain.get(x, y).a(), "({x}, {y})");
        }
    }
    // Mélange à 0 : rien à faire ; à 50 %, entre l'encre d'origine (20, 20, 20) et le dégradé.
    assert!(grad_steps(&[(m::GRAD_MIX, 0.0)], 0).is_empty());
    let half = render_with(&d, &grad_steps(&[(m::GRAD_MIX, 50.0)], 0));
    let (a, b) = (rgb(half.get(40, 100)), start);
    assert!(a[0] > 20 && a[0] < b[0] && (a[0] - (20 + b[0]) / 2).abs() < 8, "{a:?} {b:?}");
}

#[test]
fn the_gradient_can_run_across_the_whole_drawing_in_drawing_order() {
    let d = six_lines();
    let whole = render_with(&d, &grad_steps(&[(m::GRAD_WHOLE, 1.0)], 0));
    let red_of_line = |r: &Raster, i: usize| rgb(r.get(120, 25 + 30 * i))[0];
    let reds: Vec<i32> = (0..6).map(|i| red_of_line(&whole, i)).collect();
    assert!(reds.windows(2).all(|w| w[0] > w[1]), "du premier au dernier trait, le rouge baisse : {reds:?}");
    assert!(reds[0] > 190 && reds[5] < 90, "{reds:?}");
    // Par trait, les six traits sont pareils.
    let each = render_with(&d, &grad_steps(&[], 0));
    let reds: Vec<i32> = (0..6).map(|i| red_of_line(&each, i)).collect();
    assert!(reds.iter().all(|r| (r - reds[0]).abs() <= 1), "{reds:?}");
    // Trois couleurs : le milieu du trait prend la deuxième.
    let three = render_with(&one_line(100.0), &grad_steps(&[(m::GRAD_COUNT, 3.0), (m::GRAD_COLOR, 255.0), (m::GRAD_COLOR + 1, 0.0), (m::GRAD_COLOR + 2, 0.0), (m::GRAD_COLOR + 3, 0.0), (m::GRAD_COLOR + 4, 255.0), (m::GRAD_COLOR + 5, 0.0)], 0));
    let c = rgb(three.get(140, 100));
    assert!(c[1] > 200 && c[0] < 60 && c[2] < 60, "{c:?}");
}

#[test]
fn the_gradient_scrolls_with_the_frames_and_loops() {
    let d = one_line(100.0);
    let set = [(m::GRAD_SCROLL, 10.0), (m::GRAD_LOOP, 1.0), (m::GRAD_COUNT, 3.0)];
    let at = |frame: usize| render_with(&d, &grad_steps(&set, frame));
    assert!(differing(&at(0), &at(3)) > 100, "le dégradé défile");
    assert!(max_diff(&at(0), &at(10)) <= 1, "un tour de dégradé en dix images, en boucle : la même image");
    // Sans défilement, un seul dessin dérivé pour toutes les images ; avec, un par image.
    let id = |scroll: f32, frame: usize| fx::fx_id(7, &grad_steps(&[(m::GRAD_SCROLL, scroll)], frame));
    assert_eq!(id(0.0, 0), id(0.0, 30));
    assert_ne!(id(10.0, 0), id(10.0, 1));
    // Sens inverse : l'image k à contre-sens est celle de l'image −k (modulo la boucle).
    let back = render_with(&d, &grad_steps(&[(m::GRAD_SCROLL, -10.0), (m::GRAD_LOOP, 1.0), (m::GRAD_COUNT, 3.0)], 7));
    assert!(max_diff(&back, &at(3)) <= 1);
}

// ------------------------------------------------------------------ changer de plume

fn nib_steps(set: &[(usize, f32)]) -> Vec<FxStep> {
    steps(EffectKind::Nib, set, 0)
}

/// Un trait horizontal lissé de `n` points, de rayon 3, avec des appuis qui montent puis redescendent.
fn pressed_stroke(y: f32) -> Element {
    let n = 45usize;
    let points: Vec<VPoint> = (0..n).map(|i| VPoint { x: 30.0 + 220.0 * i as f32 / (n - 1) as f32, y, r: 3.0 }).collect();
    let pressure: Vec<f32> = (0..n).map(|i| 0.1 + 0.9 * (std::f32::consts::PI * i as f32 / (n - 1) as f32).sin()).collect();
    let time: Vec<f64> = (0..n).map(|i| i as f64 / 90.0).collect();
    Element::Stroke { points, color: [30, 60, 160], opacity: 1.0, hardness: 0.9, brush: None, trace: Trace { pressure, time, ..Default::default() } }
}

#[test]
fn another_nib_redraws_the_strokes_with_the_same_path_pressure_and_colour_without_touching_them() {
    crate::fxcache::monde::ecrire();
    let d = VectorDrawing { elements: vec![pressed_stroke(100.37)] };
    let before = d.clone();
    let plain = d.render(W, H);
    let s = nib_steps(&[]);
    let nibbed = render_with(&d, &s);
    assert_eq!(d, before, "les traits d'origine n'ont pas changé");
    assert!(differing(&nibbed, &plain) > 200, "une autre plume : un autre dessin");
    // Le même tracé : la masse d'encre est autour de la même ligne, entre les mêmes extrémités.
    let centre = |r: &Raster, x: usize| {
        let (mut s, mut n) = (0.0f32, 0.0f32);
        for y in 0..H {
            let a = r.get(x, y).a() as f32;
            s += a * (y as f32 + 0.5);
            n += a;
        }
        s / n.max(1.0)
    };
    for x in [60usize, 140, 220] {
        assert!((centre(&nibbed, x) - centre(&plain, x)).abs() < 3.0, "colonne {x}");
    }
    let (first, last) = ((0..W).find(|&x| thickness(&nibbed, x) > 0).unwrap(), (0..W).rev().find(|&x| thickness(&nibbed, x) > 0).unwrap());
    // (Aux bouts l'appui est faible : la plume y est fine, le trait y commence un peu plus loin.)
    assert!((first as i32 - 30).abs() < 35 && (last as i32 - 250).abs() < 35, "{first}..{last}");
    // La couleur du trait, la pression (plus épais au milieu qu'au bout).
    let c = nibbed.get(140, 100);
    assert!(c.a() > 100 && (c.b() as i32) > (c.r() as i32) + 40, "{c:?}");
    assert!(thickness(&nibbed, 140) > thickness(&nibbed, 45) + 2, "{} contre {}", thickness(&nibbed, 140), thickness(&nibbed, 45));
    // Deux calculs : les mêmes pixels ; la taille de la plume compte.
    assert_eq!(differing(&nibbed, &render_with(&d, &s)), 0);
    let big = render_with(&d, &nib_steps(&[(m::NIB_SIZE, 200.0)]));
    assert!(thickness(&big, 140) > thickness(&nibbed, 140) + 3, "{} contre {} ; en 45 : {} et {}", thickness(&big, 140), thickness(&nibbed, 140), thickness(&big, 45), thickness(&nibbed, 45));
    // Une plume introuvable : l'effet ne fait rien.
    assert_eq!(differing(&render_with(&d, &nib_steps(&[(m::NIB_ID, 1.0)])), &plain), 0);
    // Les zones remplies ne sont pas des traits : elles restent.
    let mut with_fill = d.clone();
    with_fill.elements.insert(0, Element::Fill { contours: vec![vec![[40.0, 150.0], [100.0, 150.0], [100.0, 200.0], [40.0, 200.0]]], color: [200, 30, 30], opacity: 1.0 });
    assert_eq!(render_with(&with_fill, &s).get(60, 170), with_fill.render(W, H).get(60, 170));
}

#[test]
fn every_library_brush_works_as_a_nib_and_a_redrawn_stroke_is_replayed_once() {
    crate::fxcache::monde::ecrire();
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    let names: Vec<String> = lib.presets.iter().filter(|p| p.target == crate::brushtex::Target::Brush).map(|p| p.name.clone()).collect();
    assert!(names.len() >= 6, "{names:?}");
    let d = VectorDrawing { elements: vec![pressed_stroke(120.71)] };
    let plain = d.render(W, H);
    for name in ["Encre", "Feutre", "Fusain", "Craie", "Gouache", "Pinceau rond"] {
        let id = m::name_id(name);
        assert!(m::nib_name(id).is_some(), "{name} est publiée par la bibliothèque");
        let out = render_with(&d, &nib_steps(&[(m::NIB_ID, id as f32)]));
        assert!(total_alpha(&out) > 2000 && differing(&out, &plain) > 100, "{name}");
    }
    // Chaque plume a un numéro à elle ; la calligraphique, celle de SHOT, est toujours là.
    let ids: std::collections::BTreeSet<u32> = names.iter().map(|n| m::name_id(n)).collect();
    assert_eq!(ids.len(), names.len(), "pas de collision de numéros");
    assert_eq!(m::nib_choices(&names)[0].1, m::CALLIGRAPHY);
    // Rejeu : une fois par trait et par plume, pas à chaque image (un trait neuf, jamais vu).
    let fresh = VectorDrawing { elements: vec![pressed_stroke(60.911)] };
    let s = nib_steps(&[(m::NIB_ID, m::name_id("Fusain") as f32)]);
    let (first, second) = run_serial(|| {
        let r0 = crate::vector::replay_count();
        let _ = render_with(&fresh, &s);
        let r1 = crate::vector::replay_count();
        let _ = render_with(&fresh, &s);
        (r1 - r0, crate::vector::replay_count() - r1)
    });
    assert!(first >= 1 && second == 0, "premier calcul : {first} rejeux, deuxième : {second}");
}

#[test]
fn applying_the_nib_for_good_gives_the_pictures_of_the_effect_and_keeps_fills_and_pressure() {
    crate::fxcache::monde::ecrire();
    let mut d = VectorDrawing { elements: vec![pressed_stroke(80.13), pressed_stroke(160.29)] };
    d.elements.insert(0, Element::Fill { contours: vec![vec![[120.0, 110.0], [180.0, 110.0], [180.0, 140.0], [120.0, 140.0]]], color: [10, 150, 60], opacity: 0.8 });
    let s = nib_steps(&[(m::NIB_ID, m::name_id("Feutre") as f32)]);
    let shown = render_with(&d, &s);
    let nib = m::NibCtx::resolve(&m::NibS::of(&s[0].values)).expect("plume");
    let mut baked = d.clone();
    assert_eq!(m::bake_nib(&mut baked, &nib), 2, "deux traits refaits, la zone remplie reste");
    assert!(matches!(&baked.elements[0], Element::Fill { .. }));
    for e in &baked.elements[1..] {
        let Element::Stroke { brush, trace, points, .. } = e else { panic!("un trait") };
        assert!(brush.is_some(), "le trait porte le réglage de la plume");
        assert_eq!(trace.pressure.len(), points.len(), "les appuis sont gardés");
    }
    assert_eq!(differing(&baked.render(W, H), &shown), 0, "le dessin refait est celui que l'effet montrait");
    // Et l'original n'a pas bougé.
    assert!(matches!(&d.elements[1], Element::Stroke { brush: None, .. }));
}

// ------------------------------------------------------------------ empilés et déterministes

#[test]
fn the_material_effects_stack_with_the_stroke_effects_and_stay_deterministic() {
    crate::fxcache::monde::ecrire();
    let mut elements = six_lines().elements;
    elements.push(smooth_stroke(&[(20.0, 200.0, 3.0), (120.0, 180.0, 5.0), (250.0, 205.0, 2.0)]));
    let d = VectorDrawing { elements };
    let stack = [
        effect(EffectKind::Write, &[(t::WRITE_PROGRESS, 85.0), (t::WRITE_SOFT, 6.0)]),
        effect(EffectKind::Nib, &[]),
        effect(EffectKind::Dry, &[(m::DRY_EVERY, 2.0)]),
        effect(EffectKind::StrokeGradient, &[(m::GRAD_SCROLL, 5.0), (m::GRAD_WHOLE, 1.0)]),
        effect(EffectKind::Breath, &[(t::BREATH_AMPLITUDE, 40.0)]),
        effect(EffectKind::Sketch, &[(t::SKETCH_PASSES, 2.0)]),
    ];
    let s = fx::steps_at(&stack, 5);
    assert_eq!(s.len(), 6);
    let a = render_with(&d, &s);
    assert_eq!(differing(&a, &render_with(&d, &s)), 0);
    assert_eq!(differing(&a, &run_serial(|| render_with(&d, &s))), 0, "un fil ou plusieurs");
    assert!(total_alpha(&a) > 1000);
    // Avec le liseré et l'encre, par-dessus (sur les pixels, dans l'ordre de la pile).
    let more = [s.clone(), steps(EffectKind::Outline, &[], 5), steps(EffectKind::Bleed, &[], 5)].concat();
    let raster = d.render(W, H);
    let out = fx::apply_with(&raster, Some(&d), &more, 0);
    assert_eq!(differing(&out, &fx::apply_with(&raster, Some(&d), &more, 0)), 0);
    assert!(total_alpha(&out) > total_alpha(&a));
}

#[test]
fn the_pixel_effects_do_not_depend_on_the_number_of_threads() {
    let d = VectorDrawing { elements: crate::fxtrait_tests::dense_drawing(120, 0, W.max(600), H.max(400)).elements };
    let raster = d.render(600, 400);
    let all = [steps(EffectKind::Outline, &[(m::OUTLINE_WIDTH, 7.0)], 0), steps(EffectKind::Bleed, &[], 0), steps(EffectKind::Dry, &[], 0)].concat();
    crate::parallele::set_plafond(1);
    let one = fx::apply_with(&raster, None, &all, 0);
    crate::parallele::set_plafond(0);
    let many = fx::apply_with(&raster, None, &all, 0);
    assert_eq!(differing(&one, &many), 0);
}

#[test]
fn extreme_and_broken_settings_never_panic_and_stay_inside_the_sheet() {
    crate::fxcache::monde::ecrire();
    let d = VectorDrawing { elements: six_lines().elements };
    let raster = d.render(W, H);
    for kind in m::KINDS {
        let params = kind.params();
        let lows: Vec<f32> = params.iter().map(|p| p.min).collect();
        let highs: Vec<f32> = params.iter().map(|p| p.max).collect();
        let mids: Vec<f32> = params.iter().map(|p| (p.min + p.max) / 2.0).collect();
        let defaults: Vec<f32> = params.iter().map(|p| p.default).collect();
        for values in [lows, highs, mids, defaults, vec![], vec![f32::NAN; 20], vec![f32::INFINITY, -1e9, 1e9, 3.0, -7.0, 0.0, 99.0, 5.0, 5.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]] {
            let step = FxStep { kind, values };
            let _ = kind.changes(&step.values);
            let out = fx::apply_with(&raster, Some(&d), std::slice::from_ref(&step), 0);
            assert_eq!((out.width, out.height), (W, H));
            let bare = fx::apply_with(&raster, None, std::slice::from_ref(&step), 0);
            assert_eq!((bare.width, bare.height), (W, H));
        }
    }
    // Un bitmap vide, un dessin vide, un trait réduit à un point.
    let empty = Raster::new(W, H);
    for kind in m::KINDS {
        let step = FxStep { kind, values: kind.params().iter().map(|p| p.default).collect() };
        let _ = fx::apply_with(&empty, Some(&VectorDrawing::default()), std::slice::from_ref(&step), 0);
    }
    let dot = VectorDrawing { elements: vec![smooth_stroke(&[(50.0, 50.0, 4.0), (50.0, 50.0, 4.0)])] };
    for kind in [EffectKind::Dry, EffectKind::StrokeGradient, EffectKind::Nib] {
        let _ = render_with(&dot, &steps(kind, &[], 0));
    }
}

// ------------------------------------------------------------------ calques vectoriels et bitmap

#[test]
fn the_gradient_and_the_nib_need_a_vector_layer_the_other_three_work_on_both() {
    let mut vector = crate::document::Layer::new("V", LayerKind::Vector);
    let mut bitmap = crate::document::Layer::new("B", LayerKind::Bitmap);
    for k in m::KINDS {
        assert!(k.is_trait() && k.family() == "Matière" && k.single());
        vector.fx.push(Effect::new(k));
        bitmap.fx.push(Effect::new(k));
    }
    let kinds = |l: &crate::document::Layer| fx::layer_steps(l, 0, None).iter().map(|s| s.kind).collect::<Vec<_>>();
    assert_eq!(kinds(&vector).len(), 5);
    assert_eq!(kinds(&bitmap), vec![EffectKind::Outline, EffectKind::Dry, EffectKind::Bleed], "un bitmap garde le liseré, le trait sec et l'encre");
    for (k, vector_only) in [(EffectKind::Outline, false), (EffectKind::Dry, false), (EffectKind::Bleed, false), (EffectKind::StrokeGradient, true), (EffectKind::Nib, true)] {
        assert_eq!(k.vector_only(), vector_only, "{k:?}");
        assert_eq!(crate::fxui::offer(k, LayerKind::Vector, false), crate::fxui::Offer::Enabled);
        assert_eq!(crate::fxui::offer(k, LayerKind::Vector, true), crate::fxui::Offer::Absent);
        match crate::fxui::offer(k, LayerKind::Bitmap, false) {
            crate::fxui::Offer::Enabled => assert!(!vector_only),
            crate::fxui::Offer::Disabled(why) => assert!(vector_only && why.contains("vectoriel") && why.len() > 60, "{k:?} : {why}"),
            other => panic!("{k:?} : {other:?}"),
        }
    }
    // Le calque qu'on édite se montre tel qu'il est : le trait sec se tait, pas le liseré (qui ne lit que les pixels).
    let editing: Vec<EffectKind> = fx::without_boil(fx::layer_steps(&vector, 0, None)).iter().map(|s| s.kind).collect();
    assert_eq!(editing, vec![EffectKind::Outline, EffectKind::Bleed]);
    // Les réglages enregistrés dans une pile : les noms sont français et distincts.
    let labels: std::collections::BTreeSet<&str> = m::KINDS.iter().map(|k| k.label()).collect();
    assert_eq!(labels.len(), 5);
}

#[test]
fn every_material_setting_has_a_hint_a_style_and_sound_bounds() {
    for kind in m::KINDS {
        assert!(!kind.hint().is_empty() && !m::icon(kind).is_empty() && m::kind_of_action(m::action(kind)) == Some(kind), "{kind:?}");
        let params = kind.params();
        assert_eq!(Effect::new(kind).values_at(0).len(), params.len());
        for (k, p) in params.iter().enumerate() {
            assert!(p.min <= p.default && p.default <= p.max, "{kind:?} / {}", p.name);
            if m::color_tail(kind, k) {
                continue;
            }
            assert!(!kind.param_hint(k).is_empty(), "{kind:?} / {} n'a pas de bulle", p.name);
        }
        // Les couleurs : un rouge, puis un vert et un bleu qui le suivent.
        for k in 0..params.len() {
            if m::color_start(kind, k).is_some() {
                assert!(m::color_tail(kind, k + 1) && m::color_tail(kind, k + 2) && !m::color_tail(kind, k + 3), "{kind:?} / {k}");
            }
        }
    }
    // L'interrupteur « derrière le trait » et le choix de la plume : leur style.
    assert_eq!(EffectKind::Outline.param_style(m::OUTLINE_BEHIND), fx::ParamStyle::Switch);
    assert_eq!(EffectKind::Nib.param_style(m::NIB_ID), fx::ParamStyle::Nib);
    assert_eq!(EffectKind::StrokeGradient.param_style(m::GRAD_COUNT), fx::ParamStyle::Integer);
    // La plume par défaut est celle de SHOT, qui existe toujours.
    assert_eq!(m::NibS::of(&[]).id, m::name_id(m::CALLIGRAPHY));
    assert!(m::nib_settings(m::name_id(m::CALLIGRAPHY)).is_some());
}

// ------------------------------------------------------------------ enregistrement

#[test]
fn the_material_effects_are_saved_with_the_layer_in_version_32_and_older_projects_open_unchanged() {
    assert!(crate::project::VERSION >= 32, "32 : les effets de matière");
    let mut doc = Document::new(64, 64, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let bit = doc.new_cel(LayerKind::Bitmap);
    doc.panels[0].layers[0].kind = LayerKind::Vector;
    doc.panels[0].layers[0].frames = vec![Some(id)];
    let mut stack: Vec<Effect> = m::KINDS.iter().map(|k| Effect::new(*k)).collect();
    // Des clés (une couleur, un défilement), un choix de plume, un interrupteur.
    stack[3].set_key(0, effect(EffectKind::StrokeGradient, &[(m::GRAD_SCROLL, 0.0)]).values);
    stack[3].set_key(12, effect(EffectKind::StrokeGradient, &[(m::GRAD_SCROLL, 20.0), (m::GRAD_COLOR, 12.0)]).values);
    stack[4].set_param(m::NIB_ID, m::name_id("Feutre") as f32);
    stack[0].set_param(m::OUTLINE_BEHIND, 0.0);
    doc.panels[0].layers[0].fx = stack.clone();
    // Un calque bitmap avec les trois effets qu'il accepte.
    let mut layer = crate::document::Layer::new("Bitmap", LayerKind::Bitmap);
    layer.frames = vec![Some(bit)];
    layer.fx = vec![stack[0].clone(), stack[1].clone(), stack[2].clone()];
    doc.panels[0].layers.push(layer);
    let dir = std::env::temp_dir().join(format!("shot-test-fxmatiere-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("M.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    for name in ["\"lisere\"", "\"trait_sec\"", "\"encre_bave\"", "\"degrade_trait\"", "\"plume\""] {
        assert!(text.contains(name), "{name} est écrit : {text}");
    }
    assert!(text.contains(&format!("\"version\": {}", crate::project::VERSION)));
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].fx, doc.panels[0].layers[0].fx, "mêmes réglages et mêmes clés");
    assert_eq!(back.panels[0].layers[1].fx, doc.panels[0].layers[1].fx);
    // Un projet de la version 31 (sans effet de matière) s'ouvre tel quel.
    let version = format!("\"version\": {}", crate::project::VERSION);
    let old = text
        .replace(&version, "\"version\": 31")
        .replace("\"lisere\"", "\"flou_gaussien\"")
        .replace("\"trait_sec\"", "\"flou_gaussien\"")
        .replace("\"encre_bave\"", "\"flou_gaussien\"")
        .replace("\"degrade_trait\"", "\"flou_gaussien\"")
        .replace("\"plume\"", "\"flou_gaussien\"");
    std::fs::write(&manifest, old).unwrap();
    assert!(crate::project::load(&manifest).is_ok());
    // Une version d'après est refusée, comme toujours : et la 31 refuserait la 32 (elle ne connaît pas ces effets).
    std::fs::write(&manifest, text.replace(&version, &format!("\"version\": {}", crate::project::VERSION + 1))).unwrap();
    assert!(crate::project::load(&manifest).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------ dessin dérivé, écran et export

fn bitmap_doc(raster: Raster, fx_stack: Vec<Effect>, frames: usize) -> Document {
    let mut doc = Document::new(raster.width, raster.height, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&id).unwrap().raster = raster;
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = LayerKind::Bitmap;
    layer.frames = vec![Some(id); frames];
    layer.fx = fx_stack;
    doc
}

fn item_of(doc: &Document, frame: usize) -> PlanItem {
    render::panel_plan_at(doc, 0, frame, None).items[0].clone()
}

#[test]
fn the_derived_drawing_is_computed_in_the_background_for_a_bitmap_too_and_the_export_is_the_screen() {
    crate::fxcache::monde::ecrire();
    let bar = rect(W, H, 40, 80, 220, 100, Color32::from_rgb(40, 40, 120));
    let doc = bitmap_doc(
        bar.clone(),
        vec![effect(EffectKind::Outline, &[(m::OUTLINE_WIDTH, 5.0)]), effect(EffectKind::Dry, &[(m::DRY_ROUGH, 80.0)]), effect(EffectKind::Bleed, &[])],
        3,
    );
    let item = item_of(&doc, 1);
    assert_eq!(item.fx.len(), 3);
    let first = fx::display(&doc, &item, 0).unwrap();
    assert!(!first.ready, "calculé en arrière-plan, jamais sur le fil de l'interface");
    crate::fxcache::settle();
    let shown = fx::display(&doc, &item, 0).unwrap();
    assert!(shown.ready && matches!(shown.cel, fx::Shown::Fx(_)));
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let direct = fx::apply_with(&doc.cels[&id].raster, None, &item.fx, 0);
    assert_eq!(differing(&shown.cel.raster, &direct), 0);
    // L'export (calcul exact) montre la même image que l'écran, et l'original n'a pas bougé.
    let image = render::composite_full(&doc, &render::panel_plan_at(&doc, 0, 1, None));
    assert_eq!(image, direct.to_flat());
    assert_eq!(doc.cels[&id].raster.to_flat(), bar.to_flat());
    assert!(total_alpha(&direct) > total_alpha(&bar));
    // Une vignette (calcul doux) montre la même image une fois le calcul fini.
    let plan = render::panel_plan_at(&doc, 0, 1, None);
    let want = render::composite_pixel(&doc, &plan, 100, 77);
    assert!(want.a() > 0, "à trois pixels au-dessus du trait : dans le liseré");
    crate::fxcache::settle();
    let (got, _) = crate::fxcache::soft_scope(|| render::composite_pixel(&doc, &plan, 100, 77));
    assert_eq!(got, want);
}

#[test]
fn a_vector_layer_with_every_effect_gives_the_same_image_on_screen_and_in_the_export() {
    crate::fxcache::monde::ecrire();
    let mut scroll = Effect::new(EffectKind::StrokeGradient);
    scroll.set_key(0, effect(EffectKind::StrokeGradient, &[(m::GRAD_SCROLL, 0.0), (m::GRAD_WHOLE, 1.0)]).values);
    scroll.set_key(10, effect(EffectKind::StrokeGradient, &[(m::GRAD_SCROLL, 8.0), (m::GRAD_WHOLE, 1.0)]).values);
    let doc = drawing_doc(six_lines(), vec![effect(EffectKind::Nib, &[]), effect(EffectKind::Dry, &[(m::DRY_EVERY, 2.0)]), scroll, effect(EffectKind::Outline, &[]), effect(EffectKind::Bleed, &[])], 11);
    let item = item_of(&doc, 6);
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    // Plusieurs images : chacune a son dessin dérivé (le dégradé défile), servi sans calcul sur le fil de l'interface.
    for frame in [0usize, 3, 6, 10] {
        let item = item_of(&doc, frame);
        let _ = fx::display(&doc, &item, 0).unwrap();
        crate::fxcache::settle();
        let shown = fx::display(&doc, &item, 0).unwrap();
        assert!(shown.ready, "image {frame}");
        let direct = fx::apply_with(&doc.cels[&id].raster, doc.cels[&id].vector.as_ref(), &item.fx, 0);
        assert_eq!(differing(&shown.cel.raster, &direct), 0, "image {frame} : l'écran est le calcul exact");
        assert_eq!(render::composite_full(&doc, &render::panel_plan_at(&doc, 0, frame, None)), direct.to_flat(), "image {frame} : l'export aussi");
    }
    assert!(item.fx.iter().all(|s| s.kind.is_trait()));
}

#[test]
fn a_stack_with_the_pixel_effects_stays_on_the_processor_and_the_vector_ones_split_off() {
    let doc = drawing_doc(six_lines(), Vec::new(), 1);
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let item = |fx: Vec<FxStep>| PlanItem { cel: id, opacity: 255, tint: None, transform: None, blend: Default::default(), fx, ctg: None, fond: None, pre: None };
    let blur = FxStep { kind: EffectKind::GaussianBlur, values: vec![6.0] };
    // Le liseré et l'encre n'ont pas de passe sur la carte graphique (décision de cette série) : tout le calcul de fond.
    for kind in [EffectKind::Outline, EffectKind::Bleed, EffectKind::Dry] {
        assert!(!crate::gpufx::supported(kind), "{kind:?}");
        assert!(!crate::gpu::gpu_fx_item(&doc, &item(steps(kind, &[], 0))), "{kind:?} seul");
    }
    let mixed = item([steps(EffectKind::Outline, &[], 0), vec![blur.clone()]].concat());
    assert!(!crate::gpu::gpu_fx_item(&doc, &mixed), "liseré + flou : l'ordre compte, tout sur le processeur");
    assert!(fx::hybrid_split(&mixed).is_none());
    // Le dégradé lit les traits : il passe d'abord, la carte fait le flou.
    let grad = item([steps(EffectKind::StrokeGradient, &[], 0), vec![blur.clone()]].concat());
    let (line, rest) = fx::hybrid_split(&grad).expect("pile mêlée");
    assert_eq!(line.fx.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![EffectKind::StrokeGradient]);
    assert_eq!(rest, vec![blur]);
    assert!(crate::gpu::gpu_fx_item(&doc, &grad));
}

// ------------------------------------------------------------------ copier-coller

#[test]
fn copy_and_paste_keep_one_material_effect_of_each_kind_and_skip_the_vector_ones_on_a_bitmap() {
    use crate::fxcopie::paste_onto;
    let clip: Vec<Effect> = m::KINDS.iter().map(|k| Effect::new(*k)).collect();
    let mut bitmap = Vec::new();
    let landed = paste_onto(&mut bitmap, &clip, false, 24, false);
    assert_eq!(landed.added, vec![EffectKind::Outline, EffectKind::Dry, EffectKind::Bleed]);
    assert_eq!(landed.vector_only_skipped, vec![EffectKind::StrokeGradient, EffectKind::Nib]);
    let mut vector = vec![Effect::new(EffectKind::Outline)];
    let landed = paste_onto(&mut vector, &clip, true, 24, false);
    assert_eq!(landed.duplicates_skipped, vec![EffectKind::Outline], "une seule fois par calque");
    assert_eq!(vector.len(), 5);
}

// ------------------------------------------------------------------ dans l'application

/// Une application sans fenêtre avec un plan de 24 images et un calque de la sorte `kind`.
fn app_with(name: &str, kind: LayerKind) -> crate::app::App {
    let dir = std::env::temp_dir().join(format!("shot-fxmatiere-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(kind);
    let mut l = crate::document::Layer::new("Dessin", kind);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 6;
    app
}

#[test]
fn the_actions_toggle_each_material_effect_where_it_applies_in_one_undoable_step() {
    let mut app = app_with("actions", LayerKind::Vector);
    for kind in m::KINDS {
        let before = app.history.names().0.len();
        app.run_action(m::action(kind));
        assert!(app.fx_on(0, kind), "{kind:?} est coché");
        assert_eq!(app.history.names().0.len(), before + 1, "une étape d'annulation");
        app.run_action(m::action(kind));
        assert!(!app.fx_on(0, kind), "{kind:?} est décoché mais reste sur le calque");
        assert_eq!(app.layers()[0].fx.iter().filter(|e| e.kind == kind).count(), 1);
    }
    app.undo();
    assert!(app.fx_on(0, EffectKind::Nib), "défaire remet l'effet coché");
    // Sur un bitmap : trois oui, deux non (avec la raison dans la barre d'état).
    let mut app = app_with("bitmap", LayerKind::Bitmap);
    for kind in m::KINDS {
        app.run_action(m::action(kind));
        if kind.vector_only() {
            assert!(!app.fx_on(0, kind) && app.status.contains("vectoriel"), "{kind:?} : {}", app.status);
        } else {
            assert!(app.fx_on(0, kind), "{kind:?}");
        }
    }
    // Ajouter deux fois par le menu « Ajouter un effet » ne double pas l'effet.
    let mut app = app_with("une-fois", LayerKind::Vector);
    app.add_effect_once(0, EffectKind::Bleed);
    app.add_effect_once(0, EffectKind::Bleed);
    assert_eq!(app.layers()[0].fx.len(), 1);
}

#[test]
fn the_presets_put_ready_made_effects_on_the_layer_and_each_has_an_action() {
    let palette = crate::palette::palette_commands();
    for p in m::Preset::ALL {
        assert_eq!(m::Preset::from_action(p.action()), Some(p));
        assert!(palette.iter().any(|(id, _)| *id == p.action()), "{} absent de la palette", p.action());
        assert!(!p.label().is_empty() && !p.note().is_empty());
        assert_eq!(p.build(), p.build());
        for e in p.build() {
            assert_eq!(e.values_at(0).len(), e.kind.params().len());
        }
    }
    for kind in m::KINDS {
        assert!(palette.iter().any(|(id, _)| *id == m::action(kind)), "{} absent de la palette", m::action(kind));
    }
    assert!(palette.iter().any(|(id, _)| *id == m::BAKE_ACTION));
    assert!(!m::Preset::ChinaInk.vector_only() && !m::Preset::WhiteOutline.vector_only());
    assert!(m::Preset::WornFelt.vector_only() && m::Preset::Rainbow.vector_only() && m::Preset::CalligraphyPen.vector_only());
    // Sur un calque vectoriel : tous passent ; ceux de même sorte se remplacent.
    let mut app = app_with("presets", LayerKind::Vector);
    for p in m::Preset::ALL {
        app.run_action(p.action());
    }
    let kinds: Vec<EffectKind> = app.layers()[0].fx.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EffectKind::Bleed, EffectKind::Dry, EffectKind::Outline, EffectKind::StrokeGradient, EffectKind::Nib], "feutre usé, puis plume calligraphique remplace la plume");
    let nib = m::NibS::of(&app.layers()[0].fx[4].values);
    assert_eq!(nib.id, m::name_id(m::CALLIGRAPHY));
    app.run_action(m::Preset::WornFelt.action());
    assert_eq!(app.layers()[0].fx.iter().filter(|e| e.kind == EffectKind::Nib).count(), 1, "pas de doublon");
    // Sur un bitmap : l'encre de Chine et le contour blanc passent, les autres sont refusés sans rien poser.
    let mut app = app_with("presets-bitmap", LayerKind::Bitmap);
    for p in [m::Preset::WornFelt, m::Preset::Rainbow, m::Preset::CalligraphyPen] {
        app.run_action(p.action());
        assert!(app.layers()[0].fx.is_empty() && app.status.contains("vectoriel"), "{p:?}");
    }
    app.run_action(m::Preset::ChinaInk.action());
    app.run_action(m::Preset::WhiteOutline.action());
    assert_eq!(app.layers()[0].fx.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![EffectKind::Bleed, EffectKind::Outline]);
}

#[test]
fn applying_the_nib_for_good_replaces_the_strokes_in_one_named_step_and_undo_brings_everything_back() {
    crate::fxcache::monde::ecrire();
    let mut app = app_with("bake", LayerKind::Vector);
    let cel = app.layers()[0].frames[0].unwrap();
    let drawing = VectorDrawing { elements: vec![pressed_stroke(40.17), pressed_stroke(90.31)] };
    {
        let c = app.doc.cels.get_mut(&cel).unwrap();
        c.vector = Some(drawing.clone());
        c.rerender();
    }
    // Sans effet de plume : rien à faire.
    app.run_action(m::BAKE_ACTION);
    assert!(app.status.contains("Pas d'effet"), "{}", app.status);
    app.run_action(m::action(EffectKind::Nib));
    assert!(app.fx_on(0, EffectKind::Nib));
    let (steps_before, version) = (app.history.names().0.len(), app.doc.cels[&cel].version);
    app.run_action(m::BAKE_ACTION);
    // L'effet est retiré, les traits portent la plume, une seule étape d'annulation, nommée.
    assert!(app.layers()[0].fx.is_empty());
    let baked = app.doc.cels[&cel].vector.clone().unwrap();
    assert!(baked.elements.iter().all(|e| matches!(e, Element::Stroke { brush: Some(_), .. })));
    assert!(app.doc.cels[&cel].version > version);
    let names = app.history.names().0;
    assert_eq!(names.len(), steps_before + 1);
    assert!(names.last().is_some_and(|n| n.contains("Plume appliquée pour de bon")), "{names:?}");
    assert!(app.status.contains("appliquée pour de bon"), "{}", app.status);
    // Annuler : les traits d'origine et l'effet sont revenus.
    app.undo();
    assert!(app.fx_on(0, EffectKind::Nib));
    assert_eq!(app.doc.cels[&cel].vector.as_ref().unwrap(), &drawing);
    assert!(app.doc.cels[&cel].vector.as_ref().unwrap().elements.iter().all(|e| matches!(e, Element::Stroke { brush: None, .. })));
    // Rétablir : à nouveau appliqué.
    app.redo();
    assert!(app.layers()[0].fx.is_empty());
    assert!(app.doc.cels[&cel].vector.as_ref().unwrap().elements.iter().all(|e| matches!(e, Element::Stroke { brush: Some(_), .. })));
}

#[test]
fn the_effects_panel_draws_the_material_row_and_every_parameter_without_panicking() {
    // Un calque vectoriel avec les cinq effets, puis un bitmap : le panneau Effets se dessine (images d'essai).
    for kind in [LayerKind::Vector, LayerKind::Bitmap] {
        let mut app = app_with("panneau", kind);
        let all: Vec<Effect> = m::KINDS.iter().filter(|k| !k.vector_only() || kind == LayerKind::Vector).map(|k| Effect::new(*k)).collect();
        app.layers_mut()[0].fx = all;
        let ctx = eframe::egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                eframe::egui::CentralPanel::default().show(ctx, |ui| app.fx_panel(ui));
            });
        }
    }
}

// ------------------------------------------------------------------ mesures et planches à regarder

fn ms_of(f: impl FnOnce()) -> f64 {
    let t0 = std::time::Instant::now();
    f();
    t0.elapsed().as_secs_f64() * 1e3
}

/// Les mesures du chapitre « Effets de matière » de `docs/PERFORMANCE.md` :
/// `cargo test --release --offline --target-dir target\msvc --lib fxmatiere_tests::measure -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore]
fn measure_material_effects_on_a_dense_vector_layer_and_a_4k_bitmap() {
    crate::fxcache::monde::ecrire();
    let (w, h) = (1920usize, 1080usize);
    let d = crate::fxtrait_tests::dense_drawing(1800, 200, w, h);
    println!("{} traits (200 texturés), feuille {w} x {h}, {} fils", d.elements.len(), crate::parallele::coeurs());
    let _ = d.render(w, h);
    let raster = d.render(w, h);
    let run = |name: &str, kind: EffectKind, set: &[(usize, f32)], pixels: bool| {
        let mut times = Vec::new();
        for frame in 0..5 {
            let s = steps(kind, set, frame * 3 + 1);
            times.push(ms_of(|| {
                let _ = if pixels { fx::apply_with(&raster, None, &s, 0) } else { t::render(&d, w, h, &s) };
            }));
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        println!("{name:<52} {mean:>7.0} ms par image (min {:.0}, max {:.0})", times.iter().cloned().fold(f64::MAX, f64::min), times.iter().cloned().fold(0.0, f64::max));
    };
    println!("-- calque vectoriel de 2 000 traits, 1920 x 1080");
    run("liseré 4 px (pixels du calque)", EffectKind::Outline, &[], true);
    run("liseré 20 px, douceur 10", EffectKind::Outline, &[(m::OUTLINE_WIDTH, 20.0), (m::OUTLINE_SOFT, 10.0)], true);
    run("encre qui bave (2,5 px, bord foncé)", EffectKind::Bleed, &[], true);
    run("trait sec, par trait", EffectKind::Dry, &[], false);
    run("trait sec, toutes les 2 images", EffectKind::Dry, &[(m::DRY_EVERY, 2.0)], false);
    run("dégradé, par trait", EffectKind::StrokeGradient, &[], false);
    run("dégradé sur tout le dessin qui défile", EffectKind::StrokeGradient, &[(m::GRAD_WHOLE, 1.0), (m::GRAD_SCROLL, 5.0)], false);
    // La plume : le premier calcul rejoue les traits, les suivants les reprennent.
    let nib = steps(EffectKind::Nib, &[(m::NIB_ID, m::name_id("Feutre") as f32)], 0);
    let first = ms_of(|| {
        let _ = t::render(&d, w, h, &nib);
    });
    let again = ms_of(|| {
        let _ = t::render(&d, w, h, &nib);
    });
    println!("{:<52} {first:>7.0} ms la première fois (rejeu), {again:.0} ms ensuite", "changer de plume (Feutre)");
    let all = [steps(EffectKind::Dry, &[], 1), steps(EffectKind::StrokeGradient, &[], 1)].concat();
    let both = [all.clone(), steps(EffectKind::Outline, &[], 1), steps(EffectKind::Bleed, &[], 1)].concat();
    println!("{:<52} {:>7.0} ms", "trait sec + dégradé", ms_of(|| {
        let _ = fx::apply_with(&raster, Some(&d), &all, 0);
    }));
    println!("{:<52} {:>7.0} ms", "trait sec + dégradé + liseré + encre", ms_of(|| {
        let _ = fx::apply_with(&raster, Some(&d), &both, 0);
    }));
    println!("-- calque bitmap 4K (3840 x 2160)");
    let (w4, h4) = (3840usize, 2160usize);
    let art = crate::fxtrait_tests::dense_drawing(1800, 200, w4, h4).render(w4, h4);
    let full = rect(w4, h4, 0, 0, w4, h4, Color32::from_rgb(90, 60, 40));
    let blob = rect(w4, h4, 600, 300, 3200, 1800, Color32::from_rgb(90, 60, 40));
    for (label, src) in [("dessin au trait (2 000 traits)", &art), ("grande forme pleine", &blob), ("feuille entière pleine", &full)] {
        for (name, kind, set) in [
            ("liseré 4 px", EffectKind::Outline, vec![]),
            ("liseré 20 px", EffectKind::Outline, vec![(m::OUTLINE_WIDTH, 20.0)]),
            ("encre qui bave", EffectKind::Bleed, vec![]),
            ("trait sec (bitmap)", EffectKind::Dry, vec![]),
        ] {
            let s = steps(kind, &set, 0);
            let times: Vec<f64> = (0..3).map(|_| ms_of(|| {
                let _ = fx::apply_with(src, None, &s, 0);
            })).collect();
            println!("{label:<34} {name:<20} {:>7.0} ms (min {:.0})", times.iter().sum::<f64>() / 3.0, times.iter().cloned().fold(f64::MAX, f64::min));
        }
    }
}

/// Regarder les effets : une planche de contact écrite dans le dossier donné par `FXMATIERE_PNG`.
/// `$env:FXMATIERE_PNG = 'C:\\...'; cargo test --release --offline --target-dir target\msvc --lib fxmatiere_tests::dump -- --ignored --nocapture`
#[test]
#[ignore]
fn dump_material_effects_to_png() {
    let Ok(dir) = std::env::var("FXMATIERE_PNG") else { return };
    crate::fxcache::monde::ecrire();
    let (w, h) = (640usize, 340usize);
    let d = crate::fxtrait_tests::sample_drawing_for_dump();
    let raster = d.render(w, h);
    let panels: Vec<(&str, Vec<FxStep>)> = vec![
        ("dessin", Vec::new()),
        ("lisere blanc 4", steps(EffectKind::Outline, &[], 0)),
        ("lisere rouge 8 doux 6", steps(EffectKind::Outline, &[(m::OUTLINE_WIDTH, 8.0), (m::OUTLINE_SOFT, 6.0), (m::OUTLINE_RGB, 230.0), (m::OUTLINE_RGB + 1, 40.0), (m::OUTLINE_RGB + 2, 40.0)], 0)),
        ("lisere en anneau 50", steps(EffectKind::Outline, &[(m::OUTLINE_WIDTH, 8.0), (m::OUTLINE_BEHIND, 0.0), (m::OUTLINE_OPACITY, 50.0)], 0)),
        ("trait sec", steps(EffectKind::Dry, &[], 0)),
        ("trait sec fort", steps(EffectKind::Dry, &[(m::DRY_FREQ, 10.0), (m::DRY_LENGTH, 30.0), (m::DRY_ROUGH, 90.0)], 0)),
        ("encre qui bave", steps(EffectKind::Bleed, &[], 0)),
        ("encre qui bave fort", steps(EffectKind::Bleed, &[(m::BLEED_SPREAD, 6.0), (m::BLEED_IRREG, 100.0), (m::BLEED_DARK, 80.0), (m::BLEED_HALO, 80.0)], 0)),
        ("degrade par trait", steps(EffectKind::StrokeGradient, &[], 0)),
        ("degrade tout le dessin", steps(EffectKind::StrokeGradient, &[(m::GRAD_WHOLE, 1.0), (m::GRAD_COUNT, 4.0)], 0)),
        ("plume calligraphique", steps(EffectKind::Nib, &[], 0)),
        ("plume feutre", steps(EffectKind::Nib, &[(m::NIB_ID, m::name_id("Feutre") as f32)], 0)),
        ("plume fusain", steps(EffectKind::Nib, &[(m::NIB_ID, m::name_id("Fusain") as f32)], 0)),
        ("feutre use", m::Preset::WornFelt.build().iter().flat_map(|e| fx::steps_at(std::slice::from_ref(e), 0)).collect()),
        ("encre de chine", m::Preset::ChinaInk.build().iter().flat_map(|e| fx::steps_at(std::slice::from_ref(e), 0)).collect()),
        ("arc-en-ciel + contour", [steps(EffectKind::StrokeGradient, &[(m::GRAD_WHOLE, 1.0), (m::GRAD_COUNT, 4.0)], 0), steps(EffectKind::Outline, &[], 0)].concat()),
    ];
    let cols = 3usize;
    let rows = panels.len().div_ceil(cols);
    let mut sheet = vec![Color32::from_rgb(120, 150, 190); cols * w * rows * h];
    for (n, (name, s)) in panels.iter().enumerate() {
        let r = fx::apply_with(&raster, Some(&d), s, 0);
        let (ox, oy) = ((n % cols) * w, (n / cols) * h);
        for y in 0..h {
            for x in 0..w {
                sheet[(oy + y) * cols * w + ox + x] = render::over(Color32::from_rgb(120, 150, 190), r.get(x, y), 255, None);
            }
        }
        for x in 0..w {
            sheet[oy * cols * w + ox + x] = Color32::from_gray(200);
        }
        for y in 0..h {
            sheet[(oy + y) * cols * w + ox] = Color32::from_gray(200);
        }
        println!("{n} : {name}");
    }
    std::fs::create_dir_all(&dir).ok();
    crate::io::write_png(&std::path::Path::new(&dir).join("effets_de_matiere.png"), &sheet, cols * w, rows * h).unwrap();
    // Et sur un bitmap (la version simplifiée) : le même dessin, rasterisé, sans le dessin vectoriel.
    let bit: Vec<(&str, Vec<FxStep>)> = vec![
        ("bitmap", Vec::new()),
        ("bitmap lisere", steps(EffectKind::Outline, &[], 0)),
        ("bitmap trait sec", steps(EffectKind::Dry, &[(m::DRY_FREQ, 8.0), (m::DRY_LENGTH, 24.0)], 0)),
        ("bitmap encre", steps(EffectKind::Bleed, &[], 0)),
    ];
    let mut sheet = vec![Color32::from_rgb(120, 150, 190); 2 * w * 2 * h];
    for (n, (name, s)) in bit.iter().enumerate() {
        let r = fx::apply_with(&raster, None, s, 0);
        let (ox, oy) = ((n % 2) * w, (n / 2) * h);
        for y in 0..h {
            for x in 0..w {
                sheet[(oy + y) * 2 * w + ox + x] = render::over(Color32::from_rgb(120, 150, 190), r.get(x, y), 255, None);
            }
        }
        println!("{n} : {name}");
    }
    crate::io::write_png(&std::path::Path::new(&dir).join("effets_de_matiere_bitmap.png"), &sheet, 2 * w, 2 * h).unwrap();
}
