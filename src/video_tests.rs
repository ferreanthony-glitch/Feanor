//! Calque vidéo (13k) : cadence, séquences, images lues à la demande, enregistrement.

use std::path::PathBuf;

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::video::{self, VideoClip};

fn clip(frames: usize, fps: f64, start: usize) -> VideoClip {
    VideoClip { id: video::new_id(), name: "essai.mp4".into(), frames, fps, start, ext: "png".into() }
}

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-video-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn video_frames_follow_the_two_frame_rates() {
    // Même cadence : une image de la vidéo par image du plan, à partir du début du calque.
    let c = clip(10, 24.0, 5);
    assert_eq!((c.index_at(4, 24), c.index_at(5, 24), c.index_at(14, 24), c.index_at(15, 24)), (None, Some(0), Some(9), None));
    assert_eq!((c.plan_frames(24), c.end(24)), (10, 15));
    // Vidéo à 12 i/s dans un projet à 24 : chaque image tenue deux fois.
    let c = clip(6, 12.0, 0);
    assert_eq!((0..4).map(|f| c.index_at(f, 24)).collect::<Vec<_>>(), vec![Some(0), Some(0), Some(1), Some(1)]);
    assert_eq!(c.plan_frames(24), 12);
    // Vidéo à 30 i/s dans un projet à 24 : des images sautées.
    let c = clip(30, 30.0, 0);
    assert_eq!(c.plan_frames(24), 24);
    assert_eq!(c.index_at(23, 24), Some(28));
}

#[test]
fn ffmpeg_frame_rate_and_numbered_sequences() {
    let log = "Input #0, mov,mp4\n  Stream #0:0[0x1](und): Video: h264 (High), yuv420p, 1920x1080, 4000 kb/s, 23.98 fps, 23.98 tbr, 24k tbn\n  Stream #0:1: Audio: aac";
    assert_eq!(video::parse_fps(log), Some(23.98));
    assert_eq!(video::parse_fps("rien"), None);
    assert_eq!(video::split_numbered("plan_0012.PNG"), Some(("plan_".into(), 12, "png".into())));
    assert_eq!(video::split_numbered("sans_numero.png"), None);
    let dir = temp("sequence");
    for name in ["a_0003.png", "a_0001.png", "a_0010.png", "b_0002.png", "a_0002.jpg", "notes.txt"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    let files: Vec<String> = video::sequence_files(&dir.join("a_0001.png")).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(files, vec!["a_0001.png", "a_0003.png", "a_0010.png"], "même préfixe, même extension, dans l'ordre des numéros");
}

/// Un projet 32×32 (cadre 16×16 au centre), une vidéo de 2 images rouges puis bleues (4×2
/// pixels) dans `dir`.
fn video_project(dir: &std::path::Path) -> (Document, VideoClip) {
    let mut doc = Document::with_frame(16, 16, 24);
    let c = VideoClip { start: 1, frames: 2, ..clip(2, 24.0, 1) };
    for (k, color) in [(0, [255, 0, 0, 255]), (1, [0, 0, 255, 255])] {
        image::RgbaImage::from_pixel(4, 2, image::Rgba(color)).save(video::frame_file(dir, k, "png")).unwrap();
    }
    video::register_dir(c.id, dir.to_path_buf());
    let mut layer = Layer::new("Vidéo", LayerKind::Bitmap);
    layer.video = Some(c.clone());
    doc.panels[0].layers = vec![layer];
    (doc, c)
}

#[test]
fn video_frames_are_read_on_demand_in_the_middle_of_the_frame() {
    let dir = temp("lecture");
    let (doc, c) = video_project(&dir);
    // Le cadre 16×16 est au centre de la feuille 32×32 (8..24) ; l'image 4×2 au centre du cadre.
    let r = video::read_frame(&doc, &video::frame_file(&dir, 0, "png")).unwrap();
    assert_eq!(r.get(14, 15), Color32::RED);
    assert_eq!(r.get(17, 16), Color32::RED);
    assert_eq!(r.get(13, 15), Color32::TRANSPARENT);
    // Dans le plan : rien à l'image 0, la 1re image à l'image 1, la 2e à l'image 2.
    assert!(crate::render::panel_plan_at(&doc, 0, 0, None).items.is_empty());
    let plan = crate::render::panel_plan_at(&doc, 0, 2, None);
    assert_eq!(plan.items[0].cel, video::frame_id(&c, 1));
    assert_eq!(crate::render::composite_pixel(&doc, &plan, 15, 15), Color32::BLUE);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn video_frames_go_into_the_project_folder() {
    let src = temp("source");
    let (doc, c) = video_project(&src);
    let dir = temp("projet");
    let manifest = dir.join("V.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let copied = dir.join("videos").join(c.id.to_string());
    assert!(video::frame_file(&copied, 1, "png").is_file(), "images recopiées dans le projet");
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].video, Some(c.clone()));
    assert_eq!(video::dir_of(c.id), Some(copied), "relues depuis le projet");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&src).ok();
}

/// Import réel par ffmpeg (s'il est là) : une vidéo d'une seconde avec du son.
#[test]
#[ignore]
fn ffmpeg_import_makes_frames_and_sound() {
    let Some(ffmpeg) = crate::export::find_ffmpeg(None) else { panic!("ffmpeg introuvable") };
    let dir = temp("ffmpeg");
    // La vidéo d'essai : 24 images 64×36 en PNG et une seconde de la 440 Hz en WAV, assemblées
    // par ffmpeg (sans son générateur « lavfi », absent de certains ffmpeg).
    for k in 0..24u8 {
        image::RgbImage::from_pixel(64, 36, image::Rgb([k * 10, 100, 200])).save(dir.join(format!("{:03}.png", k + 1))).unwrap();
    }
    let rate = 48_000u32;
    let samples: Vec<i16> = (0..rate).map(|i| ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 8000.0) as i16).collect();
    let mut wav = Vec::new();
    let data_len = (samples.len() * 2) as u32;
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(dir.join("son.wav"), wav).unwrap();
    let mp4 = dir.join("essai.mp4");
    let out = std::process::Command::new(&ffmpeg)
        .args(["-hide_banner", "-y", "-framerate", "24", "-i"])
        .arg(dir.join("%03d.png"))
        .arg("-i")
        .arg(dir.join("son.wav"))
        .args(["-c:v", "mpeg4", "-c:a", "aac", "-pix_fmt", "yuv420p", "-shortest"])
        .arg(&mp4)
        .output()
        .unwrap();
    assert!(out.status.success(), "vidéo d'essai : {}", String::from_utf8_lossy(&out.stderr));
    let mut job = video::start_import(Some(ffmpeg), video::Source::Video(mp4), [32, 18], 24);
    let imported = loop {
        if let Some(r) = job.finished() {
            break r.unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert_eq!(imported.clip.frames, 24);
    assert_eq!(imported.clip.fps, 24.0);
    assert!(imported.sound.is_some(), "le son de la vidéo");
    let first = video::frame_file(&video::temp_dir(imported.clip.id), 0, "jpg");
    let img = image::open(&first).unwrap();
    assert_eq!((img.width(), img.height()), (32, 18), "à la taille du cadre");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(video::temp_dir(imported.clip.id)).ok();
}
