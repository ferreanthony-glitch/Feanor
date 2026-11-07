//! La trajectoire d'un calque (`trajectoire.rs`, Anthony, 2026-11-07) : le modèle, sans écran.

use crate::document::LayerTransform;
use crate::trajectoire::{distance, Trajectoire, ECART_MIN, LONGUEUR_MIN, MARQUE_DISTANCE_MAX};

/// Un chemin droit de (0, 0) à (100, 0).
fn droite() -> Trajectoire {
    Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).expect("un chemin")
}

fn x_des_cles(t: &Trajectoire, duree: usize) -> Vec<(usize, f32)> {
    t.cles(duree, LayerTransform::default()).iter().map(|k| (k.frame, k.t.x)).collect()
}

#[test]
fn a_path_without_marks_moves_the_layer_at_constant_speed_over_the_whole_plan() {
    let t = droite();
    let cles = x_des_cles(&t, 11);
    assert_eq!(cles.len(), 11, "une clé par image");
    for (f, x) in &cles {
        assert!((x - 10.0 * *f as f32).abs() < 1e-3, "image {f} : x = {x}");
    }
    // Le déplacement est relatif : la pose du calque au départ est gardée.
    let base = LayerTransform { x: 50.0, y: -20.0, scale: 2.0, angle: 30.0, ..Default::default() };
    let cles = t.cles(11, base);
    assert_eq!(cles[0].t, base);
    assert_eq!(cles[10].t, LayerTransform { x: 150.0, ..base });
    // Un plan d'une image : aucune clé.
    assert!(t.cles(1, base).is_empty());
    assert!(t.cles(0, base).is_empty());
}

#[test]
fn marks_share_the_time_equally_so_close_marks_mean_slow_and_far_marks_mean_fast() {
    let mut t = droite();
    assert_eq!(t.ajouter_marque(0.9), Some(0));
    assert_eq!(t.ajouter_marque(0.1), Some(0), "rangée avant 0,9");
    assert_eq!(t.marques, [0.1, 0.9]);
    // Trois intervalles sur trente images : dix images chacun. Lent, vite, lent.
    let cles = x_des_cles(&t, 31);
    let x = |f: usize| cles[f].1;
    assert!((x(0) - 0.0).abs() < 1e-3);
    assert!((x(5) - 5.0).abs() < 1e-3);
    assert!((x(10) - 10.0).abs() < 1e-3);
    assert!((x(20) - 90.0).abs() < 1e-3);
    assert!((x(25) - 95.0).abs() < 1e-3);
    assert!((x(30) - 100.0).abs() < 1e-3);
    // Une marque sur un bout, ou sur une autre marque, est refusée.
    assert_eq!(t.ajouter_marque(0.0), None);
    assert_eq!(t.ajouter_marque(1.0), None);
    assert_eq!(t.ajouter_marque(0.1 + ECART_MIN * 0.5), None);
    assert_eq!(t.marques.len(), 2);
}

#[test]
fn one_mark_equals_n_images_gives_the_path_its_own_length_in_time() {
    let mut t = droite();
    t.ajouter_marque(0.5);
    t.images_par_marque = Some(4);
    // Deux intervalles de quatre images : le chemin dure huit images, puis le calque reste.
    let cles = x_des_cles(&t, 24);
    assert_eq!(cles.len(), 9);
    assert_eq!(cles.last().unwrap().0, 8);
    assert!((cles[4].1 - 50.0).abs() < 1e-3);
    assert!((cles[8].1 - 100.0).abs() < 1e-3);
    // Un plan plus court que le chemin : les clés s'arrêtent à sa dernière image.
    let cles = x_des_cles(&t, 5);
    assert_eq!(cles.len(), 5);
    assert!((cles[4].1 - 50.0).abs() < 1e-3);
    // Zéro image par marque : comme sans réglage.
    t.images_par_marque = Some(0);
    assert_eq!(x_des_cles(&t, 11).len(), 11);
}

#[test]
fn a_short_stroke_across_the_path_is_a_mark_where_it_crosses() {
    let t = droite();
    let m = t.marque_de(&[[50.0, -5.0], [50.0, 5.0]]).expect("une marque");
    assert!((m - 0.5).abs() < 1e-3);
    // En biais aussi, et dans l'autre sens.
    let m = t.marque_de(&[[22.0, 6.0], [18.0, -6.0]]).expect("une marque");
    assert!((m - 0.2).abs() < 0.01);
    // Un trait qui frôle le chemin sans le croiser est une marque, au plus près.
    let m = t.marque_de(&[[70.0, -20.0], [70.0, -MARQUE_DISTANCE_MAX + 1.0]]).expect("une marque");
    assert!((m - 0.7).abs() < 1e-3);
    // Trop loin : pas une marque. Trop long devant le chemin : pas une marque. Trop court : rien.
    assert_eq!(t.marque_de(&[[70.0, 30.0], [70.0, 40.0]]), None);
    assert_eq!(t.marque_de(&[[50.0, -40.0], [50.0, 40.0]]), None);
    assert_eq!(t.marque_de(&[[50.0, -1.0], [50.0, 1.0]]), None);
    assert!(LONGUEUR_MIN > 2.0);
}

