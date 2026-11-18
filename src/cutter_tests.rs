//! Cutter : les traits touchÃ©s par le geste sont coupÃ©s Ã  leurs croisements, seuls les morceaux
//! touchÃ©s partent.

use eframe::egui::pos2;

use crate::cutter::{crossings, cut_strokes};
use crate::vector::{Element, VPoint, VectorDrawing};

fn stroke(points: &[(f32, f32)]) -> Element {
    Element::Stroke {
        points: points.iter().map(|&(x, y)| VPoint { x, y, r: 1.0 }).collect(),
        color: [0, 0, 0],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

fn ends(e: &Element) -> ((f32, f32), (f32, f32)) {
    let Element::Stroke { points, .. } = e else { panic!("un trait") };
    let (a, b) = (points[0], points[points.len() - 1]);
    ((a.x.round(), a.y.round()), (b.x.round(), b.y.round()))
}

#[test]
fn crossings_are_found_along_the_line() {
    let h = [pos2(0.0, 50.0), pos2(100.0, 50.0)];
    let v = [pos2(30.0, 0.0), pos2(30.0, 100.0)];
    let c = crossings(&h, &v, false);
    assert_eq!(c.len(), 1);
    assert!((c[0] - 0.3).abs() < 1e-4, "{c:?}");
    // Une boucle : la ligne se croise elle-mÃªme (deux endroits sur elle).
    let lp = [pos2(0.0, 0.0), pos2(10.0, 10.0), pos2(10.0, 0.0), pos2(0.0, 10.0)];
    assert_eq!(crossings(&lp, &lp, true).len(), 2);
}

#[test]
fn an_overshoot_is_trimmed_at_the_crossing() {
    // Un trait horizontal, un vertical qui le dÃ©passe de 20 en bas ; le geste coupe le dÃ©passement.
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)])] };
    let region = cut_strokes(&mut d, &[pos2(20.0, 62.0), pos2(40.0, 62.0)], 2.0).expect("coupÃ©");
    assert!(region.contains(pos2(30.0, 60.0)));
    assert_eq!(d.elements.len(), 2, "l'horizontal intact, le vertical raccourci");
    assert_eq!(ends(&d.elements[0]), ((0.0, 50.0), (100.0, 50.0)));
    assert_eq!(ends(&d.elements[1]), ((30.0, 0.0), (30.0, 50.0)), "arrÃªtÃ© au croisement");
}

#[test]
fn the_middle_piece_between_two_crossings_goes() {
    // Une ligne coupÃ©e par deux verticales ; le geste touche le milieu : restent les deux bouts.
    let mut d = VectorDrawing {
        elements: vec![
            stroke(&[(0.0, 50.0), (100.0, 50.0)]),
            stroke(&[(30.0, 0.0), (30.0, 100.0)]),
            stroke(&[(70.0, 0.0), (70.0, 100.0)]),
        ],
    };
    cut_strokes(&mut d, &[pos2(50.0, 40.0), pos2(50.0, 60.0)], 2.0).expect("coupÃ©");
    let horizontal: Vec<_> = d.elements.iter().map(ends).filter(|(a, b)| a.1 == 50.0 && b.1 == 50.0).collect();
    assert_eq!(horizontal, vec![((0.0, 50.0), (30.0, 50.0)), ((70.0, 50.0), (100.0, 50.0))]);
    assert_eq!(d.elements.len(), 4, "les deux verticales restent entiÃ¨res");
}

#[test]
fn a_click_cuts_the_nearest_piece_and_a_lone_stroke_goes_whole() {
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)])] };
    // Clic sur le dÃ©passement (Ã  0,5 du trait, rayon 1).
    cut_strokes(&mut d, &[pos2(30.5, 65.0)], 2.0).expect("coupÃ©");
    assert_eq!(ends(&d.elements[1]), ((30.0, 0.0), (30.0, 50.0)));
    // Clic loin de tout : rien.
    assert!(cut_strokes(&mut d, &[pos2(500.0, 500.0)], 2.0).is_none());
    // Un trait qui ne croise rien part en entier.
    let mut lone = VectorDrawing { elements: vec![stroke(&[(0.0, 0.0), (50.0, 0.0)])] };
    cut_strokes(&mut lone, &[pos2(25.0, -5.0), pos2(25.0, 5.0)], 2.0).expect("coupÃ©");
    assert!(lone.elements.is_empty());
}

