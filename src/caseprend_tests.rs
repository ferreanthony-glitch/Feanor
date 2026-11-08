//! La case qui naît prend le papier (partie 128, `caseprend.rs`) : le scénario des captures d'Anthony, en pixels et en nombres. Un trait tracé
//! au-dessus de cases pâles, puis une case pâle qui devient un plan : le trait reste entier à l'écran, sa part entre dans le plan, Ctrl+Z
//! rend tout d'un coup. Et, trouvé en passant : après Ctrl+Z d'une continuité, un trait dans la case va bien dans son image.

use eframe::egui::vec2;

use crate::traits_continus_tests::{des_plans, planche, relever, repos, sauver, tracer};
use crate::vignettes_selection_tests::{parts, present};

/// Les éléments vectoriels du dessin de l'image 0 du plan `pi`.
fn elements(b: &crate::perf_tests::Banc, pi: usize) -> usize {
    let cel = b.app.doc.panels[pi].layers[0].cel_at(0).expect("un dessin");
    b.app.doc.cels[&cel].vector.as_ref().map_or(0, |v| v.elements.len())
}

/// Les éléments des dessins **que les plans montrent** et les traits du papier. (Un dessin que plus aucun plan ne montre, laissé par l'annulation
/// d'une structure jusqu'à l'élagage, `gel.rs`, ne compte pas : `parts` compte tous les dessins du projet.)
fn parts_montrees(b: &crate::perf_tests::Banc) -> (usize, usize) {
    let doc = &b.app.doc;
    let cels: std::collections::BTreeSet<crate::document::CelId> = doc.panels.iter().flat_map(|p| p.layers.iter().flat_map(|l| l.frames.iter().flatten().copied())).collect();
    let el = cels.iter().filter_map(|c| doc.cels.get(c)?.vector.as_ref()).map(|v| v.elements.len()).sum();
    (el, doc.annotations.traces.len())
}

#[test]
fn a_case_born_under_an_earlier_stroke_takes_its_part_and_the_stroke_stays_whole_on_screen() {
    let Some(mut b) = planche(des_plans(2, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    assert!(l.empty.len() >= 3, "{} cases pâles", l.empty.len());
    let (c1, p1) = (xf.rect(l.cards[1].thumb), xf.rect(l.empty[1]));
    // Un trait du milieu de la case 2 au milieu de la deuxième case pâle, en passant par la première : celle-ci naît (partie 113, le bout d'un
    // trait fait la case suivante) ; le bout au-dessus de la deuxième reste sur le papier.
    let (de, a) = (c1.center(), p1.center());
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 3, "la première case pâle traversée a fait un plan");
    let (el, papier) = parts(&b);
    assert!(papier >= 1, "le bout au-dessus de la case pâle lointaine est sur le papier : {:?}", parts(&b));
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-avant");
    present(&relever(&rgba, de, a), "avant : le trait est entier, le papier se voit");
    let etapes = b.app.history.undo_len();
    // Un petit trait dans cette case pâle (la première, maintenant) : elle devient le plan 4 et prend le bout du papier qui passait là.
    let l2 = b.app.board_layout();
    let p = xf.rect(l2.empty[0]);
    assert!((p.center() - p1.center()).length() < 2.0, "la case pâle lointaine est devenue la première : {:?} / {:?}", p.center(), p1.center());
    tracer(&mut b, p.center() + vec2(-15.0, 25.0), p.center() + vec2(15.0, 25.0));
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 4);
    let (el2, papier2) = parts(&b);
    assert_eq!(papier2, 0, "le papier n'a plus rien là : la case l'a pris ({el2} éléments)");
    assert_eq!(el2, el + 2, "le bout repris et le petit trait sont dans les dessins");
    assert_eq!(elements(&b, 3), 2, "le plan né porte le bout repris et son petit trait");
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-apres");
    present(&relever(&rgba, de, a), "après : la case née ne cache plus le trait");
    // Une seule étape pour la case, son trait et la reprise : Ctrl+Z défait tout et rend le bout au papier.
    assert_eq!(b.app.history.undo_len(), etapes + 1, "{:?}", b.app.history.names().0);
    b.app.undo();
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 3);
    assert_eq!(parts(&b), (el, papier), "le papier a repris son bout");
}

