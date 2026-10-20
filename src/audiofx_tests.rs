//! Tests des effets sonores : fondus, fondu enchaîné (puissance égale, poignées), clés de volume, égaliseur,
//! écho, solo, export, fichier `effets_son.json` et sa protection, coupe d'un extrait, annulation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::App;
use crate::audio::{self, ClipEnv, MixClip, MixState, Pcm, RATE};
use crate::audiofx::{self, bake_clip, build_mix, crossfade_gains, envelope_at, junction_frames, junction_handle, process, ClipFx, Echo, VolumeKey, FILE};
use crate::document::Document;
use crate::export;
use crate::project;

fn constant(frames: usize, v: f32) -> Arc<Pcm> {
    Arc::new(Pcm::from_interleaved(&vec![v; frames], 1, RATE))
}

fn sine(freq: f32, seconds: f32, amp: f32) -> Arc<Pcm> {
    let n = (seconds * RATE as f32) as usize;
    let v: Vec<f32> = (0..n).map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / RATE as f32).sin()).collect();
    Arc::new(Pcm::from_interleaved(&v, 1, RATE))
}

/// Un projet à 24 i/s avec un son constant (0,5) de 2 secondes ; une piste vide : (document, son).
/// Le son est un vrai fichier WAV (il s'enregistre et se relit avec le projet).
fn doc_with_sound() -> (Document, u64) {
    let mut doc = Document::new(32, 32, 24);
    let n = 2 * RATE as usize;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + 2 * n as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(2 * n as u32).to_le_bytes());
    for _ in 0..n {
        bytes.extend_from_slice(&16384i16.to_le_bytes());
    }
    let pcm = audio::decode(&bytes, "wav").expect("WAV fabriqué");
    let sound = doc.add_sound("bruit.wav".into(), "wav".into(), Arc::new(bytes), Arc::new(pcm));
    doc.add_audio_track();
    (doc, sound)
}

/// Ce que rend le mixeur (la lecture) pour les échantillons `from..from + n` : le canal gauche.
fn rendered(clips: Vec<MixClip>, from: u64, n: usize) -> Vec<f32> {
    let mut m = MixState { clips, playing: true, pos: from as f64, range: (from, from + n as u64), looping: false, scrub: None };
    let mut out = vec![0.0; 2 * n];
    m.render(&mut out, 1.0);
    out.chunks(2).map(|f| f[0]).collect()
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-audiofx-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rms(p: &Pcm, from: usize, to: usize) -> f64 {
    let s: f64 = (from..to).map(|i| (p.frame(i).0 as f64).powi(2)).sum();
    (s / (to - from) as f64).sqrt()
}

// ----------------------------------------------------------------------------- fondus

#[test]
fn fades_ramp_the_volume_linearly_at_both_ends() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let (t, c) = doc.find_clip(clip).unwrap();
    doc.audio_tracks[t].clips[c].fx.fade_in = 6;
    doc.audio_tracks[t].clips[c].fx.fade_out = 12;
    // 24 i/s : une image = 2000 échantillons ; l'extrait dure 48 000 échantillons.
    let out = rendered(build_mix(&doc, true), 0, 48_000);
    let at = |i: usize| out[i];
    assert_eq!(at(0), 0.0, "le fondu d'entrée part du silence");
    assert!((at(6_000) - 0.25).abs() < 1e-3, "à la moitié du fondu d'entrée : la moitié de 0,5 ({})", at(6_000));
    assert!((at(12_000) - 0.5).abs() < 1e-3 && (at(24_000) - 0.5).abs() < 1e-3, "plein volume entre les deux");
    assert!((at(36_000) - 0.25).abs() < 2e-3, "à la moitié du fondu de sortie ({})", at(36_000));
    assert!(at(47_999) < 1e-3, "la sortie finit au silence ({})", at(47_999));
    // Monotone sur chaque fondu.
    assert!((0..12_000).step_by(500).map(at).collect::<Vec<_>>().windows(2).all(|w| w[0] <= w[1]));
    assert!((36_000..48_000).step_by(500).map(at).collect::<Vec<_>>().windows(2).all(|w| w[0] >= w[1]));
}

#[test]
fn the_clip_volume_and_the_track_volume_multiply() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.audio_tracks[0].volume = 0.5;
    doc.master_volume = 0.8;
    let (t, c) = doc.find_clip(clip).unwrap();
    doc.audio_tracks[t].clips[c].fx.gain = 1.5;
    let out = rendered(build_mix(&doc, true), 1_000, 4);
    assert!((out[0] - 0.5 * 0.5 * 0.8 * 1.5).abs() < 1e-3, "{}", out[0]);
}

