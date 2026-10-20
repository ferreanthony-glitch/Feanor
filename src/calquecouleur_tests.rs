//! Calque de couleur (`calquecouleur.rs`) : la couleur sous le trait, qui suit les images du trait.
//! Créer, lier, déplacer, supprimer, dupliquer, copier ; le miroir des clés ; le pot de peinture
//! borné par le trait ; le pinceau dans la zone ; l'annulation en une étape ; l'enregistrement.

use eframe::egui::{self, pos2, Color32};

use crate::app::{App, Tool};
use crate::calquecouleur::{self as cc, Cache, Cle, Murs};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::project;
use crate::raster::{Raster, TILE};
use crate::render;

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

/// Un anneau noir de 4 px d'épaisseur, de centre (`cx`, `cy`) et de rayon `rad`, sur une feuille
/// `w × h` ; `gap` : un trou de cette largeur à droite.
fn ring(w: usize, h: usize, (cx, cy): (f32, f32), rad: f32, gap: f32) -> Raster {
    let mut r = Raster::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            if (rad - 2.0..=rad + 2.0).contains(&d) && !(fx > cx + rad - 6.0 && (fy - cy).abs() < gap / 2.0) {
                put(&mut r, x, y, Color32::BLACK);
            }
        }
    }
    r
}

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-calquecouleur-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

/// Un plan de 12 images à deux calques : « Trait » (trois dessins d'anneau, un trou de deux images)
/// et « Autre » au-dessus (vide) ; le calque courant est le trait, l'outil le pot de peinture.
fn app_with_line(name: &str, gap: f32) -> (App, [CelId; 3]) {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    let a = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 60.0, gap)));
    let b = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 40.0, 0.0)));
    let c = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 80.0, 0.0)));
    let mut line = Layer::new("Trait", LayerKind::Bitmap);
    line.frames = [vec![Some(a); 4], vec![Some(b); 3], vec![None; 2], vec![Some(c); 3]].concat();
    doc.panels[0].layers = vec![line, Layer::new("Autre", LayerKind::Vector)];
    doc.panels[0].length = 12;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.tool = Tool::Fill;
    app.color = [255, 0, 0];
    (app, [a, b, c])
}

/// La forme d'une suite d'images : chaque dessin remplacé par son rang d'apparition.
fn shape(frames: &[Option<CelId>]) -> Vec<Option<usize>> {
    let mut seen: Vec<CelId> = Vec::new();
    frames
        .iter()
        .map(|f| {
            f.map(|id| match seen.iter().position(|&s| s == id) {
                Some(k) => k,
                None => {
                    seen.push(id);
                    seen.len() - 1
                }
            })
        })
        .collect()
}

fn undo_steps(app: &App) -> usize {
    app.history.names().0.len()
}

const RED: Color32 = Color32::from_rgb(255, 0, 0);

/// Le plan avec son calque de couleur sous le trait : [Couleur, Trait, Autre], le courant est la couleur.
fn with_colour(name: &str, gap: f32) -> (App, [CelId; 3]) {
    let (mut app, cels) = app_with_line(name, gap);
    app.run_action("calque.nouveau_couleur");
    (app, cels)
}

#[test]
fn a_new_colour_layer_sits_under_its_line_and_mirrors_its_keys() {
    let (mut app, [a, ..]) = app_with_line("creation", 0.0);
    let steps = undo_steps(&app);
    app.run_action("calque.nouveau_couleur");
    assert_eq!(app.layers().len(), 3);
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Trait couleur", "Trait", "Autre"], "juste sous le trait");
    assert_eq!(app.cur_layer, 0, "la couleur devient le calque courant");
    assert_eq!(undo_steps(&app), steps + 1, "une seule étape d'annulation");
    let (colour, line) = (&app.layers()[0], &app.layers()[1]);
    assert_eq!(colour.kind, LayerKind::Bitmap);
    assert_eq!(colour.couleur.as_ref().map(|c| c.trait_id), Some(line.id));
    assert_eq!(shape(&colour.frames), shape(&line.frames), "mêmes clés, mêmes expositions, même trou");
    assert!(colour.frames.iter().flatten().all(|c| app.doc.cels.contains_key(c) && !line.frames.contains(&Some(*c))), "des dessins neufs, vides");
    assert!(colour.frames.iter().flatten().all(|c| app.doc.cels[c].is_empty()));
    assert_eq!(colour.couleur.as_ref().map(|c| c.dessins.len()), Some(3));
    assert!(colour.couleur.as_ref().is_some_and(|c| c.dessins.contains_key(&a)));
    assert_eq!(colour.visible, line.visible);
    // Annuler retire le calque de couleur ; rétablir le rend avec ses dessins.
    app.undo();
    assert_eq!(app.layers().len(), 2);
    app.redo();
    assert_eq!(app.layers().len(), 3);
    assert_eq!(shape(&app.layers()[0].frames), shape(&app.layers()[1].frames));
}

