//! Tests de la planche de références (`planchref.rs`, `planchrefui.rs`) : éléments (post-it, texte, titre, cadre),
//! ranger, recadrer, retourner, noir et blanc, opacité, cadre qui emporte son contenu, une étape d'annulation par geste,
//! fichier (aller-retour, ancien projet à l'identique, format 2 seulement quand il le faut), import de la planche d'un
//! autre projet, grandes images sans blocage du fil de l'interface, réglages de la fenêtre à part.

use std::path::{Path, PathBuf};

use eframe::egui::{self, pos2, vec2, Rect};

use crate::app::App;
use crate::document::Document;
use crate::planchref::{self, FenetreReglage};
use crate::project;
use crate::references::{self, Kind, Reference, FILE};
use crate::referencesui::Layout;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-planchref-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_png(path: &Path, w: u32, h: u32, c: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(w, h, image::Rgba(c)).save(path).unwrap();
}

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-planchref-app-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

fn img(doc: &mut Document, name: &str, size: [u32; 2], at: egui::Pos2, scale: f32) -> u64 {
    let id = doc.new_reference_id();
    let mut r = Reference::image(id, name, "absente.png", size, at);
    r.scale = scale;
    doc.add_reference(r);
    id
}

fn block(doc: &mut Document, kind: Kind, at: egui::Pos2, text: &str) -> u64 {
    let id = doc.new_reference_id();
    let mut r = Reference::block(kind, id, at, text, 0);
    r.stamp("Léa");
    doc.add_reference(r);
    id
}

fn without_home(list: &[Reference]) -> Vec<Reference> {
    list.iter().cloned().map(|r| Reference { home: None, ..r }).collect()
}

fn disjoint(a: Rect, b: Rect) -> bool {
    !a.expand(-0.01).intersects(b)
}

// ---------------------------------------------------------------------- ranger

#[test]
fn packing_keeps_sizes_leaves_no_overlap_and_is_deterministic() {
    let items: Vec<(u64, Rect)> = [(300.0, 200.0), (100.0, 400.0), (250.0, 250.0), (80.0, 60.0), (500.0, 120.0), (210.0, 90.0), (60.0, 60.0)]
        .iter()
        .enumerate()
        .map(|(i, (w, h))| (i as u64 + 1, Rect::from_min_size(pos2(i as f32 * 7.0, i as f32 * 3.0), vec2(*w, *h))))
        .collect();
    let (placed, total) = planchref::pack(&items, planchref::GAP);
    assert_eq!(placed.len(), items.len());
    for (id, to) in &placed {
        let from = items.iter().find(|i| i.0 == *id).unwrap().1;
        assert!((to.width() - from.width()).abs() < 1e-3 && (to.height() - from.height()).abs() < 1e-3, "les tailles sont gardées");
        assert!(total.contains_rect(*to), "chacun est dans le bloc");
    }
    for (i, (_, a)) in placed.iter().enumerate() {
        for (_, b) in placed.iter().skip(i + 1) {
            assert!(disjoint(*a, *b), "pas de recouvrement : {a:?} {b:?}");
        }
    }
    // Serré : le bloc est bien plus petit qu'une seule rangée.
    let row: f32 = items.iter().map(|(_, r)| r.width() + planchref::GAP).sum();
    assert!(total.width() < row * 0.7, "{} contre {}", total.width(), row);
    // Presque carré.
    assert!(total.width() / total.height() < 3.0 && total.height() / total.width() < 3.0, "{total:?}");
    let (again, _) = planchref::pack(&items, planchref::GAP);
    assert_eq!(placed, again, "même résultat à chaque fois");
}

#[test]
fn arranging_the_selection_moves_only_positions_and_keeps_the_middle() {
    let mut doc = Document::new(32, 32, 24);
    let a = img(&mut doc, "A", [400, 300], pos2(0.0, 0.0), 0.5);
    let b = img(&mut doc, "B", [100, 100], pos2(900.0, 40.0), 2.0);
    let c = img(&mut doc, "C", [800, 200], pos2(-500.0, 700.0), 0.3);
    let d = img(&mut doc, "D", [50, 50], pos2(40.0, 40.0), 1.0);
    doc.reference_mut(c).unwrap().rotation = 30.0;
    let before: Vec<Reference> = doc.references.clone();
    let union = references::bounds_of(doc.references.iter()).unwrap();
    assert!(doc.arrange(&[a, b, c]));
    for id in [a, b, c, d] {
        let (x, y) = (before.iter().find(|r| r.id == id).unwrap(), doc.reference(id).unwrap());
        assert_eq!((x.scale, x.rotation, x.size), (y.scale, y.rotation, y.size), "tailles et angles intacts");
    }
    assert_eq!(doc.reference(d).unwrap().center(), before.iter().find(|r| r.id == d).unwrap().center(), "ce qui n'est pas choisi ne bouge pas");
    let (ba, bb, bc) = (doc.reference(a).unwrap().bounds(), doc.reference(b).unwrap().bounds(), doc.reference(c).unwrap().bounds());
    assert!(disjoint(ba, bb) && disjoint(ba, bc) && disjoint(bb, bc));
    let now = ba.union(bb).union(bc);
    let was = before.iter().filter(|r| [a, b, c].contains(&r.id)).fold(None, |u: Option<Rect>, r| Some(u.map_or(r.bounds(), |u| u.union(r.bounds())))).unwrap();
    assert!((now.center() - was.center()).length() < 0.5, "rangé autour du même centre");
    assert!(now.area() < union.area(), "plus serré qu'avant");
    assert!(!doc.arrange(&[a]), "un seul élément : rien à ranger");
}

#[test]
fn arranging_a_frame_packs_its_content_and_closes_the_frame_on_it() {
    let mut doc = Document::new(32, 32, 24);
    let f = block(&mut doc, Kind::Frame, pos2(0.0, 0.0), "Décor forêt");
    let a = img(&mut doc, "A", [100, 100], pos2(-150.0, -60.0), 1.0);
    let b = img(&mut doc, "B", [100, 60], pos2(130.0, 80.0), 1.0);
    let n = block(&mut doc, Kind::Note, pos2(0.0, 100.0), "idée");
    let outside = img(&mut doc, "Dehors", [100, 100], pos2(2000.0, 0.0), 1.0);
    let mut inside = doc.frame_contents(f);
    inside.sort();
    let mut want = vec![a, b, n];
    want.sort();
    assert_eq!(inside, want, "le cadre contient ce dont le centre est dedans");
    assert!(doc.arrange(&[f]));
    let frame = doc.reference(f).unwrap().clone();
    let area = frame.bounds();
    for id in [a, b, n] {
        assert!(area.contains_rect(doc.reference(id).unwrap().bounds()), "tout reste dans le cadre refermé");
    }
    assert!(area.width() < 640.0, "le cadre s'est refermé : {}", area.width());
    assert_eq!(doc.reference(outside).unwrap().center(), pos2(2000.0, 0.0));
    let mut again = doc.frame_contents(f);
    again.sort();
    assert_eq!(again, want, "il contient toujours la même chose");
}

