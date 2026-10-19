//! Préréglages d'effets (`prereglages.rs`) : chacun écrit les clés attendues, à partir de la tête de
//! lecture, de façon déterministe ; l'impact dure le nombre d'images voulu ; la secousse reste dans
//! les bornes de son intensité ; rien ne dépasse la fin du plan.

use crate::calqueffet::Contenu;
use crate::camfx::{Generator, Params, Preset};
use crate::fx::{Effect, EffectKind};
use crate::prereglages::{build, Construit, Contexte, Genre, Reglages};

fn ctx(debut: usize, images: usize) -> Contexte {
    Contexte { fps: 24, cadre: [1920, 1080], debut, images, centre: [50.0, 50.0], taille: [50.0, 50.0] }
}

fn made(genre: Genre, debut: usize, images: usize) -> Construit {
    build(&Reglages::new(genre, 24), &ctx(debut, images)).expect("un préréglage")
}

fn effect(c: &Construit) -> &Effect {
    c.fx.first().expect("un effet")
}

#[test]
fn every_preset_builds_the_same_keys_every_time() {
    for genre in Genre::ALL {
        for seed in [1, 7] {
            let r = Reglages { graine: seed, ..Reglages::new(genre, 24) };
            let (a, b) = (build(&r, &ctx(10, 100)).unwrap(), build(&r, &ctx(10, 100)).unwrap());
            assert_eq!(a, b, "{genre:?}");
            assert!(!a.nom.is_empty() && a.longueur >= 1 && a.debut == 10, "{genre:?}");
        }
    }
    // La graine change ce qui est tiré au hasard.
    for genre in [Genre::Impact, Genre::Tremblement, Genre::Grain] {
        let (a, b) = (
            build(&Reglages { graine: 1, eclat: true, ..Reglages::new(genre, 24) }, &ctx(0, 100)).unwrap(),
            build(&Reglages { graine: 2, eclat: true, ..Reglages::new(genre, 24) }, &ctx(0, 100)).unwrap(),
        );
        assert_ne!(a.fx, b.fx, "{genre:?} : la graine compte");
    }
}

#[test]
fn the_impact_frame_lasts_the_asked_number_of_frames_and_is_keyed_on_and_off() {
    for n in [1usize, 2, 3, 8] {
        let r = Reglages { duree: Some(n), ..Reglages::new(Genre::Impact, 24) };
        let c = build(&r, &ctx(20, 100)).unwrap();
        assert_eq!((c.debut, c.longueur), (20, n), "{n} images");
        assert_eq!(effect(&c).kind, EffectKind::Impact);
        // Allumé sur toute sa durée, à pleine force.
        for f in 20..20 + n {
            assert_eq!(effect(&c).values_at(f)[0], 100.0, "image {f} : allumée");
        }
    }
    // Sur le calque courant (où rien d'autre ne l'éteint) : une clé « éteint » juste avant et juste après.
    let r = Reglages { duree: Some(3), sur_calque: true, ..Reglages::new(Genre::Impact, 24) };
    let c = build(&r, &ctx(20, 100)).unwrap();
    let e = effect(&c);
    assert_eq!((e.values_at(19)[0], e.values_at(20)[0], e.values_at(22)[0], e.values_at(23)[0]), (0.0, 100.0, 100.0, 0.0));
    assert_eq!(e.values_at(5)[0], 0.0);
    assert_eq!(e.values_at(60)[0], 0.0);
    assert!(e.key_at(19).is_some() && e.key_at(23).is_some());
}

