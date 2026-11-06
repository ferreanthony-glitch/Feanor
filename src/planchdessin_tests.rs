//! Tests du crayon de la planche de références (`planchdessin.rs`, `planchdessinui.rs`) : trait, gomme, annulation et
//! rétablissement, enregistrement et réouverture, ancien fichier, zoom, stylet, fenêtre à part, canevas blanc dans chaque
//! thème, lisibilité sur le blanc, et la règle « sans latence » avec quelques centaines de traits.

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect};

use crate::app::App;
use crate::document::Document;
use crate::planchdessin::{self, CANVAS};
use crate::planchref;
use crate::project;
use crate::references::{self, Kind, Reference, FILE};
use crate::referencesui::planchdessinui::Outil;
use crate::theme::{self, ChoixTheme, Couleurs, ReglageTheme};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-planchdessin-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-planchdessin-app-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

const NONE: egui::Modifiers = egui::Modifiers::NONE;

fn run_frame(ctx: &egui::Context, app: &mut App, t: &mut f64, viewport: egui::ViewportId, events: Vec<egui::Event>, mods: egui::Modifiers) -> egui::FullOutput {
    *t += 0.05;
    let mut info = egui::ViewportInfo::default();
    info.focused = Some(true);
    let mut viewports = egui::ViewportIdMap::default();
    viewports.insert(viewport, info);
    let input = egui::RawInput { viewport_id: viewport, viewports, screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 640.0))), time: Some(*t), events, modifiers: mods, ..Default::default() };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.references_panel(ui));
    })
}

fn step(ctx: &egui::Context, app: &mut App, t: &mut f64, events: Vec<egui::Event>) {
    let _ = run_frame(ctx, app, t, egui::ViewportId::ROOT, events, NONE);
}

fn moved(p: Pos2) -> egui::Event {
    egui::Event::PointerMoved(p)
}

fn button(p: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: NONE }
}

fn key(k: egui::Key) -> egui::Event {
    egui::Event::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers: NONE }
}

fn steps(app: &App) -> usize {
    app.history.names().0.len()
}

fn strokes(app: &App) -> Vec<&Reference> {
    app.doc.references.iter().filter(|r| r.kind == Kind::Stroke).collect()
}

/// Une planche vide, dessinée deux fois (la vue est celle d'origine : un point d'écran = une unité du tableau).
fn app_with_board(name: &str) -> (egui::Context, App, f64) {
    let (ctx, mut app) = headless(name);
    let mut t = 0.0;
    step(&ctx, &mut app, &mut t, vec![]);
    step(&ctx, &mut app, &mut t, vec![]);
    (ctx, app, t)
}

/// Un geste complet : appui au premier point, glissé par les autres, bouton lâché au dernier.
fn drag(ctx: &egui::Context, app: &mut App, t: &mut f64, pts: &[Pos2]) {
    step(ctx, app, t, vec![moved(pts[0])]);
    step(ctx, app, t, vec![button(pts[0], true)]);
    for p in &pts[1..] {
        step(ctx, app, t, vec![moved(*p)]);
    }
    step(ctx, app, t, vec![button(*pts.last().unwrap(), false)]);
    step(ctx, app, t, vec![]);
}

fn line(from: Pos2, to: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n).map(|i| from + (to - from) * (i as f32 / n as f32)).collect()
}

/// Un point de la planche, loin des bords, sûr d'être dans le tableau.
fn board_point(app: &App, dx: f32, dy: f32) -> Pos2 {
    let (rect, _) = app.refs.geometry().expect("dessiné");
    rect.center() + vec2(dx, dy)
}

// ---------------------------------------------------------------------- le trait

#[test]
fn a_pencil_stroke_is_one_undo_step_and_redo_brings_it_back() {
    let (ctx, mut app, mut t) = app_with_board("trait");
    app.run_action("references.crayon");
    assert_eq!(app.refs.pen.outil, Outil::Crayon);
    step(&ctx, &mut app, &mut t, vec![]);
    let n = steps(&app);
    let (panels, cels) = (app.doc.panels.len(), app.doc.cels.len());
    let a = board_point(&app, -100.0, -40.0);
    drag(&ctx, &mut app, &mut t, &line(a, a + vec2(200.0, 60.0), 12));
    let list = strokes(&app);
    assert_eq!(list.len(), 1, "un trait posé");
    let s = list[0];
    assert!(s.points.len() >= 8 && s.width > 0.0 && s.uid.len() > 3 && s.created > 0, "points, épaisseur, identité : {s:?}");
    assert!(s.plans.is_empty(), "un trait n'est lié à aucun plan");
    assert_eq!(steps(&app), n + 1, "une seule étape d'annulation");
    assert_eq!(app.history.names().0.last().copied(), Some("Trait de crayon"));
    assert_eq!((app.doc.panels.len(), app.doc.cels.len()), (panels, cels), "rien ne touche aux plans ni aux dessins");
    let before = app.doc.references.clone();
    app.run_action("edition.annuler");
    assert!(strokes(&app).is_empty(), "annuler retire le trait");
    assert_eq!(steps(&app), n);
    app.run_action("edition.retablir");
    assert_eq!(app.doc.references, before, "rétablir le rend identique");
}

