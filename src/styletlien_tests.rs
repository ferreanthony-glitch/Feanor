//! Le stylet branché (`styletlien.rs`, Anthony, 2026-11-07) : la récriture des événements sans écran, puis le bouton et
//! la gomme dans l'application entière. (Le relevé de l'écoute est imposé par `stylet.force` : l'écoute elle-même est
//! commune à tout le processus et appartient à `stylet_tests.rs`.)

use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};

use crate::app::Tool;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::stylet::{Lecture, Releve};
use crate::styletlien::{recrire, Lien};

const DT: f64 = 1.0 / 60.0;

fn touche(phase: TouchPhase, p: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(0), id: TouchId(0), phase, pos: p, force: Some(0.5) }
}

fn bouton(p: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }
}

fn boutons(events: &[Event]) -> Vec<(PointerButton, bool)> {
    events.iter().filter_map(|e| match e { Event::PointerButton { button, pressed, .. } => Some((*button, *pressed)), _ => None }).collect()
}

fn touchers(events: &[Event]) -> usize {
    events.iter().filter(|e| matches!(e, Event::Touch { .. })).count()
}

// ------------------------------------------------------------------------------------------------ la récriture, sans écran

#[test]
fn the_pen_button_held_at_contact_turns_the_press_and_its_release_into_the_secondary_button() {
    let p = pos2(10.0, 10.0);
    let mut lien = Lien::default();
    // L'appui : l'événement tactile puis le bouton principal, comme egui-winit les donne.
    let mut e = vec![touche(TouchPhase::Start, p), Event::PointerMoved(p), bouton(p, PointerButton::Primary, true)];
    recrire(&mut e, &mut lien, true);
    assert_eq!(boutons(&e), [(PointerButton::Secondary, true)]);
    assert_eq!(touchers(&e), 0, "l'événement tactile du début s'en va avec l'appui");
    assert!(matches!(e[0], Event::PointerMoved(_)), "le mouvement reste, et le premier");
    assert!(lien.bouton_tenu);
    // Le mouvement : rien ne change.
    let mut e = vec![touche(TouchPhase::Move, p), Event::PointerMoved(p)];
    recrire(&mut e, &mut lien, false);
    assert_eq!(touchers(&e), 1);
    // Le relâchement : secondaire aussi, même si le bouton du stylet a été lâché entre-temps.
    let mut e = vec![touche(TouchPhase::End, p), bouton(p, PointerButton::Primary, false), Event::PointerGone];
    recrire(&mut e, &mut lien, false);
    assert_eq!(boutons(&e), [(PointerButton::Secondary, false)]);
    assert!(!lien.bouton_tenu);
}

#[test]
fn without_the_button_and_with_the_mouse_nothing_is_rewritten() {
    let p = pos2(10.0, 10.0);
    let mut lien = Lien::default();
    let mut e = vec![touche(TouchPhase::Start, p), Event::PointerMoved(p), bouton(p, PointerButton::Primary, true)];
    recrire(&mut e, &mut lien, false);
    assert_eq!(boutons(&e), [(PointerButton::Primary, true)]);
    assert_eq!(touchers(&e), 1);
    // Le bouton pressé en cours de trait ne change pas le relâchement.
    let mut e = vec![touche(TouchPhase::End, p), bouton(p, PointerButton::Primary, false)];
    recrire(&mut e, &mut lien, true);
    assert_eq!(boutons(&e), [(PointerButton::Primary, false)]);
    // La souris (pas d'événement tactile) : jamais touchée, même le bouton du stylet enfoncé.
    let mut e = vec![Event::PointerMoved(p), bouton(p, PointerButton::Primary, true), bouton(p, PointerButton::Primary, false)];
    recrire(&mut e, &mut lien, true);
    assert_eq!(boutons(&e), [(PointerButton::Primary, true), (PointerButton::Primary, false)]);
    assert_eq!(lien, Lien::default());
}

// ------------------------------------------------------------------------------------------------ dans l'application

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    // Comme la vraie fenêtre : les événements passent par `raw_input_hook`.
    b.crochet = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert!(b.app.settings.stylet_windows, "allumé d'origine");
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

fn releve(lecture: Lecture, bouton_au_contact: bool) -> Releve {
    Releve { lecture, bouton_au_contact, ..Default::default() }
}

fn pointe_posee(bouton: bool) -> Lecture {
    Lecture { portee: true, contact: true, bouton, appui: 0.5, ..Default::default() }
}

