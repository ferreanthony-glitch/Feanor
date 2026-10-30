//! Essais des exports V1 (2026-10-30) : le film pro (tailles, cadences, formats, transparence, informations gravées), les images par
//! plan, le PDF à police embarquée, le son par piste, le montage (FCPXML, marqueurs, son dans l'EDL), les calques séparés et les clés
//! de caméra.
//!
//! Les essais qui ont besoin de ffmpeg le disent quand ils sont sautés (`SAUTÉ : …` sur la sortie d'erreur) : un vert silencieux
//! ne prouverait rien. Cette machine a celui de Krita (H.264 par Windows ou OpenH264, ProRes, DNx), qui sert à de vrais encodages.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::export::{self, Encoders, ExportJob, Format, SoundSpan};
use crate::exportvideo::{self, BurnSettings, VideoRate, VideoSize, VideoSpec};
use crate::fxcache::monde;

// ------------------------------------------------------------------------------------------ outils

pub(crate) fn dossier_neuf(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-exports-v1-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Fait tourner un export jusqu'au bout, comme l'interface (avec une limite de temps).
pub(crate) fn court(job: &mut ExportJob, doc: &Document) {
    let t = Instant::now();
    while job.outcome.is_none() {
        job.step(doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(180), "export trop long : {}", job.phase());
    }
}

/// ffmpeg et ses encodeurs, ou rien avec un message qui dit que l'essai est sauté.
pub(crate) fn ffmpeg_ou_saute(quoi: &str) -> Option<(PathBuf, Encoders)> {
    match export::find_ffmpeg(None).and_then(|f| export::probe(&f).ok().map(|e| (f, e))) {
        Some(x) => Some(x),
        None => {
            eprintln!("SAUTÉ : {quoi} : aucun ffmpeg trouvé sur cette machine");
            None
        }
    }
}

/// Le compte rendu de ffmpeg sur un fichier (flux, tailles, cadences).
fn decrire(ffmpeg: &Path, file: &Path) -> String {
    let out = std::process::Command::new(ffmpeg).args(["-hide_banner", "-nostdin", "-i"]).arg(file).output().expect("ffmpeg");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Les images d'un fichier vidéo, comptées par ffmpeg (copie vers rien).
fn compter_images(ffmpeg: &Path, file: &Path) -> usize {
    let out = std::process::Command::new(ffmpeg).args(["-hide_banner", "-nostdin", "-i"]).arg(file).args(["-map", "0:v:0", "-c", "copy", "-f", "null", "-"]).output().expect("ffmpeg");
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    let at = text.rfind("frame=").unwrap_or_else(|| panic!("pas de compte d'images dans :\n{text}"));
    text[at + 6..].trim_start().chars().take_while(char::is_ascii_digit).collect::<String>().parse().expect("nombre d'images")
}

/// La première image d'un film, en RVBA droit.
fn premiere_image(ffmpeg: &Path, file: &Path) -> Vec<u8> {
    let out = std::process::Command::new(ffmpeg).args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"]).arg(file).args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"]).output().expect("ffmpeg");
    out.stdout
}

/// Un projet de `n` plans de `len` images, 12 images par seconde, cadre 64×36, dont chaque plan est rempli de `couleur` (prémultipliée).
pub(crate) fn projet_uni(n: usize, len: usize, couleur: Color32) -> Document {
    projet_uni_de(n, len, couleur, 64, 36)
}

/// Comme `projet_uni`, avec un cadre de `w` × `h`.
pub(crate) fn projet_uni_de(n: usize, len: usize, couleur: Color32, w: usize, h: usize) -> Document {
    let mut doc = Document::with_frame(w, h, 12);
    // (Un plan est une scène : chaque plan a son numéro, PL001, PL002…)
    for _ in 1..n {
        let k = doc.panels.len() - 1;
        doc.add_scene_after(k);
    }
    for p in 0..n {
        let id = doc.new_cel(LayerKind::Bitmap);
        let (sw, sh) = (doc.width, doc.height);
        let r = &mut doc.cels.get_mut(&id).unwrap().raster;
        for y in 0..sh {
            for x in 0..sw {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = couleur;
            }
        }
        let mut l = Layer::new("Fond", LayerKind::Bitmap);
        l.frames = vec![Some(id); len];
        doc.panels[p].layers = vec![l];
        doc.panels[p].length = len;
    }
    doc
}

// ------------------------------------------------------------------------------------- film : tailles

#[test]
fn video_sizes_keep_the_proportions_and_stay_even() {
    let hd = [1920, 1080];
    assert_eq!(VideoSize::Project.dimensions(hd), [1920, 1080]);
    assert_eq!(VideoSize::Project.dimensions([1517, 959]), [1516, 958]);
    assert_eq!(VideoSize::Half.dimensions(hd), [960, 540]);
    assert_eq!(VideoSize::Uhd.dimensions(hd), [3840, 2160]);
    assert_eq!(VideoSize::Hd1080.dimensions(hd), [1920, 1080]);
    assert_eq!(VideoSize::Hd720.dimensions(hd), [1280, 720]);
    // Un cadre 4:3 : 1080p garde sa hauteur, 4K UHD sa largeur.
    assert_eq!(VideoSize::Hd1080.dimensions([1600, 1200]), [1440, 1080]);
    assert_eq!(VideoSize::Uhd.dimensions([1600, 1200]), [3840, 2880]);
    // Un cadre carré, une largeur libre impaire : toujours pair.
    assert_eq!(VideoSize::Width(1001).dimensions([1000, 1000]), [1002, 1002]);
    assert_eq!(VideoSize::Width(1280).dimensions(hd), [1280, 720]);
    assert_eq!(VideoSize::Width(1).dimensions(hd), [2, 2]);
    for size in VideoSize::CHOICES.into_iter().chain([VideoSize::Width(777)]) {
        for frame in [[1920, 1080], [1517, 959], [64, 36], [1000, 1000], [999, 3]] {
            let [w, h] = size.dimensions(frame);
            assert!(w % 2 == 0 && h % 2 == 0 && w >= 2 && h >= 2, "{size:?} {frame:?} -> {w}×{h}");
        }
    }
    // L'ancienne fonction dit la même chose.
    assert_eq!(export::video_size(hd, true), VideoSize::Half.dimensions(hd));
    assert_eq!(export::video_size([1517, 959], false), VideoSize::Project.dimensions([1517, 959]));
}

#[test]
fn video_rates_convert_only_when_they_differ() {
    assert_eq!(VideoRate::Project.conversion(24), None);
    assert_eq!(VideoRate::F24.conversion(24), None, "la même cadence : rien à convertir");
    assert_eq!(VideoRate::F23976.conversion(24), Some("24000/1001"));
    assert_eq!(VideoRate::F2997.conversion(24), Some("30000/1001"));
    assert_eq!(VideoRate::F25.conversion(12), Some("25"));
    assert!((VideoRate::F23976.fps(24) - 23.976).abs() < 1e-3);
    assert!((VideoRate::F2997.fps(24) - 29.97).abs() < 1e-2);
    assert_eq!(VideoRate::Project.fps(15), 15.0);
    let labels: Vec<&str> = VideoRate::CHOICES.iter().map(|r| r.label()).collect();
    assert_eq!(labels.len(), 8);
    assert!(labels.contains(&"23,976 images par seconde") && labels.contains(&"60 images par seconde"));
}

