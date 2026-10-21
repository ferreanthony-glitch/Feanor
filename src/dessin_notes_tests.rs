//! Notes d'Anthony, lot « dessin » (2026-10-20) : tracé droit (Maj), choisir un objet au clic (Ctrl,
//! Ctrl+Maj), un pinceau par sorte de calque, enregistrer la brosse, réglages repliés, fond de l'aperçu,
//! remplir la sélection (Maj+Suppr), clés de position.

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};

use crate::app::{App, Mode, Tool};
use crate::brush::BrushSettings;
use crate::brushtex::Target;
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::pinceaux::Panneau;
use crate::theme::{self, ChoixTheme, ReglageTheme};

const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const CTRL_SHIFT: Modifiers = Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };
const SHIFT_ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: true, mac_cmd: false, command: false };

// ------------------------------------------------------------------------------------ l'appli

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-dessin-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    // (La bibliothèque des pinceaux importés se lit sur un autre fil : on l'attend.)
    let debut = std::time::Instant::now();
    while app.brushes_job.is_some() && debut.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(2));
        app.poll_brushes();
    }
    app
}

/// Un plan 400 × 300 à un calque de cette sorte, à l'écran (1280 × 720), outil Pinceau en rouge.
fn banc_dessin(kind: LayerKind) -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", kind)];
    doc.panels[0].length = 4;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.tool = Tool::Brush;
    b.app.color = [255, 0, 0];
    b.app.brush.size = 8.0;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

/// Le point (`x`, `y`) de la feuille, à l'écran.
fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn bouton(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers }
}

/// Un glisser du pointeur sur les points `pts` de la feuille, une image d'écran par point, avec ces
/// touches tenues pendant tout le geste.
fn glisser(b: &mut Banc, pts: &[(f32, f32)], touches: Modifiers) {
    glisser_avec(b, pts, touches, touches);
}

