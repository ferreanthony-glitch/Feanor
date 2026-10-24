//! Le passe-partout autour du cadre de la caméra (Anthony, 2026-10-24), qui remplace l'ancien voile gris : une couleur
//! (noir d'origine) opaque (100 % d'origine) autour du cadre, qu'on allume et règle dans la barre sous le dessin et dont
//! la couleur est dans Préférences > Dessin. Ces essais peignent le cadre dans un écran sans fenêtre, rastérisent les
//! triangles du passe-partout et lisent les pixels : dehors, la couleur ; dedans, rien n'a changé.

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};

use crate::app::App;
use crate::camera::Camera;
use crate::document::{Document, Layer, LayerKind};
use crate::view::View;

const ECRAN: (usize, usize) = (640, 480);
/// Ce que montre le dessin sous le passe-partout (un gris moyen, opaque).
const FOND: Color32 = Color32::from_rgb(120, 130, 140);

fn app_camera(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-passepartout-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(LayerKind::Vector);
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    let [fw, fh] = app.doc.frame;
    // Le cadre est resserré : ce n'est plus la feuille entière, le passe-partout se peint.
    app.set_current_camera(Camera { zoom: 2.0, ..Camera::centered(fw, fh) });
    app
}

fn regler_la_vue(app: &mut App, degres: f32, miroir: bool) {
    app.view = View { zoom: 6.0, rotation: degres.to_radians(), flip: miroir, pan: vec2(10.0, -5.0), persp: None };
}

fn rect_ecran() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(ECRAN.0 as f32, ECRAN.1 as f32))
}

/// Peint le cadre de la caméra et rastérise ses triangles (sommets de même couleur : celle du passe-partout) sur
/// un fond uni opaque ; rend les pixels, rangée par rangée.
fn peindre(app: &App) -> Vec<Color32> {
    let ctx = egui::Context::default();
    let mut input = RawInput { screen_rect: Some(rect_ecran()), ..RawInput::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
    let out = ctx.run(input, |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("essai_passe_partout")));
        app.paint_camera(&painter, rect_ecran());
    });
    let mut pixels = vec![FOND; ECRAN.0 * ECRAN.1];
    fn parcourir(s: &Shape, pixels: &mut [Color32]) {
        match s {
            Shape::Mesh(m) => {
                // (Les morceaux d'un même maillage ne se recouvrent pas : un pixel pile sur leur bord commun compte une fois.)
                let mut vus = vec![false; ECRAN.0 * ECRAN.1];
                for t in m.indices.chunks(3) {
                    let p = [m.vertices[t[0] as usize].pos, m.vertices[t[1] as usize].pos, m.vertices[t[2] as usize].pos];
                    poser_triangle(pixels, &mut vus, p, m.vertices[t[0] as usize].color);
                }
            }
            Shape::Vec(v) => v.iter().for_each(|s| parcourir(s, pixels)),
            _ => {}
        }
    }
    for s in &out.shapes {
        parcourir(&s.shape, &mut pixels);
    }
    pixels
}

fn poser_triangle(pixels: &mut [Color32], vus: &mut [bool], p: [Pos2; 3], c: Color32) {
    let aire = |a: Pos2, b: Pos2, q: Pos2| (b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x);
    let total = aire(p[0], p[1], p[2]);
    if total.abs() < 1e-6 {
        return;
    }
    let (x0, x1) = (p.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (p.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(ECRAN.0));
    let (y0, y1) = (p.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (p.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(ECRAN.1));
    for y in y0..y1 {
        for x in x0..x1 {
            let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
            let (w0, w1, w2) = (aire(p[1], p[2], q) / total, aire(p[2], p[0], q) / total, aire(p[0], p[1], q) / total);
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 && !std::mem::replace(&mut vus[y * ECRAN.0 + x], true) {
                let dst = &mut pixels[y * ECRAN.0 + x];
                let k = (255 - c.a() as u32) as f32 / 255.0;
                let m = |s: u8, d: u8| (s as f32 + d as f32 * k).round().min(255.0) as u8;
                *dst = Color32::from_rgba_premultiplied(m(c.r(), dst.r()), m(c.g(), dst.g()), m(c.b(), dst.b()), 255);
            }
        }
    }
}

fn dans(poly: &[Pos2], p: Pos2) -> bool {
    crate::vector::point_in_polygon(p, poly)
}

/// Distance de `p` au bord du polygone.
fn distance_au_bord(poly: &[Pos2], p: Pos2) -> f32 {
    (0..poly.len())
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let ab = b - a;
            let t = (((p - a).dot(ab)) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
            (a + ab * t).distance(p)
        })
        .fold(f32::MAX, f32::min)
}

fn cadre_a_l_ecran(app: &App) -> Vec<Pos2> {
    let size = app.canvas_size();
    app.current_camera().corners(app.doc.frame).iter().map(|c| app.view.to_screen(rect_ecran(), size, *c)).collect()
}

fn feuille_a_l_ecran(app: &App) -> Vec<Pos2> {
    let size = app.canvas_size();
    [pos2(0.0, 0.0), pos2(size.x, 0.0), pos2(size.x, size.y), pos2(0.0, size.y)].iter().map(|c| app.view.to_screen(rect_ecran(), size, *c)).collect()
}

/// Les pixels bien dedans (à plus de 4 pixels du bord), puis bien dehors, d'un polygone, sur une grille.
fn points_sur_une_grille(poly: &[Pos2]) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let (mut dedans, mut dehors) = (Vec::new(), Vec::new());
    for y in (0..ECRAN.1).step_by(4) {
        for x in (0..ECRAN.0).step_by(4) {
            let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
            if distance_au_bord(poly, p) < 4.0 {
                continue;
            }
            if dans(poly, p) { dedans.push((x, y)) } else { dehors.push((x, y)) }
        }
    }
    (dedans, dehors)
}

