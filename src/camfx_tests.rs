//! Effets de caméra en préréglages : clés déterministes pour une graine, dans les bornes de
//! l'intensité, une explosion qui s'éteint, des zooms qui finissent à la valeur demandée, un
//! vertigo qui garde la taille du sujet, des clés ordinaires que l'on peut défaire.

use crate::camera::{camera_at, ref_distance, Camera, CameraKey};
use crate::camfx::{self, Generator, Params, Plan, Preset, MAX_KEYS};
use crate::document::Document;
use crate::history::{Edit, History, Nav};

const FRAME: [usize; 2] = [200, 100];
const FPS: u32 = 24;

fn centered() -> Camera {
    Camera::centered(200, 100)
}

/// Pose l'effet `p` à l'image `start` d'un plan de `len` images dont les clés sont `keys` ; renvoie le plan.
fn run(p: &Params, start: usize, len: usize, keys: &mut Vec<CameraKey>) -> Plan {
    let g = Generator::new(p, FPS);
    let extra: Vec<usize> = keys.iter().map(|k| k.frame).collect();
    let original = keys.clone();
    let base = move |f: usize| camera_at(&original, f, centered());
    let plan = camfx::plan(&g, FRAME, start, len, &extra, !extra.is_empty(), &base);
    camfx::write(keys, &plan);
    plan
}

fn shakes() -> [Preset; 4] {
    [Preset::Shoulder, Preset::LightShake, Preset::Explosion, Preset::Jolt]
}

fn samples(p: &Params) -> Vec<camfx::Sample> {
    let g = Generator::new(p, FPS);
    g.frames().into_iter().map(|f| g.at(f)).collect()
}

fn peak(s: &[camfx::Sample]) -> f32 {
    s.iter().map(|s| s.dx.hypot(s.dy)).fold(0.0, f32::max)
}

#[test]
fn the_same_seed_gives_the_same_keys_and_another_seed_gives_others() {
    for preset in shakes() {
        let p = Params::new(preset, FPS);
        assert_eq!(samples(&p), samples(&p), "{}", preset.label());
        let again = Params { seed: 1, ..p };
        assert_eq!(samples(&p), samples(&again));
        let other = Params { seed: 2, ..p };
        assert_ne!(samples(&p), samples(&other), "{} : une autre graine donne une autre secousse", preset.label());
    }
    // Les clés écrites dans un plan sont les mêmes, elles aussi.
    let p = Params::new(Preset::Shoulder, FPS);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    run(&p, 10, 200, &mut a);
    run(&p, 10, 200, &mut b);
    assert_eq!(a, b);
}

#[test]
fn offsets_stay_within_the_bounds_of_the_intensity() {
    for preset in shakes() {
        for intensity in [0.25, 1.0, 2.5] {
            for seed in 0..25 {
                let p = Params { intensity, seed, punch: true, ..Params::new(preset, FPS) };
                let g = Generator::new(&p, FPS);
                let b = g.bounds();
                let mut top = 0.0_f32;
                for f in 0..=p.duration {
                    let s = g.at(f);
                    assert!(s.dx.abs() <= b.pos + 1e-6 && s.dy.abs() <= b.pos + 1e-6, "{} i={intensity} graine {seed} image {f} : {s:?} > {b:?}", preset.label());
                    assert!(s.angle.abs() <= b.angle + 1e-6, "{} rotation {} > {}", preset.label(), s.angle, b.angle);
                    assert!((s.zoom - 1.0).abs() <= b.punch + 1e-6, "{} zoom {}", preset.label(), s.zoom);
                    top = top.max(s.dx.abs()).max(s.dy.abs());
                }
                assert!(top > 0.0, "{} bouge", preset.label());
            }
        }
    }
}

