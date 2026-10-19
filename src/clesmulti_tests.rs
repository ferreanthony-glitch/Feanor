//! Tests du choix de clés de dessins sur plusieurs calques à la fois dans le montage (Anthony,
//! 2026-10-19 : « un moyen simple de sélectionner plusieurs images clés, de plusieurs calques en
//! même temps ») : au rectangle sur toutes les rangées couvertes, Ctrl + clic sur n'importe quel
//! calque, Maj + clic en rectangle entre la clé de départ et la clé cliquée ; puis glisser, copier,
//! coller, supprimer sur ce choix, une étape d'annulation par opération.

use eframe::egui::{self, pos2};

use crate::app::App;
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::montagekeys::{key_counts, key_counts_text, KeyRect, KeyRef};

/// Trois calques, du haut vers le bas : A, B, C (la pile est rangée du bas vers le haut).
/// A : clés aux images 0, 5, 10 ; B : 0, 5, 10 ; C : 0 et 8. Plan de 20 images.
struct Fixture {
    app: App,
    panel: u64,
    a: u64,
    b: u64,
    c: u64,
}

fn drawn(doc: &mut Document) -> CelId {
    doc.new_cel(LayerKind::Vector)
}

fn layer_with(doc: &mut Document, name: &str, starts: &[usize], len: usize) -> Layer {
    let mut l = Layer::new(name, LayerKind::Vector);
    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(len);
        let cel = drawn(doc);
        for f in s..end {
            l.set(f, Some(cel));
        }
    }
    l
}

