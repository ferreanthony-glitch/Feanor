//! Essais de la passe « optim-rendu » (2026-10-23, `docs/PERFORMANCE.md`, section 24) : ce que l'audit mesuré a trouvé, et
//! que le comportement reste le même.
//!
//! 1. Tirer un réglage (opacité d'un calque, clé de position, clé d'effet, exposition, fin du plan) ne renvoie aucune tuile à
//!    la carte graphique (`Renderer::invalidate_params`).
//! 2. Les numéros des plans et des séquences se trouvent une fois par image d'écran (`numerotation::Numeros`), la règle et la
//!    barre des séquences du montage ne peignent que leur partie visible : mêmes textes, à la même place.

use std::time::Instant;

use eframe::egui::{pos2, Event, Modifiers, PointerButton, Pos2, Rect, Shape};

use crate::app::Mode;
use crate::document::{Document, LayerTransform};
use crate::numerotation::Numeros;
use crate::perf_tests::{banc, plan_charge, projet_long, Banc};
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

// ------------------------------------------------------------------ 2. numéros des plans et des séquences

/// Un film de `plans` plans en séquences de dix (sans dessin), avec ce qui change les titres : des plans et des séquences
/// nommés (un nom fait d'espaces ne compte pas), des versions de plans ; en numérotation verrouillée, des plans insérés
/// (12A…), un plan déplacé (numéros dans le désordre) et un numéro illisible (le rang le remplace).
fn film_numerote(plans: usize, verrouille: bool) -> Document {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut at = 0;
    for i in 1..plans {
        at = if i % 10 == 0 { doc.add_sequence_after(at) } else { doc.add_scene_after(at) };
    }
    for (k, s) in doc.scenes.iter_mut().enumerate() {
        match k % 41 {
            5 => s.name = format!("  Contrechamp {k} "),
            9 => s.name = "   ".into(),
            _ => {}
        }
    }
    for (k, s) in doc.sequences.iter_mut().enumerate() {
        if k % 7 == 3 {
            s.name = format!("Poursuite {k}");
        }
    }
    for i in [2, 30, plans / 2] {
        doc.nouvelle_version(i, 0);
    }
    if verrouille {
        doc.lock_numbering();
        for at in [11, 11, 40, 0, plans / 3] {
            doc.add_scene_after(at);
        }
        doc.add_sequence_after(plans / 4);
        doc.move_panel(5, plans / 2);
        let id = doc.panels[17].scene;
        if let Some(s) = doc.scene_mut(id) {
            s.number = "pas un numéro".into();
        }
    }
    doc
}

#[test]
fn numbering_in_one_pass_matches_the_document() {
    for verrouille in [false, true] {
        let doc = film_numerote(600, verrouille);
        let n = Numeros::of(&doc);
        for i in 0..doc.panels.len() {
            assert_eq!(crate::planversui::titre_avec_version_n(&doc, &n, i), crate::planversui::titre_avec_version(&doc, i), "titre du plan {i} (verrouillée : {verrouille})");
        }
        let scenes: Vec<u64> = doc.scenes.iter().map(|s| s.id).chain([0, u64::MAX]).collect();
        for id in scenes {
            assert_eq!(n.scene_label(&doc, id), doc.scene_label(id), "plan {id}");
        }
        let seqs: Vec<u64> = doc.sequences.iter().map(|s| s.id).chain([0, u64::MAX]).collect();
        for id in seqs {
            assert_eq!(n.sequence_code(&doc, id), doc.sequence_code(id), "numéro de la séquence {id}");
            assert_eq!(n.sequence_label(&doc, id), doc.sequence_label(id), "nom de la séquence {id}");
            assert_eq!(n.sequence_number(&doc, id), doc.sequence_number(id), "rang de la séquence {id}");
        }
        // (Le film a bien de quoi : des numéros à lettre, des noms, des versions.)
        let titres: Vec<String> = (0..doc.panels.len()).map(|i| crate::planversui::titre_avec_version(&doc, i)).collect();
        assert!(titres.iter().any(|t| t.contains("Contrechamp")) && titres.iter().any(|t| t.contains(" · ")));
        if verrouille {
            assert!(titres.iter().any(|t| t.contains("12A")), "des plans insérés en numérotation verrouillée");
        }
    }
}

