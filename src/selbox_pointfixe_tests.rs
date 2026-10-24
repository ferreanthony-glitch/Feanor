//! Le point fixe des poignées et le centre des rotations (Anthony, 2026-10-23) : deux réglages de l'application
//! (`selbox::PointFixe`, `selbox::CentreRotation`), pour la boîte de la Sélection (vectorielle, bitmap, plusieurs
//! calques) comme pour l'outil Transformer.
//! - « Poignée opposée » (d'office) : un coin ou un côté tiré garde la poignée opposée en place ;
//! - « Centre » : tout se fait autour du pivot — exactement ce que faisait Ctrl tenu jusqu'ici ; Ctrl tenu donne
//!   maintenant l'autre mode que celui du réglage ;
//! - « Rotation autour de » : le pivot (d'office) ou, pour une rotation prise hors d'un coin, le coin opposé.

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};

use crate::app::{App, Mode, Tool};
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::layertools::{stretched, BoxHandle};
use crate::perf_tests::{banc_taille, Banc};
use crate::selbox::*;
use crate::vector::Affine;

fn near(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 1e-2
}

fn rect() -> Rect {
    Rect::from_min_max(pos2(100.0, 100.0), pos2(300.0, 200.0))
}

fn mods(point: PointFixe, rotation: CentreRotation) -> Mods {
    Mods::with(false, false, false, point, rotation)
}

const OPPOSEE: PointFixe = PointFixe::Opposee;
const CENTRE: PointFixe = PointFixe::Centre;
const PIVOT: CentreRotation = CentreRotation::Pivot;
const COIN: CentreRotation = CentreRotation::CoinOppose;

/// Une boîte droite, une boîte tournée et inclinée, une boîte au pivot déplacé.
fn boites() -> Vec<SelFrame> {
    let droite = SelFrame::new(rect());
    let tournee = droite.then(&Affine::rotate_about(pos2(200.0, 150.0), 30f32.to_radians())).then(&Affine { a: 1.0, b: 0.2, c: 0.0, d: 1.0, tx: -30.0, ty: 0.0 });
    let pivot = SelFrame { pivot: Some(pos2(130.0, 120.0)), ..droite };
    vec![droite, tournee, pivot]
}

// ------------------------------------------------------------------------------------ la géométrie

#[test]
fn opposite_mode_a_corner_drag_keeps_the_opposite_corner_in_place() {
    for f in boites() {
        for k in 0..4 {
            let c = f.corners();
            let (start, to) = (c[k], c[k] + vec2(37.0, -23.0));
            for m in [mods(OPPOSEE, PIVOT), Mods { shift: true, ..mods(OPPOSEE, PIVOT) }, Mods { keep: true, ..mods(OPPOSEE, PIVOT) }] {
                let after = f.then(&gesture(Handle::Corner(k), &f, start, to, m)).corners();
                assert!(near(after[(k + 2) % 4], c[(k + 2) % 4]), "coin {k} tiré ({m:?}) : le coin opposé reste {:?} → {:?}", c[(k + 2) % 4], after[(k + 2) % 4]);
            }
            // Sans proportions, le coin tiré suit la souris.
            let after = f.then(&gesture(Handle::Corner(k), &f, start, to, mods(OPPOSEE, PIVOT))).corners();
            assert!(near(after[k], to), "le coin tiré va sous la souris : {:?} / {to:?}", after[k]);
        }
    }
}

#[test]
fn opposite_mode_a_side_drag_keeps_the_opposite_side_in_place() {
    for f in boites() {
        for k in 0..4 {
            let mid = f.m.apply(f.mid_local(k));
            let opposite = f.m.apply(f.mid_local(k + 2));
            let (a, b) = (f.corners()[(k + 2) % 4], f.corners()[(k + 3) % 4]);
            // Mettre à l'échelle par le milieu du côté : le côté opposé (ses deux coins) ne bouge pas.
            let after = f.then(&gesture(Handle::Edge(k), &f, mid, mid + vec2(25.0, 31.0), mods(OPPOSEE, PIVOT)));
            assert!(near(after.m.apply(f.mid_local(k + 2)), opposite), "côté {k} : le milieu opposé reste");
            assert!(near(after.corners()[(k + 2) % 4], a) && near(after.corners()[(k + 3) % 4], b), "côté {k} : les coins du côté opposé restent");
            // Incliner par le bord : le bord opposé ne bouge pas non plus.
            let after = f.then(&gesture(Handle::Skew(k), &f, mid, mid + vec2(18.0, -12.0), mods(OPPOSEE, PIVOT)));
            assert!(near(after.corners()[(k + 2) % 4], a) && near(after.corners()[(k + 3) % 4], b), "bord {k} incliné : le bord opposé reste");
        }
    }
}

