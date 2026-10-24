//! Grab et Pull (`sculpt.rs`, `sculptapp.rs`) : ce que chaque outil déplace, la force nulle, l'annulation exacte, les
//! clés choisies, l'aplat, et l'absence de rejeu du moteur sur le fil de l'interface.

use eframe::egui::{pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::app::{Mode, Tool};
use crate::document::{CelId, Document, Layer, LayerKind, Strate};
use crate::fond::FondReglages;
use crate::montagekeys::KeyRef;
use crate::perf_tests::{banc_taille, Banc};
use crate::raster::{Raster, TILE};
use crate::sculpt::{self, Attenuation, Reglages};
use crate::vector::{Element, VPoint, VectorDrawing};

// ---------------------------------------------------------------------------------------------- le cœur

fn ligne_verticale(x: f32, y0: f32, y1: f32, pas: f32) -> Element {
    let n = ((y1 - y0) / pas).round() as usize;
    Element::Stroke {
        points: (0..=n).map(|k| VPoint { x, y: y0 + k as f32 * pas, r: 1.5 }).collect(),
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

fn points_de(e: &Element) -> Vec<VPoint> {
    match e {
        Element::Stroke { points, .. } => points.clone(),
        _ => panic!("un trait"),
    }
}

#[test]
fn the_four_curves_fall_from_full_weight_at_the_centre_to_nothing_at_the_edge() {
    for a in Attenuation::ALL {
        assert_eq!(a.weight(0.0), 1.0, "{a:?} : plein poids au centre");
        assert_eq!(a.weight(1.0), 0.0, "{a:?} : rien au bord");
        assert_eq!(a.weight(1.5), 0.0, "{a:?} : rien au-delà");
        let mut avant = 1.0;
        for k in 1..100 {
            let w = a.weight(k as f32 / 100.0);
            assert!((0.0..=1.0).contains(&w) && w <= avant + 1e-6, "{a:?} : ne remonte jamais ({k}) ");
            avant = w;
        }
    }
    // Au milieu du rayon : la douce et la linéaire à moitié, la pointue au quart, la constante pleine.
    assert!((Attenuation::Douce.weight(0.5) - 0.5).abs() < 1e-6);
    assert!((Attenuation::Lineaire.weight(0.5) - 0.5).abs() < 1e-6);
    assert!((Attenuation::Pointue.weight(0.5) - 0.25).abs() < 1e-6);
    assert_eq!(Attenuation::Constante.weight(0.5), 1.0);
    assert_eq!(Attenuation::Constante.weight(0.999), 1.0);
}

#[test]
fn pressure_scales_the_radius_and_the_force_only_when_their_switch_is_on() {
    let mut r = Reglages::grab();
    r.rayon = 100.0;
    r.force = 0.8;
    assert_eq!(r.effectifs(0.25), (100.0, 0.8), "interrupteurs éteints : la pression ne compte pas");
    r.rayon_appui = true;
    assert_eq!(r.effectifs(0.25), (25.0, 0.8));
    r.force_appui = true;
    let (rayon, force) = r.effectifs(0.5);
    assert!((rayon - 50.0).abs() < 1e-4 && (force - 0.4).abs() < 1e-6);
    assert_eq!(r.effectifs(1.0), (100.0, 0.8), "à la souris, la pression vaut 100 %");
}

#[test]
fn grab_catches_only_the_points_inside_the_radius_with_the_weight_of_their_distance() {
    let d = VectorDrawing { elements: vec![ligne_verticale(100.0, 0.0, 200.0, 5.0), ligne_verticale(300.0, 0.0, 200.0, 5.0)] };
    for att in Attenuation::ALL {
        let prises = sculpt::prendre(&d, pos2(100.0, 100.0), 30.0, att);
        assert!(!prises.is_empty());
        for p in &prises {
            assert_eq!(p.at.element, 0, "le trait lointain n'est pas pris");
            let dist = p.depart.distance(pos2(100.0, 100.0));
            assert!(dist < 30.0, "{att:?} : un point à {dist} est pris");
            assert!((p.poids - att.weight(dist / 30.0)).abs() < 1e-6);
        }
        // Tout point du rayon qui a un poids est pris, et eux seuls.
        let attendus = points_de(&d.elements[0]).iter().filter(|p| att.weight(p.pos().distance(pos2(100.0, 100.0)) / 30.0) > 0.0).count();
        assert_eq!(prises.len(), attendus, "{att:?}");
    }
}

#[test]
fn grab_moves_the_caught_points_by_delta_times_weight_times_force_and_keeps_the_widths() {
    let d0 = VectorDrawing { elements: vec![ligne_verticale(100.0, 0.0, 200.0, 5.0)] };
    let prises = sculpt::prendre(&d0, pos2(100.0, 100.0), 30.0, Attenuation::Lineaire);
    let mut d = d0.clone();
    sculpt::grab_appliquer(&mut d, &prises, vec2(20.0, 0.0), 0.5);
    for (avant, apres) in points_de(&d0.elements[0]).iter().zip(points_de(&d.elements[0])) {
        let w = Attenuation::Lineaire.weight(avant.pos().distance(pos2(100.0, 100.0)) / 30.0);
        assert!((apres.x - (avant.x + 20.0 * w * 0.5)).abs() < 1e-4, "y = {} : {} contre {}", avant.y, apres.x, avant.x + 10.0 * w);
        assert_eq!(apres.y, avant.y, "le décalage est horizontal");
        assert_eq!(apres.r, avant.r, "l'épaisseur reste");
    }
    // Le centre suit à la moitié (force 0,5), un point à 15 px du quart de 20 (poids 0,5 × 0,5 × 20 = 5).
    let centre = points_de(&d.elements[0]).into_iter().find(|p| p.y == 100.0).unwrap();
    assert!((centre.x - 110.0).abs() < 1e-4);
    // Revenir au point d'appui remet tout en place : le décalage est toujours celui depuis l'appui.
    sculpt::grab_appliquer(&mut d, &prises, vec2(0.0, 0.0), 0.5);
    assert_eq!(d, d0);
}

#[test]
fn a_strength_of_zero_changes_nothing_for_points_and_for_pixels() {
    let d0 = VectorDrawing { elements: vec![ligne_verticale(100.0, 0.0, 200.0, 5.0)] };
    let prises = sculpt::prendre(&d0, pos2(100.0, 100.0), 40.0, Attenuation::Douce);
    let mut d = d0.clone();
    sculpt::grab_appliquer(&mut d, &prises, vec2(50.0, 20.0), 0.0);
    assert_eq!(d, d0, "Grab, force 0");
    let mut d = d0.clone();
    let mut boites = sculpt::Boites::de(&d);
    assert!(!sculpt::pull_pas(&mut d, &mut boites, pos2(100.0, 100.0), vec2(8.0, 0.0), 40.0, Attenuation::Douce, 0.0));
    assert_eq!(d, d0, "Pull, force 0");
    // Les pixels.
    let r0 = raster_avec_ligne();
    let mut r = r0.clone();
    assert_eq!(sculpt::pousser(&mut r, pos2(50.0, 50.0), vec2(8.0, 0.0), 20.0, Attenuation::Douce, 0.0), eframe::egui::Rect::NOTHING);
    assert!(memes_pixels(&r, &r0, 0, 0, 100, 100));
    let mut r = r0.clone();
    sculpt::saisir_raster(&r0, &mut r, pos2(50.0, 50.0), 20.0, Attenuation::Douce, 0.0, vec2(30.0, 0.0));
    assert!(memes_pixels(&r, &r0, 0, 0, 100, 100));
}

#[test]
fn pull_stretches_the_line_along_the_motion_and_the_set_of_points_follows_the_brush() {
    let d0 = VectorDrawing { elements: vec![ligne_verticale(100.0, 0.0, 200.0, 4.0)] };
    let mut d = d0.clone();
    let mut boites = sculpt::Boites::de(&d);
    // Le pinceau part du point (100, 100) et glisse vers la droite par pas de 5 px, force 1 : le point du centre le suit.
    let (rayon, force) = (30.0, 1.0);
    let mut c = pos2(100.0, 100.0);
    for _ in 0..12 {
        assert!(sculpt::pull_pas(&mut d, &mut boites, c, vec2(5.0, 0.0), rayon, Attenuation::Douce, force));
        c.x += 5.0;
    }
    let pts = points_de(&d.elements[0]);
    let le_plus_a_droite = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    assert!((le_plus_a_droite - 160.0).abs() < 1.0, "le point du centre a suivi le pinceau : x = {le_plus_a_droite}");
    // Les points loin de là n'ont jamais été sous le pinceau.
    for p in &pts {
        if (p.y - 100.0).abs() > 70.0 {
            assert_eq!(p.x, 100.0, "y = {}", p.y);
        }
    }
    // Un trait étiré : l'ordre des points le long du trait est gardé et leurs épaisseurs aussi.
    assert!(pts.windows(2).all(|w| w[1].y >= w[0].y - 1e-3), "le trait ne se replie pas");
    assert!(pts.iter().all(|p| p.r == 1.5));
    // Avec une force plus faible, le point du centre reste en arrière du pinceau : l'ensemble sous le pinceau change.
    let mut d = d0.clone();
    let mut boites = sculpt::Boites::de(&d);
    let mut c = pos2(100.0, 100.0);
    for _ in 0..12 {
        sculpt::pull_pas(&mut d, &mut boites, c, vec2(5.0, 0.0), rayon, Attenuation::Douce, 0.4);
        c.x += 5.0;
    }
    let loin = points_de(&d.elements[0]).iter().map(|p| p.x).fold(f32::MIN, f32::max);
    assert!(loin > 105.0 && loin < 150.0, "le point tiré reste en arrière du pinceau (x = {loin})");
}

#[test]
fn a_pull_on_a_two_point_line_inserts_points_so_the_stretch_is_smooth() {
    let ligne = Element::Stroke { points: vec![VPoint { x: 0.0, y: 50.0, r: 2.0 }, VPoint { x: 100.0, y: 50.0, r: 2.0 }], color: [0; 3], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() };
    let mut d = VectorDrawing { elements: vec![ligne] };
    let mut boites = sculpt::Boites::de(&d);
    assert!(sculpt::pull_pas(&mut d, &mut boites, pos2(50.0, 50.0), vec2(0.0, 12.0), 30.0, Attenuation::Douce, 1.0));
    let pts = points_de(&d.elements[0]);
    assert!(pts.len() > 8, "des points de plus dans le cercle : {}", pts.len());
    // Une bosse lisse : le plus haut point est au centre, les extrémités n'ont pas bougé, et rien ne fait de marche.
    let milieu = pts.iter().min_by(|a, b| (a.x - 50.0).abs().total_cmp(&(b.x - 50.0).abs())).unwrap();
    assert!((milieu.y - 62.0).abs() < 0.5, "le centre suit : {}", milieu.y);
    assert_eq!((pts[0].x, pts[0].y), (0.0, 50.0));
    assert_eq!((pts[pts.len() - 1].x, pts[pts.len() - 1].y), (100.0, 50.0));
    assert!(pts.windows(2).all(|w| (w[1].y - w[0].y).abs() < 4.0), "pas de marche entre deux points voisins");
    // Un élément loin du pinceau n'est pas regardé, ni changé.
    let loin = VectorDrawing { elements: vec![ligne_verticale(900.0, 0.0, 100.0, 50.0)] };
    let (mut d2, mut b2) = (loin.clone(), sculpt::Boites::de(&loin));
    assert!(!sculpt::pull_pas(&mut d2, &mut b2, pos2(50.0, 50.0), vec2(0.0, 12.0), 30.0, Attenuation::Douce, 1.0));
    assert_eq!(d2, loin);
}

/// Un pas de Pull sur un gros dessin (2 000 traits de 100 points) : seuls les éléments près du pinceau sont regardés.
/// `cargo test --release --offline --target-dir target\msvc a_vector_pull_step -- --ignored --nocapture`
#[test]
#[ignore]
fn a_vector_pull_step_over_a_big_drawing_costs_far_less_than_a_millisecond() {
    use std::time::Instant;
    let elements: Vec<Element> = (0..2000).map(|k| ligne_verticale(5.0 + (k % 100) as f32 * 19.0, 20.0 + (k / 100) as f32 * 50.0, 120.0 + (k / 100) as f32 * 50.0, 1.0)).collect();
    let n_points: usize = elements.iter().map(|e| points_de(e).len()).sum();
    let mut d = VectorDrawing { elements };
    let mut boites = sculpt::Boites::de(&d);
    let t = Instant::now();
    let pas = 200;
    for k in 0..pas {
        sculpt::pull_pas(&mut d, &mut boites, pos2(500.0 + k as f32 * 2.0, 300.0), vec2(2.0, 0.0), 40.0, Attenuation::Douce, 1.0);
    }
    let ms = t.elapsed().as_secs_f64() * 1e3 / pas as f64;
    println!("  Pull sur {n_points} points : {ms:.3} ms par pas");
    assert!(ms < 2.0, "{ms:.3} ms par pas");
}

#[test]
fn a_long_gap_gets_extra_points_that_keep_the_shape_the_widths_and_the_pressures() {
    let mut trace = crate::vector::Trace::default();
    trace.pressure = vec![0.2, 1.0];
    trace.time = vec![0.0, 1.0];
    let e = Element::Stroke { points: vec![VPoint { x: 0.0, y: 50.0, r: 1.0 }, VPoint { x: 100.0, y: 50.0, r: 3.0 }], color: [0; 3], opacity: 1.0, hardness: 1.0, brush: None, trace };
    let mut d = VectorDrawing { elements: vec![e] };
    assert!(sculpt::densifier(&mut d, pos2(50.0, 50.0), 20.0, 5.0));
    let Element::Stroke { points, trace, .. } = &d.elements[0] else { panic!() };
    assert!(points.len() >= 20, "un point tous les 5 px au plus : {}", points.len());
    assert!(trace.aligned(points.len()), "un appui par point");
    for (i, p) in points.iter().enumerate() {
        assert_eq!(p.y, 50.0, "sur la droite d'origine");
        let t = p.x / 100.0;
        assert!((p.r - (1.0 + 2.0 * t)).abs() < 1e-4, "rayon interpolé");
        assert!((trace.pressure[i] - (0.2 + 0.8 * t)).abs() < 1e-4, "appui interpolé");
        assert!((trace.time[i] - t as f64).abs() < 1e-4, "instant interpolé");
    }
    assert!(points.windows(2).all(|w| w[0].pos().distance(w[1].pos()) <= 5.0 + 1e-3));
    // Un segment loin du cercle n'est pas touché.
    let mut loin = VectorDrawing { elements: vec![ligne_verticale(500.0, 0.0, 100.0, 50.0)] };
    assert!(!sculpt::densifier(&mut loin, pos2(0.0, 0.0), 20.0, 5.0));
}

#[test]
fn filled_areas_move_with_the_points_and_texts_stay_rigid() {
    let carre = Element::Fill { contours: vec![vec![[90.0, 90.0], [110.0, 90.0], [110.0, 110.0], [90.0, 110.0]]], color: [1, 2, 3], opacity: 1.0 };
    let d0 = VectorDrawing { elements: vec![carre] };
    let prises = sculpt::prendre(&d0, pos2(100.0, 100.0), 40.0, Attenuation::Constante);
    assert_eq!(prises.len(), 4);
    let mut d = d0.clone();
    sculpt::grab_appliquer(&mut d, &prises, vec2(10.0, 5.0), 1.0);
    let Element::Fill { contours, .. } = &d.elements[0] else { panic!() };
    assert_eq!(contours[0][0], [100.0, 95.0]);
    assert_eq!(contours[0][2], [120.0, 115.0]);
}

// ---------------------------------------------------------------------------------------------- les pixels

fn poser_pixel(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let i = r.tile_index_of(x, y);
    r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = c;
}

/// 100 × 100 : un trait vertical épais de 4 px en x = 48..52, et un damier de petites taches autour (pour qu'un
/// déplacement se voie partout).
fn raster_avec_ligne() -> Raster {
    let mut r = Raster::new(100, 100);
    for y in 10..90 {
        for x in 48..52 {
            poser_pixel(&mut r, x, y, Color32::from_rgba_premultiplied(200, 0, 0, 255));
        }
    }
    r
}

fn memes_pixels(a: &Raster, b: &Raster, x0: usize, y0: usize, x1: usize, y1: usize) -> bool {
    (y0..y1).all(|y| (x0..x1).all(|x| a.get(x, y) == b.get(x, y)))
}

#[test]
fn a_pull_step_on_pixels_moves_those_inside_the_radius_and_leaves_every_other_pixel_identical() {
    let r0 = raster_avec_ligne();
    let mut r = r0.clone();
    let centre = pos2(50.0, 50.0);
    let rayon = 20.0;
    let zone = sculpt::pousser(&mut r, centre, vec2(6.0, 0.0), rayon, Attenuation::Douce, 1.0);
    assert!(zone.contains(centre));
    // Hors du cercle : identique, pixel pour pixel.
    for y in 0..100 {
        for x in 0..100 {
            let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
            if p.distance(centre) >= rayon {
                assert_eq!(r.get(x, y), r0.get(x, y), "({x}, {y}) est hors du cercle");
            }
        }
    }
    // Dedans : le trait a glissé vers la droite au centre (le centre du trait était en x = 50 ; il est vers 56).
    assert_eq!(r.get(48, 50).a(), 0, "le bord gauche du trait a quitté sa place au centre");
    assert!(r.get(55, 50).a() > 200 && r.get(57, 50).a() > 200, "le trait est maintenant vers x = 56");
    // Plus on s'éloigne du centre, moins il a bougé : vers le bord du cercle, le trait est presque en place.
    assert!(r.get(49, 33).a() > 200, "à 17 px du centre, le trait n'a presque pas bougé");
    // Un trait qui ne passe pas sous le pinceau : rien à voir.
    let mut vide = Raster::new(100, 100);
    assert!(sculpt::pousser(&mut vide, centre, vec2(6.0, 0.0), rayon, Attenuation::Douce, 1.0) != eframe::egui::Rect::NOTHING);
    assert!(vide.is_blank(), "aucune tuile n'est créée pour du transparent");
}

#[test]
fn a_pixel_grab_moves_the_centre_with_the_cursor_and_keeps_pixels_beyond_the_radius_plus_the_shift_identical() {
    let r0 = raster_avec_ligne();
    let mut r = r0.clone();
    let centre = pos2(50.0, 50.0);
    let (rayon, delta) = (20.0, vec2(8.0, 0.0));
    let zone = sculpt::saisir_raster(&r0, &mut r, centre, rayon, Attenuation::Douce, 1.0, delta);
    assert!(zone.contains(centre) && zone.contains(pos2(58.0, 50.0)));
    for y in 0..100 {
        for x in 0..100 {
            let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
            if p.distance(centre) >= rayon + delta.length() + 1.0 {
                assert_eq!(r.get(x, y), r0.get(x, y), "({x}, {y}) est loin du cercle");
            }
        }
    }
    // Le centre du trait (x = 50) est tiré de 8 px, comme un point d'un trait vectoriel (poids 1 au centre).
    assert_eq!(r.get(47, 50).a(), 0, "il n'y a plus de trait à gauche");
    assert!(r.get(56, 50).a() > 200 && r.get(57, 50).a() > 200, "le trait est maintenant vers x = 58");
    // Dans le même geste, revenir à l'appui (restaurer, puis décalage nul) remet l'image comme elle était.
    sculpt::restaurer(&mut r, &r0, zone);
    assert!(memes_pixels(&r, &r0, 0, 0, 100, 100), "restaurer rend exactement l'original");
}

#[test]
fn a_big_grab_drags_the_content_far_away_without_holes_in_the_circle() {
    let mut r0 = Raster::new(200, 100);
    // Un aplat plein de 120 × 60 de rouge, et un point bleu au centre du cercle.
    for y in 20..80 {
        for x in 40..160 {
            poser_pixel(&mut r0, x, y, Color32::from_rgba_premultiplied(200, 0, 0, 255));
        }
    }
    for y in 47..54 {
        for x in 97..104 {
            poser_pixel(&mut r0, x, y, Color32::from_rgba_premultiplied(0, 0, 255, 255));
        }
    }
    let mut r = r0.clone();
    sculpt::saisir_raster(&r0, &mut r, pos2(100.5, 50.5), 25.0, Attenuation::Douce, 1.0, vec2(60.0, 0.0));
    // La tache bleue a suivi le curseur (au-delà du rayon : un décalage plus grand que lui replie la feuille de caoutchouc,
    // comme les points d'un trait) ; l'aplat n'a pas de trou (étiré, il reste plein).
    let bleu = (150..=170).flat_map(|x| (45..=55).map(move |y| (x, y))).filter(|&(x, y)| r.get(x, y).b() > 150).count();
    assert!(bleu > 0, "la tache bleue est arrivée près du curseur");
    assert!(r.get(100, 50).a() > 0, "l'aplat tiré laisse de la matière là où le point est parti");
    assert!(r.get(130, 50).a() > 0);
}

/// Une feuille de 1920 × 1080 pleine de traits (un quadrillage de lignes de 3 px tous les 20 px).
fn feuille_quadrillee() -> Raster {
    let mut r = Raster::new(1920, 1080);
    for y in 0..1080 {
        for x in 0..1920 {
            if x % 20 < 3 || y % 20 < 3 {
                poser_pixel(&mut r, x, y, Color32::from_rgba_premultiplied(30, 30, 30, 255));
            }
        }
    }
    r
}

/// Ce que coûte un pas de Pull et un pas de Grab sur les pixels, selon le rayon (sur le fil de l'interface, une fois par image
/// d'écran) : `cargo test --release --offline --target-dir target\msvc a_pixel_step_costs -- --ignored --nocapture`
#[test]
#[ignore]
fn a_pixel_step_costs_milliseconds_not_a_replay_at_any_radius() {
    use std::time::Instant;
    let r0 = feuille_quadrillee();
    println!("  rayon | Pull : ms par pas (décalage 8 px) | Grab : ms par pas (décalage 120 px, restaurer compris)");
    for rayon in [30.0f32, 60.0, 120.0, 250.0, 400.0] {
        let mut r = r0.clone();
        let t = Instant::now();
        let n = 20;
        for k in 0..n {
            sculpt::pousser(&mut r, pos2(900.0 + k as f32 * 8.0, 500.0), vec2(8.0, 0.0), rayon, Attenuation::Douce, 1.0);
        }
        let pull = t.elapsed().as_secs_f64() * 1e3 / n as f64;
        let mut r = r0.clone();
        let mut zone = eframe::egui::Rect::NOTHING;
        let t = Instant::now();
        for k in 1..=n {
            sculpt::restaurer(&mut r, &r0, zone);
            zone = sculpt::saisir_raster(&r0, &mut r, pos2(900.0, 500.0), rayon, Attenuation::Douce, 1.0, vec2(6.0 * k as f32, 0.0));
        }
        let grab = t.elapsed().as_secs_f64() * 1e3 / n as f64;
        println!("  {rayon:>5} | {pull:>8.2} ms | {grab:>8.2} ms");
        assert!(pull < 60.0 && grab < 60.0, "rayon {rayon} : Pull {pull:.1} ms, Grab {grab:.1} ms");
    }
}

// ---------------------------------------------------------------------------------------------- dans l'application

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn bouton(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Un glisser du pointeur sur les points de la feuille `pts`, une image d'écran par point.
fn glisser(b: &mut Banc, pts: &[(f32, f32)]) {
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true)], 1.0 / 60.0);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    }
    let fin = ecran(b, pts[pts.len() - 1].0, pts[pts.len() - 1].1);
    b.image(vec![bouton(fin, false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un plan 400 × 300 de quatre images, un calque de cette sorte, l'outil donné.
fn banc(kind: LayerKind, tool: Tool) -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", kind)];
    doc.panels[0].length = 4;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.set_tool(tool);
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

/// Donne à l'image `frame` un dessin vectoriel fait de ces éléments, et rend son numéro.
fn dessin_vectoriel(b: &mut Banc, frame: usize, elements: Vec<Element>) -> CelId {
    let id = b.app.doc.new_cel(LayerKind::Vector);
    let cel = b.app.doc.cels.get_mut(&id).unwrap();
    cel.vector.as_mut().unwrap().elements = elements;
    cel.rerender();
    b.app.doc.panels[0].layers[0].set(frame, Some(id));
    id
}

/// Donne à l'image `frame` un dessin bitmap, et rend son numéro.
fn dessin_bitmap(b: &mut Banc, frame: usize, raster: Raster) -> CelId {
    let id = b.app.doc.new_cel(LayerKind::Bitmap);
    b.app.doc.cels.get_mut(&id).unwrap().raster = raster;
    b.app.doc.panels[0].layers[0].set(frame, Some(id));
    id
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

fn dessin(b: &Banc, cel: CelId) -> VectorDrawing {
    b.app.doc.cels[&cel].vector.clone().unwrap()
}

fn grand_raster() -> Raster {
    let mut r = Raster::new(400, 300);
    for y in 40..260 {
        for x in 98..102 {
            poser_pixel(&mut r, x, y, Color32::from_rgba_premultiplied(0, 0, 0, 255));
        }
    }
    r
}

fn memes_tuiles(a: &Raster, b: &Raster) -> bool {
    a.tiles.len() == b.tiles.len() && a.tiles.iter().zip(&b.tiles).all(|(x, y)| x == y)
}

#[test]
fn the_tools_are_in_the_toolbar_next_to_the_transform_tools_with_a_name_a_description_and_an_action() {
    let groupe = crate::toolbars::TOOL_GROUPS.iter().find(|g| g.iter().any(|s| s.contains(&Tool::Transform))).expect("le groupe de Transformer");
    let rang = |t: Tool| groupe.iter().position(|s| s.contains(&t));
    assert!(rang(Tool::Grab).is_some() && rang(Tool::Grab) == rang(Tool::Pull), "Grab et Pull dans une même case");
    assert_eq!(rang(Tool::Grab), rang(Tool::Transform).map(|r| r + 1), "juste après la case de Transformer et Déformer");
    assert_eq!((Tool::Grab.label(), Tool::Pull.label()), ("Grab", "Pull"));
    assert_eq!((Tool::Grab.action(), Tool::Pull.action()), ("outil.grab", "outil.pull"));
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for t in [Tool::Grab, Tool::Pull] {
        assert!(crate::keymap::action_info(t.action()).is_some_and(|i| i.implemented), "{}", t.action());
        assert!(map.warnings.iter().all(|w| !w.contains(t.action())), "{}", t.action());
        assert!(map.keys_text(t.action()).is_empty(), "pas de touche par défaut : {}", t.action());
    }
    // Choisir l'outil par son action.
    let mut b = banc(LayerKind::Vector, Tool::Brush);
    b.app.run_action("outil.pull");
    assert_eq!(b.app.tool, Tool::Pull);
    b.app.run_action("outil.grab");
    assert_eq!(b.app.tool, Tool::Grab);
}

#[test]
fn the_brush_size_keys_change_the_radius_of_the_tool_that_is_chosen() {
    let mut b = banc(LayerKind::Vector, Tool::Grab);
    let (grab, pull, pinceau) = (b.app.sculpte.grab.rayon, b.app.sculpte.pull.rayon, b.app.brush.size);
    b.app.run_action("outil.taille_plus");
    assert!(b.app.sculpte.grab.rayon > grab, "Grab : le rayon grandit");
    assert_eq!((b.app.sculpte.pull.rayon, b.app.brush.size), (pull, pinceau), "ni Pull ni le pinceau");
    b.app.set_tool(Tool::Pull);
    b.app.run_action("outil.taille_moins");
    assert!(b.app.sculpte.pull.rayon < pull, "Pull : le rayon rétrécit");
    for _ in 0..80 {
        b.app.run_action("outil.taille_plus");
    }
    assert_eq!(b.app.sculpte.pull.rayon, Reglages::RAYON_MAX, "le rayon a un plafond");
}

#[test]
fn grab_on_a_vector_layer_moves_the_points_in_the_radius_only_in_one_undo_step_without_replaying_the_engine() {
    let mut b = banc(LayerKind::Vector, Tool::Grab);
    let cel = dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0), ligne_verticale(300.0, 20.0, 180.0, 4.0)]);
    b.chauffer(3);
    b.app.sculpte.grab = Reglages { rayon: 40.0, force: 1.0, attenuation: Attenuation::Lineaire, ..Reglages::grab() };
    let avant = dessin(&b, cel);
    let n = etapes(&b);
    let replays = crate::vector::replay_count();
    glisser(&mut b, &[(100.0, 100.0), (110.0, 100.0), (120.0, 100.0), (130.0, 100.0)]);
    assert_eq!(crate::vector::replay_count(), replays, "aucun rejeu du moteur sur le fil de l'interface, geste et lâcher compris");
    let apres = dessin(&b, cel);
    let (pa, pb) = (points_de(&avant.elements[0]), points_de(&apres.elements[0]));
    assert_eq!(pa.len(), pb.len(), "les écarts de 4 px sont courts : aucun point de plus");
    for (a, p) in pa.iter().zip(&pb) {
        let dist = (a.y - 100.0).abs();
        if dist >= 40.0 {
            assert_eq!(p.x, a.x, "y = {} est hors du rayon", a.y);
        } else {
            let w = Attenuation::Lineaire.weight(dist / 40.0);
            assert!((p.x - (a.x + 30.0 * w)).abs() < 0.6, "y = {} : {} contre {}", a.y, p.x, a.x + 30.0 * w);
        }
        assert_eq!(p.r, a.r);
    }
    assert_eq!(apres.elements[1], avant.elements[1], "l'autre trait n'a pas bougé");
    assert_eq!(etapes(&b), n + 1, "une seule étape d'annulation");
    assert_eq!(b.app.history.names().0.last().copied(), Some("Grab"));
    assert!(b.app.rejeu.is_pending(cel) || !dessin(&b, cel).elements[0].cache_missing(), "le rendu exact est demandé au fil de travail");
    // Annuler rend exactement le dessin d'avant ; rétablir, celui d'après.
    b.app.undo();
    assert_eq!(dessin(&b, cel), avant, "annulation exacte");
    b.app.redo();
    assert_eq!(dessin(&b, cel), apres);
}

#[test]
fn grab_shows_the_moved_pixels_during_the_gesture_before_anything_is_replayed() {
    let mut b = banc(LayerKind::Vector, Tool::Grab);
    let cel = dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]);
    b.chauffer(3);
    b.app.sculpte.grab = Reglages { rayon: 40.0, force: 1.0, ..Reglages::grab() };
    assert!(b.app.doc.cels[&cel].raster.get(100, 100).a() > 0);
    let p0 = ecran(&b, 100.0, 100.0);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true)], 1.0 / 60.0);
    let replays = crate::vector::replay_count();
    let p1 = ecran(&b, 130.0, 100.0);
    b.image(vec![Event::PointerMoved(p1)], 1.0 / 60.0);
    // En plein geste : l'image du dessin montre le trait tiré, le document n'a pas encore sa nouvelle géométrie.
    let raster = &b.app.doc.cels[&cel].raster;
    assert_eq!(raster.get(100, 100).a(), 0, "le trait a quitté sa place au centre");
    assert!(raster.get(129, 100).a() > 0 || raster.get(130, 100).a() > 0, "il est maintenant près du curseur");
    assert_eq!(points_de(&dessin(&b, cel).elements[0])[20].x, 100.0, "la géométrie du document ne change qu'au lâcher");
    assert_eq!(crate::vector::replay_count(), replays, "aucun rejeu pendant le geste");
    assert!(b.app.sculpt_occupe(cel), "le dessin est occupé : un rendu exact qui arrive attend la fin");
    b.image(vec![bouton(p1, false)], 1.0 / 60.0);
    assert!(!b.app.sculpt_occupe(cel));
    assert_eq!(crate::vector::replay_count(), replays, "ni au lâcher");
}