#[test]
fn only_an_ordinary_drawing_layer_can_carry_a_colour_layer() {
    let (mut app, _) = app_with_line("refus", 0.0);
    app.doc.panels[0].layers.push(Layer::new_folder("Dossier"));
    app.cur_layer = 2;
    app.run_action("calque.nouveau_couleur");
    assert_eq!(app.layers().len(), 3, "pas sous un dossier");
    assert!(app.status.contains("calque de dessin"), "{}", app.status);
    // Sous un calque de couleur : un autre calque de couleur du même trait.
    app.cur_layer = 0;
    app.run_action("calque.nouveau_couleur");
    app.run_action("calque.nouveau_couleur");
    let n = app.layers().iter().filter(|l| l.couleur.is_some()).count();
    assert_eq!(n, 2, "deux calques de couleur pour le même trait");
    assert!(app.layers().iter().filter_map(|l| l.couleur.as_ref()).all(|c| c.trait_id == app.layers()[2].id));
}

#[test]
fn the_colour_follows_the_line_keys_and_keeps_its_paint() {
    let (mut app, [a, b, c]) = with_colour("miroir", 0.0);
    let colour_a = app.layers()[0].frames[0].expect("dessin de couleur");
    let colour_b = app.layers()[0].frames[4].expect("dessin de couleur");
    // De la couleur sur la première clé.
    app.cur_frame = 0;
    app.fill_at(pos2(200.0, 150.0));
    assert_eq!(app.doc.cels[&colour_a].raster.get(200, 150), RED);
    // La clé du trait avance de deux images (le trou et les autres suivent) : sa couleur la suit.
    app.layers_mut()[1].frames = [vec![None; 2], vec![Some(a); 4], vec![Some(b); 3], vec![None], vec![Some(c); 2]].concat();
    app.sync_couleurs();
    let colour = &app.layers()[0];
    assert_eq!(shape(&colour.frames), shape(&app.layers()[1].frames));
    assert_eq!(colour.frames[2], Some(colour_a), "le même dessin de couleur, deux images plus loin");
    assert_eq!(colour.frames[6], Some(colour_b));
    assert_eq!(colour.frames[0], None);
    assert_eq!(app.doc.cels[&colour_a].raster.get(200, 150), RED, "la couleur est restée");
    // Une nouvelle clé du trait (« Nouveau dessin ») reçoit une couleur vide, et le trait reste le calque courant.
    app.cur_layer = 1;
    app.cur_frame = 0;
    let before = app.layers()[0].frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len();
    app.new_image(false);
    assert_eq!(app.cur_layer, 1);
    let colour = &app.layers()[0];
    assert_eq!(shape(&colour.frames), shape(&app.layers()[1].frames));
    assert_eq!(colour.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len(), before + 1);
    // Supprimer une clé du trait : sa couleur part de la pile ; une seule étape d'annulation, qui rend tout.
    let steps = undo_steps(&app);
    let (layer_before, colour_before) = (app.layers()[1].frames.clone(), app.layers()[0].frames.clone());
    app.cur_frame = app.layers()[1].frames.iter().rposition(Option::is_some).expect("une clé");
    app.delete_image();
    assert_eq!(undo_steps(&app), steps + 1);
    assert_eq!(shape(&app.layers()[0].frames), shape(&app.layers()[1].frames));
    assert_ne!(app.layers()[0].frames, colour_before);
    app.undo();
    assert_eq!(app.layers()[1].frames, layer_before);
    assert_eq!(app.layers()[0].frames, colour_before, "la couleur revient avec le trait");
}

#[test]
fn pre_and_post_behaviour_and_transform_follow_the_line() {
    let (mut app, _) = with_colour("comportement", 0.0);
    app.layers_mut()[1].pre = crate::document::Behaviour::Loop;
    app.layers_mut()[1].post = crate::document::Behaviour::Hold;
    app.layers_mut()[1].transform.x = 12.0;
    app.layers_mut()[1].depth = 0.5;
    app.sync_couleurs();
    let colour = &app.layers()[0];
    assert_eq!((colour.pre, colour.post), (crate::document::Behaviour::Loop, crate::document::Behaviour::Hold));
    assert_eq!((colour.transform.x, colour.depth), (12.0, 0.5));
}

#[test]
fn the_keys_of_a_colour_layer_cannot_be_edited_on_their_own() {
    let (mut app, _) = with_colour("cles", 0.0);
    let frames = app.layers()[0].frames.clone();
    let steps = undo_steps(&app);
    app.new_image(false);
    assert!(app.status.contains("suivent"), "{}", app.status);
    app.exposure_plus();
    app.delete_image();
    assert_eq!(app.layers()[0].frames, frames);
    assert_eq!(undo_steps(&app), steps, "rien n'a été enregistré");
    assert!(app.layers()[0].keys_follow_line() && !app.layers()[1].keys_follow_line());
}

