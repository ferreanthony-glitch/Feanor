//! Remplissage au griffonnage d'un calque de couleur (`griffonnage.rs`, « On fusionne », Anthony, 2026-10-21) : le
//! remplissage des zones fermées, la fermeture des trous, le calque de couleur qui griffonne (stockage, ordre avec la peinture
//! libre, gomme, annulation), le recalcul quand le trait change (en arrière-plan, jamais sur le fil de l'interface),
//! les copies, l'enregistrement, et la conversion des anciens calques CTG en calques de couleur (même image).
//!
//! Les essais qui passent par le dessin dérivé affiché (`fx::differe`, le cache des dessins dérivés) touchent à l'état
//! commun : ils le prennent en lecture (`fxcache::monde::lire`), et leurs dessins portent des versions à eux (le cache est
//! commun à tous les essais).

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, Color32, Pos2};

use crate::app::{App, Tool};
use crate::calquecouleur::CalqueCouleur;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{ajouter_heritage_ctg, dossier, ecrire_json, empreinte, lire_json, projet_enregistre, GOLDEN_CTG};
use crate::fx;
use crate::fxcache::monde;
use crate::griffonnage::{self, GriffonInput};
use crate::raster::{Raster, TILE};
use crate::render::{self, PlanItem};

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

// ------------------------------------------------------------------ l'algorithme

/// Une feuille 64×64 : un carré de traits noirs de (10, 10) à (40, 40), épais de 2 pixels.
fn square() -> Raster {
    let mut r = Raster::new(64, 64);
    for i in 10..=40 {
        for t in 0..2 {
            for (x, y) in [(i, 10 + t), (i, 40 - t), (10 + t, i), (40 - t, i)] {
                r.tile_mut(0)[y * 64 + x] = Color32::BLACK;
            }
        }
    }
    r
}

fn dot(r: &mut Raster, x: usize, y: usize, c: Color32) {
    r.tile_mut(0)[y * 64 + x] = c;
}

#[test]
fn a_graffiti_fills_the_closed_area_it_touches() {
    let lines = square();
    let mut g = Raster::new(64, 64);
    dot(&mut g, 25, 25, Color32::RED);
    let out = griffonnage::fill(&[&lines], &g, 0);
    assert_eq!(out.get(20, 30), Color32::RED, "toute la zone");
    assert_eq!(out.get(12, 25), Color32::RED, "jusqu'au trait");
    assert_eq!(out.get(11, 25), Color32::RED, "et sous le trait");
    assert_eq!(out.get(5, 5), Color32::TRANSPARENT, "pas dehors");
    assert_eq!(out.get(50, 25), Color32::TRANSPARENT);
}

#[test]
fn the_most_present_colour_wins_and_lines_do_not_count() {
    let lines = square();
    let mut g = Raster::new(64, 64);
    for x in 20..25 {
        dot(&mut g, x, 20, Color32::BLUE);
    }
    dot(&mut g, 30, 30, Color32::RED);
    // Un graffiti posé sur le trait ne remplit rien.
    dot(&mut g, 10, 50, Color32::GREEN);
    dot(&mut g, 10, 25, Color32::GREEN);
    let out = griffonnage::fill(&[&lines], &g, 0);
    assert_eq!(out.get(30, 30), Color32::BLUE, "le bleu est le plus présent dans la zone");
    assert_eq!(out.get(5, 5), Color32::GREEN, "le vert hors du carré remplit le dehors");
}

#[test]
fn small_holes_in_the_lines_are_closed() {
    // Le carré, avec un trou de 4 pixels dans son côté gauche (y de 23 à 26).
    let mut lines = square();
    for y in 23..27 {
        for x in 10..12 {
            lines.tile_mut(0)[y * 64 + x] = Color32::TRANSPARENT;
        }
    }
    let mut g = Raster::new(64, 64);
    dot(&mut g, 25, 25, Color32::RED);
    let leaking = griffonnage::fill(&[&lines], &g, 0);
    assert_eq!(leaking.get(5, 5), Color32::RED, "sans fermeture : la couleur fuit dehors");
    let closed = griffonnage::fill(&[&lines], &g, 2);
    assert_eq!(closed.get(5, 5), Color32::TRANSPARENT, "trou de 4 pixels fermé (2 de chaque côté)");
    assert_eq!(closed.get(25, 25), Color32::RED);
    assert_eq!(closed.get(13, 30), Color32::RED, "la couleur revient jusqu'au trait");
    assert_eq!(closed.get(11, 30), Color32::RED, "et dessous");
    assert_eq!(closed.get(4, 25), Color32::TRANSPARENT, "rien ne sort par le trou");
    // Un trou plus grand que la fermeture laisse passer.
    assert_eq!(griffonnage::fill(&[&lines], &g, 1).get(5, 5), Color32::RED);
}

#[test]
fn without_lines_a_graffiti_fills_everything() {
    let mut g = Raster::new(64, 64);
    dot(&mut g, 3, 3, Color32::RED);
    let out = griffonnage::fill(&[], &g, 0);
    assert_eq!(out.get(63, 63), Color32::RED);
}

