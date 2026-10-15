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
    assert_eq!(order, [2, 3, 4, 1, 0]);
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
    assert!(matches!(r, Err(PropagateError::KeyOutsideRange)));
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
