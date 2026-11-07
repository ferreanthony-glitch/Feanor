//! La trajectoire d'un calque (`trajectoire.rs`, Anthony, 2026-11-07) : le modèle, sans écran.

use crate::document::LayerTransform;
use crate::poskeys::pose_at;
use crate::trajectoire::{distance, Cles, DessinJalon, Trajectoire, ECART_MIN, LONGUEUR_MIN, MARQUE_DISTANCE_MAX};

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
    assert_eq!(t.images_des_jalons(31), [0, 10, 20, 30]);
    // Une marque sur un bout, ou sur une autre marque, est refusée.
    assert_eq!(t.ajouter_marque(0.0), None);
    assert_eq!(t.ajouter_marque(1.0), None);
    assert_eq!(t.ajouter_marque(0.1 + ECART_MIN * 0.5), None);
    assert_eq!(t.marques.len(), 2);
}

#[test]
fn the_end_can_be_set_and_the_start_the_end_and_the_marks_can_be_retimed() {
    let mut t = droite();
    t.ajouter_marque(0.5);
    // Une fin posée : le chemin dure du départ à la fin, puis le calque reste au bout.
    t.fin = Some(10);
    assert_eq!(t.images_des_jalons(31), [0, 5, 10]);
    let cles = x_des_cles(&t, 31);
    assert_eq!(cles.len(), 11);
    assert!((cles[5].1 - 50.0).abs() < 1e-3 && (cles[10].1 - 100.0).abs() < 1e-3);
    assert_eq!(pose_at(&t.cles(31, LayerTransform::default()), 25, LayerTransform::default()).x, 100.0, "après la fin, au bout");
    // Une fin au-delà du plan est bornée à sa dernière image.
    t.fin = Some(80);
    assert_eq!(t.images_des_jalons(31), [0, 15, 30]);
    // Recaler la fin : les clés d'entre suivent (la marque reste à la moitié du temps).
    assert_eq!(t.recaler(2, 20, 31), Some(20));
    assert_eq!((t.fin, t.images_des_jalons(31)), (Some(20), vec![0, 10, 20]));
    // Recaler le départ.
    assert_eq!(t.recaler(0, 4, 31), Some(4));
    assert_eq!((t.depart, t.images_des_jalons(31)), (4, vec![4, 12, 20]));
    assert_eq!(x_des_cles(&t, 31).first(), Some(&(4, 0.0)));
    // Le départ ne passe pas la fin, la fin ne passe ni le départ ni le plan.
    assert_eq!(t.recaler(0, 25, 31), Some(19));
    assert_eq!(t.recaler(0, 4, 31), Some(4));
    assert_eq!(t.recaler(2, 2, 31), Some(5));
    assert_eq!(t.recaler(2, 20, 31), Some(20));
    assert_eq!(t.recaler(2, 60, 31), Some(30));
    assert_eq!(t.recaler(2, 20, 31), Some(20));
    // Recaler une marque : son temps devient explicite, borné à une image de ses voisines.
    assert_eq!(t.recaler(1, 6, 31), Some(6));
    assert_eq!(t.images_des_jalons(31), [4, 6, 20]);
    assert!(t.temps.as_ref().is_some_and(|v| v.len() == 1 && (v[0] - 2.0 / 16.0).abs() < 1e-3));
    assert_eq!(t.recaler(1, 0, 31), Some(5), "au plus près du départ : une image après");
    assert_eq!(t.recaler(1, 40, 31), Some(19), "au plus près de la fin : une image avant");
    // La vitesse suit : la marque à l'image 19, la première moitié du chemin prend presque tout le temps.
    t.recaler(1, 19, 31);
    let cles = x_des_cles(&t, 31);
    let x_a = |f: usize| cles.iter().find(|k| k.0 == f).unwrap().1;
    assert!((x_a(19) - 50.0).abs() < 1e-3 && x_a(12) < 30.0 && (x_a(20) - 100.0).abs() < 1e-3);
    // Une marque ajoutée prend le milieu du temps de son intervalle ; une marque retirée emporte son temps ; « temps égaux » oublie tout.
    t.recaler(1, 12, 31);
    t.ajouter_marque(0.25);
    assert_eq!(t.temps.as_ref().map(|v| v.len()), Some(2));
    assert_eq!(t.images_des_jalons(31), [4, 8, 12, 20]);
    t.retirer_marque(0);
    assert_eq!(t.images_des_jalons(31), [4, 12, 20]);
    t.temps_egaux();
    assert_eq!((t.temps.clone(), t.images_des_jalons(31)), (None, vec![4, 12, 20]));
    // Un jalon qui n'existe pas, un plan trop court : rien.
    assert_eq!(t.recaler(5, 10, 31), None);
    assert_eq!(t.recaler(2, 10, 5), None);
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
    // Les déplacements aux jalons, relatifs au départ.
    assert_eq!(t.decalages(), [[0.0, 0.0], [100.0, 0.0], [200.0, 0.0]]);
}