// ---------------------------------------------------------------------- recadrer, retourner, opacité

#[test]
fn cropping_keeps_what_is_seen_in_place_even_flipped_and_turned() {
    for (flip_h, flip_v, rotation) in [(false, false, 0.0), (true, false, 0.0), (false, true, 25.0), (true, true, 140.0)] {
        let mut r = Reference::image(1, "t", "1.png", [200, 100], pos2(40.0, -30.0));
        r.scale = 2.0;
        r.flip_h = flip_h;
        r.flip_v = flip_v;
        r.rotation = rotation;
        // Un point du tableau et le pixel de l'image qu'il montre.
        let (sx, sy) = (if flip_h { -1.0 } else { 1.0 }, if flip_v { -1.0 } else { 1.0 });
        let p = r.from_local(vec2(sx * (0.4 - 0.5) * 400.0, sy * (0.55 - 0.5) * 200.0));
        let at = r.unit_at(p).unwrap();
        let mut c = r.clone();
        c.set_crop(Some([0.1, 0.2, 0.7, 0.9]));
        assert!(c.half().x < r.half().x && c.half().y < r.half().y, "la partie visible est plus petite");
        let still = c.unit_at(p).unwrap();
        assert!((still.0 - at.0).abs() < 1e-3 && (still.1 - at.1).abs() < 1e-3, "{flip_h} {flip_v} {rotation} : {at:?} -> {still:?}");
        // Rétablir l'image entière ne bouge rien non plus.
        c.set_crop(None);
        assert!((c.center() - r.center()).length() < 1e-3 && c.crop.is_none());
    }
}

#[test]
fn dragging_a_crop_handle_cuts_the_right_side_of_the_image() {
    let r = Reference::image(1, "t", "1.png", [100, 100], pos2(0.0, 0.0));
    let s = r.scale;
    // Sans miroir : le côté gauche de l'écran est le début de l'image.
    let c = r.crop_from_drag(-1, 0, pos2(-50.0 * s + 25.0 * s, 0.0));
    assert!((c[0] - 0.25).abs() < 1e-3 && c[2] == 1.0 && c[1] == 0.0 && c[3] == 1.0, "{c:?}");
    // Avec miroir horizontal : le côté gauche de l'écran est la fin de l'image.
    let m = Reference { flip_h: true, ..r.clone() };
    let c = m.crop_from_drag(-1, 0, pos2(-50.0 * s + 25.0 * s, 0.0));
    assert!((c[2] - 0.75).abs() < 1e-3 && c[0] == 0.0, "{c:?}");
    // Jamais plus petit que le minimum, jamais hors de l'image.
    let c = r.crop_from_drag(1, 1, pos2(-1.0e6, -1.0e6));
    assert!(c[2] - c[0] >= planchref::CROP_MIN - 1e-6 && c[3] - c[1] >= planchref::CROP_MIN - 1e-6, "{c:?}");
    let c = r.crop_from_drag(-1, -1, pos2(-1.0e6, -1.0e6));
    assert_eq!(c, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn flip_grey_and_opacity_are_undone_one_step_each_by_the_commands() {
    let (_ctx, mut app) = headless("transformer");
    let a = img(&mut app.doc, "A", [100, 50], pos2(0.0, 0.0), 1.0);
    let b = img(&mut app.doc, "B", [100, 50], pos2(300.0, 0.0), 1.0);
    app.refs.sel = vec![a, b];
    app.run_action("references.retourner_h");
    assert!(app.doc.reference(a).unwrap().flip_h && app.doc.reference(b).unwrap().flip_h, "toute la sélection");
    assert_eq!(app.history.names().0.last().copied(), Some("Retourner l'image"));
    app.run_action("references.retourner_v");
    assert!(app.doc.reference(a).unwrap().flip_v);
    app.run_action("references.noir_blanc");
    assert!(app.doc.reference(a).unwrap().grey && app.doc.reference(b).unwrap().grey);
    assert_eq!(app.history.names().0.last().copied(), Some("Références en noir et blanc"));
    app.run_action("references.noir_blanc");
    assert!(!app.doc.reference(a).unwrap().grey, "un second appel l'éteint");
    let n = app.history.names().0.len();
    // L'opacité de toute la sélection, d'un coup.
    app.refs_apply("Opacité de l'élément", |x| x.opacity = 0.4);
    assert_eq!(app.history.names().0.len(), n + 1);
    assert!((app.doc.reference(b).unwrap().opacity - 0.4).abs() < 1e-6);
    for _ in 0..5 {
        app.run_action("edition.annuler");
    }
    for id in [a, b] {
        let r = app.doc.reference(id).unwrap();
        assert!(!r.flip_h && !r.flip_v && !r.grey && r.opacity == 1.0, "tout est défait, une étape par geste");
    }
}

// ---------------------------------------------------------------------- post-it, textes, cadres

#[test]
fn notes_texts_titles_and_frames_are_added_edited_and_undone_by_commands() {
    let (_ctx, mut app) = headless("elements");
    for (action, kind, step) in [
        ("references.postit", Kind::Note, "Ajouter un post-it"),
        ("references.texte", Kind::Text, "Ajouter un texte"),
        ("references.titre", Kind::Title, "Ajouter un titre"),
        ("references.cadre", Kind::Frame, "Ajouter un cadre"),
    ] {
        let n = app.doc.references.len();
        app.refs.sel.clear();
        app.run_action(action);
        assert_eq!(app.doc.references.len(), n + 1, "{action}");
        let id = app.refs.sel[0];
        let r = app.doc.reference(id).unwrap();
        assert_eq!(r.kind, kind);
        assert!(!r.uid.is_empty() && r.created > 0 && r.modified == r.created, "identité posée : {r:?}");
        assert_eq!(app.history.names().0.last().copied(), Some(step));
        assert_eq!(app.refs.text_focus, Some(id), "on peut écrire tout de suite");
    }
    let uids: std::collections::HashSet<&str> = app.doc.references.iter().map(|r| r.uid.as_str()).collect();
    assert_eq!(uids.len(), 4, "des identifiants tous différents");
    assert_eq!(app.doc.references[0].kind, Kind::Frame, "le cadre est au fond");
    // Le texte : une étape, qui s'annule.
    let note = app.doc.references.iter().find(|r| r.kind == Kind::Note).unwrap().id;
    app.refs_step("Écrire dans l'élément");
    app.doc.reference_mut(note).unwrap().text = "Penser à la pluie".into();
    app.run_action("edition.annuler");
    assert_eq!(app.doc.reference(note).unwrap().text, "");
    // Annuler les quatre ajouts.
    for _ in 0..4 {
        app.run_action("edition.annuler");
    }
    assert!(app.doc.references.is_empty());
}

#[test]
fn a_frame_asked_with_a_selection_surrounds_it_behind_it() {
    let (_ctx, mut app) = headless("autour");
    let a = img(&mut app.doc, "A", [100, 100], pos2(0.0, 0.0), 1.0);
    let b = img(&mut app.doc, "B", [100, 100], pos2(400.0, 100.0), 1.0);
    app.refs.sel = vec![a, b];
    app.run_action("references.cadre");
    let f = app.refs.sel[0];
    let frame = app.doc.reference(f).unwrap().clone();
    assert_eq!(frame.kind, Kind::Frame);
    for id in [a, b] {
        assert!(frame.bounds().contains_rect(app.doc.reference(id).unwrap().bounds()));
    }
    assert_eq!(app.doc.references[0].id, f, "derrière");
    assert_eq!(app.history.names().0.last().copied(), Some("Mettre la sélection dans un cadre"));
    app.run_action("edition.annuler");
    assert_eq!(app.doc.references.len(), 2);
}

#[test]
fn only_the_title_strip_and_the_border_of_a_frame_can_be_grabbed() {
    let mut doc = Document::new(32, 32, 24);
    let f = block(&mut doc, Kind::Frame, pos2(0.0, 0.0), "Cadre");
    let fr = doc.reference(f).unwrap().clone();
    let h = fr.half();
    assert!(fr.hit(pos2(0.0, -h.y + 10.0)), "le bandeau");
    assert!(fr.hit(pos2(h.x - 3.0, 0.0)), "le bord");
    assert!(!fr.hit(pos2(0.0, 0.0)), "le milieu laisse passer");
    assert!(!fr.hit(pos2(h.x + 30.0, 0.0)), "dehors");
    // Une image dans le cadre est attrapée avant le cadre, même sous son bandeau.
    let i = img(&mut doc, "I", [100, 40], pos2(0.0, -h.y + 10.0), 1.0);
    assert_eq!(references::hit_test(doc.references.iter(), pos2(0.0, -h.y + 10.0)), Some(i));
    assert_eq!(references::hit_test(doc.references.iter(), pos2(h.x - 3.0, 0.0)), Some(f));
    // Un cadre ne monte pas devant ce qu'il contient.
    assert!(!doc.raise_reference(f));
    assert_eq!(doc.references[0].id, f);
    assert!(doc.lower_reference(i) && doc.references[0].id == f && doc.references[1].id == i, "derrière tout, mais devant les cadres");
}

// ---------------------------------------------------------------------- à la souris

fn step(ctx: &egui::Context, app: &mut App, t: &mut f64, mods: egui::Modifiers, events: Vec<egui::Event>) {
    *t += 0.05;
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))), time: Some(*t), events, modifiers: mods, ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.references_panel(ui));
    });
}

