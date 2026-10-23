//! Essais de la passe « optim-rendu » (2026-10-23, `docs/PERFORMANCE.md`, section 24) : ce que l'audit mesuré a trouvé, et
//! que le comportement reste le même.
//!
//! 1. Tirer un réglage (opacité d'un calque, clé de position, clé d'effet, exposition, fin du plan) ne renvoie aucune tuile à
//!    la carte graphique (`Renderer::invalidate_params`).
//! 2. Les numéros des plans et des séquences se trouvent une fois par image d'écran (`numerotation::Numeros`), la règle et la
//!    barre des séquences du montage ne peignent que leur partie visible : mêmes textes, à la même place.
//! 4. Les vignettes de la planche gardées sur la carte graphique sont bornées en octets, sans jamais lâcher celles de l'écran.
//! 6. La disposition de la planche et les lignes des notes sont gardées tant que rien de ce dont elles dépendent ne change :
//!    la disposition gardée est celle qu'on referait (planches au hasard).

use std::collections::{HashMap, HashSet};
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

// ------------------------------------------------------------------ 4. vignettes de la planche bornées

#[test]
fn board_thumbnails_stay_under_their_memory_bound_while_zooming() {
    let mut b = banc(projet_long(100, 1, 4, 1), false);
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.board_zoom = 0.0;
    let limite: u64 = 12 << 20;
    b.app.thumbs.limite = Some(limite);
    b.chauffer(5);
    b.attendre_vignettes();
    let mut pire = 0u64;
    // Les cases visibles qui avaient une vignette à l'étape d'avant.
    let mut avant: HashSet<u64> = HashSet::new();
    for z in [0.5f32, 1.0, 2.0, 3.0, 4.0, 2.0, 1.0, 0.5] {
        b.app.board_zoom = z;
        for step in 0..10 {
            b.app.board_pan = eframe::egui::vec2(0.0, -(step as f32) * 300.0 * z);
            b.chauffer(2);
            b.attendre_vignettes();
            let pass = b.ctx.cumulative_pass_nr();
            let (octets, _) = b.app.thumbs.octets();
            let ecran = b.app.thumbs.octets_a_l_ecran(pass);
            pire = pire.max(octets);
            assert!(octets <= limite + ecran, "zoom {z}, étape {step} : {octets} octets gardés pour une limite de {limite} (+ {ecran} à l'écran)");
            // Une case qui reste à l'écran garde sa vignette (aucune ne disparaît sous les yeux).
            let layout = b.app.board_layout();
            let (xf, view) = (b.app.vignettes.xf, b.app.vignettes.view);
            let visibles: Vec<u64> = layout.cards.iter().filter(|c| xf.rect(c.thumb).intersects(view) && c.mv.is_none()).map(|c| c.id).collect();
            for id in &visibles {
                if avant.contains(id) {
                    assert!(b.app.thumbs.case_gardee(*id), "zoom {z}, étape {step} : la case {id}, restée à l'écran, a perdu sa vignette");
                }
            }
            avant = visibles.into_iter().filter(|id| b.app.thumbs.case_gardee(*id)).collect();
            assert!(!avant.is_empty(), "zoom {z}, étape {step} : des vignettes à l'écran");
        }
    }
    println!("vignettes de la planche : au plus {:.1} Mo gardés (limite {:.0} Mo)", pire as f64 / 1e6, limite as f64 / 1e6);
}

// ------------------------------------------------------------------ 6. disposition de la planche gardée

/// Un tirage pseudo-aléatoire reproductible (xorshift). `tourner` : des caméras tournées aussi.
struct Hasard {
    x: u64,
    tourner: bool,
}

impl Hasard {
    fn new(x: u64) -> Hasard {
        Hasard { x, tourner: false }
    }

    fn n(&mut self, max: usize) -> usize {
        self.x ^= self.x << 13;
        self.x ^= self.x >> 7;
        self.x ^= self.x << 17;
        (self.x % max.max(1) as u64) as usize
    }

    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.n(10_000) as f32 / 10_000.0
    }
}