#[test]
fn pulling_the_path_bends_it_where_it_is_taken_and_leaves_the_far_ends_alone() {
    let mut t = droite();
    t.ajouter_marque(0.5);
    // Tirer le milieu de 20 vers le bas, sur un rayon de 30 : le milieu descend de 20, à 15 de là de moins, les bouts ne bougent pas.
    t.tirer(0.5, 0.0, 20.0, 30.0);
    assert!((t.point_a(0.5)[1] - 20.0).abs() < 1.5, "{:?}", t.point_a(0.5));
    let a_15 = t.plus_proche([65.0, 0.0]);
    assert!(a_15.1 > 5.0 && a_15.1 < 15.0, "à mi-rayon, à peu près à mi-hauteur : {a_15:?}");
    assert_eq!(t.point_a(0.0), [0.0, 0.0]);
    assert_eq!(t.point_a(1.0), [100.0, 0.0]);
    assert!(t.longueur() > 100.0, "le chemin s'est allongé en se tordant");
    assert_eq!(t.marques, [0.5], "les marques restent à leur fraction");
    assert!(t.points.windows(2).all(|s| distance(s[0], s[1]) <= crate::trajectoire::PAS + 1e-3), "repris au pas");
    // Tirer un bout : il suit.
    t.tirer(1.0, 10.0, 0.0, 30.0);
    assert!((t.point_a(1.0)[0] - 110.0).abs() < 1e-3);
    assert_eq!(t.point_a(0.0), [0.0, 0.0]);
    // Le rayon de tirage : un bon tiers du chemin, 30 au moins.
    assert!((droite().rayon_de_tirage() - 30.0).abs() < 1e-3);
    let longue = Trajectoire::nouvelle(&[[0.0, 0.0], [1000.0, 0.0]]).unwrap();
    assert!((longue.rayon_de_tirage() - 300.0).abs() < 1e-3);
}