/// Comme `glisser`, avec `debut` tenues à l'appui et `ensuite` pendant les déplacements.
fn glisser_avec(b: &mut Banc, pts: &[(f32, f32)], debut: Modifiers, ensuite: Modifiers) {
    let p = |b: &Banc, k: usize| ecran(b, pts[k].0, pts[k].1);
    b.touches = debut;
    let p0 = p(b, 0);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, debut)], 1.0 / 60.0);
    b.touches = ensuite;
    for k in 1..pts.len() {
        let pk = p(b, k);
        b.image(vec![Event::PointerMoved(pk)], 1.0 / 60.0);
    }
    // Une image sans mouvement : Maj tenue, le segment suit encore la pointe.
    b.image(Vec::new(), 1.0 / 60.0);
    let fin = p(b, pts.len() - 1);
    b.image(vec![bouton(fin, false, ensuite)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un clic en (`x`, `y`) de la feuille avec ces touches.
fn cliquer(b: &mut Banc, x: f32, y: f32, touches: Modifiers) {
    glisser(b, &[(x, y)], touches);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// L'opacité du pixel (`x`, `y`) du dessin ; 0 s'il n'y a plus de dessin (un trait annulé reprend aussi
/// la création du dessin).
fn alpha(b: &Banc, x: usize, y: usize) -> u8 {
    match b.app.layers()[0].frames.first().copied().flatten() {
        Some(cel) => b.app.doc.cels[&cel].raster.get(x, y).a(),
        None => 0,
    }
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

// ------------------------------------------------------------------------------------ 1. tracé droit

#[test]
fn shift_held_from_the_press_draws_a_straight_line_from_the_start_to_the_pointer() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    let avant = etapes(&b);
    // Départ en (60, 120), puis un détour par le bas et par le haut, et l'arrivée en (300, 120).
    glisser(&mut b, &[(60.0, 120.0), (120.0, 200.0), (200.0, 40.0), (300.0, 120.0)], Modifiers::SHIFT);
    for x in [70, 120, 180, 240, 296] {
        assert!(alpha(&b, x, 120) > 200, "le segment passe par ({x}, 120)");
    }
    assert!(alpha(&b, 60, 120) > 200 && alpha(&b, 300, 120) > 200, "il va du départ à l'arrivée");
    assert_eq!(alpha(&b, 120, 190), 0, "le détour par le bas n'est pas tracé");
    assert_eq!(alpha(&b, 200, 50), 0, "le détour par le haut non plus");
    assert_eq!(alpha(&b, 180, 140), 0, "le trait est droit : rien à côté");
    // Un seul geste, une seule étape d'annulation.
    assert_eq!(etapes(&b), avant + 1);
    b.app.undo();
    assert_eq!(alpha(&b, 180, 120), 0, "annulé d'un coup");
}

#[test]
fn without_shift_the_stroke_stays_freehand() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    glisser(&mut b, &[(60.0, 120.0), (120.0, 200.0), (200.0, 40.0), (300.0, 120.0)], Modifiers::NONE);
    assert!(alpha(&b, 90, 160) > 150, "le début du détour est tracé (de (60, 120) à (120, 200))");
    assert_eq!(alpha(&b, 180, 140), 0, "mais pas la ligne droite");
}

#[test]
fn shift_pressed_in_the_middle_of_a_stroke_goes_straight_from_the_last_point() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    // Tracé libre jusqu'en (100, 200), puis Maj : un segment droit jusqu'en (300, 200).
    let p = |b: &Banc, x: f32, y: f32| ecran(b, x, y);
    let d = p(&b, 60.0, 100.0);
    b.image(vec![Event::PointerMoved(d)], 1.0 / 60.0);
    b.image(vec![bouton(d, true, Modifiers::NONE)], 1.0 / 60.0);
    for (x, y) in [(80.0, 150.0), (100.0, 200.0)] {
        let q = p(&b, x, y);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.touches = Modifiers::SHIFT;
    for (x, y) in [(150.0, 100.0), (220.0, 260.0), (300.0, 200.0)] {
        let q = p(&b, x, y);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    let fin = p(&b, 300.0, 200.0);
    b.image(vec![bouton(fin, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(alpha(&b, 70, 125) > 150, "le tracé libre d'avant Maj reste");
    for x in [120, 200, 260, 298] {
        assert!(alpha(&b, x, 200) > 200, "puis le segment droit de (100, 200) à (300, 200) : ({x}, 200)");
    }
    assert_eq!(alpha(&b, 150, 110), 0, "les détours de Maj ne sont pas tracés");
}

#[test]
fn shift_alt_snaps_the_straight_line_to_15_degrees() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    // Du départ (60, 100) vers (300, 142) : environ 10° ; aligné par 15°, la ligne monte à 15°.
    // Alt est pris à l'appui (pipette) : on ne le tient que pendant le trait.
    glisser_avec(&mut b, &[(60.0, 100.0), (180.0, 120.0), (300.0, 142.0)], Modifiers::SHIFT, SHIFT_ALT);
    let d = (300.0f32 - 60.0, 142.0f32 - 100.0);
    let len = (d.0 * d.0 + d.1 * d.1).sqrt();
    let (c, s) = (15f32.to_radians().cos(), 15f32.to_radians().sin());
    let (ex, ey) = (60.0 + len * c, 100.0 + len * s);
    assert!(alpha(&b, ex as usize - 2, ey as usize - 1) > 100, "le bout est sur la droite à 15° : ({ex:.0}, {ey:.0})");
    assert_eq!(alpha(&b, 300, 142), 0, "et plus sur le point visé à la souris");
    let mid_x = 60.0 + (ex - 60.0) * 0.5;
    let mid_y = 100.0 + (mid_x - 60.0) * 15f32.to_radians().tan();
    assert!(alpha(&b, mid_x as usize, mid_y as usize) > 150, "le milieu aussi : ({mid_x:.0}, {mid_y:.0})");
}

#[test]
fn shift_works_with_the_eraser_too() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    // Une barre rouge de 40 à 360 à y = 120, sans Maj.
    glisser(&mut b, &[(40.0, 120.0), (200.0, 120.0), (360.0, 120.0)], Modifiers::NONE);
    assert!(alpha(&b, 200, 120) > 200);
    b.app.tool = Tool::Eraser;
    b.app.eraser.size = 12.0;
    b.chauffer(2);
    glisser(&mut b, &[(100.0, 120.0), (150.0, 200.0), (260.0, 220.0), (300.0, 120.0)], Modifiers::SHIFT);
    assert_eq!(alpha(&b, 200, 120), 0, "la gomme a passé en ligne droite de (100, 120) à (300, 120)");
    assert!(alpha(&b, 60, 120) > 200 && alpha(&b, 340, 120) > 200, "le reste de la barre est là");
}

#[test]
fn shift_draws_a_straight_vector_stroke() {
    let mut b = banc_dessin(LayerKind::Vector);
    glisser(&mut b, &[(60.0, 120.0), (120.0, 200.0), (200.0, 40.0), (300.0, 120.0)], Modifiers::SHIFT);
    let cel = b.app.layers()[0].frames[0].expect("un dessin");
    let d = b.app.drawing(cel).expect("dessin vectoriel");
    assert_eq!(d.elements.len(), 1, "un seul trait");
    match &d.elements[0] {
        crate::vector::Element::Stroke { points, .. } => {
            assert!(points.len() >= 2);
            assert!(points.iter().all(|p| (p.y - 120.0).abs() < 1.5), "tous les points sur la droite : {:?}", points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>());
            let (lo, hi) = points.iter().fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p.x), h.max(p.x)));
            assert!(lo < 66.0 && hi > 294.0, "de (60, 120) à (300, 120) : {lo} à {hi}");
        }
        e => panic!("un trait attendu : {e:?}"),
    }
}

#[test]
fn a_guided_stroke_ignores_shift() {
    // Un trait aligné sur un guide est déjà droit : Maj n'y ajoute rien (pas de segment en attente).
    let mut app = app("droit-guide");
    app.guides.stroke = Some((pos2(10.0, 10.0), None));
    let mods = Modifiers::SHIFT;
    app.droit_start(pos2(10.0, 10.0));
    app.stroke_point(pos2(50.0, 50.0), 1.0, &mods, false);
    assert!(!app.droit.pending());
}

// ------------------------------------------------------------------------------------ 2. choisir un objet

/// Un dessin 400 × 300 à un calque bitmap avec deux taches : A (de (40, 40) à (80, 70)) et B (de
/// (200, 100) à (260, 140)), séparées.
fn deux_taches(b: &mut Banc) {
    let cel = b.app.doc.new_cel(LayerKind::Bitmap);
    {
        let r = &mut b.app.doc.cels.get_mut(&cel).unwrap().raster;
        for (x0, y0, x1, y1) in [(40usize, 40usize, 80usize, 70usize), (200, 100, 260, 140)] {
            for y in y0..y1 {
                for x in x0..x1 {
                    let idx = r.tile_index_of(x, y);
                    r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgb(200, 30, 30);
                }
            }
        }
    }
    b.app.doc.panels[0].layers[0].set(0, Some(cel));
    b.app.renderer.invalidate_all();
    b.chauffer(3);
}

#[test]
fn an_object_is_the_connected_shape_of_drawn_pixels() {
    let mut r = crate::raster::Raster::new(100, 80);
    let mut put = |x: usize, y: usize| {
        let idx = r.tile_index_of(x, y);
        r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(40, 0, 0, 200);
    };
    // Une ligne, un pixel en diagonale (voisin par un coin), puis un îlot séparé par un vide.
    for x in 10..30 {
        put(x, 20);
    }
    put(30, 21);
    put(50, 20);
    let (mask, outline) = crate::selobjet::object_at(&r, 12, 20).expect("une forme");
    assert!(mask.at(10, 20) > 0.0 && mask.at(29, 20) > 0.0 && mask.at(30, 21) > 0.0, "huit voisins : la diagonale en fait partie");
    assert_eq!(mask.at(50, 20), 0.0, "l'îlot séparé n'en fait pas partie");
    assert!(!outline.is_empty());
    assert!(crate::selobjet::object_at(&r, 12, 30).is_none(), "un pixel vide n'est pas un objet");
    // Un clic à côté d'un trait fin le trouve, dans la portée.
    assert!(crate::selobjet::object_near(&r, pos2(20.5, 23.0), 4.0).is_some());
    assert!(crate::selobjet::object_near(&r, pos2(20.5, 40.0), 4.0).is_none());
}

#[test]
fn ctrl_click_chooses_the_object_under_the_pointer_on_a_bitmap_layer() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    deux_taches(&mut b);
    cliquer(&mut b, 60.0, 55.0, CTRL);
    let sel = b.app.bsel.as_ref().expect("la tache A est choisie");
    assert!(sel.mask.at(50, 50) > 0.0 && sel.mask.at(230, 120) == 0.0, "A seule");
    assert_eq!(b.app.tool, Tool::Select, "la zone bitmap garde l'outil Sélection");
    // Un clic (Ctrl) sur B remplace la sélection.
    cliquer(&mut b, 230.0, 120.0, CTRL);
    let sel = b.app.bsel.as_ref().expect("la tache B est choisie");
    assert!(sel.mask.at(230, 120) > 0.0 && sel.mask.at(50, 50) == 0.0, "B seule");
    // Un clic dans le vide désélectionne.
    cliquer(&mut b, 150.0, 250.0, CTRL);
    assert!(b.app.bsel.is_none());
}

#[test]
fn ctrl_shift_click_adds_another_object_and_a_second_one_removes_it() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    deux_taches(&mut b);
    cliquer(&mut b, 60.0, 55.0, CTRL);
    assert!(b.app.bsel.is_some());
    // Ctrl+Maj + clic sur B : A reste, B s'ajoute (sélection sur tous les calques).
    cliquer(&mut b, 230.0, 120.0, CTRL_SHIFT);
    let sel = b.app.multi.sel.as_ref().expect("une sélection cumulée");
    assert_eq!(sel.parts.len(), 1, "un seul calque");
    match &sel.parts[0].kind {
        crate::multisel::PartKind::Bitmap { mask, .. } => assert!(mask.at(50, 50) > 0.0 && mask.at(230, 120) > 0.0, "A et B"),
        _ => panic!("une zone bitmap"),
    }
    assert!(b.app.bsel.is_none(), "la zone d'un seul calque est reprise dans la sélection cumulée");
    // Ctrl+Maj + clic sur B encore : B est retirée.
    cliquer(&mut b, 230.0, 120.0, CTRL_SHIFT);
    let sel = b.app.multi.sel.as_ref().expect("A reste");
    match &sel.parts[0].kind {
        crate::multisel::PartKind::Bitmap { mask, .. } => assert!(mask.at(50, 50) > 0.0 && mask.at(230, 120) == 0.0, "A seule"),
        _ => panic!("une zone bitmap"),
    }
}

