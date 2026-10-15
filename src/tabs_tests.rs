//! Onglets du bas (13e) : arrière-plan (aucun, couleur, damier) et notes dessinées.

use crate::document::{BackgroundKind, Document, TimeNote};
use crate::project;

fn round_trip(doc: &Document, tag: &str) -> Document {
    let dir = std::env::temp_dir().join(format!("pelure-test-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    project::save(doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    back
}

#[test]
fn only_a_colour_background_is_rendered() {
    let mut doc = Document::new(16, 16, 24);
    doc.paper = [250, 240, 230];
    assert_eq!(doc.render_paper(), Some([250, 240, 230]));
    doc.background = BackgroundKind::Checker;
    assert_eq!(doc.render_paper(), None, "le damier n'est qu'un affichage : l'export est transparent");
    doc.background = BackgroundKind::None;
    assert_eq!(doc.render_paper(), None);
}

#[test]
fn background_is_saved() {
    let mut doc = Document::new(16, 16, 24);
    doc.background = BackgroundKind::Checker;
    doc.paper_b = [10, 20, 30];
    let back = round_trip(&doc, "fond");
    assert_eq!(back.background, BackgroundKind::Checker);
    assert_eq!(back.paper_b, [10, 20, 30]);
    let plain = round_trip(&Document::new(16, 16, 24), "fond2");
    assert_eq!(plain.background, BackgroundKind::Color, "un projet sans réglage garde sa couleur de papier");
}

#[test]
fn drawn_notes_are_saved_and_follow_the_images() {
    let mut doc = Document::new(16, 16, 24);
    let note = TimeNote { color: 1, width: 3.0, points: vec![[2.0, 0.5], [6.0, 0.25], [12.0, 0.75]] };
    doc.panels[0].cues.notes.push(note.clone());
    let back = round_trip(&doc, "notes");
    assert_eq!(back.panels[0].cues.notes, vec![note]);

    // Trois images insérées à l'image 5 : ce qui est après recule de trois.
    let p = &mut doc.panels[0];
    crate::images::shift_panel(p, 5, 3, false);
    assert_eq!(p.cues.notes[0].points, vec![[2.0, 0.5], [9.0, 0.25], [15.0, 0.75]]);
    // Les images 8 à 11 supprimées : ce qui y était se serre au bord.
    crate::images::shift_panel(p, 8, 4, true);
    assert_eq!(p.cues.notes[0].points, vec![[2.0, 0.5], [8.0, 0.25], [11.0, 0.75]]);
}

#[test]
fn position_and_cut_slide_notes() {
    use crate::notes::slide_notes;
    let mut notes = vec![
        TimeNote { color: 0, width: 2.0, points: vec![[1.0, 0.0], [3.0, 0.0]] },
        TimeNote { color: 2, width: 2.0, points: vec![[10.0, 0.0]] },
    ];
    slide_notes(&mut notes, 2.0, None);
    assert_eq!(notes[0].points[0][0], 3.0);
    assert_eq!(notes[1].points[0][0], 12.0);
    // Découper à l'image 4 : seul ce qui est à droite glisse.
    slide_notes(&mut notes, -5.0, Some(4.0));
    assert_eq!(notes[0].points, vec![[3.0, 0.0], [0.0, 0.0]], "le bout droit du trait suit");
    assert_eq!(notes[1].points[0][0], 7.0);
}
