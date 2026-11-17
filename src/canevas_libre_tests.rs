//! Essais du canevas libre des Vignettes (partie 101) : aucun plan n'est choisi d'office, un trait part de n'importe où, et au lâcher
//! chaque part entre dans le plan de la case où elle tombe (ou dans le plan neuf d'une case vide), le reste restant sur la planche ; l'outil
//! Caméra des Vignettes.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Key, Modifiers};

use crate::app::Tool;
use crate::boardui::BORDER_CURRENT;
use crate::document::{Document, LayerKind};
use crate::perf_tests::Banc;
use crate::vignettes_tests::{clic_avec, encre, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};
use crate::workspace::Espace;

fn plan_seul(bitmap: bool) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    if bitmap {
        en_bitmap(&mut doc);
    }
    doc
}

fn en_bitmap(doc: &mut Document) {
    for p in &mut doc.panels {
        for l in &mut p.layers {
            l.kind = LayerKind::Bitmap;
        }
    }
}

fn hors_cadres(b: &Banc) -> usize {
    let a = &b.app.doc.annotations;
    // (Partie 127 : un trait bitmap laisse ses pixels sur la couche bitmap de la planche.)
    a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)).count() + a.bitmap.morceaux.len()
}

/// Les cadres noirs de sélection peints à la dernière image.
fn cadres_noirs(b: &mut Banc) -> usize {
    b.garder_formes = true;
    b.chauffer(2);
    let vue = b.app.vignettes.view;
    b.formes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.stroke.color == BORDER_CURRENT && vue.contains_rect(r.rect.expand(3.0)))).count()
}

fn echap(b: &mut Banc) {
    let ev = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![ev(true)], 1.0 / 60.0);
    b.image(vec![ev(false)], 1.0 / 60.0);
    b.chauffer(2);
}

fn ids_des_cels(doc: &Document, plan: usize) -> usize {
    doc.panels[plan].layers.iter().flat_map(|l| l.frames.iter().flatten()).filter(|id| doc.cel_a_du_trait(**id)).count()
}

// ------------------------------------------------------------------------------ aucun plan choisi d'office

#[test]
fn no_plan_is_chosen_at_opening_nor_after_a_stroke_nor_when_coming_back_and_the_current_plan_stays_valid() {
    let mut b = ouvert(quatre_plans(), 4);
    assert!(b.app.vignettes.libre);
    assert_eq!(cadres_noirs(&mut b), 0, "pas de cadre noir à l'ouverture");
    let n = pas_d_annulation(&b);
    let v = vignette(&b, 4);
    glisser(&mut b, &vague(v.center() - vec2(20.0, 0.0), v.center() + vec2(20.0, 0.0), 8));
    assert!(encre(&b.app.doc, 2), "le trait est dans le plan de la case");
    assert!(b.app.vignettes.libre, "dessiner ne choisit pas le plan");
    assert_eq!(cadres_noirs(&mut b), 0, "pas de cadre noir après un trait");
    assert_eq!(pas_d_annulation(&b), n + 1);
    // Le plan courant reste valide pour les panneaux (Calques…).
    assert!(b.app.cur_panel < b.app.doc.panels.len() && b.app.cur_layer < b.app.layers().len());
    // Quitter les Vignettes et y revenir, l'espace Storyboard : cohérents.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    assert!(b.app.cur_panel < b.app.doc.panels.len());
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    assert!(b.app.vignettes.libre);
    assert_eq!(cadres_noirs(&mut b), 0);
    assert_eq!(pas_d_annulation(&b), n + 1, "aucune étape d'annulation de plus");
}

#[test]
fn shift_click_chooses_a_plan_and_escape_or_a_click_on_the_paper_removes_it() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 4);
    clic_avec(&mut b, v.center(), Modifiers::SHIFT);
    assert!(!b.app.vignettes.libre, "Maj + clic : un choix voulu");
    assert_eq!(b.app.cur_panel, 2, "le plan cliqué devient le plan courant");
    assert_eq!(cadres_noirs(&mut b), 1, "son cadre de sélection est là");
    echap(&mut b);
    assert!(b.app.vignettes.libre, "Échap retire le choix");
    assert_eq!(cadres_noirs(&mut b), 0);
    clic_avec(&mut b, v.center(), Modifiers::SHIFT);
    assert!(!b.app.vignettes.libre);
    let papier = pos2(b.app.vignettes.view.left() + 5.0, b.app.vignettes.view.bottom() - 5.0);
    clic_avec(&mut b, papier, Modifiers::NONE);
    assert!(b.app.vignettes.libre, "un clic sur le papier retire le choix");
    assert_eq!(cadres_noirs(&mut b), 0);
}

