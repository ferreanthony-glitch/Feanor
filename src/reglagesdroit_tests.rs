//! Clic droit dans le dessin : la fenêtre flottante des réglages de l'outil (Anthony, 2026-10-28 ; `reglagesdroit.rs`).
//! Sauf la Sélection et la Pipette, qui gardent le menu du dessin.

use eframe::egui::{epaint::Shape, pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2, Rect, TouchDeviceId, TouchId, TouchPhase};

use crate::app::Tool;
use crate::document::Document;
use crate::perf_tests::{banc_taille, jusqu_au_repos, Banc};

const DT: f64 = 1.0 / 60.0;

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert!(!b.app.board_shown(), "le dessin, pas la planche");
    b
}

fn ecran(b: &Banc) -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), b.taille)
}

fn clic_droit(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(4);
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(3);
}

fn echap(b: &mut Banc) {
    let touche = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![touche(true)], DT);
    b.image(vec![touche(false)], DT);
    b.chauffer(3);
}

/// Un trait au milieu du dessin.
fn trait_au_milieu(b: &mut Banc) {
    let c = b.app.canvas_rect.center();
    let (a, z) = (c - vec2(60.0, 0.0), c + vec2(60.0, 10.0));
    b.image(vec![Event::PointerMoved(a)], DT);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], DT);
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 10.0))], DT);
    }
    b.image(vec![Event::PointerButton { pos: z, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerGone], DT);
    b.chauffer(3);
}

fn encre(b: &Banc) -> bool {
    let doc = &b.app.doc;
    doc.panels.iter().flat_map(|p| p.layers.iter()).flat_map(|l| l.frames.iter().flatten()).any(|c| doc.cels.get(c).is_some_and(|c| !c.raster.is_blank() || c.vector.as_ref().is_some_and(|v| !v.elements.is_empty())))
}

fn textes_peints(shape: &Shape, sortie: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => sortie.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| textes_peints(s, sortie)),
        _ => {}
    }
}

/// Les textes de la dernière image avec leur place.
fn textes(b: &Banc) -> Vec<(String, Rect)> {
    let mut t = Vec::new();
    for s in &b.formes {
        textes_peints(&s.shape, &mut t);
    }
    t
}

fn fenetre(b: &Banc) -> Option<Rect> {
    b.app.reglages_droit.and_then(|e| e.rect)
}

/// Les textes peints dans la fenêtre (leur centre y est).
fn textes_de_la_fenetre(b: &Banc) -> Vec<String> {
    let Some(r) = fenetre(b) else { return Vec::new() };
    textes(b).into_iter().filter(|(_, p)| r.contains(p.center())).map(|(t, _)| t).collect()
}

fn dans_le_dessin(b: &Banc, dx: f32, dy: f32) -> Pos2 {
    b.app.canvas_rect.min + vec2(dx, dy)
}

#[test]
fn a_right_click_in_the_drawing_opens_the_settings_of_the_tool_under_the_cursor() {
    let mut b = banc();
    let avant = b.app.history.undo_len();
    let p = dans_le_dessin(&b, 40.0, 40.0);
    clic_droit(&mut b, p);
    let r = fenetre(&b).expect("la fenêtre des réglages est ouverte");
    // Sous le curseur : son coin haut gauche y est ; entièrement à l'écran ; la largeur du panneau Réglages (350, moins ses marges).
    assert!((r.min - p).length() < 2.0, "sous le curseur : {r:?} / {p:?}");
    assert!(ecran(&b).contains_rect(r), "tout à l'écran : {r:?}");
    assert!(r.width() > 250.0 && r.width() < 380.0, "la largeur du panneau : {r:?}");
    // Les réglages du pinceau, ceux du panneau : le titre de l'outil, la taille, l'opacité, « Plus de réglages ».
    let t = textes_de_la_fenetre(&b);
    for attendu in ["Pinceau", "Taille", "Opacité", "Plus de réglages"] {
        assert!(t.iter().any(|x| x.contains(attendu)), "« {attendu} » dans la fenêtre : {t:?}");
    }
    // Ouvrir ne trace rien et n'est pas une étape d'annulation.
    assert!(b.app.stroke.is_none() && !encre(&b));
    assert_eq!(b.app.history.undo_len(), avant);
}

