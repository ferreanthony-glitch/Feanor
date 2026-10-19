//! Tests des crayonnés rapides (`crayonnes.rs`, `crayonnesui.rs`) : un trait appartient à la case où
//! il commence, il est coupé à sa case, Tab et les flèches changent de case, on change l'ordre des
//! cases, « Créer les plans » (nombre, ordre, durées, légendes, dessin mis à l'échelle en vectoriel et
//! en bitmap, une seule étape d'annulation), le fichier (aller-retour, protections), l'annulation de
//! la planche, et le dessin à l'écran sans latence.

use eframe::egui::{self, pos2, vec2, Color32, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::app::{App, Mode, Tool};
use crate::crayonnes::{
    self, b64_decode, b64_encode, box_size, clip_stroke, create_plans, fit, hit, layout, navigate, parse, to_local, Cell, Move, Outil, PEdit, PageHistory, PlanOptions, Placement, Planche, Style,
    Undone, BOX_LONG, GRIDS,
};
use crate::document::Document;
use crate::fiabilite_fixture::{arbre, dossier, empreinte, racine};
use crate::perf_tests::{banc_taille, Banc};
use crate::project;
use crate::raster::Raster;
use crate::vector::{Element, VPoint, VectorDrawing};

// ------------------------------------------------------------------------------------ outils d'essai

/// Un projet de 320 × 180 (les cases font 640 × 360 : un trait y est réduit de moitié).
fn doc_small() -> Document {
    Document::with_frame(320, 180, 24)
}

fn planche_for(doc: &Document) -> Planche {
    let mut p = Planche::default();
    p.ensure(doc.frame);
    p
}

fn style() -> Style {
    Style::simple([20, 20, 20], 6.0)
}

/// Trace un trait de `points` (repère de la case `index`) avec `tool`, comme le fait la souris.
fn stroke(p: &mut Planche, index: usize, tool: Outil, points: &[Pos2]) -> Option<PEdit> {
    let mut live = p.begin(index, tool, &style(), points[0], 1.0, false, 0.0)?;
    for (k, q) in points.iter().enumerate().skip(1) {
        live.to(p, *q, 1.0, k as f64 * 0.01);
    }
    live.end(p)
}

fn elements(p: &Planche, index: usize) -> usize {
    p.cases[index].ink.vector.as_ref().map_or(0, |v| v.elements.len())
}

fn has_pixels(r: &Raster) -> bool {
    r.tiles.iter().flatten().any(|t| t.iter().any(|c| c.a() != 0))
}

/// Les pixels visibles d'un dessin : (x min, y min, x max, y max).
fn bounds_of(r: &Raster) -> Option<(usize, usize, usize, usize)> {
    let mut b: Option<(usize, usize, usize, usize)> = None;
    for y in 0..r.height {
        for x in 0..r.width {
            if r.get(x, y).a() > 0 {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
                });
            }
        }
    }
    b
}

fn layer_cel<'a>(doc: &'a Document, panel: usize, name: &str) -> Option<&'a crate::document::Cel> {
    let l = doc.panels[panel].layers.iter().find(|l| l.name == name)?;
    let id = l.frames.iter().flatten().next()?;
    doc.cels.get(id)
}

fn vector_points(doc: &Document, panel: usize) -> Vec<Vec<VPoint>> {
    let cel = layer_cel(doc, panel, "Crayonné").expect("calque Crayonné dessiné");
    cel.vector.as_ref().expect("vectoriel").elements.iter().filter_map(|e| if let Element::Stroke { points, .. } = e { Some(points.clone()) } else { None }).collect()
}

fn line(a: (f32, f32), b: (f32, f32)) -> VectorDrawing {
    VectorDrawing { elements: vec![Element::Stroke { points: vec![VPoint { x: a.0, y: a.1, r: 4.0 }, VPoint { x: b.0, y: b.1, r: 4.0 }], color: [0, 0, 0], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() }] }
}

// ------------------------------------------------------------------------------------ cases et pages

#[test]
fn a_fresh_planche_has_one_page_of_blank_boxes_in_the_frame_ratio() {
    let doc = doc_small();
    let p = planche_for(&doc);
    assert_eq!(p.grid, (4, 3));
    assert_eq!(p.len(), 12);
    assert_eq!(p.page_count(), 1);
    assert_eq!(p.size, [BOX_LONG, 360], "16:9 : 640 × 360");
    assert!(p.is_blank(), "rien à enregistrer");
    // Des cadres de toutes sortes.
    assert_eq!(box_size([1920, 1080]), [640, 360]);
    assert_eq!(box_size([1080, 1920]), [360, 640]);
    assert_eq!(box_size([1000, 1000]), [640, 640]);
    assert_eq!(box_size([2390, 1000]), [640, 268]);
}

#[test]
fn pages_follow_the_grid_and_boxes_are_added_page_by_page() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    assert_eq!(p.add_page(), 12);
    assert_eq!((p.len(), p.page_count()), (24, 2));
    assert_eq!(p.page_range(1), 12..24);
    assert_eq!(p.page_of(11), 0);
    assert_eq!(p.page_of(12), 1);
    // Une grille plus petite : plus de pages, les mêmes cases dans le même ordre.
    let ids: Vec<u64> = p.cases.iter().map(|c| c.id).collect();
    assert!(p.set_grid(2, 2));
    assert_eq!(p.page_count(), 6);
    assert_eq!(p.cases.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
    assert!(!p.set_grid(2, 2), "même grille : rien ne change");
    for (c, r) in GRIDS {
        assert!(p.set_grid(c, r) || p.grid == (c, r));
        assert_eq!(p.per_page(), c * r);
    }
    // Une page ajoutée complète d'abord la dernière.
    let mut q = planche_for(&doc);
    q.insert_after(3);
    assert_eq!(q.len(), 13);
    assert_eq!(q.add_page(), 24, "la seconde page est complétée avant la troisième");
    assert_eq!(q.len(), 36);
}

#[test]
fn boxes_are_inserted_removed_and_reordered_by_drag_and_drop() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    let ids: Vec<u64> = p.cases.iter().map(|c| c.id).collect();
    p.cases[0].caption = "A".into();
    p.cases[1].caption = "B".into();
    p.cases[2].caption = "C".into();
    // Glisser la case 0 sur la place 2 : B, C, A.
    assert!(p.move_case(0, 2));
    let caps: Vec<&str> = p.cases.iter().take(3).map(|c| c.caption.as_str()).collect();
    assert_eq!(caps, ["B", "C", "A"]);
    assert_eq!(p.cases[2].id, ids[0], "la case garde son identifiant");
    // Et en arrière.
    assert!(p.move_case(2, 0));
    assert_eq!(p.cases.iter().take(3).map(|c| c.caption.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
    assert!(!p.move_case(1, 1) && !p.move_case(0, 99) && !p.move_case(99, 0));
    // Insérer après la case 1.
    let at = p.insert_after(1);
    assert_eq!(at, 2);
    assert_eq!(p.len(), 13);
    assert_eq!(p.cases[3].caption, "C");
    assert!(p.cases[2].is_blank());
    let before = p.insert_before(0);
    assert_eq!(before, 0);
    assert_eq!(p.cases[1].caption, "A");
    // Supprimer.
    assert!(p.remove(0));
    assert!(p.remove(2));
    assert_eq!(p.len(), 12);
    assert!(!p.remove(99));
    // La dernière case qui reste est vidée, pas retirée.
    let mut one = planche_for(&doc);
    while one.len() > 1 {
        one.remove(0);
    }
    one.cases[0].caption = "seule".into();
    assert!(one.remove(0));
    assert_eq!(one.len(), 1);
    assert!(one.cases[0].is_blank());
}

#[test]
fn clearing_a_box_keeps_its_caption_and_loses_its_drawing() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    stroke(&mut p, 0, Outil::Crayon, &[pos2(100.0, 100.0), pos2(300.0, 200.0)]).expect("un trait");
    p.cases[0].caption = "gros plan".into();
    assert!(p.cases[0].has_ink() && has_pixels(&p.cases[0].ink.raster));
    assert!(p.clear_case(0));
    assert!(!p.cases[0].has_drawing() && !has_pixels(&p.cases[0].ink.raster));
    assert_eq!(p.cases[0].caption, "gros plan");
    assert!(p.cases[0].dirty.all, "l'écran doit tout renvoyer");
}

// ------------------------------------------------------------------------------------ navigation

