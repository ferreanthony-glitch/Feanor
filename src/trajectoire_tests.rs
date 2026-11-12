//! La trajectoire d'un calque (`trajectoire.rs`, Anthony, 2026-11-07) : le modèle, sans écran.

use crate::document::LayerTransform;
use crate::poskeys::pose_at;
use crate::trajectoire::{distance, Ancre, Cles, DessinJalon, Trajectoire, ECART_MIN, LONGUEUR_MIN, MARQUE_DISTANCE_MAX};

/// Un chemin droit de (0, 0) à (100, 0), en clés d'interpolation (le modèle ne pose pas les dessins des clés d'images).
fn droite() -> Trajectoire {
    let mut t = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).expect("un chemin");
    t.cles = Cles::Interpolation;
    t
}

/// Le centre de la feuille des essais du modèle (autour duquel un calque tourne).
const PIVOT: [f32; 2] = [200.0, 150.0];

/// Les clés, le point d'ancrage étant le départ du trait (le déplacement relatif).
fn cles_de(t: &Trajectoire, duree: usize, base: LayerTransform) -> Vec<crate::poskeys::PosKey> {
    t.cles(duree, base, t.points[0], PIVOT)
}

fn x_des_cles(t: &Trajectoire, duree: usize) -> Vec<(usize, f32)> {
    cles_de(t, duree, LayerTransform::default()).iter().map(|k| (k.frame, k.t.x)).collect()
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
    let cles = cles_de(&t, 11, base);
    assert_eq!(cles[0].t, base);
    assert_eq!(cles[10].t, LayerTransform { x: 150.0, ..base });
    // Un plan d'une image : aucune clé.
    assert!(cles_de(&t, 1, base).is_empty());
    assert!(cles_de(&t, 0, base).is_empty());
    // Un autre point d'ancrage (le bas du dessin, à (10, 30) sur la feuille) : c'est lui qui est mis sur le chemin, dès le départ.
    let cles = t.cles(11, base, [10.0, 30.0], PIVOT);
    assert_eq!(cles[0].t, LayerTransform { x: 40.0, y: -50.0, ..base });
    assert_eq!(cles[10].t, LayerTransform { x: 140.0, y: -50.0, ..base });
    let d = t.decalages([10.0, 30.0]);
    assert_eq!(d, [[-10.0, -30.0], [90.0, -30.0]]);
    // Les ancrages tout faits dans une boîte.
    let (min, max) = ([10.0, 20.0], [50.0, 60.0]);
    assert_eq!(Ancre::Bas.dans_la_boite(min, max), Some([30.0, 60.0]));
    assert_eq!(Ancre::Centre.dans_la_boite(min, max), Some([30.0, 40.0]));
    assert_eq!(Ancre::Haut.dans_la_boite(min, max), Some([30.0, 20.0]));
    assert_eq!(Ancre::Gauche.dans_la_boite(min, max), Some([10.0, 40.0]));
    assert_eq!(Ancre::Droite.dans_la_boite(min, max), Some([50.0, 40.0]));
    assert_eq!(Ancre::Depart.dans_la_boite(min, max), None);
    assert_eq!(Ancre::Point { x: 1.0, y: 2.0 }.dans_la_boite(min, max), None);
    // D'origine : les clés d'images, l'ancrage là où part le trait.
    let neuve = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    assert_eq!((neuve.cles, neuve.ancre), (Cles::Images, Ancre::Depart));
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
    assert_eq!(pose_at(&cles_de(&t, 31, LayerTransform::default()), 25, LayerTransform::default()).x, 100.0, "après la fin, au bout");
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
    assert_eq!(t.decalages(t.points[0]), [[0.0, 0.0], [100.0, 0.0], [200.0, 0.0]]);
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
    // Sans marques, départ à 0, fin du plan, temps égaux, clés d'images, ancrage au départ du trait : rien d'écrit de plus que les points.
    let simple = Trajectoire::nouvelle(&[[0.0, 0.0], [100.0, 0.0]]).unwrap();
    let texte = serde_json::to_string(&simple).unwrap();
    assert!(
        !texte.contains("marques") && !texte.contains("depart") && !texte.contains("fin") && !texte.contains("temps") && !texte.contains("cles") && !texte.contains("dessins") && !texte.contains("ancre") && !texte.contains("decalage") && !texte.contains("source") && !texte.contains("tourne"),
        "{texte}"
    );
    assert_eq!(serde_json::from_str::<Trajectoire>(&texte).unwrap(), simple);
    // Le départ, la fin, les temps, la sorte des clés, l'ancrage, le décalage du départ, la source et les dessins posés s'écrivent et reviennent.
    let mut t = droite();
    t.ajouter_marque(0.5);
    t.depart = 7;
    t.fin = Some(40);
    t.temps = Some(vec![0.3]);
    t.ancre = Ancre::Point { x: 3.0, y: 4.0 };
    t.decalage_depart = [1.0, -2.0];
    t.source = Some(11);
    t.dessins = vec![DessinJalon { dessin: 11, dx: 0.0, dy: 0.0, angle: 0.0 }, DessinJalon { dessin: 12, dx: 50.0, dy: 0.0, angle: 30.0 }, DessinJalon { dessin: 13, dx: 100.0, dy: 0.0, angle: 0.0 }];
    t.tourne = true;
    let texte = serde_json::to_string(&t).unwrap();
    assert!(
        texte.contains("\"depart\":7") && texte.contains("\"fin\":40") && texte.contains("\"temps\":[0.3]") && texte.contains("\"cles\":\"Interpolation\"") && texte.contains("\"ancre\":{\"Point\"") && texte.contains("\"source\":11") && texte.contains("\"dessins\"") && texte.contains("\"tourne\":true") && texte.contains("\"angle\":30.0"),
        "{texte}"
    );
    assert_eq!(texte.matches("\"angle\"").count(), 1, "un angle nul ne s'écrit pas : {texte}");
    assert_eq!(serde_json::from_str::<Trajectoire>(&texte).unwrap(), t);
    let mut bas = droite();
    bas.ancre = Ancre::Bas;
    assert!(serde_json::to_string(&bas).unwrap().contains("\"ancre\":\"Bas\""));
    // Abîmée : marques hors bornes ou en désordre, doublon de points, temps faux (pas un par marque), fin avant le départ, point d'ancrage
    // et décalage pas des nombres.
    let abimee = Trajectoire {
        points: vec![[0.0, 0.0], [0.0, 0.0], [100.0, 0.0]],
        marques: vec![0.9, 1.5, 0.2, 0.9],
        depart: 5,
        fin: Some(3),
        temps: Some(vec![0.5, 0.4, 0.3]),
        cles: Cles::Interpolation,
        ancre: Ancre::Point { x: f32::NAN, y: 0.0 },
        decalage_depart: [f32::INFINITY, 0.0],
        source: None,
        dessins: vec![DessinJalon { dessin: 1, dx: f32::NAN, dy: 0.0, angle: 0.0 }, DessinJalon { dessin: 2, dx: 0.0, dy: 0.0, angle: f32::NAN }],
        tourne: false,
        support: None,
    };
    let propre = abimee.assainie().expect("tient debout");
    assert_eq!(propre.points.len(), 2);
    assert_eq!(propre.marques, [0.2, 0.9]);
    assert_eq!((propre.fin, propre.temps.clone(), propre.dessins.len()), (None, None, 0));
    assert_eq!((propre.ancre, propre.decalage_depart), (Ancre::Depart, [0.0, 0.0]));
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
    let keys = cles_de(&t, 24, LayerTransform::default());
    assert_eq!(pose_at(&keys, 2, LayerTransform::default()).x, 0.0);
    // Un plan qui s'arrête au départ, ou juste avant : aucune clé.
    assert!(cles_de(&t, 6, LayerTransform::default()).is_empty());
    assert!(cles_de(&t, 5, LayerTransform::default()).is_empty());
    assert!(!cles_de(&t, 7, LayerTransform::default()).is_empty());
}