#[test]
fn a_crossfade_between_touching_clips_keeps_the_power_constant_and_uses_the_handles() {
    let (mut doc, sound) = doc_with_sound();
    // Deux extraits qui se touchent à l'image 24, chacun avec du son de l'autre côté de sa coupe.
    let a = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let b = doc.add_clip(0, sound, 24, 24, 24).unwrap();
    let (t, ca) = doc.find_clip(a).unwrap();
    doc.audio_tracks[t].clips[ca].fx.xfade = 12;
    // Poignée : six images de chaque côté de la jonction = 12 000 échantillons.
    let (ka, kb) = (doc.clip(a).unwrap().clone(), doc.clip(b).unwrap().clone());
    assert_eq!(junction_handle(&doc, &ka, &kb), 12_000);
    assert_eq!(junction_frames(&doc, &doc.audio_tracks[0], 0), 12.0);
    let clips = build_mix(&doc, true);
    assert_eq!(clips.len(), 2);
    let (ma, mb) = (&clips[0], &clips[1]);
    // L'extrait sortant se prolonge de six images, l'entrant commence six images plus tôt.
    assert_eq!((ma.start, ma.len), (0, 48_000 + 12_000));
    assert_eq!((mb.start, mb.offset, mb.len), (48_000 - 12_000, 48_000 - 12_000, 48_000 + 12_000));
    // Aux deux bouts du fondu, un seul extrait ; au milieu, 1/√2 de chaque côté ; la puissance reste égale.
    let (ga, gb) = (|s: u64| ma.gain_at(s), |s: u64| mb.gain_at(s - mb.start));
    assert!((ga(36_000) - 1.0).abs() < 1e-5 && gb(36_000) == 0.0, "début du fondu");
    assert!((ga(48_000) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4 && (gb(48_000) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4, "au milieu");
    assert!(ga(59_999) < 1e-3 && (gb(59_999) - 1.0).abs() < 1e-3, "fin du fondu");
    for s in (36_000..60_000).step_by(997) {
        let p = ga(s).powi(2) + gb(s).powi(2);
        assert!((p - 1.0).abs() < 1e-4, "puissance {p} à {s}");
    }
    // Entendu : le son constant garde son niveau en puissance, jamais de trou ni de bosse au-delà de +3 dB.
    let heard = rendered(clips, 36_000, 24_000);
    let (lo, hi) = heard.iter().fold((f32::MAX, 0.0f32), |(l, h), v| (l.min(*v), h.max(*v)));
    assert!(lo > 0.49 && hi < 0.5 * 2f32.sqrt() + 1e-3, "{lo} {hi}");
    assert!((heard[12_000] - 0.5 * 2.0 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3, "somme des deux au milieu");
}

#[test]
fn without_handles_the_crossfade_is_a_hard_cut() {
    let (mut doc, sound) = doc_with_sound();
    // Le suivant n'a pas de son avant lui (décalage 0) : pas de poignée.
    doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.add_clip(0, sound, 24, 0, 24).unwrap();
    doc.audio_tracks[0].clips[0].fx.xfade = 12;
    assert_eq!(junction_frames(&doc, &doc.audio_tracks[0], 0), 0.0);
    let clips = build_mix(&doc, true);
    assert_eq!(clips[0].len, 48_000);
    assert!(clips[0].env.is_none() && clips[1].env.is_none(), "rien à faire dans le mixeur");
    // Des extraits qui ne se touchent pas : de même.
    doc.audio_tracks[0].clips[1].start = 30;
    doc.audio_tracks[0].clips[1].offset = 24;
    assert_eq!(junction_frames(&doc, &doc.audio_tracks[0], 0), 0.0);
}

#[test]
fn the_handle_is_limited_by_the_sound_available_on_each_side() {
    let (mut doc, sound) = doc_with_sound();
    // Le suivant n'a que deux images de son avant lui : le fondu se réduit à deux images de chaque côté.
    doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.add_clip(0, sound, 24, 2, 24).unwrap();
    doc.audio_tracks[0].clips[0].fx.xfade = 12;
    let (a, b) = (doc.audio_tracks[0].clips[0].clone(), doc.audio_tracks[0].clips[1].clone());
    assert_eq!(junction_handle(&doc, &a, &b), 4_000);
    assert_eq!(junction_frames(&doc, &doc.audio_tracks[0], 0), 4.0);
}

#[test]
fn crossfade_gains_are_equal_power() {
    for k in 0..=20 {
        let (out, inn) = crossfade_gains(k as f64 / 20.0);
        assert!((out * out + inn * inn - 1.0).abs() < 1e-5);
    }
    assert_eq!(crossfade_gains(0.0).0, 1.0);
    assert!(crossfade_gains(1.0).0.abs() < 1e-6 && (crossfade_gains(1.0).1 - 1.0).abs() < 1e-6);
}

// ------------------------------------------------------------------------------------ clés

#[test]
fn volume_keys_interpolate_linearly_and_hold_outside() {
    let mut fx = ClipFx::default();
    assert_eq!(fx.key_gain(5.0), 1.0, "sans clé : inchangé");
    // Posées dans le désordre, rangées par temps.
    fx.set_key(10.0, 0.0);
    fx.set_key(0.0, 1.0);
    fx.set_key(4.0, 2.0);
    assert_eq!(fx.keys.iter().map(|k| k.t).collect::<Vec<_>>(), vec![0.0, 4.0, 10.0]);
    assert_eq!(fx.key_gain(0.0), 1.0);
    assert_eq!(fx.key_gain(2.0), 1.5, "entre 1 et 2");
    assert_eq!(fx.key_gain(4.0), 2.0);
    assert!((fx.key_gain(7.0) - 1.0).abs() < 1e-6, "entre 2 et 0");
    assert_eq!(fx.key_gain(10.0), 0.0);
    assert_eq!(fx.key_gain(50.0), 0.0, "tenu après la dernière clé");
    // Une clé au même instant remplace l'ancienne.
    fx.set_key(4.0, 0.5);
    assert_eq!(fx.keys.len(), 3);
    assert_eq!(fx.key_gain(4.0), 0.5);
    // Une clé tirée reste entre ses voisines et dans ses bornes.
    fx.move_key(1, 99.0, 9.0);
    assert!(fx.keys[1].t < fx.keys[2].t && fx.keys[1].t > fx.keys[0].t);
    assert_eq!(fx.keys[1].v, audiofx::MAX_GAIN);
    fx.move_key(1, -5.0, -1.0);
    assert!(fx.keys[1].t > 0.0 && fx.keys[1].v == 0.0);
    // La courbe en échantillons : même calcul.
    assert_eq!(envelope_at(&[(0.0, 1.0), (100.0, 0.0)], 25.0), 0.75);
    assert_eq!(envelope_at(&[], 3.0), 1.0);
    assert_eq!(envelope_at(&[(5.0, 0.3)], 0.0), 0.3);
}

#[test]
fn the_mixer_follows_the_volume_keys() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let (t, c) = doc.find_clip(clip).unwrap();
    // 1 → 0 sur les 12 premières images, puis remonte à 1 à la 24e.
    doc.audio_tracks[t].clips[c].fx.keys = vec![VolumeKey { t: 0.0, v: 1.0 }, VolumeKey { t: 12.0, v: 0.0 }, VolumeKey { t: 24.0, v: 1.0 }];
    let out = rendered(build_mix(&doc, true), 0, 48_000);
    assert!((out[0] - 0.5).abs() < 1e-3);
    assert!((out[12_000] - 0.25).abs() < 1e-3, "à mi-chemin de la descente ({})", out[12_000]);
    assert!(out[24_000].abs() < 1e-3, "au creux");
    assert!((out[36_000] - 0.25).abs() < 1e-3, "à mi-chemin de la remontée");
}

#[test]
fn cutting_a_clip_shares_its_effects_between_the_two_pieces() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 48).unwrap();
    {
        let (t, c) = doc.find_clip(clip).unwrap();
        let fx = &mut doc.audio_tracks[t].clips[c].fx;
        fx.fade_in = 4;
        fx.fade_out = 8;
        fx.gain = 1.5;
        fx.eq = [3.0, 0.0, -2.0];
        fx.keys = vec![VolumeKey { t: 0.0, v: 1.0 }, VolumeKey { t: 20.0, v: 0.5 }, VolumeKey { t: 40.0, v: 1.0 }];
    }
    let right = doc.split_clip(clip, 30).unwrap();
    let (l, r) = (doc.clip(clip).unwrap().fx.clone(), doc.clip(right).unwrap().fx.clone());
    assert_eq!((l.fade_in, l.fade_out, r.fade_in, r.fade_out), (4, 0, 0, 8), "chaque fondu reste de son côté");
    assert_eq!((l.gain, r.gain, l.eq, r.eq), (1.5, 1.5, [3.0, 0.0, -2.0], [3.0, 0.0, -2.0]));
    // La courbe est la même de part et d'autre : la valeur de la coupe est posée dans les deux.
    let at_cut = 0.5 + (1.0 - 0.5) * (10.0 / 20.0);
    assert!((l.key_gain(30.0) - at_cut).abs() < 1e-6 && (r.key_gain(0.0) - at_cut).abs() < 1e-6);
    assert!((r.key_gain(10.0) - 1.0).abs() < 1e-6, "la clé de l'image 40 est devenue la 10e du morceau de droite");
    assert!((l.key_gain(20.0) - 0.5).abs() < 1e-6);
}

