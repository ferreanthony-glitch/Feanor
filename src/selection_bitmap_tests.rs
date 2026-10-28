//! Transformer une sélection bitmap (lasso, puis déplacer, mettre à l'échelle, tourner par la boîte) sur un grand dessin :
//! la zone se montre par une feuille flottante pendant le geste (aucun pixel rééchantillonné à chaque image sur le fil de
//! l'interface), le rendu exact se calcule au lâcher sur un autre fil, et il est identique à celui d'avant (la zone repart
//! toujours des pixels d'origine : aucun flou cumulé).
//!
//! Mesure d'avant (dessin 3840 × 2160, un octogone qui en couvre les trois quarts, machine d'essai, `--release`) : saisie 341 ms ;
//! chaque image d'un glissé 215 ms (déplacer), 235 ms (échelle, jusqu'à 330), 240 ms (rotation, jusqu'à 418).

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, Color32};

use crate::app::{App, Tool};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::pixsel::{self, PixelMask};
use crate::pixtools::{BDrag, BSelection};
use crate::raster::{Raster, TILE};
use crate::selbox::{Handle, SelFrame};
use crate::vector::Affine;

fn app_grand(nom: &str, w: usize, h: usize) -> (App, CelId) {
    let dir = std::env::temp_dir().join(format!("shot-selbitmap-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    // Un dessin plein, sans motif répété : chaque tuile est différente.
    {
        let c = doc.cels.get_mut(&cel).unwrap();
        for ty in 0..h.div_ceil(TILE) {
            for tx in 0..w.div_ceil(TILE) {
                let idx = ty * c.raster.tiles_x + tx;
                let tile = c.raster.tile_mut(idx);
                for y in 0..TILE.min(h - ty * TILE) {
                    for x in 0..TILE.min(w - tx * TILE) {
                        let (gx, gy) = (tx * TILE + x, ty * TILE + y);
                        let (r, g, b) = ((gx * 7 + gy * 3) as u8, (gx ^ gy) as u8, (gx * gy / 97) as u8);
                        // (De l'alpha partout, entier ou non : le rééchantillonnage a des bords à mélanger.)
                        let a = if (gx / 40 + gy / 40) % 5 == 0 { 180 } else { 255 };
                        tile[y * TILE + x] = Color32::from_rgba_premultiplied(r.min(a), g.min(a), b.min(a), a);
                    }
                }
            }
        }
        c.version = 1;
    }
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app.tool = Tool::Select;
    (app, cel)
}

/// Un lasso (un octogone) qui couvre presque tout le dessin.
fn grand_lasso(app: &mut App, cel: CelId) {
    let (w, h) = (app.doc.width as f32, app.doc.height as f32);
    let polygon = vec![
        pos2(w * 0.08, h * 0.2),
        pos2(w * 0.3, h * 0.05),
        pos2(w * 0.7, h * 0.05),
        pos2(w * 0.92, h * 0.2),
        pos2(w * 0.92, h * 0.8),
        pos2(w * 0.7, h * 0.95),
        pos2(w * 0.3, h * 0.95),
        pos2(w * 0.08, h * 0.8),
    ];
    let mask = PixelMask::from_polygons(std::slice::from_ref(&polygon), app.doc.width, app.doc.height).expect("masque");
    app.bsel = Some(BSelection { cel, mask, outline: vec![polygon], lifted: None });
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn raster(app: &App, cel: CelId) -> Raster {
    app.doc.cels[&cel].raster.clone()
}

fn meme_image(a: &Raster, b: &Raster) -> bool {
    a.tiles == b.tiles
}

/// Le geste de la boîte `handle`, `n` images, de plus en plus loin ; rend la dernière position du pointeur.
fn glisser(app: &mut App, handle: Handle, de: usize, n: usize, temps: &mut Vec<f64>) {
    let frame: SelFrame = app.sel_frame().expect("boîte");
    let rect0 = frame.rect0;
    let start = match handle {
        Handle::Move => rect0.center(),
        Handle::Corner(_) => rect0.right_bottom(),
        _ => pos2(rect0.right_bottom().x + 10.0, rect0.right_bottom().y + 10.0),
    };
    app.bdrag = Some(BDrag::Transform { handle, start, frame, field: false });
    for k in de + 1..=de + n {
        let d = k as f32 * 9.0;
        let p = match handle {
            Handle::Move => pos2(start.x + d, start.y + d * 0.5),
            Handle::Corner(_) => pos2(start.x - d, start.y - d * 0.7),
            _ => pos2(start.x - d * 2.0, start.y + d),
        };
        let t = Instant::now();
        app.bitmap_drag(p, false, false);
        temps.push(ms(t));
    }
}

/// Les images jusqu'à l'arrivée du rendu exact (comme l'écran qui tourne), ou faux au bout de `limite`.
fn attendre_le_rendu(app: &mut App, limite: Duration) -> bool {
    let t = Instant::now();
    while t.elapsed() < limite {
        app.poll_bitmap_settle();
        if !app.bsel.as_ref().and_then(|s| s.lifted.as_ref()).is_some_and(|l| l.stale) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// Ce que donnait l'ancien chemin : le dessin sans la zone, les pixels posés par `aff`, au pixel près.
fn attendu(original: &Raster, mask: &PixelMask, aff: &Affine) -> Raster {
    let mut r = original.clone();
    let f = pixsel::lift(&mut r, mask);
    pixsel::stamp(&mut r, &f, aff);
    r
}

#[test]
fn a_gesture_shows_the_floating_sheet_and_the_exact_result_lands_identical_to_before() {
    let (mut app, cel) = app_grand("exact", 640, 360);
    grand_lasso(&mut app, cel);
    let original = raster(&app, cel);
    let mask = app.bsel.as_ref().unwrap().mask.clone();
    assert!(app.lift_selection());
    for (handle, nom) in [(Handle::Move, "déplacer"), (Handle::Corner(2), "échelle"), (Handle::Rotate(0), "rotation")] {
        let avant_le_geste = raster(&app, cel);
        let mut temps = Vec::new();
        glisser(&mut app, handle, 0, 8, &mut temps);
        let aff = app.bsel.as_ref().unwrap().lifted.as_ref().unwrap().aff;
        assert!(aff != Affine::IDENTITY || handle == Handle::Move, "{nom} : une transformation");
        // Pendant le geste : le dessin n'a pas été touché (aucun pixel rééchantillonné), la feuille flottante porte le geste.
        let f = app.flottante.as_ref().expect("feuille flottante");
        assert!(f.bitmap && f.src == cel && f.aff == aff, "{nom}");
        let plan = app.build_plan(0, false, true);
        let (_, sheet) = crate::flottante::slot_ids(0);
        assert!(plan.items.iter().any(|i| i.cel == sheet && i.pre.as_ref().is_some_and(|p| p.aff == aff)), "{nom} : le plan montre la feuille avec la transformation du geste");
        assert!(plan.items.iter().all(|i| i.cel != cel), "{nom} : le dessin d'origine n'est pas montré en double");
        assert!(meme_image(&raster(&app, cel), &avant_le_geste), "{nom} : l'image du dessin n'a pas bougé pendant le geste");
        // Le lâcher ne calcule rien ici : le rendu exact arrive d'un autre fil.
        let t = Instant::now();
        app.finish_bdrag();
        assert!(ms(t) < 50.0, "{nom} : le lâcher ne rééchantillonne pas sur le fil de l'interface ({:.0} ms)", ms(t));
        assert!(attendre_le_rendu(&mut app, Duration::from_secs(30)), "{nom} : le rendu exact arrive");
        assert!(app.flottante.is_none(), "{nom} : la feuille s'en va avec l'arrivée du rendu");
        let exact = attendu(&original, &mask, &aff);
        assert!(meme_image(&raster(&app, cel), &exact), "{nom} : le même résultat que le rééchantillonnage direct des pixels d'origine");
    }
}

#[test]
fn a_new_gesture_before_the_exact_result_arrives_throws_it_away_and_nothing_blurs() {
    let (mut app, cel) = app_grand("reprise", 640, 360);
    grand_lasso(&mut app, cel);
    let original = raster(&app, cel);
    let mask = app.bsel.as_ref().unwrap().mask.clone();
    assert!(app.lift_selection());
    let mut temps = Vec::new();
    glisser(&mut app, Handle::Corner(2), 0, 5, &mut temps);
    app.finish_bdrag();
    // Sans attendre : un second geste (tourner) reprend la feuille, depuis la boîte telle qu'on la voit.
    glisser(&mut app, Handle::Rotate(0), 0, 6, &mut temps);
    let aff = app.bsel.as_ref().unwrap().lifted.as_ref().unwrap().aff;
    app.finish_bdrag();
    assert!(attendre_le_rendu(&mut app, Duration::from_secs(30)));
    // (Aucun flou cumulé : le résultat est celui des pixels d'origine passés une seule fois par la transformation finale.)
    assert!(meme_image(&raster(&app, cel), &attendu(&original, &mask, &aff)));
}

#[test]
fn acting_before_the_exact_result_arrives_lays_the_pixels_first_and_undo_gives_the_old_drawing_back() {
    let (mut app, cel) = app_grand("pose", 640, 360);
    grand_lasso(&mut app, cel);
    let original = raster(&app, cel);
    let mask = app.bsel.as_ref().unwrap().mask.clone();
    let pas = app.history.undo_len();
    assert!(app.lift_selection());
    let mut temps = Vec::new();
    glisser(&mut app, Handle::Move, 0, 6, &mut temps);
    let aff = app.bsel.as_ref().unwrap().lifted.as_ref().unwrap().aff;
    app.finish_bdrag();
    // Quitter la sélection tout de suite : la zone se pose (le rendu en route est attendu), une seule étape d'annulation.
    app.drop_bitmap_selection();
    assert!(app.bsel.is_none() && app.flottante.is_none());
    assert!(meme_image(&raster(&app, cel), &attendu(&original, &mask, &aff)), "les pixels sont posés à leur place");
    assert_eq!(app.history.undo_len(), pas + 1, "une seule étape d'annulation");
    app.undo();
    assert!(meme_image(&raster(&app, cel), &original), "Ctrl+Z rend le dessin d'avant");
}

#[test]
fn erasing_inside_or_outside_a_zone_being_moved_ignores_the_pending_exact_result() {
    for inside in [true, false] {
        let (mut app, cel) = app_grand(if inside { "efface-dedans" } else { "efface-dehors" }, 320, 180);
        grand_lasso(&mut app, cel);
        let original = raster(&app, cel);
        let mask = app.bsel.as_ref().unwrap().mask.clone();
        assert!(app.lift_selection());
        let mut temps = Vec::new();
        glisser(&mut app, Handle::Move, 0, 4, &mut temps);
        let aff = app.bsel.as_ref().unwrap().lifted.as_ref().unwrap().aff;
        app.finish_bdrag();
        app.bitmap_erase(inside);
        assert!(app.flottante.is_none());
        let apres = raster(&app, cel);
        if inside {
            // Dedans : ce qui reste est le dessin sans la zone.
            let mut sans = original.clone();
            pixsel::lift(&mut sans, &mask);
            assert!(meme_image(&apres, &sans), "effacer dedans efface la zone là où elle est vue");
        } else {
            let mut seule = Raster::new(original.width, original.height);
            let mut tout = original.clone();
            let f = pixsel::lift(&mut tout, &mask);
            pixsel::stamp(&mut seule, &f, &aff);
            assert!(meme_image(&apres, &seule), "effacer autour garde seulement la zone, là où elle est vue");
        }
        // Rien ne se pose plus tard.
        std::thread::sleep(Duration::from_millis(50));
        app.poll_bitmap_settle();
        assert!(meme_image(&raster(&app, cel), &apres));
    }
}

#[test]
fn an_identity_stamp_is_the_same_as_the_general_resampling() {
    // (La saisie d'une zone pose ses pixels sans transformation : le chemin rapide donne exactement ce que donnait le général.)
    let (mut app, cel) = app_grand("identite", 320, 180);
    grand_lasso(&mut app, cel);
    let mut r = raster(&app, cel);
    let mask = app.bsel.as_ref().unwrap().mask.clone();
    let f = pixsel::lift(&mut r, &mask);
    let mut rapide = r.clone();
    pixsel::stamp(&mut rapide, &f, &Affine::IDENTITY);
    // Une transformation qui n'en est pas une (1e-7 pixel se perd dans l'arrondi) passe par le chemin général.
    let mut general = r.clone();
    let aff = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 1e-7, ty: 0.0 };
    assert!(aff != Affine::IDENTITY);
    pixsel::stamp(&mut general, &f, &aff);
    assert!(meme_image(&rapide, &general), "le chemin général donne l'image du rapide, au pixel près");
}

/// Le coût de chaque image d'un glissé sur un dessin 4K et une grande zone : la feuille flottante est faite une fois (première
/// image), puis chaque image ne change qu'un paramètre.
#[test]
fn dragging_a_big_selection_on_a_4k_drawing_costs_almost_nothing_per_frame() {
    let (mut app, cel) = app_grand("4k", 3840, 2160);
    grand_lasso(&mut app, cel);
    let t = Instant::now();
    assert!(app.lift_selection());
    let saisie = ms(t);
    let mut pire_mediane = 0.0f64;
    for (nom, handle) in [("déplacer", Handle::Move), ("échelle", Handle::Corner(2)), ("rotation", Handle::Rotate(0))] {
        let mut temps = Vec::new();
        glisser(&mut app, handle, 0, 12, &mut temps);
        let premiere = temps[0];
        let mut suite = temps[1..].to_vec();
        suite.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (mediane, pire) = (suite[suite.len() / 2], suite[suite.len() - 1]);
        let t = Instant::now();
        app.finish_bdrag();
        println!("{nom} : première image {premiere:.1} ms, médiane {mediane:.2} ms, la plus lente {pire:.1} ms, lâcher {:.1} ms (saisie {saisie:.0} ms)", ms(t));
        // (Avant : 215 à 240 ms par image, jusqu'à 418.) Une marge large : la machine d'essai peut être chargée.
        assert!(mediane < 20.0, "{nom} : une image d'un glissé ne doit rien rééchantillonner ({mediane:.1} ms)");
        assert!(pire < 120.0, "{nom} : aucune image ne fige ({pire:.1} ms)");
        assert!(premiere < 1500.0, "{nom} : la feuille flottante se fait une fois ({premiere:.0} ms)");
        pire_mediane = pire_mediane.max(mediane);
        assert!(attendre_le_rendu(&mut app, Duration::from_secs(60)), "{nom} : le rendu exact arrive");
    }
    assert!(pire_mediane < 20.0);
}

#[test]
fn lifting_tile_by_tile_gives_exactly_what_lifting_pixel_by_pixel_gave() {
    // Le soulèvement (saisie d'une zone) va tuile par tuile ; l'ancien allait pixel par pixel. Même zone, même image : mêmes pixels.
    fn naif(raster: &mut Raster, mask: &PixelMask) -> pixsel::Floating {
        fn reduit(c: Color32, k: f32) -> Color32 {
            if k >= 1.0 {
                return c;
            }
            let m = |v: u8| (v as f32 * k).round() as u8;
            Color32::from_rgba_premultiplied(m(c.r()), m(c.g()), m(c.b()), m(c.a()))
        }
        let mut pixels = vec![Color32::TRANSPARENT; mask.w * mask.h];
        for ly in 0..mask.h {
            for lx in 0..mask.w {
                let m = mask.data[ly * mask.w + lx];
                if m <= 0.0 {
                    continue;
                }
                let (x, y) = (mask.x0 + lx, mask.y0 + ly);
                let c = raster.get(x, y);
                if c.a() == 0 {
                    continue;
                }
                pixels[ly * mask.w + lx] = reduit(c, m);
                let idx = raster.tile_index_of(x, y);
                raster.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = reduit(c, 1.0 - m);
            }
        }
        pixsel::Floating { x0: mask.x0, y0: mask.y0, w: mask.w, h: mask.h, pixels }
    }
    // Un dessin dont la taille n'est pas un multiple de la tuile, avec des tuiles vides au milieu.
    let (mut app, cel) = app_grand("lift-naif", 333, 211);
    {
        let c = app.doc.cels.get_mut(&cel).unwrap();
        for idx in [1, 4, 7] {
            c.raster.tiles[idx] = None;
        }
    }
    let original = raster(&app, cel);
    let polygones = [
        // Un octogone qui déborde du dessin (le masque est borné à l'image).
        vec![pos2(-40.0, 30.0), pos2(120.0, -20.0), pos2(300.0, -10.0), pos2(380.0, 90.0), pos2(350.0, 250.0), pos2(150.0, 260.0), pos2(-30.0, 190.0)],
        // Un petit triangle sur un bord de tuile.
        vec![pos2(60.5, 60.5), pos2(70.2, 61.1), pos2(64.0, 71.7)],
        // Un rectangle aligné sur les tuiles.
        vec![pos2(64.0, 64.0), pos2(192.0, 64.0), pos2(192.0, 128.0), pos2(64.0, 128.0)],
    ];
    for polygone in polygones {
        let mask = PixelMask::from_polygons(std::slice::from_ref(&polygone), 333, 211).expect("masque");
        let (mut a, mut b) = (original.clone(), original.clone());
        let fa = pixsel::lift(&mut a, &mask);
        let fb = naif(&mut b, &mask);
        assert!(fa.pixels == fb.pixels, "les pixels soulevés sont les mêmes");
        assert!(meme_image(&a, &b), "le dessin sans la zone est le même");
    }
}