#[test]
fn pull_on_a_vector_layer_stretches_the_line_along_the_motion_in_one_undo_step() {
    let mut b = banc(LayerKind::Vector, Tool::Pull);
    let cel = dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]);
    b.chauffer(3);
    b.app.sculpte.pull = Reglages { rayon: 30.0, force: 1.0, ..Reglages::pull() };
    let avant = dessin(&b, cel);
    let n = etapes(&b);
    let replays = crate::vector::replay_count();
    glisser(&mut b, &[(100.0, 100.0), (110.0, 100.0), (120.0, 100.0), (130.0, 100.0), (140.0, 100.0), (150.0, 100.0)]);
    assert_eq!(crate::vector::replay_count(), replays);
    let pts = points_de(&dessin(&b, cel).elements[0]);
    let plus_loin = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    assert!((plus_loin - 150.0).abs() < 1.5, "le point tiré suit le pinceau : x = {plus_loin}");
    assert!(pts.iter().filter(|p| (p.y - 100.0).abs() > 70.0).all(|p| p.x == 100.0), "loin du pinceau, rien ne bouge");
    assert_eq!(etapes(&b), n + 1);
    assert_eq!(b.app.history.names().0.last().copied(), Some("Pull"));
    b.app.undo();
    assert_eq!(dessin(&b, cel), avant);
}

