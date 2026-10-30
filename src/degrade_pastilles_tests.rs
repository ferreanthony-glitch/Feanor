//! Le dégradé à plusieurs pastilles (2026-10-30, `docs/INTERFACE.md` partie 80) : couleurs et milieux, opacité à part, les cinq formes,
//! angle et échelle, inverser, tramage, format 44 (et les anciens dégradés à deux couleurs, lus à l'identique), dégradés enregistrés,
//! retouche après coup en une seule étape d'annulation, figer, zone vectorielle retrouvée, rien du moteur de pinceaux.

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::degrade::{self, Degrade, Enregistre, Fin, Genre, PastilleCouleur, PastilleOpacite, Portee, Reglages};
use crate::degrade_tests::{banc_degrade, cel_du_calque, ecran, etapes, glisser, pixel, tracer_anneau, BLEU, ROUGE};
use crate::degradeui::Phase;
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::project;
use crate::raster::Raster;
use crate::vector::Element;

const VERT: [u8; 3] = [0, 255, 0];

fn p(pos: f32, rgb: [u8; 3]) -> PastilleCouleur {
    PastilleCouleur::new(pos, rgb)
}

fn o(pos: f32, v: f32) -> PastilleOpacite {
    PastilleOpacite::new(pos, v)
}

/// Un dégradé linéaire de (0, 0) à (100, 0) : rouge, vert, bleu.
fn trois() -> Degrade {
    let mut g = Degrade::simple(Genre::Lineaire, [0.0, 0.0], [100.0, 0.0], ROUGE, BLEU, 1.0, 1.0);
    g.couleurs = vec![p(0.0, ROUGE), p(0.5, VERT), p(1.0, BLEU)];
    g
}

