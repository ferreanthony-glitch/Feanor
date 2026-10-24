//! Choisir une couleur recolore la sélection (Anthony, 2026-10-24) : vectoriel (traits, zones, textes), bitmap (alpha gardé), une
//! seule étape d'annulation par choix, rien sans sélection.

use eframe::egui::{self, pos2, Color32, Rect};

use crate::app::{App, Tool};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::pixsel::{rect_polygon, PixelMask};
use crate::pixtools::BSelection;
use crate::recolorsel::{recolorer_pixel, CLE_GESTE};
use crate::vector::{Element, VPoint};
use crate::vtools::VSelection;

const ROUGE: [u8; 3] = [200, 0, 0];
const BLEU: [u8; 3] = [0, 0, 200];
const VERT: [u8; 3] = [10, 200, 30];

fn app_avec(doc: Document, nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-recolorsel-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app
}

fn doc_un_calque(kind: LayerKind) -> (Document, CelId) {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(kind);
    let mut l = Layer::new("Dessin", kind);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    (doc, cel)
}

fn trait_simple(color: [u8; 3], y: f32) -> Element {
    Element::Stroke { points: vec![VPoint { x: 4.0, y, r: 2.0 }, VPoint { x: 50.0, y, r: 2.0 }], color, opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() }
}

fn zone(color: [u8; 3]) -> Element {
    Element::Fill { contours: vec![vec![[4.0, 20.0], [40.0, 20.0], [40.0, 44.0], [4.0, 44.0]]], color, opacity: 1.0 }
}

/// Un dessin vectoriel : 0 trait rouge, 1 trait bleu, 2 zone rouge, 3 trait rouge ; la sélection : 0 et 2 (outil Sélection).
fn app_vecto(nom: &str) -> (App, CelId) {
    let (doc, cel) = doc_un_calque(LayerKind::Vector);
    let mut app = app_avec(doc, nom);
    app.doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = vec![trait_simple(ROUGE, 6.0), trait_simple(BLEU, 12.0), zone(ROUGE), trait_simple(ROUGE, 16.0)];
    app.doc.cels.get_mut(&cel).unwrap().rerender();
    app.tool = Tool::Select;
    app.vsel = Some(VSelection { cel, items: vec![0, 2] });
    (app, cel)
}

fn couleurs(app: &App, cel: CelId) -> Vec<[u8; 3]> {
    app.drawing(cel).unwrap().elements.iter().map(|e| e.color()).collect()
}

/// Un dessin bitmap : un carré rouge opaque de 8 × 8 en (4, 4), un pixel bleu à moitié transparent en (20, 4), un pixel
/// vert hors de la zone en (40, 30) ; la zone choisie : (2, 2) à (30, 20), outil Sélection.
fn app_bitmap(nom: &str) -> (App, CelId) {
    let (doc, cel) = doc_un_calque(LayerKind::Bitmap);
    let mut app = app_avec(doc, nom);
    {
        let r = &mut app.doc.cels.get_mut(&cel).unwrap().raster;
        let mut poser = |x: usize, y: usize, c: Color32| {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = c;
        };
        for y in 4..12 {
            for x in 4..12 {
                poser(x, y, Color32::from_rgb(200, 0, 0));
            }
        }
        // Un pixel bleu à moitié transparent (prémultiplié) : alpha 128.
        poser(20, 4, Color32::from_rgba_premultiplied(0, 0, 100, 128));
        poser(40, 30, Color32::from_rgb(0, 200, 0));
    }
    app.doc.cels.get_mut(&cel).unwrap().version = 1;
    let polygone = rect_polygon(Rect::from_min_max(pos2(2.0, 2.0), pos2(30.0, 20.0)));
    let mask = PixelMask::from_polygons(std::slice::from_ref(&polygone), app.doc.width, app.doc.height).unwrap();
    app.bsel = Some(BSelection { cel, mask, outline: vec![polygone], lifted: None });
    app.tool = Tool::Select;
    (app, cel)
}

fn pixels(app: &App, cel: CelId) -> Vec<Color32> {
    app.doc.cels[&cel].raster.to_flat()
}

// ------------------------------------------------------------------------------------ vectoriel

