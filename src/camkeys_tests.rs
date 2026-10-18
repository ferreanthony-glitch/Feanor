//! Clés de caméra dans le montage et la timeline : le choix, le déplacement, le copier-coller (avec
//! l'écart entre les clés, d'un plan à l'autre), la suppression, l'annulation.

use crate::camera::{Camera, CameraKey};
use crate::camkeys::{between, choose, clamp_delta, copy, delete, exists, global_of, live, paste, shift, CamClip, CamKeyRef, Group, Pasted};
use crate::document::Document;
use crate::history::{Edit, History, Nav};

fn cam(x: f32) -> Camera {
    Camera { x, y: 50.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 }
}

/// Trois plans de 24 images.
fn doc3() -> Document {
    let mut d = Document::new(200, 100, 24);
    d.add_panel_after(0);
    d.add_panel_after(1);
    for p in &mut d.panels {
        p.set_length(24);
    }
    assert_eq!(d.panels.len(), 3);
    d
}

/// Donne au plan `pi` des clés aux images `frames` (la caméra de chaque clé garde son image dans son x).
fn keys(doc: &mut Document, pi: usize, frames: &[usize]) {
    doc.panels[pi].camera = frames.iter().map(|&f| CameraKey::new(f, cam(f as f32))).collect();
}

fn at(doc: &Document, pi: usize, frame: usize) -> CamKeyRef {
    CamKeyRef { panel: doc.panels[pi].id, frame }
}

fn frames(doc: &Document, pi: usize) -> Vec<usize> {
    doc.panels[pi].camera.iter().map(|k| k.frame).collect()
}

#[test]
fn a_click_chooses_one_key_ctrl_toggles_shift_takes_the_range() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[2, 6, 12]);
    let (a, b, c) = (at(&doc, 0, 2), at(&doc, 0, 6), at(&doc, 0, 12));
    // Un clic : elle seule, et elle devient l'ancre.
    let (chosen, anchor) = choose(&[], None, a, false, None);
    assert_eq!((chosen.clone(), anchor), (vec![a], Some(a)));
    let (chosen, anchor) = choose(&chosen, anchor, b, false, None);
    assert_eq!(chosen, vec![b], "un autre clic remplace le choix");
    // Ctrl : s'ajoute, puis se retire.
    let (chosen, anchor) = choose(&chosen, anchor, c, true, None);
    assert_eq!(chosen, vec![b, c]);
    let (chosen, _) = choose(&chosen, anchor, c, true, None);
    assert_eq!(chosen, vec![b], "Ctrl + clic sur une clé choisie la retire");
    // Maj : tout ce qui est entre l'ancre et la clé cliquée.
    let range = between(&doc, a, c);
    assert_eq!(range, vec![a, b, c]);
    let (chosen, anchor) = choose(&[b], Some(a), c, false, Some(range.clone()));
    assert_eq!((chosen, anchor), (range, Some(a)));
    // Maj sans ancre : un simple clic.
    let (chosen, _) = choose(&[], None, b, false, Some(vec![a, b]));
    assert_eq!(chosen, vec![b]);
}

#[test]
fn a_shift_range_crosses_plans_in_montage_order() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[20]);
    keys(&mut doc, 1, &[3, 10]);
    keys(&mut doc, 2, &[1]);
    let r = between(&doc, at(&doc, 2, 1), at(&doc, 0, 20));
    assert_eq!(r, vec![at(&doc, 0, 20), at(&doc, 1, 3), at(&doc, 1, 10), at(&doc, 2, 1)], "dans les deux sens, d'un plan à l'autre");
    assert_eq!(global_of(&doc, at(&doc, 1, 3)), Some(24 + 3));
    let r = between(&doc, at(&doc, 1, 3), at(&doc, 1, 10));
    assert_eq!(r.len(), 2, "dans un seul plan : les deux bouts compris");
}

