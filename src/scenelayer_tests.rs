//! Tests des calques de toute la scène (6f) : un exemplaire par panneau de la scène, le même
//! dessin partout, et des scènes qui restent séparées quand on les coupe, copie ou réunit.

use eframe::egui::Color32;

use crate::document::{CelId, Document, Layer, LayerKind, SceneWide};
use crate::project;

/// Scène 1 de trois panneaux, scène 2 d'un panneau ; un fond peint, étendu à la scène 1 depuis
/// le panneau 0.
fn doc_with_background() -> (Document, SceneWide, CelId) {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    doc.add_scene_after(2);
    assert_eq!(doc.panels.len(), 4);
    let cel = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&cel).unwrap().raster.tile_mut(0)[0] = Color32::RED;
    let mut fond = Layer::new("Fond", LayerKind::Bitmap);
    fond.set(0, Some(cel));
    doc.panels[0].layers.insert(0, fond);
    doc.set_scene_wide(0, 0, 0, true);
    let w = doc.panels[0].layers[0].scene_wide.expect("fond étendu");
    (doc, w, cel)
}

fn copy(doc: &Document, panel: usize, w: SceneWide) -> Option<&Layer> {
    doc.panels[panel].layers.iter().find(|l| l.scene_wide == Some(w))
}

#[test]
fn a_background_spans_its_scene() {
    let (mut doc, w, cel) = doc_with_background();
    for p in 0..3 {
        let l = copy(&doc, p, w).expect("un exemplaire par panneau");
        assert_eq!(l.name, "Fond");
        assert_eq!(doc.panels[p].layers[0].scene_wide, Some(w), "tout en bas");
        assert_eq!(l.frames, vec![Some(cel); doc.panels[p].frame_count()], "le même dessin pendant tout le panneau");
    }
    assert!(copy(&doc, 3, w).is_none(), "rien dans l'autre scène");
    assert_ne!(doc.panels[0].layers[0].id, doc.panels[1].layers[0].id, "chaque exemplaire a son id");
    // Un réglage changé dans le panneau courant passe aux autres, puis plus rien ne bouge.
    doc.panels[1].layers[0].opacity = 0.5;
    assert!(doc.sync_scene_layers(1, 0));
    assert!((0..3).all(|p| copy(&doc, p, w).unwrap().opacity == 0.5));
    assert!(!doc.sync_scene_layers(1, 0), "stable");
    // Une clé posée dans le panneau courant (à l'image 5) passe aux autres panneaux de la scène :
    // depuis le 2026-10-17, les clés du fond se posent et se déplacent dans le montage.
    let other = doc.new_cel(LayerKind::Bitmap);
    doc.panels[2].layers[0].set(5, Some(other));
    doc.sync_scene_layers(2, 5);
    assert!((0..3).all(|p| {
        let f = &copy(&doc, p, w).unwrap().frames;
        f[0] == Some(cel) && f[5] == Some(other)
    }));
    // Le fond suit la durée du panneau, sans l'allonger lui-même.
    doc.panels[1].set_length(60);
    doc.sync_scene_layers(1, 0);
    assert_eq!(copy(&doc, 1, w).unwrap().frames.len(), 60);
    doc.panels[1].set_length(10);
    doc.sync_scene_layers(1, 0);
    assert_eq!(doc.panels[1].frame_count(), 10);
    // Un nouveau panneau de la scène reçoit le sien.
    let i = doc.add_panel_after(0);
    doc.sync_scene_layers(i, 0);
    assert_eq!(copy(&doc, i, w).unwrap().frames[..6], [Some(cel), Some(cel), Some(cel), Some(cel), Some(cel), Some(other)]);
}

