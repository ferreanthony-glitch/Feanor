//! Tests des plans (13a) : un projet à panneaux devient un projet à plans (une scène = un plan,
//! ses panneaux bout à bout) sans rien perdre ni rien changer à l'écran ; puis les opérations
//! de plan (couper, réunir, déplacer, coller) gardent un plan par scène.

use eframe::egui::{pos2, Color32};

use crate::board::NoteStroke;
use crate::camera::{camera_at, Camera, CameraKey};
use crate::document::{CelId, Document, Layer, LayerKind};

fn cam(x: f32) -> Camera {
    Camera { x, y: 50.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 }
}

fn drawn(doc: &mut Document, color: Color32) -> CelId {
    let c = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&c).unwrap().raster.tile_mut(0)[0] = color;
    c
}

fn layer(name: &str, frames: &[(usize, CelId)]) -> Layer {
    let mut l = Layer::new(name, LayerKind::Bitmap);
    for &(f, c) in frames {
        l.set(f, Some(c));
    }
    l
}

/// Scène 1 : trois panneaux (10, 8 et 6 images) ; scène 2 : un panneau de 4 images.
/// - P1 : calque « Trait » (dessin rouge à 0), clés de caméra à 0 (x 0) et 5 (x 50) ; textes.
/// - P2 : « Trait » (dessin vert à 0), « Ombre » au-dessus (dessin bleu à 2), sans clé ; textes.
/// - P3 : « Trait » à 50 % d'opacité (réglé autrement), clé à 0 (x 100) ; textes.
fn old_project() -> (Document, [CelId; 4], [u64; 4]) {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    doc.add_scene_after(2);
    assert_eq!(doc.panels.len(), 4);
    let (red, green, blue, grey) = (
        drawn(&mut doc, Color32::RED),
        drawn(&mut doc, Color32::GREEN),
        drawn(&mut doc, Color32::BLUE),
        drawn(&mut doc, Color32::GRAY),
    );
    for (i, len) in [10, 8, 6, 4].into_iter().enumerate() {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(len);
        let texts = format!("action {}", i + 1);
        doc.panels[i].action = texts;
        doc.panels[i].dialogue = format!("dialogue {}", i + 1);
    }
    doc.panels[0].layers.push(layer("Trait", &[(0, red)]));
    doc.panels[0].camera = vec![CameraKey::new(0, cam(0.0)), CameraKey::new(5, cam(50.0))];
    doc.panels[1].layers.push(layer("Trait", &[(0, green)]));
    doc.panels[1].layers.push(layer("Ombre", &[(2, blue)]));
    let mut faint = layer("Trait", &[(0, grey)]);
    faint.opacity = 0.5;
    doc.panels[2].layers.push(faint);
    doc.panels[2].camera = vec![CameraKey::new(0, cam(100.0))];
    doc.panels[3].layers.push(layer("Trait", &[(0, grey)]));
    let ids = [doc.panels[0].id, doc.panels[1].id, doc.panels[2].id, doc.panels[3].id];
    (doc, [red, green, blue, grey], ids)
}

