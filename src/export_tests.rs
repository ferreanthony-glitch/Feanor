//! Tests des exports (phase 3b).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::Color32;

use crate::audio::{MixClip, Pcm};
use crate::camera::{Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::export::{self, Encoders, ExportJob, Format, Range};
use crate::render::{self, Plan};

/// Ancien calcul de l'image filmée (toute la feuille composée, sur un seul fil) : la référence.
fn camera_image_reference(doc: &Document, plan: &Plan, cam: &Camera) -> Vec<Color32> {
    let full = render::composite_full(doc, plan);
    let [fw, fh] = doc.frame;
    let (w, h) = (doc.width as i64, doc.height as i64);
    let get = |x: i64, y: i64| -> [f32; 4] {
        if x < 0 || y < 0 || x >= w || y >= h {
            return [0.0; 4];
        }
        let c = full[(y * w + x) as usize];
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let mut out = Vec::with_capacity(fw * fh);
    for v in 0..fh {
        for u in 0..fw {
            let p = cam.to_stage(doc.frame, u as f32 + 0.5, v as f32 + 0.5);
            let (fx, fy) = (p.x - 0.5, p.y - 0.5);
            let (ix, iy) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - ix, fy - iy);
            let (ix, iy) = (ix as i64, iy as i64);
            let (a, b, c, d) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
            let mut px = [0u8; 4];
            for k in 0..4 {
                let top = a[k] + (b[k] - a[k]) * tx;
                let bottom = c[k] + (d[k] - c[k]) * tx;
                px[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
            }
            out.push(Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3]));
        }
    }
    out
}

/// Cadre 150×90 (feuille 300×180, pas un multiple de 64) ; deux calques avec un motif, dont un
/// transformé ; `panels` panneaux de `len` images.
pub(crate) fn doc_with_drawing(panels: usize, len: usize) -> Document {
    let mut doc = Document::with_frame(150, 90, 12);
    for _ in 1..panels {
        let n = doc.panels.len() - 1;
        doc.add_panel_after(n);
    }
    for p in 0..panels {
        let mut layers = Vec::new();
        for li in 0..2 {
            let id = doc.new_cel(LayerKind::Bitmap);
            let r = &mut doc.cels.get_mut(&id).unwrap().raster;
            for y in (0..180).step_by(3) {
                for x in 0..300 {
                    if (x * (li + 2) + y + p * 7) % 11 < 4 {
                        let idx = r.tile_index_of(x, y);
                        r.tile_mut(idx)[(y % 64) * 64 + x % 64] =
                            Color32::from_rgba_premultiplied((x % 256) as u8, (y * 2 % 256) as u8, 90, 255);
                    }
                }
            }
            let mut l = Layer::new(format!("C{li}"), LayerKind::Bitmap);
            l.frames = vec![Some(id); len];
            if li == 1 {
                l.transform = LayerTransform { x: 7.5, y: -4.0, scale: 0.8, angle: 12.0 };
                l.opacity = 0.6;
            }
            layers.push(l);
        }
        doc.panels[p].layers = layers;
        doc.panels[p].length = len;
    }
    doc
}

#[test]
fn faster_camera_image_matches_the_reference() {
    let doc = doc_with_drawing(1, 1);
    let c = doc.default_camera();
    let cameras = [
        c,
        Camera { x: c.x + 37.3, y: c.y - 12.6, ..c },
        Camera { zoom: 1.7, angle: 23.0, ..c },
        // Dézoomée : voit au-delà de la feuille.
        Camera { zoom: 0.4, angle: -8.0, ..c },
        // À moitié hors de la feuille.
        Camera { x: 10.0, y: 170.0, zoom: 1.3, ..c },
        // Entièrement hors de la feuille.
        Camera { x: -400.0, ..c },
    ];
    for paper in [None, Some([250, 245, 235])] {
        let plan = render::panel_plan_at(&doc, 0, 0, paper);
        for cam in &cameras {
            let fast = render::camera_image(&doc, &plan, cam);
            let reference = camera_image_reference(&doc, &plan, cam);
            assert_eq!(fast.len(), reference.len());
            let diff = fast.iter().zip(&reference).filter(|(a, b)| a != b).count();
            assert_eq!(diff, 0, "caméra {cam:?}, papier {paper:?}");
        }
    }
}

