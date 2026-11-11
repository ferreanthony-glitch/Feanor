//! Essais de la partie 138 (`clesordre.rs`, `clesduree.rs`) : glisser des clés avant ou après une autre clé ; étirer un plan étire ses
//! dernières clés ; « Modifier la durée des clés d'images… » ; la durée de plusieurs plans choisis.
//!
//! Les gestes passent par les mêmes appels que l'écran : `begin_handle_drag` / `drag_handle` / `end_handle_drag` (une clé prise
//! dans le montage, la poignée de durée de la timeline), `begin_keys_drag` / `drag_keys` / `end_keys_drag` (plusieurs clés choisies),
//! `image_insertion` / `insert_chosen_images` (le lâcher d'images choisies dans la timeline d'Animation). Sans touche tenue : le
//! contexte d'essai n'en a aucune.

use eframe::egui;

use crate::app::App;
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::handles::{apply_key_move, Handle, KeyMove};
use crate::montagekeys::KeyRef;

fn app_avec(doc: Document, name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-cles-glisser-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app
}

/// Un calque nommé `name` aux expositions `spans` (début, fin exclue), chacune son dessin.
fn calque(doc: &mut Document, name: &str, spans: &[(usize, usize)]) -> (Layer, Vec<CelId>) {
    let mut l = Layer::new(name, LayerKind::Vector);
    let mut cels = Vec::new();
    for &(s, e) in spans {
        let c = doc.new_cel(LayerKind::Vector);
        for f in s..e {
            l.set(f, Some(c));
        }
        cels.push(c);
    }
    (l, cels)
}

/// Un plan de 20 images, un calque « A » : dessins 0 à 4, 5 à 9, 10 à 19.
fn fixture(name: &str) -> (App, Vec<CelId>, u64, u64) {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let (l, cels) = calque(&mut doc, "A", &[(0, 5), (5, 10), (10, 20)]);
    let layer = l.id;
    doc.panels[0].layers.push(l);
    let panel = doc.panels[0].id;
    (app_avec(doc, name), cels, panel, layer)
}

fn frames(app: &App, li: usize) -> Vec<Option<CelId>> {
    app.doc.panels[0].layers[li].frames.clone()
}

/// (début, dessin) de chaque exposition dessinée.
fn keys(f: &[Option<CelId>]) -> Vec<(usize, CelId)> {
    (0..f.len()).filter(|&i| f[i].is_some() && (i == 0 || f[i - 1] != f[i])).map(|i| (i, f[i].unwrap())).collect()
}

fn aller_retour(doc: &Document, nom: &str) -> Document {
    let dir = std::env::temp_dir().join(format!("shot-cles-glisser-ar-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(doc, &manifest, &mut Default::default()).unwrap();
    let d = crate::project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    d
}

/// Une clé prise par son milieu dans le montage, glissée jusqu'à l'image `to` du plan, lâchée.
fn glisser_cle(app: &mut App, start: usize, end: usize, grab: usize, to: usize) {
    assert!(app.begin_handle_drag(Handle::Key { layer: 0, start, end, grab }));
    app.drag_handle(to, to);
    app.end_handle_drag();
}

// ------------------------------------------------------------------ 1. glisser avant ou après une autre clé

#[test]
fn a_key_dropped_on_the_second_half_of_its_neighbour_goes_after_it() {
    let (mut app, c, _, _) = fixture("apres");
    let n = app.history.undo_len();
    // La clé 0-4 prise à l'image 2, lâchée à l'image 8 (seconde moitié du dessin 5-9). Avant : la clé et la suite glissaient de
    // +6 (le calque commençait par 6 images vides) ; jamais après sa voisine.
    glisser_cle(&mut app, 0, 5, 2, 8);
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[1]), (5, c[0]), (10, c[2])], "insérée après sa voisine, durées gardées");
    assert_eq!(app.doc.panels[0].frame_count(), 20, "la durée du plan ne change pas");
    assert_eq!(app.history.undo_len(), n + 1, "une étape d'annulation");
    assert!(app.status.contains("1 clé déplacée après l'image"), "{}", app.status);
}

