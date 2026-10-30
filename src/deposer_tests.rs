//! Essais des images déposées sur la planche des Vignettes (`deposer.rs`).

use std::path::PathBuf;

use eframe::egui::{pos2, Color32};

use super::*;
use crate::document::{Document, LayerKind};
use crate::perf_tests::{se_reposer, Banc};
use crate::vignettes_tests::{barre, case_vide, dernier_nom, ouvert, pas_d_annulation, quatre_plans, vignette};

/// La planche assez dézoomée pour que les cases vides tiennent dans la vue.
fn ouvert_petit(doc: Document) -> Banc {
    let mut b = ouvert(doc, 3);
    b.app.board_zoom = 0.45;
    b.chauffer(4);
    b
}

fn dossier(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-depot-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Une image PNG unie de `w` × `h` pixels.
fn png(dir: &std::path::Path, nom: &str, w: u32, h: u32, rgb: [u8; 3]) -> PathBuf {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let p = dir.join(nom);
    img.save(&p).expect("png");
    p
}

/// Attend que le fil de fond ait rendu les images et que l'interface les ait posées.
fn attendre(b: &mut Banc) {
    let debut = std::time::Instant::now();
    while !b.app.vignettes.depots.is_empty() && debut.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.chauffer(3);
    assert!(b.app.vignettes.depots.is_empty(), "le dépôt a fini");
}

fn deposer(b: &mut Banc, paths: &[PathBuf], at: impl FnOnce(&Banc) -> eframe::egui::Pos2) -> bool {
    let at = at(b);
    let pris = b.app.board_take_drop(paths, Some(at));
    if pris {
        attendre(b);
    }
    pris
}

/// Le calque « Image » du plan `i` : son rang et les images où il montre un dessin.
fn calque_image(doc: &Document, i: usize) -> Option<(usize, Vec<usize>)> {
    let p = &doc.panels[i];
    let li = p.layers.iter().position(|l| l.name == NOM_DU_CALQUE)?;
    Some((li, p.layers[li].frames.iter().enumerate().filter(|(_, f)| f.is_some()).map(|(k, _)| k).collect()))
}

/// L'image du plan `p` à l'image `f`, composée comme le film.
fn image_du_plan(doc: &Document, p: usize, f: usize) -> Vec<Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    let snap = snapshot_of(doc, &[p]);
    let rendu = Rendu::new(snap, Papier::Fond, false, Vec::new(), std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
    (*rendu.image(p, f).expect("image")).clone()
}

/// Un pixel du centre du cadre.
fn centre_du_plan(doc: &Document, p: usize, f: usize) -> Color32 {
    let img = image_du_plan(doc, p, f);
    let [w, h] = doc.frame;
    img[(h / 2) * w + w / 2]
}

#[test]
fn an_image_dropped_on_a_case_goes_to_a_separate_layer_under_the_drawing() {
    let dir = dossier("case");
    let mut b = ouvert(quatre_plans(), 3);
    let avant_calques = b.app.doc.panels[1].layers.len();
    let etapes = pas_d_annulation(&b);
    // Le plan 1 (vignette 3) : une image rouge, deux fois plus large que le cadre (16:9 → 32:9).
    let rouge = png(&dir, "rouge.png", 640, 90, [250, 10, 10]);
    assert!(deposer(&mut b, &[rouge], |b| vignette(b, 3).center()));
    let doc = &b.app.doc;
    assert_eq!(doc.panels.len(), 4, "aucun plan de plus");
    assert_eq!(doc.panels[1].layers.len(), avant_calques + 1);
    let (li, images) = calque_image(doc, 1).expect("un calque Image");
    assert_eq!(li, 0, "tout en bas, sous le dessin");
    assert_eq!(doc.panels[1].layers[li].kind, LayerKind::Bitmap);
    assert_eq!(images.len(), doc.panels[1].frame_count(), "toutes les images du plan");
    assert!(calque_image(doc, 0).is_none() && calque_image(doc, 2).is_none(), "les autres plans n'ont rien");
    // Ajustée au cadre : toute la largeur du cadre, centrée (le centre est rouge ; le haut du cadre est resté du papier).
    let img = image_du_plan(doc, 1, 0);
    let [w, h] = doc.frame;
    let c = img[(h / 2) * w + w / 2];
    assert!(c.r() > 200 && c.g() < 60, "le centre est rouge ({c:?})");
    let (haut, gauche) = (img[2 * w + w / 2], img[(h / 2) * w + 2]);
    assert!(haut.r() > 240 && haut.g() > 240, "le haut du cadre est du papier : l'image garde ses proportions ({haut:?})");
    assert!(gauche.r() > 200 && gauche.g() < 60, "elle va jusqu'au bord gauche du cadre ({gauche:?})");
    // Une seule étape, nommée.
    assert_eq!(pas_d_annulation(&b), etapes + 1);
    assert_eq!(dernier_nom(&b), "Déposer des images");
    // Un calque comme un autre : on peut le cacher.
    b.app.doc.panels[1].layers[0].visible = false;
    let cache = centre_du_plan(&b.app.doc, 1, 0);
    assert!(cache.g() > 240, "caché, il ne se voit plus");
    b.app.doc.panels[1].layers[0].visible = true;
    // Annuler remet le plan comme avant.
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert!(calque_image(&b.app.doc, 1).is_none());
    assert_eq!(b.app.doc.panels[1].layers.len(), avant_calques);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_image_on_a_mark_case_only_fills_that_case_and_a_second_one_replaces_it() {
    let dir = dossier("marque");
    let mut b = ouvert(quatre_plans(), 3);
    // Le plan 0 a deux marques : trois cases (0, 1, 2). On vise la case 1.
    let l = b.app.board_layout();
    let (c1, n) = (l.cards[1], b.app.doc.panels[0].frame_count());
    assert!(c1.frame > 0 && c1.end < n);
    assert!(deposer(&mut b, &[png(&dir, "a.png", 160, 90, [0, 200, 0])], |b| vignette(b, 1).center()));
    let (_, images) = calque_image(&b.app.doc, 0).unwrap();
    assert_eq!(images, (c1.frame..c1.end).collect::<Vec<_>>(), "seulement les images de la case");
    let cel_a = b.app.doc.panels[0].layers[0].frames[c1.frame];
    // Une autre image sur la même case : le calque « Image » est réutilisé.
    assert!(deposer(&mut b, &[png(&dir, "b.png", 160, 90, [0, 0, 200])], |b| vignette(b, 1).center()));
    assert_eq!(b.app.doc.panels[0].layers.iter().filter(|l| l.name == NOM_DU_CALQUE).count(), 1);
    assert_ne!(b.app.doc.panels[0].layers[0].frames[c1.frame], cel_a);
    let c = centre_du_plan(&b.app.doc, 0, c1.frame);
    assert!(c.b() > 150 && c.g() < 60);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn several_images_fill_the_cases_in_sequence_then_make_new_plans() {
    let dir = dossier("suite");
    let mut b = ouvert(quatre_plans(), 3);
    let paths: Vec<PathBuf> = (0..4).map(|k| png(&dir, &format!("{k}.png"), 160, 90, [40 * k as u8 + 20, 0, 0])).collect();
    let etapes = pas_d_annulation(&b);
    // Six cases (0 à 5) : on vise la 4 ; les images vont aux cases 4, 5, puis à deux plans neufs.
    assert!(deposer(&mut b, &paths, |b| vignette(b, 4).center()));
    let doc = &b.app.doc;
    assert_eq!(doc.panels.len(), 6, "deux plans de plus");
    assert!(calque_image(doc, 2).is_some() && calque_image(doc, 3).is_some() && calque_image(doc, 4).is_some() && calque_image(doc, 5).is_some());
    for i in 0..2 {
        assert!(calque_image(doc, i).is_none(), "le plan {i} n'a pas bougé");
    }
    let rouges: Vec<u8> = (2..6).map(|p| centre_du_plan(doc, p, 0).r()).collect();
    assert!(rouges.windows(2).all(|w| w[0] < w[1]), "dans l'ordre du dépôt : {rouges:?}");
    assert_eq!(pas_d_annulation(&b), etapes + 1, "une seule étape pour tout le dépôt");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 4);
    assert!((0..4).all(|i| calque_image(&b.app.doc, i).is_none()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn images_dropped_on_empty_cases_make_one_plan_each_in_order() {
    let dir = dossier("vides");
    let mut b = ouvert_petit(quatre_plans());
    let paths: Vec<PathBuf> = (0..2).map(|k| png(&dir, &format!("{k}.png"), 160, 90, [30 + 100 * k as u8, 100, 0])).collect();
    assert!(deposer(&mut b, &paths, |b| case_vide(b, 0).center()));
    let doc = &b.app.doc;
    assert_eq!(doc.panels.len(), 6);
    let (a, c) = (centre_du_plan(doc, 4, 0), centre_du_plan(doc, 5, 0));
    assert!(a.r() < c.r(), "dans l'ordre : {a:?} puis {c:?}");
    // Sur la deuxième case pâle : une case pâle sautée devient un plan vide, pour garder sa place.
    let mut b2 = ouvert_petit(quatre_plans());
    assert!(deposer(&mut b2, &paths[..1], |b| case_vide(b, 1).center()));
    assert_eq!(b2.app.doc.panels.len(), 6, "un plan vide, puis le plan de l'image");
    assert!(calque_image(&b2.app.doc, 4).is_none() && calque_image(&b2.app.doc, 5).is_some());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_image_dropped_on_a_grey_empty_case_bar_is_one_more_image_of_the_last_plan() {
    let dir = dossier("grise");
    let mut b = ouvert_petit(quatre_plans());
    b.app.vignettes.empty_joins = true;
    let n = b.app.doc.panels[3].frame_count();
    assert!(deposer(&mut b, &[png(&dir, "x.png", 160, 90, [10, 10, 220])], |b| case_vide(b, 0).center()));
    assert_eq!(b.app.doc.panels.len(), 4, "pas de plan de plus");
    assert!(b.app.doc.panels[3].frame_count() > n && !b.app.doc.panels[3].marks.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn images_dropped_between_two_cases_are_inserted_there_after_cutting_the_plan() {
    let dir = dossier("entre");
    let mut b = ouvert(quatre_plans(), 3);
    let ids_avant: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    // La barre entre les cases 2 et 3 : une coupe entre le plan 0 (dont 2 est la dernière image) et le plan 1.
    let paths: Vec<PathBuf> = (0..2).map(|k| png(&dir, &format!("{k}.png"), 160, 90, [200, 30 + 60 * k as u8, 0])).collect();
    assert!(deposer(&mut b, &paths, |b| barre(b, 2).center()));
    let doc = &b.app.doc;
    assert_eq!(doc.panels.len(), 6);
    assert_eq!(doc.panels[0].id, ids_avant[0]);
    assert!(calque_image(doc, 1).is_some() && calque_image(doc, 2).is_some(), "les deux plans neufs sont là, après le plan 0");
    assert_eq!(doc.panels[3].id, ids_avant[1], "les plans suivants ont reculé");
    // Entre deux images d'un même plan (barre grise entre les cases 0 et 1) : le plan est coupé.
    let mut b2 = ouvert(quatre_plans(), 3);
    assert!(deposer(&mut b2, &paths[..1], |b| barre(b, 0).center()));
    assert_eq!(b2.app.doc.panels.len(), 6, "le plan coupé en deux, plus le plan neuf");
    assert!(calque_image(&b2.app.doc, 1).is_some());
    b2.app.run_action("edition.annuler");
    b2.chauffer(3);
    assert_eq!(b2.app.doc.panels.len(), 4, "une seule annulation défait la coupe et l'insertion");
    assert_eq!(b2.app.doc.panels[0].marks.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dropped_references_use_their_files_and_other_files_are_ignored() {
    let dir = dossier("refs");
    let mut b = ouvert(quatre_plans(), 3);
    let f = png(&dir, "ref.png", 200, 100, [0, 180, 180]);
    let r = crate::references::Reference { scale: 1.0, home: Some(dir.clone()), ..crate::references::Reference::image(9001, "ref", "ref.png", [200, 100], eframe::egui::pos2(0.0, 0.0)) };
    b.app.doc.references.push(r);
    let at = vignette(&b, 2).center();
    assert!(b.app.board_take_refs(&[9001], at));
    attendre(&mut b);
    assert!(calque_image(&b.app.doc, 0).is_some_and(|(_, im)| im.contains(&(b.app.doc.panels[0].frame_count() - 1))), "la case 2 est la dernière du plan 0");
    // Un fichier qui n'est pas une image n'est pas pris (le reste de l'application s'en occupe).
    let txt = dir.join("notes.txt");
    std::fs::write(&txt, "bonjour").unwrap();
    assert!(!b.app.board_take_drop(&[txt], Some(at)));
    // Hors de la planche : pas pris non plus.
    assert!(!b.app.board_take_drop(&[f.clone()], Some(pos2(2.0, 2.0))));
    assert!(!b.app.board_take_drop(&[f], None));
    // Sur le fond, hors de toute case : un message, rien ne change.
    let plans = b.app.doc.panels.len();
    let loin = b.app.vignettes.view.right_bottom() - eframe::egui::vec2(4.0, 4.0);
    let g = png(&dir, "g.png", 10, 10, [1, 2, 3]);
    assert!(b.app.board_take_drop(&[g], Some(loin)));
    assert!(b.app.vignettes.depots.is_empty() && b.app.status.contains("Dépose"));
    assert_eq!(b.app.doc.panels.len(), plans);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_locked_storyboard_refuses_new_plans_and_the_big_image_is_prepared_in_the_background() {
    let dir = dossier("gros");
    let mut b = ouvert_petit(quatre_plans());
    let gros = png(&dir, "gros.png", 5000, 3000, [90, 90, 250]);
    let t0 = std::time::Instant::now();
    assert!(b.app.board_take_drop(&[gros.clone()], Some(vignette(&b, 0).center())));
    assert!(t0.elapsed() < std::time::Duration::from_millis(100), "le lâcher rend la main tout de suite ({:?})", t0.elapsed());
    assert!(!b.app.vignettes.depots.is_empty(), "le travail est en fond");
    attendre(&mut b);
    // Le raster est à la taille de la feuille : jamais l'image entière.
    let cel = b.app.doc.panels[0].layers[0].frames[0].unwrap();
    assert_eq!((b.app.doc.cels[&cel].raster.width, b.app.doc.cels[&cel].raster.height), (b.app.doc.width, b.app.doc.height));
    // Storyboard verrouillé : un plan neuf est refusé, rien ne change.
    b.app.doc.storyboard_locked = true;
    let plans = b.app.doc.panels.len();
    let g = png(&dir, "p.png", 20, 20, [1, 2, 3]);
    assert!(deposer(&mut b, &[g], |b| case_vide(b, 0).center()));
    assert_eq!(b.app.doc.panels.len(), plans);
    assert!(b.app.status.to_lowercase().contains("verrouill"), "{}", b.app.status);
    assert!(se_reposer(&mut b, 60) < 60);
    std::fs::remove_dir_all(&dir).ok();
}