#[test]
fn the_fill_stays_inside_the_closed_zone_and_goes_under_the_line() {
    let (mut app, _) = with_colour("remplir", 0.0);
    let line_cel = app.layers()[1].frames[0].expect("trait");
    let line_hash = crate::fiabilite_fixture::empreinte(&app.doc.cels[&line_cel].raster.to_rgba());
    let steps = undo_steps(&app);
    app.fill_at(pos2(200.0, 150.0));
    assert_eq!(undo_steps(&app), steps + 1, "un clic, une étape");
    let cel = app.layers()[0].frames[0].expect("couleur");
    let r = &app.doc.cels[&cel].raster;
    assert_eq!(r.get(200, 150), RED, "le dedans");
    assert_eq!(r.get(150, 150), RED, "plus loin dedans");
    assert_eq!(r.get(145, 150), RED, "jusqu'au trait (anneau de 138 à 142 ; la couleur passe dessous)");
    assert_eq!(r.get(141, 150), RED, "et sous le trait");
    assert_eq!(r.get(136, 150).a(), 0, "mais pas au-delà du trait");
    assert_eq!(r.get(10, 10).a(), 0, "pas dehors");
    assert_eq!(r.get(300, 150).a(), 0, "pas dehors");
    // Le trait n'est pas touché.
    assert_eq!(crate::fiabilite_fixture::empreinte(&app.doc.cels[&line_cel].raster.to_rgba()), line_hash);
    // À l'image : la couleur dans l'anneau, le trait par-dessus, rien dehors.
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    assert_eq!(render::composite_pixel(&app.doc, &plan, 200, 150), RED);
    assert_eq!(render::composite_pixel(&app.doc, &plan, 200, 90), Color32::BLACK, "le trait est au-dessus de la couleur");
    assert_eq!(render::composite_pixel(&app.doc, &plan, 10, 10).a(), 0);
    // L'export (l'image de la caméra) montre la même chose.
    let cam = app.doc.default_camera();
    let out = render::camera_image(&app.doc, &plan, &cam);
    let [fw, _] = app.doc.frame;
    assert_eq!(out[150 * fw + 200], RED);
    assert_eq!(out[90 * fw + 200], Color32::BLACK);
    assert_eq!(out[10 * fw + 10].a(), 0);
    // Autre couleur, même zone : elle remplace.
    app.color = [0, 0, 255];
    app.fill_at(pos2(180.0, 160.0));
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), Color32::from_rgb(0, 0, 255));
    // Annuler rend la couleur d'avant.
    app.undo();
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), RED);
    app.undo();
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150).a(), 0);
}

#[test]
fn a_fill_on_an_empty_frame_asks_for_a_line() {
    let (mut app, _) = with_colour("sans-trait", 0.0);
    app.cur_frame = 4;
    app.layers_mut()[1].frames[4] = None;
    app.sync_couleurs();
    app.cur_frame = 8;
    app.fill_at(pos2(200.0, 150.0));
    assert!(app.status.contains("pas de dessin"), "{}", app.status);
    app.cur_frame = 0;
    // Un clic au cœur d'un aplat noir : pas de zone.
    let blob = app.layers()[1].frames[0].expect("trait");
    {
        let r = &mut app.doc.cels.get_mut(&blob).expect("dessin").raster;
        for y in 20..80 {
            for x in 20..80 {
                put(r, x, y, Color32::BLACK);
            }
        }
        app.doc.cels.get_mut(&blob).expect("dessin").version += 1;
    }
    app.fill_at(pos2(50.0, 50.0));
    assert!(app.status.contains("zone fermée"), "{}", app.status);
}

#[test]
fn holes_smaller_than_the_closing_are_closed_and_bigger_ones_leak() {
    // Un trou de 6 px : fermé par les 4 px de chaque côté (8 px au plus).
    let (mut app, _) = with_colour("trou-petit", 6.0);
    app.fill_at(pos2(200.0, 150.0));
    let cel = app.layers()[0].frames[0].expect("couleur");
    assert_eq!(app.doc.cels[&cel].raster.get(300, 150).a(), 0, "rien ne sort par le petit trou");
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), RED);
    // Un trou de 30 px : la couleur fuit et colorie tout le dehors, le message le dit.
    let (mut app, _) = with_colour("trou-grand", 30.0);
    app.fill_at(pos2(200.0, 150.0));
    let cel = app.layers()[0].frames[0].expect("couleur");
    assert_eq!(app.doc.cels[&cel].raster.get(300, 150), RED, "elle a fui");
    assert_eq!(app.doc.cels[&cel].raster.get(10, 10), RED);
    assert!(app.status.contains("fuit"), "{}", app.status);
    // Plus de fermeture : un trou de 30 px se ferme avec 15 px de chaque côté.
    app.undo();
    if let Some(c) = app.layers_mut()[0].couleur.as_mut() {
        c.trous = 16;
    }
    app.fill_at(pos2(200.0, 150.0));
    assert_eq!(app.doc.cels[&cel].raster.get(300, 150).a(), 0);
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), RED);
}