fn proche(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

// ------------------------------------------------------------------------------------ le noyau

#[test]
fn several_colour_stops_and_their_midpoints_shape_the_colour_along_the_axis() {
    let g = trois();
    let (c, a) = g.couleur_a(0.25);
    assert!(proche(c[0], 127.5, 0.6) && proche(c[1], 127.5, 0.6) && c[2] == 0.0 && a == 1.0, "entre rouge et vert : {c:?}");
    let (c, _) = g.couleur_a(0.75);
    assert!(c[0] == 0.0 && proche(c[1], 127.5, 0.6) && proche(c[2], 127.5, 0.6), "entre vert et bleu : {c:?}");
    assert_eq!(g.couleur_a(0.5).0, [0.0, 255.0, 0.0], "pile sur la pastille du milieu");
    // Hors des pastilles, la couleur du bout.
    let mut h = trois();
    h.couleurs = vec![p(0.2, ROUGE), p(0.8, BLEU)];
    assert_eq!(h.couleur_a(0.0).0, [255.0, 0.0, 0.0]);
    assert_eq!(h.couleur_a(1.0).0, [0.0, 0.0, 255.0]);
    // Le milieu : où l'on est à 50/50. À 25 % du segment, la moitié ; ailleurs la courbe suit, dans les deux sens.
    let mut m = Degrade::simple(Genre::Lineaire, [0.0, 0.0], [100.0, 0.0], [0, 0, 0], [255, 255, 255], 1.0, 1.0);
    m.couleurs[0].milieu = 0.25;
    assert!(proche(m.couleur_a(0.25).0[0], 127.5, 1.0), "à 50/50 au milieu choisi : {:?}", m.couleur_a(0.25).0);
    assert!(m.couleur_a(0.5).0[0] > 160.0 && m.couleur_a(0.1).0[0] < 100.0, "la pente suit le milieu");
    m.couleurs[0].milieu = 0.75;
    assert!(proche(m.couleur_a(0.75).0[0], 127.5, 1.0) && m.couleur_a(0.5).0[0] < 100.0);
    // Le dégradé est lu au pixel : le même chemin que `peindre`.
    let mut r = Raster::new(120, 4);
    degrade::peindre(&mut r, &Portee::Tout, &Degrade { tramage: false, ..trois() }, false, None);
    let (a, b, c) = (r.get(25, 1), r.get(50, 1), r.get(75, 1));
    assert!(proche(a.r() as f32, 128.0, 3.0) && proche(a.g() as f32, 127.0, 3.0), "{a:?}");
    assert!(b.g() > 245 && b.r() < 6 && b.b() < 6, "{b:?}");
    assert!(proche(c.g() as f32, 128.0, 3.0) && proche(c.b() as f32, 127.0, 3.0), "{c:?}");
}

#[test]
fn the_opacity_has_its_own_stops_apart_from_the_colours() {
    let mut g = Degrade::simple(Genre::Lineaire, [0.0, 0.0], [100.0, 0.0], ROUGE, BLEU, 1.0, 1.0);
    g.opacites = vec![o(0.0, 1.0), o(0.5, 0.0), o(1.0, 1.0)];
    let (c0, a0) = g.couleur_a(0.0);
    let (c5, a5) = g.couleur_a(0.5);
    let (c1, a1) = g.couleur_a(1.0);
    assert_eq!((a0, a5, a1), (1.0, 0.0, 1.0), "transparent au milieu, opaque aux bouts");
    assert_eq!((c0, c1), ([255.0, 0.0, 0.0], [0.0, 0.0, 255.0]));
    assert!(proche(c5[0], 127.5, 0.6) && proche(c5[2], 127.5, 0.6), "la couleur ne dépend pas de l'opacité : {c5:?}");
    assert!(proche(g.couleur_a(0.25).1, 0.5, 1e-4) && proche(g.couleur_a(0.75).1, 0.5, 1e-4));
    // Écrit dans des pixels : le milieu est vide, les bouts sont pleins.
    let mut r = Raster::new(101, 3);
    degrade::peindre(&mut r, &Portee::Tout, &Degrade { tramage: false, ..g.clone() }, false, None);
    assert!(r.get(0, 1).a() >= 250 && r.get(100, 1).a() >= 250, "{:?} {:?}", r.get(0, 1), r.get(100, 1));
    assert!(r.get(50, 1).a() <= 6, "{:?}", r.get(50, 1));
    // « Vers le transparent » reste un raccourci : les deux séries sont celles du réglage d'avant.
    let v = Reglages { fin: Fin::VersTransparent, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(10.0, 0.0));
    assert_eq!((v.c0(), v.c1(), v.a0(), v.a1()), (ROUGE, ROUGE, 1.0, 0.0));
    // Ses propres pastilles d'opacité, des couleurs encore liées aux carrés ; l'opacité du réglage les multiplie.
    let r = Reglages { transparence: Some(vec![o(0.0, 0.5), o(1.0, 1.0)]), opacite: 0.5, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(10.0, 0.0));
    assert_eq!((r.c0(), r.c1()), (ROUGE, BLEU));
    assert_eq!((r.a0(), r.a1()), (0.25, 0.5));
}

#[test]
fn the_five_shapes_give_the_expected_parameter() {
    let base = |genre, a: [f32; 2], b: [f32; 2]| Degrade::simple(genre, a, b, ROUGE, BLEU, 1.0, 1.0);
    // Angle : un tour autour de l'appui, de la direction de l'axe.
    let g = base(Genre::Angle, [0.0, 0.0], [10.0, 0.0]);
    assert!(g.parametre(10.0, 0.0) < 0.01 || g.parametre(10.0, 0.0) > 0.99);
    assert!(proche(g.parametre(0.0, 10.0), 0.25, 1e-4) && proche(g.parametre(-10.0, 0.0), 0.5, 1e-4) && proche(g.parametre(0.0, -10.0), 0.75, 1e-4));
    assert!(proche(g.parametre(0.0, 200.0), 0.25, 1e-4), "ne dépend pas de la distance");
    // L'axe tourné donne la direction de départ.
    let h = base(Genre::Angle, [0.0, 0.0], [0.0, 10.0]);
    assert!(h.parametre(0.0, 10.0) < 0.01 || h.parametre(0.0, 10.0) > 0.99);
    assert!(proche(h.parametre(-10.0, 0.0), 0.25, 1e-4));
    // Réfléchi : le miroir de part et d'autre de l'appui.
    let g = base(Genre::Reflechi, [0.0, 0.0], [100.0, 0.0]);
    assert!(proche(g.parametre(50.0, 0.0), 0.5, 1e-5) && proche(g.parametre(-50.0, 0.0), 0.5, 1e-5) && proche(g.parametre(-30.0, 900.0), 0.3, 1e-5));
    assert_eq!((g.parametre(0.0, 0.0), g.parametre(100.0, 0.0), g.parametre(-400.0, 0.0)), (0.0, 1.0, 1.0));
    // Losange : le même écart sur les quatre pointes.
    let g = base(Genre::Losange, [0.0, 0.0], [100.0, 0.0]);
    for (x, y) in [(50.0, 0.0), (-50.0, 0.0), (0.0, 50.0), (0.0, -50.0), (25.0, 25.0), (-25.0, 25.0)] {
        assert!(proche(g.parametre(x, y), 0.5, 1e-5), "({x}, {y}) : {}", g.parametre(x, y));
    }
    assert_eq!((g.parametre(100.0, 0.0), g.parametre(0.0, 100.0), g.parametre(80.0, 80.0)), (1.0, 1.0, 1.0));
    // Un axe de longueur nulle ne casse rien.
    for genre in Genre::TOUS {
        let g = base(genre, [5.0, 5.0], [5.0, 5.0]);
        assert_eq!(g.parametre(9.0, 3.0), 0.0, "{genre:?}");
    }
    // Chacune s'écrit dans des pixels.
    for genre in Genre::TOUS {
        let g = Degrade { tramage: false, ..base(genre, [100.0, 100.0], [180.0, 100.0]) };
        let mut r = Raster::new(200, 200);
        degrade::peindre(&mut r, &Portee::Tout, &g, false, None);
        let (c, d) = (r.get(100, 100), r.get(190, 100));
        assert_eq!(c.a(), 255, "{genre:?}");
        assert!(genre == Genre::Angle || c.r() > 235, "{genre:?} : le centre est rouge {c:?}");
        assert!(genre == Genre::Angle || d.b() > 235, "{genre:?} : l'arrivée est bleue {d:?}");
    }
}

#[test]
fn the_angle_and_the_scale_typed_in_figures_move_the_end_of_the_axis() {
    let g = Degrade::simple(Genre::Lineaire, [10.0, 10.0], [10.0, 60.0], ROUGE, BLEU, 1.0, 1.0);
    assert!(proche(g.angle_degres(), 90.0, 1e-3) && proche(g.longueur(), 50.0, 1e-3));
    for a in [-170.0f32, -90.0, -45.0, 0.0, 30.0, 90.0, 179.0] {
        let fin = crate::degradeui::axe_depuis(pos2(10.0, 10.0), a, 80.0);
        let g = Degrade::simple(Genre::Lineaire, [10.0, 10.0], [fin.x, fin.y], ROUGE, BLEU, 1.0, 1.0);
        assert!(proche(g.angle_degres(), a, 1e-2) && proche(g.longueur(), 80.0, 1e-3), "{a} : {} {}", g.angle_degres(), g.longueur());
    }
    assert!(proche(Degrade::simple(Genre::Lineaire, [0.0; 2], [-10.0, 0.0], ROUGE, BLEU, 1.0, 1.0).angle_degres(), 180.0, 1e-3), "180 et jamais -180");
}

#[test]
fn reversing_turns_every_stop_the_other_way_round() {
    let mut g = trois();
    g.couleurs[0].milieu = 0.25;
    g.couleurs[1].milieu = 0.7;
    g.opacites = vec![o(0.0, 1.0), o(0.3, 0.2), o(1.0, 0.6)];
    g.opacites[0].milieu = 0.4;
    let r = g.reversed();
    assert_eq!((r.c0(), r.c1(), r.a0(), r.a1()), (BLEU, ROUGE, 0.6, 1.0));
    for k in 0..=40 {
        let t = k as f32 / 40.0;
        let (a, b) = (g.couleur_a(t), r.couleur_a(1.0 - t));
        assert!(proche(a.0[0], b.0[0], 0.6) && proche(a.0[1], b.0[1], 0.6) && proche(a.0[2], b.0[2], 0.6) && proche(a.1, b.1, 1e-3), "t = {t} : {a:?} / {b:?}");
    }
    let rr = r.reversed();
    for k in 0..=40 {
        let t = k as f32 / 40.0;
        let (a, b) = (g.couleur_a(t), rr.couleur_a(t));
        assert!(proche(a.0[0], b.0[0], 0.01) && proche(a.0[1], b.0[1], 0.01) && proche(a.1, b.1, 1e-4), "deux fois : rien n'a bougé (t = {t})");
    }
    // Dans les réglages : liés, l'inversion échange les bouts (comme avant).
    let i = Reglages { inverse: true, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(1.0, 0.0));
    assert_eq!((i.c0(), i.c1()), (BLEU, ROUGE));
}

#[test]
fn the_dither_is_deterministic_bounded_and_off_means_exactly_as_before() {
    let mut g = Degrade::simple(Genre::Lineaire, [0.0, 0.0], [200.0, 0.0], [100, 100, 100], [101, 101, 101], 1.0, 1.0);
    g.tramage = true;
    let mut ecart_max = 0i32;
    let mut valeurs = std::collections::BTreeSet::new();
    for y in 0..64usize {
        for x in 0..200usize {
            let t = g.pixel_en(x, y, 1.0);
            assert_eq!(t, g.pixel_en(x, y, 1.0), "le même pixel donne la même valeur à chaque rendu");
            let s = Degrade { tramage: false, ..g.clone() }.pixel_en(x, y, 1.0);
            assert_eq!(s, Degrade { tramage: false, ..g.clone() }.pixel(x as f32 + 0.5, y as f32 + 0.5, 1.0), "sans tramage : la formule d'avant");
            ecart_max = ecart_max.max((t.r() as i32 - s.r() as i32).abs()).max((t.a() as i32 - s.a() as i32).abs());
            assert!(t.r() <= t.a() && t.g() <= t.a() && t.b() <= t.a(), "un pixel prémultiplié valide : {t:?}");
            if (90..110).contains(&x) {
                valeurs.insert(t.r());
            }
        }
    }
    assert!(ecart_max <= 1, "moins d'un niveau d'écart : {ecart_max}");
    assert_eq!(valeurs.into_iter().collect::<Vec<_>>(), vec![100, 101], "dans la pente, les deux niveaux voisins se mêlent (plus de marche nette)");
    for (x, y) in [(0i64, 0i64), (3, 7), (-5, 12), (1000, 1000), (4095, 2159), (-300, -300)] {
        let s = degrade::seuil(x, y);
        assert!((-0.5..0.5).contains(&s), "({x}, {y}) : {s}");
    }
    // Écrit dans des pixels : deux rendus identiques, et la zone hors dégradé reste unie.
    let mut a = Raster::new(200, 8);
    let mut b = Raster::new(200, 8);
    degrade::peindre(&mut a, &Portee::Tout, &g, false, None);
    degrade::peindre(&mut b, &Portee::Tout, &g, false, None);
    assert!(a.tiles == b.tiles);
    // Vectoriel : le rendu de la zone est le même à chaque fois.
    let zone = Element::Fill { contours: vec![vec![[0.0, 0.0], [200.0, 0.0], [200.0, 8.0], [0.0, 8.0]]], color: ROUGE, opacity: 1.0, gradient: Some(g.clone()) };
    let (mut v1, mut v2) = (Raster::new(200, 8), Raster::new(200, 8));
    crate::vector::render_element(&mut v1, &zone);
    crate::vector::render_element(&mut v2, &zone);
    assert!(v1.tiles == v2.tiles);
    let nets = (0..200).filter(|&x| v1.get(x, 3).r() != v1.get(x, 4).r()).count();
    assert!(nets > 0, "le tramage s'écrit aussi dans le rendu vectoriel");
}

#[test]
fn absurd_stops_do_not_pass_the_check() {
    let bon = trois();
    assert!(bon.is_sane());
    let mut g = bon.clone();
    g.couleurs[1].pos = 1.5;
    assert!(!g.is_sane(), "position hors de 0 à 1");
    let mut g = bon.clone();
    g.couleurs = vec![p(0.8, ROUGE), p(0.2, BLEU)];
    assert!(!g.is_sane(), "pastilles dans le désordre");
    let mut g = bon.clone();
    g.couleurs[0].milieu = 0.0;
    assert!(!g.is_sane(), "milieu au bord");
    let mut g = bon.clone();
    g.opacites[0].opacite = 2.0;
    assert!(!g.is_sane(), "opacité de 200 %");
    let mut g = bon.clone();
    g.couleurs.truncate(1);
    assert!(!g.is_sane(), "une seule pastille");
    let mut g = bon.clone();
    g.couleurs = (0..=degrade::MAX_PASTILLES).map(|i| p(i as f32 / 40.0, ROUGE)).collect();
    assert!(!g.is_sane(), "trop de pastilles");
    let mut g = bon;
    g.opacites[1].pos = f32::NAN;
    assert!(!g.is_sane());
}

#[test]
fn two_gradients_with_different_stops_still_mix_between_two_plans() {
    let a = trois();
    let mut b = trois();
    b.depart = [20.0, 0.0];
    b.couleurs = vec![p(0.0, BLEU), p(0.25, ROUGE), p(0.5, VERT), p(1.0, ROUGE)];
    let milieu = Degrade::entre(&a, &b, 0.5);
    assert!(milieu.is_sane() && milieu.couleurs.len() >= 4);
    assert_eq!(milieu.depart, [10.0, 0.0]);
    let (x, y) = (a.couleur_a(0.5).0, b.couleur_a(0.5).0);
    let m = milieu.couleur_a(0.5).0;
    for k in 0..3 {
        assert!(proche(m[k], (x[k] + y[k]) / 2.0, 1.5), "{m:?} / {x:?} {y:?}");
    }
    let meme = Degrade::entre(&a, &a, 0.3);
    assert_eq!(meme.couleurs.len(), 3);
    assert_eq!(Degrade::entre(&a, &a, 0.0).couleurs, a.couleurs);
}

// ------------------------------------------------------------------------------------ le format

/// Le fichier d'un dessin du dossier du projet qui contient `"degrade"`.
fn fichier_du_degrade(dir: &std::path::Path) -> std::path::PathBuf {
    let mut pile = vec![dir.to_path_buf()];
    while let Some(d) = pile.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                pile.push(e.path());
            } else if e.path().extension().is_some_and(|x| x == "json") && std::fs::read_to_string(e.path()).unwrap().contains("\"degrade\"") {
                return e.path();
            }
        }
    }
    panic!("aucun fichier de dessin ne porte un dégradé");
}