#[test]
fn a_key_dropped_on_the_first_half_of_the_first_key_goes_before_it() {
    let (mut app, c, _, _) = fixture("avant");
    // La dernière clé (10-19) prise à l'image 12, lâchée à l'image 1 : avant la première. Avant : elle butait (la clé d'avant
    // gardait au moins une image).
    glisser_cle(&mut app, 10, 20, 12, 1);
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[2]), (10, c[0]), (15, c[1])]);
    assert_eq!(frames(&app, 0).len(), 20);
    assert!(app.status.contains("avant l'image"), "{}", app.status);
}

#[test]
fn several_chosen_keys_move_together_in_their_order() {
    let (mut app, c, panel, layer) = fixture("plusieurs");
    let key = |start| KeyRef { panel, layer, start };
    app.key_sel = vec![key(0), key(5)];
    assert!(app.begin_keys_drag(2));
    // La souris à l'image 17 : seconde moitié du dessin 10-19.
    app.drag_keys(17);
    app.end_keys_drag();
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[2]), (10, c[0]), (15, c[1])]);
    assert_eq!(app.key_sel, vec![key(10), key(15)], "les clés déplacées restent choisies");
    assert!(app.status.contains("2 clés déplacées après l'image"), "{}", app.status);
}

#[test]
fn each_layer_reorders_on_its_own_and_a_locked_or_hidden_layer_stays() {
    for (garde, nom) in [(true, "verrou"), (false, "masque")] {
        let (mut app, c, panel, layer) = fixture(nom);
        let (l2, d) = calque(&mut app.doc, "B", &[(0, 5), (5, 10), (10, 20)]);
        let (l3, e) = calque(&mut app.doc, "C", &[(0, 5), (5, 10), (10, 20)]);
        let (id2, id3) = (l2.id, l3.id);
        app.doc.panels[0].layers.push(l2);
        app.doc.panels[0].layers.push(l3);
        if garde {
            app.doc.panels[0].layers[2].locked = true;
        } else {
            app.doc.panels[0].layers[2].visible = false;
        }
        let n = app.history.undo_len();
        app.key_sel = vec![KeyRef { panel, layer, start: 0 }, KeyRef { panel, layer: id2, start: 0 }, KeyRef { panel, layer: id3, start: 0 }];
        assert!(app.begin_keys_drag(2));
        app.drag_keys(8);
        app.end_keys_drag();
        assert_eq!(keys(&frames(&app, 0)), vec![(0, c[1]), (5, c[0]), (10, c[2])], "le calque de la clé prise");
        assert_eq!(keys(&frames(&app, 1)), vec![(0, d[1]), (5, d[0]), (10, d[2])], "le deuxième calque, de son côté");
        assert_eq!(keys(&frames(&app, 2)), vec![(0, e[0]), (5, e[1]), (10, e[2])], "le calque verrouillé ou masqué ne bouge pas");
        assert!(app.status.contains("2 clés déplacées") && app.status.contains("« C »"), "{}", app.status);
        assert_eq!(app.history.undo_len(), n + 1);
    }
}

#[test]
fn a_shared_drawing_stays_the_same_drawing_and_nothing_is_lost() {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(16);
    let (mut l, c) = calque(&mut doc, "A", &[(0, 4), (4, 8), (8, 12)]);
    // Le premier dessin revient à la fin (un dessin réutilisé).
    for f in 12..16 {
        l.set(f, Some(c[0]));
    }
    doc.panels[0].layers.push(l);
    let mut app = app_avec(doc, "partage");
    let dessins = app.doc.cels.len();
    // Le dessin 4-7 lâché sur la seconde moitié du dernier (12-15) : après lui.
    glisser_cle(&mut app, 4, 8, 5, 14);
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[0]), (4, c[2]), (8, c[0]), (12, c[1])]);
    assert_eq!(app.doc.cels.len(), dessins, "aucun dessin créé ni perdu : le dessin réutilisé reste le même");
}