#[test]
fn fills_and_untouched_strokes_stay() {
    let fill = Element::Fill { contours: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]], color: [9, 9, 9], opacity: 1.0, gradient: None };
    let mut d = VectorDrawing { elements: vec![fill.clone(), stroke(&[(0.0, 200.0), (100.0, 200.0)])] };
    assert!(cut_strokes(&mut d, &[pos2(0.0, 5.0), pos2(20.0, 5.0)], 2.0).is_none(), "le geste ne touche aucun trait");
    assert_eq!(d.elements[0], fill);
    assert_eq!(d.elements.len(), 2);
}

// ------------------------------------------------------------------ partie 168 : les rÃ©glages

use eframe::egui::Pos2;

use crate::cutter::{couper, Coupe, Geste, Mode, Reglages};

fn reglages(f: impl FnOnce(&mut Reglages)) -> Reglages {
    let mut r = Reglages::default();
    f(&mut r);
    r
}

fn coupe(d: &mut VectorDrawing, geste: &[Pos2], tol: f32, r: &Reglages, autres: &[Vec<Pos2>]) -> Option<usize> {
    couper(d, &Coupe { geste, tol, reglages: r, autres }).map(|(_, n)| n)
}

fn longueur_totale(d: &VectorDrawing) -> f32 {
    d.elements
        .iter()
        .map(|e| match e {
            Element::Stroke { points, .. } => points.windows(2).map(|w| w[0].pos().distance(w[1].pos())).sum::<f32>(),
            _ => 0.0,
        })
        .sum()
}

/// Un trait vertical qui s'arrÃªte Ã  3 px d'un trait horizontal.
fn presque() -> VectorDrawing {
    VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 47.0)])] }
}

#[test]
fn a_near_crossing_counts_within_the_tolerated_gap() {
    let geste = [pos2(50.0, 40.0), pos2(50.0, 60.0)];
    // 0 : comme avant, l'horizontale ne croise rien et part en entier.
    let mut d = presque();
    coupe(&mut d, &geste, 2.0, &Reglages::default(), &[]).expect("coupÃ©");
    assert_eq!(d.elements.len(), 1, "l'horizontale part en entier : {:?}", d.elements.iter().map(ends).collect::<Vec<_>>());
    // 4 px tolÃ©rÃ©s : le bout de la verticale compte comme un croisement ; l'horizontale est coupÃ©e au point d'approche.
    let mut d = presque();
    coupe(&mut d, &geste, 2.0, &reglages(|r| r.ecart = 4.0), &[]).expect("coupÃ©");
    assert_eq!(d.elements.len(), 2);
    assert_eq!(ends(&d.elements[0]), ((0.0, 50.0), (30.0, 50.0)), "arrÃªtÃ©e au point d'approche");
    assert_eq!(ends(&d.elements[1]), ((30.0, 0.0), (30.0, 47.0)), "la verticale ne bouge pas");
    // 2 px seulement : trop court pour l'Ã©cart de 3.
    let mut d = presque();
    coupe(&mut d, &geste, 2.0, &reglages(|r| r.ecart = 2.0), &[]).expect("coupÃ©");
    assert_eq!(d.elements.len(), 1);
}

#[test]
fn default_settings_cut_as_before() {
    let mut a = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)]), stroke(&[(70.0, 0.0), (70.0, 100.0)])] };
    let mut b = a.clone();
    let g = [pos2(20.0, 62.0), pos2(40.0, 62.0)];
    cut_strokes(&mut a, &g, 2.0);
    coupe(&mut b, &g, 2.0, &Reglages::default(), &[]);
    assert_eq!(a.elements, b.elements);
}

#[test]
fn a_tapered_cut_end_thins_out() {
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)])] };
    coupe(&mut d, &[pos2(20.0, 62.0), pos2(40.0, 62.0)], 2.0, &reglages(|r| r.effile = 10.0), &[]).expect("coupÃ©");
    let Element::Stroke { points, .. } = &d.elements[1] else { panic!("un trait") };
    let (debut, fin) = (points[0], points[points.len() - 1]);
    assert_eq!((fin.x.round(), fin.y.round()), (30.0, 50.0), "arrÃªtÃ© au croisement");
    assert!(fin.r < 0.2, "le bout coupÃ© s'affine : {}", fin.r);
    assert!((debut.r - 1.0).abs() < 1e-3, "l'autre bout ne change pas : {}", debut.r);
    // Hors de l'effilÃ© (Ã  15 px du bout), le rayon est entier ; dedans, il dÃ©croÃ®t.
    // (Le rayon en y, le long du trait.)
    let r_a = |y: f32| {
        let w = points.windows(2).find(|w| w[0].y <= y && w[1].y >= y).expect("un segment");
        let t = if w[1].y > w[0].y { (y - w[0].y) / (w[1].y - w[0].y) } else { 0.0 };
        w[0].r + (w[1].r - w[0].r) * t
    };
    assert!((r_a(35.0) - 1.0).abs() < 1e-3, "{}", r_a(35.0));
    let milieu = r_a(45.0);
    assert!(milieu < 1.0 && milieu > fin.r, "{milieu}");
    // Net (0) : le rayon reste entier au bout.
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)]), stroke(&[(30.0, 0.0), (30.0, 70.0)])] };
    coupe(&mut d, &[pos2(20.0, 62.0), pos2(40.0, 62.0)], 2.0, &Reglages::default(), &[]).expect("coupÃ©");
    let Element::Stroke { points, .. } = &d.elements[1] else { panic!("un trait") };
    assert!((points[points.len() - 1].r - 1.0).abs() < 1e-3);
}

