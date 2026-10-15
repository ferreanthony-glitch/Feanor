//! Tests du moteur exposé aux autres interfaces (11a) : un trait peint par le moteur de Pelure,
//! ses tuiles, l'annulation, la gomme, les réglages et leurs aperçus.

use crate::moteur::{Dessin, Outil, Pinceaux, Trait, TUILE};

fn outil(reglage: usize) -> Outil {
    Outil { reglage, couleur: [220, 30, 30], taille: Some(20.0), opacite: Some(1.0), pression_taille: None, gomme: false }
}

/// Un trait horizontal de (40, 100) à (260, 100), pleine pression.
fn tracer(p: &Pinceaux, d: &mut Dessin, o: &Outil) -> (Vec<usize>, crate::moteur::Annulation) {
    let mut t = Trait::commencer(p, o, true);
    let mut changees = Vec::new();
    for i in 0..=44 {
        t.point(d, 40.0 + i as f32 * 5.0, 100.0, 1.0, &mut changees);
    }
    assert!(t.pointe().is_some(), "bout du trait connu pendant le geste");
    let annulation = t.finir(d, &mut changees);
    (changees, annulation)
}

#[test]
fn the_brushes_of_pelure_are_listed_with_their_previews() {
    let p = Pinceaux::de_pelure();
    let r = p.reglages();
    assert!(r.len() >= 6);
    assert_eq!(r[0].nom, "Pinceau rond");
    assert!(r.iter().any(|x| x.gomme) && r.iter().any(|x| x.texture));
    assert!(r.iter().enumerate().all(|(i, x)| x.numero == i));
    let apercu = p.apercu(0, 120, 40).unwrap();
    assert_eq!(apercu.len(), 120 * 40);
    assert!(apercu.iter().any(|&a| a > 200), "le trait de l'aperçu est visible");
    assert!(p.apercu(999, 10, 10).is_none());
    let png = p.apercu_png(0, 120, 40, [230, 230, 230]).unwrap();
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (120, 40));
}

#[test]
fn a_stroke_is_painted_in_tiles_and_undone() {
    let p = Pinceaux::de_pelure();
    let mut d = Dessin::new(300, 200);
    let (changees, annulation) = tracer(&p, &mut d, &outil(0));
    assert!(!changees.is_empty() && !annulation.est_vide());
    let milieu = d.pixel(150, 100);
    assert!(milieu[3] > 240 && milieu[0] > 200 && milieu[1] < 60, "{milieu:?}");
    assert_eq!(d.pixel(150, 20)[3], 0, "loin du trait : rien");
    // Tuiles : la partie dans l'image (300 px = 4 tuiles + 44 px).
    let derniere = (300usize).div_ceil(TUILE) - 1;
    let (x, _, w, h) = d.bornes(derniere);
    assert_eq!((x, w, h), (256, 44, 64));
    assert_eq!(d.tuile_rgba(derniere).len(), 44 * 64 * 4);
    // Annuler, puis refaire.
    let (refaire, rendues) = annulation.appliquer(&mut d);
    assert!(!rendues.is_empty());
    assert_eq!(d.pixel(150, 100)[3], 0, "annulé");
    let (_, _) = refaire.appliquer(&mut d);
    assert_eq!(d.pixel(150, 100), milieu, "refait à l'identique");
}

#[test]
fn the_eraser_takes_paint_away() {
    let p = Pinceaux::de_pelure();
    let mut d = Dessin::new(300, 200);
    tracer(&p, &mut d, &outil(0));
    let gomme = Outil { gomme: true, taille: Some(40.0), ..outil(0) };
    tracer(&p, &mut d, &gomme);
    assert!(d.pixel(150, 100)[3] < 20, "{:?}", d.pixel(150, 100));
}

#[test]
fn the_tool_overrides_size_and_pressure() {
    let p = Pinceaux::de_pelure();
    let largeur = |o: &Outil, pression: f32| {
        let mut d = Dessin::new(200, 200);
        let mut t = Trait::commencer(&p, o, true);
        let mut c = Vec::new();
        for i in 0..=20 {
            t.point(&mut d, 50.0 + i as f32 * 5.0, 100.0, pression, &mut c);
        }
        t.finir(&mut d, &mut c);
        (0..200).filter(|&y| d.pixel(100, y)[3] > 128).count()
    };
    let gros = Outil { taille: Some(40.0), ..outil(0) };
    let fin = Outil { taille: Some(10.0), ..outil(0) };
    assert!(largeur(&gros, 1.0) > largeur(&fin, 1.0) * 3);
    // Pression faible : plus fin, sauf si la taille ne suit plus la pression.
    assert!(largeur(&gros, 0.3) < largeur(&gros, 1.0));
    let sans = Outil { pression_taille: Some(false), ..gros };
    assert_eq!(largeur(&sans, 0.3), largeur(&sans, 1.0));
}