/// Un calque vectoriel avec deux traits : A de (40, 60) à (140, 60), B de (40, 160) à (140, 160).
fn deux_traits(b: &mut Banc) {
    let cel = b.app.doc.new_cel(LayerKind::Vector);
    let stroke = |y: f32| crate::vector::Element::Stroke {
        points: [40.0, 90.0, 140.0].iter().map(|&x| crate::vector::VPoint { x, y, r: 3.0 }).collect(),
        color: [20, 20, 20],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    };
    {
        let c = b.app.doc.cels.get_mut(&cel).unwrap();
        c.vector.as_mut().unwrap().elements = vec![stroke(60.0), stroke(160.0)];
        c.rerender();
    }
    b.app.doc.panels[0].layers[0].set(0, Some(cel));
    b.app.renderer.invalidate_all();
    b.chauffer(3);
}

#[test]
fn ctrl_click_and_ctrl_shift_click_accumulate_vector_strokes() {
    let mut b = banc_dessin(LayerKind::Vector);
    deux_traits(&mut b);
    // Ctrl + clic : le trait A (le lasso temporaire de Ctrl prend l'outil Sélection le temps du clic).
    cliquer(&mut b, 90.0, 60.0, CTRL);
    assert_eq!(b.app.vsel.as_ref().map(|s| s.items.clone()), Some(vec![0]));
    assert_eq!(b.app.tool, Tool::Brush, "l'outil d'avant revient au relâché de Ctrl");
    // Ctrl + Maj + clic sur B : A et B.
    cliquer(&mut b, 90.0, 160.0, CTRL_SHIFT);
    let sel = b.app.multi.sel.as_ref().expect("sélection cumulée");
    match &sel.parts[0].kind {
        crate::multisel::PartKind::Vector { items } => {
            let mut v = items.clone();
            v.sort();
            assert_eq!(v, vec![0, 1], "A et B");
        }
        _ => panic!("des traits"),
    }
    // Ctrl + Maj + clic sur B : B est retiré, A reste.
    cliquer(&mut b, 90.0, 160.0, CTRL_SHIFT);
    let sel = b.app.multi.sel.as_ref().expect("A reste");
    match &sel.parts[0].kind {
        crate::multisel::PartKind::Vector { items } => assert_eq!(items, &vec![0]),
        _ => panic!("des traits"),
    }
    // Ctrl + Maj + clic sur A : plus rien.
    cliquer(&mut b, 90.0, 60.0, CTRL_SHIFT);
    assert!(b.app.multi.sel.is_none(), "la dernière sélection retirée, il n'y a plus de sélection");
}

// ------------------------------------------------------------------------------------ 3. pinceau par sorte

fn gouache() -> BrushSettings {
    BrushSettings { size: 31.0, opacity: 0.6, ..BrushSettings::brush() }
}

fn encre() -> BrushSettings {
    BrushSettings { size: 5.0, opacity: 1.0, ..BrushSettings::brush() }
}

#[test]
fn the_panel_remembers_the_last_brush_of_each_layer_kind() {
    let mut p = Panneau::default();
    // Premier appel : rien de retenu, le pinceau du moment devient celui de la sorte courante.
    assert_eq!(p.suivre_sorte(LayerKind::Vector, &encre()), None);
    assert_eq!(p.pinceau_vecto, Some(encre()));
    assert_eq!(p.pinceau_bitmap, None);
    // On reste sur le vectoriel et on change de pinceau : il est retenu.
    assert_eq!(p.suivre_sorte(LayerKind::Vector, &gouache()), None);
    assert_eq!(p.pinceau_vecto, Some(gouache()));
    // On passe sur un calque bitmap qui n'a pas servi : le pinceau du moment reste, et devient le sien.
    assert_eq!(p.suivre_sorte(LayerKind::Bitmap, &gouache()), None);
    assert_eq!(p.pinceau_bitmap, Some(gouache()));
    // Au bitmap, on prend l'encre ; retour au vectoriel : la gouache y revient ; retour au bitmap : l'encre.
    assert_eq!(p.suivre_sorte(LayerKind::Bitmap, &encre()), None);
    assert_eq!(p.suivre_sorte(LayerKind::Vector, &encre()), Some(gouache()), "le dernier pinceau du vectoriel revient");
    assert_eq!(p.suivre_sorte(LayerKind::Vector, &gouache()), None);
    assert_eq!(p.suivre_sorte(LayerKind::Bitmap, &gouache()), Some(encre()), "le dernier pinceau du bitmap revient");
}

#[test]
fn switching_the_layer_kind_swaps_the_brush_in_the_app() {
    let mut app = app("pinceau-sorte");
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Encre", LayerKind::Vector), Layer::new("Peinture", LayerKind::Bitmap)];
    app.doc = doc;
    (app.cur_panel, app.cur_layer) = (0, 0);
    app.pinceaux = Panneau::default();
    // Sur le calque vectoriel : « encre ».
    app.brush = encre();
    app.suivre_sorte_pinceau();
    // Calque bitmap, on choisit « gouache ».
    app.cur_layer = 1;
    app.suivre_sorte_pinceau();
    app.brush = gouache();
    app.suivre_sorte_pinceau();
    // Retour au vectoriel : « encre » ; retour au bitmap : « gouache ».
    app.cur_layer = 0;
    app.suivre_sorte_pinceau();
    assert_eq!(app.brush, encre());
    app.cur_layer = 1;
    app.suivre_sorte_pinceau();
    assert_eq!(app.brush, gouache());
    // Un dossier ou un calque d'effet ne change rien.
    app.layers_mut().push(Layer::new_folder("Dossier"));
    app.cur_layer = 2;
    app.suivre_sorte_pinceau();
    assert_eq!(app.brush, gouache());
}

#[test]
fn the_brush_of_each_kind_is_kept_in_the_settings_file() {
    let mut s = crate::settings::Settings::default();
    s.brushes_panel.pinceau_vecto = Some(encre());
    s.brushes_panel.pinceau_bitmap = Some(gouache());
    s.brushes_panel.reglages_ouverts = true;
    let text = toml::to_string(&s).expect("écrit en TOML");
    let back: crate::settings::Settings = toml::from_str(&text).expect("relu");
    assert_eq!(back.brushes_panel.pinceau_vecto, Some(encre()));
    assert_eq!(back.brushes_panel.pinceau_bitmap, Some(gouache()));
    assert!(back.brushes_panel.reglages_ouverts);
    // À la séance suivante, le premier appel rend le pinceau retenu de la sorte du calque.
    let mut p = back.brushes_panel;
    assert_eq!(p.suivre_sorte(LayerKind::Bitmap, &BrushSettings::brush()), Some(gouache()), "au lancement, le pinceau d'avant revient");
    // Des réglages d'avant ces notes se lisent : rien de retenu, réglages repliés.
    let old: crate::settings::Settings = toml::from_str("[panneau_pinceaux]\nliste = false\n").expect("ancien fichier");
    assert_eq!((old.brushes_panel.pinceau_vecto, old.brushes_panel.pinceau_bitmap, old.brushes_panel.reglages_ouverts), (None, None, false));
}

