//! Dossiers de calques (13b-5) : ordre de la pile, effet sur l'image, déplacements, fusion,
//! duplication, enregistrement (format 14), lignes de la timeline.

use crate::document::{Document, Layer, LayerKind};
use crate::folders::{children_of, effect, folder_at, move_step, normalize, parent_index};
use crate::groups;
use crate::layerheads::rows_of;
use crate::project;

fn names(layers: &[Layer]) -> Vec<&str> {
    layers.iter().map(|l| l.name.as_str()).collect()
}

/// Du bas vers le haut : A, [b1, b2 dans F], F, C.
fn stack() -> Vec<Layer> {
    let mut f = Layer::new_folder("F");
    f.id = 100;
    let mut layers = vec![
        Layer::new("A", LayerKind::Bitmap),
        Layer::new("b1", LayerKind::Bitmap),
        Layer::new("b2", LayerKind::Bitmap),
        f,
        Layer::new("C", LayerKind::Bitmap),
    ];
    layers[1].parent = Some(100);
    layers[2].parent = Some(100);
    layers
}

#[test]
fn children_are_gathered_under_their_folder() {
    let mut layers = stack();
    // b1 égaré tout en haut, un dossier dans un dossier, un parent inconnu.
    let b1 = layers.remove(1);
    layers.push(b1);
    let mut g = Layer::new_folder("G");
    g.parent = Some(100);
    layers.insert(0, g);
    layers[1].parent = Some(999);
    normalize(&mut layers);
    assert_eq!(names(&layers), ["G", "A", "b2", "b1", "F", "C"]);
    assert!(layers.iter().all(|l| !l.folder || l.parent.is_none()), "un seul niveau");
    assert_eq!(layers[1].parent, None, "parent inconnu oublié");
    let fi = layers.iter().position(|l| l.name == "F").unwrap();
    assert_eq!(children_of(&layers, fi).len(), 2);
    assert_eq!(parent_index(&layers, fi - 1), Some(fi));
}

#[test]
fn a_folder_hides_and_fades_its_layers() {
    let mut layers = stack();
    layers[1].opacity = 0.5;
    layers[3].opacity = 0.5;
    assert_eq!(effect(&layers, 1), Some(0.25), "les opacités se multiplient");
    assert_eq!(effect(&layers, 0), Some(1.0), "hors dossier : rien ne change");
    assert_eq!(effect(&layers, 3), None, "le dossier lui-même n'a pas de dessin");
    layers[3].visible = false;
    assert_eq!(effect(&layers, 2), None, "dossier caché : ses calques aussi");
    assert_eq!(effect(&layers, 4), Some(1.0));
    layers[3].visible = true;
    layers[3].locked = true;
    assert!(crate::folders::locked_by_folder(&layers, 2));
}

#[test]
fn moving_one_step_respects_folders() {
    // Le plus haut calque du dossier qui monte en sort, au-dessus du dossier.
    let mut layers = stack();
    assert!(move_step(&mut layers, 2, true));
    assert_eq!(names(&layers), ["A", "b1", "F", "b2", "C"]);
    assert_eq!(layers[3].parent, None);

    // Le plus bas qui descend en sort, juste sous le dossier.
    let mut layers = stack();
    assert!(move_step(&mut layers, 1, false));
    assert_eq!(names(&layers), ["A", "b1", "b2", "F", "C"]);
    assert_eq!(layers[1].parent, None);
    assert_eq!(layers[2].parent, Some(100));

    // Le dossier descend avec ses calques, sous A.
    let mut layers = stack();
    assert!(move_step(&mut layers, 3, false));
    assert_eq!(names(&layers), ["b1", "b2", "F", "A", "C"]);

    // C descend par-dessus tout le dossier d'un seul cran.
    let mut layers = stack();
    assert!(move_step(&mut layers, 4, false));
    assert_eq!(names(&layers), ["A", "C", "b1", "b2", "F"]);
    assert!(!move_step(&mut layers, 4, true), "déjà tout en haut");
}