#[test]
fn a_continuity_without_strokes_on_the_paper_is_one_step_as_before() {
    let Some(mut b) = planche(des_plans(1, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let etapes = b.app.history.undo_len();
    assert!(b.app.board_continuite_de(None));
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert_eq!(b.app.doc.panels[0].marks.len(), 1);
    b.app.undo();
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].marks.len(), 0);
}

/// Trouvé en passant (2026-11-08, en écrivant l'essai de la continuité) : après Ctrl+Z d'une continuité, l'image courante restait au-delà de la
/// fin du plan ; un trait dans la case allait alors dans une image neuve au bout du plan, qui s'allongeait d'une image, et la vignette de la
/// bande de montage restait vide. L'image courante est ramenée dans le plan après l'historique (`App::clamp_nav`).
#[test]
fn after_undoing_a_continuity_a_stroke_in_the_case_goes_into_its_image() {
    let Some(mut b) = planche(des_plans(1, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let c0 = xf.rect(l.cards[0].thumb);
    let images = b.app.doc.panels[0].frame_count();
    assert!(b.app.board_continuite_de(None));
    b.app.undo();
    b.chauffer(2);
    assert!(b.app.cur_frame < images, "l'image courante est dans le plan après l'annulation : {} / {images}", b.app.cur_frame);
    let (de, a) = (c0.center() + vec2(-40.0, 0.0), c0.center() + vec2(40.0, 0.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels[0].frame_count(), images, "le plan garde sa durée");
    assert_eq!(elements(&b, 0), 1, "le trait est dans l'image de la case");
    let id = b.app.doc.panels[0].id;
    let rouges = b.app.thumbs.image_of(id, crate::thumbs::STRIP_WIDTH).map(|im| im.pixels.iter().filter(|p| p.r() as i32 > p.g() as i32 + 60).count());
    assert!(rouges.is_some_and(|n| n > 0), "la vignette de la bande de montage montre le trait : {rouges:?}");
}

#[test]
fn the_continuity_bar_also_takes_the_paper() {
    let Some(mut b) = planche(des_plans(1, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (c0, p1) = (xf.rect(l.cards[0].thumb), xf.rect(l.empty[1]));
    // Un trait de la case 1 au milieu de la deuxième case pâle : la première case pâle naît (partie 113), le bout au-dessus de la deuxième reste
    // sur le papier ; cette deuxième est maintenant la première case pâle, devant la barre de continuité.
    let (de, a) = (c0.center(), p1.center());
    tracer(&mut b, de, a);
    repos(&mut b);
    let (el, papier) = parts_montrees(&b);
    assert!(papier >= 1 && b.app.doc.panels.len() == 2, "le bout au-dessus de la case pâle lointaine reste sur le papier : {:?}, {} plans", (el, papier), b.app.doc.panels.len());
    let l2 = b.app.board_layout();
    assert!((xf.rect(l2.empty[0]).center() - p1.center()).length() < 2.0, "la case pâle lointaine est devenue la première");
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-continuite-avant");
    present(&relever(&rgba, de, a), "avant la continuité : le trait est entier");
    // La barre de continuité : cette case devient une image de plus du plan 2 et prend le bout du papier qui passait dans son champ.
    let etapes = b.app.history.undo_len();
    assert!(b.app.board_continuite_de(None));
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(b.app.doc.panels[1].marks.len(), 1, "une image de plus du plan 2");
    let (el2, papier2) = parts_montrees(&b);
    assert_eq!(papier2, 0, "le papier n'a plus rien là : la case l'a pris ({el2} éléments)");
    // (La case née montre une copie du dessin de la précédente, plus la part reprise, partagée entre les deux images selon leur champ.)
    assert!(el2 >= el + 2, "la case née a reçu la part : {:?} → {:?}", (el, papier), (el2, papier2));
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-continuite");
    present(&relever(&rgba, de, a), "continuité : le trait reste entier à travers la case née");
    // Ctrl+Z : une étape, tout revient.
    assert_eq!(b.app.history.undo_len(), etapes + 1, "{:?}", b.app.history.names().0);
    b.app.undo();
    repos(&mut b);
    assert_eq!(parts_montrees(&b), (el, papier), "le papier a repris son bout, les dessins sont comme avant");
    assert_eq!(b.app.doc.panels[1].marks.len(), 0);
}