#[test]
fn inserting_or_removing_images_keeps_the_start_the_end_and_each_mark_on_its_image() {
    // Un chemin de l'image 4 à l'image 20, une marque au milieu du temps (image 12).
    let mut t = droite();
    t.ajouter_marque(0.5);
    t.depart = 4;
    t.fin = Some(20);
    assert_eq!(t.images_des_jalons(40), [4, 12, 20]);
    // Trois images insérées avant le départ : tout avance de trois, le temps reste en parts égales.
    t.decaler_images(2, 3, false);
    assert_eq!((t.depart, t.fin, t.temps.clone()), (7, Some(23), None));
    assert_eq!(t.images_des_jalons(40), [7, 15, 23]);
    // Trois images insérées à l'image du départ : pareil (ce qui est à l'image ou après suit).
    t.decaler_images(7, 3, false);
    assert_eq!((t.depart, t.fin), (10, Some(26)));
    // Quatre images insérées au milieu, entre la marque (18) et la fin : la marque reste à 18, la fin passe à 30 ; le temps de la marque
    // devient explicite pour cela.
    t.decaler_images(20, 4, false);
    assert_eq!((t.depart, t.fin), (10, Some(30)));
    assert_eq!(t.images_des_jalons(40), [10, 18, 30]);
    assert!(t.temps.as_ref().is_some_and(|v| (v[0] - 8.0 / 20.0).abs() < 1e-5), "{:?}", t.temps);
    // Des images insérées après la fin : rien ne bouge.
    t.decaler_images(31, 5, false);
    assert_eq!((t.depart, t.fin, t.images_des_jalons(40)), (10, Some(30), vec![10, 18, 30]));
    // Deux images retirées avant le départ : tout recule de deux.
    t.decaler_images(0, 2, true);
    assert_eq!((t.depart, t.fin, t.images_des_jalons(40)), (8, Some(28), vec![8, 16, 28]));
    // Quatre images retirées entre le départ et la marque (10 à 14) : la marque passe à 12, la fin à 24, le départ reste.
    t.decaler_images(10, 4, true);
    assert_eq!((t.depart, t.fin, t.images_des_jalons(40)), (8, Some(24), vec![8, 12, 24]));
    // Des images retirées à cheval sur le départ (6 à 10) : le départ se ramène à 6, la marque et la fin reculent de quatre.
    t.decaler_images(6, 4, true);
    assert_eq!((t.depart, t.fin, t.images_des_jalons(40)), (6, Some(20), vec![6, 8, 20]));
    // Rien à décaler : rien ne change.
    let avant = t.clone();
    t.decaler_images(3, 0, false);
    assert_eq!(t, avant);
    // Sans fin posée (anciens projets), seul le départ suit : la fin reste la dernière image du plan.
    let mut ancienne = droite();
    ancienne.depart = 4;
    ancienne.decaler_images(2, 3, false);
    assert_eq!((ancienne.depart, ancienne.fin), (7, None));
    ancienne.decaler_images(10, 3, false);
    assert_eq!((ancienne.depart, ancienne.fin), (7, None));
}

