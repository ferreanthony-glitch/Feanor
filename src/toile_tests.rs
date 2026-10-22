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

// ---------------------------------------------------------------------------------------------
// L'écran : la texture du monde hors de la feuille (`monde.rs`).
// ---------------------------------------------------------------------------------------------

use crate::monde::Monde;
use crate::render::Area;

/// Fait avancer `monde` jusqu'à ce qu'il n'ait plus rien à composer (une image d'écran à la fois).
fn settle(monde: &mut Monde, ctx: &egui::Context, app: &mut App, plan: &render::Plan, view: Area) -> Option<crate::monde::Shown> {
    let (all, ids) = app.renderer.take_world_dirty();
    let mut shown = monde.update(ctx, &app.doc, plan, view, all, &ids);
    for _ in 0..2000 {
        if monde.pending() == 0 {
            break;
        }
        shown = monde.update(ctx, &app.doc, plan, view, false, &[]);
    }
    shown
}

#[test]
fn the_world_texture_completes_the_sheet_pixel_for_pixel() {
    let mut app = app_bitmap("monde-egal");
    stroke(&mut app, (-300.0, 140.0), (-200.0, 140.0));
    stroke(&mut app, (-60.0, 150.0), (60.0, 150.0));
    stroke(&mut app, (380.0, 280.0), (520.0, 330.0));
    stroke(&mut app, (1500.0, 1200.0), (1600.0, 1250.0));
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    let ctx = egui::Context::default();
    let mut monde = Monde::new();
    let view = Area { x0: -512, y0: -256, x1: 1792, y1: 1536 };
    let shown = settle(&mut monde, &ctx, &mut app, &plan, view).expect("quelque chose hors de la feuille");
    assert!(shown.area.x0 <= -300 && shown.area.y1 >= 1250 && shown.area.x1 >= 1600, "la zone couvre ce qui est dessiné dans la vue : {:?}", shown.area);
    let (area, k) = monde.coverage().expect("zone");
    assert_eq!(k, 1);
    let reference = render::composite_area(&app.doc, &plan, area);
    let mut drawn = 0;
    for y in area.y0..area.y1 {
        for x in area.x0..area.x1 {
            let m = monde.pixel(x, y).expect("dans la zone");
            let c = reference[((y - area.y0) as usize) * area.width() + (x - area.x0) as usize];
            if x >= 0 && y >= 0 && x < 400 && y < 300 {
                assert_eq!(m, Color32::TRANSPARENT, "la feuille a sa propre image ({x},{y})");
            } else if c != Color32::WHITE {
                assert_eq!(m, c, "({x},{y})");
                drawn += 1;
            } else {
                assert!(m == c || m.a() == 0, "({x},{y}) : {m:?}");
            }
        }
    }
    assert!(drawn > 400, "le dessin hors de la feuille est dans la texture ({drawn} pixels)");
    // Peu de tuiles composées : celles qui portent du dessin, pas toute la zone.
    assert!(monde.composed() < 80, "{} tuiles composées", monde.composed());
}

#[test]
fn nothing_outside_the_sheet_costs_nothing() {
    let mut app = app_bitmap("monde-rien");
    stroke(&mut app, (50.0, 50.0), (150.0, 90.0));
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    let ctx = egui::Context::default();
    let mut monde = Monde::new();
    assert!(settle(&mut monde, &ctx, &mut app, &plan, Area { x0: -2000, y0: -2000, x1: 4000, y1: 4000 }).is_none());
    assert_eq!(monde.bytes(), 0);
    assert_eq!(monde.composed(), 0);
    // Du dessin hors de la feuille, mais que la vue ne regarde pas : rien non plus.
    stroke(&mut app, (-900.0, 100.0), (-800.0, 100.0));
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    assert!(settle(&mut monde, &ctx, &mut app, &plan, Area { x0: 0, y0: 0, x1: 400, y1: 300 }).is_none());
    assert_eq!(monde.composed(), 0);
    // La vue y va : la texture apparaît.
    assert!(settle(&mut monde, &ctx, &mut app, &plan, Area { x0: -1000, y0: -100, x1: 200, y1: 400 }).is_some());
    assert!(monde.composed() > 0 && monde.bytes() > 0);
}

