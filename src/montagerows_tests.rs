//! Rangées de calques du montage : chaque plan montre ses calques dans son propre ordre (depuis
//! le 2026-10-17), toutes les rangées ; les poignées de la timeline sous la souris ; le collage
//! de clés sur plusieurs calques (relatif au calque courant, 2026-10-19).

use eframe::egui::pos2;

use crate::document::{Layer, LayerKind, SceneWide};
use crate::handles::Handle;
use crate::montagerows::{cell_handle, layer_in_row, LAYER_H};

/// Calques d'un plan, donnés du haut vers le bas (la pile est rangée du bas vers le haut).
fn plan(names: &[&str]) -> Vec<Layer> {
    names.iter().rev().map(|n| Layer::new(*n, LayerKind::Vector)).collect()
}

/// Les noms montrés à chaque rangée d'un plan, du haut vers le bas.
fn rows_of(layers: &[Layer], count: usize) -> Vec<Option<&str>> {
    (0..count).map(|r| layer_in_row(layers, r).map(|k| layers[k].name.as_str())).collect()
}

#[test]
fn each_plan_shows_its_layers_in_its_own_order() {
    // Anthony : « l'organisation et l'ordre des calques indépendants d'un shot à un autre ».
    let a = plan(&["Perso", "Décor"]);
    let b = plan(&["Effets", "Perso"]);
    let c = plan(&["Décor", "Ombre", "Perso"]);
    assert_eq!(rows_of(&a, 3), [Some("Perso"), Some("Décor"), None], "pas de trou : ses calques, puis plus rien");
    assert_eq!(rows_of(&b, 3), [Some("Effets"), Some("Perso"), None]);
    assert_eq!(rows_of(&c, 3), [Some("Décor"), Some("Ombre"), Some("Perso")], "son ordre à lui");
    // Un calque de toute la scène (fond) a sa rangée comme les autres.
    let mut d = plan(&["Perso", "Décor commun"]);
    d[0].scene_wide = Some(SceneWide { key: 1, scene: 1 });
    assert_eq!(rows_of(&d, 2), [Some("Perso"), Some("Décor commun")]);
    // Toutes les rangées, au-delà de 8.
    let many: Vec<String> = (1..=12).map(|i| format!("Calque {i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    assert_eq!(rows_of(&plan(&refs), 12).iter().flatten().count(), 12);
}

#[test]
fn a_pasted_layer_keeps_its_place_relative_to_the_top_most_copied_one() {
    use crate::montagekeys::target_rank;
    // Calques copiés aux rangs 1, 2 et 4 (depuis le haut) : le rang 1 va sur le calque courant.
    for cur in [0, 2, 5] {
        assert_eq!([1, 2, 4].map(|r| target_rank(cur, 1, r)), [cur, cur + 1, cur + 3], "même ordre, mêmes écarts, sous le calque courant");
    }
    // Un seul calque copié : le calque courant.
    assert_eq!(target_rank(3, 7, 7), 3);
}
/// Un calque dessiné de l'image 2 à 8 (exclue) : un dessin tenu de 2 à 5, un autre de 5 à 8.
fn drawn() -> Layer {
    let mut l = Layer::new("Perso", LayerKind::Vector);
    for f in 2..5 {
        l.set(f, Some(1));
    }
    for f in 5..8 {
        l.set(f, Some(2));
    }
    l
}

#[test]
fn the_timeline_handles_are_found_under_the_pointer() {
    let l = drawn();
    // 10 points par image, première image du plan en x = 100, piste en y = 50.
    let (x0, top, ppf, tol) = (100.0, 50.0, 10.0, 3.0);
    let at = |x: f32, y: f32| cell_handle(&l, 4, pos2(x, y), x0, top, ppf, tol);
    let mid = top + 6.0;
    let bar = top + LAYER_H - 4.0;
    assert_eq!(at(120.0, mid), Some(Handle::LayerStart { layer: 4 }));
    assert_eq!(at(181.0, mid), Some(Handle::LayerEnd { layer: 4 }));
    assert_eq!(at(150.0, mid), Some(Handle::Boundary { layer: 4, at: 5 }), "la limite entre les deux dessins");
    assert_eq!(at(135.0, bar), Some(Handle::LayerMove { layer: 4, grab: 3 }), "la barre, à l'image 3");
    assert_eq!(at(60.0, mid), None, "avant le calque");
    // Un dossier ou un calque vidéo n'a pas de poignées.
    let mut folder = drawn();
    folder.folder = true;
    assert_eq!(cell_handle(&folder, 4, pos2(120.0, mid), x0, top, ppf, tol), None);
}

#[test]
fn a_drawing_is_taken_by_its_middle_in_the_montage() {
    let l = drawn();
    let (x0, top, ppf, tol) = (100.0, 50.0, 10.0, 3.0);
    let mid = top + 6.0;
    // Au milieu du premier dessin (2..5) : on le prend pour le faire glisser.
    assert_eq!(cell_handle(&l, 4, pos2(135.0, mid), x0, top, ppf, tol), Some(Handle::Key { layer: 4, start: 2, end: 5, grab: 3 }));
    assert_eq!(cell_handle(&l, 4, pos2(165.0, mid), x0, top, ppf, tol), Some(Handle::Key { layer: 4, start: 5, end: 8, grab: 6 }));
    // Avant le calque, rien.
    assert_eq!(cell_handle(&l, 4, pos2(60.0, mid), x0, top, ppf, tol), None);
}

#[test]
fn keys_are_chosen_one_or_several_at_a_time() {
    use crate::montagekeys::{choose, KeyRef};
    let k = |start: usize| KeyRef { panel: 1, layer: 2, start };
    // Un clic : elle seule ; Ctrl + clic : ajoutée, puis retirée.
    let (sel, anchor) = choose(&[], None, Some(k(0)), false, None);
    assert_eq!((sel.clone(), anchor), (vec![k(0)], Some(k(0))));
    let (sel, _) = choose(&sel, anchor, Some(k(5)), true, None);
    assert_eq!(sel, vec![k(0), k(5)]);
    let (sel, _) = choose(&sel, anchor, Some(k(0)), true, None);
    assert_eq!(sel, vec![k(5)]);
    // Maj + clic : les clés d'entre (données par la piste), le départ reste.
    let (sel, a) = choose(&sel, Some(k(0)), Some(k(9)), false, Some(vec![k(0), k(5), k(9)]));
    assert_eq!((sel, a), (vec![k(0), k(5), k(9)], Some(k(0))));
    // À côté des dessins : sans touche, le choix se vide ; avec Ctrl, il reste.
    assert_eq!(choose(&[k(5)], None, None, true, None).0, vec![k(5)]);
    assert!(choose(&[k(5)], None, None, false, None).0.is_empty());
}

#[test]
fn a_new_key_becomes_the_only_choice() {
    use crate::handles::{key_at, key_start_taken};
    use crate::montagekeys::{choice_for_new_key, KeyRef};
    use crate::handles::runs;
    // Anthony : le bloc « Dessin » d'un plan de 10 images ; F5 à l'image 4 coupe l'exposition.
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.frames = vec![Some(1); 10];
    assert!(!key_start_taken(&l, 4));
    let end = key_at(&mut l, 4, 2, 10).expect("la clé est posée");
    // La nouvelle clé commence à l'image 4 et finit avec l'ancienne exposition.
    assert_eq!(runs(&l), vec![(0, 4, Some(1)), (4, 10, Some(2))]);
    assert_eq!(end, 10);
    // Dans le montage (storyboard) : elle seule est choisie, dans les pistes, et elle a la main.
    let c = choice_for_new_key(false, 7, 3, 4, end);
    assert_eq!(c.keys, vec![KeyRef { panel: 7, layer: 3, start: 4 }], "ni l'ancienne clé (début 0) ni le bloc entier");
    assert!(c.key_focus && !c.images_focus);
    assert_eq!(c.frames, None, "pas d'images cadrées dans le montage");
    // Dans la timeline (animation) : ses images sont cadrées, la timeline a la main.
    let c = choice_for_new_key(true, 7, 3, 4, end);
    assert_eq!(c.frames, Some((4, 10)));
    assert!(c.images_focus && !c.key_focus);
    assert_eq!(c.keys, vec![KeyRef { panel: 7, layer: 3, start: 4 }]);
    // Un nouveau dessin d'une image (Nouveau dessin, N) : une seule image cadrée.
    assert_eq!(choice_for_new_key(true, 1, 1, 12, 12).frames, Some((12, 13)), "au moins une image");
    // Un dessin posé sur une image vide, tenu jusqu'au suivant.
    let mut gap = Layer::new("Dessin", LayerKind::Vector);
    gap.frames = vec![Some(1), None, None, None, Some(5)];
    let end = key_at(&mut gap, 2, 9, 5).unwrap();
    assert_eq!((end, choice_for_new_key(true, 1, 1, 2, end).frames), (4, Some((2, 4))));
}

#[test]
fn keys_pasted_below_the_stack_create_the_missing_layers_at_the_bottom() {
    use crate::montagekeys::ensure_layers_below;
    // Trois calques copiés (rangs 0, 1, 2 de leur pile) ; le calque courant du plan d'arrivée est son
    // dernier (rang 0 sur une pile de 1) : il en manque deux, en dessous.
    let mut dst = plan(&["Décor"]);
    let specs = vec![
        (0, "Perso".to_string(), LayerKind::Vector),
        (1, "Ombre".to_string(), LayerKind::Bitmap),
        (2, "Fond".to_string(), LayerKind::Vector),
    ];
    assert_eq!(ensure_layers_below(&mut dst, &specs), 2);
    let names: Vec<&str> = dst.iter().rev().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Décor", "Ombre", "Fond"], "le calque courant garde sa place, les nouveaux sont dessous, dans l'ordre");
    assert_eq!(dst[1].kind, LayerKind::Bitmap);
    assert_eq!(ensure_layers_below(&mut dst, &specs), 0, "rien à créer une seconde fois");
    assert_eq!(ensure_layers_below(&mut dst, &[]), 0);
}

#[test]
fn keys_are_visited_one_after_the_other_across_all_layers() {
    use crate::app::Mode;
    use crate::keymap::{Keymap, DEFAULT_KEYMAP};
    use crate::montagekeys::{key_starts, step_key};
    use eframe::egui::{Key, Modifiers};
    // Anthony : « passer d'une clé à une autre, tous calques compris ».
    let mut a = Layer::new("Perso", LayerKind::Vector);
    a.frames = vec![Some(1), Some(1), Some(2), Some(2), None, Some(3)];
    let mut b = Layer::new("Décor", LayerKind::Vector);
    b.frames = vec![None, Some(7), Some(7), Some(7), Some(8)];
    let mut folder = Layer::new_folder("Dossier");
    folder.frames = vec![None, None, None, Some(9)];
    let layers = vec![a, b, folder];
    assert_eq!(key_starts(&layers, 100), [0, 1, 2, 4, 5], "les débuts de dessins de tous les calques, une fois chacun, sans les dossiers");
    assert_eq!(key_starts(&layers, 5), [0, 1, 2, 4], "rien après la fin du plan");
    let keys = key_starts(&layers, 100);
    assert_eq!(step_key(&keys, 1, true), Some(2));
    assert_eq!(step_key(&keys, 3, true), Some(4), "depuis le milieu d'une exposition");
    assert_eq!(step_key(&keys, 3, false), Some(2));
    assert_eq!(step_key(&keys, 5, true), None);
    assert_eq!(step_key(&keys, 0, false), None);
    // A / F, dans les deux modes (choix d'Anthony) ; plan précédent / suivant sur PgPréc / PgSuiv.
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    for mode in [Mode::Storyboard, Mode::Animation] {
        let press = |key| map.find(key, Some(key), Modifiers::NONE, mode).map(str::to_string);
        assert_eq!(press(Key::A).as_deref(), Some("cle.precedente"));
        assert_eq!(press(Key::F).as_deref(), Some("cle.suivante"));
        assert_eq!(press(Key::PageUp).as_deref(), Some("panneau.precedent"));
        assert_eq!(press(Key::PageDown).as_deref(), Some("panneau.suivant"));
    }
}