fn moved(p: egui::Pos2) -> egui::Event {
    egui::Event::PointerMoved(p)
}

fn button(p: egui::Pos2, pressed: bool, mods: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: mods }
}

fn ctrl() -> egui::Modifiers {
    egui::Modifiers { ctrl: true, command: true, ..Default::default() }
}

/// Un cadre avec deux images dedans, une image dehors, tout dessiné une fois.
fn app_with_frame(name: &str) -> (egui::Context, App, f64, [u64; 4]) {
    let (ctx, mut app) = headless(name);
    let f = block(&mut app.doc, Kind::Frame, pos2(0.0, 0.0), "Perso Léa");
    let a = img(&mut app.doc, "A", [100, 100], pos2(-150.0, 20.0), 1.0);
    let b = img(&mut app.doc, "B", [100, 100], pos2(120.0, 40.0), 1.0);
    let out = img(&mut app.doc, "Dehors", [100, 100], pos2(1200.0, 20.0), 1.0);
    let mut t = 0.0;
    step(&ctx, &mut app, &mut t, egui::Modifiers::NONE, vec![]);
    step(&ctx, &mut app, &mut t, egui::Modifiers::NONE, vec![]);
    (ctx, app, t, [f, a, b, out])
}

#[test]
fn moving_a_frame_by_its_title_carries_what_it_contains_in_one_undo_step() {
    let (ctx, mut app, mut t, [f, a, b, out]) = app_with_frame("cadre-emporte");
    let (rect, view) = app.refs.geometry().expect("dessiné");
    let before: Vec<_> = [f, a, b, out].iter().map(|i| app.doc.reference(*i).unwrap().center()).collect();
    let fr = app.doc.reference(f).unwrap().clone();
    let grab = view.to_screen(rect, pos2(0.0, -fr.half().y + 12.0));
    let none = egui::Modifiers::NONE;
    step(&ctx, &mut app, &mut t, none, vec![moved(grab)]);
    step(&ctx, &mut app, &mut t, none, vec![button(grab, true, none)]);
    assert_eq!(app.refs.sel, vec![f]);
    step(&ctx, &mut app, &mut t, none, vec![moved(grab + vec2(30.0, 20.0))]);
    step(&ctx, &mut app, &mut t, none, vec![moved(grab + vec2(60.0, 40.0))]);
    step(&ctx, &mut app, &mut t, none, vec![button(grab + vec2(60.0, 40.0), false, none)]);
    let (_, view2) = app.refs.geometry().unwrap();
    let d = vec2(60.0, 40.0) / view2.zoom;
    for (i, id) in [f, a, b].iter().enumerate() {
        let now = app.doc.reference(*id).unwrap().center();
        assert!((now - (before[i] + d)).length() < 0.05, "{id} suit le cadre : {:?} -> {:?}", before[i], now);
    }
    assert_eq!(app.doc.reference(out).unwrap().center(), before[3], "ce qui est dehors ne bouge pas");
    let names = app.history.names().0;
    assert_eq!(names.last().copied(), Some("Déplacer les références"), "le cadre et ce qu'il contient bougent ensemble");
    let n = names.len();
    app.run_action("edition.annuler");
    assert_eq!(app.history.names().0.len(), n - 1, "une seule étape pour tout le geste");
    for (i, id) in [f, a, b, out].iter().enumerate() {
        assert_eq!(app.doc.reference(*id).unwrap().center(), before[i], "tout est revenu");
    }
}

