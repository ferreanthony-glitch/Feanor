//! Mesurer la stabilité d'une propagation :
//!
//! cargo run --release --example scintillement -- <images vidéo> <images fabriquées> <de> <à>
//!
//! Pour chaque image t de la plage (numéros à partir de 1, comme dans `propager`), l'image t − 1
//! est déplacée par le flux optique de la vidéo puis comparée à l'image t : écart moyen par canal
//! (0..255) pour la vidéo elle-même (ce que le flux n'explique pas : occultations, flou, erreurs)
//! et pour les images fabriquées. Un écart nettement plus grand que celui de la vidéo veut dire
//! que la peinture scintille ou glisse.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use synthese::flow::{flow, FlowParams, Gray};
use synthese::propagation::warp;
use synthese::Image;

fn files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{} : {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| matches!(p.extension().and_then(|x| x.to_str()).map(|x| x.to_ascii_lowercase()).as_deref(), Some("jpg" | "jpeg" | "png")))
        .collect();
    v.sort();
    Ok(v)
}

fn load(path: &Path) -> Result<Image, String> {
    let im = image::open(path).map_err(|e| format!("{} : {e}", path.display()))?.to_rgb8();
    Ok(Image::new(im.width() as usize, im.height() as usize, 3, im.into_raw()))
}

fn mean_diff(a: &Image, b: &Image) -> f64 {
    a.data.iter().zip(&b.data).map(|(x, y)| x.abs_diff(*y) as f64).sum::<f64>() / a.data.len() as f64
}

/// L'image fabriquée numéro `n` (même numéro que l'image de la vidéo, quel que soit le nom).
fn made(dir: &[PathBuf], n: usize) -> Option<&PathBuf> {
    dir.iter().find(|p| p.file_stem().and_then(|s| s.to_str()).and_then(|s| s.trim_start_matches('0').parse::<usize>().ok()) == Some(n))
}

fn run(args: &[String]) -> Result<(), String> {
    let [video, out, from, to] = args else {
        return Err("usage : scintillement <images vidéo> <images fabriquées> <de> <à>".into());
    };
    let (video, out) = (files(Path::new(video))?, files(Path::new(out))?);
    let (from, to): (usize, usize) = (from.parse().map_err(|_| "de ?")?, to.parse().map_err(|_| "à ?")?);
    let (mut sum_v, mut sum_o, mut n) = (0.0, 0.0, 0);
    for t in from + 1..=to.min(video.len()) {
        let (Some(o0), Some(o1)) = (made(&out, t - 1), made(&out, t)) else { continue };
        let (v0, v1) = (load(&video[t - 2])?, load(&video[t - 1])?);
        let back = flow(&Gray::from_image(&v1), &Gray::from_image(&v0), &FlowParams::default());
        let dv = mean_diff(&warp(&v0, &back), &v1);
        let d_o = mean_diff(&warp(&load(o0)?, &back), &load(o1)?);
        println!("image {t} : vidéo {dv:.2}   fabriquée {d_o:.2}");
        (sum_v, sum_o, n) = (sum_v + dv, sum_o + d_o, n + 1);
    }
    if n == 0 {
        return Err("aucune paire d'images à comparer".into());
    }
    println!("moyenne : vidéo {:.2}   fabriquée {:.2}", sum_v / n as f64, sum_o / n as f64);
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("erreur : {e}");
            ExitCode::from(1)
        }
    }
}