#[test]
fn tab_and_arrows_go_to_the_next_box_across_pages() {
    // 24 cases, 4 colonnes.
    let n = |i, mv| navigate(i, 24, 4, mv);
    assert_eq!(n(0, Move::Next), 1);
    assert_eq!(n(11, Move::Next), 12, "de page en page");
    assert_eq!(n(23, Move::Next), 23, "au bout, on reste");
    assert_eq!(n(5, Move::Prev), 4);
    assert_eq!(n(0, Move::Prev), 0);
    assert_eq!(n(5, Move::Right), 6);
    assert_eq!(n(5, Move::Left), 4);
    assert_eq!(n(5, Move::Down), 9);
    assert_eq!(n(9, Move::Down), 13, "la dernière ligne mène à la première de la page suivante");
    assert_eq!(n(5, Move::Up), 1);
    assert_eq!(n(2, Move::Up), 2, "en haut, on reste");
    assert_eq!(n(20, Move::Down), 20, "dernière ligne de la dernière page : on reste");
    // Moins de cases que la grille.
    assert_eq!(navigate(5, 7, 4, Move::Down), 5, "rien dessous");
    assert_eq!(navigate(2, 7, 4, Move::Down), 6);
    assert_eq!(navigate(0, 0, 4, Move::Next), 0);
    assert_eq!(navigate(50, 7, 4, Move::Prev), 5, "une case hors bornes est ramenée");
}

#[test]
fn choosing_boxes_with_click_ctrl_and_shift() {
    let ids: Vec<u64> = (10..20).collect();
    let (s, a) = crayonnes::choose(&[], None, &ids, 3, false, false);
    assert_eq!((s.as_slice(), a), (&[13][..], Some(3)));
    let (s, a) = crayonnes::choose(&s, a, &ids, 6, true, false);
    assert_eq!((s.as_slice(), a), (&[13, 16][..], Some(6)));
    let (s, a) = crayonnes::choose(&s, a, &ids, 3, true, false);
    assert_eq!((s.as_slice(), a), (&[16][..], Some(3)), "Ctrl retire une case déjà choisie");
    let (s, _) = crayonnes::choose(&s, Some(3), &ids, 7, false, true);
    assert_eq!(s, vec![13, 14, 15, 16, 17], "Maj : de la dernière cliquée à celle-ci");
    let (s, _) = crayonnes::choose(&s, Some(7), &ids, 5, false, true);
    assert_eq!(s, vec![15, 16, 17], "dans l'autre sens aussi");
    let (s, a) = crayonnes::choose(&[10], Some(0), &ids, 99, false, false);
    assert_eq!((s, a), (vec![10], Some(0)), "une case qui n'existe pas ne change rien");
}

#[test]
fn the_keys_move_the_current_box_only_when_the_planche_has_the_hand() {
    let (_ctx, mut app) = headless("touches");
    app.welcome = false;
    app.run_action("storyboard.crayonnes");
    assert!(app.crayonnes_shown());
    app.crayonnes_prepare_for_tests();
    assert_eq!(app.crayonnes.current, 0);
    assert!(app.crayonnes_key(Key::Tab, Modifiers::NONE));
    assert_eq!(app.crayonnes.current, 1);
    assert!(app.crayonnes_key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(app.crayonnes.current, 5);
    assert!(app.crayonnes_key(Key::Tab, Modifiers::SHIFT));
    assert_eq!(app.crayonnes.current, 4);
    assert!(app.crayonnes_key(Key::PageDown, Modifiers::NONE));
    assert_eq!(app.crayonnes.page, 0, "une seule page : pas de page suivante");
    // Ctrl + flèche garde son sens ailleurs.
    assert!(!app.crayonnes_key(Key::ArrowRight, Modifiers::COMMAND));
    // Le montage reprend la main : Tab redevient « passer en animation ».
    app.panel_focus = true;
    assert!(!app.crayonnes_has_focus());
    assert!(!app.crayonnes_key(Key::Tab, Modifiers::NONE));
    assert_eq!(app.crayonnes.current, 4);
    // Fermée, la planche ne prend rien.
    app.panel_focus = false;
    app.run_action("storyboard.crayonnes");
    assert!(!app.crayonnes_shown());
    assert!(!app.crayonnes_key(Key::Tab, Modifiers::NONE));
}

// ------------------------------------------------------------------------------------ disposition

#[test]
fn the_page_layout_fits_every_grid_and_keeps_the_ratio() {
    for (cols, rows) in GRIDS {
        for avail in [Rect::from_min_size(pos2(10.0, 40.0), vec2(1400.0, 700.0)), Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 900.0)), Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 200.0))] {
            for aspect in [16.0 / 9.0, 1.0, 9.0 / 16.0, 2.39] {
                let cells = layout(avail, cols, rows, aspect);
                assert_eq!(cells.len(), cols * rows);
                for c in &cells {
                    assert!((c.frame.width() / c.frame.height() - aspect).abs() < 1e-3, "le rapport de la case est celui du cadre");
                    if avail.width() > 400.0 && avail.height() > 400.0 {
                        assert!(avail.contains_rect(c.frame) && avail.contains_rect(c.caption), "{cols}×{rows} : la case sort de l'espace ({:?} dans {avail:?})", c.frame);
                    }
                    assert!(c.frame.contains_rect(c.badge) || c.frame.intersects(c.badge));
                    assert!(c.caption.top() >= c.frame.bottom(), "la légende est sous la case");
                }
                for (i, a) in cells.iter().enumerate() {
                    for b in cells.iter().skip(i + 1) {
                        assert!(!a.frame.intersects(b.frame) && !a.frame.intersects(b.caption), "les cases ne se recouvrent pas");
                    }
                }
            }
        }
    }
    // Toucher une case.
    let cells = layout(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 700.0)), 4, 3, 16.0 / 9.0);
    assert_eq!(hit(&cells, cells[5].frame.center()), Some(5));
    assert_eq!(hit(&cells, cells[5].caption.center()), None, "la légende n'est pas le dessin");
    assert_eq!(hit(&cells, pos2(-5.0, -5.0)), None);
}

#[test]
fn a_screen_point_becomes_a_point_of_its_box() {
    let frame = Rect::from_min_size(pos2(100.0, 50.0), vec2(320.0, 180.0));
    let size = [640, 360];
    assert_eq!(to_local(frame, size, pos2(100.0, 50.0)), pos2(0.0, 0.0));
    assert_eq!(to_local(frame, size, pos2(260.0, 140.0)), pos2(320.0, 180.0));
    assert_eq!(to_local(frame, size, pos2(420.0, 230.0)), pos2(640.0, 360.0));
    let out = to_local(frame, size, pos2(500.0, 20.0));
    assert!(out.x > 640.0 && out.y < 0.0, "un point hors de la case reste dans son repère : le trait y sort sans changer de case");
}

// ------------------------------------------------------------------------------------ appartenance et coupe

#[test]
fn a_stroke_belongs_to_the_box_where_it_starts() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    let cells = layout(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 700.0)), 4, 3, 16.0 / 9.0);
    // Le glisser part dans la case 5 et traverse la case 6 jusqu'à la case 10.
    let start = cells[5].frame.center();
    let path = [start, cells[6].frame.center(), cells[10].frame.center()];
    let owner = hit(&cells, path[0]).expect("le départ est dans une case");
    assert_eq!(owner, 5);
    let local: Vec<Pos2> = path.iter().map(|q| to_local(cells[owner].frame, p.size, *q)).collect();
    stroke(&mut p, owner, Outil::Crayon, &local).expect("un trait");
    assert_eq!(elements(&p, 5), 1, "le trait est à la case où il a commencé");
    for i in (0..p.len()).filter(|&i| i != 5) {
        assert_eq!(elements(&p, i), 0, "case {i}");
        assert!(!has_pixels(&p.cases[i].ink.raster), "case {i} : aucun pixel");
    }
    // Un appui entre deux cases ne trace rien.
    let gap = pos2((cells[0].frame.right() + cells[1].frame.left()) * 0.5, cells[0].frame.center().y);
    assert_eq!(hit(&cells, gap), None);
}