/// Les textes que l'image d'écran montre vraiment (dans leur zone de coupe), avec leur place.
fn textes_visibles(b: &Banc) -> Vec<(String, [i32; 2])> {
    fn visit(shape: &Shape, clip: Rect, out: &mut Vec<(String, [i32; 2])>) {
        match shape {
            Shape::Text(t) if t.visual_bounding_rect().intersects(clip) => out.push((t.galley.job.text.clone(), [(t.pos.x * 8.0).round() as i32, (t.pos.y * 8.0).round() as i32])),
            Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        visit(&s.shape, s.clip_rect, &mut out);
    }
    out.sort();
    out
}

#[test]
fn the_montage_shows_the_same_titles_ruler_and_sequences_as_before() {
    let mut doc = projet_long(240, 1, 12, 1);
    for (k, s) in doc.sequences.iter_mut().enumerate() {
        if k % 5 == 2 {
            s.name = format!("Poursuite {k}");
        }
    }
    doc.nouvelle_version(3, 0);
    let mut b = banc(doc, false);
    b.garder_formes = true;
    let total = b.app.doc.total_frames();
    for (zoom, image) in [(0.5, 0), (2.0, total / 3), (6.0, total / 2), (16.0, total - 5)] {
        b.app.montage_zoom = zoom;
        b.app.goto_global(image);
        b.app.follow_frame = true;
        // (Le défilement qui suit la tête de lecture s'arrête d'abord : deux images de suite identiques.)
        b.chauffer(40);
        let repos = textes_visibles(&b);
        b.chauffer(2);
        assert_eq!(textes_visibles(&b), repos, "zoom {zoom} : l'écran ne bouge plus");
        crate::perf::set_ancien(true);
        b.chauffer(2);
        let avant = textes_visibles(&b);
        crate::perf::set_ancien(false);
        b.chauffer(2);
        let apres = textes_visibles(&b);
        let manque: Vec<_> = avant.iter().filter(|t| !apres.contains(t)).collect();
        let de_plus: Vec<_> = apres.iter().filter(|t| !avant.contains(t)).collect();
        assert!(manque.is_empty() && de_plus.is_empty(), "zoom {zoom}, image {image} : les mêmes textes visibles ; manquent {manque:?}, en plus {de_plus:?}");
        assert_eq!(apres, avant);
        // (Au zoom 0,5, un bloc de 12 images fait 6 points : son titre, coupé au bloc, ne se voit pas.)
        assert!(apres.iter().any(|t| t.0.starts_with("Séquence ")) && (zoom < 1.0 || apres.iter().any(|t| t.0.starts_with("Plan "))), "zoom {zoom} : titres et séquences à l'écran");
        assert!(apres.iter().any(|t| t.0.contains(':') && t.0.len() <= 6), "zoom {zoom} : des repères de la règle");
    }
}

#[test]
fn the_ruler_paints_every_second_that_can_be_seen() {
    // Toutes les secondes dont la graduation ou le texte (moins de 60 points à droite) peut tomber dans la zone sont peintes.
    for (left, fps, ppf) in [(6.0, 24, 0.5), (-5000.0, 24, 4.0), (-120000.0, 25, 16.0), (100.0, 30, 1.0)] {
        let last = 600;
        for (x0, x1) in [(0.0, 1900.0), (300.0, 900.0), (-50.0, 10.0), (5000.0, 9000.0)] {
            let clip = Rect::from_min_max(pos2(x0, 0.0), pos2(x1, 100.0));
            let (lo, hi) = crate::storyboard::visible_seconds(clip, left, fps, ppf as f32, last);
            for s in 0..=last {
                let x = left as f32 + (s * fps) as f32 * ppf as f32;
                if x + 60.0 >= clip.left() && x <= clip.right() {
                    assert!((lo..=hi).contains(&s), "seconde {s} (x = {x}) dans {clip:?} : peinte ({lo}..={hi})");
                }
            }
        }
    }
}

#[test]
fn the_numbering_of_a_long_film_costs_little_per_screen_image() {
    // 2 000 plans : un titre par bloc visible (150) et une étiquette par séquence (200), comme une image d'écran du montage.
    // Avant : l'ordre du film refait à chaque titre, ≈ 2 000 × 350 pas. La borne laisse une grande marge (machine partagée).
    let doc = film_numerote(2000, true);
    let t = Instant::now();
    let tours = 10;
    let mut n_titres = 0;
    for _ in 0..tours {
        let n = Numeros::of(&doc);
        for i in (0..doc.panels.len()).step_by(13).take(150) {
            n_titres += crate::planversui::titre_avec_version_n(&doc, &n, i).len();
        }
        for s in &doc.sequences {
            n_titres += n.sequence_label(&doc, s.id).len() + n.sequence_number(&doc, s.id);
        }
    }
    let ms = t.elapsed().as_secs_f64() * 1e3 / tours as f64;
    assert!(n_titres > 0);
    assert!(ms < 4.0, "numéros d'une image d'écran, 2 000 plans : {ms:.2} ms");
}
