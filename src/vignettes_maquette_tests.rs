//! Essais de la planche Â« comme la maquette 07 v6 Â» (2026-10-21), puis sur papier infini : le papier blanc sans carte, les cases pÃ¢les (trois
//! rangÃ©es de six au moins, une rangÃ©e pÃ¢le aprÃ¨s la derniÃ¨re rangÃ©e occupÃ©e), la disposition de deux cents plans, et le trait qui reste
//! visible un peu au-delÃ  du cadre de sa case.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Modifiers};

use crate::board;
use crate::perf_tests::se_reposer;
use crate::vignettes_tests::{appui, case_vide, ouvert, quatre_plans, vignette};

const MAGENTA: [u8; 3] = [255, 0, 255];

#[test]
fn an_empty_project_shows_three_rows_of_six_pale_cases_and_the_settings_say_so() {
    let doc = crate::document::Document::with_frame(160, 90, 24);
    let s = board::BoardSettings { card_w: 240.0, columns: 6, rows: 0 };
    let l = board::layout_with(&doc, &s, &Default::default(), false, &board::Options::pale(3));
    assert_eq!(l.cards.len(), 1);
    // Une vignette, puis cinq cases pÃ¢les dans sa rangÃ©e et deux rangÃ©es de six : trois rangÃ©es au moins en tout.
    assert_eq!(l.empty.len(), 5 + 12);
    let mut hauts: Vec<i32> = l.empty.iter().map(|e| e.top().round() as i32).collect();
    hauts.dedup();
    assert_eq!(hauts.len(), 3, "trois rangÃ©es : celle de la vignette et deux pÃ¢les ({hauts:?})");
    // Le rÃ©glage d'origine : six colonnes, trois rangÃ©es.
    let d = crate::workspace::Layout::default();
    assert_eq!((d.board_columns, d.board_min_rows), (6, 3));
    // Le minimum est un rÃ©glage : cinq rangÃ©es.
    let l5 = board::layout_with(&doc, &s, &Default::default(), false, &board::Options::pale(5));
    assert_eq!(l5.empty.len(), 5 + 4 * 6);
    // (Et l'export n'en a aucune.)
    assert!(board::layout(&doc, &s).empty.is_empty());
}

#[test]
fn there_is_always_one_pale_row_after_the_last_used_one() {
    let s = board::BoardSettings { card_w: 240.0, columns: 6, rows: 0 };
    let rangees = |l: &board::BoardLayout| {
        let mut h: Vec<i32> = l.cards.iter().map(|c| c.thumb.top().round() as i32).chain(l.empty.iter().map(|e| e.top().round() as i32)).collect();
        h.sort_unstable();
        h.dedup();
        h.len()
    };
    // Douze plans sur six colonnes : deux rangÃ©es pleines, donc trois rangÃ©es en tout (le minimum, une pÃ¢le aprÃ¨s).
    let mut doc = crate::document::Document::with_frame(160, 90, 24);
    for _ in 0..11 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    let l = board::layout_with(&doc, &s, &Default::default(), false, &board::Options::pale(3));
    assert_eq!((l.cards.len(), l.empty.len()), (12, 6));
    assert_eq!(rangees(&l), 3);
    // Un plan de plus dans la troisiÃ¨me rangÃ©e : une rangÃ©e pÃ¢le de plus apparaÃ®t.
    let last = doc.panels.len() - 1;
    doc.add_scene_after(last);
    let l = board::layout_with(&doc, &s, &Default::default(), false, &board::Options::pale(3));
    assert_eq!(l.cards.len(), 13);
    assert_eq!(l.empty.len(), 5 + 6, "le reste de la rangÃ©e et une rangÃ©e pÃ¢le entiÃ¨re");
    assert_eq!(rangees(&l), 4);
    // La derniÃ¨re rangÃ©e pleine : toujours une rangÃ©e pÃ¢le derriÃ¨re.
    for _ in 0..5 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    let l = board::layout_with(&doc, &s, &Default::default(), false, &board::Options::pale(3));
    assert_eq!((l.cards.len(), l.empty.len()), (18, 6));
    assert_eq!(rangees(&l), 4);
    assert!(l.empty.iter().all(|e| e.top() > l.cards.last().unwrap().thumb.bottom()));
}