#[test]
fn the_impact_options_write_their_values() {
    let r = Reglages { mode_impact: 1, teinte: true, couleur: [10, 20, 30], eclat: true, lignes: true, graine: 9, intensite: 1.0, ..Reglages::new(Genre::Impact, 24) };
    let v = effect(&build(&r, &ctx(0, 10)).unwrap()).values_at(0);
    assert_eq!((v[1], v[3], v[4], v[5]), (1.0, 10.0, 20.0, 30.0), "mode et teinte");
    assert!(v[6] > 0.0 && v[7] > 0.0 && v[8] > 0.0, "teinte, éclat et lignes allumés");
    assert_eq!((v[9], v[10], v[11]), (50.0, 50.0, 9.0), "centre du cadre et graine");
    let plain = effect(&build(&Reglages::new(Genre::Impact, 24), &ctx(0, 10)).unwrap()).values_at(0);
    assert!(plain[6] == 0.0 && plain[7] == 0.0 && plain[8] == 0.0, "sans option : ni teinte, ni éclat, ni lignes");
    // L'intensité renforce l'éclat sans dépasser 100.
    let strong = Reglages { intensite: 3.0, eclat: true, ..Reglages::new(Genre::Impact, 24) };
    assert_eq!(effect(&build(&strong, &ctx(0, 10)).unwrap()).values_at(0)[7], 100.0);
}

#[test]
fn the_shake_offsets_are_bounded_by_the_intensity_and_come_from_the_camera_generators() {
    let width = 1920.0;
    for style in [Preset::LightShake, Preset::Shoulder, Preset::Explosion, Preset::Jolt] {
        for intensite in [0.5f32, 1.0, 2.0] {
            let r = Reglages { style, intensite, frequence: style.default_freq(), duree: Some(24), graine: 5, ..Reglages::new(Genre::Tremblement, 24) };
            let c = build(&r, &ctx(10, 200)).unwrap();
            let e = effect(&c);
            assert_eq!(e.kind, EffectKind::Shake);
            // Les mêmes bornes que celles de l'effet de caméra de même intensité.
            let params = Params { preset: style, intensity: intensite, duration: 24, freq: style.default_freq(), seed: 5, zoom_factor: 1.0, punch: style == Preset::Explosion };
            let g = Generator::new(&params, 24);
            let b = g.bounds();
            assert!(!e.keys.is_empty());
            for k in &e.keys {
                let (dx, dy, angle, zoom) = (k.values[0], k.values[1], k.values[2], k.values[3]);
                assert!(dx.abs() <= b.pos * width + 1e-3 && dy.abs() <= b.pos * width + 1e-3, "{style:?} x{intensite} image {} : ({dx}, {dy}) > {}", k.frame, b.pos * width);
                assert!(angle.abs() <= b.angle + 1e-3, "{style:?} : rotation {angle} > {}", b.angle);
                assert!((zoom - 100.0).abs() <= b.punch * 100.0 + 1e-3, "{style:?} : zoom {zoom}");
            }
            // Et ce sont bien ces nombres-là : la clé de l'image 10 + rel est le décalage de la caméra au même instant.
            for rel in g.frames() {
                if let Some(i) = e.key_at(10 + rel) {
                    let s = g.at(rel);
                    assert_eq!(e.keys[i].values[..3], [s.dx * width, s.dy * width, s.angle]);
                }
            }
        }
    }
    // Le tremblement s'éteint : calme à la fin de sa durée (sur le calque courant, où il est posé hors de toute exposition).
    let r = Reglages { duree: Some(12), sur_calque: true, ..Reglages::new(Genre::Tremblement, 24) };
    let c = build(&r, &ctx(10, 200)).unwrap();
    let e = effect(&c);
    assert_eq!(e.values_at(22)[..3], [0.0, 0.0, 0.0], "calme après la dernière image");
    assert_eq!(e.values_at(9)[..3], [0.0, 0.0, 0.0], "et juste avant");
    assert_eq!(e.values_at(0)[..3], [0.0, 0.0, 0.0]);
}

#[test]
fn the_shake_blur_option_and_zero_intensity() {
    let with = build(&Reglages { flou_bouge: true, ..Reglages::new(Genre::Tremblement, 24) }, &ctx(0, 100)).unwrap();
    let without = build(&Reglages::new(Genre::Tremblement, 24), &ctx(0, 100)).unwrap();
    assert!(effect(&with).keys.iter().all(|k| k.values[4] > 0.0));
    assert!(effect(&without).keys.iter().all(|k| k.values[4] == 0.0));
    // Un tremblement à zéro ne bouge rien : l'effet n'est pas calculé.
    let silent = build(&Reglages { intensite: 0.0, ..Reglages::new(Genre::Tremblement, 24) }, &ctx(0, 100)).unwrap();
    assert!(crate::fx::steps_at(&silent.fx, 3).is_empty() || silent.fx[0].values_at(3)[..3].iter().all(|v| v.abs() < 0.01 * 1920.0));
}

