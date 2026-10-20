//! Toile infinie : on dessine n'importe où autour de la feuille (coordonnées négatives comprises) ; les
//! tuiles hors de la feuille sont gardées à part (`Raster::outer`), allouées là où l'on dessine
//! seulement, annulables, enregistrées, composées, exportées.

use eframe::egui::{self, pos2, Color32};

use crate::app::{App, Tool};
use crate::camera::Camera;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::raster::{is_outer_id, outer_id, outer_key, Raster, OUTER, TILE, WORLD};
use crate::render;

const RED: Color32 = Color32::from_rgb(255, 0, 0);

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-toile-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

/// Un plan 400 × 300 à un calque bitmap, pinceau sur le calque.
fn app_bitmap(name: &str) -> App {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.tool = Tool::Brush;
    app.color = [255, 0, 0];
    app
}

/// Un trait de pinceau du point `a` au point `b` (coordonnées du monde).
fn stroke(app: &mut App, a: (f32, f32), b: (f32, f32)) {
    app.begin_paint(pos2(a.0, a.1), 1.0, false);
    let n = 30;
    for k in 1..=n {
        let t = k as f32 / n as f32;
        app.stroke_to(pos2(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), 1.0);
    }
    app.end_stroke();
}

#[test]
fn tile_numbers_keep_the_window_ranks_and_code_the_signed_outer_tiles() {
    let r = Raster::new(400, 300);
    assert_eq!((r.tiles_x, r.tiles_y()), (7, 5));
    // Dans la fenêtre : le rang d'avant.
    assert_eq!(r.id_of(0, 0), 0);
    assert_eq!(r.id_of(6, 4), 4 * 7 + 6);
    assert_eq!(r.key_of(4 * 7 + 6), (6, 4));
    // Dehors : un codage qui ressort, négatif compris, et qui n'est jamais un rang de la fenêtre.
    for (tx, ty) in [(-1, 0), (0, -1), (7, 0), (0, 5), (-512, 512), (511, -511), (-3, -9)] {
        let id = r.id_of(tx, ty);
        assert!(is_outer_id(id) && id >= OUTER, "{tx},{ty}");
        assert_eq!(outer_key(id), (tx, ty));
        assert_eq!(r.key_of(id), (tx, ty));
        assert_eq!(id, outer_id(tx, ty));
    }
    assert!(!is_outer_id(r.tiles.len() - 1));
    assert_eq!(r.tile_origin(r.id_of(-2, 3)), (-2 * TILE as i64, 3 * TILE as i64));
}

#[test]
fn a_pixel_far_from_the_sheet_is_kept_in_its_own_tile_and_nothing_else_is_allocated() {
    let mut r = Raster::new(400, 300);
    assert!(r.is_blank());
    let id = r.id_of(-9, -7);
    r.tile_mut(id)[3 * TILE + 5] = RED;
    let (x, y) = (-9 * TILE as i64 + 5, -7 * TILE as i64 + 3);
    assert_eq!(r.get_i(x, y), RED);
    assert_eq!(r.get_i(x + 1, y), Color32::TRANSPARENT);
    assert_eq!(r.get(0, 0), Color32::TRANSPARENT, "la fenêtre n'a pas bougé");
    assert!(r.tiles.iter().all(Option::is_none), "pas une tuile de la feuille");
    assert_eq!(r.outer.len(), 1, "une seule tuile de plus");
    assert!(!r.is_blank());
    assert_eq!(r.drawn_tile_range(), Some((-9, -7, -9, -7)));
    // Remplacer une tuile rend l'ancienne ; vider la retire.
    let old = r.replace_slot(id, None);
    assert!(old.is_some() && r.outer.is_empty() && r.is_blank());
    assert!(r.replace_slot(id, old).is_none());
    assert_eq!(r.get_i(x, y), RED);
    // Hors du monde : transparent, sans paniquer.
    assert_eq!(r.get_i(i64::MAX / 4, 0), Color32::TRANSPARENT);
    assert_eq!(r.get_i(-WORLD * 4, 5), Color32::TRANSPARENT);
}

