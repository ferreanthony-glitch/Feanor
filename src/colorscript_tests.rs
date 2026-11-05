//! Essais de l'export du color script (`colorscript.rs`, `colorscriptui.rs`) : une image par plan (ou par case), rangées par
//! séquence, bande des couleurs dominantes facultative, fond, PNG et PDF, tâche de fond.

use image::{Rgb, RgbImage};

use super::*;
use crate::perf_tests::Banc;
use crate::vignettes_tests::{deux_sequences, ouvert, quatre_plans};

pub(super) fn tout(doc: &Document) -> Vec<usize> {
    (0..doc.panels.len()).collect()
}

pub(super) fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-colorscript-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Un projet de `n` plans dans une séquence ; `deux_sequences` en a deux.
pub(super) fn plans(n: usize) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    for _ in 1..n {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

#[test]
fn one_image_per_plan_by_default_grouped_by_sequence() {
    let doc = deux_sequences();
    let spec = construire(&doc, &tout(&doc), &Options::default());
    assert_eq!(spec.nombre(), 4, "une par plan, même si un plan a des marques d'image");
    assert_eq!(spec.blocs.len(), 2, "deux séquences");
    assert_eq!(spec.blocs[0].entrees.len(), 3);
    assert_eq!(spec.blocs[1].entrees.len(), 1);
    assert_eq!(spec.images(), vec![(0, 0), (1, 0), (2, 0), (3, 0)], "la première image de chaque plan");
    assert!(spec.options.fond == [0, 0, 0] && !spec.options.bande && spec.bande_h == 0, "noir, sans bande d'office");
    assert!(spec.options.titres && !spec.options.numeros);
}

#[test]
fn one_image_per_case_on_request_and_first_or_middle_image() {
    let doc = deux_sequences();
    let par_case = construire(&doc, &tout(&doc), &Options { par_case: true, ..Options::default() });
    assert_eq!(par_case.nombre(), 6, "le plan 0 a trois cases");
    let cases = crate::board::plan_boards(&doc, 0);
    let attendu: Vec<(usize, usize)> = cases.iter().map(|c| (0, c.1)).collect();
    assert_eq!(&par_case.images()[..3], &attendu[..]);
    // L'image du milieu du plan, ou de la case.
    let n = doc.panels[1].frame_count();
    let milieu = construire(&doc, &tout(&doc), &Options { prise: Prise::Milieu, ..Options::default() });
    assert_eq!(milieu.images()[1], (1, (n - 1) / 2));
    let milieu_cases = construire(&doc, &tout(&doc), &Options { prise: Prise::Milieu, par_case: true, ..Options::default() });
    for (k, (a, b)) in cases.iter().map(|c| (c.1, c.2)).enumerate() {
        assert_eq!(milieu_cases.images()[k], (0, (a + b - 1) / 2), "milieu de la case {k}");
    }
}

#[test]
fn rows_are_tight_wrap_at_the_limit_and_sequences_have_a_title() {
    let doc = plans(30);
    let o = Options { largeur: 100, espace: 3, par_ligne: 12, ..Options::default() };
    let spec = construire(&doc, &tout(&doc), &o);
    assert_eq!(spec.blocs.len(), 1);
    let e = &spec.blocs[0].entrees;
    assert_eq!(e.len(), 30);
    // Serrées : une image tous les largeur + espace pixels, 12 par rangée.
    assert_eq!(e[1].x - e[0].x, 103);
    assert_eq!(e[11].x - e[0].x, 11 * 103);
    assert_eq!(e[12].x, e[0].x, "la rangée suivante repart à gauche");
    assert_eq!(e[12].y - e[0].y, spec.cellule[1] as i32 + 3, "rangées serrées");
    assert_eq!(e[24].y - e[12].y, spec.cellule[1] as i32 + 3);
    assert_eq!(spec.cellule, [100, 56]);
    // L'image entière contient tout, sans rien qui dépasse.
    for en in e {
        assert!(en.x >= 0 && en.y >= spec.blocs[0].titre_y && (en.x + 100) as u32 <= spec.taille[0] && (en.y + 56) as u32 <= spec.taille[1]);
    }
    // Moins de plans que la limite : la rangée est celle des plans (pas de vide à droite).
    let petit = construire(&plans(5), &[0, 1, 2, 3, 4], &o);
    assert_eq!(petit.taille[0], petit.marge as u32 * 2 + 5 * 100 + 4 * 3);
    // Deux séquences : la seconde commence plus bas, sous son titre.
    let doc = deux_sequences();
    let s = construire(&doc, &tout(&doc), &Options::default());
    assert!(s.blocs[1].titre_y > s.blocs[0].entrees[0].y + s.cellule[1] as i32, "la deuxième séquence est sous la première");
    assert!(s.blocs[1].entrees[0].y > s.blocs[1].titre_y, "ses images sont sous son titre");
    assert_eq!(s.blocs[0].titre, doc.sequence_label(doc.sequence_of(0)));
    // Sans titres : plus serré.
    let sans = construire(&doc, &tout(&doc), &Options { titres: false, ..Options::default() });
    assert!(sans.taille[1] < s.taille[1]);
}

#[test]
fn plan_numbers_only_on_the_first_case_of_each_plan() {
    let doc = deux_sequences();
    let o = Options { numeros: true, par_case: true, ..Options::default() };
    let spec = construire(&doc, &tout(&doc), &o);
    let e: Vec<&Entree> = spec.blocs.iter().flat_map(|b| &b.entrees).collect();
    assert_eq!(e.iter().filter(|x| x.etiquette.is_some()).count(), 4, "un numéro par plan");
    assert!(e[0].etiquette.is_some() && e[1].etiquette.is_none() && e[2].etiquette.is_none());
    let sans = construire(&doc, &tout(&doc), &Options { numeros: false, par_case: true, ..Options::default() });
    assert!(sans.blocs.iter().flat_map(|b| &b.entrees).all(|x| x.etiquette.is_none()));
}

#[test]
fn the_dominant_colour_strip_is_off_by_default_and_adds_height_when_asked() {
    let doc = deux_sequences();
    let sans = construire(&doc, &tout(&doc), &Options::default());
    let avec = construire(&doc, &tout(&doc), &Options { bande: true, ..Options::default() });
    assert_eq!(sans.bande_h, 0);
    assert!(avec.bande_h > 0 && avec.taille[1] > sans.taille[1]);
    assert_eq!(avec.cellule, sans.cellule, "l'image elle-même ne change pas");
}

#[test]
fn dominant_colours_are_few_weighted_sorted_and_merged_when_close() {
    // Moitié rouge, quart vert, quart presque rouge (à fusionner avec le rouge).
    let mut img = RgbImage::new(40, 40);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = if x < 20 { Rgb([220, 20, 20]) } else if y < 20 { Rgb([20, 200, 40]) } else { Rgb([225, 25, 25]) };
    }
    let d = dominantes(&img, 5);
    assert_eq!(d.len(), 2, "le rouge et le presque rouge n'en font qu'une : {d:?}");
    assert!((d[0].1 - 0.75).abs() < 0.05 && (d[1].1 - 0.25).abs() < 0.05);
    assert!(d[0].0[0] > 200 && d[1].0[1] > 150, "{d:?}");
    assert!((d.iter().map(|c| c.1).sum::<f32>() - 1.0).abs() < 1e-4);
    // Une image unie : une couleur ; au plus n couleurs.
    assert_eq!(dominantes(&RgbImage::from_pixel(8, 8, Rgb([5, 5, 5])), 5).len(), 1);
    let mut arc = RgbImage::new(64, 8);
    for (x, _, p) in arc.enumerate_pixels_mut() {
        *p = Rgb([(x * 4) as u8, (255 - x * 4) as u8, (x * 16 % 256) as u8]);
    }
    assert!(dominantes(&arc, 3).len() <= 3);
    assert!(dominantes(&RgbImage::new(0, 0), 5).is_empty());
}