#[test]
fn undo_then_redo_and_save_then_reopen() {
    let (mut app, c, _, _) = fixture("annuler");
    let avant = frames(&app, 0);
    glisser_cle(&mut app, 0, 5, 2, 8);
    let apres = frames(&app, 0);
    app.run_action("edition.annuler");
    assert_eq!(frames(&app, 0), avant);
    app.run_action("edition.retablir");
    assert_eq!(frames(&app, 0), apres);
    let relu = aller_retour(&app.doc, "glisser");
    assert_eq!(keys(&relu.panels[0].layers[0].frames), vec![(0, c[1]), (5, c[0]), (10, c[2])]);
}

#[test]
fn a_drop_on_empty_frames_keeps_the_old_gesture() {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let (l, _) = calque(&mut doc, "A", &[(0, 5), (5, 10)]);
    doc.panels[0].layers.push(l);
    let mut app = app_avec(doc, "vide");
    let mut attendu = app.doc.panels[0].layers[0].clone();
    // La clé 5-9 prise à l'image 7, lâchée à l'image 14 : des images vides. Comme avant, elle glisse de +7 avec ce qui la suit.
    apply_key_move(&mut attendu, &[5], 7, KeyMove::Ripple, 20);
    glisser_cle(&mut app, 5, 10, 7, 14);
    assert_eq!(frames(&app, 0), attendu.frames);
    assert_eq!(keys(&frames(&app, 0))[1].0, 12);
    // Lâchée sur elle-même, rien ne s'insère non plus.
    assert_eq!(crate::clesordre::insertion_at(&app.doc.panels[0].layers[0], &[12], 13), None);
}

#[test]
fn the_animation_timeline_inserts_chosen_images_before_or_after_another_key() {
    let (mut app, c, _, _) = fixture("timeline");
    app.mode = crate::app::Mode::Animation;
    // Les images de la clé 0-4 choisies, glissées sur la seconde moitié de la clé 5-9.
    app.frame_sel = Some((0, 5));
    let d = app.image_insertion(0, 8).expect("sur un autre dessin : insertion");
    assert_eq!(app.image_insertion_lines(d), vec![(0, 10)], "la ligne d'insertion, à la fin du dessin 5-9");
    app.insert_chosen_images(0, d);
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[1]), (5, c[0]), (10, c[2])]);
    assert_eq!(app.frame_sel, Some((5, 10)), "les images suivent la clé");
    assert!(app.status.contains("1 clé déplacée après l'image"), "{}", app.status);
    // Une partie d'une clé seulement, ou des images vides : l'ancien geste (déplacer, par-dessus).
    app.frame_sel = Some((6, 8));
    assert_eq!(app.image_insertion(0, 12), None);
    app.frame_sel = Some((10, 20));
    assert_eq!(app.image_insertion(0, 25), None);
}

// ------------------------------------------------------------------ 2. étirer un plan étire ses dernières clés

/// Un plan de 20 images, trois calques de longueurs différentes : 20, 12 et 8 images.
fn trois_calques(name: &str) -> (App, [CelId; 3]) {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let (a, ca) = calque(&mut doc, "Long", &[(0, 20)]);
    let (b, cb) = calque(&mut doc, "Moyen", &[(0, 12)]);
    let (c, cc) = calque(&mut doc, "Court", &[(0, 5), (5, 8)]);
    doc.panels[0].layers.extend([a, b, c]);
    (app_avec(doc, name), [ca[0], cb[0], cc[1]])
}

fn tirer_la_fin(app: &mut App, to: usize) {
    assert!(app.begin_handle_drag(Handle::PlanEnd));
    app.drag_handle(to, to);
    app.end_handle_drag();
}