// --------------------------------------------------------------------------- film : arguments de ffmpeg

fn tous_les_encodeurs() -> Encoders {
    Encoders { h264: Some("libx264"), prores: true, openh264: false, dnxhd: true, aac: true }
}

fn args(format: Format, enc: &Encoders, size: VideoSize, rate: VideoRate, audio: Option<&Path>, mxf: bool) -> Result<String, String> {
    let frame = [1920, 1080];
    exportvideo::video_args(&VideoSpec {
        format,
        enc,
        frame,
        out_size: size.dimensions(frame),
        rate_in: "24".into(),
        rate_out: rate.conversion(24).map(str::to_string),
        audio,
        target: Path::new("C:/film/essai.mov"),
        mxf,
    })
    .map(|a| a.join(" "))
}

#[test]
fn ffmpeg_arguments_per_format() {
    let enc = tous_les_encodeurs();
    let wav = Path::new("C:/temp/son.wav");
    // MP4 : libx264 à crf 18, son AAC.
    let a = args(Format::Mp4, &enc, VideoSize::Project, VideoRate::Project, Some(wav), false).unwrap();
    assert!(a.contains("-c:v libx264 -preset medium -crf 18") && a.contains("format=yuv420p") && a.contains("-c:a aac"), "{a}");
    // ProRes 422 HQ.
    let a = args(Format::ProRes, &enc, VideoSize::Project, VideoRate::Project, None, false).unwrap();
    assert!(a.contains("-c:v prores_ks -profile:v 3") && a.contains("format=yuv422p10le") && !a.contains("alpha_bits"), "{a}");
    // ProRes 4444 avec transparence : alpha 16 bits, 4:4:4 avec canal alpha.
    let a = args(Format::ProRes4444, &enc, VideoSize::Project, VideoRate::Project, Some(wav), false).unwrap();
    assert!(a.contains("-c:v prores_ks -profile:v 4444 -alpha_bits 16") && a.contains("format=yuva444p10le") && a.contains("-c:a pcm_s16le"), "{a}");
    // DNxHR HQ : encodeur dnxhd, profil dnxhr_hq, 4:2:2 8 bits ; MXF au choix.
    let a = args(Format::DnxHr, &enc, VideoSize::Project, VideoRate::Project, None, false).unwrap();
    assert!(a.contains("-c:v dnxhd -profile:v dnxhr_hq") && a.contains("format=yuv422p ") && !a.contains("-f mxf"), "{a}");
    let a = args(Format::DnxHr, &enc, VideoSize::Project, VideoRate::Project, Some(wav), true).unwrap();
    assert!(a.contains("-f mxf") && a.contains("-c:a pcm_s16le"), "{a}");
}

#[test]
fn ffmpeg_arguments_resize_and_convert_the_rate() {
    let enc = tous_les_encodeurs();
    // Réduire : « area » ; agrandir : « lanczos » ; la cadence passe en premier, sur les images du projet.
    let a = args(Format::ProRes, &enc, VideoSize::Hd720, VideoRate::F2997, None, false).unwrap();
    assert!(a.contains("-vf fps=30000/1001,scale=1280:720:flags=area:out_color_matrix=bt709:out_range=tv,format=yuv422p10le"), "{a}");
    let a = args(Format::Mp4, &enc, VideoSize::Uhd, VideoRate::Project, None, false).unwrap();
    assert!(a.contains("scale=3840:2160:flags=lanczos") && !a.contains("fps="), "{a}");
    // La taille du projet : les images ne sont pas rééchantillonnées.
    let a = args(Format::Mp4, &enc, VideoSize::Project, VideoRate::Project, None, false).unwrap();
    assert!(a.contains("crop=1920:1080:0:0,scale=out_color_matrix=bt709:out_range=tv"), "{a}");
    // L'entrée reste à la cadence du projet : la conversion est dans le filtre.
    assert!(a.contains("-f rawvideo -pix_fmt rgba -s 1920x1080 -framerate 24 -i -"), "{a}");
    // Le débit d'un H.264 sans libx264 suit la cadence du fichier, pas celle du projet.
    let openh264 = Encoders { h264: Some("libopenh264"), ..tous_les_encodeurs() };
    let lent = args(Format::Mp4, &openh264, VideoSize::Project, VideoRate::Project, None, false).unwrap();
    let vite = args(Format::Mp4, &openh264, VideoSize::Project, VideoRate::F60, None, false).unwrap();
    assert!(lent.contains("-b:v 12442k") && vite.contains("-b:v 31104k"), "{lent}\n{vite}");
}

#[test]
fn dnxhr_refuses_sizes_below_256_by_120() {
    assert!(exportvideo::size_problem(Format::DnxHr, [128, 72]).is_some_and(|p| p.contains("256 × 120")));
    assert!(exportvideo::size_problem(Format::DnxHr, [255, 200]).is_some());
    assert_eq!(exportvideo::size_problem(Format::DnxHr, [256, 120]), None);
    assert_eq!(exportvideo::size_problem(Format::Mp4, [64, 36]), None, "les autres formats n'ont pas ce minimum");
}

#[test]
fn missing_encoders_are_reported_with_the_reason() {
    let none = Encoders::default();
    for f in [Format::Mp4, Format::ProRes, Format::ProRes4444, Format::DnxHr] {
        assert!(args(f, &none, VideoSize::Project, VideoRate::Project, None, false).is_err(), "{f:?}");
        assert!(exportvideo::missing_reason(f, &none).is_some_and(|r| r.starts_with("ce ffmpeg ne sait pas encoder")), "{f:?}");
        assert_eq!(exportvideo::missing_reason(f, &tous_les_encodeurs()), None, "{f:?}");
    }
    let sans_dnx = Encoders { dnxhd: false, ..tous_les_encodeurs() };
    assert!(exportvideo::missing_reason(Format::DnxHr, &sans_dnx).unwrap().contains("DNxHR"));
    assert_eq!(exportvideo::missing_reason(Format::ProRes, &sans_dnx), None, "ProRes n'a pas besoin du DNx");
    // Les formats des autres familles ne sont jamais grisés par ffmpeg.
    assert_eq!(exportvideo::missing_reason(Format::Wav, &none), None);
}

#[test]
fn the_windows_h264_encoder_is_preferred_and_the_reduced_quality_is_announced() {
    let list = " V....D libopenh264          OpenH264\n V....D h264_mf              H264 via MediaFoundation\n";
    let enc = export::parse_encoders(list);
    assert_eq!(enc.h264, Some("h264_mf"));
    let a = args(Format::Mp4, &enc, VideoSize::Project, VideoRate::Project, None, false).unwrap();
    assert!(a.contains("-c:v h264_mf") && a.contains("-rate_control quality -quality 100"), "{a}");
    assert!(exportvideo::h264_warning(&enc).is_some_and(|w| w.starts_with("qualité H.264 réduite : ") && w.contains("libx264") && w.contains("Windows")));
    let openh264 = Encoders { h264: Some("libopenh264"), ..enc.clone() };
    assert!(exportvideo::h264_warning(&openh264).is_some_and(|w| w.contains("libopenh264")));
    assert_eq!(exportvideo::h264_warning(&tous_les_encodeurs()), None, "libx264 : aucune réserve");
    assert_eq!(exportvideo::h264_warning(&Encoders::default()), None);
}