/// Une planche au hasard : plans, séquences, durées, marques d'image (certaines après la fin du plan), mouvements de caméra
/// (déplacés, zoomés, tournés, de deux à quatre cadres).
fn planche_au_hasard(h: &mut Hasard) -> Document {
    let [w, hh] = [[1920, 1080], [1080, 1920], [2048, 858]][h.n(3)];
    let mut doc = Document::with_frame(w, hh, 24);
    let plans = 1 + h.n(40);
    let mut at = 0;
    for _ in 1..plans {
        at = if h.n(6) == 0 { doc.add_sequence_after(at) } else { doc.add_scene_after(at) };
    }
    for i in 0..doc.panels.len() {
        doc.panels[i].length = 1 + h.n(48);
        let mut marks = Vec::new();
        for _ in 0..h.n(3) {
            marks.push(crate::document::ImageMark { id: doc.alloc(), frame: 1 + h.n(60), ..Default::default() });
        }
        marks.sort_by_key(|m| m.frame);
        doc.panels[i].marks = marks;
        if h.n(3) == 0 {
            camera_au_hasard(&mut doc, i, h);
        }
    }
    doc
}

fn camera_au_hasard(doc: &mut Document, i: usize, h: &mut Hasard) {
    let c0 = doc.panel_camera(i, 0);
    let n = doc.panels[i].frame_count();
    let mut keys = vec![crate::camera::CameraKey::new(0, c0)];
    for k in 1..2 + h.n(3) {
        let mut c = c0;
        c.x += h.f(-900.0, 900.0);
        c.y += h.f(-500.0, 500.0);
        c.zoom *= h.f(0.4, 2.5);
        // (Une caméra tournée seulement pour le constat `random_boards_with_rotated_cameras` : certaines font boucler la disposition
        // sans fin, avant comme après cette passe.)
        if h.tourner && h.n(4) == 0 {
            c.angle = h.f(-40.0, 40.0);
        }
        keys.push(crate::camera::CameraKey::new((k * n / 4).min(n.saturating_sub(1)).max(k), c));
    }
    doc.panels[i].camera = keys;
}

fn reglages_au_hasard(h: &mut Hasard) -> crate::board::BoardSettings {
    crate::board::BoardSettings { card_w: h.f(100.0, 700.0), columns: 1 + h.n(6), rows: if h.n(3) == 0 { 1 + h.n(4) } else { 0 } }
}

fn options_au_hasard(doc: &Document, h: &mut Hasard) -> crate::board::Options {
    let hold = (h.n(3) == 0).then(|| {
        let p = &doc.panels[h.n(doc.panels.len())];
        let r = Rect::from_min_size(pos2(h.f(-200.0, 0.0), h.f(-150.0, 0.0)), eframe::egui::vec2(h.f(100.0, 900.0), h.f(80.0, 600.0)));
        crate::board::Reserve { plan: p.id, rect: r, pose: (h.n(2) == 0).then(|| (h.n(4), h.f(0.0, 600.0))) }
    });
    crate::board::Options { pale_rows: (h.n(4) != 0).then(|| 1 + h.n(5)), hold }
}

fn lignes_au_hasard(doc: &Document, h: &mut Hasard) -> HashMap<u64, usize> {
    let mut lines = HashMap::new();
    for p in &doc.panels {
        for id in std::iter::once(p.id).chain(p.marks.iter().map(|m| m.id)) {
            if h.n(3) == 0 {
                lines.insert(id, 1 + h.n(3));
            }
        }
    }
    lines
}

