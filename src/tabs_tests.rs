//! Onglets du bas et vue Projet (13e) : arrière-plan (aucun, couleur, damier), notes dessinées,
//! pistes son, propriétés du projet, storyboard verrouillé, plan avant, projet fait de plans.

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
    let back = round_trip(&doc, "arriere-plan");
    assert_eq!(back.background, BackgroundKind::Checker);
    assert_eq!(back.paper_b, [10, 20, 30]);
    let plain = round_trip(&Document::new(16, 16, 24), "arriere-plan-2");
    assert_eq!(plain.background, BackgroundKind::Color, "un projet sans réglage garde sa couleur de papier");
}

#[test]
fn drawn_notes_are_saved_and_follow_the_images() {
    let mut doc = Document::new(16, 16, 24);
    let note = TimeNote { color: 1, width: 3.0, points: vec![[2.0, 0.5], [6.0, 0.25], [12.0, 0.75]] };
    doc.panels[0].cues.notes.push(note.clone());
    let back = round_trip(&doc, "notes-dessinees");
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
fn sound_tracks_lock_duplicate_and_master_volume() {
    use crate::document::SoundClip;
    let mut doc = Document::new(16, 16, 24);
    doc.add_audio_track();
    doc.audio_tracks[0].clips.push(SoundClip { id: 900, sound: 1, start: 4, offset: 0, length: 10 });
    assert!(!doc.clip_locked(900));
    doc.audio_tracks[0].locked = true;
    assert!(doc.clip_locked(900));
    let at = doc.duplicate_audio_track(0).unwrap();
    assert_eq!(at, 1);
    let copy = &doc.audio_tracks[1];
    assert!(!copy.locked, "la copie n'est pas verrouillée");
    assert_ne!(copy.clips[0].id, 900, "ses extraits ont leur propre numéro");
    assert_eq!((copy.clips[0].start, copy.clips[0].length), (4, 10));

    doc.master_volume = 0.5;
    doc.audio_tracks.truncate(1);
    doc.audio_tracks[0].clips.clear();
    let back = round_trip(&doc, "pistes-son");
    assert_eq!(back.master_volume, 0.5);
    assert!(back.audio_tracks[0].locked);
}

#[test]
fn properties_and_storyboard_lock_are_saved() {
    let mut doc = Document::new(16, 16, 24).created_now();
    assert!(doc.properties.created > 1_700_000_000, "date de création posée");
    doc.properties.title = "Brawlidays".into();
    doc.properties.info = "Animatique\nversion 2".into();
    doc.properties.author = "A. F.".into();
    doc.properties.modified = 1_760_000_000;
    doc.properties.notes = "À revoir".into();
    doc.properties.copyright = "©".into();
    doc.storyboard_locked = true;
    let back = round_trip(&doc, "proprietes-projet");
    assert_eq!(back.properties, doc.properties);
    assert!(back.storyboard_locked);
    let plain = round_trip(&Document::new(16, 16, 24), "proprietes-projet-2");
    assert_eq!(plain.properties, Default::default());
    assert!(!plain.storyboard_locked);
}

#[test]
fn a_plan_is_added_before_the_current_one() {
    let mut doc = Document::new(16, 16, 24);
    doc.add_sequence_after(0);
    // Plan 1 = premier plan de la 2e séquence : le nouveau plan se met devant lui, dans sa séquence.
    let (old, seq) = (doc.panels[1].id, doc.sequence_of(1));
    let at = doc.add_scene_before(1);
    assert_eq!(at, 1);
    assert_eq!(doc.panels.len(), 3);
    assert_eq!(doc.panels[2].id, old);
    assert_eq!(doc.sequence_of(1), seq, "dans la séquence du plan courant");
    assert_ne!(doc.panels[1].scene, doc.panels[2].scene, "un plan à lui");
    assert!(doc.is_plans());
}

#[test]
fn a_project_is_made_of_the_chosen_plans() {
    use crate::board::NoteText;
    use crate::document::{Layer, LayerKind};
    use eframe::egui::pos2;
    let mut doc = Document::with_frame(16, 12, 25);
    doc.add_scene_after(0);
    doc.add_sequence_after(1);
    doc.paper = [1, 2, 3];
    doc.start_frame = 101;
    doc.storyboard_locked = true;
    doc.properties.author = "A. F.".into();
    doc.properties.title = "Source".into();
    doc.add_audio_track();
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new("Trait", LayerKind::Bitmap);
    l.set(0, Some(cel));
    doc.panels[2].layers.push(l);
    let (a, c) = (doc.panels[0].id, doc.panels[2].id);
    let note = |anchor| NoteText { pos: pos2(0.0, 0.0), text: "n".into(), size: 16.0, color: [0, 0, 0], anchor };
    doc.board.texts = vec![note(Some(a)), note(Some(doc.panels[1].id)), note(None)];

    let new = doc.from_plans(&[c, a]).expect("deux plans");
    assert_eq!(new.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![a, c], "dans l'ordre du film");
    assert_eq!(new.sequences.len(), 2, "chaque plan garde sa séquence");
    assert_eq!((new.width, new.height, new.frame, new.fps), (doc.width, doc.height, doc.frame, 25));
    assert_eq!((new.paper, new.start_frame), ([1, 2, 3], 101));
    assert!(new.cels.contains_key(&cel), "les dessins suivent");
    assert!(new.audio_tracks.is_empty(), "le son n'est pas repris");
    assert_eq!(new.board.texts.len(), 1, "seules les notes attachées aux plans repris");
    assert!(!new.storyboard_locked);
    assert_eq!(new.properties.author, "A. F.");
    assert_eq!(new.properties.title, "");
    assert!(doc.from_plans(&[]).is_none());
}

#[test]
fn dates_are_shown_in_the_gregorian_calendar() {
    use crate::projetui::{civil_from_days, date_label};
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    assert_eq!(civil_from_days(20_741), (2026, 10, 15));
    assert_eq!(date_label(0), "inconnue");
    assert_eq!(date_label(20_741 * 86_400 + 14 * 3600 + 5 * 60 + 9), "15/10/2026, 14:05 UTC");
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
