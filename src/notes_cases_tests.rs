//! Les notes des cases ne s'écrivent plus qu'au bouton (2026-10-22) : un appui sur la bande de note dessine, l'icône au bout de la bande l'ouvre.

use eframe::egui::{self, epaint::Shape, vec2, Event, Key, Modifiers, Rect};

use crate::perf_tests::Banc;
use crate::vignettes_tests::{clic_avec, encre, glisser, icone_note, ouvert, pas_d_annulation, quatre_plans, vague, vignette};

const ICONE: &str = egui_phosphor::regular::NOTE_PENCIL;

fn bande(b: &Banc, k: usize) -> Rect {
    b.app.vignettes.xf.rect(b.app.board_layout().cards[k].note)
}

fn icones_peintes(b: &Banc) -> usize {
    fn marche(s: &Shape, n: &mut usize) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| marche(s, n)),
            Shape::Text(t) if t.galley.job.text == ICONE => *n += 1,
            _ => {}
        }
    }
    let mut n = 0;
    for s in &b.formes {
        marche(&s.shape, &mut n);
    }
    n
}

fn banc() -> Banc {
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_formes = true;
    b
}

#[test]
fn a_press_drag_that_starts_on_the_note_strip_draws_in_the_case_and_never_opens_the_note() {
    let mut b = banc();
    let (strip, case) = (bande(&b, 3), vignette(&b, 3));
    assert!(strip.top() >= case.bottom(), "la bande est sous la case");
    let n = pas_d_annulation(&b);
    // Un trait qui part du bas de la case, dans la bande (loin de l'icône), et remonte dans la case.
    let depart = egui::pos2(strip.left() + 20.0, strip.center().y);
    glisser(&mut b, &vague(depart, egui::pos2(depart.x + 40.0, case.center().y), 12));
    assert!(b.app.vignettes.note.is_none(), "la bande n'ouvre plus la note");
    assert!(encre(&b.app.doc, 1), "le trait est dans la case du plan 2");
    assert_eq!(b.app.cur_panel, 1);
    assert_eq!(pas_d_annulation(&b), n + 1, "une étape : le trait");
    // Un simple clic dans la bande n'ouvre rien non plus (un point est un point de dessin).
    let milieu = strip.center() + vec2(-30.0, 0.0);
    clic_avec(&mut b, milieu, Modifiers::NONE);
    assert!(b.app.vignettes.note.is_none());
    // Même avec du texte dans la bande : le texte n'est pas un bouton.
    b.app.doc.panels[1].notes = "Contre-champ".into();
    b.chauffer(3);
    let (strip, case) = (bande(&b, 3), vignette(&b, 3));
    glisser(&mut b, &vague(egui::pos2(strip.left() + 20.0, strip.center().y), egui::pos2(strip.left() + 60.0, case.center().y), 8));
    assert!(b.app.vignettes.note.is_none(), "ni sur son texte");
}

#[test]
fn the_icon_opens_the_note_with_its_text_and_escape_or_a_click_elsewhere_closes_it() {
    let mut b = banc();
    b.app.doc.panels[1].notes = "Contre-champ".into();
    b.chauffer(3);
    let id = b.app.board_layout().cards[3].id;
    let icone = icone_note(&b, 3);
    assert!(icone.right() <= bande(&b, 3).right() && icone.center().x > bande(&b, 3).center().x, "à droite de la bande");
    clic_avec(&mut b, icone.center(), Modifiers::NONE);
    b.chauffer(3);
    let e = b.app.vignettes.note.as_ref().expect("la note est ouverte");
    assert_eq!((e.id, e.buffer.as_str()), (id, "Contre-champ"), "avec son texte");
    assert_eq!(b.app.cur_panel, 1, "la case est courante");
    // Le texte tapé s'ajoute à la fin.
    b.image(vec![Event::Text(" serré".into())], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.vignettes.note.as_ref().map(|e| e.buffer.as_str()), Some("Contre-champ serré"), "le curseur est au bout du texte");
    // Échap ferme sans rien changer.
    let esc = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![esc(true)], 1.0 / 60.0);
    b.image(vec![esc(false)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(b.app.vignettes.note.is_none(), "Échap ferme la note");
    assert_eq!(b.app.doc.panels[1].notes, "Contre-champ", "et abandonne la saisie");
    // Un clic ailleurs la pose.
    let p = icone_note(&b, 3).center();
    clic_avec(&mut b, p, Modifiers::NONE);
    b.chauffer(3);
    b.image(vec![Event::Text("!".into())], 1.0 / 60.0);
    b.chauffer(2);
    b.app.tool = crate::app::Tool::Select;
    let ailleurs = vignette(&b, 4).center();
    clic_avec(&mut b, ailleurs, Modifiers::NONE);
    assert!(b.app.vignettes.note.is_none());
    assert_eq!(b.app.doc.panels[1].notes, "Contre-champ!", "un clic ailleurs pose la note");
}

#[test]
fn the_icon_shows_only_while_the_pointer_is_over_the_case_or_its_strip_and_on_the_case_being_written() {
    let mut b = banc();
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(icones_peintes(&b), 0, "aucune icône quand rien n'est survolé : la planche reste nette");
    // Sur la case : une icône (celle de cette case seulement).
    let case = vignette(&b, 3);
    b.image(vec![Event::PointerMoved(case.center())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(icones_peintes(&b), 1, "sur la case");
    // Sur sa bande aussi.
    let bande_3 = bande(&b, 3);
    b.image(vec![Event::PointerMoved(bande_3.center())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(icones_peintes(&b), 1, "sur la bande");
    // Loin de toute case : plus rien.
    let loin = egui::pos2(b.app.vignettes.view.left() + 4.0, b.app.vignettes.view.bottom() - 4.0);
    b.image(vec![Event::PointerMoved(loin)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(icones_peintes(&b), 0, "pointeur loin des cases");
    // La case dont la note s'écrit la garde, pointeur ailleurs.
    let p = icone_note(&b, 3).center();
    clic_avec(&mut b, p, Modifiers::NONE);
    b.chauffer(3);
    assert!(b.app.vignettes.note.is_some());
    b.image(vec![Event::PointerMoved(loin)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(icones_peintes(&b), 1, "toujours sur la case qu'on écrit");
}

#[test]
fn the_keyboard_command_writes_the_note_of_the_current_case() {
    let mut b = banc();
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == "vignettes.note"));
    b.app.select_board(1, 0);
    b.app.run_action("vignettes.note");
    b.chauffer(3);
    let id = b.app.board_layout().cards[3].id;
    assert_eq!(b.app.vignettes.note.as_ref().map(|e| e.id), Some(id), "la note de la case courante (le plan 2)");
}
