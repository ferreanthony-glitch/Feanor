//! La règle d'adaptation d'une copie à la feuille qui la reçoit (`adaptation.rs`) : l'échelle et le décalage, le
//! rééchantillonnage (feuille et hors-feuille), les traits, la caméra, les effets, les clés de position, la cadence.
//! Le parcours dans l'application (question, calcul de fond, annulation) est dans `collage_tests.rs`.

use std::collections::HashMap;

use eframe::egui::{pos2, vec2, Color32, Vec2};

use crate::adaptation::*;
use crate::camera::{Camera, CameraKey};
use crate::document::{Cel, CelId, Layer, LayerKind, LayerTransform, Panel};
use crate::fx::{Effect, EffectKind};
use crate::poskeys::PosKey;
use crate::raster::Raster;
use crate::vector::{Element, VPoint};

/// Un projet dont le cadre fait `w`×`h` : la feuille est deux fois plus grande (`Document::with_frame`).
fn orig(w: usize, h: usize, fps: u32) -> Origine {
    Origine { cadre: [w, h], feuille: [w * 2, h * 2], cadence: fps }
}

fn regle(de: Origine, vers: Origine) -> Adaptation {
    Adaptation::new(de, vers, Ajustement::Contenir, Duree::Images)
}

fn pose(r: &mut Raster, x: i64, y: i64, c: Color32) {
    let (tx, ty) = (x.div_euclid(64) as i32, y.div_euclid(64) as i32);
    let id = r.id_of(tx, ty);
    r.tile_mut(id)[y.rem_euclid(64) as usize * 64 + x.rem_euclid(64) as usize] = c;
}

fn rempli(r: &mut Raster, x0: i64, y0: i64, x1: i64, y1: i64, c: Color32) {
    for y in y0..y1 {
        for x in x0..x1 {
            pose(r, x, y, c);
        }
    }
}

const ROUGE: Color32 = Color32::from_rgba_premultiplied(255, 0, 0, 255);

// ---------------------------------------------------------------------------------------------- la règle

#[test]
fn the_rule_fits_the_source_camera_frame_to_the_destination_one() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    assert_eq!(a.s, 0.5);
    assert_eq!(a.t, Vec2::ZERO, "les centres de feuille coïncident");
    assert!(a.spatiale() && !a.retemps() && !a.identite());
    // Le centre de la feuille d'origine tombe sur celui de la feuille d'arrivée, le coin du cadre sur le coin du cadre.
    assert_eq!(a.point(pos2(3840.0, 2160.0)), pos2(1920.0, 1080.0));
    assert_eq!(a.point(pos2(3840.0 - 1920.0, 2160.0 - 1080.0)), pos2(1920.0 - 960.0, 1080.0 - 540.0));
    // Et dans l'autre sens.
    let b = regle(orig(1920, 1080, 24), orig(3840, 2160, 24));
    assert_eq!(b.s, 2.0);
    assert_eq!(b.t, Vec2::ZERO);
}

#[test]
fn contain_and_fill_differ_only_when_the_proportions_differ() {
    let (de, vers) = (orig(1920, 1080, 24), orig(1080, 1080, 24));
    assert!(de.autres_proportions(&vers));
    let contenir = Adaptation::new(de, vers, Ajustement::Contenir, Duree::Images);
    let remplir = Adaptation::new(de, vers, Ajustement::Remplir, Duree::Images);
    assert!((contenir.s - 0.5625).abs() < 1e-6, "tout le cadre d'origine reste visible : {}", contenir.s);
    assert_eq!(remplir.s, 1.0, "le cadre carré est rempli");
    assert_eq!(remplir.t, vec2(-840.0, 0.0), "recentré, décalage entier");
    // Mêmes proportions : pas de choix à faire.
    let (a, b) = (orig(1920, 1080, 24), orig(1280, 720, 24));
    assert!(!a.autres_proportions(&b));
    assert_eq!(Adaptation::new(a, b, Ajustement::Contenir, Duree::Images).s, Adaptation::new(a, b, Ajustement::Remplir, Duree::Images).s);
}

