//! L'outil Dégradé (`degrade.rs`, `degradeui.rs`) : linéaire et radial, vers le fond ou vers le transparent, dans la
//! sélection, dans la zone du pot de peinture, sur toute la feuille ; la zone vectorielle en dégradé (rendu, déplacement,
//! enregistrement, vieux projets) ; l'aplat ; une seule étape d'annulation ; rien du moteur de pinceaux pendant le glisser.

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::app::{App, Mode, Tool};
use crate::degrade::{Degrade, Fin, Genre, Portee, Reglages};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::pixsel::PixelMask;
use crate::project;
use crate::raster::{Raster, TILE};
use crate::vector::{Affine, Element};

pub(crate) const ROUGE: [u8; 3] = [255, 0, 0];
pub(crate) const BLEU: [u8; 3] = [0, 0, 255];

fn proche(a: u8, b: i32, tol: i32) -> bool {
    (a as i32 - b).abs() <= tol
}

pub(crate) fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

// ------------------------------------------------------------------------------------ le noyau

#[test]
fn a_linear_gradient_has_the_front_colour_at_the_start_the_mix_in_the_middle_and_the_back_colour_at_the_end() {
    let g = Reglages::default().degrade(ROUGE, BLEU, pos2(100.0, 0.0), pos2(300.0, 0.0));
    let mut r = Raster::new(400, 40);
    let tiles = crate::degrade::peindre(&mut r, &Portee::Tout, &g, false, None);
    assert!(!tiles.is_empty());
    // Avant le départ : la couleur de départ, tout opaque.
    for x in [0, 50, 99] {
        let p = r.get(x, 7);
        assert_eq!((p.r(), p.g(), p.b(), p.a()), (255, 0, 0, 255), "x = {x}");
    }
    // Au départ à peine entamé, au milieu à moitié, à l'arrivée complètement.
    let p = r.get(100, 7);
    assert!(proche(p.r(), 254, 2) && proche(p.b(), 1, 2), "départ {p:?}");
    let p = r.get(200, 7);
    assert!(proche(p.r(), 127, 3) && proche(p.b(), 128, 3) && p.g() == 0 && p.a() == 255, "milieu {p:?}");
    let p = r.get(299, 7);
    assert!(proche(p.r(), 1, 2) && proche(p.b(), 254, 2), "arrivée {p:?}");
    for x in [300, 350, 399] {
        let p = r.get(x, 7);
        assert_eq!((p.r(), p.g(), p.b(), p.a()), (0, 0, 255, 255), "après l'arrivée, x = {x}");
    }
    // La couleur ne dépend que de la position le long de l'axe : même colonne, même couleur.
    assert_eq!(r.get(200, 0), r.get(200, 39));
    // Un axe oblique : la couleur est celle de la projection du pixel sur l'axe.
    let g = Reglages::default().degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(100.0, 100.0));
    assert!((g.parametre(50.0, 50.0) - 0.5).abs() < 1e-5 && (g.parametre(100.0, 0.0) - 0.5).abs() < 1e-5 && g.parametre(-10.0, -10.0) == 0.0 && g.parametre(500.0, 500.0) == 1.0);
}

#[test]
fn a_radial_gradient_goes_from_the_centre_to_the_circle() {
    let g = Reglages { genre: Genre::Radial, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(200.0, 150.0), pos2(300.0, 150.0));
    let mut r = Raster::new(400, 300);
    crate::degrade::peindre(&mut r, &Portee::Tout, &g, false, None);
    let p = r.get(200, 150);
    assert!(proche(p.r(), 254, 3) && proche(p.b(), 1, 3), "centre {p:?}");
    let p = r.get(250, 150);
    assert!(proche(p.r(), 127, 3) && proche(p.b(), 128, 3), "mi-rayon {p:?}");
    for (x, y) in [(300, 150), (320, 150), (200, 260), (10, 10)] {
        let p = r.get(x, y);
        assert_eq!((p.r(), p.b(), p.a()), (0, 255, 255), "au-delà du cercle, ({x}, {y})");
    }
    // Le même rayon donne la même couleur, dans tous les sens (à un cheveu près : le centre est à mi-pixel).
    let (a, b, c, d) = (r.get(200, 100), r.get(200, 200), r.get(150, 150), r.get(250, 150));
    assert!(proche(a.r(), c.r() as i32, 3) && proche(a.r(), d.r() as i32, 3), "haut, gauche, droite : {a:?} {c:?} {d:?}");
    assert!(proche(b.r(), c.r() as i32, 3), "bas : {b:?}");
}