#[test]
fn lengthening_a_plan_stretches_the_last_key_of_every_layer() {
    let (mut app, last) = trois_calques("etirer");
    let n = app.history.undo_len();
    tirer_la_fin(&mut app, 30);
    assert_eq!(app.doc.panels[0].frame_count(), 30);
    for (li, cel) in last.iter().enumerate() {
        let l = &app.doc.panels[0].layers[li];
        assert_eq!(l.frames.len(), 30, "calque {li} : plus de vide au bout");
        assert_eq!(l.cel_at(29), Some(*cel), "calque {li} : sa dernière clé tenue");
    }
    assert_eq!(keys(&app.doc.panels[0].layers[2].frames)[1].0, 5, "la dernière clé garde son début");
    assert_eq!(app.history.undo_len(), n + 1);
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels[0].frame_count(), 20);
    assert_eq!(app.doc.panels[0].layers[1].frames.len(), 12);
    app.run_action("edition.retablir");
    assert_eq!(app.doc.panels[0].layers[1].frames.len(), 30);
    let relu = aller_retour(&app.doc, "etirer");
    assert!(relu.panels[0].layers.iter().all(|l| l.frames.len() == 30));
}

#[test]
fn an_empty_last_key_stays_empty_and_a_locked_layer_is_left_alone() {
    let (mut app, _) = trois_calques("vide-verrou");
    // Une clé vide (un dessin sans trait) en dernier sur « Court » ; « Moyen » verrouillé.
    let vide = app.doc.new_cel(LayerKind::Vector);
    for f in 8..10 {
        app.doc.panels[0].layers[2].set(f, Some(vide));
    }
    app.doc.panels[0].layers[1].locked = true;
    tirer_la_fin(&mut app, 30);
    let l = &app.doc.panels[0].layers[2];
    assert_eq!(l.cel_at(29), Some(vide), "la clé vide est tenue : le calque reste vide à l'écran");
    assert!(app.doc.cels[&vide].is_empty(), "rien n'y est dessiné");
    assert_eq!(app.doc.panels[0].layers[1].frames.len(), 12, "le calque verrouillé reste tel quel");
    assert!(app.status.contains("verrouillé"), "{}", app.status);
}

#[test]
fn ctrl_on_the_handle_still_slides_the_keys_and_shorten_then_lengthen_loses_nothing() {
    // Ctrl : `slide_plan_end`, comme avant (les clés suivent la poignée, les nouvelles images sont au début).
    let (mut app, _) = trois_calques("ctrl");
    app.doc.slide_plan_end(0, 30);
    let moyen = &app.doc.panels[0].layers[1].frames;
    assert_eq!((moyen.len(), moyen[..10].iter().all(Option::is_none)), (22, true), "décalé de 10, pas étiré");
    // Raccourcir puis rallonger par le bord de fin du montage : rien ne se perd, les fins s'étirent.
    let (mut app, last) = trois_calques("aller-retour");
    let dessins = app.doc.used_cels();
    app.doc.rogner(0, crate::rognage::Rogner::Tail(-14)).unwrap();
    assert_eq!(app.doc.panels[0].frame_count(), 6);
    app.doc.rogner(0, crate::rognage::Rogner::Tail(19)).unwrap();
    assert_eq!(app.doc.panels[0].frame_count(), 25);
    assert!(dessins.iter().all(|c| app.doc.used_cels().contains(c)), "aucun dessin perdu");
    for (li, cel) in last.iter().enumerate() {
        assert_eq!(app.doc.panels[0].layers[li].cel_at(24), Some(*cel), "calque {li}");
    }
}

// ------------------------------------------------------------------ 3. « Modifier la durée des clés d'images… »

fn sel(panel: u64, layer: u64, starts: &[usize]) -> Vec<KeyRef> {
    starts.iter().map(|&start| KeyRef { panel, layer, start }).collect()
}

