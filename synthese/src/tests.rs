use super::*;
use crate::patchmatch as pm;

/// Bruit déterministe : chaque patch 5×5 est différent de tous les autres.
pub(crate) fn noise(w: usize, h: usize, channels: usize, seed: u64) -> Image {
    let mut rng = pm::Rng::new(seed);
    let data = (0..w * h * channels).map(|_| rng.next_u32() as u8).collect();
    Image::new(w, h, channels, data)
}

/// Guide de position : rouge = x, vert = y, sur toute la plage 0..255.
pub(crate) fn position(w: usize, h: usize, dx: i32, dy: i32) -> Image {
    let mut data = Vec::with_capacity(w * h * 3);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let px = (x - dx).clamp(0, w as i32 - 1) * 255 / (w as i32 - 1);
            let py = (y - dy).clamp(0, h as i32 - 1) * 255 / (h as i32 - 1);
            data.extend_from_slice(&[px as u8, py as u8, 0]);
        }
    }
    Image::new(w, h, 3, data)
}

fn single_thread(seed: u64) -> Params {
    Params { threads: 1, seed, ..Params::default() }
}

/// Écart moyen (0..255) entre `a` et `b` décalée de (dx, dy), sur l'intérieur `margin` pixels
/// loin des bords, et part des pixels identiques.
pub(crate) fn interior_diff(a: &Image, b: &Image, dx: i32, dy: i32, margin: usize) -> (f32, f32) {
    let (mut sum, mut same, mut n) = (0u64, 0u64, 0u64);
    for y in margin..a.height - margin {
        for x in margin..a.width - margin {
            let (bx, by) = ((x as i32 - dx) as usize, (y as i32 - dy) as usize);
            let pa = &a.data[(y * a.width + x) * a.channels..][..a.channels];
            let pb = &b.data[(by * b.width + bx) * b.channels..][..b.channels];
            let d: u64 = pa.iter().zip(pb).map(|(p, q)| p.abs_diff(*q) as u64).sum();
            sum += d;
            same += (d == 0) as u64;
            n += 1;
        }
    }
    (sum as f32 / (n * a.channels as u64) as f32, same as f32 / n as f32)
}

#[test]
fn pyramid_sizes_like_ebsynth() {
    assert_eq!(pm::level_size((1200, 912), 7, 6), (1200, 912));
    assert_eq!(pm::level_size((1200, 912), 7, 5), (600, 456));
    assert_eq!(pm::level_size((1200, 912), 7, 0), (18, 14));
    // Le plus petit côté du niveau le plus grossier garde au moins 2 × patch + 1 pixels.
    assert_eq!(max_pyramid_levels((1200, 912), (1200, 912), 5), 7);
    assert_eq!(max_pyramid_levels((320, 462), (320, 462), 3), 6);
    assert_eq!(max_pyramid_levels((810, 1000), (400, 2000), 5), 6);
    assert_eq!(max_pyramid_levels((10, 10), (10, 10), 5), 0);
}

#[test]
fn channels_like_ebsynth() {
    let gray = [10, 10, 10, 255, 200, 200, 200, 255];
    let gray_alpha = [10, 10, 10, 255, 200, 200, 200, 128];
    let color = [10, 20, 30, 255, 200, 200, 200, 255];
    let color_alpha = [10, 20, 30, 0, 200, 200, 200, 255];
    for (rgba, n) in [(gray, 1), (gray_alpha, 2), (color, 3), (color_alpha, 4)] {
        assert_eq!(Image::channels_needed(&rgba), n);
        let im = Image::from_rgba(2, 1, &rgba, n);
        assert_eq!(im.data.len(), 2 * n);
        assert_eq!(im.to_rgba(), rgba);
    }
    assert_eq!(Image::from_rgba(2, 1, &gray_alpha, 2).data, [10, 255, 200, 128]);
}

#[test]
fn resample_by_two_keeps_every_other_pixel() {
    let data = (0..8 * 8).map(|i| ((i % 8) + 10 * (i / 8)) as u8).collect();
    let src = pm::Plane { w: 8, h: 8, c: 1, data };
    let out = pm::resample(&src, 4, 4);
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(out.data[y * 4 + x] as usize, 2 * x + 20 * y);
        }
    }
}

#[test]
fn upscale_doubles_and_stays_inside() {
    let nnf = [[3, 3], [4, 3], [3, 4], [9, 9]];
    let out = pm::nnf_upscale(&nnf, (2, 2), (4, 4), (20, 20), 5);
    assert_eq!(out[0], [6, 6]);
    assert_eq!(out[1], [7, 6]);
    assert_eq!(out[2], [8, 6]);
    assert_eq!(out[5], [7, 7]);
    // [9, 9] × 2 + 1 = [19, 19], ramené à 20 − 5 − 1 = 14.
    assert_eq!(out[15], [14, 14]);
}