// ------------------------------------------------------------------------------------ 4. enregistrer la brosse

#[test]
fn the_save_icon_replaces_the_chosen_brush_and_can_be_undone() {
    let mut app = app("pinceau-enregistrer");
    app.tool = Tool::Brush;
    let i = app.brushes.save_mine("Mon encre", Target::Brush, encre());
    app.brush_previews.base = Some((Target::Brush, "Mon encre".into(), crate::brushtex::MINE.into()));
    assert_eq!(app.base_preset(Target::Brush), Some(i));
    // Retouché à la main : plus aucune brosse n'a exactement ce réglage, mais on sait d'où il vient.
    app.brush = BrushSettings { size: 17.0, ..encre() };
    assert!(crate::pinceaux::choisi(&app.brushes.presets, Target::Brush, &app.brush).is_none());
    assert_eq!(app.base_preset(Target::Brush), Some(i));
    let cur = app.brush;
    app.apply_preset_menu(i, crate::brushui::PresetMenu::Update, Target::Brush, cur);
    assert_eq!(app.brushes.presets[i].settings, cur, "la brosse a pris le réglage courant");
    assert_eq!(crate::pinceaux::choisi(&app.brushes.presets, Target::Brush, &app.brush), Some(i), "on lit de nouveau son nom");
    // Rétablir l'ancien réglage.
    app.restore_preset();
    assert_eq!(app.brushes.presets[i].settings, encre());
    assert!(app.brush_previews.ancien.is_none());
    // Les brosses de SHOT ne se remplacent pas.
    let shot = app.brushes.presets.iter().position(|p| p.source == crate::pinceaux::PELURE && p.target == Target::Brush).expect("une brosse de SHOT");
    assert!(!crate::pinceaux::supprimable(&app.brushes.presets[shot]));
}

#[test]
fn the_save_icon_is_drawn_in_the_settings_panel() {
    let (ctx, mut app) = (egui::Context::default(), app("pinceau-icone"));
    app.tool = Tool::Brush;
    let textes = rendre(&ctx, &mut app, |app, ui| app.brush_summary_ui(ui));
    assert!(textes.iter().any(|t| t.contains(egui_phosphor::regular::FLOPPY_DISK)), "l'icône Enregistrer est là : {textes:?}");
}

// ------------------------------------------------------------------------------------ 5. réglages repliés

/// Rend une image du contenu de `f` et rend les textes peints.
fn rendre(ctx: &egui::Context, app: &mut App, f: impl Fn(&mut App, &mut egui::Ui)) -> Vec<String> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 900.0))), time: Some(1.0), ..Default::default() };
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| f(app, ui));
    });
    out.shapes
        .iter()
        .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some(t.galley.job.text.clone()) } else { None })
        .collect()
}

#[test]
fn everything_under_the_stroke_preview_is_folded_until_the_arrow_is_clicked() {
    let (ctx, mut app) = (egui::Context::default(), app("pinceau-repli"));
    app.tool = Tool::Brush;
    assert!(!app.pinceaux.reglages_ouverts, "replié par défaut");
    let ferme = rendre(&ctx, &mut app, |app, ui| app.brush_settings_ui(ui));
    assert!(ferme.iter().all(|t| !t.contains("Forme de la pointe")), "aucune catégorie : {ferme:?}");
    assert!(ferme.iter().any(|t| t.contains(egui_phosphor::regular::CARET_RIGHT)), "la petite flèche est là : {ferme:?}");
    // Déplié (ce que fait le clic sur la flèche) : les catégories sont là, et la flèche change de sens.
    app.pinceaux.reglages_ouverts = true;
    let ouvert = rendre(&ctx, &mut app, |app, ui| app.brush_settings_ui(ui));
    assert!(ouvert.iter().any(|t| t.contains("Forme de la pointe")), "les catégories se montrent : {ouvert:?}");
    assert!(ouvert.iter().any(|t| t.contains(egui_phosphor::regular::CARET_DOWN)));
}

// ------------------------------------------------------------------------------------ 6. fond de l'aperçu

#[test]
fn the_stroke_preview_background_is_a_grey_a_little_lighter_than_the_panel_in_the_three_themes() {
    for choix in [ChoixTheme::GrisSombre, ChoixTheme::GrisClair, ChoixTheme::Sombre] {
        let mut r = ReglageTheme::default();
        r.choisir(choix);
        theme::installer(&r);
        let p = theme::palette();
        assert!(theme::luminance(p.apercu_trait) > theme::luminance(p.panneau), "{choix:?} : plus clair que le panneau");
        assert_ne!(p.apercu_trait, Color32::WHITE, "{choix:?} : pas le papier blanc");
        // Un gris : les trois canaux proches.
        let (lo, hi) = [p.apercu_trait.r(), p.apercu_trait.g(), p.apercu_trait.b()].iter().fold((255u8, 0u8), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi - lo <= 12, "{choix:?} : un gris ({:?})", p.apercu_trait);
        // Un trait rouge ou noir ressort sur ce fond.
        for trait_ in [Color32::from_rgb(220, 30, 30), Color32::BLACK, Color32::WHITE] {
            let fond = theme::fond_apercu_trait(trait_);
            assert!(theme::contraste(trait_, fond) >= theme::CONTRASTE_APERCU, "{choix:?} : {trait_:?} sur {fond:?}");
        }
        // Un trait qui ressemble au gris du fond (la gomme, une brosse grise) : le fond est poussé, le trait reste lisible.
        let proche = p.apercu_trait;
        let fond = theme::fond_apercu_trait(proche);
        assert_ne!(fond, proche, "{choix:?} : le fond change quand le trait lui ressemble");
        assert!(theme::contraste(proche, fond) >= theme::CONTRASTE_APERCU, "{choix:?} : trait gris lisible");
    }
}

// ------------------------------------------------------------------------------------ 7. remplir la sélection

fn app_remplir(name: &str) -> App {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.tool = Tool::Select;
    app.color = [0, 0, 255];
    app
}

/// Une zone rectangulaire choisie sur le dessin courant.
fn choisir_rect(app: &mut App, r: Rect) {
    let (cel, _) = app.ensure_cel();
    let poly = crate::pixsel::rect_polygon(r);
    let mask = crate::pixsel::PixelMask::from_polygons(std::slice::from_ref(&poly), 400, 300).expect("zone");
    app.bsel = Some(crate::pixtools::BSelection { cel, mask, outline: vec![poly], lifted: None });
}

fn pixel(app: &App, x: usize, y: usize) -> Color32 {
    let cel = app.layers()[0].frames[0].expect("un dessin");
    app.doc.cels[&cel].raster.get(x, y)
}