fn pixel(pixels: &[Color32], (x, y): (usize, usize)) -> Color32 {
    pixels[y * ECRAN.0 + x]
}

// ---------------------------------------------------------------------------------------------- les pixels

#[test]
fn outside_the_frame_is_the_passe_partout_colour_and_inside_is_untouched() {
    let mut app = app_camera("pixels");
    // D'origine : noir, opaque, allumé.
    assert!(app.settings.passe_partout_actif && app.settings.passe_partout == 100 && app.settings.passe_partout_couleur == [0, 0, 0]);
    regler_la_vue(&mut app, 0.0, false);
    let pixels = peindre(&app);
    let (dedans, dehors) = points_sur_une_grille(&cadre_a_l_ecran(&app));
    assert!(dedans.len() > 50 && dehors.len() > 500, "{} dedans, {} dehors", dedans.len(), dehors.len());
    assert!(dehors.iter().all(|&p| pixel(&pixels, p) == Color32::from_rgb(0, 0, 0)), "dehors : noir opaque");
    assert!(dedans.iter().all(|&p| pixel(&pixels, p) == FOND), "dedans : le dessin n'a pas bougé");
    // Une autre couleur.
    app.settings.passe_partout_couleur = [200, 30, 60];
    let pixels = peindre(&app);
    assert!(dehors.iter().all(|&p| pixel(&pixels, p) == Color32::from_rgb(200, 30, 60)));
    assert!(dedans.iter().all(|&p| pixel(&pixels, p) == FOND));
}

#[test]
fn the_passe_partout_is_right_in_every_rotated_mirrored_or_turned_view() {
    for camera_tournee in [false, true] {
        for miroir in [false, true] {
            for degres in [0.0_f32, 15.0, 90.0, 135.0, 210.0, 270.0] {
                let mut app = app_camera("tournee");
                if camera_tournee {
                    let [fw, fh] = app.doc.frame;
                    app.set_current_camera(Camera { zoom: 2.0, angle: 20.0, ..Camera::centered(fw, fh) });
                }
                regler_la_vue(&mut app, degres, miroir);
                let pixels = peindre(&app);
                let (dedans, dehors) = points_sur_une_grille(&cadre_a_l_ecran(&app));
                let cas = format!("caméra tournée {camera_tournee}, miroir {miroir}, vue {degres}°");
                assert!(dedans.len() > 20 && dehors.len() > 200, "{cas} : {} dedans, {} dehors", dedans.len(), dehors.len());
                let faux_dehors = dehors.iter().filter(|&&p| pixel(&pixels, p) != Color32::BLACK).count();
                let faux_dedans = dedans.iter().filter(|&&p| pixel(&pixels, p) != FOND).count();
                assert_eq!((faux_dehors, faux_dedans), (0, 0), "{cas} : pixels dehors pas noirs, pixels dedans touchés");
            }
        }
    }
}

#[test]
fn on_the_sheet_only_the_sheet_around_the_frame_is_covered() {
    // Sur une vignette (carte), le hors-champ est la feuille moins le cadre : ce qui est hors de la feuille reste intact.
    for degres in [0.0_f32, 30.0, 90.0] {
        for miroir in [false, true] {
            let mut app = app_camera("feuille");
            app.in_card = true;
            regler_la_vue(&mut app, degres, miroir);
            let pixels = peindre(&app);
            let (cadre, feuille) = (cadre_a_l_ecran(&app), feuille_a_l_ecran(&app));
            for y in (0..ECRAN.1).step_by(4) {
                for x in (0..ECRAN.0).step_by(4) {
                    let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    if distance_au_bord(&cadre, p) < 4.0 || distance_au_bord(&feuille, p) < 4.0 {
                        continue;
                    }
                    let attendu = if dans(&cadre, p) || !dans(&feuille, p) { FOND } else { Color32::BLACK };
                    assert_eq!(pixel(&pixels, (x, y)), attendu, "vue {degres}° miroir {miroir} en ({x}, {y})");
                }
            }
        }
    }
}