#[test]
fn a_frame_corner_resizes_the_frame_and_the_content_stays() {
    let (ctx, mut app, mut t, [f, a, _b, _out]) = app_with_frame("cadre-taille");
    let (rect, view) = app.refs.geometry().unwrap();
    let fr = app.doc.reference(f).unwrap().clone();
    let none = egui::Modifiers::NONE;
    let a_before = app.doc.reference(a).unwrap().center();
    step(&ctx, &mut app, &mut t, none, vec![moved(fr_title(&view, rect, &fr))]);
    step(&ctx, &mut app, &mut t, none, vec![button(fr_title(&view, rect, &fr), true, none)]);
    step(&ctx, &mut app, &mut t, none, vec![button(fr_title(&view, rect, &fr), false, none)]);
    assert_eq!(app.refs.sel, vec![f], "choisi par son bandeau, ses coins se montrent");
    // (La barre de réglages est apparue : le tableau a changé de place.)
    step(&ctx, &mut app, &mut t, none, vec![]);
    let (rect, view) = app.refs.geometry().unwrap();
    let corner = view.to_screen(rect, fr.corners()[2]);
    step(&ctx, &mut app, &mut t, none, vec![moved(corner)]);
    step(&ctx, &mut app, &mut t, none, vec![button(corner, true, none)]);
    step(&ctx, &mut app, &mut t, none, vec![moved(corner + vec2(40.0, 30.0))]);
    step(&ctx, &mut app, &mut t, none, vec![moved(corner + vec2(80.0, 60.0))]);
    step(&ctx, &mut app, &mut t, none, vec![button(corner + vec2(80.0, 60.0), false, none)]);
    let now = app.doc.reference(f).unwrap();
    let grown = vec2(80.0, 60.0) / view.zoom;
    assert!((now.bounds().width() - (fr.bounds().width() + grown.x)).abs() < 0.5, "{} -> {}", fr.bounds().width(), now.bounds().width());
    assert!((now.bounds().min - fr.bounds().min).length() < 0.2, "le coin opposé ne bouge pas");
    assert_eq!(now.scale, fr.scale);
    assert_eq!(app.doc.reference(a).unwrap().center(), a_before);
    assert_eq!(app.history.names().0.last().copied(), Some("Redimensionner le cadre"));
}

fn fr_title(view: &references::View, rect: Rect, fr: &Reference) -> egui::Pos2 {
    view.to_screen(rect, pos2(fr.center().x, fr.bounds().min.y + 10.0))
}

#[test]
fn a_selection_rectangle_chooses_what_it_touches_and_shift_adds() {
    let (ctx, mut app, mut t, [_f, a, b, out]) = app_with_frame("rectangle");
    let (rect, view) = app.refs.geometry().unwrap();
    let steps = app.history.names().0.len();
    let at = |p: egui::Pos2| view.to_screen(rect, p);
    // Ctrl + tirer sur le fond : un rectangle autour de A seulement.
    let (p0, p1) = (at(pos2(-260.0, -80.0)), at(pos2(-60.0, 120.0)));
    let c = ctrl();
    step(&ctx, &mut app, &mut t, c, vec![moved(p0)]);
    step(&ctx, &mut app, &mut t, c, vec![button(p0, true, c)]);
    step(&ctx, &mut app, &mut t, c, vec![moved(p0 + (p1 - p0) * 0.5)]);
    step(&ctx, &mut app, &mut t, c, vec![moved(p1)]);
    assert_eq!(app.refs.sel, vec![a], "le rectangle choisit ce qu'il touche, pas le cadre qu'il ne contient pas");
    step(&ctx, &mut app, &mut t, c, vec![button(p1, false, c)]);
    assert_eq!(app.history.names().0.len(), steps, "choisir n'est pas une étape");
    // Maj + tirer ailleurs : on ajoute.
    let (q0, q1) = (at(pos2(60.0, -20.0)), at(pos2(200.0, 130.0)));
    let s = egui::Modifiers::SHIFT;
    step(&ctx, &mut app, &mut t, s, vec![moved(q0)]);
    step(&ctx, &mut app, &mut t, s, vec![button(q0, true, s)]);
    step(&ctx, &mut app, &mut t, s, vec![moved(q0 + (q1 - q0) * 0.5)]);
    step(&ctx, &mut app, &mut t, s, vec![moved(q1)]);
    step(&ctx, &mut app, &mut t, s, vec![button(q1, false, s)]);
    let mut sel = app.refs.sel.clone();
    sel.sort();
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(sel, want);
    assert!(!app.refs.sel.contains(&out));
    // Avec le réglage « rectangle » : tirer le fond sans touche suffit.
    app.run_action("references.rectangle");
    assert!(app.refs.marquee_default);
    let none = egui::Modifiers::NONE;
    let (r0, r1) = (at(pos2(-300.0, -200.0)), at(pos2(-250.0, -150.0)));
    step(&ctx, &mut app, &mut t, none, vec![moved(r0)]);
    step(&ctx, &mut app, &mut t, none, vec![button(r0, true, none)]);
    step(&ctx, &mut app, &mut t, none, vec![moved(r1)]);
    step(&ctx, &mut app, &mut t, none, vec![button(r1, false, none)]);
    assert!(app.refs.sel.is_empty(), "un rectangle vide ne choisit rien");
}

// ---------------------------------------------------------------------- le fichier

