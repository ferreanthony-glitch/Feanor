//! Comparer deux résultats de synthèse (le portage contre l'original, ou deux passes) :
//!
//! cargo run --release --example ecart -- a.png b.png
//!     écart moyen par canal (0..255), écart après un flou 9×9 (la structure, sans le détail des
//!     patchs qui change d'une passe à l'autre) et part des pixels qui diffèrent de plus de 32.
//! cargo run --release --example ecart -- -montage sortie.png a.png b.png …
//!     les images côte à côte, pour les regarder.

use std::process::ExitCode;

use image::{GenericImage, RgbImage};

fn load(path: &str) -> Result<RgbImage, String> {
    Ok(image::open(path).map_err(|e| format!("impossible de lire '{path}' : {e}"))?.to_rgb8())
}

/// Flou en boîte (2r+1)², bords répétés.
fn blur(im: &RgbImage, r: i32) -> Vec<f32> {
    let (w, h) = (im.width() as i32, im.height() as i32);
    let mut out = vec![0f32; (w * h * 3) as usize];
    let n = ((2 * r + 1) * (2 * r + 1)) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut s = [0f32; 3];
            for dy in -r..=r {
                for dx in -r..=r {
                    let p = im.get_pixel((x + dx).clamp(0, w - 1) as u32, (y + dy).clamp(0, h - 1) as u32);
                    for k in 0..3 {
                        s[k] += p[k] as f32;
                    }
                }
            }
            for k in 0..3 {
                out[((y * w + x) * 3 + k as i32) as usize] = s[k] / n;
            }
        }
    }
    out
}

fn compare(a: &str, b: &str) -> Result<(), String> {
    let (ia, ib) = (load(a)?, load(b)?);
    if ia.dimensions() != ib.dimensions() {
        return Err("les deux images n'ont pas la même taille".into());
    }
    let (pa, pb) = (ia.as_raw(), ib.as_raw());
    let mean = pa.iter().zip(pb).map(|(x, y)| x.abs_diff(*y) as f64).sum::<f64>() / pa.len() as f64;
    let far = pa.chunks_exact(3).zip(pb.chunks_exact(3)).filter(|(p, q)| p.iter().zip(*q).any(|(x, y)| x.abs_diff(*y) > 32)).count();
    let (ba, bb) = (blur(&ia, 4), blur(&ib, 4));
    let blurred = ba.iter().zip(&bb).map(|(x, y)| (x - y).abs() as f64).sum::<f64>() / ba.len() as f64;
    println!(
        "écart moyen {mean:.2}   après flou 9×9 {blurred:.2}   pixels à plus de 32 : {:.1} %",
        100.0 * far as f64 / (pa.len() / 3) as f64
    );
    Ok(())
}

fn montage(out: &str, paths: &[String]) -> Result<(), String> {
    let images = paths.iter().map(|p| load(p)).collect::<Result<Vec<_>, _>>()?;
    let gap = 8;
    let w = images.iter().map(|i| i.width()).sum::<u32>() + gap * (images.len() as u32).saturating_sub(1);
    let h = images.iter().map(|i| i.height()).max().unwrap_or(1);
    let mut canvas = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    let mut x = 0;
    for im in &images {
        canvas.copy_from(im, x, 0).map_err(|e| e.to_string())?;
        x += im.width() + gap;
    }
    canvas.save(out).map_err(|e| format!("impossible d'écrire '{out}' : {e}"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [flag, out, rest @ ..] if flag == "-montage" && !rest.is_empty() => montage(out, rest),
        [a, b] => compare(a, b),
        _ => Err("usage : ecart a.png b.png   ou   ecart -montage sortie.png a.png b.png …".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("erreur : {e}");
            ExitCode::from(1)
        }
    }
}