#[test]
fn a_stroke_outside_the_sheet_is_painted_undone_and_redone() {
    let mut app = app_bitmap("trait-dehors");
    let steps = app.history.names().0.len();
    // Entièrement au-dessus et à gauche de la feuille.
    stroke(&mut app, (-500.0, -300.0), (-420.0, -300.0));
    assert_eq!(app.history.names().0.len(), steps + 1, "un trait, une étape");
    let cel = app.layers()[0].frames[0].expect("dessin créé");
    let r = &app.doc.cels[&cel].raster;
    assert!(r.get_i(-460, -300).a() > 200, "le trait est là, en coordonnées négatives");
    assert_eq!(r.get_i(-460, -250).a(), 0);
    assert!(r.tiles.iter().all(Option::is_none), "rien dans la feuille");
    assert!(!r.outer.is_empty() && r.outer.len() <= 4, "quelques tuiles seulement : {}", r.outer.len());
    // Un trait qui traverse le bord de la feuille : une part dans la feuille, une part dehors.
    stroke(&mut app, (-60.0, 150.0), (60.0, 150.0));
    let r = &app.doc.cels[&cel].raster;
    assert!(r.get_i(-30, 150).a() > 200 && r.get_i(30, 150).a() > 200);
    assert!(r.get(30, 150).a() > 200, "la part dans la feuille est lue par le code d'avant");
    assert!(r.tiles.iter().any(Option::is_some));
    // Annuler le second, puis le premier : les tuiles hors de la feuille disparaissent.
    app.undo();
    let r = &app.doc.cels[&cel].raster;
    assert_eq!(r.get_i(-30, 150).a(), 0);
    assert!(r.get_i(-460, -300).a() > 200);
    app.undo();
    assert!(app.doc.cels[&cel].raster.outer.is_empty(), "la mémoire revient avec l'annulation");
    app.redo();
    app.redo();
    let r = &app.doc.cels[&cel].raster;
    assert!(r.get_i(-460, -300).a() > 200 && r.get_i(-30, 150).a() > 200);
}

#[test]
fn memory_grows_only_where_one_draws() {
    let mut app = app_bitmap("memoire");
    // Trois petits points, très loin les uns des autres : des tuiles, pas une surface.
    for (x, y) in [(-20000.0, -15000.0), (25000.0, 9000.0), (-3000.0, 30000.0)] {
        stroke(&mut app, (x, y), (x + 20.0, y));
    }
    let cel = app.layers()[0].frames[0].expect("dessin");
    let r = &app.doc.cels[&cel].raster;
    assert!(r.outer.len() <= 9, "{} tuiles pour trois petits traits", r.outer.len());
    assert!(r.tiles.iter().all(Option::is_none));
    // La tuile de la feuille n'a pas bougé de taille : l'index reste celui de la feuille.
    assert_eq!(r.tiles.len(), 7 * 5);
    // Plus loin que le monde : pas de plantage, rien n'est posé.
    stroke(&mut app, (1.0e9, 1.0e9), (1.0e9 + 20.0, 1.0e9));
    let r = &app.doc.cels[&cel].raster;
    assert!(r.outer.len() <= 9);
}

#[test]
fn the_camera_frame_can_look_at_what_is_drawn_outside_the_sheet() {
    let mut app = app_bitmap("camera-dehors");
    stroke(&mut app, (-300.0, 140.0), (-200.0, 140.0));
    let doc = &app.doc;
    let plan = render::panel_plan_at(doc, 0, 0, Some([255, 255, 255]));
    // Le monde, pixel par pixel.
    assert!(render::composite_pixel_i(doc, &plan, -250, 140).a() > 0);
    assert_eq!(render::composite_pixel_i(doc, &plan, -250, 140).r(), 255);
    assert_eq!(render::composite_pixel_i(doc, &plan, -250, 200), Color32::WHITE, "le papier partout, dehors aussi");
    // La caméra, au centre de la feuille : rien de changé (blanc, sans le trait).
    let centered = render::camera_image(doc, &plan, &doc.default_camera());
    assert!(centered.iter().all(|c| *c == Color32::WHITE));
    // La caméra déplacée sur le trait : il est dans l'image.
    let c = doc.default_camera();
    let cam = Camera { x: -250.0, y: 150.0, ..c };
    let out = render::camera_image(doc, &plan, &cam);
    let [fw, fh] = doc.frame;
    let (u, v) = (fw / 2, fh / 2 - 10);
    assert!(out[v * fw + u].g() < 100 && out[v * fw + u].r() > 200, "le trait rouge au centre de l'image : {:?}", out[v * fw + u]);
    assert_eq!(out[10 * fw + 10], Color32::WHITE);
    // Sans papier : transparent partout où il n'y a rien.
    let plan = render::panel_plan_at(doc, 0, 0, None);
    let out = render::camera_image(doc, &plan, &cam);
    assert_eq!(out[10 * fw + 10], Color32::TRANSPARENT);
    assert!(out[v * fw + u].a() > 200);
    let _ = fh;
}