fn projet_a_une_zone(nom: &str, g: &Degrade) -> (std::path::PathBuf, std::path::PathBuf, Document) {
    let dir = std::env::temp_dir().join(format!("shot-pastilles-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.shot");
    let mut doc = Document::new(200, 120, 24);
    doc.panels[0].layers = vec![Layer::new("Vecto", LayerKind::Vector)];
    doc.panels[0].length = 2;
    let cel = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames = vec![Some(cel); 2];
    let zone = Element::Fill { contours: vec![vec![[10.0, 10.0], [190.0, 10.0], [190.0, 110.0], [10.0, 110.0]]], color: ROUGE, opacity: 1.0, gradient: Some(g.clone()) };
    {
        let c = doc.cels.get_mut(&cel).unwrap();
        c.vector.as_mut().unwrap().elements = vec![zone];
        c.rerender();
    }
    (dir, manifest, doc)
}

#[test]
fn the_vector_zone_keeps_every_stop_midpoint_shape_and_dither_through_the_file() {
    let mut g = Degrade { genre: Genre::Losange, depart: [100.0, 60.0], arrivee: [160.0, 60.0], tramage: true, ..trois() };
    g.couleurs = vec![p(0.0, ROUGE), PastilleCouleur { pos: 0.4, rgb: VERT, milieu: 0.3 }, p(0.7, [250, 250, 20]), p(1.0, BLEU)];
    g.opacites = vec![o(0.0, 1.0), PastilleOpacite { pos: 0.6, opacite: 0.35, milieu: 0.8 }, o(1.0, 0.9)];
    assert!(g.is_sane());
    let (dir, manifest, doc) = projet_a_une_zone("aller-retour", &g);
    let cel = doc.panels[0].layers[0].frames[0].unwrap();
    let rendu = doc.cels[&cel].raster.to_flat();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(fichier_du_degrade(&dir)).unwrap();
    for voulu in ["losange", "pastilles", "transparence", "milieu", "tramage"] {
        assert!(texte.contains(voulu), "« {voulu} » dans le fichier : {texte}");
    }
    assert!(!texte.contains("couleur_depart") && !texte.contains("opacite_depart"), "les champs d'avant ne s'écrivent plus");
    let back = project::load(&manifest).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let cel_b = back.doc.panels[0].layers[0].frames[0].unwrap();
    let relu = back.doc.cels[&cel_b].vector.as_ref().unwrap().elements[0].gradient().unwrap().clone();
    assert_eq!(relu, g, "tout est relu tel quel");
    assert!(back.doc.cels[&cel_b].raster.to_flat() == rendu, "le rendu après relecture est le même (tramage compris)");
    // Une zone unie n'écrit toujours rien de plus.
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_old_two_colour_gradient_reads_as_two_stops_and_renders_the_same_pixel_for_pixel() {
    let vieux = r#"{"genre":"radial","depart":[1.0,2.0],"arrivee":[41.0,2.0],"couleur_depart":[255,0,0],"couleur_arrivee":[0,0,255],"opacite_depart":0.8,"opacite_arrivee":0.1}"#;
    let g: Degrade = serde_json::from_str(vieux).unwrap();
    assert_eq!(g, Degrade::simple(Genre::Radial, [1.0, 2.0], [41.0, 2.0], ROUGE, BLEU, 0.8, 0.1));
    assert!(!g.tramage && g.is_sane() && g.couleurs.len() == 2 && g.opacites.len() == 2);
    // La formule d'avant, recalculée ici : le même pixel, au niveau près.
    let ancien = |x: f32, y: f32| -> Color32 {
        let r = 40.0f32;
        let t = (((x - 1.0).powi(2) + (y - 2.0).powi(2)).sqrt() / r).clamp(0.0, 1.0);
        let m = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * t;
        let a = 0.8 + (0.1 - 0.8) * t;
        let c = |v: f32| (v * a + 0.5).clamp(0.0, 255.0) as u8;
        Color32::from_rgba_premultiplied(c(m(255, 0)), c(m(0, 0)), c(m(0, 255)), (a * 255.0 + 0.5).clamp(0.0, 255.0) as u8)
    };
    let mut r = Raster::new(80, 40);
    degrade::peindre(&mut r, &Portee::Tout, &g, false, None);
    for y in 0..40usize {
        for x in 0..80usize {
            assert_eq!(r.get(x, y), crate::render::over(Color32::TRANSPARENT, ancien(x as f32 + 0.5, y as f32 + 0.5), 255, None), "({x}, {y})");
        }
    }
    // Un projet de version 43 : la zone est écrite à l'ancienne, le projet s'ouvre, le dégradé est relu à l'identique.
    let (dir, manifest, doc) = projet_a_une_zone("vieux", &Degrade::simple(Genre::Radial, [60.0, 60.0], [110.0, 60.0], ROUGE, BLEU, 0.8, 0.1));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let fichier = fichier_du_degrade(&dir);
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&fichier).unwrap()).unwrap();
    fn remplacer(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(d) = m.get_mut("degrade") {
                    *d = serde_json::json!({"genre":"radial","depart":[60.0,60.0],"arrivee":[110.0,60.0],"couleur_depart":[255,0,0],"couleur_arrivee":[0,0,255],"opacite_depart":0.8,"opacite_arrivee":0.1});
                }
                m.values_mut().for_each(remplacer);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(remplacer),
            _ => {}
        }
    }
    remplacer(&mut json);
    std::fs::write(&fichier, serde_json::to_string(&json).unwrap()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains(&format!("\"version\": {}", project::VERSION)) && project::VERSION >= 44);
    std::fs::write(&manifest, texte.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 43")).unwrap();
    let ouvert = project::load(&manifest).unwrap();
    assert!(ouvert.warnings.is_empty(), "{:?}", ouvert.warnings);
    let cel = ouvert.doc.panels[0].layers[0].frames[0].unwrap();
    let lu = ouvert.doc.cels[&cel].vector.as_ref().unwrap().elements[0].gradient().unwrap().clone();
    assert_eq!(lu, Degrade::simple(Genre::Radial, [60.0, 60.0], [110.0, 60.0], ROUGE, BLEU, 0.8, 0.1));
    // Une version plus récente que la nôtre est refusée.
    std::fs::write(&manifest, texte.replace(&format!("\"version\": {}", project::VERSION), &format!("\"version\": {}", project::VERSION + 1))).unwrap();
    assert!(project::load(&manifest).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn absurd_stops_in_a_drawing_make_it_unreadable_like_any_damaged_drawing() {
    let mut g = trois();
    g.couleurs[1].pos = 7.0;
    let (dir, manifest, doc) = projet_a_une_zone("abime", &g);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let ouvert = project::load(&manifest).unwrap();
    assert!(ouvert.warnings.iter().any(|w| w.contains("hors limites")), "{:?}", ouvert.warnings);
    assert!(ouvert.doc.cels.values().all(|c| c.vector.as_ref().is_none_or(|v| v.elements.iter().all(|e| e.gradient().is_none()))));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ les dégradés enregistrés

#[test]
fn saved_gradients_live_in_the_application_settings_not_in_the_project() {
    // Au départ : quelques dégradés simples.
    let d = degrade::enregistres_d_origine();
    assert!(d.len() >= 5 && d.iter().all(|e| e.est_sain()));
    assert!(d.iter().any(|e| e.nom.contains("Noir")) && d.iter().any(|e| e.nom.contains("transparent")) && d.iter().any(|e| e.nom == "Ciel"));
    assert!(crate::settings::Settings::default().degrades.is_none(), "rien d'écrit : les dégradés de départ");
    // Enregistrer le dégradé courant, le rappeler : les mêmes pastilles ; liés aux carrés, ils le restent.
    let mut r = Reglages::default();
    r.couleurs = Some(vec![p(0.0, ROUGE), p(0.3, VERT), p(1.0, BLEU)]);
    r.transparence = Some(vec![o(0.0, 1.0), o(1.0, 0.2)]);
    let e = r.enregistrer("Mon essai");
    let mut autre = Reglages { fin: Fin::VersTransparent, ..Reglages::default() };
    autre.appliquer(&e);
    assert_eq!((autre.couleurs.clone(), autre.transparence.clone(), autre.fin), (r.couleurs.clone(), r.transparence.clone(), Fin::VersFond));
    let lie = Reglages { fin: Fin::VersTransparent, ..Reglages::default() }.enregistrer("Lié");
    assert!(lie.couleurs.is_none() && lie.vers_transparent);
    let mut b = Reglages { couleurs: Some(vec![p(0.0, VERT), p(1.0, VERT)]), ..Reglages::default() };
    b.appliquer(&lie);
    assert_eq!((b.couleurs, b.fin), (None, Fin::VersTransparent), "relié au panneau Couleur");
    // Dans le fichier des réglages de l'application : écrit, relu ; un dégradé absurde est écarté, les autres restent.
    let mut s = crate::settings::Settings::default();
    let mut absurde = e.clone();
    absurde.nom = "Absurde".into();
    absurde.couleurs.as_mut().unwrap()[1].pos = 9.0;
    s.degrades = Some(vec![e.clone(), lie.clone(), absurde]);
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("degrades_enregistres") && texte.contains("Mon essai"), "{texte}");
    let relu = crate::settings::Settings::apres_lecture(toml::from_str(&texte).unwrap());
    assert_eq!(relu.degrades, Some(vec![e, lie]));
    // Un fichier d'avant n'a pas le champ : les dégradés de départ ; une liste vidée à la main reste vide.
    let vide = crate::settings::Settings { degrades: Some(Vec::new()), ..Default::default() };
    let relu = crate::settings::Settings::apres_lecture(toml::from_str(&toml::to_string(&vide).unwrap()).unwrap());
    assert_eq!(relu.degrades, Some(Vec::new()));
}

// ------------------------------------------------------------------------------------ l'interface

fn bouton(pos: Pos2, pressed: bool, button: PointerButton) -> Event {
    Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE }
}

fn touche(key: egui::Key) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// Un glisser du pointeur de `de` à `vers` à l'écran (quelques pas).
fn tirer(b: &mut Banc, de: Pos2, vers: Pos2) {
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![bouton(de, true, PointerButton::Primary)], 1.0 / 60.0);
    for k in 1..=4 {
        let q = de + (vers - de) * (k as f32 / 4.0);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![bouton(vers, false, PointerButton::Primary)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Les Réglages de l'outil Dégradé dans une fenêtre à eux (assez haute pour tout montrer), image par image.
struct Reglages_ {
    ctx: egui::Context,
    t: f64,
}

impl Reglages_ {
    fn new() -> Self {
        Reglages_ { ctx: egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut crate::app::App, events: Vec<Event>) -> Vec<String> {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(320.0, 1000.0))), time: Some(self.t), events, ..Default::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
        });
        let mut textes = Vec::new();
        fn collecter(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn images(&mut self, app: &mut crate::app::App, n: usize) {
        for _ in 0..n {
            self.image(app, Vec::new());
        }
    }

    fn cliquer(&mut self, app: &mut crate::app::App, pos: Pos2) {
        self.image(app, vec![Event::PointerMoved(pos)]);
        self.image(app, vec![bouton(pos, true, PointerButton::Primary)]);
        self.image(app, vec![bouton(pos, false, PointerButton::Primary)]);
        self.image(app, Vec::new());
    }

    fn tirer(&mut self, app: &mut crate::app::App, de: Pos2, vers: Pos2) {
        self.image(app, vec![Event::PointerMoved(de)]);
        self.image(app, vec![bouton(de, true, PointerButton::Primary)]);
        for k in 1..=4 {
            self.image(app, vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 4.0))]);
        }
        self.image(app, Vec::new());
        self.image(app, vec![bouton(vers, false, PointerButton::Primary)]);
        self.image(app, Vec::new());
    }
}