#[test]
fn centre_mode_scales_and_skews_about_the_pivot() {
    for f in boites() {
        let p = f.pivot_world();
        for k in 0..4 {
            let c = f.corners()[k];
            let mid = f.m.apply(f.mid_local(k));
            for (h, start) in [(Handle::Corner(k), c), (Handle::Edge(k), mid), (Handle::Skew(k), mid)] {
                let after = f.then(&gesture(h, &f, start, start + vec2(29.0, 17.0), mods(CENTRE, PIVOT)));
                assert!(near(after.pivot_world(), p), "{h:?} en mode Centre : le pivot reste {p:?} → {:?}", after.pivot_world());
            }
        }
    }
}

#[test]
fn centre_mode_is_exactly_the_old_ctrl_gesture_and_ctrl_now_gives_the_other_mode() {
    let probes = [pos2(0.0, 0.0), pos2(123.0, 45.0), pos2(310.0, 260.0)];
    let same = |a: &Affine, b: &Affine| probes.iter().all(|p| near(a.apply(*p), b.apply(*p)));
    // L'ancien Ctrl : `Mods { ctrl: true }` sans réglage (le comportement d'avant ce réglage).
    let ancien_ctrl = Mods { ctrl: true, ..Mods::default() };
    let ancien = Mods::default();
    for f in boites() {
        for k in 0..4 {
            let c = f.corners()[k];
            let mid = f.m.apply(f.mid_local(k));
            for (h, start) in [(Handle::Corner(k), c), (Handle::Edge(k), mid), (Handle::Skew(k), mid), (Handle::Rotate(k), c + vec2(9.0, 9.0)), (Handle::Move, f.pivot_world())] {
                let to = start + vec2(-21.0, 34.0);
                let g = |m: Mods| gesture(h, &f, start, to, m);
                assert!(same(&g(mods(CENTRE, PIVOT)), &g(ancien_ctrl)), "{h:?} : « Centre » = l'ancien Ctrl");
                assert!(same(&g(mods(OPPOSEE, PIVOT)), &g(ancien)), "{h:?} : « Poignée opposée » = l'ancien geste sans touche");
                assert!(same(&g(Mods { ctrl: true, ..mods(OPPOSEE, PIVOT) }), &g(mods(CENTRE, PIVOT))), "{h:?} : Ctrl en mode opposé = centre");
                assert!(same(&g(Mods { ctrl: true, ..mods(CENTRE, PIVOT) }), &g(mods(OPPOSEE, PIVOT))), "{h:?} : Ctrl en mode centre = opposé");
            }
        }
    }
}

#[test]
fn rotation_turns_about_the_pivot_or_about_the_opposite_corner() {
    for f in boites() {
        let p = f.pivot_world();
        for k in 0..4 {
            let c = f.corners();
            let start = c[k] + (c[k] - p).normalized() * 12.0;
            let to = start + vec2(-40.0, 25.0);
            // Pivot : le pivot reste.
            let d = gesture(Handle::Rotate(k), &f, start, to, mods(OPPOSEE, PIVOT));
            assert!(near(d.apply(p), p), "rotation {k} autour du pivot");
            assert!(f.then(&d).params().angle != f.params().angle, "elle tourne");
            // Coin opposé : ce coin-là reste, le pivot bouge.
            let d = gesture(Handle::Rotate(k), &f, start, to, mods(OPPOSEE, COIN));
            let after = f.then(&d).corners();
            assert!(near(after[(k + 2) % 4], c[(k + 2) % 4]), "rotation {k} autour du coin opposé : {:?} → {:?}", c[(k + 2) % 4], after[(k + 2) % 4]);
            assert!(!near(d.apply(p), p), "le pivot tourne avec la boîte");
            assert!(near(rotation_centre(&f, k, true), c[(k + 2) % 4]) && near(rotation_centre(&f, k, false), p));
            // Le point fixe des poignées ne change rien à la rotation ; Maj garde ses pas de 15°.
            let e = gesture(Handle::Rotate(k), &f, start, to, mods(CENTRE, COIN));
            assert!(near(e.apply(c[(k + 2) % 4]), c[(k + 2) % 4]));
            let s = gesture(Handle::Rotate(k), &f, start, to, Mods { shift: true, ..mods(OPPOSEE, COIN) });
            let turn = (f.then(&s).params().angle - f.params().angle).rem_euclid(15.0);
            assert!(turn < 1e-2 || 15.0 - turn < 1e-2, "par 15° : {turn}");
        }
    }
}

// ------------------------------------------------------------------------------------ les réglages