#[test]
fn the_flash_rises_then_falls() {
    for genre in [Genre::FlashBlanc, Genre::FlashCouleur] {
        let r = Reglages { duree: Some(10), couleur: [255, 90, 40], ..Reglages::new(genre, 24) };
        let c = build(&r, &ctx(30, 100)).unwrap();
        let e = effect(&c);
        assert_eq!(e.kind, EffectKind::Flash);
        assert_eq!((c.debut, c.longueur), (30, 10));
        let amount = |f: usize| e.values_at(f)[0];
        let peak = (30..40).map(amount).fold(0.0f32, f32::max);
        assert_eq!(peak, 100.0, "le sommet est à pleine force");
        assert_eq!(amount(30), 0.0, "part de zéro");
        assert_eq!(amount(39), 0.0, "revient à zéro");
        assert!(amount(32) > amount(30) && amount(36) < amount(32).max(peak), "monte, puis redescend");
        let color = &e.values_at(32)[1..4];
        if genre == Genre::FlashBlanc {
            assert_eq!(color, [255.0, 255.0, 255.0]);
        } else {
            assert_eq!(color, [255.0, 90.0, 40.0]);
        }
    }
    // Un flash d'une seule image : au sommet tout de suite.
    let one = build(&Reglages { duree: Some(1), ..Reglages::new(Genre::FlashBlanc, 24) }, &ctx(5, 50)).unwrap();
    assert_eq!(effect(&one).values_at(5)[0], 100.0);
}

#[test]
fn the_glow_can_pulse() {
    let still = build(&Reglages::new(Genre::Lueur, 24), &ctx(4, 50)).unwrap();
    assert_eq!(effect(&still).kind, EffectKind::Bloom);
    assert_eq!(effect(&still).keys.len(), 1, "sans pulsation : une clé au départ");
    assert_eq!((still.debut, still.longueur), (4, 46), "jusqu'à la fin du plan");
    let pulsing = build(&Reglages { pulsation: true, periode: 1.0, duree: Some(48), ..Reglages::new(Genre::Lueur, 24) }, &ctx(0, 100)).unwrap();
    let e = effect(&pulsing);
    assert_eq!(e.keys.len(), 48, "une clé par image");
    let levels: Vec<f32> = e.keys.iter().map(|k| k.values[1]).collect();
    let (lo, hi) = (levels.iter().cloned().fold(f32::MAX, f32::min), levels.iter().cloned().fold(0.0, f32::max));
    assert!(lo >= 49.0 && hi <= 151.0 && hi - lo > 80.0, "autour de 100 %, ±50 % : {lo} à {hi}");
    // Période d'une seconde à 24 i/s : la pulsation revient à la même valeur 24 images plus tard.
    assert!((levels[3] - levels[27]).abs() < 1e-3);
}

#[test]
fn the_still_presets_use_the_camera_frame_and_the_image_size() {
    let c = Contexte { centre: [48.0, 52.0], taille: [60.0, 55.0], ..ctx(0, 50) };
    let v = build(&Reglages::new(Genre::Vignettage, 24), &c).unwrap();
    let values = effect(&v).values_at(0);
    assert_eq!(&values[3..7], [48.0, 52.0, 60.0, 55.0], "le cadre de la caméra sur la feuille");
    assert_eq!(effect(&v).kind, EffectKind::Vignette);
    // Un cadre deux fois plus petit : le grain et les pixels de l'aberration sont proportionnels.
    let big = build(&Reglages::new(Genre::Aberration, 24), &ctx(0, 50)).unwrap();
    let small = build(&Reglages::new(Genre::Aberration, 24), &Contexte { cadre: [960, 540], ..ctx(0, 50) }).unwrap();
    assert!((effect(&big).values_at(0)[0] - 2.0 * effect(&small).values_at(0)[0]).abs() < 1e-3);
    let grain = build(&Reglages { graine: 12, ..Reglages::new(Genre::Grain, 24) }, &ctx(3, 50)).unwrap();
    assert_eq!((effect(&grain).kind, effect(&grain).values_at(0)[2]), (EffectKind::Grain, 12.0));
    let blur = build(&Reglages { angle: 30.0, ..Reglages::new(Genre::FlouVitesse, 24) }, &ctx(0, 50)).unwrap();
    assert_eq!((effect(&blur).kind, effect(&blur).values_at(effect(&blur).keys[1].frame)[1]), (EffectKind::MotionBlur, 30.0));
}