#[test]
fn the_selection_tool_never_draws_and_leaving_the_pencil_gives_back_the_old_gestures() {
    let (ctx, mut app, mut t) = app_with_board("selection");
    // Un élément au centre et une vue à 100 % : le fond, où l'on tire, est autour.
    let id = app.doc.new_reference_id();
    app.doc.add_reference(Reference::block(Kind::Note, id, pos2(0.0, 0.0), "x", 0));
    app.refs.view = Some(references::View { center: pos2(0.0, 0.0), zoom: 1.0 });
    step(&ctx, &mut app, &mut t, vec![]);
    let n = steps(&app);
    let a = board_point(&app, 150.0, 150.0);
    drag(&ctx, &mut app, &mut t, &line(a, a + vec2(120.0, 40.0), 8));
    assert!(strokes(&app).is_empty() && steps(&app) == n, "la sélection ne trace jamais");
    // Avec le crayon : un trait. En sortir : plus de trait, et le fond déplace la vue comme avant.
    app.run_action("references.crayon");
    drag(&ctx, &mut app, &mut t, &line(a, a + vec2(120.0, 40.0), 8));
    assert_eq!(strokes(&app).len(), 1);
    app.run_action("references.crayon");
    assert_eq!(app.refs.pen.outil, Outil::Selection, "l'icône de nouveau : on en sort");
    step(&ctx, &mut app, &mut t, vec![]);
    let before = app.refs.shown_view.unwrap();
    let steps_now = steps(&app);
    drag(&ctx, &mut app, &mut t, &line(a, a + vec2(60.0, 30.0), 6));
    assert_eq!(strokes(&app).len(), 1, "plus de trait");
    assert_eq!(steps(&app), steps_now);
    let after = app.refs.shown_view.unwrap();
    assert!((after.center - before.center).length() > 5.0, "glisser le fond déplace la vue, comme avant : {before:?} -> {after:?}");
}

#[test]
fn escape_cancels_a_stroke_in_progress_then_gives_back_the_selection() {
    let (ctx, mut app, mut t) = app_with_board("echap");
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    let a = board_point(&app, -80.0, 0.0);
    step(&ctx, &mut app, &mut t, vec![moved(a)]);
    step(&ctx, &mut app, &mut t, vec![button(a, true)]);
    step(&ctx, &mut app, &mut t, vec![moved(a + vec2(40.0, 10.0))]);
    assert!(app.refs.pen.busy());
    step(&ctx, &mut app, &mut t, vec![key(egui::Key::Escape)]);
    assert!(!app.refs.pen.busy(), "Échap abandonne le trait");
    assert_eq!(app.refs.pen.outil, Outil::Crayon, "le premier Échap ne fait que ça");
    step(&ctx, &mut app, &mut t, vec![button(a + vec2(40.0, 10.0), false)]);
    assert!(strokes(&app).is_empty(), "le trait abandonné n'est pas posé");
    step(&ctx, &mut app, &mut t, vec![key(egui::Key::Escape)]);
    assert_eq!(app.refs.pen.outil, Outil::Selection, "Échap rend la sélection quand il est libre");
}

#[test]
fn escape_is_left_to_the_text_being_written() {
    let (ctx, mut app, mut t) = app_with_board("echap-texte");
    app.run_action("references.postit");
    for _ in 0..3 {
        step(&ctx, &mut app, &mut t, vec![]);
    }
    assert!(app.refs_editing().is_some());
    app.refs.focus = true;
    // Entrer dans le crayon pose le texte (on n'écrit plus) ; un Échap ensuite rend la sélection.
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    assert_eq!(app.refs_editing(), None, "le texte est posé en entrant dans le crayon");
    step(&ctx, &mut app, &mut t, vec![key(egui::Key::Escape)]);
    assert_eq!(app.refs.pen.outil, Outil::Selection);
}