#[test]
fn the_same_frame_and_sheet_change_nothing_and_a_bigger_sheet_only_shifts() {
    let a = regle(orig(1920, 1080, 24), orig(1920, 1080, 24));
    assert!(a.identite() && !a.spatiale());
    // Même cadre, autre cadence, images gardées : encore rien à faire.
    assert!(regle(orig(1920, 1080, 24), orig(1920, 1080, 25)).identite());
    // Même cadre, feuille agrandie à la main : un décalage entier, pas de rééchantillonnage.
    let grande = Origine { cadre: [1920, 1080], feuille: [4000, 2400], cadence: 24 };
    let b = regle(orig(1920, 1080, 24), grande);
    assert_eq!(b.s, 1.0);
    assert_eq!(b.t, vec2(80.0, 120.0));
    assert!(b.spatiale() && !b.identite());
}

#[test]
fn the_status_message_says_what_was_adapted() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    assert_eq!(a.resume(), "Collé depuis un projet 3840×2160 : mis à l'échelle ×0,5.");
    let b = Adaptation::new(orig(1920, 1080, 24), orig(1080, 1080, 25), Ajustement::Remplir, Duree::Secondes);
    let t = b.resume();
    assert!(t.contains("cadre rempli") && t.contains("24 → 25 i/s") && t.contains("durées gardées en secondes"), "{t}");
    let c = regle(orig(1920, 1080, 24), orig(1920, 1080, 25));
    assert!(!c.spatiale());
    let d = Adaptation::new(orig(1920, 1080, 24), Origine { cadre: [1920, 1080], feuille: [4000, 2400], cadence: 24 }, Ajustement::Contenir, Duree::Images);
    assert!(d.resume().contains("recentré"), "{}", d.resume());
}

// ---------------------------------------------------------------------------------------------- les pixels

#[test]
fn a_bitmap_is_resampled_with_its_area_and_its_colour_kept() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let mut cel = Cel::new(7680, 4320, LayerKind::Bitmap);
    // Un rectangle rouge de 400 × 200 au centre de la feuille.
    rempli(&mut cel.raster, 3840 - 200, 2160 - 100, 3840 + 200, 2160 + 100, ROUGE);
    let out = adapte_dessin(&cel, &a, 3840, 2160, 2);
    assert_eq!((out.raster.width, out.raster.height), (3840, 2160));
    assert_eq!(out.raster.get(1920, 1080), ROUGE);
    assert_eq!(out.raster.get(1920 + 101, 1080).a(), 0, "hors du rectangle réduit (200 × 100)");
    assert_eq!(out.raster.get(1920, 1080 + 51).a(), 0);
    // La surface (alpha cumulé) est celle du rectangle réduit, à 1 % près.
    let mut total = 0u64;
    for y in 1000..1160 {
        for x in 1800..2040 {
            total += out.raster.get(x, y).a() as u64;
        }
    }
    let attendu = 200.0 * 100.0 * 255.0;
    assert!((total as f64 - attendu).abs() / attendu < 0.01, "{total} contre {attendu}");
    // Aucune couleur n'a bavé : prémultiplié, tout ce qui est visible est du rouge.
    for y in 1000..1160 {
        for x in 1800..2040 {
            let c = out.raster.get(x, y);
            assert!(c.g() == 0 && c.b() == 0 && c.r() == c.a(), "pixel {x},{y} : {c:?}");
        }
    }
}