#[test]
fn ranges_follow_the_montage() {
    let doc = doc_with_drawing(3, 4);
    assert_eq!(export::frames_for(&doc, Range::Montage, 1).len(), 12);
    assert_eq!(export::frames_for(&doc, Range::Panel, 1), vec![(1, 0), (1, 1), (1, 2), (1, 3)]);
    // Les trois panneaux sont dans la même scène.
    assert_eq!(export::frames_for(&doc, Range::Scene, 2).len(), 12);
}

#[test]
fn video_sizes_are_even() {
    assert_eq!(export::video_size([1920, 1080], false), [1920, 1080]);
    assert_eq!(export::video_size([1920, 1080], true), [960, 540]);
    assert_eq!(export::video_size([1517, 959], false), [1516, 958]);
    assert_eq!(export::video_size([1000, 750], true), [500, 374]);
}

#[test]
fn encoders_are_read_from_ffmpeg_list() {
    let list = " V....D libopenh264          OpenH264 H.264\n V....D h264_mf              H264 via MediaFoundation\n \
                VFS... prores_ks            Apple ProRes\n A....D aac                  AAC\n";
    let enc = export::parse_encoders(list);
    assert_eq!(enc, Encoders { h264: Some("libopenh264"), prores: true, aac: true });
    let enc = export::parse_encoders(" V....D libx264  x264\n V....D libopenh264  x\n");
    assert_eq!(enc.h264, Some("libx264"), "x264 d'abord");
    assert_eq!(export::parse_encoders("").h264, None);
}

#[test]
fn ffmpeg_arguments() {
    let enc = Encoders { h264: Some("libopenh264"), prores: true, aac: true };
    let out = Path::new("C:/film/essai.mp4");
    let wav = Path::new("C:/temp/son.wav");
    let a = export::video_args(Format::Mp4, &enc, [1920, 1080], 24, false, Some(wav), out).unwrap();
    let joined = a.join(" ");
    assert!(joined.contains("-f rawvideo -pix_fmt rgba -s 1920x1080 -framerate 24 -i -"), "{joined}");
    assert!(joined.contains("-map 0:v:0 -map 1:a:0"));
    assert!(joined.contains("crop=1920:1080:0:0,scale=out_color_matrix=bt709:out_range=tv,format=yuv420p"));
    assert!(joined.contains("-c:v libopenh264 -b:v 12442k -profile:v high"), "{joined}");
    assert!(joined.contains("-c:a aac") && joined.contains("-shortest") && joined.ends_with("essai.mp4"));
    // Moitié, sans son, ProRes.
    let a = export::video_args(Format::ProRes, &enc, [1920, 1080], 25, true, None, Path::new("x.mov")).unwrap();
    let joined = a.join(" ");
    assert!(joined.contains("scale=960:540:flags=area") && joined.contains("format=yuv422p10le"));
    assert!(joined.contains("-c:v prores_ks -profile:v 3") && !joined.contains("-map 1:a:0") && !joined.contains("-shortest"));
    // Encodeur absent : un message clair plutôt qu'un ffmpeg qui échoue.
    let none = Encoders::default();
    assert!(export::video_args(Format::Mp4, &none, [64, 64], 24, false, None, out).is_err());
    assert!(export::video_args(Format::ProRes, &none, [64, 64], 24, false, None, out).is_err());
}