#[test]
fn strokes_follow_the_zoom_and_pressure_comes_from_the_pen() {
    let (ctx, mut app, mut t) = app_with_board("zoom");
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    let (rect, view) = app.refs.geometry().unwrap();
    assert!((view.zoom - 1.0).abs() < 1e-4);
    let a = rect.center() + vec2(-100.0, 0.0);
    drag(&ctx, &mut app, &mut t, &line(a, a + vec2(200.0, 0.0), 10));
    let at_one = strokes(&app)[0].size[0] as f32;
    // Zoom x2 à la molette, au centre : un trait de 200 points d'écran fait 100 unités du tableau.
    app.refs.view = Some(references::View { center: view.center, zoom: 2.0 });
    step(&ctx, &mut app, &mut t, vec![]);
    let b = rect.center() + vec2(-100.0, 80.0);
    drag(&ctx, &mut app, &mut t, &line(b, b + vec2(200.0, 0.0), 10));
    let second = app.doc.references.last().unwrap().clone();
    assert!((second.size[0] as f32) < at_one * 0.6, "le trait tracé à 200 % est deux fois plus court sur le tableau : {} contre {at_one}", second.size[0]);
    // Les traits suivent le zoom : leur taille à l'écran double avec lui.
    let on_screen = |r: &Reference, z: f32| r.bounds().width() * z;
    let first = strokes(&app)[0].clone();
    assert!((on_screen(&first, 2.0) / on_screen(&first, 1.0) - 2.0).abs() < 1e-3);
    // Stylet : la pression arrive par les événements « tactiles » ; une souris donne la pression pleine.
    let touch = |p: Pos2, f: f32, phase: egui::TouchPhase| egui::Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(1), phase, pos: p, force: Some(f) };
    app.refs.view = Some(references::View { center: view.center, zoom: 1.0 });
    step(&ctx, &mut app, &mut t, vec![]);
    let c = rect.center() + vec2(-100.0, -90.0);
    step(&ctx, &mut app, &mut t, vec![moved(c)]);
    step(&ctx, &mut app, &mut t, vec![touch(c, 0.2, egui::TouchPhase::Start), button(c, true)]);
    for i in 1..=10 {
        let p = c + vec2(20.0 * i as f32, 0.0);
        step(&ctx, &mut app, &mut t, vec![touch(p, 0.2 + 0.08 * i as f32, egui::TouchPhase::Move), moved(p)]);
    }
    step(&ctx, &mut app, &mut t, vec![button(c + vec2(200.0, 0.0), false)]);
    let pen = app.doc.references.last().unwrap();
    let (lo, hi) = pen.points.iter().fold((1.0_f32, 0.0_f32), |(l, h), p| (l.min(p[2]), h.max(p[2])));
    assert!(lo < 0.4 && hi > 0.8, "la pression du stylet est gardée : {lo}..{hi}");
    assert!(first.points.iter().all(|p| p[2] == 1.0), "une souris donne la pression pleine");
    assert!(planchdessin::pressure_factor(0.0) < planchdessin::pressure_factor(1.0));
}

#[test]
fn the_eraser_takes_whole_strokes_and_a_pass_is_one_step() {
    let (ctx, mut app, mut t) = app_with_board("gomme");
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    let o = board_point(&app, -150.0, -100.0);
    for k in 0..3 {
        let a = o + vec2(0.0, 60.0 * k as f32);
        drag(&ctx, &mut app, &mut t, &line(a, a + vec2(300.0, 0.0), 10));
    }
    assert_eq!(strokes(&app).len(), 3);
    // Une image et un post-it sous la gomme ne sont pas touchés.
    let id = app.doc.new_reference_id();
    app.doc.add_reference(Reference::block(Kind::Note, id, app.doc.references[0].center(), "", 0));
    app.run_action("references.gomme");
    assert_eq!(app.refs.pen.outil, Outil::Gomme);
    step(&ctx, &mut app, &mut t, vec![]);
    let n = steps(&app);
    // Un passage vertical qui coupe les deux premiers traits.
    let x = o.x + 150.0;
    drag(&ctx, &mut app, &mut t, &line(pos2(x, o.y - 20.0), pos2(x, o.y + 80.0), 12));
    assert_eq!(strokes(&app).len(), 1, "le passage a gommé deux traits entiers");
    assert!(app.doc.reference(id).is_some(), "la gomme ne touche pas au post-it");
    assert_eq!(steps(&app), n + 1, "un passage = une étape");
    app.run_action("edition.annuler");
    assert_eq!(strokes(&app).len(), 3, "une annulation rend les deux traits");
    app.run_action("edition.retablir");
    assert_eq!(strokes(&app).len(), 1);
    // Un passage dans le vide : aucune étape.
    let n = steps(&app);
    drag(&ctx, &mut app, &mut t, &line(pos2(o.x, o.y - 80.0), pos2(o.x + 300.0, o.y - 80.0), 6));
    assert_eq!(steps(&app), n);
}