#[test]
fn the_paper_is_white_and_endless_the_pale_cases_are_drawn_faintly_without_a_plus() {
    let mut b = ouvert(quatre_plans(), 4);
    // (Un peu plus petit : le numéro des plans est plus haut et les notes plus bas depuis le 2026-10-24, la dernière rangée pâle doit
    // encore tenir dans la vue pour être peinte.)
    b.app.board_zoom = 0.6;
    b.chauffer(3);
    b.garder_formes = true;
    b.garder_textes = true;
    b.image(Vec::new(), 1.0 / 60.0);
    let vue = b.app.vignettes.view;
    // Le papier remplit toute la vue, sans carte ni coins arrondis, sans ombre.
    let papier = b.formes.iter().find_map(|s| match &s.shape {
        Shape::Rect(r) if r.rect == vue => Some(r.clone()),
        _ => None,
    });
    assert_eq!(papier.expect("le papier est peint sur toute la vue").fill, egui::Color32::WHITE);
    let l = b.app.board_layout();
    assert!(b.formes.iter().all(|s| !matches!(&s.shape, Shape::Rect(r) if r.fill == egui::Color32::WHITE && r.corner_radius.nw > 0 && r.rect.width() > vue.width() * 0.3)), "plus de carte blanche aux coins arrondis");
    // Chaque case pÃ¢le est un filet trÃ¨s lÃ©ger ; aucun Â« + Â».
    let pales: Vec<_> = l.empty.iter().map(|e| b.app.vignettes.xf.rect(*e)).collect();
    assert!(!pales.is_empty());
    for p in &pales {
        assert!(b.formes.iter().any(|s| matches!(&s.shape, Shape::Rect(r) if r.rect == *p && r.stroke.width > 0.0 && r.stroke.color.r() > 200)), "la case pÃ¢le {p:?} est tracÃ©e");
    }
    assert!(!b.formes.iter().any(|s| matches!(&s.shape, Shape::Text(t) if t.galley.job.text == egui_phosphor::regular::PLUS && pales.iter().any(|p| p.contains(t.pos + t.galley.size() / 2.0)))), "plus de Â« + Â» dans les cases pÃ¢les");
    // Le papier se dÃ©place dans tous les sens, loin au-delÃ  de la planche.
    b.app.board_pan = vec2(-5000.0, 4000.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.board_pan.x < -2000.0 && b.app.board_pan.y > 1500.0, "pas de butÃ©e proche : {:?}", b.app.board_pan);
}