#[test]
fn a_dropped_layer_joins_the_folder_it_lands_in() {
    let layers = stack();
    // Une fois retiré de la pile, un calque posé au rang 2 tombe entre b1 et b2.
    let mut without_a = layers.clone();
    without_a.remove(0);
    assert_eq!(folder_at(&without_a, 1), Some(100), "entre b1 et b2");
    assert_eq!(folder_at(&without_a, 2), Some(100), "juste sous le dossier");
    assert_eq!(folder_at(&without_a, 0), None, "sous tout le dossier");
    assert_eq!(folder_at(&without_a, 4), None, "tout en haut");
}

#[test]
fn a_folder_merges_its_layers_and_duplicates_with_them() {
    let mut doc = Document::new(32, 32, 24);
    let mut layers = stack();
    for l in layers.iter_mut().filter(|l| !l.folder) {
        let cel = doc.new_cel(LayerKind::Bitmap);
        l.frames = vec![Some(cel)];
    }
    doc.panels[0].layers = layers;
    // Dupliquer le dossier : une copie du dossier et de ses calques, au-dessus.
    let at = crate::layerops::duplicate_layer(&mut doc, 0, 3);
    let l = &doc.panels[0].layers;
    assert_eq!(names(l), ["A", "b1", "b2", "F", "b1", "b2", "F copie", "C"]);
    assert_eq!(at, 6);
    assert_eq!(l[4].parent, Some(l[6].id));
    assert_ne!(l[4].cel_at(0), l[1].cel_at(0), "des dessins à eux");
    // Fusionner le dossier = fusionner ses calques ; le dossier reste.
    let merged = crate::layerops::merge_layers(&mut doc, 0, &[3]).unwrap();
    let l = &doc.panels[0].layers;
    assert_eq!(merged, 1);
    assert_eq!(names(l), ["A", "b1", "F", "b1", "b2", "F copie", "C"]);
    assert_eq!(l[1].parent, Some(100), "le calque fusionné reste dans le dossier");
}

#[test]
fn folders_are_saved() {
    let mut doc = Document::new(32, 32, 24);
    doc.panels[0].layers = stack();
    doc.panels[0].layers[3].opacity = 0.4;
    let dir = std::env::temp_dir().join(format!("pelure-test-dossiers-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("D.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert!(text.contains(&format!("\"version\": {}", project::VERSION)));
    let l = &back.panels[0].layers;
    assert_eq!(names(l), ["A", "b1", "b2", "F", "C"]);
    assert!(l[3].folder);
    assert_eq!(l[3].opacity, 0.4);
    assert_eq!(l[1].parent, Some(l[3].id), "le dossier est retrouvé par son rang");
    assert_eq!(l[2].parent, Some(l[3].id));
    assert_eq!(l[0].parent, None);
}

#[test]
fn a_duplicated_plan_keeps_its_folders_and_instance_names() {
    let mut doc = Document::new(32, 32, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut layers = stack();
    layers[1].frames = vec![Some(cel)];
    layers[1].names.insert(cel, "marche".into());
    doc.panels[0].layers = layers;
    let copy = doc.duplicate_panel(0);
    let l = &copy.layers;
    assert_eq!(l[1].parent, Some(l[3].id), "le calque rangé suit le dossier copié");
    assert_ne!(l[3].id, 100);
    let new_cel = l[1].frames[0].unwrap();
    assert_ne!(new_cel, cel);
    assert_eq!(l[1].names.get(&new_cel).map(String::as_str), Some("marche"), "le nom suit le dessin copié");
}

#[test]
fn a_closed_folder_hides_the_rows_of_its_layers() {
    let mut layers = stack();
    let rows: Vec<usize> = rows_of(&layers, &groups::defaults()).iter().map(|r| r.li).collect();
    assert_eq!(rows, [4, 3, 2, 1, 0]);
    layers[3].collapsed = true;
    let rows: Vec<usize> = rows_of(&layers, &groups::defaults()).iter().map(|r| r.li).collect();
    assert_eq!(rows, [4, 3, 0]);
}
