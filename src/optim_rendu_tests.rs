//! Essais de la passe « optim-rendu » (2026-10-23, `docs/PERFORMANCE.md`, section 24) : ce que l'audit mesuré a trouvé, et
//! que le comportement reste le même.
//!
//! 1. Tirer un réglage (opacité d'un calque, clé de position, clé d'effet, exposition, fin du plan) ne renvoie aucune tuile à
//!    la carte graphique (`Renderer::invalidate_params`).

use eframe::egui::{pos2, Event, Modifiers, PointerButton, Pos2, Shape};

use crate::app::Mode;
use crate::document::LayerTransform;
use crate::perf_tests::{banc, plan_charge, Banc};
use crate::poskeys::PosKey;

/// Tuiles envoyées à la carte graphique et compositions faites depuis le début.
fn carte(b: &Banc) -> (u64, u64) {
    let s = b.app.gpu.as_ref().expect("carte graphique").stats();
    (s.copies, s.composes)
}

fn textes(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

/// Un plan de 15 calques (40 tuiles par dessin), dans l'animation, prêt : rien sans carte graphique.
fn plan_lourd() -> Option<Banc> {
    let mut b = banc(plan_charge(15, 40, 40), true);
    b.app.gpu.as_ref()?;
    b.app.set_mode(Mode::Animation);
    b.app.cur_frame = 5;
    b.chauffer(10);
    b.attendre_vignettes();
    Some(b)
}

/// Une image d'écran pendant un geste fait par le code (les gestes du montage et de la timeline appellent ces fonctions ; ils
/// finissent le geste dès qu'aucun bouton n'est tenu : le bouton du milieu reste tenu dans la barre d'état, où il ne fait rien).
fn image(b: &mut Banc) {
    b.image(Vec::new(), 1.0 / 60.0);
}

const COIN: Pos2 = pos2(960.0, 1076.0);

fn tenir(b: &mut Banc, pressed: bool) {
    b.image(vec![Event::PointerMoved(COIN), Event::PointerButton { pos: COIN, button: PointerButton::Middle, pressed, modifiers: Modifiers::NONE }], 1.0 / 60.0);
}

#[test]
fn dragging_a_layer_opacity_bar_sends_no_tile_to_the_gpu() {
    let Some(mut b) = plan_lourd() else { return };
    b.garder_formes = true;
    b.chauffer(2);
    let mut vus = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut vus);
    }
    // La barre d'opacité d'un calque, dans les en-têtes des calques : « 100 % ».
    let Some(&(_, centre)) = vus.iter().find(|(t, _)| t == "100 %") else { panic!("aucune barre d'opacité à l'écran : {:?}", vus.iter().map(|v| &v.0).take(60).collect::<Vec<_>>()) };
    b.garder_formes = false;
    let (copies, composes) = carte(&b);
    let avant: Vec<f32> = b.app.layers().iter().map(|l| l.opacity).collect();
    let presse = Event::PointerButton { pos: centre, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(centre), presse], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![Event::PointerMoved(pos2(centre.x - 4.0 * k as f32, centre.y))], 1.0 / 60.0);
    }
    let pendant = carte(&b);
    let apres: Vec<f32> = b.app.layers().iter().map(|l| l.opacity).collect();
    assert_ne!(apres, avant, "le geste a changé une opacité");
    assert_eq!(pendant.0, copies, "pendant le geste, aucune tuile n'est renvoyée à la carte graphique");
    assert!(pendant.1 >= composes + 6, "l'image se recompose à chaque opacité ({} compositions)", pendant.1 - composes);
    let fin = pos2(centre.x - 48.0, centre.y);
    b.image(vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
}

#[test]
fn dragging_position_keys_and_drawing_keys_sends_no_tile_to_the_gpu() {
    let Some(mut b) = plan_lourd() else { return };
    let li = 3;
    b.app.select_layer(li);
    b.app.doc.panels[0].layers[li].pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(10, LayerTransform { x: 300.0, ..LayerTransform::default() })];
    b.app.renderer.invalidate_all();
    // La clé de l'image 10 choisie (la tête de lecture y va), puis la tête de lecture à l'image 8 ; les dessins des images qui
    // suivent partent d'avance à la carte graphique : on attend qu'ils y soient tous.
    b.app.click_pos_marker(li, 10, false);
    b.app.set_frame_free(8);
    crate::perf_tests::jusqu_au_repos(&mut b, 600);

    // Les clés de position (montage, timeline) : la clé de l'image 10 glisse jusqu'à l'image 6.
    tenir(&mut b, true);
    let (copies, composes) = carte(&b);
    assert!(b.app.begin_pos_drag(li, 10, 10), "le geste commence");
    for at in [9, 8, 7, 6] {
        b.app.drag_pos_keys(at);
        image(&mut b);
    }
    let pendant = carte(&b);
    assert!(b.app.pos_ui.drag.is_some(), "le geste dure");
    assert_eq!(b.app.doc.panels[0].layers[li].pos_keys[1].frame, 6, "la clé a glissé");
    assert_eq!(pendant.0, copies, "clés de position : aucune tuile renvoyée pendant le geste");
    // (À l'image 8, la clé en 9 puis en 8 change la position ; en 7 et en 6, elle reste celle de la clé.)
    assert!(pendant.1 >= composes + 2, "la position change pendant le geste");
    assert_eq!(b.app.cur_frame, 8);
    tenir(&mut b, false);
    b.chauffer(3);
    assert!(b.app.pos_ui.drag.is_none(), "le geste finit au lâcher");

    // Une limite entre deux expositions (le dessin de l'image 4 commence plus tard : l'image courante, la 5, montre celui de
    // l'image 2), puis la fin du plan (l'image 5 ne change pas : rien à recomposer).
    use crate::handles::Handle;
    for (handle, gestes, recompose) in [(Handle::Boundary { layer: 0, at: 4 }, [(5, 5), (6, 6), (7, 7)], true), (Handle::PlanEnd, [(30, 30), (25, 25), (20, 20)], false)] {
        tenir(&mut b, true);
        let (copies, composes) = carte(&b);
        let avant = b.app.doc.panels[0].clone();
        assert!(b.app.begin_handle_drag(handle), "le geste {handle:?} commence");
        for (frame, edge) in gestes {
            b.app.drag_handle(frame, edge);
            image(&mut b);
        }
        let pendant = carte(&b);
        assert!(b.app.handle_drag.is_some(), "le geste {handle:?} dure");
        let p = &b.app.doc.panels[0];
        assert!(p.frame_count() != avant.frame_count() || p.layers[0].frames != avant.layers[0].frames, "le geste {handle:?} a changé le plan");
        assert_eq!(pendant.0, copies, "{handle:?} : aucune tuile renvoyée pendant le geste");
        if recompose {
            assert!(pendant.1 > composes, "{handle:?} : l'image se recompose");
        }
        tenir(&mut b, false);
        b.chauffer(3);
        assert!(b.app.handle_drag.is_none(), "le geste {handle:?} finit au lâcher");
    }
    // (Le compteur voit bien un renvoi : `invalidate_all`, pour des pixels, renvoie tout.)
    let (copies, _) = carte(&b);
    b.app.renderer.invalidate_all();
    image(&mut b);
    assert!(carte(&b).0 > copies + 100, "invalidate_all renvoie les tuiles du plan");
}