#[test]
fn vote_with_identity_copies_the_source() {
    let source = noise(20, 16, 3, 7);
    let src = pm::Plane { w: 20, h: 16, c: 3, data: source.data.clone() };
    let mut target = pm::Plane { w: 20, h: 16, c: 3, data: vec![0; 20 * 16 * 3] };
    let nnf: Vec<[i32; 2]> = (0..20 * 16).map(|i| [(i % 20).clamp(2, 17), (i / 20).clamp(2, 13)]).collect();
    for threads in [1, 3] {
        pm::vote(&mut target, &src, 3, &nnf, 5, threads);
        // Près des bords, les patchs dont le centre a été ramené dans le style faussent le vote :
        // on ne regarde qu'à 2 × rayon des bords.
        let out = Image::new(20, 16, 3, target.data.clone());
        assert_eq!(interior_diff(&out, &source, 0, 0, 4), (0.0, 1.0));
    }
}

#[test]
fn patchmatch_lowers_the_cost() {
    let style = noise(60, 50, 3, 3);
    let plane = |im: &Image| pm::Plane { w: im.width, h: im.height, c: 3, data: im.data.clone() };
    let (source, target) = (plane(&style), plane(&noise(60, 50, 3, 4)));
    for threads in [1, 4] {
        let mut rng = pm::Rng::new(5);
        let mut nnf = pm::nnf_random((60, 50), (60, 50), 5, &mut rng);
        let mut err = vec![0.0; 60 * 50];
        let search = |iters| pm::Search {
            target: &target,
            source: &source,
            weights: &[1.0, 1.0, 1.0],
            patch_size: 5,
            uniformity: 3500.0,
            iters,
            threads,
        };
        pm::patchmatch(&search(0), &mut nnf, &mut err, &mut rng);
        let before: f32 = err.iter().sum();
        pm::patchmatch(&search(4), &mut nnf, &mut err, &mut rng);
        let after: f32 = err.iter().sum();
        assert!(after < 0.8 * before, "{threads} fils : {before} → {after}");
        assert!(nnf.iter().all(|n| (2..58).contains(&n[0]) && (2..48).contains(&n[1])));
    }
}

#[test]
fn same_guides_give_back_the_style() {
    let style = noise(48, 40, 3, 11);
    let guide = noise(48, 40, 1, 12);
    let out = synthesize(&style, 1.0, &[GuidePair { source: &guide, target: &guide, weight: 1.0 }], &single_thread(1))
        .unwrap();
    let (mean, same) = interior_diff(&out.image, &style, 0, 0, 4);
    assert!(mean < 1.0 && same > 0.97, "écart {mean}, identiques {same}");
}

/// Image douce (ondes de couleur) : un style qui a des formes, comme une peinture.
pub(crate) fn waves(w: usize, h: usize) -> Image {
    let mut data = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (x, y) = (x as f32, y as f32);
            data.push((128.0 + 100.0 * (0.30 * x + 0.10 * y).sin()) as u8);
            data.push((128.0 + 100.0 * (0.25 * y - 0.05 * x).cos()) as u8);
            data.push((128.0 + 100.0 * (0.20 * (x + y)).sin() * (0.15 * x).cos()) as u8);
        }
    }
    Image::new(w, h, 3, data)
}

#[test]
fn position_guide_moves_the_style() {
    // Le guide de position de la cible est décalé de (5, 3) : la peinture doit suivre. Sur ces
    // images, l'ebsynth original donne un écart moyen de 13 avec le style décalé (bords compris)
    // contre 62 sans décalage ; le portage 13 aussi. (Avec un style de bruit pur, l'original échoue
    // comme le portage, écart 82 : il ne garde que des morceaux cohérents mal placés.)
    let style = waves(64, 64);
    let (src, tgt) = (position(64, 64, 0, 0), position(64, 64, 5, 3));
    let guides = [GuidePair { source: &src, target: &tgt, weight: 10.0 }];
    for params in [single_thread(2), Params { threads: 4, seed: 2, ..Params::default() }] {
        let out = synthesize(&style, 1.0, &guides, &params).unwrap();
        let (shifted, _) = interior_diff(&out.image, &style, 5, 3, 10);
        let (unshifted, _) = interior_diff(&out.image, &style, 0, 0, 10);
        assert!(shifted < 14.0 && 3.0 * shifted < unshifted, "{} fils : {shifted} / {unshifted}", params.threads);
    }
}