#[test]
fn grab_and_pull_on_a_bitmap_layer_warp_the_pixels_of_the_radius_and_undo_restores_the_tiles_exactly() {
    for tool in [Tool::Grab, Tool::Pull] {
        let mut b = banc(LayerKind::Bitmap, tool);
        let cel = dessin_bitmap(&mut b, 0, grand_raster());
        b.chauffer(3);
        let reglages = Reglages { rayon: 40.0, force: 1.0, ..Reglages::grab() };
        *b.app.sculpte.de_mut(tool) = reglages;
        let avant = b.app.doc.cels[&cel].raster.clone();
        let n = etapes(&b);
        glisser(&mut b, &[(100.0, 150.0), (110.0, 150.0), (120.0, 150.0), (130.0, 150.0)]);
        let apres = b.app.doc.cels[&cel].raster.clone();
        assert_eq!(etapes(&b), n + 1, "{tool:?} : une étape");
        // Le trait vertical (x = 98..102) a été tiré vers la droite au centre du cercle (y = 150)…
        assert_eq!(apres.get(99, 150).a(), 0, "{tool:?} : le trait a quitté sa place");
        let a_droite = (100..200).filter(|&x| apres.get(x, 150).a() > 128).max().unwrap_or(0);
        assert!((124..=136).contains(&a_droite), "{tool:?} : le trait est maintenant vers x = 130 (son bord droit : {a_droite})");
        // … et tout ce qui est hors du cercle (et du chemin) n'a pas bougé d'un pixel.
        for y in 0..300 {
            for x in 0..400 {
                let d = pos2(x as f32 + 0.5, y as f32 + 0.5).distance(pos2(100.0, 150.0));
                let loin = if tool == Tool::Grab { d >= 40.0 + 30.0 + 1.0 } else { pos2(x as f32 + 0.5, y as f32 + 0.5).distance(pos2(115.0, 150.0)) >= 40.0 + 15.0 + 1.0 };
                if loin {
                    assert_eq!(apres.get(x, y), avant.get(x, y), "{tool:?} : ({x}, {y}) est hors du cercle");
                }
            }
        }
        b.app.undo();
        assert!(memes_tuiles(&b.app.doc.cels[&cel].raster, &avant), "{tool:?} : annulation exacte");
        b.app.redo();
        assert!(memes_tuiles(&b.app.doc.cels[&cel].raster, &apres), "{tool:?} : rétablir rend la déformation");
    }
}