#[test]
fn a_stroke_can_be_chosen_moved_recolored_and_deleted() {
    let (ctx, mut app, mut t) = app_with_board("selection-trait");
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    let a = board_point(&app, -100.0, 20.0);
    drag(&ctx, &mut app, &mut t, &line(a, a + vec2(200.0, 0.0), 10));
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    step(&ctx, &mut app, &mut t, vec![]);
    let id = strokes(&app)[0].id;
    // Cliquer sur le trait le choisit (la barre d'outils n'a plus l'encre : le tableau a bougé, on retrouve le trait).
    let on = {
        let (rect, view) = app.refs.geometry().unwrap();
        view.to_screen(rect, app.doc.reference(id).unwrap().center())
    };
    step(&ctx, &mut app, &mut t, vec![moved(on)]);
    step(&ctx, &mut app, &mut t, vec![button(on, true)]);
    assert_eq!(app.refs.sel, vec![id]);
    step(&ctx, &mut app, &mut t, vec![button(on, false)]);
    step(&ctx, &mut app, &mut t, vec![]);
    step(&ctx, &mut app, &mut t, vec![]);
    // (La barre de réglages a décalé le tableau : on retrouve le trait là où il est maintenant.)
    let n = steps(&app);
    let (rect, view) = app.refs.geometry().unwrap();
    let from = view.to_screen(rect, app.doc.reference(id).unwrap().center());
    let before = app.doc.reference(id).unwrap().center();
    step(&ctx, &mut app, &mut t, vec![moved(from)]);
    step(&ctx, &mut app, &mut t, vec![button(from, true)]);
    step(&ctx, &mut app, &mut t, vec![moved(from + vec2(30.0, 20.0))]);
    step(&ctx, &mut app, &mut t, vec![moved(from + vec2(60.0, 40.0))]);
    step(&ctx, &mut app, &mut t, vec![button(from + vec2(60.0, 40.0), false)]);
    let after = app.doc.reference(id).unwrap().center();
    assert!((after - before - vec2(60.0, 40.0) / view.zoom).length() < 0.1, "le trait suit la souris : {before:?} -> {after:?}");
    assert_eq!(steps(&app), n + 1, "déplacer = une étape");
    // Encre du trait choisi.
    app.refs_apply("Encre du trait", |x| x.ink = planchdessin::INKS[4]);
    assert_eq!(app.doc.reference(id).unwrap().ink, planchdessin::INKS[4]);
    app.run_action("references.supprimer");
    assert!(strokes(&app).is_empty());
    app.run_action("edition.annuler");
    assert_eq!(strokes(&app).len(), 1, "supprimer s'annule");
    // Hors du tracé, même dans son rectangle : rien n'est pris.
    let s = strokes(&app)[0].clone();
    let off = s.center() + vec2(0.0, 40.0);
    assert!(!s.hit(off) && s.hit(s.from_local(vec2(0.0, 0.0))), "seul le tracé se prend");
    assert!(references::hit_test_near([s.clone()].iter(), off, 6.0).is_none());
}

#[test]
fn strokes_are_painted_above_everything_and_picked_first() {
    let mut doc = Document::new(32, 32, 24);
    let img = Reference::image(doc.new_reference_id(), "I", "absente.png", [200, 100], pos2(0.0, 0.0));
    doc.add_reference(img);
    let s = Reference::stroke(doc.new_reference_id(), &[(pos2(-50.0, 0.0), 1.0), (pos2(50.0, 0.0), 1.0)], [200, 0, 0], 6.0, 0.5).unwrap();
    let sid = s.id;
    doc.add_reference(s);
    // Une image remontée devant (déplacée) recouvre le trait dans la pile, mais le trait reste pris le premier.
    let top = doc.references[0].id;
    doc.raise_reference(top);
    assert_eq!(doc.references.last().unwrap().id, top);
    assert_eq!(references::hit_test(doc.references.iter(), pos2(0.0, 0.0)), Some(sid));
    assert_eq!(references::hit_test(doc.references.iter(), pos2(0.0, 30.0)), Some(top), "ailleurs, c'est l'image");
}