#[test]
fn a_stroke_leaving_its_box_is_clipped_on_display_and_in_the_plan() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    // De la case vers la droite, très loin dehors, et retour.
    let pts = [pos2(500.0, 100.0), pos2(900.0, 100.0), pos2(900.0, 250.0), pos2(500.0, 250.0)];
    stroke(&mut p, 0, Outil::Crayon, &pts).expect("un trait");
    let r = &p.cases[0].ink.raster;
    assert_eq!((r.width, r.height), (640, 360), "la case est un dessin à ses bornes : rien ne dépasse à l'affichage");
    let b = bounds_of(r).expect("des pixels");
    assert!(b.2 < 640 && b.3 < 360 && b.0 >= 400, "{b:?}");
    // Le trait gardé sort bel et bien de la case (il suit la main)…
    let kept = p.cases[0].ink.vector.as_ref().expect("encre").elements.clone();
    assert!(matches!(&kept[0], Element::Stroke { points, .. } if points.iter().any(|q| q.x > 700.0)));
    // …mais dans le plan, il est coupé à la case mise à l'échelle.
    let mut d = doc_small();
    let created = create_plans(&mut d, &p, &[0], 0, &PlanOptions::default()).expect("un plan");
    let frame = d.frame_rect();
    for points in vector_points(&d, created.first) {
        for q in points {
            assert!(q.x <= frame.right() + 0.01 && q.x >= frame.left() - 0.01 && q.y >= frame.top() - 0.01 && q.y <= frame.bottom() + 0.01, "{q:?} sort du cadre {frame:?}");
        }
    }
}

#[test]
fn clipping_a_polyline_to_a_rectangle() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
    let v = |x: f32, y: f32, r: f32| VPoint { x, y, r };
    // Dedans : intact.
    let inside = clip_stroke(&[v(10.0, 10.0, 2.0), v(50.0, 60.0, 4.0)], rect);
    assert_eq!(inside, vec![vec![v(10.0, 10.0, 2.0), v(50.0, 60.0, 4.0)]]);
    // Entièrement dehors : rien.
    assert!(clip_stroke(&[v(150.0, 10.0, 2.0), v(250.0, 60.0, 4.0)], rect).is_empty());
    // Sort à droite : coupé au bord, rayon interpolé.
    let out = clip_stroke(&[v(50.0, 50.0, 2.0), v(150.0, 50.0, 6.0)], rect);
    assert_eq!(out.len(), 1);
    let last = out[0].last().expect("point");
    assert!((last.x - 100.0).abs() < 1e-3 && (last.r - 4.0).abs() < 1e-3, "{last:?}");
    // Sort et rentre : deux morceaux.
    let two = clip_stroke(&[v(50.0, 20.0, 3.0), v(150.0, 20.0, 3.0), v(150.0, 80.0, 3.0), v(50.0, 80.0, 3.0)], rect);
    assert_eq!(two.len(), 2, "{two:?}");
    assert!(two.iter().flatten().all(|q| rect.contains(q.pos())));
    // Un seul point.
    assert_eq!(clip_stroke(&[v(5.0, 5.0, 1.0)], rect).len(), 1);
    assert!(clip_stroke(&[v(500.0, 5.0, 1.0)], rect).is_empty());
    assert!(clip_stroke(&[], rect).is_empty());
    // Un trait qui traverse la case sans y avoir de point.
    let across = clip_stroke(&[v(-50.0, 50.0, 2.0), v(150.0, 50.0, 2.0)], rect);
    assert_eq!(across.len(), 1);
    assert!((across[0][0].x - 0.0).abs() < 1e-3 && (across[0][1].x - 100.0).abs() < 1e-3);
}

// ------------------------------------------------------------------------------------ gomme, pinceau, annulation

#[test]
fn the_pencil_is_vector_and_the_brush_is_bitmap_and_the_eraser_cuts_both() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    let pencil = stroke(&mut p, 0, Outil::Crayon, &[pos2(100.0, 180.0), pos2(500.0, 180.0)]);
    assert!(matches!(pencil, Some(PEdit::Ink { .. })));
    assert_eq!(elements(&p, 0), 1);
    assert!(has_pixels(&p.cases[0].ink.raster) && !p.cases[0].has_paint());
    let brush = stroke(&mut p, 0, Outil::Pinceau, &[pos2(100.0, 100.0), pos2(500.0, 100.0)]);
    assert!(matches!(brush, Some(PEdit::Paint { .. })));
    assert!(p.cases[0].has_paint(), "le pinceau peint dans l'image de la case");
    assert_eq!(elements(&p, 0), 1, "il ne crée pas de trait vectoriel");
    // La gomme en travers des deux : le trait est coupé, la peinture est trouée.
    let paint_before = p.cases[0].paint.raster.to_rgba();
    let ink_before = p.cases[0].ink.vector.clone();
    let erased = stroke(&mut p, 0, Outil::Gomme, &[pos2(300.0, 40.0), pos2(300.0, 300.0)]);
    assert!(matches!(erased, Some(PEdit::Group(_))), "une étape pour les deux");
    assert!(elements(&p, 0) >= 2, "le trait est coupé en deux morceaux");
    assert_ne!(p.cases[0].paint.raster.to_rgba(), paint_before);
    assert_eq!(p.cases[0].paint.raster.get(300, 100).a(), 0, "la peinture est effacée sous la gomme");
    assert_ne!(p.cases[0].ink.vector, ink_before);
    // Une seule annulation rend les deux.
    let mut history = PageHistory::default();
    history.push(erased.expect("étape"));
    assert_eq!(history.undo(&mut p), Undone::Page);
    assert_eq!(p.cases[0].ink.vector, ink_before);
    assert_eq!(p.cases[0].paint.raster.to_rgba(), paint_before);
    assert_eq!(history.redo(&mut p), Undone::Page);
    assert!(elements(&p, 0) >= 2);
    assert_eq!(p.cases[0].paint.raster.get(300, 100).a(), 0);
}

#[test]
fn the_eraser_on_an_empty_box_does_nothing() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    assert!(stroke(&mut p, 3, Outil::Gomme, &[pos2(10.0, 10.0), pos2(200.0, 100.0)]).is_none());
    assert!(p.is_blank());
}