#[test]
fn marks_are_found_near_their_point_moved_within_their_neighbours_and_removed() {
    let mut t = droite();
    t.ajouter_marque(0.3);
    t.ajouter_marque(0.6);
    assert_eq!(t.marque_pres_de([31.0, 2.0], 5.0), Some(0));
    assert_eq!(t.marque_pres_de([58.0, -2.0], 5.0), Some(1));
    assert_eq!(t.marque_pres_de([45.0, 0.0], 5.0), None);
    // Glisser la première au-delà de la seconde : elle s'arrête juste avant.
    let f = t.deplacer_marque(0, 0.8);
    assert!((f - (0.6 - ECART_MIN)).abs() < 1e-6);
    assert!((t.marques[0] - f).abs() < 1e-6);
    // Glisser la seconde jusqu'au bout : elle s'arrête juste avant l'arrivée.
    let f = t.deplacer_marque(1, 2.0);
    assert!((f - (1.0 - ECART_MIN)).abs() < 1e-6);
    t.retirer_marque(0);
    assert_eq!(t.marques.len(), 1);
    t.retirer_marque(7);
    assert_eq!(t.marques.len(), 1);
}

#[test]
fn the_path_is_smoothed_resampled_and_keeps_its_ends_and_a_click_is_not_a_path() {
    let brut: Vec<[f32; 2]> = (0..40).map(|i| [i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 3.0 }]).collect();
    let t = Trajectoire::nouvelle(&brut).expect("un chemin");
    assert_eq!(t.points[0], brut[0]);
    assert_eq!(*t.points.last().unwrap(), *brut.last().unwrap());
    // Le zigzag est atténué, les points sont au pas.
    assert!(t.points.iter().all(|p| p[1] > -0.1 && p[1] < 3.1));
    assert!(t.points.windows(2).all(|s| distance(s[0], s[1]) <= crate::trajectoire::PAS + 1e-3));
    assert!((t.point_a(0.0)[0] - 0.0).abs() < 1e-3);
    assert!((t.point_a(1.0)[0] - 195.0).abs() < 1e-3);
    assert!((t.point_a(0.5)[0] - 97.5).abs() < 1.0);
    // Un trait trop court, ou un seul point : pas de chemin.
    assert!(Trajectoire::nouvelle(&[[0.0, 0.0], [3.0, 0.0]]).is_none());
    assert!(Trajectoire::nouvelle(&[[0.0, 0.0]]).is_none());
    assert!(Trajectoire::nouvelle(&[]).is_none());
    // Retracer garde les marques.
    let mut t = droite();
    t.ajouter_marque(0.25);
    assert!(t.retracer(&[[0.0, 0.0], [0.0, 200.0]]));
    assert_eq!(t.marques, [0.25]);
    assert!((t.point_a(0.25)[1] - 50.0).abs() < 1e-3);
    assert!(!t.retracer(&[[0.0, 0.0], [1.0, 0.0]]));
    assert!((t.longueur() - 200.0).abs() < 1e-3);
}

#[test]
fn moving_and_stretching_the_path_keep_the_marks_at_their_fractions() {
    let mut t = droite();
    t.ajouter_marque(0.5);
    t.deplacer(10.0, 20.0);
    assert_eq!(t.point_a(0.0), [10.0, 20.0]);
    assert_eq!(t.point_a(0.5), [60.0, 20.0]);
    t.transformer(|p| [p[0] * 2.0, p[1]]);
    assert!((t.longueur() - 200.0).abs() < 1e-3);
    assert_eq!(t.point_a(0.5), [120.0, 20.0]);
    let (min, max) = t.englobant();
    assert_eq!(min, [20.0, 20.0]);
    assert_eq!(max, [220.0, 20.0]);
    // Le plus proche : sur le chemin, la fraction et la distance.
    let (f, d) = t.plus_proche([70.0, 25.0]);
    assert!((f - 0.25).abs() < 1e-3);
    assert!((d - 5.0).abs() < 1e-3);
}

#[test]
fn the_file_form_round_trips_and_a_damaged_one_is_cleaned() {
    let mut t = droite();
    t.ajouter_marque(0.4);
    t.images_par_marque = Some(3);
    let texte = serde_json::to_string(&t).unwrap();
    assert!(texte.contains("\"marques\""));
    let relue: Trajectoire = serde_json::from_str(&texte).unwrap();
    assert_eq!(relue, t);
    // Sans marques ni réglage, rien d'écrit de plus que les points.
    let simple = droite();
    let texte = serde_json::to_string(&simple).unwrap();
    assert!(!texte.contains("marques") && !texte.contains("images_par_marque"));
    assert_eq!(serde_json::from_str::<Trajectoire>(&texte).unwrap(), simple);
    // Abîmée : marques hors bornes ou en désordre, doublon de points, zéro image par marque.
    let abimee = Trajectoire { points: vec![[0.0, 0.0], [0.0, 0.0], [100.0, 0.0]], marques: vec![0.9, 1.5, 0.2, 0.9], images_par_marque: Some(0) };
    let propre = abimee.assainie().expect("tient debout");
    assert_eq!(propre.points.len(), 2);
    assert_eq!(propre.marques, [0.2, 0.9]);
    assert_eq!(propre.images_par_marque, None);
    assert!(Trajectoire { points: vec![[0.0, 0.0]], marques: vec![], images_par_marque: None }.assainie().is_none());
}