#[test]
fn the_board_round_trips_through_the_project_with_identity_and_format_two() {
    let work = temp("fichier");
    let mut doc = Document::new(32, 32, 24);
    let f = block(&mut doc, Kind::Frame, pos2(0.0, 0.0), "Décor forêt");
    let n = block(&mut doc, Kind::Note, pos2(10.0, 20.0), "Penser à la pluie\nlumière froide");
    let t = block(&mut doc, Kind::Text, pos2(10.0, 220.0), "à vérifier");
    let ti = block(&mut doc, Kind::Title, pos2(10.0, -220.0), "Forêt");
    {
        let r = doc.reference_mut(n).unwrap();
        r.color = 3;
        r.opacity = 0.6;
        r.rotation = 12.0;
    }
    doc.reference_mut(f).unwrap().size = [900, 500];
    let home = work.join("attente");
    write_png(&home.join("900301.png"), 40, 20, [10, 20, 30, 255]);
    let mut im = Reference::image(doc.new_reference_id(), "Rue", "900301.png", [40, 20], pos2(300.0, 300.0));
    im.home = Some(home.clone());
    im.crop = Some([0.25, 0.0, 1.0, 0.5]);
    im.opacity = 0.8;
    im.flip_h = true;
    im.grey = true;
    im.stamp("Max");
    let i = im.id;
    doc.add_reference(im);
    let dir = work.join("Film");
    let manifest = dir.join("Film.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(text.contains("\"version\": 2"), "format 2 : il y a du nouveau : {text}");
    for key in ["\"type\": \"cadre\"", "\"type\": \"note\"", "\"type\": \"texte\"", "\"type\": \"titre\"", "\"uid\"", "\"auteur\": \"Léa\"", "\"cree\"", "\"recadrage\"", "\"opacite\""] {
        assert!(text.contains(key), "{key} : {text}");
    }
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.iter().all(|w| !w.contains(FILE)), "{:?}", loaded.warnings);
    assert_eq!(without_home(&loaded.doc.references), without_home(&doc.references), "tout est relu, dans l'ordre de la pile");
    let r = loaded.doc.reference(n).unwrap();
    assert_eq!((r.kind, r.text.as_str(), r.color, r.rotation), (Kind::Note, "Penser à la pluie\nlumière froide", 3, 12.0));
    assert_eq!(loaded.doc.reference(i).unwrap().crop, Some([0.25, 0.0, 1.0, 0.5]));
    assert_eq!(loaded.doc.reference(i).unwrap().author, "Max");
    assert!(loaded.doc.reference(t).is_some() && loaded.doc.reference(ti).is_some());
    // Les numéros suivants ne retombent pas sur ceux des éléments relus.
    let mut d2 = loaded.doc;
    let nid = d2.new_reference_id();
    assert!(d2.references.iter().all(|r| r.id != nid));
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn an_old_project_reads_identically_and_stays_format_one_until_something_new_is_used() {
    let old = r#"{
  "format": "shot-references",
  "version": 1,
  "references": [
    { "id": 7, "nom": "Rue", "fichier": "7.png", "taille": [64, 32], "etiquettes": ["décor"], "x": 12.5, "y": -3.0,
      "echelle": 0.5, "rotation": 20.0, "miroir_h": true, "gris": true, "plans": [] },
    { "id": 9, "nom": "Cuisine", "fichier": "9.jpg", "taille": [10, 10], "x": 0.0, "y": 0.0 }
  ]
}"#;
    let list = references::parse(old).unwrap();
    assert_eq!(list.len(), 2);
    let r = &list[0];
    assert_eq!((r.id, r.name.as_str(), r.file.as_str(), r.size), (7, "Rue", "7.png", [64, 32]));
    assert_eq!((r.x, r.y, r.scale, r.rotation, r.flip_h, r.grey), (12.5, -3.0, 0.5, 20.0, true, true));
    assert_eq!((r.kind, r.opacity, r.crop, r.color, r.text.as_str()), (Kind::Image, 1.0, None, 0, ""), "rien de nouveau n'est inventé");
    assert!(r.author.is_empty() && r.created == 0, "pas d'auteur ni de date pour un ancien élément");
    assert!(r.uid.is_empty() && list[1].uid.is_empty(), "lu à l'identique : l'identifiant vient à la première modification");
    let mut touched = list[0].clone();
    touched.touch(1_700_000_000);
    assert!(!touched.uid.is_empty() && touched.modified == 1_700_000_000);
    // Réécrit sans rien de nouveau : toujours le format 1, lisible par l'ancienne version.
    let mut doc = Document::new(32, 32, 24);
    doc.references = list;
    let bytes = references::to_bytes(&doc).unwrap().unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("\"version\": 1"), "{text}");
    assert!(!text.contains("\"type\"") && !text.contains("\"opacite\"") && !text.contains("\"recadrage\""), "{text}");
    // Dès qu'on s'en sert : format 2.
    doc.references[0].opacity = 0.5;
    let text = String::from_utf8(references::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(text.contains("\"version\": 2"));
    // Un format plus récent est refusé (et le fichier mis de côté, jamais écrasé : voir `references_tests`).
    let newer = old.replace("\"version\": 1", "\"version\": 3");
    assert!(references::parse(&newer).is_err());
}

#[test]
fn damaged_new_fields_are_made_safe_on_reading() {
    let bad = r#"{ "format": "shot-references", "version": 2, "references": [
      { "id": 1, "type": "note", "texte": "ok", "fichier": "../../x.png", "taille": [0, 99999999], "x": 0, "y": 0,
        "couleur": 99, "opacite": -4.0, "recadrage": [0.5, 0.5, 0.5, 0.5] },
      { "id": 2, "nom": "I", "fichier": "2.png", "taille": [10, 10], "x": 0, "y": 0, "recadrage": [0.9, 0.0, 0.1, 1.0], "opacite": 7.0 }
    ] }"#;
    let list = references::parse(bad).unwrap();
    let n = &list[0];
    assert_eq!(n.file, "", "un bloc n'a pas de fichier");
    assert!(n.color < planchref::NOTE_COLORS.len() as u8 && n.opacity >= 0.05 && n.crop.is_none());
    assert!(n.size[0] >= 8 && n.size[1] <= 1 << 16);
    let i = &list[1];
    assert!(i.crop.is_none() && i.opacity <= 1.0, "un recadrage à l'envers disparaît, l'opacité est ramenée à 1");
}

// ---------------------------------------------------------------------- importer la planche d'un autre projet