fn app_degrade(nom: &str) -> crate::app::App {
    let mut app = crate::degrade_tests::app(nom);
    app.set_tool(Tool::Gradient);
    app.color = ROUGE;
    app.couleurs.prefs.back = BLEU;
    app
}

#[test]
fn the_settings_show_the_bands_the_shapes_the_figures_and_my_gradients() {
    let mut app = app_degrade("reglages");
    let mut fen = Reglages_::new();
    fen.images(&mut app, 3);
    let textes = fen.image(&mut app, Vec::new()).join(" | ");
    for voulu in ["Vers la couleur de fond", "Vers le transparent", "Opacité", "Inverser", "Tramage", "Tout le calque", "Mes dégradés"] {
        assert!(textes.contains(voulu), "« {voulu} » dans les réglages : {textes}");
    }
    assert!(app.degrade_ui.bande.is_some(), "la bande est à l'écran");
    assert!(app.degrade.tramage, "tramage coché d'origine");
    // Par défaut, deux pastilles liées aux carrés du panneau Couleur.
    assert!(app.degrade.couleurs.is_none() && app.degrade.transparence.is_none());
}

#[test]
fn clicking_under_the_band_adds_a_stop_dragging_moves_it_and_pulling_it_away_removes_it() {
    let mut app = app_degrade("pastilles");
    let mut fen = Reglages_::new();
    fen.images(&mut app, 3);
    let bande = app.degrade_ui.bande.expect("la bande");
    let x = |t: f32| bande.left() + bande.width() * t;
    let sous = bande.bottom() + 14.0;
    let dessus = bande.top() - 14.0;
    // Clic sous la bande, à mi-chemin : une pastille de plus, de la couleur du dégradé à cet endroit.
    fen.cliquer(&mut app, pos2(x(0.5), sous));
    let cs = app.degrade.couleurs.clone().expect("les pastilles sont maintenant les siennes");
    assert_eq!(cs.len(), 3, "{cs:?}");
    assert!(proche(cs[1].pos, 0.5, 0.03) && proche(cs[1].rgb[0] as f32, 128.0, 6.0) && proche(cs[1].rgb[2] as f32, 127.0, 6.0), "{:?}", cs[1]);
    assert_eq!(app.degrade_ui.sel_couleur, Some(1), "la pastille est choisie");
    // Glisser : déplacer, entre ses voisines.
    fen.tirer(&mut app, pos2(x(cs[1].pos), sous), pos2(x(0.8), sous));
    let cs = app.degrade.couleurs.clone().unwrap();
    assert_eq!(cs.len(), 3);
    assert!(proche(cs[1].pos, 0.8, 0.03), "{cs:?}");
    // Le losange de milieu, entre la première et la seconde : le déplacer règle le milieu.
    let milieu_x = x(cs[0].pos + (cs[1].pos - cs[0].pos) * 0.5);
    fen.tirer(&mut app, pos2(milieu_x, bande.bottom() + 4.0), pos2(x(0.2), bande.bottom() + 4.0));
    let cs = app.degrade.couleurs.clone().unwrap();
    assert!(proche(cs[0].milieu, 0.2 / cs[1].pos, 0.06), "milieu : {} (pastille à {})", cs[0].milieu, cs[1].pos);
    // Une pastille d'opacité, au-dessus de la bande.
    fen.cliquer(&mut app, pos2(x(0.3), dessus));
    let os = app.degrade.transparence.clone().expect("l'opacité aussi est la sienne");
    assert_eq!(os.len(), 3);
    assert!(proche(os[1].pos, 0.3, 0.03) && os[1].opacite > 0.99, "{os:?}");
    assert_eq!(app.degrade_ui.sel_opacite, Some(1));
    // Tirer une pastille loin de la bande : elle est supprimée ; mais il en reste deux au moins.
    let cs = app.degrade.couleurs.clone().unwrap();
    fen.tirer(&mut app, pos2(x(cs[1].pos), sous), pos2(x(cs[1].pos), sous + 70.0));
    assert_eq!(app.degrade.couleurs.as_ref().unwrap().len(), 2, "la pastille tirée vers le bas est supprimée");
    let cs = app.degrade.couleurs.clone().unwrap();
    fen.tirer(&mut app, pos2(x(cs[0].pos), sous), pos2(x(cs[0].pos), sous + 70.0));
    assert_eq!(app.degrade.couleurs.as_ref().unwrap().len(), 2, "il en reste deux au moins");
}

