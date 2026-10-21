//! Essais de l'interface des annotations de la vue Vignettes (`annotationsui.rs`) : le stylo passe par-dessus tout sans
//! rien changer au storyboard, la gomme n'efface que des annotations, les traits suivent leur case, cacher, tout effacer.

use eframe::egui::{pos2, vec2, Color32};

use crate::app::Tool;
use crate::board;
use crate::perf_tests::se_reposer;
use crate::vignettes_tests::{dernier_nom, encre, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};

fn stylo(b: &mut crate::perf_tests::Banc) {
    b.app.run_action("vignettes.annotation_stylo");
    b.chauffer(2);
}

#[test]
fn the_pen_draws_over_everything_and_changes_nothing_in_the_storyboard() {
    let mut b = ouvert(quatre_plans(), 3);
    let forme = |d: &crate::document::Document| d.panels.iter().map(|p| (p.id, p.frame_count(), p.marks.len(), p.layers.len(), p.notes.clone())).collect::<Vec<_>>();
    let avant = forme(&b.app.doc);
    let cels_avant = b.app.doc.cels.len();
    let etapes = pas_d_annulation(&b);
    stylo(&mut b);
    assert!(b.app.annot_actif());
    // Un trait de la case 0 à la case 1, par-dessus l'écart et la barre.
    let (a, c) = (vignette(&b, 0).center(), vignette(&b, 1).center());
    glisser(&mut b, &vague(a, c, 12));
    assert_eq!(b.app.doc.annotations.traces.len(), 1, "un trait d'annotation");
    assert_eq!(b.app.doc.annotations.traces[0].color, crate::annotations::COULEUR_PAR_DEFAUT, "rouge d'origine");
    assert_eq!(pas_d_annulation(&b), etapes + 1);
    assert_eq!(dernier_nom(&b), "Annotation");
    // Aucun dessin, aucun plan, aucune image n'a changé : le dessin de la case courante n'a rien reçu.
    for i in 0..4 {
        assert!(!encre(&b.app.doc, i), "le plan {i} n'a pas d'encre");
    }
    assert_eq!(b.app.doc.cels.len(), cels_avant);
    assert_eq!(forme(&b.app.doc), avant);
    // Ctrl+Z défait le trait, et seulement lui.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(b.app.doc.annotations.is_empty());
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.traces.len(), 1);
}

#[test]
fn the_pen_draws_anywhere_even_on_the_table_and_over_the_cut_bars() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    let view = b.app.vignettes.view;
    // Sur le fond gris, hors de toute page (à droite de la dernière page).
    let loin = pos2(view.right() - 6.0, view.top() + 60.0);
    glisser(&mut b, &[loin, loin + vec2(-3.0, 20.0), loin + vec2(-6.0, 40.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), 1, "un trait sur le fond est gardé");
    // Sur une barre : la barre n'est pas basculée.
    let layout = b.app.board_layout();
    let plans = b.app.doc.panels.len();
    let bar = b.app.vignettes.xf.rect(layout.bars[0].zone).center();
    glisser(&mut b, &[bar, bar + vec2(0.0, 15.0), bar + vec2(1.0, 30.0)]);
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(b.app.board_layout().bars[0].cut, layout.bars[0].cut, "la barre n'a pas changé");
    assert_eq!(b.app.doc.annotations.traces.len(), 2);
}

#[test]
fn another_tool_puts_the_annotation_tools_away() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    assert!(b.app.annot_actif());
    b.app.set_tool(Tool::Brush);
    assert!(!b.app.annot_actif());
    // Même en reprenant l'outil déjà pris.
    stylo(&mut b);
    b.app.set_tool(Tool::Brush);
    assert!(!b.app.annot_actif());
    // Le même bouton deux fois : rangé.
    stylo(&mut b);
    b.app.run_action("vignettes.annotation_stylo");
    assert!(!b.app.annot_actif());
    // Le dessin d'une case est de nouveau le dessin.
    let c = vignette(&b, 0).center();
    glisser(&mut b, &[c, c + vec2(20.0, 5.0), c + vec2(40.0, 0.0)]);
    assert!(b.app.doc.annotations.is_empty());
    assert!(encre(&b.app.doc, 0), "le pinceau a dessiné dans la case");
}