#[test]
fn several_sources_close_the_zones_together() {
    // Le carré est fait de deux calques : ses côtés gauche et droit d'un côté, haut et bas de l'autre.
    let (mut vertical, mut horizontal) = (Raster::new(64, 64), Raster::new(64, 64));
    for i in 10..=40 {
        for t in 0..2 {
            dot(&mut vertical, 10 + t, i, Color32::BLACK);
            dot(&mut vertical, 40 - t, i, Color32::BLACK);
            dot(&mut horizontal, i, 10 + t, Color32::BLACK);
            dot(&mut horizontal, i, 40 - t, Color32::BLACK);
        }
    }
    let mut g = Raster::new(64, 64);
    dot(&mut g, 25, 25, Color32::RED);
    assert_eq!(griffonnage::fill(&[&vertical], &g, 0).get(5, 5), Color32::RED, "un seul côté ne ferme rien");
    let out = griffonnage::fill(&[&vertical, &horizontal], &g, 0);
    assert_eq!(out.get(25, 25), Color32::RED);
    assert_eq!(out.get(5, 5), Color32::TRANSPARENT, "à eux deux, ils ferment la zone");
}

#[test]
fn the_stamp_is_a_disc_and_gives_back_the_tiles_it_replaced() {
    let mut r = Raster::new(64, 64);
    let old = griffonnage::tampon(&mut r, 20.0, 20.0, 5.0, Color32::RED);
    assert_eq!(r.get(20, 20), Color32::RED);
    assert_eq!(r.get(24, 20), Color32::RED);
    assert_eq!(r.get(26, 20), Color32::TRANSPARENT, "au-delà du rayon");
    assert!(!old.is_empty() && old.iter().all(|(_, t)| t.is_none()), "les tuiles d'avant : vides");
    // Hors de la feuille : rien, et pas de plantage.
    assert!(griffonnage::tampon(&mut r, -50.0, 20.0, 5.0, Color32::RED).is_empty());
    assert!(griffonnage::tampon(&mut r, 500.0, 500.0, 5.0, Color32::RED).is_empty());
}

// ------------------------------------------------------------------ le calque de couleur qui griffonne

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
    let dir = std::env::temp_dir().join(format!("shot-griffonnage-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

/// Un plan de 12 images à deux calques : « Trait » (un anneau de rayon 60 au centre de 400×300 ; `gap` : son trou) et
/// « Autre » au-dessus (vide), le calque de couleur « Trait couleur » créé dessous et courant, en mode « Griffonner »,
/// pinceau rouge. Les dessins ont des versions à eux (le cache des dessins dérivés est commun aux essais).
fn app_griffonne(name: &str, gap: f32, base: u64) -> (App, CelId) {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    // (Des numéros de dessins à cet essai : le cache des dessins dérivés, lui, est commun à tous les essais.)
    doc.reserve_id(base);
    let a = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 60.0, gap)));
    doc.cels.get_mut(&a).expect("trait").version = base;
    let mut line = Layer::new("Trait", LayerKind::Bitmap);
    line.frames = vec![Some(a); 12];
    doc.panels[0].layers = vec![line, Layer::new("Autre", LayerKind::Vector)];
    doc.panels[0].length = 12;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.run_action("calque.nouveau_couleur");
    app.run_action("calque.griffonner");
    app.tool = Tool::Brush;
    app.color = [255, 0, 0];
    (app, a)
}

fn trace(app: &mut App, from: Pos2, to: Pos2) {
    app.begin_paint(from, 1.0, false);
    for k in 0..=10 {
        let t = k as f32 / 10.0;
        app.stroke_to(pos2(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t), 1.0);
    }
    app.end_stroke();
}

/// La couleur du plan de l'image courante au pixel (`x`, `y`) : calcul exact (hors de `differe`).
fn pixel(app: &App, x: usize, y: usize) -> Color32 {
    let plan = app.build_plan(app.cur_frame, false, false);
    render::composite_pixel(&app.doc, &plan, x, y)
}

fn is_red(c: Color32) -> bool {
    c.r() > 200 && c.g() < 40 && c.b() < 40 && c.a() > 200
}

fn undo_steps(app: &App) -> usize {
    app.history.names().0.len()
}

#[test]
fn a_scribble_fills_the_closed_zone_under_the_line() {
    let (mut app, _) = app_griffonne("remplit", 0.0, 8_100_000);
    let steps = undo_steps(&app);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    assert_eq!(undo_steps(&app), steps + 1, "un geste, une étape d'annulation");
    let c = app.layers()[0].couleur.as_ref().expect("calque de couleur");
    assert!(c.remplissage && c.griffonner && c.griffonnages.len() == 1, "le griffonnage a son dessin, à part");
    let (paint, scribble) = c.griffonnages.iter().next().map(|(p, g)| (*p, *g)).expect("couple");
    assert!(app.doc.cels[&paint].is_empty(), "la peinture libre n'a pas bougé");
    assert!(!app.doc.cels[&scribble].is_empty(), "le trait est dans les griffonnages");
    // Toute la zone de l'anneau est remplie, bien au-delà du trait de pinceau, mais pas dehors.
    assert!(is_red(pixel(&app, 200, 110)), "le haut de la zone");
    assert!(is_red(pixel(&app, 170, 180)), "le bas, loin du trait");
    assert_eq!(pixel(&app, 350, 150).a(), 0, "rien hors de l'anneau");
    // Sous le trait : la couleur passe sous l'anneau (le calque de couleur est sous le trait), sans liseré.
    let plan = app.build_plan(0, false, false);
    assert_eq!(plan.items.len(), 3, "remplissage, peinture libre, trait");
    assert!(plan.items[0].ctg.is_some() && plan.items[0].cel == scribble, "le remplissage est le premier, sous la peinture");
    assert_eq!(plan.items[1].cel, paint);
    let fill = fx::shown(&app.doc, &plan.items[0]).expect("remplissage");
    assert!(fill.raster.get(259, 150).a() > 0 || fill.raster.get(260, 150).a() > 0, "la couleur passe sous le trait");
    // Annuler retire le griffonnage, et le remplissage avec lui.
    app.undo();
    assert_eq!(pixel(&app, 200, 110).a(), 0);
    app.redo();
    assert!(is_red(pixel(&app, 200, 110)));
}