#[test]
fn filling_a_pixel_follows_the_alpha_lock() {
    let bleu = [0u8, 0, 255];
    // Vide + couleur : la couleur.
    assert_eq!(crate::remplirsel::filled_pixel(Color32::TRANSPARENT, bleu, 1.0, false), Color32::from_rgb(0, 0, 255));
    // Couverture partielle (bord anticrénelé) : un mélange.
    let moitie = crate::remplirsel::filled_pixel(Color32::TRANSPARENT, bleu, 0.5, false);
    assert!(moitie.a() >= 127 && moitie.a() <= 129 && moitie.b() >= 127 && moitie.b() <= 129, "{moitie:?}");
    // Préserver la transparence : le vide reste vide, ce qui est dessiné change de couleur et garde son opacité.
    assert_eq!(crate::remplirsel::filled_pixel(Color32::TRANSPARENT, bleu, 1.0, true), Color32::TRANSPARENT);
    let rouge_mi = Color32::from_rgba_premultiplied(100, 0, 0, 100);
    let out = crate::remplirsel::filled_pixel(rouge_mi, bleu, 1.0, true);
    assert_eq!(out.a(), 100, "l'opacité reste");
    assert!(out.b() >= 99 && out.r() == 0, "la couleur est bleue : {out:?}");
}

#[test]
fn shift_delete_fills_the_bitmap_selection_with_the_current_colour_in_one_named_step() {
    let mut app = app_remplir("remplir");
    choisir_rect(&mut app, Rect::from_min_max(pos2(50.0, 50.0), pos2(120.0, 100.0)));
    let avant = app.history.names().0.len();
    app.run_action("selection.remplir");
    assert_eq!(pixel(&app, 60, 60), Color32::from_rgb(0, 0, 255), "dedans : bleu");
    assert_eq!(pixel(&app, 10, 10), Color32::TRANSPARENT, "dehors : rien");
    assert_eq!(pixel(&app, 130, 60), Color32::TRANSPARENT);
    let (undo, _) = app.history.names();
    assert_eq!(undo.len(), avant + 1, "une seule étape");
    assert!(undo.last().is_some_and(|n| n.contains("Remplir")), "elle est nommée : {:?}", undo.last());
    // Annuler / rétablir.
    app.undo();
    assert_eq!(pixel(&app, 60, 60), Color32::TRANSPARENT);
    app.redo();
    assert_eq!(pixel(&app, 60, 60), Color32::from_rgb(0, 0, 255));
}

#[test]
fn filling_respects_the_layer_lock_and_the_alpha_lock() {
    // Calque verrouillé : refusé.
    let mut app = app_remplir("remplir-verrou");
    choisir_rect(&mut app, Rect::from_min_max(pos2(50.0, 50.0), pos2(120.0, 100.0)));
    app.layers_mut()[0].locked = true;
    let avant = app.history.names().0.len();
    app.run_action("selection.remplir");
    assert_eq!(pixel(&app, 60, 60), Color32::TRANSPARENT, "un calque verrouillé ne se remplit pas");
    assert_eq!(app.history.names().0.len(), avant, "aucune étape");
    assert!(app.status.contains("verrouillé"), "{}", app.status);
    // Préserver la transparence : seul ce qui est dessiné change de couleur.
    let mut app = app_remplir("remplir-alpha");
    choisir_rect(&mut app, Rect::from_min_max(pos2(50.0, 50.0), pos2(120.0, 100.0)));
    let cel = app.layers()[0].frames[0].expect("dessin");
    {
        let r = &mut app.doc.cels.get_mut(&cel).unwrap().raster;
        let idx = r.tile_index_of(60, 60);
        r.tile_mut(idx)[60 * 64 + 60] = Color32::from_rgb(200, 0, 0);
    }
    app.layers_mut()[0].keep_alpha = true;
    app.run_action("selection.remplir");
    assert_eq!(pixel(&app, 60, 60), Color32::from_rgb(0, 0, 255), "ce qui était dessiné est bleu, et opaque comme avant");
    assert_eq!(pixel(&app, 80, 80), Color32::TRANSPARENT, "le vide reste vide");
}

#[test]
fn shift_delete_without_a_bitmap_selection_keeps_its_old_meaning() {
    // Sans zone bitmap, Maj+Suppr garde son ancien sens : « effacer hors de la sélection ».
    let mut app = app("remplir-ancien");
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Vector)];
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.run_action("selection.remplir");
    assert!(app.status.contains("Rien de sélectionné"), "{}", app.status);
}

// ------------------------------------------------------------------------------------ 8. clés de position

use crate::document::LayerTransform;
use crate::poskeys::{self, PosKey};

fn pose_x(x: f32) -> LayerTransform {
    LayerTransform { x, ..LayerTransform::default() }
}

#[test]
fn a_layer_without_keys_keeps_its_fixed_pose_and_with_keys_it_goes_from_one_to_the_next() {
    let base = pose_x(7.0);
    assert_eq!(poskeys::pose_at(&[], 12, base), base, "sans clé : la pose fixe");
    let one = [PosKey::new(4, pose_x(50.0))];
    assert_eq!(poskeys::pose_at(&one, 0, base).x, 50.0, "une seule clé : fixe");
    let keys = [PosKey::new(2, pose_x(0.0)), PosKey::new(12, pose_x(100.0))];
    assert_eq!(poskeys::pose_at(&keys, 0, base).x, 0.0, "avant la première clé : elle");
    assert_eq!(poskeys::pose_at(&keys, 2, base).x, 0.0);
    assert!((poskeys::pose_at(&keys, 7, base).x - 50.0).abs() < 1e-3, "à mi-chemin : la moitié");
    assert!((poskeys::pose_at(&keys, 4, base).x - 20.0).abs() < 1e-3, "en ligne droite");
    assert_eq!(poskeys::pose_at(&keys, 12, base).x, 100.0);
    assert_eq!(poskeys::pose_at(&keys, 40, base).x, 100.0, "après la dernière : elle");
    // Tout se mélange : rotation, échelle, étirement.
    let a = LayerTransform { x: 0.0, y: 0.0, scale: 1.0, angle: 0.0, stretch_x: 1.0, stretch_y: 1.0, ..Default::default() };
    let b = LayerTransform { x: 10.0, y: -20.0, scale: 3.0, angle: 90.0, stretch_x: 2.0, stretch_y: 0.5, ..Default::default() };
    let m = poskeys::lerp(&a, &b, 0.5);
    assert_eq!((m.x, m.y, m.scale, m.angle, m.stretch_x, m.stretch_y), (5.0, -10.0, 2.0, 45.0, 1.5, 0.75));
}