#[test]
fn strokes_do_not_move_the_arrangement_and_a_frame_carries_them() {
    let mut doc = Document::new(32, 32, 24);
    let a = doc.new_reference_id();
    doc.add_reference(Reference::image(a, "A", "a.png", [100, 100], pos2(0.0, 0.0)));
    let b = doc.new_reference_id();
    doc.add_reference(Reference::image(b, "B", "b.png", [100, 100], pos2(500.0, 0.0)));
    let s = Reference::stroke(doc.new_reference_id(), &[(pos2(900.0, 900.0), 1.0), (pos2(950.0, 900.0), 1.0)], [0, 0, 0], 4.0, 0.5).unwrap();
    let sid = s.id;
    let center = s.center();
    doc.add_reference(s);
    assert!(doc.arrange(&[a, b, sid]));
    assert_eq!(doc.reference(sid).unwrap().center(), center, "ranger ne touche pas aux traits");
    // Un cadre emporte les traits dont le centre est dedans.
    let f = doc.new_reference_id();
    let mut frame = Reference::block(Kind::Frame, f, center, "", 0);
    frame.size = [400, 300];
    doc.add_reference(frame);
    assert!(doc.frame_contents(f).contains(&sid));
}

// ---------------------------------------------------------------------- le fichier

fn some_stroke(doc: &mut Document, at: Pos2) -> u64 {
    let pts: Vec<(Pos2, f32)> = (0..30).map(|i| (at + vec2(i as f32 * 3.1, (i as f32 * 0.4).sin() * 17.3), 0.3 + 0.02 * i as f32)).collect();
    let mut s = Reference::stroke(doc.new_reference_id(), &pts, [30, 110, 220], 7.5, 0.5).unwrap();
    s.stamp("Léa");
    let id = s.id;
    doc.add_reference(s);
    id
}

#[test]
fn strokes_are_saved_in_the_board_file_and_come_back_the_same() {
    let work = temp("fichier");
    let mut doc = Document::new(32, 32, 24);
    let note = doc.new_reference_id();
    doc.add_reference(Reference::block(Kind::Note, note, pos2(10.0, 10.0), "pluie", 2));
    let s1 = some_stroke(&mut doc, pos2(-200.0, 50.0));
    let s2 = some_stroke(&mut doc, pos2(300.0, -80.0));
    doc.reference_mut(s2).unwrap().rotation = 20.0;
    doc.reference_mut(s2).unwrap().scale = 1.5;
    let dir = work.join("Film");
    let manifest = dir.join("Film.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(text.contains("\"version\": 2") && text.contains("\"type\": \"trait\"") && text.contains("\"points\"") && text.contains("\"encre\"") && text.contains("\"epaisseur\""), "{text}");
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.iter().all(|w| !w.contains(FILE)), "{:?}", loaded.warnings);
    let strip = |l: &[Reference]| l.iter().cloned().map(|r| Reference { home: None, ..r }).collect::<Vec<_>>();
    assert_eq!(strip(&loaded.doc.references), strip(&doc.references), "tout est relu, trait pour trait");
    assert_eq!(loaded.doc.reference(s1).unwrap().points.len(), doc.reference(s1).unwrap().points.len());
    // Réécrit tel quel : le même texte (rien n'est perdu ni arrondi à la relecture).
    let again = String::from_utf8(references::to_bytes(&loaded.doc).unwrap().unwrap()).unwrap();
    assert_eq!(again, text, "relu puis réécrit à l'identique");
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn a_file_without_strokes_keeps_its_format_and_an_old_one_reads_without_loss() {
    let old = r#"{ "format": "shot-references", "version": 1, "references": [
        { "id": 7, "nom": "Rue", "fichier": "7.png", "taille": [64, 32], "x": 12.5, "y": -3.0 } ] }"#;
    let list = references::parse(old).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].points.is_empty() && list[0].ink == planchdessin::INK_DEFAULT && list[0].width == 0.0, "rien n'est inventé");
    let mut doc = Document::new(32, 32, 24);
    doc.references = list;
    let text = String::from_utf8(references::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(text.contains("\"version\": 1") && !text.contains("points") && !text.contains("encre") && !text.contains("epaisseur"), "{text}");
    // Une planche de post-it (format 2, d'avant le crayon) se lit de même, sans trait.
    let board = r#"{ "format": "shot-references", "version": 2, "references": [
        { "id": 3, "taille": [220, 190], "x": 0.0, "y": 0.0, "echelle": 1.0, "type": "note", "texte": "Idée", "couleur": 1 } ] }"#;
    let l = references::parse(board).unwrap();
    assert_eq!((l[0].kind, l[0].text.as_str()), (Kind::Note, "Idée"));
    assert!(l[0].points.is_empty());
}

