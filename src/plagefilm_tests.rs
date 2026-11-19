//! Plage de lecture du film (`plagefilm.rs`) : ses bornes, ses bouts accrochés aux plans, la lecture du montage qui reste
//! dedans (en boucle ou non, en avant ou en arrière), les plans choisis, et la lecture sans plage, inchangée.

use eframe::egui::{self, RawInput};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::plagefilm::{lecture_du_film, Ancre, PlageFilm};
use crate::workspace::Espace;

/// Trois plans de 10, 20 et 30 images (60 en tout).
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (p, n) in doc.panels.iter_mut().zip([10, 20, 30]) {
        p.set_length(n);
    }
    doc
}

fn ancre(doc: &Document, plan: usize, image: usize) -> Ancre {
    Ancre { plan: doc.panels[plan].id, image }
}

#[test]
fn the_range_is_read_between_its_two_ends() {
    let doc = film();
    let rien = PlageFilm::default();
    assert_eq!(rien.bornes(&doc), None, "sans plage, rien ne borne");
    assert_eq!(lecture_du_film(&rien, &doc), (0, 60), "tout le film");

    // De l'image 5 du plan 2 (15 du film) à l'image 3 du plan 3 (33), comprise.
    let p = PlageFilm { debut: Some(ancre(&doc, 1, 5)), fin: Some(ancre(&doc, 2, 3)) };
    assert_eq!(p.bornes(&doc), Some((15, 34)));
    // Un seul bout : jusqu'à la fin du film, ou depuis son début.
    assert_eq!(PlageFilm { debut: p.debut, fin: None }.bornes(&doc), Some((15, 60)));
    assert_eq!(PlageFilm { debut: None, fin: p.fin }.bornes(&doc), Some((0, 34)));
    // Bouts croisés : comme les points d'un plan, la plage ne compte plus.
    let croisee = PlageFilm { debut: p.fin, fin: p.debut };
    assert_eq!(croisee.bornes(&doc), None);
    // Tout le film : rien à borner.
    let tout = PlageFilm { debut: Some(ancre(&doc, 0, 0)), fin: Some(ancre(&doc, 2, 29)) };
    assert_eq!(tout.bornes(&doc), None);
}

#[test]
fn the_ends_follow_their_plans() {
    let mut doc = film();
    let p = PlageFilm { debut: Some(ancre(&doc, 1, 5)), fin: Some(ancre(&doc, 2, 3)) };
    // Le premier plan s'allonge de 4 images : la plage avance avec son contenu.
    doc.panels[0].set_length(14);
    assert_eq!(p.bornes(&doc), Some((19, 38)));
    // Le plan du début raccourci sous son ancre : la dernière image du plan.
    doc.panels[1].set_length(3);
    assert_eq!(p.bornes(&doc), Some((14 + 2, 14 + 3 + 3 + 1)));
    // Le plan de la fin supprimé : il ne reste que le début, jusqu'au bout du film.
    let id = doc.panels[2].id;
    doc.remove_panels(&[id]);
    assert_eq!(p.bornes(&doc), Some((16, 17)), "le début seul, jusqu'au bout du film (17 images)");
}