fn cellules(spec: &Spec, couleurs: &[[u8; 3]]) -> Vec<Cellule> {
    (0..spec.nombre())
        .map(|k| {
            let c = couleurs[k % couleurs.len()];
            let img = RgbImage::from_pixel(spec.cellule[0], spec.cellule[1], Rgb(c));
            let palette = if spec.bande_h > 0 { dominantes(&img, 5) } else { Vec::new() };
            (img, palette)
        })
        .collect()
}

#[test]
fn the_image_has_the_background_the_images_the_titles_and_optionally_the_strip() {
    let doc = deux_sequences();
    let couleurs = [[200, 30, 30], [30, 200, 30], [30, 30, 200], [200, 200, 30]];
    let o = Options::default();
    let spec = construire(&doc, &tout(&doc), &o);
    let img = composer(&spec, &cellules(&spec, &couleurs)).unwrap();
    assert_eq!(img.dimensions(), (spec.taille[0], spec.taille[1]));
    assert_eq!(img.get_pixel(0, 0).0, [0, 0, 0], "fond noir");
    for (k, e) in spec.blocs.iter().flat_map(|b| &b.entrees).enumerate() {
        assert_eq!(img.get_pixel((e.x + 10) as u32, (e.y + 10) as u32).0, couleurs[k], "image {k}");
    }
    // Le titre de la première séquence est écrit en clair sur le fond noir.
    let b0 = &spec.blocs[0];
    let titre_zone = (0..(spec.taille[0] / 3)).any(|dx| (0..spec.titre_taille as u32).any(|dy| img.get_pixel(spec.marge as u32 + dx, b0.titre_y as u32 + dy).0[0] > 150));
    assert!(titre_zone, "le titre est écrit");
    let sans_titre = composer(&construire(&doc, &tout(&doc), &Options { titres: false, ..o.clone() }), &cellules(&construire(&doc, &tout(&doc), &Options { titres: false, ..o.clone() }), &couleurs)).unwrap();
    assert!(sans_titre.height() < img.height());
    // Sous l'image, rien sans bande : le fond.
    let e0 = &b0.entrees[0];
    assert_eq!(img.get_pixel((e0.x + 10) as u32, (e0.y + spec.cellule[1] as i32 + 1) as u32).0, [0, 0, 0]);
    // Un autre fond (clair) : le fond change, le titre devient sombre.
    let clair = Options { fond: [240, 240, 235], ..o.clone() };
    let sc = construire(&doc, &tout(&doc), &clair);
    let ic = composer(&sc, &cellules(&sc, &couleurs)).unwrap();
    assert_eq!(ic.get_pixel(0, 0).0, [240, 240, 235]);
    assert!((0..sc.taille[0] / 3).any(|dx| (0..sc.titre_taille as u32).any(|dy| ic.get_pixel(sc.marge as u32 + dx, sc.blocs[0].titre_y as u32 + dy).0[0] < 120)));
    // Avec la bande : les couleurs de l'image, sous elle.
    let ob = Options { bande: true, ..o };
    let sb = construire(&doc, &tout(&doc), &ob);
    let ib = composer(&sb, &cellules(&sb, &couleurs)).unwrap();
    let e0 = &sb.blocs[0].entrees[0];
    assert_eq!(ib.get_pixel((e0.x + 10) as u32, (e0.y + sb.cellule[1] as i32 + 1) as u32).0, couleurs[0], "la bande reprend la couleur dominante");
}