#[test]
fn smoothing_a_key_slows_the_layer_down_when_it_arrives_and_leaves() {
    let mut keys = vec![PosKey::new(0, pose_x(0.0)), PosKey::new(10, pose_x(100.0))];
    let linear: Vec<f32> = (0..=10).map(|f| poskeys::pose_at(&keys, f, LayerTransform::default()).x).collect();
    assert!((linear[2] - 20.0).abs() < 1e-3);
    // Lissées : même début et même fin, mais plus lent au départ et à l'arrivée, et toujours croissant.
    assert!(poskeys::set_eases(&mut keys, 0, true, 0.0, 0.0) && poskeys::set_eases(&mut keys, 10, true, 0.0, 0.0));
    let smooth: Vec<f32> = (0..=10).map(|f| poskeys::pose_at(&keys, f, LayerTransform::default()).x).collect();
    assert_eq!((smooth[0], smooth[10]), (0.0, 100.0));
    assert!(smooth[2] < linear[2] - 1.0, "plus lent au départ : {} contre {}", smooth[2], linear[2]);
    assert!(smooth[8] > linear[8] + 1.0, "plus lent à l'arrivée : {} contre {}", smooth[8], linear[8]);
    assert!((smooth[5] - 50.0).abs() < 1e-2, "le milieu ne change pas");
    assert!(smooth.windows(2).all(|w| w[1] >= w[0]), "toujours croissant : {smooth:?}");
    // Adoucis réglés à la main, en images : seulement l'arrivée.
    assert!(poskeys::set_eases(&mut keys, 0, false, 0.0, 0.0) && poskeys::set_eases(&mut keys, 10, false, 5.0, 0.0));
    let arrival: Vec<f32> = (0..=10).map(|f| poskeys::pose_at(&keys, f, LayerTransform::default()).x).collect();
    assert!(arrival[9] > linear[9], "ralentit en arrivant : {} contre {}", arrival[9], linear[9]);
    assert!((arrival[2] - 2.0 * arrival[1]).abs() < 1e-2 * 100.0 || arrival[2] > 0.0, "vitesse constante au départ");
    assert!(!poskeys::set_eases(&mut keys, 4, true, 0.0, 0.0), "pas de clé à cette image");
}

#[test]
fn the_keys_follow_inserted_and_removed_frames_and_the_cut_and_the_joint_of_plans() {
    let mut keys = vec![PosKey::new(2, pose_x(0.0)), PosKey::new(10, pose_x(80.0))];
    poskeys::shift_keys(&mut keys, 5, 3, false);
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 13], "trois images insérées en 5 : la clé d'après suit");
    poskeys::shift_keys(&mut keys, 5, 3, true);
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 10]);
    poskeys::shift_keys(&mut keys, 9, 3, true);
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2], "la clé des images retirées part");
    // Coupe d'un plan en 6 : le mouvement est gardé de chaque côté.
    let moving = vec![PosKey::new(0, pose_x(0.0)), PosKey::new(10, pose_x(100.0))];
    let (first, second) = poskeys::split_keys(&moving, 6, LayerTransform::default());
    assert_eq!(first.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 5], "une clé à l'image 5, la dernière du premier plan");
    assert!((first[1].t.x - 50.0).abs() < 1e-3);
    assert_eq!(second.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 4], "le second repart à 0 avec la pose de la coupe");
    assert!((second[0].t.x - 60.0).abs() < 1e-3 && second[1].t.x == 100.0);
    // Réunion de deux plans de 10 et 8 images : les clés du second suivent, décalées.
    let joined = poskeys::join_keys(&moving, 11, LayerTransform::default(), &[PosKey::new(0, pose_x(5.0)), PosKey::new(3, pose_x(9.0))], 8, LayerTransform::default());
    assert_eq!(joined.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 10, 11, 14]);
    // Deux plans sans clé et à la même pose : toujours sans clé.
    assert!(poskeys::join_keys(&[], 5, pose_x(3.0), &[], 5, pose_x(3.0)).is_empty());
    // Un plan sans clé mais à une autre pose : des clés fixes à son début et à sa fin.
    let fixed = poskeys::join_keys(&[], 5, pose_x(3.0), &[], 5, pose_x(8.0));
    assert_eq!(fixed.iter().map(|k| (k.frame, k.t.x)).collect::<Vec<_>>(), vec![(0, 3.0), (4, 3.0), (5, 8.0), (9, 8.0)]);
}

#[test]
fn cutting_and_joining_a_panel_keeps_the_motion_of_an_animated_layer() {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Perso", LayerKind::Bitmap)];
    doc.panels[0].length = 12;
    doc.panels[0].layers[0].frames = vec![None; 12];
    doc.panels[0].layers[0].pos_keys = vec![PosKey::new(0, pose_x(0.0)), PosKey::new(10, pose_x(100.0))];
    let second = doc.split_panel(0, 6).expect("coupé");
    let (a, b) = (&doc.panels[0].layers[0], &doc.panels[second].layers[0]);
    assert!((a.pose(5).x - 50.0).abs() < 1e-3, "le premier plan finit où le mouvement en était");
    assert!((b.pose(0).x - 60.0).abs() < 1e-3 && b.pose(4).x == 100.0, "le second reprend à la coupe");
    // Réunis de nouveau : le même mouvement.
    doc.merge_with_next(0).expect("réunis");
    let l = &doc.panels[0].layers[0];
    assert!((l.pose(3).x - 30.0).abs() < 1.0, "{}", l.pose(3).x);
    assert!((l.pose(9).x - 90.0).abs() < 1.0, "{}", l.pose(9).x);
}

fn avec_cle(name: &str) -> App {
    let mut app = app(name);
    let mut doc = Document::new(400, 300, 24);
    let mut layer = Layer::new("Perso", LayerKind::Bitmap);
    layer.frames = vec![None; 12];
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = 12;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.tool = Tool::Transform;
    app
}

#[test]
fn the_position_key_button_records_the_pose_and_moving_the_layer_elsewhere_animates_it() {
    let mut app = avec_cle("cle-bouton");
    app.layers_mut()[0].transform = pose_x(10.0);
    let avant = app.history.names().0.len();
    app.run_action("calque.cle_position");
    assert_eq!(app.layers()[0].pos_keys.len(), 1, "une clé à l'image 0");
    assert_eq!(app.layers()[0].pos_keys[0].t.x, 10.0, "avec la pose actuelle");
    assert_eq!(app.history.names().0.len(), avant + 1, "une étape d'annulation");
    assert!(app.history.names().0.last().is_some_and(|n| n.contains("clé de position")), "{:?}", app.history.names().0.last());
    // Image 8, le calque est déplacé (les champs x / y, les poignées…) : une clé naît là.
    app.cur_frame = 8;
    app.set_layer_pose(0, pose_x(90.0));
    assert_eq!(app.layers()[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 8]);
    assert_eq!(app.layers()[0].pose(4).x, 50.0, "entre les deux clés : à mi-chemin");
    assert_eq!(app.layers()[0].transform.x, 10.0, "la pose fixe n'a pas bougé");
    // Annuler la pose de la clé du bouton : un seul geste.
    app.undo();
    assert!(app.layers()[0].pos_keys.is_empty(), "annulé : plus de clé");
    app.redo();
    // (Rétablir rend l'état d'avant l'annulation : la clé de l'image 8 y était aussi.)
    assert_eq!(app.layers()[0].pos_keys.len(), 2);
}

