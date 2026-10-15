//! Propager une image clé peinte sur les images d'une vidéo :
//!
//! cargo run --release --example propager -- -images <dossier> -cle <n> <cle.png> -sortie <dossier>
//!
//! `-images` : un dossier d'images numérotées (JPEG ou PNG), comme celui que Pelure remplit en
//! important une vidéo (`videos/<id>/000001.jpg`…), dans l'ordre des noms. Les numéros comptent
//! à partir de 1 (le premier fichier). `-cle <n> <cle.png>` : l'image n peinte, de la même taille.
//! Options : `-de <n>` et `-a <n>` (plage, par défaut toute la vidéo), `-poids <clé> <vidéo>
//! <position> <temporel>` (par défaut 1 4 10 1, ceux d'EbSynth), `-uniformity`, `-patchsize`,
//! `-threads`, `-seed`, `-flux <dossier>` (écrit aussi le flux optique de chaque image en
//! couleurs : la teinte donne la direction, l'intensité la longueur).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use synthese::flow::{flow, Flow, Gray};
use synthese::propagation::{propagate, PropagateParams};
use synthese::Image;

fn frames_in(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{} : {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            matches!(ext.as_str(), "jpg" | "jpeg" | "png")
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("aucune image dans {}", dir.display()));
    }
    Ok(files)
}

fn load_rgba(path: &Path) -> Result<(usize, usize, Vec<u8>), String> {
    let im = image::open(path).map_err(|e| format!("{} : {e}", path.display()))?.to_rgba8();
    Ok((im.width() as usize, im.height() as usize, im.into_raw()))
}

fn save(path: &Path, im: &Image) -> Result<(), String> {
    let color = match im.channels {
        1 => image::ExtendedColorType::L8,
        2 => image::ExtendedColorType::La8,
        3 => image::ExtendedColorType::Rgb8,
        _ => image::ExtendedColorType::Rgba8,
    };
    image::save_buffer(path, &im.data, im.width as u32, im.height as u32, color).map_err(|e| format!("{} : {e}", path.display()))
}

/// Flux en couleurs : teinte = direction, intensité = longueur (blanc = immobile).
fn flow_colors(f: &Flow) -> Image {
    let max = f.data.iter().map(|u| (u[0] * u[0] + u[1] * u[1]).sqrt()).fold(1.0f32, f32::max);
    let data = f
        .data
        .iter()
        .flat_map(|u| {
            let len = (u[0] * u[0] + u[1] * u[1]).sqrt() / max;
            let hue = (u[1].atan2(u[0]) / std::f32::consts::TAU + 1.0).fract() * 6.0;
            let x = 1.0 - (hue % 2.0 - 1.0).abs();
            let (r, g, b) = match hue as u32 {
                0 => (1.0, x, 0.0),
                1 => (x, 1.0, 0.0),
                2 => (0.0, 1.0, x),
                3 => (0.0, x, 1.0),
                4 => (x, 0.0, 1.0),
                _ => (1.0, 0.0, x),
            };
            [r, g, b].map(|v| (255.0 * (1.0 - len + len * v)) as u8)
        })
        .collect();
    Image::new(f.width, f.height, 3, data)
}

fn run(args: &[String]) -> Result<(), String> {
    let (mut images, mut key, mut output, mut flow_dir) = (None, None, None, None);
    let (mut from, mut to) = (None, None);
    let mut params = PropagateParams::default();
    let mut it = args.iter();
    let next = |it: &mut std::slice::Iter<String>, name: &str| it.next().cloned().ok_or(format!("il manque la valeur de {name}"));
    fn number<T: std::str::FromStr>(v: String, name: &str) -> Result<T, String> {
        v.parse().map_err(|_| format!("valeur incorrecte pour {name} : '{v}'"))
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-images" => images = Some(PathBuf::from(next(&mut it, arg)?)),
            "-cle" => {
                let n: usize = number(next(&mut it, arg)?, arg)?;
                key = Some((n, PathBuf::from(next(&mut it, arg)?)));
            }
            "-sortie" => output = Some(PathBuf::from(next(&mut it, arg)?)),
            "-de" => from = Some(number::<usize>(next(&mut it, arg)?, arg)?),
            "-a" => to = Some(number::<usize>(next(&mut it, arg)?, arg)?),
            "-poids" => {
                let w = &mut params.weights;
                w.style = number(next(&mut it, arg)?, arg)?;
                w.video = number(next(&mut it, arg)?, arg)?;
                w.position = number(next(&mut it, arg)?, arg)?;
                w.temporal = number(next(&mut it, arg)?, arg)?;
            }
            "-uniformity" => params.synth.uniformity = number(next(&mut it, arg)?, arg)?,
            "-patchsize" => params.synth.patch_size = number(next(&mut it, arg)?, arg)?,
            "-threads" => {
                params.synth.threads = number(next(&mut it, arg)?, arg)?;
                params.flow.threads = params.synth.threads;
            }
            "-seed" => params.synth.seed = number(next(&mut it, arg)?, arg)?,
            "-flux" => flow_dir = Some(PathBuf::from(next(&mut it, arg)?)),
            other => return Err(format!("option inconnue '{other}'")),
        }
    }
    let files = frames_in(&images.ok_or("il faut -images <dossier>")?)?;
    let (key_n, key_path) = key.ok_or("il faut -cle <n> <cle.png>")?;
    let output = output.ok_or("il faut -sortie <dossier>")?;
    std::fs::create_dir_all(&output).map_err(|e| format!("{} : {e}", output.display()))?;
    let (first, last) = (from.unwrap_or(1).max(1), to.unwrap_or(files.len()).min(files.len()));
    if !(first..=last).contains(&key_n) {
        return Err(format!("la clé {key_n} n'est pas dans la plage {first}..{last}"));
    }
    let (kw, kh, rgba) = load_rgba(&key_path)?;
    let style = Image::from_rgba(kw, kh, &rgba, Image::channels_needed(&rgba));
    println!("{} images, clé {key_n}, plage {first} à {last}, {kw}×{kh}", files.len());

    let load = |i: usize| -> Result<Image, String> {
        let (w, h, rgba) = load_rgba(&files[i])?;
        Ok(Image::from_rgba(w, h, &rgba, 3))
    };
    if let Some(dir) = &flow_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        for t in first..=last {
            if t == key_n {
                continue;
            }
            let prev = if t > key_n { t - 1 } else { t + 1 };
            let f = flow(&Gray::from_image(&load(t - 1)?), &Gray::from_image(&load(prev - 1)?), &params.flow);
            save(&dir.join(format!("{t:06}.png")), &flow_colors(&f))?;
        }
    }
    let start = Instant::now();
    let mut last_time = Instant::now();
    let mut failed = None;
    let result = propagate(load, key_n - 1, &style, first - 1..last, &params, |i, im| {
        if let Err(e) = save(&output.join(format!("{:06}.png", i + 1)), im) {
            failed = Some(e);
            return false;
        }
        println!("image {} : {:.2} s", i + 1, last_time.elapsed().as_secs_f32());
        last_time = Instant::now();
        true
    });
    if let Some(e) = failed {
        return Err(e);
    }
    result.map_err(|e| e.to_string())?;
    println!("fini en {:.1} s", start.elapsed().as_secs_f32());
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("usage : propager -images <dossier> -cle <n> <cle.png> -sortie <dossier> [-de <n>] [-a <n>] [-poids <clé> <vidéo> <position> <temporel>] [-flux <dossier>]");
        return ExitCode::from(1);
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("erreur : {e}");
            ExitCode::from(1)
        }
    }
}