#[test]
fn small_gaps_in_the_line_do_not_leak_the_scribble() {
    let (mut app, _) = app_griffonne("trou", 6.0, 8_100_100);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    assert!(is_red(pixel(&app, 200, 110)));
    assert_eq!(pixel(&app, 350, 150).a(), 0, "un trou de 6 pixels est fermé (jusqu'à 8)");
    // Sans fermeture, la couleur fuit.
    if let Some(c) = app.layers_mut()[0].couleur.as_mut() {
        c.trous = 0;
    }
    assert!(is_red(pixel(&app, 350, 150)), "sans fermeture des trous, tout le dehors est colorié");
}

#[test]
fn free_paint_and_scribbles_mix_on_the_same_layer() {
    let (mut app, _) = app_griffonne("mixte", 0.0, 8_100_200);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    // Peinture libre bleue, à la main (mode éteint), par-dessus le remplissage.
    app.run_action("calque.griffonner");
    assert!(!app.layers()[0].couleur.as_ref().is_some_and(|c| c.griffonner));
    app.color = [0, 0, 255];
    app.layers_mut()[0].couleur.as_mut().expect("calque de couleur").zone = false;
    let steps = undo_steps(&app);
    trace(&mut app, pos2(170.0, 120.0), pos2(230.0, 120.0));
    assert_eq!(undo_steps(&app), steps + 1);
    let blue = pixel(&app, 200, 120);
    assert!(blue.b() > 200 && blue.r() < 40, "la peinture libre est au-dessus du remplissage : {blue:?}");
    assert!(is_red(pixel(&app, 200, 180)), "le remplissage reste en dessous, ailleurs");
    // La gomme à la main efface la peinture, pas le remplissage ; en mode Griffonner, elle efface les griffonnages.
    app.tool = Tool::Eraser;
    trace(&mut app, pos2(160.0, 120.0), pos2(240.0, 120.0));
    assert!(is_red(pixel(&app, 200, 120)), "la peinture est partie, le remplissage est là : {:?}", pixel(&app, 200, 120));
    app.run_action("calque.griffonner");
    trace(&mut app, pos2(160.0, 150.0), pos2(240.0, 150.0));
    assert_eq!(pixel(&app, 200, 180).a(), 0, "plus de griffonnage dans la zone : plus de remplissage");
}

#[test]
fn the_bucket_in_scribble_mode_drops_a_stamp_that_fills_the_zone() {
    let (mut app, _) = app_griffonne("pot", 0.0, 8_100_300);
    app.tool = Tool::Fill;
    app.fill_at(pos2(200.0, 150.0));
    assert!(is_red(pixel(&app, 170, 180)), "le tampon remplit la zone");
    let (paint, scribble) = app.layers()[0].couleur.as_ref().and_then(|c| c.griffonnages.iter().next().map(|(p, g)| (*p, *g))).expect("couple");
    assert!(app.doc.cels[&paint].is_empty());
    assert!(!app.doc.cels[&scribble].is_empty());
}

#[test]
fn the_scribble_drawing_follows_its_paint_drawing_through_the_keys() {
    let (mut app, a) = app_griffonne("cles", 0.0, 8_100_400);
    // Un autre dessin de trait à partir de l'image 6.
    let b = app.doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 40.0, 0.0)));
    app.doc.cels.get_mut(&b).expect("trait").version = 8_100_401;
    for f in 6..12 {
        app.layers_mut()[1].frames[f] = Some(b);
    }
    app.sync_couleurs();
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    app.cur_frame = 8;
    trace(&mut app, pos2(190.0, 150.0), pos2(210.0, 150.0));
    let c = app.layers()[0].couleur.clone().expect("calque de couleur");
    assert_eq!(c.griffonnages.len(), 2, "un dessin de griffonnages par dessin de peinture");
    assert_eq!(c.dessins.keys().copied().collect::<Vec<_>>(), {
        let mut k = vec![a, b];
        k.sort_unstable();
        k
    });
    // Les images tenues partagent la peinture, donc les griffonnages.
    let frames = app.layers()[0].frames.clone();
    assert!(frames[0..6].iter().all(|f| *f == frames[0]) && frames[6..12].iter().all(|f| *f == frames[6]) && frames[0] != frames[6]);
    app.cur_frame = 3;
    assert!(is_red(pixel(&app, 200, 110)));
    app.cur_frame = 9;
    assert!(is_red(pixel(&app, 200, 130)), "le second dessin a son remplissage : l'anneau de rayon 40");
    assert_eq!(pixel(&app, 200, 100).a(), 0, "hors du petit anneau (rayon 40) : rien");
}

