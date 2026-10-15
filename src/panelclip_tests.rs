//! Tests du copier-coller de panneaux (12e) : les panneaux collés sont des copies (leurs dessins
//! sont à eux), posés juste après le panneau visé, dans sa scène ; ce qui a été copié ne change
//! plus, même si on modifie ou supprime l'original.

use eframe::egui::Color32;

use crate::document::{CelId, Document, Layer, LayerKind};

/// Scène 1 : panneaux 0 et 1 ; scène 2 : panneau 2. Le panneau 1 a un dessin rouge (calque
/// « Trait ») et une durée de 10 images.
fn doc() -> (Document, CelId) {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_panel_after(0);
    doc.add_scene_after(1);
    let cel = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&cel).unwrap().raster.tile_mut(0)[0] = Color32::RED;
    let mut trait_ = Layer::new("Trait", LayerKind::Bitmap);
    trait_.set(0, Some(cel));
    doc.panels[1].layers.push(trait_);
    doc.panels[1].set_length(10);
    (doc, cel)
}

/// Le dessin du calque « Trait » d'un panneau, et sa couleur en haut à gauche.
fn drawing(doc: &Document, panel: usize) -> Option<(CelId, Color32)> {
    let l = doc.panels[panel].layers.iter().find(|l| l.name == "Trait")?;
    let id = l.cel_at(0)?;
    Some((id, doc.cels[&id].raster.get(0, 0)))
}

#[test]
fn pasted_panels_are_copies_after_the_target() {
    let (mut doc, cel) = doc();
    let (id1, scene1, scene2) = (doc.panels[1].id, doc.panels[1].scene, doc.panels[2].scene);
    let clip = doc.copy_panels(&[id1]).expect("un panneau copié");
    // Collé après le panneau 2 (scène 2) : il devient le panneau 3, de la scène 2.
    let at = doc.paste_panels(&clip, 2).expect("même taille de projet");
    assert_eq!(at, 3);
    assert_eq!(doc.panels.len(), 4);
    assert_eq!(doc.panels[3].scene, scene2, "dans la scène du panneau visé");
    assert_eq!(doc.panels[1].scene, scene1, "l'original ne bouge pas");
    assert_ne!(doc.panels[3].id, id1, "un nouveau panneau");
    assert_eq!(doc.panels[3].frame_count(), 10, "même durée");
    let (pasted, color) = drawing(&doc, 3).expect("le dessin est collé");
    assert_ne!(pasted, cel, "son propre dessin");
    assert_eq!(color, Color32::RED);
    // Modifier la copie ne touche pas l'original.
    doc.cels.get_mut(&pasted).unwrap().raster.tile_mut(0)[0] = Color32::BLUE;
    assert_eq!(drawing(&doc, 1).unwrap().1, Color32::RED);
}

#[test]
fn what_was_copied_does_not_change() {
    let (mut doc, cel) = doc();
    let clip = doc.copy_panels(&[doc.panels[1].id]).unwrap();
    // L'original est repeint puis supprimé : la copie garde le dessin du moment de la copie.
    doc.cels.get_mut(&cel).unwrap().raster.tile_mut(0)[0] = Color32::GREEN;
    let id1 = doc.panels[1].id;
    doc.remove_panels(&[id1]).expect("panneau supprimé");
    let at = doc.paste_panels(&clip, 0).unwrap();
    assert_eq!(drawing(&doc, at).unwrap().1, Color32::RED);
    // On peut coller plusieurs fois : chaque fois de nouveaux dessins.
    let again = doc.paste_panels(&clip, at).unwrap();
    assert_ne!(drawing(&doc, again).unwrap().0, drawing(&doc, at).unwrap().0);
}

#[test]
fn several_panels_keep_their_order() {
    let (mut doc, _) = doc();
    let ids = [doc.panels[2].id, doc.panels[0].id];
    let clip = doc.copy_panels(&ids).unwrap();
    assert_eq!(clip.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[1], ids[0]], "dans l'ordre du film");
    let n = doc.panels.len();
    let at = doc.paste_panels(&clip, 0).unwrap();
    assert_eq!((at, doc.panels.len()), (1, n + 2), "juste après le panneau 0");
    let scene = doc.panels[0].scene;
    assert!(doc.panels[1..3].iter().all(|p| p.scene == scene), "tous dans la scène du panneau visé");
    assert!(doc.copy_panels(&[987_654]).is_none(), "rien à copier");
}

#[test]
fn a_copy_from_another_project_size_is_refused() {
    let (doc_a, _) = doc();
    let clip = doc_a.copy_panels(&[doc_a.panels[1].id]).unwrap();
    let mut other = Document::with_frame(64, 48, 24);
    let n = other.panels.len();
    assert!(other.paste_panels(&clip, 0).is_none());
    assert_eq!(other.panels.len(), n);
}

#[test]
fn a_scene_background_stays_in_its_scene() {
    let (mut doc, _) = doc();
    // Fond de toute la scène 1, peint depuis le panneau 0.
    let fond_cel = doc.new_cel(LayerKind::Bitmap);
    let mut fond = Layer::new("Fond", LayerKind::Bitmap);
    fond.set(0, Some(fond_cel));
    doc.panels[0].layers.insert(0, fond);
    doc.set_scene_wide(0, 0, 0, true);
    let w = doc.panels[0].layers[0].scene_wide.expect("fond étendu");
    let clip = doc.copy_panels(&[doc.panels[1].id]).unwrap();
    // Collé dans la scène 1 : il garde le fond de la scène (le même dessin).
    let same = doc.paste_panels(&clip, 0).unwrap();
    doc.sync_scene_layers(same, 0);
    let l = doc.panels[same].layers.iter().find(|l| l.scene_wide == Some(w)).expect("fond de la scène");
    assert_eq!(l.cel_at(0), Some(fond_cel));
    // Collé dans la scène 2 : il ne l'emporte pas.
    let last = doc.panels.len() - 1;
    let other = doc.paste_panels(&clip, last).unwrap();
    doc.sync_scene_layers(other, 0);
    assert!(doc.panels[other].layers.iter().all(|l| l.scene_wide != Some(w)));
    assert!(drawing(&doc, other).is_some(), "ses propres calques restent");
}