#[test]
fn importing_another_projects_board_copies_items_and_images_beside_the_existing_ones() {
    let work = temp("import-planche");
    // Le projet d'origine : une image, un post-it, un cadre.
    let mut other = Document::new(32, 32, 24);
    let home = work.join("attente");
    write_png(&home.join("900401.png"), 20, 10, [200, 30, 30, 255]);
    let mut im = Reference::image(other.new_reference_id(), "Source", "900401.png", [20, 10], pos2(0.0, 0.0));
    im.home = Some(home.clone());
    im.plans = vec![99];
    let im_id = im.id;
    other.add_reference(im);
    let nid = block(&mut other, Kind::Note, pos2(400.0, 0.0), "idée d'ailleurs");
    let old_uids: Vec<String> = other.references.iter().map(|r| r.uid.clone()).collect();
    let other_manifest = work.join("Autre").join("Autre.pelure");
    project::save(&other, &other_manifest, &mut Default::default()).unwrap();
    assert!(work.join("Autre").join("references").join("900401.png").exists());

    // Ce projet : une planche déjà là, enregistré.
    let (ctx, mut app) = headless("import-planche");
    let manifest = work.join("Ici").join("Ici.pelure");
    app.path = Some(manifest.clone());
    let mine = img(&mut app.doc, "Existante", [100, 100], pos2(0.0, 0.0), 1.0);
    let mine_box = app.doc.reference(mine).unwrap().bounds();
    app.refs_import_board(other_manifest.clone());
    for _ in 0..600 {
        app.tick_references(&ctx);
        if app.doc.references.len() > 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(app.doc.references.len(), 3, "les deux éléments sont ajoutés à côté de celui qui y était : {}", app.status);
    assert_eq!(app.doc.reference(mine).unwrap().center(), mine_box.center(), "ce qui y était ne bouge pas");
    let added: Vec<&Reference> = app.doc.references.iter().filter(|r| r.id != mine).collect();
    assert!(added.iter().all(|r| r.bounds().min.x > mine_box.max.x), "à droite de l'existant");
    let ids: std::collections::HashSet<u64> = app.doc.references.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), 3, "numéros tous différents");
    assert!(added.iter().all(|r| !old_uids.contains(&r.uid) && !r.uid.is_empty()), "ce sont d'autres éléments : identifiants neufs");
    assert!(added.iter().all(|r| r.plans.is_empty()), "les plans de l'autre projet n'ont pas de sens ici");
    let copied = added.iter().find(|r| !r.file.is_empty()).expect("l'image");
    assert_eq!(copied.name, "Source");
    assert!(work.join("Ici").join("references").join(&copied.file).exists(), "l'image est copiée dans le dossier du projet");
    let note = added.iter().find(|r| r.kind == Kind::Note).unwrap();
    assert_eq!(note.text, "idée d'ailleurs");
    assert_eq!(app.history.names().0.last().copied(), Some("Importer la planche d'un autre projet"));
    // Une seule étape : annuler retire tout ce qui est venu.
    app.run_action("edition.annuler");
    assert_eq!(app.doc.references.len(), 1);
    // Un projet sans planche : un message, rien d'ajouté.
    let empty = work.join("Vide").join("Vide.pelure");
    project::save(&Document::new(32, 32, 24), &empty, &mut Default::default()).unwrap();
    app.refs_import_board(empty);
    assert!(app.status.contains("pas de planche"), "{}", app.status);
    assert!(app.refs.board_import.is_none());
    std::fs::remove_dir_all(&work).ok();
}

// ---------------------------------------------------------------------- sans blocage

#[test]
fn a_huge_image_never_blocks_the_interface_thread() {
    let work = temp("grande");
    let (ctx, mut app) = headless("grande");
    let home = work.join("fichiers");
    std::fs::create_dir_all(&home).unwrap();
    // Une grande image (JPEG : un décodage qui se compte en dizaines de millisecondes).
    let big = image::RgbImage::from_fn(6000, 4000, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8]));
    big.save(home.join("1.jpg")).unwrap();
    let id = app.doc.new_reference_id();
    let mut r = Reference::image(id, "Grande", "1.jpg", [6000, 4000], pos2(0.0, 0.0));
    r.home = Some(home);
    app.doc.add_reference(r);
    app.refs.layout = Layout::Board;
    let mut t = 0.0;
    let none = egui::Modifiers::NONE;
    let started = std::time::Instant::now();
    step(&ctx, &mut app, &mut t, none, vec![]);
    let first = started.elapsed();
    assert_eq!(app.refs.cache_bytes(), 0, "la première image d'écran ne contient pas l'image : elle est demandée à un fil à part");
    assert!(first < std::time::Duration::from_millis(1500), "la première image d'écran ne décode rien : {first:?}");
    // Les images d'écran suivantes restent courtes, le temps que l'image arrive.
    let mut worst = std::time::Duration::ZERO;
    for _ in 0..600 {
        let s = std::time::Instant::now();
        step(&ctx, &mut app, &mut t, none, vec![]);
        worst = worst.max(s.elapsed());
        if app.refs.cache_bytes() > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(app.refs.cache_bytes() > 0, "l'image finit par arriver");
    assert!(worst < std::time::Duration::from_millis(400), "aucune image d'écran n'attend le décodage : {worst:?}");
    // Le niveau demandé est une vignette, pas les 24 millions de pixels.
    assert!(app.refs.cache_bytes() < 6000 * 4000 * 4, "{}", app.refs.cache_bytes());
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn the_panel_draws_every_kind_in_every_layout_without_panicking() {
    let (ctx, mut app, mut t, _ids) = app_with_frame("dessin");
    block(&mut app.doc, Kind::Note, pos2(0.0, 300.0), "Un post-it avec un texte assez long pour passer à la ligne plusieurs fois");
    block(&mut app.doc, Kind::Text, pos2(300.0, 300.0), "Un texte");
    block(&mut app.doc, Kind::Title, pos2(-300.0, 300.0), "Un titre");
    let rot = block(&mut app.doc, Kind::Note, pos2(500.0, 100.0), "tourné");
    app.doc.reference_mut(rot).unwrap().rotation = 33.0;
    app.doc.reference_mut(rot).unwrap().opacity = 0.5;
    let crop = img(&mut app.doc, "Rognée", [200, 100], pos2(-300.0, -300.0), 1.0);
    app.doc.reference_mut(crop).unwrap().set_crop(Some([0.2, 0.2, 0.8, 0.9]));
    for layout in [Layout::Board, Layout::Grid, Layout::List] {
        app.refs.layout = layout;
        for sel in [vec![], vec![rot], vec![crop], app.doc.references.iter().map(|r| r.id).collect()] {
            app.refs.sel = sel;
            step(&ctx, &mut app, &mut t, egui::Modifiers::NONE, vec![]);
        }
    }
    // Le recadrage en cours.
    app.refs.layout = Layout::Board;
    app.refs.sel = vec![crop];
    app.run_action("references.recadrer");
    assert_eq!(app.refs.crop_mode, Some(crop));
    step(&ctx, &mut app, &mut t, egui::Modifiers::NONE, vec![]);
    app.run_action("references.recadrer");
    assert_eq!(app.refs.crop_mode, None);
    // Zoom très loin : le texte illisible n'est pas dessiné, rien ne plante.
    if let Some((_, v)) = app.refs.geometry() {
        app.refs.view = Some(references::View { zoom: references::ZOOM_MIN, ..v });
    }
    step(&ctx, &mut app, &mut t, egui::Modifiers::NONE, vec![]);
}

#[test]
fn arranging_everything_without_a_selection_is_one_step_and_the_frames_stay() {
    let (_ctx, mut app, _t, [f, a, b, out]) = app_with_frame("ranger-tout");
    app.refs.sel.clear();
    let frame_before = app.doc.reference(f).unwrap().clone();
    let n = app.history.names().0.len();
    app.run_action("references.ranger");
    assert_eq!(app.history.names().0.len(), n + 1);
    assert_eq!(app.history.names().0.last().copied(), Some("Ranger toute la planche"));
    assert_eq!(app.doc.reference(f).unwrap().center(), frame_before.center(), "les cadres ne sont pas rangés avec le reste");
    let bounds: Vec<Rect> = [a, b, out].iter().map(|i| app.doc.reference(*i).unwrap().bounds()).collect();
    assert!(disjoint(bounds[0], bounds[1]) && disjoint(bounds[0], bounds[2]) && disjoint(bounds[1], bounds[2]));
    app.run_action("edition.annuler");
    assert_eq!(app.doc.reference(out).unwrap().center(), pos2(1200.0, 20.0));
}

#[test]
fn ordering_puts_the_selection_in_front_or_behind_in_one_step() {
    let (_ctx, mut app) = headless("ordre");
    let a = img(&mut app.doc, "A", [10, 10], pos2(0.0, 0.0), 1.0);
    let b = img(&mut app.doc, "B", [10, 10], pos2(50.0, 0.0), 1.0);
    let c = img(&mut app.doc, "C", [10, 10], pos2(100.0, 0.0), 1.0);
    app.refs.sel = vec![a];
    app.run_action("references.devant");
    assert_eq!(app.doc.references.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, c, a]);
    assert_eq!(app.history.names().0.last().copied(), Some("Mettre devant"));
    app.refs.sel = vec![c];
    app.run_action("references.derriere");
    assert_eq!(app.doc.references.iter().map(|r| r.id).collect::<Vec<_>>(), vec![c, b, a]);
    app.run_action("edition.annuler");
    assert_eq!(app.doc.references.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, c, a]);
}