#[test]
fn a_huge_script_is_reduced_to_the_bounds_and_says_so() {
    let doc = plans(40);
    let o = Options { largeur: 2000, espace: 2, par_ligne: 200, ..Options::default() };
    let spec = construire(&doc, &tout(&doc), &o);
    assert!(spec.taille[0] as f32 <= crate::planche::MAX_COTE && spec.taille[0] as f32 * spec.taille[1] as f32 <= crate::planche::MAX_PIXELS, "{:?}", spec.taille);
    assert!(spec.reduit.as_deref().is_some_and(|r| r.contains("réduites")));
    assert!(spec.options.largeur < 2000);
    let normal = construire(&doc, &tout(&doc), &Options::default());
    assert!(normal.reduit.is_none());
}

#[test]
fn the_files_are_valid() {
    let doc = deux_sequences();
    let spec = construire(&doc, &tout(&doc), &Options::default());
    let img = composer(&spec, &cellules(&spec, &[[120, 60, 200]])).unwrap();
    let meta = Meta { titre: "T".into(), episode: String::new(), version: String::new(), date: "2026-11-05".into(), papier: Papier::A4 };
    let png = encoder(&img, Sortie::Png, &meta).unwrap();
    assert_eq!(image::load_from_memory(&png).unwrap().to_rgb8(), img);
    let pdf = String::from_utf8_lossy(&encoder(&img, Sortie::Pdf, &meta).unwrap()).to_string();
    assert!(pdf.starts_with("%PDF") && pdf.contains("/Count 1") && pdf.contains("/DCTDecode"));
}

pub(super) fn lancer(b: &mut Banc, job: crate::export::ExportJob) {
    b.app.start_job(job);
    let debut = std::time::Instant::now();
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && debut.elapsed() < std::time::Duration::from_secs(60) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    b.chauffer(2);
}

#[test]
fn the_job_writes_the_script_in_the_background_and_cancel_cleans_up() {
    let dir = dossier("tache");
    let mut b = ouvert(quatre_plans(), 3);
    let plans = tout(&b.app.doc);
    let sortie = dir.join("cs.png");
    let o = Options { bande: true, numeros: true, ..Options::default() };
    let job = b.app.colorscript_job(&plans, &o, &sortie).expect("tâche");
    lancer(&mut b, job);
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
    assert!(fin.as_ref().is_ok_and(|m| m.contains("Color script exporté")), "{fin:?}");
    let spec = construire(&b.app.doc, &plans, &o);
    let img = image::open(&sortie).expect("PNG").to_rgb8();
    assert_eq!(img.dimensions(), (spec.taille[0], spec.taille[1]));
    assert_eq!(img.get_pixel(0, 0).0, [0, 0, 0]);
    assert!(!dir.join("cs.png.part").exists());
    // Annuler : rien ne reste, l'ancien fichier non plus n'est pas touché.
    b.app.export_job = None;
    let autre = dir.join("autre.png");
    std::fs::write(&autre, b"avant").unwrap();
    let job = b.app.colorscript_job(&plans, &Options::default(), &autre).unwrap();
    b.app.start_job(job);
    if let Some(j) = b.app.export_job.as_mut() {
        j.cancel();
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    b.chauffer(3);
    assert!(!dir.join("autre.png.part").exists());
    assert!(std::fs::read(&autre).unwrap() == b"avant", "l'ancien fichier est intact");
    assert!(b.app.colorscript_job(&[], &Options::default(), &autre).is_err());
    // Le PDF.
    b.app.export_job = None;
    let pdf = dir.join("cs.pdf");
    let job = b.app.colorscript_job(&plans, &Options { sortie: Sortie::Pdf, ..Options::default() }, &pdf).unwrap();
    lancer(&mut b, job);
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    std::fs::remove_dir_all(&dir).ok();
}
