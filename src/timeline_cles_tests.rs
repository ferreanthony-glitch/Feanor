//! Essais de **Ctrl+C / Ctrl+V** des clés de dessins (le montage) et des images (la timeline) (Anthony, 2026-10-28) : Ctrl+V insère, Maj+Ctrl+V pose
//! sur la tête de lecture sans rien repousser ; dans le plan et d'un plan à l'autre, avec de nouveaux dessins, en une étape d'annulation.

use crate::fiabilite_fixture::meme_projet;
use crate::montagekeys::KeyRef;
use crate::perf_tests::Banc;
use crate::timeline_alt_tests::{banc, film, images, photo, rouge};

// ---------------------------------------------------------------------------------------------- Ctrl+C / Ctrl+V des clés de dessins

/// Choisit la clé de dessin qui commence à l'image `start` du calque 0 du plan `plan`, dans le montage, et la copie.
fn copier_la_cle(b: &mut Banc, plan: usize, start: usize) {
    let (p, l) = (&b.app.doc.panels[plan], &b.app.doc.panels[plan].layers[0]);
    b.app.key_sel = vec![KeyRef { panel: p.id, layer: l.id, start }];
    (b.app.key_focus, b.app.images_focus, b.app.panel_focus) = (true, false, false);
    b.app.run_action("edition.copier");
    assert!(b.app.key_clipboard.is_some(), "{}", b.app.status);
}

#[test]
fn pasted_drawing_keys_insert_after_the_key_under_the_playhead_and_push_the_rest() {
    let (doc, cels) = film();
    let mut b = banc(doc);
    copier_la_cle(&mut b, 0, 0);
    b.app.cur_frame = 2;
    let (avant, n) = (photo(&b), b.app.history.undo_len());
    (b.app.key_focus, b.app.images_focus) = (true, false);
    b.app.run_action("edition.coller");
    let f = images(&b, 0);
    // Après la clé 2 (l'image 3) : la copie de la clé 0, puis la suite repoussée d'une image.
    assert_eq!(&f[..3], &[Some(cels[0][0]), Some(cels[0][1]), Some(cels[0][2])], "{f:?} : {}", b.app.status);
    let neuf = f[3].expect("la clé collée");
    assert!(!cels[0].contains(&neuf), "un dessin à elle");
    assert_eq!(rouge(&b, neuf), rouge(&b, cels[0][0]));
    assert_eq!(&f[4..6], &[Some(cels[0][3]), Some(cels[0][4])], "la suite est repoussée");
    assert_eq!(b.app.history.undo_len(), n + 1, "une étape d'annulation");
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend le plan");
}

#[test]
fn pasted_drawing_keys_can_go_exactly_on_the_playhead_without_pushing() {
    let (doc, cels) = film();
    let mut b = banc(doc);
    copier_la_cle(&mut b, 0, 0);
    b.app.cur_frame = 2;
    let (avant, n) = (photo(&b), b.app.history.undo_len());
    (b.app.key_focus, b.app.images_focus) = (true, false);
    b.app.run_action("edition.coller_a_la_tete");
    let f = images(&b, 0);
    assert_eq!((f[0], f[1]), (Some(cels[0][0]), Some(cels[0][1])));
    let neuf = f[2].expect("la clé collée à la tête");
    assert!(!cels[0].contains(&neuf));
    assert_eq!((f[3], f[4]), (Some(cels[0][3]), Some(cels[0][4])), "rien n'est repoussé : la clé recouvre celle de la tête");
    assert_eq!(b.app.history.undo_len(), n + 1);
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend le plan");
}

#[test]
fn drawing_keys_paste_into_the_current_layer_of_another_plan() {
    let (doc, cels) = film();
    let mut b = banc(doc);
    copier_la_cle(&mut b, 0, 3);
    // Le plan 1 : le calque courant, la tête sur son image 1.
    b.app.set_panel(1);
    (b.app.cur_layer, b.app.cur_frame) = (0, 1);
    (b.app.key_focus, b.app.images_focus) = (true, false);
    let (avant, n) = (photo(&b), b.app.history.undo_len());
    b.app.run_action("edition.coller");
    let f = images(&b, 1);
    assert_eq!(&f[..2], &[Some(cels[1][0]), Some(cels[1][1])]);
    let neuf = f[2].expect("collée après la clé 1");
    assert!(!cels[0].contains(&neuf) && !cels[1].contains(&neuf), "de nouveaux dessins, pas ceux du plan d'origine");
    assert_eq!(rouge(&b, neuf), rouge(&b, cels[0][3]), "la copie a les pixels de la clé copiée");
    assert_eq!(&f[3..6], &[Some(cels[1][2]), Some(cels[1][3]), Some(cels[1][4])]);
    assert_eq!(images(&b, 0), (0..5).map(|k| Some(cels[0][k])).collect::<Vec<_>>(), "le plan d'origine est intact");
    assert_eq!(b.app.history.undo_len(), n + 1);
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend les plans");
}

