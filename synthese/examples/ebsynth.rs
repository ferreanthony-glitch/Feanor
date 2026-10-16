//! Le portage avec la même ligne de commande qu'ebsynth, pour comparer sur ses exemples :
//!
//! cargo run --release --example ebsynth -- -style style.png -guide src.png tgt.png -output out.png
//!
//! Options d'ebsynth : -style, -guide, -weight (pour le style ou le guide qui précède), -output,
//! -uniformity, -patchsize, -pyramidlevels, -searchvoteiters, -patchmatchiters, -extrapass3x3,
//! -stopthreshold (pour la carte graphique seulement, comme ebsynth), -backend cpu|gpu (gpu : avec
//! la fonction « gpu », `--features gpu`). En plus : -threads <n> et -seed <n>.

use std::process::ExitCode;
use std::time::Instant;

use synthese::{max_pyramid_levels, Cpu, GuidePair, Image, Params, Synthesizer};

struct Guide {
    source: String,
    target: String,
    weight: Option<f32>,
}

fn load(path: &str) -> Result<(usize, usize, Vec<u8>), String> {
    let im = image::open(path).map_err(|e| format!("impossible de lire '{path}' : {e}"))?.to_rgba8();
    Ok((im.width() as usize, im.height() as usize, im.into_raw()))
}

fn run(args: &[String]) -> Result<(), String> {
    let mut style: Option<String> = None;
    let mut style_weight: Option<f32> = None;
    let mut guides: Vec<Guide> = Vec::new();
    let mut output = "output.png".to_string();
    let mut params = Params::default();
    let mut backend = "cpu".to_string();
    // Le dernier -style ou -guide lu : c'est à lui que s'applique -weight.
    let mut last_is_style: Option<bool> = None;

    let mut it = args.iter();
    let value = |name: &str, it: &mut std::slice::Iter<String>| {
        it.next().cloned().ok_or_else(|| format!("il manque la valeur de {name}"))
    };
    fn number<T: std::str::FromStr>(name: &str, v: String) -> Result<T, String> {
        v.parse().map_err(|_| format!("valeur incorrecte pour {name} : '{v}'"))
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-style" => {
                style = Some(value("-style", &mut it)?);
                style_weight = None;
                last_is_style = Some(true);
            }
            "-guide" => {
                let source = value("-guide", &mut it)?;
                let target = value("-guide", &mut it)?;
                guides.push(Guide { source, target, weight: None });
                last_is_style = Some(false);
            }
            "-output" => output = value("-output", &mut it)?,
            "-weight" => {
                let w: f32 = number("-weight", value("-weight", &mut it)?)?;
                if w < 0.0 {
                    return Err("les poids doivent être positifs ou nuls".into());
                }
                match last_is_style {
                    Some(true) => style_weight = Some(w),
                    Some(false) => guides.last_mut().expect("guide").weight = Some(w),
                    None => return Err("-weight doit suivre un -style ou un -guide".into()),
                }
            }
            "-uniformity" => params.uniformity = number("-uniformity", value("-uniformity", &mut it)?)?,
            "-patchsize" => params.patch_size = number("-patchsize", value("-patchsize", &mut it)?)?,
            "-pyramidlevels" => {
                params.pyramid_levels = Some(number("-pyramidlevels", value("-pyramidlevels", &mut it)?)?)
            }
            "-searchvoteiters" => {
                params.search_vote_iters = number("-searchvoteiters", value("-searchvoteiters", &mut it)?)?
            }
            "-patchmatchiters" => {
                params.patchmatch_iters = number("-patchmatchiters", value("-patchmatchiters", &mut it)?)?
            }
            "-stopthreshold" => {
                params.stop_threshold = number("-stopthreshold", value("-stopthreshold", &mut it)?)?;
            }
            "-extrapass3x3" => params.extra_pass_3x3 = true,
            "-backend" => {
                backend = value("-backend", &mut it)?;
                if backend != "cpu" && (backend != "gpu" || !cfg!(feature = "gpu")) {
                    return Err(format!("calcul '{backend}' inconnu (cpu, ou gpu avec --features gpu)"));
                }
            }
            "-threads" => params.threads = number("-threads", value("-threads", &mut it)?)?,
            "-seed" => params.seed = number("-seed", value("-seed", &mut it)?)?,
            other => return Err(format!("option inconnue '{other}'")),
        }
    }
    let style_path = style.ok_or("il faut une image -style")?;

    // Comme ebsynth : chaque image ne garde que les canaux utiles (gris, couleur, transparence).
    let (sw, sh, rgba) = load(&style_path)?;
    let style = Image::from_rgba(sw, sh, &rgba, Image::channels_needed(&rgba));
    let mut pairs = Vec::new();
    for g in &guides {
        let (gw, gh, src) = load(&g.source)?;
        let (tw, th, tgt) = load(&g.target)?;
        let channels = Image::channels_needed(&src).max(Image::channels_needed(&tgt));
        pairs.push((Image::from_rgba(gw, gh, &src, channels), Image::from_rgba(tw, th, &tgt, channels)));
    }
    let n = guides.len().max(1) as f32;
    let refs: Vec<GuidePair> = pairs
        .iter()
        .zip(&guides)
        .map(|((s, t), g)| GuidePair { source: s, target: t, weight: g.weight.unwrap_or(1.0 / n) })
        .collect();

    let levels = refs.first().map_or(0, |g| {
        let max = max_pyramid_levels((sw, sh), (g.target.width, g.target.height), params.patch_size);
        params.pyramid_levels.unwrap_or(max).clamp(1, max.max(1))
    });
    println!("uniformity: {:.0}", params.uniformity);
    println!("patchsize: {}", params.patch_size);
    println!("pyramidlevels: {levels}");
    println!("searchvoteiters: {}", params.search_vote_iters);
    println!("patchmatchiters: {}", params.patchmatch_iters);
    println!("extrapass3x3: {}", if params.extra_pass_3x3 { "yes" } else { "no" });
    println!("channels: style {}, guides {}", style.channels, pairs.iter().map(|p| p.0.channels).sum::<usize>());

    let synth: Box<dyn Synthesizer> = match backend.as_str() {
        #[cfg(feature = "gpu")]
        "gpu" => {
            let start = Instant::now();
            let gpu = synthese::gpu::Gpu::new()?;
            println!("backend: gpu, {} (ouverte en {:.2} s)", gpu.name(), start.elapsed().as_secs_f32());
            Box::new(gpu)
        }
        _ => {
            println!("backend: cpu");
            Box::new(Cpu)
        }
    };
    let start = Instant::now();
    let out = synth.synthesize(&style, style_weight.unwrap_or(1.0), &refs, &params).map_err(|e| e.to_string())?;
    println!("time: {:.2} s", start.elapsed().as_secs_f32());

    let im = &out.image;
    let color = match im.channels {
        1 => image::ExtendedColorType::L8,
        2 => image::ExtendedColorType::La8,
        3 => image::ExtendedColorType::Rgb8,
        _ => image::ExtendedColorType::Rgba8,
    };
    image::save_buffer(&output, &im.data, im.width as u32, im.height as u32, color)
        .map_err(|e| format!("impossible d'écrire '{output}' : {e}"))?;
    println!("result was written to {output}");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("usage : ebsynth -style <style.png> -guide <source.png> <cible.png> [-weight <poids>] … -output <sortie.png>");
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