#[test]
fn upscaling_keeps_flat_colours_flat_and_edges_soft() {
    let a = regle(orig(1920, 1080, 24), orig(3840, 2160, 24));
    let mut cel = Cel::new(3840, 2160, LayerKind::Bitmap);
    let vert = Color32::from_rgba_premultiplied(10, 200, 30, 255);
    rempli(&mut cel.raster, 1920 - 50, 1080 - 50, 1920 + 50, 1080 + 50, vert);
    let out = adapte_dessin(&cel, &a, 7680, 4320, 2);
    for (x, y) in [(3840, 2160), (3840 + 50, 2160 - 60), (3840 - 80, 2160 + 80)] {
        let c = out.raster.get(x, y);
        assert!((c.r() as i32 - 10).abs() <= 1 && (c.g() as i32 - 200).abs() <= 1 && (c.b() as i32 - 30).abs() <= 1 && c.a() == 255, "{c:?}");
    }
    assert_eq!(out.raster.get(3840 + 140, 2160).a(), 0);
}

#[test]
fn what_is_drawn_off_the_sheet_follows_the_rule_too() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let mut cel = Cel::new(7680, 4320, LayerKind::Bitmap);
    // Hors de la feuille, en haut à gauche (coordonnées négatives) et très loin à droite.
    rempli(&mut cel.raster, -120, -120, -80, -80, ROUGE);
    rempli(&mut cel.raster, 20_000, 20_000, 20_040, 20_040, ROUGE);
    assert!(cel.raster.has_beyond());
    let out = adapte_dessin(&cel, &a, 3840, 2160, 1);
    assert_eq!(out.raster.get_i(-50, -50), ROUGE, "(-100, -100) devient (-50, -50)");
    assert_eq!(out.raster.get_i(10_010, 10_010), ROUGE, "(20 020, 20 020) devient (10 010, 10 010), toujours hors feuille");
    assert_eq!(out.raster.get_i(-100, -100).a(), 0);
    assert!(out.raster.has_beyond());
}

#[test]
fn the_same_frame_on_a_bigger_sheet_copies_the_pixels_exactly() {
    let grande = Origine { cadre: [160, 90], feuille: [400, 240], cadence: 24 };
    let a = regle(orig(160, 90, 24), grande);
    assert_eq!(a.t, vec2(40.0, 30.0));
    let mut cel = Cel::new(320, 180, LayerKind::Bitmap);
    pose(&mut cel.raster, 10, 10, Color32::from_rgba_premultiplied(1, 2, 3, 255));
    pose(&mut cel.raster, 319, 179, Color32::from_rgba_premultiplied(9, 8, 7, 255));
    let out = adapte_dessin(&cel, &a, 400, 240, 1);
    assert_eq!(out.raster.get(50, 40), Color32::from_rgba_premultiplied(1, 2, 3, 255));
    assert_eq!(out.raster.get(359, 209), Color32::from_rgba_premultiplied(9, 8, 7, 255));
    assert_eq!(out.raster.get(10, 10).a(), 0);
}

#[test]
fn the_fond_strata_follow_their_drawing() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let mut cel = Cel::new(7680, 4320, LayerKind::Bitmap);
    let mut strate = crate::document::Strate::new(7680, 4320);
    rempli(&mut strate.peinture.raster, 3800, 2120, 3880, 2200, ROUGE);
    rempli(&mut strate.coupe, 3800, 2120, 3840, 2160, Color32::from_rgba_premultiplied(0, 0, 0, 255));
    cel.fond = Some(Box::new(strate));
    let out = adapte_dessin(&cel, &a, 3840, 2160, 1);
    let f = out.fond.expect("le fond suit");
    assert_eq!(f.peinture.raster.get(1920, 1080), ROUGE);
    assert_eq!(f.peinture.raster.get(1890, 1080).a(), 0);
    assert_eq!(f.coupe.get(1910, 1070).a(), 255);
    assert_eq!(f.coupe.get(1930, 1090).a(), 0);
}

// ---------------------------------------------------------------------------------------------- les traits

