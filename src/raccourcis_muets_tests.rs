//! Les raccourcis qui « manquent parfois » (Anthony, 2026-11-16, « notamment le zoom ») : un glisser de la bande de montage dont le
//! lâcher n'a pas été vu ne reste plus en cours (il mettait toutes les touches de côté) ; un bouton qui a pris le clavier le rend ;
//! et quand une touche de raccourci ne fait rien, la barre d'état dit pourquoi.

use eframe::egui::{self, pos2, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, ViewportId};

use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::storyboard::MontageDrag;

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1600, 900);
    b.chauffer(8);
    b
}

/// Un point de la feuille, loin du centre (où se posent les fenêtres).
fn sur_la_feuille(b: &Banc) -> Pos2 {
    let r = b.app.canvas_rect;
    pos2(r.left() + 30.0, r.top() + 30.0)
}

fn taper(b: &mut Banc, key: Key) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
}

fn zoom(b: &Banc) -> f32 {
    b.app.view.zoom
}

#[test]
fn a_montage_drag_whose_release_was_missed_no_longer_mutes_every_shortcut() {
    let mut b = banc();
    let p = sur_la_feuille(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    // Le lâcher n'a pas été vu par la bande (cachée, ou refaite pendant le geste) : le glisser est resté en cours.
    for drag in [MontageDrag::Scrub, MontageDrag::Move { panel: b.app.doc.panels[0].id, target: 0, merge: None }] {
        b.app.montage_drag = Some(drag);
        b.chauffer(1);
        assert!(b.app.montage_drag.is_none(), "aucun bouton appuyé : le geste est fini comme au lâcher");
        assert!(!b.app.dragging());
        let avant = zoom(&b);
        taper(&mut b, Key::Num2);
        assert!(zoom(&b) > avant * 1.1, "la touche 2 zoome de nouveau ({avant} → {})", zoom(&b));
    }
    // Un extrait de son tiré depuis la timeline, pareil.
    b.app.timeline_sound_track = Some(0);
    b.chauffer(1);
    assert!(b.app.timeline_sound_track.is_none());
    assert_eq!(b.app.doc.panels.len(), 1, "le plan « déplacé » à sa propre place est toujours là, seul");
}

#[test]
fn a_shortcut_typed_during_a_drag_is_not_done_and_the_status_bar_says_why() {
    let mut b = banc();
    let p = sur_la_feuille(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    // Le bouton est tenu (par la molette, qui ne dessine pas) : le glisser du montage est vraiment en cours.
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Middle, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.app.montage_drag = Some(MontageDrag::Scrub);
    let avant = zoom(&b);
    taper(&mut b, Key::Num2);
    assert_eq!(zoom(&b), avant, "pendant le geste, la touche attend");
    assert!(b.app.status.contains("Zoom avant") && b.app.status.contains("glisser est en cours"), "barre d'état : {}", b.app.status);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Middle, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert!(!b.app.dragging());
    taper(&mut b, Key::Num2);
    assert!(zoom(&b) > avant * 1.1, "le geste lâché, la touche zoome");
}

#[test]
fn a_shortcut_typed_while_a_text_field_has_the_keyboard_says_so_when_the_mouse_is_elsewhere() {
    let mut b = banc();
    let p = sur_la_feuille(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    // « Enregistrer la disposition » : une fenêtre dont la case du nom a le clavier.
    b.app.workspace_name = Some(String::new());
    b.chauffer(3);
    assert!(b.ctx.wants_keyboard_input(), "la case du nom a le clavier");
    let avant = zoom(&b);
    taper(&mut b, Key::Num2);
    assert_eq!(zoom(&b), avant);
    assert!(b.app.status.contains("Zoom avant") && b.app.status.contains("case où l'on écrit"), "barre d'état : {}", b.app.status);
    // Échap ferme la fenêtre : rien n'est dit de plus, et le raccourci revient.
    b.app.status.clear();
    taper(&mut b, Key::Escape);
    b.chauffer(2);
    assert!(b.app.workspace_name.is_none());
    assert!(b.app.status.is_empty() || !b.app.status.contains("n'a pas été fait"), "barre d'état : {}", b.app.status);
    taper(&mut b, Key::Num2);
    assert!(zoom(&b) > avant * 1.1);
}

#[test]
fn a_shortcut_typed_while_a_window_waits_for_an_answer_says_so() {
    let mut b = banc();
    let p = sur_la_feuille(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.app.about_window = true;
    b.chauffer(3);
    let avant = zoom(&b);
    taper(&mut b, Key::Num2);
    assert_eq!(zoom(&b), avant);
    assert!(b.app.status.contains("Zoom avant") && b.app.status.contains("fenêtre attend"), "barre d'état : {}", b.app.status);
}

#[test]
fn a_button_that_took_the_keyboard_gives_it_back_but_a_text_field_keeps_it() {
    let mut b = banc();
    let run = |b: &mut Banc, texte: bool| -> bool {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), b.taille)), ..RawInput::default() };
        input.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
        let mut garde = false;
        for _ in 0..3 {
            let ctx = b.ctx.clone();
            let app = &mut b.app;
            let mut nom = String::from("nom");
            let _ = ctx.run(input.clone(), |ctx| {
                egui::Area::new(egui::Id::new("essai_focus")).fixed_pos(pos2(5.0, 5.0)).show(ctx, |ui| {
                    let r = if texte { ui.text_edit_singleline(&mut nom) } else { ui.button("Bouton") };
                    if !r.has_focus() && !garde {
                        r.request_focus();
                    }
                });
                garde = true;
                app.handle_shortcuts(ctx);
            });
        }
        b.ctx.wants_keyboard_input()
    };
    assert!(!run(&mut b, false), "le bouton a rendu le clavier");
    assert!(run(&mut b, true), "la case où l'on écrit le garde");
}

