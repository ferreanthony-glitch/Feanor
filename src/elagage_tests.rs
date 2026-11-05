//! Élagage des dessins que plus rien ne retient (`elagage.rs`, `Gel::elaguer`) : un plan supprimé reste tant que l'annulation peut le rendre,
//! une version rangée aussi, et seuls les dessins déjà vus montrés partent ; l'annulation, après élagage, rend toujours des dessins exacts.

use std::collections::HashMap;

use eframe::egui::Color32;

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::perf_tests::banc;
use crate::raster::TILE_PIXELS;

fn doc_de_plans(n: usize) -> (Document, Vec<CelId>) {
    let mut doc = Document::new(256, 192, 24);
    doc.panels[0].layers = vec![Layer::new("B", LayerKind::Bitmap)];
    let mut cur = 0;
    for _ in 1..n {
        cur = doc.add_scene_after(cur);
    }
    let mut ids = Vec::new();
    for i in 0..doc.panels.len() {
        let c = doc.new_cel(LayerKind::Bitmap);
        {
            let cel = doc.cels.get_mut(&c).unwrap();
            for t in 0..3 {
                let tile = cel.raster.tile_mut(t);
                for k in 0..TILE_PIXELS {
                    if k % (3 + i) == 0 {
                        tile[k] = Color32::from_rgba_premultiplied(10 + i as u8, 20, 30, 255);
                    }
                }
            }
            cel.version += 1;
        }
        doc.panels[i].layers = vec![Layer::new("B", LayerKind::Bitmap)];
        doc.panels[i].layers[0].frames = vec![Some(c)];
        ids.push(c);
    }
    (doc, ids)
}

fn a_plat(doc: &Document, id: CelId) -> Vec<Color32> {
    doc.cels[&id].raster.to_flat()
}

/// Un passage d'élagage (sans attendre le délai) ; il faut deux passages de suite pour qu'un dessin sans attache parte.
fn passe(b: &mut crate::perf_tests::Banc) -> usize {
    b.app.gel.elagage_a_zero();
    let a = b.app.gel.elaguer(&mut b.app.doc, &b.app.history, true);
    crate::lacher::attendre(std::time::Duration::from_secs(10));
    a
}

/// Deux passages : ce que le second retire.
fn passes(b: &mut crate::perf_tests::Banc) -> usize {
    passe(b) + passe(b)
}

#[test]
fn a_removed_plan_stays_while_undo_can_bring_it_back_and_goes_when_nothing_holds_it() {
    let (doc, ids) = doc_de_plans(6);
    let avant: HashMap<CelId, Vec<Color32>> = ids.iter().map(|&id| (id, a_plat(&doc, id))).collect();
    let mut b = banc(doc, false);
    b.app.gel.activer(true);
    assert_eq!(passes(&mut b), 0);
    let cible = b.app.doc.panels[2].id;
    b.app.select_panel(2);
    b.app.structure_op(|d, _| d.remove_panels(&[cible]));
    assert!(b.app.doc.panels.iter().all(|p| p.id != cible));
    assert_eq!(passes(&mut b), 0, "l'annulation peut rendre le plan : son dessin reste");
    assert!(b.app.doc.cels.contains_key(&ids[2]));
    // Annuler après l'élagage : le plan est revenu, ses pixels sont exacts.
    b.app.undo();
    assert!(b.app.doc.panels.iter().any(|p| p.id == cible));
    for &id in &ids {
        assert_eq!(a_plat(&b.app.doc, id), avant[&id], "dessin {id} après annulation");
    }
    // Refaire la suppression, puis oublier l'historique : plus rien ne retient le dessin, il part ; les autres restent.
    b.app.redo();
    assert!(b.app.doc.panels.iter().all(|p| p.id != cible));
    b.app.history.clear();
    assert_eq!(passe(&mut b), 0, "premier passage : le dessin est noté sans attache");
    assert_eq!(passe(&mut b), 1, "second passage : il part");
    assert!(!b.app.doc.cels.contains_key(&ids[2]));
    for (i, &id) in ids.iter().enumerate() {
        if i != 2 {
            assert_eq!(a_plat(&b.app.doc, id), avant[&id], "dessin {id} resté intact");
        }
    }
    assert_eq!(b.app.gel.elagues, 1);
}

#[test]
fn a_temporary_drawing_is_never_pruned_and_an_orphan_waits_one_pass() {
    let (doc, _) = doc_de_plans(3);
    let mut b = banc(doc, false);
    b.app.gel.activer(true);
    assert_eq!(passes(&mut b), 0);
    // Un dessin temporaire (comme la feuille flottante d'une sélection) : jamais dans un plan.
    let temporaire = crate::flottante::BASE_CEL;
    let c = crate::document::Cel::new(256, 192, LayerKind::Bitmap);
    b.app.doc.cels.insert(temporaire, c);
    // Un dessin neuf pas encore posé dans un plan : il attend un passage, puis part.
    let orphelin = b.app.doc.new_cel(LayerKind::Bitmap);
    assert_eq!(passe(&mut b), 0);
    assert!(b.app.doc.cels.contains_key(&orphelin), "un passage ne suffit pas");
    assert_eq!(passe(&mut b), 1);
    assert!(!b.app.doc.cels.contains_key(&orphelin));
    assert!(b.app.doc.cels.contains_key(&temporaire), "le dessin temporaire reste");
}

#[test]
fn an_older_version_of_a_plan_keeps_its_drawings() {
    let (doc, ids) = doc_de_plans(3);
    let mut b = banc(doc, false);
    b.app.gel.activer(true);
    assert_eq!(passes(&mut b), 0);
    // Une nouvelle version du plan 1 : l'ancienne est rangée dans `versions`.
    b.app.structure_op(|d, _| {
        d.nouvelle_version(1, 1)?;
        Some(1)
    });
    b.app.history.clear();
    passes(&mut b);
    assert!(b.app.doc.cels.contains_key(&ids[1]), "le dessin de la version rangée reste");
    let rangee = b.app.doc.plans_ranges().count();
    assert_eq!(rangee, 1);
}

#[test]
fn a_shared_project_is_never_pruned() {
    let (mut doc, ids) = doc_de_plans(3);
    doc.set_id_prefix(7);
    let mut b = banc(doc, false);
    b.app.gel.activer(true);
    assert_eq!(passes(&mut b), 0);
    let cible = b.app.doc.panels[1].id;
    b.app.structure_op(|d, _| d.remove_panels(&[cible]));
    b.app.history.clear();
    assert_eq!(passes(&mut b), 0);
    assert!(b.app.doc.cels.contains_key(&ids[1]));
}

#[test]
fn no_pruning_while_something_is_going_on() {
    let (doc, ids) = doc_de_plans(3);
    let mut b = banc(doc, false);
    b.app.gel.activer(true);
    passes(&mut b);
    let cible = b.app.doc.panels[1].id;
    b.app.structure_op(|d, _| d.remove_panels(&[cible]));
    b.app.history.clear();
    for _ in 0..2 {
        b.app.gel.elagage_a_zero();
        assert_eq!(b.app.gel.elaguer(&mut b.app.doc, &b.app.history, false), 0, "geste ou lecture en cours");
    }
    assert!(b.app.doc.cels.contains_key(&ids[1]));
    assert_eq!(passes(&mut b), 1, "au calme, il part");
}