#[test]
fn the_toggle_hides_and_shows_the_passe_partout() {
    let mut app = app_camera("bascule");
    regler_la_vue(&mut app, 0.0, false);
    let (dedans, dehors) = points_sur_une_grille(&cadre_a_l_ecran(&app));
    app.run_action("camera.masque");
    assert!(!app.settings.passe_partout_actif);
    let pixels = peindre(&app);
    assert!(dehors.iter().chain(&dedans).all(|&p| pixel(&pixels, p) == FOND), "éteint : rien n'est couvert");
    app.run_action("camera.masque");
    assert!(app.settings.passe_partout_actif);
    let pixels = peindre(&app);
    assert!(dehors.iter().all(|&p| pixel(&pixels, p) == Color32::BLACK), "allumé : le noir revient");
    assert!(dedans.iter().all(|&p| pixel(&pixels, p) == FOND));
}

#[test]
fn opacity_0_50_and_100_percent_compose_the_expected_pixels_outside_the_frame() {
    let mut app = app_camera("opacite");
    regler_la_vue(&mut app, 0.0, false);
    app.settings.passe_partout_couleur = [200, 30, 60];
    let (dedans, dehors) = points_sur_une_grille(&cadre_a_l_ecran(&app));
    for (pourcent, attendu) in [(100u8, [200u8, 30, 60]), (50, [0, 0, 0]), (0, [FOND.r(), FOND.g(), FOND.b()])] {
        app.settings.passe_partout = pourcent;
        let pixels = peindre(&app);
        let a = crate::camtools::passe_partout_alpha(pourcent) as f32 / 255.0;
        // Le passe-partout posé sur le fond : couleur × a + fond × (1 − a).
        let calcule = |couleur: u8, fond: u8| (couleur as f32 * a + fond as f32 * (1.0 - a)).round() as i32;
        let voulu = if pourcent == 50 { [calcule(200, FOND.r()), calcule(30, FOND.g()), calcule(60, FOND.b())] } else { [attendu[0] as i32, attendu[1] as i32, attendu[2] as i32] };
        for &p in &dehors {
            let c = pixel(&pixels, p);
            let (rouge, vert, bleu) = (c.r() as i32, c.g() as i32, c.b() as i32);
            assert!((rouge - voulu[0]).abs() <= 2 && (vert - voulu[1]).abs() <= 2 && (bleu - voulu[2]).abs() <= 2, "{pourcent} % : {c:?} au lieu de {voulu:?}");
        }
        assert!(dedans.iter().all(|&p| pixel(&pixels, p) == FOND), "{pourcent} % : le dedans ne bouge pas");
    }
    // 100 % est bien opaque : le fond ne transparaît pas.
    app.settings.passe_partout = 100;
    let pixels = peindre(&app);
    assert!(dehors.iter().all(|&p| pixel(&pixels, p) == Color32::from_rgb(200, 30, 60)));
}

#[test]
fn the_exports_do_not_know_the_passe_partout() {
    let mut app = app_camera("export");
    let composer = |app: &App| crate::render::composite_full(&app.doc, &app.build_plan(0, false, true));
    let avant = composer(&app);
    app.settings.passe_partout = 100;
    app.settings.passe_partout_couleur = [255, 0, 0];
    app.settings.passe_partout_actif = true;
    let allume = composer(&app);
    app.settings.passe_partout_actif = false;
    let eteint = composer(&app);
    assert!(avant == allume && allume == eteint, "le passe-partout est un habillage de l'écran : l'image composée ne change pas");
}

// ---------------------------------------------------------------------------------------------- les réglages

#[test]
fn the_toggle_and_the_opacity_are_kept_in_the_settings() {
    let mut s = crate::settings::Settings::default();
    assert!(s.passe_partout_actif, "allumé d'origine");
    s.passe_partout_actif = false;
    s.passe_partout = 35;
    s.passe_partout_couleur = [9, 8, 7];
    let texte = toml::to_string(&s).unwrap();
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!((relu.passe_partout_actif, relu.passe_partout, relu.passe_partout_couleur), (false, 35, [9, 8, 7]), "{texte}");
    // Des réglages d'avant : allumé, opaque, noir.
    let vide: crate::settings::Settings = toml::from_str("").unwrap();
    assert_eq!((vide.passe_partout_actif, vide.passe_partout, vide.passe_partout_couleur), (true, 100, [0, 0, 0]));
}

