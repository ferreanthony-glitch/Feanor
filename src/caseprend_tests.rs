//! La case qui naît prend le papier (partie 128, `caseprend.rs`) : le scénario des captures d'Anthony, en pixels et en nombres. Un trait tracé
//! sur le papier au-dessus de cases pâles, puis une case pâle qui devient un plan : le trait reste entier à l'écran, **il entre entier dans le
//! plan** (partie 131 : avant, seule sa part dans le champ de la case), Ctrl+Z rend tout d'un coup. Et, trouvé en passant : après Ctrl+Z d'une
//! continuité, un trait dans la case va bien dans son image.

use eframe::egui::{pos2, vec2, Pos2};

use crate::traits_continus_tests::{des_plans, planche, relever, repos, sauver, tracer};
use crate::vignettes_selection_tests::{parts, present};

/// Un point du papier au-dessus de la case pâle `e` (écran) : dans sa marge, hors du champ de toute case. (La marge d'une case pâle qui n'est
/// pas la première de sa séquence est du papier : `boardui::board_press`.)
fn au_dessus_de_la_pale(b: &crate::perf_tests::Banc, e: usize) -> Pos2 {
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let r = xf.rect(l.empty[e]);
    let p = pos2(r.center().x, r.top() - r.width() * 0.06);
    let sur_la_planche = xf.to_board(p);
    assert!(!crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction()).iter().any(|c| c.contains(sur_la_planche)), "hors du champ de toute case");
    assert!(crate::board::hit(&l, sur_la_planche, 1.0) == crate::board::Hit::Nothing, "sur le papier");
    p
}

/// Les points du trait `rang` du dessin de l'image 0 du plan `pi`, dans son dessin.
fn points(b: &crate::perf_tests::Banc, pi: usize, rang: usize) -> Vec<Pos2> {
    let Some(cel) = b.app.doc.panels[pi].layers[0].cel_at(0) else { return Vec::new() };
    match b.app.doc.cels[&cel].vector.as_ref().and_then(|v| v.elements.get(rang)) {
        Some(crate::vector::Element::Stroke { points, .. }) => points.iter().map(|p| p.pos()).collect(),
        _ => Vec::new(),
    }
}

