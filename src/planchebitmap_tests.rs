//! La couche bitmap de la planche, sans écran : les affinités, les masques, l'écriture et la lecture du dossier.

use super::*;

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-planche-bitmap-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn un_trait() -> Raster {
    let mut r = pixels_vides();
    for x in -70..70 {
        poser_pixel(&mut r, x, x / 3, Color32::from_rgba_premultiplied(200, 30, 40, 255), false);
    }
    r
}

fn morceau(id: u64, r: &Raster) -> Morceau {
    Morceau { id, anchor: Ancre { card: 7, after: 0, rank: 0 }, dessus: false, lien: Some(9), pose: A64::echelle_decalage(0.001, 0.2, -0.1), pixels: Arc::new(r.clone()) }
}

fn fichiers(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
    v.sort();
    v
}

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
    let dir = dossier("relu");
    let r = un_trait();
    let mut b = PlancheBitmap::default();
    b.morceaux.push(morceau(42, &r));
    let mut m = Masque::default();
    m.poser(5, 5);
    assert!(b.noter_empreinte(9, 3, m, &r, |_| true));
    let sig = b.empreintes[0].signature;
    assert!(ecrire(&dir, &b, true).unwrap() >= 2);
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
    assert_eq!(lu.empreintes[0].signature, sig, "la signature est relue telle quelle");
    // Plus rien : le dossier se vide (au dernier passage seulement).
    // (La même couche, vidée : son numéro est celui du dossier. Une autre couche mettrait ce dossier de côté.)
    let vide = PlancheBitmap { id_couche: b.id_couche, ..Default::default() };
    ecrire(&dir, &vide, false).unwrap();
    assert!(dir.join(DOSSIER).join("m42.png").exists(), "rien n'est retiré au premier passage");
    ecrire(&dir, &vide, true).unwrap();
    assert!(!dir.join(DOSSIER).join(INDEX).exists());
    assert!(!dir.join(DOSSIER).join("m42.png").exists());
    // Un index d'une version plus récente : rien n'est touché.
    std::fs::create_dir_all(dir.join(DOSSIER)).unwrap();
    std::fs::write(dir.join(DOSSIER).join(INDEX), br#"{"format":"shot-planche-bitmap","version":99}"#).unwrap();
    std::fs::write(dir.join(DOSSIER).join("m1.png"), b"x").unwrap();
    let lu = lire(&dir, &mut avert);
    assert!(lu.illisible && !avert.is_empty());
    ecrire(&dir, &lu, true).unwrap();
    assert!(dir.join(DOSSIER).join("m1.png").exists(), "rien n'est effacé d'un dossier qu'on n'a pas su lire");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_missing_image_stays_in_the_index_and_the_layer_stays_writable() {
    // Un dossier synchronisé : l'index est arrivé, une image pas encore.
    let dir = dossier("fantome");
    let r = un_trait();
    let mut b = PlancheBitmap::default();
    b.morceaux.push(morceau(42, &r));
    b.morceaux.push(morceau(43, &r));
    ecrire(&dir, &b, true).unwrap();
    let d = dir.join(DOSSIER);
    let image = std::fs::read(d.join("m43.png")).unwrap();
    std::fs::remove_file(d.join("m43.png")).unwrap();
    let mut avert = Vec::new();
    let mut lu = lire(&dir, &mut avert);
    println!("{avert:?}");
    assert!(!lu.illisible, "une image qui manque ne coupe pas la couche");
    assert_eq!(lu.morceaux.len(), 1);
    assert!(lu.peut_poser(1));
    // Un morceau neuf, enregistré : l'index garde aussi celui dont l'image manque.
    lu.morceaux.push(morceau(44, &r));
    ecrire(&dir, &lu, true).unwrap();
    assert!(d.join("m44.png").exists(), "le neuf est écrit");
    // L'image arrive : tout est là à la réouverture.
    std::fs::write(d.join("m43.png"), &image).unwrap();
    let relu = lire(&dir, &mut Vec::new());
    let mut ids: Vec<u64> = relu.morceaux.iter().map(|m| m.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![42, 43, 44], "rien n'est perdu");
    // « Enregistrer sous » : un autre dossier reçoit aussi l'image d'un morceau qu'on n'a pas en mémoire.
    std::fs::remove_file(d.join("m43.png")).unwrap();
    std::fs::write(d.join("m43.png"), b"pas une image").unwrap();
    let lu = lire(&dir, &mut Vec::new());
    assert_eq!(lu.morceaux.len(), 2);
    let autre = dossier("fantome-sous");
    ecrire(&autre, &lu, true).unwrap();
    assert_eq!(std::fs::read(autre.join(DOSSIER).join("m43.png")).unwrap(), b"pas une image", "recopiée telle quelle");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&autre).ok();
}