#[test]
fn detaching_a_layer_that_has_scribbles_frees_it_without_changing_the_image() {
    let (mut app, a) = app_griffonne("detache", 0.0, 8_150_000);
    trace(&mut app, pos2(160.0, 150.0), pos2(190.0, 150.0));
    let line = app.layers()[1].id;
    let before = render::composite_full(&app.doc, &app.build_plan(0, false, false));
    let steps = undo_steps(&app);
    app.run_action("calque.detacher_couleur");
    assert_eq!(undo_steps(&app), steps + 1);
    let c = app.layers()[0].couleur.clone().expect("toujours un calque de couleur");
    assert!(!c.est_lie() && c.remplissage && c.griffonnages.len() == 1, "libre, avec ses griffonnages");
    assert_eq!(c.sources, vec![line], "l'ancien trait devient une source : l'image ne change pas");
    assert_eq!(render::composite_full(&app.doc, &app.build_plan(0, false, false)), before);
    // Libre, il garde sa place dans la pile et ses images, et se peint comme un calque ordinaire.
    app.sync_couleurs();
    assert_eq!(app.layers()[0].couleur.as_ref().map(|c| c.est_lie()), Some(false));
    assert!(app.layers()[0].frames.iter().flatten().count() >= 1);
    // Il suit toujours le trait : une cloison dans l'anneau change son remplissage.
    {
        let c = app.doc.cels.get_mut(&a).expect("trait");
        for y in 90..210 {
            for x in 198..203 {
                put(&mut c.raster, x, y, Color32::BLACK);
            }
        }
        c.version += 1;
    }
    assert_eq!(pixel(&app, 230, 180).a(), 0, "l'autre côté de la cloison n'est plus rempli");
    assert!(is_red(pixel(&app, 170, 180)));
    app.undo();
    assert!(app.layers()[0].couleur.as_ref().is_some_and(|c| c.est_lie()), "annuler le relie");
}

#[test]
fn a_colour_layer_that_loses_its_line_keeps_its_scribbles_as_a_free_layer() {
    let (mut app, _) = app_griffonne("trait-perdu", 0.0, 8_150_100);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    app.layers_mut().remove(1);
    app.sync_couleurs();
    let c = app.layers()[0].couleur.clone().expect("le calque de couleur reste");
    assert!(!c.est_lie() && !c.griffonnages.is_empty(), "libre, griffonnages gardés");
    // Sans griffonnages, le même accident laisse un calque ordinaire.
    let (mut other, _) = app_griffonne("trait-perdu-2", 0.0, 8_150_200);
    other.layers_mut().remove(1);
    other.sync_couleurs();
    assert!(other.layers()[0].couleur.is_none(), "sans griffonnage : un calque bitmap ordinaire, comme avant");
}

#[test]
fn the_griffonner_command_is_one_undo_step_and_needs_a_colour_layer() {
    let (mut app, _) = app_griffonne("commande", 0.0, 8_150_300);
    let steps = undo_steps(&app);
    app.run_action("calque.griffonner");
    assert_eq!(undo_steps(&app), steps + 1);
    assert!(!app.layers()[0].couleur.as_ref().is_some_and(|c| c.griffonner));
    app.undo();
    assert!(app.layers()[0].couleur.as_ref().is_some_and(|c| c.griffonner));
    // Sur un calque ordinaire : un message, rien d'autre.
    app.cur_layer = 1;
    let steps = undo_steps(&app);
    app.run_action("calque.griffonner");
    assert_eq!(undo_steps(&app), steps);
    assert!(app.status.contains("calque de couleur"), "{}", app.status);
}

#[test]
fn the_old_propagation_entry_point_still_exists_and_shows_no_fill() {
    // `propager.rs` appelle encore `crate::ctg::input` pour ses images guides.
    let (app, _) = app_griffonne("ancienne-entree", 0.0, 8_150_400);
    assert_eq!(crate::ctg::input(app.layers(), 0, 0), None);
}
// ------------------------------------------------------------------ le recalcul quand le trait change

fn arc_of(doc: &Document, item: &PlanItem) -> Arc<Cel> {
    match fx::shown(doc, item).expect("remplissage") {
        fx::Shown::Fx(c) => c,
        fx::Shown::Plain(_) => panic!("le remplissage est un dessin dérivé"),
    }
}

/// Un document 64×64 : le carré de traits, des griffonnages rouges au centre ; l'élément du plan de leur remplissage.
fn doc_fill(base: u64) -> (Document, CelId, CelId, PlanItem) {
    let mut doc = Document::new(64, 64, 24);
    doc.reserve_id(base);
    let (lines, g) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    doc.cels.get_mut(&lines).expect("trait").raster = square();
    dot(&mut doc.cels.get_mut(&g).expect("griffonnage").raster, 25, 25, Color32::RED);
    for (k, id) in [lines, g].into_iter().enumerate() {
        doc.cels.get_mut(&id).expect("dessin").version = base + k as u64;
    }
    let item = PlanItem {
        cel: g,
        opacity: 255,
        tint: None,
        transform: None,
        blend: Default::default(),
        fx: Vec::new(),
        ctg: Some(GriffonInput { sources: vec![lines], gap: 0 }),
        fond: None,
        pre: None,
    };
    (doc, lines, g, item)
}