#[test]
fn deleting_smoothing_and_clearing_keys_are_named_undoable_steps() {
    let mut app = avec_cle("cle-gestes");
    app.run_action("calque.cle_position");
    app.cur_frame = 6;
    app.set_layer_pose(0, pose_x(60.0));
    app.cur_frame = 11;
    app.set_layer_pose(0, pose_x(110.0));
    // Lisser la clé de l'image 6.
    app.cur_frame = 6;
    let n = app.history.names().0.len();
    app.run_action("calque.cle_position_lisser");
    assert!(app.layers()[0].pos_keys[1].smooth);
    assert_eq!(app.history.names().0.len(), n + 1);
    app.run_action("calque.cle_position_lisser");
    assert!(!app.layers()[0].pos_keys[1].is_eased(), "une seconde fois : vitesse constante");
    // Supprimer la clé d'ici (une étape, annulable).
    let n = app.history.names().0.len();
    app.run_action("calque.cle_position_supprimer");
    assert_eq!(app.layers()[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 11]);
    assert_eq!(app.history.names().0.len(), n + 1);
    app.undo();
    assert_eq!(app.layers()[0].pos_keys.len(), 3);
    // Tout supprimer : le calque garde la pose qu'il a à l'image courante.
    app.cur_frame = 3;
    let at_3 = app.layers()[0].pose(3);
    app.run_action("calque.cles_position_supprimer");
    assert!(app.layers()[0].pos_keys.is_empty());
    assert_eq!(app.layers()[0].transform, at_3, "il ne saute pas à l'image courante");
}

#[test]
fn an_animated_layer_has_its_pose_in_every_plan_the_export_and_the_thumbnails_use() {
    let mut app = avec_cle("cle-rendu");
    let cel = app.doc.new_cel(LayerKind::Bitmap);
    app.doc.panels[0].layers[0].frames = vec![Some(cel); 12];
    app.layers_mut()[0].pos_keys = vec![PosKey::new(0, pose_x(0.0)), PosKey::new(10, pose_x(100.0))];
    for (frame, expected) in [(0usize, None), (5, Some(50.0f32)), (10, Some(100.0)), (11, Some(100.0))] {
        let plan = crate::render::panel_plan_at(&app.doc, 0, frame, None);
        let t = plan.items.first().and_then(|i| i.transform);
        match expected {
            None => assert!(t.is_none(), "à l'image 0 la pose est celle d'origine : {t:?}"),
            Some(x) => assert!((t.expect("transformée").x - x).abs() < 1e-3, "image {frame} : {t:?}"),
        }
    }
    // L'affichage de travail (build_plan) suit aussi l'image demandée, pas seulement l'image courante.
    let plan = app.build_plan(5, false, false);
    assert!((plan.items[0].transform.expect("transformée").x - 50.0).abs() < 1e-3);
    // La vignette d'un plan rangé aussi (la pile des versions).
    let panel = app.doc.panels[0].clone();
    let stored = crate::render::stored_panel_plan(&app.doc, &panel, 5, None);
    assert!((stored.items[0].transform.expect("transformée").x - 50.0).abs() < 1e-3);
}

#[test]
fn position_keys_are_saved_in_the_project_and_an_old_project_opens_without_keys() {
    assert!(crate::project::VERSION >= 33);
    let mut app = avec_cle("cle-projet");
    let mut keys = vec![PosKey::new(0, pose_x(0.0)), PosKey::new(8, LayerTransform { x: 40.0, y: -12.5, scale: 1.5, angle: 30.0, stretch_x: -1.2, stretch_y: 0.9, ..Default::default() })];
    keys[0].smooth = true;
    keys[1].ease_in = 3.0;
    app.layers_mut()[0].pos_keys = keys.clone();
    let dir = std::env::temp_dir().join(format!("shot-cles-position-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("C.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("cles_position"), "le champ est écrit");
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].pos_keys, keys, "les clés reviennent telles quelles");
    // Sans clé, le champ n'est pas écrit : un projet sans animation reste lisible par une version d'avant.
    app.layers_mut()[0].pos_keys.clear();
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    assert!(!std::fs::read_to_string(&manifest).unwrap().contains("cles_position"));
    assert!(crate::project::load(&manifest).unwrap().doc.panels[0].layers[0].pos_keys.is_empty());
    // Des clés abîmées (pas un nombre, deux sur la même image) se lisent sans planter.
    let clean = poskeys::from_entries(vec![
        poskeys::Entree { image: 3, x: f32::NAN, y: 0.0, echelle: 1.0, rotation: 0.0, etirement_x: 1.0, etirement_y: 1.0, inclinaison_x: 0.0, inclinaison_y: 0.0, ecart_profondeur: 0.0, lisse: false, adoucir_entree: 0.0, adoucir_sortie: 0.0 },
        poskeys::Entree { image: 5, x: 1.0, y: 0.0, echelle: 1.0, rotation: 0.0, etirement_x: 1.0, etirement_y: 1.0, inclinaison_x: 0.0, inclinaison_y: 0.0, ecart_profondeur: 0.0, lisse: false, adoucir_entree: 0.0, adoucir_sortie: 0.0 },
        poskeys::Entree { image: 5, x: 2.0, y: 0.0, echelle: 1.0, rotation: 0.0, etirement_x: 1.0, etirement_y: 1.0, inclinaison_x: 0.0, inclinaison_y: 0.0, ecart_profondeur: 0.0, lisse: false, adoucir_entree: 0.0, adoucir_sortie: 0.0 },
        poskeys::Entree { image: 1, x: 0.0, y: 0.0, echelle: 1.0, rotation: 0.0, etirement_x: 1.0, etirement_y: 1.0, inclinaison_x: 0.0, inclinaison_y: 0.0, ecart_profondeur: 0.0, lisse: false, adoucir_entree: 0.0, adoucir_sortie: 0.0 },
    ]);
    assert_eq!(clean.iter().map(|k| (k.frame, k.t.x)).collect::<Vec<_>>(), vec![(1, 0.0), (5, 2.0)]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn flipping_or_scaling_an_animated_layer_moves_all_its_keys() {
    let mut app = avec_cle("cle-miroir");
    let cel = app.doc.new_cel(LayerKind::Bitmap);
    app.doc.panels[0].layers[0].frames = vec![Some(cel); 12];
    app.layers_mut()[0].pos_keys = vec![PosKey::new(0, pose_x(10.0)), PosKey::new(6, pose_x(60.0))];
    app.run_action("calque.miroir_h");
    assert!(app.layers()[0].pos_keys.iter().all(|k| k.t.stretch_x < 0.0), "toutes les clés sont retournées : {:?}", app.layers()[0].pos_keys);
    // Une feuille agrandie : les décalages des clés suivent ceux du calque.
    let doc = &mut app.doc;
    doc.panels[0].layers[0].pos_keys = vec![PosKey::new(0, pose_x(10.0)), PosKey::new(6, pose_x(60.0))];
    let [fw, fh] = doc.frame;
    doc.resize_frame(fw * 2, fh * 2, true);
    assert_eq!(doc.panels[0].layers[0].pos_keys.iter().map(|k| k.t.x).collect::<Vec<_>>(), vec![20.0, 120.0]);
}

#[test]
fn timeline_markers_are_found_under_the_pointer_and_dragged_in_one_undo_step() {
    let mut app = avec_cle("cle-timeline");
    app.layers_mut()[0].pos_keys = vec![PosKey::new(2, pose_x(0.0)), PosKey::new(6, pose_x(50.0)), PosKey::new(9, pose_x(80.0))];
    let rows = vec![crate::layerheads::Row { li: 0, top: 0.0, h: 24.0 }];
    let (r, cell, grid_top) = (Rect::from_min_size(pos2(100.0, 40.0), vec2(600.0, 200.0)), 20.0, 60.0);
    let x_of = |f: usize| r.left() + (f as f32 + 0.5) * cell;
    let y = grid_top + 12.0;
    assert_eq!(app.pos_marker_in_timeline(pos2(x_of(6), y), r, cell, &rows, grid_top), Some((0, 6)));
    assert_eq!(app.pos_marker_in_timeline(pos2(x_of(6) + 6.0, y + 3.0), r, cell, &rows, grid_top), Some((0, 6)), "un peu à côté, c'est le losange");
    assert_eq!(app.pos_marker_in_timeline(pos2(x_of(4), y), r, cell, &rows, grid_top), None, "entre deux losanges : rien");
    assert_eq!(app.pos_marker_in_timeline(pos2(x_of(6), y + 30.0), r, cell, &rows, grid_top), None, "hors de la ligne");
    // Un clic choisit le losange et y met la tête de lecture.
    app.click_pos_marker(0, 6, false);
    assert_eq!((app.cur_frame, app.pos_ui.sel.clone()), (6, vec![(app.layers()[0].id, 6)]));
    // Ctrl + clic en ajoute un.
    app.click_pos_marker(0, 9, true);
    assert_eq!(app.pos_ui.sel.len(), 2);
    // Glisser le losange 9 de deux images : les deux clés choisies bougent ensemble, une étape.
    let n = app.history.names().0.len();
    assert!(app.begin_pos_drag(0, 9, 9));
    app.drag_pos_keys(11);
    assert_eq!(app.layers()[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 8, 11]);
    app.drag_pos_keys(40);
    assert_eq!(app.layers()[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 8, 11], "bloqué à la dernière image du plan (11)");
    app.end_pos_drag();
    assert_eq!(app.history.names().0.len(), n + 1);
    assert!(app.history.names().0.last().is_some_and(|nm| nm.contains("clés de position")), "{:?}", app.history.names().0.last());
    assert!(app.pos_keys_have_focus() || !app.images_focus);
    app.undo();
    assert_eq!(app.layers()[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 6, 9]);
    // Suppr sur les clés choisies de la timeline.
    app.click_pos_marker(0, 6, false);
    app.run_action("edition.supprimer");
    assert_eq!(app.layers()[0].pos_keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 9]);
}

