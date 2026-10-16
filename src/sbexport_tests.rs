//! Export vers Storyboard Pro (14b-1) : ce qui part, le script, le résultat, et les images et
//! les sons préparés (sans Storyboard Pro : l'exécutable donné n'existe pas).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::audio::{Pcm, RATE};
use crate::document::{Document, ImageMark, Layer, LayerKind};
use crate::sbexport::{self, parse_result, prepare, safe_name, SbExportJob, Stage};

/// Deux plans dans deux séquences. Le premier (10 images, deux calques, celui du dessus réglé
/// à 60 %) a une marque d'image à l'image 4 ; le second (6 images) a un dossier caché à 50 %
/// dont le calque (le dessin du dessus, sans sa transformation) change de dessin à l'image 3. Une piste son à 50 %, coupée, avec un extrait.
fn sample() -> Document {
    let mut doc = crate::export_tests::doc_with_drawing(1, 10);
    doc.panels[0].action = "Plan large".into();
    doc.panels[0].marks.push(ImageMark {
        id: 9_000_001,
        frame: 4,
        action: "Il court".into(),
        dialogue: "Vite !\n\"Allez\"".into(),
        ..Default::default()
    });
    let i = doc.add_sequence_after(0);
    let a = doc.panels[0].layers[0].frames[0].unwrap();
    let b = doc.panels[0].layers[1].frames[0].unwrap();
    let mut folder = Layer::new_folder("Dossier");
    folder.visible = false;
    folder.opacity = 0.5;
    let mut child = Layer::new("Perso", LayerKind::Vector);
    child.parent = Some(folder.id);
    child.frames = vec![Some(b), Some(b), Some(b), Some(a), Some(a), Some(a)];
    let p = &mut doc.panels[i];
    p.layers = vec![folder, child];
    p.length = 6;
    p.notes = "Fin".into();

    doc.add_audio_track();
    doc.audio_tracks[0].volume = 0.5;
    doc.audio_tracks[0].muted = true;
    let pcm = Pcm::from_interleaved(&vec![0.5f32; 2 * RATE as usize], 2, RATE);
    let sound = doc.add_sound("bruit.wav".into(), "wav".into(), Arc::new(Vec::new()), Arc::new(pcm));
    doc.add_clip(0, sound, 3, 2, 6).expect("extrait");
    doc
}

#[test]
fn titles_become_folder_names() {
    assert_eq!(safe_name("  Épisode 3 : la fuite?. "), "Épisode 3 _ la fuite_");
    assert_eq!(safe_name("a/b\\c*"), "a_b_c_");
    assert_eq!(safe_name(" .. "), "Projet Pelure");
}

#[test]
fn plans_and_storyboard_images_become_scenes_and_panels() {
    let doc = sample();
    let out = prepare(&doc, " Essai ");
    assert_eq!(out.title, "Essai");
    assert_eq!((out.fps, out.size), (12, [150, 90]));
    assert_eq!(out.scenes.len(), 2);
    let s0 = &out.scenes[0];
    assert_eq!(s0.panels.iter().map(|p| (p.start, p.frames)).collect::<Vec<_>>(), vec![(0, 4), (4, 6)]);
    assert_eq!(s0.panels[0].action, "Plan large");
    assert_eq!((s0.panels[1].action.as_str(), s0.panels[1].dialogue.as_str()), ("Il court", "Vite !\n\"Allez\""));
    let names: Vec<&str> = s0.panels[0].layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["C0", "C1"], "du fond au premier plan");
    assert!((s0.panels[0].layers[1].opacity - 0.6).abs() < 1e-6);
    assert!(s0.panels[0].layers.iter().all(|l| l.drawn && l.visible && !l.vector));
    // Le dossier ne part pas ; son calque en prend la visibilité et l'opacité.
    let s1 = &out.scenes[1];
    assert_eq!(s1.panels.len(), 1);
    let l = &s1.panels[0].layers;
    assert_eq!(l.len(), 1);
    assert_eq!((l[0].name.as_str(), l[0].vector, l[0].visible), ("Perso", true, false));
    assert!((l[0].opacity - 0.5).abs() < 1e-6);
    assert_eq!(s1.panels[0].notes, "Fin");
    // Deux séquences : chacune son plan.
    assert_eq!(out.sequences.iter().map(|s| (s.first, s.last)).collect::<Vec<_>>(), vec![(0, 0), (1, 1)]);
    assert_eq!(out.tracks.len(), 1);
    assert_eq!((out.tracks[0].clips[0].start, out.tracks[0].clips[0].file.as_str()), (3, "sons/piste01_001.wav"));
    let notes = out.notes.join(" | ");
    assert!(notes.contains("Animation : 1 calque"), "{notes}");
    assert!(notes.contains("Dossiers"), "{notes}");
    assert!(notes.contains("coupée"), "{notes}");
    assert_eq!(sbexport::counts(&out), (2, 3, 5, 1));

    // Une seule séquence sans nom : pas de séquence à créer.
    let one = prepare(&crate::export_tests::doc_with_drawing(1, 4), "x");
    assert!(one.sequences.is_empty());
}

