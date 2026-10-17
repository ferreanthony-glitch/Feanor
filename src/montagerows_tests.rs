//! Rangées de calques du montage : chaque plan montre ses calques dans son propre ordre (depuis
//! le 2026-10-17), toutes les rangées ; les poignées de la timeline sous la souris ; la piste
//! d'un calque (son nom) sert au collage sur plusieurs calques.

use eframe::egui::pos2;

use crate::document::{Layer, LayerKind, SceneWide};
use crate::handles::Handle;
use crate::montagerows::{cell_handle, layer_in_row, layer_on_track, TrackKey, LAYER_H};

/// Calques d'un plan, donnés du haut vers le bas (la pile est rangée du bas vers le haut).
fn plan(names: &[&str]) -> Vec<Layer> {
    names.iter().rev().map(|n| Layer::new(*n, LayerKind::Vector)).collect()
}

fn key(name: &str, nth: usize) -> TrackKey {
    TrackKey { name: name.into(), nth }
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
fn a_layer_track_is_its_name_for_pasting() {
    // Le collage sur plusieurs calques retrouve un calque par son nom, où qu'il soit dans la pile.
    let a = plan(&["Perso", "Décor"]);
    let b = plan(&["Effets", "Perso"]);
    assert_eq!(layer_on_track(&a, &key("Perso", 0)), Some(1));
    assert_eq!(layer_on_track(&b, &key("Perso", 0)), Some(0));
    assert_eq!(layer_on_track(&b, &key("Décor", 0)), None);
    let twice = plan(&["Trait", "Trait"]);
    assert_eq!(layer_on_track(&twice, &key("Trait", 1)), Some(0), "le deuxième « Trait » en partant du haut");
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
fn keys_pasted_from_several_layers_find_or_create_their_layers() {
    use crate::montagekeys::ensure_tracks;
    use crate::montagerows::track_of;
    // La pile d'origine, du haut vers le bas : Perso, Ombre, Décor. Le plan d'arrivée n'a que Décor.
    let src = plan(&["Perso", "Ombre", "Décor"]);
    let wanted: Vec<(TrackKey, LayerKind, usize)> = (0..3).map(|li| (track_of(&src, li), LayerKind::Vector, src.len() - 1 - li)).collect();
    let mut dst = plan(&["Décor"]);
    assert_eq!(ensure_tracks(&mut dst, &wanted), 2);
    let names: Vec<&str> = dst.iter().rev().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Perso", "Ombre", "Décor"], "les calques créés gardent l'ordre d'origine, au-dessus");
    assert_eq!(ensure_tracks(&mut dst, &wanted), 0, "rien à créer une seconde fois");
    // La piste d'un calque : son nom et son rang parmi ceux du même nom, en partant du haut.
    let twice = plan(&["Trait", "Trait"]);
    assert_eq!((track_of(&twice, 1), track_of(&twice, 0)), (key("Trait", 0), key("Trait", 1)));
}