#[test]
fn pulling_the_left_edge_keeps_the_keys_under_the_same_sound() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 40).unwrap();
    doc.audio_tracks[0].clips[0].fx.keys = vec![VolumeKey { t: 10.0, v: 0.0 }, VolumeKey { t: 30.0, v: 1.0 }];
    // On cache les 4 premières images : la clé de l'image 10 est maintenant à l'image 6.
    assert!(doc.set_clip_span(clip, 0, 4, 40, false));
    let ks = &doc.clip(clip).unwrap().fx.keys;
    assert_eq!((ks[0].t, ks[1].t), (6.0, 26.0));
    // On cache jusqu'après la première clé : la courbe garde la valeur qu'elle avait au bord.
    let before = doc.clip(clip).unwrap().fx.key_gain(10.0);
    assert!(doc.set_clip_span(clip, 0, 14, 40, false));
    let c = doc.clip(clip).unwrap();
    assert!((c.fx.key_gain(0.0) - before).abs() < 1e-6, "{:?}", c.fx.keys);
    // Découvrir du son à gauche décale les clés vers la droite.
    assert!(doc.set_clip_span(clip, 0, 10, 40, false));
    assert!(doc.clip(clip).unwrap().fx.keys.iter().all(|k| k.t >= 0.0));
}