#[test]
fn the_planche_history_undoes_and_redoes_strokes_and_structure() {
    let doc = doc_small();
    let mut p = planche_for(&doc);
    let mut h = PageHistory::default();
    assert_eq!(h.undo(&mut p), Undone::Nothing);
    // Deux traits : deux étapes.
    for k in 0..2 {
        let y = 80.0 + 60.0 * k as f32;
        h.push(stroke(&mut p, 1, Outil::Crayon, &[pos2(50.0, y), pos2(400.0, y)]).expect("trait"));
    }
    assert_eq!(elements(&p, 1), 2);
    assert_eq!(h.undo(&mut p), Undone::Page);
    assert_eq!(elements(&p, 1), 1);
    assert!(h.can_redo());
    // Une nouvelle étape efface le futur.
    h.push(stroke(&mut p, 1, Outil::Crayon, &[pos2(50.0, 300.0), pos2(400.0, 300.0)]).expect("trait"));
    assert!(!h.can_redo());
    assert_eq!(h.undo(&mut p), Undone::Page);
    assert_eq!(h.undo(&mut p), Undone::Page);
    assert_eq!(elements(&p, 1), 0);
    assert!(!has_pixels(&p.cases[1].ink.raster), "l'image suit le dessin");
    // Une étape de structure : la planche entière.
    p.cases[0].caption = "avant".into();
    let ids: Vec<u64> = p.cases.iter().map(|c| c.id).collect();
    let before = Box::new(p.clone());
    p.move_case(0, 5);
    p.set_grid(2, 2);
    h.push(PEdit::Whole(before));
    assert_ne!(p.cases.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
    assert_eq!(h.undo(&mut p), Undone::Page);
    assert_eq!(p.cases.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
    assert_eq!((p.grid, p.cases[0].caption.as_str()), ((4, 3), "avant"));
    assert!(p.cases.iter().all(|c| c.dirty.all), "tout est renvoyé à l'écran");
    assert_eq!(h.redo(&mut p), Undone::Page);
    assert_eq!(p.grid, (2, 2));
}

// ------------------------------------------------------------------------------------ créer les plans

/// Une planche de trois cases : un trait (0), une légende seule (1), de la peinture et un trait (2).
fn planche_of_three(doc: &Document) -> Planche {
    let mut p = planche_for(doc);
    p.cases[0].ink.vector = Some(line((100.0, 50.0), (300.0, 150.0)));
    p.cases[0].caption = "  Plan large  ".into();
    p.cases[1].caption = "Gros plan sur la main".into();
    for y in 60..80 {
        for x in 100..120 {
            let i = p.cases[2].paint.raster.tile_index_of(x, y);
            p.cases[2].paint.raster.tile_mut(i)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(0, 0, 0, 255);
        }
    }
    p.cases[2].ink.vector = Some(line((20.0, 20.0), (60.0, 60.0)));
    // Leur image, comme si on les avait tracés.
    p.cases[0].ink.rerender();
    p.cases[2].ink.rerender();
    p
}

#[test]
fn creating_plans_makes_one_plan_per_box_in_order_with_duration_and_caption() {
    let mut doc = doc_small();
    let p = planche_of_three(&doc);
    let first_id = doc.panels[0].id;
    let created = create_plans(&mut doc, &p, &[0, 1, 2], 0, &PlanOptions { placement: Placement::AfterCurrent, seconds: 3.0 }).expect("plans");
    assert_eq!(created.ids.len(), 3);
    assert_eq!(doc.panels.len(), 4, "le plan courant et trois plans");
    assert_eq!(doc.panels[0].id, first_id, "le plan courant ne bouge pas");
    assert_eq!(doc.panels.iter().skip(1).map(|q| q.id).collect::<Vec<_>>(), created.ids, "dans l'ordre des cases, juste après le plan courant");
    assert_eq!(created.first, 1);
    for q in doc.panels.iter().skip(1) {
        assert_eq!(q.frame_count(), 72, "3 s à 24 images par seconde");
        assert_eq!(q.length, 72);
    }
    // Chaque plan est une scène à lui, dans la séquence du plan courant.
    let scenes: std::collections::HashSet<u64> = doc.panels.iter().map(|q| q.scene).collect();
    assert_eq!(scenes.len(), 4);
    assert_eq!(doc.sequences.len(), 1);
    assert!(doc.is_plans());
    // Les légendes (nettoyées) vont dans le texte d'action.
    let actions: Vec<&str> = doc.panels.iter().skip(1).map(|q| q.action.as_str()).collect();
    assert_eq!(actions, ["Plan large", "Gros plan sur la main", ""]);
    // Le calque « Crayonné » est sous un calque de dessin vide.
    for q in doc.panels.iter().skip(1) {
        let names: Vec<&str> = q.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names[0], "Crayonné");
        assert_eq!(*names.last().expect("calque"), "Dessin");
    }
    assert!(doc.panels[2].layers[0].frames.iter().all(Option::is_none), "case sans dessin : calque Crayonné vide");
    assert!(layer_cel(&doc, 3, "Crayonné (pinceau)").is_some(), "la peinture a son calque bitmap");
    assert!(layer_cel(&doc, 1, "Crayonné (pinceau)").is_none());
    // La durée par défaut est de deux secondes.
    let mut other = doc_small();
    create_plans(&mut other, &p, &[1], 0, &PlanOptions::default()).expect("plan");
    assert_eq!(other.panels[1].frame_count(), 48);
}

#[test]
fn plans_can_start_a_new_sequence_after_the_current_one() {
    let mut doc = doc_small();
    doc.add_sequence_after(0);
    doc.add_scene_after(1);
    assert_eq!((doc.sequences.len(), doc.panels.len()), (2, 3));
    let p = planche_of_three(&doc);
    let created = create_plans(&mut doc, &p, &[0, 1], 0, &PlanOptions { placement: Placement::NewSequence, seconds: 1.0 }).expect("plans");
    assert_eq!(doc.sequences.len(), 3, "une séquence de plus");
    // La première séquence garde son plan ; les deux plans neufs forment la séquence suivante.
    assert_eq!(doc.panels.len(), 5);
    let seq = doc.sequence_ids();
    assert_eq!(seq[1], seq[2]);
    assert_ne!(seq[0], seq[1]);
    assert_ne!(seq[2], seq[3], "les anciennes séquences viennent après");
    assert_eq!(doc.panels[1].id, created.ids[0]);
    assert_eq!(doc.panels[2].id, created.ids[1]);
    assert_eq!(created.first, 1);
}

#[test]
fn the_vector_drawing_stays_vector_and_is_scaled_to_the_frame() {
    // Cadre 320 × 180, cases 640 × 360 : échelle 0,5.
    let mut doc = doc_small();
    let p = planche_of_three(&doc);
    let created = create_plans(&mut doc, &p, &[0], 0, &PlanOptions::default()).expect("plan");
    let cel = layer_cel(&doc, created.first, "Crayonné").expect("dessin");
    assert!(cel.vector.is_some(), "un trait reste un trait");
    let frame = doc.frame_rect();
    let pts = vector_points(&doc, created.first);
    assert_eq!(pts.len(), 1);
    let (a, b) = (pts[0][0], pts[0][pts[0].len() - 1]);
    assert!((a.x - (frame.left() + 50.0)).abs() < 1e-3 && (a.y - (frame.top() + 25.0)).abs() < 1e-3, "{a:?} {frame:?}");
    assert!((b.x - (frame.left() + 150.0)).abs() < 1e-3 && (b.y - (frame.top() + 75.0)).abs() < 1e-3, "{b:?}");
    assert!((a.r - 2.0).abs() < 1e-3, "la largeur suit l'échelle : 4 → 2, {}", a.r);
    assert!(has_pixels(&cel.raster), "le dessin est rendu");
    // Plus grand : un cadre de 1280 × 720 double les cases.
    let mut big = Document::with_frame(1280, 720, 24);
    let q = planche_of_three(&big);
    let c = create_plans(&mut big, &q, &[0], 0, &PlanOptions::default()).expect("plan");
    let f = big.frame_rect();
    let pts = vector_points(&big, c.first);
    assert!((pts[0][0].x - (f.left() + 200.0)).abs() < 1e-3 && (pts[0][0].r - 8.0).abs() < 1e-3, "{:?}", pts[0][0]);
    // L'affine d'une case dans un cadre d'un autre rapport : uniforme, centrée.
    let a = fit([640, 360], Rect::from_min_size(pos2(10.0, 20.0), vec2(300.0, 300.0)));
    assert!((a.a - 300.0 / 640.0).abs() < 1e-4 && a.a == a.d);
    assert!((a.tx - 10.0).abs() < 1e-3 && a.ty > 20.0);
}

#[test]
fn the_bitmap_painting_is_resampled_to_the_frame() {
    // Réduite de moitié : le carré (100..120, 60..80) de la case devient (50..60, 30..40) dans le cadre.
    let mut doc = doc_small();
    let p = planche_of_three(&doc);
    let created = create_plans(&mut doc, &p, &[2], 0, &PlanOptions::default()).expect("plan");
    let frame = doc.frame_rect();
    let cel = layer_cel(&doc, created.first, "Crayonné (pinceau)").expect("calque bitmap");
    assert!(cel.vector.is_none(), "la peinture reste du bitmap");
    assert_eq!((cel.raster.width, cel.raster.height), (doc.width, doc.height));
    let (x0, y0, x1, y1) = bounds_of(&cel.raster).expect("des pixels");
    let (fx, fy) = (frame.left() as usize, frame.top() as usize);
    assert!((x0 as i64 - (fx + 50) as i64).abs() <= 2 && (x1 as i64 - (fx + 59) as i64).abs() <= 2, "x {x0}..{x1} attendu {}..{}", fx + 50, fx + 59);
    assert!((y0 as i64 - (fy + 30) as i64).abs() <= 2 && (y1 as i64 - (fy + 39) as i64).abs() <= 2, "y {y0}..{y1} attendu {}..{}", fy + 30, fy + 39);
    // Le trait vectoriel de la même case a son calque à part.
    assert!(layer_cel(&doc, created.first, "Crayonné").expect("encre").vector.is_some());
    // Agrandie : le même carré double (cadre de 1280 × 720).
    let mut big = Document::with_frame(1280, 720, 24);
    let q = planche_of_three(&big);
    let c = create_plans(&mut big, &q, &[2], 0, &PlanOptions::default()).expect("plan");
    let cel = layer_cel(&big, c.first, "Crayonné (pinceau)").expect("calque bitmap");
    let f = big.frame_rect();
    let (x0, y0, x1, y1) = bounds_of(&cel.raster).expect("des pixels");
    let (fx, fy) = (f.left() as usize, f.top() as usize);
    assert!((x0 as i64 - (fx + 200) as i64).abs() <= 3 && (x1 as i64 - (fx + 239) as i64).abs() <= 3, "x {x0}..{x1}");
    assert!((y0 as i64 - (fy + 120) as i64).abs() <= 3 && (y1 as i64 - (fy + 159) as i64).abs() <= 3, "y {y0}..{y1}");
    // Rien ne sort du cadre.
    assert!(x0 >= fx && y0 >= fy && x1 < fx + 1280 && y1 < fy + 720);
}

#[test]
fn a_blank_selection_or_an_unknown_box_makes_nothing() {
    let mut doc = doc_small();
    let p = planche_of_three(&doc);
    assert!(create_plans(&mut doc, &p, &[], 0, &PlanOptions::default()).is_none());
    assert!(create_plans(&mut doc, &p, &[99], 0, &PlanOptions::default()).is_none());
    assert_eq!(doc.panels.len(), 1);
}

// ------------------------------------------------------------------------------------ dans l'application

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-crayonnes-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let app = App::sans_fenetre(&ctx, None);
    (ctx, app)
}