#[test]
fn the_timecode_preset_makes_an_overlay_not_an_effect() {
    let c = made(Genre::Timecode, 0, 50);
    assert!(c.fx.is_empty());
    assert!(matches!(c.effet.contenu, Contenu::Incrustation(_)));
    assert_eq!((c.debut, c.longueur), (0, 50), "tout le plan");
    let mut r = Reglages::new(Genre::Timecode, 24);
    r.incrustation.source = crate::incrustation::Source::NomPlan;
    let named = build(&r, &ctx(0, 50)).unwrap();
    match named.effet.contenu {
        Contenu::Incrustation(i) => assert_eq!(i.source, crate::incrustation::Source::NomPlan),
        Contenu::Ajustement => panic!("une incrustation"),
    }
}

#[test]
fn nothing_goes_past_the_end_of_the_plan() {
    // Un impact de 3 images posé à l'avant-dernière image d'un plan de 10 : 2 images, coupé.
    let c = build(&Reglages { duree: Some(3), ..Reglages::new(Genre::Impact, 24) }, &ctx(8, 10)).unwrap();
    assert_eq!((c.debut, c.longueur, c.coupe), (8, 2, true));
    // Jusqu'à la fin du plan n'est pas « coupé ».
    let c = build(&Reglages::new(Genre::Lueur, 24), &ctx(8, 10)).unwrap();
    assert_eq!((c.longueur, c.coupe), (2, false));
    // Après la fin, ou à une image qui n'existe pas : rien.
    assert!(build(&Reglages::new(Genre::Impact, 24), &ctx(10, 10)).is_none());
    assert!(build(&Reglages::new(Genre::Impact, 24), &ctx(50, 10)).is_none());
    // Le tremblement ne pose aucune clé au-delà du plan.
    let c = build(&Reglages { duree: Some(48), ..Reglages::new(Genre::Tremblement, 24) }, &ctx(5, 20)).unwrap();
    assert!(effect(&c).keys.iter().all(|k| k.frame < 20));
    assert_eq!(c.longueur, 15);
}

#[test]
fn settings_are_brought_back_to_sane_values() {
    let r = Reglages { intensite: f32::NAN, duree: Some(0), graine: 99_999, mode_impact: 9, periode: -3.0, seuil: 900.0, frequence: 1e9, angle: f32::INFINITY, ..Reglages::new(Genre::Impact, 24) };
    let s = r.sanitized(24);
    assert_eq!((s.intensite, s.duree, s.graine, s.mode_impact), (1.0, Some(1), 999, 3));
    assert!(s.periode >= 0.1 && s.seuil <= 255.0 && s.frequence <= 12.0 && s.angle == 0.0);
    // Même absurdes, les réglages construisent sans planter.
    assert!(build(&r, &ctx(0, 10)).is_some());
}

#[test]
fn each_preset_has_a_name_a_note_and_an_action_that_leads_back_to_it() {
    for g in Genre::ALL {
        assert!(!g.label().is_empty() && !g.layer_name().is_empty() && g.note().len() > 20);
        assert_eq!(Genre::from_action(g.action()), Some(g));
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == g.action()), "{}", g.action());
    }
    assert_eq!(Genre::from_action("effet.inconnu"), None);
    // Les durées proposées : l'impact en images, les flashs en fraction de seconde, les autres jusqu'à la fin.
    assert_eq!(Genre::Impact.default_frames(24), Some(2));
    assert_eq!(Genre::FlashBlanc.default_frames(24), Some(8));
    assert_eq!(Genre::Timecode.default_frames(24), None);
}