#[test]
fn the_transform_tool_gesture_on_an_animated_layer_sets_the_key_of_the_current_frame() {
    let mut b = banc_dessin(LayerKind::Bitmap);
    deux_taches(&mut b);
    b.app.tool = Tool::Transform;
    b.app.layers_mut()[0].pos_keys = vec![PosKey::new(0, LayerTransform::default())];
    b.app.cur_frame = 3;
    b.chauffer(3);
    // Prendre le calque par le milieu de la tache B (dans sa boîte) et le tirer de 40 px à droite.
    let (a, c) = (ecran(&b, 230.0, 120.0), ecran(&b, 270.0, 120.0));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=4 {
        let p = a + (c - a) * (k as f32 / 4.0);
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    }
    b.image(vec![bouton(c, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let keys = &b.app.layers()[0].pos_keys;
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 3], "une clé est née à l'image 3");
    assert!((keys[1].t.x - 40.0).abs() < 1.5, "40 px à droite : {}", keys[1].t.x);
    assert_eq!(keys[0].t.x, 0.0, "la clé du début n'a pas bougé");
    assert_eq!(b.app.layers()[0].transform, LayerTransform::default(), "la pose fixe non plus");
    // Un seul geste : une seule étape.
    b.app.undo();
    assert_eq!(b.app.layers()[0].pos_keys.len(), 1);
}

#[test]
fn the_timeline_and_the_tool_panel_draw_with_position_keys_without_trouble() {
    // Une fenêtre plus haute que `banc_dessin` : sous la bande d'onglets, les réglages de l'outil
    // Transformer descendent jusqu'au groupe « Clés de position ».
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
    doc.panels[0].length = 4;
    let mut b = banc_taille(doc, false, 1280, 1000);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(10);
    b.app.layers_mut()[0].pos_keys = vec![PosKey::new(0, pose_x(0.0)), PosKey::new(3, pose_x(20.0))];
    b.app.tool = Tool::Transform;
    // (Les clés de position sont dans « Plus de réglages » de l'outil, replié d'office : déplié ici.)
    b.app.pinceaux.regler_plus(Tool::Transform.action(), true);
    b.garder_textes = true;
    b.chauffer(6);
    assert!(b.textes.iter().any(|t| t.contains("Clés de position")), "le groupe de réglages est là : {:?}", b.textes.iter().filter(|t| t.len() < 40).collect::<Vec<_>>());
}
// ------------------------------------------------------------------------------------ sans latence

#[test]
fn laying_down_a_long_straight_segment_and_choosing_an_object_cost_one_ordinary_stroke() {
    use std::time::Instant;
    let mut app = app_remplir("droit-temps");
    app.tool = Tool::Brush;
    app.brush.size = 24.0;
    // Une ligne de 380 pixels : l'aperçu pendant le geste ne fait rien de plus que viser ; la pose, au relâché,
    // est un trait ordinaire.
    app.begin_paint(pos2(10.0, 150.0), 1.0, false);
    app.droit_start(pos2(10.0, 150.0));
    let t = Instant::now();
    for k in 0..200 {
        app.stroke_point(pos2(10.0 + k as f32 * 1.9, 150.0 + (k as f32 * 0.3).sin() * 40.0), 1.0, &Modifiers::SHIFT, false);
    }
    let aim = t.elapsed();
    assert!(app.droit.pending());
    let t = Instant::now();
    app.end_stroke();
    let lay = t.elapsed();
    println!("tracé droit : viser 200 points {aim:?}, poser le segment et finir le trait {lay:?}");
    assert!(aim.as_millis() < 20, "viser ne dessine rien : {aim:?}");
    assert!(lay.as_millis() < 250, "la pose d'un segment de 380 px reste celle d'un trait : {lay:?}");
    // Choisir un objet : la forme d'un dessin de 1920 × 1080 d'un tenant, au clic.
    let mut r = crate::raster::Raster::new(1920, 1080);
    for y in 100..1000 {
        for x in 100..1800 {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgb(10, 10, 10);
        }
    }
    let t = Instant::now();
    let (mask, _) = crate::selobjet::object_at(&r, 500, 500).expect("une forme");
    let pick = t.elapsed();
    println!("choisir un objet de 1,5 million de pixels : {pick:?}");
    assert!(mask.at(500, 500) > 0.0);
    assert!(pick.as_millis() < 600, "un clic sur un très grand objet : {pick:?}");
}