#[test]
fn a_click_outside_the_shape_colours_the_whole_outside_but_not_the_inside() {
    let (mut app, _) = with_colour("dehors", 0.0);
    app.color = [0, 200, 0];
    app.fill_at(pos2(10.0, 10.0));
    let cel = app.layers()[0].frames[0].expect("couleur");
    let r = &app.doc.cels[&cel].raster;
    assert_eq!(r.get(10, 10), Color32::from_rgb(0, 200, 0));
    assert_eq!(r.get(390, 290), Color32::from_rgb(0, 200, 0));
    assert_eq!(r.get(200, 150).a(), 0, "le dedans de l'anneau reste libre");
}

#[test]
fn the_fill_works_with_a_vector_line() {
    use crate::vector::{Element, VPoint};
    let (mut app, _) = app_with_line("vecto", 0.0);
    // Le calque de trait devient vectoriel : un carré fermé.
    let cel = app.doc.new_cel(LayerKind::Vector);
    let pts = [(120.0, 80.0), (280.0, 80.0), (280.0, 220.0), (120.0, 220.0), (120.0, 80.0)];
    {
        let c = app.doc.cels.get_mut(&cel).unwrap();
        c.vector.as_mut().unwrap().elements = vec![Element::Stroke {
            points: pts.iter().map(|&(x, y)| VPoint { x, y, r: 2.5 }).collect(),
            color: [0, 0, 0],
            opacity: 1.0,
            hardness: 1.0,
            brush: None,
            trace: Default::default(),
        }];
        c.rerender();
    }
    app.layers_mut()[0].kind = LayerKind::Vector;
    app.layers_mut()[0].frames = vec![Some(cel); 12];
    app.run_action("calque.nouveau_couleur");
    app.fill_at(pos2(200.0, 150.0));
    let colour = app.layers()[0].frames[0].expect("couleur");
    let r = &app.doc.cels[&colour].raster;
    assert_eq!(app.doc.cels[&colour].vector.is_none(), true, "la couleur est en pixels");
    assert_eq!(r.get(200, 150), RED);
    assert_eq!(r.get(125, 150), RED);
    assert_eq!(r.get(100, 150).a(), 0);
    assert_eq!(r.get(200, 60).a(), 0);
}

#[test]
fn a_brush_stroke_stays_in_the_zone_where_it_starts() {
    let (mut app, _) = with_colour("pinceau", 0.0);
    app.tool = Tool::Brush;
    app.color = [0, 0, 255];
    let cel = app.layers()[0].frames[0].expect("couleur");
    app.begin_paint(pos2(200.0, 150.0), 1.0, false);
    for k in 0..=60 {
        app.stroke_to(pos2(200.0 + k as f32 * 3.0, 150.0), 1.0);
    }
    app.end_stroke();
    let r = &app.doc.cels[&cel].raster;
    assert!(r.get(220, 150).a() > 100, "dans l'anneau : peint");
    assert_eq!(r.get(300, 150).a(), 0, "hors de l'anneau : rien");
    assert_eq!(r.get(380, 150).a(), 0);
    // Zone éteinte : le trait peint partout, sous le trait comme un calque ordinaire.
    app.undo();
    if let Some(c) = app.layers_mut()[0].couleur.as_mut() {
        c.zone = false;
    }
    app.begin_paint(pos2(200.0, 150.0), 1.0, false);
    for k in 0..=60 {
        app.stroke_to(pos2(200.0 + k as f32 * 3.0, 150.0), 1.0);
    }
    app.end_stroke();
    assert!(app.doc.cels[&cel].raster.get(300, 150).a() > 100, "sans la zone, le trait sort");
    // La gomme n'est pas limitée à la zone.
    app.tool = Tool::Eraser;
    app.begin_paint(pos2(300.0, 150.0), 1.0, false);
    app.stroke_to(pos2(310.0, 150.0), 1.0);
    app.end_stroke();
    assert_eq!(app.doc.cels[&cel].raster.get(305, 150).a(), 0);
}