#[test]
fn a_zero_strength_gesture_leaves_the_drawing_untouched_and_records_no_step() {
    for (kind, tool) in [(LayerKind::Vector, Tool::Grab), (LayerKind::Vector, Tool::Pull), (LayerKind::Bitmap, Tool::Grab), (LayerKind::Bitmap, Tool::Pull)] {
        let mut b = banc(kind, tool);
        let cel = if kind == LayerKind::Vector { dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]) } else { dessin_bitmap(&mut b, 0, grand_raster()) };
        b.chauffer(3);
        b.app.sculpte.de_mut(tool).force = 0.0;
        let (raster, vect) = (b.app.doc.cels[&cel].raster.clone(), b.app.doc.cels[&cel].vector.clone());
        let n = etapes(&b);
        glisser(&mut b, &[(100.0, 100.0), (120.0, 100.0), (150.0, 100.0)]);
        assert_eq!(etapes(&b), n, "{kind:?} {tool:?} : aucune étape");
        assert_eq!(b.app.doc.cels[&cel].vector, vect, "{kind:?} {tool:?}");
        assert!(memes_tuiles(&b.app.doc.cels[&cel].raster, &raster), "{kind:?} {tool:?} : l'image n'a pas changé");
    }
}

/// Un dessin vectoriel à quatre images, dont trois (0, 1, 2) ont leur propre dessin, et leurs trois clés choisies.
fn banc_trois_cles() -> (Banc, [CelId; 3]) {
    let mut b = banc(LayerKind::Vector, Tool::Grab);
    let cels = [0usize, 1, 2].map(|k| dessin_vectoriel(&mut b, k, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0), ligne_verticale(250.0 + k as f32 * 10.0, 20.0, 180.0, 4.0)]));
    b.chauffer(3);
    b.app.sculpte.grab = Reglages { rayon: 40.0, force: 1.0, attenuation: Attenuation::Douce, ..Reglages::grab() };
    (b, cels)
}

