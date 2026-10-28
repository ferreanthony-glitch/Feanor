//! Essais d'**Alt + glisser** des clés de dessins dans les pistes du montage (Anthony, 2026-10-28) : elles sont dupliquées à
//! l'endroit lâché, comme les images de la timeline (Ctrl fait de même) ; les copies ont leurs propres dessins ; une seule
//! étape d'annulation ; Échap abandonne. « Réordonner les clés » passe sur Ctrl + Maj ; Maj ne change pas.

use eframe::egui::{self, Modifiers};

use crate::app::App;
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::handles::{clamp_key_delta, key_move_for, key_preview, Handle, KeyMove};
use crate::montagekeys::{key_move_name, KeyRef};

const ALT: Modifiers = Modifiers { alt: true, ..Modifiers::NONE };
const CTRL: Modifiers = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
const MAJ: Modifiers = Modifiers { shift: true, ..Modifiers::NONE };
const CTRL_MAJ: Modifiers = Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE };

/// Un plan de 20 images, un calque « A » à trois dessins : images 0 à 4, 5 à 9, 10 à 19.
fn fixture(name: &str) -> (App, [CelId; 3], u64, u64) {
    let dir = std::env::temp_dir().join(format!("shot-montage-alt-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let mut l = Layer::new("A", LayerKind::Vector);
    let cels = [doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector)];
    for (c, (s, e)) in cels.iter().zip([(0, 5), (5, 10), (10, 20)]) {
        for f in s..e {
            l.set(f, Some(*c));
        }
    }
    let layer = l.id;
    doc.panels[0].layers.push(l);
    let panel = doc.panels[0].id;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    (app, cels, panel, layer)
}

fn frames(app: &App) -> Vec<Option<CelId>> {
    app.doc.panels[0].layers[0].frames.clone()
}

fn starts(f: &[Option<CelId>]) -> Vec<usize> {
    (0..f.len()).filter(|&i| f[i].is_some() && (i == 0 || f[i - 1] != f[i])).collect()
}

#[test]
fn the_keys_held_choose_the_key_gesture() {
    assert_eq!(key_move_for(&Modifiers::NONE), KeyMove::Ripple);
    assert_eq!(key_move_for(&ALT), KeyMove::Copy, "Alt duplique");
    assert_eq!(key_move_for(&CTRL), KeyMove::Copy, "Ctrl aussi, comme dans la timeline");
    assert_eq!(key_move_for(&MAJ), KeyMove::Single, "Maj : la clé seule, inchangé");
    assert_eq!(key_move_for(&CTRL_MAJ), KeyMove::Reorder, "Ctrl + Maj : réordonner");
    assert_eq!(key_move_for(&Modifiers { alt: true, shift: true, ..Modifiers::NONE }), KeyMove::Copy, "Alt + Maj : copie, comme la timeline");
    assert_eq!(key_move_name(KeyMove::Copy, false), "Dupliquer une clé");
    assert_eq!(key_move_name(KeyMove::Copy, true), "Dupliquer des clés");
}

#[test]
fn alt_drag_duplicates_the_chosen_keys_with_their_own_drawings_in_one_undo_step() {
    let (mut app, cels, panel, layer) = fixture("copie");
    let before = frames(&app);
    let n = app.history.undo_len();
    let key = |start| KeyRef { panel, layer, start };
    app.key_sel = vec![key(0), key(5)];
    assert!(app.begin_keys_drag(0));
    app.drag_keys_mode(10, KeyMove::Copy);
    app.end_keys_drag();
    let f = frames(&app);
    // Les originales ne bougent pas…
    assert_eq!(f[..10], before[..10], "Alt copie : rien ne quitte sa place");
    // …les copies sont à +10, avec la même durée (5 images chacune, la seconde coupée à 20), et de nouveaux dessins.
    let (c0, c5) = (f[10].expect("copie"), f[15].expect("copie"));
    assert!(f[10..15].iter().all(|x| *x == Some(c0)) && f[15..20].iter().all(|x| *x == Some(c5)), "{f:?}");
    assert!(!cels.contains(&c0) && !cels.contains(&c5) && c0 != c5, "des dessins à elles");
    assert!(app.doc.cels.contains_key(&c0) && app.doc.cels.contains_key(&c5));
    // Les copies deviennent le choix.
    assert_eq!(app.key_sel, vec![key(10), key(15)]);
    // Une seule étape d'annulation.
    assert_eq!(app.history.undo_len(), n + 1);
    app.run_action("edition.annuler");
    assert_eq!(frames(&app), before);
}

#[test]
fn a_copy_that_runs_past_the_end_of_the_plan_is_cut_and_never_starts_outside_it() {
    let (mut app, _, panel, layer) = fixture("fin");
    app.key_sel = vec![KeyRef { panel, layer, start: 10 }];
    assert!(app.begin_keys_drag(10));
    // Trop loin : ramené pour que la copie commence encore dans le plan (image 19).
    app.drag_keys_mode(99, KeyMove::Copy);
    app.end_keys_drag();
    let f = frames(&app);
    assert_eq!(f.len(), 20, "la durée du plan ne change pas");
    assert_eq!(starts(&f), vec![0, 5, 10, 19], "{f:?}");
    assert_eq!(clamp_key_delta(&app.doc.panels[0].layers[0], &[10], 99, KeyMove::Copy, 20), 9);
    assert_eq!(clamp_key_delta(&app.doc.panels[0].layers[0], &[5], -99, KeyMove::Copy, 20), -5);
    assert_eq!(key_preview(&app.doc.panels[0].layers[0], &[10], 5, KeyMove::Copy, 20), (vec![(15, 20)], vec![15]));
}

#[test]
fn escape_abandons_an_alt_drag_of_keys() {
    let (mut app, _, panel, layer) = fixture("echap");
    let before = frames(&app);
    let (n, cels) = (app.history.undo_len(), app.doc.cels.len());
    app.key_sel = vec![KeyRef { panel, layer, start: 5 }];
    assert!(app.begin_keys_drag(5));
    app.drag_keys_mode(12, KeyMove::Copy);
    // Échap : rien n'est posé, aucun dessin n'est créé, aucune étape.
    app.cancel_drags();
    assert!(app.keys_drag.is_none());
    assert_eq!(frames(&app), before);
    assert_eq!(app.history.undo_len(), n);
    assert_eq!(app.doc.cels.len(), cels);
}

#[test]
fn dragging_a_single_key_with_alt_duplicates_it_too() {
    let (mut app, cels, panel, layer) = fixture("seule");
    let before = frames(&app);
    let n = app.history.undo_len();
    app.begin_handle_drag(Handle::Key { layer: 0, start: 5, end: 10, grab: 7 });
    // Le geste se lit sur les touches à chaque image d'écran : ici on le pose comme le ferait Alt.
    app.key_drag_mode = KeyMove::Copy;
    app.key_drag_delta = 10;
    app.end_handle_drag();
    let f = frames(&app);
    assert_eq!(f[..10], before[..10]);
    let c = f[15].expect("copie");
    assert!(f[15..20].iter().all(|x| *x == Some(c)) && !cels.contains(&c), "{f:?}");
    assert_eq!(app.key_sel, vec![KeyRef { panel, layer, start: 15 }]);
    assert_eq!(app.history.undo_len(), n + 1);
    app.run_action("edition.annuler");
    assert_eq!(frames(&app), before);
}

#[test]
fn ctrl_shift_reorders_like_alt_used_to_and_shift_still_moves_the_key_alone() {
    // Réordonner (Ctrl + Maj) : l'ancien geste d'Alt. La clé 5 (images 5 à 9) glissée de 9 passe après la dernière.
    let (mut app, cels, panel, layer) = fixture("ordre");
    app.key_sel = vec![KeyRef { panel, layer, start: 5 }];
    assert!(app.begin_keys_drag(5));
    app.drag_keys_mode(14, key_move_for(&CTRL_MAJ));
    app.end_keys_drag();
    let f = frames(&app);
    assert_eq!(starts(&f), vec![0, 5, 15], "{f:?}");
    assert_eq!((f[0], f[5], f[15]), (Some(cels[0]), Some(cels[2]), Some(cels[1])), "les mêmes dessins, dans un autre ordre");
    // Maj : seule la clé choisie bouge, la suivante reste.
    let (mut app, _, panel, layer) = fixture("maj");
    app.key_sel = vec![KeyRef { panel, layer, start: 5 }];
    assert!(app.begin_keys_drag(5));
    app.drag_keys_mode(6, key_move_for(&MAJ));
    app.end_keys_drag();
    assert_eq!(starts(&frames(&app)), vec![0, 6, 10]);
}