// ---------------------------------------------------------------------- la fenêtre à part

#[test]
fn the_window_settings_are_remembered_cleaned_and_default_to_nothing_written() {
    let w = FenetreReglage::default();
    assert!(w.est_defaut() && w.alpha() == 1.0);
    assert!(toml::to_string(&w).unwrap().trim().is_empty(), "rien n'est écrit par défaut");
    let set = FenetreReglage { detached: true, fullscreen: true, screen: 2, transparent: true, opacity: 55, on_top: true };
    let text = toml::to_string(&set).unwrap();
    let back: FenetreReglage = toml::from_str(&text).unwrap();
    assert_eq!(back, set, "{text}");
    assert!((set.alpha() - 0.55).abs() < 1e-6);
    assert_eq!(FenetreReglage { transparent: false, opacity: 30, ..set.clone() }.alpha(), 1.0, "pas transparente : opaque");
    let wild = FenetreReglage { opacity: 3, screen: 200, ..set }.nettoye();
    assert!(wild.opacity == 20 && wild.screen <= 16);
    // Un réglage inconnu ou absent : les valeurs d'origine.
    let old: FenetreReglage = toml::from_str("").unwrap();
    assert!(old.est_defaut());
}

#[test]
fn the_window_commands_change_the_settings_and_they_survive_a_restart() {
    let (_ctx, mut app) = headless("fenetre");
    assert!(app.settings.references_fenetre.est_defaut());
    app.run_action("references.toujours_dessus");
    assert!(app.settings.references_fenetre.on_top);
    app.run_action("references.ecran_2");
    let w = app.settings.references_fenetre.clone();
    assert!(w.fullscreen && w.screen == 2, "{w:?}");
    assert_eq!(app.detach_request, Some(crate::dock::Pane::References), "elle est d'abord détachée");
    // Réglages relus du disque, comme au démarrage suivant.
    let reloaded = crate::settings::Settings::load();
    assert_eq!(reloaded.references_fenetre, w, "retenus d'une séance à l'autre");
    app.run_action("references.plein_ecran");
    assert!(!app.settings.references_fenetre.fullscreen, "un second appel en sort");
    app.run_action("references.transparente");
    assert!(app.settings.references_fenetre.transparent);
    assert_eq!(crate::settings::Settings::load().references_fenetre.screen, 2);
    app.run_action("references.transparente");
    assert!(!app.settings.references_fenetre.transparent);
}

#[test]
fn screens_are_listed_main_first_and_looked_up_by_number() {
    let list = vec![
        planchref::Ecran { x: 0, y: 0, largeur: 1920, hauteur: 1080 },
        planchref::Ecran { x: 1920, y: 0, largeur: 2560, hauteur: 1440 },
        planchref::Ecran { x: -1280, y: 100, largeur: 1280, hauteur: 1024 },
    ];
    assert_eq!(planchref::ecran_numero(&list, 1), Some(list[0]));
    assert_eq!(planchref::ecran_numero(&list, 2), Some(list[1]));
    assert_eq!(planchref::ecran_numero(&list, 4), None);
    assert_eq!(planchref::ecran_numero(&list, 0), None, "0 : l'écran où est la fenêtre");
    assert_eq!(planchref::ecran_de(&list, 2000, 50), Some(1));
    assert_eq!(planchref::ecran_de(&list, -10, 200), Some(2));
    assert_eq!(planchref::ecran_de(&[], 0, 0), None);
    // Windows : la liste réelle est cohérente (au moins un écran, le principal d'abord, tailles positives).
    let real = planchref::ecrans();
    if cfg!(windows) && !real.is_empty() {
        assert!(real[0].contient(0, 0) || real.iter().all(|e| !e.contient(0, 0)), "{real:?}");
        assert!(real.iter().all(|e| e.largeur > 0 && e.hauteur > 0));
    }
}

