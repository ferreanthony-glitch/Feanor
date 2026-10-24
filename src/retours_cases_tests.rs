//! Retour de terrain d'Anthony (2026-10-24, branche `retours-terrain`) : les cases des Vignettes qui se réunissent après un passage par le
//! Storyboard (une clé retouchée au montage laissait sa case en arrière ; `cases.rs`).

use eframe::egui::vec2;

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::vignettes_tests::{case_vide, glisser, ouvert, quatre_plans, vague, vignette};
use crate::workspace::Espace;

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-retours-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}


// ------------------------------------------------------------------------------------ 1. les cases suivent leurs clés

/// Les cases (images de storyboard) de chaque plan : (première image, fin) de chacune, d'après la planche.
fn cases_de(doc: &Document) -> Vec<Vec<(usize, usize)>> {
    (0..doc.panels.len()).map(|i| crate::board::plan_boards(doc, i).into_iter().map(|(_, a, b)| (a, b)).collect()).collect()
}

/// Les images où un dessin commence, sur le calque « Dessin » (le premier) du plan `i`.
fn cles_de(doc: &Document, i: usize) -> Vec<usize> {
    let l = &doc.panels[i].layers[0];
    (0..l.frames.len()).filter(|&f| l.frames[f].is_some() && (f == 0 || l.frames[f - 1] != l.frames[f])).collect()
}

fn marques(doc: &Document, i: usize) -> Vec<usize> {
    doc.panels[i].marks.iter().map(|m| m.frame).collect()
}

/// Le dessin que chaque case montre à sa première image : deux cases qui montrent le même sont « réunies » à l'écran.
fn dessins_des_cases(doc: &Document, i: usize) -> Vec<Option<CelId>> {
    let l = &doc.panels[i].layers[0];
    crate::board::plan_boards(doc, i).into_iter().map(|(_, debut, _)| l.frames.get(debut).copied().flatten()).collect()
}

fn toutes_differentes(dessins: &[Option<CelId>]) -> bool {
    dessins.iter().all(Option::is_some) && dessins.iter().enumerate().all(|(k, d)| dessins[..k].iter().all(|a| a != d))
}

/// La vue Vignettes avec le dernier plan dessiné sur `n` cases de continuité (la barre grise devant la case vide), une clé de
/// dessin par case.
fn plan_en_continuite(n: usize) -> Banc {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 5);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    for _ in 1..n {
        b.app.vignettes.empty_joins = true;
        let e = case_vide(&b, 0);
        glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 5.0), 10));
    }
    b
}

/// Le plan de continuité, ouvert dans l'espace Storyboard ; son rang.
fn au_storyboard(n: usize) -> (Banc, usize) {
    let mut b = plan_en_continuite(n);
    let d = b.app.doc.panels.len() - 1;
    b.app.select_panel(d);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(6);
    (b, d)
}

fn retour_aux_vignettes(b: &mut Banc) {
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(8);
}

/// Glisse la clé du calque `li` qui commence à l'image `de` jusqu'à l'image `vers`, comme à la souris dans le montage (les clés
/// d'après suivent).
fn glisser_la_cle(b: &mut Banc, plan: usize, li: usize, de: usize, vers: usize) {
    let k = b.app.key_ref(plan, li, de);
    assert!(k.is_some(), "une clé commence à l'image {de}");
    b.app.click_key(k, false, false);
    assert!(b.app.begin_keys_drag(de));
    b.app.drag_keys(vers);
    b.app.end_keys_drag();
    b.chauffer(2);
}

#[test]
fn a_trip_through_the_storyboard_keeps_every_case_of_a_plan() {
    let (mut b, d) = au_storyboard(3);
    assert_eq!(cases_de(&b.app.doc)[d], vec![(0, 48), (48, 96), (96, 144)]);
    assert_eq!(cles_de(&b.app.doc, d), vec![0, 48, 96], "une clé par case, au montage");
    retour_aux_vignettes(&mut b);
    assert_eq!(cases_de(&b.app.doc)[d], vec![(0, 48), (48, 96), (96, 144)]);
    assert!(toutes_differentes(&dessins_des_cases(&b.app.doc, d)), "chaque case montre son propre dessin");
}