// ------------------------------------------------------------------------- solo et muet

#[test]
fn solo_silences_the_other_tracks_even_a_muted_solo_one_is_heard() {
    let (mut doc, sound) = doc_with_sound();
    doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.add_audio_track();
    doc.add_clip(1, sound, 0, 0, 24).unwrap();
    assert_eq!(build_mix(&doc, true).len(), 2);
    doc.audio_tracks[1].solo = true;
    assert_eq!(build_mix(&doc, true).len(), 1, "seule la piste en solo s'entend");
    doc.audio_tracks[1].muted = true;
    assert_eq!(build_mix(&doc, true).len(), 1, "le solo l'emporte sur le muet");
    doc.audio_tracks[1].solo = false;
    assert_eq!(build_mix(&doc, true).len(), 1, "sans solo, la piste muette se tait");
    assert!(audiofx::audible(&doc, &doc.audio_tracks[0]) && !audiofx::audible(&doc, &doc.audio_tracks[1]));
}

// -------------------------------------------------------------- égaliseur, écho, thread

#[test]
fn the_equaliser_boosts_and_cuts_its_bands() {
    let measure = |freq: f32, band: usize, db: f32| {
        let pcm = sine(freq, 1.0, 0.1);
        let mut fx = ClipFx::default();
        fx.eq[band] = db;
        let out = process(&pcm, &fx);
        let (a, b) = (RATE as usize / 2, RATE as usize);
        rms(&out, a, b) / rms(&pcm, a, b)
    };
    // Basses : une étagère à +12 dB double (×4) un 50 Hz, et ne touche pas un 8 kHz.
    let low_boost = measure(50.0, 0, 12.0);
    assert!((3.5..4.3).contains(&low_boost), "graves +12 dB sur 50 Hz : ×{low_boost}");
    assert!((measure(8000.0, 0, 12.0) - 1.0).abs() < 0.05);
    // Aigus : −12 dB sur 10 kHz (×0,25) ; un 100 Hz reste en place.
    let high_cut = measure(10_000.0, 2, -12.0);
    assert!((0.22..0.29).contains(&high_cut), "aigus −12 dB sur 10 kHz : ×{high_cut}");
    assert!((measure(100.0, 2, -12.0) - 1.0).abs() < 0.05);
    // Médiums : +12 dB au centre de la cloche.
    let mid = measure(1000.0, 1, 12.0);
    assert!((3.6..4.3).contains(&mid), "médiums +12 dB sur 1 kHz : ×{mid}");
    // Aucun réglage : le son n'est pas refait.
    assert!(ClipFx::default().is_dry());
    let pcm = sine(440.0, 0.1, 0.3);
    assert!(Arc::ptr_eq(&audiofx::processed(&pcm, &ClipFx::default(), true), &pcm));
}

#[test]
fn the_echo_repeats_the_sound_later_and_weaker() {
    // Une impulsion : l'écho se lit sur les échantillons.
    let mut v = vec![0.0f32; 20_000];
    v[0] = 1.0;
    let pcm = Pcm::from_interleaved(&v, 1, RATE);
    let fx = ClipFx { echo: Echo { mix: 1.0, delay_ms: 100.0, feedback: 0.5 }, ..Default::default() };
    let out = process(&pcm, &fx);
    let at = |i: usize| out.frame(i).0;
    assert!((at(0) - 1.0).abs() < 2e-3, "le son sec");
    assert!((at(4_800) - 1.0).abs() < 2e-3, "premier retour, 100 ms plus tard ({})", at(4_800));
    assert!((at(9_600) - 0.5).abs() < 2e-3 && (at(14_400) - 0.25).abs() < 2e-3, "chaque retour est moitié plus faible");
    assert!(at(4_799).abs() < 1e-3 && at(100).abs() < 1e-3);
    // La queue de l'écho prolonge le son.
    assert!(out.frames() > pcm.frames() + 4_800);
    // Le niveau règle le mélange.
    let half = process(&pcm, &ClipFx { echo: Echo { mix: 0.5, delay_ms: 100.0, feedback: 0.0 }, ..Default::default() });
    assert!((half.frame(4_800).0 - 0.5).abs() < 2e-3);
}

