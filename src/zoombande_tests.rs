//! Tests du zoom de la bande du bas, calé sur la tête de lecture (`zoombande.rs`) : la règle (où la
//! tête doit rester), le calcul du défilement, et le geste complet dans une zone de défilement.

use eframe::egui::{self, pos2, vec2, RawInput, Rect};

use crate::app::App;
use crate::zoombande::{head_x, scroll_delta, view_x_to_keep, Strip, StripGeo};

#[test]
fn the_playhead_keeps_its_place_in_the_view_or_goes_to_the_middle() {
    assert_eq!(view_x_to_keep(120.0, 400.0), 120.0, "dans la vue : elle y reste");
    assert_eq!(view_x_to_keep(0.0, 400.0), 0.0);
    assert_eq!(view_x_to_keep(400.0, 400.0), 400.0);
    assert_eq!(view_x_to_keep(-30.0, 400.0), 200.0, "à gauche de la vue : on la ramène au milieu");
    assert_eq!(view_x_to_keep(950.0, 400.0), 200.0, "à droite de la vue : au milieu aussi");
}

#[test]
fn the_scroll_after_a_zoom_puts_the_playhead_back_where_it_was() {
    // La bande commence à 6 ; la vue montre à partir de l'abscisse 300 ; la tête est à l'image 100.
    let (origin, view_left, view_w) = (6.0_f32, 300.0_f32, 400.0_f32);
    for (old, new) in [(4.0_f32, 8.0_f32), (8.0, 2.0), (4.0, 4.5), (1.0, 16.0)] {
        let before = head_x(origin, old, 100.0);
        // Tête dans la vue ? Sinon elle ira au milieu.
        let want = view_x_to_keep(before - view_left, view_w);
        let after = head_x(origin, new, 100.0);
        let delta = scroll_delta(after, view_left, want);
        // Le contenu se déplace de `delta` : la tête est à `after + delta`.
        let landed = after + delta - view_left;
        assert!((landed - want).abs() < 1e-3, "{old} -> {new} : la tête tombe à {landed}, voulu {want}");
        if (0.0..=view_w).contains(&(before - view_left)) {
            assert!((landed - (before - view_left)).abs() < 1e-3, "{old} -> {new} : elle reste où elle était");
        }
    }
}

#[test]
fn zooming_with_the_playhead_out_of_view_centres_it_first() {
    let (origin, view_left, view_w) = (6.0_f32, 0.0_f32, 400.0_f32);
    let before = head_x(origin, 4.0, 500.0) - view_left; // 2006 : très loin à droite
    assert!(before > view_w);
    let want = view_x_to_keep(before, view_w);
    let after = head_x(origin, 8.0, 500.0);
    let landed = after + scroll_delta(after, view_left, want) - view_left;
    assert_eq!(landed, 200.0, "au milieu de la vue");
}

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-zoom-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let app = App::sans_fenetre(&ctx, None);
    (ctx, app)
}

/// Une image d'une zone de défilement horizontale de 10 000 points, qui montre la bande avec
/// l'échelle `scale` ; renvoie le décalage de la zone. `start` : décalage imposé à la première image.
fn frame(ctx: &egui::Context, app: &mut App, time: f64, scale: f32, head_pos: f32, start: Option<f32>) -> f32 {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))), time: Some(time), ..Default::default() };
    let mut offset = 0.0;
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut area = egui::ScrollArea::horizontal().auto_shrink([false, false]);
            if let Some(x) = start {
                area = area.horizontal_scroll_offset(x);
            }
            let out = area.show(ui, |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10_000.0, 20.0), egui::Sense::hover());
                app.follow_strip_anchor(ui, Strip::Montage, r.left() + 6.0, scale, head_pos);
            });
            offset = out.state.offset.x;
        });
    });
    offset
}