// ---------------------------------------------------------------------------------------------- Ctrl+C / Ctrl+V des images de la timeline

fn copier_les_images(b: &mut Banc, plan: usize, de: usize, a: usize) {
    b.app.set_panel(plan);
    b.app.cur_layer = 0;
    b.app.frame_sel = Some((de, a));
    (b.app.images_focus, b.app.key_focus, b.app.panel_focus) = (true, false, false);
    b.app.run_action("edition.copier");
    assert!(b.app.image_clipboard.is_some(), "{}", b.app.status);
}

#[test]
fn pasted_images_insert_before_the_playhead_and_push_the_rest() {
    let (doc, cels) = film();
    let mut b = banc(doc);
    copier_les_images(&mut b, 0, 0, 2);
    b.app.frame_sel = None;
    b.app.cur_frame = 3;
    let (avant, n) = (photo(&b), b.app.history.undo_len());
    b.app.run_action("edition.coller");
    let f = images(&b, 0);
    assert_eq!(&f[..3], &[Some(cels[0][0]), Some(cels[0][1]), Some(cels[0][2])]);
    assert!(f[3].is_some() && f[4].is_some() && !cels[0].contains(&f[3].unwrap()) && !cels[0].contains(&f[4].unwrap()), "de nouveaux dessins : {f:?}");
    assert_eq!((rouge(&b, f[3].unwrap()), rouge(&b, f[4].unwrap())), (rouge(&b, cels[0][0]), rouge(&b, cels[0][1])));
    assert_eq!((f[5], f[6]), (Some(cels[0][3]), Some(cels[0][4])), "la suite est repoussée de deux images");
    assert_eq!(b.app.history.undo_len(), n + 1);
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend le plan");
}

#[test]
fn pasted_images_can_go_exactly_on_the_playhead_without_pushing() {
    let (doc, cels) = film();
    let mut b = banc(doc);
    copier_les_images(&mut b, 0, 0, 2);
    b.app.frame_sel = None;
    b.app.cur_frame = 3;
    let (avant, n) = (photo(&b), b.app.history.undo_len());
    b.app.run_action("edition.coller_a_la_tete");
    let f = images(&b, 0);
    assert_eq!(&f[..3], &[Some(cels[0][0]), Some(cels[0][1]), Some(cels[0][2])]);
    assert!(f[3].is_some() && f[4].is_some() && !cels[0].contains(&f[3].unwrap()), "{f:?}");
    assert_eq!((rouge(&b, f[3].unwrap()), rouge(&b, f[4].unwrap())), (rouge(&b, cels[0][0]), rouge(&b, cels[0][1])));
    // La clé 4 (« cels[0][4] ») était à l'image 4 : la copie la recouvre, rien ne recule.
    assert_eq!(f.len(), 5, "le plan ne s'allonge pas, rien n'est repoussé : {f:?}");
    assert_eq!(b.app.history.undo_len(), n + 1);
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend le plan");
}

#[test]
fn copied_images_paste_into_the_current_layer_of_another_plan_both_ways() {
    for a_la_tete in [false, true] {
        let (doc, cels) = film();
        let mut b = banc(doc);
        copier_les_images(&mut b, 0, 1, 3);
        b.app.set_panel(1);
        (b.app.cur_layer, b.app.cur_frame) = (0, 3);
        b.app.frame_sel = None;
        b.app.images_focus = true;
        let (avant, n) = (photo(&b), b.app.history.undo_len());
        b.app.run_action(if a_la_tete { "edition.coller_a_la_tete" } else { "edition.coller" });
        let f = images(&b, 1);
        let (x, y) = (f[3].expect("collée"), f[4].expect("collée"));
        assert!(!cels[0].contains(&x) && !cels[1].contains(&x) && !cels[0].contains(&y) && !cels[1].contains(&y), "de nouveaux dessins");
        assert_eq!((rouge(&b, x), rouge(&b, y)), (rouge(&b, cels[0][1]), rouge(&b, cels[0][2])));
        if a_la_tete {
            assert_eq!(f.len(), 5, "sur la tête : rien n'est repoussé ({f:?})");
        } else {
            assert_eq!((f[5], f[6]), (Some(cels[1][3]), Some(cels[1][4])), "inséré : la suite est repoussée ({f:?})");
        }
        assert_eq!(images(&b, 0), (0..5).map(|k| Some(cels[0][k])).collect::<Vec<_>>(), "le plan d'origine est intact");
        assert_eq!(b.app.history.undo_len(), n + 1, "{a_la_tete}");
        b.app.run_action("edition.annuler");
        meme_projet(&avant, &photo(&b)).expect("annuler rend les plans");
    }
}