#[test]
fn the_intensity_scales_the_offsets_and_bounds() {
    for preset in shakes() {
        let one = Params { intensity: 1.0, ..Params::new(preset, FPS) };
        let two = Params { intensity: 2.0, ..one };
        for (a, b) in samples(&one).iter().zip(samples(&two).iter()) {
            assert!((b.dx - 2.0 * a.dx).abs() < 1e-6 && (b.dy - 2.0 * a.dy).abs() < 1e-6 && (b.angle - 2.0 * a.angle).abs() < 1e-5, "{}", preset.label());
        }
        let (b1, b2) = (Generator::new(&one, FPS).bounds(), Generator::new(&two, FPS).bounds());
        assert!((b2.pos - 2.0 * b1.pos).abs() < 1e-6 && (b2.angle - 2.0 * b1.angle).abs() < 1e-6);
    }
    // L'explosion est plus forte que la caméra à l'épaule (secousses comprises), plus forte que la
    // secousse rapide, plus forte que le tremblement léger.
    let top = |p: Preset| Generator::new(&Params::new(p, FPS), FPS).bounds().pos;
    assert!(top(Preset::Explosion) > top(Preset::Shoulder) && top(Preset::Shoulder) > top(Preset::Jolt) && top(Preset::Jolt) > top(Preset::LightShake));
}

#[test]
fn continuous_shakes_start_and_end_calm_and_abrupt_ones_start_hard_and_end_calm() {
    for preset in [Preset::Shoulder, Preset::LightShake] {
        let s = samples(&Params::new(preset, FPS));
        assert_eq!((s[0].dx, s[0].dy, s[0].angle), (0.0, 0.0, 0.0), "{} démarre calme", preset.label());
    }
    for preset in shakes() {
        let s = samples(&Params::new(preset, FPS));
        let last = s.last().unwrap();
        assert_eq!((last.dx, last.dy, last.angle, last.zoom), (0.0, 0.0, 0.0, 1.0), "{} finit sur la caméra de base", preset.label());
    }
    for preset in [Preset::Explosion, Preset::Jolt] {
        let s = samples(&Params::new(preset, FPS));
        assert!(s[0].dx.hypot(s[0].dy) > 0.2 * Generator::new(&Params::new(preset, FPS), FPS).bounds().pos, "{} frappe d'emblée", preset.label());
        assert!(preset.is_abrupt());
    }
}

#[test]
fn the_explosion_decays() {
    for seed in 0..20 {
        let p = Params { seed, ..Params::new(Preset::Explosion, FPS) };
        let s = samples(&p);
        assert_eq!(s.len(), p.duration + 1, "une clé par image");
        let third = s.len() / 3;
        let (first, last) = (peak(&s[..third]), peak(&s[s.len() - third..]));
        assert!(first > 4.0 * last, "graine {seed} : début {first} fin {last}");
        // Et l'énergie baisse de tiers en tiers.
        let energy = |a: &[camfx::Sample]| a.iter().map(|s| s.dx * s.dx + s.dy * s.dy).sum::<f32>();
        let (e1, e2, e3) = (energy(&s[..third]), energy(&s[third..2 * third]), energy(&s[2 * third..]));
        assert!(e1 > e2 && e2 > e3, "graine {seed} : {e1} {e2} {e3}");
        // Le coup de zoom retombe lui aussi.
        let zooms: Vec<f32> = s.iter().map(|s| s.zoom).collect();
        assert!(zooms[0] > 1.05 && zooms[0] > zooms[third] && zooms[third] > zooms[2 * third] && *zooms.last().unwrap() == 1.0);
    }
    // Sans le coup de zoom : jamais de zoom.
    let no_punch = samples(&Params { punch: false, ..Params::new(Preset::Explosion, FPS) });
    assert!(no_punch.iter().all(|s| s.zoom == 1.0));
}

#[test]
fn the_key_density_stays_reasonable() {
    // Une clé par image pour une explosion de 1,2 s, une clé sur deux pour la caméra à l'épaule.
    assert_eq!(Generator::new(&Params::new(Preset::Explosion, FPS), FPS).frames().len(), 29 + 1);
    let shoulder = Generator::new(&Params::new(Preset::Shoulder, FPS), FPS).frames();
    assert_eq!(shoulder.len(), 96 / 2 + 1);
    assert!(shoulder.windows(2).all(|w| w[1] - w[0] == 2));
    // Un effet très long espace ses clés au lieu d'en poser des milliers.
    for preset in Preset::ALL {
        let p = Params { duration: 20_000, ..Params::new(preset, FPS) };
        let n = Generator::new(&p, FPS).frames().len();
        assert!(n <= MAX_KEYS + 1, "{} : {n} clés", preset.label());
    }
    // Les zooms n'en posent que deux, le vertigo deux douzaines.
    assert_eq!(Generator::new(&Params::new(Preset::ZoomIn, FPS), FPS).frames().len(), 2);
    assert_eq!(Generator::new(&Params::new(Preset::CrashZoom, FPS), FPS).frames().len(), 2);
    assert_eq!(Generator::new(&Params::new(Preset::Vertigo, FPS), FPS).frames().len(), 25);
    // Mille clés de caméra se lisent sans peine : la caméra de chaque image d'un plan de 5000 images.
    let mut keys = Vec::new();
    let p = Params { duration: 4000, ..Params::new(Preset::LightShake, FPS) };
    run(&p, 0, 5000, &mut keys);
    assert!(keys.len() > 900 && keys.len() <= MAX_KEYS + 2, "{}", keys.len());
    let t = std::time::Instant::now();
    let mut sum = 0.0;
    for f in 0..5000 {
        sum += camera_at(&keys, f, centered()).x;
    }
    assert!(sum.is_finite());
    assert!(t.elapsed().as_millis() < 2000, "5000 images lues en {:?}", t.elapsed());
}