#[test]
fn towards_transparent_is_an_alpha_ramp_of_the_front_colour() {
    let reglages = Reglages { fin: Fin::VersTransparent, ..Reglages::default() };
    let g = reglages.degrade(ROUGE, BLEU, pos2(100.0, 0.0), pos2(300.0, 0.0));
    let mut r = Raster::new(400, 10);
    crate::degrade::peindre(&mut r, &Portee::Tout, &g, false, None);
    let a = |x: usize| r.get(x, 3).a() as i32;
    assert_eq!(a(50), 255, "plein au départ");
    assert!((a(200) - 128).abs() <= 3, "à moitié au milieu : {}", a(200));
    assert_eq!(a(350), 0, "rien à l'arrivée");
    assert!(a(120) > a(160) && a(160) > a(200) && a(200) > a(240) && a(240) > a(280), "la rampe descend");
    // Toujours la couleur d'avant-plan : un rouge de plus en plus transparent (prémultiplié : r = alpha).
    for x in [50, 150, 200, 250] {
        let p = r.get(x, 3);
        assert!(proche(p.r(), p.a() as i32, 2) && p.g() == 0 && p.b() == 0, "x = {x} : {p:?}");
    }
    // L'opacité du réglage multiplie le tout ; inverser échange les bouts.
    let demi = Reglages { opacite: 0.5, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(100.0, 0.0));
    assert!((demi.a0 - 0.5).abs() < 1e-6 && (demi.a1 - 0.5).abs() < 1e-6);
    let inverse = Reglages { inverse: true, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(100.0, 0.0));
    assert_eq!((inverse.c0, inverse.c1), (BLEU, ROUGE));
    let inv_transp = Reglages { inverse: true, fin: Fin::VersTransparent, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(100.0, 0.0));
    assert_eq!((inv_transp.a0, inv_transp.a1), (0.0, 1.0), "inversé : il apparaît");
}

#[test]
fn a_selection_clips_the_gradient_with_an_antialiased_edge() {
    let g = Reglages::default().degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(200.0, 0.0));
    let mut r = Raster::new(200, 100);
    // Un rectangle dont le bord gauche coupe le pixel 50 en deux.
    let poly = vec![pos2(50.5, 20.0), pos2(150.0, 20.0), pos2(150.0, 60.0), pos2(50.5, 60.0)];
    let mask = PixelMask::from_polygons(&[poly], 200, 100).expect("une zone");
    crate::degrade::peindre(&mut r, &Portee::Masque(&mask), &g, false, None);
    assert_eq!(r.get(10, 30).a(), 0, "dehors à gauche");
    assert_eq!(r.get(170, 30).a(), 0, "dehors à droite");
    assert_eq!(r.get(100, 10).a(), 0, "dehors en haut");
    assert_eq!(r.get(100, 70).a(), 0, "dehors en bas");
    assert_eq!(r.get(100, 30).a(), 255, "dedans");
    let bord = r.get(50, 30).a();
    assert!((100..=160).contains(&bord), "le bord est anticrénelé : {bord}");
    // Sur un calque qui préserve la transparence, rien ne s'écrit où il n'y a rien.
    let mut vide = Raster::new(200, 100);
    crate::degrade::peindre(&mut vide, &Portee::Masque(&mask), &g, true, None);
    assert!(vide.is_blank(), "le vide reste vide");
}

#[test]
fn a_zone_replaces_inside_and_passes_under_the_line_in_the_edge() {
    let g = Reglages::default().degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(100.0, 0.0));
    let (w, h) = (100usize, 50usize);
    let mut r = Raster::new(w, h);
    // Un trait noir en (30, 20) : dans le débord, le dégradé passe dessous.
    put(&mut r, 30, 20, Color32::BLACK);
    put(&mut r, 20, 20, Color32::from_rgb(9, 9, 9));
    let mut region = vec![false; w * h];
    for y in 10..30 {
        for x in 10..29 {
            region[y * w + x] = true;
        }
    }
    let grown = crate::fill::dilate(&region, w, h, 2);
    crate::degrade::peindre(&mut r, &Portee::Zone { region: &region, grown: &grown }, &g, false, None);
    assert_eq!(r.get(30, 20), Color32::BLACK, "le trait n'est pas abîmé");
    assert!(r.get(29, 22).a() == 255 && r.get(31, 22).a() == 0, "le débord va jusqu'à 2 px : {:?} {:?}", r.get(29, 22), r.get(31, 22));
    // Dans la zone, la couleur remplace l'existant (même un gris foncé presque noir).
    assert!(r.get(20, 20).a() == 255 && r.get(20, 20).r() > 200, "{:?}", r.get(20, 20));
    assert_eq!(r.get(5, 5).a(), 0, "hors de la zone, rien");
}

// ------------------------------------------------------------------------------------ l'appli

pub(crate) fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-degrade-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let debut = std::time::Instant::now();
    while app.brushes_job.is_some() && debut.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(2));
        app.poll_brushes();
    }
    app
}

