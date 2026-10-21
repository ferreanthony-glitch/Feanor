//! Essais de la planche « comme la maquette 07 v6 » (2026-10-21) : une carte blanche qui s'arrête après sa dernière rangée, une seule
//! case vide en « + », les proportions de la maquette, et le trait qui reste visible un peu au-delà du cadre de sa case.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Modifiers};

use crate::board;
use crate::perf_tests::se_reposer;
use crate::vignettes_tests::{appui, case_vide, ouvert, quatre_plans, vignette};

const MAGENTA: [u8; 3] = [255, 0, 255];

#[test]
fn the_board_ends_after_the_last_row_and_has_a_single_empty_case_instead_of_a_grid() {
    let doc = quatre_plans();
    for (colonnes, lignes) in [(4, 4), (6, 4), (3, 0), (4, 1)] {
        let s = board::BoardSettings { card_w: 240.0, columns: colonnes, rows: lignes };
        let l = board::layout(&doc, &s);
        assert_eq!(l.empty.len(), 1, "{colonnes} colonnes : une seule case vide");
        // Elle suit la dernière vignette : dans sa rangée si elle y tient, sinon au début de la rangée suivante.
        let derniere = l.cards.last().unwrap();
        let e = l.empty[0];
        assert!(e.left() >= derniere.thumb.right() || e.top() > derniere.thumb.top(), "après la dernière vignette");
        // La planche ne dépasse jamais la rangée de la case vide : rien dessous que la bande de note et la marge de la carte.
        let bas = l.pages.last().unwrap().bottom();
        assert!((bas - (e.bottom() + board::NOTE_GAP + board::NOTE_LINE_H + board::PAGE_PAD)).abs() < 1e-3, "{colonnes} colonnes, {lignes} lignes : la carte s'arrête sous la case vide");
        assert!((l.size.y - (bas + board::MARGIN)).abs() < 1e-3, "la planche s'arrête après la carte et sa marge");
    }
    // Une planche de plus de rangées par page que de rangées utiles n'est plus complétée.
    let court = board::layout(&doc, &board::BoardSettings { card_w: 240.0, columns: 6, rows: 12 });
    assert_eq!(court.pages.len(), 1);
    assert!(court.pages[0].height() < 3.0 * (board::LABEL_H + 240.0 * 90.0 / 160.0 + board::NOTE_GAP + board::NOTE_LINE_H + board::ROW_GAP), "deux rangées au plus : {}", court.pages[0].height());
}

#[test]
fn the_card_is_white_and_rounded_with_a_faint_plus_in_the_empty_case() {
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_formes = true;
    b.garder_textes = true;
    b.image(Vec::new(), 1.0 / 60.0);
    let page = b.app.vignettes.xf.rect(b.app.board_layout().pages[0]);
    let carte = b
        .formes
        .iter()
        .find_map(|s| match &s.shape {
            Shape::Rect(r) if r.rect == page => Some(r.clone()),
            _ => None,
        })
        .expect("la carte blanche est peinte");
    assert_eq!(carte.fill, egui::Color32::WHITE);
    assert!(carte.corner_radius.nw > 0, "coins arrondis");
    // Pas d'ombre portée : aucune autre forme pleine de la taille de la carte.
    assert_eq!(b.formes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.rect.size() == page.size() && r.fill != egui::Color32::TRANSPARENT)).count(), 1);
    let vide = b.app.vignettes.xf.rect(b.app.board_layout().empty[0]);
    let plus = b.formes.iter().filter(|s| matches!(&s.shape, Shape::Text(t) if t.galley.job.text == egui_phosphor::regular::PLUS && vide.contains(t.pos + t.galley.size() / 2.0))).count();
    assert_eq!(plus, 1, "un seul « + », au milieu de la case vide");
    // Dessiner dans cette case fait toujours le plan.
    let e = case_vide(&b, 0);
    let (plans, avant) = (b.app.doc.panels.len(), b.app.history.undo_len());
    b.image(vec![Event::PointerMoved(e.center())], 1.0 / 60.0);
    b.image(vec![appui(e.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..8 {
        b.image(vec![Event::PointerMoved(e.center() + vec2(k as f32 * 4.0, 0.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(e.center() + vec2(28.0, 0.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.panels.len(), plans + 1);
    assert_eq!(b.app.history.undo_len(), avant + 1, "une seule étape d'annulation");
}

#[test]
fn a_kept_stroke_stays_visible_a_little_beyond_the_case_edge_and_the_margin_holds_no_paper() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.color = MAGENTA;
    let case = vignette(&b, 1);
    let anneau = case.width() * board::RING_FRACTION;
    let depart = case.center();
    let fin = pos2(case.right() + 3.0 * anneau, depart.y);
    b.image(vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    b.image(vec![appui(depart, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![Event::PointerMoved(depart + (fin - depart) * (k as f32 / 12.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    se_reposer(&mut b, 400);
    let (id, doc_frame) = (b.app.board_layout().cards[1].id, b.app.doc.frame);
    let width = crate::thumbs::width_for(case.width() * b.ctx.pixels_per_point());
    let (img, marge) = b.app.thumbs.case_image_of(id, width).expect("la vignette de la case est composée");
    let (tw, th) = crate::thumbs::thumb_size_of(doc_frame, width, crate::thumbs::scale_pct());
    assert_eq!(marge, crate::thumbs::marge_px(tw));
    assert_eq!(img.size, [tw + 2 * marge, th + 2 * marge], "le cadre et sa marge tout autour");
    let at = |x: usize, y: usize| img.pixels[y * img.size[0] + x];
    // Le papier ne déborde pas : les coins de la marge sont transparents, le cadre est blanc.
    assert_eq!(at(0, 0).a(), 0);
    assert_eq!(at(img.size[0] - 1, img.size[1] - 1).a(), 0);
    assert_eq!(at(img.size[0] / 2, 1).a(), 0, "au-dessus du cadre : rien");
    assert_eq!(at(marge + 1, marge + 1), egui::Color32::WHITE, "le cadre est le papier");
    // Le trait, lui, passe le bord droit et se voit dans la marge, à la hauteur où on l'a tiré.
    let y = marge + ((depart.y - case.top()) / case.height() * th as f32).round() as usize;
    let dans_la_marge = (marge + tw..marge + tw + marge).any(|x| (y.saturating_sub(2)..=y + 2).any(|yy| {
        let c = at(x, yy);
        c.a() > 200 && c.r() > 150 && c.b() > 150 && c.g() < 110
    }));
    assert!(dans_la_marge, "le trait de couleur reste visible juste après le cadre (marge de {marge} pixels)");
}