/// Un Â« # Â» : deux horizontales et deux verticales qui dÃ©passent de 5 px de chaque cÃ´tÃ© ; l'horizontale du haut a une queue de
/// 50 px Ã  droite.
fn diese() -> VectorDrawing {
    VectorDrawing {
        elements: vec![
            stroke(&[(15.0, 20.0), (130.0, 20.0)]),
            stroke(&[(15.0, 80.0), (85.0, 80.0)]),
            stroke(&[(20.0, 15.0), (20.0, 85.0)]),
            stroke(&[(80.0, 15.0), (80.0, 85.0)]),
        ],
    }
}

#[test]
fn cleaning_overshoots_removes_short_ends_and_keeps_pieces_between_crossings() {
    let r = reglages(|r| {
        r.mode = Mode::Depassements;
        r.depassement = 12.0;
    });
    // Un lasso autour de tout : les sept bouts de 5 px partent, la queue de 50 px reste, les morceaux entre croisements aussi.
    let tour = [pos2(-10.0, -10.0), pos2(150.0, -10.0), pos2(150.0, 100.0), pos2(-10.0, 100.0)];
    let mut d = diese();
    let r_lasso = Reglages { geste: Geste::Lasso, ..r };
    assert_eq!(coupe(&mut d, &tour, 2.0, &r_lasso, &[]), Some(7));
    assert!((longueur_totale(&d) - (4.0 * 60.0 + 50.0)).abs() < 0.1, "{}", longueur_totale(&d));
    // Le mÃªme tour en travers (il entoure tout) : pareil.
    let mut d = diese();
    assert_eq!(coupe(&mut d, &tour, 2.0, &r, &[]), Some(7));
    // Un passage en travers sur un seul bout : lui seul.
    let mut d = diese();
    assert_eq!(coupe(&mut d, &[pos2(16.0, 10.0), pos2(16.0, 30.0)], 1.0, &r, &[]), Some(1));
    assert!((longueur_totale(&d) - (115.0 + 3.0 * 70.0 - 5.0)).abs() < 0.1, "{}", longueur_totale(&d));
    // Un trait seul (sans croisement) ne part jamais en mode DÃ©passements.
    let mut seul = VectorDrawing { elements: vec![stroke(&[(0.0, 0.0), (8.0, 0.0)])] };
    assert!(coupe(&mut seul, &tour, 2.0, &r, &[]).is_none());
    assert_eq!(seul.elements.len(), 1);
    // Plus courts que 3 px seulement : rien ne part.
    let mut d = diese();
    assert!(coupe(&mut d, &tour, 2.0, &Reglages { depassement: 3.0, ..r }, &[]).is_none());
}

#[test]
fn the_lasso_removes_the_pieces_entirely_inside() {
    let mut d = VectorDrawing {
        elements: vec![
            stroke(&[(0.0, 50.0), (100.0, 50.0)]),
            stroke(&[(30.0, 0.0), (30.0, 100.0)]),
            stroke(&[(70.0, 0.0), (70.0, 100.0)]),
        ],
    };
    let tour = [pos2(25.0, 40.0), pos2(75.0, 40.0), pos2(75.0, 60.0), pos2(25.0, 60.0)];
    let r = reglages(|r| r.geste = Geste::Lasso);
    assert_eq!(coupe(&mut d, &tour, 2.0, &r, &[]), Some(1), "seul le milieu de l'horizontale est entiÃ¨rement dedans");
    let horizontal: Vec<_> = d.elements.iter().map(ends).filter(|(a, b)| a.1 == 50.0 && b.1 == 50.0).collect();
    assert_eq!(horizontal, vec![((0.0, 50.0), (30.0, 50.0)), ((70.0, 50.0), (100.0, 50.0))]);
    assert_eq!(d.elements.len(), 4, "les verticales restent entiÃ¨res");
}

