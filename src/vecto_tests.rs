//! Rendu des brosses, étape 5 (`docs/VECTORIEL-COMME-BITMAP.md`) : le trait vectoriel garde sa
//! trace et rejoue le moteur bitmap trait pour trait ; ses pixels sont gardés en cache et suivent
//! les transformations sans rejeu ; les anciens traits se relisent et rendent comme avant.

use std::time::Instant;

use eframe::egui::{pos2, Pos2, Rect};

use crate::brush::{BrushSettings, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushLibrary;
use crate::document::{Cel, LayerKind};
use crate::raster::Raster;
use crate::vector::{self, Affine, Element, Trace, VPoint, VectorDrawing};

/// Un geste en S avec l'appui en cloche, `n` échantillons, décalé de `dx`, `dy`.
fn geste(n: usize, dx: f32, dy: f32) -> Vec<(Pos2, f32, f64)> {
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let p = pos2(30.0 + dx + t * 300.0, 90.0 + dy + (t * std::f32::consts::TAU).sin() * 40.0);
            (p, 0.1 + 0.85 * (t * std::f32::consts::PI).sin(), k as f64 / 200.0)
        })
        .collect()
}

/// Le trait tracé en bitmap par le moteur (avec le toucher et la couleur B de l'application), et
/// l'élément vectoriel qu'un lâcher garderait (`commit_vector_stroke`).
fn bitmap_and_element(s: &BrushSettings, lib: &BrushLibrary, w: usize, h: usize, pts: &[(Pos2, f32, f64)]) -> (Raster, Element) {
    let mut raster = Raster::new(w, h);
    let mut st = StrokeState::new(0, StrokeMode::Paint([20, 20, 20]), *s, None, true).with_textures(lib.textures(s)).with_feel(Toucher::default()).with_secondary([200, 40, 40]);
    let mut dirty = Vec::new();
    for &(p, pr, t) in pts {
        st.add_point_at(&mut raster, p, pr, t, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    let trace = Trace { pressure: st.path_pressure.clone(), time: st.path_time.clone(), feel: Some(Toucher::default()), secondary: Some([200, 40, 40]), lien: None, cache: Default::default() };
    let e = Element::Stroke { points: st.path.clone(), color: [20, 20, 20], opacity: s.opacity, hardness: s.hardness, brush: lib.for_vector(s).map(Box::new), trace };
    (raster, e)
}

fn same_pixels(a: &Raster, b: &Raster) -> Result<(), String> {
    near_pixels(a, b, 0)
}

/// Les deux images sont les mêmes, à `tol` niveaux près sur chaque composante (les arrondis
/// flottants d'une position calculée autrement).
fn near_pixels(a: &Raster, b: &Raster, tol: i32) -> Result<(), String> {
    let (mut n, mut worst) = (0usize, 0i32);
    for y in 0..a.height {
        for x in 0..a.width {
            let (p, q) = (a.get(x, y).to_array(), b.get(x, y).to_array());
            let d = (0..4).map(|k| (p[k] as i32 - q[k] as i32).abs()).max().unwrap_or(0);
            if d > tol {
                n += 1;
                worst = worst.max(d);
            }
        }
    }
    if n == 0 { Ok(()) } else { Err(format!("{n} pixels différents, écart jusqu'à {worst}")) }
}

#[test]
fn a_vector_stroke_renders_exactly_like_its_bitmap_preview() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let pts = geste(160, 0.0, 0.0);
    // Toutes les brosses livrées : graphite (pointe, papier, grain du crayon, accumulation), encre
    // (connexion de taille à courbe), gouache (humide, poils), feutre (pointe aplatie), pinceau
    // rond ; et une brosse à deux couleurs.
    let mut deux = BrushSettings::brush();
    deux.dynamics.color = crate::brush::Connection { source: crate::brush::Source::Pressure, ..Default::default() };
    for (nom, s) in [
        ("graphite", BrushSettings::graphite()),
        ("encre", BrushSettings::ink()),
        ("gouache", BrushSettings::gouache()),
        ("feutre", BrushSettings::felt()),
        ("fusain", BrushSettings::charcoal()),
        ("pinceau rond", BrushSettings::brush()),
        ("deux couleurs", deux),
    ] {
        let (bitmap, e) = bitmap_and_element(&s, &lib, 400, 200, &pts);
        let mut vecto = Raster::new(400, 200);
        vector::render_element(&mut vecto, &e);
        same_pixels(&bitmap, &vecto).unwrap_or_else(|err| panic!("{nom} : {err}"));
        // Le second rendu vient du cache : les mêmes pixels, sans rejouer.
        assert!(e.cache_bytes() > 0, "{nom} : pixels gardés");
        let mut again = Raster::new(400, 200);
        vector::render_element(&mut again, &e);
        same_pixels(&vecto, &again).unwrap_or_else(|err| panic!("{nom}, depuis le cache : {err}"));
    }
}

#[test]
fn a_moved_stroke_is_its_shifted_pixels_until_released() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let (_, e) = bitmap_and_element(&BrushSettings::graphite(), &lib, 400, 200, &geste(120, 0.0, 0.0));
    let mut first = Raster::new(400, 200);
    vector::render_element(&mut first, &e);
    // Déplacé d'un nombre entier de pixels : les pixels copiés, sans rejeu (le grain du papier
    // suit le trait pendant le geste).
    let moved = e.transformed_preview(&Affine::translate(eframe::egui::vec2(7.0, -3.0)));
    let mut shifted = Raster::new(400, 200);
    vector::render_element(&mut shifted, &moved);
    for y in 10..190 {
        for x in 10..390 {
            assert_eq!(shifted.get(x, y), first.get((x as i32 - 7) as usize, (y as i32 + 3) as usize), "({x}, {y})");
        }
    }
    // Tourné : rééchantillonné depuis le cache, à peu près la même quantité d'encre.
    let turned = e.transformed_preview(&Affine::rotate_about(pos2(180.0, 90.0), 0.3));
    let mut rot = Raster::new(400, 200);
    vector::render_element(&mut rot, &turned);
    let ink = |r: &Raster| (0..200).flat_map(|y| (0..400).map(move |x| (x, y))).map(|(x, y)| r.get(x, y).a() as f64).sum::<f64>();
    assert!((ink(&rot) / ink(&first) - 1.0).abs() < 0.15, "{} contre {}", ink(&rot), ink(&first));
    // Au lâcher, le cache est refait : un rendu exact, rejoué sur la nouvelle place (le papier est
    // collé à la feuille, le grain change donc un peu : pas les mêmes pixels, la même encre).
    let mut released = moved.clone();
    released.invalidate_cache();
    let mut exact = Raster::new(400, 200);
    vector::render_element(&mut exact, &released);
    assert!(same_pixels(&shifted, &exact).is_err(), "le grain collé à la feuille a changé sous le trait");
    assert!((ink(&exact) / ink(&shifted) - 1.0).abs() < 0.1);
    // Et la trace est intacte : rejouer donne ce que l'aperçu aurait donné là (à l'arrondi
    // flottant près : les positions décalées ne s'additionnent pas dans le même ordre).
    let (bitmap, _) = bitmap_and_element(&BrushSettings::graphite(), &lib, 400, 200, &geste(120, 7.0, -3.0));
    near_pixels(&bitmap, &exact, 2).unwrap();
}