/// Les données écrites dans le script, relues.
fn data_of(script: &str) -> Value {
    let line = script.lines().find(|l| l.starts_with("var PELURE = ")).expect("données");
    serde_json::from_str(line.trim_start_matches("var PELURE = ").trim_end_matches(';')).expect("JSON")
}

#[test]
fn the_script_carries_the_project() {
    let doc = sample();
    let mut out = prepare(&doc, "Titre \u{2028} « 2 »");
    out.scenes[0].panels[0].layers[0].image = Some("images/0001_001_001.png".into());
    let s = sbexport::script(&out, Path::new(r"C:\tmp\données"), Path::new(r"C:\sortie\Titre"));
    assert!(!s.contains("@DONNEES@") && !s.contains("@DESTINATION@") && !s.contains("@PELURE@"));
    assert!(s.contains("var PELURE_DONNEES = \"C:/tmp/données\";"));
    assert!(s.contains("var PELURE_DESTINATION = \"C:/sortie/Titre\";"));
    assert!(!s.contains('\u{2028}') && s.contains("\\u2028"), "séparateur de ligne échappé (ECMAScript 3)");
    // Pour l'essayer hors de Pelure (imitation de l'interface de script) : PELURE_SCRIPT_ESSAI.
    if let Some(p) = std::env::var_os("PELURE_SCRIPT_ESSAI") {
        std::fs::write(p, &s).unwrap();
    }
    let d = data_of(&s);
    assert_eq!(d["titre"], "Titre \u{2028} « 2 »");
    let p1 = &d["plans"][0]["panneaux"][1];
    assert_eq!(p1["duree"], 6);
    assert_eq!(p1["dialogue"], "Vite !\n\"Allez\"");
    let c = &d["plans"][0]["panneaux"][0]["calques"];
    assert_eq!(c[0]["image"], "images/0001_001_001.png");
    assert_eq!(c[1]["image"], "", "pas encore calculée : calque vide");
    assert_eq!(c[1]["opacite"], 60.0);
    assert_eq!(d["plans"][1]["panneaux"][0]["calques"][0]["visible"], false);
    assert_eq!(d["sequences"][1]["premier"], 1);
    assert_eq!(d["pistes"][0]["extraits"][0]["debut"], 3);
    // Les fonctions de l'interface de script de Storyboard Pro employées.
    for f in ["appendScene(", "insertPanel(true", "setPanelDuration(", "importImageAsLayer(", "setLayerOpacity(", "createSequence(", "importSoundBuffer(", "saveAs(", "deleteScene("] {
        assert!(s.contains(f), "{f}");
    }
}

#[test]
fn the_script_result_is_read() {
    let text = "pelure-construction\t1\r\nplans\t12\nenregistre\t0\t1\nprojet\t24\t1920\t1080\nremarque\tplan 3 : nom refuse\nremarque\tson a.wav\tnon importe\n";
    let r = parse_result(text).unwrap();
    assert_eq!(r.scenes, 12);
    assert!(!r.saved_as && r.saved_all);
    assert_eq!((r.fps, r.size), (Some(24.0), Some([1920, 1080])));
    assert_eq!(r.remarks, vec!["plan 3 : nom refuse".to_string(), "son a.wav non importe".to_string()]);
    assert!(r.error.is_none());
    assert_eq!(parse_result("pelure-construction\t1\nerreur\tTypeError: x\n").unwrap().error.as_deref(), Some("TypeError: x"));
    assert!(parse_result("autre chose").is_err());
}