fn trait_vectoriel() -> Cel {
    let mut cel = Cel::new(7680, 4320, LayerKind::Vector);
    cel.vector.as_mut().expect("vectoriel").elements.push(Element::Stroke {
        points: vec![VPoint { x: 3640.0, y: 2160.0, r: 8.0 }, VPoint { x: 4040.0, y: 2160.0, r: 8.0 }],
        color: [200, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    });
    cel.rerender();
    cel
}

#[test]
fn vector_strokes_scale_their_points_and_their_widths_and_render_again() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let out = adapte_dessin(&trait_vectoriel(), &a, 3840, 2160, 1);
    let Element::Stroke { points, .. } = &out.vector.as_ref().expect("vectoriel").elements[0] else { panic!("un trait") };
    assert_eq!((points[0].x, points[0].y, points[0].r), (1820.0, 1080.0, 4.0));
    assert_eq!((points[1].x, points[1].y, points[1].r), (2020.0, 1080.0, 4.0));
    assert_eq!((out.raster.width, out.raster.height), (3840, 2160));
    assert!(out.raster.get(1920, 1080).a() > 200, "le rendu est refait à la nouvelle place");
    assert_eq!(out.raster.get(1920, 1080 + 10).a(), 0, "et à la nouvelle épaisseur (rayon 4)");
    assert_eq!(out.raster.get(3840, 2160).a(), 0, "l'ancienne place est vide");
}

#[test]
fn pasted_vector_elements_and_bitmap_zones_keep_their_place_in_the_frame() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let cel = trait_vectoriel();
    let elements = adapte_elements(&cel.vector.as_ref().expect("vectoriel").elements, &a);
    let Element::Stroke { points, .. } = &elements[0] else { panic!("un trait") };
    assert_eq!(points[0].x, 1820.0);
    // Une zone de 100 × 60 pixels au centre de la feuille d'origine.
    let zone = crate::pixsel::Floating { x0: 3790, y0: 2130, w: 100, h: 60, pixels: vec![ROUGE; 100 * 60] };
    let z = adapte_zone(&zone, &a).expect("la zone reste sur la feuille");
    let (cx, cy) = (z.x0 as f32 + z.w as f32 / 2.0, z.y0 as f32 + z.h as f32 / 2.0);
    assert!((cx - 1920.0).abs() <= 1.5 && (cy - 1080.0).abs() <= 1.5, "le centre de la zone reste au centre : {cx}, {cy}");
    assert!((z.w as i32 - 50).abs() <= 3 && (z.h as i32 - 30).abs() <= 3, "{}×{}", z.w, z.h);
    assert!(z.pixels.iter().filter(|p| p.a() == 255).count() > 1000);
    // Une zone qui tombe hors de la feuille d'arrivée : rien à coller.
    let dehors = Adaptation::new(orig(1920, 1080, 24), orig(100, 100, 24), Ajustement::Remplir, Duree::Images);
    let l = crate::pixsel::Floating { x0: 3500, y0: 100, w: 10, h: 10, pixels: vec![ROUGE; 100] };
    assert!(adapte_zone(&l, &dehors).is_none(), "« remplir » : la zone tombe hors de la petite feuille, laissée de côté");
}

// ---------------------------------------------------------------------------------------------- caméra, pose, effets

#[test]
fn camera_keys_follow_the_rule_and_keep_their_lens() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let c = Camera { x: 3840.0 + 500.0, y: 2160.0 - 300.0, zoom: 1.5, angle: 10.0, dolly: 100.0, tilt: 3.0, pan: -2.0 };
    let k = adapte_cle_camera(&CameraKey::new(5, c), &a);
    assert_eq!((k.cam.x, k.cam.y), (1920.0 + 250.0, 1080.0 - 150.0));
    assert_eq!(k.cam.dolly, 50.0, "l'avancée suit la largeur du cadre");
    assert_eq!((k.cam.zoom, k.cam.angle, k.cam.tilt, k.cam.pan), (1.5, 10.0, 3.0, -2.0));
    assert_eq!(k.frame, 5);
    // Une caméra au centre de la feuille reste au centre.
    let centre = adapte_camera(&Camera::centered(7680, 4320), &a);
    assert_eq!((centre.x, centre.y), (1920.0, 1080.0));
    // Aspect différent, « contenir » : l'avancée suit le rapport des largeurs, la position celui du cadre contenu.
    let b = Adaptation::new(orig(1920, 1080, 24), orig(1080, 1080, 24), Ajustement::Contenir, Duree::Images);
    let cam = adapte_camera(&Camera { dolly: 80.0, ..Camera::centered(3840, 2160) }, &b);
    assert_eq!(cam.dolly, 80.0 * 1080.0 / 1920.0);
    assert_eq!((cam.x, cam.y), (1080.0, 1080.0));
}