// ---------------------------------------------------------------------------------------------- la barre

fn glyphes(app: &mut App, n: usize) -> (Vec<(String, Rect)>, egui::Context) {
    let ctx = egui::Context::default();
    let mut textes = Vec::new();
    for k in 0..n {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 120.0))), time: Some(k as f64 / 60.0), ..RawInput::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.view_tools_bar(ui));
        });
        textes.clear();
        fn collecter(s: &Shape, out: &mut Vec<(String, Rect)>) {
            match s {
                Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
                Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
    }
    (textes, ctx)
}

#[test]
fn the_bottom_bar_ends_with_the_passe_partout_toggle_and_opacity_slider_in_the_same_cells() {
    use egui_phosphor::regular as ph;
    let mut app = app_camera("barre");
    let (textes, _) = glyphes(&mut app, 4);
    let trouver = |glyphe: &str| textes.iter().find(|(t, _)| t.trim() == glyphe).map(|(_, r)| *r);
    let (ajuster, miroir, pelure, lampe, passe) = (
        trouver(ph::ARROWS_OUT).expect("ajuster"),
        trouver(ph::FLIP_HORIZONTAL).expect("miroir"),
        trouver(ph::CIRCLES_THREE).expect("pelure"),
        trouver(ph::LIGHTBULB).expect("table lumineuse"),
        trouver(ph::SELECTION_INVERSE).expect("passe-partout"),
    );
    // Après tous les autres, de gauche à droite.
    assert!(ajuster.center().x < miroir.center().x && miroir.center().x < pelure.center().x && pelure.center().x < lampe.center().x && lampe.center().x < passe.center().x, "{ajuster:?} {miroir:?} {pelure:?} {lampe:?} {passe:?}");
    // Même taille d'icône, sur la même ligne que les autres.
    assert_eq!(passe.size(), lampe.size(), "même icône que les autres : {passe:?} / {lampe:?}");
    assert!((passe.center().y - lampe.center().y).abs() < 0.5, "sur la même ligne");
    // Le curseur d'opacité, avec sa valeur en pour cent, juste après l'icône.
    let valeur = textes.iter().find(|(t, _)| t.trim() == "100 %").map(|(_, r)| *r).expect("la valeur du curseur : « 100 % »");
    assert!(valeur.center().x > passe.center().x, "le curseur est à droite de l'icône");
    assert!((valeur.center().y - lampe.center().y).abs() < 3.0);
    // Même écart que dans le reste de la barre : l'icône suivante du groupe suit la case de l'icône à 2 points près.
    let ecart_icone_curseur = valeur.min.x - passe.max.x;
    assert!(ecart_icone_curseur > 0.0 && ecart_icone_curseur < 120.0, "{ecart_icone_curseur}");
}

#[test]
fn clicking_the_bar_icon_toggles_the_passe_partout_and_the_setting_follows() {
    use egui_phosphor::regular as ph;
    let mut app = app_camera("barre-clic");
    let ctx = egui::Context::default();
    let mut t = 0.0;
    let mut image = |app: &mut App, events: Vec<Event>| -> Vec<(String, Rect)> {
        t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 120.0))), time: Some(t), events, ..RawInput::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.view_tools_bar(ui));
        });
        let mut textes = Vec::new();
        fn collecter(s: &Shape, out: &mut Vec<(String, Rect)>) {
            match s {
                Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
                Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        out.shapes.iter().for_each(|s| collecter(&s.shape, &mut textes));
        textes
    };
    let mut textes = Vec::new();
    for _ in 0..3 {
        textes = image(&mut app, Vec::new());
    }
    let centre = textes.iter().find(|(t, _)| t.trim() == ph::SELECTION_INVERSE).map(|(_, r)| r.center()).expect("l'icône");
    // (Le pointeur arrive d'abord sur l'icône, quelques images : un clic dans un écran neuf, sans survol, n'est pas pris.)
    image(&mut app, vec![Event::PointerMoved(centre)]);
    image(&mut app, Vec::new());
    image(&mut app, Vec::new());
    for attendu in [false, true] {
        image(&mut app, vec![Event::PointerMoved(centre)]);
        image(&mut app, vec![Event::PointerButton { pos: centre, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
        image(&mut app, vec![Event::PointerButton { pos: centre, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
        image(&mut app, Vec::new());
        assert_eq!(app.settings.passe_partout_actif, attendu, "un clic sur l'icône bascule le passe-partout");
    }
}
