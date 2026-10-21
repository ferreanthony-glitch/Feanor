//! Tremblement du trait (`tremblement.rs`) : le champ de déplacement (déterministe, borné, lisse),
//! les variantes selon l'image, la vitesse et la cadence, l'effet de la pile (vectoriel seulement,
//! enregistré, clés), le cache des variantes, l'export, et la règle « sans latence ».

use std::time::Instant;

use eframe::egui::{pos2, Color32};

use crate::document::{Document, LayerKind};
use crate::fx::{self, Effect, EffectKind, FxStep, Shown};
use crate::fxexact_tests::{differing, random_raster};
use crate::raster::{Raster, TILE};
use crate::render::{self, PlanItem};
use crate::tremblement::{self as t, Field, Settings};

/// Les valeurs d'un tremblement : intensité, fréquence, variantes, vitesse, épaisseur, graine,
/// animer à l'arrêt, et la variante montrée.
fn values(intensity: f32, frequency: f32, variants: f32, speed: f32, thickness: f32, rest: bool, variant: usize) -> Vec<f32> {
    vec![intensity, frequency, variants, speed, thickness, 1.0, if rest { 1.0 } else { 0.0 }, variant as f32]
}

fn step(v: Vec<f32>) -> FxStep {
    FxStep { kind: EffectKind::Boil, values: v }
}

