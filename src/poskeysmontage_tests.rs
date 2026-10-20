//! Les clés de position dans le montage (Anthony, 2026-10-20 : « ajoute le glisser dans le montage ») : les
//! losanges verts, au milieu de la piste de chaque calque, se choisissent, se glissent, se suppriment et ont leur
//! menu comme dans la timeline, sans rien changer aux gestes du dessin dessous ni aux losanges d'effets en haut.
//!
//! Les essais passent par la vraie fenêtre sans écran (`Banc`) : on retrouve où les losanges sont tombés à
//! l'écran d'après les formes peintes, puis on clique et on glisse avec le pointeur.

use eframe::egui::{self, epaint::Shape, pos2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::poskeys::{PosKey, self};
use crate::soundtrack::MenuTarget;
use crate::workspace::Espace;

const PPF: f32 = 12.0;
const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const SHIFT: Modifiers = Modifiers { alt: false, ctrl: false, shift: true, mac_cmd: false, command: false };

// ------------------------------------------------------------------------------------ le banc

fn pose(x: f32) -> crate::document::LayerTransform {
    crate::document::LayerTransform { x, ..Default::default() }
}

/// Deux plans de 24 images. Plan 1 : « Perso », un dessin de 0 à 12 puis un autre de 12 à 24, clés de position
/// aux images 4, 10 et 16. Plan 2 : « Fond », un seul dessin, clés aux images 3 et 9.
fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.add_scene_after(0);
    for i in 0..2 {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(24);
    }
    let (a, b, c) = (
        doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap)),
        doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap)),
        doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap)),
    );
    let mut perso = Layer::new("Perso", LayerKind::Bitmap);
    perso.frames = (0..24).map(|f| Some(if f < 12 { a } else { b })).collect();
    perso.pos_keys = vec![PosKey::new(4, pose(0.0)), PosKey::new(10, pose(50.0)), PosKey::new(16, pose(90.0))];
    doc.panels[0].layers.push(perso);
    let mut fond = Layer::new("Fond", LayerKind::Bitmap);
    fond.frames = vec![Some(c); 24];
    fond.pos_keys = vec![PosKey::new(3, pose(0.0)), PosKey::new(9, pose(30.0))];
    doc.panels[1].layers.push(fond);
    doc
}

/// Le montage en grand, à 12 pixels par image, sur le plan 1.
fn banc_montage() -> Banc {
    let mut b = banc_taille(film(), false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = PPF;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.garder_textes = true;
    b.chauffer(8);
    b
}

fn chercher(shape: &Shape, fill: Color32, out: &mut Vec<Pos2>) {
    match shape {
        Shape::Path(p) if p.fill == fill && p.points.len() == 4 => {
            let r = egui::Rect::from_points(&p.points);
            out.push(r.center());
        }
        Shape::Vec(v) => v.iter().for_each(|s| chercher(s, fill, out)),
        _ => {}
    }
}

/// Les centres des losanges de cette couleur peints par la dernière image, de gauche à droite.
fn losanges(b: &Banc, fill: Color32) -> Vec<Pos2> {
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, fill, &mut out);
    }
    out.sort_by(|a, b| a.x.total_cmp(&b.x));
    out
}

/// Où sont les losanges verts : l'ordonnée de leur piste, et l'abscisse de l'image `f` du film.
struct Lieu {
    cy: f32,
    x4: f32,
}

impl Lieu {
    fn x(&self, global: usize) -> f32 {
        self.x4 + (global as f32 - 4.0) * PPF
    }
    fn at(&self, global: usize) -> Pos2 {
        pos2(self.x(global), self.cy)
    }
}