fn fixture(name: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("shot-clesmulti-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(32, 24, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let c = layer_with(&mut doc, "C", &[0, 8], 20);
    let b = layer_with(&mut doc, "B", &[0, 5, 10], 20);
    let a = layer_with(&mut doc, "A", &[0, 5, 10], 20);
    let ids = (a.id, b.id, c.id);
    doc.panels[0].layers.extend([c, b, a]);
    let panel = doc.panels[0].id;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 2;
    app.cur_frame = 0;
    Fixture { app, panel, a: ids.0, b: ids.1, c: ids.2 }
}

fn frames_of(app: &App) -> Vec<Vec<Option<u64>>> {
    app.doc.panels[0].layers.iter().map(|l| l.frames.clone()).collect()
}

fn names_of(app: &App) -> Vec<String> {
    app.doc.panels[0].layers.iter().rev().map(|l| l.name.clone()).collect()
}

impl Fixture {
    fn k(&self, layer: u64, start: usize) -> KeyRef {
        KeyRef { panel: self.panel, layer, start }
    }
}

#[test]
fn a_marquee_picks_every_key_it_touches_on_all_the_rows_it_spans() {
    let mut f = fixture("marquee");
    f.app.key_rect = Some(KeyRect { origin: pos2(0.0, 0.0), before: Vec::new(), add: false, toggle: false });
    // Rangées 0 à 2 (A, B, C), images 4 à 9 : A et B ont leurs clés 0 (jusqu'à 5) et 5, C ses clés 0 (jusqu'à 8) et 8.
    f.app.drag_key_rect(0..=2, 4, 9);
    assert_eq!(key_counts(&f.app.key_sel), (6, 3));
    assert_eq!(key_counts_text(&f.app.key_sel), "6 clés sur 3 calques");
    // Seulement les rangées 1 et 2.
    f.app.drag_key_rect(1..=2, 4, 9);
    assert_eq!(key_counts(&f.app.key_sel), (4, 2));
    assert!(!f.app.key_sel.iter().any(|k| k.layer == f.a), "A n'est pas couvert");
    // Un rectangle étroit, une seule rangée : une seule clé.
    f.app.drag_key_rect(0..=0, 11, 12);
    assert_eq!(f.app.key_sel, vec![f.k(f.a, 10)]);
    // Maj ajoute au choix d'avant ; Ctrl bascule les clés touchées (comme pour la caméra).
    let (a5, b5) = (f.k(f.a, 5), f.k(f.b, 5));
    f.app.key_rect = Some(KeyRect { origin: pos2(0.0, 0.0), before: vec![a5], add: true, toggle: false });
    f.app.drag_key_rect(1..=1, 6, 7);
    assert_eq!(f.app.key_sel, vec![a5, b5], "Maj : ajoutée");
    f.app.key_rect = Some(KeyRect { origin: pos2(0.0, 0.0), before: vec![a5, b5], add: false, toggle: true });
    f.app.drag_key_rect(1..=1, 6, 7);
    assert_eq!(f.app.key_sel, vec![a5], "Ctrl : B@5 était choisie, le rectangle la retire");
}

#[test]
fn ctrl_toggles_a_key_on_any_layer_and_shift_takes_the_rectangle_between() {
    let mut f = fixture("clic");
    let (a5, b10, c8, a0) = (f.k(f.a, 5), f.k(f.b, 10), f.k(f.c, 8), f.k(f.a, 0));
    // Un clic : A@5 seule, et c'est la clé de départ.
    f.app.click_key(Some(a5), false, false);
    assert_eq!(f.app.key_sel, vec![a5]);
    // Ctrl + clic sur un autre calque : elle s'ajoute ; encore : elle se retire.
    f.app.click_key(Some(c8), true, false);
    assert_eq!(key_counts(&f.app.key_sel), (2, 2));
    f.app.click_key(Some(c8), true, false);
    assert_eq!(f.app.key_sel, vec![a5]);
    // Maj + clic sur B@10 : le rectangle entre A@5 et B@10 : rangées A et B, images 5 à 10.
    f.app.click_key(Some(a5), false, false);
    f.app.click_key(Some(b10), false, true);
    let mut got = f.app.key_sel.clone();
    got.sort_by_key(|k| (k.layer, k.start));
    let mut want = vec![f.k(f.a, 5), f.k(f.a, 10), f.k(f.b, 5), f.k(f.b, 10)];
    want.sort_by_key(|k| (k.layer, k.start));
    assert_eq!(got, want, "deux calques, deux clés chacun");
    assert_eq!(f.app.key_anchor, Some(a5), "la clé de départ reste");
    // Le sens inverse donne le même rectangle (de C@8 vers A@0).
    f.app.click_key(Some(c8), false, false);
    f.app.click_key(Some(a0), false, true);
    assert_eq!(key_counts(&f.app.key_sel).1, 3, "les trois calques");
    assert!(f.app.key_sel.contains(&f.k(f.b, 5)) && f.app.key_sel.contains(&c8) && f.app.key_sel.contains(&a0));
    assert!(!f.app.key_sel.contains(&f.k(f.a, 10)), "après l'image 8 : hors du rectangle");
    assert!(f.app.status.contains("clés sur 3 calques"), "{}", f.app.status);
}

#[test]
fn dragging_chosen_keys_on_several_layers_ripples_each_layer_in_one_undo_step() {
    let mut f = fixture("glisser");
    let before = frames_of(&f.app);
    f.app.key_sel = vec![f.k(f.a, 5), f.k(f.b, 5)];
    assert!(f.app.begin_keys_drag(5));
    f.app.drag_keys_mode(7, crate::handles::KeyMove::Ripple);
    f.app.end_keys_drag();
    let after = frames_of(&f.app);
    assert_ne!(after, before);
    // A et B (rangs 2 et 1 dans la pile du bas vers le haut) : leurs clés 5 et 10 ont glissé de 2, C n'a pas bougé.
    let starts = |layer: &Vec<Option<u64>>| -> Vec<usize> { (0..layer.len()).filter(|&i| layer[i].is_some() && (i == 0 || layer[i - 1] != layer[i])).collect() };
    assert_eq!(starts(&after[2]), vec![0, 7, 12], "A : 5 et 10 deviennent 7 et 12");
    assert_eq!(starts(&after[1]), vec![0, 7, 12], "B aussi");
    assert_eq!(starts(&after[0]), vec![0, 8], "C ne bouge pas");
    assert_eq!(key_counts(&f.app.key_sel), (2, 2), "les clés déplacées restent choisies");
    // Une seule étape d'annulation défait tout.
    f.app.run_action("edition.annuler");
    assert_eq!(frames_of(&f.app), before);
}

#[test]
fn shift_drag_moves_only_the_chosen_keys_of_each_layer() {
    let mut f = fixture("maj");
    f.app.key_sel = vec![f.k(f.a, 5), f.k(f.b, 5)];
    assert!(f.app.begin_keys_drag(5));
    f.app.drag_keys_mode(6, crate::handles::KeyMove::Single);
    f.app.end_keys_drag();
    let starts = |layer: &Vec<Option<u64>>| -> Vec<usize> { (0..layer.len()).filter(|&i| layer[i].is_some() && (i == 0 || layer[i - 1] != layer[i])).collect() };
    let after = frames_of(&f.app);
    assert_eq!(starts(&after[2]), vec![0, 6, 10], "A : seule la clé 5 a bougé, la clé 10 reste");
    assert_eq!(starts(&after[1]), vec![0, 6, 10], "B aussi");
}

#[test]
fn copy_and_paste_keep_each_key_on_its_own_layer_relative_to_the_top_most_one() {
    let mut f = fixture("coller");
    let before = frames_of(&f.app);
    // On copie les clés 0 de A (haut) et de B (juste en dessous).
    f.app.key_sel = vec![f.k(f.a, 0), f.k(f.b, 0)];
    f.app.copy_keys();
    assert!(f.app.key_clipboard.is_some());
    // Calque courant : B (une rangée plus bas que A) ; la tête de lecture à l'image 2 : le bloc
    // s'insère après la clé de B qui l'a (0 à 5), donc à l'image 5.
    f.app.cur_layer = 1;
    f.app.cur_frame = 2;
    f.app.paste_keys();
    // A (le plus haut copié) va sur B, B va sur C, qui est en dessous : aucun calque créé, A ne bouge pas.
    assert_eq!(names_of(&f.app), vec!["A", "B", "C"]);
    assert!(f.app.status.contains("2 clés insérées sur 2 calques"), "{}", f.app.status);
    let after = frames_of(&f.app);
    assert_eq!(after[2], before[2], "A (le calque d'origine) n'est pas touché");
    for (name, li) in [("B", 1), ("C", 0)] {
        assert!(after[li][5].is_some() && after[li][5] != before[li][5], "{name} a reçu une clé à l'image 5");
        assert_eq!(after[li][..5], before[li][..5], "{name} : ce qui précède ne bouge pas");
    }
    // Chaque clé collée est sur son calque : le dessin de B en 5 est une copie de celui de A en 0.
    let copy_of = |li: usize, at: usize| f.app.doc.panels[0].layers[li].frames[at];
    assert_ne!(copy_of(1, 5), copy_of(0, 5), "B et C ont chacun leur copie");
    // Les clés collées sont choisies, et une seule annulation défait le collage.
    assert_eq!(key_counts(&f.app.key_sel), (2, 2));
    f.app.run_action("edition.annuler");
    assert_eq!(frames_of(&f.app), before);
}

#[test]
fn pasting_below_the_last_layer_creates_the_missing_layers_in_the_same_undo_step() {
    let mut f = fixture("creer");
    f.app.key_sel = vec![f.k(f.a, 0), f.k(f.b, 0)];
    f.app.copy_keys();
    // Calque courant : C, le dernier de la pile ; le second calque copié n'a pas de place dessous.
    f.app.cur_layer = 0;
    f.app.cur_frame = 15;
    f.app.paste_keys_at_head();
    assert_eq!(f.app.doc.panels[0].layers.len(), 4, "un calque créé au bas de la pile");
    assert_eq!(names_of(&f.app), vec!["A", "B", "C", "B"], "le nouveau reprend le nom du calque copié");
    assert_eq!(f.app.doc.panels[0].layers[f.app.cur_layer].name, "C", "le calque courant reste C");
    assert!(f.app.status.contains("créé"), "{}", f.app.status);
    f.app.run_action("edition.annuler");
    assert_eq!(f.app.doc.panels[0].layers.len(), 3, "l'annulation retire aussi le calque créé");
}

#[test]
fn delete_removes_the_chosen_keys_of_all_the_layers_in_one_step() {
    let mut f = fixture("suppr");
    let before = frames_of(&f.app);
    f.app.key_sel = vec![f.k(f.a, 5), f.k(f.b, 5), f.k(f.c, 8)];
    f.app.delete_keys(false);
    let after = frames_of(&f.app);
    assert_ne!(after, before);
    assert!(f.app.key_sel.is_empty());
    f.app.run_action("edition.annuler");
    assert_eq!(frames_of(&f.app), before, "une étape d'annulation pour les trois calques");
}