#[test]
fn chosen_keys_that_are_gone_drop_out() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[2, 6]);
    let sel = vec![at(&doc, 0, 6), at(&doc, 0, 2), at(&doc, 0, 2), at(&doc, 0, 9), CamKeyRef { panel: 9999, frame: 2 }];
    assert_eq!(live(&doc, &sel), vec![at(&doc, 0, 2), at(&doc, 0, 6)], "sans doublon, dans l'ordre du montage, les clés qui n'existent plus en moins");
    assert!(exists(&doc, at(&doc, 0, 2)) && !exists(&doc, at(&doc, 0, 3)));
}

fn group(chosen: &[usize], all: &[usize], len: usize) -> Group {
    Group { chosen: chosen.to_vec(), all: all.to_vec(), len }
}

#[test]
fn a_single_key_still_moves_between_its_neighbours() {
    // Plan de 24 images, clés à 0, 10 et 20 (le comportement d'avant les choix multiples).
    let all = [0, 10, 20];
    assert_eq!(clamp_delta(&[group(&[0], &all, 24)], 50), 9, "la première s'arrête avant la suivante");
    assert_eq!(clamp_delta(&[group(&[10], &all, 24)], 4), 4);
    assert_eq!(clamp_delta(&[group(&[10], &all, 24)], 50), 9, "elle s'arrête avant la clé suivante");
    assert_eq!(clamp_delta(&[group(&[10], &all, 24)], -50), -9, "et après la précédente");
    assert_eq!(clamp_delta(&[group(&[20], &all, 24)], 50), 3, "la dernière va jusqu'à la fin du plan (image 23)");
    assert_eq!(clamp_delta(&[group(&[0], &all, 24)], -5), 0, "pas avant l'image 0");
    // Une seule clé : tout le plan.
    assert_eq!(clamp_delta(&[group(&[3], &[3], 24)], 50), 20);
    assert_eq!(clamp_delta(&[group(&[3], &[3], 24)], -50), -3);
    // Rien de choisi : rien ne bouge.
    assert_eq!(clamp_delta(&[], 5), 0);
    assert_eq!(clamp_delta(&[group(&[], &all, 24)], 5), 0);
}

#[test]
fn chosen_keys_move_together_without_passing_an_unchosen_one() {
    // Clés 2, 4, 8, 15 ; 4 et 8 choisies : elles avancent ensemble jusqu'à la 15 (non choisie) et reculent jusqu'à la 2.
    let all = [2, 4, 8, 15];
    let g = [group(&[4, 8], &all, 24)];
    assert_eq!(clamp_delta(&g, 99), 6, "8 + 6 = 14 : juste avant la 15");
    assert_eq!(clamp_delta(&g, -99), -1, "4 − 1 = 3 : juste après la 2");
    // Choisies de part et d'autre de clés non choisies : le plus serré décide.
    let g = [group(&[2, 15], &all, 24)];
    assert_eq!(clamp_delta(&g, 1), 1, "la 2 avance d'un cran (jusqu'à 3, avant la 4), la 15 aussi");
    assert_eq!(clamp_delta(&g, 5), 1, "la 2 bute sur la 4 non choisie");
    assert_eq!(clamp_delta(&g, -5), -2, "la 2 s'arrête à l'image 0");
    // Plusieurs plans : le plus serré décide.
    let g = [group(&[5], &[5], 24), group(&[3], &[3], 8)];
    assert_eq!(clamp_delta(&g, 50), 4, "le plan de 8 images s'arrête à l'image 7");
    assert_eq!(clamp_delta(&g, -50), -3);
}