#[test]
fn damaged_strokes_are_made_safe_or_dropped() {
    let bad = r#"{ "format": "shot-references", "version": 2, "references": [
        { "id": 1, "taille": [1, 1], "x": 1e30, "y": 0.0, "type": "trait", "points": "100 200 900 500 500 -200 100000000 0 50 oups 1 2", "epaisseur": -4.0 },
        { "id": 2, "taille": [10, 10], "x": 0.0, "y": 0.0, "type": "trait", "points": "" },
        { "id": 3, "taille": [10, 10], "x": 0.0, "y": 0.0, "type": "note", "points": "100 100 100", "epaisseur": 9.0, "encre": [1, 2, 3] }
    ] }"#;
    let list = references::parse(bad).unwrap();
    assert_eq!(list.len(), 2, "un trait sans point ne reste pas");
    let s = &list[0];
    assert!(s.width >= planchdessin::WIDTH_MIN && s.width <= planchdessin::WIDTH_MAX);
    assert!(s.points.iter().all(|p| p[2] >= 0.0 && p[2] <= 1.0 && p[0].abs() <= 1.0e6), "{:?}", s.points);
    assert!(s.size[0] >= 8 && s.size[1] >= 8);
    let n = &list[1];
    assert!(n.points.is_empty() && n.width == 0.0 && n.ink == planchdessin::INK_DEFAULT, "un post-it n'a pas de trait");
    // Beaucoup de points : éclaircis, jamais coupés d'un côté.
    let many: Vec<(Pos2, f32)> = (0..20_000).map(|i| (pos2(i as f32, (i as f32 * 0.01).sin() * 50.0), 1.0)).collect();
    let r = Reference::stroke(5, &many, [0, 0, 0], 3.0, 0.0).unwrap();
    assert!(r.points.len() <= planchdessin::POINTS_MAX && r.points.len() >= 1000);
    let (first, last) = (r.points[0], r.points[r.points.len() - 1]);
    assert!(last[0] - first[0] > 19_000.0, "le trait garde ses deux bouts");
}

// ---------------------------------------------------------------------- fenêtre à part

#[test]
fn the_pencil_works_in_the_detached_window_too() {
    let (ctx, mut app) = headless("fenetre");
    let vid = crate::referencesui::planchrefui::refs_viewport(false);
    let mut t = 0.0;
    let mut frame = |app: &mut App, events: Vec<egui::Event>| {
        let _ = run_frame(&ctx, app, &mut t, vid, events, NONE);
    };
    app.run_action("references.crayon");
    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    let (rect, _) = app.refs.geometry().expect("dessiné");
    let a = rect.center() + vec2(-60.0, 0.0);
    frame(&mut app, vec![moved(a)]);
    frame(&mut app, vec![button(a, true)]);
    for i in 1..=8 {
        frame(&mut app, vec![moved(a + vec2(15.0 * i as f32, 5.0 * i as f32))]);
    }
    frame(&mut app, vec![button(a + vec2(120.0, 40.0), false)]);
    assert_eq!(strokes(&app).len(), 1, "un trait dans la fenêtre à part");
    // En plein écran, avec le crayon : le premier Échap rend la sélection, le suivant sort du plein écran.
    app.settings.references_fenetre.fullscreen = true;
    frame(&mut app, vec![key(egui::Key::Escape)]);
    assert_eq!(app.refs.pen.outil, Outil::Selection);
    assert!(app.settings.references_fenetre.fullscreen, "le premier Échap rend la sélection");
    frame(&mut app, vec![key(egui::Key::Escape)]);
    assert!(!app.settings.references_fenetre.fullscreen, "le suivant quitte le plein écran, comme avant");
}

// ---------------------------------------------------------------------- canevas blanc

/// La couleur du fond que la planche peint sur toute sa place.
fn canvas_fill(out: &egui::FullOutput, board: Rect) -> Option<Color32> {
    out.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Rect(r) if (r.rect.min - board.min).length() < 0.6 && (r.rect.max - board.max).length() < 0.6 => Some(r.fill),
        _ => None,
    })
}