#[test]
fn an_echoed_clip_ringing_to_the_end_of_its_sound_keeps_its_tail() {
    let (mut doc, _) = doc_with_sound();
    let short = doc.add_sound("coup.wav".into(), "wav".into(), Arc::new(Vec::new()), constant(24_000, 0.4));
    let clip = doc.add_clip(0, short, 0, 0, 12).unwrap();
    let (t, c) = doc.find_clip(clip).unwrap();
    doc.audio_tracks[t].clips[c].fx.echo = Echo { mix: 0.5, delay_ms: 200.0, feedback: 0.5 };
    let clips = build_mix(&doc, true);
    assert!(clips[0].len > 24_000, "la queue de l'écho sonne après la fin de l'extrait ({})", clips[0].len);
    // Un extrait coupé au milieu du son n'est pas prolongé (il révélerait la suite du son).
    doc.audio_tracks[t].clips[c].length = 6;
    assert_eq!(build_mix(&doc, true)[0].len, 12_000);
}

#[test]
fn the_playback_mix_never_waits_for_the_processing() {
    let pcm = sine(300.0, 20.0, 0.2);
    let fx = ClipFx { eq: [6.0, 0.0, 0.0], echo: Echo { mix: 0.3, delay_ms: 300.0, feedback: 0.4 }, ..Default::default() };
    let t0 = Instant::now();
    let first = audiofx::processed(&pcm, &fx, false);
    let took = t0.elapsed();
    assert!(Arc::ptr_eq(&first, &pcm), "en attendant, le son tel quel");
    assert!(took < Duration::from_millis(50), "la lecture n'attend pas le calcul : {took:?}");
    // Le calcul se fait à part, puis le résultat est pris.
    let t = Instant::now();
    while !audiofx::is_ready(&pcm, &fx) {
        assert!(t.elapsed() < Duration::from_secs(30), "calcul trop long");
        std::thread::sleep(Duration::from_millis(10));
    }
    let done = audiofx::processed(&pcm, &fx, false);
    assert!(!Arc::ptr_eq(&done, &pcm) && done.frames() > pcm.frames());
}

// ---------------------------------------------------------------- l'export et la lecture

#[test]
fn the_wav_export_applies_the_effects_exactly_like_playback() {
    let (mut doc, sound) = doc_with_sound();
    let a = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.add_clip(0, sound, 24, 24, 24).unwrap();
    {
        let (t, c) = doc.find_clip(a).unwrap();
        let fx = &mut doc.audio_tracks[t].clips[c].fx;
        fx.fade_in = 6;
        fx.xfade = 12;
        fx.gain = 1.2;
    }
    doc.audio_tracks[0].volume = 0.9;
    let dir = temp("wav");
    let path = dir.join("son.wav");
    let clips = audio::mix_clips_blocking(&doc);
    export::write_wav(&path, clips.clone(), 0, 96_000).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 44 + 96_000 * 4);
    let sample = |i: usize| i16::from_le_bytes(bytes[44 + i * 4..][..2].try_into().unwrap()) as f32 / 32767.0;
    let played = rendered(clips, 0, 96_000);
    for i in [0usize, 3_000, 6_000, 20_000, 36_000, 48_000, 55_000, 70_000, 95_000] {
        assert!((sample(i) - played[i]).abs() < 2.0 / 32767.0, "échantillon {i} : fichier {} / lecture {}", sample(i), played[i]);
    }
    // Les effets y sont : le fondu d'entrée (0 au départ, la moitié à mi-fondu), le volume.
    assert_eq!(sample(0), 0.0);
    assert!((sample(6_000) - 0.5 * 1.2 * 0.9 * 0.5).abs() < 2e-3, "{}", sample(6_000));
    assert!((sample(20_000) - 0.5 * 1.2 * 0.9).abs() < 2e-3);
    // Au milieu du fondu enchaîné : 1/√2 de chaque extrait (le second sans le volume de l'extrait).
    let mid = 0.5 * 0.9 * std::f32::consts::FRAC_1_SQRT_2 * (1.2 + 1.0);
    assert!((sample(48_000) - mid).abs() < 3e-3, "{} / {mid}", sample(48_000));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_scrubbed_sound_goes_through_the_same_mix() {
    let (mut doc, sound) = doc_with_sound();
    doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.audio_tracks[0].clips[0].fx.fade_in = 12;
    // Le défilement joue une image à partir de l'image 6 (milieu du fondu) avec les mêmes extraits.
    let mut m = MixState { clips: audio::mix_clips(&doc), scrub: Some((12_000.0, 14_000.0)), ..Default::default() };
    let mut out = vec![0.0; 2 * 4];
    m.render(&mut out, 1.0);
    assert!((out[0] - 0.25).abs() < 1e-3, "{}", out[0]);
}