/// Attend que le remplissage affiché de `item` ne soit plus `old`.
fn wait_new(doc: &Document, item: &PlanItem, old: &Arc<Cel>) -> Arc<Cel> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        fx::griffon_poll();
        let now = fx::differe(|| arc_of(doc, item));
        if !Arc::ptr_eq(&now, old) {
            return now;
        }
        assert!(Instant::now() < deadline, "le remplissage n'est pas arrivé");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn on_screen_the_fill_comes_from_the_background_and_the_previous_one_stays_meanwhile() {
    monde::lire();
    let (mut doc, lines, g, item) = doc_fill(8_200_000);
    // Premier affichage : rien (jamais les griffonnages eux-mêmes, ni un calcul sur le fil de l'interface).
    let first = fx::differe(|| arc_of(&doc, &item));
    assert!(first.raster.is_blank(), "rien tant que le remplissage n'est pas calculé");
    let ready = wait_new(&doc, &item, &first);
    assert_eq!(ready.raster.get(20, 30), Color32::RED, "le remplissage de la zone");
    assert_eq!(ready.raster.get(5, 5), Color32::TRANSPARENT);
    // Le trait change (une cloison coupe le carré en deux) : l'écran garde le remplissage d'avant (le même dessin, sans calcul),
    // puis le nouveau arrive.
    for y in 11..40 {
        dot(&mut doc.cels.get_mut(&lines).expect("trait").raster, 22, y, Color32::BLACK);
    }
    doc.cels.get_mut(&lines).expect("trait").version += 1;
    let shown = fx::differe(|| arc_of(&doc, &item));
    assert!(Arc::ptr_eq(&shown, &ready), "le remplissage d'avant en attendant le nouveau");
    let newer = wait_new(&doc, &item, &ready);
    assert_eq!(newer.raster.get(15, 30), Color32::TRANSPARENT, "la moitié gauche n'a pas de griffonnage : plus de couleur");
    assert_eq!(newer.raster.get(30, 30), Color32::RED, "la moitié du griffonnage reste remplie");
    // Même résultat que le calcul exact (export, enregistrement).
    assert_eq!(newer.raster.to_flat(), arc_of(&doc, &item).raster.to_flat());
    // Un griffonnage de plus à gauche : la moitié gauche se remplit aussi.
    dot(&mut doc.cels.get_mut(&g).expect("griffonnage").raster, 15, 30, Color32::BLUE);
    doc.cels.get_mut(&g).expect("griffonnage").version += 1;
    let after_scribble = fx::differe(|| arc_of(&doc, &item));
    assert!(Arc::ptr_eq(&after_scribble, &newer));
    let last = wait_new(&doc, &item, &newer);
    assert_eq!(last.raster.get(15, 30), Color32::BLUE);
    assert_eq!(last.raster.get(30, 30), Color32::RED);
}

#[test]
fn the_exact_fill_is_computed_at_once_for_export_and_save() {
    monde::lire();
    let (doc, _, _, item) = doc_fill(8_200_100);
    // Hors de `differe` : le bon tout de suite (pas de remplissage d'avant, pas d'attente).
    let now = arc_of(&doc, &item);
    assert_eq!(now.raster.get(20, 30), Color32::RED);
}

#[test]
fn editing_the_line_layer_recomputes_the_fill_of_a_colour_layer() {
    monde::lire();
    let (mut app, a) = app_griffonne("recalcul", 0.0, 8_200_200);
    trace(&mut app, pos2(180.0, 150.0), pos2(190.0, 150.0));
    let plan = app.build_plan(0, false, false);
    let item = plan.items[0].clone();
    let first = fx::differe(|| arc_of(&app.doc, &item));
    let ready = wait_new(&app.doc, &item, &first);
    assert!(is_red(ready.raster.get(170, 180)), "toute la zone");
    // Une cloison verticale dans l'anneau : les deux moitiés sont séparées, seule celle du griffonnage reste rouge.
    {
        let c = app.doc.cels.get_mut(&a).expect("trait");
        for y in 90..210 {
            for x in 198..203 {
                put(&mut c.raster, x, y, Color32::BLACK);
            }
        }
        c.version += 1;
    }
    let plan = app.build_plan(0, false, false);
    let item = plan.items[0].clone();
    assert_eq!(item.ctg.as_ref().map(|i| i.sources.clone()), Some(vec![a]), "le trait est la source du remplissage");
    let shown = fx::differe(|| arc_of(&app.doc, &item));
    assert!(Arc::ptr_eq(&shown, &ready), "le remplissage d'avant en attendant");
    let newer = wait_new(&app.doc, &item, &ready);
    assert!(is_red(newer.raster.get(170, 180)), "le côté du griffonnage");
    assert_eq!(newer.raster.get(230, 180).a(), 0, "l'autre côté n'a plus de couleur");
}

