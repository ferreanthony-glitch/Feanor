//! La couche bitmap de la planche, sans écran : les affinités, les masques, l'écriture et la lecture du dossier.

use super::*;

#[test]
fn an_affine_and_its_inverse_give_back_the_point() {
    let a = A64 { a: 0.8, b: -0.3, c: 0.25, d: 1.1, tx: 12.5, ty: -40.0 };
    let i = a.inverse().expect("inversible");
    let (x, y) = a.applique(31.0, -7.0);
    let (u, v) = i.applique(x, y);
    assert!((u - 31.0).abs() < 1e-9 && (v + 7.0).abs() < 1e-9);
    assert_eq!(A64::echelle_decalage(1.0, 3.0, -4.0).translation_entiere(), Some((3, -4)));
    assert_eq!(A64::echelle_decalage(1.0, 3.4, -4.0).translation_entiere(), None);
}

#[test]
fn a_mask_keeps_exactly_its_pixels() {
    let mut m = Masque::default();
    for (x, y) in [(0, 0), (63, 63), (64, 0), (-1, -1), (-65, 130)] {
        m.poser(x, y);
    }
    let mut vus = Vec::new();
    m.chaque(|x, y| vus.push((x, y)));
    vus.sort_unstable();
    assert_eq!(vus, vec![(-65, 130), (-1, -1), (0, 0), (63, 63), (64, 0)]);
    assert!(m.contient(-1, -1) && !m.contient(1, 1));
}

#[test]
fn the_folder_is_written_read_back_and_cleaned() {
    let dir = std::env::temp_dir().join(format!("shot-test-planche-bitmap-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = pixels_vides();
    for x in -70..70 {
        poser_pixel(&mut r, x, x / 3, Color32::from_rgba_premultiplied(200, 30, 40, 255), false);
    }
    let mut b = PlancheBitmap::default();
    let ancre = Ancre { card: 7, after: 0, rank: 0 };
    b.morceaux.push(Morceau { id: 42, anchor: ancre, dessus: false, lien: Some(9), pose: A64::echelle_decalage(0.001, 0.2, -0.1), pixels: Arc::new(r.clone()) });
    let mut m = Masque::default();
    m.poser(5, 5);
    b.noter_empreinte(9, 3, m);
    assert!(ecrire(&dir, &b).unwrap() >= 2);
    let mut avert = Vec::new();
    let lu = lire(&dir, &mut avert);
    assert!(avert.is_empty(), "{avert:?}");
    assert_eq!(lu.morceaux.len(), 1);
    assert_eq!(lu.morceaux[0].pose, b.morceaux[0].pose);
    let mut a = Vec::new();
    chaque_pixel(&r, |x, y, c| a.push((x, y, c)));
    let mut l = Vec::new();
    chaque_pixel(&lu.morceaux[0].pixels, |x, y, c| l.push((x, y, c)));
    assert_eq!(a, l, "les pixels relus sont exactement ceux écrits");
    assert!(lu.empreintes[0].masque.contient(5, 5));
    // Plus rien : le dossier se vide.
    ecrire(&dir, &PlancheBitmap::default()).unwrap();
    assert!(!dir.join(DOSSIER).join(INDEX).exists());
    assert!(!dir.join(DOSSIER).join("m42.png").exists());
    // Un index d'une version plus récente : rien n'est touché.
    std::fs::create_dir_all(dir.join(DOSSIER)).unwrap();
    std::fs::write(dir.join(DOSSIER).join(INDEX), br#"{"format":"shot-planche-bitmap","version":99}"#).unwrap();
    std::fs::write(dir.join(DOSSIER).join("m1.png"), b"x").unwrap();
    let lu = lire(&dir, &mut avert);
    assert!(lu.illisible && !avert.is_empty());
    ecrire(&dir, &lu).unwrap();
    assert!(dir.join(DOSSIER).join("m1.png").exists(), "rien n'est effacé d'un dossier qu'on n'a pas su lire");
    std::fs::remove_dir_all(&dir).ok();
}