#[test]
fn the_two_settings_are_app_settings_with_their_defaults_and_old_files_still_load() {
    let s = crate::settings::Settings::default();
    assert_eq!((s.point_fixe, s.rotation_autour), (PointFixe::Opposee, CentreRotation::Pivot));
    let texte = toml::to_string(&s).unwrap();
    assert!(!texte.contains("point_fixe_poignees") && !texte.contains("rotation_autour_de"), "les valeurs d'office ne s'écrivent pas");
    let s = crate::settings::Settings { point_fixe: PointFixe::Centre, rotation_autour: CentreRotation::CoinOppose, ..Default::default() };
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("point_fixe_poignees = \"centre\"") && texte.contains("rotation_autour_de = \"coin_oppose\""), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!((relu.point_fixe, relu.rotation_autour), (PointFixe::Centre, CentreRotation::CoinOppose));
    let ancien: crate::settings::Settings = toml::from_str("copie_secours_minutes = 3\n").unwrap();
    assert_eq!((ancien.point_fixe, ancien.rotation_autour), (PointFixe::Opposee, CentreRotation::Pivot), "un ancien fichier : les valeurs d'office");
}

fn appli(nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-pointfixe-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

#[test]
fn the_selection_box_gestures_read_the_settings() {
    let mut app = appli("mods");
    assert!(!app.box_mods(false, false).from_pivot(), "d'office : la poignée opposée");
    assert!(app.box_mods(false, true).from_pivot(), "Ctrl : le centre");
    assert!(!app.box_mods(false, false).coin);
    app.settings.point_fixe = PointFixe::Centre;
    app.settings.rotation_autour = CentreRotation::CoinOppose;
    assert!(app.box_mods(false, false).from_pivot());
    assert!(!app.box_mods(false, true).from_pivot(), "Ctrl : la poignée opposée");
    assert!(app.box_mods(true, false).coin && app.box_mods(true, false).shift);
}

#[test]
fn the_layer_stretch_keeps_the_opposite_side_or_the_centre() {
    let pivot = pos2(100.0, 50.0);
    let b = Rect::from_min_max(pos2(80.0, 40.0), pos2(120.0, 60.0));
    for t0 in [LayerTransform::default(), LayerTransform { angle: 25.0, scale: 1.4, x: 6.0, y: -3.0, ..Default::default() }] {
        let a0 = t0.affine(pivot);
        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()];
        let mids = [b.center_top(), b.right_center(), b.center_bottom(), b.left_center()];
        for k in 0..4 {
            let m = corners[k] + vec2(13.0, 9.0);
            let t = stretched(t0, pivot, b, BoxHandle::Corner(k), m, false, false);
            assert!(t.affine(pivot).apply(corners[(k + 2) % 4]).distance(a0.apply(corners[(k + 2) % 4])) < 1e-3, "coin {k} : le coin opposé reste");
            let t = stretched(t0, pivot, b, BoxHandle::Corner(k), m, false, true);
            assert!(t.affine(pivot).apply(b.center()).distance(a0.apply(b.center())) < 1e-3, "coin {k}, mode Centre : le centre reste");
            let m = mids[k] + vec2(11.0, 7.0);
            let t = stretched(t0, pivot, b, BoxHandle::Edge(k), m, false, false);
            assert!(t.affine(pivot).apply(mids[(k + 2) % 4]).distance(a0.apply(mids[(k + 2) % 4])) < 1e-3, "côté {k} : le côté opposé reste");
            let t = stretched(t0, pivot, b, BoxHandle::Edge(k), m, false, true);
            assert!(t.affine(pivot).apply(b.center()).distance(a0.apply(b.center())) < 1e-3, "côté {k}, mode Centre : le centre reste");
        }
    }
}

// ------------------------------------------------------------------------------------ l'outil Transformer, à l'écran

fn banc_transformer() -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
    doc.panels[0].length = 4;
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    let cel = b.app.doc.new_cel(LayerKind::Bitmap);
    {
        let r = &mut b.app.doc.cels.get_mut(&cel).unwrap().raster;
        for y in 70..170usize {
            for x in 90..250usize {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgb(200, 30, 30);
            }
        }
    }
    b.app.doc.panels[0].layers[0].set(0, Some(cel));
    b.app.renderer.invalidate_all();
    b.app.tool = Tool::Transform;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

fn ecran(b: &Banc, p: Pos2) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), p)
}