// ------------------------------------------------------------------------- film : informations gravées

#[test]
fn burnt_information_is_drawn_on_the_image_without_touching_the_project() {
    monde::lire();
    let mut doc = projet_uni_de(2, 24, Color32::from_rgb(200, 200, 200), 320, 180);
    doc.panels[1].action = "Il entre dans la pièce en courant très vite et sans bruit pour ne pas réveiller toute la maison".into();
    doc.panels[1].dialogue = "— Chut !".into();
    let calques = doc.panels[1].layers.len();
    let sans = BurnSettings { enabled: false, ..BurnSettings::default() };
    assert!(exportvideo::boxes(&doc, 1, 0, &sans).is_empty(), "désactivé : rien");
    let tout = BurnSettings { enabled: true, plan: true, timecode: true, sequence: true, texts: true };
    let boites = exportvideo::boxes(&doc, 1, 3, &tout);
    assert_eq!(boites.len(), 4, "plan, séquence, time code, textes");
    let seul = BurnSettings { enabled: true, plan: false, timecode: true, sequence: false, texts: false };
    assert_eq!(exportvideo::boxes(&doc, 1, 3, &seul).len(), 1);
    // L'image change là où le texte se pose, et seulement là : l'original, partagé, n'est pas touché.
    let [w, h] = doc.frame;
    let origine = vec![Color32::from_rgb(200, 200, 200); w * h];
    let gravee = exportvideo::burn_into(&doc, 1, 3, &tout, &origine);
    assert_ne!(gravee, origine);
    assert!(origine.iter().all(|c| *c == Color32::from_rgb(200, 200, 200)));
    let touches = gravee.iter().zip(&origine).filter(|(a, b)| a != b).count();
    assert!(touches > 200 && touches < w * h / 2, "{touches} pixels touchés");
    // Le haut gauche (numéro de plan) et le bas (textes) sont gravés ; le milieu de l'image ne l'est pas.
    let touche = |x: usize, y: usize| gravee[y * w + x] != origine[y * w + x];
    assert!((0..60).any(|x| (0..30).any(|y| touche(x, y))), "le numéro du plan, en haut à gauche");
    assert!((0..w).any(|x| (h - 40..h).any(|y| touche(x, y))), "les textes, en bas");
    assert!(!touche(w / 2, h / 2), "le milieu de l'image reste intact");
    // Aucun calque ajouté au projet.
    assert_eq!(doc.panels[1].layers.len(), calques);
}

#[test]
fn burnt_texts_follow_the_marks_and_wrap() {
    let mut doc = projet_uni(1, 24, Color32::WHITE);
    doc.panels[0].action = "Plan entier".into();
    doc.panels[0].dialogue = "Salut".into();
    doc.panels[0].marks.push(crate::document::ImageMark { id: 90_001, frame: 10, action: "Gros plan sur la main".into(), dialogue: "— Merci.".into(), ..Default::default() });
    let debut = exportvideo::texts_at(&doc, 0, 0, 40);
    assert_eq!(debut, "Action : Plan entier\nDialogue : Salut");
    let marque = exportvideo::texts_at(&doc, 0, 12, 40);
    assert_eq!(marque, "Action : Gros plan sur la main\nDialogue : — Merci.");
    // Un long texte passe à la ligne, quatre lignes au plus, avec des points de suspension.
    doc.panels[0].action = "mot ".repeat(80);
    let long = exportvideo::texts_at(&doc, 0, 0, 30);
    assert_eq!(long.lines().count(), 4);
    assert!(long.lines().all(|l| l.chars().count() <= 31) && long.ends_with('…'), "{long}");
    assert_eq!(exportvideo::wrap_chars("un deux trois quatre", 9), ["un deux", "trois", "quatre"]);
}

// --------------------------------------------------------------------------- film : vrais encodages

fn encoder_un_film(ffmpeg: &Path, enc: &Encoders, doc: &Document, format: Format, size: VideoSize, rate: VideoRate, mxf: bool, burn: BurnSettings, out: &Path) -> Result<String, String> {
    let frames = export::frames_for(doc, export::Range::Montage, 0);
    let frame = doc.frame;
    let fps = doc.fps;
    let (size_px, rate_out) = (size.dimensions(frame), rate.conversion(fps).map(str::to_string));
    let enc = enc.clone();
    let cible = out.to_path_buf();
    let spec = move |wav: Option<&Path>| {
        exportvideo::video_args(&VideoSpec { format, enc: &enc, frame, out_size: size_px, rate_in: fps.to_string(), rate_out: rate_out.clone(), audio: wav, target: &cible, mxf })
    };
    let mut job = ExportJob::video_with_sound_alpha(ffmpeg, spec, out, frames, None::<SoundSpan>, format.has_alpha()).avec_infos(burn);
    court(&mut job, doc);
    job.outcome.clone().expect("terminé")
}