#[test]
fn a_cleared_or_erased_outside_is_really_empty() {
    let mut app = app_bitmap("effacer");
    stroke(&mut app, (-400.0, 50.0), (-300.0, 50.0));
    let cel = app.layers()[0].frames[0].expect("dessin");
    assert!(!app.doc.cels[&cel].is_empty());
    // La gomme efface aussi hors de la feuille.
    app.tool = Tool::Eraser;
    stroke(&mut app, (-420.0, 50.0), (-280.0, 50.0));
    let r = &app.doc.cels[&cel].raster;
    assert_eq!(r.get_i(-350, 50).a(), 0, "le trait est gommé");
    // Un dessin dont il ne reste que du vide hors de la feuille n'est pas « plein ».
    app.undo();
    assert!(r_has(&app, cel));
    app.clear_image();
    assert!(app.doc.cels[&cel].is_empty(), "vider le dessin vide aussi le hors-feuille");
}

fn r_has(app: &App, cel: u64) -> bool {
    !app.doc.cels[&cel].is_empty()
}

#[test]
fn old_sheets_keep_their_pixels_and_their_tiles() {
    // Un dessin fait de la feuille seule se comporte comme avant : rien de hors-feuille.
    let mut doc = Document::new(256, 128, 24);
    let mut r = Raster::new(256, 128);
    for y in 10..40 {
        for x in 10..40 {
            let id = r.tile_index_of(x, y);
            r.tile_mut(id)[(y % TILE) * TILE + x % TILE] = RED;
        }
    }
    let cel = doc.add_cel(Cel::bitmap(r));
    doc.panels[0].layers[0] = Layer::new("A", LayerKind::Bitmap);
    doc.panels[0].layers[0].frames = vec![Some(cel); 2];
    let plan = render::panel_plan_at(&doc, 0, 0, Some([255, 255, 255]));
    let flat = render::composite_full(&doc, &plan);
    let area = render::composite_area(&doc, &plan, render::Area::sheet(&doc));
    assert_eq!(flat, area, "la composition du monde donne la feuille d'avant, pixel pour pixel");
    assert!(doc.cels[&cel].raster.outer.is_empty());
}