#[test]
fn a_clip_exported_to_storyboard_pro_carries_its_effects() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    doc.audio_tracks[0].clips[0].fx.fade_out = 12;
    doc.audio_tracks[0].clips[0].fx.gain = 2.0;
    let samples = bake_clip(&doc, doc.clip(clip).unwrap());
    assert_eq!(samples.len(), 48_000);
    assert!((samples[10_000][0] - 1.0).abs() < 1e-3, "volume ×2 sur 0,5");
    assert!((samples[36_000][0] - 0.5).abs() < 2e-3, "à mi-fondu de sortie ({})", samples[36_000][0]);
    assert!(samples[47_999][0].abs() < 1e-3);
}

#[test]
fn the_envelope_is_pure_and_reads_the_same_everywhere() {
    let env = ClipEnv { lead: 0, nominal: 1000, fade_in: 100, fade_out: 100, xf_in: 0, xf_out_at: 0, xf_out: 0, keys: vec![(0.0, 1.0), (1000.0, 0.5)] };
    assert_eq!(env.gain_at(0), 0.0);
    assert!((env.gain_at(50) - 0.5 * (1.0 - 0.025)).abs() < 1e-4);
    assert!((env.gain_at(500) - 0.75).abs() < 1e-4);
    assert!(env.gain_at(999) < 0.01);
}

// --------------------------------------------------------------------------------- fichier

#[test]
fn the_effects_file_round_trips_with_the_project() {
    let (mut doc, sound) = doc_with_sound();
    let a = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let b = doc.add_clip(0, sound, 24, 24, 24).unwrap();
    doc.add_audio_track();
    {
        let (t, c) = doc.find_clip(a).unwrap();
        let fx = &mut doc.audio_tracks[t].clips[c].fx;
        fx.gain = 1.25;
        fx.fade_in = 3;
        fx.fade_out = 5;
        fx.xfade = 8;
        fx.keys = vec![VolumeKey { t: 2.0, v: 0.5 }, VolumeKey { t: 9.0, v: 1.5 }];
        fx.eq = [2.0, -3.0, 4.0];
        fx.echo = Echo { mix: 0.4, delay_ms: 180.0, feedback: 0.6 };
    }
    doc.audio_tracks[1].solo = true;
    let dir = temp("aller-retour");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(text.contains("\"extraits\"") && text.contains("fondu_entree") && text.contains("egaliseur") && text.contains("\"solo\""), "{text}");
    // L'extrait sans effet n'y est pas.
    assert!(!text.contains(&format!("\"extrait\": {b}")), "{text}");
    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    assert_eq!(loaded.doc.audio_tracks, doc.audio_tracks);
    // Les effets des sons n'ont pas de version du projet à eux : ils sont à côté du manifeste (la 27 est
    // celle des calques d'effets).
    assert_eq!(project::VERSION, 31);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn without_effects_there_is_no_file() {
    let (mut doc, sound) = doc_with_sound();
    doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let dir = temp("sans-effet");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists());
    doc.audio_tracks[0].clips[0].fx.fade_in = 2;
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(FILE).exists());
    doc.audio_tracks[0].clips[0].fx = ClipFx::default();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists(), "plus d'effet : le fichier part");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_damaged_or_newer_effects_file_is_never_deleted() {
    let (mut doc, sound) = doc_with_sound();
    doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let dir = temp("protege");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();

    std::fs::write(dir.join(FILE), "{ pas du json").unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    assert!(loaded.doc.audio_tracks[0].clips[0].fx.is_default());
    project::save(&loaded.doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "{ pas du json", "rien à écrire : le fichier reste");

    let newer = r#"{"format":"shot-effets-son","version":99,"extraits":[]}"#;
    std::fs::write(dir.join(FILE), newer).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.iter().any(|w| w.contains("plus récente")), "{:?}", loaded.warnings);
    project::save(&loaded.doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), newer);

    // Un effet neuf à écrire : l'ancien fichier est mis de côté.
    doc.audio_tracks[0].clips[0].fx.gain = 0.5;
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("effets_son.json.illisible")).unwrap(), newer);
    assert_eq!(project::load(&manifest).unwrap().doc.audio_tracks[0].clips[0].fx.gain, 0.5);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn reading_the_file_brings_wild_values_back_into_range() {
    let text = r#"{"format":"shot-effets-son","version":1,"extraits":[
        {"extrait":1,"volume":1e9,"fondu_entree":999999999,"egaliseur":[99.0,-99.0,0.0],
         "cles":[{"t":5.0,"v":3.0},{"t":1.0,"v":-4.0},{"t":null,"v":1.0}],
         "echo":{"niveau":7.0,"delai_ms":1.0,"retour":5.0}},
        {"extrait":1,"volume":0.1}
    ],"pistes":[{"piste":9,"solo":true}]}"#;
    // (Une clé sans temps est refusée à la lecture : le fichier entier est illisible, jamais à moitié lu.)
    assert!(audiofx::parse(text).is_err());
    let text = text.replace(r#",{"t":null,"v":1.0}"#, "");
    let l = audiofx::parse(&text).unwrap();
    assert_eq!(l.clips.len(), 1, "un seul réglage par extrait");
    let fx = &l.clips[0].1;
    assert_eq!(fx.gain, audiofx::MAX_GAIN);
    assert_eq!(fx.eq, [audiofx::EQ_RANGE, -audiofx::EQ_RANGE, 0.0]);
    assert_eq!((fx.echo.mix, fx.echo.delay_ms, fx.echo.feedback), (1.0, 20.0, 0.9));
    assert!(fx.keys.windows(2).all(|w| w[0].t <= w[1].t) && fx.keys.iter().all(|k| (0.0..=audiofx::MAX_GAIN).contains(&k.v)));
    assert_eq!(l.solo, vec![9]);
    // Un numéro inconnu est ignoré à l'application.
    let (mut doc, _) = doc_with_sound();
    l.install(&mut doc);
    assert!(!doc.audio_tracks[0].solo);
}