fn calque_complet() -> Layer {
    let mut l = Layer::new("Décor", LayerKind::Bitmap);
    l.transform = LayerTransform { x: 100.0, y: -40.0, scale: 1.3, angle: 12.0, stretch_x: 1.1, stretch_y: 0.9, ..Default::default() };
    l.pos_keys = vec![PosKey::new(0, l.transform), PosKey::new(8, LayerTransform { x: 400.0, y: 200.0, ..l.transform })];
    let mut flou = Effect::new(EffectKind::GaussianBlur);
    flou.values[0] = 40.0;
    flou.set_key(0, vec![40.0]);
    flou.set_key(10, vec![80.0]);
    let mut ombre = Effect::new(EffectKind::DropShadow);
    ombre.values = vec![10.0, -12.0, 6.0, 60.0];
    let mut vignette = Effect::new(EffectKind::Vignette);
    vignette.values[1] = 70.0;
    l.fx = vec![flou, ombre, vignette];
    l.fond = Some(crate::fond::FondReglages { debord: 4, trous: 6, ..Default::default() });
    l.couleur = Some(crate::calquecouleur::CalqueCouleur { trous: 4, ..crate::calquecouleur::CalqueCouleur::new(0) });
    l
}

#[test]
fn layer_poses_position_keys_and_pixel_effect_parameters_follow_the_rule() {
    let a = regle(orig(3840, 2160, 24), orig(1920, 1080, 24));
    let mut l = calque_complet();
    adapte_calque(&mut l, &a);
    assert_eq!((l.transform.x, l.transform.y), (50.0, -20.0));
    assert_eq!((l.transform.scale, l.transform.angle, l.transform.stretch_x, l.transform.stretch_y), (1.3, 12.0, 1.1, 0.9));
    assert_eq!((l.pos_keys[1].t.x, l.pos_keys[1].t.y, l.pos_keys[1].frame), (200.0, 100.0, 8));
    // Flou : le rayon en pixels, valeurs et clés.
    assert_eq!(l.fx[0].values[0], 20.0);
    assert_eq!((l.fx[0].keys[0].values[0], l.fx[0].keys[1].values[0]), (20.0, 40.0));
    // Ombre : les deux décalages et le flou en pixels ; l'opacité en pour cent ne bouge pas.
    assert_eq!(l.fx[1].values, vec![5.0, -6.0, 3.0, 60.0]);
    // Vignettage : rien n'est en pixels.
    let mut attendu = Effect::new(EffectKind::Vignette).values;
    attendu[1] = 70.0;
    assert_eq!(l.fx[2].values, attendu);
    // Fond sous le trait et fermeture des trous : en pixels, entiers, jamais nuls s'ils ne l'étaient pas.
    let f = l.fond.expect("fond");
    assert_eq!((f.debord, f.trous), (2, 3));
    assert_eq!(l.couleur.as_ref().expect("calque de couleur").trous, 2, "la fermeture des trous du calque de couleur (peinture et remplissage au griffonnage)");
    let mut petit = calque_complet();
    adapte_calque(&mut petit, &Adaptation::new(orig(3840, 2160, 24), orig(384, 216, 24), Ajustement::Contenir, Duree::Images));
    assert_eq!(petit.fond.expect("fond").debord, 1, "au moins 1 pixel");
}