/// Un glisser du pointeur de `a` à `c` (points de l'écran), en huit pas.
fn glisser_ecran(b: &mut Banc, a: Pos2, c: Pos2, touches: Modifiers) {
    let bouton = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: touches };
    b.touches = touches;
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(a + (c - a) * (k as f32 / 8.0))], 1.0 / 60.0);
    }
    b.image(vec![bouton(c, false)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
}

/// Où le point `p` du dessin du calque est maintenant sur la feuille.
fn sur_la_feuille(b: &Banc, p: Pos2) -> Pos2 {
    b.app.layers()[0].pose(0).affine(b.app.doc.stage_center()).apply(p)
}

#[test]
fn the_transform_tool_keeps_the_opposite_corner_or_the_centre_and_rotates_about_the_opposite_corner() {
    let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    // (réglage, Ctrl, le point qui doit rester)
    for (point, touches, garde_centre) in [(PointFixe::Opposee, Modifiers::NONE, false), (PointFixe::Centre, Modifiers::NONE, true), (PointFixe::Opposee, ctrl, true), (PointFixe::Centre, ctrl, false)] {
        let mut b = banc_transformer();
        b.app.settings.point_fixe = point;
        let boite = b.app.layer_content_box(0, 0).expect("le calque a un dessin");
        let (coin, oppose, centre) = (boite.right_bottom(), boite.left_top(), boite.center());
        let (a, c) = (ecran(&b, coin), ecran(&b, coin + vec2(40.0, 30.0)));
        glisser_ecran(&mut b, a, c, touches);
        let t = b.app.layers()[0].pose(0);
        assert!(t.stretch_x > 1.05 && t.stretch_y > 1.05, "le calque a grandi ({point:?}, Ctrl {}) : {t:?}", touches.ctrl);
        let fixe = if garde_centre { centre } else { oppose };
        assert!(sur_la_feuille(&b, fixe).distance(fixe) < 1.0, "{point:?}, Ctrl {} : {fixe:?} reste en place → {:?}", touches.ctrl, sur_la_feuille(&b, fixe));
    }
    // Le milieu d'un côté : le côté opposé reste (mode d'office).
    let mut b = banc_transformer();
    let boite = b.app.layer_content_box(0, 0).unwrap();
    let (a, c) = (ecran(&b, boite.right_center()), ecran(&b, boite.right_center() + vec2(50.0, 0.0)));
    glisser_ecran(&mut b, a, c, Modifiers::NONE);
    assert!(b.app.layers()[0].pose(0).stretch_x > 1.1);
    assert!(sur_la_feuille(&b, boite.left_center()).distance(boite.left_center()) < 1.0, "le côté gauche reste");
    // Juste hors d'un coin : tourner, autour du centre (d'office) ou du coin opposé.
    for (rotation, fixe) in [(CentreRotation::Pivot, boite.center()), (CentreRotation::CoinOppose, boite.left_top())] {
        let mut b = banc_transformer();
        b.app.settings.rotation_autour = rotation;
        let hors = ecran(&b, boite.right_bottom()) + vec2(14.0, 14.0);
        assert_eq!(b.app.transform_handle_at(hors), Some(BoxHandle::RotateCorner(2)), "hors du coin : tourner");
        glisser_ecran(&mut b, hors, hors + vec2(-60.0, 40.0), Modifiers::NONE);
        let t = b.app.layers()[0].pose(0);
        assert!(t.angle.abs() > 5.0, "le calque a tourné : {t:?}");
        assert!(sur_la_feuille(&b, fixe).distance(fixe) < 1.0, "{rotation:?} : {fixe:?} reste en place → {:?}", sur_la_feuille(&b, fixe));
    }
}

// ------------------------------------------------------------------------------------ le panneau Réglages

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn textes_du_panneau(app: &mut App) -> Vec<(String, Rect)> {
    let ctx = egui::Context::default();
    let mut textes = Vec::new();
    for i in 0..4 {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 900.0))), time: Some(i as f64 / 60.0), ..RawInput::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
        });
        textes.clear();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
    }
    textes
}

#[test]
fn the_two_choices_sit_above_more_settings_for_the_selection_and_transform_tools() {
    use egui_phosphor::regular as ph;
    for tool in [Tool::Select, Tool::Transform] {
        let mut app = appli(&format!("panneau-{}", tool.action()));
        app.set_tool(tool);
        let textes = textes_du_panneau(&mut app);
        let y = |t: &str| textes.iter().find(|(s, _)| s.trim() == t).map(|(_, r)| r.center().y);
        let plus = y("Plus de réglages").unwrap_or_else(|| panic!("{tool:?} : « Plus de réglages » : {textes:?}"));
        for icone in [ph::CORNERS_OUT, ph::ARROWS_OUT_CARDINAL, ph::CROSSHAIR_SIMPLE, ph::ANCHOR_SIMPLE] {
            let at = y(icone).unwrap_or_else(|| panic!("{tool:?} : {icone:?} est dans les réglages : {textes:?}"));
            assert!(at < plus, "{tool:?} : {icone:?} est au-dessus de « Plus de réglages »");
        }
    }
    // Pas pour le pinceau.
    let mut app = appli("panneau-pinceau");
    app.set_tool(Tool::Brush);
    let textes = textes_du_panneau(&mut app);
    assert!(!textes.iter().any(|(s, _)| s.trim() == ph::ANCHOR_SIMPLE));
}