#[test]
fn the_file_form_round_trips_and_a_damaged_one_is_cleaned() {
    let mut t = droite();
    t.ajouter_marque(0.4);
    let texte = serde_json::to_string(&t).unwrap();
    assert!(texte.contains("\"marques\""));
    let relue: Trajectoire = serde_json::from_str(&texte).unwrap();
    assert_eq!(relue, t);
    // Sans marques, départ à 0, fin du plan, temps égaux, clés d'interpolation : rien d'écrit de plus que les points.
    let simple = droite();
    let texte = serde_json::to_string(&simple).unwrap();
    assert!(!texte.contains("marques") && !texte.contains("depart") && !texte.contains("fin") && !texte.contains("temps") && !texte.contains("cles") && !texte.contains("dessins"), "{texte}");
    assert_eq!(serde_json::from_str::<Trajectoire>(&texte).unwrap(), simple);
    // Le départ, la fin, les temps, la sorte des clés et les dessins posés s'écrivent et reviennent.
    let mut t = droite();
    t.ajouter_marque(0.5);
    t.depart = 7;
    t.fin = Some(40);
    t.temps = Some(vec![0.3]);
    t.cles = Cles::Images;
    t.dessins = vec![DessinJalon { dessin: 12, dx: 50.0, dy: 0.0 }, DessinJalon { dessin: 13, dx: 100.0, dy: 0.0 }];
    let texte = serde_json::to_string(&t).unwrap();
    assert!(texte.contains("\"depart\":7") && texte.contains("\"fin\":40") && texte.contains("\"temps\":[0.3]") && texte.contains("\"cles\":\"Images\"") && texte.contains("\"dessins\""), "{texte}");
    assert_eq!(serde_json::from_str::<Trajectoire>(&texte).unwrap(), t);
    // Abîmée : marques hors bornes ou en désordre, doublon de points, temps faux (pas un par marque), fin avant le départ.
    let abimee = Trajectoire {
        points: vec![[0.0, 0.0], [0.0, 0.0], [100.0, 0.0]],
        marques: vec![0.9, 1.5, 0.2, 0.9],
        depart: 5,
        fin: Some(3),
        temps: Some(vec![0.5, 0.4, 0.3]),
        cles: Cles::Interpolation,
        dessins: vec![DessinJalon { dessin: 1, dx: f32::NAN, dy: 0.0 }],
    };
    let propre = abimee.assainie().expect("tient debout");
    assert_eq!(propre.points.len(), 2);
    assert_eq!(propre.marques, [0.2, 0.9]);
    assert_eq!((propre.fin, propre.temps.clone(), propre.dessins.len()), (None, None, 0));
    // Des temps bons sont gardés.
    let bonne = Trajectoire { marques: vec![0.2, 0.9], temps: Some(vec![0.1, 0.8]), ..droite() };
    assert_eq!(bonne.assainie().unwrap().temps, Some(vec![0.1, 0.8]));
    assert!(Trajectoire { points: vec![[0.0, 0.0]], ..droite() }.assainie().is_none());
}

#[test]
fn the_path_starts_at_its_start_image_and_nothing_moves_before_it() {
    let mut t = droite();
    t.ajouter_marque(0.5);
    t.depart = 5;
    // Un plan de 24 images : le chemin va de l'image 5 à l'image 23, la marque au milieu du temps, à l'image 14.
    assert_eq!(t.images_des_jalons(24), [5, 14, 23]);
    assert_eq!(t.fraction_a_l_image(3, 24), None, "avant le départ, rien");
    assert_eq!(t.fraction_a_l_image(5, 24), Some(0.0));
    assert!((t.fraction_a_l_image(14, 24).unwrap() - 0.5).abs() < 1e-3);
    assert_eq!(t.fraction_a_l_image(23, 24), Some(1.0));
    let cles = x_des_cles(&t, 24);
    assert_eq!(cles.len(), 19, "une clé par image, de 5 à 23");
    assert_eq!(cles[0], (5, 0.0));
    assert!((cles[18].1 - 100.0).abs() < 1e-3);
    // Avant la première clé, la pose est celle de la première clé : le calque attend au départ.
    let keys = t.cles(24, LayerTransform::default());
    assert_eq!(pose_at(&keys, 2, LayerTransform::default()).x, 0.0);
    // Un plan qui s'arrête au départ, ou juste avant : aucune clé.
    assert!(t.cles(6, LayerTransform::default()).is_empty());
    assert!(t.cles(5, LayerTransform::default()).is_empty());
    assert!(!t.cles(7, LayerTransform::default()).is_empty());
}

#[test]
fn image_keys_pose_no_position_key_the_drawings_do_the_work() {
    let mut t = droite();
    t.ajouter_marque(0.3);
    t.cles = Cles::Images;
    assert!(t.cles(31, LayerTransform::default()).is_empty(), "en clés d'images, ce sont des dessins, posés par l'application");
    assert_eq!(t.images_des_jalons(31), [0, 15, 30]);
    let d = t.decalages();
    assert_eq!(d.len(), 3);
    assert!((d[1][0] - 30.0).abs() < 1e-3 && d[1][1].abs() < 1e-3 && (d[2][0] - 100.0).abs() < 1e-3, "{d:?}");
    t.cles = Cles::Interpolation;
    assert_eq!(t.cles(31, LayerTransform::default()).len(), 31);
}