#[test]
fn the_plus_saves_the_current_gradient_in_the_settings_and_a_click_on_a_thumbnail_recalls_it() {
    let mut app = app_degrade("enregistres");
    let mut fen = Reglages_::new();
    fen.images(&mut app, 3);
    assert!(app.settings.degrades.is_none(), "au départ : les dégradés de départ, rien d'écrit");
    let n0 = degrade::enregistres_d_origine().len();
    assert_eq!(app.degrade_ui.vignettes.len(), n0, "une vignette par dégradé de départ");
    // Un dégradé à soi : trois pastilles.
    app.degrade.couleurs = Some(vec![p(0.0, ROUGE), p(0.4, VERT), p(1.0, BLEU)]);
    app.degrade.transparence = Some(vec![o(0.0, 1.0), o(1.0, 0.5)]);
    fen.images(&mut app, 2);
    let plus = app.degrade_ui.plus.expect("le bouton +");
    fen.cliquer(&mut app, plus.center());
    let liste = app.settings.degrades.clone().expect("la liste est maintenant la sienne");
    assert_eq!(liste.len(), n0 + 1);
    assert_eq!(liste[n0].couleurs.as_ref().unwrap().len(), 3);
    // Rappeler un autre (le premier : noir vers blanc), puis revenir au sien.
    let premiere = app.degrade_ui.vignettes[0];
    fen.cliquer(&mut app, premiere.center());
    assert_eq!(app.degrade.couleurs.as_ref().map(|c| (c[0].rgb, c[1].rgb)), Some(([0, 0, 0], [255, 255, 255])));
    let derniere = *app.degrade_ui.vignettes.last().unwrap();
    fen.cliquer(&mut app, derniere.center());
    assert_eq!(app.degrade.couleurs.as_ref().unwrap().len(), 3, "le dégradé enregistré est rappelé tel quel");
    assert_eq!(app.degrade.transparence.as_ref().map(|t| t[1].opacite), Some(0.5));
    // Le fichier des réglages l'a gardé (d'une séance à l'autre).
    let relu = crate::settings::Settings::load();
    assert_eq!(relu.degrades.as_ref().map(|l| l.len()), Some(n0 + 1), "écrit dans les réglages de l'application");
}
// ------------------------------------------------------------------------------------ la retouche après coup