#[test]
fn a_stroke_on_an_image_without_a_line_makes_an_empty_line_drawing() {
    let (mut app, _) = with_colour("sans-trait-pinceau", 0.0);
    app.layers_mut()[1].frames[4] = None;
    app.layers_mut()[1].frames[5] = None;
    app.layers_mut()[1].frames[6] = None;
    app.sync_couleurs();
    assert_eq!(app.layers()[0].frames[5], None);
    app.tool = Tool::Brush;
    app.layers_mut()[0].couleur.as_mut().unwrap().zone = false;
    app.cur_frame = 5;
    let steps = undo_steps(&app);
    app.begin_paint(pos2(100.0, 100.0), 1.0, false);
    app.stroke_to(pos2(120.0, 100.0), 1.0);
    app.end_stroke();
    assert!(app.layers()[1].frames[5].is_some(), "un dessin de trait vide a été posé");
    let colour = app.layers()[0].frames[5].expect("couleur");
    assert!(app.doc.cels[&colour].raster.get(110, 100).a() > 0);
    assert_eq!(shape(&app.layers()[0].frames), shape(&app.layers()[1].frames));
    assert!(undo_steps(&app) > steps);
    app.undo();
    assert_eq!(app.layers()[1].frames[5], None, "tout s'annule");
}

#[test]
fn hiding_the_line_hides_its_colour_but_the_colour_can_be_hidden_alone() {
    let (mut app, _) = with_colour("visibilite", 0.0);
    app.toggle_layer_visible(1);
    assert!(!app.layers()[1].visible && !app.layers()[0].visible, "le trait emmène sa couleur");
    app.toggle_layer_visible(0);
    assert!(app.layers()[0].visible && !app.layers()[1].visible, "la couleur seule se montre avec son œil");
    app.toggle_layer_visible(0);
    assert!(!app.layers()[0].visible);
    app.toggle_layer_visible(1);
    assert!(app.layers()[1].visible && app.layers()[0].visible);
    // Le verrou est à chacun : on colorie sous un trait verrouillé.
    app.layers_mut()[1].locked = true;
    assert!(app.layers()[0].drawable() && !app.layers()[1].drawable());
    // Le calque courant seul, c'est le trait et sa couleur.
    app.cur_layer = 1;
    app.show_current_layer_only();
    assert!(app.layers()[0].visible && app.layers()[1].visible && !app.layers()[2].visible);
}

#[test]
fn deleting_the_line_deletes_its_colour_and_deleting_the_colour_keeps_the_line() {
    let (mut app, _) = with_colour("suppression", 0.0);
    let steps = undo_steps(&app);
    app.cur_layer = 1;
    app.delete_chosen_layers();
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Autre"], "le trait et sa couleur");
    assert_eq!(undo_steps(&app), steps + 1);
    app.undo();
    assert_eq!(app.layers().len(), 3);
    // La couleur seule.
    app.cur_layer = 0;
    app.layer_sel.clear();
    app.delete_chosen_layers();
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Trait", "Autre"]);
}

#[test]
fn duplicating_the_line_duplicates_its_colour_with_its_own_drawings() {
    let (mut app, _) = with_colour("duplication", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    let colour_cel = app.layers()[0].frames[0].expect("couleur");
    app.cur_layer = 1;
    let steps = undo_steps(&app);
    app.duplicate_chosen_layers();
    assert_eq!(undo_steps(&app), steps + 1);
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Trait couleur", "Trait", "Trait couleur copie", "Trait copie", "Autre"], "la copie de la couleur sous la copie du trait");
    let (copy_line, copy_colour) = (app.layers()[3].clone(), app.layers()[2].clone());
    assert_eq!(copy_colour.couleur.as_ref().map(|c| c.trait_id), Some(copy_line.id), "elle colore la copie, pas l'original");
    assert_eq!(app.layers()[0].couleur.as_ref().map(|c| c.trait_id), Some(app.layers()[1].id));
    assert_eq!(shape(&copy_colour.frames), shape(&copy_line.frames));
    let copied = copy_colour.frames[0].expect("copie");
    assert_ne!(copied, colour_cel, "ses propres dessins");
    assert_eq!(app.doc.cels[&copied].raster.get(200, 150), RED, "avec la couleur");
    // Modifier la copie ne touche pas l'original.
    put(&mut app.doc.cels.get_mut(&copied).unwrap().raster, 200, 150, Color32::BLUE);
    assert_eq!(app.doc.cels[&colour_cel].raster.get(200, 150), RED);
    // Les deux sont choisis : la couleur ne se copie pas deux fois.
    app.cur_layer = 1;
    app.layer_sel = vec![app.layers()[0].id];
    let n = app.layers().len();
    app.duplicate_chosen_layers();
    assert_eq!(app.layers().len(), n + 2, "le trait et sa couleur, une fois");
}

#[test]
fn copying_and_pasting_the_line_brings_its_colour() {
    let (mut app, _) = with_colour("collage", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    app.cur_layer = 1;
    app.copy_layer();
    app.cur_layer = 2;
    app.paste_layer();
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names.len(), 5, "{names:?}");
    let line = app.layers().iter().position(|l| l.id == app.layer().id).expect("courant");
    assert_eq!(app.layers()[line].name, "Trait");
    let colour = &app.layers()[line - 1];
    assert_eq!(colour.couleur.as_ref().map(|c| c.trait_id), Some(app.layers()[line].id), "sous la copie du trait");
    assert_eq!(shape(&colour.frames), shape(&app.layers()[line].frames));
    let cel = colour.frames[0].expect("couleur collée");
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), RED);
    assert_ne!(Some(cel), app.layers()[0].frames[0]);
}