#[test]
fn a_real_film_in_every_format_has_the_asked_size_and_rate() {
    let Some((ffmpeg, enc)) = ffmpeg_ou_saute("films de chaque format") else { return };
    monde::lire();
    let doc = projet_uni(2, 12, Color32::from_rgb(180, 90, 40));
    let dir = dossier_neuf("films");
    // (format, extension, MXF, encodeur présent, fragment attendu dans le compte rendu de ffmpeg)
    let cas = [
        (Format::Mp4, "mp4", false, enc.h264.is_some(), "h264"),
        (Format::ProRes, "mov", false, enc.prores, "prores"),
        (Format::ProRes4444, "mov", false, enc.prores, "prores (4444)"),
        (Format::DnxHr, "mov", false, enc.dnxhd, "dnxhd"),
        (Format::DnxHr, "mxf", true, enc.dnxhd, "dnxhd"),
    ];
    for (format, ext, mxf, present, attendu) in cas {
        if !present {
            eprintln!("SAUTÉ : {format:?} : ce ffmpeg n'a pas l'encodeur");
            continue;
        }
        let out = dir.join(format!("{format:?}.{ext}"));
        // 12 images par seconde converties en 24 : le double d'images ; largeur libre 320 → 320×180 (DNxHR veut 256×120 au moins).
        let r = encoder_un_film(&ffmpeg, &enc, &doc, format, VideoSize::Width(320), VideoRate::F24, mxf, BurnSettings::default(), &out);
        assert!(r.is_ok(), "{format:?} : {r:?}");
        let texte = decrire(&ffmpeg, &out);
        assert!(texte.contains(attendu), "{format:?} : « {attendu} » manque :\n{texte}");
        assert!(texte.contains("320x180"), "{format:?} : taille\n{texte}");
        assert!(texte.contains("24 fps"), "{format:?} : cadence\n{texte}");
        assert_eq!(compter_images(&ffmpeg, &out), 48, "{format:?} : 24 images à 12 par seconde = 2 s = 48 images à 24");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_real_film_keeps_its_ntsc_rate_and_the_project_size() {
    let Some((ffmpeg, enc)) = ffmpeg_ou_saute("film à 23,976") else { return };
    if !enc.prores {
        eprintln!("SAUTÉ : film à 23,976 : ce ffmpeg n'a pas ProRes");
        return;
    }
    monde::lire();
    let mut doc = projet_uni(1, 24, Color32::from_rgb(10, 120, 200));
    doc.fps = 24;
    let dir = dossier_neuf("ntsc");
    let out = dir.join("ntsc.mov");
    let r = encoder_un_film(&ffmpeg, &enc, &doc, Format::ProRes, VideoSize::Project, VideoRate::F23976, false, BurnSettings::default(), &out);
    assert!(r.is_ok(), "{r:?}");
    let texte = decrire(&ffmpeg, &out);
    assert!(texte.contains("64x36") && texte.contains("23.98 fps"), "{texte}");
    assert_eq!(doc.fps, 24, "le projet ne change pas");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn prores_4444_keeps_the_transparency_with_unpremultiplied_colours() {
    let Some((ffmpeg, enc)) = ffmpeg_ou_saute("ProRes 4444 avec transparence") else { return };
    if !enc.prores {
        eprintln!("SAUTÉ : ProRes 4444 : ce ffmpeg n'a pas ProRes");
        return;
    }
    monde::lire();
    // Un fond transparent, un dessin à moitié transparent : (100, 50, 24) droit à 50 % = (50, 25, 12) prémultiplié.
    let doc = projet_uni(1, 6, Color32::from_rgba_premultiplied(50, 25, 12, 128));
    let dir = dossier_neuf("alpha");
    let out = dir.join("alpha.mov");
    let r = encoder_un_film(&ffmpeg, &enc, &doc, Format::ProRes4444, VideoSize::Project, VideoRate::Project, false, BurnSettings::default(), &out);
    assert!(r.is_ok(), "{r:?}");
    let px = premiere_image(&ffmpeg, &out);
    assert_eq!(px.len(), 64 * 36 * 4);
    let centre = &px[(18 * 64 + 32) * 4..(18 * 64 + 32) * 4 + 4];
    let (r, g, b, a) = (centre[0] as i32, centre[1] as i32, centre[2] as i32, centre[3] as i32);
    assert!((a - 128).abs() <= 2, "alpha {a}");
    assert!((r - 100).abs() <= 4 && (g - 50).abs() <= 4 && (b - 24).abs() <= 4, "couleur droite attendue (100, 50, 24), lue ({r}, {g}, {b})");
    // Sans la dé-prémultiplication, on lirait (50, 25, 12) : trop sombre.
    assert!(r > 80);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_real_film_with_burnt_information_differs_from_the_plain_one() {
    let Some((ffmpeg, enc)) = ffmpeg_ou_saute("film avec informations gravées") else { return };
    if !enc.prores {
        eprintln!("SAUTÉ : film avec informations gravées : ce ffmpeg n'a pas ProRes");
        return;
    }
    monde::lire();
    let mut doc = projet_uni_de(1, 6, Color32::from_rgb(60, 60, 60), 320, 180);
    doc.panels[0].dialogue = "— Bonjour.".into();
    let dir = dossier_neuf("gravure");
    let (a, b) = (dir.join("sans.mov"), dir.join("avec.mov"));
    let tout = BurnSettings { enabled: true, plan: true, timecode: true, sequence: true, texts: true };
    assert!(encoder_un_film(&ffmpeg, &enc, &doc, Format::ProRes, VideoSize::Project, VideoRate::Project, false, BurnSettings::default(), &a).is_ok());
    assert!(encoder_un_film(&ffmpeg, &enc, &doc, Format::ProRes, VideoSize::Project, VideoRate::Project, false, tout, &b).is_ok());
    let (pa, pb) = (premiere_image(&ffmpeg, &a), premiere_image(&ffmpeg, &b));
    assert_eq!(pa.len(), pb.len());
    let diff = pa.iter().zip(&pb).filter(|(x, y)| (**x as i32 - **y as i32).abs() > 30).count();
    assert!(diff > 400, "les informations se voient dans le film ({diff} valeurs changées)");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------- images par plan

use crate::exportstills::{self, StillFormat, StillsPer, StillsSettings};

/// Un film de trois plans (le deuxième a deux marques d'image), numérotation verrouillée.
fn film_de_trois_plans() -> Document {
    let mut doc = projet_uni(3, 12, Color32::from_rgba_premultiplied(50, 25, 12, 128));
    doc.lock_numbering();
    doc.panels[1].marks.push(crate::document::ImageMark { id: 70_001, frame: 4, action: "Gros plan".into(), ..Default::default() });
    doc.panels[1].marks.push(crate::document::ImageMark { id: 70_002, frame: 8, dialogue: "— Oui.".into(), ..Default::default() });
    doc
}

#[test]
fn stills_are_named_like_the_montage_export() {
    let doc = film_de_trois_plans();
    let tous: Vec<usize> = (0..3).collect();
    let par_plan = exportstills::items(&doc, &tous, StillsPer::Plan);
    assert_eq!(par_plan.len(), 3);
    let noms: Vec<&str> = par_plan.iter().map(|s| s.name.as_str()).collect();
    for (k, nom) in noms.iter().enumerate() {
        assert_eq!(*nom, crate::editorial::file_safe(&crate::editorial::clip_name(&doc, k)), "même nom que le clip de l'export pour le montage");
        assert!(nom.starts_with("SQ0") && nom.contains("_PL"), "{nom}");
    }
    assert_eq!(par_plan.iter().map(|s| s.frame).collect::<Vec<_>>(), [(0, 0), (1, 0), (2, 0)]);
    // Une par case : le plan 2 a trois cases (son début et ses deux marques).
    let par_case = exportstills::items(&doc, &tous, StillsPer::Case);
    assert_eq!(par_case.len(), 5);
    assert_eq!(par_case[1].name, noms[1]);
    assert_eq!(par_case[2].name, format!("{}_C02", noms[1]));
    assert_eq!(par_case[3].name, format!("{}_C03", noms[1]));
    assert_eq!(par_case[2].frame, (1, 4));
    assert_eq!(par_case[3].frame, (1, 8));
    // Jamais deux noms pareils.
    let mut uniques: Vec<&str> = par_case.iter().map(|s| s.name.as_str()).collect();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(uniques.len(), par_case.len());
    // Un plan hors du film est ignoré ; un plan à plusieurs versions porte la sienne.
    assert_eq!(exportstills::items(&doc, &[0, 9], StillsPer::Plan).len(), 1);
    assert_eq!(exportstills::still_size(VideoSize::Project, [1517, 959]), [1517, 959], "pas de rognage à une taille paire");
    assert_eq!(exportstills::still_size(VideoSize::Width(32), [64, 36]), [32, 18]);
}

#[test]
fn stills_are_written_as_png_or_jpeg_with_or_without_the_paper() {
    monde::lire();
    let doc = film_de_trois_plans();
    let tous: Vec<usize> = (0..3).collect();
    let items = exportstills::items(&doc, &tous, StillsPer::Plan);
    let dir = dossier_neuf("stills-png");
    // PNG sans le papier : le dessin à moitié transparent est écrit « droit » (100, 50, 24, 128).
    let s = StillsSettings { transparent: true, ..StillsSettings::default() };
    let mut job = exportstills::job(&dir, &items, doc.frame, &s, false);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    for it in &items {
        let img = image::open(dir.join(format!("{}.png", it.name))).expect("PNG").to_rgba8();
        assert_eq!(img.dimensions(), (64, 36));
        let p = img.get_pixel(32, 18).0;
        assert!((p[3] as i32 - 128).abs() <= 1 && (p[0] as i32 - 100).abs() <= 2 && (p[1] as i32 - 50).abs() <= 2, "{p:?}");
    }
    assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".part")));
    // PNG avec le papier, à la moitié de la taille : opaque.
    let dir2 = dossier_neuf("stills-papier");
    let s = StillsSettings { size: VideoSize::Half, ..StillsSettings::default() };
    let mut job = exportstills::job(&dir2, &items, doc.frame, &s, false);
    court(&mut job, &doc);
    let img = image::open(dir2.join(format!("{}.png", items[0].name))).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (32, 18));
    assert_eq!(img.get_pixel(10, 10).0[3], 255, "avec le papier : opaque");
    // JPEG à une largeur libre : posé sur du blanc.
    let dir3 = dossier_neuf("stills-jpeg");
    let s = StillsSettings { format: StillFormat::Jpeg, size: VideoSize::Width(128), quality: 95, ..StillsSettings::default() };
    let mut job = exportstills::job(&dir3, &items, doc.frame, &s, false);
    court(&mut job, &doc);
    let octets = std::fs::read(dir3.join(format!("{}.jpg", items[0].name))).expect("JPEG");
    assert_eq!(&octets[..2], &[0xFF, 0xD8]);
    let img = image::load_from_memory(&octets).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (128, 72));
    let c = img.get_pixel(60, 30).0;
    assert!(c[0] > 150 && c[1] > 130, "sur fond blanc, pas sur du noir : {c:?}");
    for d in [&dir, &dir2, &dir3] {
        std::fs::remove_dir_all(d).ok();
    }
}