#[test]
fn zoom_presets_end_at_the_requested_value() {
    let base = centered();
    for (preset, factor, want) in [
        (Preset::ZoomIn, 1.5, 1.5),
        (Preset::ZoomIn, 3.0, 3.0),
        (Preset::ZoomOut, 2.0, 0.5),
        (Preset::CrashZoom, 3.0, 3.0),
        (Preset::Vertigo, 2.0, 2.0),
    ] {
        let p = Params { zoom_factor: factor, ..Params::new(preset, FPS) };
        let mut keys = Vec::new();
        let plan = run(&p, 5, 400, &mut keys);
        let end = 5 + p.duration;
        assert_eq!(plan.last_frame(), Some(end), "{}", preset.label());
        let z = |f: usize| camera_at(&keys, f, centered()).zoom;
        assert!((z(5) - base.zoom).abs() < 1e-6, "{} part de la focale actuelle", preset.label());
        assert!((z(end) - want * base.zoom).abs() < 1e-4, "{} : {} au lieu de {want}", preset.label(), z(end));
        assert!((z(end + 100) - want).abs() < 1e-4, "{} garde la focale d'arrivée", preset.label());
        assert!((camera_at(&keys, end, centered()).focal() - 35.0 * want).abs() < 0.01);
    }
    // Un zoom adouci : à mi-parcours, la moitié du chemin (géométrique) ; un zoom choc a déjà fait l'essentiel.
    let mut keys = Vec::new();
    let smooth = Params { zoom_factor: 4.0, ..Params::new(Preset::ZoomIn, FPS) };
    run(&smooth, 0, 400, &mut keys);
    let mid = camera_at(&keys, smooth.duration / 2, centered()).zoom;
    assert!((mid - 2.0).abs() < 0.02, "à mi-chemin : {mid}");
    let q = camera_at(&keys, smooth.duration / 4, centered()).zoom;
    assert!(q < 4.0_f32.powf(0.25), "départ adouci : {q}");
    let mut crash = Vec::new();
    let sharp = Params { zoom_factor: 4.0, ..Params::new(Preset::CrashZoom, FPS) };
    run(&sharp, 0, 400, &mut crash);
    let q = camera_at(&crash, sharp.duration / 2, centered()).zoom;
    assert!(q > 4.0_f32.powf(0.6), "le zoom choc a déjà fait l'essentiel à mi-parcours : {q}");
    assert!(q > camera_at(&keys, sharp.duration / 2, centered()).zoom, "plus vite que le zoom adouci");
}