#[test]
fn choosing_a_colour_gives_it_to_the_selected_strokes_and_fills() {
    let (mut app, cel) = app_vecto("vecto");
    app.choose_front(VERT);
    assert_eq!(app.color, VERT);
    assert_eq!(couleurs(&app, cel), vec![VERT, BLEU, VERT, ROUGE], "les éléments choisis prennent la couleur, les autres non");
    // Le dessin suit : la zone (sans moteur) est redessinée dans la zone touchée, tout de suite.
    let p = app.doc.cels[&cel].raster.get(20, 30);
    assert_eq!((p.r(), p.g(), p.b(), p.a()), (VERT[0], VERT[1], VERT[2], 255), "la zone est verte à l'image : {p:?}");
    // Le trait bleu non choisi est resté bleu à l'image.
    let b = app.doc.cels[&cel].raster.get(20, 12);
    assert!(b.b() > 150 && b.r() < 60, "{b:?}");
    // La sélection reste choisie.
    assert_eq!(app.vsel.as_ref().map(|s| s.items.clone()), Some(vec![0, 2]));
}

#[test]
fn the_same_colour_again_changes_nothing_and_adds_no_step() {
    let (mut app, _) = app_vecto("meme");
    app.choose_front(ROUGE);
    assert_eq!(app.history.undo_len(), 0, "la couleur était déjà celle-là : rien à annuler");
}

#[test]
fn one_colour_choice_is_one_undo_step_and_undo_gives_the_colours_back() {
    let (mut app, cel) = app_vecto("annuler");
    let pas = app.history.undo_len();
    app.choose_front(VERT);
    assert_eq!(app.history.undo_len(), pas + 1);
    app.undo();
    assert_eq!(couleurs(&app, cel), vec![ROUGE, BLEU, ROUGE, ROUGE], "une annulation rend toutes les couleurs");
    assert_eq!(app.history.undo_len(), pas);
    app.redo();
    assert_eq!(couleurs(&app, cel), vec![VERT, BLEU, VERT, ROUGE]);
    // Deux choix qui se suivent hors d'un geste : deux étapes. (Annuler a lâché la sélection : on la reprend.)
    app.vsel = Some(VSelection { cel, items: vec![0, 2] });
    app.gesture.reset();
    app.choose_front([1, 2, 3]);
    app.gesture.reset();
    app.choose_front([4, 5, 6]);
    assert_eq!(app.history.undo_len(), pas + 3);
}

/// Une image d'écran du geste : le début d'image referme un geste que rien n'a prolongé (`end_gesture_if_idle`), le
/// panneau choisit, la fin d'image prolonge le geste tant que le curseur est tenu.
fn image_de_geste(app: &mut App, couleur: Option<[u8; 3]>, tenu: bool) {
    app.gesture.end_frame(false);
    if let Some(c) = couleur {
        app.choose_front(c);
    }
    if tenu {
        app.keep_gesture(CLE_GESTE);
    }
}

#[test]
fn dragging_a_slider_is_a_single_undo_step_for_the_whole_drag() {
    let (mut app, cel) = app_vecto("curseur");
    let pas = app.history.undo_len();
    for k in 0..30u8 {
        image_de_geste(&mut app, Some([k * 8, 200 - k * 4, 30]), true);
    }
    assert_eq!(app.history.undo_len(), pas + 1, "trente images de glissé, une seule étape");
    // Un arrêt du curseur tenu, sans changement, ne coupe pas le geste.
    image_de_geste(&mut app, None, true);
    image_de_geste(&mut app, Some([250, 250, 0]), true);
    assert_eq!(app.history.undo_len(), pas + 1);
    // Lâché : deux images sans rien tenir, puis un autre choix est une autre étape.
    image_de_geste(&mut app, None, false);
    image_de_geste(&mut app, None, false);
    image_de_geste(&mut app, Some([9, 9, 250]), false);
    assert_eq!(app.history.undo_len(), pas + 2);
    // Annuler le dernier choix rend la couleur du glissé d'avant, annuler le glissé rend les couleurs d'origine.
    app.undo();
    assert_eq!(couleurs(&app, cel)[0], [250, 250, 0]);
    app.undo();
    assert_eq!(couleurs(&app, cel), vec![ROUGE, BLEU, ROUGE, ROUGE]);
}