/// Au repos, un calque de couleur qui remplit ne coûte rien : le remplissage arrive, puis SHOT ne demande plus d'image
/// (règle « sans latence » : pas de calcul qui se redemande à chaque image, pas de repeinture inutile).
#[test]
fn an_application_with_a_filling_colour_layer_goes_back_to_rest() {
    use crate::perf_tests::{banc, se_reposer};
    // (Le moindre calcul d'effet d'un autre essai réveille cette application : état commun pour elle seule.)
    monde::ecrire();
    let mut doc = Document::new(400, 300, 24);
    doc.reserve_id(8_400_000);
    let a = doc.add_cel(Cel::bitmap(ring(400, 300, (200.0, 150.0), 60.0, 0.0)));
    doc.cels.get_mut(&a).expect("trait").version = 8_400_000;
    let mut line = Layer::new("Trait", LayerKind::Bitmap);
    line.frames = vec![Some(a); 12];
    doc.panels[0].layers = vec![line, Layer::new("Autre", LayerKind::Vector)];
    doc.panels[0].length = 12;
    let mut b = banc(doc, false);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.run_action("calque.nouveau_couleur");
    b.app.run_action("calque.griffonner");
    b.app.tool = Tool::Brush;
    b.app.color = [255, 0, 0];
    trace(&mut b.app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT finit par se reposer avec un calque qui remplit");
    // Le remplissage est arrivé, et il ne se redemande pas.
    let plan = b.app.build_plan(0, false, false);
    assert!(plan.items[0].ctg.is_some());
    assert!(fx::ready(&b.app.doc, &plan.items[0], 0).is_some(), "le remplissage calculé en arrière-plan est là");
    let repos: Vec<_> = (0..8).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = repos.iter().map(|m| m.reveil).min().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "au repos, SHOT demande à être repeint dans {pire:?}");
    assert_eq!(crate::fxcache::pending(), 0, "plus aucun calcul de fond demandé");
}
// ------------------------------------------------------------------ copies, enregistrement

#[test]
fn duplicating_a_colour_layer_copies_its_scribbles_instead_of_sharing_them() {
    let (mut app, _) = app_griffonne("copie", 0.0, 8_300_000);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    let src = app.layers()[0].couleur.clone().expect("couleur");
    let before = app.layers().len();
    // Le plan entier dupliqué : les griffonnages sont copiés avec lui.
    let copy = app.doc.duplicate_panel(0);
    let dup = copy.layers.iter().find_map(|l| l.couleur.as_ref().filter(|c| c.remplissage)).expect("calque de couleur copié");
    assert_eq!(dup.griffonnages.len(), src.griffonnages.len());
    for (p, g) in &dup.griffonnages {
        assert!(!src.griffonnages.contains_key(p) && !src.griffonnages.values().any(|s| s == g), "ni la peinture ni les griffonnages ne sont partagés");
        assert!(app.doc.cels.contains_key(g));
    }
    let orig = *src.griffonnages.values().next().expect("griffonnage");
    let copied = *dup.griffonnages.values().next().expect("griffonnage copié");
    assert_eq!(app.doc.cels[&orig].raster.to_flat(), app.doc.cels[&copied].raster.to_flat(), "même contenu");
    // Un calque dupliqué seul (le trait et sa couleur ensemble).
    app.cur_layer = 1;
    app.run_action("calque.dupliquer");
    assert!(app.layers().len() > before);
    let copies: Vec<&CalqueCouleur> = app.layers().iter().filter_map(|l| l.couleur.as_ref()).filter(|c| c.remplissage).collect();
    assert_eq!(copies.len(), 2);
    let ids: std::collections::BTreeSet<CelId> = copies.iter().flat_map(|c| c.griffonnages.values().copied()).collect();
    assert_eq!(ids.len(), 2, "deux dessins de griffonnages distincts");
}

