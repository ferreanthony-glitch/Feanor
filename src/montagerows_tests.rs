//! Pistes de calques du montage (chantier 5) : une piste par nom, toutes montrées, et les poignées
//! de la timeline sous la souris.

use eframe::egui::pos2;

use crate::document::{Layer, LayerKind, SceneWide};
use crate::handles::Handle;
use crate::montagerows::{cell_handle, layer_on_track, layer_tracks, TrackKey, LAYER_H};

/// Calques d'un plan, donnés du haut vers le bas (la pile est rangée du bas vers le haut).
fn plan(names: &[&str]) -> Vec<Layer> {
    names.iter().rev().map(|n| Layer::new(*n, LayerKind::Vector)).collect()
}

fn key(name: &str, nth: usize) -> TrackKey {
    TrackKey { name: name.into(), nth }
}

fn names(tracks: &[TrackKey]) -> Vec<String> {
    tracks.iter().map(TrackKey::label).collect()
}

#[test]
fn one_track_per_layer_name_whatever_its_place_in_each_plan() {
    let a = plan(&["Perso", "Décor"]);
    // Ici « Perso » est sous un calque nouveau, et « Décor » manque.
    let b = plan(&["Effets", "Perso"]);
    let c = plan(&["Perso", "Ombre", "Décor"]);
    let tracks = layer_tracks([a.as_slice(), b.as_slice(), c.as_slice()]);
    assert_eq!(names(&tracks), ["Effets", "Perso", "Ombre", "Décor"]);
    // Le calque de chaque plan sur la piste « Perso », quel que soit son rang.
    assert_eq!(layer_on_track(&a, &key("Perso", 0)), Some(1));
    assert_eq!(layer_on_track(&b, &key("Perso", 0)), Some(0));
    assert_eq!(layer_on_track(&c, &key("Perso", 0)), Some(2));
    // Un plan sans calque de ce nom : un trou dans la piste.
    assert_eq!(layer_on_track(&b, &key("Décor", 0)), None);
}

#[test]
fn every_layer_has_a_track_beyond_eight() {
    let many: Vec<String> = (1..=12).map(|i| format!("Calque {i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    let p = plan(&refs);
    assert_eq!(layer_tracks([p.as_slice()]).len(), 12, "avant le chantier 5 : 8 au plus");
}

#[test]
fn layers_of_the_same_name_in_one_plan_get_their_own_tracks() {
    let a = plan(&["Trait", "Trait", "Fond"]);
    let b = plan(&["Trait", "Fond"]);
    let tracks = layer_tracks([a.as_slice(), b.as_slice()]);
    assert_eq!(names(&tracks), ["Trait", "Trait (2)", "Fond"]);
    assert_eq!(layer_on_track(&a, &tracks[1]), Some(1), "le deuxième « Trait » en partant du haut");
    assert_eq!(layer_on_track(&b, &tracks[1]), None);
}

#[test]
fn scene_wide_layers_have_their_own_track() {
    // Depuis le 2026-10-17, un calque de toute la scène (fond) a sa piste comme les autres.
    let mut a = plan(&["Perso", "Décor commun"]);
    a[0].scene_wide = Some(SceneWide { key: 1, scene: 1 });
    let tracks = layer_tracks([a.as_slice()]);
    assert_eq!(names(&tracks), ["Perso", "Décor commun"]);
    assert_eq!(layer_on_track(&a, &key("Décor commun", 0)), Some(0));
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