// ------------------------------------------------------------------------------ dessiner partout

/// Partie 131 (avant : la part dans la case allait à son plan, le bout dans la case vide faisait naître le sien, le reste restait sur la planche).
/// Un trait appartient tout entier à l'endroit où il commence : parti du papier, il est au papier, entier, même s'il traverse une case et finit
/// dans une case vide ; aucun plan ne naît.
#[test]
fn a_stroke_from_the_paper_through_a_case_into_empty_cases_stays_whole_on_the_paper_in_one_undo_step() {
    for bitmap in [false, true] {
        let mut b = ouvert(plan_seul(bitmap), 4);
        let v = vignette(&b, 0);
        let (plans, n) = (b.app.doc.panels.len(), pas_d_annulation(&b));
        // Du papier sous la case, à travers elle, dans la première case vide et au-delà.
        let de = v.left_bottom() + vec2(40.0, 60.0);
        let fin = v.right_top() + vec2(v.width() * 1.3, -30.0);
        let l = b.app.board_layout();
        let depart = b.app.vignettes.xf.to_board(de);
        assert!(!crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction()).iter().any(|c| c.contains(depart)), "le trait part hors du champ de toute case");
        glisser(&mut b, &vague(de, fin, 40));
        b.chauffer(8);
        assert!(!encre(&b.app.doc, 0), "bitmap {bitmap} : rien n'entre dans le plan de la case traversée");
        assert_eq!(hors_cadres(&b), 1, "bitmap {bitmap} : le trait est sur la planche, entier, en un seul morceau");
        assert_eq!(pas_d_annulation(&b), n + 1, "bitmap {bitmap} : une seule étape");
        assert!(b.app.vignettes.libre);
        assert_eq!(b.app.doc.panels.len(), plans, "bitmap {bitmap} : le bout du trait ne fait naître aucun plan");
        // Annuler : tout part d'un coup ; rétablir : tout revient.
        b.app.run_action("edition.annuler");
        b.chauffer(3);
        assert_eq!((b.app.doc.panels.len(), encre(&b.app.doc, 0), hors_cadres(&b)), (plans, false, 0), "bitmap {bitmap}");
        assert_eq!(pas_d_annulation(&b), n);
        b.app.run_action("edition.retablir");
        b.chauffer(3);
        assert!(b.app.doc.panels.len() == plans && !encre(&b.app.doc, 0) && hors_cadres(&b) == 1, "rétabli");
    }
}