/// `tirer` avec des points de la feuille.
fn tirer_f(b: &mut Banc, de: (f32, f32), vers: (f32, f32)) {
    let (a, c) = (ecran(b, de.0, de.1), ecran(b, vers.0, vers.1));
    tirer(b, a, c);
}

fn poser_axe(b: &mut Banc, de: (f32, f32), vers: (f32, f32)) {
    b.app.set_tool(Tool::Gradient);
    b.app.color = ROUGE;
    b.app.couleurs.prefs.back = BLEU;
    glisser(b, &[de, vers], Modifiers::NONE);
}

fn phase(b: &Banc) -> Option<Phase> {
    b.app.degrade_geste.as_ref().map(|g| g.phase)
}

#[test]
fn after_the_release_the_axis_stays_and_its_handles_move_the_gradient_in_the_same_undo_step() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    let avant = etapes(&b.app).len();
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    assert_eq!(phase(&b), Some(Phase::Pose), "l'axe reste après le lâcher");
    assert_eq!(etapes(&b.app).len(), avant + 1);
    let m = pixel(&b, 200, 150);
    assert!(proche(m.r() as f32, 128.0, 14.0), "violet au milieu de l'axe : {m:?}");
    // Tirer la poignée d'arrivée vers (200, 150) : le dégradé se recalcule, bleu dès là.
    tirer_f(&mut b, (280.0, 150.0), (200.0, 150.0));
    assert_eq!(phase(&b), Some(Phase::Pose));
    let m = pixel(&b, 200, 150);
    assert!(m.b() > 240 && m.r() < 15, "bleu à la nouvelle arrivée : {m:?}");
    assert_eq!(etapes(&b.app).len(), avant + 1, "toujours une seule étape « Dégradé »");
    assert_eq!(etapes(&b.app).last().map(String::as_str), Some("Dégradé"));
    // Tirer la poignée de départ.
    tirer_f(&mut b, (120.0, 150.0), (160.0, 150.0));
    assert!(pixel(&b, 150, 150).r() > 240, "rouge avant le nouveau départ : {:?}", pixel(&b, 150, 150));
    assert_eq!(etapes(&b.app).len(), avant + 1);
    // Les réglages : la forme (radial, autour du nouveau départ).
    b.app.degrade.genre = Genre::Radial;
    b.chauffer(3);
    assert_eq!(phase(&b), Some(Phase::Pose));
    assert_eq!(etapes(&b.app).len(), avant + 1);
    let (centre, loin) = (pixel(&b, 162, 150), pixel(&b, 250, 150));
    assert!(centre.r() > 230 && loin.b() > 240, "radial : rouge au centre, bleu au bord : {centre:?} {loin:?}");
    // Les pastilles : trois couleurs.
    b.app.degrade.couleurs = Some(vec![p(0.0, ROUGE), p(0.5, VERT), p(1.0, BLEU)]);
    b.chauffer(3);
    let v = pixel(&b, 180, 150);
    assert!(v.g() > 200, "le vert est au milieu du rayon : {v:?}");
    assert_eq!(etapes(&b.app).len(), avant + 1);
    // L'angle et l'échelle tapés (ce que font les deux champs) : l'axe change, le dégradé suit.
    {
        let g = b.app.degrade_geste.as_mut().unwrap();
        g.fin = crate::degradeui::axe_depuis(g.depart, 0.0, 20.0);
    }
    b.chauffer(3);
    assert!(pixel(&b, 190, 150).b() > 240 && pixel(&b, 170, 150).g() > 200, "le rayon a diminué de moitié : {:?} {:?}", pixel(&b, 190, 150), pixel(&b, 170, 150));
    assert_eq!(etapes(&b.app).len(), avant + 1);
    // Ctrl+Z enlève tout le dégradé d'un coup.
    b.app.undo();
    b.chauffer(2);
    assert_eq!(pixel(&b, 150, 150).a(), 0, "l'intérieur est de nouveau vide");
    assert_eq!(pixel(&b, 200, 100).a(), 255, "le trait est toujours là");
    assert_eq!(etapes(&b.app).len(), avant);
    assert!(b.app.degrade_geste.is_none(), "la séance ne survit pas à l'annulation");
    b.app.redo();
    assert!(pixel(&b, 150, 150).a() > 0, "rétablir remet le dégradé retouché");
}