#[test]
fn the_canvas_is_white_in_every_theme_in_the_panel_and_in_the_detached_window() {
    for choix in [ChoixTheme::GrisSombre, ChoixTheme::Shot, ChoixTheme::Papier, ChoixTheme::GrisClair, ChoixTheme::Sombre, ChoixTheme::Personnalise] {
        theme::installer(&ReglageTheme { choix, perso: Couleurs::default() });
        let (ctx, mut app) = headless("blanc");
        let mut t = 0.0;
        for viewport in [egui::ViewportId::ROOT, crate::referencesui::planchrefui::refs_viewport(false)] {
            let mut last = None;
            for _ in 0..3 {
                last = Some(run_frame(&ctx, &mut app, &mut t, viewport, vec![], NONE));
            }
            let (board, _) = app.refs.geometry().expect("dessiné");
            let fill = canvas_fill(&last.unwrap(), board).unwrap_or_else(|| panic!("{choix:?} : aucun fond de la taille de la planche"));
            assert_eq!(fill, CANVAS, "{choix:?} : le fond de la planche est blanc (fenêtre {viewport:?})");
        }
    }
    theme::installer(&ReglageTheme::default());
    // Fenêtre transparente : le blanc prend l'opacité choisie, il ne devient pas gris.
    let (ctx, mut app) = headless("blanc-transparent");
    app.settings.references_fenetre.transparent = true;
    app.settings.references_fenetre.opacity = 50;
    let vid = crate::referencesui::planchrefui::refs_viewport(true);
    let mut t = 0.0;
    let mut out = run_frame(&ctx, &mut app, &mut t, vid, vec![], NONE);
    for _ in 0..2 {
        out = run_frame(&ctx, &mut app, &mut t, vid, vec![], NONE);
    }
    let (board, _) = app.refs.geometry().unwrap();
    let fill = canvas_fill(&out, board).expect("fond");
    assert!(fill.r() == fill.g() && fill.g() == fill.b() && fill.a() < 255 && fill.a() > 100, "du blanc à moitié transparent : {fill:?}");
}

#[test]
fn what_is_written_on_the_canvas_stays_readable_on_white() {
    let white = Color32::WHITE;
    for kind in [Kind::Note, Kind::Text, Kind::Title, Kind::Frame] {
        for color in 0..planchref::NOTE_COLORS.len() as u8 {
            let r = Reference::block(kind, 1, pos2(0.0, 0.0), "x", color);
            let (ink, _) = crate::referencesui::block_text_look(&r, 1.0);
            let bg = match kind {
                Kind::Note => Color32::from_rgb(r.tint()[0], r.tint()[1], r.tint()[2]),
                Kind::Frame => {
                    let t = r.tint();
                    let mix = |c: u8| (c as f32 * 0.85 + 255.0 * 0.15) as u8;
                    Color32::from_rgb(mix(t[0]), mix(t[1]), mix(t[2]))
                }
                _ => white,
            };
            assert!(theme::contraste(ink, bg) >= 4.5, "{kind:?} couleur {color} : contraste {:.2} (texte {ink:?} sur {bg:?})", theme::contraste(ink, bg));
        }
    }
    // Poignées et contour de sélection : visibles sur le blanc (3:1 au moins).
    assert!(theme::contraste(planchdessin::SELECTION, white) >= 3.0);
    assert!(theme::contraste(planchdessin::CANVAS_TEXT_SOFT, white) >= 4.0);
    // Chaque encre du crayon se voit sur le blanc.
    for (ink, name) in planchdessin::INKS.iter().zip(planchdessin::INK_NAMES) {
        assert!(theme::contraste(Color32::from_rgb(ink[0], ink[1], ink[2]), white) >= 3.0, "{name}");
    }
    // Le trait d'un cadre se voit sur le blanc (la teinte pleine est trop pâle).
    for c in planchref::NOTE_COLORS {
        let edge = planchdessin::ink_on_canvas(c).map(|v| (v as f32 * 1.55).min(255.0) as u8);
        assert!(theme::contraste(Color32::from_rgb(edge[0], edge[1], edge[2]), white) >= 2.2);
    }
}

// ---------------------------------------------------------------------- sans latence

fn many_strokes(doc: &mut Document, n: usize, points: usize) {
    for k in 0..n {
        let (cx, cy) = ((k % 20) as f32 * 220.0, (k / 20) as f32 * 220.0);
        let pts: Vec<(Pos2, f32)> = (0..points).map(|i| (pos2(cx + i as f32 * 2.2, cy + (i as f32 * 0.2 + k as f32).sin() * 40.0), 0.4 + 0.5 * ((i as f32 * 0.1).sin().abs()))).collect();
        let mut s = Reference::stroke(doc.new_reference_id(), &pts, planchdessin::INKS[k % 8], 4.0, 0.5).unwrap();
        s.stamp("Léa");
        doc.add_reference(s);
    }
}