#[test]
fn engine_strokes_are_replayed_by_the_worker_never_on_the_interface_thread() {
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    let preset = lib.presets.iter().find(|p| p.name == "Feutre").expect("brosse").settings;
    let pts: Vec<_> = (0..=30).map(|i| (pos2(8.0 + i as f32, 20.0 + (i as f32 * 0.3).sin() * 4.0), 0.8, i as f64 / 100.0)).collect();
    let trait_moteur = crate::affichage_tests::trait_rouge(&preset, &lib, 64, 48, &pts);
    let (doc, cel) = crate::affichage_tests::doc_with(64, 48, vec![trait_moteur]);
    let mut app = app_avec(doc, "moteur");
    // (Le calque est celui de ce dessin.)
    app.doc.panels[0].layers[0].kind = LayerKind::Vector;
    app.doc.panels[0].layers[0].frames = vec![Some(cel); 24];
    app.tool = Tool::Select;
    app.vsel = Some(VSelection { cel, items: vec![0] });
    assert!(app.drawing(cel).unwrap().elements[0].cache_ready(), "au départ, le trait a ses pixels exacts en cache");
    app.choose_front(VERT);
    assert_eq!(couleurs(&app, cel), vec![VERT]);
    assert!(app.drawing(cel).unwrap().elements[0].cache_missing(), "aucun rejeu sur le fil de l'interface : le trait n'a plus ses pixels en cache");
    assert!(app.rejeu.is_pending(cel), "le rendu exact est demandé au fil de travail");
}

#[test]
fn a_selection_on_another_tool_or_a_locked_layer_is_left_alone() {
    // Un outil qui ne garde pas la sélection (le pinceau) : choisir une couleur sert au pinceau, la sélection oubliée n'est pas touchée.
    let (mut app, cel) = app_vecto("pinceau");
    app.tool = Tool::Brush;
    app.choose_front(VERT);
    assert_eq!(app.color, VERT);
    assert_eq!(couleurs(&app, cel), vec![ROUGE, BLEU, ROUGE, ROUGE]);
    // Un calque verrouillé : rien.
    let (mut app, cel) = app_vecto("verrou");
    app.doc.panels[0].layers[0].locked = true;
    app.choose_front(VERT);
    assert_eq!(couleurs(&app, cel), vec![ROUGE, BLEU, ROUGE, ROUGE]);
}

#[test]
fn without_a_selection_choosing_a_colour_only_sets_the_brush_colour() {
    // Vectoriel.
    let (mut app, cel) = app_vecto("sans-v");
    app.vsel = None;
    let avant = (couleurs(&app, cel), pixels(&app, cel), app.history.undo_len());
    app.choose_front(VERT);
    assert_eq!(app.color, VERT, "la couleur d'avant-plan change");
    assert_eq!((couleurs(&app, cel), app.history.undo_len()), (avant.0, avant.2), "le dessin ne change pas, aucune étape");
    assert!(pixels(&app, cel) == avant.1, "l'image non plus");
    // Bitmap.
    let (mut app, cel) = app_bitmap("sans-b");
    app.bsel = None;
    let (avant, pas) = (pixels(&app, cel), app.history.undo_len());
    app.choose_front(VERT);
    assert_eq!(app.color, VERT);
    assert!(pixels(&app, cel) == avant, "le dessin bitmap ne change pas");
    assert_eq!(app.history.undo_len(), pas);
}

// ------------------------------------------------------------------------------------ bitmap

#[test]
fn the_pixel_recolour_keeps_the_alpha_and_mixes_by_the_coverage() {
    // Opaque : la couleur neuve, tout simplement.
    assert_eq!(recolorer_pixel(Color32::from_rgb(200, 0, 0), VERT, 1.0), Color32::from_rgb(10, 200, 30));
    // Transparent : rien.
    assert_eq!(recolorer_pixel(Color32::TRANSPARENT, VERT, 1.0), Color32::TRANSPARENT);
    // À moitié transparent : l'alpha reste, la couleur prémultipliée suit.
    let moitie = recolorer_pixel(Color32::from_rgba_premultiplied(0, 0, 100, 128), [200, 100, 0], 1.0);
    assert_eq!(moitie.a(), 128);
    assert!((moitie.r() as i32 - 100).abs() <= 1 && (moitie.g() as i32 - 50).abs() <= 1 && moitie.b() <= 1, "{moitie:?}");
    // Un bord de zone à moitié couvert : à mi-chemin entre l'ancienne et la nouvelle couleur.
    let bord = recolorer_pixel(Color32::from_rgb(200, 0, 0), [0, 200, 0], 0.5);
    assert_eq!(bord.a(), 255);
    assert!((bord.r() as i32 - 100).abs() <= 1 && (bord.g() as i32 - 100).abs() <= 1, "{bord:?}");
    // Jamais de couleur au-dessus de l'alpha (prémultipliée).
    for a in [1u8, 17, 100, 254] {
        let p = recolorer_pixel(Color32::from_rgba_premultiplied(0, 0, a, a), [255, 255, 255], 1.0);
        assert!(p.r() <= p.a() && p.g() <= p.a() && p.b() <= p.a() && p.a() == a, "{p:?}");
    }
}

