//! Un exemple de chaque export V1, écrit depuis une copie du projet d'essai : pour regarder les résultats à la main (lecteur vidéo, lecteur de
//! PDF, logiciel de montage). Sauté d'office : il lit deux variables d'environnement.
//!
//! ```text
//! SHOT_EXEMPLES_PROJET = le fichier .pelure d'une COPIE du projet d'essai
//! SHOT_EXEMPLES_SORTIE = un dossier (créé s'il manque) où tout s'écrit
//! cargo test --release --offline --target-dir target\msvc --lib ecrire_des_exemples -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::editorial::{self, Fcpxml, Media, Options};
use crate::editorialjob::EditorialJob;
use crate::export::{self, ExportJob, Format, GifSettings, Range, SoundSpan};
use crate::exports_v1_tests::court;
use crate::exportcamera::{self, CameraSettings};
use crate::exportlayers::{self, LayersSettings};
use crate::exportstems::{self, AudioFormat, Container, Depth, StemsSettings};
use crate::exportstills::{self, StillFormat, StillsPer, StillsSettings};
use crate::exportvideo::{self, BurnSettings, VideoRate, VideoSize, VideoSpec};
use crate::fxcache::monde;
use crate::pdfmise::{Paper, PdfLayout};

fn variable(nom: &str) -> Option<PathBuf> {
    std::env::var_os(nom).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

#[test]
#[ignore = "écrit des exemples dans SHOT_EXEMPLES_SORTIE depuis le projet SHOT_EXEMPLES_PROJET (voir l'en-tête du fichier)"]
fn ecrire_des_exemples_de_chaque_export() {
    let (Some(projet), Some(sortie)) = (variable("SHOT_EXEMPLES_PROJET"), variable("SHOT_EXEMPLES_SORTIE")) else {
        eprintln!("SAUTÉ : SHOT_EXEMPLES_PROJET et SHOT_EXEMPLES_SORTIE ne sont pas définis");
        return;
    };
    monde::lire();
    let loaded = crate::project::load(&projet).unwrap_or_else(|e| panic!("ouverture de {} : {e}", projet.display()));
    let doc = loaded.doc;
    std::fs::create_dir_all(&sortie).unwrap();
    let ffmpeg = export::find_ffmpeg(None).and_then(|f| export::probe(&f).ok().map(|e| (f, e)));
    let fps = doc.fps;
    let frames = export::frames_for(&doc, Range::Scene, 0);
    println!("projet : {} plans, {} images, {} i/s, cadre {:?} ; exemples : la première séquence ({} images)", doc.panels.len(), doc.total_frames(), fps, doc.frame, frames.len());
    let first = doc.panel_start(frames[0].0) + frames[0].1;
    let (start, len) = export::sample_span(fps, first, frames.len());
    let a_du_son = export::has_sound(&crate::audio::mix_clips(&doc), start, len);
    let base = projet.file_stem().map_or("projet".to_string(), |s| s.to_string_lossy().into_owned());
    let dire = |quoi: &str, chemin: &Path, resultat: &Option<Result<String, String>>| println!("{quoi} : {} -> {resultat:?}", chemin.display());

    // Les films.
    if let Some((ff, enc)) = &ffmpeg {
        let burn = BurnSettings { enabled: true, plan: true, timecode: true, sequence: true, texts: true };
        let cas = [(Format::Mp4, "film.mp4", false), (Format::ProRes, "film_prores422hq.mov", false), (Format::ProRes4444, "film_prores4444_transparent.mov", false), (Format::DnxHr, "film_dnxhr.mov", false), (Format::DnxHr, "film_dnxhr.mxf", true)];
        for (format, nom, mxf) in cas {
            if let Some(r) = exportvideo::missing_reason(format, enc) {
                println!("{nom} : GRISÉ ({r})");
                continue;
            }
            let out = sortie.join(nom);
            let (frame, size) = (doc.frame, VideoSize::Hd720.dimensions(doc.frame));
            let enc2 = enc.clone();
            let cible = out.clone();
            let spec = move |wav: Option<&Path>| exportvideo::video_args(&VideoSpec { format, enc: &enc2, frame, out_size: size, rate_in: fps.to_string(), rate_out: VideoRate::F25.conversion(fps).map(str::to_string), audio: wav, target: &cible, mxf });
            let span = a_du_son.then_some(SoundSpan { start, len });
            let mut job = ExportJob::video_with_sound_alpha(ff, spec, &out, frames.clone(), span, format.has_alpha()).avec_infos(burn);
            court(&mut job, &doc);
            dire(nom, &out, &job.outcome);
        }
        let out = sortie.join("film.gif");
        let rate = export::gif_fps(12, fps);
        let kept = export::gif_sample(&frames, fps, rate);
        let args = export::gif_args(doc.frame, rate, &GifSettings::default(), &out);
        let mut job = ExportJob::gif(ff, args, &out, kept).expect("gif");
        court(&mut job, &doc);
        dire("film.gif", &out, &job.outcome);
    } else {
        println!("films : SAUTÉS (aucun ffmpeg)");
    }

    // Images.
    let dir = sortie.join("images_png");
    std::fs::create_dir_all(&dir).unwrap();
    let mut job = ExportJob::images(&dir, &base, frames.iter().copied().take(6).collect(), doc.frame, true);
    court(&mut job, &doc);
    dire("images PNG (6 premières images)", &dir, &job.outcome);
    let plans = export::panels_of(&frames);
    for (nom, per, format, transparent) in [("images_par_plan_png", StillsPer::Plan, StillFormat::Png, true), ("images_par_case_jpeg", StillsPer::Case, StillFormat::Jpeg, false)] {
        let dir = sortie.join(nom);
        std::fs::create_dir_all(&dir).unwrap();
        let items = exportstills::items(&doc, &plans, per);
        let s = StillsSettings { per, format, transparent, size: VideoSize::Hd720, quality: 92 };
        let mut job = exportstills::job(&dir, &items, doc.frame, &s, false);
        court(&mut job, &doc);
        dire(nom, &dir, &job.outcome);
    }

    // Storyboard PDF : deux mises en page.
    let (shots, boards) = export::storyboard_of_with(&doc, &plans, false);
    for (nom, layout) in [("storyboard_A4_paysage_6.pdf", PdfLayout::default()), ("storyboard_A3_portrait_4.pdf", PdfLayout { paper: Paper::A3, landscape: false, per_page: 4, ..PdfLayout::default() })] {
        let out = sortie.join(nom);
        let mut job = ExportJob::pdf_layout(&out, &base, shots.clone(), boards.clone(), doc.frame, layout);
        court(&mut job, &doc);
        dire(nom, &out, &job.outcome);
    }

    // Son.
    let out = sortie.join("son_mixe.wav");
    let mut job = ExportJob::sound(&out, SoundSpan { start, len });
    court(&mut job, &doc);
    dire("son mixé", &out, &job.outcome);
    for (nom, format) in [("son_par_piste_wav16", AudioFormat::default()), ("son_par_piste_aiff24", AudioFormat { container: Container::Aiff, depth: Depth::Bits24 })] {
        let dir = sortie.join(nom);
        std::fs::create_dir_all(&dir).unwrap();
        let s = StemsSettings { format, with_mix: true };
        // (Le projet d'essai n'a pas de son : sans piste, l'exemple vient d'un petit film avec deux pistes.)
        let (sdoc, sfirst, sframes) = if exportstems::plan(&doc, first, frames.len(), &s, &base).is_empty() {
            let d = crate::editorial_tests::film_with_sound(24);
            let n = d.total_frames();
            (d, 0, n)
        } else {
            (crate::project::load(&projet).expect("projet").doc, first, frames.len())
        };
        let (sstart, slen) = export::sample_span(sdoc.fps, sfirst, sframes);
        let files = exportstems::plan(&sdoc, sfirst, sframes, &s, &base);
        println!("{nom} : {} fichiers {:?}", files.len(), files.iter().map(|f| f.file.as_str()).collect::<Vec<_>>());
        if files.is_empty() {
            continue;
        }
        let d = dir.clone();
        let mut job = ExportJob::task(nom.into(), &dir, "ok".into(), move |snap, cancel| exportstems::write_all(snap, &d, &files, sstart, slen, format, cancel).map(|_| ()));
        court(&mut job, &sdoc);
        dire(nom, &dir, &job.outcome);
    }

    // Montage : EDL, XML, FCPXML, OTIO, avec le son et les marqueurs.
    if let Some((ff, enc)) = &ffmpeg {
        let media = if enc.prores { Media::ProRes } else if enc.h264.is_some() { Media::H264 } else { Media::ProRes };
        let o = Options { fcpxml: true, fcpxml_version: Fcpxml::V1_10, media, half: true, whole_film: false, ..Options::default() };
        match editorial::build_plan(&doc, &o, 0, "Exemple montage") {
            Ok(plan) => match EditorialJob::start(plan, o, ff.clone(), enc.clone(), &sortie) {
                Ok(mut job) => {
                    while job.running() {
                        job.step(&doc, Duration::from_millis(20));
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    let fichiers = match &job.outcome {
                        Some(Ok(r)) => r.files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>(),
                        other => vec![format!("{other:?}")],
                    };
                    println!("montage : {} -> {fichiers:?}", job.dir.display());
                }
                Err(e) => println!("montage : {e}"),
            },
            Err(e) => println!("montage : {e}"),
        }
    }

    // Calques séparés (le premier plan) et caméra.
    let dir = sortie.join("calques_separes");
    std::fs::create_dir_all(&dir).unwrap();
    let un_plan = export::frames_for(&doc, Range::Panel, 0);
    let info = exportlayers::plans_info(&doc, &[0], false);
    let mut job = exportlayers::job(&dir, info, un_plan, &LayersSettings::default(), false);
    court(&mut job, &doc);
    dire("calques séparés (le premier plan)", &dir, &job.outcome);
    let out = sortie.join("camera.json");
    let s = CameraSettings::default();
    let target = out.clone();
    let tous: Vec<usize> = (0..doc.panels.len()).collect();
    let mut job = ExportJob::task("camera".into(), &out, "ok".into(), move |snap, _| {
        let e = exportcamera::build(snap, &tous, "Exemple", &s);
        exportcamera::write(&e, &target, &s).map(|_| ())
    });
    court(&mut job, &doc);
    dire("caméra", &out, &job.outcome);
}