#[test]
fn changing_tool_layer_frame_or_pressing_enter_or_escape_freezes_the_gradient() {
    // Changer d'outil.
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    let n = etapes(&b.app).len();
    b.app.set_tool(Tool::Brush);
    b.chauffer(2);
    assert!(b.app.degrade_geste.is_none(), "autre outil : figé");
    b.app.set_tool(Tool::Gradient);
    b.chauffer(2);
    assert!(b.app.degrade_geste.is_none(), "reprendre l'outil ne rend pas un bitmap retouchable");
    assert_eq!(etapes(&b.app).len(), n);
    // Échap fige, sans rien annuler.
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    let n = etapes(&b.app).len();
    let rouge = pixel(&b, 130, 150);
    b.image(vec![touche(egui::Key::Escape)], 1.0 / 60.0);
    b.chauffer(1);
    assert!(b.app.degrade_geste.is_none(), "Échap : figé");
    assert_eq!(etapes(&b.app).len(), n, "Échap n'annule pas");
    assert_eq!(pixel(&b, 130, 150), rouge);
    // La poignée n'est plus une poignée : appuyer au même endroit trace un autre dégradé (une étape de plus).
    poser_axe(&mut b, (120.0, 150.0), (200.0, 150.0));
    assert_eq!(etapes(&b.app).len(), n + 1, "un second dégradé, pas une retouche du premier");
    // Entrée aussi.
    b.image(vec![touche(egui::Key::Enter)], 1.0 / 60.0);
    b.chauffer(1);
    assert!(b.app.degrade_geste.is_none());
    // Une autre image (Entrée avance aussi d'une image : on revient d'abord à la première).
    b.app.goto_frame(0);
    b.chauffer(1);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    assert_eq!(phase(&b), Some(Phase::Pose));
    b.app.goto_frame(2);
    b.chauffer(2);
    assert!(b.app.degrade_geste.is_none(), "autre image : figé (image {}, séance {:?})", b.app.cur_frame, b.app.degrade_geste.as_ref().map(|g| (g.phase, g.contexte, g.etape)));
    // Une autre étape d'annulation (un trait).
    b.app.goto_frame(0);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    b.app.history.push(crate::history::Edit::Group(Vec::new()));
    b.chauffer(2);
    assert!(b.app.degrade_geste.is_none(), "une autre étape s'est glissée : figé");
}

#[test]
fn nothing_of_the_brush_engine_runs_during_the_gesture_the_handles_or_the_settings() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    let rejeux = crate::vector::replay_count();
    let cel = cel_du_calque(&b);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    assert_eq!(crate::vector::replay_count(), rejeux, "le tracé et le lâcher");
    tirer_f(&mut b, (280.0, 150.0), (240.0, 150.0));
    assert_eq!(crate::vector::replay_count(), rejeux, "tirer une poignée");
    b.app.degrade.genre = Genre::Angle;
    b.chauffer(3);
    b.app.degrade.couleurs = Some(vec![p(0.0, ROUGE), p(0.5, VERT), p(1.0, BLEU)]);
    b.chauffer(3);
    b.app.degrade.tramage = false;
    b.chauffer(3);
    assert_eq!(crate::vector::replay_count(), rejeux, "les réglages");
    // Et pendant une retouche en cours (pointeur tenu), rien n'est écrit : l'aperçu est à part.
    let p0 = ecran(&b, 240.0, 150.0);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, PointerButton::Primary)], 1.0 / 60.0);
    assert_eq!(phase(&b), Some(Phase::Retouche));
    for k in 0..20 {
        let q = ecran(&b, 240.0 - 3.0 * k as f32, 150.0);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        assert!(b.app.stroke.is_none() && b.app.vstroke.is_none());
    }
    assert_eq!(crate::vector::replay_count(), rejeux, "pendant la retouche");
    let en_retouche = b.app.drawing(cel).map(|d| d.elements.len());
    b.image(vec![bouton(ecran(&b, 180.0, 150.0), false, PointerButton::Primary)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(phase(&b), Some(Phase::Pose));
    assert_eq!(b.app.drawing(cel).map(|d| d.elements.len()), Some(2));
    assert!(en_retouche.is_some());
    assert_eq!(crate::vector::replay_count(), rejeux, "au relâché");
}

#[test]
fn the_preview_during_a_retouch_is_the_mesh_and_the_pixels_are_written_at_the_release() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    let cel = cel_du_calque(&b);
    let ecrit = b.app.doc.cels[&cel].raster.clone();
    // Appuyer sur la poignée d'arrivée : la retouche commence, l'écriture est défaite (l'aperçu montre le dégradé).
    let p0 = ecran(&b, 280.0, 150.0);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, PointerButton::Primary)], 1.0 / 60.0);
    assert_eq!(phase(&b), Some(Phase::Retouche));
    assert_eq!(b.app.doc.cels[&cel].raster.get(150, 150).a(), 0, "pendant la retouche, rien n'est écrit : l'aperçu est à part");
    b.image(vec![Event::PointerMoved(ecran(&b, 200.0, 150.0))], 1.0 / 60.0);
    assert_eq!(b.app.doc.cels[&cel].raster.get(150, 150).a(), 0);
    b.image(vec![bouton(ecran(&b, 200.0, 150.0), false, PointerButton::Primary)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.doc.cels[&cel].raster.get(150, 150).a() == 255, "le relâché réécrit le dégradé");
    assert!(b.app.doc.cels[&cel].raster.tiles != ecrit.tiles);
}