#[test]
fn choosing_a_colour_recolours_the_selected_pixels_and_keeps_alpha_and_the_rest() {
    let (mut app, cel) = app_bitmap("bitmap");
    let avant = pixels(&app, cel);
    app.choose_front(VERT);
    let apres = pixels(&app, cel);
    let w = app.doc.width;
    // Le carré : vert, opaque. Le pixel à moitié transparent : alpha gardé, couleur neuve.
    for y in 4..12 {
        for x in 4..12 {
            assert_eq!(apres[y * w + x], Color32::from_rgb(10, 200, 30), "({x}, {y})");
        }
    }
    let moitie = apres[4 * w + 20];
    assert_eq!(moitie.a(), 128, "l'alpha est gardé");
    assert!((moitie.g() as i32 - (200 * 128 / 255)).abs() <= 1 && moitie.b() as i32 <= 30 * 128 / 255 + 1, "{moitie:?}");
    // Tout ce qui est hors de la zone, ou transparent, n'a pas bougé.
    assert_eq!(apres[30 * w + 40], avant[30 * w + 40], "un pixel hors de la zone");
    let change = apres.iter().zip(&avant).filter(|(a, b)| a != b).count();
    assert_eq!(change, 8 * 8 + 1, "le carré et le pixel à moitié transparent, rien d'autre");
    assert_eq!(apres[10 * w + 25], Color32::TRANSPARENT, "un pixel vide de la zone reste vide");
    // La sélection est toujours là.
    assert!(app.bsel.is_some());
}

#[test]
fn a_bitmap_colour_choice_is_one_undo_step_even_while_dragging() {
    let (mut app, cel) = app_bitmap("bitmap-glisse");
    let (avant, pas) = (pixels(&app, cel), app.history.undo_len());
    for k in 0..20u8 {
        image_de_geste(&mut app, Some([k * 10, 250 - k * 10, 20 + k]), true);
    }
    assert_eq!(app.history.undo_len(), pas + 1, "vingt images de glissé : une étape");
    assert!(pixels(&app, cel) != avant);
    app.undo();
    assert!(pixels(&app, cel) == avant, "une annulation rend tous les pixels d'avant");
    assert_eq!(app.history.undo_len(), pas);
    app.redo();
    assert!(pixels(&app, cel) != avant);
}

#[test]
fn lifted_pixels_are_recoloured_too_and_land_with_the_move_in_one_step() {
    let (mut app, cel) = app_bitmap("soulevee");
    let (avant, pas) = (pixels(&app, cel), app.history.undo_len());
    assert!(app.lift_selection());
    app.choose_front(VERT);
    assert!(app.bsel.as_ref().is_some_and(|s| s.lifted.is_some()), "la zone reste soulevée");
    let w = app.doc.width;
    assert_eq!(pixels(&app, cel)[6 * w + 6], Color32::from_rgb(10, 200, 30), "les pixels soulevés sont recolorés sur le dessin");
    // La pose (quitter la sélection) : une seule étape, qui rend le dessin d'avant.
    app.drop_bitmap_selection();
    assert_eq!(app.history.undo_len(), pas + 1);
    app.undo();
    assert!(pixels(&app, cel) == avant);
}

#[test]
fn the_colour_panel_ways_all_recolour() {
    // Pastille, code hexadécimal, intervertir, couleurs par défaut : tous passent par `choose_front` / `recolorier_la_selection`.
    let (mut app, cel) = app_vecto("manieres");
    app.gesture.reset();
    app.choose_front(crate::couleur::parse_hex("#102030").unwrap());
    assert_eq!(couleurs(&app, cel)[0], [0x10, 0x20, 0x30], "le code hexadécimal");
    app.gesture.reset();
    app.couleurs.prefs.back = [7, 8, 9];
    app.swap_colors();
    assert_eq!(app.color, [7, 8, 9]);
    assert_eq!(couleurs(&app, cel)[0], [7, 8, 9], "intervertir les couleurs");
    app.gesture.reset();
    app.default_colors();
    assert_eq!(couleurs(&app, cel)[0], crate::couleur::NOIR, "les couleurs par défaut");
    // La pipette, elle, ne recolore rien.
    app.gesture.reset();
    let avant = couleurs(&app, cel);
    app.set_front([1, 1, 1]);
    assert_eq!(couleurs(&app, cel), avant);
}