#[test]
fn a_recoloured_stroke_forgets_its_cache() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let (_, e) = bitmap_and_element(&BrushSettings::felt(), &lib, 400, 200, &geste(100, 0.0, 0.0));
    let mut first = Raster::new(400, 200);
    vector::render_element(&mut first, &e);
    // Une copie recolorée partage le cache de l'original : sa signature a changé, elle est
    // rerendue dans sa couleur ; et un trait sans réglage ne prend pas les pixels de son jumeau.
    let mut red = e.clone();
    red.set_color([200, 30, 30]);
    let mut r = Raster::new(400, 200);
    vector::render_element(&mut r, &red);
    let c = r.get(180, 90);
    assert!(c.r() > c.g() + 60, "{c:?}");
    let mut plain = e.clone();
    if let Element::Stroke { brush, .. } = &mut plain {
        *brush = None;
    }
    let mut p = Raster::new(400, 200);
    vector::render_element(&mut p, &plain);
    assert!(same_pixels(&first, &p).is_err(), "le trait lisse n'est pas le feutre");
}

#[test]
fn old_strokes_and_edited_strokes_still_render() {
    let lib = BrushLibrary::builtin();
    lib.share();
    // Un trait d'avant l'étape 5 : pas de réglage, pas de trace → le trait lisse d'avant.
    let old = Element::Stroke { points: vec![VPoint { x: 20.0, y: 40.0, r: 4.0 }, VPoint { x: 180.0, y: 40.0, r: 4.0 }], color: [0, 0, 0], opacity: 1.0, hardness: 0.9, brush: None, trace: Default::default() };
    let mut r = Raster::new(200, 80);
    vector::render_element(&mut r, &old);
    assert!(r.get(100, 40).a() >= 250 && r.get(100, 60).a() == 0);
    // Un trait texturé d'avant l'étape 5 (réglage, mais pas d'appuis) : l'appui vient des rayons.
    let s = BrushSettings::graphite();
    let textured_old = Element::Stroke { points: (0..=40).map(|k| VPoint { x: 20.0 + 4.0 * k as f32, y: 40.0, r: 3.0 }).collect(), color: [0, 0, 0], opacity: 0.9, hardness: 0.8, brush: lib.for_vector(&s).map(Box::new), trace: Default::default() };
    let mut r2 = Raster::new(200, 80);
    vector::render_element(&mut r2, &textured_old);
    assert!((24..176).filter(|&x| r2.get(x, 40).a() > 0).count() > 100);
    // Coupé par la gomme : des morceaux qui se rendent, avec leurs appuis (interpolés aux coupes).
    let (_, e) = bitmap_and_element(&s, &lib, 400, 200, &geste(120, 0.0, 0.0));
    let mut d = VectorDrawing { elements: vec![e.clone()] };
    assert!(vector::erase_circle(&mut d, pos2(180.0, 90.0), 12.0, false));
    assert!(d.elements.len() >= 2);
    let mut r3 = Raster::new(400, 200);
    for p in &d.elements {
        if let Element::Stroke { points, trace, .. } = p {
            assert!(trace.pressure.len() == points.len() && trace.time.len() == points.len() && trace.feel.is_some());
        }
        vector::render_element(&mut r3, p);
    }
    let inked = |x0: usize, x1: usize| (x0..x1).flat_map(|x| (20..180).map(move |y| (x, y))).filter(|&(x, y)| r3.get(x, y).a() > 0).count();
    assert!(inked(30, 160) > 200 && inked(200, 330) > 200, "les deux morceaux se rendent");
    assert_eq!(inked(176, 184), 0, "le trou de la gomme");
    // Enregistré puis relu : la trace suit, le cache non, et le rendu est le même.
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains("\"appui\"") && json.contains("\"instants\"") && !json.contains("cache"));
    let back: Element = serde_json::from_str(&json).unwrap();
    // Les appuis sont enregistrés au 1/65535, les instants au dixième de milliseconde depuis le
    // premier : le trait relu est le même à ces arrondis près.
    let (Element::Stroke { points: pe, trace: te, .. }, Element::Stroke { points: pb, trace: tb, .. }) = (&e, &back) else { panic!() };
    assert_eq!(pe, pb);
    assert_eq!((te.feel, te.secondary), (tb.feel, tb.secondary));
    assert!(te.pressure.iter().zip(&tb.pressure).all(|(a, b)| (a - b).abs() <= 1.0 / 65535.0));
    assert!(te.time.iter().zip(&tb.time).all(|(a, b)| ((a - te.time[0]) - b).abs() <= 1e-4));
    assert_eq!(back.cache_bytes(), 0);
    let (mut a, mut b) = (Raster::new(400, 200), Raster::new(400, 200));
    vector::render_element(&mut a, &e);
    vector::render_element(&mut b, &back);
    near_pixels(&a, &b, 1).unwrap();
    // Un vieux fichier sans « trace » se relit.
    let legacy: Element = serde_json::from_str(r#"{"type":"Stroke","points":[{"x":1.0,"y":2.0,"r":3.0}],"color":[0,0,0],"opacity":1.0,"hardness":1.0}"#).unwrap();
    assert!(matches!(legacy, Element::Stroke { ref trace, .. } if trace.pressure.is_empty() && trace.feel.is_none()));
}