/// Un changement au hasard de ce qui fait la planche (ou de rien qui la fasse : un nom de plan, une note).
fn changer_au_hasard(doc: &mut Document, s: &mut crate::board::BoardSettings, lines: &mut HashMap<u64, usize>, joins: &mut bool, opt: &mut crate::board::Options, h: &mut Hasard) -> &'static str {
    let i = h.n(doc.panels.len());
    match h.n(14) {
        0 => {
            camera_au_hasard(doc, i, h);
            "caméra"
        }
        1 => {
            if let Some(k) = doc.panels[i].camera.last_mut() {
                k.cam.x += 0.5;
            }
            "une clé de caméra bouge d'un demi-pixel"
        }
        2 => {
            doc.panels[i].camera.clear();
            "caméra retirée"
        }
        3 => {
            let id = doc.alloc();
            let frame = 1 + h.n(doc.panels[i].frame_count() + 4);
            doc.panels[i].marks.push(crate::document::ImageMark { id, frame, ..Default::default() });
            doc.panels[i].marks.sort_by_key(|m| m.frame);
            "marque d'image"
        }
        4 => {
            doc.panels[i].length = 1 + h.n(60);
            "durée"
        }
        5 => {
            *lines = lignes_au_hasard(doc, h);
            "lignes de notes"
        }
        6 => {
            *joins = !*joins;
            "case pâle jointe"
        }
        7 => {
            *s = reglages_au_hasard(h);
            "réglages"
        }
        8 => {
            *opt = options_au_hasard(doc, h);
            "geste de caméra retenu"
        }
        9 => {
            doc.frame = [[1920, 1080], [1080, 1920], [2048, 858]][h.n(3)];
            "taille du cadre"
        }
        10 => {
            let j = h.n(doc.panels.len());
            doc.panels.swap(i, j);
            "deux plans échangés"
        }
        11 => {
            let id = doc.panels[i].scene;
            if let Some(sc) = doc.scene_mut(id) {
                sc.name = format!("Plan {}", h.n(100));
            }
            "nom de plan (rien)"
        }
        12 => {
            doc.panels[i].notes = format!("note {}", h.n(1000));
            "texte d'une note (rien : ses lignes sont dans `lines`)"
        }
        _ => {
            let j = h.n(doc.panels.len());
            doc.panels[i].scene = doc.panels[j].scene;
            "scène d'un plan"
        }
    }
}

