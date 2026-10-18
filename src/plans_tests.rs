//! Tests des plans (13a) : un projet Ã  panneaux devient un projet Ã  plans (une scÃ¨ne = un plan,
//! ses panneaux bout Ã  bout) sans rien perdre ni rien changer Ã  l'Ã©cran ; puis les opÃ©rations
//! de plan (couper, rÃ©unir, dÃ©placer, coller) gardent un plan par scÃ¨ne.

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

/// ScÃ¨ne 1 : trois panneaux (10, 8 et 6 images) ; scÃ¨ne 2 : un panneau de 4 images.
/// - P1 : calque Â« Trait Â» (dessin rouge Ã  0), clÃ©s de camÃ©ra Ã  0 (x 0) et 5 (x 50) ; textes.
/// - P2 : Â« Trait Â» (dessin vert Ã  0), Â« Ombre Â» au-dessus (dessin bleu Ã  2), sans clÃ© ; textes.
/// - P3 : Â« Trait Â» Ã  50 % d'opacitÃ© (rÃ©glÃ© autrement), clÃ© Ã  0 (x 100) ; textes.
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
    assert_eq!(doc.total_frames(), total, "le film garde sa durÃ©e (le son reste calÃ©)");
    let plan = &doc.panels[0];
    assert_eq!(plan.id, ids[0], "le plan garde l'identitÃ© du premier panneau");
    assert_eq!(plan.frame_count(), 24);
    // Les dÃ©buts des panneaux 2 et 3 deviennent des marques d'image, avec leurs textes.
    let marks: Vec<(usize, &str, &str)> = plan.marks.iter().map(|m| (m.frame, m.action.as_str(), m.dialogue.as_str())).collect();
    assert_eq!(marks, vec![(10, "action 2", "dialogue 2"), (18, "action 3", "dialogue 3")]);
    assert_eq!((plan.action.as_str(), plan.dialogue.as_str()), ("action 1", "dialogue 1"), "le plan garde les textes du premier");
    // Â« Trait Â» de P1 et P2 (rÃ©glÃ©s pareil) : un seul calque ; celui de P3 (50 %) : un calque Ã  lui.
    let traits: Vec<&Layer> = plan.layers.iter().filter(|l| l.name == "Trait").collect();
    assert_eq!(traits.len(), 2);
    let main = traits.iter().find(|l| l.opacity == 1.0).unwrap();
    assert_eq!(main.cel_at(0), Some(red));
    assert_eq!(main.cel_at(10), Some(green));
    let faint = traits.iter().find(|l| l.opacity == 0.5).unwrap();
    assert_eq!(faint.cel_at(18), Some(grey));
    assert_eq!(faint.cel_at(10), None, "rien avant son panneau");
    // Â« Ombre Â» s'ajoute juste au-dessus du calque oÃ¹ va le Â« Trait Â» de P2.
    let k_trait = plan.layers.iter().position(|l| l.id == main.id).unwrap();
    let k_ombre = plan.layers.iter().position(|l| l.name == "Ombre").unwrap();
    assert_eq!(k_ombre, k_trait + 1);
    assert_eq!(plan.layers[k_ombre].cel_at(12), Some(blue));
    // L'autre scÃ¨ne n'a pas bougÃ©.
    assert_eq!(doc.panels[1].id, ids[3]);
    assert!(doc.panels[1].marks.is_empty());
}

#[test]
fn held_cameras_stay_held_across_the_joins() {
    let (mut doc, _, _) = old_project();
    let default = doc.default_camera();
    // Avant : P1 va de 0 Ã  50 puis tient 50 ; P2 (sans clÃ©) garde 50 ; P3 est Ã  100.
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
        assert!((before[f] - after[f]).abs() < 1e-3, "image {f} : {} avant, {} aprÃ¨s", before[f], after[f]);
    }
    assert_eq!(after[17], 50.0, "le plan fixe de P2 tient jusqu'Ã  sa derniÃ¨re image");
    assert_eq!(after[18], 100.0, "coupe nette au dÃ©but de P3");
}