/// Partie 131 (avant : chaque case recevait sa part, dans l'image qu'elle montre). Le trait va tout entier à l'image que montre **la case où il
/// commence** ; les autres cases du plan ne reçoivent rien.
#[test]
fn a_stroke_goes_whole_to_the_key_shown_by_the_case_where_it_starts() {
    // Le plan 1 a trois cases (images 0, a, b) : un trait parti de la troisième, à travers la deuxième, est dans l'image de la troisième.
    for bitmap in [false, true] {
        let mut doc = quatre_plans();
        if bitmap {
            en_bitmap(&mut doc);
        }
        let mut b = ouvert(doc, 4);
        let layout = b.app.board_layout();
        let frames: Vec<usize> = layout.cards.iter().take(3).map(|c| c.frame).collect();
        assert!(frames[0] < frames[1] && frames[1] < frames[2]);
        // Une clé de dessin par case : un petit trait dans chacune (sans elles, les trois cases montrent le même dessin).
        let v = vignette(&b, 0);
        glisser(&mut b, &vague(v.center() - vec2(6.0, 12.0), v.center() - vec2(-6.0, 12.0), 4));
        b.app.doc.cles_de_case(0, frames[1]);
        b.app.doc.cles_de_case(0, frames[2]);
        b.chauffer(4);
        let quantite = |b: &Banc, f: usize| -> usize {
            let d = &b.app.doc;
            d.panels[0]
                .layers
                .iter()
                .filter_map(|l| l.cel_at(f))
                .filter_map(|id| d.cels.get(&id))
                .map(|c| c.vector.as_ref().map_or(0, |v| v.elements.len()) + c.raster.tiles.iter().flatten().map(|t| t.iter().filter(|px| px.a() != 0).count()).sum::<usize>())
                .sum()
        };
        let avant: Vec<usize> = frames.iter().map(|f| quantite(&b, *f)).collect();
        assert!(avant.iter().all(|q| *q > 0), "bitmap {bitmap} : chaque case a sa clé : {avant:?}");
        let (v1, v2) = (vignette(&b, 1), vignette(&b, 2));
        let n = pas_d_annulation(&b);
        glisser(&mut b, &vague(v2.center(), v1.center(), 24));
        let apres: Vec<usize> = frames.iter().map(|f| quantite(&b, *f)).collect();
        assert_eq!(apres[0], avant[0], "bitmap {bitmap} : rien dans la clé de la case 1 : {avant:?} {apres:?}");
        assert_eq!(apres[1], avant[1], "bitmap {bitmap} : rien dans la clé de la case 2, que le trait traverse : {avant:?} {apres:?}");
        assert!(apres[2] > avant[2], "bitmap {bitmap} : tout le trait est dans la clé de la case 3, où il commence : {avant:?} {apres:?}");
        assert_eq!(hors_cadres(&b), 0, "bitmap {bitmap} : rien sur le papier");
        assert_eq!(pas_d_annulation(&b), n + 1, "bitmap {bitmap}");
    }
}

/// Partie 131 (avant : la part du trait dans la case verrouillée restait sur la planche, par-dessus elle). Un trait qui traverse une case ne lui
/// donne rien : il reste entier dans le plan où il commence, le calque verrouillé n'est pas touché, rien ne va sur le papier.
#[test]
fn a_stroke_crossing_a_locked_case_stays_whole_in_its_own_plan() {
    let mut b = ouvert(quatre_plans(), 4);
    // (Les cases 5 et 6 : les plans 3 et 4, d'une seule case chacun. Le plan 1 en a trois, qui montreraient toutes un dessin neuf : il garde
    // la règle d'avant, `render::dessins_communs`.)
    for l in &mut b.app.doc.panels[3].layers {
        l.locked = true;
    }
    b.chauffer(3);
    let (v4, v5) = (vignette(&b, 4), vignette(&b, 5));
    glisser(&mut b, &vague(v4.center(), v5.center(), 24));
    assert!(encre(&b.app.doc, 2), "le trait est dans le plan de la case où il commence");
    assert!(!encre(&b.app.doc, 3), "le calque verrouillé n'a rien reçu");
    assert_eq!(hors_cadres(&b), 0, "rien sur le papier : le trait est entier dans son plan");
}