/// L'Espace 3D ouvert (une seule vue), la souris en `p`.
fn espace3d_ouvert() -> Banc {
    let mut b = banc();
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.espace3d.on && !b.app.camera_view);
    b
}

#[test]
fn in_the_3d_space_the_zoom_keys_zoom_the_3d_view_not_the_hidden_sheet() {
    let mut b = espace3d_ouvert();
    let centre = b.app.canvas_rect.center();
    b.image(vec![Event::PointerMoved(centre)], 1.0 / 60.0);
    let (feuille, d0) = (zoom(&b), b.app.espace3d.cam.expect("vue d'édition").distance);
    taper(&mut b, Key::Num2);
    let d1 = b.app.espace3d.cam.unwrap().distance;
    assert!((d1 - d0 / 1.25).abs() < d0 * 0.01, "2 rapproche la vue du même pas que la feuille : {d0} → {d1}");
    assert_eq!(zoom(&b), feuille, "la feuille cachée ne bouge pas");
    // La souris ailleurs (en haut à gauche, sur la barre de menus) : la vue 3D zoome quand même.
    b.image(vec![Event::PointerMoved(pos2(4.0, 4.0))], 1.0 / 60.0);
    taper(&mut b, Key::Num1);
    let d2 = b.app.espace3d.cam.unwrap().distance;
    assert!((d2 - d0).abs() < d0 * 0.01, "1 la ramène : {d1} → {d2}");
}

#[test]
fn with_several_3d_views_the_zoom_keys_zoom_the_view_under_the_mouse_or_else_the_last_active_one() {
    use crate::espace3dvues::Disposition;
    let mut b = espace3d_ouvert();
    b.app.e3_vues_poser(Disposition::Quatre);
    b.chauffer(3);
    let rects = b.app.espace3d.vues.rects.clone();
    assert!(rects.len() >= 2, "plusieurs vues : {rects:?}");
    let mesure = |b: &Banc, i: usize| {
        let c = b.app.espace3d.vues.cadres[i];
        if c.point.est_ortho() { c.ortho.span } else { c.orbite.distance }
    };
    let (i, r) = rects[0];
    let autres: Vec<(usize, f32)> = rects[1..].iter().map(|&(j, _)| (j, mesure(&b, j))).collect();
    b.image(vec![Event::PointerMoved(r.center())], 1.0 / 60.0);
    let avant = mesure(&b, i);
    taper(&mut b, Key::Num2);
    assert!(mesure(&b, i) < avant * 0.9, "la vue sous la souris zoome : {avant} → {}", mesure(&b, i));
    for (j, m) in &autres {
        assert_eq!(mesure(&b, *j), *m, "la vue {j} ne bouge pas");
    }
    // La souris hors des vues : la dernière vue active zoome.
    let active = b.app.espace3d.vues.active;
    let avant = mesure(&b, active);
    b.image(vec![Event::PointerMoved(pos2(4.0, 4.0))], 1.0 / 60.0);
    taper(&mut b, Key::Num1);
    assert!(mesure(&b, active) > avant * 1.1, "la vue active ({active}) dézoome : {avant} → {}", mesure(&b, active));
}