fn choisir_les_cles(b: &mut Banc, frames: &[usize]) {
    let panel = b.app.doc.panels[0].id;
    let layer = b.app.doc.panels[0].layers[0].id;
    b.app.key_sel = frames.iter().map(|&start| KeyRef { panel, layer, start }).collect();
}

#[test]
fn with_several_keys_chosen_the_same_deformation_is_applied_to_every_one_and_undone_in_one_step() {
    let (mut b, cels) = banc_trois_cles();
    choisir_les_cles(&mut b, &[0, 1, 2]);
    let avant: Vec<VectorDrawing> = cels.iter().map(|&c| dessin(&b, c)).collect();
    let n = etapes(&b);
    glisser(&mut b, &[(100.0, 100.0), (115.0, 100.0), (130.0, 100.0)]);
    let apres: Vec<VectorDrawing> = cels.iter().map(|&c| dessin(&b, c)).collect();
    for (k, (a, p)) in avant.iter().zip(&apres).enumerate() {
        assert_ne!(a.elements[0], p.elements[0], "la clé {k} a été déformée");
        assert_eq!(a.elements[1], p.elements[1], "son autre trait est loin du cercle");
        // La même déformation partout : mêmes points à la même place.
        assert_eq!(p.elements[0], apres[0].elements[0], "la clé {k} est déformée comme la première");
    }
    let centre = points_de(&apres[2].elements[0]).into_iter().find(|p| p.y == 100.0).unwrap();
    assert!((centre.x - 130.0).abs() < 0.6, "le centre a suivi le curseur : {}", centre.x);
    assert_eq!(etapes(&b), n + 1, "une seule étape pour les trois dessins");
    b.app.undo();
    for (k, &c) in cels.iter().enumerate() {
        assert_eq!(dessin(&b, c), avant[k], "annulation exacte de la clé {k}");
    }
    assert_eq!(etapes(&b), n);
    b.app.redo();
    for (k, &c) in cels.iter().enumerate() {
        assert_eq!(dessin(&b, c), apres[k], "rétablir la clé {k}");
    }
}