#[test]
fn the_window_stays_whole_on_the_screen_near_its_edges() {
    let mut b = banc();
    let c = b.app.canvas_rect;
    // Les coins du dessin.
    for p in [c.max - vec2(5.0, 5.0), pos2(c.max.x - 5.0, c.min.y + 5.0), pos2(c.min.x + 5.0, c.max.y - 5.0)] {
        clic_droit(&mut b, p);
        let r = fenetre(&b).unwrap_or_else(|| panic!("ouverte en {p:?}"));
        assert!(ecran(&b).contains_rect(r), "tout à l'écran pour un clic en {p:?} : {r:?}");
        echap(&mut b);
        assert!(fenetre(&b).is_none());
    }
    // Les coins de l'écran, même hors du dessin : la fenêtre est ramenée à l'écran, et défile si l'écran est trop bas.
    for p in [pos2(1599.0, 899.0), pos2(1599.0, 0.0), pos2(0.0, 899.0)] {
        b.app.reglages_droit = Some(crate::reglagesdroit::ReglagesDroit { pos: p, rect: None });
        b.chauffer(4);
        let r = fenetre(&b).unwrap_or_else(|| panic!("ouverte en {p:?}"));
        assert!(ecran(&b).contains_rect(r), "tout à l'écran pour un point en {p:?} : {r:?}");
        echap(&mut b);
    }
    // Un écran très bas : la fenêtre ne dépasse pas, elle défile.
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1000, 320);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    b.app.reglages_droit = Some(crate::reglagesdroit::ReglagesDroit { pos: pos2(300.0, 200.0), rect: None });
    b.chauffer(5);
    let r = fenetre(&b).expect("ouverte");
    assert!(Rect::from_min_size(pos2(0.0, 0.0), b.taille).contains_rect(r), "dans l'écran bas : {r:?}");
}

#[test]
fn every_tool_opens_the_window_except_selection_and_the_picker_which_keep_their_menu() {
    let mut b = banc();
    let p = dans_le_dessin(&b, 40.0, 40.0);
    for tool in Tool::ALL {
        b.app.set_tool(tool);
        b.chauffer(3);
        clic_droit(&mut b, p);
        let exception = matches!(tool, Tool::Select | Tool::SelectColor | Tool::Picker);
        assert_eq!(b.app.reglages_droit.is_some(), !exception, "{tool:?}");
        if exception {
            // Le menu du dessin d'avant (Annuler, Couper, Copier, Coller…) s'est ouvert.
            let t: Vec<String> = textes(&b).into_iter().map(|(t, _)| t).collect();
            assert!(t.iter().any(|x| x.contains("Supprimer la sélection")) && t.iter().any(|x| x.contains("Ajuster à la fenêtre")), "{tool:?} : le menu du dessin : {t:?}");
        } else {
            let t = textes_de_la_fenetre(&b);
            assert!(t.iter().any(|x| x.contains(tool.label())), "{tool:?} : le titre de ses réglages dans la fenêtre : {t:?}");
            let r = fenetre(&b).unwrap();
            assert!(ecran(&b).contains_rect(r), "{tool:?} : {r:?}");
        }
        assert!(b.app.stroke.is_none());
        // (Échap ferme la fenêtre, ou le menu.)
        echap(&mut b);
        b.image(vec![Event::PointerGone], DT);
        b.chauffer(2);
        assert!(b.app.reglages_droit.is_none(), "{tool:?}");
    }
}