#[test]
fn cancelling_the_stills_removes_what_was_written() {
    monde::lire();
    let doc = projet_uni(40, 3, Color32::from_rgb(10, 200, 30));
    let tous: Vec<usize> = (0..40).collect();
    let items = exportstills::items(&doc, &tous, StillsPer::Plan);
    let dir = dossier_neuf("stills-annule");
    let mut job = exportstills::job(&dir, &items, doc.frame, &StillsSettings::default(), false);
    let t = Instant::now();
    while job.outcome.is_none() && !matches!(job.progress(), export::Progress::Running(n, _) if n >= 3) {
        job.step(&doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(1));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    job.cancel();
    assert!(matches!(job.outcome, Some(Err(_))));
    let restes: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(restes.is_empty(), "rien ne reste après l'annulation : {restes:?}");
    std::fs::remove_dir_all(&dir).ok();
}

// ----------------------------------------------------------------------------- storyboard PDF à la carte

use crate::pdf::{Board, BoardImage};
use crate::pdffont::Fonts;
use crate::pdfmise::{self, Paper, PdfLayout};

fn planche(seq: &str, titre: &str, action: &str) -> Board {
    Board { title: titre.into(), duration: "2 s".into(), sequence: seq.into(), action: action.into(), dialogue: String::new(), notes: String::new(), comments: Vec::new() }
}

fn une_image() -> BoardImage {
    let mut jpeg = Vec::new();
    let rgb = vec![200u8; 16 * 9 * 3];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80).encode(&rgb, 16, 9, image::ExtendedColorType::Rgb8).unwrap();
    BoardImage { width: 16, height: 9, jpeg }
}

#[test]
fn pdf_grids_are_computed_for_each_page_layout() {
    let table = [(1, true, (1, 1)), (2, true, (2, 1)), (2, false, (1, 2)), (3, true, (3, 1)), (3, false, (1, 3)), (4, true, (2, 2)), (6, true, (3, 2)), (6, false, (2, 3)), (8, true, (4, 2)), (8, false, (2, 4)), (9, true, (3, 3)), (9, false, (3, 3))];
    for (n, paysage, attendu) in table {
        assert_eq!(pdfmise::grid(n, paysage), attendu, "{n} cases, paysage {paysage}");
    }
    for n in pdfmise::PER_PAGE_CHOICES {
        for paysage in [true, false] {
            let (c, r) = pdfmise::grid(n, paysage);
            assert!(c * r >= n, "{n} cases tiennent dans {c} × {r}");
        }
    }
    let l = PdfLayout { paper: Paper::A3, landscape: false, per_page: 5, ..PdfLayout::default() };
    assert_eq!(l.per_page_ok(), 4, "un nombre hors de la liste devient le plus proche permis");
    assert_eq!(l.page_size(), (842.0, 1191.0));
    assert_eq!(PdfLayout { landscape: true, ..l }.page_size(), (1191.0, 842.0));
    assert_eq!(PdfLayout { paper: Paper::Letter, landscape: true, ..l }.page_size(), (792.0, 612.0));
    assert!(PdfLayout::default().label().contains("A4 paysage") && PdfLayout::default().image_width() >= 600);
}

fn compter_pages(pdf: &[u8]) -> usize {
    let t = String::from_utf8_lossy(pdf);
    t.matches("/Type /Page /Parent").count()
}

#[test]
fn pdf_pages_follow_the_layout() {
    let boards: Vec<Board> = (0..10).map(|i| planche("1", &format!("Plan {i}"), "Il marche.")).collect();
    let images: Vec<BoardImage> = (0..10).map(|_| une_image()).collect();
    for (per, attendu) in [(1, 10), (2, 5), (3, 4), (4, 3), (6, 2), (8, 2), (9, 2)] {
        let l = PdfLayout { per_page: per, ..PdfLayout::default() };
        let bytes = pdfmise::storyboard_layout_with("Essai", "2026-10-30", &boards, &images, 16.0 / 9.0, &l, Fonts::Std);
        assert!(bytes.starts_with(b"%PDF"));
        assert_eq!(compter_pages(&bytes), attendu, "{per} cases par page");
    }
    // Les dimensions de la page : A3 paysage, Lettre portrait.
    let a3 = pdfmise::storyboard_layout_with("Essai", "d", &boards[..1], &images[..1], 16.0 / 9.0, &PdfLayout { paper: Paper::A3, ..PdfLayout::default() }, Fonts::Std);
    assert!(String::from_utf8_lossy(&a3).contains("/MediaBox [0 0 1191.00 842.00]"));
    let lettre = pdfmise::storyboard_layout_with("Essai", "d", &boards[..1], &images[..1], 16.0 / 9.0, &PdfLayout { paper: Paper::Letter, landscape: false, ..PdfLayout::default() }, Fonts::Std);
    assert!(String::from_utf8_lossy(&lettre).contains("/MediaBox [0 0 612.00 792.00]"));
    // Une nouvelle séquence ouvre une nouvelle page.
    let deux: Vec<Board> = vec![planche("1", "A", ""), planche("2", "B", "")];
    let bytes = pdfmise::storyboard_layout_with("Essai", "d", &deux, &images[..2], 16.0 / 9.0, &PdfLayout::default(), Fonts::Std);
    assert_eq!(compter_pages(&bytes), 2);
}