#[test]
fn notes_and_background_follow() {
    let (mut doc, _, ids) = old_project();
    // Une note attachÃ©e Ã  P2 ; un fond de toute la scÃ¨ne 1.
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
    // Couper Ã  la marque : elle redevient le dÃ©but d'un plan, avec ses textes.
    let second = doc.split_plan(0, 10).expect("coupÃ©");
    assert!(doc.is_plans());
    assert_eq!(doc.panels.len(), 3);
    let p = &doc.panels[second];
    assert_eq!((p.action.as_str(), p.frame_count()), ("action 2", 14));
    assert_eq!(p.marks.iter().map(|m| m.frame).collect::<Vec<_>>(), vec![8], "la marque de P3 suit, dÃ©calÃ©e");
    assert_eq!(doc.board.texts[0].anchor, Some(p.id), "la note suit le nouveau plan");
    assert_ne!(p.layers.iter().find_map(|l| l.cel_at(0)), Some(green), "ses propres copies des dessins");
    assert_eq!(doc.panels[0].frame_count(), 10);
    assert_eq!(doc.sequence_of(0), doc.sequence_of(second), "mÃªme sÃ©quence");
    // RÃ©unir : la marque revient Ã  l'image 10, avec les mÃªmes textes.
    doc.join_next_plan(0).expect("rÃ©unis");
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
    // Le plan 0 posÃ© devant le plan 2 : il reste dans sa sÃ©quence.
    doc.move_plans(&[ids[0]], Some(ids[2])).expect("dÃ©placÃ©");
    assert!(doc.is_plans());
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[1], ids[0], ids[2], ids[3], ids[4]]);
    assert_eq!(doc.sequence_of(1), s1);
    // PosÃ© au milieu de la sÃ©quence 2 : il la rejoint.
    let at = doc.move_plans(&[ids[0]], Some(ids[4])).expect("dÃ©placÃ©");
    assert_eq!(doc.panels[at].id, ids[0]);
    assert_eq!(doc.sequence_of(at), s2);
    assert!(doc.is_plans());
    assert!(doc.move_plans(&[ids[0]], Some(ids[4])).is_none(), "dÃ©jÃ  lÃ  : rien ne change");
    // Coller deux plans : chacun dans sa scÃ¨ne.
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
    assert_eq!(warnings.len(), 1, "seul celui qui diffÃ¨re est signalÃ© : {warnings:?}");
    assert_eq!(doc.panels[0].ref3d, Some(r), "le plan garde le sien");
}

#[test]
fn chosen_plans_that_follow_each_other_are_joined_into_one() {
    // Anthony : Â« quand deux plans ou plus sont sÃ©lectionnÃ©s, l'option de fusionner les plans
    // doit Ãªtre possible Â».
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    doc.add_sequence_after(2);
    assert!(doc.is_plans());
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let lens: Vec<usize> = doc.panels.iter().map(|p| p.frame_count()).collect();
    doc.panels[0].action = "un".into();
    doc.panels[2].action = "trois".into();
    // Pas de trou, pas deux sÃ©quences, au moins deux plans.
    assert!(doc.join_plans(&[ids[0], ids[2]]).is_none(), "ils doivent se suivre");
    assert!(doc.join_plans(&[ids[2], ids[3]]).is_none(), "une seule sÃ©quence");
    assert!(doc.join_plans(&[ids[1]]).is_none(), "au moins deux");
    // Les trois de la premiÃ¨re sÃ©quence (dans le dÃ©sordre) : un seul plan, le premier, sans
    // repÃ¨re aux jointures (Anthony : Â« fusionner les deux plans laisse un repÃ¨re sur la
    // timeline, tu peux les enlever ? Â») ; les textes des plans fusionnÃ©s le rejoignent.
    let at = doc.join_plans(&[ids[2], ids[0], ids[1]]).expect("rÃ©unis");
    assert!(doc.is_plans());
    assert_eq!(doc.panels.len(), 2);
    let plan = &doc.panels[at];
    assert_eq!(plan.id, ids[0]);
    assert_eq!(plan.frame_count(), lens[0] + lens[1] + lens[2]);
    assert!(plan.marks.is_empty(), "pas de marque d'image aux jointures");
    assert_eq!(plan.action, "un\ntrois");
}
#[test]
fn joining_a_plan_with_the_next_leaves_no_mark() {
    // Anthony : Â« RÃ©unir le plan et le suivant Â» ne garde pas de marque non plus.
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    let lens: Vec<usize> = doc.panels.iter().map(|p| p.frame_count()).collect();
    doc.panels[1].dialogue = "rÃ©plique".into();
    let at = doc.join_next_plan_without_mark(0).expect("rÃ©unis");
    let plan = &doc.panels[at];
    assert_eq!(plan.frame_count(), lens[0] + lens[1]);
    assert!(plan.marks.is_empty());
    assert_eq!(plan.dialogue, "rÃ©plique", "les textes du plan rÃ©uni sont gardÃ©s");
}
// ------------------------------------------------------------ collage et enregistrement (perte de plan ?)

fn round_trip(doc: &Document, name: &str) -> Document {
    let (doc, warnings) = round_trip_with_warnings(doc, name);
    assert!(warnings.is_empty(), "{warnings:?}");
    doc
}

fn round_trip_lenient(doc: &Document, name: &str) -> Document {
    round_trip_with_warnings(doc, name).0
}