#[test]
fn with_one_key_chosen_or_none_only_the_current_drawing_is_deformed() {
    for choix in [vec![], vec![1]] {
        let (mut b, cels) = banc_trois_cles();
        choisir_les_cles(&mut b, &choix);
        let avant: Vec<VectorDrawing> = cels.iter().map(|&c| dessin(&b, c)).collect();
        glisser(&mut b, &[(100.0, 100.0), (130.0, 100.0)]);
        assert_ne!(dessin(&b, cels[0]), avant[0], "le dessin qu'on voit est déformé");
        assert_eq!(dessin(&b, cels[1]), avant[1], "{choix:?} : les autres non");
        assert_eq!(dessin(&b, cels[2]), avant[2], "{choix:?} : les autres non");
    }
}

#[test]
fn chosen_keys_on_another_layer_do_not_count() {
    let (mut b, cels) = banc_trois_cles();
    let panel = b.app.doc.panels[0].id;
    b.app.key_sel = (0..3).map(|start| KeyRef { panel, layer: u64::MAX, start }).collect();
    let avant: Vec<VectorDrawing> = cels.iter().map(|&c| dessin(&b, c)).collect();
    glisser(&mut b, &[(100.0, 100.0), (130.0, 100.0)]);
    assert_ne!(dessin(&b, cels[0]), avant[0]);
    assert_eq!(dessin(&b, cels[1]), avant[1]);
}