/// Un plan 400 × 300 à un calque de cette sorte, à l'écran (1280 × 720), outil Pinceau noir ; avant-plan rouge, arrière-plan
/// bleu pour le dégradé.
pub(crate) fn banc_degrade(kind: LayerKind) -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", kind)];
    doc.panels[0].length = 4;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.tool = Tool::Brush;
    b.app.color = [0, 0, 0];
    b.app.brush.size = 8.0;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

pub(crate) fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn bouton(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers }
}

/// Un glisser du pointeur sur les points `pts` de la feuille, une image d'écran par point, avec ces touches tenues.
pub(crate) fn glisser(b: &mut Banc, pts: &[(f32, f32)], touches: Modifiers) {
    b.touches = touches;
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, touches)], 1.0 / 60.0);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
    let fin = ecran(b, pts[pts.len() - 1].0, pts[pts.len() - 1].1);
    b.image(vec![bouton(fin, false, touches)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un anneau carré de (100, 100) à (300, 200), tracé au pinceau en noir.
pub(crate) fn tracer_anneau(b: &mut Banc) {
    b.app.set_tool(Tool::Brush);
    b.app.color = [0, 0, 0];
    glisser(b, &[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0), (300.0, 150.0), (300.0, 200.0), (200.0, 200.0), (100.0, 200.0), (100.0, 150.0), (100.0, 100.0)], Modifiers::NONE);
    b.app.set_tool(Tool::Gradient);
    b.app.color = ROUGE;
    b.app.couleurs.prefs.back = BLEU;
}

pub(crate) fn cel_du_calque(b: &Banc) -> CelId {
    b.app.layers()[0].frames[0].expect("un dessin")
}

pub(crate) fn pixel(b: &Banc, x: usize, y: usize) -> Color32 {
    match b.app.layers()[0].frames.first().copied().flatten() {
        Some(cel) => b.app.doc.cels[&cel].raster.get(x, y),
        None => Color32::TRANSPARENT,
    }
}

pub(crate) fn etapes(app: &App) -> Vec<String> {
    app.history.names().0.iter().map(|n| n.to_string()).collect()
}

#[test]
fn dragging_in_a_closed_region_fills_that_region_with_the_gradient_as_the_fill_tool_finds_it() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    let avant = etapes(&b.app).len();
    // La zone est celle du pot de peinture : même recherche, même tolérance, même débord (cherchée avant le geste).
    let (w, h) = (b.app.doc.width, b.app.doc.height);
    let source = b.app.fill_source();
    let tol = (b.app.fill.tolerance * 255.0).round() as u8;
    let region = crate::fill::flood(&source, w, h, 150, 150, tol);
    let grown = crate::fill::dilate(&region, w, h, b.app.fill.expand);
    assert!(region.iter().filter(|&&r| r).count() > 5_000);
    glisser(&mut b, &[(120.0, 150.0), (280.0, 150.0)], Modifiers::NONE);
    let cel = cel_du_calque(&b);
    let raster = &b.app.doc.cels[&cel].raster;
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            if region[k] {
                assert_eq!(raster.get(x, y).a(), 255, "toute la zone du pot est remplie, ({x}, {y})");
            } else if !grown[k] {
                assert_eq!(raster.get(x, y), source[k], "rien ne change hors de la zone et de son débord, ({x}, {y})");
            }
        }
    }
    let (g, d) = (raster.get(124, 150), raster.get(276, 150));
    assert!(g.r() > 240 && g.b() < 15, "le côté du départ est rouge : {g:?}");
    assert!(d.b() > 240 && d.r() < 15, "le côté de l'arrivée est bleu : {d:?}");
    let m = raster.get(200, 150);
    assert!(proche(m.r(), 128, 12) && proche(m.b(), 127, 12), "au milieu de l'axe : {m:?}");
    assert_eq!(raster.get(50, 150).a(), 0, "hors de l'anneau, rien");
    assert_eq!(raster.get(200, 50).a(), 0, "hors de l'anneau, rien");
    assert_eq!(raster.get(200, 100).a(), 255, "le trait reste");
    // Une seule étape d'annulation, nommée.
    assert_eq!(etapes(&b.app).len(), avant + 1);
    assert_eq!(etapes(&b.app).last().map(String::as_str), Some("Dégradé"));
    b.app.undo();
    let raster = &b.app.doc.cels[&cel_du_calque(&b)].raster;
    assert_eq!(raster.get(124, 150).a(), 0, "annulé d'un coup : l'intérieur est de nouveau vide");
    assert_eq!(raster.get(200, 100).a(), 255, "le trait est toujours là");
}