/// Constat de la passe (pas un essai de ce qu'elle change) : les mêmes planches au hasard, avec des caméras tournées ; chaque
/// disposition sur un fil à part, cinq secondes au plus. Lancé à part : `... random_boards_with_rotated_cameras -- --ignored
/// --nocapture` ; il dit la première qui ne finit pas.
#[test]
#[ignore]
fn random_boards_with_rotated_cameras() {
    use std::sync::{Arc, Mutex};
    // (Le fil d'essai dit, avant chaque disposition, ce qu'il va disposer ; s'il ne dit plus rien pendant cinq secondes, c'est
    // celle-là.)
    let etat: Arc<Mutex<(u64, String)>> = Arc::new(Mutex::new((0, String::new())));
    let fin = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (e2, f2) = (etat.clone(), fin.clone());
    std::thread::spawn(move || {
        let mut h = Hasard::new(0x9e37_79b9_7f4a_7c15);
        h.tourner = true;
        for planche in 0..40 {
            let mut doc = planche_au_hasard(&mut h);
            let mut s = reglages_au_hasard(&mut h);
            let mut lines = lignes_au_hasard(&doc, &mut h);
            let mut joins = h.n(2) == 0;
            let mut opt = options_au_hasard(&doc, &mut h);
            let mut quoi = "départ";
            for etape in 0..25 {
                let cams: Vec<String> = doc.panels.iter().enumerate().filter(|(_, p)| p.camera.len() > 1).map(|(i, p)| format!("plan {i} ({} images) : {:?}", p.frame_count(), p.camera.iter().map(|k| (k.frame, k.cam.x, k.cam.y, k.cam.zoom, k.cam.angle)).collect::<Vec<_>>())).collect();
                {
                    let mut e = e2.lock().expect("état");
                    e.0 += 1;
                    e.1 = format!("planche {planche}, étape {etape} ({quoi}) : cadre {:?}, {} plans, réglages {s:?}, retenue {opt:?}, case pâle jointe {joins}\n  {}", doc.frame, doc.panels.len(), cams.join("\n  "));
                }
                std::hint::black_box(crate::board::layout_with(&doc, &s, &lines, joins, &opt));
                quoi = changer_au_hasard(&mut doc, &mut s, &mut lines, &mut joins, &mut opt, &mut h);
            }
        }
        f2.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let mut vu = 0;
    let mut depuis = Instant::now();
    while !fin.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let e = etat.lock().expect("état");
        if e.0 != vu {
            vu = e.0;
            depuis = Instant::now();
        } else if depuis.elapsed() > std::time::Duration::from_secs(5) {
            println!("CONSTAT : la disposition ne finit pas en 5 s : {}", e.1);
            return;
        }
    }
    println!("toutes les dispositions finissent");
}

#[test]
fn the_kept_board_layout_is_the_one_computed_from_scratch() {
    let mut h = Hasard::new(0x9e37_79b9_7f4a_7c15);
    let mut faites = 0;
    for planche in 0..40 {
        let mut doc = planche_au_hasard(&mut h);
        let mut s = reglages_au_hasard(&mut h);
        let mut lines = lignes_au_hasard(&doc, &mut h);
        let mut joins = h.n(2) == 0;
        let mut opt = options_au_hasard(&doc, &mut h);
        let mut cache = crate::board::CacheDisposition::default();
        let mut quoi = "départ";
        for etape in 0..25 {
            let neuve = crate::board::layout_with(&doc, &s, &lines, joins, &opt);
            let gardee = crate::board::layout_cache(&doc, &s, &lines, joins, &opt, &mut cache);
            assert_eq!(gardee, neuve, "planche {planche}, étape {etape} ({quoi})");
            // Rien n'a changé : la disposition n'est pas refaite.
            let calculs = cache.calculs;
            assert_eq!(crate::board::layout_cache(&doc, &s, &lines, joins, &opt, &mut cache), neuve);
            assert_eq!(cache.calculs, calculs, "planche {planche}, étape {etape} : refaite sans changement");
            faites += cache.calculs;
            quoi = changer_au_hasard(&mut doc, &mut s, &mut lines, &mut joins, &mut opt, &mut h);
        }
    }
    assert!(faites > 40, "les changements refont la disposition");
}

#[test]
fn the_board_view_keeps_its_layout_and_note_lines_until_something_changes() {
    let mut doc = projet_long(60, 1, 8, 1);
    for i in 0..doc.panels.len() {
        doc.panels[i].notes = if i % 3 == 0 { format!("Plan {i} : le personnage entre par la gauche, regarde la fenêtre, puis sort lentement du cadre, très lentement.") } else { String::new() };
        if i % 4 == 1 {
            let mut h = Hasard::new(i as u64 + 7);
            camera_au_hasard(&mut doc, i, &mut h);
        }
    }
    let mut b = banc(doc, false);
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.board_zoom = 0.0;
    b.chauffer(6);
    let calculs = b.app.vignettes.disposition.borrow().calculs;
    b.chauffer(10);
    assert_eq!(b.app.vignettes.disposition.borrow().calculs, calculs, "au repos, la planche ne se refait pas");
    // Les mêmes lignes de notes et la même planche que le code d'avant (tout refait à chaque image).
    let (lignes, planche) = (b.app.vignettes.lines.clone(), b.app.board_layout());
    crate::perf::set_ancien(true);
    b.chauffer(2);
    let (lignes_avant, planche_avant) = (b.app.vignettes.lines.clone(), b.app.board_layout());
    crate::perf::set_ancien(false);
    assert_eq!(lignes, lignes_avant);
    assert_eq!(planche, planche_avant);
    assert!(lignes.values().any(|&n| n > 1), "des notes sur plusieurs lignes");
    // Une note plus longue : ses lignes et la planche suivent à l'image suivante.
    let court = lignes[&b.app.doc.panels[3].id];
    b.app.doc.panels[3].notes = "Une note courte.".into();
    b.chauffer(2);
    assert!(b.app.vignettes.lines[&b.app.doc.panels[3].id] < court, "la note raccourcie prend moins de lignes");
    assert!(b.app.vignettes.disposition.borrow().calculs > calculs, "la planche suit");
    crate::perf::set_ancien(true);
    b.chauffer(2);
    let avant = (b.app.vignettes.lines.clone(), b.app.board_layout());
    crate::perf::set_ancien(false);
    b.chauffer(2);
    assert_eq!((b.app.vignettes.lines.clone(), b.app.board_layout()), avant);
}