fn frame_ms(ctx: &egui::Context, app: &mut App, t: &mut f64, n: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..n {
        step(ctx, app, t, vec![]);
    }
    start.elapsed().as_secs_f64() * 1000.0 / n as f64
}

#[test]
fn a_few_hundred_strokes_cost_nothing_per_frame_and_nothing_during_a_gesture() {
    let (ctx, mut app) = headless("latence");
    many_strokes(&mut app.doc, 400, 90);
    assert_eq!(strokes(&app).len(), 400);
    let mut t = 0.0;
    // Tout voir : tous les traits sont à l'écran.
    for _ in 0..3 {
        step(&ctx, &mut app, &mut t, vec![]);
    }
    let all_visible = frame_ms(&ctx, &mut app, &mut t, 20);
    // Zoom sur un coin : presque tous les traits sont hors de l'écran.
    let (_, view) = app.refs.geometry().unwrap();
    app.refs.view = Some(references::View { center: pos2(100.0, 100.0), zoom: view.zoom.max(1.0) * 2.0 });
    for _ in 0..3 {
        step(&ctx, &mut app, &mut t, vec![]);
    }
    let corner = frame_ms(&ctx, &mut app, &mut t, 20);
    // Même planche sans aucun trait, au même zoom.
    let (ctx0, mut app0) = headless("latence-vide");
    let mut t0 = 0.0;
    app0.refs.view = app.refs.view;
    for _ in 0..3 {
        step(&ctx0, &mut app0, &mut t0, vec![]);
    }
    let empty = frame_ms(&ctx0, &mut app0, &mut t0, 20);
    // Un geste de crayon sur la planche de 400 traits : le coût de chaque image du geste.
    app.run_action("references.crayon");
    step(&ctx, &mut app, &mut t, vec![]);
    let a = board_point(&app, -60.0, -20.0);
    step(&ctx, &mut app, &mut t, vec![moved(a)]);
    let mut times: Vec<f64> = Vec::new();
    let mut timed = |app: &mut App, ev: Vec<egui::Event>| {
        let s = Instant::now();
        step(&ctx, app, &mut t, ev);
        times.push(s.elapsed().as_secs_f64() * 1000.0);
    };
    timed(&mut app, vec![button(a, true)]);
    for i in 1..=60 {
        timed(&mut app, vec![moved(a + vec2(2.0 * i as f32, (i as f32 * 0.3).sin() * 20.0))]);
    }
    let last = 61; // (l'appui, 60 déplacements, puis le relâchement)
    timed(&mut app, vec![button(a + vec2(120.0, 0.0), false)]);
    // (Le premier appui et le dernier : les deux images où quelque chose d'unique se passe.)
    let (press_ms, release_ms) = (times[0], times[last]);
    let worst_move = times[1..last].iter().cloned().fold(0.0_f64, f64::max);
    let worst = times.iter().cloned().fold(0.0_f64, f64::max);
    assert_eq!(strokes(&app).len(), 401);
    // L'enregistrement des 401 traits et la copie des éléments que la planche fait à chaque image.
    let s = Instant::now();
    let bytes = references::to_bytes(&app.doc).unwrap().unwrap();
    let write_ms = s.elapsed().as_secs_f64() * 1000.0;
    let s = Instant::now();
    for _ in 0..100 {
        let copy = app.doc.references.clone();
        std::hint::black_box(&copy);
    }
    let clone_us = s.elapsed().as_secs_f64() * 1e4;
    let s = Instant::now();
    let parsed = references::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let read_ms = s.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(parsed.len(), 401);
    eprintln!(
        "[planchdessin] 400 traits x 90 points : image avec tout à l'écran {all_visible:.2} ms, sur un coin {corner:.2} ms, planche vide {empty:.2} ms ; \
         image de l'appui {press_ms:.2} ms, du glissé (la pire de 60) {worst_move:.2} ms, du relâchement (le trait entre dans la planche) {release_ms:.2} ms ; copie des éléments {clone_us:.1} µs ; écriture {write_ms:.1} ms ({} Ko), lecture {read_ms:.1} ms",
        bytes.len() / 1024
    );
    // (Marges larges : d'autres compilations tournent en même temps.)
    assert!(corner < empty + 30.0, "hors de l'écran, les traits ne coûtent presque rien : {corner:.2} ms contre {empty:.2} ms");
    assert!(worst_move < 40.0, "une image du glissé sur 400 traits : {worst_move:.2} ms");
    let _ = worst;
    assert!(clone_us < 2000.0, "copier les éléments ne recopie pas les points : {clone_us:.1} µs");
}