#[test]
fn a_plain_click_does_nothing_and_alt_fills_the_whole_layer() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    let avant = etapes(&b.app).len();
    glisser(&mut b, &[(150.0, 150.0)], Modifiers::NONE);
    assert_eq!(etapes(&b.app).len(), avant, "un clic seul ne pose rien");
    assert_eq!(pixel(&b, 150, 150).a(), 0);
    // Alt : toute la feuille, hors de l'anneau aussi (et l'Alt du dégradé n'est pas la pipette : la couleur n'a pas bougé).
    glisser(&mut b, &[(20.0, 20.0), (380.0, 20.0)], Modifiers::ALT);
    assert_eq!(b.app.color, ROUGE, "Alt ne prend pas de couleur avec cet outil");
    for (x, y) in [(5, 5), (60, 250), (200, 150), (395, 295)] {
        assert_eq!(pixel(&b, x, y).a(), 255, "({x}, {y})");
    }
    let (a, c) = (pixel(&b, 30, 250), pixel(&b, 370, 250));
    assert!(a.r() > c.r() && c.b() > a.b(), "rouge à gauche, bleu à droite : {a:?} {c:?}");
    assert_eq!(etapes(&b.app).len(), avant + 1);
    b.app.undo();
    assert_eq!(pixel(&b, 5, 5).a(), 0);
}

#[test]
fn a_selection_is_filled_alone() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    let (cel, _) = b.app.ensure_cel();
    let poly = vec![pos2(40.0, 40.5), pos2(160.0, 40.5), pos2(160.0, 90.0), pos2(40.0, 90.0)];
    let mask = PixelMask::from_polygons(&[poly], 400, 300).expect("une zone");
    b.app.bsel = Some(crate::pixtools::BSelection { cel, mask, outline: Vec::new(), lifted: None });
    // (La zone reste avec l'outil Dégradé.)
    b.chauffer(2);
    assert!(b.app.bsel.is_some(), "la sélection reste avec le dégradé");
    let avant = etapes(&b.app).len();
    glisser(&mut b, &[(50.0, 60.0), (150.0, 60.0)], Modifiers::NONE);
    assert_eq!(pixel(&b, 100, 60).a(), 255, "dans la sélection");
    assert!(pixel(&b, 45, 60).r() > pixel(&b, 155, 60).r(), "du rouge vers le bleu");
    assert_eq!(pixel(&b, 20, 60).a(), 0, "hors de la sélection");
    assert_eq!(pixel(&b, 200, 150).a(), 0, "la zone fermée de l'anneau n'est pas remplie : la sélection l'emporte");
    assert!((100..=160).contains(&(pixel(&b, 100, 40).a() as i32)), "bord anticrénelé : {}", pixel(&b, 100, 40).a());
    assert_eq!(etapes(&b.app).len(), avant + 1);
}

#[test]
fn the_preview_never_runs_the_brush_engine_and_writes_nothing_before_the_release() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    let (cel, trait_seul) = (cel_du_calque(&b), b.app.doc.cels[&cel_du_calque(&b)].raster.clone());
    let rejeux = crate::vector::replay_count();
    let p0 = ecran(&b, 120.0, 150.0);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    assert!(b.app.degrade_geste.is_some(), "le geste est en cours");
    for k in 0..30 {
        let p = ecran(&b, 120.0 + 5.0 * k as f32, 150.0);
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
        assert!(b.app.stroke.is_none() && b.app.vstroke.is_none(), "aucun trait de pinceau pendant le glisser");
    }
    assert_eq!(crate::vector::replay_count(), rejeux, "le moteur de pinceaux n'a pas rejoué un seul trait pendant le glisser");
    assert!(b.app.doc.cels[&cel].raster.tiles == trait_seul.tiles, "rien n'est écrit avant le lâcher : l'aperçu est à part");
    assert_eq!(b.app.drawing(cel).map(|d| d.elements.len()), Some(1), "le dessin n'a pas changé");
    // Le lâcher : une zone en dégradé, sans rejouer de trait non plus.
    let fin = ecran(&b, 265.0, 150.0);
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.degrade_geste.is_none());
    assert_eq!(b.app.drawing(cel).map(|d| d.elements.len()), Some(2));
    assert_eq!(crate::vector::replay_count(), rejeux, "pas de rejeu au lâcher non plus : les traits ont leurs pixels en cache");
}

// ------------------------------------------------------------------------------------ vectoriel