#[test]
fn other_lines_cut_without_being_drawn_on() {
    // Une ligne de coupe d'un autre calque (en x 30) : l'horizontale est coupÃ©e contre elle.
    let mut d = VectorDrawing { elements: vec![stroke(&[(0.0, 50.0), (100.0, 50.0)])] };
    let autre = vec![vec![pos2(30.0, 0.0), pos2(30.0, 100.0)]];
    coupe(&mut d, &[pos2(50.0, 40.0), pos2(50.0, 60.0)], 2.0, &Reglages::default(), &autre).expect("coupÃ©");
    assert_eq!(d.elements.len(), 1, "rien n'est ajoutÃ© au dessin");
    assert_eq!(ends(&d.elements[0]), ((0.0, 50.0), (30.0, 50.0)));
}

#[test]
fn the_settings_are_kept_with_the_workstation_settings() {
    let mut s = crate::settings::Settings::default();
    let texte = toml::to_string(&s).unwrap();
    assert!(!texte.contains("[cutter]"), "les rÃ©glages d'origine ne s'Ã©crivent pas");
    s.cutter = reglages(|r| {
        r.ecart = 3.5;
        r.effile = 12.0;
        r.mode = Mode::Depassements;
        r.depassement = 20.0;
        r.portee = crate::cutter::Portee::Visibles;
        r.geste = Geste::Lasso;
    });
    let texte = toml::to_string(&s).unwrap();
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!(crate::settings::Settings::apres_lecture(relu).cutter, s.cutter);
    // Une valeur hors bornes Ã©crite Ã  la main est ramenÃ©e.
    let abime = texte.replace("ecart_tolere = 3.5", "ecart_tolere = 900.0");
    assert_ne!(abime, texte);
    let relu: crate::settings::Settings = toml::from_str(&abime).unwrap();
    assert_eq!(crate::settings::Settings::apres_lecture(relu).cutter.ecart, crate::cutter::ECART_MAX);
}

/// Un gros dessin : 300 traits (150 horizontaux, 150 verticaux, 40 points chacun) et 100 lignes d'un autre calque ; un geste en
/// travers, avec et sans Ã©cart tolÃ©rÃ©, en coupe et en nettoyage. Il doit rester bien sous l'image d'Ã©cran.
#[test]
fn a_big_drawing_cuts_fast() {
    let dense = |a: (f32, f32), b: (f32, f32)| -> Element {
        let pts: Vec<(f32, f32)> = (0..40)
            .map(|k| {
                let t = k as f32 / 39.0;
                (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
            })
            .collect();
        stroke(&pts)
    };
    let mut els = Vec::new();
    for k in 0..150 {
        let v = 5.0 + k as f32 * 6.0;
        els.push(dense((0.0, v), (900.0, v + 3.0)));
        els.push(dense((v, 0.0), (v + 3.0, 900.0)));
    }
    let autres: Vec<Vec<Pos2>> = (0..100).map(|k| vec![pos2(0.0, k as f32 * 9.0 + 2.0), pos2(900.0, k as f32 * 9.0 + 7.0)]).collect();
    let geste: Vec<Pos2> = (0..60).map(|k| pos2(300.0 + k as f32 * 5.0, 200.0 + (k as f32 * 0.3).sin() * 40.0)).collect();
    let essais = [
        Reglages::default(),
        reglages(|r| r.ecart = 3.0),
        reglages(|r| {
            r.mode = Mode::Depassements;
            r.ecart = 3.0;
            r.depassement = 30.0;
        }),
    ];
    for r in essais {
        // (Le meilleur de cinq : la suite complète fait tourner d'autres essais sur tous les cœurs en même temps.)
        let ms = (0..5)
            .map(|_| {
                let mut d = VectorDrawing { elements: els.clone() };
                let t = std::time::Instant::now();
                coupe(&mut d, &geste, 2.0, &r, &autres);
                t.elapsed().as_secs_f64() * 1000.0
            })
            .fold(f64::MAX, f64::min);
        eprintln!("cutter, 300 traits + 100 lignes, {:?}, Ã©cart {} : {ms:.1} ms", r.mode, r.ecart);
        assert!(ms < 50.0, "trop lent : {ms:.1} ms");
    }
}