/// Les textes écrits par `Tj` dans les pages d'un PDF à police embarquée, relus par la table `ToUnicode` de chaque police.
fn textes_du_pdf(pdf: &[u8]) -> Vec<String> {
    let t = String::from_utf8_lossy(pdf).into_owned();
    // Les tables ToUnicode, dans l'ordre : normale (F3), grasse (F4).
    let mut tables: Vec<std::collections::HashMap<String, String>> = Vec::new();
    let mut rest = t.as_str();
    while let Some(i) = rest.find("begincmap") {
        let fin = rest[i..].find("endcmap").expect("fin de table") + i;
        let mut m = std::collections::HashMap::new();
        for l in rest[i..fin].lines() {
            let l = l.trim();
            if l.starts_with('<') && l.matches('<').count() == 2 {
                let mut it = l.split(['<', '>']).filter(|s| !s.trim().is_empty());
                let (g, u) = (it.next().unwrap().to_string(), it.next().unwrap().to_string());
                m.insert(g, u);
            }
        }
        tables.push(m);
        rest = &rest[fin..];
    }
    let mut out = Vec::new();
    for bt in t.split("BT /").skip(1) {
        let font = &bt[..2];
        let table = match font {
            "F3" => &tables[0],
            "F4" => &tables[1],
            _ => continue,
        };
        let a = bt.find('<').unwrap();
        let b = bt[a..].find('>').unwrap() + a;
        let hex = &bt[a + 1..b];
        let mut s = String::new();
        for g in hex.as_bytes().chunks(4) {
            let g = std::str::from_utf8(g).unwrap();
            let u = table.get(g).unwrap_or_else(|| panic!("glyphe {g} sans lettre dans ToUnicode"));
            let units: Vec<u16> = u.as_bytes().chunks(4).map(|c| u16::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect();
            s.push_str(&String::from_utf16(&units).unwrap());
        }
        out.push(s);
    }
    out
}

#[test]
fn the_pdf_embeds_a_tolerated_truetype_font_and_writes_accents_without_question_marks() {
    let texte = "Éléphant à Çà, cœur à 5 € « œuvre » — naïve façade";
    let boards = vec![
        Board { title: "Séquence 1 — plan 12A".into(), duration: "2 s 12".into(), sequence: "Été".into(), action: texte.into(), dialogue: "— Où ça ? Ça ira…".into(), notes: "Œil, ÿ, ñ, ü, ß".into(), comments: vec![("Zoë".into(), "Très bien".into())] },
    ];
    let fonts = Fonts::choose(&pdfmise::tout_le_texte("Essai éàçœ€", "2026-10-30", &boards));
    if !fonts.is_embedded() {
        eprintln!("SAUTÉ : police embarquée : aucune police TrueType permise dans C:\\Windows\\Fonts sur cette machine");
        return;
    }
    let nom = fonts.name();
    let bytes = pdfmise::storyboard_layout_with("Essai éàçœ€", "2026-10-30", &boards, &[une_image()], 16.0 / 9.0, &PdfLayout::default(), fonts);
    let t = String::from_utf8_lossy(&bytes);
    for needle in ["/CIDFontType2", "/Identity-H", "/FontFile2", "/ToUnicode", "/FontDescriptor", "/CIDToGIDMap /Identity", "/W [ "] {
        assert!(t.contains(needle), "« {needle} » manque dans le PDF");
    }
    assert!(t.contains(&nom), "le nom de la police ({nom}) est dans le PDF");
    // Le texte relu est exactement celui d'origine : aucune lettre remplacée par « ? ».
    let lus = textes_du_pdf(&bytes);
    let tout = lus.join("\n");
    assert!(lus.iter().any(|l| l == texte), "l'action, mot pour mot : {lus:?}");
    for attendu in ["Séquence 1 — plan 12A", "— Où ça ? Ça ira…", "Œil, ÿ, ñ, ü, ß", "Très bien", "Commentaire (Zoë) : ", "Essai éàçœ€", "Séquence Été · 2026-10-30", "Page 1 / 1", "2 s 12"] {
        assert!(lus.iter().any(|l| l == attendu || l.ends_with(attendu)), "« {attendu} » manque :\n{tout}");
    }
    // Les seuls « ? » sont ceux du texte lui-même (« Où ça ? »).
    assert_eq!(tout.matches('?').count(), 1, "{tout}");
    // La police embarquée est lisible et sa licence d'incorporation est permise.
    let i = t.find("/FontFile2").unwrap();
    let _ = i;
    let flux = flux_police(&bytes);
    let face = ttf_parser::Face::parse(&flux, 0).expect("la police embarquée se relit");
    assert!(crate::pdffont::embeddable(&face), "licence d'incorporation permise");
    assert!(face.glyph_index('é').is_some() && face.glyph_index('œ').is_some() && face.glyph_index('€').is_some());
}

/// Les octets de la première police du PDF (le flux `FontFile2`, décompressé).
fn flux_police(pdf: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let marque = b"/FontFile2 ";
    let at = pdf.windows(marque.len()).position(|w| w == marque).expect("FontFile2");
    let n: String = String::from_utf8_lossy(&pdf[at + marque.len()..at + marque.len() + 8]).chars().take_while(char::is_ascii_digit).collect();
    let debut = format!("\n{n} 0 obj\n");
    let o = pdf.windows(debut.len()).position(|w| w == debut.as_bytes()).expect("objet de la police");
    let s = pdf[o..].windows(7).position(|w| w == b"stream\n").unwrap() + o + 7;
    let e = pdf[s..].windows(10).position(|w| w == b"\nendstream").unwrap() + s;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&pdf[s..e]).read_to_end(&mut out).expect("police compressée");
    out
}

#[test]
fn the_embedding_licence_is_read_from_the_os2_table() {
    let Some(dir) = std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts")) else {
        eprintln!("SAUTÉ : licence d'incorporation : pas de dossier de polices Windows");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("arial.ttf")).or_else(|_| std::fs::read(dir.join("segoeui.ttf"))) else {
        eprintln!("SAUTÉ : licence d'incorporation : ni Arial ni Segoe UI dans {}", dir.display());
        return;
    };
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let origine = crate::pdffont::embeddable(&face);
    // On change `fsType` (deux octets, après `xAvgCharWidth`, au début de la table OS/2) : 2 = licence restreinte, 4 = aperçu et impression,
    // 8 = modifiable, 0x0200 = bitmaps seulement.
    let raw = ttf_parser::RawFace::parse(&data, 0).unwrap();
    let os2 = raw.table_records.into_iter().find(|r| r.tag == ttf_parser::Tag::from_bytes(b"OS/2")).expect("table OS/2").offset as usize;
    let avec = |fs: u16| {
        let mut d = data.clone();
        d[os2 + 8..os2 + 10].copy_from_slice(&fs.to_be_bytes());
        let f = ttf_parser::Face::parse(&d, 0).unwrap();
        crate::pdffont::embeddable(&f)
    };
    assert!(!avec(0x0002), "restreinte : jamais");
    assert!(!avec(0x0200), "bitmaps seulement : jamais");
    assert!(avec(0x0000) && avec(0x0004) && avec(0x0008), "installable, aperçu et impression, modifiable : permis");
    eprintln!("licence de la police des essais : {}", if origine { "permise" } else { "REFUSÉE (le PDF retombe sur Helvetica)" });
}