#[test]
fn scene_changes_keep_backgrounds_apart() {
    let (mut doc, w, cel) = doc_with_background();
    // Couper la scène au panneau 1 : la nouvelle scène a son propre fond, sur une copie du dessin.
    let i = doc.split_scene(1).unwrap();
    doc.sync_scene_layers(i, 0);
    assert!(copy(&doc, 0, w).is_some());
    assert!(copy(&doc, 1, w).is_none());
    let w2 = doc.panels[1].layers[0].scene_wide.expect("fond de la nouvelle scène");
    assert_ne!(w2.key, w.key);
    assert_eq!(w2.scene, doc.panels[1].scene);
    let cel2 = doc.panels[1].layers[0].frames[0].unwrap();
    assert_ne!(cel2, cel, "sa propre copie du dessin");
    assert_eq!(doc.panels[2].layers[0].frames[0], Some(cel2));
    // Réunir les scènes : les panneaux rejoints prennent le fond de la scène qui les accueille.
    let j = doc.join_previous_scene(1).unwrap();
    doc.sync_scene_layers(j, 0);
    for p in 0..3 {
        assert_eq!(copy(&doc, p, w).unwrap().frames[0], Some(cel));
        assert!(doc.panels[p].layers.iter().all(|l| l.scene_wide != Some(w2)));
    }
    // Un panneau posé dans l'autre scène perd le fond de la sienne.
    let k = doc.move_panel(0, 4).unwrap();
    doc.sync_scene_layers(k, 0);
    assert_ne!(doc.panels[k].scene, doc.panels[0].scene);
    assert!(doc.panels[k].layers.iter().all(|l| l.scene_wide.is_none()));
    assert!(!doc.panels[k].layers.is_empty());
    assert!(copy(&doc, 0, w).is_some());
}

#[test]
fn copies_and_unlinking() {
    let (mut doc, w, cel) = doc_with_background();
    // Copier la scène : la copie a son propre fond ; l'original ne change pas.
    let i = doc.duplicate_scene_after(0);
    doc.sync_scene_layers(i, 0);
    let wc = doc.panels[i].layers[0].scene_wide.unwrap();
    assert_ne!(wc, w);
    assert_ne!(doc.panels[i].layers[0].frames[0], Some(cel));
    assert!((0..3).all(|p| copy(&doc, p, w).unwrap().frames[0] == Some(cel)));
    // Copier un panneau dans la scène : même fond, même dessin.
    let d = doc.duplicate_panel_after(0);
    doc.sync_scene_layers(d, 0);
    assert_eq!(copy(&doc, d, w).unwrap().frames[0], Some(cel));
    // Rendre le calque au seul panneau 0 : les autres gardent chacun leur copie du dessin.
    doc.set_scene_wide(0, 0, 0, false);
    doc.sync_scene_layers(0, 0);
    assert_eq!(doc.panels[0].layers[0].frames[0], Some(cel));
    assert!(doc.panels[0].layers[0].scene_wide.is_none());
    let c1 = doc.panels[1].layers[0].frames[0].unwrap();
    assert_ne!(c1, cel);
    assert!(doc.panels[1].layers[0].scene_wide.is_none());
}

#[test]
fn removing_and_merging() {
    let (mut doc, w, _) = doc_with_background();
    assert!(doc.merge_layer_down(0, 1).is_err(), "le fond ne se fusionne pas");
    // Réunir deux panneaux : leurs calques propres se suivent, le fond reste unique.
    doc.merge_with_next(0).unwrap();
    doc.sync_scene_layers(0, 0);
    assert_eq!(doc.panels[0].layers.iter().filter(|l| l.scene_wide == Some(w)).count(), 1);
    assert_eq!(doc.panels[0].layers.len(), 2, "le fond et le dessin");
    doc.remove_scene_layer(w);
    assert!(doc.panels.iter().all(|p| !p.layers.is_empty() && p.layers.iter().all(|l| l.scene_wide.is_none())));
}

#[test]
fn backgrounds_are_saved() {
    let (doc, _, _) = doc_with_background();
    let dir = std::env::temp_dir().join(format!("pelure-test-fond-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let mut back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    // Relu en plans (13a) : la scène 1 est un seul plan ; son fond y devient un calque ordinaire,
    // le même dessin tenu sur tout le plan.
    assert!(back.is_plans());
    assert_eq!(back.panels.len(), 2);
    let fond = &back.panels[0].layers[0];
    assert!(fond.scene_wide.is_none());
    let cel = fond.frames[0];
    assert!(cel.is_some());
    assert_eq!(fond.frames.len(), back.panels[0].frame_count());
    assert!(fond.frames.iter().all(|f| *f == cel), "un seul dessin, tenu");
    assert!(back.panels[1].layers.iter().all(|l| l.scene_wide.is_none()));
    assert!(!back.sync_scene_layers(0, 0), "plus de calque de toute la scène");
}