#[test]
fn an_unreadable_index_refuses_new_pieces_and_save_as_copies_the_folder_as_is() {
    let dir = dossier("illisible");
    let d = dir.join(DOSSIER);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(INDEX), b"{ abime").unwrap();
    std::fs::write(d.join("m5.png"), b"des pixels").unwrap();
    let lu = lire(&dir, &mut Vec::new());
    assert!(lu.illisible);
    assert!(!lu.peut_poser(1), "rien n'est posé sur une couche qu'on ne saurait pas enregistrer");
    let mut essai = lu.clone();
    assert!(essai.ajouter(&BoardLayout::default(), un_trait(), A64::IDENTITE, None, None, false).is_none());
    ecrire(&dir, &lu, true).unwrap();
    assert_eq!(fichiers(&d), vec!["index.json".to_string(), "m5.png".to_string()], "rien n'est touché");
    let autre = dossier("illisible-sous");
    ecrire(&autre, &lu, true).unwrap();
    assert_eq!(fichiers(&autre.join(DOSSIER)), fichiers(&d), "« Enregistrer sous » recopie tout le dossier");
    assert_eq!(std::fs::read(autre.join(DOSSIER).join(INDEX)).unwrap(), b"{ abime");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&autre).ok();
}

#[test]
fn more_pieces_than_the_bound_are_all_read_and_none_is_deleted() {
    let dir = dossier("borne");
    let r = un_trait();
    let mut b = PlancheBitmap::default();
    for i in 0..(MAX_MORCEAUX + 3) as u64 {
        b.morceaux.push(morceau(i + 1, &r));
    }
    assert!(!b.peut_poser(1), "au-delà de la borne, plus rien ne se pose");
    let mut essai = b.clone();
    assert!(essai.ajouter(&BoardLayout::default(), un_trait(), A64::IDENTITE, None, None, false).is_none(), "ajouter refuse");
    ecrire(&dir, &b, true).unwrap();
    let avant = fichiers(&dir.join(DOSSIER)).len();
    let lu = lire(&dir, &mut Vec::new());
    assert_eq!(lu.morceaux.len(), MAX_MORCEAUX + 3, "tout est lu, rien n'est coupé");
    ecrire(&dir, &lu, true).unwrap();
    assert_eq!(fichiers(&dir.join(DOSSIER)).len(), avant, "aucune image n'est effacée");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn removing_a_piece_alone_counts_as_a_change() {
    // (Le film partagé fait avancer sa révision d'après ce compte : une suppression seule doit compter.)
    let dir = dossier("suppression");
    let r = un_trait();
    let mut b = PlancheBitmap::default();
    b.morceaux.push(morceau(1, &r));
    b.morceaux.push(morceau(2, &r));
    ecrire(&dir, &b, true).unwrap();
    assert_eq!(ecrire(&dir, &b, true).unwrap(), 0, "rien n'a changé : rien ne compte");
    b.morceaux.pop();
    assert!(ecrire(&dir, &b, true).unwrap() > 0, "un morceau retiré compte");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_signature_changes_when_any_pixel_under_the_mask_changes() {
    let mut r = un_trait();
    let mut m = Masque::default();
    m.poser(10, 3);
    m.poser(-30, -10);
    let s = signature(&r, &m);
    poser_pixel(&mut r, 50, 50, Color32::WHITE, false);
    assert_eq!(signature(&r, &m), s, "hors du masque : rien ne change");
    poser_pixel(&mut r, 10, 3, Color32::from_rgba_premultiplied(0, 0, 0, 255), false);
    assert_ne!(signature(&r, &m), s, "sous le masque : la signature change");
}

// ------------------------------------------------------------------------------ contre-relecture : coupures, couche vidée, dossier d'un autre projet

#[test]
fn the_first_pass_keeps_citing_a_piece_that_left_for_a_drawing() {
    // Un morceau part vers un dessin (la mémoire ne l'a plus) : tant que le dessin n'est pas écrit, l'index du disque doit le citer encore.
    let dir = dossier("union");
    let r = un_trait();
    let mut b = PlancheBitmap::default();
    b.morceaux.push(morceau(1, &r));
    b.morceaux.push(morceau(2, &r));
    ecrire(&dir, &b, true).unwrap();
    b.morceaux.retain(|m| m.id != 1);
    ecrire(&dir, &b, false).unwrap();
    // (Une coupure ici, avant les dessins : on rouvre.)
    let lu = lire(&dir, &mut Vec::new());
    let ids: Vec<u64> = lu.morceaux.iter().map(|m| m.id).collect();
    println!("après le premier passage : {ids:?}");
    assert!(ids.contains(&1), "le morceau parti vers un dessin pas encore écrit est toujours là : rien n'est perdu");
    // Le dernier passage, après les dessins : l'index exact, l'image part.
    ecrire(&dir, &b, true).unwrap();
    let lu = lire(&dir, &mut Vec::new());
    assert_eq!(lu.morceaux.iter().map(|m| m.id).collect::<Vec<_>>(), vec![2]);
    assert!(!dir.join(DOSSIER).join("m1.png").exists());
    std::fs::remove_dir_all(&dir).ok();
}

// (Le verrou d'un fichier tenu par un autre programme est celui de Windows : l'essai n'existe que là.)
#[cfg(windows)]
#[test]
fn emptying_the_layer_removes_images_first_and_a_locked_image_never_makes_it_unreadable() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = dossier("vidage");
    let r = un_trait();
    let mut b = PlancheBitmap::default();
    b.morceaux.push(morceau(42, &r));
    b.morceaux.push(morceau(43, &r));
    ecrire(&dir, &b, true).unwrap();
    let d = dir.join(DOSSIER);
    // Une image tenue par un autre programme (OneDrive, un antivirus) : elle ne peut pas partir.
    let verrou = std::fs::OpenOptions::new().read(true).share_mode(0).open(d.join("m42.png")).unwrap();
    let vide = PlancheBitmap { id_couche: b.id_couche, dossier_lu: Some(dir.clone()), ..Default::default() };
    ecrire(&dir, &vide, true).unwrap();
    println!("fichiers après le vidage, une image tenue : {:?}", fichiers(&d));
    assert!(!d.join("m43.png").exists(), "l'image libre est partie");
    drop(verrou);
    let lu = lire(&dir, &mut Vec::new());
    assert!(!lu.illisible, "une image qui n'a pas pu partir ne rend pas la couche illisible");
    assert!(lu.peut_poser(1));
    // Des images sans index (l'index perdu) : gardées, la couche reste utilisable.
    std::fs::remove_file(d.join(INDEX)).ok();
    let mut avert = Vec::new();
    let lu = lire(&dir, &mut avert);
    println!("{avert:?}");
    assert!(!lu.illisible && lu.peut_poser(1), "des images sans index ne coupent pas la couche");
    let mut neuve = lu.clone();
    neuve.morceaux.push(morceau(44, &r));
    ecrire(&dir, &neuve, true).unwrap();
    assert!(d.join("m42.png").exists(), "une image inconnue n'est jamais effacée");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_as_into_a_folder_of_another_project_mixes_and_deletes_nothing() {
    let autre = dossier("autre-projet");
    let r = un_trait();
    let mut b_autre = PlancheBitmap::default();
    b_autre.morceaux.push(morceau(7, &r));
    ecrire(&autre, &b_autre, true).unwrap();
    let avant = fichiers(&autre.join(DOSSIER));
    // Mon projet, lu ailleurs, enregistré sous ce dossier-là.
    let mien = dossier("mon-projet");
    let mut b = PlancheBitmap::default();
    b.morceaux.push(morceau(8, &r));
    ecrire(&mien, &b, true).unwrap();
    let mut b = lire(&mien, &mut Vec::new());
    b.dossier_lu = Some(mien.clone());
    ecrire(&autre, &b, false).unwrap();
    ecrire(&autre, &b, true).unwrap();
    let de_cote = autre.join(format!("{DOSSIER}.autre"));
    println!("mis de côté : {:?} ; le mien : {:?}", fichiers(&de_cote), fichiers(&autre.join(DOSSIER)));
    assert_eq!(fichiers(&de_cote), avant, "le dossier de l'autre projet est mis de côté tel quel");
    assert_eq!(lire(&autre, &mut Vec::new()).morceaux.iter().map(|m| m.id).collect::<Vec<_>>(), vec![8], "rien de mêlé");
    std::fs::remove_dir_all(&autre).ok();
    std::fs::remove_dir_all(&mien).ok();
}