fn round_trip_with_warnings(doc: &Document, name: &str) -> (Document, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("pelure-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    crate::project::save(doc, &manifest, &mut Default::default()).unwrap();
    let loaded = crate::project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    (loaded.doc, loaded.warnings)
}

/// Ce qui doit survivre Ã  l'enregistrement : pour chaque plan, son action, sa scÃ¨ne (par rang) et
/// la couleur de son dessin.
fn summary(doc: &Document) -> Vec<(String, usize, Option<Color32>)> {
    doc.panels
        .iter()
        .map(|p| {
            let color = p.layers.iter().find_map(|l| l.cel_at(0)).and_then(|c| doc.cels.get(&c)).map(|c| c.raster.get(0, 0));
            (p.action.clone(), doc.scene_number(p.scene), color)
        })
        .collect()
}

fn three_plans() -> Document {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (i, color) in [Color32::RED, Color32::GREEN, Color32::BLUE].into_iter().enumerate() {
        let c = drawn(&mut doc, color);
        doc.panels[i].layers.clear();
        doc.panels[i].layers.push(layer("Trait", &[(0, c)]));
        doc.panels[i].action = format!("plan {}", i + 1);
    }
    doc
}

#[test]
fn pasted_plans_survive_a_save_in_the_same_sequence() {
    for (copied, at) in [(vec![1], 1), (vec![0, 1], 1), (vec![0, 1, 2], 2), (vec![2], 0)] {
        let mut doc = three_plans();
        let ids: Vec<u64> = copied.iter().map(|&i| doc.panels[i].id).collect();
        let clip = doc.copy_panels(&ids).unwrap();
        doc.paste_plans(&clip, at).unwrap();
        // CollÃ© deux fois, comme un second Ctrl+V.
        doc.paste_plans(&clip, at).unwrap();
        assert_eq!(doc.panels.len(), 3 + 2 * copied.len());
        assert!(doc.is_plans());
        let mut panel_ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
        panel_ids.sort();
        panel_ids.dedup();
        assert_eq!(panel_ids.len(), doc.panels.len(), "identifiants de plan uniques");
        let back = round_trip(&doc, "collage");
        assert_eq!(summary(&back), summary(&doc), "copiÃ©s {copied:?} collÃ©s aprÃ¨s {at}");
    }
}

#[test]
fn raw_panels_pasted_in_a_scene_are_joined_as_marks_on_reload_not_lost() {
    // `paste_panels` (sans « s ») laisse deux panneaux dans la même scène : ce n'est pas un collage
    // de plans (l'application passe par `paste_plans`). À la relecture, le projet est converti en
    // plans, panneaux bout à bout avec une marque d'image à chaque début : rien ne disparaît,
    // mais le nombre de plans change. C'était l'écart « 5 à l'écriture, 4 à la relecture ».
    let mut doc = three_plans();
    let clip = doc.copy_panels(&[doc.panels[0].id]).unwrap();
    doc.paste_panels(&clip, 0).unwrap();
    assert_eq!(doc.panels.len(), 4);
    assert!(!doc.is_plans());
    let frames: usize = doc.panels.iter().map(|p| p.frame_count()).sum();
    let back = round_trip_lenient(&doc, "collage-brut");
    assert!(back.is_plans());
    assert_eq!(back.panels.len(), 3);
    assert_eq!(back.panels.iter().map(|p| p.frame_count()).sum::<usize>(), frames, "toutes les images y sont");
    assert_eq!(back.panels[0].marks.len(), 1, "le début du panneau collé est une marque");
}
#[test]
fn duplicated_and_split_plans_survive_a_save() {
    let mut doc = three_plans();
    doc.duplicate_scene_after(1);
    assert_eq!(doc.panels.len(), 4);
    assert!(doc.is_plans());
    let back = round_trip(&doc, "dupli");
    assert_eq!(summary(&back), summary(&doc));
}

#[test]
fn a_project_saved_with_duplicate_ids_loads_every_plan() {
    // Des plans collés qui auraient gardé les identifiants de l'original : même plan, même scène.
    let doc = three_plans();
    let dir = std::env::temp_dir().join(format!("pelure-test-doublons-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    let scenes = json["sequences"][0]["scenes"].as_array_mut().unwrap();
    let (scene_id, panel_id) = (scenes[0]["id"].clone(), scenes[0]["panneaux"][0]["id"].clone());
    for s in scenes.iter_mut().skip(1) {
        s["id"] = scene_id.clone();
        s["panneaux"][0]["id"] = panel_id.clone();
    }
    std::fs::write(&manifest, serde_json::to_string(&json).unwrap()).unwrap();
    let loaded = crate::project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(summary(&loaded.doc), summary(&doc), "aucun plan perdu");
    assert!(loaded.doc.is_plans());
    let mut ids: Vec<u64> = loaded.doc.panels.iter().map(|p| p.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "identifiants redevenus uniques");
    assert!(loaded.warnings.iter().any(|w| w.contains("renumérotés")), "{:?}", loaded.warnings);
}

#[test]
fn chosen_cut_and_joined_plans_survive_a_save() {
    let mut doc = three_plans();
    // Nouveau projet à partir des plans choisis.
    let ids = [doc.panels[0].id, doc.panels[2].id];
    let part = doc.from_plans(&ids).unwrap();
    assert!(part.is_plans());
    assert_eq!(summary(&round_trip(&part, "choisis")), summary(&part));
    assert_eq!(part.panels.len(), 2);
    // Couper un plan en deux, puis coller le second.
    let at = doc.split_plan(1, 10).unwrap();
    assert_eq!(doc.panels.len(), 4);
    assert!(doc.is_plans());
    let clip = doc.copy_panels(&[doc.panels[at].id]).unwrap();
    doc.paste_plans(&clip, at).unwrap();
    assert_eq!(doc.panels.len(), 5);
    assert_eq!(summary(&round_trip(&doc, "coupe")), summary(&doc));
}