/// Le glissé d'une sélection vectorielle (Anthony : « le mode transformation ralentit énormément
/// quand je sélectionne un objet vectoriel et que je veux le déplacer ») : cinquante traits de
/// crayon graphite sur un dessin 1920×1080, tous déplacés image après image. Avec le cache, chaque
/// image pose les pixels gardés ; sans (comme avant), chaque image rejoue le moteur.
/// `cargo test --release --offline --target-dir target\audit vector_selection_drag -- --ignored --nocapture`
#[test]
#[ignore]
fn vector_selection_drag_is_fast_with_the_pixel_cache() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let s = BrushSettings::graphite();
    let mut cel = Cel::new(1920, 1080, LayerKind::Vector);
    let mut originals = Vec::new();
    for k in 0..50 {
        let (_, e) = bitmap_and_element(&s, &lib, 1920, 1080, &geste(200, (k % 5) as f32 * 300.0, (k / 5) as f32 * 95.0));
        originals.push(e);
    }
    cel.vector.as_mut().unwrap().elements = originals.clone();
    let t0 = Instant::now();
    cel.rerender();
    println!("premier rendu (rejeu de 50 traits) : {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    let bytes: usize = cel.vector.as_ref().unwrap().elements.iter().map(Element::cache_bytes).sum();
    println!("cache : {} Ko pour 50 traits ({} Ko par trait)", bytes / 1024, bytes / 1024 / 50);
    let frames = 30;
    let mut drag = |with_cache: bool| -> f64 {
        let mut total = 0.0;
        for f in 1..=frames {
            let aff = Affine::translate(eframe::egui::vec2(3.0 * f as f32, 2.0 * f as f32));
            let t = Instant::now();
            let d = cel.vector.as_mut().unwrap();
            let mut region = Rect::NOTHING;
            for (i, src) in originals.iter().enumerate() {
                region = region.union(d.elements[i].bounds().expand(1.5));
                d.elements[i] = src.transformed_preview(&aff);
                if !with_cache {
                    d.elements[i].invalidate_cache();
                }
                region = region.union(d.elements[i].bounds().expand(1.5));
            }
            cel.rerender_region(region.expand(2.0));
            total += t.elapsed().as_secs_f64() * 1e3;
        }
        total / frames as f64
    };
    let cached = drag(true);
    println!("glissé avec le cache : {cached:.1} ms par image");
    let replayed = drag(false);
    println!("glissé sans le cache (comme avant) : {replayed:.1} ms par image");
    assert!(cached < 16.0, "{cached:.1} ms par image");
    assert!(replayed > cached);
}