#[test]
fn a_vector_zone_keeps_its_gradient_and_the_tool_shows_the_axis_again_when_it_is_chosen() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    let cel = cel_du_calque(&b);
    poser_axe(&mut b, (120.0, 150.0), (280.0, 150.0));
    b.image(vec![touche(egui::Key::Escape)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(b.app.degrade_geste.is_none(), "figé");
    let n = etapes(&b.app).len();
    let grad = |b: &Banc| b.app.drawing(cel).unwrap().elements[0].gradient().cloned().expect("la zone garde son dégradé");
    assert!(proche(grad(&b).arrivee[0], 280.0, 1.0));
    // La zone est choisie (Sélection) puis l'outil Dégradé est repris : l'axe revient.
    b.app.vsel = Some(crate::vtools::VSelection { cel, items: vec![0] });
    b.app.set_tool(Tool::Gradient);
    b.chauffer(2);
    assert_eq!(phase(&b), Some(Phase::Pose), "l'axe est de nouveau affiché");
    let g = b.app.degrade_geste.as_ref().unwrap();
    assert!(proche(g.depart.x, 120.0, 1.0) && proche(g.fin.x, 280.0, 1.0));
    // Retoucher : tirer la poignée d'arrivée. Une étape de plus, une seule quelles que soient les retouches.
    tirer_f(&mut b, (280.0, 150.0), (240.0, 150.0));
    assert!(proche(grad(&b).arrivee[0], 240.0, 1.5), "{:?}", grad(&b));
    assert_eq!(etapes(&b.app).len(), n + 1);
    tirer_f(&mut b, (240.0, 150.0), (220.0, 150.0));
    assert!(proche(grad(&b).arrivee[0], 220.0, 1.5));
    b.app.degrade.couleurs = Some(vec![p(0.0, ROUGE), p(0.5, VERT), p(1.0, BLEU)]);
    b.chauffer(3);
    assert_eq!(grad(&b).couleurs.len(), 3, "les pastilles suivent");
    assert_eq!(etapes(&b.app).len(), n + 1, "toujours une seule étape pour toutes les retouches");
    // Ctrl+Z : le dégradé d'avant les retouches (arrivée à 280, deux couleurs) revient.
    b.app.undo();
    b.chauffer(2);
    let g = grad(&b);
    assert!(proche(g.arrivee[0], 280.0, 1.0) && g.couleurs.len() == 2, "{g:?}");
    // Le dégradé n'est jamais perdu : la zone le garde, même sans séance.
    assert!(b.app.degrade_geste.is_none() || phase(&b) == Some(Phase::Pose));
}

#[test]
fn the_default_gradient_is_still_front_to_back_linked_to_the_colour_squares() {
    let r = Reglages::default();
    assert!(r.couleurs.is_none() && r.transparence.is_none() && r.fin == Fin::VersFond && !r.inverse && r.opacite == 1.0 && r.tramage);
    let g = r.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(10.0, 0.0));
    assert_eq!((g.c0(), g.c1(), g.a0(), g.a1(), g.couleurs.len(), g.opacites.len()), (ROUGE, BLEU, 1.0, 1.0, 2, 2));
    // Les carrés changent : le dégradé suit tant qu'on n'a pas touché aux pastilles.
    let g = r.degrade(VERT, ROUGE, pos2(0.0, 0.0), pos2(10.0, 0.0));
    assert_eq!((g.c0(), g.c1()), (VERT, ROUGE));
    let _ = vec2(0.0, 0.0);
    let _ = Enregistre { nom: String::new(), couleurs: None, transparence: None, vers_transparent: false };
}

/// Capture du panneau Réglages de l'outil Dégradé (à regarder) :
/// `SHOT_CAPTURE_DIR=… SHOT_CAPTURE_PROJET=<fichier .pelure d'une copie> cargo test --release --offline --target-dir target\msvc degrade_capture -- --ignored --nocapture`
#[test]
#[ignore]
fn degrade_capture() {
    let dir = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let doc = match std::env::var_os("SHOT_CAPTURE_PROJET") {
        Some(p) => project::load(std::path::Path::new(&p)).expect("projet").doc,
        None => Document::new(400, 300, 24),
    };
    let mut b = crate::perf_tests::banc_taille(doc, true, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.chauffer(8);
    b.app.set_tool(Tool::Gradient);
    b.app.color = ROUGE;
    b.app.couleurs.prefs.back = BLEU;
    b.app.degrade.couleurs = Some(vec![p(0.0, ROUGE), PastilleCouleur { pos: 0.45, rgb: [250, 220, 40], milieu: 0.35 }, p(1.0, BLEU)]);
    b.app.degrade.transparence = Some(vec![o(0.0, 1.0), o(0.7, 1.0), o(1.0, 0.25)]);
    b.app.degrade_ui.sel_couleur = Some(1);
    b.chauffer(8);
    let Some(rgba) = b.capturer() else { return println!("pas de carte graphique") };
    image::save_buffer(dir.join("degrade-reglages.png"), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    println!("CAPTURE {}", dir.join("degrade-reglages.png").display());
}