#[test]
fn one_key_takes_the_duration_and_the_following_keys_shift() {
    let (mut app, c, panel, layer) = fixture("duree-une");
    let n = app.history.undo_len();
    assert!(app.change_key_durations(&sel(panel, layer, &[0]), 8));
    // La dernière clé, tenue jusqu'à la fin du plan, absorbe : le plan garde ses 20 images.
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[0]), (8, c[1]), (13, c[2])]);
    assert_eq!(frames(&app, 0).len(), 20);
    assert_eq!(app.history.undo_len(), n + 1);
    assert!(app.status.starts_with("1 clé passée à 8 images sur 1 calque"), "{}", app.status);
    // 0 : refusé, rien ne change.
    let avant = frames(&app, 0);
    assert!(!app.change_key_durations(&sel(panel, layer, &[0]), 0));
    assert_eq!(frames(&app, 0), avant);
    assert_eq!(app.history.undo_len(), n + 1);
    assert!(app.status.contains("au moins une image"), "{}", app.status);
}

#[test]
fn several_keys_of_different_durations_on_two_layers_and_a_locked_one() {
    let (mut app, c, panel, layer) = fixture("duree-plusieurs");
    let (l2, d) = calque(&mut app.doc, "B", &[(0, 3), (3, 20)]);
    let (l3, _) = calque(&mut app.doc, "Verrou", &[(0, 4), (4, 20)]);
    let (id2, id3) = (l2.id, l3.id);
    app.doc.panels[0].layers.extend([l2, l3]);
    app.doc.panels[0].layers[2].locked = true;
    let mut keys_sel = sel(panel, layer, &[0, 10]);
    keys_sel.extend(sel(panel, id2, &[0]));
    keys_sel.extend(sel(panel, id3, &[0]));
    // La fenêtre : des durées différentes, le champ est vide.
    app.mode = crate::app::Mode::Storyboard;
    app.key_sel = keys_sel.clone();
    app.run_action("cles.duree");
    assert_eq!(app.duree_cles.as_ref().map(|d| d.text.clone()), Some(String::new()), "durées différentes : champ vide");
    app.key_sel = sel(panel, layer, &[0, 5]);
    app.run_action("cles.duree");
    assert_eq!(app.duree_cles.as_ref().map(|d| d.text.clone()), Some("5".into()), "la durée commune");
    app.duree_cles = None;
    assert!(app.change_key_durations(&keys_sel, 6));
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[0]), (6, c[1]), (11, c[2])], "« A » : 6, la suivante décalée, la dernière (choisie) à 6");
    assert_eq!(frames(&app, 0).len(), 17);
    assert_eq!(keys(&frames(&app, 1)), vec![(0, d[0]), (6, d[1])], "« B » : la suite décalée");
    assert_eq!(frames(&app, 1).len(), 20, "« B » tenu jusqu'au bout le reste");
    assert_eq!(keys(&frames(&app, 2))[1].0, 4, "le calque verrouillé ne change pas");
    assert!(app.status.starts_with("3 clés passées à 6 images sur 2 calques") && app.status.contains("« Verrou »"), "{}", app.status);
    let apres = (frames(&app, 0), frames(&app, 1));
    app.run_action("edition.annuler");
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[0]), (5, c[1]), (10, c[2])]);
    app.run_action("edition.retablir");
    assert_eq!((frames(&app, 0), frames(&app, 1)), apres);
    let relu = aller_retour(&app.doc, "duree");
    assert_eq!(relu.panels[0].layers[0].frames, apres.0);
}

#[test]
fn a_key_lengthened_past_the_end_lengthens_the_plan_and_shifts_the_next_one() {
    let (mut app, c, panel, layer) = fixture("duree-depasse");
    app.doc.add_scene_after(0);
    let (autre, oc) = calque(&mut app.doc, "Autre", &[(0, 12)]);
    app.doc.panels[0].layers.push(autre);
    let debut_suivant = app.doc.panel_start(1);
    assert_eq!(debut_suivant, 20);
    // La clé 5-9 passe à 20 images : la dernière commence alors à 25, après la fin (20) : le plan s'allonge à 35.
    assert!(app.change_key_durations(&sel(panel, layer, &[5]), 20));
    assert_eq!(keys(&frames(&app, 0)), vec![(0, c[0]), (5, c[1]), (25, c[2])]);
    assert_eq!(app.doc.panels[0].frame_count(), 35);
    assert_eq!(app.doc.panel_start(1), 35, "le plan suivant se décale");
    assert_eq!(app.doc.panels[0].layers[1].cel_at(34), Some(oc[0]), "l'autre calque étire sa dernière clé");
    assert!(app.status.contains("allongé à 1 s 11 i"), "{}", app.status);
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels[0].frame_count(), 20);
    assert_eq!(app.doc.panel_start(1), 20);
    assert_eq!(app.doc.panels[0].layers[1].frames.len(), 12);
}