#[test]
fn the_aplat_follows_the_same_deformation_and_its_undo_is_part_of_the_same_step() {
    for kind in [LayerKind::Bitmap, LayerKind::Vector] {
        let mut b = banc(kind, Tool::Grab);
        let cel = if kind == LayerKind::Vector { dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]) } else { dessin_bitmap(&mut b, 0, grand_raster()) };
        b.app.doc.panels[0].layers[0].fond = Some(FondReglages::default());
        // Ce qu'on a peint dans l'aplat : une barre verticale rouge à côté du trait, et une coupe.
        {
            let c = b.app.doc.cels.get_mut(&cel).unwrap();
            let (w, h) = (c.raster.width, c.raster.height);
            let mut s = Strate::new(w, h);
            for y in 40..260 {
                for x in 103..109 {
                    poser_pixel(&mut s.peinture.raster, x, y, Color32::from_rgba_premultiplied(255, 0, 0, 255));
                }
                for x in 111..113 {
                    poser_pixel(&mut s.coupe, x, y, Color32::from_rgba_premultiplied(0, 0, 0, 255));
                }
            }
            c.fond = Some(Box::new(s));
        }
        b.chauffer(3);
        b.app.sculpte.grab = Reglages { rayon: 40.0, force: 1.0, ..Reglages::grab() };
        let (peinture0, coupe0) = {
            let s = b.app.doc.cels[&cel].fond.as_ref().unwrap();
            (s.peinture.raster.clone(), s.coupe.clone())
        };
        let n = etapes(&b);
        glisser(&mut b, &[(100.0, 150.0), (115.0, 150.0), (130.0, 150.0)]);
        assert_eq!(etapes(&b), n + 1, "{kind:?} : une seule étape pour le dessin et l'aplat");
        let (peinture1, coupe1) = {
            let s = b.app.doc.cels[&cel].fond.as_ref().unwrap();
            (s.peinture.raster.clone(), s.coupe.clone())
        };
        // La barre rouge a été tirée vers la droite au centre du cercle ; loin du cercle, elle n'a pas bougé.
        assert_eq!(peinture1.get(104, 150).a(), 0, "{kind:?} : la peinture de l'aplat a quitté sa place");
        let a_droite = (100..220).filter(|&x| peinture1.get(x, 150).r() > 100).max().unwrap_or(0);
        assert!((132..=144).contains(&a_droite), "{kind:?} : elle suit le curseur (son bord droit : {a_droite}, avant 108)");
        assert_eq!(peinture1.get(105, 60), peinture0.get(105, 60), "{kind:?} : loin du cercle, rien");
        assert_ne!(coupe1.get(112, 150), coupe0.get(112, 150), "{kind:?} : la coupe suit aussi");
        assert_eq!(coupe1.get(112, 60), coupe0.get(112, 60));
        b.app.undo();
        let s = b.app.doc.cels[&cel].fond.as_ref().unwrap();
        assert!(memes_tuiles(&s.peinture.raster, &peinture0), "{kind:?} : la peinture revient exactement");
        assert!(memes_tuiles(&s.coupe, &coupe0), "{kind:?} : la coupe aussi");
        b.app.redo();
        let s = b.app.doc.cels[&cel].fond.as_ref().unwrap();
        assert!(memes_tuiles(&s.peinture.raster, &peinture1), "{kind:?} : rétablir");
    }
}