#[test]
fn pdf_texts_can_be_hidden_and_the_header_is_optional() {
    let b = Board { title: "Plan 1".into(), duration: "2 s".into(), sequence: "1".into(), action: "Il entre dans la pièce".into(), dialogue: "Salut".into(), notes: "Notes importantes".into(), comments: Vec::new() };
    if !Fonts::choose(&pdfmise::tout_le_texte("Mon film", "2026-10-30", std::slice::from_ref(&b))).is_embedded() {
        eprintln!("SAUTÉ : textes du PDF : pas de police embarquée sur cette machine");
        return;
    }
    let tout = |l: &PdfLayout| {
        let f = Fonts::choose(&pdfmise::tout_le_texte("Mon film", "2026-10-30", std::slice::from_ref(&b)));
        textes_du_pdf(&pdfmise::storyboard_layout_with("Mon film", "2026-10-30", std::slice::from_ref(&b), &[une_image()], 16.0 / 9.0, l, f)).join("\n")
    };
    let plein = tout(&PdfLayout::default());
    for n in ["Plan 1", "2 s", "Il entre dans la pièce", "Salut", "Notes importantes", "Mon film", "2026-10-30", "Page 1 / 1"] {
        assert!(plein.contains(n), "« {n} » manque :\n{plein}");
    }
    let rien = tout(&PdfLayout { action: false, dialogue: false, notes: false, duration: false, number: false, header: false, page_numbers: false, ..PdfLayout::default() });
    for n in ["Plan 1", "2 s", "Il entre", "Salut", "Notes", "Mon film", "2026-10-30", "Page 1"] {
        assert!(!rien.contains(n), "« {n} » ne devrait pas être écrit :\n{rien}");
    }
    let sans_action = tout(&PdfLayout { action: false, ..PdfLayout::default() });
    assert!(!sans_action.contains("Il entre") && sans_action.contains("Salut"), "{sans_action}");
}

#[test]
fn without_an_embeddable_font_the_pdf_falls_back_to_the_standard_fonts() {
    let b = planche("1", "Plan é", "Un 日 est remplacé");
    let bytes = pdfmise::storyboard_layout_with("Titre", "d", &[b], &[une_image()], 16.0 / 9.0, &PdfLayout::default(), Fonts::Std);
    let t = String::from_utf8_lossy(&bytes);
    assert!(t.contains("/F1 3 0 R") && !t.contains("/FontFile2") && !t.contains("/F3"), "Helvetica seulement");
    assert!(t.contains("(Plan \\351)"), "WinAnsi d'avant : é = 351 en octal");
    let fonts = Fonts::choose("é日");
    if fonts.is_embedded() {
        assert_eq!(fonts.missing("é日"), vec!['日'], "ce que la police ne sait pas écrire est annoncé");
    }
    assert_eq!(Fonts::Std.missing("é日€"), vec!['日'], "WinAnsi : é et € passent, pas 日");
}
// --------------------------------------------------------------------------------------- son par piste

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::audio::{MixClip, Pcm};
use crate::editorial_tests::film_with_sound;
use crate::exportstems::{self, AudioFormat, Container, Depth, StemsSettings};

/// Les échantillons d'un fichier WAV 16 ou 24 bits écrit par SHOT (entiers, canaux entrelacés), avec son en-tête relu.
fn lire_wav(bytes: &[u8]) -> (u16, Vec<i32>) {
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 1, "PCM");
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2, "stéréo");
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 48_000);
    let bits = u16::from_le_bytes([bytes[34], bytes[35]]);
    assert_eq!(&bytes[36..40], b"data");
    let data = &bytes[44..];
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize, data.len(), "taille des données");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize, 36 + data.len(), "taille du RIFF");
    let n = bits as usize / 8;
    let v = data.chunks(n).map(|c| if n == 2 { i16::from_le_bytes([c[0], c[1]]) as i32 } else { i32::from_le_bytes([c[0], c[1], c[2], if c[2] & 0x80 != 0 { 0xFF } else { 0 }]) }).collect();
    (bits, v)
}

/// Les échantillons d'un AIFF écrit par SHOT, avec la fréquence relue dans son nombre à virgule flottante étendu.
fn lire_aiff(bytes: &[u8]) -> (u16, f64, Vec<i32>) {
    assert_eq!(&bytes[..4], b"FORM");
    assert_eq!(&bytes[8..16], b"AIFFCOMM");
    assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 18);
    assert_eq!(u16::from_be_bytes([bytes[20], bytes[21]]), 2, "stéréo");
    let frames = u32::from_be_bytes(bytes[22..26].try_into().unwrap()) as usize;
    let bits = u16::from_be_bytes([bytes[26], bytes[27]]);
    let exp = (((bytes[28] as i32 & 0x7F) << 8) | bytes[29] as i32) - 16383;
    let mant = u64::from_be_bytes(bytes[30..38].try_into().unwrap());
    let rate = mant as f64 / (1u64 << 63) as f64 * 2f64.powi(exp);
    assert_eq!(&bytes[38..42], b"SSND");
    let ssnd = u32::from_be_bytes(bytes[42..46].try_into().unwrap()) as usize;
    assert_eq!(&bytes[46..54], &[0u8; 8], "décalage et taille de bloc nuls");
    let data = &bytes[54..];
    assert_eq!(ssnd, 8 + data.len(), "taille du bloc SSND");
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize, bytes.len() - 8, "taille du FORM");
    let n = bits as usize / 8;
    assert_eq!(data.len(), frames * 2 * n);
    let v = data.chunks(n).map(|c| if n == 2 { i16::from_be_bytes([c[0], c[1]]) as i32 } else { i32::from_be_bytes([if c[0] & 0x80 != 0 { 0xFF } else { 0 }, c[0], c[1], c[2]]) }).collect();
    (bits, rate, v)
}

/// Un extrait de 1000 échantillons stéréo : gauche 8000, droite −4000 (sur 32768), joué tel quel.
fn clip_gauche_droite() -> MixClip {
    let data: Vec<i16> = (0..1000).flat_map(|_| [8000i16, -4000]).collect();
    MixClip::plain(Arc::new(Pcm { data, peaks: Vec::new() }), 0, 0, 1000, 1.0)
}