#[test]
fn texture_by_numbers_puts_each_texture_in_its_region() {
    // Style : bruit rouge à gauche, bruit bleu à droite ; le guide est la segmentation (noir à
    // gauche, blanc à droite). Cible : un disque blanc au milieu du noir.
    let (w, h) = (64, 48);
    let mut rng = pm::Rng::new(3);
    let (mut style, mut seg) = (Vec::new(), Vec::new());
    for _y in 0..h {
        for x in 0..w {
            let v = (rng.next_u32() % 128) as u8;
            let right = x >= w / 2;
            style.extend_from_slice(&if right { [v, v / 2, 128 + v] } else { [128 + v, v / 2, v] });
            seg.push(if right { 255 } else { 0 });
        }
    }
    let dist = |x: usize, y: usize| ((x as f32 - 32.0).powi(2) + (y as f32 - 24.0).powi(2)).sqrt();
    let target: Vec<u8> = (0..w * h).map(|i| if dist(i % w, i / w) < 14.0 { 255 } else { 0 }).collect();
    let (style, seg, target) = (Image::new(w, h, 3, style), Image::new(w, h, 1, seg), Image::new(w, h, 1, target));
    let out = synthesize(&style, 1.0, &[GuidePair { source: &seg, target: &target, weight: 1.0 }], &Params::default())
        .unwrap();
    let (mut good, mut n) = (0, 0);
    for y in 0..h {
        for x in 0..w {
            if (dist(x, y) - 14.0).abs() < 4.0 {
                continue;
            }
            let p = &out.image.data[(y * w + x) * 3..][..3];
            good += ((dist(x, y) < 14.0) == (p[2] > p[0])) as usize;
            n += 1;
        }
    }
    assert!(good as f32 > 0.97 * n as f32, "{good} / {n}");
}

#[test]
fn one_thread_is_reproducible() {
    let style = noise(40, 40, 3, 31);
    let (src, tgt) = (position(40, 40, 0, 0), position(40, 40, -4, 2));
    let guides = [GuidePair { source: &src, target: &tgt, weight: 2.0 }];
    let a = synthesize(&style, 1.0, &guides, &single_thread(9)).unwrap();
    let b = synthesize(&style, 1.0, &guides, &single_thread(9)).unwrap();
    assert_eq!(a.image, b.image);
    assert_eq!(a.nnf, b.nnf);
}

#[test]
fn extra_pass_and_different_sizes() {
    // Cible plus grande que le style, passe 3×3 : tout reste dans les bornes du style.
    let style = noise(30, 26, 4, 41);
    let (src, tgt) = (position(30, 26, 0, 0), position(50, 34, 0, 0));
    let params = Params { extra_pass_3x3: true, threads: 3, ..Params::default() };
    let out = synthesize(&style, 1.0, &[GuidePair { source: &src, target: &tgt, weight: 1.0 }], &params).unwrap();
    assert_eq!((out.image.width, out.image.height, out.image.channels), (50, 34, 4));
    assert_eq!(out.nnf.len(), 50 * 34);
    assert!(out.nnf.iter().all(|n| (1..29).contains(&n[0]) && (1..25).contains(&n[1])));
}

#[test]
fn bad_inputs_are_refused() {
    let style = noise(30, 30, 3, 1);
    let guide = noise(30, 30, 1, 2);
    let small = noise(20, 30, 1, 3);
    let color = noise(30, 30, 3, 4);
    let ok = GuidePair { source: &guide, target: &guide, weight: 1.0 };
    let p = Params::default();
    assert_eq!(synthesize(&style, 1.0, &[], &p).unwrap_err(), Error::NoGuide);
    let even = Params { patch_size: 4, ..Params::default() };
    assert_eq!(synthesize(&style, 1.0, &[ok], &even).unwrap_err(), Error::BadPatchSize(4));
    let bad_size = GuidePair { source: &small, target: &guide, weight: 1.0 };
    assert_eq!(synthesize(&style, 1.0, &[ok, bad_size], &p).unwrap_err(), Error::SizeMismatch(1));
    let bad_channels = GuidePair { source: &guide, target: &color, weight: 1.0 };
    assert_eq!(synthesize(&style, 1.0, &[bad_channels], &p).unwrap_err(), Error::ChannelMismatch(0));
    let tiny = noise(10, 10, 3, 5);
    let tiny_guide = noise(10, 10, 1, 6);
    let t = GuidePair { source: &tiny_guide, target: &tiny_guide, weight: 1.0 };
    assert_eq!(synthesize(&tiny, 1.0, &[t], &p).unwrap_err(), Error::TooSmall);
    assert_eq!(synthesize(&style, -1.0, &[ok], &p).unwrap_err(), Error::BadWeight);
}
