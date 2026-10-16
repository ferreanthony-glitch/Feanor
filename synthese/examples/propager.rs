//! Propager des images clés peintes sur les images d'une vidéo :
//!
//! cargo run --release --example propager -- -images <dossier> -cle <n> <cle.png> [-cle …] -sortie <dossier>
//!
//! `-images` : un dossier d'images numérotées (JPEG ou PNG), comme celui que Pelure remplit en
//! important une vidéo (`videos/<id>/000001.jpg`…), dans l'ordre des noms. Les numéros comptent
//! à partir de 1 (le premier fichier). `-cle <n> <cle.png>` : l'image n peinte, de la même taille ;
//! autant de clés qu'on veut, mélangées entre elles.
//! Options : `-de <n>` et `-a <n>` (plage, par défaut toute la vidéo), `-poids <clé> <vidéo>
//! <position> <temporel>` (par défaut 1 4 10 1, ceux d'EbSynth), `-uniformity`, `-patchsize`,
//! `-threads`, `-seed`, `-attache <poids>` (du fondu enchaîné dans le mélange, 0,1 par défaut),
//! `-douceur <largeur>` (de la bascule d'une clé à l'autre, voir `BlendParams::softness`),
//! `-fondu` (simple fondu enchaîné entre les clés, pour comparer), `-gpu` (synthèse sur la carte
//! graphique, avec `--features gpu`), `-flux <dossier>` (écrit aussi
//! le flux optique de chaque image en couleurs : la teinte donne la direction, l'intensité la
//! longueur).

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use synthese::blend::{BlendParams, ErrorMap};
use synthese::flow::{flow, Flow, Gray};
use synthese::propagation::{propagate_keys_on, Key, PropagateParams};
use synthese::{Cpu, Error, GuidePair, Image, Output, Params, Synthesizer};

/// La synthèse choisie, chronométrée, ainsi que le mélange (le reste du temps : flux optique quand
/// il n'est pas caché derrière la synthèse, fichiers).
struct Timed {
    inner: Box<dyn Synthesizer>,
    spent: Cell<Duration>,
    blending: Cell<Duration>,
}

impl Synthesizer for Timed {
    fn synthesize(&self, style: &Image, w: f32, guides: &[GuidePair], params: &Params) -> Result<Output, Error> {
        let start = Instant::now();
        let out = self.inner.synthesize(style, w, guides, params);
        self.spent.set(self.spent.get() + start.elapsed());
        out
    }

    fn blend(&self, a: &Image, ea: &ErrorMap, b: &Image, eb: &ErrorMap, alpha: f32, params: &BlendParams) -> Image {
        let start = Instant::now();
        let out = self.inner.blend(a, ea, b, eb, alpha, params);
        self.blending.set(self.blending.get() + start.elapsed());
        out
    }
}

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
    let (mut images, mut output, mut flow_dir) = (None, None, None);
    let mut on_gpu = false;
    let mut keys: Vec<(usize, PathBuf)> = Vec::new();
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
                keys.push((n, PathBuf::from(next(&mut it, arg)?)));
            }
            "-attache" => params.blend.screen = number(next(&mut it, arg)?, arg)?,
            "-douceur" => params.blend.softness = number(next(&mut it, arg)?, arg)?,
            "-fondu" => params.blend.linear = true,
            "-gpu" => on_gpu = true,
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
    if keys.is_empty() {
        return Err("il faut au moins une -cle <n> <cle.png>".into());
    }
    let output = output.ok_or("il faut -sortie <dossier>")?;
    std::fs::create_dir_all(&output).map_err(|e| format!("{} : {e}", output.display()))?;
    let (first, last) = (from.unwrap_or(1).max(1), to.unwrap_or(files.len()).min(files.len()));
    if let Some((n, _)) = keys.iter().find(|(n, _)| !(first..=last).contains(n)) {
        return Err(format!("la clé {n} n'est pas dans la plage {first}..{last}"));
    }
    let mut styles = Vec::new();
    for (_, path) in &keys {
        let (kw, kh, rgba) = load_rgba(path)?;
        styles.push(Image::from_rgba(kw, kh, &rgba, Image::channels_needed(&rgba)));
    }
    let numbers: Vec<String> = keys.iter().map(|(n, _)| n.to_string()).collect();
    println!("{} images, clés {}, plage {first} à {last}, {}×{}", files.len(), numbers.join(", "), styles[0].width, styles[0].height);

    let load = |i: usize| -> Result<Image, String> {
        let (w, h, rgba) = load_rgba(&files[i])?;
        Ok(Image::from_rgba(w, h, &rgba, 3))
    };
    if let Some(dir) = &flow_dir {
        // Flux de chaque image vers la précédente.
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        let (mut spent, mut n) = (Duration::ZERO, 0);
        for t in first + 1..=last {
            let (a, b) = (Gray::from_image(&load(t - 1)?), Gray::from_image(&load(t - 2)?));
            let start = Instant::now();
            let f = flow(&a, &b, &params.flow);
            (spent, n) = (spent + start.elapsed(), n + 1);
            save(&dir.join(format!("{t:06}.png")), &flow_colors(&f))?;
        }
        println!("flux optique : {:.3} s par image", spent.as_secs_f32() / n.max(1) as f32);
    }
    let inner: Box<dyn Synthesizer> = if on_gpu {
        #[cfg(feature = "gpu")]
        {
            let gpu = synthese::gpu::Gpu::new()?;
            println!("carte graphique : {}", gpu.name());
            Box::new(gpu)
        }
        #[cfg(not(feature = "gpu"))]
        return Err("-gpu demande --features gpu".into());
    } else {
        Box::new(Cpu)
    };
    let synth = Timed { inner, spent: Cell::new(Duration::ZERO), blending: Cell::new(Duration::ZERO) };
    let key_list: Vec<Key> = keys.iter().zip(&styles).map(|((n, _), style)| Key { frame: n - 1, style }).collect();
    let start = Instant::now();
    let mut last_time = Instant::now();
    let mut failed = None;
    let result = propagate_keys_on(&synth, load, &key_list, first - 1..last, &params, |i, im| {
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
    let total = start.elapsed().as_secs_f32();
    let (synth_time, blend_time) = (synth.spent.get().as_secs_f32(), synth.blending.get().as_secs_f32());
    println!(
        "fini en {total:.1} s : synthèse {synth_time:.1} s, mélange {blend_time:.1} s, le reste (flux, fichiers) {:.1} s",
        total - synth_time - blend_time
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("usage : propager -images <dossier> -cle <n> <cle.png> [-cle …] -sortie <dossier> [-de <n>] [-a <n>] [-poids <clé> <vidéo> <position> <temporel>] [-attache <poids>] [-fondu] [-flux <dossier>]");
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
