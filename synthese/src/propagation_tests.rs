use crate::flow::Flow;
use crate::propagation::*;
use crate::Image;

/// Décor coloré (ondes de plusieurs tailles), défini partout pour pouvoir le faire glisser.
fn scene(x: f32, y: f32) -> [u8; 3] {
    let wave = |a: f32, b: f32, p: f32| (a * x + b * y + p).sin();
    let r = 128.0 + 60.0 * wave(0.09, 0.04, 0.2) + 40.0 * wave(0.45, -0.31, 1.0);
    let g = 128.0 + 60.0 * wave(-0.05, 0.11, 1.3) + 40.0 * wave(0.27, 0.52, 2.1);
    let b = 128.0 + 60.0 * wave(0.13, -0.08, 2.4) + 40.0 * wave(-0.38, 0.41, 0.4);
    [r as u8, g as u8, b as u8]
}

/// Image t de la vidéo : le décor glisse de (2, 1) pixels par image.
fn frame(t: usize, w: usize, h: usize) -> Image {
    let data = (0..w * h).flat_map(|i| scene((i % w) as f32 - 2.0 * t as f32, (i / w) as f32 - t as f32)).collect();
    Image::new(w, h, 3, data)
}

/// « Peinture » de la clé : une fonction des couleurs, pour connaître le bon résultat sur
/// chaque image (la même fonction appliquée à l'image de la vidéo).
fn paint(im: &Image) -> Image {
    let data = im.data.chunks_exact(3).flat_map(|p| [255 - p[1], p[0], p[2] / 2]).collect();
    Image::new(im.width, im.height, 3, data)
}

fn mean_diff(a: &Image, b: &Image, margin: usize) -> f32 {
    let (mut s, mut n) = (0u64, 0u64);
    for y in margin..a.height - margin {
        for x in margin..a.width - margin {
            let o = (y * a.width + x) * 3;
            s += (0..3).map(|k| a.data[o + k].abs_diff(b.data[o + k]) as u64).sum::<u64>();
            n += 3;
        }
    }
    s as f32 / n as f32
}

const W: usize = 96;
const H: usize = 72;

fn run(key: usize, frames: usize) -> (Vec<usize>, Vec<Option<Image>>) {
    let style = paint(&frame(key, W, H));
    let (mut order, mut out) = (Vec::new(), vec![None; frames]);
    propagate(|t| Ok(frame(t, W, H)), key, &style, 0..frames, &PropagateParams::default(), |t, im| {
        order.push(t);
        out[t] = Some(im.clone());
        true
    })
    .unwrap();
    (order, out)
}

#[test]
fn the_painting_follows_the_motion() {
    let (order, out) = run(0, 5);
    assert_eq!(order, [0, 1, 2, 3, 4]);
    let style = paint(&frame(0, W, H));
    for (t, im) in out.iter().enumerate().skip(1) {
        let truth = paint(&frame(t, W, H));
        let (got, still) = (mean_diff(im.as_ref().unwrap(), &truth, 12), mean_diff(&style, &truth, 12));
        assert!(got < 6.0 && got * 4.0 < still, "image {t} : écart {got}, sans suivre le mouvement {still}");
    }
}

#[test]
fn from_a_middle_key_both_ways() {
    let (order, out) = run(2, 5);
    assert_eq!(order, [2, 1, 0, 3, 4]);
    for (t, im) in out.iter().enumerate() {
        let got = mean_diff(im.as_ref().unwrap(), &paint(&frame(t, W, H)), 12);
        assert!(got < 6.0, "image {t} : écart {got}");
    }
}

#[test]
fn errors_and_cancel() {
    let style = paint(&frame(0, W, H));
    let p = PropagateParams::default();
    let r = propagate(|t| Ok(frame(t, W, H)), 5, &style, 0..3, &p, |_, _| true);
    assert!(matches!(r, Err(PropagateError::KeyOutsideRange(5))));
    let r = propagate_keys(|t| Ok(frame(t, W, H)), &[], 0..3, &p, |_, _| true);
    assert!(matches!(r, Err(PropagateError::NoKey)));
    let twice = [Key { frame: 1, style: &style }, Key { frame: 1, style: &style }];
    let r = propagate_keys(|t| Ok(frame(t, W, H)), &twice, 0..3, &p, |_, _| true);
    assert!(matches!(r, Err(PropagateError::DuplicateKey(1))));
    let small = paint(&frame(0, W - 2, H));
    let r = propagate_keys(|t| Ok(frame(t, W, H)), &[Key { frame: 0, style: &style }, Key { frame: 2, style: &small }], 0..3, &p, |_, _| true);
    assert!(matches!(r, Err(PropagateError::Size(2))));
    let mut calls = 0;
    let r = propagate(|t| Ok(frame(t, W, H)), 0, &style, 0..5, &p, |_, _| {
        calls += 1;
        calls < 2
    });
    assert!(matches!(r, Err(PropagateError::Cancelled)));
    assert_eq!(calls, 2);
    let r = propagate(|t| Ok(frame(t, if t == 1 { W - 2 } else { W }, H)), 0, &style, 0..3, &p, |_, _| true);
    assert!(matches!(r, Err(PropagateError::Size(1))));
    let r = propagate(|t| if t == 2 { Err("illisible".into()) } else { Ok(frame(t, W, H)) }, 0, &style, 0..3, &p, |_, _| true);
    assert!(matches!(r, Err(PropagateError::Load(2, _))));
}