#[test]
fn a_new_stroke_far_from_the_sheet_recomposes_a_few_tiles_only() {
    let mut app = app_bitmap("monde-incremental");
    stroke(&mut app, (-2000.0, -1500.0), (-1900.0, -1500.0));
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    let ctx = egui::Context::default();
    let mut monde = Monde::new();
    let view = Area { x0: -2400, y0: -1800, x1: 600, y1: 600 };
    settle(&mut monde, &ctx, &mut app, &plan, view).expect("monde");
    let before = monde.composed();
    // Un second trait, plus loin dans la même vue.
    stroke(&mut app, (-1200.0, -1000.0), (-1100.0, -960.0));
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    let t = std::time::Instant::now();
    settle(&mut monde, &ctx, &mut app, &plan, view).expect("monde");
    let took = t.elapsed();
    let more = monde.composed() - before;
    assert!(more > 0 && more < 12, "{more} tuiles pour un petit trait");
    assert!(took.as_millis() < 100, "{took:?} pour un petit trait hors de la feuille");
    // Le résultat est celui d'un monde refait de zéro.
    let (area, _) = monde.coverage().expect("zone");
    let reference = render::composite_area(&app.doc, &plan, area);
    for y in (area.y0..area.y1).step_by(3) {
        for x in (area.x0..area.x1).step_by(3) {
            let c = reference[((y - area.y0) as usize) * area.width() + (x - area.x0) as usize];
            let m = monde.pixel(x, y).expect("pixel");
            if c != Color32::WHITE && !(x >= 0 && y >= 0 && x < 400 && y < 300) {
                assert_eq!(m, c, "({x},{y})");
            }
        }
    }
    // Annuler le trait : la texture le perd.
    app.undo();
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    settle(&mut monde, &ctx, &mut app, &plan, view).expect("monde");
    assert_eq!(monde.pixel(-1150, -980).map(|c| c.a()), Some(0), "le second trait est parti");
    assert!(monde.pixel(-1950, -1500).is_some_and(|c| c.a() > 200), "le premier reste");
}

#[test]
fn a_zoomed_out_view_gets_a_bounded_reduced_texture() {
    let mut app = app_bitmap("monde-reduit");
    stroke(&mut app, (-20000.0, -15000.0), (-19900.0, -15000.0));
    stroke(&mut app, (20000.0, 9000.0), (20100.0, 9000.0));
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    let ctx = egui::Context::default();
    let mut monde = Monde::new();
    let view = Area { x0: -30000, y0: -30000, x1: 30000, y1: 30000 };
    settle(&mut monde, &ctx, &mut app, &plan, view).expect("monde");
    let (area, k) = monde.coverage().expect("zone");
    assert!(k > 1, "réduit : {k}");
    assert!(area.width() / k <= crate::monde::MAX_SIDE as usize && area.height() / k <= crate::monde::MAX_SIDE as usize);
    assert!(monde.bytes() <= 2 * 4 * (crate::monde::MAX_SIDE * crate::monde::MAX_SIDE) as u64, "{} octets", monde.bytes());
    // Les deux traits se voient encore, en plus petit.
    assert!(monde.pixel(-19950, -15000).is_some_and(|c| c.a() > 0));
    assert!(monde.pixel(20050, 9000).is_some_and(|c| c.a() > 0));
}

#[test]
fn drawing_and_panning_far_from_the_origin_stays_fast() {
    let mut app = app_bitmap("monde-perf");
    let (ox, oy) = (12000.0_f32, -9000.0_f32);
    let ctx = egui::Context::default();
    let mut monde = Monde::new();
    let mut worst = std::time::Duration::ZERO;
    let mut view = Area { x0: ox as i64 - 800, y0: oy as i64 - 500, x1: ox as i64 + 800, y1: oy as i64 + 500 };
    // Une série de traits, chacun suivi d'une image d'écran.
    for k in 0..12 {
        let y = oy + k as f32 * 20.0;
        app.begin_paint(pos2(ox, y), 1.0, false);
        for s in 1..=20 {
            app.stroke_to(pos2(ox + s as f32 * 8.0, y + (s as f32 * 0.3).sin() * 6.0), 1.0);
            let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
            let (all, ids) = app.renderer.take_world_dirty();
            let t = std::time::Instant::now();
            monde.update(&ctx, &app.doc, &plan, view, all, &ids);
            worst = worst.max(t.elapsed());
        }
        app.end_stroke();
    }
    // Un défilement de la vue, d'une image à l'autre.
    let plan = render::panel_plan_at(&app.doc, 0, 0, Some([255, 255, 255]));
    for step in 0..40 {
        view.x0 += 25;
        view.x1 += 25;
        view.y0 += step % 3;
        view.y1 += step % 3;
        let (all, ids) = app.renderer.take_world_dirty();
        let t = std::time::Instant::now();
        monde.update(&ctx, &app.doc, &plan, view, all, &ids);
        worst = worst.max(t.elapsed());
    }
    println!("pire image hors de la feuille : {worst:?} ; {} tuiles composées", monde.composed());
    assert!(worst.as_millis() < 60, "pire image : {worst:?}");
}