/// Une feuille `w` × `h` avec une ligne horizontale épaisse de `thick` pixels à la hauteur `y`.
fn line_sheet(w: usize, h: usize, y: usize, thick: usize) -> Raster {
    let mut r = Raster::new(w, h);
    for yy in y..y + thick {
        for x in 20..w - 20 {
            let i = r.tile_index_of(x, yy);
            r.tile_mut(i)[(yy % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(0, 0, 0, 255);
        }
    }
    r
}

fn flat_differences(a: &Raster, b: &Raster) -> usize {
    differing(a, b)
}

// ------------------------------------------------------------------ le champ

#[test]
fn the_same_settings_and_variant_always_give_the_same_pixels() {
    let sheet = line_sheet(300, 200, 90, 4);
    let v = values(6.0, 60.0, 3.0, 2.0, 40.0, false, 1);
    let a = t::boil(&sheet, &v, 0);
    let b = t::boil(&sheet, &v, 0);
    assert_eq!(flat_differences(&a, &b), 0, "deux calculs : les mêmes pixels");
    // Le champ lui-même, au même point.
    let f = Field::new(&Settings::of(&v));
    assert_eq!(f.at(123.4, 56.7), Field::new(&Settings::of(&v)).at(123.4, 56.7));
    // Une autre variante, une autre graine : un autre dessin.
    let other = t::boil(&sheet, &values(6.0, 60.0, 3.0, 2.0, 40.0, false, 2), 0);
    assert!(flat_differences(&a, &other) > 100, "les variantes diffèrent");
    let mut seeded = v.clone();
    seeded[t::SEED] = 2.0;
    assert!(flat_differences(&a, &t::boil(&sheet, &seeded, 0)) > 100, "la graine change le dessin");
}

#[test]
fn the_same_frame_always_asks_for_the_same_derived_drawing() {
    let mut e = Effect::new(EffectKind::Boil);
    e.set_param(t::SPEED, 3.0);
    let ids: Vec<u64> = (0..12).map(|f| fx::fx_id(77, &fx::steps_at(std::slice::from_ref(&e), f))).collect();
    let again: Vec<u64> = (0..12).map(|f| fx::fx_id(77, &fx::steps_at(std::slice::from_ref(&e), f))).collect();
    assert_eq!(ids, again, "lecture, export et défilement demandent les mêmes dessins");
    assert_eq!(ids[0], ids[2], "trois images pour la même variante");
    assert_ne!(ids[2], ids[3]);
}

#[test]
fn the_displacement_never_exceeds_the_intensity() {
    for intensity in [0.5f32, 3.0, 12.0, 20.0] {
        for frequency in [0.0f32, 25.0, 60.0, 100.0] {
            for variant in 0..3 {
                let s = Settings::of(&values(intensity, frequency, 3.0, 2.0, 0.0, false, variant));
                let f = Field::new(&s);
                let mut longest = 0.0f32;
                for k in 0..4000 {
                    let (x, y) = ((k * 37 % 2000) as f32 * 1.013 + 0.31, (k * 91 % 1500) as f32 * 0.987 + 0.17);
                    let [dx, dy, n] = f.at(x, y);
                    let len = dx.hypot(dy);
                    assert!(len <= intensity * 1.0001, "{len} > {intensity} (fréquence {frequency})");
                    assert!((-1.0..=1.0).contains(&n));
                    longest = longest.max(len);
                }
                assert!(longest > intensity * 0.4, "le champ s'étend : {longest} sur {intensity}");
            }
        }
    }
    // Sur un vrai dessin : un point ne s'éloigne pas de plus que l'intensité (plus un pixel de lissage).
    let mut sheet = Raster::new(200, 200);
    let i = sheet.tile_index_of(100, 100);
    sheet.tile_mut(i)[(100 % TILE) * TILE + 100 % TILE] = Color32::BLACK;
    for variant in 0..4 {
        let out = t::boil(&sheet, &values(5.0, 70.0, 4.0, 1.0, 0.0, false, variant), 0);
        for y in 0..200 {
            for x in 0..200 {
                if out.get(x, y).a() > 0 {
                    let d = ((x as f32 - 100.0).powi(2) + (y as f32 - 100.0).powi(2)).sqrt();
                    assert!(d <= 5.0 + 2.0, "le point est parti à {d} px");
                }
            }
        }
    }
}

#[test]
fn the_displacement_is_smooth_along_the_path_and_gets_jittery_with_the_frequency() {
    // Une longueur d'onde de L pixels : le déplacement ne varie pas plus vite que ~6 × a / (L/2) par
    // pixel (deux axes, interpolation lissée).
    let mean_step = |frequency: f32| {
        let s = Settings::of(&values(10.0, frequency, 3.0, 2.0, 0.0, false, 0));
        let f = Field::new(&s);
        let bound = 6.0 * s.intensity / f.cell;
        let (mut sum, mut n) = (0.0f32, 0);
        for k in 0..2000 {
            let (x, y) = (k as f32 * 0.5 + 3.0, 40.0 + (k as f32 * 0.07).sin() * 30.0);
            let (a, b) = (f.at(x, y), f.at(x + 0.5, y));
            let jump = (b[0] - a[0]).hypot(b[1] - a[1]);
            assert!(jump <= bound * 0.5 * 1.001, "saut de {jump} pour un demi-pixel, borne {}", bound * 0.5);
            sum += jump;
            n += 1;
        }
        sum / n as f32
    };
    let smooth = mean_step(0.0);
    let medium = mean_step(50.0);
    let jittery = mean_step(100.0);
    assert!(smooth < medium && medium < jittery, "plus la fréquence monte, plus c'est nerveux : {smooth} {medium} {jittery}");
    assert!(smooth < 0.2, "fréquence 0 : très doux ({smooth} px par demi-pixel)");
    // Un trait horizontal garde un contour continu : d'une colonne à la suivante, le haut du trait ne
    // bouge pas plus que la pente maximale du champ (3 × intensité / maille) ne le permet.
    let sheet = line_sheet(400, 120, 60, 3);
    let v = values(4.0, 70.0, 3.0, 2.0, 0.0, false, 1);
    let out = t::boil(&sheet, &v, 0);
    let slope = 3.0 * 4.0 / Field::new(&Settings::of(&v)).cell;
    let top = |x: usize| (0..120).find(|&y| out.get(x, y).a() > 128).map(|y| y as f32);
    let mut last: Option<f32> = None;
    for x in 30..370 {
        if let (Some(p), Some(q)) = (last, top(x)) {
            assert!((q - p).abs() <= slope + 1.5, "le trait se casse entre {} et {x} : {p} -> {q} (pente {slope})", x - 1);
        }
        last = top(x).or(last);
    }
}

#[test]
fn the_boiled_drawing_is_the_exact_field_applied_everywhere_the_tiles_are_skipped_only_when_empty() {
    // Le calcul par tuiles (réseau tiré une fois par tuile, tuiles sans encre sautées) rend la même
    // image que la lecture pixel par pixel du champ de référence.
    let mut sheet = line_sheet(300, 200, 90, 4);
    for k in 0..40 {
        let (x, y) = (210 + k % 7, 20 + k * 2);
        let i = sheet.tile_index_of(x, y);
        sheet.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(120, 30, 30, 200);
    }
    let v = values(7.0, 80.0, 3.0, 2.0, 0.0, false, 2);
    let got = t::boil(&sheet, &v, 0);
    let f = Field::new(&Settings::of(&v));
    let mut want = Raster::new(300, 200);
    for y in 0..200 {
        for x in 0..300 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let [dx, dy, _] = f.at(px, py);
            let c = crate::fxwarp::sample(&sheet, px + dx, py + dy);
            let a = c[3].round().clamp(0.0, 255.0);
            if a > 0.0 {
                let ch = |v: f32| v.round().clamp(0.0, a) as u8;
                let i = want.tile_index_of(x, y);
                want.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(ch(c[0]), ch(c[1]), ch(c[2]), a as u8);
            }
        }
    }
    assert_eq!(flat_differences(&got, &want), 0);
    // Une feuille vide reste vide, et sans effet le dessin est rendu tel quel.
    assert!(t::boil(&Raster::new(100, 100), &v, 0).tiles.iter().all(Option::is_none));
    assert_eq!(flat_differences(&t::boil(&sheet, &values(0.0, 50.0, 3.0, 2.0, 0.0, false, 0), 0), &sheet), 0);
}

#[test]
fn the_thickness_jitter_only_moves_the_soft_edges_and_stays_bounded() {
    // Un trait de 3 px aux bords adoucis : l'épaisseur varie sans jamais sortir de la couverture.
    let mut sheet = Raster::new(200, 100);
    for x in 10..190 {
        for (dy, a) in [(48usize, 90u8), (49, 255), (50, 255), (51, 255), (52, 90)] {
            let i = sheet.tile_index_of(x, dy);
            sheet.tile_mut(i)[(dy % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(0, 0, 0, a);
        }
    }
    let flat = values(0.0, 50.0, 3.0, 2.0, 100.0, false, 0);
    let out = t::boil(&sheet, &flat, 0);
    let (mut thicker, mut thinner) = (false, false);
    for x in 15..185 {
        for y in 0..100 {
            let (a, b) = (sheet.get(x, y).a(), out.get(x, y).a());
            if a == 255 {
                assert_eq!(b, 255, "le coeur du trait ne bouge pas");
            }
            if a == 0 {
                assert_eq!(b, 0, "pas d'encre là où il n'y en avait pas");
            }
            thicker |= b > a + 10;
            thinner |= b + 10 < a;
        }
    }
    assert!(thicker && thinner, "le trait s'épaissit par endroits et s'amincit ailleurs");
}

#[test]
fn a_reduced_preview_displaces_by_the_same_amount_on_the_screen() {
    // L'aperçu réduit de moitié lit le même champ (en pixels de la feuille) : le point garde sa place
    // à un pixel de l'aperçu près.
    let mut sheet = Raster::new(256, 256);
    for y in 120..136 {
        for x in 120..136 {
            let i = sheet.tile_index_of(x, y);
            sheet.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::BLACK;
        }
    }
    let v = values(10.0, 40.0, 3.0, 2.0, 0.0, false, 1);
    let centroid = |r: &Raster, k: f32| {
        let (mut sx, mut sy, mut n) = (0.0f32, 0.0f32, 0.0f32);
        for y in 0..r.height {
            for x in 0..r.width {
                let a = r.get(x, y).a() as f32;
                sx += (x as f32 + 0.5) * k * a;
                sy += (y as f32 + 0.5) * k * a;
                n += a;
            }
        }
        (sx / n, sy / n)
    };
    let full = t::boil(&sheet, &v, 0);
    let half = t::boil(&crate::fxproxy::downscale(&sheet, 1), &v, 1);
    assert_eq!((half.width, half.height), (128, 128));
    let (a, b) = (centroid(&full, 1.0), centroid(&half, 2.0));
    assert!((a.0 - b.0).abs() < 2.5 && (a.1 - b.1).abs() < 2.5, "{a:?} contre {b:?}");
}

// ------------------------------------------------------------------ variantes, vitesse, cadence

#[test]
fn the_variant_cycles_with_the_frame_the_speed_and_the_variant_count() {
    assert_eq!((0..8).map(|f| t::variant_at(f, 2, 3)).collect::<Vec<_>>(), vec![0, 0, 1, 1, 2, 2, 0, 0]);
    assert_eq!((0..6).map(|f| t::variant_at(f, 1, 2)).collect::<Vec<_>>(), vec![0, 1, 0, 1, 0, 1]);
    assert_eq!((0..9).map(|f| t::variant_at(f, 3, 6)).collect::<Vec<_>>(), vec![0, 0, 0, 1, 1, 1, 2, 2, 2]);
    // Bornes : une vitesse ou un nombre de variantes nuls ne plantent pas.
    assert_eq!(t::variant_at(7, 0, 0), 0);
    // Des réglages hors bornes sont ramenés (2 à 6 variantes, 1 à 12 images).
    let s = Settings::of(&[50.0, 150.0, 99.0, 0.0, -3.0, 0.0, 0.7, 41.0]);
    assert_eq!((s.intensity, s.frequency, s.variants, s.speed, s.thickness, s.seed, s.rest), (20.0, 100.0, 6, 1, 0.0, 1, true));
    assert!(s.variant < 6);
}

#[test]
fn at_24_fps_every_two_frames_makes_twelve_changes_per_second() {
    let e = Effect::new(EffectKind::Boil);
    assert_eq!(e.values_at(0)[t::SPEED], 2.0, "par défaut : une variante toutes les deux images");
    let variants: Vec<usize> = (0..24).map(|f| Settings::of(&fx::steps_at(std::slice::from_ref(&e), f)[0].values).variant).collect();
    let changes = variants.windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(changes, 11, "12 variantes en une seconde : 11 changements dedans, le 12e au bouclage");
    let two_seconds = (0..48).map(|f| Settings::of(&fx::steps_at(std::slice::from_ref(&e), f)[0].values).variant).collect::<Vec<_>>();
    assert_eq!(two_seconds.windows(2).filter(|w| w[0] != w[1]).count(), 23, "12 changements par seconde");
    // Le même effet à d'autres vitesses.
    for speed in [1usize, 2, 3, 4, 6, 12] {
        let mut e = Effect::new(EffectKind::Boil);
        e.set_param(t::SPEED, speed as f32);
        e.set_param(t::VARIANTS, 4.0);
        let seq: Vec<usize> = (0..48).map(|f| Settings::of(&fx::steps_at(std::slice::from_ref(&e), f)[0].values).variant).collect();
        let runs = seq.windows(2).filter(|w| w[0] != w[1]).count() + 1;
        assert_eq!(runs, 48usize.div_ceil(speed), "vitesse {speed}");
        assert!(seq.chunks(speed).all(|c| c.iter().all(|&v| v == c[0])), "la variante tient {speed} images");
    }
}

#[test]
fn keys_animate_the_intensity_and_the_variant_follows_the_frame() {
    let mut e = Effect::new(EffectKind::Boil);
    e.set_key(0, values(0.0, 55.0, 3.0, 2.0, 0.0, false, 0)[..7].to_vec());
    e.set_key(10, values(10.0, 55.0, 3.0, 2.0, 0.0, false, 0)[..7].to_vec());
    assert!(fx::steps_at(std::slice::from_ref(&e), 0).is_empty(), "intensité nulle à la première clé : rien à faire");
    let mid = &fx::steps_at(std::slice::from_ref(&e), 5)[0].values;
    assert!((mid[t::INTENSITY] - 5.0).abs() < 1e-4, "entre deux clés : en ligne droite");
    assert_eq!(Settings::of(mid).variant, 2 % 3);
    assert_eq!(fx::steps_at(std::slice::from_ref(&e), 99)[0].values[t::INTENSITY], 10.0);
    // Un réglage qui ne s'anime pas se pose dans toutes les clés.
    e.set_param(t::REST, 1.0);
    assert!(e.keys.iter().all(|k| k.values[t::REST] == 1.0) && e.values[t::REST] == 1.0);
}

// ------------------------------------------------------------------ vectoriel seulement

#[test]
fn the_effect_only_applies_to_vector_layers() {
    let mut vector = crate::document::Layer::new("V", LayerKind::Vector);
    let mut bitmap = crate::document::Layer::new("B", LayerKind::Bitmap);
    vector.fx.push(Effect::new(EffectKind::Boil));
    bitmap.fx.push(Effect::new(EffectKind::Boil));
    bitmap.fx.push(Effect::new(EffectKind::GaussianBlur));
    let on_vector = fx::layer_steps(&vector, 0, None);
    assert_eq!(on_vector.len(), 1);
    assert_eq!(on_vector[0].values.len(), 8, "les 7 réglages, puis la variante");
    let on_bitmap = fx::layer_steps(&bitmap, 0, None);
    assert_eq!(on_bitmap.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![EffectKind::GaussianBlur], "le flou reste, pas le tremblement");
    assert!(EffectKind::Boil.vector_only() && !EffectKind::GaussianBlur.vector_only());
    assert_eq!(EffectKind::Boil.family(), "Trait", "le tremblement est de la famille du trait (menu « Trait »)");
    assert_eq!(fx::without_boil(on_vector).len(), 0);
}

// ------------------------------------------------------------------ enregistrement

#[test]
fn the_settings_are_saved_with_the_layer_in_a_format_from_27_on() {
    // (26 pour le tremblement du trait ; 27 depuis les calques d'effets, qui ne le changent pas.)
    assert!(crate::project::VERSION >= 27);
    let mut doc = Document::new(64, 64, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = LayerKind::Vector;
    layer.frames = vec![Some(id)];
    let mut boil = Effect::new(EffectKind::Boil);
    boil.values = vec![7.5, 20.0, 5.0, 3.0, 35.0, 42.0, 1.0];
    boil.set_key(4, vec![2.0, 80.0, 4.0, 1.0, 0.0, 42.0, 1.0]);
    layer.fx = vec![boil];
    let dir = std::env::temp_dir().join(format!("shot-test-tremblement-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"tremblement\""), "{text}");
    assert!(text.contains(&format!("\"version\": {}", crate::project::VERSION)), "un fichier d'avant refuse celui-ci");
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].fx, doc.panels[0].layers[0].fx, "mêmes réglages et mêmes clés");
    // Une version d'après est refusée, comme toujours.
    let newer = text.replace(&format!("\"version\": {}", crate::project::VERSION), &format!("\"version\": {}", crate::project::VERSION + 1));
    std::fs::write(&manifest, newer).unwrap();
    assert!(crate::project::load(&manifest).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------ cache et affichage

/// Un document avec un dessin vectoriel (un trait de 4 px) de numéro `id`, et l'élément du plan qui
/// le passe par `steps`.
fn doc_and_item(id: u64, steps: Vec<FxStep>) -> (Document, PlanItem) {
    let mut doc = Document::new(320, 256, 24);
    let mut cel = crate::document::Cel::bitmap(random_raster(320, 256, id as u32));
    cel.version = 1;
    doc.cels.insert(id, cel);
    let item = PlanItem { cel: id, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: steps, ctg: None, fond: None, pre: None };
    (doc, item)
}

#[test]
fn every_setting_has_its_own_derived_drawing() {
    crate::fxcache::monde::ecrire();
    let base = values(4.0, 50.0, 3.0, 2.0, 10.0, false, 1);
    let id = |v: &[f32]| fx::fx_id(55, &[step(v.to_vec())]);
    let reference = id(&base);
    for (k, other) in [(t::INTENSITY, 5.0), (t::FREQUENCY, 51.0), (t::VARIANTS, 4.0), (t::SPEED, 3.0), (t::THICKNESS, 11.0), (t::SEED, 2.0), (t::VARIANT, 2.0)] {
        let mut v = base.clone();
        v[k] = other;
        assert_ne!(id(&v), reference, "le réglage {k} change le dessin dérivé");
    }
    assert_eq!(id(&base.clone()), reference, "les mêmes réglages : le même dessin");
    assert_ne!(fx::fx_id(56, &[step(base)]), reference, "un autre dessin : un autre numéro");
}

#[test]
fn changing_a_setting_recomputes_and_the_previous_variant_stays_on_screen_meanwhile() {
    crate::fxcache::monde::ecrire();
    let a = step(values(4.0, 50.0, 3.0, 2.0, 0.0, false, 0));
    let (doc, mut item) = doc_and_item(900_000_301, vec![a]);
    assert!(!fx::display(&doc, &item, 0).unwrap().ready, "calculé en arrière-plan");
    crate::fxcache::settle();
    let first = fx::display(&doc, &item, 0).unwrap();
    assert!(first.ready && first.id == fx::shown_id(&item));
    let first_id = first.id;
    let Shown::Fx(first_cel) = first.cel else { panic!("dessin dérivé") };
    let exact = fx::apply(&doc.cels[&item.cel].raster, &item.fx);
    assert_eq!(differing(&first_cel.raster, &exact), 0, "le même dessin que le calcul direct");
    // L'intensité change : le nouvel état n'est pas prêt, l'écran garde la variante d'avant.
    item.fx = vec![step(values(9.0, 50.0, 3.0, 2.0, 0.0, false, 0))];
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(!d.ready && d.id == first_id, "la variante d'avant en attendant");
    crate::fxcache::settle();
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(d.ready && d.id == fx::shown_id(&item) && d.id != first_id);
    // Revenir au premier réglage : déjà gardé.
    item.fx = vec![step(values(4.0, 50.0, 3.0, 2.0, 0.0, false, 0))];
    assert!(fx::display(&doc, &item, 0).unwrap().ready, "gardé sous son propre numéro");
}

#[test]
fn a_retouched_drawing_never_shows_the_boiled_version_from_before_the_retouch() {
    crate::fxcache::monde::ecrire();
    let (mut doc, item) = doc_and_item(900_000_302, vec![step(values(5.0, 50.0, 3.0, 2.0, 0.0, false, 0))]);
    fx::display(&doc, &item, 0);
    crate::fxcache::settle();
    assert!(fx::display(&doc, &item, 0).unwrap().ready);
    // Le trait qu'on vient de poser : la version du dessin change.
    let c = doc.cels.get_mut(&item.cel).unwrap();
    c.raster.tile_mut(0)[5] = Color32::BLACK;
    c.version += 1;
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(!d.ready);
    assert!(matches!(d.cel, Shown::Plain(_)) && d.id == item.cel, "le dessin tel quel, pas le tremblement d'avant qui n'a pas le nouveau trait");
    crate::fxcache::settle();
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(d.ready && matches!(d.cel, Shown::Fx(_)));
    // Les autres effets gardent leur comportement d'avant (le dernier résultat reste affiché).
    let (mut doc, blur) = doc_and_item(900_000_303, vec![FxStep { kind: EffectKind::GaussianBlur, values: vec![5.37] }]);
    fx::display(&doc, &blur, 0);
    crate::fxcache::settle();
    let c = doc.cels.get_mut(&blur.cel).unwrap();
    c.version += 1;
    assert!(matches!(fx::display(&doc, &blur, 0).unwrap().cel, Shown::Fx(_)));
}

/// Un document de `frames` images avec un calque vectoriel qui tremble et un dessin de lignes.
fn boiling_doc(frames: usize) -> Document {
    let mut doc = Document::new(256, 128, 24);
    let id = doc.new_cel(LayerKind::Vector);
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        for y in 30..100 {
            if y % 14 < 4 {
                for x in 20..236 {
                    let i = cel.raster.tile_index_of(x, y);
                    cel.raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(0, 0, 0, 255);
                }
            }
        }
        cel.version = 1;
    }
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = LayerKind::Vector;
    layer.frames = vec![Some(id); frames];
    let mut e = Effect::new(EffectKind::Boil);
    e.values[t::INTENSITY] = 6.0;
    layer.fx = vec![e];
    doc
}

#[test]
fn the_export_and_the_thumbnails_show_the_variant_of_each_frame() {
    crate::fxcache::monde::ecrire();
    let doc = boiling_doc(8);
    let image = |frame: usize| render::composite_full(&doc, &render::panel_plan_at(&doc, 0, frame, None));
    let (f0, f1, f2, f6) = (image(0), image(1), image(2), image(6));
    assert_eq!(f0, f1, "deux images pour la même variante");
    assert_ne!(f0, f2, "la variante suivante");
    assert_eq!(f0, f6, "trois variantes : on revient au bout de six images");
    // Le dessin lui-même n'a pas bougé.
    let plain = doc.cels[&doc.panels[0].layers[0].frames[0].unwrap()].raster.to_flat();
    assert_ne!(f0, plain);
    // Une vignette (calcul doux) montre la même variante, une fois le calcul fini.
    let plan = render::panel_plan_at(&doc, 0, 2, None);
    let want = render::composite_pixel(&doc, &plan, 100, 32);
    crate::fxcache::settle();
    let (got, _) = crate::fxcache::soft_scope(|| render::composite_pixel(&doc, &plan, 100, 32));
    assert_eq!(got, want);
}

// ------------------------------------------------------------------ à l'arrêt

#[test]
fn at_rest_only_the_layers_that_animate_follow_the_clock() {
    let still = values(4.0, 50.0, 3.0, 2.0, 0.0, false, 0);
    let animated = values(4.0, 50.0, 3.0, 2.0, 0.0, true, 0);
    // L'image virtuelle du repos : l'horloge en images du projet.
    assert_eq!(t::rest_frame(1000, 24), 24);
    assert_eq!(t::rest_frame(500, 25), 12);
    let rest = Some(t::rest_frame(1000, 24));
    assert_eq!(t::variant_for(&still, 5, rest), t::variant_at(5, 2, 3), "éteint : la variante de l'image courante");
    assert_eq!(t::variant_for(&animated, 5, rest), t::variant_at(24, 2, 3), "allumé : celle de l'horloge");
    assert_eq!(t::variant_for(&animated, 5, None), t::variant_at(5, 2, 3), "en lecture ou à l'export : toujours l'image");
    // Le plan de l'écran : un élément qui anime à l'arrêt réveille l'écran, pas les autres.
    let item = |v: Vec<f32>| PlanItem { cel: 1, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: vec![step(v)], ctg: None, fond: None, pre: None };
    assert_eq!(t::rest_wake(&[item(still.clone())], 24, 1234), None, "image fixe : l'écran ne se redessine pas");
    assert_eq!(t::rest_wake(&[], 24, 1234), None);
    let wake = t::rest_wake(&[item(animated.clone())], 24, 1234).expect("réveil");
    assert!(wake.as_millis() > 0 && wake.as_millis() <= 84, "au plus une période de 83 ms (12 Hz) : {wake:?}");
    // Après ce temps, la variante a changé ; avant, non.
    let now = 1234u64;
    let at = |ms: u64| t::variant_for(&animated, 0, Some(t::rest_frame(ms, 24)));
    assert_eq!(at(now), at(now + wake.as_millis() as u64 - 2));
    assert_ne!(at(now), at(now + wake.as_millis() as u64 + 1));
    // Plus lent : plus rare.
    let mut slow = animated.clone();
    slow[t::SPEED] = 6.0;
    let w = t::rest_wake(&[item(slow)], 24, 1234).unwrap();
    assert!(w.as_millis() <= 250, "{w:?}");
    // Le plus pressé gagne.
    let both = t::rest_wake(&[item(animated.clone()), item(values(4.0, 50.0, 3.0, 12.0, 0.0, true, 0))], 24, 1234).unwrap();
    assert!(both <= wake);
    // À 60 Hz d'écran il n'y a rien à dire : aucune période n'est plus courte qu'une image du projet.
    assert!(t::rest_wake(&[item(values(4.0, 50.0, 3.0, 1.0, 0.0, true, 0))], 24, 5).unwrap().as_millis() <= 42);
}

#[test]
fn the_other_variants_are_prefetched_only_for_layers_that_animate_at_rest() {
    let item = |v: Vec<f32>| PlanItem { cel: 1, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: vec![step(v)], ctg: None, fond: None, pre: None };
    assert!(t::other_variants(&[item(values(4.0, 50.0, 3.0, 2.0, 0.0, false, 1))]).is_empty());
    let others = t::other_variants(&[item(values(4.0, 50.0, 4.0, 2.0, 0.0, true, 1))]);
    let ids: Vec<u64> = others.iter().map(fx::shown_id).collect();
    assert_eq!(others.len(), 3, "les trois autres variantes de quatre");
    assert!(ids.iter().all(|i| *i != fx::shown_id(&item(values(4.0, 50.0, 4.0, 2.0, 0.0, true, 1)))));
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 3, "chacune son dessin");
}

// ------------------------------------------------------------------ sans latence

/// Un trait vectoriel texturé (graphite, feutre ou fusain, fin) sur une feuille `w` × `h`.
fn fine_strokes(n: usize, w: usize, h: usize) -> Vec<crate::vector::Element> {
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    (0..n)
        .map(|k| {
            let name = ["Feutre", "Fusain", "Gouache"][k % 3];
            let mut s = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
            s.size = 5.0 + (k % 5) as f32 * 2.0;
            let (x0, y0) = (80.0 + (k * 97 % (w - 400)) as f32, 60.0 + (k * 53 % (h - 200)) as f32);
            let pts: Vec<_> = (0..=60)
                .map(|i| {
                    let u = i as f32 / 60.0;
                    (pos2(x0 + 300.0 * u, y0 + 90.0 * (u * 5.0 + k as f32).sin()), 0.3 + 0.7 * (u * std::f32::consts::PI).sin(), i as f64 / 200.0)
                })
                .collect();
            crate::affichage_tests::trait_rouge(&s, &lib, w, h, &pts)
        })
        .collect()
}

/// Lecture d'un plan dont le calque vectoriel (traits texturés) tremble : l'interface ne rejoue
/// jamais le moteur, et chaque image est servie sans attendre (la variante prête, ou le dessin).
fn play_boiling_plan(strokes: usize, w: usize, h: usize, frames: usize, real_time: bool) -> (Vec<f64>, usize, usize) {
    let (mut doc, id) = crate::affichage_tests::doc_with(w, h, fine_strokes(strokes, w, h));
    doc.fps = 24;
    {
        let layer = &mut doc.panels[0].layers[0];
        layer.kind = LayerKind::Vector;
        layer.frames = vec![Some(id); frames];
        let mut e = Effect::new(EffectKind::Boil);
        e.values[t::INTENSITY] = 4.0;
        layer.fx = vec![e];
    }
    doc.cels.get_mut(&id).unwrap().version = 1;
    let replays_before = crate::vector::replay_count();
    let mut ui_ms = Vec::new();
    let (mut waited, mut shown_ready) = (0, 0);
    let started = Instant::now();
    for frame in 0..frames {
        let t0 = Instant::now();
        // Ce que fait l'écran à chaque image de la lecture : le plan, ce qu'il montre, ce qui vient.
        let plan = render::panel_plan_at(&doc, 0, frame, None);
        for item in &plan.items {
            let d = fx::display(&doc, item, 0).expect("dessin");
            if d.ready {
                shown_ready += 1;
            } else {
                waited += 1;
            }
        }
        let at = crate::playback::Position { panel: 0, frame, playing: true, montage: false, looping: true, range: (0, frames) };
        let _ = crate::playback::upcoming_derived(&doc, at, 0, 8);
        ui_ms.push(t0.elapsed().as_secs_f64() * 1e3);
        if real_time {
            let due = started + std::time::Duration::from_secs_f64((frame + 1) as f64 / 24.0);
            if let Some(rest) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(rest);
            }
        } else {
            crate::fxcache::settle();
        }
    }
    assert_eq!(crate::vector::replay_count(), replays_before, "le moteur n'est jamais rejoué sur le fil de l'interface");
    (ui_ms, waited, shown_ready)
}

#[test]
fn playing_a_boiling_vector_plan_never_replays_the_engine_on_the_interface_thread() {
    crate::fxcache::monde::ecrire();
    let (ui, _waited, ready) = play_boiling_plan(6, 640, 360, 24, false);
    assert_eq!(ui.len(), 24);
    assert!(ready > 0, "des variantes ont été servies prêtes");
    // Une fois les trois variantes calculées, un tour de plus ne demande plus rien.
    crate::fxcache::settle();
}

#[test]
fn the_boil_is_offered_in_every_menu_on_vector_layers_and_shown_disabled_on_bitmap_ones() {
    use crate::fxui::{offer, Offer, BOIL_VECTOR_ONLY};
    // Calque vectoriel : l'entrée est utilisable ; calque bitmap : grisée, avec la raison ; dossier : absente.
    assert_eq!(offer(EffectKind::Boil, LayerKind::Vector, false), Offer::Enabled);
    assert_eq!(offer(EffectKind::Boil, LayerKind::Bitmap, false), Offer::Disabled(BOIL_VECTOR_ONLY));
    assert_eq!(offer(EffectKind::Boil, LayerKind::Vector, true), Offer::Absent);
    assert_eq!(offer(EffectKind::Boil, LayerKind::Bitmap, true), Offer::Absent);
    // La raison dit que c'est pour les calques vectoriels.
    assert!(BOIL_VECTOR_ONLY.contains("calque vectoriel"));
    // Les autres effets vont sur les deux sortes de calques, et sont absents d'un dossier.
    for kind in EffectKind::ALL.into_iter().filter(|k| !k.vector_only()) {
        assert_eq!(offer(kind, LayerKind::Bitmap, false), Offer::Enabled, "{kind:?}");
        assert_eq!(offer(kind, LayerKind::Vector, false), Offer::Enabled, "{kind:?}");
        assert_eq!(offer(kind, LayerKind::Vector, true), Offer::Absent, "{kind:?}");
    }
    // Le tremblement et les effets du trait (`fxtrait.rs`) sont les seuls réservés au vectoriel.
    let only: Vec<_> = EffectKind::ALL.into_iter().filter(|k| k.vector_only()).collect();
    assert!(only.contains(&EffectKind::Boil) && only.iter().all(|k| *k == EffectKind::Boil || k.is_trait()));
}

#[test]
fn the_command_palette_finds_the_boil_by_its_name() {
    use crate::keymap::ACTIONS;
    use crate::palette::rank;
    let commands: Vec<(&str, &str)> = ACTIONS.iter().filter(|a| a.implemented).map(|a| (a.id, a.label)).collect();
    for word in ["tremblement", "tremblement du trait", "trembl", "boil"] {
        let hits = rank(&commands, word);
        let ids: Vec<&str> = hits.iter().map(|&i| commands[i].0).collect();
        assert!(ids.contains(&"fx.tremblement"), "« {word} » ne trouve pas le tremblement du trait : {ids:?}");
    }
    // Les commandes du tremblement du trait passent devant les tremblements de caméra.
    let hits = rank(&commands, "tremblement");
    assert_eq!(commands[hits[0]].0, "fx.tremblement");
}

/// Mesure : la lecture à 24 ips d'un plan de 60 traits texturés qui tremblent (1920 × 1080, trois
/// variantes, une toutes les deux images) ; temps de l'interface par image, images servies sans
/// attendre, coût d'une variante en arrière-plan (pleine résolution et aperçu), mémoire.
/// `cargo test --release --offline --target-dir target\msvc boiling_playback_cost -- --ignored --nocapture`
#[test]
#[ignore]
fn boiling_playback_cost() {
    let (w, h) = (1920, 1080);
    let t0 = Instant::now();
    let (ui, waited, ready) = play_boiling_plan(60, w, h, 96, true);
    let mean = ui.iter().sum::<f64>() / ui.len() as f64;
    let max = ui.iter().cloned().fold(0.0, f64::max);
    let mut sorted = ui.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "lecture de 96 images à 24 ips (60 traits texturés, 1920 × 1080, 3 variantes) en {:.1} s : temps de l'interface par image {mean:.3} ms en moyenne, {:.3} ms au 99e centile, {max:.3} ms au plus ; {ready} images servies prêtes, {waited} en attente de la variante (le dessin ou la variante d'avant) ; rejeux du moteur sur le fil de l'interface : 0",
        t0.elapsed().as_secs_f64(),
        sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)],
    );
    // Le coût d'une variante, en arrière-plan.
    let (doc, id) = crate::affichage_tests::doc_with(w, h, fine_strokes(60, w, h));
    let raster = &doc.cels[&id].raster;
    for level in [0u8, 1] {
        let src = if level == 0 { raster.clone() } else { crate::fxproxy::downscale(raster, level) };
        let v = values(4.0, 55.0, 3.0, 2.0, 0.0, false, 1);
        let t1 = Instant::now();
        let out = crate::fx::apply_scaled(&src, &[step(v)], level);
        let bytes = out.tiles.iter().flatten().count() * 64 * 64 * 4;
        println!("une variante, niveau {level} ({}×{}) : {:.1} ms, {:.1} Mo ; dessin d'origine {:.1} Mo", src.width, src.height, t1.elapsed().as_secs_f64() * 1e3, bytes as f64 / 1e6, src.tiles.iter().flatten().count() as f64 * 16384.0 / 1e6);
    }
    // Le pire cas : toute la feuille couverte d'encre (aucune tuile sautée).
    let full = random_raster(w, h, 7);
    let t3 = Instant::now();
    let _ = t::boil(&full, &values(4.0, 55.0, 3.0, 2.0, 0.0, false, 1), 0);
    println!("une variante, feuille entièrement couverte ({} tuiles) : {:.1} ms", full.tiles.iter().flatten().count(), t3.elapsed().as_secs_f64() * 1e3);
    // Avec l'épaisseur variable.
    let t2 = Instant::now();
    let _ = t::boil(raster, &values(4.0, 55.0, 3.0, 2.0, 60.0, false, 1), 0);
    println!("une variante avec épaisseur variable : {:.1} ms", t2.elapsed().as_secs_f64() * 1e3);
}