fn lieu(b: &Banc) -> Lieu {
    let l = losanges(b, crate::theme::POIGNEE_CALQUE);
    assert_eq!(l.len(), 5, "trois losanges au plan 1, deux au plan 2 : {l:?}");
    // Images 4, 10, 16 du plan 1 ; 24 + 3 et 24 + 9 du plan 2.
    for (p, g) in l.iter().zip([4, 10, 16, 27, 33]) {
        assert!((p.x - (l[0].x + (g as f32 - 4.0) * PPF)).abs() < 0.6, "le losange de l'image {g} est à sa place : {l:?}");
        assert!((p.y - l[0].y).abs() < 0.6, "tous au milieu de leur piste");
    }
    Lieu { cy: l[0].y, x4: l[0].x }
}

fn bouton(p: Pos2, pressed: bool, button: PointerButton, mods: Modifiers) -> Event {
    Event::PointerButton { pos: p, button, pressed, modifiers: mods }
}

fn clic(b: &mut Banc, p: Pos2, mods: Modifiers) {
    b.touches = mods;
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![bouton(p, true, PointerButton::Primary, mods)], 1.0 / 60.0);
    b.image(vec![bouton(p, false, PointerButton::Primary, mods)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
}

fn clic_droit(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![bouton(p, true, PointerButton::Secondary, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![bouton(p, false, PointerButton::Secondary, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
}

/// Prend en `a`, tire jusqu'en `z` (six pas), lâche.
fn glisser(b: &mut Banc, a: Pos2, z: Pos2, mods: Modifiers) {
    b.touches = mods;
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true, PointerButton::Primary, mods)], 1.0 / 60.0);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![bouton(z, false, PointerButton::Primary, mods)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
}

fn cles(b: &Banc, plan: usize) -> Vec<usize> {
    b.app.doc.panels[plan].layers[0].pos_keys.iter().map(|k| k.frame).collect()
}

fn id(b: &Banc, plan: usize) -> u64 {
    b.app.doc.panels[plan].layers[0].id
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

// ------------------------------------------------------------------------------------ le choix

#[test]
fn the_centre_diamond_wins_over_the_drawing_under_it_and_only_there() {
    let mut b = banc_montage();
    let l = lieu(&b);
    // Un clic sur le losange de l'image 10 : la clé est choisie, la tête de lecture y va, aucune clé de dessin.
    clic(&mut b, l.at(10), Modifiers::NONE);
    assert_eq!(b.app.pos_ui.sel, vec![(id(&b, 0), 10)]);
    assert_eq!((b.app.cur_panel, b.app.cur_frame), (0, 10), "la tête de lecture va sur la clé");
    assert!(b.app.key_sel.is_empty(), "pas de clé de dessin sous le losange");
    assert!(b.app.pos_keys_have_focus());
    // Au-dessus du losange (la bande des clés d'effets), c'est le dessin : la clé de position n'est plus choisie.
    clic(&mut b, pos2(l.x(10), l.cy - 10.0), Modifiers::NONE);
    assert!(b.app.pos_ui.sel.is_empty(), "un clic ailleurs lâche le choix");
    assert!(!b.app.key_sel.is_empty(), "le dessin dessous a été choisi : {:?}", b.app.key_sel);
    // À deux images du losange, au même niveau : le dessin aussi.
    clic(&mut b, l.at(10) + egui::vec2(2.0 * PPF, 0.0), Modifiers::NONE);
    assert!(b.app.pos_ui.sel.is_empty());
    assert!(!b.app.key_sel.is_empty());
    // Et un clic sur le losange reprend la main aux clés de dessins.
    clic(&mut b, l.at(16), Modifiers::NONE);
    assert_eq!(b.app.pos_ui.sel, vec![(id(&b, 0), 16)]);
    assert!(b.app.key_sel.is_empty(), "le choix des dessins est lâché");
}

#[test]
fn ctrl_and_shift_add_to_the_choice_and_ctrl_takes_one_away() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let perso = id(&b, 0);
    clic(&mut b, l.at(4), Modifiers::NONE);
    clic(&mut b, l.at(16), CTRL);
    assert_eq!(b.app.pos_ui.sel, vec![(perso, 4), (perso, 16)], "Ctrl ajoute");
    clic(&mut b, l.at(4), CTRL);
    assert_eq!(b.app.pos_ui.sel, vec![(perso, 16)], "Ctrl sur une clé choisie la retire");
    // Maj : toutes celles d'entre la dernière cliquée et celle-ci.
    clic(&mut b, l.at(4), Modifiers::NONE);
    clic(&mut b, l.at(16), SHIFT);
    let mut sel = b.app.pos_ui.sel.clone();
    sel.sort();
    assert_eq!(sel, vec![(perso, 4), (perso, 10), (perso, 16)]);
}

#[test]
fn a_diamond_of_another_plan_takes_that_plan_and_leaves_the_old_choice() {
    let mut b = banc_montage();
    let l = lieu(&b);
    clic(&mut b, l.at(10), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 0);
    clic(&mut b, l.at(24 + 9), CTRL);
    assert_eq!(b.app.cur_panel, 1, "le plan du losange devient courant");
    assert_eq!(b.app.pos_ui.sel, vec![(id(&b, 1), 9)], "les clés choisies sont d'un seul plan");
    assert_eq!((b.app.global_frame(), b.app.cur_layer), (24 + 9, 0));
}

#[test]
fn the_choice_stays_with_the_diamonds_it_names() {
    let mut b = banc_montage();
    let l = lieu(&b);
    clic(&mut b, l.at(10), Modifiers::NONE);
    // Un clic sur le dessin, ailleurs : le choix est lâché et Suppr ne vise plus les clés de position.
    clic(&mut b, pos2(l.x(20), l.cy - 10.0), Modifiers::NONE);
    assert!(b.app.pos_ui.sel.is_empty());
    assert!(!b.app.pos_keys_have_focus());
}

#[test]
fn the_pointer_over_a_diamond_is_a_hand_with_a_tooltip_and_nowhere_else() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let bulle_visible = |b: &Banc| b.textes.iter().any(|t| t.contains("Clé de position, image"));
    // Sur le dessin, hors des losanges : pas de bulle de clé de position, même après longtemps.
    b.image(vec![Event::PointerMoved(l.at(10) + egui::vec2(2.0 * PPF, 0.0))], 1.0 / 60.0);
    for _ in 0..8 {
        b.image(Vec::new(), 0.2);
    }
    assert!(!bulle_visible(&b), "pas de bulle ailleurs");
    // Sur le losange : la main qui prend, tout de suite ; la bulle ensuite (son attente d'une seconde est le
    // réglage général des bulles, `bulles::DELAI` : le banc ne rend pas le temps depuis le dernier mouvement).
    b.image(vec![Event::PointerMoved(l.at(10))], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.curseur, egui::CursorIcon::Grab);
    for _ in 0..8 {
        b.image(Vec::new(), 0.2);
    }
    let bulle = b.textes.iter().find(|t| t.contains("Clé de position, image")).cloned();
    assert!(bulle.is_some_and(|t| t.contains("Perso")), "la bulle dit la clé et son calque : {:?}", b.textes.iter().filter(|t| t.contains("Clé")).collect::<Vec<_>>());
    assert!(b.textes.iter().any(|t| t.contains("Glisser pour la déplacer")), "et comment la prendre");
}

// ------------------------------------------------------------------------------------ glisser

#[test]
fn dragging_a_diamond_moves_the_chosen_keys_in_one_named_undo_step_and_leaves_the_drawing_alone() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let dessins = b.app.doc.panels[0].layers[0].frames.clone();
    clic(&mut b, l.at(4), Modifiers::NONE);
    clic(&mut b, l.at(10), CTRL);
    let avant = etapes(&b);
    // Prendre le losange 10, au milieu du dessin, et le tirer de 3 images : 4 et 10 bougent ensemble.
    glisser(&mut b, l.at(10), l.at(13), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![7, 13, 16]);
    assert_eq!(b.app.doc.panels[0].layers[0].frames, dessins, "le dessin dessous n'a pas bougé");
    assert!(b.app.key_rect.is_none() && b.app.montage_drag.is_none() && b.app.handle_drag.is_none() && b.app.keys_drag.is_none());
    assert_eq!(etapes(&b), avant + 1, "un geste, une étape");
    assert!(b.app.history.names().0.last().is_some_and(|n| n.contains("clés de position")), "{:?}", b.app.history.names().0.last());
    let mut sel = b.app.pos_ui.sel.clone();
    sel.sort();
    assert_eq!(sel, vec![(id(&b, 0), 7), (id(&b, 0), 13)], "les clés déplacées restent choisies");
    b.app.undo();
    assert_eq!(cles(&b, 0), vec![4, 10, 16]);
    assert_eq!(b.app.doc.panels[0].layers[0].frames, dessins);
}

#[test]
fn a_diamond_taken_outside_the_choice_becomes_the_choice_and_ctrl_does_not_start_a_rectangle() {
    let mut b = banc_montage();
    let l = lieu(&b);
    clic(&mut b, l.at(4), Modifiers::NONE);
    // La clé 16 n'est pas choisie : prise avec Ctrl, elle emmène seule (et pas de rectangle de choix).
    glisser(&mut b, l.at(16), l.at(18), CTRL);
    assert_eq!(cles(&b, 0), vec![4, 10, 18]);
    assert!(b.app.key_rect.is_none() && b.app.key_sel.is_empty());
    assert_eq!(b.app.pos_ui.sel, vec![(id(&b, 0), 18)]);
}

#[test]
fn the_keys_snap_to_frames_and_stay_inside_the_plan() {
    let mut b = banc_montage();
    let l = lieu(&b);
    // Entre deux images : la clé tombe sur une image entière.
    glisser(&mut b, l.at(10), l.at(10) + egui::vec2(2.4 * PPF, 0.0), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![4, 12, 16]);
    b.app.undo();
    // Tirée bien au-delà de la fin du plan (jusque dans le plan suivant) : bloquée à la dernière image (23).
    clic(&mut b, l.at(16), Modifiers::NONE);
    glisser(&mut b, l.at(16), l.at(16) + egui::vec2(20.0 * PPF, 0.0), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![4, 10, 23]);
    assert_eq!(b.app.cur_panel, 0, "le plan reste celui du calque");
    assert_eq!(cles(&b, 1), vec![3, 9], "l'autre plan n'a rien reçu");
    b.app.undo();
    // Tirée à gauche du début : bloquée à l'image 0, et le groupe garde ses écarts.
    clic(&mut b, l.at(4), Modifiers::NONE);
    clic(&mut b, l.at(10), CTRL);
    glisser(&mut b, l.at(10), l.at(10) - egui::vec2(9.0 * PPF, 0.0), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![0, 6, 16], "décalage limité par la clé la plus à gauche");
}

#[test]
fn a_key_dropped_on_another_replaces_it_as_in_the_timeline() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let avant = etapes(&b);
    glisser(&mut b, l.at(10), l.at(16), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![4, 16], "la clé arrivée remplace celle de l'image 16 (règle de la timeline)");
    assert_eq!(etapes(&b), avant + 1);
    b.app.undo();
    assert_eq!(cles(&b, 0), vec![4, 10, 16], "l'annulation rend les deux");
}