impl App {
    /// Pour les essais : ce que fait la première image de la planche (planche prête, bornes).
    pub(crate) fn crayonnes_prepare_for_tests(&mut self) {
        let frame = self.doc.frame;
        self.doc.crayonnes.ensure(frame);
        self.crayonnes.focus = true;
    }
}

#[test]
fn creating_plans_in_the_application_is_one_undo_step_and_the_boxes_stay_checked() {
    let (_ctx, mut app) = headless("un-pas");
    app.welcome = false;
    app.crayonnes_prepare_for_tests();
    let frame = app.doc.frame;
    let planche = planche_of_three(&Document::with_frame(frame[0], frame[1], 24));
    let ids: Vec<u64> = planche.cases.iter().take(3).map(|c| c.id).collect();
    app.doc.crayonnes = planche;
    app.doc.crayonnes.ensure(frame);
    app.run_action("storyboard.crayonnes");
    assert!(app.crayonnes_shown());
    let before_plans = app.doc.panels.len();
    // Rien de choisi : rien ne se fait.
    assert!(!app.crayonnes_create_plans(PlanOptions::default()));
    assert_eq!(app.doc.panels.len(), before_plans);
    app.crayonnes.selected = ids.clone();
    let history_before = app.history.names().0.len();
    assert!(app.crayonnes_create_plans(PlanOptions { placement: Placement::AfterCurrent, seconds: 2.0 }));
    assert_eq!(app.doc.panels.len(), before_plans + 3);
    assert_eq!(app.history.names().0.len(), history_before + 1, "une seule étape d'annulation du projet");
    assert!(app.doc.crayonnes.cases.iter().take(3).all(|c| c.used), "les cases sont cochées");
    assert_eq!(app.doc.crayonnes.cases.iter().take(3).map(|c| c.id).collect::<Vec<_>>(), ids, "les cases restent sur la page, dans le même ordre");
    assert!(app.doc.crayonnes.cases[0].has_ink(), "et gardent leur dessin");
    assert_eq!(app.cur_panel, 1, "on arrive sur le premier plan fait");
    // Le projet est annulé d'un coup (Édition > Annuler, ou Ctrl+Z dans le montage).
    app.undo();
    assert_eq!(app.doc.panels.len(), before_plans, "un seul Ctrl+Z défait les trois plans");
    app.redo();
    assert_eq!(app.doc.panels.len(), before_plans + 3);
    // Ctrl+Z dans la planche : les plans partent, et les coches reviennent.
    app.crayonnes.focus = true;
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.len(), before_plans, "annuler dans la planche défait la création des plans");
    assert!(app.doc.crayonnes.cases.iter().take(3).all(|c| !c.used), "les coches sont revenues");
    app.run_action("edition.retablir");
    assert_eq!(app.doc.panels.len(), before_plans + 3);
    assert!(app.doc.crayonnes.cases.iter().take(3).all(|c| c.used));
}

#[test]
fn a_page_undo_does_not_undo_the_project_when_it_changed_since() {
    let (_ctx, mut app) = headless("pas-double");
    app.welcome = false;
    let frame = app.doc.frame;
    app.doc.crayonnes = planche_of_three(&Document::with_frame(frame[0], frame[1], 24));
    app.doc.crayonnes.ensure(frame);
    app.run_action("storyboard.crayonnes");
    let ids: Vec<u64> = app.doc.crayonnes.cases.iter().take(2).map(|c| c.id).collect();
    app.crayonnes.selected = ids;
    let n = app.doc.panels.len();
    assert!(app.crayonnes_create_plans(PlanOptions::default()));
    // Le projet défait déjà la création (depuis le montage) : la planche ne défait pas autre chose.
    app.undo();
    app.new_panel();
    let n_after = app.doc.panels.len();
    assert_eq!(n_after, n + 1);
    app.crayonnes.focus = true;
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.len(), n_after, "la planche ne défait que ses coches : le nouveau plan reste");
    assert!(app.doc.crayonnes.cases.iter().take(2).all(|c| !c.used));
}

#[test]
fn the_commands_need_the_planche_to_be_open_and_are_known_to_the_palette() {
    let (_ctx, mut app) = headless("commandes");
    app.welcome = false;
    app.run_action("crayonnes.creer_plans");
    assert!(app.status.contains("Ouvre d'abord"), "{}", app.status);
    assert!(!app.crayonnes_shown());
    let commands = crate::palette::palette_commands();
    for id in ["storyboard.crayonnes", "crayonnes.case_suivante", "crayonnes.creer_plans", "crayonnes.ajouter_page", "crayonnes.supprimer_case"] {
        assert!(commands.iter().any(|(c, _)| *c == id), "{id} manque à la palette");
    }
    // Ouvrir depuis l'animation revient au storyboard.
    app.set_mode(Mode::Animation);
    app.run_action("storyboard.crayonnes");
    assert_eq!(app.mode, Mode::Storyboard);
    assert!(app.crayonnes_shown());
    // Les commandes de la barre.
    app.crayonnes_prepare_for_tests();
    app.run_action("crayonnes.ajouter_page");
    assert_eq!((app.doc.crayonnes.len(), app.crayonnes.page, app.crayonnes.current), (24, 1, 12));
    app.run_action("crayonnes.page_precedente");
    assert_eq!(app.crayonnes.page, 0);
    app.run_action("crayonnes.inserer_case");
    assert_eq!(app.doc.crayonnes.len(), 25);
    app.run_action("crayonnes.supprimer_case");
    assert_eq!(app.doc.crayonnes.len(), 24);
    // Annuler dans la planche défait la suppression, l'insertion, puis l'ajout de la page.
    app.crayonnes.focus = true;
    for expected in [25, 24, 12] {
        app.run_action("edition.annuler");
        assert_eq!(app.doc.crayonnes.len(), expected);
    }
    app.run_action("storyboard.crayonnes");
    assert!(!app.crayonnes_shown());
}

// ------------------------------------------------------------------------------------ le fichier

fn saved(name: &str, doc: &Document) -> (std::path::PathBuf, std::path::PathBuf) {
    let travail = dossier(name);
    let manifeste = travail.join("Film").join("Film.pelure");
    project::save(doc, &manifeste, &mut Default::default()).expect("enregistrement");
    (travail, manifeste)
}

fn drawn_doc() -> Document {
    let mut doc = doc_small();
    doc.crayonnes = planche_of_three(&doc);
    doc.crayonnes.cases[1].used = true;
    doc.crayonnes.set_grid(3, 2);
    stroke(&mut doc.crayonnes, 0, Outil::Crayon, &[pos2(100.0, 200.0), pos2(250.0, 220.0), pos2(400.0, 150.0)]).expect("trait");
    doc
}

#[test]
fn the_planche_round_trips_through_the_project_file() {
    let doc = drawn_doc();
    let (_t, manifeste) = saved("crayonnes-aller-retour", &doc);
    let dir = racine(&manifeste).to_path_buf();
    let tree = arbre(&dir);
    assert!(tree.contains_key(crayonnes::FILE), "{:?}", tree.keys().collect::<Vec<_>>());
    assert!(!tree.keys().any(|k| k.ends_with(".tmp") || k.ends_with(".illisible")));
    let l = project::load(&manifeste).expect("relecture");
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    let (a, b) = (&doc.crayonnes, &l.doc.crayonnes);
    assert_eq!((a.size, a.grid, a.len()), (b.size, b.grid, b.len()));
    for (x, y) in a.cases.iter().zip(&b.cases) {
        assert_eq!((x.id, &x.caption, x.used), (y.id, &y.caption, y.used));
        assert_eq!(x.ink.vector, y.ink.vector, "mêmes traits");
        assert_eq!(x.paint.raster.to_rgba(), y.paint.raster.to_rgba(), "même peinture");
    }
    // L'image de l'encre se refait à la première vue, identique à celle d'avant.
    let mut again = l.doc.crayonnes.clone();
    again.cases[0].render_if_stale();
    assert_eq!(again.cases[0].ink.raster.to_rgba(), a.cases[0].ink.raster.to_rgba());
    // Réenregistrer sans rien changer ne touche à rien (octet pour octet).
    let mut l = l;
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    assert_eq!(arbre(&dir), tree);
}