#[test]
fn copying_and_pasting_a_line_layer_brings_the_scribbles_of_its_colour_layer() {
    let (mut app, _) = app_griffonne("coller", 0.0, 8_300_300);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    app.cur_layer = 1;
    app.run_action("calque.copier");
    app.run_action("calque.coller");
    let fills: Vec<&CalqueCouleur> = app.layers().iter().filter_map(|l| l.couleur.as_ref()).filter(|c| c.remplissage).collect();
    assert_eq!(fills.len(), 2, "le calque de couleur est revenu avec le trait collé");
    let ids: std::collections::BTreeSet<CelId> = fills.iter().flat_map(|c| c.griffonnages.values().copied()).collect();
    assert_eq!(ids.len(), 2, "des griffonnages à lui, pas ceux de l'original");
    assert!(ids.iter().all(|g| app.doc.cels.get(g).is_some_and(|c| !c.is_empty())), "avec leur contenu");
}
#[test]
fn a_copied_line_drawing_takes_a_copy_of_the_scribbles_of_its_colour_drawing() {
    let (mut app, a) = app_griffonne("copie-dessin", 0.0, 8_300_400);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    let copy = app.doc.duplicate_cel(a);
    crate::calquecouleur::noter_copie(&mut app.doc, 0, 1, a, copy);
    let c = app.layers()[0].couleur.clone().expect("couleur");
    let (paint, fresh) = (c.dessins[&a], c.dessins[&copy]);
    assert_ne!(paint, fresh, "la copie a sa peinture");
    let (g, g2) = (c.griffonnages[&paint], c.griffonnages[&fresh]);
    assert_ne!(g, g2, "et ses griffonnages, pas ceux de l'original");
    assert_eq!(app.doc.cels[&g].raster.to_flat(), app.doc.cels[&g2].raster.to_flat(), "le même contenu au départ");
}
#[test]
fn scribbles_are_saved_with_the_project_and_come_back_the_same() {
    let (mut app, _) = app_griffonne("enregistre", 0.0, 8_300_100);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    let dir = dossier("griffonnage-enregistre");
    let manifeste = dir.join("G.pelure");
    crate::project::save(&app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let m = lire_json(&manifeste);
    assert_eq!(m["version"], crate::project::VERSION, "le format courant");
    let loaded = crate::project::load(&manifeste).expect("lecture");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let back = loaded.doc;
    let c = back.panels[0].layers[0].couleur.clone().expect("calque de couleur");
    assert!(c.remplissage && c.griffonner && c.est_lie());
    assert_eq!(c.griffonnages.len(), 1);
    let (_, g) = c.griffonnages.iter().next().map(|(p, g)| (*p, *g)).expect("couple");
    let old = *app.layers()[0].couleur.as_ref().expect("c").griffonnages.values().next().expect("g");
    assert_eq!(back.cels[&g].raster.to_flat(), app.doc.cels[&old].raster.to_flat(), "les griffonnages sont revenus tels quels");
    // Le remplissage relu est le même.
    let plan_a = render::panel_plan_at(&app.doc, 0, 0, None);
    let plan_b = render::panel_plan_at(&back, 0, 0, None);
    assert_eq!(render::composite_full(&app.doc, &plan_a), render::composite_full(&back, &plan_b));
    // Le fichier du dessin de griffonnages est un dessin ordinaire du projet.
    assert!(dir.join("dessins").join(format!("{g}.png")).exists());
}

#[test]
fn used_drawings_include_the_scribbles_of_every_colour_layer() {
    let (mut app, _) = app_griffonne("utilises", 0.0, 8_300_200);
    trace(&mut app, pos2(180.0, 150.0), pos2(220.0, 150.0));
    let g = *app.layers()[0].couleur.as_ref().expect("c").griffonnages.values().next().expect("g");
    assert!(app.doc.used_cels().contains(&g), "enregistré avec le projet");
    assert!(crate::empreinte::dessins_du_plan(&app.doc.panels[0]).contains(&g), "compté dans les dessins du plan (comparaison, envois, projet partagé)");
}

// ------------------------------------------------------------------ les calques CTG des projets d'avant

/// Les images du calque « Aplats » du projet d'essai dont on compare l'image à celle de l'ancien calque CTG.
const IMAGES_CTG: [usize; 6] = [0, 3, 7, 12, 18, 23];

fn empreinte_du_calque(doc: &Document, li: usize, frame: usize) -> u64 {
    let plan = render::layer_plan_at(doc, 0, li, frame).expect("plan du calque");
    let px = render::composite_full(doc, &plan);
    let octets: Vec<u8> = px.iter().flat_map(|c| c.to_array()).collect();
    empreinte(&octets)
}

#[test]
fn an_old_project_with_a_ctg_layer_opens_with_the_same_image_and_stays_editable() {
    let (f, manifeste) = projet_enregistre("ctg-ancien");
    ajouter_heritage_ctg(&manifeste);
    let m = lire_json(&manifeste);
    assert_eq!(m["version"], 35, "un projet d'avant");
    let calques = &m["sequences"][0]["scenes"][0]["panneaux"][0]["calques"];
    assert!(calques.as_array().expect("calques").iter().any(|c| c.get("ctg").is_some()), "l'ancien champ ctg est là");
    let loaded = crate::project::load(&manifeste).expect("lecture sans erreur");
    assert!(loaded.warnings.iter().any(|w| w.contains("CTG")), "la conversion est annoncée : {:?}", loaded.warnings);
    let doc = loaded.doc;
    let layers = &doc.panels[0].layers;
    let li = layers.iter().position(|l| l.name == "Aplats").expect("le calque Aplats");
    let l = &layers[li];
    // Un calque de couleur libre (sa source, `perso`, n'est pas juste au-dessus), qui garde ses griffonnages et ses sources.
    let c = l.couleur.as_ref().expect("devenu un calque de couleur");
    assert!(!c.est_lie() && c.remplissage && c.griffonner, "libre, avec son remplissage, en mode Griffonner comme avant");
    assert_eq!(c.trous, 6, "la fermeture des trous du calque CTG");
    let perso = layers.iter().find(|l| l.name == "Personnage").map(|l| l.id).expect("perso");
    assert_eq!(c.sources, vec![perso], "la source du calque CTG devient une source supplémentaire");
    assert_eq!(c.griffonnages.len(), 1);
    assert!(l.frames.iter().flatten().all(|p| doc.cels[p].is_empty() && c.griffonnages.contains_key(p)), "des dessins de peinture vides, un par dessin de griffonnages");
    assert_eq!(l.kind, LayerKind::Bitmap);
    assert!(!l.visible && l.locked, "tout le reste du calque est tel qu'il était");
    // La même image que l'ancien calque CTG, pixel pour pixel.
    for (k, frame) in IMAGES_CTG.into_iter().enumerate() {
        assert_eq!(format!("{:016x}", empreinte_du_calque(&doc, li, frame)), GOLDEN_CTG[k], "image {frame} du calque Aplats");
    }
    // Les griffonnages d'avant sont les fichiers d'avant (rien de réécrit, rien d'effacé) : le dessin de griffonnages est le
    // dessin que le calque CTG montrait.
    let g = *c.griffonnages.values().next().expect("griffonnage");
    let original = f.doc.panels[0].layers.iter().find(|l| l.name == "Aplats").and_then(|l| l.couleur.as_ref()).and_then(|c| c.griffonnages.values().next().copied()).expect("griffonnage d'origine");
    assert_eq!(g, original, "même dessin, même fichier");
    assert_eq!(doc.cels[&g].raster.to_flat(), f.doc.cels[&original].raster.to_flat());
    // Il reste modifiable : on griffonne dessus, c'est un griffonnage de plus (la peinture libre n'est pas touchée).
    let mut app = app("ctg-modifiable");
    app.doc = doc;
    app.doc.storyboard_locked = false;
    app.cur_panel = 0;
    app.cur_layer = li;
    app.cur_frame = 0;
    app.layers_mut()[li].locked = false;
    app.layers_mut()[li].visible = true;
    app.tool = Tool::Brush;
    app.color = [10, 200, 30];
    let before = app.doc.cels[&g].raster.to_flat();
    trace(&mut app, pos2(100.0, 100.0), pos2(140.0, 100.0));
    assert!(app.doc.cels[&g].raster.to_flat() != before, "le trait est dans les griffonnages ({})", app.status);
    let paint = app.layers()[li].shown_at(0).expect("peinture");
    assert!(app.doc.cels[&paint].is_empty(), "la peinture libre n'a pas bougé");
    app.undo();
    assert_eq!(app.doc.cels[&g].raster.to_flat(), before, "une étape d'annulation");
}

#[test]
fn a_ctg_layer_right_under_its_source_with_matching_images_becomes_a_linked_colour_layer() {
    // Un projet « d'avant » : un calque de traits (trois dessins) et, juste dessous, un calque CTG aux images qui se
    // recouvrent exactement.
    let mut doc = Document::new(64, 64, 24);
    doc.reserve_id(8_500_000);
    let (l1, l2, g1, g2) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    doc.cels.get_mut(&l1).expect("trait").raster = square();
    let mut moved = Raster::new(64, 64);
    for i in 20..=50 {
        for t in 0..2 {
            for (x, y) in [(i, 20 + t), (i, 50 - t), (20 + t, i), (50 - t, i)] {
                dot(&mut moved, x, y, Color32::BLACK);
            }
        }
    }
    doc.cels.get_mut(&l2).expect("trait").raster = moved;
    dot(&mut doc.cels.get_mut(&g1).expect("graffiti").raster, 25, 25, Color32::RED);
    dot(&mut doc.cels.get_mut(&g2).expect("graffiti").raster, 30, 30, Color32::BLUE);
    let mut lines = Layer::new("Traits", LayerKind::Bitmap);
    lines.frames = [vec![Some(l1); 3], vec![None; 1], vec![Some(l2); 2]].concat();
    let mut ctg = Layer::new("Couleurs", LayerKind::Bitmap);
    ctg.frames = [vec![Some(g1); 3], vec![None; 1], vec![Some(g2); 2]].concat();
    doc.panels[0].layers = vec![ctg, lines];
    doc.panels[0].length = 6;
    let expected: Vec<Vec<Color32>> = [(l1, g1), (l2, g2)].into_iter().map(|(l, g)| griffonnage::fill(&[&doc.cels[&l].raster], &doc.cels[&g].raster, 4).to_flat()).collect();
    let dir = dossier("ctg-lie");
    let manifeste = dir.join("C.pelure");
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let mut m = lire_json(&manifeste);
    m["version"] = 35.into();
    m["sequences"][0]["scenes"][0]["panneaux"][0]["calques"][0]["ctg"] = serde_json::json!({"sources": [1], "trou": 4});
    ecrire_json(&manifeste, &m);
    let loaded = crate::project::load(&manifeste).expect("lecture");
    let back = loaded.doc;
    let (colour, line) = (&back.panels[0].layers[0], &back.panels[0].layers[1]);
    let c = colour.couleur.as_ref().expect("calque de couleur");
    assert!(c.est_lie() && c.trait_id == line.id, "lié à son premier calque source, qui est juste au-dessus");
    assert!(c.sources.is_empty(), "son trait est sa seule source");
    assert_eq!(c.dessins.len(), 2, "un dessin de peinture par dessin du trait");
    assert_eq!(c.griffonnages.len(), 2);
    assert_eq!(crate::calquecouleur::trait_de(&back.panels[0].layers, 0), Some(1));
    assert_eq!(colour.frames.iter().map(Option::is_some).collect::<Vec<_>>(), line.frames.iter().map(Option::is_some).collect::<Vec<_>>(), "il suit les images du trait");
    // La même image qu'avant, et le même calcul que l'ancien remplissage.
    for (frame, want) in [(0, 0), (2, 0), (4, 1), (5, 1)] {
        let plan = render::layer_plan_at(&back, 0, 0, frame).expect("plan");
        let got = render::composite_full(&back, &plan);
        assert_eq!(got, expected[want], "image {frame}");
    }
    assert!(render::layer_plan_at(&back, 0, 0, 3).is_none(), "pas de dessin : rien, comme avant");
}

#[test]
fn nothing_ctg_remains_in_the_commands() {
    for a in crate::keymap::ACTIONS {
        assert!(!a.id.contains("ctg"), "action {} : plus de calque CTG", a.id);
        assert!(!a.label.contains("CTG"), "action {} : {}", a.id, a.label);
    }
    let toml = include_str!("../keymap.toml");
    assert!(!toml.contains("ctg"), "keymap.toml parle encore de CTG");
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == "calque.griffonner"));
    // Une action CTG qu'une vieille configuration citerait n'est plus connue : rien ne se passe.
    let mut app = app("sans-ctg");
    let layers = app.layers().len();
    app.run_action("calque.nouveau_ctg");
    assert_eq!(app.layers().len(), layers, "plus de nouveau calque CTG");
}