// --------------------------------------------------------------------------- l'annulation

fn app_with(doc: Document, name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-audiofx-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app
}

#[test]
fn every_effect_edit_is_one_named_undo_step() {
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let mut app = app_with(doc, "annuler");
    app.sel_clip = Some(clip);
    app.cur_panel = 0;
    app.run_action("son.fondu_entree");
    assert_eq!(app.doc.clip(clip).unwrap().fx.fade_in, crate::audiofxui::DEFAULT_FADE);
    assert_eq!(app.history.names().0.last().copied(), Some("Fondu d'entrée"));
    app.run_action("son.fondu_sortie");
    assert_eq!(app.history.names().0.last().copied(), Some("Fondu de sortie"));
    // Une clé à la tête de lecture (dans l'extrait).
    app.cur_frame = 8;
    app.run_action("son.cle_volume");
    assert_eq!(app.doc.clip(clip).unwrap().fx.keys.len(), 1);
    assert_eq!(app.history.names().0.last().copied(), Some("Clé de volume"));
    // Suppr, le montage ayant la main et la clé étant choisie : une étape, la clé part.
    app.panel_focus = true;
    app.run_action("edition.supprimer");
    assert!(app.doc.clip(clip).unwrap().fx.keys.is_empty());
    assert_eq!(app.history.names().0.last().copied(), Some("Supprimer la clé de volume"));
    // Le solo d'une piste.
    app.run_action("son.solo");
    assert!(app.doc.audio_tracks[0].solo);
    assert_eq!(app.history.names().0.last().copied(), Some("Piste en solo"));

    // Chaque étape se défait, dans l'ordre.
    app.undo();
    assert!(!app.doc.audio_tracks[0].solo);
    app.undo();
    assert_eq!(app.doc.clip(clip).unwrap().fx.keys.len(), 1);
    app.undo();
    assert!(app.doc.clip(clip).unwrap().fx.keys.is_empty());
    app.undo();
    assert_eq!(app.doc.clip(clip).unwrap().fx.fade_out, 0);
    app.undo();
    assert!(app.doc.clip(clip).unwrap().fx.is_default());
}