#[test]
fn the_picker_reads_outside_the_sheet_and_the_fill_stays_on_it() {
    let mut app = app_bitmap("pipette-dehors");
    stroke(&mut app, (-300.0, 140.0), (-200.0, 140.0));
    app.color = [10, 20, 30];
    app.pick_color(pos2(-250.0, 140.0));
    assert!(app.color[0] > 200 && app.color[1] < 60, "la pipette prend le rouge du trait hors de la feuille : {:?}", app.color);
    // Le pot de peinture ne remplit pas hors de la feuille, et le dit.
    let cel = app.layers()[0].frames[0].expect("dessin");
    let tiles = app.doc.cels[&cel].raster.outer.len();
    app.status.clear();
    app.fill_at(pos2(-250.0, 300.0));
    assert_eq!(app.doc.cels[&cel].raster.outer.len(), tiles);
    assert!(app.status.contains("pot de peinture"), "{}", app.status);
}
/// Des images d'écran entières avec du dessin hors de la feuille : la vue le montre, quelle que soit
/// la rotation, le miroir, la caméra, et rien ne plante.
#[test]
fn the_canvas_shows_what_is_drawn_outside_the_sheet_in_every_view() {
    let mut app = app_bitmap("ecran-dehors");
    stroke(&mut app, (-300.0, 140.0), (-200.0, 140.0));
    stroke(&mut app, (900.0, 700.0), (1000.0, 720.0));
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |app: &mut App| {
        time += 1.0 / 60.0;
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(1280.0, 720.0))), time: Some(time), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
        let out = ctx.run(input, |ctx| app.frame(ctx));
        out.shapes.len()
    };
    frame(&mut app);
    app.fit_pending = false;
    app.view.zoom = 0.25;
    app.view.pan = egui::vec2(0.0, 0.0);
    for _ in 0..4 {
        frame(&mut app);
    }
    assert!(app.monde.coverage().is_some(), "la vue regarde le trait hors de la feuille");
    assert!(app.monde.pixel(-250, 140).is_some_and(|c| c.a() > 200), "le trait est dans la texture");
    // Autres vues : tournée, en miroir, loin, caméra, zoom extrême.
    for (rotation, flip, zoom, pan) in [(0.7, false, 0.25, egui::vec2(30.0, -20.0)), (0.0, true, 0.1, egui::vec2(-200.0, 100.0)), (2.5, true, 0.02, egui::vec2(0.0, 0.0)), (0.0, false, 8.0, egui::vec2(400.0, 300.0))] {
        app.view.rotation = rotation;
        app.view.flip = flip;
        app.view.zoom = zoom;
        app.view.pan = pan;
        for _ in 0..3 {
            frame(&mut app);
        }
    }
    app.camera_view = true;
    frame(&mut app);
    app.camera_view = false;
    // Le dessin hors de la feuille disparaît (annuler tout) : la texture est lâchée.
    app.undo();
    app.undo();
    app.view.rotation = 0.0;
    app.view.flip = false;
    app.view.zoom = 0.25;
    app.view.pan = egui::vec2(0.0, 0.0);
    for _ in 0..3 {
        frame(&mut app);
    }
    assert!(app.monde.coverage().is_none() && app.monde.bytes() == 0, "plus rien hors de la feuille, plus de mémoire");
}
/// Un glissé au pointeur dans une image d'écran entière : le trait tombe sous le pointeur, quels que soient
/// la rotation et le miroir de la vue (Anthony, 2026-10-22 : le miroir tournant montrait un voile de travers).
#[test]
fn a_stroke_drawn_in_a_mirrored_rotated_view_lands_under_the_pointer() {
    use eframe::egui::{Event, Modifiers, PointerButton, RawInput, Rect};
    let tau = std::f32::consts::PI;
    for (k, (rotation, flip)) in [(0.0, true), (tau / 4.0, true), (tau / 2.0, true), (tau * 0.75, true), (tau, true), (tau / 6.0, true), (tau / 4.0, false)].into_iter().enumerate() {
        let mut app = app_bitmap(&format!("pointeur-miroir-{k}"));
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut frame = |app: &mut App, events: Vec<Event>| {
            time += 1.0 / 60.0;
            let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(1280.0, 720.0))), time: Some(time), events, ..Default::default() };
            input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
            let _ = ctx.run(input, |ctx| app.frame(ctx));
        };
        frame(&mut app, Vec::new());
        app.fit_pending = false;
        app.view.zoom = 1.0;
        app.view.pan = egui::vec2(0.0, 0.0);
        app.view.rotation = rotation;
        app.view.flip = flip;
        for _ in 0..3 {
            frame(&mut app, Vec::new());
        }
        let (rect, size) = (app.canvas_rect, app.canvas_size());
        let (a, b) = (rect.center() + egui::vec2(-100.0, -50.0), rect.center() + egui::vec2(100.0, 50.0));
        frame(&mut app, vec![Event::PointerMoved(a)]);
        frame(&mut app, vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
        for i in 1..=20 {
            frame(&mut app, vec![Event::PointerMoved(a + (b - a) * (i as f32 / 20.0))]);
        }
        frame(&mut app, vec![Event::PointerButton { pos: b, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
        for _ in 0..3 {
            frame(&mut app, Vec::new());
        }
        let cel = app.cel_here().and_then(|c| app.doc.cels.get(&c)).expect("un dessin");
        // Le trait est là où l'écran l'a vu : en des points du segment, à un pixel ou deux près.
        for t in [0.15_f32, 0.5, 0.85] {
            let p = app.view.to_canvas(rect, size, a + (b - a) * t);
            let (x, y) = (p.x.round() as i64, p.y.round() as i64);
            let hit = (-3..=3).any(|dy| (-3..=3).any(|dx| cel.raster.get((x + dx) as usize, (y + dy) as usize).a() > 0));
            assert!(hit, "rotation {rotation}, miroir {flip} : pas de trait près de ({x}, {y}) pour t = {t}");
        }
    }
}