#[test]
fn an_unused_planche_writes_no_file_and_a_cleared_one_removes_it() {
    let (_t, manifeste) = saved("crayonnes-vide", &doc_small());
    assert!(!arbre(racine(&manifeste)).contains_key(crayonnes::FILE), "pas de planche, pas de fichier");
    // Une planche ouverte mais vierge (la page vide) non plus.
    let mut doc = doc_small();
    doc.crayonnes.ensure(doc.frame);
    let (_t, manifeste) = saved("crayonnes-page-vierge", &doc);
    assert!(!arbre(racine(&manifeste)).contains_key(crayonnes::FILE));
    // Tout effacer retire le fichier lisible.
    let (_t, manifeste) = saved("crayonnes-effacee", &drawn_doc());
    assert!(arbre(racine(&manifeste)).contains_key(crayonnes::FILE));
    let mut l = project::load(&manifeste).expect("lecture");
    for i in 0..l.doc.crayonnes.len() {
        l.doc.crayonnes.clear_case(i);
        l.doc.crayonnes.cases[i].caption.clear();
        l.doc.crayonnes.cases[i].used = false;
    }
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let tree = arbre(racine(&manifeste));
    assert!(!tree.contains_key(crayonnes::FILE) && !tree.keys().any(|k| k.ends_with(".illisible")), "{:?}", tree.keys().collect::<Vec<_>>());
}

#[test]
fn a_sidecar_from_a_newer_version_or_damaged_is_reported_and_never_destroyed() {
    for newer in [true, false] {
        let (_t, manifeste) = saved(if newer { "crayonnes-recente" } else { "crayonnes-abimee" }, &drawn_doc());
        let dir = racine(&manifeste).to_path_buf();
        let path = dir.join(crayonnes::FILE);
        if newer {
            let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).expect("lecture")).expect("JSON");
            v["version"] = serde_json::Value::from(2);
            v["champ_inconnu_d_une_version_future"] = serde_json::Value::from("à garder");
            std::fs::write(&path, serde_json::to_vec_pretty(&v).expect("JSON")).expect("écriture");
        } else {
            let bytes = std::fs::read(&path).expect("lecture");
            std::fs::write(&path, &bytes[..bytes.len() / 2]).expect("écriture");
        }
        let avant = arbre(&dir);
        let mut l = project::load(&manifeste).expect("le projet s'ouvre sans sa planche");
        assert_eq!(l.warnings.len(), 1, "{:#?}", l.warnings);
        assert!(l.warnings[0].contains(crayonnes::FILE) && (!newer || l.warnings[0].contains("plus récente")), "{:?}", l.warnings);
        assert!(l.doc.crayonnes.is_blank(), "rien n'est lu d'un fichier que cette version ne sait pas lire");
        assert_eq!(arbre(&dir), avant, "ouvrir ne touche à aucun fichier");
        // Enregistrer par-dessus : le fichier reste tel quel, octet pour octet.
        project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
        assert_eq!(arbre(&dir).get(crayonnes::FILE), avant.get(crayonnes::FILE), "le fichier illisible a été effacé ou réécrit");
        // Par le fil d'écriture de l'application aussi (prepare_save + SaveJob).
        l.written.clear();
        let job = project::prepare_save(&l.doc, &manifeste, &mut l.written, None).expect("cliché");
        std::thread::spawn(move || job.run_with(None, 2)).join().expect("fil").expect("écriture");
        assert_eq!(arbre(&dir).get(crayonnes::FILE), avant.get(crayonnes::FILE));
        assert!(!arbre(&dir).keys().any(|k| k.ends_with(".tmp") || k.ends_with(".illisible")));
        // Une planche neuve : son fichier est écrit, l'ancien est mis de côté.
        l.doc.crayonnes.ensure(l.doc.frame);
        stroke(&mut l.doc.crayonnes, 0, Outil::Crayon, &[pos2(30.0, 30.0), pos2(200.0, 100.0)]).expect("trait");
        project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
        let relu = project::load(&manifeste).expect("relecture");
        assert!(relu.warnings.is_empty(), "le fichier neuf se relit : {:?}", relu.warnings);
        assert_eq!(elements(&relu.doc.crayonnes, 0), 1);
        let garde = dir.join(format!("{}.illisible", crayonnes::FILE));
        assert!(garde.exists(), "l'ancien fichier est gardé à côté");
        assert_eq!(empreinte(&std::fs::read(&garde).expect("lecture")), *avant.get(crayonnes::FILE).expect("fichier d'origine"));
    }
}

#[test]
fn the_background_save_job_writes_the_planche_atomically() {
    let doc = drawn_doc();
    let travail = dossier("crayonnes-fil");
    let manifeste = travail.join("Film").join("Film.pelure");
    let mut written = Default::default();
    let job = project::prepare_save(&doc, &manifeste, &mut written, None).expect("cliché");
    // La planche change pendant l'écriture : le fichier est celui du cliché.
    let mut doc = doc;
    doc.crayonnes.cases[0].caption = "changée après le cliché".into();
    std::thread::spawn(move || job.run_with(None, 2)).join().expect("fil").expect("écriture");
    let l = project::load(&manifeste).expect("relecture");
    assert_eq!(l.doc.crayonnes.cases[0].caption.trim(), "Plan large");
    assert!(!arbre(racine(&manifeste)).keys().any(|k| k.ends_with(".tmp")));
}

#[test]
fn reading_the_file_is_defensive() {
    let doc = drawn_doc();
    let text = String::from_utf8(crayonnes::bytes_of(&doc.crayonnes).expect("octets").expect("non vide")).expect("UTF-8");
    let ok = parse(&text).expect("lisible");
    assert_eq!(ok.len(), 12);
    let edit = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        f(&mut v);
        parse(&serde_json::to_string(&v).expect("JSON"))
    };
    assert!(edit(&|v| v["format"] = "autre".into()).is_err());
    assert!(edit(&|v| v["version"] = 9.into()).err().unwrap_or_default().contains("plus récente"));
    assert!(edit(&|v| v["taille"] = serde_json::json!([0, 10])).is_err());
    assert!(edit(&|v| v["taille"] = serde_json::json!([100000, 10])).is_err());
    assert!(edit(&|v| v["grille"] = serde_json::json!([0, 3])).is_err());
    assert!(edit(&|v| v["grille"] = serde_json::json!([9, 3])).is_err());
    assert!(parse("pas du JSON").is_err());
    assert!(parse("").is_err());
    // Des identifiants en double sont renumérotés, des légendes trop longues coupées.
    let p = edit(&|v| {
        v["cases"][1]["id"] = v["cases"][0]["id"].clone();
        v["cases"][2]["id"] = 0.into();
        v["cases"][0]["legende"] = "x".repeat(5000).into();
    })
    .expect("lisible");
    let ids: std::collections::HashSet<u64> = p.cases.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), p.len());
    assert!(!ids.contains(&0));
    assert_eq!(p.cases[0].caption.chars().count(), crayonnes::CAPTION_MAX);
    // Des traits absurdes sont écartés (coordonnée infinie, rayon démesuré), sans perdre les autres.
    let p = edit(&|v| {
        v["cases"][0]["traits"].as_array_mut().expect("traits").push(serde_json::json!({"type": "Stroke", "points": [{"x": 3.0e30, "y": 1.0, "r": 2.0}], "color": [0, 0, 0], "opacity": 1.0, "hardness": 1.0}));
        v["cases"][0]["traits"].as_array_mut().expect("traits").push(serde_json::json!({"type": "Stroke", "points": [{"x": 1.0, "y": 1.0, "r": 1.0e9}], "color": [0, 0, 0], "opacity": 1.0, "hardness": 1.0}));
        v["cases"][0]["traits"].as_array_mut().expect("traits").push(serde_json::json!({"type": "Text", "text": "non", "police": "Arial", "taille": 12.0, "color": [0, 0, 0], "opacity": 1.0, "placement": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]}));
    })
    .expect("lisible");
    assert_eq!(p.cases[0].ink.vector.as_ref().expect("encre").elements.len(), 2, "les deux traits de la case, sans les trois absurdes");
    // Une peinture qui n'est pas un PNG, ou d'une autre taille, est ignorée.
    let p = edit(&|v| v["cases"][2]["peinture"] = "AAAA".into()).expect("lisible");
    assert!(!p.cases[2].has_paint());
    // Trop de cases.
    let many = edit(&|v| {
        let one = v["cases"][5].clone();
        v["cases"] = serde_json::Value::Array(vec![one; 3000]);
    });
    assert!(many.is_err());
}