#[test]
fn dragging_a_fade_handle_is_a_single_step_and_escape_restores() {
    use crate::soundtrack::ClipEdge;
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let mut app = app_with(doc, "glisser");
    let (ppf, lanes_top) = (10.0f32, 100.0f32);
    // La poignée du fondu d'entrée est au coin du haut gauche de l'extrait choisi.
    app.sel_clip = Some(clip);
    let origin = egui::pos2(6.0 + 0.5, lanes_top + 4.0);
    let hit = app.lane_hit(origin, 6.0, ppf, lanes_top);
    assert_eq!(hit.and_then(|h| h.1).map(|h| h.1), Some(ClipEdge::FadeIn));
    app.begin_clip_drag(clip, ClipEdge::FadeIn, origin);
    for dx in [20.0, 40.0, 60.0] {
        app.update_clip_drag(egui::pos2(origin.x + dx, origin.y), ppf, lanes_top);
    }
    assert_eq!(app.doc.clip(clip).unwrap().fx.fade_in, 6, "6 images à 10 points par image");
    assert_eq!(app.history.names().0.len(), 0, "rien dans l'historique tant que le geste dure");
    if let Some(crate::storyboard::MontageDrag::Clip(d)) = app.montage_drag.take() {
        app.finish_clip_drag(d);
    }
    assert_eq!(app.history.names().0, vec!["Fondu d'entrée"]);
    app.undo();
    assert_eq!(app.doc.clip(clip).unwrap().fx.fade_in, 0);

    // Échap pendant le geste : tout revient sans étape.
    app.begin_clip_drag(clip, ClipEdge::FadeOut, egui::pos2(250.0, lanes_top + 4.0));
    app.update_clip_drag(egui::pos2(200.0, lanes_top + 4.0), ppf, lanes_top);
    assert_eq!(app.doc.clip(clip).unwrap().fx.fade_out, 5);
    app.cancel_montage_drag();
    assert_eq!(app.doc.clip(clip).unwrap().fx.fade_out, 0);
}

#[test]
fn a_double_click_puts_a_key_and_the_key_can_be_dragged() {
    use crate::soundtrack::ClipEdge;
    let (mut doc, sound) = doc_with_sound();
    let clip = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let mut app = app_with(doc, "cles");
    let (left, ppf, lanes_top) = (6.0f32, 10.0f32, 100.0f32);
    app.sel_clip = Some(clip);
    // Au milieu de l'extrait (image 12), à mi-hauteur : le gain 1.
    let cr = crate::audiofxui::clip_rect(0, app.doc.clip(clip).unwrap(), left, ppf, lanes_top);
    let p = egui::pos2(cr.left() + 12.0 * ppf, crate::audiofxui::gain_y(cr, 1.0));
    app.double_click_clip(0, clip, p, left, ppf, lanes_top);
    let keys = app.doc.clip(clip).unwrap().fx.keys.clone();
    assert_eq!(keys, vec![VolumeKey { t: 12.0, v: 1.0 }]);
    assert_eq!(app.sfx.key, Some((clip, 0)));
    // Elle se prend à la souris (l'extrait étant choisi) et se tire vers le bas et vers la droite.
    let hit = app.lane_hit(p, left, ppf, lanes_top);
    assert_eq!(hit.and_then(|h| h.1), Some((clip, ClipEdge::Key(0))));
    app.begin_clip_drag(clip, ClipEdge::Key(0), p);
    app.update_clip_drag(egui::pos2(p.x + 30.0, crate::audiofxui::gain_y(cr, 0.25)), ppf, lanes_top);
    let k = app.doc.clip(clip).unwrap().fx.keys[0];
    assert_eq!(k.t, 15.0);
    assert!((k.v - 0.25).abs() < 0.03, "{}", k.v);
}

#[test]
fn a_crossfade_is_set_from_the_command_only_between_touching_clips() {
    let (mut doc, sound) = doc_with_sound();
    let a = doc.add_clip(0, sound, 0, 0, 24).unwrap();
    let b = doc.add_clip(0, sound, 30, 30, 10).unwrap();
    let mut app = app_with(doc, "enchaine");
    app.sel_clip = Some(a);
    app.run_action("son.enchaine");
    assert_eq!(app.doc.clip(a).unwrap().fx.xfade, 0, "le suivant ne touche pas : refusé");
    // On le colle contre le premier.
    assert!(app.doc.set_clip_span(b, 0, 24, 34, false));
    app.run_action("son.enchaine");
    assert_eq!(app.doc.clip(a).unwrap().fx.xfade, audiofx::DEFAULT_XFADE);
    assert_eq!(app.history.names().0.last().copied(), Some("Fondu enchaîné du son"));
    app.run_action("son.enchaine");
    assert_eq!(app.doc.clip(a).unwrap().fx.xfade, 0, "la commande bascule");
}

#[test]
fn the_new_sound_commands_are_in_the_palette() {
    for id in ["son.fondu_entree", "son.fondu_sortie", "son.enchaine", "son.cle_volume", "son.solo", "son.effets_reinit"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
}

#[test]
fn level_per_frame_follows_the_effects() {
    let fx = ClipFx { gain: 2.0, fade_in: 4, ..Default::default() };
    assert!((audiofx::level_in_frame(&fx, 0.0, 24) - 2.0 * 0.5 / 4.0).abs() < 1e-5);
    assert_eq!(audiofx::level_in_frame(&fx, 10.0, 24), 2.0);
}