#[test]
fn a_scene_becomes_one_plan_with_its_panels_end_to_end() {
    let (mut doc, [red, green, blue, grey], ids) = old_project();
    let total = doc.total_frames();
    let warnings = doc.into_plans();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(doc.is_plans());
    assert_eq!(doc.panels.len(), 2, "deux plans");
    assert_eq!(doc.total_frames(), total, "le film garde sa durée (le son reste calé)");
    let plan = &doc.panels[0];
    assert_eq!(plan.id, ids[0], "le plan garde l'identité du premier panneau");
    assert_eq!(plan.frame_count(), 24);
    // Les débuts des panneaux 2 et 3 deviennent des marques d'image, avec leurs textes.
    let marks: Vec<(usize, &str, &str)> = plan.marks.iter().map(|m| (m.frame, m.action.as_str(), m.dialogue.as_str())).collect();
    assert_eq!(marks, vec![(10, "action 2", "dialogue 2"), (18, "action 3", "dialogue 3")]);
    assert_eq!((plan.action.as_str(), plan.dialogue.as_str()), ("action 1", "dialogue 1"), "le plan garde les textes du premier");
    // « Trait » de P1 et P2 (réglés pareil) : un seul calque ; celui de P3 (50 %) : un calque à lui.
    let traits: Vec<&Layer> = plan.layers.iter().filter(|l| l.name == "Trait").collect();
    assert_eq!(traits.len(), 2);
    let main = traits.iter().find(|l| l.opacity == 1.0).unwrap();
    assert_eq!(main.cel_at(0), Some(red));
    assert_eq!(main.cel_at(10), Some(green));
    let faint = traits.iter().find(|l| l.opacity == 0.5).unwrap();
    assert_eq!(faint.cel_at(18), Some(grey));
    assert_eq!(faint.cel_at(10), None, "rien avant son panneau");
    // « Ombre » s'ajoute juste au-dessus du calque où va le « Trait » de P2.
    let k_trait = plan.layers.iter().position(|l| l.id == main.id).unwrap();
    let k_ombre = plan.layers.iter().position(|l| l.name == "Ombre").unwrap();
    assert_eq!(k_ombre, k_trait + 1);
    assert_eq!(plan.layers[k_ombre].cel_at(12), Some(blue));
    // L'autre scène n'a pas bougé.
    assert_eq!(doc.panels[1].id, ids[3]);
    assert!(doc.panels[1].marks.is_empty());
}

#[test]
fn held_cameras_stay_held_across_the_joins() {
    let (mut doc, _, _) = old_project();
    let default = doc.default_camera();
    // Avant : P1 va de 0 à 50 puis tient 50 ; P2 (sans clé) garde 50 ; P3 est à 100.
    let before: Vec<f32> = (0..24)
        .map(|g| {
            let (i, f) = doc.locate(g);
            doc.panel_camera(i, f).x
        })
        .collect();
    doc.into_plans();
    let keys = &doc.panels[0].camera;
    let after: Vec<f32> = (0..24).map(|f| camera_at(keys, f, default).x).collect();
    for f in 0..24 {
        assert!((before[f] - after[f]).abs() < 1e-3, "image {f} : {} avant, {} après", before[f], after[f]);
    }
    assert_eq!(after[17], 50.0, "le plan fixe de P2 tient jusqu'à sa dernière image");
    assert_eq!(after[18], 100.0, "coupe nette au début de P3");
}

#[test]
fn notes_and_background_follow() {
    let (mut doc, _, ids) = old_project();
    // Une note attachée à P2 ; un fond de toute la scène 1.
    doc.board.strokes.push(NoteStroke { points: Vec::new(), color: [0, 0, 0], anchor: Some(ids[1]) });
    let fond_cel = doc.new_cel(LayerKind::Bitmap);
    doc.panels[0].layers.insert(0, layer("Fond", &[(0, fond_cel)]));
    doc.set_scene_wide(0, 0, 0, true);
    doc.sync_scene_layers(0, 0);
    doc.into_plans();
    let mark = doc.panels[0].marks[0].id;
    assert_eq!(doc.board.strokes[0].anchor, Some(mark), "la note suit l'image de storyboard de P2");
    let fond = doc.panels[0].layers.iter().find(|l| l.name == "Fond").expect("le fond reste");
    assert!(fond.scene_wide.is_none(), "un calque ordinaire");
    assert!((0..24).all(|f| fond.cel_at(f) == Some(fond_cel)), "son dessin tenu sur tout le plan");
}