#[test]
fn base64_round_trips_and_refuses_what_is_not_base64() {
    for n in 0..40usize {
        let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
        let text = b64_encode(&bytes);
        assert_eq!(b64_decode(&text).expect("décodé"), bytes, "{n} octets");
        assert!(text.len() % 4 == 0);
    }
    assert_eq!(b64_encode(b"Man"), "TWFu");
    assert_eq!(b64_encode(b"Ma"), "TWE=");
    assert_eq!(b64_encode(b"M"), "TQ==");
    assert!(b64_decode("TWF").is_none() && b64_decode("T@Fu").is_none() && b64_decode("=WFu").is_none());
}

// ------------------------------------------------------------------------------------ à l'écran

fn page_open() -> Banc {
    let doc = doc_small();
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.run_action("storyboard.crayonnes");
    assert_eq!(b.app.crayonnes_tool(), Outil::Crayon, "ouvrir la planche prend le crayon");
    b.chauffer(4);
    b
}

fn press(pos: Pos2, down: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE }
}

/// Un glisser de la souris sur la page : un point par image d'écran. Rend le temps de chaque image
/// (ms) pendant le tracé.
fn drag(b: &mut Banc, path: &[Pos2]) -> Vec<f64> {
    let mut times = Vec::new();
    b.image(vec![Event::PointerMoved(path[0])], 1.0 / 60.0);
    times.push(b.image(vec![press(path[0], true)], 1.0 / 60.0).construction);
    for q in &path[1..] {
        times.push(b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0).construction);
    }
    times.push(b.image(vec![press(*path.last().expect("point"), false)], 1.0 / 60.0).construction);
    b.image(Vec::new(), 1.0 / 60.0);
    times
}

fn arc(c: Pos2, rx: f32, ry: f32, n: usize) -> Vec<Pos2> {
    (0..=n).map(|k| c + vec2((k as f32 / n as f32 * 6.0).cos() * rx, (k as f32 / n as f32 * 6.0).sin() * ry)).collect()
}

#[test]
fn drawing_on_the_page_lands_in_the_box_where_the_stroke_starts() {
    let mut b = page_open();
    assert_eq!(b.app.crayonnes.cells.len(), 12, "douze cases sur la page");
    let cells: Vec<Cell> = b.app.crayonnes.cells.clone();
    // Un rond dans la case 4, qui sort sur la case 5 (la souris suit, la case ne change pas).
    let start = cells[4].frame.center();
    let path: Vec<Pos2> = (0..30).map(|k| start + vec2(k as f32 * 6.0, (k as f32 * 0.4).sin() * 25.0)).collect();
    assert!(path.iter().any(|q| cells[5].frame.contains(*q)), "le trait traverse la case voisine");
    drag(&mut b, &path);
    let p = &b.app.doc.crayonnes;
    assert_eq!(elements(p, 4), 1);
    for i in (0..p.len()).filter(|&i| i != 4) {
        assert!(!p.cases[i].has_drawing(), "case {i}");
    }
    assert_eq!(b.app.crayonnes.current, 4, "la case touchée devient la case courante");
    assert!(b.app.modified);
    assert!(b.app.crayonnes.history.can_undo());
    assert!(!b.app.rejeu.has_pending(), "aucun rejeu du moteur : le trait lisse est exact tout de suite");
    // Annuler dans la planche.
    b.app.run_action("edition.annuler");
    assert_eq!(elements(&b.app.doc.crayonnes, 4), 0);
    b.image(Vec::new(), 1.0 / 60.0);
}

#[test]
fn the_preview_of_a_stroke_is_in_the_box_image_the_frame_it_arrives() {
    let mut b = page_open();
    let cell = b.app.crayonnes.cells[7];
    let c = cell.frame.center();
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(vec![press(c, true)], 1.0 / 60.0);
    for k in 1..=12 {
        let q = c + vec2(k as f32 * 7.0, (k as f32 * 0.5).sin() * 18.0);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 120.0);
        // Le point reçu à cette image est déjà dans l'image de la case (sans latence), le trait n'étant
        // pas fini.
        assert!(b.app.crayonnes.live.is_some());
        let case = &b.app.doc.crayonnes.cases[7];
        assert!(has_pixels(&case.ink.raster), "image {k} : l'aperçu est déjà posé");
        assert!(case.dirty.is_clean_for_tests(), "image {k} : les tuiles ont été envoyées à l'écran dans la même image");
    }
    b.image(vec![press(c, false)], 1.0 / 60.0);
    assert!(b.app.crayonnes.live.is_none());
    assert_eq!(elements(&b.app.doc.crayonnes, 7), 1);
}

impl crate::crayonnes::Dirty {
    fn is_clean_for_tests(&self) -> bool {
        !self.all && self.ink.is_empty() && self.paint.is_empty()
    }
}

#[test]
fn a_page_of_twenty_four_boxes_of_quick_strokes_stays_light_while_drawing() {
    let mut b = page_open();
    // Une grille de 6 × 4, deux pages de 24 : on remplit la première de croquis rapides.
    b.app.crayonnes_set_grid(6, 4);
    b.app.crayonnes_add_page();
    b.app.crayonnes_goto(0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.crayonnes.cells.len(), 24);
    let cells: Vec<Cell> = b.app.crayonnes.cells.clone();
    let mut all = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        for k in 0..3 {
            let c = cell.frame.center() + vec2((k as f32 - 1.0) * 6.0, (i % 3) as f32 * 3.0);
            all.extend(drag(&mut b, &arc(c, 12.0 + k as f32, 7.0, 10)));
        }
    }
    assert!(b.app.doc.crayonnes.cases.iter().take(24).all(|c| elements_of(c) == 3), "trois traits par case");
    // Maintenant un long trait dans la dernière case, sur une page pleine de croquis.
    let cell = cells[23];
    let path = arc(cell.frame.center(), cell.frame.width() * 0.4, cell.frame.height() * 0.35, 80);
    let times = drag(&mut b, &path);
    let moves = &times[2..times.len() - 1];
    let (mean, worst) = (moves.iter().sum::<f64>() / moves.len() as f64, moves.iter().cloned().fold(0.0, f64::max));
    println!("CRAYONNÉS : page de 24 cases pleines, trait de {} points : image moyenne {mean:.2} ms, pire {worst:.2} ms", moves.len());
    let ink_ms = all.iter().sum::<f64>() / all.len() as f64;
    println!("CRAYONNÉS : 72 traits rapides, image moyenne {ink_ms:.2} ms");
    // (Large : pas de carte graphique ici, et la machine peut être chargée. Le chiffre est affiché.)
    assert!(mean < 250.0 && worst < 2000.0, "image moyenne {mean:.1} ms, pire {worst:.1} ms");
    assert!(!b.app.rejeu.has_pending());
}

fn elements_of(c: &crate::crayonnes::Case) -> usize {
    c.ink.vector.as_ref().map_or(0, |v| v.elements.len())
}