#[test]
fn dates_are_written_in_french_order() {
    assert_eq!(planchref::date_fr(0), "01/01/1970");
    assert_eq!(planchref::date_fr(951_782_400), "29/02/2000");
    assert_eq!(planchref::date_fr(951_782_400 + 86_399), "29/02/2000");
    assert_eq!(planchref::date_fr(951_782_400 + 86_400), "01/03/2000");
}

#[test]
fn every_new_command_is_registered_and_runs_without_a_project_panel() {
    for a in [
        "references.postit",
        "references.texte",
        "references.titre",
        "references.cadre",
        "references.ranger",
        "references.recadrer",
        "references.retourner_h",
        "references.retourner_v",
        "references.noir_blanc",
        "references.devant",
        "references.derriere",
        "references.tout_selectionner",
        "references.importer_planche",
        "references.rectangle",
        "references.detacher",
        "references.plein_ecran",
        "references.ecran_2",
        "references.transparente",
        "references.toujours_dessus",
    ] {
        assert!(crate::keymap::ACTIONS.iter().any(|x| x.id == a), "{a} est une action");
    }
    let (_ctx, mut app) = headless("commandes");
    // Sans rien de choisi, les commandes de transformation disent pourquoi et ne cassent rien.
    for a in ["references.retourner_h", "references.noir_blanc", "references.devant", "references.derriere", "references.recadrer", "references.ranger"] {
        app.run_action(a);
        assert!(app.doc.references.is_empty());
    }
}

// ---------------------------------------------------------------------- démonstration pour l'écran

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

/// Une copie du projet d'essai (`PLANCHREF_DEMO_SRC`) dans `PLANCHREF_DEMO_DIR`, avec une planche de références faite de formes et de
/// dégradés générés par code (aucune image venue d'ailleurs) : post-it, titre, texte, cadres, images rognées, en noir et blanc, transparentes.
#[test]
#[ignore = "écrit une copie du projet d'essai avec une planche de démonstration"]
fn generate_a_board_demo_project_for_the_screen() {
    let src = PathBuf::from(std::env::var("PLANCHREF_DEMO_SRC").expect("PLANCHREF_DEMO_SRC"));
    let dst = PathBuf::from(std::env::var("PLANCHREF_DEMO_DIR").expect("PLANCHREF_DEMO_DIR"));
    std::fs::remove_dir_all(&dst).ok();
    copy_dir(&src, &dst);
    let manifest = std::fs::read_dir(&dst).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "pelure" || x == "shot")).expect("le projet");
    let mut loaded = project::load(&manifest).unwrap();
    let doc = &mut loaded.doc;
    let home = dst.join("attente-demo");
    std::fs::create_dir_all(&home).unwrap();
    let gradient = |name: &str, w: u32, h: u32, a: [u8; 3], b: [u8; 3], disc: Option<[u8; 3]>| {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            let k = y as f32 / h as f32;
            let mut c = [0, 1, 2].map(|i| (a[i] as f32 * (1.0 - k) + b[i] as f32 * k) as u8);
            if let Some(d) = disc {
                let (dx, dy) = (x as f32 - w as f32 * 0.68, y as f32 - h as f32 * 0.36);
                if (dx * dx + dy * dy).sqrt() < h as f32 * 0.2 {
                    c = d;
                }
            }
            image::Rgb(c)
        });
        img.save(home.join(name)).unwrap();
    };
    gradient("1001.png", 640, 360, [120, 170, 230], [60, 110, 70], Some([255, 236, 170]));
    gradient("1002.png", 480, 320, [30, 50, 70], [10, 20, 30], Some([220, 220, 240]));
    gradient("1003.png", 360, 480, [200, 120, 90], [90, 40, 60], Some([250, 210, 120]));
    gradient("1004.png", 520, 300, [90, 140, 90], [30, 70, 40], None);
    let mut add_img = |doc: &mut Document, name: &str, file: &str, size: [u32; 2], at: egui::Pos2, scale: f32| -> u64 {
        let id = doc.new_reference_id();
        let mut r = Reference::image(id, name, file, size, at);
        r.scale = scale;
        r.home = Some(home.clone());
        r.stamp("Anthony");
        doc.add_reference(r);
        id
    };
    let title = block(doc, Kind::Title, pos2(0.0, -420.0), "Séquence de la forêt");
    doc.reference_mut(title).unwrap().size = [900, 110];
    let f1 = block(doc, Kind::Frame, pos2(-380.0, 0.0), "Décor forêt");
    let a = add_img(doc, "Clairière", "1001.png", [640, 360], pos2(-560.0, -40.0), 0.5);
    let b = add_img(doc, "Sous-bois", "1004.png", [520, 300], pos2(-250.0, 60.0), 0.4);
    doc.reference_mut(b).unwrap().set_crop(Some([0.15, 0.1, 0.85, 0.95]));
    let n1 = block(doc, Kind::Note, pos2(-400.0, 190.0), "Lumière rasante,\nbrume au sol");
    doc.reference_mut(n1).unwrap().rotation = -3.0;
    let f2 = block(doc, Kind::Frame, pos2(330.0, 0.0), "Perso Léa");
    doc.reference_mut(f2).unwrap().color = 1;
    let c = add_img(doc, "Léa de nuit", "1003.png", [360, 480], pos2(260.0, 10.0), 0.4);
    doc.reference_mut(c).unwrap().grey = true;
    let d = add_img(doc, "Lune", "1002.png", [480, 320], pos2(470.0, 30.0), 0.35);
    doc.reference_mut(d).unwrap().opacity = 0.6;
    let n2 = block(doc, Kind::Note, pos2(380.0, 190.0), "Manteau rouge, jamais d'ombre sur son visage");
    doc.reference_mut(n2).unwrap().color = 1;
    let t = block(doc, Kind::Text, pos2(0.0, 360.0), "À vérifier avec la décoratrice : la couleur du ciel au plan 12");
    doc.reference_mut(t).unwrap().size = [700, 80];
    assert!(doc.arrange(&[f1]) && doc.arrange(&[f2]));
    project::save(doc, &manifest, &mut loaded.written).unwrap();
    println!("PROJET DE DÉMONSTRATION : {}", manifest.display());
}