#[test]
fn layer_images_and_sounds_are_prepared() {
    let doc = sample();
    let base = std::env::temp_dir().join(format!("pelure-sbexport-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let template = base.join("modele").join("modele.sboard");
    std::fs::create_dir_all(template.parent().unwrap()).unwrap();
    std::fs::write(&template, "<project/>").unwrap();
    std::fs::create_dir_all(template.parent().unwrap().join("logs")).unwrap();
    let out = prepare(&doc, "Essai");
    let mut job = SbExportJob::start(out, base.join("absent.exe"), template, base.join("sortie").join("Essai")).unwrap();
    assert_eq!(job.total_images(), 5);
    let t0 = Instant::now();
    while job.running() && t0.elapsed() < Duration::from_secs(30) {
        job.step(&doc, Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(20));
    }
    let Stage::Done(res) = &job.stage else { panic!("pas fini") };
    let err = res.as_ref().expect_err("Storyboard Pro absent");
    assert!(err.contains("ne démarre pas"), "{err}");
    let data = job.work.join("donnees");
    // Les deux images de storyboard du premier plan ont les mêmes dessins : une image par
    // calque, partagée. Le calque du dossier : son premier dessin.
    let mut pngs: Vec<String> = std::fs::read_dir(data.join("images")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    pngs.sort();
    assert_eq!(pngs, ["0001_001_001.png", "0001_001_002.png", "0002_001_001.png"]);
    let img = image::open(data.join("images").join("0001_001_002.png")).unwrap();
    assert_eq!((img.width(), img.height()), (150, 90), "le cadre");
    let script = std::fs::read_to_string(data.join(sbexport::SCRIPT)).unwrap();
    let d = data_of(&script);
    assert_eq!(d["plans"][0]["panneaux"][1]["calques"][1]["image"], "images/0001_001_002.png");
    assert_eq!(d["plans"][1]["panneaux"][0]["calques"][0]["image"], "images/0002_001_001.png");
    // Le modèle copié, sans ses journaux.
    assert!(job.work.join("projet").join("modele.sboard").is_file());
    assert!(!job.work.join("projet").join("logs").exists());
    // L'extrait : images 2 à 8 du son (6 images à 12 i/s = ½ s), au volume de la piste.
    let wav = std::fs::read(data.join("sons").join("piste01_001.wav")).unwrap();
    assert_eq!(wav.len(), 44 + RATE as usize / 2 * 4);
    let v = i16::from_le_bytes([wav[44], wav[45]]);
    assert!((v as i32 - 8192).abs() < 4, "0,5 × 0,5 : {v}");
    let _ = std::fs::remove_dir_all(&job.work);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn an_empty_layer_image_is_not_written() {
    // Un calque dont le dessin est vide à cette image : pas d'image, calque vide.
    let mut doc = crate::export_tests::doc_with_drawing(1, 3);
    let empty = doc.new_cel(LayerKind::Bitmap);
    doc.panels[0].layers[0].frames = vec![Some(empty); 3];
    let out = prepare(&doc, "Vide");
    let template = std::env::temp_dir().join(format!("pelure-sbexport-vide-{}", std::process::id()));
    let mut job = SbExportJob::start(out, PathBuf::from("absent.exe"), template.join("m.sboard"), template.join("x")).unwrap();
    let t0 = Instant::now();
    while matches!(job.stage, Stage::Images) && t0.elapsed() < Duration::from_secs(10) {
        job.step(&doc, Duration::from_millis(50));
    }
    let script = std::fs::read_to_string(job.work.join("donnees").join(sbexport::SCRIPT)).unwrap();
    let d = data_of(&script);
    assert_eq!(d["plans"][0]["panneaux"][0]["calques"][0]["image"], "");
    assert_ne!(d["plans"][0]["panneaux"][0]["calques"][1]["image"], "");
    job.cancel();
    while job.running() && t0.elapsed() < Duration::from_secs(10) {
        job.step(&doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = std::fs::remove_dir_all(&job.work);
}

#[test]
fn an_empty_template_is_read() {
    // Un modèle vide (aucun panneau) : lisible comme modèle, pas comme projet à importer.
    let src = r#"<?xml version="1.0"?><project><options><resolution size="1280,720"/><framerate val="25"/></options>
<scenes><scene name="Top" id="top"><columns><column type="0" name="ATV-1"/></columns></scene></scenes><timeline><scene id="top"/></timeline></project>"#;
    let path = std::env::temp_dir().join(format!("pelure-modele-{}.sboard", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let sb = crate::sboard::read_template(&path).expect("modèle vide lisible");
    assert_eq!((sb.size, sb.fps, sb.shots.len()), ([1280, 720], 25, 0));
    assert!(crate::sboard::read(&path).is_err());
    let _ = std::fs::remove_file(&path);
}