/// Un projet enregistré dans un dossier neuf, puis relu.
fn round_trip(doc: &Document, name: &str) -> (crate::project::Loaded, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("shot-toile-save-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(doc, &manifest, &mut Default::default()).unwrap();
    (crate::project::load(&manifest).unwrap(), dir)
}

#[test]
fn what_is_drawn_outside_the_sheet_is_saved_and_read_back() {
    let mut app = app_bitmap("enregistrer");
    // Loin à gauche et en haut, loin à droite et en bas, et une part sur le bord bas de la feuille
    // (400 × 300 : la dernière rangée de tuiles déborde de la feuille de 20 pixels).
    stroke(&mut app, (-500.0, -300.0), (-420.0, -300.0));
    stroke(&mut app, (900.0, 700.0), (980.0, 720.0));
    stroke(&mut app, (100.0, 305.0), (180.0, 305.0));
    stroke(&mut app, (100.0, 150.0), (180.0, 150.0));
    let cel = app.layers()[0].frames[0].expect("dessin");
    let before = app.doc.cels[&cel].raster.clone();
    assert!(before.has_beyond() && before.outer.len() >= 3);
    let (back, dir) = round_trip(&app.doc, "dehors");
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let cel = back.doc.panels[0].layers[0].frames[0].expect("dessin relu");
    let after = &back.doc.cels[&cel].raster;
    for (x, y) in [(-460, -300), (940, 710), (140, 305), (140, 150), (50, 50)] {
        assert_eq!(after.get_i(x, y), before.get_i(x, y), "pixel {x},{y}");
    }
    assert!(after.get_i(-460, -300).a() > 200 && after.get_i(940, 710).a() > 200 && after.get_i(140, 305).a() > 200);
    // Tout ce qui est hors de la feuille est identique, tuile pour tuile.
    let a = before.beyond_sheet_tiles();
    let b = after.beyond_sheet_tiles();
    assert_eq!(a.len(), b.len());
    for ((ka, ta), (kb, tb)) in a.iter().zip(&b) {
        assert_eq!(ka, kb);
        assert_eq!(crate::raster::unpremultiply(ta[0]), crate::raster::unpremultiply(tb[0]));
        assert!(ta.iter().zip(tb.iter()).all(|(p, q)| crate::raster::unpremultiply(*p) == crate::raster::unpremultiply(*q)), "tuile {ka:?}");
    }
    // Les fichiers sont là, et le manifeste les nomme.
    let text = std::fs::read_to_string(dir.join("T.pelure")).unwrap();
    assert!(text.contains("dessins_hors"));
    let files: Vec<String> = std::fs::read_dir(dir.join("dessins")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(files.iter().any(|f| f.ends_with("_hors.png")) && files.iter().any(|f| f.ends_with("_hors.json")), "{files:?}");
    // Rien de plus à l'enregistrement suivant : un projet relu se réenregistre sans rien réécrire.
    let manifest = dir.join("T.pelure");
    let mut written = back.written.clone();
    let job = crate::project::prepare_save(&back.doc, &manifest, &mut written, None).unwrap();
    assert_eq!(job.run().unwrap().written, 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn erasing_the_outside_removes_its_files_and_a_sheet_only_drawing_writes_none() {
    let mut app = app_bitmap("effacer-fichiers");
    stroke(&mut app, (-500.0, -300.0), (-420.0, -300.0));
    let dir = std::env::temp_dir().join(format!("shot-toile-effacer-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    let mut written = Default::default();
    crate::project::save(&app.doc, &manifest, &mut written).unwrap();
    let has_hors = |dir: &std::path::Path| std::fs::read_dir(dir.join("dessins")).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains("_hors"));
    assert!(has_hors(&dir));
    // Le dessin n'a plus rien hors de la feuille (clear_image + un trait dans la feuille).
    app.clear_image();
    stroke(&mut app, (100.0, 100.0), (150.0, 100.0));
    crate::project::save(&app.doc, &manifest, &mut written).unwrap();
    assert!(!has_hors(&dir), "les fichiers de l'extérieur sont retirés");
    assert!(!std::fs::read_to_string(&manifest).unwrap().contains("dessins_hors"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_damaged_outside_file_costs_only_the_outside() {
    let mut app = app_bitmap("abime");
    stroke(&mut app, (-500.0, -300.0), (-420.0, -300.0));
    stroke(&mut app, (100.0, 100.0), (150.0, 100.0));
    let dir = std::env::temp_dir().join(format!("shot-toile-abime-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    for entry in std::fs::read_dir(dir.join("dessins")).unwrap().flatten() {
        if entry.file_name().to_string_lossy().ends_with("_hors.json") {
            std::fs::write(entry.path(), b"{ pas du json").unwrap();
        }
    }
    let back = crate::project::load(&manifest).unwrap();
    assert!(back.warnings.iter().any(|w| w.contains("hors de la feuille")), "{:?}", back.warnings);
    let cel = back.doc.panels[0].layers[0].frames[0].expect("dessin");
    assert!(back.doc.cels[&cel].raster.get(120, 100).a() > 200, "la feuille est là");
    assert!(back.doc.cels[&cel].raster.outer.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

fn line(points: &[(f32, f32)], r: f32, color: [u8; 3]) -> crate::vector::Element {
    crate::vector::Element::Stroke {
        points: points.iter().map(|&(x, y)| crate::vector::VPoint { x, y, r }).collect(),
        color,
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

fn vector_cel(elements: Vec<crate::vector::Element>) -> Cel {
    let mut c = Cel::new(400, 300, LayerKind::Vector);
    c.vector.as_mut().unwrap().elements = elements;
    c.rerender();
    c
}

#[test]
fn vector_strokes_and_fills_are_rendered_outside_the_sheet() {
    use crate::vector::Element;
    let fill = Element::Fill { contours: vec![vec![[-300.0, 700.0], [-200.0, 700.0], [-200.0, 800.0], [-300.0, 800.0]]], color: [0, 0, 255], opacity: 1.0 };
    let c = vector_cel(vec![
        line(&[(-500.0, -300.0), (-420.0, -300.0)], 4.0, [255, 0, 0]),
        // À cheval sur le bord droit de la feuille (400).
        line(&[(350.0, 100.0), (480.0, 100.0)], 4.0, [0, 120, 0]),
        // Très loin.
        line(&[(12000.0, 9000.0), (12050.0, 9000.0)], 3.0, [200, 0, 200]),
        fill,
    ]);
    let r = &c.raster;
    assert_eq!(r.get_i(-460, -300), Color32::from_rgb(255, 0, 0), "tout dehors, négatif");
    assert!(r.get_i(370, 100).a() > 200 && r.get_i(450, 100).a() > 200, "dedans et dehors à la fois");
    assert_eq!(r.get(370, 100).a() > 200, true, "la part dans la feuille est lue comme avant");
    assert!(r.get_i(12020, 9000).a() > 200, "très loin");
    assert_eq!(r.get_i(-250, 750), Color32::from_rgb(0, 0, 255), "une zone remplie dehors");
    assert_eq!(r.get_i(-250, 600).a(), 0);
    assert!(r.outer.len() >= 4 && r.outer.len() < 20, "peu de tuiles : {}", r.outer.len());
    // Sur une feuille qui ne contient rien d'autre, le dessin reste vide de la feuille.
    let other = vector_cel(vec![line(&[(-100.0, -100.0), (-50.0, -100.0)], 3.0, [0, 0, 0])]);
    assert!(other.raster.tiles.iter().all(Option::is_none) && !other.is_empty());
}

#[test]
fn a_region_redone_outside_the_sheet_matches_a_full_render() {
    use crate::vector::Affine;
    let base = vector_cel(vec![
        line(&[(-300.0, -200.0), (-100.0, -150.0), (-250.0, -80.0)], 5.0, [255, 0, 0]),
        line(&[(-200.0, -250.0), (-150.0, -50.0)], 4.0, [0, 0, 0]),
        line(&[(500.0, 400.0), (700.0, 450.0)], 6.0, [0, 100, 0]),
    ]);
    let mut partial = base.clone();
    let d = partial.vector.as_mut().unwrap();
    let before = d.elements[0].bounds();
    d.elements[0] = d.elements[0].transformed(&Affine::translate(eframe::egui::vec2(60.0, 25.0)));
    let after = d.elements[0].bounds();
    partial.rerender_region(before.union(after).expand(3.0));
    let mut full = partial.clone();
    full.rerender();
    let (a, b) = (&partial.raster, &full.raster);
    let mut diff = 0;
    for y in -400..500 {
        for x in -500..800 {
            if a.get_i(x, y) != b.get_i(x, y) {
                diff += 1;
            }
        }
    }
    assert_eq!(diff, 0, "refaire la zone donne le rendu complet, hors de la feuille aussi");
    // Les mêmes tuiles dessinées.
    assert_eq!(a.drawn_ids().len(), b.drawn_ids().len());
}

#[test]
fn a_vector_stroke_drawn_outside_the_sheet_is_kept_undone_and_saved() {
    let mut app = app(&"vecto-dehors");
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Vecto", LayerKind::Vector)];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.tool = Tool::Brush;
    app.color = [255, 0, 0];
    stroke(&mut app, (-500.0, -300.0), (-400.0, -300.0));
    let cel = app.layers()[0].frames[0].expect("dessin");
    // Le rendu exact d'un trait au réglage complet vient d'un fil de travail : on le laisse arriver.
    let ctx = egui::Context::default();
    for _ in 0..200 {
        app.poll_rejeu(&ctx);
        if !app.rejeu.has_pending() && app.rejeu_deferred.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let c = &app.doc.cels[&cel];
    assert_eq!(c.vector.as_ref().map(|v| v.elements.len()), Some(1), "le trait est un élément vectoriel");
    assert!(c.raster.get_i(-450, -300).a() > 150, "et son image est hors de la feuille");
    // Annuler le retire du dessin et de l'image.
    app.undo();
    let c = &app.doc.cels[&cel];
    assert!(c.vector.as_ref().is_none_or(|v| v.elements.is_empty()));
    assert!(c.raster.get_i(-450, -300).a() == 0);
    app.redo();
    // Enregistrer et relire : l'image hors de la feuille se refait des éléments.
    let (back, dir) = round_trip(&app.doc, "vecto-dehors");
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let cel = back.doc.panels[0].layers[0].frames[0].expect("dessin relu");
    let c = &back.doc.cels[&cel];
    assert_eq!(c.vector.as_ref().map(|v| v.elements.len()), Some(1));
    assert!(c.raster.get_i(-450, -300).a() > 150, "relu : le trait est toujours là, dehors");
    std::fs::remove_dir_all(&dir).ok();
}