#[test]
fn the_colour_layer_cannot_leave_its_line_and_follows_it_when_it_moves() {
    let (mut app, _) = with_colour("deplacement", 0.0);
    app.cur_layer = 1;
    app.move_layer(true);
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Autre", "Trait couleur", "Trait"], "le trait est monté, la couleur l'a suivi");
    assert_eq!(app.layer().name, "Trait", "le calque courant reste le même");
    // Déplacer la couleur seule est refusé.
    app.cur_layer = 1;
    app.move_layer(true);
    assert!(app.status.contains("reste sous"), "{}", app.status);
    assert_eq!(app.layers()[1].name, "Trait couleur");
    app.move_layer_to(1, 0);
    assert_eq!(app.layers()[1].name, "Trait couleur");
    // Déposer un autre calque entre le trait et sa couleur : la couleur reste collée au trait.
    app.move_layer_to(0, 1);
    let names: Vec<&str> = app.layers().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Autre", "Trait couleur", "Trait"], "{names:?}");
}

#[test]
fn a_colour_layer_in_a_folder_stays_with_its_line() {
    let (mut app, _) = app_with_line("dossier", 0.0);
    // Le trait est rangé dans un dossier.
    let folder = Layer::new_folder("Dossier");
    let fid = folder.id;
    app.layers_mut().push(folder);
    app.layers_mut()[0].parent = Some(fid);
    crate::folders::normalize(app.layers_mut());
    app.cur_layer = app.layers().iter().position(|l| l.name == "Trait").unwrap();
    app.run_action("calque.nouveau_couleur");
    let colour = app.layers().iter().find(|l| l.couleur.is_some()).expect("couleur");
    let line = app.layers().iter().find(|l| l.name == "Trait").expect("trait");
    assert_eq!(colour.parent, line.parent, "dans le même dossier");
    assert!(colour.parent.is_some());
}

#[test]
fn a_colour_layer_whose_line_is_gone_becomes_an_ordinary_layer() {
    let (mut app, _) = with_colour("orphelin", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    let id = app.layers()[0].id;
    let cel = app.layers()[0].frames[0].expect("couleur");
    app.layers_mut().remove(1);
    app.sync_couleurs();
    let l = app.layers().iter().find(|l| l.id == id).expect("la couleur reste");
    assert!(l.couleur.is_none(), "plus de lien");
    assert_eq!(l.frames[0], Some(cel), "ses dessins restent");
    // Détacher : ses images ne suivent plus rien.
    let (mut app, _) = with_colour("detacher", 0.0);
    app.run_action("calque.detacher_couleur");
    assert!(app.layer().couleur.is_none());
    let frames = app.layer().frames.clone();
    app.layers_mut()[1].frames[0] = None;
    app.sync_couleurs();
    assert_eq!(app.layer().frames, frames, "les images de la couleur détachée sont à elle");
}

#[test]
fn merging_a_line_with_its_colour_gives_one_ordinary_layer() {
    let (mut app, _) = with_colour("fusion", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    app.cur_layer = 1;
    app.merge_layer_down();
    assert_eq!(app.layers().len(), 2, "{:?}", app.status);
    assert!(app.layers().iter().all(|l| l.couleur.is_none()), "le lien n'a plus d'objet");
    let merged = &app.layers()[0];
    let cel = merged.frames[0].expect("fusion");
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), RED, "la couleur");
    assert_eq!(app.doc.cels[&cel].raster.get(200, 90), Color32::BLACK, "et le trait par-dessus");
}

