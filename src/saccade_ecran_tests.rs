//! La postérisation temporelle de la caméra (« par paliers ») **dans l'application** : on coche la case dans les Réglages de
//! l'outil Caméra, puis on avance image par image et on lit la caméra que l'écran montre (la vue caméra, le cadre) et celle
//! des exports. Avant la correction, l'écran lisait toujours la caméra douce (`current_camera`) : seuls les exports et les
//! vignettes montraient les paliers, jamais la lecture ni la vue caméra.

use std::sync::Arc;

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::app::{App, Tool};
use crate::camera::{Camera, CameraKey};
use crate::document::Document;
use crate::exportrender::{self, Papier, Rendu};
use crate::render;

fn app_en_mouvement(nom: &str, len: usize) -> App {
    let dir = std::env::temp_dir().join(format!("shot-saccade-ecran-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(150, 90, 12);
    doc.panels[0].set_length(len);
    let c = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: 0.0, ..c }), CameraKey::new(len - 1, Camera { x: 190.0, ..c })];
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app
}

/// Un écran sans fenêtre où l'on peint le panneau Caméra (réglages) et où l'on clique.
struct Ecran {
    ctx: egui::Context,
    t: f64,
}

impl Ecran {
    fn image(&mut self, app: &mut App, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(520.0, 1400.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.camera_panel(ui));
        });
        fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
                Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        let mut textes = Vec::new();
        out.shapes.iter().for_each(|s| collecter(&s.shape, &mut textes));
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize) -> Vec<(String, Rect)> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.image(app, Vec::new());
        }
        t
    }

    fn cliquer(&mut self, app: &mut App, p: egui::Pos2) {
        self.image(app, vec![Event::PointerMoved(p)]);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
        self.chauffer(app, 3);
    }
}

/// La case « Par paliers » du panneau de l'outil Caméra, cochée à la souris.
fn coche_dans_le_panneau(app: &mut App) {
    let mut ecran = Ecran { ctx: egui::Context::default(), t: 0.0 };
    let textes = ecran.chauffer(app, 4);
    assert!(textes.iter().any(|(t, _)| t.contains("Mouvement par paliers")), "la section est dans le panneau : {textes:?}");
    let (_, r) = textes.iter().find(|(t, _)| t == "Par paliers").expect("la case « Par paliers »");
    // La case est à gauche de son texte.
    ecran.cliquer(app, pos2(r.left() - 10.0, r.center().y));
}

fn ecran() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 480.0))
}

/// Ce que montre la vue caméra à l'image `f`.
fn vue_a(app: &mut App, f: usize) -> (f32, egui::Vec2) {
    app.cur_frame = f;
    app.apply_camera_view(ecran());
    (app.view.zoom, app.view.pan)
}

#[test]
fn ticking_the_box_in_the_camera_settings_makes_the_screen_view_and_the_playback_step() {
    let mut app = app_en_mouvement("vue", 20);
    app.tool = Tool::Camera;
    assert!(!app.doc.panels[0].saccade.actif);
    coche_dans_le_panneau(&mut app);
    assert!(app.doc.panels[0].saccade.actif, "la case atteint le plan courant");
    assert_eq!(app.doc.panels[0].saccade.cadence, 2);
    // Cadence 3 (le réglage du nombre est testé par ailleurs).
    app.doc.panels[0].saccade.cadence = 3;

    // La vue caméra, lecture arrêtée, outil Caméra : la caméra douce, celle que les poignées tirent.
    app.camera_view = true;
    let douce: Vec<_> = (0..20).map(|f| vue_a(&mut app, f)).collect();
    assert!(douce.windows(2).all(|w| w[0].1 != w[1].1), "l'outil Caméra arrêté montre le mouvement doux");

    // En lecture : les paliers, comme les exports.
    app.playing = true;
    let lecture: Vec<_> = (0..20).map(|f| vue_a(&mut app, f)).collect();
    for debut in (0..18).step_by(3) {
        assert!(lecture[debut] == lecture[debut + 1] && lecture[debut] == lecture[debut + 2], "palier {debut} constant à l'écran : {lecture:?}");
        assert!(lecture[debut].1 != lecture[debut + 3].1, "le palier suivant bouge");
    }
    // Un autre outil que la Caméra, arrêté : les paliers aussi (on s'arrête sur une image et on la voit comme à l'export).
    app.playing = false;
    app.tool = Tool::Pan;
    let arret: Vec<_> = (0..20).map(|f| vue_a(&mut app, f)).collect();
    assert!(arret == lecture, "à l'arrêt hors de l'outil Caméra, l'écran montre la caméra des paliers");
    // La caméra de l'écran est celle du document, celle de l'export.
    for f in 0..20 {
        app.cur_frame = f;
        assert_eq!(app.camera_montree(), app.doc.panel_camera(0, f));
    }
    // Décochée : le mouvement doux, à l'identique.
    app.doc.panels[0].saccade.actif = false;
    let doux: Vec<_> = (0..20).map(|f| vue_a(&mut app, f)).collect();
    assert!(doux == douce);
}

#[test]
fn the_export_of_the_still_and_the_range_show_what_the_screen_shows() {
    let mut app = app_en_mouvement("export", 12);
    app.doc = {
        let mut d = crate::export_tests::doc_with_drawing(1, 12);
        let c = d.default_camera();
        d.panels[0].camera = vec![CameraKey::new(0, Camera { x: c.x - 20.0, ..c }), CameraKey::new(11, Camera { x: c.x + 20.0, ..c })];
        d
    };
    app.tool = Tool::Camera;
    coche_dans_le_panneau(&mut app);
    assert!(app.doc.panels[0].saccade.actif);
    app.doc.panels[0].saccade.cadence = 4;
    app.tool = Tool::Pan;
    let rendu = Rendu::new(exportrender::snapshot_of(&app.doc, &[0]), Papier::Fond, false, Vec::new(), Arc::default());
    for f in 0..12 {
        app.cur_frame = f;
        let plan = render::panel_plan_at(&app.doc, 0, f, app.doc.render_paper());
        let a_l_ecran = render::camera_image(&app.doc, &plan, &app.camera_montree());
        let image = crate::fxcache::avec_privee(256 << 20, || rendu.image(0, f)).expect("image");
        assert!(*image == a_l_ecran, "image {f} : l'export montre la caméra de l'écran");
    }
}