#[test]
fn escape_during_the_gesture_puts_the_keys_back_and_records_nothing() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let avant = etapes(&b);
    let (a, z) = (l.at(10), l.at(13));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true, PointerButton::Primary, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    assert_eq!(cles(&b, 0), vec![4, 13, 16], "en cours de geste, la clé suit la souris");
    assert!(b.app.dragging());
    b.app.cancel_drags();
    assert_eq!(cles(&b, 0), vec![4, 10, 16]);
    assert!(b.app.pos_ui.drag.is_none());
    b.image(vec![bouton(z, false, PointerButton::Primary, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(cles(&b, 0), vec![4, 10, 16]);
    assert_eq!(etapes(&b), avant);
}

// ------------------------------------------------------------------------------------ Suppr et menu

#[test]
fn the_delete_key_removes_the_chosen_position_keys_only() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let dessins = b.app.doc.panels[0].layers[0].frames.clone();
    clic(&mut b, l.at(4), Modifiers::NONE);
    clic(&mut b, l.at(10), CTRL);
    let avant = etapes(&b);
    b.image(
        vec![Event::Key { key: egui::Key::Delete, physical_key: Some(egui::Key::Delete), pressed: true, repeat: false, modifiers: Modifiers::NONE }],
        1.0 / 60.0,
    );
    b.chauffer(2);
    assert_eq!(cles(&b, 0), vec![16], "Suppr retire les clés de position choisies");
    assert_eq!(b.app.doc.panels[0].layers[0].frames, dessins, "pas les dessins");
    assert_eq!(etapes(&b), avant + 1);
    assert!(b.app.history.names().0.last().is_some_and(|n| n.contains("clés de position")));
    b.app.undo();
    assert_eq!(cles(&b, 0), vec![4, 10, 16]);
}

#[test]
fn the_right_click_opens_the_position_key_menu_on_a_diamond_and_chooses_it() {
    let mut b = banc_montage();
    let l = lieu(&b);
    clic_droit(&mut b, l.at(10));
    assert!(matches!(b.app.menu_target, Some(MenuTarget::PosKey)), "le menu des clés de position");
    assert_eq!(b.app.pos_ui.sel, vec![(id(&b, 0), 10)], "le losange non choisi l'est seul");
    assert!(b.textes.iter().any(|t| t.contains("Lisser la clé de position")), "{:?}", b.textes.iter().filter(|t| t.contains("clé")).collect::<Vec<_>>());
    assert!(b.textes.iter().any(|t| t.contains("Supprimer la clé de position")));
    // Sur le dessin dessous (hors du losange), le menu est celui du calque.
    clic(&mut b, pos2(0.0, 0.0), Modifiers::NONE);
    b.chauffer(2);
    clic_droit(&mut b, l.at(10) + egui::vec2(2.0 * PPF, 0.0));
    assert!(matches!(b.app.menu_target, Some(MenuTarget::Layer(_))));
}

#[test]
fn the_smooth_command_of_the_menu_rounds_the_key_under_the_playhead() {
    let mut b = banc_montage();
    let l = lieu(&b);
    clic_droit(&mut b, l.at(10));
    let avant = etapes(&b);
    b.app.run_action("calque.cle_position_lisser");
    assert!(b.app.doc.panels[0].layers[0].pos_keys[1].smooth, "la clé 10 est lissée");
    assert_eq!(etapes(&b), avant + 1);
    b.chauffer(2);
    // Un losange lissé est un rond : on le prend encore.
    glisser(&mut b, l.at(10), l.at(12), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![4, 12, 16]);
    assert!(poskeys::key_at(&b.app.doc.panels[0].layers[0].pos_keys, 12).is_some_and(|i| b.app.doc.panels[0].layers[0].pos_keys[i].smooth));
}

// ------------------------------------------------------------------------------------ pas de régression

#[test]
fn the_gestures_of_the_drawing_under_are_unchanged() {
    let mut b = banc_montage();
    let l = lieu(&b);
    let perso = &b.app.doc.panels[0].layers[0];
    let (a, c) = (perso.frames[0], perso.frames[12]);
    // La limite entre les deux dessins (image 12) se tire : l'un s'allonge, l'autre raccourcit.
    clic(&mut b, pos2(l.x(12) - PPF * 0.5, l.cy - 10.0), Modifiers::NONE);
    glisser(&mut b, pos2(l.x(12) - PPF * 0.5, l.cy), pos2(l.x(14) - PPF * 0.5, l.cy), Modifiers::NONE);
    let frames = &b.app.doc.panels[0].layers[0].frames;
    assert_eq!((frames[12], frames[13], frames[14]), (a, a, c), "la limite est passée de 12 à 14");
    // (La suite du calque est poussée de deux images, avec ses clés de position : c'était déjà ainsi.)
    assert_eq!(cles(&b, 0), vec![4, 10, 18], "les clés d'après la limite suivent les images poussées");
    assert!(b.app.pos_ui.sel.is_empty() && b.app.pos_ui.drag.is_none());
    b.app.undo();
    assert_eq!(b.app.doc.panels[0].layers[0].frames[13], c);
    assert_eq!(cles(&b, 0), vec![4, 10, 16]);
}

#[test]
fn a_gesture_that_starts_elsewhere_lets_go_of_the_position_choice_and_keeps_its_own_meaning() {
    let mut b = banc_montage();
    let l = lieu(&b);
    clic(&mut b, l.at(10), Modifiers::NONE);
    assert!(!b.app.pos_ui.sel.is_empty());
    // Ctrl + glisser depuis le dessin du plan 2, hors de tout losange : le rectangle de choix des clés de dessins.
    let dessus = pos2(l.x(24 + 14), l.cy - 10.0);
    b.touches = CTRL;
    b.image(vec![Event::PointerMoved(dessus)], 1.0 / 60.0);
    b.image(vec![bouton(dessus, true, PointerButton::Primary, CTRL)], 1.0 / 60.0);
    for k in 1..=4 {
        b.image(vec![Event::PointerMoved(dessus + egui::vec2(k as f32 * 6.0, 4.0))], 1.0 / 60.0);
    }
    assert!(b.app.key_rect.is_some(), "le rectangle de choix est tiré");
    assert!(b.app.pos_ui.sel.is_empty(), "le geste n'est pas parti d'un losange vert : le choix est lâché");
    b.image(vec![bouton(dessus, false, PointerButton::Primary, CTRL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    assert_eq!(cles(&b, 0), vec![4, 10, 16]);
    assert_eq!(cles(&b, 1), vec![3, 9]);
}

/// Un effet à clés 2, 10 et 14 sur « Perso », choisi : ses losanges violets sont en haut de la piste.
fn avec_effet(b: &mut Banc) {
    let mut blur = Effect::new(EffectKind::GaussianBlur);
    for (f, v) in [(2, 3.0), (10, 6.0), (14, 9.0)] {
        blur.set_key(f, vec![v]);
    }
    b.app.doc.panels[0].layers[0].fx = vec![blur];
    (b.app.cur_panel, b.app.cur_layer) = (0, 0);
    b.app.select_fx(0);
    b.chauffer(4);
}

fn frames_fx(b: &Banc) -> Vec<usize> {
    b.app.doc.panels[0].layers[0].fx[0].keys.iter().map(|k| k.frame).collect()
}

#[test]
fn effect_diamonds_on_top_and_position_diamonds_in_the_middle_do_not_conflict() {
    let mut b = banc_montage();
    avec_effet(&mut b);
    let l = lieu(&b);
    let violets = losanges(&b, crate::fxkeys::color());
    assert_eq!(violets.len(), 3, "trois losanges violets : {violets:?}");
    let haut = violets[0].y;
    assert!(haut < l.cy - 6.0, "le violet est au-dessus du vert ({haut} < {})", l.cy);
    let fx_at = |g: usize| pos2(l.x(g), haut);
    // Au même endroit sur l'axe des images (10), deux losanges : le violet en haut, le vert au milieu.
    clic(&mut b, fx_at(10), Modifiers::NONE);
    assert!(!b.app.fx_key_sel.is_empty() && b.app.pos_ui.sel.is_empty(), "le violet est pris en haut");
    clic(&mut b, l.at(10), Modifiers::NONE);
    assert!(b.app.fx_key_sel.is_empty() && !b.app.pos_ui.sel.is_empty(), "le vert au milieu, et le violet est lâché");
    // Le glisser d'une clé d'effet marche comme avant : elle emmène les clés d'après de son effet.
    let avant = etapes(&b);
    glisser(&mut b, fx_at(10), fx_at(12), Modifiers::NONE);
    assert_eq!(frames_fx(&b), vec![2, 12, 16]);
    assert_eq!(cles(&b, 0), vec![4, 10, 16], "les clés de position n'ont pas bougé");
    assert_eq!(etapes(&b), avant + 1);
    b.app.undo();
    assert_eq!(frames_fx(&b), vec![2, 10, 14]);
    // Et le vert se glisse sans toucher à l'effet.
    glisser(&mut b, l.at(10), l.at(11), Modifiers::NONE);
    assert_eq!(cles(&b, 0), vec![4, 11, 16]);
    assert_eq!(frames_fx(&b), vec![2, 10, 14], "l'effet n'a pas bougé");
}

#[test]
fn hit_testing_is_pure_and_cheap_to_call_on_every_frame() {
    let mut b = banc_montage();
    let l = lieu(&b);
    // La même fonction qui sert au survol : un point loin de tout losange ne trouve rien, sur toute la bande.
    let rows = b.app.montage_rows(300.0);
    // Les blocs des deux plans, comme la bande les pose (`x_of(début) + 1` à `x_of(fin) - 1`).
    let blocks = vec![egui::Rect::from_min_max(pos2(l.x(0) - 5.0, 100.0), pos2(l.x(24) - 7.0, 200.0)), egui::Rect::from_min_max(pos2(l.x(24) - 5.0, 100.0), pos2(l.x(48) - 7.0, 200.0))];
    let top = 0.0;
    let centre = rows.layers_top() + crate::montagerows::LAYER_H * 0.5;
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(10), top + centre), top, &blocks, PPF, &rows), Some((0, 0, 10)));
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(24 + 9), top + centre), top, &blocks, PPF, &rows), Some((1, 0, 9)));
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(7), top + centre), top, &blocks, PPF, &rows), None, "entre deux losanges");
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(10), top + centre - 9.0), top, &blocks, PPF, &rows), None, "au-dessus : la bande des effets");
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(10), top + centre + 9.0), top, &blocks, PPF, &rows), None, "au-dessous : le dessin");
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(10), top + rows.layers_top() - 3.0), top, &blocks, PPF, &rows), None, "hors des pistes de calques");
    // Au bord de la piste : la clé est dans le plan (image < durée) seulement.
    b.app.doc.panels[0].layers[0].pos_keys.push(PosKey::new(30, pose(0.0)));
    assert_eq!(b.app.montage_pos_key_at(pos2(l.x(23), top + centre), top, &blocks, PPF, &rows), None, "une clé au-delà de la fin du plan n'est pas prise");
}

#[test]
fn the_range_between_two_keys_is_pure() {
    let keys = vec![PosKey::new(2, pose(0.0)), PosKey::new(5, pose(0.0)), PosKey::new(9, pose(0.0))];
    assert_eq!(poskeys::frames_between(&keys, 2, 9), vec![2, 5, 9]);
    assert_eq!(poskeys::frames_between(&keys, 9, 5), vec![5, 9], "dans l'autre sens");
    assert_eq!(poskeys::frames_between(&keys, 3, 3), vec![3], "au moins celle qu'on clique");
    assert_eq!(crate::poskeysui::marker_near([2usize, 5, 9].into_iter(), 61.0, |f| f as f32 * 12.0, 5.0), Some(5));
    assert_eq!(crate::poskeysui::marker_near([2usize, 5, 9].into_iter(), 80.0, |f| f as f32 * 12.0, 5.0), None);
}