#[test]
fn wav_mixdown() {
    let dir = std::env::temp_dir().join(format!("pelure-test-wav-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("son.wav");
    // Un son constant de 0,5 à gauche et −0,25 à droite, de l'échantillon 100 à 300.
    let pcm = Pcm::from_interleaved(&[0.5, -0.25].repeat(1000), 2, crate::audio::RATE);
    let clip = MixClip { pcm: Arc::new(pcm), start: 100, offset: 0, len: 200, gain: 1.0 };
    assert!(export::has_sound(std::slice::from_ref(&clip), 0, 150));
    assert!(!export::has_sound(std::slice::from_ref(&clip), 300, 50));
    export::write_wav(&path, vec![clip], 50, 400).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 44 + 400 * 4);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 48_000);
    let sample = |i: usize, ch: usize| i16::from_le_bytes(bytes[44 + i * 4 + ch * 2..][..2].try_into().unwrap());
    assert_eq!((sample(0, 0), sample(0, 1)), (0, 0), "avant l'extrait : silence");
    assert_eq!((sample(60, 0), sample(60, 1)), (16384, -8192), "pendant (échantillon 110)");
    assert_eq!(sample(399, 0), 0, "après");
    // Images → échantillons : 24 i/s = 2000 échantillons par image.
    assert_eq!(export::sample_span(24, 3, 2), (6000, 4000));
    std::fs::remove_dir_all(&dir).ok();
}

/// Fait tourner un export jusqu'au bout, comme l'interface (avec une limite de temps).
fn run(job: &mut ExportJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.outcome.is_none() {
        job.step(doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60), "export trop long");
    }
}

#[test]
fn image_sequence_export_writes_every_frame() {
    let mut doc = doc_with_drawing(2, 3);
    // Une caméra qui bouge dans le premier panneau, fixe dans le second (images tenues).
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, c), CameraKey::new(2, Camera { x: c.x + 20.0, ..c })];
    let dir = std::env::temp_dir().join(format!("pelure-test-png-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let mut job = ExportJob::images(&dir, "essai", frames, doc.frame, true);
    run(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, (1..=6).map(|i| format!("essai_{i:04}.png")).collect::<Vec<_>>());
    // La dernière image est bien celle que filme la caméra du second panneau.
    let img = image::open(dir.join("essai_0006.png")).unwrap().to_rgba8();
    let plan = render::panel_plan_at(&doc, 1, 2, Some(doc.paper));
    let expected = render::camera_image(&doc, &plan, &doc.panel_camera(1, 2));
    assert_eq!(img.get_pixel(40, 30).0, crate::raster::unpremultiply(expected[30 * 150 + 40]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn video_export_with_ffmpeg_if_installed() {
    let Some(ffmpeg) = export::find_ffmpeg(None) else {
        eprintln!("ffmpeg absent : test vidéo sauté");
        return;
    };
    let enc = export::probe(&ffmpeg).unwrap();
    if enc.h264.is_none() {
        eprintln!("ffmpeg sans H.264 : test vidéo sauté");
        return;
    }
    let doc = doc_with_drawing(2, 6);
    let dir = std::env::temp_dir().join(format!("pelure-test-video-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("essai.mp4");
    let wav = dir.join("son.wav");
    let pcm = Pcm::from_interleaved(&[0.3, 0.3].repeat(48_000), 2, crate::audio::RATE);
    export::write_wav(&wav, vec![MixClip { pcm: Arc::new(pcm), start: 0, offset: 0, len: 48_000, gain: 1.0 }], 0, 48_000).unwrap();
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let args = export::video_args(Format::Mp4, &enc, doc.frame, doc.fps, false, Some(&wav), &out).unwrap();
    let mut job = ExportJob::video(&ffmpeg, args, &out, frames, Some(wav.clone())).unwrap();
    run(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let size = std::fs::metadata(&out).unwrap().len();
    assert!(size > 1000, "vidéo vide ({size} octets)");
    assert!(!wav.exists(), "le son temporaire est effacé");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_a_video_removes_the_file() {
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return };
    let Ok(enc) = export::probe(&ffmpeg) else { return };
    if enc.h264.is_none() {
        return;
    }
    let doc = doc_with_drawing(1, 200);
    let dir = std::env::temp_dir().join(format!("pelure-test-annule-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("annule.mp4");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let args = export::video_args(Format::Mp4, &enc, doc.frame, doc.fps, false, None, &out).unwrap();
    let mut job = ExportJob::video(&ffmpeg, args, &out, frames, None).unwrap();
    job.step(&doc, Duration::from_millis(30));
    job.cancel();
    assert!(matches!(job.outcome, Some(Err(_))));
    assert!(!out.exists(), "la vidéo inachevée est effacée");
    std::fs::remove_dir_all(&dir).ok();
}