// ------------------------------------------------------------------ 4. la durée de plusieurs plans choisis

/// Trois plans de 10, 20 et 30 images ; sur chacun un calque tenu jusqu'au bout et un autre de 4 images.
fn trois_plans(name: &str) -> App {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (i, n) in [10, 20, 30].into_iter().enumerate() {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(n);
        let (a, _) = calque(&mut doc, "Tenu", &[(0, n)]);
        let (b, _) = calque(&mut doc, "Court", &[(0, 4)]);
        doc.panels[i].layers.extend([a, b]);
    }
    app_avec(doc, name)
}

#[test]
fn several_plans_take_the_same_duration_the_next_ones_shift_and_last_keys_stretch() {
    let mut app = trois_plans("plans");
    let ids: Vec<u64> = app.doc.panels.iter().map(|p| p.id).collect();
    app.board_sel = ids.clone();
    app.run_action("montage.duree_plans");
    assert_eq!(app.duree_plans.as_ref().map(|d| d.text.clone()), Some(String::new()), "durées différentes : champ vide");
    app.duree_plans = None;
    let n = app.history.undo_len();
    assert_eq!(app.change_plans_duration(&ids, "24"), Some(3));
    assert_eq!(app.doc.panels.iter().map(|p| p.frame_count()).collect::<Vec<_>>(), vec![24, 24, 24]);
    assert_eq!((app.doc.panel_start(1), app.doc.panel_start(2)), (24, 48), "les plans suivants se décalent");
    assert_eq!(app.doc.panels[0].layers[1].frames.len(), 24, "allongé : la dernière clé du calque court s'étire");
    assert_eq!(app.doc.panels[2].layers[1].frames.len(), 4, "raccourci : comme avant, la fin est rognée, le calque court ne change pas");
    assert_eq!(app.doc.hidden(2), (0, 6), "ce qui est rogné reste en réserve");
    assert!(app.status.starts_with("3 plans passés à 1 s (24 i)"), "{}", app.status);
    assert_eq!(app.history.undo_len(), n + 1);
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.iter().map(|p| p.frame_count()).collect::<Vec<_>>(), vec![10, 20, 30]);
    app.run_action("edition.retablir");
    assert_eq!(app.doc.panels.iter().map(|p| p.frame_count()).collect::<Vec<_>>(), vec![24, 24, 24]);
    let relu = aller_retour(&app.doc, "plans");
    assert_eq!(relu.panels.iter().map(|p| p.frame_count()).collect::<Vec<_>>(), vec![24, 24, 24]);
}

#[test]
fn a_single_plan_duration_works_as_before_and_several_chosen_open_the_shared_window() {
    let mut app = trois_plans("un-plan");
    // Un seul plan : « Durée… » ouvre sa fenêtre à lui, comme avant.
    app.run_action("montage.duree");
    assert!(app.duration_dialog.is_some() && app.duree_plans.is_none());
    app.duration_dialog = None;
    assert_eq!(app.rogner_plan(0, crate::rognage::Rogner::Duration(16), "Durée"), Some(6));
    assert_eq!(app.doc.panels[0].frame_count(), 16);
    assert_eq!(app.doc.panel_start(1), 16);
    // Plusieurs plans choisis, dont celui-ci : « Durée… » ouvre la fenêtre des plans choisis.
    app.board_sel = app.doc.panels.iter().map(|p| p.id).collect();
    app.run_action("montage.duree");
    assert!(app.duree_plans.is_some() && app.duration_dialog.is_none());
}