#[test]
fn a_zoom_scrolls_the_strip_so_the_playhead_stays_put() {
    let (ctx, mut app) = headless("scroll");
    // Image 1 : échelle 4, la vue commence à 300 : la tête (image 100) est à 400 + 6 - 300 = 106.
    let mut t = 0.0;
    let o1 = frame(&ctx, &mut app, t, 4.0, 100.0, Some(300.0));
    assert!((o1 - 300.0).abs() < 1.0, "décalage de départ {o1}");
    // (À quelques points près : la zone de défilement a ses marges.)
    assert!((app.strip_geo[0].head_view_x - 106.0).abs() < 8.0, "la tête est à {} dans la vue", app.strip_geo[0].head_view_x);

    // Le zoom double l'échelle : la tête doit rester à 106 de la gauche de la vue.
    app.anchor_strip_zoom(Strip::Montage);
    t += 0.1;
    let mut o = frame(&ctx, &mut app, t, 8.0, 100.0, None);
    for _ in 0..3 {
        t += 0.1;
        o = frame(&ctx, &mut app, t, 8.0, 100.0, None);
    }
    // 100 * 8 + 6 = 806 : la vue doit commencer à 806 - 106 = 700.
    assert!((o - 700.0).abs() < 2.0, "la zone a défilé à {o} au lieu de 700");
    assert!(app.strip_anchor[0].is_none(), "la demande est consommée");
}

#[test]
fn a_zoom_with_the_playhead_off_screen_brings_it_to_the_middle() {
    let (ctx, mut app) = headless("hors-vue");
    let mut t = 0.0;
    // Vue à 0 : la tête (image 100 à l'échelle 8 : 806) est hors de la vue de 400.
    frame(&ctx, &mut app, t, 8.0, 100.0, Some(0.0));
    assert!(app.strip_geo[0].head_view_x > app.strip_geo[0].view_w);
    app.anchor_strip_zoom(Strip::Montage);
    let mut o = 0.0;
    for _ in 0..4 {
        t += 0.1;
        o = frame(&ctx, &mut app, t, 16.0, 100.0, None);
    }
    // À l'échelle 16 la tête est à 1606 ; au milieu d'une vue de 400 : la vue commence à 1606 - 200.
    let width = app.strip_geo[0].view_w;
    assert!((o - (1606.0 - width / 2.0)).abs() < 8.0, "défilement {o}, vue de {width}");
}

#[test]
fn an_old_request_from_a_hidden_strip_is_forgotten() {
    let (ctx, mut app) = headless("vieille");
    let mut t = 0.0;
    frame(&ctx, &mut app, t, 4.0, 100.0, Some(300.0));
    app.anchor_strip_zoom(Strip::Montage);
    // Dix images d'écran passent sans que la bande soit affichée.
    for _ in 0..10 {
        t += 0.1;
        let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))), time: Some(t), ..Default::default() }, |_| {});
    }
    t += 0.1;
    let o = frame(&ctx, &mut app, t, 8.0, 100.0, None);
    assert!((o - 300.0).abs() < 1.0, "pas de saut : {o}");
}

#[test]
fn the_zoom_gestures_all_ask_for_the_anchor() {
    // Le curseur, la molette et le raccourci de chaque bande demandent le calage.
    let storyboard = include_str!("storyboard.rs");
    let timeline = include_str!("timeline.rs");
    let cues = include_str!("cues.rs");
    let app = include_str!("app.rs");
    assert!(storyboard.matches("anchor_strip_zoom(crate::zoombande::Strip::Montage)").count() >= 2, "curseur et molette du montage");
    assert!(timeline.contains("anchor_strip_zoom(crate::zoombande::Strip::Timeline)"), "molette de la timeline");
    assert!(cues.contains("anchor_strip_zoom(crate::zoombande::Strip::Timeline)"), "curseur de la timeline");
    assert!(app.contains("self.anchor_strip_zoom(Strip::Montage)") && app.contains("self.anchor_strip_zoom(Strip::Timeline)"), "raccourcis et boutons");
    let _ = StripGeo::default();
}