/// Les éléments vectoriels du dessin de l'image 0 du plan `pi` (aucun s'il n'a pas de dessin).
fn elements(b: &crate::perf_tests::Banc, pi: usize) -> usize {
    let Some(cel) = b.app.doc.panels[pi].layers[0].cel_at(0) else { return 0 };
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

/// Partie 131 (avant : la case née prenait la part du trait dans son champ, le reste restait sur le papier). Elle prend le trait **entier**,
/// à sa place exacte, au-delà de son cadre aussi.
#[test]
fn a_case_born_under_an_earlier_stroke_takes_it_whole_and_the_stroke_stays_whole_on_screen() {
    let Some(mut b) = planche(des_plans(2, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    assert!(l.empty.len() >= 3, "{} cases pâles", l.empty.len());
    let p1 = xf.rect(l.empty[1]);
    // Un trait du papier : du dessus de la troisième case pâle au milieu de la deuxième. Aucune case ne naît, rien n'entre dans un plan.
    let (de, a) = (au_dessus_de_la_pale(&b, 2), p1.center());
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 2, "un trait du papier ne fait naître aucun plan, même s'il finit dans une case pâle");
    assert_eq!(parts(&b), (0, 1), "le trait est sur le papier, entier");
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-avant");
    present(&relever(&rgba, de, a), "avant : le trait du papier se voit, entier");
    let etapes = b.app.history.undo_len();
    // Un petit trait dans la deuxième case pâle : elle devient le plan 4 (un plan vide naît devant elle) et prend le trait du papier, entier.
    tracer(&mut b, p1.center() + vec2(-15.0, 25.0), p1.center() + vec2(15.0, 25.0));
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 4);
    assert_eq!(parts(&b), (2, 0), "le papier n'a plus rien : la case née a pris le trait, en un seul élément, et porte son petit trait");
    assert_eq!(elements(&b, 3), 2, "le plan né porte le trait repris et son petit trait");
    assert_eq!(elements(&b, 2), 0, "le plan vide né devant lui n'a rien pris : le trait ne passait pas dans son champ");
    // Le trait repris est entier dans le dessin du plan : son départ est au-dessus de la case voisine, à droite et au-dessus du cadre de la sienne.
    let l3 = b.app.board_layout();
    let pose = crate::horschamp::pose_de_la_case(&b.app.doc, &l3, 3).expect("la case née");
    let cadre = l3.cards[3].thumb;
    let repris: Vec<Pos2> = [points(&b, 3, 0), points(&b, 3, 1)].into_iter().max_by_key(Vec::len).unwrap().into_iter().map(|p| pose.planche(p)).collect();
    assert!(repris.iter().any(|p| p.x > cadre.right() + cadre.width() * 0.5) && repris.iter().any(|p| p.y < cadre.top()), "le trait garde ses points hors du cadre ({cadre:?}) : de {:?} à {:?}", repris.first(), repris.last());
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-apres");
    present(&relever(&rgba, de, a), "après : le trait reste entier à l'écran, la case née le montre au-delà de son cadre");
    // Une seule étape pour les cases, le petit trait et la reprise : Ctrl+Z défait tout et rend le trait au papier.
    assert_eq!(b.app.history.undo_len(), etapes + 1, "{:?}", b.app.history.names().0);
    b.app.undo();
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(parts_montrees(&b), (0, 1), "le papier a repris son trait");
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
    let p0 = xf.rect(l.empty[0]);
    // Un trait du papier : du dessus de la deuxième case pâle jusque dans la première, celle qui est devant la barre de continuité (son milieu
    // est au-dessus de la première : c'est à elle qu'il est attaché, `annotations::Ancre`).
    let (de, a) = (au_dessus_de_la_pale(&b, 1), p0.center() - vec2(p0.width() * 0.3, 0.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!((parts_montrees(&b), b.app.doc.panels.len()), ((0, 1), 1), "le trait est sur le papier, entier ; aucun plan n'est né");
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-continuite-avant");
    present(&relever(&rgba, de, a), "avant la continuité : le trait est entier");
    // La barre de continuité : cette case devient une image de plus du plan 1 et prend le trait du papier qui passait dans son champ, entier.
    let etapes = b.app.history.undo_len();
    assert!(b.app.board_continuite_de(None));
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 1);
    assert_eq!(b.app.doc.panels[0].marks.len(), 1, "une image de plus du plan 1");
    assert_eq!(parts_montrees(&b), (1, 0), "le papier n'a plus rien : la case née a pris le trait, entier, en un seul élément");
    let image = b.app.doc.panels[0].marks[0].frame;
    let cel = b.app.doc.panels[0].layers[0].cel_at(image).expect("le dessin de la case née");
    assert_eq!(b.app.doc.cels[&cel].vector.as_ref().map_or(0, |v| v.elements.len()), 1, "il est dans le dessin de l'image de la case née");
    // (La case née se colle à la précédente, la rangée se resserre, et le trait du papier a suivi la case à laquelle il était attaché : la case
    // l'a pris là où il était alors. C'est la règle du papier, partie 128.)
    let d = b.app.vignettes.xf.rect(b.app.board_layout().cards[1].thumb).min - p0.min;
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "case-prend-continuite");
    present(&relever(&rgba, de + d, a + d), "continuité : le trait reste entier à l'écran, la case née le montre au-delà de son cadre");
    // Ctrl+Z : une étape, tout revient.
    assert_eq!(b.app.history.undo_len(), etapes + 1, "{:?}", b.app.history.names().0);
    b.app.undo();
    repos(&mut b);
    assert_eq!(parts_montrees(&b), (0, 1), "le papier a repris son trait");
    assert_eq!(b.app.doc.panels[0].marks.len(), 0);
}

/// Partie 131 (avant, partie 113 : le bout d'un trait qui tombait dans la case pâle suivante la faisait naître). Un trait appartient à la case
/// où il commence : son bout ne fait plus rien naître, il reste entier dans son plan et se voit par-dessus la case pâle.
#[test]
fn the_tip_of_a_stroke_no_longer_makes_the_next_pale_case() {
    let Some(mut b) = planche(des_plans(2, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (c1, p0) = (xf.rect(l.cards[1].thumb), xf.rect(l.empty[0]));
    let etapes = b.app.history.undo_len();
    let (de, a) = (c1.center(), p0.center());
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 2, "aucun plan ne naît");
    assert_eq!(parts(&b), (1, 0), "le trait est entier dans le dessin du plan où il commence ; rien sur le papier");
    assert_eq!(elements(&b, 1), 1);
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "bout-sans-naissance");
    present(&relever(&rgba, de, a), "le trait se voit en entier, jusque dans la case pâle");
}

/// Partie 131 : une case pâle naît quand on pose le stylet dedans **ou dans sa marge** (la première case pâle de sa séquence ; hors de la zone
/// de la barre devant elle, qui reste une barre) ; la marge d'une case pâle plus lointaine est du papier.
#[test]
fn pressing_in_the_margin_of_the_next_pale_case_makes_its_plan_and_a_farther_margin_is_paper() {
    let Some(mut b) = planche(des_plans(2, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let p0 = xf.rect(l.empty[0]);
    // Sous la première case pâle, à 5 % de sa largeur : dans sa marge, hors du champ de toute case dessinée.
    let de = pos2(p0.center().x, p0.bottom() + p0.width() * 0.05);
    let champs = crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction());
    assert!(!champs.iter().any(|c| c.contains(xf.to_board(de))), "hors du champ des cases dessinées");
    assert!(crate::board::hit(&l, xf.to_board(de), 1.0) == crate::board::Hit::Nothing, "hors du cadre de la case pâle : dans sa marge");
    let etapes = b.app.history.undo_len();
    tracer(&mut b, de, p0.center());
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 3, "la case pâle est née : {}", b.app.status);
    assert_eq!((elements(&b, 2), parts(&b)), (1, (1, 0)), "le trait est dans son plan, entier");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule étape pour le plan et le trait");
    // La marge d'une case pâle plus lointaine : du papier.
    let Some(mut b) = planche(des_plans(2, false), 6, 1.0) else { return };
    let de = au_dessus_de_la_pale(&b, 2);
    tracer(&mut b, de, de + vec2(40.0, -4.0));
    repos(&mut b);
    assert_eq!((b.app.doc.panels.len(), parts(&b)), (2, (0, 1)), "aucun plan ne naît : le trait est sur le papier");
}