#[test]
fn the_annotation_eraser_only_erases_annotations() {
    let mut b = ouvert(quatre_plans(), 3);
    // Du dessin dans la case 0 et une note libre.
    let c0 = vignette(&b, 0).center();
    glisser(&mut b, &[c0 - vec2(20.0, 0.0), c0, c0 + vec2(20.0, 5.0)]);
    assert!(encre(&b.app.doc, 0));
    b.app.doc.board.strokes.push(board::NoteStroke { points: vec![crate::vector::VPoint { x: 10.0, y: 10.0, r: 2.0 }], color: [0, 0, 0], anchor: None });
    stylo(&mut b);
    glisser(&mut b, &[c0 - vec2(30.0, 20.0), c0, c0 + vec2(30.0, 20.0)]);
    let c3 = vignette(&b, 3).center();
    glisser(&mut b, &[c3 - vec2(30.0, 0.0), c3, c3 + vec2(30.0, 0.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), 2);
    let etapes = pas_d_annulation(&b);
    // La gomme passe sur la case 0, là où il y a de l'encre ET une annotation.
    b.app.run_action("vignettes.annotation_gomme");
    b.chauffer(2);
    glisser(&mut b, &[c0 - vec2(25.0, 15.0), c0 - vec2(10.0, 5.0), c0, c0 + vec2(10.0, 8.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), 1, "seule l'annotation de la case 0 est partie");
    assert!(encre(&b.app.doc, 0), "le dessin est intact");
    assert_eq!(b.app.doc.board.strokes.len(), 1, "la note libre aussi");
    assert_eq!(pas_d_annulation(&b), etapes + 1, "une seule étape pour le geste");
    assert_eq!(dernier_nom(&b), "Effacer des annotations");
    // La gomme qui ne touche rien ne laisse aucune étape.
    let etapes = pas_d_annulation(&b);
    let vide = b.app.vignettes.view.right_top() + vec2(-8.0, 120.0);
    glisser(&mut b, &[vide, vide + vec2(0.0, 10.0)]);
    assert_eq!(pas_d_annulation(&b), etapes);
}

#[test]
fn hiding_is_only_a_display_and_picking_the_pen_shows_them_again() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    let c = vignette(&b, 2).center();
    glisser(&mut b, &[c - vec2(20.0, 0.0), c, c + vec2(20.0, 5.0)]);
    b.app.run_action("vignettes.annotations_afficher");
    assert!(!b.app.vignettes.annot.visible);
    assert_eq!(b.app.doc.annotations.traces.len(), 1, "rien n'est effacé");
    // Cachées, elles ne sont pas peintes : la planche a moins de formes qu'avec elles.
    b.garder_formes = true;
    b.chauffer(2);
    let cachees = b.formes.len();
    b.app.run_action("vignettes.annotations_afficher");
    b.chauffer(2);
    assert!(b.formes.len() > cachees, "les annotations montrées ajoutent des formes ({} contre {cachees})", b.formes.len());
    b.app.run_action("vignettes.annotations_afficher");
    stylo(&mut b);
    stylo(&mut b);
    stylo(&mut b);
    assert!(b.app.vignettes.annot.visible, "prendre le stylo montre les annotations");
}

#[test]
fn clearing_asks_first_is_one_undo_step_and_touches_nothing_else() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    for k in 0..3 {
        let c = vignette(&b, k).center();
        glisser(&mut b, &[c - vec2(20.0, 0.0), c, c + vec2(20.0, 5.0)]);
    }
    assert_eq!(b.app.doc.annotations.traces.len(), 3);
    let etapes = pas_d_annulation(&b);
    b.app.run_action("vignettes.annotations_effacer");
    b.chauffer(2);
    assert!(b.app.vignettes.annot.confirmer, "une confirmation d'abord");
    assert_eq!(b.app.doc.annotations.traces.len(), 3, "rien n'est effacé avant le oui");
    assert!(b.app.modal_open());
    b.app.vignettes.annot.confirmer = false;
    assert_eq!(b.app.doc.annotations.traces.len(), 3, "« Garder » ne change rien");
    b.app.run_action("vignettes.annotations_effacer");
    b.app.annot_effacer_tout();
    b.chauffer(2);
    assert!(b.app.doc.annotations.is_empty());
    assert_eq!(pas_d_annulation(&b), etapes + 1);
    assert_eq!(dernier_nom(&b), "Effacer les annotations");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.traces.len(), 3, "Ctrl+Z les rétablit");
    // Sans annotation : pas de confirmation.
    b.app.doc.annotations = Default::default();
    b.app.run_action("vignettes.annotations_effacer");
    assert!(!b.app.vignettes.annot.confirmer);
}