#[test]
fn audio_files_are_written_as_wav_16_wav_24_and_aiff() {
    let dir = dossier_neuf("audio-formats");
    // Le WAV 16 bits est octet pour octet l'ancien.
    let ancien = dir.join("ancien.wav");
    export::write_wav(&ancien, vec![clip_gauche_droite()], 0, 1000).unwrap();
    let neuf = dir.join("neuf.wav");
    assert!(exportstems::write_audio_until(&neuf, vec![clip_gauche_droite()], 0, 1000, AudioFormat::default(), None).unwrap());
    assert_eq!(std::fs::read(&ancien).unwrap(), std::fs::read(&neuf).unwrap());
    let l = 8000.0 / 32768.0;
    let r = -4000.0 / 32768.0;
    // WAV 24 bits : 3 octets par échantillon, la même valeur à 24 bits.
    let w24 = dir.join("a24.wav");
    exportstems::write_audio_until(&w24, vec![clip_gauche_droite()], 0, 1000, AudioFormat { container: Container::Wav, depth: Depth::Bits24 }, None).unwrap();
    let octets = std::fs::read(&w24).unwrap();
    assert_eq!(octets.len(), 44 + 1000 * 2 * 3);
    let (bits, v) = lire_wav(&octets);
    assert_eq!(bits, 24);
    assert_eq!(v.len(), 2000);
    assert!((v[0] as f64 - l * 8_388_607.0).abs() < 1.0 && (v[1] as f64 - r * 8_388_607.0).abs() < 1.0, "{} {}", v[0], v[1]);
    assert!(v.iter().step_by(2).all(|x| *x == v[0]) && v.iter().skip(1).step_by(2).all(|x| *x == v[1]));
    // AIFF 16 et 24 bits : en-têtes, fréquence en virgule flottante étendue, échantillons gros-boutistes.
    for (depth, bits_attendus) in [(Depth::Bits16, 16u16), (Depth::Bits24, 24)] {
        let p = dir.join(format!("a{bits_attendus}.aiff"));
        exportstems::write_audio_until(&p, vec![clip_gauche_droite()], 0, 1000, AudioFormat { container: Container::Aiff, depth }, None).unwrap();
        let (bits, rate, v) = lire_aiff(&std::fs::read(&p).unwrap());
        assert_eq!((bits, rate), (bits_attendus, 48_000.0));
        let max = if bits == 16 { 32767.0 } else { 8_388_607.0 };
        assert!((v[0] as f64 - l * max).abs() < 1.0 && (v[1] as f64 - r * max).abs() < 1.0, "{bits} bits : {} {}", v[0], v[1]);
        assert_eq!(v.len(), 2000);
    }
    assert!(!dir.join("neuf.wav.part").exists());
    // Annuler : aucun fichier, aucun reste.
    let flag = AtomicBool::new(true);
    let p = dir.join("annule.wav");
    assert!(!exportstems::write_audio_until(&p, vec![clip_gauche_droite()], 0, 1000, AudioFormat::default(), Some(&flag)).unwrap());
    assert!(!p.exists() && !export::part_of(&p).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stem_file_names_are_the_track_names() {
    assert_eq!(exportstems::file_name_of("Voix é", "x"), "Voix é", "les accents sont gardés");
    assert_eq!(exportstems::file_name_of("Bruits : fond / ambiance ?", "x"), "Bruits _ fond _ ambiance _");
    assert_eq!(exportstems::file_name_of("  Musique...  ", "x"), "Musique");
    assert_eq!(exportstems::file_name_of("", "Piste 3"), "Piste 3");
    assert_eq!(exportstems::file_name_of("CON", "Piste 1"), "Piste 1", "nom réservé de Windows");
    assert_eq!(exportstems::file_name_of("com3", "Piste 1"), "Piste 1");
}

#[test]
fn stems_follow_mute_and_solo_and_skip_empty_tracks() {
    let mut doc = film_with_sound(24);
    let s = StemsSettings::default();
    let n = doc.total_frames();
    let p = exportstems::plan(&doc, 0, n, &s, "Mon film");
    assert_eq!(p.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["Voix é.wav", "Musique.wav"]);
    assert_eq!(p[0].track, Some(0));
    // Avec le mixage : en dernier, nommé d'après le projet ; AIFF si on le veut.
    let avec = StemsSettings { with_mix: true, format: AudioFormat { container: Container::Aiff, depth: Depth::Bits24 } };
    let p = exportstems::plan(&doc, 0, n, &avec, "Mon film");
    assert_eq!(p.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["Voix é.aiff", "Musique.aiff", "Mon film_mix.aiff"]);
    assert_eq!(p[2].track, None);
    // Muette : pas écrite ; solo sur l'autre : seule celle-là.
    doc.audio_tracks[0].muted = true;
    assert_eq!(exportstems::plan(&doc, 0, n, &s, "x").iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["Musique.wav"]);
    doc.audio_tracks[0].muted = false;
    doc.audio_tracks[1].solo = true;
    assert_eq!(exportstems::plan(&doc, 0, n, &s, "x").iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["Musique.wav"], "une piste en solo couvre les autres");
    doc.audio_tracks[1].solo = false;
    // Une étendue sans extrait de la voix (après son dernier extrait) : la piste n'est pas écrite.
    let apres = doc.audio_tracks[0].clips.iter().map(|c| c.end()).max().unwrap();
    let p = exportstems::plan(&doc, apres, 4, &s, "x");
    assert_eq!(p.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["Musique.wav"]);
    // Deux pistes de même nom : jamais deux fichiers pareils.
    doc.audio_tracks[1].name = "VOIX É".into();
    let p = exportstems::plan(&doc, 0, n, &s, "x");
    assert_eq!(p.len(), 2);
    assert_ne!(p[0].file.to_lowercase(), p[1].file.to_lowercase());
    // Aucun son : rien.
    assert!(exportstems::plan(&projet_uni(1, 4, Color32::WHITE), 0, 4, &avec, "x").is_empty());
}

#[test]
fn stems_add_up_to_the_mix_and_cancel_cleanly() {
    monde::lire();
    let doc = film_with_sound(24);
    let n = doc.total_frames();
    let (start, len) = export::sample_span(doc.fps, 0, n);
    let dir = dossier_neuf("stems");
    let s = StemsSettings { with_mix: true, ..StemsSettings::default() };
    let files = exportstems::plan(&doc, 0, n, &s, "film");
    assert_eq!(files.len(), 3);
    let (d, f2, fmt) = (dir.clone(), files.clone(), s.format);
    let mut job = ExportJob::task("Son par piste".into(), &dir, "ok".into(), move |snap, cancel| exportstems::write_all(snap, &d, &f2, start, len, fmt, cancel).map(|_| ()));
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let lus: Vec<Vec<i32>> = files.iter().map(|f| lire_wav(&std::fs::read(dir.join(&f.file)).unwrap()).1).collect();
    for v in &lus {
        assert_eq!(v.len() as u64, len * 2, "même durée pour tous, du début de l'étendue");
    }
    assert!(lus[0].iter().any(|x| *x != 0) && lus[1].iter().any(|x| *x != 0), "chaque piste a du son");
    // Le volume de la piste est dedans : la musique est à 4000 × 0,5.
    assert_eq!(*lus[1].iter().max().unwrap(), (4000.0f32 / 32768.0 * 0.5 * 32767.0).round() as i32);
    // Les pistes, additionnées, font le mixage (à l'arrondi près).
    let pire = (0..lus[2].len()).map(|i| (lus[0][i] + lus[1][i] - lus[2][i]).abs()).max().unwrap();
    assert!(pire <= 2, "écart de {pire}");
    assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".part")));
    std::fs::remove_dir_all(&dir).ok();
    // Annulé en route : rien ne reste.
    let dir = dossier_neuf("stems-annule");
    let flag = AtomicBool::new(true);
    assert!(exportstems::write_all(&doc, &dir, &files, start, len, fmt, &flag).is_err());
    assert!(std::fs::read_dir(&dir).unwrap().flatten().next().is_none(), "annulé : aucun fichier");
    std::fs::remove_dir_all(&dir).ok();
}