/// Partie 131 (avant : le bout du trait dans la case vide faisait naître son plan). Le trait reste entier dans le plan où il commence, au-delà
/// du cadre aussi, et se relit tel quel.
#[test]
fn what_is_drawn_beyond_the_frame_is_the_same_in_the_storyboard_plan_after_saving_and_reopening() {
    let mut b = ouvert(plan_seul(false), 4);
    let v = vignette(&b, 0);
    glisser(&mut b, &vague(v.center(), v.right_top() + vec2(v.width() * 1.3, 0.0), 40));
    b.chauffer(8);
    assert!(b.app.doc.panels.len() == 1 && encre(&b.app.doc, 0), "aucun plan ne naît : le trait est entier dans le sien");
    let cadre = b.app.doc.frame[0] as f32;
    let au_dela = b.app.doc.cels.values().filter_map(|c| c.vector.as_ref()).flat_map(|v| v.elements.iter()).any(|e| matches!(e, crate::vector::Element::Stroke { points, .. } if points.iter().any(|p| p.x > cadre * 1.5)));
    assert!(au_dela, "le trait garde ses points au-delà du cadre, dans le dessin de son plan");
    assert_eq!(hors_cadres(&b), 0);
    let dossier = std::env::temp_dir().join(format!("shot-test-canevas-libre-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dossier).ok();
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("P.pelure");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    let points = |d: &Document, i: usize| -> Vec<(i64, i64)> {
        d.panels[i]
            .layers
            .iter()
            .flat_map(|l| l.frames.iter().flatten())
            .filter_map(|id| d.cels.get(id).and_then(|c| c.vector.as_ref()))
            .flat_map(|v| v.elements.iter())
            .flat_map(|e| match e {
                crate::vector::Element::Stroke { points, .. } => points.iter().map(|p| ((p.x * 100.0) as i64, (p.y * 100.0) as i64)).collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect()
    };
    assert_eq!(relu.doc.panels.len(), b.app.doc.panels.len());
    for i in 0..b.app.doc.panels.len() {
        let (a, r) = (points(&b.app.doc, i), points(&relu.doc, i));
        assert!(a == r, "plan {i} : mêmes points après réouverture ({} contre {})", a.len(), r.len());
        assert_eq!(ids_des_cels(&b.app.doc, i) > 0, ids_des_cels(&relu.doc, i) > 0, "plan {i}");
    }
    std::fs::remove_dir_all(&dossier).ok();
}

// ------------------------------------------------------------------------------ l'outil Caméra

#[test]
fn the_camera_tool_sits_in_its_own_group_after_the_eraser_and_the_storyboard_keeps_its_own() {
    use crate::toolbars::{TOOL_GROUPS, TOOL_GROUPS_VIGNETTES};
    assert_eq!(TOOL_GROUPS_VIGNETTES.len(), 2);
    assert_eq!(TOOL_GROUPS_VIGNETTES[0].last().copied(), Some(&[Tool::Eraser][..]));
    assert_eq!(TOOL_GROUPS_VIGNETTES[1], &[&[Tool::Camera][..]]);
    assert!(TOOL_GROUPS.iter().any(|g| g.iter().any(|s| s.contains(&Tool::Camera))), "le Storyboard garde son outil Caméra");
}

#[test]
fn the_camera_tool_chooses_a_camera_with_a_click_moves_a_frame_from_the_first_press_in_one_undo_step_and_drawing_is_free_again_after() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.set_tool(Tool::Camera);
    let v = vignette(&b, 4);
    let n = pas_d_annulation(&b);
    // Un clic : la caméra de la case est choisie, son plan est courant, aucune étape d'annulation.
    clic_avec(&mut b, v.center(), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 2);
    assert_eq!(b.app.vignettes.camera_choisie, Some(b.app.doc.panels[2].id));
    assert!(!b.app.vignettes.libre);
    assert_eq!(pas_d_annulation(&b), n);
    // Échap la lâche.
    echap(&mut b);
    assert!(b.app.vignettes.libre && b.app.vignettes.camera_choisie.is_none());
    // Un glissé sur le cadre d'une autre case, sans clic avant : le cadre bouge, une seule étape, les dessins ne bougent pas.
    let w = v.width();
    let a = vignette(&b, 3);
    let avant = b.app.doc.panels[1].camera.clone();
    b.image(vec![Event::PointerMoved(a.center())], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(a.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(a.center() + vec2(w * 0.5 * k as f32 / 8.0, 0.0))], 1.0 / 60.0);
    }
    b.image(vec![crate::vignettes_tests::appui(a.center() + vec2(w * 0.5, 0.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(b.app.cur_panel, 1);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "les clés A et B");
    assert_ne!(b.app.doc.panels[1].camera, avant);
    assert_eq!(pas_d_annulation(&b), n + 1, "une étape par geste");
    assert!(!encre(&b.app.doc, 1), "le geste ne touche pas aux dessins");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].camera, avant);
    assert_eq!(pas_d_annulation(&b), n);
    // Revenir à un outil de dessin : le dessin est libre, aucun plan choisi.
    b.app.set_tool(Tool::Brush);
    assert!(b.app.vignettes.libre && b.app.vignettes.camera_choisie.is_none());
    assert_eq!(cadres_noirs(&mut b), 0);
}

#[test]
fn the_camera_tool_shows_the_frames_of_the_case_under_the_pointer() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.set_tool(Tool::Camera);
    b.garder_formes = true;
    let v = vignette(&b, 4);
    b.image(vec![Event::PointerMoved(pos2(b.app.vignettes.view.right() - 3.0, b.app.vignettes.view.bottom() - 3.0))], 1.0 / 60.0);
    let sans: usize = b.formes.iter().filter(|s| matches!(s.shape, Shape::Circle(_))).count();
    b.image(vec![Event::PointerMoved(v.center())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let avec: usize = b.formes.iter().filter(|s| matches!(s.shape, Shape::Circle(_))).count();
    assert!(avec >= sans + 4, "les poignées des coins du cadre se montrent au survol ({sans} puis {avec})");
}

#[test]
fn the_storyboard_camera_tool_still_works() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    b.app.set_tool(Tool::Camera);
    b.chauffer(3);
    assert_eq!(b.app.tool, Tool::Camera);
    let _: egui::Rect = b.app.vignettes.view;
    b.app.run_action("camera.cle_ici");
    assert!(b.app.camera_key_here().is_some(), "une clé de caméra posée à l'image courante");
}

// ------------------------------------------------------------------------------ plan à mouvement de caméra, couleur

/// Le plan 2 des quatre (case K = 3), avec un cadre B à droite de A.
fn avec_mouvement() -> Banc {
    let mut doc = crate::vignettes_camera_tests::plan_court();
    crate::vignettes_camera_tests::deux_cadres(&mut doc, 1, 160.0, 0.0);
    ouvert(doc, 6)
}

/// Le point de l'écran `s` vu comme un point du dessin de la tuile du plan 2.
fn dessin_sous(b: &Banc, s: eframe::egui::Pos2) -> eframe::egui::Pos2 {
    let l = b.app.board_layout();
    let t = &l.move_of(crate::vignettes_camera_tests::K).expect("mouvement").tuile;
    let p = b.app.vignettes.xf.to_board(s);
    t.zone.centre + (p - t.rect.center()) / t.zone.echelle
}

#[test]
fn a_stroke_drawn_in_the_frame_b_of_a_camera_move_is_written_where_it_was_drawn_and_survives_saving() {
    let mut b = avec_mouvement();
    b.app.set_tool(Tool::Brush);
    let l = b.app.board_layout();
    let tuile = b.app.vignettes.xf.rect(l.move_of(crate::vignettes_camera_tests::K).unwrap().tuile.rect);
    let cadres = crate::vignettes_camera_tests::cadre_ecran(&b, 1);
    let cb = crate::vignettes_camera_tests::centre(cadres);
    let (de, a) = (cb - vec2(30.0, 10.0), cb + vec2(30.0, 14.0));
    assert!(tuile.contains(de) && tuile.contains(a), "le trait est dans la tuile : {tuile:?} {de:?} {a:?}");
    let n = pas_d_annulation(&b);
    glisser(&mut b, &vague(de, a, 12));
    b.chauffer(6);
    assert!(encre(&b.app.doc, 1), "le trait est dans le plan à mouvement");
    assert_eq!(pas_d_annulation(&b), n + 1);
    assert_eq!(hors_cadres(&b), 0, "tout est dans le champ : rien sur la planche");
    // Écrit au bon endroit de la feuille : son premier et son dernier point sont ceux de la souris, vus par la tuile.
    let pts: Vec<_> = b.app.doc.panels[1]
        .layers
        .iter()
        .flat_map(|l| l.frames.iter().flatten())
        .filter_map(|id| b.app.doc.cels.get(id).and_then(|c| c.vector.as_ref()))
        .flat_map(|v| v.elements.iter())
        .filter_map(|e| match e {
            crate::vector::Element::Stroke { points, .. } => Some(points.clone()),
            _ => None,
        })
        .next()
        .expect("un trait vectoriel");
    let (p0, p1) = (dessin_sous(&b, de), dessin_sous(&b, a));
    let (q0, q1) = (pts[0].pos(), pts[pts.len() - 1].pos());
    assert!((q0 - p0).length() < 6.0 && (q1 - p1).length() < 6.0, "écrit où il a été tracé : {q0:?} {p0:?} / {q1:?} {p1:?}");
    // La caméra n'a pas bougé, et le trait est relu à l'identique après l'enregistrement.
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    let dossier = std::env::temp_dir().join(format!("shot-test-canevas-mvt-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dossier).ok();
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("P.pelure");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    assert!(encre(&relu.doc, 1) && relu.doc.panels[1].camera == b.app.doc.panels[1].camera);
    std::fs::remove_dir_all(&dossier).ok();
}

/// Partie 131 (avant : la part dans le champ de la tuile allait au plan, le reste sur la planche). Parti du papier, le trait y est tout entier,
/// à travers la tuile d'un mouvement de caméra aussi, avec la couleur du pinceau.
#[test]
fn a_stroke_from_the_paper_across_a_camera_move_is_on_the_board_whole_with_the_colour_of_the_brush() {
    let mut b = avec_mouvement();
    b.app.set_tool(Tool::Brush);
    b.app.color = [200, 30, 40];
    let l = b.app.board_layout();
    let tuile = b.app.vignettes.xf.rect(l.move_of(crate::vignettes_camera_tests::K).unwrap().tuile.rect);
    // Du papier au-dessus de la tuile, hors de tout champ, à travers elle, puis au-delà à droite.
    let champs = crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction());
    let libre = |p: egui::Pos2| !champs.iter().any(|c| c.contains(b.app.vignettes.xf.to_board(p)));
    let de = (1..40).map(|k| pos2(tuile.center().x, tuile.top() - 10.0 * k as f32)).find(|p| libre(*p) && b.app.vignettes.view.contains(*p)).expect("du papier au-dessus de la tuile");
    let fin = pos2(tuile.right() + 90.0, tuile.center().y);
    let n = pas_d_annulation(&b);
    let avant = encre(&b.app.doc, 1);
    glisser(&mut b, &vague(de, fin, 40));
    b.chauffer(10);
    assert_eq!(pas_d_annulation(&b), n + 1, "une seule étape");
    assert_eq!(encre(&b.app.doc, 1), avant, "rien n'entre dans le plan traversé");
    assert_eq!(hors_cadres(&b), 1, "le trait est sur la planche, entier");
    let a = &b.app.doc.annotations;
    for t in a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)) {
        assert_eq!(t.color, [200, 30, 40], "la couleur du trait d'origine");
        assert_eq!(t.alpha, 255, "l'opacité du trait d'origine");
        // Et le rendu de ce tronçon a cette couleur-là (pas plus clair).
        let pts = a.points_planche(&l, a.traces.iter().position(|x| x.id == t.id).unwrap()).unwrap();
        let py = t.brosse.as_ref().expect("la recette du pinceau").pyramide_ou_calcul(&t.points, t.color).expect("rendu");
        let _ = pts;
        let plein = py.niveaux[0].blocs.iter().flat_map(|bl| bl.pixels.iter()).filter(|c| c.a() >= 250).count();
        assert!(plein > 0, "le rendu a des pixels pleins");
        let ok = py.niveaux[0].blocs.iter().flat_map(|bl| bl.pixels.iter()).filter(|c| c.a() >= 250).all(|c| (c.r() as i32 - 200).abs() < 8 && (c.g() as i32 - 30).abs() < 8 && (c.b() as i32 - 40).abs() < 8);
        assert!(ok, "les pixels pleins du trait de la planche gardent la couleur du pinceau");
    }
}
#[test]
fn the_part_of_a_stroke_outside_its_case_keeps_the_colour_and_opacity_of_the_brush_while_it_is_drawn() {
    let mut b = ouvert(plan_seul(false), 4);
    b.app.set_tool(Tool::Brush);
    b.app.color = [200, 30, 40];
    let v = vignette(&b, 0);
    let (de, fin) = (v.center(), v.right_top() + vec2(v.width() * 0.6, -20.0));
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
    for q in vague(de, fin, 16) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 120.0);
    }
    let d = b.app.vignettes.debord.clone().or_else(|| b.app.vignettes.debord_garde.clone()).expect("le trait déborde de sa case");
    let teintes: Vec<egui::Color32> = d
        .shapes
        .iter()
        .filter_map(|(_, s)| match s {
            Shape::LineSegment { stroke, .. } => Some(stroke.color),
            Shape::Path(p) => match p.stroke.color {
                egui::epaint::ColorMode::Solid(c) => Some(c),
                _ => None,
            },
            Shape::Circle(c) => Some(c.fill),
            _ => None,
        })
        .collect();
    assert!(!teintes.is_empty());
    assert!(teintes.iter().all(|c| c.r() == 200 && c.g() == 30 && c.b() == 40 && c.a() == 255), "plein, de la couleur du pinceau : {teintes:?}");
    b.image(vec![crate::vignettes_tests::appui(fin, false, Modifiers::NONE)], 1.0 / 60.0);
}