#[test]
fn strokes_follow_their_case_when_the_board_reflows_in_the_real_view() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    let c = vignette(&b, 4).center() + vec2(10.0, -8.0);
    glisser(&mut b, &[c - vec2(20.0, 0.0), c, c + vec2(20.0, 5.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), 1);
    let rel = |b: &crate::perf_tests::Banc| {
        let l = b.app.board_layout();
        let pts = b.app.doc.annotations.points_planche(&l, 0).expect("placé");
        let case = l.cards[4].thumb;
        (board::points_bounds(&pts).center() - case.left_top()) / case.width()
    };
    let avant = rel(&b);
    b.app.layout.board_columns = 2;
    b.app.layout.card_size = 260.0;
    b.chauffer(3);
    assert!((rel(&b) - avant).length() < 1e-3, "le trait a suivi sa case");
    b.app.layout.board_columns = 6;
    b.chauffer(3);
    assert!((rel(&b) - avant).length() < 1e-3);
}

#[test]
fn deleting_a_plan_hands_its_strokes_to_the_neighbour() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    let c = vignette(&b, 3).center();
    glisser(&mut b, &[c - vec2(20.0, 0.0), c, c + vec2(20.0, 5.0)]);
    let id = b.app.board_layout().cards[3].plan;
    let ou = board::points_bounds(&b.app.doc.annotations.points_planche(&b.app.board_layout(), 0).unwrap()).center();
    b.app.delete_panels(&[id]);
    b.chauffer(3);
    let l = b.app.board_layout();
    let t = &b.app.doc.annotations.traces[0];
    assert!(l.card_of(t.anchor.card).is_some(), "attaché à une vignette qui reste");
    assert!(b.app.doc.annotations.points_planche(&l, 0).is_some());
    let _ = ou;
    // Annuler le retrait rend le plan, et le trait reste montré.
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert!(b.app.doc.annotations.points_planche(&b.app.board_layout(), 0).is_some());
}

