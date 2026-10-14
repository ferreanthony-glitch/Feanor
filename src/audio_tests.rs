//! Tests du son : conversion, mixage, extraits sur les pistes, enregistrement.

use std::sync::Arc;

use crate::audio::{self, MixClip, MixState, Pcm, PEAK_BLOCK, RATE};
use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::project;

/// Fichier WAV 16 bits mono : `n` échantillons à `rate`, tous à la valeur `v`.
fn wav(rate: u32, n: usize, v: i16) -> Vec<u8> {
    let data_len = (n * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for _ in 0..n {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// Son constant à `v`, déjà au format interne, de `frames` échantillons.
fn constant(frames: usize, v: f32) -> Arc<Pcm> {
    Arc::new(Pcm::from_interleaved(&vec![v; frames], 1, RATE))
}

/// Document à 24 i/s avec un son de `seconds` secondes (valeur 0,5).
fn doc_with_sound(seconds: f64) -> (Document, u64) {
    let mut doc = Document::new(32, 32, 24);
    let pcm = constant((seconds * RATE as f64) as usize, 0.5);
    let id = doc.add_sound("essai.wav".into(), "wav".into(), Arc::new(Vec::new()), pcm);
    (doc, id)
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn resampling_and_stereo() {
    // 1 s en mono à 24 kHz → 1 s en stéréo à 48 kHz, la voix copiée dans les deux canaux.
    let src: Vec<f32> = (0..24_000).map(|i| if i < 12_000 { 0.25 } else { -0.5 }).collect();
    let pcm = Pcm::from_interleaved(&src, 1, 24_000);
    assert_eq!(pcm.frames(), RATE as usize);
    let (l, r) = pcm.frame(100);
    assert!((l - 0.25).abs() < 0.001 && l == r);
    assert!((pcm.frame(40_000).0 + 0.5).abs() < 0.001);
    assert_eq!(pcm.peaks.len(), (RATE as usize).div_ceil(PEAK_BLOCK));
    assert!((pcm.peak_between(0, 1000) - 0.25).abs() < 0.01);
    assert!((pcm.peak_between(0, RATE as usize) - 0.5).abs() < 0.01);
}

#[test]
fn decode_wav_and_reject_garbage() {
    let pcm = audio::decode(&wav(44_100, 44_100, 16_384), "wav").expect("WAV lisible");
    // 1 s à 44,1 kHz → 1 s à 48 kHz (à un échantillon près).
    assert!((pcm.frames() as i64 - RATE as i64).abs() <= 1);
    assert!((pcm.frame(1000).0 - 0.5).abs() < 0.001);
    assert!(audio::decode(b"pas du son du tout", "mp3").is_err());
}

#[test]
fn mixer_positions_loop_and_stop() {
    let clip = MixClip {
        pcm: constant(1000, 0.5),
        start: 100,
        offset: 0,
        len: 1000,
        gain: 1.0,
    };
    let mut m = MixState {
        clips: vec![clip.clone()],
        playing: true,
        pos: 0.0,
        range: (0, 300),
        looping: false,
        scrub: None,
    };
    let mut out = vec![0.0; 600];
    m.render(&mut out, 1.0);
    assert_eq!(out[2 * 99], 0.0, "avant l'extrait : silence");
    assert!((out[2 * 100] - 0.5).abs() < 0.001, "l'extrait commence à 100");
    assert!(!m.playing && m.pos == 300.0, "arrêt à la fin de la plage");

    m.playing = true;
    m.looping = true;
    m.pos = 0.0;
    let mut out = vec![0.0; 2 * 350];
    m.render(&mut out, 1.0);
    assert!(m.playing);
    assert_eq!(m.pos, 50.0, "en boucle, on repart du début de la plage");

    // Décalage : l'extrait commence à l'échantillon 900 du son, qui n'en a que 1000.
    m.clips = vec![MixClip { offset: 900, ..clip }];
    m.pos = 100.0 + 150.0;
    m.looping = false;
    m.range = (0, 2000);
    let mut out = vec![0.0; 2];
    m.render(&mut out, 1.0);
    assert_eq!(out[0], 0.0, "au-delà de la fin du son : silence");
}

#[test]
fn scrub_plays_once() {
    let mut m = MixState {
        clips: vec![MixClip {
            pcm: constant(1000, 0.5),
            start: 0,
            offset: 0,
            len: 1000,
            gain: 0.5,
        }],
        scrub: Some((10.0, 20.0)),
        ..Default::default()
    };
    let mut out = vec![0.0; 2 * 15];
    m.render(&mut out, 1.0);
    assert!((out[0] - 0.25).abs() < 0.001, "le gain de la piste s'applique");
    assert_eq!(out[2 * 12], 0.0, "le défilement ne joue qu'une image");
    assert!(m.scrub.is_none());
}

#[test]
fn mix_skips_muted_tracks_and_converts_frames() {
    let (mut doc, sound) = doc_with_sound(2.0);
    doc.place_sound(sound, 24, None).unwrap();
    let clips = audio::mix_clips(&doc);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].start, 48_000, "image 24 à 24 i/s = 1 s");
    assert_eq!(clips[0].len, 96_000);
    doc.audio_tracks[0].muted = true;
    assert!(audio::mix_clips(&doc).is_empty());
}

#[test]
fn clips_on_tracks() {
    let (mut doc, sound) = doc_with_sound(1.0);
    assert_eq!(doc.sounds[&sound].frames(24), 24);
    let a = doc.place_sound(sound, 0, None).unwrap();
    assert_eq!(doc.audio_tracks.len(), 1, "première piste créée à l'import");
    // Même endroit : la piste est prise, une deuxième est créée.
    let b = doc.place_sound(sound, 10, Some(0)).unwrap();
    assert_eq!(doc.audio_tracks.len(), 2);
    assert_eq!(doc.find_clip(b).unwrap().0, 1);
    // Plus loin : la première piste est libre.
    let c = doc.place_sound(sound, 30, None).unwrap();
    assert_eq!(doc.find_clip(c).unwrap().0, 0);
    assert_eq!(doc.audio_end(), 54);

    // Déplacer a sur c : refusé ; à côté : accepté.
    assert!(!doc.set_clip_span(a, 0, 20, 44, true));
    assert!(doc.set_clip_span(a, 0, 5, 29, true));
    assert_eq!(doc.clip(a).unwrap().start, 5);

    // Bord gauche : on cache le début du son (le son reste sous les mêmes images).
    assert!(doc.set_clip_span(a, 0, 9, 29, false));
    let ca = doc.clip(a).unwrap();
    assert_eq!((ca.start, ca.offset, ca.length), (9, 4, 20));
    // On ne peut pas découvrir plus que le son, ni avant son début.
    assert!(!doc.set_clip_span(a, 0, 9, 30, false), "au-delà de la fin du son");
    assert!(!doc.set_clip_span(a, 0, 4, 29, false), "avant le début du son");

    // Couper : deux morceaux qui se suivent, le son continue.
    let right = doc.split_clip(a, 19).unwrap();
    let (l, r) = (doc.clip(a).unwrap().clone(), doc.clip(right).unwrap().clone());
    assert_eq!((l.start, l.length, l.offset), (9, 10, 4));
    assert_eq!((r.start, r.length, r.offset), (19, 10, 14));
    assert!(doc.split_clip(a, 9).is_none(), "pas de coupe au bord");
    assert!(doc.remove_clip(right));
    assert!(doc.clip(right).is_none());
}

#[test]
fn undo_restores_clips_but_keeps_track_settings() {
    let (mut doc, sound) = doc_with_sound(1.0);
    let clip = doc.place_sound(sound, 0, None).unwrap();
    let snap = Edit::Structure {
        structure: doc.structure(),
        nav: Nav { panel: 0, layer: 0 },
    };
    assert!(doc.set_clip_span(clip, 0, 30, 54, true));
    doc.audio_tracks[0].muted = true;
    doc.audio_tracks[0].volume = 0.4;
    let mut history = History::default();
    history.push(snap);
    let mut nav = Nav { panel: 0, layer: 0 };
    history.undo(&mut doc, &mut nav);
    assert_eq!(doc.clip(clip).unwrap().start, 0, "le déplacement est annulé");
    assert!(doc.audio_tracks[0].muted, "le réglage de la piste reste");
    assert_eq!(doc.audio_tracks[0].volume, 0.4);
}

#[test]
fn project_with_sound_round_trip() {
    let dir = temp_dir("son");
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let mut doc = Document::new(32, 32, 24);
    let bytes = wav(48_000, 24_000, 8_000);
    let pcm = audio::decode(&bytes, "wav").unwrap();
    let sound = doc.add_sound("voix.wav".into(), "wav".into(), Arc::new(bytes.clone()), Arc::new(pcm));
    let clip = doc.place_sound(sound, 12, None).unwrap();
    doc.split_clip(clip, 18).unwrap();
    doc.audio_tracks[0].name = "Voix".into();
    doc.audio_tracks[0].volume = 0.8;
    doc.add_audio_track();

    let mut written = Default::default();
    project::save(&doc, &manifest, &mut written).unwrap();
    let file = manifest.parent().unwrap().join("sons").join(format!("{sound}.wav"));
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "le son est copié tel quel");

    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let back = loaded.doc;
    assert_eq!(back.audio_tracks, doc.audio_tracks);
    assert_eq!(back.sounds[&sound].name, "voix.wav");
    assert_eq!(back.sounds[&sound].pcm.frames(), 24_000);
    // Les identifiants neufs ne recouvrent pas ceux des sons et extraits chargés.
    let mut again = back;
    let t = again.add_audio_track();
    assert!(t > clip && t > sound);

    // Plus d'extrait : le fichier son disparaît au prochain enregistrement.
    for track in &mut doc.audio_tracks {
        track.clips.clear();
    }
    project::save(&doc, &manifest, &mut written).unwrap();
    assert!(!file.exists());

    // Un son manquant n'empêche pas d'ouvrir : on le signale.
    let place = doc.place_sound(sound, 0, None);
    assert!(place.is_some());
    project::save(&doc, &manifest, &mut written).unwrap();
    std::fs::remove_file(&file).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert_eq!(loaded.warnings.len(), 1);
    assert!(loaded.doc.audio_tracks.iter().all(|t| t.clips.is_empty()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn sound_file_name_cannot_escape_project() {
    let dir = temp_dir("son-chemin");
    let manifest = dir.join("P.pelure");
    let doc = Document::new(32, 32, 24);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["sons"] = serde_json::json!([{ "id": 99, "nom": "x", "fichier": "../../secret.wav" }]);
    json["pistes_son"] =
        serde_json::json!([{ "id": 98, "extraits": [{ "id": 97, "son": 99, "debut": 0, "duree": 10 }] }]);
    std::fs::write(&manifest, serde_json::to_vec(&json).unwrap()).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert_eq!(loaded.warnings.len(), 1);
    assert!(loaded.warnings[0].contains("invalide"));
    std::fs::remove_dir_all(&dir).ok();
}