#[test]
fn effect_values_stay_in_the_bounds_and_whole_numbers_stay_whole() {
    let mut flou = vec![150.0];
    adapte_valeurs(EffectKind::GaussianBlur, &mut flou, 2.0);
    assert_eq!(flou, vec![200.0], "la borne du flou");
    // Le trait sec : les trous pour 100 px de trait sont une densité, elle monte quand les traits raccourcissent.
    let mut sec = Effect::new(EffectKind::Dry).values;
    let avant = sec[crate::fxmatiere::DRY_FREQ];
    adapte_valeurs(EffectKind::Dry, &mut sec, 0.5);
    assert_eq!(sec[crate::fxmatiere::DRY_FREQ], avant * 2.0);
    assert_eq!(sec[crate::fxmatiere::DRY_LENGTH], Effect::new(EffectKind::Dry).values[crate::fxmatiere::DRY_LENGTH] * 0.5);
    // Le grain : sa taille est un entier en pixels.
    let mut grain = Effect::new(EffectKind::Grain).values;
    grain[1] = 3.0;
    adapte_valeurs(EffectKind::Grain, &mut grain, 0.5);
    assert_eq!(grain[1], 2.0, "1,5 arrondi, puis dans les bornes 1 à 8");
    // Le tremblement : l'intensité est en pixels, les variantes non.
    let mut boil = Effect::new(EffectKind::Boil).values;
    adapte_valeurs(EffectKind::Boil, &mut boil, 0.5);
    assert_eq!(boil[crate::tremblement::INTENSITY], 1.5);
    assert_eq!(boil[crate::tremblement::VARIANTS], 3.0);
}

#[test]
fn every_pixel_parameter_of_every_effect_is_recognised_by_its_unit() {
    // Garde : un effet neuf dont un paramètre est en pixels doit le dire dans son unité, sans quoi il ne suit pas la
    // règle. On vérifie que chaque paramètre en pixels bouge, et qu'aucun autre ne bouge.
    for kind in EffectKind::ALL {
        let base: Vec<f32> = kind.params().iter().map(|p| (p.min + (p.max - p.min) * 0.7).round()).collect();
        let mut v = base.clone();
        adapte_valeurs(kind, &mut v, 0.5);
        for (k, p) in kind.params().iter().enumerate() {
            let px = p.unit.trim_start().starts_with("px");
            let densite = kind == EffectKind::Dry && k == crate::fxmatiere::DRY_FREQ;
            if px || densite {
                assert_ne!(v[k], base[k], "{} : « {} » est en pixels et doit suivre", kind.label(), p.name);
            } else {
                assert_eq!(v[k], base[k], "{} : « {} » n'est pas en pixels et ne doit pas bouger", kind.label(), p.name);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------- la cadence

fn un_plan(n: usize) -> Panel {
    let mut doc = crate::document::Document::with_frame(160, 90, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for _ in 0..n {
        layer.frames.push(Some(doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap))));
    }
    layer.roles.insert(12, crate::roles::Role::Key);
    let mut flou = Effect::new(EffectKind::GaussianBlur);
    flou.set_key(12, vec![9.0]);
    layer.fx = vec![flou];
    layer.pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(12, LayerTransform { x: 10.0, ..Default::default() })];
    let mut p = doc.panels.remove(0);
    p.layers = vec![layer];
    p.length = n;
    p.camera = vec![CameraKey::new(0, Camera::centered(320, 180)), CameraKey::new(12, Camera::centered(320, 180))];
    p.marks = vec![crate::document::ImageMark { id: 7, frame: 12, ..Default::default() }];
    p.cues.bookmarks = vec![6, 18];
    p.cues.in_point = Some(6);
    p.cues.out_point = Some(18);
    p
}

#[test]
fn keeping_the_images_changes_no_frame_counts() {
    let a = Adaptation::new(orig(1920, 1080, 24), orig(1920, 1080, 25), Ajustement::Contenir, Duree::Images);
    let mut p = un_plan(24);
    adapte_plan(&mut p, &a);
    assert_eq!(p.length, 24);
    assert_eq!(p.layers[0].frames.len(), 24);
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 12]);
}