#[test]
fn on_a_vector_layer_the_zone_keeps_a_gradient_instead_of_pixels() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    let cel = cel_du_calque(&b);
    let avant = etapes(&b.app).len();
    glisser(&mut b, &[(120.0, 150.0), (280.0, 150.0)], Modifiers::NONE);
    let d = b.app.drawing(cel).expect("un dessin vectoriel");
    assert_eq!(d.elements.len(), 2, "le trait et une zone");
    let Element::Fill { gradient: Some(g), color, .. } = &d.elements[0] else { panic!("la zone passe sous le trait : {:?}", d.elements[0]) };
    assert_eq!((g.genre, g.c0, g.c1), (Genre::Lineaire, ROUGE, BLEU));
    assert_eq!(*color, ROUGE);
    assert!((g.depart[0] - 120.0).abs() < 1.0 && (g.arrivee[0] - 280.0).abs() < 1.0, "l'axe tiré : {g:?}");
    // Rendue par le rendu vectoriel : rouge au départ, bleu à l'arrivée, le trait par-dessus.
    let raster = &b.app.doc.cels[&cel].raster;
    let (a, c, m) = (raster.get(124, 150), raster.get(276, 150), raster.get(200, 150));
    assert!(a.r() > 235 && a.b() < 20 && a.a() == 255, "{a:?}");
    assert!(c.b() > 235 && c.r() < 20 && c.a() == 255, "{c:?}");
    assert!(proche(m.r(), 128, 14) && proche(m.b(), 127, 14), "{m:?}");
    assert_eq!(raster.get(50, 150).a(), 0, "hors de la zone");
    // Une seule étape d'annulation ; annuler retire la zone.
    assert_eq!(etapes(&b.app).len(), avant + 1);
    b.app.undo();
    assert_eq!(b.app.drawing(cel).map(|d| d.elements.len()), Some(1));
    assert_eq!(b.app.doc.cels[&cel].raster.get(124, 150).a(), 0);
    b.app.redo();
    assert_eq!(b.app.drawing(cel).map(|d| d.elements.len()), Some(2), "rétablir remet la zone");
}

#[test]
fn a_gradient_zone_moves_with_its_shape_and_the_axis_follows() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    let cel = cel_du_calque(&b);
    glisser(&mut b, &[(120.0, 150.0), (280.0, 150.0)], Modifiers::NONE);
    // Déplacer la zone choisie (les flèches) : l'axe la suit.
    b.app.vsel = Some(crate::vtools::VSelection { cel, items: vec![0] });
    b.app.nudge_selection(vec2(30.0, 10.0));
    let Element::Fill { gradient: Some(g), contours, .. } = &b.app.drawing(cel).unwrap().elements[0] else { panic!() };
    assert!((g.depart[0] - 150.0).abs() < 0.01 && (g.depart[1] - 160.0).abs() < 0.01 && (g.arrivee[0] - 310.0).abs() < 0.01, "{g:?}");
    assert!(contours[0].iter().all(|p| p[0] >= 100.0 + 30.0 - 1.0), "la forme a bougé de la même façon");
    // Au pixel : le rouge est au départ déplacé, le bleu à l'arrivée déplacée.
    let raster = &b.app.doc.cels[&cel].raster;
    assert!(raster.get(158, 160).r() > 235, "{:?}", raster.get(158, 160));
    // Transformations de l'élément : déplacement, échelle, rotation, déformation par un point.
    let e = b.app.drawing(cel).unwrap().elements[0].clone();
    let g0 = *e.gradient().unwrap();
    let t = e.transformed(&Affine::translate(vec2(-50.0, 5.0)));
    let g1 = *t.gradient().unwrap();
    assert!((g1.depart[0] - (g0.depart[0] - 50.0)).abs() < 1e-3 && (g1.arrivee[1] - (g0.arrivee[1] + 5.0)).abs() < 1e-3);
    let s = Affine { a: 2.0, b: 0.0, c: 0.0, d: 2.0, tx: 0.0, ty: 0.0 };
    let g2 = *e.transformed(&s).gradient().unwrap();
    assert!((g2.depart[0] - 2.0 * g0.depart[0]).abs() < 1e-3 && (g2.arrivee[0] - 2.0 * g0.arrivee[0]).abs() < 1e-3, "le rayon suit l'échelle");
    // La forme déformée en enveloppe ou en perspective (`warp_element_with`) déplace l'axe comme ses points.
    let decale = |p: Pos2| pos2(p.x + 7.0, p.y * 1.5);
    let w = crate::persp::warp_element_with(&e, &decale, 1.0);
    let gw = *w.gradient().unwrap();
    assert!((gw.depart[0] - (g0.depart[0] + 7.0)).abs() < 1e-3 && (gw.depart[1] - g0.depart[1] * 1.5).abs() < 1e-3);
    // Choisir une couleur pour la zone la remet unie.
    let mut unie = e.clone();
    unie.set_color([1, 2, 3]);
    assert!(unie.gradient().is_none());
}