#[test]
fn dragging_a_key_in_the_storyboard_takes_its_case_with_it() {
    let (mut b, d) = au_storyboard(3);
    // La limite entre la première et la deuxième case tirée de 12 images : les clés d'après suivent.
    glisser_la_cle(&mut b, d, 0, 48, 60);
    assert_eq!(cles_de(&b.app.doc, d), vec![0, 60, 108]);
    assert_eq!(marques(&b.app.doc, d), vec![60, 108], "chaque case a suivi sa clé");
    retour_aux_vignettes(&mut b);
    assert_eq!(cases_de(&b.app.doc)[d], vec![(0, 60), (60, 108), (108, 144)]);
    assert!(toutes_differentes(&dessins_des_cases(&b.app.doc, d)), "chaque case montre son propre dessin : {:?}", dessins_des_cases(&b.app.doc, d));
    // Une seule étape d'annulation défait la clé et sa case.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(cles_de(&b.app.doc, d), vec![0, 48, 96]);
    assert_eq!(marques(&b.app.doc, d), vec![48, 96]);
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    assert_eq!(marques(&b.app.doc, d), vec![60, 108]);
}

#[test]
fn the_cases_stay_with_their_keys_when_the_project_is_saved_and_reopened() {
    let (mut b, d) = au_storyboard(3);
    glisser_la_cle(&mut b, d, 0, 48, 60);
    retour_aux_vignettes(&mut b);
    let dir = dossier("cases-enregistrement");
    let manifest = dir.join("P.shot");
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifest).expect("réouverture");
    assert_eq!(cles_de(&relu.doc, d), vec![0, 60, 108]);
    assert_eq!(marques(&relu.doc, d), vec![60, 108]);
    assert!(toutes_differentes(&dessins_des_cases(&relu.doc, d)));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn deleting_the_key_of_a_case_in_the_storyboard_reunites_it_with_the_previous_case_and_keeps_its_text() {
    let (mut b, d) = au_storyboard(3);
    b.app.doc.panels[d].marks[0].action = "Léa entre".into();
    let k = b.app.key_ref(d, 0, 48);
    b.app.click_key(k, false, false);
    b.app.delete_keys(false);
    b.chauffer(2);
    assert_eq!(cles_de(&b.app.doc, d), vec![0, 96], "la clé de la deuxième case n'est plus");
    assert_eq!(marques(&b.app.doc, d), vec![96], "sa case est réunie à la première");
    assert!(b.app.doc.panels[d].action.contains("Léa entre"), "son texte n'est pas perdu : {:?}", b.app.doc.panels[d].action);
    retour_aux_vignettes(&mut b);
    assert_eq!(cases_de(&b.app.doc)[d], vec![(0, 96), (96, 144)]);
    assert!(toutes_differentes(&dessins_des_cases(&b.app.doc, d)));
    // Annuler rend la clé et la case, avec son texte.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(marques(&b.app.doc, d), vec![48, 96]);
    assert_eq!(b.app.doc.panels[d].marks[0].action, "Léa entre");
    assert_eq!(cles_de(&b.app.doc, d), vec![0, 48, 96]);
}

#[test]
fn blanking_the_images_of_a_case_in_the_storyboard_keeps_the_case() {
    let (mut b, d) = au_storyboard(3);
    let k = b.app.key_ref(d, 0, 48);
    b.app.click_key(k, false, false);
    b.app.delete_keys(true);
    b.chauffer(2);
    assert_eq!(marques(&b.app.doc, d), vec![48, 96], "la case reste : ses images sont vides, pas réunies à l'autre");
}

#[test]
fn a_case_stays_glued_to_the_other_layers_when_only_one_layer_moves_its_key() {
    let (mut b, d) = au_storyboard(3);
    // Un deuxième calque de dessin, avec une clé à chaque case.
    let mut l2 = Layer::new("Calque 2", LayerKind::Bitmap);
    for _ in 0..3 {
        let c = b.app.doc.new_cel(LayerKind::Bitmap);
        l2.frames.extend(std::iter::repeat_n(Some(c), 48));
    }
    b.app.doc.panels[d].layers.push(l2);
    glisser_la_cle(&mut b, d, 0, 48, 60);
    assert_eq!(cles_de(&b.app.doc, d), vec![0, 60, 108]);
    assert_eq!(marques(&b.app.doc, d), vec![48, 96], "l'autre calque tient encore ses clés aux cases : elles restent");
}