#[test]
fn split_at_a_mark_and_join_again() {
    let (mut doc, [_, green, ..], _) = old_project();
    doc.into_plans();
    let mark = doc.panels[0].marks[0].clone();
    doc.board.texts.push(crate::board::NoteText { pos: pos2(0.0, 0.0), text: "n".into(), size: 1.0, color: [0; 3], anchor: Some(mark.id) });
    // Couper à la marque : elle redevient le début d'un plan, avec ses textes.
    let second = doc.split_plan(0, 10).expect("coupé");
    assert!(doc.is_plans());
    assert_eq!(doc.panels.len(), 3);
    let p = &doc.panels[second];
    assert_eq!((p.action.as_str(), p.frame_count()), ("action 2", 14));
    assert_eq!(p.marks.iter().map(|m| m.frame).collect::<Vec<_>>(), vec![8], "la marque de P3 suit, décalée");
    assert_eq!(doc.board.texts[0].anchor, Some(p.id), "la note suit le nouveau plan");
    assert_ne!(p.layers.iter().find_map(|l| l.cel_at(0)), Some(green), "ses propres copies des dessins");
    assert_eq!(doc.panels[0].frame_count(), 10);
    assert_eq!(doc.sequence_of(0), doc.sequence_of(second), "même séquence");
    // Réunir : la marque revient à l'image 10, avec les mêmes textes.
    doc.join_next_plan(0).expect("réunis");
    assert!(doc.is_plans());
    let plan = &doc.panels[0];
    assert_eq!(plan.frame_count(), 24);
    assert_eq!(plan.marks.iter().map(|m| (m.frame, m.action.clone())).collect::<Vec<_>>(), vec![(10, "action 2".to_string()), (18, "action 3".to_string())]);
    assert!(doc.split_plan(0, 0).is_none() && doc.split_plan(0, 24).is_none(), "pas de coupe au bord");
}

#[test]
fn moving_and_pasting_keep_one_plan_per_scene() {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    doc.add_sequence_after(2);
    doc.add_scene_after(3);
    assert!(doc.is_plans());
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let (s1, s2) = (doc.sequence_of(0), doc.sequence_of(3));
    // Le plan 0 posé devant le plan 2 : il reste dans sa séquence.
    doc.move_plans(&[ids[0]], Some(ids[2])).expect("déplacé");
    assert!(doc.is_plans());
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[1], ids[0], ids[2], ids[3], ids[4]]);
    assert_eq!(doc.sequence_of(1), s1);
    // Posé au milieu de la séquence 2 : il la rejoint.
    let at = doc.move_plans(&[ids[0]], Some(ids[4])).expect("déplacé");
    assert_eq!(doc.panels[at].id, ids[0]);
    assert_eq!(doc.sequence_of(at), s2);
    assert!(doc.is_plans());
    assert!(doc.move_plans(&[ids[0]], Some(ids[4])).is_none(), "déjà là : rien ne change");
    // Coller deux plans : chacun dans sa scène.
    let clip = doc.copy_panels(&[ids[1], ids[2]]).unwrap();
    let first = doc.paste_plans(&clip, 0).unwrap();
    assert_eq!(first, 1);
    assert!(doc.is_plans());
    assert_ne!(doc.panels[1].scene, doc.panels[2].scene);
    assert_eq!(doc.sequence_of(1), s1);
}

#[test]
fn a_second_3d_reference_is_reported() {
    let (mut doc, _, _) = old_project();
    let r = crate::document::Ref3d::new(1, crate::view3d::Cam3d::fit(([0.0; 3], [1.0; 3]), [32.0, 24.0]));
    doc.panels[0].ref3d = Some(r.clone());
    doc.panels[1].ref3d = Some(crate::document::Ref3d { opacity: 0.3, ..r.clone() });
    doc.panels[2].ref3d = Some(r.clone());
    let warnings = doc.into_plans();
    assert_eq!(warnings.len(), 1, "seul celui qui diffère est signalé : {warnings:?}");
    assert_eq!(doc.panels[0].ref3d, Some(r), "le plan garde le sien");
}