/// Une autre « peinture », pour distinguer ce qui vient de chaque clé.
fn paint2(im: &Image) -> Image {
    let data = im.data.chunks_exact(3).flat_map(|p| [p[2], 255 - p[0], p[1]]).collect();
    Image::new(im.width, im.height, 3, data)
}

/// Propage les clés données (image, peinture) sur 0..frames ; chaque image doit sortir une fois.
fn run_keys(keys: &[(usize, Image)], frames: usize) -> Vec<Image> {
    let keys: Vec<Key> = keys.iter().map(|(f, s)| Key { frame: *f, style: s }).collect();
    let mut out = vec![None; frames];
    propagate_keys(|t| Ok(frame(t, W, H)), &keys, 0..frames, &PropagateParams::default(), |t, im| {
        assert!(out[t].is_none(), "image {t} deux fois");
        out[t] = Some(im.clone());
        true
    })
    .unwrap();
    out.into_iter().map(|o| o.expect("chaque image")).collect()
}

#[test]
fn two_keys_of_the_same_painting() {
    let keys = [(0, paint(&frame(0, W, H))), (6, paint(&frame(6, W, H)))];
    let out = run_keys(&keys, 8);
    for (t, im) in out.iter().enumerate() {
        let got = mean_diff(im, &paint(&frame(t, W, H)), 12);
        assert!(got < 6.0, "image {t} : écart {got}");
    }
}

#[test]
fn two_different_paintings_hand_over() {
    // Clé 0 peinte d'une façon, clé 6 d'une autre, aux couleurs toutes différentes. Les couleurs
    // d'ensemble suivent le fondu enchaîné : on passe régulièrement de l'une à l'autre (écarts
    // mesurés : 0, 15, 24, 34, 43, 54, 71 avec la première, l'inverse avec la seconde).
    let keys = [(0, paint(&frame(0, W, H))), (6, paint2(&frame(6, W, H)))];
    let out = run_keys(&keys, 7);
    let d1 = |t: usize| mean_diff(&out[t], &paint(&frame(t, W, H)), 12);
    let d2 = |t: usize| mean_diff(&out[t], &paint2(&frame(t, W, H)), 12);
    for t in 0..6 {
        assert!(d1(t) < d1(t + 1) && d2(t + 1) < d2(t), "image {t} : {} {} / {} {}", d1(t), d1(t + 1), d2(t), d2(t + 1));
    }
    assert!(d1(2) < d2(2) && d2(4) < d1(4), "passage au milieu : {} {} / {} {}", d1(2), d2(2), d1(4), d2(4));
}

#[test]
fn warp_by_a_whole_shift() {
    let im = frame(0, 20, 10);
    let out = warp(&im, &Flow { width: 20, height: 10, data: vec![[2.0, 1.0]; 200] });
    for y in 0..9 {
        for x in 0..18 {
            assert_eq!(out.data[(y * 20 + x) * 3..][..3], im.data[((y + 1) * 20 + x + 2) * 3..][..3]);
        }
    }
}

#[test]
fn position_guide_spans_the_range() {
    let pos = Flow { width: 3, height: 2, data: vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [0.0, 1.0], [1.0, 1.0], [2.0, 1.0]] };
    let g = position_guide(&pos);
    assert_eq!(g.channels, 2);
    assert_eq!(g.data, [0, 0, 128, 0, 255, 0, 0, 255, 128, 255, 255, 255]);
}

#[test]
fn a_kept_image_drops_its_empty_tiles_without_loss() {
    // 70 × 40 en RVBA (carreaux de bord incomplets) : un seul petit carré peint.
    let (w, h) = (70, 40);
    let mut data = vec![0u8; w * h * 4];
    for y in 33..38 {
        for x in 64..69 {
            data[(y * w + x) * 4..][..4].copy_from_slice(&[200, 10, 30, 255]);
        }
    }
    let im = Image::new(w, h, 4, data);
    let s = Sparse::from_image(&im);
    assert_eq!(s.to_image().data, im.data, "sans perte");
    assert!(s.bytes() <= 32 * 32 * 4, "un seul carreau gardé : {}", s.bytes());
    // Une image vide ne garde rien ; une pleine, tout.
    assert_eq!(Sparse::from_image(&Image::new(w, h, 4, vec![0; w * h * 4])).bytes(), 0);
    let full = Image::new(w, h, 3, vec![7; w * h * 3]);
    assert_eq!(Sparse::from_image(&full).to_image().data, full.data);
}