#[test]
fn a_selected_filled_zone_receives_the_gradient_without_adding_one() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    let cel = cel_du_calque(&b);
    // Le pot de peinture remplit la zone d'une couleur unie ; on la choisit ; le dégradé la change en place.
    b.app.set_tool(Tool::Fill);
    b.app.color = [0, 200, 0];
    glisser(&mut b, &[(150.0, 150.0)], Modifiers::NONE);
    assert!(matches!(b.app.drawing(cel).unwrap().elements[0], Element::Fill { gradient: None, .. }));
    let n = b.app.drawing(cel).unwrap().elements.len();
    b.app.set_tool(Tool::Gradient);
    b.app.color = ROUGE;
    b.app.vsel = Some(crate::vtools::VSelection { cel, items: vec![0] });
    glisser(&mut b, &[(120.0, 150.0), (280.0, 150.0)], Modifiers::NONE);
    let d = b.app.drawing(cel).unwrap();
    assert_eq!(d.elements.len(), n, "pas de zone de plus");
    assert!(matches!(&d.elements[0], Element::Fill { gradient: Some(g), .. } if g.c0 == ROUGE && g.c1 == BLEU));
    // Avec une sélection de traits seuls : rien à remplir.
    b.app.vsel = Some(crate::vtools::VSelection { cel, items: vec![n - 1] });
    let avant = etapes(&b.app).len();
    glisser(&mut b, &[(120.0, 150.0), (280.0, 150.0)], Modifiers::NONE);
    assert_eq!(etapes(&b.app).len(), avant, "un trait n'est pas une zone");
    assert!(b.app.status.contains("zone"), "{}", b.app.status);
}