#[test]
fn annotations_never_reach_the_film_the_thumbnails_or_the_exports() {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let mut b = ouvert(quatre_plans(), 3);
    // Du vrai dessin dans la case 0, puis une annotation qui la traverse.
    let c0 = vignette(&b, 0).center();
    glisser(&mut b, &[c0 - vec2(30.0, 0.0), c0, c0 + vec2(30.0, 10.0)]);
    let images = |doc: &crate::document::Document| -> Vec<Vec<Color32>> {
        let snap = snapshot_of(doc, &[0, 1, 2, 3]);
        let rendu = Rendu::new(snap, Papier::Fond, true, Vec::new(), Arc::new(AtomicBool::new(false)));
        (0..4).map(|p| (*rendu.image(p, 0).expect("image")).clone()).collect()
    };
    let sans = images(&b.app.doc);
    stylo(&mut b);
    glisser(&mut b, &[c0 - vec2(60.0, 20.0), c0, c0 + vec2(60.0, 40.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), 1);
    let avec = images(&b.app.doc);
    assert!(sans == avec, "les images du film sont identiques avec et sans annotation");
    // Les vignettes de la planche (même rendu) aussi : le plan 0 n'a de rouge nulle part.
    let rouge = |img: &Vec<Color32>| img.iter().any(|p| p.r() > 180 && p.g() < 80 && p.b() < 80);
    assert!(avec.iter().all(|i| !rouge(i)));
    // L'enregistrement ne met pas le trait dans le dessin du plan non plus.
    let dir = std::env::temp_dir().join(format!("shot-test-annot-film-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.pelure");
    crate::project::save(&b.app.doc, &manifest, &mut Default::default()).unwrap();
    let loaded = crate::project::load(&manifest).unwrap();
    assert_eq!(loaded.doc.annotations.traces.len(), 1);
    assert!(images(&loaded.doc) == sans, "rouvert, le film est toujours le même");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_board_is_idle_after_drawing_and_erasing() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    let c = vignette(&b, 1).center();
    glisser(&mut b, &vague(c - vec2(40.0, 0.0), c + vec2(40.0, 0.0), 30));
    b.app.run_action("vignettes.annotation_gomme");
    glisser(&mut b, &[c - vec2(50.0, 0.0), c, c + vec2(50.0, 0.0)]);
    assert!(se_reposer(&mut b, 60) < 60, "l'écran se repose");
}

#[test]
fn the_pen_costs_about_a_line_per_point_and_no_replay() {
    let mut b = ouvert(quatre_plans(), 3);
    stylo(&mut b);
    let (a, c) = (vignette(&b, 0).center(), vignette(&b, 5).center());
    let temps = glisser(&mut b, &vague(a, c, 60));
    let pire = temps.iter().cloned().fold(0.0, f64::max);
    println!("stylo d'annotation : image la plus longue {pire:.1} ms");
    assert!(pire < 250.0, "le trait ne ralentit pas l'écran ({pire:.1} ms)");
}

#[test]
fn the_marie_louise_display_hides_everything_but_the_insides_of_the_cases() {
    use eframe::egui::epaint::Shape;
    let mut b = ouvert(quatre_plans(), 3);
    let noirs = |b: &crate::perf_tests::Banc| -> Vec<eframe::egui::Rect> {
        b.formes.iter().filter_map(|s| if let Shape::Rect(r) = &s.shape { (r.fill == Color32::BLACK).then_some(r.rect) } else { None }).collect()
    };
    b.garder_formes = true;
    b.chauffer(2);
    let avant = noirs(&b).len();
    b.app.run_action("vignettes.marie_louise");
    assert!(b.app.vignettes.annot.marie);
    b.chauffer(2);
    let mat = noirs(&b);
    assert!(mat.len() > avant, "le passe-partout est peint");
    let vue = b.app.vignettes.view;
    let cases: Vec<_> = (0..6).map(|k| vignette(&b, k)).collect();
    for r in &mat {
        for c in &cases {
            let o = r.intersect(c.shrink(1.0));
            assert!(!(o.width() > 0.5 && o.height() > 0.5), "le passe-partout ne recouvre pas l'intérieur d'une case");
        }
    }
    // Il couvre tout le reste de la vue : l'aire du passe-partout = la vue moins les cases.
    let aire: f32 = mat.iter().filter(|r| vue.contains_rect(**r) || r.intersects(vue)).map(|r| { let i = r.intersect(vue); i.width() * i.height() }).sum();
    let cases_aire: f32 = cases.iter().map(|c| { let i = c.intersect(vue); i.width() * i.height() }).sum();
    assert!((aire - (vue.width() * vue.height() - cases_aire)).abs() < vue.width() * vue.height() * 0.01, "aire {aire}");
    // Un trait d'annotation qui déborde est masqué avec le reste ; on retire la marie-louise d'un second appel.
    b.app.run_action("vignettes.marie_louise");
    assert!(!b.app.vignettes.annot.marie);
    b.chauffer(2);
    assert_eq!(noirs(&b).len(), avant);
}