#[test]
fn moved_keys_keep_their_cameras_and_the_order() {
    let mut ks: Vec<CameraKey> = [2usize, 4, 8, 15].iter().map(|&f| CameraKey::new(f, cam(f as f32 * 10.0))).collect();
    ks[1].ease_in = 3.0;
    let at = shift(&mut ks, &[4, 8], 5);
    assert_eq!(at, vec![9, 13]);
    assert_eq!(ks.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 9, 13, 15]);
    assert_eq!(ks[1].cam.x, 40.0, "la caméra suit sa clé");
    assert_eq!(ks[1].ease_in, 3.0, "et ses adoucis");
    assert_eq!(ks[0].cam.x, 20.0, "les clés non choisies ne bougent pas");
    // Vers l'arrière.
    let mut ks: Vec<CameraKey> = [4usize, 8].iter().map(|&f| CameraKey::new(f, cam(f as f32))).collect();
    shift(&mut ks, &[8], -3);
    assert_eq!(ks.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![4, 5]);
}

#[test]
fn copy_keeps_the_spacing_and_paste_lands_at_the_playhead() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[2, 5, 11]);
    doc.panels[0].camera[1].ease_out = 4.0;
    // Copie dans le désordre : l'écart se compte depuis la première clé.
    let sel = [at(&doc, 0, 11), at(&doc, 0, 2), at(&doc, 0, 5)];
    let clip = copy(&doc, &sel).expect("des clés choisies");
    assert_eq!(clip.keys.iter().map(|k| k.offset).collect::<Vec<_>>(), vec![0, 3, 9]);
    assert_eq!(clip.keys[1].key.ease_out, 4.0);
    // Collage dans le plan suivant, à l'image 4 : les écarts sont gardés, les caméras aussi.
    let at_g = 24 + 4;
    let done = paste(&mut doc, &clip, at_g, None);
    assert_eq!(done, Pasted { placed: vec![at(&doc, 1, 4), at(&doc, 1, 7), at(&doc, 1, 13)], skipped: 0 });
    assert_eq!(frames(&doc, 1), vec![0, 4, 7, 13], "le plan sans clé reçoit d'abord une clé à son début (sa caméra de continuité)");
    assert_eq!(doc.panels[1].camera[2].cam.x, 5.0, "la clé collée garde sa caméra");
    assert_eq!(doc.panels[1].camera[2].ease_out, 4.0, "et ses adoucis");
    assert_eq!(frames(&doc, 0), vec![2, 5, 11], "le plan d'origine ne change pas");
    // Coller de nouveau au même endroit remplace, sans doublon.
    let again = paste(&mut doc, &clip, at_g, None);
    assert_eq!(again.placed.len(), 3);
    assert_eq!(frames(&doc, 1), vec![0, 4, 7, 13]);
}

#[test]
fn a_paste_can_cross_plans_in_the_montage_but_not_in_animation() {
    let make = || {
        let mut d = doc3();
        keys(&mut d, 0, &[2, 20]);
        d
    };
    let doc = make();
    let clip = copy(&doc, &[at(&doc, 0, 2), at(&doc, 0, 20)]).unwrap();
    assert_eq!(clip.keys.iter().map(|k| k.offset).collect::<Vec<_>>(), vec![0, 18]);
    // Montage : posée à l'image 10 du plan 1, la seconde clé (écart 18) tombe dans le plan 2.
    let mut montage = make();
    let done = paste(&mut montage, &clip, 24 + 10, None);
    assert_eq!(done, Pasted { placed: vec![at(&montage, 1, 10), at(&montage, 2, 4)], skipped: 0 });
    assert_eq!(frames(&montage, 2), vec![0, 4], "le plan d'arrivée reçoit sa clé de début avant");
    // Animation (un seul plan permis) : la clé du plan suivant est laissée de côté, et comptée.
    let mut anim = make();
    let done = paste(&mut anim, &clip, 24 + 10, Some(1));
    assert_eq!(done, Pasted { placed: vec![at(&anim, 1, 10)], skipped: 1 });
    assert!(anim.panels[2].camera.is_empty());
    // Après la fin du dernier plan : laissées de côté.
    let mut end = make();
    let done = paste(&mut end, &clip, 3 * 24 - 1, None);
    assert_eq!((done.placed.len(), done.skipped), (1, 1));
    // Un presse-papiers vide ne fait rien.
    let mut same = make();
    assert_eq!(paste(&mut same, &CamClip { keys: vec![] }, 5, None), Pasted { placed: vec![], skipped: 0 });
    assert_eq!(frames(&same, 0), frames(&doc, 0));
}