#[test]
fn with_the_aplat_as_target_only_the_aplat_is_deformed() {
    let mut b = banc(LayerKind::Bitmap, Tool::Grab);
    let cel = dessin_bitmap(&mut b, 0, grand_raster());
    b.app.doc.panels[0].layers[0].fond = Some(FondReglages::default());
    {
        let c = b.app.doc.cels.get_mut(&cel).unwrap();
        let mut s = Strate::new(c.raster.width, c.raster.height);
        for y in 40..260 {
            for x in 103..109 {
                poser_pixel(&mut s.peinture.raster, x, y, Color32::from_rgba_premultiplied(255, 0, 0, 255));
            }
        }
        c.fond = Some(Box::new(s));
    }
    b.app.aplat_cible = Some(b.app.doc.panels[0].layers[0].id);
    b.chauffer(3);
    assert!(b.app.aplat_en_cible());
    b.app.sculpte.grab = Reglages { rayon: 40.0, force: 1.0, ..Reglages::grab() };
    let trait0 = b.app.doc.cels[&cel].raster.clone();
    glisser(&mut b, &[(100.0, 150.0), (130.0, 150.0)]);
    assert!(memes_tuiles(&b.app.doc.cels[&cel].raster, &trait0), "le trait n'a pas bougé");
    assert_eq!(b.app.doc.cels[&cel].fond.as_ref().unwrap().peinture.raster.get(104, 150).a(), 0, "l'aplat, si");
}

#[test]
fn a_locked_layer_or_an_empty_image_is_refused_without_a_gesture() {
    let mut b = banc(LayerKind::Vector, Tool::Grab);
    // Pas de dessin à cette image : rien à déformer.
    glisser(&mut b, &[(100.0, 100.0), (130.0, 100.0)]);
    assert!(b.app.sculpt_geste.is_none());
    assert_eq!(etapes(&b), 0);
    let cel = dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]);
    b.chauffer(2);
    b.app.layers_mut()[0].locked = true;
    let avant = dessin(&b, cel);
    glisser(&mut b, &[(100.0, 100.0), (130.0, 100.0)]);
    assert_eq!(dessin(&b, cel), avant, "calque verrouillé");
    assert!(b.app.sculpt_geste.is_none());
}

#[test]
fn changing_the_tool_during_a_gesture_finishes_it_in_a_single_step() {
    let mut b = banc(LayerKind::Vector, Tool::Pull);
    let cel = dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]);
    b.chauffer(3);
    b.app.sculpte.pull = Reglages { rayon: 30.0, force: 1.0, ..Reglages::pull() };
    let n = etapes(&b);
    let p0 = ecran(&b, 100.0, 100.0);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true)], 1.0 / 60.0);
    let p1 = ecran(&b, 120.0, 100.0);
    b.image(vec![Event::PointerMoved(p1)], 1.0 / 60.0);
    assert!(b.app.sculpt_geste.is_some());
    b.app.set_tool(Tool::Brush);
    assert!(b.app.sculpt_geste.is_none());
    assert_eq!(etapes(&b), n + 1);
    assert!(points_de(&dessin(&b, cel).elements[0]).iter().any(|p| p.x > 110.0));
}

#[test]
fn the_settings_panel_for_both_tools_draws_without_a_panic() {
    for tool in [Tool::Grab, Tool::Pull] {
        let mut b = banc(LayerKind::Vector, tool);
        b.app.show_pane(crate::dock::Pane::ToolOptions);
        b.garder_textes = true;
        b.chauffer(3);
        let textes = b.textes.join("|");
        assert!(textes.contains("Atténuation") && textes.contains("Rayon (px)") && textes.contains("Force"), "{tool:?} : {textes}");
        assert!(textes.contains(tool.label()), "{tool:?} : le titre du panneau");
    }
}

#[test]
fn the_brush_circle_is_drawn_with_the_cursor_on_and_not_with_it_off() {
    let mut b = banc(LayerKind::Vector, Tool::Grab);
    let _ = dessin_vectoriel(&mut b, 0, vec![ligne_verticale(100.0, 20.0, 180.0, 4.0)]);
    b.garder_formes = true;
    let p = ecran(&b, 200.0, 150.0);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let cercles = |b: &Banc| b.formes.iter().filter(|f| matches!(&f.shape, eframe::egui::epaint::Shape::Circle(c) if c.center.distance(p) < 0.5 && c.radius > 20.0)).count();
    assert!(cercles(&b) >= 1, "le cercle du pinceau est tracé");
    assert_ne!(b.curseur, eframe::egui::CursorIcon::Default);
    b.app.sculpte.grab.curseur = false;
    b.image(vec![Event::PointerMoved(p + vec2(1.0, 0.0))], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(cercles(&b), 0, "« Afficher le curseur » éteint : pas de cercle");
    assert_eq!(b.curseur, eframe::egui::CursorIcon::Crosshair);
}