#[test]
fn a_size_change_in_the_window_is_the_one_of_the_panel() {
    let mut b = banc();
    let p = dans_le_dessin(&b, 40.0, 40.0);
    clic_droit(&mut b, p);
    let r = fenetre(&b).expect("ouverte");
    let avant = b.app.brush.size;
    // La valeur de la taille (« 12.0 px ») : le premier texte « px » de la fenêtre, tout en haut.
    let (_, valeur) = textes(&b).into_iter().filter(|(t, pos)| t.ends_with(" px") && r.contains(pos.center())).min_by(|a, c| a.1.top().total_cmp(&c.1.top())).expect("la taille en pixels dans la fenêtre");
    let a = valeur.center();
    b.image(vec![Event::PointerMoved(a)], DT);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], DT);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(a + vec2(6.0 * k as f32, 0.0))], DT);
    }
    b.image(vec![Event::PointerButton { pos: a + vec2(48.0, 0.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(4);
    assert!(b.app.brush.size > avant + 1.0, "la taille du pinceau a changé : {avant} -> {}", b.app.brush.size);
    assert!(b.app.reglages_droit.is_some(), "régler dans la fenêtre ne la ferme pas");
    assert!(!encre(&b), "et ne trace rien");
    // Le panneau Réglages (à droite, hors de la fenêtre) montre la même valeur : le même réglage.
    let r = fenetre(&b).expect("ouverte");
    let dans: Vec<String> = textes(&b).into_iter().filter(|(t, pos)| t.ends_with(" px") && r.contains(pos.center())).map(|(t, _)| t).collect();
    let hors: Vec<String> = textes(&b).into_iter().filter(|(t, pos)| t.ends_with(" px") && !r.contains(pos.center())).map(|(t, _)| t).collect();
    assert!(!dans.is_empty() && dans.iter().all(|t| hors.contains(t)), "la fenêtre {dans:?} et le panneau {hors:?} disent la même taille");
    // Et dans l'autre sens : un changement fait ailleurs (le raccourci) se voit dans la fenêtre.
    b.app.brush.size = 33.0;
    b.chauffer(3);
    let dans = textes_de_la_fenetre(&b);
    assert!(dans.iter().any(|t| t.starts_with("33")), "{dans:?}");
}

#[test]
fn a_click_outside_closes_the_window_and_does_nothing_else() {
    let mut b = banc();
    let avant = b.app.history.undo_len();
    let p = dans_le_dessin(&b, 40.0, 40.0);
    clic_droit(&mut b, p);
    let r = fenetre(&b).expect("ouverte");
    // Un clic dans la fenêtre (sur son titre) ne la ferme pas et ne trace rien dans le dessin dessous.
    let titre = textes(&b).into_iter().find(|(t, pos)| t.contains("Pinceau") && r.contains(pos.center())).map(|(_, pos)| pos.center()).expect("le titre");
    clic(&mut b, titre);
    assert!(b.app.reglages_droit.is_some(), "un clic dedans ne la ferme pas");
    assert!(!encre(&b) && b.app.history.undo_len() == avant);
    // Un clic dans le dessin, dehors : fermée, et pas de trait, pas d'étape d'annulation.
    let dehors = dans_le_dessin(&b, 600.0, 500.0);
    assert!(!r.contains(dehors));
    clic(&mut b, dehors);
    assert!(b.app.reglages_droit.is_none(), "un clic dehors la ferme");
    assert!(b.app.stroke.is_none());
    assert!(!encre(&b), "ce clic n'a rien tracé");
    assert_eq!(b.app.history.undo_len(), avant, "ni étape d'annulation");
    // Un geste glissé qui part de dehors : rien non plus (le trait ne part pas).
    clic_droit(&mut b, p);
    assert!(b.app.reglages_droit.is_some());
    let a = dans_le_dessin(&b, 500.0, 400.0);
    b.image(vec![Event::PointerMoved(a)], DT);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], DT);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(a + vec2(10.0 * k as f32, 3.0 * k as f32))], DT);
    }
    b.image(vec![Event::PointerButton { pos: a + vec2(60.0, 18.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(3);
    assert!(b.app.reglages_droit.is_none());
    assert!(!encre(&b) && b.app.history.undo_len() == avant, "le glissé qui ferme ne trace pas");
    // Après, le dessin répond de nouveau : un trait.
    trait_au_milieu(&mut b);
    assert!(encre(&b), "le pinceau trace de nouveau");
    assert_eq!(b.app.history.undo_len(), avant + 1);
}

#[test]
fn a_click_on_a_panel_or_a_tool_button_only_closes_the_window() {
    let mut b = banc();
    let outil = b.app.tool;
    let avant = b.app.history.undo_len();
    let p = dans_le_dessin(&b, 40.0, 40.0);
    // La barre d'outils à gauche : un clic sur chacun de ses boutons, fenêtre ouverte, ne choisit aucun outil.
    for y in (60..700).step_by(30) {
        clic_droit(&mut b, p);
        assert!(b.app.reglages_droit.is_some(), "ouverte avant le clic en y = {y}");
        clic(&mut b, pos2(20.0, y as f32));
        assert!(b.app.reglages_droit.is_none(), "fermée par le clic en y = {y}");
        assert_eq!(b.app.tool, outil, "ce clic n'a pas changé d'outil (y = {y})");
    }
    // Un clic sur le menu du haut : fermée, aucun menu ouvert ni commande lancée.
    clic_droit(&mut b, p);
    clic(&mut b, pos2(30.0, 10.0));
    assert!(b.app.reglages_droit.is_none());
    assert_eq!(b.app.history.undo_len(), avant);
}

#[test]
fn escape_a_second_right_click_and_a_pen_tap_close_the_window_too() {
    let mut b = banc();
    let p = dans_le_dessin(&b, 40.0, 40.0);
    // Échap.
    clic_droit(&mut b, p);
    assert!(b.app.reglages_droit.is_some());
    echap(&mut b);
    assert!(b.app.reglages_droit.is_none(), "Échap la ferme");
    assert_eq!(b.app.tool, Tool::Brush);
    // Un nouveau clic droit (dans le dessin, hors de la fenêtre) la ferme, et n'en ouvre pas une autre.
    clic_droit(&mut b, p);
    assert!(b.app.reglages_droit.is_some());
    { let p = dans_le_dessin(&b, 700.0, 450.0); clic_droit(&mut b, p); }
    assert!(b.app.reglages_droit.is_none(), "un clic droit dehors la ferme");
    // La pointe du stylet : un événement tactile, puis la souris que le système y ajoute.
    clic_droit(&mut b, p);
    assert!(b.app.reglages_droit.is_some());
    let a = dans_le_dessin(&b, 700.0, 450.0);
    let touche = Event::Touch { device_id: TouchDeviceId(0), id: TouchId(1), phase: TouchPhase::Start, pos: a, force: Some(0.6) };
    b.image(vec![Event::PointerMoved(a), touche], DT);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.chauffer(3);
    assert!(b.app.reglages_droit.is_none(), "l'appui du stylet la ferme");
    assert!(!encre(&b), "sans tracer");
}

#[test]
fn switching_to_selection_or_the_picker_closes_the_window() {
    let mut b = banc();
    { let p = dans_le_dessin(&b, 40.0, 40.0); clic_droit(&mut b, p); }
    assert!(b.app.reglages_droit.is_some());
    b.app.set_tool(Tool::Select);
    b.chauffer(3);
    assert!(b.app.reglages_droit.is_none());
}

#[test]
fn selection_and_the_picker_keep_the_drawing_menu() {
    let mut b = banc();
    for tool in [Tool::Select, Tool::SelectColor, Tool::Picker] {
        b.app.set_tool(tool);
        b.chauffer(3);
        { let p = dans_le_dessin(&b, 40.0, 40.0); clic_droit(&mut b, p); }
        assert!(b.app.reglages_droit.is_none(), "{tool:?}");
        let t: Vec<String> = textes(&b).into_iter().map(|(t, _)| t).collect();
        for attendu in ["Annuler", "Copier", "Coller", "Supprimer la sélection", "Ajuster à la fenêtre", "Animer ce plan"] {
            assert!(t.iter().any(|x| x.contains(attendu)), "{tool:?} : « {attendu} » dans le menu du dessin : {t:?}");
        }
        echap(&mut b);
    }
}

#[test]
fn the_window_does_not_ask_for_continuous_repaints() {
    let mut b = banc();
    { let p = dans_le_dessin(&b, 40.0, 40.0); clic_droit(&mut b, p); }
    assert!(b.app.reglages_droit.is_some());
    let (images, _) = jusqu_au_repos(&mut b, 80);
    assert!(images < 80, "au repos, la fenêtre ouverte ne demande aucune repeinture continue ({images} images)");
    let m = b.image(Vec::new(), 0.5);
    assert!(m.reveil >= std::time::Duration::from_millis(400), "réveil : {:?}{}", m.reveil, b.raison_du_reveil());
}

#[test]
fn opening_the_window_recomputes_nothing_heavy() {
    let mut b = banc();
    // Un trait, puis le repos : le dessin est à jour.
    trait_au_milieu(&mut b);
    jusqu_au_repos(&mut b, 80);
    let (cels, rejeu) = (b.app.doc.cels.len(), b.app.rejeu.has_pending());
    { let p = dans_le_dessin(&b, 40.0, 40.0); clic_droit(&mut b, p); }
    assert!(b.app.reglages_droit.is_some());
    assert_eq!(b.app.doc.cels.len(), cels, "aucun dessin de plus");
    assert_eq!(b.app.rejeu.has_pending(), rejeu, "aucun rejeu demandé par l'ouverture");
}

#[test]
fn the_vignettes_card_keeps_its_menu_which_offers_the_settings() {
    use crate::vignettes_tests::{ouvert, quatre_plans, vignette};
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_formes = true;
    b.app.set_tool(Tool::Brush);
    b.chauffer(3);
    let v = vignette(&b, 4).center();
    clic_droit(&mut b, v);
    assert!(b.app.reglages_droit.is_none(), "sur une case, le clic droit garde son menu");
    let t = textes(&b);
    assert!(t.iter().any(|(x, _)| x.contains("Animer cette image")), "le menu de la case : {:?}", t.iter().map(|(x, _)| x.as_str()).collect::<Vec<_>>());
    let (_, entree) = t.into_iter().find(|(x, _)| x.contains("Réglages de l'outil")).expect("l'entrée « Réglages de l'outil… »");
    clic(&mut b, entree.center());
    assert!(b.app.reglages_droit.is_some(), "l'entrée ouvre la fenêtre");
    let r = fenetre(&b).unwrap();
    assert!(ecran(&b).contains_rect(r));
    // Un clic sur une case, dehors : fermée, la case n'est pas dessinée.
    let avant = b.app.history.undo_len();
    let dehors = vignette(&b, 0).center();
    assert!(!r.contains(dehors), "{r:?} / {dehors:?}");
    clic(&mut b, dehors);
    assert!(b.app.reglages_droit.is_none());
    assert_eq!(b.app.history.undo_len(), avant, "ni trait ni étape d'annulation");
    // Avec la Sélection, la case garde son menu seul, sans l'entrée.
    b.app.set_tool(Tool::Select);
    b.chauffer(3);
    clic_droit(&mut b, v);
    assert!(textes(&b).iter().all(|(x, _)| !x.contains("Réglages de l'outil")));
}
