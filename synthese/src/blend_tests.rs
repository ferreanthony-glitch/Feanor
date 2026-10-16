use crate::blend::*;
use crate::Image;

/// Texture fine (ondes serrées) décalée de `dx` pixels et éclaircie de `lift`, sur 1 canal.
fn texture(w: usize, h: usize, dx: f32, lift: f32) -> Image {
    let data = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 - dx, (i / w) as f32);
            (100.0 + lift + 40.0 * (0.9 * x + 0.4 * y).sin() + 30.0 * (-0.5 * x + 1.1 * y).cos()) as u8
        })
        .collect();
    Image::new(w, h, 1, data)
}

fn flat_error(w: usize, h: usize, v: f32) -> ErrorMap {
    ErrorMap::new(&vec![v; w * h], w, h, 8)
}

/// Moyenne des écarts entre voisins (horizontaux et verticaux) sur la zone [x0, x1) × [y0, y1) :
/// le contraste des détails.
fn detail(im: &Image, x0: usize, x1: usize, y0: usize, y1: usize) -> f32 {
    let (mut s, mut n) = (0f32, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = im.data[y * im.width + x] as f32;
            s += (im.data[y * im.width + x + 1] as f32 - p).abs() + (im.data[(y + 1) * im.width + x] as f32 - p).abs();
            n += 2;
        }
    }
    s / n as f32
}

fn mean(im: &Image) -> f32 {
    im.data.iter().map(|v| *v as f32).sum::<f32>() / im.data.len() as f32
}

#[test]
fn error_map_averages_cells() {
    let e: Vec<f32> = (0..16 * 8).map(|i| if i % 16 < 8 { 1.0 } else { 3.0 }).collect();
    let m = ErrorMap::new(&e, 16, 8, 8);
    assert_eq!(m.data, [1.0, 3.0]);
    assert_eq!(m.at(0, 0), 1.0);
    assert_eq!(m.at(15, 7), 3.0);
    // À mi-chemin entre les centres des deux carrés (x = 7,5), la moyenne.
    assert!((m.at(7, 4) - 1.875).abs() < 1e-5 && (m.at(8, 4) - 2.125).abs() < 1e-5);
}

#[test]
fn selection_slides_with_alpha() {
    // Part de l'erreur de A : 0,3 partout (A meilleure).
    let (ea, eb) = (flat_error(16, 8, 3.0), flat_error(16, 8, 7.0));
    let at = |alpha: f32, softness: f32| selection(&ea, &eb, alpha, softness)[0];
    assert_eq!((at(0.6, 0.0), at(0.75, 0.0)), (1.0, 0.0));
    // Douce : à mi-chemin du seuil (α = 0,7), la moitié ; la bascule s'étale sur 0,5 d'α.
    assert!((at(0.7, 0.5) - 0.5).abs() < 1e-5);
    assert!((at(0.5, 0.5) - 0.9).abs() < 1e-5 && at(0.4, 0.5) == 1.0 && at(0.95, 0.5) == 0.0);
}

#[test]
fn near_the_first_key_it_is_a() {
    let (a, b) = (texture(64, 48, 0.0, 0.0), texture(64, 48, 3.0, 40.0));
    let e = flat_error(64, 48, 1.0);
    let out = blend(&a, &e, &b, &e, 0.0, &BlendParams::default());
    assert!(out.data.iter().zip(&a.data).all(|(x, y)| x.abs_diff(*y) <= 1));
    let same = blend(&a, &e, &a, &e, 0.4, &BlendParams::default());
    assert!(same.data.iter().zip(&a.data).all(|(x, y)| x.abs_diff(*y) <= 1));
}

#[test]
fn keeps_the_detail_that_a_fade_washes_out() {
    // B est la même texture décalée de 3 pixels et plus claire : un fondu à mi-chemin superpose les
    // deux et délave le détail ; le mélange (bascule nette) garde le détail d'une seule, et la
    // couleur moyenne du fondu. (Avec une bascule douce, des erreurs exactement égales comme ici
    // mélangent les détails à moitié.)
    let (a, b) = (texture(96, 64, 0.0, 0.0), texture(96, 64, 3.0, 40.0));
    let e = flat_error(96, 64, 1.0);
    let fade = blend(&a, &e, &b, &e, 0.5, &BlendParams { linear: true, ..BlendParams::default() });
    let mixed = blend(&a, &e, &b, &e, 0.5, &BlendParams { softness: 0.0, ..BlendParams::default() });
    let reference = detail(&a, 8, 88, 8, 56);
    let (d_fade, d_mixed) = (detail(&fade, 8, 88, 8, 56), detail(&mixed, 8, 88, 8, 56));
    assert!(d_fade < 0.8 * reference, "fondu {d_fade} / {reference}");
    assert!(d_mixed > 0.9 * reference, "mélange {d_mixed} / {reference}");
    assert!((mean(&mixed) - mean(&fade)).abs() < 3.0, "{} / {}", mean(&mixed), mean(&fade));
}

#[test]
fn each_region_takes_the_better_sequence() {
    // A est meilleure à gauche, B à droite : à mi-chemin, chaque moitié garde le détail de la
    // meilleure.
    let (w, h) = (96, 64);
    let (a, b) = (texture(w, h, 0.0, 0.0), texture(w, h, 3.0, 0.0));
    let ea = ErrorMap::new(&(0..w * h).map(|i| if i % w < w / 2 { 1.0 } else { 10.0 }).collect::<Vec<_>>(), w, h, 8);
    let eb = ErrorMap::new(&(0..w * h).map(|i| if i % w < w / 2 { 10.0 } else { 1.0 }).collect::<Vec<_>>(), w, h, 8);
    let out = blend(&a, &ea, &b, &eb, 0.5, &BlendParams::default());
    let close = |x0: usize, x1: usize, im: &Image| {
        let (mut s, mut n) = (0u32, 0u32);
        for y in 8..h - 8 {
            for x in x0..x1 {
                let p = y * w + x;
                let (go, gi) = (out.data[p + 1] as i32 - out.data[p] as i32, im.data[p + 1] as i32 - im.data[p] as i32);
                s += go.abs_diff(gi);
                n += 1;
            }
        }
        s as f32 / n as f32
    };
    // (L'attache au fondu tire un peu vers l'autre image : environ 2 sur des écarts de 40.)
    assert!(close(8, 36, &a) < 4.0 && close(8, 36, &b) > 20.0, "gauche : {} / {}", close(8, 36, &a), close(8, 36, &b));
    assert!(close(60, 88, &b) < 4.0 && close(60, 88, &a) > 20.0, "droite : {} / {}", close(60, 88, &b), close(60, 88, &a));
}