/// Un film de trois plans de 10 images, ouvert dans l'espace Montage, sans son.
fn fixture(nom: &str) -> (App, egui::Context) {
    let dir = std::env::temp_dir().join(format!("shot-plage-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for i in 0..3 {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(10);
        let mut l = Layer::new("A", LayerKind::Vector);
        for f in 0..10 {
            let c = doc.new_cel(LayerKind::Vector);
            l.set(f, Some(c));
        }
        doc.panels[i].layers.push(l);
    }
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.set_espace(Espace::Montage);
    app.sound_on = false;
    (app, ctx)
}

/// Fait passer `secondes` d'horloge, une image d'écran tous les 1/60 s (24 images par seconde pour le film).
fn avancer(app: &mut App, ctx: &egui::Context, secondes: f64) {
    let pas = 1.0 / 60.0;
    let mut t = app.last_time.unwrap_or(100.0) + pas;
    for _ in 0..=(secondes / pas).round() as usize {
        let _ = ctx.run(RawInput { time: Some(t), ..Default::default() }, |c| app.tick_playback(c));
        t += pas;
    }
}

/// Lance la lecture du film et note chaque image du film montrée, pendant `secondes`.
fn lire(app: &mut App, ctx: &egui::Context, secondes: f64) -> Vec<usize> {
    app.toggle_play_scope(true);
    let mut vues = vec![app.global_frame()];
    let pas = 1.0 / 60.0;
    let mut t = 100.0;
    for _ in 0..=(secondes / pas).round() as usize {
        let _ = ctx.run(RawInput { time: Some(t), ..Default::default() }, |c| app.tick_playback(c));
        t += pas;
        if vues.last() != Some(&app.global_frame()) {
            vues.push(app.global_frame());
        }
    }
    vues
}

#[test]
fn playing_the_film_starts_at_the_range_and_stops_at_its_end() {
    let (mut app, ctx) = fixture("une-fois");
    app.looping = false;
    // De l'image 5 du plan 1 à l'image 2 du plan 3 (5 à 22 du film).
    app.goto_global(5);
    app.run_action("plage.debut");
    app.goto_global(22);
    app.run_action("plage.fin");
    assert_eq!(app.film_play_range(), (5, 23));
    // La tête est hors de la plage (au début du film) : la lecture part du début de la plage.
    app.goto_global(0);
    let vues = lire(&mut app, &ctx, 2.0);
    assert_eq!(vues.first(), Some(&5));
    assert_eq!(vues, (5..23).collect::<Vec<_>>(), "chaque image de la plage, à travers les plans, une seule fois");
    assert!(!app.playing, "arrêtée à la fin de la plage");
    assert_eq!((app.cur_panel, app.cur_frame), (2, 2));
}

#[test]
fn a_looping_play_stays_inside_the_range() {
    let (mut app, ctx) = fixture("boucle");
    app.looping = true;
    app.goto_global(8);
    app.run_action("plage.debut");
    app.goto_global(12);
    app.run_action("plage.fin");
    let vues = lire(&mut app, &ctx, 1.5);
    assert!(app.playing, "la boucle continue");
    assert!(vues.iter().all(|g| (8..13).contains(g)), "jamais hors de la plage : {vues:?}");
    assert!(vues.windows(2).any(|w| w == [12, 8]), "de la fin de la plage à son début : {vues:?}");
}

#[test]
fn playing_backward_stops_at_the_start_of_the_range() {
    let (mut app, ctx) = fixture("arriere");
    app.looping = false;
    app.goto_global(12);
    app.run_action("plage.debut");
    app.goto_global(25);
    app.run_action("montage.lecture_arriere");
    assert!(app.playing && app.play_speed < 0.0);
    avancer(&mut app, &ctx, 2.0);
    assert!(!app.playing, "arrêtée au début de la plage");
    assert_eq!(app.global_frame(), 12);
}

#[test]
fn the_chosen_plans_become_the_range() {
    let (mut app, _ctx) = fixture("choisis");
    app.select_panel(1);
    app.board_sel = vec![app.doc.panels[2].id];
    app.run_action("plage.plans_choisis");
    assert_eq!(app.film_play_range(), (10, 30));
    app.run_action("plage.enlever");
    assert_eq!(app.film_play_range(), (0, 30));
    assert!(!app.plage_film.posee());
}

#[test]
fn another_project_starts_without_a_range() {
    // Chaque projet compte ses plans depuis le début : une plage gardée viserait les plans de même numéro du nouveau projet.
    let (mut app, _ctx) = fixture("autre-projet");
    app.goto_global(12);
    app.run_action("plage.debut");
    assert!(app.plage_film.posee());
    app.load_doc(film());
    assert!(!app.plage_film.posee());
    assert_eq!(app.film_play_range(), (0, 60));
}

#[test]
fn an_end_set_before_the_start_takes_its_place() {
    let (mut app, _ctx) = fixture("croise");
    app.goto_global(15);
    app.run_action("plage.debut");
    app.goto_global(6);
    app.run_action("plage.fin");
    assert_eq!(app.film_play_range(), (0, 7), "le début, après la fin, est enlevé");
    assert!(app.status.contains("enlevé"), "{}", app.status);
}

#[test]
fn without_a_range_the_film_plays_as_before() {
    let (mut app, ctx) = fixture("sans-plage");
    app.looping = false;
    app.goto_global(0);
    let vues = lire(&mut app, &ctx, 2.0);
    assert_eq!(vues, (0..30).collect::<Vec<_>>());
    assert!(!app.playing);
    // La lecture d'un plan seul ignore la plage du film : elle reste entre ses propres points.
    app.goto_global(14);
    app.run_action("plage.debut");
    app.goto_global(3);
    app.toggle_play_scope(false);
    avancer(&mut app, &ctx, 1.0);
    assert_eq!(app.cur_panel, 0, "le plan seul");
}