#[test]
fn the_drawing_can_turn_with_the_path_around_its_anchor_and_keeps_its_start_orientation() {
    // Un quart de cercle de rayon 100, de (0, 100) vers la droite puis vers le bas jusqu'à (100, 200) : la direction tourne de 90°.
    let brut: Vec<[f32; 2]> = (0..=60).map(|i| { let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 60.0 * std::f32::consts::FRAC_PI_2; [100.0 * a.cos(), 200.0 + 100.0 * a.sin()] }).collect();
    let mut t = Trajectoire::nouvelle(&brut).expect("un chemin");
    t.cles = Cles::Interpolation;
    assert!((t.point_a(0.0)[0]).abs() < 1e-3 && (t.point_a(0.0)[1] - 100.0).abs() < 1e-3, "{:?}", t.point_a(0.0));
    assert!(t.angle_a(0.0).abs() < 1e-3, "0 au départ");
    let a1 = t.angle_a(1.0);
    assert!((a1 - 90.0).abs() < 6.0, "un quart de tour à l'arrivée (y vers le bas : le sens des aiguilles) : {a1}");
    let a_mi = t.angle_a(0.5);
    assert!((a_mi - 45.0).abs() < 6.0, "la moitié à mi-chemin : {a_mi}");
    // Sans « tourne », les clés gardent l'angle de la pose propre.
    let base = LayerTransform { x: 10.0, y: 20.0, angle: 15.0, ..Default::default() };
    let cles = t.cles(31, base, t.points[0], PIVOT);
    assert!(cles.iter().all(|k| k.t.angle == 15.0));
    // Avec : l'angle suit le chemin, et le point d'ancrage reste sur le chemin malgré la rotation autour du centre de la feuille.
    t.tourne = true;
    let cles = t.cles(31, base, t.points[0], PIVOT);
    assert_eq!(cles.len(), 31);
    assert_eq!(cles[0].t, base, "au départ, la pose propre telle quelle");
    assert!((cles[30].t.angle - (15.0 + a1)).abs() < 1e-3, "{}", cles[30].t.angle);
    let pivot = eframe::egui::pos2(PIVOT[0], PIVOT[1]);
    // Le point d'ancrage (le départ du trait, en coordonnées du dessin sous `base`) : là où la pose `base` l'envoie, c'est (0, 100).
    let ancre_dessin = base.affine(pivot).inverse().unwrap().apply(eframe::egui::pos2(0.0, 100.0));
    for k in [0, 10, 20, 30] {
        let sur_la_feuille = cles[k].t.affine(pivot).apply(ancre_dessin);
        let attendu = t.point_a(t.fraction_a_l_image(k, 31).unwrap());
        assert!((sur_la_feuille.x - attendu[0]).abs() < 0.05 && (sur_la_feuille.y - attendu[1]).abs() < 0.05, "image {k} : {sur_la_feuille:?} pour {attendu:?}");
    }
    // D'une image à l'autre, l'angle ne saute jamais d'un tour.
    assert!(cles.windows(2).all(|w| (w[1].t.angle - w[0].t.angle).abs() < 20.0));
    // Les poses des jalons : déplacement et angle ; sans « tourne », l'angle est nul.
    t.ajouter_marque(0.5);
    let poses = t.poses(t.points[0]);
    assert_eq!(poses.len(), 3);
    assert!(poses[0].1.abs() < 1e-3 && (poses[1].1 - 45.0).abs() < 6.0 && (poses[2].1 - 90.0).abs() < 6.0, "{poses:?}");
    assert!((poses[2].0[0] - 100.0).abs() < 1e-3 && (poses[2].0[1] - 100.0).abs() < 1e-3, "{:?}", poses[2].0);
    t.tourne = false;
    assert!(t.poses(t.points[0]).iter().all(|p| p.1 == 0.0));
    // Un demi-tour et plus : l'angle continue au-delà de 180° au lieu de repartir de −180°.
    let brut: Vec<[f32; 2]> = (0..=120).map(|i| { let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 120.0 * (1.5 * std::f32::consts::PI); [100.0 * a.cos(), 200.0 + 100.0 * a.sin()] }).collect();
    let mut rond = Trajectoire::nouvelle(&brut).expect("un chemin");
    rond.cles = Cles::Interpolation;
    rond.tourne = true;
    let cles = rond.cles(61, LayerTransform::default(), rond.points[0], PIVOT);
    assert!((cles[60].t.angle - 270.0).abs() < 8.0, "trois quarts de tour : {}", cles[60].t.angle);
    assert!(cles.windows(2).all(|w| (w[1].t.angle - w[0].t.angle).abs() < 20.0));
}

#[test]
fn image_keys_pose_no_position_key_the_drawings_do_the_work() {
    let mut t = droite();
    t.ajouter_marque(0.3);
    t.cles = Cles::Images;
    assert!(cles_de(&t, 31, LayerTransform::default()).is_empty(), "en clés d'images, ce sont des dessins, posés par l'application");
    assert_eq!(t.images_des_jalons(31), [0, 15, 30]);
    let d = t.decalages(t.points[0]);
    assert_eq!(d.len(), 3);
    assert!((d[1][0] - 30.0).abs() < 1e-3 && d[1][1].abs() < 1e-3 && (d[2][0] - 100.0).abs() < 1e-3, "{d:?}");
    t.cles = Cles::Interpolation;
    assert_eq!(cles_de(&t, 31, LayerTransform::default()).len(), 31);
}