#[test]
fn colour_layers_are_saved_and_read_back() {
    let (mut app, _) = with_colour("enregistrement", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    let dir = std::env::temp_dir().join(format!("shot-calquecouleur-save-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("C.pelure");
    project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let back = project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    assert!(text.contains("\"calque_couleur\"") && text.contains(&format!("\"version\": {}", project::VERSION)));
    assert!(project::VERSION >= 30, "30 : la version du calque de couleur");
    let layers = &back.doc.panels[0].layers;
    assert_eq!(layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Trait couleur", "Trait", "Autre"]);
    let (colour, line) = (&layers[0], &layers[1]);
    let link = colour.couleur.as_ref().expect("lien");
    assert_eq!(link.trait_id, line.id, "le lien retrouve son trait");
    assert_eq!((link.trous, link.zone), (cc::TROUS, true));
    assert_eq!(link.dessins.len(), 3);
    assert_eq!(shape(&colour.frames), shape(&line.frames));
    let cel = colour.frames[0].expect("couleur");
    assert_eq!(back.doc.cels[&cel].raster.get(200, 150), RED, "la couleur a été enregistrée");
    // Rien à refaire après la lecture : le miroir ne change plus rien.
    let mut doc = back.doc;
    assert!(!cc::refleter(&mut doc, 0));
    assert!(!cc::reparer(&mut doc.panels[0].layers));
}

#[test]
fn a_project_from_before_opens_unchanged() {
    // Un projet sans calque de couleur n'écrit rien de nouveau, et se relit aussi sous son ancien numéro.
    let mut doc = Document::new(64, 64, 24);
    let cel = doc.add_cel(Cel::bitmap(ring(64, 64, (32.0, 32.0), 20.0, 0.0)));
    doc.panels[0].layers[0].kind = LayerKind::Bitmap;
    doc.panels[0].layers[0].frames = vec![Some(cel); 3];
    let dir = std::env::temp_dir().join(format!("shot-calquecouleur-vieux-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("V.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(!text.contains("calque_couleur"));
    let old = text.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 27");
    std::fs::write(&manifest, old).unwrap();
    let back = project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(back.warnings.is_empty());
    assert!(back.doc.panels[0].layers.iter().all(|l| l.couleur.is_none()));
    assert_eq!(back.doc.panels[0].layers[0].frames, vec![Some(cel); 3]);
}

#[test]
fn a_damaged_link_is_dropped_without_losing_the_layer() {
    let (app, _) = with_colour("lien-abime", 0.0);
    let dir = std::env::temp_dir().join(format!("shot-calquecouleur-abime-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("A.pelure");
    project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    // Le rang du trait est dément : 9 calques n'existent pas ; et un lien du calque sur lui-même.
    let bad = text.replace("\"trait\": 1", "\"trait\": 9");
    assert_ne!(bad, text);
    std::fs::write(&manifest, bad).unwrap();
    let back = project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.doc.panels[0].layers.len(), 3);
    assert!(back.doc.panels[0].layers.iter().all(|l| l.couleur.is_none()));
}

#[test]
fn the_walls_are_computed_once_per_version_of_the_line() {
    let r = ring(400, 300, (200.0, 150.0), 60.0, 0.0);
    let mut cache = Cache::default();
    let cle = Cle { cel: 7, version: 1, trous: 4 };
    let a = cache.obtenir(cle, &r).expect("murs");
    let b = cache.obtenir(cle, &r).expect("murs");
    assert!(std::sync::Arc::ptr_eq(&a, &b), "le même calcul");
    let c = cache.obtenir(Cle { version: 2, ..cle }, &r).expect("murs");
    assert!(!std::sync::Arc::ptr_eq(&a, &c), "une nouvelle version du trait, de nouveaux murs");
    assert!(cache.obtenir(cle, &Raster::new(10, 10)).is_none(), "pas de trait, pas de murs");
    // Préchauffer : le calcul se fait sur un autre fil, et arrive.
    let mut cache = Cache::default();
    cache.prechauffer(cle, &r);
    assert!(cache.calcule() || cache.est_pret(cle));
    let start = std::time::Instant::now();
    while !cache.est_pret(cle) && start.elapsed().as_secs() < 20 {
        cache.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(cache.est_pret(cle), "les murs sont arrivés");
    assert!(cache.obtenir(cle, &r).is_some());
}

#[test]
fn a_zone_is_the_closed_inside_with_a_margin_under_the_line() {
    let r = ring(400, 300, (200.0, 150.0), 60.0, 0.0);
    let murs = Murs::new(&r, 4).expect("murs");
    let z = murs.zone(200, 150).expect("zone");
    assert!(!z.dehors);
    assert!(z.contient(200, 150) && z.contient(150, 150));
    assert!(z.contient(258, 150), "le bord intérieur du trait (anneau de 258 à 262)");
    assert!(z.contient(259, 150), "et dessous");
    assert!(!z.contient(263, 150) && !z.contient(10, 10));
    // Le dehors.
    let out = murs.zone(10, 10).expect("dehors");
    assert!(out.dehors && out.contient(10, 10) && out.contient(399, 299) && !out.contient(200, 150));
    // Pas de trait : pas de murs ; hors de la feuille : pas de zone.
    assert!(Murs::new(&Raster::new(50, 50), 4).is_none());
    assert!(murs.zone(400, 10).is_none());
    // Le pochoir de la zone.
    let p = z.pochoir(400, 300);
    assert_eq!(p.get(200, 150).a(), 255);
    assert_eq!(p.get(10, 10).a(), 0);
}

#[test]
fn filling_a_big_sheet_is_fast_enough_to_click() {
    // 3840 × 2160, un personnage de 1000 × 1500 px : la première zone (murs compris) et la suivante.
    let mut r = Raster::new(3840, 2160);
    for y in 300..1800 {
        for x in 1200..2200 {
            let on = (y - 300) % 250 < 4 || (x - 1200) % 250 < 4 || y == 1799 || x == 2199;
            if on {
                put(&mut r, x, y, Color32::BLACK);
            }
        }
    }
    let start = std::time::Instant::now();
    let murs = Murs::new(&r, 4).expect("murs");
    let walls = start.elapsed();
    let start = std::time::Instant::now();
    let zone = murs.zone(1300, 400).expect("zone");
    let flood = start.elapsed();
    let mut paint = Raster::new(3840, 2160);
    let start = std::time::Instant::now();
    let tiles = zone.peindre(&mut paint, Some(RED));
    let painted = start.elapsed();
    println!("murs {walls:?}, zone {flood:?}, peinture {painted:?}, {} tuiles", tiles.len());
    assert!(!zone.dehors && paint.get(1300, 400) == RED && paint.get(1500, 400).a() == 0);
    // Large marge : l'essai ne mesure que l'ordre de grandeur (machine chargée, version de débogage).
    assert!(walls.as_millis() < 5000 && flood.as_millis() < 2000 && painted.as_millis() < 2000);
}

#[test]
fn the_export_to_storyboard_pro_carries_the_colour_as_an_ordinary_layer_and_says_so() {
    let (mut app, _) = with_colour("sbp", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    let out = crate::sbexport::prepare(&app.doc, "Film");
    let layers = &out.scenes[0].panels[0].layers;
    assert_eq!(layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Trait couleur", "Trait", "Autre"], "du bas vers le haut, la couleur sous le trait");
    assert!(layers[0].drawn && !layers[0].vector, "un calque bitmap avec un dessin");
    assert!(out.notes.iter().any(|n| n.contains("Calques de couleur") && n.contains("à vérifier")), "{:?}", out.notes);
}

#[test]
fn a_copy_of_a_line_drawing_takes_the_colour_of_the_original() {
    let (mut app, _) = with_colour("copie-dessin", 0.0);
    app.fill_at(pos2(200.0, 150.0));
    app.cur_layer = 1;
    app.cur_frame = 0;
    app.new_image(true);
    let f = app.cur_frame;
    assert!(f > 0);
    let colour = &app.layers()[0];
    let cel = colour.frames[f].expect("couleur de la copie");
    assert_eq!(app.doc.cels[&cel].raster.get(200, 150), RED, "la couleur est copiée avec le dessin");
    assert_ne!(Some(cel), colour.frames[0], "mais c'est la sienne");
    assert_eq!(shape(&colour.frames), shape(&app.layers()[1].frames));
    // Un nouveau dessin vide, lui, a une couleur vide.
    app.new_image(false);
    let g = app.cur_frame;
    let cel = app.layers()[0].frames[g].expect("couleur");
    assert!(app.doc.cels[&cel].is_empty());
}

#[test]
fn a_colour_set_beside_a_new_line_drawing_in_the_same_step_is_adopted() {
    // Une commande qui travaille sur le trait et sur sa couleur à la fois (coller des images de
    // plusieurs calques…) : la couleur posée à l'image du nouveau dessin du trait est la sienne.
    let (mut app, _) = with_colour("adoption", 0.0);
    let n = app.doc.new_cel(LayerKind::Bitmap);
    let cn = app.doc.new_cel(LayerKind::Bitmap);
    put(&mut app.doc.cels.get_mut(&cn).unwrap().raster, 10, 10, RED);
    app.layers_mut()[1].frames[7] = Some(n);
    app.layers_mut()[1].frames[8] = Some(n);
    app.layers_mut()[0].frames[7] = Some(cn);
    app.layers_mut()[0].frames[8] = Some(cn);
    app.sync_couleurs();
    assert_eq!(app.layers()[0].frames[7], Some(cn));
    assert_eq!(app.layers()[0].couleur.as_ref().and_then(|c| c.dessins.get(&n)), Some(&cn));
    // Sans cela (le trait seul change), une couleur vide neuve.
    let m = app.doc.new_cel(LayerKind::Bitmap);
    app.layers_mut()[1].frames[9] = Some(m);
    app.sync_couleurs();
    let fresh = app.layers()[0].frames[9].expect("couleur");
    assert!(app.doc.cels[&fresh].is_empty() && fresh != cn);
}