#[test]
fn shift_or_ctrl_with_a_press_chooses_instead_of_drawing_and_the_number_is_the_handle() {
    let mut b = page_open();
    let cells: Vec<Cell> = b.app.crayonnes.cells.clone();
    let ids: Vec<u64> = b.app.doc.crayonnes.cases.iter().map(|c| c.id).collect();
    let ctrl_press = |pos: Pos2, down: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::COMMAND };
    let shift_press = |pos: Pos2, down: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::SHIFT };
    let c1 = cells[1].frame.center();
    b.image(vec![Event::PointerMoved(c1)], 1.0 / 60.0);
    b.image(vec![ctrl_press(c1, true)], 1.0 / 60.0);
    b.image(vec![ctrl_press(c1, false)], 1.0 / 60.0);
    assert_eq!(b.app.crayonnes.selected, vec![ids[1]]);
    let c4 = cells[4].frame.center();
    b.image(vec![Event::PointerMoved(c4)], 1.0 / 60.0);
    b.image(vec![shift_press(c4, true)], 1.0 / 60.0);
    b.image(vec![shift_press(c4, false)], 1.0 / 60.0);
    assert_eq!(b.app.crayonnes.selected, ids[1..=4].to_vec(), "Maj : de la dernière case cliquée à celle-ci");
    assert!(b.app.doc.crayonnes.is_blank(), "choisir ne dessine pas");
    // Un clic sur le numéro choisit une seule case.
    let badge = cells[6].badge.center();
    b.image(vec![Event::PointerMoved(badge)], 1.0 / 60.0);
    b.image(vec![press(badge, true)], 1.0 / 60.0);
    b.image(vec![press(badge, false)], 1.0 / 60.0);
    assert_eq!(b.app.crayonnes.selected, vec![ids[6]]);
    assert!(b.app.doc.crayonnes.is_blank(), "le numéro n'est pas une zone de dessin");
    // Glisser le numéro de la case 6 sur la case 2 : l'ordre change.
    let target = cells[2].frame.center();
    b.image(vec![Event::PointerMoved(badge)], 1.0 / 60.0);
    b.image(vec![press(badge, true)], 1.0 / 60.0);
    for k in 1..=6 {
        let q = badge + (target - badge) * (k as f32 / 6.0);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(vec![press(target, false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let after: Vec<u64> = b.app.doc.crayonnes.cases.iter().map(|c| c.id).collect();
    assert_eq!(after[2], ids[6], "la case 7 est passée en 3e place");
    assert_eq!(&after[3..7], &ids[2..6], "les autres glissent d'un cran");
    // Et l'ordre s'annule.
    b.app.crayonnes.focus = true;
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.crayonnes.cases.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
}

#[test]
fn the_next_box_opens_by_itself_after_a_pause_only_when_asked() {
    let mut b = page_open();
    let cells: Vec<Cell> = b.app.crayonnes.cells.clone();
    // Éteint : la case courante ne bouge pas.
    drag(&mut b, &arc(cells[0].frame.center(), 20.0, 12.0, 20));
    for _ in 0..5 {
        b.image(Vec::new(), 0.5);
    }
    assert_eq!(b.app.crayonnes.current, 0);
    // Allumé : après la pause, la case suivante.
    b.app.run_action("crayonnes.case_auto");
    assert!(b.app.crayonnes.auto_next);
    drag(&mut b, &arc(cells[0].frame.center(), 20.0, 12.0, 20));
    assert_eq!(b.app.crayonnes.current, 0, "pas encore");
    b.image(Vec::new(), 0.5);
    assert_eq!(b.app.crayonnes.current, 0, "la pause n'est pas finie");
    b.image(Vec::new(), 1.0);
    assert_eq!(b.app.crayonnes.current, 1, "après la pause, la case suivante");
    // Reprendre le crayon avant la fin de la pause annule l'attente.
    drag(&mut b, &arc(cells[1].frame.center(), 20.0, 12.0, 20));
    b.image(Vec::new(), 0.4);
    drag(&mut b, &arc(cells[1].frame.center(), 12.0, 8.0, 20));
    b.image(Vec::new(), 0.5);
    assert_eq!(b.app.crayonnes.current, 1);
}

#[test]
fn captions_are_edited_under_the_boxes_and_become_the_plan_action() {
    let mut b = page_open();
    let cells: Vec<Cell> = b.app.crayonnes.cells.clone();
    // Cliquer dans la légende de la case 2, taper.
    let at = cells[2].caption.center();
    b.image(vec![Event::PointerMoved(at)], 1.0 / 60.0);
    b.image(vec![press(at, true)], 1.0 / 60.0);
    b.image(vec![press(at, false)], 1.0 / 60.0);
    b.image(vec![Event::Text("Entrée du bar".into())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.crayonnes.cases[2].caption, "Entrée du bar");
    assert!(b.app.modified);
    // La légende fait un plan.
    let id = b.app.doc.crayonnes.cases[2].id;
    // Quitter le champ d'abord (un appui entre deux cases).
    let gap = pos2((cells[0].frame.right() + cells[1].frame.left()) * 0.5, cells[0].frame.center().y);
    b.image(vec![Event::PointerMoved(gap)], 1.0 / 60.0);
    b.image(vec![press(gap, true)], 1.0 / 60.0);
    b.image(vec![press(gap, false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.app.crayonnes.selected = vec![id];
    assert!(b.app.crayonnes_create_plans(PlanOptions::default()));
    assert_eq!(b.app.doc.panels[b.app.cur_panel].action, "Entrée du bar");
    // Le champ de légende s'annule avec la planche.
    b.app.crayonnes.focus = true;
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.crayonnes.cases[2].caption, "", "la saisie de la légende était une étape d'annulation");
}

#[test]
fn the_tools_of_the_application_choose_the_tool_of_the_page() {
    let (_ctx, mut app) = headless("outils");
    app.welcome = false;
    for (tool, expected) in [(Tool::Pencil, Outil::Crayon), (Tool::Brush, Outil::Pinceau), (Tool::Eraser, Outil::Gomme), (Tool::Fill, Outil::Crayon)] {
        app.tool = tool;
        assert_eq!(app.crayonnes_tool(), expected, "{tool:?}");
    }
}

#[test]
fn a_new_project_starts_a_new_planche_state() {
    let mut b = page_open();
    let cell = b.app.crayonnes.cells[0];
    drag(&mut b, &arc(cell.frame.center(), 20.0, 10.0, 20));
    assert!(b.app.crayonnes.history.can_undo());
    b.app.doc_epoch += 1;
    b.app.doc = doc_small();
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.app.crayonnes.history.can_undo(), "l'annulation de l'autre projet ne sert pas dans celui-ci");
    assert!(b.app.crayonnes.open, "la planche reste ouverte");
    assert!(b.app.doc.crayonnes.is_blank());
}

#[test]
fn the_planche_view_replaces_the_drawing_only_in_storyboard() {
    let mut b = page_open();
    assert!(b.app.crayonnes_shown());
    b.app.set_mode(Mode::Animation);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.app.crayonnes_shown(), "en animation, le dessin est là");
    b.app.set_mode(Mode::Storyboard);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.crayonnes_shown());
    assert_eq!(b.app.pane_title(crate::dock::Pane::Canvas), "Crayonnés");
}

/// Des images de la planche (page remplie de croquis, cases choisies et cochées, grille de 2 × 2) dans
/// le dossier `SHOT_CAPTURE_DIR`, pour en vérifier la mise en page à l'œil. Sans carte graphique : rien.
#[test]
#[ignore]
fn crayonnes_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut b = crate::perf_tests::banc(Document::with_frame(1920, 1080, 24), true);
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("CRAYONNÉS pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("CRAYONNÉS {nom}");
    };
    b.app.run_action("storyboard.crayonnes");
    b.chauffer(6);
    let cells: Vec<Cell> = b.app.crayonnes.cells.clone();
    println!("CRAYONNÉS cases à 1920 × 1080 : {:?}", cells.iter().take(3).map(|c| c.frame).collect::<Vec<_>>());
    for (i, cell) in cells.iter().enumerate().take(9) {
        let c = cell.frame.center();
        let (rx, ry) = (cell.frame.width() * 0.3, cell.frame.height() * 0.28);
        drag(&mut b, &arc(c, rx, ry, 40));
        drag(&mut b, &[c - vec2(rx, 0.0), c + vec2(rx * 0.8, ry * 0.2 * (i as f32 - 4.0) / 4.0)]);
    }
    for (i, text) in ["Plan large, la rue", "Il entre", "Gros plan sur la main", "", "Contrechamp"].iter().enumerate() {
        b.app.doc.crayonnes.cases[i].caption = text.to_string();
    }
    let ids: Vec<u64> = b.app.doc.crayonnes.cases.iter().map(|c| c.id).collect();
    b.app.crayonnes.selected = vec![ids[1], ids[2]];
    b.app.doc.crayonnes.cases[0].used = true;
    b.chauffer(3);
    sauver(&b, "crayonnes-1-page.png");
    b.app.crayonnes_set_grid(2, 2);
    b.chauffer(3);
    sauver(&b, "crayonnes-2-grille-2x2.png");
    b.app.crayonnes_set_grid(6, 4);
    b.app.crayonnes.dialog = true;
    b.chauffer(3);
    sauver(&b, "crayonnes-3-grille-6x4-fenetre.png");
}