#[test]
fn the_gradient_zone_is_saved_reloaded_exported_and_old_projects_open() {
    let dir = std::env::temp_dir().join(format!("shot-degrade-projet-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("D.shot");
    let mut doc = Document::new(200, 120, 24);
    doc.panels[0].layers = vec![Layer::new("Vecto", LayerKind::Vector)];
    doc.panels[0].length = 2;
    let cel = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames = vec![Some(cel); 2];
    let g = Reglages { genre: Genre::Radial, fin: Fin::VersTransparent, opacite: 0.8, ..Reglages::default() }.degrade(ROUGE, BLEU, pos2(60.0, 60.0), pos2(110.0, 60.0));
    let zone = Element::Fill { contours: vec![vec![[10.0, 10.0], [190.0, 10.0], [190.0, 110.0], [10.0, 110.0]]], color: ROUGE, opacity: 1.0, gradient: Some(g) };
    let unie = Element::Fill { contours: vec![vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]]], color: [1, 2, 3], opacity: 1.0, gradient: None };
    {
        let c = doc.cels.get_mut(&cel).unwrap();
        let d = c.vector.as_mut().unwrap();
        d.elements = vec![zone.clone(), unie.clone()];
        c.rerender();
    }
    let rendu_avant = doc.cels[&cel].raster.to_flat();
    assert!(rendu_avant[60 * 200 + 60].a() > 150 && rendu_avant[60 * 200 + 120].a() < 40, "le centre est plein, le bord transparent");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    // Le fichier du dessin dit le dégradé ; la zone unie, rien de plus qu'avant.
    let mut dit = 0;
    let mut pile = vec![dir.clone()];
    while let Some(p) = pile.pop() {
        for e in std::fs::read_dir(&p).unwrap().flatten() {
            if e.path().is_dir() {
                pile.push(e.path());
            } else if e.path().extension().is_some_and(|x| x == "json") {
                let t = std::fs::read_to_string(e.path()).unwrap();
                if t.contains("\"degrade\"") {
                    assert!(t.contains("\"radial\"") && t.contains("couleur_arrivee") && t.contains("opacite_arrivee"), "{t}");
                    assert_eq!(t.matches("\"degrade\"").count(), 1, "seule la zone en dégradé l'écrit");
                    dit += 1;
                }
            }
        }
    }
    assert_eq!(dit, 1, "un fichier de dessin porte le dégradé");
    let back = project::load(&manifest).unwrap().doc;
    let cel_b = back.panels[0].layers[0].frames[0].unwrap();
    let d = back.cels[&cel_b].vector.as_ref().expect("vectoriel");
    assert_eq!(d.elements.len(), 2);
    assert_eq!(d.elements[0].gradient(), Some(&g), "le dégradé est relu tel quel");
    assert!(d.elements[1].gradient().is_none());
    assert!(back.cels[&cel_b].raster.to_flat() == rendu_avant, "le rendu après relecture est le même");
    // Un projet d'avant (version 37) s'ouvre tel quel ; un fichier sans le champ est une zone unie.
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains(&format!("\"version\": {}", project::VERSION)) && project::VERSION >= 38);
    std::fs::write(&manifest, texte.replace(&format!("\"version\": {}", project::VERSION), "\"version\": 37")).unwrap();
    let vieux = project::load(&manifest).unwrap().doc;
    let cel_v = vieux.panels[0].layers[0].frames[0].unwrap();
    assert_eq!(vieux.cels[&cel_v].vector.as_ref().unwrap().elements[0].gradient(), Some(&g));
    let sans_champ = serde_json::to_string(&unie).unwrap();
    assert!(!sans_champ.contains("degrade"), "une zone unie n'écrit rien de plus : {sans_champ}");
    let relu: Element = serde_json::from_str(&sans_champ).unwrap();
    assert!(relu.gradient().is_none() && relu == unie);
    // Une version plus récente est refusée, comme toujours.
    std::fs::write(&manifest, texte.replace(&format!("\"version\": {}", project::VERSION), &format!("\"version\": {}", project::VERSION + 1))).unwrap();
    assert!(project::load(&manifest).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_gradient_with_absurd_values_is_refused_at_reading() {
    let dir = std::env::temp_dir().join(format!("shot-degrade-abime-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("A.shot");
    let mut doc = Document::new(100, 100, 24);
    doc.panels[0].layers = vec![Layer::new("Vecto", LayerKind::Vector)];
    let cel = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames = vec![Some(cel); doc.panels[0].length.max(1)];
    let g = Reglages::default().degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(1.0e9, 0.0));
    doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = vec![Element::Fill { contours: vec![vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]]], color: ROUGE, opacity: 1.0, gradient: Some(g) }];
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    // Comme tout dessin aux valeurs absurdes : illisible, dit, et son fichier n'est pas modifié (le projet s'ouvre).
    let ouvert = project::load(&manifest).unwrap();
    assert!(ouvert.warnings.iter().any(|w| w.contains("hors limites")), "{:?}", ouvert.warnings);
    assert!(ouvert.doc.cels.values().all(|c| c.vector.as_ref().is_none_or(|v| v.elements.iter().all(|e| e.gradient().is_none()))), "le dessin abîmé n'est pas pris");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ l'aplat et le calque de couleur

/// Un anneau noir carré (de 4 px) de (60, 60) à (220, 200).
pub(crate) fn anneau(w: usize, h: usize) -> Raster {
    let mut r = Raster::new(w, h);
    for y in 58..=62 {
        for x in 58..=222 {
            put(&mut r, x, y, Color32::BLACK);
            put(&mut r, x, y + 140, Color32::BLACK);
        }
    }
    for x in 58..=62 {
        for y in 58..=202 {
            put(&mut r, x, y, Color32::BLACK);
            put(&mut r, x + 160, y, Color32::BLACK);
        }
    }
    r
}

#[test]
fn the_gradient_fills_the_aplat_region_when_the_aplat_is_targeted() {
    let mut app = app("aplat");
    let mut doc = Document::new(400, 300, 24);
    let cel = doc.add_cel(Cel::bitmap(anneau(400, 300)));
    let mut ligne = Layer::new("Trait", LayerKind::Bitmap);
    ligne.frames = vec![Some(cel); 4];
    doc.panels[0].layers = vec![ligne];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.toggle_fond(0);
    app.select_aplat(0);
    assert!(app.aplat_en_cible());
    app.set_tool(Tool::Gradient);
    assert!(app.aplat_en_cible(), "choisir le dégradé garde l'aplat visé");
    app.color = ROUGE;
    app.couleurs.prefs.back = BLEU;
    let ligne_avant = app.doc.cels[&cel].raster.to_flat();
    let avant = etapes(&app).len();
    // Le même geste que pour le pot de peinture : appui dans la zone fermée, glisser, lâcher.
    app.degrade_press(pos2(80.0, 130.0), false);
    assert!(app.degrade_geste.is_some());
    app.degrade_drag(pos2(200.0, 130.0), false);
    app.degrade_release();
    let s = app.doc.cels[&cel].fond.as_ref().expect("la strate de l'aplat");
    let (g, d, m) = (s.peinture.raster.get(70, 130), s.peinture.raster.get(210, 130), s.peinture.raster.get(140, 130));
    assert!(g.r() > 240 && g.b() < 15 && g.a() == 255, "départ rouge : {g:?}");
    assert!(d.b() > 240 && d.r() < 15 && d.a() == 255, "arrivée bleue : {d:?}");
    assert!(proche(m.r(), 128, 20) && proche(m.b(), 127, 20), "milieu : {m:?}");
    assert_eq!(s.peinture.raster.get(20, 130).a(), 0, "hors de la zone fermée, rien");
    assert_eq!(s.peinture.raster.get(140, 20).a(), 0);
    assert!(app.doc.cels[&cel].raster.to_flat() == ligne_avant, "le trait n'est pas touché");
    assert_eq!(etapes(&app).len(), avant + 1, "une seule étape");
    assert_eq!(etapes(&app).last().map(String::as_str), Some("Aplat : dégradé"));
    // Annuler : l'aplat redevient vide.
    app.undo();
    let s = app.doc.cels[&cel].fond.as_ref().expect("la strate");
    assert!(s.peinture.raster.is_blank(), "la peinture de l'aplat est annulée d'un coup");
    // Alt : tout l'aplat.
    app.degrade_press(pos2(10.0, 10.0), true);
    app.degrade_drag(pos2(390.0, 10.0), false);
    app.degrade_release();
    let s = app.doc.cels[&cel].fond.as_ref().unwrap();
    assert_eq!(s.peinture.raster.get(5, 5).a(), 255);
    assert_eq!(s.peinture.raster.get(395, 295).a(), 255);
    // Le trait fermé a un clic sur le trait lui-même : rien, avec un message.
    app.undo();
    app.degrade_press(pos2(60.0, 130.0), false);
    assert!(app.degrade_geste.is_none() && app.status.contains("zone fermée"), "{}", app.status);
}

#[test]
fn the_gradient_fills_the_zone_of_a_colour_layer_under_the_line() {
    let mut app = app("couleur");
    let mut doc = Document::new(400, 300, 24);
    let cel = doc.add_cel(Cel::bitmap(anneau(400, 300)));
    let mut ligne = Layer::new("Trait", LayerKind::Bitmap);
    ligne.frames = vec![Some(cel); 4];
    doc.panels[0].layers = vec![ligne, Layer::new("Autre", LayerKind::Vector)];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.run_action("calque.nouveau_couleur");
    assert!(app.layer().couleur.is_some(), "le calque courant est la couleur");
    app.set_tool(Tool::Gradient);
    app.color = ROUGE;
    app.couleurs.prefs.back = BLEU;
    let avant = etapes(&app).len();
    app.degrade_press(pos2(80.0, 130.0), false);
    assert!(app.degrade_geste.is_some(), "{}", app.status);
    app.degrade_drag(pos2(200.0, 130.0), false);
    app.degrade_release();
    let id = app.layers()[0].frames[0].expect("le dessin de la couleur");
    let r = &app.doc.cels[&id].raster;
    assert!(r.get(70, 130).r() > 240 && r.get(210, 130).b() > 240, "{:?} {:?}", r.get(70, 130), r.get(210, 130));
    assert_eq!(r.get(20, 130).a(), 0, "hors de la zone fermée par le trait");
    assert_eq!(etapes(&app).len(), avant + 1);
    app.undo();
    assert!(app.doc.cels[&id].is_empty());
}

// ------------------------------------------------------------------------------------ l'outil dans l'interface

#[test]
fn the_tool_is_next_to_the_fill_tool_with_its_icon_tooltip_and_shortcut() {
    // Dans la case du pot de peinture de la barre d'outils.
    let case = crate::toolbars::TOOL_GROUPS.iter().flat_map(|g| g.iter()).find(|s| s.contains(&Tool::Fill)).expect("la case du pot");
    assert!(case.contains(&Tool::Gradient));
    assert!(Tool::ALL.contains(&Tool::Gradient));
    assert_eq!((Tool::Gradient.label(), Tool::Gradient.action()), ("Dégradé", "outil.degrade"));
    assert!(Tool::Gradient.description().len() > 40 && !Tool::Gradient.icon().is_empty());
    assert_ne!(Tool::Gradient.icon(), Tool::Fill.icon());
    // Une action du fichier de raccourcis, avec la touche libre Alt + G, et qui prend l'outil.
    let mut app = app("raccourci");
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == "outil.degrade"));
    assert_eq!(app.keymap.keys_text("outil.degrade"), "Alt+G");
    app.run_action("outil.degrade");
    assert_eq!(app.tool, Tool::Gradient);
    // Les réglages : la forme, la fin, l'opacité, inverser.
    let ctx = egui::Context::default();
    let mut textes: Vec<String> = Vec::new();
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 900.0))), ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
        });
        textes.clear();
        fn collecter(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
    }
    for voulu in ["Vers la couleur de fond", "Vers le transparent", "Opacité", "Inverser"] {
        assert!(textes.iter().any(|t| t.contains(voulu)), "« {voulu} » dans les réglages : {textes:?}");
    }
}

#[test]
fn the_gradient_is_never_picked_by_alt_and_a_new_field_is_classified() {
    // L'outil, ses réglages et son geste sont rangés (l'essai des onglets le vérifie aussi) : le réglage est partagé,
    // le geste est celui d'un projet.
    let app = app("champs");
    assert_eq!(app.degrade, Reglages::default());
    assert!(app.degrade_geste.is_none());
    let g: Degrade = Reglages::default().degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(10.0, 0.0));
    assert!(g.is_sane());
}

#[test]
fn a_big_sheet_is_written_in_a_few_milliseconds_not_by_the_brush_engine() {
    let g = Reglages::default().degrade(ROUGE, BLEU, pos2(0.0, 0.0), pos2(1920.0, 1080.0));
    let mut r = Raster::new(1920, 1080);
    let t = std::time::Instant::now();
    let tiles = crate::degrade::peindre(&mut r, &Portee::Tout, &g, false, None);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    assert_eq!(tiles.len(), r.tiles.len());
    // (Large marge : la machine peut être chargée, et les essais tournent sans optimisation.)
    assert!(ms < 3000.0, "{ms} ms");
}