#[test]
fn a_paste_at_the_start_of_an_empty_plan_adds_no_extra_key() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[0, 8]);
    let clip = copy(&doc, &[at(&doc, 0, 0), at(&doc, 0, 8)]).unwrap();
    paste(&mut doc, &clip, 24, None);
    assert_eq!(frames(&doc, 1), vec![0, 8]);
}

#[test]
fn copying_nothing_copies_nothing() {
    let doc = doc3();
    assert!(copy(&doc, &[]).is_none());
    assert!(copy(&doc, &[at(&doc, 0, 3)]).is_none(), "une clé qui n'existe pas n'est pas copiée");
}

#[test]
fn deleting_removes_only_the_chosen_keys_and_nothing_without_a_choice() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[2, 6, 12]);
    keys(&mut doc, 1, &[1]);
    let sel = [at(&doc, 0, 6), at(&doc, 1, 1), at(&doc, 0, 7)];
    let n = delete(&mut doc, &sel);
    assert_eq!(n, 2, "la clé qui n'existe pas ne compte pas");
    assert_eq!(frames(&doc, 0), vec![2, 12]);
    assert!(doc.panels[1].camera.is_empty());
    let before = doc.panels[0].camera.clone();
    assert_eq!(delete(&mut doc, &[]), 0);
    assert_eq!(doc.panels[0].camera, before, "rien de choisi : rien ne part");
}

#[test]
fn every_edit_is_one_named_undo_step_that_restores_the_keys() {
    let mut doc = doc3();
    keys(&mut doc, 0, &[2, 6, 12]);
    let original = doc.panels[0].camera.clone();
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut history = History::default();
    let step = |doc: &mut Document, history: &mut History, nav: Nav, name: &str, edit: &dyn Fn(&mut Document)| {
        let snap = Edit::Structure { structure: doc.structure(), nav };
        edit(doc);
        let before = history.context.replace(name.to_string());
        history.push(snap);
        history.context = before;
    };
    step(&mut doc, &mut history, nav, "Déplacer des clés de caméra", &|d| {
        let sel = [CamKeyRef { panel: d.panels[0].id, frame: 6 }, CamKeyRef { panel: d.panels[0].id, frame: 12 }];
        let mut ks = d.panels[0].camera.clone();
        shift(&mut ks, &sel.iter().map(|k| k.frame).collect::<Vec<_>>(), 3);
        d.panels[0].camera = ks;
    });
    step(&mut doc, &mut history, nav, "Supprimer des clés de caméra", &|d| {
        let id = d.panels[0].id;
        delete(d, &[CamKeyRef { panel: id, frame: 2 }]);
    });
    let clip = copy(&doc, &[at(&doc, 0, 9)]).unwrap();
    step(&mut doc, &mut history, nav, "Coller des clés de caméra", &|d| {
        paste(d, &clip, 30, None);
    });
    assert_eq!(history.names().0, vec!["Déplacer des clés de caméra", "Supprimer des clés de caméra", "Coller des clés de caméra"]);
    assert_eq!(frames(&doc, 0), vec![9, 15]);
    assert_eq!(frames(&doc, 1), vec![0, 6], "collé dans le plan 2, avec sa clé de début");
    // Annuler trois fois : les clés d'origine, caméra pour caméra.
    for _ in 0..3 {
        assert!(history.undo(&mut doc, &mut nav));
    }
    assert_eq!(doc.panels[0].camera, original);
    assert!(doc.panels[1].camera.is_empty());
    // Rétablir : le collage revient.
    for _ in 0..3 {
        assert!(history.redo(&mut doc, &mut nav));
    }
    assert_eq!(frames(&doc, 1), vec![0, 6]);
}