#[test]
fn zoom_presets_start_from_the_current_focal_and_never_reset_the_camera() {
    // La caméra du plan est à 70 mm, décalée et tournée : le zoom part de là.
    let b = Camera { x: 130.0, y: 40.0, zoom: 2.0, angle: 7.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
    let mut keys = vec![CameraKey::new(0, b)];
    let p = Params { zoom_factor: 1.5, ..Params::new(Preset::ZoomIn, FPS) };
    let g = Generator::new(&p, FPS);
    let original = keys.clone();
    let plan = camfx::plan(&g, FRAME, 3, 200, &[0], true, &move |f| camera_at(&original, f, centered()));
    camfx::write(&mut keys, &plan);
    let end = camera_at(&keys, 3 + p.duration, centered());
    assert!((end.zoom - 3.0).abs() < 1e-5 && (end.focal() - 105.0).abs() < 1e-3, "70 mm × 1,5 = 105 mm");
    assert_eq!((end.x, end.y, end.angle), (130.0, 40.0, 7.0), "la position et la rotation ne bougent pas");
}

#[test]
fn the_vertigo_keeps_the_subject_size() {
    let p = Params { zoom_factor: 2.0, ..Params::new(Preset::Vertigo, FPS) };
    let mut keys = Vec::new();
    run(&p, 4, 400, &mut keys);
    let d0 = ref_distance(FRAME);
    let m0 = centered().magnification(FRAME);
    for f in 4..=4 + p.duration {
        let c = camera_at(&keys, f, centered());
        assert!((c.magnification(FRAME) / m0 - 1.0).abs() < 1e-3, "image {f} : le sujet a changé de taille ({})", c.magnification(FRAME));
    }
    let end = camera_at(&keys, 4 + p.duration, centered());
    assert!((end.zoom - 2.0).abs() < 1e-5, "la focale double");
    assert!((end.distance(FRAME) - 2.0 * d0).abs() < 1e-2, "et la caméra recule d'autant : {}", end.distance(FRAME));
    // Le fond, lui, bouge : un calque en profondeur ne garde pas sa taille (c'est tout l'effet).
    assert!(end.dolly < 0.0, "la caméra a reculé");
}

#[test]
fn the_effect_adds_to_the_current_camera_and_keeps_the_other_keys() {
    // Un travelling de 0 à 100 sur 48 images, une clé plus loin ; l'effet couvre les 48 premières images.
    let mut a = CameraKey::new(0, Camera { x: 0.0, ..centered() });
    a.ease_out = 5.0;
    let mut keys = vec![a, CameraKey::new(48, Camera { x: 100.0, ..centered() }), CameraKey::new(80, Camera { x: 100.0, y: 10.0, ..centered() })];
    let original = keys.clone();
    let p = Params { duration: 48, ..Params::new(Preset::Shoulder, FPS) };
    let g = Generator::new(&p, FPS);
    run(&p, 0, 200, &mut keys);
    let bound = g.bounds().pos * FRAME[0] as f32;
    for f in (0..=48).step_by(2) {
        let k = keys.iter().find(|k| k.frame == f).unwrap_or_else(|| panic!("clé {f}"));
        let base = camera_at(&original, f, centered());
        assert!((k.cam.x - base.x).abs() <= bound + 1e-3 && (k.cam.y - base.y).abs() <= bound + 1e-3, "image {f} : ajouté à la caméra de base");
    }
    assert_eq!(keys[0].ease_out, 5.0, "les adoucis d'une clé reprise sont gardés");
    assert_eq!(keys.last().unwrap(), &original[2], "la clé d'après l'effet ne bouge pas");
    assert!(keys.windows(2).all(|w| w[0].frame < w[1].frame), "clés triées, une par image");
    // Les extrémités de l'effet retombent sur la caméra de base.
    assert!((keys.iter().find(|k| k.frame == 48).unwrap().cam.x - 100.0).abs() < 1e-4);
    assert!((keys[0].cam.x - 0.0).abs() < 1e-4);
    // Une clé existante hors de la grille de l'effet est gardée et reçoit aussi le décalage.
    let mut odd = vec![CameraKey::new(0, centered()), CameraKey::new(7, Camera { x: 150.0, ..centered() })];
    run(&p, 0, 200, &mut odd);
    let k7 = odd.iter().find(|k| k.frame == 7).expect("la clé 7 reste");
    assert!((k7.cam.x - 150.0).abs() <= bound + 1e-3 && k7.cam.x != 150.0, "elle est décalée, pas écrasée : {}", k7.cam.x);
}

#[test]
fn effect_keys_are_ordinary_camera_keys() {
    let mut keys = Vec::new();
    let p = Params::new(Preset::Explosion, FPS);
    let plan = run(&p, 10, 100, &mut keys);
    // Plan sans clé, effet qui commence plus tard et d'un coup : une clé de début et une clé calme juste avant.
    assert_eq!(plan.home.map(|h| h.0), Some(0));
    assert_eq!(plan.before.map(|h| h.0), Some(9));
    let frames: Vec<usize> = keys.iter().map(|k| k.frame).collect();
    assert_eq!(&frames[..3], &[0, 9, 10]);
    assert_eq!(*frames.last().unwrap(), 10 + p.duration);
    assert_eq!(keys[1].cam, centered(), "la clé d'avant est la caméra de base");
    assert_eq!(keys[0].cam, centered());
    // Ni adoucis ni rien d'étrange : des clés comme les autres.
    assert!(keys.iter().all(|k| !k.smooth && k.ease_in == 0.0 && k.ease_out == 0.0));
    // Elles se modifient comme les autres.
    crate::camera::set_key(&mut keys, 12, Camera { x: 5.0, ..centered() });
    assert_eq!(keys.iter().find(|k| k.frame == 12).unwrap().cam.x, 5.0);
    // Au début du plan : pas de clé calme avant (il n'y a pas d'image avant), pas de clé de début en plus.
    let mut at_start = Vec::new();
    let plan = run(&p, 0, 100, &mut at_start);
    assert!(plan.home.is_none() && plan.before.is_none());
    assert_eq!(at_start[0].frame, 0);
}

#[test]
fn an_effect_is_cut_by_the_end_of_the_plan_and_refused_past_it() {
    // Plan de 20 images : le zoom (48 images) est coupé à l'image 19.
    let mut keys = Vec::new();
    let plan = run(&Params::new(Preset::ZoomIn, FPS), 10, 20, &mut keys);
    assert!(plan.truncated);
    assert_eq!(plan.last_frame(), Some(19));
    assert!(keys.iter().all(|k| k.frame < 20));
    // Départ après la fin du plan : rien.
    let mut keys = Vec::new();
    let plan = run(&Params::new(Preset::ZoomIn, FPS), 25, 20, &mut keys);
    assert!(plan.keys.is_empty() && keys.is_empty());
    // Un effet qui tient dans le plan n'est pas coupé.
    let plan = run(&Params::new(Preset::Jolt, FPS), 2, 100, &mut Vec::new());
    assert!(!plan.truncated);
}

#[test]
fn settings_are_kept_in_safe_bounds() {
    let wild = Params { intensity: f32::NAN, duration: 0, freq: f32::INFINITY, zoom_factor: -3.0, ..Params::new(Preset::Shoulder, FPS) }.sanitized(FPS);
    assert!(wild.intensity.is_finite() && wild.duration >= 1 && wild.freq <= 12.0 && wild.zoom_factor >= 1.0);
    // Chaque préréglage a des réglages de départ utilisables.
    for preset in Preset::ALL {
        let p = Params::new(preset, FPS);
        assert!(p.duration >= 1 && p.intensity == 1.0 && p.zoom_factor >= 1.0, "{}", preset.label());
        assert_eq!(p.sanitized(FPS), p);
        assert!(!preset.label().is_empty() && !preset.note().is_empty());
    }
    // Changer de préréglage garde l'intensité et la graine, reprend le reste.
    let p = Params { intensity: 1.7, seed: 42, ..Params::new(Preset::Shoulder, FPS) }.with_preset(Preset::ZoomIn, FPS);
    assert_eq!((p.intensity, p.seed, p.preset), (1.7, 42, Preset::ZoomIn));
    assert_eq!(p.duration, 48);
}

#[test]
fn an_effect_is_one_named_undo_step() {
    let mut doc = Document::new(200, 100, 24);
    doc.panels[0].set_length(120);
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut history = History::default();
    let before = doc.panels[0].camera.clone();
    assert!(before.is_empty());
    let p = Params::new(Preset::Explosion, FPS);
    let snap = Edit::Structure { structure: doc.structure(), nav };
    let g = Generator::new(&p, FPS);
    let plan = camfx::plan(&g, doc.frame, 10, doc.panels[0].frame_count(), &[], false, &|f| doc.panel_camera(0, f));
    camfx::write(&mut doc.panels[0].camera, &plan);
    let name = history.context.replace(format!("Effet de caméra : {}", p.preset.label()));
    history.push(snap);
    history.context = name;
    assert_eq!(history.names().0, vec!["Effet de caméra : Explosion / impact"]);
    assert!(doc.panels[0].camera.len() > 20);
    assert!(history.undo(&mut doc, &mut nav));
    assert!(doc.panels[0].camera.is_empty(), "une seule annulation défait tout l'effet");
    assert!(history.redo(&mut doc, &mut nav));
    assert!(doc.panels[0].camera.len() > 20);
}