#[test]
fn the_pen_button_at_contact_on_the_drawing_starts_no_stroke_and_begins_the_ring_gesture() {
    let mut b = banc();
    let centre = b.app.canvas_rect.center();
    b.app.stylet.force = Some(releve(pointe_posee(true), true));
    b.image(vec![touche(TouchPhase::Start, centre), Event::PointerMoved(centre), bouton(centre, PointerButton::Primary, true)], DT);
    b.chauffer(1);
    assert!(b.app.stroke.is_none(), "la pointe posée avec le bouton ne trace rien");
    assert!(b.app.anneau.geste.is_some(), "c'est un appui du bouton secondaire sur le dessin : l'anneau commence son geste");
    // Un mouvement puis le relâchement : le geste se termine, toujours sans trait.
    for k in 1..=4 {
        let p = centre + vec2(20.0 * k as f32, 0.0);
        b.image(vec![touche(TouchPhase::Move, p), Event::PointerMoved(p)], DT);
    }
    let fin = centre + vec2(80.0, 0.0);
    b.image(vec![touche(TouchPhase::End, fin), bouton(fin, PointerButton::Primary, false), Event::PointerGone], DT);
    b.chauffer(2);
    assert!(b.app.stroke.is_none());
    assert!(b.app.anneau.geste.is_none());
    assert!(!b.app.stylet.lien.bouton_tenu);
    // Sans le bouton : un trait, comme toujours.
    b.app.stylet.force = Some(releve(pointe_posee(false), false));
    b.image(vec![touche(TouchPhase::Start, centre), Event::PointerMoved(centre), bouton(centre, PointerButton::Primary, true)], DT);
    assert!(b.app.stroke.is_some(), "sans le bouton, la pointe trace");
    b.image(vec![touche(TouchPhase::End, centre), bouton(centre, PointerButton::Primary, false), Event::PointerGone], DT);
    b.chauffer(2);
    assert!(b.app.stroke.is_none());
}

#[test]
fn the_inverted_pen_takes_the_eraser_and_gives_the_tool_back() {
    let mut b = banc();
    assert_eq!(b.app.tool, Tool::Brush);
    let en_l_air = |retourne: bool| Lecture { portee: true, retourne, ..Default::default() };
    b.app.stylet.force = Some(releve(en_l_air(true), false));
    b.image(Vec::new(), DT);
    assert_eq!(b.app.tool, Tool::Eraser);
    // Posée : toujours la gomme.
    b.app.stylet.force = Some(releve(Lecture { portee: true, contact: true, gomme: true, appui: 0.4, ..Default::default() }, false));
    b.image(Vec::new(), DT);
    assert_eq!(b.app.tool, Tool::Eraser);
    // Remis à l'endroit : le pinceau revient.
    b.app.stylet.force = Some(releve(en_l_air(false), false));
    b.image(Vec::new(), DT);
    assert_eq!(b.app.tool, Tool::Brush);
    // Un outil changé pendant que le stylet est retourné reste.
    b.app.stylet.force = Some(releve(en_l_air(true), false));
    b.image(Vec::new(), DT);
    assert_eq!(b.app.tool, Tool::Eraser);
    b.app.set_tool(Tool::Pencil);
    b.app.stylet.force = Some(releve(en_l_air(false), false));
    b.image(Vec::new(), DT);
    assert_eq!(b.app.tool, Tool::Pencil);
    // Déjà sur la gomme : retourner puis remettre n'y change rien.
    b.app.set_tool(Tool::Eraser);
    b.app.stylet.force = Some(releve(en_l_air(true), false));
    b.image(Vec::new(), DT);
    b.app.stylet.force = Some(releve(en_l_air(false), false));
    b.image(Vec::new(), DT);
    assert_eq!(b.app.tool, Tool::Eraser);
    // Le réglage éteint : plus rien, ni la gomme ni le bouton.
    b.app.set_tool(Tool::Brush);
    b.app.settings.stylet_windows = false;
    let centre = b.app.canvas_rect.center();
    b.app.stylet.force = Some(releve(Lecture { retourne: true, ..pointe_posee(true) }, true));
    b.image(vec![touche(TouchPhase::Start, centre), Event::PointerMoved(centre), bouton(centre, PointerButton::Primary, true)], DT);
    assert_eq!(b.app.tool, Tool::Brush);
    assert!(b.app.stroke.is_some(), "éteint : la pointe trace, bouton ou pas");
    b.image(vec![touche(TouchPhase::End, centre), bouton(centre, PointerButton::Primary, false), Event::PointerGone], DT);
    b.chauffer(2);
}
