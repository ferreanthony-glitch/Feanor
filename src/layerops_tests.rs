//! Opérations sur plusieurs calques (13b-3) : fusion des choisis, duplication, calques vides,
//! choix par groupe, raccourcis repris de TVPaint.

use eframe::egui::{Color32, Key, Modifiers};

use crate::app::Mode;
use crate::document::{Document, Layer, LayerKind};
use crate::keymap::{Keymap, DEFAULT_KEYMAP};
use crate::layerops::{choose, duplicate_layer, is_empty_layer, merge_layers, Choose};

/// Plan de 4 calques bitmap d'une image : C0 (le plus bas) à C3 ; C0 a un point rouge en
/// (1, 1), C2 un point bleu en (5, 5), C1 et C3 sont vides.
fn plan() -> Document {
    let mut doc = Document::new(64, 64, 24);
    let mut layers = Vec::new();
    for i in 0..4 {
        let mut l = Layer::new(format!("C{i}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        let at = match i {
            0 => Some((1, Color32::from_rgb(255, 0, 0))),
            2 => Some((5, Color32::from_rgb(0, 0, 255))),
            _ => None,
        };
        if let Some((p, c)) = at {
            let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
            let idx = r.tile_index_of(p, p);
            r.tile_mut(idx)[p * crate::raster::TILE + p] = c;
        }
        l.frames = vec![Some(cel)];
        layers.push(l);
    }
    doc.panels[0].layers = layers;
    doc
}

#[test]
fn chosen_layers_merge_into_the_lowest_one() {
    let mut doc = plan();
    let at = merge_layers(&mut doc, 0, &[2, 0]).unwrap();
    assert_eq!(at, 0);
    let names: Vec<_> = doc.panels[0].layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["C0", "C1", "C3"], "C2 fondu dans C0, C1 passe au-dessus");
    let cel = doc.panels[0].layers[0].cel_at(0).unwrap();
    let r = &doc.cels[&cel].raster;
    assert_eq!(r.get(1, 1).r(), 255);
    assert_eq!(r.get(5, 5).b(), 255, "les deux dessins dans le calque fusionné");
}

#[test]
fn a_refused_merge_changes_nothing() {
    let mut doc = plan();
    doc.panels[0].layers[3].visible = false;
    let before: Vec<u64> = doc.panels[0].layers.iter().map(|l| l.id).collect();
    assert!(merge_layers(&mut doc, 0, &[0, 1, 2, 3]).is_err());
    let after: Vec<u64> = doc.panels[0].layers.iter().map(|l| l.id).collect();
    assert_eq!(before, after);
    assert!(merge_layers(&mut doc, 0, &[1]).is_err(), "il en faut deux");
}

#[test]
fn a_duplicated_layer_has_its_own_drawings() {
    let mut doc = plan();
    let at = duplicate_layer(&mut doc, 0, 0);
    assert_eq!(at, 1);
    let (a, b) = (&doc.panels[0].layers[0], &doc.panels[0].layers[1]);
    assert_eq!(b.name, "C0 copie");
    assert_ne!(a.id, b.id);
    assert_ne!(a.cel_at(0), b.cel_at(0), "un dessin à elle");
    assert_eq!(doc.cels[&b.cel_at(0).unwrap()].raster.get(1, 1).r(), 255);
}

#[test]
fn empty_layers_and_choices() {
    let mut doc = plan();
    let empty: Vec<bool> = doc.panels[0].layers.iter().map(|l| is_empty_layer(&doc, l)).collect();
    assert_eq!(empty, [false, true, false, true]);
    let layers = &mut doc.panels[0].layers;
    layers[1].group = 3;
    layers[3].group = 3;
    layers[2].visible = false;
    let ids = |v: &[usize]| v.iter().map(|&i| layers[i].id).collect::<Vec<_>>();
    assert_eq!(choose(layers, 1, Choose::SameGroup), ids(&[1, 3]));
    assert_eq!(choose(layers, 1, Choose::OtherGroups), ids(&[0, 2]));
    assert_eq!(choose(layers, 1, Choose::Visible), ids(&[0, 1, 3]));
    assert_eq!(choose(layers, 1, Choose::All).len(), 4);
    assert!(choose(layers, 1, Choose::Nothing).is_empty());
}

#[test]
fn a_new_layer_goes_where_it_is_asked() {
    use crate::newlayer::{insert_index, Place};
    let mut doc = plan();
    let layers = &mut doc.panels[0].layers;
    // Groupe 2 : C1 et C2 ; le calque courant est C1.
    layers[1].group = 2;
    layers[2].group = 2;
    assert_eq!(insert_index(layers, 1, Place::AboveCurrent), 2);
    assert_eq!(insert_index(layers, 1, Place::BelowCurrent), 1);
    assert_eq!(insert_index(layers, 1, Place::Top), 4);
    assert_eq!(insert_index(layers, 1, Place::Bottom), 0);
    assert_eq!(insert_index(layers, 1, Place::AboveGroup), 3, "au-dessus de C2, le plus haut du groupe");
    assert_eq!(insert_index(layers, 1, Place::BelowGroup), 1, "sous C1, le plus bas du groupe");
}

#[test]
fn tvpaint_layer_keys_are_in_place() {
    let map = Keymap::from_texts(DEFAULT_KEYMAP, None);
    assert!(map.warnings.is_empty(), "{:#?}", map.warnings);
    let press = |key, m| map.find(key, Some(key), m, Mode::Animation).map(str::to_string);
    let ctrl = Modifiers::COMMAND;
    assert_eq!(press(Key::ArrowUp, ctrl).as_deref(), Some("calque.monter"));
    assert_eq!(press(Key::ArrowDown, ctrl).as_deref(), Some("calque.descendre"));
    assert_eq!(press(Key::ArrowUp, Modifiers::SHIFT).as_deref(), Some("calque.choisir_dessus"));
    assert_eq!(press(Key::ArrowDown, Modifiers::SHIFT).as_deref(), Some("calque.choisir_dessous"));
    assert_eq!(press(Key::ArrowUp, Modifiers::NONE).as_deref(), Some("calque.dessus"));
    // Touches TVPaint déjà prises dans Pelure : elles gardent leur fonction.
    assert_eq!(press(Key::N, ctrl).as_deref(), Some("fichier.nouveau"));
    assert_eq!(press(Key::End, ctrl).as_deref(), Some("montage.fin"));
}