#[test]
fn drawing_in_the_last_pale_row_makes_the_plans_before_it_and_a_new_pale_row_appears() {
    let mut b = ouvert(quatre_plans(), 4);
    let l = b.app.board_layout();
    // Quatre colonnes : six vignettes (deux rangÃ©es), puis deux cases pÃ¢les et une rangÃ©e pÃ¢le de quatre.
    assert_eq!(l.empty.len(), 2 + 4);
    let rangees = |l: &board::BoardLayout| {
        let mut h: Vec<i32> = l.empty.iter().map(|e| e.top().round() as i32).collect();
        h.dedup();
        h.len()
    };
    assert_eq!(rangees(&l), 2);
    // (Un peu plus petit, pour que la derniÃ¨re rangÃ©e pÃ¢le tienne dans la vue.)
    b.app.board_zoom = 0.6;
    b.chauffer(3);
    // Dessiner dans la toute derniÃ¨re case pÃ¢le : cinq plans vides avant elle, puis le plan dessinÃ©.
    let derniere = case_vide(&b, 5);
    let (plans, avant) = (b.app.doc.panels.len(), b.app.history.undo_len());
    b.image(vec![Event::PointerMoved(derniere.center())], 1.0 / 60.0);
    b.image(vec![appui(derniere.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..8 {
        b.image(vec![Event::PointerMoved(derniere.center() + vec2(k as f32 * 4.0, 0.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(derniere.center() + vec2(28.0, 0.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.panels.len(), plans + 6, "cinq plans vides pour garder l'ordre, et le plan dessinÃ©");
    assert_eq!(b.app.history.undo_len(), avant + 1, "une seule Ã©tape d'annulation");
    let l = b.app.board_layout();
    assert_eq!(l.cards.len(), 12, "trois rangÃ©es de quatre");
    assert_eq!(l.empty.len(), 4, "une rangÃ©e pÃ¢le de plus, la seule");
    assert!(l.empty.iter().all(|e| e.top() > l.cards.last().unwrap().thumb.bottom()));
    // Annuler : tout revient, d'un coup.
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(b.app.board_layout().empty.len(), 6);
}

#[test]
fn the_layout_of_two_hundred_plans_is_cheap() {
    let mut doc = crate::document::Document::with_frame(160, 90, 24);
    for _ in 0..199 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    // Une vingtaine de mouvements de camÃ©ra : travelling, diagonales, zoom arriÃ¨re.
    let a = doc.default_camera();
    for (k, i) in (0..200).step_by(10).enumerate() {
        let n = doc.panels[i].frame_count().max(2);
        doc.set_duration(i, n.max(25));
        let n = doc.panels[i].frame_count();
        let b = match k % 4 {
            0 => a.shifted(160.0, 0.0),
            1 => a.shifted(160.0, 90.0),
            2 => crate::camera::Camera { zoom: a.zoom / 2.0, ..a },
            _ => a.shifted(0.0, 90.0),
        };
        doc.panels[i].camera.clear();
        crate::camera::set_key(&mut doc.panels[i].camera, 0, a);
        crate::camera::set_key(&mut doc.panels[i].camera, n - 1, b);
    }
    let s = board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 };
    let notes = std::collections::HashMap::new();
    let opt = board::Options::pale(3);
    let l = board::layout_with(&doc, &s, &notes, false, &opt);
    assert_eq!(l.cards.len(), 200);
    assert!(l.cards.iter().filter(|c| c.mouvement).count() >= 20);
    // Chaque image d'Ã©cran recalcule la planche : mesurÃ©e sur plusieurs, le pire cas reste bien en deÃ§Ã  d'une image d'Ã©cran.
    // (La médiane de vingt mesures, pas la pire : un fil d'essai que le système suspend une milliseconde, la suite entière tournant en parallèle, ne dit rien du coût d'une disposition, 0,4 ms.)
    let mut mesures: Vec<std::time::Duration> = Vec::new();
    for _ in 0..20 {
        let t = std::time::Instant::now();
        let l = board::layout_with(&doc, &s, &notes, false, &opt);
        mesures.push(t.elapsed());
        assert_eq!(l.cards.len(), 200);
    }
    mesures.sort();
    let pire = mesures[mesures.len() / 2];
    eprintln!("mise en page de 200 plans (20 mouvements) : médiane {pire:?}");
    assert!(pire < std::time::Duration::from_millis(5), "200 plans, 20 mouvements : {pire:?} par disposition");
    // Et la disposition reste ordonnÃ©e : aucune case ne chevauche une autre.
    for (i, c) in l.cards.iter().enumerate() {
        for d in l.cards.iter().skip(i + 1) {
            if c.row.abs_diff(d.row) <= 2 {
                assert!(!c.foot.shrink(1.0).intersects(d.foot.shrink(1.0)) || c.mv.is_some() || d.mv.is_some(), "deux cases se chevauchent");
            }
        }
    }
}
#[test]
fn a_kept_stroke_stays_visible_a_little_beyond_the_case_edge_and_the_margin_holds_no_paper() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.color = MAGENTA;
    let case = vignette(&b, 1);
    let anneau = case.width() * board::RING_FRACTION;
    let depart = case.center();
    let fin = pos2(case.right() + 0.5 * anneau, depart.y);
    b.image(vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    b.image(vec![appui(depart, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![Event::PointerMoved(depart + (fin - depart) * (k as f32 / 12.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    se_reposer(&mut b, 400);
    let (id, doc_frame) = (b.app.board_layout().cards[1].id, b.app.doc.frame);
    let width = crate::thumbs::width_for(case.width() * b.ctx.pixels_per_point());
    let (img, marge) = b.app.thumbs.case_image_of(id, width).expect("la vignette de la case est composÃ©e");
    let (tw, th) = crate::thumbs::thumb_size_of(doc_frame, width, crate::thumbs::scale_pct());
    // (Partie 131 : une marge par côté, qui va jusqu'au dessin du plan. Le trait ne sort qu'à droite : là, la marge va au moins jusqu'à lui ;
    // des trois autres côtés, c'est celle du réglage.)
    let [marge, haut, droite, bas] = marge;
    assert_eq!((marge, haut, bas), (crate::thumbs::marge_px(tw), crate::thumbs::marge_px(tw), crate::thumbs::marge_px(tw)));
    assert!(droite >= marge, "à droite, la marge va au moins jusqu'au trait : {droite}");
    assert!((marge as f32 / tw as f32 - 0.12).abs() < 0.01, "la marge est de 12 % de la largeur de la case : {marge} pour {tw}");
    // Elle dépasse l'écart serré d'un même plan : les marges de deux cases voisines se recouvrent, la dernière peinte l'emporte.
    assert!(board::RING_FRACTION * 240.0 > board::gap_same(240.0), "la marge recouvre un peu la voisine");
    assert_eq!(img.size, [tw + marge + droite, th + 2 * marge], "le cadre et ses marges tout autour");
    let at = |x: usize, y: usize| img.pixels[y * img.size[0] + x];
    // Le papier ne dÃ©borde pas : les coins de la marge sont transparents, le cadre est blanc.
    assert_eq!(at(0, 0).a(), 0);
    assert_eq!(at(img.size[0] - 1, img.size[1] - 1).a(), 0);
    assert_eq!(at(img.size[0] / 2, 1).a(), 0, "au-dessus du cadre : rien");
    assert_eq!(at(marge + 1, marge + 1), egui::Color32::WHITE, "le cadre est le papier");
    // Le trait, lui, passe le bord droit et se voit dans la marge, Ã  la hauteur oÃ¹ on l'a tirÃ©.
    let y = marge + ((depart.y - case.top()) / case.height() * th as f32).round() as usize;
    let dans_la_marge = (marge + tw..marge + tw + droite).any(|x| (y.saturating_sub(2)..=y + 2).any(|yy| {
        let c = at(x, yy);
        c.a() > 200 && c.r() > 150 && c.b() > 150 && c.g() < 110
    }));
    assert!(dans_la_marge, "le trait de couleur reste visible juste aprÃ¨s le cadre (marge de {marge} pixels)");
}