#[test]
fn keeping_the_duration_in_seconds_rescales_every_frame_count() {
    // 24 → 12 : tout est divisé par deux.
    let a = Adaptation::new(orig(1920, 1080, 24), orig(1920, 1080, 12), Ajustement::Contenir, Duree::Secondes);
    assert!(a.retemps());
    let mut p = un_plan(24);
    let avant: Vec<Option<CelId>> = p.layers[0].frames.clone();
    adapte_plan(&mut p, &a);
    assert_eq!(p.length, 12);
    assert_eq!(p.layers[0].frames.len(), 12);
    for j in 0..12 {
        assert_eq!(p.layers[0].frames[j], avant[2 * j], "l'image {j} est le dessin qui était là à la même seconde");
    }
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 6]);
    assert_eq!(p.layers[0].fx[0].keys[0].frame, 6);
    assert_eq!(p.layers[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 6]);
    assert!(p.layers[0].roles.contains_key(&6));
    assert_eq!(p.marks[0].frame, 6);
    assert_eq!((p.cues.bookmarks.clone(), p.cues.in_point, p.cues.out_point), (vec![3, 9], Some(3), Some(9)));
    // 24 → 25 : 24 images deviennent 25, le premier et le dernier dessin restent aux extrémités.
    let b = Adaptation::new(orig(1920, 1080, 24), orig(1920, 1080, 25), Ajustement::Contenir, Duree::Secondes);
    let mut q = un_plan(24);
    let dessins = q.layers[0].frames.clone();
    adapte_plan(&mut q, &b);
    assert_eq!(q.length, 25);
    assert_eq!(q.layers[0].frames.len(), 25);
    assert_eq!(q.layers[0].frames[0], dessins[0]);
    assert_eq!(q.layers[0].frames[24], dessins[23]);
    assert_eq!(q.camera[1].frame, 13, "12 × 25 / 24 = 12,5, arrondi vers le haut comme « Changer la cadence »");
    // Plus lent : des dessins tenus plus longtemps, jamais de dessin inventé.
    let lent = Adaptation::new(orig(1920, 1080, 12), orig(1920, 1080, 24), Ajustement::Contenir, Duree::Secondes);
    let mut r = un_plan(12);
    let six = r.layers[0].frames.clone();
    adapte_plan(&mut r, &lent);
    assert_eq!(r.layers[0].frames.len(), 24);
    assert!(r.layers[0].frames.iter().all(|f| six.contains(f)));
    assert_eq!(r.layers[0].frames[0], r.layers[0].frames[1], "un dessin tenu deux fois plus longtemps");
}

#[test]
fn space_and_time_adapt_together() {
    let a = Adaptation::new(orig(3840, 2160, 24), orig(1920, 1080, 12), Ajustement::Contenir, Duree::Secondes);
    let mut p = un_plan(24);
    p.camera[1].cam = Camera { x: 3840.0 + 100.0, ..Camera::centered(7680, 4320) };
    adapte_plan(&mut p, &a);
    assert_eq!(p.camera[1].frame, 6);
    assert_eq!(p.camera[1].cam.x, 1920.0 + 50.0);
}

#[test]
fn a_cel_map_without_a_spatial_change_is_shared_not_resampled() {
    let a = regle(orig(1920, 1080, 24), orig(1920, 1080, 12));
    let mut cels: HashMap<CelId, Cel> = HashMap::new();
    let mut cel = Cel::new(3840, 2160, LayerKind::Bitmap);
    pose(&mut cel.raster, 5, 5, ROUGE);
    cels.insert(1, cel);
    let p = Progression::default();
    let out = adapte_dessins(&cels, &a, &p);
    let tuile = |m: &HashMap<CelId, Cel>| m[&1].raster.tiles.iter().flatten().next().expect("tuile").clone();
    assert!(tuile(&out).same_as(&tuile(&cels)), "les tuiles sont partagées");
    assert_eq!(p.fait.load(std::sync::atomic::Ordering::Relaxed), p.total.load(std::sync::atomic::Ordering::Relaxed));
}
